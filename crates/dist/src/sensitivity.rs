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
}

impl Indices {
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
    let mut out =
        Indices { method, columns: Vec::new(), factors: keys.to_vec(), rows: Vec::new(), base: None, s2: None };
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
    Ok(out)
}

/// The runs and their outputs: run, a column per factor, the output.
pub fn design_csv(keys: &[String], rows: &[Vec<f64>], y: &[f64], output: &str) -> String {
    let mut csv = format!("run,{},{}\n", keys.join(","), output);
    for (k, (r, v)) in rows.iter().zip(y).enumerate() {
        let vals: Vec<String> = r.iter().chain(std::iter::once(v)).map(|x| format!("{x}")).collect();
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

/// Write `design.csv` in `dir`.
pub fn write_design(dir: &std::path::Path, keys: &[String], rows: &[Vec<f64>], y: &[f64], output: &str) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("design.csv"), design_csv(keys, rows, y, output))?;
    Ok(())
}

/// Write `indices.csv` (and `s2.csv`) in `dir`.
pub fn write_indices(dir: &std::path::Path, i: &Indices) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    std::fs::write(dir.join("indices.csv"), indices_csv(i))?;
    if let Some(s2) = s2_csv(i) {
        std::fs::write(dir.join("s2.csv"), s2)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
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
        let design = design_csv(&keys(), &r, &y, "m.out");
        assert!(design.starts_with("run,a.x,b.y,m.out\n1,2,1.5,6.75\n"), "{design}");
        assert_eq!(design.lines().count(), 6);
        assert!(s2_csv(&i).is_none());
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
