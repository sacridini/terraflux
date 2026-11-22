//! `objects.join`: columns of a table (the principal components of the objects' attributes, a model's scores, a
//! survey's values by object) joined to the objects' own attributes, matched by `id`, so that everything that reads
//! the attributes (objects.samples, objects.classify, objects.rules, the inspector) sees them.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{Objects, objects_in};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Join));
}

/// Up to a few ids, for a message.
fn some(ids: &[i64]) -> String {
    let shown: Vec<String> = ids.iter().take(8).map(i64::to_string).collect();
    let more = if ids.len() > 8 { format!(" and {} more", ids.len() - 8) } else { String::new() };
    format!("{}{more}", shown.join(", "))
}

/// The row of `t` of each object (by the id of the objects' table), or the reason the ids do not match.
fn rows_of(o: &Objects, t: &Table) -> std::result::Result<Vec<usize>, String> {
    let ids = t.column("id").ok_or("the table has no column id")?;
    let mut at: Vec<Option<usize>> = vec![None; o.count + 1];
    let (mut unknown, mut twice, mut none) = (Vec::new(), Vec::new(), 0usize);
    for (r, &v) in ids.iter().enumerate() {
        if v.is_nan() || v.fract() != 0.0 {
            none += 1;
        } else if v < 1.0 || v > o.count as f64 {
            unknown.push(v as i64);
        } else if at[v as usize].replace(r).is_some() {
            twice.push(v as i64);
        }
    }
    let mut problems = Vec::new();
    if none > 0 {
        problems.push(format!("{none} rows have no whole id"));
    }
    if !unknown.is_empty() {
        problems.push(format!("ids that are no object's (the objects are 1 to {}): {}", o.count, some(&unknown)));
    }
    if !twice.is_empty() {
        problems.push(format!("ids twice: {}", some(&twice)));
    }
    let own = o.table.column("id").ok_or("the objects' table has no column id")?;
    let mut missing = Vec::new();
    let mut rows = Vec::with_capacity(o.count);
    for &id in &own {
        match at.get(id as usize).copied().flatten() {
            Some(r) => rows.push(r),
            None => missing.push(id as i64),
        }
    }
    if !missing.is_empty() {
        problems.push(format!("objects with no row: {}", some(&missing)));
    }
    if problems.is_empty() { Ok(rows) } else { Err(problems.join("; ")) }
}

/// `objects.join`.
struct Join;

impl Operator for Join {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.join",
            "Columns of a table joined to the objects' attributes, matched by id: a row of the table for each object \
             (the principal components of ml.pca's table, the scores of ml.predict, values counted by another \
             operator), in any order. Every operator that reads the objects' attributes then sees them: \
             objects.samples gives them in its training table, objects.classify reads them as the model's \
             predictors. A table with an id that is no object's, an id twice, or no row for an object is refused, and \
             the message names the ids.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects, with their attributes"))
        .input(PortSpec::new("table", Type::Table, "a row per object, with its id (column id) and the values to join"))
        .output(PortSpec::new("objects", Type::Objects, "the objects with the columns added to their attributes"))
        .output(PortSpec::new("table", Type::Table, "the objects' table with the columns"))
        .param(PortSpec::optional(
            "columns",
            Type::Any,
            "the columns to join (default: every numeric column of the table, but id, that the objects' table lacks)",
        ))
        .param(PortSpec::optional(
            "replace",
            Type::Bool,
            "replace a column the objects already have (default false: it is refused)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let t = match args.get("table") {
            Some(Value::Table(t)) => t,
            Some(v) => return Err(bad(format!("table: a table, not a {:?}", v.ty()))),
            None => return Err(bad("connect a table".into())),
        };
        let replace: bool = args.param("replace", false)?;
        let names: Vec<String> = match args.opt_param::<Vec<String>>("columns")? {
            Some(c) => c,
            None => t
                .columns
                .iter()
                .filter(|c| *c != "id" && !t.is_text(c) && (replace || !o.table.columns.contains(c)))
                .cloned()
                .collect(),
        };
        if names.is_empty() {
            return Err(bad("no column to join: the table has none that the objects lack (columns names them)".into()));
        }
        if let Some(i) = (1..names.len()).find(|&i| names[..i].contains(&names[i])) {
            return Err(bad(format!("the column {:?} is twice", names[i])));
        }
        let mut from = Vec::new();
        for n in &names {
            if n == "id" {
                return Err(bad("id is the key, not a column to join".into()));
            }
            let j =
                t.columns.iter().position(|c| c == n).ok_or_else(|| {
                    bad(format!("the table has no column {n:?} (its columns: {})", t.columns.join(", ")))
                })?;
            if !replace && o.table.columns.contains(n) {
                return Err(bad(format!(
                    "the objects already have a column {n:?}: rename it in the table, or set replace to true"
                )));
            }
            from.push(j);
        }
        let rows = rows_of(o, t).map_err(|e| bad(format!("the table's ids do not match the objects': {e}")))?;
        let mut out = o.table.clone();
        for (n, &j) in names.iter().zip(&from) {
            let v: Vec<f64> = rows.iter().map(|&r| t.rows[r][j]).collect();
            let k = match out.columns.iter().position(|c| c == n) {
                Some(k) => k,
                None => {
                    out.columns.push(n.clone());
                    for row in out.rows.iter_mut() {
                        row.push(f64::NAN);
                    }
                    out.columns.len() - 1
                }
            };
            for (row, x) in out.rows.iter_mut().zip(v) {
                row[k] = x;
            }
            match t.labels.get(n) {
                Some(l) => {
                    out.labels.insert(n.clone(), l.clone());
                }
                None => {
                    out.labels.remove(n);
                }
            }
        }
        let joined = o.as_ref().clone().with_table(out.clone()).map_err(|e| bad(e.0))?;
        let mut res = outputs([("table", Value::Table(out))]);
        res.insert("objects".into(), Value::Objects(Arc::new(joined)));
        Ok(res)
    }
}
