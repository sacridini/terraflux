//! The evaluation of species distribution models (landschaft's evaluate, boyce, threshold and omission_rate,
//! after dismo's evaluate, ecospat's Boyce index and ENMeval's omission rates): AUC, the point-biserial
//! correlation, the confusion table at every threshold (kappa, TSS...), the thresholds that optimize them, the
//! continuous Boyce index and the omission rates at the minimum training presence and its 10th percentile. R's
//! rounding and quantiles, scipy's average ranks; the p-value of the correlation (Student's t) to rounding.

use terraflux_ls_sys::ffi;

use crate::change::numpy_sum;

use std::f64::consts::LOG10_2;

/// R's round(x, digits) (R 4's algorithm, landschaft's _r_round).
pub fn r_round(x: f64, digits: i32) -> f64 {
    if x.is_nan() || !x.is_finite() || x == 0.0 {
        return x;
    }
    if digits == 0 {
        return x.round_ties_even();
    }
    let dig = (digits as f64 + 0.5).floor();
    let sgn = if x < 0.0 { -1.0 } else { 1.0 };
    let x = x.abs();
    let (_, exp) = frexp(x);
    if LOG10_2 * (0.5 + (exp - 1) as f64) + dig > 15.0 {
        return sgn * x;
    }
    let p10 = 10f64.powi(dig as i32);
    let x10 = p10 * x;
    let i10 = x10.floor();
    let xd = i10 / p10;
    let xu = x10.ceil() / p10;
    let du = xu - x;
    let dd = x - xd;
    sgn * if du < dd || (i10 % 2.0 == 1.0 && du == dd) { xu } else { xd }
}

/// C's frexp: x = m 2^e with 0.5 <= |m| < 1.
fn frexp(x: f64) -> (f64, i32) {
    if x == 0.0 || !x.is_finite() {
        return (x, 0);
    }
    let bits = x.to_bits();
    let exp = ((bits >> 52) & 0x7ff) as i32;
    if exp == 0 {
        let (m, e) = frexp(x * 2f64.powi(54));
        return (m, e - 54);
    }
    let m = f64::from_bits((bits & !(0x7ffu64 << 52)) | (1022u64 << 52));
    (m, exp - 1022)
}

/// A positive integer ratio num / den rounded to 64 bits of mantissa, then to a double (landschaft's
/// float(_round_bits(Fraction(num, den)))).
fn round64_ratio(num: u128, den: u128) -> f64 {
    if num == 0 {
        return 0.0;
    }
    // e with 2^e <= num / den < 2^(e + 1)
    let mut e = (128 - num.leading_zeros()) as i32 - (128 - den.leading_zeros()) as i32;
    let ge = |e: i32| if e >= 0 { num >= den << e } else { num << (-e) >= den };
    if !ge(e) {
        e -= 1;
    }
    // m = num / den * 2^(63 - e), in [2^63, 2^64)
    let shift = 63 - e;
    let (n, d) = if shift >= 0 { (num << shift, den) } else { (num, den << (-shift)) };
    let (mut q, r) = (n / d, n % d);
    if 2 * r > d || (2 * r == d && q % 2 == 1) {
        q += 1;
    }
    (q as u64 as f64) * 2f64.powi(-shift)
}

/// The exact sum of two doubles rounded to 64 bits of mantissa, then to a double (float(_round_bits(
/// Fraction(a) + Fraction(b)))).
fn round64_sum(a: f64, b: f64) -> f64 {
    let parts = |x: f64| -> (i128, i32) {
        if x == 0.0 {
            return (0, 0);
        }
        let bits = x.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i32;
        let frac = (bits & ((1u64 << 52) - 1)) as i128;
        let (m, e) = if exp == 0 { (frac, -1074) } else { (frac | (1 << 52), exp - 1075) };
        (if x < 0.0 { -m } else { m }, e)
    };
    let ((ma, ea), (mb, eb)) = (parts(a), parts(b));
    if ma == 0 || mb == 0 || (ea - eb).abs() > 60 {
        return a + b;
    }
    let e = ea.min(eb);
    let n = (ma << (ea - e)) + (mb << (eb - e));
    if n == 0 {
        return 0.0;
    }
    let neg = n < 0;
    let mut m = n.unsigned_abs();
    let mut e = e;
    let bits = 128 - m.leading_zeros() as i32;
    if bits > 64 {
        let s = bits - 64;
        let (q, r) = (m >> s, m & ((1u128 << s) - 1));
        let half = 1u128 << (s - 1);
        m = if r > half || (r == half && q & 1 == 1) { q + 1 } else { q };
        e += s;
    }
    let v = (m as u64 as f64) * 2f64.powi(e);
    if neg { -v } else { v }
}

/// numpy's arange-based seq of landschaft's _folds._r_seq.
pub fn r_seq(start: f64, end: f64, by: f64) -> Vec<f64> {
    let delta = end - start;
    if delta == 0.0 && end == 0.0 {
        return vec![end];
    }
    let n = delta / by;
    let dd = delta.abs() / end.abs().max(start.abs());
    if dd < 100.0 * f64::EPSILON {
        return vec![start];
    }
    let n = (n + 1e-10) as i64;
    (0..=n)
        .map(|k| {
            let x = start + k as f64 * by;
            if by > 0.0 { x.min(end) } else { x.max(end) }
        })
        .collect()
}

/// R's quantile type 7 of the values at the probabilities.
fn quantile7(x: &[f64], probs: &[f64]) -> Vec<f64> {
    let mut x = x.to_vec();
    x.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = x.len();
    probs
        .iter()
        .map(|&p| {
            let index = 1.0 + (n.saturating_sub(1)) as f64 * p;
            let (lo, hi) = (index.floor() as usize, index.ceil() as usize);
            let mut q = x[lo - 1];
            if index > lo as f64 && x[hi - 1] != q {
                let h = index - lo as f64;
                q = (1.0 - h) * q + h * x[hi - 1];
            }
            q
        })
        .collect()
}

/// scipy's rankdata: average ranks, 1-based.
pub fn rankdata(x: &[f64]) -> Vec<f64> {
    let mut idx: Vec<usize> = (0..x.len()).collect();
    idx.sort_by(|&a, &b| x[a].partial_cmp(&x[b]).unwrap().then(a.cmp(&b)));
    let mut ranks = vec![0.0; x.len()];
    let mut i = 0;
    while i < idx.len() {
        let mut j = i;
        while j + 1 < idx.len() && x[idx[j + 1]] == x[idx[i]] {
            j += 1;
        }
        let r = 0.5 * ((j + 1) as f64 + i as f64 + 1.0);
        for k in &idx[i..=j] {
            ranks[*k] = r;
        }
        i = j + 1;
    }
    ranks
}

/// R's cor by method: pearson, spearman (on scipy's ranks) or kendall.
fn r_cor(x: &[f64], y: &[f64], method: &str) -> Result<f64, String> {
    let r = match method {
        "spearman" => ffi::sdm_r_cor2(&rankdata(x), &rankdata(y), false),
        "pearson" => ffi::sdm_r_cor2(x, y, false),
        "kendall" => ffi::sdm_r_cor2(x, y, true),
        m => return Err(format!("method must be pearson, spearman or kendall, got {m:?}")),
    };
    r.map_err(|e| e.to_string())
}

/// ln Gamma (Lanczos).
fn ln_gamma(x: f64) -> f64 {
    const G: [f64; 9] = [
        0.999_999_999_999_809_9,
        676.520_368_121_885_1,
        -1_259.139_216_722_402_8,
        771.323_428_777_653_1,
        -176.615_029_162_140_6,
        12.507_343_278_686_905,
        -0.138_571_095_265_720_12,
        9.984_369_578_019_572e-6,
        1.505_632_735_149_311_6e-7,
    ];
    if x < 0.5 {
        return (std::f64::consts::PI / (std::f64::consts::PI * x).sin()).ln() - ln_gamma(1.0 - x);
    }
    let x = x - 1.0;
    let mut a = G[0];
    let t = x + 7.5;
    for (i, g) in G.iter().enumerate().skip(1) {
        a += g / (x + i as f64);
    }
    0.5 * (2.0 * std::f64::consts::PI).ln() + (x + 0.5) * t.ln() - t + a.ln()
}

/// The regularized incomplete beta function I_x(a, b) (continued fraction).
fn inc_beta(a: f64, b: f64, x: f64) -> f64 {
    if x <= 0.0 {
        return 0.0;
    }
    if x >= 1.0 {
        return 1.0;
    }
    let front = (ln_gamma(a + b) - ln_gamma(a) - ln_gamma(b) + a * x.ln() + b * (1.0 - x).ln()).exp();
    if x > (a + 1.0) / (a + b + 2.0) {
        return 1.0 - inc_beta(b, a, 1.0 - x);
    }
    let tiny = 1e-300;
    let (mut c, mut d) = (1.0, 1.0 - (a + b) * x / (a + 1.0));
    if d.abs() < tiny {
        d = tiny;
    }
    d = 1.0 / d;
    let mut f = d;
    for m in 1..400 {
        let m = m as f64;
        let num = m * (b - m) * x / ((a + 2.0 * m - 1.0) * (a + 2.0 * m));
        d = 1.0 + num * d;
        d = if d.abs() < tiny { tiny } else { d };
        c = 1.0 + num / c;
        c = if c.abs() < tiny { tiny } else { c };
        d = 1.0 / d;
        f *= d * c;
        let num = -(a + m) * (a + b + m) * x / ((a + 2.0 * m) * (a + 2.0 * m + 1.0));
        d = 1.0 + num * d;
        d = if d.abs() < tiny { tiny } else { d };
        c = 1.0 + num / c;
        c = if c.abs() < tiny { tiny } else { c };
        d = 1.0 / d;
        let delta = d * c;
        f *= delta;
        if (delta - 1.0).abs() < 1e-16 {
            break;
        }
    }
    front * f / a
}

/// Student's t distribution function with df degrees of freedom.
fn student_cdf(t: f64, df: f64) -> f64 {
    if t.is_infinite() {
        return if t > 0.0 { 1.0 } else { 0.0 };
    }
    let lower = 0.5 * inc_beta(df / 2.0, 0.5, df / (df + t * t));
    if t < 0.0 { lower } else { 1.0 - lower }
}

/// The continuous Boyce index (ecospat's ecospat.boyce): its correlation (rounded to 3 digits), the
/// predicted-to-expected ratios of the classes, their middles and the classes used.
pub struct Boyce {
    pub cor: f64,
    pub f_ratio: Vec<f64>,
    pub hs: Vec<f64>,
    pub used: Vec<usize>,
}

/// The classes of Boyce's index: none (a moving window), a number, or breaks.
pub enum Classes<'a> {
    Window(Option<f64>, usize),
    Count(usize),
    Breaks(&'a [f64]),
}

fn values(v: &[f64]) -> Vec<f64> {
    v.iter().copied().filter(|x| !x.is_nan()).collect()
}

pub fn boyce(
    presences: &[f64],
    available: &[f64],
    classes: &Classes,
    method: &str,
    rm_duplicate: bool,
) -> Result<Boyce, String> {
    let obs = values(presences);
    let fit = values(available);
    if obs.is_empty() || fit.is_empty() {
        return Err("no prediction values".into());
    }
    let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let py_min = |a: f64, b: f64| if b < a { b } else { a };
    let py_max = |a: f64, b: f64| if b > a { b } else { a };
    let mini = py_min(min(&fit), min(&obs));
    let maxi = py_max(max(&fit), max(&obs));
    let (vec, upper): (Vec<f64>, Vec<f64>) = match classes {
        Classes::Breaks(b) => {
            let mut b = b.to_vec();
            b.sort_by(|x, y| x.partial_cmp(y).unwrap());
            let vec: Vec<f64> =
                std::iter::once(mini).chain(b.into_iter().filter(|x| *x >= mini && *x <= maxi)).collect();
            let upper = vec[1..].iter().copied().chain(std::iter::once(maxi)).collect();
            (vec, upper)
        }
        Classes::Window(window, res) => {
            let w = window.unwrap_or((max(&fit) - min(&fit)) / 10.0);
            let vec = r_seq(mini, maxi - w, (maxi - mini - w) / *res as f64);
            let upper = vec.iter().map(|v| v + w).collect();
            (vec, upper)
        }
        Classes::Count(n) => {
            let vec = r_seq(mini, maxi, (maxi - mini) / *n as f64);
            let upper = vec[1..].iter().copied().chain(std::iter::once(maxi)).collect();
            (vec, upper)
        }
    };
    let mut so = obs.clone();
    so.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut sf = fit.clone();
    sf.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let count = |s: &[f64], lo: f64, hi: f64| -> f64 {
        (s.partition_point(|&x| x <= hi) as i64 - s.partition_point(|&x| x < lo) as i64) as f64
    };
    let mut f = Vec::new();
    let mut starts = Vec::new();
    let mut hs = Vec::new();
    for k in 0..vec.len() {
        let pi = count(&so, vec[k], upper[k]) / obs.len() as f64;
        let ei = count(&sf, vec[k], upper[k]) / fit.len() as f64;
        let v = r_round(pi / ei, 10);
        if v.is_nan() {
            continue;
        }
        f.push(v);
        starts.push(vec[k]);
        hs.push(round64_sum(vec[k], upper[k]) / 2.0);
    }
    let mut used: Vec<usize> = (0..f.len()).collect();
    let b = if f.len() < 2 {
        f64::NAN
    } else {
        if rm_duplicate {
            used = (0..f.len()).filter(|&k| k + 1 == f.len() || f[k + 1] != f[k]).collect();
        }
        let fu: Vec<f64> = used.iter().map(|&k| f[k]).collect();
        let su: Vec<f64> = used.iter().map(|&k| starts[k]).collect();
        r_cor(&fu, &su, method)?
    };
    Ok(Boyce { cor: r_round(b, 3), f_ratio: f, hs, used })
}

/// The confusion table's columns.
pub const TABLE: [&str; 16] =
    ["threshold", "tp", "fp", "fn", "tn", "kappa", "ccr", "tpr", "tnr", "fpr", "fnr", "ppp", "npp", "mcr", "or", "tss"];
/// The optimal thresholds.
pub const THRESHOLDS: [&str; 7] =
    ["max_kappa", "max_spec_sens", "no_omission", "equal_prevalence", "equal_sens_spec", "mtp", "p10"];
/// The statistics.
pub const STATS: [&str; 8] = ["auc", "cor", "pcor", "np", "na", "prevalence", "max_tss", "boyce"];

/// dismo's evaluate: the statistics, the confusion table (a column per TABLE) and the thresholds.
pub struct Evaluation {
    pub stats: Vec<f64>,
    pub table: Vec<Vec<f64>>,
    pub thresholds: Vec<f64>,
    pub boyce: Boyce,
}

/// The 10th percentile training presence threshold (ENMeval's).
fn p10(train: &[f64]) -> f64 {
    let n = train.len();
    let k = if n < 10 { (n as f64 * 0.9).floor() } else { (n as f64 * 0.9).ceil() } as usize;
    if k < 1 {
        return f64::NAN;
    }
    let mut s = train.to_vec();
    s.sort_by(|a, b| b.partial_cmp(a).unwrap());
    s[k - 1]
}

pub fn evaluate(presences: &[f64], background: &[f64], thresholds: Option<&[f64]>) -> Result<Evaluation, String> {
    let p = values(presences);
    let a = values(background);
    if p.is_empty() || a.is_empty() {
        return Err("need predictions at presences and at background points".into());
    }
    let (n_p, n_a) = (p.len(), a.len());
    let both: Vec<f64> = p.iter().chain(&a).copied().collect();
    let ranks = rankdata(&both);
    let r = numpy_sum(&ranks[..n_p]) - n_p as f64 * (n_p as f64 + 1.0) / 2.0;
    let auc = r / (n_a as f64 * n_p as f64);
    let (mut cor, mut pcor) = (f64::NAN, f64::NAN);
    if n_p + n_a > 2 {
        let ind: Vec<f64> = (0..n_p + n_a).map(|k| if k < n_p { 1.0 } else { 0.0 }).collect();
        cor = r_cor(&both, &ind, "pearson")?;
        if cor.is_finite() {
            let df = (n_p + n_a - 2) as f64;
            let t =
                if cor.abs() < 1.0 { df.sqrt() * cor / (1.0 - cor * cor).sqrt() } else { f64::INFINITY.copysign(cor) };
            let (lo, hi) = (student_cdf(t, df), student_cdf(-t, df));
            pcor = 2.0 * if hi < lo { hi } else { lo };
        }
    }
    let tr: Vec<f64> = match thresholds {
        Some(t) => {
            let mut t = t.to_vec();
            t.sort_by(|x, y| x.partial_cmp(y).unwrap());
            t
        }
        None => {
            let probs: Vec<f64> = (0..=1000).map(|k| k as f64 / 1000.0).collect();
            let t1 = if p.len() > 1000 { quantile7(&p, &probs) } else { p.clone() };
            let t2 = if a.len() > 1000 { quantile7(&a, &probs) } else { a.clone() };
            let mut t: Vec<f64> = t1.iter().chain(&t2).map(|v| r_round(*v, 8)).collect();
            t.sort_by(|x, y| x.partial_cmp(y).unwrap());
            t.dedup();
            let last = t[t.len() - 1];
            t.iter().map(|v| v - 0.0001).chain([last, last + 0.0001]).collect()
        }
    };
    let mut ps = p.clone();
    ps.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let mut as_ = a.clone();
    as_.sort_by(|x, y| x.partial_cmp(y).unwrap());
    let (np_, na_) = (n_p as f64, n_a as f64);
    let nn = (n_p + n_a) as f64;
    let m = tr.len();
    let mut cols: Vec<Vec<f64>> = vec![Vec::with_capacity(m); TABLE.len()];
    for &t in &tr {
        let tp = np_ - ps.partition_point(|&x| x < t) as f64;
        let fp = na_ - as_.partition_point(|&x| x < t) as f64;
        let fn_ = np_ - tp;
        let tn = na_ - fp;
        let tpr = tp / (tp + fn_);
        let tnr = tn / (fp + tn);
        let pra = (tp + tn) / nn;
        let pry = (tp + fp) / nn * (tp + fn_) / nn;
        let prn = (fn_ + tn) / nn * (fp + tn) / nn;
        let pre = pry + prn;
        let row = [
            t,
            tp,
            fp,
            fn_,
            tn,
            (pra - pre) / (1.0 - pre),
            (tp + tn) / nn,
            tpr,
            tnr,
            fp / (fp + tn),
            fn_ / (tp + fn_),
            tp / (tp + fp),
            tn / (fn_ + tn),
            (fp + fn_) / nn,
            tp * tn / (fn_ * fp),
            tpr + tnr - 1.0,
        ];
        for (c, v) in cols.iter_mut().zip(row) {
            c.push(v);
        }
    }
    let col = |name: &str| &cols[TABLE.iter().position(|c| *c == name).unwrap()];
    let prevalence = (col("tp")[0] + col("fn")[0]) / nn;
    let first = |values: &[f64], max: bool| -> f64 {
        let v: Vec<f64> = values
            .iter()
            .map(|x| if x.is_nan() { if max { f64::NEG_INFINITY } else { f64::INFINITY } } else { *x })
            .collect();
        let mut best = 0;
        for k in 1..v.len() {
            if (max && v[k] > v[best]) || (!max && v[k] < v[best]) {
                best = k;
            }
        }
        tr[best]
    };
    let (tpr, tnr) = (col("tpr"), col("tnr"));
    let ss: Vec<f64> = (0..m).map(|k| tpr[k] + tnr[k]).collect();
    let no_omit = (0..m).filter(|&k| col("fn")[k] == 0.0).max();
    let thresholds = vec![
        first(col("kappa"), true),
        first(&ss, true),
        no_omit.map_or(f64::NAN, |k| tr[k]),
        first(&tr.iter().map(|t| (t - prevalence).abs()).collect::<Vec<_>>(), false),
        first(&(0..m).map(|k| (tpr[k] - tnr[k]).abs()).collect::<Vec<_>>(), false),
        p.iter().copied().fold(f64::INFINITY, f64::min),
        p10(&p),
    ];
    let avail: Vec<f64> = a.iter().chain(&p).copied().collect();
    let b = boyce(&p, &avail, &Classes::Window(None, 100), "spearman", true)?;
    let max_tss = (0..m)
        .map(|k| tpr[k] + tnr[k] - 1.0)
        .filter(|x| !x.is_nan())
        .fold(f64::NAN, |a, x| if a.is_nan() || x > a { x } else { a });
    let stats = vec![auc, cor, pcor, np_, na_, prevalence, max_tss, b.cor];
    Ok(Evaluation { stats, table: cols, thresholds, boyce: b })
}

/// A threshold by method: one of THRESHOLDS, or the highest whose sensitivity reaches `sensitivity`.
pub fn threshold(presences: &[f64], background: &[f64], method: &str, sensitivity: f64) -> Result<f64, String> {
    let e = evaluate(presences, background, None)?;
    if method == "sensitivity" {
        let tpr = &e.table[TABLE.iter().position(|c| *c == "tpr").unwrap()];
        let ok = (0..tpr.len()).filter(|&k| tpr[k] >= sensitivity).max();
        return Ok(ok.map_or(f64::NAN, |k| e.table[0][k]));
    }
    match THRESHOLDS.iter().position(|t| *t == method) {
        Some(k) => Ok(e.thresholds[k]),
        None => Err(format!("unknown method {method:?}; one of {}, sensitivity", THRESHOLDS.join(", "))),
    }
}

/// The share of the test presences below the training's minimum (mtp) or 10th percentile (p10) presence.
pub fn omission_rate(train: &[f64], test: &[f64], threshold: &str) -> Result<f64, String> {
    let (train, test) = (values(train), values(test));
    let t = match threshold {
        "mtp" => train.iter().copied().fold(f64::INFINITY, f64::min),
        "p10" => p10(&train),
        t => return Err(format!("threshold must be mtp or p10, got {t:?}")),
    };
    if t.is_nan() {
        return Ok(f64::NAN);
    }
    if test.is_empty() {
        return Ok(f64::NAN);
    }
    let k = test.iter().filter(|&&x| x < t).count();
    Ok(round64_ratio(k as u128, test.len() as u128))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding_to_64_bits() {
        assert_eq!(round64_ratio(1, 3), 1.0 / 3.0);
        assert_eq!(round64_ratio(0, 3), 0.0);
        assert_eq!(round64_sum(0.1, 0.2), 0.30000000000000004);
        assert_eq!(r_round(0.5, 0), 0.0);
    }
}
