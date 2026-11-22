use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub struct CEmitter {
    out: String,
    fn_ret: HashMap<String, Ty>,
    var_ty: HashMap<String, Ty>, // Local variable types in the current function
    struct_fields: HashMap<String, HashMap<String, Ty>>,
    error: Option<String>,
}

impl CEmitter {
    pub fn new() -> Self {
        Self {
            out: String::new(),
            fn_ret: HashMap::new(),
            var_ty: HashMap::new(),
            struct_fields: HashMap::new(),
            error: None,
        }
    }

    pub fn emit(&mut self, p: &Program) -> Result<String, String> {
        self.out
            .push_str("#include <stdio.h>\n#include <stdint.h>\n#include <stdbool.h>\n\n");
        self.out.push_str("typedef const char* velox_str;\n\n");
        // Register every struct before generating definitions.
        for st in &p.structs {
            let fields = st
                .fields
                .iter()
                .map(|(n, t)| (n.clone(), t.clone()))
                .collect();
            self.struct_fields.insert(st.name.clone(), fields);
        }

        // Emit dependencies first: C requires complete by-value field types.
        let mut emitted = HashSet::new();
        let mut remaining = p.structs.len();

        while remaining > 0 {
            let mut progress = false;

            for st in &p.structs {
                if emitted.contains(&st.name) {
                    continue;
                }

                let ready = st.fields.iter().all(|(_, ty)| match ty {
                    Ty::Struct(dep) => emitted.contains(dep),
                    _ => true,
                });

                if !ready {
                    continue;
                }

                self.out
                    .push_str(&format!("typedef struct {} {{\n", st.name));
                for (n, t) in &st.fields {
                    self.out.push_str(&format!("  {} {};\n", c_ty(t), n));
                }
                self.out.push_str(&format!("}} {};\n", st.name));

                emitted.insert(st.name.clone());
                remaining -= 1;
                progress = true;
            }

            if !progress {
                return Err(
                    "E0308: Struct definitions contain a recursive value dependency".into(),
                );
            }
        }
        self.out.push('\n');

        // Register return types
        for f in &p.fns {
            self.fn_ret.insert(f.name.clone(), f.ret.clone());
        }

        // Emit forward declarations
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

        // Emit external C declarations
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

        // Emit function bodies
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
                self.out.push_str("}\n");
            }
        }
        if let Some(err) = self.error.take() {
            Err(err)
        } else {
            Ok(self.out.clone())
        }
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
            Stmt::FieldAssign(target, value) => {
                let lhs = self.expr(target);
                let rhs = self.expr(value);
                self.out.push_str(&format!("{}{} = {};\n", pad, lhs, rhs));
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
                let saved = self.var_ty.clone();

                self.out.push_str(&format!("{}if ({}) {{\n", pad, cx));
                for s in t {
                    self.emit_stmt(s, ind + 1);
                }
                self.var_ty = saved.clone();

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
                self.var_ty = saved;
            }
            Stmt::While(c, b) => {
                let cx = self.expr(c);
                let saved = self.var_ty.clone();
                self.out.push_str(&format!("{}while ({}) {{\n", pad, cx));
                for s in b {
                    self.emit_stmt(s, ind + 1);
                }
                self.out.push_str(&format!("{}}}\n", pad));
                self.var_ty = saved;
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
        let reductions = match analyze_parallel_body(body, var, &self.var_ty) {
            Ok(items) => items,
            Err(err) => {
                if self.error.is_none() {
                    self.error = Some(err);
                }
                return;
            }
        };

        let pad = "  ".repeat(ind);
        let s = self.expr(start);
        let e = self.expr(end);
        let saved = self.var_ty.clone();
        self.var_ty.insert(var.to_string(), Ty::I64);

        if reductions.is_empty() {
            self.out
                .push_str(&format!("{}#pragma omp parallel for\n", pad));
        } else {
            self.out.push_str(&format!(
                "{}#pragma omp parallel for reduction(+: {})\n",
                pad,
                reductions.join(", ")
            ));
        }

        self.out.push_str(&format!(
            "{}for (int64_t {v} = {s}; {v} < {e}; {v}++) {{\n",
            pad,
            v = var,
            s = s,
            e = e
        ));

        for stmt in body {
            self.emit_stmt(stmt, ind + 1);
        }

        self.out.push_str(&format!("{}}}\n", pad));
        self.var_ty = saved;
    }

    fn expr_ty(&self, e: &Expr) -> Ty {
        match e {
            Expr::Int(_) => Ty::I64,
            Expr::Float(_) => Ty::F64,
            Expr::Bool(_) => Ty::Bool,
            Expr::Not(_) => Ty::Bool,
            Expr::Neg(inner) => self.expr_ty(inner),
            Expr::Str(_) => Ty::Str,
            Expr::Var(n) => self.var_ty.get(n).cloned().unwrap_or(Ty::I64),
            Expr::Call(n, _) => self.fn_ret.get(n).cloned().unwrap_or(Ty::I64),
            Expr::StructInit(n, _) => Ty::Struct(n.clone()),
            Expr::Field(base, field) => match self.expr_ty(base) {
                Ty::Struct(ref n) => self
                    .struct_fields
                    .get(n)
                    .and_then(|m| m.get(field))
                    .cloned()
                    .unwrap_or(Ty::I64),
                _ => Ty::I64,
            },
            Expr::Bin(l, op, r) => match op {
                BinOp::Eq
                | BinOp::Lt
                | BinOp::Gt
                | BinOp::Le
                | BinOp::Ge
                | BinOp::And
                | BinOp::Or => Ty::Bool,
                _ => {
                    let lt = self.expr_ty(l);
                    let rt = self.expr_ty(r);
                    if lt == Ty::F64 && rt == Ty::F64 {
                        Ty::F64
                    } else {
                        Ty::I64
                    }
                }
            },
        }
    }

    fn expr(&mut self, e: &Expr) -> String {
        match e {
            Expr::Not(inner) => format!("(!{})", self.expr(inner)),
            Expr::Neg(inner) => format!("(-{})", self.expr(inner)),
            Expr::Int(n) => n.to_string(),
            Expr::Bool(v) => {
                if *v {
                    "true".into()
                } else {
                    "false".into()
                }
            }
            Expr::Float(f) => {
                let s = format!("{}", f);
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    s
                } else {
                    format!("{}.0", s)
                }
            }
            Expr::Str(s) => format!("\"{}\"", escape_c_string(s)),
            Expr::Var(v) => v.clone(),
            Expr::Field(base, field) => format!("({}).{}", self.expr(base), field),
            Expr::StructInit(name, fields) => {
                let values: Vec<String> = fields
                    .iter()
                    .map(|(n, e)| format!(".{} = {}", n, self.expr(e)))
                    .collect();
                format!("({}){{ {} }}", name, values.join(", "))
            }
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
                    BinOp::And => "&&",
                    BinOp::Or => "||",
                };
                format!("({} {} {})", lo, o, ro)
            }
            Expr::Call(n, args) => {
                if n == "print" || n == "println" || n == "write" {
                    let Some(a) = args.first() else {
                        return "(void)0".into();
                    };
                    let at = self.expr_ty(a);
                    let value = self.expr(a);
                    let newline = if n == "write" { "" } else { "\\n" };

                    return match at {
                        Ty::I64 => format!("printf(\"%lld{}\", (long long)({}))", newline, value),
                        Ty::F64 => format!("printf(\"%g{}\", (double)({}))", newline, value),
                        Ty::Bool => format!(
                            "printf(\"%s{}\", ({} ? \"true\" : \"false\"))",
                            newline, value
                        ),
                        Ty::Str => format!("printf(\"%s{}\", {})", newline, value),
                        _ => format!("(void)({})", value),
                    };
                }
                let a: Vec<String> = args.iter().map(|a| self.expr(a)).collect();
                format!("{}({})", n, a.join(", "))
            }
        }
    }
}

fn escape_c_string(s: &str) -> String {
    let mut out = String::new();

    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\000"),
            c if c.is_control() => {
                let mut buf = [0u8; 4];
                for byte in c.encode_utf8(&mut buf).as_bytes() {
                    out.push_str(&format!("\\{:03o}", byte));
                }
            }
            c => out.push(c),
        }
    }

    out
}

fn c_ty(t: &Ty) -> String {
    match t {
        Ty::I64 => "int64_t".into(),
        Ty::F64 => "double".into(),
        Ty::Bool => "bool".into(),
        Ty::Str => "velox_str".into(),
        Ty::Void => "void".into(),
        Ty::Struct(name) => name.clone(),
    }
}

#[derive(Default)]
struct ParallelAccess {
    reads: HashMap<String, usize>,
    writes: HashMap<String, usize>,
    reduction_writes: HashMap<String, usize>,
    invalid_writes: HashSet<String>,
}

fn expr_root_var(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Var(name) => Some(name),
        Expr::Field(base, _) => expr_root_var(base),
        _ => None,
    }
}

fn analyze_parallel_body(
    body: &[Stmt],
    loop_var: &str,
    outer_types: &HashMap<String, Ty>,
) -> Result<Vec<String>, String> {
    let outer: HashSet<String> = outer_types.keys().cloned().collect();
    let mut locals = HashSet::new();
    locals.insert(loop_var.to_string());

    let mut access = ParallelAccess::default();
    analyze_parallel_block(body, &outer, &mut locals, &mut access)?;

    let mut reductions = Vec::new();

    for (name, writes) in &access.writes {
        let valid_reduction = !access.invalid_writes.contains(name)
            && access.reduction_writes.get(name).copied().unwrap_or(0) == *writes
            && access.reads.get(name).copied().unwrap_or(0) == *writes;

        if !valid_reduction {
            return Err(format!(
                "E0601: parallel for writes to shared variable `{}`，but the update is not a supported reduction expression; currently supported forms are `{} = {} + expr` or `{} = expr + {}`",
                name, name, name, name, name
            ));
        }

        match outer_types.get(name) {
            Some(Ty::I64) | Some(Ty::F64) => {}
            Some(_) => {
                return Err(format!(
                    "E0602: parallel for reduction variable `{}` must have a numeric type",
                    name
                ));
            }
            None => {
                return Err(format!(
                    "E0603: cannot determine the type of parallel for variable `{}` ",
                    name
                ));
            }
        }

        reductions.push(name.clone());
    }

    reductions.sort();
    Ok(reductions)
}

fn analyze_parallel_block(
    stmts: &[Stmt],
    outer: &HashSet<String>,
    locals: &mut HashSet<String>,
    access: &mut ParallelAccess,
) -> Result<(), String> {
    for stmt in stmts {
        match stmt {
            Stmt::Let { name, init, .. } => {
                count_parallel_reads(init, outer, locals, access);
                locals.insert(name.clone());
            }

            Stmt::Assign(name, expr) => {
                if outer.contains(name) && !locals.contains(name) {
                    *access.writes.entry(name.clone()).or_insert(0) += 1;

                    if is_reduction_add(expr, name) {
                        *access.reduction_writes.entry(name.clone()).or_insert(0) += 1;
                    } else {
                        access.invalid_writes.insert(name.clone());
                    }
                }

                count_parallel_reads(expr, outer, locals, access);
            }

            Stmt::FieldAssign(target, value) => {
                if let Some(root) = expr_root_var(target) {
                    if outer.contains(root) && !locals.contains(root) {
                        return Err(format!(
                            "E0601: parallel for cannot modify shared struct field `{}`",
                            root
                        ));
                    }
                }
                count_parallel_reads(value, outer, locals, access);
            }

            Stmt::If(cond, then_body, else_body) => {
                count_parallel_reads(cond, outer, locals, access);

                let mut then_locals = locals.clone();
                analyze_parallel_block(then_body, outer, &mut then_locals, access)?;

                let mut else_locals = locals.clone();
                analyze_parallel_block(else_body, outer, &mut else_locals, access)?;
            }

            Stmt::While(cond, body) => {
                count_parallel_reads(cond, outer, locals, access);

                let mut nested_locals = locals.clone();
                analyze_parallel_block(body, outer, &mut nested_locals, access)?;
            }

            Stmt::Expr(expr) => {
                count_parallel_reads(expr, outer, locals, access);
            }

            Stmt::Return(_) => {
                return Err("E0604: return is not allowed inside parallel for".to_string());
            }

            Stmt::ParallelFor { .. } => {
                return Err(
                    "E0605: nested parallel for loops are not currently supported".to_string(),
                );
            }
        }
    }

    Ok(())
}

fn count_parallel_reads(
    expr: &Expr,
    outer: &HashSet<String>,
    locals: &HashSet<String>,
    access: &mut ParallelAccess,
) {
    match expr {
        Expr::Var(name) => {
            if outer.contains(name) && !locals.contains(name) {
                *access.reads.entry(name.clone()).or_insert(0) += 1;
            }
        }

        Expr::Not(inner) | Expr::Neg(inner) => count_parallel_reads(inner, outer, locals, access),

        Expr::Bin(left, _, right) => {
            count_parallel_reads(left, outer, locals, access);
            count_parallel_reads(right, outer, locals, access);
        }

        Expr::Call(_, args) => {
            for arg in args {
                count_parallel_reads(arg, outer, locals, access);
            }
        }
        Expr::Field(base, _) => count_parallel_reads(base, outer, locals, access),
        Expr::StructInit(_, fields) => {
            for (_, e) in fields {
                count_parallel_reads(e, outer, locals, access);
            }
        }
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) => {}
    }
}

fn is_reduction_add(expr: &Expr, target: &str) -> bool {
    match expr {
        Expr::Bin(left, BinOp::Add, right) => {
            (matches!(left.as_ref(), Expr::Var(name) if name == target)
                && count_var_occurrences(right, target) == 0)
                || (matches!(right.as_ref(), Expr::Var(name) if name == target)
                    && count_var_occurrences(left, target) == 0)
        }
        _ => false,
    }
}

fn count_var_occurrences(expr: &Expr, target: &str) -> usize {
    match expr {
        Expr::Not(inner) | Expr::Neg(inner) => count_var_occurrences(inner, target),

        Expr::Var(name) => usize::from(name == target),

        Expr::Bin(left, _, right) => {
            count_var_occurrences(left, target) + count_var_occurrences(right, target)
        }

        Expr::Call(_, args) => args
            .iter()
            .map(|arg| count_var_occurrences(arg, target))
            .sum(),
        Expr::Field(base, _) => count_var_occurrences(base, target),
        Expr::StructInit(_, fields) => fields
            .iter()
            .map(|(_, e)| count_var_occurrences(e, target))
            .sum(),
        Expr::Int(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Str(_) => 0,
    }
}
