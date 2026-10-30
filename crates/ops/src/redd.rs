//! Verra's VT0007, the Unplanned Deforestation Allocation (UDef-A) of a jurisdiction's expected deforestation to its
//! forest, as Clark Labs' UDef-ARP implements it: the negligible risk threshold of the distance to the forest's edge
//! (`redd.nrt`), the vulnerability in classes of geometric widths (`redd.vulnerability`: the benchmark from that
//! distance, or an alternative map of vulnerability), the relative frequency of deforestation by class and
//! subdivision and its density (`redd.fit`), that density in another period adjusted to the activity data, never
//! above a cell (`redd.allocate`), and the predicted against the actual deforestation in the cells of an assessment
//! grid (`redd.assess`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use crate::change::numpy_sum;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Nrt));
    r.add(Arc::new(Vulnerability));
    r.add(Arc::new(Fit));
    r.add(Arc::new(Allocate));
    r.add(Arc::new(Assess));
}

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

/// A map's values with missing cells as 0 (UDef-ARP's binary maps).
fn zeroed(m: &Map) -> Vec<f64> {
    m.data.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect()
}

/// A cell's area in hectares (its sides in metres).
fn cell_ha(g: &Grid) -> f64 {
    g.transform[1] * g.transform[5].abs() / 10_000.0
}

/// The same size as `like`, else an error.
fn same(args: &Args, a: &Map, like: &Map, what: &str) -> Result<()> {
    if a.data.len() != like.data.len() {
        return Err(fail(args, format!("{what} and the other maps differ in size")));
    }
    Ok(())
}

/// The jurisdiction's mask (non-zero), all of the map without one.
fn mask_of(args: &Args, like: &Map, port: &str) -> Result<Vec<f64>> {
    match args.opt_map(port)? {
        Some(m) => {
            same(args, m, like, port)?;
            Ok(zeroed(m))
        }
        None => Ok(vec![1.0; like.data.len()]),
    }
}

// -- the negligible risk threshold ------------------------------------------------------------------------------------

/// numpy's arange(start, stop, step) of floats: the second value start + step, the others start + i (second - start).
fn arange(start: f64, stop: f64, step: f64) -> Vec<f64> {
    let n = ((stop - start) / step).ceil().max(0.0) as usize;
    let second = start + step;
    let delta = second - start;
    (0..n).map(|i| if i == 1 { second } else { start + i as f64 * delta }).collect()
}

/// VulnerabilityMap.nrt_calculation: the distance below which 99.5 % of the deforestation happened (the middle of the
/// first bin, a cell wide, where the cumulative share of the deforested cells' distances reaches 0.995).
pub fn nrt(distance: &[f64], deforestation: &[f64], mask: &[f64], cell: f64) -> Option<f64> {
    let mut v: Vec<f64> = (0..distance.len())
        .map(|i| {
            let d = if distance[i].is_nan() { 0.0 } else { distance[i] };
            d * mask[i] * deforestation[i]
        })
        .filter(|v| *v != 0.0)
        .collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let (min, max) = (v[0], v[v.len() - 1]);
    let edges = arange(min, max + cell, cell);
    if edges.len() < 2 {
        return None;
    }
    // np.histogram over the edges: [e_i, e_i+1), the last bin closed
    let last = edges.len() - 1;
    let cum: Vec<usize> = edges
        .iter()
        .enumerate()
        .map(|(j, &e)| if j < last { v.partition_point(|x| *x < e) } else { v.partition_point(|x| *x <= e) })
        .collect();
    let hist: Vec<f64> = cum.windows(2).map(|w| (w[1] - w[0]) as f64).collect();
    let total: f64 = hist.iter().sum();
    let mut acc = 0.0;
    let mut index = 0;
    for (i, h) in hist.iter().enumerate() {
        acc += h / total;
        if acc >= 0.995 {
            index = i;
            break;
        }
    }
    Some(((edges[index] + edges[index + 1]) / 2.0).trunc())
}

struct Nrt;

impl Operator for Nrt {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "redd.nrt",
            "VT0007's negligible risk threshold (UDef-ARP's nrt_calculation): the distance to the forest's edge \
             below which 99.5 % of the deforestation of a period happened, the middle of the first bin (a cell \
             wide) where the cumulative share of the deforested cells reaches 0.995, in whole map units.",
            Access::Global,
        )
        .input(PortSpec::new("distance", Type::Map, "the distance of the forest's cells to the edge, in map units"))
        .input(PortSpec::new("deforestation", Type::Map, "the deforestation of the period (1)"))
        .input(PortSpec::optional("mask", Type::Map, "the jurisdiction (non-zero; default all the map)"))
        .output(PortSpec::new("nrt", Type::Number, "the negligible risk threshold, in map units"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (d, def) = (args.map("distance")?, args.map("deforestation")?);
        same(args, def, d, "deforestation")?;
        let mask = mask_of(args, d, "mask")?;
        let n = nrt(&d.data, &zeroed(def), &mask, d.grid.transform[1])
            .ok_or_else(|| fail(args, "no deforested cell of the jurisdiction has a distance"))?;
        Ok(outputs([("nrt", Value::Number(n))]))
    }
}

// -- the vulnerability classes ----------------------------------------------------------------------------------------

/// VulnerabilityMap.geometric_classification: the forest's distance to the edge in `n` classes of geometric widths
/// from the cell size to the threshold (2 the nearest the edge, n + 1 the farthest below it; 1 beyond it).
pub fn benchmark_classes(distance: &[f64], mask: &[f64], nrt: f64, n: usize, cell: f64) -> Vec<f64> {
    let ll = cell.trunc();
    let ul = nrt.trunc();
    let r = (ll / ul).powf(1.0 / n as f64);
    let mut risk: Vec<[f64; 2]> = (0..n).map(|i| [ul * r.powf(i as f64), ul * r.powf((i + 1) as f64)]).collect();
    risk[n - 1][1] = ll;
    risk[0][0] = ul;
    let mut a: Vec<f64> = distance.iter().zip(mask).map(|(d, m)| if d.is_nan() { 0.0 } else { d * m }).collect();
    a.iter_mut().filter(|v| **v >= ul).for_each(|v| *v = 1.0);
    for (i, [lower, upper]) in risk.iter().enumerate() {
        a.iter_mut().filter(|v| *lower > **v && **v >= *upper).for_each(|v| *v = (i + 2) as f64);
    }
    a.iter().map(|v| v.trunc()).collect()
}

/// VulnerabilityMap.geometric_classification_alternative: a map of vulnerability, rescaled to 1-2 by its largest
/// value, in `n` classes of geometric widths (1 the lowest), inside the jurisdiction's forest. As UDef-ARP, a
/// float32 map is rescaled in float32 and its largest values (2 exactly) are left at 2.
pub fn alternative_classes(v: &[f64], mask: &[f64], forest: &[f64], n: usize) -> Vec<f64> {
    let max = v.iter().copied().filter(|x| !x.is_nan()).fold(f64::NEG_INFINITY, f64::max);
    let single = v.iter().all(|x| x.is_nan() || f64::from(*x as f32) == *x);
    let r = 2f64.powf(1.0 / n as f64);
    let mut risk: Vec<[f64; 2]> =
        (0..n).rev().map(|i| [1.0 + (2.0 - r.powf(i as f64)), 1.0 + (2.0 - r.powf((i + 1) as f64))]).collect();
    risk[0][1] = 1.0;
    let mut a: Vec<f64> = (0..v.len())
        .map(|i| {
            let x = if v[i].is_nan() { 0.0 } else { v[i] };
            let scaled =
                if single { f64::from(1.0f32 + (x as f32 * 1.0f32) / max as f32) } else { 1.0 + x * 1.0 / max };
            let m = mask[i] * forest[i];
            if single { f64::from(scaled as f32 * m as f32) } else { scaled * m }
        })
        .collect();
    for (i, [upper, lower]) in risk.iter().enumerate() {
        a.iter_mut().filter(|x| *upper > **x && **x >= *lower).for_each(|x| *x = (i + 1) as f64);
    }
    a.iter().map(|v| v.trunc()).collect()
}

struct Vulnerability;

impl Operator for Vulnerability {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "redd.vulnerability",
            "VT0007's vulnerability map in classes of geometric widths (UDef-ARP's geometric classification): the \
             benchmark from the distance to the forest's edge (classes 2 to n + 1 from the edge to the negligible \
             risk threshold, 1 beyond it), or an alternative from a map of vulnerability (a potential, a \
             probability; classes 1 to n from the lowest), inside the jurisdiction; 0 elsewhere.",
            Access::Global,
        )
        .input(PortSpec::optional("distance", Type::Map, "the benchmark: the distance to the forest's edge"))
        .input(PortSpec::optional("nrt", Type::Number, "with the distance: the negligible risk threshold"))
        .input(PortSpec::optional("vulnerability", Type::Map, "or an alternative: a map of vulnerability"))
        .input(PortSpec::optional("forest", Type::Map, "with a vulnerability: the forest (non-zero)"))
        .input(PortSpec::optional("mask", Type::Map, "the jurisdiction (non-zero; default all the map)"))
        .output(PortSpec::new("classes", Type::Map, "the vulnerability class of each cell, 0 outside"))
        .param(PortSpec::optional("classes", Type::Integer, "the classes of geometric widths (default 29)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let n: usize = args.param("classes", 29)?;
        if n == 0 {
            return Err(fail(args, "classes: 1 or more"));
        }
        let out = match (args.opt_map("distance")?, args.opt_map("vulnerability")?) {
            (Some(_), Some(_)) => return Err(fail(args, "the distance (benchmark) or a vulnerability, not both")),
            (Some(d), None) => {
                let nrt = args.number("nrt")?;
                let cell = d.grid.transform[1];
                if nrt.trunc() <= cell.trunc() {
                    return Err(fail(args, "the threshold must be above the cell size"));
                }
                Map::new(d.grid.clone(), benchmark_classes(&d.data, &mask_of(args, d, "mask")?, nrt, n, cell))
            }
            (None, Some(v)) => {
                let forest = args.map("forest")?;
                same(args, forest, v, "forest")?;
                Map::new(v.grid.clone(), alternative_classes(&v.data, &mask_of(args, v, "mask")?, &zeroed(forest), n))
            }
            (None, None) => return Err(fail(args, "connect the distance to the edge, or a map of vulnerability")),
        };
        Ok(outputs([("classes", Value::Map(out))]))
    }
}

// -- the relative frequency and the density -------------------------------------------------------------------------

/// The modelling region of every cell: class × 1000 + subdivision (0 outside the classes).
fn regions_of(classes: &[f64], subdivisions: Option<&[f64]>) -> Vec<i64> {
    (0..classes.len())
        .map(|i| {
            let c = if classes[i].is_nan() { 0 } else { classes[i] as i64 };
            if c <= 0 {
                return 0;
            }
            let s = subdivisions.map_or(0, |s| if s[i].is_nan() { 0 } else { s[i] as i64 });
            i64::from((c * 1000 + s) as i16)
        })
        .collect()
}

/// The relative frequency table: a row per modelling region (increasing): its deforested cells, its cells and their
/// ratio.
fn frequency(regions: &[i64], deforestation: &[f64]) -> Vec<(i64, f64, f64, f64)> {
    let mut ids: Vec<i64> = regions.iter().copied().filter(|r| *r != 0).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut rows: Vec<(i64, f64, f64, f64)> = ids.iter().map(|&id| (id, 0.0, 0.0, 0.0)).collect();
    for (i, &r) in regions.iter().enumerate() {
        if r != 0 {
            let k = ids.binary_search(&r).unwrap();
            rows[k].2 += 1.0;
            if deforestation[i] != 0.0 && (r as f64 * deforestation[i]) != 0.0 {
                rows[k].1 += 1.0;
            }
        }
    }
    rows.iter_mut().for_each(|r| r.3 = r.1 / r.2);
    rows
}

fn table_of(rows: &[(i64, f64, f64, f64)]) -> Table {
    let mut t =
        Table::new(["id", "class", "subdivision", "deforested", "cells", "frequency"].map(String::from).to_vec());
    for &(id, d, a, f) in rows {
        t.rows.push(vec![id as f64, (id / 1000) as f64, (id % 1000) as f64, d, a, f]);
    }
    t
}

struct Fit;

impl Operator for Fit {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "redd.fit",
            "VT0007's fit of a vulnerability map to a period's deforestation (UDef-ARP's fitting): the relative \
             frequency of deforestation in every modelling region (a vulnerability class in a subdivision of the \
             jurisdiction) and the density it gives every cell, in hectares.",
            Access::Global,
        )
        .input(PortSpec::new("classes", Type::Map, "the vulnerability classes (redd.vulnerability)"))
        .input(PortSpec::new("deforestation", Type::Map, "the deforestation of the period (1)"))
        .input(PortSpec::optional("subdivisions", Type::Map, "the subdivisions of the jurisdiction (ids 0-999)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per modelling region: id (class × 1000 + subdivision), class, subdivision, deforested and all \
             its cells, frequency",
        ))
        .output(PortSpec::new("density", Type::Map, "the fitted density of deforestation, hectares per cell"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (c, def) = (args.map("classes")?, args.map("deforestation")?);
        same(args, def, c, "deforestation")?;
        let sub = args.opt_map("subdivisions")?;
        if let Some(s) = sub {
            same(args, s, c, "subdivisions")?;
        }
        let regions = regions_of(&c.data, sub.map(|s| &s.data[..]));
        let rows = frequency(&regions, &zeroed(def));
        let ha = cell_ha(&c.grid);
        let density: Vec<f64> = regions
            .iter()
            .map(|r| {
                let f = if *r == 0 { 0.0 } else { rows[rows.binary_search_by_key(r, |x| x.0).unwrap()].3 };
                // as UDef-ARP writes it: float32
                f64::from((f * ha) as f32)
            })
            .collect();
        Ok(outputs([
            ("table", Value::Table(table_of(&rows))),
            ("density", Value::Map(Map::new(c.grid.clone(), density))),
        ]))
    }
}

// -- the allocation -------------------------------------------------------------------------------------------------

/// pandas' groupby sum (Kahan's compensated summation).
fn kahan(values: impl Iterator<Item = f64>) -> f64 {
    let (mut sum, mut comp) = (0.0f64, 0.0f64);
    for v in values {
        let y = v - comp;
        let t = sum + y;
        comp = t - sum - y;
        if comp.is_nan() {
            comp = 0.0;
        }
        sum = t;
    }
    sum
}

/// AllocationTool.calculate_missing_bins_rf: the frequency of a region of the prediction missing from the fit, the
/// average of its class's fitted regions weighted by their cells.
fn fill_missing(fit: &[(i64, f64, f64, f64)], ids: &[i64]) -> std::result::Result<Vec<(i64, f64)>, String> {
    let mut out: Vec<(i64, f64)> = Vec::new();
    for &id in ids {
        if let Some(r) = fit.iter().find(|r| r.0 == id) {
            out.push((id, r.3));
            continue;
        }
        let zone = id / 1000;
        let rows: Vec<&(i64, f64, f64, f64)> = fit.iter().filter(|r| r.0 / 1000 == zone).collect();
        if rows.is_empty() {
            return Err(format!("the vulnerability class {zone} has no cells in the fit"));
        }
        let total = kahan(rows.iter().map(|r| r.2 * r.3));
        let area = kahan(rows.iter().map(|r| r.2));
        out.push((id, total / area));
    }
    Ok(out)
}

struct Allocate;

impl Operator for Allocate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "redd.allocate",
            "VT0007's allocation (UDef-ARP's prediction): the fitted relative frequency of every modelling region \
             (a region the fit lacks takes its class's average over the subdivisions) gives each cell a density; \
             the densities are scaled to the activity data, the expected deforestation (or a period's observed \
             one, to confirm the model), never above a cell's area, again until the adjustment ratio is 1.00001 or \
             less.",
            Access::Global,
        )
        .input(PortSpec::new("classes", Type::Map, "the vulnerability classes of the period"))
        .input(PortSpec::new("table", Type::Table, "the relative frequency table (redd.fit)"))
        .input(PortSpec::optional("subdivisions", Type::Map, "the subdivisions, as in the fit"))
        .input(PortSpec::optional("expected", Type::Number, "the expected deforestation, hectares"))
        .input(PortSpec::optional("deforestation", Type::Map, "or the period's observed deforestation (1)"))
        .output(PortSpec::new("density", Type::Map, "the expected deforestation of each cell, hectares"))
        .output(PortSpec::new(
            "info",
            Type::Table,
            "adjustment_ratio, iterations, expected_ha, allocated_ha, filled (regions the fit lacked)",
        ))
        .param(PortSpec::optional("max_iterations", Type::Integer, "the adjustments at most (default 50)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let c = args.map("classes")?;
        let Some(Value::Table(t)) = args.get("table") else { return Err(fail(args, "connect the fit's table")) };
        let col = |n: &str| t.column(n).ok_or_else(|| fail(args, format!("the table has no column {n:?}")));
        let (ids, defs, cells, freq) = (col("id")?, col("deforested")?, col("cells")?, col("frequency")?);
        let fit: Vec<(i64, f64, f64, f64)> =
            (0..ids.len()).map(|k| (ids[k] as i64, defs[k], cells[k], freq[k])).collect();
        let sub = args.opt_map("subdivisions")?;
        if let Some(s) = sub {
            same(args, s, c, "subdivisions")?;
        }
        let regions = regions_of(&c.data, sub.map(|s| &s.data[..]));
        let mut present: Vec<i64> = regions.iter().copied().filter(|r| *r != 0).collect();
        present.sort_unstable();
        present.dedup();
        let filled = present.iter().filter(|id| !fit.iter().any(|r| r.0 == **id)).count();
        let freqs = fill_missing(&fit, &present).map_err(|e| fail(args, e))?;
        let ha = cell_ha(&c.grid);
        let mut density: Vec<f64> = regions
            .iter()
            .map(|r| if *r == 0 { 0.0 } else { freqs[freqs.binary_search_by_key(r, |x| x.0).unwrap()].1 * ha })
            .collect();
        let target = match (args.get("expected"), args.opt_map("deforestation")?) {
            (Some(_), Some(_)) => return Err(fail(args, "the expected deforestation or a period's, not both")),
            (Some(_), None) => args.number("expected")?,
            (None, Some(d)) => {
                same(args, d, c, "deforestation")?;
                numpy_sum(&zeroed(d).iter().map(|v| v * ha).collect::<Vec<_>>())
            }
            (None, None) => return Err(fail(args, "connect the expected deforestation (or a period's)")),
        };
        let max_iter: usize = args.param("max_iterations", 50)?;
        let mut ar = target / numpy_sum(&density);
        let mut it = 0;
        while ar > 1.00001 && it <= max_iter {
            density.iter_mut().for_each(|v| *v = (ar * *v).min(ha));
            ar = target / numpy_sum(&density);
            it += 1;
        }
        if it > max_iter {
            return Err(fail(
                args,
                format!(
                    "the adjustment ratio is still {ar} after {max_iter} iterations: the cells cannot take the \
                     expected deforestation (raise max_iterations, or lower the expectation)"
                ),
            ));
        }
        // the last ratio applied once more, as UDef-ARP writes the map (float32)
        let out: Vec<f64> = density.iter().map(|v| f64::from((ar * v).min(ha) as f32)).collect();
        let allocated = numpy_sum(&out);
        let info = Table {
            columns: ["adjustment_ratio", "iterations", "expected_ha", "allocated_ha", "filled"]
                .map(String::from)
                .to_vec(),
            rows: vec![vec![ar, it as f64, target, allocated, filled as f64]],
            ..Default::default()
        };
        Ok(outputs([("density", Value::Map(Map::new(c.grid.clone(), out))), ("info", Value::Table(info))]))
    }
}

// -- the assessment ---------------------------------------------------------------------------------------------------

/// The median of values (the mean of the two middle ones of an even number).
fn median(v: &[f64]) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let n = s.len();
    if n == 0 {
        return f64::NAN;
    }
    if n % 2 == 1 { s[n / 2] } else { (s[n / 2 - 1] + s[n / 2]) / 2.0 }
}

/// Theil and Sen's slope (the median of the slopes between pairs of different x) and intercept (scipy's
/// theilslopes: the median of y minus the slope times the median of x).
fn theil_sen(x: &[f64], y: &[f64]) -> (f64, f64) {
    let mut slopes = Vec::new();
    for i in 0..x.len() {
        for j in 0..i {
            if x[i] != x[j] {
                slopes.push((y[i] - y[j]) / (x[i] - x[j]));
            }
        }
    }
    let slope = median(&slopes);
    (slope, median(y) - slope * median(x))
}

struct Assess;

impl Operator for Assess {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "redd.assess",
            "VT0007's assessment of an allocation (UDef-ARP's model evaluation): the predicted and the actual \
             deforestation in the square cells of an assessment grid wholly inside the jurisdiction (centred on \
             the grid's nodes from the map's corner, as UDef-ARP's Thiessen polygons), and their agreement: the \
             mean and median absolute errors (also as shares of a cell), the intersection over union, R², the \
             ordinary least squares and Theil-Sen lines.",
            Access::Global,
        )
        .input(PortSpec::new("density", Type::Map, "the predicted deforestation of each cell, hectares"))
        .input(PortSpec::new("deforestation", Type::Map, "the actual deforestation (1)"))
        .input(PortSpec::optional("mask", Type::Map, "the jurisdiction (non-zero; default all the map)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per assessment cell: cell, x, y (its centre), actual_ha, predicted_ha, residual_ha",
        ))
        .output(PortSpec::new(
            "metrics",
            Type::Table,
            "samples, mae, mae_percent, medae, medae_percent, iou_percent, agreement, difference, r2, ols_slope, \
             ols_intercept, theil_sen_slope, theil_sen_intercept",
        ))
        .output(PortSpec::new("residuals", Type::Map, "each assessment cell's residual (predicted - actual), ha"))
        .param(PortSpec::new("grid_area", Type::Number, "the area of an assessment cell, hectares"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (p, def) = (args.map("density")?, args.map("deforestation")?);
        same(args, def, p, "deforestation")?;
        let mask = mask_of(args, p, "mask")?;
        let g = &p.grid;
        let grid_area: f64 = args.param("grid_area", f64::NAN)?;
        let px = g.transform[1].trunc();
        let size = ((grid_area * 10_000.0).sqrt().trunc() / px).floor() as i64;
        if !(size >= 1) {
            return Err(fail(args, "grid_area: at least a cell's area, hectares"));
        }
        let (nx, ny) = (g.width as i64, g.height as i64);
        let ha = cell_ha(g);
        let half = size as f64 / 2.0;
        // every square centred on a node (k, j) of the grid of `size` cells from the corner: its mask's area, and
        // the cells whose centres it holds (GDAL's rule: from its left and top edges, short of the others)
        struct Square {
            x: f64,
            y: f64,
            area: f64,
            cells: Vec<usize>,
        }
        let mut squares = Vec::new();
        let mut j = -size;
        while j <= ny + size {
            let mut k = -size;
            while k <= nx + size {
                let (cx, cy) = (k as f64, j as f64);
                let (x0, x1, y0, y1) = (cx - half, cx + half, cy - half, cy + half);
                let (c0, c1) = ((x0.floor().max(0.0)) as i64, (x1.ceil().min(nx as f64)) as i64);
                let (r0, r1) = ((y0.floor().max(0.0)) as i64, (y1.ceil().min(ny as f64)) as i64);
                let mut area = 0.0;
                let mut cells = Vec::new();
                for r in r0..r1 {
                    for c in c0..c1 {
                        let i = (r * nx + c) as usize;
                        if mask[i] == 0.0 {
                            continue;
                        }
                        let ox = (x1.min((c + 1) as f64) - x0.max(c as f64)).max(0.0);
                        let oy = (y1.min((r + 1) as f64) - y0.max(r as f64)).max(0.0);
                        area += ox * oy;
                        let (mx, my) = (c as f64 + 0.5, r as f64 + 0.5);
                        if mx >= x0 && mx < x1 && my >= y0 && my < y1 {
                            cells.push(i);
                        }
                    }
                }
                if area > 0.0 {
                    squares.push(Square { x: cx, y: cy, area, cells });
                }
                k += size;
            }
            j += size;
        }
        let max_area = squares.iter().map(|s| s.area).fold(0.0, f64::max);
        squares.retain(|s| s.area / max_area > 0.999);
        if squares.is_empty() {
            return Err(fail(args, "no assessment cell fits in the jurisdiction: a smaller grid_area"));
        }
        let mut t =
            Table::new(["cell", "x", "y", "actual_ha", "predicted_ha", "residual_ha"].map(String::from).to_vec());
        let mut residuals = vec![f64::NAN; p.data.len()];
        let (mut xs, mut ys) = (Vec::new(), Vec::new());
        let tr = &g.transform;
        for (n, s) in squares.iter().enumerate() {
            let actual = s.cells.iter().filter(|&&i| def.data[i] != 0.0 && !def.data[i].is_nan()).count() as f64 * ha;
            let predicted: f64 = s.cells.iter().map(|&i| if p.data[i].is_nan() { 0.0 } else { p.data[i] }).sum();
            let (gx, gy) = (tr[0] + s.x * tr[1], tr[3] + s.y * tr[5]);
            t.rows.push(vec![(n + 1) as f64, gx, gy, actual, predicted, predicted - actual]);
            for &i in &s.cells {
                residuals[i] = predicted - actual;
            }
            xs.push(actual);
            ys.push(predicted);
        }
        let m = xs.len() as f64;
        let diff: Vec<f64> = xs.iter().zip(&ys).map(|(a, b)| (a - b).abs()).collect();
        let medae = median(&diff);
        let difference: f64 = diff.iter().sum();
        let mae = difference / m;
        let agreement: f64 = xs.iter().zip(&ys).map(|(a, b)| a.min(*b)).sum();
        let union: f64 = xs.iter().zip(&ys).map(|(a, b)| a.max(*b)).sum();
        let iou = if union == 0.0 { 0.0 } else { agreement / union * 100.0 };
        let (mx, my) = (xs.iter().sum::<f64>() / m, ys.iter().sum::<f64>() / m);
        let sxy: f64 = xs.iter().zip(&ys).map(|(a, b)| (a - mx) * (b - my)).sum();
        let sxx: f64 = xs.iter().map(|a| (a - mx) * (a - mx)).sum();
        let syy: f64 = ys.iter().map(|b| (b - my) * (b - my)).sum();
        let slope = sxy / sxx;
        let r = sxy / (sxx * syy).sqrt();
        let (ts, ti) = theil_sen(&xs, &ys);
        let area = grid_area.trunc();
        let metrics = Table {
            columns: [
                "samples",
                "mae",
                "mae_percent",
                "medae",
                "medae_percent",
                "iou_percent",
                "agreement",
                "difference",
                "r2",
                "ols_slope",
                "ols_intercept",
                "theil_sen_slope",
                "theil_sen_intercept",
            ]
            .map(String::from)
            .to_vec(),
            rows: vec![vec![
                m,
                mae,
                mae / area * 100.0,
                medae,
                medae / area * 100.0,
                iou,
                agreement,
                difference,
                r * r,
                slope,
                my - slope * mx,
                ts,
                ti,
            ]],
            ..Default::default()
        };
        Ok(outputs([
            ("table", Value::Table(t)),
            ("metrics", Value::Table(metrics)),
            ("residuals", Value::Map(Map::new(p.grid.clone(), residuals))),
        ]))
    }
}
