#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    I64,
    F64,
    Str,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Vis {
    Public,
    Private,
    Export,
}

#[derive(Debug, Clone)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Str(String),
    Var(String),
    Bin(Box<Expr>, BinOp, Box<Expr>),
    Call(String, Vec<Expr>),
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Lt,
    Gt,
    Le,
    Ge,
}

#[derive(Debug, Clone)]
pub enum Stmt {
    Let {
        name: String,
        mut_: bool,
        ty: Option<Ty>,
        init: Expr,
    },
    Assign(String, Expr),
    Return(Option<Expr>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Expr(Expr),
    ParallelFor {
        var: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
}

#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub vis: Vis,
    pub origin: String,
    pub is_extern: bool, // FFI 函数标记
    pub params: Vec<(String, Ty)>,
    pub ret: Ty,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub fns: Vec<FnDef>,
}
