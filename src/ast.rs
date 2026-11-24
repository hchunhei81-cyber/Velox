#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    I64,
    F64,
    Bool,
    Str,
    Void,
    Struct(String),
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
    Bool(bool),
    Str(String),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Var(String),
    Bin(Box<Expr>, BinOp, Box<Expr>),
    Call(String, Vec<Expr>),
    PythonCall {
        module: String,
        function: String,
        args: Vec<Expr>,
    },
    StructInit(String, Vec<(String, Expr)>),
    Field(Box<Expr>, String),
    Located(Box<Expr>, crate::lexer::Span),
}

#[derive(Debug, Clone, Copy)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
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
    FieldAssign(Expr, Expr),
    Return(Option<Expr>),
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Expr(Expr),
    For {
        var: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
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
    pub is_extern: bool, // Marks an FFI function
    pub params: Vec<(String, Ty)>,
    pub ret: Ty,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub vis: Vis,
    pub origin: String,
    pub fields: Vec<(String, Ty)>,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub fns: Vec<FnDef>,
    pub structs: Vec<StructDef>,
    pub python_imports: Vec<String>,
}
