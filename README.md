# Velox

> Simple syntax. Explicit types. Native-speed execution.

Velox is an experimental general-purpose programming language focused on readable syntax, static type checking, and native execution. Its current compiler is written in Rust and translates Velox source into C, then invokes Clang to produce an executable.

**Project status: bootstrap compiler.** The frontend, type checker, C backend, parallel-loop support, Python embedding, diagnostics API, and an early VS Code language server are implemented. Self-hosting and LLVM/MLIR backends are future work, not current capabilities.

## Highlights

- **Compact syntax:** inferred local types, optional return annotations for `void` functions, `pub` visibility, and concise struct construction.
- **Static type checking:** `i64`, `f64`, `bool`, `str`, `void`, and user-defined structs; invalid operations and mismatched fields/types are rejected before C compilation.
- **Familiar control flow:** `if` / `else`, `while`, range-based `for`, and `parallel for`.
- **Native compilation:** Velox emits C and uses Clang; ordinary Velox programs do not need to embed or link Python.
- **Parallel execution:** `parallel for` can use OpenMP. Supported reductions are lowered to OpenMP reductions; builds without OpenMP support retain a serial fallback.
- **Python embedding:** explicitly import a Python module and call supported functions through the embedded CPython runtime.
- **Editor tooling:** an early LSP provides diagnostics, completion, semantic tokens, hover, and same-document go-to-definition.

## Requirements

Required:

- Rust toolchain (`rustc` and `cargo`)
- Clang (on macOS, install Xcode Command Line Tools)

Optional, depending on the features you use:

- OpenMP runtime, such as `libomp` (`brew install libomp` on macOS), for parallel execution
- CPython development headers and link configuration (`python3-config --embed` or equivalent) for `import`-based Python embedding

Without an available OpenMP runtime, supported parallel loops execute through the compiler's serial fallback. Python development files are needed only for programs that use Python embedding.

## Build

```bash
git clone https://github.com/novarift1/Velox.git
cd Velox
cargo build --release
```

The compiler is produced at `target/release/veloxc`.

For a development build:

```bash
cargo run --bin veloxc -- example/hello.vx -o hello
./hello
```

## Quick start

Create `hello.vx`:

```velox
fn fibonacci(n: i64) -> i64 {
    let a: i64 = 0;
    let b: i64 = 1;
    let i: i64 = 0;

    while i < n {
        let next = a + b;
        a = b;
        b = next;
        i = i + 1;
    }

    return a;
}

fn main() {
    println(fibonacci(20)); // 6765
}
```

Compile and run:

```bash
veloxc hello.vx -o hello
./hello
```

`main` has an implicit `void` return type. Functions that return a value must declare it explicitly, such as `-> i64`.

## Language tour

### Types and variables

The current built-in types are:

| Type | Meaning |
|---|---|
| `i64` | 64-bit signed integer |
| `f64` | 64-bit floating-point number |
| `bool` | Boolean value (`true` or `false`) |
| `str` | String |
| `void` | No return value |

Local variable types can be written explicitly or inferred from the initializer:

```velox
let count: i64 = 42;
let inferred_count = 42;
let ratio: f64 = 3.5;
let enabled: bool = true;
let message: str = "hello";
```

There are no implicit conversions between unrelated types. Conditions and logical operators require boolean expressions; integers are not treated as truthy values.

### Functions and visibility

Use `fn` for a function. A function without a return annotation returns `void`; a value-returning function declares its return type.

```velox
pub fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn internal_helper() {
    println("Only available within this source module");
}
```

- `pub` makes a function or struct public to code that imports its source file.
- Declarations are private by default when `pub` is omitted.
- `public` and `private` are not visibility keywords in the current syntax.

### Structs

Declare named fields and construct values with named arguments:

```velox
pub struct Point {
    x: i64,
    y: i64,
}

fn distance_squared(point: Point) -> i64 {
    return point.x * point.x + point.y * point.y;
}

fn main() {
    let origin = Point(x: 0, y: 0);
    println(distance_squared(origin));
}
```

The concise `Point(x: ..., y: ...)` form is recommended. The earlier brace form, `Point { x: ..., y: ... }`, remains accepted for compatibility. Struct construction uses named fields; positional struct arguments are not part of the current syntax. The checker validates missing, unknown, duplicate, and incorrectly typed fields.

Struct values can be passed to functions and returned by value. Nested struct fields and field access are supported.

### Control flow and ranges

```velox
fn sum_to(limit: i64) -> i64 {
    let total: i64 = 0;

    for i in 0..limit {
        total = total + i;
    }

    return total;
}

fn main() {
    let n = 5;

    if n > 0 {
        println(sum_to(n));
    } else {
        println(0);
    }
}
```

The current control-flow constructs are `if` / `else`, `while`, and range-based `for`. Range loops use the `start..end` form. Conditions must be `bool`.

### Source files and visibility

`using` brings another Velox source file into the compilation unit:

```velox
using "math.vx";

fn main() {
    println(square(12));
}
```

- Paths are resolved relative to the source file containing the `using` statement.
- Imported source files are parsed and merged as syntax trees; this is not textual inclusion.
- Circular source inclusion is rejected.
- Duplicate top-level function names across included files are diagnosed.
- A function or struct intended for use from another source file should be declared `pub`.

This is a lightweight source-file composition mechanism, not yet a namespaced package/module system.

### Parallel loops

Use `parallel for` for supported data-parallel loops:

```velox
fn main() {
    let count: i64 = 100000;
    let total: i64 = 0;

    parallel for i in 0..count {
        total = total + i;
    }

    println(total);
}
```

The compiler recognizes supported reduction patterns and lowers them to OpenMP when the toolchain provides OpenMP. Other shared writes are subject to conservative safety checks; the compiler rejects unsupported or potentially unsafe shared updates rather than silently assuming they are race-free.

If OpenMP is unavailable, the compiler has a serial fallback for supported parallel loops. Nested `parallel for` is currently rejected.

### Python embedding

Python integration is explicit and separate from ordinary Velox compilation:

```velox
import math;

fn main() {
    let result = py.math.sqrt(144.0);
    println(result);
}
```

`import math;` makes the Python module available through the `py` namespace. The compiler embeds CPython through its C API; it does not launch a Python subprocess for each call.

Notes:

- Python embedding requires a compatible CPython development installation and linker configuration.
- Only functions supported by the current Velox checker/runtime integration are available; this is not yet unrestricted access to every Python object or API.
- A program without Python imports does not need to link against CPython.
- The older `import python module;` spelling has been removed. Use `import module;`.

## Built-in output functions

The compiler provides output built-ins for the currently supported primitive values:

- `print(...)`
- `println(...)`
- `write(...)`

`write` writes without appending a newline. `println` is the newline-oriented output form. The compiler rejects unsupported argument types rather than applying implicit formatting conversions.

## Compiler and tooling

### Command line

```text
veloxc <file.vx> [-o <output>]   Compile a source file to a native executable
veloxc --explain <ERROR_CODE>    Show documentation for a compiler error
veloxc --help                    Show command-line help
```

When `-o` is omitted, the compiler chooses an output name based on the input file. Compilation invokes Clang after generating C.

### Generated C

The current backend emits C source as an intermediate artifact. This keeps the compiler implementation small while the language frontend evolves. It also makes it possible to inspect generated code and the OpenMP constructs used for parallel loops.

### Analysis API and diagnostics

The Rust library exposes `analyze_source(source, origin)` for editor and tooling integrations. It runs lexical analysis, parsing, and type checking without generating C or invoking Clang.

`AnalysisDiagnostic` includes:

- optional compiler error code
- human-readable message
- error or warning severity
- start and end UTF-8 byte offsets
- one-based line and column information when available

Source locations are preserved through parsing and type checking so diagnostics can identify the failing expression or operand. Editor integrations must convert offsets/columns to the position encoding they negotiate (commonly UTF-16).

### VS Code support

The extension is in `editors/vscode/velox-language`. Current language-server/editor features include:

- `.vx` language recognition and TextMate syntax highlighting
- semantic tokens
- compiler diagnostics with source ranges
- keyword and symbol completion
- struct-field completion after member access
- hover information for indexed symbols
- go-to-definition for symbols in the current document
- snippets, bracket pairing, indentation, comment toggling, and folding

The language server is an early implementation. Cross-file go-to-definition, full semantic indexing, formatting, and rename support are not currently implemented. See [`editors/vscode/velox-language/README.md`](editors/vscode/velox-language/README.md) for setup instructions.

## Error diagnostics

Compiler error documentation is maintained in [`docs/errors/`](docs/errors/). Use:

```bash
veloxc --explain E0308
```

The error documentation is intended to be the reference for individual diagnostic codes and examples.

## Repository layout

```text
Velox/
├── Cargo.toml
├── src/
│   ├── ast.rs
│   ├── lexer.rs
│   ├── parser.rs
│   ├── types.rs
│   ├── driver.rs
│   ├── main.rs
│   ├── bin/
│   │   └── velox-lsp.rs
│   └── codegen/
│       └── c.rs
├── editors/
│   └── vscode/
│       └── velox-language/
├── docs/
│   └── errors/
└── example/
```

## Current limitations

Velox is still in its bootstrap stage. In particular:

- the compiler currently uses a C backend and Clang; LLVM and MLIR backends are future work
- the language does not yet have Rust-style ownership and borrowing
- `using` is source-file AST composition, not a full namespaced module system
- Python embedding supports the currently integrated subset, not unrestricted Python interoperability
- the language server does not yet provide full cross-file navigation, formatting, or rename
- the language surface and diagnostics may evolve as the compiler stabilizes

## Roadmap

The roadmap is intentionally conservative; checked-off items describe implemented foundations, not production completeness.

### Frontend and bootstrap
- [x] Lexer, parser, AST, and static type checker
- [x] C code generation and Clang-driven native compilation
- [x] Structs, primitive types, expressions, and control flow
- [x] Source-file composition with `using`
- [x] `pub` visibility and private-by-default declarations
- [x] OpenMP-backed parallel loops with supported reductions and serial fallback
- [x] Compiler diagnostics and `--explain` documentation
- [x] Analysis API and early language-server integration
- [x] CPython embedding for supported imported-module calls
- [ ] Self-hosting compiler

### Future compiler and language work
- [ ] Namespaced module/package system
- [ ] Ownership and borrowing model
- [ ] LLVM backend
- [ ] MLIR backend
- [ ] Expanded standard library and broader Python interoperability
- [ ] Additional data structures and language constructs

## Design notes

**Why emit C first?** C provides a practical native-code path while the language frontend and semantics are still changing. It keeps the initial backend relatively small and allows generated code to be inspected. LLVM/MLIR can be evaluated after the language core stabilizes.

**Why private by default?** Cross-file visibility is explicit: declarations are local unless marked `pub`. This reduces accidental API exposure while the module system remains lightweight.

**Why is `using` AST composition?** Parsing source files into syntax trees preserves source-level structure and origin information. That enables diagnostics for duplicate names, circular inclusion, and visibility violations without treating source files as raw text.

**Why are parallel updates checked conservatively?** Correct parallel execution requires proving that shared updates are safe or recognized reductions. The compiler accepts supported reduction forms and rejects unsupported shared-write patterns rather than silently permitting data races.

## Contributing

Before adding a compiler error code, read [`docs/errors/README.md`](docs/errors/README.md). Keep the diagnostic documentation, `--explain` output, and regression tests in sync.

Run the test suite and formatting checks before submitting changes:

```bash
cargo fmt --all
cargo test
git diff --check
```

## License

BSD 3-Clause. See the repository's license file.
