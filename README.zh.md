# Velox

> 简单如 Python，安全如 Rust，快如 C。

Velox 是一门通用编程语言，同时瞄准三个目标：

1. **好读** —— 语法极简，不摆谱。
2. **安全** —— 默认 `private`，无隐式转换，Phase 2 引入所有权。
3. **快** —— 零成本抽象，`parallel for` 走 OpenMP，Phase 2/3 换 LLVM/MLIR。

**当前状态：Bootstrap。** 编译器（`veloxc`）目前用 Rust 写，输出 C 代码。长期路线是自举（Velox 编译 Velox），然后换 LLVM、再换 MLIR 后端。

---

## 依赖

- `rustc` + `cargo`（已在 stable ≥ 1.70 验证）
- `clang`（macOS 上 Xcode Command Line Tools 自带）
- **可选**：`brew install libomp` —— 让 `parallel for` 真并行。不装也能跑，串行执行结果正确。

---

## 构建

```bash
git clone <repo>
cd velox
cargo build --release
```

产物在 `./target/release/veloxc`。

---

## 快速上手

**Hello world**（`example/hello.vx`）：

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

**模块 + 可见性**（`example/parallel.vx`）：

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

看生成的 C：

```bash
cat par.c
# 会看到：
#   #pragma omp parallel for
#   #pragma omp critical
```

---

## 语言参考（当前子集）

### 类型

| 类型   | 底层          |
|--------|---------------|
| `i64`  | `int64_t`     |
| `str`  | `const char*` |
| `void` | `void`        |

**无隐式转换**。`i64` 永远不会自动变成 `str`，反过来也一样。

### 声明

```velox
let x: i64 = 42;        // 显式
let y = 42;             // 推断为 i64
let mut z: i64 = 0;     // `mut` 为 Phase 2 预留（目前不检查）
```

### 函数

```velox
public fn add(a: i64, b: i64) -> i64 {
  return a + b;
}

fn helper() -> void {
  // 不写修饰符 => private
}
```

`private`（或不写）意味着函数只在**定义它的文件**内可见。跨文件调用会报 `E0301`。

### 控制流

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

条件必须是 `i64`——没有 truthy 值。

### 模块

```velox
using "path/to/file.vx";
```

- 路径**相对于当前文件所在目录**。
- 循环引用报 `E0201`。
- 两个文件定义同名函数报 `E0102`。
- `using` 目前实现为 **AST 合并**，不是文本内联——但还没有命名空间，Phase 2 补上。

### 并行

```velox
parallel for i in 0..N {
  // 循环体
}
```

底层展开成 OpenMP。对**循环外变量**的赋值会自动包一层 `#pragma omp critical`。

- 不支持嵌套 `parallel for`（报 `E0402`）。
- 没有 `libomp` 时 clang 会给出警告，循环串行执行。`-Wno-unknown-pragmas` 会压掉这条噪音。

### 内置函数

| 名字      | 签名             | 展开                             |
|-----------|------------------|----------------------------------|
| `print`   | `(i64) -> void`  | `printf("%lld\n", ...)`          |
| `println` | `(str) -> void`  | `printf("%s\n", ...)`            |

---

## 编译器 CLI

```
veloxc <file.vx> [-o <out>]    编译到 <out>（默认：输入去掉 .vx）
veloxc --explain <CODE>        打印某个错误码的文档
veloxc --help                  查看用法
```

退出码：`0` 成功，`1` 任何错误（语法、类型、clang 失败）。

---

## 错误码

完整文档在 [`docs/errors/`](docs/errors/)。终端直接查：

```bash
./target/release/veloxc --explain E0308
```

| 码    | 含义                      |
|-------|---------------------------|
| E0100 | 未定义变量                |
| E0101 | 未定义函数                |
| E0102 | 函数跨文件重名            |
| E0200 | `using` 找不到文件        |
| E0201 | `using` 循环引用          |
| E0301 | 调用私有函数              |
| E0308 | 类型不匹配                |
| E0309 | 返回值类型不符            |
| E0400 | 缺少 return               |
| E0401 | 条件 / 循环边界不是 i64   |
| E0402 | 嵌套 `parallel for`       |

---

## 目录结构

```
velox/
├── Cargo.toml
├── src/
│   ├── main.rs            CLI 入口
│   ├── driver.rs          编排：收集文件 → 解析 → 检查 → 生成 C → 调 clang
│   ├── lexer.rs           词法分析
│   ├── ast.rs             AST 类型定义
│   ├── parser.rs          递归下降解析器
│   ├── types.rs           类型检查 + 可见性规则
│   └── codegen/
│       └── c.rs           C 代码生成（parallel for 走 OpenMP）
├── docs/
│   └── errors/            每个错误码一个 Markdown（--explain 的数据源）
└── example/
    ├── hello.vx           斐波那契
    ├── math.vx            public/private 示例
    └── parallel.vx        using + parallel for 示例
```

---

## 路线图

### Phase 1 —— Bootstrap（进行中）
- [x] 词法、语法、类型检查、C 代码生成
- [x] `using`（AST 合并）、`public` / `private`
- [x] `parallel for` 走 OpenMP
- [x] `--explain` 查询错误码
- [ ] Token 带 `Span` —— 错误信息里有行:列
- [ ] 多错误收集（不再遇错就 `return`）
- [ ] `struct`、`enum`、数组、`f64`、`bool`
- [ ] REPL（`veloxc repl`）
- [ ] 自举 —— 用 Velox 写编译器（`veloxc-self/`）

### Phase 2 —— 性能与模块
- [ ] LLVM 后端（替代 C 输出）
- [ ] 真模块系统，带命名空间（`math::square`）
- [ ] 所有权 + 借用（Rust 风格，但单线程内放宽）
- [ ] `comptime` 与 const 泛型
- [ ] SIMD 原语（`simd::add`、`simd::dot`）

### Phase 3 —— GPU / AI
- [ ] MLIR 后端
- [ ] `tile[T, ...]` 一等多维数组
- [ ] `parallel for` 下沉到 Metal（macOS）/ CUDA（Linux + NVIDIA）
- [ ] Python FFI

---

## 值得知道的设计决策

**为什么先走 C 后端？**
因为这样可以在几天内迭代语言语义，而不是几个月。LLVM / MLIR 等前端稳定后再换。多数语言都走过这条路（Nim → C、早期 Rust → LLVM、Zig → LLVM、早期 Swift → ObjC）。

**为什么默认 `private`（而非 Java 的 package-private）？**
目前还没有"包"的概念。Phase 2 引入模块系统后，会在 private 和 public 之间加一层 `package`（或 `internal`）。

**为什么 `using` 做 AST 合并而非文本内联？**
C 式文本内联让跨文件诊断变得不可能——你没法知道某个函数来自哪个文件。带 per-function `origin` 的 AST 合并免费换来 `E0102`（重名）和 `E0301`（私有访问）两条诊断。

**为什么 `parallel for` 内的赋值都加 `critical`？**
因为我们还没有能力证明循环携带依赖是归约。每次写都加锁很保守，但正确。Phase 2 会加 `reduce (+)` 子句。

---

## 贡献

加新错误码前请先读 [`docs/errors/README.md`](docs/errors/README.md)。每个错误码必须有：

1. `docs/errors/` 下的一份 Markdown，带可复现的例子。
2. `--explain` 条目。
3. `example/` 下的一个测试用例。

代码风格：`cargo fmt` + `cargo clippy -- -D warnings`。

---

## License

MIT OR Apache-2.0