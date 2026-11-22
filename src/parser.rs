use crate::ast::*;
use crate::lexer::Tok;

pub struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    pub fn new(mut toks: Vec<Tok>) -> Self {
        if toks.last() != Some(&Tok::Eof) {
            toks.push(Tok::Eof);
        }
        Self { toks, pos: 0 }
    }
    fn peek(&self) -> &Tok {
        self.toks.last().map_or(&Tok::Eof, |_| {
            self.toks
                .get(self.pos)
                .unwrap_or_else(|| self.toks.last().unwrap())
        })
    }
    fn bump(&mut self) -> Tok {
        let t = self.peek().clone();
        if t != Tok::Eof {
            self.pos += 1;
        }
        t
    }
    fn eat(&mut self, t: &Tok) -> Result<(), String> {
        if self.peek() == t {
            self.bump();
            Ok(())
        } else {
            Err(format!("expected {:?}, got {:?}", t, self.peek()))
        }
    }
    fn ident(&mut self) -> Result<String, String> {
        match self.bump() {
            Tok::Ident(s) => Ok(s),
            t => Err(format!("expected ident, got {:?}", t)),
        }
    }
    fn ty(&mut self) -> Result<Ty, String> {
        match self.bump() {
            Tok::Ident(s) => match s.as_str() {
                "i64" => Ok(Ty::I64),
                "f64" => Ok(Ty::F64),
                "str" => Ok(Ty::Str),
                "void" => Ok(Ty::Void),
                _ => Err(format!("unknown type `{}`", s)),
            },
            t => Err(format!("expected type, got {:?}", t)),
        }
    }

    pub fn program(&mut self) -> Result<Program, String> {
        let mut fns = Vec::new();
        while *self.peek() != Tok::Eof {
            fns.push(self.fn_def()?);
        }
        Ok(Program { fns })
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
                let cond = self.expr()?;
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
                let cond = self.expr()?;
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
                    if let Expr::Var(name) = e {
                        Ok(Stmt::Assign(name, rhs))
                    } else {
                        Err("invalid assign target".into())
                    }
                } else {
                    self.eat(&Tok::Semi)?;
                    Ok(Stmt::Expr(e))
                }
            }
        }
    }

    fn expr(&mut self) -> Result<Expr, String> {
        self.cmp()
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
            l = Expr::Bin(Box::new(l), op, Box::new(r));
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
            l = Expr::Bin(Box::new(l), op, Box::new(r));
        }
        Ok(l)
    }

    fn mul(&mut self) -> Result<Expr, String> {
        let mut l = self.atom()?;
        loop {
            let op = match self.peek() {
                Tok::Star => BinOp::Mul,
                Tok::Slash => BinOp::Div,
                Tok::Percent => BinOp::Mod,
                _ => break,
            };
            self.bump();
            let r = self.atom()?;
            l = Expr::Bin(Box::new(l), op, Box::new(r));
        }
        Ok(l)
    }

    fn atom(&mut self) -> Result<Expr, String> {
        match self.bump() {
            Tok::Int(n) => Ok(Expr::Int(n)),
            Tok::Float(f) => Ok(Expr::Float(f)),
            Tok::Str(s) => Ok(Expr::Str(s)),
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
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Tok::LParen => {
                let e = self.expr()?;
                self.eat(&Tok::RParen)?;
                Ok(e)
            }
            t => Err(format!("unexpected token {:?}", t)),
        }
    }
}
