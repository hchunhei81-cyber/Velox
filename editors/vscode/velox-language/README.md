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
- Live compiler diagnostics with source ranges
- Semantic completion for functions, structs, parameters and local variables
- Struct field completion after member access
- Hover information for indexed symbols
- Go to Definition for symbols in the current document

## Language server

The extension starts the Velox language server automatically when a Velox file is opened in a workspace containing a Velox `Cargo.toml` project.

The language server is built with Cargo when its release binary is missing. Rust and Cargo must therefore be installed and available to the extension.

To inspect server output, open **View → Output** and select **Velox Language Server**.

## Install locally

1. Open this folder in VS Code:

   `editors/vscode/velox-language`

2. Press `F5` to launch an Extension Development Host.

3. Open a `.vx` file inside a Velox project.

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

- Go to Definition currently resolves symbols within the open document; cross-file navigation is not implemented.
- Symbol information is derived from the document's token stream and is not yet backed by a complete compiler semantic index.
- Formatting and rename support are not implemented.
