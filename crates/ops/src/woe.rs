//! Weights of evidence of land use transitions (Bonham-Carter 1994), as Dinamica EGO calibrates them and
//! landschaft's `WeightsOfEvidence` reproduces: `woe.fit`, `woe.probability`.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::markov::classes_of;
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

/// How a variable is cut into ranges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VarSpec {
    pub categorical: bool,
    /// The buffer increment (`None`: a hundredth of the variable's range over the class's cells).
    pub increment: Option<f64>,
    pub min_delta: f64,
    pub max_delta: f64,
    pub tolerance: f64,
    /// Given range limits (no generalisation).
    pub breaks: Option<Vec<f64>>,
}

/// How a variable's cells are bucketed for one initial class.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub categorical: bool,
    /// The categories, or the given range limits.
    pub classes: Vec<f64>,
    pub start: f64,
    pub increment: f64,
    pub n: usize,
}

impl Layout {
    fn limit(&self, k: usize) -> f64 {
        if self.categorical {
            if k < self.classes.len() { self.classes[k] } else { self.classes[self.classes.len() - 1] + 1.0 }
        } else {
            self.start + k as f64 * self.increment
        }
    }
}

/// The ranges and weights of one variable for one transition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub from: f64,
    pub to: f64,
    pub variable: String,
    pub spec: VarSpec,
    pub layout: Layout,
    /// The bucket index ending each range.
    pub limits: Vec<usize>,
    /// W+ of each range.
    pub weights: Vec<f64>,
}

impl Entry {
    /// The range of each bucket.
    fn range_of_bucket(&self) -> Vec<usize> {
        let mut out = vec![0usize; self.layout.n];
        let mut lo = 0;
        for (r, &hi) in self.limits.iter().enumerate() {
            for b in out.iter_mut().take(hi).skip(lo) {
                *b = r;
            }
            lo = hi;
        }
        out
    }
}

/// One row of Dinamica's weights report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    pub from: f64,
    pub to: f64,
    pub variable: String,
    pub lower: f64,
    pub upper: f64,
    pub possible: i64,
    pub executed: i64,
    pub weight: f64,
    pub contrast: f64,
    pub significant: bool,
}

/// Fitted weights of evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WoeModel {
    pub transitions: Vec<(f64, f64)>,
    /// Per transition, its variables sorted by name.
    pub entries: Vec<Entry>,
    pub rows: Vec<Row>,
    pub fix_abnormal: bool,
}

impl WoeModel {
    /// The weights report as a table (the variables by their index in `variables`).
    pub fn table(&self) -> (Table, Vec<String>) {
        let mut vars: Vec<String> = self.rows.iter().map(|r| r.variable.clone()).collect();
        vars.sort();
        vars.dedup();
        let mut t = Table::new(
            ["from", "to", "variable", "lower", "upper", "possible", "executed", "weight", "contrast", "significant"]
                .map(String::from)
                .to_vec(),
        );
        for r in &self.rows {
            let v = vars.iter().position(|x| x == &r.variable).unwrap() as f64;
            t.rows.push(vec![
                r.from,
                r.to,
                v,
                r.lower,
                r.upper,
                r.possible as f64,
                r.executed as f64,
                r.weight,
                r.contrast,
                if r.significant { 1.0 } else { 0.0 },
            ]);
        }
        (t, vars)
    }
}

/// The settings of a fit.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    /// Per variable: "categorical", an increment, a list of limits, or { increment, min_delta, max_delta,
    /// tolerance, breaks, categorical }; and per transition ("from->to", as landschaft's (from, to) keys) the
    /// same of its variables, over those of every transition.
    #[serde(default)]
    pub ranges: BTreeMap<String, serde_json::Value>,
    /// W+ set by hand: per transition ("from->to") and variable, a weight per range (null: the one fitted).
    #[serde(default)]
    pub weights: BTreeMap<String, BTreeMap<String, Vec<Option<f64>>>>,
    #[serde(default = "auto")]
    pub increment: serde_json::Value,
    #[serde(default = "one")]
    pub min_delta: f64,
    #[serde(default = "max_delta")]
    pub max_delta: f64,
    #[serde(default = "five")]
    pub tolerance: f64,
    #[serde(default)]
    pub categorical: Vec<String>,
    #[serde(default)]
    pub fix_abnormal: bool,
    #[serde(default = "any")]
    pub possible: String,
    /// The transitions (default: those of the Markov chain given, else every one observed).
    #[serde(default)]
    pub transitions: Option<Vec<(f64, f64)>>,
}

fn auto() -> serde_json::Value {
    "auto".into()
}
fn one() -> f64 {
    1.0
}
fn max_delta() -> f64 {
    100000.0
}
fn five() -> f64 {
    5.0
}
fn any() -> String {
    "any".into()
}

impl Default for Settings {
    fn default() -> Self {
        serde_json::from_value(serde_json::json!({})).unwrap()
    }
}

fn increment_of(v: &serde_json::Value) -> Result<Option<f64>> {
    match v {
        serde_json::Value::Null => Ok(None),
        serde_json::Value::String(s) if s == "auto" => Ok(None),
        serde_json::Value::Number(n) => Ok(n.as_f64()),
        other => Err(Error::msg(format!("an increment must be a number or \"auto\", not {other}"))),
    }
}

/// landschaft's `_spec`: a variable's settings from its entry in `ranges`.
fn spec(name: &str, value: Option<&serde_json::Value>, s: &Settings) -> Result<VarSpec> {
    let default = VarSpec {
        categorical: false,
        increment: increment_of(&s.increment)?,
        min_delta: s.min_delta,
        max_delta: s.max_delta,
        tolerance: s.tolerance,
        breaks: None,
    };
    let Some(value) = value else { return Ok(default) };
    Ok(match value {
        serde_json::Value::Null => default,
        serde_json::Value::String(t) if t == "categorical" => VarSpec { categorical: true, ..default },
        serde_json::Value::Array(a) => {
            let mut b: Vec<f64> = a
                .iter()
                .map(|x| x.as_f64().ok_or_else(|| Error::msg("limits must be numbers")))
                .collect::<Result<_>>()?;
            b.sort_by(|x, y| x.partial_cmp(y).unwrap());
            VarSpec { breaks: Some(b), ..default }
        }
        serde_json::Value::Number(n) => VarSpec { increment: n.as_f64(), ..default },
        serde_json::Value::Object(o) => {
            if o.get("categorical").and_then(|v| v.as_bool()).unwrap_or(false) {
                VarSpec { categorical: true, ..default }
            } else {
                let mut v = default;
                for (k, x) in o {
                    match k.as_str() {
                        "increment" => v.increment = increment_of(x)?,
                        "min_delta" => v.min_delta = x.as_f64().ok_or_else(|| Error::msg("min_delta"))?,
                        "max_delta" => v.max_delta = x.as_f64().ok_or_else(|| Error::msg("max_delta"))?,
                        "tolerance" => v.tolerance = x.as_f64().ok_or_else(|| Error::msg("tolerance"))?,
                        "breaks" => {
                            let mut b: Vec<f64> =
                                serde_json::from_value(x.clone()).map_err(|e| Error::msg(e.to_string()))?;
                            b.sort_by(|p, q| p.partial_cmp(q).unwrap());
                            v.breaks = Some(b);
                        }
                        "categorical" => {}
                        _ => return Err(Error::msg(format!("unknown options for {name:?}: {k:?}"))),
                    }
                }
                v
            }
        }
        other => return Err(Error::msg(format!("cannot read the range settings of {name:?}: {other}"))),
    })
}

/// Buckets of the cells of explicit range limits: the range of each value, -1 outside them.
fn explicit_buckets(values: &[f64], edges: &[f64]) -> Vec<i32> {
    values
        .iter()
        .map(|&f| {
            if f.is_nan() || f < edges[0] || f >= edges[edges.len() - 1] {
                -1
            } else {
                (edges.partition_point(|&e| e <= f) as i32) - 1
            }
        })
        .collect()
}

/// The extent of a variable over the cells of a class: its smallest and largest finite values and, for a
/// categorical one, its distinct values. Extents of parts of a map merge into the whole map's.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Extent {
    pub lo: f64,
    pub hi: f64,
    pub any: bool,
    pub values: Vec<f64>,
}

impl Extent {
    pub fn of(values: &[f64], cells: &[bool], categorical: bool) -> Extent {
        let mut e = Extent { lo: f64::INFINITY, hi: f64::NEG_INFINITY, any: false, values: Vec::new() };
        for (x, c) in values.iter().zip(cells) {
            if *c && x.is_finite() {
                e.any = true;
                e.lo = e.lo.min(*x);
                e.hi = e.hi.max(*x);
                if categorical {
                    e.values.push(*x);
                }
            }
        }
        e.values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        e.values.dedup();
        e
    }

    pub fn merge(&mut self, o: &Extent) {
        if !o.any {
            return;
        }
        if !self.any {
            *self = o.clone();
            return;
        }
        self.lo = self.lo.min(o.lo);
        self.hi = self.hi.max(o.hi);
        self.values.extend_from_slice(&o.values);
        self.values.sort_by(|a, b| a.partial_cmp(b).unwrap());
        self.values.dedup();
    }
}

/// landschaft's `_layout`: the buckets of a variable for one initial class, from its extent over the class's
/// cells.
pub fn layout_from(v: &VarSpec, e: &Extent) -> Result<Layout> {
    if v.categorical {
        let classes = e.values.clone();
        let n = classes.len();
        return Ok(Layout { categorical: true, classes, start: 0.0, increment: 1.0, n });
    }
    if let Some(edges) = &v.breaks {
        let n = edges.len().saturating_sub(1);
        return Ok(Layout { categorical: false, classes: edges.clone(), start: 0.0, increment: 1.0, n });
    }
    let (lo, hi) = if e.any { (e.lo, e.hi) } else { (0.0, 0.0) };
    let inc = match v.increment {
        None => {
            if hi > lo {
                (hi - lo) / 100.0
            } else {
                1.0
            }
        }
        Some(i) => i,
    };
    if inc.is_nan() || inc <= 0.0 {
        return Err(Error::msg("the increment of a variable must be > 0"));
    }
    // Python's min(0.0, math.floor(lo / inc) * inc): floor gives an int (no -0.0)
    let mut fl = (lo / inc).floor();
    if fl == 0.0 {
        fl = 0.0;
    }
    let x = fl * inc;
    let start = if x < 0.0 { x } else { 0.0 };
    let top = ((hi / inc).floor() + 1.0) * inc;
    let n = ((top - start) / inc).round_ties_even().max(1.0) as usize; // Python's round()
    Ok(Layout { categorical: false, classes: Vec::new(), start, increment: inc, n })
}

/// The bucket of each cell of a variable for a fit (-1 where missing or out of the layout).
pub fn fit_buckets(values: &[f64], lay: &Layout, v: &VarSpec) -> Result<Vec<i32>> {
    if lay.categorical {
        return Ok(ffi::woe_buckets(values, true, &lay.classes, 0.0, 1.0, lay.n as i64)?);
    }
    if v.breaks.is_some() {
        return Ok(explicit_buckets(values, &lay.classes));
    }
    Ok(ffi::woe_buckets(values, false, &[], lay.start, lay.increment, lay.n as i64)?)
}

fn layout(values: &[f64], v: &VarSpec, cells: &[bool]) -> Result<(Layout, Vec<i32>)> {
    let lay = layout_from(v, &Extent::of(values, cells, v.categorical))?;
    let b = fit_buckets(values, &lay, v)?;
    Ok((lay, b))
}

/// The bucket of every cell of a (possibly new) driver map for an entry: values beyond the calibrated extent
/// take the first or the last bucket.
fn cell_buckets(e: &Entry, values: &[f64]) -> Result<Vec<i32>> {
    let l = &e.layout;
    if e.spec.breaks.is_some() {
        return Ok(explicit_buckets(values, &l.classes));
    }
    if l.categorical {
        return Ok(ffi::woe_buckets(values, true, &l.classes, 0.0, 1.0, l.n as i64)?);
    }
    let mut k = ffi::woe_buckets(values, false, &[], l.start, l.increment, l.n as i64)?;
    let top = l.start + l.n as f64 * l.increment;
    for (b, &v) in k.iter_mut().zip(values) {
        if *b < 0 {
            if v < l.start {
                *b = 0;
            } else if v >= top {
                *b = l.n as i32 - 1;
            }
        }
    }
    Ok(k)
}

/// The settings of each variable (`variables` sorted by name), from a fit's settings.
pub fn variable_specs(variables: &[String], s: &Settings) -> Result<Vec<VarSpec>> {
    if s.possible != "any" && s.possible != "stay" {
        return Err(Error::msg("possible must be \"any\" or \"stay\""));
    }
    // (the transitions' own settings apart)
    let mut common: BTreeMap<String, serde_json::Value> =
        s.ranges.iter().filter(|(k, _)| transition_key(k).is_none()).map(|(k, v)| (k.clone(), v.clone())).collect();
    for name in &s.categorical {
        common.insert(name.clone(), "categorical".into());
    }
    if let Some(unknown) = common.keys().find(|k| !variables.contains(k)) {
        return Err(Error::msg(format!("no driver named {unknown:?}")));
    }
    variables.iter().map(|name| spec(name, common.get(name), s)).collect()
}

/// The transition a key of `ranges` or `weights` names: "from->to".
pub fn transition_key(k: &str) -> Option<(f64, f64)> {
    let (a, b) = k.split_once("->")?;
    Some((a.trim().parse().ok()?, b.trim().parse().ok()?))
}

/// The key of a transition in `ranges` and `weights`.
pub fn key_of(f: f64, t: f64) -> String {
    format!("{}->{}", crate::value::class_label(f), crate::value::class_label(t))
}

/// A variable's settings for a transition: its own in `ranges["from->to"]`, else those of every transition.
fn spec_for(f: f64, t: f64, name: &str, common: &VarSpec, s: &Settings) -> Result<VarSpec> {
    let own = s.ranges.iter().find(|(k, _)| transition_key(k) == Some((f, t))).and_then(|(_, v)| v.get(name));
    match own {
        Some(v) => {
            let categorical = s.categorical.iter().any(|c| c == name);
            let v = if categorical { serde_json::Value::String("categorical".into()) } else { v.clone() };
            spec(name, Some(&v), s)
        }
        None => Ok(common.clone()),
    }
}

/// The weights set by hand over those fitted: of every entry with some, its ranges' W+ (and the report's).
pub fn set_weights(entries: &mut [Entry], rows: &mut [Row], s: &Settings) -> Result<()> {
    for (k, by_var) in &s.weights {
        let (f, t) =
            transition_key(k).ok_or_else(|| Error::msg(format!("weights: {k:?} is no transition (from->to)")))?;
        for (name, w) in by_var {
            let e = entries
                .iter_mut()
                .find(|e| e.from == f && e.to == t && &e.variable == name)
                .ok_or_else(|| Error::msg(format!("weights: no {name:?} for the transition {k}")))?;
            if w.len() != e.weights.len() {
                return Err(Error::msg(format!(
                    "weights of {k} {name}: {} for {} ranges (refit the ranges, or give one a range)",
                    w.len(),
                    e.weights.len()
                )));
            }
            let mine: Vec<&mut Row> =
                rows.iter_mut().filter(|r| r.from == f && r.to == t && &r.variable == name).collect();
            for ((x, given), row) in e.weights.iter_mut().zip(w).zip(mine) {
                if let Some(g) = given {
                    *x = *g;
                    row.weight = *g;
                }
            }
        }
    }
    Ok(())
}

/// The transitions observed in a matrix of counts (classes x classes), rows then columns.
pub fn observed_transitions(classes: &[f64], counts: &[i64]) -> Vec<(f64, f64)> {
    let k = classes.len();
    let mut t = Vec::new();
    for i in 0..k {
        for j in 0..k {
            if i != j && counts[i * k + j] != 0 {
                t.push((classes[i], classes[j]));
            }
        }
    }
    t
}

/// The ranges and weights of one variable for one transition, from the cells of its initial class by bucket
/// and final class (`counts`: buckets x `fin`).
#[allow(clippy::too_many_arguments)]
pub fn entry_from_counts(
    f: f64,
    t: f64,
    name: &str,
    v: VarSpec,
    lay: Layout,
    counts: &[i64],
    fin: &[f64],
    s: &Settings,
) -> Result<(Entry, Vec<Row>)> {
    let j = fin.partition_point(|&c| c < t);
    let stay = fin.partition_point(|&c| c < f);
    let has_t = j < fin.len() && fin[j] == t;
    let has_stay = stay < fin.len() && fin[stay] == f;
    let c = counts;
    let nf = fin.len();
    let executed: Vec<i64> = (0..lay.n).map(|r| if has_t { c[r * nf + j] } else { 0 }).collect();
    let possible: Vec<i64> = if s.possible == "any" {
        (0..lay.n).map(|r| c[r * nf..(r + 1) * nf].iter().sum()).collect()
    } else {
        (0..lay.n).map(|r| executed[r] + if has_stay { c[r * nf + stay] } else { 0 }).collect()
    };
    let limits: Vec<usize> = if lay.categorical || v.breaks.is_some() {
        (1..=lay.n).collect()
    } else {
        ffi::woe_generalise(&possible, &executed, v.min_delta, v.max_delta, v.tolerance, s.fix_abnormal)?
            .into_iter()
            .map(|x| x as usize)
            .collect()
    };
    let mut cb = vec![0i64];
    let mut cn = vec![0i64];
    for r in 0..lay.n {
        cb.push(cb[r] + possible[r]);
        cn.push(cn[r] + executed[r]);
    }
    let (big_a, big_n) = (cb[lay.n] as f64, cn[lay.n] as f64);
    let mut weights = Vec::with_capacity(limits.len());
    let mut rows = Vec::new();
    let mut lo = 0;
    for &hi in &limits {
        let (nb, bb) = (cn[hi] - cn[lo], cb[hi] - cb[lo]);
        let w = ffi::woe_weights(nb as f64, bb as f64, big_n, big_a, s.fix_abnormal);
        weights.push(w.plus);
        let (lower, upper) =
            if v.breaks.is_none() { (lay.limit(lo), lay.limit(hi)) } else { (lay.classes[lo], lay.classes[hi]) };
        rows.push(Row {
            from: f,
            to: t,
            variable: name.to_string(),
            lower,
            upper,
            possible: bb,
            executed: nb,
            weight: w.plus,
            contrast: w.contrast,
            significant: w.significant,
        });
        lo = hi;
    }
    Ok((Entry { from: f, to: t, variable: name.to_string(), spec: v, layout: lay, limits, weights }, rows))
}

/// Calibrate the ranges and weights from the map at the start (`t0`) and at the end (`t1`).
pub fn fit(
    t0: &Map,
    t1: &Map,
    drivers: &Stack,
    s: &Settings,
    transitions: Option<Vec<(f64, f64)>>,
) -> Result<WoeModel> {
    let a = &t0.data;
    let b = &t1.data;
    if a.len() != b.len() || drivers.grid.cells() != a.len() {
        return Err(Error::msg("the maps and the drivers differ in size"));
    }
    let mut variables = drivers.names();
    variables.sort();
    let specs = variable_specs(&variables, s)?;
    let transitions = match transitions.or_else(|| s.transitions.clone()) {
        Some(t) => t,
        None => {
            let cls = classes_of(&[t0, t1]);
            observed_transitions(&cls, &ffi::crosstab(a, b, &cls, &cls)?)
        }
    };
    let fin = classes_of(&[t1]);
    let valid: Vec<bool> = a.iter().zip(b.iter()).map(|(x, y)| !x.is_nan() && !y.is_nan()).collect();
    let mut cache: BTreeMap<String, (Layout, Vec<i64>)> = BTreeMap::new();
    let mut entries = Vec::new();
    let mut rows = Vec::new();
    for &(f, t) in &transitions {
        for (name, common) in variables.iter().zip(&specs) {
            let own = spec_for(f, t, name, common, s)?;
            let v = &own;
            let values = drivers.by_name(name).unwrap();
            let sig = format!("{f}|{name}|{}|{:?}|{:?}", v.categorical, v.increment, v.breaks);
            if !cache.contains_key(&sig) {
                let cells: Vec<bool> = valid.iter().zip(a.iter()).map(|(&ok, &x)| ok && x == f).collect();
                let (lay, buckets) = layout(values, v, &cells)?;
                let counts = ffi::woe_counts(a, b, &buckets, lay.n as i64, f, &fin)?;
                cache.insert(sig.clone(), (lay, counts));
            }
            let (lay, c) = &cache[&sig];
            let (e, r) = entry_from_counts(f, t, name, v.clone(), lay.clone(), c, &fin, s)?;
            entries.push(e);
            rows.extend(r);
        }
    }
    set_weights(&mut entries, &mut rows, s)?;
    Ok(WoeModel { transitions, entries, rows, fix_abnormal: s.fix_abnormal })
}

/// The probabilities of the transitions at each cell (Dinamica EGO's CalcWOfEProbabilityMap):
/// exp(S_ij) / (1 + sum_k exp(S_ik)) over the transitions from the same class, S the sum of the weights of
/// the cell's ranges (a variable missing at a cell adds nothing); NaN at cells of other classes.
pub fn probability(model: &WoeModel, landscape: &Map, drivers: &Stack) -> Result<Stack> {
    let n = landscape.data.len();
    if drivers.grid.cells() != n {
        return Err(Error::msg("the drivers differ in size from the landscape"));
    }
    let mut sums: Vec<Vec<f64>> = Vec::with_capacity(model.transitions.len());
    // the buckets of a variable by a layout, computed once (the transitions from a class share their layouts)
    let mut known: Vec<(&Entry, Vec<i32>)> = Vec::new();
    for &(f, t) in &model.transitions {
        let mut buckets = Vec::new();
        let mut weights = Vec::new();
        let mut offsets = vec![0i64];
        for e in model.entries.iter().filter(|e| e.from == f && e.to == t) {
            let values =
                drivers.by_name(&e.variable).ok_or_else(|| Error::msg(format!("drivers lack {:?}", e.variable)))?;
            let same = |o: &Entry| o.variable == e.variable && o.layout == e.layout && o.spec == e.spec;
            match known.iter().find(|(o, _)| same(o)) {
                Some((_, b)) => buckets.extend_from_slice(b),
                None => {
                    let b = cell_buckets(e, values)?;
                    buckets.extend_from_slice(&b);
                    known.push((e, b));
                }
            }
            let rb = e.range_of_bucket();
            weights.extend(rb.iter().map(|&r| e.weights[r]));
            offsets.push(weights.len() as i64);
        }
        sums.push(ffi::woe_probability(&landscape.data, f, &buckets, &weights, &offsets)?);
    }
    // normalise the transitions from each class together, in the order the classes first appear
    let mut done = vec![false; model.transitions.len()];
    for i in 0..model.transitions.len() {
        if done[i] {
            continue;
        }
        let f = model.transitions[i].0;
        let idx: Vec<usize> = (0..model.transitions.len()).filter(|&k| model.transitions[k].0 == f).collect();
        let mut block: Vec<f64> = idx.iter().flat_map(|&k| sums[k].iter().copied()).collect();
        ffi::woe_normalise(&mut block, idx.len() as i32, n as i64)?;
        for (r, &k) in idx.iter().enumerate() {
            sums[k].copy_from_slice(&block[r * n..(r + 1) * n]);
            done[k] = true;
        }
    }
    Ok(Stack {
        grid: landscape.grid.clone(),
        keys: model.transitions.iter().map(|&(f, t)| Key::Transition(f, t)).collect(),
        layers: sums.into_iter().map(Arc::new).collect(),
    })
}

struct FitOp;

impl Operator for FitOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "woe.fit",
            "Weights of evidence of each transition, calibrated on two maps as Dinamica EGO does: the ranges of a \
             continuous variable from nested buffers of `increment` cut by line generalisation (a new range when \
             the direction deviates by more than `tolerance` degrees or the range passes `max_delta` cells, none \
             below `min_delta`), a range per category of a categorical one; W+ per range.",
            Access::Global,
        )
        .input(PortSpec::new("t0", Type::Map, "the map at the start"))
        .input(PortSpec::new("t1", Type::Map, "the map at the end"))
        .input(PortSpec::new("drivers", Type::Stack, "the explanatory variables"))
        .input(PortSpec::optional("markov", Type::Markov, "its transitions with a rate are calibrated"))
        .output(PortSpec::new("model", Type::Woe, "the weights"))
        .output(PortSpec::new("table", Type::Table, "Dinamica's weights report"))
        .param(PortSpec::optional("increment", Type::Any, "buffer increment, or \"auto\" (a hundredth of the range)"))
        .param(PortSpec::optional("min_delta", Type::Number, "smallest range, cells (1)"))
        .param(PortSpec::optional("max_delta", Type::Number, "largest range, cells (100000)"))
        .param(PortSpec::optional("tolerance", Type::Number, "tolerance angle, degrees (5)"))
        .param(PortSpec::optional(
            "ranges",
            Type::Any,
            "per variable: \"categorical\", an increment, limits or options; per transition (\"1->2\") its own",
        ))
        .param(PortSpec::optional(
            "weights",
            Type::Any,
            "W+ set by hand: {\"1->2\": {variable: [a weight per range, or null]}} (the viewer's WoE editor)",
        ))
        .param(PortSpec::optional("categorical", Type::Any, "names of categorical variables"))
        .param(PortSpec::optional("fix_abnormal", Type::Bool, "finite weights for ranges without transitions"))
        .param(PortSpec::optional("possible", Type::Text, "\"any\" (default) or \"stay\"").choices(["any", "stay"]))
        .param(PortSpec::optional("transitions", Type::Any, "[[from, to], ...] (default: the Markov chain's)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s: Settings = serde_json::from_value(serde_json::Value::Object(args.params.clone()))
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let transitions = match args.get("markov") {
            Some(_) => Some(args.markov("markov")?.transitions()),
            None => None,
        };
        let m = fit(args.map("t0")?, args.map("t1")?, args.stack("drivers")?, &s, transitions)?;
        let (table, _) = m.table();
        Ok(outputs([("model", Value::Woe(Arc::new(m))), ("table", Value::Table(table))]))
    }
}

struct ProbabilityOp;

impl Operator for ProbabilityOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "woe.probability",
            "The probability of each calibrated transition at every cell of its initial class, from the weights of \
             evidence (Dinamica EGO's CalcWOfEProbabilityMap).",
            Access::Local,
        )
        .input(PortSpec::new("model", Type::Woe, "the weights"))
        .input(PortSpec::new("landscape", Type::Map, "the map the probabilities are for"))
        .input(PortSpec::new("drivers", Type::Stack, "the explanatory variables (they may have changed)"))
        .output(PortSpec::new("probability", Type::Stack, "a layer per transition"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let p = probability(args.woe("model")?, args.map("landscape")?, args.stack("drivers")?)?;
        Ok(outputs([("probability", Value::Stack(p))]))
    }
}

pub fn register(r: &mut Registry) {
    r.add(Arc::new(FitOp));
    r.add(Arc::new(ProbabilityOp));
}
