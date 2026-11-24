# Velox

> 简洁的语法，明确的类型，原生执行性能。

Velox 是一门实验性的通用编程语言，目标是兼顾可读性、静态类型检查和原生执行。当前编译器使用 Rust 编写，将 Velox 源码转换为 C，再调用 Clang 生成可执行程序。

**项目状态：Bootstrap 编译器阶段。** 当前已实现语言前端、类型检查器、C 后端、并行循环、Python 嵌入、诊断 API，以及早期版本的 VS Code 语言服务器。自举编译器和 LLVM/MLIR 后端属于未来计划，并非当前已具备的能力。

## 核心特性

- **精简语法：** 局部变量类型推断、`void` 函数可省略返回类型、使用 `pub` 声明公开可见性、简洁的结构体初始化。
- **静态类型检查：** 支持 `i64`、`f64`、`bool`、`str`、`void` 和用户定义结构体；类型不匹配、非法运算和结构体字段错误会在生成 C 之前被检查。
- **常见控制流：** `if` / `else`、`while`、范围 `for` 和 `parallel for`。
- **原生编译：** Velox 生成 C 并调用 Clang；普通 Velox 程序不需要嵌入或链接 Python。
- **并行执行：** `parallel for` 可使用 OpenMP；支持的归约模式会转换为 OpenMP reduction。缺少 OpenMP 时，受支持的循环可使用串行回退路径。
- **Python 嵌入：** 显式导入 Python 模块，通过嵌入式 CPython 运行时调用当前已支持的函数。
- **编辑器工具：** 早期 LSP 提供诊断、补全、语义高亮、悬停信息和当前文档内的跳转定义。

## 环境要求

必需：

- Rust 工具链（`rustc` 和 `cargo`）
- Clang（macOS 可通过 Xcode Command Line Tools 安装）

按需安装：

- OpenMP 运行时，例如 macOS 上的 `libomp`（`brew install libomp`），用于并行执行
- CPython 开发头文件和链接配置（例如 `python3-config --embed`），用于 Python 嵌入

如果系统没有可用的 OpenMP 运行时，受支持的并行循环会通过编译器的串行回退路径执行。只有使用 Python 嵌入的程序才需要 CPython 开发环境。

## 构建

```bash
git clone https://github.com/novarift1/Velox.git
cd Velox
cargo build --release
```

编译器位于 `target/release/veloxc`。

开发期间也可以直接运行：

```bash
cargo run --bin veloxc -- example/hello.vx -o hello
./hello
```

## 快速开始

创建 `hello.vx`：

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

编译并运行：

```bash
veloxc hello.vx -o hello
./hello
```

`main` 省略返回类型时默认返回 `void`。返回具体值的函数必须显式声明返回类型，例如 `-> i64`。

## 语言概览

### 类型与变量

当前内置类型：

| 类型 | 含义 |
|---|---|
| `i64` | 64 位有符号整数 |
| `f64` | 64 位浮点数 |
| `bool` | 布尔值（`true` 或 `false`） |
| `str` | 字符串 |
| `void` | 无返回值 |

局部变量可以显式标注类型，也可以根据初始化表达式推断：

```velox
let count: i64 = 42;
let inferred_count = 42;
let ratio: f64 = 3.5;
let enabled: bool = true;
let message: str = "hello";
```

不同类型之间不会自动进行隐式转换。条件表达式和逻辑运算要求布尔值；整数不会被当作 truthy/falsy 值处理。

### 函数与可见性

使用 `fn` 声明函数。省略返回类型表示返回 `void`；返回值函数必须显式声明返回类型。

```velox
pub fn add(a: i64, b: i64) -> i64 {
    return a + b;
}

fn internal_helper() {
    println("Only available within this source module");
}
```

- `pub` 将函数或结构体声明为公开，可供导入该源文件的代码访问。
- 不写 `pub` 时，声明默认是私有的。
- 当前语法不再把 `public` 和 `private` 作为可见性关键字。

### 结构体

结构体使用命名字段，推荐使用带命名参数的函数式初始化：

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

推荐使用简洁形式 `Point(x: ..., y: ...)`。为兼容旧代码，花括号形式 `Point { x: ..., y: ... }` 仍然可用。结构体初始化使用命名字段；当前不支持位置参数。类型检查器会检查缺失字段、未知字段、重复字段和字段值类型错误。

结构体值可以作为函数参数，也可以按值返回；支持嵌套结构体字段和字段访问。

### 控制流与范围循环

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

当前控制流包括 `if` / `else`、`while` 和范围 `for`。范围循环使用 `start..end` 形式。条件必须是 `bool`。

### 源文件组合与可见性

使用 `using` 将另一个 Velox 源文件纳入当前编译单元：

```velox
using "math.vx";

fn main() {
    println(square(12));
}
```

- 路径相对于包含该 `using` 语句的源文件解析。
- 被引入的源文件会被解析为语法树并合并，而不是直接进行文本包含。
- 循环引入会被拒绝。
- 不同文件中重复的顶层函数名会触发诊断。
- 如果函数或结构体需要被其他源文件访问，应使用 `pub` 声明。

这是轻量级的源文件组合机制，还不是带命名空间的完整模块/包系统。

### 并行循环

使用 `parallel for` 表达受支持的数据并行循环：

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

编译器会识别受支持的归约模式，并在工具链提供 OpenMP 时将其转换为 OpenMP。其他共享写入会接受保守的安全检查；对于不支持或可能不安全的共享更新，编译器会报错，而不是默认认为它们没有数据竞争。

如果系统没有 OpenMP，受支持的并行循环会使用串行回退路径。目前不支持嵌套 `parallel for`。

### Python 嵌入

Python 集成是显式启用的，并与普通 Velox 编译相互独立：

```velox
import math;

fn main() {
    let result = py.math.sqrt(144.0);
    println(result);
}
```

`import math;` 将 Python 模块引入 `py` 命名空间。编译器通过 CPython C API 嵌入 Python 运行时；不会为每次调用启动一个 Python 子进程。

注意事项：

- Python 嵌入需要兼容的 CPython 开发环境和链接配置。
- 当前仅支持 Velox 类型检查器和运行时集成所覆盖的函数；这还不是对所有 Python 对象和 API 的无限制访问。
- 不包含 Python 导入的程序不需要链接 CPython。
- 旧写法 `import python module;` 已移除，请使用 `import module;`。

## 内置输出函数

编译器为当前支持的基本类型提供输出内置函数：

- `print(...)`
- `println(...)`
- `write(...)`

`write` 不会自动追加换行符；`println` 用于带换行的输出。对于不支持的参数类型，编译器会报错，而不会尝试隐式格式化转换。

## 编译器与开发工具

### 命令行

```text
veloxc <file.vx> [-o <output>]   将源文件编译为原生可执行程序
veloxc --explain <ERROR_CODE>    显示指定编译错误码的文档
veloxc --help                    显示命令行帮助
```

省略 `-o` 时，编译器会根据输入文件名选择输出名称。编译过程中会先生成 C，再调用 Clang。

### 生成的 C 代码

当前后端会生成 C 源码作为中间产物。这种方式让编译器后端保持精简，便于在语言前端持续演进时快速迭代。开发者也可以检查生成的 C 代码以及并行循环对应的 OpenMP 指令。

### 分析 API 与诊断信息

Rust 库提供 `analyze_source(source, origin)`，供编辑器和其他工具集成。它会运行词法分析、语法解析和类型检查，但不会生成 C，也不会调用 Clang。

`AnalysisDiagnostic` 包含：

- 可选的编译错误码
- 人类可读的错误信息
- 错误或警告级别
- 源码中的 UTF-8 字节起止偏移
- 可用时提供从 1 开始的行号和列号

源码位置信息会在解析和类型检查阶段保留，使诊断能够定位到具体表达式或操作数。编辑器集成需要将偏移/列号转换为其协商使用的位置编码（通常是 UTF-16）。

### VS Code 支持

扩展位于 `editors/vscode/velox-language`。当前语言服务器和编辑器功能包括：

- `.vx` 文件识别和 TextMate 语法高亮
- 语义 token 高亮
- 带源码范围的编译诊断
- 关键字和符号补全
- 成员访问后的结构体字段补全
- 已索引符号的悬停信息
- 当前文档内的跳转到定义
- 代码片段、括号配对、缩进、注释切换和代码折叠

语言服务器仍处于早期阶段。跨文件跳转定义、完整语义索引、格式化和重命名目前尚未实现。安装与调试方式请参阅 [`editors/vscode/velox-language/README.md`](editors/vscode/velox-language/README.md)。

## 错误诊断

编译错误文档维护在 [`docs/errors/`](docs/errors/) 中。可以通过以下命令查看：

```bash
veloxc --explain E0308
```

每个错误码对应的文档用于说明错误原因并提供示例。

## 仓库结构

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

## 当前限制

Velox 仍处于 Bootstrap 阶段，主要限制包括：

- 当前使用 C 后端和 Clang；LLVM、MLIR 后端属于未来工作。
- 尚未实现 Rust 风格的所有权和借用系统。
- `using` 是源文件语法树组合机制，还不是完整的命名空间模块系统。
- Python 嵌入只覆盖当前已集成的功能，并非无限制的 Python 互操作。
- 语言服务器尚不支持完整的跨文件导航、格式化和重命名。
- 随着编译器逐步稳定，语法和诊断接口仍可能调整。

## 路线图

路线图保持谨慎；已勾选项目代表基础能力已实现，不代表已经达到生产级成熟度。

### 前端与 Bootstrap
- [x] 词法分析器、语法解析器、AST 和静态类型检查器
- [x] C 代码生成和基于 Clang 的原生编译
- [x] 结构体、基本类型、表达式和控制流
- [x] 使用 `using` 组合源文件
- [x] `pub` 可见性和默认私有声明
- [x] 基于 OpenMP 的并行循环、受支持的归约和串行回退
- [x] 编译诊断和 `--explain` 错误文档
- [x] 分析 API 和早期语言服务器集成
- [x] 对受支持模块调用的 CPython 嵌入
- [ ] 自举编译器

### 后续编译器与语言工作
- [ ] 带命名空间的模块/包系统
- [ ] 所有权与借用模型
- [ ] LLVM 后端
- [ ] MLIR 后端
- [ ] 扩展标准库和更广泛的 Python 互操作
- [ ] 更多数据结构和语言构造

## 设计说明

**为什么先生成 C？** C 提供了一条务实的原生代码路径，适合语言前端和语义仍在快速变化的阶段。它能让初期后端保持精简，也方便检查生成代码。待语言核心稳定后，再评估 LLVM/MLIR。

**为什么默认私有？** 在模块系统仍较轻量的阶段，跨文件 API 需要显式声明：没有 `pub` 的声明只在本源文件范围内可见。这能减少意外暴露的接口。

**为什么 `using` 采用 AST 合并？** 将源文件解析为语法树，可以保留结构和来源信息。这样编译器就能诊断重复名称、循环引入和可见性错误，而不是把源文件当作纯文本处理。

**为什么保守检查并行更新？** 正确的并行执行要求证明共享更新安全，或识别为受支持的归约模式。编译器接受已支持的归约形式，并拒绝不支持的共享写入模式，避免静默引入数据竞争。

## 参与开发

新增编译错误码前，请先阅读 [`docs/errors/README.md`](docs/errors/README.md)。诊断文档、`--explain` 输出和回归测试应保持同步。

提交修改前运行测试和格式检查：

```bash
cargo fmt --all
cargo test
git diff --check
```

## 许可证

BSD 3-Clause，详见仓库中的许可证文件。
