use crate::ast::*;
use crate::lexer::{Span, SpannedToken, Tok};

pub struct Parser {
    toks: Vec<SpannedToken>,
    pos: usize,
    last_span: Option<Span>,
    error_span: Option<Span>,
    condition_depth: usize,
}

impl Parser {
    pub fn new(toks: Vec<Tok>) -> Self {
        let tokens = toks
            .into_iter()
            .map(|kind| SpannedToken {
                kind,
                span: Span::default(),
            })
            .collect();
        Self::new_spanned(tokens)
    }

    pub fn new_spanned(mut toks: Vec<SpannedToken>) -> Self {
        if !matches!(toks.last().map(|t| &t.kind), Some(Tok::Eof)) {
            toks.push(SpannedToken {
                kind: Tok::Eof,
                span: Span::default(),
            });
        }
        Self {
            toks,
            pos: 0,
            last_span: None,
            error_span: None,
            condition_depth: 0,
        }
    }

    fn current_span(&self) -> Span {
        self.toks
            .get(self.pos)
            .map(|t| t.span)
            .or(self.last_span)
            .unwrap_or_default()
    }

    pub fn program_with_span(mut self) -> Result<Program, (String, Span)> {
        match self.program() {
            Ok(program) => Ok(program),
            Err(message) => {
                let span = self.error_span.unwrap_or_else(|| self.current_span());
                Err((message, span))
            }
        }
    }
    fn peek(&self) -> &Tok {
        self.toks
            .get(self.pos)
            .or_else(|| self.toks.last())
            .map(|t| &t.kind)
            .unwrap_or(&Tok::Eof)
    }

    fn bump(&mut self) -> Tok {
        let token = self.toks.get(self.pos).cloned().unwrap_or(SpannedToken {
            kind: Tok::Eof,
            span: Span::default(),
        });

        if token.kind != Tok::Eof {
            self.pos += 1;
            self.last_span = Some(token.span);
        }
        token.kind
    }
    fn eat(&mut self, t: &Tok) -> Result<(), String> {
        if self.peek() == t {
            self.bump();
            Ok(())
        } else {
            self.error_span = Some(self.current_span());
            Err(format!("expected {:?}, got {:?}", t, self.peek()))
        }
    }
    fn ident(&mut self) -> Result<String, String> {
        match self.bump() {
            Tok::Ident(s) => Ok(s),
            t => {
                self.error_span = self.last_span;
                Err(format!("expected ident, got {:?}", t))
            }
        }
    }
    fn ty(&mut self) -> Result<Ty, String> {
        match self.bump() {
            Tok::Ident(s) => match s.as_str() {
                "i64" => Ok(Ty::I64),
                "f64" => Ok(Ty::F64),
                "bool" => Ok(Ty::Bool),
                "str" => Ok(Ty::Str),
                "void" => Ok(Ty::Void),
                _ => Ok(Ty::Struct(s)),
            },
            t => {
                self.error_span = self.last_span;
                Err(format!("expected type, got {:?}", t))
            }
        }
    }

    pub fn program(&mut self) -> Result<Program, String> {
        let mut fns = Vec::new();
        let mut structs = Vec::new();
        let mut python_imports = Vec::new();
        while *self.peek() != Tok::Eof {
            if matches!(self.peek(), Tok::Ident(s) if s == "import") {
                self.bump();
                let module = self.ident()?;
                if python_imports.contains(&module) {
                    return Err(format!(
                        "E0500: Python module `{}` is imported more than once",
                        module
                    ));
                }
                python_imports.push(module);
                self.eat(&Tok::Semi)?;
                continue;
            }

            let vis = match self.peek() {
                Tok::Public => {
                    self.bump();
                    Vis::Public
                }
                Tok::Export => {
                    self.bump();
                    Vis::Export
                }
                _ => Vis::Private,
            };

            if *self.peek() == Tok::Struct {
                self.bump();
                let name = self.ident()?;
                self.eat(&Tok::LBrace)?;
                let mut fields = Vec::new();
                while *self.peek() != Tok::RBrace {
                    let field = self.ident()?;
                    self.eat(&Tok::Colon)?;
                    let ty = self.ty()?;
                    fields.push((field, ty));
                    if *self.peek() == Tok::Comma || *self.peek() == Tok::Semi {
                        self.bump();
                    } else if *self.peek() != Tok::RBrace {
                        return Err("expected `,` or `}` in struct".into());
                    }
                }
                self.eat(&Tok::RBrace)?;
                structs.push(StructDef {
                    name,
                    vis,
                    origin: String::new(),
                    fields,
                });
            } else {
                fns.push(self.fn_def(vis)?);
            }
        }
        Ok(Program {
            fns,
            structs,
            python_imports,
        })
    }

    fn fn_def(&mut self, vis: Vis) -> Result<FnDef, String> {
        self.eat(&Tok::Fn)?;
        let name = self.ident()?;
        self.eat(&Tok::LParen)?;
        let mut params = Vec::new();
        if *self.peek() != Tok::RParen {
            loop {
                let p = self.ident()?;
                self.eat(&Tok::Colon)?;
                let t = self.ty()?;
                params.push((p, t));
                if *self.peek() == Tok::Comma {
                    self.bump();
                } else {
                    break;
                }
            }
        }
        self.eat(&Tok::RParen)?;
        let ret = if *self.peek() == Tok::Arrow {
            self.bump();
            self.ty()?
        } else {
            Ty::Void
        };
        let body = self.block()?;
        Ok(FnDef {
            name,
            vis,
            origin: String::new(),
            is_extern: false,
            params,
            ret,
            body,
        })
    }

    fn block(&mut self) -> Result<Vec<Stmt>, String> {
        self.eat(&Tok::LBrace)?;
        let mut stmts = Vec::new();
        while *self.peek() != Tok::RBrace {
            if *self.peek() == Tok::Eof {
                return Err("unexpected EOF: missing `}`".into());
            }
            stmts.push(self.stmt()?);
        }
        self.eat(&Tok::RBrace)?;
        Ok(stmts)
    }

    fn stmt(&mut self) -> Result<Stmt, String> {
        match self.peek() {
            Tok::Let => {
                self.bump();
                let mut_ = if *self.peek() == Tok::Mut {
                    self.bump();
                    true
                } else {
                    false
                };
                let name = self.ident()?;
                let ty = if *self.peek() == Tok::Colon {
                    self.bump();
                    Some(self.ty()?)
                } else {
                    None
                };
                self.eat(&Tok::Eq)?;
                let init = self.expr()?;
                self.eat(&Tok::Semi)?;
                Ok(Stmt::Let {
                    name,
                    mut_,
                    ty,
                    init,
                })
            }
            Tok::Break => {
                self.bump();
                self.eat(&Tok::Semi)?;
                Ok(Stmt::Break)
            }
            Tok::Continue => {
                self.bump();
                self.eat(&Tok::Semi)?;
                Ok(Stmt::Continue)
            }
            Tok::Return => {
                self.bump();
                if *self.peek() == Tok::Semi {
                    self.bump();
                    return Ok(Stmt::Return(None));
                }
                let e = self.expr()?;
                self.eat(&Tok::Semi)?;
                Ok(Stmt::Return(Some(e)))
            }
            Tok::If => {
                self.bump();
                let cond = self.condition_expr()?;
                let then = self.block()?;
                let els = if *self.peek() == Tok::Else {
                    self.bump();
                    if *self.peek() == Tok::If {
                        vec![self.stmt()?]
                    } else {
                        self.block()?
                    }
                } else {
                    Vec::new()
                };
                Ok(Stmt::If(cond, then, els))
            }
            Tok::While => {
                self.bump();
                let cond = self.condition_expr()?;
                let body = self.block()?;
                Ok(Stmt::While(cond, body))
            }
            Tok::For => {
                self.bump();
                let var = self.ident()?;
                self.eat(&Tok::In)?;
                let start = self.expr()?;
                self.eat(&Tok::DotDot)?;
                let end = self.range_end_expr()?;
                let body = self.block()?;
                Ok(Stmt::For {
                    var,
                    start,
                    end,
                    body,
                })
            }
            Tok::Parallel => {
                self.bump();
                self.eat(&Tok::For)?;
                let var = self.ident()?;
                self.eat(&Tok::In)?;
                let start = self.expr()?;
                self.eat(&Tok::DotDot)?;
                let end = self.range_end_expr()?;
                let body = self.block()?;
                Ok(Stmt::ParallelFor {
                    var,
                    start,
                    end,
                    body,
                })
            }
            _ => {
                let e = self.expr()?;
                let compound = match self.peek() {
                    Tok::PlusEq => Some(BinOp::Add),
                    Tok::MinusEq => Some(BinOp::Sub),
                    Tok::StarEq => Some(BinOp::Mul),
                    Tok::SlashEq => Some(BinOp::Div),
                    _ => None,
                };
                if *self.peek() == Tok::Eq || compound.is_some() {
                    let op = compound;
                    self.bump();
                    let rhs = self.expr()?;
                    self.eat(&Tok::Semi)?;
                    let value = match op {
                        Some(op) => combine_expr(e.clone(), op, rhs),
                        None => rhs,
                    };
                    match strip_located(&e) {
                        Expr::Var(name) => Ok(Stmt::Assign(name.clone(), value)),
                        Expr::Field(_, _) => Ok(Stmt::FieldAssign(e, value)),
                        _ => Err("invalid assign target".into()),
                    }
                } else if *self.peek() == Tok::PlusPlus || *self.peek() == Tok::MinusMinus {
                    let op = if *self.peek() == Tok::PlusPlus {
                        BinOp::Add
                    } else {
                        BinOp::Sub
                    };
                    self.bump();
                    self.eat(&Tok::Semi)?;
                    let value = combine_expr(e.clone(), op, Expr::Int(1));
                    match strip_located(&e) {
                        Expr::Var(name) => Ok(Stmt::Assign(name.clone(), value)),
                        Expr::Field(_, _) => Ok(Stmt::FieldAssign(e, value)),
                        _ => Err("invalid increment/decrement target".into()),
                    }
                } else {
                    self.eat(&Tok::Semi)?;
                    Ok(Stmt::Expr(e))
                }
            }
        }
    }

    fn range_end_expr(&mut self) -> Result<Expr, String> {
        self.condition_depth += 1;
        let result = self.expr();
        self.condition_depth -= 1;
        result
    }

    fn condition_expr(&mut self) -> Result<Expr, String> {
        // A bare identifier immediately followed by the block opener is a condition variable,
        // not a struct literal (e.g. `if ready { ... }`).
        if let Tok::Ident(name) = self.peek().clone() {
            if self.toks.get(self.pos + 1).map(|t| &t.kind) == Some(&Tok::LBrace) {
                self.bump();
                let span = self.last_span.unwrap_or_default();
                return Ok(located(Expr::Var(name), span));
            }
        }
        self.condition_depth += 1;
        let result = self.expr();
        self.condition_depth -= 1;
        result
    }

    fn expr(&mut self) -> Result<Expr, String> {
        let cond = self.logical_or()?;
        if *self.peek() != Tok::Question {
            return Ok(cond);
        }
        self.bump();
        let yes = self.expr()?;
        self.eat(&Tok::Colon)?;
        let no = self.expr()?;
        let start = expr_span(&cond).map(|s| s.start);
        let end = expr_span(&no).map(|s| s.end);
        let node = Expr::Ternary(Box::new(cond), Box::new(yes), Box::new(no));
        Ok(match (start, end) {
            (Some(start), Some(end)) => located(node, Span { start, end }),
            _ => node,
        })
    }

    fn logical_or(&mut self) -> Result<Expr, String> {
        let mut left = self.logical_and()?;
        while *self.peek() == Tok::OrOr {
            self.bump();
            let right = self.logical_and()?;
            left = combine_expr(left, BinOp::Or, right);
        }
        Ok(left)
    }

    fn logical_and(&mut self) -> Result<Expr, String> {
        let mut left = self.bit_or()?;
        while *self.peek() == Tok::AndAnd {
            self.bump();
            let right = self.bit_or()?;
            left = combine_expr(left, BinOp::And, right);
        }
        Ok(left)
    }

    fn bit_or(&mut self) -> Result<Expr, String> {
        let mut left = self.bit_xor()?;
        while *self.peek() == Tok::Pipe {
            self.bump();
            let right = self.bit_xor()?;
            left = combine_expr(left, BinOp::BitOr, right);
        }
        Ok(left)
    }

    fn bit_xor(&mut self) -> Result<Expr, String> {
        let mut left = self.bit_and()?;
        while *self.peek() == Tok::Caret {
            self.bump();
            let right = self.bit_and()?;
            left = combine_expr(left, BinOp::BitXor, right);
        }
        Ok(left)
    }

    fn bit_and(&mut self) -> Result<Expr, String> {
        let mut left = self.cmp()?;
        while *self.peek() == Tok::Amp {
            self.bump();
            let right = self.cmp()?;
            left = combine_expr(left, BinOp::BitAnd, right);
        }
        Ok(left)
    }

    fn cmp(&mut self) -> Result<Expr, String> {
        let mut l = self.shift()?;
        loop {
            let op = match self.peek() {
                Tok::EqEq => BinOp::Eq,
                Tok::NotEq => BinOp::Ne,
                Tok::Lt => BinOp::Lt,
                Tok::Gt => BinOp::Gt,
                Tok::Le => BinOp::Le,
                Tok::Ge => BinOp::Ge,
                _ => break,
            };
            self.bump();
            let r = self.shift()?;
            l = combine_expr(l, op, r);
        }
        Ok(l)
    }

    fn shift(&mut self) -> Result<Expr, String> {
        let mut left = self.add()?;
        loop {
            let op = match self.peek() {
                Tok::Shl => BinOp::Shl,
                Tok::Shr => BinOp::Shr,
                _ => break,
            };
            self.bump();
            let right = self.add()?;
            left = combine_expr(left, op, right);
        }
        Ok(left)
    }

    fn add(&mut self) -> Result<Expr, String> {
        let mut l = self.mul()?;
        loop {
            let op = match self.peek() {
                Tok::Plus => BinOp::Add,
                Tok::Minus => BinOp::Sub,
                _ => break,
            };
            self.bump();
            let r = self.mul()?;
            l = combine_expr(l, op, r);
        }
        Ok(l)
    }

    fn mul(&mut self) -> Result<Expr, String> {
        let mut l = self.unary()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let r = self.unary()?;
            l = combine_expr(l, op, r);
        }
        Ok(l)
    }

    fn unary(&mut self) -> Result<Expr, String> {
        if *self.peek() == Tok::Minus {
            self.bump();
            let start = self.last_span.unwrap_or_default().start;
            let inner = self.unary()?;
            let end = expr_span(&inner).map(|s| s.end).unwrap_or(start);
            return Ok(located(Expr::Neg(Box::new(inner)), Span { start, end }));
        }
        if *self.peek() == Tok::Bang {
            self.bump();
            let start = self.last_span.unwrap_or_default().start;
            let inner = self.unary()?;
            let end = expr_span(&inner).map(|s| s.end).unwrap_or(start);
            return Ok(located(Expr::Not(Box::new(inner)), Span { start, end }));
        }
        let mut e = self.atom()?;
        while *self.peek() == Tok::Dot {
            self.bump();
            let field = self.ident()?;
            let start = expr_span(&e).map(|s| s.start);
            let end = self.last_span.map(|s| s.end);
            let field_expr = Expr::Field(Box::new(e), field);
            e = match (start, end) {
                (Some(start), Some(end)) => located(field_expr, Span { start, end }),
                _ => field_expr,
            };
        }
        Ok(e)
    }

    fn named_fields(&mut self, close: Tok) -> Result<Vec<(String, Expr)>, String> {
        let mut fields = Vec::new();
        if *self.peek() != close {
            loop {
                let field = self.ident()?;
                self.eat(&Tok::Colon)?;
                let value = self.expr()?;
                fields.push((field, value));
                if *self.peek() == Tok::Comma {
                    self.bump();
                    if *self.peek() == close {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
        self.eat(&close)?;
        Ok(fields)
    }

    fn atom(&mut self) -> Result<Expr, String> {
        let token = self.bump();
        let token_span = self.last_span.unwrap_or_default();
        match token {
            Tok::Int(n) => Ok(located(Expr::Int(n), token_span)),
            Tok::Float(f) => Ok(located(Expr::Float(f), token_span)),
            Tok::True => Ok(located(Expr::Bool(true), token_span)),
            Tok::False => Ok(located(Expr::Bool(false), token_span)),
            Tok::Str(s) => Ok(located(Expr::Str(s), token_span)),
            Tok::Ident(name) => {
                if name == "py" {
                    self.eat(&Tok::Dot)?;
                    let module = self.ident()?;
                    self.eat(&Tok::Dot)?;
                    let function = self.ident()?;
                    self.eat(&Tok::LParen)?;
                    let mut args = Vec::new();
                    if *self.peek() != Tok::RParen {
                        loop {
                            args.push(self.expr()?);
                            if *self.peek() == Tok::Comma {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.eat(&Tok::RParen)?;
                    let end = self.last_span.unwrap_or(token_span).end;
                    return Ok(located(
                        Expr::PythonCall {
                            module,
                            function,
                            args,
                        },
                        Span {
                            start: token_span.start,
                            end,
                        },
                    ));
                }
                if *self.peek() == Tok::LParen {
                    self.bump();
                    let is_named_init = matches!(self.peek(), Tok::Ident(_))
                        && self
                            .toks
                            .get(self.pos + 1)
                            .is_some_and(|token| token.kind == Tok::Colon);
                    if is_named_init {
                        let fields = self.named_fields(Tok::RParen)?;
                        let end = self.last_span.unwrap_or(token_span).end;
                        let init = Expr::StructInit(name, fields);
                        return Ok(located(
                            located(init, token_span),
                            Span {
                                start: token_span.start,
                                end,
                            },
                        ));
                    }

                    let mut args = Vec::new();
                    if *self.peek() != Tok::RParen {
                        loop {
                            args.push(self.expr()?);
                            if *self.peek() == Tok::Comma {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.eat(&Tok::RParen)?;
                    let end = self.last_span.unwrap_or(token_span).end;
                    let call = Expr::Call(name, args);
                    Ok(located(
                        located(call, token_span),
                        Span {
                            start: token_span.start,
                            end,
                        },
                    ))
                } else if *self.peek() == Tok::LBrace && self.condition_depth == 0 {
                    self.bump();
                    let fields = self.named_fields(Tok::RBrace)?;
                    let end = self.last_span.unwrap_or(token_span).end;
                    let init = Expr::StructInit(name, fields);
                    Ok(located(
                        located(init, token_span),
                        Span {
                            start: token_span.start,
                            end,
                        },
                    ))
                } else {
                    Ok(located(Expr::Var(name), token_span))
                }
            }
            Tok::LParen => {
                let e = self.expr()?;
                self.eat(&Tok::RParen)?;
                Ok(e)
            }
            t => {
                self.error_span = self.last_span;
                Err(format!("unexpected token {:?}", t))
            }
        }
    }
}

fn expr_span(expr: &Expr) -> Option<Span> {
    match expr {
        Expr::Located(_, span) => Some(*span),
        _ => None,
    }
}

fn located(expr: Expr, span: Span) -> Expr {
    Expr::Located(Box::new(expr), span)
}

fn combine_expr(left: Expr, op: BinOp, right: Expr) -> Expr {
    let start = expr_span(&left).map(|s| s.start);
    let end = expr_span(&right).map(|s| s.end);
    let expr = Expr::Bin(Box::new(left), op, Box::new(right));

    match (start, end) {
        (Some(start), Some(end)) => located(expr, Span { start, end }),
        _ => expr,
    }
}

fn strip_located(mut expr: &Expr) -> &Expr {
    while let Expr::Located(inner, _) = expr {
        expr = inner;
    }
    expr
}

#[cfg(test)]
mod syntax_round_one_tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(source: &str) -> Program {
        let tokens = Lexer::new(source).tokenize_spanned().unwrap();
        Parser::new_spanned(tokens).program().unwrap()
    }

    #[test]
    fn supports_short_python_imports() {
        let program = parse("import math; import time; fn main() {}");
        assert_eq!(program.python_imports, vec!["math", "time"]);
    }

    #[test]
    fn rejects_legacy_python_import_spelling() {
        let tokens = Lexer::new("import python math; fn main() {}")
            .tokenize_spanned()
            .unwrap();
        let result = Parser::new_spanned(tokens).program();
        assert!(result.is_err());
    }

    #[test]
    fn supports_pub_and_default_private_visibility() {
        let program = parse(
            r#"pub struct PublicType { value: i64 }
               struct LocalType { value: i64 }
               pub fn exposed() {}
               fn internal() {}"#,
        );
        assert_eq!(program.structs[0].vis, Vis::Public);
        assert_eq!(program.structs[1].vis, Vis::Private);
        assert_eq!(program.fns[0].vis, Vis::Public);
        assert_eq!(program.fns[1].vis, Vis::Private);
    }

    #[test]
    fn omitted_function_return_type_means_void() {
        let program = parse("fn simple() {} fn explicit() -> void {}");
        assert_eq!(program.fns[0].ret, Ty::Void);
        assert_eq!(program.fns[1].ret, Ty::Void);
    }

    #[test]
    fn supports_call_and_brace_struct_initializers() {
        let program = parse(
            r#"struct User { name: str, age: i64 }
               fn main() {
                   let first = User(name: "Alice", age: 20);
                   let second = User { name: "Bob", age: 30 };
               }"#,
        );

        let body = &program.fns[0].body;
        let Stmt::Let { init: first, .. } = &body[0] else {
            panic!("expected first let");
        };
        let Stmt::Let { init: second, .. } = &body[1] else {
            panic!("expected second let");
        };

        assert!(matches!(
            strip_located(first),
            Expr::StructInit(name, _) if name == "User"
        ));
        assert!(matches!(
            strip_located(second),
            Expr::StructInit(name, _) if name == "User"
        ));
    }
}
