use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub struct CEmitter {
    out: String,
    fn_ret: HashMap<String, Ty>,
    var_ty: HashMap<String, Ty>, // 当前函数局部变量类型（简易追踪）
}

impl CEmitter {
    pub fn new() -> Self {
        Self {
            out: String::new(),
            fn_ret: HashMap::new(),
            var_ty: HashMap::new(),
        }
    }

    pub fn emit(&mut self, p: &Program) -> Result<String, String> {
        self.out
            .push_str("#include <stdio.h>\n#include <stdint.h>\n\n");
        self.out.push_str("typedef const char* velox_str;\n\n");

        // 登记返回类型
        for f in &p.fns {
            self.fn_ret.insert(f.name.clone(), f.ret.clone());
        }

        // 前置声明
        for f in &p.fns {
            if f.name == "main" || f.is_extern {
                continue;
            }
            let params: Vec<String> = f
                .params
                .iter()
                .map(|(n, t)| format!("{} {}", c_ty(t), n))
                .collect();
            self.out.push_str(&format!(
                "{} {}({});\n",
                c_ty(&f.ret),
                f.name,
                params.join(", ")
            ));
        }
        self.out.push('\n');

        // extern 声明（C 头）
        for f in &p.fns {
            if !f.is_extern {
                continue;
            }
            let params: Vec<String> = f.params.iter().map(|(_, t)| c_ty(t).to_string()).collect();
            self.out.push_str(&format!(
                "extern {} {}({});\n",
                c_ty(&f.ret),
                f.name,
                params.join(", ")
            ));
        }
        self.out.push('\n');

        // 函数体
        for f in &p.fns {
            if f.is_extern {
                continue;
            }
            self.var_ty.clear();
            for (pn, pt) in &f.params {
                self.var_ty.insert(pn.clone(), pt.clone());
            }
            if f.name == "main" {
                self.out.push_str("int main() {\n");
                for s in &f.body {
                    self.emit_stmt(s, 1);
                }
                self.out.push_str("  return 0;\n}\n");
            } else {
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|(n, t)| format!("{} {}", c_ty(t), n))
                    .collect();
                self.out.push_str(&format!(
                    "{} {}({}) {{\n",
                    c_ty(&f.ret),
                    f.name,
                    params.join(", ")
                ));
                for s in &f.body {
                    self.emit_stmt(s, 1);
                }
                if f.ret != Ty::Void {
                    self.out.push_str("  return 0;\n");
                }
                self.out.push_str("}\n");
            }
        }
        Ok(self.out.clone())
    }

    fn emit_stmt(&mut self, s: &Stmt, ind: usize) {
        let pad = "  ".repeat(ind);
        match s {
            Stmt::Let { name, ty, init, .. } => {
                let it = self.expr_ty(init);
                let t = ty.clone().unwrap_or(it.clone());
                self.var_ty.insert(name.clone(), t.clone());
                let ct = c_ty(&t);
                let e = self.expr(init);
                self.out
                    .push_str(&format!("{}{} {} = {};\n", pad, ct, name, e));
            }
            Stmt::Assign(n, e) => {
                let e = self.expr(e);
                self.out.push_str(&format!("{}{} = {};\n", pad, n, e));
            }
            Stmt::Return(e) => match e {
                Some(e) => {
                    let ex = self.expr(e);
                    self.out.push_str(&format!("{}return {};\n", pad, ex));
                }
                None => self.out.push_str(&format!("{}return;\n", pad)),
            },
            Stmt::If(c, t, e) => {
                let cx = self.expr(c);
                self.out.push_str(&format!("{}if ({}) {{\n", pad, cx));
                for s in t {
                    self.emit_stmt(s, ind + 1);
                }
                self.out.push_str(&format!("{}}}", pad));
                if !e.is_empty() {
                    self.out.push_str(" else {\n");
                    for s in e {
                        self.emit_stmt(s, ind + 1);
                    }
                    self.out.push_str(&format!("{}}}\n", pad));
                } else {
                    self.out.push('\n');
                }
            }
            Stmt::While(c, b) => {
                let cx = self.expr(c);
                self.out.push_str(&format!("{}while ({}) {{\n", pad, cx));
                for s in b {
                    self.emit_stmt(s, ind + 1);
                }
                self.out.push_str(&format!("{}}}\n", pad));
            }
            Stmt::Expr(e) => {
                let ex = self.expr(e);
                self.out.push_str(&format!("{}{};\n", pad, ex));
            }
            Stmt::ParallelFor {
                var,
                start,
                end,
                body,
            } => {
                self.emit_parallel_for(var, start, end, body, ind);
            }
        }
    }

    fn emit_parallel_for(
        &mut self,
        var: &str,
        start: &Expr,
        end: &Expr,
        body: &[Stmt],
        ind: usize,
    ) {
        let pad = "  ".repeat(ind);
        let free = collect_free_vars(body, var);
        let s = self.expr(start);
        let e = self.expr(end);
        self.var_ty.insert(var.to_string(), Ty::I64);

        self.out
            .push_str(&format!("{}#pragma omp parallel for\n", pad));
        self.out.push_str(&format!(
            "{}for (int64_t {v} = {s}; {v} < {e}; {v}++) {{\n",
            pad,
            v = var,
            s = s,
            e = e
        ));
        for stmt in body {
            self.emit_par_stmt(stmt, ind + 1, &free);
        }
        self.out.push_str(&format!("{}}}\n", pad));
        self.var_ty.remove(var);
    }

    fn emit_par_stmt(&mut self, s: &Stmt, ind: usize, free: &HashSet<String>) {
        match s {
            Stmt::Assign(n, e) if free.contains(n) => {
                let pad = "  ".repeat(ind);
                let ex = self.expr(e);
                self.out.push_str(&format!("{}#pragma omp critical\n", pad));
                self.out
                    .push_str(&format!("{}{{ {} = {}; }}\n", pad, n, ex));
            }
            Stmt::If(c, t, e) => {
                let pad = "  ".repeat(ind);
                let cx = self.expr(c);
                self.out.push_str(&format!("{}if ({}) {{\n", pad, cx));
                for stmt in t {
                    self.emit_par_stmt(stmt, ind + 1, free);
                }
                self.out.push_str(&format!("{}}}", pad));
                if !e.is_empty() {
                    self.out.push_str(" else {\n");
                    for stmt in e {
                        self.emit_par_stmt(stmt, ind + 1, free);
                    }
                    self.out.push_str(&format!("{}}}\n", pad));
                } else {
                    self.out.push('\n');
                }
            }
            Stmt::While(c, b) => {
                let pad = "  ".repeat(ind);
                let cx = self.expr(c);
                self.out.push_str(&format!("{}while ({}) {{\n", pad, cx));
                for stmt in b {
                    self.emit_par_stmt(stmt, ind + 1, free);
                }
                self.out.push_str(&format!("{}}}\n", pad));
            }
            _ => self.emit_stmt(s, ind),
        }
    }

    fn expr_ty(&self, e: &Expr) -> Ty {
        match e {
            Expr::Int(_) => Ty::I64,
            Expr::Float(_) => Ty::F64,
            Expr::Str(_) => Ty::Str,
            Expr::Var(n) => self.var_ty.get(n).cloned().unwrap_or(Ty::I64),
            Expr::Call(n, _) => self.fn_ret.get(n).cloned().unwrap_or(Ty::I64),
            Expr::Bin(_, op, _) => {
                match op {
                    BinOp::Eq | BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge => Ty::I64,
                    _ => Ty::I64, // 简化：算术运算按 i64（f64 混合需显式）
                }
            }
        }
    }

    fn expr(&mut self, e: &Expr) -> String {
        match e {
            Expr::Int(n) => n.to_string(),
            Expr::Float(f) => {
                let s = format!("{}", f);
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    s
                } else {
                    format!("{}.0", s)
                }
            }
            Expr::Str(s) => format!(
                "\"{}\"",
                s.replace('\\', "\\\\")
                    .replace('"', "\\\"")
                    .replace('\n', "\\n")
                    .replace('\t', "\\t")
            ),
            Expr::Var(v) => v.clone(),
            Expr::Bin(l, op, r) => {
                let lo = self.expr(l);
                let ro = self.expr(r);
                let o = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Mod => "%",
                    BinOp::Eq => "==",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::Le => "<=",
                    BinOp::Ge => ">=",
                };
                format!("({} {} {})", lo, o, ro)
            }
            Expr::Call(n, args) => {
                if n == "print" {
                    let a = &args[0];
                    let at = self.expr_ty(a);
                    let s = self.expr(a);
                    return match at {
                        Ty::I64 => format!("printf(\"%lld\\n\", (long long)({}))", s),
                        Ty::F64 => format!("printf(\"%g\\n\", (double)({}))", s),
                        _ => format!("(void)({})", s),
                    };
                }
                if n == "println" {
                    let a = self.expr(&args[0]);
                    return format!("printf(\"%s\\n\", {})", a);
                }
                let a: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                format!("{}({})", n, a.join(", "))
            }
        }
    }
}

fn c_ty(t: &Ty) -> &'static str {
    match t {
        Ty::I64 => "int64_t",
        Ty::F64 => "double",
        Ty::Str => "velox_str",
        Ty::Void => "void",
    }
}

fn collect_free_vars(body: &[Stmt], loop_var: &str) -> HashSet<String> {
    let mut used = HashSet::new();
    let mut local = HashSet::new();
    local.insert(loop_var.to_string());
    walk_collect(body, &mut used, &mut local);
    used.difference(&local).cloned().collect()
}

fn walk_collect(stmts: &[Stmt], used: &mut HashSet<String>, local: &mut HashSet<String>) {
    for s in stmts {
        match s {
            Stmt::Let { name, init, .. } => {
                expr_vars(init, used);
                local.insert(name.clone());
            }
            Stmt::Assign(n, e) => {
                used.insert(n.clone());
                expr_vars(e, used);
            }
            Stmt::Return(e) => {
                if let Some(e) = e {
                    expr_vars(e, used);
                }
            }
            Stmt::If(c, t, e) => {
                expr_vars(c, used);
                walk_collect(t, used, local);
                walk_collect(e, used, local);
            }
            Stmt::While(c, b) => {
                expr_vars(c, used);
                walk_collect(b, used, local);
            }
            Stmt::Expr(e) => expr_vars(e, used),
            Stmt::ParallelFor { .. } => {}
        }
    }
}

fn expr_vars(e: &Expr, used: &mut HashSet<String>) {
    match e {
        Expr::Var(n) => {
            used.insert(n.clone());
        }
        Expr::Bin(l, _, r) => {
            expr_vars(l, used);
            expr_vars(r, used);
        }
        Expr::Call(_, args) => {
            for a in args {
                expr_vars(a, used);
            }
        }
        _ => {}
    }
}
