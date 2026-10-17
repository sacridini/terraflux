//! Global sensitivity analysis as SALib (1.6) does it, the same numbers: Morris's elementary effects (the
//! trajectories of a grid of levels; μ, μ*, σ and μ*'s bootstrap interval) and Sobol's indices from Saltelli's
//! sampling of a scrambled Sobol' sequence (scipy.stats.qmc's; the first and total order indices, the second order
//! ones, and their bootstrap intervals). `sensitivity.design` gives the runs, `sensitivity.analyze` the indices from
//! their results (`terraflux sensitivity` runs a model's runs, here or on many computers).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::change::numpy_sum;
use crate::signatures::NumpyRng;
pub use crate::special::ndtri;
use crate::sobol_directions::{POLY, VINIT};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Design));
    r.add(Arc::new(Analyze));
}

// -- Sobol' sequences -----------------------------------------------------------------------------------------------

const BITS: usize = 30;

/// scipy.stats.qmc.Sobol(d, scramble, seed).random(n): Joe and Kuo's direction numbers, scrambled by random lower
/// triangular matrices and a digital shift (numpy's default_rng(seed)), the points in Gray code order.
#[allow(clippy::needless_range_loop)]
pub fn sobol_points(d: usize, n: usize, scramble: bool, seed: u64) -> std::result::Result<Vec<Vec<f64>>, String> {
    if d > POLY.len() {
        return Err(format!("at most {} dimensions", POLY.len()));
    }
    let mut sv = vec![[0u32; BITS]; d];
    for v in sv.first_mut().iter_mut().flat_map(|r| r.iter_mut()) {
        *v = 1;
    }
    for k in 1..d {
        let p = POLY[k];
        let m = (32 - p.leading_zeros()) as usize - 1;
        sv[k][..m].copy_from_slice(&VINIT[k][..m]);
        for j in m..BITS {
            let mut newv = sv[k][j - m];
            let mut pow2 = 1u32;
            for i in 0..m {
                pow2 <<= 1;
                if (p >> (m - 1 - i)) & 1 == 1 {
                    newv ^= pow2.wrapping_mul(sv[k][j - i - 1]);
                }
            }
            sv[k][j] = newv;
        }
    }
    let mut pow2 = 1u32;
    for b in 0..BITS {
        for row in sv.iter_mut() {
            row[BITS - 1 - b] = row[BITS - 1 - b].wrapping_mul(pow2);
        }
        pow2 <<= 1;
    }
    let mut quasi = vec![0u32; d];
    if scramble {
        let mut rng = NumpyRng::new(seed);
        for q in quasi.iter_mut() {
            for b in 0..BITS {
                *q = q.wrapping_add((rng.integers(2) as u32) << b);
            }
        }
        // lower triangular matrices, their diagonals 1
        let mut ltm = vec![[[0u32; BITS]; BITS]; d];
        for m in ltm.iter_mut() {
            for (i, row) in m.iter_mut().enumerate() {
                for (j, x) in row.iter_mut().enumerate() {
                    let bit = rng.integers(2) as u32;
                    if j <= i {
                        *x = bit;
                    }
                }
                row[i] = 1;
            }
        }
        for k in 0..d {
            for j in 0..BITS {
                let vdj = sv[k][j];
                let (mut l, mut t2) = (1u32, 0u32);
                for p in (0..BITS).rev() {
                    let mut lsmdp = 0u32;
                    let mut pw = 1u32;
                    for i in 0..BITS {
                        lsmdp = lsmdp.wrapping_add(ltm[k][p][BITS - 1 - i].wrapping_mul(pw));
                        pw = pw.wrapping_mul(2);
                    }
                    let t1 = (0..BITS).map(|b| ((lsmdp >> b) & 1) * ((vdj >> b) & 1)).sum::<u32>() % 2;
                    t2 = t2.wrapping_add(t1.wrapping_mul(l));
                    l = l.wrapping_mul(2);
                }
                sv[k][j] = t2;
            }
        }
    }
    let scale = 1.0 / (1u64 << BITS) as f64;
    let mut out = Vec::with_capacity(n);
    if n > 0 {
        out.push(quasi.iter().map(|&q| f64::from(q) * scale).collect());
    }
    for i in 0..n.saturating_sub(1) as u64 {
        let l = (!i).trailing_zeros() as usize;
        for (q, row) in quasi.iter_mut().zip(&sv) {
            *q ^= row[l];
        }
        out.push(quasi.iter().map(|&q| f64::from(q) * scale).collect());
    }
    Ok(out)
}

/// SALib's scale_samples: unit values onto the factors' ranges.
fn scale(rows: &mut [Vec<f64>], bounds: &[(f64, f64)]) {
    for row in rows {
        for (x, (lo, hi)) in row.iter_mut().zip(bounds) {
            *x = *x * (hi - lo) + lo;
        }
    }
}

/// SALib's sobol.sample (Saltelli's scheme): per base point A, then A with each factor from B, then (second order)
/// B with each factor from A, then B; `n` base points of a sequence of twice the factors.
pub fn saltelli_sample(
    bounds: &[(f64, f64)],
    n: usize,
    second_order: bool,
    seed: u64,
) -> std::result::Result<Vec<Vec<f64>>, String> {
    let d = bounds.len();
    let base = sobol_points(2 * d, n, true, seed)?;
    let mut rows = Vec::new();
    for b in &base {
        rows.push(b[..d].to_vec());
        for k in 0..d {
            rows.push((0..d).map(|j| if j == k { b[j + d] } else { b[j] }).collect());
        }
        if second_order {
            for k in 0..d {
                rows.push((0..d).map(|j| if j == k { b[j] } else { b[j + d] }).collect());
            }
        }
        rows.push(b[d..].to_vec());
    }
    scale(&mut rows, bounds);
    Ok(rows)
}

// -- Morris ---------------------------------------------------------------------------------------------------------

/// The step of a grid of `levels` levels.
fn delta(levels: usize) -> f64 {
    levels as f64 / (2.0 * (levels as f64 - 1.0))
}

/// SALib's morris.sample: `trajectories` trajectories of a factor at a time on a grid of `levels` levels (numpy's
/// default_rng(seed): the order of the factors, the directions, the start), scaled to the ranges.
pub fn morris_sample(bounds: &[(f64, f64)], trajectories: usize, levels: usize, seed: u64) -> Vec<Vec<f64>> {
    let k = bounds.len();
    let dl = delta(levels);
    let half = levels / 2;
    let bound = 1.0 - dl;
    let grid: Vec<f64> = (0..half)
        .map(|i| if half > 1 && i == half - 1 { bound } else { i as f64 * (bound / (half.max(2) - 1) as f64) })
        .collect();
    let mut rng = NumpyRng::new(seed);
    let mut rows = Vec::with_capacity(trajectories * (k + 1));
    for _ in 0..trajectories {
        // P*: the identity's rows shuffled
        let mut perm: Vec<usize> = (0..k).collect();
        rng.shuffle(&mut perm);
        let dstar: Vec<f64> = (0..k).map(|_| if rng.integers(2) == 0 { -1.0 } else { 1.0 }).collect();
        let xstar: Vec<f64> = (0..k).map(|_| grid[rng.integers(grid.len() as u64) as usize]).collect();
        for i in 0..=k {
            rows.push(
                (0..k)
                    .map(|j| {
                        // 2 B P*ᵀ: twice the rows of P* before i with a one in column j
                        let c: f64 = (0..i).map(|a| 2.0 * f64::from(u8::from(perm[j] == a))).sum();
                        let d = (c - 1.0) * dstar[j];
                        xstar[j] + (dl / 2.0) * (d + 1.0)
                    })
                    .collect(),
            );
        }
    }
    scale(&mut rows, bounds);
    rows
}

/// numpy's std of contiguous values (pairwise sums), with `ddof`.
fn std(v: &[f64], ddof: f64) -> f64 {
    let n = v.len() as f64;
    let m = numpy_sum(v) / n;
    (numpy_sum(&v.iter().map(|x| (x - m) * (x - m)).collect::<Vec<_>>()) / (n - ddof)).sqrt()
}

/// What Morris's method gives each factor.
#[derive(Debug, Clone, PartialEq)]
pub struct Morris {
    pub mu: Vec<f64>,
    pub mu_star: Vec<f64>,
    pub sigma: Vec<f64>,
    pub mu_star_conf: Vec<f64>,
}

/// SALib's morris.analyze: the elementary effects of the trajectories of `x` (rows, a column per factor) and their
/// outputs `y`; μ*'s interval from `resamples` bootstraps (numpy's default_rng(seed)).
#[allow(clippy::needless_range_loop)]
pub fn morris_analyze(
    x: &[Vec<f64>],
    y: &[f64],
    levels: usize,
    resamples: usize,
    conf: f64,
    seed: u64,
) -> std::result::Result<Morris, String> {
    let k = x.first().map_or(0, Vec::len);
    if k == 0 || y.len() != x.len() || !x.len().is_multiple_of(k + 1) {
        return Err(format!("{} runs and {} outputs: trajectories of {} runs", x.len(), y.len(), k + 1));
    }
    let t = x.len() / (k + 1);
    let dl = delta(levels);
    // ee[v][t]: the output where the factor is higher less where it is lower, over the step
    let mut ee = vec![vec![0.0; t]; k];
    for tr in 0..t {
        let base = tr * (k + 1);
        for s in 0..k {
            let (a, b) = (&x[base + s], &x[base + s + 1]);
            for v in 0..k {
                let diff = b[v] - a[v];
                if diff > 0.0 {
                    ee[v][tr] = (y[base + s + 1] - y[base + s]) / dl;
                } else if diff < 0.0 {
                    ee[v][tr] = (y[base + s] - y[base + s + 1]) / dl;
                }
            }
        }
    }
    let n = t as f64;
    let seq = |v: &[f64]| v.iter().fold(0.0, |a, b| a + b);
    let mu: Vec<f64> = ee.iter().map(|e| seq(e) / n).collect();
    let mu_star: Vec<f64> = ee.iter().map(|e| seq(&e.iter().map(|x| x.abs()).collect::<Vec<_>>()) / n).collect();
    let sigma: Vec<f64> = ee
        .iter()
        .zip(&mu)
        .map(|(e, m)| (seq(&e.iter().map(|x| (x - m) * (x - m)).collect::<Vec<_>>()) / (n - 1.0)).sqrt())
        .collect();
    let mut rng = NumpyRng::new(seed);
    let z = ndtri(0.5 + conf / 2.0);
    let mu_star_conf = ee
        .iter()
        .map(|e| {
            let draws: Vec<f64> = (0..resamples)
                .map(|_| {
                    let r: Vec<f64> = (0..t).map(|_| e[rng.integers(t as u64) as usize].abs()).collect();
                    numpy_sum(&r) / n
                })
                .collect();
            z * std(&draws, 1.0)
        })
        .collect();
    Ok(Morris { mu, mu_star, sigma, mu_star_conf })
}

// -- Sobol' indices -------------------------------------------------------------------------------------------------

/// What Sobol's method gives each factor (and pair of factors: NaN off the upper triangle).
#[derive(Debug, Clone, PartialEq)]
pub struct Sobol {
    pub s1: Vec<f64>,
    pub s1_conf: Vec<f64>,
    pub st: Vec<f64>,
    pub st_conf: Vec<f64>,
    pub s2: Vec<Vec<f64>>,
    pub s2_conf: Vec<Vec<f64>>,
}

/// Columns of values (the resampled outputs), or one column.
struct Cols {
    rows: usize,
    cols: usize,
    v: Vec<f64>,
}

impl Cols {
    fn one(v: &[f64]) -> Cols {
        Cols { rows: v.len(), cols: 1, v: v.to_vec() }
    }
    fn at(&self, i: usize, q: usize) -> f64 {
        self.v[i * self.cols + q]
    }
}

/// numpy's mean along the rows of every column: pairwise over one column, in order down the columns of several.
fn col_means(m: &Cols, f: &dyn Fn(usize, usize) -> f64) -> Vec<f64> {
    let n = m.rows as f64;
    if m.cols == 1 {
        return vec![numpy_sum(&(0..m.rows).map(|i| f(i, 0)).collect::<Vec<_>>()) / n];
    }
    (0..m.cols).map(|q| (0..m.rows).fold(0.0, |a, i| a + f(i, q)) / n).collect()
}

/// np.var(np.r_[A, B], axis=0), and np.ptp of it all.
fn var_of(a: &Cols, b: &Cols) -> (Vec<f64>, f64) {
    let rows = a.rows + b.rows;
    let get = |i: usize, q: usize| if i < a.rows { a.at(i, q) } else { b.at(i - a.rows, q) };
    let n = rows as f64;
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for v in a.v.iter().chain(&b.v) {
        lo = lo.min(*v);
        hi = hi.max(*v);
    }
    let var = if a.cols == 1 {
        let y: Vec<f64> = (0..rows).map(|i| get(i, 0)).collect();
        let m = numpy_sum(&y) / n;
        vec![numpy_sum(&y.iter().map(|v| (v - m) * (v - m)).collect::<Vec<_>>()) / n]
    } else {
        (0..a.cols)
            .map(|q| {
                let m = (0..rows).fold(0.0, |s, i| s + get(i, q)) / n;
                (0..rows).fold(0.0, |s, i| {
                    let d = get(i, q) - m;
                    s + d * d
                }) / n
            })
            .collect()
    };
    (var, hi - lo)
}

fn divided(num: Vec<f64>, var: &[f64]) -> Vec<f64> {
    num.iter().zip(var).map(|(n, v)| if *v > f64::EPSILON { n / v } else { 0.0 }).collect()
}

fn first_order(a: &Cols, ab: &Cols, b: &Cols) -> Vec<f64> {
    let (var, ptp) = var_of(a, b);
    if ptp <= f64::EPSILON {
        return vec![0.0; a.cols];
    }
    divided(col_means(a, &|i, q| b.at(i, q) * (ab.at(i, q) - a.at(i, q))), &var)
}

fn total_order(a: &Cols, ab: &Cols, b: &Cols) -> Vec<f64> {
    let (var, ptp) = var_of(a, b);
    if ptp <= f64::EPSILON {
        return vec![0.0; a.cols];
    }
    let m = col_means(a, &|i, q| {
        let d = a.at(i, q) - ab.at(i, q);
        d * d
    });
    divided(m.iter().map(|x| 0.5 * x).collect(), &var)
}

fn second_order(a: &Cols, abj: &Cols, abk: &Cols, baj: &Cols, b: &Cols) -> Vec<f64> {
    let (var, ptp) = var_of(a, b);
    if ptp <= f64::EPSILON {
        return vec![0.0; a.cols];
    }
    let vjk = divided(col_means(a, &|i, q| baj.at(i, q) * abk.at(i, q) - a.at(i, q) * b.at(i, q)), &var);
    let (sj, sk) = (first_order(a, abj, b), first_order(a, abk, b));
    vjk.iter().zip(&sj).zip(&sk).map(|((v, j), k)| v - j - k).collect()
}

/// SALib's sobol.analyze: the indices from the outputs of Saltelli's runs (`d` factors), the intervals from
/// `resamples` bootstraps (numpy's default_rng(seed)).
#[allow(clippy::needless_range_loop)]
pub fn sobol_analyze(
    y: &[f64],
    d: usize,
    second_order_too: bool,
    resamples: usize,
    conf: f64,
    seed: u64,
) -> std::result::Result<Sobol, String> {
    let step = if second_order_too { 2 * d + 2 } else { d + 2 };
    if d == 0 || !y.len().is_multiple_of(step) {
        return Err(format!("{} outputs are not Saltelli's runs of {d} factors", y.len()));
    }
    let n = y.len() / step;
    // the outputs standardised
    let mean = numpy_sum(y) / y.len() as f64;
    let sd = std(y, 0.0);
    let y: Vec<f64> = y.iter().map(|v| (v - mean) / sd).collect();
    let pick = |off: usize| -> Vec<f64> { (0..n).map(|i| y[i * step + off]).collect() };
    let (a, b) = (pick(0), pick(step - 1));
    let ab: Vec<Vec<f64>> = (0..d).map(|j| pick(j + 1)).collect();
    let ba: Vec<Vec<f64>> = if second_order_too { (0..d).map(|j| pick(j + 1 + d)).collect() } else { Vec::new() };
    let mut rng = NumpyRng::new(seed);
    let r: Vec<usize> = (0..n * resamples).map(|_| rng.integers(n as u64) as usize).collect();
    let resampled = |v: &[f64]| Cols { rows: n, cols: resamples, v: r.iter().map(|&k| v[k]).collect() };
    let z = ndtri(0.5 + conf / 2.0);
    let (ra, rb) = (resampled(&a), resampled(&b));
    let (ca, cb) = (Cols::one(&a), Cols::one(&b));
    let mut s = Sobol {
        s1: vec![0.0; d],
        s1_conf: vec![0.0; d],
        st: vec![0.0; d],
        st_conf: vec![0.0; d],
        s2: vec![vec![f64::NAN; d]; d],
        s2_conf: vec![vec![f64::NAN; d]; d],
    };
    let (_, var_diff) = var_of(&ra, &rb);
    for j in 0..d {
        let (cab, rab) = (Cols::one(&ab[j]), resampled(&ab[j]));
        s.s1[j] = first_order(&ca, &cab, &cb)[0];
        s.s1_conf[j] = if var_diff != 0.0 { z * std(&first_order(&ra, &rab, &rb), 1.0) } else { 0.0 };
        s.st[j] = total_order(&ca, &cab, &cb)[0];
        s.st_conf[j] = if var_diff != 0.0 { z * std(&total_order(&ra, &rab, &rb), 1.0) } else { 0.0 };
    }
    if second_order_too {
        for j in 0..d {
            for k in j + 1..d {
                let (abj, abk, baj) = (Cols::one(&ab[j]), Cols::one(&ab[k]), Cols::one(&ba[j]));
                s.s2[j][k] = second_order(&ca, &abj, &abk, &baj, &cb)[0];
                let (rabj, rabk, rbaj) = (resampled(&ab[j]), resampled(&ab[k]), resampled(&ba[j]));
                s.s2_conf[j][k] = z * std(&second_order(&ra, &rabj, &rabk, &rbaj, &rb), 1.0);
            }
        }
    }
    Ok(s)
}

// -- the operators --------------------------------------------------------------------------------------------------

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

/// The factors of a setting: a list of [name, low, high] (their order), or {name: [low, high]} (by name).
pub fn factors_of(v: Option<&serde_json::Value>) -> std::result::Result<Vec<(String, f64, f64)>, String> {
    let bad = || "factors: [[name, low, high], ...] or {name: [low, high]}".to_string();
    let out: Vec<(String, f64, f64)> = match v {
        Some(serde_json::Value::Array(items)) => items
            .iter()
            .map(|i| match i.as_array().map(Vec::as_slice) {
                Some([n, lo, hi]) => Ok((
                    n.as_str().ok_or_else(bad)?.to_string(),
                    lo.as_f64().ok_or_else(bad)?,
                    hi.as_f64().ok_or_else(bad)?,
                )),
                _ => Err(bad()),
            })
            .collect::<std::result::Result<_, _>>()?,
        Some(serde_json::Value::Object(o)) => o
            .iter()
            .map(|(n, r)| match r.as_array().map(Vec::as_slice) {
                Some([lo, hi]) => Ok((n.clone(), lo.as_f64().ok_or_else(bad)?, hi.as_f64().ok_or_else(bad)?)),
                _ => Err(bad()),
            })
            .collect::<std::result::Result<_, _>>()?,
        _ => return Err(bad()),
    };
    if out.is_empty() || out.iter().any(|(_, lo, hi)| hi.is_nan() || lo.is_nan() || hi <= lo) {
        return Err("factors: one at least, each with low below high".into());
    }
    Ok(out)
}

struct Design;

impl Operator for Design {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "sensitivity.design",
            "The runs of a global sensitivity analysis (SALib's samplers, the same values): Morris's trajectories \
             (a factor changed at a time on a grid of levels; n trajectories of factors + 1 runs) or Saltelli's \
             runs of Sobol's method (a scrambled Sobol' sequence; n base points, a power of 2, of 2 × factors + 2 \
             runs, or factors + 2 without the second order). Run the model at every row (`terraflux \
             sensitivity`, or a loop) and give the outputs to sensitivity.analyze.",
            Access::None,
        )
        .output(PortSpec::new("table", Type::Table, "a row per run: run, then a column per factor"))
        .param(PortSpec::new("method", Type::Text, "morris or sobol").choices(["morris", "sobol"]))
        .param(PortSpec::new(
            "factors",
            Type::Any,
            "the factors and their ranges: [[name, low, high], ...] (or {name: [low, high]}); a model's input \
             ($name) or setting (node.setting) for terraflux sensitivity",
        ))
        .param(PortSpec::optional("n", Type::Integer, "trajectories (morris) or base points (sobol; default 10 / 64)"))
        .param(PortSpec::optional("levels", Type::Integer, "morris: the grid's levels (default 4)"))
        .param(PortSpec::optional("second_order", Type::Bool, "sobol: the pairs' indices too (default true)"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of numpy's default_rng (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let f = factors_of(args.params.get("factors")).map_err(|e| fail(args, e))?;
        let bounds: Vec<(f64, f64)> = f.iter().map(|(_, a, b)| (*a, *b)).collect();
        let method: String = args.param("method", String::new())?;
        let seed: u64 = args.param("seed", 1)?;
        let rows = match method.as_str() {
            "morris" => morris_sample(&bounds, args.param("n", 10)?, args.param("levels", 4)?, seed),
            "sobol" => saltelli_sample(&bounds, args.param("n", 64)?, args.param("second_order", true)?, seed)
                .map_err(|e| fail(args, e))?,
            m => return Err(fail(args, format!("method: morris or sobol, not {m:?}"))),
        };
        let mut t = Table::new(std::iter::once("run".to_string()).chain(f.iter().map(|(n, ..)| n.clone())).collect());
        t.rows =
            rows.into_iter().enumerate().map(|(k, r)| std::iter::once((k + 1) as f64).chain(r).collect()).collect();
        Ok(outputs([("table", Value::Table(t))]))
    }
}

struct Analyze;

impl Operator for Analyze {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "sensitivity.analyze",
            "The indices of a global sensitivity analysis from its runs and their outputs (SALib's analyses, the \
             same numbers): Morris's μ, μ* (the mean absolute elementary effect: the importance) and σ (the \
             interactions and non-linearity) with μ*'s bootstrap interval; or Sobol's first order (S1: the share \
             of the output's variance due to the factor alone), total (ST: with its interactions) and second order \
             (S2: of pairs) indices with their bootstrap intervals.",
            Access::None,
        )
        .input(PortSpec::new("design", Type::Table, "the runs (sensitivity.design's table)"))
        .input(PortSpec::new("results", Type::Table, "the outputs of the runs, a row each in the same order"))
        .output(PortSpec::new(
            "indices",
            Type::Table,
            "a row per factor (in the design's order): factor (1..), then mu, mu_star, sigma, mu_star_conf (morris) \
             or s1, s1_conf, st, st_conf (sobol)",
        ))
        .output(PortSpec::optional("s2", Type::Table, "sobol: the second order indices, a row and a column per factor"))
        .output(PortSpec::optional("s2_conf", Type::Table, "sobol: their intervals"))
        .param(PortSpec::new("method", Type::Text, "morris or sobol").choices(["morris", "sobol"]))
        .param(PortSpec::optional("column", Type::Text, "the results' column (default its last)"))
        .param(PortSpec::optional("levels", Type::Integer, "morris: the grid's levels, as the design's (default 4)"))
        .param(PortSpec::optional("second_order", Type::Bool, "sobol: as the design (default true)"))
        .param(PortSpec::optional("resamples", Type::Integer, "the bootstraps of the intervals (default 100)"))
        .param(PortSpec::optional("confidence", Type::Number, "the level of the intervals (default 0.95)"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of the bootstraps (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (Some(Value::Table(design)), Some(Value::Table(results))) = (args.get("design"), args.get("results"))
        else {
            return Err(fail(args, "connect the design and the results (tables)"));
        };
        let factors: Vec<String> = design.columns.iter().filter(|c| *c != "run").cloned().collect();
        let x: Vec<Vec<f64>> = design
            .rows
            .iter()
            .map(|r| design.columns.iter().zip(r).filter(|(c, _)| *c != "run").map(|(_, v)| *v).collect())
            .collect();
        let column: Option<String> = args.opt_param("column")?;
        let y = match column {
            Some(c) => results.column(&c).ok_or_else(|| fail(args, format!("the results have no column {c:?}")))?,
            None => {
                let c = results.columns.last().ok_or_else(|| fail(args, "the results have no column"))?;
                results.column(c).unwrap()
            }
        };
        if y.iter().any(|v| v.is_nan()) {
            return Err(fail(args, "a run has no output"));
        }
        let resamples: usize = args.param("resamples", 100)?;
        let conf: f64 = args.param("confidence", 0.95)?;
        let seed: u64 = args.param("seed", 1)?;
        let method: String = args.param("method", String::new())?;
        let row = |k: usize, v: &[f64]| std::iter::once((k + 1) as f64).chain(v.iter().copied()).collect::<Vec<_>>();
        match method.as_str() {
            "morris" => {
                let m = morris_analyze(&x, &y, args.param("levels", 4)?, resamples, conf, seed)
                    .map_err(|e| fail(args, e))?;
                let mut t = Table::new(["factor", "mu", "mu_star", "sigma", "mu_star_conf"].map(String::from).to_vec());
                for k in 0..factors.len() {
                    t.rows.push(row(k, &[m.mu[k], m.mu_star[k], m.sigma[k], m.mu_star_conf[k]]));
                }
                Ok(outputs([("indices", Value::Table(t))]))
            }
            "sobol" => {
                let so: bool = args.param("second_order", true)?;
                let s = sobol_analyze(&y, factors.len(), so, resamples, conf, seed).map_err(|e| fail(args, e))?;
                let mut t = Table::new(["factor", "s1", "s1_conf", "st", "st_conf"].map(String::from).to_vec());
                for k in 0..factors.len() {
                    t.rows.push(row(k, &[s.s1[k], s.s1_conf[k], s.st[k], s.st_conf[k]]));
                }
                let pairs = |m: &[Vec<f64>]| Table {
                    columns: std::iter::once("factor".to_string()).chain(factors.iter().cloned()).collect(),
                    rows: m.iter().enumerate().map(|(k, r)| row(k, r)).collect(),
                };
                let mut out = outputs([("indices", Value::Table(t))]);
                if so {
                    out.insert("s2".into(), Value::Table(pairs(&s.s2)));
                    out.insert("s2_conf".into(), Value::Table(pairs(&s.s2_conf)));
                }
                Ok(out)
            }
            m => Err(fail(args, format!("method: morris or sobol, not {m:?}"))),
        }
    }
}
