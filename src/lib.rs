pub mod ast;
pub mod codegen {
    pub mod c;
}
pub mod driver;
pub mod ffi;
pub mod lexer;
pub mod parser;
pub mod python;
pub mod types;

use lexer::Span;

/// Severity of a compiler diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

/// A compiler diagnostic suitable for IDE and command-line consumers.
///
/// `line` and `column` are 1-based when available.
/// They remain `None` until lexer/parser spans are implemented.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalysisDiagnostic {
    pub code: Option<String>,
    pub message: String,
    pub severity: DiagnosticSeverity,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// UTF-8 byte offsets in the original source.
    pub start: Option<usize>,
    pub end: Option<usize>,
}

impl AnalysisDiagnostic {
    fn from_message(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = message
            .split_once(':')
            .map(|(prefix, _)| prefix.trim())
            .filter(|prefix| {
                prefix.len() >= 2
                    && prefix.starts_with('E')
                    && prefix[1..].chars().all(|c| c.is_ascii_digit())
            })
            .map(str::to_string);

        Self {
            code,
            message,
            severity: DiagnosticSeverity::Error,
            line: None,
            column: None,
            start: None,
            end: None,
        }
    }

    fn at_span(message: impl Into<String>, span: Span) -> Self {
        let mut diagnostic = Self::from_message(message);
        diagnostic.start = Some(span.start);
        diagnostic.end = Some(span.end);
        diagnostic
    }
}

/// Analyze one Velox source file without generating C or invoking Clang.
///
/// This API runs the same lexer, parser and type checker used by `veloxc`.
/// `using` directives are ignored in this initial single-file API; workspace
/// import resolution will be added with the language-server implementation.
pub fn analyze_source(source: &str, origin: &str) -> Result<(), AnalysisDiagnostic> {
    // Mask using-directive bytes without changing source length or line endings.
    // This keeps every token span aligned with the original document.
    let mut masked = source.as_bytes().to_vec();
    let mut offset = 0usize;
    for line in source.split_inclusive('\n') {
        let content = line.trim_start();
        if content.starts_with("using \"") {
            for byte in &mut masked[offset..offset + line.len()] {
                if *byte != b'\n' && *byte != b'\r' {
                    *byte = b' ';
                }
            }
        }
        offset += line.len();
    }
    let masked = String::from_utf8(masked).expect("space masking preserves valid UTF-8");

    let tokens = lexer::Lexer::new(&masked)
        .tokenize_spanned()
        .map_err(|error| AnalysisDiagnostic::at_span(error.message, error.span))?;

    let mut program = parser::Parser::new_spanned(tokens)
        .program_with_span()
        .map_err(|(message, span)| AnalysisDiagnostic::at_span(message, span))?;

    for function in &mut program.fns {
        function.origin = origin.to_string();
        function.is_extern = false;
    }

    for structure in &mut program.structs {
        structure.origin = origin.to_string();
    }

    let mut checker = types::Checker::new();
    checker
        .check(&program)
        .map_err(AnalysisDiagnostic::from_message)?;

    Ok(())
}
