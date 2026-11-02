//! Checks of their own, besides `check.assert`'s expression: the values within a range, no cells without a value
//! where there should be one, an area kept from a map to another, a map's accuracy above a minimum, the demand of a
//! step met. None changes the data: each says whether it passed (`ok`), and a failure stops the run with its
//! reason (or only warns, with `warn`), as `check.assert` does; the application shows each as a badge.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::values::number_text;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Range));
    r.add(Arc::new(NoData));
    r.add(Arc::new(AreaKept));
    r.add(Arc::new(Accuracy));
    r.add(Arc::new(DemandMet));
}

/// Whether an operator is a check (its node gets a badge of how it fared).
pub fn is_check(op: &str) -> bool {
    op.starts_with("check.")
}

/// What a check of its own says when it fails and no message is given (none: not one of these).
pub fn default_message(op: &str) -> Option<&'static str> {
    Some(match op {
        "check.range" => "values are out of the range",
        "check.no_nodata" => "cells have no value",
        "check.area_kept" => "the area was not kept",
        "check.accuracy" => "the map is not accurate enough",
        "check.demand" => "the demand was not met",
        _ => return None,
    })
}

/// The settings every check has: what a failure means, and whether it only warns.
fn common(sig: OpSignature, default_message: &str) -> OpSignature {
    sig.output(PortSpec::new("ok", Type::Bool, "whether it passed (with warn; else it passed)"))
        .param(PortSpec::optional("message", Type::Text, &format!("what it means when it fails ({default_message})")))
        .param(PortSpec::optional("warn", Type::Bool, "only warn when it fails, the run going on (false)"))
}

/// A check's end: passed, or failed with its message and what failed (`detail`), stopping the run (or warning).
fn verdict(ctx: &mut Ctx, args: &Args, passed: bool, default_message: &str, detail: &str) -> Result<Outputs> {
    if !passed {
        let message: String = args.param("message", default_message.to_string())?;
        let text = format!("{message} ({detail})");
        if !args.param("warn", false)? {
            return Err(Error::msg(format!("node {:?}: {text}", args.node)));
        }
        ctx.warn(text);
    }
    let mut out = Outputs::new();
    out.insert("ok".into(), Value::Bool(passed));
    Ok(out)
}

/// The values of an input checked cell by cell: a map's, every layer's of a stack, a number.
fn values_of<'a>(args: &'a Args, port: &str) -> Result<Vec<&'a [f64]>> {
    match args.get(port) {
        Some(Value::Map(m)) => Ok(vec![m.data.as_slice()]),
        Some(Value::Stack(s)) => Ok(s.layers.iter().map(|l| l.as_slice()).collect()),
        Some(v) if v.as_number().is_some() => Ok(Vec::new()),
        Some(v) => Err(Error::msg(format!("node {:?}: {port:?} is a {:?}, not a map or a number", args.node, v.ty()))),
        None => Err(Error::msg(format!("node {:?}: no input {port:?}", args.node))),
    }
}

/// Two maps of the same cells (else an error naming them).
fn same_cells(args: &Args, a: (&str, &Map), b: (&str, &Map)) -> Result<()> {
    if a.1.data.len() != b.1.data.len() {
        return Err(Error::msg(format!(
            "node {:?}: {:?} and {:?} differ in size ({} and {} cells): put them on one grid first",
            args.node,
            a.0,
            b.0,
            a.1.data.len(),
            b.1.data.len()
        )));
    }
    Ok(())
}

/// The cells of each class of a map (those with a value).
fn class_cells(m: &Map) -> BTreeMap<i64, usize> {
    let mut out = BTreeMap::new();
    for x in m.data.iter().filter(|x| !x.is_nan()) {
        *out.entry(x.round() as i64).or_insert(0) += 1;
    }
    out
}

/// Every value within a range.
struct Range;

impl Operator for Range {
    fn signature(&self) -> OpSignature {
        common(
            OpSignature::new(
                "check.range",
                "A check that every value is within a range: each cell with a value of a map (every layer of a \
                 stack), or a number, between min and max (both included); the probabilities between 0 and 1, the \
                 heights above 0.",
                Access::Global,
            )
            .input(PortSpec::new("value", Type::Any, "the map, stack or number checked"))
            .param(PortSpec::optional("min", Type::Number, "the lowest value allowed (no lower bound)"))
            .param(PortSpec::optional("max", Type::Number, "the highest value allowed (no upper bound)")),
            "a value is out of the range",
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let min: Option<f64> = args.opt_param("min")?;
        let max: Option<f64> = args.opt_param("max")?;
        if min.is_none() && max.is_none() {
            return Err(Error::msg(format!("node {:?}: give min, max or both", args.node)));
        }
        let lo = min.unwrap_or(f64::NEG_INFINITY);
        let hi = max.unwrap_or(f64::INFINITY);
        let range = match (min, max) {
            (Some(a), Some(b)) => format!("between {} and {}", number_text(a), number_text(b)),
            (Some(a), None) => format!("at least {}", number_text(a)),
            _ => format!("at most {}", number_text(hi)),
        };
        let layers = values_of(args, "value")?;
        if layers.is_empty() {
            let x = args.number("value")?;
            let ok = x >= lo && x <= hi;
            return verdict(
                ctx,
                args,
                ok,
                "values are out of the range",
                &format!("{} is not {range}", number_text(x)),
            );
        }
        let (mut out, mut known, mut low, mut high) = (0usize, 0usize, f64::INFINITY, f64::NEG_INFINITY);
        for d in layers {
            for &x in d.iter().filter(|x| !x.is_nan()) {
                known += 1;
                if x < lo || x > hi {
                    out += 1;
                    low = low.min(x);
                    high = high.max(x);
                }
            }
        }
        let detail = if out == 0 {
            format!("every one of {known} cells {range}")
        } else {
            format!("{out} of {known} cells not {range}, from {} to {}", number_text(low), number_text(high))
        };
        verdict(ctx, args, out == 0, "values are out of the range", &detail)
    }
}

/// No cells without a value where there should be one.
struct NoData;

impl Operator for NoData {
    fn signature(&self) -> OpSignature {
        common(
            OpSignature::new(
                "check.no_nodata",
                "A check that a map has a value where it should: no cell without a value (NaN) in the map (every \
                 layer of a stack), or only within the cells of `within` that have one (the study area), up to a \
                 share of the cells allowed.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Any, "the map or stack checked"))
            .input(PortSpec::optional("within", Type::Map, "where a value is expected: its cells with a value (all)"))
            .param(PortSpec::optional(
                "max_share",
                Type::Number,
                "the share of the cells allowed without a value, 0 to 1 (0)",
            )),
            "cells have no value",
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let layers = values_of(args, "map")?;
        if layers.is_empty() {
            return Err(Error::msg(format!("node {:?}: \"map\" must be a map or a stack", args.node)));
        }
        let within = args.opt_map("within")?;
        let max_share: f64 = args.param("max_share", 0.0)?;
        let (mut missing, mut expected) = (0usize, 0usize);
        for d in layers {
            if let Some(w) = within
                && w.data.len() != d.len()
            {
                return Err(Error::msg(format!(
                    "node {:?}: \"map\" and \"within\" differ in size: put them on one grid first",
                    args.node
                )));
            }
            for (i, x) in d.iter().enumerate() {
                if within.is_some_and(|w| w.data[i].is_nan()) {
                    continue;
                }
                expected += 1;
                missing += usize::from(x.is_nan());
            }
        }
        let share = if expected == 0 { 0.0 } else { missing as f64 / expected as f64 };
        let detail = format!("{missing} of {expected} cells without a value ({:.4} of them)", share);
        verdict(ctx, args, share <= max_share, "cells have no value", &detail)
    }
}

/// An area kept from a map to another.
struct AreaKept;

impl Operator for AreaKept {
    fn signature(&self) -> OpSignature {
        common(
            OpSignature::new(
                "check.area_kept",
                "A check that an area is kept from a map to another (a step to the next, the start to the end): the \
                 cells of a class (or, without one, the cells with a value) the same in both, up to a tolerance; \
                 with `within` (a protected area), every cell of it with a value keeps its class.",
                Access::Global,
            )
            .input(PortSpec::new("before", Type::Map, "the map before"))
            .input(PortSpec::new("after", Type::Map, "the map after"))
            .input(PortSpec::optional("within", Type::Map, "an area where no cell may change: its cells with a value"))
            .param(PortSpec::optional("class", Type::Number, "the class whose area is kept (the cells with a value)"))
            .param(PortSpec::optional("tolerance", Type::Number, "the cells the area may differ by (0)")),
            "the area was not kept",
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (a, b) = (args.map("before")?, args.map("after")?);
        same_cells(args, ("before", a), ("after", b))?;
        let class: Option<f64> = args.opt_param("class")?;
        let tolerance: f64 = args.param("tolerance", 0.0)?;
        let count = |m: &Map| match class {
            Some(c) => m.data.iter().filter(|x| **x == c).count(),
            None => m.data.iter().filter(|x| !x.is_nan()).count(),
        };
        let (n0, n1) = (count(a), count(b));
        let what = match class {
            Some(c) => format!("class {}", number_text(c)),
            None => "cells with a value".to_string(),
        };
        let mut ok = (n1 as f64 - n0 as f64).abs() <= tolerance;
        let mut detail = format!("{what}: {n0} cells before, {n1} after");
        if let Some(w) = args.opt_map("within")? {
            same_cells(args, ("before", a), ("within", w))?;
            let changed = (0..a.data.len())
                .filter(|&i| {
                    !w.data[i].is_nan() && !(a.data[i] == b.data[i] || a.data[i].is_nan() && b.data[i].is_nan())
                })
                .count();
            if changed > 0 {
                ok = false;
            }
            detail.push_str(&format!("; {changed} cells changed within the area kept"));
        }
        verdict(ctx, args, ok, "the area was not kept", &detail)
    }
}

/// A map's accuracy against a reference above a minimum.
struct Accuracy;

impl Operator for Accuracy {
    fn signature(&self) -> OpSignature {
        common(
            OpSignature::new(
                "check.accuracy",
                "A check that a map of classes agrees enough with a reference: the overall agreement (the share of \
                 the cells with a value in both of the same class) or Cohen's kappa at least a minimum.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map checked (a simulated or classified map)"))
            .input(PortSpec::new("reference", Type::Map, "the reference (observed) map"))
            .param(PortSpec::new("min", Type::Number, "the lowest agreement allowed, 0 to 1"))
            .param(
                PortSpec::optional("measure", Type::Text, "overall (default) or kappa").choices(["overall", "kappa"]),
            ),
            "the map is not accurate enough",
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (m, r) = (args.map("map")?, args.map("reference")?);
        same_cells(args, ("map", m), ("reference", r))?;
        let min: f64 = args.opt_param("min")?.ok_or_else(|| Error::msg(format!("node {:?}: give min", args.node)))?;
        let measure: String = args.param("measure", "overall".to_string())?;
        let (value, n) = agreement(m, r, measure == "kappa")?;
        if n == 0 {
            return Err(Error::msg(format!("node {:?}: no cell has a value in both maps", args.node)));
        }
        let name = if measure == "kappa" { "kappa" } else { "overall agreement" };
        let detail = format!("{name} {:.4} of {n} cells, the minimum {}", value, number_text(min));
        verdict(ctx, args, value >= min, "the map is not accurate enough", &detail)
    }
}

/// The overall agreement (or Cohen's kappa) of two maps of classes over the cells with a value in both, and those
/// cells.
fn agreement(m: &Map, r: &Map, kappa: bool) -> Result<(f64, usize)> {
    let mut joint: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    let mut n = 0usize;
    for (a, b) in m.data.iter().zip(r.data.iter()) {
        if a.is_nan() || b.is_nan() {
            continue;
        }
        n += 1;
        *joint.entry((a.round() as i64, b.round() as i64)).or_insert(0) += 1;
    }
    if n == 0 {
        return Ok((f64::NAN, 0));
    }
    let same: usize = joint.iter().filter(|((a, b), _)| a == b).map(|(_, c)| *c).sum();
    let po = same as f64 / n as f64;
    if !kappa {
        return Ok((po, n));
    }
    let (mut rows, mut cols): (BTreeMap<i64, usize>, BTreeMap<i64, usize>) = (BTreeMap::new(), BTreeMap::new());
    for (&(a, b), &c) in &joint {
        *rows.entry(a).or_insert(0) += c;
        *cols.entry(b).or_insert(0) += c;
    }
    let pe: f64 =
        rows.iter().map(|(k, c)| *c as f64 * *cols.get(k).unwrap_or(&0) as f64).sum::<f64>() / (n as f64 * n as f64);
    Ok((if pe >= 1.0 { 1.0 } else { (po - pe) / (1.0 - pe) }, n))
}

/// The demand of a step met by the map allocated.
struct DemandMet;

impl Operator for DemandMet {
    fn signature(&self) -> OpSignature {
        common(
            OpSignature::new(
                "check.demand",
                "A check that an allocation met its demand: the cells of each class of the demand in the map of the \
                 step equal to the demand of that step, up to a tolerance (in cells, or a share of the demand).",
                Access::Global,
            )
            .input(PortSpec::new("landscape", Type::Map, "the map allocated at the step"))
            .input(PortSpec::new("demand", Type::Demand, "the cells of each class at each step"))
            .input(PortSpec::new("step", Type::Integer, "the step (1, 2...; 0 the start)"))
            .param(PortSpec::optional("tolerance", Type::Number, "the cells a class may differ by (0)"))
            .param(PortSpec::optional(
                "relative",
                Type::Bool,
                "the tolerance a share of each class's demand instead of cells (false)",
            )),
            "the demand was not met",
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("landscape")?;
        let d = args.demand("demand")?;
        let step = args.integer("step")?;
        if step < 0 {
            return Err(Error::msg(format!("node {:?}: the step must be 0 or more, not {step}", args.node)));
        }
        let want = d.at(step as usize, &d.classes).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let tolerance: f64 = args.param("tolerance", 0.0)?;
        let relative: bool = args.param("relative", false)?;
        let have = class_cells(m);
        let mut off = Vec::new();
        for (c, w) in d.classes.iter().zip(want) {
            let h = *have.get(&(c.round() as i64)).unwrap_or(&0) as f64;
            let allowed = if relative { tolerance * w.abs() } else { tolerance };
            if (h - w).abs() > allowed + 1e-9 {
                off.push(format!("class {}: {} cells, the demand {}", number_text(*c), number_text(h), number_text(w)));
            }
        }
        let detail = if off.is_empty() {
            format!("the {} classes of step {step} met", d.classes.len())
        } else {
            format!("step {step}: {}", off.join("; "))
        };
        verdict(ctx, args, off.is_empty(), "the demand was not met", &detail)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Demand;

    fn map(v: &[f64]) -> Value {
        Value::Map(Map::new(Arc::new(terraflux_raster::Grid::new(v.len(), 1)), v.to_vec()))
    }

    fn run(op: &dyn Operator, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> (Result<Outputs>, Vec<String>) {
        let params = params.as_object().cloned().unwrap_or_default();
        let args =
            Args { node: "c", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        let mut ctx = Ctx::default();
        let out = op.run(&mut ctx, &args);
        (out, ctx.warnings)
    }

    fn ok(r: &(Result<Outputs>, Vec<String>)) -> bool {
        matches!(r.0.as_ref().map(|o| &o["ok"]), Ok(Value::Bool(true)))
    }

    #[test]
    fn a_range_of_values() {
        let p = map(&[0.0, 0.4, f64::NAN, 1.0]);
        assert!(ok(&run(&Range, vec![("value", p.clone())], serde_json::json!({ "min": 0, "max": 1 }))));
        let (out, _) = run(&Range, vec![("value", map(&[0.2, 1.5, -1.0]))], serde_json::json!({ "min": 0, "max": 1 }));
        let e = out.err().unwrap().to_string();
        assert!(e.contains("2 of 3 cells not between 0 and 1, from -1 to 1.5"), "{e}");
        let r = run(&Range, vec![("value", Value::Number(3.0))], serde_json::json!({ "max": 2, "warn": true }));
        assert!(!ok(&r) && r.1[0].contains("3 is not at most 2"), "{:?}", r.1);
        assert!(run(&Range, vec![("value", p)], serde_json::json!({})).0.is_err(), "no bounds");
    }

    #[test]
    fn no_cells_without_a_value() {
        let m = map(&[1.0, f64::NAN, 2.0, f64::NAN]);
        let (out, _) = run(&NoData, vec![("map", m.clone())], serde_json::json!({}));
        assert!(out.err().unwrap().to_string().contains("2 of 4 cells without a value"));
        // only where the study area has a value
        let area = map(&[1.0, f64::NAN, 1.0, f64::NAN]);
        assert!(ok(&run(&NoData, vec![("map", m.clone()), ("within", area)], serde_json::json!({}))));
        assert!(ok(&run(&NoData, vec![("map", m)], serde_json::json!({ "max_share": 0.5 }))));
    }

    #[test]
    fn an_area_kept() {
        let (a, b) = (map(&[1.0, 2.0, 2.0, 3.0]), map(&[2.0, 1.0, 2.0, 3.0]));
        assert!(ok(&run(
            &AreaKept,
            vec![("before", a.clone()), ("after", b.clone())],
            serde_json::json!({ "class": 1 })
        )));
        let c = map(&[1.0, 1.0, 2.0, 3.0]);
        let (out, _) =
            run(&AreaKept, vec![("before", a.clone()), ("after", c.clone())], serde_json::json!({ "class": 1 }));
        assert!(out.err().unwrap().to_string().contains("class 1: 1 cells before, 2 after"));
        assert!(ok(&run(
            &AreaKept,
            vec![("before", a.clone()), ("after", c)],
            serde_json::json!({ "class": 1, "tolerance": 1 })
        )));
        // a protected area: its cells keep their classes
        let within = map(&[1.0, f64::NAN, f64::NAN, f64::NAN]);
        let (out, _) =
            run(&AreaKept, vec![("before", a.clone()), ("after", b), ("within", within)], serde_json::json!({}));
        assert!(out.err().unwrap().to_string().contains("1 cells changed within the area kept"));
        let (out, _) = run(&AreaKept, vec![("before", a), ("after", map(&[1.0]))], serde_json::json!({}));
        assert!(out.err().unwrap().to_string().contains("differ in size"));
    }

    #[test]
    fn an_accuracy_above_a_minimum() {
        let (m, r) = (map(&[1.0, 1.0, 2.0, 2.0, f64::NAN]), map(&[1.0, 2.0, 2.0, 2.0, 1.0]));
        assert!(ok(&run(
            &Accuracy,
            vec![("map", m.clone()), ("reference", r.clone())],
            serde_json::json!({ "min": 0.75 })
        )));
        // kappa: po 0.75, pe (2·1 + 2·3) / 16 = 0.5, kappa 0.5
        let (out, _) =
            run(&Accuracy, vec![("map", m), ("reference", r)], serde_json::json!({ "min": 0.6, "measure": "kappa" }));
        let e = out.err().unwrap().to_string();
        assert!(e.contains("kappa 0.5000 of 4 cells, the minimum 0.6"), "{e}");
    }

    #[test]
    fn a_demand_met() {
        let d = Value::Demand(Arc::new(Demand {
            classes: vec![1.0, 2.0],
            times: vec![0.0, 1.0],
            table: vec![vec![2.0, 2.0], vec![1.0, 3.0]],
        }));
        let m = map(&[1.0, 2.0, 2.0, 2.0]);
        let step = |k: i64| ("step", Value::Integer(k));
        assert!(ok(&run(
            &DemandMet,
            vec![("landscape", m.clone()), ("demand", d.clone()), step(1)],
            serde_json::json!({})
        )));
        let (out, _) =
            run(&DemandMet, vec![("landscape", m.clone()), ("demand", d.clone()), step(0)], serde_json::json!({}));
        let e = out.err().unwrap().to_string();
        assert!(e.contains("step 0: class 1: 1 cells, the demand 2; class 2: 3 cells, the demand 2"), "{e}");
        assert!(ok(&run(
            &DemandMet,
            vec![("landscape", m), ("demand", d), step(0)],
            serde_json::json!({ "tolerance": 0.5, "relative": true })
        )));
    }
}
