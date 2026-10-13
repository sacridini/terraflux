//! Connectivity by circuits, through landschaft's C++ of Circuitscape 6.1 and Omniscape 0.7: Omniscape's
//! omnidirectional connectivity (moving-window circuits: the cumulative current, the flow potential and the
//! current normalised by it) and Circuitscape's advanced mode on a raster (currents and voltages from sources
//! and grounds). landschaft's Python around the kernels (the sources from the resistance, the threshold and
//! cutoff, the reclassification, the conditions, single precision, the no-data mask) is here, step for step.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Omniscape));
    r.add(Arc::new(Advanced));
}

/// Omniscape's Int64(round(parse(Float64, value))): halves to even.
fn whole(v: f64, name: &str) -> Result<i32> {
    if !v.is_finite() {
        return Err(Error::msg(format!("{name} must be finite")));
    }
    Ok(v.round_ties_even() as i32)
}

/// Through single precision (and back), as numpy's astype(float32).astype(float64).
fn single(v: &mut [f64]) {
    for x in v.iter_mut() {
        *x = f64::from(*x as f32);
    }
}

/// Omniscape's reclassify_resistance!: each (value, new value) applied in turn, matched against the original
/// values (NaN: no data); values not in the table kept.
pub fn reclassify(r: &[f64], table: &[(f64, f64)]) -> Vec<f64> {
    let mut out = r.to_vec();
    for &(old, new) in table {
        for (o, &v) in out.iter_mut().zip(r) {
            if v == old {
                *o = new;
            }
        }
    }
    out
}

/// Omniscape's source_from_resistance: the conductance (1 / resistance, or the values themselves) after the
/// reclassification, 0 below 1 / r_cutoff and on no data.
pub fn source_from_resistance(r: &[f64], r_cutoff: f64, conductance: bool, table: Option<&[(f64, f64)]>) -> Vec<f64> {
    let r = match table {
        Some(t) => reclassify(r, t),
        None => r.to_vec(),
    };
    let least = 1.0 / r_cutoff;
    r.into_iter()
        .map(|v| {
            let s = if conductance { v } else { 1.0 / v };
            if s.is_nan() || s < least { 0.0 } else { s }
        })
        .collect()
}

/// A reclassification table of a setting: [[value, new value], ...] (null: no data).
fn table_of(args: &Args) -> Result<Option<Vec<(f64, f64)>>> {
    match args.params.get("reclass_table") {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Array(rows)) => rows
            .iter()
            .map(|r| {
                let r = r.as_array().filter(|r| r.len() == 2).ok_or_else(|| {
                    Error::msg(format!("node {:?}: reclass_table rows are [value, new value]", args.node))
                })?;
                Ok((r[0].as_f64().unwrap_or(f64::NAN), r[1].as_f64().unwrap_or(f64::NAN)))
            })
            .collect::<Result<Vec<_>>>()
            .map(Some),
        Some(v) => Err(Error::msg(format!("node {:?}: reclass_table: [[value, new value], ...], not {v}", args.node))),
    }
}

/// The maps of Omniscape's run: the cumulative current, the flow potential and the normalised current (those
/// asked for), and the targets solved.
pub struct OmniscapeMaps {
    pub current: Vec<f64>,
    pub flow_potential: Option<Vec<f64>>,
    pub normalized: Option<Vec<f64>>,
    pub n_targets: i64,
}

/// The settings of Omniscape (landschaft's omniscape's arguments).
#[derive(Debug, Clone)]
pub struct OmniscapeSettings {
    pub radius: f64,
    pub block_size: f64,
    pub buffer: f64,
    pub source_threshold: f64,
    pub r_cutoff: f64,
    pub resistance_is_conductance: bool,
    pub four_neighbors: bool,
    pub flow_potential: bool,
    pub normalized_current: bool,
    pub correct_artifacts: bool,
    pub mask_nodata: bool,
    pub reclass_table: Option<Vec<(f64, f64)>>,
    /// "none", "1", "2" or "both".
    pub compare_to_future: String,
    pub comparisons: [String; 2],
    pub bounds: [(f64, f64); 2],
    pub single: bool,
}

impl Default for OmniscapeSettings {
    fn default() -> OmniscapeSettings {
        OmniscapeSettings {
            radius: 0.0,
            block_size: 1.0,
            buffer: 0.0,
            source_threshold: 0.0,
            r_cutoff: f64::INFINITY,
            resistance_is_conductance: false,
            four_neighbors: false,
            flow_potential: false,
            normalized_current: false,
            correct_artifacts: true,
            mask_nodata: true,
            reclass_table: None,
            compare_to_future: "none".into(),
            comparisons: ["within".into(), "within".into()],
            bounds: [(0.0, 0.0), (0.0, 0.0)],
            single: false,
        }
    }
}

/// The conditions of a run: the present and future values of condition 1 and 2.
#[derive(Default)]
pub struct Conditions<'a> {
    pub condition1: Option<&'a [f64]>,
    pub condition1_future: Option<&'a [f64]>,
    pub condition2: Option<&'a [f64]>,
    pub condition2_future: Option<&'a [f64]>,
}

/// landschaft's omniscape: the resistance (ny x nx, NaN no data) and the sources (none: from the resistance).
pub fn omniscape(
    resistance: &[f64],
    source: Option<&[f64]>,
    nx: usize,
    ny: usize,
    st: &OmniscapeSettings,
    c: &Conditions,
) -> Result<OmniscapeMaps> {
    let mut r = resistance.to_vec();
    if st.single {
        single(&mut r);
    }
    if r.iter().any(|v| !v.is_nan() && *v <= 0.0) {
        return Err(Error::msg("resistance (or conductance) values must be > 0"));
    }
    let radius = whole(st.radius, "radius")?;
    let buffer = whole(st.buffer, "buffer")?;
    let mut block = whole(st.block_size, "block_size")?;
    if radius < 0 || buffer < 0 || block < 1 {
        return Err(Error::msg("radius and buffer must be >= 0 and block_size >= 1"));
    }
    // (an even block: the next odd one, as Omniscape with its warning)
    if block % 2 == 0 {
        block += 1;
    }
    let s: Vec<f64> = match source {
        None => source_from_resistance(&r, st.r_cutoff, st.resistance_is_conductance, st.reclass_table.as_deref()),
        Some(src) => {
            if src.len() != r.len() {
                return Err(Error::msg("the source raster must have the resistance's shape"));
            }
            let mut s = src.to_vec();
            if st.single {
                single(&mut s);
            }
            s.into_iter()
                .map(|v| if v.is_nan() { 0.0 } else { v })
                .map(|v| if v < st.source_threshold { 0.0 } else { v })
                .collect()
        }
    };
    if let Some(t) = &st.reclass_table {
        r = reclassify(&r, t);
        if st.single {
            single(&mut r);
        }
        if r.iter().any(|v| !v.is_nan() && *v <= 0.0) {
            return Err(Error::msg("the reclassified resistance must be > 0"));
        }
    }
    // the conditions
    let cmp = &st.compare_to_future;
    if c.condition2.is_some() && c.condition1.is_none() {
        return Err(Error::msg("condition2 needs condition1"));
    }
    let mut keep: Vec<Vec<f64>> = Vec::new();
    let mut take = |v: Option<&[f64]>, name: &str| -> Result<Option<usize>> {
        match v {
            None => Ok(None),
            Some(v) if v.len() != r.len() => Err(Error::msg(format!("{name} must have the resistance's shape"))),
            Some(v) => {
                let mut v = v.to_vec();
                if st.single {
                    single(&mut v);
                }
                keep.push(v);
                Ok(Some(keep.len() - 1))
            }
        }
    };
    let (c1, c1f, c2, c2f) = if c.condition1.is_some() {
        if !["none", "1", "2", "both"].contains(&cmp.as_str()) {
            return Err(Error::msg("compare_to_future: none, 1, 2 or both"));
        }
        for (k, m) in st.comparisons.iter().enumerate() {
            if m != "within" && m != "equal" {
                return Err(Error::msg(format!("comparison{}: within or equal", k + 1)));
            }
        }
        let (fut1, fut2) = (["1", "both"].contains(&cmp.as_str()), ["2", "both"].contains(&cmp.as_str()));
        if fut1 && c.condition1_future.is_none() {
            return Err(Error::msg("compare_to_future needs condition1_future"));
        }
        if fut2 && c.condition2.is_some() && c.condition2_future.is_none() {
            return Err(Error::msg("compare_to_future needs condition2_future"));
        }
        (
            take(c.condition1, "condition1")?,
            take(if fut1 { c.condition1_future } else { None }, "condition1_future")?,
            take(c.condition2, "condition2")?,
            take(if fut2 { c.condition2_future } else { None }, "condition2_future")?,
        )
    } else {
        if c.condition1_future.is_some() || c.condition2_future.is_some() {
            return Err(Error::msg("future conditions need condition1"));
        }
        (None, None, None, None)
    };
    let slice = |k: Option<usize>| -> &[f64] { k.map_or(&[][..], |i| keep[i].as_slice()) };
    let comparisons = [
        if st.comparisons[0] == "equal" { 1.0 } else { 0.0 },
        st.bounds[0].0,
        st.bounds[0].1,
        if st.comparisons[1] == "equal" { 1.0 } else { 0.0 },
        st.bounds[1].0,
        st.bounds[1].1,
    ];
    let compute_fp = st.flow_potential || st.normalized_current;
    let out = ffi::circuit_omniscape(
        &r,
        &s,
        nx as i64,
        ny as i64,
        radius,
        buffer,
        block,
        st.four_neighbors,
        st.resistance_is_conductance,
        st.correct_artifacts,
        compute_fp,
        slice(c1),
        slice(c1f),
        slice(c2),
        slice(c2f),
        &comparisons,
        0.0,
    )?;
    let mut cum = out.current;
    let mut fp = compute_fp.then_some(out.flow_potential);
    let mut norm = match (&fp, st.normalized_current) {
        (Some(f), true) => Some(
            cum.iter()
                .zip(f)
                .map(|(a, b)| {
                    let v = a / b;
                    if v.is_nan() { 0.0 } else { v }
                })
                .collect::<Vec<f64>>(),
        ),
        _ => None,
    };
    if !st.flow_potential {
        fp = None;
    }
    let mask = |v: &mut Vec<f64>| {
        if st.mask_nodata {
            for (x, rv) in v.iter_mut().zip(&r) {
                if rv.is_nan() {
                    *x = f64::NAN;
                }
            }
        }
        if st.single {
            single(v);
        }
    };
    mask(&mut cum);
    if let Some(f) = &mut fp {
        mask(f);
    }
    if let Some(n) = &mut norm {
        mask(n);
    }
    Ok(OmniscapeMaps { current: cum, flow_potential: fp, normalized: norm, n_targets: out.n_targets })
}

struct Omniscape;

impl Operator for Omniscape {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "circuit.omniscape",
            "Omnidirectional connectivity with Omniscape's moving-window circuits (Omniscape 0.7 through \
             landschaft's C++): a target every block_size cells injects its block's source strength into the \
             sources within radius, grounded at the target; the node currents of every window add up into the \
             cumulative current. The flow potential (the same on a conductance of 1) and the current normalised by \
             it (above 1: channelled; below: impeded) when asked. Sources from the resistance (its conductance) \
             unless given; conditions keep the sources whose values match the target's.",
            Access::Global,
        )
        .input(PortSpec::new("resistance", Type::Map, "the resistance (or conductance), > 0; no value: no data"))
        .input(PortSpec::optional("source", Type::Map, "the source strengths (default: from the resistance)"))
        .input(PortSpec::optional("condition1", Type::Map, "conditional connectivity: the first condition"))
        .input(PortSpec::optional("condition1_future", Type::Map, "its future values (compare_to_future)"))
        .input(PortSpec::optional("condition2", Type::Map, "a second condition"))
        .input(PortSpec::optional("condition2_future", Type::Map, "its future values"))
        .output(PortSpec::new("current", Type::Map, "the cumulative current (cum_currmap)"))
        .output(PortSpec::new("flow_potential", Type::Map, "the flow potential (flow_potential: true)"))
        .output(PortSpec::new("normalized", Type::Map, "the current over the flow potential (normalized_current)"))
        .param(PortSpec::new("radius", Type::Number, "the window's radius, in cells"))
        .param(PortSpec::optional("block_size", Type::Number, "targets every this many cells, odd (1)"))
        .param(PortSpec::optional("buffer", Type::Number, "cells beyond the radius in each window (0)"))
        .param(PortSpec::optional("source_threshold", Type::Number, "sources given: below this none (0)"))
        .param(PortSpec::optional("r_cutoff", Type::Number, "sources from the resistance: none above it (inf)"))
        .param(PortSpec::optional("resistance_is_conductance", Type::Bool, "the map holds conductances"))
        .param(PortSpec::optional("four_neighbors", Type::Bool, "the four orthogonal neighbours only"))
        .param(PortSpec::optional("flow_potential", Type::Bool, "the flow potential too"))
        .param(PortSpec::optional("normalized_current", Type::Bool, "the normalised current too"))
        .param(PortSpec::optional("correct_artifacts", Type::Bool, "undo the blocks' artifacts (true)"))
        .param(PortSpec::optional("mask_nodata", Type::Bool, "no value where the resistance has none (true)"))
        .param(PortSpec::optional("reclass_table", Type::Any, "[[value, new value], ...] (null: no data)"))
        .param(
            PortSpec::optional("compare_to_future", Type::Text, "none (default), 1, 2 or both")
                .choices(["none", "1", "2", "both"]),
        )
        .param(PortSpec::optional("comparison1", Type::Text, "within (default) or equal").choices(["within", "equal"]))
        .param(PortSpec::optional("comparison2", Type::Text, "within (default) or equal").choices(["within", "equal"]))
        .param(PortSpec::optional("condition1_lower", Type::Number, "within: the least difference (0)"))
        .param(PortSpec::optional("condition1_upper", Type::Number, "within: the largest difference (0)"))
        .param(PortSpec::optional("condition2_lower", Type::Number, "within: the least difference (0)"))
        .param(PortSpec::optional("condition2_upper", Type::Number, "within: the largest difference (0)"))
        .param(PortSpec::optional("precision", Type::Text, "double (default) or single").choices(["double", "single"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let r = args.map("resistance")?;
        let d = OmniscapeSettings::default();
        let precision: String = args.param("precision", "double".to_string())?;
        if precision != "double" && precision != "single" {
            return Err(fail(Error::msg("precision: double or single")));
        }
        let st = OmniscapeSettings {
            radius: args.param("radius", f64::NAN)?,
            block_size: args.param("block_size", d.block_size)?,
            buffer: args.param("buffer", d.buffer)?,
            source_threshold: args.param("source_threshold", d.source_threshold)?,
            r_cutoff: args.opt_param("r_cutoff")?.unwrap_or(d.r_cutoff),
            resistance_is_conductance: args.param("resistance_is_conductance", false)?,
            four_neighbors: args.param("four_neighbors", false)?,
            flow_potential: args.param("flow_potential", false)?,
            normalized_current: args.param("normalized_current", false)?,
            correct_artifacts: args.param("correct_artifacts", true)?,
            mask_nodata: args.param("mask_nodata", true)?,
            reclass_table: table_of(args)?,
            compare_to_future: args.param("compare_to_future", "none".to_string())?,
            comparisons: [
                args.param("comparison1", "within".to_string())?,
                args.param("comparison2", "within".to_string())?,
            ],
            bounds: [
                (args.param("condition1_lower", 0.0)?, args.param("condition1_upper", 0.0)?),
                (args.param("condition2_lower", 0.0)?, args.param("condition2_upper", 0.0)?),
            ],
            single: precision == "single",
        };
        let opt = |p: &str| -> Result<Option<&[f64]>> { Ok(args.opt_map(p)?.map(|m| m.data.as_slice())) };
        let c = Conditions {
            condition1: opt("condition1")?,
            condition1_future: opt("condition1_future")?,
            condition2: opt("condition2")?,
            condition2_future: opt("condition2_future")?,
        };
        let (ny, nx) = r.shape();
        let source = opt("source")?;
        let m = omniscape(&r.data, source, nx, ny, &st, &c).map_err(fail)?;
        let map = |v: Vec<f64>| Value::Map(Map::new(r.grid.clone(), v));
        let mut out = Outputs::new();
        out.insert("current".into(), map(m.current));
        if let Some(f) = m.flow_potential {
            out.insert("flow_potential".into(), map(f));
        }
        if let Some(n) = m.normalized {
            out.insert("normalized".into(), map(n));
        }
        Ok(out)
    }
}

/// landschaft's circuit.advanced: the conductance (NaN: no node), the sources (amps) and the grounds
/// (conductance to ground, +inf direct); NaN in the sources and grounds is nothing.
pub fn advanced(
    conductance: &[f64],
    source: &[f64],
    ground: &[f64],
    nx: usize,
    ny: usize,
    four: bool,
) -> Result<ffi::CircuitOut> {
    if source.len() != conductance.len() || ground.len() != conductance.len() {
        return Err(Error::msg("conductance, source and ground must have the same shape"));
    }
    if conductance.contains(&f64::INFINITY) {
        return Err(Error::msg("conductances must be finite"));
    }
    if ground.iter().any(|v| v.is_finite() && *v < 0.0) {
        return Err(Error::msg("ground conductances cannot be negative"));
    }
    let nan0 = |v: &[f64]| v.iter().map(|x| if x.is_nan() { 0.0 } else { *x }).collect::<Vec<f64>>();
    Ok(ffi::circuit_advanced(&nan0(conductance), &nan0(source), &nan0(ground), nx as i64, ny as i64, four)?)
}

struct Advanced;

impl Operator for Advanced {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "circuit.advanced",
            "Currents and voltages of a raster circuit with sources and grounds, Circuitscape 6.1's advanced mode \
             through landschaft's C++: each cell with a conductance a node joined to its 8 neighbours (4 asked) by \
             the average conductance; current injected at the sources, drawn at the grounds (a conductance to \
             ground, or a direct ground: inf); every component with both solved exactly. The node current is the \
             larger of a cell's inflow and outflow.",
            Access::Global,
        )
        .input(PortSpec::new("conductance", Type::Map, "the conductance of each cell (no value: no node)"))
        .input(PortSpec::new("source", Type::Map, "the current injected at each cell, amps"))
        .input(PortSpec::new("ground", Type::Map, "the conductance to ground of each cell (inf: direct)"))
        .output(PortSpec::new("current", Type::Map, "the node current of each cell"))
        .output(PortSpec::new("voltage", Type::Map, "the voltage of each cell"))
        .param(PortSpec::optional("four_neighbors", Type::Bool, "the four orthogonal neighbours only"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let g = args.map("conductance")?;
        let (ny, nx) = g.shape();
        let out = advanced(
            &g.data,
            &args.map("source")?.data,
            &args.map("ground")?.data,
            nx,
            ny,
            args.param("four_neighbors", false)?,
        )
        .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let map = |v: Vec<f64>| Value::Map(Map::new(g.grid.clone(), v));
        Ok(crate::outputs([("current", map(out.current)), ("voltage", map(out.voltage))]))
    }
}
