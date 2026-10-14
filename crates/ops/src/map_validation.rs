//! The comparisons of simulated land change with the observed (landschaft's simulate/_validation.py and
//! _validate.py): the three-map comparison of Pontius et al. (2011) at several resolutions with its budget of
//! agreement and figure of merit (lulcc's ThreeMapComparison, AgreementBudget and FigureOfMerit, its table by
//! landschaft's C++), MOLUSCE's kappas and dependence coefficients and its error budget (Pontius 2002), and FUTURES'
//! validation (r.futures.validation: disagreement, kappa, kappa simulation, change metrics), whole or by tiles or
//! regions (r.futures.gridvalidation). R's sums where lulcc sums, numpy's where MOLUSCE and FUTURES do.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::change::numpy_sum;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(ThreeMap));
    r.add(Arc::new(Kappa));
    r.add(Arc::new(ErrorBudget));
    r.add(Arc::new(Futures));
    r.add(Arc::new(FuturesGrid));
}

fn cols(names: &[&str]) -> Vec<String> {
    names.iter().map(|s| s.to_string()).collect()
}

/// R's sum(v, na.rm = TRUE).
fn sum_na(v: &[f64]) -> f64 {
    ffi::r_sum(&v.iter().copied().filter(|x| !x.is_nan()).collect::<Vec<_>>())
}

fn maps<'a>(args: &'a Args, ports: &[&str]) -> Result<Vec<&'a Map>> {
    let m: Vec<&Map> = ports.iter().map(|p| args.map(p)).collect::<Result<_>>()?;
    if m.iter().any(|x| x.grid.width != m[0].grid.width || x.grid.height != m[0].grid.height) {
        return Err(Error::msg(format!("node {:?}: the maps differ in shape", args.node)));
    }
    Ok(m)
}

// ---------------------------------------------------------------------------------- three maps (lulcc)

/// raster::aggregate(fun = sum, na.rm = TRUE, expand = TRUE): blocks from the top-left, NaN when all are.
fn aggregate_sum(v: &[f64], ny: usize, nx: usize, f: usize) -> Vec<f64> {
    let (fy, fx) = (f.min(ny), f.min(nx));
    let (by, bx) = (ny.div_ceil(fy), nx.div_ceil(fx));
    let (mut out, mut any) = (vec![0.0; by * bx], vec![false; by * bx]);
    for y in 0..ny {
        for x in 0..nx {
            let v = v[y * nx + x];
            if !v.is_nan() {
                let b = (y / fy) * bx + x / fx;
                out[b] += v;
                any[b] = true;
            }
        }
    }
    out.iter().zip(any).map(|(&s, a)| if a { s } else { f64::NAN }).collect()
}

/// lulcc's table of three maps at blocks of `f` cells with its totals.
fn tmc_table(x: &[f64], x1: &[f64], y1: &[f64], ny: usize, nx: usize, classes: &[f64], f: usize) -> Result<Vec<f64>> {
    let ones: Vec<f64> = x.iter().map(|v| if v.is_nan() { f64::NAN } else { 1.0 }).collect();
    let block = |m: &[f64]| if f > 1 { aggregate_sum(m, ny, nx, f) } else { m.to_vec() };
    let wt: Vec<f64> = block(&ones).into_iter().filter(|v| !v.is_nan()).collect();
    let counts = |m: &[f64]| -> Vec<f64> {
        classes
            .iter()
            .flat_map(|&c| {
                let ind: Vec<f64> = m
                    .iter()
                    .map(|&v| {
                        if v.is_nan() {
                            f64::NAN
                        } else if v == c {
                            1.0
                        } else {
                            0.0
                        }
                    })
                    .collect();
                block(&ind).into_iter().filter(|v| !v.is_nan())
            })
            .collect()
    };
    let n = classes.len();
    let mut tab = ffi::sim_tmc_table(n as i32, &counts(x), &counts(x1), &counts(y1), &wt)?;
    let nn = n + 1;
    let at = |r: usize, k: usize| r * nn + k;
    for j in 0..n {
        for k in 0..n {
            let v: Vec<f64> = (0..n).map(|m| tab[at(j + nn * m, k)]).collect();
            tab[at(j + nn * n, k)] = sum_na(&v);
        }
    }
    for j in 0..nn {
        for k in 0..n {
            let v: Vec<f64> = (0..n).map(|i| tab[at(i + j * nn, k)]).collect();
            tab[at((j + 1) * nn - 1, k)] = sum_na(&v);
        }
    }
    for j in 0..nn {
        for i in 0..nn {
            let v: Vec<f64> = (0..n).map(|k| tab[at(i + j * nn, k)]).collect();
            tab[at(i + j * nn, nn - 1)] = sum_na(&v);
        }
    }
    Ok(tab)
}

/// lulcc's AgreementBudget of one level: misses, hits, wrong hits, false alarms (and correct rejections).
fn budget(tab: &[f64], n: usize, transition: bool, from: &[usize], to: &[usize]) -> Vec<f64> {
    let nn = n + 1;
    let t = |i: usize, j: usize, k: usize| tab[((i - 1) + (j - 1) * nn) * nn + (k - 1)];
    let mut a = vec![0.0; n];
    for &j in to {
        let mut sub = vec![0.0; n];
        for &i in from {
            if i != j {
                sub[i - 1] = t(i, j, i);
            }
        }
        a[j - 1] = sum_na(&sub);
    }
    let a = ffi::r_sum(&a);
    let mut b = vec![0.0; n];
    for &j in to {
        let mut sub = vec![0.0; n];
        for &i in from {
            if i != j {
                sub[i - 1] = t(i, j, j);
            }
        }
        b[j - 1] = ffi::r_sum(&sub);
    }
    let b = ffi::r_sum(&b);
    let mut c = vec![0.0; n];
    for &j in to {
        let mut sub = vec![0.0; n];
        for &i in from {
            let mut subsub = vec![0.0; n];
            for k in 1..=n {
                if i != j && i != k && j != k {
                    subsub[k - 1] = t(i, j, k);
                }
            }
            sub[i - 1] = sum_na(&subsub);
        }
        c[j - 1] = sum_na(&sub);
    }
    let mut c = ffi::r_sum(&c);
    if to.len() == 1 {
        let (f, tt) = (from[0], to[0]);
        let mut sub = vec![0.0; n];
        for j in 1..=n {
            if j != f && j != tt {
                sub[j - 1] = t(f, j, tt);
            }
        }
        c += ffi::r_sum(&sub);
    }
    let mut d = vec![0.0; n];
    for &j in from {
        let mut sub = vec![0.0; n];
        for &k in to {
            if k != j {
                sub[k - 1] = t(j, j, k);
            }
        }
        d[j - 1] = ffi::r_sum(&sub);
    }
    let d = ffi::r_sum(&d);
    if transition {
        return vec![a, b, c, d];
    }
    let mut e = vec![0.0; n];
    for &i in from {
        e[i - 1] = t(i, i, i);
    }
    vec![a, b, c, d, ffi::r_sum(&e)]
}

/// lulcc's FigureOfMerit: overall, per class, per transition (n x n, from x to).
fn figure_of_merit(tab: &[f64], n: usize) -> (f64, Vec<f64>, Vec<f64>) {
    let nn = n + 1;
    let t = |r: usize, k: usize| tab[(r - 1) * nn + (k - 1)];
    let (mut a, mut b) = (0.0, 0.0);
    for j in 1..=n {
        a = sum_na(&[a, t(j * nn, j)]);
        b = sum_na(&[b, t(j + (j - 1) * nn, j)]);
    }
    let overall = (a - b) / (1.0 - b);
    let category = (1..=n)
        .map(|i| {
            let (mut a, mut b) = (0.0, 0.0);
            for j in 1..=n {
                a = sum_na(&[a, t(i + (j - 1) * nn, j)]);
                b = sum_na(&[b, t(i + (j - 1) * nn, nn)]);
            }
            let c = t(i + (i - 1) * nn, i);
            (a - c) / (b - c)
        })
        .collect();
    let mut transition = vec![0.0; n * n];
    for j in 1..=n {
        for i in 1..=n {
            let a = t(i + (j - 1) * nn, j);
            let b = t(i + (j - 1) * nn, nn);
            let mut c = 0.0;
            for k in 1..=n {
                c = sum_na(&[c, t(i + (k - 1) * nn, j)]);
            }
            transition[(i - 1) * n + (j - 1)] = a / (b + c - a);
        }
    }
    (overall, category, transition)
}

struct ThreeMap;

impl Operator for ThreeMap {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "validate.three_map",
            "The three-map comparison of a simulation (Pontius, Peethambaram & Castella 2011; lulcc's \
             ThreeMapComparison through landschaft): the reference at the start and at the end and the simulated \
             end compared at blocks of several sizes, the budget of agreement and disagreement (misses, hits, wrong \
             hits, false alarms, correct rejections; overall, per class, per transition) and the figure of merit. \
             The cells without a value in any map are left out of all three.",
            Access::Global,
        )
        .input(PortSpec::new("t0", Type::Map, "the reference map at the start"))
        .input(PortSpec::new("t1", Type::Map, "the reference map at the end"))
        .input(PortSpec::new("simulated", Type::Map, "the simulated map of the end"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "per factor lulcc's table: t0 class within t1 class (empty: total), the simulated classes, total",
        ))
        .output(PortSpec::new(
            "budget",
            Type::Table,
            "level (0 overall, 1 class, 2 transition), from, to, factor, misses, hits, wrong_hits, false_alarms, \
             correct_rejections",
        ))
        .output(PortSpec::new("figure_of_merit", Type::Table, "per factor: overall, then per class"))
        .output(PortSpec::new("transitions", Type::Table, "per factor and class from: the figure of merit to each"))
        .param(PortSpec::optional("factors", Type::Any, "the blocks' sides in cells (default [1])"))
        .param(PortSpec::optional("categories", Type::Any, "the classes (default those of the maps)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let m = maps(args, &["t0", "t1", "simulated"])?;
        let (ny, nx) = (m[0].grid.height, m[0].grid.width);
        let miss: Vec<bool> = (0..ny * nx).map(|i| m.iter().any(|x| x.data[i].is_nan())).collect();
        let masked: Vec<Vec<f64>> =
            m.iter().map(|x| x.data.iter().zip(&miss).map(|(&v, &k)| if k { f64::NAN } else { v }).collect()).collect();
        let classes: Vec<f64> = match args.opt_param::<Vec<f64>>("categories")? {
            Some(c) => c,
            None => {
                let mut c: Vec<f64> = masked.iter().flatten().copied().filter(|v| !v.is_nan()).collect();
                c.sort_by(f64::total_cmp);
                c.dedup();
                c
            }
        };
        let n = classes.len();
        if n < 2 {
            return Err(fail("the comparison needs at least two classes".into()));
        }
        let factors: Vec<usize> = match args.params.get("factors") {
            None | Some(serde_json::Value::Null) => vec![1],
            Some(serde_json::Value::Number(f)) => vec![f.as_f64().unwrap_or(0.0) as usize],
            Some(_) => args.param("factors", vec![1])?,
        };
        if factors.iter().any(|&f| f < 1) {
            return Err(fail("the factors must be at least 1".into()));
        }
        let nn = n + 1;
        let label = |k: usize| if k < n { classes[k] } else { f64::NAN };
        let mut table = Table::new(
            cols(&["factor", "reference_t0", "reference_t1"])
                .into_iter()
                .chain(classes.iter().map(|c| crate::value::class_label(*c)))
                .chain(["total".to_string()])
                .collect(),
        );
        let mut budget_t = Table::new(cols(&[
            "level",
            "from",
            "to",
            "factor",
            "misses",
            "hits",
            "wrong_hits",
            "false_alarms",
            "correct_rejections",
        ]));
        let mut fom = Table::new(
            cols(&["factor", "overall"])
                .into_iter()
                .chain(classes.iter().map(|c| crate::value::class_label(*c)))
                .collect(),
        );
        let mut trans = Table::new(
            cols(&["factor", "from"])
                .into_iter()
                .chain(classes.iter().map(|c| crate::value::class_label(*c)))
                .collect(),
        );
        let every: Vec<usize> = (1..=n).collect();
        for &f in &factors {
            let tab = tmc_table(&masked[0], &masked[1], &masked[2], ny, nx, &classes, f)?;
            for j in 0..nn {
                for i in 0..nn {
                    let r = i + j * nn;
                    table.rows.push(
                        [f as f64, label(i), label(j)]
                            .into_iter()
                            .chain(tab[r * nn..(r + 1) * nn].iter().copied())
                            .collect(),
                    );
                }
            }
            let mut push = |level: f64, from: f64, to: f64, v: Vec<f64>| {
                let mut row = vec![level, from, to, f as f64];
                row.extend(&v);
                row.resize(9, f64::NAN);
                budget_t.rows.push(row);
            };
            push(0.0, f64::NAN, f64::NAN, budget(&tab, n, false, &every, &every));
            for i in 1..=n {
                push(1.0, classes[i - 1], f64::NAN, budget(&tab, n, false, &[i], &every));
            }
            for i in 1..=n {
                for j in 1..=n {
                    push(2.0, classes[i - 1], classes[j - 1], budget(&tab, n, true, &[i], &[j]));
                }
            }
            let (o, c, t) = figure_of_merit(&tab, n);
            fom.rows.push([f as f64, o].into_iter().chain(c).collect());
            for i in 0..n {
                trans
                    .rows
                    .push([f as f64, classes[i]].into_iter().chain(t[i * n..(i + 1) * n].iter().copied()).collect());
            }
        }
        Ok(outputs([
            ("table", Value::Table(table)),
            ("budget", Value::Table(budget_t)),
            ("figure_of_merit", Value::Table(fom)),
            ("transitions", Value::Table(trans)),
        ]))
    }
}

// ------------------------------------------------------------------------------------------- MOLUSCE

/// The cross table of two class vectors over the sorted classes of each (or of both): row-major, rows x cols.
fn cross(x: &[f64], y: &[f64], expand: bool) -> (Vec<f64>, usize, usize) {
    let uniq = |v: &[f64]| {
        let mut u = v.to_vec();
        u.sort_by(f64::total_cmp);
        u.dedup();
        u
    };
    let (mut gx, mut gy) = (uniq(x), uniq(y));
    if expand {
        gx = uniq(&[gx.clone(), gy].concat());
        gy = gx.clone();
    }
    let (r, c) = (gx.len(), gy.len());
    let mut t = vec![0.0; r * c];
    for (a, b) in x.iter().zip(y) {
        let i = gx.binary_search_by(|v| v.total_cmp(a)).unwrap();
        let j = gy.binary_search_by(|v| v.total_cmp(b)).unwrap();
        t[i * c + j] += 1.0;
    }
    (t, r, c)
}

/// numpy's mean (pairwise sum over the count).
fn np_mean(v: &[f64]) -> f64 {
    numpy_sum(v) / v.len() as f64
}

/// MOLUSCE's DependenceCoef of two maps: kappa, kappa location, kappa histogram, correctness, Cramer's V, joint
/// information uncertainty, correlation.
fn compare_maps(a: &[f64], b: &[f64]) -> std::result::Result<[f64; 7], String> {
    let (x, y): (Vec<f64>, Vec<f64>) =
        a.iter().zip(b).filter(|(p, q)| !p.is_nan() && !q.is_nan()).map(|(&p, &q)| (p, q)).unzip();
    if x.is_empty() {
        return Err("no cell has a value in both maps".into());
    }
    let n = x.len() as f64;
    // np.corrcoef (its dot products by BLAS: the last bits may differ)
    let (mx, my) = (np_mean(&x), np_mean(&y));
    let (dx, dy): (Vec<f64>, Vec<f64>) = x.iter().zip(&y).map(|(p, q)| (p - mx, q - my)).unzip();
    let sxy = numpy_sum(&dx.iter().zip(&dy).map(|(p, q)| p * q).collect::<Vec<_>>());
    let sxx = numpy_sum(&dx.iter().map(|p| p * p).collect::<Vec<_>>());
    let syy = numpy_sum(&dy.iter().map(|q| q * q).collect::<Vec<_>>());
    let correlation = (sxy / (sxx.sqrt() * syy.sqrt())).clamp(-1.0, 1.0);
    let (t, r, c) = cross(&x, &y, true);
    let p: Vec<f64> = t.iter().map(|v| v / n).collect();
    let mut pa = 0.0;
    for i in 0..r {
        pa += p[i * c + i];
    }
    let rows: Vec<f64> = (0..r).map(|i| t[i * c..(i + 1) * c].iter().sum::<f64>() / n).collect();
    let colsum: Vec<f64> = (0..c).map(|j| (0..r).map(|i| t[i * c + j]).sum::<f64>() / n).collect();
    let mut pexpect = 0.0;
    let mut pmax = 0.0;
    for k in 0..r {
        pexpect += rows[k] * colsum[k];
        pmax += rows[k].min(colsum[k]);
    }
    let kappa = (pa - pexpect) / (1.0 - pexpect);
    let kappa_location = (pa - pexpect) / (pmax - pexpect);
    let kappa_histogram = (pmax - pexpect) / (1.0 - pexpect);
    let mut s = 0.0;
    for i in 0..r {
        s += t[i * c + i];
    }
    let correctness = 100.0 * s / n;
    let (t, r, c) = cross(&x, &y, false);
    let srows: Vec<f64> = (0..r).map(|i| t[i * c..(i + 1) * c].iter().sum()).collect();
    let scols: Vec<f64> = (0..c).map(|j| (0..r).map(|i| t[i * c + j]).sum()).collect();
    let terms: Vec<f64> = (0..r * c)
        .map(|k| {
            let e = 1.0 * srows[k / c] * scols[k % c] / n;
            if e == 0.0 { 0.0 } else { (t[k] - e).powi(2) / e }
        })
        .collect();
    let x2 = numpy_sum(&terms);
    let cramer = if r.min(c) > 1 { (x2 / (n * (c - 1).min(r - 1) as f64)).sqrt() } else { f64::NAN };
    let pr: Vec<f64> = srows.iter().map(|v| v / n).filter(|v| *v != 0.0).collect();
    let pc: Vec<f64> = scols.iter().map(|v| v / n).filter(|v| *v != 0.0).collect();
    let entropy = |v: &[f64]| -numpy_sum(&v.iter().map(|p| p * p.ln()).collect::<Vec<_>>());
    let (hx, hy) = (entropy(&pr), entropy(&pc));
    let pt: Vec<f64> = t.iter().map(|v| v / n).filter(|v| *v != 0.0).collect();
    let hxy = entropy(&pt);
    let jiu = if hx + hy != 0.0 { 2.0 * ((hx + hy - hxy) / (hx + hy)) } else { f64::NAN };
    Ok([kappa, kappa_location, kappa_histogram, correctness, cramer, jiu, correlation])
}

struct Kappa;

impl Operator for Kappa {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "validate.kappa",
            "MOLUSCE's comparison of two maps of classes (DependenceCoef through landschaft): kappa, kappa location \
             and kappa histogram and the % of cells of the same class over the classes of both maps; Cramer's V \
             and the joint information uncertainty; the correlation of the class values.",
            Access::Global,
        )
        .input(PortSpec::new("reference", Type::Map, "the reference map"))
        .input(PortSpec::new("simulated", Type::Map, "the simulated map"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "kappa, kappa_location, kappa_histogram, correctness, cramer, jiu, correlation",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = maps(args, &["reference", "simulated"])?;
        let v = compare_maps(&m[0].data, &m[1].data).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let mut t = Table::new(cols(&[
            "kappa",
            "kappa_location",
            "kappa_histogram",
            "correctness",
            "cramer",
            "jiu",
            "correlation",
        ]));
        t.rows = vec![v.to_vec()];
        Ok(outputs([("table", Value::Table(t))]))
    }
}

/// numpy's sum of every s x s block from the top-left (the edges cut off): (rows, cols, sums).
fn block_sums(v: &[f64], ny: usize, nx: usize, s: usize) -> (usize, usize, Vec<f64>) {
    let (by, bx) = (ny / s, nx / s);
    let out = (0..by * bx)
        .map(|b| {
            let (r, c) = (b / bx, b % bx);
            let cells: Vec<f64> = (0..s * s).map(|k| v[(r * s + k / s) * nx + c * s + k % s]).collect();
            numpy_sum(&cells)
        })
        .collect();
    (by, bx, out)
}

struct ErrorBudget;

impl Operator for ErrorBudget {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "validate.error_budget",
            "MOLUSCE's error budget (Pontius 2002, through landschaft): the agreement of the maps when the \
             simulation has no, medium or perfect information of quantity and location, at the maps' resolution \
             and coarser ones, merging scale x scale cells each time (the edges left over dropped).",
            Access::Global,
        )
        .input(PortSpec::new("reference", Type::Map, "the reference map"))
        .input(PortSpec::new("simulated", Type::Map, "the simulated map (no classes the reference lacks)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "per resolution: cells, no_no, no_medium, medium_medium, medium_perfect, perfect_perfect",
        ))
        .param(PortSpec::optional("iterations", Type::Integer, "the resolutions (default 3)"))
        .param(PortSpec::optional("scale", Type::Integer, "cells merged per side each time (default 2)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let m = maps(args, &["reference", "simulated"])?;
        let scale: usize = args.param("scale", 2)?;
        if scale < 2 {
            return Err(fail("scale must be at least 2".into()));
        }
        let iterations: usize = args.param("iterations", 3)?;
        let (r, s) = (&m[0].data, &m[1].data);
        let uniq = |v: &[f64]| {
            let mut u: Vec<f64> = v.iter().copied().filter(|x| !x.is_nan()).collect();
            u.sort_by(f64::total_cmp);
            u.dedup();
            u
        };
        let cats = uniq(r);
        if let Some(e) = uniq(s).iter().find(|c| !cats.contains(c)) {
            return Err(fail(format!("the simulation has classes the reference lacks: {e}")));
        }
        let (mut ny, mut nx) = (m[0].grid.height, m[0].grid.width);
        let mask: Vec<bool> = r.iter().zip(s.iter()).map(|(a, b)| a.is_nan() || b.is_nan()).collect();
        let mut w: Vec<f64> = mask.iter().map(|&k| if k { 0.0 } else { 1.0 }).collect();
        let fill = |v: &[f64]| -> Vec<f64> {
            v.iter().zip(&mask).map(|(&x, &k)| if k || x.is_nan() { 0.0 } else { x }).collect()
        };
        let (rf, sf) = (fill(r), fill(s));
        let mut rj: Vec<Vec<f64>> =
            cats.iter().map(|&c| rf.iter().map(|&v| if v == c { 1.0 } else { 0.0 }).collect()).collect();
        let mut sj: Vec<Vec<f64>> =
            cats.iter().map(|&c| sf.iter().map(|&v| if v == c { 1.0 } else { 0.0 }).collect()).collect();
        let mut t =
            Table::new(cols(&["cells", "no_no", "no_medium", "medium_medium", "medium_perfect", "perfect_perfect"]));
        let mut side = 1usize;
        for _ in 0..iterations {
            let sum_w = numpy_sum(&w);
            let weighted = |v: &[f64]| numpy_sum(&w.iter().zip(v).map(|(a, b)| a * b).collect::<Vec<_>>()) / sum_w;
            let size = cats.len() as f64;
            let level = |f: &dyn Fn(usize, usize) -> f64| {
                let mut arr = vec![0.0; w.len()];
                for k in 0..cats.len() {
                    for (i, a) in arr.iter_mut().enumerate() {
                        *a += f(k, i);
                    }
                }
                weighted(&arr)
            };
            let no_no = level(&|k, i| rj[k][i].min(1.0 / size));
            let sw: Vec<f64> = sj.iter().map(|v| weighted(v)).collect();
            let no_med = level(&|k, i| rj[k][i].min(sw[k]));
            let med_med = level(&|k, i| rj[k][i].min(sj[k][i]));
            let mut med_per = 0.0;
            for k in 0..cats.len() {
                med_per += weighted(&rj[k]).min(sw[k]);
            }
            t.rows.push(vec![side as f64, no_no, no_med, med_med, med_per, 1.0]);
            if ny < scale || nx < scale {
                continue;
            }
            let (by, bx, bw) = block_sums(&w, ny, nx, scale);
            let merge = |v: &Vec<f64>| -> Vec<f64> {
                let vw: Vec<f64> = v.iter().zip(&w).map(|(a, b)| a * b).collect();
                let (_, _, s) = block_sums(&vw, ny, nx, scale);
                s.iter().zip(&bw).map(|(&x, &d)| if d == 0.0 { 0.0 } else { 1.0 * x / d }).collect()
            };
            rj = rj.iter().map(merge).collect();
            sj = sj.iter().map(merge).collect();
            w = bw.iter().map(|v| 1.0 * v / (scale * scale) as f64).collect();
            (ny, nx) = (by, bx);
            side *= scale;
        }
        Ok(outputs([("table", Value::Table(t))]))
    }
}

// ------------------------------------------------------------------------------------------- FUTURES

/// r.futures.validation of the cells (original, reference, simulated or reference, simulated).
struct FuturesResult {
    classes: Vec<f64>,
    quantity: Vec<f64>,
    allocation: Vec<f64>,
    /// total_quantity, total_allocation, kappa, kappasimulation, misses, hits, false_alarms, null_successes,
    /// figure_of_merit, producer, user, initially_developed (NaN where None; the last nine without an original
    /// absent).
    scalars: Vec<(&'static str, f64)>,
}

const FUTURES_SCALARS: [&str; 12] = [
    "total_quantity",
    "total_allocation",
    "kappa",
    "kappasimulation",
    "misses",
    "hits",
    "false_alarms",
    "null_successes",
    "figure_of_merit",
    "producer",
    "user",
    "initially_developed",
];

fn futures_validation(
    original: Option<&[f64]>,
    reference: &[f64],
    simulated: &[f64],
) -> std::result::Result<FuturesResult, String> {
    let n_cells = reference.len();
    let mut combos: BTreeMap<Vec<i64>, f64> = BTreeMap::new();
    for i in 0..n_cells {
        let vals: Vec<f64> = original.map(|o| o[i]).into_iter().chain([reference[i], simulated[i]]).collect();
        if vals.iter().any(|v| v.is_nan()) {
            continue;
        }
        if vals.iter().any(|v| (v - v.round_ties_even()).abs() > 0.0) {
            return Err("the maps must hold integer classes".into());
        }
        *combos.entry(vals.iter().map(|&v| v as i64).collect()).or_default() += 1.0;
    }
    let mut cats: Vec<i64> = combos.keys().flatten().copied().collect();
    cats.sort_unstable();
    cats.dedup();
    let with_original = original.is_some();
    let names: &[&str] = if with_original { &FUTURES_SCALARS } else { &FUTURES_SCALARS[..3] };
    if cats.is_empty() {
        return Ok(FuturesResult {
            classes: vec![],
            quantity: vec![],
            allocation: vec![],
            scalars: names.iter().map(|&k| (k, f64::NAN)).collect(),
        });
    }
    let n = cats.len();
    let index = |v: i64| cats.binary_search(&v).unwrap();
    let (mut ref_sim, mut orig_sim, mut orig_ref) = (vec![0.0; n * n], vec![0.0; n * n], vec![0.0; n * n]);
    let binary = cats == [0, 1];
    let (mut hits, mut false_alarms, mut misses, mut null_successes, mut initially) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (line, &cnt) in &combos {
        if with_original {
            let (o, r, s) = (index(line[0]), index(line[1]), index(line[2]));
            ref_sim[s * n + r] += cnt;
            orig_sim[o * n + s] += cnt;
            orig_ref[o * n + r] += cnt;
            if binary {
                if o == 0 {
                    match (r, s) {
                        (0, 1) => false_alarms += cnt,
                        (1, 0) => misses += cnt,
                        (1, 1) => hits += cnt,
                        _ => null_successes += cnt,
                    }
                } else {
                    initially += cnt;
                }
            }
        } else {
            let (r, s) = (index(line[0]), index(line[1]));
            ref_sim[s * n + r] += cnt;
        }
    }
    let mut extra: Vec<(&'static str, f64)> = Vec::new();
    if with_original && binary {
        let total = misses + hits + false_alarms + null_successes + initially;
        let share = |v: f64| if total != 0.0 { v / total } else { f64::NAN };
        let ratio = |a: f64, b: f64| if b != 0.0 { a / b } else { 0.0 };
        extra = vec![
            ("misses", share(misses)),
            ("hits", share(hits)),
            ("false_alarms", share(false_alarms)),
            ("null_successes", share(null_successes)),
            ("figure_of_merit", ratio(hits, misses + hits + false_alarms)),
            ("producer", ratio(hits, misses + hits)),
            ("user", ratio(hits, hits + false_alarms)),
            ("initially_developed", share(initially)),
        ];
    } else if with_original {
        extra = FUTURES_SCALARS[4..].iter().map(|&k| (k, f64::NAN)).collect();
    }
    let col_sum: Vec<f64> = (0..n).map(|j| (0..n).map(|i| ref_sim[i * n + j]).sum()).collect();
    let row_sum: Vec<f64> = (0..n).map(|i| ref_sim[i * n..(i + 1) * n].iter().sum()).collect();
    let total: f64 = ref_sim.iter().sum();
    let quantity: Vec<f64> = (0..n).map(|k| (col_sum[k] - row_sum[k]).abs() / total).collect();
    let diag: Vec<f64> = (0..n).map(|k| ref_sim[k * n + k]).collect();
    let allocation: Vec<f64> = (0..n).map(|k| 2.0 * (col_sum[k] - diag[k]).min(row_sum[k] - diag[k]) / total).collect();
    let p0 = diag.iter().sum::<f64>() / total;
    let pe = numpy_sum(&(0..n).map(|k| (col_sum[k] / total) * (row_sum[k] / total)).collect::<Vec<_>>());
    let ratio = |a: f64, b: f64| if b == 0.0 || (a / b).is_nan() { f64::NAN } else { a / b };
    let mut scalars = vec![
        ("total_quantity", numpy_sum(&quantity) / 2.0),
        ("total_allocation", numpy_sum(&allocation) / 2.0),
        ("kappa", ratio(p0 - pe, 1.0 - pe)),
    ];
    if with_original {
        let orow: Vec<f64> = (0..n).map(|i| orig_sim[i * n..(i + 1) * n].iter().sum()).collect();
        let rrow: Vec<f64> = (0..n).map(|i| orig_ref[i * n..(i + 1) * n].iter().sum()).collect();
        let ototal: f64 = orig_sim.iter().sum();
        let e: Vec<f64> = (0..n)
            .map(|i| {
                let c: Vec<f64> =
                    (0..n).map(|j| (orig_sim[i * n + j] / orow[i]) * (orig_ref[i * n + j] / rrow[i])).collect();
                numpy_sum(&c) * (orow[i] / ototal)
            })
            .collect();
        let pe = numpy_sum(&e.iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect::<Vec<_>>());
        scalars.push(("kappasimulation", ratio(p0 - pe, 1.0 - pe)));
        scalars.extend(extra);
    }
    Ok(FuturesResult { classes: cats.iter().map(|&c| c as f64).collect(), quantity, allocation, scalars })
}

struct Futures;

impl Operator for Futures {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "validate.futures",
            "FUTURES' validation (r.futures.validation through landschaft): the quantity and allocation \
             disagreement per class and in total (Pontius & Millones 2011) and kappa; with the original map, kappa \
             simulation (van Vliet et al. 2011) and, for 0/1 maps, misses, hits, false alarms, null successes, the \
             figure of merit, producer's and user's accuracy (Pontius et al. 2008).",
            Access::Global,
        )
        .input(PortSpec::new("reference", Type::Map, "the observed map (integer classes)"))
        .input(PortSpec::new("simulated", Type::Map, "the simulated map"))
        .input(PortSpec::optional("original", Type::Map, "the map at the start"))
        .output(PortSpec::new("metrics", Type::Table, "the scalar metrics (empty where they cannot be computed)"))
        .output(PortSpec::new("classes", Type::Table, "per class: quantity and allocation disagreement"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = maps(args, &["reference", "simulated"])?;
        let original = args.opt_map("original")?;
        if original.is_some_and(|o| o.data.len() != m[0].data.len()) {
            return Err(Error::msg(format!("node {:?}: the maps differ in shape", args.node)));
        }
        let r = futures_validation(original.map(|o| &o.data[..]), &m[0].data, &m[1].data)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let mut metrics = Table::new(r.scalars.iter().map(|(k, _)| k.to_string()).collect());
        metrics.rows = vec![r.scalars.iter().map(|(_, v)| *v).collect()];
        let mut classes = Table::new(cols(&["class", "quantity", "allocation"]));
        classes.rows = (0..r.classes.len()).map(|k| vec![r.classes[k], r.quantity[k], r.allocation[k]]).collect();
        Ok(outputs([("metrics", Value::Table(metrics)), ("classes", Value::Table(classes))]))
    }
}

struct FuturesGrid;

impl Operator for FuturesGrid {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "validate.futures_grid",
            "FUTURES' validation per tile or per region (r.futures.gridvalidation through landschaft): a row per \
             tile of size x size cells from the top-left (its row, column and centre) or per region, the metrics \
             of validate.futures and the cells compared.",
            Access::Global,
        )
        .input(PortSpec::new("reference", Type::Map, "the observed map (integer classes)"))
        .input(PortSpec::new("simulated", Type::Map, "the simulated map"))
        .input(PortSpec::optional("original", Type::Map, "the map at the start"))
        .input(PortSpec::optional("regions", Type::Map, "region ids, in place of tiles"))
        .output(PortSpec::new("table", Type::Table, "per tile or region: where, cells, the metrics"))
        .param(PortSpec::optional("size", Type::Integer, "the tiles' side in cells"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let m = maps(args, &["reference", "simulated"])?;
        let original = args.opt_map("original")?;
        let regions = args.opt_map("regions")?;
        let size: Option<usize> = args.opt_param("size")?;
        if size.is_some() == regions.is_some() {
            return Err(fail("give the tile size or the regions".into()));
        }
        let (ny, nx) = (m[0].grid.height, m[0].grid.width);
        let all: Vec<&[f64]> =
            [&m[0].data[..], &m[1].data[..]].into_iter().chain(original.map(|o| &o.data[..])).collect();
        if all.iter().any(|v| v.len() != ny * nx) || regions.is_some_and(|r| r.data.len() != ny * nx) {
            return Err(fail("the maps differ in shape".into()));
        }
        let mut rows: Vec<Vec<(String, f64)>> = Vec::new();
        let mut one = |sel: &dyn Fn(usize) -> bool, info: Vec<(String, f64)>| -> Result<()> {
            let sub: Vec<Vec<f64>> =
                all.iter().map(|v| (0..ny * nx).map(|i| if sel(i) { v[i] } else { f64::NAN }).collect()).collect();
            let r = futures_validation(sub.get(2).map(|v| &v[..]), &sub[0], &sub[1]).map_err(fail)?;
            let mut row = info;
            let cells = (0..ny * nx).filter(|&i| sel(i) && all.iter().all(|v| !v[i].is_nan())).count();
            row.push(("cells".into(), cells as f64));
            for (k, v) in &r.scalars {
                row.push((k.to_string(), *v));
            }
            for (i, c) in r.classes.iter().enumerate() {
                row.push((format!("quantity_class_{}", *c as i64), r.quantity[i]));
                row.push((format!("allocation_class_{}", *c as i64), r.allocation[i]));
            }
            rows.push(row);
            Ok(())
        };
        if let Some(size) = size {
            if size < 1 {
                return Err(fail("size must be at least 1".into()));
            }
            let t = m[0].grid.transform;
            let xs: Vec<f64> = (0..nx).map(|c| (c as f64 + 0.5) * t[1] + t[0]).collect();
            let ys: Vec<f64> = (0..ny).map(|r| (r as f64 + 0.5) * t[5] + t[3]).collect();
            for r0 in (0..ny).step_by(size) {
                for c0 in (0..nx).step_by(size) {
                    let sel = |i: usize| (r0..r0 + size).contains(&(i / nx)) && (c0..c0 + size).contains(&(i % nx));
                    let info = vec![
                        ("row".to_string(), (r0 / size) as f64),
                        ("col".to_string(), (c0 / size) as f64),
                        ("x".to_string(), np_mean(&xs[c0..(c0 + size).min(nx)])),
                        ("y".to_string(), np_mean(&ys[r0..(r0 + size).min(ny)])),
                    ];
                    one(&sel, info)?;
                }
            }
        } else {
            let reg = &regions.unwrap().data;
            let mut ids: Vec<f64> = reg.iter().copied().filter(|v| !v.is_nan()).collect();
            ids.sort_by(f64::total_cmp);
            ids.dedup();
            for id in ids {
                one(&|i: usize| reg[i] == id, vec![("region".to_string(), id)])?;
            }
        }
        // pandas' columns: in the order they first appear
        let mut names: Vec<String> = Vec::new();
        for r in &rows {
            for (k, _) in r {
                if !names.contains(k) {
                    names.push(k.clone());
                }
            }
        }
        let mut t = Table::new(names.clone());
        t.rows = rows
            .iter()
            .map(|r| names.iter().map(|n| r.iter().find(|(k, _)| k == n).map_or(f64::NAN, |(_, v)| *v)).collect())
            .collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}
