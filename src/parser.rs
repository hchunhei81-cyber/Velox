use crate::ast::*;
use crate::lexer::{Span, SpannedToken, Tok};

pub struct Parser {
    toks: Vec<SpannedToken>,
    pos: usize,
    last_span: Option<Span>,
    error_span: Option<Span>,
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
                let span = self
                    .error_span
                    .or(self.last_span)
                    .unwrap_or_else(|| self.current_span());
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
        while *self.peek() != Tok::Eof {
            let start_pos = self.pos;
            let vis = match self.peek() {
                Tok::Public => {
                    self.bump();
                    Vis::Public
                }
                Tok::Private => {
                    self.bump();
                    Vis::Private
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
                // fn_def consumes its own visibility modifier.
                self.pos = start_pos;
                fns.push(self.fn_def()?);
            }
        }
        Ok(Program { fns, structs })
    }

    fn fn_def(&mut self) -> Result<FnDef, String> {
        let vis = match self.peek() {
            Tok::Public => {
                self.bump();
                Vis::Public
            }
            Tok::Private => {
                self.bump();
                Vis::Private
            }
            Tok::Export => {
                self.bump();
                Vis::Export
            }
            _ => Vis::Private,
        };
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
        self.eat(&Tok::Arrow)?;
        let ret = self.ty()?;
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
            Tok::Parallel => {
                self.bump();
                self.eat(&Tok::For)?;
                let var = self.ident()?;
                self.eat(&Tok::In)?;
                let start = self.expr()?;
                self.eat(&Tok::DotDot)?;
                let end = self.expr()?;
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
                if *self.peek() == Tok::Eq {
                    self.bump();
                    let rhs = self.expr()?;
                    self.eat(&Tok::Semi)?;
                    match strip_located(&e) {
                        Expr::Var(name) => Ok(Stmt::Assign(name.clone(), rhs)),
                        Expr::Field(_, _) => Ok(Stmt::FieldAssign(e, rhs)),
                        _ => Err("invalid assign target".into()),
                    }
                } else {
                    self.eat(&Tok::Semi)?;
                    Ok(Stmt::Expr(e))
                }
            }
        }
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
        self.expr()
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.logical_or()
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
        let mut left = self.cmp()?;
        while *self.peek() == Tok::AndAnd {
            self.bump();
            let right = self.cmp()?;
            left = combine_expr(left, BinOp::And, right);
        }
        Ok(left)
    }

    fn cmp(&mut self) -> Result<Expr, String> {
        let mut l = self.add()?;
        loop {
            let op = match self.peek() {
                Tok::EqEq => BinOp::Eq,
                Tok::Lt => BinOp::Lt,
                Tok::Gt => BinOp::Gt,
                Tok::Le => BinOp::Le,
                Tok::Ge => BinOp::Ge,
                _ => break,
            };
            self.bump();
            let r = self.add()?;
            l = combine_expr(l, op, r);
        }
        Ok(l)
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
                if *self.peek() == Tok::LParen {
                    self.bump();
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
                } else if *self.peek() == Tok::LBrace {
                    self.bump();
                    let mut fields = Vec::new();
                    if *self.peek() != Tok::RBrace {
                        loop {
                            let field = self.ident()?;
                            self.eat(&Tok::Colon)?;
                            let value = self.expr()?;
                            fields.push((field, value));
                            if *self.peek() == Tok::Comma {
                                self.bump();
                            } else {
                                break;
                            }
                        }
                    }
                    self.eat(&Tok::RBrace)?;
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
