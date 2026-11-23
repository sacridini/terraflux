//! Simulation from a step selection model (amt's make_issf_model, redistribution_kernel and simulate_path in
//! continuous space, Signer et al. 2024), a resistance surface from its habitat terms and the use of a landscape by
//! the simulated paths: the library's `_issf_sim.py`, with its C++ for the draws (R's generator, carried from the
//! kernel's first cycle into the path as the library does), so the paths are the library's bits, with one thread or
//! many.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::{RRng, ffi};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::formula::{Col, Frame, Kind, Level, Node, Spec, Terms, arith, codes_of, columns_of, parse_formula, raw_of};
use super::issf::StepModel;
use super::ssf::{Distribution, distribution_of};
use super::{Times, fail, give, time, track_column};
use crate::sdm::points::{grid_cells, grid_of, to_crs};
use crate::{Args, Ctx, Map, Operator, Outputs, Registry, Result, Stack, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(IssfModel));
    r.add(Arc::new(SimulatePath));
    r.add(Arc::new(Resistance));
    r.add(Arc::new(PathDensity));
}

type Res<T> = std::result::Result<T, String>;

/// The movement terms of a model: the step's length and turn angle, and their log and cosine.
const MOVE: [&str; 4] = ["sl_", "ta_", "log_sl_", "cos_ta_"];
const MATH: [&str; 6] = ["log", "exp", "sqrt", "sin", "cos", "log1p"];
const OP_FN: [i32; 6] = [12, 13, 14, 15, 16, 17];

fn ce(e: cxx::Exception) -> String {
    e.what().to_string()
}

// ----------------------------------------------------------------------------------------- the recipe

/// A column of the model matrix as a product: its factors as (variable, level) pairs, level -1 for a number.
type Recipe = Vec<(usize, i64)>;

/// The columns of the model matrix as products: their names and, for each, its factors, in the order and with the
/// names of `Terms::design` (the library's `_recipe`).
pub fn recipe(sp: &Spec) -> Res<(Vec<String>, Vec<Recipe>)> {
    let (mut names, mut cols) = (Vec::new(), Vec::new());
    for t in &sp.terms {
        if sp.strata.is_some_and(|s| *t == [s]) {
            continue;
        }
        let mut part: Vec<(String, Recipe)> = vec![(String::new(), vec![])];
        for &i in t {
            let v = &sp.variables[i];
            let vcols: Vec<(String, (usize, i64))> = if v.kind == Kind::Num {
                vec![(v.label.clone(), (i, -1))]
            } else {
                let margin: Vec<usize> = t.iter().copied().filter(|&j| j != i).collect();
                let first = usize::from(margin.is_empty() || sp.terms.contains(&margin));
                if v.levels.len() < first + 1 {
                    return Err(format!("{}: a factor with fewer than 2 levels has no contrasts", v.label));
                }
                (first..v.levels.len()).map(|k| (format!("{}{}", v.label, v.labels[k]), (i, k as i64))).collect()
            };
            let mut next = Vec::with_capacity(vcols.len() * part.len());
            for (vn, vf) in &vcols {
                for (pn, pf) in &part {
                    let name = if pn.is_empty() { vn.clone() } else { format!("{pn}:{vn}") };
                    let mut f = pf.clone();
                    f.push(*vf);
                    next.push((name, f));
                }
            }
            part = next;
        }
        for (nm, f) in part {
            names.push(nm);
            cols.push(f);
        }
    }
    Ok((names, cols))
}

// ------------------------------------------------------------------------------------------ the model

/// A model from coefficients by the names of its terms: the `coefficients` setting (an object, or a list of
/// [term, value] pairs to keep their order) and the distributions the candidate steps are drawn from.
pub fn make_model(given: &[(String, f64)], sl: Distribution, ta: Distribution) -> Res<StepModel> {
    if given.is_empty() {
        return Err("coefficients: named numbers, one for each term".into());
    }
    for (i, (k, _)) in given.iter().enumerate() {
        if given[..i].iter().any(|(j, _)| j == k) {
            return Err("coefficients: named numbers, one for each term".into());
        }
    }
    if given.iter().any(|(_, v)| !v.is_finite()) {
        return Err("coefficients: finite numbers".into());
    }
    let text =
        format!("case_ ~ {} + strata(step_id_)", given.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>().join(" + "));
    let sp = parse_formula(&text)?;
    if sp.variables.iter().any(|v| matches!(&v.ast, Node::Call(n, _) if n == "factor")) {
        return Err("coefficients: a factor term needs the levels of a fit (use move.fit_issf)".into());
    }
    let (names, _) = recipe(&sp)?;
    let mut values: Vec<(String, f64)> = given.to_vec();
    let mut ordered = Vec::with_capacity(names.len());
    for nm in &names {
        let direct = values.iter().position(|(k, _)| k == nm);
        // amt's ssf_weights reads a:b as b:a too
        let alt: String = nm.split(':').rev().collect::<Vec<_>>().join(":");
        let at = direct.or_else(|| values.iter().position(|(k, _)| *k == alt));
        match at {
            Some(i) => ordered.push(values.remove(i).1),
            None => return Err(format!("coefficients: no coefficient for the term {nm:?}")),
        }
    }
    if !values.is_empty() {
        let mut left: Vec<String> = values.into_iter().map(|(k, _)| format!("{k:?}")).collect();
        left.sort();
        return Err(format!("coefficients: the terms [{}] repeat a term of the formula", left.join(", ")));
    }
    let p = names.len();
    let nan = f64::NAN;
    Ok(StepModel {
        formula: sp.text.clone(),
        terms: Terms { spec: sp, columns: names },
        coef: ordered,
        vcov: vec![nan; p * p],
        means: vec![0.0; p],
        loglik: vec![nan, nan],
        iter: 0,
        n: 0,
        nevent: 0,
        wald: nan,
        score: nan,
        converged: true,
        n_omitted: 0,
        sl_distr: Some(sl),
        ta_distr: Some(ta),
        data: None,
    })
}

/// The coefficients of the setting: an object of numbers, or a list of [term, number] pairs.
fn coefficients_of(args: &Args) -> Result<Vec<(String, f64)>> {
    let node = args.node;
    let bad = || fail(node, "coefficients: terms to numbers, {\"slope\": -0.05, \"log_sl_\": 0.3}");
    let number = |v: &serde_json::Value| v.as_f64().ok_or_else(bad);
    match args.params.get("coefficients") {
        Some(serde_json::Value::Object(o)) => o.iter().map(|(k, v)| Ok((k.clone(), number(v)?))).collect(),
        Some(serde_json::Value::Array(a)) => a
            .iter()
            .map(|p| match p.as_array().map(Vec::as_slice) {
                Some([k, v]) => Ok((k.as_str().ok_or_else(bad)?.to_string(), number(v)?)),
                _ => Err(bad()),
            })
            .collect(),
        _ => Err(bad()),
    }
}

/// The distributions of a simulation: the ones given, else those of the model (the library's `_distributions`).
fn distributions(
    m: &StepModel,
    sl: Option<&Distribution>,
    ta: Option<&Distribution>,
) -> Res<(Distribution, Distribution)> {
    let (Some(s), Some(t)) = (sl.or(m.sl_distr.as_ref()), ta.or(m.ta_distr.as_ref())) else {
        return Err("the model has no step length and turn angle distributions (the random steps had several, or it \
                    is not from move.fit_issf with sl_distr and ta_distr): connect sl_distr and ta_distr, or use \
                    move.issf_model"
            .into());
    };
    if !["gamma", "exp", "lnorm", "hnorm", "unif"].contains(&s.name.as_str()) {
        return Err(format!("sl_distr: {:?} is not a distribution of step lengths", s.name));
    }
    if !["vonmises", "unif"].contains(&t.name.as_str()) {
        return Err(format!("ta_distr: {:?} is not a distribution of turn angles", t.name));
    }
    Ok((s.clone(), t.clone()))
}

/// `move.issf_model`.
struct IssfModel;

impl Operator for IssfModel {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.issf_model",
            "A step selection model from coefficients, to simulate from (amt's make_issf_model): the coefficients \
             by the names of the terms of a formula, {\"forest_end\": 1.5, \"sl_\": -0.002, \"cos_ta_\": 0.4}. The \
             terms are R's (log(sl_), forest_end:sl_, I(elev^2), exp(x)...); the covariates are those \
             move.extract_covariates makes (name_end and name_start, or name for the end of the step) and the \
             movement terms sl_, ta_, log_sl_ and cos_ta_. An interaction can be given in either order (a:b or \
             b:a), as amt reads it; a factor needs the levels of a fit. The numbers are the model's: its covariance \
             and log-likelihood have none (NaN). The candidate steps are drawn from the distributions given \
             (amt's default is an exponential of rate 1 and a uniform turn angle: make them with \
             move.distribution). A setting is an object whose keys are sorted: give a list of [term, value] pairs \
             to keep the order of the terms the library sums them in.",
            Access::None,
        )
        .input(PortSpec::new(
            "sl_distr",
            Type::StepDistribution,
            "the step length distribution to draw candidates from",
        ))
        .input(PortSpec::new("ta_distr", Type::StepDistribution, "the turn angle distribution to draw candidates from"))
        .output(PortSpec::new("model", Type::StepModel, "the step selection model"))
        .param(PortSpec::new(
            "coefficients",
            Type::Any,
            "the coefficients by the names of their terms: {\"slope\": -0.05, \"log_sl_\": 0.3}, or a list of \
             [term, value] pairs",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let given = coefficients_of(args)?;
        let need = |p: &str| -> Result<Distribution> {
            Ok((**distribution_of(args, p)?.ok_or_else(|| fail(node, format!("connect {p} (move.distribution)")))?)
                .clone())
        };
        let model = make_model(&given, need("sl_distr")?, need("ta_distr")?).map_err(|e| fail(node, e))?;
        Ok(outputs([("model", Value::StepModel(Arc::new(model)))]))
    }
}

// ---------------------------------------------------------------------------------------- the layers

/// A raster of the covariates: its name (as move.extract_covariates names its column), its grid and its values.
type Layer = (String, Arc<Grid>, Arc<Vec<f64>>);

/// The rasters connected as inputs of any name other than `skip`, a layer each (a stack's: `input_layer`).
fn layers_of(args: &Args, skip: &[&str]) -> Result<Vec<Layer>> {
    let node = args.node;
    let mut layers: Vec<Layer> = Vec::new();
    for (port, value) in &args.inputs {
        if skip.contains(&port.as_str()) {
            continue;
        }
        let value = crate::files::as_port(value.clone(), Type::Any)?;
        match &value {
            Value::Map(m) => layers.push((port.clone(), m.grid.clone(), m.data.clone())),
            Value::Stack(s) if s.layers.len() == 1 => layers.push((port.clone(), s.grid.clone(), s.layers[0].clone())),
            Value::Stack(s) => {
                for (key, l) in s.keys.iter().zip(&s.layers) {
                    layers.push((format!("{port}_{key}"), s.grid.clone(), l.clone()));
                }
            }
            other => return Err(fail(node, format!("{port}: a raster, not a {}", other.ty().name()))),
        }
    }
    if layers.is_empty() {
        return Err(fail(node, "connect the covariates (rasters, as inputs of any names: forest, elev...)"));
    }
    let names: Vec<&String> = layers.iter().map(|l| &l.0).collect();
    if names.iter().enumerate().any(|(i, n)| names[..i].contains(n)) {
        return Err(fail(node, format!("covariates: the layers need different names, got {names:?}")));
    }
    for (name, g, _) in &layers {
        let t = g.transform;
        if t[2] != 0.0 || t[4] != 0.0 {
            return Err(fail(node, format!("{name}: rotated rasters are not supported")));
        }
        if !(t[1] > 0.0 && t[5] < 0.0) {
            return Err(fail(
                node,
                format!("{name}: rasters with north up (cells growing eastwards and southwards) are expected"),
            ));
        }
    }
    Ok(layers)
}

// ----------------------------------------------------------------------------------- the formula

/// The reverse Polish program of an arithmetic expression of the formula.
fn program(node: &Node, leaf: &dyn Fn(&str) -> Res<(i32, f64)>, ops: &mut Vec<i32>, args: &mut Vec<f64>) -> Res<()> {
    match node {
        Node::Num(v) => {
            ops.push(0);
            args.push(*v);
        }
        Node::Paren(e) | Node::Pos(e) => return program(e, leaf, ops, args),
        Node::Neg(e) => {
            program(e, leaf, ops, args)?;
            ops.push(10);
            args.push(0.0);
        }
        Node::Name(n) => {
            let (op, a) = leaf(n)?;
            ops.push(op);
            args.push(a);
        }
        Node::Call(name, a) => {
            let Some(arg) = a.first() else { return Err(format!("formula: {name}() takes one argument")) };
            program(arg, leaf, ops, args)?;
            if name == "I" {
            } else if name == "abs" {
                ops.push(11);
                args.push(0.0);
            } else if let Some(k) = MATH.iter().position(|m| m == name) {
                ops.push(OP_FN[k]);
                args.push(0.0);
            } else {
                return Err(format!("formula: {name}() cannot be inside an expression"));
            }
        }
        Node::Bin(op, l, r) => {
            let code = match op {
                '+' => 20,
                '-' => 21,
                '*' => 22,
                '/' => 23,
                '^' => 24,
                o => return Err(format!("formula: '{o}' is not supported inside an expression")),
            };
            program(l, leaf, ops, args)?;
            program(r, leaf, ops, args)?;
            ops.push(code);
            args.push(0.0);
        }
    }
    Ok(())
}

/// name -> (operation, argument): the covariates (`x`, `x_end` at the end of the step, `x_start` at its start) and the
/// movement terms.
fn leaf_of(names: &[String]) -> impl Fn(&str) -> Res<(i32, f64)> + '_ {
    move |name: &str| {
        let at = |n: &str| names.iter().position(|x| x == n);
        if let Some(i) = at(name) {
            return Ok((1, i as f64));
        }
        if let Some(i) = name.strip_suffix("_end").and_then(at) {
            return Ok((1, i as f64));
        }
        if let Some(i) = name.strip_suffix("_start").and_then(at) {
            return Ok((2, i as f64));
        }
        if let Some(k) = MOVE.iter().position(|m| *m == name) {
            return Ok((3 + k as i32, 0.0));
        }
        let shown: Vec<String> = names.iter().take(3).map(|n| format!("{n}_end, {n}_start")).collect();
        Err(format!(
            "the term {name:?} of the model is not a covariate ({}{}) nor one of {}",
            shown.join(", "),
            if names.len() > 3 { "..." } else { "" },
            MOVE.join(", ")
        ))
    }
}

/// The simulation's spec: the formula of the model as programs, the layers, the study area and the distributions.
fn spec_of(
    m: &StepModel,
    layers: &[Layer],
    sl: &Distribution,
    ta: &Distribution,
    n_control: i64,
    tolerance: f64,
) -> Res<ffi::MoveSimSpec> {
    let names: Vec<String> = layers.iter().map(|l| l.0.clone()).collect();
    let sp = &m.terms.spec;
    let (cols_names, cols) = recipe(sp)?;
    if cols_names != m.terms.columns {
        return Err("the columns of the model do not match its coefficients".into());
    }
    let leaf = leaf_of(&names);
    let (mut ops, mut args) = (Vec::new(), Vec::new());
    let (mut var_ptr, mut var_factor, mut level_ptr, mut levels) = (vec![0i32], vec![], vec![0i32], Vec::<f64>::new());
    for (i, v) in sp.variables.iter().enumerate() {
        if Some(i) == sp.strata {
            ops.push(0);
            args.push(0.0);
            var_factor.push(0);
        } else if v.kind == Kind::Num {
            program(&v.ast, &leaf, &mut ops, &mut args)?;
            var_factor.push(0);
        } else {
            let inner = v.inner.as_ref().ok_or("formula: factor() takes one argument")?;
            program(inner, &leaf, &mut ops, &mut args)?;
            var_factor.push(1);
            for l in &v.levels {
                levels.push(match l {
                    Level::Num(x) => *x,
                    Level::Bool(b) => f64::from(u8::from(*b)),
                    Level::Text(_) => {
                        return Err(format!("the levels of {:?} are not numbers: a raster's values are", v.label));
                    }
                });
            }
        }
        var_ptr.push(ops.len() as i32);
        level_ptr.push(levels.len() as i32);
    }
    let (mut col_ptr, mut col_var, mut col_level) = (vec![0i32], vec![], vec![]);
    for c in &cols {
        for &(vi, lv) in c {
            col_var.push(vi as i32);
            col_level.push(lv as i32);
        }
        col_ptr.push(col_var.len() as i32);
    }
    let coef: Vec<f64> = m.coef.iter().map(|&c| if c.is_nan() { 0.0 } else { c }).collect();
    let (mut values, mut dims, mut grids) = (Vec::new(), Vec::new(), Vec::new());
    let mut ext: Option<(f64, f64, f64, f64)> = None;
    for (_, g, v) in layers {
        let (xmin, xmax, ymin, ymax, xres, yres, ny, nx) = grid_of(g);
        if v.len() != ny * nx {
            return Err("a covariate has not as many values as its grid has cells".into());
        }
        values.extend_from_slice(v);
        dims.extend([ny as i64, nx as i64]);
        grids.extend([xmin, xmax, ymin, ymax, xres, yres]);
        ext = Some(match ext {
            None => (xmin, xmax, ymin, ymax),
            Some(e) => (e.0.max(xmin), e.1.min(xmax), e.2.max(ymin), e.3.min(ymax)),
        });
    }
    let e = ext.ok_or("covariates: at least one raster")?;
    Ok(ffi::MoveSimSpec {
        sl_name: sl.name.clone(),
        ta_name: ta.name.clone(),
        sl_par: sl.draw_params(),
        ta_par: ta.draw_params(),
        bbox: vec![e.0, e.1, e.2, e.3],
        n_control,
        tolerance_outside: tolerance,
        values,
        dims,
        grids,
        var_ptr,
        op: ops,
        arg: args,
        var_factor,
        level_ptr,
        level: levels,
        col_ptr,
        col_var,
        col_level,
        coef,
    })
}

// ----------------------------------------------------------------------------------------- the start

/// The time of a start: a datetime (nanoseconds since 1970) or a number.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Clock {
    Instant(i64),
    Number(f64),
}

/// Where a simulation starts (amt's make_start): a location, the direction of the step before it, the time and the
/// time between steps (seconds, or units when the time is a number).
#[derive(Debug, Clone)]
struct Start {
    label: Option<String>,
    x: f64,
    y: f64,
    ta: f64,
    t: Clock,
    dt: f64,
}

const EPOCH: Clock = Clock::Instant(0);

/// A time given in a setting: a number, or a datetime in a text.
fn clock_of(v: &serde_json::Value) -> Res<Clock> {
    match v {
        serde_json::Value::Number(n) => n.as_f64().map(Clock::Number).ok_or_else(|| "time: a number".to_string()),
        serde_json::Value::String(s) => match time::parse(s) {
            Some(ns) => Ok(Clock::Instant(ns)),
            None => s.trim().parse::<f64>().map(Clock::Number).map_err(|_| format!("time: {s:?} is not a datetime")),
        },
        _ => Err("time: a datetime in a text, or a number".into()),
    }
}

/// The starts: the first location of each track of `start`, or the settings x and y.
fn starts_of(args: &Args, crs: Option<&str>) -> Result<Vec<Start>> {
    let node = args.node;
    let e = |m: String| fail(node, m);
    let direction = args.opt_param::<f64>("direction")?;
    let time_set = match args.params.get("time") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => Some(clock_of(v).map_err(e)?),
    };
    let dt_set = args.opt_param::<f64>("dt")?;
    let finish = |label: Option<String>, x: f64, y: f64, ta: f64, t: Clock| -> Result<Start> {
        if !(x.is_finite() && y.is_finite() && ta.is_finite()) {
            return Err(fail(node, "start: finite numbers for x, y and the direction"));
        }
        let t = time_set.unwrap_or(t);
        let dt = dt_set.unwrap_or(if matches!(t, Clock::Number(_)) { 1.0 } else { 3600.0 });
        if !dt.is_finite() {
            return Err(fail(node, "dt: a number of seconds"));
        }
        Ok(Start { label, x, y, ta, t, dt })
    };
    let Some(Value::Vector(v)) = args.get("start") else {
        if let Some(other) = args.get("start") {
            return Err(fail(node, format!("start: tracks or points (a vector), not a {}", other.ty().name())));
        }
        let (Some(x), Some(y)) = (args.opt_param::<f64>("x")?, args.opt_param::<f64>("y")?) else {
            return Err(fail(
                node,
                "give the start: a vector of locations as the input start, or the settings x and y",
            ));
        };
        return Ok(vec![finish(None, x, y, direction.unwrap_or(0.0), time_set.unwrap_or(EPOCH))?]);
    };
    if v.is_empty() {
        return Err(fail(node, "start: no locations"));
    }
    let (mut px, mut py) = (Vec::with_capacity(v.len()), Vec::with_capacity(v.len()));
    for g in &v.geometries {
        match g {
            Geometry::Point(p) => {
                px.push(p[0]);
                py.push(p[1]);
            }
            _ => return Err(fail(node, "start: the locations must be points")),
        }
    }
    to_crs(&mut px, &mut py, v.crs.as_deref(), crs).map_err(e)?;
    let track = track_column(args, v).ok().map(|k| (k.clone(), super::codes(v.column(&k).unwrap())));
    let stamps = ["timestamp", "time", "datetime", "date_time", "t"]
        .iter()
        .find_map(|c| v.column(c))
        .map(Times::of)
        .transpose()
        .map_err(e)?;
    // the rows of each track in order of appearance
    let mut groups: Vec<Vec<usize>> = Vec::new();
    match &track {
        Some((_, code)) => {
            let mut seen: BTreeMap<i64, usize> = BTreeMap::new();
            for (i, c) in code.iter().enumerate() {
                let g = *seen.entry(*c).or_insert_with(|| {
                    groups.push(vec![]);
                    groups.len() - 1
                });
                groups[g].push(i);
            }
        }
        None => groups.push((0..v.len()).collect()),
    }
    let many = groups.len() > 1;
    let mut out = Vec::new();
    for (k, rows) in groups.iter().enumerate() {
        let i = rows[0];
        // the direction of the first step (anticlockwise from east), none with a single location
        let ta = match rows.get(1) {
            Some(&j) => (py[j] - py[i]).atan2(px[j] - px[i]),
            None => 0.0,
        };
        let t = match &stamps {
            Some(Times::Instants(x)) => x[i].map_or(EPOCH, Clock::Instant),
            Some(Times::Numbers(x)) => Clock::Number(x[i]),
            None => EPOCH,
        };
        let label = match (&track, many) {
            (Some((col, _)), _) => match v.column(col).unwrap() {
                Column::Texts(x) => Some(x[i].clone()),
                Column::Numbers(x) => Some(format!("{}", x[i])),
            },
            (None, true) => Some(format!("start{}", k + 1)),
            (None, false) => None,
        };
        out.push(finish(label, px[i], py[i], direction.unwrap_or(ta), t)?);
    }
    Ok(out)
}

// ----------------------------------------------------------------------------------- simulate_path

/// `move.simulate_path`.
struct SimulatePath;

impl Operator for SimulatePath {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.simulate_path",
            "Paths simulated from a fitted step selection function (amt's redistribution_kernel and simulate_path in \
             continuous space; Signer et al. 2024): from the start, n_control candidate steps are drawn from the step \
             length and turn angle distributions (the tentative ones of the model, or those connected), the \
             covariates are read at their ends (and starts), each candidate has the weight exp(b . x) of the model \
             matrix of the formula, movement terms included (importance sampling), and one is drawn with those \
             weights; each step goes on from the one before with its direction (atan2) as the direction of the step \
             before. R's generator, with the numbers the library's: the kernel's first cycle is made (and spent) \
             before the path, as the library's redistribution_kernel is, so seed s gives the path the library gives. \
             Connect the covariates as inputs of any names (forest, elev...): the terms of the model name them \
             (forest_end at the end of the candidate step, forest_start at its start); sl_, ta_, log_sl_ and \
             cos_ta_ are the step's length and turn angle. n_paths paths take a kernel each, with the seeds seed, \
             seed + 1...; every start gives its own. A path that leaves the study area (more than \
             tolerance_outside of the candidates that could be drawn end outside the covariates' extent) or has no \
             candidate with a weight stops with a warning and the rows after it have no value. In continuous \
             space only: amt's discrete mode follows GEOS's circle and biases the step lengths to the cell size.",
            Access::Global,
        )
        .input(PortSpec::new("model", Type::StepModel, "the step selection model (move.fit_issf, move.issf_model)"))
        .input(PortSpec::optional(
            "start",
            Type::Vector,
            "tracks or points: the first location of each track starts a path, with its time and the direction of \
             its first step (the settings time, direction and dt override them)",
        ))
        .input(PortSpec::optional("sl_distr", Type::StepDistribution, "step lengths to draw from, instead of the model's"))
        .input(PortSpec::optional("ta_distr", Type::StepDistribution, "turn angles to draw from, instead of the model's"))
        .output(PortSpec::new(
            "paths",
            Type::Vector,
            "the points of the paths: track (path_1, path_2...), x_, y_, t_, dt, step_ (0 the start), in the CRS of the \
             covariates",
        ))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .param(PortSpec::optional("x", Type::Number, "the start, without a start input: x"))
        .param(PortSpec::optional("y", Type::Number, "the start, without a start input: y"))
        .param(PortSpec::optional(
            "direction",
            Type::Number,
            "the direction of the step before the start, radians anticlockwise from east (default 0, or that of the \
             first step of the start's track)",
        ))
        .param(PortSpec::optional(
            "time",
            Type::Any,
            "the time of the start: a datetime in a text or a number (default 1970-01-01, or the track's)",
        ))
        .param(PortSpec::optional(
            "dt",
            Type::Number,
            "the time between steps, seconds (default one hour; 1 when the time is a number)",
        ))
        .param(PortSpec::optional("n_steps", Type::Integer, "the steps of each path (100)"))
        .param(PortSpec::optional("n_control", Type::Integer, "the candidate steps of each cycle (1000000, amt's)"))
        .param(PortSpec::optional("n_sample", Type::Integer, "the points the kernel draws in its first cycle (1)"))
        .param(PortSpec::optional(
            "tolerance_outside",
            Type::Number,
            "the fraction of the candidates that could be drawn and end outside the study area above which a path \
             stops (0)",
        ))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of R's generator (1); path i uses seed + i - 1"))
        .param(PortSpec::optional("n_paths", Type::Integer, "the paths from each start (1)"))
        .extra(Type::Any)
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let e = |m: String| fail(node, m);
        let m = model_of(args)?;
        let layers = layers_of(args, &["model", "start", "sl_distr", "ta_distr"])?;
        let crs = crate::prepare::crs_of(&layers[0].1);
        let n_steps: i64 = args.param("n_steps", 100)?;
        let n_control: i64 = args.param("n_control", 1_000_000)?;
        let n_sample: i64 = args.param("n_sample", 1)?;
        let tol: f64 = args.param("tolerance_outside", 0.0)?;
        let seed: i64 = args.param("seed", 1)?;
        let n_paths: i64 = args.param("n_paths", 1)?;
        if n_steps < 0 {
            return Err(fail(node, "n_steps: 0 or more"));
        }
        if n_control < 1 || n_sample < 1 {
            return Err(fail(node, "n_control and n_sample: 1 or more"));
        }
        if n_paths < 1 {
            return Err(fail(node, "n_paths: 1 or more"));
        }
        if !(0.0..=1.0).contains(&tol) {
            return Err(fail(node, "tolerance_outside: a fraction, from 0 to 1"));
        }
        if seed < 1 - n_paths || seed + n_paths - 1 > i64::from(i32::MAX) {
            return Err(fail(node, "seed: a whole number within 32 bits"));
        }
        let sl = distribution_of(args, "sl_distr")?.map(|d| &**d);
        let ta = distribution_of(args, "ta_distr")?.map(|d| &**d);
        let (sl, ta) = distributions(&m, sl, ta).map_err(e)?;
        let starts = starts_of(args, crs.as_deref())?;
        let spec = spec_of(&m, &layers, &sl, &ta, n_control, tol).map_err(e)?;
        let (mut track, mut xs, mut ys, mut t_text, mut t_num, mut dts, mut steps) =
            (vec![], vec![], vec![], vec![], vec![], vec![], vec![]);
        let datetimes = starts.iter().all(|s| matches!(s.t, Clock::Instant(_)));
        if !datetimes && starts.iter().any(|s| matches!(s.t, Clock::Instant(_))) {
            return Err(fail(node, "start: the times are datetimes for some starts and numbers for others"));
        }
        for s in &starts {
            for i in 1..=n_paths {
                let name = match &s.label {
                    Some(l) => format!("{l}_path_{i}"),
                    None => format!("path_{i}"),
                };
                let mut rng = RRng::new((seed + i - 1) as i32);
                let k = ffi::move_sim_kernel(rng.pin_mut(), s.x, s.y, s.ta, n_sample, &spec).map_err(|x| e(ce(x)))?;
                match k.status {
                    1 => ctx.warn(format!(
                        "node {node:?}: {name}: more than {}% of the candidate steps that could be drawn end outside \
                         the study area. Terminating simulations here.",
                        fmt_g3(100.0 * tol)
                    )),
                    2 => {
                        return Err(fail(
                            node,
                            format!(
                                "{name}: too few positive probabilities: no candidate step has a weight above 0 (the \
                                 covariates are missing at the end of all of them, or the weights overflow)"
                            ),
                        ));
                    }
                    _ => {}
                }
                let p = ffi::move_sim_path(rng.pin_mut(), s.x, s.y, s.ta, n_steps, &spec).map_err(|x| e(ce(x)))?;
                match p.status {
                    1 => ctx.warn(format!(
                        "node {node:?}: {name}: simulation stopped after {} time steps, because the animal stepped out \
                         of the landscape.",
                        p.done
                    )),
                    2 => ctx.warn(format!(
                        "node {node:?}: {name}: simulation stopped after {} time steps: no candidate step has a weight \
                         above 0.",
                        p.done
                    )),
                    _ => {}
                }
                let dt_ns = (s.dt * 1e9).round() as i64;
                for j in 0..=n_steps as usize {
                    track.push(name.clone());
                    xs.push(p.x[j]);
                    ys.push(p.y[j]);
                    steps.push(j as f64);
                    dts.push(s.dt);
                    match s.t {
                        Clock::Instant(t0) => t_text.push(time::format(t0 + dt_ns * j as i64)),
                        Clock::Number(t0) => t_num.push(t0 + s.dt * j as f64),
                    }
                }
            }
        }
        let geometries = xs
            .iter()
            .zip(&ys)
            .map(|(&x, &y)| if x.is_nan() || y.is_nan() { Geometry::Empty } else { Geometry::Point([x, y]) })
            .collect();
        let attributes = vec![
            ("track".to_string(), Column::Texts(track)),
            ("x_".to_string(), Column::Numbers(xs)),
            ("y_".to_string(), Column::Numbers(ys)),
            ("t_".to_string(), if datetimes { Column::Texts(t_text) } else { Column::Numbers(t_num) }),
            ("dt".to_string(), Column::Numbers(dts)),
            ("step_".to_string(), Column::Numbers(steps)),
        ];
        Ok(give("paths", Vector { geometries, attributes, crs }))
    }
}

/// Python's `:.3g`.
fn fmt_g3(v: f64) -> String {
    super::formula::fmt_g(v, 3)
}

/// The model of the input `model`.
fn model_of(args: &Args) -> Result<Arc<StepModel>> {
    match args.get("model") {
        Some(Value::StepModel(m)) => Ok(m.clone()),
        Some(v) => Err(fail(args.node, format!("model: a step selection model, not a {}", v.ty().name()))),
        None => Err(fail(args.node, "connect the model (move.fit_issf or move.issf_model)")),
    }
}

// ------------------------------------------------------------------------------------------ resistance

/// The columns of a model that depend on habitat only: (name, factors) of those kept, the names of the terms of
/// movement left out and of those at the start of the step.
type Habitat = (Vec<(String, Recipe)>, Vec<String>, Vec<String>);

fn habitat(m: &StepModel, names: &[String]) -> Res<Habitat> {
    let sp = &m.terms.spec;
    let (cols_names, cols) = recipe(sp)?;
    let leaf = leaf_of(names);
    // 0 end, 1 start, 2 movement: what each variable reads
    let mut kind: BTreeMap<usize, [bool; 3]> = BTreeMap::new();
    for (i, v) in sp.variables.iter().enumerate() {
        if Some(i) == sp.strata {
            continue;
        }
        let mut ks = [false; 3];
        for nm in columns_of(&v.ast) {
            let (op, _) = leaf(&nm)?;
            ks[match op {
                1 => 0,
                2 => 1,
                _ => 2,
            }] = true;
        }
        kind.insert(i, ks);
    }
    let (mut keep, mut moved, mut start) = (vec![], vec![], vec![]);
    for (nm, c) in cols_names.into_iter().zip(cols) {
        let mut ks = [false; 3];
        for (vi, _) in &c {
            for (k, on) in kind[vi].iter().enumerate() {
                ks[k] |= on;
            }
        }
        if ks[2] {
            moved.push(nm);
        } else if ks[1] {
            start.push(nm);
        } else {
            keep.push((nm, c));
        }
    }
    Ok((keep, moved, start))
}

/// A list of names as Python prints it: `['a', 'b']`.
fn py_list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", "))
}

/// How the selection becomes resistance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transform {
    Negexp,
    Linear,
    Inverse,
    Log,
}

/// What `resistance` gives: the resistance, the relative selection `exp(eta - max eta)` and the warnings.
pub struct Resisted {
    pub resistance: Vec<f64>,
    pub selection: Vec<f64>,
    pub warnings: Vec<String>,
}

/// A resistance surface from the habitat part of a step selection model (the library's `resistance`) on the cells
/// of `values` (one layer per name, one grid).
pub fn resistance(
    m: &StepModel,
    names: &[String],
    values: &[Arc<Vec<f64>>],
    transform: Transform,
    c: f64,
    r_max: f64,
    max_scale: bool,
) -> Res<Resisted> {
    if !(c.is_finite() && c > 0.0) || !(r_max.is_finite() && r_max > 1.0) {
        return Err("c: above 0; r_max: above 1".into());
    }
    let (keep, moved, start) = habitat(m, names)?;
    let mut warnings = Vec::new();
    if !moved.is_empty() || !start.is_empty() {
        let mut out = Vec::new();
        if !moved.is_empty() {
            out.push(format!("the terms of movement {}", py_list(&moved)));
        }
        if !start.is_empty() {
            out.push(format!("the terms at the start of the step {}", py_list(&start)));
        }
        warnings.push(format!("resistance: left out {}: a cell has no step", out.join(" and ")));
    }
    if keep.is_empty() {
        return Err("the model has no term of habitat left".into());
    }
    let n = values[0].len();
    let mut df = Frame::default();
    for (nm, v) in names.iter().zip(values) {
        df.push(nm, Col::Num((**v).clone()));
        df.push(&format!("{nm}_end"), Col::Num((**v).clone()));
    }
    let sp = &m.terms.spec;
    let mut vals: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    let mut eta = vec![0.0f64; n];
    for (nm, cols) in &keep {
        let at = m.terms.columns.iter().position(|c| c == nm).unwrap();
        let b = m.coef[at];
        if b.is_nan() {
            continue;
        }
        let mut col: Option<Vec<f64>> = None;
        for &(vi, lv) in cols {
            let v = &sp.variables[vi];
            if let std::collections::btree_map::Entry::Vacant(slot) = vals.entry(vi) {
                let x = if v.kind == Kind::Num {
                    arith(&v.ast, &df, n)?
                } else {
                    let inner = v.inner.as_ref().ok_or("formula: factor() takes one argument")?;
                    codes_of(&raw_of(inner, &df, n)?, &v.levels, &v.label)?.into_iter().map(|c| c as f64).collect()
                };
                slot.insert(x);
            }
            let x = &vals[&vi];
            let x: Vec<f64> = if lv < 0 {
                x.clone()
            } else {
                x.iter().map(|&c| if c < 0.0 { f64::NAN } else { f64::from(u8::from(c == lv as f64)) }).collect()
            };
            col = Some(match col {
                None => x,
                Some(p) => p.iter().zip(&x).map(|(a, b)| a * b).collect(),
            });
        }
        let col = col.unwrap();
        for (e, x) in eta.iter_mut().zip(&col) {
            *e += x * b;
        }
    }
    let bad: Vec<bool> = eta.iter().map(|e| e.is_nan()).collect();
    let emax =
        eta.iter().copied().filter(|e| !e.is_nan()).fold(f64::NAN, |a, b| if a.is_nan() || b > a { b } else { a });
    // w / max(w) = exp(eta - max eta): no overflow, the scale cancels in every transformation but "inverse"
    let wr: Vec<f64> = eta.iter().map(|&e| (e - emax).exp()).collect();
    let r: Vec<f64> = match transform {
        Transform::Inverse => eta.iter().map(|&e| (-e).exp()).collect(),
        Transform::Log => eta.iter().map(|&e| 1.0 + c * (emax - e)).collect(),
        Transform::Negexp | Transform::Linear => {
            let wmin = wr
                .iter()
                .copied()
                .filter(|e| !e.is_nan())
                .fold(f64::NAN, |a, b| if a.is_nan() || b < a { b } else { a });
            let h: Vec<f64> = if max_scale {
                wr.clone()
            } else if wmin < 1.0 {
                wr.iter().map(|w| (w - wmin) / (1.0 - wmin)).collect()
            } else {
                vec![1.0; n]
            };
            if transform == Transform::Linear {
                h.iter().map(|h| r_max - (r_max - 1.0) * h).collect()
            } else {
                h.iter().map(|h| r_max - (r_max - 1.0) * (1.0 - (-c * h).exp()) / (1.0 - (-c).exp())).collect()
            }
        }
    };
    let mask =
        |v: Vec<f64>| -> Vec<f64> { v.into_iter().zip(&bad).map(|(x, &b)| if b { f64::NAN } else { x }).collect() };
    Ok(Resisted { resistance: mask(r), selection: mask(wr), warnings })
}

/// numpy's allclose (rtol 1e-5, atol 1e-8) of two grids' numbers.
fn close(a: &[f64], b: &[f64]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() <= 1e-8 + 1e-5 * y.abs())
}

/// `move.resistance`.
struct Resistance;

impl Operator for Resistance {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.resistance",
            "A resistance surface from the habitat part of a step selection model: the relative selection w = \
             exp(sum(b_k x_k)) of each cell from the covariates' terms alone (the terms of movement, sl_, log_sl_, \
             cos_ta_ and ta_, the interactions with them and the covariates at the start of the step are left out, \
             with a warning that says which: a cell has no step), turned into resistance (high where the animal \
             avoids), ready for cost.transition or the circuit models. With h the selection scaled to [0, 1] \
             (scale minmax: (w - min w) / (max w - min w); max: w / max w): negexp, r_max - (r_max - 1) (1 - exp(-c \
             h)) / (1 - exp(-c)), the negative exponential of habitat suitability Keeley et al. (2016) recommend, 1 \
             for the best cell and r_max for the worst; linear, r_max - (r_max - 1) h; inverse, 1 / w, not scaled \
             (Zeller et al. 2012 list it among the ways of turning a selection into a resistance); log, 1 + c (log \
             max w - log w). The covariates are connected as inputs of any names, on one grid. Cells where a \
             covariate of the kept terms has no value have none.",
            Access::Global,
        )
        .input(PortSpec::new("model", Type::StepModel, "the step selection model (move.fit_issf, move.issf_model)"))
        .output(PortSpec::new("resistance", Type::Map, "the resistance, on the grid of the covariates"))
        .output(PortSpec::new(
            "selection",
            Type::Map,
            "the relative selection used, exp(eta - max eta): 1 for the best cell",
        ))
        .param(
            PortSpec::optional("transform", Type::Text, "negexp (default), linear, inverse or log")
                .choices(["negexp", "linear", "inverse", "log"]),
        )
        .param(PortSpec::optional("c", Type::Number, "the shape of negexp, the slope of log (8)"))
        .param(PortSpec::optional("r_max", Type::Number, "the resistance of the least selected cell (100)"))
        .param(PortSpec::optional("scale", Type::Text, "minmax (default) or max").choices(["minmax", "max"]))
        .extra(Type::Any)
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let e = |m: String| fail(node, m);
        let m = model_of(args)?;
        let layers = layers_of(args, &["model"])?;
        let transform = match args.param("transform", "negexp".to_string())?.as_str() {
            "negexp" | "" => Transform::Negexp,
            "linear" => Transform::Linear,
            "inverse" => Transform::Inverse,
            "log" => Transform::Log,
            _ => return Err(fail(node, "transform: negexp, linear, inverse or log")),
        };
        let max_scale = match args.param("scale", "minmax".to_string())?.as_str() {
            "minmax" | "" => false,
            "max" => true,
            _ => return Err(fail(node, "scale: minmax or max")),
        };
        let (c, r_max): (f64, f64) = (args.param("c", 8.0)?, args.param("r_max", 100.0)?);
        if !(c.is_finite() && c > 0.0) || !(r_max.is_finite() && r_max > 1.0) {
            return Err(fail(node, "c: above 0; r_max: above 1"));
        }
        let g0 = layers[0].1.clone();
        let (a0, d0) = (grid_of(&g0), [g0.height, g0.width]);
        for (name, g, v) in &layers {
            let a = grid_of(g);
            let same = [g.height, g.width] == d0
                && v.len() == layers[0].2.len()
                && close(&[a.0, a.1, a.2, a.3, a.4, a.5], &[a0.0, a0.1, a0.2, a0.3, a0.4, a0.5]);
            if !same {
                return Err(fail(node, format!("covariates: the rasters must share one grid ({name} does not)")));
            }
        }
        let names: Vec<String> = layers.iter().map(|l| l.0.clone()).collect();
        let values: Vec<Arc<Vec<f64>>> = layers.iter().map(|l| l.2.clone()).collect();
        let r = resistance(&m, &names, &values, transform, c, r_max, max_scale).map_err(e)?;
        for w in r.warnings {
            ctx.warn(format!("node {node:?}: {w}"));
        }
        Ok(outputs([
            ("resistance", Value::Map(Map::new(g0.clone(), r.resistance))),
            ("selection", Value::Map(Map::new(g0, r.selection))),
        ]))
    }
}

// ---------------------------------------------------------------------------------------- path density

/// The share of the positions of `paths` in each cell of `like` (NaN where `like` is, 0 where none fell).
fn share(x: &[f64], y: &[f64], like: &Map, ok: &[bool]) -> Res<Vec<f64>> {
    let cells = grid_cells(x, y, &like.grid);
    let mut counts = vec![0.0f64; like.data.len()];
    for &c in &cells {
        if c >= 0 && ok[c as usize] {
            counts[c as usize] += 1.0;
        }
    }
    let total: f64 = counts.iter().sum();
    if total == 0.0 {
        return Err("no position of the paths falls on a cell of like".into());
    }
    Ok(counts.iter().zip(ok).map(|(c, &ok)| if ok { c / total } else { f64::NAN }).collect())
}

/// `move.path_density`.
struct PathDensity;

impl Operator for PathDensity {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.path_density",
            "The use of a landscape by simulated paths: the share of positions in each cell. The end points of the \
             steps of one or many paths (what move.simulate_path gives) are counted in the cells of like (terra's \
             cellFromXY rule: a point on the edge between two cells is in the cell to its right / below) and divided \
             by their total, so the cells sum to 1. It is the utilisation distribution of steady state of Signer et \
             al. (2024) (the step end points of long paths, from several starts if need be) without smoothing: \
             smooth the map yourself if you want a kernel density estimate. 0 where no position fell, none where \
             like has none (those cells take no positions and are not in the total). With by, one band per value of \
             a column (the track, say), each summing to 1.",
            Access::Global,
        )
        .input(PortSpec::new("paths", Type::Vector, "the paths: points with x_ and y_ (move.simulate_path)"))
        .input(PortSpec::new("like", Type::Map, "a raster whose grid is the result's"))
        .output(PortSpec::new("density", Type::Any, "the shares: a map, or a stack with one band per value of by"))
        .param(PortSpec::optional(
            "include_start",
            Type::Bool,
            "count the first position of each path (step_ 0, the start), which is not the end of a step (false)",
        ))
        .param(PortSpec::optional(
            "by",
            Type::Text,
            "a column that names the paths (track): one band per value",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let e = |m: String| fail(node, m);
        let v = super::tracks_of(args, "paths")?;
        let like = args.map("like")?;
        let include_start: bool = args.param("include_start", false)?;
        let by = args.opt_param::<String>("by")?.filter(|b| !b.is_empty());
        let (mut x, mut y) = match (v.column("x_"), v.column("y_")) {
            (Some(x), Some(y)) => (x.numbers(), y.numbers()),
            _ => {
                return Err(fail(node, "paths: a table with the columns x_ and y_ (the paths of move.simulate_path)"));
            }
        };
        to_crs(&mut x, &mut y, v.crs.as_deref(), crate::prepare::crs_of(&like.grid).as_deref()).map_err(e)?;
        let by_col = match &by {
            Some(b) => Some(v.column(b).ok_or_else(|| fail(node, format!("by: the column {b:?} is not in paths")))?),
            None => None,
        };
        let rows: Vec<usize> = match v.column("step_") {
            Some(s) if !include_start => {
                let s = s.numbers();
                (0..v.len()).filter(|&i| s[i] != 0.0).collect()
            }
            _ => (0..v.len()).collect(),
        };
        let ok: Vec<bool> = like.data.iter().map(|v| !v.is_nan()).collect();
        let pick = |rows: &[usize]| -> (Vec<f64>, Vec<f64>) {
            (rows.iter().map(|&i| x[i]).collect(), rows.iter().map(|&i| y[i]).collect())
        };
        let Some(col) = by_col else {
            let (px, py) = pick(&rows);
            let d = share(&px, &py, like, &ok).map_err(e)?;
            return Ok(outputs([("density", Value::Map(Map::new(like.grid.clone(), d)))]));
        };
        // the values of the column in the order pandas' groupby sorts them (missing ones left out)
        let mut groups: Vec<(super::Key, String, Vec<usize>)> = Vec::new();
        for &i in &rows {
            let k = super::Key::of(col, i);
            if k == super::Key::Na {
                continue;
            }
            match groups.iter_mut().find(|g| g.0 == k) {
                Some(g) => g.2.push(i),
                None => {
                    let label = match col {
                        Column::Texts(t) => t[i].clone(),
                        Column::Numbers(n) => format!("{}", n[i]),
                    };
                    groups.push((k, label, vec![i]));
                }
            }
        }
        groups.sort_by(|a, b| a.0.cmp(&b.0));
        if groups.is_empty() {
            return Err(fail(node, "no position of the paths falls on a cell of like"));
        }
        let mut layers = Vec::new();
        let mut keys = Vec::new();
        for (_, label, r) in &groups {
            let (px, py) = pick(r);
            layers.push(Arc::new(share(&px, &py, like, &ok).map_err(e)?));
            keys.push(crate::Key::Name(label.clone()));
        }
        let stack = Stack { grid: like.grid.clone(), keys, layers };
        Ok(outputs([("density", Value::Stack(stack))]))
    }
}
