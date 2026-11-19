//! `objects.samples` and `objects.classify`: classes of objects by examples. The samples (points or polygons with a
//! class) pick the objects under them and make a training table; any model of `ml.fit` learns it; the model then
//! gives every object a class, a probability of each class and a margin.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry};

use super::{ObjectClass, objects_in};
use crate::sdm::extrapolation::Frame;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Samples));
    r.add(Arc::new(Classify));
}

/// A text column of codes (1.. in the order of `names`).
fn text_column(t: &mut Table, name: &str, names: Vec<String>) {
    t.labels.insert(name.to_string(), names);
}

/// The first few ids of a list, for a message.
fn some(ids: &[u32]) -> String {
    let shown: Vec<String> = ids.iter().take(5).map(u32::to_string).collect();
    format!("{}{}", shown.join(", "), if ids.len() > 5 { ", ..." } else { "" })
}

/// `objects.samples`.
struct Samples;

impl Operator for Samples {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.samples",
            "Training samples of objects: points or polygons with a class pick the objects under them. A point gives \
             the object of its cell; a polygon gives every object with at least half of its cells inside it (a cell \
             is inside when its centre is). An object picked with two different classes is left out, and so are \
             points outside every object, with a warning. The training table (class, then the numeric attributes of \
             the objects; a row per sampled object, in id order, the `objects` output marks them) goes to ml.fit \
             with column = class (every other column is then a predictor, no id among them). Classes are numbered by their names in alphabetical order, so a model fitted on the table has the codes \
             1.. of that order.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with the attributes (objects.features)"))
        .input(PortSpec::new("samples", Type::Vector, "points or polygons with a class, in any coordinate system"))
        .output(PortSpec::new("table", Type::Table, "class (text) and the numeric attributes, a row per sampled object"))
        .output(PortSpec::new(
            "objects",
            Type::Objects,
            "the objects with the sampled ones classed, the rest unclassified (to see the samples)",
        ))
        .output(PortSpec::new("counts", Type::Table, "class, objects and cells of the training objects"))
        .param(PortSpec::optional(
            "class",
            Type::Text,
            "the samples' attribute holding the class, text or numbers (default class)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let o = objects_in(args, "objects")?;
        let attr: String = args.param("class", "class".to_string())?;
        let v = crate::vector::on_grid(crate::vector::vector_of(args, "samples")?, &o.grid)?;
        let col = v.attributes.iter().find(|(n, _)| *n == attr).map(|(_, c)| c).ok_or_else(|| {
            let have: Vec<&str> = v.attributes.iter().map(|(n, _)| n.as_str()).collect();
            Error::msg(format!("node {node:?}: the samples have no attribute {attr:?} ({})", have.join(", ")))
        })?;
        // the class of each sample as text (none: empty)
        let text: Vec<String> = match col {
            Column::Texts(x) => x.clone(),
            Column::Numbers(x) => x.iter().map(|&f| if f.is_nan() { String::new() } else { format!("{f}") }).collect(),
        };
        let mut names: Vec<String> = text.iter().filter(|s| !s.is_empty()).cloned().collect();
        names.sort();
        names.dedup();
        let code = |s: &str| names.binary_search_by(|n| n.as_str().cmp(s)).ok();

        let g = &o.grid;
        let t = g.transform;
        if t[2] != 0.0 || t[4] != 0.0 {
            return Err(Error::msg(format!("node {node:?}: the objects' grid is rotated")));
        }
        let (w, h) = (g.width as i64, g.height as i64);
        // the objects of each sample: (object index, ) with the class of the sample
        let mut picked: Vec<Vec<usize>> = vec![Vec::new(); o.count];
        let (mut outside, mut classless, mut unfit) = (0usize, 0usize, 0usize);
        // polygons: their cells in one call
        let polys: Vec<usize> = (0..v.len()).filter(|&i| v.geometries[i].is_polygonal()).collect();
        let mut poly_runs = None;
        if !polys.is_empty() {
            let geoms: Vec<&Geometry> = polys.iter().map(|&i| &v.geometries[i]).collect();
            poly_runs =
                Some(crate::vector::runs(&geoms, g, false).map_err(|e| Error::msg(format!("node {node:?}: {e}")))?);
        }
        // the object under a point: 0 none
        let object_at = |xy: [f64; 2]| -> u32 {
            let (c, r) = (((xy[0] - t[0]) / t[1]).floor(), ((xy[1] - t[3]) / t[5]).floor());
            if c >= 0.0 && r >= 0.0 && (c as i64) < w && (r as i64) < h {
                o.labels[r as usize * g.width + c as usize]
            } else {
                0
            }
        };
        for (k, class) in text.iter().enumerate() {
            if code(class).is_none() {
                classless += 1;
                continue;
            }
            match &v.geometries[k] {
                Geometry::Point(_) | Geometry::MultiPoint(_) => {
                    let ps = match &v.geometries[k] {
                        Geometry::Point(p) => vec![*p],
                        Geometry::MultiPoint(ps) => ps.clone(),
                        _ => unreachable!(),
                    };
                    for p in ps {
                        match object_at(p) {
                            0 => outside += 1,
                            id => picked[id as usize - 1].push(k),
                        }
                    }
                }
                Geometry::Empty => {}
                geo if geo.is_polygonal() => {
                    let p = polys.iter().position(|&i| i == k).unwrap();
                    let r = poly_runs.as_ref().unwrap();
                    let mut inside: BTreeMap<u32, u32> = BTreeMap::new();
                    for q in r.offsets[p] as usize..r.offsets[p + 1] as usize {
                        let row = r.y[q] as usize * g.width;
                        for x in r.x0[q]..r.x1[q] {
                            let l = o.labels[row + x as usize];
                            if l != 0 {
                                *inside.entry(l).or_default() += 1;
                            }
                        }
                    }
                    let mut any = false;
                    for (l, n) in inside {
                        if 2 * n >= o.cells[l as usize - 1] {
                            picked[l as usize - 1].push(k);
                            any = true;
                        }
                    }
                    if !any {
                        outside += 1;
                    }
                }
                _ => unfit += 1,
            }
        }
        if outside > 0 {
            ctx.warn(format!(
                "objects.samples: {outside} sample(s) are outside every object (or a polygon holds no object by half): ignored"
            ));
        }
        if classless > 0 {
            ctx.warn(format!("objects.samples: {classless} sample(s) without a class ({attr}): ignored"));
        }
        if unfit > 0 {
            ctx.warn(format!("objects.samples: {unfit} sample(s) are lines: ignored"));
        }
        // the class of each object: none, or the only one its samples agree on
        let mut class_of = vec![0u32; o.count];
        let mut conflict: Vec<u32> = Vec::new();
        for (k, ks) in picked.iter().enumerate() {
            let mut cs: Vec<usize> = ks.iter().filter_map(|&s| code(&text[s])).collect();
            cs.dedup();
            cs.sort_unstable();
            cs.dedup();
            match cs.len() {
                0 => {}
                1 => class_of[k] = cs[0] as u32 + 1,
                _ => conflict.push(k as u32 + 1),
            }
        }
        if !conflict.is_empty() {
            ctx.warn(format!(
                "objects.samples: {} object(s) hold samples of different classes and are left out (ids {})",
                conflict.len(),
                some(&conflict)
            ));
        }
        // the training table
        let numeric: Vec<usize> = (1..o.table.columns.len())
            .filter(|&j| !o.table.is_text(&o.table.columns[j]) && o.table.columns[j] != "class")
            .collect();
        // (no id: ml.fit takes every column but the target as a predictor; the rows are those of the objects marked in
        // the `objects` output, in id order)
        let mut table = Table::new(
            ["class"]
                .into_iter()
                .map(String::from)
                .chain(numeric.iter().map(|&j| o.table.columns[j].clone()))
                .collect(),
        );
        let mut counts = vec![(0.0, 0.0); names.len()];
        for (k, &class) in class_of.iter().enumerate() {
            if class == 0 {
                continue;
            }
            let c = class as usize - 1;
            counts[c].0 += 1.0;
            counts[c].1 += o.cells[k] as f64;
            let mut row = vec![class as f64];
            row.extend(numeric.iter().map(|&j| o.table.rows[k][j]));
            table.rows.push(row);
        }
        text_column(&mut table, "class", names.clone());
        let mut ct = Table::new(["class", "objects", "cells"].map(String::from).to_vec());
        ct.rows = counts.iter().enumerate().map(|(c, n)| vec![(c + 1) as f64, n.0, n.1]).collect();
        text_column(&mut ct, "class", names.clone());
        let classes = names.into_iter().map(|name| ObjectClass { name, colour: None }).collect();
        let marked = o.as_ref().clone().with_classes(classes, class_of, Vec::new())?;
        let mut out = outputs([("table", Value::Table(table)), ("counts", Value::Table(ct))]);
        out.insert("objects".into(), Value::Objects(Arc::new(marked)));
        Ok(out)
    }
}

/// `objects.classify`.
struct Classify;

impl Operator for Classify {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.classify",
            "Objects classed by a fitted model (ml.fit on the table of objects.samples, a forest or a gradient \
             boosting): the model reads each object's attributes, matched by the names of its predictors, and gives \
             the probability of each class; the object takes the class of the largest probability (the first on \
             ties), or stays unclassified when that is under `min`. The probabilities are the objects' memberships; \
             the margin (the largest minus the second largest) tells the objects the model is least sure of. The \
             attributes must be those of the training: run objects.features with the same settings.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with the attributes the model was fitted on"))
        .input(PortSpec::new("model", Type::MlModel, "the model fitted on the samples' table (column class)"))
        .input(PortSpec::optional(
            "samples",
            Type::Table,
            "the table of objects.samples: its class names for the model's codes",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the objects with their classes and memberships"))
        .output(PortSpec::new("table", Type::Table, "id, class (text), the probability of each class and margin"))
        .output(PortSpec::new("map", Type::Map, "the class (1..) of each cell's object, no value when unclassified"))
        .param(PortSpec::optional(
            "min",
            Type::Number,
            "the least probability for a class, 0 to 1 (default 0: every object takes its most probable class)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let o = objects_in(args, "objects")?;
        let Some(Value::MlModel(model)) = args.get("model") else {
            return Err(Error::msg(format!("node {node:?}: connect a fitted model")));
        };
        let min: f64 = args.param("min", 0.0)?;
        if !(0.0..=1.0).contains(&min) {
            return Err(Error::msg(format!("node {node:?}: min {min}: 0 to 1")));
        }
        let codes = model.classes().to_vec();
        if codes.is_empty() {
            return Err(Error::msg(format!("node {node:?}: the model is of values, not classes")));
        }
        let samples = match args.get("samples") {
            None => None,
            Some(Value::Table(t)) => Some(t),
            Some(v) => return Err(Error::msg(format!("node {node:?}: samples: a table, not a {:?}", v.ty()))),
        };
        let names: Vec<String> = codes
            .iter()
            .map(|&c| samples.and_then(|t| t.label("class", c)).map_or_else(|| format!("class {c}"), str::to_string))
            .collect();
        let predictors = model.predictors().to_vec();
        let mut columns = Vec::new();
        for p in &predictors {
            columns.push(o.table.column(p).ok_or_else(|| {
                Error::msg(format!(
                    "node {node:?}: the objects have no attribute {p:?} that the model reads: run objects.features \
                     with the same settings as for the samples (attributes: {})",
                    o.table.columns.join(", ")
                ))
            })?);
        }
        let probs = model
            .outputs(&Frame { names: predictors, columns })
            .map_err(|e| Error::msg(format!("node {node:?}: {e}")))?;
        let n = o.count;
        let mut class_of = vec![0u32; n];
        let mut margin = vec![f64::NAN; n];
        for i in 0..n {
            if probs.iter().any(|p| p[i].is_nan()) {
                continue;
            }
            let (mut a, mut second) = (0usize, f64::NEG_INFINITY);
            for c in 1..probs.len() {
                if probs[c][i] > probs[a][i] {
                    second = probs[a][i].max(second);
                    a = c;
                } else {
                    second = second.max(probs[c][i]);
                }
            }
            let top = probs[a][i];
            margin[i] = top - if second == f64::NEG_INFINITY { 0.0 } else { second };
            if top >= min {
                class_of[i] = a as u32 + 1;
            }
        }
        let membership: Vec<Vec<f64>> =
            probs.iter().map(|p| p.iter().map(|&x| if x.is_nan() { 0.0 } else { x }).collect()).collect();
        let classes: Vec<ObjectClass> =
            names.iter().map(|name| ObjectClass { name: name.clone(), colour: None }).collect();
        let mut table = Table::new(
            ["id", "class"]
                .into_iter()
                .map(String::from)
                .chain(names.iter().map(|c| format!("p_{c}")))
                .chain(["margin".to_string()])
                .collect(),
        );
        for i in 0..n {
            let mut row = vec![(i + 1) as f64, if class_of[i] == 0 { f64::NAN } else { class_of[i] as f64 }];
            row.extend(probs.iter().map(|p| p[i]));
            row.push(margin[i]);
            table.rows.push(row);
        }
        text_column(&mut table, "class", names);
        let per: Vec<f64> = class_of.iter().map(|&k| if k == 0 { f64::NAN } else { k as f64 }).collect();
        let data: Vec<f64> = o.labels.iter().map(|&l| if l == 0 { f64::NAN } else { per[l as usize - 1] }).collect();
        let classed = o.as_ref().clone().with_classes(classes, class_of, membership)?;
        let mut out = outputs([("table", Value::Table(table)), ("map", Value::Map(Map::new(o.grid.clone(), data)))]);
        out.insert("objects".into(), Value::Objects(Arc::new(classed)));
        Ok(out)
    }
}
