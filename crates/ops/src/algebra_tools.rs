//! What an editor of map algebra's scripts ([`crate::algebra`]) needs: the parts of a script to colour, its first
//! error where it is, the names to complete where the cursor is (the inputs, a stack's layers, the functions and
//! the methods), the script run on a sample of the cells for a preview, and a line computed in the scope of a
//! script (a console's).
//!
//! The inputs are described by what the node is connected to ([`describe`]): a stack's layers are known from the
//! values of the last run, or from the headers of rasters on disk, before anything runs.

use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;

use terraflux_raster::tiff::Window;

use crate::algebra::{
    Expr, FUNCTIONS, METHODS, Out, Scope, Script, Stmt, WORDS, band_names, parse_located, parse_script,
};
use crate::{Error, Key, Result, Value};

/// What a part of a script is, for its colour.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    Number,
    /// A name given a value in the script.
    Name,
    /// One of the node's inputs.
    Input,
    /// A stack's layer: `s.layer`.
    Layer,
    Function,
    Method,
    /// A word of the language (`out`, `if`, `and`, `null`...).
    Word,
    Operator,
    Comment,
}

/// What an input of the node is.
#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Map,
    /// A number (its value when known).
    Number(Option<f64>),
    /// A stack: its layers, as the script names them and as they are named, when known.
    Stack(Option<Vec<(String, String)>>),
    /// Not known yet (what it is connected to has not run).
    Unknown,
}

/// The node's inputs by name.
pub type Inputs = BTreeMap<String, Input>;

/// What a value is as an input of map algebra (rasters on disk: by their headers, nothing read).
pub fn describe(v: &Value) -> Input {
    let stack = |keys: &[Key]| {
        let names = band_names(keys);
        Input::Stack(Some(names.into_iter().zip(keys.iter().map(|k| k.to_string())).collect()))
    };
    match v {
        Value::Map(_) => Input::Map,
        Value::Stack(s) => stack(&s.keys),
        // (as map.calc's port takes them: every band, unless one was chosen)
        Value::Raster(r) if r.layers.len() > 1 && !r.picked => {
            stack(&r.layers.iter().map(|l| Key::Name(l.name.clone())).collect::<Vec<_>>())
        }
        Value::Raster(_) => Input::Map,
        v => v.as_number().map_or(Input::Unknown, |x| Input::Number(Some(x))),
    }
}

/// Why a value does not have the layers a script reads of it (by the names the script reads them: `nir`, `_1997`):
/// one map, not a stack, or a stack without some of them (which, and those it has). None when it has them, or when
/// what it is is not known.
pub fn lacks_layers(v: &Value, layers: &[String]) -> Option<String> {
    match describe(v) {
        Input::Stack(Some(names)) => {
            let missing: Vec<&str> =
                layers.iter().filter(|l| !names.iter().any(|(n, _)| n == *l)).map(String::as_str).collect();
            (!missing.is_empty()).then(|| {
                let has: Vec<&str> = names.iter().map(|(n, _)| n.as_str()).collect();
                format!("it has no layer {} (it has {})", missing.join(", "), has.join(", "))
            })
        }
        Input::Map => Some(format!("one map, not a stack with the layers {}", layers.join(", "))),
        _ => None,
    }
}

/// The layers a script reads of each name it is given (`img.nir`: `nir` of `img`), in order: the layers an input
/// must have. A name given a value in the script reads the layers of the inputs it comes from layer by layer
/// (`f = lu == 1`, then `f._1997`: `_1997` of `lu`); a name given a map (a layer, a reduction over the layers)
/// comes from none.
pub fn layers_read(text: &str) -> BTreeMap<String, Vec<String>> {
    let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let Ok(script) = parse_script(text) else { return out };
    // the inputs each name given in the script comes from, layer by layer
    let mut roots: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let of =
        |n: &str, roots: &BTreeMap<String, Vec<String>>| roots.get(n).cloned().unwrap_or_else(|| vec![n.to_string()]);
    /// The names whose layers an expression's value has: those it reads whole, outside a reduction or a method.
    fn whole(e: &Expr, out: &mut Vec<String>) {
        match e {
            Expr::Var(n) if !out.contains(n) => out.push(n.clone()),
            Expr::Unary(_, a) => whole(a, out),
            Expr::Binary(_, a, b) => {
                whole(a, out);
                whole(b, out);
            }
            Expr::If(c, a, b) => {
                whole(c, out);
                whole(a, out);
                whole(b, out);
            }
            Expr::Call(f, args)
                if !matches!(f.as_str(), "mean" | "sd" | "median" | "sum" | "count" | "min" | "max" | "percentile") =>
            {
                args.iter().for_each(|a| whole(a, out))
            }
            _ => {}
        }
    }
    let read = |e: &Expr, roots: &BTreeMap<String, Vec<String>>, out: &mut BTreeMap<String, Vec<String>>| {
        e.walk(&mut |x| {
            if let Expr::Layer(n, l) = x {
                for r in of(n, roots) {
                    let v = out.entry(r).or_default();
                    if !v.contains(l) {
                        v.push(l.clone());
                    }
                }
            }
        });
    };
    for (_, st) in &script.statements {
        match st {
            Stmt::Set { name, layer, mask, value, .. } => {
                if let Some(m) = mask {
                    read(m, &roots, &mut out);
                }
                read(value, &roots, &mut out);
                if layer.is_none() {
                    let mut names = Vec::new();
                    whole(value, &mut names);
                    let mut from: Vec<String> = names.iter().flat_map(|n| of(n, &roots)).collect();
                    // a part changed: what it was too
                    if mask.is_some() {
                        from.extend(of(name, &roots));
                    }
                    from.dedup();
                    roots.insert(name.clone(), from);
                }
            }
            Stmt::Value(e) => read(e, &roots, &mut out),
            Stmt::Out(_) => {}
        }
    }
    out
}

/// The help of the functions: name, as written, what it gives.
pub const FUNCTION_HELP: [(&str, &str, &str); 25] = [
    ("abs", "abs(x)", "the absolute value"),
    ("sqrt", "sqrt(x)", "the square root"),
    ("exp", "exp(x)", "e to the power x"),
    ("ln", "ln(x)", "the natural logarithm"),
    ("log", "log(x)", "the natural logarithm (as Python's)"),
    ("log10", "log10(x)", "the logarithm in base 10"),
    ("floor", "floor(x)", "the whole number below"),
    ("ceil", "ceil(x)", "the whole number above"),
    ("round", "round(x)", "the nearest whole number"),
    ("isnull", "isnull(x)", "1 where x is missing, else 0"),
    ("coalesce", "coalesce(x, y)", "x, and y where x is missing"),
    ("clamp", "clamp(x, lo, hi)", "x kept between lo and hi"),
    ("nbsum", "nbsum(x, r)", "the sum of the square of radius r cells around each cell"),
    ("nbmean", "nbmean(x, r)", "the mean of the square of radius r cells around each cell"),
    ("nbmin", "nbmin(x, r)", "the least value of the square of radius r cells around each cell"),
    ("nbmax", "nbmax(x, r)", "the largest value of the square of radius r cells around each cell"),
    ("nbcount", "nbcount(x, r)", "the cells with a value in the square of radius r cells around each cell"),
    ("min", "min(a, b, ...) or min(s)", "the least of the values; of a stack's layers at each cell"),
    ("max", "max(a, b, ...) or max(s)", "the largest of the values; of a stack's layers at each cell"),
    ("mean", "mean(s)", "the mean of a stack's layers at each cell (a map's: x.mean())"),
    ("sd", "sd(s)", "the standard deviation of a stack's layers at each cell"),
    ("median", "median(s)", "the median of a stack's layers at each cell"),
    ("sum", "sum(s)", "the sum of a stack's layers at each cell"),
    ("count", "count(s)", "how many of a stack's layers have a value at each cell"),
    ("percentile", "percentile(s, p)", "the p-th percentile of a stack's layers at each cell"),
];

/// The help of the methods: name, as written, what it gives.
pub const METHOD_HELP: [(&str, &str, &str); 8] = [
    ("mean", "x.mean()", "the mean of all the cells: a number"),
    ("sd", "x.sd()", "the standard deviation of all the cells: a number"),
    ("median", "x.median()", "the median of all the cells: a number"),
    ("sum", "x.sum()", "the sum of all the cells: a number"),
    ("count", "x.count()", "how many cells have a value: a number"),
    ("min", "x.min()", "the least value of all the cells: a number"),
    ("max", "x.max()", "the largest value of all the cells: a number"),
    ("percentile", "x.percentile(p)", "the p-th percentile of all the cells: a number"),
];

/// The help of the words of the language.
const WORD_HELP: [(&str, &str); 12] = [
    ("out", "out name = value: an output of the node"),
    ("if", "a if c else b"),
    ("else", "a if c else b"),
    ("then", "if c then a else b"),
    ("and", "both"),
    ("or", "either"),
    ("not", "the opposite"),
    ("null", "a missing value"),
    ("None", "a missing value"),
    ("True", "1"),
    ("False", "0"),
    ("pi", "3.14159..."),
];

fn is_name(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

/// The first character after `i` that is not a space or a tab.
fn next_char(b: &[u8], mut i: usize) -> Option<u8> {
    while i < b.len() && (b[i] == b' ' || b[i] == b'\t') {
        i += 1;
    }
    b.get(i).copied()
}

/// Whether the name at `i` is read after a dot (`x.name`): a layer or a method.
fn after_dot(b: &[u8], i: usize) -> bool {
    i >= 2 && b[i - 1] == b'.' && (is_name(b[i - 2]) || b[i - 2] == b')')
}

/// The parts of a script, in order, for their colours (the spaces left out).
pub fn parts(text: &str, inputs: &Inputs) -> Vec<(Range<usize>, Part)> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let c = b[i];
        let start = i;
        if c == b'#' {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            out.push((start..i, Part::Comment));
        } else if c.is_ascii_digit()
            || (c == b'.' && b.get(i + 1).is_some_and(u8::is_ascii_digit) && !after_dot(b, i + 1))
        {
            while i < b.len() && (b[i].is_ascii_digit() || b[i] == b'.') {
                i += 1;
            }
            if i < b.len()
                && (b[i] == b'e' || b[i] == b'E')
                && b.get(i + 1).is_some_and(|d| d.is_ascii_digit() || *d == b'-' || *d == b'+')
            {
                i += 2;
                while i < b.len() && b[i].is_ascii_digit() {
                    i += 1;
                }
            }
            out.push((start..i, Part::Number));
        } else if c.is_ascii_alphabetic() || c == b'_' {
            while i < b.len() && is_name(b[i]) {
                i += 1;
            }
            let word = &text[start..i];
            let call = next_char(b, i) == Some(b'(');
            let part = if after_dot(b, start) {
                if call { Part::Method } else { Part::Layer }
            } else if WORDS.contains(&word) {
                Part::Word
            } else if call {
                Part::Function
            } else if inputs.contains_key(word) {
                Part::Input
            } else {
                Part::Name
            };
            out.push((start..i, part));
        } else if c.is_ascii_whitespace() {
            i += 1;
        } else {
            i += text[i..].chars().next().map_or(1, char::len_utf8);
            out.push((start..i, Part::Operator));
        }
    }
    out
}

/// An error of a script: where it is (bytes), its line and what it is.
#[derive(Debug, Clone, PartialEq)]
pub struct Problem {
    pub at: Range<usize>,
    pub line: usize,
    pub message: String,
}

/// The bytes of a line of a text (from 1), its spaces at the ends left out.
fn line_range(text: &str, line: usize) -> Range<usize> {
    let mut start = 0;
    for (k, l) in text.split('\n').enumerate() {
        if k + 1 == line {
            let lead = l.len() - l.trim_start().len();
            return start + lead..start + l.trim_end().len().max(lead);
        }
        start += l.len() + 1;
    }
    text.len()..text.len()
}

/// The bytes of a name in a line (the first time it is there; after `s.` for a layer).
fn find_name(text: &str, line: usize, name: &str, of: Option<&str>) -> Option<Range<usize>> {
    let r = line_range(text, line);
    let b = text.as_bytes();
    let mut i = r.start;
    while let Some(k) = text[i..r.end].find(name) {
        let (s, e) = (i + k, i + k + name.len());
        let whole = (s == 0 || !is_name(b[s - 1])) && (e >= b.len() || !is_name(b[e]));
        let dotted = match of {
            Some(o) => s > o.len() && b[s - 1] == b'.' && text[..s - 1].ends_with(o),
            None => s == 0 || b[s - 1] != b'.',
        };
        if whole && dotted {
            return Some(s..e);
        }
        i = e;
    }
    None
}

/// The number of edits from one word to another (to suggest the name meant).
fn distance(a: &str, b: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (a.to_lowercase().chars().collect(), b.to_lowercase().chars().collect());
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let here = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != cb)).min(row[j] + 1).min(row[j + 1] + 1);
            prev = here;
        }
    }
    row[b.len()]
}

/// The name of `known` closest to `name`, when close enough to be a typing error.
fn closest<'a>(name: &str, known: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    known.map(|k| (distance(name, k), k)).filter(|(d, k)| *d <= 2.max(k.len() / 3)).min().map(|(_, k)| k)
}

/// The first error of a script: a mistake of the language, a name that is neither an input nor given a value
/// before, a layer a stack does not have. (What only shows when it runs, as stacks of different layers, is told
/// by [`preview`].)
pub fn problem(text: &str, inputs: &Inputs) -> Option<Problem> {
    let script = match parse_located(text) {
        Ok(s) => s,
        Err((e, line, at)) => return Some(Problem { at, line, message: e.0 }),
    };
    let mut named: Vec<String> = Vec::new();
    for (line, st) in &script.statements {
        let line = *line;
        let mut found = None;
        let mut check = |e: &Expr, named: &[String]| {
            e.walk(&mut |x| {
                if found.is_some() {
                    return;
                }
                let known = |n: &str| inputs.contains_key(n) || named.iter().any(|v| v == n);
                match x {
                    Expr::Var(n) if !known(n) => {
                        let all = inputs.keys().map(String::as_str).chain(named.iter().map(String::as_str));
                        let hint = closest(n, all).map_or(String::new(), |c| format!(": did you mean {c}?"));
                        found = Some((n.clone(), None, format!("no input or name {n}{hint}")));
                    }
                    Expr::Layer(s, l) if !known(s) => {
                        found = Some((s.clone(), None, format!("no input or name {s} (for its layer {l})")));
                    }
                    Expr::Layer(s, l) if !named.iter().any(|v| v == s) => match inputs.get(s) {
                        Some(Input::Map) => {
                            found = Some((l.clone(), Some(s.clone()), format!("{s} is a map: it has no layers")));
                        }
                        Some(Input::Number(_)) => {
                            found = Some((l.clone(), Some(s.clone()), format!("{s} is a number: it has no layers")));
                        }
                        Some(Input::Stack(Some(layers))) if !layers.iter().any(|(n, _)| n == l) => {
                            let hint = closest(l, layers.iter().map(|(n, _)| n.as_str()))
                                .map_or(String::new(), |c| format!(": did you mean {s}.{c}?"));
                            let all: Vec<&str> = layers.iter().take(8).map(|(n, _)| n.as_str()).collect();
                            let more = if layers.len() > 8 { ", ..." } else { "" };
                            found = Some((
                                l.clone(),
                                Some(s.clone()),
                                format!("{s} has no layer {l}{hint} (its layers: {}{more})", all.join(", ")),
                            ));
                        }
                        _ => {}
                    },
                    _ => {}
                }
            });
        };
        match st {
            Stmt::Set { name, layer, mask, value, .. } => {
                // a part of a value (its layer, its cells where): the value had one before
                if (layer.is_some() || mask.is_some()) && !inputs.contains_key(name) && !named.contains(name) {
                    let at = find_name(text, line, name, None).unwrap_or_else(|| line_range(text, line));
                    let what = if layer.is_some() { "a layer of" } else { "the cells of" };
                    return Some(Problem {
                        at,
                        line,
                        message: format!("{name} has no value yet to change {what} it: {name} = ... first"),
                    });
                }
                if let Some(m) = mask {
                    check(m, &named);
                }
                check(value, &named);
                if !named.contains(name) {
                    named.push(name.clone());
                }
            }
            Stmt::Out(name) => {
                if !inputs.contains_key(name) && !named.contains(name) {
                    let at = find_name(text, line, name, None).unwrap_or_else(|| line_range(text, line));
                    return Some(Problem { at, line, message: format!("no input or name {name} to give out") });
                }
            }
            Stmt::Value(e) => check(e, &named),
        }
        if let Some((n, of, message)) = found {
            let at = find_name(text, line, &n, of.as_deref()).unwrap_or_else(|| line_range(text, line));
            return Some(Problem { at, line, message });
        }
    }
    None
}

/// A name offered where the cursor is.
#[derive(Debug, Clone, PartialEq)]
pub struct Completion {
    /// What is shown.
    pub label: String,
    /// What replaces the word typed.
    pub text: String,
    /// What it is: an input's kind, a layer's name in its file, a function's help.
    pub detail: String,
    pub part: Part,
}

/// The names given a value in a script (`name = ...`, `out name = ...`, `name[...] = ...`), its lines read one by
/// one (a script being typed is seldom whole).
pub fn names_given(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in text.lines().flat_map(|l| l.split('#').next().unwrap_or("").split(';')) {
        let l = line.trim_start();
        let l = l.strip_prefix("out ").map_or(l, str::trim_start);
        let end = l.bytes().position(|c| !is_name(c)).unwrap_or(l.len());
        let name = &l[..end];
        if name.is_empty() || name.as_bytes()[0].is_ascii_digit() || WORDS.contains(&name) {
            continue;
        }
        // then a layer or cells (`.layer`, `[...]`) and one `=`
        let rest = l[end..].trim_start();
        let rest = match rest.strip_prefix('.') {
            Some(r) => r.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_').trim_start(),
            None => rest,
        };
        let rest = if rest.starts_with('[') { rest.rsplit_once(']').map_or("", |(_, r)| r).trim_start() } else { rest };
        if rest.starts_with('=') && !rest.starts_with("==") && !out.iter().any(|n| n == name) {
            out.push(name.to_string());
        }
    }
    out
}

/// What fits where the cursor is (`cursor`, bytes): the word typed there (the bytes it covers, replaced by the
/// completion chosen) and the names that begin with it, then those that have it (of three letters or more). After `s.`, a stack's layers and
/// the methods; else the inputs, the names given a value in the script, the functions and the words.
pub fn complete(text: &str, cursor: usize, inputs: &Inputs) -> (Range<usize>, Vec<Completion>) {
    let cursor = cursor.min(text.len());
    let b = text.as_bytes();
    let mut start = cursor;
    while start > 0 && is_name(b[start - 1]) {
        start -= 1;
    }
    let at = start..cursor;
    let typed = &text[start..cursor];
    // in a comment, or a number
    let line_start = text[..start].rfind('\n').map_or(0, |k| k + 1);
    if text[line_start..start].contains('#') || typed.starts_with(|c: char| c.is_ascii_digit()) {
        return (at, Vec::new());
    }
    let mut all: Vec<Completion> = Vec::new();
    if start >= 1 && b[start - 1] == b'.' {
        // after a name and a dot: its layers, the methods
        let mut s = start - 1;
        while s > 0 && is_name(b[s - 1]) {
            s -= 1;
        }
        let of = &text[s..start - 1];
        if of.is_empty() && !(start >= 2 && b[start - 2] == b')') {
            return (at, Vec::new());
        }
        if let Some(Input::Stack(Some(layers))) = inputs.get(of) {
            for (i, (name, raw)) in layers.iter().enumerate() {
                let detail = if raw == name { format!("layer {}", i + 1) } else { format!("layer {}: {raw}", i + 1) };
                all.push(Completion { label: name.clone(), text: name.clone(), detail, part: Part::Layer });
            }
        }
        if !matches!(inputs.get(of), Some(Input::Number(_))) {
            for (m, written, help) in METHOD_HELP {
                let n = METHODS.iter().find(|(x, _)| *x == m).map_or(0, |(_, n)| *n);
                let text = if n == 0 { format!("{m}()") } else { format!("{m}(") };
                all.push(Completion {
                    label: written.trim_start_matches("x.").to_string(),
                    text,
                    detail: help.to_string(),
                    part: Part::Method,
                });
            }
        }
    } else {
        for (name, input) in inputs {
            let detail = match input {
                Input::Map => "map".to_string(),
                Input::Number(Some(x)) => format!("number = {x}"),
                Input::Number(None) => "number".to_string(),
                Input::Stack(Some(l)) => format!("stack of {} layers", l.len()),
                Input::Stack(None) => "stack".to_string(),
                Input::Unknown => "input".to_string(),
            };
            all.push(Completion { label: name.clone(), text: name.clone(), detail, part: Part::Input });
        }
        for name in names_given(text) {
            if !inputs.contains_key(&name) && name != typed {
                all.push(Completion { label: name.clone(), text: name, detail: "name".into(), part: Part::Name });
            }
        }
        for (f, written, help) in FUNCTION_HELP {
            debug_assert!(FUNCTIONS.iter().any(|(x, _, _)| *x == f) || f == "count");
            all.push(Completion {
                label: written.to_string(),
                text: format!("{f}("),
                detail: help.to_string(),
                part: Part::Function,
            });
        }
        for (w, help) in WORD_HELP {
            let text = if w == "out" { "out ".to_string() } else { w.to_string() };
            all.push(Completion { label: w.to_string(), text, detail: help.to_string(), part: Part::Word });
        }
    }
    let key = |c: &Completion| c.text.trim_end_matches(['(', ')', ' ']).to_string();
    let low = typed.to_lowercase();
    let mut first: Vec<Completion> = Vec::new();
    let mut then: Vec<Completion> = Vec::new();
    for c in all {
        let k = key(&c);
        // (the word typed whole already: nothing to complete)
        if k == typed && c.part != Part::Function && c.part != Part::Method {
            continue;
        }
        let kl = k.to_lowercase();
        if kl.starts_with(&low) {
            first.push(c);
        } else if low.len() >= 3 && kl.contains(&low) {
            then.push(c);
        }
    }
    first.extend(then);
    (at, first)
}

/// What a script gave on a sample of the cells: the grid of the sample (one cell every `step` rows and columns of
/// the inputs') and the outputs.
#[derive(Debug)]
pub struct Preview {
    pub width: usize,
    pub height: usize,
    pub step: usize,
    pub outputs: Vec<(String, Out)>,
}

/// The cells of the inputs, sampled (one every `step` rows and columns) or as they are.
struct Cells<'a> {
    width: usize,
    height: usize,
    maps: Vec<(&'a str, Cow<'a, [f64]>)>,
    numbers: Vec<(&'a str, f64)>,
    stacks: Vec<(&'a str, Layers<'a>)>,
}

/// A stack's layers, sampled or as they are.
type Layers<'a> = Vec<(Key, Cow<'a, [f64]>)>;

impl Cells<'_> {
    fn scope(&self) -> Scope<'_> {
        Scope {
            width: self.width,
            height: self.height,
            maps: self.maps.iter().map(|(n, c)| (*n, c.as_ref())).collect(),
            numbers: self.numbers.iter().copied().collect(),
            stacks: self
                .stacks
                .iter()
                .map(|(n, l)| (*n, l.iter().map(|(k, c)| (k.clone(), c.as_ref())).collect()))
                .collect(),
        }
    }
}

/// The grid's side over a sample's: the step that keeps the cells at most `most`.
fn step_for(width: usize, height: usize, most: usize) -> usize {
    let mut step = 1;
    while width.div_ceil(step) * height.div_ceil(step) > most.max(1) {
        step += 1;
    }
    step
}

/// One cell every `step` rows and columns.
fn sample<'a>(cells: &'a [f64], width: usize, height: usize, step: usize) -> Cow<'a, [f64]> {
    if step == 1 {
        return Cow::Borrowed(cells);
    }
    let w = width.div_ceil(step);
    let mut out = Vec::with_capacity(w * height.div_ceil(step));
    for r in (0..height).step_by(step) {
        out.extend((0..width).step_by(step).map(|c| cells[r * width + c]));
    }
    Cow::Owned(out)
}

/// The inputs' cells, one every `step` rows and columns (`most`: the cells a sample has at most; none: every
/// cell). Rasters on disk are read sampled (the whole of a layer when every cell is wanted).
fn cells<'a>(inputs: &'a BTreeMap<String, Value>, most: Option<usize>) -> Result<(Cells<'a>, usize)> {
    let mut grid: Option<(usize, usize, &str)> = None;
    let mut check = |w: usize, h: usize, name: &'a str| -> Result<()> {
        match grid {
            None => grid = Some((w, h, name)),
            Some((gw, gh, first)) if (gw, gh) != (w, h) => {
                return Err(Error::msg(format!("{name} is not on the grid of {first}")));
            }
            _ => {}
        }
        Ok(())
    };
    for (name, v) in inputs {
        match v {
            Value::Map(m) => check(m.grid.width, m.grid.height, name)?,
            Value::Stack(s) => check(s.grid.width, s.grid.height, name)?,
            Value::Raster(r) if r.mixed => {
                return Err(Error::msg(format!("{name}: rasters of different grids (raster.match makes them one)")));
            }
            Value::Raster(r) => check(r.grid.width, r.grid.height, name)?,
            _ => {}
        }
    }
    let (width, height) = grid.map_or((0, 0), |(w, h, _)| (w, h));
    let step = most.map_or(1, |m| step_for(width, height, m));
    let mut c = Cells {
        width: width.div_ceil(step),
        height: height.div_ceil(step),
        maps: Vec::new(),
        numbers: Vec::new(),
        stacks: Vec::new(),
    };
    let full = Window { x0: 0, y0: 0, width, height };
    let read = |r: &crate::files::Rasters, layer: usize| -> Result<Cow<'a, [f64]>> {
        Ok(if step == 1 {
            Cow::Owned(r.read(layer)?.to_vec())
        } else {
            Cow::Owned(r.sample(layer, full, step)?.into_iter().map(f64::from).collect())
        })
    };
    for (name, v) in inputs {
        match v {
            Value::Map(m) => c.maps.push((name, sample(&m.data, width, height, step))),
            Value::Stack(s) => c.stacks.push((
                name,
                s.keys.iter().cloned().zip(s.layers.iter().map(|l| sample(l, width, height, step))).collect(),
            )),
            Value::Raster(r) if r.layers.len() > 1 && !r.picked => {
                let layers = r
                    .layers
                    .iter()
                    .enumerate()
                    .map(|(i, l)| Ok((Key::Name(l.name.clone()), read(r, i)?)))
                    .collect::<Result<_>>()?;
                c.stacks.push((name, layers));
            }
            Value::Raster(r) => c.maps.push((name, read(r, r.chosen)?)),
            v => match v.as_number() {
                Some(x) => c.numbers.push((name, x)),
                None => return Err(Error::msg(format!("{name} is neither a map, a stack nor a number"))),
            },
        }
    }
    Ok((c, step))
}

/// The neighbourhoods of a script over a sample: their radii in the sample's cells.
fn sampled_radii(script: &mut Script, step: usize) {
    fn walk(e: &mut Expr, step: usize) {
        match e {
            Expr::Call(f, args) => {
                if f.starts_with("nb")
                    && let Some(Expr::Num(r)) = args.get_mut(1)
                {
                    *r = (*r / step as f64).round();
                }
                args.iter_mut().for_each(|a| walk(a, step));
            }
            Expr::Unary(_, a) => walk(a, step),
            Expr::Binary(_, a, b) => {
                walk(a, step);
                walk(b, step);
            }
            Expr::If(c, a, b) => {
                walk(c, step);
                walk(a, step);
                walk(b, step);
            }
            Expr::Method(x, _, args) => {
                walk(x, step);
                args.iter_mut().for_each(|a| walk(a, step));
            }
            _ => {}
        }
    }
    for (_, st) in &mut script.statements {
        match st {
            Stmt::Set { mask, value, .. } => {
                if let Some(m) = mask {
                    walk(m, step);
                }
                walk(value, step);
            }
            Stmt::Value(e) => walk(e, step),
            Stmt::Out(_) => {}
        }
    }
}

/// A script run on a sample of its inputs' cells, at most `most` of them (the neighbourhoods' radii made the
/// sample's): what each output looks like, at once. The numbers of a whole map (`x.mean()`) are the sample's.
pub fn preview(text: &str, inputs: &BTreeMap<String, Value>, most: usize) -> Result<Preview> {
    let mut script = parse_script(text)?;
    let (c, step) = cells(inputs, Some(most))?;
    if step > 1 {
        sampled_radii(&mut script, step);
    }
    let outputs = script.run(&c.scope())?;
    Ok(Preview { width: c.width, height: c.height, step, outputs })
}

/// A line computed in the scope of a script, on every cell of the inputs (the script's names given their values
/// first; the script left out when it has an error): a value (`img.mean()`, `ndvi > 0.5`) or a statement, whose
/// name's value it gives (`x = a * 2`). The grid's size and the value.
pub fn evaluate_line(script: &str, line: &str, inputs: &BTreeMap<String, Value>) -> Result<(usize, usize, Out)> {
    let mut statements: Vec<(usize, Stmt)> = parse_script(script)
        .map(|s| s.statements)
        .unwrap_or_default()
        .into_iter()
        .filter_map(|(l, st)| match st {
            Stmt::Set { name, layer, mask, value, .. } => Some((l, Stmt::Set { name, layer, mask, value, out: false })),
            // a value alone in the script: nothing named
            Stmt::Out(_) | Stmt::Value(_) => None,
        })
        .collect();
    let asked = parse_script(line).map_err(|e| Error::msg(e.0))?;
    let n = asked.statements.len();
    for (k, (_, st)) in asked.statements.into_iter().enumerate() {
        let last = k + 1 == n;
        // (on the line after the script's: an error says the line typed, not the script's)
        let at = usize::MAX;
        match st {
            Stmt::Value(e) if last => {
                statements.push((at, Stmt::Set { name: "_".into(), layer: None, mask: None, value: e, out: true }))
            }
            Stmt::Value(_) => {}
            Stmt::Set { name, layer, mask, value, .. } => {
                statements.push((at, Stmt::Set { name: name.clone(), layer, mask, value, out: false }));
                if last {
                    statements.push((at, Stmt::Out(name)));
                }
            }
            Stmt::Out(name) if last => statements.push((at, Stmt::Out(name))),
            Stmt::Out(_) => {}
        }
    }
    let script = Script { statements, lines: false };
    let (c, _) = cells(inputs, None)?;
    let mut out = script.run(&c.scope())?;
    let (_, v) = out.pop().ok_or_else(|| Error::msg("nothing to compute"))?;
    Ok((c.width, c.height, v))
}

/// The values of all the cells of a map that have one: the least, the mean, the largest and how many are missing.
pub fn summary(cells: &[f64]) -> (f64, f64, f64, usize) {
    let (mut lo, mut hi, mut sum, mut n) = (f64::INFINITY, f64::NEG_INFINITY, 0.0, 0usize);
    for &x in cells.iter().filter(|x| !x.is_nan()) {
        lo = lo.min(x);
        hi = hi.max(x);
        sum += x;
        n += 1;
    }
    if n == 0 {
        return (f64::NAN, f64::NAN, f64::NAN, cells.len());
    }
    (lo, sum / n as f64, hi, cells.len() - n)
}

/// The names a script reads that are not given a value in it: the inputs it wants.
pub fn reads(text: &str) -> Vec<String> {
    let Ok(script) = parse_script(text) else { return Vec::new() };
    let mut named: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for (_, st) in &script.statements {
        let mut read: Vec<String> = Vec::new();
        match st {
            Stmt::Set { name, layer, mask, value, .. } => {
                if layer.is_some() || mask.is_some() {
                    read.push(name.clone());
                }
                read.extend(mask.iter().flat_map(Expr::names));
                read.extend(value.names());
            }
            Stmt::Out(name) => read.push(name.clone()),
            Stmt::Value(e) => read.extend(e.names()),
        }
        for n in read {
            if !named.contains(&n) && !out.contains(&n) {
                out.push(n);
            }
        }
        if let Stmt::Set { name, .. } = st {
            named.insert(name.clone());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{Map, Stack};

    fn inputs() -> Inputs {
        [
            ("land".to_string(), Input::Map),
            ("k".to_string(), Input::Number(Some(2.0))),
            (
                "img".to_string(),
                Input::Stack(Some(vec![
                    ("red".into(), "red".into()),
                    ("nir".into(), "nir".into()),
                    ("_1997".into(), "1997".into()),
                ])),
            ),
        ]
        .into()
    }

    #[test]
    fn the_layers_a_script_needs() {
        let t = "x = img.nir
img = other
out y = (x - img.red) / s.b1 if s.b1 > 0 else s.red
x[m.a > 0] = 0";
        let l = layers_read(t);
        assert_eq!(l["s"], vec!["b1", "red"]);
        assert_eq!(l["img"], vec!["nir"], "img given a value: other's after");
        assert_eq!(l["other"], vec!["red"]);
        assert!(!l.contains_key("x"), "a layer: a map");
        assert_eq!(l["m"], vec!["a"], "a condition's layers too");
        // through names: a stack made of an input layer by layer, not a reduction
        let l = layers_read(
            "f = lu == 1
out lost = f._1997 and not f._2011
t = mean(s)
h = abs(g * 2)
out u = t.a + h.b",
        );
        assert_eq!(l["lu"], vec!["_1997", "_2011"]);
        assert_eq!(l["g"], vec!["b"]);
        assert!(!l.contains_key("s") && !l.contains_key("f"), "{l:?}");
        let stack = Value::Stack(Stack {
            grid: grid(1, 1),
            keys: vec![Key::Name("nir".into())],
            layers: vec![Arc::new(vec![1.0])],
        });
        assert_eq!(lacks_layers(&stack, &["nir".into()]), None);
        assert_eq!(lacks_layers(&stack, &["nir".into(), "red".into()]).unwrap(), "it has no layer red (it has nir)");
        assert!(lacks_layers(&Value::Number(1.0), &["nir".into()]).is_none());
    }

    #[test]
    fn the_parts_coloured() {
        let t = "out ndvi = (img.nir - img.red) / img.nir.max() # the index\nx = abs(land) * 2.5";
        let p = parts(t, &inputs());
        let of = |w: &str| p.iter().find(|(r, _)| &t[r.clone()] == w).map(|(_, k)| *k);
        assert_eq!(of("out"), Some(Part::Word));
        assert_eq!(of("ndvi"), Some(Part::Name));
        assert_eq!(of("img"), Some(Part::Input));
        assert_eq!(of("nir"), Some(Part::Layer));
        assert_eq!(of("max"), Some(Part::Method));
        assert_eq!(of("abs"), Some(Part::Function));
        assert_eq!(of("2.5"), Some(Part::Number));
        assert_eq!(of("# the index"), Some(Part::Comment));
        assert_eq!(of("*"), Some(Part::Operator));
    }

    #[test]
    fn problems_where_they_are() {
        let i = inputs();
        let at = |t: &str| problem(t, &i).map(|p| (t[p.at.clone()].to_string(), p.line, p.message));
        assert_eq!(at("land * 2"), None);
        // the language's
        let (w, line, m) = at("x = land\ny = x = 2").unwrap();
        assert_eq!((w.as_str(), line), ("=", 2));
        assert!(m.contains("use =="), "{m}");
        let (w, _, m) = at("x = abs(land, 2)").unwrap();
        assert_eq!(w, "abs");
        assert!(m.contains("abs takes 1"), "{m}");
        let (w, _, _) = at("x = land.median(2)").unwrap();
        assert_eq!(w, "median");
        assert_eq!(at("x = land $ 2").unwrap().0, "$");
        assert_eq!(at("x = img.1997").unwrap().0, "1997");
        // names
        let (w, line, m) = at("a = land\n\nb = lnad * 2").unwrap();
        assert_eq!((w.as_str(), line), ("lnad", 3));
        assert!(m.contains("did you mean land?"), "{m}");
        assert!(at("b = a * 2\na = land").unwrap().2.contains("no input or name a"));
        let (w, _, m) = at("x = img.nri").unwrap();
        assert_eq!(w, "nri");
        assert!(m.contains("did you mean img.nir?") && m.contains("red, nir, _1997"), "{m}");
        assert!(at("x = land.red").unwrap().2.contains("land is a map"));
        assert!(at("x[land == 1] = 0").unwrap().2.contains("x has no value yet"));
        assert_eq!(at("out y").unwrap().0, "y");
        // a layer added to a stack named in the script: not known before it runs
        assert_eq!(at("s = img * 2\ns.new = s.red\nout q = s.new"), None);
        // an input not known yet: anything of it
        let mut u = inputs();
        u.insert("u".into(), Input::Unknown);
        assert_eq!(problem("x = u.anything + u", &u), None);
    }

    #[test]
    fn completions_where_the_cursor_is() {
        let i = inputs();
        let labels = |t: &str| complete(t, t.len(), &i).1.into_iter().map(|c| c.label).collect::<Vec<_>>();
        // the layers of a stack, then the methods
        let l = labels("x = img.");
        assert_eq!(&l[..3], ["red", "nir", "_1997"]);
        assert!(l.contains(&"mean()".to_string()) && l.contains(&"percentile(p)".to_string()));
        assert_eq!(labels("x = img.n"), ["nir"]);
        // the layer's name in its file, beside the one the script reads
        let c = complete("img._1", 6, &i).1;
        assert_eq!((c[0].text.as_str(), c[0].detail.as_str()), ("_1997", "layer 3: 1997"));
        // a map's methods only
        assert_eq!(labels("land.me"), ["mean()", "median()"]);
        assert!(labels("k.").is_empty());
        // the inputs, the names given, the functions, the words
        let l = labels("ndvi = 1\nout q = nd");
        assert_eq!(l, ["ndvi"]);
        assert_eq!(labels("x = la"), ["land"]);
        let l = labels("x = me");
        assert!(l.contains(&"mean(s)".to_string()) && l.contains(&"median(s)".to_string()), "{l:?}");
        assert!(labels("x = per").contains(&"percentile(s, p)".to_string()));
        assert_eq!(labels("o"), ["out", "or"]);
        // the word replaced
        let (at, c) = complete("x = img.ni + 1", 10, &i);
        assert_eq!((at, c[0].text.as_str()), (8..10, "nir"));
        // nothing in a comment or a number
        assert!(labels("x = 1 # la").is_empty());
        assert!(labels("x = 1.5").is_empty());
        assert_eq!(names_given("a = 1\nout b = a\nc[a > 0] = 2 # d = 3\ns.x = 1; e == 2\nout a"), ["a", "b", "c", "s"]);
    }

    fn grid(w: usize, h: usize) -> Arc<terraflux_raster::Grid> {
        Arc::new(terraflux_raster::Grid::new(w, h))
    }

    #[test]
    fn a_preview_on_a_sample() {
        let (w, h) = (100, 80);
        let land: Vec<f64> = (0..w * h).map(|i| (i % w) as f64).collect();
        let s = Stack {
            grid: grid(w, h),
            keys: vec![Key::Name("a".into()), Key::Name("b".into())],
            layers: vec![Arc::new(land.clone()), Arc::new(land.iter().map(|x| x * 3.0).collect())],
        };
        let inputs: BTreeMap<String, Value> = [
            ("land".to_string(), Value::Map(Map::new(grid(w, h), land.clone()))),
            ("s".to_string(), Value::Stack(s)),
            ("k".to_string(), Value::Number(2.0)),
        ]
        .into();
        let p = preview("out m = mean(s) * k\nout top = land.max()\nout n = nbmax(land, 8)", &inputs, 2000).unwrap();
        // 8000 cells: one every 2 rows and columns
        assert_eq!((p.width, p.height, p.step), (50, 40, 2));
        let Out::Map(m) = &p.outputs[0].1 else { panic!() };
        assert_eq!(m.len(), 2000);
        assert_eq!((m[0], m[1], m[50]), (0.0, 8.0, 0.0));
        assert!(matches!(p.outputs[1].1, Out::Number(x) if x == 98.0));
        // the radius in the sample's cells: 4 of them, 8 of the map's
        let Out::Map(n) = &p.outputs[2].1 else { panic!() };
        assert_eq!(n[0], 8.0);
        // every cell when they are few
        assert_eq!(preview("land + 1", &inputs, 10_000).unwrap().step, 1);
        // what only shows when it runs
        let e = preview("out x = s + land.max() + q", &inputs, 2000).unwrap_err();
        assert!(e.0.contains("no input"), "{}", e.0);
    }

    #[test]
    fn a_line_in_the_scope_of_the_script() {
        let (w, h) = (10, 10);
        let land: Vec<f64> = (0..w * h).map(|i| i as f64).collect();
        let inputs: BTreeMap<String, Value> = [("land".to_string(), Value::Map(Map::new(grid(w, h), land)))].into();
        let script = "half = land / 2\nout big = half > 10";
        let n = |line: &str| match evaluate_line(script, line, &inputs).unwrap().2 {
            Out::Number(x) => x,
            o => panic!("{o:?}"),
        };
        assert_eq!(n("land.max()"), 99.0);
        assert_eq!(n("half.max()"), 49.5);
        assert_eq!(n("big.sum()"), 79.0);
        assert_eq!(n("x = half.mean() * 2"), 49.5);
        let (gw, gh, v) = evaluate_line(script, "land > 50", &inputs).unwrap();
        assert_eq!((gw, gh), (10, 10));
        assert!(matches!(v, Out::Map(c) if c.iter().sum::<f64>() == 49.0));
        // a script with an error: the inputs alone
        assert_eq!(evaluate_line("half = ", "land.min()", &inputs).unwrap().2, Out::Number(0.0));
        assert!(evaluate_line(script, "nothing + 1", &inputs).is_err());
        assert_eq!(summary(&[1.0, f64::NAN, 3.0]), (1.0, 2.0, 3.0, 1));
        assert_eq!(reads("a = land * k\nb = a + img.red\nc[x > 0] = 1\nout d"), ["land", "k", "img", "c", "x", "d"]);
    }

    #[test]
    fn inputs_described() {
        let s = Stack {
            grid: grid(1, 1),
            keys: vec![Key::Name("1997".into()), Key::Name("NDVI-2020".into())],
            layers: vec![Arc::new(vec![0.0]), Arc::new(vec![0.0])],
        };
        assert_eq!(
            describe(&Value::Stack(s)),
            Input::Stack(Some(vec![("_1997".into(), "1997".into()), ("NDVI_2020".into(), "NDVI-2020".into())]))
        );
        assert_eq!(describe(&Value::Integer(3)), Input::Number(Some(3.0)));
        assert_eq!(describe(&Value::Text("x".into())), Input::Unknown);
    }
}
