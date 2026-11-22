use crate::ast::{FnDef, Program, Vis};
use crate::ffi::{self, FfiFn};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn compile(src_path: &str, out: &str) -> Result<(), String> {
    let mut visited = HashSet::new();
    let (vx_files, h_files) = collect_files(Path::new(src_path), &mut visited)?;

    // 1. 解析 .vx 文件
    let mut vx_fns = Vec::new();
    for f in &vx_files {
        let raw =
            fs::read_to_string(f).map_err(|e| format!("E0200: 读不到 `{}`: {}", f.display(), e))?;
        let stripped = strip_using(&raw);
        let toks = crate::lexer::Lexer::new(&stripped).tokenize()?;
        let mut prog = crate::parser::Parser::new(toks).program()?;
        let origin = f.to_string_lossy().to_string();
        for fd in &mut prog.fns {
            fd.origin = origin.clone();
            fd.is_extern = false;
        }
        vx_fns.extend(prog.fns);
    }

    // 2. 从 .h 提取 FFI 函数
    let mut extern_fns: Vec<FnDef> = Vec::new();
    for h in &h_files {
        let raw = ffi::extract_functions(h)?;
        for f in raw {
            extern_fns.push(FnDef {
                name: f.name,
                vis: Vis::Export,
                origin: h.to_string_lossy().to_string(),
                is_extern: true,
                params: f
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (format!("a{}", i), t.clone()))
                    .collect(),
                ret: f.ret,
                body: Vec::new(),
            });
        }
    }

    // 3. 类型检查
    let prog = Program { fns: vx_fns };
    let mut checker = crate::types::Checker::new();
    for f in &extern_fns {
        checker.register_extern(
            &f.name,
            f.params.iter().map(|(_, t)| t.clone()).collect(),
            f.ret.clone(),
        );
    }
    checker.check(&prog)?;

    // 4. 合并 extern + user fns，交给 codegen
    let mut all = extern_fns;
    all.extend(prog.fns);
    let prog = Program { fns: all };
    let c = crate::codegen::c::CEmitter::new().emit(&prog)?;

    // 5. 写 C + 编译
    let cfile = format!("{}.c", out);
    fs::write(&cfile, &c).map_err(|e| format!("cannot write {}: {}", cfile, e))?;

    let openmp = has_openmp();
    let mut cmd = Command::new("clang");
    cmd.arg("-O2");
    if openmp {
        cmd.arg("-fopenmp");
    } else {
        cmd.arg("-Wno-unknown-pragmas");
    }
    for h in &h_files {
        if let Some(name) = h.file_name().and_then(|s| s.to_str()) {
            if let Some(lib) = ffi::lib_for_header(name) {
                cmd.arg(format!("-l{}", lib));
            }
        }
    }
    cmd.args(["-o", out, &cfile]);
    let status = cmd.status().map_err(|e| format!("clang 启动失败: {}", e))?;
    if !status.success() {
        return Err(format!("clang 失败: {}", cfile));
    }
    Ok(())
}

/// --py 模式：生成 _<stem>.dylib 和 <stem>.py
pub fn compile_py(src_path: &str, out_dir: &str) -> Result<(), String> {
    let mut visited = HashSet::new();
    let (vx_files, h_files) = collect_files(Path::new(src_path), &mut visited)?;

    let mut vx_fns = Vec::new();
    let mut exports: Vec<FfiFn> = Vec::new();
    for f in &vx_files {
        let raw = fs::read_to_string(f).map_err(|e| format!("E0200: {}", e))?;
        let stripped = strip_using(&raw);
        let toks = crate::lexer::Lexer::new(&stripped).tokenize()?;
        let mut prog = crate::parser::Parser::new(toks).program()?;
        let origin = f.to_string_lossy().to_string();
        for fd in &mut prog.fns {
            fd.origin = origin.clone();
            fd.is_extern = false;
            if fd.vis == Vis::Export {
                exports.push(FfiFn {
                    name: fd.name.clone(),
                    params: fd.params.iter().map(|(_, t)| t.clone()).collect(),
                    ret: fd.ret.clone(),
                });
            }
        }
        vx_fns.extend(prog.fns);
    }

    let mut extern_fns: Vec<FnDef> = Vec::new();
    for h in &h_files {
        let raw = ffi::extract_functions(h)?;
        for f in raw {
            extern_fns.push(FnDef {
                name: f.name,
                vis: Vis::Export,
                origin: h.to_string_lossy().to_string(),
                is_extern: true,
                params: f
                    .params
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (format!("a{}", i), t.clone()))
                    .collect(),
                ret: f.ret,
                body: Vec::new(),
            });
        }
    }

    let prog = Program { fns: vx_fns };
    let mut checker = crate::types::Checker::new();
    for f in &extern_fns {
        checker.register_extern(
            &f.name,
            f.params.iter().map(|(_, t)| t.clone()).collect(),
            f.ret.clone(),
        );
    }
    checker.check(&prog)?;

    let mut all = extern_fns;
    all.extend(prog.fns);
    let prog = Program { fns: all };
    let c = crate::codegen::c::CEmitter::new().emit(&prog)?;

    if exports.is_empty() {
        return Err("E0500: --py 模式需要至少一个 `export fn`".into());
    }

    let stem = Path::new(src_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "invalid filename".to_string())?;
    let libname = format!("_{}.dylib", stem);
    let cfile_path = Path::new(out_dir).join(format!("{}.c", stem));
    let libpath = Path::new(out_dir).join(&libname);
    let pypath = Path::new(out_dir).join(format!("{}.py", stem));

    fs::write(&cfile_path, &c).map_err(|e| format!("写 .c 失败: {}", e))?;

    let mut cmd = Command::new("clang");
    cmd.args(["-O2", "-shared", "-fPIC"]);
    if has_openmp() {
        cmd.arg("-fopenmp");
    } else {
        cmd.arg("-Wno-unknown-pragmas");
    }
    for h in &h_files {
        if let Some(name) = h.file_name().and_then(|s| s.to_str()) {
            if let Some(lib) = ffi::lib_for_header(name) {
                cmd.arg(format!("-l{}", lib));
            }
        }
    }
    cmd.args([
        "-o",
        libpath.to_str().unwrap(),
        cfile_path.to_str().unwrap(),
    ]);
    let status = cmd.status().map_err(|e| format!("clang 失败: {}", e))?;
    if !status.success() {
        return Err("clang -shared 失败".into());
    }

    let py = crate::python::generate_wrapper(&libname, &exports);
    fs::write(&pypath, py).map_err(|e| format!("写 .py 失败: {}", e))?;

    println!("generated: {}", libpath.display());
    println!("generated: {}", pypath.display());
    Ok(())
}

fn strip_using(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("using \""))
        .map(|l| format!("{}\n", l))
        .collect()
}

/// 深度优先收集：vx 文件、h 头文件（顺序：被依赖的在前）
fn collect_files(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("E0200: 找不到 `{}`: {}", path.display(), e))?;
    if !visited.insert(canonical.clone()) {
        // 已访问过——可能是 DAG 共享依赖，不算环；只有递归栈内的才是环。
        // 简化：visited 里就看当作已处理，返回空。
        return Ok((Vec::new(), Vec::new()));
    }
    let dir = canonical.parent().unwrap_or_else(|| Path::new("."));
    let src = fs::read_to_string(&canonical)
        .map_err(|e| format!("E0200: 读不到 `{}`: {}", path.display(), e))?;

    let mut vx = Vec::new();
    let mut h = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("using \"") {
            let dep = match rest.find('"') {
                Some(i) => &rest[..i],
                None => return Err(format!("malformed using: {}", line)),
            };
            let dep_path = dir.join(dep);
            if dep.ends_with(".h") {
                let dc = match dep_path.canonicalize() {
                    Ok(path) => path,
                    Err(_) if ffi::is_system_header(dep) => PathBuf::from(dep),
                    Err(e) => {
                        return Err(format!(
                            "E0200: 找不到头文件 `{}`: {}",
                            dep_path.display(),
                            e
                        ));
                    }
                };
                if visited.insert(dc.clone()) {
                    h.push(dc);
                }
            } else {
                let (dv, dh) = collect_files(&dep_path, visited)?;
                vx.extend(dv);
                h.extend(dh);
            }
        }
    }
    vx.push(canonical);
    Ok((vx, h))
}

fn has_openmp() -> bool {
    let test = "/tmp/_velox_omp_test.c";
    if fs::write(test, "int main(){return 0;}").is_err() {
        return false;
    }
    Command::new("clang")
        .args(["-fopenmp", "-o", "/tmp/_velox_omp_test", test])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
