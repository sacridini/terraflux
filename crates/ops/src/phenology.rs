//! The seasons of a time series: the kernels of `objects.series` (a series per object) and `raster.phenology` (a series
//! per cell), which are the same numbers.
//!
//! A series is its dates (days since 1970-01-01, strictly increasing) and a value on each, NaN where there is none (a
//! cloud). Three groups of measures, each from the values that exist:
//!
//! - `phenology`: the maximum, the minimum, the amplitude (maximum less minimum), the day of the peak (the first
//!   maximum), the integral (trapezoids over the days: value times days), and the season by the amplitude method of
//!   TIMESAT (Jonsson and Eklundh 2004): each side of the peak has its own base, its own minimum (the left minimum of
//!   the values up to the peak, the right minimum of those from it on); the start level is `left + t (max - left)` and
//!   the end level `right + t (max - right)` for the threshold `t`. The start is where the series, walked back from the
//!   peak, first falls below the start level (found by linear interpolation between the two dates it falls between),
//!   the end where it first falls below the end level walking forward; the length is the end less the start, the rise
//!   rate is (max - start level) over the days from the start to the peak and the fall rate (max - end level) over the
//!   days from the peak to the end.
//!   No crossing on a side (the peak is the first or the last date, so that side has nothing to cross) leaves the
//!   start or the end NaN.
//! - the smoothing the measures are taken on: none (the dates that have a value, joined by straight lines),
//!   Savitzky-Golay (a polynomial fitted to each window, as scipy's `savgol_filter` with mode "interp", on the series of
//!   all the dates with the holes filled by straight lines in time and the ends by their nearest value: the filter
//!   reads the dates as evenly spaced, so it is exact for evenly spaced dates and a warning says when they are not), or
//!   the harmonic fit below evaluated at the dates.
//! - `harmonics`: the least squares fit of `n` harmonics of a period of one year (365.25 days), with a straight trend
//!   when asked (Jakubauskas et al. 2001; the model of CCDC without its breaks). With `t` the days since 1 January of
//!   the first date's year, the model is `mean + trend * (t - mean of t) / 365.25 + sum_k a_k cos(2 pi k t / 365.25) +
//!   b_k sin(...)`; the amplitude of the k-th harmonic is `sqrt(a_k^2 + b_k^2)` and its phase `atan2(b_k, a_k)` from 0 to
//!   2 pi, so the harmonic peaks `phase * 365.25 / (2 pi k)` days after 1 January; the RMSE is the root of the mean
//!   squared residual over the values. Fewer values than `2n + 2` (`2n + 3` with a trend) give NaN.

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::swat_weather::{civil_from_days, days_from_civil};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Stack, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Phenology));
}

/// The days in a year the harmonics' period is.
const YEAR: f64 = 365.25;

/// The measures of the group `phenology`, in order.
pub const PHENOLOGY: [&str; 13] = [
    "max",
    "min",
    "amplitude",
    "peak_doy",
    "peak_date",
    "integral",
    "start_doy",
    "start_date",
    "end_doy",
    "end_date",
    "length",
    "rise_rate",
    "fall_rate",
];

/// How the series is smoothed for the measures of the season.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Smooth {
    None,
    /// Savitzky-Golay: the window (odd) and the order of the polynomial.
    Savgol(usize, usize),
    /// The harmonic fit of the settings' `n` and `trend`.
    Harmonic,
}

/// What the kernels take besides the series.
#[derive(Debug, Clone, Copy)]
pub struct Settings {
    pub smooth: Smooth,
    /// The fraction of the amplitude above the base where the season starts and ends, 0 to 1 (excluded).
    pub threshold: f64,
    /// The harmonics fitted.
    pub n: usize,
    pub trend: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { smooth: Smooth::None, threshold: 0.5, n: 2, trend: false }
    }
}

/// The day of the year (1 for 1 January) of a day since 1970-01-01, the fraction kept.
pub fn doy(day: f64) -> f64 {
    let (y, _, _) = civil_from_days(day.floor() as i64);
    day - days_from_civil(y, 1, 1) as f64 + 1.0
}

/// A date in a text: `YYYY-MM-DD`, `YYYY_MM_DD`, `YYYY.MM.DD` or `YYYYMMDD` (not part of a longer run of digits), the
/// first that is a date, as days since 1970-01-01.
pub fn date_in(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    let digits = |from: usize, n: usize| b.get(from..from + n).is_some_and(|x| x.iter().all(u8::is_ascii_digit));
    let alone = |from: usize, n: usize| {
        (from == 0 || !b[from - 1].is_ascii_digit()) && b.get(from + n).is_none_or(|c| !c.is_ascii_digit())
    };
    let number = |from: usize, n: usize| s[from..from + n].parse::<i64>().ok();
    let valid = |y: i64, m: i64, d: i64| {
        ((1900..=2100).contains(&y) && (1..=12).contains(&m) && (1..=31).contains(&d)).then(|| {
            let day = days_from_civil(y, m, d);
            // (a day that is not in the month, 30 February, is no date)
            (civil_from_days(day) == (y, m, d)).then_some(day as f64)
        })
    };
    for i in 0..b.len() {
        // YYYY-MM-DD and its kin
        if digits(i, 4)
            && matches!(b.get(i + 4), Some(b'-' | b'_' | b'.'))
            && digits(i + 5, 2)
            && b.get(i + 7) == b.get(i + 4)
            && digits(i + 8, 2)
            && alone(i, 10)
            && let (Some(y), Some(m), Some(d)) = (number(i, 4), number(i + 5, 2), number(i + 8, 2))
            && let Some(Some(day)) = valid(y, m, d)
        {
            return Some(day);
        }
        // YYYYMMDD
        if digits(i, 8)
            && alone(i, 8)
            && let (Some(y), Some(m), Some(d)) = (number(i, 4), number(i + 4, 2), number(i + 6, 2))
            && let Some(Some(day)) = valid(y, m, d)
        {
            return Some(day);
        }
    }
    None
}

/// The dates of the layers of a series: the setting `dates` (a list of YYYY-MM-DD, one a layer) or, without it, a
/// date in each layer's name; strictly increasing. Days since 1970-01-01.
pub fn dates_of(node: &str, args: &Args, names: &[String]) -> Result<Vec<f64>> {
    let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let days: Vec<f64> = match args.opt_param::<Vec<String>>("dates")? {
        Some(list) => {
            if list.len() != names.len() {
                return Err(bad(format!("dates: {} dates for {} layers", list.len(), names.len())));
            }
            list.iter()
                .map(|d| crate::data::parse_date(d).map(|x| x as f64).map_err(|e| bad(format!("dates: {e}"))))
                .collect::<Result<_>>()?
        }
        None => names
            .iter()
            .map(|n| {
                date_in(n).ok_or_else(|| {
                    bad(format!(
                        "no date in the name of the layer {n:?} (YYYY-MM-DD or YYYYMMDD): name the layers by their \
                         dates, or give the setting dates"
                    ))
                })
            })
            .collect::<Result<_>>()?,
    };
    if let Some(k) = (1..days.len()).find(|&k| days[k] <= days[k - 1]) {
        return Err(bad(format!(
            "the dates must increase: the layer {:?} is not after the layer {:?}",
            names[k],
            names[k - 1]
        )));
    }
    Ok(days)
}

// -- the least squares ----------------------------------------------------------------------------------------------

/// The least squares solution of `a x = y` (`a` a row per observation) by Householder's QR; none when the columns
/// are dependent or there are fewer rows than columns.
fn lstsq(a: &[Vec<f64>], y: &[f64]) -> Option<Vec<f64>> {
    let (m, p) = (a.len(), a.first().map_or(0, Vec::len));
    if m < p || p == 0 {
        return None;
    }
    // column-major copy
    let mut q: Vec<Vec<f64>> = (0..p).map(|j| a.iter().map(|r| r[j]).collect()).collect();
    let mut b = y.to_vec();
    let scale = q.iter().flatten().fold(0.0f64, |s, v| s.max(v.abs())).max(1e-300);
    for j in 0..p {
        let norm = q[j][j..].iter().map(|v| v * v).sum::<f64>().sqrt();
        if norm <= 1e-12 * scale {
            return None;
        }
        let alpha = if q[j][j] > 0.0 { -norm } else { norm };
        let mut v: Vec<f64> = q[j][j..].to_vec();
        v[0] -= alpha;
        let vv: f64 = v.iter().map(|x| x * x).sum();
        if vv > 0.0 {
            for col in q[j..].iter_mut() {
                let dot: f64 = (j..m).map(|i| v[i - j] * col[i]).sum();
                let f = 2.0 * dot / vv;
                for i in j..m {
                    col[i] -= f * v[i - j];
                }
            }
            let dot: f64 = (j..m).map(|i| v[i - j] * b[i]).sum();
            let f = 2.0 * dot / vv;
            for i in j..m {
                b[i] -= f * v[i - j];
            }
        }
    }
    // back substitution on R
    let mut x = vec![0.0; p];
    for j in (0..p).rev() {
        let s: f64 = (j + 1..p).map(|c| q[c][j] * x[c]).sum();
        if q[j][j].abs() <= 1e-12 * scale {
            return None;
        }
        x[j] = (b[j] - s) / q[j][j];
    }
    Some(x)
}

// -- the smoothing --------------------------------------------------------------------------------------------------

/// The values at positions `at` of the polynomial of degree `order` fitted by least squares to `y` at positions
/// 0, 1... (none when the fit is singular).
fn polyfit_at(y: &[f64], order: usize, at: &[f64]) -> Option<Vec<f64>> {
    let w = y.len();
    let c = (w as f64 - 1.0) / 2.0;
    let s = if c > 0.0 { c } else { 1.0 };
    let row = |x: f64| -> Vec<f64> { (0..=order).map(|p| ((x - c) / s).powi(p as i32)).collect() };
    let a: Vec<Vec<f64>> = (0..w).map(|i| row(i as f64)).collect();
    let coef = lstsq(&a, y)?;
    Some(at.iter().map(|&x| row(x).iter().zip(&coef).map(|(r, k)| r * k).sum()).collect())
}

/// scipy's `savgol_filter(x, window, order, mode="interp")`: each value from the polynomial of the order fitted to the
/// window around it, the first and last half windows from the polynomial fitted to the first and the last window.
pub fn savgol_filter(x: &[f64], window: usize, order: usize) -> std::result::Result<Vec<f64>, String> {
    if window.is_multiple_of(2) || window < 3 {
        return Err(format!("window {window}: an odd number, 3 or more"));
    }
    if order >= window {
        return Err(format!("order {order}: below the window {window}"));
    }
    if window > x.len() {
        return Err(format!("window {window}: more than the {} values", x.len()));
    }
    let (n, m) = (x.len(), window / 2);
    // the weights of the centre value: the first row of the pseudo-inverse of the window's polynomial basis
    let basis: Vec<Vec<f64>> =
        (0..window).map(|k| (0..=order).map(|p| ((k as f64 - m as f64) / m as f64).powi(p as i32)).collect()).collect();
    // (the weights are the solution of the normal equations A'A z = e0, weight_k = A_k . z)
    let p = order + 1;
    let mut ata = vec![vec![0.0; p + 1]; p];
    for i in 0..p {
        for j in 0..p {
            ata[i][j] = basis.iter().map(|r| r[i] * r[j]).sum();
        }
    }
    ata[0][p] = 1.0;
    for c in 0..p {
        let pivot = (c..p).max_by(|&a, &b| ata[a][c].abs().total_cmp(&ata[b][c].abs())).unwrap();
        ata.swap(c, pivot);
        let d = ata[c][c];
        if d.abs() < 1e-14 {
            return Err("the window and the order give a singular fit".into());
        }
        for r in 0..p {
            if r != c {
                let f = ata[r][c] / d;
                let pivot_row = ata[c].clone();
                for (x, y) in ata[r][c..].iter_mut().zip(&pivot_row[c..]) {
                    *x -= f * y;
                }
            }
        }
    }
    let z: Vec<f64> = (0..p).map(|i| ata[i][p] / ata[i][i]).collect();
    let weights: Vec<f64> = basis.iter().map(|r| r.iter().zip(&z).map(|(a, b)| a * b).sum()).collect();
    let mut out = vec![0.0; n];
    for i in m..n - m {
        out[i] = (0..window).map(|k| weights[k] * x[i + k - m]).sum();
    }
    let at: Vec<f64> = (0..m).map(|k| k as f64).collect();
    let head = polyfit_at(&x[..window], order, &at).ok_or("the window and the order give a singular fit")?;
    out[..m].copy_from_slice(&head);
    let at: Vec<f64> = (window - m..window).map(|k| k as f64).collect();
    let tail = polyfit_at(&x[n - window..], order, &at).ok_or("the window and the order give a singular fit")?;
    out[n - m..].copy_from_slice(&tail);
    Ok(out)
}

/// The series with its holes (NaN) filled by straight lines in time, the ends by their nearest value; none with fewer
/// than two values.
pub fn fill(t: &[f64], y: &[f64]) -> Option<Vec<f64>> {
    let known: Vec<usize> = (0..y.len()).filter(|&i| y[i].is_finite()).collect();
    if known.len() < 2 {
        return None;
    }
    let mut out = y.to_vec();
    for i in 0..y.len() {
        if y[i].is_finite() {
            continue;
        }
        let after = known.partition_point(|&k| k < i);
        out[i] = match (after.checked_sub(1).map(|a| known[a]), known.get(after).copied()) {
            (Some(a), Some(b)) => y[a] + (y[b] - y[a]) * (t[i] - t[a]) / (t[b] - t[a]),
            (Some(a), None) => y[a],
            (None, Some(b)) => y[b],
            (None, None) => return None,
        };
    }
    Some(out)
}

/// The days of the series from 1 January of the first date's year.
fn origin(t: &[f64]) -> f64 {
    let (y, _, _) = civil_from_days(t[0].floor() as i64);
    days_from_civil(y, 1, 1) as f64
}

/// The design row of the harmonic model at `t` (days from the origin), `tc` the centred time in years.
fn design(t: f64, tc: f64, n: usize, trend: bool) -> Vec<f64> {
    let mut r = vec![1.0];
    if trend {
        r.push(tc);
    }
    for k in 1..=n {
        let a = 2.0 * std::f64::consts::PI * k as f64 * t / YEAR;
        r.push(a.cos());
        r.push(a.sin());
    }
    r
}

/// The names of the measures of the group `harmonics` for `n` harmonics: the mean, the trend (when asked), the
/// amplitude and the phase of each harmonic and the RMSE.
pub fn harmonic_names(n: usize, trend: bool) -> Vec<String> {
    let mut v = vec!["hmean".to_string()];
    if trend {
        v.push("trend".into());
    }
    for k in 1..=n {
        v.push(format!("amp{k}"));
        v.push(format!("phase{k}"));
    }
    v.push("rmse".into());
    v
}

/// The fit of `n` harmonics (and a trend): the coefficients, the values the fit gives at the dates `t`, and the
/// measures in the order of [`harmonic_names`]; none with too few values.
struct Fit {
    values: Vec<f64>,
    measures: Vec<f64>,
}

fn fit_harmonics(t: &[f64], y: &[f64], n: usize, trend: bool) -> Option<Fit> {
    let o = origin(t);
    let idx: Vec<usize> = (0..y.len()).filter(|&i| y[i].is_finite()).collect();
    let need = 2 * n + 2 + usize::from(trend);
    if n == 0 || idx.len() < need {
        return None;
    }
    let mean_t = idx.iter().map(|&i| t[i] - o).sum::<f64>() / idx.len() as f64;
    let row = |i: usize| design(t[i] - o, (t[i] - o - mean_t) / YEAR, n, trend);
    let a: Vec<Vec<f64>> = idx.iter().map(|&i| row(i)).collect();
    let yy: Vec<f64> = idx.iter().map(|&i| y[i]).collect();
    let c = lstsq(&a, &yy)?;
    let predict = |i: usize| row(i).iter().zip(&c).map(|(r, k)| r * k).sum::<f64>();
    let values: Vec<f64> = (0..t.len()).map(predict).collect();
    let rmse = (idx.iter().map(|&i| (y[i] - values[i]).powi(2)).sum::<f64>() / idx.len() as f64).sqrt();
    let mut m = vec![c[0]];
    let first = if trend {
        m.push(c[1]);
        2
    } else {
        1
    };
    for k in 0..n {
        let (a, b) = (c[first + 2 * k], c[first + 2 * k + 1]);
        let phase = b.atan2(a).rem_euclid(2.0 * std::f64::consts::PI);
        m.push((a * a + b * b).sqrt());
        m.push(phase);
    }
    m.push(rmse);
    Some(Fit { values, measures: m })
}

/// The measures of the group `harmonics` ([`harmonic_names`]), NaN when the series has too few values.
pub fn harmonics(t: &[f64], y: &[f64], n: usize, trend: bool) -> Vec<f64> {
    match fit_harmonics(t, y, n, trend) {
        Some(f) => f.measures,
        None => vec![f64::NAN; harmonic_names(n, trend).len()],
    }
}

/// The series the measures of the season are taken on, on the dates it is given for: the dates and the values.
/// `None` when it cannot be made (too few values, a window longer than the series).
pub fn smoothed(t: &[f64], y: &[f64], s: &Settings) -> Option<(Vec<f64>, Vec<f64>)> {
    let known: Vec<usize> = (0..y.len()).filter(|&i| y[i].is_finite()).collect();
    if known.len() < 3 {
        return None;
    }
    match s.smooth {
        Smooth::None => Some((known.iter().map(|&i| t[i]).collect(), known.iter().map(|&i| y[i]).collect())),
        Smooth::Savgol(window, order) => {
            let f = fill(t, y)?;
            Some((t.to_vec(), savgol_filter(&f, window, order).ok()?))
        }
        Smooth::Harmonic => fit_harmonics(t, y, s.n, s.trend).map(|f| (t.to_vec(), f.values)),
    }
}

/// The measures of the group `phenology` ([`PHENOLOGY`]) of a series, NaN where it has too few values or a season
/// has no crossing.
pub fn phenology(t: &[f64], y: &[f64], s: &Settings) -> [f64; 13] {
    let mut out = [f64::NAN; 13];
    let Some((ts, vs)) = smoothed(t, y, s) else { return out };
    let n = vs.len();
    let mut peak = 0;
    for i in 1..n {
        if vs[i] > vs[peak] {
            peak = i;
        }
    }
    let max = vs[peak];
    let min = vs.iter().copied().fold(f64::INFINITY, f64::min);
    let left = vs[..=peak].iter().copied().fold(f64::INFINITY, f64::min);
    let right = vs[peak..].iter().copied().fold(f64::INFINITY, f64::min);
    let (level_left, level_right) = (left + s.threshold * (max - left), right + s.threshold * (max - right));
    let integral: f64 = (0..n - 1).map(|i| (vs[i] + vs[i + 1]) / 2.0 * (ts[i + 1] - ts[i])).sum();
    // the season: the nearest crossing of the level on each side of the peak
    let start = (0..peak)
        .rev()
        .find(|&j| vs[j] < level_left)
        .map(|j| ts[j] + (level_left - vs[j]) / (vs[j + 1] - vs[j]) * (ts[j + 1] - ts[j]));
    let end = (peak + 1..n)
        .find(|&j| vs[j] < level_right)
        .map(|j| ts[j - 1] + (vs[j - 1] - level_right) / (vs[j - 1] - vs[j]) * (ts[j] - ts[j - 1]));
    let day = |d: Option<f64>| d.unwrap_or(f64::NAN);
    let (start, end) = (day(start), day(end));
    let rise = if start.is_finite() && ts[peak] > start { (max - level_left) / (ts[peak] - start) } else { f64::NAN };
    let fall = if end.is_finite() && end > ts[peak] { (max - level_right) / (end - ts[peak]) } else { f64::NAN };
    let d = |x: f64| if x.is_finite() { doy(x) } else { f64::NAN };
    out[0] = max;
    out[1] = min;
    out[2] = max - min;
    out[3] = doy(ts[peak]);
    out[4] = ts[peak];
    out[5] = integral;
    out[6] = d(start);
    out[7] = start;
    out[8] = d(end);
    out[9] = end;
    out[10] = end - start;
    out[11] = rise;
    out[12] = fall;
    out
}

// -- raster.phenology -----------------------------------------------------------------------------------------------

/// The settings of the kernels from a node's settings.
pub fn settings_of(node: &str, args: &Args) -> Result<Settings> {
    let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let threshold: f64 = args.param("season_threshold", 0.5)?;
    if !(threshold > 0.0 && threshold < 1.0) {
        return Err(bad(format!("season_threshold {threshold}: above 0 and below 1")));
    }
    let n: usize = args.param("n", 2)?;
    if n == 0 || n > 6 {
        return Err(bad(format!("n {n}: 1 to 6 harmonics")));
    }
    let smooth = match args.param("smooth", "none".to_string())?.as_str() {
        "none" => Smooth::None,
        "savgol" => {
            let (window, order): (usize, usize) = (args.param("window", 7)?, args.param("order", 2)?);
            if window.is_multiple_of(2) || window < 3 || order >= window {
                return Err(bad(format!(
                    "window {window}, order {order}: an odd window of 3 or more and an order below it"
                )));
            }
            Smooth::Savgol(window, order)
        }
        "harmonic" => Smooth::Harmonic,
        o => return Err(bad(format!("smooth {o:?}: none, savgol or harmonic"))),
    };
    Ok(Settings { smooth, threshold, n, trend: args.param("trend", false)? })
}

/// The groups asked for (`values` is for objects only).
pub fn groups_of(node: &str, args: &Args, allowed: &[&str], default: &[&str]) -> Result<Vec<String>> {
    let groups: Vec<String> = match args.params.get("groups") {
        None | Some(serde_json::Value::Null) => default.iter().map(|s| s.to_string()).collect(),
        Some(serde_json::Value::String(s)) => vec![s.clone()],
        Some(v) => serde_json::from_value(v.clone())
            .map_err(|_| Error::msg(format!("node {node:?}: groups: a list of {}", allowed.join(", "))))?,
    };
    if let Some(g) = groups.iter().find(|g| !allowed.contains(&g.as_str())) {
        return Err(Error::msg(format!("node {node:?}: group {g:?}: one of {}", allowed.join(", "))));
    }
    if groups.is_empty() {
        return Err(Error::msg(format!("node {node:?}: groups: at least one of {}", allowed.join(", "))));
    }
    Ok(groups)
}

/// `raster.phenology`.
struct Phenology;

impl Operator for Phenology {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.phenology",
            "The seasons of the time series of each cell (the same numbers objects.series gives each object): a stack \
             with a layer per date, the dates in the layers' names (YYYY-MM-DD or YYYYMMDD) or the setting dates. \
             Group phenology: max, min, amplitude, peak_doy and peak_date (days since 1970-01-01), integral \
             (trapezoids, value times days), start and end of the season (start_doy, start_date, end_doy, end_date), \
             length, rise_rate and fall_rate. The season is TIMESAT's amplitude method (Jonsson and Eklundh 2004): \
             each side of the peak starts from its own minimum, the level of the start is left minimum + \
             season_threshold (max - left minimum) and that of the end right minimum + season_threshold (max - right \
             minimum), crossed on its side of the peak, found by linear interpolation. \
             Group harmonics: the least squares fit of n harmonics of a one-year period (Jakubauskas et al. 2001): \
             hmean, trend (with trend), amp and phase of each, rmse. A cell with NaN on a date is fitted without it; \
             too few values give NaN. smooth savgol (window, order) or harmonic smooths the series first.",
            Access::Local,
        )
        .input(PortSpec::new("series", Type::Any, "a stack: a layer per date, the dates in their names"))
        .output(PortSpec::new("metrics", Type::Stack, "a layer per measure"))
        .param(PortSpec::optional(
            "dates",
            Type::Any,
            "the layers' dates, YYYY-MM-DD, a list in the layers' order (default: read in their names)",
        ))
        .param(PortSpec::optional("groups", Type::Any, "phenology (default) and/or harmonics"))
        .param(
            PortSpec::optional(
                "smooth",
                Type::Text,
                "none (default), savgol or harmonic: the series the season is measured on",
            )
            .choices(["none", "savgol", "harmonic"]),
        )
        .param(PortSpec::optional("window", Type::Integer, "savgol: the window, odd (default 7)"))
        .param(PortSpec::optional(
            "order",
            Type::Integer,
            "savgol: the polynomial's order, below the window (default 2)",
        ))
        .param(PortSpec::optional("n", Type::Integer, "the harmonics, 1 to 6 (default 2)"))
        .param(PortSpec::optional("trend", Type::Bool, "harmonics: a straight trend too (default false)"))
        .param(PortSpec::optional(
            "season_threshold",
            Type::Number,
            "the fraction of the amplitude above the base where the season starts and ends, 0 to 1 (default 0.5)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let l = crate::raster_stats::layers(args, "series")?;
        let t = dates_of(node, args, &l.names)?;
        if t.len() < 4 {
            return Err(Error::msg(format!("node {node:?}: {} dates: 4 at least", t.len())));
        }
        let s = settings_of(node, args)?;
        let groups = groups_of(node, args, &["phenology", "harmonics"], &["phenology"])?;
        let (pheno, harm) = (groups.iter().any(|g| g == "phenology"), groups.iter().any(|g| g == "harmonics"));
        let mut names: Vec<String> = Vec::new();
        if pheno {
            names.extend(PHENOLOGY.iter().map(|s| s.to_string()));
        }
        if harm {
            names.extend(harmonic_names(s.n, s.trend));
        }
        uneven_warning(ctx, node, &t, &s);
        let cells = l.grid.cells();
        let per: Vec<Vec<f64>> = (0..cells)
            .into_par_iter()
            .map(|i| {
                let y: Vec<f64> = l.data.iter().map(|d| d[i]).collect();
                let mut v = Vec::with_capacity(names.len());
                if pheno {
                    v.extend(phenology(&t, &y, &s));
                }
                if harm {
                    v.extend(harmonics(&t, &y, s.n, s.trend));
                }
                v
            })
            .collect();
        let layers = (0..names.len()).map(|k| Arc::new(per.iter().map(|v| v[k]).collect())).collect();
        let stack =
            Stack { grid: l.grid.clone(), keys: names.into_iter().map(crate::value::Key::Name).collect(), layers };
        Ok(outputs([("metrics", Value::Stack(stack))]))
    }
}

/// A warning when the dates are not evenly spaced and the Savitzky-Golay filter reads them as if they were.
pub fn uneven_warning(ctx: &mut Ctx, node: &str, t: &[f64], s: &Settings) {
    if !matches!(s.smooth, Smooth::Savgol(..)) || t.len() < 3 {
        return;
    }
    let gaps: Vec<f64> = t.windows(2).map(|w| w[1] - w[0]).collect();
    let (lo, hi) = gaps.iter().fold((f64::INFINITY, 0.0f64), |(a, b), &g| (a.min(g), b.max(g)));
    if hi > lo * 1.25 {
        ctx.warn(format!(
            "node {node:?}: the dates are {lo} to {hi} days apart: the Savitzky-Golay filter reads them as evenly \
             spaced (a window is a number of dates, not of days)"
        ));
    }
}
