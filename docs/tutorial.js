(() => {
  "use strict";

  const translations = {
    en: {
      contents:"CONTENTS", navIntro:"Overview", navQuick:"Quick start", navFormat:"File format",
      navSyntax:"Basic syntax", navTypes:"Type system", navControl:"Control flow", navFunctions:"Functions",
      navStructs:"Structs", navModules:"Modules & dependencies", navKeywords:"Keywords", navOperators:"Operators",
      navCompiler:"Compiler workflow", navBootstrap:"Self-hosting", navErrors:"Troubleshooting", navReference:"Complete example",
      sideNote:"This guide follows the current repository implementation; unsupported features are not assumed.",
      heroEyebrow:"THE VELOX PROGRAMMING LANGUAGE", heroTitle:"Simple like Python. Safe like Rust. Fast like C.",
      heroCopy:"Start with your first .vx program. Learn the file format, syntax, types, compilation workflow, and the Stage 1 self-hosted compiler.",
      metaOne:"Bilingual guide · 中文 / English", metaTwo:"Based on the current repository",
      accuracyTitle:"Implementation status", accuracyText:"This guide documents syntax and commands evidenced by the current project and its tests. Velox is under active development; it is not a complete replacement for Rust, Python, or C. The compiler and tests remain the source of truth.",
      introTitle:"Meet Velox", introP:"Velox is a programming language progressing toward self-hosting. The current project uses a Rust Stage 0 compiler to compile the Velox-written Stage 1 compiler. The compiler emits C, which is then built by a native C compiler to produce later stages.",
      introCard1:"Explicit functions, type annotations, code blocks, and familiar control flow.", introCard2:"Core types include i64, f64, bool, str, void, and user-defined structs.", introCard3:"The current backend translates Velox into C, then invokes a native C compiler such as clang.",
      quickTitle:"Quick start", quickP:"Run these commands from the repository root. The project has multiple Cargo binaries, so explicitly select the compiler with --bin veloxc.",
      prereqTitle:"Requirements", prereq1:"Rust toolchain and Cargo to build the Stage 0 compiler.", prereq2:"A C compiler such as clang to turn generated C into a native executable.", prereq3:"A terminal opened at the repository root.",
      quickNote:"The invocation shown matches the current project development/test workflow. If veloxc is already built, invoke target/release/veloxc directly with the same source-file and -o argument form.",
      formatTitle:"File format & style", fileExtTitle:"Source files", fileExtP:"Velox source files use the .vx extension. A typical file imports dependencies with using, then declares structs, functions, and the main entry point.",
      styleTitle:"Recommended style", style1:"Use four spaces for indentation.", style2:"Terminate statements with semicolons.", style3:"Use braces for code blocks.", style4:"Put type annotations after variable names.",
      syntaxTitle:"Basic syntax", commentsTitle:"Comments", commentsP:"Examples and source files use C-style line and block comments. When relying on a specific comment form, verify it against the lexer implementation.",
      variablesTitle:"Variable declarations", variablesP:"Use let for a local binding and let mut for a mutable binding. Types can be written explicitly. Type inference is still being refined, so explicit annotations are recommended while learning.",
      mutTitle:"let vs let mut", mutText:"Use an immutable binding when it will not be reassigned. Use let mut when reassignment is required. The mut modifier follows let.",
      literalsTitle:"Literals", tableLiteral:"Category", tableExample:"Examples", tableMeaning:"Meaning", litInt:"Integer values, commonly used with i64.", litFloat:"Floating-point values, commonly used with f64.", litBool:"Boolean values.", litString:"String literal.",
      typesTitle:"Type system", typesP:"The current self-hosted type checker exposes the basic types below. Struct types receive their own internal type identifiers.",
      typeName:"Type", typePurpose:"Purpose", typeSample:"Declaration example", typeI64:"64-bit integer", typeF64:"64-bit floating-point number", typeBool:"Boolean value", typeStr:"String", typeVoid:"Return type for functions with no value", typeStruct:"User-defined struct type",
      typeAdviceTitle:"Type annotation advice", typeAdviceText:"At this stage, annotate local variables, function parameters, and return values explicitly. In particular, writing let p: Point = ... avoids relying on inference paths that are still being completed.",
      controlTitle:"Control flow", rangeNote:"The range-loop example uses 0..5. The current project tests cover forms such as 0..3; consult parser/codegen tests for exact boundary semantics.",
      functionsTitle:"Functions", functionsP:"Declare functions with fn. Parameters use name: type syntax, and -> introduces the return type. Use void for a function with no return value.",
      fnRule1:"The parameter list follows the function name.", fnRule2:"Separate parameters with commas.", fnRule3:"Use return to return a value from a function.", fnRule4:"main is the entry function used by current examples and compiler invocation.",
      structsTitle:"Structs", structsP:"Use struct to define an aggregate data type. Each field has a name and type; initialize a value using the struct name and named fields.",
      structNote:"Struct declarations, initialization, and field access are part of the current implementation. Inference for local struct variables and struct-returning calls should still be verified with regression tests. This example explicitly annotates Point.",
      modulesTitle:"Modules & dependencies", modulesP:"The current project uses using \"path.vx\" to include another source file. Dependency paths are resolved relative to the referring file; the compiler collects dependencies and processes them before their dependents.",
      depTitle:"Dependency checks", depText:"The compiler avoids processing the same dependency more than once and detects dependency cycles. The current regression tests expect E0203 for a circular dependency.",
      keywordsTitle:"Keyword reference", keywordsP:"This table lists core reserved words explicitly used by current examples and implementation. It is not a substitute for the complete lexer token list; consult the lexer when adding keywords.",
      kwFn:"Declare a function", kwLet:"Declare a binding", kwMut:"Mark a binding mutable", kwReturn:"Return from a function", kwIf:"Conditional branch", kwElse:"Alternative branch", kwWhile:"Condition-controlled loop", kwFor:"Range loop", kwIn:"Iteration syntax in a range loop", kwStruct:"Declare a struct", kwUsing:"Include another source file", kwBool:"Boolean literals",
      operatorsTitle:"Operators", operatorsP:"The operators below appear in current syntax examples and code generation. Exact precedence and complete combinations are defined by the parser implementation.",
      opGroup:"Group", opSymbols:"Symbols", opUse:"Purpose", opArithmetic:"Arithmetic", opArithmeticUse:"Numeric operations", opCompare:"Comparison", opCompareUse:"Compare two expressions", opLogic:"Logical", opLogicUse:"Combine or negate conditions", opAssign:"Assignment", opAssignUse:"Assign to a mutable variable", opRange:"Range", opRangeUse:"Range expression for for loops", opField:"Field access", opFieldUse:"Access a struct field", opReturn:"Return type", opReturnUse:"Function return-type marker",
      compilerTitle:"Compiler workflow", compilerP:"The current compiler pipeline can be summarized as: read source files, resolve dependencies, lex, parse, type-check, generate C, and invoke a native C compiler.",
      pipe1:"Velox source and dependencies", pipe2:"Tokens and AST", pipe3:"Type and semantic checks", pipe4:"Generate C source", pipe5:"Native executable",
      compileCmdTitle:"Common compile commands", compilerNote:"The -o option selects the output path. The generated C intermediate path depends on the current driver implementation. To test the C backend independently, use the repository's codegen_check harness.",
      bootstrapTitle:"Stage 0 → Stage 1 → Stage 2 → Stage 3", bootstrapP:"A key Velox milestone is compiling the Velox compiler with itself. Stage 0 is the Rust compiler; Stage 1 is the VX compiler built by Stage 0; Stage 1 generates Stage 2, and later stages continue compiling the same VX compiler source.",
      stage0:"The bootstrap compiler implemented in Rust.", stage1:"The VX compiler built by Stage 0 from selfhost/src/main.vx.", stage2:"The next compiler stage generated by executing Stage 1.", stage3:"Continue self-compilation and check that generated output is stable.",
      bootstrapProofTitle:"Determinism check", bootstrapProofText:"The project has verified that the Stage 2 and Stage 3 generated C files are byte-for-byte identical, and that Stage 3 can execute the self-compilation workflow again. This is important bootstrap consistency evidence, not proof that every language feature is complete.",
      errorsTitle:"Troubleshooting", err1Q:"Cargo reports multiple binaries", err1A:"Select the compiler explicitly: cargo run --bin veloxc -- ... . Do not omit --bin veloxc.",
      err2Q:"clang cannot be found", err2A:"Install and configure a C compiler, then ensure clang is available on PATH. The current Velox backend relies on an external C compiler to build native programs.",
      err3Q:"A struct local variable fails to compile", err3A:"During development, annotate the variable explicitly, for example let p: Pair = Pair { ... };. The inferred-type code-generation path still needs regression coverage.",
      err4Q:"Circular dependency", err4A:"Inspect the graph of using declarations for a cycle. The current driver regression test uses error code E0203.",
      err5Q:"Program output differs from expectation", err5A:"Confirm parsing and type checking first, then inspect generated C. Compile and run that C output separately to isolate frontend, backend, and native compilation issues.",
      referenceTitle:"Complete example: define, compile, run", endTitle:"Keep building Velox.", endText:"Start with small programs, type checking, and code-generation tests. Expand language capabilities incrementally, and keep regression tests and bootstrap verification in step with implementation.",
    },
    zh: {}
  };

  const zh = {
    contents:"目录 / CONTENTS", navIntro:"介绍", navQuick:"快速开始", navFormat:"文件格式", navSyntax:"基础语法", navTypes:"类型系统", navControl:"流程控制", navFunctions:"函数", navStructs:"结构体", navModules:"模块与依赖", navKeywords:"关键字", navOperators:"运算符", navCompiler:"编译器工作流", navBootstrap:"自举", navErrors:"错误排查", navReference:"参考示例",
    sideNote:"以当前仓库实现为准；尚未实现的语言特性不会在此假定存在。",
    heroEyebrow:"THE VELOX PROGRAMMING LANGUAGE", heroTitle:"简单如 Python，安全如 Rust，快速如 C。",
    heroCopy:"从第一段 .vx 程序开始，了解 Velox 的文件格式、语法、类型、编译流程，以及 Stage 1 自举编译器。",
    metaOne:"双语指南 · 中文 / English", metaTwo:"面向当前仓库实现",
    accuracyTitle:"实现状态说明", accuracyText:"本指南描述当前项目中已经出现并经过测试的语法与命令。Velox 仍在开发中；不要把本指南理解为完整的 Rust、Python 或 C 替代品。具体可用特性以当前编译器和测试为准。",
    introTitle:"认识 Velox", introP:"Velox 是一门正在自举（self-hosting）过程中的编程语言。当前工程采用 Rust 编写 Stage 0 编译器，再由 Velox 编写 Stage 1 编译器，并通过 C 代码生成与本机 C 编译器构建后续阶段。",
    introCard1:"采用显式函数、类型注解、代码块和直观的控制流程。", introCard2:"具有 i64、f64、bool、str、void 与结构体类型等基础类型。", introCard3:"当前后端将 Velox 程序转换为 C，再调用 clang 等 C 编译器生成本机可执行文件。",
    quickTitle:"快速开始", quickP:"在仓库根目录执行以下命令。由于工程包含多个 Cargo binary，调用编译器时请明确指定 --bin veloxc。",
    prereqTitle:"环境要求", prereq1:"Rust 工具链与 Cargo：构建 Stage 0 编译器。", prereq2:"C 编译器（例如 clang）：将生成的 C 源码编译为本机程序。", prereq3:"终端：在仓库根目录运行命令。",
    quickNote:"上面的调用方式与当前项目测试/开发流程一致。若你使用已构建的 veloxc，可直接调用 target/release/veloxc，并保留相同的源文件与 -o 参数形式。",
    formatTitle:"文件格式与代码风格", fileExtTitle:"源文件", fileExtP:"Velox 源代码使用 .vx 扩展名。文件通常以 using 声明依赖，再定义结构体、函数和 main 入口。",
    styleTitle:"推荐格式", style1:"使用 4 个空格缩进。", style2:"语句以分号结束。", style3:"代码块使用花括号。", style4:"类型注解写在变量名之后。",
    syntaxTitle:"基础语法", commentsTitle:"注释", commentsP:"当前示例和源码采用 C 风格单行注释与块注释；如果你正在验证某种注释形式，请以 lexer 的实际支持为准。",
    variablesTitle:"变量声明", variablesP:"let 声明局部变量；let mut 声明可变变量。类型可以显式写出。当前编译器支持的类型推断仍在完善中，建议初学时优先写出类型注解。",
    mutTitle:"let 与 let mut", mutText:"不可变绑定用于不会重新赋值的变量；需要重新赋值时使用 let mut。不要将 mut 放在类型前面。",
    literalsTitle:"字面量", tableLiteral:"类别", tableExample:"示例", tableMeaning:"说明", litInt:"整数值，常用于 i64。", litFloat:"浮点值，常用于 f64。", litBool:"布尔值。", litString:"字符串字面量。",
    typesTitle:"类型系统", typesP:"当前自托管类型检查器中可见的基础类型 ID 如下。结构体类型在内部以独立类型编号表示。",
    typeName:"类型", typePurpose:"用途", typeSample:"声明示例", typeI64:"64 位整数", typeF64:"64 位浮点数", typeBool:"布尔值", typeStr:"字符串", typeVoid:"无返回值函数的返回类型", typeStruct:"用户定义的结构体类型",
    typeAdviceTitle:"类型注解建议", typeAdviceText:"当前阶段建议给局部变量、函数参数和返回值写清楚类型。特别是结构体局部变量，显式写成 let p: Point = ...; 可以避免依赖尚在完善的推断路径。",
    controlTitle:"流程控制", rangeNote:"范围循环示例使用 0..5。当前项目测试已覆盖类似 0..3 的循环形式；范围边界的精确语义请以当前 parser/codegen 测试为准。",
    functionsTitle:"函数", functionsP:"使用 fn 声明函数。参数采用 name: type 形式；返回类型通过 -> 指定。没有返回值的函数可使用 void。",
    fnRule1:"函数名后面紧跟参数列表。", fnRule2:"参数之间使用逗号分隔。", fnRule3:"return 用于从函数返回值。", fnRule4:"main 是当前示例和编译入口使用的程序入口函数。",
    structsTitle:"结构体", structsP:"使用 struct 定义聚合数据类型。字段声明由字段名和类型组成；初始化时使用结构体名与命名字段。",
    structNote:"结构体定义、初始化与字段访问已进入项目实现范围；局部变量自动推断、结构体返回值推断等代码生成边界仍应通过回归测试确认。教程示例显式标注 Point 类型。",
    modulesTitle:"模块与依赖", modulesP:"当前项目使用 using \"path.vx\" 引入其他源文件。依赖路径按引用文件位置解析；编译器会收集依赖并在依赖方之前处理。",
    depTitle:"依赖检查", depText:"编译器会避免重复处理同一依赖，并检测循环依赖。当前回归测试中，循环依赖会报告 E0203。",
    keywordsTitle:"关键字速查", keywordsP:"下表列出当前语言示例与实现中明确使用的核心保留词。它不是对 lexer 全部 token 的替代清单；新增关键字请以 lexer 实现为准。",
    kwFn:"声明函数", kwLet:"声明变量绑定", kwMut:"声明可变绑定", kwReturn:"从函数返回", kwIf:"条件分支", kwElse:"备选分支", kwWhile:"条件循环", kwFor:"范围循环", kwIn:"范围循环中的迭代语法", kwStruct:"声明结构体", kwUsing:"引入源文件", kwBool:"布尔字面量",
    operatorsTitle:"运算符", operatorsP:"以下运算符出现在当前语法示例和编译器代码生成中。优先级和完整组合规则以 parser 的实现为准。",
    opGroup:"分组", opSymbols:"符号", opUse:"用途", opArithmetic:"算术", opArithmeticUse:"数值运算", opCompare:"比较", opCompareUse:"比较两个表达式", opLogic:"逻辑", opLogicUse:"逻辑组合与取反", opAssign:"赋值", opAssignUse:"给可变变量赋值", opRange:"范围", opRangeUse:"for 范围表达式", opField:"字段", opFieldUse:"访问结构体字段", opReturn:"返回类型", opReturnUse:"函数返回类型标记",
    compilerTitle:"编译器工作流", compilerP:"当前编译器的核心过程可以概括为：读取文件、解析依赖、词法分析、语法分析、类型检查、生成 C，再调用本机 C 编译器。",
    pipe1:"Velox 源文件与依赖", pipe2:"Token 与 AST", pipe3:"类型与语义检查", pipe4:"生成 C 源码", pipe5:"本机可执行文件",
    compileCmdTitle:"常用编译命令", compilerNote:"输出路径由 -o 指定。生成 C 的中间文件路径取决于当前 driver 实现；若需单独检查 C 后端，可使用仓库中的 codegen_check 测试 harness。",
    bootstrapTitle:"Stage 0 → Stage 1 → Stage 2 → Stage 3", bootstrapP:"Velox 的关键里程碑是用 Velox 编译器编译 Velox 编译器自身。当前仓库的 Stage 0 是 Rust 编译器；Stage 1 是由 Stage 0 编译出来的 VX 编译器；Stage 1 生成 Stage 2，后续阶段继续编译同一份 VX 编译器源码。",
    stage0:"Rust 实现的引导编译器。", stage1:"由 Stage 0 编译 selfhost/src/main.vx 得到的 VX 编译器。", stage2:"由 Stage 1 执行并生成的下一阶段编译器。", stage3:"继续自编译，并检查生成结果是否稳定。",
    bootstrapProofTitle:"确定性验证", bootstrapProofText:"项目已验证 Stage 2 与 Stage 3 生成的 C 文件逐字节一致，并且 Stage 3 能再次执行自编译流程。这是重要的自举一致性证据，但不等同于所有语言功能均已完成。",
    errorsTitle:"常见错误与排查", err1Q:"Cargo 提示有多个 binary", err1A:"命令中指定编译器目标：cargo run --bin veloxc -- ...。不要省略 --bin veloxc。",
    err2Q:"找不到 clang", err2A:"安装并配置可用的 C 编译器，确认 clang 在 PATH 中；Velox 当前后端依赖外部 C 编译器生成本机程序。",
    err3Q:"结构体局部变量编译失败", err3A:"开发阶段优先显式标注变量类型，例如 let p: Pair = Pair { ... };。自动推断路径仍需由 codegen 回归测试确认。",
    err4Q:"循环依赖", err4A:"检查 using 文件关系是否形成环。当前驱动器的循环依赖回归使用错误码 E0203。",
    err5Q:"程序输出与预期不同", err5A:"先确认 parser 和 type checker 均通过，再检查生成的 C 文件；最后单独编译并运行 C 输出，以区分前端、后端与本机编译阶段的问题。",
    referenceTitle:"完整示例：从定义到运行", endTitle:"Keep building Velox.", endText:"从小程序、类型检查与代码生成测试开始，逐步扩展语言能力，并始终让回归测试和自举验证跟上实现。",
  };
  translations.zh = zh;

  const languageButton = document.getElementById("langToggle");
  const toast = document.getElementById("toast");
  let currentLanguage = "zh";

  function setLanguage(language) {
    currentLanguage = language;
    document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
    document.querySelectorAll("[data-i18n]").forEach((element) => {
      const key = element.dataset.i18n;
      const value = translations[language][key];
      if (typeof value === "string") element.textContent = value;
    });
    languageButton.textContent = language === "zh" ? "EN" : "中文";
    languageButton.setAttribute("aria-label", language === "zh" ? "Switch to English" : "切换到中文");
    try { localStorage.setItem("velox-tutorial-language", language); } catch (_) {}
  }

  languageButton.addEventListener("click", () => {
    setLanguage(currentLanguage === "zh" ? "en" : "zh");
  });

  document.querySelectorAll(".copy-btn").forEach((button) => {
    button.addEventListener("click", async () => {
      const text = button.dataset.copy || "";
      try {
        await navigator.clipboard.writeText(text);
        toast.textContent = currentLanguage === "zh" ? "已复制到剪贴板" : "Copied to clipboard";
      } catch (_) {
        toast.textContent = currentLanguage === "zh" ? "复制失败：请手动选择代码" : "Copy failed: select the code manually";
      }
      toast.classList.add("show");
      window.setTimeout(() => toast.classList.remove("show"), 1800);
    });
  });

  const backTop = document.getElementById("backTop");
  window.addEventListener("scroll", () => {
    backTop.classList.toggle("visible", window.scrollY > 500);
  }, { passive: true });
  backTop.addEventListener("click", () => window.scrollTo({ top: 0, behavior: "smooth" }));

  const sections = [...document.querySelectorAll("main section[id]")];
  const navLinks = [...document.querySelectorAll("#toc a")];
  if ("IntersectionObserver" in window) {
    const observer = new IntersectionObserver((entries) => {
      entries.forEach((entry) => {
        if (!entry.isIntersecting) return;
        navLinks.forEach((link) => link.classList.toggle("active", link.getAttribute("href") === `#${entry.target.id}`));
      });
    }, { rootMargin: "-20% 0px -70% 0px" });
    sections.forEach((section) => observer.observe(section));
  }

  try {
    const saved = localStorage.getItem("velox-tutorial-language");
    if (saved === "en" || saved === "zh") currentLanguage = saved;
  } catch (_) {}
  setLanguage(currentLanguage);
})();