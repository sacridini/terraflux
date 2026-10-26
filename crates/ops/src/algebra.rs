//! Map algebra: `map.calc` computes maps, stacks and numbers from a script over its inputs, in a language close
//! to Python's (Dinamica EGO's Calculate Map, with statements).
//!
//! ```text
//! ndvi = (img.nir - img.red) / (img.nir + img.red)
//! out ndvi
//! out forest = land == 1 and ndvi > ndvi.percentile(25)
//! out cleaned = land
//! cleaned[land == 9] = null
//! out mean_ndvi = ndvi.mean()
//! ```
//!
//! - **Values:** numbers, `pi`, `null` (or `None`: a missing cell), `True` and `False` (1 and 0); the node's
//!   inputs by port name (maps, numbers, stacks) and a stack's layers as `stack.layer` (their names made names:
//!   [`band_names`]).
//! - **Operators:** `+ - * / %`, `**` and `^` (powers), comparisons `< <= > >= == !=` (1 or 0; chained as in
//!   Python, `5 <= x < 8`), `and or not` (also `&& || !`), `a if c else b` (also `if c then a else b` and
//!   `c ? a : b`). A missing value makes the result missing (`isnull` tests it).
//! - **Functions:** `abs sqrt exp ln log log10 floor ceil round min max clamp(x, lo, hi) isnull(x) coalesce(x, y)`
//!   (`y` where `x` is missing).
//! - **Neighbourhoods** (a square of radius `r` cells around each cell, the cell included, missing cells left
//!   out): `nbsum(x, r) nbmean(x, r) nbmin(x, r) nbmax(x, r) nbcount(x, r)` (cells with a value; `r` a whole number or
//!   a number input).
//! - **Stacks:** an operation on a stack is made on each of its layers (with a map or a number; layer by layer
//!   with a stack of as many layers). `mean sd median sum count min max (s)` and `percentile(s, p)` are the value
//!   of each cell over the layers, missing values left out.
//! - **Methods:** `x.mean() x.sd() x.median() x.sum() x.count() x.min() x.max() x.percentile(p)`: one number from
//!   every cell with a value (of every layer of a stack).
//! - **Statements**, one a line (or separated by `;`; `#` begins a comment): `name = value`; `name[where] = value`
//!   (the cells where the condition holds, the others keeping theirs); `name.layer = value` (a stack's layer
//!   replaced or added); `out name = value` and `out name`, the node's outputs (a map, a stack or a number). A
//!   script without `out` gives its last value as the map `map`.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Value};

/// A parsed expression.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Num(f64),
    Var(String),
    /// A layer of a stack: `stack.layer`.
    Layer(String, String),
    Unary(char, Box<Expr>),
    Binary(&'static str, Box<Expr>, Box<Expr>),
    If(Box<Expr>, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
    /// A number from all the cells of a value: `x.mean()`, `x.percentile(90)`.
    Method(Box<Expr>, String, Vec<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(&'static str),
    /// The end of a statement: a new line outside brackets, or `;`.
    End,
}

const OPS: [&str; 26] = [
    "**", "<=", ">=", "==", "!=", "&&", "||", "<", ">", "+", "-", "*", "/", "%", "^", "(", ")", "[", "]", ",", "?",
    ":", "!", ".", "=", ";",
];

/// The words of the language, which no value is named.
const WORDS: [&str; 12] = ["if", "then", "else", "and", "or", "not", "out", "pi", "null", "None", "True", "False"];

/// The tokens of a text and the line of each. In a script a new line (outside brackets) and `;` end a
/// statement; in an expression they are spaces.
fn lex(s: &str, script: bool) -> Result<(Vec<Tok>, Vec<usize>)> {
    let b = s.as_bytes();
    let (mut i, mut line, mut depth) = (0, 1, 0usize);
    let (mut out, mut lines) = (Vec::new(), Vec::new());
    // a dot right after a name or a bracket reads a layer, even before a digit (to be told how to name it)
    let attached = |i: usize| i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b')');
    while i < b.len() {
        let c = b[i] as char;
        if c == '\n' {
            if script && depth == 0 {
                out.push(Tok::End);
                lines.push(line);
            }
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
        } else if c.is_ascii_digit()
            || (c == '.' && i + 1 < b.len() && (b[i + 1] as char).is_ascii_digit() && !attached(i))
        {
            let start = i;
            while i < b.len() && ((b[i] as char).is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
                i += 1;
                if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
                    i += 1;
                }
                while i < b.len() && (b[i] as char).is_ascii_digit() {
                    i += 1;
                }
            }
            let t = &s[start..i];
            out.push(Tok::Num(t.parse().map_err(|_| Error::msg(format!("{t:?} is not a number")))?));
            lines.push(line);
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Tok::Id(s[start..i].to_string()));
            lines.push(line);
        } else {
            let op = OPS.iter().find(|o| s[i..].starts_with(*o)).ok_or_else(|| {
                let c = s[i..].chars().next().unwrap_or(c);
                Error::msg(format!("unexpected {c:?} at {}", i + 1))
            })?;
            match *op {
                "(" | "[" => depth += 1,
                ")" | "]" => depth = depth.saturating_sub(1),
                _ => {}
            }
            out.push(if *op == ";" && script { Tok::End } else { Tok::Op(op) });
            lines.push(line);
            i += op.len();
        }
    }
    Ok((out, lines))
}

struct Parser {
    toks: Vec<Tok>,
    lines: Vec<usize>,
    at: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }

    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if *o == op)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Id(x)) if x == w)
    }

    /// The end of a statement (or of the text).
    fn at_end(&self) -> bool {
        matches!(self.peek(), None | Some(Tok::End))
    }

    fn line(&self) -> usize {
        self.lines.get(self.at).or(self.lines.last()).copied().unwrap_or(1)
    }

    fn expect_op(&mut self, op: &str) -> Result<()> {
        if self.is_op(op) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.unexpected(&format!("expected {op:?}")))
        }
    }

    fn expect_word(&mut self, w: &str) -> Result<()> {
        if self.is_word(w) {
            self.at += 1;
            Ok(())
        } else {
            Err(self.unexpected(&format!("expected {w:?}")))
        }
    }

    /// What was expected and where; a `=` where a value goes is a comparison written with one sign.
    fn unexpected(&self, what: &str) -> Error {
        if self.is_op("=") {
            return Error::msg("use == to compare (= gives a name a value, at the start of a line)");
        }
        Error::msg(format!("{what} {}", self.here()))
    }

    fn here(&self) -> String {
        match self.peek() {
            None => "at the end".into(),
            Some(Tok::End) => "at the end of the line".into(),
            Some(Tok::Num(n)) => format!("before {n}"),
            Some(Tok::Id(x)) => format!("before {x:?}"),
            Some(Tok::Op(o)) => format!("before {o:?}"),
        }
    }

    fn expr(&mut self) -> Result<Expr> {
        if self.is_word("if") {
            self.at += 1;
            let c = self.expr()?;
            self.expect_word("then")?;
            let a = self.expr()?;
            self.expect_word("else")?;
            let b = self.expr()?;
            return Ok(Expr::If(Box::new(c), Box::new(a), Box::new(b)));
        }
        let a = self.or()?;
        if self.is_op("?") {
            self.at += 1;
            let x = self.expr()?;
            self.expect_op(":")?;
            let y = self.expr()?;
            return Ok(Expr::If(Box::new(a), Box::new(x), Box::new(y)));
        }
        // Python's: a if c else b
        if self.is_word("if") {
            self.at += 1;
            let c = self.or()?;
            self.expect_word("else")?;
            let b = self.expr()?;
            return Ok(Expr::If(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(a)
    }

    fn or(&mut self) -> Result<Expr> {
        let mut l = self.and()?;
        while self.is_op("||") || self.is_word("or") {
            self.at += 1;
            l = Expr::Binary("or", Box::new(l), Box::new(self.and()?));
        }
        Ok(l)
    }

    fn and(&mut self) -> Result<Expr> {
        let mut l = self.not()?;
        while self.is_op("&&") || self.is_word("and") {
            self.at += 1;
            l = Expr::Binary("and", Box::new(l), Box::new(self.not()?));
        }
        Ok(l)
    }

    fn not(&mut self) -> Result<Expr> {
        if self.is_op("!") || self.is_word("not") {
            self.at += 1;
            return Ok(Expr::Unary('!', Box::new(self.not()?)));
        }
        self.cmp()
    }

    /// A comparison, or a chain of them as in Python: `a < b <= c` is `a < b and b <= c`.
    fn cmp(&mut self) -> Result<Expr> {
        let mut left = self.add()?;
        let mut chain: Option<Expr> = None;
        while let Some(op) = ["<=", ">=", "==", "!=", "<", ">"].into_iter().find(|o| self.is_op(o)) {
            self.at += 1;
            let right = self.add()?;
            let c = Expr::Binary(op, Box::new(left), Box::new(right.clone()));
            chain = Some(match chain {
                None => c,
                Some(before) => Expr::Binary("and", Box::new(before), Box::new(c)),
            });
            left = right;
        }
        Ok(chain.unwrap_or(left))
    }

    fn add(&mut self) -> Result<Expr> {
        let mut l = self.mul()?;
        loop {
            let op = if self.is_op("+") {
                "+"
            } else if self.is_op("-") {
                "-"
            } else {
                return Ok(l);
            };
            self.at += 1;
            l = Expr::Binary(op, Box::new(l), Box::new(self.mul()?));
        }
    }

    fn mul(&mut self) -> Result<Expr> {
        let mut l = self.unary()?;
        loop {
            let op = match () {
                _ if self.is_op("*") => "*",
                _ if self.is_op("/") => "/",
                _ if self.is_op("%") => "%",
                _ => return Ok(l),
            };
            self.at += 1;
            l = Expr::Binary(op, Box::new(l), Box::new(self.unary()?));
        }
    }

    fn unary(&mut self) -> Result<Expr> {
        if self.is_op("-") {
            self.at += 1;
            return Ok(Expr::Unary('-', Box::new(self.unary()?)));
        }
        if self.is_op("+") {
            self.at += 1;
            return self.unary();
        }
        self.pow()
    }

    fn pow(&mut self) -> Result<Expr> {
        let base = self.postfix()?;
        if self.is_op("^") || self.is_op("**") {
            self.at += 1;
            return Ok(Expr::Binary("^", Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }

    /// A value and what follows it: a layer (`s.layer`), a method (`x.mean()`).
    fn postfix(&mut self) -> Result<Expr> {
        let mut e = self.atom()?;
        while self.is_op(".") {
            self.at += 1;
            let of = match &e {
                Expr::Var(v) => v.clone(),
                _ => "x".into(),
            };
            let name = match self.peek().cloned() {
                Some(Tok::Id(n)) => n,
                Some(Tok::Num(n)) => {
                    return Err(Error::msg(format!("a layer's name begins with a letter or _: {of}._{n}")));
                }
                _ => return Err(Error::msg(format!("expected a layer of {of:?} after the dot"))),
            };
            self.at += 1;
            if self.is_op("(") {
                let args = self.args()?;
                check_method(&name, &args)?;
                e = Expr::Method(Box::new(e), name, args);
            } else {
                e = match e {
                    Expr::Var(v) => Expr::Layer(v, name),
                    _ => return Err(Error::msg(format!("a layer is read from a stack's name: s.{name}"))),
                };
            }
        }
        Ok(e)
    }

    /// The values between brackets after a function or a method.
    fn args(&mut self) -> Result<Vec<Expr>> {
        self.expect_op("(")?;
        let mut args = Vec::new();
        if !self.is_op(")") {
            loop {
                args.push(self.expr()?);
                if self.is_op(",") {
                    self.at += 1;
                } else {
                    break;
                }
            }
        }
        self.expect_op(")")?;
        Ok(args)
    }

    fn atom(&mut self) -> Result<Expr> {
        let t = match self.peek().cloned() {
            None | Some(Tok::End) => return Err(Error::msg("the expression ends too early")),
            Some(t) => t,
        };
        self.at += 1;
        match t {
            Tok::Num(n) => Ok(Expr::Num(n)),
            Tok::Op("(") => {
                let e = self.expr()?;
                self.expect_op(")")?;
                Ok(e)
            }
            Tok::Id(id) => match id.as_str() {
                "pi" => Ok(Expr::Num(std::f64::consts::PI)),
                "null" | "None" => Ok(Expr::Num(f64::NAN)),
                "True" => Ok(Expr::Num(1.0)),
                "False" => Ok(Expr::Num(0.0)),
                "if" | "then" | "else" | "and" | "or" | "not" | "out" => Err(Error::msg(format!("unexpected {id:?}"))),
                _ if self.is_op("(") => {
                    let args = self.args()?;
                    check_call(&id, &args)?;
                    Ok(Expr::Call(id, args))
                }
                _ => Ok(Expr::Var(id)),
            },
            Tok::Op("=") => Err(Error::msg("use == to compare (= gives a name a value, at the start of a line)")),
            Tok::Op(o) => Err(Error::msg(format!("unexpected {o:?}"))),
            Tok::End => unreachable!(),
        }
    }

    /// A name a statement gives a value (not a word of the language).
    fn name(&mut self) -> Result<String> {
        match self.peek().cloned() {
            Some(Tok::Id(n)) if !WORDS.contains(&n.as_str()) => {
                self.at += 1;
                Ok(n)
            }
            Some(Tok::Id(n)) => Err(Error::msg(format!("{n:?} is a word of the language, not a name"))),
            _ => Err(self.unexpected("expected a name")),
        }
    }

    fn statement(&mut self) -> Result<Stmt> {
        if self.is_word("out") {
            self.at += 1;
            let name = self.name()?;
            if self.at_end() {
                return Ok(Stmt::Out(name));
            }
            self.expect_op("=")?;
            let value = self.expr()?;
            return Ok(Stmt::Set { name, layer: None, mask: None, value, out: true });
        }
        // name, name.layer, name[where], name.layer[where], then =
        let start = self.at;
        if let Some(Tok::Id(name)) = self.peek().cloned()
            && !WORDS.contains(&name.as_str())
        {
            self.at += 1;
            let mut layer = None;
            if self.is_op(".")
                && let Some(Tok::Id(l)) = self.toks.get(self.at + 1).cloned()
                && self.toks.get(self.at + 2) != Some(&Tok::Op("("))
            {
                self.at += 2;
                layer = Some(l);
            }
            let mut mask = None;
            if self.is_op("[") {
                self.at += 1;
                mask = Some(self.expr()?);
                self.expect_op("]")?;
            }
            if self.is_op("=") {
                self.at += 1;
                let value = self.expr()?;
                return Ok(Stmt::Set { name, layer, mask, value, out: false });
            }
            if mask.is_some() {
                return Err(self.unexpected("expected \"=\" (name[where] = value)"));
            }
            self.at = start;
        }
        Ok(Stmt::Value(self.expr()?))
    }
}

/// The functions: name, least and most values taken.
const FUNCTIONS: [(&str, usize, usize); 24] = [
    ("abs", 1, 1),
    ("sqrt", 1, 1),
    ("exp", 1, 1),
    ("ln", 1, 1),
    ("log", 1, 1),
    ("log10", 1, 1),
    ("floor", 1, 1),
    ("ceil", 1, 1),
    ("round", 1, 1),
    ("isnull", 1, 1),
    ("coalesce", 2, 2),
    ("clamp", 3, 3),
    ("nbsum", 2, 2),
    ("nbmean", 2, 2),
    ("nbmin", 2, 2),
    ("nbmax", 2, 2),
    ("nbcount", 2, 2),
    ("min", 1, usize::MAX),
    ("max", 1, usize::MAX),
    ("mean", 1, 1),
    ("sd", 1, 1),
    ("median", 1, 1),
    ("sum", 1, 1),
    ("percentile", 2, 2),
];

/// The functions of a stack's layers at each cell (and `count`, which is also a method).
const REDUCTIONS: [&str; 8] = ["mean", "sd", "median", "sum", "count", "min", "max", "percentile"];

/// The methods: name and values taken.
const METHODS: [(&str, usize); 8] =
    [("mean", 0), ("sd", 0), ("median", 0), ("sum", 0), ("count", 0), ("min", 0), ("max", 0), ("percentile", 1)];

fn check_call(name: &str, args: &[Expr]) -> Result<()> {
    if name == "count" {
        return if args.len() == 1 { Ok(()) } else { Err(Error::msg("count takes 1 value: a stack")) };
    }
    let (_, lo, hi) =
        FUNCTIONS.iter().find(|(f, _, _)| *f == name).ok_or_else(|| Error::msg(format!("no function {name:?}")))?;
    if args.len() < *lo || args.len() > *hi {
        return Err(Error::msg(if *hi == usize::MAX {
            format!("{name} takes a stack, or two values or more")
        } else {
            format!("{name} takes {lo} value(s), not {}", args.len())
        }));
    }
    if name.starts_with("nb") {
        // a whole number, or the name of a number input (its value checked when the expression runs)
        match &args[1] {
            Expr::Num(r) if *r >= 0.0 && r.fract() == 0.0 => {}
            Expr::Var(_) => {}
            _ => {
                return Err(Error::msg(format!("the radius of {name} is a whole number of cells, or a number input")));
            }
        }
    }
    Ok(())
}

fn check_method(name: &str, args: &[Expr]) -> Result<()> {
    let all = || METHODS.iter().map(|(m, _)| format!("{m}()")).collect::<Vec<_>>().join(", ");
    let (_, n) = METHODS
        .iter()
        .find(|(m, _)| *m == name)
        .ok_or_else(|| Error::msg(format!("no method {name:?} (the methods: {})", all())))?;
    if args.len() != *n {
        return Err(Error::msg(format!("{name}() takes {n} value(s), not {}", args.len())));
    }
    Ok(())
}

/// Parse an expression.
pub fn parse(text: &str) -> Result<Expr> {
    let (toks, lines) = lex(text, false)?;
    let mut p = Parser { toks, lines, at: 0 };
    let e = p.expr()?;
    if p.at < p.toks.len() {
        return Err(p.unexpected("unexpected text"));
    }
    Ok(e)
}

/// A statement of a script.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// `name = value`, `name.layer = value`, `name[where] = value`, `name.layer[where] = value`; with `out`
    /// (`out name = value`), the name is an output of the node.
    Set { name: String, layer: Option<String>, mask: Option<Expr>, value: Expr, out: bool },
    /// `out name`: the value of a name, an output of the node.
    Out(String),
    /// A value alone: the result of a script without `out`.
    Value(Expr),
}

/// A parsed script: its statements with their lines.
#[derive(Debug, Clone, PartialEq)]
pub struct Script {
    pub statements: Vec<(usize, Stmt)>,
    /// Whether it has more than one line (its errors then say which).
    lines: bool,
}

impl Script {
    /// The names of its outputs (`out`), in order.
    pub fn outputs(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for (_, s) in &self.statements {
            let n = match s {
                Stmt::Set { name, out: true, .. } | Stmt::Out(name) => name,
                _ => continue,
            };
            if !out.contains(n) {
                out.push(n.clone());
            }
        }
        out
    }

    fn at(&self, line: usize, e: Error) -> Error {
        if self.lines { Error::msg(format!("line {line}: {}", e.0)) } else { e }
    }
}

/// Parse a script (one expression is a script).
pub fn parse_script(text: &str) -> Result<Script> {
    let lines = text.trim().contains(['\n', ';']);
    let at = |line: usize, e: Error| if lines { Error::msg(format!("line {line}: {}", e.0)) } else { e };
    let (toks, ls) = lex(text, true).map_err(|e| at(1, e))?;
    let mut p = Parser { toks, lines: ls, at: 0 };
    let mut statements = Vec::new();
    loop {
        while p.peek() == Some(&Tok::End) {
            p.at += 1;
        }
        if p.peek().is_none() {
            break;
        }
        let line = p.line();
        let s = p.statement().map_err(|e| at(line, e))?;
        if !p.at_end() {
            return Err(at(line, p.unexpected("unexpected text")));
        }
        statements.push((line, s));
    }
    if statements.is_empty() {
        return Err(Error::msg("the expression is empty"));
    }
    Ok(Script { statements, lines })
}

impl Expr {
    /// The inputs the expression reads (ports; stacks for their layers).
    pub fn names(&self) -> Vec<String> {
        let mut out = Vec::new();
        self.walk(&mut |e| match e {
            Expr::Var(n) | Expr::Layer(n, _) if !out.contains(n) => out.push(n.clone()),
            _ => {}
        });
        out
    }

    /// The largest neighbourhood radius it uses (cells).
    pub fn radius(&self) -> usize {
        let mut r = 0;
        self.walk(&mut |e| {
            if let Expr::Call(f, a) = e
                && f.starts_with("nb")
            {
                // a radius given by an input: known only when it runs (at least 1 here)
                r = r.max(match a[1] {
                    Expr::Num(x) => x as usize,
                    _ => 1,
                });
            }
        });
        r
    }

    fn walk(&self, f: &mut dyn FnMut(&Expr)) {
        f(self);
        match self {
            Expr::Unary(_, a) => a.walk(f),
            Expr::Binary(_, a, b) => {
                a.walk(f);
                b.walk(f);
            }
            Expr::If(c, a, b) => {
                c.walk(f);
                a.walk(f);
                b.walk(f);
            }
            Expr::Call(_, args) => args.iter().for_each(|a| a.walk(f)),
            Expr::Method(x, _, args) => {
                x.walk(f);
                args.iter().for_each(|a| a.walk(f));
            }
            _ => {}
        }
    }
}

/// The names a script gives the layers of a stack: a layer's name when it is a name (letters, digits and `_`, not
/// beginning with a digit); else its characters other than those made `_`, and `_` before a first digit
/// (`1997` is `_1997`, `NDVI-2020` `NDVI_2020`); a band without a name (its number from 0) is `b1`, `b2`...; two
/// of one name, the second `_2`.
pub fn band_names(keys: &[Key]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for (i, k) in keys.iter().enumerate() {
        let raw = k.to_string();
        let mut name = if raw.is_empty() || (matches!(k, Key::Name(_)) && raw == i.to_string()) {
            format!("b{}", i + 1)
        } else {
            let s: String = raw.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
            if s.starts_with(|c: char| c.is_ascii_digit()) { format!("_{s}") } else { s }
        };
        if WORDS.contains(&name.as_str()) {
            name.push('_');
        }
        if out.contains(&name) {
            let k = (2..).find(|k| !out.contains(&format!("{name}_{k}"))).unwrap();
            name = format!("{name}_{k}");
        }
        out.push(name);
    }
    out
}

/// What an expression reads: maps, numbers and stacks (their layers in order, with their keys) by name; the grid's
/// size.
pub struct Scope<'a> {
    pub width: usize,
    pub height: usize,
    pub maps: BTreeMap<&'a str, &'a [f64]>,
    pub numbers: BTreeMap<&'a str, f64>,
    pub stacks: BTreeMap<&'a str, Vec<(Key, &'a [f64])>>,
}

fn truth(x: f64) -> f64 {
    if x.is_nan() { f64::NAN } else { (x != 0.0) as u8 as f64 }
}

/// Cells computed together: the values in between stay in the processor's cache.
const BLOCK: usize = 2048;

/// Fewer cells than this are computed in the calling thread.
const PARALLEL: usize = 4 * BLOCK;

/// An operand: a value of the block computed before, a map (an input or a neighbourhood computed before), a number.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Arg {
    Reg(usize),
    Map(usize),
    Num(f64),
}

impl Arg {
    fn key(self) -> (u8, u64) {
        match self {
            Arg::Reg(i) => (0, i as u64),
            Arg::Map(i) => (1, i as u64),
            Arg::Num(x) => (2, x.to_bits()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Op1 {
    Neg,
    Not,
    Abs,
    Sqrt,
    Exp,
    Ln,
    Log10,
    Floor,
    Ceil,
    Round,
    IsNull,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Op2 {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Lt,
    Le,
    Gt,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Min,
    Max,
    Coalesce,
    /// The smaller, a missing value left out (a stack's `min`).
    MinSkip,
    /// The larger, a missing value left out.
    MaxSkip,
    /// `clamp`'s floor: `lo` where `x < lo`.
    AtLeast,
    /// `clamp`'s ceiling: `hi` where `x > hi`.
    AtMost,
}

/// An instruction; the last field is the register it writes.
#[derive(Debug, Clone, Copy)]
enum Ins {
    One(Op1, Arg, usize),
    Two(Op2, Arg, Arg, usize),
    Pick(Arg, Arg, Arg, usize),
}

impl Ins {
    fn args(&self) -> [Option<Arg>; 3] {
        match *self {
            Ins::One(_, a, _) => [Some(a), None, None],
            Ins::Two(_, a, b, _) => [Some(a), Some(b), None],
            Ins::Pick(c, a, b, _) => [Some(c), Some(a), Some(b)],
        }
    }

    fn args_mut(&mut self) -> [Option<&mut Arg>; 3] {
        match self {
            Ins::One(_, a, _) => [Some(a), None, None],
            Ins::Two(_, a, b, _) => [Some(a), Some(b), None],
            Ins::Pick(c, a, b, _) => [Some(c), Some(a), Some(b)],
        }
    }

    fn dst(&mut self) -> &mut usize {
        match self {
            Ins::One(.., d) | Ins::Two(.., d) | Ins::Pick(.., d) => d,
        }
    }
}

fn op1(op: Op1, x: f64) -> f64 {
    match op {
        Op1::Neg => -x,
        Op1::Not => {
            if x.is_nan() {
                f64::NAN
            } else {
                (x == 0.0) as u8 as f64
            }
        }
        Op1::Abs => x.abs(),
        Op1::Sqrt => x.sqrt(),
        Op1::Exp => x.exp(),
        Op1::Ln => x.ln(),
        Op1::Log10 => x.log10(),
        Op1::Floor => x.floor(),
        Op1::Ceil => x.ceil(),
        Op1::Round => x.round(),
        Op1::IsNull => x.is_nan() as u8 as f64,
    }
}

fn op2(op: Op2, a: f64, b: f64) -> f64 {
    let cmp = |c: bool| if a.is_nan() || b.is_nan() { f64::NAN } else { c as u8 as f64 };
    match op {
        Op2::Add => a + b,
        Op2::Sub => a - b,
        Op2::Mul => a * b,
        Op2::Div => a / b,
        Op2::Rem => a % b,
        Op2::Pow => a.powf(b),
        Op2::Lt => cmp(a < b),
        Op2::Le => cmp(a <= b),
        Op2::Gt => cmp(a > b),
        Op2::Ge => cmp(a >= b),
        Op2::Eq => cmp(a == b),
        Op2::Ne => cmp(a != b),
        Op2::And => cmp(a != 0.0 && b != 0.0),
        Op2::Or => cmp(a != 0.0 || b != 0.0),
        Op2::Min | Op2::Max if a.is_nan() || b.is_nan() => f64::NAN,
        Op2::Min => a.min(b),
        Op2::Max => a.max(b),
        Op2::MinSkip => extreme::<false>(a, b),
        Op2::MaxSkip => extreme::<true>(a, b),
        Op2::Coalesce => {
            if a.is_nan() {
                b
            } else {
                a
            }
        }
        Op2::AtLeast => {
            if a < b {
                b
            } else {
                a
            }
        }
        Op2::AtMost => {
            if a > b {
                b
            } else {
                a
            }
        }
    }
}

fn pick(c: f64, a: f64, b: f64) -> f64 {
    match truth(c) {
        t if t.is_nan() => f64::NAN,
        t if t != 0.0 => a,
        _ => b,
    }
}

/// An operand over a block: its values, or one number for all.
#[derive(Clone, Copy)]
enum Src<'a> {
    Cells(&'a [f64]),
    One(f64),
}

fn each1(d: &mut [f64], a: Src, f: impl Fn(f64) -> f64) {
    match a {
        Src::Cells(a) => d.iter_mut().zip(a).for_each(|(d, a)| *d = f(*a)),
        Src::One(a) => d.fill(f(a)),
    }
}

fn each2(d: &mut [f64], a: Src, b: Src, f: impl Fn(f64, f64) -> f64) {
    match (a, b) {
        (Src::Cells(a), Src::Cells(b)) => d.iter_mut().zip(a).zip(b).for_each(|((d, a), b)| *d = f(*a, *b)),
        (Src::Cells(a), Src::One(b)) => d.iter_mut().zip(a).for_each(|(d, a)| *d = f(*a, b)),
        (Src::One(a), Src::Cells(b)) => d.iter_mut().zip(b).for_each(|(d, b)| *d = f(a, *b)),
        (Src::One(a), Src::One(b)) => d.fill(f(a, b)),
    }
}

fn each3(d: &mut [f64], c: Src, a: Src, b: Src) {
    let at = |s: Src, i: usize| match s {
        Src::Cells(v) => v[i],
        Src::One(x) => x,
    };
    match (c, a, b) {
        (Src::Cells(c), Src::Cells(a), Src::Cells(b)) => {
            d.iter_mut().zip(c).zip(a).zip(b).for_each(|(((d, c), a), b)| *d = pick(*c, *a, *b))
        }
        _ => d.iter_mut().enumerate().for_each(|(i, d)| *d = pick(at(c, i), at(a, i), at(b, i))),
    }
}

// one loop per operator, each compiled on its own (and vectorised where it can be)
fn apply1(op: Op1, d: &mut [f64], a: Src) {
    match op {
        Op1::Neg => each1(d, a, |x| op1(Op1::Neg, x)),
        Op1::Not => each1(d, a, |x| op1(Op1::Not, x)),
        Op1::Abs => each1(d, a, |x| op1(Op1::Abs, x)),
        Op1::Sqrt => each1(d, a, |x| op1(Op1::Sqrt, x)),
        Op1::Exp => each1(d, a, |x| op1(Op1::Exp, x)),
        Op1::Ln => each1(d, a, |x| op1(Op1::Ln, x)),
        Op1::Log10 => each1(d, a, |x| op1(Op1::Log10, x)),
        Op1::Floor => each1(d, a, |x| op1(Op1::Floor, x)),
        Op1::Ceil => each1(d, a, |x| op1(Op1::Ceil, x)),
        Op1::Round => each1(d, a, |x| op1(Op1::Round, x)),
        Op1::IsNull => each1(d, a, |x| op1(Op1::IsNull, x)),
    }
}

fn apply2(op: Op2, d: &mut [f64], a: Src, b: Src) {
    macro_rules! by {
        ($($o:ident),*) => {
            match op { $(Op2::$o => each2(d, a, b, |x, y| op2(Op2::$o, x, y)),)* }
        };
    }
    by!(Add, Sub, Mul, Div, Rem, Pow, Lt, Le, Gt, Ge, Eq, Ne, And, Or, Min, Max, MinSkip, MaxSkip, Coalesce, AtLeast, AtMost)
}

/// A compiled script: instructions run over a block of cells at a time, with the registers they write, and the
/// values it gives (each computed for every cell).
#[derive(Debug)]
struct Program {
    ins: Vec<Ins>,
    regs: usize,
    results: Vec<Arg>,
}

impl Program {
    /// The cells `start..start + len` of every result (`outs`, one block of each).
    fn block(&self, maps: &[&[f64]], start: usize, outs: &mut [&mut [f64]], regs: &mut [Vec<f64>]) {
        let Some(len) = outs.first().map(|o| o.len()) else { return };
        // a map is read in place, never copied
        fn src<'r>(a: Arg, regs: &'r [Vec<f64>], maps: &[&'r [f64]], start: usize, len: usize) -> Src<'r> {
            match a {
                Arg::Reg(r) => Src::Cells(&regs[r][..len]),
                Arg::Map(m) => Src::Cells(&maps[m][start..start + len]),
                Arg::Num(x) => Src::One(x),
            }
        }
        for ins in &self.ins {
            let dst = match *ins {
                Ins::One(.., d) | Ins::Two(.., d) | Ins::Pick(.., d) => d,
            };
            // the register written is never one read by the same instruction
            let mut d = std::mem::take(&mut regs[dst]);
            let r: &[Vec<f64>] = regs;
            match *ins {
                Ins::One(op, a, _) => apply1(op, &mut d[..len], src(a, r, maps, start, len)),
                Ins::Two(op, a, b, _) => {
                    apply2(op, &mut d[..len], src(a, r, maps, start, len), src(b, r, maps, start, len))
                }
                Ins::Pick(c, a, b, _) => each3(
                    &mut d[..len],
                    src(c, r, maps, start, len),
                    src(a, r, maps, start, len),
                    src(b, r, maps, start, len),
                ),
            }
            regs[dst] = d;
        }
        for (out, a) in outs.iter_mut().zip(&self.results) {
            match *a {
                Arg::Reg(r) => out.copy_from_slice(&regs[r][..len]),
                Arg::Map(m) => out.copy_from_slice(&maps[m][start..start + len]),
                Arg::Num(x) => out.fill(x),
            }
        }
    }

    /// Every cell of the grid of every result, block by block, the blocks in parallel.
    fn run(&self, maps: &[&[f64]], n: usize) -> Vec<Vec<f64>> {
        let mut outs: Vec<Vec<f64>> = self.results.iter().map(|_| vec![0.0; n]).collect();
        {
            // each block of every result together
            let mut blocks: Vec<Vec<&mut [f64]>> = (0..n.div_ceil(BLOCK)).map(|_| Vec::new()).collect();
            for o in outs.iter_mut() {
                for (b, part) in o.chunks_mut(BLOCK).enumerate() {
                    blocks[b].push(part);
                }
            }
            let regs = || vec![vec![0.0; BLOCK.min(n)]; self.regs];
            if n < PARALLEL {
                let mut r = regs();
                for (b, mut parts) in blocks.into_iter().enumerate() {
                    self.block(maps, b * BLOCK, &mut parts, &mut r);
                }
            } else {
                crate::threads::ensure();
                blocks
                    .into_par_iter()
                    .enumerate()
                    .for_each_init(regs, |r, (b, mut parts)| self.block(maps, b * BLOCK, &mut parts, r));
            }
        }
        outs
    }
}

/// A map read by an expression: an input's, or one computed while compiling (a neighbourhood).
enum Slot<'a> {
    Input(&'a [f64]),
    Made(Vec<f64>),
}

impl Slot<'_> {
    fn cells(&self) -> &[f64] {
        match self {
            Slot::Input(v) => v,
            Slot::Made(v) => v,
        }
    }
}

/// A layer of a stack while compiling: its name in a script, its key in the stack made, its cells.
#[derive(Debug, Clone)]
struct Layer {
    name: String,
    key: Key,
    arg: Arg,
}

/// A value while compiling: one value a cell (a map, or a number for all), or the layers of a stack.
#[derive(Debug, Clone)]
enum Val {
    One(Arg),
    Layers(Vec<Layer>),
}

/// A value a script gives.
#[derive(Debug, Clone)]
pub enum Out {
    Number(f64),
    Map(Vec<f64>),
    Stack(Vec<(Key, Vec<f64>)>),
}

/// Turns a script into one program: names resolved, numbers folded, a value computed twice computed once; the
/// neighbourhoods, the statistics of a whole map and the percentiles of stacks computed when they are met, from
/// the cells of what they read.
struct Compiler<'s, 'a> {
    scope: &'s Scope<'a>,
    maps: Vec<Slot<'a>>,
    /// The maps of the inputs by name (`x`, `s.layer`).
    named: HashMap<String, usize>,
    /// What was computed whole (a neighbourhood, a statistic) by what it is.
    made: HashMap<String, Arg>,
    ins: Vec<Ins>,
    seen: HashMap<(u8, u8, [(u8, u64); 3]), Arg>,
    /// The names the script gave values.
    vars: HashMap<String, Val>,
}

impl<'s, 'a> Compiler<'s, 'a> {
    fn new(scope: &'s Scope<'a>) -> Self {
        Compiler {
            scope,
            maps: Vec::new(),
            named: HashMap::new(),
            made: HashMap::new(),
            ins: Vec::new(),
            seen: HashMap::new(),
            vars: HashMap::new(),
        }
    }

    fn cells(&self) -> usize {
        self.scope.width * self.scope.height
    }

    fn input(&mut self, key: String, cells: &'a [f64]) -> Arg {
        if let Some(&m) = self.named.get(&key) {
            return Arg::Map(m);
        }
        self.maps.push(Slot::Input(cells));
        self.named.insert(key, self.maps.len() - 1);
        Arg::Map(self.maps.len() - 1)
    }

    fn made(&mut self, cells: Vec<f64>) -> Arg {
        self.maps.push(Slot::Made(cells));
        Arg::Map(self.maps.len() - 1)
    }

    /// The map of a value at every cell, computed now if it is not one already.
    fn whole(&mut self, a: Arg) -> usize {
        match a {
            Arg::Map(m) => m,
            Arg::Num(x) => {
                let n = self.cells();
                let Arg::Map(m) = self.made(vec![x; n]) else { unreachable!() };
                m
            }
            Arg::Reg(_) => {
                let p = allocate(self.ins.clone(), &[a]);
                let cells = {
                    let maps: Vec<&[f64]> = self.maps.iter().map(Slot::cells).collect();
                    p.run(&maps, self.cells()).pop().unwrap()
                };
                let Arg::Map(m) = self.made(cells) else { unreachable!() };
                m
            }
        }
    }

    fn emit(&mut self, kind: u8, op: u8, args: [Option<Arg>; 3], ins: impl FnOnce(usize) -> Ins) -> Arg {
        let key = (kind, op, args.map(|a| a.map_or((3, 0), Arg::key)));
        if let Some(&a) = self.seen.get(&key) {
            return a;
        }
        let r = self.ins.len();
        self.ins.push(ins(r));
        self.seen.insert(key, Arg::Reg(r));
        Arg::Reg(r)
    }

    fn one(&mut self, op: Op1, a: Arg) -> Arg {
        match a {
            Arg::Num(x) => Arg::Num(op1(op, x)),
            a => self.emit(0, op as u8, [Some(a), None, None], |r| Ins::One(op, a, r)),
        }
    }

    fn two(&mut self, op: Op2, a: Arg, b: Arg) -> Arg {
        match (a, b) {
            (Arg::Num(x), Arg::Num(y)) => Arg::Num(op2(op, x, y)),
            _ => self.emit(1, op as u8, [Some(a), Some(b), None], |r| Ins::Two(op, a, b, r)),
        }
    }

    fn pick(&mut self, c: Arg, a: Arg, b: Arg) -> Arg {
        match (c, a, b) {
            (Arg::Num(c), Arg::Num(a), Arg::Num(b)) => Arg::Num(pick(c, a, b)),
            (Arg::Num(c), a, b) => match truth(c) {
                t if t.is_nan() => Arg::Num(f64::NAN),
                t if t != 0.0 => a,
                _ => b,
            },
            _ => self.emit(2, 0, [Some(c), Some(a), Some(b)], |r| Ins::Pick(c, a, b, r)),
        }
    }

    /// An operation on values made cell by cell: on each layer of the stacks among them (with the maps and
    /// numbers; layer by layer between stacks), named as the first stack's.
    fn each(&mut self, vals: &[Val], mut f: impl FnMut(&mut Self, &[Arg]) -> Arg) -> Result<Val> {
        let first = vals.iter().find_map(|v| match v {
            Val::Layers(l) => Some(l.clone()),
            Val::One(_) => None,
        });
        let Some(first) = first else {
            let args: Vec<Arg> = vals.iter().map(|v| if let Val::One(a) = v { *a } else { unreachable!() }).collect();
            return Ok(Val::One(f(self, &args)));
        };
        for v in vals {
            if let Val::Layers(l) = v
                && l.len() != first.len()
            {
                return Err(Error::msg(format!(
                    "a stack of {} layers and one of {}: between stacks, an operation is made layer by layer",
                    first.len(),
                    l.len()
                )));
            }
        }
        let mut out = Vec::with_capacity(first.len());
        for (i, layer) in first.into_iter().enumerate() {
            let args: Vec<Arg> = vals
                .iter()
                .map(|v| match v {
                    Val::One(a) => *a,
                    Val::Layers(l) => l[i].arg,
                })
                .collect();
            out.push(Layer { arg: f(self, &args), ..layer });
        }
        Ok(Val::Layers(out))
    }

    /// The value of a name: one the script gave, else an input.
    fn named_value(&mut self, name: &str) -> Result<Val> {
        if let Some(v) = self.vars.get(name) {
            return Ok(v.clone());
        }
        if let Some(&m) = self.scope.maps.get(name) {
            return Ok(Val::One(self.input(name.to_string(), m)));
        }
        if let Some(&x) = self.scope.numbers.get(name) {
            return Ok(Val::One(Arg::Num(x)));
        }
        if let Some(layers) = self.scope.stacks.get(name) {
            let keys: Vec<Key> = layers.iter().map(|(k, _)| k.clone()).collect();
            let names = band_names(&keys);
            let layers = layers.clone();
            let out = names
                .into_iter()
                .zip(layers)
                .map(|(n, (key, cells))| Layer { arg: self.input(format!("{name}.{n}"), cells), name: n, key })
                .collect();
            return Ok(Val::Layers(out));
        }
        Err(Error::msg(format!("no input {name:?} (a map, a stack or a number)")))
    }

    fn lower(&mut self, e: &Expr) -> Result<Val> {
        Ok(match e {
            Expr::Num(x) => Val::One(Arg::Num(*x)),
            Expr::Var(name) => self.named_value(name)?,
            Expr::Layer(stack, layer) => {
                let known = self.vars.contains_key(stack.as_str())
                    || self.scope.maps.contains_key(stack.as_str())
                    || self.scope.stacks.contains_key(stack.as_str());
                if !known {
                    return Err(Error::msg(format!("no layer {layer:?} in a stack {stack:?}")));
                }
                match self.named_value(stack)? {
                    Val::Layers(l) => match l.iter().find(|x| x.name == *layer) {
                        Some(x) => Val::One(x.arg),
                        None => {
                            let names: Vec<&str> = l.iter().map(|x| x.name.as_str()).collect();
                            return Err(Error::msg(format!(
                                "no layer {layer:?} in a stack {stack:?} (its layers: {})",
                                names.join(", ")
                            )));
                        }
                    },
                    Val::One(_) => return Err(Error::msg(format!("{stack} is a map, not a stack: it has no layers"))),
                }
            }
            Expr::Unary(op, a) => {
                let a = self.lower(a)?;
                let op = if *op == '-' { Op1::Neg } else { Op1::Not };
                self.each(&[a], |c, x| c.one(op, x[0]))?
            }
            Expr::Binary(op, a, b) => {
                let op = match *op {
                    "+" => Op2::Add,
                    "-" => Op2::Sub,
                    "*" => Op2::Mul,
                    "/" => Op2::Div,
                    "%" => Op2::Rem,
                    "^" => Op2::Pow,
                    "<" => Op2::Lt,
                    "<=" => Op2::Le,
                    ">" => Op2::Gt,
                    ">=" => Op2::Ge,
                    "==" => Op2::Eq,
                    "!=" => Op2::Ne,
                    "and" => Op2::And,
                    "or" => Op2::Or,
                    _ => unreachable!(),
                };
                let (a, b) = (self.lower(a)?, self.lower(b)?);
                self.each(&[a, b], |c, x| c.two(op, x[0], x[1]))?
            }
            Expr::If(c, a, b) => {
                let (c, a, b) = (self.lower(c)?, self.lower(a)?, self.lower(b)?);
                self.each(&[c, a, b], |s, x| s.pick(x[0], x[1], x[2]))?
            }
            Expr::Call(f, args) if f.starts_with("nb") => self.neighbourhood(f, args)?,
            Expr::Call(f, args) => {
                let v = args.iter().map(|a| self.lower(a)).collect::<Result<Vec<Val>>>()?;
                if REDUCTIONS.contains(&f.as_str()) && (v.len() == 1 || f == "percentile") {
                    return self.over_layers(f, &v);
                }
                match f.as_str() {
                    "min" | "max" => {
                        let op = if f == "max" { Op2::Max } else { Op2::Min };
                        self.each(&v, |c, x| x[1..].iter().fold(x[0], |acc, b| c.two(op, acc, *b)))?
                    }
                    "coalesce" => self.each(&v, |c, x| c.two(Op2::Coalesce, x[0], x[1]))?,
                    "clamp" => self.each(&v, |c, x| {
                        let lo = c.two(Op2::AtLeast, x[0], x[1]);
                        c.two(Op2::AtMost, lo, x[2])
                    })?,
                    _ => {
                        let op = match f.as_str() {
                            "abs" => Op1::Abs,
                            "sqrt" => Op1::Sqrt,
                            "exp" => Op1::Exp,
                            "ln" | "log" => Op1::Ln,
                            "log10" => Op1::Log10,
                            "floor" => Op1::Floor,
                            "ceil" => Op1::Ceil,
                            "round" => Op1::Round,
                            "isnull" => Op1::IsNull,
                            _ => unreachable!(),
                        };
                        self.each(&v, |c, x| c.one(op, x[0]))?
                    }
                }
            }
            Expr::Method(x, name, args) => {
                let v = self.lower(x)?;
                let p = match args.first() {
                    None => 0.0,
                    Some(a) => match self.lower(a)? {
                        Val::One(Arg::Num(p)) => p,
                        _ => return Err(Error::msg(format!("the percentile of {name}() is a number"))),
                    },
                };
                Val::One(Arg::Num(self.statistic(name, p, &v)?))
            }
        })
    }

    /// A neighbourhood, of a map or of each layer of a stack.
    fn neighbourhood(&mut self, f: &str, args: &[Expr]) -> Result<Val> {
        let r = match &args[1] {
            Expr::Num(r) => *r as usize,
            Expr::Var(n) => match self.scope.numbers.get(n.as_str()) {
                Some(&x) if x >= 0.0 && x.fract() == 0.0 => x as usize,
                Some(&x) => return Err(Error::msg(format!("the radius of {f} ({n} = {x}) is not a whole number"))),
                None => return Err(Error::msg(format!("the radius of {f} ({n}) is not a number input"))),
            },
            _ => unreachable!(),
        };
        let v = self.lower(&args[0])?;
        let f = f.to_string();
        self.each(&[v], |c, x| {
            let key = format!("{f}({:?}, {r})", x[0].key());
            if let Some(&a) = c.made.get(&key) {
                return a;
            }
            let m = c.whole(x[0]);
            let (w, h) = (c.scope.width, c.scope.height);
            let cells = c.maps[m].cells();
            let out = match f.as_str() {
                "nbmin" => box_extreme::<false>(cells, w, h, r),
                "nbmax" => box_extreme::<true>(cells, w, h, r),
                _ => box_sums(cells, w, h, r, &f),
            };
            let a = c.made(out);
            c.made.insert(key, a);
            a
        })
    }

    /// The value of each cell over the layers of a stack, missing values left out.
    fn over_layers(&mut self, f: &str, v: &[Val]) -> Result<Val> {
        let Val::Layers(layers) = &v[0] else {
            let m = if f == "percentile" { "x.percentile(p)".to_string() } else { format!("x.{f}()") };
            return Err(Error::msg(format!(
                "{f} takes a stack (the value of each cell over its layers); of the cells of a map, it is {m}"
            )));
        };
        let args: Vec<Arg> = layers.iter().map(|l| l.arg).collect();
        if args.is_empty() {
            return Ok(Val::One(Arg::Num(f64::NAN)));
        }
        // how many layers have a value, and their sum
        let mut count = Arg::Num(0.0);
        let mut sum = Arg::Num(0.0);
        if matches!(f, "mean" | "sd" | "sum" | "count") {
            for &a in &args {
                let null = self.one(Op1::IsNull, a);
                let has = self.one(Op1::Not, null);
                count = self.two(Op2::Add, count, has);
                let value = self.two(Op2::Coalesce, a, Arg::Num(0.0));
                sum = self.two(Op2::Add, sum, value);
            }
        }
        let any = self.two(Op2::Gt, count, Arg::Num(0.0));
        Ok(Val::One(match f {
            "count" => count,
            "sum" => self.pick(any, sum, Arg::Num(f64::NAN)),
            // 0 / 0 where no layer has a value
            "mean" => self.two(Op2::Div, sum, count),
            "sd" => {
                let mean = self.two(Op2::Div, sum, count);
                let mut squares = Arg::Num(0.0);
                for &a in &args {
                    let d = self.two(Op2::Sub, a, mean);
                    let d2 = self.two(Op2::Mul, d, d);
                    let d2 = self.two(Op2::Coalesce, d2, Arg::Num(0.0));
                    squares = self.two(Op2::Add, squares, d2);
                }
                let n1 = self.two(Op2::Sub, count, Arg::Num(1.0));
                let var = self.two(Op2::Div, squares, n1);
                let sd = self.one(Op1::Sqrt, var);
                let two = self.two(Op2::Gt, count, Arg::Num(1.0));
                self.pick(two, sd, Arg::Num(f64::NAN))
            }
            "min" | "max" => {
                let op = if f == "max" { Op2::MaxSkip } else { Op2::MinSkip };
                args[1..].iter().fold(args[0], |acc, b| self.two(op, acc, *b))
            }
            _ => {
                let p = match (f, v.get(1)) {
                    ("median", _) => 50.0,
                    (_, Some(Val::One(Arg::Num(p)))) => *p,
                    _ => return Err(Error::msg("the percentile of percentile(s, p) is a number")),
                };
                if !(0.0..=100.0).contains(&p) {
                    return Err(Error::msg(format!("a percentile is between 0 and 100, not {p}")));
                }
                let key = format!("percentile({:?}, {p})", args.iter().map(|a| a.key()).collect::<Vec<_>>());
                if let Some(&a) = self.made.get(&key) {
                    return Ok(Val::One(a));
                }
                let slots: Vec<usize> = args.iter().map(|a| self.whole(*a)).collect();
                let cells = {
                    let layers: Vec<&[f64]> = slots.iter().map(|m| self.maps[*m].cells()).collect();
                    cell_percentiles(&layers, self.cells(), p)
                };
                let a = self.made(cells);
                self.made.insert(key, a);
                a
            }
        }))
    }

    /// A number from every cell with a value of a map, or of every layer of a stack.
    fn statistic(&mut self, name: &str, p: f64, v: &Val) -> Result<f64> {
        let args: Vec<Arg> = match v {
            Val::One(Arg::Num(_)) => return Err(Error::msg(format!("{name}() is of a map or a stack, not a number"))),
            Val::One(a) => vec![*a],
            Val::Layers(l) => l.iter().map(|x| x.arg).collect(),
        };
        if name == "percentile" && !(0.0..=100.0).contains(&p) {
            return Err(Error::msg(format!("a percentile is between 0 and 100, not {p}")));
        }
        let key = format!("{name}({:?}, {p})", args.iter().map(|a| a.key()).collect::<Vec<_>>());
        if let Some(Arg::Num(x)) = self.made.get(&key) {
            return Ok(*x);
        }
        let slots: Vec<usize> = args.iter().map(|a| self.whole(*a)).collect();
        let x = {
            let parts: Vec<&[f64]> = slots.iter().map(|m| self.maps[*m].cells()).collect();
            statistic(name, p, &parts)
        };
        self.made.insert(key, Arg::Num(x));
        Ok(x)
    }

    fn statement(&mut self, s: &Stmt, last: bool, outs: bool) -> Result<()> {
        match s {
            Stmt::Value(_) if !last || outs => Err(Error::msg(
                "a value computed and not kept: give it a name (x = ...), or compare with ==",
            )),
            Stmt::Value(_) => Ok(()),
            Stmt::Out(name) => self.named_value(name).map(|_| ()),
            Stmt::Set { name, layer, mask, value, .. } => {
                let value = self.lower(value)?;
                let new = match layer {
                    None => match mask {
                        None => value,
                        Some(m) => {
                            let old = self.named_value(name)?;
                            let c = self.lower(m)?;
                            self.masked(name, old, c, value)?
                        }
                    },
                    Some(l) => {
                        let Val::Layers(mut layers) = self.named_value(name)? else {
                            return Err(Error::msg(format!(
                                "{name} is a map: {name}.{l} = ... sets a layer of a stack"
                            )));
                        };
                        let at = layers.iter().position(|x| x.name == *l);
                        let value = match mask {
                            None => value,
                            Some(m) => {
                                let Some(at) = at else {
                                    return Err(Error::msg(format!("no layer {l:?} in a stack {name:?}")));
                                };
                                let c = self.lower(m)?;
                                self.masked(name, Val::One(layers[at].arg), c, value)?
                            }
                        };
                        let Val::One(arg) = value else {
                            return Err(Error::msg(format!("{name}.{l} is one layer: its value is a map, not a stack")));
                        };
                        match at {
                            Some(at) => layers[at].arg = arg,
                            None => layers.push(Layer { name: l.clone(), key: Key::Name(l.clone()), arg }),
                        }
                        Val::Layers(layers)
                    }
                };
                self.vars.insert(name.clone(), new);
                Ok(())
            }
        }
    }

    /// `old[c] = value`: the value where the condition holds, the old value elsewhere (and where the condition is
    /// missing).
    fn masked(&mut self, name: &str, old: Val, c: Val, value: Val) -> Result<Val> {
        if matches!(old, Val::One(_)) && (matches!(c, Val::Layers(_)) || matches!(value, Val::Layers(_))) {
            return Err(Error::msg(format!(
                "{name} is a map: the condition and the value of {name}[...] = ... are maps or numbers"
            )));
        }
        self.each(&[old, c, value], |s, x| {
            let holds = s.two(Op2::Coalesce, x[1], Arg::Num(0.0));
            s.pick(holds, x[2], x[0])
        })
    }
}

/// The instructions the results need, with the fewest registers: a value's register is reused after its last
/// reading (never by the instruction reading it).
fn allocate(ins: Vec<Ins>, results: &[Arg]) -> Program {
    let n = ins.len();
    let reads = |x: &Ins| x.args().into_iter().flatten().filter_map(|a| if let Arg::Reg(r) = a { Some(r) } else { None });
    let mut needed = vec![false; n];
    let mut kept = vec![false; n];
    for a in results {
        if let Arg::Reg(r) = *a {
            needed[r] = true;
            kept[r] = true;
        }
    }
    for i in (0..n).rev() {
        if needed[i] {
            for r in reads(&ins[i]) {
                needed[r] = true;
            }
        }
    }
    let mut last = vec![usize::MAX; n];
    for (i, x) in ins.iter().enumerate().filter(|(i, _)| needed[*i]) {
        for r in reads(x) {
            last[r] = i;
        }
    }
    let (mut reg, mut free, mut regs, mut out) = (vec![usize::MAX; n], Vec::<usize>::new(), 0, Vec::new());
    for (i, mut x) in ins.into_iter().enumerate() {
        if !needed[i] {
            continue;
        }
        let read: Vec<usize> = reads(&x).collect();
        for a in x.args_mut().into_iter().flatten() {
            if let Arg::Reg(r) = a {
                *r = reg[*r];
            }
        }
        let d = free.pop().unwrap_or_else(|| {
            regs += 1;
            regs - 1
        });
        reg[i] = d;
        *x.dst() = d;
        out.push(x);
        // the values read for the last time: their registers free for the next instructions
        for r in read {
            if last[r] == i && !kept[r] && !free.contains(&reg[r]) {
                free.push(reg[r]);
            }
        }
    }
    let results = results
        .iter()
        .map(|a| match *a {
            Arg::Reg(r) => Arg::Reg(reg[r]),
            a => a,
        })
        .collect();
    Program { ins: out, regs, results }
}

/// The value at the percentile `p` (0 to 100) of sorted values, between the two nearest (numpy's and R's default).
fn at_percentile(sorted: &[f64], p: f64) -> f64 {
    let Some(&first) = sorted.first() else { return f64::NAN };
    let h = (sorted.len() - 1) as f64 * p / 100.0;
    let lo = h.floor() as usize;
    let frac = h - lo as f64;
    if frac == 0.0 || lo + 1 >= sorted.len() {
        return if lo < sorted.len() { sorted[lo] } else { first };
    }
    sorted[lo] + frac * (sorted[lo + 1] - sorted[lo])
}

/// The percentile `p` of each cell over the layers, missing values left out.
fn cell_percentiles(layers: &[&[f64]], n: usize, p: f64) -> Vec<f64> {
    let mut out = vec![0.0; n];
    crate::threads::ensure();
    out.par_chunks_mut(BLOCK).enumerate().for_each_init(Vec::new, |buf: &mut Vec<f64>, (b, o)| {
        for (i, o) in o.iter_mut().enumerate() {
            let at = b * BLOCK + i;
            buf.clear();
            buf.extend(layers.iter().map(|l| l[at]).filter(|x| !x.is_nan()));
            buf.sort_unstable_by(f64::total_cmp);
            *o = at_percentile(buf, p);
        }
    });
    out
}

/// A statistic of the cells with a value of maps: the same to the bit whatever the threads (the sums of fixed
/// parts added in order).
fn statistic(name: &str, p: f64, parts: &[&[f64]]) -> f64 {
    const PART: usize = 1 << 16;
    crate::threads::ensure();
    // each part's (count, sum, sum of squared differences from `m`, min, max)
    let pass = |m: f64| -> (f64, f64, f64, f64, f64) {
        let mut all = (0.0, 0.0, 0.0, f64::NAN, f64::NAN);
        for v in parts {
            let each: Vec<(f64, f64, f64, f64, f64)> = v
                .par_chunks(PART)
                .map(|c| {
                    let mut t = (0.0, 0.0, 0.0, f64::NAN, f64::NAN);
                    for &x in c.iter().filter(|x| !x.is_nan()) {
                        t.0 += 1.0;
                        t.1 += x;
                        t.2 += (x - m) * (x - m);
                        t.3 = extreme::<false>(t.3, x);
                        t.4 = extreme::<true>(t.4, x);
                    }
                    t
                })
                .collect();
            for t in each {
                all = (all.0 + t.0, all.1 + t.1, all.2 + t.2, extreme::<false>(all.3, t.3), extreme::<true>(all.4, t.4));
            }
        }
        all
    };
    match name {
        "median" | "percentile" => {
            let mut v: Vec<f64> = parts.iter().flat_map(|part| part.iter().copied().filter(|x| !x.is_nan())).collect();
            v.par_sort_unstable_by(f64::total_cmp);
            at_percentile(&v, if name == "median" { 50.0 } else { p })
        }
        _ => {
            let (count, sum, _, min, max) = pass(0.0);
            match name {
                "count" => count,
                "sum" if count > 0.0 => sum,
                "mean" => sum / count,
                "min" => min,
                "max" => max,
                "sd" if count > 1.0 => (pass(sum / count).2 / (count - 1.0)).sqrt(),
                _ => f64::NAN,
            }
        }
    }
}

/// The sums (`nbsum`), means (`nbmean`) or counts (`nbcount`) of the values (missing ones left out) in the square
/// of radius `r` around each cell, from summed-area tables.
fn box_sums(v: &[f64], w: usize, h: usize, r: usize, f: &str) -> Vec<f64> {
    let (w1, h1) = (w + 1, h + 1);
    let mut s = vec![0.0f64; w1 * h1];
    let mut c = vec![0.0f64; w1 * h1];
    if w * h == 0 {
        return Vec::new();
    }
    crate::threads::ensure();
    // each row's running sums, then down the columns: the additions of a cell in the same order as one pass
    s[w1..].par_chunks_mut(w1).zip(c[w1..].par_chunks_mut(w1)).enumerate().for_each(|(y, (s, c))| {
        let (mut rs, mut rc) = (0.0, 0.0);
        for x in 0..w {
            let x_ = v[y * w + x];
            if !x_.is_nan() {
                rs += x_;
                rc += 1.0;
            }
            s[x + 1] = rs;
            c[x + 1] = rc;
        }
    });
    for y in 1..h1 {
        let (above, row) = s.split_at_mut(y * w1);
        row[..w1].iter_mut().zip(&above[(y - 1) * w1..]).for_each(|(a, b)| *a = *b + *a);
        let (above, row) = c.split_at_mut(y * w1);
        row[..w1].iter_mut().zip(&above[(y - 1) * w1..]).for_each(|(a, b)| *a = *b + *a);
    }
    let mut out = vec![0.0; w * h];
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
        for (x, o) in row.iter_mut().enumerate() {
            let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
            let at = |t: &[f64]| t[y1 * w1 + x1] - t[y0 * w1 + x1] - t[y1 * w1 + x0] + t[y0 * w1 + x0];
            let (sum, count) = (at(&s), at(&c));
            *o = match f {
                "nbsum" if count > 0.0 => sum,
                "nbmean" if count > 0.0 => sum / count,
                "nbcount" => count,
                _ => f64::NAN,
            };
        }
    });
    out
}

/// The larger (`MAX`) or smaller of two values, a missing one left out.
#[inline]
fn extreme<const MAX: bool>(a: f64, b: f64) -> f64 {
    if a.is_nan() || (!b.is_nan() && if MAX { b > a } else { b < a }) { b } else { a }
}

/// The extreme of the window of `2r + 1` values around each value of a line, missing values left out, by van
/// Herk and Gil-Werman: three comparisons a value, whatever the radius. `g` and `s` are room for the work.
fn line_extreme<const MAX: bool>(src: &[f64], r: usize, dst: &mut [f64], g: &mut Vec<f64>, s: &mut Vec<f64>) {
    let len = src.len();
    if len == 0 {
        return;
    }
    // a window as long as the line covers it all from every cell
    let r = r.min(len - 1);
    let (k, p) = (2 * r + 1, len + 2 * r);
    // the line between r missing values on each side, cut into blocks of k
    let at = |j: usize| if j >= r && j < r + len { src[j - r] } else { f64::NAN };
    g.resize(p, 0.0);
    s.resize(p, 0.0);
    for j in 0..p {
        g[j] = if j % k == 0 { at(j) } else { extreme::<MAX>(g[j - 1], at(j)) };
    }
    for j in (0..p).rev() {
        s[j] = if j % k == k - 1 || j == p - 1 { at(j) } else { extreme::<MAX>(s[j + 1], at(j)) };
    }
    for (i, d) in dst.iter_mut().enumerate() {
        *d = extreme::<MAX>(s[i], g[i + 2 * r]);
    }
}

/// The extreme along every row of a `w` by `h` grid, the rows in parallel.
fn rows_extreme<const MAX: bool>(v: &[f64], w: usize, r: usize) -> Vec<f64> {
    let mut out = vec![0.0; v.len()];
    out.par_chunks_mut(w).zip(v.par_chunks(w)).for_each_init(
        || (Vec::new(), Vec::new()),
        |(g, s), (o, row)| line_extreme::<MAX>(row, r, o, g, s),
    );
    out
}

/// The `w` by `h` grid turned on its side (`h` by `w`), by tiles that stay in the cache.
fn transpose(v: &[f64], w: usize, h: usize) -> Vec<f64> {
    const T: usize = 32;
    let mut out = vec![0.0; v.len()];
    // a band of T rows of the result is T columns of the grid
    out.par_chunks_mut(T * h).enumerate().for_each(|(b, band)| {
        let x0 = b * T;
        let x1 = (x0 + T).min(w);
        for y0 in (0..h).step_by(T) {
            for y in y0..(y0 + T).min(h) {
                for x in x0..x1 {
                    band[(x - x0) * h + y] = v[y * w + x];
                }
            }
        }
    });
    out
}

/// The largest (`MAX`) or smallest value (missing ones left out) in the square of radius `r` around each cell:
/// along the rows, then along the columns.
fn box_extreme<const MAX: bool>(v: &[f64], w: usize, h: usize, r: usize) -> Vec<f64> {
    if w * h == 0 {
        return Vec::new();
    }
    crate::threads::ensure();
    let rows = rows_extreme::<MAX>(v, w, r);
    let cols = rows_extreme::<MAX>(&transpose(&rows, w, h), h, r);
    transpose(&cols, h, w)
}


impl Expr {
    /// The value of every cell of the grid.
    pub fn evaluate(&self, s: &Scope) -> Result<Vec<f64>> {
        let mut c = Compiler::new(s);
        let a = match c.lower(self)? {
            Val::One(a) => a,
            Val::Layers(_) => return Err(Error::msg("the expression gives a stack, not a map")),
        };
        let p = allocate(std::mem::take(&mut c.ins), &[a]);
        let maps: Vec<&[f64]> = c.maps.iter().map(Slot::cells).collect();
        Ok(p.run(&maps, s.width * s.height).pop().unwrap())
    }
}

impl Script {
    /// What the script gives: its outputs by name (without `out`, its last value as `map`), every map of them
    /// computed in one pass over the cells.
    pub fn run(&self, s: &Scope) -> Result<Vec<(String, Out)>> {
        let mut c = Compiler::new(s);
        let outs = self.outputs();
        let n = self.statements.len();
        let mut last = None;
        for (i, (line, st)) in self.statements.iter().enumerate() {
            c.statement(st, i + 1 == n, !outs.is_empty()).map_err(|e| self.at(*line, e))?;
            if i + 1 == n && outs.is_empty() {
                last = Some(match st {
                    Stmt::Value(e) => c.lower(e).map_err(|e| self.at(*line, e))?,
                    Stmt::Set { name, .. } | Stmt::Out(name) => c.named_value(name)?,
                });
            }
        }
        let vals: Vec<(String, Val)> = match last {
            Some(v) => vec![("map".to_string(), v)],
            None => outs.iter().map(|o| Ok((o.clone(), c.named_value(o)?))).collect::<Result<_>>()?,
        };
        let mut results = Vec::new();
        for (_, v) in &vals {
            match v {
                Val::One(Arg::Num(_)) => {}
                Val::One(a) => results.push(*a),
                Val::Layers(l) => results.extend(l.iter().map(|x| x.arg)),
            }
        }
        let p = allocate(std::mem::take(&mut c.ins), &results);
        let maps: Vec<&[f64]> = c.maps.iter().map(Slot::cells).collect();
        let mut cells = p.run(&maps, s.width * s.height).into_iter();
        Ok(vals
            .into_iter()
            .map(|(name, v)| {
                let o = match v {
                    Val::One(Arg::Num(x)) => Out::Number(x),
                    Val::One(_) => Out::Map(cells.next().unwrap()),
                    Val::Layers(l) => Out::Stack(l.into_iter().map(|x| (x.key, cells.next().unwrap())).collect()),
                };
                (name, o)
            })
            .collect())
    }
}

/// `map.calc`.
pub struct Calc;

impl Operator for Calc {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "map.calc",
            "Maps, stacks and numbers computed cell by cell from a script over the node's inputs (any names: maps, \
             numbers, stacks), as Dinamica EGO's Calculate Map, in a language close to Python's: arithmetic \
             (+ - * / % **), comparisons (chained: 5 <= x < 8), and/or/not, a if c else b (or if c then a else b), \
             abs sqrt exp ln log log10 floor ceil round min max clamp isnull coalesce, the neighbourhood functions \
             nbsum nbmean nbmin nbmax nbcount(x, radius), a stack's layers as s.layer, an operation on a stack made \
             on each layer, mean sd median sum count min max percentile(s, p) of a stack's layers at each cell, and \
             x.mean() x.sd() x.median() x.sum() x.count() x.min() x.max() x.percentile(p), a number from all the \
             cells. One statement a line (# a comment): name = value, name[where] = value, s.layer = value, and \
             out name = value (or out name) for each output, a map, a stack or a number; without out, the last \
             value is the map. A missing value gives a missing cell.",
            Access::Local,
        )
        .extra(Type::Any)
        .param(PortSpec::new(
            "expression",
            Type::Text,
            "the expression or the script, e.g. if landuse == 3 then slope else null",
        ))
        .output(PortSpec::new("map", Type::Map, "the map computed (by a script without out)"))
        .outputs_from("expression")
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let text: String =
            args.opt_param("expression")?.ok_or_else(|| Error::msg(format!("node {node:?}: no expression")))?;
        let script =
            parse_script(&text).map_err(|x| Error::msg(format!("node {node:?}: the expression: {}", x.0)))?;
        let mut grid = None;
        let mut scope =
            Scope { width: 0, height: 0, maps: BTreeMap::new(), numbers: BTreeMap::new(), stacks: BTreeMap::new() };
        let mut check_grid = |g: &Arc<terraflux_raster::Grid>, name: &str| -> Result<()> {
            match &grid {
                None => grid = Some(g.clone()),
                Some(first) if first.width != g.width || first.height != g.height => {
                    return Err(Error::msg(format!("node {node:?}: {name} is not on the grid of the other maps")));
                }
                _ => {}
            }
            Ok(())
        };
        for (name, v) in &args.inputs {
            match v {
                Value::Map(m) => {
                    check_grid(&m.grid, name)?;
                    scope.maps.insert(name, &m.data);
                }
                Value::Stack(st) => {
                    check_grid(&st.grid, name)?;
                    let layers = st.keys.iter().cloned().zip(st.layers.iter().map(|l| l.as_slice())).collect();
                    scope.stacks.insert(name, layers);
                }
                v => match v.as_number() {
                    Some(x) => {
                        scope.numbers.insert(name, x);
                    }
                    None => {
                        return Err(Error::msg(format!("node {node:?}: {name} is neither a map nor a number")));
                    }
                },
            }
        }
        if let Some(g) = &grid {
            scope.width = g.width;
            scope.height = g.height;
        }
        let on_grid =
            || grid.clone().ok_or_else(|| Error::msg(format!("node {node:?}: map.calc needs a map among its inputs")));
        // a script without out gives one map
        let one = script.outputs().is_empty();
        let mut out = Outputs::new();
        for (name, o) in script.run(&scope).map_err(|x| Error::msg(format!("node {node:?}: {}", x.0)))? {
            let v = match o {
                Out::Number(x) if one => {
                    let g = on_grid()?;
                    let n = g.width * g.height;
                    Value::Map(Map::new(g, vec![x; n]))
                }
                Out::Number(x) => Value::Number(x),
                Out::Map(cells) => Value::Map(Map::new(on_grid()?, cells)),
                Out::Stack(_) if one => {
                    return Err(Error::msg(format!(
                        "node {node:?}: the result is a stack: give it a name with out (out s2 = ...)"
                    )));
                }
                Out::Stack(layers) => {
                    let (keys, layers) = layers.into_iter().map(|(k, c)| (k, Arc::new(c))).unzip();
                    Value::Stack(Stack { grid: on_grid()?, keys, layers })
                }
            };
            out.insert(name, v);
        }
        Ok(out)
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Calc));
}

#[cfg(test)]
mod tests {
    use super::*;

    // The tree evaluator of map.calc 1.15.0, kept as the reference the compiled one must equal.

    /// A value while evaluating: a number for every cell, or one per cell.
    #[derive(Debug, Clone)]
    enum V {
        One(f64),
        Cells(Vec<f64>),
    }

    fn binary(op: &str, a: f64, b: f64) -> f64 {
        let cmp = |c: bool| if a.is_nan() || b.is_nan() { f64::NAN } else { c as u8 as f64 };
        match op {
            "+" => a + b,
            "-" => a - b,
            "*" => a * b,
            "/" => a / b,
            "%" => a % b,
            "^" => a.powf(b),
            "<" => cmp(a < b),
            "<=" => cmp(a <= b),
            ">" => cmp(a > b),
            ">=" => cmp(a >= b),
            "==" => cmp(a == b),
            "!=" => cmp(a != b),
            "and" => {
                if a.is_nan() || b.is_nan() {
                    f64::NAN
                } else {
                    (a != 0.0 && b != 0.0) as u8 as f64
                }
            }
            "or" => {
                if a.is_nan() || b.is_nan() {
                    f64::NAN
                } else {
                    (a != 0.0 || b != 0.0) as u8 as f64
                }
            }
            _ => unreachable!(),
        }
    }

    fn map2(a: V, b: V, f: impl Fn(f64, f64) -> f64) -> V {
        match (a, b) {
            (V::One(x), V::One(y)) => V::One(f(x, y)),
            (V::Cells(mut x), V::One(y)) => {
                x.iter_mut().for_each(|v| *v = f(*v, y));
                V::Cells(x)
            }
            (V::One(x), V::Cells(mut y)) => {
                y.iter_mut().for_each(|v| *v = f(x, *v));
                V::Cells(y)
            }
            (V::Cells(mut x), V::Cells(y)) => {
                x.iter_mut().zip(y).for_each(|(v, w)| *v = f(*v, w));
                V::Cells(x)
            }
        }
    }

    fn map1(a: V, f: impl Fn(f64) -> f64) -> V {
        match a {
            V::One(x) => V::One(f(x)),
            V::Cells(mut x) => {
                x.iter_mut().for_each(|v| *v = f(*v));
                V::Cells(x)
            }
        }
    }

    /// Sums and counts of the values (missing ones left out) in the square of radius `r` around each cell, from
    /// summed-area tables.
    fn box_sums_ref(v: &[f64], w: usize, h: usize, r: usize) -> (Vec<f64>, Vec<f64>) {
        let (w1, h1) = (w + 1, h + 1);
        let mut s = vec![0.0f64; w1 * h1];
        let mut c = vec![0.0f64; w1 * h1];
        for y in 0..h {
            let (mut rs, mut rc) = (0.0, 0.0);
            for x in 0..w {
                let x_ = v[y * w + x];
                if !x_.is_nan() {
                    rs += x_;
                    rc += 1.0;
                }
                s[(y + 1) * w1 + x + 1] = s[y * w1 + x + 1] + rs;
                c[(y + 1) * w1 + x + 1] = c[y * w1 + x + 1] + rc;
            }
        }
        let mut sum = vec![0.0; w * h];
        let mut count = vec![0.0; w * h];
        for y in 0..h {
            let (y0, y1) = (y.saturating_sub(r), (y + r + 1).min(h));
            for x in 0..w {
                let (x0, x1) = (x.saturating_sub(r), (x + r + 1).min(w));
                let at = |t: &[f64]| t[y1 * w1 + x1] - t[y0 * w1 + x1] - t[y1 * w1 + x0] + t[y0 * w1 + x0];
                sum[y * w + x] = at(&s);
                count[y * w + x] = at(&c);
            }
        }
        (sum, count)
    }

    fn box_extreme_ref(v: &[f64], w: usize, h: usize, r: usize, max: bool) -> Vec<f64> {
        let mut out = vec![f64::NAN; w * h];
        for y in 0..h {
            for x in 0..w {
                let mut best = f64::NAN;
                for yy in y.saturating_sub(r)..(y + r + 1).min(h) {
                    for xx in x.saturating_sub(r)..(x + r + 1).min(w) {
                        let z = v[yy * w + xx];
                        if !z.is_nan() && (best.is_nan() || (max && z > best) || (!max && z < best)) {
                            best = z;
                        }
                    }
                }
                out[y * w + x] = best;
            }
        }
        out
    }

    impl Expr {
        fn eval_ref(&self, s: &Scope) -> Result<V> {
            let n = s.width * s.height;
            Ok(match self {
                Expr::Num(x) => V::One(*x),
                Expr::Var(name) => {
                    if let Some(m) = s.maps.get(name.as_str()) {
                        V::Cells(m.to_vec())
                    } else if let Some(x) = s.numbers.get(name.as_str()) {
                        V::One(*x)
                    } else {
                        return Err(Error::msg(format!("no input {name:?} (a map or a number)")));
                    }
                }
                Expr::Layer(stack, layer) => match s
                    .stacks
                    .get(stack.as_str())
                    .and_then(|l| l.iter().find(|(k, _)| *k == Key::Name(layer.clone())))
                {
                    Some((_, m)) => V::Cells(m.to_vec()),
                    None => return Err(Error::msg(format!("no layer {layer:?} in a stack {stack:?}"))),
                },
                Expr::Unary('-', a) => map1(a.eval_ref(s)?, |x| -x),
                Expr::Unary(_, a) => map1(a.eval_ref(s)?, |x| if x.is_nan() { f64::NAN } else { (x == 0.0) as u8 as f64 }),
                Expr::Binary(op, a, b) => {
                    let op: &str = op;
                    map2(a.eval_ref(s)?, b.eval_ref(s)?, |x, y| binary(op, x, y))
                }
                Expr::If(c, a, b) => {
                    let (c, a, b) = (c.eval_ref(s)?, a.eval_ref(s)?, b.eval_ref(s)?);
                    let pick = |c: f64, a: f64, b: f64| match truth(c) {
                        t if t.is_nan() => f64::NAN,
                        t if t != 0.0 => a,
                        _ => b,
                    };
                    match (c, a, b) {
                        (V::One(c), V::One(a), V::One(b)) => V::One(pick(c, a, b)),
                        (c, a, b) => {
                            let at = |v: &V, i: usize| match v {
                                V::One(x) => *x,
                                V::Cells(x) => x[i],
                            };
                            V::Cells((0..n).map(|i| pick(at(&c, i), at(&a, i), at(&b, i))).collect())
                        }
                    }
                }
                Expr::Call(f, args) => self.call_ref(f, args, s)?,
                Expr::Method(..) => unreachable!(),
            })
        }

        fn call_ref(&self, f: &str, args: &[Expr], s: &Scope) -> Result<V> {
            let (w, h) = (s.width, s.height);
            if f.starts_with("nb") {
                let r = match &args[1] {
                    Expr::Num(r) => *r as usize,
                    Expr::Var(n) => match s.numbers.get(n.as_str()) {
                        Some(&x) if x >= 0.0 && x.fract() == 0.0 => x as usize,
                        Some(&x) => return Err(Error::msg(format!("the radius of {f} ({n} = {x}) is not a whole number"))),
                        None => return Err(Error::msg(format!("the radius of {f} ({n}) is not a number input"))),
                    },
                    _ => unreachable!(),
                };
                let cells = match args[0].eval_ref(s)? {
                    V::One(x) => vec![x; w * h],
                    V::Cells(v) => v,
                };
                return Ok(V::Cells(match f {
                    "nbsum" | "nbmean" | "nbcount" => {
                        let (sum, count) = box_sums_ref(&cells, w, h, r);
                        match f {
                            "nbsum" => sum.iter().zip(&count).map(|(s, c)| if *c > 0.0 { *s } else { f64::NAN }).collect(),
                            "nbmean" => {
                                sum.iter().zip(&count).map(|(s, c)| if *c > 0.0 { s / c } else { f64::NAN }).collect()
                            }
                            _ => count,
                        }
                    }
                    "nbmin" => box_extreme_ref(&cells, w, h, r, false),
                    _ => box_extreme_ref(&cells, w, h, r, true),
                }));
            }
            let mut vals = args.iter().map(|a| a.eval_ref(s)).collect::<Result<Vec<V>>>()?;
            Ok(match f {
                "min" | "max" => {
                    let max = f == "max";
                    let mut acc = vals.remove(0);
                    for v in vals {
                        acc = map2(acc, v, |a, b| {
                            if a.is_nan() || b.is_nan() {
                                f64::NAN
                            } else if max {
                                a.max(b)
                            } else {
                                a.min(b)
                            }
                        });
                    }
                    acc
                }
                "coalesce" => {
                    let b = vals.pop().unwrap();
                    map2(vals.pop().unwrap(), b, |a, b| if a.is_nan() { b } else { a })
                }
                "clamp" => {
                    let hi = vals.pop().unwrap();
                    let lo = vals.pop().unwrap();
                    let x = vals.pop().unwrap();
                    map2(map2(x, lo, |x, l| if x < l { l } else { x }), hi, |x, u| if x > u { u } else { x })
                }
                _ => {
                    let g: fn(f64) -> f64 = match f {
                        "abs" => f64::abs,
                        "sqrt" => f64::sqrt,
                        "exp" => f64::exp,
                        "ln" => f64::ln,
                        "log10" => f64::log10,
                        "floor" => f64::floor,
                        "ceil" => f64::ceil,
                        "round" => f64::round,
                        "isnull" => |x: f64| x.is_nan() as u8 as f64,
                        _ => unreachable!(),
                    };
                    map1(vals.pop().unwrap(), g)
                }
            })
        }

        fn evaluate_ref(&self, s: &Scope) -> Result<Vec<f64>> {
            Ok(match self.eval_ref(s)? {
                V::One(x) => vec![x; s.width * s.height],
                V::Cells(v) => v,
            })
        }
    }

    fn scope<'a>(maps: &'a [(&'a str, Vec<f64>)], w: usize, h: usize) -> Scope<'a> {
        Scope {
            width: w,
            height: h,
            maps: maps.iter().map(|(n, v)| (*n, v.as_slice())).collect(),
            numbers: [("k", 2.0)].into(),
            stacks: BTreeMap::new(),
        }
    }

    fn calc(text: &str, maps: &[(&str, Vec<f64>)], w: usize, h: usize) -> Vec<f64> {
        parse(text).unwrap().evaluate(&scope(maps, w, h)).unwrap()
    }

    fn same(a: &[f64], b: &[f64]) {
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b) {
            assert!((x.is_nan() && y.is_nan()) || (x - y).abs() < 1e-12, "{a:?} != {b:?}");
        }
    }

    #[test]
    fn arithmetic_and_precedence() {
        let a = vec![1.0, 2.0, 3.0, f64::NAN];
        same(&calc("1 + 2 * 3 ^ 2", &[], 2, 2), &[19.0; 4]);
        same(&calc("-a ^ 2 + k", &[("a", a.clone())], 2, 2), &[1.0, -2.0, -7.0, f64::NAN]);
        same(&calc("(a + 1) % 2", &[("a", a.clone())], 2, 2), &[0.0, 1.0, 0.0, f64::NAN]);
        same(&calc("2 ^ 3 ^ 2", &[], 1, 1), &[512.0]);
    }

    #[test]
    fn logic_and_conditions() {
        let a = vec![1.0, 2.0, 3.0, f64::NAN];
        same(&calc("if a >= 2 and not a == 3 then 10 else a", &[("a", a.clone())], 2, 2), &[1.0, 10.0, 3.0, f64::NAN]);
        same(&calc("a < 2 || a > 2 ? 1 : 0", &[("a", a.clone())], 2, 2), &[1.0, 0.0, 1.0, f64::NAN]);
        same(&calc("isnull(a) + coalesce(a, -1)", &[("a", a.clone())], 2, 2), &[1.0, 2.0, 3.0, 0.0]);
        same(&calc("min(a, 2, k) + max(clamp(a, 2, 2.5), 0)", &[("a", a)], 2, 2), &[3.0, 4.0, 4.5, f64::NAN]);
    }

    #[test]
    fn neighbourhoods() {
        // 3 x 3: 1..9 with a missing centre
        let a = vec![1.0, 2.0, 3.0, 4.0, f64::NAN, 6.0, 7.0, 8.0, 9.0];
        same(&calc("nbsum(a, 1)", &[("a", a.clone())], 3, 3), &[7.0, 16.0, 11.0, 22.0, 40.0, 28.0, 19.0, 34.0, 23.0]);
        same(&calc("nbcount(a, 1)", &[("a", a.clone())], 3, 3), &[3.0, 5.0, 3.0, 5.0, 8.0, 5.0, 3.0, 5.0, 3.0]);
        same(&calc("nbmax(a, 1)", &[("a", a.clone())], 3, 3), &[4.0, 6.0, 6.0, 8.0, 9.0, 9.0, 8.0, 9.0, 9.0]);
        same(&calc("nbmin(a, 0)", &[("a", a.clone())], 3, 3), &a);
        same(&calc("nbmean(a == 2, 1)", &[("a", a.clone())], 3, 3)[..3], &[1.0 / 3.0, 1.0 / 5.0, 1.0 / 3.0]);
        assert_eq!(parse("nbsum(a, 2) + nbmax(a, 1)").unwrap().radius(), 2);
        // a radius given by a number input
        assert_eq!(calc("nbsum(a, k)", &[("a", a.clone())], 3, 3), calc("nbsum(a, 2)", &[("a", a.clone())], 3, 3));
    }

    #[test]
    fn errors_say_where() {
        for (bad, why) in [
            ("1 +", "ends too early"),
            ("a = 1", "=="),
            ("foo(1)", "no function"),
            ("nbsum(a, 1.5)", "whole number"),
            ("abs(1, 2)", "takes 1"),
            ("(1 + 2", "expected \")\""),
            ("1 2", "unexpected"),
            ("if a then 1", "expected \"else\""),
        ] {
            let e = parse(bad).err().unwrap().0;
            assert!(e.contains(why), "{bad}: {e}");
        }
        let e = parse("b + 1").unwrap().evaluate(&scope(&[], 1, 1)).err().unwrap().0;
        assert!(e.contains("no input \"b\""), "{e}");
        assert_eq!(parse("x + s.slope").unwrap().names(), vec!["x".to_string(), "s".to_string()]);
    }

    /// A generator of numbers (xorshift): the tests draw the same every time.
    struct Draw(u64);

    impl Draw {
        fn below(&mut self, n: u64) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 % n
        }

        fn sub(&mut self, depth: u32) -> Box<Expr> {
            Box::new(self.expr(depth - 1))
        }

        /// An expression of everything the language has, over the maps a and b, the layer s.x and the number k.
        fn expr(&mut self, depth: u32) -> Expr {
            if depth == 0 || self.below(4) == 0 {
                return match self.below(5) {
                    0 => Expr::Num([0.0, 1.0, 2.0, -1.5, 0.5, f64::NAN][self.below(6) as usize]),
                    1 => Expr::Var("a".into()),
                    2 => Expr::Var("b".into()),
                    3 => Expr::Var("k".into()),
                    _ => Expr::Layer("s".into(), "x".into()),
                };
            }
            match self.below(7) {
                0 => Expr::Unary(['-', '!'][self.below(2) as usize], self.sub(depth)),
                1 | 2 => {
                    let ops = ["+", "-", "*", "/", "%", "^", "<", "<=", ">", ">=", "==", "!=", "and", "or"];
                    Expr::Binary(ops[self.below(ops.len() as u64) as usize], self.sub(depth), self.sub(depth))
                }
                3 => Expr::If(self.sub(depth), self.sub(depth), self.sub(depth)),
                4 => {
                    let f = ["abs", "sqrt", "exp", "ln", "log10", "floor", "ceil", "round", "isnull"]
                        [self.below(9) as usize];
                    Expr::Call(f.into(), vec![*self.sub(depth)])
                }
                5 => match self.below(4) {
                    0 => Expr::Call("coalesce".into(), vec![*self.sub(depth), *self.sub(depth)]),
                    1 => Expr::Call("clamp".into(), vec![*self.sub(depth), *self.sub(depth), *self.sub(depth)]),
                    k => {
                        let args = (0..2 + self.below(3)).map(|_| *self.sub(depth)).collect();
                        Expr::Call(if k == 2 { "min" } else { "max" }.into(), args)
                    }
                },
                _ => {
                    let f = ["nbsum", "nbmean", "nbmin", "nbmax", "nbcount"][self.below(5) as usize];
                    let r = if self.below(4) == 0 { Expr::Var("k".into()) } else { Expr::Num(self.below(5) as f64) };
                    Expr::Call(f.into(), vec![*self.sub(depth), r])
                }
            }
        }

        /// Cells of a few classes, a share of them missing.
        fn cells(&mut self, n: usize) -> Vec<f64> {
            (0..n)
                .map(|_| match self.below(10) {
                    0 => f64::NAN,
                    1 => self.below(1000) as f64 / 7.0,
                    k => (k % 5) as f64,
                })
                .collect()
        }
    }

    /// The compiled evaluator gives what the tree evaluator gave, cell by cell (zero and minus zero count as the
    /// same), and the same errors.
    #[test]
    fn compiled_as_the_tree() {
        let mut d = Draw(0x9e37_79b9_7f4a_7c15);
        // small grids in one thread; one larger than a few blocks, in parallel
        for (w, h, exprs) in [(37, 23, 400), (1, 9, 100), (9, 1, 100), (211, 97, 60)] {
            let (a, b, x) = (d.cells(w * h), d.cells(w * h), d.cells(w * h));
            let s = Scope {
                width: w,
                height: h,
                maps: [("a", a.as_slice()), ("b", b.as_slice())].into(),
                numbers: [("k", 2.0)].into(),
                stacks: [("s", vec![(Key::Name("x".into()), x.as_slice())])].into(),
            };
            for _ in 0..exprs {
                let e = d.expr(5);
                let (new, old) = (e.evaluate(&s).unwrap(), e.evaluate_ref(&s).unwrap());
                for (i, (p, q)) in new.iter().zip(&old).enumerate() {
                    assert!(p == q || (p.is_nan() && q.is_nan()), "{e:?} at {i}: {p} != {q}");
                }
            }
        }
        let one = [1.0];
        let s = Scope {
            width: 1,
            height: 1,
            maps: [("a", one.as_slice())].into(),
            numbers: [("h", 0.5)].into(),
            stacks: BTreeMap::new(),
        };
        for bad in ["s.y * 2", "nbsum(a, z)", "nbmax(a, h)"] {
            let e = parse(bad).unwrap();
            assert_eq!(e.evaluate(&s).err().unwrap().0, e.evaluate_ref(&s).err().unwrap().0);
        }
        let e = parse("c + 1").unwrap().evaluate(&s).err().unwrap().0;
        assert_eq!(e, "no input \"c\" (a map, a stack or a number)");
    }

    #[test]
    fn windows_larger_than_the_map() {
        let a = Draw(7).cells(5 * 3);
        let s = Scope { maps: [("a", a.as_slice())].into(), ..scope(&[], 5, 3) };
        for f in ["nbmin", "nbmax", "nbsum", "nbcount"] {
            let e = parse(&format!("{f}(a, 50)")).unwrap();
            same(&e.evaluate(&s).unwrap(), &e.evaluate_ref(&s).unwrap());
        }
    }

    /// A value computed twice is computed once, numbers are folded, and registers are reused.
    #[test]
    fn compiled_small() {
        let (a, b) = ([1.0], [2.0]);
        let s = Scope { maps: [("a", a.as_slice()), ("b", b.as_slice())].into(), ..scope(&[], 1, 1) };
        let program = |text: &str| {
            let mut c = Compiler::new(&s);
            let Val::One(a) = c.lower(&parse(text).unwrap()).unwrap() else { panic!("{text}") };
            allocate(c.ins, &[a])
        };
        let p = program("(a + b) * (a + b) + (a + b)");
        assert_eq!(p.ins.len(), 3, "{p:?}");
        assert_eq!(program("2 * k + 1 ^ 2").results, [Arg::Num(5.0)]);
        assert_eq!(program("if k > 1 then a else b").results, [Arg::Map(0)]);
        assert!(program("if k < 1 then a else b").ins.is_empty());
        // a chain: two registers whatever its length
        let p = program("((((a + 1) * 2 - b) / 3 + a) * b - 1) / 2");
        assert!(p.regs <= 2, "{p:?}");
        // the same neighbourhood twice: computed once
        let mut c = Compiler::new(&s);
        c.lower(&parse("nbmax(a, 1) + nbmax(a, 1) * nbmin(a, 1)").unwrap()).unwrap();
        assert_eq!(c.maps.len(), 3);
    }

    /// The compiled evaluator against the tree evaluator on 9 million cells:
    /// `cargo test -p terraflux-ops --release --lib algebra::tests::speed -- --ignored --nocapture`.
    #[test]
    #[ignore = "a benchmark"]
    fn speed() {
        crate::threads::ensure();
        let (w, h) = (3000, 3000);
        let mut d = Draw(11);
        let (a, b, x) = (d.cells(w * h), d.cells(w * h), d.cells(w * h));
        let s = Scope {
            width: w,
            height: h,
            maps: [("a", a.as_slice()), ("b", b.as_slice())].into(),
            numbers: [("k", 2.0)].into(),
            stacks: [("s", vec![(Key::Name("x".into()), x.as_slice())])].into(),
        };
        println!("{} threads, {w} x {h} cells; the tree, compiled", rayon::current_num_threads());
        for text in [
            "a * 2 + b",
            "clamp((1200 - s.x) / 900, 0, 1)",
            "if a == 1 then sqrt(s.x * b) else 0",
            "if a >= 2 and not a == 3 then 10 else if a == 3 then b * 2 else min(a, b, k)",
            "(a == 1) * 3 + (a == 1) * b - (a == 1) / (b + 1)",
            "nbmean(a == 1, 2)",
            "if a == 1 then 2 - (nbmin(a == 1, 1) == 0) else 0",
            "nbmax(a, 5)",
            "nbmin(b, 25)",
        ] {
            let e = parse(text).unwrap();
            let t = std::time::Instant::now();
            let old = e.evaluate_ref(&s).unwrap();
            let t_old = t.elapsed().as_secs_f64();
            let t = std::time::Instant::now();
            let new = e.evaluate(&s).unwrap();
            let t_new = t.elapsed().as_secs_f64();
            same(&new, &old);
            println!("{:>8.1} ms {:>8.1} ms {:>6.1}x  {text}", t_old * 1e3, t_new * 1e3, t_old / t_new);
        }
    }

    /// A script's outputs over the maps a and b, the stack s (layers x, y and z) and the number k.
    fn script(text: &str) -> Result<Vec<(String, Out)>> {
        let a = [1.0, 2.0, 3.0, f64::NAN, 5.0, 6.0];
        let b = [0.0, 1.0, 0.0, 1.0, f64::NAN, 1.0];
        let (x, y, z) = (
            [1.0, 2.0, 3.0, 4.0, 5.0, f64::NAN],
            [3.0, f64::NAN, 1.0, 4.0, 9.0, f64::NAN],
            [2.0, 2.0, 2.0, 4.0, 1.0, f64::NAN],
        );
        let s = Scope {
            width: 3,
            height: 2,
            maps: [("a", a.as_slice()), ("b", b.as_slice())].into(),
            numbers: [("k", 2.0)].into(),
            stacks: [(
                "s",
                vec![
                    (Key::Name("x".into()), x.as_slice()),
                    (Key::Name("y".into()), y.as_slice()),
                    (Key::Name("z".into()), z.as_slice()),
                ],
            )]
            .into(),
        };
        parse_script(text)?.run(&s)
    }

    fn map_of(outs: &[(String, Out)], name: &str) -> Vec<f64> {
        match outs.iter().find(|(n, _)| n == name).map(|(_, o)| o) {
            Some(Out::Map(v)) => v.clone(),
            o => panic!("{name}: {o:?}"),
        }
    }

    fn number_of(outs: &[(String, Out)], name: &str) -> f64 {
        match outs.iter().find(|(n, _)| n == name).map(|(_, o)| o) {
            Some(Out::Number(x)) => *x,
            o => panic!("{name}: {o:?}"),
        }
    }

    fn stack_of(outs: &[(String, Out)], name: &str) -> Vec<(Key, Vec<f64>)> {
        match outs.iter().find(|(n, _)| n == name).map(|(_, o)| o) {
            Some(Out::Stack(v)) => v.clone(),
            o => panic!("{name}: {o:?}"),
        }
    }

    const N: f64 = f64::NAN;

    #[test]
    fn as_python_writes_it() {
        let a = vec![1.0, 2.0, 3.0, f64::NAN, 5.0, 7.0];
        for (python, ours) in [
            ("a ** 2", "a ^ 2"),
            ("-a ** 2", "-(a ^ 2)"),
            ("1 if a > 2 else 0", "if a > 2 then 1 else 0"),
            ("a if a < 3 else a * 2 if a < 6 else 0", "if a < 3 then a else if a < 6 then a * 2 else 0"),
            ("2 <= a < 6", "2 <= a and a < 6"),
            ("1 < a <= 5 != a", "1 < a and a <= 5 and 5 != a"),
            ("True + False * 2", "1"),
            ("None", "null"),
            ("log(a)", "ln(a)"),
            ("a  # a comment", "a"),
        ] {
            let m = [("a", a.clone())];
            same(&calc(python, &m, 3, 2), &calc(ours, &m, 3, 2));
        }
        for (bad, why) in [
            ("if a = 1 then 1 else 0", "use == to compare"),
            ("s.1997 + 1", "s._1997"),
            ("a.foo()", "no method \"foo\""),
            ("a.mean(1)", "mean() takes 0"),
            ("min()", "min takes a stack, or two values or more"),
        ] {
            let e = parse(bad).err().unwrap().0;
            assert!(e.contains(why), "{bad}: {e}");
        }
    }

    #[test]
    fn statements_and_masks() {
        // the cells where a condition holds; the others, and those where it is missing, keep their value
        let o = script("c = a\nc[a == 1] = 0\nc[2 <= a < 4] = 20\nc[b == 1] = c + 100").unwrap();
        same(&map_of(&o, "map"), &[0.0, 120.0, 20.0, N, 5.0, 106.0]);
        // ; separates statements; # begins a comment; a script without out gives its last value
        let o = script("x = a * k; y = x + 1  # two statements\ny - x").unwrap();
        same(&map_of(&o, "map"), &[1.0, 1.0, 1.0, N, 1.0, 1.0]);
        // outputs: maps and numbers, the value a name has at the end
        let o = script("out m = a\nm[a > 2] = 0\nout n = a.max()\nout k").unwrap();
        assert_eq!(o.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["m", "n", "k"]);
        same(&map_of(&o, "m"), &[1.0, 2.0, 0.0, N, 0.0, 0.0]);
        assert_eq!(number_of(&o, "n"), 6.0);
        assert_eq!(number_of(&o, "k"), 2.0);
        // an input given a new value is the script's own: the input is unchanged elsewhere
        let o = script("out c = a + 1\na = a * 0\nout d = a").unwrap();
        same(&map_of(&o, "c"), &[2.0, 3.0, 4.0, N, 6.0, 7.0]);
        for (bad, why) in [
            ("x = a\nx == 1\nout x", "line 2: a value computed and not kept"),
            ("a + 1\nout b", "line 1: a value computed and not kept"),
            ("out y", "no input \"y\""),
            ("c[a > 1] = 0", "no input \"c\""),
            ("a[s > 1] = 0", "a is a map"),
            ("out if = 3", "\"if\" is a word"),
            ("x = (a +\n 1)\nx[a = 1] = 0", "line 3: use == to compare"),
        ] {
            let e = script(bad).err().unwrap().0;
            assert!(e.contains(why), "{bad}: {e}");
        }
        // a new line inside brackets does not end the statement
        let o = script("x = (a +\n  1)\nout x").unwrap();
        same(&map_of(&o, "x"), &[2.0, 3.0, 4.0, N, 6.0, 7.0]);
    }

    #[test]
    fn stacks_layer_by_layer() {
        // a stack and a number, a map, a stack: each layer, the keys kept
        let o = script("out t = s * 2 + a\nout u = s - s\nout v = nbmax(s, 1)").unwrap();
        let t = stack_of(&o, "t");
        assert_eq!(t.iter().map(|(k, _)| k.to_string()).collect::<Vec<_>>(), ["x", "y", "z"]);
        same(&t[0].1, &[3.0, 6.0, 9.0, N, 15.0, N]);
        same(&t[1].1, &[7.0, N, 5.0, N, 23.0, N]);
        same(&stack_of(&o, "u")[2].1, &[0.0, 0.0, 0.0, 0.0, 0.0, N]);
        same(&stack_of(&o, "v")[0].1, &[5.0, 5.0, 5.0, 5.0, 5.0, 5.0]);
        // a layer read, replaced and added; masks on a stack
        let o = script("out t = s\nt.y = t.y * 10\nt.w = t.x + t.z\nt[a > 2] = 0\nout one = t.w").unwrap();
        let t = stack_of(&o, "t");
        assert_eq!(t.iter().map(|(k, _)| k.to_string()).collect::<Vec<_>>(), ["x", "y", "z", "w"]);
        same(&t[1].1, &[30.0, N, 0.0, 40.0, 0.0, 0.0]);
        same(&map_of(&o, "one"), &[3.0, 4.0, 0.0, 8.0, 0.0, 0.0]);
        for (bad, why) in [
            ("out t = s + stack2", "no input \"stack2\""),
            ("out t = s\nt.x = s", "line 2: t.x is one layer"),
            ("out m = a.x", "a is a map, not a stack"),
            ("out m = s.q", "no layer \"q\" in a stack \"s\" (its layers: x, y, z)"),
        ] {
            let e = script(bad).err().unwrap().0;
            assert!(e.contains(why), "{bad}: {e}");
        }
    }

    #[test]
    fn over_the_layers_of_each_cell() {
        let o = script(
            "out mean = mean(s)\nout sd = sd(s)\nout median = median(s)\nout p25 = percentile(s, 25)\n\
             out sum = sum(s)\nout count = count(s)\nout min = min(s)\nout max = max(s)\nout pairwise = min(s, 2)",
        )
        .unwrap();
        // the layers at each cell: (1 3 2) (2 - 2) (3 1 2) (4 4 4) (5 9 1) (- - -)
        same(&map_of(&o, "mean"), &[2.0, 2.0, 2.0, 4.0, 5.0, N]);
        same(&map_of(&o, "sd"), &[1.0, 0.0, 1.0, 0.0, 4.0, N]);
        same(&map_of(&o, "median"), &[2.0, 2.0, 2.0, 4.0, 5.0, N]);
        same(&map_of(&o, "p25"), &[1.5, 2.0, 1.5, 4.0, 3.0, N]);
        same(&map_of(&o, "sum"), &[6.0, 4.0, 6.0, 12.0, 15.0, N]);
        same(&map_of(&o, "count"), &[3.0, 2.0, 3.0, 3.0, 3.0, 0.0]);
        same(&map_of(&o, "min"), &[1.0, 2.0, 1.0, 4.0, 1.0, N]);
        same(&map_of(&o, "max"), &[3.0, 2.0, 3.0, 4.0, 9.0, N]);
        // min of a stack and a number: each layer
        same(&stack_of(&o, "pairwise")[0].1, &[1.0, 2.0, 2.0, 2.0, 2.0, N]);
        let e = script("out m = mean(a)").err().unwrap().0;
        assert!(e.contains("mean takes a stack") && e.contains("x.mean()"), "{e}");
        let e = script("out m = percentile(s, 120)").err().unwrap().0;
        assert!(e.contains("between 0 and 100"), "{e}");
    }

    #[test]
    fn numbers_of_all_the_cells() {
        // a: 1 2 3 - 5 6
        let o = script(
            "out mean = a.mean()\nout sd = a.sd()\nout median = a.median()\nout p90 = a.percentile(90)\n\
             out n = a.count()\nout lo = a.min()\nout hi = (a * 2).max()\nout all = s.sum()\n\
             out above = a > a.mean()",
        )
        .unwrap();
        assert_eq!(number_of(&o, "mean"), 3.4);
        assert!((number_of(&o, "sd") - 2.073644135332772).abs() < 1e-12);
        assert_eq!(number_of(&o, "median"), 3.0);
        assert!((number_of(&o, "p90") - 5.6).abs() < 1e-12);
        assert_eq!(number_of(&o, "n"), 5.0);
        assert_eq!(number_of(&o, "lo"), 1.0);
        assert_eq!(number_of(&o, "hi"), 12.0);
        assert_eq!(number_of(&o, "all"), 15.0 + 17.0 + 11.0);
        same(&map_of(&o, "above"), &[0.0, 0.0, 0.0, N, 1.0, 1.0]);
        let e = script("out m = k.mean()").err().unwrap().0;
        assert!(e.contains("not a number"), "{e}");
        // the same to the bit whatever the threads: sums of fixed parts
        let mut d = Draw(3);
        let big = d.cells(300_000);
        let at = |t: usize| {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(t).build().unwrap();
            pool.install(|| statistic("sd", 0.0, &[&big]))
        };
        assert_eq!(at(1).to_bits(), at(3).to_bits());
    }

    #[test]
    fn layers_named_as_names() {
        let keys = [
            Key::Name("1997".into()),
            Key::Name("NDVI-2020".into()),
            Key::Name("2".into()),
            Key::Name("if".into()),
            Key::Class(1.0),
            Key::Name("a-b".into()),
            Key::Name("a_b".into()),
            Key::Name("elevation".into()),
        ];
        assert_eq!(band_names(&keys), ["_1997", "NDVI_2020", "b3", "if_", "_1", "a_b", "a_b_2", "elevation"]);
        // and read by them
        let (one, two) = ([1.0], [2.0]);
        let s = Scope {
            width: 1,
            height: 1,
            maps: BTreeMap::new(),
            numbers: BTreeMap::new(),
            stacks: [("img", vec![(Key::Name("0".into()), one.as_slice()), (Key::Name("1997".into()), two.as_slice())])]
                .into(),
        };
        same(&parse("img.b1 + img._1997 * 10").unwrap().evaluate(&s).unwrap(), &[21.0]);
    }

    #[test]
    fn outputs_seen_by_the_graph() {
        // the graph reads the outputs from the text without this parser: the same names
        for text in [
            "out a = 1\nout b\n# out c\nx = 1; out d = x",
            "  out   e=1 ; out f\nout_g = 2\nout h # a comment",
            "a + 1",
        ] {
            let ours = parse_script(text).map(|s| s.outputs()).unwrap_or_default();
            assert_eq!(ours, terraflux_graph::declared_outputs(text), "{text}");
        }
    }
}
