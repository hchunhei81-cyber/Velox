use veloxc::{analyze_source, DiagnosticSeverity};

#[test]
fn analysis_api_accepts_valid_program() {
    let source = r#"
        fn main() -> void {
            let x: i64 = 42;
            print(x);
        }
    "#;

    assert!(analyze_source(source, "valid.vx").is_ok());
}

#[test]
fn analysis_api_returns_structured_type_error() {
    let source = r#"
        fn main() -> void {
            let x: i64 = true;
            print(x);
        }
    "#;

    let diagnostic = analyze_source(source, "invalid.vx").unwrap_err();

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(diagnostic.code.is_some());
    assert!(!diagnostic.message.is_empty());
}

#[test]
fn analysis_api_returns_syntax_error() {
    let source = r#"
        fn main() -> void {
            let x: i64 = ;
        }
    "#;

    let diagnostic = analyze_source(source, "syntax.vx").unwrap_err();

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert!(!diagnostic.message.is_empty());
    assert!(diagnostic.start.is_some(), "missing start offset");
    assert!(diagnostic.end.is_some(), "missing end offset");

    let start = diagnostic.start.unwrap();
    let end = diagnostic.end.unwrap();
    assert!(start <= end && end <= source.len());
    assert!(source.is_char_boundary(start));
    assert!(source.is_char_boundary(end));
    assert_eq!(
        &source[start..end],
        ";",
        "span should identify the unexpected semicolon"
    );
}

#[test]
fn analysis_api_accepts_structs_and_boolean_logic() {
    let source = r#"
        struct Point {
            x: i64,
            y: i64,
        }

        fn main() -> void {
            let p = Point { x: 3, y: 4 };
            let ready: bool = p.x < p.y && true;
            print(ready);
        }
    "#;

    assert!(analyze_source(source, "structs.vx").is_ok());
}
