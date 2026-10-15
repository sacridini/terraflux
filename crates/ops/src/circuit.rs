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
    r.add(Arc::new(Pairwise));
    r.add(Arc::new(OneToAll));
    r.add(Arc::new(NetworkAdvanced));
    r.add(Arc::new(NetworkPairwise));
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
        .output(PortSpec::optional("flow_potential", Type::Map, "the flow potential (flow_potential: true)"))
        .output(PortSpec::optional("normalized", Type::Map, "the current over the flow potential (normalized_current)"))
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

/// Circuitscape's no-data value (its ASCII grids'): the cells without a value of its rasters.
const NODATA: f64 = -9999.0;

/// A raster as Circuitscape reads it: no value as -9999.
fn cells_of(v: &[f64]) -> Vec<f64> {
    v.iter().map(|x| if x.is_nan() { NODATA } else { *x }).collect()
}

/// Circuitscape's read_cellmap: reclassified (a lookup on the values read), then 1 / resistance (or the
/// conductance), the -9999 cells at 0.
pub fn habitat_conductance(cells: &[f64], is_resistance: bool, reclass: Option<&[(f64, f64)]>) -> Result<Vec<f64>> {
    let mut c = cells.to_vec();
    if let Some(t) = reclass {
        for v in c.iter_mut() {
            if let Some((_, new)) = t.iter().find(|(old, _)| old == v) {
                *v = *new;
            }
        }
    }
    if is_resistance && c.contains(&0.0) {
        return Err(Error::msg(
            "zero resistance values are not supported for habitat maps; use a short-circuit region file instead",
        ));
    }
    Ok(c.iter()
        .map(|&v| {
            if v == NODATA {
                0.0
            } else if is_resistance {
                1.0 / v
            } else {
                v
            }
        })
        .collect())
}

/// Focal points as Circuitscape reads a raster of ids: the cells in column-major order with an id of 0 or more,
/// sorted by id (stably): their 0-based rows, columns and ids.
pub struct FocalPoints {
    pub rows: Vec<usize>,
    pub cols: Vec<usize>,
    pub ids: Vec<i64>,
}

impl FocalPoints {
    pub fn of(map: &[f64], nx: usize, ny: usize) -> FocalPoints {
        let pm: Vec<f64> = cells_of(map).into_iter().map(|v| if v == NODATA { 0.0 } else { v }).collect();
        let mut found: Vec<(usize, usize, f64)> = Vec::new();
        for j in 0..nx {
            for i in 0..ny {
                let v = pm[i * nx + j];
                if v > 0.0 {
                    found.push((i, j, v));
                }
            }
        }
        found.sort_by(|a, b| a.2.total_cmp(&b.2));
        FocalPoints {
            rows: found.iter().map(|f| f.0).collect(),
            cols: found.iter().map(|f| f.1).collect(),
            ids: found.iter().map(|f| f.2 as i64).collect(),
        }
    }

    /// Only the focal points of the given ids.
    pub fn retain(&mut self, ids: &[i64]) {
        let keep: Vec<bool> = self.ids.iter().map(|i| ids.contains(i)).collect();
        let pick = |v: &[usize]| v.iter().zip(&keep).filter(|(_, k)| **k).map(|(x, _)| *x).collect::<Vec<_>>();
        self.rows = pick(&self.rows);
        self.cols = pick(&self.cols);
        self.ids = self.ids.iter().zip(&keep).filter(|(_, k)| **k).map(|(x, _)| *x).collect();
    }

    /// The ids, once each, increasing.
    pub fn unique(&self) -> Vec<i64> {
        let mut u = self.ids.clone();
        u.sort_unstable();
        u.dedup();
        u
    }

    /// Whether an id covers several cells (focal regions).
    pub fn has_regions(&self) -> bool {
        self.unique().len() != self.ids.len()
    }
}

/// The inputs of Circuitscape's raster modes: the conductances, the short-circuit regions (empty: none), the
/// focal points.
/// A raster circuit: the conductances, the short-circuit regions, the focal points, the size.
pub struct Circuit {
    pub g: Vec<f64>,
    pub poly: Vec<i64>,
    pub points: FocalPoints,
    pub nx: usize,
    pub ny: usize,
}

impl Circuit {
    fn of(args: &Args) -> Result<(Circuit, Arc<terraflux_raster::Grid>)> {
        let fail = |e: String| Error::msg(format!("node {:?}: {e}", args.node));
        let h = args.map("habitat")?;
        let (ny, nx) = h.shape();
        let shaped = |m: &Map, name: &str| -> Result<Vec<f64>> {
            if m.data.len() != h.data.len() {
                return Err(fail(format!("the {name} must have the habitat's shape")));
            }
            Ok(cells_of(&m.data).into_iter().map(|v| if v == NODATA { 0.0 } else { v }).collect())
        };
        let mut g = habitat_conductance(
            &cells_of(&h.data),
            args.param("habitat_is_resistance", true)?,
            table_of(args)?.as_deref(),
        )
        .map_err(|e| fail(e.to_string()))?;
        let poly: Vec<i64> = match args.opt_map("polygons")? {
            Some(p) => {
                let p = shaped(p, "polygons")?;
                if p.iter().any(|v| *v != v.round()) {
                    return Err(fail("the polygon raster must hold integer ids".into()));
                }
                p.into_iter().map(|v| v as i64).collect()
            }
            None => Vec::new(),
        };
        if let Some(m) = args.opt_map("mask")? {
            let m = shaped(m, "mask")?;
            for (x, k) in g.iter_mut().zip(&m) {
                *x *= if *k > 0.0 { 1.0 } else { 0.0 };
            }
            if g.iter().sum::<f64>() == 0.0 {
                return Err(fail("Mask file deleted everything!".into()));
            }
        }
        let points = FocalPoints::of(&args.map("points")?.data, nx, ny);
        if points.unique().len() < 2 {
            return Err(fail("less than two focal points".into()));
        }
        Ok((Circuit { g, poly, points, nx, ny }, h.grid.clone()))
    }
}

/// The settings of the raster modes shared by pairwise and one-to-all.
fn raster_mode(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new("habitat", Type::Map, "the resistance (or conductance); no value: no node"))
        .input(PortSpec::new(
            "points",
            Type::Map,
            "the focal points' ids (0 or no value elsewhere; cells of one id: a region)",
        ))
        .input(PortSpec::optional("polygons", Type::Map, "short-circuit regions: ids > 0, their cells one node"))
        .input(PortSpec::optional("mask", Type::Map, "cells where it is not positive (or has no value) removed"))
        .output(PortSpec::new("resistances", Type::Table, "the effective resistances"))
        .output(PortSpec::new("cum", Type::Map, "the cumulative current (cum_map)"))
        .output(PortSpec::new("max", Type::Map, "the largest current of the pairs (max_map)"))
        .param(PortSpec::optional("habitat_is_resistance", Type::Bool, "resistances (default) or conductances"))
        .param(PortSpec::optional("four_neighbors", Type::Bool, "the four orthogonal neighbours only"))
        .param(PortSpec::optional("avg_resistances", Type::Bool, "neighbours joined by their resistances' average"))
        .param(PortSpec::optional("cum_map", Type::Bool, "the cumulative current"))
        .param(PortSpec::optional("max_map", Type::Bool, "the largest current"))
        .param(PortSpec::optional("focal_zero", Type::Bool, "current 0 on the focal nodes in the maps"))
        .param(PortSpec::optional("reclass_table", Type::Any, "[[value, new value], ...] applied to the habitat first"))
}

fn map_of(grid: &Arc<terraflux_raster::Grid>, v: Vec<f64>) -> Value {
    Value::Map(Map::new(grid.clone(), v))
}

struct Pairwise;

impl Operator for Pairwise {
    fn signature(&self) -> OpSignature {
        raster_mode(OpSignature::new(
            "circuit.pairwise",
            "Circuitscape's pairwise mode on rasters (landschaft's circuitscape_pairwise): the effective resistance \
             between every pair of focal points (-1: not connected), every component factorised once and its \
             pairs solved together; the cumulative and the largest current maps when asked.",
            Access::Global,
        ))
        .param(PortSpec::optional("null_currents", Type::Bool, "no value instead of 0 where no current (in the maps)"))
        .param(PortSpec::optional("log_transform", Type::Bool, "log10 of the current maps"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (mut c, grid) = Circuit::of(args)?;
        let pairs = included_pairs_of(args, false)?;
        let excluded = match &pairs {
            Some(p) => {
                if p.mode == "include" {
                    c.points.retain(&p.ids);
                }
                p.excluded()
            }
            None => Default::default(),
        };
        let cells: Vec<i64> = c.points.rows.iter().zip(&c.points.cols).map(|(r, k)| (r * c.nx + k) as i64).collect();
        let regions = c.points.has_regions();
        let ids = if regions { c.points.unique() } else { c.points.ids.clone() };
        let n = ids.len();
        let ex: Vec<u8> = (0..n * n).map(|k| u8::from(excluded.contains(&(ids[k / n], ids[k % n])))).collect();
        let (cum, max): (bool, bool) = (args.param("cum_map", false)?, args.param("max_map", false)?);
        let flags = [
            u8::from(cum),
            u8::from(max),
            u8::from(args.param("focal_zero", false)?),
            u8::from(args.param("null_currents", false)?),
            0,
            u8::from(args.param("log_transform", false)?),
            0,
            0,
        ];
        let out = ffi::circuit_pairwise(
            &c.g,
            &c.poly,
            c.nx as i64,
            c.ny as i64,
            &cells,
            if regions { &c.points.ids } else { &[] },
            &ex,
            args.param("avg_resistances", false)?,
            args.param("four_neighbors", false)?,
            &flags,
        )?;
        let mut t =
            crate::Table::new(std::iter::once("id".to_string()).chain(ids.iter().map(|i| i.to_string())).collect());
        for (a, id) in ids.iter().enumerate() {
            t.rows
                .push(std::iter::once(*id as f64).chain(out.resistances[a * n..(a + 1) * n].iter().copied()).collect());
        }
        let mut o = Outputs::new();
        o.insert("resistances".into(), Value::Table(t));
        if cum {
            o.insert("cum".into(), map_of(&grid, out.cum));
        }
        if max {
            o.insert("max".into(), map_of(&grid, out.max));
        }
        Ok(o)
    }
}

/// create_new_polymap with a point map: the focal points and regions become short-circuit regions.
fn point_polymap(poly: &[i64], point_map: &[i64], ids: &[i64], nx: usize, ny: usize) -> Vec<i64> {
    if poly.is_empty() {
        return point_map.to_vec();
    }
    let mut newpoly = poly.to_vec();
    let mut u = ids.to_vec();
    u.sort_unstable();
    u.dedup();
    let no_regions = u.len() == ids.len();
    // the cells of the point map in column-major order
    let cells: Vec<usize> =
        (0..nx).flat_map(|j| (0..ny).map(move |i| i * nx + j)).filter(|&k| point_map[k] != 0).collect();
    if no_regions {
        let k = *poly.iter().max().unwrap();
        for c in cells {
            if poly[c] == 0 {
                newpoly[c] = point_map[c] + k;
            }
        }
        return newpoly;
    }
    let k = (*poly.iter().max().unwrap()).max(*point_map.iter().max().unwrap());
    for c in cells {
        let (v1, v2) = (point_map[c], newpoly[c]);
        if v2 == 0 {
            newpoly[c] = k + v1;
            continue;
        }
        if v1 != v2 {
            for x in newpoly.iter_mut() {
                if *x == v2 {
                    *x = v1;
                }
            }
        }
    }
    newpoly
}

/// One problem of one-to-all: its sources, grounds (cells row-major, values) and the cell checked.
struct Problem {
    src: (Vec<i64>, Vec<f64>),
    gnd: (Vec<i64>, Vec<f64>),
    check: i64,
    strength: f64,
    n: i64,
    /// Its own short-circuit regions (focal regions with included pairs), else the shared ones.
    poly: Option<Vec<i64>>,
}

struct OneToAll;

impl Operator for OneToAll {
    fn signature(&self) -> OpSignature {
        raster_mode(OpSignature::new(
            "circuit.one_to_all",
            "Circuitscape's one-to-all (or all-to-one) mode on rasters (landschaft's circuitscape_one_to_all): \
             for every focal point in turn, 1 A (or its strength) injected there with every other point a direct \
             ground (one-to-all), or the point grounded and the others injecting (all-to-one); the voltage over \
             the strength at each point (one-to-all), the cumulative and largest current maps when asked. \
             Circuitscape's quirks kept (strengths by position); all-to-one measures no resistances, so its \
             table's `value` column is 0, as Circuitscape's: read its current maps.",
            Access::Global,
        ))
        .param(PortSpec::optional("all_to_one", Type::Bool, "all-to-one instead of one-to-all"))
        .param(PortSpec::optional("strengths", Type::Any, "[[id, strength], ...], by position as Circuitscape"))
        .param(PortSpec::optional(
            "batch",
            Type::Integer,
            "the problems solved together (default 4 x the threads; the last bits depend on it)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: String| Error::msg(format!("node {:?}: {e}", args.node));
        let (mut c, grid) = Circuit::of(args)?;
        let strengths: Option<Vec<(f64, f64)>> = match args.params.get("strengths") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => {
                let rows: Vec<[f64; 2]> =
                    serde_json::from_value(v.clone()).map_err(|e| fail(format!("strengths: {e}")))?;
                Some(rows.into_iter().map(|r| (r[0], r[1])).collect())
            }
        };
        let threads = crate::threads::ensure();
        let o = OneToAllOptions {
            one_to_all: !args.param("all_to_one", false)?,
            strengths,
            pairs: included_pairs_of(args, false)?,
            cum: args.param("cum_map", false)?,
            max: args.param("max_map", false)?,
            focal_zero: args.param("focal_zero", false)?,
            volt_maps: false,
            cur_maps: false,
            avg_resistances: args.param("avg_resistances", false)?,
            four_neighbors: args.param("four_neighbors", false)?,
            batch: args.opt_param::<usize>("batch")?.unwrap_or(4 * threads).max(1),
        };
        let r = one_to_all(&mut c, &o)?;
        let mut t =
            crate::Table::new(vec!["id".into(), if o.one_to_all { "resistance".into() } else { "value".into() }]);
        t.rows = r.ids.iter().zip(&r.values).map(|(u, v)| vec![*u as f64, *v]).collect();
        let mut out = Outputs::new();
        out.insert("resistances".into(), Value::Table(t));
        if o.cum {
            out.insert("cum".into(), map_of(&grid, r.cum));
        }
        if o.max {
            out.insert("max".into(), map_of(&grid, r.max));
        }
        Ok(out)
    }
}

/// How the one-to-all (all-to-one) driver runs.
pub struct OneToAllOptions {
    pub one_to_all: bool,
    /// (id, strength) rows, by position as Circuitscape.
    pub strengths: Option<Vec<(f64, f64)>>,
    pub pairs: Option<IncludedPairs>,
    pub cum: bool,
    pub max: bool,
    pub focal_zero: bool,
    pub volt_maps: bool,
    pub cur_maps: bool,
    pub avg_resistances: bool,
    pub four_neighbors: bool,
    /// The problems solved together.
    pub batch: usize,
}

/// A point's id and its voltage and current maps (when asked).
pub type PointMaps = (i64, Option<Vec<f64>>, Option<Vec<f64>>);

/// The results of the one-to-all driver: per focal id its value, the cumulative and largest current maps, and
/// per point solved its voltage and current maps when asked.
pub struct OneToAllResult {
    pub ids: Vec<i64>,
    pub values: Vec<f64>,
    pub cum: Vec<f64>,
    pub max: Vec<f64>,
    pub maps: Vec<PointMaps>,
}

/// Circuitscape's one-to-all (all-to-one) driver (landschaft's _one_to_all, run_onetoall).
pub fn one_to_all(c: &mut Circuit, o: &OneToAllOptions) -> Result<OneToAllResult> {
    let (nx, ny) = (c.nx, c.ny);
    let one_to_all = o.one_to_all;
    let mut strengths = o.strengths.clone();
    let pairs = o.pairs.clone();
    if let Some(p) = &pairs {
        c.points.retain(&p.ids);
        if let Some(st) = &mut strengths {
            st.retain(|(id, _)| p.ids.contains(&(*id as i64)));
        }
    }
    // include mode: the pairs not marked; exclude mode: the marked ones
    let mode = if pairs.as_ref().is_some_and(|p| p.mode == "include") { 0 } else { 1 };
    let focal_zero = o.focal_zero;
    let mut maps = Vec::new();
    let p = &c.points;
    let cells: Vec<usize> = p.rows.iter().zip(&p.cols).map(|(r, k)| r * nx + k).collect();
    let mut point_map = vec![0i64; nx * ny];
    for (k, &cell) in cells.iter().enumerate() {
        point_map[cell] = p.ids[k];
    }
    let unique = p.unique();
    // the first cell of each id
    let mut unique_map = vec![0i64; nx * ny];
    for &u in &unique {
        let first = p.ids.iter().position(|&i| i == u).unwrap();
        unique_map[cells[first]] = u;
    }
    let base_poly = point_polymap(&c.poly, &point_map, &p.ids, nx, ny);
    let no_regions = p.ids.len() == p.unique().len();
    let mut res = vec![0.0; unique.len()];
    let mut cum = vec![0.0; nx * ny];
    let mut mx = vec![NODATA; nx * ny];
    let policy = if one_to_all { 2 } else { 1 };
    let problem = |i: usize| -> Option<Problem> {
        let n = unique[i];
        if let Some(st) = &strengths
            && i >= st.len()
        {
            return None;
        }
        let strength = strengths.as_ref().map_or(1.0, |s| s[i].1);
        // the focal points this one is not paired with leave the point map
        let mut pm = point_map.clone();
        let mut poly = None;
        if let Some(pr) = &pairs {
            let k = pr.ids.len();
            let row = pr.ids.iter().position(|&x| x == n)?;
            for j in 0..k {
                if j != row && pr.matrix[row * k + j] == mode {
                    let gone = pr.ids[j];
                    pm.iter_mut().filter(|v| **v == gone).for_each(|v| *v = 0);
                }
            }
            if !no_regions {
                poly = Some(point_polymap(&c.poly, &pm, &p.ids, nx, ny));
            }
        }
        if pm.iter().sum::<i64>() == n {
            return None;
        }
        let smap: Option<Vec<f64>> = strengths.as_ref().map(|st| {
            let mut m = vec![0.0; nx * ny];
            for (k, &cell) in cells.iter().enumerate() {
                if k < st.len() {
                    m[cell] = if pm[cell] == 0 { 1.0 } else { st[k].1 };
                }
            }
            m
        });
        let (src, gnd): (Vec<f64>, Vec<f64>) = if one_to_all {
            (
                unique_map.iter().map(|&u| if u == n { strength } else { 0.0 }).collect(),
                pm.iter().map(|&v| if v == n || v == 0 { 0.0 } else { f64::INFINITY }).collect(),
            )
        } else {
            let src = match &smap {
                Some(m) => unique_map.iter().zip(m).map(|(&u, &v)| if u == n { 0.0 } else { v }).collect(),
                None => unique_map
                    .iter()
                    .zip(&pm)
                    .map(|(&u, &v)| {
                        if v == n {
                            0.0
                        } else if u != 0 {
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .collect(),
            };
            (src, pm.iter().map(|&v| if v == n { f64::INFINITY } else { 0.0 }).collect())
        };
        let nonzero = |v: &[f64]| -> (Vec<i64>, Vec<f64>) {
            v.iter().enumerate().filter(|(_, x)| **x != 0.0).map(|(k, x)| (k as i64, *x)).unzip()
        };
        let check = cells[p.ids.iter().position(|&x| x == n).unwrap()] as i64;
        Some(Problem { src: nonzero(&src), gnd: nonzero(&gnd), check, strength, n, poly })
    };
    let batch = o.batch.max(1);
    let mut b0 = 0;
    while b0 < unique.len() {
        let range = b0..(b0 + batch).min(unique.len());
        let probs: Vec<(usize, Option<Problem>)> = range.clone().map(|i| (i, problem(i))).collect();
        for (i, q) in &probs {
            if q.is_none() {
                res[*i] = -1.0;
            }
        }
        let todo: Vec<(usize, &Problem)> = probs.iter().filter_map(|(i, q)| q.as_ref().map(|q| (*i, q))).collect();
        // one call per graph (focal regions with included pairs rebuild it per point)
        let mut groups: Vec<Vec<(usize, &Problem)>> = Vec::new();
        for (i, q) in todo {
            match groups.last_mut() {
                Some(g) if q.poly.is_none() && g[0].1.poly.is_none() => g.push((i, q)),
                _ => groups.push(vec![(i, q)]),
            }
        }
        for todo in groups {
            let poly = todo[0].1.poly.as_ref().unwrap_or(&base_poly);
            if !todo.is_empty() {
                let mut lists = [Vec::new(), Vec::new(), vec![0i64], Vec::new(), Vec::new(), vec![0i64]];
                let (mut sc, mut sv, mut gc, mut gv) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
                let mut checks = Vec::new();
                for (_, q) in &todo {
                    sc.extend(&q.src.0);
                    sv.extend(&q.src.1);
                    lists[2].push(sc.len() as i64);
                    gc.extend(&q.gnd.0);
                    gv.extend(&q.gnd.1);
                    lists[5].push(gc.len() as i64);
                    checks.push(q.check);
                }
                let out = ffi::circuit_multi_advanced(
                    &c.g,
                    poly,
                    nx as i64,
                    ny as i64,
                    o.avg_resistances,
                    o.four_neighbors,
                    &sc,
                    &sv,
                    &lists[2],
                    &gc,
                    &gv,
                    &lists[5],
                    &checks,
                    policy,
                )?;
                let nm = &out.nodemap;
                for (k, (i, q)) in todo.iter().enumerate() {
                    let (a, b) = (out.node_offsets[k] as usize, out.node_offsets[k + 1] as usize);
                    let (volt, curr) = (&out.voltage[a..b], &out.current[a..b]);
                    let active = &out.active[out.active_offsets[k] as usize..out.active_offsets[k + 1] as usize];
                    res[*i] = if out.solved[k] == 0 {
                        -1.0
                    } else if !one_to_all {
                        0.0
                    } else {
                        let cell = unique_map.iter().position(|&u| u == q.n).unwrap();
                        let node = nm[cell];
                        let val = if node < 0 { 0.0 } else { volt[node as usize] / q.strength };
                        if val == 0.0 { -1.0 } else { val }
                    };
                    let (mut vmap, mut cmap) = (Vec::new(), Vec::new());
                    for (cell, &node) in nm.iter().enumerate() {
                        let mut cur = if node >= 0 { curr[node as usize] } else { 0.0 };
                        if focal_zero && active.contains(&node) {
                            cur = 0.0;
                        }
                        cum[cell] += cur;
                        if cur > mx[cell] {
                            mx[cell] = cur;
                        }
                        if o.volt_maps {
                            vmap.push(if node >= 0 { volt[node as usize] } else { 0.0 });
                        }
                        if o.cur_maps {
                            cmap.push(cur);
                        }
                    }
                    if o.volt_maps || o.cur_maps {
                        maps.push((q.n, o.volt_maps.then_some(vmap), o.cur_maps.then_some(cmap)));
                    }
                }
            }
        }
        b0 = range.end;
    }
    for v in cum.iter_mut().chain(mx.iter_mut()) {
        if *v < NODATA {
            *v = NODATA;
        }
    }
    Ok(OneToAllResult { ids: unique, values: res, cum, max: mx, maps })
}

/// Circuitscape's include / exclude pairs: the mode, the focal ids and the n x n 0/1 matrix marking the pairs.
#[derive(Debug, Clone)]
pub struct IncludedPairs {
    pub mode: String,
    pub ids: Vec<i64>,
    pub matrix: Vec<i64>,
}

impl IncludedPairs {
    /// The pairs of ids not solved (include mode: the unmarked ones; exclude mode: the marked ones).
    pub fn excluded(&self) -> std::collections::HashSet<(i64, i64)> {
        let target = if self.mode == "include" { 0 } else { 1 };
        let n = self.ids.len();
        let mut out = std::collections::HashSet::new();
        for j in 0..n {
            for i in 0..n {
                if self.matrix[i * n + j] == target && self.matrix[j * n + i] == target {
                    out.insert((self.ids[i], self.ids[j]));
                }
            }
        }
        out
    }
}

/// Circuitscape's read_included_pairs: a min/max matrix or a mode list of pairs (`starts_from_zero`: ids of a
/// network numbered from 0, shifted by 1).
pub fn read_included_pairs(text: &str, starts_from_zero: bool) -> Result<IncludedPairs> {
    let lines: Vec<&str> = text.lines().collect();
    let first = lines.first().copied().unwrap_or("");
    let nums = |l: &str| -> Result<Vec<f64>> {
        l.split_whitespace().map(|t| t.parse::<f64>().map_err(|_| Error::msg(format!("not a number: {t:?}")))).collect()
    };
    let shift = i64::from(starts_from_zero);
    if first.starts_with("min") {
        let word = |l: &str| l.split_whitespace().nth(1).and_then(|t| t.parse::<f64>().ok());
        let (minval, maxval) = (word(first).unwrap_or(0.0), word(lines.get(1).copied().unwrap_or("")).unwrap_or(0.0));
        let rows: Vec<Vec<f64>> = lines[2.min(lines.len())..]
            .iter()
            .filter(|l| !l.trim().is_empty())
            .map(|l| nums(l))
            .collect::<Result<_>>()?;
        let ids: Vec<i64> = rows.first().map_or(Vec::new(), |r| r[1..].iter().map(|v| *v as i64 + shift).collect());
        let n = ids.len();
        let mut matrix = vec![0i64; n * n];
        for (i, r) in rows.iter().skip(1).enumerate().take(n) {
            for j in 0..n {
                let v = r.get(j + 1).copied().unwrap_or(0.0);
                let v = if v > maxval { 0.0 } else { v };
                matrix[i * n + j] = i64::from(v >= minval);
            }
        }
        return Ok(IncludedPairs { mode: "include".into(), ids, matrix });
    }
    if first.starts_with("mode") {
        let mode = first.split_whitespace().nth(1).unwrap_or("").to_string();
        let flat: Vec<f64> =
            lines[1..].iter().filter(|l| !l.trim().is_empty()).map(|l| nums(l)).collect::<Result<Vec<_>>>()?.concat();
        let pairs: Vec<(i64, i64)> =
            flat.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0] as i64 + shift, c[1] as i64 + shift)).collect();
        let mut ids: Vec<i64> = pairs.iter().flat_map(|&(a, b)| [a, b]).filter(|&v| v != 0).collect();
        ids.sort_unstable();
        ids.dedup();
        let n = ids.len();
        let mut matrix = vec![0i64; n * n];
        for (a, b) in pairs {
            if let (Ok(i), Ok(j)) = (ids.binary_search(&a), ids.binary_search(&b)) {
                matrix[i * n + j] = 1;
                matrix[j * n + i] = 1;
            }
        }
        return Ok(IncludedPairs { mode, ids, matrix });
    }
    Err(Error::msg("Error reading focal node include/exclude pairs file."))
}

/// A text file, gzipped or not.
pub fn read_text(path: &str) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|e| Error::msg(format!("{path}: {e}")))?;
    if path.to_lowercase().ends_with("gz") {
        use std::io::Read;
        let mut s = String::new();
        flate2::read::GzDecoder::new(&bytes[..])
            .read_to_string(&mut s)
            .map_err(|e| Error::msg(format!("{path}: {e}")))?;
        return Ok(s);
    }
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The included pairs of a node's `included_pairs` parameter (the path of Circuitscape's file).
fn included_pairs_of(args: &Args, starts_from_zero: bool) -> Result<Option<IncludedPairs>> {
    match args.opt_param::<String>("included_pairs")?.filter(|p| !p.is_empty()) {
        None => Ok(None),
        Some(path) => read_included_pairs(&read_text(&path)?, starts_from_zero)
            .map(Some)
            .map_err(|e| Error::msg(format!("node {:?}: included_pairs: {e}", args.node))),
    }
}

/// The rows of a Circuitscape text table (values split by spaces or commas).
fn read_table(text: &str) -> Result<Vec<Vec<f64>>> {
    text.lines()
        .map(|l| l.replace(',', " "))
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            l.split_whitespace()
                .map(|t| t.parse::<f64>().map_err(|_| Error::msg(format!("not a number: {t:?}"))))
                .collect()
        })
        .collect()
}

/// A table of `ncol` columns from a node's input (a table) or its file parameter.
fn rows_of(args: &Args, port: &str, file: &str, ncol: usize) -> Result<Option<Vec<Vec<f64>>>> {
    let fail = |m: String| Error::msg(format!("node {:?}: {port}: {m}", args.node));
    let rows = match args.get(port) {
        Some(Value::Table(t)) => t.rows.clone(),
        Some(_) => return Err(fail("a table".into())),
        None => match args.opt_param::<String>(file)?.filter(|p| !p.is_empty()) {
            Some(path) => read_table(&read_text(&path)?).map_err(|e| fail(e.to_string()))?,
            None => return Ok(None),
        },
    };
    if rows.iter().any(|r| r.len() != ncol) {
        return Err(fail(format!("expected {ncol} columns on every line")));
    }
    Ok(Some(rows))
}

/// A Circuitscape edge list as its load_graph reads it: nodes from 1 (shifted when they start at 0), values.
/// An edge list: from, to (nodes from 1), values, and whether the nodes were numbered from 0.
type Graph = (Vec<i64>, Vec<i64>, Vec<f64>, bool);

fn read_graph(args: &Args) -> Result<Graph> {
    let fail = |m: &str| Error::msg(format!("node {:?}: {m}", args.node));
    let t = rows_of(args, "graph", "graph_file", 3)?
        .ok_or_else(|| fail("connect a graph (node, node, value) or give graph_file"))?;
    if t.is_empty() {
        return Err(fail("the graph has no edges"));
    }
    let (mut i, mut j): (Vec<i64>, Vec<i64>) = t.iter().map(|r| (r[0] as i64, r[1] as i64)).unzip();
    let low = i.iter().chain(&j).copied().min().unwrap();
    if low > 1 {
        return Err(fail(&format!("the graph numbers its nodes from {low}; Circuitscape counts from 1 (or 0)")));
    }
    if low < 0 {
        return Err(fail("negative node numbers"));
    }
    let zero = low == 0;
    if zero {
        i.iter_mut().chain(j.iter_mut()).for_each(|v| *v += 1);
    }
    Ok((i, j, t.iter().map(|r| r[2]).collect(), zero))
}

/// The edges for the solver: 0-based nodes, conductances, the number of nodes.
fn edges(i: &[i64], j: &[i64], w: &[f64], is_resistance: bool) -> (Vec<i32>, Vec<i32>, Vec<f64>, i32) {
    let w = w.iter().map(|v| if is_resistance { 1.0 / v } else { *v }).collect();
    let m = i.iter().chain(j).copied().max().unwrap_or(0) as i32;
    (i.iter().map(|v| (*v - 1) as i32).collect(), j.iter().map(|v| (*v - 1) as i32).collect(), w, m)
}

/// Not within 1e-6 of 0 (NaN too).
fn not_zero(v: f64) -> bool {
    v.abs().partial_cmp(&1e-6) != Some(std::cmp::Ordering::Less)
        && v.abs().partial_cmp(&1e-6) != Some(std::cmp::Ordering::Equal)
}

/// write_currents: the branches whose current is not within 1e-6 of 0 (nodes from 1).
fn branch_table(r: &[i32], c: &[i32], v: &[f64]) -> crate::Table {
    let mut t = crate::Table::new(vec!["from".into(), "to".into(), "current".into()]);
    t.rows = (0..r.len())
        .filter(|&k| not_zero(v[k]))
        .map(|k| vec![f64::from(r[k] + 1), f64::from(c[k] + 1), v[k]])
        .collect();
    t
}

fn node_table(values: &[f64], name: &str) -> crate::Table {
    let mut t = crate::Table::new(vec!["node".into(), name.into()]);
    t.rows = values.iter().enumerate().map(|(k, v)| vec![(k + 1) as f64, *v]).collect();
    t
}

fn network_params(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::optional("graph", Type::Table, "the edges: node, node, resistance (or conductance)"))
        .param(PortSpec::optional("graph_file", Type::Text, "or Circuitscape's edge list file"))
        .param(PortSpec::optional("graph_is_resistance", Type::Bool, "the edge values are resistances (default true)"))
}

struct NetworkAdvanced;

impl Operator for NetworkAdvanced {
    fn signature(&self) -> OpSignature {
        network_params(OpSignature::new(
            "circuit.network_advanced",
            "Circuitscape's advanced mode on a network (data_type = network; landschaft's \
             circuitscape_network_advanced): currents injected at nodes and grounds (a resistance of 0 a direct \
             ground) on a graph of edges numbered from 1 (or 0, then every number shifted, as Circuitscape.jl). \
             Duplicate edges add up. Every component with a source and a ground is solved.",
            Access::None,
        ))
        .input(PortSpec::optional("sources", Type::Table, "node, current"))
        .input(PortSpec::optional("grounds", Type::Table, "node, resistance (or conductance) to ground"))
        .output(PortSpec::new("voltages", Type::Table, "node, voltage (0 where not solved)"))
        .output(PortSpec::new("node_currents", Type::Table, "node, current"))
        .output(PortSpec::new("branch_currents", Type::Table, "from, to, current"))
        .param(PortSpec::optional("sources_file", Type::Text, "or Circuitscape's source file"))
        .param(PortSpec::optional("grounds_file", Type::Text, "or Circuitscape's ground file"))
        .param(PortSpec::optional(
            "ground_is_resistance",
            Type::Bool,
            "the ground values are resistances (default true)",
        ))
        .param(
            PortSpec::optional("remove_src_or_gnd", Type::Text, "nodes that are both: keepall (default)...")
                .choices(["keepall", "rmvsrc", "rmvgnd", "rmvall"]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let policy = match args.param("remove_src_or_gnd", "keepall".to_string())?.as_str() {
            "keepall" => 0,
            "rmvsrc" => 1,
            "rmvgnd" => 2,
            "rmvall" => 3,
            p => return Err(fail(format!("remove_src_or_gnd must be keepall, rmvsrc, rmvgnd or rmvall, not {p:?}"))),
        };
        let (i, j, w, zero) = read_graph(args)?;
        let (a, b, w, m) = edges(&i, &j, &w, args.param("graph_is_resistance", true)?);
        let strengths = |port: &str, file: &str| -> Result<Vec<Vec<f64>>> {
            let mut t =
                rows_of(args, port, file, 2)?.ok_or_else(|| fail(format!("connect the {port} or give {file}")))?;
            if t.iter().any(|r| r[0] == 0.0) || zero {
                t.iter_mut().for_each(|r| r[0] += 1.0);
            }
            Ok(t)
        };
        let src_t = strengths("sources", "sources_file")?;
        let mut gnd_t = strengths("grounds", "grounds_file")?;
        if args.param("ground_is_resistance", true)? {
            gnd_t.iter_mut().for_each(|r| r[1] = 1.0 / r[1]);
        }
        let (mut src, mut gnd) = (vec![0.0; m as usize], vec![0.0; m as usize]);
        for (table, target, name) in [(&src_t, &mut src, "source"), (&gnd_t, &mut gnd, "ground")] {
            for r in table {
                let id = r[0] as i64;
                if id < 1 || id > i64::from(m) {
                    return Err(fail(format!("a {name} node is not in the graph (nodes 1..{m})")));
                }
                target[(id - 1) as usize] = r[1];
            }
        }
        let r = ffi::circuit_network_advanced(&a, &b, &w, m, &src, &gnd, policy)?;
        Ok([
            ("voltages".to_string(), Value::Table(node_table(&r.voltage, "voltage"))),
            ("node_currents".to_string(), Value::Table(node_table(&r.current, "current"))),
            ("branch_currents".to_string(), Value::Table(branch_table(&r.branch_r, &r.branch_c, &r.branch_v))),
        ]
        .into())
    }
}

struct NetworkPairwise;

impl Operator for NetworkPairwise {
    fn signature(&self) -> OpSignature {
        network_params(OpSignature::new(
            "circuit.network_pairwise",
            "Circuitscape's pairwise mode on a network (landschaft's circuitscape_network_pairwise): the effective \
             resistance between every pair of focal nodes (-1: not connected), every component factorised once; \
             included or excluded pairs from Circuitscape's file; with the map options every pair's node voltages, \
             node and branch currents (JSON), and the currents summed over the pairs.",
            Access::None,
        ))
        .output(PortSpec::new("resistances", Type::Table, "a row per focal node: its id, then its resistances"))
        .output(PortSpec::optional(
            "cum_node_currents",
            Type::Table,
            "with current_maps: node, current summed over the pairs",
        ))
        .output(PortSpec::optional("cum_branch_currents", Type::Table, "with current_maps: from, to, current"))
        .output(PortSpec::optional("maps", Type::Any, "with the map options: each pair's values (JSON)"))
        .param(PortSpec::new("focal_nodes", Type::Any, "the focal node numbers, or the path of Circuitscape's file"))
        .param(PortSpec::optional("included_pairs", Type::Text, "Circuitscape's include / exclude pairs file"))
        .param(PortSpec::optional("voltage_maps", Type::Bool, "every pair's node voltages"))
        .param(PortSpec::optional("current_maps", Type::Bool, "every pair's currents, and their sums"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let (i, j, w, zero) = read_graph(args)?;
        let (a, b, w, m) = edges(&i, &j, &w, args.param("graph_is_resistance", true)?);
        // read_focal_points: every number of the file, column by column; from 0 shifted to 1
        let mut fp: Vec<i64> = match args.params.get("focal_nodes") {
            Some(serde_json::Value::String(path)) => {
                let rows = read_table(&read_text(path)?).map_err(|e| fail(e.to_string()))?;
                if !rows.is_empty() && rows.iter().all(|r| r.len() == rows[0].len()) {
                    (0..rows[0].len()).flat_map(|c| rows.iter().map(move |r| r[c] as i64)).collect()
                } else {
                    rows.iter().flatten().map(|v| *v as i64).collect()
                }
            }
            Some(v) => serde_json::from_value::<Vec<f64>>(v.clone())
                .map_err(|e| fail(format!("focal_nodes: {e}")))?
                .into_iter()
                .map(|x| x as i64)
                .collect(),
            None => return Err(fail("give the focal nodes".into())),
        };
        if fp.iter().min() == Some(&0) {
            fp.iter_mut().for_each(|v| *v += 1);
        }
        let pairs = included_pairs_of(args, zero)?;
        let mut excluded = std::collections::HashSet::new();
        if let Some(p) = &pairs {
            if p.mode == "include" {
                fp.retain(|v| p.ids.contains(v));
            }
            excluded = p.excluded();
        }
        let n = fp.len();
        let ex: Vec<u8> = (0..n * n).map(|k| u8::from(excluded.contains(&(fp[k / n], fp[k % n])))).collect();
        let (volt, cur): (bool, bool) = (args.param("voltage_maps", false)?, args.param("current_maps", false)?);
        let points: Vec<i32> =
            fp.iter().map(|&v| if v >= 1 && v <= i64::from(m) { (v - 1) as i32 } else { -1 }).collect();
        let r = ffi::circuit_network_pairwise(&a, &b, &w, m, &points, &ex, volt, cur, cur)?;
        let mut t =
            crate::Table::new(std::iter::once("id".to_string()).chain(fp.iter().map(|v| v.to_string())).collect());
        for (p, id) in fp.iter().enumerate() {
            t.rows.push(std::iter::once(*id as f64).chain(r.resistances[p * n..(p + 1) * n].iter().copied()).collect());
        }
        let mut out = Outputs::new();
        out.insert("resistances".into(), Value::Table(t));
        if !r.cum.is_empty() {
            out.insert("cum_node_currents".into(), Value::Table(node_table(&r.cum, "current")));
            // one slot per input edge: the first line of a node pair (either way round) takes its summed current
            let total: std::collections::HashMap<(i64, i64), f64> = (0..r.cum_r.len())
                .map(|k| ((i64::from(r.cum_r[k]) + 1, i64::from(r.cum_c[k]) + 1), r.cum_v[k]))
                .collect();
            let mut seen = std::collections::HashSet::new();
            let mut ct = crate::Table::new(vec!["from".into(), "to".into(), "current".into()]);
            for (&p, &q) in i.iter().zip(&j) {
                let key = (p.min(q), p.max(q));
                if !seen.insert(key) {
                    continue;
                }
                let v = total.get(&key).copied().unwrap_or(0.0);
                if not_zero(v) {
                    ct.rows.push(vec![p as f64, q as f64, v]);
                }
            }
            out.insert("cum_branch_currents".into(), Value::Table(ct));
        }
        if volt || cur {
            let mut maps = serde_json::Map::new();
            for k in 0..r.pair_ci.len() {
                let key = format!("{}_{}", fp[r.pair_ci[k] as usize], fp[r.pair_cj[k] as usize]);
                let (a0, a1) = (r.pair_node_off[k] as usize, r.pair_node_off[k + 1] as usize);
                let nodes: Vec<f64> = r.pair_nodes[a0..a1].iter().map(|v| f64::from(*v) + 1.0).collect();
                let (b0, b1) = (r.pair_branch_off[k] as usize, r.pair_branch_off[k + 1] as usize);
                let branches =
                    branch_table(&r.pair_branch_r[b0..b1], &r.pair_branch_c[b0..b1], &r.pair_branch_v[b0..b1]);
                let mut entry = serde_json::json!({
                    "nodes": nodes,
                    "node_currents": r.pair_current[a0..a1].to_vec(),
                    "branch_currents": branches.rows,
                });
                if volt {
                    entry["voltages"] = serde_json::json!(r.pair_voltage[a0..a1].to_vec());
                }
                maps.insert(key, entry);
            }
            out.insert("maps".into(), Value::Json(serde_json::Value::Object(maps)));
        }
        Ok(out)
    }
}
