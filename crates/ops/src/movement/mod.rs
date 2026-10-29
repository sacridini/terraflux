//! Movement ecology (`move.*`), landschaft's `ls.move`: the tracks of tagged animals as points, one feature per
//! location, ordered by track and time, with a column of the time and one of the track. The first part is R's
//! move2 0.5.0 (Kranstauber et al. 2024), the same numbers: Movebank's files, the distance, duration, speed,
//! direction and turn of every step (on the sphere of S2 and the ellipsoid of GeographicLib in longitude /
//! latitude), duplicates, one location per interval and Movebank's outliers, locations interpolated in time,
//! tracks as lines and segments, a local projection, Brownian tracks. The second is amt 0.3.1's (Signer et al.
//! 2019) steps of step selection functions: tracks resampled in bursts, the steps with their lengths and turn
//! angles, summaries of tracks, day and night, distributions of steps, random steps and the covariates at their
//! ends. The kernels are landschaft's C++ (`terraflux-ls-sys`), the work around them its Python's.
//!
//! The time is a column of datetimes written as text (ISO 8601, UTC: Movebank's `timestamp`, GDAL's datetime
//! fields) or of numbers (seconds, or any unit); the track a column of names or numbers. Both are found by their
//! usual names (`timestamp`, `time`... and `individual-local-identifier`, `track`, `id`...) or named in each node
//! (`time`, `track`).

mod amt;
mod filters;
mod geometry;
mod ssf;
pub mod time;
mod tracks;

use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::vector::attribute_table;
use crate::{Args, Error, Outputs, Registry, Result, Value, outputs};

pub use ssf::Distribution;

pub fn register(r: &mut Registry) {
    tracks::register(r);
    filters::register(r);
    geometry::register(r);
    amt::register(r);
    ssf::register(r);
}

const TIME_NAMES: [&str; 5] = ["timestamp", "time", "datetime", "date_time", "t"];
const TRACK_NAMES: [&str; 8] = [
    "individual-tag-local-identifier",
    "individual-local-identifier",
    "individual_local_identifier",
    "track",
    "track_id",
    "id",
    "individual",
    "animal",
];
/// The column of the bursts of `move.resample`.
pub const BURST: &str = "burst_";

pub(crate) fn fail(node: &str, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {node:?}: {m}"))
}

/// The settings `time` and `track` every operator takes.
pub(crate) fn column_params(s: OpSignature) -> OpSignature {
    s.param(PortSpec::optional(
        "time",
        Type::Text,
        "the column of the time (default: timestamp, time, datetime, date_time or t)",
    ))
    .param(PortSpec::optional(
        "track",
        Type::Text,
        "the column of the track (default: individual-tag-local-identifier, individual-local-identifier, track, \
         track_id, id, individual or animal)",
    ))
}

fn find(v: &Vector, given: Option<String>, names: &[&str], what: &str) -> std::result::Result<String, String> {
    if let Some(g) = given.filter(|g| !g.is_empty()) {
        return if v.column(&g).is_some() { Ok(g) } else { Err(format!("{what} column {g:?} not found")) };
    }
    names.iter().find(|n| v.column(n).is_some()).map(|n| n.to_string()).ok_or_else(|| {
        format!("which column holds the {what}? Give the setting {what}, or name it one of {}", names[..4].join(", "))
    })
}

/// The (time, track) columns of tracks: the settings, or the usual names.
pub(crate) fn columns(args: &Args, v: &Vector) -> Result<(String, String)> {
    let t = find(v, args.opt_param("time")?, &TIME_NAMES, "time").map_err(|e| fail(args.node, e))?;
    let k = find(v, args.opt_param("track")?, &TRACK_NAMES, "track").map_err(|e| fail(args.node, e))?;
    Ok((t, k))
}

/// The track column only (steps, bursts).
pub(crate) fn track_column(args: &Args, v: &Vector) -> Result<String> {
    find(v, args.opt_param("track")?, &TRACK_NAMES, "track").map_err(|e| fail(args.node, e))
}

/// A column's times: datetimes (nanoseconds since 1970, None missing) or numbers.
#[derive(Debug, Clone)]
pub(crate) enum Times {
    Instants(Vec<Option<i64>>),
    Numbers(Vec<f64>),
}

impl Times {
    pub fn of(c: &Column) -> std::result::Result<Times, String> {
        match c {
            Column::Numbers(x) => Ok(Times::Numbers(x.clone())),
            Column::Texts(x) => {
                let mut out = Vec::with_capacity(x.len());
                for s in x {
                    if s.trim().is_empty() {
                        out.push(None);
                    } else {
                        out.push(Some(
                            time::parse(s).ok_or_else(|| format!("the time {s:?} is not a datetime (ISO 8601)"))?,
                        ));
                    }
                }
                Ok(Times::Instants(out))
            }
        }
    }

    /// The times in seconds (pandas: the nanoseconds as a float over 1e9).
    pub fn seconds(&self) -> Vec<f64> {
        match self {
            Times::Numbers(x) => x.clone(),
            Times::Instants(x) => x.iter().map(|t| t.map_or(f64::NAN, |t| t as f64 / 1e9)).collect(),
        }
    }

    /// From location i to location j, in seconds (datetimes: the exact difference of their nanoseconds).
    pub fn lag(&self, i: usize, j: usize) -> f64 {
        match self {
            Times::Numbers(x) => x[j] - x[i],
            Times::Instants(x) => match (x[i], x[j]) {
                (Some(a), Some(b)) => (b - a) as f64 / 1e9,
                _ => f64::NAN,
            },
        }
    }
}

/// A key of a value of a column, ordered as pandas sorts them (missing last).
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Key {
    Num(f64),
    Text(String),
    Na,
}

impl Key {
    pub fn of(c: &Column, i: usize) -> Key {
        match c {
            Column::Numbers(x) if x[i].is_nan() => Key::Na,
            Column::Numbers(x) => Key::Num(if x[i] == 0.0 { 0.0 } else { x[i] }),
            Column::Texts(x) if x[i].is_empty() => Key::Na,
            Column::Texts(x) => Key::Text(x[i].clone()),
        }
    }

    pub fn cmp(&self, o: &Key) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (self, o) {
            (Key::Num(a), Key::Num(b)) => a.total_cmp(b),
            (Key::Text(a), Key::Text(b)) => a.cmp(b),
            (Key::Na, Key::Na) => Equal,
            (Key::Na, _) => Greater,
            (_, Key::Na) => Less,
            (Key::Num(_), Key::Text(_)) => Less,
            (Key::Text(_), Key::Num(_)) => Greater,
        }
    }

    fn hash_key(&self) -> String {
        match self {
            Key::Num(v) => format!("n{}", v.to_bits()),
            Key::Text(s) => format!("t{s}"),
            Key::Na => "na".into(),
        }
    }
}

/// pandas' factorize of a column (use_na_sentinel=False): a code per value in the order they first appear.
pub(crate) fn codes(c: &Column) -> Vec<i64> {
    let mut seen: HashMap<String, i64> = HashMap::new();
    (0..c.len())
        .map(|i| {
            let n = seen.len() as i64;
            *seen.entry(Key::of(c, i).hash_key()).or_insert(n)
        })
        .collect()
}

/// The locations of validated tracks, as arrays.
pub(crate) struct Locations {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub code: Vec<i64>,
    pub times: Times,
    pub lonlat: bool,
    pub a: f64,
    pub inv_flattening: f64,
    pub time_col: String,
    pub track_col: String,
}

/// Whether a coordinate system is longitude / latitude (none: no).
pub(crate) fn is_lonlat(crs: Option<&str>) -> std::result::Result<bool, String> {
    match crs {
        None => Ok(false),
        Some(c) => terraflux_gdal::is_geographic(c),
    }
}

/// The locations must be consecutive per track and their times must not decrease within a track.
pub(crate) fn check_order(code: &[i64], seconds: Option<&[f64]>, strictly: bool) -> std::result::Result<(), String> {
    if code.windows(2).any(|w| w[1] < w[0] || w[1] > w[0] + 1) {
        return Err("the locations of each track must be consecutive: sort them by track and time first \
                    (move.as_tracks)"
            .into());
    }
    if let Some(s) = seconds {
        for i in 1..code.len() {
            if code[i] == code[i - 1] {
                let d = s[i] - s[i - 1];
                if d < 0.0 {
                    return Err("the times must not decrease within a track: sort them by track and time first \
                                (move.as_tracks)"
                        .into());
                }
                if strictly && d == 0.0 {
                    return Err("some locations of a track share their time (speeds would divide by 0): drop them \
                                with move.filter_unique"
                        .into());
                }
            }
        }
    }
    Ok(())
}

/// The tracks of a port, validated (points, none empty, ordered by track and time).
pub(crate) fn locations(args: &Args, v: &Vector, strictly: bool) -> Result<Locations> {
    let node = args.node;
    let (t, k) = columns(args, v)?;
    let mut x = Vec::with_capacity(v.len());
    let mut y = Vec::with_capacity(v.len());
    for g in &v.geometries {
        match g {
            Geometry::Point(p) => {
                x.push(p[0]);
                y.push(p[1]);
            }
            Geometry::Empty => {
                return Err(fail(node, "some locations are empty: drop them first (vector.filter)"));
            }
            _ => return Err(fail(node, "the geometry must be points")),
        }
    }
    let code = codes(v.column(&k).unwrap());
    let times = Times::of(v.column(&t).unwrap()).map_err(|e| fail(node, e))?;
    check_order(&code, Some(&times.seconds()), strictly).map_err(|e| fail(node, e))?;
    let lonlat = is_lonlat(v.crs.as_deref()).map_err(|e| fail(node, e))?;
    let (mut a, mut rf) = (6378137.0, 298.257223563);
    if lonlat {
        (a, rf) = terraflux_gdal::ellipsoid(v.crs.as_deref().unwrap()).map_err(|e| fail(node, e))?;
    }
    Ok(Locations { x, y, code, times, lonlat, a, inv_flattening: rf, time_col: t, track_col: k })
}

/// The tracks as pandas would sort them: the tracks in the order they first appear, each by time (stable).
pub(crate) fn sorted(v: &Vector, time: &str, track: &str) -> std::result::Result<Vector, String> {
    let code = codes(v.column(track).unwrap());
    let sec = Times::of(v.column(time).unwrap())?.seconds();
    let mut order: Vec<usize> = (0..v.len()).collect();
    // numpy's lexsort (stable), a missing time last
    order.sort_by(|&i, &j| code[i].cmp(&code[j]).then_with(|| nan_last(sec[i], sec[j])));
    Ok(v.select(&order))
}

/// Numbers in order, NaN last.
pub(crate) fn nan_last(a: f64, b: f64) -> std::cmp::Ordering {
    use std::cmp::Ordering::*;
    match (a.is_nan(), b.is_nan()) {
        (true, true) => Equal,
        (true, false) => Greater,
        (false, true) => Less,
        _ => a.partial_cmp(&b).unwrap(),
    }
}

/// Divide metres (or the CRS's units) by this for `units` (m, km).
pub(crate) fn length_factor(units: Option<&str>, crs: Option<&str>, lonlat: bool) -> std::result::Result<f64, String> {
    let Some(u) = units else { return Ok(1.0) };
    let f = match u {
        "m" => 1.0,
        "km" => 1000.0,
        _ => return Err("units: m or km".into()),
    };
    if lonlat {
        return Ok(f);
    }
    let unit = crs_unit(crs)?.ok_or("the tracks have no coordinate system: their distances have no units")?;
    Ok(f / unit)
}

fn crs_unit(crs: Option<&str>) -> std::result::Result<Option<f64>, String> {
    match crs {
        None => Ok(None),
        Some(c) => terraflux_gdal::linear_units(c).map(Some),
    }
}

pub(crate) fn duration_factor(units: Option<&str>) -> std::result::Result<f64, String> {
    Ok(match units {
        None | Some("s") => 1.0,
        Some("min") => 60.0,
        Some("h") => 3600.0,
        Some("d") => 86400.0,
        _ => return Err("units: s, min, h or d".into()),
    })
}

pub(crate) fn speed_factor(
    units: Option<&str>,
    crs: Option<&str>,
    lonlat: bool,
) -> std::result::Result<(f64, f64), String> {
    let Some(u) = units else { return Ok((1.0, 1.0)) };
    let (mut length, duration) = match u {
        "m/s" => (1.0, 1.0),
        "km/h" => (1000.0, 3600.0),
        "m/h" => (1.0, 3600.0),
        "km/d" => (1000.0, 86400.0),
        _ => return Err("units: m/s, km/h, m/h or km/d".into()),
    };
    if !lonlat {
        let unit = crs_unit(crs)?.ok_or("the tracks have no coordinate system: their speeds have no units")?;
        length /= unit;
    }
    Ok((length, duration))
}

/// The tracks of an input.
pub(crate) fn tracks_of<'a>(args: &'a Args, port: &str) -> Result<&'a Arc<Vector>> {
    crate::vector::vector_of(args, port)
}

/// Tracks (or steps) given out: the features and their attribute table.
pub(crate) fn give(port: &str, v: Vector) -> Outputs {
    let t = attribute_table(&v);
    outputs([(port, Value::Vector(Arc::new(v))), ("table", Value::Table(t))])
}

/// A column set (replaced, or added at the end).
pub(crate) fn set_column(v: &mut Vector, name: &str, c: Column) {
    match v.attributes.iter_mut().find(|(n, _)| n == name) {
        Some((_, old)) => *old = c,
        None => v.attributes.push((name.to_string(), c)),
    }
}

/// The values of a column at rows (missing past the end: NaN or "").
pub(crate) fn take(c: &Column, rows: &[usize]) -> Column {
    match c {
        Column::Numbers(x) => Column::Numbers(rows.iter().map(|&i| x.get(i).copied().unwrap_or(f64::NAN)).collect()),
        Column::Texts(x) => Column::Texts(rows.iter().map(|&i| x.get(i).cloned().unwrap_or_default()).collect()),
    }
}

/// The threads of the kernels (Terraflux's).
pub(crate) fn threads() -> i32 {
    crate::threads::current() as i32
}
