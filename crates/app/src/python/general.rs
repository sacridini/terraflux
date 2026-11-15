//! The Python counterparts of the operators of the rest (bio, accuracy, redd, stats, uncertainty, sensitivity, fragmentation, landscape, nlm, metrics, forest_code, data, batch, raster).
//!
//! Left as a comment, on purpose: `batch.run` (its operator names and parameters are Terraflux's, the library's tools
//! are other functions), `bio.randomise` and `bio.properties` (the library works on a spatial result and mutates the
//! basedata), `metrics.sample` and `metrics.correlation` (the library takes and gives the long table of the metrics,
//! Terraflux's is wide), `raster.stats`, `raster.classify`, `raster.convert` without a path, and the operators the
//! library leaves to the Python ecosystem (focal, majority, sieve, label, fill, mosaic, contour, expand, shrink,
//! standardize, correlation, legend).

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[
    Py {
        op: "accuracy.sample_size",
        call: "tf.accuracy.sample_size",
        args: &[
            ("map", ""),
            ("user_accuracy", "user_accuracy"),
            ("standard_error", "standard_error"),
            ("n", "n"),
            ("allocation", "allocation"),
            ("minimum", "minimum"),
        ],
        defaults: &[],
        outs: &[("table", ".table"), ("n", ".n")],
        kind: Kind::Plain,
    },
    Py {
        op: "accuracy.assess",
        call: "tf.accuracy.assess",
        args: &[
            ("map", ""),
            ("points", "points"),
            ("samples", "samples"),
            ("strata", "strata"),
            ("reference", "reference"),
            ("estimator", "estimator"),
            ("confidence", "confidence"),
            ("cell_area", "cell_area"),
        ],
        defaults: &[],
        outs: &[("table", ".table"), ("overall", ".overall"), ("matrix", ".matrix"), ("counts", ".counts")],
        kind: Kind::Plain,
    },
    Py {
        op: "stats.getis_ord",
        call: "tf.stats.getis_ord",
        args: &[("map", ""), ("contiguity", "contiguity")],
        defaults: &[],
        outs: &[("g", ".g"), ("z", ".z"), ("p", ".p"), ("spots", ".spots")],
        kind: Kind::Plain,
    },
    Py {
        op: "stats.lisa",
        call: "tf.stats.lisa",
        args: &[
            ("map", ""),
            ("permutations", "permutations"),
            ("alpha", "alpha"),
            ("seed", "seed"),
            ("contiguity", "contiguity"),
        ],
        defaults: &[],
        outs: &[("i", ".i"), ("quadrant", ".quadrant"), ("z", ".z"), ("p", ".p"), ("clusters", ".clusters")],
        kind: Kind::Plain,
    },
    Py {
        op: "stats.trend",
        call: "tf.stats.trend",
        args: &[("maps", ""), ("alpha", "alpha")],
        defaults: &[],
        outs: &[
            ("slope", ".slope"),
            ("intercept", ".intercept"),
            ("p", ".p"),
            ("z", ".z"),
            ("tau", ".tau"),
            ("trend", ".trend"),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "raster.local_correlation",
        call: "tf.stats.local_correlation",
        args: &[("x", ""), ("y", "y"), ("radius", "radius"), ("shape", "shape"), ("min_cells", "min_cells")],
        defaults: &[],
        outs: &[("correlation", ".correlation"), ("slope", ".slope"), ("intercept", ".intercept"), ("r2", ".r2")],
        kind: Kind::Plain,
    },
    Py {
        op: "raster.tabulate",
        call: "tf.tabulate_area",
        args: &[("zones", ""), ("classes", "classes"), ("units", "units")],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "uncertainty.interval",
        call: "tf.uncertainty.interval",
        args: &[("value", ""), ("sd", "sd"), ("level", "level")],
        defaults: &[],
        outs: &[("low", "[0]"), ("high", "[1]")],
        kind: Kind::Plain,
    },
    Py {
        op: "redd.fit",
        call: "tf.redd.fit",
        args: &[("classes", ""), ("deforestation", "deforestation"), ("subdivisions", "subdivisions")],
        defaults: &[],
        outs: &[("table", ".table"), ("density", ".density")],
        kind: Kind::Plain,
    },
    Py {
        op: "data.environment90m",
        call: "tf.download.environment90m",
        args: &[("sub_catchments", ""), ("variables", "variables"), ("stats", "stats"), ("to_map", "to_map")],
        defaults: &[],
        outs: &[("table", ".table"), ("map", ".map")],
        kind: Kind::Plain,
    },
    Py {
        op: "forest_code.reserve",
        call: "tf.forest_code.reserve",
        args: &[
            ("properties", ""),
            ("vegetation", "vegetation"),
            ("share", "share"),
            ("app", "app"),
            ("vegetation_2008", "vegetation_2008"),
            ("fiscal_module", "fiscal_module"),
            ("count_app", "count_app"),
        ],
        defaults: &[],
        outs: &[("table", ".table"), ("balance", ".balance")],
        kind: Kind::Plain,
    },
    Py {
        op: "nlm.rescale",
        call: "tf.nlm.rescale",
        args: &[("map", "")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    // the kernels of GuidosToolbox: the foreground's classes, or the map coded 0 missing, 1 background, 2 foreground
    Py {
        op: "fragmentation.clustering",
        call: "tf.clustering_fac",
        args: &[("map", ""), ("window", "window_size")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Habitat,
    },
    Py {
        op: "fragmentation.contagion",
        call: "tf.contagion_pff",
        args: &[("map", ""), ("window", "window_size")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Habitat,
    },
    Py {
        op: "fragmentation.entropy_map",
        call: "tf.spatial_entropy_map",
        args: &[("map", ""), ("window", "window_size")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Habitat,
    },
    Py {
        op: "landscape.mspa_tiled",
        call: "tf.mspa_tiled",
        args: &[
            ("map", ""),
            ("connectivity", "connectivity"),
            ("edge_width", "edge_width"),
            ("internal", "internal"),
            ("tile_size", "tile_size"),
            ("overlap", "overlap"),
        ],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Habitat,
    },
    Py {
        op: "bio.points",
        call: "tf.bio.BaseData.from_points",
        args: &[
            ("points", ""),
            ("label", "label_columns"),
            ("cell_sizes", "cell_sizes"),
            ("cell_origins", "cell_origins"),
            ("count", "count_column"),
        ],
        defaults: &[],
        outs: &[("basedata", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "bio.region_grower",
        call: "tf.bio.region_grower",
        args: &[
            ("basedata", ""),
            ("index", "index"),
            ("conditions", "conditions"),
            ("calculations", "calculations"),
            ("tree", "tree"),
            ("matrix", "matrix"),
            ("tie_breaker", "tie_breaker"),
            ("seed", "prng_seed"),
        ],
        defaults: &[("prng_seed", "1")],
        outs: &[("tree", ".tree")],
        kind: Kind::Plain,
    },
];

// -- helpers ----------------------------------------------------------------------------------------------------------

/// The settings and inputs of the node outside `known`, as comments (what the script does not pass).
fn rest(n: &Node, known: &[&str]) -> String {
    let mut s = String::new();
    for k in n.inputs.keys().filter(|k| !known.contains(&k.as_str())) {
        s.push_str(&format!("# (not passed: the input {k})\n"));
    }
    for (k, x) in n.params.iter().filter(|(k, _)| !known.contains(&k.as_str())) {
        s.push_str(&format!("# (not passed: {k} = {})\n", literal(x)));
    }
    s
}

/// A port whose value is the variable itself.
fn one(var: &str, port: &str) -> HashMap<String, String> {
    HashMap::from([(port.to_string(), var.to_string())])
}

/// The outputs: port and the expression.
fn outs(pairs: &[(&str, String)]) -> HashMap<String, String> {
    pairs.iter().map(|(p, e)| (p.to_string(), e.clone())).collect()
}

/// A setting that is one text or a list of them, always as a list (the library takes a sequence of columns).
fn listed(n: &Node, name: &str, sc: &Scope) -> Result<Option<String>, String> {
    match n.params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Array(_)) => setting(n, name, sc),
        Some(x) if terraflux_graph::setting_ref(x).is_some() => setting(n, name, sc),
        Some(_) => Ok(setting(n, name, sc)?.map(|s| format!("[{s}]"))),
    }
}

/// The separator or quote of a delimited text: Terraflux's `guess` is the library's None, `tab` and `space` are characters.
fn guessed(n: &Node, name: &str, sc: &Scope) -> Result<Option<String>, String> {
    match n.params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(match s.as_str() {
            "" | "guess" => None,
            "tab" => Some(text("\t")),
            "space" => Some(text(" ")),
            s => Some(text(s)),
        }),
        Some(_) => setting(n, name, sc),
    }
}

/// The map of a GuidosToolbox kernel as the library reads it: the foreground's classes, or the map coded 0, 1, 2.
fn habitat(n: &Node, sc: &Scope) -> Result<String, String> {
    let m = value_of(n, "map", sc)?.ok_or("its map is not connected")?;
    Ok(match value_of(n, "foreground", sc)? {
        Some(fg) => format!("tf.reclassify({m}, foreground={fg})"),
        None => format!("{m}.fillna(0).astype(\"uint8\")"),
    })
}

/// GDAL's names of the resampling methods as the library writes them.
fn resampling_name(s: &str) -> Result<String, String> {
    match s {
        "near" => Ok("nearest".into()),
        "cubicspline" => Ok("cubic_spline".into()),
        "sum" => Err("its resampling sum: not in the library".into()),
        s => Ok(s.to_string()),
    }
}

/// The resampling given (a name, converted), or by the data as Terraflux chooses: classes (whole numbers) the nearest
/// cell or, when the cells grow (`coarser`, a Python expression), the most common; values bilinear or the mean.
fn resampling(n: &Node, sc: &Scope, raster: &str, coarser: &str) -> Result<String, String> {
    match n.params.get("resampling") {
        Some(Value::String(s)) if s != "auto" && !s.is_empty() => Ok(text(&resampling_name(s)?)),
        Some(x) if terraflux_graph::setting_ref(x).is_some() => Ok(setting(n, "resampling", sc)?.unwrap_or_default()),
        _ => Ok(format!(
            "((\"mode\" if {coarser} else \"nearest\") if np.issubdtype({raster}.dtype, np.integer) else (\"average\" if {coarser} else \"bilinear\"))"
        )),
    }
}

/// The first number of a setting that is a number or a list of them (a literal), else a Python expression.
fn first_number(n: &Node, name: &str, sc: &Scope) -> Result<Option<String>, String> {
    Ok(match n.params.get(name) {
        None | Some(Value::Null) => None,
        Some(Value::Array(a)) => a.first().map(literal),
        Some(_) => setting(n, name, sc)?,
    })
}

/// The settings of a landscape metrics node that are the library's metric selection and options.
const METRIC_SETTINGS: &[&str] = &[
    "metrics",
    "type",
    "extras",
    "directions",
    "count_boundary",
    "consider_boundary",
    "edge_depth",
    "cell_center",
    "classes_max",
    "neighbourhood",
    "ordered",
    "base",
    "convention",
    "search_radius",
    "connect_threshold",
    "boundary_contrast",
    "contrast",
    "similarity",
];

/// The names of a node's settings that are `known` or the metrics' selection and options.
fn with_metrics<'a>(known: &[&'a str]) -> Vec<&'a str> {
    known.iter().copied().chain(METRIC_SETTINGS.iter().copied()).collect()
}

/// The selection and options of the landscape metrics as the library's keywords (as `metrics.class`'s special).
fn metric_args(n: &Node, sc: &Scope, skip: &[&str]) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    for (k, x) in &n.params {
        if skip.contains(&k.as_str()) || !METRIC_SETTINGS.contains(&k.as_str()) {
            continue;
        }
        let v = setting(n, k, sc)?.unwrap_or_else(|| literal(x));
        match k.as_str() {
            "metrics" => args.push(format!("metric={v}")),
            // (the library's types end in " metric": "diversity metric")
            "type" => match x.as_str() {
                Some(t) if !t.ends_with(" metric") => args.push(format!("type={}", text(&format!("{t} metric")))),
                _ => args.push(format!("type={v}")),
            },
            k => args.push(format!("{k}={v}")),
        }
    }
    Ok(args)
}

/// The zones of a signature or a series of metrics: polygons as `zones`, points as buffers of `size` and `shape`, or
/// the `grid`'s squares, as the library's keywords (the code that makes them, and the keywords, when the node has them).
fn zonal(n: &Node, var: &str, sc: &Scope) -> Result<(String, Vec<String>), String> {
    let mut code = String::new();
    let mut kw = Vec::new();
    if let Some(p) = value_of(n, "plots", sc)? {
        let size = setting(n, "size", sc)?.unwrap_or_else(|| "None".into());
        let shape = setting(n, "shape", sc)?.unwrap_or_else(|| text("square"));
        code.push_str(&format!(
            "_{var}_plots = {p}\n_{var}_zonal = dict(zones=_{var}_plots) if _{var}_plots.geom_type.isin([\"Polygon\", \"MultiPolygon\"]).all() else dict(points=_{var}_plots, size={size}, shape={shape})\n"
        ));
        kw.push(format!("**_{var}_zonal"));
    }
    if let Some(g) = setting(n, "grid", sc)? {
        kw.push(format!("grid={g}"));
    }
    Ok((code, kw))
}

/// The maps of a series (or one map) as the rasters of the library: a stack's layers as a list.
fn layers(n: &Node, var: &str, port: &str, sc: &Scope) -> Result<String, String> {
    let m = value_of(n, port, sc)?.ok_or_else(|| format!("its {port} is not connected"))?;
    Ok(format!(
        "_{var}_maps = {m}\n_{var}_layers = [_{var}_maps.isel(band=i, drop=True) for i in range(_{var}_maps.sizes[\"band\"])] if \"band\" in _{var}_maps.dims and _{var}_maps.sizes[\"band\"] > 1 else _{var}_maps\n"
    ))
}

/// The keys of a table of metrics: the columns that name a row, not a metric (Terraflux's own list).
const KEYS: &str = "(\"class\", \"patch\", \"id\", \"plot_id\", \"size\", \"zone\", \"date\", \"layer\", \"percentage_inside\", \"landscape\", \"cluster\")";

/// The spatial signatures of a table of metrics: its keys as the index, the metrics `columns` or all the others.
fn signatures_of(n: &Node, var: &str, sc: &Scope) -> Result<String, String> {
    let t = value_of(n, "table", sc)?.ok_or("its table is not connected")?;
    let mut code = format!(
        "_{var}_keys = [c for c in {t}.columns if c in {KEYS}]\n_{var}_in = {t}.set_index(_{var}_keys) if _{var}_keys else {t}\n"
    );
    if let Some(c) = listed(n, "columns", sc)? {
        code.push_str(&format!("_{var}_in = _{var}_in[{c}]\n"));
    }
    code.push_str(&format!("_{var}_sig = tf.SpatialSignatureAnalysis.from_table(_{var}_in)\n"));
    Ok(code)
}

/// The rows of a result of the signatures with their keys back as columns.
fn keyed(var: &str, df: &str) -> String {
    format!("{df}.reset_index(drop=not _{var}_keys)")
}

/// The clustering's settings as the library's keywords.
fn cluster_args(n: &Node, sc: &Scope) -> Result<Vec<String>, String> {
    let mut a = Vec::new();
    for (ours, theirs) in [("method", "method"), ("components", "n_components"), ("n_init", "n_init"), ("seed", "seed")]
    {
        if let Some(v) = setting(n, ours, sc)? {
            a.push(format!("{theirs}={v}"));
        }
    }
    Ok(a)
}

/// A table of the groups of a basedata as Terraflux's: `group` (1..), the coordinates (x, y, z) and the columns of
/// `df` (indexed by the groups' names). `out` is the table.
fn group_table(var: &str, bd: &str, df: &str, out: &str) -> String {
    format!(
        "_{var}_g = [{bd}.group_index(g) for g in {df}.index]\n{out} = {df}.reset_index(drop=True)\n[{out}.insert(i, [\"x\", \"y\", \"z\"][i] if i < 3 else f\"axis_{{i}}\", {bd}.coords[_{var}_g, i]) for i in range({bd}.n_axes)]\n{out}.insert(0, \"group\", np.array(_{var}_g) + 1)\n"
    )
}

/// The `definition_query` as the library takes it: none when empty.
fn query(n: &Node, sc: &Scope) -> Result<Option<String>, String> {
    match n.params.get("definition_query") {
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        _ => setting(n, "definition_query", sc),
    }
}

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let v = |k: &str| value_of(n, k, sc);
    let need =
        |k: &str, what: &str| -> Result<String, String> { v(k)?.ok_or_else(|| format!("{what} is not connected")) };
    let code: Code = match n.op.as_str() {
        // -- accuracy, stats, uncertainty, sensitivity ----------------------------------------------------------------
        "accuracy.sample" => {
            let map = need("map", "its map")?;
            let mut args = vec![map.clone()];
            if let Some(s) = v("sizes")? {
                args.push(format!("sizes={s}"));
            }
            args.extend(given(n, &[("n", "n")], sc)?);
            args.push(format!("seed={}", setting(n, "seed", sc)?.unwrap_or_else(|| "1".into())));
            let mut code = rest(n, &["map", "sizes", "n", "seed"]);
            code.push_str(&format!("{var} = tf.accuracy.sample({})\n", args.join(", ")));
            // (the stratum of the cells drawn, missing elsewhere)
            code.push_str(&format!(
                "_{var}_c = np.full({map}.shape[-2:], np.nan)\n_{var}_c[{var}[\"row\"].to_numpy(), {var}[\"col\"].to_numpy()] = {var}[\"stratum\"].to_numpy()\n_{var}_c = ({map}.isel(band=0, drop=True) if \"band\" in {map}.dims else {map}).copy(data=_{var}_c)\n"
            ));
            (code, outs(&[("points", var.to_string()), ("cells", format!("_{var}_c"))]), Need::Nothing)
        }
        "stats.variogram" => {
            let mut args = vec![need("points", "its points")?];
            args.extend(given(
                n,
                &[
                    ("field", "field"),
                    ("cutoff", "cutoff"),
                    ("width", "width"),
                    ("psill", "psill"),
                    ("range", "range"),
                    ("nugget", "nugget"),
                    ("fit_nugget", "fit_nugget"),
                ],
                sc,
            )?);
            match n.params.get("model") {
                Some(Value::String(s)) if s == "none" => args.push("model=None".into()),
                _ => args.extend(given(n, &[("model", "model")], sc)?),
            }
            let known = ["points", "field", "cutoff", "width", "model", "psill", "range", "nugget", "fit_nugget"];
            let code = format!("{}{var} = tf.stats.variogram({})\n", rest(n, &known), args.join(", "));
            (code, outs(&[("table", format!("{var}.table")), ("model", format!("{var}.model"))]), Need::Nothing)
        }
        "stats.kriging" => {
            let points = need("points", "its points")?;
            let like = need("like", "its map (like)")?;
            let model = match v("model")? {
                Some(m) => m,
                None => {
                    let shape = setting(n, "shape", sc)?.unwrap_or_else(|| text("spherical"));
                    let psill = setting(n, "psill", sc)?.ok_or("connect a model, or give its psill")?;
                    let range = setting(n, "range", sc)?.ok_or("connect a model, or give its range")?;
                    let nugget = setting(n, "nugget", sc)?.unwrap_or_else(|| "0".into());
                    format!("{{\"shape\": {shape}, \"psill\": {psill}, \"range\": {range}, \"nugget\": {nugget}}}")
                }
            };
            let mut args = vec![points, like, model];
            args.extend(given(n, &[("field", "field"), ("nmax", "nmax")], sc)?);
            let known = ["points", "like", "model", "field", "nmax", "shape", "psill", "range", "nugget"];
            let code = format!("{}{var} = tf.stats.kriging({})\n", rest(n, &known), args.join(", "));
            (
                code,
                outs(&[("prediction", format!("{var}.prediction")), ("variance", format!("{var}.variance"))]),
                Need::Nothing,
            )
        }
        "sensitivity.analyze" => {
            let mut args = vec![need("design", "its design")?, need("results", "its results")?];
            args.extend(given(
                n,
                &[
                    ("column", "column"),
                    ("method", "method"),
                    ("levels", "levels"),
                    ("second_order", "second_order"),
                    ("resamples", "resamples"),
                    ("confidence", "confidence"),
                    ("seed", "seed"),
                ],
                sc,
            )?);
            let known =
                ["design", "results", "column", "method", "levels", "second_order", "resamples", "confidence", "seed"];
            let code = format!("{}{var} = tf.sensitivity.analyze({})\n", rest(n, &known), args.join(", "));
            // (the factors' names in a column `factor`)
            let table = |f: &str| format!("{var}.{f}.rename_axis(\"factor\").reset_index()");
            (
                code,
                outs(&[("indices", table("indices")), ("s2", table("s2")), ("s2_conf", table("s2_conf"))]),
                Need::Nothing,
            )
        }
        "uncertainty.delta" | "uncertainty.monte_carlo" => {
            let delta = n.op == "uncertainty.delta";
            let expr = n.params.get("expression").and_then(Value::as_str).ok_or("no expression")?;
            let parameters = match n.params.get("parameters") {
                Some(Value::Array(a)) => a,
                _ => return Err("its parameters: [[\"name\", value, sd], ...]".into()),
            };
            let (mut names, mut values, mut sds) = (Vec::new(), Vec::new(), Vec::new());
            for p in parameters {
                match p.as_array().map(Vec::as_slice) {
                    Some([Value::String(name), value, sd]) if value.is_number() && sd.is_number() => {
                        names.push(name.clone());
                        values.push(literal(value));
                        sds.push(literal(sd));
                    }
                    _ => return Err("its parameters: [[\"name\", value, sd], ...]".into()),
                }
            }
            // the expression run by the calculator's helper over the node's maps and numbers and the drawn parameters
            let mut f_args = Vec::new();
            for k in n.inputs.keys() {
                f_args.push(format!("{k}={}", v(k)?.unwrap_or_else(|| "None".into())));
            }
            f_args.extend(names.iter().enumerate().map(|(i, name)| format!("{name}=p[{i}]")));
            let (call, keys): (&str, &[(&str, &str)]) = if delta {
                ("delta_method", &[("method", "method"), ("step", "step"), ("scheme", "scheme")])
            } else {
                ("monte_carlo", &[("n", "n"), ("seed", "seed"), ("quantiles", "quantiles")])
            };
            let inputs: Vec<&str> = n.inputs.keys().map(String::as_str).collect();
            let known: Vec<&str> = ["expression", "parameters", "method", "step", "scheme", "n", "seed", "quantiles"]
                .into_iter()
                .chain(inputs)
                .collect();
            let mut code = rest(n, &known);
            code.push_str(&format!(
                "def _{var}_f(p):\n    return _calc({}, {})[\"map\"]\n",
                text(expr),
                f_args.join(", ")
            ));
            let mut args =
                vec![format!("_{var}_f"), format!("[{}]", values.join(", ")), format!("[{}]", sds.join(", "))];
            args.extend(given(n, keys, sc)?);
            code.push_str(&format!("{var} = tf.uncertainty.{call}({})\n", args.join(", ")));
            let o = if delta {
                outs(&[("value", format!("{var}.value")), ("sd", format!("{var}.sd"))])
            } else {
                // (the quantiles of a map are rows over its cells in the library: not given)
                outs(&[("mean", format!("{var}.mean")), ("sd", format!("{var}.sd"))])
            };
            (code, o, Need::Calc)
        }
        // -- redd -----------------------------------------------------------------------------------------------------
        "redd.vulnerability" => {
            let mut tail = Vec::new();
            for (ours, theirs) in [("mask", "mask"), ("classes", "classes")] {
                if let Some(x) = v(ours)? {
                    tail.push(format!("{theirs}={x}"));
                }
            }
            let (call, args, known): (&str, Vec<String>, &[&str]) = match (v("distance")?, v("vulnerability")?) {
                (Some(_), Some(_)) => return Err("the distance (benchmark) or a vulnerability, not both".into()),
                (Some(d), None) => (
                    "tf.redd.benchmark",
                    vec![d, need("nrt", "its negligible risk threshold (nrt)")?],
                    &["distance", "nrt", "mask", "classes"],
                ),
                (None, Some(x)) => (
                    "tf.redd.alternative",
                    vec![x, need("forest", "its forest")?],
                    &["vulnerability", "forest", "mask", "classes"],
                ),
                (None, None) => return Err("connect the distance to the edge, or a map of vulnerability".into()),
            };
            let args: Vec<String> = args.into_iter().chain(tail).collect();
            let code = format!("{}{var} = {call}({})\n", rest(n, known), args.join(", "));
            (code, one(var, "classes"), Need::Nothing)
        }
        "redd.allocate" => {
            let mut args = vec![need("classes", "its classes")?, need("table", "the fit's table")?];
            for k in ["subdivisions", "expected", "deforestation", "max_iterations"] {
                if let Some(x) = v(k)? {
                    args.push(format!("{k}={x}"));
                }
            }
            let known = ["classes", "table", "subdivisions", "expected", "deforestation", "max_iterations"];
            let mut code = rest(n, &known);
            code.push_str(&format!("import pandas as pd\n{var} = tf.redd.allocate({})\n", args.join(", ")));
            code.push_str(&format!(
                "_{var}_info = pd.DataFrame({{\"adjustment_ratio\": [{var}.adjustment_ratio], \"iterations\": [{var}.iterations], \"expected_ha\": [{var}.expected_ha], \"allocated_ha\": [{var}.allocated_ha], \"filled\": [{var}.filled]}})\n"
            ));
            (code, outs(&[("density", format!("{var}.density")), ("info", format!("_{var}_info"))]), Need::Nothing)
        }
        "redd.assess" => {
            let mut args = vec![need("density", "its density")?, need("deforestation", "its deforestation")?];
            args.push(format!("grid_area={}", need("grid_area", "its grid_area")?));
            if let Some(m) = v("mask")? {
                args.push(format!("mask={m}"));
            }
            let mut code = rest(n, &["density", "deforestation", "mask", "grid_area"]);
            code.push_str(&format!(
                "import pandas as pd\n{var} = tf.redd.assess({})\n_{var}_metrics = pd.DataFrame([{var}.metrics])\n",
                args.join(", ")
            ));
            let o = outs(&[
                ("table", format!("{var}.table")),
                ("metrics", format!("_{var}_metrics")),
                ("residuals", format!("{var}.residuals")),
            ]);
            (code, o, Need::Nothing)
        }
        // -- forest code, data ----------------------------------------------------------------------------------------
        "forest_code.app" => {
            let known = [
                "rivers",
                "river_width",
                "lakes",
                "urban",
                "springs",
                "veredas",
                "slope",
                "elevation",
                "hilltops",
                "landcover",
                "properties",
                "consolidated",
                "protected_classes",
                "fiscal_module",
                "over_four_modules",
            ];
            let mut args = Vec::new();
            for k in known {
                if let Some(x) = v(k)? {
                    args.push(format!("{k}={x}"));
                }
            }
            // (the library gives neither art. 61-B's recomposition nor the properties' table: those ports are not given)
            let code = format!("{}{var} = tf.forest_code.app({})\n", rest(n, &known), args.join(", "));
            (
                code,
                outs(&[("app", format!("{var}.app")), ("table", format!("{var}.table.drop(columns=\"name\")"))]),
                Need::Nothing,
            )
        }
        "data.download" => {
            if n.inputs.contains_key("area") {
                return Err("its area (polygons to cut the data to): not in the script yet".into());
            }
            let source = match n.params.get("source") {
                Some(Value::String(s)) if !s.is_empty() => Some(s.as_str()),
                Some(Value::String(_)) | None => None,
                Some(_) => return Err("its source is a value of the model: not in the script yet".into()),
            };
            let (mut first, mut args): (Option<String>, Vec<String>) = (None, Vec::new());
            let (call, mut known): (&str, Vec<&str>) = match source {
                None if n.params.contains_key("url") => {
                    first = setting(n, "url", sc)?;
                    ("tf.download.raster", vec!["url"])
                }
                Some(s @ ("worldclim" | "worldclim_future")) => {
                    args.extend(given(
                        n,
                        &[("variable", "variable"), ("layers", "layers"), ("resolution", "resolution")],
                        sc,
                    )?);
                    if s == "worldclim_future" {
                        args.push(format!("gcm={}", setting(n, "gcm", sc)?.unwrap_or_else(|| text("MPI-ESM1-2-HR"))));
                        args.extend(given(n, &[("ssp", "ssp"), ("period", "period")], sc)?);
                    }
                    ("tf.download.worldclim", vec!["variable", "layers", "resolution", "gcm", "ssp", "period"])
                }
                Some(s @ ("chelsa" | "chelsa_future")) => {
                    args.extend(given(n, &[("layers", "layers")], sc)?);
                    if s == "chelsa_future" {
                        args.push(format!("gcm={}", setting(n, "gcm", sc)?.unwrap_or_else(|| text("MPI-ESM1-2-HR"))));
                        args.extend(given(n, &[("ssp", "ssp"), ("period", "period")], sc)?);
                    }
                    ("tf.download.chelsa", vec!["layers", "gcm", "ssp", "period"])
                }
                Some("copernicus_dem") => {
                    args.extend(given(n, &[("dem_resolution", "resolution")], sc)?);
                    ("tf.download.copernicus_dem", vec!["dem_resolution"])
                }
                Some("soilgrids") => {
                    args.extend(given(n, &[("property", "property"), ("depth", "depth"), ("stat", "stat")], sc)?);
                    ("tf.download.soilgrids", vec!["property", "depth", "stat"])
                }
                Some("mapbiomas") => {
                    args.extend(given(n, &[("years", "years"), ("collection", "collection")], sc)?);
                    ("tf.download.mapbiomas", vec!["years", "collection"])
                }
                Some("hydrography90m") => {
                    args.extend(given(n, &[("variable", "variable")], sc)?);
                    ("tf.download.hydrography90m", vec!["variable"])
                }
                Some(s) => return Err(format!("its source {s}: not a function of the library")),
                None => return Err("its source (or url) is not given".into()),
            };
            if let Some(l) = v("like")? {
                args.insert(0, format!("like={l}"));
            }
            args.extend(given(n, &[("extent", "extent"), ("cellsize", "cellsize"), ("resampling", "resampling")], sc)?);
            known.extend(["source", "like", "extent", "cellsize", "resampling"]);
            let all: Vec<String> = first.into_iter().chain(args).collect();
            let code = format!("{}{var} = {call}({})\n", rest(n, &known), all.join(", "));
            (code, one(var, "raster"), Need::Nothing)
        }
        // -- fragmentation, landscape ---------------------------------------------------------------------------------
        "fragmentation.entropy" => {
            let m = habitat(n, sc)?;
            let code = format!(
                "{}import pandas as pd\n{var} = tf.spatial_entropy({m})\n_{var}_pairs = pd.DataFrame({{\"code\": np.flatnonzero({var}.histogram), \"count\": {var}.histogram[{var}.histogram > 0]}})\n",
                rest(n, &["map", "foreground"])
            );
            (
                code,
                outs(&[("index", format!("{var}.entropy_index")), ("pairs", format!("_{var}_pairs"))]),
                Need::Nothing,
            )
        }
        "fragmentation.gray_convolution" => {
            let mut args = vec![format!("{}.fillna(255).astype(\"uint8\")", need("map", "its map")?)];
            args.extend(given(
                n,
                &[("metric", "metric"), ("window", "window_size"), ("mask_missing", "mask_missing")],
                sc,
            )?);
            // (the library's missing is 255, or -0.01 in its output: no value)
            let code = format!(
                "{}{var} = tf.gray_spatial_convolution({})\n{var} = {var}.where(({var} >= 0) & ({var} <= 100))\n",
                rest(n, &["map", "metric", "window", "mask_missing"]),
                args.join(", ")
            );
            (code, one(var, "map"), Need::Nothing)
        }
        "fragmentation.landscape_mosaic" => {
            let m = need("map", "its map")?;
            let mut given_classes = Vec::new();
            for (name, code) in [("agriculture", 1), ("natural", 2), ("developed", 3)] {
                if let Some(c) = listed(n, name, sc)? {
                    given_classes.push((code, c));
                }
            }
            let mut code = rest(n, &["map", "agriculture", "natural", "developed", "window", "classes"]);
            let input = if given_classes.is_empty() {
                format!("{m}.fillna(0).astype(\"uint8\")")
            } else {
                // (the first of the three whose classes hold the cell; the others missing)
                let mut e = "0".to_string();
                for (k, c) in given_classes.iter().rev() {
                    e = format!("np.where(np.isin({m}.values, {c}), {k}, {e})");
                }
                format!("{m}.copy(data={e}.astype(\"uint8\"))")
            };
            code.push_str(&format!("_{var}_in = {input}\n"));
            let window = setting(n, "window", sc)?.unwrap_or_else(|| "27".into());
            let classes = setting(n, "classes", sc)?.unwrap_or_else(|| "19".into());
            code.push_str(&format!(
                "{var} = tf.landscape_mosaic(_{var}_in, window_size={window}, output_classes={classes})\n_{var}_stats = tf.landscape_mosaic_stats(_{var}_in, window_size={window}).rename(columns={{\"class_code\": \"class\", \"pixel_count\": \"cells\", \"percentage\": \"percent\", \"recode_class_19\": \"class_19\"}})[[\"class\", \"cells\", \"percent\", \"class_19\"]]\n"
            ));
            (code, outs(&[("map", var.to_string()), ("stats", format!("_{var}_stats"))]), Need::Nothing)
        }
        "landscape.contortion" => {
            let m = habitat(n, sc)?;
            let mut a = vec![format!("_{var}_in")];
            a.extend(given(n, &[("min_area", "min_area")], sc)?);
            let mut d = a.clone();
            d.push("return_display=True".into());
            let code = format!(
                "{}_{var}_in = {m}\n{var} = tf.contortion({})\n_{var}_display = tf.contortion({})\n",
                rest(n, &["map", "foreground", "min_area"]),
                a.join(", "),
                d.join(", ")
            );
            (code, outs(&[("map", var.to_string()), ("display", format!("_{var}_display"))]), Need::Nothing)
        }
        "landscape.parcellation" => {
            let mut args = vec![format!("{}.fillna(0)", need("map", "its map")?)];
            args.extend(given(n, &[("connectivity", "connectivity")], sc)?);
            args.push("return_labels=True".into());
            // (the library rounds the statistics: APS and AWAPS to 2 decimals, the division to 4, PARC to 2)
            let code = format!(
                "{}import pandas as pd\n{var} = tf.parcellation({})\n_{var}_stats = {var}.stats.rename(columns={{\"Class\": \"class\", \"Count\": \"parcels\", \"Area[pixels]\": \"cells\", \"APS\": \"aps\", \"AWAPS\": \"awaps\", \"AWAPS/data\": \"awaps_data\", \"DIVISION\": \"division\", \"PARC[%]\": \"parc\"}})\n_{var}_summary = pd.DataFrame({{\"parcels\": [{var}.global_stats[\"total_parcels\"]], \"cells\": [{var}.global_stats[\"data_area\"]], \"aps\": [{var}.global_stats[\"aps\"]], \"awaps\": [{var}.global_stats[\"awaps\"]], \"division\": [{var}.global_stats[\"division\"]], \"parc\": [{var}.global_stats[\"parc\"]]}})\n",
                rest(n, &["map", "connectivity"]),
                args.join(", ")
            );
            let o = outs(&[
                ("parcels", format!("{var}.parcel_labels")),
                ("stats", format!("_{var}_stats")),
                ("summary", format!("_{var}_summary")),
            ]);
            (code, o, Need::Nothing)
        }
        // -- nlm ------------------------------------------------------------------------------------------------------
        "nlm.binarize" => {
            let m = need("map", "its map")?;
            let breaks = need("breaks", "its breaks")?;
            let code = format!("{}{var} = tf.nlm.binarize({m}, {breaks})\n", rest(n, &["map", "breaks"]));
            // (several shares: a layer each, by share)
            let map = match n.params.get("breaks") {
                Some(Value::Array(_)) => format!("{var}.rename(p=\"band\")"),
                _ => var.to_string(),
            };
            (code, outs(&[("map", map)]), Need::Nothing)
        }
        "nlm.mosaic_field" => {
            let like = v("like")?;
            let size = |ours: &str, dim: &str| -> Result<String, String> {
                Ok(match &like {
                    Some(l) => format!("{l}.sizes[\"{dim}\"]"),
                    None => setting(n, ours, sc)?.unwrap_or_else(|| "256".into()),
                })
            };
            let mut args = vec![size("columns", "x")?, size("rows", "y")?];
            args.extend(given(
                n,
                &[
                    ("resolution", "resolution"),
                    ("rescale", "rescale"),
                    ("collect", "collect"),
                    ("infinit", "infinit"),
                    ("nlmr_compat", "nlmr_compat"),
                ],
                sc,
            )?);
            args.extend(given(n, &[("mean", "mosaic_mean"), ("sd", "mosaic_sd")], sc)?);
            match n.params.get("n") {
                Some(Value::Number(k)) if k.as_i64() == Some(0) => args.push("n=None".into()),
                _ => args.extend(given(n, &[("n", "n")], sc)?),
            }
            args.push(format!("seed={}", setting(n, "seed", sc)?.unwrap_or_else(|| "1".into())));
            let flag = |k: &str| matches!(n.params.get(k), Some(Value::Bool(true)));
            // (with collect or infinit the library gives a dict of what was asked)
            let dict = flag("collect") || flag("infinit");
            let known = [
                "like",
                "rows",
                "columns",
                "resolution",
                "crs",
                "seed",
                "rescale",
                "n",
                "mean",
                "sd",
                "collect",
                "infinit",
                "nlmr_compat",
            ];
            let mut code = rest(n, &known);
            code.push_str(&format!("{var} = tf.nlm.mosaic_field({})\n", args.join(", ")));
            let field = if dict && n.params.get("n").and_then(Value::as_i64) != Some(0) {
                format!("{var}[\"mosaic_field\"]")
            } else {
                var.to_string()
            };
            // a drawn landscape's places and coordinate system (the library's models have none): `like`'s, else the setting's
            if let Some(l) = &like {
                code.push_str(&format!(
                    "{field} = {field}.assign_coords(x={l}.x, y={l}.y)\nif {l}.rio.crs is not None:\n    {field} = {field}.rio.write_crs({l}.rio.crs)\n"
                ));
            } else if let Some(crs) = setting(n, "crs", sc)?.filter(|c| c != "\"\"") {
                code.push_str(&format!("{field} = {field}.rio.write_crs({crs})\n"));
            }
            let mut o = outs(&[("map", field)]);
            let mut flow = Need::Nothing;
            if dict && flag("collect") {
                o.insert(
                    "steps".into(),
                    format!("_stack({{str(k): {var}[\"steps\"].sel(step=k) for k in {var}[\"steps\"].step.values}})"),
                );
                flow = Need::Flow;
            }
            if dict && flag("infinit") {
                o.insert("infinit".into(), format!("{var}[\"infinit\"]"));
            }
            (code, o, flow)
        }
        // -- raster ---------------------------------------------------------------------------------------------------
        "raster.clip" => {
            let r = need("raster", "its raster")?;
            let e = match n.params.get("extent") {
                Some(Value::Array(e)) if e.len() == 4 => e.iter().map(literal).collect::<Vec<_>>(),
                _ => return Err("its extent: [xmin, ymin, xmax, ymax] written in the model".into()),
            };
            // gdal_translate -projwin's window: the first cell at floor(x + 0.001) of the corner, its size rounded
            let (x0, y0, x1, y1) = (&e[0], &e[1], &e[2], &e[3]);
            let skipped = rest(n, &["raster", "extent"]);
            let code = format!(
                "{skipped}_{var}_t = {r}.rio.transform()\n_{var}_c = int(np.floor(({x0} - _{var}_t.c) / _{var}_t.a + 0.001))\n_{var}_r = int(np.floor(({y1} - _{var}_t.f) / _{var}_t.e + 0.001))\n_{var}_w = int(({x1} - {x0}) / _{var}_t.a + 0.5)\n_{var}_h = int(({y0} - {y1}) / _{var}_t.e + 0.5)\n{var} = {r}.isel(x=slice(_{var}_c, _{var}_c + _{var}_w), y=slice(_{var}_r, _{var}_r + _{var}_h))\n"
            );
            (code, one(var, "raster"), Need::Nothing)
        }
        "raster.match" => {
            let r = need("raster", "its raster")?;
            let like = need("like", "the raster whose grid to take")?;
            let coarser = format!("abs({like}.rio.resolution()[0]) > abs({r}.rio.resolution()[0]) * 1.5");
            let m = resampling(n, sc, &r, &coarser)?;
            let code = format!(
                "{}{var} = tf.match_grid({r}, {like}, resampling={m})\n",
                rest(n, &["raster", "like", "resampling"])
            );
            (code, one(var, "raster"), Need::Nothing)
        }
        "raster.resample" => {
            let r = need("raster", "its raster")?;
            let res = need("resolution", "its resolution")?;
            let x = first_number(n, "resolution", sc)?.ok_or("its resolution")?;
            let coarser = format!("{x} > abs({r}.rio.resolution()[0]) * 1.01");
            let m = resampling(n, sc, &r, &coarser)?;
            let code = format!(
                "{}{var} = tf.reproject({r}, res={res}, resampling={m})\n",
                rest(n, &["raster", "resolution", "resampling"])
            );
            (code, one(var, "raster"), Need::Nothing)
        }
        "raster.reproject" => {
            for k in ["extent", "options"] {
                if n.params.get(k).is_some_and(|x| !x.is_null() && x.as_str() != Some("")) {
                    return Err(format!("its {k}: not in the script yet"));
                }
            }
            let r = need("raster", "its raster")?;
            let crs = setting(n, "crs", sc)?.filter(|c| c != "\"\"");
            let res = setting(n, "resolution", sc)?;
            if crs.is_none() && res.is_none() {
                return Err("give a CRS or a resolution".into());
            }
            let coarser = match (&crs, first_number(n, "resolution", sc)?) {
                (None, Some(x)) => format!("{x} > abs({r}.rio.resolution()[0]) * 1.5"),
                _ => "False".to_string(),
            };
            let m = resampling(n, sc, &r, &coarser)?;
            let mut args = vec![r.clone()];
            args.extend(crs);
            if let Some(x) = res {
                args.push(format!("res={x}"));
            }
            args.push(format!("resampling={m}"));
            let known = ["raster", "crs", "resolution", "resampling", "source_crs", "extent", "options"];
            let code = format!("{}{var} = tf.reproject({})\n", rest(n, &known), args.join(", "));
            (code, one(var, "raster"), Need::Nothing)
        }
        "raster.zonal" => {
            let m = need("map", "its map")?;
            let zones = need("zones", "its zones")?;
            let stats: Option<Vec<String>> = match n.params.get("stats") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) => Some(vec![s.clone()]),
                Some(Value::Array(a)) => Some(a.iter().filter_map(|x| x.as_str().map(String::from)).collect()),
                Some(_) => return Err("its stats: names written in the model".into()),
            };
            if stats.iter().flatten().any(|s| s.starts_with("percentile")) {
                return Err("its percentiles (percentile_<p> here, p<p> in the library): not in the script yet".into());
            }
            let mut args = vec![m.clone(), zones.clone()];
            if let Some(s) = &stats {
                args.push(format!("stats={}", literal(&Value::from(s.clone()))));
            }
            let to_map = setting(n, "to_map", sc)?.unwrap_or_else(|| text("mean"));
            let code = format!(
                "{}{var} = tf.zonal_stats({})\n_{var}_map = tf.zonal_map({m}, {zones}, stat={to_map})\n",
                rest(n, &["map", "zones", "stats", "to_map"]),
                args.join(", ")
            );
            (code, outs(&[("table", var.to_string()), ("map", format!("_{var}_map"))]), Need::Nothing)
        }
        "raster.cell_area" => {
            let m = need("map", "its map")?;
            if n.params.get("geographic").is_some_and(|g| g.as_str() != Some("auto")) {
                return Err("its geographic (yes or no): not in the script yet".into());
            }
            let per = match n.params.get("units").and_then(Value::as_str) {
                None | Some("m2") => "1e6",
                Some("hectares") => "100",
                Some("km2") => "1",
                Some(u) => return Err(format!("its units {u}: m2, hectares or km2")),
            };
            let code = format!(
                "{}{var} = {m}.copy(data=tf.sdm.cell_area_km2({m}) * {per})\n",
                rest(n, &["map", "units", "geographic"])
            );
            (code, one(var, "map"), Need::Nothing)
        }
        "raster.convert" => {
            let r = need("raster", "its raster")?;
            let path = setting(n, "path", sc)?.ok_or("its path: the file made (else a cache of Terraflux's)")?;
            let mut args = vec![r, path.clone()];
            if let Some(t) = n.params.get("type").and_then(Value::as_str).filter(|t| !t.is_empty()) {
                let d = match t {
                    "Byte" => "uint8",
                    "Int8" => "int8",
                    "UInt16" => "uint16",
                    "Int16" => "int16",
                    "UInt32" => "uint32",
                    "Int32" => "int32",
                    "UInt64" => "uint64",
                    "Int64" => "int64",
                    "Float32" => "float32",
                    "Float64" => "float64",
                    t => return Err(format!("its type {t}: not a cell type of the library")),
                };
                args.push(format!("dtype={}", text(d)));
            }
            if let Some(f) = n.params.get("format").and_then(Value::as_str).filter(|f| !f.is_empty() && *f != "GTiff") {
                args.push(format!("driver={}", text(f)));
            }
            let code = format!(
                "{}tf.write_raster({})\n{var} = tf.load_raster({path})\n",
                rest(n, &["raster", "path", "type", "format"]),
                args.join(", ")
            );
            (code, one(var, "raster"), Need::Nothing)
        }
        // -- metrics --------------------------------------------------------------------------------------------------
        "metrics.window" => {
            let w = setting(n, "window", sc)?.unwrap_or_else(|| "5".into());
            let window = match n.params.get("shape").and_then(Value::as_str) {
                None | Some("square") => w,
                Some("circle") => {
                    format!("np.hypot(*np.mgrid[-({w} // 2):{w} // 2 + 1, -({w} // 2):{w} // 2 + 1]) <= {w} // 2")
                }
                Some(s) => return Err(format!("its shape {s}: square or circle")),
            };
            let mut args = vec![need("map", "its map")?, window, "level=\"landscape\"".to_string()];
            args.extend(metric_args(n, sc, &[])?);
            let known = with_metrics(&["map", "window", "shape"]);
            let code = format!("{}{var} = _stack(tf.window_metrics({}))\n", rest(n, &known), args.join(", "));
            (code, one(var, "maps"), Need::Flow)
        }
        "metrics.signatures" => {
            let mut code = layers(n, var, "maps", sc)?;
            let (zc, kw) = zonal(n, var, sc)?;
            code.push_str(&zc);
            let mut args = vec![format!("_{var}_layers")];
            args.extend(given(
                n,
                &[
                    ("landscape_metrics", "landscape_metrics"),
                    ("class_metrics", "class_metrics"),
                    ("classes", "classes"),
                    ("fillna", "fillna"),
                ],
                sc,
            )?);
            args.extend(kw);
            args.extend(metric_args(n, sc, &["metrics", "type", "extras"])?);
            let known = with_metrics(&[
                "maps",
                "plots",
                "landscape_metrics",
                "class_metrics",
                "classes",
                "fillna",
                "grid",
                "size",
                "shape",
            ]);
            code.push_str(&format!(
                "{var} = tf.SpatialSignatureAnalysis({})\n_{var}_table = {var}.table.reset_index()\n",
                args.join(", ")
            ));
            (format!("{}{code}", rest(n, &known)), outs(&[("table", format!("_{var}_table"))]), Need::Nothing)
        }
        "metrics.series" => {
            for k in ["type", "extras"] {
                if n.params.contains_key(k) {
                    return Err(format!("its {k}: the library takes the metrics by name here"));
                }
            }
            let mut code = layers(n, var, "maps", sc)?;
            let (zc, kw) = zonal(n, var, sc)?;
            code.push_str(&zc);
            // the dates: given, else the layers' names that are numbers, else 1, 2...
            let dates = match setting(n, "dates", sc)? {
                Some(d) => d,
                None => format!(
                    "[int(d) if str(d).isdigit() else i for i, d in enumerate(_{var}_maps[\"band\"].values, start=1)] if isinstance(_{var}_layers, list) else None"
                ),
            };
            let mut args = vec![format!("_{var}_layers"), format!("dates={dates}")];
            args.extend(kw);
            args.extend(metric_args(n, sc, &["metrics", "type", "extras"])?);
            let metrics = setting(n, "metrics", sc)?.unwrap_or_else(|| "None".into());
            let (call, fillna) = match n.params.get("level").and_then(Value::as_str) {
                None | Some("landscape") => ("landscape_metrics", String::new()),
                Some("class") => (
                    "class_metrics",
                    format!(", fillna={}", setting(n, "fillna", sc)?.unwrap_or_else(|| "True".into())),
                ),
                Some(l) => return Err(format!("its level {l}: landscape or class")),
            };
            let known = with_metrics(&["maps", "plots", "level", "dates", "grid", "size", "shape", "fillna"]);
            code.push_str(&format!(
                "{var} = tf.SpatioTemporalAnalysis({})\n_{var}_table = {var}.{call}({metrics}{fillna}).reset_index()\n",
                args.join(", ")
            ));
            (format!("{}{code}", rest(n, &known)), outs(&[("table", format!("_{var}_table"))]), Need::Nothing)
        }
        "metrics.pca" => {
            let mut code = signatures_of(n, var, sc)?;
            let k = setting(n, "components", sc)?.unwrap_or_default();
            code.push_str(&format!("{var} = _{var}_sig.pca({k})\n"));
            code.push_str(&format!(
                "_{var}_loadings = {var}.loadings.T.reset_index(drop=True).rename_axis(None, axis=1)\n_{var}_loadings.insert(0, \"explained\", {var}.explained_variance_ratio)\n_{var}_loadings.insert(0, \"component\", range(1, len(_{var}_loadings) + 1))\n"
            ));
            let o = outs(&[("scores", keyed(var, &format!("{var}.scores"))), ("loadings", format!("_{var}_loadings"))]);
            (format!("{}{code}", rest(n, &["table", "columns", "components"])), o, Need::Nothing)
        }
        "metrics.nmf" => {
            let mut code = signatures_of(n, var, sc)?;
            let mut args = vec![need("factors", "its factors")?];
            args.extend(given(n, &[("max_iter", "max_iter"), ("tol", "tol")], sc)?);
            code.push_str(&format!("{var} = _{var}_sig.nmf({})\n", args.join(", ")));
            code.push_str(&format!(
                "_{var}_components = {var}.components.T.reset_index(drop=True).rename_axis(None, axis=1)\n_{var}_components.insert(0, \"factor\", range(1, len(_{var}_components) + 1))\n"
            ));
            let o = outs(&[
                ("scores", keyed(var, &format!("{var}.scores"))),
                ("components", format!("_{var}_components")),
                ("error", format!("{var}.reconstruction_error")),
                ("iterations", format!("{var}.n_iter")),
            ]);
            (format!("{}{code}", rest(n, &["table", "columns", "factors", "max_iter", "tol"])), o, Need::Nothing)
        }
        "metrics.cluster" => {
            let mut code = signatures_of(n, var, sc)?;
            let mut args = vec![need("k", "its k")?];
            args.extend(cluster_args(n, sc)?);
            // (the rows without NA, their metrics and their cluster)
            code.push_str(&format!(
                "_{var}_table = _{var}_sig.table.dropna().assign(cluster=_{var}_sig.cluster({}).to_numpy())\n",
                args.join(", ")
            ));
            let known = ["table", "columns", "k", "method", "components", "n_init", "seed"];
            (
                format!("{}{code}", rest(n, &known)),
                outs(&[("table", keyed(var, &format!("_{var}_table")))]),
                Need::Nothing,
            )
        }
        "metrics.clustergram" => {
            let mut code = signatures_of(n, var, sc)?;
            let lo = setting(n, "k_min", sc)?.unwrap_or_else(|| "1".into());
            let hi = setting(n, "k_max", sc)?.unwrap_or_else(|| "8".into());
            let mut args = vec![format!("range({lo}, {hi} + 1)")];
            args.extend(cluster_args(n, sc)?);
            args.extend(given(n, &[("pca_weighted", "pca_weighted")], sc)?);
            code.push_str(&format!("{var} = _{var}_sig.clustergram({})\n", args.join(", ")));
            let named = |f: &str| {
                keyed(var, &format!("{var}.{f}.rename(columns=lambda k: f\"k{{k}}\").rename_axis(None, axis=1)"))
            };
            let known =
                ["table", "columns", "k_min", "k_max", "method", "components", "n_init", "seed", "pca_weighted"];
            (
                format!("{}{code}", rest(n, &known)),
                outs(&[("labels", named("labels")), ("means", named("means"))]),
                Need::Nothing,
            )
        }
        // -- bio ------------------------------------------------------------------------------------------------------
        "bio.table" => {
            let mut args = vec![need("path", "its path")?];
            let group = listed(n, "group_columns", sc)?.ok_or("give the group columns")?;
            args.push(format!("group_columns={group}"));
            let columns = [
                ("label_columns", "label_columns"),
                ("count_columns", "sample_count_columns"),
                ("cell_sizes", "cell_sizes"),
                ("cell_origins", "cell_origins"),
            ];
            for (ours, theirs) in columns {
                if let Some(x) = listed(n, ours, sc)? {
                    args.push(format!("{theirs}={x}"));
                }
            }
            for (ours, theirs) in [("separator", "sep"), ("quote", "quote")] {
                if let Some(x) = guessed(n, ours, sc)? {
                    args.push(format!("{theirs}={x}"));
                }
            }
            args.extend(given(n, &[("binarise", "binarise_counts"), ("crs", "crs")], sc)?);
            let known = [
                "path",
                "group_columns",
                "label_columns",
                "count_columns",
                "cell_sizes",
                "cell_origins",
                "separator",
                "quote",
                "binarise",
                "crs",
            ];
            let code = format!("{}{var} = tf.bio.BaseData.from_table({})\n", rest(n, &known), args.join(", "));
            (code, one(var, "basedata"), Need::Nothing)
        }
        "bio.vector" => {
            let mut args = vec![
                need("features", "its features")?,
                need("label", "its label")?,
                need("cell_sizes", "its cell sizes")?,
            ];
            if let Some(x) = listed(n, "cell_origins", sc)? {
                args.push(format!("cell_origins={x}"));
            }
            if let Some(x) = listed(n, "count", sc)? {
                args.push(format!("count_columns={x}"));
            }
            match n.params.get("measure").and_then(Value::as_str) {
                None | Some("none") | Some("") => {}
                Some(m) => args.push(format!("measure={}", text(m))),
            }
            args.extend(given(n, &[("binarise", "binarise_counts")], sc)?);
            let known = ["features", "label", "cell_sizes", "cell_origins", "count", "measure", "binarise"];
            let code = format!("{}{var} = tf.bio.BaseData.from_vector({})\n", rest(n, &known), args.join(", "));
            (code, one(var, "basedata"), Need::Nothing)
        }
        "bio.occurrences" => {
            let source = match v("occurrences")? {
                Some(o) => o,
                None => setting(n, "path", sc)?.ok_or("choose the file, or connect the records")?,
            };
            let mut args = vec![source, format!("cell_sizes={}", need("cell_sizes", "its cell sizes")?)];
            let keys = [
                ("cell_origins", "cell_origins"),
                ("to_crs", "to_crs"),
                ("label", "label"),
                ("count", "count"),
                ("max_uncertainty", "max_uncertainty"),
                ("basis_of_record", "basis_of_record"),
                ("exclude_issues", "exclude_issues"),
                ("present_only", "present_only"),
                ("drop_duplicates", "drop_duplicates"),
            ];
            args.extend(given(n, &keys, sc)?);
            let known = [
                "occurrences",
                "path",
                "cell_sizes",
                "cell_origins",
                "to_crs",
                "label",
                "count",
                "max_uncertainty",
                "basis_of_record",
                "exclude_issues",
                "present_only",
                "drop_duplicates",
            ];
            let code = format!("{}{var} = tf.bio.BaseData.from_occurrences({})\n", rest(n, &known), args.join(", "));
            (code, one(var, "basedata"), Need::Nothing)
        }
        "bio.hex" | "bio.h3" => {
            let hex = n.op == "bio.hex";
            let records = need("records", "its records")?;
            let label = need("label", "its label")?;
            let (xn, yn) = if hex { ("x", "y") } else { ("lon", "lat") };
            let mut code = format!("_{var}_d = {records}\n");
            match (setting(n, xn, sc)?, setting(n, yn, sc)?) {
                (Some(x), Some(y)) => code.push_str(&format!("_{var}_xy = ({x}, {y})\n")),
                _ => code.push_str(&format!(
                    "if hasattr(_{var}_d, \"geometry\"):\n    _{var}_d = _{var}_d.assign(_x=_{var}_d.geometry.x, _y=_{var}_d.geometry.y)\n    _{var}_xy = (\"_x\", \"_y\")\nelse:\n    _{var}_xy = ({}, {})\n",
                    text(xn),
                    text(yn)
                )),
            }
            let count = given(n, &[("count", "count_column")], sc)?;
            let call = if hex {
                let mut a = vec![
                    format!("_{var}_d"),
                    format!("_{var}_xy[0]"),
                    format!("_{var}_xy[1]"),
                    label,
                    need("size", "its size")?,
                ];
                a.extend(given(n, &[("orientation", "orientation"), ("origin", "origin")], sc)?);
                a.extend(count);
                a.push(format!("crs=getattr(_{var}_d, \"crs\", None)"));
                format!("tf.bio.BaseData.from_hex({})", a.join(", "))
            } else {
                // (the library takes the latitude first)
                let mut a = vec![
                    format!("_{var}_d"),
                    format!("_{var}_xy[1]"),
                    format!("_{var}_xy[0]"),
                    label,
                    need("resolution", "its resolution")?,
                ];
                a.extend(count);
                format!("tf.bio.BaseData.from_h3({})", a.join(", "))
            };
            let known = ["records", "label", "size", "resolution", "orientation", "origin", "count", xn, yn];
            (format!("{}{code}{var} = {call}\n", rest(n, &known)), one(var, "basedata"), Need::Nothing)
        }
        "bio.raster" => {
            let r = need("raster", "its raster")?;
            if n.params.get("labels_as_bands") == Some(&Value::Bool(false)) {
                return Err("its labels_as_bands false (the values as labels): not in the script yet".into());
            }
            if n.params.get("given_label").is_some_and(|g| g.as_str().is_some_and(|s| !s.is_empty())) {
                return Err("its given_label: not in the script yet".into());
            }
            let label = setting(n, "label", sc)?.unwrap_or_else(|| text("layer"));
            let mut args = vec![format!("_{var}_r"), text("band")];
            args.extend(given(
                n,
                &[("cell_sizes", "cell_sizes"), ("cell_origins", "cell_origins"), ("threshold", "threshold")],
                sc,
            )?);
            let known =
                ["raster", "labels_as_bands", "given_label", "label", "cell_sizes", "cell_origins", "threshold"];
            let code = format!(
                "{}_{var}_r = {r}\nif \"band\" not in _{var}_r.dims:\n    _{var}_r = _{var}_r.expand_dims(band=[{label}])\n{var} = tf.bio.BaseData.from_xarray({})\n",
                rest(n, &known),
                args.join(", ")
            );
            (code, one(var, "basedata"), Need::Nothing)
        }
        "bio.tree" => {
            if n.params.get("path").and_then(Value::as_str).is_some_and(|p| p.contains('(') || p.contains(';')) {
                return Err("its path is the Newick text itself: not in the script yet".into());
            }
            let mut args = vec![need("path", "its path")?];
            args.extend(given(n, &[("format", "format"), ("index", "index")], sc)?);
            let code =
                format!("{}{var} = tf.bio.read_tree({})\n", rest(n, &["path", "format", "index"]), args.join(", "));
            (code, one(var, "tree"), Need::Nothing)
        }
        "bio.matrix" => {
            let mut args = vec![need("path", "its path")?];
            for k in ["element_columns", "row_columns", "col_columns"] {
                if let Some(x) = listed(n, k, sc)? {
                    args.push(format!("{k}={x}"));
                }
            }
            args.extend(given(
                n,
                &[
                    ("values_start_col", "values_start_col"),
                    ("sparse", "sparse"),
                    ("value_column", "value_column"),
                    ("self_similarity", "self_similarity"),
                ],
                sc,
            )?);
            for (ours, theirs) in [("separator", "sep"), ("quote", "quote")] {
                if let Some(x) = guessed(n, ours, sc)? {
                    args.push(format!("{theirs}={x}"));
                }
            }
            let known = [
                "path",
                "element_columns",
                "row_columns",
                "col_columns",
                "values_start_col",
                "sparse",
                "value_column",
                "self_similarity",
                "separator",
                "quote",
            ];
            let code = format!("{}{var} = tf.bio.read_matrix({})\n", rest(n, &known), args.join(", "));
            (code, one(var, "matrix"), Need::Nothing)
        }
        "bio.cluster" => {
            let bd = need("basedata", "its basedata")?;
            let mut args = vec![bd.clone()];
            args.extend(given(n, &[("index", "index"), ("linkage", "linkage"), ("conditions", "conditions")], sc)?);
            if let Some(q) = query(n, sc)? {
                args.push(format!("definition_query={q}"));
            }
            args.extend(given(
                n,
                &[
                    ("calculations", "calculations"),
                    ("tie_breaker", "tie_breaker"),
                    ("clear_singletons", "clear_singletons"),
                ],
                sc,
            )?);
            for k in ["tree", "matrix"] {
                if let Some(x) = v(k)? {
                    args.push(format!("{k}={x}"));
                }
            }
            args.push(format!("prng_seed={}", setting(n, "seed", sc)?.unwrap_or_else(|| "1".into())));
            let known = [
                "basedata",
                "index",
                "linkage",
                "conditions",
                "definition_query",
                "calculations",
                "tie_breaker",
                "clear_singletons",
                "tree",
                "matrix",
                "seed",
                "k",
            ];
            let mut code = rest(n, &known);
            code.push_str(&format!("{var} = tf.bio.cluster({})\n", args.join(", ")));
            let mut o = outs(&[("tree", format!("{var}.tree"))]);
            // (the dendrogram's nodes and the calculations' table are the library's own columns: not given)
            if let Some(k) = setting(n, "k", sc)? {
                code.push_str(&format!("_{var}_cut = {var}.cut({k})\n"));
                code.push_str(&group_table(
                    var,
                    &bd,
                    &format!("_{var}_cut.to_frame(\"cluster\")"),
                    &format!("_{var}_clusters"),
                ));
                code.push_str(&format!("_{var}_map = {bd}.to_raster(_{var}_cut) if {bd}.n_axes == 2 else None\n"));
                o.insert("clusters".into(), format!("_{var}_clusters"));
                o.insert("map".into(), format!("_{var}_map"));
            }
            (code, o, Need::Nothing)
        }
        "bio.spatial" => {
            let bd = need("basedata", "its basedata")?;
            let conditions = setting(n, "conditions", sc)?.ok_or("give the conditions")?;
            let calculations = setting(n, "calculations", sc)?.ok_or("give the calculations")?;
            let mut args = vec![bd.clone(), conditions, calculations];
            if let Some(q) = query(n, sc)? {
                args.push(format!("definition_query={q}"));
            }
            for k in ["tree", "matrix"] {
                if let Some(x) = v(k)? {
                    args.push(format!("{k}={x}"));
                }
            }
            args.push(format!("prng_seed={}", setting(n, "seed", sc)?.unwrap_or_else(|| "0".into())));
            // (the list indices are the library's long tables, not Terraflux's JSON: `lists` is not given)
            let known = ["basedata", "conditions", "calculations", "definition_query", "tree", "matrix", "seed"];
            let mut code = rest(n, &known);
            code.push_str(&format!("{var} = tf.bio.spatial_analysis({})\n", args.join(", ")));
            code.push_str(&group_table(var, &bd, &format!("{var}.table"), &format!("_{var}_table")));
            code.push_str(&format!("_{var}_maps = _stack({{c: {var}.to_raster(c) for c in {var}.table.columns}}) if {bd}.n_axes == 2 else None\n"));
            (code, outs(&[("table", format!("_{var}_table")), ("maps", format!("_{var}_maps"))]), Need::Flow)
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

    fn script_of(toml: &str) -> Script {
        let m = Model::from_toml(toml).unwrap();
        script(&m, &Registry::standard(), None)
    }

    #[test]
    fn fragmentation_and_metrics_of_a_land_cover() {
        let s = script_of(
            r#"
format = "terraflux/1"

[inputs.land]
kind = "map"
path = "data/land.tif"

[[nodes]]
id = "contagion"
op = "fragmentation.contagion"
inputs = { map = "$land" }
params = { foreground = [1, 2], window = 13 }

[[nodes]]
id = "entropy"
op = "fragmentation.entropy"
inputs = { map = "$land" }
params = { foreground = 1 }

[[nodes]]
id = "mosaic"
op = "fragmentation.landscape_mosaic"
inputs = { map = "$land" }
params = { agriculture = [1, 2], natural = 3, developed = 4, window = 27 }

[[nodes]]
id = "parcels"
op = "landscape.parcellation"
inputs = { map = "$land" }

[[nodes]]
id = "window"
op = "metrics.window"
inputs = { map = "$land" }
params = { metrics = ["shdi", "ed"], window = 9, shape = "circle" }

[[nodes]]
id = "sig"
op = "metrics.signatures"
inputs = { maps = "$land" }
params = { grid = 5000, landscape_metrics = ["shdi", "ed"], class_metrics = "pland" }

[[nodes]]
id = "pca"
op = "metrics.pca"
inputs = { table = "sig.table" }
params = { components = 2 }

[[nodes]]
id = "groups"
op = "metrics.cluster"
inputs = { table = "sig.table" }
params = { k = 3 }
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("contagion = tf.contagion_pff(tf.reclassify(land, foreground=[1, 2]), window_size=13)"),
            "{t}"
        );
        assert!(t.contains("entropy = tf.spatial_entropy(tf.reclassify(land, foreground=1))"), "{t}");
        assert!(
            t.contains("np.where(np.isin(land.values, [1, 2]), 1, np.where(np.isin(land.values, [3]), 2, np.where(np.isin(land.values, [4]), 3, 0)))"),
            "{t}"
        );
        assert!(t.contains("mosaic = tf.landscape_mosaic(_mosaic_in, window_size=27, output_classes=19)"), "{t}");
        assert!(t.contains("parcels = tf.parcellation(land.fillna(0), return_labels=True)"), "{t}");
        assert!(
            t.contains("window = _stack(tf.window_metrics(land, np.hypot(*np.mgrid[-(9 // 2):9 // 2 + 1, -(9 // 2):9 // 2 + 1]) <= 9 // 2, level=\"landscape\", metric=[\"shdi\", \"ed\"]))"),
            "{t}"
        );
        assert!(t.contains("sig = tf.SpatialSignatureAnalysis(_sig_layers, landscape_metrics=[\"shdi\", \"ed\"], class_metrics=\"pland\", grid=5000)"), "{t}");
        assert!(t.contains("_pca_sig = tf.SpatialSignatureAnalysis.from_table(_pca_in)"), "{t}");
        assert!(t.contains("pca = _pca_sig.pca(2)"), "{t}");
        assert!(t.contains("_groups_sig.cluster(3)"), "{t}");
        assert!(t.contains("def _stack(layers):"), "{t}");
        parses(t);
    }

    #[test]
    fn an_accuracy_assessment_and_a_few_rasters() {
        let s = script_of(
            r#"
format = "terraflux/1"

[inputs.map]
kind = "map"
path = "data/classes.tif"

[[nodes]]
id = "reference"
op = "vector.read"
params = { path = "data/reference.gpkg" }

[[nodes]]
id = "size"
op = "accuracy.sample_size"
inputs = { map = "$map" }
params = { user_accuracy = { "1" = 0.7, "2" = 0.9 }, minimum = 30 }

[[nodes]]
id = "sample"
op = "accuracy.sample"
inputs = { map = "$map", sizes = "size.table" }
params = { seed = 7 }

[[nodes]]
id = "assess"
op = "accuracy.assess"
inputs = { map = "$map", points = "reference.vector" }
params = { confidence = 0.9 }

[[nodes]]
id = "clip"
op = "raster.clip"
inputs = { raster = "$map" }
params = { extent = [0, 0, 1000, 1000] }

[[nodes]]
id = "reproject"
op = "raster.reproject"
inputs = { raster = "clip.raster" }
params = { crs = "EPSG:4326", resampling = "near" }

[[nodes]]
id = "zonal"
op = "raster.zonal"
inputs = { map = "reproject.raster", zones = "sample.cells" }
params = { stats = ["mean", "max"] }

[[nodes]]
id = "tab"
op = "raster.tabulate"
inputs = { zones = "sample.cells", classes = "$map" }
params = { units = "hectares" }
"#,
        );
        let t = &s.text;
        assert!(s.left.is_empty(), "{:?}\n{t}", s.left);
        assert!(
            t.contains("size = tf.accuracy.sample_size(map_, user_accuracy={\"1\": 0.7, \"2\": 0.9}, minimum=30)"),
            "{t}"
        );
        assert!(t.contains("sample = tf.accuracy.sample(map_, sizes=size.table, seed=7)"), "{t}");
        assert!(t.contains("assess = tf.accuracy.assess(map_, points=reference, confidence=0.9)"), "{t}");
        assert!(t.contains("reproject = tf.reproject(clip, \"EPSG:4326\", resampling=\"nearest\")"), "{t}");
        assert!(t.contains("zonal = tf.zonal_stats(reproject, _sample_c, stats=[\"mean\", \"max\"])"), "{t}");
        assert!(t.contains("tab = tf.tabulate_area(_sample_c, classes=map_, units=\"hectares\")"), "{t}");
        parses(t);
    }

    #[test]
    fn biodiversity_statistics_and_uncertainty() {
        let s = script_of(
            r#"
format = "terraflux/1"

[[nodes]]
id = "t"
op = "bio.table"
params = { path = "data/d.csv", group_columns = ["x", "y"], label_columns = "sp", cell_sizes = [1, 1], separator = "tab" }

[[nodes]]
id = "tree"
op = "bio.tree"
params = { path = "data/t.nwk" }

[[nodes]]
id = "sp"
op = "bio.spatial"
inputs = { basedata = "t.basedata", tree = "tree.tree" }
params = { conditions = "sp_circle (radius => 2)", calculations = ["richness", "pd"] }

[[nodes]]
id = "cl"
op = "bio.cluster"
inputs = { basedata = "t.basedata" }
params = { k = 3 }

[[nodes]]
id = "pts"
op = "vector.read"
params = { path = "data/s.gpkg" }

[[nodes]]
id = "like"
op = "nlm.random"
params = { rows = 10, columns = 10 }

[[nodes]]
id = "v"
op = "stats.variogram"
inputs = { points = "pts.vector" }
params = { field = "z", model = "exponential" }

[[nodes]]
id = "k"
op = "stats.kriging"
inputs = { points = "pts.vector", like = "like.map", model = "v.model" }
params = { field = "z" }

[[nodes]]
id = "mc"
op = "uncertainty.monte_carlo"
inputs = { carbon = "like.map" }
params = { expression = "carbon * f", parameters = [["f", 0.5, 0.1]], n = 50, seed = 3 }

[[nodes]]
id = "dl"
op = "uncertainty.delta"
params = { expression = "a * b", parameters = [["a", 2.0, 0.1], ["b", 3.0, 0.2]] }

[[nodes]]
id = "iv"
op = "uncertainty.interval"
inputs = { value = "dl.value", sd = "dl.sd" }

[[nodes]]
id = "bin"
op = "nlm.binarize"
inputs = { map = "like.map" }
params = { breaks = [0.2, 0.5] }

[[nodes]]
id = "dd"
op = "data.download"
inputs = { like = "like.map" }
params = { source = "worldclim", layers = [1, 12] }

[[nodes]]
id = "design"
op = "sensitivity.design"
params = { method = "morris", factors = [["a", 0, 1], ["b", 0, 1]], n = 2 }

[[nodes]]
id = "an"
op = "sensitivity.analyze"
inputs = { design = "design.table", results = "design.table" }
params = { method = "morris", column = "a" }

[[nodes]]
id = "vul"
op = "redd.vulnerability"
inputs = { distance = "like.map", nrt = 90 }

[[nodes]]
id = "fit"
op = "redd.fit"
inputs = { classes = "vul.classes", deforestation = "like.map" }

[[nodes]]
id = "al"
op = "redd.allocate"
inputs = { classes = "vul.classes", table = "fit.table", expected = 100 }

[[nodes]]
id = "assess"
op = "redd.assess"
inputs = { density = "al.density", deforestation = "like.map" }
params = { grid_area = 100 }
"#,
        );
        let t = &s.text;
        assert!(
            s.left.is_empty(),
            "{:?}
{t}",
            s.left
        );
        assert!(
            t.contains("t = tf.bio.BaseData.from_table(MODEL_DIR / \"data/d.csv\", group_columns=[\"x\", \"y\"], label_columns=[\"sp\"], cell_sizes=[1, 1], sep=\"\\t\")"),
            "{t}"
        );
        assert!(t.contains("tree = tf.bio.read_tree(MODEL_DIR / \"data/t.nwk\")"), "{t}");
        assert!(t.contains("sp = tf.bio.spatial_analysis(t, \"sp_circle (radius => 2)\", [\"richness\", \"pd\"], tree=tree, prng_seed=0)"), "{t}");
        assert!(t.contains("cl = tf.bio.cluster(t, prng_seed=1)") && t.contains("_cl_cut = cl.cut(3)"), "{t}");
        assert!(t.contains("v = tf.stats.variogram(pts, field=\"z\", model=\"exponential\")"), "{t}");
        assert!(t.contains("k = tf.stats.kriging(pts, like, v.model, field=\"z\")"), "{t}");
        assert!(
            t.contains("def _mc_f(p):") && t.contains("_calc(\"carbon * f\", carbon=like, f=p[0])[\"map\"]"),
            "{t}"
        );
        assert!(t.contains("mc = tf.uncertainty.monte_carlo(_mc_f, [0.5], [0.1], n=50, seed=3)"), "{t}");
        assert!(t.contains("dl = tf.uncertainty.delta_method(_dl_f, [2.0, 3.0], [0.1, 0.2])"), "{t}");
        assert!(t.contains("iv = tf.uncertainty.interval(dl.value, sd=dl.sd)"), "{t}");
        assert!(t.contains("bin = tf.nlm.binarize(like, [0.2, 0.5])"), "{t}");
        assert!(t.contains("dd = tf.download.worldclim(like=like, layers=[1, 12])"), "{t}");
        assert!(t.contains("an = tf.sensitivity.analyze(design, design, column=\"a\", method=\"morris\")"), "{t}");
        assert!(t.contains("tf.redd.benchmark("), "{t}");
        assert!(t.contains("tf.redd.fit(") && t.contains("tf.redd.allocate(") && t.contains("tf.redd.assess("), "{t}");
        assert!(t.contains("def _calc(script, **inputs):"), "{t}");
        parses(t);
    }
}
