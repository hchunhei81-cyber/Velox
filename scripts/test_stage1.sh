#!/bin/bash
set -euo pipefail

cd "$(dirname "$0")/.."

HARNESS="selfhost/src/__stage1_regression.vx"
STAGE1="/tmp/velox-stage1-regression"
WORK="/tmp/velox-stage1-regression-tests"

cleanup() {
    rm -f "$HARNESS"
}
trap cleanup EXIT

mkdir -p "$WORK"

cat > "$WORK/math.vx" <<'VX'
pub fn add(a: i64, b: i64) -> i64 {
    return a + b;
}
VX

cat > "$WORK/duplicate.vx" <<'VX'
using "math.vx";
using "math.vx";
fn main() {
    println(add(20, 22));
}
VX

cat > "$WORK/cycle-a.vx" <<'VX'
using "cycle-b.vx";
fn main() {}
VX

cat > "$WORK/cycle-b.vx" <<'VX'
using "cycle-a.vx";
VX

cat > "$WORK/single.vx" <<'VX'
fn main() {
    println(42);
}
VX

cat > "$WORK/field-mutable.vx" <<'VX'
struct Point { x: i64, y: i64 }
fn main() {
    let mut p = Point { x: 40, y: 2 };
    p.x = 40;
    p.y = p.x + 2;
    println(p.x + p.y);
}
VX

cat > "$WORK/field-immutable.vx" <<'VX'
struct Point { x: i64 }
fn main() {
    let p = Point { x: 1 };
    p.x = 2;
}
VX

cat > "$WORK/field-mismatch.vx" <<'VX'
struct Point { x: i64 }
fn main() {
    let mut p = Point { x: 1 };
    p.x = true;
}
VX

cat > "$WORK/nested-field.vx" <<'VX'
struct Inner { x: i64 }
struct Outer { inner: Inner }
fn main() {
    let mut p = Outer { inner: Inner { x: 1 } };
    p.inner.x = 40;
    println(p.inner.x + 2);
}
VX

cat > "$WORK/nested-immutable.vx" <<'VX'
struct Inner { x: i64 }
struct Outer { inner: Inner }
fn main() {
    let p = Outer { inner: Inner { x: 1 } };
    p.inner.x = 2;
}
VX

cat > "$WORK/return-field.vx" <<'VX'
struct Point { x: i64 }
fn make_point() -> Point {
    return Point { x: 1 };
}
fn main() {
    make_point().x = 2;
}
VX

cat > "$WORK/parallel-stage1.vx" <<'VX'
fn main() {
    parallel for i in 0..3 {
    }
    println(42);
}
VX

cat > "$WORK/parallel-outer-write.vx" <<'VX'
fn main() {
    let mut total: i64 = 0;
    parallel for i in 0..3 {
        total = total + 1;
    }
}
VX

cat > "$WORK/parallel-local-write.vx" <<'VX'
fn main() {
    parallel for i in 0..3 {
        let mut local: i64 = 0;
        local = local + 1;
    }
    println(42);
}
VX

cat > "$WORK/parallel-reduction.vx" <<'VX'
fn main() {
    let mut sum: i64 = 0;
    let mut count: i64 = 0;
    parallel for i in 0..1000 {
        sum = sum + i;
        count = i + count;
    }
    println(sum);
    println(count);
}
VX

cat > "$WORK/parallel-f64-reduction.vx" <<'VX'
fn main() {
    let mut sum: f64 = 0.0;
    parallel for i in 0..10 {
        sum = sum + 0.5;
    }
    println(sum == 5.0);
}
VX

cat > "$WORK/f64-print.vx" <<'VX'
fn main() {
    println(0.1 + 0.2);
    let pi: f64 = 3.141592653589793;
    println(pi);
}
VX

cat > "$WORK/scientific-notation.vx" <<'VX'
fn main() {
    println(1e3 == 1000.0);
    println(2.5E+2 == 250.0);
    println(1.25e-2 > 0.0124);
    println(1e-3 + 2.5e-4 > 0.0012);
}
VX

cat > "$WORK/not-equal.vx" <<'VX'
fn main() {
    println(1 == 1);
    println(1 != 2);
    println(1 < 2);
    println(2 > 1);
    println(2 <= 2);
    println(2 >= 2);
    println(1.5 < 2.5);
    println(2.5 >= 2.5);
    println(true == true);
    println(true != false);
    println((1 + 2) < 4);
    println((1 < 2) && (3 >= 3));
}
VX

cat > "$HARNESS" <<VX
using "lexer.vx";
using "ast.vx";
using "parser.vx";
using "types.vx";
using "codegen.vx";
using "driver.vx";

fn main() {
    if compile_file("$WORK/single.vx", "$WORK/single") {
        println("SINGLE PASS");
    } else {
        println("SINGLE FAIL");
    }

    if compile_file("$WORK/duplicate.vx", "$WORK/duplicate") {
        println("DUPLICATE PASS");
    } else {
        println("DUPLICATE FAIL");
    }

    if compile_file("$WORK/cycle-a.vx", "$WORK/cycle") {
        println("CYCLE FAIL");
    } else {
        println("CYCLE PASS");
    }

    if compile_file("$WORK/field-mutable.vx", "$WORK/field-mutable") {
        println("FIELD MUTABLE PASS");
    } else {
        println("FIELD MUTABLE FAIL");
    }

    if compile_file("$WORK/field-immutable.vx", "$WORK/field-immutable") {
        println("FIELD IMMUTABLE FAIL");
    } else {
        println("FIELD IMMUTABLE PASS");
    }

    if compile_file("$WORK/field-mismatch.vx", "$WORK/field-mismatch") {
        println("FIELD MISMATCH FAIL");
    } else {
        println("FIELD MISMATCH PASS");
    }

    if compile_file("$WORK/nested-field.vx", "$WORK/nested-field") {
        println("NESTED FIELD PASS");
    } else {
        println("NESTED FIELD FAIL");
    }

    if compile_file("$WORK/nested-immutable.vx", "$WORK/nested-immutable") {
        println("NESTED IMMUTABLE FAIL");
    } else {
        println("NESTED IMMUTABLE PASS");
    }

    if compile_file("$WORK/parallel-stage1.vx", "$WORK/parallel-stage1") {
        println("PARALLEL STAGE1 COMPILE PASS");
    } else {
        println("PARALLEL STAGE1 COMPILE FAIL");
    }

    if compile_file("$WORK/parallel-outer-write.vx", "$WORK/parallel-outer-write") {
        println("PARALLEL OUTER WRITE FAIL");
    } else {
        println("PARALLEL OUTER WRITE REJECT");
    }

    if compile_file("$WORK/parallel-local-write.vx", "$WORK/parallel-local-write") {
        println("PARALLEL LOCAL WRITE PASS");
    } else {
        println("PARALLEL LOCAL WRITE FAIL");
    }

    if compile_file("$WORK/parallel-reduction.vx", "$WORK/parallel-reduction") {
        println("PARALLEL REDUCTION COMPILE PASS");
    } else {
        println("PARALLEL REDUCTION COMPILE FAIL");
    }

    if compile_file("$WORK/parallel-f64-reduction.vx", "$WORK/parallel-f64-reduction") {
        println("PARALLEL F64 REDUCTION COMPILE PASS");
    } else {
        println("PARALLEL F64 REDUCTION COMPILE FAIL");
    }

    if compile_file("$WORK/f64-print.vx", "$WORK/f64-print") {
        println("F64 PRINT COMPILE PASS");
    } else {
        println("F64 PRINT COMPILE FAIL");
    }

    if compile_file("$WORK/scientific-notation.vx", "$WORK/scientific-notation") {
        println("SCIENTIFIC NOTATION COMPILE PASS");
    } else {
        println("SCIENTIFIC NOTATION COMPILE FAIL");
    }

    if compile_file("$WORK/not-equal.vx", "$WORK/not-equal") {
        println("NOT EQUAL COMPILE PASS");
    } else {
        println("NOT EQUAL COMPILE FAIL");
    }

    if compile_file("$WORK/return-field.vx", "$WORK/return-field") {
        println("RETURN FIELD FAIL");
    } else {
        println("RETURN FIELD PASS");
    }

}
VX

echo "=== Build Stage 1 ==="
cargo run --release --bin veloxc -- "$HARNESS" -o "$STAGE1"

echo "=== Run Stage 1 regression ==="
"$STAGE1" | tee "$WORK/result.log"

grep -q "SINGLE PASS" "$WORK/result.log"
grep -q "DUPLICATE PASS" "$WORK/result.log"
grep -q "E0203: circular dependency" "$WORK/result.log"
grep -q "CYCLE PASS" "$WORK/result.log"
grep -q "FIELD MUTABLE PASS" "$WORK/result.log"
grep -q "FIELD IMMUTABLE PASS" "$WORK/result.log"
grep -q "FIELD MISMATCH PASS" "$WORK/result.log"
grep -q "E0500: cannot modify a field of immutable variable 'p'" "$WORK/result.log"
grep -q "E0500: field assignment target must be rooted in a local variable" "$WORK/result.log"
grep -q "E0308: field assignment type mismatch: expected i64, found bool" "$WORK/result.log"
grep -q "NESTED FIELD PASS" "$WORK/result.log"
grep -q "NESTED IMMUTABLE PASS" "$WORK/result.log"
grep -q "RETURN FIELD PASS" "$WORK/result.log"
grep -q "PARALLEL STAGE1 COMPILE PASS" "$WORK/result.log"
grep -q "PARALLEL OUTER WRITE REJECT" "$WORK/result.log"
grep -q "PARALLEL LOCAL WRITE PASS" "$WORK/result.log"

grep -q "PARALLEL REDUCTION COMPILE PASS" "$WORK/result.log"
grep -qE '#pragma omp parallel for reduction\(\+: .*,' "$WORK/parallel-reduction.c"

clang -std=c11 -O2 -Xpreprocessor -fopenmp \
    -I"$(brew --prefix libomp)/include" \
    "$WORK/parallel-reduction.c" \
    -L"$(brew --prefix libomp)/lib" -lomp \
    -o "$WORK/parallel-reduction"
output=$("$WORK/parallel-reduction")
expected=$(printf '499500\n499500')
if [ "$output" != "$expected" ]; then
    echo "PARALLEL REDUCTION OUTPUT MISMATCH"
    printf 'Expected:\n%s\nActual:\n%s\n' "$expected" "$output"
    exit 1
fi
echo "MULTI REDUCTION EXECUTION PASS"

grep -q "PARALLEL F64 REDUCTION COMPILE PASS" "$WORK/result.log"
grep -qE '#pragma omp parallel for reduction\(\+: .*\)' "$WORK/parallel-f64-reduction.c"

clang -std=c11 -O2 -Xpreprocessor -fopenmp \
    -I"$(brew --prefix libomp)/include" \
    "$WORK/parallel-f64-reduction.c" \
    -L"$(brew --prefix libomp)/lib" -lomp \
    -o "$WORK/parallel-f64-reduction"

output=$("$WORK/parallel-f64-reduction")
if [ "$output" != "true" ]; then
    echo "F64 REDUCTION OUTPUT MISMATCH"
    printf 'Expected: true\nActual: %s\n' "$output"
    exit 1
fi
echo "F64 REDUCTION EXECUTION PASS"

grep -q "F64 PRINT COMPILE PASS" "$WORK/result.log"
clang -std=c11 -O2 "$WORK/f64-print.c" -o "$WORK/f64-print"
output=$("$WORK/f64-print")
expected=$(printf '0.30000000000000004\n3.1415926535897931')
if [ "$output" != "$expected" ]; then
    echo "F64 PRINT OUTPUT MISMATCH"
    printf 'Expected:\\n%s\\nActual:\\n%s\\n' "$expected" "$output"
    exit 1
fi
echo "F64 PRINT EXECUTION PASS"

grep -q "SCIENTIFIC NOTATION COMPILE PASS" "$WORK/result.log"
clang -std=c11 -O2 "$WORK/scientific-notation.c" -o "$WORK/scientific-notation"
output=$("$WORK/scientific-notation")
expected=$(printf 'true\ntrue\ntrue\ntrue')
if [ "$output" != "$expected" ]; then
    echo "SCIENTIFIC NOTATION OUTPUT MISMATCH"
    printf 'Expected:\n%s\nActual:\n%s\n' "$expected" "$output"
    exit 1
fi
echo "SCIENTIFIC NOTATION EXECUTION PASS"

grep -q "NOT EQUAL COMPILE PASS" "$WORK/result.log"
clang -std=c11 -O2 "$WORK/not-equal.c" -o "$WORK/not-equal"
output=$("$WORK/not-equal")
expected=$(printf 'true\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue\ntrue')
if [ "$output" != "$expected" ]; then
    echo "NOT EQUAL OUTPUT MISMATCH"
    printf 'Expected:\n%s\nActual:\n%s\n' "$expected" "$output"
    exit 1
fi
echo "NOT EQUAL EXECUTION PASS"

grep -q "#pragma omp parallel for" "$WORK/parallel-stage1.c"
clang -std=c11 -O2 "$WORK/parallel-stage1.c" -o "$WORK/parallel-stage1"
output=$("$WORK/parallel-stage1")
test "$output" = "42"
echo "PARALLEL FOR CODEGEN PASS"

echo "=== Compile generated C ==="
for name in single duplicate; do
    clang -std=c11 -O2 "$WORK/$name.c" -o "$WORK/$name"
    output=$("$WORK/$name")
    test "$output" = "42"
    echo "$name execution: $output PASS"
done


clang -std=c11 -O2 "$WORK/field-mutable.c" -o "$WORK/field-mutable"
output=$("$WORK/field-mutable")
test "$output" = "82"
echo "struct field assignment execution: $output PASS"

clang -std=c11 -O2 "$WORK/nested-field.c" -o "$WORK/nested-field"
output=$("$WORK/nested-field")
test "$output" = "42"
echo "nested struct field execution: $output PASS"

echo "=== Code generation regression ==="
cargo run --release --bin veloxc -- selfhost/src/codegen_check.vx -o "$WORK/codegen-check"
"$WORK/codegen-check" | tee "$WORK/codegen.log"
grep -q "CODEGEN PASSED" "$WORK/codegen.log"

echo "=== Literal type inference regression ==="
grep -q "int64_t vxv_696e6665727265645f69" /tmp/velox-stage1-generated.c
grep -q "double vxv_696e6665727265645f66" /tmp/velox-stage1-generated.c
grep -q "velox_str vxv_696e6665727265645f73" /tmp/velox-stage1-generated.c
grep -q "bool vxv_696e6665727265645f62" /tmp/velox-stage1-generated.c
echo "I64 F64 STR BOOL INFERENCE PASS"
grep -q "int64_t vxv_696e6665727265645f636f7079" /tmp/velox-stage1-generated.c
grep -q "int64_t vxv_696e6665727265645f73756d" /tmp/velox-stage1-generated.c
grep -q "bool vxv_696e6665727265645f636d70" /tmp/velox-stage1-generated.c
echo "VARIABLE ARITHMETIC COMPARISON INFERENCE PASS"
grep -q "int64_t vxv_696e6665727265645f63616c6c" /tmp/velox-stage1-generated.c
echo "FUNCTION RETURN TYPE INFERENCE PASS"

clang -std=c11 -O2 /tmp/velox-stage1-generated.c -o "$WORK/codegen-check-run"
output=$("$WORK/codegen-check-run")
expected=$(printf 'Velox\ntrue\nfalse\n6\n42')
if [ "$output" != "$expected" ]; then
    echo "CODEGEN OUTPUT MISMATCH"
    printf 'Expected:\n%s\nActual:\n%s\n' "$expected" "$output"
    exit 1
fi
echo "STRING BOOL LOOP FUNCTION STRUCT PASS"

echo "=== Three-stage VX self-bootstrap ==="
mkdir -p /tmp/velox-bootstrap

cargo run --release --bin veloxc -- selfhost/src/__bootstrap_test.vx -o "$WORK/bootstrap-stage1"
"$WORK/bootstrap-stage1" | tee "$WORK/bootstrap-stage1.log"
grep -q "BOOTSTRAP PASS" "$WORK/bootstrap-stage1.log"

clang -w /tmp/velox-bootstrap/stage2.c -o "$WORK/bootstrap-stage2"
"$WORK/bootstrap-stage2" | tee "$WORK/bootstrap-stage2.log"
grep -q "COMPILATION PASSED" "$WORK/bootstrap-stage2.log"

echo "=== Stage 2 / Stage 3 deterministic comparison ==="
cmp /tmp/velox-bootstrap/stage2.c /tmp/velox-bootstrap/stage3.c
echo "STAGE 2 / STAGE 3 IDENTICAL"

clang -w /tmp/velox-bootstrap/stage3.c -o "$WORK/bootstrap-stage3"
cp /tmp/velox-bootstrap/stage3.c "$WORK/stage3-before.c"
"$WORK/bootstrap-stage3" | tee "$WORK/bootstrap-stage3.log"
grep -q "COMPILATION PASSED" "$WORK/bootstrap-stage3.log"

cmp "$WORK/stage3-before.c" /tmp/velox-bootstrap/stage3.c
echo "STAGE 3 SELF-COMPILATION PASS"

echo "=== STAGE 1 ALL TESTS PASSED ==="
