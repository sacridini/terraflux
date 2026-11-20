//! Track cleaning and thinning (the library's `tf.move`, from R's move2utils 0.4.6, Kranstauber, Safi and Scharf
//! 2026): the allometric speed limit of Hirt et al. (2017), the hard speed cap, the GPS quality filter and the
//! thinning of tracks in time and in distance. The kernels are the library's C++ (`terraflux-ls-sys`); the tracks run
//! in parallel there, so the result does not depend on the number of threads.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::{
    Times, check_order, codes, column_params, columns, fail, give, is_lonlat, length_factor, set_column, threads,
    track_column, tracks_of,
};
use crate::{Args, Ctx, Operator, Outputs, Registry, Result, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(MaxSpeed));
    r.add(Arc::new(FlagSpeed));
    r.add(Arc::new(GpsQuality));
    r.add(Arc::new(ThinTime));
    r.add(Arc::new(ThinDistance));
}

const MODES: [&str; 3] = ["flying", "running", "swimming"];
/// The masses (kg) of the data Hirt et al. fitted.
const HIRT_RANGE: (f64, f64) = (3e-8, 1.084e5);
const NS: i64 = 1_000_000_000;

/// R's `match.arg`: the full name or a unique beginning of it (the index of the mode).
fn mode_of(mode: &str) -> std::result::Result<usize, String> {
    let bad = || format!("mode: one of {}", MODES.join(", "));
    let hits: Vec<usize> = match MODES.iter().position(|m| *m == mode) {
        Some(i) => vec![i],
        None => (0..3).filter(|&i| !mode.is_empty() && MODES[i].starts_with(mode)).collect(),
    };
    if hits.len() == 1 { Ok(hits[0]) } else { Err(bad()) }
}

/// v_phys_estimate of one mass: (v_max, lower, upper, km/h), with a warning outside Hirt et al.'s data.
fn v_phys(ctx: &mut Ctx, node: &str, mass: f64, mode: &str, ci_level: f64) -> Result<[f64; 4]> {
    let mode = mode_of(mode).map_err(|e| fail(node, e))?;
    if mass.is_nan() || mass <= 0.0 {
        return Err(fail(node, "mass: a positive number (kg)"));
    }
    let level_ok = ci_level > 0.0 && ci_level < 1.0;
    if !level_ok {
        return Err(fail(node, "ci_level: a number in (0, 1)"));
    }
    if mass < HIRT_RANGE.0 || mass > HIRT_RANGE.1 {
        ctx.warn(format!(
            "node {node:?}: mass outside the range of the Hirt et al. (2017) data ({:e}, {:e}) kg: the prediction is \
             an extrapolation",
            HIRT_RANGE.0, HIRT_RANGE.1
        ));
    }
    let v = terraflux_ls_sys::ffi::move_v_phys(&[mass], mode as i32, ci_level)?;
    Ok([v[0], v[1], v[2], v[3]])
}

/// The seconds of a column of times: datetimes as their whole seconds plus the fraction (one rounding, not two:
/// nanoseconds over 1e9 would round twice near 1e9 s), NaN where missing.
fn posix_seconds(t: &Times) -> Vec<f64> {
    match t {
        Times::Numbers(x) => x.clone(),
        Times::Instants(x) => x
            .iter()
            .map(|t| {
                t.map_or(f64::NAN, |ns| {
                    let whole = ns.div_euclid(NS);
                    whole as f64 + (ns - whole * NS) as f64 / 1e9
                })
            })
            .collect(),
    }
}

/// The locations of tracks that are points with a position (x, y), all finite.
fn points(node: &str, v: &Vector, what: &str) -> Result<(Vec<f64>, Vec<f64>)> {
    if v.geometries.iter().any(|g| matches!(g, Geometry::Empty)) {
        return Err(fail(
            node,
            format!("some locations are empty: drop them before {what} (vector.filter, or move.filter_gps_quality)"),
        ));
    }
    let (mut x, mut y) = (Vec::with_capacity(v.len()), Vec::with_capacity(v.len()));
    for g in &v.geometries {
        match g {
            Geometry::Point(p) => {
                x.push(p[0]);
                y.push(p[1]);
            }
            _ => return Err(fail(node, "the geometry must be points")),
        }
    }
    if !(x.iter().all(|a| a.is_finite()) && y.iter().all(|a| a.is_finite())) {
        return Err(fail(node, format!("some locations have non-finite coordinates: remove them before {what}")));
    }
    Ok((x, y))
}

fn kept(v: &Vector, keep: impl Fn(usize) -> bool) -> Vector {
    let rows: Vec<usize> = (0..v.len()).filter(|&i| keep(i)).collect();
    v.select(&rows)
}

fn flags(x: &[i8]) -> Column {
    Column::Numbers(x.iter().map(|&f| if f < 0 { f64::NAN } else { f64::from(f) }).collect())
}

fn mask(x: &[bool]) -> Column {
    Column::Numbers(x.iter().map(|&f| f64::from(u8::from(f))).collect())
}

/// `move.max_speed`.
struct MaxSpeed;

impl Operator for MaxSpeed {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.max_speed",
            "The allometric maximum speed of an animal from its body mass (move2utils' v_phys_estimate): the scaling \
             law of Hirt et al. (2017), v = a M^b (1 - exp(-h M^i)) with the parameters of its Supplementary Table 4 \
             for each locomotion mode, and the interval of the parameter uncertainty (delta method) at ci_level. A \
             ceiling for speeds that cannot be real (move.flag_speed), not a cruising speed; specialists (cheetah, \
             pronghorn) can exceed it. A warning outside the data of Hirt et al. (3e-8 to 1.084e5 kg).",
            Access::None,
        )
        .output(PortSpec::new("v_max", Type::Number, "the maximum speed, m/s"))
        .output(PortSpec::new("lower", Type::Number, "the lower bound of the interval, m/s"))
        .output(PortSpec::new("upper", Type::Number, "the upper bound of the interval, m/s"))
        .output(PortSpec::new("kmh", Type::Number, "the maximum speed, km/h (Hirt et al.'s unit)"))
        .output(PortSpec::new("table", Type::Table, "a row: mass, v_max, lower, upper, kmh"))
        .param(PortSpec::new("mass", Type::Number, "the body mass, kg"))
        .param(
            PortSpec::optional("mode", Type::Text, "the locomotion: flying (default), running or swimming")
                .choices(MODES),
        )
        .param(PortSpec::optional("ci_level", Type::Number, "the level of the interval, in (0, 1) (0.95)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let mass: f64 = args.param("mass", f64::NAN)?;
        let mode: String = args.param("mode", "flying".to_string())?;
        let ci: f64 = args.param("ci_level", 0.95)?;
        let [v, lo, hi, kmh] = v_phys(ctx, node, mass, &mode, ci)?;
        let mut t = Table::new(["mass", "v_max", "lower", "upper", "kmh"].map(String::from).to_vec());
        t.rows.push(vec![mass, v, lo, hi, kmh]);
        Ok(crate::outputs([
            ("v_max", Value::Number(v)),
            ("lower", Value::Number(lo)),
            ("upper", Value::Number(hi)),
            ("kmh", Value::Number(kmh)),
            ("table", Value::Table(t)),
        ]))
    }
}

/// `move.flag_speed`.
struct FlagSpeed;

impl Operator for FlagSpeed {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.flag_speed",
                "Flag the locations next to a step faster than a speed limit (move2utils' mt_flag_speed_cap with a \
                 hard cap). The speed of a step is its length over its time lag (metres per second: haversine on a \
                 sphere of 6 371 008.8 m in longitude / latitude, the CRS's units per second otherwise). A location \
                 is flagged when the step leaving it or the step arriving at it is faster than the limit (the data \
                 cannot tell which end is wrong); locations without a time are left out of the track for this, steps \
                 with a zero or negative lag have no speed. The limit is v_max (m/s), or the allometric maximum of \
                 mass (kg) and mode (move.max_speed), with no margin added. jitter also flags the locations next to a \
                 step shorter than it (the noise floor of the device). Columns added: is_outlier (any flag), \
                 step_speed (of the step leaving the location, NaN at the last), is_speed_above_cap, \
                 is_step_below_jitter (with jitter), flagged_by_speed and loglr_speed (log(speed / the least flagged \
                 speed), one-sided); the flags are 1, 0 or NaN (no time). Empty locations must be dropped first \
                 (move.filter_gps_quality).",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks: points, each track's rows consecutive in time"))
            .output(PortSpec::new("tracks", Type::Vector, "the tracks with the columns of the flags"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .output(PortSpec::new("v_max", Type::Number, "the limit used, m/s"))
            .param(PortSpec::optional("v_max", Type::Number, "the limit, m/s (or the CRS's units per second)"))
            .param(PortSpec::optional("mass", Type::Number, "instead of v_max: the body mass (kg) of the limit"))
            .param(
                PortSpec::optional("mode", Type::Text, "with mass: flying, running or swimming (no default)")
                    .choices(MODES),
            )
            .param(PortSpec::optional("jitter", Type::Number, "also flag steps shorter than this (metres)"))
            .param(PortSpec::optional("remove", Type::Bool, "give only the locations not flagged (false)")),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let given: Option<f64> = args.opt_param("v_max")?;
        let mass: Option<f64> = args.opt_param("mass")?;
        let mode: Option<String> = args.opt_param::<String>("mode")?.filter(|m| !m.is_empty());
        let v_max = match (given, mass) {
            (Some(_), Some(_)) => return Err(fail(node, "give v_max, or mass and mode, not both")),
            (Some(x), None) if mode.is_none() => x,
            (Some(_), None) => return Err(fail(node, "give v_max, or mass and mode, not both")),
            (None, Some(m)) => match mode.as_deref() {
                Some(md) => v_phys(ctx, node, m, md, 0.95)?[0],
                None => return Err(fail(node, "with mass, give the mode too: flying, running or swimming")),
            },
            (None, None) => return Err(fail(node, "give v_max (m/s), or mass (kg) and mode")),
        };
        if !(v_max.is_finite() && v_max > 0.0) {
            return Err(fail(node, "v_max: a positive number (m/s)"));
        }
        let jitter: Option<f64> = args.opt_param("jitter")?;
        if jitter.is_some_and(|j| !(j.is_finite() && j >= 0.0)) {
            return Err(fail(node, "jitter: a non-negative number (metres) or none"));
        }
        let (t, k) = columns(args, v)?;
        let (x, y) = points(node, v, "flagging speeds")?;
        let code = codes(v.column(&k).unwrap());
        check_order(&code, None, false).map_err(|e| fail(node, e))?;
        let times = Times::of(v.column(&t).unwrap()).map_err(|e| fail(node, e))?;
        let lonlat = is_lonlat(v.crs.as_deref()).map_err(|e| fail(node, e))?;
        let r = terraflux_ls_sys::ffi::move_speed_cap(
            &x,
            &y,
            &posix_seconds(&times),
            &code,
            lonlat,
            v_max,
            jitter.unwrap_or(f64::NAN),
            threads(),
        )?;
        let outlier: Vec<bool> = r.above.iter().zip(&r.below).map(|(&a, &b)| a == 1 || b == 1).collect();
        let mut out = (**v).clone();
        set_column(&mut out, "is_outlier", mask(&outlier));
        set_column(&mut out, "step_speed", Column::Numbers(r.speed));
        set_column(&mut out, "is_speed_above_cap", flags(&r.above));
        if jitter.is_some() {
            set_column(&mut out, "is_step_below_jitter", flags(&r.below));
        }
        set_column(&mut out, "flagged_by_speed", mask(&outlier));
        set_column(&mut out, "loglr_speed", Column::Numbers(r.loglr));
        if args.param("remove", false)? {
            let rows: Vec<usize> = (0..out.len()).filter(|&i| !outlier[i]).collect();
            out = out.select(&rows);
        }
        let mut o = give("tracks", out);
        o.insert("v_max".into(), Value::Number(v_max));
        Ok(o)
    }
}

/// Movebank's column `gps_satellite_count` by its names: `gps_satellite_count`, `gps-satellite-count` and
/// `gps:satellite-count` (the one Movebank exports).
fn find_column<'a>(v: &'a Vector, underscore_name: &str) -> Option<&'a Column> {
    let dashed = underscore_name.replace('_', "-");
    let colon = match dashed.split_once('-') {
        Some((p, rest)) if !p.is_empty() && p.bytes().all(|b| b.is_ascii_lowercase()) => format!("{p}:{rest}"),
        _ => dashed.clone(),
    };
    [underscore_name.to_string(), dashed, colon].iter().find_map(|n| v.column(n))
}

/// A criterion's limit: the default when not given, none (off) when null or the text "none".
fn limit(args: &Args, name: &str, default: f64) -> Result<Option<f64>> {
    match args.params.get(name) {
        None => Ok(Some(default)),
        Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::String(s)) if s.is_empty() || s.eq_ignore_ascii_case("none") => Ok(None),
        Some(_) => Ok(Some(args.param(name, default)?)),
    }
}

/// `move.filter_gps_quality`.
struct GpsQuality;

impl Operator for GpsQuality {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.filter_gps_quality",
            "The tracks without the locations of an unreliable GPS geometry (move2utils' mt_filter_gps_quality): too \
             few satellites (a fix needs four, five is the usual floor), a high dilution of precision or a large \
             horizontal-accuracy estimate. The columns are Movebank's, found as gps:satellite-count, gps:dop and \
             eobs:horizontal-accuracy-estimate (also with underscores, gps_dop, or dashes, gps-dop); a missing \
             column skips its criterion, and so does none (null in JSON). Values that are missing or not numbers do not drop a \
             location. Also drops the locations without a position (drop_empty), which no detector can score.",
            Access::None,
        )
        .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
        .output(PortSpec::new("tracks", Type::Vector, "the locations kept"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .param(PortSpec::optional("sat_min", Type::Any, "keep at least this many satellites (5; none: no limit)"))
        .param(PortSpec::optional("dop_max", Type::Any, "keep a dilution of precision of at most this (10; none)"))
        .param(PortSpec::optional("hacc_max", Type::Any, "keep an accuracy estimate (m) of at most this (100; none)"))
        .param(PortSpec::optional("drop_empty", Type::Bool, "drop the locations without a position (true)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let v = tracks_of(args, "tracks")?;
        let mut keep = vec![true; v.len()];
        if args.param("drop_empty", true)? {
            for (k, g) in keep.iter_mut().zip(&v.geometries) {
                *k = !matches!(g, Geometry::Empty);
            }
        }
        for (column, lim, low) in [
            ("gps_satellite_count", limit(args, "sat_min", 5.0)?, true),
            ("gps_dop", limit(args, "dop_max", 10.0)?, false),
            ("eobs_horizontal_accuracy_estimate", limit(args, "hacc_max", 100.0)?, false),
        ] {
            let (Some(lim), Some(c)) = (lim, find_column(v, column)) else { continue };
            for (i, k) in keep.iter_mut().enumerate() {
                let x = match c {
                    Column::Numbers(x) => x[i],
                    Column::Texts(s) => s[i].trim().parse::<f64>().unwrap_or(f64::NAN),
                };
                // a missing value compares false: it does not drop the location
                if (low && x < lim) || (!low && x > lim) {
                    *k = false;
                }
            }
        }
        Ok(give("tracks", kept(v, |i| keep[i])))
    }
}

/// Seconds of a number, or a text such as `45 mins`, `45min`, `2 hours` (lubridate's units).
fn duration(v: &serde_json::Value, what: &str) -> std::result::Result<f64, String> {
    let bad = || format!("{what}: a number of seconds, or a text such as '45 mins'");
    let out = match v {
        serde_json::Value::Number(n) => n.as_f64().ok_or_else(bad)?,
        serde_json::Value::String(s) => {
            let s = s.trim();
            let split = s
                .find(|c: char| c.is_ascii_alphabetic())
                .ok_or_else(|| format!("{what}: cannot read {s:?} as a duration (e.g. '45 mins')"))?;
            let (num, unit) = (s[..split].trim(), s[split..].trim().to_lowercase());
            let digits = num.strip_prefix(['+', '-']).unwrap_or(num);
            let valid = !digits.is_empty()
                && digits.bytes().all(|b| b.is_ascii_digit() || b == b'.')
                && digits.bytes().filter(|&b| b == b'.').count() <= 1
                && digits.ends_with(|c: char| c.is_ascii_digit());
            let f = match unit.as_str() {
                "s" | "sec" | "secs" | "second" | "seconds" => 1.0,
                "min" | "mins" | "minute" | "minutes" => 60.0,
                "h" | "hr" | "hrs" | "hour" | "hours" => 3600.0,
                "d" | "day" | "days" => 86400.0,
                "w" | "week" | "weeks" => 604800.0,
                "month" | "months" => 2629800.0,
                "year" | "years" => 31557600.0,
                _ => 0.0,
            };
            if !valid || f == 0.0 {
                return Err(format!("{what}: cannot read {s:?} as a duration (e.g. '45 mins')"));
            }
            num.parse::<f64>().map_err(|_| bad())? * f
        }
        _ => return Err(bad()),
    };
    if out.is_finite() { Ok(out) } else { Err(format!("{what}: a finite duration")) }
}

/// What the thinning gives: the locations selected, or all with the column `thin_selected`.
fn thinned(args: &Args, v: &Vector, selected: &[bool]) -> Result<Outputs> {
    match args.param("output", "selected".to_string())?.as_str() {
        "selected" => Ok(give("tracks", kept(v, |i| selected[i]))),
        "flag" => {
            let mut out = v.clone();
            set_column(&mut out, "thin_selected", mask(selected));
            Ok(give("tracks", out))
        }
        o => Err(fail(args.node, format!("output {o:?}: selected or flag"))),
    }
}

fn criterion(args: &Args) -> Result<bool> {
    match args.param("criterion", "closest".to_string())?.as_str() {
        "closest" => Ok(true),
        "first" => Ok(false),
        _ => Err(fail(args.node, "criterion: \"closest\" or \"first\"")),
    }
}

fn output_param(s: OpSignature) -> OpSignature {
    s.param(
        PortSpec::optional(
            "output",
            Type::Text,
            "selected (default): only the locations kept; flag: all, with the column thin_selected (1 kept, 0 not)",
        )
        .choices(["selected", "flag"]),
    )
}

/// `move.thin_time`.
struct ThinTime;

impl Operator for ThinTime {
    fn signature(&self) -> OpSignature {
        output_param(column_params(
            OpSignature::new(
                "move.thin_time",
                "Thin each track so that the kept locations are about interval apart (move2utils' mt_thin_time, the \
                 move2 version of move::thinTrackTime): the longest chain of locations whose successive time lags are \
                 within interval +/- tolerance, by a dynamic programme over the locations sorted by time. A gap longer \
                 than interval + tolerance splits the track and is never bridged. It is not move.filter_per_interval: \
                 the interval is a lag between kept locations, not a clock bucket. criterion closest (default): of \
                 chains of the same length the least total deviation from interval; first: the one ending first. The \
                 geometry is not read; a track need not be sorted; locations without a time are never kept.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks"))
            .output(PortSpec::new("tracks", Type::Vector, "the locations kept (output flag: all)"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::new("interval", Type::Any, "the lag: seconds, or a text such as 45 mins or 2 hours"))
            .param(PortSpec::optional(
                "tolerance",
                Type::Any,
                "half-width of the accepted lags, in the same form (a tenth of the interval; at most the interval)",
            ))
            .param(
                PortSpec::optional("criterion", Type::Text, "closest (default) or first").choices(["closest", "first"]),
            ),
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let closest = criterion(args)?;
        let interval =
            duration(args.params.get("interval").ok_or_else(|| fail(node, "give the interval"))?, "interval")
                .map_err(|e| fail(node, e))?;
        let tolerance = match args.params.get("tolerance").filter(|t| !t.is_null()) {
            None => interval / 10.0,
            Some(t) => duration(t, "tolerance").map_err(|e| fail(node, e))?,
        };
        if interval <= 0.0 {
            return Err(fail(node, "interval: positive"));
        }
        if tolerance < 0.0 {
            return Err(fail(node, "tolerance: not negative"));
        }
        if tolerance > interval {
            return Err(fail(node, "tolerance: not above the interval"));
        }
        let (t, k) = columns(args, v)?;
        let n = v.len();
        let mut selected = vec![false; n];
        if n > 0 {
            let code = codes(v.column(&k).unwrap());
            let secs = posix_seconds(&Times::of(v.column(&t).unwrap()).map_err(|e| fail(node, e))?);
            // by track, then time, a missing time last (stable)
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&i, &j| code[i].cmp(&code[j]).then_with(|| super::nan_last(secs[i], secs[j])));
            let ts: Vec<f64> = order.iter().map(|&i| secs[i]).collect();
            let cs: Vec<i64> = order.iter().map(|&i| code[i]).collect();
            let sel = terraflux_ls_sys::ffi::move_thin_time(&ts, &cs, interval, tolerance, closest, threads())?;
            for (s, &i) in sel.iter().zip(&order) {
                selected[i] = *s != 0;
            }
        }
        thinned(args, v, &selected)
    }
}

/// `move.thin_distance`.
struct ThinDistance;

impl Operator for ThinDistance {
    fn signature(&self) -> OpSignature {
        output_param(
            OpSignature::new(
                "move.thin_distance",
                "Thin each track so that the kept locations are about distance metres of travel apart (move2utils' \
                 mt_thin_distance). The distance of each step is move.distance's, in metres whatever the CRS's unit, \
                 added along the track in the order of the rows. method step (default): the first location, then each \
                 one at which the travel since the last kept one reaches distance; interval: the longest chain whose \
                 successive travel is within distance +/- tolerance, by the dynamic programme of move.thin_time over \
                 the cumulative distance (a single step longer than distance + tolerance splits the track). Empty \
                 locations must be dropped first (move.filter_gps_quality).",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks: points"))
            .output(PortSpec::new("tracks", Type::Vector, "the locations kept (output flag: all)"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::new("distance", Type::Number, "the travel between kept locations, metres"))
            .param(PortSpec::optional("method", Type::Text, "step (default) or interval").choices(["step", "interval"]))
            .param(PortSpec::optional(
                "tolerance",
                Type::Number,
                "interval: half-width of the accepted travel, metres (a tenth of the distance; at most the distance)",
            ))
            .param(
                PortSpec::optional("criterion", Type::Text, "interval: closest (default) or first")
                    .choices(["closest", "first"]),
            )
            .param(PortSpec::optional(
                "track",
                Type::Text,
                "the column of the track (default: individual-tag-local-identifier, individual-local-identifier, \
                 track, track_id, id, individual or animal)",
            )),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let method: String = args.param("method", "step".to_string())?;
        if method != "step" && method != "interval" {
            return Err(fail(node, "method: \"step\" or \"interval\""));
        }
        let closest = criterion(args)?;
        let distance: f64 = args.param("distance", f64::NAN)?;
        if !(distance.is_finite() && distance > 0.0) {
            return Err(fail(node, "distance: a positive number (metres)"));
        }
        let given: Option<f64> = args.opt_param("tolerance")?;
        let mut tol = 0.0;
        if method == "interval" {
            tol = given.unwrap_or(distance / 10.0);
            if tol < 0.0 {
                return Err(fail(node, "tolerance: not negative"));
            }
            if tol > distance {
                return Err(fail(node, "tolerance: not above the distance"));
            }
        } else if given.is_some() {
            ctx.warn(format!(
                "node {node:?}: tolerance is ignored when method is \"step\"; it applies only to \"interval\""
            ));
        }
        let k = track_column(args, v)?;
        let n = v.len();
        let mut selected = vec![false; n];
        if n > 0 {
            let (x, y) = points(node, v, "thinning by distance")?;
            let lonlat = is_lonlat(v.crs.as_deref()).map_err(|e| fail(node, e))?;
            let code = codes(v.column(&k).unwrap());
            // the rows of each track, in their order
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by_key(|&i| code[i]);
            let xs: Vec<f64> = order.iter().map(|&i| x[i]).collect();
            let ys: Vec<f64> = order.iter().map(|&i| y[i]).collect();
            let cs: Vec<i64> = order.iter().map(|&i| code[i]).collect();
            let mut step = terraflux_ls_sys::ffi::move_distances(&xs, &ys, &cs, lonlat, threads())?;
            let f = length_factor(Some("m"), v.crs.as_deref(), lonlat).map_err(|e| fail(node, e))?;
            if f != 1.0 {
                step.iter_mut().for_each(|s| *s /= f);
            }
            let method = i32::from(method == "interval");
            let sel = terraflux_ls_sys::ffi::move_thin_distance(&step, &cs, method, distance, tol, closest, threads())?;
            for (s, &i) in sel.iter().zip(&order) {
                selected[i] = *s != 0;
            }
        }
        thinned(args, v, &selected)
    }
}
