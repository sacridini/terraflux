//! amt 0.3.1's steps of tracks (Signer et al. 2019): tracks resampled at a regular rate in bursts
//! (`move.resample`, `move.filter_min_n_burst`), the steps with their lengths, directions and turn angles
//! (`move.steps`), day and night (`move.time_of_day`), the net squared displacement (`move.nsd`) and the summaries of
//! each track (`move.track_summary`: cum_dist, tot_dist, straightness, msd, sinuosity, tac).

use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::{
    BURST, Key, Locations, Times, codes, column_params, columns, fail, give, locations, set_column, take, track_column,
    tracks_of,
};
use crate::sdm::points::to_crs;
use crate::{Args, Ctx, Operator, Outputs, Registry, Result};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Resample));
    r.add(Arc::new(MinBurst));
    r.add(Arc::new(Steps));
    r.add(Arc::new(TimeOfDay));
    r.add(Arc::new(Nsd));
    r.add(Arc::new(Summary));
}

/// A duration in seconds: a number, or a text such as 2h, 15min, 1.5 hours.
pub(crate) fn seconds(v: &serde_json::Value, what: &str) -> std::result::Result<f64, String> {
    let bad = || format!("{what}: seconds, or a text such as 2h or 15min");
    match v {
        serde_json::Value::Number(n) => n.as_f64().ok_or_else(bad),
        serde_json::Value::String(s) => {
            let s = s.trim().to_lowercase();
            let split = s.find(|c: char| c.is_ascii_alphabetic()).ok_or_else(bad)?;
            let (num, unit) = (s[..split].trim(), s[split..].trim());
            let n: f64 = num.parse().map_err(|_| bad())?;
            let f = match unit {
                "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
                "min" | "mins" | "minute" | "minutes" => 60.0,
                "h" | "hr" | "hour" | "hours" => 3600.0,
                "d" | "day" | "days" => 86400.0,
                _ => return Err(bad()),
            };
            Ok(n * f)
        }
        _ => Err(bad()),
    }
}

/// Group codes: the tracks, or their bursts (a burst of one track never joined to another's).
fn groups(args: &Args, v: &Vector, code: &[i64]) -> Result<Vec<i64>> {
    let Some(b) = v.column(BURST) else { return Ok(code.to_vec()) };
    let burst = codes(b);
    let mut seen: HashMap<(i64, i64), i64> = HashMap::new();
    let group: Vec<i64> = code
        .iter()
        .zip(&burst)
        .map(|(&c, &b)| {
            let n = seen.len() as i64;
            *seen.entry((c, b)).or_insert(n)
        })
        .collect();
    if group.windows(2).any(|w| w[1] < w[0]) {
        return Err(fail(
            args.node,
            format!("the locations of each burst must be consecutive (the column {BURST:?} of move.resample)"),
        ));
    }
    Ok(group)
}

/// `move.resample`.
struct Resample;

impl Operator for Resample {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.resample",
                "Locations at a regular rate, in bursts (amt's track_resample, the same locations and bursts): from \
                 the location start of each track, the next location kept is the first at least rate - tolerance \
                 later, in the same burst when it is at most rate + tolerance later, else it starts a new burst; \
                 the locations in between are dropped. Each location kept gets its burst (1, 2... within its \
                 track) in the column burst_, which move.steps and move.random_steps follow.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("tracks", Type::Vector, "the locations kept, with burst_"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::new("rate", Type::Any, "the rate: seconds, or a text such as 2h or 15min"))
            .param(PortSpec::new("tolerance", Type::Any, "the tolerance: seconds, or a text such as 15min"))
            .param(PortSpec::optional(
                "start",
                Type::Integer,
                "the location (0-based, in each track) to start at (0)",
            )),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let s = locations(args, v, false)?;
        let get = |p: &str| args.params.get(p).ok_or_else(|| fail(node, format!("give the {p}")));
        let rate = seconds(get("rate")?, "rate").map_err(|e| fail(node, e))?;
        let tol = seconds(get("tolerance")?, "tolerance").map_err(|e| fail(node, e))?;
        let start: i64 = args.param("start", 0)?;
        if start < 0 {
            return Err(fail(node, "start: a location, 0 or more"));
        }
        let burst = terraflux_ls_sys::ffi::move_amt_resample(&s.times.seconds(), &s.code, rate, tol, start)?;
        let keep: Vec<usize> = (0..v.len()).filter(|&i| burst[i] > 0).collect();
        let mut out = v.select(&keep);
        set_column(&mut out, BURST, Column::Numbers(keep.iter().map(|&i| burst[i] as f64).collect()));
        Ok(give("tracks", out))
    }
}

/// `move.filter_min_n_burst`.
struct MinBurst;

impl Operator for MinBurst {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.filter_min_n_burst",
            "The bursts (move.resample) of at least min_n locations (amt's filter_min_n_burst).",
            Access::None,
        )
        .input(PortSpec::new("tracks", Type::Vector, "the tracks in bursts"))
        .output(PortSpec::new("tracks", Type::Vector, "the bursts kept"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .param(PortSpec::optional("min_n", Type::Integer, "the least locations of a burst (3)"))
        .param(PortSpec::optional("track", Type::Text, "the column of the track (default: the usual names)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let b = v
            .column(BURST)
            .ok_or_else(|| fail(node, format!("column {BURST:?} not found: make bursts with move.resample")))?;
        let k = track_column(args, v)?;
        let tk = v.column(&k).unwrap();
        let min_n: i64 = args.param("min_n", 3)?;
        let key = |i: usize| format!("{:?}|{:?}", Key::of(tk, i), Key::of(b, i));
        let mut size: HashMap<String, i64> = HashMap::new();
        for i in 0..v.len() {
            *size.entry(key(i)).or_default() += 1;
        }
        let keep: Vec<usize> = (0..v.len()).filter(|&i| size[&key(i)] >= min_n).collect();
        Ok(give("tracks", v.select(&keep)))
    }
}

/// `move.steps`.
struct Steps;

impl Operator for Steps {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.steps",
                "The steps between consecutive locations of each track (amt's steps and steps_by_burst, the same \
                 numbers), of each burst when the tracks have move.resample's burst_: a line per step, with its \
                 track, burst_, start and end (x1_, x2_, y1_, y2_), length sl_, direction direction_p \
                 (anticlockwise from east, in (-pi, pi]), turn angle ta_ from the previous step (NaN at the first \
                 step of each track or burst and after a step of length 0), the times t1_, t2_ and dt_ (seconds). In \
                 longitude / latitude the lengths are great-circle distances in metres (sf::st_distance) and the \
                 directions geodesic bearings turned into angles from east: amt reads the bearing, clockwise from \
                 north, as if it were that angle; here it is converted, pi / 2 - bearing.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("steps", Type::Vector, "a line per step"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::optional(
                "lonlat",
                Type::Bool,
                "lengths and directions on the sphere (default: when the coordinate system is geographic)",
            ))
            .param(
                PortSpec::optional(
                    "keep_cols",
                    Type::Text,
                    "the tracks' other columns kept: start (of the location a step starts at), end, both (suffixes \
                     _start and _end) or none",
                )
                .choices(["none", "start", "end", "both"]),
            ),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let s = locations(args, v, false)?;
        let group = groups(args, v, &s.code)?;
        let ll = args.opt_param::<bool>("lonlat")?.unwrap_or(s.lonlat);
        let st = terraflux_ls_sys::ffi::move_amt_steps(&s.x, &s.y, &group, ll)?;
        let n = group.len();
        let i: Vec<usize> = (0..n.saturating_sub(1)).filter(|&j| group[j + 1] == group[j]).collect();
        let i2: Vec<usize> = i.iter().map(|j| j + 1).collect();
        let pick = |x: &[f64], rows: &[usize]| Column::Numbers(rows.iter().map(|&j| x[j]).collect());
        let tcol = v.column(&s.time_col).unwrap();
        let mut attributes = vec![(s.track_col.clone(), take(v.column(&s.track_col).unwrap(), &i))];
        if let Some(b) = v.column(BURST) {
            attributes.push((BURST.to_string(), take(b, &i)));
        }
        attributes.push(("x1_".into(), pick(&s.x, &i)));
        attributes.push(("x2_".into(), pick(&s.x, &i2)));
        attributes.push(("y1_".into(), pick(&s.y, &i)));
        attributes.push(("y2_".into(), pick(&s.y, &i2)));
        attributes.push(("sl_".into(), pick(&st.sl, &i)));
        attributes.push(("direction_p".into(), pick(&st.dir, &i)));
        attributes.push(("ta_".into(), pick(&st.ta, &i)));
        attributes.push(("t1_".into(), take(tcol, &i)));
        attributes.push(("t2_".into(), take(tcol, &i2)));
        attributes.push(("dt_".into(), Column::Numbers(i.iter().map(|&j| s.times.lag(j, j + 1)).collect())));
        let keep: String = args.param("keep_cols", "none".to_string())?;
        if keep != "none" && !keep.is_empty() {
            if !["start", "end", "both"].contains(&keep.as_str()) {
                return Err(fail(node, "keep_cols: start, end, both or none"));
            }
            let other: Vec<&(String, Column)> =
                v.attributes.iter().filter(|(c, _)| *c != s.time_col && *c != s.track_col && c != BURST).collect();
            for (place, rows) in [("start", &i), ("end", &i2)] {
                if keep == place || keep == "both" {
                    for (c, col) in &other {
                        let name = if keep == "both" { format!("{c}_{place}") } else { c.clone() };
                        attributes.push((name, take(col, rows)));
                    }
                }
            }
        }
        let geometries =
            i.iter().map(|&j| Geometry::LineString(vec![[s.x[j], s.y[j]], [s.x[j + 1], s.y[j + 1]]])).collect();
        Ok(give("steps", Vector { geometries, attributes, crs: v.crs.clone() }))
    }
}

/// Whether features are the steps of move.steps (or move.random_steps).
pub(crate) fn is_steps(v: &Vector) -> bool {
    v.column("x1_").is_some()
}

/// A numbers column of steps.
pub(crate) fn numbers(v: &Vector, name: &str) -> std::result::Result<Vec<f64>, String> {
    match v.column(name) {
        Some(c) => Ok(c.numbers()),
        None => Err(format!("no column {name:?}: the steps of move.steps")),
    }
}

/// `move.time_of_day`.
struct TimeOfDay;

impl Operator for TimeOfDay {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.time_of_day",
                "Day or night at each location, or at the end or start of each step (amt's time_of_day): sunrise \
                 and sunset of the location's date (UTC) as suncalc computes them (the sun's upper edge at -0.833 \
                 degrees), and with include_crepuscule dawn and dusk (the sun at -6 degrees, civil twilight). A \
                 column of day, night (and dawn, dusk); empty where the sun does not rise or set that day. suncalc \
                 takes the noon of the machine's time zone; here, the noon of UTC.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "tracks (with a coordinate system) or their steps"))
            .output(PortSpec::new("tracks", Type::Vector, "the tracks or steps with the column"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::optional("include_crepuscule", Type::Bool, "also dawn and dusk (false)"))
            .param(
                PortSpec::optional("where", Type::Text, "steps: at their end (default) or start")
                    .choices(["end", "start"]),
            )
            .param(PortSpec::optional("column", Type::Text, "the column given (tod)")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let (mut xs, mut ys, t): (Vec<f64>, Vec<f64>, Vec<f64>) = if is_steps(v) {
            let e = match args.param("where", "end".to_string())?.as_str() {
                "end" => "2",
                "start" => "1",
                _ => return Err(fail(node, "where: end or start")),
            };
            let num = |c: &str| numbers(v, c).map_err(|m| fail(node, m));
            let tcol = v.column(&format!("t{e}_")).ok_or_else(|| fail(node, format!("no column t{e}_")))?;
            let t = Times::of(tcol).map_err(|m| fail(node, m))?.seconds();
            (num(&format!("x{e}_"))?, num(&format!("y{e}_"))?, t)
        } else {
            let s: Locations = locations(args, v, false)?;
            let t = s.times.seconds();
            (s.x, s.y, t)
        };
        let crs = v.crs.as_deref().ok_or_else(|| fail(node, "the locations have no coordinate system"))?;
        to_crs(&mut xs, &mut ys, Some(crs), Some("EPSG:4326")).map_err(|e| fail(node, e))?;
        let noon: Vec<f64> = t.iter().map(|&t| (t / 86400.0).floor() * 86400.0 + 43200.0).collect();
        let sun = terraflux_ls_sys::ffi::move_sun_times(&xs, &ys, &noon)?;
        // lubridate's %within%: 0 <= t - start <= end - start
        let within = |i: usize, start: f64, end: f64| {
            let d = t[i] - start;
            (d >= 0.0 && d <= end - start, d.is_nan() || (end - start).is_nan())
        };
        let crepuscule: bool = args.param("include_crepuscule", false)?;
        let tod: Vec<String> = (0..t.len())
            .map(|i| {
                let (day, missing) = within(i, sun.sunrise[i], sun.sunset[i]);
                let mut s = if missing {
                    ""
                } else if day {
                    "day"
                } else {
                    "night"
                };
                if crepuscule {
                    if within(i, sun.dawn[i], sun.sunrise[i]).0 {
                        s = "dawn";
                    }
                    if within(i, sun.sunset[i], sun.dusk[i]).0 {
                        s = "dusk";
                    }
                }
                s.to_string()
            })
            .collect();
        let name: String = args.param("column", "tod".to_string())?;
        let mut out = (**v).clone();
        set_column(&mut out, &name, Column::Texts(tod));
        Ok(give("tracks", out))
    }
}

/// `move.nsd`.
struct Nsd;

impl Operator for Nsd {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.nsd",
                "The net squared displacement: the squared distance of each location from the first of its track \
                 (amt's nsd), in the units of the coordinates (squared degrees in longitude / latitude, as amt).",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("tracks", Type::Vector, "the tracks with the column"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::optional("column", Type::Text, "the column given (nsd)")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = tracks_of(args, "tracks")?;
        let s = locations(args, v, false)?;
        let mut start = 0;
        let nsd: Vec<f64> = (0..s.code.len())
            .map(|i| {
                if i == 0 || s.code[i] != s.code[i - 1] {
                    start = i;
                }
                let (dx, dy) = (s.x[i] - s.x[start], s.y[i] - s.y[start]);
                dx * dx + dy * dy
            })
            .collect();
        let name: String = args.param("column", "nsd".to_string())?;
        let mut out = (**v).clone();
        set_column(&mut out, &name, Column::Numbers(nsd));
        Ok(give("tracks", out))
    }
}

/// amt's summaries of one track (rows a..b of the locations).
fn summaries(s: &Locations, a: usize, b: usize) -> Result<[f64; 6]> {
    use terraflux_ls_sys::ffi;
    let (x, y) = (&s.x[a..b], &s.y[a..b]);
    let zeros = vec![0i64; b - a];
    let d = ffi::move_distances(x, y, &zeros, s.lonlat, 1)?;
    let cum = if d.len() > 1 { ffi::move_r_sum(&d[..d.len() - 1]) } else { 0.0 };
    let ends_x = [x[0], x[x.len() - 1]];
    let ends_y = [y[0], y[y.len() - 1]];
    let tot = ffi::move_distances(&ends_x, &ends_y, &[0, 0], s.lonlat, 1)?[0];
    let msd = ffi::move_amt_msd(x, y)?;
    let st = ffi::move_amt_steps(x, y, &zeros, s.lonlat)?;
    let k = st.sl.len() - 1;
    let (sl, ta) = (&st.sl[..k], &st.ta[..k]);
    let sinuosity = if k >= 2 { ffi::move_amt_sinuosity(sl, ta)? } else { f64::NAN };
    let tac = ffi::move_amt_tac(ta);
    Ok([cum, tot, tot / cum, msd, sinuosity, tac])
}

/// `move.track_summary`.
struct Summary;

impl Operator for Summary {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.track_summary",
                "amt's summaries of each track, a line per track (move.track_lines's, with n_locations and the \
                 track's data): cum_dist (the sum of its steps, metres in longitude / latitude), tot_dist (from the \
                 first to the last location), straightness (tot_dist / cum_dist), msd (the mean squared distance of \
                 the locations from their centre), sinuosity (Benhamou 2004, the turn angles in radians: amt takes \
                 them as degrees; NaN under two steps) and tac (the turn angle correlation, Dray et al. 2010).",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("tracks", Type::Vector, "a line per track with its summaries"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = tracks_of(args, "tracks")?;
        let s = locations(args, v, false)?;
        let n = s.code.len();
        let starts: Vec<usize> = (0..n).filter(|&i| i == 0 || s.code[i] != s.code[i - 1]).collect();
        let ends: Vec<usize> = starts.iter().skip(1).copied().chain(std::iter::once(n)).collect();
        let rows: Vec<[f64; 6]> =
            starts.iter().zip(&ends).map(|(&a, &b)| summaries(&s, a, b)).collect::<Result<_>>()?;
        let (t, k) = columns(args, v)?;
        let mut out = super::geometry::lines_of(v, &t, &k, &s.code);
        for (j, name) in ["cum_dist", "tot_dist", "straightness", "msd", "sinuosity", "tac"].iter().enumerate() {
            set_column(&mut out, name, Column::Numbers(rows.iter().map(|r| r[j]).collect()));
        }
        Ok(give("tracks", out))
    }
}
