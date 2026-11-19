//! `objects.rules` and `objects.nearest`: classes of objects by fuzzy membership, as eCognition's class hierarchy and
//! its Standard Nearest Neighbour.
//!
//! - **Rules:** each class has a rule (an [`super::calc`] expression: a membership from 0 to 1) and may have a parent;
//!   the membership of a class is the smaller of its own rule and its parent's membership (a child inherits its
//!   mother's rule and "and"s its own). Only the leaves of the hierarchy (the classes that are no one's parent) are
//!   what an object becomes: the one of the largest membership, unclassified when that is under `min`.
//! - **Nearest neighbour:** the membership of a class is the membership of the nearest sample of the class in the
//!   space of the chosen attributes (each divided by its standard deviation over the objects): `exp(-k d^2)` with
//!   `k = ln(1 / slope)`, so at a distance of 1 it is `slope`.
//!
//! Both give the same outputs: the objects with their classes and memberships, a table (id, class, `m_<class>`,
//! margin) and a map of the class of each cell.

use std::sync::Arc;

use serde::Deserialize;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::calc::evaluate_numbers;
use super::{ObjectClass, Objects, objects_in};
use crate::algebra::{Script, parse_script_objects};
use crate::vector_algebra::script_name;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Rules));
    r.add(Arc::new(Nearest));
}

/// A class of the hierarchy as the setting writes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Spec {
    name: String,
    #[serde(default)]
    rule: Option<String>,
    #[serde(default)]
    parent: Option<String>,
    #[serde(default, alias = "color")]
    colour: Option<String>,
}

/// A class of the hierarchy of `objects.rules`: its name, its mother (an index of the list), its rule's text (none: it
/// is as its mother) and its colour.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleClass {
    pub name: String,
    pub parent: Option<usize>,
    pub rule: Option<String>,
    pub colour: Option<[u8; 3]>,
}

/// The hierarchy of the setting `classes` (a list of `{ name, rule, parent, colour }`), in its order, checked: names
/// that are not empty or repeated, colours as #rrggbb, mothers that are in the list, no cycle.
pub fn tree(v: &serde_json::Value) -> Result<Vec<RuleClass>> {
    let specs: Vec<Spec> = serde_json::from_value(v.clone()).map_err(|e| {
        Error::msg(format!(
            "classes: a list of {{ name = \"water\", rule = \"above(ndwi, 0, 0.2)\", parent = \"wet\", colour = \
             \"#3366cc\" }} ({e})"
        ))
    })?;
    if specs.is_empty() {
        return Err(Error::msg("classes: the list is empty"));
    }
    let mut out: Vec<RuleClass> = Vec::with_capacity(specs.len());
    for (k, s) in specs.iter().enumerate() {
        if s.name.is_empty() {
            return Err(Error::msg(format!("classes: the class {} has no name", k + 1)));
        }
        if let Some(j) = specs[..k].iter().position(|p| p.name == s.name) {
            return Err(Error::msg(format!(
                "classes: the name {:?} is of the classes {} and {}",
                s.name,
                j + 1,
                k + 1
            )));
        }
        if let Some(j) = specs[..k].iter().position(|p| script_name(&p.name) == script_name(&s.name)) {
            return Err(Error::msg(format!(
                "classes: {:?} and {:?} give the same column name ({})",
                specs[j].name,
                s.name,
                script_name(&s.name)
            )));
        }
        let colour = match s.colour.as_deref().filter(|c| !c.is_empty()) {
            None => None,
            Some(c) => Some(
                crate::legend::rgb(c)
                    .ok_or_else(|| Error::msg(format!("classes: the colour of {:?}: #rrggbb, not {c:?}", s.name)))?,
            ),
        };
        let parent = match s.parent.as_deref().filter(|p| !p.is_empty()) {
            None => None,
            Some(p) => Some(specs.iter().position(|c| c.name == p).ok_or_else(|| {
                let have: Vec<&str> = specs.iter().map(|c| c.name.as_str()).collect();
                Error::msg(format!(
                    "classes: the parent {p:?} of {:?} is not a class of the list ({})",
                    s.name,
                    have.join(", ")
                ))
            })?),
        };
        out.push(RuleClass {
            name: s.name.clone(),
            parent,
            rule: s.rule.clone().filter(|r| !r.trim().is_empty()),
            colour,
        });
    }
    // a cycle: walking up from a class comes back to it (or to a class that never ends)
    for k in 0..out.len() {
        let mut at = k;
        let mut path = vec![out[k].name.clone()];
        while let Some(p) = out[at].parent {
            path.push(out[p].name.clone());
            if p == k || path.len() > out.len() + 1 {
                return Err(Error::msg(format!("classes: a cycle of parents: {}", path.join(" -> "))));
            }
            at = p;
        }
    }
    Ok(out)
}

/// The leaves of a hierarchy: the classes that are no one's parent.
pub fn leaves(t: &[RuleClass]) -> Vec<usize> {
    (0..t.len()).filter(|&k| !t.iter().any(|c| c.parent == Some(k))).collect()
}

/// The membership of every class (a vector per class over the objects): each rule's value, 0 to 1 (a missing value
/// is 0), the smaller of it and the mother's.
fn memberships(o: &Objects, below: Option<&Objects>, t: &[RuleClass]) -> Result<Vec<Vec<f64>>> {
    let n = o.count;
    let mut own: Vec<Option<Vec<f64>>> = Vec::with_capacity(t.len());
    for c in t {
        let Some(text) = &c.rule else {
            own.push(None);
            continue;
        };
        let at = |e: Error| Error::msg(format!("class {:?}: the rule: {}", c.name, e.0));
        let script: Script = parse_script_objects(text).map_err(at)?;
        let mut r = evaluate_numbers(o, below, &script).map_err(at)?;
        let (_, v) = r.pop().ok_or_else(|| at(Error::msg("it gives no value")))?;
        own.push(Some(v.into_iter().map(|x| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) }).collect()));
    }
    // mothers before daughters (no cycle: checked)
    let mut eff: Vec<Option<Vec<f64>>> = vec![None; t.len()];
    while eff.iter().any(Option::is_none) {
        for k in 0..t.len() {
            if eff[k].is_some() {
                continue;
            }
            let up = match t[k].parent {
                None => Some(vec![1.0; n]),
                Some(p) => eff[p].clone(),
            };
            let Some(up) = up else { continue };
            eff[k] = Some(match &own[k] {
                None => up,
                Some(v) => v.iter().zip(&up).map(|(a, b)| a.min(*b)).collect(),
            });
        }
    }
    Ok(eff.into_iter().map(Option::unwrap_or_default).collect())
}

/// The outputs of a classification by memberships: each object takes the candidate class (one of `candidates`, an
/// index of `classes`) of the largest membership, the first on ties, none when it is under `min`.
fn conclude(
    o: &Objects,
    classes: Vec<ObjectClass>,
    membership: Vec<Vec<f64>>,
    candidates: &[usize],
    min: f64,
) -> Result<Outputs> {
    let n = o.count;
    let mut class_of = vec![0u32; n];
    let mut margin = vec![0.0f64; n];
    for i in 0..n {
        let (mut best, mut top, mut second) = (usize::MAX, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for &c in candidates {
            let m = membership[c][i];
            if m > top {
                second = top;
                (best, top) = (c, m);
            } else if m > second {
                second = m;
            }
        }
        if best == usize::MAX {
            continue;
        }
        margin[i] = top - if second == f64::NEG_INFINITY { 0.0 } else { second };
        // (a membership of 0 in every class is none, even with min 0)
        if top >= min && top > 0.0 {
            class_of[i] = best as u32 + 1;
        }
    }
    let names: Vec<String> = classes.iter().map(|c| c.name.clone()).collect();
    let mut table = Table::new(
        ["id", "class"]
            .into_iter()
            .map(String::from)
            .chain(names.iter().map(|c| format!("m_{}", script_name(c))))
            .chain(["margin".to_string()])
            .collect(),
    );
    for i in 0..n {
        let mut row = vec![(i + 1) as f64, if class_of[i] == 0 { f64::NAN } else { class_of[i] as f64 }];
        row.extend(membership.iter().map(|m| m[i]));
        row.push(margin[i]);
        table.rows.push(row);
    }
    table.labels.insert("class".to_string(), names);
    let per: Vec<f64> = class_of.iter().map(|&k| if k == 0 { f64::NAN } else { k as f64 }).collect();
    let data: Vec<f64> = o.labels.iter().map(|&l| if l == 0 { f64::NAN } else { per[l as usize - 1] }).collect();
    let classed = o.clone().with_classes(classes, class_of, membership)?;
    let mut out = outputs([("table", Value::Table(table)), ("map", Value::Map(Map::new(o.grid.clone(), data)))]);
    out.insert("objects".into(), Value::Objects(Arc::new(classed)));
    Ok(out)
}

fn check_min(node: &str, min: f64) -> Result<()> {
    if (0.0..=1.0).contains(&min) { Ok(()) } else { Err(Error::msg(format!("node {node:?}: min {min}: 0 to 1"))) }
}

/// `objects.rules`.
struct Rules;

impl Operator for Rules {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.rules",
            "Objects classed by a hierarchy of fuzzy rules, as eCognition's class hierarchy. `classes` lists the \
             classes, each { name, rule, parent, colour }: the rule is an objects.calc expression, a membership from \
             0 to 1 (above(ndwi, 0, 0.2), below(ndvi, 0.2, 0.4) and not border_to(\"urban\") > 0.5; a value out of 0 \
             to 1 is cut, a missing one is 0; several lines are allowed, the last is the value); a class with a parent \
             inherits its membership: the membership of a class is the smaller of its own rule and its parent's (a \
             class without a rule is its parent's, or 1 at the root). Only the leaves (the classes that are no one's \
             parent) are what an object becomes: the leaf of the largest membership, the first of the list on ties, \
             unclassified when it is under `min`; the parents only narrow what their daughters may take. All the \
             classes are in the output, in the list's order, with the membership of each (a parent's region is its \
             descendants'). A rule that reads classes (border_to(\"water\"), nb.count(cls == \"water\")) reads the \
             classes the objects have before this node: chain two objects.rules for a second pass.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with the attributes the rules read"))
        .input(PortSpec::optional(
            "below",
            Type::Objects,
            "the level beneath: objects whose parent level is these objects (for children.*)",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the objects with their classes and memberships"))
        .output(PortSpec::new("table", Type::Table, "id, class (text), m_<class> and margin"))
        .output(PortSpec::new("map", Type::Map, "the class (1.. in the list) of each cell's object, no value if none"))
        .param(PortSpec::new(
            "classes",
            Type::Any,
            "the hierarchy: [{ \"name\": \"water\", \"rule\": \"above(ndwi, 0, 0.2)\", \"parent\": \"wet\", \"colour\": \
             \"#3366cc\" }, ...] (rule, parent and colour are optional)",
        ))
        .param(PortSpec::optional(
            "min",
            Type::Number,
            "the least membership for a class, 0 to 1 (default 0.1, eCognition's minimum membership value)",
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
        let min: f64 = args.param("min", 0.1)?;
        check_min(node, min)?;
        let given: serde_json::Value =
            args.opt_param("classes")?.ok_or_else(|| Error::msg(format!("node {node:?}: no classes")))?;
        let t = tree(&given).map_err(bad)?;
        let m = memberships(o, below, &t).map_err(bad)?;
        let classes = t.iter().map(|c| ObjectClass { name: c.name.clone(), colour: c.colour }).collect();
        conclude(o, classes, m, &leaves(&t), min).map_err(bad)
    }
}

/// `objects.nearest`.
struct Nearest;

impl Operator for Nearest {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.nearest",
            "Objects classed by their nearest samples, as eCognition's Standard Nearest Neighbour. The samples are the \
             table of objects.samples (class and attributes). The distance of an object to a sample is d = \
             sqrt(sum over the attributes of ((v - s) / sd)^2), each attribute divided by its standard deviation over \
             the objects (the sum is not divided by their number: the more attributes, the farther things are; an \
             attribute missing in either is left out, and so is one that does not vary). The membership of a class is \
             that of its nearest sample, exp(-k d^2) with k = ln(1 / slope): 1 at d = 0 and `slope` at d = 1. The \
             object takes the class of the largest membership, unclassified when it is under `min`.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with the attributes (objects.features)"))
        .input(PortSpec::new("samples", Type::Table, "the table of objects.samples: class and the attributes"))
        .output(PortSpec::new("objects", Type::Objects, "the objects with their classes and memberships"))
        .output(PortSpec::new("table", Type::Table, "id, class (text), m_<class> and margin"))
        .output(PortSpec::new("map", Type::Map, "the class (1..) of each cell's object, no value when unclassified"))
        .param(PortSpec::optional(
            "columns",
            Type::Any,
            "the attributes of the feature space (default: the colours in both tables, the bands' means \
             (mean_<band>), brightness, ndvi, ndwi and ndmi; without any, every numeric attribute but id, class, \
             cells, area and the centroid; a few, chosen well, work better than many)",
        ))
        .param(PortSpec::optional(
            "slope",
            Type::Number,
            "the membership at a distance of 1, between 0 and 1 (default 0.2, eCognition's function slope)",
        ))
        .param(PortSpec::optional(
            "min",
            Type::Number,
            "the least membership for a class, 0 to 1 (default 0.1, eCognition's minimum membership value)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let Some(Value::Table(s)) = args.get("samples") else {
            return Err(bad("connect the samples' table (objects.samples)".into()));
        };
        let min: f64 = args.param("min", 0.1)?;
        check_min(node, min)?;
        let slope: f64 = args.param("slope", 0.2)?;
        if !(slope > 0.0 && slope < 1.0) {
            return Err(bad(format!("slope {slope}: above 0 and below 1")));
        }
        let names = s.labels.get("class").ok_or_else(|| bad("the samples have no text column class".into()))?.clone();
        let codes = s.column("class").ok_or_else(|| bad("the samples have no column class".into()))?;
        let numeric = |t: &Table, c: &String| t.columns.contains(c) && !t.is_text(c);
        let columns: Vec<String> = match args.opt_param::<Vec<String>>("columns")? {
            Some(c) if !c.is_empty() => {
                for name in &c {
                    if !numeric(&o.table, name) {
                        return Err(bad(format!(
                            "no numeric attribute {name:?} in the objects (attributes: {})",
                            o.table.columns.join(", ")
                        )));
                    }
                    if !numeric(s, name) {
                        return Err(bad(format!(
                            "no numeric attribute {name:?} in the samples (columns: {})",
                            s.columns.join(", ")
                        )));
                    }
                }
                c
            }
            _ => {
                // the colours: the bands' means and the indices; the size, the place and the spread of the
                // objects would weigh as much as a colour and blur the classes (11% right on SLIC's objects)
                let common = |c: &&String| *c != "id" && *c != "class" && numeric(&o.table, c) && numeric(s, c);
                let colour = |c: &&String| {
                    c.starts_with("mean_") || ["brightness", "ndvi", "ndwi", "ndmi"].contains(&c.as_str())
                };
                let spectral: Vec<String> = o.table.columns.iter().filter(common).filter(colour).cloned().collect();
                if spectral.is_empty() {
                    const PLACE: [&str; 4] = ["cells", "area", "centroid_x", "centroid_y"];
                    o.table.columns.iter().filter(common).filter(|c| !PLACE.contains(&c.as_str())).cloned().collect()
                } else {
                    spectral
                }
            }
        };
        // (values of the objects, the standard deviation, values of the samples), the attributes that vary
        let mut feats: Vec<(Vec<f64>, f64, Vec<f64>)> = Vec::new();
        for c in &columns {
            let v = o.table.column(c).unwrap_or_default();
            let ok: Vec<f64> = v.iter().copied().filter(|x| x.is_finite()).collect();
            if ok.is_empty() {
                continue;
            }
            let mean = ok.iter().sum::<f64>() / ok.len() as f64;
            let sd = (ok.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / ok.len() as f64).sqrt();
            if sd > 0.0 {
                feats.push((v, sd, s.column(c).unwrap_or_default()));
            }
        }
        if feats.is_empty() {
            return Err(bad("no attribute common to the objects and the samples that varies".into()));
        }
        let per: Vec<(String, usize)> = names
            .iter()
            .enumerate()
            .map(|(c, n)| (n.clone(), codes.iter().filter(|&&x| x == (c + 1) as f64).count()))
            .collect();
        super::warnings::counts(ctx, "objects.nearest", node, &per);
        let used: Vec<Vec<f64>> = columns.iter().map(|c| o.table.column(c).unwrap_or_default()).collect();
        super::warnings::correlated(ctx, "objects.nearest", node, &columns, &used);
        let k = (1.0 / slope).ln();
        let n = o.count;
        let mut membership = vec![vec![0.0f64; n]; names.len()];
        for (j, &code) in codes.iter().enumerate() {
            if !(code >= 1.0 && (code as usize) <= names.len()) {
                continue;
            }
            let class = code as usize - 1;
            for i in 0..n {
                let (mut d2, mut used) = (0.0, 0usize);
                for (v, sd, sv) in &feats {
                    let d = (v[i] - sv[j]) / sd;
                    if d.is_finite() {
                        d2 += d * d;
                        used += 1;
                    }
                }
                if used > 0 {
                    let m = (-k * d2).exp();
                    if m > membership[class][i] {
                        membership[class][i] = m;
                    }
                }
            }
        }
        let classes: Vec<ObjectClass> =
            names.iter().map(|name| ObjectClass { name: name.clone(), colour: None }).collect();
        let all: Vec<usize> = (0..names.len()).collect();
        conclude(o, classes, membership, &all, min).map_err(|e| bad(e.0))
    }
}
