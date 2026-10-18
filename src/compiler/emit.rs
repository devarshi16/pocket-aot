use super::{
    parse::{Expr, Stmt},
    Error,
};
use std::collections::{BTreeMap, BTreeSet};
pub const APIS: &[&str] = &[
    "cls",
    "pset",
    "pget",
    "rectfill",
    "rect",
    "line",
    "circfill",
    "circ",
    "print",
    "spr",
    "sspr",
    "sget",
    "sset",
    "mget",
    "mset",
    "map",
    "fget",
    "fset",
    "camera",
    "clip",
    "pal",
    "palt",
    "color",
    "cursor",
    "btn",
    "btnp",
    "add",
    "del",
    "deli",
    "count",
    "flr",
    "ceil",
    "abs",
    "min",
    "max",
    "mid",
    "sgn",
    "sqrt",
    "sin",
    "cos",
    "atan2",
    "rnd",
    "srand",
    "time",
    "t",
    "tostr",
    "tonum",
    "type",
    "sub",
    "sfx",
    "music",
    "assert",
    "foreach",
    "split",
    "band",
    "bor",
    "bxor",
    "bnot",
    "shl",
    "shr",
    "lshr",
    "rotl",
    "rotr",
    "ord",
    "chr",
    "stat",
    "printh",
    "rawget",
    "rawset",
    "rawequal",
    "rawlen",
    "select",
    "peek",
    "poke",
    "peek2",
    "poke2",
    "peek4",
    "poke4",
    "memcpy",
    "memset",
    "reload",
    "cartdata",
    "dget",
    "dset",
    "menuitem",
    "flip",
    "run",
    "reset",
    "stop",
    "extcmd",
    "oval",
    "ovalfill",
    "rrect",
    "rrectfill",
    "fillp",
    "tline",
    "setmetatable",
    "getmetatable",
    "all",
    "pairs",
    "ipairs",
    "next",
    "unpack",
    "pack",
    "cocreate",
    "coresume",
    "costatus",
    "yield",
    "load",
    "ls",
    "serial",
];
/// Builtins that publish their own result list (possibly empty or several values).
const MULTI_RESULT_APIS: &[&str] = &[
    "camera", "cursor", "clip", "select", "assert", "tonum", "ord", "peek", "peek2", "peek4",
    "next", "pairs", "ipairs", "unpack", "coresume", "yield",
];
/// Builtins that always produce exactly one value. A trailing call to one of
/// them needs one slot rather than a whole expansion area, which keeps nested
/// calls such as `mid(0,flr(rnd(8)),max(1,n))` small on the Pico stack.
const SINGLE_RESULT_APIS: &[&str] = &[
    "flr",
    "ceil",
    "abs",
    "sgn",
    "sqrt",
    "sin",
    "cos",
    "atan2",
    "min",
    "max",
    "mid",
    "rnd",
    "band",
    "bor",
    "bxor",
    "bnot",
    "shl",
    "shr",
    "lshr",
    "rotl",
    "rotr",
    "tostr",
    "chr",
    "sub",
    "type",
    "count",
    "btn",
    "btnp",
    "pget",
    "sget",
    "mget",
    "fget",
    "stat",
    "time",
    "t",
    "split",
    "rawget",
    "rawequal",
    "rawlen",
    "getmetatable",
    "setmetatable",
    "costatus",
    "cocreate",
    "all",
    "dget",
];
/// Canonical PICO-8 APIs this target does not provide. Calling one stops the
/// conversion, because deferring would hide a real capability gap behind a
/// runtime error; an undefined cart-local name is not in this set.
const UNIMPLEMENTED_APIS: &[&str] = &["save", "cstore"];
/// Values per expression list. A final multi-value expression may add up to
/// EXPANSION values beyond the fixed entries; this bounds each call site's
/// stack array for the Pico's 16 KB stack.
const MAX_LIST: usize = 64;
const EXPANSION: usize = 16;
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
    /// Index of the first `...` value in argv, inside a vararg function.
    vararg: Option<usize>,
    /// Visible goto labels, innermost block last, mapped to unique C labels.
    labels: Vec<BTreeMap<String, String>>,
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
        // Copy globals field by field: GCC gives each whole-struct copy of a
        // static global its own stack slot, which grows long functions' frames.
        if s.starts_with("g_") && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') {
            self.line(format!("V {n}={{{s}.n,{s}.tag}};"));
        } else {
            self.line(format!("V {n}={s};"));
        }
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
        let saved_labels = std::mem::take(&mut self.labels);
        let (args, vararg) = match args.split_last() {
            Some((last, named)) if last == "..." => (named, Some(named.len())),
            _ => (args, None),
        };
        let saved_vararg = std::mem::replace(&mut self.vararg, vararg);
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
        self.labels = saved_labels;
        self.vararg = saved_vararg;
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
    fn multi(&self, e: &Expr) -> bool {
        if let Expr::Call(f, _) = e {
            if let Expr::Var(n) = f.as_ref() {
                if SINGLE_RESULT_APIS.contains(&n.as_str())
                    && !self.assigned.contains(n)
                    && !self.scopes.iter().any(|s| s.contains_key(n))
                {
                    return false;
                }
            }
        }
        matches!(e, Expr::Call(..) | Expr::MethodCall(..)) || is_vararg(e)
    }
    fn vararg_base(&self) -> Result<usize, Error> {
        self.vararg
            .ok_or_else(|| Error::new(0, "'...' used outside a vararg function"))
    }
    /// Append a final multi-value expression (call results or `...`) to `a`.
    fn expand(&mut self, e: &Expr, a: &str, n: &str, capacity: usize) -> Result<(), Error> {
        if is_vararg(e) {
            let base = self.vararg_base()?;
            self.line(format!("for(int j={base};j<argc;j++){{if({n}>={capacity})p8_fail(\"too many values in expression list\");{a}[{n}++]=argv[j];}}"));
        } else {
            self.expr(e)?;
            self.line(format!("if({n}+p8_result_count()>{capacity})p8_fail(\"too many values in expression list\");for(int j=0;j<p8_result_count();j++){a}[{n}++]=p8_result(j);"));
        }
        Ok(())
    }
    fn args(&mut self, args: &[Expr]) -> Result<(String, String), Error> {
        if args.len() > MAX_LIST {
            return Err(Error::new(
                0,
                "at most 64 values per expression list are supported",
            ));
        }
        let a = self.fresh();
        let n = self.fresh();
        let capacity = if args.last().is_some_and(|e| self.multi(e)) {
            args.len() - 1 + EXPANSION
        } else {
            args.len() + 1
        };
        self.line(format!("V {a}[{capacity}];int {n}=0;"));
        for (i, e) in args.iter().enumerate() {
            if i + 1 == args.len() && self.multi(e) {
                self.expand(e, &a, &n, capacity)?;
            } else {
                let v = self.expr(e)?;
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
                let length = cbytes(s);
                if length > 8191 {
                    return Err(Error::new(
                        0,
                        "strings longer than 8191 bytes are not supported yet",
                    ));
                }
                if super::charset::bytes(s).contains(&0) {
                    self.val(format!("p8_strn({},{length})", cstr(s)))
                } else {
                    self.val(format!("p8_str({})", cstr(s)))
                }
            }
            Expr::Var(n) if n == "..." => {
                let base = self.vararg_base()?;
                self.val(format!("argc>{base}?argv[{base}]:p8_nil()"))
            }
            Expr::Var(n) => {
                if APIS.contains(&n.as_str()) {
                    self.apis.insert(n.clone());
                }
                // Only assignment statements change an uncaptured local, and no
                // statement runs inside an expression, so it needs no copy.
                if let Some(b) = self.scopes.iter().rev().find_map(|s| s.get(n)) {
                    if !b.cell {
                        return Ok(b.c.clone());
                    }
                }
                self.val(self.read(n))
            }
            Expr::Unary(op, a) => {
                match op.as_str() {
                    "@" => self.apis.insert("peek".into()),
                    "%" => self.apis.insert("peek2".into()),
                    "$" => self.apis.insert("peek4".into()),
                    _ => false,
                };
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
                if is_vararg(tail) {
                    let base = self.vararg_base()?;
                    self.line(format!(
                        "for(int j={base};j<argc;j++)p8_set({t},p8_int({start}+j-{base}),argv[j]);"
                    ));
                } else {
                    self.expr(tail)?;
                    self.line(format!("for(int j=0;j<p8_result_count();j++)p8_set({t},p8_int({start}+j),p8_result(j));"));
                }
                t
            }
            Expr::Function(a, b) => self.function(a, b)?,
            Expr::MethodCall(object, name, args) => {
                let object = self.expr(object)?;
                let f = self.val(format!("p8_get({object},p8_str({}))", cstr(name)));
                let (n, a) = self.args(args)?;
                let room = if args.last().is_some_and(|e| self.multi(e)) {
                    args.len() - 1 + EXPANSION
                } else {
                    args.len() + 1
                };
                self.line(format!("if({n}>={room})p8_fail(\"too many method arguments\");for(int j={n};j>0;j--){a}[j]={a}[j-1];{a}[0]={object};{n}++;"));
                self.val(format!("p8_call({f},{n},{n}?{a}:NULL)"))
            }
            Expr::Call(f, args) => {
                if let Expr::Var(n) = f.as_ref() {
                    if !self.scopes.iter().any(|s| s.contains_key(n)) && !self.assigned.contains(n)
                    {
                        if APIS.contains(&n.as_str()) {
                            // A trailing call or '...' expands to an unknown number of
                            // values, so the syntactic count is not the runtime count.
                            if !args.last().is_some_and(|e| self.multi(e)) {
                                validate_api(n, args.len())?;
                            }
                        } else if UNIMPLEMENTED_APIS.contains(&n.as_str()) {
                            return Err(Error::new(0, &format!("unsupported function/API '{n}'")));
                        }
                        // Any other undefined name is an ordinary nil global. Lua
                        // raises only if the call is reached, so defer to runtime
                        // instead of rejecting a cartridge for an unused branch.
                    }
                }
                let f = self.expr(f)?;
                let (n, a) = self.args(args)?;
                self.val(format!("p8_call({f},{n},{n}?{a}:NULL)"))
            }
        })
    }
    fn body(&mut self, b: &[Stmt]) -> Result<(), Error> {
        let mut labels = BTreeMap::new();
        for s in b {
            if let Stmt::Label(name) = s {
                let c = format!("L{}", self.fresh());
                if labels.insert(name.clone(), c).is_some() {
                    return Err(Error::new(0, &format!("label '{name}' already defined")));
                }
            }
        }
        self.labels.push(labels);
        let result = self.statements(b);
        self.labels.pop();
        result
    }
    fn statements(&mut self, b: &[Stmt]) -> Result<(), Error> {
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
                if *local {
                    // Locals outlive their statement, so evaluate the values in an
                    // inner block: its argument array then does not stay on the
                    // native stack for the rest of the enclosing block.
                    // An uncaptured local is the C variable itself; binding it only
                    // after the values keeps `local x=x` reading the outer x.
                    let names: Vec<String> = dest
                        .into_iter()
                        .map(|(n, _, _)| n.expect("local destination"))
                        .collect();
                    let temps: Vec<String> = names.iter().map(|_| self.fresh()).collect();
                    for t in &temps {
                        self.line(format!("V {t}=p8_nil();"));
                    }
                    self.line("{");
                    let (count, vals) = self.args(rhs)?;
                    for (i, t) in temps.iter().enumerate() {
                        self.line(format!("if({count}>{i}){t}={vals}[{i}];"));
                    }
                    self.line("}");
                    for (n, t) in names.into_iter().zip(temps) {
                        if self.captures.contains(&n) {
                            self.local(&n, &t);
                        } else {
                            let scope = self.scopes.last_mut().unwrap();
                            scope.insert(n, Binding { c: t, cell: false });
                        }
                    }
                    return Ok(());
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
                let builtin = match values.as_slice() {
                    [Expr::Call(f, args)] => match f.as_ref() {
                        Expr::Var(kind)
                            if ["all", "pairs", "ipairs"].contains(&kind.as_str())
                                && args.len() == 1
                                && !self.scopes.iter().any(|s| s.contains_key(kind))
                                && !self.assigned.contains(kind) =>
                        {
                            Some((kind.clone(), args[0].clone()))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let Some((kind, table)) = builtin else {
                    return self.generic_for(names, values, body);
                };
                let args = [table];
                let t = self.expr(&args[0])?;
                let c = self.fresh();
                let last = self.fresh();
                let k = self.fresh();
                let v = self.fresh();
                self.line(format!("{{int {c}=0;V {last}=p8_nil(),{k},{v};while(p8_iter_next({},{t},&{c},&{last},&{k},&{v})){{p8_tick();",cstr(&kind)));
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
                // The closure's capture list is a block-scoped compound literal.
                self.line("{");
                let f = self.function(a, b)?;
                self.write(n, &f);
                self.line("}");
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
            Stmt::Label(name) => {
                let c = self.labels.last().and_then(|l| l.get(name)).cloned();
                let c = c.ok_or_else(|| Error::new(0, "internal label lookup failed"))?;
                self.line(format!("{c}:;"));
            }
            Stmt::Goto(name) => {
                let target = self.labels.iter().rev().find_map(|l| l.get(name)).cloned();
                let Some(c) = target else {
                    return Err(Error::new(
                        0,
                        &format!("no visible label '{name}' for goto"),
                    ));
                };
                // Backward jumps can loop, so each goto spends instruction budget.
                self.line(format!("p8_tick();goto {c};"));
            }
        }
        Ok(())
    }
    /// `for names in explist`: call f(s, control) until the first result is nil.
    fn generic_for(
        &mut self,
        names: &[String],
        values: &[Expr],
        body: &[Stmt],
    ) -> Result<(), Error> {
        let (count, list) = self.args(values)?;
        let f = self.fresh();
        let state = self.fresh();
        let control = self.fresh();
        self.line(format!("{{V {f}={count}>0?{list}[0]:p8_nil(),{state}={count}>1?{list}[1]:p8_nil(),{control}={count}>2?{list}[2]:p8_nil();"));
        self.line(format!(
            "while(1){{p8_tick();{{V step[2]={{{state},{control}}};p8_call({f},2,step);}}"
        ));
        let results: Vec<String> = names.iter().map(|_| self.fresh()).collect();
        for (i, r) in results.iter().enumerate() {
            self.line(format!("V {r}=p8_result({i});"));
        }
        let first = results
            .first()
            .ok_or_else(|| Error::new(0, "generic for needs a variable"))?;
        self.line(format!("if({first}.tag==P8_NIL)break;{control}={first};"));
        self.scopes.push(BTreeMap::new());
        for (n, r) in names.iter().zip(&results) {
            self.local(n, r);
        }
        self.loops += 1;
        self.body(body)?;
        self.loops -= 1;
        self.scopes.pop();
        self.line("}}");
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
fn is_vararg(e: &Expr) -> bool {
    matches!(e, Expr::Var(n) if n == "...")
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
        // PICO-8 ignores surplus arguments to its built-ins, as Lua does.
        "print" => (1, 16),
        // Omitted coordinates and sizes read as 0, so short calls are legal.
        "spr" => (1, 7),
        "sspr" => (1, 10),
        "sset" => (2, 3),
        "mset" => (3, 3),
        "map" => (0, 7),
        "fget" | "count" | "tonum" | "deli" => (1, 2),
        "tostr" => (0, 2),
        "fset" | "add" | "sub" => (2, 3),
        "camera" | "btn" | "btnp" => (0, 2),
        "cursor" => (0, 3),
        "clip" => (0, 5),
        "pal" => (0, 3),
        "palt" => (0, 4),
        // max(x) is max(x,0) in PICO-8; the omitted operand reads as zero.
        "min" | "max" => (1, 2),
        "atan2" | "del" => (2, 2),
        // stat(28,k) queries a specific key; this target reports no keyboard.
        "stat" => (1, 2),
        // mid(a,b) compares against a third operand of zero.
        "mid" => (2, 3),
        "sfx" => (1, 4),
        "music" => (0, 4),
        "assert" => (1, 16),
        "printh" => (1, 4),
        "peek" | "peek2" | "peek4" | "next" => (1, 2),
        "poke" | "poke2" | "poke4" | "coresume" => (1, 64),
        "memcpy" | "memset" => (3, 3),
        "reload" => (0, 4),
        // dset(i) stores zero; the runtime already treats a missing value as 0.
        "dset" => (1, 2),
        "setmetatable" => (2, 2),
        "menuitem" | "extcmd" | "unpack" => (1, 3),
        "flip" | "reset" => (0, 0),
        "run" => (0, 1),
        "fillp" => (0, 3),
        "stop" => (0, 4),
        "oval" | "ovalfill" => (4, 5),
        "rrect" | "rrectfill" => (4, 6),
        "tline" => (6, 9),
        "pack" | "yield" => (0, 64),
        "load" => (1, 3),
        "ls" => (0, 1),
        "serial" => (3, 3),
        _ => (1, 1),
    };
    if n < min || n > max {
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
        vararg: None,
        labels: Vec::new(),
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
            if MULTI_RESULT_APIS.contains(n) {
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
