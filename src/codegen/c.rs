use crate::ast::*;
use std::collections::{HashMap, HashSet};

pub struct CEmitter {
    out: String,
    fn_ret: HashMap<String, Ty>,
    var_ty: HashMap<String, Ty>, // Local variable types in the current function
    struct_fields: HashMap<String, HashMap<String, Ty>>,
    error: Option<String>,
    current_fn: String,
    temp_counter: usize,
    extern_fns: HashSet<String>,
}

impl CEmitter {
    pub fn new() -> Self {
        Self {
            out: String::new(),
            fn_ret: HashMap::new(),
            var_ty: HashMap::new(),
            struct_fields: HashMap::new(),
            error: None,
            current_fn: String::new(),
            temp_counter: 0,
            extern_fns: HashSet::new(),
        }
    }

    pub fn emit(&mut self, p: &Program) -> Result<String, String> {
        self.extern_fns = p
            .fns
            .iter()
            .filter(|f| f.is_extern)
            .map(|f| f.name.clone())
            .collect();

        self.out
            .push_str("#include <stdio.h>\n#include <stdint.h>\n#include <stdbool.h>\n#include <stdlib.h>\n\n");
        self.out.push_str("typedef const char* velox_str;\n\n");
        self.out.push_str(
            r#"
#include <string.h>

static void velox_runtime_fail(const char *message) {
    fprintf(stderr, "Velox runtime error: %s\n", message);
    exit(EXIT_FAILURE);
}

static velox_str velox_read_file(velox_str path) {
    FILE *file = fopen(path, "rb");
    if (!file) velox_runtime_fail("cannot open file for reading");
    if (fseek(file, 0, SEEK_END) != 0)
        velox_runtime_fail("cannot seek input file");
    long size = ftell(file);
    if (size < 0) velox_runtime_fail("cannot determine input file size");
    if (fseek(file, 0, SEEK_SET) != 0)
        velox_runtime_fail("cannot rewind input file");

    char *buffer = (char *)malloc((size_t)size + 1);
    if (!buffer) velox_runtime_fail("out of memory reading file");
    size_t count = fread(buffer, 1, (size_t)size, file);
    bool failed = count != (size_t)size || ferror(file);
    fclose(file);
    if (failed) velox_runtime_fail("failed to read input file");
    if (memchr(buffer, '\0', count))
        velox_runtime_fail("input file contains an embedded NUL byte");
    buffer[count] = '\0';
    return buffer;
}

static bool velox_write_file(velox_str path, velox_str content) {
    FILE *file = fopen(path, "wb");
    if (!file) return false;
    size_t length = strlen(content);
    size_t written = fwrite(content, 1, length, file);
    int close_result = fclose(file);
    return written == length && close_result == 0;
}

static int64_t velox_str_len(velox_str value) {
    return (int64_t)strlen(value);
}

static int64_t velox_str_byte_at(velox_str value, int64_t index) {
    size_t length = strlen(value);
    if (index < 0 || (uint64_t)index >= length)
        velox_runtime_fail("string byte index out of bounds");
    return (int64_t)(unsigned char)value[index];
}

static int64_t velox_str_byte_at_fast(velox_str value, int64_t index, int64_t length) {
    if (index < 0 || length < 0 || index >= length)
        velox_runtime_fail("string byte index out of bounds");
    return (int64_t)(unsigned char)value[index];
}

static velox_str velox_str_slice(velox_str value, int64_t start, int64_t end) {
    size_t length = strlen(value);
    if (start < 0 || end < start || (uint64_t)end > length)
        velox_runtime_fail("invalid string slice bounds");
    size_t count = (size_t)(end - start);
    char *result = (char *)malloc(count + 1);
    if (!result) velox_runtime_fail("out of memory slicing string");
    memcpy(result, value + start, count);
    result[count] = '\0';
    return result;
}

static velox_str velox_str_concat(velox_str left, velox_str right) {
    size_t a = strlen(left), b = strlen(right);
    if (a > SIZE_MAX - b - 1)
        velox_runtime_fail("string concatenation size overflow");
    char *result = (char *)malloc(a + b + 1);
    if (!result) velox_runtime_fail("out of memory concatenating strings");
    memcpy(result, left, a);
    memcpy(result + a, right, b + 1);
    return result;
}

static bool velox_str_eq(velox_str left, velox_str right) {
    return strcmp(left, right) == 0;
}

typedef struct {
    int64_t *data;
    size_t len;
    size_t cap;
} velox_i64_list;

static velox_i64_list *velox_get_i64_list(int64_t handle) {
    velox_i64_list *list = (velox_i64_list *)(intptr_t)handle;
    if (!list) velox_runtime_fail("invalid integer list handle");
    return list;
}

static int64_t velox_list_i64_new(void) {
    velox_i64_list *list = (velox_i64_list *)calloc(1, sizeof(*list));
    if (!list) velox_runtime_fail("out of memory creating integer list");
    return (int64_t)(intptr_t)list;
}

static void velox_list_i64_push(int64_t handle, int64_t value) {
    velox_i64_list *list = velox_get_i64_list(handle);
    if (list->len == list->cap) {
        size_t next = list->cap ? list->cap * 2 : 8;
        if (next < list->cap || next > SIZE_MAX / sizeof(int64_t))
            velox_runtime_fail("integer list capacity overflow");
        int64_t *data = (int64_t *)realloc(list->data, next * sizeof(int64_t));
        if (!data) velox_runtime_fail("out of memory growing integer list");
        list->data = data;
        list->cap = next;
    }
    list->data[list->len++] = value;
}

static int64_t velox_list_i64_get(int64_t handle, int64_t index) {
    velox_i64_list *list = velox_get_i64_list(handle);
    if (index < 0 || (uint64_t)index >= list->len)
        velox_runtime_fail("integer list index out of bounds");
    return list->data[index];
}

static void velox_list_i64_set(int64_t handle, int64_t index, int64_t value) {
    velox_i64_list *list = velox_get_i64_list(handle);
    if (index < 0 || (uint64_t)index >= list->len)
        velox_runtime_fail("integer list index out of bounds");
    list->data[index] = value;
}

static int64_t velox_list_i64_len(int64_t handle) {
    velox_i64_list *list = velox_get_i64_list(handle);
    if (list->len > INT64_MAX)
        velox_runtime_fail("integer list length overflow");
    return (int64_t)list->len;
}
"#,
        );
        self.out.push_str(
            r#"
static void velox_i64_fail(const char *operation) {
    fprintf(stderr, "Velox runtime error: %s\n", operation);
    exit(EXIT_FAILURE);
}

static int64_t velox_i64_add(int64_t a, int64_t b) {
    int64_t result;
    if (__builtin_add_overflow(a, b, &result))
        velox_i64_fail("i64 addition overflow");
    return result;
}

static int64_t velox_i64_sub(int64_t a, int64_t b) {
    int64_t result;
    if (__builtin_sub_overflow(a, b, &result))
        velox_i64_fail("i64 subtraction overflow");
    return result;
}

static int64_t velox_i64_mul(int64_t a, int64_t b) {
    int64_t result;
    if (__builtin_mul_overflow(a, b, &result))
        velox_i64_fail("i64 multiplication overflow");
    return result;
}

static int64_t velox_i64_div(int64_t a, int64_t b) {
    if (b == 0) velox_i64_fail("i64 division by zero");
    if (a == INT64_MIN && b == -1)
        velox_i64_fail("i64 division overflow");
    return a / b;
}

static int64_t velox_i64_mod(int64_t a, int64_t b) {
    if (b == 0) velox_i64_fail("i64 modulo by zero");
    if (a == INT64_MIN && b == -1) return 0;
    return a % b;
}

static int64_t velox_i64_neg(int64_t a) {
    if (a == INT64_MIN) velox_i64_fail("i64 negation overflow");
    return -a;
}

"#,
        );
        if !p.python_imports.is_empty() {
            self.out
                .push_str("#define PY_SSIZE_T_CLEAN\n#include <Python.h>\n#include <stdlib.h>\n\n");
            self.out.push_str(
                r#"
static double velox_py_call_f64(const char *module_name,
                                const char *function_name,
                                int argc,
                                double *argv) {
    if (!Py_IsInitialized()) {
        Py_Initialize();
        PyEval_SaveThread();
    }
    PyGILState_STATE gil = PyGILState_Ensure();
    PyObject *module = PyImport_ImportModule(module_name);
    if (!module) {
        PyErr_Print();
        PyGILState_Release(gil);
        exit(EXIT_FAILURE);
    }
    PyObject *function = PyObject_GetAttrString(module, function_name);
    if (!function || !PyCallable_Check(function)) {
        PyErr_Print();
        Py_XDECREF(function);
        Py_DECREF(module);
        PyGILState_Release(gil);
        exit(EXIT_FAILURE);
    }
    PyObject *args = PyTuple_New(argc);
    if (!args) {
        PyErr_Print();
        Py_DECREF(function);
        Py_DECREF(module);
        PyGILState_Release(gil);
        exit(EXIT_FAILURE);
    }
    for (int i = 0; i < argc; ++i) {
        PyObject *value = PyFloat_FromDouble(argv[i]);
        if (!value) {
            PyErr_Print();
            Py_DECREF(args);
            Py_DECREF(function);
            Py_DECREF(module);
            PyGILState_Release(gil);
            exit(EXIT_FAILURE);
        }
        PyTuple_SET_ITEM(args, i, value);
    }
    PyObject *result = PyObject_CallObject(function, args);
    PyObject *number = result ? PyNumber_Float(result) : NULL;
    if (!number) {
        PyErr_Print();
        Py_XDECREF(result);
        Py_DECREF(args);
        Py_DECREF(function);
        Py_DECREF(module);
        PyGILState_Release(gil);
        exit(EXIT_FAILURE);
    }
    double value = PyFloat_AsDouble(number);
    if (PyErr_Occurred()) {
        PyErr_Print();
        Py_DECREF(number);
        Py_DECREF(result);
        Py_DECREF(args);
        Py_DECREF(function);
        Py_DECREF(module);
        PyGILState_Release(gil);
        exit(EXIT_FAILURE);
    }
    Py_DECREF(number);
    Py_DECREF(result);
    Py_DECREF(args);
    Py_DECREF(function);
    Py_DECREF(module);
    PyGILState_Release(gil);
    return value;
}
"#,
            );
        }
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

                let c_name = c_struct_name(&st.name);
                self.out
                    .push_str(&format!("typedef struct {} {{\n", c_name));
                for (n, t) in &st.fields {
                    self.out
                        .push_str(&format!("  {} {};\n", c_ty(t), c_field_name(n)));
                }
                self.out.push_str(&format!("}} {};\n", c_name));

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
        for (name, ret) in [
            ("read_file", Ty::Str),
            ("write_file", Ty::Bool),
            ("str_len", Ty::I64),
            ("str_byte_at", Ty::I64),
            ("str_byte_at_fast", Ty::I64),
            ("str_slice", Ty::Str),
            ("str_concat", Ty::Str),
            ("str_eq", Ty::Bool),
            ("list_i64_new", Ty::I64),
            ("list_i64_push", Ty::Void),
            ("list_i64_get", Ty::I64),
            ("list_i64_set", Ty::Void),
            ("list_i64_len", Ty::I64),
        ] {
            self.fn_ret.insert(name.into(), ret);
        }

        // Emit forward declarations
        for f in &p.fns {
            if f.name == "main" || f.is_extern {
                continue;
            }
            let params: Vec<String> = f
                .params
                .iter()
                .map(|(n, t)| format!("{} {}", c_ty(t), c_var_name(n)))
                .collect();
            self.out.push_str(&format!(
                "{} {}({});\n",
                c_ty(&f.ret),
                self.c_fn_name(&f.name),
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
            self.current_fn = f.name.clone();
            for (pn, pt) in &f.params {
                self.var_ty.insert(pn.clone(), pt.clone());
            }
            if f.name == "main" {
                self.out.push_str("int main() {\n");
                if !p.python_imports.is_empty() {
                    self.out
                        .push_str("  Py_Initialize();\n  PyEval_SaveThread();\n");
                }
                for s in &f.body {
                    self.emit_stmt(s, 1);
                }
                self.out.push_str("  return 0;\n}\n");
            } else {
                let params: Vec<String> = f
                    .params
                    .iter()
                    .map(|(n, t)| format!("{} {}", c_ty(t), c_var_name(n)))
                    .collect();
                self.out.push_str(&format!(
                    "{} {}({}) {{\n",
                    c_ty(&f.ret),
                    self.c_fn_name(&f.name),
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

    fn c_fn_name(&self, name: &str) -> String {
        if name == "main" || self.extern_fns.contains(name) {
            name.to_string()
        } else {
            mangle_c_name("vxf_", name)
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
                    .push_str(&format!("{}{} {} = {};\n", pad, ct, c_var_name(name), e));
            }
            Stmt::Assign(n, e) => {
                let e = self.expr(e);
                self.out
                    .push_str(&format!("{}{} = {};\n", pad, c_var_name(n), e));
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
                None => {
                    if self.current_fn == "main" {
                        self.out.push_str(&format!("{}return 0;\n", pad));
                    } else {
                        self.out.push_str(&format!("{}return;\n", pad));
                    }
                }
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
            Stmt::For {
                var,
                start,
                end,
                body,
            } => self.emit_for(var, start, end, body, ind),
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

    fn fresh_temp(&mut self) -> String {
        loop {
            let name = format!("velox_internal_loop_end_{}", self.temp_counter);
            self.temp_counter += 1;

            if !self.var_ty.contains_key(&name)
                && !self.fn_ret.contains_key(&name)
                && !self.struct_fields.contains_key(&name)
            {
                return name;
            }
        }
    }

    fn emit_for(&mut self, var: &str, start: &Expr, end: &Expr, body: &[Stmt], ind: usize) {
        let pad = "  ".repeat(ind);
        let s = self.expr(start);
        let e = self.expr(end);
        let bound = self.fresh_temp();
        let c_var = c_var_name(var);
        let saved = self.var_ty.clone();

        self.var_ty.insert(var.to_string(), Ty::I64);

        // Isolate the generated temporary in its own C scope.
        self.out.push_str(&format!("{}{{\n", pad));
        self.out
            .push_str(&format!("{}  int64_t {} = {};\n", pad, bound, e));
        self.out.push_str(&format!(
            "{}  for (int64_t {v} = {s}; {v} < {e}; {v}++) {{\n",
            pad,
            v = c_var,
            s = s,
            e = bound
        ));

        for stmt in body {
            self.emit_stmt(stmt, ind + 2);
        }

        self.out.push_str(&format!("{}  }}\n", pad));
        self.out.push_str(&format!("{}}}\n", pad));
        self.var_ty = saved;
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
        let bound = self.fresh_temp();
        let c_var = c_var_name(var);
        let reductions: Vec<String> = reductions.iter().map(|name| c_var_name(name)).collect();
        let saved = self.var_ty.clone();
        self.var_ty.insert(var.to_string(), Ty::I64);

        // Evaluate the range bound once, before entering the OpenMP loop.
        self.out.push_str(&format!("{}{{\n", pad));
        self.out
            .push_str(&format!("{}  int64_t {} = {};\n", pad, bound, e));

        if reductions.is_empty() {
            self.out
                .push_str(&format!("{}  #pragma omp parallel for\n", pad));
        } else {
            self.out.push_str(&format!(
                "{}  #pragma omp parallel for reduction(+: {})\n",
                pad,
                reductions.join(", ")
            ));
        }

        self.out.push_str(&format!(
            "{}  for (int64_t {v} = {s}; {v} < {e}; {v}++) {{\n",
            pad,
            v = c_var,
            s = s,
            e = bound
        ));

        for stmt in body {
            self.emit_stmt(stmt, ind + 2);
        }

        self.out.push_str(&format!("{}  }}\n", pad));
        self.out.push_str(&format!("{}}}\n", pad));
        self.var_ty = saved;
    }

    fn expr_ty(&self, e: &Expr) -> Ty {
        match e {
            Expr::Located(inner, _) => self.expr_ty(inner),
            Expr::Int(_) => Ty::I64,
            Expr::Float(_) => Ty::F64,
            Expr::Bool(_) => Ty::Bool,
            Expr::Not(_) => Ty::Bool,
            Expr::Neg(inner) => self.expr_ty(inner),
            Expr::Str(_) => Ty::Str,
            Expr::Var(n) => self.var_ty.get(n).cloned().unwrap_or(Ty::I64),
            Expr::Call(n, _) => self.fn_ret.get(n).cloned().unwrap_or(Ty::I64),
            Expr::PythonCall { .. } => Ty::F64,
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
                | BinOp::Ne
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
            Expr::Located(inner, _) => self.expr(inner),
            Expr::Not(inner) => format!("(!{})", self.expr(inner)),
            Expr::Neg(inner) => {
                let value = self.expr(inner);
                if self.expr_ty(inner) == Ty::I64 {
                    format!("velox_i64_neg((int64_t)({}))", value)
                } else {
                    format!("(-{})", value)
                }
            }
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
            Expr::Var(v) => c_var_name(v),
            Expr::Field(base, field) => format!("({}).{}", self.expr(base), c_field_name(field)),
            Expr::StructInit(name, fields) => {
                let values: Vec<String> = fields
                    .iter()
                    .map(|(n, e)| format!(".{} = {}", c_field_name(n), self.expr(e)))
                    .collect();
                format!("({}){{ {} }}", c_struct_name(name), values.join(", "))
            }
            Expr::Bin(l, op, r) => {
                let integer_arithmetic = self.expr_ty(l) == Ty::I64
                    && self.expr_ty(r) == Ty::I64
                    && matches!(
                        op,
                        BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod
                    );

                let lo = self.expr(l);
                let ro = self.expr(r);

                if integer_arithmetic {
                    let helper = match op {
                        BinOp::Add => Some("velox_i64_add"),
                        BinOp::Sub => Some("velox_i64_sub"),
                        BinOp::Mul => Some("velox_i64_mul"),
                        BinOp::Div => Some("velox_i64_div"),
                        BinOp::Mod => Some("velox_i64_mod"),
                        _ => None,
                    };

                    if let Some(helper) = helper {
                        return format!("{}((int64_t)({}), (int64_t)({}))", helper, lo, ro);
                    }
                }

                let o = match op {
                    BinOp::Add => "+",
                    BinOp::Sub => "-",
                    BinOp::Mul => "*",
                    BinOp::Div => "/",
                    BinOp::Mod => "%",
                    BinOp::Eq => "==",
                    BinOp::Ne => "!=",
                    BinOp::Lt => "<",
                    BinOp::Gt => ">",
                    BinOp::Le => "<=",
                    BinOp::Ge => ">=",
                    BinOp::And => "&&",
                    BinOp::Or => "||",
                };
                format!("({} {} {})", lo, o, ro)
            }
            Expr::PythonCall {
                module,
                function,
                args,
            } => {
                let values: Vec<String> = args
                    .iter()
                    .map(|arg| format!("(double)({})", self.expr(arg)))
                    .collect();
                format!(
                    "velox_py_call_f64(\"{}\", \"{}\", {}, (double[]){{{}}})",
                    escape_c_string(module),
                    escape_c_string(function),
                    args.len(),
                    values.join(", ")
                )
            }
            Expr::Call(n, args) => {
                let builtin = match n.as_str() {
                    "read_file" => Some(("velox_read_file", 1)),
                    "write_file" => Some(("velox_write_file", 2)),
                    "str_len" => Some(("velox_str_len", 1)),
                    "str_byte_at" => Some(("velox_str_byte_at", 2)),
                    "str_byte_at_fast" => Some(("velox_str_byte_at_fast", 3)),
                    "str_slice" => Some(("velox_str_slice", 3)),
                    "str_concat" => Some(("velox_str_concat", 2)),
                    "str_eq" => Some(("velox_str_eq", 2)),
                    "list_i64_new" => return "velox_list_i64_new()".into(),
                    "list_i64_push" => Some(("velox_list_i64_push", 2)),
                    "list_i64_get" => Some(("velox_list_i64_get", 2)),
                    "list_i64_set" => Some(("velox_list_i64_set", 3)),
                    "list_i64_len" => Some(("velox_list_i64_len", 1)),
                    _ => None,
                };
                if let Some((helper, count)) = builtin {
                    let values: Vec<String> =
                        args.iter().take(count).map(|arg| self.expr(arg)).collect();
                    let call = format!("{}({})", helper, values.join(", "));
                    return if n == "list_i64_push" || n == "list_i64_set" {
                        format!("({}, (void)0)", call)
                    } else {
                        call
                    };
                }

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
                format!("{}({})", self.c_fn_name(n), a.join(", "))
            }
        }
    }
}

fn mangle_c_name(prefix: &str, name: &str) -> String {
    let encoded: String = name
        .as_bytes()
        .iter()
        .map(|byte| format!("{:02x}", byte))
        .collect();
    format!("{}{}", prefix, encoded)
}

fn c_var_name(name: &str) -> String {
    mangle_c_name("vxv_", name)
}

fn c_struct_name(name: &str) -> String {
    mangle_c_name("vxt_", name)
}

fn c_field_name(name: &str) -> String {
    mangle_c_name("vxm_", name)
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
        Ty::Struct(name) => c_struct_name(name),
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
        Expr::Located(inner, _) => expr_root_var(inner),
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

            Stmt::For {
                var,
                start,
                end,
                body,
            } => {
                count_parallel_reads(start, outer, locals, access);
                count_parallel_reads(end, outer, locals, access);

                let mut nested_locals = locals.clone();
                nested_locals.insert(var.clone());
                analyze_parallel_block(body, outer, &mut nested_locals, access)?;
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
        Expr::Located(inner, _) => count_parallel_reads(inner, outer, locals, access),
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

        Expr::Call(_, args) | Expr::PythonCall { args, .. } => {
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

fn strip_located(mut expr: &Expr) -> &Expr {
    while let Expr::Located(inner, _) = expr {
        expr = inner;
    }
    expr
}

fn is_reduction_add(expr: &Expr, target: &str) -> bool {
    match strip_located(expr) {
        Expr::Bin(left, BinOp::Add, right) => {
            (matches!(strip_located(left), Expr::Var(name) if name == target)
                && count_var_occurrences(right, target) == 0)
                || (matches!(strip_located(right), Expr::Var(name) if name == target)
                    && count_var_occurrences(left, target) == 0)
        }
        _ => false,
    }
}

fn count_var_occurrences(expr: &Expr, target: &str) -> usize {
    match expr {
        Expr::Located(inner, _) => count_var_occurrences(inner, target),
        Expr::Not(inner) | Expr::Neg(inner) => count_var_occurrences(inner, target),

        Expr::Var(name) => usize::from(name == target),

        Expr::Bin(left, _, right) => {
            count_var_occurrences(left, target) + count_var_occurrences(right, target)
        }

        Expr::Call(_, args) | Expr::PythonCall { args, .. } => args
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

impl Default for CEmitter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod c_string_escape_tests {
    use super::escape_c_string;

    #[test]
    fn escapes_quotes_slashes_and_control_characters() {
        let input = "a\\\"b\n\r\t\0\u{0001}7";
        assert_eq!(escape_c_string(input), r#"a\\\"b\n\r\t\000\0017"#);
    }

    #[test]
    fn escapes_unicode_control_characters_as_utf8_bytes() {
        assert_eq!(escape_c_string("\u{0085}"), r"\302\205");
    }

    #[test]
    fn preserves_printable_unicode() {
        assert_eq!(escape_c_string("Velox 中文 🚀"), "Velox 中文 🚀");
    }
}
