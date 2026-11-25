#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    I64,
    F64,
    Bool,
    Str,
    Void,
    Struct(String),
    ArrayI64,
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
    Array(Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Slice(Box<Expr>, Box<Expr>, Box<Expr>),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    Var(String),
    Bin(Box<Expr>, BinOp, Box<Expr>),
    Ternary(Box<Expr>, Box<Expr>, Box<Expr>),
    Match(Box<Expr>, Vec<(Option<i64>, Expr)>),
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
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
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
    IndexAssign(Box<Expr>, Box<Expr>, Expr),
    Return(Option<Expr>),
    Break,
    Continue,
    If(Expr, Vec<Stmt>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Expr(Expr),
    For {
        var: String,
        start: Expr,
        end: Expr,
        body: Vec<Stmt>,
    },
    ForEach {
        var: String,
        iter: Expr,
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
