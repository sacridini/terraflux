//! `objects.similar`: the objects that look like some examples. Each object takes the similarity of its attributes to
//! those of the nearest example (the best of them), and the most similar, or those above a threshold, are the
//! selection: the way to label many objects of a class from a few, or to find what a click on one object means. The
//! attributes are the embeddings of `objects.embed` (`emb_1`...) when the objects have them, else the objects'
//! numeric attributes standardised (each divided by its standard deviation, after its mean is taken away), so that
//! the area does not outweigh the NDVI. The Viewer lights the same function's result as its threshold moves.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{Objects, objects_in};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Similar));
}

/// How the likeness of two objects is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// The cosine of the angle between the two vectors, -1 to 1 (1: the same direction).
    Cosine,
    /// 1 / (1 + the Euclidean distance), 0 to 1 (1: the same place).
    Euclidean,
}

impl Metric {
    pub fn parse(text: &str) -> std::result::Result<Metric, String> {
        match text {
            "cosine" => Ok(Metric::Cosine),
            "euclidean" => Ok(Metric::Euclidean),
            t => Err(format!("metric: cosine or euclidean, not {t:?}")),
        }
    }
}

/// The similarity of each object to its nearest example, and what it was measured on.
pub struct Found {
    /// A value per object (id 1 first), NaN where an attribute is missing.
    pub similarity: Vec<f64>,
    /// Which attributes, in words: for the note of the node and the Viewer.
    pub used: String,
}

/// The columns the objects are compared on, and whether they are standardised: those named, else the embeddings
/// `emb_*` when there are some, else every numeric attribute but the id, the cells and the area.
fn columns_of(o: &Objects, named: Option<&[String]>) -> std::result::Result<(Vec<String>, bool), String> {
    let t = &o.table;
    if let Some(names) = named {
        if names.is_empty() {
            return Err("columns is empty".into());
        }
        for n in names {
            if t.columns.iter().position(|c| c == n).is_none_or(|_| t.is_text(n)) {
                return Err(format!(
                    "the objects have no numeric attribute {n:?} (attributes: {})",
                    t.columns.join(", ")
                ));
            }
        }
        let emb = names.iter().all(|n| n.starts_with("emb_"));
        return Ok((names.to_vec(), !emb));
    }
    let emb: Vec<String> = t.columns.iter().filter(|c| c.starts_with("emb_") && !t.is_text(c)).cloned().collect();
    if !emb.is_empty() {
        return Ok((emb, false));
    }
    let attributes: Vec<String> = t
        .columns
        .iter()
        .filter(|c| !matches!(c.as_str(), "id" | "cells" | "area" | "class" | "similarity") && !t.is_text(c))
        .cloned()
        .collect();
    if attributes.is_empty() {
        return Err("the objects have no numeric attributes to compare (run objects.features)".into());
    }
    Ok((attributes, true))
}

/// The similarity of every object of `o` to the examples (ids, 1 to count), on `columns` (none: see the module's
/// text) by `metric`. The examples themselves are 1.
pub fn similarity(
    o: &Objects,
    examples: &[u32],
    columns: Option<&[String]>,
    metric: Metric,
) -> std::result::Result<Found, String> {
    if examples.is_empty() {
        return Err("no examples: give ids, or sample objects first".into());
    }
    if let Some(&bad) = examples.iter().find(|&&e| e == 0 || e as usize > o.count) {
        return Err(format!("example {bad} is no object's (the objects are 1 to {})", o.count));
    }
    let (names, standardise) = columns_of(o, columns)?;
    let mut cols: Vec<Vec<f64>> = names.iter().filter_map(|n| o.table.column(n)).collect();
    if standardise {
        // (a column of one value says nothing: left out)
        let mut kept = Vec::new();
        let mut kept_names = Vec::new();
        for (c, name) in cols.into_iter().zip(&names) {
            let v: Vec<f64> = c.iter().copied().filter(|x| x.is_finite()).collect();
            if v.is_empty() {
                continue;
            }
            let mean = v.iter().sum::<f64>() / v.len() as f64;
            let sd = (v.iter().map(|x| (x - mean) * (x - mean)).sum::<f64>() / v.len() as f64).sqrt();
            if sd > 0.0 {
                kept.push(c.iter().map(|x| (x - mean) / sd).collect());
                kept_names.push(name.clone());
            }
        }
        if kept.is_empty() {
            return Err("every attribute has one value for all the objects: nothing tells them apart".into());
        }
        cols = kept;
        let shown: Vec<&str> = kept_names.iter().take(6).map(String::as_str).collect();
        let more = if kept_names.len() > 6 { format!(" and {} more", kept_names.len() - 6) } else { String::new() };
        return finish(o, &cols, examples, metric, format!("the standardised attributes {}{more}", shown.join(", ")));
    }
    let shown: Vec<&str> = names.iter().take(3).map(String::as_str).collect();
    let more = if names.len() > 3 { format!(" ... {}", names[names.len() - 1]) } else { String::new() };
    finish(o, &cols, examples, metric, format!("the columns {}{more} as they are", shown.join(", ")))
}

fn finish(
    o: &Objects,
    cols: &[Vec<f64>],
    examples: &[u32],
    metric: Metric,
    used: String,
) -> std::result::Result<Found, String> {
    let n = o.count;
    let row = |i: usize| -> Vec<f64> { cols.iter().map(|c| c[i]).collect() };
    let rows: Vec<Vec<f64>> = (0..n).map(row).collect();
    let norm = |v: &[f64]| v.iter().map(|x| x * x).sum::<f64>().sqrt();
    let ex: Vec<&Vec<f64>> =
        examples.iter().map(|&e| &rows[e as usize - 1]).filter(|v| v.iter().all(|x| x.is_finite())).collect();
    if ex.is_empty() {
        return Err("the examples have a missing attribute: nothing to compare with".into());
    }
    let similarity = rows
        .iter()
        .map(|v| {
            if v.iter().any(|x| !x.is_finite()) {
                return f64::NAN;
            }
            ex.iter()
                .map(|e| match metric {
                    Metric::Cosine => {
                        let (a, b) = (norm(v), norm(e));
                        if a == 0.0 || b == 0.0 {
                            0.0
                        } else {
                            v.iter().zip(e.iter()).map(|(x, y)| x * y).sum::<f64>() / (a * b)
                        }
                    }
                    Metric::Euclidean => {
                        1.0 / (1.0 + v.iter().zip(e.iter()).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt())
                    }
                })
                .fold(f64::NEG_INFINITY, f64::max)
        })
        .collect();
    Ok(Found { similarity, used })
}

/// The objects (not the examples) with a similarity of at least `threshold`, the most similar first.
pub fn above(similarity: &[f64], examples: &[u32], threshold: f64) -> Vec<u32> {
    let mut ids: Vec<u32> = (1..=similarity.len() as u32)
        .filter(|id| !examples.contains(id) && similarity[*id as usize - 1] >= threshold)
        .collect();
    ids.sort_by(|a, b| similarity[*b as usize - 1].total_cmp(&similarity[*a as usize - 1]).then(a.cmp(b)));
    ids
}

/// The `n` most similar objects (not the examples), the most similar first.
pub fn top(similarity: &[f64], examples: &[u32], n: usize) -> Vec<u32> {
    let mut ids = above(similarity, examples, f64::NEG_INFINITY);
    ids.truncate(n);
    ids
}

/// A column of a table, a value per row: replaced when the table has it, else added at the end.
fn set_column(t: &mut Table, name: &str, values: &[f64]) {
    match t.columns.iter().position(|c| c == name) {
        Some(j) => t.rows.iter_mut().zip(values).for_each(|(r, &v)| r[j] = v),
        None => {
            t.columns.push(name.to_string());
            t.rows.iter_mut().zip(values).for_each(|(r, &v)| r.push(v));
        }
    }
}

struct Similar;

impl Operator for Similar {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.similar",
            "The objects that look like some examples: every object takes the similarity of its attributes to those \
             of the nearest example (the best of them), in a column `similarity`, and the most similar (`top`) or \
             those above a `threshold` are the selection. The examples are the ids in `examples`, or the rows of \
             `samples` (a table with a column id), or, with neither, the objects that have a class (objects.samples' \
             `objects`). The attributes are `columns`, else the embeddings of objects.embed (`emb_*`) when there are \
             some, else the numeric attributes standardised (the output `used` says which). `metric` cosine (-1 to 1) \
             or euclidean (1 / (1 + distance)). To label many objects of a class from a few: select them, and the \
             Viewer lights the same result as the threshold moves.",
            Access::Global,
        )
        .input(PortSpec::new(
            "objects",
            Type::Objects,
            "the objects, with their attributes (objects.features, objects.embed)",
        ))
        .input(PortSpec::optional("samples", Type::Table, "a table with a column id: the examples"))
        .output(PortSpec::new(
            "objects",
            Type::Objects,
            "the same objects with the column similarity in their attributes",
        ))
        .output(PortSpec::new(
            "selection",
            Type::Table,
            "id and similarity of the objects selected, the most similar first",
        ))
        .output(PortSpec::new("used", Type::Text, "the attributes the objects were compared on"))
        .param(PortSpec::optional("examples", Type::Any, "the ids of the examples, a list"))
        .param(PortSpec::optional("columns", Type::Any, "the attributes to compare on, a list of names"))
        .param(
            PortSpec::optional("metric", Type::Text, "cosine (default) or euclidean").choices(["cosine", "euclidean"]),
        )
        .param(PortSpec::optional(
            "top",
            Type::Integer,
            "the selection: this many of the most similar (default 20 without a threshold)",
        ))
        .param(PortSpec::optional(
            "threshold",
            Type::Number,
            "the selection: the objects with at least this similarity",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let fail = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let mut examples: Vec<u32> = match args.opt_param::<Vec<f64>>("examples")? {
            Some(v) => v.iter().map(|&x| x as u32).collect(),
            None => Vec::new(),
        };
        match args.get("samples") {
            None => {}
            Some(Value::Table(t)) => {
                let ids = t.column("id").ok_or_else(|| fail("samples: the table has no column id".into()))?;
                examples.extend(ids.iter().filter(|x| x.is_finite()).map(|&x| x as u32));
            }
            Some(v) => return Err(fail(format!("samples: a table, not a {:?}", v.ty()))),
        }
        if examples.is_empty() {
            examples =
                (1..=o.count as u32).filter(|&i| o.class_of.get(i as usize - 1).is_some_and(|&c| c != 0)).collect();
        }
        examples.sort_unstable();
        examples.dedup();
        let columns: Option<Vec<String>> = args.opt_param("columns")?;
        let metric = Metric::parse(&args.param("metric", "cosine".to_string())?).map_err(fail)?;
        let (top_n, threshold): (Option<i64>, Option<f64>) = (args.opt_param("top")?, args.opt_param("threshold")?);
        if top_n.is_some() && threshold.is_some() {
            return Err(fail("top or threshold, not both".into()));
        }
        if top_n.is_some_and(|n| n < 1) {
            return Err(fail("top: 1 or more".into()));
        }
        let found = similarity(o, &examples, columns.as_deref(), metric).map_err(fail)?;
        let chosen = match threshold {
            Some(t) => above(&found.similarity, &examples, t),
            None => top(&found.similarity, &examples, top_n.unwrap_or(20) as usize),
        };
        if chosen.is_empty() {
            ctx.warn(format!("node {node:?}: no object is similar enough to the {} example(s)", examples.len()));
        }
        let mut sel = Table::new(vec!["id".into(), "similarity".into()]);
        sel.rows = chosen.iter().map(|&id| vec![f64::from(id), found.similarity[id as usize - 1]]).collect();
        let mut marked = o.as_ref().clone();
        set_column(&mut marked.table, "similarity", &found.similarity);
        let mut out = outputs([("selection", Value::Table(sel)), ("used", Value::Text(found.used.clone()))]);
        out.insert("objects".into(), Value::Objects(Arc::new(marked)));
        Ok(out)
    }
}
