//! The preparation of FUTURES' inputs (landschaft's simulate/_futures.py): the development pressure of the cells
//! (r.futures.devpressure, its r.mfilter by landschaft's C++) and the library of the patches developed between two
//! maps (r.futures.calib's patch library), which futures.simulation takes.

use std::sync::Arc;

use serde_json::json;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(DevPressure));
    r.add(Arc::new(PatchLibrary));
    r.add(Arc::new(Calibrate));
}

/// r.futures.devpressure's filter: the weight of each cell of the (2 size + 1)^2 window at distance d (0 beyond
/// size and at the centre).
fn weights(size: usize, method: &str, gamma: f64, scaling: f64) -> Vec<f64> {
    let side = 2 * size + 1;
    (0..side * side)
        .map(|k| {
            let (i, j) = ((k / side) as f64 - size as f64, (k % side) as f64 - size as f64);
            let d = (i * i + j * j).sqrt();
            let d = if d <= size as f64 { d } else { 0.0 };
            match method {
                "occurrence" => {
                    if d > 0.0 {
                        1.0
                    } else {
                        0.0
                    }
                }
                "gravity" => {
                    let den = d.powf(gamma);
                    if den == 0.0 { 0.0 } else { scaling / den }
                }
                _ => {
                    if d > 0.0 {
                        scaling * (-2.0 * d / gamma).exp()
                    } else {
                        0.0
                    }
                }
            }
        })
        .collect()
}

/// The development pressure of every cell (landschaft's development_pressure).
#[allow(clippy::too_many_arguments)]
pub fn development_pressure(
    v: &[f64],
    ny: usize,
    nx: usize,
    size: usize,
    method: &str,
    gamma: f64,
    scaling: f64,
    pad: bool,
) -> Result<Vec<f64>> {
    let w = weights(size, method, gamma, scaling);
    let out: Vec<f64> = if pad {
        let (by, bx) = (ny + 2 * size, nx + 2 * size);
        let mut big = vec![0.0; by * bx];
        for y in 0..ny {
            for x in 0..nx {
                let val = v[y * nx + x];
                big[(y + size) * bx + x + size] = if val.is_nan() { 0.0 } else { val };
            }
        }
        let f = ffi::sim_futures_mfilter(&big, by as i64, bx as i64, &w, size as i32)?;
        (0..ny * nx)
            .map(|k| {
                let (y, x) = (k / nx, k % nx);
                if v[k].is_nan() { f64::NAN } else { f64::from(f[(y + size) * bx + x + size]) }
            })
            .collect()
    } else {
        ffi::sim_futures_mfilter(v, ny as i64, nx as i64, &w, size as i32)?.into_iter().map(f64::from).collect()
    };
    Ok(out)
}

struct DevPressure;

impl Operator for DevPressure {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "futures.devpressure",
            "The development pressure of each cell (r.futures.devpressure through landschaft): the sum over the \
             developed cells within a radius (a circular window, the cell itself left out) of 1 (occurrence), \
             scaling / d^gamma (gravity) or scaling exp(-2 d / gamma) (kernel), as GRASS's r.mfilter computes it; \
             the pressure futures.simulation starts from.",
            Access::Global,
        )
        .input(PortSpec::new("developed", Type::Map, "1 developed, 0 not, missing excluded"))
        .output(PortSpec::new("map", Type::Map, "the pressure (floats; missing where excluded)"))
        .param(PortSpec::new("size", Type::Integer, "the window's radius in cells"))
        .param(PortSpec::optional("method", Type::Text, "gravity (default), occurrence or kernel").choices([
            "gravity",
            "occurrence",
            "kernel",
        ]))
        .param(PortSpec::optional("gamma", Type::Number, "the decay with distance (default 1.5)"))
        .param(PortSpec::optional("scaling_factor", Type::Number, "default 1"))
        .param(PortSpec::optional(
            "pad",
            Type::Bool,
            "pad with undeveloped cells (the module's -n, default): the cells near the edges get their pressure",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: &str| Error::msg(format!("node {:?}: {m}", args.node));
        let m = args.map("developed")?;
        let size: usize = args.opt_param("size")?.ok_or_else(|| fail("set the size"))?;
        let method: String = args.param("method", "gravity".into())?;
        if !matches!(method.as_str(), "gravity" | "occurrence" | "kernel") {
            return Err(fail("method is gravity, occurrence or kernel"));
        }
        let out = development_pressure(
            &m.data,
            m.grid.height,
            m.grid.width,
            size,
            &method,
            args.param("gamma", 1.5)?,
            args.param("scaling_factor", 1.0)?,
            args.param("pad", true)?,
        )?;
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), out)))]))
    }
}

/// The 4-connected patches of a mask (scipy's label, in raster order): a label per cell (0 outside) and their
/// count.
fn label4(mask: &[bool], ny: usize, nx: usize) -> (Vec<usize>, usize) {
    let mut labels = vec![0usize; ny * nx];
    let mut n = 0;
    let mut stack = Vec::new();
    for start in 0..ny * nx {
        if !mask[start] || labels[start] != 0 {
            continue;
        }
        n += 1;
        labels[start] = n;
        stack.push(start);
        while let Some(k) = stack.pop() {
            let (y, x) = (k / nx, k % nx);
            let mut visit = |j: usize| {
                if mask[j] && labels[j] == 0 {
                    labels[j] = n;
                    stack.push(j);
                }
            };
            if y > 0 {
                visit(k - nx);
            }
            if y + 1 < ny {
                visit(k + nx);
            }
            if x > 0 {
                visit(k - 1);
            }
            if x + 1 < nx {
                visit(k + 1);
            }
        }
    }
    (labels, n)
}

struct PatchLibrary;

impl Operator for PatchLibrary {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "futures.patch_library",
            "The sizes of the patches developed between two maps (r.futures.calib's patch library through \
             landschaft): the 4-connected groups of cells developed in the end and not at the start (inside a \
             subregion), in cells, sorted; one library per subregion (each patch cut to its part in it), or one \
             for all. The patch_sizes futures.simulation draws from.",
            Access::Global,
        )
        .input(PortSpec::new("start", Type::Map, "developed (1) at the start"))
        .input(PortSpec::new("end", Type::Map, "developed (1) at the end"))
        .input(PortSpec::optional("subregions", Type::Map, "subregions (0 and missing ignored)"))
        .output(PortSpec::new("library", Type::Any, "{ ids, lists }, as futures.simulation's patch_sizes"))
        .output(PortSpec::new("table", Type::Table, "subregion (empty: all), size"))
        .param(PortSpec::optional("per_subregion", Type::Bool, "a library per subregion (default false)"))
        .param(PortSpec::optional("threshold", Type::Number, "the smallest patch area kept (map units; 0)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (m0, m1) = (args.map("start")?, args.map("end")?);
        let sub = args.opt_map("subregions")?;
        let threshold: f64 = args.param("threshold", 0.0)?;
        let (library, t) = patch_library(m0, m1, sub, args.param("per_subregion", false)?, threshold)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        Ok(outputs([("library", Value::Json(library)), ("table", Value::Table(t))]))
    }
}

/// The cells developed in `end` and not at `start`, inside a subregion (all cells without subregions).
fn new_development(m0: &Map, m1: &Map, sub: Option<&Map>) -> Vec<bool> {
    let subv = |k: usize| sub.map_or(1.0, |s| s.data[k]);
    (0..m1.data.len())
        .map(|k| {
            let (s, e, a) = (subv(k), m1.data[k], m0.data[k]);
            (!s.is_nan() && s != 0.0) && (!e.is_nan() && e != 0.0) && (a.is_nan() || a == 0.0)
        })
        .collect()
}

/// The patch library of two maps (landschaft's patch_library): { ids, lists } as futures.simulation's
/// patch_sizes, and as a table (subregion, size).
pub fn patch_library(
    m0: &Map,
    m1: &Map,
    sub: Option<&Map>,
    per_subregion: bool,
    threshold: f64,
) -> std::result::Result<(serde_json::Value, Table), String> {
    {
        let (ny, nx) = (m1.grid.height, m1.grid.width);
        if m0.data.len() != ny * nx || sub.is_some_and(|s| s.data.len() != ny * nx) {
            return Err("the maps differ in size".into());
        }
        let (ew, ns) = (m1.grid.transform[1].abs(), m1.grid.transform[5].abs());
        let subv = |k: usize| sub.map_or(1.0, |s| s.data[k]);
        let diff = new_development(m0, m1, sub);
        let cell = ((ns + ew) / 2.0).powi(2);
        let thr: f64 = threshold - 1e-6;
        let (labels, n) = label4(&diff, ny, nx);
        let sizes_of = |counts: &[usize]| -> Vec<f64> {
            let mut s: Vec<f64> = counts
                .iter()
                .map(|&c| c as f64 * (ew * ns))
                .filter(|&a| a > thr)
                .map(|a| (a / cell).round_ties_even())
                .collect();
            s.sort_by(f64::total_cmp);
            s
        };
        let mut counts = vec![0usize; n + 1];
        for &l in &labels {
            counts[l] += 1;
        }
        let whole = sizes_of(&counts[1..]);
        let mut library = json!({"ids": null, "lists": [whole.clone()]});
        let mut rows: Vec<Vec<f64>> = whole.iter().map(|&s| vec![f64::NAN, s]).collect();
        if per_subregion {
            let mut cats: Vec<f64> = (0..ny * nx).map(subv).filter(|v| !v.is_nan()).collect();
            cats.sort_by(f64::total_cmp);
            cats.dedup();
            if cats.len() > 1 {
                let mut lists = Vec::new();
                rows.clear();
                for &c in &cats {
                    let mut per = vec![0usize; n + 1];
                    for k in 0..ny * nx {
                        if subv(k) == c && labels[k] > 0 {
                            per[labels[k]] += 1;
                        }
                    }
                    let present: Vec<usize> = per[1..].iter().copied().filter(|&p| p > 0).collect();
                    let s = sizes_of(&present);
                    rows.extend(s.iter().map(|&v| vec![c, v]));
                    lists.push(s);
                }
                library = json!({"ids": cats, "lists": lists});
            }
        }
        let mut t = Table::new(vec!["subregion".into(), "size".into()]);
        t.rows = rows;
        Ok((library, t))
    }
}

/// The area and perimeter of every 4-connected patch of a mask (landschaft's _patch_geometry: r.clump and
/// r.object.geometry -m), in the order of their labels: a cell's edge on another patch or outside adds its length
/// (ew above and below, ns left and right).
fn patch_geometry(mask: &[bool], ny: usize, nx: usize, ew: f64, ns: f64) -> Vec<[f64; 2]> {
    let (labels, n) = label4(mask, ny, nx);
    let mut count = vec![0usize; n + 1];
    // the edges per label: above, below, left, right
    let mut edges = vec![[0usize; 4]; n + 1];
    for k in 0..ny * nx {
        let l = labels[k];
        if l == 0 {
            continue;
        }
        count[l] += 1;
        let (y, x) = (k / nx, k % nx);
        let other = |j: Option<usize>| j.is_none_or(|j| labels[j] != l);
        let e = &mut edges[l];
        e[0] += usize::from(other((y > 0).then(|| k - nx)));
        e[1] += usize::from(other((y + 1 < ny).then(|| k + nx)));
        e[2] += usize::from(other((x > 0).then(|| k - 1)));
        e[3] += usize::from(other((x + 1 < nx).then(|| k + 1)));
    }
    (1..=n)
        .map(|l| {
            let e = edges[l];
            let perimeter = 0.0 + e[0] as f64 * ew + e[1] as f64 * ew + e[2] as f64 * ns + e[3] as f64 * ns;
            [count[l] as f64 * (ew * ns), perimeter]
        })
        .collect()
}

/// numpy's histogram of `bins` equal bins over a range, as densities (numpy's edges by linspace, its bins of the
/// values in the range), times 100.
fn histogram(values: &[f64], bins: usize, (lo, hi): (f64, f64)) -> std::result::Result<Vec<f64>, String> {
    if bins == 0 {
        return Err("a histogram of no bins: the observed patches are too alike (their range below a bin)".into());
    }
    let (first, last) = if lo == hi { (lo - 0.5, hi + 0.5) } else { (lo, hi) };
    // np.linspace(first, last, bins + 1)
    let delta = last - first;
    let step = delta / bins as f64;
    let mut edges: Vec<f64> = (0..=bins)
        .map(|i| if step == 0.0 { i as f64 / bins as f64 * delta + first } else { i as f64 * step + first })
        .collect();
    edges[bins] = last;
    let mut n = vec![0u64; bins];
    for &v in values.iter().filter(|v| **v >= first && **v <= last) {
        let mut i = ((v - first) / delta * bins as f64) as usize;
        if i == bins {
            i -= 1;
        }
        if v < edges[i] {
            i -= 1;
        }
        if v >= edges[i + 1] && i != bins - 1 {
            i += 1;
        }
        n[i] += 1;
    }
    let total: u64 = n.iter().sum();
    Ok((0..bins).map(|i| n[i] as f64 / (edges[i + 1] - edges[i]) / total as f64 * 100.0).collect())
}

/// The histograms' chi-square distance (landschaft's _compare_histograms): half the sum, over the bins not empty
/// in both, of (a - b)^2 / (a + b), numpy's.
fn compare_histograms(a: &[f64], b: &[f64]) -> f64 {
    let v: Vec<f64> =
        a.iter().zip(b).map(|(&x, &y)| if x == 0.0 && y == 0.0 { 0.0 } else { (x - y) * (x - y) / (x + y) }).collect();
    0.5 * crate::change::numpy_sum(&v)
}

/// The patches' areas (in cells) and compactness (perimeter / (2 sqrt(pi area))), of the patches above a
/// threshold.
fn shapes(g: &[[f64; 2]], thr: f64, cell: f64) -> (Vec<f64>, Vec<f64>) {
    g.iter().filter(|p| p[0] > thr).map(|p| (p[0] / cell, p[1] / (2.0 * (std::f64::consts::PI * p[0]).sqrt()))).unzip()
}

fn min_max(v: &[f64]) -> (f64, f64) {
    (v.iter().copied().fold(f64::INFINITY, f64::min), v.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

/// The parameters of a FUTURES calibration (r.futures.calib) that futures.simulation does not take.
const CALIBRATION: [&str; 7] =
    ["compactness_means", "compactness_ranges", "discount_factors", "repeat", "seed", "threshold", "per_subregion"];

struct Calibrate;

impl Operator for Calibrate {
    fn signature(&self) -> OpSignature {
        let sim = crate::futures::Simulation.signature();
        let mut sig = OpSignature::new(
            "futures.calibrate",
            "The calibration of FUTURES' patches (r.futures.calib through landschaft's calibrate_patches): the \
             patches developed between the start and the end give the patch library and the histograms of patch \
             area and compactness (perimeter / (2 sqrt(pi area))); every combination of the compactness means, \
             ranges and discount factors is simulated repeat times from the start (futures.simulation's settings, \
             seeds (seed + k) 10000 + i) and its patches compared with the observed ones by the histograms' \
             chi-square distance. The combinations from the best, as landschaft's.",
            Access::Global,
        );
        sig.inputs = sim.inputs.into_iter().filter(|p| p.name != "seed").collect();
        let replaced = ["patch_sizes", "compactness_mean", "compactness_range", "discount_factor"];
        sig.params = sim.params.into_iter().filter(|p| !replaced.contains(&p.name.as_str())).collect();
        sig.input(PortSpec::new("end", Type::Map, "developed (1) at the end of the observed period"))
            .output(PortSpec::new(
                "table",
                Type::Table,
                "per combination, from the best: discount_factor, compactness_mean, compactness_range, area_error, \
                 compactness_error (each over its largest) and combined_error (their mean)",
            ))
            .output(PortSpec::new(
                "library",
                Type::Any,
                "the observed patch library, as futures.simulation's patch_sizes",
            ))
            .param(PortSpec::optional("compactness_means", Type::Any, "the compactness means to try (default [0.4])"))
            .param(PortSpec::optional("compactness_ranges", Type::Any, "the ranges to try (default [0.05])"))
            .param(PortSpec::optional("discount_factors", Type::Any, "the discount factors to try (default [1])"))
            .param(PortSpec::optional("repeat", Type::Integer, "simulations per combination (default 10)"))
            .param(PortSpec::optional("seed", Type::Integer, "the first combination's seed (default 1)"))
            .param(PortSpec::optional("threshold", Type::Number, "the smallest patch area kept (map units; 0)"))
            .param(PortSpec::optional("per_subregion", Type::Bool, "a patch library per subregion (default false)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let (m0, m1) = (args.map("landscape")?, args.map("end")?);
        let sub = args.opt_map("subregions")?;
        let (ny, nx) = (m1.grid.height, m1.grid.width);
        let (ew, ns) = (m1.grid.transform[1].abs(), m1.grid.transform[5].abs());
        let cell = ((ns + ew) / 2.0).powi(2);
        let threshold: f64 = args.param("threshold", 0.0)?;
        let thr = threshold - 1e-6;
        let (library, _) = patch_library(m0, m1, sub, args.param("per_subregion", false)?, threshold).map_err(err)?;
        // the observed patches' histograms
        let observed = patch_geometry(&new_development(m0, m1, sub), ny, nx, ew, ns);
        let (area, compact) = shapes(&observed, thr, cell);
        if area.is_empty() {
            return Err(err("no patches developed between the start and the end".into()));
        }
        let (range_a, range_c) = (min_max(&area), min_max(&compact));
        let bins_a = ((range_a.1 - range_a.0) / 1.0) as usize;
        let bins_c = ((range_c.1 - range_c.0) / 0.1) as usize;
        let h_area = histogram(&area, bins_a, range_a).map_err(err)?;
        let h_comp = histogram(&compact, bins_c, range_c).map_err(err)?;
        let list = |name: &str, default: f64| -> Result<Vec<f64>> {
            match args.params.get(name) {
                None | Some(serde_json::Value::Null) => Ok(vec![default]),
                Some(serde_json::Value::Number(x)) => Ok(vec![x.as_f64().unwrap()]),
                Some(v) => serde_json::from_value(v.clone()).map_err(|e| err(format!("{name}: {e}"))),
            }
        };
        let (means, ranges, discounts) =
            (list("compactness_means", 0.4)?, list("compactness_ranges", 0.05)?, list("discount_factors", 1.0)?);
        let repeat: i64 = args.param("repeat", 10)?;
        if repeat < 1 {
            return Err(err("repeat: at least 1".into()));
        }
        let mut s: i64 = args.param("seed", 1)?;
        let mut base = args.params.clone();
        for k in CALIBRATION {
            base.remove(k);
        }
        base.insert("patch_sizes".into(), library.clone());
        let mut inputs = args.inputs.clone();
        inputs.remove("end");
        let mut rows: Vec<[f64; 5]> = Vec::new();
        for &mean in &means {
            for &rng in &ranges {
                for &disc in &discounts {
                    let mut p = base.clone();
                    p.insert("compactness_mean".into(), json!(mean));
                    p.insert("compactness_range".into(), json!(rng));
                    p.insert("discount_factor".into(), json!(disc));
                    let settings: crate::futures::Settings =
                        serde_json::from_value(serde_json::Value::Object(p.clone())).map_err(|e| err(e.to_string()))?;
                    let (mut d_area, mut d_comp) = (0.0, 0.0);
                    for i in 0..repeat {
                        let mut ins = inputs.clone();
                        ins.insert("seed".into(), Value::Integer(s * 10000 + i));
                        let sim_args = Args { node, inputs: ins, params: &p };
                        // the runs' warnings left out, as landschaft's
                        let run = crate::futures::simulate(&mut Ctx::default(), &settings, &sim_args)?;
                        let last = run.development.last().ok_or_else(|| err("the demand has no steps".into()))?;
                        let developed: Vec<bool> = last.data.iter().map(|&v| !v.is_nan() && v > 0.0).collect();
                        let (a, c) = shapes(&patch_geometry(&developed, ny, nx, ew, ns), thr, cell);
                        d_area += compare_histograms(&h_area, &histogram(&a, bins_a, range_a).map_err(err)?);
                        d_comp += compare_histograms(&h_comp, &histogram(&c, bins_c, range_c).map_err(err)?);
                    }
                    rows.push([disc, d_area / repeat as f64, mean, rng, d_comp / repeat as f64]);
                    s += 1;
                }
            }
        }
        let max_a = rows.iter().map(|r| r[1]).fold(f64::NEG_INFINITY, f64::max);
        let max_c = rows.iter().map(|r| r[4]).fold(f64::NEG_INFINITY, f64::max);
        let mut out: Vec<Vec<f64>> = rows
            .iter()
            .map(|r| {
                let (na, nc) = (r[1] / max_a, r[4] / max_c);
                vec![r[0], r[2], r[3], na, nc, (na + nc) / 2.0]
            })
            .collect();
        out.sort_by(|a, b| a[5].total_cmp(&b[5]));
        let columns = [
            "discount_factor",
            "compactness_mean",
            "compactness_range",
            "area_error",
            "compactness_error",
            "combined_error",
        ];
        let mut t = Table::new(columns.map(String::from).to_vec());
        t.rows = out;
        Ok(outputs([("table", Value::Table(t)), ("library", Value::Json(library))]))
    }
}
