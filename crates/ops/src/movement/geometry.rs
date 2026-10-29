//! Tracks as lines and segments, the data of each track, locations interpolated in time and a local projection
//! (move2's mt_track_lines, mt_segments, mt_track_data, mt_interpolate and mt_aeqd_crs).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};

use super::{Key, Times, codes, column_params, columns, fail, give, is_lonlat, take, time, tracks_of};
use crate::{Args, Ctx, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(TrackLines));
    r.add(Arc::new(Segments));
    r.add(Arc::new(TrackData));
    r.add(Arc::new(Interpolate));
    r.add(Arc::new(Aeqd));
}

/// The tracks must be consecutive and in time order: (time, track, codes).
fn ordered(args: &Args, v: &Vector) -> Result<(String, String, Vec<i64>)> {
    let node = args.node;
    let (t, k) = columns(args, v)?;
    let code = codes(v.column(&k).unwrap());
    if code.windows(2).any(|w| w[1] < w[0] || w[1] > w[0] + 1) {
        return Err(fail(node, "the locations of each track must be consecutive: sort them by track and time first"));
    }
    let sec = Times::of(v.column(&t).unwrap()).map_err(|e| fail(node, e))?.seconds();
    if (1..code.len()).any(|i| code[i] == code[i - 1] && sec[i] - sec[i - 1] < 0.0) {
        return Err(fail(node, "the times must not decrease within a track: sort them by track and time first"));
    }
    Ok((t, k, code))
}

/// mt_track_data: per track (in the order they appear), its first row's index, and the columns (but the time and
/// the track's) whose value does not change within any track.
fn track_data(v: &Vector, time: &str, track: &str) -> (Vec<usize>, Vec<(String, Column)>) {
    let code = codes(v.column(track).unwrap());
    let ntracks = code.iter().max().map_or(0, |m| *m as usize + 1);
    let mut first = vec![usize::MAX; ntracks];
    for (i, &c) in code.iter().enumerate() {
        if first[c as usize] == usize::MAX {
            first[c as usize] = i;
        }
    }
    let keep: Vec<(String, Column)> = v
        .attributes
        .iter()
        .filter(|(n, _)| n != time && n != track)
        .filter(|(_, c)| {
            // nunique(dropna=False) <= 1 in every track
            (0..v.len()).all(|i| {
                let f = first[code[i] as usize];
                let (a, b) = (Key::of(c, i), Key::of(c, f));
                a.cmp(&b).is_eq()
            })
        })
        .map(|(n, c)| (n.clone(), take(c, &first)))
        .collect();
    (first, keep)
}

/// `move.track_lines`.
struct TrackLines;

impl Operator for TrackLines {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.track_lines",
                "Each track as a line through its locations in time order (move2's mt_track_lines): one feature \
                 per track, its id, the number of locations (n_locations) and the columns whose value does not \
                 change within a track (move.track_data's). Empty locations are left out; a track with a single \
                 location gets no line (an empty geometry).",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("lines", Type::Vector, "a line per track"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = tracks_of(args, "tracks")?;
        let ok: Vec<usize> = (0..v.len()).filter(|&i| !matches!(v.geometries[i], Geometry::Empty)).collect();
        let sub = v.select(&ok);
        if sub.geometries.iter().any(|g| !matches!(g, Geometry::Point(_))) {
            return Err(fail(args.node, "the geometry must be points"));
        }
        let (t, k, code) = ordered(args, &sub)?;
        Ok(give("lines", lines_of(&sub, &t, &k, &code)))
    }
}

/// A line per track with its id, n_locations and its data.
pub(crate) fn lines_of(sub: &Vector, t: &str, k: &str, code: &[i64]) -> Vector {
    let n = code.len();
    let starts: Vec<usize> = (0..n).filter(|&i| i == 0 || code[i] != code[i - 1]).collect();
    let ends: Vec<usize> = starts.iter().skip(1).copied().chain(std::iter::once(n)).collect();
    let xy = |i: usize| -> Xy {
        let Geometry::Point(p) = sub.geometries[i] else { unreachable!() };
        p
    };
    let geometries = starts
        .iter()
        .zip(&ends)
        .map(|(&a, &b)| if b - a >= 2 { Geometry::LineString((a..b).map(xy).collect()) } else { Geometry::Empty })
        .collect();
    let mut attributes = vec![
        (k.to_string(), take(sub.column(k).unwrap(), &starts)),
        ("n_locations".to_string(), Column::Numbers(starts.iter().zip(&ends).map(|(a, b)| (b - a) as f64).collect())),
    ];
    let (_, data) = track_data(sub, t, k);
    attributes.extend(data.into_iter().filter(|(n, _)| n != "n_locations"));
    Vector { geometries, attributes, crs: sub.crs.clone() }
}

/// `move.segments`.
struct Segments;

impl Operator for Segments {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.segments",
                "The step from each location to the next of its track as a line (move2's mt_segments): one feature \
                 per location with its attributes, a two-point line per step and the location itself (a point) at \
                 the last location of each track.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("segments", Type::Vector, "a line per step"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let (_, _, code) = ordered(args, v)?;
        let mut pts = Vec::with_capacity(v.len());
        for g in &v.geometries {
            match g {
                Geometry::Point(p) => pts.push(*p),
                Geometry::Empty => return Err(fail(node, "some locations are empty: drop them first")),
                _ => return Err(fail(node, "the geometry must be points")),
            }
        }
        let mut out = (**v).clone();
        for i in 0..v.len().saturating_sub(1) {
            if code[i + 1] == code[i] {
                out.geometries[i] = Geometry::LineString(vec![pts[i], pts[i + 1]]);
            }
        }
        Ok(give("segments", out))
    }
}

/// `move.track_data`.
struct TrackData;

impl Operator for TrackData {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.track_data",
                "One row per track (in the order they appear) with the columns whose value does not change within \
                 a track (move2's mt_track_data): the animal's, the tag's, the study's... as features without a \
                 geometry.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("tracks", Type::Vector, "a feature per track, without a geometry"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = tracks_of(args, "tracks")?;
        let (t, k) = columns(args, v)?;
        let (first, data) = track_data(v, &t, &k);
        let mut attributes = vec![(k.clone(), take(v.column(&k).unwrap(), &first))];
        attributes.extend(data);
        let out = Vector { geometries: vec![Geometry::Empty; first.len()], attributes, crs: v.crs.clone() };
        Ok(give("tracks", out))
    }
}

/// Times wanted by `move.interpolate`: a list (datetimes as text, or numbers).
fn wanted_list(args: &Args, times: &Times) -> Result<Option<Vec<f64>>> {
    let node = args.node;
    match args.params.get("times") {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(u)) => {
            let Times::Instants(x) = times else {
                return Err(fail(node, "a unit for times needs datetimes in the time column"));
            };
            let present: Vec<i64> = x.iter().flatten().copied().collect();
            let (Some(&lo), Some(&hi)) = (present.iter().min(), present.iter().max()) else { return Ok(Some(vec![])) };
            let unit = time::unit(u).map_err(|e| fail(node, e))?;
            let step = time::step(u).map_err(|e| fail(node, e))?;
            let lo = time::floor(lo, unit);
            let f = time::floor(hi, unit);
            let hi = if f == hi { f } else { time::add(f, step) };
            let mut out = Vec::new();
            let mut t = lo;
            while t <= hi {
                out.push(t as f64);
                t = time::add(t, step);
            }
            Ok(Some(out))
        }
        Some(serde_json::Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for it in items {
                out.push(match (it, times) {
                    (serde_json::Value::String(s), Times::Instants(_)) => {
                        time::parse(s).ok_or_else(|| fail(node, format!("times: {s:?} is not a datetime")))? as f64
                    }
                    (serde_json::Value::Number(n), Times::Numbers(_)) => n.as_f64().unwrap_or(f64::NAN),
                    _ => return Err(fail(node, "times: datetimes (text) for datetimes, numbers for numbers")),
                });
            }
            Ok(Some(out))
        }
        _ => Err(fail(node, "times: a list of times, or a unit (1 hour, 15 mins...)")),
    }
}

/// The time keys of a column (nanoseconds for datetimes) as numbers.
fn time_values(times: &Times) -> Vec<f64> {
    match times {
        Times::Numbers(x) => x.clone(),
        Times::Instants(x) => x.iter().map(|t| t.map_or(f64::NAN, |t| t as f64)).collect(),
    }
}

/// `move.interpolate`.
struct Interpolate;

impl Operator for Interpolate {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.interpolate",
                "Locations at given times between the recorded ones (move2's mt_interpolate). Without times, the \
                 records without a location (empty points) get one between the locations before and after them. \
                 With times (a list, or a unit such as 1 hour or 15 mins for a regular sequence from the first to \
                 the last time), a record is added for each time within the span of each track that the track does \
                 not have, and located. A location is placed on the great circle between its neighbours in \
                 proportion to the time (in longitude / latitude, and through it in a projected system, as move2 \
                 does with s2), on the straight line without a coordinate system. Gaps longer than max_time_lag are \
                 left empty. Ordered by track and time.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("tracks", Type::Vector, "the tracks with the locations interpolated"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::optional(
                "times",
                Type::Any,
                "the times: a list (datetimes as text, or numbers), or a unit (1 hour, 15 mins, day...)",
            ))
            .param(PortSpec::optional("max_time_lag", Type::Number, "gaps longer than this (seconds) left empty"))
            .param(PortSpec::optional("omit", Type::Bool, "keep only the records at the times (false)")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let (t, k, _) = ordered(args, v)?;
        if v.geometries.iter().any(|g| !matches!(g, Geometry::Point(_) | Geometry::Empty)) {
            return Err(fail(node, "the geometry must be points"));
        }
        let tcol = v.column(&t).unwrap().clone();
        let times = Times::of(&tcol).map_err(|e| fail(node, e))?;
        let wanted = wanted_list(args, &times)?;
        let mut df = (**v).clone();
        let mut target: Vec<bool>;
        if let Some(w) = &wanted {
            let tv = time_values(&times);
            let code = codes(v.column(&k).unwrap());
            let ntracks = code.iter().max().map_or(0, |m| *m as usize + 1);
            // per track, the times wanted within its span that it does not have
            let mut add_rows: Vec<(usize, f64)> = Vec::new();
            for c in 0..ntracks {
                let rows: Vec<usize> = (0..v.len()).filter(|&i| code[i] == c as i64).collect();
                let own: Vec<f64> = rows.iter().map(|&i| tv[i]).filter(|x| !x.is_nan()).collect();
                let (Some(lo), Some(hi)) = (own.iter().copied().reduce(f64::min), own.iter().copied().reduce(f64::max))
                else {
                    continue;
                };
                for &x in w {
                    if x >= lo && x <= hi && !own.contains(&x) {
                        add_rows.push((rows[0], x));
                    }
                }
            }
            if !add_rows.is_empty() {
                let n0 = df.len();
                let n = n0 + add_rows.len();
                // new records: the track and the time, nothing else
                for (name, c) in df.attributes.iter_mut() {
                    let is_track = *name == k;
                    let is_time = *name == t;
                    match c {
                        Column::Numbers(x) => {
                            for &(r, tx) in &add_rows {
                                x.push(if is_track {
                                    x[r]
                                } else if is_time {
                                    tx
                                } else {
                                    f64::NAN
                                });
                            }
                        }
                        Column::Texts(x) => {
                            for &(r, tx) in &add_rows {
                                x.push(if is_track {
                                    x[r].clone()
                                } else if is_time {
                                    time::format(tx as i64)
                                } else {
                                    String::new()
                                });
                            }
                        }
                    }
                }
                df.geometries.resize(n, Geometry::Empty);
                // the tracks in the order they appear, each by time (stable)
                let code2 = codes(df.column(&k).unwrap());
                let sec = Times::of(df.column(&t).unwrap()).map_err(|e| fail(node, e))?.seconds();
                let mut order: Vec<usize> = (0..n).collect();
                order.sort_by(|&i, &j| code2[i].cmp(&code2[j]).then_with(|| super::nan_last(sec[i], sec[j])));
                df = df.select(&order);
            }
            let tv2 = time_values(&Times::of(df.column(&t).unwrap()).map_err(|e| fail(node, e))?);
            target =
                (0..df.len()).map(|i| w.contains(&tv2[i]) && matches!(df.geometries[i], Geometry::Empty)).collect();
        } else {
            target = vec![true; df.len()];
        }
        let n = df.len();
        let times2 = Times::of(df.column(&t).unwrap()).map_err(|e| fail(node, e))?;
        let sec = times2.seconds();
        let code = codes(df.column(&k).unwrap());
        let empty: Vec<bool> = df.geometries.iter().map(|g| matches!(g, Geometry::Empty)).collect();
        // vctrs::vec_fill_missing within each track
        let mut prev: Vec<i64> = (0..n).map(|i| if empty[i] { -1 } else { i as i64 }).collect();
        let mut next = prev.clone();
        for i in 1..n {
            if prev[i] < 0 && code[i - 1] == code[i] {
                prev[i] = prev[i - 1];
            }
        }
        for i in (0..n.saturating_sub(1)).rev() {
            if next[i] < 0 && code[i + 1] == code[i] {
                next[i] = next[i + 1];
            }
        }
        let max_lag: Option<f64> = args.opt_param("max_time_lag")?;
        let c = 1.0 / 60.0; // difftimes in seconds read as minutes (as.numeric(units = "mins"))
        let mut todo: Vec<(usize, usize, usize, f64)> = Vec::new();
        for i in 0..n {
            if prev[i] < 0 || next[i] < 0 {
                continue;
            }
            let (p, q) = (prev[i] as usize, next[i] as usize);
            let (tp, tn) = (sec[p], sec[q]);
            let prop = ((sec[i] - tp) * c) / ((tn - tp) * c);
            let interval = tn - tp;
            let within = max_lag.is_none_or(|l| (if interval.is_nan() { f64::INFINITY } else { interval }) <= l);
            if !prop.is_nan() && target[i] && within && empty[i] {
                todo.push((i, p, q, prop));
            }
        }
        if !todo.is_empty() {
            let xy = |i: usize| -> Xy {
                let Geometry::Point(p) = df.geometries[i] else { unreachable!() };
                p
            };
            let (mut x0, mut y0, mut x1, mut y1, mut f) = (vec![], vec![], vec![], vec![], vec![]);
            for &(_, p, q, prop) in &todo {
                let (a, b) = (xy(p), xy(q));
                x0.push(a[0]);
                y0.push(a[1]);
                x1.push(b[0]);
                y1.push(b[1]);
                f.push(prop);
            }
            let crs = df.crs.clone();
            let lonlat_crs = is_lonlat(crs.as_deref()).map_err(|e| fail(node, e))?;
            let e = |m: String| fail(node, m);
            if let (Some(c), false) = (&crs, lonlat_crs) {
                let tr = terraflux_gdal::Transform::new(c, "EPSG:4326").map_err(e)?;
                tr.points(&mut x0, &mut y0);
                tr.points(&mut x1, &mut y1);
            }
            let mut p = terraflux_ls_sys::ffi::move_interpolate(&x0, &y0, &x1, &y1, &f, crs.is_some())?;
            if let (Some(c), false) = (&crs, lonlat_crs) {
                terraflux_gdal::Transform::new("EPSG:4326", c).map_err(e)?.points(&mut p.x, &mut p.y);
            }
            for (j, &(i, ..)) in todo.iter().enumerate() {
                df.geometries[i] = Geometry::Point([p.x[j], p.y[j]]);
            }
        }
        if let (true, Some(w)) = (args.param("omit", false)?, &wanted) {
            let tv = time_values(&times2);
            let rows: Vec<usize> = (0..n).filter(|&i| w.contains(&tv[i])).collect();
            df = df.select(&rows);
            target.clear();
        }
        Ok(give("tracks", df))
    }
}

/// R's mean of the plain loop landschaft takes for a projected centroid.
fn plain_mean(v: &[f64]) -> f64 {
    let mut s = 0.0;
    for &a in v {
        s += a;
    }
    s / v.len() as f64
}

/// The PROJ string of move2's mt_aeqd_crs centred on the tracks (landschaft's aeqd_crs).
pub(crate) fn aeqd(v: &Vector, center: &serde_json::Value, units: &str) -> std::result::Result<String, String> {
    if units != "m" && units != "km" {
        return Err("units: m or km".into());
    }
    let (lon, lat) = match center {
        serde_json::Value::String(how) => {
            if how != "centroid" && how != "center" {
                return Err("center: centroid, center or [longitude, latitude]".into());
            }
            let mut x = Vec::new();
            let mut y = Vec::new();
            for g in &v.geometries {
                match g {
                    Geometry::Point(p) => {
                        x.push(p[0]);
                        y.push(p[1]);
                    }
                    Geometry::Empty => {}
                    _ => return Err("the geometry must be points".into()),
                }
            }
            if x.is_empty() {
                return Err("no locations".into());
            }
            let crs = v.crs.as_deref();
            let lonlat = is_lonlat(crs)?;
            if how == "centroid" {
                if lonlat {
                    let c = terraflux_ls_sys::ffi::move_s2_centroid(&x, &y).map_err(|e| e.what().to_string())?;
                    (c[0], c[1])
                } else {
                    let (mut cx, mut cy) = ([plain_mean(&x)], [plain_mean(&y)]);
                    if let Some(c) = crs {
                        terraflux_gdal::Transform::new(c, "EPSG:4326")?.points(&mut cx, &mut cy);
                    }
                    (cx[0], cy[0])
                }
            } else {
                if let (false, Some(c)) = (lonlat, crs) {
                    terraflux_gdal::Transform::new(c, "EPSG:4326")?.points(&mut x, &mut y);
                }
                let mid = |v: &[f64]| {
                    let lo = v.iter().copied().fold(f64::INFINITY, f64::min);
                    let hi = v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                    (lo + hi) / 2.0
                };
                (mid(&x), mid(&y))
            }
        }
        serde_json::Value::Array(a) if a.len() == 2 && a.iter().all(|x| x.is_number()) => {
            (a[0].as_f64().unwrap(), a[1].as_f64().unwrap())
        }
        _ => return Err("center: centroid, center or [longitude, latitude]".into()),
    };
    Ok(format!(
        "+proj=aeqd +lat_0={lat:.6} +lon_0={lon:.6} +x_0=0 +y_0=0 +ellps=WGS84 +datum=WGS84 +units={units} +no_defs"
    ))
}

/// `move.aeqd_crs`.
struct Aeqd;

impl Operator for Aeqd {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.aeqd_crs",
            "An azimuthal equidistant projection centred on the tracks (move2's mt_aeqd_crs): distances and \
             directions from the centre are true in it, a local metric projection for tracks anywhere; and the \
             tracks in it. The centre: the centroid of the locations (on the sphere for longitude / latitude), \
             the middle of their longitude and latitude ranges (center), or [longitude, latitude]. move2 takes the \
             centroid to EPSG:4236 (Hu Tzu Shan) where WGS84 is meant, about a kilometre off; here it stays in \
             WGS84, as landschaft's.",
            Access::None,
        )
        .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
        .output(PortSpec::new("crs", Type::Text, "the projection, a PROJ string (for vector.reproject's crs)"))
        .output(PortSpec::new("tracks", Type::Vector, "the tracks in it"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .param(PortSpec::optional("center", Type::Any, "centroid (default), center or [longitude, latitude]"))
        .param(PortSpec::optional("units", Type::Text, "m (default) or km").choices(["m", "km"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let center = args.params.get("center").cloned().unwrap_or_else(|| "centroid".into());
        let units: String = args.param("units", "m".to_string())?;
        let crs = aeqd(v, &center, &units).map_err(|e| fail(node, e))?;
        let projected = match &v.crs {
            Some(_) => terraflux_gdal::reproject_vector(v, &crs).map_err(|e| fail(node, e))?,
            None => return Err(fail(node, "the tracks have no coordinate system to project from")),
        };
        let t = crate::vector::attribute_table(&projected);
        Ok(outputs([
            ("crs", Value::Text(crs)),
            ("tracks", Value::Vector(Arc::new(projected))),
            ("table", Value::Table(t)),
        ]))
    }
}
