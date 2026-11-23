//! The formula of a step selection function (R's, in a string), as the library's `_issf.py` reads it: the subset of
//! R's formulas step selection uses, parsed to the terms and the columns of R's `model.matrix` (treatment
//! contrasts with the rule of the margins, `-1`, `+0` and `+1` ignored, the strata sorted as `factor()` sorts
//! them), and the values of each column cell by cell as the library computes them: where it calls its C++ for the
//! bits of R's `log`, `exp`, `sin`, `cos`, `log1p`, `sqrt` and `^`, so do we (through the shim); the rest is IEEE.

use serde::{Deserialize, Serialize};
use terraflux_ls_sys::ffi;
use terraflux_raster::vector::{Column, Vector};

use crate::Table;

pub type Res<T> = std::result::Result<T, String>;

// ------------------------------------------------------------------------------------------------ the numbers

/// Numbers in JSON with the values JSON has none for: NaN is `null`, infinities the texts "inf" and "-inf".
pub mod nanjson {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    #[derive(Serialize, Deserialize)]
    #[serde(untagged)]
    enum N {
        F(f64),
        S(String),
        Null(()),
    }

    fn to(v: f64) -> N {
        if v.is_nan() {
            N::Null(())
        } else if v.is_infinite() {
            N::S(if v > 0.0 { "inf" } else { "-inf" }.into())
        } else {
            N::F(v)
        }
    }

    fn from(n: N) -> f64 {
        match n {
            N::F(v) => v,
            N::S(s) if s == "-inf" => f64::NEG_INFINITY,
            N::S(_) => f64::INFINITY,
            N::Null(()) => f64::NAN,
        }
    }

    /// A number.
    pub mod one {
        use super::*;

        pub fn serialize<S: Serializer>(v: &f64, s: S) -> Result<S::Ok, S::Error> {
            to(*v).serialize(s)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<f64, D::Error> {
            Ok(from(N::deserialize(d)?))
        }
    }

    /// A list of numbers.
    pub mod many {
        use super::*;

        pub fn serialize<S: Serializer>(v: &[f64], s: S) -> Result<S::Ok, S::Error> {
            v.iter().map(|&x| to(x)).collect::<Vec<_>>().serialize(s)
        }

        pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<f64>, D::Error> {
            Ok(Vec::<N>::deserialize(d)?.into_iter().map(from).collect())
        }
    }
}

/// Python's `format(v, ".{prec}g")`.
pub fn fmt_g(v: f64, prec: usize) -> String {
    if v.is_nan() {
        return "nan".into();
    }
    if v.is_infinite() {
        return if v > 0.0 { "inf" } else { "-inf" }.into();
    }
    fn strip(s: &str) -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() }
    }
    if v == 0.0 {
        return if v.is_sign_negative() { "-0" } else { "0" }.into();
    }
    let e = format!("{:.*e}", prec - 1, v);
    let (mant, exp) = e.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if exp < -4 || exp >= prec as i32 {
        format!("{}e{}{:02}", strip(mant), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        strip(&format!("{:.*}", (prec as i32 - 1 - exp) as usize, v))
    }
}

/// The text of a number in a label: a whole number without its point, else 15 significant digits.
fn num_text(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 { format!("{}", v as i64) } else { fmt_g(v, 15) }
}

/// Python's repr of a text.
fn py_str(s: &str) -> String {
    let q = if s.contains('\'') && !s.contains('"') { '"' } else { '\'' };
    let mut o = String::from(q);
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if c == q => {
                o.push('\\');
                o.push(c)
            }
            c => o.push(c),
        }
    }
    o.push(q);
    o
}

/// Python's repr of a float.
fn py_float(v: f64) -> String {
    if v.is_finite() && v == v.trunc() && v.abs() < 1e16 { format!("{v:.1}") } else { format!("{v:?}") }
}

fn py_list(items: &[String]) -> String {
    format!("[{}]", items.join(", "))
}

// ------------------------------------------------------------------------------------------------ the formula

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(String),
    Name(String),
    Op(String),
}

impl Tok {
    fn text(&self) -> &str {
        match self {
            Tok::Num(s) | Tok::Name(s) | Tok::Op(s) => s,
        }
    }
}

/// The end of the number that starts at `i` (`\d+\.?\d*` or `\.\d+`, then an exponent), if there is one.
fn number_end(c: &[char], i: usize) -> Option<usize> {
    let digit = |k: usize| c.get(k).is_some_and(|c| c.is_ascii_digit());
    let mut j;
    if digit(i) {
        j = i;
        while digit(j) {
            j += 1;
        }
        if c.get(j) == Some(&'.') {
            j += 1;
        }
        while digit(j) {
            j += 1;
        }
    } else if c.get(i) == Some(&'.') && digit(i + 1) {
        j = i + 1;
        while digit(j) {
            j += 1;
        }
    } else {
        return None;
    }
    if matches!(c.get(j), Some('e' | 'E')) {
        let mut k = j + 1;
        if matches!(c.get(k), Some('+' | '-')) {
            k += 1;
        }
        if digit(k) {
            while digit(k) {
                k += 1;
            }
            j = k;
        }
    }
    Some(j)
}

fn tokens(text: &str) -> Res<Vec<Tok>> {
    let c: Vec<char> = text.trim_end().chars().collect();
    let (mut out, mut pos) = (Vec::new(), 0);
    while pos < c.len() {
        let mut i = pos;
        while i < c.len() && c[i].is_whitespace() {
            i += 1;
        }
        let word = |k: usize| c[k].is_ascii_alphanumeric() || c[k] == '.' || c[k] == '_';
        if let Some(j) = number_end(&c, i) {
            out.push(Tok::Num(c[i..j].iter().collect()));
            pos = j;
        } else if i < c.len() && (c[i].is_ascii_alphabetic() || c[i] == '.') {
            let mut j = i + 1;
            while j < c.len() && word(j) {
                j += 1;
            }
            out.push(Tok::Name(c[i..j].iter().collect()));
            pos = j;
        } else if i < c.len() && c[i] == '`' && c[i + 1..].contains(&'`') && c.get(i + 1) != Some(&'`') {
            let j = i + 1 + c[i + 1..].iter().position(|&x| x == '`').unwrap();
            out.push(Tok::Name(c[i + 1..j].iter().collect()));
            pos = j + 1;
        } else if i + 4 <= c.len() && c[i..].starts_with(&['%', 'i', 'n', '%']) {
            out.push(Tok::Op("%in%".into()));
            pos = i + 4;
        } else if i < c.len() && "~+-*/^:(),".contains(c[i]) {
            out.push(Tok::Op(c[i].to_string()));
            pos = i + 1;
        } else {
            let end = (pos + 12).min(c.len());
            let seen: String = c[pos..end].iter().collect();
            return Err(format!("formula: cannot read {} (position {})", py_str(&seen), pos + 1));
        }
    }
    Ok(out)
}

/// The expression of a formula's right side.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Node {
    Num(f64),
    Name(String),
    Call(String, Vec<Node>),
    Paren(Box<Node>),
    Neg(Box<Node>),
    Pos(Box<Node>),
    Bin(char, Box<Node>, Box<Node>),
}

/// binding powers (left, right): R's + - below * / below : below unary minus below ^ (right to left)
fn infix(op: &str) -> Option<(u8, u8)> {
    Some(match op {
        "+" | "-" => (10, 11),
        "*" | "/" => (20, 21),
        ":" => (30, 31),
        "^" => (50, 49),
        _ => return None,
    })
}

const UNARY: u8 = 40;
const MATH: [&str; 6] = ["log", "exp", "sqrt", "sin", "cos", "log1p"];
const SUPPORTED: &str = "log, exp, sqrt, sin, cos, log1p, abs, I, factor and strata";

struct Parser {
    t: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.i)
    }

    fn take(&mut self) -> Option<Tok> {
        let t = self.t.get(self.i).cloned();
        self.i += 1;
        t
    }

    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if o == op)
    }

    fn expect(&mut self, op: &str) -> Res<()> {
        match self.take() {
            Some(Tok::Op(o)) if o == op => Ok(()),
            other => Err(format!(
                "formula: expected {}, found {}",
                py_str(op),
                other.map_or("None".to_string(), |t| py_str(t.text()))
            )),
        }
    }

    fn expr(&mut self, rbp: u8) -> Res<Node> {
        let mut left = self.prefix()?;
        loop {
            let v = match self.peek() {
                Some(Tok::Op(v)) if infix(v).is_some() || v == "%in%" => v.clone(),
                _ => return Ok(left),
            };
            if v == "%in%" {
                return Err("formula: %in% is not supported".into());
            }
            let (lbp, nbp) = infix(&v).unwrap();
            if lbp <= rbp {
                return Ok(left);
            }
            self.take();
            let right = self.expr(nbp)?;
            left = Node::Bin(v.chars().next().unwrap(), Box::new(left), Box::new(right));
        }
    }

    fn prefix(&mut self) -> Res<Node> {
        match self.take() {
            Some(Tok::Num(v)) => Ok(Node::Num(v.parse().unwrap_or(f64::NAN))),
            Some(Tok::Name(v)) => {
                if self.is_op("(") {
                    self.take();
                    let mut args = Vec::new();
                    if !self.is_op(")") {
                        loop {
                            args.push(self.expr(0)?);
                            if self.is_op(",") {
                                self.take();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect(")")?;
                    Ok(Node::Call(v, args))
                } else {
                    Ok(Node::Name(v))
                }
            }
            Some(Tok::Op(v)) if v == "(" => {
                let e = self.expr(0)?;
                self.expect(")")?;
                Ok(Node::Paren(Box::new(e)))
            }
            Some(Tok::Op(v)) if v == "+" || v == "-" => {
                let e = Box::new(self.expr(UNARY)?);
                Ok(if v == "-" { Node::Neg(e) } else { Node::Pos(e) })
            }
            None => Err("formula: the expression ends where a term is expected".into()),
            Some(t) => Err(format!("formula: unexpected {}", py_str(t.text()))),
        }
    }
}

/// R's deparse of the call (the labels of terms): spaces around + - *, none around / ^ :.
pub fn deparse(a: &Node) -> String {
    match a {
        Node::Num(v) => num_text(*v),
        Node::Name(n) => n.clone(),
        Node::Call(n, args) => format!("{n}({})", args.iter().map(deparse).collect::<Vec<_>>().join(", ")),
        Node::Paren(e) => format!("({})", deparse(e)),
        Node::Neg(e) => format!("-{}", deparse(e)),
        Node::Pos(e) => format!("+{}", deparse(e)),
        Node::Bin(op, l, r) => {
            let sp = if "+-*".contains(*op) { " " } else { "" };
            format!("{}{sp}{op}{sp}{}", deparse(l), deparse(r))
        }
    }
}

/// The values a column of numbers or a factor holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Num,
    Factor,
}

/// A level of a factor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Level {
    Num(#[serde(with = "nanjson::one")] f64),
    Text(String),
    Bool(bool),
}

impl Level {
    /// R's name of the level.
    pub fn label(&self) -> String {
        match self {
            Level::Bool(b) => if *b { "TRUE" } else { "FALSE" }.into(),
            Level::Num(v) => num_text(*v),
            Level::Text(s) => s.clone(),
        }
    }
}

/// A variable of the formula (a column or a transformation of columns), by the label R gives it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Var {
    pub label: String,
    pub ast: Node,
    pub kind: Kind,
    /// a `factor(...)`'s argument
    pub inner: Option<Node>,
    pub levels: Vec<Level>,
    pub labels: Vec<String>,
}

/// A parsed formula: the response, its variables (by first appearance) and the terms (sorted variable indices).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Spec {
    pub text: String,
    pub response: String,
    pub variables: Vec<Var>,
    pub terms: Vec<Vec<usize>>,
    /// the index of the `strata(...)` variable
    pub strata: Option<usize>,
}

fn check_call(name: &str, args: &[Node]) -> Res<()> {
    if MATH.contains(&name) || name == "abs" {
        if args.len() != 1 {
            return Err(format!("formula: {name}() takes one argument ({name}(x, base) is not supported)"));
        }
    } else if ["I", "factor", "strata"].contains(&name) {
        if args.len() != 1 {
            return Err(format!(
                "formula: {name}() takes one argument{}",
                if name == "strata" { " (combine the columns of the strata first)" } else { "" }
            ));
        }
    } else {
        return Err(format!(
            "formula: the function {name}() is not supported (only {SUPPORTED}; tan, poly, ns, scale, offset and \
             cluster are left out)"
        ));
    }
    Ok(())
}

/// A name R takes as it is: letters, digits, dots and underscores, not starting with a digit or an underscore.
pub fn is_syntactic(s: &str) -> bool {
    let mut c = s.chars();
    c.next().is_some_and(|c| c.is_ascii_alphabetic() || c == '.')
        && c.all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

struct Expander {
    variables: Vec<Var>,
}

impl Expander {
    fn var(&mut self, node: &Node) -> Vec<usize> {
        let label = deparse(node);
        let i = match self.variables.iter().position(|v| v.label == label) {
            Some(i) => i,
            None => {
                self.variables.push(Var {
                    label,
                    ast: node.clone(),
                    kind: Kind::Num,
                    inner: None,
                    levels: vec![],
                    labels: vec![],
                });
                self.variables.len() - 1
            }
        };
        vec![i]
    }

    fn expand(&mut self, node: &Node) -> Res<Vec<Vec<usize>>> {
        fn cross(a: &[Vec<usize>], b: &[Vec<usize>]) -> Vec<Vec<usize>> {
            let mut out = Vec::new();
            for x in a {
                for y in b {
                    let mut t: Vec<usize> = x.iter().chain(y).copied().collect();
                    t.sort_unstable();
                    t.dedup();
                    out.push(t);
                }
            }
            out
        }
        match node {
            Node::Num(v) => {
                if *v != 0.0 && *v != 1.0 {
                    return Err(format!("formula: the number {} is not a term", num_text(*v)));
                }
                Ok(vec![]) // 0 and 1 are the intercept, which the fit always has (as coxph)
            }
            Node::Paren(e) | Node::Pos(e) => self.expand(e),
            Node::Neg(e) => {
                if matches!(**e, Node::Num(_)) {
                    return Ok(vec![]);
                }
                Err("formula: a minus sign takes out a term or the intercept (- 1): '-x' is not a term".into())
            }
            Node::Name(_) => Ok(vec![self.var(node)]),
            Node::Call(name, args) => {
                check_call(name, args)?;
                Ok(vec![self.var(node)])
            }
            Node::Bin(op, left, right) => match op {
                '+' => {
                    let a = self.expand(left)?;
                    let b = self.expand(right)?;
                    let mut out = a.clone();
                    out.extend(b.into_iter().filter(|t| !a.contains(t)));
                    Ok(out)
                }
                '-' => {
                    let a = self.expand(left)?;
                    let b = if matches!(**right, Node::Num(_)) { vec![] } else { self.expand(right)? };
                    Ok(a.into_iter().filter(|t| !b.contains(t)).collect())
                }
                ':' => {
                    let a = self.expand(left)?;
                    let b = self.expand(right)?;
                    Ok(cross(&a, &b))
                }
                '*' => {
                    let a = self.expand(left)?;
                    let b = self.expand(right)?;
                    let mut terms = a.clone();
                    terms.extend(b.iter().filter(|t| !a.contains(t)).cloned());
                    for t in cross(&a, &b) {
                        if !terms.contains(&t) {
                            terms.push(t);
                        }
                    }
                    Ok(terms)
                }
                op => Err(format!(
                    "formula: {} is not supported between terms (write I(a {op} b))",
                    py_str(&op.to_string())
                )),
            },
        }
    }
}

/// R's formula in a string, as the library parses it.
pub fn parse_formula(text: &str) -> Res<Spec> {
    let Some((lhs, rhs)) = text.split_once('~') else {
        return Err("formula: a string such as 'case_ ~ elevation + log(sl_) + strata(step_id_)'".into());
    };
    let resp = lhs.trim();
    let quoted =
        resp.len() > 2 && resp.starts_with('`') && resp.ends_with('`') && !resp[1..resp.len() - 1].contains('`');
    if !(is_syntactic(resp) || quoted) {
        return Err(format!("formula: the left side must be the name of the column of cases, not {}", py_str(resp)));
    }
    let resp = resp.trim_matches('`').to_string();
    let mut p = Parser { t: tokens(rhs)?, i: 0 };
    let ast = p.expr(0)?;
    if let Some(t) = p.peek() {
        return Err(format!("formula: unexpected {}", py_str(t.text())));
    }
    let mut ex = Expander { variables: vec![] };
    let mut terms: Vec<Vec<usize>> = Vec::new();
    for t in ex.expand(&ast)? {
        if !terms.contains(&t) {
            terms.push(t);
        }
    }
    terms.sort_by_key(|t| t.len()); // main effects first, then the interactions (R's terms, keep.order = FALSE)
    let s: Vec<usize> = ex
        .variables
        .iter()
        .enumerate()
        .filter(|(_, v)| matches!(&v.ast, Node::Call(n, _) if n == "strata"))
        .map(|(i, _)| i)
        .collect();
    if s.len() > 1 {
        return Err("formula: only one strata(...) is allowed".into());
    }
    if let Some(&s0) = s.first()
        && terms.iter().any(|t| t.contains(&s0) && t.len() > 1)
    {
        return Err("formula: strata(...) cannot be in an interaction".into());
    }
    Ok(Spec {
        text: text.trim().to_string(),
        response: resp,
        variables: ex.variables,
        terms,
        strata: s.first().copied(),
    })
}

/// The names of the columns an expression reads.
pub fn columns_of(node: &Node) -> Vec<String> {
    match node {
        Node::Name(n) => vec![n.clone()],
        Node::Call(_, args) => args.iter().flat_map(columns_of).collect(),
        Node::Paren(e) | Node::Neg(e) | Node::Pos(e) => columns_of(e),
        Node::Bin(_, l, r) => columns_of(l).into_iter().chain(columns_of(r)).collect(),
        Node::Num(_) => vec![],
    }
}

// --------------------------------------------------------------------------------------------- the values

/// A column of a data frame: numbers (NaN where one is missing), logicals or text (None: missing).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Col {
    Num(#[serde(with = "nanjson::many")] Vec<f64>),
    Bool(Vec<Option<bool>>),
    Text(Vec<Option<String>>),
}

impl Col {
    pub fn len(&self) -> usize {
        match self {
            Col::Num(v) => v.len(),
            Col::Bool(v) => v.len(),
            Col::Text(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn select(&self, rows: &[usize]) -> Col {
        match self {
            Col::Num(v) => Col::Num(rows.iter().map(|&i| v[i]).collect()),
            Col::Bool(v) => Col::Bool(rows.iter().map(|&i| v[i]).collect()),
            Col::Text(v) => Col::Text(rows.iter().map(|&i| v[i].clone()).collect()),
        }
    }

    fn is_na(&self, i: usize) -> bool {
        match self {
            Col::Num(v) => v[i].is_nan(),
            Col::Bool(v) => v[i].is_none(),
            Col::Text(v) => v[i].is_none(),
        }
    }

    /// The value as the level it would be.
    fn level(&self, i: usize) -> Level {
        match self {
            Col::Num(v) => Level::Num(v[i]),
            Col::Bool(v) => Level::Bool(v[i].unwrap_or(false)),
            Col::Text(v) => Level::Text(v[i].clone().unwrap_or_default()),
        }
    }
}

/// A data frame: named columns of one length.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub names: Vec<String>,
    pub cols: Vec<Col>,
}

impl Frame {
    pub fn len(&self) -> usize {
        self.cols.first().map_or(0, Col::len)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get(&self, name: &str) -> Option<&Col> {
        self.names.iter().position(|n| n == name).map(|i| &self.cols[i])
    }

    pub fn push(&mut self, name: &str, col: Col) {
        match self.names.iter().position(|n| n == name) {
            Some(i) => self.cols[i] = col,
            None => {
                self.names.push(name.to_string());
                self.cols.push(col);
            }
        }
    }

    pub fn select(&self, rows: &[usize]) -> Frame {
        Frame { names: self.names.clone(), cols: self.cols.iter().map(|c| c.select(rows)).collect() }
    }

    /// The attributes of features: numbers and texts ("" is a missing text).
    pub fn from_vector(v: &Vector) -> Frame {
        let mut f = Frame::default();
        for (n, c) in &v.attributes {
            f.push(
                n,
                match c {
                    Column::Numbers(x) => Col::Num(x.clone()),
                    Column::Texts(x) => Col::Text(x.iter().map(|s| (!s.is_empty()).then(|| s.clone())).collect()),
                },
            );
        }
        f
    }

    /// A table: its columns of names are text.
    pub fn from_table(t: &Table) -> Frame {
        let mut f = Frame::default();
        for (j, n) in t.columns.iter().enumerate() {
            let v: Vec<f64> = t.rows.iter().map(|r| r[j]).collect();
            f.push(
                n,
                if t.is_text(n) {
                    Col::Text(v.iter().map(|&x| t.label(n, x).map(str::to_string)).collect())
                } else {
                    Col::Num(v)
                },
            );
        }
        f
    }
}

pub fn column<'a>(df: &'a Frame, name: &str) -> Res<&'a Col> {
    df.get(name).ok_or_else(|| format!("the column {} of the formula is not in the data", py_str(name)))
}

/// The values of an arithmetic expression, one per row.
fn arith(node: &Node, df: &Frame, n: usize) -> Res<Vec<f64>> {
    match node {
        Node::Num(v) => Ok(vec![*v; n]),
        Node::Paren(e) | Node::Pos(e) => arith(e, df, n),
        Node::Neg(e) => Ok(arith(e, df, n)?.into_iter().map(|v| -v).collect()),
        Node::Name(name) => match column(df, name)? {
            Col::Num(v) => Ok(v.clone()),
            Col::Bool(v) => Ok(v.iter().map(|b| b.map_or(f64::NAN, |b| f64::from(u8::from(b)))).collect()),
            Col::Text(_) => {
                Err(format!("the column {} is not a number: it cannot be used in an expression", py_str(name)))
            }
        },
        Node::Call(name, args) => {
            let Some(arg) = args.first() else {
                return Err(format!("formula: {name}() takes one argument"));
            };
            if name == "I" {
                return arith(arg, df, n);
            }
            if name == "abs" {
                return Ok(arith(arg, df, n)?.into_iter().map(f64::abs).collect());
            }
            if MATH.contains(&name.as_str()) {
                let x = arith(arg, df, n)?;
                return ffi::move_issf_fn(name, &x).map_err(|e| e.what().to_string());
            }
            Err(format!("formula: {name}() cannot be inside an expression"))
        }
        Node::Bin(op, left, right) => {
            let a = arith(left, df, n)?;
            let b = arith(right, df, n)?;
            let f = |g: fn(f64, f64) -> f64| a.iter().zip(&b).map(|(&x, &y)| g(x, y)).collect();
            match op {
                '+' => Ok(f(|x, y| x + y)),
                '-' => Ok(f(|x, y| x - y)),
                '*' => Ok(f(|x, y| x * y)),
                '/' => Ok(f(|x, y| x / y)),
                '^' => ffi::move_issf_pow(&a, &b).map_err(|e| e.what().to_string()),
                op => Err(format!("formula: {} is not supported inside an expression", py_str(&op.to_string()))),
            }
        }
    }
}

/// The levels of factor(x): False and True for a logical, else the sorted values.
fn levels_of(raw: &Col) -> Vec<Level> {
    match raw {
        Col::Bool(_) => vec![Level::Bool(false), Level::Bool(true)],
        Col::Num(v) => {
            let mut x: Vec<f64> = v.iter().copied().filter(|x| !x.is_nan()).collect();
            x.sort_by(f64::total_cmp);
            x.dedup_by(|a, b| a == b);
            x.into_iter().map(Level::Num).collect()
        }
        Col::Text(v) => {
            let mut x: Vec<&String> = v.iter().flatten().collect();
            x.sort();
            x.dedup();
            x.into_iter().map(|s| Level::Text(s.clone())).collect()
        }
    }
}

fn level_repr(l: &Level) -> String {
    match l {
        Level::Num(v) => py_float(*v),
        Level::Text(s) => py_str(s),
        Level::Bool(b) => if *b { "True" } else { "False" }.into(),
    }
}

/// The 0-based level of each value, -1 for a missing one; a value that is none of the levels is an error.
fn codes_of(raw: &Col, levels: &[Level], label: &str) -> Res<Vec<i64>> {
    let mut out = Vec::with_capacity(raw.len());
    let mut new: Vec<Level> = Vec::new();
    for i in 0..raw.len() {
        if raw.is_na(i) {
            out.push(-1);
            continue;
        }
        let l = raw.level(i);
        match levels.iter().position(|x| *x == l) {
            Some(k) => out.push(k as i64),
            None => {
                if !new.contains(&l) {
                    new.push(l);
                }
                out.push(-1);
            }
        }
    }
    if !new.is_empty() {
        let new: Vec<String> = new.iter().map(level_repr).collect();
        let have: Vec<String> = levels.iter().map(|l| py_str(&l.label())).collect();
        return Err(format!("{label}: new levels {} (the fit has {})", py_list(&new), py_list(&have)));
    }
    Ok(out)
}

/// The values a factor() is made of: a column as it is, or the values of an expression.
fn raw_of(node: &Node, df: &Frame, n: usize) -> Res<Col> {
    match node {
        Node::Name(name) => Ok(column(df, name)?.clone()),
        Node::Paren(e) => raw_of(e, df, n),
        _ => Ok(Col::Num(arith(node, df, n)?)),
    }
}

/// Decides at the fit whether a variable is a number or a factor, and its levels.
pub fn set_kind(v: &mut Var, df: &Frame, n: usize) -> Res<()> {
    match &v.ast {
        Node::Call(name, args) if name == "factor" => {
            v.kind = Kind::Factor;
            v.inner = args.first().cloned();
        }
        Node::Name(name) => {
            let number = matches!(column(df, name)?, Col::Num(_));
            v.kind = if number { Kind::Num } else { Kind::Factor };
            v.inner = if number { None } else { Some(v.ast.clone()) };
        }
        _ => v.kind = Kind::Num,
    }
    if v.kind == Kind::Factor {
        let raw = raw_of(v.inner.as_ref().ok_or("formula: factor() takes one argument")?, df, n)?;
        v.levels = levels_of(&raw);
        v.labels = v.levels.iter().map(Level::label).collect();
    }
    Ok(())
}

/// strata(x): the codes of factor(x) (levels in order: numbers by value, text by sort), -1 for a missing one.
pub fn strata_codes(raw: &Col, label: &str) -> Res<Vec<i64>> {
    codes_of(raw, &levels_of(raw), label)
}

/// The raw values of the strata: the column or the expression inside `strata(...)`.
pub fn strata_raw(sv: &Node, df: &Frame, n: usize) -> Res<Col> {
    raw_of(sv, df, n)
}

// --------------------------------------------------------------------------------------------- the terms

/// What the formula needs to build the model matrix of a fit, or of new data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Terms {
    pub spec: Spec,
    /// the names of the columns of the model matrix
    pub columns: Vec<String>,
}

impl Terms {
    /// The model matrix of `df` (columns of n values, NaN where a value is missing) and its column names.
    pub fn design(&self, df: &Frame, n: usize) -> Res<(Vec<Vec<f64>>, Vec<String>)> {
        let sp = &self.spec;
        enum Val {
            Num(Vec<f64>),
            Codes(Vec<i64>),
        }
        let mut vals: Vec<Option<Val>> = Vec::with_capacity(sp.variables.len());
        for (i, v) in sp.variables.iter().enumerate() {
            if Some(i) == sp.strata {
                vals.push(None);
            } else if v.kind == Kind::Num {
                vals.push(Some(Val::Num(arith(&v.ast, df, n)?)));
            } else {
                let inner = v.inner.as_ref().ok_or("formula: factor() takes one argument")?;
                vals.push(Some(Val::Codes(codes_of(&raw_of(inner, df, n)?, &v.levels, &v.label)?)));
            }
        }
        let (mut cols, mut names) = (Vec::new(), Vec::new());
        for t in &sp.terms {
            if sp.strata.is_some_and(|s| *t == [s]) {
                continue;
            }
            let mut part: Vec<(String, Option<Vec<f64>>)> = vec![(String::new(), None)];
            for &i in t {
                let v = &sp.variables[i];
                let vcols: Vec<(String, Vec<f64>)> = match vals[i].as_ref().unwrap() {
                    Val::Num(x) if v.kind == Kind::Num => vec![(v.label.clone(), x.clone())],
                    Val::Num(_) => unreachable!(),
                    Val::Codes(c) => {
                        let margin: Vec<usize> = t.iter().copied().filter(|&j| j != i).collect();
                        // contrasts, or an indicator of each level
                        let first = usize::from(margin.is_empty() || sp.terms.contains(&margin));
                        if v.levels.len() < first + 1 {
                            return Err(format!("{}: a factor with fewer than 2 levels has no contrasts", v.label));
                        }
                        (first..v.levels.len())
                            .map(|k| {
                                (
                                    format!("{}{}", v.label, v.labels[k]),
                                    c.iter()
                                        .map(|&c| if c < 0 { f64::NAN } else { f64::from(u8::from(c == k as i64)) })
                                        .collect(),
                                )
                            })
                            .collect()
                    }
                };
                let mut next = Vec::with_capacity(vcols.len() * part.len());
                for (vn, vv) in &vcols {
                    for (pn, pv) in &part {
                        let name = if pn.is_empty() { vn.clone() } else { format!("{pn}:{vn}") };
                        let col = match pv {
                            None => vv.clone(),
                            Some(p) => p.iter().zip(vv).map(|(a, b)| a * b).collect(),
                        };
                        next.push((name, Some(col)));
                    }
                }
                part = next;
            }
            for (nm, arr) in part {
                names.push(nm);
                cols.push(arr.unwrap_or_default());
            }
        }
        Ok((cols, names))
    }
}

/// The model matrix of `df` by a formula text (the kinds of its variables read from `df`): the columns of n values and
/// their names, as the library's `fit.model_matrix(df)` of a fit on `df`.
pub fn model_matrix(df: &Frame, formula: &str) -> Res<(Vec<Vec<f64>>, Vec<String>)> {
    let mut sp = parse_formula(formula)?;
    let n = df.len();
    for i in 0..sp.variables.len() {
        if Some(i) != sp.strata {
            let mut v = sp.variables[i].clone();
            set_kind(&mut v, df, n)?;
            sp.variables[i] = v;
        }
    }
    Terms { spec: sp, columns: vec![] }.design(df, n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec(f: &str) -> Spec {
        parse_formula(f).unwrap()
    }

    #[test]
    fn g_format_is_pythons() {
        assert_eq!(fmt_g(0.1, 15), "0.1");
        assert_eq!(fmt_g(1.5e20, 15), "1.5e+20");
        assert_eq!(fmt_g(1e-5, 15), "1e-05");
        assert_eq!(fmt_g(123456.789, 15), "123456.789");
        assert_eq!(num_text(3.0), "3");
        assert_eq!(num_text(2.5), "2.5");
    }

    #[test]
    fn the_terms_of_a_formula_in_r_order() {
        let s = spec("case_ ~ elevation:log(sl_) + sl_ + log(sl_) + strata(step_id_)");
        let labels: Vec<String> = s
            .terms
            .iter()
            .map(|t| t.iter().map(|&i| s.variables[i].label.clone()).collect::<Vec<_>>().join(":"))
            .collect();
        assert_eq!(labels, ["sl_", "log(sl_)", "strata(step_id_)", "elevation:log(sl_)"]);
        assert_eq!(s.strata, Some(3));
        assert_eq!(s.response, "case_");
    }

    #[test]
    fn the_intercept_terms_are_ignored() {
        let a = spec("case_ ~ x - 1 + strata(s)");
        let b = spec("case_ ~ 0 + x + strata(s)");
        let c = spec("case_ ~ 1 + x + strata(s)");
        assert_eq!(a.terms, b.terms);
        assert_eq!(a.terms, c.terms);
    }

    #[test]
    fn errors_are_the_library_s() {
        let e = |f: &str| parse_formula(f).unwrap_err();
        assert_eq!(e("case_ x"), "formula: a string such as 'case_ ~ elevation + log(sl_) + strata(step_id_)'");
        assert_eq!(e("1 + 2 ~ x"), "formula: the left side must be the name of the column of cases, not '1 + 2'");
        assert_eq!(e("case_ ~ x $ y"), "formula: cannot read ' $ y' (position 3)");
        assert_eq!(e("case_ ~ (x"), "formula: expected ')', found None");
        assert_eq!(e("case_ ~ x +"), "formula: the expression ends where a term is expected");
        assert_eq!(e("case_ ~ x )"), "formula: unexpected ')'");
        assert_eq!(e("case_ ~ x %in% y"), "formula: %in% is not supported");
        assert_eq!(e("case_ ~ 2 + x"), "formula: the number 2 is not a term");
        assert_eq!(e("case_ ~ x / y"), "formula: '/' is not supported between terms (write I(a / b))");
        assert_eq!(
            e("case_ ~ tan(x)"),
            "formula: the function tan() is not supported (only log, exp, sqrt, sin, cos, log1p, abs, I, factor and \
             strata; tan, poly, ns, scale, offset and cluster are left out)"
        );
        assert_eq!(e("case_ ~ log(x, 2)"), "formula: log() takes one argument (log(x, base) is not supported)");
        assert_eq!(
            e("case_ ~ strata(a, b)"),
            "formula: strata() takes one argument (combine the columns of the strata first)"
        );
        assert_eq!(e("case_ ~ strata(a) + strata(b)"), "formula: only one strata(...) is allowed");
        assert_eq!(e("case_ ~ strata(a):x"), "formula: strata(...) cannot be in an interaction");
        assert_eq!(
            e("case_ ~ -x"),
            "formula: a minus sign takes out a term or the intercept (- 1): '-x' is not a term"
        );
    }

    fn frame() -> Frame {
        let mut f = Frame::default();
        f.push("x", Col::Num(vec![1.0, 2.0, 3.0, 4.0]));
        f.push("lu", Col::Text(["b", "a", "c", "a"].iter().map(|s| Some(s.to_string())).collect()));
        f
    }

    fn matrix(f: &str, df: &Frame) -> (Vec<Vec<f64>>, Vec<String>) {
        let mut sp = spec(f);
        let n = df.len();
        for i in 0..sp.variables.len() {
            if Some(i) != sp.strata {
                let mut v = sp.variables[i].clone();
                set_kind(&mut v, df, n).unwrap();
                sp.variables[i] = v;
            }
        }
        Terms { spec: sp, columns: vec![] }.design(df, n).unwrap()
    }

    #[test]
    fn factors_take_treatment_contrasts_with_the_margin_rule() {
        let df = frame();
        let (m, names) = matrix("y ~ lu + x + lu:x", &df);
        assert_eq!(names, ["lub", "luc", "x", "lub:x", "luc:x"]);
        assert_eq!(m[0], [1.0, 0.0, 0.0, 0.0]); // b
        assert_eq!(m[1], [0.0, 0.0, 1.0, 0.0]); // c
        assert_eq!(m[3], [1.0, 0.0, 0.0, 0.0]); // b:x
        // without the main effect of the factor, an indicator of each of its levels
        let (_, names) = matrix("y ~ lu:x", &df);
        assert_eq!(names, ["lua:x", "lub:x", "luc:x"]);
        // with the main effect of x (the margin), contrasts; the variables in the order they first appear
        let (_, names) = matrix("y ~ x:lu + lu + x", &df);
        assert_eq!(names, ["lub", "luc", "x", "x:lub", "x:luc"]);
    }

    #[test]
    fn the_cells_are_the_arithmetic_of_the_library() {
        let df = frame();
        let (m, names) = matrix("y ~ log(x) + I(x^2) + sqrt(x) - 1", &df);
        assert_eq!(names, ["log(x)", "I(x^2)", "sqrt(x)"]);
        assert_eq!(m[1], [1.0, 4.0, 9.0, 16.0]);
        assert_eq!(m[0][0], 0.0);
        assert!((m[0][1] - 2f64.ln()).abs() < 1e-15);
        // a missing value stays missing
        let mut df = frame();
        df.push("x", Col::Num(vec![1.0, f64::NAN, 3.0, 4.0]));
        let (m, _) = matrix("y ~ x", &df);
        assert!(m[0][1].is_nan());
    }

    #[test]
    fn a_new_level_is_an_error() {
        let df = frame();
        let sp = {
            let mut sp = spec("y ~ lu");
            let mut v = sp.variables[0].clone();
            set_kind(&mut v, &df, 4).unwrap();
            sp.variables[0] = v;
            sp
        };
        let mut other = Frame::default();
        other.push("lu", Col::Text(vec![Some("z".into())]));
        let e = Terms { spec: sp, columns: vec![] }.design(&other, 1).unwrap_err();
        assert_eq!(e, "lu: new levels ['z'] (the fit has ['a', 'b', 'c'])");
    }
}
