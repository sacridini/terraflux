//! Step selection functions (amt's fit_issf and log_rss): conditional logistic regression as survival's `clogit`
//! fits it (the exact partial likelihood, Therneau's `coxexact`), through the library's C++ in R's arithmetic, so
//! the coefficients, their covariance, the log-likelihoods, the iterations, the tests and the log-RSS are R's bits.
//! The formula is R's, in a string ([`super::formula`]).

use std::collections::HashSet;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::vector::{Column, Vector};

use super::formula::{
    Col, Frame, Kind, Node, Spec, Terms, columns_of, deparse, is_syntactic, nanjson, parse_formula, set_kind,
    strata_codes, strata_raw,
};
use super::ssf::{Distribution, distribution_of};
use super::{codes, fail, track_column, tracks_of};
use crate::{Args, Ctx, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(FitIssf));
    r.add(Arc::new(LogRss));
}

type Res<T> = std::result::Result<T, String>;

fn ce(e: cxx::Exception) -> String {
    e.what().to_string()
}

const EPS: f64 = 1e-9;
const ITER_MAX: i32 = 20;

/// The rows a fit used and their strata (what the bootstrap of the log-RSS draws from).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitData {
    pub frame: Frame,
    pub strata: Col,
}

/// A fitted step selection function (the object of `survival::clogit` that amt's `fit_issf` returns).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepModel {
    pub formula: String,
    /// the formula's terms: its variables with their levels, and the names of the model matrix's columns
    pub terms: Terms,
    /// the coefficients by the names of the model matrix's columns (NaN for a column found collinear)
    #[serde(with = "nanjson::many")]
    pub coef: Vec<f64>,
    /// their covariance, p x p, row-major (zero rows and columns for a NaN coefficient)
    #[serde(with = "nanjson::many")]
    pub vcov: Vec<f64>,
    /// the values the columns were centred on (0 for a column of only -1, 0 and 1)
    #[serde(with = "nanjson::many")]
    pub means: Vec<f64>,
    /// the log-likelihood at zero and at the fit
    #[serde(with = "nanjson::many")]
    pub loglik: Vec<f64>,
    pub iter: i64,
    pub n: usize,
    pub nevent: usize,
    #[serde(with = "nanjson::one")]
    pub wald: f64,
    #[serde(with = "nanjson::one")]
    pub score: f64,
    pub converged: bool,
    pub n_omitted: usize,
    /// the distributions the random steps were drawn from, which a simulation draws its candidate steps from
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sl_distr: Option<Distribution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ta_distr: Option<Distribution>,
    /// the rows used, for the bootstrap
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<FitData>,
}

/// The standard errors, z and p of `summary(fit)$coefficients`.
pub struct Summary {
    pub exp_coef: Vec<f64>,
    pub se: Vec<f64>,
    pub z: Vec<f64>,
    pub p: Vec<f64>,
}

/// sqrt(diag(var)): R takes the square roots of a vector in extended precision, that of one number correctly rounded.
fn sqrt_diag(v: &[f64], p: usize) -> Res<Vec<f64>> {
    let d: Vec<f64> = (0..p).map(|i| v[i * p + i]).collect();
    ffi::move_issf_fn(if p == 1 { "sqrt1" } else { "sqrt" }, &d).map_err(ce)
}

impl StepModel {
    /// The names of the model matrix's columns.
    pub fn names(&self) -> &[String] {
        &self.terms.columns
    }

    /// The coefficients that are not NaN (`extractAIC`'s degrees of freedom).
    pub fn edf(&self) -> usize {
        self.coef.iter().filter(|c| !c.is_nan()).count()
    }

    /// `extractAIC(fit)[2]`: -2 * the log-likelihood + 2 * edf.
    pub fn aic(&self) -> f64 {
        -2.0 * self.loglik[1] + 2.0 * self.edf() as f64
    }

    /// `summary(fit)$coefficients`: exp(coef), se, z and the p-value `pchisq(z^2, 1, lower.tail = FALSE)`.
    pub fn summary(&self) -> Res<Summary> {
        let p = self.coef.len();
        let se = sqrt_diag(&self.vcov, p)?;
        let z: Vec<f64> = self.coef.iter().zip(&se).map(|(b, s)| b / s).collect();
        let zz: Vec<f64> = z.iter().map(|z| z * z).collect();
        let pv = ffi::move_issf_pchisq(&zz, 1.0, false);
        Ok(Summary { exp_coef: ffi::move_issf_fn("exp", &self.coef).map_err(ce)?, se, z, p: pv })
    }

    /// `confint(fit)`: the Wald intervals `coef +- qnorm(1 - (1 - level) / 2) * se` (lower, upper).
    pub fn confint(&self, level: f64) -> Res<(Vec<f64>, Vec<f64>)> {
        if !(level > 0.0 && level < 1.0) {
            return Err("level: between 0 and 1".into());
        }
        let a = (1.0 - level) / 2.0;
        let fac = [ffi::move_issf_qnorm(a), ffi::move_issf_qnorm(1.0 - a)];
        let se = sqrt_diag(&self.vcov, self.coef.len())?;
        let lo = self.coef.iter().zip(&se).map(|(b, s)| b + s * fac[0]).collect();
        let hi = self.coef.iter().zip(&se).map(|(b, s)| b + s * fac[1]).collect();
        Ok((lo, hi))
    }
}

/// Where the strata of a fit come from.
pub enum Strata<'a> {
    /// `strata(...)` in the formula
    Formula,
    /// a column of the data
    Column(&'a str),
    /// one value per row
    Values(Col),
}

/// Conditional logistic regression of `data` by `sp` (the library's `_fit`): the model and the warnings.
pub fn fit(data: &Frame, sp: &Spec, strata: &Strata, keep_data: bool) -> Res<(StepModel, Vec<String>)> {
    let n = data.len();
    if n == 0 {
        return Err("data: no rows".into());
    }
    let mut sp = sp.clone();
    for v in &mut sp.variables {
        v.kind = Kind::Num;
        v.inner = None;
        v.levels.clear();
        v.labels.clear();
    }
    for i in 0..sp.variables.len() {
        if Some(i) != sp.strata {
            let mut v = sp.variables[i].clone();
            set_kind(&mut v, data, n)?;
            sp.variables[i] = v;
        }
    }
    let mut terms = Terms { spec: sp, columns: vec![] };
    let (xcols, names) = terms.design(data, n)?;
    if xcols.is_empty() {
        return Err("the formula has no covariates".into());
    }
    let sp = terms.spec.clone();
    // the cases: True / False or 0 / 1
    let cases_err = || format!("{}: the cases must be True / False or 0 / 1", sp.response);
    let status: Vec<f64> = match super::formula::column(data, &sp.response)? {
        Col::Bool(v) => v.iter().map(|b| b.map_or(f64::NAN, |b| f64::from(u8::from(b)))).collect(),
        Col::Num(v) => {
            if v.iter().any(|x| !x.is_nan() && *x != 0.0 && *x != 1.0) {
                return Err(cases_err());
            }
            v.clone()
        }
        Col::Text(_) => return Err(cases_err()),
    };
    // the strata
    let (sraw, sname) = match strata {
        Strata::Formula => {
            let s = sp
                .strata
                .ok_or("No strata is provided, please make sure the formula includes a strata: strata(step_id_)")?;
            let Node::Call(_, args) = &sp.variables[s].ast else { unreachable!() };
            (strata_raw(&args[0], data, n)?, deparse(&args[0]))
        }
        Strata::Column(c) => (super::formula::column(data, c)?.clone(), c.to_string()),
        Strata::Values(c) => {
            if c.len() != n {
                return Err("strata: one value per row".into());
            }
            (c.clone(), "strata".to_string())
        }
    };
    let scodes = strata_codes(&sraw, &format!("strata({sname})"))?;
    let keep: Vec<usize> =
        (0..n).filter(|&i| !(xcols.iter().any(|c| c[i].is_nan()) || status[i].is_nan() || scodes[i] < 0)).collect();
    if keep.is_empty() {
        return Err("no row without a missing value".into());
    }
    let p = names.len();
    let xk: Vec<f64> = xcols.iter().flat_map(|c| keep.iter().map(|&i| c[i])).collect();
    if xk.iter().any(|x| !x.is_finite()) {
        return Err("data contains an infinite predictor (log(0), a division by zero...)".into());
    }
    let sk: Vec<u8> = keep.iter().map(|&i| status[i] as u8).collect();
    let nevent: usize = sk.iter().map(|&s| s as usize).sum();
    if nevent == 0 {
        return Err("no case among the rows used".into());
    }
    let stk: Vec<i32> = keep.iter().map(|&i| scodes[i] as i32).collect();
    let nk = keep.len();
    let toler_chol = 2f64.powi(-39); // .Machine$double.eps^.75
    let r = match ffi::move_issf_fit(&xk, nk as i64, p as i64, &sk, &stk, ITER_MAX, EPS, toler_chol, true) {
        Ok(r) => r,
        Err(e) => {
            let col = |j: usize| &xk[j * nk..(j + 1) * nk];
            let cons: Vec<String> = (0..p)
                .filter(|&j| {
                    let c = col(j);
                    let (lo, hi) = (
                        c.iter().copied().fold(f64::INFINITY, f64::min),
                        c.iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    );
                    lo == hi && ![-1.0, 0.0, 1.0].contains(&c[0])
                })
                .map(|j| format!("'{}'", names[j]))
                .collect();
            let shown: Vec<String> =
                if cons.is_empty() { names.iter().map(|n| format!("'{n}'")).collect() } else { cons };
            return Err(format!(
                "the column(s) {} have a constant value other than 0, 1 and -1 (R: {})",
                format_args!("[{}]", shown.join(", ")),
                e.what()
            ));
        }
    };
    let mut warnings = Vec::new();
    // the (r, c) of a p x p column-major matrix
    let at = |m: &[f64], r: usize, c: usize| m[c * p + r];
    if r.flag == 1000 {
        warnings.push("Ran out of iterations and did not converge".to_string());
    } else {
        let toler_inf = EPS.sqrt();
        let mut bad = Vec::new();
        for j in 0..p {
            let mut acc = 0.0f64;
            for l in 0..p {
                acc += r.u[l] * at(&r.var_scaled, l, j);
            }
            let inf = acc.abs();
            if inf > EPS && inf > toler_inf * r.coef_scaled[j].abs() {
                bad.push((j + 1).to_string());
            }
        }
        if !bad.is_empty() {
            warnings.push(format!("Loglik converged before variable {}; beta may be infinite.", bad.join(",")));
        }
    }
    let nab: Vec<usize> = (0..p).filter(|&j| !r.coef[j].is_nan()).collect();
    let wald = if nab.is_empty() {
        f64::NAN
    } else {
        let k = nab.len();
        // the submatrix of the covariance, column-major (R's way of passing it)
        let mut sub = Vec::with_capacity(k * k);
        for &c in &nab {
            for &rr in &nab {
                sub.push(at(&r.var, rr, c));
            }
        }
        let b: Vec<f64> = nab.iter().map(|&j| r.coef[j]).collect();
        ffi::move_issf_wald(&sub, &b).map_err(ce)?
    };
    let mut vcov = vec![0.0; p * p];
    for rr in 0..p {
        for c in 0..p {
            vcov[rr * p + c] = at(&r.var, rr, c);
        }
    }
    terms.columns = names;
    let data_kept = keep_data.then(|| {
        let mut used: HashSet<String> = sp.variables.iter().flat_map(|v| columns_of(&v.ast)).collect();
        used.insert(sp.response.clone());
        let mut frame = Frame::default();
        for (name, col) in data.names.iter().zip(&data.cols) {
            if used.contains(name) {
                frame.push(name, col.select(&keep));
            }
        }
        FitData { frame, strata: sraw.select(&keep) }
    });
    let model = StepModel {
        formula: sp.text.clone(),
        terms,
        coef: r.coef.clone(),
        vcov,
        means: r.means.clone(),
        loglik: vec![r.loglik0, r.loglik1],
        iter: r.iter as i64,
        n: nk,
        nevent,
        wald,
        score: r.score,
        converged: r.flag != 1000,
        n_omitted: n - nk,
        sl_distr: None,
        ta_distr: None,
        data: data_kept,
    };
    Ok((model, warnings))
}

// -------------------------------------------------------------------------------------------------- log-RSS

/// predict(fit, x, type = "lp", reference = "sample") plus `uncenter`: the centred row times the coefficients, in R's
/// order, and then the sum of coef * means.
fn lp(m: &StepModel, x: &Frame, what: &str) -> Res<Vec<f64>> {
    let n = x.len();
    let (cols, _) = m.terms.design(x, n).map_err(|e| format!("{what}: {e}"))?;
    let c0: Vec<f64> = m.coef.iter().map(|&c| if c.is_nan() { 0.0 } else { c }).collect();
    let mut acc = vec![0.0f64; n];
    for (j, col) in cols.iter().enumerate() {
        for i in 0..n {
            acc[i] += (col[i] - m.means[j]) * c0[j];
        }
    }
    let prod: Vec<f64> = m.coef.iter().zip(&m.means).map(|(c, m)| c * m).filter(|v| !v.is_nan()).collect();
    let unc = ffi::move_issf_sum(&prod);
    Ok(acc.into_iter().map(|a| (a + 0.0) + unc).collect())
}

fn log_rss_point(m: &StepModel, x1: &Frame, x2: &Frame) -> Res<Vec<f64>> {
    let b = lp(m, x2, "x2")?[0];
    Ok(lp(m, x1, "x1")?.into_iter().map(|a| a - b).collect())
}

/// How the log-RSS gets its interval.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Ci {
    None,
    Se,
    Boot,
}

/// The log-RSS and its interval `(lwr, upr)` (amt's log_rss): the values, the interval and the warnings.
pub type LogRssOut = (Vec<f64>, Option<(Vec<f64>, Vec<f64>)>, Vec<String>);

/// amt's `log_rss` of the covariates `x1` against `x2` (one row).
pub fn log_rss(m: &StepModel, x1: &Frame, x2: &Frame, ci: Ci, level: f64, n_boot: usize, seed: i32) -> Res<LogRssOut> {
    if x2.len() != 1 {
        return Err(format!("The data.frame provided as 'x2' should have exactly 1 row (it has {}).", x2.len()));
    }
    if ci != Ci::None && !(0.0..=1.0).contains(&level) {
        return Err("ci_level: a number between 0 and 1".into());
    }
    if ci == Ci::Boot && n_boot < 1 {
        return Err("n_boot: a whole number, 1 or more".into());
    }
    if ci == Ci::Boot && m.data.is_none() {
        return Err("Please refit your step selection model with model=True: the bootstrap needs the data.".into());
    }
    let lrss = log_rss_point(m, x1, x2)?;
    match ci {
        Ci::None => Ok((lrss, None, vec![])),
        Ci::Se => {
            let (m1, _) = m.terms.design(x1, x1.len()).map_err(|e| format!("x1: {e}"))?;
            let (m2, _) = m.terms.design(x2, 1).map_err(|e| format!("x2: {e}"))?;
            let (n, p) = (x1.len(), m1.len());
            let delta: Vec<Vec<f64>> = (0..p).map(|j| m1[j].iter().map(|v| v - m2[j][0]).collect()).collect();
            // dv = delta v, summed over l in order; var = dv . delta, summed over j in order
            let mut dv = vec![vec![0.0f64; n]; p];
            for (l, dl) in delta.iter().enumerate() {
                for (j, dvj) in dv.iter_mut().enumerate() {
                    for (x, d) in dvj.iter_mut().zip(dl) {
                        *x += d * m.vcov[l * p + j];
                    }
                }
            }
            let mut var = vec![0.0f64; n];
            for (dvj, dj) in dv.iter().zip(&delta) {
                for ((x, a), b) in var.iter_mut().zip(dvj).zip(dj) {
                    *x += a * b;
                }
            }
            let se = ffi::move_issf_fn(if n == 1 { "sqrt1" } else { "sqrt" }, &var).map_err(ce)?;
            let z = ffi::move_issf_qnorm(1.0 - ((1.0 - level) / 2.0));
            let lwr = lrss.iter().zip(&se).map(|(l, s)| l - z * s).collect();
            let upr = lrss.iter().zip(&se).map(|(l, s)| l + z * s).collect();
            Ok((lrss, Some((lwr, upr)), vec![]))
        }
        Ci::Boot => {
            let (lwr, upr, w) = boot(m, x1, x2, &lrss, level, n_boot, seed)?;
            Ok((lrss, Some((lwr, upr)), w))
        }
    }
}

/// The bootstrap of amt's log_rss (with the library's three corrections): each sample draws, with replacement, as
/// many strata as the data has cases, refits the model and takes its log-RSS; the interval is
/// `mle + (q - mean(boot))`.
fn boot(
    m: &StepModel,
    x1: &Frame,
    x2: &Frame,
    mle: &[f64],
    level: f64,
    n_boot: usize,
    seed: i32,
) -> Res<(Vec<f64>, Vec<f64>, Vec<String>)> {
    let data = m
        .data
        .as_ref()
        .ok_or("Please refit your step selection model with model=True: the bootstrap needs the data.")?;
    let key = |i: usize| -> String {
        match &data.strata {
            Col::Num(v) => format!("n{}", if v[i] == 0.0 { 0.0 } else { v[i] }.to_bits()),
            Col::Bool(v) => format!("b{:?}", v[i]),
            Col::Text(v) => format!("t{:?}", v[i]),
        }
    };
    let total = data.strata.len();
    let mut seen = std::collections::HashMap::new();
    let mut rows: Vec<Vec<usize>> = Vec::new();
    for i in 0..total {
        let next = rows.len();
        let id = *seen.entry(key(i)).or_insert(next);
        if id == rows.len() {
            rows.push(vec![]);
        }
        rows[id].push(i);
    }
    let n_ids = rows.len() as i64;
    let case = super::formula::column(&data.frame, &m.terms.spec.response)?;
    let n_obs: i64 = match case {
        Col::Num(v) => v.iter().sum::<f64>() as i64,
        Col::Bool(v) => v.iter().filter(|b| **b == Some(true)).count() as i64,
        Col::Text(_) => 0,
    };
    let mut rng = terraflux_ls_sys::RRng::new(seed);
    let mut warnings = Vec::new();
    let n1 = x1.len();
    // the log-RSS of each sample: arr[b][i] is that of the row i of x1 in the sample b
    let mut arr: Vec<Vec<f64>> = Vec::with_capacity(n_boot);
    for _ in 0..n_boot {
        let mut attempts = 0;
        loop {
            let d = ffi::rrng_sample_int(rng.pin_mut(), n_ids, n_obs, true, &[]).map_err(ce)?;
            attempts += 1;
            let mut idx = Vec::new();
            let mut new_ids = Vec::new();
            for (k, &s) in d.iter().enumerate() {
                let r = &rows[s as usize - 1];
                idx.extend_from_slice(r);
                new_ids.extend(std::iter::repeat_n((k + 1) as f64, r.len()));
            }
            let sample = data.frame.select(&idx);
            let one = fit(&sample, &m.terms.spec, &Strata::Values(Col::Num(new_ids)), false)
                .and_then(|(refit, _)| log_rss_point(&refit, x1, x2));
            match one {
                Ok(v) => {
                    arr.push(v);
                    break;
                }
                Err(_) => {
                    if attempts + 1 == 25 {
                        warnings.push(
                            "Having trouble resampling a valid dataset: the bootstrap refits keep failing".to_string(),
                        );
                    }
                    if attempts + 1 == 50 {
                        return Err("Bootstrapping failed. The resampled datasets cannot be fitted.".into());
                    }
                }
            }
        }
    }
    let p_lwr = (1.0 - level) / 2.0;
    let probs = [p_lwr, 1.0 - p_lwr];
    let (mut lwr, mut upr) = (vec![0.0; n1], vec![0.0; n1]);
    for i in 0..n1 {
        let of_row: Vec<f64> = arr.iter().map(|s| s[i]).collect();
        let q = ffi::move_hr_quantile7(&of_row, &probs).map_err(ce)?;
        let mean = ffi::move_hr_mean(&of_row);
        lwr[i] = mle[i] + (q[0] - mean);
        upr[i] = mle[i] + (q[1] - mean);
    }
    Ok((lwr, upr, warnings))
}

// --------------------------------------------------------------------------------------------- the operators

/// A table of numbers and, for a column of names, its names (codes 1, 2... in the order they first appear).
fn named_columns(cols: &[(String, Col)], extra: Vec<(&str, Vec<f64>)>) -> Table {
    let n = cols.first().map_or_else(|| extra.first().map_or(0, |e| e.1.len()), |c| c.1.len());
    let mut t = Table::new(cols.iter().map(|c| c.0.clone()).chain(extra.iter().map(|e| e.0.to_string())).collect());
    let mut data: Vec<Vec<f64>> = Vec::new();
    for (name, col) in cols {
        data.push(match col {
            Col::Num(v) => v.clone(),
            Col::Bool(v) => {
                coded(name, v.iter().map(|b| b.map(|b| if b { "TRUE" } else { "FALSE" }.to_string())), &mut t)
            }
            Col::Text(v) => coded(name, v.iter().cloned(), &mut t),
        });
    }
    data.extend(extra.into_iter().map(|e| e.1));
    t.rows = (0..n).map(|i| data.iter().map(|c| c[i]).collect()).collect();
    t
}

fn coded(name: &str, v: impl Iterator<Item = Option<String>>, t: &mut Table) -> Vec<f64> {
    let mut names: Vec<String> = Vec::new();
    let out = v
        .map(|s| match s {
            None => f64::NAN,
            Some(s) => {
                let k = names.iter().position(|x| *x == s).unwrap_or_else(|| {
                    names.push(s);
                    names.len() - 1
                });
                (k + 1) as f64
            }
        })
        .collect();
    t.labels.insert(name.to_string(), names);
    out
}

/// The summary of a fit as a table: the term (its name), coef, exp_coef, se, z, p and the Wald interval.
pub fn summary_table(m: &StepModel) -> Res<Table> {
    let s = m.summary()?;
    let (lo, hi) = m.confint(0.95)?;
    let names: Vec<Option<String>> = m.names().iter().map(|n| Some(n.clone())).collect();
    let mut t = Table::new(vec!["term".into()]);
    let term = coded("term", names.into_iter(), &mut t);
    // coded() put the names in order of first appearance: the terms are unique, so one code each
    let cols = [
        ("coef", m.coef.clone()),
        ("exp_coef", s.exp_coef),
        ("se", s.se),
        ("z", s.z),
        ("p", s.p),
        ("lower", lo),
        ("upper", hi),
    ];
    t.columns.extend(cols.iter().map(|c| c.0.to_string()));
    t.rows = (0..term.len()).map(|i| std::iter::once(term[i]).chain(cols.iter().map(|c| c.1[i])).collect()).collect();
    Ok(t)
}

/// A one-row table of how the fit went.
pub fn fit_table(m: &StepModel) -> Table {
    let mut t = Table::new(
        ["loglik_null", "loglik", "aic", "n", "nevent", "iter", "converged", "n_omitted", "wald", "score"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    );
    t.rows.push(vec![
        m.loglik[0],
        m.loglik[1],
        m.aic(),
        m.n as f64,
        m.nevent as f64,
        m.iter as f64,
        f64::from(u8::from(m.converged)),
        m.n_omitted as f64,
        m.wald,
        m.score,
    ]);
    t
}

/// The name of a column as a term of a formula: as it is, or between backticks.
fn term_name(c: &str) -> String {
    if is_syntactic(c) { c.to_string() } else { format!("`{c}`") }
}

/// The columns extract_covariates added to the steps: numbers whose names do not end in `_` (amt's own columns do:
/// x1_, sl_, ta_, t1_, dt_, case_, step_id_, burst_...), other than the track and direction_p.
pub fn covariates(v: &Vector, track: &str) -> Vec<String> {
    v.attributes
        .iter()
        .filter(|(n, c)| matches!(c, Column::Numbers(_)) && n != track && n != "direction_p" && !n.ends_with('_'))
        .map(|(n, _)| n.clone())
        .collect()
}

/// `move.fit_issf`.
struct FitIssf;

impl Operator for FitIssf {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.fit_issf",
            "A step selection function fitted by conditional logistic regression (amt's fit_issf, fit_ssf and \
             fit_clogit are one function): survival's clogit with its exact partial likelihood (Therneau's \
             coxexact: one death time for each stratum, Gail et al.'s recursions when a stratum has several cases), \
             computed as survival 3.7-0 does in R 4.4.2 (the same iterations and sums, R's exp and log), so the \
             coefficients, their covariance, the log-likelihoods, the iterations, the tests and the log-RSS are R's \
             numbers. The formula is R's, in a text, for the terms step selection uses: columns, log, exp, sqrt, \
             sin, cos, log1p, abs and I(...) of them, factor(x), a:b and a*b, strata(x); - 1 and + 0 are ignored (the \
             fit has no intercept either way). A column of names is a factor with R's treatment contrasts (the first \
             in order is the reference). Without a formula: case_ ~ every covariate that move.extract_covariates \
             added (the numeric columns whose names do not end in _, besides the track) + log(sl_) + cos(ta_) + the \
             strata. Rows with a missing value in the formula are left out; the concordance coxph prints is not \
             computed.",
            Access::None,
        )
        .input(PortSpec::new(
            "steps",
            Type::Vector,
            "the random steps (move.random_steps) with the covariates (move.extract_covariates)",
        ))
        .input(PortSpec::optional(
            "sl_distr",
            Type::StepDistribution,
            "the distribution of step lengths the random steps were drawn from (kept in the model, for simulations)",
        ))
        .input(PortSpec::optional(
            "ta_distr",
            Type::StepDistribution,
            "the distribution of turn angles the random steps were drawn from (kept in the model)",
        ))
        .output(PortSpec::new("model", Type::StepModel, "the fitted step selection function"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "the coefficients: term, coef, exp_coef, se, z, p and the 95 % Wald interval (lower, upper)",
        ))
        .output(PortSpec::new(
            "fit",
            Type::Table,
            "one row: loglik_null, loglik, aic, n, nevent, iter, converged, n_omitted, wald, score",
        ))
        .param(PortSpec::optional("formula", Type::Text, "R's formula (see above; default: the one described)"))
        .param(PortSpec::optional(
            "strata",
            Type::Text,
            "track_step (default): one stratum for each step of each track (the step_id_ of random_steps restarts \
             for each track, and amt fits the ids it is given); or the name of a column. Ignored when the formula \
             has strata(...)",
        ))
        .param(PortSpec::optional("track", Type::Text, "the column of the track (default: the usual names)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "steps")?;
        let mut frame = Frame::from_vector(v);
        let e = |m: String| fail(node, m);
        if frame.get("case_").is_none() {
            return Err(fail(node, "steps: the random steps of move.random_steps (no column case_)"));
        }
        let given: Option<String> = args.opt_param::<String>("formula")?.filter(|f| !f.trim().is_empty());
        let by: String = args.param("strata", "track_step".to_string())?;
        let by = if by.trim().is_empty() { "track_step".to_string() } else { by };
        let in_formula = match &given {
            Some(f) => parse_formula(f).map_err(e)?.strata.is_some(),
            None => false,
        };
        // the stratum: the column the formula names, or the one made of the track and the step
        let stratum = if by == "track_step" { "stratum_".to_string() } else { by.clone() };
        let track = track_column(args, v);
        // made also when the formula names it: strata(stratum_) written by hand
        let names_it = given.as_ref().is_some_and(|f| f.contains("stratum_"));
        if by == "track_step" && (!in_formula || names_it) && frame.get("stratum_").is_none() {
            let k = track.clone()?;
            if frame.get("step_id_").is_none() {
                return Err(fail(node, "steps: the random steps of move.random_steps (no column step_id_)"));
            }
            let t = codes(v.column(&k).unwrap());
            let id = v.column("step_id_").unwrap().numbers();
            // pandas' groupby([track, step_id_], sort=False).ngroup() + 1: the groups in the order they appear
            let mut seen: std::collections::HashMap<(i64, u64), f64> = std::collections::HashMap::new();
            let out: Vec<f64> = (0..v.len())
                .map(|i| {
                    let n = seen.len() as f64 + 1.0;
                    *seen.entry((t[i], id[i].to_bits())).or_insert(n)
                })
                .collect();
            frame.push("stratum_", Col::Num(out));
        }
        let (formula, strata) = match &given {
            Some(f) => (f.clone(), if in_formula { Strata::Formula } else { Strata::Column(&stratum) }),
            None => {
                let mut terms: Vec<String> =
                    covariates(v, track.as_deref().unwrap_or("")).iter().map(|c| term_name(c)).collect();
                terms.push("log(sl_)".into());
                terms.push("cos(ta_)".into());
                terms.push(format!("strata({})", term_name(&stratum)));
                (format!("case_ ~ {}", terms.join(" + ")), Strata::Formula)
            }
        };
        let sp = parse_formula(&formula).map_err(e)?;
        let (mut model, warnings) = fit(&frame, &sp, &strata, true).map_err(e)?;
        for w in warnings {
            ctx.warn(format!("node {node:?}: {w}"));
        }
        model.sl_distr = distribution_of(args, "sl_distr")?.map(|d| (**d).clone());
        model.ta_distr = distribution_of(args, "ta_distr")?.map(|d| (**d).clone());
        let table = summary_table(&model).map_err(e)?;
        let summary = fit_table(&model);
        Ok(outputs([
            ("model", Value::StepModel(Arc::new(model))),
            ("table", Value::Table(table)),
            ("fit", Value::Table(summary)),
        ]))
    }
}

/// A frame from the JSON of a setting: an object of columns, each a list of values (or one value).
fn frame_of_json(v: &serde_json::Value, what: &str) -> Res<Frame> {
    use serde_json::Value as J;
    let obj = v.as_object().ok_or_else(|| format!("{what}: an object of columns, {{\"elevation\": [100, 120]}}"))?;
    let mut f = Frame::default();
    let mut len = None;
    for (name, val) in obj {
        let items: Vec<&J> = match val {
            J::Array(a) => a.iter().collect(),
            one => vec![one],
        };
        if len.is_some_and(|l| l != items.len()) {
            return Err(format!("{what}: the columns have different lengths"));
        }
        len = Some(items.len());
        let col = if items.iter().all(|x| x.is_number() || x.is_null()) {
            Col::Num(items.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect())
        } else if items.iter().all(|x| x.is_boolean() || x.is_null()) {
            Col::Bool(items.iter().map(|x| x.as_bool()).collect())
        } else if items.iter().all(|x| x.is_string() || x.is_null()) {
            Col::Text(items.iter().map(|x| x.as_str().map(str::to_string)).collect())
        } else {
            return Err(format!("{what}.{name}: numbers, texts or true / false, not a mix"));
        };
        f.push(name, col);
    }
    Ok(f)
}

/// amt's name of a covariate in the result of log_rss: `elevation_x1`, `sl_x1`.
fn appended(s: &str) -> String {
    if s.ends_with("_x1") {
        s.to_string()
    } else if s.ends_with('_') {
        format!("{s}x1")
    } else {
        format!("{s}_x1")
    }
}

/// `move.log_rss`.
struct LogRss;

impl Operator for LogRss {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.log_rss",
            "The log relative selection strength of the covariates x1 against x2 (amt's log_rss; Avgar et al. 2017): \
             how much more (or less) a step ending where the covariates are x1 is selected than one ending where \
             they are x2, log(w(x1) / w(x2)), from a model of move.fit_issf. As amt, it is predict(fit, x1, type = \
             \"lp\", reference = \"sample\") less that of x2, each plus sum(coef * means), so the values are R's \
             bits; a NaN coefficient counts as zero. Give x1 and x2 as tables or as settings ({\"elevation\": [50, \
             100], \"sl_\": [100, 20]} and {\"elevation\": 100, \"sl_\": 100}); every covariate of the formula must \
             be in both (a column of names takes a name of the fit's levels). The interval: se (Wald, from the \
             covariance of the coefficients) or boot (amt's bootstrap with the library's three corrections: each \
             sample draws, with replacement, as many strata as the data has cases, refits and takes the log-RSS; \
             the interval is the estimate plus the quantile less the mean of the samples; a refit that fails is \
             drawn again, up to 49 times).",
            Access::None,
        )
        .input(PortSpec::new("model", Type::StepModel, "the fitted step selection function (move.fit_issf)"))
        .input(PortSpec::optional("x1", Type::Table, "the covariates to compare, one row each"))
        .input(PortSpec::optional("x2", Type::Table, "the covariates of the reference, one row"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "the columns of x1 (named elevation_x1), log_rss and, with ci, lwr and upr",
        ))
        .param(PortSpec::optional(
            "x1",
            Type::Any,
            "the covariates to compare, columns to lists of values: {\"elevation\": [50, 100]}",
        ))
        .param(PortSpec::optional("x2", Type::Any, "the reference, columns to a value: {\"elevation\": 100}"))
        .param(
            PortSpec::optional("ci", Type::Text, "the interval: none (default), se or boot")
                .choices(["none", "se", "boot"]),
        )
        .param(PortSpec::optional("level", Type::Number, "the confidence level (0.95)"))
        .param(PortSpec::optional("n_boot", Type::Integer, "bootstrap samples (1000)"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of R's generator for the bootstrap (1)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let e = |m: String| fail(node, m);
        let m = match args.get("model") {
            Some(Value::StepModel(m)) => m.clone(),
            Some(v) => return Err(fail(node, format!("model: a step selection model, not a {}", v.ty().name()))),
            None => return Err(fail(node, "connect the model (move.fit_issf)")),
        };
        let frame = |port: &str| -> Result<Frame> {
            match args.get(port) {
                Some(Value::Table(t)) => Ok(Frame::from_table(t)),
                Some(v) => Err(fail(node, format!("{port}: a table, not a {}", v.ty().name()))),
                None => match args.params.get(port) {
                    Some(j) if !j.is_null() => frame_of_json(j, port).map_err(e),
                    _ => Err(fail(node, format!("give {port}: a table or the setting {port}"))),
                },
            }
        };
        let (x1, x2) = (frame("x1")?, frame("x2")?);
        let ci = match args.param("ci", "none".to_string())?.as_str() {
            "none" | "" => Ci::None,
            "se" => Ci::Se,
            "boot" => Ci::Boot,
            _ => return Err(fail(node, "ci: none, se or boot")),
        };
        let level: f64 = args.param("level", 0.95)?;
        let n_boot: i64 = args.param("n_boot", 1000)?;
        let seed: i32 = args.param("seed", 1)?;
        let (lrss, bounds, warnings) = log_rss(&m, &x1, &x2, ci, level, n_boot.max(0) as usize, seed).map_err(e)?;
        for w in warnings {
            ctx.warn(format!("node {node:?}: {w}"));
        }
        let cols: Vec<(String, Col)> = x1.names.iter().zip(&x1.cols).map(|(n, c)| (appended(n), c.clone())).collect();
        let mut extra = vec![("log_rss", lrss)];
        if let Some((lwr, upr)) = bounds {
            extra.push(("lwr", lwr));
            extra.push(("upr", upr));
        }
        Ok(outputs([("table", Value::Table(named_columns(&cols, extra)))]))
    }
}
