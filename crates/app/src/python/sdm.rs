//! The Python counterparts of the operators of species distribution models (sdm).
//!
//! Not translated: `sdm.maxnet` (the library's `MaxNet` is an estimator, not the `SDMResult` that `tf.sdm.predict`
//! takes, which is what `sdm.maxnet_predict` calls), and `ml.fit`, `ml.predict`, `ml.importance` and
//! `ml.partial_dependence` (scikit-learn's own calls on a table: no library function does the method).

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[
    Py {
        op: "sdm.accessible_area",
        call: "tf.sdm.accessible_area",
        args: &[
            ("occurrences", ""),
            ("regions", "regions"),
            ("within", "within"),
            ("method", "method"),
            ("distance", "distance"),
            ("resolution", "resolution"),
            ("x", "x"),
            ("y", "y"),
        ],
        defaults: &[],
        outs: &[("area", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.autocorrelation_range",
        call: "tf.sdm.autocorrelation_range",
        args: &[("env", ""), ("n", "n"), ("seed", "seed"), ("deg_to_metre", "deg_to_metre")],
        defaults: &[("seed", "1")],
        outs: &[("range", "[\"range\"]"), ("block_size", "[\"block_size\"]"), ("layers", "[\"layers\"]")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.bias_surface",
        call: "tf.sdm.bias_surface",
        args: &[
            ("occurrences", ""),
            ("env", "env"),
            ("bandwidth", "bandwidth"),
            ("isotropic", "isotropic"),
            ("limits", "limits"),
            ("weights", "weights"),
            ("rescale", "rescale"),
            ("mask", "mask"),
            ("x", "x"),
            ("y", "y"),
        ],
        defaults: &[],
        outs: &[("bias", ""), ("bandwidth", ".attrs[\"bandwidth\"]")],
        kind: Kind::Plain,
    },
    // (the folds' `presence`, `x` and `y` columns: the library takes the records of sdm.dataset / sdm.prepare, whose
    // presences and coordinates it knows)
    Py {
        op: "sdm.buffer_folds",
        call: "tf.sdm.buffer_folds",
        args: &[
            ("records", ""),
            ("size", "size"),
            ("presence_background", "presence_background"),
            ("add_background", "add_background"),
        ],
        defaults: &[],
        outs: &[("folds", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.checkerboard_folds",
        call: "tf.sdm.checkerboard_folds",
        args: &[("records", ""), ("env", "env"), ("aggregation", "aggregation")],
        defaults: &[],
        outs: &[("records", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.dataset",
        call: "tf.sdm.dataset",
        args: &[("presences", ""), ("background", "background"), ("env", "env"), ("x", "x"), ("y", "y")],
        defaults: &[("background", "None")],
        outs: &[("records", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.download_gbif",
        call: "tf.sdm.download_gbif",
        args: &[
            ("species", ""),
            ("country", "country"),
            ("area", "area"),
            ("year", "year"),
            ("basis_of_record", "basis_of_record"),
            ("max_uncertainty", "max_uncertainty"),
            ("limit", "limit"),
            ("has_geospatial_issue", "has_geospatial_issue"),
            ("workers", "n_workers"),
        ],
        defaults: &[],
        outs: &[("occurrences", ""), ("query", ".attrs[\"gbif\"]")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.jackknife_folds",
        call: "tf.sdm.jackknife_folds",
        args: &[("records", "")],
        defaults: &[],
        outs: &[("records", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.migrate",
        call: "tf.sdm.migrate",
        args: &[
            ("initial", ""),
            ("suitability", "suitability"),
            ("barriers", "barriers"),
            ("steps", "steps"),
            ("kernel", "kernel"),
            ("ini_maturity", "ini_maturity"),
            ("full_maturity", "full_maturity"),
            ("propagules", "propagules"),
            ("threshold", "threshold"),
            ("ldd_frequency", "ldd_frequency"),
            ("ldd_min", "ldd_min"),
            ("ldd_max", "ldd_max"),
            ("barrier_type", "barrier_type"),
            ("seed", "seed"),
        ],
        defaults: &[],
        outs: &[
            ("map", ".map"),
            ("occupied", ".occupied"),
            ("steps", ".steps.rename(step=\"band\")"),
            ("table", ".table"),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.one_per_cell",
        call: "tf.sdm.one_per_cell",
        args: &[("occurrences", ""), ("raster", "raster"), ("drop_missing", "drop_missing"), ("x", "x"), ("y", "y")],
        defaults: &[],
        outs: &[("occurrences", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.quadrant_folds",
        call: "tf.sdm.quadrant_folds",
        args: &[("records", ""), ("orientation", "orientation")],
        defaults: &[],
        outs: &[("records", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.random_folds",
        call: "tf.sdm.random_folds",
        args: &[("records", ""), ("k", "k"), ("seed", "seed")],
        defaults: &[("seed", "1")],
        outs: &[("records", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.select_predictors",
        call: "tf.sdm.select_predictors",
        args: &[
            ("env", ""),
            ("area", "area"),
            ("vif", "vif"),
            ("cor", "cor"),
            ("keep", "keep"),
            ("method", "method"),
            ("size", "size"),
            ("seed", "seed"),
        ],
        defaults: &[("seed", "1")],
        outs: &[("env", ""), ("selection", ".attrs[\"selection\"]")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.spatial_blocks",
        call: "tf.sdm.spatial_blocks",
        args: &[
            ("records", ""),
            ("size", "size"),
            ("rows_cols", "rows_cols"),
            ("k", "k"),
            ("hexagon", "hexagon"),
            ("flat_top", "flat_top"),
            ("selection", "selection"),
            ("iteration", "iteration"),
            ("offset", "offset"),
            ("extend", "extend"),
            ("deg_to_metre", "deg_to_metre"),
            ("seed", "seed"),
        ],
        defaults: &[("seed", "1")],
        outs: &[("records", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.spatial_clusters",
        call: "tf.sdm.spatial_clusters",
        args: &[
            ("records", ""),
            ("k", "k"),
            ("env", "env"),
            ("scale", "scale"),
            ("algorithm", "algorithm"),
            ("seed", "seed"),
        ],
        defaults: &[("seed", "1")],
        outs: &[("records", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "sdm.thin",
        call: "tf.sdm.thin",
        args: &[
            ("occurrences", ""),
            ("distance", "distance"),
            ("reps", "reps"),
            ("seed", "seed"),
            ("x", "x"),
            ("y", "y"),
        ],
        defaults: &[("seed", "1")],
        outs: &[("occurrences", ""), ("kept", ".attrs[\"thin\"][\"kept\"]")],
        kind: Kind::Plain,
    },
];

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    match n.op.as_str() {
        "sdm.clean_occurrences" => clean(n, var, sc).map(Some),
        "sdm.area_of_habitat" => habitat(n, var, sc).map(Some),
        "sdm.evaluate" => evaluate(n, var, sc).map(Some),
        "sdm.boyce" => boyce(n, var, sc).map(Some),
        "sdm.threshold" => threshold(n, var, sc).map(Some),
        "sdm.omission_rate" => omission(n, var, sc).map(Some),
        "ml.potential" => potential(n, var, sc).map(Some),
        _ => Ok(None),
    }
}

/// An input that must be connected.
fn need(n: &Node, k: &str, sc: &Scope) -> Result<String, String> {
    value_of(n, k, sc)?.ok_or_else(|| format!("its {k} is not connected"))
}

/// The records' cleaning: CoordinateCleaner's tests as the library's arguments; a reference given by an input goes
/// into the test's options (`ref`).
fn clean(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let mut args = vec![need(n, "occurrences", sc)?];
    args.extend(given(n, &[("species", "species"), ("action", "action"), ("x", "x"), ("y", "y")], sc)?);
    // (the test, its main option, whether it runs without a setting)
    const WITH_REF: [(&str, &str, bool); 6] = [
        ("capitals", "buffer", true),
        ("centroids", "buffer", true),
        ("institutions", "buffer", true),
        ("seas", "scale", true),
        ("urban", "", false),
        ("countries", "", false),
    ];
    let mut code = String::new();
    let mut handled: Vec<&str> = Vec::new();
    for (test, main, on) in WITH_REF {
        let Some(r) = value_of(n, &format!("{test}_ref"), sc)? else { continue };
        let own = n.params.get(test);
        if own.is_some_and(|v| terraflux_graph::setting_ref(v).is_some()) {
            return Err(format!("its {test}_ref with {test} set by a reference: not in the script yet"));
        }
        let opts = match own {
            None if on => Some(format!("{{\"ref\": {r}}}")),
            None => None,
            Some(Value::Bool(true)) => Some(format!("{{\"ref\": {r}}}")),
            Some(Value::Bool(false) | Value::Null) => None,
            Some(Value::Number(v)) if !main.is_empty() => Some(format!("{{{}: {v}, \"ref\": {r}}}", text(main))),
            Some(Value::String(s)) if test == "countries" => Some(format!("{{\"column\": {}, \"ref\": {r}}}", text(s))),
            Some(Value::Object(o)) => {
                let items: Vec<String> = o.iter().map(|(k, v)| format!("{}: {}", text(k), literal(v))).collect();
                Some(format!("{{{}, \"ref\": {r}}}", items.join(", ")))
            }
            Some(v) => return Err(format!("its {test} = {}: not in the script with {test}_ref", literal(v))),
        };
        handled.push(test);
        match opts {
            Some(o) => args.push(format!("{test}={o}")),
            None => {
                code.push_str(&format!("# (not passed: the input {test}_ref; the test {test} does not run)\n"));
                if let Some(v) = own {
                    args.push(format!("{test}={}", literal(v)));
                }
            }
        }
    }
    for t in [
        "equal",
        "zeros",
        "capitals",
        "centroids",
        "seas",
        "urban",
        "countries",
        "outliers",
        "gbif",
        "institutions",
        "duplicates",
        "aohi",
    ] {
        if !handled.contains(&t) {
            args.extend(given(n, &[(t, t)], sc)?);
        }
    }
    code.push_str("import pandas as pd\n");
    code.push_str(&format!("{var} = tf.sdm.clean_occurrences({})\n", args.join(", ")));
    code.push_str(&format!("{var}_counts = pd.DataFrame([{var}.attrs[\"clean\"]])\n"));
    let outs =
        HashMap::from([("occurrences".to_string(), var.to_string()), ("counts".to_string(), format!("{var}_counts"))]);
    Ok((code, outs, Need::Nothing))
}

/// The area of habitat: the original vegetation and the elevation as they are; the restored shares and the
/// anthropic land of Terraflux's gain have no argument in the library.
fn habitat(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    for k in ["restore", "anthropic"] {
        if n.inputs.contains_key(k) {
            return Err(format!("its input {k}: not in the script yet"));
        }
    }
    let ranges = need(n, "ranges", sc)?;
    let cover = need(n, "land_cover", sc)?;
    let natural = setting(n, "natural", sc)?.ok_or("its natural classes are not given")?;
    let mut args = vec![ranges, cover.clone(), natural.clone()];
    args.extend(given(
        n,
        &[
            ("suitability", "suitability"),
            ("include_unlisted", "include_unlisted"),
            ("elevation_limits", "elevation_limits"),
            ("cell_area", "cell_area"),
            ("loss", "loss"),
        ],
        sc,
    )?);
    let original = value_of(n, "original", sc)?;
    // (where the original has no value Terraflux takes the current cover, class by class)
    if let Some(o) = &original {
        args.push(format!("original={o}.sel(band={natural}).fillna({cover}.sel(band={natural}))"));
    }
    if let Some(e) = value_of(n, "elevation", sc)? {
        args.push(format!("elevation={e}"));
    }
    let mut code = format!("{var} = tf.sdm.area_of_habitat({})\n", args.join(", "));
    let mut outs = HashMap::from([("table".to_string(), format!("{var}.table"))]);
    if original.is_some() {
        code.push_str("import xarray as xr\n");
        code.push_str(&format!(
            "{var}_gain = xr.concat([{var}.gain_map(s) for s in {var}.table[\"species\"]], dim=\"band\")\n"
        ));
        code.push_str(&format!(
            "{var}_gain = {var}_gain.assign_coords(band={var}.table[\"species\"].astype(str).tolist())\n"
        ));
        outs.insert("gain".to_string(), format!("{var}_gain"));
    }
    Ok((code, outs, Need::Nothing))
}

/// The predictions of an input (a table's column, the first by default; a map's or an array's cells as they are),
/// as a line of code and the name it gives.
fn predictions(n: &Node, var: &str, port: &str, sc: &Scope) -> Result<(String, String), String> {
    let p = need(n, port, sc)?;
    let name = format!("{var}_{port}");
    let col = setting(n, "column", sc)?.unwrap_or_else(|| format!("{p}.columns[0]"));
    Ok((format!("{name} = {p}[{col}] if hasattr({p}, \"columns\") else {p}\n"), name))
}

/// A Boyce index's classes as Terraflux's table (the windows kept in `used` as 1).
fn boyce_table(b: &str, name: &str) -> String {
    format!(
        "{name} = pd.DataFrame({{\"f_ratio\": {b}[\"f_ratio\"], \"hs\": {b}[\"hs\"], \"used\": \
         np.isin(np.arange(len({b}[\"hs\"])), {b}[\"used\"]).astype(float)}})\n"
    )
}

fn evaluate(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let (c1, p) = predictions(n, var, "presences", sc)?;
    let (c2, a) = predictions(n, var, "background", sc)?;
    let mut args = vec![p, a];
    args.extend(given(n, &[("thresholds", "thresholds")], sc)?);
    let mut code = format!("import pandas as pd\n{c1}{c2}");
    code.push_str(&format!("{var} = tf.sdm.evaluate({})\n", args.join(", ")));
    code.push_str(&format!("{var}_stats = pd.DataFrame([{var}.stats])\n"));
    code.push_str(&format!("{var}_thresholds = pd.DataFrame([{var}.thresholds])\n"));
    code.push_str(&boyce_table(&format!("{var}.boyce"), &format!("{var}_boyce")));
    let outs = HashMap::from([
        ("stats".to_string(), format!("{var}_stats")),
        ("table".to_string(), format!("{var}.table")),
        ("thresholds".to_string(), format!("{var}_thresholds")),
        ("boyce".to_string(), format!("{var}_boyce")),
    ]);
    Ok((code, outs, Need::Nothing))
}

fn boyce(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let (c1, p) = predictions(n, var, "presences", sc)?;
    let (c2, a) = predictions(n, var, "available", sc)?;
    let mut args = vec![p, a];
    args.extend(given(
        n,
        &[
            ("nclass", "nclass"),
            ("window", "window"),
            ("res", "res"),
            ("method", "method"),
            ("rm_duplicate", "rm_duplicate"),
        ],
        sc,
    )?);
    let mut code = format!("import pandas as pd\n{c1}{c2}");
    code.push_str(&format!("{var} = tf.sdm.boyce({})\n", args.join(", ")));
    code.push_str(&boyce_table(var, &format!("{var}_table")));
    let outs =
        HashMap::from([("cor".to_string(), format!("{var}[\"cor\"]")), ("table".to_string(), format!("{var}_table"))]);
    Ok((code, outs, Need::Nothing))
}

fn threshold(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let (c1, p) = predictions(n, var, "presences", sc)?;
    let (c2, a) = predictions(n, var, "background", sc)?;
    let mut args = vec![p, a];
    args.extend(given(n, &[("method", "method"), ("sensitivity", "sensitivity")], sc)?);
    let code = format!("{c1}{c2}{var} = tf.sdm.threshold({})\n", args.join(", "));
    Ok((code, HashMap::from([("threshold".to_string(), var.to_string())]), Need::Nothing))
}

fn omission(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let (c1, p) = predictions(n, var, "train", sc)?;
    let (c2, a) = predictions(n, var, "test", sc)?;
    let mut args = vec![p, a];
    args.extend(given(n, &[("threshold", "threshold")], sc)?);
    let code = format!("{c1}{c2}{var} = tf.sdm.omission_rate({})\n", args.join(", "));
    Ok((code, HashMap::from([("rate".to_string(), var.to_string())]), Need::Nothing))
}

/// The potential of each class: the library's `Estimator` with scikit-learn's forest or boosting, fitted to the maps
/// and its probabilities on the drivers.
fn potential(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let landscape = need(n, "landscape", sc)?;
    let drivers = need(n, "drivers", sc)?;
    let later = value_of(n, "later", sc)?;
    let cells = value_of(n, "cells", sc)?;
    let boosting = match choice(n, "method").unwrap_or("random_forest") {
        "random_forest" => false,
        "gradient_boosting" => true,
        other => return Err(format!("the method {other:?}: not in the script yet")),
    };
    let mut code = String::new();
    let mut a;
    let importance = if boosting {
        code.push_str(
            "# (importance: the share of the gain of the splits of Terraflux's boosting is not in scikit-learn)\n",
        );
        "None".to_string()
    } else {
        format!("{var}.importance(\"impurity\").T.rename_axis(\"class\").reset_index()")
    };
    let model = if boosting {
        code.push_str("from sklearn.ensemble import HistGradientBoostingClassifier\n");
        a = given(
            n,
            &[
                ("max_depth", "max_depth"),
                ("min_samples_leaf", "min_samples_leaf"),
                ("max_features", "max_features"),
                ("learning_rate", "learning_rate"),
                ("max_iter", "max_iter"),
                ("max_leaf_nodes", "max_leaf_nodes"),
                ("l2_regularization", "l2_regularization"),
                ("max_bins", "max_bins"),
            ],
            sc,
        )?;
        // (Terraflux's boosting never stops early)
        a.push("early_stopping=False".into());
        "HistGradientBoostingClassifier"
    } else {
        code.push_str("from sklearn.ensemble import RandomForestClassifier\n");
        a = given(
            n,
            &[
                ("n_estimators", "n_estimators"),
                ("max_depth", "max_depth"),
                ("min_samples_split", "min_samples_split"),
                ("min_samples_leaf", "min_samples_leaf"),
                ("bootstrap", "bootstrap"),
            ],
            sc,
        )?;
        match choice(n, "max_features") {
            Some("all") => a.push("max_features=None".into()),
            _ => a.extend(given(n, &[("max_features", "max_features")], sc)?),
        }
        "RandomForestClassifier"
    };
    a.push(format!("random_state={}", setting(n, "random_state", sc)?.unwrap_or_else(|| "0".into())));
    let target = match setting(n, "target", sc)? {
        Some(t) => t,
        None => text(if later.is_some() { "expansion" } else { "presence" }),
    };
    let mut est = vec![format!("{model}({})", a.join(", ")), format!("target={target}")];
    est.extend(given(
        n,
        &[("classes", "classes"), ("sample", "sample"), ("stratify", "stratify"), ("balance", "balance")],
        sc,
    )?);
    est.push(format!("seed={}", setting(n, "seed", sc)?.unwrap_or_else(|| "1".into())));
    let mut fit = vec![landscape, drivers.clone()];
    if let Some(l) = later {
        fit.push(format!("t1={l}"));
    }
    if let Some(c) = cells {
        // (Terraflux learns from the cells that are not 0)
        fit.push(format!("cells=({c}.fillna(0) != 0)"));
    }
    code.push_str(&format!("{var} = tf.simulate.Estimator({}).fit({})\n", est.join(", "), fit.join(", ")));
    code.push_str(&format!("{var}_potential = {var}.predict({drivers}).rename({{\"class\": \"band\"}})\n"));
    let outs = HashMap::from([
        ("potential".to_string(), format!("{var}_potential")),
        ("importance".to_string(), importance),
        ("model".to_string(), var.to_string()),
    ]);
    Ok((code, outs, Need::Nothing))
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

    #[test]
    fn the_species_distribution_workflow() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "b"
op = "nlm.random"

[[nodes]]
id = "env"
op = "stack.of"
inputs = { temp = "a.map", rain = "b.map" }

[[nodes]]
id = "occ"
op = "sdm.background"
inputs = { env = "env.stack" }
params = { n = 50 }

[[nodes]]
id = "clean"
op = "sdm.clean_occurrences"
inputs = { occurrences = "occ.points" }
params = { seas = false, outliers = false, capitals = 5000, action = "drop" }

[[nodes]]
id = "thin"
op = "sdm.thin"
inputs = { occurrences = "clean.occurrences" }
params = { distance = 10000 }

[[nodes]]
id = "data"
op = "sdm.dataset"
inputs = { presences = "thin.occurrences", env = "env.stack" }

[[nodes]]
id = "folds"
op = "sdm.spatial_blocks"
inputs = { records = "data.records" }
params = { size = 50000, k = 4 }

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "folds.records" }

[[nodes]]
id = "ev"
op = "sdm.evaluate"
inputs = { presences = "fit.predictions", background = "fit.predictions" }
params = { column = "maxnet" }

[[nodes]]
id = "boyce"
op = "sdm.boyce"
inputs = { presences = "fit.predictions", available = "fit.predictions" }

[[nodes]]
id = "thr"
op = "sdm.threshold"
inputs = { presences = "fit.predictions", background = "fit.predictions" }
params = { method = "max_kappa" }

[[nodes]]
id = "om"
op = "sdm.omission_rate"
inputs = { train = "fit.predictions", test = "fit.predictions" }
params = { threshold = "mtp" }
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains(
                "clean = tf.sdm.clean_occurrences(occ, action=\"drop\", capitals=5000, seas=False, outliers=False)"
            ),
            "{t}"
        );
        assert!(t.contains("thin = tf.sdm.thin(clean, distance=10000, seed=1)"), "{t}");
        assert!(t.contains("data = tf.sdm.dataset(thin, env=env, background=None)"), "{t}");
        assert!(t.contains("folds = tf.sdm.spatial_blocks(data, size=50000, k=4, seed=1)"), "{t}");
        assert!(t.contains("ev = tf.sdm.evaluate(ev_presences, ev_background)"), "{t}");
        assert!(
            t.contains("ev_presences = fit.predictions[\"maxnet\"] if hasattr(fit.predictions, \"columns\")"),
            "{t}"
        );
        assert!(t.contains("thr = tf.sdm.threshold(thr_presences, thr_background, method=\"max_kappa\")"), "{t}");
        assert!(t.contains("om = tf.sdm.omission_rate(om_train, om_test, threshold=\"mtp\")"), "{t}");
        parses(t);
    }

    #[test]
    fn the_habitat_the_dispersal_and_the_potential() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "b"
op = "nlm.random"

[[nodes]]
id = "cover"
op = "stack.of"
inputs = { forest = "a.map", crops = "b.map" }

[[nodes]]
id = "ranges"
op = "stack.of"
inputs = { toucan = "a.map" }

[[nodes]]
id = "aoh"
op = "sdm.area_of_habitat"
inputs = { ranges = "ranges.stack", land_cover = "cover.stack", original = "cover.stack" }
params = { natural = ["forest"], suitability = { toucan = ["forest"] }, cell_area = 1.0 }

[[nodes]]
id = "mig"
op = "sdm.migrate"
inputs = { initial = "a.map", suitability = "cover.stack" }
params = { kernel = [1.0, 0.4], steps = 2 }

[[nodes]]
id = "pot"
op = "ml.potential"
inputs = { landscape = "a.map", drivers = "cover.stack" }
params = { classes = [1], sample = 1000, n_estimators = 20 }
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("aoh = tf.sdm.area_of_habitat(ranges, cover, [\"forest\"], suitability={\"toucan\": [\"forest\"]}, cell_area=1.0, original=cover.sel(band=[\"forest\"]).fillna(cover.sel(band=[\"forest\"])))"),
            "{t}"
        );
        assert!(t.contains("mig = tf.sdm.migrate(a, suitability=cover, steps=2, kernel=[1.0, 0.4]"), "{t}");
        assert!(
            t.contains("pot = tf.simulate.Estimator(RandomForestClassifier(n_estimators=20, random_state=0), target=\"presence\", classes=[1], sample=1000, seed=1).fit(a, cover)"),
            "{t}"
        );
        assert!(t.contains("fit(a, cover)"), "{t}");
        assert!(!t.contains("# (importance"), "{t}");
        parses(t);
    }

    #[test]
    fn the_boosting_has_no_gain_importance() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "pot"
op = "ml.potential"
inputs = { landscape = "a.map", drivers = "a.map" }
params = { method = "gradient_boosting" }
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        assert!(s.text.contains("# (importance: the share of the gain"), "{}", s.text);
        assert!(s.text.contains("early_stopping=False"), "{}", s.text);
    }
}
