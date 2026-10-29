//! Tracks: Movebank's files (`move.read_tracks`, move2's mt_read), any points or table of locations
//! (`move.as_tracks`), Brownian tracks (`move.simulate_brownian`), and the distance, duration, speed, direction and
//! turn of every step (`move.distance`, `move.time_lags`, `move.speed`, `move.azimuth`, `move.turn_angle`).

use std::io::Read as _;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::{
    Times, column_params, columns, duration_factor, fail, give, length_factor, locations, set_column, sorted,
    speed_factor, threads, tracks_of,
};
use crate::{Args, Ctx, Operator, Outputs, Registry, Result, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(ReadTracks));
    r.add(Arc::new(AsTracks));
    r.add(Arc::new(Brownian));
    for m in [Measure::Distance, Measure::TimeLags, Measure::Speed, Measure::Azimuth, Measure::TurnAngle] {
        r.add(Arc::new(m));
    }
}

const BOOLEAN: [&str; 9] = [
    "visible",
    "manually-marked-outlier",
    "algorithm-marked-outlier",
    "import-marked-outlier",
    "manually-marked-valid",
    "manually_marked_outlier",
    "algorithm_marked_outlier",
    "import_marked_outlier",
    "manually_marked_valid",
];
const TEXT: [&str; 6] = [
    "individual-local-identifier",
    "tag-local-identifier",
    "individual-taxon-canonical-name",
    "study-name",
    "sensor-type",
    "comments",
];
/// pandas' default missing values.
const NA: [&str; 19] = [
    "", "#N/A", "#N/A N/A", "#NA", "-1.#IND", "-1.#QNAN", "-NaN", "-nan", "1.#IND", "1.#QNAN", "<NA>", "N/A", "NA",
    "NULL", "NaN", "None", "n/a", "nan", "null",
];

/// The text of a Movebank file: a CSV, gzipped, or the one CSV in a zip (Env-DATA's).
fn movebank_text(path: &str) -> std::result::Result<String, String> {
    let lower = path.to_lowercase();
    let bytes = if lower.ends_with(".zip") {
        let root = format!("/vsizip/{path}");
        let names: Vec<String> =
            terraflux_gdal::read_dir(&root)?.into_iter().filter(|n| n.to_lowercase().ends_with(".csv")).collect();
        if names.len() != 1 {
            return Err("the zip file should contain one csv file".into());
        }
        let mut f = terraflux_gdal::VsiFile::open(&format!("{root}/{}", names[0]))?;
        let mut b = Vec::new();
        f.read_to_end(&mut b).map_err(|e| e.to_string())?;
        b
    } else {
        let raw = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
        if raw.starts_with(&[0x1f, 0x8b]) {
            let mut b = Vec::new();
            flate2::read::MultiGzDecoder::new(&raw[..]).read_to_end(&mut b).map_err(|e| format!("{path}: {e}"))?;
            b
        } else {
            raw
        }
    };
    let text = String::from_utf8_lossy(&bytes).into_owned();
    Ok(text.strip_prefix('\u{feff}').map(str::to_string).unwrap_or(text))
}

/// A column of a CSV as pandas reads it: numbers when every value is one (or missing), else texts; Movebank's
/// flags as numbers 1 / 0.
fn csv_column(name: &str, values: Vec<Option<String>>) -> Column {
    let missing = |v: &Option<String>| v.as_deref().is_none_or(|s| NA.contains(&s));
    if BOOLEAN.contains(&name) {
        return Column::Numbers(
            values
                .iter()
                .map(|v| match v.as_deref().map(|s| s.trim().to_lowercase()) {
                    Some(s) if s == "true" => 1.0,
                    Some(s) if s == "false" => 0.0,
                    _ => f64::NAN,
                })
                .collect(),
        );
    }
    if !TEXT.contains(&name) {
        let numbers: Option<Vec<f64>> = values
            .iter()
            .map(|v| if missing(v) { Some(f64::NAN) } else { v.as_deref().unwrap().trim().parse::<f64>().ok() })
            .collect();
        if let Some(n) = numbers {
            return Column::Numbers(n);
        }
    }
    Column::Texts(values.into_iter().map(|v| if missing(&v) { String::new() } else { v.unwrap() }).collect())
}

/// Movebank's tracks from the text of a file (move2's mt_read): points in EPSG:4326, the time `timestamp`, the
/// track the individual (with the tag when an individual carried several), records without a location empty.
pub fn movebank(text: &str, sort: bool) -> std::result::Result<(Vector, String), String> {
    let rows = crate::bio::basedata::read_csv(text, ',', '"');
    let mut rows = rows.into_iter().filter(|r| !(r.len() == 1 && r[0].is_none()));
    let header: Vec<String> = rows.next().ok_or("an empty file")?.into_iter().map(Option::unwrap_or_default).collect();
    let body: Vec<Vec<Option<String>>> = rows.collect();
    for c in ["timestamp", "location-long", "location-lat"] {
        if !header.iter().any(|h| h == c) {
            return Err(format!("not a Movebank file: no {c} column"));
        }
    }
    if !header.iter().any(|h| h == "individual-local-identifier") {
        return Err("not a Movebank file: no individual-local-identifier column".into());
    }
    let mut attributes: Vec<(String, Column)> = Vec::new();
    let (mut lon, mut lat) = (Vec::new(), Vec::new());
    for (j, name) in header.iter().enumerate() {
        let values: Vec<Option<String>> = body.iter().map(|r| r.get(j).cloned().flatten()).collect();
        match name.as_str() {
            "location-long" | "location-lat" => {
                let Column::Numbers(v) = csv_column("", values) else {
                    return Err(format!("{name}: numbers"));
                };
                if name == "location-long" { lon = v } else { lat = v }
            }
            "timestamp" => {
                let c = Column::Texts(values.into_iter().map(Option::unwrap_or_default).collect());
                Times::of(&c)?;
                attributes.push((name.clone(), c));
            }
            _ => attributes.push((name.clone(), csv_column(name, values))),
        }
    }
    let mut track = "individual-local-identifier".to_string();
    let text_of = |c: &Column, i: usize| match c {
        Column::Texts(t) => t[i].clone(),
        Column::Numbers(x) => {
            if x[i].is_nan() {
                String::new()
            } else {
                crate::value::class_label(x[i])
            }
        }
    };
    let find = |n: &str| attributes.iter().find(|(a, _)| a == n).map(|(_, c)| c.clone());
    if let (Some(ind), Some(tag)) = (find("individual-local-identifier"), find("tag-local-identifier")) {
        let mut pairs: std::collections::HashMap<String, std::collections::HashSet<String>> = Default::default();
        for i in 0..ind.len() {
            pairs.entry(text_of(&ind, i)).or_default().insert(text_of(&tag, i));
        }
        if pairs.values().any(|t| t.len() > 1) {
            let joined = (0..ind.len())
                .map(|i| {
                    let (a, b) = (text_of(&ind, i), text_of(&tag, i));
                    if a.is_empty() || b.is_empty() { String::new() } else { format!("{a}_{b}") }
                })
                .collect();
            track = "individual-tag-local-identifier".into();
            attributes.push((track.clone(), Column::Texts(joined)));
        }
    }
    let geometries = lon
        .iter()
        .zip(&lat)
        .map(|(&x, &y)| if x.is_nan() || y.is_nan() { Geometry::Empty } else { Geometry::Point([x, y]) })
        .collect();
    let v = Vector { geometries, attributes, crs: Some("EPSG:4326".into()) };
    let v = if sort { sorted(&v, "timestamp", &track)? } else { v };
    Ok((v, track))
}

/// `move.read_tracks`.
struct ReadTracks;

impl Operator for ReadTracks {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.read_tracks",
            "Animal tracks from a Movebank file (move2's mt_read, landschaft's load_tracks): the CSV files Movebank \
             exports (also .csv.gz) and the zip files of Env-DATA (the CSV inside), as points in EPSG:4326, one per \
             record. The time is timestamp (UTC), the track the individual (individual-local-identifier; when an \
             individual carried several tags, the individual and the tag, in a new individual-tag-local-identifier \
             column, as move2 does); the outlier flags are numbers 1 and 0. Records without coordinates (other \
             sensors) keep an empty location. Ordered by track and time (stable), as every move node needs.",
            Access::None,
        )
        .output(PortSpec::new("tracks", Type::Vector, "the locations: points with the file's columns"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .param(PortSpec::new("path", Type::Text, "the file (.csv, .csv.gz, .zip), relative to the model's folder"))
        .param(PortSpec::optional(
            "sort",
            Type::Bool,
            "order by track and time (true; move2 keeps the file's order)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let path: String = args.param("path", String::new())?;
        if path.is_empty() {
            return Err(fail(node, "choose the file"));
        }
        let text = movebank_text(&path).map_err(|e| fail(node, e))?;
        let (v, _) = movebank(&text, args.param("sort", true)?).map_err(|e| fail(node, format!("{path}: {e}")))?;
        Ok(give("tracks", v))
    }
}

/// `move.as_tracks`.
struct AsTracks;

impl Operator for AsTracks {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.as_tracks",
                "Tracks from points or a table of locations (move2's mt_as_move2, landschaft's as_tracks): ordered \
                 by track and time (stable), the tracks in the order they first appear, as every move node needs. \
                 The time is a column of datetimes as text (ISO 8601, UTC) or of numbers; the track one of names or \
                 numbers. A table's x and y columns become points in the coordinate system given.",
                Access::None,
            )
            .input(PortSpec::optional("vector", Type::Vector, "points, one per location"))
            .input(PortSpec::optional("table", Type::Table, "or a table with a column of x and one of y"))
            .output(PortSpec::new("tracks", Type::Vector, "the locations, ordered"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::optional("x", Type::Text, "a table's column of the x or longitude (x)"))
            .param(PortSpec::optional("y", Type::Text, "a table's column of the y or latitude (y)"))
            .param(PortSpec::optional("crs", Type::Text, "a table's coordinate system: EPSG:4326, a WKT..."))
            .param(PortSpec::optional("sort", Type::Bool, "order by track and time (true)")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v: Vector = match (args.get("vector"), args.get("table")) {
            (Some(Value::Vector(v)), _) => {
                if v.geometries.iter().any(|g| !matches!(g, Geometry::Point(_) | Geometry::Empty)) {
                    return Err(fail(node, "the geometry must be points"));
                }
                (**v).clone()
            }
            (_, Some(Value::Table(t))) => {
                let col = |p: &str| -> Result<usize> {
                    let name: String = args.param(p, p.to_string())?;
                    t.columns.iter().position(|c| *c == name).ok_or_else(|| {
                        fail(node, format!("the table has no column {name:?} (its columns: {})", t.columns.join(", ")))
                    })
                };
                let (x, y) = (col("x")?, col("y")?);
                Vector {
                    geometries: t
                        .rows
                        .iter()
                        .map(|r| {
                            if r[x].is_nan() || r[y].is_nan() { Geometry::Empty } else { Geometry::Point([r[x], r[y]]) }
                        })
                        .collect(),
                    attributes: (0..t.columns.len())
                        .filter(|&j| j != x && j != y)
                        .map(|j| (t.columns[j].clone(), Column::Numbers(t.rows.iter().map(|r| r[j]).collect())))
                        .collect(),
                    crs: args.opt_param::<String>("crs")?.filter(|c| !c.trim().is_empty()),
                }
            }
            _ => return Err(fail(node, "connect the points (vector) or a table of locations (table)")),
        };
        let (t, k) = columns(args, &v)?;
        Times::of(v.column(&t).unwrap()).map_err(|e| fail(node, e))?;
        let v = if args.param("sort", true)? { sorted(&v, &t, &k).map_err(|e| fail(node, e))? } else { v };
        Ok(give("tracks", v))
    }
}

/// One value or a list of one per track (a list of lists).
fn per_track(
    v: Option<&serde_json::Value>,
    default: serde_json::Value,
    k: usize,
) -> std::result::Result<Vec<serde_json::Value>, String> {
    let v = v.cloned().unwrap_or(default);
    match &v {
        serde_json::Value::Array(items) if !items.is_empty() && items[0].is_array() => {
            if items.len() != k {
                return Err("one value per track: as many as the tracks".into());
            }
            Ok(items.clone())
        }
        _ => Ok(vec![v; k]),
    }
}

fn numbers(v: &serde_json::Value, what: &str) -> std::result::Result<Vec<f64>, String> {
    match v {
        serde_json::Value::Number(n) => Ok(vec![n.as_f64().unwrap_or(f64::NAN)]),
        serde_json::Value::Array(a) => a.iter().map(|x| x.as_f64().ok_or_else(|| format!("{what}: numbers"))).collect(),
        _ => Err(format!("{what}: a number or a list of numbers")),
    }
}

/// `move.simulate_brownian`.
struct Brownian;

impl Operator for Brownian {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.simulate_brownian",
            "Tracks of Brownian motion (move2's mt_sim_brownian_motion): each step moves by normal draws of \
             standard deviation sqrt(sigma^2 dt) along x and along y, from the start. R's generator after \
             set.seed(seed): the same tracks as move2 (the tracks one after the other, x before y). Columns time \
             and track, points without a coordinate system.",
            Access::None,
        )
        .output(PortSpec::new("tracks", Type::Vector, "the locations"))
        .output(PortSpec::new("table", Type::Table, "time and track of each"))
        .param(PortSpec::optional(
            "t",
            Type::Any,
            "the times (numbers; default 1 to 10): one list for every track, or a list of one per track",
        ))
        .param(PortSpec::optional(
            "sigma",
            Type::Any,
            "the diffusion: a number or one per step, for every track, or a list of one per track (1)",
        ))
        .param(PortSpec::optional("tracks", Type::Any, "how many tracks (ids 1, 2...) or their ids (2)"))
        .param(PortSpec::optional("start", Type::Any, "the first location [x, y], or a list of one per track ([0, 0])"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of R's generator (1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let e = |m: String| fail(node, m);
        let ids: Vec<serde_json::Value> = match args.params.get("tracks") {
            None => (1..=2).map(|i| i.into()).collect(),
            Some(serde_json::Value::Number(n)) => {
                let k = n.as_f64().unwrap_or(0.0);
                if k < 1.0 || k.fract() != 0.0 {
                    return Err(e("tracks: a whole number, 1 or more, or the ids".into()));
                }
                (1..=k as i64).map(|i| i.into()).collect()
            }
            Some(serde_json::Value::Array(a)) if !a.is_empty() => a.clone(),
            _ => return Err(e("tracks: how many, or their ids".into())),
        };
        let names: Vec<String> = ids
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            })
            .collect();
        if names.iter().collect::<std::collections::HashSet<_>>().len() != names.len() {
            return Err(e("tracks: duplicated ids".into()));
        }
        let k = ids.len();
        let default_t: serde_json::Value = (1..=10).collect::<Vec<i64>>().into();
        let times = per_track(args.params.get("t"), default_t, k).map_err(e)?;
        let sigmas = per_track(args.params.get("sigma"), 1.0.into(), k).map_err(e)?;
        let starts = per_track(args.params.get("start"), serde_json::json!([0.0, 0.0]), k).map_err(e)?;
        let seed: i32 = args.param("seed", 1)?;
        let mut rng = terraflux_ls_sys::RRng::new(seed);
        let ints = ids.iter().all(|v| v.as_i64().is_some());
        let (mut time, mut x, mut y, mut track_n, mut track_t) = (vec![], vec![], vec![], vec![], vec![]);
        for j in 0..k {
            let t = numbers(&times[j], "t").map_err(e)?;
            let s = numbers(&sigmas[j], "sigma").map_err(e)?;
            let st = numbers(&starts[j], "start").map_err(e)?;
            if st.len() != 2 {
                return Err(e("start: [x, y]".into()));
            }
            if t.windows(2).any(|w| w[1] < w[0]) {
                return Err(e("t: the times must not decrease".into()));
            }
            if s.iter().any(|&v| v < 0.0) {
                return Err(e("sigma: values of 0 or more".into()));
            }
            if s.len() != 1 && s.len() != t.len().saturating_sub(1) {
                return Err(e("sigma: one value or one per step".into()));
            }
            let p = terraflux_ls_sys::ffi::move_brownian(rng.pin_mut(), &t, &s, st[0], st[1])?;
            time.extend_from_slice(&t);
            x.extend_from_slice(&p.x);
            y.extend_from_slice(&p.y);
            for _ in 0..t.len() {
                track_n.push(ids[j].as_f64().unwrap_or(f64::NAN));
                track_t.push(names[j].clone());
            }
        }
        let track = if ints { Column::Numbers(track_n) } else { Column::Texts(track_t) };
        let v = Vector {
            geometries: x.iter().zip(&y).map(|(&a, &b)| Geometry::Point([a, b])).collect(),
            attributes: vec![("time".into(), Column::Numbers(time)), ("track".into(), track)],
            crs: None,
        };
        Ok(give("tracks", v))
    }
}

/// Time from each location to the next of its track, in seconds (NaN at the last of each track).
pub(crate) fn lags(times: &Times, code: &[i64]) -> Vec<f64> {
    let n = code.len();
    let mut out = vec![f64::NAN; n];
    for i in 0..n.saturating_sub(1) {
        if code[i + 1] == code[i] {
            out[i] = times.lag(i, i + 1);
        }
    }
    out
}

/// The measures of steps.
#[derive(Clone, Copy)]
enum Measure {
    Distance,
    TimeLags,
    Speed,
    Azimuth,
    TurnAngle,
}

impl Operator for Measure {
    fn signature(&self) -> OpSignature {
        let (name, doc, column, units, choices): (&str, &str, &str, &str, &[&str]) = match self {
            Measure::Distance => (
                "move.distance",
                "The distance from each location to the next of its track (move2's mt_distance, the same numbers): \
                 on the sphere of S2 (radius 6 371 010 m, as sf::st_distance with s2) in longitude / latitude, the \
                 straight line in the coordinate system's units otherwise. NaN at the last location of each track.",
                "distance",
                "m (the default in longitude / latitude) or km; none keeps metres or the CRS's units",
                &["m", "km"],
            ),
            Measure::TimeLags => (
                "move.time_lags",
                "The time from each location to the next of its track (move2's mt_time_lags), exact between \
                 datetimes (differences of nanoseconds). NaN at the last location of each track.",
                "time_lag",
                "s (default), min, h or d",
                &["s", "min", "h", "d"],
            ),
            Measure::Speed => (
                "move.speed",
                "The speed of each step: the distance over the time lag (move2's mt_speed). Locations of a track \
                 must not share their time (move.filter_unique drops them). NaN at the last location of each track.",
                "speed",
                "m/s, km/h, m/h or km/d (default: metres, or the CRS's units, per second)",
                &["m/s", "km/h", "m/h", "km/d"],
            ),
            Measure::Azimuth => (
                "move.azimuth",
                "The direction of each step, clockwise from north (move2's mt_azimuth): the geodesic azimuth on the \
                 coordinate system's ellipsoid in longitude / latitude (lwgeom's st_geod_azimuth, GeographicLib, \
                 the same numbers), in (-pi, pi]; the direction on the grid, atan2(dx, dy), in a projected system or \
                 without one. NaN at the last location of each track and where a location repeats the previous.",
                "azimuth",
                "rad (default) or deg",
                &["rad", "deg"],
            ),
            Measure::TurnAngle => (
                "move.turn_angle",
                "The change of direction at each location, in [-pi, pi) (move2's mt_turnangle): the azimuth of the \
                 step leaving it minus that of the step arriving, positive to the right. NaN at the first and last \
                 location of each track.",
                "turn_angle",
                "rad (default) or deg",
                &["rad", "deg"],
            ),
        };
        column_params(
            OpSignature::new(name, doc, Access::None)
                .input(PortSpec::new("tracks", Type::Vector, "the tracks: points ordered by track and time"))
                .output(PortSpec::new("tracks", Type::Vector, "the tracks with the column"))
                .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
                .param(PortSpec::optional("units", Type::Text, units).choices(choices.iter().copied()))
                .param(PortSpec::optional("column", Type::Text, &format!("the column given ({column})"))),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let units: Option<String> = args.opt_param::<String>("units")?.filter(|u| !u.is_empty());
        let units = units.as_deref();
        let s = locations(args, v, matches!(self, Measure::Speed))?;
        let crs = v.crs.as_deref();
        let e = |m: String| fail(node, m);
        let (column, values) = match self {
            Measure::Distance => {
                let d = terraflux_ls_sys::ffi::move_distances(&s.x, &s.y, &s.code, s.lonlat, threads())?;
                let f = length_factor(units, crs, s.lonlat).map_err(e)?;
                ("distance", if f == 1.0 { d } else { d.iter().map(|x| x / f).collect() })
            }
            Measure::TimeLags => {
                let out = lags(&s.times, &s.code);
                let f = duration_factor(units).map_err(e)?;
                ("time_lag", if f == 1.0 { out } else { out.iter().map(|x| x / f).collect() })
            }
            Measure::Speed => {
                let d = terraflux_ls_sys::ffi::move_distances(&s.x, &s.y, &s.code, s.lonlat, threads())?;
                let l = lags(&s.times, &s.code);
                let v: Vec<f64> = d.iter().zip(&l).map(|(a, b)| a / b).collect();
                let (length, duration) = speed_factor(units, crs, s.lonlat).map_err(e)?;
                (
                    "speed",
                    if (length, duration) == (1.0, 1.0) {
                        v
                    } else {
                        v.iter().map(|x| x / length * duration).collect()
                    },
                )
            }
            Measure::Azimuth | Measure::TurnAngle => {
                let az = terraflux_ls_sys::ffi::move_azimuths(
                    &s.x,
                    &s.y,
                    &s.code,
                    s.lonlat,
                    s.a,
                    s.inv_flattening,
                    threads(),
                )?;
                let (column, values) = if matches!(self, Measure::Azimuth) {
                    ("azimuth", az)
                } else {
                    ("turn_angle", terraflux_ls_sys::ffi::move_turn_angles(&az, threads()))
                };
                let values = match units.unwrap_or("rad") {
                    "rad" => values,
                    "deg" => values.iter().map(|x| x * (180.0 / std::f64::consts::PI)).collect(),
                    _ => return Err(e("units: rad or deg".into())),
                };
                (column, values)
            }
        };
        let name: String = args.param("column", column.to_string())?;
        let mut out = (**v).clone();
        set_column(&mut out, &name, Column::Numbers(values));
        Ok(give("tracks", out))
    }
}
