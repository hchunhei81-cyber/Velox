use crate::ast::*;
use crate::lexer::Span;
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

#[derive(Clone)]
struct FnInfo {
    params: Vec<Ty>,
    ret: Ty,
    vis: Vis,
    origin: String,
}

#[derive(Clone)]
struct VarInfo {
    ty: Ty,
    mutable: bool,
}

#[derive(Clone)]
struct StructInfo {
    fields: HashMap<String, Ty>,
    vis: Vis,
    origin: String,
}

fn validate_struct_cycles(structs: &[StructDef]) -> Result<(), String> {
    let graph: HashMap<String, Vec<String>> = structs
        .iter()
        .map(|st| {
            let deps = st
                .fields
                .iter()
                .filter_map(|(_, ty)| {
                    if let Ty::Struct(name) = ty {
                        Some(name.clone())
                    } else {
                        None
                    }
                })
                .collect();
            (st.name.clone(), deps)
        })
        .collect();

    fn visit(
        name: &str,
        graph: &HashMap<String, Vec<String>>,
        state: &mut HashMap<String, u8>,
    ) -> Result<(), String> {
        match state.get(name).copied().unwrap_or(0) {
            1 => {
                return Err(format!(
                    "E0308: struct `{}` has a recursive value type and cannot contain itself directly or indirectly",
                    name
                ))
            }
            2 => return Ok(()),
            _ => {}
        }

        state.insert(name.to_string(), 1);

        if let Some(deps) = graph.get(name) {
            for dep in deps {
                visit(dep, graph, state)?;
            }
        }

        state.insert(name.to_string(), 2);
        Ok(())
    }

    let mut state = HashMap::new();
    for name in graph.keys() {
        visit(name, &graph, &mut state)?;
    }
    Ok(())
}

pub struct Checker {
    fns: HashMap<String, FnInfo>,
    structs: HashMap<String, StructInfo>,
    python_imports: HashSet<String>,
    error_span: Cell<Option<Span>>,
}

fn strip_located(mut expr: &Expr) -> &Expr {
    while let Expr::Located(inner, _) = expr {
        expr = inner;
    }
    expr
}

fn expr_span(expr: &Expr) -> Option<Span> {
    match expr {
        Expr::Located(_, span) => Some(*span),
        _ => None,
    }
}

fn field_root_var(expr: &Expr) -> Option<&str> {
    match strip_located(expr) {
        Expr::Var(name) => Some(name),
        Expr::Field(base, _) => field_root_var(base),
        _ => None,
    }
}

impl Checker {
    pub fn new() -> Self {
        let mut fns = HashMap::new();
        fns.insert(
            "print".into(),
            FnInfo {
                params: vec![],
                ret: Ty::Void,
                vis: Vis::Public,
                origin: "<builtin>".into(),
            },
        );
        fns.insert(
            "println".into(),
            FnInfo {
                params: vec![Ty::Str],
                ret: Ty::Void,
                vis: Vis::Public,
                origin: "<builtin>".into(),
            },
        );
        for (name, params, ret) in [
            ("read_file", vec![Ty::Str], Ty::Str),
            ("write_file", vec![Ty::Str, Ty::Str], Ty::Bool),
            ("str_len", vec![Ty::Str], Ty::I64),
            ("str_byte_at", vec![Ty::Str, Ty::I64], Ty::I64),
            ("str_byte_at_fast", vec![Ty::Str, Ty::I64, Ty::I64], Ty::I64),
            ("str_slice", vec![Ty::Str, Ty::I64, Ty::I64], Ty::Str),
            ("str_concat", vec![Ty::Str, Ty::Str], Ty::Str),
            ("str_eq", vec![Ty::Str, Ty::Str], Ty::Bool),
            ("list_i64_new", vec![], Ty::I64),
            ("list_i64_push", vec![Ty::I64, Ty::I64], Ty::Void),
            ("list_i64_get", vec![Ty::I64, Ty::I64], Ty::I64),
            ("list_i64_set", vec![Ty::I64, Ty::I64, Ty::I64], Ty::Void),
            ("list_i64_len", vec![Ty::I64], Ty::I64),
        ] {
            fns.insert(
                name.into(),
                FnInfo {
                    params,
                    ret,
                    vis: Vis::Public,
                    origin: "<builtin>".into(),
                },
            );
        }

        Self {
            fns,
            structs: HashMap::new(),
            python_imports: HashSet::new(),
            error_span: Cell::new(None),
        }
    }

    pub fn last_error_span(&self) -> Option<Span> {
        self.error_span.get()
    }

    fn mark_error_span(&self, span: Option<Span>) {
        if self.error_span.get().is_none() {
            self.error_span.set(span);
        }
    }

    pub fn register_extern(&mut self, name: &str, params: Vec<Ty>, ret: Ty) -> Result<(), String> {
        if self.fns.contains_key(name) {
            return Err(format!("E0102: duplicate extern function `{}`", name));
        }
        self.fns.insert(
            name.into(),
            FnInfo {
                params,
                ret,
                vis: Vis::Public,
                origin: "<extern>".into(),
            },
        );
        Ok(())
    }

    pub fn check(&mut self, p: &Program) -> Result<(), String> {
        self.error_span.set(None);
        self.python_imports.clear();
        for module in &p.python_imports {
            if module != "math" {
                return Err(format!(
                    "E0500: unsupported Python module `{}`; currently supported module is `math`",
                    module
                ));
            }
            self.python_imports.insert(module.clone());
        }
        for st in &p.structs {
            if self.structs.contains_key(&st.name) {
                return Err(format!("E0105: duplicate struct `{}`", st.name));
            }
            let mut fields = HashMap::new();
            for (name, ty) in &st.fields {
                if *ty == Ty::Void {
                    return Err(format!(
                        "E0308: struct field `{}` cannot have type void",
                        name
                    ));
                }
                if fields.insert(name.clone(), ty.clone()).is_some() {
                    return Err(format!("E0106: duplicate struct field `{}`", name));
                }
            }
            self.structs.insert(
                st.name.clone(),
                StructInfo {
                    fields,
                    vis: st.vis,
                    origin: st.origin.clone(),
                },
            );
        }
        validate_struct_cycles(&p.structs)?;

        for st in &p.structs {
            for (_, ty) in &st.fields {
                self.validate_type(ty, &st.origin, false)?;
            }
        }
        for f in &p.fns {
            for (_, ty) in &f.params {
                self.validate_type(ty, &f.origin, false)?;
            }
            self.validate_type(&f.ret, &f.origin, true)?;
        }
        for f in &p.fns {
            if f.is_extern {
                continue;
            }
            if let Some(prev) = self.fns.get(&f.name) {
                return Err(format!(
                    "E0102: function `{}` is defined more than once in `{}` and `{}`",
                    f.name, prev.origin, f.origin
                ));
            }
            if f.name == "main" && (!f.params.is_empty() || f.ret != Ty::Void) {
                return Err("E0404: `main` must have the signature `fn main() -> void`".into());
            }
            self.fns.insert(
                f.name.clone(),
                FnInfo {
                    params: f.params.iter().map(|(_, t)| t.clone()).collect(),
                    ret: f.ret.clone(),
                    vis: f.vis,
                    origin: f.origin.clone(),
                },
            );
        }

        for f in &p.fns {
            if f.is_extern {
                continue;
            }
            let mut env = HashMap::new();
            for (name, ty) in &f.params {
                if *ty == Ty::Void {
                    return Err(format!("E0308: parameter `{}` cannot have type void", name));
                }
                if env.contains_key(name) {
                    return Err(format!("E0103: duplicate parameter `{}`", name));
                }
                env.insert(
                    name.clone(),
                    VarInfo {
                        ty: ty.clone(),
                        mutable: false,
                    },
                );
            }
            let returns = self.check_block(&f.body, &mut env, &f.ret, &f.origin, false)?;
            if f.ret != Ty::Void && !returns {
                return Err(format!(
                    "E0400: function `{}` does not return a value on every reachable path",
                    f.name
                ));
            }
        }
        Ok(())
    }

    fn validate_type(&self, ty: &Ty, origin: &str, allow_void: bool) -> Result<(), String> {
        match ty {
            Ty::Void if !allow_void => {
                Err("E0308: void is not allowed in this type position".into())
            }
            Ty::Struct(name) => {
                let info = self
                    .structs
                    .get(name)
                    .ok_or_else(|| format!("E0308: unknown struct type `{}`", name))?;
                if info.vis == Vis::Private && info.origin != origin {
                    return Err(format!("E0301: struct `{}` is private", name));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn check_block(
        &self,
        b: &[Stmt],
        env: &mut HashMap<String, VarInfo>,
        ret: &Ty,
        origin: &str,
        parallel: bool,
    ) -> Result<bool, String> {
        let mut returns = false;
        for stmt in b {
            let r = self.check_stmt(stmt, env, ret, origin, parallel)?;
            if !returns && r {
                returns = true;
            }
        }
        Ok(returns)
    }

    fn check_stmt(
        &self,
        s: &Stmt,
        env: &mut HashMap<String, VarInfo>,
        ret: &Ty,
        origin: &str,
        parallel: bool,
    ) -> Result<bool, String> {
        match s {
            Stmt::Let {
                name,
                mut_,
                ty,
                init,
            } => {
                if env.contains_key(name) {
                    return Err(format!(
                        "E0104: variable `{}` is declared more than once in this scope",
                        name
                    ));
                }
                let it = self.check_expr(init, env, origin)?;
                let t = ty.clone().unwrap_or(it.clone());
                if t == Ty::Void {
                    return Err(format!(
                        "E0308: `{}` cannot be declared with type void",
                        name
                    ));
                }
                if t != it {
                    self.mark_error_span(expr_span(init));
                    return Err(format!(
                        "E0308: `{}` is declared as {:?}, but its initializer has type {:?}",
                        name, t, it
                    ));
                }
                env.insert(
                    name.clone(),
                    VarInfo {
                        ty: t,
                        mutable: *mut_,
                    },
                );
                Ok(false)
            }
            Stmt::Assign(n, e) => {
                let v = env
                    .get(n)
                    .cloned()
                    .ok_or_else(|| format!("E0100: undefined variable `{}`", n))?;
                if !v.mutable {
                    return Err(format!(
                        "E0500: cannot modify immutable variable `{}`，use `let mut` instead",
                        n
                    ));
                }
                let t = self.check_expr(e, env, origin)?;
                if v.ty != t {
                    self.mark_error_span(expr_span(e));
                    return Err(format!(
                        "E0308: `{}` must have type {:?}, but the expression has type {:?}",
                        n, v.ty, t
                    ));
                }
                Ok(false)
            }
            Stmt::FieldAssign(target, value) => {
                let root = match field_root_var(target) {
                    Some(root) => root,
                    None => {
                        self.mark_error_span(expr_span(target));
                        return Err(
                            "E0500: field assignment target must be rooted in a local variable"
                                .into(),
                        );
                    }
                };

                let var = match env.get(root) {
                    Some(var) => var,
                    None => {
                        self.mark_error_span(expr_span(target));
                        return Err(format!("E0100: undefined variable `{}`", root));
                    }
                };

                if !var.mutable {
                    self.mark_error_span(expr_span(target));
                    return Err(format!(
                        "E0500: cannot modify a field of immutable variable `{}`; declare it with `let mut`",
                        root
                    ));
                }

                if !matches!(strip_located(target), Expr::Field(_, _)) {
                    self.mark_error_span(expr_span(target));
                    return Err("E0500: invalid field assignment target".into());
                }

                let expected = self.check_expr(target, env, origin)?;
                let actual = self.check_expr(value, env, origin)?;

                if expected != actual {
                    self.mark_error_span(expr_span(value));
                    return Err(format!(
                        "E0308: field assignment type mismatch: expected {:?}, found {:?}",
                        expected, actual
                    ));
                }
                Ok(false)
            }
            Stmt::Return(e) => {
                if parallel {
                    if let Some(expr) = e {
                        self.mark_error_span(expr_span(expr));
                    }
                    return Err("E0403: `return` is not allowed inside `parallel for`".into());
                }
                match e {
                    Some(expr) => {
                        let t = self.check_expr(expr, env, origin)?;
                        if &t != ret {
                            self.mark_error_span(expr_span(expr));
                            return Err(format!(
                                "E0309: return type is {:?}, but {:?} was expected",
                                t, ret
                            ));
                        }
                    }
                    None if *ret != Ty::Void => {
                        return Err("E0309: missing return value".into());
                    }
                    None => {}
                }
                Ok(true)
            }
            Stmt::If(c, then_body, else_body) => {
                if self.check_expr(c, env, origin)? != Ty::Bool {
                    self.mark_error_span(expr_span(c));
                    return Err("E0401: `if` condition must have type `bool`".into());
                }
                let mut a = env.clone();
                let mut b = env.clone();
                let ar = self.check_block(then_body, &mut a, ret, origin, parallel)?;
                let br = self.check_block(else_body, &mut b, ret, origin, parallel)?;
                Ok(ar && br)
            }
            Stmt::While(c, body) => {
                if self.check_expr(c, env, origin)? != Ty::Bool {
                    self.mark_error_span(expr_span(c));
                    return Err("E0401: `while` condition must have type `bool`".into());
                }
                let mut inner = env.clone();
                self.check_block(body, &mut inner, ret, origin, parallel)?;
                Ok(false)
            }
            Stmt::Expr(e) => {
                self.check_expr(e, env, origin)?;
                Ok(false)
            }
            Stmt::For {
                var,
                start,
                end,
                body,
            } => {
                if env.contains_key(var) {
                    return Err(format!(
                        "E0104: loop variable `{}` is declared more than once in this scope",
                        var
                    ));
                }
                let start_ty = self.check_expr(start, env, origin)?;
                if start_ty != Ty::I64 {
                    self.mark_error_span(expr_span(start));
                    return Err("E0401: `for` bounds must have type `i64`".into());
                }
                if self.check_expr(end, env, origin)? != Ty::I64 {
                    self.mark_error_span(expr_span(end));
                    return Err("E0401: `for` bounds must have type `i64`".into());
                }
                let mut inner = env.clone();
                inner.insert(
                    var.clone(),
                    VarInfo {
                        ty: Ty::I64,
                        mutable: false,
                    },
                );
                self.check_block(body, &mut inner, ret, origin, parallel)?;
                Ok(false)
            }
            Stmt::ParallelFor {
                var,
                start,
                end,
                body,
            } => {
                if parallel {
                    return Err("E0402: nested `parallel for` loops are not supported".into());
                }
                if env.contains_key(var) {
                    return Err(format!(
                        "E0104: loop variable `{}` is declared more than once in this scope",
                        var
                    ));
                }
                let start_ty = self.check_expr(start, env, origin)?;
                if start_ty != Ty::I64 {
                    self.mark_error_span(expr_span(start));
                    return Err("E0401: `parallel for` bounds must have type `i64`".into());
                }
                if self.check_expr(end, env, origin)? != Ty::I64 {
                    self.mark_error_span(expr_span(end));
                    return Err("E0401: `parallel for` bounds must have type `i64`".into());
                }
                let mut inner = env.clone();
                inner.insert(
                    var.clone(),
                    VarInfo {
                        ty: Ty::I64,
                        mutable: false,
                    },
                );
                self.check_block(body, &mut inner, ret, origin, true)?;
                Ok(false)
            }
        }
    }

    fn check_expr(
        &self,
        e: &Expr,
        env: &HashMap<String, VarInfo>,
        origin: &str,
    ) -> Result<Ty, String> {
        let result = self.check_expr_inner(e, env, origin);
        if result.is_err() {
            self.mark_error_span(expr_span(e));
        }
        result
    }

    fn check_expr_inner(
        &self,
        e: &Expr,
        env: &HashMap<String, VarInfo>,
        origin: &str,
    ) -> Result<Ty, String> {
        match e {
            Expr::Located(inner, _) => self.check_expr(inner, env, origin),
            Expr::Int(_) => Ok(Ty::I64),
            Expr::Float(_) => Ok(Ty::F64),
            Expr::Bool(_) => Ok(Ty::Bool),
            Expr::Str(_) => Ok(Ty::Str),
            Expr::StructInit(name, values) => {
                let info = self
                    .structs
                    .get(name)
                    .ok_or_else(|| format!("E0308: unknown struct `{}`", name))?;
                if info.vis == Vis::Private && info.origin != origin {
                    return Err(format!("E0301: struct `{}` is private", name));
                }
                let mut seen = HashMap::new();
                for (field, expr) in values {
                    let expected = match info.fields.get(field) {
                        Some(expected) => expected,
                        None => {
                            self.mark_error_span(expr_span(expr));
                            return Err(format!(
                                "E0308: struct `{}` has no field `{}`",
                                name, field
                            ));
                        }
                    };
                    let actual = self.check_expr(expr, env, origin)?;
                    if &actual != expected {
                        self.mark_error_span(expr_span(expr));
                        return Err(format!(
                            "E0308: type mismatch for field `{}.{}`",
                            name, field
                        ));
                    }
                    if seen.insert(field, ()).is_some() {
                        self.mark_error_span(expr_span(expr));
                        return Err(format!(
                            "E0308: field `{}` is initialized more than once",
                            field
                        ));
                    }
                }
                if seen.len() != info.fields.len() {
                    return Err(format!(
                        "E0308: struct `{}` initialization is missing required fields",
                        name
                    ));
                }
                Ok(Ty::Struct(name.clone()))
            }
            Expr::Field(base, field) => {
                let bt = self.check_expr(base, env, origin)?;
                if let Ty::Struct(name) = bt {
                    let info = self
                        .structs
                        .get(&name)
                        .ok_or_else(|| format!("E0308: unknown struct `{}`", name))?;
                    if info.vis == Vis::Private && info.origin != origin {
                        return Err(format!("E0301: struct `{}` is private", name));
                    }
                    info.fields
                        .get(field)
                        .cloned()
                        .ok_or_else(|| format!("E0308: struct `{}` has no field `{}`", name, field))
                } else {
                    Err("E0308: field access is only valid on struct values".into())
                }
            }
            Expr::Var(n) => env
                .get(n)
                .map(|v| v.ty.clone())
                .ok_or_else(|| format!("E0100: undefined variable `{}`", n)),
            Expr::Bin(l, op, r) => {
                let a = self.check_expr(l, env, origin)?;
                let b = self.check_expr(r, env, origin)?;

                let invalid_left = match op {
                    BinOp::And | BinOp::Or => a != Ty::Bool,
                    BinOp::Eq | BinOp::Ne => false,
                    BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod => !matches!(a, Ty::I64 | Ty::F64),
                };
                let invalid_right = match op {
                    BinOp::And | BinOp::Or => b != Ty::Bool,
                    BinOp::Eq | BinOp::Ne => a != b,
                    BinOp::Lt
                    | BinOp::Gt
                    | BinOp::Le
                    | BinOp::Ge
                    | BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod => !matches!(b, Ty::I64 | Ty::F64) || a != b,
                };
                if invalid_right {
                    self.mark_error_span(expr_span(r));
                } else if invalid_left {
                    self.mark_error_span(expr_span(l));
                }

                match op {
                    BinOp::And | BinOp::Or => {
                        if a == Ty::Bool && b == Ty::Bool {
                            Ok(Ty::Bool)
                        } else {
                            Err("E0308: `&&` and `||` operands must both have type `bool`".into())
                        }
                    }
                    BinOp::Eq | BinOp::Ne => {
                        if a == b && matches!(a, Ty::I64 | Ty::F64 | Ty::Bool) {
                            Ok(Ty::Bool)
                        } else {
                            Err("E0308: both operands of equality comparison must have the same comparable type".into())
                        }
                    }
                    BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => match (a, b) {
                        (Ty::I64, Ty::I64) | (Ty::F64, Ty::F64) => Ok(Ty::Bool),
                        (Ty::I64, Ty::F64) | (Ty::F64, Ty::I64) => {
                            Err("E0308: mixed `i64` and `f64` operations require an explicit conversion".into())
                        }
                        _ => Err("E0308: comparison operands must have the same numeric type".into()),
                    },
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        match (a, b) {
                            (Ty::I64, Ty::I64) => Ok(Ty::I64),
                            (Ty::F64, Ty::F64) if !matches!(op, BinOp::Mod) => Ok(Ty::F64),
                            (Ty::F64, Ty::F64) => Err("E0308: `%` is only supported for `i64` operands".into()),
                            (Ty::I64, Ty::F64) | (Ty::F64, Ty::I64) => {
                                Err("E0308: mixed `i64` and `f64` operations require an explicit conversion".into())
                            }
                            _ => Err("E0308: arithmetic operators require numeric operands".into()),
                        }
                    }
                }
            }
            Expr::Neg(inner) => {
                let t = self.check_expr(inner, env, origin)?;
                if matches!(t, Ty::I64 | Ty::F64) {
                    Ok(t)
                } else {
                    self.mark_error_span(expr_span(inner));
                    Err("E0308: unary `-` requires a numeric operand".into())
                }
            }
            Expr::Not(inner) => {
                let t = self.check_expr(inner, env, origin)?;
                if t == Ty::Bool {
                    Ok(Ty::Bool)
                } else {
                    self.mark_error_span(expr_span(inner));
                    Err("E0308: the `!` operator requires a `bool` operand".into())
                }
            }
            Expr::PythonCall {
                module,
                function,
                args,
            } => {
                if !self.python_imports.contains(module) {
                    return Err(format!(
                        "E0501: Python module `{}` must be imported with `import python {};`",
                        module, module
                    ));
                }
                if module != "math" {
                    return Err(format!("E0500: unsupported Python module `{}`", module));
                }
                let arity = match function.as_str() {
                    "sqrt" | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "exp" | "log"
                    | "log10" | "floor" | "ceil" | "fabs" => 1,
                    "pow" | "atan2" => 2,
                    _ => {
                        return Err(format!(
                            "E0502: unsupported function `{}.{}()`",
                            module, function
                        ))
                    }
                };
                if args.len() != arity {
                    return Err(format!(
                        "E0308: `{}.{}()` expects {} argument(s), but {} were provided",
                        module,
                        function,
                        arity,
                        args.len()
                    ));
                }
                for arg in args {
                    let ty = self.check_expr(arg, env, origin)?;
                    if !matches!(ty, Ty::I64 | Ty::F64) {
                        self.mark_error_span(expr_span(arg));
                        return Err(format!(
                            "E0308: `{}.{}()` arguments must be numeric, found {:?}",
                            module, function, ty
                        ));
                    }
                }
                Ok(Ty::F64)
            }
            Expr::Call(n, args) => {
                if n == "print" || n == "println" || n == "write" {
                    if args.len() != 1 {
                        return Err(format!("E0308: `{}` expects exactly one argument", n));
                    }
                    let t = self.check_expr(&args[0], env, origin)?;
                    if !matches!(t, Ty::I64 | Ty::F64 | Ty::Bool | Ty::Str) {
                        self.mark_error_span(expr_span(&args[0]));
                        return Err(format!(
                            "E0308: `{}` does not support values of type {:?}",
                            n, t
                        ));
                    }
                    return Ok(Ty::Void);
                }
                let info = self
                    .fns
                    .get(n)
                    .cloned()
                    .ok_or_else(|| format!("E0101: undefined function `{}`", n))?;
                if info.vis == Vis::Private
                    && info.origin != origin
                    && info.origin != "<builtin>"
                    && info.origin != "<extern>"
                {
                    return Err(format!(
                        "E0301: `{}` is private to `{}` and cannot be called from `{}`",
                        n, info.origin, origin
                    ));
                }
                if info.params.len() != args.len() {
                    return Err(format!(
                        "E0308: `{}` expects {} arguments, but {} were provided",
                        n,
                        info.params.len(),
                        args.len()
                    ));
                }
                for (arg, param) in args.iter().zip(&info.params) {
                    let t = self.check_expr(arg, env, origin)?;
                    if &t != param {
                        self.mark_error_span(expr_span(arg));
                        return Err(format!(
                            "E0308: argument to `{}` has type {:?}, but {:?} was expected",
                            n, t, param
                        ));
                    }
                }
                Ok(info.ret)
            }
        }
    }
}

impl Default for Checker {
    fn default() -> Self {
        Self::new()
    }
}
