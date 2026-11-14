//! Weather for a SWAT+ project from tables (landschaft's `hydro/swat/_weather.py`, bit for bit): observed daily
//! series as the SWAT+ Editor's weather import writes them (`*.pcp`, `*.tmp`, `*.slr`, `*.hmd`, `*.wnd` and their
//! `*.cli` lists, -99 for missing days), and the weather generator statistics (`weather-wgn.cli`, SWAT's WXGEN
//! parameters) computed from the series (`swat.wgn`) or given. The sums, means and standard deviations are numpy's
//! and pandas' (pairwise summation), in the same order.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::npmath;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(WgnOp));
}

/// The monthly columns of `weather-wgn.cli` (SWAT+'s `weather_wgn_cli_mon`).
pub const WGN_COLUMNS: [&str; 14] = [
    "tmp_max_ave",
    "tmp_min_ave",
    "tmp_max_sd",
    "tmp_min_sd",
    "pcp_ave",
    "pcp_sd",
    "pcp_skew",
    "wet_dry",
    "wet_wet",
    "pcp_days",
    "pcp_hhr",
    "slr_ave",
    "dew_ave",
    "wnd_ave",
];

// ------------------------------------------------------------------------------------------------------- dates

/// Days since 1970-01-01 of a civil date.
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// The civil date (year, month, day) of a day since 1970-01-01.
pub fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// The day of the year (1 to 366) of a day.
pub fn day_of_year(day: i64) -> i64 {
    let (y, _, _) = civil_from_days(day);
    day - days_from_civil(y, 1, 1) + 1
}

fn month_len(y: i64, m: i64) -> i64 {
    match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A date as text (2001-01-31, 2001/01/31 or 20010131, a time after it left out), as days since 1970-01-01.
pub fn parse_date(s: &str) -> Option<i64> {
    let head = s.trim().split([' ', 'T']).next()?;
    let parts: Vec<&str> = if head.contains('-') || head.contains('/') {
        head.split(['-', '/']).collect()
    } else if head.len() == 8 && head.bytes().all(|b| b.is_ascii_digit()) {
        vec![&head[0..4], &head[4..6], &head[6..8]]
    } else {
        return None;
    };
    let [y, m, d] = parts.as_slice() else { return None };
    let (y, m, d): (i64, i64, i64) = (y.parse().ok()?, m.parse().ok()?, d.parse().ok()?);
    ((1..=12).contains(&m) && d >= 1 && d <= month_len(y, m)).then(|| days_from_civil(y, m, d))
}

// ----------------------------------------------------------------------------------------------------- tables

/// A column by name without regard to case.
pub fn col_of(t: &Table, name: &str) -> Option<usize> {
    t.columns.iter().position(|c| c.eq_ignore_ascii_case(name))
}

/// Daily series: dates (days, sorted) and a column of values per station (NaN for a missing day).
#[derive(Debug, Clone)]
pub struct Frame {
    pub days: Vec<i64>,
    pub cols: Vec<(String, Vec<f64>)>,
}

impl Frame {
    pub fn col(&self, name: &str) -> Option<&[f64]> {
        self.cols.iter().find(|(n, _)| n == name).map(|(_, v)| v.as_slice())
    }
}

/// The stations: name, latitude and longitude (degrees) and elevation (m).
#[derive(Debug, Clone)]
pub struct Stations {
    pub names: Vec<String>,
    pub lat: Vec<f64>,
    pub lon: Vec<f64>,
    pub elev: Vec<f64>,
}

impl Stations {
    pub fn index(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }
}

pub fn stations_of(t: &Table) -> Result<Stations> {
    let need = |c: &str| col_of(t, c).ok_or_else(|| Error::msg(format!("the station table needs a '{c}' column")));
    let (lat, lon, elev) = (need("lat")?, need("lon")?, need("elev")?);
    let name = col_of(t, "name").ok_or_else(|| Error::msg("the station table needs a \"name\" column"))?;
    let mut s = Stations { names: Vec::new(), lat: Vec::new(), lon: Vec::new(), elev: Vec::new() };
    for r in &t.rows {
        let n = t.cell(name, r[name]);
        if s.names.contains(&n) {
            return Err(Error::msg(format!("the station table repeats the station {n:?}")));
        }
        s.names.push(n);
        s.lat.push(r[lat]);
        s.lon.push(r[lon]);
        s.elev.push(r[elev]);
    }
    Ok(s)
}

/// Python's repr of a list of strings.
pub fn py_list(v: &[String]) -> String {
    format!("[{}]", v.iter().map(|s| format!("'{s}'")).collect::<Vec<_>>().join(", "))
}

/// A table of daily series (landschaft's `_frame`): the dates in a column `date` (2001-01-31 as text, or 20010131)
/// or in `year` and `day` (of the year), a column per station, NaN for a missing day. The dates are sorted.
pub fn frame_of(t: &Table, what: &str, stations: Option<&Stations>) -> Result<Frame> {
    let date = col_of(t, "date");
    let ymd = (col_of(t, "year"), col_of(t, "day"));
    let mut days = Vec::with_capacity(t.rows.len());
    for r in &t.rows {
        let d = match (date, ymd) {
            (Some(j), _) => parse_date(&t.cell(j, r[j])),
            (None, (Some(y), Some(d))) if !r[y].is_nan() && !r[d].is_nan() => {
                Some(days_from_civil(r[y] as i64, 1, 1) + r[d] as i64 - 1)
            }
            (None, (Some(_), Some(_))) => None,
            _ => {
                return Err(Error::msg(format!(
                    "{what}: the table needs a column date (2001-01-31 or 20010131) or the columns year and day"
                )));
            }
        };
        days.push(d.ok_or_else(|| Error::msg(format!("{what}: a row has no valid date")))?);
    }
    let mut order: Vec<usize> = (0..days.len()).collect();
    order.sort_by_key(|&i| days[i]);
    if order.windows(2).any(|w| days[w[0]] == days[w[1]]) {
        return Err(Error::msg(format!("{what}: dates repeat")));
    }
    let skip = |j: usize| Some(j) == date || (date.is_none() && (Some(j) == ymd.0 || Some(j) == ymd.1));
    let cols: Vec<(String, Vec<f64>)> = (0..t.columns.len())
        .filter(|&j| !skip(j))
        .map(|j| (t.columns[j].clone(), order.iter().map(|&i| t.rows[i][j]).collect()))
        .collect();
    if let Some(st) = stations {
        let missing: Vec<String> = cols.iter().filter(|(n, _)| st.index(n).is_none()).map(|(n, _)| n.clone()).collect();
        if !missing.is_empty() {
            return Err(Error::msg(format!("{what}: stations {} are not in the station table", py_list(&missing))));
        }
    }
    Ok(Frame { days: order.iter().map(|&i| days[i]).collect(), cols })
}

// -------------------------------------------------------------------------------------------- numpy's numbers

/// numpy's pairwise summation of float64 (`DOUBLE_pairwise_sum`).
fn pairwise(a: &[f64]) -> f64 {
    let n = a.len();
    if n < 8 {
        let mut res = 0.0;
        for &x in a {
            res += x;
        }
        res
    } else if n <= 128 {
        let mut r = [a[0], a[1], a[2], a[3], a[4], a[5], a[6], a[7]];
        let mut i = 8;
        while i < n - (n % 8) {
            for j in 0..8 {
                r[j] += a[i + j];
            }
            i += 8;
        }
        let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
        while i < n {
            res += a[i];
            i += 1;
        }
        res
    } else {
        let mut n2 = n / 2;
        n2 -= n2 % 8;
        pairwise(&a[..n2]) + pairwise(&a[n2..])
    }
}

/// numpy's (and pandas') sum of a float64 array: zero plus the pairwise sum.
pub fn np_sum(a: &[f64]) -> f64 {
    0.0 + pairwise(a)
}

/// pandas' mean of values without missing ones (NaN for none).
pub fn mean(a: &[f64]) -> f64 {
    if a.is_empty() { f64::NAN } else { np_sum(a) / a.len() as f64 }
}

/// pandas' sample standard deviation (ddof 1; NaN under two values).
pub fn std1(a: &[f64]) -> f64 {
    let n = a.len();
    if n <= 1 {
        return f64::NAN;
    }
    let avg = np_sum(a) / n as f64;
    let sqr: Vec<f64> = a.iter().map(|x| (avg - x) * (avg - x)).collect();
    (np_sum(&sqr) / (n - 1) as f64).sqrt()
}

// ------------------------------------------------------------------------------------ the weather generator

/// A column of the generator not derived from daily data: a value, twelve monthly values or, by station, either.
#[derive(Debug, Clone)]
pub enum Monthly {
    All(f64),
    Twelve(Vec<f64>),
    ByStation(Vec<(String, Vec<f64>)>),
}

impl Monthly {
    fn get(&self, station: &str, month: i64) -> Result<f64> {
        match self {
            Monthly::All(v) => Ok(*v),
            Monthly::Twelve(v) => Ok(v[month as usize - 1]),
            Monthly::ByStation(s) => s
                .iter()
                .find(|(n, _)| n == station)
                .map(|(_, v)| v[month as usize - 1])
                .ok_or_else(|| Error::msg(format!("monthly: no values for the station {station:?}"))),
        }
    }
}

fn twelve(v: &serde_json::Value) -> Result<Vec<f64>> {
    let bad = || Error::msg("monthly values are a number, 12 numbers or, by station, either");
    match v {
        serde_json::Value::Number(n) => Ok(vec![n.as_f64().ok_or_else(bad)?; 12]),
        serde_json::Value::Array(a) if a.len() == 12 => {
            a.iter().map(|x| x.as_f64().ok_or_else(bad)).collect::<Result<Vec<f64>>>()
        }
        _ => Err(bad()),
    }
}

/// The `monthly` setting: `{column: value}`, a value being a number, 12 numbers or `{station: number or 12 numbers}`.
pub fn parse_monthly(v: Option<serde_json::Value>) -> Result<Vec<(String, Monthly)>> {
    let Some(v) = v else { return Ok(Vec::new()) };
    let serde_json::Value::Object(m) = v else {
        return Err(Error::msg("monthly is an object: {column: a number, 12 numbers or {station: ...}}"));
    };
    let mut out = Vec::new();
    for (k, x) in m {
        let val = match &x {
            serde_json::Value::Object(by) => {
                let mut s = Vec::new();
                for (st, y) in by {
                    s.push((st.clone(), twelve(y)?));
                }
                Monthly::ByStation(s)
            }
            serde_json::Value::Number(n) => Monthly::All(n.as_f64().unwrap_or(f64::NAN)),
            other => Monthly::Twelve(twelve(other)?),
        };
        out.push((k, val));
    }
    Ok(out)
}

/// One station and month of the generator: the `WGN_COLUMNS` and the years of rain record.
#[derive(Debug, Clone)]
pub struct WgnRow {
    pub station: String,
    pub month: i64,
    pub v: [f64; 14],
    pub rain_yrs: f64,
}

fn dropna(days: &[i64], v: &[f64]) -> Vec<(i64, f64)> {
    days.iter().zip(v).filter(|(_, x)| !x.is_nan()).map(|(d, x)| (*d, *x)).collect()
}

/// The values of a series by calendar month (missing ones left out).
fn by_month(s: &[(i64, f64)]) -> [Vec<f64>; 12] {
    let mut out: [Vec<f64>; 12] = Default::default();
    for (d, x) in s {
        out[civil_from_days(*d).1 as usize - 1].push(*x);
    }
    out
}

/// The dew point (Magnus' formula at the mean temperature) of each day with a humidity and both temperatures.
fn dew_series(hmd: &Frame, st: &str, tmax: &Frame, tmin: &Frame) -> Result<Vec<(i64, f64)>> {
    let rh = hmd.col(st).ok_or_else(|| Error::msg(format!("hmd: no station {st:?}")))?;
    let tx = tmax.col(st).ok_or_else(|| Error::msg(format!("the dew point of {st:?} needs its tmax")))?;
    let tn = tmin.col(st).ok_or_else(|| Error::msg(format!("the dew point of {st:?} needs its tmin")))?;
    let mut out = Vec::new();
    for (k, &d) in hmd.days.iter().enumerate() {
        if rh[k].is_nan() {
            continue;
        }
        let (Ok(i), Ok(j)) = (tmax.days.binary_search(&d), tmin.days.binary_search(&d)) else { continue };
        let tav = (tx[i] + tn[j]) / 2.0;
        let g = rh[k].max(1e-6).ln() + 17.27 * tav / (237.3 + tav);
        let dew = 237.3 * g / (17.27 - g);
        if !dew.is_nan() {
            out.push((d, dew));
        }
    }
    Ok(out)
}

/// SWAT's weather generator statistics (WXGEN) of each station from daily series (landschaft's
/// `wgn_from_series`): by calendar month over all years, the means and standard deviations of the maximum and
/// minimum temperature, the mean monthly precipitation, the standard deviation and skew of the daily
/// precipitation, the probabilities of a wet day after a dry and a wet day, the mean number of wet days and the
/// mean daily solar radiation, dew point and wind speed when the series are given; `monthly` gives or replaces
/// columns.
#[allow(clippy::too_many_arguments)]
pub fn wgn_from_series(
    pcp: &Frame,
    tmax: &Frame,
    tmin: &Frame,
    slr: Option<&Frame>,
    hmd: Option<&Frame>,
    wnd: Option<&Frame>,
    dewpoint: Option<&Frame>,
    monthly: &[(String, Monthly)],
) -> Result<Vec<WgnRow>> {
    let mut rows: Vec<WgnRow> = Vec::new();
    for (st, pv) in &pcp.cols {
        let p = dropna(&pcp.days, pv);
        let tx = tmax.col(st).map(|c| dropna(&tmax.days, c)).unwrap_or_default();
        let tn = tmin.col(st).map(|c| dropna(&tmin.days, c)).unwrap_or_default();
        let (txm, tnm) = (by_month(&tx), by_month(&tn));
        let ymd: Vec<(i64, i64)> = p
            .iter()
            .map(|(d, _)| {
                let (y, m, _) = civil_from_days(*d);
                (y, m)
            })
            .collect();
        let years_of = |ks: &[usize]| {
            let mut ys: Vec<i64> = ks.iter().map(|&k| ymd[k].0).collect();
            ys.sort_unstable();
            ys.dedup();
            ys.len()
        };
        let rain_yrs = years_of(&(0..p.len()).collect::<Vec<_>>()) as f64;
        // the series of the other variables: their monthly values, when this station has them
        let mut extra: Vec<(usize, [Vec<f64>; 12])> = Vec::new();
        for (col, series) in [(11, slr), (13, wnd), (12, dewpoint)] {
            if let Some(c) = series.and_then(|f| f.col(st).map(|c| (f, c))) {
                extra.push((col, by_month(&dropna(&c.0.days, c.1))));
            }
        }
        if !extra.iter().any(|(c, _)| *c == 12)
            && let Some(h) = hmd.filter(|h| h.col(st).is_some())
        {
            extra.push((12, by_month(&dew_series(h, st, tmax, tmin)?)));
        }
        for m in 1..=12i64 {
            let ks: Vec<usize> = (0..p.len()).filter(|&k| ymd[k].1 == m).collect();
            let pm: Vec<f64> = ks.iter().map(|&k| p[k].1).collect();
            let mut v = [f64::NAN; 14];
            let mi = m as usize - 1;
            v[0] = mean(&txm[mi]);
            v[2] = std1(&txm[mi]);
            v[1] = mean(&tnm[mi]);
            v[3] = std1(&tnm[mi]);
            let years = years_of(&ks).max(1) as f64;
            v[4] = np_sum(&pm) / years;
            let sd = std1(&pm);
            v[5] = sd;
            let n = pm.len();
            v[6] = if n > 2 && sd > 0.0 {
                let mu = mean(&pm);
                let cubes: Vec<f64> = pm.iter().map(|x| npmath::pow((x - mu) / sd, 3.0)).collect();
                n as f64 * np_sum(&cubes) / ((n - 1) * (n - 2)) as f64
            } else {
                0.0
            };
            // wet days after dry and wet days: only the days whose previous day has a value
            let (mut dry_days, mut wet_days, mut wet_after_dry, mut wet_after_wet) = (0usize, 0usize, 0usize, 0usize);
            for &k in &ks {
                if k > 0 && p[k - 1].0 == p[k].0 - 1 {
                    let (was_wet, wet) = (p[k - 1].1 > 0.0, p[k].1 > 0.0);
                    if was_wet {
                        wet_days += 1;
                        wet_after_wet += usize::from(wet);
                    } else {
                        dry_days += 1;
                        wet_after_dry += usize::from(wet);
                    }
                }
            }
            v[7] = if dry_days > 0 { wet_after_dry as f64 / dry_days as f64 } else { 0.0 };
            v[8] = if wet_days > 0 { wet_after_wet as f64 / wet_days as f64 } else { 0.0 };
            v[9] = pm.iter().filter(|x| **x > 0.0).count() as f64 / years;
            for (col, by) in &extra {
                v[*col] = mean(&by[mi]);
            }
            rows.push(WgnRow { station: st.clone(), month: m, v, rain_yrs });
        }
    }
    for (col, val) in monthly {
        let Some(c) = WGN_COLUMNS.iter().position(|w| w == col) else {
            return Err(Error::msg(format!("unknown weather generator column '{col}'")));
        };
        for r in &mut rows {
            r.v[c] = val.get(&r.station, r.month)?;
        }
    }
    let missing: Vec<String> = WGN_COLUMNS
        .iter()
        .enumerate()
        .filter(|(c, _)| rows.iter().any(|r| r.v[*c].is_nan()))
        .map(|(_, n)| n.to_string())
        .collect();
    if !missing.is_empty() || rows.is_empty() {
        return Err(Error::msg(format!(
            "the weather generator needs {}: give their daily series or monthly=",
            py_list(&missing)
        )));
    }
    Ok(rows)
}

/// The generator's statistics as a table: station, month, the `WGN_COLUMNS` and `rain_yrs`.
pub fn wgn_table(rows: &[WgnRow]) -> Table {
    let mut columns = vec!["station".to_string(), "month".to_string()];
    columns.extend(WGN_COLUMNS.iter().map(|c| c.to_string()));
    columns.push("rain_yrs".into());
    let mut t = Table::new(columns);
    let mut names: Vec<String> = Vec::new();
    for r in rows {
        let k = names.iter().position(|n| *n == r.station).unwrap_or_else(|| {
            names.push(r.station.clone());
            names.len() - 1
        });
        let mut row = vec![(k + 1) as f64, r.month as f64];
        row.extend_from_slice(&r.v);
        row.push(r.rain_yrs);
        t.rows.push(row);
    }
    if !names.is_empty() {
        t.labels.insert("station".into(), names);
    }
    t
}

/// The generator's statistics from a table as `wgn_table` gives it (the SWAT+ WGN database's, by station and month).
pub fn wgn_rows_of(t: &Table) -> Result<Vec<WgnRow>> {
    let need = |c: &str| col_of(t, c).ok_or_else(|| Error::msg(format!("the wgn table needs a column {c:?}")));
    let (st, mo, ry) = (need("station")?, need("month")?, need("rain_yrs")?);
    let cols: Vec<usize> = WGN_COLUMNS.iter().map(|c| need(c)).collect::<Result<_>>()?;
    Ok(t.rows
        .iter()
        .map(|r| {
            let mut v = [0.0; 14];
            for (k, j) in cols.iter().enumerate() {
                v[k] = r[*j];
            }
            WgnRow { station: t.cell(st, r[st]), month: r[mo] as i64, v, rain_yrs: r[ry] }
        })
        .collect())
}

// ------------------------------------------------------------------------------------------------ the climate

/// The weather of a SWAT+ project (landschaft's `SwatClimate`).
#[derive(Debug, Clone)]
pub struct Climate {
    pub stations: Stations,
    /// pcp, tmax, tmin, slr, hmd, wnd (the ones given).
    pub series: Vec<(&'static str, Frame)>,
    pub wgn: Vec<WgnRow>,
}

/// A weather file written: its name, kind, coordinates as the file has them and its first and last days.
#[derive(Debug, Clone)]
pub struct Record {
    pub fname: String,
    pub ext: &'static str,
    pub lat: f64,
    pub lon: f64,
    pub first: i64,
    pub last: i64,
}

/// A weather generator station as the editor stores it.
#[derive(Debug, Clone)]
pub struct WgnStation {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
    pub elev: f64,
    pub rain_yrs: i64,
    pub months: Vec<(i64, [f64; 14])>,
}

/// The weather of a project from tables (landschaft's `climate`).
pub fn climate(
    stations: &Table,
    series: &[(&'static str, Option<&Table>)],
    wgn: Option<&Table>,
    monthly: &[(String, Monthly)],
) -> Result<Climate> {
    let st = stations_of(stations)?;
    let mut frames: Vec<(&'static str, Frame)> = Vec::new();
    for (name, t) in series {
        if let Some(t) = t {
            frames.push((name, frame_of(t, name, Some(&st))?));
        }
    }
    let get = |n: &str| frames.iter().find(|(k, _)| *k == n).map(|(_, f)| f);
    let wgn = match wgn {
        Some(t) => wgn_rows_of(t)?,
        None => {
            let need = |n: &str| get(n).ok_or_else(|| Error::msg(format!("the weather needs the series {n}")));
            wgn_from_series(
                need("pcp")?,
                need("tmax")?,
                need("tmin")?,
                get("slr"),
                get("hmd"),
                get("wnd"),
                None,
                monthly,
            )?
        }
    };
    Ok(Climate { stations: st, series: frames, wgn })
}

/// A name as a file name's stem.
pub fn file_stem(name: &str) -> String {
    name.chars().map(|c| if c.is_alphanumeric() || c == '_' || c == '.' { c } else { '_' }).collect()
}

fn rjust(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w { s.to_string() } else { " ".repeat(w - n) + s }
}

/// The weather files (name, text) and the record of each data file.
pub type WeatherFiles = (Vec<(String, String)>, Vec<Record>);

impl Climate {
    fn frame(&self, name: &str) -> Option<&Frame> {
        self.series.iter().find(|(k, _)| *k == name).map(|(_, f)| f)
    }

    /// The weather files (name, text) in the order written, and one record per data file.
    pub fn files(&self) -> Result<WeatherFiles> {
        let kinds: [(&'static str, &str, &[&str]); 5] = [
            ("pcp", "Precipitation", &["pcp"]),
            ("tmp", "Temperature", &["tmax", "tmin"]),
            ("slr", "Solar radiation", &["slr"]),
            ("hmd", "Relative humidity", &["hmd"]),
            ("wnd", "Wind speed", &["wnd"]),
        ];
        let mut out: Vec<(String, String)> = Vec::new();
        let mut put = |name: String, text: String| match out.iter_mut().find(|(n, _)| *n == name) {
            Some(e) => e.1 = text,
            None => out.push((name, text)),
        };
        let mut records = Vec::new();
        for (ext, desc, cols) in kinds {
            let Some(frames) = cols.iter().map(|c| self.frame(c)).collect::<Option<Vec<&Frame>>>() else { continue };
            let names: Vec<usize> = (0..self.stations.names.len())
                .filter(|&i| frames.iter().all(|f| f.col(&self.stations.names[i]).is_some()))
                .collect();
            if names.is_empty() {
                continue;
            }
            let mut listing =
                vec![format!("{ext}.cli: {desc} file names - file written by landschaft"), "filename".to_string()];
            for i in names {
                let st = &self.stations.names[i];
                let fname = format!("{}.{ext}", file_stem(st));
                listing.push(fname.clone());
                let first = frames.iter().filter_map(|f| f.days.first()).min().copied();
                let last = frames.iter().filter_map(|f| f.days.last()).max().copied();
                let (Some(first), Some(last)) = (first, last) else {
                    return Err(Error::msg(format!("the series of {st:?} have no dates")));
                };
                let (lat3, lon3, elev3) = (
                    format!("{:.3}", self.stations.lat[i]),
                    format!("{:.3}", self.stations.lon[i]),
                    format!("{:.3}", self.stations.elev[i]),
                );
                let (y0, y1) = (civil_from_days(first).0, civil_from_days(last).0);
                let mut lines = vec![
                    format!("{fname}: {desc} data - file written by landschaft"),
                    rjust("nbyr", 4) + &rjust("tstep", 10) + &rjust("lat", 10) + &rjust("lon", 10) + &rjust("elev", 10),
                    rjust(&(y1 - y0 + 1).to_string(), 4)
                        + &rjust("0", 10)
                        + &rjust(&lat3, 10)
                        + &rjust(&lon3, 10)
                        + &rjust(&elev3, 10),
                ];
                let vals: Vec<(&Frame, &[f64])> = frames.iter().map(|f| (*f, f.col(st).unwrap())).collect();
                for d in first..=last {
                    let (y, _, _) = civil_from_days(d);
                    let mut line = format!("{y}{} ", rjust(&day_of_year(d).to_string(), 5));
                    for (f, c) in &vals {
                        let x = f.days.binary_search(&d).map(|k| c[k]).unwrap_or(f64::NAN);
                        let x = if x.is_nan() { -99.0 } else { x };
                        line += &rjust(&format!("{x:.5}"), 10);
                    }
                    lines.push(line);
                }
                put(fname.clone(), lines.join("\n") + "\n");
                // the editor reads the coordinates back from the file's third line
                records.push(Record {
                    fname,
                    ext,
                    lat: lat3.parse().unwrap_or(f64::NAN),
                    lon: lon3.parse().unwrap_or(f64::NAN),
                    first,
                    last,
                });
            }
            put(format!("{ext}.cli"), listing.join("\n") + "\n");
        }
        Ok((out, records))
    }

    /// The weather generator stations as the editor stores them (one per station).
    pub fn wgn_records(&self) -> Result<Vec<WgnStation>> {
        let mut names: Vec<&str> = Vec::new();
        for r in &self.wgn {
            if !names.contains(&r.station.as_str()) {
                names.push(&r.station);
            }
        }
        names
            .into_iter()
            .map(|st| {
                let i = self
                    .stations
                    .index(st)
                    .ok_or_else(|| Error::msg(format!("the generator's station {st:?} is not in the station table")))?;
                let mut months: Vec<(i64, [f64; 14])> =
                    self.wgn.iter().filter(|r| r.station == st).map(|r| (r.month, r.v)).collect();
                months.sort_by_key(|m| m.0);
                let first = self.wgn.iter().find(|r| r.station == st).unwrap();
                // (the first row of the station in month order carries the years, as pandas' sorted frame does)
                let rain =
                    self.wgn.iter().filter(|r| r.station == st).min_by_key(|r| r.month).unwrap_or(first).rain_yrs;
                Ok(WgnStation {
                    name: format!("wgn_{}", file_stem(st)),
                    lat: self.stations.lat[i],
                    lon: self.stations.lon[i],
                    elev: self.stations.elev[i],
                    rain_yrs: rain as i64,
                    months,
                })
            })
            .collect()
    }

    /// The days every series covers (the SWAT+ Editor's default simulation period): the latest first day and the
    /// earliest last day of the weather files.
    pub fn period(&self, records: &[Record]) -> Result<(i64, i64)> {
        let start = records.iter().map(|r| r.first).max();
        let end = records.iter().map(|r| r.last).min();
        match (start, end) {
            (Some(s), Some(e)) => Ok((s, e)),
            _ => Err(Error::msg(
                "the stations of the tables have no weather file (a series needs its station in the station table)",
            )),
        }
    }
}

/// time.sim from a period, as the editor sets it (day 0: the year's first or last day).
pub fn time_sim(start: i64, end: i64) -> [i64; 4] {
    let (sy, _, _) = civil_from_days(start);
    let (ey, em, ed) = civil_from_days(end);
    let sd = day_of_year(start);
    [if sd > 1 { sd } else { 0 }, sy, if em == 12 && ed == 31 { 0 } else { day_of_year(end) }, ey]
}

// ---------------------------------------------------------------------------------------------- the operator

struct WgnOp;

fn table_in<'a>(args: &'a Args, port: &str) -> Result<&'a Table> {
    match args.get(port) {
        Some(Value::Table(t)) => Ok(t),
        Some(v) => Err(Error::msg(format!("{port} is a {:?}, not a table", v.ty()))),
        None => Err(Error::msg(format!("connect the table {port}"))),
    }
}

fn opt_table<'a>(args: &'a Args, port: &str) -> Result<Option<&'a Table>> {
    match args.get(port) {
        None => Ok(None),
        Some(_) => table_in(args, port).map(Some),
    }
}

impl Operator for WgnOp {
    fn signature(&self) -> OpSignature {
        let series = |name: &str, what: &str| PortSpec::new(name, Type::Table, what);
        OpSignature::new(
            "swat.wgn",
            "SWAT's weather generator statistics (WXGEN, landschaft's wgn_from_series, bit for bit with numpy's \
             sums) of each station from its daily series: by calendar month over all years, the means and standard \
             deviations of the maximum and minimum temperature, the mean monthly precipitation (the month's total \
             averaged over the years with data), the standard deviation and skew of the daily precipitation, the \
             probabilities of a wet day after a dry and after a wet day, the mean number of wet days, and the mean \
             daily solar radiation, dew point and wind speed when their series are given (the dew point from the \
             humidity by Magnus' formula at the mean temperature). What is not derived from daily data (pcp_hhr, the \
             largest half-hour rainfall of each month, at least) is given in monthly. A series is a table of one row \
             per day: a column date (2001-01-31 or 20010131) or year and day (of the year), and a column per \
             station, empty for a missing day.",
            Access::None,
        )
        .input(series("pcp", "daily precipitation (mm)"))
        .input(series("tmax", "daily maximum temperature (deg C)"))
        .input(series("tmin", "daily minimum temperature (deg C)"))
        .input(PortSpec::optional("slr", Type::Table, "daily solar radiation (MJ/m2)"))
        .input(PortSpec::optional("hmd", Type::Table, "daily relative humidity (fraction)"))
        .input(PortSpec::optional("wnd", Type::Table, "daily wind speed (m/s)"))
        .input(PortSpec::optional("dewpoint", Type::Table, "daily dew point (deg C), instead of the humidity's"))
        .output(PortSpec::new(
            "wgn",
            Type::Table,
            "the statistics: station, month and the columns tmp_max_ave, tmp_min_ave, tmp_max_sd, tmp_min_sd, \
             pcp_ave, pcp_sd, pcp_skew, wet_dry, wet_wet, pcp_days, pcp_hhr, slr_ave, dew_ave, wnd_ave, rain_yrs",
        ))
        .param(PortSpec::optional(
            "monthly",
            Type::Any,
            "values not derived from daily data, by column: {\"pcp_hhr\": 8, \"slr_ave\": [4, 7, 11, ...12 values], \
             \"dew_ave\": {\"station\": 4}}",
        ))
    }

    fn run(&self, _: &mut Ctx, args: &Args) -> Result<Outputs> {
        (|| -> Result<Outputs> {
            let frame =
                |p: &str| -> Result<Option<Frame>> { opt_table(args, p)?.map(|t| frame_of(t, p, None)).transpose() };
            let need = |p: &str| frame(p)?.ok_or_else(|| Error::msg(format!("connect the table {p}")));
            let (pcp, tmax, tmin) = (need("pcp")?, need("tmax")?, need("tmin")?);
            let (slr, hmd, wnd, dew) = (frame("slr")?, frame("hmd")?, frame("wnd")?, frame("dewpoint")?);
            let monthly = parse_monthly(args.opt_param("monthly")?)?;
            let rows =
                wgn_from_series(&pcp, &tmax, &tmin, slr.as_ref(), hmd.as_ref(), wnd.as_ref(), dew.as_ref(), &monthly)?;
            let mut o = Outputs::new();
            o.insert("wgn".into(), Value::Table(wgn_table(&rows)));
            Ok(o)
        })()
        .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        for d in [-1000, 0, 365, 11016, 11017, 20000] {
            let (y, m, dd) = civil_from_days(d);
            assert_eq!(days_from_civil(y, m, dd), d);
        }
        assert_eq!(civil_from_days(days_from_civil(2000, 2, 29)), (2000, 2, 29));
        assert_eq!(day_of_year(days_from_civil(2001, 12, 31)), 365);
        assert_eq!(day_of_year(days_from_civil(2000, 12, 31)), 366);
        assert_eq!(parse_date("2001-03-05 12:00"), Some(days_from_civil(2001, 3, 5)));
        assert_eq!(parse_date("20010305"), Some(days_from_civil(2001, 3, 5)));
        assert_eq!(parse_date("2001-02-30"), None);
    }

    #[test]
    fn the_sum_is_numpys_pairwise() {
        // 0.1 added ten thousand times: numpy's pairwise sum, not the running one
        let a = vec![0.1; 10000];
        let running: f64 = a.iter().sum();
        assert_ne!(np_sum(&a), running);
        assert_eq!(np_sum(&[1.0, 2.0, 3.0]), 6.0);
    }
}
