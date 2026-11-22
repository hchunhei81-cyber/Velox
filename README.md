# Velox

> Simple like Python, safe like Rust, fast like C.

Velox is a general-purpose programming language targeting three goals at once:

1. **Readable** — minimal syntax, no ceremony.
2. **Safe** — `private` by default, no hidden conversions, ownership in Phase 2.
3. **Fast** — zero-cost abstractions, OpenMP for data parallelism, LLVM/MLIR in Phase 2/3.

**Status: bootstrap.** The compiler (`veloxc`) is currently written in Rust and emits C. The long-term plan is self-hosting (Velox compiles Velox) followed by an LLVM and then MLIR backend.

---

## Requirements

- `rustc` + `cargo` (tested with stable ≥ 1.70)
- `clang` (ships with Xcode Command Line Tools on macOS)
- *Optional:* `brew install libomp` — enables real parallelism for `parallel for`. Without it, loops run serially but stay correct.

---

## Build

```bash
git clone <repo>
cd velox
cargo build --release
```

The binary is at `./target/release/veloxc`.

---

## Quick start

**Hello world** (`example/hello.vx`):

```velox
fn fib(n: i64) -> i64 {
  let a: i64 = 0;
  let b: i64 = 1;
  let i: i64 = 0;
  while i < n {
    let t: i64 = a + b;
    a = b;
    b = t;
    i = i + 1;
  }
  return a;
}

fn main() -> void {
  print(fib(20));   // 6765
}
```

```bash
./target/release/veloxc example/hello.vx -o hello
./hello
# 6765
```

**Modules + visibility** (`example/parallel.vx`):

```velox
using "math.vx";

fn heavy(x: i64) -> i64 {
  let s: i64 = 0;
  let i: i64 = 0;
  while i < 200000 {
    s = s + (square(i) % 13);
    i = i + 1;
  }
  return s + x;
}

fn main() -> void {
  let n: i64 = 8;
  let sum: i64 = 0;
  parallel for i in 0..n {
    sum = sum + heavy(i);
  }
  print(sum);
}
```

```bash
./target/release/veloxc example/parallel.vx -o par
time ./par
```

Inspect the generated C:

```bash
cat par.c
# Look for:
#   #pragma omp parallel for
#   #pragma omp critical
```

---

## Language reference (current subset)

### Types

| Type   | Underlying |
|--------|------------|
| `i64`  | `int64_t`  |
| `str`  | `const char*` |
| `void` | `void`     |

No implicit conversions. `i64` never becomes `str` and vice versa.

### Declarations

```velox
let x: i64 = 42;        // explicit
let y = 42;             // inferred i64
let mut z: i64 = 0;     // `mut` reserved for Phase 2 (not enforced yet)
```

### Functions

```velox
public fn add(a: i64, b: i64) -> i64 {
  return a + b;
}

fn helper() -> void {
  // no visibility modifier => private
}
```

`private` (or omitted) means the function is only visible inside the file that defines it. Calling it from another file is `E0301`.

### Control flow

```velox
if x > 0 {
  print(x);
} else if x == 0 {
  print(0);
} else {
  print(-1);
}

while x < 10 {
  x = x + 1;
}
```

Conditions must be `i64` (no truthy values).

### Modules

```velox
using "path/to/file.vx";
```

- Path is **relative to the file containing the `using` statement**.
- Circular imports are rejected (`E0201`).
- Two functions with the same name across files is `E0102`.
- `using` is implemented as **AST merge**, not text include — but without namespaces yet. That comes in Phase 2.

### Parallelism

```velox
parallel for i in 0..N {
  // loop body
}
```

Lowers to OpenMP. Assignments to variables **outside** the loop body are automatically wrapped in `#pragma omp critical`.

- Nested `parallel for` is rejected (`E0402`).
- If `libomp` is missing, clang emits a warning and the loop runs serially. The `-Wno-unknown-pragmas` flag silences the noise.

### Built-in functions

| Name      | Signature        | Emits                          |
|-----------|------------------|--------------------------------|
| `print`   | `(i64) -> void`  | `printf("%lld\n", ...)`        |
| `println` | `(str) -> void`  | `printf("%s\n", ...)`          |

---

## Compiler CLI

```
veloxc <file.vx> [-o <out>]    Compile to <out> (default: input without .vx)
veloxc --explain <CODE>        Print the doc for an error code
veloxc --help                  Show usage
```

Exit codes: `0` success, `1` any error (parse, type, clang).

---

## Error codes

Full documentation lives in [`docs/errors/`](docs/errors/). Query any of them in-terminal:

```bash
./target/release/veloxc --explain E0308
```

| Code  | Meaning                              |
|-------|--------------------------------------|
| E0100 | Undefined variable                   |
| E0101 | Undefined function                   |
| E0102 | Function name collision across files |
| E0200 | `using` file not found               |
| E0201 | Circular `using`                     |
| E0301 | Call to a private function           |
| E0308 | Type mismatch                        |
| E0309 | Return type mismatch                 |
| E0400 | Missing return                       |
| E0401 | Condition / bound not `i64`          |
| E0402 | Nested `parallel for`                |

---

## Analysis API and source diagnostics

The library exposes `analyze_source(source, origin)` for editor and tooling
integration. It runs the lexer, parser, and type checker without generating C
or invoking Clang.

`AnalysisDiagnostic` provides:

- `code`: optional compiler error code.
- `message`: human-readable diagnostic.
- `severity`: `Error` or `Warning`.
- `start` / `end`: UTF-8 byte offsets into the original source.
- `line` / `column`: 1-based source position when available.

Columns count Unicode scalar values, not UTF-8 bytes or UTF-16 code units.
Language-server integrations should convert columns to the position encoding
negotiated with the editor (typically UTF-16).

Expression locations are preserved through parsing and type checking so
diagnostics can point to the innermost failing expression or operand.

## Repository layout

```
velox/
├── Cargo.toml
├── src/
│   ├── main.rs            CLI entry point
│   ├── driver.rs          Orchestration: collect files → parse → check → codegen → clang
│   ├── lexer.rs           Tokenizer
│   ├── ast.rs             AST types
│   ├── parser.rs          Recursive-descent parser
│   ├── types.rs           Type checker + visibility rules
│   └── codegen/
│       └── c.rs           C emitter (OpenMP for parallel for)
├── docs/
│   └── errors/            One Markdown file per error code (data source for --explain)
└── example/
    ├── hello.vx           Fibonacci
    ├── math.vx            public/private demo
    └── parallel.vx        using + parallel for
```

---

## Roadmap

### Phase 1 — Bootstrap (in progress)
- [x] Lexer, parser, type checker, C codegen
- [x] `using` (AST merge), `public` / `private`
- [x] `parallel for` via OpenMP
- [x] `--explain` for error codes
- [ ] `Span` on tokens — line:column in error messages
- [ ] Multi-error reporting (collect instead of early return)
- [ ] `struct`, `enum`, arrays, `f64`, `bool`
- [ ] REPL (`veloxc repl`)
- [ ] Self-hosting — write the compiler in Velox (`veloxc-self/`)

### Phase 2 — Performance & modules
- [ ] LLVM backend (replace C emission)
- [ ] Real module system with namespaces (`math::square`)
- [ ] Ownership + borrowing (Rust-style, but relaxed inside a thread)
- [ ] `comptime` and const generics
- [ ] SIMD primitives (`simd::add`, `simd::dot`)

### Phase 3 — GPU / AI
- [ ] MLIR backend
- [ ] `tile[T, ...]` first-class multidimensional arrays
- [ ] `parallel for` lowered to Metal (macOS) / CUDA (Linux+NVIDIA)
- [ ] Python FFI

---

## Design decisions worth knowing

**Why C as the first backend?**
Because it lets us iterate on language semantics in days, not months. LLVM/MLIR come later, once the frontend has stabilized. Most languages take the same path (Nim → C, early Rust → LLVM, Zig → LLVM, early Swift → ObjC).

**Why `private` by default (not Java's package-private)?**
There is no package concept yet. Once real modules land in Phase 2, we'll add a `package` (or `internal`) visibility tier between private and public.

**Why does `using` do AST merge instead of text include?**
Text include (C-style) makes cross-file diagnostics impossible — you can't tell which file a function came from. AST merge with per-function `origin` gives us `E0102` (name collision) and `E0301` (private access) for free.

**Why is assignment inside `parallel for` wrapped in `critical`?**
Because we don't yet have a way to prove a loop-carried dependence is a reduction. Locking every write is pessimistic but correct. Phase 2 will add `reduce (+)` clauses.

---

## Contributing

Before adding a new error code, read [`docs/errors/README.md`](docs/errors/README.md). Every code must have:
1. A Markdown file under `docs/errors/` with a reproducer.
2. A `--explain` entry.
3. A test case in `example/`.

Code style: `cargo fmt` + `cargo clippy -- -D warnings`.

---

## License

BSD 3-Clause