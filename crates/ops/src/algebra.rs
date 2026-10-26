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

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use rayon::prelude::*;
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
    by!(Add, Sub, Mul, Div, Rem, Pow, Lt, Le, Gt, Ge, Eq, Ne, And, Or, Min, Max, Coalesce, AtLeast, AtMost)
}

/// A compiled expression: instructions run over a block of cells at a time, with the registers they write.
#[derive(Debug)]
struct Program {
    ins: Vec<Ins>,
    regs: usize,
    result: Arg,
}

impl Program {
    /// The cells `start..start + out.len()`.
    fn block(&self, maps: &[&[f64]], start: usize, out: &mut [f64], regs: &mut [Vec<f64>]) {
        let len = out.len();
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
        match self.result {
            Arg::Reg(r) => out.copy_from_slice(&regs[r][..len]),
            Arg::Map(m) => out.copy_from_slice(&maps[m][start..start + len]),
            Arg::Num(x) => out.fill(x),
        }
    }

    /// Every cell of the grid, block by block, the blocks in parallel.
    fn run(&self, maps: &[&[f64]], n: usize) -> Vec<f64> {
        match self.result {
            Arg::Num(x) => return vec![x; n],
            Arg::Map(m) => return maps[m].to_vec(),
            Arg::Reg(_) => {}
        }
        let mut out = vec![0.0; n];
        let regs = || vec![vec![0.0; BLOCK.min(n)]; self.regs];
        if n < PARALLEL {
            let mut r = regs();
            for (b, o) in out.chunks_mut(BLOCK).enumerate() {
                self.block(maps, b * BLOCK, o, &mut r);
            }
        } else {
            crate::threads::ensure();
            out.par_chunks_mut(BLOCK).enumerate().for_each_init(regs, |r, (b, o)| self.block(maps, b * BLOCK, o, r));
        }
        out
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

/// Turns an expression into programs: names resolved, numbers folded, a value computed twice computed once, and
/// the neighbourhoods computed whole before the program that reads them.
struct Compiler<'s, 'a> {
    scope: &'s Scope<'a>,
    maps: Vec<Slot<'a>>,
    /// The maps by what they are: an input's name, a layer, a neighbourhood with its radius.
    named: HashMap<String, usize>,
    ins: Vec<Ins>,
    seen: HashMap<(u8, u8, [(u8, u64); 3]), Arg>,
}

impl<'s, 'a> Compiler<'s, 'a> {
    fn new(scope: &'s Scope<'a>) -> Self {
        Compiler { scope, maps: Vec::new(), named: HashMap::new(), ins: Vec::new(), seen: HashMap::new() }
    }

    fn map(&mut self, key: String, cells: impl FnOnce() -> Slot<'a>) -> Arg {
        if let Some(&m) = self.named.get(&key) {
            return Arg::Map(m);
        }
        self.maps.push(cells());
        self.named.insert(key, self.maps.len() - 1);
        Arg::Map(self.maps.len() - 1)
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

    fn lower(&mut self, e: &Expr) -> Result<Arg> {
        Ok(match e {
            Expr::Num(x) => Arg::Num(*x),
            Expr::Var(name) => {
                if let Some(&m) = self.scope.maps.get(name.as_str()) {
                    self.map(name.clone(), || Slot::Input(m))
                } else if let Some(&x) = self.scope.numbers.get(name.as_str()) {
                    Arg::Num(x)
                } else {
                    return Err(Error::msg(format!("no input {name:?} (a map or a number)")));
                }
            }
            Expr::Layer(stack, layer) => match self.scope.layers.get(&(stack.as_str(), layer.as_str())) {
                Some(&m) => self.map(format!("{stack}.{layer}"), || Slot::Input(m)),
                None => return Err(Error::msg(format!("no layer {layer:?} in a stack {stack:?}"))),
            },
            Expr::Unary('-', a) => {
                let a = self.lower(a)?;
                self.one(Op1::Neg, a)
            }
            Expr::Unary(_, a) => {
                let a = self.lower(a)?;
                self.one(Op1::Not, a)
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
                self.two(op, a, b)
            }
            Expr::If(c, a, b) => {
                let (c, a, b) = (self.lower(c)?, self.lower(a)?, self.lower(b)?);
                self.pick(c, a, b)
            }
            Expr::Call(f, args) if f.starts_with("nb") => self.neighbourhood(f, args)?,
            Expr::Call(f, args) => {
                let mut v = args.iter().map(|a| self.lower(a)).collect::<Result<Vec<Arg>>>()?;
                match f.as_str() {
                    "min" | "max" => {
                        let op = if f == "max" { Op2::Max } else { Op2::Min };
                        let mut acc = v[0];
                        for b in &v[1..] {
                            acc = self.two(op, acc, *b);
                        }
                        acc
                    }
                    "coalesce" => self.two(Op2::Coalesce, v[0], v[1]),
                    "clamp" => {
                        let lo = self.two(Op2::AtLeast, v[0], v[1]);
                        self.two(Op2::AtMost, lo, v[2])
                    }
                    _ => {
                        let op = match f.as_str() {
                            "abs" => Op1::Abs,
                            "sqrt" => Op1::Sqrt,
                            "exp" => Op1::Exp,
                            "ln" => Op1::Ln,
                            "log10" => Op1::Log10,
                            "floor" => Op1::Floor,
                            "ceil" => Op1::Ceil,
                            "round" => Op1::Round,
                            "isnull" => Op1::IsNull,
                            _ => unreachable!(),
                        };
                        self.one(op, v.pop().unwrap())
                    }
                }
            }
        })
    }

    /// A neighbourhood: the map it is computed on, by a program of its own, then the windows over it.
    fn neighbourhood(&mut self, f: &str, args: &[Expr]) -> Result<Arg> {
        let r = match &args[1] {
            Expr::Num(r) => *r as usize,
            Expr::Var(n) => match self.scope.numbers.get(n.as_str()) {
                Some(&x) if x >= 0.0 && x.fract() == 0.0 => x as usize,
                Some(&x) => return Err(Error::msg(format!("the radius of {f} ({n} = {x}) is not a whole number"))),
                None => return Err(Error::msg(format!("the radius of {f} ({n}) is not a number input"))),
            },
            _ => unreachable!(),
        };
        let key = format!("{f}({:?}, {r})", args[0]);
        if let Some(&m) = self.named.get(&key) {
            return Ok(Arg::Map(m));
        }
        let (w, h) = (self.scope.width, self.scope.height);
        let p = self.program(&args[0])?;
        let cells = {
            let maps: Vec<&[f64]> = self.maps.iter().map(Slot::cells).collect();
            p.run(&maps, w * h)
        };
        let out = match f {
            "nbmin" => box_extreme::<false>(&cells, w, h, r),
            "nbmax" => box_extreme::<true>(&cells, w, h, r),
            _ => box_sums(&cells, w, h, r, f),
        };
        Ok(self.map(key, || Slot::Made(out)))
    }

    /// The program of an expression, with registers reused once their value is read for the last time.
    fn program(&mut self, e: &Expr) -> Result<Program> {
        let outer = (std::mem::take(&mut self.ins), std::mem::take(&mut self.seen));
        let result = self.lower(e);
        let (ins, _) = (std::mem::replace(&mut self.ins, outer.0), std::mem::replace(&mut self.seen, outer.1));
        let result = result?;
        Ok(allocate(ins, result))
    }
}

/// The instructions the result needs, with the fewest registers: a value's register is reused after its last
/// reading (never by the instruction reading it).
fn allocate(ins: Vec<Ins>, result: Arg) -> Program {
    let n = ins.len();
    let reads = |x: &Ins| x.args().into_iter().flatten().filter_map(|a| if let Arg::Reg(r) = a { Some(r) } else { None });
    let mut needed = vec![false; n];
    if let Arg::Reg(r) = result {
        needed[r] = true;
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
            if last[r] == i && Arg::Reg(r) != result && !free.contains(&reg[r]) {
                free.push(reg[r]);
            }
        }
    }
    let result = match result {
        Arg::Reg(r) => Arg::Reg(reg[r]),
        a => a,
    };
    Program { ins: out, regs, result }
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
        let p = c.program(self)?;
        let maps: Vec<&[f64]> = c.maps.iter().map(Slot::cells).collect();
        Ok(p.run(&maps, s.width * s.height))
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
                Expr::Layer(stack, layer) => match s.layers.get(&(stack.as_str(), layer.as_str())) {
                    Some(m) => V::Cells(m.to_vec()),
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
                layers: [(("s", "x"), x.as_slice())].into(),
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
            layers: BTreeMap::new(),
        };
        for bad in ["c + 1", "s.y * 2", "nbsum(a, z)", "nbmax(a, h)"] {
            let e = parse(bad).unwrap();
            assert_eq!(e.evaluate(&s).err().unwrap().0, e.evaluate_ref(&s).err().unwrap().0);
        }
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
        let program = |text: &str| Compiler::new(&s).program(&parse(text).unwrap()).unwrap();
        let p = program("(a + b) * (a + b) + (a + b)");
        assert_eq!(p.ins.len(), 3, "{p:?}");
        assert_eq!(program("2 * k + 1 ^ 2").result, Arg::Num(5.0));
        assert_eq!(program("if k > 1 then a else b").result, Arg::Map(0));
        assert!(program("if k < 1 then a else b").ins.is_empty());
        // a chain: two registers whatever its length
        let p = program("((((a + 1) * 2 - b) / 3 + a) * b - 1) / 2");
        assert!(p.regs <= 2, "{p:?}");
        // the same neighbourhood twice: computed once
        let mut c = Compiler::new(&s);
        c.program(&parse("nbmax(a, 1) + nbmax(a, 1) * nbmin(a, 1)").unwrap()).unwrap();
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
            layers: [(("s", "x"), x.as_slice())].into(),
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
}
