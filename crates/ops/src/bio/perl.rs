//! The Perl of Biodiverse's spatial conditions (landschaft's bio/_perlexpr.py): numbers, strings, regexes,
//! array references, the condition variables, the operators with Perl's precedence, `my` variables, some maths
//! and calls to the `sp_*` functions with named arguments, read into a small syntax tree.

use std::sync::LazyLock;

use regex::Regex;

use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Ast {
    Num(f64),
    Str(String),
    /// Pattern and flags.
    Regex(String, String),
    List(Vec<Ast>),
    Hash(Vec<Ast>),
    Var(String),
    Idx(String, Box<Ast>),
    Array(String),
    Aref(String),
    Un(String, Box<Ast>),
    Bin(String, Box<Ast>, Box<Ast>),
    Tern(Box<Ast>, Box<Ast>, Box<Ast>),
    /// A call: its arguments (pairs for `key => value`).
    Call(String, Vec<Ast>),
    Seq(Vec<Ast>),
    My(String, Box<Ast>),
    Undef,
    Pair(Box<Ast>, Box<Ast>),
    /// A key waiting for its value (while reading a list).
    Key(Box<Ast>),
}

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Num(f64),
    Str(String),
    Regex(String, String),
    Qw(Vec<String>),
    Var(String),
    ArrVar(String),
    Ident(String),
    Op(&'static str),
    Eof,
}

#[derive(Debug, Clone)]
struct Tok {
    kind: Kind,
    pos: usize,
}

fn fail(m: String) -> Error {
    Error::msg(m)
}

const OPS: [&str; 34] = [
    "<=>", "**", "=>", "==", "!=", "<=", ">=", "&&", "||", "//", "=~", "!~", "->", "<", ">", "+", "-", "*", "/", "%",
    "!", "?", ":", "(", ")", "[", "]", "{", "}", ",", ".", "\\", ";", "=",
];

static NUM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:\d[\d_]*(?:\.[\d_]*)?|\.\d[\d_]*)(?:[eE][+-]?\d+)?").unwrap());
static IDENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z_]\w*(?:::\w+)*").unwrap());
static VAR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z_]\w*").unwrap());
static FLAGS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[msixpn]*").unwrap());

/// The text between the delimiter at `i` and its closing pair, and the index after it.
fn delimited(src: &[char], i: usize) -> Result<(String, usize)> {
    let open = src[i];
    let close = match open {
        '(' => ')',
        '[' => ']',
        '{' => '}',
        '<' => '>',
        c => c,
    };
    let mut depth = 1;
    let mut j = i + 1;
    let mut out = String::new();
    while j < src.len() {
        let ch = src[j];
        if ch == '\\' && j + 1 < src.len() {
            out.push(src[j]);
            out.push(src[j + 1]);
            j += 2;
            continue;
        }
        if open != close && ch == open {
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 {
                return Ok((out, j + 1));
            }
        }
        out.push(ch);
        j += 1;
    }
    Err(fail(format!("unterminated delimiter {open:?} at {i}")))
}

fn unescape_single(s: &str) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\' && i + 1 < c.len() && (c[i + 1] == '\\' || c[i + 1] == '\'') {
            out.push(c[i + 1]);
            i += 2;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    out
}

fn unescape_double(s: &str) -> Result<String> {
    if s.contains('$') || s.contains('@') {
        return Err(fail("interpolated strings are not supported".into()));
    }
    let c: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < c.len() {
        if c[i] == '\\' && i + 1 < c.len() {
            let ch = match c[i + 1] {
                'n' => '\n',
                't' => '\t',
                'r' => '\r',
                '0' => '\0',
                'a' => '\x07',
                'b' => '\x08',
                'f' => '\x0c',
                'v' => '\x0b',
                x @ ('\\' | '\'' | '"') => x,
                x => {
                    out.push('\\');
                    x
                }
            };
            out.push(ch);
            i += 2;
        } else {
            out.push(c[i]);
            i += 1;
        }
    }
    Ok(out)
}

fn tokenize(text: &str) -> Result<Vec<Tok>> {
    let src: Vec<char> = text.chars().collect();
    let n = src.len();
    let rest = |i: usize| -> String { src[i..].iter().collect() };
    let mut toks: Vec<Tok> = Vec::new();
    let mut i = 0;
    while i < n {
        let c = src[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        if c == '#' {
            i = (i..n).find(|&j| src[j] == '\n').map_or(n, |j| j + 1);
            continue;
        }
        // a slash starts a regex after =~ / !~ (and where a term is expected)
        if c == '/' && matches!(toks.last(), Some(Tok { kind: Kind::Op("=~" | "!~" | "(" | "," | "=>"), .. })) {
            let (body, j) = delimited(&src, i)?;
            let f = FLAGS.find(&rest(j)).map_or(String::new(), |m| m.as_str().to_string());
            let len = f.chars().count();
            toks.push(Tok { kind: Kind::Regex(body, f), pos: i });
            i = j + len;
            continue;
        }
        if c.is_ascii_digit() || (c == '.' && i + 1 < n && src[i + 1].is_ascii_digit()) {
            let r = rest(i);
            if let Some(m) = NUM.find(&r) {
                let s = m.as_str().replace('_', "");
                let v: f64 = s.parse().map_err(|_| fail(format!("bad number {s:?} at {i}")))?;
                toks.push(Tok { kind: Kind::Num(v), pos: i });
                i += m.as_str().chars().count();
                continue;
            }
        }
        if c == '\'' {
            let (body, j) = delimited(&src, i)?;
            toks.push(Tok { kind: Kind::Str(unescape_single(&body)), pos: i });
            i = j;
            continue;
        }
        if c == '"' {
            let (body, j) = delimited(&src, i)?;
            toks.push(Tok { kind: Kind::Str(unescape_double(&body)?), pos: i });
            i = j;
            continue;
        }
        if c == '$' || c == '@' {
            let r = rest(i + 1);
            let Some(m) = VAR.find(&r) else { return Err(fail(format!("unexpected {c:?} at {i}"))) };
            let name = m.as_str().to_string();
            let len = name.chars().count();
            toks.push(Tok { kind: if c == '$' { Kind::Var(name) } else { Kind::ArrVar(name) }, pos: i });
            i += 1 + len;
            continue;
        }
        let r = rest(i);
        if let Some(m) = IDENT.find(&r) {
            let word = m.as_str().to_string();
            let j = i + word.chars().count();
            // quote-like operators
            if ["q", "qq", "qr", "m", "qw"].contains(&word.as_str()) && j < n {
                let mut k = j;
                while k < n && src[k].is_whitespace() {
                    k += 1;
                }
                if k < n && !src[k].is_alphanumeric() && !",;=)".contains(src[k]) {
                    let (body, e) = delimited(&src, k)?;
                    match word.as_str() {
                        "q" => {
                            toks.push(Tok { kind: Kind::Str(unescape_single(&body)), pos: i });
                            i = e;
                        }
                        "qq" => {
                            toks.push(Tok { kind: Kind::Str(unescape_double(&body)?), pos: i });
                            i = e;
                        }
                        "qw" => {
                            toks.push(Tok {
                                kind: Kind::Qw(body.split_whitespace().map(String::from).collect()),
                                pos: i,
                            });
                            i = e;
                        }
                        _ => {
                            let f = FLAGS.find(&rest(e)).map_or(String::new(), |m| m.as_str().to_string());
                            let len = f.chars().count();
                            toks.push(Tok { kind: Kind::Regex(body, f), pos: i });
                            i = e + len;
                        }
                    }
                    continue;
                }
            }
            toks.push(Tok { kind: Kind::Ident(word), pos: i });
            i = j;
            continue;
        }
        let Some(op) = OPS.iter().filter(|op| r.starts_with(**op)).max_by_key(|op| op.len()) else {
            return Err(fail(format!("unexpected character {c:?} at {i}")));
        };
        toks.push(Tok { kind: Kind::Op(op), pos: i });
        i += op.len();
    }
    toks.push(Tok { kind: Kind::Eof, pos: n });
    Ok(toks)
}

/// Binding power and associativity (true: right) of a binary operator, Perl's precedence.
fn binary(op: &str) -> Option<(u8, bool)> {
    Some(match op {
        "or" | "xor" => (1, false),
        "and" => (2, false),
        "?" => (6, true),
        "||" | "//" => (7, false),
        "&&" => (8, false),
        "==" | "!=" | "<=>" | "eq" | "ne" | "cmp" => (11, false),
        "<" | ">" | "<=" | ">=" | "lt" | "gt" | "le" | "ge" => (12, false),
        "+" | "-" | "." => (14, false),
        "*" | "/" | "%" | "x" => (15, false),
        "=~" | "!~" => (16, false),
        "**" => (18, true),
        _ => return None,
    })
}

const WORD_OPS: [&str; 12] = ["and", "or", "not", "xor", "eq", "ne", "lt", "gt", "le", "ge", "cmp", "x"];
const NAMED_UNARY: [&str; 13] =
    ["abs", "sqrt", "sin", "cos", "log", "exp", "int", "defined", "length", "lc", "uc", "deg2rad", "rad2deg"];

fn constant(word: &str) -> Option<f64> {
    use std::f64::consts::PI;
    Some(match word.strip_prefix("Math::Trig::").unwrap_or(word) {
        "pi" => PI,
        "pi2" => 2.0 * PI,
        "pip2" => PI / 2.0,
        "pip4" => PI / 4.0,
        "pi4" if word.starts_with("Math::Trig::") => 4.0 * PI,
        _ => return None,
    })
}

struct Parser {
    toks: Vec<Tok>,
    i: usize,
}

impl Parser {
    fn peek(&self) -> &Tok {
        &self.toks[self.i.min(self.toks.len() - 1)]
    }

    fn next(&mut self) -> Tok {
        let t = self.toks[self.i.min(self.toks.len() - 1)].clone();
        self.i += 1;
        t
    }

    fn at_op(&self, ops: &[&str]) -> bool {
        matches!(&self.peek().kind, Kind::Op(o) if ops.contains(o))
    }

    fn expect_op(&mut self, op: &str) -> Result<()> {
        let t = self.next();
        match t.kind {
            Kind::Op(o) if o == op => Ok(()),
            k => Err(fail(format!("expected {op:?} at {}, got {k:?}", t.pos))),
        }
    }

    fn program(&mut self) -> Result<Ast> {
        let mut stmts = Vec::new();
        while self.peek().kind != Kind::Eof {
            if self.at_op(&[";"]) {
                self.next();
                continue;
            }
            stmts.push(self.statement()?);
            if !self.at_op(&[";"]) && self.peek().kind != Kind::Eof {
                return Err(fail(format!("unexpected {:?} at {}", self.peek().kind, self.peek().pos)));
            }
        }
        Ok(match stmts.len() {
            0 => Ast::Num(0.0),
            1 => stmts.pop().unwrap(),
            _ => Ast::Seq(stmts),
        })
    }

    fn statement(&mut self) -> Result<Ast> {
        if self.peek().kind == Kind::Ident("my".into()) {
            self.next();
            let v = self.next();
            let Kind::Var(name) = v.kind else {
                return Err(fail(format!("expected a scalar after 'my' at {}", v.pos)));
            };
            self.expect_op("=")?;
            return Ok(Ast::My(name, Box::new(self.expr(0)?)));
        }
        if self.peek().kind == Kind::Ident("return".into()) {
            self.next();
        }
        self.expr(0)
    }

    fn expr(&mut self, min_bp: u8) -> Result<Ast> {
        let mut left = self.prefix()?;
        loop {
            let t = self.peek().clone();
            let op = match &t.kind {
                Kind::Op(o) => o.to_string(),
                Kind::Ident(w) if WORD_OPS.contains(&w.as_str()) => w.clone(),
                _ => break,
            };
            let Some((bp, right)) = binary(&op) else { break };
            if bp < min_bp {
                break;
            }
            self.next();
            if op == "?" {
                let a = self.expr(0)?;
                self.expect_op(":")?;
                let b = self.expr(bp)?;
                left = Ast::Tern(Box::new(left), Box::new(a), Box::new(b));
                continue;
            }
            if op == "=~" || op == "!~" {
                let rhs = self.next();
                let Kind::Regex(p, f) = rhs.kind else {
                    return Err(fail(format!("expected a regex after {op} at {}", rhs.pos)));
                };
                left = Ast::Bin(op, Box::new(left), Box::new(Ast::Regex(p, f)));
                continue;
            }
            let r = self.expr(if right { bp } else { bp + 1 })?;
            left = Ast::Bin(op, Box::new(left), Box::new(r));
        }
        Ok(left)
    }

    fn prefix(&mut self) -> Result<Ast> {
        let t = self.next();
        match t.kind {
            Kind::Num(v) => Ok(Ast::Num(v)),
            Kind::Str(s) => Ok(Ast::Str(s)),
            Kind::Regex(p, f) => Ok(Ast::Regex(p, f)),
            Kind::Qw(w) => Ok(Ast::List(w.into_iter().map(Ast::Str).collect())),
            Kind::Var(name) => {
                if name == "self" && self.at_op(&["->"]) {
                    self.next();
                    let m = self.next();
                    let Kind::Ident(method) = m.kind else {
                        return Err(fail(format!("expected a method name at {}", m.pos)));
                    };
                    return self.call(method);
                }
                if self.at_op(&["["]) {
                    self.next();
                    let idx = self.expr(0)?;
                    self.expect_op("]")?;
                    return Ok(Ast::Idx(name, Box::new(idx)));
                }
                Ok(Ast::Var(name))
            }
            Kind::ArrVar(name) => Ok(Ast::Array(name)),
            Kind::Ident(word) => {
                if self.at_op(&["=>"]) {
                    return Ok(Ast::Str(word)); // a bareword hash key
                }
                if word == "not" {
                    return Ok(Ast::Un("!".into(), Box::new(self.expr(3)?)));
                }
                if let Some(v) = constant(&word).filter(|_| !self.at_op(&["("])) {
                    return Ok(Ast::Num(v));
                }
                if word == "undef" {
                    if self.at_op(&["("]) {
                        self.next();
                        self.expect_op(")")?;
                    }
                    return Ok(Ast::Undef);
                }
                self.call(word)
            }
            Kind::Op(op) => match op {
                "(" => {
                    if self.at_op(&[")"]) {
                        self.next();
                        return Ok(Ast::List(Vec::new()));
                    }
                    let mut items = self.list_items(")")?;
                    Ok(if items.len() == 1 { items.pop().unwrap() } else { Ast::List(items) })
                }
                "[" => Ok(Ast::List(self.list_items("]")?)),
                "{" => Ok(Ast::Hash(self.list_items("}")?)),
                "!" => Ok(Ast::Un("!".into(), Box::new(self.expr(17)?))),
                "-" => Ok(Ast::Un("-".into(), Box::new(self.expr(17)?))),
                "+" => self.expr(17),
                "\\" => {
                    let inner = self.expr(17)?;
                    Ok(match inner {
                        Ast::Array(name) => Ast::Aref(name),
                        x => x,
                    })
                }
                _ => Err(fail(format!("unexpected {op:?} at {}", t.pos))),
            },
            Kind::Eof => Err(fail(format!("unexpected end at {}", t.pos))),
        }
    }

    fn list_items(&mut self, close: &str) -> Result<Vec<Ast>> {
        let mut items: Vec<Ast> = Vec::new();
        while !self.at_op(&[close]) {
            let item = self.expr(5)?;
            if let Some(Ast::Key(k)) = items.last() {
                let k = k.clone();
                *items.last_mut().unwrap() = Ast::Pair(k, Box::new(item));
            } else {
                items.push(item);
            }
            if self.at_op(&["=>"]) {
                self.next();
                let key = items.pop().unwrap();
                items.push(Ast::Key(Box::new(key)));
                continue;
            }
            if self.at_op(&[","]) {
                self.next();
                continue;
            }
            if !self.at_op(&[close]) {
                return Err(fail(format!("expected ',' or {close:?} at {}", self.peek().pos)));
            }
        }
        self.expect_op(close)?;
        Ok(items)
    }

    fn call(&mut self, name: String) -> Result<Ast> {
        let items = if self.at_op(&["("]) {
            self.next();
            self.list_items(")")?
        } else if NAMED_UNARY.contains(&name.as_str()) {
            vec![self.expr(13)?]
        } else {
            return Err(fail(format!("unknown bareword {name:?}")));
        };
        Ok(Ast::Call(name, items))
    }
}

/// The syntax tree of a condition.
pub fn parse(src: &str) -> Result<Ast> {
    Parser { toks: tokenize(src)?, i: 0 }.program()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conditions_parse() {
        let a = parse("sp_circle (radius => 200000)").unwrap();
        assert_eq!(
            a,
            Ast::Call(
                "sp_circle".into(),
                vec![Ast::Pair(Box::new(Ast::Str("radius".into())), Box::new(Ast::Num(200000.0)))]
            )
        );
        let b = parse("$D <= 3 && $nbr_y > $y").unwrap();
        let Ast::Bin(op, _, _) = b else { panic!() };
        assert_eq!(op, "&&");
        assert!(matches!(parse("my $a = 2; $D < $a").unwrap(), Ast::Seq(_)));
        assert!(matches!(parse("sp_match_regex (re => qr/^1/)").unwrap(), Ast::Call(..)));
    }
}
