//! `objects.calc`: the objects' calculator. A script of [`crate::algebra`] (Python's syntax, `out name = value`, as
//! `vector.calc`) computed for every object at once over the objects' table, plus what only objects have:
//!
//! - **Names:** each attribute of the table, `cls` (the object's class name, `"unclassified"` when it has none, as eCognition says), `id`, `cells`
//!   and `area` (unless the table has a column of that name).
//! - **Neighbours:** `nb.mean(x)` (the mean of `x` over the neighbours weighted by the length of the shared border, as
//!   eCognition's), `nb.min(x)`, `nb.max(x)`, `nb.sum(x)`, `nb.count(cond)` (the neighbours where it is true);
//!   `border_to("water")` (the share of the object's perimeter, the image's edge included as in `objects.features`'
//!   `rel_border`, shared with objects of that class) and `distance_to("road")` (from the object's centroid to the
//!   nearest centroid of an object of that class, in map units; 0 for an object of the class; NaN when there is
//!   none).
//! - **Levels:** `parent.x` (the attribute `x` of the object's parent, a level above is needed) and, with the input
//!   `below` (objects whose parent level is these objects), `children.count(cond)`, `children.area(cond)` (the share
//!   of the object's cells that are in children where `cond` holds) and `children.mean(x)`. Their argument is
//!   computed over the children (their `cls`, their attributes, their `parent.x`).
//! - **Fuzzy membership:** `above(x, a, b)`, `below(x, a, b)`, `between(x, a, b, c, d)` and `about(x, m, w)` (0 to 1,
//!   a missing value stays missing); and, always, `and` is the smaller of its two values, `or` the larger and `not x`
//!   is 1 - x (with values of 0 and 1 that is logic as usual).
//!
//! The calls are done before the script runs: each is computed over the objects (its argument as a script over the
//! objects), kept as a hidden column and put in the script's tree in place of the call; the fuzzy functions, `and`,
//! `or` and `not` are written there as the arithmetic they are. What remains is a plain script over the objects'
//! columns, which [`crate::vector_algebra::run_frame`] runs (texts compared with texts, as in `vector.calc`).

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::Column;

use super::features::nearest;
use super::{Objects, objects_in};
use crate::algebra::{Expr, Script, Stmt, parse_script_objects};
use crate::vector_algebra::{Frame, run_frame, script_name};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Calc));
}

/// The columns a script over objects reads: the table's (by their script names), then `cls`, `id`, `cells`, `area`
/// where the table has none of the name.
pub(super) fn attributes(o: &Objects) -> Vec<(String, Column)> {
    let t = &o.table;
    let mut out: Vec<(String, Column)> = Vec::new();
    for (j, c) in t.columns.iter().enumerate() {
        let name = script_name(c);
        if out.iter().any(|(n, _)| *n == name) {
            continue;
        }
        out.push((
            name,
            if t.is_text(c) {
                Column::Texts(t.rows.iter().map(|r| t.cell(j, r[j])).collect())
            } else {
                Column::Numbers(t.rows.iter().map(|r| r[j]).collect())
            },
        ));
    }
    let mut add = |name: &str, c: Column| {
        if !out.iter().any(|(n, _)| n == name) {
            out.push((name.to_string(), c));
        }
    };
    add(
        "cls",
        Column::Texts((1..=o.count as u32).map(|id| o.class_name(id).unwrap_or("unclassified").to_string()).collect()),
    );
    add("id", Column::Numbers((1..=o.count).map(|i| i as f64).collect()));
    add("cells", Column::Numbers(o.cells.iter().map(|&c| c as f64).collect()));
    let area = o.cell_area();
    add("area", Column::Numbers(o.cells.iter().map(|&c| c as f64 * area).collect()));
    out
}

/// Whether `below` is the level beneath `o` (its parent level is the same objects).
fn check_below(o: &Objects, below: Option<&Objects>) -> Result<()> {
    let Some(b) = below else { return Ok(()) };
    let same = b
        .parent
        .as_ref()
        .is_some_and(|p| p.count == o.count && (Arc::ptr_eq(&p.labels, &o.labels) || *p.labels == *o.labels));
    if same {
        Ok(())
    } else {
        Err(Error::msg("below: objects whose parent level is these objects (segment from a level)"))
    }
}

fn bin(op: &'static str, a: Expr, b: Expr) -> Expr {
    Expr::Binary(op, Box::new(a), Box::new(b))
}

/// A fuzzy function written as arithmetic over its (already rewritten) arguments.
fn fuzzy(f: &str, a: &[Expr]) -> Expr {
    let clamp01 = |e: Expr| Expr::Call("clamp".into(), vec![e, Expr::Num(0.0), Expr::Num(1.0)]);
    // 0 up to `lo`, 1 from `hi`, a line between
    let up = |x: &Expr, lo: &Expr, hi: &Expr| {
        clamp01(bin("/", bin("-", x.clone(), lo.clone()), bin("-", hi.clone(), lo.clone())))
    };
    let down = |x: &Expr, lo: &Expr, hi: &Expr| bin("-", Expr::Num(1.0), up(x, lo, hi));
    match f {
        "above" => up(&a[0], &a[1], &a[2]),
        "below" => down(&a[0], &a[1], &a[2]),
        "between" => Expr::Call("min".into(), vec![up(&a[0], &a[1], &a[2]), down(&a[0], &a[3], &a[4])]),
        "about" => {
            let off = Expr::Call("abs".into(), vec![bin("-", a[0].clone(), a[1].clone())]);
            clamp01(bin("-", Expr::Num(1.0), bin("/", off, a[2].clone())))
        }
        _ => unreachable!("a fuzzy function"),
    }
}

/// The objects of one level and what a script over them has been given so far.
struct Level<'a> {
    o: &'a Objects,
    below: Option<&'a Objects>,
    /// The columns: the attributes, then the hidden ones the calls made.
    attrs: Vec<(String, Column)>,
    /// The statements rewritten before the one at hand (a name they gave is a value an argument can read).
    done: Vec<(usize, Stmt)>,
}

impl<'a> Level<'a> {
    fn new(o: &'a Objects, below: Option<&'a Objects>) -> Level<'a> {
        Level { o, below, attrs: attributes(o), done: Vec::new() }
    }

    /// An expression (already rewritten) computed for every object.
    fn eval(&self, e: &Expr) -> Result<Column> {
        // the statements before, as names only (no outputs)
        let mut statements: Vec<(usize, Stmt)> = self
            .done
            .iter()
            .filter_map(|(l, s)| match s {
                Stmt::Out(_) => None,
                Stmt::Set { name, layer, mask, value, .. } => Some((
                    *l,
                    Stmt::Set {
                        name: name.clone(),
                        layer: layer.clone(),
                        mask: mask.clone(),
                        value: value.clone(),
                        out: false,
                    },
                )),
                s => Some((*l, s.clone())),
            })
            .collect();
        statements.push((statements.last().map_or(1, |s| s.0), Stmt::Value(e.clone())));
        let frame = Frame { n: self.o.count, attributes: &self.attrs, vector: None };
        let mut r = run_frame(&frame, &BTreeMap::new(), &Script { statements, lines: false })?;
        Ok(r.remove(0).1)
    }

    /// An expression (already rewritten) as numbers: a text is refused.
    fn numbers(&self, e: &Expr) -> Result<Vec<f64>> {
        match self.eval(e)? {
            Column::Numbers(x) => Ok(x),
            Column::Texts(_) => Err(Error::msg("a text where a number goes: compare it (cls == \"water\")")),
        }
    }

    /// A hidden column, by the call that made it; the name to read it by.
    fn hide(&mut self, key: String, c: impl FnOnce(&Level) -> Result<Column>) -> Result<Expr> {
        if !self.attrs.iter().any(|(n, _)| *n == key) {
            let c = c(self)?;
            self.attrs.push((key.clone(), c));
        }
        Ok(Expr::Var(key))
    }

    /// The perimeter of each object in cell edges: the edges that face another object, no object or the image's edge.
    fn perimeter(&self) -> Vec<f64> {
        let o = self.o;
        let (w, h) = (o.grid.width, o.grid.height);
        let mut p = vec![0.0f64; o.count];
        for r in 0..h {
            for c in 0..w {
                let i = r * w + c;
                let a = o.labels[i];
                if a == 0 {
                    continue;
                }
                p[a as usize - 1] += ((c == 0 || o.labels[i - 1] != a) as usize
                    + (c + 1 == w || o.labels[i + 1] != a) as usize
                    + (r == 0 || o.labels[i - w] != a) as usize
                    + (r + 1 == h || o.labels[i + w] != a) as usize) as f64;
            }
        }
        p
    }

    /// The centroids in map coordinates (as `objects.features`' centroid_x and centroid_y).
    fn centroids(&self) -> Vec<(f64, f64)> {
        let o = self.o;
        let (w, t) = (o.grid.width, o.grid.transform);
        let mut s = vec![(0.0f64, 0.0f64); o.count];
        for (i, &l) in o.labels.iter().enumerate() {
            if l != 0 {
                let e = &mut s[l as usize - 1];
                e.0 += (i / w) as f64;
                e.1 += (i % w) as f64;
            }
        }
        s.iter()
            .zip(&o.cells)
            .map(|(&(sr, sc), &n)| {
                let (cr, cc) = (sr / n as f64 + 0.5, sc / n as f64 + 0.5);
                (t[0] + t[1] * cc + t[2] * cr, t[3] + t[4] * cc + t[5] * cr)
            })
            .collect()
    }

    /// The number of a class (1..) by its name.
    fn class(&self, f: &str, name: &str) -> Result<u32> {
        if self.o.classes.is_empty() {
            return Err(Error::msg(format!("{f}({name:?}): the objects have no classes (classify them first)")));
        }
        match self.o.classes.iter().position(|c| c.name == name) {
            Some(k) => Ok(k as u32 + 1),
            None => {
                let have: Vec<&str> = self.o.classes.iter().map(|c| c.name.as_str()).collect();
                Err(Error::msg(format!("{f}({name:?}): no class of that name (the classes: {})", have.join(", "))))
            }
        }
    }

    /// `parent.m`: an attribute of the parent, for each object.
    fn parent(&mut self, m: &str) -> Result<Expr> {
        let o = self.o;
        let Some(p) = o.parent.as_ref() else {
            return Err(Error::msg(format!("parent.{m} needs a parent level (segment from a level)")));
        };
        self.hide(format!("parent.{m}"), |_| {
            let pa = attributes(p);
            let Some((_, col)) = pa.iter().find(|(n, _)| n == m) else {
                let have: Vec<&str> = pa.iter().map(|(n, _)| n.as_str()).collect();
                return Err(Error::msg(format!("the parent level has no attribute {m:?} ({})", have.join(", "))));
            };
            let at = |k: usize| (o.parent_of[k] as usize).checked_sub(1);
            Ok(match col {
                Column::Numbers(x) => Column::Numbers((0..o.count).map(|k| at(k).map_or(f64::NAN, |q| x[q])).collect()),
                Column::Texts(x) => {
                    Column::Texts((0..o.count).map(|k| at(k).map_or(String::new(), |q| x[q].clone())).collect())
                }
            })
        })
    }

    /// `nb.f(x)`: over the neighbours of each object.
    fn neighbours(&mut self, f: &str, key: String, arg: &Expr) -> Result<Expr> {
        let x = self.numbers(arg)?;
        self.hide(key, |l| {
            let o = l.o;
            Ok(Column::Numbers(
                (0..o.count)
                    .map(|k| {
                        let seen = o.adjacency[k].iter().map(|&(j, len)| (x[j as usize - 1], len as f64));
                        let seen = seen.filter(|(v, _)| !v.is_nan());
                        match f {
                            "mean" => {
                                let (s, w) = seen.fold((0.0, 0.0), |(s, w), (v, len)| (s + v * len, w + len));
                                if w == 0.0 { f64::NAN } else { s / w }
                            }
                            "sum" => seen.map(|(v, _)| v).sum(),
                            "count" => seen.filter(|(v, _)| *v != 0.0).count() as f64,
                            "min" => seen.map(|(v, _)| v).fold(f64::NAN, f64::min),
                            _ => seen.map(|(v, _)| v).fold(f64::NAN, f64::max),
                        }
                    })
                    .collect(),
            ))
        })
    }

    /// `children.f(x)`: over the children of each object (`x` computed over them).
    fn children(&mut self, f: &str, key: String, arg: &Expr) -> Result<Expr> {
        let Some(b) = self.below else {
            return Err(Error::msg(format!(
                "children.{f} needs the input below (objects whose parent level is these objects)"
            )));
        };
        // the argument over the children, a level of its own (its parent is this level)
        let mut lower = Level::new(b, None);
        let arg = lower.rewrite(arg)?;
        let x = lower.numbers(&arg)?;
        self.hide(key, |l| {
            let o = l.o;
            let mut n = vec![0.0f64; o.count];
            let mut s = vec![0.0f64; o.count];
            for (j, &p) in b.parent_of.iter().enumerate() {
                if p == 0 || x[j].is_nan() {
                    continue;
                }
                let k = p as usize - 1;
                match f {
                    "mean" => {
                        n[k] += 1.0;
                        s[k] += x[j];
                    }
                    "area" => s[k] += if x[j] != 0.0 { b.cells[j] as f64 / o.cells[k] as f64 } else { 0.0 },
                    _ => s[k] += (x[j] != 0.0) as u8 as f64,
                }
            }
            if f == "mean" {
                s.iter_mut().zip(&n).for_each(|(s, &n)| *s = if n == 0.0 { f64::NAN } else { *s / n });
            }
            Ok(Column::Numbers(s))
        })
    }

    /// An expression with its calls of objects computed (hidden columns put in their place) and the fuzzy functions,
    /// `and`, `or` and `not` written as arithmetic.
    fn rewrite(&mut self, e: &Expr) -> Result<Expr> {
        Ok(match e {
            Expr::Num(_) | Expr::Str(_) | Expr::Var(_) => e.clone(),
            Expr::Layer(of, m) if of == "parent" => self.parent(m)?,
            Expr::Layer(..) => e.clone(),
            Expr::Unary('!', a) => bin("-", Expr::Num(1.0), self.rewrite(a)?),
            Expr::Unary(op, a) => Expr::Unary(*op, Box::new(self.rewrite(a)?)),
            Expr::Binary("and", a, b) => Expr::Call("min".into(), vec![self.rewrite(a)?, self.rewrite(b)?]),
            Expr::Binary("or", a, b) => Expr::Call("max".into(), vec![self.rewrite(a)?, self.rewrite(b)?]),
            Expr::Binary(op, a, b) => bin(op, self.rewrite(a)?, self.rewrite(b)?),
            Expr::If(c, a, b) => {
                Expr::If(Box::new(self.rewrite(c)?), Box::new(self.rewrite(a)?), Box::new(self.rewrite(b)?))
            }
            Expr::Method(x, m, args) => Expr::Method(
                Box::new(self.rewrite(x)?),
                m.clone(),
                args.iter().map(|a| self.rewrite(a)).collect::<Result<_>>()?,
            ),
            Expr::Call(f, args) => match f.as_str() {
                "border_to" | "distance_to" => {
                    let Some(Expr::Str(name)) = args.first() else {
                        return Err(Error::msg(format!("{f} takes the name of a class, in quotes: {f}(\"water\")")));
                    };
                    let c = self.class(f, name)?;
                    self.hide(format!("{e:?}"), |l| Ok(Column::Numbers(l.to_class(f, c))))?
                }
                "nb.mean" | "nb.min" | "nb.max" | "nb.sum" | "nb.count" => {
                    let arg = self.rewrite(&args[0])?;
                    self.neighbours(&f[3..], format!("{e:?}"), &arg)?
                }
                "children.count" | "children.area" | "children.mean" => {
                    self.children(&f[9..], format!("{e:?}"), &args[0])?
                }
                _ => {
                    let args: Vec<Expr> = args.iter().map(|a| self.rewrite(a)).collect::<Result<_>>()?;
                    if matches!(f.as_str(), "above" | "below" | "between" | "about") {
                        fuzzy(f, &args)
                    } else {
                        Expr::Call(f.clone(), args)
                    }
                }
            },
        })
    }

    /// `border_to` or `distance_to` the class `c` of each object.
    fn to_class(&self, f: &str, c: u32) -> Vec<f64> {
        let o = self.o;
        if f == "border_to" {
            let p = self.perimeter();
            return (0..o.count)
                .map(|k| {
                    let l: f64 = o.adjacency[k]
                        .iter()
                        .filter(|&&(nb, _)| o.class_of[nb as usize - 1] == c)
                        .map(|&(_, l)| l as f64)
                        .sum();
                    if p[k] == 0.0 { f64::NAN } else { l / p[k] }
                })
                .collect();
        }
        let members: Vec<usize> = (0..o.count).filter(|&k| o.class_of[k] == c).collect();
        let mut d = nearest(&self.centroids(), &members);
        members.iter().for_each(|&k| d[k] = 0.0);
        d
    }

    fn statement(&mut self, s: &Stmt) -> Result<Stmt> {
        Ok(match s {
            Stmt::Set { name, layer, mask, value, out } => Stmt::Set {
                name: name.clone(),
                layer: layer.clone(),
                mask: mask.as_ref().map(|m| self.rewrite(m)).transpose()?,
                value: self.rewrite(value)?,
                out: *out,
            },
            Stmt::Out(n) => Stmt::Out(n.clone()),
            Stmt::Value(e) => Stmt::Value(self.rewrite(e)?),
        })
    }
}

/// What a script over objects gives: its outputs (`out`; without, its last value as `map`) by name, a column each, a
/// value per object (texts as `Column::Texts`). `below` are the objects of the level beneath, for `children`.
pub fn evaluate(o: &Objects, below: Option<&Objects>, script: &Script) -> Result<Vec<(String, Column)>> {
    check_below(o, below)?;
    let mut level = Level::new(o, below);
    let mut statements = Vec::with_capacity(script.statements.len());
    for (line, s) in &script.statements {
        let s2 = level.statement(s).map_err(|e| script.at(*line, e))?;
        level.done.push((*line, s2.clone()));
        statements.push((*line, s2));
    }
    let frame = Frame { n: o.count, attributes: &level.attrs, vector: None };
    run_frame(&frame, &BTreeMap::new(), &Script { statements, ..script.clone() })
}

/// [`evaluate`] as numbers (a text output is refused), for what reads a value per object (memberships).
pub fn evaluate_numbers(o: &Objects, below: Option<&Objects>, script: &Script) -> Result<Vec<(String, Vec<f64>)>> {
    evaluate(o, below, script)?
        .into_iter()
        .map(|(n, c)| match c {
            Column::Numbers(x) => Ok((n, x)),
            Column::Texts(_) => Err(Error::msg(format!("{n} is a text where a number goes"))),
        })
        .collect()
}

/// The table with the given columns: one of the same name replaced, else added; a text as the codes of its names
/// (in order) in `labels`.
fn with_columns(t: &Table, given: Vec<(String, Column)>) -> Result<Table> {
    let mut t = t.clone();
    for (name, c) in given {
        if name == "id" {
            return Err(Error::msg("id is the objects' number: give the value another name"));
        }
        let j = match t.columns.iter().position(|x| *x == name) {
            Some(j) => j,
            None => {
                t.columns.push(name.clone());
                t.rows.iter_mut().for_each(|r| r.push(f64::NAN));
                t.columns.len() - 1
            }
        };
        match c {
            Column::Numbers(x) => {
                t.labels.remove(&name);
                t.rows.iter_mut().zip(x).for_each(|(r, v)| r[j] = v);
            }
            Column::Texts(x) => {
                let mut names: Vec<String> = x.iter().filter(|s| !s.is_empty()).cloned().collect();
                names.sort();
                names.dedup();
                for (r, s) in t.rows.iter_mut().zip(&x) {
                    r[j] = names.binary_search(s).map_or(f64::NAN, |i| (i + 1) as f64);
                }
                t.labels.insert(name, names);
            }
        }
    }
    Ok(t)
}

/// `objects.calc`.
struct Calc;

impl Operator for Calc {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.calc",
            "Attributes of objects computed from a script over the others, in map algebra's language (Python's \
             syntax, as vector.calc): each attribute of the objects by its name, cls (the class name, \"unclassified\" \
             when it has none), id, cells and area, numbers and texts (\"wet\" if ndwi > 0 else \"dry\"; cls == \
             \"water\"). Of the context: nb.mean(x) (over the neighbours, weighted by the shared border, as \
             eCognition's), nb.min(x), nb.max(x), nb.sum(x), nb.count(cond), border_to(\"water\") (the share of the \
             perimeter shared with objects of that class), distance_to(\"road\") (centroid to the nearest centroid \
             of that class, map units; NaN when there is none), parent.x (an attribute of the parent: a level \
             above is needed) and, with `below`, children.count(cond), children.area(cond) (the share of the \
             object's area in children where it holds) and children.mean(x). Fuzzy membership: above(x, a, b), \
             below(x, a, b), between(x, a, b, c, d) and about(x, m, w), from 0 to 1; and, always, `and` is the \
             smaller of two values, `or` the larger and `not x` is 1 - x (with 0 and 1, logic as usual). A \
             comparison or arithmetic with a missing value gives missing (an objects.rules rule reads missing as \
             0): isnull(x) tests it and coalesce(x, y) gives x, and y where x is missing. One \
             statement a line (# a comment): name = value, name[where] = value, and out name = value (or out name) \
             for each attribute given; without out, the last value is the attribute `column`. An attribute of the \
             same name is replaced, else it is added.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with their attributes (objects.features)"))
        .input(PortSpec::optional(
            "below",
            Type::Objects,
            "the level beneath: objects whose parent level is these objects (for children.*)",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the objects with the attributes"))
        .output(PortSpec::new("table", Type::Table, "their attributes, a row per object"))
        .param(PortSpec::optional("column", Type::Text, "the attribute's name (a script without out)"))
        .param(PortSpec::new(
            "expression",
            Type::Text,
            "its value, e.g. nb.mean(mean_nir); or a script: wet = above(ndwi, 0, 0.3); out water = wet and \
             border_to(\"water\") > 0.2",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |e: Error| Error::msg(format!("node {node:?}: {}", e.0));
        let o = objects_in(args, "objects")?;
        let below = match args.get("below") {
            None => None,
            Some(Value::Objects(b)) => Some(&**b),
            Some(v) => return Err(bad(Error::msg(format!("below is a {:?}, not objects", v.ty())))),
        };
        let column: String = args.param("column", String::new())?;
        let text: String =
            args.opt_param("expression")?.ok_or_else(|| Error::msg(format!("node {node:?}: no expression")))?;
        let script =
            parse_script_objects(&text).map_err(|e| Error::msg(format!("node {node:?}: the expression: {}", e.0)))?;
        let mut given = evaluate(o, below, &script).map_err(bad)?;
        match (script.outputs().is_empty(), column.is_empty()) {
            (true, true) => {
                return Err(Error::msg(format!(
                    "node {node:?}: name the column, or give the attributes with out (out wet = above(ndwi, 0, 1))"
                )));
            }
            (true, false) => given[0].0 = column,
            (false, false) => {
                return Err(Error::msg(format!(
                    "node {node:?}: the column {column:?} and a script with out: the out lines name the attributes \
                     (leave the column empty)"
                )));
            }
            (false, true) => {}
        }
        let table = with_columns(&o.table, given).map_err(bad)?;
        let new = o.as_ref().clone().with_table(table.clone()).map_err(bad)?;
        let mut out = outputs([("table", Value::Table(table))]);
        out.insert("objects".into(), Value::Objects(Arc::new(new)));
        Ok(out)
    }
}
