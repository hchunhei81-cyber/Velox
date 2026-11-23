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

echo "=== Compile generated C ==="
for name in single duplicate; do
    clang -std=c11 -O2 "$WORK/$name.c" -o "$WORK/$name"
    output=$("$WORK/$name")
    test "$output" = "42"
    echo "$name execution: $output PASS"
done

echo "=== STAGE 1 ALL TESTS PASSED ==="
