# Velox Self-hosted Compiler

The compiler implementation written in Velox itself.

## Bootstrap stages

- Stage 0: Rust bootstrap compiler (`veloxc`)
- Stage 1: Velox self-hosted compiler compiled by Stage 0
- Stage 2: Stage 1 compiles the self-hosted compiler again

## Initial implementation order

1. Lexer
2. AST
3. Parser
4. Type checker
5. C code generator
6. Compiler driver
7. CLI
