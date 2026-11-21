//! The dynamic Brownian bridge movement model and its directional kin (the library's `tf.move`, from R's move2utils
//! 0.4.6, Kranstauber, Safi and Scharf 2026, on the C kernels of the `move` package, Kranstauber et al. 2012): the motion
//! variance of each track as columns of its locations, the utilization distribution (a band per track or burst), the
//! volume of a distribution, the probability of locations outside it, the window to give the variance and the masking of
//! bridges. The kernels are the library's C++ (`terraflux-ls-sys`); the windows of the variance and the strips of the
//! grid run in parallel there and are added in a fixed order, so the result does not depend on the number of threads.
//! Tracks (or bursts) of many run side by side on one thread each, as the library does, within the central cap.

use std::collections::HashMap;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::homerange::{Res, ffi_err, geometry_of, geos, isopleths, labels_table, like_grid, number_label, shape_area};
use super::homerange::{Spec, spec_of};
use super::tracks_of;
use super::{BURST, Key as ColKey, Times, check_order, codes, column_params, columns, fail, give, set_column, threads};
use crate::{Args, Ctx, Key, Operator, Outputs, Registry, Result, Stack, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Variance(Model::Dbbmm)));
    r.add(Arc::new(Variance(Model::Dbgb)));
    r.add(Arc::new(UdOp(Model::Dbbmm)));
    r.add(Arc::new(UdOp(Model::Dbgb)));
    r.add(Arc::new(SuggestWindow));
    r.add(Arc::new(MaskSegments));
    r.add(Arc::new(UdVolume));
    r.add(Arc::new(UdOuterProbability));
}

const EXT_BBMM: f64 = 4.0;
const EXT_DBGB: f64 = 5.0;
const NA_HOW: [&str; 4] = ["median", "mean", "zero", "approx"];
/// The columns the variance adds, by model (the last is the location error used).
const BBMM_COLUMNS: [&str; 3] = ["motion_variance", "in_windows", "interest"];
const DBGB_COLUMNS: [&str; 4] = ["para_sd", "orth_sd", "n_estim", "seg_interest"];
const BBMM_ERROR: &str = "location_error";
const DBGB_ERROR: &str = "dbgb_location_error";

#[derive(Clone, Copy, PartialEq)]
enum Model {
    Dbbmm,
    Dbgb,
}

impl Model {
    fn name(self) -> &'static str {
        match self {
            Model::Dbbmm => "dbbmm",
            Model::Dbgb => "dbgb",
        }
    }

    fn margin(self) -> i64 {
        match self {
            Model::Dbbmm => 11,
            Model::Dbgb => 15,
        }
    }

    fn code(self) -> i32 {
        match self {
            Model::Dbbmm => 0,
            Model::Dbgb => 1,
        }
    }

    fn error_column(self) -> &'static str {
        match self {
            Model::Dbbmm => BBMM_ERROR,
            Model::Dbgb => DBGB_ERROR,
        }
    }

    fn columns(self) -> &'static [&'static str] {
        match self {
            Model::Dbbmm => &BBMM_COLUMNS,
            Model::Dbgb => &DBGB_COLUMNS,
        }
    }

    /// The column of the segments that count.
    fn interest_column(self) -> &'static str {
        match self {
            Model::Dbbmm => "interest",
            Model::Dbgb => "seg_interest",
        }
    }
}

// ------------------------------------------------------------------------------------------------ the tracks

/// A track, or a burst of one, with what the bridge needs in the computing coordinates.
struct Unit {
    label: String,
    start: usize,
    stop: usize,
    x: Vec<f64>,
    y: Vec<f64>,
    /// the times in minutes since 1970
    tm: Vec<f64>,
    loc_err: Option<Vec<f64>>,
    /// the variance (dBBMM) or the standard deviation along the bridge (dBGB), NaN where no window covers
    var: Option<Vec<f64>>,
    /// the standard deviation across the bridge (dBGB)
    orth: Vec<f64>,
    /// the windows that cover each location (NaN where none)
    windows: Vec<f64>,
    /// the segments that count
    interest: Vec<bool>,
}

/// The locations of validated tracks.
struct Prep {
    x: Vec<f64>,
    y: Vec<f64>,
    crs: Option<String>,
    lonlat: bool,
    code: Vec<i64>,
    labels: Vec<String>,
    secs: Vec<f64>,
}

fn column_label(c: &Column, i: usize) -> String {
    match c {
        Column::Texts(t) => t[i].clone(),
        Column::Numbers(n) => number_label(n[i]),
    }
}

/// The column of the bursts: `burst` is auto (the column of move.resample, if there), none, or a column.
fn burst_column(args: &Args, v: &Vector) -> Result<Option<String>> {
    let bad = || fail(args.node, "burst: auto, none or the name of a column");
    let name = match args.params.get("burst") {
        None | Some(serde_json::Value::Null) => "auto".to_string(),
        Some(serde_json::Value::Bool(true)) => "auto".to_string(),
        Some(serde_json::Value::Bool(false)) => "none".to_string(),
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(_) => return Err(bad()),
    };
    match name.as_str() {
        "none" | "" => Ok(None),
        "auto" => Ok(v.column(BURST).map(|_| BURST.to_string())),
        n if v.column(n).is_some() => Ok(Some(n.to_string())),
        n => Err(fail(args.node, format!("burst column {n:?} not found"))),
    }
}

/// The seconds of the times as the library takes them: the whole seconds and the fraction apart.
fn seconds_of(t: &Times) -> Vec<f64> {
    match t {
        Times::Numbers(x) => x.clone(),
        Times::Instants(x) => x
            .iter()
            .map(|t| match t {
                None => f64::NAN,
                Some(ns) => {
                    let whole = ns.div_euclid(1_000_000_000);
                    whole as f64 + (ns - whole * 1_000_000_000) as f64 / 1e9
                }
            })
            .collect(),
    }
}

fn prepare(args: &Args, v: &Vector, what: &str) -> Result<Prep> {
    let node = args.node;
    if v.is_empty() {
        return Err(fail(node, "tracks: no locations"));
    }
    let (mut x, mut y) = (Vec::with_capacity(v.len()), Vec::with_capacity(v.len()));
    for g in &v.geometries {
        match g {
            Geometry::Point(p) => {
                x.push(p[0]);
                y.push(p[1]);
            }
            Geometry::Empty => {
                return Err(fail(node, format!("some locations are empty: drop them before {what} (vector.filter)")));
            }
            _ => return Err(fail(node, "the geometry must be points")),
        }
    }
    if !(x.iter().all(|a| a.is_finite()) && y.iter().all(|a| a.is_finite())) {
        return Err(fail(node, format!("some locations have non-finite coordinates: remove them before {what}")));
    }
    let lonlat = super::is_lonlat(v.crs.as_deref()).map_err(|e| fail(node, e))?;
    let (t, k) = columns(args, v)?;
    let tcol = v.column(&t).unwrap();
    let kcol = v.column(&k).unwrap();
    let tcode = codes(kcol);
    let secs = seconds_of(&Times::of(tcol).map_err(|e| fail(node, e))?);
    if secs.iter().any(|s| s.is_nan()) {
        return Err(fail(node, format!("some locations have no time: remove them before {what}")));
    }
    let n = v.len();
    let burst = burst_column(args, v)?;
    let (code, first): (Vec<i64>, Vec<usize>) = match &burst {
        None => {
            let k = tcode.iter().max().map_or(0, |&m| m as usize + 1);
            let mut first = vec![usize::MAX; k];
            for (i, &c) in tcode.iter().enumerate() {
                if first[c as usize] == usize::MAX {
                    first[c as usize] = i;
                }
            }
            (tcode, first)
        }
        Some(b) => {
            let bcol = v.column(b).unwrap();
            if (0..n).any(|i| ColKey::of(bcol, i) == ColKey::Na) {
                return Err(fail(
                    node,
                    format!("the burst column {b:?} has missing values: drop those locations first"),
                ));
            }
            let bcode = codes(bcol);
            let mut seen: HashMap<(i64, i64), i64> = HashMap::new();
            let mut first = Vec::new();
            let mut code = Vec::with_capacity(n);
            for i in 0..n {
                let next = seen.len() as i64;
                let c = *seen.entry((tcode[i], bcode[i])).or_insert_with(|| {
                    first.push(i);
                    next
                });
                code.push(c);
            }
            (code, first)
        }
    };
    let tracks: Vec<String> = first.iter().map(|&i| column_label(kcol, i)).collect();
    let bursts: Vec<Option<String>> = match &burst {
        None => vec![None; first.len()],
        Some(b) => first.iter().map(|&i| Some(column_label(v.column(b).unwrap(), i))).collect(),
    };
    let labels = tracks
        .iter()
        .zip(&bursts)
        .map(|(a, b)| match b {
            None => a.clone(),
            Some(b) => format!("{a}__{b}"),
        })
        .collect();
    Ok(Prep { x, y, crs: v.crs.clone(), lonlat, code, labels, secs })
}

/// [start, stop) of each unit (their rows consecutive).
fn bounds(code: &[i64]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut s = 0;
    for i in 1..=code.len() {
        if i == code.len() || code[i] != code[i - 1] {
            out.push((s, i));
            s = i;
        }
    }
    out
}

fn units_from(p: &Prep, x: &[f64], y: &[f64]) -> Res<Vec<Unit>> {
    check_order(&p.code, Some(&p.secs), true)?;
    Ok(bounds(&p.code)
        .into_iter()
        .enumerate()
        .map(|(u, (s, e))| Unit {
            label: p.labels[u].clone(),
            start: s,
            stop: e,
            x: x[s..e].to_vec(),
            y: y[s..e].to_vec(),
            tm: p.secs[s..e].iter().map(|t| t / 60.0).collect(),
            loc_err: None,
            var: None,
            orth: Vec::new(),
            windows: Vec::new(),
            interest: Vec::new(),
        })
        .collect())
}

// ----------------------------------------------------------------------------------------------------- the CRS

fn same_crs(a: &str, b: &str) -> bool {
    a == b || terraflux_gdal::same_crs(a, b).unwrap_or(false)
}

/// A CRS the bridge can run in: none (plain coordinates), or projected with the metre as unit.
fn suitable(crs: Option<&str>) -> Res<bool> {
    let Some(c) = crs else { return Ok(true) };
    if terraflux_gdal::is_geographic(c)? {
        return Ok(false);
    }
    Ok((terraflux_gdal::linear_units(c)? - 1.0).abs() < 1e-9)
}

fn transform_xy(x: &[f64], y: &[f64], src: Option<&str>, dst: Option<&str>) -> Res<(Vec<f64>, Vec<f64>)> {
    let (mut ox, mut oy) = (x.to_vec(), y.to_vec());
    if let (Some(s), Some(d)) = (src, dst)
        && !same_crs(s, d)
    {
        terraflux_gdal::Transform::new(s, d)?.points(&mut ox, &mut oy);
        if !(ox.iter().all(|a| a.is_finite()) && oy.iter().all(|a| a.is_finite())) {
            return Err("some locations cannot be projected to the computing coordinate system".into());
        }
    }
    Ok((ox, oy))
}

/// The local projection of the tracks (`move.aeqd_crs`): centred on their centroid or on the middle of their ranges.
fn local_crs(v: &Vector, how: &str) -> Res<String> {
    super::geometry::aeqd(v, &how.into(), "m")
}

// ------------------------------------------------------------------------------------------ the location error

enum Resolved {
    Nothing,
    Scalar(f64),
    Values(Vec<f64>),
}

/// Movebank's column by its names: with underscores, with dashes or with the colon Movebank exports.
fn find_column(v: &Vector, name: &str) -> Option<String> {
    let dashed = name.replace('_', "-");
    let colon = match dashed.find('-') {
        Some(i) if i > 0 && dashed[..i].bytes().all(|b| b.is_ascii_lowercase()) => {
            format!("{}:{}", &dashed[..i], &dashed[i + 1..])
        }
        _ => dashed.clone(),
    };
    [name.to_string(), dashed, colon].into_iter().find(|n| v.column(n).is_some())
}

fn clean_sigma(c: &Column) -> Vec<f64> {
    let raw: Vec<f64> = match c {
        Column::Numbers(x) => x.clone(),
        Column::Texts(t) => t.iter().map(|s| s.trim().parse::<f64>().unwrap_or(f64::NAN)).collect(),
    };
    raw.into_iter().map(|s| if !s.is_finite() || s < 0.0 { f64::NAN } else { s }).collect()
}

/// Argos' location classes: the standard deviation in metres.
fn argos_sigma(c: &Column) -> Vec<f64> {
    let class = |s: &str| match s.to_uppercase().as_str() {
        "3" => 250.0,
        "2" => 500.0,
        "1" => 1500.0,
        "0" | "A" => 5000.0,
        "B" => 10000.0,
        _ => f64::NAN,
    };
    match c {
        Column::Texts(t) => t.iter().map(|s| class(s)).collect(),
        Column::Numbers(x) => x
            .iter()
            .map(|&v| {
                if v.is_nan() {
                    f64::NAN
                } else if v.fract() == 0.0 {
                    class(&format!("{}", v as i64))
                } else {
                    class(&format!("{v}"))
                }
            })
            .collect(),
    }
}

/// `location_error`: none, a number (metres), `auto` or the name of a column.
fn resolve_error(args: &Args, ctx: &mut Ctx, v: &Vector) -> Result<Resolved> {
    let node = args.node;
    let bad = || fail(node, "location_error: none, a number (metres), a column of the tracks or auto");
    match args.params.get("location_error") {
        None | Some(serde_json::Value::Null) => Ok(Resolved::Nothing),
        Some(serde_json::Value::Number(n)) => {
            let e = n.as_f64().ok_or_else(bad)?;
            if !e.is_finite() || e < 0.0 {
                return Err(fail(node, "location_error: a number must be non-negative and finite"));
            }
            Ok(Resolved::Scalar(e))
        }
        Some(serde_json::Value::String(s)) if s.is_empty() || s == "none" => Ok(Resolved::Nothing),
        Some(serde_json::Value::String(s)) if s == "auto" => {
            if let Some(c) = find_column(v, "eobs_horizontal_accuracy_estimate") {
                return Ok(Resolved::Values(clean_sigma(v.column(&c).unwrap())));
            }
            if let Some(c) = find_column(v, "argos_lc") {
                return Ok(Resolved::Values(argos_sigma(v.column(&c).unwrap())));
            }
            ctx.warn(format!(
                "node {node:?}: location_error auto: no recognised quality column (eobs_horizontal_accuracy_estimate, \
                 argos_lc) found, no location error is used"
            ));
            Ok(Resolved::Nothing)
        }
        Some(serde_json::Value::String(s)) => {
            let c = if v.column(s).is_some() { Some(s.clone()) } else { find_column(v, s) };
            let c = c.ok_or_else(|| {
                fail(node, format!("location_error names a column ({s:?}) that is not in the tracks"))
            })?;
            Ok(Resolved::Values(clean_sigma(v.column(&c).unwrap())))
        }
        Some(_) => Err(bad()),
    }
}

/// The error of one unit: zeros for none, the number repeated, or its slice with the missing values filled.
fn error_of(r: &Resolved, start: usize, stop: usize, how: i32, name: &str) -> Res<Vec<f64>> {
    let n = stop - start;
    match r {
        Resolved::Nothing => Ok(vec![0.0; n]),
        Resolved::Scalar(s) => Ok(vec![*s; n]),
        Resolved::Values(v) => {
            let e = &v[start..stop];
            if !e.iter().any(|a| a.is_nan()) {
                return Ok(e.to_vec());
            }
            let f = ffi::move_impute_loc_err(e, how);
            match f.status {
                1 => Err(format!(
                    "location_error is entirely missing (n = {n}, {name}): it cannot be filled with \
                     location_error_na = {}; zero only if no error at all is intended",
                    NA_HOW[how as usize]
                )),
                2 => Err("location_error_na approx needs at least two known errors in the track".into()),
                _ => Ok(f.values),
            }
        }
    }
}

// -------------------------------------------------------------------------------------------------- the variance

struct Window {
    size: i32,
    margin: i32,
}

fn window_of(args: &Args, model: Model) -> Result<Window> {
    let node = args.node;
    let size: i64 = args.param("window_size", 31)?;
    let margin: i64 = args.param("margin", model.margin())?;
    if size % 2 != 1 || margin % 2 != 1 || margin < 1 {
        return Err(fail(node, "margin and window_size must both be odd (and positive)"));
    }
    if size < 2 * margin + 1 {
        return Err(fail(node, "window_size must be at least 2 * margin + 1"));
    }
    if size > i64::from(i32::MAX) {
        return Err(fail(node, "window_size: too large"));
    }
    Ok(Window { size: size as i32, margin: margin as i32 })
}

fn na_how_of(args: &Args) -> Result<i32> {
    let s: String = args.param("location_error_na", "median".to_string())?;
    NA_HOW
        .iter()
        .position(|h| *h == s)
        .map(|i| i as i32)
        .ok_or_else(|| fail(args.node, "location_error_na: median, mean, zero or approx"))
}

/// `fn(item, threads)` of each item: side by side, one thread each, with at least as many items as threads; else one
/// after the other, each on all the threads. Every item is computed on its own, so the bits are the same either way.
fn each<T: Sync, R: Send>(
    items: &[T],
    threads: i32,
    side_by_side: bool,
    f: impl Fn(&T, i32) -> R + Sync + Send,
) -> Vec<R> {
    if side_by_side && threads > 1 && items.len() >= threads as usize {
        match rayon::ThreadPoolBuilder::new().num_threads(threads as usize).build() {
            Ok(pool) => return pool.install(|| items.par_iter().map(|it| f(it, 1)).collect()),
            Err(_) => return items.iter().map(|it| f(it, threads)).collect(),
        }
    }
    items.iter().map(|it| f(it, threads)).collect()
}

enum Fit {
    Bbmm { variance: Vec<f64>, n_estim: Vec<i32> },
    Dbgb { para: Vec<f64>, orth: Vec<f64>, n_estim: Vec<i32> },
}

fn fit_one(model: Model, u: &Unit, w: &Window, th: i32) -> Res<Fit> {
    let le = u.loc_err.as_ref().expect("the error of a unit");
    match model {
        Model::Dbgb => {
            let r = ffi::move_dbgb_variance(&u.x, &u.y, &u.tm, le, w.size, w.margin, th).map_err(ffi_err)?;
            Ok(Fit::Dbgb { para: r.para_sd, orth: r.orth_sd, n_estim: r.n_estim })
        }
        Model::Dbbmm => {
            let mut lag: Vec<f64> = u.tm.windows(2).map(|p| p[1] - p[0]).collect();
            lag.push(0.0);
            let r = ffi::move_dbbmm_variance(&u.x, &u.y, &lag, le, w.size, w.margin, th).map_err(ffi_err)?;
            Ok(Fit::Bbmm { variance: r.variance, n_estim: r.n_estim })
        }
    }
}

/// The windows that cover each location (NaN where none), and the locations every window covers.
fn windows_and_interest(ne: &[i32]) -> (Vec<f64>, Vec<bool>) {
    let windows = ne.iter().map(|&n| if n > 0 { f64::from(n) } else { f64::NAN }).collect();
    let max = ne.iter().copied().max().unwrap_or(0);
    let interest = ne.iter().map(|&n| n > 0 && n == max).collect();
    (windows, interest)
}

/// The variance of each unit with enough locations (the others stay without one, as skipped).
#[allow(clippy::too_many_arguments)]
fn fit_variance(
    ctx: &mut Ctx,
    node: &str,
    model: Model,
    units: &mut [Unit],
    resolved: &Resolved,
    how: i32,
    w: &Window,
    threads: i32,
) -> Res<()> {
    let enough = |u: &Unit| u.x.len() >= w.size as usize;
    let skipped: Vec<&str> = units.iter().filter(|u| !enough(u)).map(|u| u.label.as_str()).collect();
    if !skipped.is_empty() && units.len() == 1 {
        return Err(format!(
            "window_size ({}) cannot be larger than the number of locations ({})",
            w.size,
            units[0].x.len()
        ));
    }
    if !skipped.is_empty() {
        if skipped.len() == units.len() {
            return Err("No track had enough locations for the given window_size".into());
        }
        ctx.warn(format!("node {node:?}: tracks skipped (fewer locations than window_size): {}", skipped.join(", ")));
    }
    for u in units.iter_mut().filter(|u| u.x.len() >= w.size as usize) {
        u.loc_err = Some(error_of(resolved, u.start, u.stop, how, &u.label)?);
    }
    let todo: Vec<usize> = (0..units.len()).filter(|&i| units[i].loc_err.is_some()).collect();
    let fits = {
        let view: Vec<&Unit> = todo.iter().map(|&i| &units[i]).collect();
        each(&view, threads, true, |u, th| fit_one(model, u, w, th))
    };
    for (&i, fit) in todo.iter().zip(fits) {
        let u = &mut units[i];
        match fit? {
            Fit::Bbmm { variance, n_estim } => {
                let (windows, interest) = windows_and_interest(&n_estim);
                u.var = Some(variance);
                u.windows = windows;
                u.interest = interest;
            }
            Fit::Dbgb { para, orth, n_estim } => {
                let (windows, interest) = windows_and_interest(&n_estim);
                u.var = Some(para);
                u.orth = orth;
                u.windows = windows;
                u.interest = interest;
            }
        }
    }
    Ok(())
}

fn flag(x: &[bool]) -> Column {
    Column::Numbers(x.iter().map(|&f| f64::from(u8::from(f))).collect())
}

/// `move.dbbmm_variance` and `move.dbgb_variance`.
struct Variance(Model);

impl Operator for Variance {
    fn signature(&self) -> OpSignature {
        let (name, doc) = match self.0 {
            Model::Dbbmm => (
                "move.dbbmm_variance",
                "The motion variance of the dynamic Brownian bridge along each track (move2utils' mt_dbbmm_variance, \
                 Kranstauber et al. 2012): a window of window_size locations slides along the track; in each, the \
                 Brownian motion variance that best explains the locations between their neighbours (every second one \
                 left out in turn, as a bridge between its neighbours) is found by maximum likelihood, and so is one \
                 with a break point in it, kept when it lowers the BIC. The variance of a location is the mean over \
                 the windows that cover it, margin - 1 locations at the start and margin at the end have none. \
                 window_size and margin are odd, window_size at least 2 margin + 1 (move.suggest_window proposes them \
                 from the sampling); a track with fewer locations than window_size is skipped, with a warning (an \
                 error when it is the only one). location_error is the standard deviation of the location error, in \
                 metres: a number, the name of a column, or auto (the column eobs_horizontal_accuracy_estimate, else \
                 the classes of argos_lc: 3 250 m, 2 500 m, 1 1500 m, 0 and A 5000 m, B 10000 m); it matters, because \
                 a real error taken for movement makes the variance (and the distribution) too large; missing values \
                 are filled in each track by location_error_na (median, mean, zero or approx, along the track). \
                 Longitude / latitude is projected to the local azimuthal equidistant projection of move.aeqd_crs \
                 (centred on the centroid) for the computation; the variance is in m^2 per minute (the CRS's unit \
                 squared for other projections). Columns added to the locations: motion_variance (empty where no \
                 window covers), in_windows (the windows that cover), interest (1 where every window covers: the \
                 segments of the distribution, which move.mask_segments can leave out of it) and location_error (the \
                 errors used). With bursts (move.resample) each burst is a track of its own.",
            ),
            Model::Dbgb => (
                "move.dbgb_variance",
                "The variances of the directional Brownian bridge along and across the movement (move2utils' \
                 mt_dbgb_variance): as move.dbbmm_variance, but each leave-one-out residual of a window is split along \
                 the direction from the expected position to the next location (the parallel part) and across it (the \
                 orthogonal part); each axis has a variance of its own and up to one break point of its own, found by \
                 the BIC in four steps (none, the parallel axis alone, the orthogonal alone, the second given the \
                 first). The standard deviation of a location is the root of the mean variance of the windows that \
                 cover it. margin is 15 by default (odd, window_size at least 2 margin + 1). Columns added: para_sd \
                 and orth_sd (standard deviations, empty where no window covers), n_estim (the windows that cover), \
                 seg_interest (1 where every window covers: the segments of the distribution, which \
                 move.mask_segments can leave out) and dbgb_location_error (the errors used). The dBGB suits movement \
                 with a direction (a persistent walk); it needs a long track: windows of 31 locations with margins of \
                 15 leave a core of few. The other settings are those of move.dbbmm_variance.",
            ),
        };
        let margin_doc = match self.0 {
            Model::Dbbmm => "odd, window_size at least 2 margin + 1 (11)",
            Model::Dbgb => "odd, window_size at least 2 margin + 1 (15)",
        };
        column_params(
            OpSignature::new(name, doc, Access::None)
                .input(PortSpec::new(
                    "tracks",
                    Type::Vector,
                    "the tracks: points, each track's rows consecutive in time",
                ))
                .output(PortSpec::new("tracks", Type::Vector, "the tracks with the columns of the variance"))
                .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
                .param(PortSpec::optional(
                    "location_error",
                    Type::Any,
                    "the standard deviation of the location error, metres: none, a number, a column or auto",
                ))
                .param(PortSpec::optional("window_size", Type::Integer, "the locations of a window, odd (31)"))
                .param(PortSpec::optional("margin", Type::Integer, margin_doc))
                .param(
                    PortSpec::optional("location_error_na", Type::Text, "what a missing error becomes (median)")
                        .choices(NA_HOW),
                )
                .param(PortSpec::optional(
                    "burst",
                    Type::Text,
                    "auto (default: the bursts of move.resample are tracks of their own), none, or a column",
                )),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let model = self.0;
        let v = tracks_of(args, "tracks")?;
        let w = window_of(args, model)?;
        let how = na_how_of(args)?;
        let resolved = resolve_error(args, ctx, v)?;
        let tag = |e: String| fail(node, e);
        let p = prepare(args, v, "estimating the variance")?;
        let work = if p.lonlat { Some(local_crs(v, "centroid").map_err(tag)?) } else { p.crs.clone() };
        let (px, py) = transform_xy(&p.x, &p.y, p.crs.as_deref(), work.as_deref()).map_err(tag)?;
        let mut units = units_from(&p, &px, &py).map_err(tag)?;
        fit_variance(ctx, node, model, &mut units, &resolved, how, &w, threads()).map_err(tag)?;
        let n = v.len();
        let nan = || vec![f64::NAN; n];
        let (mut c0, mut c1, mut c2, mut ce) = (nan(), nan(), nan(), nan());
        let mut interest = vec![false; n];
        for u in units.iter().filter(|u| u.var.is_some()) {
            let r = u.start..u.stop;
            c0[r.clone()].copy_from_slice(u.var.as_ref().unwrap());
            if model == Model::Dbgb {
                c1[r.clone()].copy_from_slice(&u.orth);
            }
            c2[r.clone()].copy_from_slice(&u.windows);
            ce[r.clone()].copy_from_slice(u.loc_err.as_ref().unwrap());
            interest[r].copy_from_slice(&u.interest);
        }
        let mut out = (**v).clone();
        match model {
            Model::Dbbmm => {
                set_column(&mut out, "motion_variance", Column::Numbers(c0));
                set_column(&mut out, "in_windows", Column::Numbers(c2));
                set_column(&mut out, "interest", flag(&interest));
            }
            Model::Dbgb => {
                set_column(&mut out, "para_sd", Column::Numbers(c0));
                set_column(&mut out, "orth_sd", Column::Numbers(c1));
                set_column(&mut out, "n_estim", Column::Numbers(c2));
                set_column(&mut out, "seg_interest", flag(&interest));
            }
        }
        set_column(&mut out, model.error_column(), Column::Numbers(ce));
        Ok(give("tracks", out))
    }
}

// ----------------------------------------------------------------------------------------------------- the grid

/// The grid of a distribution: terra's extent and counts.
#[derive(Clone, Copy, Debug)]
struct G {
    xmin: f64,
    xmax: f64,
    ymin: f64,
    ymax: f64,
    nrow: usize,
    ncol: usize,
}

impl G {
    fn xres(&self) -> f64 {
        (self.xmax - self.xmin) / self.ncol as f64
    }

    fn yres(&self) -> f64 {
        (self.ymax - self.ymin) / self.nrow as f64
    }

    /// The centres of the columns and of the rows, both ascending (the rows from the bottom up, as the kernel takes them).
    fn vectors(&self) -> (Vec<f64>, Vec<f64>) {
        let (xr, yr) = (self.xres(), self.yres());
        let xs = (0..self.ncol).map(|i| self.xmin + (i as f64 + 0.5) * xr).collect();
        let ys = (0..self.nrow).map(|k| self.ymax - (((self.nrow - 1 - k) as f64 + 0.5) * yr)).collect();
        (xs, ys)
    }

    fn grid(&self, wkt: Option<&String>) -> Grid {
        let mut g = Grid::new(self.ncol, self.nrow);
        g.transform = [self.xmin, self.xres(), 0.0, self.ymax, 0.0, -self.yres()];
        g.geo.wkt = wkt.cloned();
        g
    }
}

fn min_max(x: &[f64]) -> (f64, f64) {
    (x.iter().copied().fold(f64::INFINITY, f64::min), x.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

/// The bounding box of the points widened by `ext` times its width and height on each side.
fn extcalc(px: &[f64], py: &[f64], ext: f64) -> [f64; 4] {
    let (xmin, xmax) = min_max(px);
    let (ymin, ymax) = min_max(py);
    let (xr, yr) = (xmax - xmin, ymax - ymin);
    [xmin - ext * xr, xmax + ext * xr, ymin - ext * yr, ymax + ext * yr]
}

/// `.make_raster`: the widened box with its sides rounded up to whole cells, the excess split evenly.
fn make_grid(px: &[f64], py: &[f64], cell_size: Option<f64>, dim_size: Option<usize>, ext: f64) -> Res<G> {
    let [xmin, xmax, ymin, ymax] = extcalc(px, py, ext);
    let (xr, yr) = (xmax - xmin, ymax - ymin);
    let cell = cell_size.unwrap_or_else(|| xr.max(yr) / dim_size.unwrap_or(100) as f64);
    if !(cell > 0.0 && cell.is_finite()) {
        return Err("the locations have no extent to build a grid on: give like".into());
    }
    let ymin2 = ymin - ((yr / cell).ceil() * cell - yr) / 2.0;
    let ymax2 = ymax + ((yr / cell).ceil() * cell - yr) / 2.0;
    let xmin2 = xmin - ((xr / cell).ceil() * cell - xr) / 2.0;
    let xmax2 = xmax + ((xr / cell).ceil() * cell - xr) / 2.0;
    let nr = ((ymax2 - ymin2) / cell).round_ties_even();
    let nc = ((xmax2 - xmin2) / cell).round_ties_even();
    if nr < 1.0 || nc < 1.0 {
        return Err(
            "the grid would have no cells: the locations are on a line, give like or a larger ext_factor".into()
        );
    }
    Ok(G { xmin: xmin2, xmax: xmax2, ymin: ymin2, ymax: ymax2, nrow: nr as usize, ncol: nc as usize })
}

/// The extent a grid of cells of size `cell` needs so that every window stays inside it.
fn needed_range(b: [f64; 4], env: [f64; 4], cell: f64, extra: f64) -> [f64; 4] {
    [
        b[0].min(env[0] - (0.5 + extra) * cell),
        b[1].max(env[1] + (0.5 + extra) * cell),
        b[2].min(env[2] - (1.5 + extra) * cell),
        b[3].max(env[3] + (0.5 + extra) * cell),
    ]
}

fn grid_from_range(r: [f64; 4], cell: f64) -> G {
    let (xr, yr) = (r[1] - r[0], r[3] - r[2]);
    let nc = ((xr / cell - 1e-9).ceil() as i64).max(1);
    let nr = ((yr / cell - 1e-9).ceil() as i64).max(1);
    let xpad = (nc as f64 * cell - xr) / 2.0;
    let ypad = (nr as f64 * cell - yr) / 2.0;
    G {
        xmin: r[0] - xpad,
        xmax: r[1] + xpad,
        ymin: r[2] - ypad,
        ymax: r[3] + ypad,
        nrow: nr as usize,
        ncol: nc as usize,
    }
}

/// The inputs of the kernel for one unit (the same for the probe and the distribution).
struct Kernel {
    x: Vec<f64>,
    y: Vec<f64>,
    s1: Vec<f64>,
    s2: Vec<f64>,
    t: Vec<f64>,
    le: Vec<f64>,
    interest: Vec<u8>,
    time_step: f64,
    ext: f64,
}

fn check_time_step(ts: f64, derived: bool) -> Res<f64> {
    if ts.is_finite() && ts > 0.0 {
        return Ok(ts);
    }
    Err(if derived {
        "The default time_step is a fraction of the shortest time lag, which is not positive here (duplicate or \
         unsorted times?): give time_step in minutes"
            .into()
    } else {
        "time_step must be a single positive number (minutes)".to_string()
    })
}

fn kernel_of(model: Model, u: &Unit, le: &[f64], time_step: Option<f64>) -> Res<Kernel> {
    let var = u.var.as_ref().expect("a unit with a variance");
    let diffs = || u.tm.windows(2).map(|p| p[1] - p[0]);
    match model {
        Model::Dbbmm => {
            let n = u.x.len();
            if n < 2 {
                return Err(format!("A UD needs at least two fixes; got {n} in {}", u.label));
            }
            let ts = check_time_step(
                time_step.unwrap_or_else(|| diffs().fold(f64::INFINITY, f64::min) / 15.0),
                time_step.is_none(),
            )?;
            let t0 = u.tm.iter().copied().fold(f64::INFINITY, f64::min);
            let s1: Vec<f64> = var.iter().map(|&v| if v.is_nan() { 0.0 } else { v }).collect();
            Ok(Kernel {
                x: u.x.clone(),
                y: u.y.clone(),
                s2: s1.clone(),
                s1,
                t: u.tm.iter().map(|t| t - t0).collect(),
                le: le.to_vec(),
                interest: u.interest.iter().map(|&b| u8::from(b)).collect(),
                time_step: ts,
                ext: EXT_BBMM,
            })
        }
        Model::Dbgb => {
            let ne = &u.windows;
            let all_nan = ne.iter().all(|v| v.is_nan());
            let core: Vec<bool> = if all_nan {
                u.interest.clone()
            } else {
                let max = ne.iter().copied().filter(|v| !v.is_nan()).fold(f64::NEG_INFINITY, f64::max);
                ne.iter().map(|&v| !v.is_nan() && v == max).collect()
            };
            let ts = check_time_step(
                time_step.unwrap_or_else(|| diffs().fold(f64::INFINITY, f64::min) / 20.1),
                time_step.is_none(),
            )?;
            let m = core.iter().filter(|&&c| c).count();
            if m < 2 {
                return Err(format!("A UD needs at least two fixes of interest; got {m} in {}", u.label));
            }
            let pick = |a: &[f64]| -> Vec<f64> { (0..a.len()).filter(|&i| core[i]).map(|i| a[i]).collect() };
            let nz = |a: &[f64]| -> Vec<f64> { a.iter().map(|&v| if v.is_nan() { 0.0 } else { v }).collect() };
            let keep: Vec<u8> = (0..core.len()).filter(|&i| core[i]).map(|i| u8::from(u.interest[i])).collect();
            if !keep[..keep.len() - 1].iter().any(|&k| k != 0) {
                return Err(
                    "A UD needs at least one bridge of interest; every bridge is excluded (see seg_interest and \
                            move.mask_segments)"
                        .into(),
                );
            }
            Ok(Kernel {
                x: pick(&u.x),
                y: pick(&u.y),
                s1: pick(&nz(var)),
                s2: pick(&nz(&u.orth)),
                t: pick(&u.tm),
                le: pick(le),
                interest: keep,
                time_step: ts,
                ext: EXT_DBGB,
            })
        }
    }
}

struct Probe {
    env: [f64; 4],
    sides: [bool; 4],
    safe: bool,
}

fn probe(model: Model, k: &Kernel, g: &G) -> Res<Probe> {
    let (xs, ys) = g.vectors();
    if xs.len() < 2 || ys.len() < 2 {
        let (a, b) = (xs.len() < 2, ys.len() < 2);
        return Ok(Probe {
            env: [f64::INFINITY, f64::NEG_INFINITY, f64::INFINITY, f64::NEG_INFINITY],
            sides: [a, a, b, b],
            safe: false,
        });
    }
    let r = ffi::move_ud_probe(
        model.code(),
        &k.x,
        &k.y,
        &k.s1,
        &k.s2,
        &k.t,
        &k.le,
        &k.interest,
        &xs,
        &ys,
        k.time_step,
        k.ext,
    )
    .map_err(ffi_err)?;
    let sides = [r[4] < 0.0, r[5] > (xs.len() - 1) as f64, r[6] > (ys.len() - 1) as f64, r[7] < 0.0];
    Ok(Probe { env: [r[0], r[1], r[2], r[3]], sides, safe: !sides.iter().any(|&s| s) })
}

fn union_env(probes: &[Probe]) -> [f64; 4] {
    let f = |i: usize, init: f64, op: fn(f64, f64) -> f64| probes.iter().fold(init, |a, p| op(a, p.env[i]));
    [
        f(0, f64::INFINITY, f64::min),
        f(1, f64::NEG_INFINITY, f64::max),
        f(2, f64::INFINITY, f64::min),
        f(3, f64::NEG_INFINITY, f64::max),
    ]
}

fn probes_of(model: Model, kernels: &[Kernel], g: &G) -> Res<Vec<Probe>> {
    kernels.iter().map(|k| probe(model, k, g)).collect()
}

struct GridArgs {
    res: Option<f64>,
    dim_size: Option<usize>,
    ext_factor: f64,
    max_cells: f64,
}

fn check_cells(g: &G, a: &GridArgs) -> Res<()> {
    let n = g.nrow as f64 * g.ncol as f64;
    if n > a.max_cells {
        let what = match a.res {
            Some(r) => format!("res = {r}"),
            None => format!("dim_size = {}", a.dim_size.unwrap_or(100)),
        };
        return Err(format!(
            "The grid from {what} would have {} x {} = {n:.3e} cells, above the limit of {:.3e} cells (max_cells): use a \
             {}, or raise max_cells",
            g.ncol,
            g.nrow,
            a.max_cells,
            if a.res.is_some() { "larger res" } else { "smaller dim_size" }
        ));
    }
    Ok(())
}

/// `.ud_auto_grid`: the grid of `res` or `dim_size` on the widened box, kept when every bridge window fits, else enlarged
/// to the windows (the cell size grows with `dim_size`, the cells are added with `res`). With whether it was enlarged.
fn auto_grid(model: Model, px: &[f64], py: &[f64], a: &GridArgs, kernels: &[Kernel]) -> Res<(G, bool)> {
    let g = make_grid(px, py, a.res, if a.res.is_some() { None } else { a.dim_size }, a.ext_factor)?;
    check_cells(&g, a)?;
    let probes = probes_of(model, kernels, &g)?;
    if probes.iter().all(|p| p.safe) {
        return Ok((g, false));
    }
    let env = union_env(&probes);
    let bx = extcalc(px, py, a.ext_factor);
    let mut cell = a.res.unwrap_or_else(|| (bx[1] - bx[0]).max(bx[3] - bx[2]) / a.dim_size.unwrap_or(100) as f64);
    if a.res.is_none() {
        // the fixed point of cell = the longest side of the needed range / dim_size (converges for dim_size >= 3)
        let mut done = false;
        for _ in 0..500 {
            let need = needed_range(bx, env, cell, 0.0);
            let new = (need[1] - need[0]).max(need[3] - need[2]) / a.dim_size.unwrap_or(100) as f64;
            if new <= cell * (1.0 + 1e-12) {
                done = true;
                break;
            }
            cell = new;
        }
        if !done {
            return Err(format!(
                "dim_size = {} is too small to hold the bridges' kernel windows with the margin the kernel needs (at \
                 least 3): use a larger dim_size, or give res",
                a.dim_size.unwrap_or(100)
            ));
        }
    }
    for attempt in 0..4 {
        if a.res.is_none() && attempt > 0 {
            cell *= 1.0 + 1e-9;
        }
        let need = needed_range(bx, env, cell, if a.res.is_some() { f64::from(attempt) } else { 0.0 });
        if a.res.is_some() {
            let nc = ((need[1] - need[0]) / cell - 1e-9).ceil().max(1.0);
            let nr = ((need[3] - need[2]) / cell - 1e-9).ceil().max(1.0);
            if nc * nr > a.max_cells {
                return Err(format!(
                    "Enlarging the grid so that every bridge's window fits would need {nc} x {nr} = {:.3e} cells of \
                     {cell} map units, above the limit of {:.3e} cells (max_cells): use a larger res, or give dim_size, \
                     which fixes the number of cells",
                    nc * nr,
                    a.max_cells
                ));
            }
        }
        let g = grid_from_range(need, cell);
        if probes_of(model, kernels, &g)?.iter().all(|p| p.safe) {
            return Ok((g, true));
        }
    }
    Err("could not build a UD grid large enough for the kernel windows (please report this)".into())
}

/// The grid of a template, which is never enlarged: an error says how large it has to be.
fn template_of(grid: &Grid) -> Res<G> {
    let t = grid.transform;
    if t[2] != 0.0 || t[4] != 0.0 || t[5] >= 0.0 || t[1] <= 0.0 {
        return Err("like: a north-up raster without rotation".into());
    }
    let (nx, ny) = (grid.width, grid.height);
    Ok(G { xmin: t[0], xmax: t[0] + nx as f64 * t[1], ymin: t[3] + ny as f64 * t[5], ymax: t[3], nrow: ny, ncol: nx })
}

fn check_template(model: Model, g: &G, kernels: &[Kernel]) -> Res<()> {
    let probes = probes_of(model, kernels, g)?;
    if probes.iter().all(|p| p.safe) {
        return Ok(());
    }
    if g.nrow < 2 || g.ncol < 2 {
        return Err(format!(
            "like has {} row(s) and {} column(s); the kernel needs at least two of each",
            g.nrow, g.ncol
        ));
    }
    let sides: Vec<&str> = ["left", "right", "bottom", "top"]
        .into_iter()
        .enumerate()
        .filter(|(i, _)| probes.iter().any(|p| p.sides[*i]))
        .map(|(_, n)| n)
        .collect();
    let need = needed_range([g.xmin, g.xmax, g.ymin, g.ymax], union_env(&probes), g.xres().max(g.yres()), 0.0);
    Err(format!(
        "like is too small: the bridges' kernel windows reach beyond its {} edge(s). A template is never enlarged. A \
         raster with the extent x {} .. {}, y {} .. {} keeps its resolution and is large enough. Or leave like out and \
         give res or dim_size to build a grid large enough",
        sides.join(", "),
        need[0].floor(),
        need[1].ceil(),
        need[2].floor(),
        need[3].ceil()
    ))
}

// ----------------------------------------------------------------------------------------------- the UD operator

fn grid_args(args: &Args, has_like: bool) -> Result<GridArgs> {
    let node = args.node;
    let res: Option<f64> = args.opt_param("res")?;
    let dim: Option<i64> = args.opt_param("dim_size")?;
    let ext_factor: f64 = args.param("ext_factor", 0.5)?;
    let max_cells: f64 = args.param("max_cells", 1e8)?;
    if !(ext_factor.is_finite() && ext_factor >= 0.0) {
        return Err(fail(node, "ext_factor: a non-negative number"));
    }
    let given: Vec<&str> = [(has_like, "like"), (res.is_some(), "res"), (dim.is_some(), "dim_size")]
        .into_iter()
        .filter(|(g, _)| *g)
        .map(|(_, n)| n)
        .collect();
    if given.len() > 1 {
        return Err(fail(
            node,
            format!(
                "give only one of like (a template), res (a cell size) or dim_size (cells along the longest side); got {}",
                given.join(" and ")
            ),
        ));
    }
    if let Some(r) = res
        && !(r.is_finite() && r > 0.0)
    {
        return Err(fail(node, "res: a positive number, the cell size in the units of the computing CRS"));
    }
    if let Some(d) = dim
        && d < 2
    {
        return Err(fail(node, "dim_size: a whole number of at least 2 (cells along the longest side)"));
    }
    if !(max_cells.is_finite() && max_cells > 0.0) {
        return Err(fail(node, "max_cells: a positive number"));
    }
    let dim_size = if given.is_empty() { Some(100) } else { dim.map(|d| d as usize) };
    Ok(GridArgs { res, dim_size, ext_factor, max_cells })
}

/// The errors given to a distribution of tracks that already have their variance: none, or one per location.
fn override_error(args: &Args, ctx: &mut Ctx, v: &Vector) -> Result<Option<Vec<f64>>> {
    let n = v.len();
    let out = match resolve_error(args, ctx, v)? {
        Resolved::Nothing => return Ok(None),
        Resolved::Scalar(s) => vec![s; n],
        Resolved::Values(x) => x,
    };
    if out.iter().any(|a| a.is_nan()) {
        return Err(fail(args.node, "location_error must not contain missing values"));
    }
    Ok(Some(out))
}

fn numbers(v: &Vector, name: &str) -> Vec<f64> {
    match v.column(name) {
        Some(Column::Numbers(x)) => x.clone(),
        Some(Column::Texts(t)) => t.iter().map(|s| s.trim().parse::<f64>().unwrap_or(f64::NAN)).collect(),
        None => Vec::new(),
    }
}

/// The distribution of the bridge: the CRS it is computed in, the units with their variance, the grid.
struct Computed {
    units: Vec<Unit>,
    grid: G,
    layers: Vec<Vec<f64>>,
    work: Option<String>,
    enlarged: bool,
}

/// `move.dbbmm_ud` and `move.dbgb_ud`.
struct UdOp(Model);

impl Operator for UdOp {
    fn signature(&self) -> OpSignature {
        let (name, doc) = match self.0 {
            Model::Dbbmm => (
                "move.dbbmm_ud",
                "The utilization distribution of the dynamic Brownian bridge, a band for each track (move2utils' \
                 mt_dbbmm_ud, Kranstauber et al. 2012; Horne et al. 2007): each pair of consecutive locations is a \
                 Brownian bridge whose variance is the motion variance of the locations (move.dbbmm_variance) plus \
                 the location error at its ends; it is walked in steps of time_step (minutes, by default a fifteenth \
                 of the shortest time lag) and each step spreads a bivariate normal over the cells within four standard \
                 deviations of its position. The cells add the steps' densities times the cell area and each band is \
                 divided by its sum: the probability of the animal being in the cell, summing to 1 (the tracks share \
                 one grid, so the bands compare directly). Only the segments of interest count (move.mask_segments). \
                 The tracks may come with the columns of move.dbbmm_variance, used as they are (a location_error given \
                 here replaces the errors they hold; with longitude / latitude the projection is the centroid's, \
                 recomputed from the tracks given), or without them: the variance is then computed here with \
                 window_size, margin, location_error and location_error_na. The grid: the raster like (never \
                 enlarged: an error says how large it has to be), or the box of the locations widened by ext_factor of \
                 its sides, in cells of res or dim_size cells along its longest side (100), enlarged when the bridges' \
                 windows do not fit (told only with verbose); the cost grows with the cells times the steps: a coarse \
                 grid first. The distribution is computed and given in the coordinate system of the tracks when it is \
                 projected in metres, else in the local azimuthal equidistant projection of move.aeqd_crs centred on \
                 the middle of the ranges of longitude and latitude (metres); crs input warps it back to the \
                 coordinate system of the tracks (GDAL bilinear, cells below 0 made 0, each band summed to 1 again: \
                 not the same bits on every machine). With bursts (move.resample) a band is a burst, named \
                 track__burst. A track with fewer locations than window_size gets no band (a warning).",
            ),
            Model::Dbgb => (
                "move.dbgb_ud",
                "The utilization distribution of the directional Brownian bridge, a band for each track (move2utils' \
                 mt_dbgb_ud): as move.dbbmm_ud, but each step of a bridge spreads a bivariate normal with two standard \
                 deviations, along and across the direction from the bridge's position to the next location \
                 (move.dbgb_variance), over the cells within five of the larger one, with time_step by default a \
                 twentieth of the shortest time lag. Only the core of each track counts (the locations every window of \
                 the variance covers) and in it the segments of seg_interest (move.mask_segments): it needs long tracks \
                 and a template (like) larger than the dBBMM's, as the windows reach farther. margin is 15 by default. \
                 The other settings and the output are those of move.dbbmm_ud.",
            ),
        };
        let margin_doc = match self.0 {
            Model::Dbbmm => "without the variance columns: odd, window_size at least 2 margin + 1 (11)",
            Model::Dbgb => "without the variance columns: odd, window_size at least 2 margin + 1 (15)",
        };
        column_params(
            OpSignature::new(name, doc, Access::None)
                .input(PortSpec::new("tracks", Type::Vector, "the tracks: points, each track's rows consecutive in time"))
                .input(PortSpec::optional(
                    "like",
                    Type::Raster,
                    "the raster whose grid to compute on (north up, projected in metres); else one made from the locations",
                ))
                .output(PortSpec::new("ud", Type::Stack, "a band for each track (or burst), the cells summing to 1"))
                .param(PortSpec::optional("res", Type::Number, "without like: the cell size, in the units of the computing CRS"))
                .param(PortSpec::optional("dim_size", Type::Integer, "without like: the cells along the longest side (100)"))
                .param(PortSpec::optional("ext_factor", Type::Number, "the box of the locations widened by this fraction of its sides (0.5)"))
                .param(PortSpec::optional("time_step", Type::Number, "the step of the integration over time, minutes (a fraction of the shortest lag)"))
                .param(PortSpec::optional("window_size", Type::Integer, "without the variance columns: the locations of a window, odd (31)"))
                .param(PortSpec::optional("margin", Type::Integer, margin_doc))
                .param(PortSpec::optional(
                    "location_error",
                    Type::Any,
                    "the standard deviation of the location error, metres: none, a number, a column or auto",
                ))
                .param(
                    PortSpec::optional("location_error_na", Type::Text, "what a missing error becomes (median)")
                        .choices(NA_HOW),
                )
                .param(
                    PortSpec::optional("crs", Type::Text, "projected (default): the computing CRS; input: the CRS of the tracks")
                        .choices(["projected", "input"]),
                )
                .param(PortSpec::optional("max_cells", Type::Number, "the most cells a grid made from res or dim_size may have (1e8)"))
                .param(PortSpec::optional("verbose", Type::Bool, "say when the grid was enlarged to hold the bridges (false)"))
                .param(PortSpec::optional(
                    "burst",
                    Type::Text,
                    "auto (default: the bursts of move.resample are tracks of their own), none, or a column",
                )),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let model = self.0;
        let tag = |e: String| fail(node, e);
        let crs_mode: String = args.param("crs", "projected".to_string())?;
        if crs_mode != "projected" && crs_mode != "input" {
            return Err(fail(node, "crs: projected or input"));
        }
        let v = tracks_of(args, "tracks")?;
        let like = like_grid(args)?;
        let ga = grid_args(args, like.is_some())?;
        let time_step: Option<f64> = args.opt_param("time_step")?;
        let verbose: bool = args.param("verbose", false)?;
        let th = threads();
        let c = compute(ctx, args, model, v, like.as_deref(), &ga, time_step, th).map_err(tag)?;
        if c.enlarged && verbose {
            ctx.warn(format!(
                "node {node:?}: grid enlarged so that every bridge's kernel window fits: {} x {} cells of {:.4} map units \
                 (the box from ext_factor = {} was too small)",
                c.grid.ncol,
                c.grid.nrow,
                c.grid.xres(),
                ga.ext_factor
            ));
        }
        let wkt = c.work.as_deref().map(|w| terraflux_gdal::crs_wkt(w).unwrap_or_else(|_| w.to_string()));
        let keys: Vec<Key> = c.units.iter().map(|u| Key::Name(u.label.clone())).collect();
        let tcrs = v.crs.as_deref();
        let (grid, layers) = match (crs_mode.as_str(), like.is_some(), c.work.as_deref(), tcrs) {
            ("input", false, Some(w), Some(t)) if !same_crs(w, t) => {
                let (g, layers) = warp_back(&c.layers, &c.grid.grid(wkt.as_ref()), t).map_err(tag)?;
                (Arc::new(g), layers)
            }
            _ => (Arc::new(c.grid.grid(wkt.as_ref())), c.layers),
        };
        let layers = layers.into_iter().map(Arc::new).collect();
        Ok(crate::outputs([("ud", Value::Stack(Stack { grid, keys, layers }))]))
    }
}

/// The units with a variance, their grid and their distributions.
#[allow(clippy::too_many_arguments)]
fn compute(
    ctx: &mut Ctx,
    args: &Args,
    model: Model,
    v: &Vector,
    like: Option<&Grid>,
    ga: &GridArgs,
    time_step: Option<f64>,
    th: i32,
) -> Res<Computed> {
    let node = args.node;
    let e = |e: crate::Error| e.to_string();
    let p = prepare(args, v, "the distribution").map_err(e)?;
    let has_cols = model.columns().iter().all(|c| v.column(c).is_some());
    let tmpl = like.map(|g| (g, g.geo.wkt.clone()));
    let tcrs = p.crs.as_deref();
    let work: Option<String> = if has_cols && p.lonlat {
        Some(local_crs(v, "centroid")?)
    } else if has_cols {
        p.crs.clone()
    } else if let Some((_, tc)) = &tmpl {
        if let Some(c) = tc
            && !suitable(Some(c))?
        {
            return Err("like: its CRS must be projected in metres (the distribution is computed on its grid)".into());
        }
        tc.clone()
    } else if suitable(tcrs)? {
        p.crs.clone()
    } else {
        Some(local_crs(v, "center")?)
    };
    if let (Some((_, Some(tc))), Some(w)) = (&tmpl, &work)
        && !same_crs(w, tc)
    {
        return Err(
            "like: its CRS must be the CRS the variance was computed in: re-project it, or give the tracks without \
                    the variance columns"
                .into(),
        );
    }
    let (px, py) = transform_xy(&p.x, &p.y, tcrs, work.as_deref())?;
    let mut units = units_from(&p, &px, &py)?;
    let keep: Vec<usize> = if has_cols {
        let le_all = match v.column(model.error_column()) {
            Some(_) => numbers(v, model.error_column()),
            None => {
                return Err(format!(
                    "the tracks have the variance columns but no {} column: run move.{}_variance again",
                    model.error_column(),
                    model.name()
                ));
            }
        };
        let cols: Vec<Vec<f64>> = model.columns().iter().map(|c| numbers(v, c)).collect();
        let over = override_error(args, ctx, v).map_err(e)?;
        let mut ok = Vec::new();
        for (i, u) in units.iter_mut().enumerate() {
            let r = u.start..u.stop;
            u.loc_err = Some(le_all[r.clone()].to_vec());
            let (var, windows, interest) = match model {
                Model::Dbbmm => (&cols[0], &cols[1], &cols[2]),
                Model::Dbgb => (&cols[0], &cols[2], &cols[3]),
            };
            u.var = Some(var[r.clone()].to_vec());
            if model == Model::Dbgb {
                u.orth = cols[1][r.clone()].to_vec();
            }
            u.windows = windows[r.clone()].to_vec();
            u.interest = interest[r.clone()].iter().map(|&b| b != 0.0).collect();
            let skipped = u.loc_err.as_ref().unwrap().iter().all(|a| a.is_nan())
                && u.var.as_ref().unwrap().iter().all(|a| a.is_nan());
            if skipped {
                continue; // a track the variance skipped
            }
            if let Some(o) = &over {
                u.loc_err = Some(o[r].to_vec());
            }
            ok.push(i);
        }
        ok
    } else {
        let w = window_of(args, model).map_err(e)?;
        let how = na_how_of(args).map_err(e)?;
        let resolved = resolve_error(args, ctx, v).map_err(e)?;
        fit_variance(ctx, node, model, &mut units, &resolved, how, &w, th)?;
        (0..units.len()).filter(|&i| units[i].loc_err.is_some()).collect()
    };
    if keep.is_empty() {
        return Err("no track has a variance to compute a distribution from".into());
    }
    let kernels = keep
        .iter()
        .map(|&i| kernel_of(model, &units[i], units[i].loc_err.as_ref().unwrap(), time_step))
        .collect::<Res<Vec<Kernel>>>()?;
    let (grid, enlarged) = match &tmpl {
        Some((g, _)) => {
            let g = template_of(g)?;
            check_template(model, &g, &kernels)?;
            (g, false)
        }
        None => {
            let ax: Vec<f64> = keep.iter().flat_map(|&i| units[i].x.iter().copied()).collect();
            let ay: Vec<f64> = keep.iter().flat_map(|&i| units[i].y.iter().copied()).collect();
            auto_grid(model, &ax, &ay, ga, &kernels)?
        }
    };
    let (xs, ys) = grid.vectors();
    let side = grid.nrow * grid.ncol <= 25_000_000;
    let results = each(&kernels, th, side, |k, t| {
        ffi::move_ud_cells(
            model.code(),
            &k.x,
            &k.y,
            &k.s1,
            &k.s2,
            &k.t,
            &k.le,
            &k.interest,
            &xs,
            &ys,
            k.time_step,
            k.ext,
            t,
        )
        .map_err(ffi_err)
    });
    let layers = results.into_iter().collect::<Res<Vec<Vec<f64>>>>()?;
    // the units that make bands, in order
    let mut it = units.into_iter().enumerate();
    let mut kept = Vec::with_capacity(keep.len());
    for &i in &keep {
        for (j, u) in it.by_ref() {
            if j == i {
                kept.push(u);
                break;
            }
        }
    }
    Ok(Computed { units: kept, grid, layers, work, enlarged })
}

// ---------------------------------------------------------------------------------------- back to the tracks' CRS

/// The bands warped to another CRS (GDAL's bilinear, which chooses the grid as terra's project does), the negative and
/// missing cells made 0 and every band summed to 1 again (move2utils' .warp_ud_to_target).
fn warp_back(layers: &[Vec<f64>], grid: &Grid, target: &str) -> Res<(Grid, Vec<Vec<f64>>)> {
    use std::sync::atomic::{AtomicU32, Ordering};

    static NEXT: AtomicU32 = AtomicU32::new(0);
    terraflux_gdal::gdal().map_err(|e| format!("crs input needs GDAL: {e}"))?;
    let dir = std::env::temp_dir().join(format!(
        "terraflux-ud-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let result = (|| -> Res<(Grid, Vec<Vec<f64>>)> {
        let (src, dst) = (dir.join("ud.tif"), dir.join("ud_back.tif"));
        let names: Vec<String> = (0..layers.len()).map(|i| format!("b{i}")).collect();
        let refs: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut w =
            crate::files::RowsWriter::create_bands(&src, grid, &refs, terraflux_raster::DataType::F64, Some(f64::NAN))
                .map_err(|e| e.to_string())?;
        let bands: Vec<&[f64]> = layers.iter().map(Vec::as_slice).collect();
        w.push_bands(&bands).map_err(|e| e.to_string())?;
        w.finish().map_err(|e| e.to_string())?;
        let args: Vec<String> = [
            "-t_srs",
            target,
            "-r",
            "bilinear",
            "-srcnodata",
            "nan",
            "-dstnodata",
            "nan",
            "-multi",
            "-wo",
            "NUM_THREADS=1",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        terraflux_gdal::warp(&[src.as_path()], &dst, &args)?;
        let back = crate::files::Rasters::open(&dst.to_string_lossy(), None, None).map_err(|e| e.to_string())?;
        let s = back.stack().map_err(|e| e.to_string())?;
        let mut out: Vec<Vec<f64>> = s.layers.iter().map(|l| l.to_vec()).collect();
        for band in &mut out {
            for v in band.iter_mut() {
                if !v.is_finite() || *v < 0.0 {
                    *v = 0.0;
                }
            }
            let total = ffi::move_r_sum(band);
            if total.is_finite() && total > 0.0 {
                for v in band.iter_mut() {
                    *v /= total;
                }
            }
        }
        let mut g = (*s.grid).clone();
        g.geo.wkt = terraflux_gdal::crs_wkt(target).ok().or(g.geo.wkt);
        Ok((g, out))
    })();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

// -------------------------------------------------------------------------------------------- suggest_window

fn nearest_odd(z: f64) -> i64 {
    let mut k = z.clamp(-2e9, 2e9).round_ties_even() as i64;
    if k % 2 == 0 {
        k += 1;
    }
    k.max(1)
}

fn floor_odd(z: f64) -> i64 {
    let mut k = z.clamp(-2e9, 2e9).floor() as i64;
    if k % 2 == 0 {
        k -= 1;
    }
    k.max(1)
}

/// `move.suggest_window`.
struct SuggestWindow;

impl Operator for SuggestWindow {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.suggest_window",
                "A window size and margin for move.dbbmm_variance from the sampling of each track (move2utils' \
                 mt_suggest_dbbmm_window, without its plot): the window covers about target_hours at the median time lag \
                 (an odd number of locations, at least 11 and at most a quarter of the track), the margin is a quarter \
                 of it (odd, at least 3, at most (window_size - 1) / 2). A window much shorter than the movement's \
                 changes of behaviour sees none of them; a longer one averages them. A track with fewer than 11 \
                 locations or fewer than two time lags above zero gets no suggestion (a warning, empty numbers). The \
                 table has a row for each track: window_size, margin, median_dt_secs, window_hours, n (locations) and \
                 n_windows (the whole windows that fit).",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks: points, each track's rows consecutive in time"))
            .output(PortSpec::new("table", Type::Table, "the suggestion for each track"))
            .param(PortSpec::optional("target_hours", Type::Number, "the hours a window should cover (4)")),
        )
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let target: f64 = args.param("target_hours", 4.0)?;
        if !(target.is_finite() && target > 0.0) {
            return Err(fail(node, "target_hours: a positive number"));
        }
        let (t, k) = columns(args, v)?;
        let code = codes(v.column(&k).unwrap());
        let secs = seconds_of(&Times::of(v.column(&t).unwrap()).map_err(|e| fail(node, e))?);
        let kcol = v.column(&k).unwrap();
        let mut names = Vec::new();
        let mut rows = Vec::new();
        for (s, e) in bounds(&code) {
            let id = column_label(kcol, s);
            let n = e - s;
            names.push(id.clone());
            let none = vec![f64::NAN, f64::NAN, f64::NAN, f64::NAN, n as f64, f64::NAN];
            if n < 11 {
                ctx.warn(format!("node {node:?}: too few locations for the dBBMM (< 11) in {id}: no suggestion"));
                rows.push(none);
                continue;
            }
            let mut dt: Vec<f64> =
                secs[s..e].windows(2).map(|p| p[1] - p[0]).filter(|d| d.is_finite() && *d > 0.0).collect();
            if dt.len() < 2 {
                ctx.warn(format!("node {node:?}: too few finite time lags in {id}: no suggestion"));
                rows.push(none);
                continue;
            }
            dt.sort_by(|a, b| a.total_cmp(b));
            let med =
                if dt.len() % 2 == 1 { dt[dt.len() / 2] } else { (dt[dt.len() / 2 - 1] + dt[dt.len() / 2]) / 2.0 };
            let raw_w = target * 3600.0 / med;
            let cap_w = 11.max(floor_odd(n as f64 / 4.0));
            let w = 11.max(nearest_odd(raw_w).min(cap_w));
            let m_int = 3.max(nearest_odd((w - 1) as f64 / 4.0));
            let m = m_int.min(floor_odd((w - 1) as f64 / 2.0));
            rows.push(vec![w as f64, m as f64, med, w as f64 * med / 3600.0, n as f64, (n as i64 / w) as f64]);
        }
        let mut table = labels_table(
            &["track", "window_size", "margin", "median_dt_secs", "window_hours", "n", "n_windows"],
            &names,
        );
        for (i, r) in rows.into_iter().enumerate() {
            let mut row = vec![(i + 1) as f64];
            row.extend(r);
            table.rows.push(row);
        }
        Ok(crate::outputs([("table", Value::Table(table))]))
    }
}

// ---------------------------------------------------------------------------------------------- mask_segments

/// `move.mask_segments`.
struct MaskSegments;

impl Operator for MaskSegments {
    fn signature(&self) -> OpSignature {
        column_params(
            OpSignature::new(
                "move.mask_segments",
                "Leave segments out of the distribution (move2utils' mt_mask_segments): interest (seg_interest for the \
                 dBGB) becomes 0 for them. The segment i of a track is the bridge from its location i to i + 1 (from 0, \
                 R counts from 1), the one that the time between two fixes spans: leave out the bridges across long \
                 gaps, as the vignette 'UD_gap_aware_ud' of move2utils does (gap, in hours: the segments whose time \
                 lag is longer), or the ones you name: segments, a list of numbers from 0 to n - 2 applied to every \
                 track, or an object by track (the names of the bands of move.dbbmm_ud) with a list each; column, a \
                 column of the tracks that is not 0 (or empty) where the segment starting there is left out. The tracks \
                 must have the columns of move.dbbmm_variance or move.dbgb_variance. With bursts (move.resample) the \
                 tracks are the bursts.",
                Access::None,
            )
            .input(PortSpec::new("tracks", Type::Vector, "the tracks with the columns of the variance"))
            .output(PortSpec::new("tracks", Type::Vector, "the tracks with interest updated"))
            .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
            .param(PortSpec::optional("segments", Type::Any, "the segments to leave out: a list of numbers, or an object by track"))
            .param(PortSpec::optional("gap", Type::Number, "leave out the segments whose time lag is longer than this, hours"))
            .param(PortSpec::optional("column", Type::Text, "a column of the tracks: not 0 where the segment is left out"))
            .param(
                PortSpec::optional("model", Type::Text, "which of the two to mask, when the tracks have the columns of both")
                    .choices(["dbbmm", "dbgb"]),
            )
            .param(PortSpec::optional(
                "burst",
                Type::Text,
                "auto (default: the bursts of move.resample are tracks of their own), none, or a column",
            )),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let v = tracks_of(args, "tracks")?;
        let tag = |e: String| fail(node, e);
        let has = |m: Model| v.column(m.interest_column()).is_some();
        let model = match args.opt_param::<String>("model")?.as_deref() {
            Some("dbbmm") => Model::Dbbmm,
            Some("dbgb") => Model::Dbgb,
            Some("") | None => match (has(Model::Dbbmm), has(Model::Dbgb)) {
                (true, true) => {
                    return Err(fail(
                        node,
                        "the tracks have the columns of both models: name one with model (dbbmm or dbgb)",
                    ));
                }
                (false, true) => Model::Dbgb,
                _ => Model::Dbbmm,
            },
            Some(_) => return Err(fail(node, "model: dbbmm or dbgb")),
        };
        if !has(model) {
            return Err(fail(
                node,
                format!("no {} column: run move.{}_variance first", model.interest_column(), model.name()),
            ));
        }
        let p = prepare(args, v, "masking segments")?;
        let mut interest: Vec<bool> = numbers(v, model.interest_column()).iter().map(|&b| b != 0.0).collect();
        let segments = args.params.get("segments").filter(|s| !s.is_null());
        let gap: Option<f64> = args.opt_param("gap")?;
        let column: Option<String> = args.opt_param::<String>("column")?.filter(|c| !c.is_empty());
        if segments.is_none() && gap.is_none() && column.is_none() {
            return Err(fail(node, "give segments, gap or column: which segments to leave out"));
        }
        let ranges = bounds(&p.code);
        if let Some(c) = &column {
            let col = v.column(c).ok_or_else(|| fail(node, format!("column {c:?} not found")))?;
            for i in 0..v.len() {
                let on = match col {
                    Column::Numbers(x) => !x[i].is_nan() && x[i] != 0.0,
                    Column::Texts(t) => matches!(t[i].trim().to_lowercase().as_str(), "1" | "true" | "t" | "yes"),
                };
                if on {
                    interest[i] = false;
                }
            }
        }
        if let Some(h) = gap {
            if !(h.is_finite() && h >= 0.0) {
                return Err(fail(node, "gap: a number of hours, not negative"));
            }
            for &(s, e) in &ranges {
                for (flag, w) in interest[s..e - 1].iter_mut().zip(p.secs[s..e].windows(2)) {
                    if w[1] - w[0] > h * 3600.0 {
                        *flag = false;
                    }
                }
            }
        }
        let cut = |label: &str, s: usize, e: usize, seg: &[f64], interest: &mut Vec<bool>| -> Res<()> {
            let n = e - s;
            let seg: Vec<f64> = seg.iter().copied().filter(|f| !f.is_nan()).collect();
            let top = n as f64 - 2.0;
            if seg.iter().any(|&f| f < 0.0 || f > top || f.fract() != 0.0) {
                return Err(format!("segments must be integers in 0..{} (track {label})", n as i64 - 2));
            }
            for f in seg {
                interest[s + f as usize] = false;
            }
            Ok(())
        };
        let list = |j: &serde_json::Value| -> Res<Vec<f64>> {
            match j {
                serde_json::Value::Number(n) => Ok(vec![n.as_f64().unwrap_or(f64::NAN)]),
                serde_json::Value::Array(a) => {
                    a.iter().map(|x| x.as_f64().ok_or_else(|| "segments: numbers".to_string())).collect()
                }
                _ => Err("segments: a list of numbers, or an object by track".to_string()),
            }
        };
        match segments {
            Some(serde_json::Value::Object(m)) => {
                if let Some(k) = m.keys().find(|k| !p.labels.contains(k)) {
                    return Err(fail(node, format!("segments names no track: {k:?}")));
                }
                for (u, &(s, e)) in ranges.iter().enumerate() {
                    if let Some(j) = m.get(&p.labels[u]) {
                        cut(&p.labels[u], s, e, &list(j).map_err(tag)?, &mut interest).map_err(tag)?;
                    }
                }
            }
            Some(j) => {
                let seg = list(j).map_err(tag)?;
                for (u, &(s, e)) in ranges.iter().enumerate() {
                    cut(&p.labels[u], s, e, &seg, &mut interest).map_err(tag)?;
                }
            }
            None => {}
        }
        let mut out = (**v).clone();
        set_column(&mut out, model.interest_column(), flag(&interest));
        Ok(give("tracks", out))
    }
}

// ------------------------------------------------------------------------------------- volume and probability

/// The bands of a distribution: a raster of one band (a map) or of several (a stack).
/// The grid, the names and the cells of the bands of a distribution.
struct Bands {
    grid: Arc<Grid>,
    names: Vec<String>,
    layers: Vec<Arc<Vec<f64>>>,
}

fn bands_of(node: &str, value: &Value) -> Result<Bands> {
    let value = crate::files::as_port(value.clone(), Type::Any)?;
    match &value {
        Value::Stack(s) => Ok(Bands {
            grid: s.grid.clone(),
            names: s
                .keys
                .iter()
                .enumerate()
                .map(|(i, k)| match k {
                    Key::Name(n) if !n.is_empty() => n.clone(),
                    _ => format!("ud_{}", i + 1),
                })
                .collect(),
            layers: s.layers.clone(),
        }),
        Value::Map(m) => {
            Ok(Bands { grid: m.grid.clone(), names: vec!["ud".to_string()], layers: vec![m.data.clone()] })
        }
        other => Err(fail(node, format!("ud: a raster (the ud of move.dbbmm_ud), not a {}", other.ty().name()))),
    }
}

/// `move.ud_volume`.
struct UdVolume;

impl Operator for UdVolume {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.ud_volume",
            "The volume of a distribution (move2utils' ud_volume): each cell's cumulative probability when the cells are \
             added from the largest value down, a band for each band of the input. The cells with volume up to 0.95 \
             are the smallest area that holds 95 % of the use. A band whose total is not positive is left as it is, \
             missing cells stay missing, cells of equal value are taken in the order of the cells. With levels (numbers \
             in [0, 1], 0.5 and 0.95 for the core and the home range) the contours of the volume at those levels are \
             also given as polygons (R's contourLines, as move.home_range's kde isopleths: a band's contour that reaches \
             the raster's edge is an error: use a larger grid), each with its band, level and area.",
            Access::None,
        )
        .input(PortSpec::new("ud", Type::Any, "the distribution: the ud of move.dbbmm_ud, move.dbgb_ud or move.home_range"))
        .output(PortSpec::new("volume", Type::Stack, "the volume of each band"))
        .output(PortSpec::optional("polygons", Type::Vector, "with levels: a polygon for each band and level"))
        .output(PortSpec::optional("table", Type::Table, "with levels: the areas, as the polygons"))
        .param(PortSpec::optional("levels", Type::Any, "the levels of the contours: a number or a list, in [0, 1]"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let tag = |e: String| fail(node, e);
        let value = args.get("ud").ok_or_else(|| fail(node, "connect the ud"))?;
        let Bands { grid, names, layers } = bands_of(node, value)?;
        let vol: Vec<Vec<f64>> = layers.iter().map(|l| ffi::move_ud_volume(l)).collect();
        let mut out = crate::outputs([(
            "volume",
            Value::Stack(Stack {
                grid: grid.clone(),
                keys: names.iter().cloned().map(Key::Name).collect(),
                layers: vol.iter().cloned().map(Arc::new).collect(),
            }),
        )]);
        let levels: Option<Vec<f64>> = match args.params.get("levels") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::Number(n)) => Some(vec![n.as_f64().unwrap_or(f64::NAN)]),
            Some(serde_json::Value::Array(a)) => Some(a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()),
            Some(_) => return Err(fail(node, "levels: a number or a list of numbers")),
        };
        if let Some(mut levels) = levels {
            if levels.is_empty() || levels.iter().any(|l| !l.is_finite() || !(0.0..=1.0).contains(l)) {
                return Err(fail(node, "levels: numbers between 0 and 1"));
            }
            levels.sort_by(|a, b| a.total_cmp(b));
            let g = geos().map_err(tag)?;
            let spec: Spec = spec_of(&grid).map_err(tag)?;
            let mut geometries = Vec::new();
            let (mut band, mut level, mut area) = (Vec::new(), Vec::new(), Vec::new());
            for (k, v) in vol.iter().enumerate() {
                let polys =
                    isopleths(v, &spec, &levels, g).map_err(|e| fail(node, format!("band {}: {e}", names[k])))?;
                for (lv, shape) in levels.iter().zip(polys).rev() {
                    area.push(shape_area(&shape));
                    geometries.push(geometry_of(&shape, true));
                    band.push(names[k].clone());
                    level.push(*lv);
                }
            }
            let mut table = labels_table(&["band", "level", "area"], &names);
            let mut ordinal: HashMap<&str, f64> = HashMap::new();
            for (i, n) in names.iter().enumerate() {
                ordinal.insert(n.as_str(), (i + 1) as f64);
            }
            for i in 0..band.len() {
                table.rows.push(vec![ordinal[band[i].as_str()], level[i], area[i]]);
            }
            let polygons = Vector {
                geometries,
                attributes: vec![
                    ("band".into(), Column::Texts(band)),
                    ("level".into(), Column::Numbers(level)),
                    ("area".into(), Column::Numbers(area)),
                ],
                crs: grid.geo.wkt.clone(),
            };
            out.insert("polygons".into(), Value::Vector(Arc::new(polygons)));
            out.insert("table".into(), Value::Table(table));
        }
        Ok(out)
    }
}

/// `move.ud_outer_probability`.
struct UdOuterProbability;

impl Operator for UdOuterProbability {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.ud_outer_probability",
            "The outer probability of locations (move2utils' ud_outer_probability): the volume of the distribution at \
             each one, the share of the probability in the cells at least as likely as the cell that holds the \
             location: small inside the core of the use, near 1 far out; empty for a location outside the raster. It \
             flags the fixes the animal's own distribution finds unlikely (GPS errors, excursions). The points are taken \
             to the raster's coordinate system. A distribution of several bands needs band (the name of a band, or its \
             position from 1). Column added: outer_probability.",
            Access::None,
        )
        .input(PortSpec::new("points", Type::Vector, "the locations: points"))
        .input(PortSpec::new("ud", Type::Any, "the distribution: the ud of move.dbbmm_ud, move.dbgb_ud or move.home_range"))
        .output(PortSpec::new("points", Type::Vector, "the locations with the column outer_probability"))
        .output(PortSpec::new("table", Type::Table, "their attributes that are numbers"))
        .param(PortSpec::optional("band", Type::Any, "the band of a distribution of several: its name or its position from 1"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let tag = |e: String| fail(node, e);
        let pts = tracks_of(args, "points")?;
        let value = args.get("ud").ok_or_else(|| fail(node, "connect the ud"))?;
        let Bands { grid, names, layers } = bands_of(node, value)?;
        let band = if layers.len() == 1 {
            0
        } else {
            match args.params.get("band") {
                None | Some(serde_json::Value::Null) => {
                    return Err(fail(node, "the distribution has several bands: name one with band"));
                }
                Some(serde_json::Value::String(s)) => match names.iter().position(|n| n == s) {
                    Some(i) => i,
                    None => return Err(fail(node, format!("band {s:?} is not one of {}", names.join(", ")))),
                },
                Some(serde_json::Value::Number(n)) => {
                    let i = n.as_f64().unwrap_or(0.0);
                    if i.fract() != 0.0 || i < 1.0 || i > layers.len() as f64 {
                        return Err(fail(node, format!("band: a position from 1 to {}", layers.len())));
                    }
                    i as usize - 1
                }
                Some(_) => return Err(fail(node, "band: a name or a position")),
            }
        };
        let (mut xs, mut ys) = (Vec::with_capacity(pts.len()), Vec::with_capacity(pts.len()));
        for g in &pts.geometries {
            match g {
                Geometry::Point(p) => {
                    xs.push(p[0]);
                    ys.push(p[1]);
                }
                Geometry::Empty => {
                    xs.push(f64::NAN);
                    ys.push(f64::NAN);
                }
                _ => return Err(fail(node, "the geometry must be points")),
            }
        }
        if let (Some(src), Some(dst)) = (pts.crs.as_deref(), grid.geo.wkt.as_deref())
            && !same_crs(src, dst)
        {
            let ok: Vec<usize> = (0..xs.len()).filter(|&i| xs[i].is_finite() && ys[i].is_finite()).collect();
            let (mut ox, mut oy): (Vec<f64>, Vec<f64>) = ok.iter().map(|&i| (xs[i], ys[i])).unzip();
            terraflux_gdal::Transform::new(src, dst).map_err(tag)?.points(&mut ox, &mut oy);
            let (mut nx, mut ny) = (vec![f64::NAN; xs.len()], vec![f64::NAN; xs.len()]);
            for (k, &i) in ok.iter().enumerate() {
                nx[i] = ox[k];
                ny[i] = oy[k];
            }
            (xs, ys) = (nx, ny);
        }
        let vol = ffi::move_ud_volume(&layers[band]);
        let cells = crate::sdm::points::grid_cells(&xs, &ys, &grid);
        let vals: Vec<f64> = cells.iter().map(|&c| if c >= 0 { vol[c as usize] } else { f64::NAN }).collect();
        let mut out = (**pts).clone();
        set_column(&mut out, "outer_probability", Column::Numbers(vals));
        Ok(give("points", out))
    }
}
