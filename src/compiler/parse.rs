use super::Error;

#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub s: String,
    pub line: usize,
    pub kind: u8,
}
#[derive(Clone, Debug)]
pub enum Expr {
    Num(String),
    Str(String),
    Bool(bool),
    Nil,
    Var(String),
    Unary(String, Box<Expr>),
    Binary(String, Box<Expr>, Box<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Table(Vec<(Expr, Expr)>),
    TableTail(Vec<(Expr, Expr)>, usize, Box<Expr>),
    Scalar(Box<Expr>),
    Function(Vec<String>, Vec<Stmt>),
    MethodCall(Box<Expr>, String, Vec<Expr>),
}
#[derive(Clone, Debug)]
pub enum Stmt {
    Assign(Vec<Expr>, Vec<Expr>, bool),
    Compound(Expr, String, Expr),
    Call(Expr),
    If(Vec<(Expr, Vec<Stmt>)>, Vec<Stmt>),
    While(Expr, Vec<Stmt>),
    Repeat(Vec<Stmt>, Expr),
    For(String, Expr, Expr, Expr, Vec<Stmt>),
    Function(String, Vec<String>, Vec<Stmt>),
    LocalFunction(String, Vec<String>, Vec<Stmt>),
    ForIn(Vec<String>, Vec<Expr>, Vec<Stmt>),
    Return(Vec<Expr>),
    Break,
    Block(Vec<Stmt>),
}

fn ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_' || (!ch.is_ascii() && !ch.is_whitespace())
}
fn ident_continue(ch: char) -> bool {
    ident_start(ch) || ch.is_ascii_digit()
}
// PICO-8 uses Lua's [=[...]=] long literal delimiters, including [[...]].
fn long_open(c: &[char], i: usize) -> Option<(usize, usize)> {
    if c.get(i) != Some(&'[') {
        return None;
    }
    let mut j = i + 1;
    while c.get(j) == Some(&'=') {
        j += 1;
    }
    (c.get(j) == Some(&'[')).then_some((j - i - 1, j + 1))
}
fn long_body(
    c: &[char],
    i: &mut usize,
    line: &mut usize,
    level: usize,
    start: usize,
) -> Result<String, Error> {
    let mut s = String::new();
    // An immediate newline after the opening delimiter is not part of a long string.
    if matches!(c.get(*i), Some('\r' | '\n')) {
        let first = c[*i];
        *i += 1;
        if c.get(*i)
            .is_some_and(|ch| (*ch == '\r' || *ch == '\n') && *ch != first)
        {
            *i += 1;
        }
        *line += 1;
    }
    while *i < c.len() {
        if c[*i] == ']' {
            let mut j = *i + 1;
            while c.get(j) == Some(&'=') {
                j += 1;
            }
            if j - *i - 1 == level && c.get(j) == Some(&']') {
                *i = j + 1;
                return Ok(s);
            }
        }
        let ch = c[*i];
        *i += 1;
        if ch == '\r' || ch == '\n' {
            if c.get(*i)
                .is_some_and(|next| (*next == '\r' || *next == '\n') && *next != ch)
            {
                *i += 1;
            }
            *line += 1;
            s.push('\n');
        } else {
            s.push(ch);
        }
    }
    Err(Error::new(
        start,
        "unterminated long string or block comment",
    ))
}
fn byte_char(value: u32) -> char {
    char::from_u32(if value < 128 { value } else { 0xe000 + value }).unwrap()
}
pub fn lex(src: &str) -> Result<Vec<Token>, Error> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut line = 1;
    let mut out = Vec::new();
    while i < c.len() {
        if c[i].is_whitespace() {
            if c[i] == '\n' {
                line += 1;
            }
            i += 1;
            continue;
        }
        if c[i] == '-' && c.get(i + 1) == Some(&'-') {
            i += 2;
            if let Some((level, body)) = long_open(&c, i) {
                let start = line;
                i = body;
                long_body(&c, &mut i, &mut line, level, start)?;
            } else {
                while i < c.len() && c[i] != '\n' {
                    i += 1;
                }
            }
            continue;
        }
        let start = i;
        let token_line = line;
        if let Some((level, body)) = long_open(&c, i) {
            i = body;
            let s = long_body(&c, &mut i, &mut line, level, token_line)?;
            out.push(Token {
                s,
                line: token_line,
                kind: 2,
            });
            continue;
        }
        if c[i] == '\'' || c[i] == '"' {
            let q = c[i];
            i += 1;
            let mut s = String::new();
            while i < c.len() && c[i] != q {
                if c[i] == '\\' {
                    i += 1;
                    if i >= c.len() {
                        break;
                    }
                    if c[i].is_ascii_digit() {
                        let mut value = 0u32;
                        let mut digits = 0;
                        while i < c.len() && c[i].is_ascii_digit() && digits < 3 {
                            value = value * 10 + c[i].to_digit(10).unwrap();
                            i += 1;
                            digits += 1;
                        }
                        if value == 0 || value > 255 {
                            return Err(Error::new(
                                line,
                                "numeric string escape must be between 1 and 255 in this subset",
                            ));
                        }
                        s.push(byte_char(value));
                        continue;
                    }
                    if c[i] == 'x' {
                        i += 1;
                        let mut value = 0;
                        for _ in 0..2 {
                            let digit =
                                c.get(i).and_then(|ch| ch.to_digit(16)).ok_or_else(|| {
                                    Error::new(
                                        line,
                                        "hex string escape needs two hexadecimal digits",
                                    )
                                })?;
                            value = value * 16 + digit;
                            i += 1;
                        }
                        if value == 0 {
                            return Err(Error::new(
                                line,
                                "NUL string bytes are not supported by the runtime",
                            ));
                        }
                        s.push(byte_char(value));
                        continue;
                    }
                    if c[i] == 'z' {
                        i += 1;
                        while i < c.len() && c[i].is_whitespace() {
                            if c[i] == '\n' {
                                line += 1;
                            }
                            i += 1;
                        }
                        continue;
                    }
                    let ch = match c[i] {
                        '*' => '\x01',
                        '#' => '\x02',
                        '-' => '\x03',
                        '|' => '\x04',
                        '+' => '\x05',
                        '^' => '\x06',
                        'a' => '\x07',
                        'b' => '\x08',
                        't' => '\t',
                        'n' => '\n',
                        'v' => '\x0b',
                        'f' => '\x0c',
                        'r' => '\r',
                        '\\' => '\\',
                        '\'' => '\'',
                        '"' => '"',
                        '\n' => {
                            line += 1;
                            '\n'
                        }
                        '\r' => {
                            if c.get(i + 1) == Some(&'\n') {
                                i += 1;
                            }
                            line += 1;
                            '\n'
                        }
                        _ => return Err(Error::new(line, "invalid PICO-8 string escape")),
                    };
                    s.push(ch);
                    i += 1;
                } else {
                    if c[i] == '\n' {
                        line += 1;
                    }
                    if c[i] == '\0' {
                        return Err(Error::new(
                            line,
                            "NUL string bytes are not supported by the runtime",
                        ));
                    }
                    s.push(c[i]);
                    i += 1;
                }
            }
            if i >= c.len() {
                return Err(Error::new(token_line, "unterminated string"));
            }
            i += 1;
            out.push(Token {
                s,
                line: token_line,
                kind: 2,
            });
            continue;
        }
        if c[i].is_ascii_digit()
            || (c[i] == '.' && c.get(i + 1).is_some_and(|x| x.is_ascii_digit()))
        {
            if c[i] == '0' && matches!(c.get(i + 1), Some('x' | 'X')) {
                i += 2;
                while i < c.len()
                    && (c[i].is_ascii_hexdigit() || (c[i] == '.' && c.get(i + 1) != Some(&'.')))
                {
                    i += 1;
                }
            } else if c[i] == '0' && matches!(c.get(i + 1), Some('b' | 'B')) {
                i += 2;
                while i < c.len()
                    && (matches!(c[i], '0' | '1') || (c[i] == '.' && c.get(i + 1) != Some(&'.')))
                {
                    i += 1;
                }
            } else {
                while i < c.len() && c[i].is_ascii_digit() {
                    i += 1;
                }
                if c.get(i) == Some(&'.') && c.get(i + 1) != Some(&'.') {
                    i += 1;
                    while i < c.len() && c[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                if matches!(c.get(i), Some('e' | 'E')) {
                    let digit = i + if matches!(c.get(i + 1), Some('+' | '-')) {
                        2
                    } else {
                        1
                    };
                    // A compact `.5end` is a number followed by `end`, not an exponent.
                    if c.get(digit).is_some_and(|ch| ch.is_ascii_digit()) {
                        i = digit;
                        while i < c.len() && c[i].is_ascii_digit() {
                            i += 1;
                        }
                    }
                }
            }
            out.push(Token {
                s: c[start..i].iter().collect(),
                line: token_line,
                kind: 1,
            });
            continue;
        }
        if ident_start(c[i]) {
            i += 1;
            while i < c.len() && ident_continue(c[i]) {
                i += 1;
            }
            out.push(Token {
                s: c[start..i].iter().collect(),
                line: token_line,
                kind: 0,
            });
            continue;
        }
        let mut s = c[i].to_string();
        i += 1;
        // PICO-8 spells integer division with a backslash; keep a common AST operator.
        if s == "\\" {
            s = "//".into();
        }
        if s == "//" && c.get(i) == Some(&'=') {
            s.push('=');
            i += 1;
        }
        if i < c.len() {
            let pair = format!("{}{}", s, c[i]);
            if [
                "==", "~=", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=", "..", "//", "<<", ">>",
                "^^", "&=", "|=", "::",
            ]
            .contains(&pair.as_str())
            {
                s = pair;
                i += 1;
            }
        }
        if s == ".." && c.get(i) == Some(&'.') {
            s.push('.');
            i += 1;
        } else if ["..", "//", "<<", ">>", "^^"].contains(&s.as_str()) && c.get(i) == Some(&'=') {
            s.push('=');
            i += 1;
        }
        out.push(Token {
            s,
            line: token_line,
            kind: 0,
        });
    }
    out.push(Token {
        s: "<eof>".into(),
        line,
        kind: 0,
    });
    Ok(out)
}

pub struct Parser {
    t: Vec<Token>,
    i: usize,
    pub current_line: usize,
    short_line: Option<usize>,
}
impl Parser {
    pub fn new(src: &str) -> Result<Self, Error> {
        Ok(Self {
            t: lex(src)?,
            i: 0,
            current_line: 1,
            short_line: None,
        })
    }
    fn tok(&self) -> &Token {
        &self.t[self.i]
    }
    fn at(&self, s: &str) -> bool {
        self.tok().s == s && self.tok().kind == 0
    }
    fn take(&mut self) -> Token {
        let t = self.t[self.i].clone();
        self.current_line = t.line;
        if self.i + 1 < self.t.len() {
            self.i += 1;
        }
        t
    }
    fn eat(&mut self, s: &str) -> bool {
        if self.at(s) {
            self.take();
            true
        } else {
            false
        }
    }
    fn err(&self, s: &str) -> Error {
        Error::new(self.tok().line, s)
    }
    fn need(&mut self, s: &str) -> Result<(), Error> {
        if self.eat(s) {
            Ok(())
        } else {
            Err(self.err(&format!("expected '{s}', found '{}'", self.tok().s)))
        }
    }
    fn name(&mut self) -> Result<String, Error> {
        let t = self.take();
        if t.kind == 0 && t.s.chars().next().is_some_and(ident_start) && !is_keyword(&t.s) {
            Ok(t.s)
        } else {
            Err(Error::new(t.line, "expected an identifier"))
        }
    }
    pub fn parse(&mut self) -> Result<Vec<Stmt>, Error> {
        let s = self.block(&["<eof>"])?;
        self.need("<eof>")?;
        Ok(s)
    }
    fn block(&mut self, ends: &[&str]) -> Result<Vec<Stmt>, Error> {
        let mut b = Vec::new();
        while !ends.iter().any(|s| self.at(s)) {
            if self.at("<eof>") {
                return Err(self.err("unexpected end of source"));
            }
            if self.eat(";") {
                continue;
            }
            b.push(self.statement()?);
        }
        Ok(b)
    }
    fn statement(&mut self) -> Result<Stmt, Error> {
        if self.eat("function") {
            let name = self.name()?;
            let mut target = Expr::Var(name.clone());
            while self.eat(".") {
                target = Expr::Index(Box::new(target), Box::new(Expr::Str(self.name()?)));
            }
            let method = if self.eat(":") {
                target = Expr::Index(Box::new(target), Box::new(Expr::Str(self.name()?)));
                true
            } else {
                false
            };
            let (mut args, body) = self.function_body()?;
            if method {
                args.insert(0, "self".into());
            }
            return Ok(if matches!(target, Expr::Var(_)) {
                Stmt::Function(name, args, body)
            } else {
                Stmt::Assign(vec![target], vec![Expr::Function(args, body)], false)
            });
        }
        if self.eat("if") {
            let line = self.current_line;
            let cond = self.expr(0)?;
            if !(self.eat("then") || self.eat("do")) {
                // PICO-8 shorthand: if (condition) statements on the same line.
                if self.tok().line != line {
                    return Err(self.err("expected 'then' or a same-line shorthand statement"));
                }
                let b = self.short_block(line)?;
                let other = if self.tok().line == line && self.eat("else") {
                    self.short_block(line)?
                } else {
                    vec![]
                };
                return Ok(Stmt::If(vec![(cond, b)], other));
            }
            let mut branches = vec![(cond, self.block(&["elseif", "else", "end"])?)];
            while self.eat("elseif") {
                let e = self.expr(0)?;
                self.need("then")?;
                branches.push((e, self.block(&["elseif", "else", "end"])?));
            }
            let other = if self.eat("else") {
                self.block(&["end"])?
            } else {
                vec![]
            };
            self.need("end")?;
            return Ok(Stmt::If(branches, other));
        }
        if self.eat("while") {
            let line = self.current_line;
            let e = self.expr(0)?;
            let b = if self.eat("do") {
                let b = self.block(&["end"])?;
                self.need("end")?;
                b
            } else {
                if self.tok().line != line {
                    return Err(self.err("expected 'do' or a same-line shorthand statement"));
                }
                self.short_block(line)?
            };
            return Ok(Stmt::While(e, b));
        }
        if self.eat("repeat") {
            let b = self.block(&["until"])?;
            self.need("until")?;
            let e = self.expr(0)?;
            return Ok(Stmt::Repeat(b, e));
        }
        if self.eat("for") {
            let n = self.name()?;
            if self.eat("=") {
                let a = self.expr(0)?;
                self.need(",")?;
                let b = self.expr(0)?;
                let c = if self.eat(",") {
                    self.expr(0)?
                } else {
                    Expr::Num("1".into())
                };
                self.need("do")?;
                let body = self.block(&["end"])?;
                self.need("end")?;
                return Ok(Stmt::For(n, a, b, c, body));
            }
            let mut names = vec![n];
            while self.eat(",") {
                names.push(self.name()?);
            }
            self.need("in")?;
            let values = self.exprs()?;
            self.need("do")?;
            let body = self.block(&["end"])?;
            self.need("end")?;
            return Ok(Stmt::ForIn(names, values, body));
        }
        if self.eat("do") {
            let b = self.block(&["end"])?;
            self.need("end")?;
            return Ok(Stmt::Block(b));
        }
        if self.eat("return") {
            let val = if ["end", "else", "elseif", "until", ";", "<eof>"]
                .iter()
                .any(|x| self.at(x))
                || self.short_line.is_some_and(|line| self.tok().line > line)
            {
                vec![]
            } else {
                self.exprs()?
            };
            return Ok(Stmt::Return(val));
        }
        if self.eat("break") {
            return Ok(Stmt::Break);
        }
        if self.at("goto") || self.at("::") {
            return Err(self.err("goto statements and labels are not supported yet"));
        }
        if self.eat("?") {
            let line = self.current_line;
            let args = if self.tok().line != line || self.at("<eof>") {
                vec![]
            } else {
                self.exprs()?
            };
            return Ok(Stmt::Call(Expr::Call(
                Box::new(Expr::Var("print".into())),
                args,
            )));
        }
        let local = self.eat("local");
        if local && self.eat("function") {
            let name = self.name()?;
            let (args, body) = self.function_body()?;
            return Ok(Stmt::LocalFunction(name, args, body));
        }
        let first = if local {
            Expr::Var(self.name()?)
        } else {
            self.expr(0)?
        };
        if matches!(&first, Expr::Call(..) | Expr::MethodCall(..)) && !local && !self.at(",") {
            return Ok(Stmt::Call(first));
        }
        let mut lhs = vec![first];
        while self.eat(",") {
            lhs.push(if local {
                Expr::Var(self.name()?)
            } else {
                self.expr(0)?
            });
        }
        if self.eat("=") {
            let rhs = self.exprs()?;
            return Ok(Stmt::Assign(lhs, rhs, local));
        }
        if [
            "+=", "-=", "*=", "/=", "%=", "&=", "|=", "..=", "//=", "<<=", ">>=", "^^=",
        ]
        .iter()
        .any(|s| self.at(s))
        {
            if lhs.len() != 1 {
                return Err(self.err("compound assignment needs one destination"));
            }
            let token = self.take().s;
            let op = token[..token.len() - 1].to_string();
            let rhs = self.expr(0)?;
            if local {
                return Err(self.err("compound local declaration is invalid"));
            }
            return Ok(Stmt::Compound(lhs.remove(0), op, rhs));
        }
        if local {
            return Ok(Stmt::Assign(lhs, vec![], true));
        }
        Err(self.err("expected assignment or function call"))
    }
    fn function_body(&mut self) -> Result<(Vec<String>, Vec<Stmt>), Error> {
        self.need("(")?;
        let mut args = Vec::new();
        if !self.at(")") {
            loop {
                if self.at("...") {
                    return Err(self.err("varargs are not supported yet"));
                }
                args.push(self.name()?);
                if !self.eat(",") {
                    break;
                }
            }
        }
        self.need(")")?;
        // An anonymous function in a short statement still has a normal multiline body.
        let old = self.short_line.take();
        let body = self.block(&["end"])?;
        self.need("end")?;
        self.short_line = old;
        Ok((args, body))
    }
    fn short_block(&mut self, line: usize) -> Result<Vec<Stmt>, Error> {
        let old = self.short_line.replace(line);
        let mut body = Vec::new();
        while self.tok().line == line
            && !["<eof>", "else", "elseif", "end", "until"]
                .iter()
                .any(|end| self.at(end))
        {
            if self.eat(";") {
                continue;
            }
            body.push(self.statement()?);
        }
        self.short_line = old;
        Ok(body)
    }
    fn call_args(&mut self) -> Result<Vec<Expr>, Error> {
        if self.eat("(") {
            let args = if self.at(")") { vec![] } else { self.exprs()? };
            self.need(")")?;
            Ok(args)
        } else if self.at("{") || self.tok().kind == 2 {
            Ok(vec![self.expr(13)?])
        } else {
            Err(self.err("expected function call arguments"))
        }
    }
    fn exprs(&mut self) -> Result<Vec<Expr>, Error> {
        let mut a = vec![self.expr(0)?];
        while self.eat(",") {
            a.push(self.expr(0)?);
        }
        Ok(a)
    }
    fn expr(&mut self, min: u8) -> Result<Expr, Error> {
        let t = self.take();
        let mut e = if t.kind == 1 {
            Expr::Num(t.s)
        } else if t.kind == 2 {
            Expr::Str(t.s)
        } else {
            match t.s.as_str() {
                "nil" => Expr::Nil,
                "true" => Expr::Bool(true),
                "false" => Expr::Bool(false),
                "-" | "not" | "#" | "~" => Expr::Unary(t.s, Box::new(self.expr(11)?)),
                "(" => {
                    let e = self.expr(0)?;
                    self.need(")")?;
                    Expr::Scalar(Box::new(e))
                }
                "{" => {
                    let mut items = Vec::new();
                    let mut array = 1;
                    let mut tail_index = None;
                    while !self.at("}") {
                        tail_index = None;
                        let (k, v) = if self.eat("[") {
                            let k = self.expr(0)?;
                            self.need("]")?;
                            self.need("=")?;
                            (k, self.expr(0)?)
                        } else if self.t.get(self.i + 1).is_some_and(|t| t.s == "=") {
                            let n = self.name()?;
                            self.need("=")?;
                            (Expr::Str(n), self.expr(0)?)
                        } else {
                            let v = self.expr(0)?;
                            let k = Expr::Num(array.to_string());
                            if matches!(v, Expr::Call(..) | Expr::MethodCall(..)) {
                                tail_index = Some(array);
                            }
                            array += 1;
                            (k, v)
                        };
                        items.push((k, v));
                        if !(self.eat(",") || self.eat(";")) {
                            break;
                        }
                    }
                    self.need("}")?;
                    if let Some(index) = tail_index {
                        let (_, tail) = items.pop().unwrap();
                        Expr::TableTail(items, index, Box::new(tail))
                    } else {
                        Expr::Table(items)
                    }
                }
                "function" => {
                    let (args, body) = self.function_body()?;
                    Expr::Function(args, body)
                }
                "..." => return Err(Error::new(t.line, "varargs are not supported yet")),
                _ => {
                    if t.s.chars().next().is_some_and(ident_start) && !is_keyword(&t.s) {
                        Expr::Var(t.s)
                    } else {
                        return Err(Error::new(
                            t.line,
                            &format!("unsupported expression '{}'", t.s),
                        ));
                    }
                }
            }
        };
        loop {
            if self.eat("[") {
                let k = self.expr(0)?;
                self.need("]")?;
                e = Expr::Index(Box::new(e), Box::new(k));
                continue;
            }
            if self.eat(".") {
                let key = self.name()?;
                e = Expr::Index(Box::new(e), Box::new(Expr::Str(key)));
                continue;
            }
            if self.eat(":") {
                let method = self.name()?;
                let args = self.call_args()?;
                e = Expr::MethodCall(Box::new(e), method, args);
                continue;
            }
            if self.at("(") || self.at("{") || self.tok().kind == 2 {
                let a = self.call_args()?;
                e = Expr::Call(Box::new(e), a);
                continue;
            }
            let op = self.tok().s.clone();
            let Some((l, r)) = binding(&op) else {
                break;
            };
            if l < min {
                break;
            }
            self.take();
            let rhs = self.expr(r)?;
            e = Expr::Binary(op, Box::new(e), Box::new(rhs));
        }
        Ok(e)
    }
}
fn is_keyword(s: &str) -> bool {
    [
        "if", "then", "else", "elseif", "end", "function", "return", "for", "while", "repeat",
        "until", "local", "do", "break", "goto", "in", "and", "or", "not", "nil", "true", "false",
    ]
    .contains(&s)
}
fn binding(op: &str) -> Option<(u8, u8)> {
    Some(match op {
        "or" => (1, 2),
        "and" => (2, 3),
        "==" | "~=" | "!=" | "<" | ">" | "<=" | ">=" => (3, 4),
        "|" => (4, 5),
        "^^" => (5, 6),
        "&" => (6, 7),
        "<<" | ">>" => (7, 8),
        ".." => (8, 8),
        "+" | "-" => (9, 10),
        "*" | "/" | "//" | "%" => (10, 11),
        "^" => (12, 12),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn official_pico8_escapes_and_long_literals() {
        let tokens = lex("a='\\*\\#\\-\\|\\+\\^\\a\\b\\t\\n\\v\\f\\r\\x80\\255'\n--[=[ ignore ]===] ]=]\nb=[==[\nraw\\string\r\nnext]==]").unwrap();
        assert_eq!(
            tokens[2].s,
            "\x01\x02\x03\x04\x05\x06\x07\x08\t\n\x0b\x0c\r\u{e080}\u{e0ff}"
        );
        assert_eq!(tokens[5].s, "raw\\string\nnext");
        assert_eq!(tokens[5].kind, 2);
    }
    #[test]
    fn short_return_stops_at_its_line() {
        let body =
            Parser::new("function _draw()\nif(a)return\nif(b)return\nrectfill(0,0,1,1,2)\nend")
                .unwrap()
                .parse()
                .unwrap();
        let Stmt::Function(_, _, body) = &body[0] else {
            panic!()
        };
        assert_eq!(body.len(), 3);
        let Stmt::If(branches, _) = &body[0] else {
            panic!()
        };
        assert!(matches!(&branches[0].1[0], Stmt::Return(values) if values.is_empty()));
    }
    #[test]
    fn functions_generic_loops_methods_and_shorthand_calls() {
        let body = Parser::new("local function f(x) return function(y) return x+y end end\nfunction player:draw() ?self.x end\nfor k,v in pairs(players) do v:draw() end\ndirs=split'0,1,2'\n⬆️=2\nif(a) x=1 else x=2\nwhile(x<3) x+=1").unwrap().parse().unwrap();
        assert!(matches!(body[0], Stmt::LocalFunction(..)));
        assert!(matches!(body[1], Stmt::Assign(..)));
        assert!(matches!(body[2], Stmt::ForIn(..)));
        assert!(matches!(body[3], Stmt::Assign(..)));
        let Stmt::Assign(lhs, _, _) = &body[4] else {
            panic!()
        };
        assert!(matches!(&lhs[0], Expr::Var(n) if n == "⬆️"));
        let Stmt::If(_, other) = &body[5] else {
            panic!()
        };
        assert_eq!(other.len(), 1);
        assert!(matches!(body[6], Stmt::While(..)));
    }
    #[test]
    fn lex_binary_and_concat_compound_without_ambiguity() {
        let tokens = lex("a=0b101.1 b=0x1..'x' c..='z' d>>=1").unwrap();
        assert_eq!(tokens[2].s, "0b101.1");
        assert!(tokens.iter().any(|t| t.s == "..="));
        assert!(tokens.iter().any(|t| t.s == ">>="));
        assert!(tokens.iter().any(|t| t.s == "0x1"));
    }
    #[test]
    fn multiple_returns_parentheses_and_table_tail() {
        let body = Parser::new("function f() return 1,g() end\na,b=f()\nc,d=(f())\nx={[99]=1, f(),}\ny={f(), z=1}\nz={(f())}\nfunction compact()return .5end").unwrap().parse().unwrap();
        let Stmt::Function(_, _, b) = &body[0] else {
            panic!()
        };
        assert!(matches!(&b[0], Stmt::Return(v) if v.len() == 2));
        let Stmt::Assign(_, rhs, _) = &body[2] else {
            panic!()
        };
        assert!(matches!(&rhs[0], Expr::Scalar(_)));
        let Stmt::Assign(_, rhs, _) = &body[3] else {
            panic!()
        };
        assert!(matches!(&rhs[0], Expr::TableTail(items, 1, _) if items.len() == 1));
        let Stmt::Assign(_, rhs, _) = &body[4] else {
            panic!()
        };
        assert!(matches!(&rhs[0], Expr::Table(_)));
        let Stmt::Assign(_, rhs, _) = &body[5] else {
            panic!()
        };
        assert!(matches!(&rhs[0], Expr::Table(_)));
    }
}
