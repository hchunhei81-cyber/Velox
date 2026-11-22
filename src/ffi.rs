use crate::ast::Ty;
use std::path::Path;
use std::process::Command;

pub struct FfiFn {
    pub name: String,
    pub params: Vec<Ty>,
    pub ret: Ty,
}

pub fn extract_functions(header: &Path) -> Result<Vec<FfiFn>, String> {
    if header.file_name().and_then(|n| n.to_str()) == Some("math.h") {
        return Ok(math_functions());
    }

    let mut cmd = Command::new("clang");
    cmd.args(["-Xclang", "-ast-dump=json", "-fsyntax-only"]);

    if header.exists() {
        cmd.arg(header);
    } else if let Some(name) = header.to_str().filter(|n| is_system_header(n)) {
        cmd.args(["-x", "c", "-include", name, "/dev/null"]);
    } else {
        return Err(format!("E0200: 找不到头文件 `{}`", header.display()));
    }

    let out = cmd
        .output()
        .map_err(|e| format!("E0202: clang 调用失败: {}", e))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("E0202: clang ast-dump 失败: {}", stderr.trim()));
    }
    let json: serde_json::Value =
        serde_json::from_slice(&out.stdout).map_err(|e| format!("E0202: JSON 解析失败: {}", e))?;
    let mut fns = Vec::new();
    walk(&json, &mut fns, header);
    Ok(fns)
}

fn walk(node: &serde_json::Value, fns: &mut Vec<FfiFn>, header: &Path) {
    if node.get("kind").and_then(|v| v.as_str()) == Some("FunctionDecl") {
        if let Some(f) = parse_fn(node, header) {
            // 去重
            if !fns.iter().any(|x| x.name == f.name) {
                fns.push(f);
            }
        }
    }
    if let Some(inner) = node.get("inner").and_then(|v| v.as_array()) {
        for c in inner {
            walk(c, fns, header);
        }
    }
}

fn parse_fn(node: &serde_json::Value, header: &Path) -> Option<FfiFn> {
    // 普通项目头文件只提取自身声明。
    // 系统头文件的 Clang AST 节点可能省略 loc.file，不能据此过滤函数。
    if !header.to_str().map(is_system_header).unwrap_or(false) {
        let file = node.pointer("/loc/file").and_then(|v| v.as_str())?;
        let hname = header.file_name()?.to_str()?;
        if !file.ends_with(hname) {
            return None;
        }
    }

    // 排除隐式 / static inline 等
    let storage = node
        .get("storageClass")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if storage == "static" {
        return None;
    }

    let name = node.get("name")?.as_str()?.to_string();
    let qual = node.pointer("/type/qualType")?.as_str()?;
    // qual 形如 "int (int, double)" 或 "void (void)"
    let paren = qual.find('(')?;
    let ret_src = qual[..paren].trim();
    let params_src = qual[paren + 1..].trim_end_matches(')');

    let ret = parse_c_ty(ret_src)?;
    let mut params = Vec::new();
    let ps = params_src.trim();
    if !ps.is_empty() && ps != "void" {
        for p in ps.split(',') {
            params.push(parse_c_ty(p.trim())?);
        }
    }
    Some(FfiFn { name, params, ret })
}

fn parse_c_ty(s: &str) -> Option<Ty> {
    // 去 const/restrict/volatile
    let s: String = s
        .split_whitespace()
        .filter(|w| !matches!(*w, "const" | "volatile" | "restrict" | "__restrict"))
        .collect::<Vec<_>>()
        .join(" ");
    let s = s.trim();
    match s {
        "void" => Some(Ty::Void),
        "int" | "long" | "long long" | "int64_t" | "size_t" | "ssize_t" | "unsigned int"
        | "unsigned long" | "uint64_t" | "int32_t" => Some(Ty::I64),
        "double" | "float" => Some(Ty::F64),
        "char *" | "char*" => Some(Ty::Str),
        _ => None,
    }
}

fn math_functions() -> Vec<FfiFn> {
    let unary = [
        "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "exp", "exp2",
        "expm1", "log", "log10", "log2", "log1p", "sqrt", "cbrt", "fabs", "ceil", "floor", "round",
        "trunc",
    ];

    let binary = [
        "atan2",
        "pow",
        "fmod",
        "remainder",
        "hypot",
        "copysign",
        "fmin",
        "fmax",
    ];

    let mut functions = Vec::new();

    for name in unary {
        functions.push(FfiFn {
            name: name.to_string(),
            params: vec![Ty::F64],
            ret: Ty::F64,
        });
    }

    for name in binary {
        functions.push(FfiFn {
            name: name.to_string(),
            params: vec![Ty::F64, Ty::F64],
            ret: Ty::F64,
        });
    }

    functions
}

pub fn is_system_header(name: &str) -> bool {
    matches!(name, "math.h" | "pthread.h" | "dlfcn.h")
}

pub fn lib_for_header(name: &str) -> Option<&'static str> {
    match name {
        "math.h" => Some("m"),
        "pthread.h" => Some("pthread"),
        "dlfcn.h" => Some("dl"),
        _ => None,
    }
}
