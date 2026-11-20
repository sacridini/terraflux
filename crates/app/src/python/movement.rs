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
            let mut args = vec![tracks, format!("{{{}}}", layers.join(", "))];
            if let Some(w) = opt(n, "where", sc)? {
                args.push(format!("where={w}"));
            }
            let code = format!("{var} = {}({})\n", py.call, args.join(", "));
            (code, out(&[("tracks", var.to_string()), ("table", frame_table(var))]), Need::Nothing)
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
}
