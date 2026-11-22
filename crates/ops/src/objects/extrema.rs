//! `objects.extrema`: the local extrema of an attribute among neighbouring objects, as eCognition's *find local
//! extrema*. An object is a maximum (or a minimum) when no other candidate near it has a larger (smaller) value; near
//! is within a radius between centroids (a grid of buckets as wide as the radius, so each object looks at nine buckets)
//! or within some steps across the objects' neighbours (a breadth-first search). With `strict` (eCognition's *exclude
//! equal*) a tie is no extremum. The objects are independent, so they run in parallel: the same result on any number of
//! threads.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::Column;

use super::calc::evaluate;
use super::levels::class_numbers;
use super::{ObjectClass, Objects, objects_in};
use crate::algebra::parse_script_objects;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Extrema));
}

/// How near an object must be to be compared.
enum Near {
    /// Within this distance between centroids, in map units.
    Radius(f64),
    /// Within this many steps across neighbouring objects.
    Steps(usize),
}

/// The centroid of each object, in map units from the grid's corner (only differences are used).
pub(super) fn centroids(o: &Objects) -> Vec<(f64, f64)> {
    let w = o.grid.width;
    let (dx, dy) = (o.grid.transform[1].abs(), o.grid.transform[5].abs());
    let mut sum = vec![(0.0f64, 0.0f64); o.count];
    for (i, &l) in o.labels.iter().enumerate() {
        if l != 0 {
            let s = &mut sum[l as usize - 1];
            s.0 += (i % w) as f64;
            s.1 += (i / w) as f64;
        }
    }
    sum.iter().zip(&o.cells).map(|(s, &n)| (s.0 / n.max(1) as f64 * dx, s.1 / n.max(1) as f64 * dy)).collect()
}

/// What the search reads.
struct Field<'a> {
    o: &'a Objects,
    near: Near,
    mid: Vec<(f64, f64)>,
    buckets: HashMap<(i64, i64), Vec<u32>>,
    v: &'a [f64],
    cand: &'a [bool],
    max: bool,
    strict: bool,
}

impl Field<'_> {
    /// Whether object `j` (0-based) has a value that stops `i` from being an extremum.
    fn beats(&self, i: usize, j: usize) -> bool {
        let (v, c) = (self.v, self.cand);
        if j == i || !c[j] || v[j].is_nan() {
            return false;
        }
        match (self.max, self.strict) {
            (true, true) => v[i] <= v[j],
            (true, false) => v[i] < v[j],
            (false, true) => v[i] >= v[j],
            (false, false) => v[i] > v[j],
        }
    }

    /// Whether the object `i` (0-based) is an extremum among the candidates near it.
    fn is_extremum(&self, i: usize) -> bool {
        if !self.cand[i] || self.v[i].is_nan() {
            return false;
        }
        match self.near {
            Near::Radius(r) => {
                let mid = &self.mid;
                let (bx, by) = ((mid[i].0 / r).floor() as i64, (mid[i].1 / r).floor() as i64);
                let lim = r * (1.0 + 1e-9);
                for kx in bx - 1..=bx + 1 {
                    for ky in by - 1..=by + 1 {
                        for &j in self.buckets.get(&(kx, ky)).map_or(&[][..], |b| b.as_slice()) {
                            let j = j as usize;
                            let d = ((mid[i].0 - mid[j].0).powi(2) + (mid[i].1 - mid[j].1).powi(2)).sqrt();
                            if d <= lim && self.beats(i, j) {
                                return false;
                            }
                        }
                    }
                }
                true
            }
            Near::Steps(n) => {
                let mut seen: HashSet<u32> = HashSet::new();
                seen.insert(i as u32 + 1);
                let mut front = vec![i as u32 + 1];
                for _ in 0..n {
                    let mut next = Vec::new();
                    for &id in &front {
                        for &(nb, _) in self.o.neighbours_of(id) {
                            if seen.insert(nb) {
                                if self.beats(i, nb as usize - 1) {
                                    return false;
                                }
                                next.push(nb);
                            }
                        }
                    }
                    if next.is_empty() {
                        break;
                    }
                    front = next;
                }
                true
            }
        }
    }
}

/// `objects.extrema`.
struct Extrema;

impl Operator for Extrema {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.extrema",
            "The local extrema of an attribute among neighbouring objects, as eCognition's find local extrema: an \
             object is a maximum (kind max) or a minimum (kind min) when no other object near it has a larger or \
             smaller value of the attribute (a column of the table, or an expression as objects.calc's: mean_nir, \
             nb.mean(ndvi)). Near is within radius map units between centroids, or within steps objects across the \
             neighbours (default 1 step: the adjacent objects). among limits the classes compared and marked (default \
             all); the others are neither. strict (the default; eCognition's exclude equal) makes a tie no extremum, \
             so two equal neighbours are both left out; with strict off, an object tied with the highest is one too. \
             An object whose value is missing is none and is not compared. The extrema get the class given by class \
             (default maximum or minimum, made when the objects have none of the name), the others keep theirs. The \
             table has the column extremum: 1 for an extremum, 0 for a candidate that is not, missing for an object \
             not compared. The same result on any number of threads.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with their attributes (objects.features)"))
        .output(PortSpec::new("objects", Type::Objects, "the objects, the extrema with the class"))
        .output(PortSpec::new("table", Type::Table, "the attributes and the column extremum"))
        .param(PortSpec::new(
            "attribute",
            Type::Text,
            "a column of the table (mean_nir) or an expression as objects.calc's (nb.mean(ndvi))",
        ))
        .param(PortSpec::optional("kind", Type::Text, "max (the default) or min").choices(["max", "min"]))
        .param(PortSpec::optional(
            "radius",
            Type::Number,
            "the distance between centroids, in map units, within which objects are compared",
        ))
        .param(PortSpec::optional(
            "steps",
            Type::Integer,
            "how many objects across the neighbours are compared (default 1)",
        ))
        .param(PortSpec::optional(
            "among",
            Type::Any,
            "the class names of the objects compared and marked (default: all)",
        ))
        .param(PortSpec::optional("class", Type::Text, "the class given to the extrema (default maximum or minimum)"))
        .param(PortSpec::optional(
            "strict",
            Type::Bool,
            "a tie is no extremum (the default); false lets an object tied with the highest be one",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let text: String = args.opt_param("attribute")?.ok_or_else(|| bad("no attribute".into()))?;
        let max = match args.param("kind", "max".to_string())?.as_str() {
            "max" => true,
            "min" => false,
            other => return Err(bad(format!("kind {other:?}: max or min"))),
        };
        let strict: bool = args.param("strict", true)?;
        let radius: Option<f64> = args.opt_param("radius")?;
        let steps: Option<i64> = args.opt_param("steps")?;
        let near = match (radius, steps) {
            (Some(_), Some(_)) => return Err(bad("give radius or steps, not both".into())),
            (Some(r), None) => {
                if !r.is_finite() || r <= 0.0 {
                    return Err(bad(format!("radius {r}: above 0")));
                }
                Near::Radius(r)
            }
            (None, Some(s)) => {
                if s < 1 {
                    return Err(bad(format!("steps {s}: 1 or more")));
                }
                Near::Steps(s as usize)
            }
            (None, None) => Near::Steps(1),
        };
        let name: String = args.param("class", if max { "maximum" } else { "minimum" }.to_string())?;
        if name.is_empty() {
            return Err(bad("class: a name".into()));
        }
        // the attribute: an expression over the table's columns, evaluated as objects.calc does
        let script = parse_script_objects(&text).map_err(|e| bad(format!("the attribute {text:?}: {}", e.0)))?;
        let mut given = evaluate(o, None, &script).map_err(|e| bad(format!("the attribute {text:?}: {}", e.0)))?;
        if given.is_empty() {
            return Err(bad(format!("the attribute {text:?} gives no value")));
        }
        let v = match given.remove(0).1 {
            Column::Numbers(x) => x,
            Column::Texts(_) => return Err(bad(format!("the attribute {text:?} is a text: a number is needed"))),
        };
        let cand: Vec<bool> = match args.opt_param::<Vec<String>>("among")? {
            Some(names) if !names.is_empty() => {
                let set = class_numbers(o, &names).map_err(|e| bad(e.0))?;
                o.class_of.iter().map(|k| set.contains(k)).collect()
            }
            _ => vec![true; o.count],
        };
        if v.iter().zip(&cand).all(|(x, &c)| !c || x.is_nan()) && o.count > 0 {
            ctx.warn(format!(
                "node {node:?}: the attribute {text:?} has no value in the objects compared: no extremum"
            ));
        }
        let mid = if matches!(near, Near::Radius(_)) { centroids(o) } else { Vec::new() };
        let mut buckets: HashMap<(i64, i64), Vec<u32>> = HashMap::new();
        if let Near::Radius(r) = near {
            for (i, m) in mid.iter().enumerate() {
                if cand[i] && !v[i].is_nan() {
                    buckets.entry(((m.0 / r).floor() as i64, (m.1 / r).floor() as i64)).or_default().push(i as u32);
                }
            }
        }
        let field = Field { o, near, mid, buckets, v: &v, cand: &cand, max, strict };
        let hit: Vec<bool> = (0..o.count).into_par_iter().map(|i| field.is_extremum(i)).collect();
        // the class of the extrema: the one of the name, made when there is none
        let mut classes = o.classes.clone();
        let k = match classes.iter().position(|c| c.name == name) {
            Some(k) => k,
            None => {
                classes.push(ObjectClass { name: name.clone(), colour: None });
                classes.len() - 1
            }
        };
        let mut membership = o.membership.clone();
        if !membership.is_empty() && membership.len() < classes.len() {
            membership.push(hit.iter().map(|&h| if h { 1.0 } else { 0.0 }).collect());
        }
        let class_of: Vec<u32> = o.class_of.iter().zip(&hit).map(|(&c, &h)| if h { k as u32 + 1 } else { c }).collect();
        // the table: the column extremum (one of the name replaced)
        let mut table = o.table.clone();
        let col = match table.columns.iter().position(|c| c == "extremum") {
            Some(j) => j,
            None => {
                table.columns.push("extremum".into());
                table.rows.iter_mut().for_each(|r| r.push(f64::NAN));
                table.columns.len() - 1
            }
        };
        table.labels.remove("extremum");
        for (i, r) in table.rows.iter_mut().enumerate() {
            r[col] = if !cand[i] || v[i].is_nan() {
                f64::NAN
            } else if hit[i] {
                1.0
            } else {
                0.0
            };
        }
        let new = o
            .as_ref()
            .clone()
            .with_classes(classes, class_of, membership)
            .and_then(|n| n.with_table(table.clone()))
            .map_err(|e| bad(e.0))?;
        let mut out: Outputs = outputs([("table", Value::Table(table))]);
        out.insert("objects".into(), Value::Objects(Arc::new(new)));
        Ok(out)
    }
}
