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

fn assert_diagnostic_span(source: &str, expected_text: &str, expected_line: u32) {
    let diagnostic = analyze_source(source, "span.vx").unwrap_err();

    let start = diagnostic.start.expect("missing start offset");
    let end = diagnostic.end.expect("missing end offset");

    assert!(start <= end && end <= source.len());
    assert!(source.is_char_boundary(start));
    assert!(source.is_char_boundary(end));

    assert_eq!(
        &source[start..end],
        expected_text,
        "diagnostic should identify the innermost failing expression"
    );

    assert_eq!(diagnostic.line, Some(expected_line));

    let expected_column = source[..start].rsplit('\n').next().unwrap().chars().count() as u32 + 1;

    assert_eq!(
        diagnostic.column,
        Some(expected_column),
        "column should count Unicode characters from 1"
    );
}

#[test]
fn analysis_api_points_to_invalid_initializer() {
    let source = "fn main() -> void {\n    let x: i64 = true;\n}\n";
    assert_diagnostic_span(source, "true", 2);
}

#[test]
fn analysis_api_points_to_invalid_struct_field_value() {
    let source =
        "struct Point {\n    x: i64,\n}\nfn main() -> void {\n    let p = Point { x: true };\n}\n";
    assert_diagnostic_span(source, "true", 5);
}

#[test]
fn analysis_api_points_to_invalid_binary_operand() {
    let source = "fn main() -> void {\n    let x: i64 = 1 + true;\n}\n";
    assert_diagnostic_span(source, "true", 2);
}

#[test]
fn analysis_api_points_to_invalid_unary_operand() {
    let source = "fn main() -> void {\n    let x: i64 = -true;\n}\n";
    assert_diagnostic_span(source, "true", 2);
}

#[test]
fn analysis_api_points_to_undefined_function_name() {
    let source = "fn main() -> void {\n    missing();\n}\n";
    assert_diagnostic_span(source, "missing", 2);
}

#[test]
fn analysis_api_counts_unicode_characters_in_columns() {
    let source = "fn main() -> void {\n    let label: str = \"中文\"; let x: i64 = true;\n}\n";
    assert_diagnostic_span(source, "true", 2);
}

#[test]
fn analysis_api_points_to_unexpected_struct_token() {
    let source = "struct Point { x: i64 y: i64 }";
    let diagnostic = analyze_source(source, "struct.vx").unwrap_err();
    let start = diagnostic.start.expect("missing start offset");
    let end = diagnostic.end.expect("missing end offset");

    assert_eq!(&source[start..end], "y");
}

#[test]
fn analysis_api_points_to_eof_when_closing_brace_is_missing() {
    let source = "fn main() -> void { let x: i64 = 1;";
    let diagnostic = analyze_source(source, "eof.vx").unwrap_err();

    assert_eq!(diagnostic.start, Some(source.len()));
    assert_eq!(diagnostic.end, Some(source.len()));
}

#[test]
fn analysis_api_points_to_wrong_function_argument_count() {
    let source = "fn take(x: i64) -> void { print(x); }\nfn main() -> void {\n    take();\n}\n";
    assert_diagnostic_span(source, "take", 3);
}

#[test]
fn analysis_api_points_to_wrong_print_argument_count() {
    let source = "fn main() -> void {\n    print(1, 2);\n}\n";
    assert_diagnostic_span(source, "print", 2);
}

#[test]
fn analysis_api_points_to_unknown_struct_field_value() {
    let source = "struct Point { x: i64 }\nfn main() -> void {\n    let p = Point { z: 1 };\n}\n";
    assert_diagnostic_span(source, "1", 3);
}

#[test]
fn analysis_api_points_to_duplicate_struct_field_value() {
    let source =
        "struct Point { x: i64 }\nfn main() -> void {\n    let p = Point { x: 1, x: 2 };\n}\n";
    assert_diagnostic_span(source, "2", 3);
}

#[test]
fn analysis_api_points_to_struct_init_with_missing_field() {
    let source =
        "struct Point { x: i64, y: i64 }\nfn main() -> void {\n    let p = Point { x: 1 };\n}\n";
    assert_diagnostic_span(source, "Point", 3);
}

#[test]
fn analysis_api_points_to_immutable_field_assignment() {
    let source = "struct Point { x: i64 }\nfn main() -> void {\n    let p = Point { x: 1 };\n    p.x = 2;\n}\n";
    assert_diagnostic_span(source, "p.x", 4);
}

#[test]
fn analysis_api_points_to_undefined_field_root() {
    let source = "struct Point { x: i64 }\nfn main() -> void {\n    missing.x = 2;\n}\n";
    assert_diagnostic_span(source, "missing.x", 3);
}

#[test]
fn analysis_api_points_to_nonlocal_field_assignment_target() {
    let source = "struct Point { x: i64 }\nfn make_point() -> Point {\n    return Point { x: 1 };\n}\nfn main() -> void {\n    make_point().x = 2;\n}\n";
    assert_diagnostic_span(source, "make_point().x", 6);
}

#[test]
fn analysis_api_points_to_invalid_for_start_bound() {
    let source = "fn main() -> void {\n    for i in true..10 {\n        print(i);\n    }\n}\n";
    assert_diagnostic_span(source, "true", 2);
}

#[test]
fn analysis_api_points_to_invalid_for_end_bound() {
    let source = "fn main() -> void {\n    for i in 0..true {\n        print(i);\n    }\n}\n";
    assert_diagnostic_span(source, "true", 2);
}

#[test]
fn analysis_api_points_to_invalid_parallel_for_bound() {
    let source = "fn main() -> void {\n    parallel for i in true..10 {\n        let x: i64 = i;\n    }\n}\n";
    assert_diagnostic_span(source, "true", 2);
}

#[test]
fn analysis_api_points_to_return_inside_parallel_for() {
    let source =
        "fn main() -> void {\n    parallel for i in 0..10 {\n        return i;\n    }\n}\n";
    assert_diagnostic_span(source, "i", 3);
}
