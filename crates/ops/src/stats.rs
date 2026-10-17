//! Statistics of maps: the monotonic trend of every cell along a series (`stats.trend`: the Mann-Kendall test and
//! Sen's slope, as pymannkendall's original_test), and the spatial autocorrelation of a map over the contiguity of
//! its cells (`stats.moran`: global Moran's I; `stats.lisa`: the local Moran's I and its quadrants; `stats.getis_ord`:
//! Getis and Ord's Gi* and its hot and cold spots), as PySAL's esda.

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::change::numpy_sum;
use crate::signatures::NumpyRng;
use crate::special::{ndtr, ndtri};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Trend));
    r.add(Arc::new(Moran));
    r.add(Arc::new(Lisa));
    r.add(Arc::new(GetisOrd));
}

fn fail(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

/// numpy's median of values (the mean of the two middle ones of an even number), NaN without values; a zero
/// positive (numpy's partition does not tell -0 from 0).
fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    match n {
        0 => f64::NAN,
        _ if n % 2 == 1 => v[n / 2] + 0.0,
        _ => (v[n / 2 - 1] + v[n / 2]) / 2.0 + 0.0,
    }
}

// -- the trend --------------------------------------------------------------------------------------------------------

/// The Mann-Kendall test and Sen's slope of a series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MannKendall {
    pub s: f64,
    pub var_s: f64,
    pub z: f64,
    pub p: f64,
    pub tau: f64,
    pub slope: f64,
    pub intercept: f64,
    /// 1 increasing, -1 decreasing, 0 no trend (at alpha).
    pub trend: f64,
}

/// pymannkendall's original_test: S over the values with one (ties corrected in its variance), the two-sided p of
/// its normal approximation; Sen's slope, the median of the slopes between every pair (over the positions 0, 1...),
/// and its intercept.
pub fn mann_kendall(series: &[f64], alpha: f64) -> MannKendall {
    let x: Vec<f64> = series.iter().copied().filter(|v| !v.is_nan()).collect();
    let n = x.len();
    let mut s = 0i64;
    for k in 0..n.saturating_sub(1) {
        for v in &x[k + 1..] {
            s += i64::from(*v > x[k]) - i64::from(*v < x[k]);
        }
    }
    let mut sorted = x.clone();
    sorted.sort_by(f64::total_cmp);
    let mut ties = Vec::new();
    let mut k = 0;
    while k < n {
        let mut j = k;
        while j < n && sorted[j] == sorted[k] {
            j += 1;
        }
        ties.push((j - k) as f64);
        k = j;
    }
    let nf = n as f64;
    let base = (n * n.saturating_sub(1) * (2 * n + 5)) as f64;
    let var_s = if ties.len() == n {
        base / 18.0
    } else {
        (base - numpy_sum(&ties.iter().map(|t| t * (t - 1.0) * (2.0 * t + 5.0)).collect::<Vec<_>>())) / 18.0
    };
    let s = s as f64;
    let tau = s / (0.5 * nf * (nf - 1.0));
    let z = if s > 0.0 {
        (s - 1.0) / var_s.sqrt()
    } else if s < 0.0 {
        (s + 1.0) / var_s.sqrt()
    } else {
        0.0
    };
    let p = 2.0 * (1.0 - ndtr(z.abs()));
    let h = z.abs() > ndtri(1.0 - alpha / 2.0);
    let trend = if h && z < 0.0 {
        -1.0
    } else if h && z > 0.0 {
        1.0
    } else {
        0.0
    };
    // Sen's slope over the positions of the series as it is (pairs with a missing value left out)
    let m = series.len();
    let mut d: Vec<f64> = Vec::with_capacity(m * m.saturating_sub(1) / 2);
    for i in 0..m.saturating_sub(1) {
        for j in i + 1..m {
            let v = (series[j] - series[i]) / (j - i) as f64;
            if !v.is_nan() {
                d.push(v);
            }
        }
    }
    let slope = median(&mut d);
    let mut valid = x.clone();
    let mut pos: Vec<f64> = (0..m).filter(|&i| !series[i].is_nan()).map(|i| i as f64).collect();
    let intercept = median(&mut valid) - median(&mut pos) * slope;
    MannKendall { s, var_s, z, p, tau, slope, intercept, trend }
}

/// A series' grid and layers.
type Layers = (Arc<terraflux_raster::Grid>, Vec<Arc<Vec<f64>>>);

/// The layers of a series of maps on one grid (a stack, maps along a loop, rasters on disk).
fn layers_of(args: &Args, port: &str) -> Result<Layers> {
    match args.get(port) {
        Some(Value::Stack(s)) => Ok((s.grid.clone(), s.layers.clone())),
        Some(Value::Raster(r)) => {
            let s = r.stack()?;
            Ok((s.grid, s.layers))
        }
        Some(Value::Series(items)) => {
            let maps: Vec<&Map> =
                items.iter().filter_map(|v| if let Value::Map(m) = v { Some(m) } else { None }).collect();
            if maps.len() != items.len() || maps.is_empty() {
                return Err(fail(args, "maps: a series of maps"));
            }
            Ok((maps[0].grid.clone(), maps.iter().map(|m| m.data.clone()).collect()))
        }
        Some(v) => Err(fail(args, format!("maps: a layer per date, not a {:?}", v.ty()))),
        None => Err(fail(args, "connect the maps, a layer per date")),
    }
}

struct Trend;

impl Operator for Trend {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "stats.trend",
            "The monotonic trend of every cell along a series of maps (pymannkendall's original_test, the same \
             numbers): the Mann-Kendall test (S, its variance corrected for ties, z, the two-sided p, Kendall's tau, \
             increasing or decreasing at alpha) and Sen's slope (the median of the slopes between every pair of \
             dates, per step of the series) with its intercept; the missing values of a cell left out.",
            Access::Local,
        )
        .input(PortSpec::new("maps", Type::Raster, "the maps, a layer per date (a stack, a loop's series, rasters)"))
        .output(PortSpec::new("slope", Type::Map, "Sen's slope, per step"))
        .output(PortSpec::new("intercept", Type::Map, "its intercept, at the first step"))
        .output(PortSpec::new("p", Type::Map, "the Mann-Kendall test's two-sided p"))
        .output(PortSpec::new("z", Type::Map, "its z"))
        .output(PortSpec::new("tau", Type::Map, "Kendall's tau"))
        .output(PortSpec::new("trend", Type::Map, "1 increasing, -1 decreasing, 0 none (at alpha)"))
        .param(PortSpec::optional("alpha", Type::Number, "the significance level (default 0.05)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, layers) = layers_of(args, "maps")?;
        if layers.len() < 3 {
            return Err(fail(args, "three dates at least"));
        }
        let alpha: f64 = args.param("alpha", 0.05)?;
        let n = grid.cells();
        let cells: Vec<Option<MannKendall>> = (0..n)
            .into_par_iter()
            .map(|i| {
                let series: Vec<f64> = layers.iter().map(|l| l[i]).collect();
                (series.iter().filter(|v| !v.is_nan()).count() >= 3).then(|| mann_kendall(&series, alpha))
            })
            .collect();
        let map = |f: fn(&MannKendall) -> f64| {
            Value::Map(Map::new(grid.clone(), cells.iter().map(|c| c.as_ref().map_or(f64::NAN, f)).collect()))
        };
        Ok(outputs([
            ("slope", map(|m| m.slope)),
            ("intercept", map(|m| m.intercept)),
            ("p", map(|m| m.p)),
            ("z", map(|m| m.z)),
            ("tau", map(|m| m.tau)),
            ("trend", map(|m| m.trend)),
        ]))
    }
}

// -- spatial autocorrelation ----------------------------------------------------------------------------------------

/// The cells with a value and the contiguity of each among them (rook: 4 neighbours, queen: 8), by their index
/// among the cells with a value.
pub struct Lattice {
    pub cells: Vec<usize>,
    pub neighbours: Vec<Vec<usize>>,
}

pub fn lattice(data: &[f64], ny: usize, nx: usize, queen: bool) -> Lattice {
    let mut index = vec![usize::MAX; ny * nx];
    let cells: Vec<usize> = (0..ny * nx).filter(|&i| !data[i].is_nan()).collect();
    for (k, &i) in cells.iter().enumerate() {
        index[i] = k;
    }
    let neighbours = cells
        .iter()
        .map(|&i| {
            let (r, c) = ((i / nx) as i64, (i % nx) as i64);
            let mut out = Vec::new();
            for dr in -1..=1i64 {
                for dc in -1..=1i64 {
                    if (dr == 0 && dc == 0) || (!queen && dr != 0 && dc != 0) {
                        continue;
                    }
                    let (rr, cc) = (r + dr, c + dc);
                    if rr < 0 || cc < 0 || rr >= ny as i64 || cc >= nx as i64 {
                        continue;
                    }
                    let k = index[rr as usize * nx + cc as usize];
                    if k != usize::MAX {
                        out.push(k);
                    }
                }
            }
            out
        })
        .collect();
    Lattice { cells, neighbours }
}

/// The spatial lag of row-standardised weights: the mean of every cell's neighbours (0 without any).
fn lag(l: &Lattice, z: &[f64]) -> Vec<f64> {
    l.neighbours
        .iter()
        .map(|nb| if nb.is_empty() { 0.0 } else { nb.iter().map(|&j| z[j]).sum::<f64>() / nb.len() as f64 })
        .collect()
}

/// The global Moran's I of values over a lattice (row-standardised weights) and its inference under normality
/// and randomisation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlobalMoran {
    pub i: f64,
    pub ei: f64,
    pub vi_norm: f64,
    pub z_norm: f64,
    pub p_norm: f64,
    pub vi_rand: f64,
    pub z_rand: f64,
    pub p_rand: f64,
}

/// A two-sided p of a z (esda's: the tail it falls in, doubled).
fn two_sided(z: f64) -> f64 {
    2.0 * if z > 0.0 { ndtr(-z) } else { ndtr(z) }
}

pub fn global_moran(y: &[f64], l: &Lattice) -> GlobalMoran {
    let n = y.len() as f64;
    let mean = numpy_sum(y) / n;
    let z: Vec<f64> = y.iter().map(|v| v - mean).collect();
    let z2ss = numpy_sum(&z.iter().map(|v| v * v).collect::<Vec<_>>());
    // the weights' sums: s0 the weights, s1 half the squared sums of w_ij + w_ji, s2 the squared row + column sums
    let w = |i: usize| if l.neighbours[i].is_empty() { 0.0 } else { 1.0 / l.neighbours[i].len() as f64 };
    let s0: f64 = (0..y.len()).map(|i| w(i) * l.neighbours[i].len() as f64).sum();
    let mut s1 = 0.0;
    let mut col = vec![0.0; y.len()];
    for (i, nb) in l.neighbours.iter().enumerate() {
        for &j in nb {
            let a = w(i) + w(j);
            s1 += a * a;
            col[j] += w(i);
        }
    }
    let s1 = s1 / 2.0;
    let s2: f64 = (0..y.len()).map(|i| (w(i) * l.neighbours[i].len() as f64 + col[i]).powi(2)).sum();
    let zl = lag(l, &z);
    let inum: f64 = z.iter().zip(&zl).map(|(a, b)| a * b).sum();
    let i = n / s0 * inum / z2ss;
    let ei = -1.0 / (n - 1.0);
    let (n2, s02) = (n * n, s0 * s0);
    let vi_norm = (n2 * s1 - n * s2 + 3.0 * s02) / ((n - 1.0) * (n + 1.0) * s02) - (1.0 / (n - 1.0)).powi(2);
    let k = (z.iter().map(|v| v.powi(4)).sum::<f64>() / n) / (z.iter().map(|v| v * v).sum::<f64>() / n).powi(2);
    let a = n * ((n2 - 3.0 * n + 3.0) * s1 - n * s2 + 3.0 * s02);
    let b = k * ((n2 - n) * s1 - 2.0 * n * s2 + 6.0 * s02);
    let vi_rand = (a - b) / ((n - 1.0) * (n - 2.0) * (n - 3.0) * s02) - ei * ei;
    let (z_norm, z_rand) = ((i - ei) / vi_norm.sqrt(), (i - ei) / vi_rand.sqrt());
    GlobalMoran { i, ei, vi_norm, z_norm, p_norm: two_sided(z_norm), vi_rand, z_rand, p_rand: two_sided(z_rand) }
}

/// The contiguity setting.
fn queen_of(args: &Args) -> Result<bool> {
    let c: String = args.param("contiguity", "queen".to_string())?;
    match c.as_str() {
        "queen" => Ok(true),
        "rook" => Ok(false),
        c => Err(fail(args, format!("contiguity: queen or rook, not {c:?}"))),
    }
}

fn contiguity(sig: OpSignature) -> OpSignature {
    sig.param(
        PortSpec::optional("contiguity", Type::Text, "the neighbours: queen (default, 8) or rook (4)")
            .choices(["queen", "rook"]),
    )
}

struct Moran;

impl Operator for Moran {
    fn signature(&self) -> OpSignature {
        contiguity(
            OpSignature::new(
                "stats.moran",
                "The global Moran's I of a map (PySAL's esda.Moran, to rounding): how much neighbouring cells are \
                 alike, over the contiguity of the cells with a value (row-standardised weights); its expectation, \
                 variance, z and two-sided p under normality and under randomisation, and a pseudo p of random \
                 permutations of the values.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the values"))
            .output(PortSpec::new(
                "table",
                Type::Table,
                "I, EI, VI_norm, z_norm, p_norm, VI_rand, z_rand, p_rand, p_sim, n",
            ))
            .param(PortSpec::optional("permutations", Type::Integer, "the random permutations (default 999; 0 none)"))
            .param(PortSpec::optional("seed", Type::Integer, "the seed of the permutations (default 1)")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let (ny, nx) = m.shape();
        let l = lattice(&m.data, ny, nx, queen_of(args)?);
        let y: Vec<f64> = l.cells.iter().map(|&i| m.data[i]).collect();
        if y.len() < 4 {
            return Err(fail(args, "four cells with a value at least"));
        }
        let g = global_moran(&y, &l);
        let perms: usize = args.param("permutations", 999)?;
        let p_sim = if perms > 0 {
            let mut rng = NumpyRng::new(args.param("seed", 1)?);
            let mut shuffled = y.clone();
            let larger = (0..perms)
                .filter(|_| {
                    rng.shuffle(&mut shuffled);
                    global_moran(&shuffled, &l).i >= g.i
                })
                .count();
            (larger.min(perms - larger) as f64 + 1.0) / (perms as f64 + 1.0)
        } else {
            f64::NAN
        };
        let mut t = Table::new(
            ["I", "EI", "VI_norm", "z_norm", "p_norm", "VI_rand", "z_rand", "p_rand", "p_sim", "n"]
                .map(String::from)
                .to_vec(),
        );
        t.rows.push(vec![
            g.i,
            g.ei,
            g.vi_norm,
            g.z_norm,
            g.p_norm,
            g.vi_rand,
            g.z_rand,
            g.p_rand,
            p_sim,
            y.len() as f64,
        ]);
        Ok(outputs([("table", Value::Table(t))]))
    }
}

/// The local Moran's I of every cell, its quadrant (1 high-high, 2 low-high, 3 low-low, 4 high-low) and its
/// expectation and variance under randomisation.
pub struct LocalMoran {
    pub i: Vec<f64>,
    pub quadrant: Vec<f64>,
    pub ei: Vec<f64>,
    pub vi: Vec<f64>,
    pub z: Vec<f64>,
}

pub fn local_moran(y: &[f64], l: &Lattice) -> LocalMoran {
    let n = y.len() as f64;
    let mean = numpy_sum(y) / n;
    let dev: Vec<f64> = y.iter().map(|v| v - mean).collect();
    let sd = (numpy_sum(&dev.iter().map(|v| v * v).collect::<Vec<_>>()) / n).sqrt();
    let z: Vec<f64> = dev.iter().map(|v| v / sd).collect();
    let den = numpy_sum(&z.iter().map(|v| v * v).collect::<Vec<_>>());
    let zl = lag(l, &z);
    let i: Vec<f64> = z.iter().zip(&zl).map(|(a, b)| (n - 1.0) * a * b / den).collect();
    let quadrant = z
        .iter()
        .zip(&zl)
        .map(|(a, b)| match (*a > 0.0, *b > 0.0) {
            (true, true) => 1.0,
            (false, true) => 2.0,
            (false, false) => 3.0,
            (true, false) => 4.0,
        })
        .collect();
    let m2 = numpy_sum(&z.iter().map(|v| v * v).collect::<Vec<_>>()) / n;
    let m4 = numpy_sum(&z.iter().map(|v| v.powi(4)).collect::<Vec<_>>()) / n;
    let b2 = m4 / (m2 * m2);
    let n1 = n - 1.0;
    let (mut ei, mut vi) = (Vec::with_capacity(y.len()), Vec::with_capacity(y.len()));
    for nb in &l.neighbours {
        let (wi, wi2) = if nb.is_empty() { (0.0, 0.0) } else { (1.0, 1.0 / nb.len() as f64) };
        ei.push(-wi / n1);
        let v = wi2 * (n - b2) / n1 + (wi * wi - wi2) * (2.0 * b2 - n) / (n1 * (n - 2.0)) - (-wi / n1).powi(2);
        vi.push(v);
    }
    let zs = i.iter().zip(&ei).zip(&vi).map(|((a, e), v)| (a - e) / v.sqrt()).collect();
    LocalMoran { i, quadrant, ei, vi, z: zs }
}

/// A pseudo p of every cell from conditional permutations: its neighbours drawn at random among the other cells,
/// `perms` times (a generator per cell, from the seed), the share of draws as extreme as the statistic (folded,
/// esda's).
fn conditional_p(
    n: usize,
    l: &Lattice,
    perms: usize,
    seed: u64,
    stat: &(dyn Fn(usize, &[usize]) -> f64 + Sync),
) -> Vec<f64> {
    (0..n)
        .into_par_iter()
        .map(|i| {
            let k = l.neighbours[i].len();
            if k == 0 || perms == 0 {
                return f64::NAN;
            }
            let observed = stat(i, &l.neighbours[i]);
            let mut rng = NumpyRng::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(i as u64));
            let mut larger = 0;
            for _ in 0..perms {
                let pick: Vec<usize> =
                    rng.choice_distinct(n - 1, k).into_iter().map(|j| if j >= i { j + 1 } else { j }).collect();
                if stat(i, &pick) >= observed {
                    larger += 1;
                }
            }
            (larger.min(perms - larger) as f64 + 1.0) / (perms as f64 + 1.0)
        })
        .collect()
}

fn on_map(m: &Map, l: &Lattice, v: &[f64]) -> Value {
    let mut out = vec![f64::NAN; m.data.len()];
    for (k, &i) in l.cells.iter().enumerate() {
        out[i] = v[k];
    }
    Value::Map(Map::new(m.grid.clone(), out))
}

struct Lisa;

impl Operator for Lisa {
    fn signature(&self) -> OpSignature {
        contiguity(
            OpSignature::new(
                "stats.lisa",
                "The local Moran's I of every cell (Anselin's LISA; PySAL's esda.Moran_Local, to rounding): how \
                 much it and its neighbours are alike, its quadrant (1 high among high, 2 low among high, 3 low \
                 among low, 4 high among low), its z under randomisation, a pseudo p of conditional permutations \
                 (its neighbours drawn among the other cells), and the clusters: the quadrant of the cells of p \
                 below alpha.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the values"))
            .output(PortSpec::new("i", Type::Map, "the local Moran's I"))
            .output(PortSpec::new("quadrant", Type::Map, "1 high-high, 2 low-high, 3 low-low, 4 high-low"))
            .output(PortSpec::new("z", Type::Map, "its z under randomisation"))
            .output(PortSpec::new("p", Type::Map, "the pseudo p of the permutations"))
            .output(PortSpec::new("clusters", Type::Map, "the quadrant where p < alpha, 0 elsewhere"))
            .param(PortSpec::optional("permutations", Type::Integer, "per cell (default 99; 0 none)"))
            .param(PortSpec::optional("alpha", Type::Number, "the significance of the clusters (default 0.05)"))
            .param(PortSpec::optional("seed", Type::Integer, "the seed of the permutations (default 1)")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let (ny, nx) = m.shape();
        let l = lattice(&m.data, ny, nx, queen_of(args)?);
        let y: Vec<f64> = l.cells.iter().map(|&i| m.data[i]).collect();
        if y.len() < 4 {
            return Err(fail(args, "four cells with a value at least"));
        }
        let lm = local_moran(&y, &l);
        let perms: usize = args.param("permutations", 99)?;
        let alpha: f64 = args.param("alpha", 0.05)?;
        let n = y.len() as f64;
        let mean = numpy_sum(&y) / n;
        let z: Vec<f64> = y.iter().map(|v| v - mean).collect();
        let den = z.iter().map(|v| v * v).sum::<f64>();
        let stat =
            |i: usize, nb: &[usize]| (n - 1.0) * z[i] * (nb.iter().map(|&j| z[j]).sum::<f64>() / nb.len() as f64) / den;
        let p = conditional_p(y.len(), &l, perms, args.param("seed", 1)?, &stat);
        let clusters: Vec<f64> = p.iter().zip(&lm.quadrant).map(|(p, q)| if *p < alpha { *q } else { 0.0 }).collect();
        Ok(outputs([
            ("i", on_map(m, &l, &lm.i)),
            ("quadrant", on_map(m, &l, &lm.quadrant)),
            ("z", on_map(m, &l, &lm.z)),
            ("p", on_map(m, &l, &p)),
            ("clusters", on_map(m, &l, &clusters)),
        ]))
    }
}

/// Getis and Ord's Gi* (the cell itself among its neighbours, row-standardised) and its z.
pub fn getis_ord(y: &[f64], l: &Lattice) -> (Vec<f64>, Vec<f64>) {
    let n = y.len() as f64;
    let total = numpy_sum(y);
    let mean = total / n;
    let squares = numpy_sum(&y.iter().map(|v| v * v).collect::<Vec<_>>()) / n;
    let var = squares - mean * mean;
    let (mut g, mut z) = (Vec::with_capacity(y.len()), Vec::with_capacity(y.len()));
    for (i, nb) in l.neighbours.iter().enumerate() {
        let w = 1.0 / (nb.len() + 1) as f64;
        let gi = (y[i] * w + nb.iter().map(|&j| y[j] * w).sum::<f64>()) / total;
        let card = 1.0;
        let ev = card / n;
        let mut vg = card * (n - card);
        vg /= n - 1.0;
        vg *= 1.0 / (n * n);
        vg *= var / (mean * mean);
        g.push(gi);
        z.push((gi - ev) / vg.sqrt());
    }
    (g, z)
}

struct GetisOrd;

impl Operator for GetisOrd {
    fn signature(&self) -> OpSignature {
        contiguity(
            OpSignature::new(
                "stats.getis_ord",
                "Getis and Ord's Gi* of every cell (PySAL's esda.G_Local with star and row-standardised weights, to \
                 rounding): the share of the map's total around the cell, itself included, its z and p, and the hot \
                 and cold spots at 90, 95 and 99% confidence (as ArcGIS's Gi_Bin: 1 to 3 hot, -1 to -3 cold).",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the values (positive)"))
            .output(PortSpec::new("g", Type::Map, "Gi*"))
            .output(PortSpec::new("z", Type::Map, "its z"))
            .output(PortSpec::new("p", Type::Map, "its one-sided p (of |z|)"))
            .output(PortSpec::new(
                "spots",
                Type::Map,
                "3, 2, 1 hot spots at 99, 95, 90%; -1 to -3 cold; 0 none",
            )),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let (ny, nx) = m.shape();
        let l = lattice(&m.data, ny, nx, queen_of(args)?);
        let y: Vec<f64> = l.cells.iter().map(|&i| m.data[i]).collect();
        if y.len() < 3 {
            return Err(fail(args, "three cells with a value at least"));
        }
        let (g, z) = getis_ord(&y, &l);
        let p: Vec<f64> = z.iter().map(|v| ndtr(-v.abs())).collect();
        let spots: Vec<f64> = z
            .iter()
            .zip(&p)
            .map(|(z, p)| {
                let level = match 2.0 * p {
                    q if q < 0.01 => 3.0,
                    q if q < 0.05 => 2.0,
                    q if q < 0.10 => 1.0,
                    _ => 0.0,
                };
                level * z.signum()
            })
            .collect();
        Ok(outputs([
            ("g", on_map(m, &l, &g)),
            ("z", on_map(m, &l, &z)),
            ("p", on_map(m, &l, &p)),
            ("spots", on_map(m, &l, &spots)),
        ]))
    }
}
