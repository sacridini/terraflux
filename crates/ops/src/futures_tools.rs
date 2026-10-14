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
        let (ny, nx) = (m1.grid.height, m1.grid.width);
        let sub = args.opt_map("subregions")?;
        if m0.data.len() != ny * nx || sub.is_some_and(|s| s.data.len() != ny * nx) {
            return Err(Error::msg(format!("node {:?}: the maps differ in size", args.node)));
        }
        let (ew, ns) = (m1.grid.transform[1].abs(), m1.grid.transform[5].abs());
        let subv = |k: usize| sub.map_or(1.0, |s| s.data[k]);
        let diff: Vec<bool> = (0..ny * nx)
            .map(|k| {
                let (s, e, a) = (subv(k), m1.data[k], m0.data[k]);
                (!s.is_nan() && s != 0.0) && (!e.is_nan() && e != 0.0) && (a.is_nan() || a == 0.0)
            })
            .collect();
        let cell = ((ns + ew) / 2.0).powi(2);
        let thr: f64 = args.param::<f64>("threshold", 0.0)? - 1e-6;
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
        if args.param("per_subregion", false)? {
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
        Ok(outputs([("library", Value::Json(library)), ("table", Value::Table(t))]))
    }
}
