mod ast;
mod driver;
mod ffi;
mod lexer;
mod parser;
mod python;
mod types;
mod codegen {
    pub mod c;
}

use std::env;
use std::fs;
use std::process;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 && (args[1] == "--help" || args[1] == "-h") {
        println!("Velox compiler {}", env!("CARGO_PKG_VERSION"));
        println!("\nUsage:");
        println!("  veloxc <file.vx> [-o <output>]");
        println!("  veloxc <file.vx> --py [-o <output-dir>]");
        println!("  veloxc --explain <ERROR_CODE>");
        println!("  veloxc --version");
        return;
    }
    if args.len() >= 2 && args[1] == "--version" {
        println!("veloxc {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    if args.len() >= 3 && args[1] == "--explain" {
        explain(&args[2]);
        return;
    }

    if args.len() < 2 {
        eprintln!("usage:");
        eprintln!("  veloxc <file.vx> [-o <out>]");
        eprintln!("  veloxc <file.vx> --py [-o <outdir>]");
        eprintln!("  veloxc --explain <CODE>");
        process::exit(1);
    }

    let src_path = &args[1];

    // --py 模式
    if args.iter().any(|a| a == "--py") {
        let out_dir = find_after(&args, "-o").unwrap_or_else(|| ".".to_string());
        match driver::compile_py(src_path, &out_dir) {
            Ok(_) => {}
            Err(e) => {
                eprintln!("error: {}", e);
                process::exit(1);
            }
        }
        return;
    }

    let out = find_after(&args, "-o").unwrap_or_else(|| {
        if src_path.ends_with(".vx") {
            src_path.trim_end_matches(".vx").to_string()
        } else {
            "a.out".to_string()
        }
    });

    match driver::compile(src_path, &out) {
        Ok(_) => println!("built: {}", out),
        Err(e) => {
            eprintln!("error: {}", e);
            process::exit(1);
        }
    }
}

fn find_after(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn explain(code: &str) {
    let path = format!("docs/errors/{}.md", code);
    match fs::read_to_string(&path) {
        Ok(s) => print!("{}", s),
        Err(_) => {
            eprintln!("unknown error code `{}`", code);
            if let Ok(entries) = fs::read_dir("docs/errors") {
                let mut names: Vec<String> = entries
                    .flatten()
                    .filter_map(|e| {
                        let n = e.file_name().to_string_lossy().to_string();
                        n.strip_suffix(".md").map(|s| s.to_string())
                    })
                    .filter(|s| s != "README")
                    .collect();
                names.sort();
                if !names.is_empty() {
                    eprintln!("\navailable codes:");
                    for n in names {
                        eprintln!("  {}", n);
                    }
                }
            }
            process::exit(1);
        }
    }
}
