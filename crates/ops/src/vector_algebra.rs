//! Map algebra over vector features (`vector.calc`, `vector.filter`): a script of [`crate::algebra`], in Python's
//! syntax, computed for every feature at once, each attribute a value per feature (as a map's cell). Besides
//! numbers:
//!
//! - **Texts:** an attribute of texts compared with a text, with another or with a text input (`uso == "forest"`,
//!   `"a" <= name < "n"`, in the order of their characters), chosen (`"high" if area > 10 else "low"`, `coalesce`,
//!   `uso[area < 1] = "other"`), tested (`isnull(uso)`: an empty text is no value) and counted (`uso.count()`);
//!   `float(x)` reads a text attribute or a text input as numbers (`"12.5"` is 12.5; a text that is not a number,
//!   none). Arithmetic on texts, and a text where a number goes (`uso > 3`), are refused with what to write.
//! - **The geometry:** `geometry.area`, `geometry.perimeter`, `geometry.length`, `geometry.x`, `geometry.y`
//!   (`vector.geometry`'s measures, in the features' units).
//! - `feature`, the feature's number (1..), unless an attribute has that name.
//!
//! Texts are computed as numbers: every text the script can meet (of the attributes it reads, its texts, its text
//! inputs) numbered in their order, so that comparing their numbers compares them; which values are texts is
//! known from their kinds, checked before the script runs.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use terraflux_raster::vector::{Column, Vector};

use crate::algebra::{Expr, Out, Scope, Script, Stmt};
use crate::measures::{MEASURES, measure};
use crate::{Error, Key, Result, Value};

/// What a value is, for every feature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Number,
    Text,
    /// `None` alone: either.
    Missing,
}

const COMPARISONS: [&str; 6] = ["<", "<=", ">", ">=", "==", "!="];

/// The measures, for the errors.
fn measures() -> String {
    MEASURES.iter().map(|m| format!("geometry.{m}")).collect::<Vec<_>>().join(", ")
}

/// How a value is told in an error.
fn shown(e: &Expr) -> String {
    match e {
        Expr::Var(n) => n.clone(),
        Expr::Str(t) => format!("{t:?}"),
        _ => "a text".into(),
    }
}

fn unify(a: Kind, b: Kind, what: &str) -> Result<Kind> {
    match (a, b) {
        (Kind::Text, Kind::Number) | (Kind::Number, Kind::Text) => {
            Err(Error::msg(format!("{what}: its values are both texts or both numbers")))
        }
        (Kind::Missing, k) | (k, _) => Ok(k),
    }
}

/// The kinds of the values of a script over features, statement by statement, and the script written over
/// numbers: its texts by their numbers, `float(x)` by the attribute read as numbers (a name of the scope).
struct Kinds<'a> {
    v: &'a Vector,
    inputs: &'a BTreeMap<String, Value>,
    texts: &'a [String],
    /// The kinds of the names the script gave values.
    vars: HashMap<String, Kind>,
    /// The text attributes `float` reads.
    floats: BTreeSet<String>,
}

impl Kinds<'_> {
    fn code(&self, t: &str) -> f64 {
        code(self.texts, t)
    }

    fn name(&self, n: &str) -> Result<Kind> {
        if let Some(k) = self.vars.get(n) {
            return Ok(*k);
        }
        if n == "geometry" {
            return Err(Error::msg(format!("the geometry is read by its measures: {}", measures())));
        }
        if let Some(c) = self.v.column(n) {
            return Ok(if matches!(c, Column::Texts(_)) { Kind::Text } else { Kind::Number });
        }
        if n == "feature" {
            return Ok(Kind::Number);
        }
        match self.inputs.get(n) {
            Some(Value::Text(_)) => Ok(Kind::Text),
            Some(v) if v.as_number().is_some() => Ok(Kind::Number),
            _ => {
                let names: Vec<&str> = self.v.attributes.iter().map(|(a, _)| a.as_str()).collect();
                Err(Error::msg(format!(
                    "the script names {n:?}, neither an attribute (the features': {}) nor an input",
                    if names.is_empty() { "none".into() } else { names.join(", ") }
                )))
            }
        }
    }

    /// A value where a number goes.
    fn number(&mut self, e: &Expr) -> Result<Expr> {
        let (k, e2) = self.of(e)?;
        if k == Kind::Text {
            return Err(Error::msg(format!(
                "{} is a text where a number goes: compare it (== \"...\"), or read it as a number with float(...)",
                shown(e)
            )));
        }
        Ok(e2)
    }

    /// The kind of a value, and the value over numbers.
    fn of(&mut self, e: &Expr) -> Result<(Kind, Expr)> {
        let n = |e: Expr| Ok((Kind::Number, e));
        match e {
            Expr::Num(x) => Ok((if x.is_nan() { Kind::Missing } else { Kind::Number }, e.clone())),
            Expr::Str(t) => Ok((Kind::Text, Expr::Num(self.code(t)))),
            Expr::Var(name) => Ok((self.name(name)?, e.clone())),
            Expr::Layer(of, m) if of == "geometry" && !self.vars.contains_key(of) => {
                if MEASURES.contains(&m.as_str()) {
                    n(e.clone())
                } else {
                    Err(Error::msg(format!("the geometry has no measure {m:?} (its measures: {})", measures())))
                }
            }
            Expr::Layer(of, m) => Err(Error::msg(format!(
                "{of}.{m}: an attribute has no layers (the geometry's measures are {})",
                measures()
            ))),
            Expr::Unary(op, a) => n(Expr::Unary(*op, Box::new(self.number(a)?))),
            Expr::Binary(op, a, b) if COMPARISONS.contains(op) => {
                let ((ka, a2), (kb, b2)) = (self.of(a)?, self.of(b)?);
                if matches!((ka, kb), (Kind::Text, Kind::Number) | (Kind::Number, Kind::Text)) {
                    return Err(Error::msg(format!(
                        "a text compared with a number ({} {op} {}): compare texts with texts (uso == \"forest\"), \
                         or read the text as a number with float(...)",
                        if ka == Kind::Text { shown(a) } else { "a number".into() },
                        if kb == Kind::Text { shown(b) } else { "a number".into() },
                    )));
                }
                n(Expr::Binary(op, Box::new(a2), Box::new(b2)))
            }
            Expr::Binary(op, a, b) => {
                if *op == "+" && (self.of(a)?.0 == Kind::Text || self.of(b)?.0 == Kind::Text) {
                    return Err(Error::msg("texts are not joined nor added: compare them (uso == \"forest\")"));
                }
                n(Expr::Binary(op, Box::new(self.number(a)?), Box::new(self.number(b)?)))
            }
            Expr::If(c, a, b) => {
                let c2 = self.number(c)?;
                let ((ka, a2), (kb, b2)) = (self.of(a)?, self.of(b)?);
                Ok((unify(ka, kb, "a if c else b")?, Expr::If(Box::new(c2), Box::new(a2), Box::new(b2))))
            }
            Expr::Call(f, args) if f == "isnull" => n(Expr::Call(f.clone(), vec![self.of(&args[0])?.1])),
            Expr::Call(f, args) if f == "coalesce" => {
                let ((ka, a2), (kb, b2)) = (self.of(&args[0])?, self.of(&args[1])?);
                Ok((unify(ka, kb, "coalesce(x, y)")?, Expr::Call(f.clone(), vec![a2, b2])))
            }
            Expr::Call(f, args) if f == "float" => {
                let a = &args[0];
                let parse = |t: &str| Expr::Num(t.trim().parse().unwrap_or(f64::NAN));
                match self.of(a)? {
                    (Kind::Text, _) => match a {
                        Expr::Str(t) => n(parse(t)),
                        Expr::Var(name) if self.vars.contains_key(name) => Err(Error::msg(format!(
                            "float(x) reads a text attribute or a text input: {name} is a text the script computes"
                        ))),
                        Expr::Var(name) if self.v.column(name).is_some() => {
                            self.floats.insert(name.clone());
                            n(Expr::Var(float_name(name)))
                        }
                        Expr::Var(name) => match self.inputs.get(name) {
                            Some(Value::Text(t)) => n(parse(t)),
                            _ => unreachable!("a text input"),
                        },
                        _ => Err(Error::msg("float(x) reads a text attribute or a text input: float(name)")),
                    },
                    (_, a2) => n(a2),
                }
            }
            Expr::Call(f, _) if f.starts_with("nb") => {
                Err(Error::msg(format!("a feature has no neighbours: {f} and the like are for maps")))
            }
            Expr::Call(f, args) => {
                n(Expr::Call(f.clone(), args.iter().map(|a| self.number(a)).collect::<Result<_>>()?))
            }
            Expr::Method(x, m, args) => {
                let x2 = if m == "count" { self.of(x)?.1 } else { self.number(x)? };
                let args = args.iter().map(|a| self.number(a)).collect::<Result<_>>()?;
                n(Expr::Method(Box::new(x2), m.clone(), args))
            }
        }
    }

    /// A statement checked and written over numbers.
    fn statement(&mut self, s: &Stmt) -> Result<Stmt> {
        Ok(match s {
            Stmt::Set { name, layer: Some(l), .. } => {
                return Err(Error::msg(format!("{name}.{l} = ...: an attribute has no layers (name it {name}_{l})")));
            }
            Stmt::Set { name, .. } if name == "geometry" => {
                return Err(Error::msg("geometry is the features' geometry: give the value another name"));
            }
            Stmt::Set { name, layer: None, mask, value, out } => {
                let (kv, value) = self.of(value)?;
                let (kind, mask) = match mask {
                    None => (kv, None),
                    Some(m) => {
                        let m = self.number(m)?;
                        (unify(self.name(name)?, kv, &format!("{name}[...] = ..."))?, Some(m))
                    }
                };
                self.vars.insert(name.clone(), kind);
                Stmt::Set { name: name.clone(), layer: None, mask, value, out: *out }
            }
            Stmt::Out(name) => {
                self.name(name)?;
                s.clone()
            }
            Stmt::Value(e) => Stmt::Value(self.of(e)?.1),
        })
    }
}

/// The name in the scope of a text attribute read as numbers (not a name a script can write).
fn float_name(attribute: &str) -> String {
    format!("float({attribute})")
}

/// The number of a text among the texts (in order): none for an empty text.
fn code(texts: &[String], t: &str) -> f64 {
    if t.is_empty() {
        return f64::NAN;
    }
    texts.binary_search_by(|x| x.as_str().cmp(t)).map_or(f64::NAN, |i| i as f64)
}

/// The names a script reads or writes, the measures of the geometry it reads and its texts.
fn met(script: &Script) -> (BTreeSet<String>, BTreeSet<String>, BTreeSet<String>) {
    let (mut names, mut geometry, mut texts) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    let mut walk = |e: &Expr| {
        e.walk(&mut |x| match x {
            Expr::Var(n) => {
                names.insert(n.clone());
            }
            Expr::Layer(of, m) if of == "geometry" => {
                geometry.insert(m.clone());
            }
            Expr::Str(t) => {
                texts.insert(t.clone());
            }
            _ => {}
        })
    };
    for (_, s) in &script.statements {
        match s {
            Stmt::Set { mask, value, .. } => {
                mask.iter().for_each(&mut walk);
                walk(value);
            }
            Stmt::Value(e) => walk(e),
            Stmt::Out(_) => {}
        }
    }
    for (_, s) in &script.statements {
        if let Stmt::Set { name, .. } | Stmt::Out(name) = s {
            names.insert(name.clone());
        }
    }
    (names, geometry, texts)
}

/// What a script over features gives: its outputs (`out`; without, its last value as `map`) by name, a column
/// each, a value per feature (a number for all given to every feature). The node's other inputs are numbers and
/// texts by their names (the features and tables among them left out).
pub fn run(v: &Vector, inputs: &BTreeMap<String, Value>, script: &Script) -> Result<Vec<(String, Column)>> {
    let mut others = BTreeMap::new();
    for (port, x) in inputs {
        match x {
            Value::Vector(_) | Value::Table(_) => {}
            Value::Text(_) => {
                others.insert(port.clone(), x.clone());
            }
            x if x.as_number().is_some() => {
                others.insert(port.clone(), x.clone());
            }
            _ => {
                return Err(Error::msg(format!(
                    "{port} is neither a number nor a text (an expression's other inputs are)"
                )));
            }
        }
    }
    let (names, geometry, literals) = met(script);
    // every text it can meet, in order
    let mut texts: BTreeSet<String> = literals;
    for n in &names {
        if let Some(Column::Texts(t)) = v.column(n) {
            texts.extend(t.iter().filter(|x| !x.is_empty()).cloned());
        }
        if let Some(Value::Text(t)) = others.get(n) {
            texts.insert(t.clone());
        }
    }
    texts.remove("");
    let texts: Vec<String> = texts.into_iter().collect();
    // its kinds, and the script over numbers
    let mut k = Kinds { v, inputs: &others, texts: &texts, vars: HashMap::new(), floats: BTreeSet::new() };
    let mut statements = Vec::with_capacity(script.statements.len());
    for (line, s) in &script.statements {
        statements.push((*line, k.statement(s).map_err(|e| script.at(*line, e))?));
    }
    let kinds: Vec<(String, Kind)> = match script.outputs() {
        outs if outs.is_empty() => {
            let kind = match &script.statements.last().expect("a statement").1 {
                Stmt::Value(e) => k.of(e)?.0,
                Stmt::Set { name, .. } | Stmt::Out(name) => k.name(name)?,
            };
            vec![("map".into(), kind)]
        }
        outs => outs.into_iter().map(|o| Ok((o.clone(), k.name(&o)?))).collect::<Result<_>>()?,
    };
    let floats = std::mem::take(&mut k.floats);
    let numbers = Script { statements, ..script.clone() };
    // the scope: the attributes it reads (texts by their numbers), `feature`, the measures, the inputs
    let n = v.len();
    let mut made: Vec<(String, Vec<f64>)> = Vec::new();
    for name in &names {
        if let Some(Column::Texts(t)) = v.column(name) {
            made.push((name.clone(), t.iter().map(|x| code(&texts, x)).collect()));
        }
    }
    for name in &floats {
        if let Some(c) = v.column(name) {
            made.push((float_name(name), c.numbers()));
        }
    }
    if v.column("feature").is_none() {
        made.push(("feature".into(), (1..=n).map(|i| i as f64).collect()));
    }
    let measured: Vec<(String, Vec<f64>)> = geometry.iter().filter_map(|m| Some((m.clone(), measure(v, m)?))).collect();
    let mut scope =
        Scope { width: n, height: 1, maps: BTreeMap::new(), numbers: BTreeMap::new(), stacks: BTreeMap::new() };
    for (name, c) in &v.attributes {
        if let Column::Numbers(x) = c
            && name != "geometry"
        {
            scope.maps.insert(name.as_str(), x.as_slice());
        }
    }
    for (name, cells) in &made {
        scope.maps.insert(name.as_str(), cells.as_slice());
    }
    for (port, x) in &others {
        let x = match x {
            Value::Text(t) => code(&texts, t),
            x => x.as_number().expect("a number"),
        };
        scope.numbers.insert(port.as_str(), x);
    }
    if !measured.is_empty() {
        let layers = measured.iter().map(|(m, cells)| (Key::Name(m.clone()), cells.as_slice())).collect();
        scope.stacks.insert("geometry", layers);
    }
    let text = |x: f64| if x.is_nan() { String::new() } else { texts.get(x as usize).cloned().unwrap_or_default() };
    numbers
        .run(&scope)?
        .into_iter()
        .zip(kinds)
        .map(|((name, o), (_, kind))| {
            let values = match o {
                Out::Number(x) => vec![x; n],
                Out::Map(cells) => cells,
                Out::Stack(_) => return Err(Error::msg(format!("{name} is not a value per feature"))),
            };
            let c = match kind {
                Kind::Text => Column::Texts(values.into_iter().map(text).collect()),
                _ => Column::Numbers(values),
            };
            Ok((name, c))
        })
        .collect()
}

/// The features `vector.filter` keeps, by their place: those where the script's last line is true (a feature
/// where it is missing left out).
pub fn kept(v: &Vector, inputs: &BTreeMap<String, Value>, script: &Script) -> Result<Vec<usize>> {
    let mut given = run(v, inputs, script)?;
    if !script.outputs().is_empty() {
        return Err(Error::msg("vector.filter keeps the features where the script's last line is true: no out"));
    }
    let Column::Numbers(keep) = given.remove(0).1 else {
        return Err(Error::msg("the expression gives texts: a feature is kept where it is true (uso == \"forest\")"));
    };
    Ok(keep.iter().enumerate().filter(|(_, k)| !k.is_nan() && **k != 0.0).map(|(i, _)| i).collect())
}

/// The features with the attributes `vector.calc` gives: each `out` (a script without, its last value as
/// `column`), an attribute of the same name replaced, else added.
pub fn calculated(v: &Vector, inputs: &BTreeMap<String, Value>, script: &Script, column: &str) -> Result<Vector> {
    let mut given = run(v, inputs, script)?;
    match (script.outputs().is_empty(), column.is_empty()) {
        (true, true) => {
            return Err(Error::msg(
                "name the column, or give the attributes with out (out density = pop / geometry.area)",
            ));
        }
        (true, false) => given[0].0 = column.to_string(),
        (false, false) => {
            return Err(Error::msg(format!(
                "the column {column:?} and a script with out: the out lines name the attributes (leave the column \
                 empty)"
            )));
        }
        (false, true) => {}
    }
    let mut v = v.clone();
    for (name, c) in given {
        match v.attributes.iter_mut().find(|(n, _)| *n == name) {
            Some((_, old)) => *old = c,
            None => v.attributes.push((name, c)),
        }
    }
    Ok(v)
}

/// The name [`every_name`] gives the value of a script's last line (one without `out`).
pub const LAST_LINE: &str = "(its last line)";

/// What every name a script gives a value holds at the end: its outputs first (a script without `out` ending
/// with a value: that value, named [`LAST_LINE`]), then the others.
pub fn every_name(v: &Vector, inputs: &BTreeMap<String, Value>, script: &Script) -> Result<Vec<(String, Column)>> {
    let mut s = script.clone();
    if script.outputs().is_empty()
        && let Some((line, Stmt::Value(e))) = s.statements.last().cloned()
    {
        let value = Stmt::Set { name: LAST_LINE.into(), layer: None, mask: None, value: e, out: true };
        *s.statements.last_mut().expect("a statement") = (line, value);
    }
    let line = s.statements.last().map_or(1, |(l, _)| *l);
    let mut given: Vec<String> = Vec::new();
    for (_, st) in &script.statements {
        if let Stmt::Set { name, .. } = st
            && !given.contains(name)
        {
            given.push(name.clone());
        }
    }
    let outs = s.outputs();
    s.statements.extend(given.into_iter().filter(|g| !outs.contains(g)).map(|g| (line, Stmt::Out(g))));
    run(v, inputs, &s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::algebra::parse_script;
    use terraflux_raster::vector::Geometry;

    fn squares() -> Vector {
        let square =
            |x: f64, s: f64| Geometry::Polygon(vec![vec![[x, 0.0], [x + s, 0.0], [x + s, s], [x, s], [x, 0.0]]]);
        Vector {
            geometries: vec![square(0.0, 1.0), square(10.0, 2.0), square(20.0, 3.0)],
            attributes: vec![
                ("uso".into(), Column::Texts(vec!["floresta".into(), "pasto".into(), String::new()])),
                ("pop".into(), Column::Numbers(vec![10.0, 20.0, 90.0])),
                ("code".into(), Column::Texts(vec!["12.5".into(), "x".into(), "3".into()])),
            ],
            crs: None,
        }
    }

    fn calc(text: &str, inputs: &[(&str, Value)]) -> Result<Vec<(String, Column)>> {
        let inputs = inputs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        run(&squares(), &inputs, &parse_script(text)?)
    }

    fn numbers(c: &Column) -> Vec<f64> {
        match c {
            Column::Numbers(x) => x.clone(),
            Column::Texts(t) => panic!("texts {t:?}"),
        }
    }

    fn texts(c: &Column) -> Vec<String> {
        match c {
            Column::Texts(t) => t.clone(),
            Column::Numbers(x) => panic!("numbers {x:?}"),
        }
    }

    fn same(a: &[f64], b: &[f64]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x.is_nan() && y.is_nan()) || (x - y).abs() < 1e-12)
    }

    #[test]
    fn texts_compared() {
        let r = calc("uso == \"floresta\"", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[1.0, 0.0, f64::NAN]), "{r:?}");
        let r = calc("uso != 'floresta' and pop > 5", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[0.0, 1.0, f64::NAN]), "{r:?}");
        // in the order of their characters, as Python's
        let r = calc("\"g\" <= uso < \"q\"", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[0.0, 1.0, f64::NAN]), "{r:?}");
        let r = calc("isnull(uso)", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[0.0, 0.0, 1.0]), "{r:?}");
        // a text input
        let r = calc("uso == classe", &[("classe", Value::Text("pasto".into()))]).unwrap();
        assert!(same(&numbers(&r[0].1), &[0.0, 1.0, f64::NAN]), "{r:?}");
        let r = calc("uso.count()", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[2.0, 2.0, 2.0]), "{r:?}");
    }

    #[test]
    fn texts_given() {
        let r =
            calc("out size = \"big\" if pop > 15 else \"small\"\nout uso\nuso[isnull(uso)] = \"outro\"", &[]).unwrap();
        assert_eq!(texts(&r[0].1), ["small", "big", "big"]);
        assert_eq!(texts(&r[1].1), ["floresta", "pasto", "outro"]);
        let r = calc("coalesce(uso, \"nada\")", &[]).unwrap();
        assert_eq!(texts(&r[0].1), ["floresta", "pasto", "nada"]);
        let r = calc("out label = \"x\"\nout none = None", &[]).unwrap();
        assert_eq!(texts(&r[0].1), ["x", "x", "x"]);
        assert!(numbers(&r[1].1).iter().all(|x| x.is_nan()));
        // a text that holds a number
        let r = calc("float(code) * 2", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[25.0, f64::NAN, 6.0]), "{r:?}");
        let r = calc("uso = 3\nfloat(uso) + float('1')", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[4.0, 4.0, 4.0]), "{r:?}");
    }

    #[test]
    fn texts_refused() {
        let e = |t: &str| calc(t, &[]).unwrap_err().0;
        assert!(e("uso > 3").contains("a text compared with a number (uso > a number)"), "{}", e("uso > 3"));
        assert!(e("uso + \"a\"").contains("not joined"));
        assert!(e("pop * uso").contains("uso is a text where a number goes"));
        assert!(
            e("x = 1\nx[pop > 1] = \"a\"").contains("line 2: x[...] = ...: its values are both texts or both numbers")
        );
        assert!(e("\"a\" if pop > 1 else 2").contains("a if c else b"));
        assert!(e("uso.mean()").contains("where a number goes"));
        assert!(e("u = uso\nfloat(u)").contains("u is a text the script computes"));
        assert!(e("nbsum(pop, 1)").contains("no neighbours"));
        assert!(e("usos == 'a'").contains("neither an attribute (the features': uso, pop, code) nor an input"));
    }

    #[test]
    fn geometry_measures() {
        let r = calc("out a = geometry.area\nout p = geometry.perimeter\nout x = geometry.x\nout share = pop / geometry.area / (pop / geometry.area).sum()", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[1.0, 4.0, 9.0]));
        assert!(same(&numbers(&r[1].1), &[4.0, 8.0, 12.0]));
        assert!(same(&numbers(&r[2].1), &[0.5, 11.0, 21.5]));
        let d = [10.0, 5.0, 10.0];
        let s: f64 = d.iter().sum();
        assert!(same(&numbers(&r[3].1), &d.map(|x| x / s)));
        let e = |t: &str| calc(t, &[]).unwrap_err().0;
        assert!(e("geometry.volume").contains("no measure \"volume\""));
        assert!(e("geometry").contains("read by its measures"));
        assert!(e("geometry = 1").contains("another name"));
        assert!(e("pop.x").contains("an attribute has no layers"));
        let r = calc("feature * 10", &[]).unwrap();
        assert!(same(&numbers(&r[0].1), &[10.0, 20.0, 30.0]));
    }

    #[test]
    fn every_name_given() {
        let s = parse_script("d = pop / geometry.area\nbig = d > 9\nbig and uso == 'floresta'").unwrap();
        let r = every_name(&squares(), &BTreeMap::new(), &s).unwrap();
        let names: Vec<&str> = r.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, [LAST_LINE, "d", "big"]);
        assert!(same(&numbers(&r[0].1), &[1.0, 0.0, f64::NAN]));
        assert!(same(&numbers(&r[1].1), &[10.0, 5.0, 10.0]));
        let s = parse_script("out label = 'a'\nn = 2").unwrap();
        let names: Vec<String> =
            every_name(&squares(), &BTreeMap::new(), &s).unwrap().into_iter().map(|x| x.0).collect();
        assert_eq!(names, ["label", "n"]);
    }

    #[test]
    fn on_maps_texts_refused() {
        let x = [1.0];
        let scope = Scope {
            width: 1,
            height: 1,
            maps: BTreeMap::from([("x", x.as_slice())]),
            numbers: BTreeMap::new(),
            stacks: BTreeMap::new(),
        };
        let e = |t: &str| parse_script(t).unwrap().run(&scope).unwrap_err().0;
        assert!(e("x == \"a\"").contains("vector features"), "{}", e("x == \"a\""));
        assert!(e("float(x)").contains("vector features"));
        assert!(parse_script("x == \"a").unwrap_err().0.contains("a text not closed"));
        let quoted = parse_script(r#"x == "a\"b""#).unwrap();
        assert!(
            matches!(&quoted.statements[0].1, Stmt::Value(Expr::Binary(_, _, b)) if **b == Expr::Str("a\"b".into()))
        );
    }
}
