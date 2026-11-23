//! The Python counterparts of the operators of movement ecology (move).
//!
//! Terraflux's tracks are vector features (points with a time and a track column) and the library's a GeoDataFrame
//! of the same, so the tracks pass through as they are; the library's measures (distance, speed, azimuth...) give
//! an array, which the script puts in a column of a copy of the tracks, as the operator does. The steps and the
//! random steps are a DataFrame (the library has no line geometry for them: x1_, y1_, x2_, y2_ are their ends).
//! Every operator has a `table` output (the attributes that are numbers, a `feature` column first): written as an
//! expression of the frame.
//!
//! The entries of `MAPPED` are what the guard tests check against the library; the code is written by `special`,
//! which reads them (the table of each output, a column set, a distribution's table).

#![allow(unused_imports)]

use super::*;

/// An entry: the operator, the library's function, the arguments (ours => theirs), the defaults, the main output.
macro_rules! mv {
    ($op:literal, $call:literal, [$($a:literal => $b:literal),*], [$($k:literal = $v:literal),*], $out:literal) => {
        Py {
            op: $op,
            call: $call,
            args: &[$(($a, $b)),*],
            defaults: &[$(($k, $v)),*],
            outs: &[($out, "")],
            kind: Kind::Plain,
        }
    };
}

/// The operators of this family called with their arguments mapped (their code: `special`, from these).
pub(super) const MAPPED: &[Py] = &[
    mv!("move.read_tracks", "tf.move.load_tracks", ["path" => "", "sort" => "sort"], [], "tracks"),
    mv!(
        "move.as_tracks",
        "tf.move.as_tracks",
        ["vector" => "", "time" => "time", "track" => "track", "x" => "x", "y" => "y", "crs" => "crs", "sort" => "sort"],
        [],
        "tracks"
    ),
    mv!(
        "move.simulate_brownian",
        "tf.move.simulate_brownian",
        ["t" => "t", "sigma" => "sigma", "tracks" => "tracks", "start" => "start", "seed" => "seed"],
        ["seed" = "1"],
        "tracks"
    ),
    mv!("move.aeqd_crs", "tf.move.aeqd_crs", ["tracks" => "", "center" => "center", "units" => "units"], [], "crs"),
    mv!(
        "move.filter_unique",
        "tf.move.filter_unique",
        [
            "tracks" => "", "criterion" => "criterion", "columns_also" => "columns_also", "seed" => "seed",
            "time" => "time", "track" => "track"
        ],
        ["seed" = "1"],
        "tracks"
    ),
    mv!(
        "move.filter_per_interval",
        "tf.move.filter_per_interval",
        [
            "tracks" => "", "unit" => "unit", "criterion" => "criterion", "seed" => "seed", "time" => "time",
            "track" => "track"
        ],
        ["seed" = "1"],
        "tracks"
    ),
    mv!("move.filter_movebank_visible", "tf.move.filter_movebank_visible", ["tracks" => ""], [], "tracks"),
    mv!(
        "move.filter_min_n_burst",
        "tf.move.filter_min_n_burst",
        ["tracks" => "", "min_n" => "min_n", "track" => "track"],
        [],
        "tracks"
    ),
    mv!(
        "move.resample",
        "tf.move.resample",
        [
            "tracks" => "", "rate" => "rate", "tolerance" => "tolerance", "start" => "start", "time" => "time",
            "track" => "track"
        ],
        [],
        "tracks"
    ),
    mv!(
        "move.interpolate",
        "tf.move.interpolate",
        [
            "tracks" => "", "times" => "times", "max_time_lag" => "max_time_lag", "omit" => "omit", "time" => "time",
            "track" => "track"
        ],
        [],
        "tracks"
    ),
    mv!(
        "move.distance",
        "tf.move.distance",
        ["tracks" => "", "units" => "units", "time" => "time", "track" => "track"],
        [],
        "tracks"
    ),
    mv!(
        "move.time_lags",
        "tf.move.time_lags",
        ["tracks" => "", "units" => "units", "time" => "time", "track" => "track"],
        [],
        "tracks"
    ),
    mv!(
        "move.speed",
        "tf.move.speed",
        ["tracks" => "", "units" => "units", "time" => "time", "track" => "track"],
        [],
        "tracks"
    ),
    mv!(
        "move.azimuth",
        "tf.move.azimuth",
        ["tracks" => "", "units" => "units", "time" => "time", "track" => "track"],
        [],
        "tracks"
    ),
    mv!(
        "move.turn_angle",
        "tf.move.turn_angle",
        ["tracks" => "", "units" => "units", "time" => "time", "track" => "track"],
        [],
        "tracks"
    ),
    mv!("move.nsd", "tf.move.nsd", ["tracks" => "", "time" => "time", "track" => "track"], [], "tracks"),
    mv!(
        "move.time_of_day",
        "tf.move.time_of_day",
        [
            "tracks" => "", "include_crepuscule" => "include_crepuscule", "where" => "where", "time" => "time",
            "track" => "track"
        ],
        [],
        "tracks"
    ),
    mv!("move.track_data", "tf.move.track_data", ["tracks" => "", "time" => "time", "track" => "track"], [], "tracks"),
    mv!("move.track_lines", "tf.move.track_lines", ["tracks" => "", "time" => "time", "track" => "track"], [], "lines"),
    mv!("move.segments", "tf.move.segments", ["tracks" => "", "time" => "time", "track" => "track"], [], "segments"),
    mv!(
        "move.steps",
        "tf.move.steps",
        ["tracks" => "", "lonlat" => "lonlat", "keep_cols" => "keep_cols", "time" => "time", "track" => "track"],
        [],
        "steps"
    ),
    mv!(
        "move.distribution",
        "tf.move.make_distribution",
        [
            "name" => "", "shape" => "shape", "scale" => "scale", "rate" => "rate", "meanlog" => "meanlog",
            "sdlog" => "sdlog", "sd" => "sd", "min" => "min", "max" => "max", "kappa" => "kappa", "mu" => "mu"
        ],
        [],
        "distribution"
    ),
    mv!("move.fit_distribution", "tf.move.fit_distr", ["values" => "", "name" => "name"], [], "distribution"),
    mv!(
        "move.random_numbers",
        "tf.move.random_numbers",
        ["distribution" => "", "n" => "n", "seed" => "seed"],
        ["seed" = "1"],
        "table"
    ),
    mv!(
        "move.random_steps",
        "tf.move.random_steps",
        [
            "steps" => "", "n_control" => "n_control", "sl_distr" => "sl_distr", "ta_distr" => "ta_distr",
            "n_draws" => "n_draws", "seed" => "seed"
        ],
        ["seed" = "1"],
        "steps"
    ),
    mv!("move.extract_covariates", "tf.move.extract_covariates", ["tracks" => "", "where" => "where"], [], "tracks"),
    mv!(
        "move.surface_distance",
        "tf.move.surface_distance",
        ["steps" => "", "dem" => "dem", "spacing" => "spacing", "name" => "name"],
        [],
        "steps"
    ),
    mv!("move.fit_issf", "tf.move.fit_issf", ["steps" => "", "formula" => "formula", "strata" => "strata"], [], "model"),
    mv!(
        "move.log_rss",
        "tf.move.log_rss",
        ["model" => "", "ci" => "ci", "level" => "ci_level", "n_boot" => "n_boot", "seed" => "seed"],
        ["seed" = "1"],
        "table"
    ),
    mv!("move.issf_model", "tf.move.make_issf_model", ["sl_distr" => "sl", "ta_distr" => "ta"], [], "model"),
    mv!(
        "move.simulate_path",
        "tf.move.redistribution_kernel",
        [
            "model" => "", "sl_distr" => "sl_distr", "ta_distr" => "ta_distr", "n_control" => "n_control",
            "n_sample" => "n_sample", "tolerance_outside" => "tolerance_outside"
        ],
        [],
        "paths"
    ),
    mv!(
        "move.resistance",
        "tf.move.resistance",
        ["model" => "", "transform" => "transform", "c" => "c", "r_max" => "r_max", "scale" => "scale"],
        [],
        "resistance"
    ),
    mv!(
        "move.path_density",
        "tf.move.path_density",
        ["paths" => "", "like" => "like", "by" => "by", "include_start" => "include_start"],
        [],
        "density"
    ),
    mv!(
        "move.max_speed",
        "tf.move.v_phys_estimate",
        ["mass" => "mass", "mode" => "mode", "ci_level" => "ci_level"],
        [],
        "v_max"
    ),
    mv!(
        "move.flag_speed",
        "tf.move.flag_speed",
        [
            "tracks" => "", "v_max" => "v_max", "mass" => "mass", "mode" => "mode", "jitter" => "jitter",
            "remove" => "remove", "time" => "time", "track" => "track"
        ],
        [],
        "tracks"
    ),
    mv!(
        "move.filter_gps_quality",
        "tf.move.filter_gps_quality",
        [
            "tracks" => "", "sat_min" => "sat_min", "dop_max" => "dop_max", "hacc_max" => "hacc_max",
            "drop_empty" => "drop_empty"
        ],
        [],
        "tracks"
    ),
    mv!(
        "move.thin_time",
        "tf.move.thin_time",
        [
            "tracks" => "", "interval" => "interval", "tolerance" => "tolerance", "criterion" => "criterion",
            "time" => "time", "track" => "track"
        ],
        [],
        "tracks"
    ),
    mv!(
        "move.thin_distance",
        "tf.move.thin_distance",
        [
            "tracks" => "", "distance" => "distance", "method" => "method", "tolerance" => "tolerance",
            "criterion" => "criterion", "track" => "track"
        ],
        [],
        "tracks"
    ),
    mv!(
        "move.corridor",
        "tf.move.corridor",
        [
            "tracks" => "", "speed_threshold" => "speed_threshold", "circvar_threshold" => "circvar_threshold",
            "min_segments" => "min_segments", "time" => "time", "track" => "track"
        ],
        [],
        "tracks"
    ),
    mv!(
        "move.ud_distance",
        "tf.move.ud_distance",
        [
            "ud" => "", "ud_b" => "ud_b", "method" => "method", "mask_quantile" => "mask_quantile", "reg" => "reg",
            "max_iter" => "max_iter", "tol" => "tol", "gc" => "gc", "threshold" => "threshold",
            "exact_max_iter" => "exact_max_iter"
        ],
        [],
        "table"
    ),
    mv!(
        "move.validate",
        "tf.move.validate",
        [
            "use" => "", "surface" => "", "kind" => "kind", "threshold" => "threshold", "quantile" => "quantile",
            "support" => "support", "track" => "track", "spacing" => "spacing"
        ],
        [],
        "table"
    ),
    // the home range by method (its code: `special`, which calls hr_mcp, hr_kde or hr_locoh and the bandwidths)
    mv!(
        "move.home_range",
        "tf.move.home_range",
        ["tracks" => "", "method" => "method", "levels" => "levels"],
        [],
        "polygons"
    ),
    mv!(
        "move.home_range_overlap",
        "tf.move.hr_overlap",
        ["polygons" => "", "type" => "type", "conditional" => "conditional", "which" => "which"],
        [],
        "table"
    ),
    mv!("move.intensity_use", "tf.move.intensity_use", ["tracks" => "", "track" => "track"], [], "table"), // the dynamic Brownian bridges (their code: `special`)
    mv!(
        "move.dbbmm_variance",
        "tf.move.dbbmm_variance",
        [
            "tracks" => "", "location_error" => "location_error", "window_size" => "window_size", "margin" => "margin",
            "location_error_na" => "location_error_na", "burst" => "burst", "time" => "time", "track" => "track"
        ],
        ["location_error" = "None"],
        "tracks"
    ),
    mv!(
        "move.dbgb_variance",
        "tf.move.dbgb_variance",
        [
            "tracks" => "", "location_error" => "location_error", "window_size" => "window_size", "margin" => "margin",
            "location_error_na" => "location_error_na", "burst" => "burst", "time" => "time", "track" => "track"
        ],
        ["location_error" = "None"],
        "tracks"
    ),
    mv!(
        "move.dbbmm_ud",
        "tf.move.dbbmm_ud",
        [
            "tracks" => "", "like" => "like", "res" => "res", "dim_size" => "dim_size", "ext_factor" => "ext_factor",
            "time_step" => "time_step", "window_size" => "window_size", "margin" => "margin",
            "location_error" => "location_error", "location_error_na" => "location_error_na", "crs" => "crs",
            "max_cells" => "max_cells", "verbose" => "verbose", "burst" => "burst", "time" => "time",
            "track" => "track"
        ],
        [],
        "ud"
    ),
    mv!(
        "move.dbgb_ud",
        "tf.move.dbgb_ud",
        [
            "tracks" => "", "like" => "like", "res" => "res", "dim_size" => "dim_size", "ext_factor" => "ext_factor",
            "time_step" => "time_step", "window_size" => "window_size", "margin" => "margin",
            "location_error" => "location_error", "location_error_na" => "location_error_na", "crs" => "crs",
            "max_cells" => "max_cells", "verbose" => "verbose", "burst" => "burst", "time" => "time",
            "track" => "track"
        ],
        [],
        "ud"
    ),
    mv!(
        "move.suggest_window",
        "tf.move.suggest_dbbmm_window",
        ["tracks" => "", "target_hours" => "target_hours", "time" => "time", "track" => "track"],
        [],
        "table"
    ),
    mv!(
        "move.mask_segments",
        "tf.move.mask_segments",
        [
            "tracks" => "", "segments" => "segments", "model" => "model", "burst" => "burst", "time" => "time",
            "track" => "track"
        ],
        [],
        "tracks"
    ),
    mv!("move.ud_volume", "tf.move.ud_volume", ["ud" => ""], [], "volume"),
    mv!(
        "move.ud_outer_probability",
        "tf.move.ud_outer_probability",
        ["points" => "", "ud" => "ud", "band" => "band"],
        [],
        "points"
    ),
];

/// An input or setting as a Python expression; none when absent or an empty text (the usual names, no units).
fn opt(n: &Node, name: &str, sc: &Scope) -> Result<Option<String>, String> {
    Ok(value_of(n, name, sc)?.filter(|v| v != "\"\"" && v != "None"))
}

/// The call's arguments from the entry's mapping (the first positional `first`, if given, else the node's), then
/// the defaults; and the comments of the settings and inputs not passed (those in `also` are used elsewhere).
fn arguments(
    n: &Node,
    py: &Py,
    sc: &Scope,
    first: Option<String>,
    also: &[&str],
) -> Result<(Vec<String>, String), String> {
    let mut args: Vec<String> = Vec::new();
    let mut given: Vec<&str> = Vec::new();
    for (ours, theirs) in py.args {
        let v = match (&first, theirs.is_empty()) {
            (Some(f), true) => Some(f.clone()),
            _ => opt(n, ours, sc)?,
        };
        let Some(v) = v else {
            if theirs.is_empty() {
                return Err(format!("its {ours} is not connected or given"));
            }
            continue;
        };
        // the library's None is Terraflux's "none"
        if *ours == "keep_cols" && v == "\"none\"" {
            continue;
        }
        args.push(if theirs.is_empty() { v } else { format!("{theirs}={v}") });
        given.push(theirs);
    }
    for (k, v) in py.defaults {
        if !given.contains(k) {
            args.push(format!("{k}={v}"));
        }
    }
    let known = |k: &str| py.args.iter().any(|(a, _)| *a == k) || also.contains(&k);
    let mut said = String::new();
    for (k, v) in &n.params {
        if !known(k) {
            said.push_str(&format!("# (not passed: {k} = {})\n", literal(v)));
        }
    }
    for k in n.inputs.keys().filter(|k| !known(k)) {
        said.push_str(&format!("# (not passed: the input {k})\n"));
    }
    Ok((args, said))
}

/// The entry of an operator.
fn entry(op: &str) -> Result<&'static Py, String> {
    MAPPED.iter().find(|p| p.op == op).ok_or_else(|| format!("{op}: no counterpart"))
}

/// The attributes that are numbers (the booleans as 1 and 0) of a frame, a `feature` column (1...) first: the
/// operators' `table` output.
fn frame_table(v: &str) -> String {
    format!(
        "{v}.select_dtypes([\"number\", \"bool\"]).astype(\"float64\").reset_index(drop=True).rename_axis(\"feature\")\
         .reset_index().assign(feature=lambda d: d[\"feature\"] + 1)"
    )
}

/// What a measure of the tracks gives, as the column of a copy of them: the operator and the column's name.
const MEASURES: &[(&str, &str)] = &[
    ("move.distance", "distance"),
    ("move.time_lags", "time_lag"),
    ("move.speed", "speed"),
    ("move.azimuth", "azimuth"),
    ("move.turn_angle", "turn_angle"),
    ("move.nsd", "nsd"),
    ("move.time_of_day", "tod"),
];

/// Operators that give a frame (a call, then the suffix): the port of the result.
const FRAMES: &[(&str, &str, &str)] = &[
    ("move.read_tracks", "tracks", ""),
    ("move.simulate_brownian", "tracks", ""),
    ("move.filter_unique", "tracks", ""),
    ("move.filter_per_interval", "tracks", ""),
    ("move.filter_movebank_visible", "tracks", ""),
    ("move.filter_min_n_burst", "tracks", ""),
    ("move.resample", "tracks", ""),
    ("move.interpolate", "tracks", ""),
    ("move.track_lines", "lines", ""),
    // indexed by the track: the track as a column
    ("move.track_data", "tracks", ".reset_index()"),
    ("move.steps", "steps", ""),
    ("move.random_steps", "steps", ""),
    ("move.surface_distance", "steps", ""),
];

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let op = n.op.as_str();
    if !op.starts_with("move.") {
        return Ok(None);
    }
    let out = |pairs: &[(&str, String)]| -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
    };
    if let Some((_, column)) = MEASURES.iter().find(|(o, _)| *o == op) {
        let py = entry(op)?;
        let tracks = value_of(n, "tracks", sc)?.ok_or("its tracks are not connected")?;
        let (args, said) = arguments(n, py, sc, None, &["column"])?;
        let name = setting(n, "column", sc)?.filter(|c| c != "\"\"").unwrap_or_else(|| text(column));
        let code = format!("{said}{var} = {tracks}.copy()\n{var}[{name}] = {}({})\n", py.call, args.join(", "));
        return Ok(Some((code, out(&[("tracks", var.to_string()), ("table", frame_table(var))]), Need::Nothing)));
    }
    if let Some((_, port, suffix)) = FRAMES.iter().find(|(o, _, _)| *o == op) {
        let py = entry(op)?;
        let (args, said) = arguments(n, py, sc, None, &[])?;
        let code = format!("{said}{var} = {}({}){suffix}\n", py.call, args.join(", "));
        return Ok(Some((code, out(&[(port, var.to_string()), ("table", frame_table(var))]), Need::Nothing)));
    }
    let code: Code = match op {
        "move.as_tracks" => {
            let py = entry(op)?;
            // points, or a table with a column of x and one of y
            let vector = value_of(n, "vector", sc)?;
            let table = value_of(n, "table", sc)?;
            let by_table = vector.is_none();
            let data = vector.or(table).ok_or("connect the points (vector) or a table of locations (table)")?;
            let (mut args, said) = arguments(n, py, sc, Some(data), &["table"])?;
            if by_table {
                // the operator's default columns
                for k in ["x", "y"] {
                    if !args.iter().any(|a| a.starts_with(&format!("{k}="))) {
                        args.push(format!("{k}=\"{k}\""));
                    }
                }
            }
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("tracks", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
        }
        "move.aeqd_crs" => {
            let py = entry(op)?;
            let tracks = value_of(n, "tracks", sc)?.ok_or("its tracks are not connected")?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let crs = format!("{var}_crs");
            let code = format!("{said}{crs} = {}({})\n{var} = {tracks}.to_crs({crs})\n", py.call, args.join(", "));
            let outs = out(&[("crs", format!("{crs}.srs")), ("tracks", var.to_string()), ("table", frame_table(var))]);
            (code, outs, Need::Nothing)
        }
        "move.segments" => {
            let py = entry(op)?;
            let tracks = value_of(n, "tracks", sc)?.ok_or("its tracks are not connected")?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            // the lines (a GeoSeries) as the geometry of the tracks: a feature per location with its attributes
            let code = format!("{said}{var} = {tracks}.set_geometry({}({}))\n", py.call, args.join(", "));
            (code, out(&[("segments", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
        }
        "move.track_summary" => {
            let tracks = value_of(n, "tracks", sc)?.ok_or("its tracks are not connected")?;
            let mut by = String::new();
            for k in ["time", "track"] {
                if let Some(v) = opt(n, k, sc)? {
                    by.push_str(&format!(", {k}={v}"));
                }
            }
            // a line per track, then the summaries (one per track, in the order of the lines)
            let mut code = format!("{var} = tf.move.track_lines({tracks}{by})\n");
            for f in ["cum_dist", "tot_dist", "straightness", "msd", "sinuosity", "tac"] {
                code.push_str(&format!("{var}[\"{f}\"] = tf.move.{f}({tracks}{by}).to_numpy()\n"));
            }
            (code, out(&[("tracks", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
        }
        "move.distribution" => {
            let py = entry(op)?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            let table = format!("_pd.DataFrame([{var}.params])");
            (code, out(&[("distribution", var.to_string()), ("table", table)]), Need::Flow)
        }
        "move.fit_distribution" => {
            let py = entry(op)?;
            let values = value_of(n, "values", sc)?.ok_or("its values are not connected")?;
            let name = value_of(n, "name", sc)?.ok_or("its name is not given")?;
            // the column of the values: sl_ for lengths, ta_ for angles
            let column = match setting(n, "column", sc)?.filter(|c| c != "\"\"") {
                Some(c) => c,
                None => match n.params.get("name").and_then(Value::as_str) {
                    Some("vonmises" | "unif") => "\"ta_\"".to_string(),
                    Some(_) => "\"sl_\"".to_string(),
                    None => format!("(\"ta_\" if {name} in (\"vonmises\", \"unif\") else \"sl_\")"),
                },
            };
            let (_, said) = arguments(n, py, sc, Some(values.clone()), &["column"])?;
            let code = format!("{said}{var} = {}({values}[{column}], {name})\n", py.call);
            let table = format!("_pd.DataFrame([{var}.params])");
            (code, out(&[("distribution", var.to_string()), ("table", table)]), Need::Flow)
        }
        "move.random_numbers" => {
            let py = entry(op)?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("table", format!("_pd.DataFrame({{\"value\": {var}}})"))]), Need::Flow)
        }
        "move.extract_covariates" => {
            let py = entry(op)?;
            let tracks = value_of(n, "tracks", sc)?.ok_or("its tracks are not connected")?;
            // the rasters: the inputs of any other name, a column each
            let mut layers = Vec::new();
            for k in n.inputs.keys().filter(|k| k.as_str() != "tracks") {
                layers.push(format!("{}: {}", text(k), value_of(n, k, sc)?.ok_or("a raster is not connected")?));
            }
            if layers.is_empty() {
                return Err("connect the rasters (inputs of any names: forest, elev...)".into());
            }
            let dict = format!("{{{}}}", layers.join(", "));
            let mut args = vec![tracks.clone(), dict.clone()];
            let where_ = opt(n, "where", sc)?;
            if let Some(w) = &where_ {
                args.push(format!("where={w}"));
            }
            let plain = format!("{}({})", py.call, args.join(", "));
            // where = along: the library's along_summary (the summaries as columns); a reference to a setting
            // chooses when the script runs
            let by = n.params.get("where").and_then(Value::as_str);
            let mut along_args = vec![tracks.clone(), dict.clone()];
            match n.params.get("stats") {
                Some(Value::String(s)) if s.starts_with('$') => {
                    let v = opt(n, "stats", sc)?.ok_or("its stats are not given")?;
                    along_args
                        .push(format!("stats=[p.strip() for p in {v}.split(\",\")] if isinstance({v}, str) else {v}"));
                }
                Some(Value::String(s)) => {
                    let list: Vec<String> = s.split(',').map(str::trim).filter(|p| !p.is_empty()).map(text).collect();
                    if !list.is_empty() {
                        along_args.push(format!("stats=[{}]", list.join(", ")));
                    }
                }
                _ => {
                    if let Some(v) = opt(n, "stats", sc)? {
                        along_args.push(format!("stats={v}"));
                    }
                }
            }
            let summary = format!("tf.move.along_summary({})", along_args.join(", "));
            let call = match (by, &where_) {
                (Some("along"), _) => summary,
                (Some(w), Some(expr)) if w.starts_with('$') => format!("{summary} if {expr} == \"along\" else {plain}"),
                _ => plain,
            };
            let code = format!("{var} = {call}\n");
            // the values themselves (amt's extract_covariates_along): a row per cell in terra's order, the step
            // (1-based) first, then a column per raster
            let first = n.inputs.keys().find(|k| k.as_str() != "tracks").map(|k| text(k)).unwrap_or_default();
            let values = format!(
                "_pd.concat([(g if isinstance(g, _pd.DataFrame) else _pd.DataFrame({{{first}: g}})).assign(step=i + 1) \
                 for i, g in enumerate(tf.move.extract_covariates_along({tracks}, {dict}))], ignore_index=True)\
                 .pipe(lambda d: d[[\"step\", *d.columns[:-1]]])"
            );
            let outs = out(&[("tracks", var.to_string()), ("table", frame_table(var)), ("along", values)]);
            (code, outs, Need::Nothing)
        }
        "move.fit_issf" => {
            let py = entry(op)?;
            let steps = value_of(n, "steps", sc)?.ok_or("its steps are not connected")?;
            let text_of = |k: &str| -> Result<Option<String>, String> {
                match n.params.get(k) {
                    None | Some(Value::Null) => Ok(None),
                    Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
                    Some(Value::String(s)) if !s.starts_with('$') => Ok(Some(s.clone())),
                    Some(_) => Err(format!("its {k}: write it in the node, not as a reference")),
                }
            };
            let formula = text_of("formula")?;
            let by = text_of("strata")?.unwrap_or_else(|| "track_step".to_string());
            let in_formula = formula.as_ref().is_some_and(|f| f.contains("strata("));
            // (the library takes the distributions from the random steps themselves)
            let (_, said) = arguments(n, py, sc, None, &["formula", "strata", "track", "sl_distr", "ta_distr"])?;
            let names = terraflux_ops::movement::TRACK_NAMES.iter().map(|t| text(t)).collect::<Vec<_>>().join(", ");
            let mut code = said;
            let mut stratum = by.clone();
            let mut source = steps.clone();
            let names_it = formula.as_ref().is_some_and(|f| f.contains("stratum_"));
            if by == "track_step" && (!in_formula || names_it) {
                // one stratum for each step of each track: the step_id_ of the random steps restarts for each track
                // (also when the formula names stratum_ itself)
                let track = match opt(n, "track", sc)? {
                    Some(t) => t,
                    None => format!("next(c for c in ({names}) if c in {steps}.columns)"),
                };
                let by_step = format!("{steps}.groupby([{track}, \"step_id_\"], sort=False).ngroup() + 1");
                code.push_str(&format!("{var}_data = {steps}.assign(stratum_={by_step})\n"));
                stratum = "stratum_".to_string();
                source = format!("{var}_data");
            }
            let call = match &formula {
                Some(f) if in_formula => format!("{}({source}, {})", py.call, text(f)),
                Some(f) => format!("{}({source}, {}, strata={})", py.call, text(f), text(&stratum)),
                None => {
                    // every covariate extract_covariates added: the numbers whose names do not end in _, but the track
                    let track = match opt(n, "track", sc)? {
                        Some(t) => t,
                        None => format!("next((c for c in ({names}) if c in {source}.columns), None)"),
                    };
                    let name = "c if _re.fullmatch(r\"[A-Za-z.][A-Za-z0-9._]*\", c) else f\"`{c}`\"";
                    let keep = format!(
                        "{source}[c].dtype.kind in \"iuf\" and not c.endswith(\"_\") and c not in ({track}, \"direction_p\")"
                    );
                    code.push_str(&format!(
                        "import re as _re\n{var}_terms = [{name} for c in {source}.columns if {keep}]\n"
                    ));
                    let all =
                        format!("[*{var}_terms, \"log(sl_)\", \"cos(ta_)\", \"strata(\" + {} + \")\"]", text(&stratum));
                    format!("{}({source}, \"case_ ~ \" + \" + \".join({all}))", py.call)
                }
            };
            code.push_str(&format!("{var} = {call}\n"));
            let table = format!(
                "_pd.concat([{var}.summary(), {var}.confint().set_axis([\"lower\", \"upper\"], axis=1)], axis=1)\
                 .rename_axis(\"term\").reset_index()"
            );
            let fit = format!(
                "_pd.DataFrame([{{\"loglik_null\": {var}.loglik[0], \"loglik\": {var}.loglik[1], \"aic\": {var}.aic, \
                 \"n\": {var}.n, \"nevent\": {var}.nevent, \"iter\": {var}.iter, \"converged\": float({var}.converged), \
                 \"n_omitted\": {var}.n_omitted, \"wald\": {var}.wald, \"score\": {var}.score}}])"
            );
            (code, out(&[("model", var.to_string()), ("table", table), ("fit", fit)]), Need::Flow)
        }
        "move.log_rss" => {
            let py = entry(op)?;
            let model = value_of(n, "model", sc)?.ok_or("its model is not connected")?;
            // x1 and x2: the table connected, or the setting's columns as a data frame
            let frame = |port: &str, one: bool| -> Result<String, String> {
                if n.inputs.contains_key(port) {
                    return value_of(n, port, sc)?.ok_or_else(|| format!("its {port} is not connected"));
                }
                let index = if one { ", index=[0]" } else { "" };
                match n.params.get(port) {
                    Some(Value::Object(_)) => {
                        Ok(format!("_pd.DataFrame({}{index})", opt(n, port, sc)?.unwrap_or_default()))
                    }
                    Some(Value::String(s)) if s.starts_with('$') => {
                        Ok(format!("_pd.DataFrame({}{index})", opt(n, port, sc)?.unwrap_or_default()))
                    }
                    _ => Err(format!("its {port} is not given: connect a table or give the columns")),
                }
            };
            let (x1, x2) = (frame("x1", false)?, frame("x2", true)?);
            let (mut args, said) = arguments(n, py, sc, Some(model), &["x1", "x2"])?;
            args.insert(1, format!("x1={x1}"));
            args.insert(2, format!("x2={x2}"));
            // the library's None is Terraflux's "none"
            args.retain(|a| a != "ci=\"none\"");
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("table", var.to_string())]), Need::Flow)
        }
        "move.issf_model" => {
            let py = entry(op)?;
            // the coefficients: an object of terms, or a list of [term, value] pairs (the order they are summed in)
            let coefs = match n.params.get("coefficients") {
                Some(Value::Object(_)) => opt(n, "coefficients", sc)?.unwrap_or_default(),
                Some(Value::Array(_)) => format!("dict({})", opt(n, "coefficients", sc)?.unwrap_or_default()),
                Some(Value::String(s)) if s.starts_with('$') => opt(n, "coefficients", sc)?.unwrap_or_default(),
                _ => return Err("its coefficients are not given".into()),
            };
            let (mut args, said) = arguments(n, py, sc, None, &["coefficients"])?;
            args.insert(0, coefs);
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("model", var.to_string())]), Need::Nothing)
        }
        "move.simulate_path" => {
            let py = entry(op)?;
            let model = value_of(n, "model", sc)?.ok_or("its model is not connected")?;
            // the covariates: the inputs of any other name, a layer each
            let skip = ["model", "start", "sl_distr", "ta_distr"];
            let names: Vec<&str> = n.inputs.keys().map(String::as_str).filter(|k| !skip.contains(k)).collect();
            let mut layers = Vec::new();
            for k in &names {
                layers.push(format!("{}: {}", text(k), value_of(n, k, sc)?.ok_or("a raster is not connected")?));
            }
            if layers.is_empty() {
                return Err("connect the covariates (rasters, inputs of any names: forest, elev...)".into());
            }
            let mut also = vec!["n_steps", "n_paths", "x", "y", "direction", "time", "dt", "seed", "start"];
            also.extend(&names);
            let (mut args, said) = arguments(n, py, sc, Some(model), &also)?;
            let number =
                |k: &str, d: &str| -> Result<String, String> { Ok(opt(n, k, sc)?.unwrap_or_else(|| d.to_string())) };
            let (steps, paths, seed) = (number("n_steps", "100")?, number("n_paths", "1")?, number("seed", "1")?);
            let (direction, when, dt) = (opt(n, "direction", sc)?, opt(n, "time", sc)?, opt(n, "dt", sc)?);
            let mut code = said;
            let starts = format!("{var}_starts");
            if let Some(s) = value_of(n, "start", sc)? {
                // the first location of each track, with its time and the direction of its first step
                let names = terraflux_ops::movement::TRACK_NAMES.iter().map(|t| text(t)).collect::<Vec<_>>().join(", ");
                let (tc, tt) = (format!("{var}_track"), format!("{var}_time"));
                let kw = |k: &str, v: Option<String>, d: &str| match v {
                    Some(v) => format!(", {k}={v}"),
                    None if d.is_empty() => String::new(),
                    None => format!(", {k}={d}"),
                };
                let dir = direction
                    .unwrap_or_else(|| "float(np.arctan2(b.y - a.y, b.x - a.x)) if b is not None else 0.0".to_string());
                let stamps = "(\"timestamp\", \"time\", \"datetime\", \"date_time\", \"t\")";
                let time = kw("time", when, &format!("(g[{tt}].iloc[0] if {tt} else None)"));
                code.push_str(&format!(
                    "{tc} = next((c for c in ({names}) if c in {s}.columns), None)\n\
                     {tt} = next((c for c in {stamps} if c in {s}.columns), None)\n\
                     {starts} = []\n\
                     for _, g in ({s}.groupby({tc}, sort=False) if {tc} else [(None, {s})]):\n    \
                     a, b = g.geometry.iloc[0], (g.geometry.iloc[1] if len(g) > 1 else None)\n    \
                     {starts}.append((g[{tc}].iloc[0] if {tc} else None, tf.move.make_start((a.x, a.y), {dir}{time}{})))\n",
                    kw("dt", dt, ""),
                ));
            } else {
                let (x, y) = (opt(n, "x", sc)?, opt(n, "y", sc)?);
                let (Some(x), Some(y)) = (x, y) else {
                    return Err(
                        "give the start: a vector of locations as the input start, or the settings x and y".into()
                    );
                };
                let mut extra = String::new();
                if let Some(t) = when {
                    extra.push_str(&format!(", time={t}"));
                }
                if let Some(d) = dt {
                    extra.push_str(&format!(", dt={d}"));
                }
                code.push_str(&format!(
                    "{starts} = [(None, tf.move.make_start(({x}, {y}), {}{extra}))]\n",
                    direction.unwrap_or_else(|| "0.0".to_string())
                ));
            }
            args.insert(1, format!("{var}_start"));
            args.insert(2, format!("{{{}}}", layers.join(", ")));
            args.push(format!("seed={seed} + {var}_i"));
            let label =
                format!("(str({var}_label) + \"_path_\" if {var}_label is not None else \"path_\") + str({var}_i + 1)");
            code.push_str(&format!(
                "import pandas as _pd\n\
                 {var}_all = []\n\
                 for {var}_label, {var}_start in {starts}:\n    \
                 for {var}_i in range({paths}):\n        \
                 {var}_k = {}({})\n        \
                 {var}_all.append(tf.move.simulate_path({var}_k, {steps}).assign(track={label}))\n\
                 {var} = _pd.concat({var}_all, ignore_index=True)\n",
                py.call,
                args.join(", ")
            ));
            (code, out(&[("paths", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
        }
        "move.resistance" => {
            let py = entry(op)?;
            let model = value_of(n, "model", sc)?.ok_or("its model is not connected")?;
            let names: Vec<&str> = n.inputs.keys().map(String::as_str).filter(|k| *k != "model").collect();
            let mut layers = Vec::new();
            for k in &names {
                layers.push(format!("{}: {}", text(k), value_of(n, k, sc)?.ok_or("a raster is not connected")?));
            }
            if layers.is_empty() {
                return Err("connect the covariates (rasters, inputs of any names: forest, elev...)".into());
            }
            let (mut args, said) = arguments(n, py, sc, Some(model), &names)?;
            args.insert(1, format!("{{{}}}", layers.join(", ")));
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("resistance", var.to_string())]), Need::Nothing)
        }
        "move.path_density" => {
            let py = entry(op)?;
            opt(n, "paths", sc)?.ok_or("its paths are not connected")?;
            opt(n, "like", sc)?.ok_or("its like is not connected")?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("density", var.to_string())]), Need::Nothing)
        }
        "move.max_speed" => {
            let py = entry(op)?;
            opt(n, "mass", sc)?.ok_or("its mass is not given")?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            let table = format!(
                "_pd.DataFrame([{{\"mass\": {var}.mass, \"v_max\": {var}.v_max, \"lower\": {var}.lower, \
                 \"upper\": {var}.upper, \"kmh\": {var}.kmh}}])"
            );
            let outs = out(&[
                ("v_max", format!("{var}.v_max")),
                ("lower", format!("{var}.lower")),
                ("upper", format!("{var}.upper")),
                ("kmh", format!("{var}.kmh")),
                ("table", table),
            ]);
            (code, outs, Need::Flow)
        }
        "move.flag_speed" => {
            let py = entry(op)?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            let outs = out(&[
                ("tracks", var.to_string()),
                ("table", frame_table(var)),
                ("v_max", format!("{var}.attrs[\"v_max_used\"]")),
            ]);
            (code, outs, Need::Nothing)
        }
        "move.filter_gps_quality" => {
            let py = entry(op)?;
            let (mut args, said) = arguments(n, py, sc, None, &[])?;
            // the library's None is Terraflux's "none": no limit
            for a in args.iter_mut() {
                if let Some((k, v)) = a.split_once('=')
                    && ["sat_min", "dop_max", "hacc_max"].contains(&k)
                    && v.trim_matches('"').eq_ignore_ascii_case("none")
                {
                    *a = format!("{k}=None");
                }
            }
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("tracks", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
        }
        "move.thin_time" | "move.thin_distance" => {
            let py = entry(op)?;
            let tracks = value_of(n, "tracks", sc)?.ok_or("its tracks are not connected")?;
            let (args, said) = arguments(n, py, sc, None, &["output"])?;
            // the selected locations (the library's filter_thin_*), or all with the column thin_selected
            let code = match n.params.get("output").and_then(Value::as_str).unwrap_or("selected") {
                "selected" => format!(
                    "{said}{var} = {}({})\n",
                    py.call.replacen("tf.move.", "tf.move.filter_", 1),
                    args.join(", ")
                ),
                "flag" => format!(
                    "{said}{var} = {tracks}.copy()\n{var}[\"thin_selected\"] = {}({})\n",
                    py.call,
                    args.join(", ")
                ),
                o if o.starts_with('$') => {
                    return Err("write its output (selected or flag) in the node, not as a reference".into());
                }
                o => return Err(format!("its output {o:?}: selected or flag")),
            };
            (code, out(&[("tracks", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
        }
        "move.corridor" => {
            let py = entry(op)?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            // the corridor segments as lines (the columns of the tracks are named as the tracks'); the thresholds the
            // library used are in the attributes of the result
            let mut by = String::new();
            for k in ["time", "track"] {
                if let Some(v) = opt(n, k, sc)? {
                    by.push_str(&format!(", {k}={v}"));
                }
            }
            let used = format!("{var}.attrs[\"corridor\"]");
            let outs = out(&[
                ("tracks", var.to_string()),
                ("table", frame_table(var)),
                ("lines", format!("tf.move.corridor_lines({var}{by})")),
                ("speed_threshold", format!("{used}[\"speed_threshold\"]")),
                ("circvar_threshold", format!("{used}[\"circvar_threshold\"]")),
                ("thresholds", format!("_pd.DataFrame([{used}])")),
            ]);
            (code, outs, Need::Flow)
        }
        "move.ud_distance" => {
            let py = entry(op)?;
            opt(n, "ud", sc)?.ok_or("its ud is not connected")?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            // a number for two UDs, a table of the pairs for more
            let table = format!(
                "({var} if isinstance({var}, _pd.DataFrame) else \
                 _pd.DataFrame({{\"a\": [\"ud_1\"], \"b\": [\"ud_2\"], \"distance\": [{var}]}}))"
            );
            (code, out(&[("distance", var.to_string()), ("table", table)]), Need::Flow)
        }
        "move.validate" => {
            let py = entry(op)?;
            opt(n, "use", sc)?.ok_or("its use is not connected")?;
            opt(n, "surface", sc)?.ok_or("its surface is not connected")?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            let outs = out(&[
                ("table", format!("{var}.table")),
                ("note", format!("{var}.note")),
                ("threshold", format!("{var}.threshold")),
                ("corridor", format!("{var}.corridor")),
            ]);
            (code, outs, Need::Flow)
        }
        "move.home_range" => {
            entry(op)?;
            let tracks = value_of(n, "tracks", sc)?.ok_or("its tracks are not connected")?;
            let method = match n.params.get("method") {
                None => "mcp",
                Some(Value::String(m)) if ["mcp", "kde", "locoh"].contains(&m.as_str()) => m.as_str(),
                Some(_) => return Err("its method: write mcp, kde or locoh in the node, not as a reference".into()),
            };
            let used: &[&str] = match method {
                "mcp" => &["method", "levels", "track"],
                "locoh" => &["method", "levels", "track", "type", "n", "rand_buffer"],
                _ => &["method", "levels", "track", "bandwidth", "rescale", "which_min", "num_parts", "res", "factor"],
            };
            let mut said = String::new();
            for (k, v) in &n.params {
                if !used.contains(&k.as_str()) {
                    said.push_str(&format!("# (not passed: {k} = {})\n", literal(v)));
                }
            }
            for k in n.inputs.keys().filter(|k| k.as_str() != "tracks" && !(k.as_str() == "like" && method == "kde")) {
                said.push_str(&format!("# (not passed: the input {k})\n"));
            }
            // the arguments of the settings given, as ", name=value"
            let kw = |names: &[&str]| -> Result<String, String> {
                let mut s = String::new();
                for k in names {
                    if let Some(v) = opt(n, k, sc)? {
                        s.push_str(&format!(", {k}={v}"));
                    }
                }
                Ok(s)
            };
            let mut outs = vec![
                ("polygons", format!("tf.move.hr_isopleths({var})")),
                ("table", format!("tf.move.hr_area({var}).drop(columns=\"what\")")),
            ];
            let code = match method {
                "mcp" => format!("{said}{var} = tf.move.hr_mcp({tracks}{})\n", kw(&["levels", "track"])?),
                "locoh" => format!(
                    "{said}{var} = tf.move.hr_locoh({tracks}{})\n",
                    kw(&["n", "type", "levels", "rand_buffer", "track"])?
                ),
                _ => {
                    let mut code = said;
                    let like = value_of(n, "like", sc)?;
                    let by_size = kw(&["factor", "res", "track"])?;
                    let trast = match like {
                        Some(l) => Some(l),
                        None if by_size.contains("factor=") || by_size.contains("res=") => {
                            code.push_str(&format!("{var}_trast = tf.move.make_trast({tracks}{by_size})\n"));
                            Some(format!("{var}_trast"))
                        }
                        None => None,
                    };
                    let (rescale, track) = (kw(&["rescale"])?, kw(&["track"])?);
                    let trast_kw = trast.as_ref().map(|t| format!(", trast={t}")).unwrap_or_default();
                    let h = match n.params.get("bandwidth") {
                        None | Some(Value::Null) => None,
                        Some(Value::String(s)) if s.is_empty() || s == "ref" => None,
                        Some(Value::String(s)) if s == "pi" => {
                            Some(format!("tf.move.hr_kde_pi({tracks}{rescale}{track})"))
                        }
                        Some(Value::String(s)) if s == "lscv" => {
                            Some(format!("tf.move.hr_kde_lscv({tracks}{}{rescale}{track}).h", kw(&["which_min"])?))
                        }
                        Some(Value::String(s)) if s == "ref_scaled" => {
                            // the isopleth searched is the highest level
                            let top = match n.params.get("levels") {
                                Some(Value::Array(a)) => {
                                    a.iter().filter_map(Value::as_f64).fold(f64::MIN, f64::max).to_string()
                                }
                                Some(Value::Number(x)) => x.to_string(),
                                Some(_) => {
                                    let l = opt(n, "levels", sc)?.ok_or("its levels are not given")?;
                                    format!("(max({l}) if isinstance({l}, (list, tuple)) else {l})")
                                }
                                None => "0.95".to_string(),
                            };
                            Some(format!(
                                "tf.move.hr_kde_ref_scaled({tracks}{trast_kw}, levels={top}{}{track})[\"h\"].to_dict()",
                                kw(&["num_parts"])?
                            ))
                        }
                        Some(Value::String(s)) if s.starts_with('$') => opt(n, "bandwidth", sc)?,
                        Some(Value::String(s)) => {
                            return Err(format!("its bandwidth {s:?}: ref, ref_scaled, pi or lscv"));
                        }
                        Some(v) => Some(literal(v)),
                    };
                    let h = match h {
                        None if !rescale.is_empty() => Some(format!("tf.move.hr_kde_ref({tracks}{rescale}{track})")),
                        h => h,
                    };
                    let h_kw = h.map(|h| format!(", h={h}")).unwrap_or_default();
                    code.push_str(&format!(
                        "{var} = tf.move.hr_kde({tracks}{h_kw}{trast_kw}{}{track})\n",
                        kw(&["levels"])?
                    ));
                    outs.push(("ud", format!("tf.move.hr_ud({var})")));
                    outs.push(("bandwidths", format!("{var}.h")));
                    code
                }
            };
            (code, out(&outs), Need::Nothing)
        }
        "move.home_range_overlap" => {
            let py = entry(op)?;
            // the library's overlap takes the estimates themselves: the one the polygons or the UD come from
            let from = opt(n, "polygons", sc)?.or(opt(n, "ud", sc)?).ok_or("connect the polygons or the ud")?;
            let hr = ["tf.move.hr_isopleths(", "tf.move.hr_ud("]
                .iter()
                .find_map(|p| from.strip_prefix(p).and_then(|r| r.strip_suffix(')')))
                .ok_or("its polygons or ud are not those of move.home_range: the script cannot rebuild the overlap")?;
            let (args, said) = arguments(n, py, sc, Some(hr.to_string()), &["ud"])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("table", var.to_string())]), Need::Nothing)
        }
        "move.intensity_use" => {
            let py = entry(op)?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            // a Series by track: the track as a column
            let code = format!("{said}{var} = {}({}).reset_index()\n", py.call, args.join(", "));
            (code, out(&[("table", var.to_string())]), Need::Nothing)
        }
        "move.dbbmm_variance" | "move.dbgb_variance" | "move.dbbmm_ud" | "move.dbgb_ud" => {
            let py = entry(op)?;
            let (mut args, said) = arguments(n, py, sc, None, &[])?;
            // the library's None is Terraflux's "none"
            for a in args.iter_mut() {
                if let Some((k, v)) = a.split_once('=')
                    && ["burst", "location_error"].contains(&k)
                    && v.trim_matches('"').eq_ignore_ascii_case("none")
                {
                    *a = format!("{k}=None");
                }
            }
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            if op.ends_with("_ud") {
                (code, out(&[("ud", var.to_string())]), Need::Nothing)
            } else {
                (code, out(&[("tracks", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
            }
        }
        "move.suggest_window" => {
            let py = entry(op)?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            // one suggestion for a track, a dict by track for several, none for a track too short
            let table = format!(
                "_pd.DataFrame([{{\"track\": k, **(v._asdict() if v is not None else {{}})}} for k, v in \
                 ({var} if isinstance({var}, dict) else {{1: {var}}}).items()])"
            );
            (code, out(&[("table", table)]), Need::Flow)
        }
        "move.mask_segments" => {
            let py = entry(op)?;
            let tracks = value_of(n, "tracks", sc)?.ok_or("its tracks are not connected")?;
            let given = |k: &str| n.params.get(k).is_some_and(|v| !v.is_null());
            if given("gap") {
                return Err("its gap: the library's mask_segments takes the segments themselves (segments)".into());
            }
            if given("column") && given("segments") {
                return Err("its segments and its column together: use one node for each".into());
            }
            let (mut args, said) = arguments(n, py, sc, None, &["column", "gap"])?;
            // a column of the tracks: the library takes one flag for each location
            if let Some(c) = opt(n, "column", sc)? {
                args.insert(1, format!("segments={tracks}[{c}].fillna(0).to_numpy(bool)"));
            }
            for a in args.iter_mut() {
                if let Some((k, v)) = a.split_once('=')
                    && k == "burst"
                    && v.trim_matches('"').eq_ignore_ascii_case("none")
                {
                    *a = "burst=None".to_string();
                }
            }
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("tracks", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
        }
        "move.ud_volume" => {
            let py = entry(op)?;
            let (args, said) = arguments(n, py, sc, None, &[])?;
            let code = format!("{said}{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("volume", var.to_string())]), Need::Nothing)
        }
        "move.ud_outer_probability" => {
            let py = entry(op)?;
            let points = value_of(n, "points", sc)?.ok_or("its points are not connected")?;
            let ud = value_of(n, "ud", sc)?.ok_or("its ud is not connected")?;
            let (_, said) = arguments(n, py, sc, None, &[])?;
            // the node's band is a name or a position from 1, the library's a label or a position from 0
            let band = match n.params.get("band") {
                None | Some(Value::Null) => String::new(),
                Some(Value::Number(x)) if x.as_i64().is_some() => format!(", band={}", x.as_i64().unwrap() - 1),
                Some(Value::String(s)) if !s.starts_with('$') => format!(", band={}", text(s)),
                Some(_) => return Err("its band: write a name or a position in the node, not as a reference".into()),
            };
            let code = format!(
                "{said}{var} = {points}.copy()\n{var}[\"outer_probability\"] = {}({points}, {ud}{band})\n",
                py.call
            );
            (code, out(&[("points", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
        }
        _ => return Ok(None),
    };
    Ok(Some(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script parses as Python (when there is a Python to ask).
    fn parses(t: &str) {
        if let Ok(o) = std::process::Command::new("python")
            .args(["-I", "-c", "import ast, sys; ast.parse(sys.stdin.read())"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(t.as_bytes())?;
                c.wait_with_output()
            })
        {
            assert!(o.status.success(), "{}\n{t}", String::from_utf8_lossy(&o.stderr));
        }
    }

    /// Every operator of the family has a translation (an entry here, or code of its own).
    #[test]
    fn every_move_operator_is_translated() {
        let r = Registry::standard();
        for s in r.signatures().iter().filter(|s| s.name.starts_with("move.")) {
            assert!(MAPPED.iter().any(|p| p.op == s.name) || s.name == "move.track_summary", "{}", s.name);
        }
    }

    /// A step selection function: the tracks read, projected, resampled, their steps, the distributions fitted,
    /// the random steps and the covariates at their ends.
    #[test]
    fn a_step_selection_function_as_the_library() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "clean"
op = "move.filter_unique"
inputs = { tracks = "tracks.tracks" }

[[nodes]]
id = "hourly"
op = "move.filter_per_interval"
inputs = { tracks = "clean.tracks" }
params = { unit = "2 hours", criterion = "random" }

[[nodes]]
id = "local"
op = "move.aeqd_crs"
inputs = { tracks = "hourly.tracks" }

[[nodes]]
id = "regular"
op = "move.resample"
inputs = { tracks = "local.tracks" }
params = { rate = "2h", tolerance = "15min" }

[[nodes]]
id = "bursts"
op = "move.filter_min_n_burst"
inputs = { tracks = "regular.tracks" }
params = { min_n = 4 }

[[nodes]]
id = "speed"
op = "move.speed"
inputs = { tracks = "bursts.tracks" }
params = { units = "km/h", column = "kmh" }

[[nodes]]
id = "steps"
op = "move.steps"
inputs = { tracks = "speed.tracks" }
params = { keep_cols = "none" }

[[nodes]]
id = "lengths"
op = "move.fit_distribution"
inputs = { values = "steps.steps" }
params = { name = "gamma" }

[[nodes]]
id = "angles"
op = "move.fit_distribution"
inputs = { values = "steps.steps" }
params = { name = "vonmises" }

[[nodes]]
id = "draws"
op = "move.random_numbers"
inputs = { distribution = "lengths.distribution" }
params = { n = 50 }

[[nodes]]
id = "random"
op = "move.random_steps"
inputs = { steps = "steps.steps", sl_distr = "lengths.distribution", ta_distr = "angles.distribution" }
params = { n_control = 5, seed = 7 }

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "forest.tif" }

[[nodes]]
id = "covariates"
op = "move.extract_covariates"
inputs = { tracks = "random.steps", forest = "forest.raster" }
params = { where = "both" }

[[nodes]]
id = "day"
op = "move.time_of_day"
inputs = { tracks = "steps.steps" }
params = { include_crepuscule = true }

[[nodes]]
id = "summary"
op = "move.track_summary"
inputs = { tracks = "hourly.tracks" }

[[nodes]]
id = "gamma"
op = "move.distribution"
params = { name = "gamma", shape = 2.0, rate = 0.5 }

[[nodes]]
id = "coords"
op = "table.read"
params = { path = "coords.csv" }

[[nodes]]
id = "located"
op = "move.as_tracks"
inputs = { table = "coords.table" }
params = { time = "t", track = "id", crs = "EPSG:32722" }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(t.contains("tracks = tf.move.load_tracks(MODEL_DIR / \"fishers.csv\")\n"), "{t}");
        assert!(t.contains("clean = tf.move.filter_unique(tracks, seed=1)\n"), "{t}");
        assert!(
            t.contains("hourly = tf.move.filter_per_interval(clean, unit=\"2 hours\", criterion=\"random\", seed=1)\n"),
            "{t}"
        );
        assert!(t.contains("local_crs = tf.move.aeqd_crs(hourly)\nlocal = hourly.to_crs(local_crs)\n"), "{t}");
        assert!(t.contains("regular = tf.move.resample(local, rate=\"2h\", tolerance=\"15min\")\n"), "{t}");
        assert!(t.contains("bursts = tf.move.filter_min_n_burst(regular, min_n=4)\n"), "{t}");
        assert!(t.contains("speed = bursts.copy()\nspeed[\"kmh\"] = tf.move.speed(bursts, units=\"km/h\")\n"), "{t}");
        // keep_cols "none" is the library's default
        assert!(t.contains("steps = tf.move.steps(speed)\n"), "{t}");
        assert!(t.contains("lengths = tf.move.fit_distr(steps[\"sl_\"], \"gamma\")\n"), "{t}");
        assert!(t.contains("angles = tf.move.fit_distr(steps[\"ta_\"], \"vonmises\")\n"), "{t}");
        assert!(t.contains("draws = tf.move.random_numbers(lengths, n=50, seed=1)\n"), "{t}");
        assert!(
            t.contains(
                "random = tf.move.random_steps(steps, n_control=5, sl_distr=lengths, ta_distr=angles, seed=7)\n"
            ) || t.contains(
                "random = tf.move.random_steps(steps, sl_distr=lengths, ta_distr=angles, n_control=5, seed=7)\n"
            ),
            "{t}"
        );
        assert!(
            t.contains("covariates = tf.move.extract_covariates(random, {\"forest\": forest}, where=\"both\")\n"),
            "{t}"
        );
        assert!(t.contains("day[\"tod\"] = tf.move.time_of_day(steps, include_crepuscule=True)\n"), "{t}");
        assert!(
            t.contains(
                "summary = tf.move.track_lines(hourly)\nsummary[\"cum_dist\"] = tf.move.cum_dist(hourly).to_numpy()\n"
            ),
            "{t}"
        );
        assert!(t.contains("gamma = tf.move.make_distribution(\"gamma\", shape=2.0, rate=0.5)\n"), "{t}");
        assert!(
            t.contains(
                "located = tf.move.as_tracks(coords, time=\"t\", track=\"id\", crs=\"EPSG:32722\", x=\"x\", y=\"y\")\n"
            ),
            "{t}"
        );
        parses(t);
    }

    /// Cleaning and thinning: the speed limit of a mass, the cap, the GPS filter and the thinning of tracks.
    #[test]
    fn cleaning_and_thinning_as_the_library() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "limit"
op = "move.max_speed"
params = { mass = 5, mode = "running", ci_level = 0.9 }

[[nodes]]
id = "good"
op = "move.filter_gps_quality"
inputs = { tracks = "tracks.tracks" }
params = { sat_min = 6, dop_max = "none" }

[[nodes]]
id = "flagged"
op = "move.flag_speed"
inputs = { tracks = "good.tracks" }
params = { v_max = 12.5, jitter = 3, remove = true }

[[nodes]]
id = "by_mass"
op = "move.flag_speed"
inputs = { tracks = "good.tracks" }
params = { mass = 2, mode = "flying" }

[[nodes]]
id = "hourly"
op = "move.thin_time"
inputs = { tracks = "flagged.tracks" }
params = { interval = "1 hour", tolerance = "10 mins", criterion = "first" }

[[nodes]]
id = "marked"
op = "move.thin_time"
inputs = { tracks = "flagged.tracks" }
params = { interval = 3600, output = "flag" }

[[nodes]]
id = "spaced"
op = "move.thin_distance"
inputs = { tracks = "hourly.tracks" }
params = { distance = 300, method = "interval", tolerance = 60 }

[[nodes]]
id = "marked_distance"
op = "move.thin_distance"
inputs = { tracks = "hourly.tracks" }
params = { distance = 300, output = "flag" }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(t.contains("limit = tf.move.v_phys_estimate(mass=5, mode=\"running\", ci_level=0.9)\n"), "{t}");
        assert!(t.contains("good = tf.move.filter_gps_quality(tracks, sat_min=6, dop_max=None)\n"), "{t}");
        assert!(t.contains("flagged = tf.move.flag_speed(good, v_max=12.5, jitter=3, remove=True)\n"), "{t}");
        assert!(t.contains("by_mass = tf.move.flag_speed(good, mass=2, mode=\"flying\")\n"), "{t}");
        assert!(
            t.contains("hourly = tf.move.filter_thin_time(flagged, interval=\"1 hour\", tolerance=\"10 mins\", criterion=\"first\")\n"),
            "{t}"
        );
        assert!(
            t.contains(
                "marked = flagged.copy()\nmarked[\"thin_selected\"] = tf.move.thin_time(flagged, interval=3600)\n"
            ),
            "{t}"
        );
        assert!(
            t.contains(
                "spaced = tf.move.filter_thin_distance(hourly, distance=300, method=\"interval\", tolerance=60)\n"
            ),
            "{t}"
        );
        assert!(
            t.contains("marked_distance = hourly.copy()\nmarked_distance[\"thin_selected\"] = tf.move.thin_distance(hourly, distance=300)\n"),
            "{t}"
        );
        parses(t);
    }

    /// Corridors and the distance between UDs: the library's calls, the segments as lines, the thresholds used.
    #[test]
    fn corridors_and_ud_distances_as_the_library() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "local"
op = "move.aeqd_crs"
inputs = { tracks = "tracks.tracks" }

[[nodes]]
id = "paths"
op = "move.corridor"
inputs = { tracks = "local.tracks" }
params = { speed_threshold = 0.3, circvar_threshold = 0.25, min_segments = 3, time = "t" }

[[nodes]]
id = "uds"
op = "raster.read"
params = { path = "uds.tif" }

[[nodes]]
id = "apart"
op = "move.ud_distance"
inputs = { ud = "uds.raster" }
params = { method = "exact", mask_quantile = 0.9 }

[[nodes]]
id = "gap"
op = "move.ud_distance"
inputs = { ud = "uds.raster", ud_b = "uds.raster" }
params = { gc = true, reg = 0.02, max_iter = 1500 }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(
            t.contains(
                "paths = tf.move.corridor(local, speed_threshold=0.3, circvar_threshold=0.25, min_segments=3, time=\"t\")\n"
            ),
            "{t}"
        );
        assert!(t.contains("apart = tf.move.ud_distance(uds, method=\"exact\", mask_quantile=0.9)\n"), "{t}");
        assert!(t.contains("gap = tf.move.ud_distance(uds, ud_b=uds, reg=0.02, max_iter=1500, gc=True)\n"), "{t}");
        parses(t);
    }

    /// The validation of a surface with the use of the animals: the library's call, its parts as the outputs.
    #[test]
    fn validation_as_the_library() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "uds"
op = "raster.read"
params = { path = "uds.tif" }

[[nodes]]
id = "current"
op = "raster.read"
params = { path = "current.tif" }

[[nodes]]
id = "check"
op = "move.validate"
inputs = { use = "uds.raster", surface = "current.raster" }
params = { kind = "resistance", quantile = 0.8, support = 1 }

[[nodes]]
id = "note"
op = "table.write"
inputs = { table = "check.table" }
params = { path = "check.csv" }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(
            t.contains("check = tf.move.validate(uds, current, kind=\"resistance\", quantile=0.8, support=1)\n"),
            "{t}"
        );
        assert!(t.contains("check.table"), "{t}");
        parses(t);
    }

    /// Home ranges: the library's hr_mcp, hr_kde (with the bandwidth chosen, the template), hr_locoh, the isopleths and
    /// the areas as outputs, the overlaps by the estimates they came from, and the intensity of use.
    #[test]
    fn home_ranges_as_the_library() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "local"
op = "move.aeqd_crs"
inputs = { tracks = "tracks.tracks" }

[[nodes]]
id = "box"
op = "move.home_range"
inputs = { tracks = "local.tracks" }
params = { levels = [0.5, 0.95] }

[[nodes]]
id = "kernel"
op = "move.home_range"
inputs = { tracks = "local.tracks" }
params = { method = "kde", levels = [0.5, 0.9], bandwidth = "pi", rescale = "unitvar", res = 50 }

[[nodes]]
id = "scaled"
op = "move.home_range"
inputs = { tracks = "local.tracks" }
params = { method = "kde", levels = 0.9, bandwidth = "ref_scaled", num_parts = 2, factor = 2, track = "id" }

[[nodes]]
id = "cv"
op = "move.home_range"
inputs = { tracks = "local.tracks" }
params = { method = "kde", bandwidth = "lscv", which_min = "local" }

[[nodes]]
id = "given"
op = "move.home_range"
inputs = { tracks = "local.tracks", like = "grid.raster" }
params = { method = "kde", bandwidth = [30, 45], levels = 0.9 }

[[nodes]]
id = "hulls"
op = "move.home_range"
inputs = { tracks = "local.tracks" }
params = { method = "locoh", type = "a", n = 900, levels = 0.5, rand_buffer = 0.001 }

[[nodes]]
id = "grid"
op = "raster.read"
params = { path = "grid.tif" }

[[nodes]]
id = "areas"
op = "move.home_range_overlap"
inputs = { polygons = "box.polygons" }
params = { which = "all" }

[[nodes]]
id = "shared"
op = "move.home_range_overlap"
inputs = { ud = "kernel.ud" }
params = { type = "ba", conditional = true }

[[nodes]]
id = "back"
op = "move.intensity_use"
inputs = { tracks = "local.tracks" }
params = { track = "id" }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(t.contains("box = tf.move.hr_mcp(local, levels=[0.5, 0.95])\n"), "{t}");
        assert!(
            t.contains("kernel_trast = tf.move.make_trast(local, res=50)\nkernel = tf.move.hr_kde(local, h=tf.move.hr_kde_pi(local, rescale=\"unitvar\"), trast=kernel_trast, levels=[0.5, 0.9])\n"),
            "{t}"
        );
        assert!(t.contains("scaled_trast = tf.move.make_trast(local, factor=2, track=\"id\")\n"), "{t}");
        assert!(
            t.contains(
                "scaled = tf.move.hr_kde(local, h=tf.move.hr_kde_ref_scaled(local, trast=scaled_trast, levels=0.9, num_parts=2, track=\"id\")[\"h\"].to_dict(), trast=scaled_trast, levels=0.9, track=\"id\")\n"
            ),
            "{t}"
        );
        assert!(t.contains("cv = tf.move.hr_kde(local, h=tf.move.hr_kde_lscv(local, which_min=\"local\").h)\n"), "{t}");
        assert!(t.contains("given = tf.move.hr_kde(local, h=[30, 45], trast=grid, levels=0.9)\n"), "{t}");
        assert!(
            t.contains("hulls = tf.move.hr_locoh(local, n=900, type=\"a\", levels=0.5, rand_buffer=0.001)\n"),
            "{t}"
        );
        assert!(t.contains("areas = tf.move.hr_overlap(box, which=\"all\")\n"), "{t}");
        assert!(t.contains("shared = tf.move.hr_overlap(kernel, type=\"ba\", conditional=True)\n"), "{t}");
        assert!(t.contains("back = tf.move.intensity_use(local, track=\"id\").reset_index()\n"), "{t}");
        parses(t);
    }

    /// The dynamic Brownian bridges: the library's variance and distribution of either model, the masks, the volume,
    /// the outer probability and the window suggestion.
    #[test]
    fn bridges_as_the_library() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "local"
op = "move.aeqd_crs"
inputs = { tracks = "tracks.tracks" }

[[nodes]]
id = "window"
op = "move.suggest_window"
inputs = { tracks = "local.tracks" }
params = { target_hours = 6 }

[[nodes]]
id = "var"
op = "move.dbbmm_variance"
inputs = { tracks = "local.tracks" }
params = { location_error = 25, window_size = 21, margin = 5, location_error_na = "approx", burst = "none" }

[[nodes]]
id = "plain"
op = "move.dbbmm_variance"
inputs = { tracks = "local.tracks" }

[[nodes]]
id = "gvar"
op = "move.dbgb_variance"
inputs = { tracks = "local.tracks" }
params = { location_error = "auto", margin = 7 }

[[nodes]]
id = "gaps"
op = "move.mask_segments"
inputs = { tracks = "var.tracks" }
params = { segments = [3, 4, 5], model = "dbbmm" }

[[nodes]]
id = "flags"
op = "move.mask_segments"
inputs = { tracks = "var.tracks" }
params = { column = "bad" }

[[nodes]]
id = "grid"
op = "raster.read"
params = { path = "grid.tif" }

[[nodes]]
id = "ud"
op = "move.dbbmm_ud"
inputs = { tracks = "gaps.tracks", like = "grid.raster" }
params = { time_step = 2, crs = "projected" }

[[nodes]]
id = "gud"
op = "move.dbgb_ud"
inputs = { tracks = "local.tracks" }
params = { location_error = 25, dim_size = 80, ext_factor = 0.3, verbose = true }

[[nodes]]
id = "vol"
op = "move.ud_volume"
inputs = { ud = "ud.ud" }
params = { levels = [0.5, 0.95] }

[[nodes]]
id = "outer"
op = "move.ud_outer_probability"
inputs = { points = "local.tracks", ud = "ud.ud" }
params = { band = 1 }

[[nodes]]
id = "outer_name"
op = "move.ud_outer_probability"
inputs = { points = "local.tracks", ud = "gud.ud" }
params = { band = "M1" }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(t.contains("window = tf.move.suggest_dbbmm_window(local, target_hours=6)\n"), "{t}");
        assert!(
            t.contains("var = tf.move.dbbmm_variance(local, location_error=25, window_size=21, margin=5, location_error_na=\"approx\", burst=None)\n"),
            "{t}"
        );
        assert!(t.contains("plain = tf.move.dbbmm_variance(local, location_error=None)\n"), "{t}");
        assert!(t.contains("gvar = tf.move.dbgb_variance(local, location_error=\"auto\", margin=7)\n"), "{t}");
        assert!(t.contains("gaps = tf.move.mask_segments(var, segments=[3, 4, 5], model=\"dbbmm\")\n"), "{t}");
        assert!(
            t.contains("flags = tf.move.mask_segments(var, segments=var[\"bad\"].fillna(0).to_numpy(bool))\n"),
            "{t}"
        );
        assert!(t.contains("ud = tf.move.dbbmm_ud(gaps, like=grid, time_step=2, crs=\"projected\")\n"), "{t}");
        assert!(
            t.contains("gud = tf.move.dbgb_ud(local, dim_size=80, ext_factor=0.3, location_error=25, verbose=True)\n"),
            "{t}"
        );
        assert!(t.contains("# (not passed: levels = [0.5, 0.95])\nvol = tf.move.ud_volume(ud)\n"), "{t}");
        assert!(
            t.contains(
                "outer = local.copy()\nouter[\"outer_probability\"] = tf.move.ud_outer_probability(local, ud, band=0)\n"
            ),
            "{t}"
        );
        assert!(t.contains("tf.move.ud_outer_probability(local, gud, band=\"M1\")\n"), "{t}");
        parses(t);
    }

    /// The values along the steps: where = along is the library's along_summary with the stats, a reference to a
    /// setting chooses at run time, and the `along` output is extract_covariates_along as a long table.
    #[test]
    fn values_along_the_steps_as_the_library() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[inputs.w]
kind = "value"
value = "along"

[inputs.s]
kind = "value"
value = "mean, delta"

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "steps"
op = "move.steps"
inputs = { tracks = "tracks.tracks" }

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "dem.tif" }

[[nodes]]
id = "slope"
op = "raster.read"
params = { path = "slope.tif" }

[[nodes]]
id = "all"
op = "move.extract_covariates"
inputs = { tracks = "steps.steps", dem = "dem.raster" }
params = { where = "along" }

[[nodes]]
id = "text"
op = "move.extract_covariates"
inputs = { tracks = "steps.steps", dem = "dem.raster", slope = "slope.raster" }
params = { where = "along", stats = "ascent, descent" }

[[nodes]]
id = "listed"
op = "move.extract_covariates"
inputs = { tracks = "steps.steps", dem = "dem.raster" }
params = { where = "along", stats = ["mean", "max"] }

[[nodes]]
id = "later"
op = "move.extract_covariates"
inputs = { tracks = "steps.steps", dem = "dem.raster" }
params = { where = "$w", stats = "$s" }

[[nodes]]
id = "end"
op = "move.extract_covariates"
inputs = { tracks = "steps.steps", dem = "dem.raster" }

[[nodes]]
id = "fit"
op = "move.fit_distribution"
inputs = { values = "text.along" }
params = { name = "gamma", column = "dem" }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(t.contains("all_ = tf.move.along_summary(steps, {\"dem\": dem})\n"), "{t}");
        assert!(
            t.contains("text = tf.move.along_summary(steps, {\"dem\": dem, \"slope\": slope}, stats=[\"ascent\", \"descent\"])\n"),
            "{t}"
        );
        assert!(
            t.contains("listed = tf.move.along_summary(steps, {\"dem\": dem}, stats=[\"mean\", \"max\"])\n"),
            "{t}"
        );
        assert!(t.contains("later = tf.move.along_summary(steps, {\"dem\": dem}, stats="), "{t}");
        assert!(
            t.contains(
                "stats=[p.strip() for p in s.split(\",\")] if isinstance(s, str) else s) if w == \"along\" else \
                 tf.move.extract_covariates(steps, {\"dem\": dem}, where=w)\n"
            ),
            "{t}"
        );
        assert!(t.contains("end = tf.move.extract_covariates(steps, {\"dem\": dem})\n"), "{t}");
        // the values: extract_covariates_along, a row per cell with the step first
        assert!(t.contains("tf.move.extract_covariates_along(steps, {\"dem\": dem, \"slope\": slope})"), "{t}");
        assert!(t.contains(".assign(step=i + 1)"), "{t}");
        parses(t);
    }

    /// The length of the steps on the relief: the library's surface_distance with the DEM, its spacing and name.
    #[test]
    fn steps_on_the_relief() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "steps"
op = "move.steps"
inputs = { tracks = "tracks.tracks" }

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "dem.tif" }

[[nodes]]
id = "relief"
op = "move.surface_distance"
inputs = { steps = "steps.steps", dem = "dem.raster" }
params = { spacing = 5, name = "len3d" }

[[nodes]]
id = "plain"
op = "move.surface_distance"
inputs = { steps = "steps.steps", dem = "dem.raster" }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(t.contains("relief = tf.move.surface_distance(steps, dem=dem, spacing=5, name=\"len3d\")\n"), "{t}");
        assert!(t.contains("plain = tf.move.surface_distance(steps, dem=dem)\n"), "{t}");
        parses(t);
    }

    /// A step selection function and its log-RSS: the stratum made of the track and the step, the formula as given or
    /// made of the covariates, the interval's arguments.
    #[test]
    fn a_step_selection_function_and_its_log_rss() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "steps"
op = "move.steps"
inputs = { tracks = "tracks.tracks" }

[[nodes]]
id = "random"
op = "move.random_steps"
inputs = { steps = "steps.steps" }

[[nodes]]
id = "plain"
op = "move.fit_issf"
inputs = { steps = "random.steps" }

[[nodes]]
id = "given"
op = "move.fit_issf"
inputs = { steps = "random.steps" }
params = { formula = "case_ ~ elev + log(sl_)" }

[[nodes]]
id = "by_id"
op = "move.fit_issf"
inputs = { steps = "random.steps" }
params = { formula = "case_ ~ elev + strata(step_id_)", track = "id" }

[[nodes]]
id = "column"
op = "move.fit_issf"
inputs = { steps = "random.steps" }
params = { strata = "burst_" }

[[nodes]]
id = "rss"
op = "move.log_rss"
inputs = { model = "plain.model" }
params = { x1 = { elev = [100, 200], sl_ = [10, 20] }, x2 = { elev = 100, sl_ = 10 }, ci = "boot", level = 0.9, n_boot = 50, seed = 3 }

[[nodes]]
id = "rss_none"
op = "move.log_rss"
inputs = { model = "plain.model", x1 = "tab.table", x2 = "tab.table" }
params = { ci = "none" }

[[nodes]]
id = "tab"
op = "table.read"
params = { path = "x.csv" }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        // the stratum of each step of each track, then the formula of the covariates
        assert!(
            t.contains(
                "plain_data = random.assign(stratum_=random.groupby([next(c for c in (\"individual-tag-local-identifier\""
            ),
            "{t}"
        );
        assert!(t.contains("\"step_id_\"], sort=False).ngroup() + 1)\n"), "{t}");
        assert!(t.contains("import re as _re\nplain_terms = [c if _re.fullmatch("), "{t}");
        assert!(
            t.contains("plain = tf.move.fit_issf(plain_data, \"case_ ~ \" + \" + \".join([*plain_terms, \"log(sl_)\", \"cos(ta_)\", \"strata(\" + \"stratum_\" + \")\"]))\n"),
            "{t}"
        );
        assert!(
            t.contains("given = tf.move.fit_issf(given_data, \"case_ ~ elev + log(sl_)\", strata=\"stratum_\")\n"),
            "{t}"
        );
        // a formula with its own strata is passed as it is, the track named in the node
        assert!(t.contains("by_id = tf.move.fit_issf(random, \"case_ ~ elev + strata(step_id_)\")\n"), "{t}");
        assert!(t.contains("\"strata(\" + \"burst_\" + \")\"]))\n"), "{t}");
        assert!(
            t.contains("rss = tf.move.log_rss(plain, x1=_pd.DataFrame({\"elev\": [100, 200], \"sl_\": [10, 20]}), x2=_pd.DataFrame({\"elev\": 100, \"sl_\": 10}, index=[0]), ci=\"boot\", ci_level=0.9, n_boot=50, seed=3)\n"),
            "{t}"
        );
        assert!(t.contains("rss_none = tf.move.log_rss(plain, x1=tab, x2=tab, seed=1)\n"), "{t}");
        parses(t);
    }

    /// A simulation from a model of coefficients: the library's make_issf_model, redistribution_kernel and
    /// simulate_path in a loop over the starts and the paths (the seeds one after the other), resistance and density.
    #[test]
    fn simulated_paths_their_resistance_and_their_density() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "sl"
op = "move.distribution"
params = { name = "gamma", shape = 2, scale = 50 }

[[nodes]]
id = "ta"
op = "move.distribution"
params = { name = "vonmises", kappa = 2 }

[[nodes]]
id = "model"
op = "move.issf_model"
inputs = { sl_distr = "sl.distribution", ta_distr = "ta.distribution" }
params = { coefficients = { forest_end = 1.5, sl_ = -0.002 } }

[[nodes]]
id = "ordered"
op = "move.issf_model"
inputs = { sl_distr = "sl.distribution", ta_distr = "ta.distribution" }
params = { coefficients = [["sl_", -0.002], ["forest_end", 1.5]] }

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "forest.tif" }

[[nodes]]
id = "tracks"
op = "move.read_tracks"
params = { path = "fishers.csv" }

[[nodes]]
id = "paths"
op = "move.simulate_path"
inputs = { model = "model.model", forest = "forest.map" }
params = { x = 500, y = 500, direction = 0.5, time = "2020-01-01 00:00:00", dt = 1800, n_steps = 20, n_control = 1000, n_paths = 3, seed = 42, tolerance_outside = 1 }

[[nodes]]
id = "from_tracks"
op = "move.simulate_path"
inputs = { model = "model.model", forest = "forest.map", start = "tracks.tracks", sl_distr = "sl.distribution" }
params = { n_steps = 5 }

[[nodes]]
id = "plain"
op = "move.simulate_path"
inputs = { model = "model.model", forest = "forest.map" }
params = { x = 1, y = 2 }

[[nodes]]
id = "res"
op = "move.resistance"
inputs = { model = "model.model", forest = "forest.map" }
params = { transform = "linear", r_max = 50 }

[[nodes]]
id = "res_plain"
op = "move.resistance"
inputs = { model = "model.model", forest = "forest.map" }

[[nodes]]
id = "use"
op = "move.path_density"
inputs = { paths = "paths.paths", like = "forest.map" }
params = { by = "track", include_start = true }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("model = tf.move.make_issf_model({\"forest_end\": 1.5, \"sl_\": -0.002}, sl=sl, ta=ta)\n"),
            "{t}"
        );
        assert!(
            t.contains(
                "ordered = tf.move.make_issf_model(dict([[\"sl_\", -0.002], [\"forest_end\", 1.5]]), sl=sl, ta=ta)\n"
            ),
            "{t}"
        );
        // a start from the settings, a path for each seed (seed, seed + 1...), the tracks named path_1...
        assert!(
            t.contains(
                "paths_starts = [(None, tf.move.make_start((500, 500), 0.5, time=\"2020-01-01 00:00:00\", dt=1800))]\n"
            ),
            "{t}"
        );
        assert!(t.contains("for paths_label, paths_start in paths_starts:\n    for paths_i in range(3):\n"), "{t}");
        assert!(
            t.contains("paths_k = tf.move.redistribution_kernel(model, paths_start, {\"forest\": forest}, n_control=1000, tolerance_outside=1, seed=42 + paths_i)\n"),
            "{t}"
        );
        assert!(t.contains("paths_all.append(tf.move.simulate_path(paths_k, 20).assign(track=(str(paths_label) + \"_path_\" if paths_label is not None else \"path_\") + str(paths_i + 1)))\n"), "{t}");
        assert!(t.contains("paths = pd.concat(paths_all, ignore_index=True)\n"), "{t}");
        // a start from tracks: the first location of each, its time and the direction of its first step
        assert!(t.contains("from_tracks_starts = []\nfor _, g in (tracks.groupby(from_tracks_track, sort=False) if from_tracks_track else [(None, tracks)]):\n"), "{t}");
        assert!(t.contains("float(np.arctan2(b.y - a.y, b.x - a.x)) if b is not None else 0.0, time=(g[from_tracks_time].iloc[0] if from_tracks_time else None)"), "{t}");
        assert!(t.contains("sl_distr=sl, n_control") || t.contains("sl_distr=sl"), "{t}");
        assert!(t.contains("plain_starts = [(None, tf.move.make_start((1, 2), 0.0))]\n"), "{t}");
        assert!(!t.contains("__import__"), "{t}");
        assert!(
            t.contains("res = tf.move.resistance(model, {\"forest\": forest}, transform=\"linear\", r_max=50)\n"),
            "{t}"
        );
        assert!(t.contains("res_plain = tf.move.resistance(model, {\"forest\": forest})\n"), "{t}");
        assert!(
            t.contains("use = tf.move.path_density(paths, like=forest, by=\"track\", include_start=True)\n"),
            "{t}"
        );
        assert!(!t.contains("__import__"), "{t}");
        parses(t);
    }
}
