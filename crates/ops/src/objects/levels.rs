//! The levels and the operations on objects: `objects.merge`, `objects.grow`, `objects.remove_small` and
//! `objects.reclassify`. A level above is made only by merging the objects of the level below (neighbours that join),
//! never by printing one segmentation into another nor by segmenting a layer filtered by objects.
//!
//! Every operation joins neighbours, so each object stays connected. An object left alone keeps its row of the
//! table; a merged object has its `cells` and `area` summed and every other numeric attribute as the mean of its parts
//! weighted by their cells (a text attribute: the value of the part with most cells), so run `objects.features` again
//! for exact attributes. The class of a merged object is that of its largest part; memberships are weighted like the
//! attributes. Objects inside a level above (their `parent`) only join objects of the same parent, so they stay
//! inside it.
//!
//! The loops "until nothing changes" are `control.repeat` with `until` the `done` output of the operation (true when it
//! changed nothing), or the `count` output (the number of objects) compared from a step to the next.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::Column;

use super::calc::{attributes, evaluate};
use super::{ObjectClass, Objects, find, objects_in};
use crate::algebra::{Expr, Script, Stmt, parse_script_objects};
use crate::vector_algebra::{Frame, run_frame, script_name};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Merge));
    r.add(Arc::new(Grow));
    r.add(Arc::new(RemoveSmall));
    r.add(Arc::new(Reclassify));
}

/// Sets of objects (index 0-based) that join: the smaller index stays the root.
struct Sets(Vec<u32>);

impl Sets {
    fn new(n: usize) -> Sets {
        Sets((0..n as u32).collect())
    }

    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (find(&mut self.0, a as u32), find(&mut self.0, b as u32));
        if ra != rb {
            self.0[ra.max(rb) as usize] = ra.min(rb);
        }
    }

    fn groups(&mut self) -> Vec<u32> {
        (0..self.0.len() as u32).map(|k| find(&mut self.0, k)).collect()
    }
}

/// The objects with each group of `group` (a root per object) made one object, ids by the first cell; and the new
/// id (1-based) of every old object.
fn regroup(o: &Objects, group: &[u32]) -> Result<(Objects, Vec<u32>)> {
    let mut id_of_root = vec![0u32; o.count];
    let mut next = 0u32;
    let mut labels = Vec::with_capacity(o.labels.len());
    for &l in o.labels.iter() {
        if l == 0 {
            labels.push(0);
            continue;
        }
        let r = group[l as usize - 1] as usize;
        if id_of_root[r] == 0 {
            next += 1;
            id_of_root[r] = next;
        }
        labels.push(id_of_root[r]);
    }
    let n = next as usize;
    let new_of: Vec<u32> = (0..o.count).map(|k| id_of_root[group[k] as usize]).collect();
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (k, &m) in new_of.iter().enumerate() {
        members[m as usize - 1].push(k);
    }
    let area = o.cell_area();
    let t = &o.table;
    let mut table = Table { columns: t.columns.clone(), rows: Vec::with_capacity(n), labels: t.labels.clone() };
    let weight = |k: usize| o.cells[k] as f64;
    for (m, parts) in members.iter().enumerate() {
        let id = (m + 1) as f64;
        let row = if parts.len() == 1 {
            let mut r = t.rows[parts[0]].clone();
            r[0] = id;
            r
        } else {
            let total: f64 = parts.iter().map(|&k| weight(k)).sum();
            t.columns
                .iter()
                .enumerate()
                .map(|(j, c)| match c.as_str() {
                    _ if j == 0 => id,
                    "cells" => total,
                    "area" => total * area,
                    _ if t.is_text(c) => {
                        let mut best = (f64::NAN, 0.0);
                        for &k in parts {
                            let code = t.rows[k][j];
                            if code.is_nan() {
                                continue;
                            }
                            let w: f64 = parts.iter().filter(|&&q| t.rows[q][j] == code).map(|&q| weight(q)).sum();
                            if w > best.1 {
                                best = (code, w);
                            }
                        }
                        best.0
                    }
                    _ => {
                        let (s, w) = parts
                            .iter()
                            .filter(|&&k| !t.rows[k][j].is_nan())
                            .fold((0.0, 0.0), |(s, w), &k| (s + t.rows[k][j] * weight(k), w + weight(k)));
                        if w > 0.0 { s / w } else { f64::NAN }
                    }
                })
                .collect()
        };
        table.rows.push(row);
    }
    let class_of: Vec<u32> = members
        .iter()
        .map(|parts| {
            let mut best = parts[0];
            for &k in parts {
                if o.cells[k] > o.cells[best] {
                    best = k;
                }
            }
            o.class_of[best]
        })
        .collect();
    let membership: Vec<Vec<f64>> = o
        .membership
        .iter()
        .map(|mem| {
            members
                .iter()
                .map(|parts| {
                    let total: f64 = parts.iter().map(|&k| weight(k)).sum();
                    parts.iter().map(|&k| mem[k] * weight(k)).sum::<f64>() / total
                })
                .collect()
        })
        .collect();
    let mut out = Objects::indexed(o.grid.clone(), labels, n).with_table(table)?.with_classes(
        o.classes.clone(),
        class_of,
        membership,
    )?;
    if let Some(p) = &o.parent {
        let parent_of = members.iter().map(|parts| o.parent_of[parts[0]]).collect();
        out = out.with_parent(p.clone(), parent_of)?;
    }
    Ok((out, new_of))
}

/// Whether two objects may join: of the same parent when there is a level above.
fn same_parent(o: &Objects, a: usize, b: usize) -> bool {
    o.parent.is_none() || o.parent_of[a] == o.parent_of[b]
}

/// The neighbouring pairs (a < b, 0-based) with the length of their border.
fn pairs(o: &Objects) -> Vec<(usize, usize, u32)> {
    let mut v = Vec::new();
    for (a, nb) in o.adjacency.iter().enumerate() {
        for &(b, len) in nb {
            if b as usize - 1 > a {
                v.push((a, b as usize - 1, len));
            }
        }
    }
    v
}

/// The expression with `a.x` and `b.x` as the columns `a_x` and `b_x` of the pair frame.
fn sides(e: &Expr) -> Expr {
    let all = |v: &[Expr]| v.iter().map(sides).collect::<Vec<_>>();
    match e {
        Expr::Layer(of, m) if of == "a" || of == "b" => Expr::Var(format!("{of}_{}", script_name(m))),
        Expr::Num(_) | Expr::Str(_) | Expr::Var(_) | Expr::Layer(..) => e.clone(),
        Expr::Unary(c, x) => Expr::Unary(*c, Box::new(sides(x))),
        Expr::Binary(op, x, y) => Expr::Binary(op, Box::new(sides(x)), Box::new(sides(y))),
        Expr::If(c, x, y) => Expr::If(Box::new(sides(c)), Box::new(sides(x)), Box::new(sides(y))),
        Expr::Call(f, args) => Expr::Call(f.clone(), all(args)),
        Expr::Method(x, m, args) => Expr::Method(Box::new(sides(x)), m.clone(), all(args)),
    }
}

/// A condition over the pairs of neighbours (`a.x`, `b.x`, `border`): true where it holds (not zero, not missing).
fn pair_condition(o: &Objects, ps: &[(usize, usize, u32)], text: &str) -> Result<Vec<bool>> {
    let script = parse_script_objects(text).map_err(|e| Error::msg(format!("the condition: {}", e.0)))?;
    let statements = script
        .statements
        .iter()
        .map(|(l, s)| {
            let s = match s {
                Stmt::Set { name, layer, mask, value, out } => Stmt::Set {
                    name: name.clone(),
                    layer: layer.clone(),
                    mask: mask.as_ref().map(sides),
                    value: sides(value),
                    out: *out,
                },
                Stmt::Out(n) => Stmt::Out(n.clone()),
                Stmt::Value(e) => Stmt::Value(sides(e)),
            };
            (*l, s)
        })
        .collect();
    let script = Script { statements, ..script };
    if ps.is_empty() {
        return Ok(Vec::new());
    }
    let mut cols: Vec<(String, Column)> = Vec::new();
    for (n, c) in attributes(o) {
        for (side, pick) in [("a", 0), ("b", 1)] {
            let at = |p: &(usize, usize, u32)| if pick == 0 { p.0 } else { p.1 };
            let col = match &c {
                Column::Numbers(x) => Column::Numbers(ps.iter().map(|p| x[at(p)]).collect()),
                Column::Texts(x) => Column::Texts(ps.iter().map(|p| x[at(p)].clone()).collect()),
            };
            cols.push((format!("{side}_{n}"), col));
        }
    }
    cols.push(("border".into(), Column::Numbers(ps.iter().map(|p| p.2 as f64).collect())));
    let frame = Frame { n: ps.len(), attributes: &cols, vector: None };
    let mut r = run_frame(&frame, &BTreeMap::new(), &script)?;
    if r.is_empty() {
        return Err(Error::msg("the condition gives no value"));
    }
    truth(r.remove(0).1)
}

/// A column as yes or no: a number that is not 0 and not missing is yes.
fn truth(c: Column) -> Result<Vec<bool>> {
    match c {
        Column::Numbers(x) => Ok(x.into_iter().map(|v| v != 0.0 && !v.is_nan()).collect()),
        Column::Texts(_) => Err(Error::msg("the condition is a text: compare it (cls == \"water\")")),
    }
}

/// The classes named, as their numbers (1-based).
fn class_numbers(o: &Objects, names: &[String]) -> Result<BTreeSet<u32>> {
    names
        .iter()
        .map(|n| {
            o.classes.iter().position(|c| c.name == *n).map(|k| k as u32 + 1).ok_or_else(|| {
                let have: Vec<&str> = o.classes.iter().map(|c| c.name.as_str()).collect();
                Error::msg(format!("no class {n:?} in the objects (classes: {})", have.join(", ")))
            })
        })
        .collect()
}

fn done_outputs(before: usize, o: Objects) -> Outputs {
    let count = o.count;
    outputs([
        ("objects", Value::Objects(Arc::new(o))),
        ("count", Value::Integer(count as i64)),
        ("done", Value::Bool(count == before)),
    ])
}

fn node_err(node: &str) -> impl Fn(Error) -> Error + '_ {
    move |e| Error::msg(format!("node {node:?}: {}", e.0))
}

/// `objects.merge`.
struct Merge;

impl Operator for Merge {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.merge",
            "Merges neighbouring objects. by = class (the default): neighbours of the same class become one object \
             (`classes`: only those classes); by = condition: the pairs of neighbours where `condition` holds, an \
             objects.calc expression over the pair (a.mean_red and b.mean_red are the attributes of the two \
             neighbours, border the length of their shared border, e.g. abs(a.mean_red - b.mean_red) < 0.1), are \
             merged, transitively. With `level` the result is a new level above and `below` holds the input objects \
             inside it (the objects of a level are made by merging, never by printing one segmentation into another). \
             An object left alone keeps its attributes; a merged one has cells and area summed and the other \
             numeric attributes as the mean of its parts weighted by their cells (objects.features again for exact \
             ones), the class of its largest part. Objects inside a level above only merge with those of the same \
             parent. `count` is the number of objects and `done` is true when nothing merged: in a control.repeat \
             set until to the done of this node to merge until nothing changes.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with their classes (by class) and attributes"))
        .output(PortSpec::new("objects", Type::Objects, "the merged objects (the new level with `level`)"))
        .output(PortSpec::optional(
            "below",
            Type::Objects,
            "with `level`: the input objects, their parent the merged object",
        ))
        .output(PortSpec::new("count", Type::Integer, "the number of objects after the merge"))
        .output(PortSpec::new("done", Type::Bool, "true when no objects merged"))
        .param(PortSpec::optional("by", Type::Text, "class (default) or condition"))
        .param(PortSpec::optional("classes", Type::Any, "by class: the class names that merge (default: all)"))
        .param(PortSpec::optional(
            "condition",
            Type::Text,
            "by condition: an expression over a pair of neighbours, with a.x, b.x and border",
        ))
        .param(PortSpec::optional("level", Type::Bool, "make a new level above (default: false, replace)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let bad = node_err(args.node);
        let o = objects_in(args, "objects")?;
        let condition: Option<String> = args.opt_param("condition")?.filter(|c: &String| !c.trim().is_empty());
        let by: String = args.param("by", if condition.is_some() { "condition" } else { "class" }.to_string())?;
        let level: bool = args.param("level", false)?;
        if level && o.parent.is_some() {
            return Err(bad(Error::msg("level: the objects have a level above already")));
        }
        let ps = pairs(o);
        let yes: Vec<bool> = match by.as_str() {
            "class" => {
                let only = match args.opt_param::<Vec<String>>("classes")? {
                    Some(names) if !names.is_empty() => Some(class_numbers(o, &names).map_err(&bad)?),
                    _ => None,
                };
                ps.iter()
                    .map(|&(a, b, _)| {
                        let c = o.class_of[a];
                        c != 0 && c == o.class_of[b] && only.as_ref().is_none_or(|s| s.contains(&c))
                    })
                    .collect()
            }
            "condition" => {
                let text = condition.ok_or_else(|| bad(Error::msg("by condition needs a condition")))?;
                pair_condition(o, &ps, &text).map_err(&bad)?
            }
            other => return Err(bad(Error::msg(format!("by {other:?}: class or condition")))),
        };
        let mut sets = Sets::new(o.count);
        for (&(a, b, _), _) in ps.iter().zip(&yes).filter(|(_, y)| **y) {
            if same_parent(o, a, b) {
                sets.union(a, b);
            }
        }
        let (merged, new_of) = regroup(o, &sets.groups())?;
        let mut out = done_outputs(o.count, merged);
        if level {
            let Value::Objects(m) = &out["objects"] else { unreachable!() };
            let below = o.as_ref().clone().with_parent(m.clone(), new_of)?;
            out.insert("below".into(), Value::Objects(Arc::new(below)));
        }
        Ok(out)
    }
}

/// `objects.grow`.
struct Grow;

impl Operator for Grow {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.grow",
            "Grows the objects of a class into the neighbours that meet a condition, as eCognition's grow: the \
             condition is an objects.calc expression evaluated on the candidate neighbour (e.g. mean_nir > 0.2 and \
             cls == \"unclassified\"); the neighbours where it holds (a candidate between two grown objects goes to \
             the one with the longer border) are merged in and keep the class, and it repeats, the condition \
             computed again over the grown objects, until nothing changes or `max_steps`. The merged objects' \
             attributes are the means of their parts weighted by cells (objects.features again for exact ones). \
             `count` is the number of objects, `done` is true when nothing grew.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with their classes and attributes"))
        .output(PortSpec::new("objects", Type::Objects, "the objects after the growth"))
        .output(PortSpec::new("count", Type::Integer, "the number of objects after the growth"))
        .output(PortSpec::new("done", Type::Bool, "true when nothing grew"))
        .param(PortSpec::new("class", Type::Text, "the class that grows"))
        .param(PortSpec::new(
            "condition",
            Type::Text,
            "what a neighbour meets to be merged in (an objects.calc expression)",
        ))
        .param(PortSpec::optional("max_steps", Type::Integer, "the most steps (default 100)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let bad = node_err(args.node);
        let o = objects_in(args, "objects")?;
        let class: String = args.param("class", String::new())?;
        let c = *class_numbers(o, std::slice::from_ref(&class)).map_err(&bad)?.first().unwrap();
        let text: String = args.param("condition", String::new())?;
        let script = parse_script_objects(&text).map_err(|e| bad(Error::msg(format!("the condition: {}", e.0))))?;
        let max: usize = args.param("max_steps", 100usize)?;
        let mut cur: Objects = o.as_ref().clone();
        for _ in 0..max {
            let mut r = evaluate(&cur, None, &script).map_err(&bad)?;
            if r.is_empty() {
                return Err(bad(Error::msg("the condition gives no value")));
            }
            let yes = truth(r.remove(0).1).map_err(&bad)?;
            let mut group: Vec<u32> = (0..cur.count as u32).collect();
            let mut changed = false;
            for k in 0..cur.count {
                if !yes[k] || cur.class_of[k] == c {
                    continue;
                }
                // the grown neighbour with the longest border
                let to = cur.adjacency[k]
                    .iter()
                    .filter(|&&(b, _)| cur.class_of[b as usize - 1] == c && same_parent(&cur, k, b as usize - 1))
                    .max_by_key(|&&(b, len)| (len, std::cmp::Reverse(b)));
                if let Some(&(b, _)) = to {
                    group[k] = b - 1;
                    changed = true;
                }
            }
            if !changed {
                break;
            }
            cur = regroup(&cur, &group)?.0;
        }
        Ok(done_outputs(o.count, cur))
    }
}

/// `objects.remove_small`.
struct RemoveSmall;

/// The attributes the similarity of two objects is told by: the bands' means, else every numeric attribute but the
/// size.
fn similarity_columns(t: &Table) -> Vec<usize> {
    let numeric = |c: &String| !t.is_text(c) && !["id", "cells", "area"].contains(&c.as_str());
    let means: Vec<usize> =
        (0..t.columns.len()).filter(|&j| t.columns[j].starts_with("mean_") && numeric(&t.columns[j])).collect();
    if means.is_empty() { (0..t.columns.len()).filter(|&j| numeric(&t.columns[j])).collect() } else { means }
}

impl Operator for RemoveSmall {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.remove_small",
            "Merges the objects smaller than `min_cells` (or `min_area`, map units squared) into a neighbour, as \
             eCognition's remove and raster.sieve: into = similar (the default) goes to the neighbour whose \
             band means (the mean_* attributes, else every numeric attribute but the size) are nearest, Euclidean \
             distance; into = border to the neighbour with the longest shared border. It repeats until no small \
             object is left (a merged group may still be small); one with no neighbour stays. `classes`: only the \
             small objects of these classes go. The merged objects' attributes are the means of their parts \
             weighted by cells (objects.features again for exact ones). `count` is the number of objects and `done` \
             is true when nothing merged.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with their attributes"))
        .output(PortSpec::new("objects", Type::Objects, "the objects without the small ones"))
        .output(PortSpec::new("count", Type::Integer, "the number of objects after"))
        .output(PortSpec::new("done", Type::Bool, "true when no object merged"))
        .param(PortSpec::optional("min_cells", Type::Number, "objects with fewer cells go"))
        .param(PortSpec::optional("min_area", Type::Number, "objects with a smaller area (map units squared) go"))
        .param(PortSpec::optional("into", Type::Text, "similar (default) or border"))
        .param(PortSpec::optional(
            "classes",
            Type::Any,
            "the class names whose small objects go (default: all)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let bad = node_err(args.node);
        let o = objects_in(args, "objects")?;
        let min_cells: Option<f64> = args.opt_param("min_cells")?;
        let min_area: Option<f64> = args.opt_param("min_area")?;
        if min_cells.is_none() && min_area.is_none() {
            return Err(bad(Error::msg("give min_cells or min_area")));
        }
        let similar = match args.param("into", "similar".to_string())?.as_str() {
            "similar" => true,
            "border" => false,
            other => return Err(bad(Error::msg(format!("into {other:?}: similar or border")))),
        };
        let only = match args.opt_param::<Vec<String>>("classes")? {
            Some(names) if !names.is_empty() => Some(class_numbers(o, &names).map_err(&bad)?),
            _ => None,
        };
        let mut cur: Objects = o.as_ref().clone();
        let area = cur.cell_area();
        loop {
            let small: Vec<usize> = (0..cur.count)
                .filter(|&k| {
                    let cells = cur.cells[k] as f64;
                    (min_cells.is_some_and(|m| cells < m) || min_area.is_some_and(|m| cells * area < m))
                        && only.as_ref().is_none_or(|s| s.contains(&cur.class_of[k]))
                })
                .collect();
            let cols = similarity_columns(&cur.table);
            let dist = |a: usize, b: usize| -> f64 {
                let (ra, rb) = (&cur.table.rows[a], &cur.table.rows[b]);
                let mut s = 0.0;
                let mut n = 0;
                for &j in &cols {
                    if !ra[j].is_nan() && !rb[j].is_nan() {
                        s += (ra[j] - rb[j]).powi(2);
                        n += 1;
                    }
                }
                if n == 0 { f64::INFINITY } else { s.sqrt() }
            };
            let mut sets = Sets::new(cur.count);
            let mut changed = false;
            for &k in &small {
                let nbs = cur.adjacency[k].iter().filter(|&&(b, _)| same_parent(&cur, k, b as usize - 1));
                let to = if similar {
                    // the nearest, the longest border and the lowest id deciding a tie
                    nbs.min_by(|&&(a, la), &&(b, lb)| {
                        dist(k, a as usize - 1).total_cmp(&dist(k, b as usize - 1)).then(lb.cmp(&la)).then(a.cmp(&b))
                    })
                } else {
                    nbs.max_by_key(|&&(b, len)| (len, std::cmp::Reverse(b)))
                };
                if let Some(&(b, _)) = to {
                    sets.union(k, b as usize - 1);
                    changed = true;
                }
            }
            if !changed {
                break;
            }
            cur = regroup(&cur, &sets.groups())?.0;
        }
        Ok(done_outputs(o.count, cur))
    }
}

/// `objects.reclassify`.
struct Reclassify;

impl Operator for Reclassify {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.reclassify",
            "Renames and joins classes: `classes` maps old names to new ones (e.g. {\"marsh\": \"wet\", \"water\": \
             \"wet\"}); a class not in the map keeps its name, and classes given the same name become one. \
             `colours` gives the colour of a class by its (new) name, #rrggbb. With `merge`, neighbours that now \
             share a class are merged as objects.merge by class does. `count` is the number of objects, `done` is \
             true when no objects merged.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the classified objects"))
        .output(PortSpec::new("objects", Type::Objects, "the objects with the new classes"))
        .output(PortSpec::new("count", Type::Integer, "the number of objects after"))
        .output(PortSpec::new("done", Type::Bool, "true when no objects merged"))
        .param(PortSpec::optional("classes", Type::Any, "old class name to new class name, as a table"))
        .param(PortSpec::optional("colours", Type::Any, "new class name to colour (#rrggbb), as a table"))
        .param(PortSpec::optional("merge", Type::Bool, "merge neighbours that share a class (default: false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let bad = node_err(args.node);
        let o = objects_in(args, "objects")?;
        let map: BTreeMap<String, String> = args.opt_param("classes")?.unwrap_or_default();
        let colours: BTreeMap<String, String> = args.opt_param("colours")?.unwrap_or_default();
        let merge: bool = args.param("merge", false)?;
        for old in map.keys() {
            if !o.classes.iter().any(|c| c.name == *old) {
                let have: Vec<&str> = o.classes.iter().map(|c| c.name.as_str()).collect();
                return Err(bad(Error::msg(format!("classes: no class {old:?} (classes: {})", have.join(", ")))));
            }
        }
        if let Some(n) = map.values().find(|n| n.trim().is_empty()) {
            return Err(bad(Error::msg(format!("classes: an empty name for {n:?}"))));
        }
        // the new classes in the order of the old ones; a new class has the first colour its old ones had
        let mut classes: Vec<ObjectClass> = Vec::new();
        let mut number: Vec<u32> = Vec::new();
        for c in &o.classes {
            let name = map.get(&c.name).unwrap_or(&c.name).clone();
            match classes.iter().position(|x| x.name == name) {
                Some(k) => {
                    if classes[k].colour.is_none() {
                        classes[k].colour = c.colour;
                    }
                    number.push(k as u32 + 1);
                }
                None => {
                    classes.push(ObjectClass { name, colour: c.colour });
                    number.push(classes.len() as u32);
                }
            }
        }
        for (name, text) in &colours {
            let k = classes
                .iter()
                .position(|c| c.name == *name)
                .ok_or_else(|| bad(Error::msg(format!("colours: no class {name:?} after the renaming"))))?;
            classes[k].colour =
                Some(crate::legend::rgb(text).ok_or_else(|| {
                    bad(Error::msg(format!("colours: the colour of {name:?}: #rrggbb, not {text:?}")))
                })?);
        }
        let class_of: Vec<u32> = o.class_of.iter().map(|&k| if k == 0 { 0 } else { number[k as usize - 1] }).collect();
        // the memberships of joined classes: the largest
        let mut membership: Vec<Vec<f64>> = Vec::new();
        if !o.membership.is_empty() {
            membership = vec![vec![0.0; o.count]; classes.len()];
            for (old, m) in o.membership.iter().enumerate() {
                let new = number[old] as usize - 1;
                for (x, &v) in membership[new].iter_mut().zip(m) {
                    *x = x.max(v);
                }
            }
        }
        // a table column of the classes' names follows them
        let mut table = o.table.clone();
        if table.is_text("class") && table.columns.iter().any(|c| c == "class") {
            let j = table.columns.iter().position(|c| c == "class").unwrap();
            table.labels.insert("class".into(), classes.iter().map(|c| c.name.clone()).collect());
            for (r, &k) in table.rows.iter_mut().zip(&class_of) {
                r[j] = if k == 0 { f64::NAN } else { k as f64 };
            }
        }
        let mut new = o.as_ref().clone().with_table(table)?.with_classes(classes, class_of, membership)?;
        if merge {
            let mut sets = Sets::new(new.count);
            for (a, b, _) in pairs(&new) {
                if new.class_of[a] != 0 && new.class_of[a] == new.class_of[b] && same_parent(&new, a, b) {
                    sets.union(a, b);
                }
            }
            new = regroup(&new, &sets.groups())?.0;
        }
        Ok(done_outputs(o.count, new))
    }
}
