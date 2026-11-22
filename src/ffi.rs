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
        return Err(format!(
            "E0200: cannot find header file `{}`",
            header.display()
        ));
    }

    let out = cmd
        .output()
        .map_err(|e| format!("E0202: failed to invoke clang: {}", e))?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("E0202: clang AST dump failed: {}", stderr.trim()));
    }
    let json: serde_json::Value = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("E0202: failed to parse clang JSON output: {}", e))?;
    let mut fns = Vec::new();
    walk(&json, &mut fns, header);
    Ok(fns)
}

fn walk(node: &serde_json::Value, fns: &mut Vec<FfiFn>, header: &Path) {
    if node.get("kind").and_then(|v| v.as_str()) == Some("FunctionDecl") {
        if let Some(f) = parse_fn(node, header) {
            // Remove duplicate declarations
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
    // For project headers, extract declarations originating from the header itself.
    // Clang AST nodes from system headers may omit loc.file; do not use that field alone to filter functions.
    if !header.to_str().map(is_system_header).unwrap_or(false) {
        let file = node.pointer("/loc/file").and_then(|v| v.as_str())?;
        let hname = header.file_name()?.to_str()?;
        if !file.ends_with(hname) {
            return None;
        }
    }

    // Exclude implicit declarations, static inline functions, and unsupported declarations.
    let storage = node
        .get("storageClass")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if storage == "static" {
        return None;
    }

    let name = node.get("name")?.as_str()?.to_string();
    let qual = node.pointer("/type/qualType")?.as_str()?;
    // qual has a form such as "int (int, double)" or "void (void)"
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
    // Remove const, restrict, and volatile qualifiers.
    let s: String = s
        .split_whitespace()
        .filter(|w| !matches!(*w, "const" | "volatile" | "restrict" | "__restrict"))
        .collect::<Vec<_>>()
        .join(" ");
    let s = s.trim();
    match s {
        "void" => Some(Ty::Void),
        "_Bool" | "bool" => Some(Ty::Bool),
        // Accept only types whose ABI compatibility can be guaranteed by the current Velox type system.
        // C int, float, int32_t, unsigned int, and related types remain unsupported until i32/f32 support is implemented.
        "long" | "long long" | "int64_t" | "size_t" | "ssize_t" | "unsigned long" | "uint64_t" => {
            Some(Ty::I64)
        }
        "double" => Some(Ty::F64),
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
