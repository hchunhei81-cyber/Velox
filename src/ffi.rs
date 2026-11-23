use crate::ast::Ty;
use std::fs;
use std::path::Path;
use std::process::Command;

pub struct FfiFn {
    pub name: String,
    pub params: Vec<Ty>,
    pub ret: Ty,
}

pub fn extract_functions(header: &Path) -> Result<Vec<FfiFn>, String> {
    let system_alias = !header.exists() && header.to_str().is_some_and(is_system_header);

    // Built-in declarations apply only to the unresolved system-header alias.
    // A real project file named math.h must be parsed normally.
    if system_alias && header.to_str() == Some("math.h") {
        return Ok(math_functions());
    }

    let target_header = if header.exists() {
        fs::canonicalize(header).map_err(|e| {
            format!(
                "E0200: cannot resolve header file `{}`: {}",
                header.display(),
                e
            )
        })?
    } else if system_alias {
        resolve_system_header(header)?
    } else {
        return Err(format!(
            "E0200: cannot find header file `{}`",
            header.display()
        ));
    };

    let mut cmd = Command::new("clang");
    cmd.args(["-Xclang", "-ast-dump=json", "-fsyntax-only"]);

    if header.exists() {
        cmd.arg(header);
    } else {
        let name = header
            .to_str()
            .ok_or_else(|| format!("E0200: invalid system header path `{}`", header.display()))?;
        cmd.args(["-x", "c", "-include", name, "/dev/null"]);
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
    let mut current_file: Option<String> = None;
    walk(&json, &mut fns, &target_header, &mut current_file)?;
    Ok(fns)
}

// Ask Clang's dependency generator which physical file an unresolved system
// header alias resolves to. This avoids basename-based source attribution.
fn resolve_system_header(header: &Path) -> Result<std::path::PathBuf, String> {
    let name = header
        .to_str()
        .ok_or_else(|| format!("E0200: invalid system header name `{}`", header.display()))?;

    let out = Command::new("clang")
        .args([
            "-M",
            "-MT",
            "velox_probe",
            "-x",
            "c",
            "-include",
            name,
            "/dev/null",
        ])
        .output()
        .map_err(|e| format!("E0202: failed to resolve system header `{}`: {}", name, e))?;

    if !out.status.success() {
        return Err(format!(
            "E0202: clang could not resolve system header `{}`: {}",
            name,
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    let dependencies = parse_make_dependencies(&String::from_utf8_lossy(&out.stdout))?;
    let wanted = header.file_name().and_then(|s| s.to_str()).unwrap_or(name);
    let mut matches = dependencies
        .iter()
        .filter(|dependency| {
            Path::new(dependency).file_name().and_then(|s| s.to_str()) == Some(wanted)
        })
        .map(Path::new);

    let first = matches
        .next()
        .ok_or_else(|| format!("E0200: Clang dependency output did not contain `{}`", name))?;

    if matches.next().is_some() {
        return Err(format!(
            "E0200: Clang resolved `{}` to multiple files; refusing ambiguous header origin",
            name
        ));
    }

    fs::canonicalize(first).map_err(|e| {
        format!(
            "E0200: cannot canonicalize resolved system header `{}`: {}",
            first.display(),
            e
        )
    })
}

// Parse Makefile-style escaped dependency paths, including escaped spaces.
fn parse_make_dependencies(source: &str) -> Result<Vec<String>, String> {
    let flattened = source.replace("\\\r\n", " ").replace("\\\n", " ");
    let (_, dependencies) = flattened.split_once(':').ok_or_else(|| {
        "E0202: malformed clang dependency output (missing target separator)".to_string()
    })?;

    let mut result = Vec::new();
    let mut token = String::new();
    let mut escaped = false;

    for ch in dependencies.chars() {
        if escaped {
            token.push(ch);
            escaped = false;
        } else {
            match ch {
                '\\' => escaped = true,
                c if c.is_whitespace() => {
                    if !token.is_empty() {
                        result.push(std::mem::take(&mut token));
                    }
                }
                _ => token.push(ch),
            }
        }
    }

    if escaped {
        token.push('\\');
    }
    if !token.is_empty() {
        result.push(token);
    }

    Ok(result)
}

fn walk(
    node: &serde_json::Value,
    fns: &mut Vec<FfiFn>,
    header: &Path,
    current_file: &mut Option<String>,
) -> Result<(), String> {
    // Clang elides repeated file names. For macro-generated declarations,
    // expansionLoc is the declaration's effective source location; spellingLoc
    // can point into an unrelated macro-definition header.
    if let Some(file) = reported_declaration_file(node) {
        *current_file = Some(file.to_string());
    }

    if node.get("kind").and_then(|v| v.as_str()) == Some("FunctionDecl")
        && current_file
            .as_deref()
            .is_some_and(|file| same_file_path(file, header))
    {
        if let Some(f) = parse_fn(node) {
            merge_ffi_fn(fns, f, header)?;
        }
    }

    if let Some(inner) = node.get("inner").and_then(|v| v.as_array()) {
        for child in inner {
            walk(child, fns, header, current_file)?;
        }
    }

    Ok(())
}

fn merge_ffi_fn(fns: &mut Vec<FfiFn>, candidate: FfiFn, header: &Path) -> Result<(), String> {
    if let Some(existing) = fns.iter().find(|f| f.name == candidate.name) {
        let same_signature = existing.params == candidate.params && existing.ret == candidate.ret;

        if same_signature {
            return Ok(());
        }

        return Err(format!(
            "E0203: conflicting declarations for extern function `{}` in header `{}`",
            candidate.name,
            header.display()
        ));
    }

    fns.push(candidate);
    Ok(())
}

fn reported_declaration_file(node: &serde_json::Value) -> Option<&str> {
    node.pointer("/range/begin/expansionLoc/file")
        .or_else(|| node.pointer("/loc/expansionLoc/file"))
        .or_else(|| node.pointer("/loc/file"))
        .or_else(|| node.pointer("/range/begin/file"))
        .and_then(|value| value.as_str())
}

fn parse_fn(node: &serde_json::Value) -> Option<FfiFn> {
    // Variadic functions and declarations with unsupported ABI shapes are rejected.
    if node.get("variadic").and_then(|v| v.as_bool()) == Some(true) {
        return None;
    }

    let storage = node
        .get("storageClass")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if storage == "static" {
        return None;
    }

    let name = node.get("name")?.as_str()?.to_string();
    let qual = node.pointer("/type/qualType")?.as_str()?;

    let open = qual.find('(')?;
    let close = find_matching_paren(qual, open)?;
    if !qual[close + 1..].trim().is_empty() {
        return None;
    }

    let ret = parse_c_ty(qual[..open].trim())?;
    let params_src = qual[open + 1..close].trim();
    let mut params = Vec::new();

    if !params_src.is_empty() && params_src != "void" {
        for param in split_c_params(params_src)? {
            if param.trim() == "..." {
                return None;
            }
            params.push(parse_c_ty(param.trim())?);
        }
    }

    Some(FfiFn { name, params, ret })
}

fn same_file_path(reported: &str, requested: &Path) -> bool {
    match (fs::canonicalize(reported), fs::canonicalize(requested)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn find_matching_paren(source: &str, open: usize) -> Option<usize> {
    let mut depth = 0usize;

    for (offset, ch) in source[open..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(open + offset);
                }
            }
            _ => {}
        }
    }

    None
}

fn split_c_params(source: &str) -> Option<Vec<&str>> {
    let mut params = Vec::new();
    let mut depth = 0usize;
    let mut start = 0usize;

    for (offset, ch) in source.char_indices() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => {
                depth = depth.checked_sub(1)?;
            }
            ',' if depth == 0 => {
                params.push(source[start..offset].trim());
                start = offset + ch.len_utf8();
            }
            _ => {}
        }
    }

    if depth != 0 {
        return None;
    }

    params.push(source[start..].trim());
    Some(params)
}

fn parse_c_ty(source: &str) -> Option<Ty> {
    let normalized = source.split_whitespace().collect::<Vec<_>>().join(" ");

    // Top-level const on a by-value scalar does not change its ABI.
    let scalar = if !normalized.contains('*') {
        normalized.strip_prefix("const ").unwrap_or(&normalized)
    } else {
        &normalized
    };

    match scalar {
        "void" => Some(Ty::Void),
        "_Bool" | "bool" => Some(Ty::Bool),
        "int64_t" => Some(Ty::I64),
        "double" => Some(Ty::F64),

        // Only immutable C strings are compatible with Velox's string ABI.
        "const char *" | "const char*" | "char const *" | "char const*" => Some(Ty::Str),

        // Deliberately reject platform-sized, unsigned, narrow, and pointer types.
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

// Only unresolved system-header aliases request these libraries. A project
// header with the same basename must not change the linker's library set.
pub fn lib_for_header(header: &Path) -> Option<&'static str> {
    if header.exists() {
        return None;
    }

    header.to_str().and_then(|name| match name {
        "math.h" => Some("m"),
        "pthread.h" => Some("pthread"),
        "dlfcn.h" => Some("dl"),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_ffi_declarations_are_deduplicated() {
        let mut functions = Vec::new();
        let header = Path::new("api.h");

        merge_ffi_fn(
            &mut functions,
            FfiFn {
                name: "api".into(),
                params: vec![Ty::I64, Ty::Str],
                ret: Ty::I64,
            },
            header,
        )
        .unwrap();

        merge_ffi_fn(
            &mut functions,
            FfiFn {
                name: "api".into(),
                params: vec![Ty::I64, Ty::Str],
                ret: Ty::I64,
            },
            header,
        )
        .unwrap();

        assert_eq!(functions.len(), 1);
    }

    #[test]
    fn conflicting_ffi_declarations_are_rejected() {
        let mut functions = Vec::new();
        let header = Path::new("api.h");

        merge_ffi_fn(
            &mut functions,
            FfiFn {
                name: "api".into(),
                params: vec![Ty::I64],
                ret: Ty::I64,
            },
            header,
        )
        .unwrap();

        let error = merge_ffi_fn(
            &mut functions,
            FfiFn {
                name: "api".into(),
                params: vec![Ty::F64],
                ret: Ty::I64,
            },
            header,
        )
        .unwrap_err();

        assert!(error.contains("E0203"));
        assert!(error.contains("api"));
        assert!(error.contains("api.h"));
        assert_eq!(functions.len(), 1);
    }

    #[test]
    fn ffi_type_mapping_accepts_only_supported_abi_types() {
        assert!(matches!(parse_c_ty("int64_t"), Some(Ty::I64)));
        assert!(matches!(parse_c_ty("const int64_t"), Some(Ty::I64)));
        assert!(matches!(parse_c_ty("double"), Some(Ty::F64)));
        assert!(matches!(parse_c_ty("_Bool"), Some(Ty::Bool)));
        assert!(matches!(parse_c_ty("const char *"), Some(Ty::Str)));

        for unsupported in [
            "uint64_t",
            "unsigned long",
            "long",
            "long long",
            "size_t",
            "ssize_t",
            "int",
            "float",
            "char *",
            "void *",
        ] {
            assert!(
                parse_c_ty(unsupported).is_none(),
                "unexpectedly accepted C type: {unsupported}"
            );
        }
    }

    #[test]
    fn c_parameter_split_respects_nested_parentheses() {
        let params = split_c_params("int64_t, int (*)(int, int), const char *")
            .expect("valid parameter list");

        assert_eq!(params.len(), 3);
        assert_eq!(params[0], "int64_t");
        assert_eq!(params[1], "int (*)(int, int)");
        assert_eq!(params[2], "const char *");
    }

    #[test]
    fn ffi_parser_rejects_function_pointers_and_variadics() {
        let callback = serde_json::json!({
            "kind": "FunctionDecl",
            "name": "callback_api",
            "type": {
                "qualType": "void (int (*)(int, int))"
            }
        });

        assert!(parse_fn(&callback).is_none());

        let variadic = serde_json::json!({
            "kind": "FunctionDecl",
            "name": "variadic_api",
            "variadic": true,
            "type": {
                "qualType": "int (int, ...)"
            }
        });

        assert!(parse_fn(&variadic).is_none());
    }

    #[test]
    fn ffi_parser_accepts_supported_typedef_signatures() {
        let node = serde_json::json!({
            "kind": "FunctionDecl",
            "name": "probe",
            "type": {
                "qualType": "int64_t (int64_t, const char *)"
            }
        });

        let parsed = parse_fn(&node).expect("supported signature should parse");

        assert_eq!(parsed.name, "probe");
        assert!(matches!(parsed.ret, Ty::I64));
        assert_eq!(parsed.params.len(), 2);
        assert!(matches!(parsed.params[0], Ty::I64));
        assert!(matches!(parsed.params[1], Ty::Str));
    }

    #[test]
    fn macro_expansion_location_takes_precedence_for_declaration_origin() {
        let node = serde_json::json!({
            "kind": "FunctionDecl",
            "loc": {
                "includedFrom": { "file": "/sdk/usr/include/pthread.h" }
            },
            "range": {
                "begin": {
                    "spellingLoc": {
                        "file": "/sdk/usr/include/AvailabilityInternal.h"
                    },
                    "expansionLoc": {
                        "file": "/sdk/usr/include/pthread.h"
                    }
                }
            }
        });

        assert_eq!(
            reported_declaration_file(&node),
            Some("/sdk/usr/include/pthread.h")
        );
    }

    #[test]
    fn make_dependency_parser_handles_continuations_and_escaped_spaces() {
        let source = r#"velox_probe: /sdk/usr/include/pthread.h \
 /tmp/header\ with\ spaces.h
"#;
        let parsed = parse_make_dependencies(source).expect("valid dependency output");

        assert_eq!(
            parsed,
            vec!["/sdk/usr/include/pthread.h", "/tmp/header with spaces.h"]
        );
    }

    #[test]
    fn system_header_alias_resolves_to_a_real_file() {
        let resolved =
            resolve_system_header(Path::new("pthread.h")).expect("pthread.h should resolve");

        assert!(resolved.is_absolute());
        assert_eq!(
            resolved.file_name().and_then(|s| s.to_str()),
            Some("pthread.h")
        );
    }

    #[test]
    fn project_header_excludes_declarations_from_nested_headers() {
        let root = std::env::temp_dir().join(format!(
            "velox-ffi-nested-header-test-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();

        let header = root.join("public_api.h");
        let dependency = root.join("dependency.h");

        fs::write(
            &dependency,
            "#include <stdint.h>\nint64_t dependency_only(int64_t x);\n",
        )
        .unwrap();
        fs::write(
            &header,
            "#include <stdint.h>\n#include \"dependency.h\"\nint64_t public_api(int64_t x);\n",
        )
        .unwrap();

        let functions = extract_functions(&header).expect("header should parse");
        let names: Vec<_> = functions.iter().map(|f| f.name.as_str()).collect();

        assert!(names.contains(&"public_api"), "found: {names:?}");
        assert!(
            !names.contains(&"dependency_only"),
            "nested declaration leaked into API: {names:?}"
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn canonical_header_matching_rejects_same_basename_in_another_directory() {
        let root = std::env::temp_dir().join(format!("velox-ffi-path-test-{}", std::process::id()));

        let _ = fs::remove_dir_all(&root);
        let a = root.join("a");
        let b = root.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();

        let header_a = a.join("common.h");
        let header_b = b.join("common.h");
        fs::write(&header_a, "/* a */").unwrap();
        fs::write(&header_b, "/* b */").unwrap();

        assert!(same_file_path(header_a.to_str().unwrap(), &header_a));
        assert!(!same_file_path(header_b.to_str().unwrap(), &header_a));

        fs::remove_dir_all(root).unwrap();
    }
}
