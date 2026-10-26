//! Map algebra written by a Terraflux before 1.17, brought to the language's one syntax, Python's (done when a
//! model is read): `if c then a else b` and `c ? a : b` become `a if c else b`, `^` `**`, `&& || !` `and or not`,
//! `null` `None` and `ln` `log`. Only the statements that have one of them are written again (from what they
//! mean, their brackets kept); the others, and the comments between them, are left as they were.

use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(String),
    Id(String),
    Op(&'static str),
    /// The end of a statement: a new line outside brackets, or `;`.
    End,
}

const OPS: [&str; 26] = [
    "**", "<=", ">=", "==", "!=", "&&", "||", "<", ">", "+", "-", "*", "/", "%", "^", "(", ")", "[", "]", ",", "?",
    ":", "!", ".", "=", ";",
];

/// The tokens of a text and where each is; None when it has a character the language does not.
fn lex(s: &str, script: bool) -> Option<Vec<(Tok, Range<usize>)>> {
    let b = s.as_bytes();
    let (mut i, mut depth, mut out) = (0, 0usize, Vec::new());
    let attached = |i: usize| i > 0 && (b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'_' || b[i - 1] == b')');
    while i < b.len() {
        let c = b[i] as char;
        if c == '\n' {
            if script && depth == 0 {
                out.push((Tok::End, i..i));
            }
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
            out.push((Tok::Num(s[start..i].to_string()), start..i));
        } else if c.is_ascii_alphabetic() || c == '_' {
            let start = i;
            while i < b.len() && ((b[i] as char).is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push((Tok::Id(s[start..i].to_string()), start..i));
        } else {
            let op = OPS.iter().find(|o| s[i..].starts_with(*o))?;
            match *op {
                "(" | "[" => depth += 1,
                ")" | "]" => depth = depth.saturating_sub(1),
                _ => {}
            }
            out.push((if *op == ";" && script { Tok::End } else { Tok::Op(op) }, i..i + op.len()));
            i += op.len();
        }
    }
    Some(out)
}

/// An expression as written: its brackets kept, its numbers as they were typed.
#[derive(Debug, Clone)]
enum E {
    Atom(String),
    Paren(Box<E>),
    Unary(&'static str, Box<E>),
    /// `or`, `and`, `+ - * / % **`.
    Bin(&'static str, Box<E>, Box<E>),
    /// A comparison, or a chain of them (`a < b <= c`).
    Cmp(Vec<E>, Vec<&'static str>),
    If(Box<E>, Box<E>, Box<E>),
    Call(String, Vec<E>),
    /// `x.name` (a layer) or `x.name(...)` (a method).
    Dot(Box<E>, String, Option<Vec<E>>),
}

/// How tightly each kind of expression binds: a part of a lower one is written between brackets.
fn level(e: &E) -> u8 {
    match e {
        E::If(..) => 0,
        E::Bin("or", ..) => 1,
        E::Bin("and", ..) => 2,
        E::Unary("not ", _) => 3,
        E::Cmp(..) => 4,
        E::Bin("+" | "-", ..) => 5,
        E::Bin("*" | "/" | "%", ..) => 6,
        E::Unary(..) => 7,
        E::Bin(..) => 8,
        _ => 9,
    }
}

/// An expression in the one syntax, between brackets when it binds less tightly than `least`.
fn show(e: &E, least: u8) -> String {
    let s = match e {
        E::Atom(a) => a.clone(),
        E::Paren(x) => format!("({})", show(x, 0)),
        E::Unary(op, x) => format!("{op}{}", show(x, if *op == "not " { 3 } else { 7 })),
        E::Bin(op, a, b) => {
            let l = level(e);
            if *op == "**" {
                format!("{} ** {}", show(a, 9), show(b, 7))
            } else {
                format!("{} {op} {}", show(a, l), show(b, l + 1))
            }
        }
        E::Cmp(xs, ops) => {
            let mut s = show(&xs[0], 5);
            for (op, x) in ops.iter().zip(&xs[1..]) {
                s.push_str(&format!(" {op} {}", show(x, 5)));
            }
            s
        }
        E::If(c, a, b) => format!("{} if {} else {}", show(a, 1), show(c, 1), show(b, 0)),
        E::Call(f, args) => format!("{f}({})", args.iter().map(|a| show(a, 0)).collect::<Vec<_>>().join(", ")),
        E::Dot(x, name, args) => match args {
            None => format!("{}.{name}", show(x, 9)),
            Some(a) => {
                format!("{}.{name}({})", show(x, 9), a.iter().map(|a| show(a, 0)).collect::<Vec<_>>().join(", "))
            }
        },
    };
    if level(e) < least { format!("({s})") } else { s }
}

/// A parser of both syntaxes, the old one and Python's.
struct P<'a> {
    toks: &'a [(Tok, Range<usize>)],
    at: usize,
}

impl P<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at).map(|(t, _)| t)
    }

    fn is_op(&self, op: &str) -> bool {
        matches!(self.peek(), Some(Tok::Op(o)) if *o == op)
    }

    fn is_word(&self, w: &str) -> bool {
        matches!(self.peek(), Some(Tok::Id(x)) if x == w)
    }

    fn eat_op(&mut self, op: &str) -> Option<()> {
        self.is_op(op).then(|| self.at += 1)
    }

    fn eat_word(&mut self, w: &str) -> Option<()> {
        self.is_word(w).then(|| self.at += 1)
    }

    fn expr(&mut self) -> Option<E> {
        if self.eat_word("if").is_some() {
            let c = self.expr()?;
            self.eat_word("then")?;
            let a = self.expr()?;
            self.eat_word("else")?;
            let b = self.expr()?;
            return Some(E::If(Box::new(c), Box::new(a), Box::new(b)));
        }
        let a = self.or()?;
        if self.eat_op("?").is_some() {
            let x = self.expr()?;
            self.eat_op(":")?;
            let y = self.expr()?;
            return Some(E::If(Box::new(a), Box::new(x), Box::new(y)));
        }
        if self.eat_word("if").is_some() {
            let c = self.or()?;
            self.eat_word("else")?;
            let b = self.expr()?;
            return Some(E::If(Box::new(c), Box::new(a), Box::new(b)));
        }
        Some(a)
    }

    fn or(&mut self) -> Option<E> {
        let mut l = self.and()?;
        while self.eat_op("||").or_else(|| self.eat_word("or")).is_some() {
            l = E::Bin("or", Box::new(l), Box::new(self.and()?));
        }
        Some(l)
    }

    fn and(&mut self) -> Option<E> {
        let mut l = self.not()?;
        while self.eat_op("&&").or_else(|| self.eat_word("and")).is_some() {
            l = E::Bin("and", Box::new(l), Box::new(self.not()?));
        }
        Some(l)
    }

    fn not(&mut self) -> Option<E> {
        if self.eat_op("!").or_else(|| self.eat_word("not")).is_some() {
            return Some(E::Unary("not ", Box::new(self.not()?)));
        }
        self.cmp()
    }

    fn cmp(&mut self) -> Option<E> {
        let first = self.add()?;
        let (mut xs, mut ops) = (vec![first], Vec::new());
        while let Some(op) = ["<=", ">=", "==", "!=", "<", ">"].into_iter().find(|o| self.is_op(o)) {
            self.at += 1;
            ops.push(op);
            xs.push(self.add()?);
        }
        Some(if ops.is_empty() { xs.pop()? } else { E::Cmp(xs, ops) })
    }

    fn add(&mut self) -> Option<E> {
        let mut l = self.mul()?;
        while let Some(op) = ["+", "-"].into_iter().find(|o| self.is_op(o)) {
            self.at += 1;
            l = E::Bin(op, Box::new(l), Box::new(self.mul()?));
        }
        Some(l)
    }

    fn mul(&mut self) -> Option<E> {
        let mut l = self.unary()?;
        while let Some(op) = ["*", "/", "%"].into_iter().find(|o| self.is_op(o)) {
            self.at += 1;
            l = E::Bin(op, Box::new(l), Box::new(self.unary()?));
        }
        Some(l)
    }

    fn unary(&mut self) -> Option<E> {
        if let Some(op) = ["-", "+"].into_iter().find(|o| self.is_op(o)) {
            self.at += 1;
            return Some(E::Unary(op, Box::new(self.unary()?)));
        }
        let base = self.postfix()?;
        if self.eat_op("^").or_else(|| self.eat_op("**")).is_some() {
            return Some(E::Bin("**", Box::new(base), Box::new(self.unary()?)));
        }
        Some(base)
    }

    fn postfix(&mut self) -> Option<E> {
        let mut e = self.atom()?;
        while self.eat_op(".").is_some() {
            let Some(Tok::Id(name)) = self.peek().cloned() else { return None };
            self.at += 1;
            let args = if self.is_op("(") { Some(self.args()?) } else { None };
            e = E::Dot(Box::new(e), name, args);
        }
        Some(e)
    }

    fn args(&mut self) -> Option<Vec<E>> {
        self.eat_op("(")?;
        let mut args = Vec::new();
        if self.eat_op(")").is_some() {
            return Some(args);
        }
        loop {
            args.push(self.expr()?);
            if self.eat_op(",").is_none() {
                break;
            }
        }
        self.eat_op(")")?;
        Some(args)
    }

    fn atom(&mut self) -> Option<E> {
        let t = self.peek()?.clone();
        self.at += 1;
        match t {
            Tok::Num(n) => Some(E::Atom(n)),
            Tok::Op("(") => {
                let e = self.expr()?;
                self.eat_op(")")?;
                Some(E::Paren(Box::new(e)))
            }
            Tok::Id(id) if self.is_op("(") => {
                let args = self.args()?;
                Some(E::Call(if id == "ln" { "log".into() } else { id }, args))
            }
            Tok::Id(id) if id == "null" => Some(E::Atom("None".into())),
            Tok::Id(id) => Some(E::Atom(id)),
            _ => None,
        }
    }

    /// A statement: what comes before its value (`out name =`, `name.layer[where] =`), and its value.
    fn statement(&mut self) -> Option<String> {
        let start = self.at;
        if self.eat_word("out").is_some() {
            let Some(Tok::Id(name)) = self.peek().cloned() else { return None };
            self.at += 1;
            if self.at_end() {
                return Some(format!("out {name}"));
            }
            self.eat_op("=")?;
            return Some(format!("out {name} = {}", show(&self.expr()?, 0)));
        }
        if let Some(Tok::Id(name)) = self.peek().cloned() {
            self.at += 1;
            let mut target = name;
            if self.is_op(".")
                && let Some((Tok::Id(l), _)) = self.toks.get(self.at + 1)
                && self.toks.get(self.at + 2).map(|(t, _)| t) != Some(&Tok::Op("("))
            {
                target = format!("{target}.{l}");
                self.at += 2;
            }
            if self.eat_op("[").is_some() {
                target = format!("{target}[{}]", show(&self.expr()?, 0));
                self.eat_op("]")?;
            }
            if self.eat_op("=").is_some() {
                return Some(format!("{target} = {}", show(&self.expr()?, 0)));
            }
            self.at = start;
        }
        Some(show(&self.expr()?, 0))
    }

    fn at_end(&self) -> bool {
        matches!(self.peek(), None | Some(Tok::End))
    }
}

/// Whether tokens have a form of the old syntax.
fn old(toks: &[(Tok, Range<usize>)]) -> bool {
    toks.iter().enumerate().any(|(i, (t, _))| match t {
        Tok::Op(o) => matches!(*o, "?" | "^" | "&&" | "||" | "!"),
        Tok::Id(x) => {
            x == "then" || x == "null" || (x == "ln" && toks.get(i + 1).map(|(t, _)| t) == Some(&Tok::Op("(")))
        }
        _ => false,
    })
}

/// A text in the one syntax, with each statement (a line of a script, the whole of an expression) that had an old
/// form written again; None when it had none, or cannot be read.
fn upgrade(text: &str, script: bool) -> Option<String> {
    let toks = lex(text, script)?;
    if !old(&toks) {
        return None;
    }
    let mut out = String::new();
    let mut copied = 0;
    let mut i = 0;
    while i < toks.len() {
        if toks[i].0 == Tok::End {
            i += 1;
            continue;
        }
        let end = toks[i..].iter().position(|(t, _)| *t == Tok::End).map_or(toks.len(), |k| i + k);
        let statement = &toks[i..end];
        if old(statement) {
            let mut p = P { toks: statement, at: 0 };
            let new = p.statement()?;
            if p.at != statement.len() {
                return None;
            }
            let (from, to) = (statement[0].1.start, statement[statement.len() - 1].1.end);
            out.push_str(&text[copied..from]);
            out.push_str(&new);
            copied = to;
        }
        i = end;
    }
    out.push_str(&text[copied..]);
    Some(out)
}

/// A script (`map.calc`, `check.assert`, `agents.update`'s rules) in the one syntax; None when it had nothing old.
pub fn upgrade_script(text: &str) -> Option<String> {
    upgrade(text, true)
}

/// An expression (`value.calc`, `table.filter`...: a new line is a space) in the one syntax; None when it had
/// nothing old.
pub fn upgrade_expression(text: &str) -> Option<String> {
    upgrade(text, false)
}

/// The settings written in map algebra: the operator (its name now), the setting, and whether it is a script.
pub const SETTINGS: [(&str, &str, bool); 10] = [
    ("raster.calc", "expression", true),
    ("check.assert", "expression", true),
    ("agents.update", "rules", true),
    ("value.calc", "expression", false),
    ("table.filter", "expression", false),
    ("table.calc", "expression", false),
    ("vector.filter", "expression", false),
    ("vector.calc", "expression", false),
    ("uncertainty.delta", "expression", false),
    ("uncertainty.monte_carlo", "expression", false),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_old_forms_are_written_in_python_s() {
        for (old, new) in [
            ("if a > 2 then 1 else 0", "1 if a > 2 else 0"),
            ("if a < 3 then a else if a < 6 then a * 2 else 0", "a if a < 3 else a * 2 if a < 6 else 0"),
            ("if (if a then b else c) then 1 else 2", "1 if (b if a else c) else 2"),
            ("if c then (if a then 1 else 2) else 3", "(1 if a else 2) if c else 3"),
            ("if c then if a then 1 else 2 else 3", "(1 if a else 2) if c else 3"),
            ("a > 1 ? 2 : 3", "2 if a > 1 else 3"),
            ("2 * (a ? b : c)", "2 * (b if a else c)"),
            ("1 + 2 * 3 ^ 2", "1 + 2 * 3 ** 2"),
            ("-a ^ 2", "-a ** 2"),
            ("(1 - q) ^ 3", "(1 - q) ** 3"),
            ("a && !b || c", "a and not b or c"),
            ("!(a == 1)", "not (a == 1)"),
            ("ln(a) + null", "log(a) + None"),
            ("exp(-((t - 0.6) ^ 2) / 0.02)", "exp(-((t - 0.6) ** 2) / 0.02)"),
            ("if isnull(suit) then null else d == 0", "None if isnull(suit) else d == 0"),
            ("if e > 0.9 then (if t == 3 then 1 else t + 1) else t", "(1 if t == 3 else t + 1) if e > 0.9 else t"),
            ("if s.y2000 > 1 then nbsum(x, 1) else x.mean()", "nbsum(x, 1) if s.y2000 > 1 else x.mean()"),
        ] {
            assert_eq!(upgrade_expression(old).as_deref(), Some(new), "{old}");
        }
        // nothing old: left alone
        for text in ["a if c else b", "a ** 2 and not b", "x = 1\nout x", "None", "$name"] {
            assert_eq!(upgrade_script(text), None, "{text}");
        }
    }

    #[test]
    fn only_the_statements_with_an_old_form_are_written_again() {
        let old = "# the forest\nforest = lu == 1   # of each year\nout lost = if forest._1997 then 1 else 0  # lost\n\
                   x[d > 2] = null; y = (a + b) * 2\n";
        let new = "# the forest\nforest = lu == 1   # of each year\nout lost = 1 if forest._1997 else 0  # lost\n\
                   x[d > 2] = None; y = (a + b) * 2\n";
        assert_eq!(upgrade_script(old).as_deref(), Some(new));
        assert_eq!(
            upgrade_script("wealth = land * cells\nrich = if wealth > 10 then 1 else 0").as_deref(),
            Some("wealth = land * cells\nrich = 1 if wealth > 10 else 0")
        );
        // in an expression a new line is a space
        assert_eq!(upgrade_expression("if a\nthen 1\nelse 2").as_deref(), Some("1 if a else 2"));
    }
}
