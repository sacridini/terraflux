//! A sensitivity analysis of a model, shared by `terraflux sensitivity` and the application: the design's rows (one
//! at a time, Morris, Sobol), the candidates they make (the settings of each run), the indices from the outputs and
//! the files written (`design.csv`, `indices.csv`, `s2.csv`).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as Json;
use terraflux_ops::sensitivity as sens;

use crate::batch::Param;
use crate::{Error, Result};

/// How the factors are varied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Method {
    /// One factor at a time from the middle of the ranges to each end.
    Oat,
    /// Morris's elementary effects.
    Morris,
    /// Sobol's variance-based indices of Saltelli's runs.
    Sobol,
}

impl Method {
    pub fn parse(s: &str) -> Result<Method> {
        match s.trim().to_lowercase().as_str() {
            "oat" => Ok(Method::Oat),
            "morris" => Ok(Method::Morris),
            "sobol" => Ok(Method::Sobol),
            m => Err(Error::Other(format!("unknown method {m:?} (oat, morris or sobol)"))),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Method::Oat => "oat",
            Method::Morris => "morris",
            Method::Sobol => "sobol",
        }
    }

    /// Morris's trajectories or Sobol's base points by default (OAT has none).
    pub fn default_n(self) -> usize {
        match self {
            Method::Oat => 1,
            Method::Morris => 10,
            Method::Sobol => 64,
        }
    }
}

/// The settings of a design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Design {
    /// Morris's trajectories or Sobol's base points (a power of 2).
    pub n: usize,
    /// Morris's levels of the grid.
    pub levels: usize,
    /// Sobol: the second order indices too.
    pub second_order: bool,
    /// The seed of the design and of the bootstraps.
    pub seed: u64,
}

impl Design {
    /// The defaults of a method: `n` by the method, 4 levels, no second order, seed 1.
    pub fn new(method: Method) -> Design {
        Design { n: method.default_n(), levels: 4, second_order: false, seed: 1 }
    }
}

/// The runs of a design of `k` factors.
pub fn runs(method: Method, k: usize, d: &Design) -> usize {
    match method {
        Method::Oat => 2 * k + 1,
        Method::Morris => d.n * (k + 1),
        Method::Sobol if d.second_order => d.n * (2 * k + 2),
        Method::Sobol => d.n * (k + 2),
    }
}

/// The design's rows (a value per factor, in the ranges `bounds`).
pub fn rows(method: Method, bounds: &[(f64, f64)], d: &Design) -> Result<Vec<Vec<f64>>> {
    match method {
        Method::Oat => Ok(sens::oat_sample(bounds)),
        Method::Morris => {
            if d.levels < 2 {
                return Err(Error::Other("Morris needs 2 levels at least".into()));
            }
            Ok(sens::morris_sample(bounds, d.n, d.levels, d.seed))
        }
        Method::Sobol => sens::saltelli_sample(bounds, d.n, d.second_order, d.seed).map_err(Error::Other),
    }
}

/// The runs of a design of `k` factors with `r` replicates at each point.
pub fn total_runs(method: Method, k: usize, d: &Design, r: usize) -> usize {
    runs(method, k, d) * r.max(1)
}

/// The model seeds of the replicates of a point: `first`, `first + 1`...
pub fn seeds(first: i64, r: usize) -> Vec<i64> {
    (0..r.max(1) as i64).map(|k| first + k).collect()
}

/// The outputs of a design's points, each averaged over its replicates.
#[derive(Debug, Clone, PartialEq)]
pub struct Replicated {
    /// The replicates at each point.
    pub r: usize,
    /// The mean output of each point over the replicates that ran (NaN when none did): what `analyze` takes.
    pub mean: Vec<f64>,
    /// The standard deviation among the replicates that ran (NaN with fewer than 2).
    pub sd: Vec<f64>,
    /// The replicates that ran at each point.
    pub ran: Vec<usize>,
    /// The replicates that gave no output.
    pub failed: usize,
    /// The share (0..1) of the variance of all the runs' outputs that is chance: the mean variance among the
    /// replicates of a point over it. None with one replicate, or no variance.
    pub chance: Option<f64>,
}

fn variance(v: &[f64]) -> Option<f64> {
    let n = v.len();
    (n >= 2).then(|| {
        let m = v.iter().sum::<f64>() / n as f64;
        v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1) as f64
    })
}

/// The outputs of all the runs (`r` consecutive ones per point, as `batch::calibration_tasks` orders them; none for
/// a run that failed) as the mean and spread of each point, and the share of the variance that is chance.
pub fn replicated(values: &[Option<f64>], r: usize) -> Replicated {
    let r = r.max(1);
    let (mut mean, mut sd, mut ran, mut within) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for point in values.chunks(r) {
        let v: Vec<f64> = point.iter().flatten().copied().filter(|x| x.is_finite()).collect();
        mean.push(if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 });
        let var = variance(&v);
        sd.push(var.map_or(f64::NAN, f64::sqrt));
        within.extend(var);
        ran.push(v.len());
    }
    let all: Vec<f64> = values.iter().flatten().copied().filter(|x| x.is_finite()).collect();
    let chance = if r < 2 || within.is_empty() {
        None
    } else {
        variance(&all)
            .filter(|t| *t > 0.0)
            .map(|t| (within.iter().sum::<f64>() / within.len() as f64 / t).clamp(0.0, 1.0))
    };
    let failed = values.len() - ran.iter().sum::<usize>();
    Replicated { r, mean, sd, ran, failed, chance }
}

/// The settings of each row: the factors' keys to their values (the whole numbers rounded for an integer).
pub fn candidates(params: &[Param], rows: &[Vec<f64>]) -> Vec<BTreeMap<String, Json>> {
    rows.iter()
        .map(|r| {
            params
                .iter()
                .zip(r)
                .map(|(p, v)| {
                    let v = if p.integer { serde_json::json!(v.round() as i64) } else { serde_json::json!(v) };
                    (p.key.clone(), v)
                })
                .collect()
        })
        .collect()
}

/// What an analysis gives each factor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Indices {
    pub method: Method,
    /// The columns after the factor's name.
    pub columns: Vec<String>,
    pub factors: Vec<String>,
    /// A row per factor, a value per column.
    pub rows: Vec<Vec<f64>>,
    /// OAT: the output at the base.
    pub base: Option<f64>,
    /// Sobol with the second order: a row and a column per factor.
    pub s2: Option<Vec<Vec<f64>>>,
    /// The replicates at each point of the design (the output is their mean).
    #[serde(default = "one")]
    pub replicates: usize,
    /// With replicates: the share (0..1) of the output's variance that comes from the seed, not the factors.
    #[serde(default)]
    pub chance: Option<f64>,
    /// Morris and Sobol: the ranking value again with a fraction of the design (none for OAT, or a design too small).
    #[serde(default)]
    pub convergence: Option<Convergence>,
}

/// Whether the ranking value (μ* or ST) has settled: the analysis again with a quarter, half and all of the design.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Convergence {
    /// The column ranked: `mu_star` or `st`.
    pub by: String,
    /// The trajectories or base points used at each step, the whole design last.
    pub n: Vec<usize>,
    /// A row per factor, a value per step.
    pub values: Vec<Vec<f64>>,
    /// Stable: with half the design every factor's value is within `TOLERANCE` of the largest value of all the
    /// factors (a share of it), and the factors that matter keep their order.
    pub stable: bool,
}

/// The share of the largest value that a factor's value may move from half the design to all of it.
pub const TOLERANCE: f64 = 0.05;
/// A factor matters when its value is above this share of the largest.
pub const MATTERS: f64 = 0.1;

fn one() -> usize {
    1
}

/// The sizes (trajectories or base points) to analyse: a quarter, half and all of `n`, those with `min` at least.
fn steps(n: usize, min: usize) -> Vec<usize> {
    let mut s: Vec<usize> = [n / 4, n / 2, n].into_iter().filter(|&m| m >= min).collect();
    s.dedup();
    s
}

/// The ranking value of every factor with the first `m` trajectories or base points of the design.
fn ranking_at(method: Method, rows: &[Vec<f64>], y: &[f64], d: &Design, m: usize) -> Option<Vec<f64>> {
    let k = rows.first()?.len();
    let len = m * match method {
        Method::Oat => return None,
        Method::Morris => k + 1,
        Method::Sobol if d.second_order => 2 * k + 2,
        Method::Sobol => k + 2,
    };
    if len > rows.len() || len > y.len() {
        return None;
    }
    if method == Method::Morris {
        sens::morris_analyze(&rows[..len], &y[..len], d.levels, 2, 0.95, d.seed).ok().map(|r| r.mu_star)
    } else {
        sens::sobol_analyze(&y[..len], k, d.second_order, 2, 0.95, d.seed).ok().map(|r| r.st)
    }
}

/// How the ranking value moved as the design grew, for Morris (at least 2 trajectories on the smallest step) and
/// Sobol (4 base points). `full` is the ranking value with the whole design.
fn convergence(method: Method, rows: &[Vec<f64>], y: &[f64], d: &Design, full: &[f64]) -> Option<Convergence> {
    let (min, by, k) = match method {
        Method::Oat => return None,
        Method::Morris => (2, "mu_star", rows.first()?.len()),
        Method::Sobol => (4, "st", rows.first()?.len()),
    };
    let n = d.n;
    let sizes = steps(n, min);
    // all the design is the last step; it needs a smaller one to compare with
    if sizes.len() < 2 || sizes.last() != Some(&n) || full.len() != k {
        return None;
    }
    let mut values = vec![Vec::new(); k];
    for &m in &sizes[..sizes.len() - 1] {
        let v = ranking_at(method, rows, y, d, m)?;
        for (f, x) in values.iter_mut().zip(v) {
            f.push(x);
        }
    }
    for (f, x) in values.iter_mut().zip(full) {
        f.push(*x);
    }
    let at = |s: usize| -> Vec<f64> { values.iter().map(|f| f[s]).collect() };
    let (last, half) = (at(sizes.len() - 1), at(sizes.len() - 2));
    let top = last.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let moved = last.iter().zip(&half).all(|(a, b)| (a - b).abs() <= TOLERANCE * top);
    let order = |v: &[f64]| -> Vec<usize> {
        let mut o: Vec<usize> = (0..k).filter(|&i| last[i] > MATTERS * top).collect();
        o.sort_by(|&a, &b| v[b].partial_cmp(&v[a]).unwrap_or(std::cmp::Ordering::Equal));
        o
    };
    let stable = top.is_finite() && top > 0.0 && moved && order(&last) == order(&half);
    Some(Convergence { by: by.into(), n: sizes, values, stable })
}

impl Indices {
    /// The replicates' outcome kept with the indices.
    pub fn with_replicates(mut self, rep: &Replicated) -> Indices {
        self.replicates = rep.r;
        self.chance = rep.chance;
        self
    }

    /// The column the factors are ranked by: the swing, μ*, the total order index.
    pub fn ranked_by(&self) -> &'static str {
        match self.method {
            Method::Oat => "swing",
            Method::Morris => "mu_star",
            Method::Sobol => "st",
        }
    }

    /// The factors' positions, the most important first.
    pub fn order(&self) -> Vec<usize> {
        let by = self.columns.iter().position(|c| c == self.ranked_by());
        let mut o: Vec<usize> = (0..self.factors.len()).collect();
        if let Some(by) = by {
            o.sort_by(|&a, &b| {
                let (x, y) = (self.rows[a][by], self.rows[b][by]);
                // (a NaN last)
                y.partial_cmp(&x).unwrap_or_else(|| x.is_nan().cmp(&y.is_nan()))
            });
        }
        o
    }
}

/// The indices from the outputs `y` of the design's `rows` (the factors' `keys`, their order).
pub fn analyze(
    method: Method,
    keys: &[String],
    rows: &[Vec<f64>],
    y: &[f64],
    d: &Design,
    resamples: usize,
) -> Result<Indices> {
    let cols = |c: &[&str]| c.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let k = keys.len();
    let mut out = Indices {
        method,
        columns: Vec::new(),
        factors: keys.to_vec(),
        rows: Vec::new(),
        base: None,
        s2: None,
        replicates: 1,
        chance: None,
        convergence: None,
    };
    match method {
        Method::Oat => {
            let o = sens::oat_analyze(y, k).map_err(Error::Other)?;
            out.columns = cols(&["low", "high", "swing"]);
            out.rows = (0..k).map(|i| vec![o.low[i], o.high[i], o.swing[i]]).collect();
            out.base = Some(o.base);
        }
        Method::Morris => {
            let m = sens::morris_analyze(rows, y, d.levels, resamples, 0.95, d.seed).map_err(Error::Other)?;
            out.columns = cols(&["mu", "mu_star", "sigma", "mu_star_conf"]);
            out.rows = (0..k).map(|i| vec![m.mu[i], m.mu_star[i], m.sigma[i], m.mu_star_conf[i]]).collect();
        }
        Method::Sobol => {
            let s = sens::sobol_analyze(y, k, d.second_order, resamples, 0.95, d.seed).map_err(Error::Other)?;
            out.columns = cols(&["s1", "s1_conf", "st", "st_conf"]);
            out.rows = (0..k).map(|i| vec![s.s1[i], s.s1_conf[i], s.st[i], s.st_conf[i]]).collect();
            if d.second_order {
                out.s2 = Some(s.s2);
            }
        }
    }
    if let Some(by) = out.columns.iter().position(|c| c == out.ranked_by()) {
        let full: Vec<f64> = out.rows.iter().map(|r| r[by]).collect();
        out.convergence = convergence(method, rows, y, d, &full);
    }
    Ok(out)
}

/// The design's points and their outputs: point, a column per factor, the output (with replicates: its mean, then
/// `sd` among the replicates and `n`, those that ran).
pub fn design_csv(keys: &[String], rows: &[Vec<f64>], y: &[f64], output: &str, rep: Option<&Replicated>) -> String {
    let rep = rep.filter(|p| p.r > 1);
    let extra = if rep.is_some() { ",sd,n" } else { "" };
    let mut csv = format!("run,{},{}{extra}\n", keys.join(","), output);
    for (k, (r, v)) in rows.iter().zip(y).enumerate() {
        let mut vals: Vec<String> = r.iter().chain(std::iter::once(v)).map(|x| format!("{x}")).collect();
        if let Some(p) = rep {
            vals.push(format!("{}", p.sd[k]));
            vals.push(format!("{}", p.ran[k]));
        }
        csv.push_str(&format!("{},{}\n", k + 1, vals.join(",")));
    }
    csv
}

/// The indices: factor, then the method's columns (OAT: the base output too, on each row).
pub fn indices_csv(i: &Indices) -> String {
    let mut csv = format!("factor,{}{}\n", i.columns.join(","), if i.base.is_some() { ",base" } else { "" });
    for (key, r) in i.factors.iter().zip(&i.rows) {
        let mut vals: Vec<String> = r.iter().map(|x| format!("{x}")).collect();
        vals.extend(i.base.map(|b| format!("{b}")));
        csv.push_str(&format!("{key},{}\n", vals.join(",")));
    }
    csv
}

/// Sobol's second order indices, when the analysis has them.
pub fn s2_csv(i: &Indices) -> Option<String> {
    let s2 = i.s2.as_ref()?;
    let mut csv = format!("factor,{}\n", i.factors.join(","));
    for (key, r) in i.factors.iter().zip(s2) {
        let v: Vec<String> = r.iter().map(|x| format!("{x}")).collect();
        csv.push_str(&format!("{key},{}\n", v.join(",")));
    }
    Some(csv)
}

/// How the ranking value moved with the design's size: factor, then the value at each size (`mu_star_n5`...).
pub fn convergence_csv(i: &Indices) -> Option<String> {
    let c = i.convergence.as_ref()?;
    let head: Vec<String> = c.n.iter().map(|n| format!("{}_n{n}", c.by)).collect();
    let mut csv = format!("factor,{}\n", head.join(","));
    for (key, r) in i.factors.iter().zip(&c.values) {
        let v: Vec<String> = r.iter().map(|x| format!("{x}")).collect();
        csv.push_str(&format!("{key},{}\n", v.join(",")));
    }
    Some(csv)
}

/// Write `design.csv` in `dir`.
pub fn write_design(
    dir: &std::path::Path,
    keys: &[String],
    rows: &[Vec<f64>],
    y: &[f64],
    output: &str,
    rep: Option<&Replicated>,
) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("design.csv"), design_csv(keys, rows, y, output, rep))?;
    Ok(())
}

/// Write `indices.csv` (and `s2.csv`, `convergence.csv`) in `dir`.
pub fn write_indices(dir: &std::path::Path, i: &Indices) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("indices.csv"), indices_csv(i))?;
    if let Some(s2) = s2_csv(i) {
        std::fs::write(dir.join("s2.csv"), s2)?;
    }
    if let Some(c) = convergence_csv(i) {
        std::fs::write(dir.join("convergence.csv"), c)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use super::*;

    fn keys() -> Vec<String> {
        vec!["a.x".into(), "b.y".into()]
    }

    #[test]
    fn the_runs_of_a_design_are_its_rows() {
        let bounds = [(0.0, 1.0), (1.0, 3.0), (-1.0, 1.0)];
        for (m, second) in
            [(Method::Oat, false), (Method::Morris, false), (Method::Sobol, false), (Method::Sobol, true)]
        {
            let d = Design { n: 4, second_order: second, ..Design::new(m) };
            assert_eq!(rows(m, &bounds, &d).unwrap().len(), runs(m, 3, &d), "{m:?} {second}");
        }
        let d = Design { n: 8, second_order: true, ..Design::new(Method::Sobol) };
        assert_eq!(runs(Method::Sobol, 3, &d), 64);
        assert_eq!(runs(Method::Oat, 3, &d), 7);
        assert_eq!(runs(Method::Morris, 3, &Design::new(Method::Morris)), 40);
        assert_eq!(Method::parse("Sobol").unwrap(), Method::Sobol);
        assert!(Method::parse("fast").is_err());
    }

    #[test]
    fn the_whole_numbers_are_rounded_in_the_candidates() {
        let params = [Param::parse("a.x=1:9", 2).unwrap(), Param::parse("b.y=0.5:2.5", 2).unwrap()];
        assert!(params[0].integer && !params[1].integer);
        let c = candidates(&params, &[vec![2.6, 1.25]]);
        assert_eq!(c[0]["a.x"], 3);
        assert_eq!(c[0]["b.y"], 1.25);
    }

    #[test]
    fn oat_indices_and_their_files() {
        let bounds = [(0.0, 4.0), (1.0, 2.0)];
        let d = Design::new(Method::Oat);
        let r = rows(Method::Oat, &bounds, &d).unwrap();
        let y: Vec<f64> = r.iter().map(|r| 3.0 * r[0] + 0.5 * r[1]).collect();
        let i = analyze(Method::Oat, &keys(), &r, &y, &d, 10).unwrap();
        assert_eq!(i.order(), [0, 1]);
        assert_eq!(i.rows[0], [0.0 * 3.0 + 0.75, 12.75, 12.0]);
        let csv = indices_csv(&i);
        assert!(csv.starts_with("factor,low,high,swing,base\na.x,0.75,12.75,12,"), "{csv}");
        let design = design_csv(&keys(), &r, &y, "m.out", None);
        assert!(design.starts_with("run,a.x,b.y,m.out\n1,2,1.5,6.75\n"), "{design}");
        assert_eq!(design.lines().count(), 6);
        assert!(s2_csv(&i).is_none());
        assert_eq!((i.replicates, i.chance), (1, None));
    }

    #[test]
    fn the_replicates_of_a_point_are_averaged() {
        assert_eq!(seeds(5, 3), [5, 6, 7]);
        assert_eq!(total_runs(Method::Oat, 2, &Design::new(Method::Oat), 3), 15);
        // 3 points of 2 replicates
        let v = [Some(1.0), Some(3.0), Some(10.0), Some(10.0), Some(4.0), Some(8.0)];
        let p = replicated(&v, 2);
        assert_eq!(p.mean, [2.0, 10.0, 6.0]);
        assert_eq!(p.ran, [2, 2, 2]);
        assert!((p.sd[0] - 2.0f64.sqrt()).abs() < 1e-12 && p.sd[1] == 0.0);
        assert_eq!(p.failed, 0);
        // one replicate: the outputs themselves, no chance
        let one = replicated(&[Some(1.0), Some(2.0)], 1);
        assert_eq!((one.mean.clone(), one.chance), (vec![1.0, 2.0], None));
        let csv = design_csv(&keys(), &vec![vec![0.0, 0.0]; 3], &p.mean, "o", Some(&p));
        assert!(csv.starts_with("run,a.x,b.y,o,sd,n\n1,0,0,2,1.41421"), "{csv}");
        assert!(csv.lines().last().unwrap().starts_with("3,0,0,6,2.8284"), "{csv}");
        assert!(!design_csv(&keys(), &[vec![0.0, 0.0]], &one.mean[..1], "o", Some(&one)).contains("sd"));
    }

    #[test]
    fn the_chance_is_the_share_of_the_variance_among_the_replicates() {
        // within-point variances 2, 0 and 8 (mean 10/3) over the variance of all six outputs
        let v = [1.0, 3.0, 10.0, 10.0, 4.0, 8.0].map(Some);
        let c = replicated(&v, 2).chance.unwrap();
        let all = [1.0, 3.0, 10.0, 10.0, 4.0, 8.0];
        let m = all.iter().sum::<f64>() / 6.0;
        let total = all.iter().map(|x| (x - m).powi(2)).sum::<f64>() / 5.0;
        assert!((c - (10.0 / 3.0) / total).abs() < 1e-12, "{c}");
        // identical replicates: no chance at all
        let none = replicated(&[1.0, 1.0, 5.0, 5.0].map(Some), 2);
        assert_eq!(none.chance, Some(0.0));
        // no variance anywhere
        assert_eq!(replicated(&[2.0; 4].map(Some), 2).chance, None);
    }

    #[test]
    fn a_failed_replicate_is_left_out_of_the_mean() {
        let v = [Some(1.0), None, None, None, Some(4.0), Some(6.0)];
        let p = replicated(&v, 2);
        assert_eq!(p.mean[0], 1.0);
        assert!(p.mean[1].is_nan() && p.sd[0].is_nan() && p.sd[1].is_nan());
        assert_eq!(p.mean[2], 5.0);
        assert_eq!((p.ran.clone(), p.failed), (vec![1, 0, 2], 3));
        // only the point with two replicates tells the chance
        assert!(p.chance.is_some());
        let old: Indices =
            serde_json::from_str(r#"{"method":"oat","columns":[],"factors":[],"rows":[],"base":null,"s2":null}"#)
                .unwrap();
        assert_eq!((old.replicates, old.chance), (1, None));
    }

    fn ishigami(r: &[f64]) -> f64 {
        r[0].sin() + 7.0 * r[1].sin().powi(2) + 0.1 * r[2].powi(4) * r[0].sin()
    }

    fn three() -> Vec<String> {
        vec!["a.x".into(), "b.y".into(), "c.z".into()]
    }

    #[test]
    fn a_linear_morris_design_is_stable_with_few_trajectories() {
        let bounds = [(0.0, 1.0), (0.0, 1.0), (0.0, 1.0)];
        let f = |r: &[f64]| 4.0 * r[0] + 2.0 * r[1] + 0.05 * r[2];
        // 3 trajectories: a half of 1 is too little, so only 2 steps would remain: none here
        for (n, steps) in [(10, vec![2, 5, 10]), (4, vec![2, 4]), (3, Vec::new())] {
            let d = Design { n, ..Design::new(Method::Morris) };
            let r = rows(Method::Morris, &bounds, &d).unwrap();
            let y: Vec<f64> = r.iter().map(|r| f(r)).collect();
            let i = analyze(Method::Morris, &three(), &r, &y, &d, 10).unwrap();
            match &i.convergence {
                Some(c) => {
                    assert_eq!((c.n.clone(), c.by.as_str()), (steps, "mu_star"), "n {n}");
                    assert!(c.stable, "{c:?}");
                    // the effect of a linear function is the same at every size
                    assert!((c.values[0][0] - 4.0).abs() < 1e-9 && (c.values[0].last().unwrap() - 4.0).abs() < 1e-9);
                    let csv = convergence_csv(&i).unwrap();
                    assert!(csv.starts_with(&format!("factor,mu_star_n{}", c.n[0])), "{csv}");
                    assert_eq!(csv.lines().count(), 4);
                }
                None => assert!(steps.is_empty(), "n {n}"),
            }
        }
    }

    #[test]
    fn a_noisy_morris_design_is_not_stable() {
        let bounds = [(0.0, 1.0), (0.0, 1.0)];
        let d = Design { n: 8, ..Design::new(Method::Morris) };
        let r = rows(Method::Morris, &bounds, &d).unwrap();
        // an output that is all interaction and wiggle: the effects change with the trajectory
        let y: Vec<f64> = r.iter().map(|r| (25.0 * r[0] * r[1]).sin() + (17.0 * r[0]).cos()).collect();
        let i = analyze(Method::Morris, &keys(), &r, &y, &d, 10).unwrap();
        let c = i.convergence.unwrap();
        assert_eq!(c.n, [2, 4, 8]);
        assert!(!c.stable, "{c:?}");
    }

    #[test]
    fn sobol_converges_on_the_ishigami_function_with_many_base_points() {
        let bounds = [(-PI, PI); 3];
        let d = Design { n: 2048, ..Design::new(Method::Sobol) };
        let r = rows(Method::Sobol, &bounds, &d).unwrap();
        let y: Vec<f64> = r.iter().map(|r| ishigami(r)).collect();
        let i = analyze(Method::Sobol, &three(), &r, &y, &d, 10).unwrap();
        let c = i.convergence.as_ref().unwrap();
        assert_eq!((c.n.clone(), c.by.as_str()), (vec![512, 1024, 2048], "st"));
        assert!(c.stable, "{c:?}");
        // the known total order indices: 0.56, 0.44, 0.24
        for (got, want) in c.values.iter().map(|v| v[2]).zip([0.56, 0.44, 0.24]) {
            assert!((got - want).abs() < 0.05, "{got} {want}");
        }
        let dir = std::env::temp_dir().join(format!("terraflux-conv-{}", std::process::id()));
        write_indices(&dir, &i).unwrap();
        assert!(dir.join("convergence.csv").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_small_sobol_design_has_the_structure_but_not_the_verdict() {
        let bounds = [(-PI, PI); 3];
        let d = Design { n: 8, ..Design::new(Method::Sobol) };
        let r = rows(Method::Sobol, &bounds, &d).unwrap();
        let y: Vec<f64> = r.iter().map(|r| ishigami(r)).collect();
        let i = analyze(Method::Sobol, &three(), &r, &y, &d, 10).unwrap();
        let c = i.convergence.as_ref().unwrap();
        // a quarter (2 base points) is below the least of 4
        assert_eq!(c.n, [4, 8]);
        assert_eq!((c.values.len(), c.values[0].len()), (3, 2));
        assert!(!c.stable, "{c:?}");
        // 4 base points have no half to compare with, and OAT has none
        let d4 = Design { n: 4, ..d.clone() };
        let r4 = rows(Method::Sobol, &bounds, &d4).unwrap();
        let y4: Vec<f64> = r4.iter().map(|r| ishigami(r)).collect();
        assert!(analyze(Method::Sobol, &three(), &r4, &y4, &d4, 10).unwrap().convergence.is_none());
        let o = Design::new(Method::Oat);
        let ro = rows(Method::Oat, &bounds, &o).unwrap();
        let yo: Vec<f64> = ro.iter().map(|r| r[0]).collect();
        assert!(analyze(Method::Oat, &three(), &ro, &yo, &o, 10).unwrap().convergence.is_none());
        // a record from before the check still opens
        let old = r#"{"method":"oat","columns":[],"factors":[],"rows":[],"base":null,"s2":null}"#;
        assert!(serde_json::from_str::<Indices>(old).unwrap().convergence.is_none());
    }

    #[test]
    fn sobol_with_the_second_order_gives_a_matrix() {
        let bounds = [(0.0, 1.0), (0.0, 1.0)];
        let d = Design { n: 16, second_order: true, ..Design::new(Method::Sobol) };
        let r = rows(Method::Sobol, &bounds, &d).unwrap();
        let y: Vec<f64> = r.iter().map(|r| r[0] + 0.1 * r[1]).collect();
        let i = analyze(Method::Sobol, &keys(), &r, &y, &d, 10).unwrap();
        assert_eq!(i.columns, ["s1", "s1_conf", "st", "st_conf"]);
        assert_eq!(i.order()[0], 0, "the first factor matters more: {i:?}");
        let s2 = s2_csv(&i).unwrap();
        assert!(s2.starts_with("factor,a.x,b.y\na.x,"), "{s2}");
        let dir = std::env::temp_dir().join(format!("terraflux-sens-{}", std::process::id()));
        write_indices(&dir, &i).unwrap();
        assert!(dir.join("indices.csv").exists() && dir.join("s2.csv").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
