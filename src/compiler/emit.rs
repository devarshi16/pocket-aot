use super::{
    parse::{Expr, Stmt},
    Error,
};
use std::collections::{BTreeMap, BTreeSet};
pub const APIS: &[&str] = &[
    "cls", "pset", "pget", "rectfill", "rect", "line", "circfill", "circ", "print", "spr", "sspr",
    "sget", "sset", "mget", "mset", "map", "fget", "fset", "camera", "clip", "pal", "palt",
    "color", "cursor", "btn", "btnp", "add", "del", "deli", "count", "flr", "ceil", "abs", "min",
    "max", "mid", "sgn", "sqrt", "sin", "cos", "atan2", "rnd", "srand", "time", "t", "tostr",
    "tonum", "type", "sub", "sfx", "music", "assert", "foreach", "split", "band", "bor", "bxor",
    "bnot", "shl", "shr", "lshr", "rotl", "rotr", "ord", "chr", "stat", "printh", "rawget",
    "rawset", "rawequal", "rawlen", "select",
];
#[derive(Clone)]
struct Binding {
    c: String,
    cell: bool,
}
fn global(n: &str) -> String {
    format!(
        "g_{}",
        n.as_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}
struct Emitter {
    out: String,
    seq: usize,
    scopes: Vec<BTreeMap<String, Binding>>,
    apis: BTreeSet<String>,
    loops: usize,
    captures: BTreeSet<String>,
    functions: Vec<(usize, String)>,
    next_fn: usize,
    assigned: BTreeSet<String>,
}
impl Emitter {
    fn line(&mut self, s: impl AsRef<str>) {
        self.out.push_str(s.as_ref());
        self.out.push('\n');
    }
    fn fresh(&mut self) -> String {
        self.seq += 1;
        format!("v{}", self.seq)
    }
    fn val(&mut self, s: String) -> String {
        let n = self.fresh();
        self.line(format!("V {n}={s};"));
        n
    }
    fn binding(&self, n: &str) -> Binding {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.get(n))
            .cloned()
            .unwrap_or(Binding {
                c: global(n),
                cell: false,
            })
    }
    fn read(&self, n: &str) -> String {
        let b = self.binding(n);
        if b.cell {
            format!("p8_cell_get({})", b.c)
        } else {
            b.c
        }
    }
    fn write(&mut self, n: &str, v: &str) {
        let b = self.binding(n);
        if b.cell {
            self.line(format!("p8_cell_set({},{v});", b.c));
        } else {
            self.line(format!("{}={v};", b.c));
        }
    }
    fn local(&mut self, n: &str, v: &str) {
        let c = self.fresh();
        let cell = self.captures.contains(n);
        self.line(if cell {
            format!("V {c}=p8_cell({v});")
        } else {
            format!("V {c}={v};")
        });
        self.scopes
            .last_mut()
            .unwrap()
            .insert(n.into(), Binding { c, cell });
    }
    fn scoped(&mut self, b: &[Stmt]) -> Result<(), Error> {
        self.line("{");
        self.scopes.push(BTreeMap::new());
        self.body(b)?;
        self.scopes.pop();
        self.line("}");
        Ok(())
    }
    fn function(&mut self, args: &[String], body: &[Stmt]) -> Result<String, Error> {
        let mut used = BTreeSet::new();
        vars(body, &mut used);
        let mut visible = BTreeMap::new();
        for scope in &self.scopes {
            for (n, b) in scope {
                if used.contains(n) {
                    visible.insert(n.clone(), b.clone());
                }
            }
        }
        if visible.len() > 64 {
            return Err(Error::new(
                0,
                "a closure captures more than 64 local variables",
            ));
        }
        let id = self.next_fn;
        self.next_fn += 1;
        let saved_out = std::mem::take(&mut self.out);
        let saved_scopes = std::mem::take(&mut self.scopes);
        let saved_loops = self.loops;
        let saved_captures = std::mem::replace(&mut self.captures, nested_uses(body));
        self.loops = 0;
        let mut scope = BTreeMap::new();
        for (i, (n, b)) in visible.iter().enumerate() {
            if !b.cell {
                return Err(Error::new(0, "internal capture was not boxed"));
            }
            scope.insert(
                n.clone(),
                Binding {
                    c: format!("p8_capture(self,{i})"),
                    cell: true,
                },
            );
        }
        self.scopes = vec![scope];
        self.line(format!(
            "static V fn_{id}(V self,int argc,const V *argv) {{"
        ));
        for (i, a) in args.iter().enumerate() {
            self.local(a, &format!("argc>{i}?argv[{i}]:p8_nil()"));
        }
        self.body(body)?;
        self.line("return p8_return(0,NULL); }");
        let def = std::mem::replace(&mut self.out, saved_out);
        self.scopes = saved_scopes;
        self.loops = saved_loops;
        self.captures = saved_captures;
        self.functions.push((id, def));
        let caps = if visible.is_empty() {
            "NULL".into()
        } else {
            format!(
                "(V[]){{{}}}",
                visible
                    .values()
                    .map(|b| b.c.clone())
                    .collect::<Vec<_>>()
                    .join(",")
            )
        };
        Ok(self.val(format!("p8_closure({id},{},{caps})", visible.len())))
    }
    fn multi(e: &Expr) -> bool {
        matches!(e, Expr::Call(..) | Expr::MethodCall(..))
    }
    fn args(&mut self, args: &[Expr]) -> Result<(String, String), Error> {
        if args.len() > 16 {
            return Err(Error::new(
                0,
                "at most 16 values per expression list are supported",
            ));
        }
        let a = self.fresh();
        let n = self.fresh();
        let capacity = if args.last().is_some_and(Self::multi) {
            16
        } else {
            args.len() + 1
        };
        self.line(format!("V {a}[{capacity}];int {n}=0;"));
        for (i, e) in args.iter().enumerate() {
            let v = self.expr(e)?;
            if i + 1 == args.len() && Self::multi(e) {
                self.line(format!("if({n}+p8_result_count()>16)p8_fail(\"more than 16 values\");for(int j=0;j<p8_result_count();j++){a}[{n}++]=p8_result(j);"));
            } else {
                self.line(format!("{a}[{n}++]={v};"));
            }
        }
        Ok((n, a))
    }
    fn expr(&mut self, e: &Expr) -> Result<String, Error> {
        Ok(match e {
            Expr::Nil => self.val("p8_nil()".into()),
            Expr::Bool(b) => self.val(format!("p8_bool({})", *b as u8)),
            Expr::Num(s) => self.val(format!(
                "p8_num((int32_t)UINT32_C({}))",
                super::number(s)? as u32
            )),
            Expr::Str(s) => {
                if cbytes(s) > 8191 {
                    return Err(Error::new(
                        0,
                        "strings longer than 8191 bytes are not supported yet",
                    ));
                }
                self.val(format!("p8_str({})", cstr(s)))
            }
            Expr::Var(n) => {
                if APIS.contains(&n.as_str()) {
                    self.apis.insert(n.clone());
                }
                self.val(self.read(n))
            }
            Expr::Unary(op, a) => {
                let a = self.expr(a)?;
                self.val(format!("p8_unary({},{a})", cstr(op)))
            }
            Expr::Binary(op, a, b) => {
                let a = self.expr(a)?;
                if op == "and" || op == "or" {
                    let n = self.val(a.clone());
                    self.line(format!(
                        "if ({}p8_truth({a})) {{",
                        if op == "and" { "" } else { "!" }
                    ));
                    let b = self.expr(b)?;
                    self.line(format!("{n}={b}; }}"));
                    n
                } else {
                    let b = self.expr(b)?;
                    self.val(format!("p8_binary({},{a},{b})", cstr(op)))
                }
            }
            Expr::Index(a, k) => {
                let a = self.expr(a)?;
                let k = self.expr(k)?;
                self.val(format!("p8_get({a},{k})"))
            }
            Expr::Table(items) => {
                let t = self.val("p8_table()".into());
                for (k, v) in items {
                    let k = self.expr(k)?;
                    let v = self.expr(v)?;
                    self.line(format!("p8_set({t},{k},{v});"));
                }
                t
            }
            Expr::Scalar(e) => self.expr(e)?,
            Expr::TableTail(items, start, tail) => {
                let t = self.expr(&Expr::Table(items.clone()))?;
                self.expr(tail)?;
                self.line(format!("for(int j=0;j<p8_result_count();j++)p8_set({t},p8_int({start}+j),p8_result(j));"));
                t
            }
            Expr::Function(a, b) => self.function(a, b)?,
            Expr::MethodCall(object, name, args) => {
                let object = self.expr(object)?;
                let f = self.val(format!("p8_get({object},p8_str({}))", cstr(name)));
                let (n, a) = self.args(args)?;
                self.line(format!("if({n}>=16)p8_fail(\"more than 16 method arguments\");for(int j={n};j>0;j--){a}[j]={a}[j-1];{a}[0]={object};{n}++;"));
                self.val(format!("p8_call({f},{n},{n}?{a}:NULL)"))
            }
            Expr::Call(f, args) => {
                if let Expr::Var(n) = f.as_ref() {
                    if !self.scopes.iter().any(|s| s.contains_key(n)) && !self.assigned.contains(n)
                    {
                        if APIS.contains(&n.as_str()) {
                            validate_api(n, args.len())?;
                        } else {
                            return Err(Error::new(0, &format!("unsupported function/API '{n}'")));
                        }
                    }
                }
                let f = self.expr(f)?;
                let (n, a) = self.args(args)?;
                self.val(format!("p8_call({f},{n},{n}?{a}:NULL)"))
            }
        })
    }
    fn body(&mut self, b: &[Stmt]) -> Result<(), Error> {
        for s in b {
            let scoped = !matches!(s, Stmt::Assign(_, _, true) | Stmt::LocalFunction(..));
            if scoped {
                self.line("{");
            }
            self.stmt(s)?;
            if scoped {
                self.line("}");
            }
        }
        Ok(())
    }
    fn stmt(&mut self, s: &Stmt) -> Result<(), Error> {
        match s {
            Stmt::Assign(lhs, rhs, local) => {
                let mut dest = Vec::new();
                for e in lhs {
                    match e {
                        Expr::Var(n) => dest.push((Some(n.clone()), String::new(), String::new())),
                        Expr::Index(t, k) if !local => {
                            let t = self.expr(t)?;
                            let k = self.expr(k)?;
                            dest.push((None, t, k));
                        }
                        _ => return Err(Error::new(0, "unsupported assignment destination")),
                    }
                }
                let (count, vals) = self.args(rhs)?;
                for (i, (n, t, k)) in dest.into_iter().enumerate() {
                    let v = format!("{count}>{i}?{vals}[{i}]:p8_nil()");
                    if let Some(n) = n {
                        if *local {
                            self.local(&n, &v);
                        } else {
                            self.write(&n, &v);
                        }
                    } else {
                        self.line(format!("p8_set({t},{k},{v});"));
                    }
                }
            }
            Stmt::Compound(lhs, op, rhs) => {
                let (dest, t, k, old) = match lhs {
                    Expr::Var(n) => (
                        Some(n.clone()),
                        String::new(),
                        String::new(),
                        self.val(self.read(n)),
                    ),
                    Expr::Index(t, k) => {
                        let t = self.expr(t)?;
                        let k = self.expr(k)?;
                        let old = self.val(format!("p8_get({t},{k})"));
                        (None, t, k, old)
                    }
                    _ => return Err(Error::new(0, "invalid compound assignment")),
                };
                let r = self.expr(rhs)?;
                let v = self.val(format!("p8_binary({},{old},{r})", cstr(op)));
                if let Some(n) = dest {
                    self.write(&n, &v);
                } else {
                    self.line(format!("p8_set({t},{k},{v});"));
                }
            }
            Stmt::Call(e) => {
                self.expr(e)?;
            }
            Stmt::If(b, o) => self.branch(b, o)?,
            Stmt::While(e, b) => {
                self.line("while(1){p8_tick();");
                let v = self.expr(e)?;
                self.line(format!("if(!p8_truth({v}))break;"));
                self.loops += 1;
                self.scoped(b)?;
                self.loops -= 1;
                self.line("}");
            }
            Stmt::Repeat(b, e) => {
                self.line("while(1){p8_tick();");
                self.scopes.push(BTreeMap::new());
                self.loops += 1;
                self.body(b)?;
                self.loops -= 1;
                let v = self.expr(e)?;
                self.line(format!("if(p8_truth({v}))break;"));
                self.scopes.pop();
                self.line("}");
            }
            Stmt::For(name, a, b, c, body) => {
                let a = self.expr(a)?;
                let b = self.expr(b)?;
                let c = self.expr(c)?;
                let it = self.fresh();
                let end = self.fresh();
                let step = self.fresh();
                self.line(format!("{{int64_t {it}=p8_number({a}),{end}=p8_number({b}),{step}=p8_number({c});if(!{step})p8_fail(\"zero loop step\");for(;{step}>0?{it}<={end}:{it}>={end};{it}+={step}){{p8_tick();"));
                self.scopes.push(BTreeMap::new());
                self.local(name, &format!("p8_num((int32_t){it})"));
                self.loops += 1;
                self.body(body)?;
                self.loops -= 1;
                self.scopes.pop();
                self.line("}}");
            }
            Stmt::ForIn(names, values, body) => {
                let [Expr::Call(f, args)] = values.as_slice() else {
                    return Err(Error::new(
                        0,
                        "generic loops currently require all(), pairs() or ipairs()",
                    ));
                };
                let Expr::Var(kind) = f.as_ref() else {
                    return Err(Error::new(
                        0,
                        "custom generic iterators are not supported yet",
                    ));
                };
                if !["all", "pairs", "ipairs"].contains(&kind.as_str())
                    || args.len() != 1
                    || self.scopes.iter().any(|s| s.contains_key(kind))
                    || self.assigned.contains(kind)
                {
                    return Err(Error::new(
                        0,
                        "custom generic iterators are not supported yet",
                    ));
                }
                let t = self.expr(&args[0])?;
                let c = self.fresh();
                let last = self.fresh();
                let k = self.fresh();
                let v = self.fresh();
                self.line(format!("{{int {c}=0;V {last}=p8_nil(),{k},{v};while(p8_iter_next({},{t},&{c},&{last},&{k},&{v})){{p8_tick();",cstr(kind)));
                self.scopes.push(BTreeMap::new());
                for (i, n) in names.iter().enumerate() {
                    let value = if kind == "all" {
                        if i == 0 {
                            v.as_str()
                        } else {
                            "p8_nil()"
                        }
                    } else if i == 0 {
                        &k
                    } else if i == 1 {
                        &v
                    } else {
                        "p8_nil()"
                    };
                    self.local(n, value);
                }
                self.loops += 1;
                self.body(body)?;
                self.loops -= 1;
                self.scopes.pop();
                self.line("}}");
            }
            Stmt::Function(n, a, b) => {
                let f = self.function(a, b)?;
                self.write(n, &f);
            }
            Stmt::LocalFunction(n, a, b) => {
                self.local(n, "p8_nil()");
                let f = self.function(a, b)?;
                self.write(n, &f);
            }
            Stmt::Return(e) => {
                let (n, a) = self.args(e)?;
                self.line(format!("return p8_return({n},{n}?{a}:NULL);"));
            }
            Stmt::Break => {
                if self.loops == 0 {
                    return Err(Error::new(0, "break outside a loop"));
                }
                self.line("break;");
            }
            Stmt::Block(b) => self.scoped(b)?,
        }
        Ok(())
    }
    fn branch(&mut self, b: &[(Expr, Vec<Stmt>)], other: &[Stmt]) -> Result<(), Error> {
        if let Some((v, body)) = b.first() {
            let v = self.expr(v)?;
            self.line(format!("if(p8_truth({v}))"));
            self.scoped(body)?;
            self.line("else{");
            self.branch(&b[1..], other)?;
            self.line("}");
        } else {
            self.scoped(other)?;
        }
        Ok(())
    }
}
fn cbytes(s: &str) -> usize {
    super::charset::bytes(s).len()
}
fn validate_api(name: &str, n: usize) -> Result<(), Error> {
    let (min, max) = match name {
        "cls" | "color" => (0, 1),
        "rnd" => (0, 16),
        "time" | "t" => (0, 0),
        "foreach" | "shl" | "shr" | "lshr" | "rotl" | "rotr" => (2, 2),
        "band" | "bor" | "bxor" => (1, 16),
        "split" => (1, 3),
        "ord" => (1, 3),
        "chr" => (0, 16),
        "pset" => (2, 3),
        "pget" | "sget" | "mget" => (2, 2),
        "rectfill" | "rect" => (4, 5),
        "line" => (0, 5),
        "rawget" | "rawequal" => (2, 2),
        "rawset" => (3, 3),
        "select" => (1, 16),
        "circfill" | "circ" => (2, 4),
        "print" => (1, 4),
        "spr" => (3, 7),
        "sspr" => (6, 10),
        "sset" => (2, 3),
        "mset" => (3, 3),
        "map" => (0, 7),
        "fget" | "count" | "tostr" | "deli" => (1, 2),
        "fset" | "add" | "sub" => (2, 3),
        "camera" | "btn" | "btnp" => (0, 2),
        "cursor" => (0, 3),
        "clip" => (0, 5),
        "pal" => (0, 3),
        "palt" => (0, 2),
        "min" | "max" | "atan2" | "del" => (2, 2),
        "mid" => (3, 3),
        "sfx" => (1, 4),
        "music" => (0, 4),
        "assert" => (1, 16),
        _ => (1, 1),
    };
    if n < min
        || n > max
        || (name == "clip" && n != 0 && n != 4 && n != 5)
        || (name == "pal" && n == 1)
    {
        Err(Error::new(
            0,
            &format!("unsupported {name}() overload with {n} arguments"),
        ))
    } else {
        Ok(())
    }
}

pub fn cstr(s: &str) -> String {
    let mut out = String::from("\"");
    for b in super::charset::bytes(s) {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            32..=126 => out.push(b as char),
            _ => out.push_str(&format!("\\{:03o}", b)),
        }
    }
    out.push('"');
    out
}

fn vars_expr(e: &Expr, n: &mut BTreeSet<String>) {
    match e {
        Expr::Var(s) => {
            n.insert(s.clone());
        }
        Expr::Scalar(a) | Expr::Unary(_, a) => vars_expr(a, n),
        Expr::Binary(_, a, b) | Expr::Index(a, b) => {
            vars_expr(a, n);
            vars_expr(b, n);
        }
        Expr::Call(f, a) => {
            vars_expr(f, n);
            for e in a {
                vars_expr(e, n);
            }
        }
        Expr::MethodCall(f, _, a) => {
            vars_expr(f, n);
            for e in a {
                vars_expr(e, n);
            }
        }
        Expr::Table(a) => {
            for (k, v) in a {
                vars_expr(k, n);
                vars_expr(v, n);
            }
        }
        Expr::TableTail(items, _, tail) => {
            for (k, v) in items {
                vars_expr(k, n);
                vars_expr(v, n);
            }
            vars_expr(tail, n);
        }
        Expr::Function(args, b) => {
            n.extend(args.iter().cloned());
            vars(b, n);
        }
        _ => {}
    }
}
fn vars(b: &[Stmt], n: &mut BTreeSet<String>) {
    for s in b {
        match s {
            Stmt::Assign(a, b, _) => {
                for e in a.iter().chain(b) {
                    vars_expr(e, n);
                }
            }
            Stmt::Compound(a, _, b) => {
                vars_expr(a, n);
                vars_expr(b, n);
            }
            Stmt::Call(e) => vars_expr(e, n),
            Stmt::If(a, b) => {
                for (e, b) in a {
                    vars_expr(e, n);
                    vars(b, n);
                }
                vars(b, n);
            }
            Stmt::While(e, b) | Stmt::Repeat(b, e) => {
                vars_expr(e, n);
                vars(b, n);
            }
            Stmt::For(name, a, b, c, body) => {
                n.insert(name.clone());
                for e in [a, b, c] {
                    vars_expr(e, n);
                }
                vars(body, n);
            }
            Stmt::ForIn(a, b, body) => {
                n.extend(a.iter().cloned());
                for e in b {
                    vars_expr(e, n);
                }
                vars(body, n);
            }
            Stmt::Function(name, a, b) | Stmt::LocalFunction(name, a, b) => {
                n.insert(name.clone());
                n.extend(a.iter().cloned());
                vars(b, n);
            }
            Stmt::Return(es) => {
                for e in es {
                    vars_expr(e, n);
                }
            }
            Stmt::Block(b) => vars(b, n),
            _ => {}
        }
    }
}
fn nested_uses(b: &[Stmt]) -> BTreeSet<String> {
    fn ex(e: &Expr, n: &mut BTreeSet<String>) {
        match e {
            Expr::Function(a, b) => {
                n.extend(a.iter().cloned());
                vars(b, n);
            }
            Expr::Scalar(a) | Expr::Unary(_, a) => ex(a, n),
            Expr::Binary(_, a, b) | Expr::Index(a, b) => {
                ex(a, n);
                ex(b, n);
            }
            Expr::Call(f, a) | Expr::MethodCall(f, _, a) => {
                ex(f, n);
                for e in a {
                    ex(e, n);
                }
            }
            Expr::Table(a) => {
                for (k, v) in a {
                    ex(k, n);
                    ex(v, n);
                }
            }
            Expr::TableTail(a, _, tail) => {
                for (k, v) in a {
                    ex(k, n);
                    ex(v, n);
                }
                ex(tail, n);
            }
            _ => {}
        }
    }
    fn scan(b: &[Stmt], n: &mut BTreeSet<String>) {
        for s in b {
            match s {
                Stmt::Function(_, a, b) | Stmt::LocalFunction(_, a, b) => {
                    n.extend(a.iter().cloned());
                    vars(b, n);
                }
                Stmt::Assign(a, b, _) => {
                    for e in a.iter().chain(b) {
                        ex(e, n);
                    }
                }
                Stmt::Compound(a, _, b) => {
                    ex(a, n);
                    ex(b, n);
                }
                Stmt::Call(e) => ex(e, n),
                Stmt::Return(es) => {
                    for e in es {
                        ex(e, n);
                    }
                }
                Stmt::If(a, b) => {
                    for (e, b) in a {
                        ex(e, n);
                        scan(b, n);
                    }
                    scan(b, n);
                }
                Stmt::For(_, a, b, c, body) => {
                    for e in [a, b, c] {
                        ex(e, n);
                    }
                    scan(body, n);
                }
                Stmt::ForIn(_, a, b) => {
                    for e in a {
                        ex(e, n);
                    }
                    scan(b, n);
                }
                Stmt::While(e, b) | Stmt::Repeat(b, e) => {
                    ex(e, n);
                    scan(b, n);
                }
                Stmt::Block(b) => scan(b, n),
                _ => {}
            }
        }
    }
    let mut n = BTreeSet::new();
    scan(b, &mut n);
    n
}
fn assigned_expr(e: &Expr, n: &mut BTreeSet<String>) {
    match e {
        Expr::Function(_, b) => assigned(b, n),
        Expr::Scalar(a) | Expr::Unary(_, a) => assigned_expr(a, n),
        Expr::Binary(_, a, b) | Expr::Index(a, b) => {
            assigned_expr(a, n);
            assigned_expr(b, n);
        }
        Expr::Call(f, a) | Expr::MethodCall(f, _, a) => {
            assigned_expr(f, n);
            for e in a {
                assigned_expr(e, n);
            }
        }
        Expr::Table(a) => {
            for (k, v) in a {
                assigned_expr(k, n);
                assigned_expr(v, n);
            }
        }
        Expr::TableTail(a, _, v) => {
            for (k, v) in a {
                assigned_expr(k, n);
                assigned_expr(v, n);
            }
            assigned_expr(v, n);
        }
        _ => {}
    }
}
fn assigned(b: &[Stmt], n: &mut BTreeSet<String>) {
    for s in b {
        match s {
            Stmt::Assign(a, b, _) => {
                for e in a {
                    if let Expr::Var(s) = e {
                        n.insert(s.clone());
                    }
                    assigned_expr(e, n);
                }
                for e in b {
                    assigned_expr(e, n);
                }
            }
            Stmt::Function(name, _, b) | Stmt::LocalFunction(name, _, b) => {
                n.insert(name.clone());
                assigned(b, n);
            }
            Stmt::If(a, b) => {
                for (e, b) in a {
                    assigned_expr(e, n);
                    assigned(b, n);
                }
                assigned(b, n);
            }
            Stmt::While(e, b) | Stmt::Repeat(b, e) => {
                assigned_expr(e, n);
                assigned(b, n);
            }
            Stmt::For(_, a, b, c, body) => {
                for e in [a, b, c] {
                    assigned_expr(e, n);
                }
                assigned(body, n);
            }
            Stmt::ForIn(_, a, b) => {
                for e in a {
                    assigned_expr(e, n);
                }
                assigned(b, n);
            }
            Stmt::Block(b) => assigned(b, n),
            Stmt::Call(e) => assigned_expr(e, n),
            Stmt::Return(es) => {
                for e in es {
                    assigned_expr(e, n);
                }
            }
            Stmt::Compound(a, _, b) => {
                assigned_expr(a, n);
                assigned_expr(b, n);
            }
            _ => {}
        }
    }
}
pub fn emit(body: &[Stmt], title: &str) -> Result<(String, Vec<String>), Error> {
    let mut names = BTreeSet::new();
    vars(body, &mut names);
    for n in ["_init", "_draw", "_update", "_update60"] {
        names.insert(n.into());
    }
    let mut set = BTreeSet::new();
    assigned(body, &mut set);
    let mut e = Emitter {
        out: String::new(),
        seq: 0,
        scopes: vec![BTreeMap::new()],
        apis: BTreeSet::new(),
        loops: 0,
        captures: nested_uses(body),
        functions: Vec::new(),
        next_fn: 0,
        assigned: set,
    };
    e.line("void p8_program_init(void) { p8_enter();");
    for (i, n) in APIS.iter().enumerate() {
        if names.contains(*n) {
            e.line(format!("{}=p8_closure({},0,NULL);", global(n), 10000 + i));
        }
    }
    e.body(body)?;
    e.line(format!(
        "if({}.tag!=P8_NIL)p8_call({},0,NULL);p8_leave();}}",
        global("_init"),
        global("_init")
    ));
    let init = std::mem::take(&mut e.out);
    e.line("/* Pocket AOT native closures; no Lua VM. */\n#include \"p8.h\"");
    e.line(format!("const char *p8_title={};", cstr(title)));
    e.line(format!(
        "const unsigned p8_target_hz={};",
        if e.assigned.contains("_update60") {
            60
        } else {
            30
        }
    ));
    for n in &names {
        e.line(format!("static V {};", global(n)));
    }
    for (_, def) in e.functions.clone() {
        e.line(def);
    }
    e.line("V p8_dispatch(int id,V self,int argc,const V *argv){switch(id){");
    for (id, _) in e.functions.clone() {
        e.line(format!("case {id}:return fn_{id}(self,argc,argv);"));
    }
    for (i, n) in APIS.iter().enumerate() {
        if names.contains(*n) {
            // These builtins publish their own result list, including zero results.
            // Keep other APIs scalar: e.g. foreach may leave callback results behind.
            if matches!(*n, "camera" | "cursor" | "clip" | "select" | "assert") {
                e.line(format!(
                    "case {}:return p8_api_id(P8_API_{},argc,argv);",
                    10000 + i,
                    n
                ));
            } else {
                e.line(format!(
                    "case {}:{{V result=p8_api_id(P8_API_{},argc,argv);return p8_return(1,&result);}}",
                    10000 + i, n
                ));
            }
        }
    }
    e.line("default:p8_fail(\"invalid native function\");return p8_nil();}}");
    e.line(init);
    e.line(format!("void p8_program_update(void){{if({}.tag!=P8_NIL)p8_call({},0,NULL);else if({}.tag!=P8_NIL)p8_call({},0,NULL);}}",global("_update60"),global("_update60"),global("_update"),global("_update")));
    e.line(format!(
        "void p8_program_draw(void){{if({}.tag!=P8_NIL)p8_call({},0,NULL);}}",
        global("_draw"),
        global("_draw")
    ));
    e.line("void p8_program_roots(void){");
    for n in &names {
        e.line(format!("p8_mark({});", global(n)));
    }
    e.line("}");
    e.line("void p8_program_clear(void){");
    for n in &names {
        e.line(format!("{}=p8_nil();", global(n)));
    }
    e.line("}");
    e.line("V p8_debug_global(const char *name){");
    for n in &names {
        e.line(format!(
            "if(!strcmp(name,{}))return {};",
            cstr(n),
            global(n)
        ));
    }
    e.line("return p8_nil();}");
    Ok((e.out, e.apis.into_iter().collect()))
}
