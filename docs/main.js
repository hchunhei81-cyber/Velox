const translations = {
  en: {
    navHome:"Home", navFeatures:"Features", navStart:"Quick start", navTutorial:"Tutorial",
    headerCta:"Read the tutorial", eyebrow:"A language in active development",
    heroTitle:'Build with clarity.<br><span>Move at the speed of C.</span>',
    heroDescription:"Velox is a compiled programming language designed around a simple idea: approachable syntax, strong type safety, and native performance.",
    primaryCta:"Explore the tutorial", secondaryCta:"Get started", github:"Visit Github", heroNote:"Rust-powered compiler · C code generation",
    codeReady:"Ready to compile", visualCaption:"Readable by design. Native by default.",
    principleOne:"Simple like Python", principleOneSub:"Clean, familiar syntax",
    principleTwo:"Safe like Rust", principleTwoSub:"Explicit types and checks",
    principleThree:"Fast like C", principleThreeSub:"Compiled native output",
    featuresKicker:"THE VELOX APPROACH", featuresTitle:'Less friction.<br><span>More control.</span>',
    featuresIntro:"A focused language and compiler toolchain, built to make the path from source code to native executable easy to understand.",
    feature1Title:"Readable source", feature1Text:"Functions, typed variables, loops, and structs use a compact syntax that keeps the code close to the problem.",
    feature2Title:"Static type checking", feature2Text:"Catch incompatible values and invalid operations before generated code is compiled.",
    feature3Title:"C code generation", feature3Text:"The compiler translates Velox source into C, which can then be built with a native C compiler.",
    feature4Title:"Self-hosting direction", feature4Text:"The self-hosted compiler is being developed through staged compilation and reproducible bootstrap checks.",
    startKicker:"FIRST STEPS", startTitle:'From <span>.vx</span><br>to executable.',
    startText:"Create a source file, invoke the Velox compiler, then compile the generated C output with Clang.",
    fullGuide:"Open the full setup guide", copy:"Copy", copied:"Copied",
    commentBuild:"# Build the Velox compiler", commentGenerate:"# Generate and build the native executable",
    terminalStatus:"Example workflow", ctaKicker:"KEEP EXPLORING",
    ctaTitle:"Learn the language, one concept at a time.", ctaButton:"Go to Tutorial",
    footerTagline:"Simple like Python. Safe like Rust. Fast like C."
  },
  zh: {
    navHome:"首页", navFeatures:"特性", navStart:"快速开始", navTutorial:"教程",
    headerCta:"阅读教程", eyebrow:"一门正在积极开发的语言",
    heroTitle:'清晰地构建。<br><span>拥有 C 一样的速度。</span>',
    heroDescription:"Velox 是一门编译型编程语言，目标明确：易读易学的语法、严格的静态类型检查，以及原生执行性能。",
    primaryCta:"探索教程", secondaryCta:"快速开始", github:"访问Github", heroNote:"Rust 驱动的编译器 · 生成 C 代码",
    codeReady:"准备编译", visualCaption:"为可读性而设计，默认生成原生代码。",
    principleOne:"像 Python 一样简单", principleOneSub:"简洁、熟悉的语法",
    principleTwo:"像 Rust 一样安全", principleTwoSub:"明确的类型与检查",
    principleThree:"像 C 一样快速", principleThreeSub:"编译为原生程序",
    featuresKicker:"VELOX 的设计理念", featuresTitle:'减少阻力。<br><span>掌控更多。</span>',
    featuresIntro:"专注的语言与编译工具链，让从源代码到原生可执行程序的过程清晰、直接。",
    feature1Title:"易读的源代码", feature1Text:"函数、类型标注变量、循环和结构体采用紧凑语法，让代码贴近问题本身。",
    feature2Title:"静态类型检查", feature2Text:"在编译生成代码之前，发现类型不兼容和无效操作。",
    feature3Title:"生成 C 代码", feature3Text:"Velox 编译器将源代码转换为 C，再交给原生 C 编译器构建。",
    feature4Title:"迈向自举编译", feature4Text:"自托管编译器通过分阶段编译和可复现的 bootstrap 检查持续开发。",
    startKicker:"从这里开始", startTitle:'从 <span>.vx</span><br>到可执行程序。',
    startText:"创建源文件，运行 Velox 编译器，再使用 Clang 编译生成的 C 代码。",
    fullGuide:"查看完整安装与编译教程", copy:"复制命令", copied:"已复制",
    commentBuild:"# 构建 Velox 编译器", commentGenerate:"# 生成并构建原生可执行程序",
    terminalStatus:"示例工作流", ctaKicker:"继续探索",
    ctaTitle:"循序渐进，掌握 Velox 的每个概念。", ctaButton:"进入教程",
    footerTagline:"像 Python 一样简单，像 Rust 一样安全，像 C 一样快速。"
  }
};

let currentLanguage = localStorage.getItem("velox-language") || "en";
const languageToggle = document.getElementById("languageToggle");

function setLanguage(language) {
  currentLanguage = language;
  localStorage.setItem("velox-language", language);
  document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
  document.querySelectorAll("[data-i18n]").forEach((element) => {
    const key = element.dataset.i18n;
    if (translations[language][key]) element.textContent = translations[language][key];
  });
  document.querySelectorAll("[data-i18n-html]").forEach((element) => {
    const key = element.dataset.i18nHtml;
    if (translations[language][key]) element.innerHTML = translations[language][key];
  });
  languageToggle.textContent = language === "en" ? "中文" : "EN";
  languageToggle.setAttribute("aria-label", language === "en" ? "切换到中文" : "Switch to English");
}

languageToggle.addEventListener("click", () => {
  setLanguage(currentLanguage === "en" ? "zh" : "en");
});

const menuToggle = document.getElementById("menuToggle");
const mainNav = document.querySelector(".main-nav");
menuToggle.addEventListener("click", () => {
  const isOpen = mainNav.classList.toggle("open");
  menuToggle.setAttribute("aria-expanded", String(isOpen));
  menuToggle.setAttribute("aria-label", isOpen ? "Close navigation" : "Open navigation");
});
mainNav.querySelectorAll("a").forEach((link) => {
  link.addEventListener("click", () => {
    mainNav.classList.remove("open");
    menuToggle.setAttribute("aria-expanded", "false");
  });
});

document.getElementById("year").textContent = new Date().getFullYear();

document.getElementById("copyCommands").addEventListener("click", async (event) => {
  const commands = [
    "cargo build --release",
    "cargo run --release --bin veloxc -- hello.vx -o hello",
    "./hello"
  ].join("\n");
  const button = event.currentTarget;
  try {
    await navigator.clipboard.writeText(commands);
    button.textContent = translations[currentLanguage].copied;
    window.setTimeout(() => {
      button.textContent = translations[currentLanguage].copy;
    }, 1600);
  } catch {
    button.textContent = "Unavailable";
  }
});

setLanguage(currentLanguage);
