use crate::ast::*;
use std::collections::HashMap;

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

pub struct Checker {
    fns: HashMap<String, FnInfo>,
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
        Self { fns }
    }

    pub fn register_extern(&mut self, name: &str, params: Vec<Ty>, ret: Ty) {
        self.fns.insert(
            name.into(),
            FnInfo {
                params,
                ret,
                vis: Vis::Public,
                origin: "<extern>".into(),
            },
        );
    }

    pub fn check(&mut self, p: &Program) -> Result<(), String> {
        for f in &p.fns {
            if f.is_extern {
                continue;
            }
            if let Some(prev) = self.fns.get(&f.name) {
                if prev.origin != "<builtin>" && prev.origin != "<extern>" {
                    return Err(format!(
                        "E0102: fn `{}` 在 `{}` 和 `{}` 中重名",
                        f.name, prev.origin, f.origin
                    ));
                }
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
                    return Err(format!("E0308: 参数 `{}` 不能是 void", name));
                }
                if env.contains_key(name) {
                    return Err(format!("E0103: 重复参数 `{}`", name));
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
                    "E0400: fn `{}` 并非所有执行路径都有 return",
                    f.name
                ));
            }
        }
        Ok(())
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
                let it = self.check_expr(init, env, origin)?;
                let t = ty.clone().unwrap_or(it.clone());
                if t == Ty::Void {
                    return Err(format!("E0308: `{}` 不能声明为 void", name));
                }
                if t != it {
                    return Err(format!("E0308: `{}` 声明 {:?}，实际 {:?}", name, t, it));
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
                    .ok_or_else(|| format!("E0100: 未定义变量 `{}`", n))?;
                if !v.mutable {
                    return Err(format!("E0500: 不能修改不可变变量 `{}`，请使用 let mut", n));
                }
                let t = self.check_expr(e, env, origin)?;
                if v.ty != t {
                    return Err(format!("E0308: `{}` 应为 {:?}，实际 {:?}", n, v.ty, t));
                }
                Ok(false)
            }
            Stmt::Return(e) => {
                if parallel {
                    return Err("E0403: 不允许在 parallel for 内使用 return".into());
                }
                match e {
                    Some(expr) => {
                        let t = self.check_expr(expr, env, origin)?;
                        if &t != ret {
                            return Err(format!("E0309: return {:?}，期望 {:?}", t, ret));
                        }
                    }
                    None if *ret != Ty::Void => {
                        return Err("E0309: 缺少返回值".into());
                    }
                    None => {}
                }
                Ok(true)
            }
            Stmt::If(c, then_body, else_body) => {
                if self.check_expr(c, env, origin)? != Ty::I64 {
                    return Err("E0401: if 条件必须 i64".into());
                }
                let mut a = env.clone();
                let mut b = env.clone();
                let ar = self.check_block(then_body, &mut a, ret, origin, parallel)?;
                let br = self.check_block(else_body, &mut b, ret, origin, parallel)?;
                Ok(ar && br)
            }
            Stmt::While(c, body) => {
                if self.check_expr(c, env, origin)? != Ty::I64 {
                    return Err("E0401: while 条件必须 i64".into());
                }
                let mut inner = env.clone();
                self.check_block(body, &mut inner, ret, origin, parallel)?;
                Ok(false)
            }
            Stmt::Expr(e) => {
                self.check_expr(e, env, origin)?;
                Ok(false)
            }
            Stmt::ParallelFor {
                var,
                start,
                end,
                body,
            } => {
                if parallel {
                    return Err("E0402: 不支持嵌套 parallel for".into());
                }
                if self.check_expr(start, env, origin)? != Ty::I64
                    || self.check_expr(end, env, origin)? != Ty::I64
                {
                    return Err("E0401: parallel for 范围必须 i64".into());
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
        match e {
            Expr::Int(_) => Ok(Ty::I64),
            Expr::Float(_) => Ok(Ty::F64),
            Expr::Str(_) => Ok(Ty::Str),
            Expr::Var(n) => env
                .get(n)
                .map(|v| v.ty.clone())
                .ok_or_else(|| format!("E0100: 未定义变量 `{}`", n)),
            Expr::Bin(l, op, r) => {
                let a = self.check_expr(l, env, origin)?;
                let b = self.check_expr(r, env, origin)?;
                match (a, b) {
                    (Ty::I64, Ty::I64) => Ok(Ty::I64),
                    (Ty::F64, Ty::F64) => match op {
                        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div => Ok(Ty::F64),
                        BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => Ok(Ty::I64),
                        BinOp::Mod => Err("E0308: % 只支持 i64".into()),
                    },
                    (Ty::I64, Ty::F64) | (Ty::F64, Ty::I64) => {
                        Err("E0308: 混合 i64/f64 需显式转换".into())
                    }
                    _ => Err("E0308: 运算符要求 i64 或 f64".into()),
                }
            }
            Expr::Call(n, args) => {
                if n == "print" {
                    if args.len() != 1 {
                        return Err("E0308: print 需要 1 个参数".into());
                    }
                    let t = self.check_expr(&args[0], env, origin)?;
                    if t != Ty::I64 && t != Ty::F64 {
                        return Err(format!("E0308: print 不支持 {:?}", t));
                    }
                    return Ok(Ty::Void);
                }
                let info = self
                    .fns
                    .get(n)
                    .cloned()
                    .ok_or_else(|| format!("E0101: 未定义函数 `{}`", n))?;
                if info.vis == Vis::Private
                    && info.origin != origin
                    && info.origin != "<builtin>"
                    && info.origin != "<extern>"
                {
                    return Err(format!(
                        "E0301: `{}` 是 `{}` 的私有函数，`{}` 无权调用",
                        n, info.origin, origin
                    ));
                }
                if info.params.len() != args.len() {
                    return Err(format!(
                        "E0308: `{}` 期望 {} 参数，给 {}",
                        n,
                        info.params.len(),
                        args.len()
                    ));
                }
                for (arg, param) in args.iter().zip(&info.params) {
                    let t = self.check_expr(arg, env, origin)?;
                    if &t != param {
                        return Err(format!("E0308: `{}` 参数 {:?}，期望 {:?}", n, t, param));
                    }
                }
                Ok(info.ret)
            }
        }
    }
}
