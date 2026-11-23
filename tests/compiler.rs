use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

struct Case {
    root: PathBuf,
    source: PathBuf,
    executable: PathBuf,
}

impl Case {
    fn new(source_text: &str) -> Self {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root =
            std::env::temp_dir().join(format!("velox-integration-{}-{}", std::process::id(), id));

        fs::create_dir_all(&root).unwrap();

        let source = root.join("main.vx");
        let executable = root.join("program");

        fs::write(&source, source_text).unwrap();

        Self {
            root,
            source,
            executable,
        }
    }

    fn compile(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_veloxc"))
            .arg(&self.source)
            .arg("-o")
            .arg(&self.executable)
            .output()
            .expect("failed to launch veloxc")
    }

    fn compile_without_openmp(&self) -> Output {
        Command::new(env!("CARGO_BIN_EXE_veloxc"))
            .arg(&self.source)
            .arg("-o")
            .arg(&self.executable)
            .env("VELOX_DISABLE_OPENMP", "1")
            .output()
            .expect("failed to launch veloxc without OpenMP")
    }

    fn run(&self) -> Output {
        Command::new(&self.executable)
            .output()
            .expect("failed to launch compiled program")
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn parallel_plus_reduction_executes_correctly() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut sum: i64 = 0;
    parallel for i in 0..1000 {
        sum = sum + i;
    }
    print(sum);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "499500");
}

#[test]
fn multiple_parallel_reductions_work() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut a: i64 = 0;
    let mut b: i64 = 0;

    parallel for i in 0..100 {
        a = a + i;
        b = b + (i * 2);
    }

    print(a + b);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "14850");
}

#[test]
fn reduction_inside_if_branches_works() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut sum: i64 = 0;

    parallel for i in 0..100 {
        if i < 50 {
            sum = sum + i;
        } else {
            sum = sum + i;
        }
    }

    print(sum);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "4950");
}

#[test]
fn read_only_shared_capture_is_allowed() {
    let case = Case::new(
        r#"
fn main() -> void {
    let base: i64 = 7;

    parallel for i in 0..10 {
        let x: i64 = base + i;
    }
}
"#,
    );

    let build = case.compile();
    assert!(
        build.status.success(),
        "read-only capture rejected: {}",
        stderr(&build)
    );

    let run = case.run();
    assert!(run.status.success());
}

#[test]
fn unsafe_shared_assignment_is_rejected_and_old_binary_removed() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut x: i64 = 0;

    parallel for i in 0..10 {
        x = i;
    }
}
"#,
    );

    // Simulate a stale binary left by a previous successful build.
    fs::write(&case.executable, "stale executable").unwrap();

    let build = case.compile();

    assert!(!build.status.success());
    assert!(
        stderr(&build).contains("E0601"),
        "expected E0601, got: {}",
        stderr(&build)
    );
    assert!(
        !case.executable.exists(),
        "stale executable survived failed compilation"
    );
}

#[test]
fn extra_reduction_variable_read_is_rejected() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut sum: i64 = 0;

    parallel for i in 0..10 {
        sum = sum + i;
        print(sum);
    }
}
"#,
    );

    let build = case.compile();

    assert!(
        !build.status.success(),
        "unsafe reduction read unexpectedly compiled"
    );
    assert!(
        stderr(&build).contains("E0601"),
        "expected E0601, got: {}",
        stderr(&build)
    );
}

#[test]
fn non_add_shared_update_is_rejected() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut product: i64 = 1;

    parallel for i in 0..10 {
        product = product * i;
    }
}
"#,
    );

    let build = case.compile();
    assert!(!build.status.success());
    assert!(
        stderr(&build).contains("E0601"),
        "expected E0601, got: {}",
        stderr(&build)
    );
}

#[test]
fn reduction_variable_read_in_condition_is_rejected() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut sum: i64 = 0;

    parallel for i in 0..10 {
        sum = sum + i;
        if sum > 0 {
            let x: i64 = i;
        }
    }
}
"#,
    );

    let build = case.compile();
    assert!(!build.status.success());
    assert!(
        stderr(&build).contains("E0601"),
        "expected E0601, got: {}",
        stderr(&build)
    );
}

#[test]
fn f64_plus_reduction_executes_correctly() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut sum: f64 = 0.0;

    parallel for i in 0..10 {
        sum = sum + 0.5;
    }

    print(sum);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "5");
}

#[test]
fn reduction_inside_nested_while_executes_correctly() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut sum: i64 = 0;

    parallel for i in 0..10 {
        let mut j: i64 = 0;
        while j < 2 {
            sum = sum + i;
            j = j + 1;
        }
    }

    print(sum);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "90");
}

#[test]
fn no_openmp_fallback_executes_reduction_serially() {
    let case = Case::new(
        r#"
fn main() -> void {
    let mut sum: i64 = 0;

    parallel for i in 0..1000 {
        sum = sum + i;
    }

    print(sum);
}
"#,
    );

    let build = case.compile_without_openmp();
    assert!(
        build.status.success(),
        "serial fallback compile failed: {}",
        stderr(&build)
    );

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "499500");
}

#[test]
fn bool_literals_and_print_work() {
    let case = Case::new(
        r#"
fn main() -> void {
    let yes: bool = true;
    let no: bool = false;
    print(yes);
    print(no);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "true\nfalse");
}

#[test]
fn comparisons_produce_bool_and_control_flow_works() {
    let case = Case::new(
        r#"
fn positive(x: i64) -> bool {
    return x > 0;
}

fn main() -> void {
    let ok: bool = positive(5);
    let mut i: i64 = 0;

    while i < 3 {
        if ok {
            print(i);
        }
        i = i + 1;
    }
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "0\n1\n2");
}

#[test]
fn bool_function_parameter_and_return_work() {
    let case = Case::new(
        r#"
fn invert(x: bool) -> bool {
    return x == false;
}

fn main() -> void {
    print(invert(false));
    print(invert(true));
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "true\nfalse");
}

#[test]
fn integer_condition_is_rejected() {
    let case = Case::new(
        r#"
fn main() -> void {
    if 1 {
        print(1);
    }
}
"#,
    );

    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0401"));
}

#[test]
fn bool_arithmetic_is_rejected() {
    let case = Case::new(
        r#"
fn main() -> void {
    let x: bool = true + false;
}
"#,
    );

    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"));
}

#[test]
fn bool_return_type_mismatch_is_rejected() {
    let case = Case::new(
        r#"
fn broken() -> bool {
    return 1;
}

fn main() -> void {}
"#,
    );

    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0309"));
}

#[test]
fn bool_logical_operators_short_circuit() {
    let case = Case::new(
        r#"
fn mark() -> bool {
    print(99);
    return true;
}
fn main() -> void {
    let a: bool = false && mark();
    let b: bool = true || mark();
    let c: bool = !false;
    let d: bool = true && !false || false;
    print(a);
    print(b);
    print(c);
    print(d);
}
"#,
    );
    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "false\ntrue\ntrue\ntrue");
}

#[test]
fn bool_logical_precedence_is_correct() {
    let case = Case::new(
        r#"
fn main() -> void {
    let a: bool = true || false && false;
    let b: bool = (true || false) && false;
    print(a);
    print(b);
}
"#,
    );
    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "true\nfalse");
}

#[test]
fn logical_operators_reject_non_bool_operands() {
    let case = Case::new(
        r#"
fn main() -> void {
    let x: bool = 1 && true;
}
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"));
}

#[test]
fn logical_not_rejects_non_bool_operand() {
    let case = Case::new(
        r#"
fn main() -> void {
    let x: bool = !1;
}
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"));
}

#[test]
fn struct_literal_field_access_and_arithmetic_work() {
    let case = Case::new(
        r#"
struct Point { x: f64, y: f64 }
fn norm2(p: Point) -> f64 { return p.x * p.x + p.y * p.y; }
fn main() -> void {
    let p = Point { x: 3.0, y: 4.0 };
    print(norm2(p));
}
"#,
    );
    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "25");
}

#[test]
fn struct_can_be_returned_by_value() {
    let case = Case::new(
        r#"
struct Pair { a: i64, b: i64 }
fn make_pair(x: i64) -> Pair { return Pair { a: x, b: x + 1 }; }
fn main() -> void { let p = make_pair(40); print(p.a + p.b); }
"#,
    );
    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "81");
}

#[test]
fn struct_missing_field_is_rejected() {
    let case = Case::new(
        r#"
struct Point { x: i64, y: i64 }
fn main() -> void { let p = Point { x: 1 }; print(p.x); }
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"), "got: {}", stderr(&build));
}

#[test]
fn struct_unknown_field_is_rejected() {
    let case = Case::new(
        r#"
struct Point { x: i64 }
fn main() -> void { let p = Point { x: 1, z: 2 }; print(p.x); }
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"), "got: {}", stderr(&build));
}

#[test]
fn duplicate_struct_fields_are_rejected() {
    let case = Case::new(
        r#"
struct Point { x: i64, x: i64 }
fn main() -> void { let p = Point { x: 1 }; print(p.x); }
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0106"), "got: {}", stderr(&build));
}

#[test]
fn nested_struct_fields_work() {
    let case = Case::new(
        r#"
struct Point { x: f64, y: f64 }
struct Rectangle { top_left: Point, bottom_right: Point }

fn main() -> void {
    let p = Point { x: 3.0, y: 4.0 };
    let r = Rectangle {
        top_left: p,
        bottom_right: Point { x: 20.0, y: 30.0 }
    };
    print(r.top_left.x + r.bottom_right.y);
}
"#,
    );
    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "33");
}

#[test]
fn mutable_struct_field_assignment_works() {
    let case = Case::new(
        r#"
struct Point { x: i64, y: i64 }
fn main() -> void {
    let mut p = Point { x: 1, y: 2 };
    p.x = 40;
    p.y = p.x + 2;
    print(p.x + p.y);
}
"#,
    );
    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run).trim(), "82");
}

#[test]
fn immutable_struct_field_assignment_is_rejected() {
    let case = Case::new(
        r#"
struct Point { x: i64 }
fn main() -> void {
    let p = Point { x: 1 };
    p.x = 2;
    print(p.x);
}
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0500"), "got: {}", stderr(&build));
}

#[test]
fn struct_field_assignment_type_mismatch_is_rejected() {
    let case = Case::new(
        r#"
struct Point { x: i64 }
fn main() -> void {
    let mut p = Point { x: 1 };
    p.x = true;
}
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"), "got: {}", stderr(&build));
}

#[test]
fn recursive_struct_value_type_is_rejected() {
    let case = Case::new(
        r#"
struct A { b: B }
struct B { a: A }
fn main() -> void {}
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"), "got: {}", stderr(&build));
}

#[test]
fn print_and_println_support_all_primitive_types() {
    let case = Case::new(
        r#"
fn main() -> void {
    print("hello");
    println(42);
    println(2.5);
    println(true);
    print(false);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run), "hello\n42\n2.5\ntrue\nfalse\n");
}

#[test]
fn write_outputs_without_newline_for_all_primitive_types() {
    let case = Case::new(
        r#"
fn main() -> void {
    write("hello");
    write(42);
    write(2.5);
    write(true);
    write(false);
    println("!");
}
"#,
    );
    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run), "hello422.5truefalse!\n");
}

#[test]
fn unary_minus_supports_integer_float_variables_and_parentheses() {
    let case = Case::new(
        r#"
fn main() -> void {
    let x = -42;
    let y = -3.5;
    let z = -(x + 8);
    println(x);
    println(y);
    println(z);
    println(-(-5));
}
"#,
    );
    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run), "-42\n-3.5\n34\n5\n");
}

#[test]
fn unary_minus_rejects_non_numeric_operands() {
    let case = Case::new(
        r#"
fn main() -> void {
    let invalid = -true;
}
"#,
    );
    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"), "got: {}", stderr(&build));
}

#[test]
fn lexer_rejects_raw_nul_byte_in_source() {
    let case = Case::new("fn main() -> void {}\0");
    let build = case.compile();
    assert!(!build.status.success());
    assert!(
        stderr(&build).contains("NUL byte"),
        "got: {}",
        stderr(&build)
    );
}

#[test]
fn print_and_println_reject_unsupported_types() {
    let case = Case::new(
        r#"
struct Point { x: i64 }
fn main() -> void {
    let p = Point { x: 1 };
    println(p);
}
"#,
    );

    let build = case.compile();
    assert!(!build.status.success());
    assert!(stderr(&build).contains("E0308"));
}

#[test]
fn ordinary_loop_bound_expression_is_evaluated_once() {
    let case = Case::new(
        r#"
fn get_limit() -> i64 {
    println("bound");
    return 3;
}

fn main() -> void {
    for i in 0..get_limit() {
        println(i);
    }
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run), "bound\n0\n1\n2\n");
}

#[test]
fn parallel_loop_bound_expression_is_evaluated_once() {
    let case = Case::new(
        r#"
fn get_limit() -> i64 {
    println("bound");
    return 3;
}

fn main() -> void {
    parallel for i in 0..get_limit() {
        let x: i64 = i;
    }
}
"#,
    );

    let build = case.compile_without_openmp();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run), "bound\n");
}

#[test]
fn c_backend_mangles_c_keywords_and_library_identifiers() {
    let case = Case::new(
        r#"
struct switch { default: i64 }
fn printf() -> i64 { return 41; }
fn main() -> void {
    let int = 1;
    let value = switch(default: printf());
    println(value.default + int);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));
    let run = case.run();
    assert!(run.status.success());
    assert_eq!(stdout(&run), "42\n");
}

fn assert_i64_runtime_error(source: &str, expected: &str) {
    let case = Case::new(source);

    let build = case.compile();
    assert!(
        build.status.success(),
        "compile failed unexpectedly: {}",
        stderr(&build)
    );

    let run = case.run();
    assert!(
        !run.status.success(),
        "program unexpectedly succeeded; stdout: {}",
        stdout(&run)
    );

    assert!(
        stderr(&run).contains(expected),
        "expected runtime error {:?}, got stderr: {}",
        expected,
        stderr(&run)
    );
}

#[test]
fn i64_addition_overflow_is_reported_at_runtime() {
    assert_i64_runtime_error(
        r#"
fn main() {
    let max: i64 = 9223372036854775807;
    let one: i64 = 1;
    let result: i64 = max + one;
    print(result);
}
"#,
        "i64 addition overflow",
    );
}

#[test]
fn i64_subtraction_overflow_is_reported_at_runtime() {
    assert_i64_runtime_error(
        r#"
fn main() {
    let min: i64 = -9223372036854775807 - 1;
    let one: i64 = 1;
    let result: i64 = min - one;
    print(result);
}
"#,
        "i64 subtraction overflow",
    );
}

#[test]
fn i64_multiplication_overflow_is_reported_at_runtime() {
    assert_i64_runtime_error(
        r#"
fn main() {
    let max: i64 = 9223372036854775807;
    let two: i64 = 2;
    let result: i64 = max * two;
    print(result);
}
"#,
        "i64 multiplication overflow",
    );
}

#[test]
fn i64_division_by_zero_is_reported_at_runtime() {
    assert_i64_runtime_error(
        r#"
fn main() {
    let one: i64 = 1;
    let zero: i64 = 0;
    let result: i64 = one / zero;
    print(result);
}
"#,
        "i64 division by zero",
    );
}

#[test]
fn i64_min_divided_by_negative_one_is_reported_at_runtime() {
    assert_i64_runtime_error(
        r#"
fn main() {
    let min: i64 = -9223372036854775807 - 1;
    let negative_one: i64 = -1;
    let result: i64 = min / negative_one;
    print(result);
}
"#,
        "i64 division overflow",
    );
}

#[test]
fn i64_negation_overflow_is_reported_at_runtime() {
    assert_i64_runtime_error(
        r#"
fn main() {
    let min: i64 = -9223372036854775807 - 1;
    let result: i64 = -min;
    print(result);
}
"#,
        "i64 negation overflow",
    );
}

#[test]
fn i64_modulo_by_zero_is_reported_at_runtime() {
    assert_i64_runtime_error(
        r#"
fn main() {
    let one: i64 = 1;
    let zero: i64 = 0;
    let result: i64 = one % zero;
    print(result);
}
"#,
        "i64 modulo by zero",
    );
}

#[test]
fn i64_min_modulo_negative_one_returns_zero() {
    let case = Case::new(
        r#"
fn main() {
    let min: i64 = -9223372036854775807 - 1;
    let negative_one: i64 = -1;
    print(min % negative_one);
}
"#,
    );

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success(), "runtime failed: {}", stderr(&run));
    assert_eq!(stdout(&run).trim(), "0");
}

#[test]
fn bootstrap_runtime_strings_files_and_integer_lists_work() {
    let case = Case::new("");
    let input = case.root.join("runtime-input.txt");
    let output = case.root.join("runtime-output.txt");
    fs::write(&input, "hello").unwrap();

    let input_literal = input.to_str().unwrap();
    let output_literal = output.to_str().unwrap();
    let source = format!(
        r#"
fn main() {{
    let content: str = read_file("{}");
    println(str_len(content));
    println(str_byte_at(content, 1));
    let part: str = str_slice(content, 1, 4);
    let joined: str = str_concat(part, "!");
    println(joined);
    println(str_eq(joined, "ell!"));
    let values: i64 = list_i64_new();
    list_i64_push(values, 7);
    list_i64_push(values, 9);
    println(list_i64_len(values));
    println(list_i64_get(values, 1));
    let written: bool = write_file("{}", joined);
    println(written);
}}
"#,
        input_literal, output_literal
    );
    fs::write(&case.source, source).unwrap();

    let build = case.compile();
    assert!(build.status.success(), "compile failed: {}", stderr(&build));

    let run = case.run();
    assert!(run.status.success(), "run failed: {}", stderr(&run));
    assert_eq!(stdout(&run), "5\n101\nell!\ntrue\n2\n9\ntrue\n");
    assert_eq!(fs::read_to_string(output).unwrap(), "ell!");
}
