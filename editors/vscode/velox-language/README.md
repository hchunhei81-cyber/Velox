# Velox Language Support

VS Code language support for the Velox programming language.

## Features

- `.vx` file recognition
- TextMate syntax highlighting
- Struct, function, field and primitive-type highlighting
- Code snippets for functions, structs, control flow and common expressions
- Automatic bracket pairing and indentation
- Comment toggling and code folding
- Workspace editor color customization

## Install locally

1. Open this folder in VS Code:
   `editors/vscode/velox-language`
2. Press `F5` to launch an Extension Development Host.
3. Open a `.vx` file in the new window.

Alternatively, use **Extensions: Install from VSIX...** after packaging the extension.

## Snippets

| Prefix | Description |
|---|---|
| `fn` | Function |
| `main` | Main function |
| `struct` | Struct declaration |
| `newstruct` | Struct literal |
| `mut` | Mutable variable |
| `if` / `ife` | Conditional |
| `while` | While loop |
| `pfor` | Parallel for loop |
| `pr` / `pln` | Print |
| `using` | Import source file |

## Current limitations

This extension currently uses TextMate grammar and snippets. Semantic completion,
go-to-definition, diagnostics and formatting require a future Velox language server.
