//! Map algebra: `map.calc` computes a map from an expression over its inputs, as Dinamica EGO's Calculate Map.
//!
//! ```text
//! if landuse == 3 and slope < 10 then 1 else 0
//! (dist_roads < 500) * suitability + 0.1 * nbmean(landuse == 2, 1)
//! drivers.altitude / 1000
//! ```
//!
//! - **Values:** numbers, `pi`, `null` (a missing cell); the node's inputs by port name (maps, numbers, and the
//!   layers of a stack as `stack.layer`).
//! - **Operators:** `+ - * / % ^`, comparisons `< <= > >= == !=` (1 or 0), `and or not` (also `&& || !`),
//!   `if c then a else b` and `c ? a : b`. A missing value makes the result missing (`isnull` tests it).
//! - **Functions:** `abs sqrt exp ln log10 floor ceil round min max clamp(x, lo, hi) isnull(x)
//!   coalesce(x, y)` (`y` where `x` is missing).
//! - **Neighbourhoods** (a square of radius `r` cells around each cell, the cell included, missing cells left
//!   out): `nbsum(x, r) nbmean(x, r) nbmin(x, r) nbmax(x, r) nbcount(x, r)` (cells with a value; `r` a whole number or
//!   a number input).

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value, outputs};

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
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Id(String),
    Op(&'static str),
}

const OPS: [&str; 22] = [
    "<=", ">=", "==", "!=", "&&", "||", "<", ">", "+", "-", "*", "/", "%", "^", "(", ")", ",", "?", ":", "!", ".", "=",
];

fn lex(s: &str) -> Result<Vec<Tok>> {
    let b = s.as_bytes();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i] as char;
        if c.is_whitespace() {
            i += 1;
        } else if c.is_ascii_digit() || (c == '.' && i + 1 < b.len() && (b[i + 1] as char).is_ascii_digit()) {
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
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(Tok::Id(s[start..i].to_string()));
        } else {
            let op = OPS
                .iter()
                .find(|o| s[i..].starts_with(*o))
                .ok_or_else(|| Error::msg(format!("unexpected {c:?} at {}", i + 1)))?;
            if *op == "=" {
                return Err(Error::msg("use == to compare"));
            }
            out.push(Tok::Op(op));
            i += op.len();
        }
    }
    Ok(out)
}

struct Parser {
    toks: Vec<Tok>,
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

    fn expect_op(&mut self, op: &str) -> Result<()> {
        if self.is_op(op) {
            self.at += 1;
            Ok(())
        } else {
            Err(Error::msg(format!("expected {op:?} {}", self.here())))
        }
    }

    fn expect_word(&mut self, w: &str) -> Result<()> {
        if self.is_word(w) {
            self.at += 1;
            Ok(())
        } else {
            Err(Error::msg(format!("expected {w:?} {}", self.here())))
        }
    }

    fn here(&self) -> String {
        match self.peek() {
            None => "at the end".into(),
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
        let c = self.or()?;
        if self.is_op("?") {
            self.at += 1;
            let a = self.expr()?;
            self.expect_op(":")?;
            let b = self.expr()?;
            return Ok(Expr::If(Box::new(c), Box::new(a), Box::new(b)));
        }
        Ok(c)
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

    fn cmp(&mut self) -> Result<Expr> {
        let l = self.add()?;
        for op in ["<=", ">=", "==", "!=", "<", ">"] {
            if self.is_op(op) {
                self.at += 1;
                let r = self.add()?;
                return Ok(Expr::Binary(op, Box::new(l), Box::new(r)));
            }
        }
        Ok(l)
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
        let base = self.atom()?;
        if self.is_op("^") {
            self.at += 1;
            return Ok(Expr::Binary("^", Box::new(base), Box::new(self.unary()?)));
        }
        Ok(base)
    }

    fn atom(&mut self) -> Result<Expr> {
        let t = self.peek().cloned().ok_or_else(|| Error::msg("the expression ends too early"))?;
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
                "null" => Ok(Expr::Num(f64::NAN)),
                "if" | "then" | "else" | "and" | "or" | "not" => Err(Error::msg(format!("unexpected {id:?}"))),
                _ if self.is_op("(") => {
                    self.at += 1;
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
                    check_call(&id, &args)?;
                    Ok(Expr::Call(id, args))
                }
                _ if self.is_op(".") => {
                    self.at += 1;
                    match self.peek().cloned() {
                        Some(Tok::Id(layer)) => {
                            self.at += 1;
                            Ok(Expr::Layer(id, layer))
                        }
                        _ => Err(Error::msg(format!("expected a layer of {id:?} after the dot"))),
                    }
                }
                _ => Ok(Expr::Var(id)),
            },
            Tok::Op(o) => Err(Error::msg(format!("unexpected {o:?}"))),
        }
    }
}

const FUNCTIONS: [(&str, usize); 16] = [
    ("abs", 1),
    ("sqrt", 1),
    ("exp", 1),
    ("ln", 1),
    ("log10", 1),
    ("floor", 1),
    ("ceil", 1),
    ("round", 1),
    ("isnull", 1),
    ("coalesce", 2),
    ("clamp", 3),
    ("nbsum", 2),
    ("nbmean", 2),
    ("nbmin", 2),
    ("nbmax", 2),
    ("nbcount", 2),
];

fn check_call(name: &str, args: &[Expr]) -> Result<()> {
    if name == "min" || name == "max" {
        return if args.len() >= 2 { Ok(()) } else { Err(Error::msg(format!("{name} takes two values or more"))) };
    }
    let (_, n) =
        FUNCTIONS.iter().find(|(f, _)| *f == name).ok_or_else(|| Error::msg(format!("no function {name:?}")))?;
    if args.len() != *n {
        return Err(Error::msg(format!("{name} takes {n} value(s), not {}", args.len())));
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

/// Parse an expression.
pub fn parse(text: &str) -> Result<Expr> {
    let mut p = Parser { toks: lex(text)?, at: 0 };
    let e = p.expr()?;
    if p.at < p.toks.len() {
        return Err(Error::msg(format!("unexpected text {}", p.here())));
    }
    Ok(e)
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
            _ => {}
        }
    }
}

/// A value while evaluating: a number for every cell, or one per cell.
#[derive(Debug, Clone)]
enum V {
    One(f64),
    Cells(Vec<f64>),
}

/// What an expression reads: maps and numbers by name, stacks' layers by (stack, layer); the grid's size.
pub struct Scope<'a> {
    pub width: usize,
    pub height: usize,
    pub maps: BTreeMap<&'a str, &'a [f64]>,
    pub numbers: BTreeMap<&'a str, f64>,
    pub layers: BTreeMap<(&'a str, &'a str), &'a [f64]>,
}

fn truth(x: f64) -> f64 {
    if x.is_nan() { f64::NAN } else { (x != 0.0) as u8 as f64 }
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
fn box_sums(v: &[f64], w: usize, h: usize, r: usize) -> (Vec<f64>, Vec<f64>) {
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

fn box_extreme(v: &[f64], w: usize, h: usize, r: usize, max: bool) -> Vec<f64> {
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
    fn eval(&self, s: &Scope) -> Result<V> {
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
            Expr::Layer(stack, layer) => match s.layers.get(&(stack.as_str(), layer.as_str())) {
                Some(m) => V::Cells(m.to_vec()),
                None => return Err(Error::msg(format!("no layer {layer:?} in a stack {stack:?}"))),
            },
            Expr::Unary('-', a) => map1(a.eval(s)?, |x| -x),
            Expr::Unary(_, a) => map1(a.eval(s)?, |x| if x.is_nan() { f64::NAN } else { (x == 0.0) as u8 as f64 }),
            Expr::Binary(op, a, b) => {
                let op: &str = op;
                map2(a.eval(s)?, b.eval(s)?, |x, y| binary(op, x, y))
            }
            Expr::If(c, a, b) => {
                let (c, a, b) = (c.eval(s)?, a.eval(s)?, b.eval(s)?);
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
            Expr::Call(f, args) => self.call(f, args, s)?,
        })
    }

    fn call(&self, f: &str, args: &[Expr], s: &Scope) -> Result<V> {
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
            let cells = match args[0].eval(s)? {
                V::One(x) => vec![x; w * h],
                V::Cells(v) => v,
            };
            return Ok(V::Cells(match f {
                "nbsum" | "nbmean" | "nbcount" => {
                    let (sum, count) = box_sums(&cells, w, h, r);
                    match f {
                        "nbsum" => sum.iter().zip(&count).map(|(s, c)| if *c > 0.0 { *s } else { f64::NAN }).collect(),
                        "nbmean" => {
                            sum.iter().zip(&count).map(|(s, c)| if *c > 0.0 { s / c } else { f64::NAN }).collect()
                        }
                        _ => count,
                    }
                }
                "nbmin" => box_extreme(&cells, w, h, r, false),
                _ => box_extreme(&cells, w, h, r, true),
            }));
        }
        let mut vals = args.iter().map(|a| a.eval(s)).collect::<Result<Vec<V>>>()?;
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

    /// The value of every cell of the grid.
    pub fn evaluate(&self, s: &Scope) -> Result<Vec<f64>> {
        Ok(match self.eval(s)? {
            V::One(x) => vec![x; s.width * s.height],
            V::Cells(v) => v,
        })
    }
}

/// `map.calc`.
pub struct Calc;

impl Operator for Calc {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "map.calc",
            "A map computed cell by cell from an expression over the node's inputs (any names: maps, numbers, \
             stacks as stack.layer), as Dinamica EGO's Calculate Map: arithmetic, comparisons, and/or/not, \
             if-then-else, abs sqrt exp ln log10 floor ceil round min max clamp isnull coalesce, and the \
             neighbourhood functions nbsum nbmean nbmin nbmax nbcount(x, radius). A missing value gives a \
             missing cell.",
            Access::Local,
        )
        .extra(Type::Any)
        .param(PortSpec::new("expression", Type::Text, "the expression, e.g. if landuse == 3 then slope else null"))
        .output(PortSpec::new("map", Type::Map, "the map computed"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let text: String =
            args.opt_param("expression")?.ok_or_else(|| Error::msg(format!("node {:?}: no expression", args.node)))?;
        let e = parse(&text).map_err(|x| Error::msg(format!("node {:?}: the expression: {}", args.node, x.0)))?;
        let mut grid = None;
        let mut scope =
            Scope { width: 0, height: 0, maps: BTreeMap::new(), numbers: BTreeMap::new(), layers: BTreeMap::new() };
        let mut check_grid = |g: &Arc<terraflux_raster::Grid>, name: &str| -> Result<()> {
            match &grid {
                None => grid = Some(g.clone()),
                Some(first) if first.width != g.width || first.height != g.height => {
                    return Err(Error::msg(format!(
                        "node {:?}: {name} is not on the grid of the other maps",
                        args.node
                    )));
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
                    for (k, l) in st.keys.iter().zip(&st.layers) {
                        if let crate::Key::Name(layer) = k {
                            scope.layers.insert((name, layer), l);
                        }
                    }
                }
                v => match v.as_number() {
                    Some(x) => {
                        scope.numbers.insert(name, x);
                    }
                    None => {
                        return Err(Error::msg(format!("node {:?}: {name} is neither a map nor a number", args.node)));
                    }
                },
            }
        }
        let grid =
            grid.ok_or_else(|| Error::msg(format!("node {:?}: map.calc needs a map among its inputs", args.node)))?;
        scope.width = grid.width;
        scope.height = grid.height;
        let cells = e.evaluate(&scope).map_err(|x| Error::msg(format!("node {:?}: {}", args.node, x.0)))?;
        Ok(outputs([("map", Value::Map(Map::new(grid, cells)))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Calc));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scope<'a>(maps: &'a [(&'a str, Vec<f64>)], w: usize, h: usize) -> Scope<'a> {
        Scope {
            width: w,
            height: h,
            maps: maps.iter().map(|(n, v)| (*n, v.as_slice())).collect(),
            numbers: [("k", 2.0)].into(),
            layers: BTreeMap::new(),
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
}
