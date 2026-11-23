fn add_python_link_flags(cmd: &mut Command) -> Result<(), String> {
    let includes = Command::new("python3-config")
        .arg("--includes")
        .output()
        .map_err(|e| format!("Python embedding requires `python3-config`: {}", e))?;
    if !includes.status.success() {
        return Err("python3-config --includes failed".into());
    }
    cmd.args(String::from_utf8_lossy(&includes.stdout).split_whitespace());

    let libdir = Command::new("python3")
        .args([
            "-c",
            "import sysconfig; print(sysconfig.get_config_var('LIBDIR') or '')",
        ])
        .output()
        .map_err(|e| format!("failed to query Python library directory: {}", e))?;
    if !libdir.status.success() {
        return Err("failed to query Python library directory".into());
    }

    let path = String::from_utf8_lossy(&libdir.stdout).trim().to_string();
    if path.is_empty() {
        return Err("Python LIBDIR is empty".into());
    }
    cmd.arg(format!("-L{}", path));
    cmd.arg(format!("-Wl,-rpath,{}", path));

    let ldflags = Command::new("python3-config")
        .args(["--ldflags", "--embed"])
        .output()
        .map_err(|e| format!("failed to query Python linker flags: {}", e))?;
    if !ldflags.status.success() {
        return Err("python3-config --ldflags --embed failed".into());
    }
    cmd.args(String::from_utf8_lossy(&ldflags.stdout).split_whitespace());

    Ok(())
}

use crate::ast::{FnDef, Program, StructDef, Vis};
use crate::ffi::{self, FfiFn};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// Merge declarations from multiple headers before type checking and codegen.
// Identical declarations share one C symbol; incompatible ABI declarations fail.
fn merge_extern_fn(functions: &mut Vec<FnDef>, candidate: FnDef) -> Result<(), String> {
    if let Some(existing) = functions.iter().find(|f| f.name == candidate.name) {
        let same_params = existing.params.len() == candidate.params.len()
            && existing
                .params
                .iter()
                .zip(&candidate.params)
                .all(|((_, a), (_, b))| a == b);
        let same_signature = same_params && existing.ret == candidate.ret;

        if same_signature {
            return Ok(());
        }

        return Err(format!(
            "E0203: conflicting declarations for extern function `{}` in `{}` and `{}`",
            candidate.name, existing.origin, candidate.origin
        ));
    }

    functions.push(candidate);
    Ok(())
}

pub fn compile(src_path: &str, out: &str) -> Result<(), String> {
    let output_path = Path::new(out);

    let mut visited = HashSet::new();
    let mut active = HashSet::new();
    let (vx_files, h_files) = collect_files(Path::new(src_path), &mut visited, &mut active)?;

    // Never allow the executable output to overwrite a source or header file.
    if let Ok(output_canonical) = output_path.canonicalize() {
        for input in vx_files.iter().chain(h_files.iter()) {
            if input.canonicalize().ok().as_ref() == Some(&output_canonical) {
                return Err(format!(
                    "E0204: output path `{}` conflicts with an input file",
                    out
                ));
            }
        }
    }

    // Remove stale output only after verifying it cannot overwrite an input.
    if output_path.exists() {
        fs::remove_file(output_path)
            .map_err(|e| format!("failed to remove previous output `{}`: {}", out, e))?;
    }

    // 1. Parse .vx source files
    let mut vx_fns = Vec::new();
    let mut vx_structs: Vec<StructDef> = Vec::new();
    let mut python_imports = Vec::new();
    for f in &vx_files {
        let raw = fs::read_to_string(f)
            .map_err(|e| format!("E0200: cannot read `{}`: {}", f.display(), e))?;
        let stripped = strip_using(&raw);
        let toks = crate::lexer::Lexer::new(&stripped).tokenize()?;
        let mut prog = crate::parser::Parser::new(toks).program()?;
        let origin = f.to_string_lossy().to_string();
        for fd in &mut prog.fns {
            fd.origin = origin.clone();
            fd.is_extern = false;
        }
        for sd in &mut prog.structs {
            sd.origin = origin.clone();
        }
        python_imports.extend(prog.python_imports);
        vx_fns.extend(prog.fns);
        vx_structs.extend(prog.structs);
    }

    // 2. Extract FFI functions from .h headers
    let mut extern_fns: Vec<FnDef> = Vec::new();
    for h in &h_files {
        let raw = ffi::extract_functions(h)?;
        for f in raw {
            merge_extern_fn(
                &mut extern_fns,
                FnDef {
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
                },
            )?;
        }
    }

    // 3. Perform type checking
    let prog = Program {
        fns: vx_fns,
        structs: vx_structs,
        python_imports,
    };
    let mut checker = crate::types::Checker::new();
    for f in &extern_fns {
        checker.register_extern(
            &f.name,
            f.params.iter().map(|(_, t)| t.clone()).collect(),
            f.ret.clone(),
        )?;
    }
    checker.check(&prog)?;

    // 4. Merge extern and user functions for code generation
    let mut all = extern_fns;
    all.extend(prog.fns);
    let prog = Program {
        fns: all,
        structs: prog.structs,
        python_imports: prog.python_imports,
    };
    let c = crate::codegen::c::CEmitter::new().emit(&prog)?;

    // 5. Generate C source and compile it
    let cfile = format!("{}.c", out);
    fs::write(&cfile, &c).map_err(|e| format!("cannot write {}: {}", cfile, e))?;

    let openmp = has_openmp();
    let mut cmd = Command::new("clang");
    cmd.arg("-O2");
    if !prog.python_imports.is_empty() {
        add_python_link_flags(&mut cmd)?;
    }
    if openmp {
        cmd.arg("-fopenmp");
    } else {
        cmd.arg("-Wno-unknown-pragmas");
    }
    for h in &h_files {
        if h.file_name().is_some() {
            if let Some(lib) = ffi::lib_for_header(h) {
                cmd.arg(format!("-l{}", lib));
            }
        }
    }
    cmd.args(["-o", out, &cfile]);
    let status = cmd
        .status()
        .map_err(|e| format!("failed to start clang: {}", e))?;
    if !status.success() {
        return Err(format!("clang compilation failed: {}", cfile));
    }
    Ok(())
}

/// --py mode: generate _<stem>.dylib and <stem>.py
pub fn compile_py(src_path: &str, out_dir: &str) -> Result<(), String> {
    let mut visited = HashSet::new();
    let mut active = HashSet::new();
    let (vx_files, h_files) = collect_files(Path::new(src_path), &mut visited, &mut active)?;

    let mut vx_fns = Vec::new();
    let mut vx_structs: Vec<StructDef> = Vec::new();
    let mut python_imports = Vec::new();
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
                if fd
                    .params
                    .iter()
                    .any(|(_, t)| matches!(t, crate::ast::Ty::Struct(_)))
                    || matches!(&fd.ret, crate::ast::Ty::Struct(_))
                {
                    return Err(format!(
                        "E0501: --py does not yet support exporting functions that use struct ABI `{}`",
                        fd.name
                    ));
                }
                exports.push(FfiFn {
                    name: fd.name.clone(),
                    params: fd.params.iter().map(|(_, t)| t.clone()).collect(),
                    ret: fd.ret.clone(),
                });
            }
        }
        for sd in &mut prog.structs {
            sd.origin = origin.clone();
        }
        python_imports.extend(prog.python_imports);
        vx_fns.extend(prog.fns);
        vx_structs.extend(prog.structs);
    }

    let mut extern_fns: Vec<FnDef> = Vec::new();
    for h in &h_files {
        let raw = ffi::extract_functions(h)?;
        for f in raw {
            merge_extern_fn(
                &mut extern_fns,
                FnDef {
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
                },
            )?;
        }
    }

    if !python_imports.is_empty() {
        return Err("E0503: Python imports are not supported in `--py` mode yet".into());
    }

    let prog = Program {
        fns: vx_fns,

        structs: vx_structs,

        python_imports,
    };
    let mut checker = crate::types::Checker::new();
    for f in &extern_fns {
        checker.register_extern(
            &f.name,
            f.params.iter().map(|(_, t)| t.clone()).collect(),
            f.ret.clone(),
        )?;
    }
    checker.check(&prog)?;

    let mut all = extern_fns;
    all.extend(prog.fns);
    let prog = Program {
        fns: all,
        structs: prog.structs,
        python_imports: prog.python_imports,
    };
    let c = crate::codegen::c::CEmitter::new().emit(&prog)?;

    if exports.is_empty() {
        return Err("E0500: --py mode requires at least one `export fn`".into());
    }

    let stem = Path::new(src_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "invalid filename".to_string())?;
    let libname = format!("_{}.dylib", stem);
    let cfile_path = Path::new(out_dir).join(format!("{}.c", stem));
    let libpath = Path::new(out_dir).join(&libname);
    let pypath = Path::new(out_dir).join(format!("{}.py", stem));

    fs::write(&cfile_path, &c).map_err(|e| format!("failed to write C source file: {}", e))?;

    let mut cmd = Command::new("clang");
    cmd.args(["-O2", "-shared", "-fPIC"]);
    if has_openmp() {
        cmd.arg("-fopenmp");
    } else {
        cmd.arg("-Wno-unknown-pragmas");
    }
    for h in &h_files {
        if h.file_name().is_some() {
            if let Some(lib) = ffi::lib_for_header(h) {
                cmd.arg(format!("-l{}", lib));
            }
        }
    }
    cmd.arg("-o").arg(&libpath).arg(&cfile_path);
    let status = cmd
        .status()
        .map_err(|e| format!("clang compilation failed: {}", e))?;
    if !status.success() {
        return Err("clang shared-library compilation failed".into());
    }

    let py = crate::python::generate_wrapper(&libname, &exports);
    fs::write(&pypath, py).map_err(|e| format!("failed to write Python wrapper: {}", e))?;

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

/// Collect .vx source files and .h headers using depth-first traversal (dependencies first).
fn collect_files(
    path: &Path,
    visited: &mut HashSet<PathBuf>,
    active: &mut HashSet<PathBuf>,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let canonical = path
        .canonicalize()
        .map_err(|e| format!("E0200: cannot find `{}`: {}", path.display(), e))?;

    if active.contains(&canonical) {
        return Err(format!(
            "E0203: circular dependency detected `{}`",
            canonical.display()
        ));
    }

    if visited.contains(&canonical) {
        return Ok((Vec::new(), Vec::new()));
    }

    active.insert(canonical.clone());
    let dir = canonical.parent().unwrap_or_else(|| Path::new("."));
    let src = fs::read_to_string(&canonical)
        .map_err(|e| format!("E0200: cannot read `{}`: {}", path.display(), e))?;

    let mut vx = Vec::new();
    let mut h = Vec::new();
    for line in src.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("using \"") {
            let dep = match rest.find('"') {
                Some(i) => &rest[..i],
                None => return Err(format!("malformed using directive: {}", line)),
            };
            let dep_path = dir.join(dep);
            if dep.ends_with(".h") {
                let dc = match dep_path.canonicalize() {
                    Ok(path) => path,
                    Err(_) if ffi::is_system_header(dep) => PathBuf::from(dep),
                    Err(e) => {
                        return Err(format!(
                            "E0200: cannot find header file `{}`: {}",
                            dep_path.display(),
                            e
                        ));
                    }
                };
                if visited.insert(dc.clone()) {
                    h.push(dc);
                }
            } else {
                let (dv, dh) = collect_files(&dep_path, visited, active)?;
                vx.extend(dv);
                h.extend(dh);
            }
        }
    }
    active.remove(&canonical);
    visited.insert(canonical.clone());
    vx.push(canonical);
    Ok((vx, h))
}

fn has_openmp() -> bool {
    // Allow OpenMP to be explicitly disabled when testing the serial fallback.
    if std::env::var_os("VELOX_DISABLE_OPENMP").is_some() {
        return false;
    }

    // Use separate files for each compiler process to prevent parallel tests from overwriting one another.
    let stem = format!("_velox_omp_test_{}", std::process::id());
    let test = std::env::temp_dir().join(format!("{}.c", stem));
    let binary = std::env::temp_dir().join(&stem);

    if fs::write(&test, "int main(){return 0;}").is_err() {
        return false;
    }

    let ok = Command::new("clang")
        .arg("-fopenmp")
        .arg("-o")
        .arg(&binary)
        .arg(&test)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    let _ = fs::remove_file(&test);
    let _ = fs::remove_file(&binary);
    ok
}

#[cfg(test)]
mod extern_merge_tests {
    use super::*;
    use crate::ast::Ty;

    fn declaration(origin: &str, params: Vec<Ty>, ret: Ty) -> FnDef {
        FnDef {
            name: "shared_api".into(),
            vis: Vis::Export,
            origin: origin.into(),
            is_extern: true,
            params: params
                .into_iter()
                .enumerate()
                .map(|(i, ty)| (format!("a{i}"), ty))
                .collect(),
            ret,
            body: Vec::new(),
        }
    }

    #[test]
    fn identical_extern_declarations_across_headers_are_merged() {
        let mut functions = Vec::new();

        merge_extern_fn(
            &mut functions,
            declaration("first.h", vec![Ty::I64, Ty::Str], Ty::I64),
        )
        .unwrap();
        merge_extern_fn(
            &mut functions,
            declaration("second.h", vec![Ty::I64, Ty::Str], Ty::I64),
        )
        .unwrap();

        assert_eq!(functions.len(), 1);
        assert_eq!(functions[0].origin, "first.h");
    }

    #[test]
    fn conflicting_extern_declarations_across_headers_are_rejected() {
        let mut functions = Vec::new();

        merge_extern_fn(
            &mut functions,
            declaration("first.h", vec![Ty::I64], Ty::I64),
        )
        .unwrap();

        let error = merge_extern_fn(
            &mut functions,
            declaration("second.h", vec![Ty::F64], Ty::I64),
        )
        .unwrap_err();

        assert!(error.contains("E0203"));
        assert!(error.contains("shared_api"));
        assert!(error.contains("first.h"));
        assert!(error.contains("second.h"));
        assert_eq!(functions.len(), 1);
    }
}


#[cfg(test)]
mod dependency_regression_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn test_dir() -> PathBuf {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);

        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "velox-deps-test-{}-{}",
                std::process::id(),
                id
            ));

            match fs::create_dir(&dir) {
                Ok(()) => return dir,
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("failed to create test directory: {}", error),
            }
        }
    }

    #[test]
    fn duplicate_dependencies_are_collected_once() {
        let dir = test_dir();
        let math = dir.join("math.vx");
        let main = dir.join("main.vx");

        fs::write(&math, "pub fn add(a: i64, b: i64) -> i64 { return a + b; }\n").unwrap();
        fs::write(
            &main,
            "using \"math.vx\";\nusing \"math.vx\";\nfn main() {}\n",
        ).unwrap();

        let (files, _) = collect_files(
            &main,
            &mut HashSet::new(),
            &mut HashSet::new(),
        ).unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(files[0], math.canonicalize().unwrap());
        assert_eq!(files[1], main.canonicalize().unwrap());

        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn circular_dependencies_return_e0203() {
        let dir = test_dir();
        let a = dir.join("a.vx");
        let b = dir.join("b.vx");

        fs::write(&a, "using \"b.vx\";\nfn main() {}\n").unwrap();
        fs::write(&b, "using \"a.vx\";\n").unwrap();

        let result = collect_files(
            &a,
            &mut HashSet::new(),
            &mut HashSet::new(),
        );

        let error = result.unwrap_err();
        assert!(error.contains("E0203"), "unexpected error: {}", error);

        fs::remove_dir_all(dir).unwrap();
    }
}
