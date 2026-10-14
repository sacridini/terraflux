//! diffeR's comparisons at coarser resolutions (landschaft's simulate/_difference.py, diffeR 0.0-8): the shares
//! of the classes in blocks of cells (memberships), the composite cross-tabulation of two maps at a coarser
//! resolution (Pontius & Cheuk 2006), the quantity, exchange and shift differences at resolutions growing by a
//! factor (differenceMR), the mean absolute deviation of two continuous maps by component and resolution (MAD), and
//! a sample's cross-tabulation scaled to the population (sample2pop). The sums are R's (80-bit `sum`) and terra's
//! (`global`, `aggregate`: double sums in the order of the cells), so the numbers are diffeR's.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::change_analysis::{dropped, overall};
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Memberships));
    r.add(Arc::new(Composite));
    r.add(Arc::new(Resolutions));
    r.add(Arc::new(Mad));
    r.add(Arc::new(SampleToPopulation));
}

/// terra's global(sum, na.rm = TRUE): a double sum in cell order, NaN skipped (NaN without any value).
fn terra_sum(v: &[f64]) -> f64 {
    let mut s = 0.0;
    let mut any = false;
    for &x in v {
        if !x.is_nan() {
            s += x;
            any = true;
        }
    }
    if any { s } else { f64::NAN }
}

/// The cells of the blocks of `f` x `f` cells (row-major blocks, the cells of a block row by row), NaN beyond the
/// map: (block rows, block columns, the blocks' cells).
fn blocks(v: &[f64], ny: usize, nx: usize, f: usize) -> (usize, usize, Vec<f64>) {
    let (by, bx) = (ny.div_ceil(f), nx.div_ceil(f));
    let mut out = vec![f64::NAN; by * bx * f * f];
    for y in 0..ny {
        for x in 0..nx {
            let (b, i, j) = ((y / f) * bx + x / f, y % f, x % f);
            out[b * f * f + i * f + j] = v[y * nx + x];
        }
    }
    (by, bx, out)
}

/// terra's aggregate(fun = "mean") without na.rm: NaN where a block has a missing (or padded) cell.
fn agg_mean(v: &[f64], ny: usize, nx: usize, f: usize) -> Vec<f64> {
    let (_, _, b) = blocks(v, ny, nx, f);
    let ff = f * f;
    b.chunks(ff).map(|c| c.iter().fold(0.0, |s, &x| s + x) / ff as f64).collect()
}

/// terra's aggregate(fun = "sum"): a double sum in cell order (NaN skipped with `na_rm`, NaN without any value).
fn agg_sum(v: &[f64], ny: usize, nx: usize, f: usize, na_rm: bool) -> Vec<f64> {
    let (_, _, b) = blocks(v, ny, nx, f);
    b.chunks(f * f).map(|c| if na_rm { terra_sum(c) } else { c.iter().fold(0.0, |s, &x| s + x) }).collect()
}

/// The classes of maps: their values, sorted.
fn classes_of(maps: &[&[f64]]) -> Vec<f64> {
    let mut c: Vec<f64> = maps.iter().flat_map(|m| ffi::map_classes(m)).collect();
    c.sort_by(f64::total_cmp);
    c.dedup();
    c
}

fn indicator(v: &[f64], c: f64) -> Vec<f64> {
    v.iter()
        .map(|&x| {
            if x.is_nan() {
                f64::NAN
            } else if x == c {
                1.0
            } else {
                0.0
            }
        })
        .collect()
}

/// diffeR's composite cross-tabulation of `classes` at blocks of `f` x `f` cells, proportions (k x k).
fn composite(comp: &[f64], refm: &[f64], ny: usize, nx: usize, classes: &[f64], f: usize) -> Vec<f64> {
    let comp_ag: Vec<Vec<f64>> = classes.iter().map(|&c| agg_mean(&indicator(comp, c), ny, nx, f)).collect();
    let ref_ag: Vec<Vec<f64>> = classes.iter().map(|&c| agg_mean(&indicator(refm, c), ny, nx, f)).collect();
    let ones: Vec<f64> = comp.iter().map(|x| if x.is_nan() { f64::NAN } else { 1.0 }).collect();
    let weights = agg_sum(&ones, ny, nx, f, false);
    // np.minimum: NaN when either is
    let mins: Vec<Vec<f64>> = comp_ag
        .iter()
        .zip(&ref_ag)
        .map(|(a, b)| {
            a.iter().zip(b).map(|(&x, &y)| if x.is_nan() || y.is_nan() { f64::NAN } else { x.min(y) }).collect()
        })
        .collect();
    let mut sum_diag = vec![0.0; weights.len()];
    for m in &mins {
        for (s, v) in sum_diag.iter_mut().zip(m) {
            *s += v;
        }
    }
    let total = terra_sum(&weights);
    let k = classes.len();
    let mut out = vec![0.0; k * k];
    for i in 0..k {
        for j in 0..k {
            let a: Vec<f64> = (0..weights.len())
                .map(|b| {
                    weights[b] * ((comp_ag[i][b] - mins[i][b]) * (ref_ag[j][b] - mins[j][b])) / (1.0 - sum_diag[b])
                })
                .collect();
            out[i * k + j] = terra_sum(&a) / total;
        }
    }
    for i in 0..k {
        let a: Vec<f64> = mins[i].iter().zip(&weights).map(|(m, w)| m * w).collect();
        out[i * k + i] = terra_sum(&a) / total;
    }
    out
}

/// R's log(x, base).
fn log_base(x: f64, base: f64) -> f64 {
    if base == 2.0 {
        x.log2()
    } else if base == 10.0 {
        x.log10()
    } else {
        x.ln() / base.ln()
    }
}

/// 1, fact, fact^2... up to the larger side (differenceMR caps the last at that side, MAD does not).
fn factors(nrow: usize, ncol: usize, fact: usize, cap: bool) -> Vec<usize> {
    let mdim = nrow.max(ncol);
    let lg = log_base(mdim as f64, fact as f64);
    let mut maxp = lg.floor() as u32;
    if lg - lg.round_ties_even() != 0.0 {
        maxp += 1;
    }
    let mut out: Vec<usize> = std::iter::once(1).chain((1..=maxp).map(|p| fact.pow(p))).collect();
    if cap && *out.last().unwrap() > mdim {
        *out.last_mut().unwrap() = mdim;
    }
    out
}

/// A matrix as a table: a row per class (its class, then a column per class).
fn matrix_table(first: &str, rows: &[f64], cols: &[f64], m: &[f64]) -> Table {
    let mut t = Table::new(
        std::iter::once(first.to_string()).chain(cols.iter().map(|c| crate::value::class_label(*c))).collect(),
    );
    let k = cols.len();
    t.rows = rows
        .iter()
        .enumerate()
        .map(|(i, &c)| std::iter::once(c).chain(m[i * k..(i + 1) * k].iter().copied()).collect())
        .collect();
    t
}

fn same_grid<'a>(args: &'a Args, a: &str, b: &str) -> Result<(&'a Map, &'a Map)> {
    let (x, y) = (args.map(a)?, args.map(b)?);
    if x.grid.width != y.grid.width || x.grid.height != y.grid.height {
        return Err(Error::msg(format!("node {:?}: the maps differ in size", args.node)));
    }
    Ok((x, y))
}

fn factor_of(args: &Args, default: usize) -> Result<usize> {
    let f: usize = args.param("factor", default)?;
    if f < 1 {
        return Err(Error::msg(format!("node {:?}: the factor is 1 or more", args.node)));
    }
    Ok(f)
}

/// The grid of blocks of `f` x `f` cells (terra's aggregate: the extent grown to whole blocks).
fn coarse_grid(g: &Grid, f: usize) -> Grid {
    let mut c = g.clone();
    c.width = g.width.div_ceil(f);
    c.height = g.height.div_ceil(f);
    let t = g.transform;
    c.transform = [t[0], t[1] * f as f64, t[2] * f as f64, t[3], t[4] * f as f64, t[5] * f as f64];
    c
}

struct Memberships;

impl Operator for Memberships {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "change.memberships",
            "The share of each class in each block of factor x factor cells (diffeR's memberships through \
             landschaft: terra's segregate and aggregate by the mean); blocks with a missing cell, and those cut \
             by the right and bottom edges, have none.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "a map of classes"))
        .output(PortSpec::new("stack", Type::Stack, "a layer per class, on the grid of the blocks"))
        .param(PortSpec::optional("factor", Type::Integer, "cells per side of a block (default 2)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let f = factor_of(args, 2)?;
        let (ny, nx) = (m.grid.height, m.grid.width);
        let classes = classes_of(&[&m.data]);
        let layers = classes.iter().map(|&c| Arc::new(agg_mean(&indicator(&m.data, c), ny, nx, f))).collect();
        Ok(outputs([(
            "stack",
            Value::Stack(Stack {
                grid: Arc::new(coarse_grid(&m.grid, f)),
                keys: classes.iter().map(|&c| Key::Class(c)).collect(),
                layers,
            }),
        )]))
    }
}

struct Composite;

impl Operator for Composite {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "change.composite",
            "The cross-tabulation of two maps at a coarser resolution by the composite method (diffeR's composite \
             through landschaft, Pontius & Cheuk 2006): each block of factor x factor cells a mix of classes, the \
             agreement of a class the smaller of its two shares, the rest shared among the pairs of classes; \
             blocks with a missing cell or cut by the edge left out. Proportions of the domain.",
            Access::Global,
        )
        .input(PortSpec::new("comp", Type::Map, "the comparison map (rows)"))
        .input(PortSpec::new("ref", Type::Map, "the reference map (columns)"))
        .output(PortSpec::new("table", Type::Table, "a row per class of either map: its class, then the shares"))
        .param(PortSpec::new("factor", Type::Integer, "cells per side of a block"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (a, b) = same_grid(args, "comp", "ref")?;
        let f = factor_of(args, 2)?;
        let classes = classes_of(&[&a.data, &b.data]);
        let m = composite(&a.data, &b.data, a.grid.height, a.grid.width, &classes, f);
        Ok(outputs([("table", Value::Table(matrix_table("comp", &classes, &classes, &m)))]))
    }
}

/// The population of each category from a table of two columns (category, size).
fn population_of(t: &Table) -> Result<Vec<(f64, f64)>> {
    if t.columns.len() != 2 {
        return Err(Error::msg("population must have two columns (category, size)"));
    }
    Ok(t.rows.iter().map(|r| (r[0], r[1])).collect())
}

/// diffeR's sample2pop: each row of the matrix (k x k, rows `classes`) scaled to its category's population.
fn sample_to_population(m: &mut [f64], classes: &[f64], pop: &[(f64, f64)]) -> Result<()> {
    let k = classes.len();
    if pop.len() != k {
        return Err(Error::msg("population and the matrix must have the same number of rows"));
    }
    for &(cat, size) in pop {
        let j = classes
            .iter()
            .position(|&c| c == cat)
            .ok_or_else(|| Error::msg(format!("category {cat} of the population is not in the matrix")))?;
        let s = ffi::r_sum(&m[j * k..(j + 1) * k]);
        for v in &mut m[j * k..(j + 1) * k] {
            *v = size * *v / s;
        }
    }
    Ok(())
}

struct Resolutions;

impl Operator for Resolutions {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "change.resolutions",
            "The quantity, exchange and shift differences between two maps at coarser and coarser resolutions \
             (diffeR's differenceMR through landschaft): the cross-tabulation at the maps' resolution, then the \
             composite one at blocks of factor, factor^2... cells per side up to the larger side; shift shrinks \
             as the resolution coarsens, quantity stays.",
            Access::Global,
        )
        .input(PortSpec::new("comp", Type::Map, "the comparison map"))
        .input(PortSpec::new("ref", Type::Map, "the reference map"))
        .input(PortSpec::optional("population", Type::Table, "category, size: to scale the first matrix"))
        .output(PortSpec::new("table", Type::Table, "Multiples, Resolution, Quantity, Exchange, Shift, Overall"))
        .param(PortSpec::optional("factor", Type::Integer, "the growth of a block's side (default 2)"))
        .param(PortSpec::optional("percent", Type::Bool, "in % of the domain (default) or proportions"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let (a, b) = same_grid(args, "comp", "ref")?;
        let f = factor_of(args, 2)?;
        let (ny, nx) = (a.grid.height, a.grid.width);
        let all = classes_of(&[&a.data, &b.data]);
        let counts = ffi::crosstab(&a.data, &b.data, &all, &all)?;
        let (kept, mut m) = dropped(&counts, &all);
        if let Some(Value::Table(p)) = args.get("population") {
            sample_to_population(&mut m, &kept, &population_of(p).map_err(fail)?).map_err(fail)?;
        }
        let k = kept.len();
        let by_column: Vec<f64> = (0..k).flat_map(|j| (0..k).map(move |i| (i, j))).map(|(i, j)| m[i * k + j]).collect();
        let total = ffi::r_sum(&by_column);
        let m: Vec<f64> = m.iter().map(|v| v / total * 100.0).collect();
        let row = |mat: &[f64], n: usize| {
            let o = overall(mat, n);
            [o[0], o[1], o[2], o[4]]
        };
        let facts = factors(ny, nx, f, true);
        let res = a.grid.transform[1].abs();
        let mut rows = vec![row(&m, k)];
        let classes = classes_of(&[&a.data]);
        for &fct in &facts[1..] {
            let c: Vec<f64> = composite(&a.data, &b.data, ny, nx, &classes, fct).iter().map(|v| v * 100.0).collect();
            rows.push(row(&c, classes.len()));
        }
        let percent: bool = args.param("percent", true)?;
        let mut t = Table::new(
            ["Multiples", "Resolution", "Quantity", "Exchange", "Shift", "Overall"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        t.rows = facts
            .iter()
            .zip(rows)
            .map(|(&fct, r)| {
                let r = if percent { r } else { r.map(|v| v / 100.0) };
                [fct as f64, fct as f64 * res].into_iter().chain(r).collect()
            })
            .collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}

struct Mad;

impl Operator for Mad {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "change.mad",
            "The mean absolute deviation between two continuous maps and its components (diffeR's MAD through \
             landschaft): the deviation split into quantity (its mean), strata (deviations that cancel within \
             each stratum) and element (the rest), at the maps' resolution and at blocks of 2, 4, 8... cells (sums \
             of the deviations).",
            Access::Global,
        )
        .input(PortSpec::new("grid1", Type::Map, "the first map"))
        .input(PortSpec::new("grid2", Type::Map, "the second map (subtracted)"))
        .input(PortSpec::optional("strata", Type::Map, "strata (0 and missing outside every one; default one)"))
        .output(PortSpec::new("table", Type::Table, "Multiples, Resolution, Quantity, Strata, Element, Total"))
        .param(PortSpec::optional("multiple", Type::Bool, "every resolution (default) or the maps' only"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (g1, g2) = same_grid(args, "grid1", "grid2")?;
        let (ny, nx) = (g1.grid.height, g1.grid.width);
        let s: Vec<f64> = match args.opt_map("strata")? {
            Some(st) => {
                if st.data.len() != g1.data.len() {
                    return Err(Error::msg(format!("node {:?}: the strata differ in size", args.node)));
                }
                st.data.iter().map(|&v| if v == 0.0 { f64::NAN } else { v }).collect()
            }
            None => g1.data.iter().map(|v| if v.is_nan() { f64::NAN } else { 1.0 }).collect(),
        };
        let dev: Vec<f64> = g1.data.iter().zip(g2.data.iter()).map(|(a, b)| a - b).collect();
        let mut values: Vec<f64> = s.iter().copied().filter(|v| !v.is_nan()).collect();
        values.sort_by(f64::total_cmp);
        values.dedup();
        let nozeros = terra_sum(&s.iter().map(|v| if v.is_nan() { f64::NAN } else { 1.0 }).collect::<Vec<_>>());
        let made: Vec<Vec<f64>> = values
            .iter()
            .map(|&c| {
                dev.iter()
                    .zip(&s)
                    .map(|(d, &v)| {
                        d * if v.is_nan() {
                            f64::NAN
                        } else if v == c {
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .collect()
            })
            .collect();
        let components = |maps: &[Vec<f64>]| {
            let sums: Vec<f64> = maps.iter().map(|x| terra_sum(x)).collect();
            let q = ffi::r_sum(&sums.iter().map(|v| v / nozeros).collect::<Vec<_>>()).abs();
            let st = ffi::r_sum(&sums.iter().map(|v| v.abs()).collect::<Vec<_>>()) / nozeros - q;
            let abs: Vec<f64> =
                maps.iter().map(|x| terra_sum(&x.iter().map(|v| v.abs()).collect::<Vec<_>>())).collect();
            let tot = ffi::r_sum(&abs) / nozeros;
            (q, st, tot)
        };
        let facts = factors(ny, nx, 2, false);
        let res = g1.grid.transform[1].abs();
        let multiple: bool = args.param("multiple", true)?;
        let mut rows = Vec::new();
        let (q, st, tot) = components(&made);
        rows.push((facts[0], [facts[0] as f64 * res, q, st, tot - q - st, tot]));
        if multiple {
            for &f in &facts[1..] {
                let agg: Vec<Vec<f64>> = made.iter().map(|x| agg_sum(x, ny, nx, f, true)).collect();
                let (q, st, tot) = components(&agg);
                rows.push((f, [f as f64 * res, q, st, tot - q - st, tot]));
            }
        }
        let names = ["Resolution", "Quantity", "Strata", "Element", "Total"];
        let mut t = Table::new(
            if multiple { std::iter::once("Multiples").chain(names).collect::<Vec<_>>() } else { names.to_vec() }
                .iter()
                .map(|s| s.to_string())
                .collect(),
        );
        t.rows = rows
            .into_iter()
            .map(|(f, r)| if multiple { std::iter::once(f as f64).chain(r).collect() } else { r.to_vec() })
            .collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}

struct SampleToPopulation;

impl Operator for SampleToPopulation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "change.sample_to_population",
            "A sample's cross-tabulation scaled to the population of each row's class (diffeR's sample2pop through \
             landschaft, by the row's class rather than its position): each row times its population over its \
             sum.",
            Access::Global,
        )
        .input(PortSpec::new("matrix", Type::Table, "a cross-tabulation (change.crosstab): class, then the counts"))
        .input(PortSpec::new("population", Type::Table, "category, size"))
        .output(PortSpec::new("table", Type::Table, "the matrix scaled"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let (Some(Value::Table(m)), Some(Value::Table(p))) = (args.get("matrix"), args.get("population")) else {
            return Err(fail(Error::msg("connect the matrix and the population")));
        };
        let k = m.rows.len();
        if m.columns.len() != k + 1 {
            return Err(fail(Error::msg("the matrix must be square (a class column, then a column per class)")));
        }
        let classes: Vec<f64> = m.rows.iter().map(|r| r[0]).collect();
        let mut v: Vec<f64> = m.rows.iter().flat_map(|r| r[1..].iter().copied()).collect();
        sample_to_population(&mut v, &classes, &population_of(p).map_err(fail)?).map_err(fail)?;
        let mut t = Table::new(m.columns.clone());
        t.rows = (0..k)
            .map(|i| std::iter::once(classes[i]).chain(v[i * k..(i + 1) * k].iter().copied()).collect())
            .collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}
