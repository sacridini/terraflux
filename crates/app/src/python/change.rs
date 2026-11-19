//! The Python counterparts of the operators of land change (change, validate, woe, markov, demand, dynamic, futures, molusce, clumondo, patches, plus).

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[
    Py {
        op: "change.crosstab",
        call: "tf.simulate.crosstab",
        args: &[("from", "from_map"), ("to", "to_map")],
        defaults: &[],
        // (a row per class, its class in the column `from`, the others named by their class)
        outs: &[("table", ".rename_axis(index=\"from\", columns=None).reset_index().rename(columns=str)")],
        kind: Kind::Plain,
    },
    Py {
        op: "change.composite",
        call: "tf.simulate.composite_matrix",
        args: &[("comp", "comp"), ("ref", "ref"), ("factor", "factor")],
        defaults: &[],
        outs: &[("table", ".rename_axis(index=\"comp\", columns=None).reset_index().rename(columns=str)")],
        kind: Kind::Plain,
    },
    Py {
        op: "change.memberships",
        call: "tf.simulate.memberships",
        args: &[("map", ""), ("factor", "factor")],
        defaults: &[],
        outs: &[("stack", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "change.resolutions",
        call: "tf.simulate.difference_resolutions",
        args: &[
            ("comp", "comp"),
            ("ref", "ref"),
            ("factor", "factor"),
            ("percent", "percent"),
            ("population", "population"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "change.mad",
        call: "tf.simulate.mad",
        args: &[("grid1", "grid1"), ("grid2", "grid2"), ("strata", "strata"), ("multiple", "multiple")],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "validate.similarity",
        call: "tf.simulate.similarity_of_differences",
        args: &[
            ("start", "t0"),
            ("observed", "observed"),
            ("simulated", "simulated"),
            ("windows", "windows"),
            ("decay", "decay"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "validate.error_budget",
        call: "tf.simulate.error_budget",
        args: &[
            ("reference", "reference"),
            ("simulated", "simulated"),
            ("iterations", "iterations"),
            ("scale", "scale"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "validate.futures_grid",
        call: "tf.simulate.futures_validation_grid",
        args: &[
            ("reference", "reference"),
            ("simulated", "simulated"),
            ("original", "original"),
            ("regions", "regions"),
            ("size", "size"),
        ],
        defaults: &[],
        outs: &[("table", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "futures.devpressure",
        call: "tf.simulate.development_pressure",
        args: &[
            ("developed", ""),
            ("size", "size"),
            ("method", "method"),
            ("gamma", "gamma"),
            ("scaling_factor", "scaling_factor"),
            ("pad", "pad"),
        ],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "futures.patch_library",
        call: "tf.simulate.patch_library",
        args: &[
            ("start", "start"),
            ("end", "end"),
            ("subregions", "subregions"),
            ("per_subregion", "per_subregion"),
            ("threshold", "threshold"),
        ],
        defaults: &[],
        outs: &[("library", "")],
        kind: Kind::Plain,
    },
];

/// An input or setting of the node, as an error when it is not given.
fn must(n: &Node, name: &str, sc: &Scope) -> Result<String, String> {
    value_of(n, name, sc)?.ok_or_else(|| format!("its {name} is not connected"))
}

/// The outputs: a port each with its expression.
fn outs_of(list: &[(&str, String)]) -> HashMap<String, String> {
    list.iter().map(|(p, e)| (p.to_string(), e.clone())).collect()
}

/// `tf.simulate.changes(maps, times=..., cell_area=...)` of a node of a series of maps.
fn changes_call(n: &Node, sc: &Scope) -> Result<String, String> {
    let mut args = vec![must(n, "maps", sc)?];
    args.extend(given(n, &[("times", "times"), ("cell_area", "cell_area")], sc)?);
    Ok(format!("tf.simulate.changes({})", args.join(", ")))
}

/// A map in GuidosToolbox's coding (0 missing, 1 background, 2 foreground): the classes given as the foreground,
/// else the map as it is.
fn gtb_map(n: &Node, port: &str, sc: &Scope) -> Result<String, String> {
    let v = must(n, port, sc)?;
    Ok(match value_of(n, "foreground", sc)? {
        Some(fg) => format!("tf.reclassify({v}, foreground={fg})"),
        None => format!("{v}.fillna(0).astype(\"uint8\")"),
    })
}

/// A land cover as the landscape mosaic takes it (0 missing, 1 agriculture, 2 natural, 3 developed): the classes
/// of each given, the others missing, else the map as it is.
fn mosaic_map(n: &Node, port: &str, sc: &Scope) -> Result<String, String> {
    let v = must(n, port, sc)?;
    let mut conds = Vec::new();
    let mut codes = Vec::new();
    for (name, code) in [("agriculture", 1), ("natural", 2), ("developed", 3)] {
        if let Some(c) = value_of(n, name, sc)? {
            conds.push(format!("np.isin({v}, {c})"));
            codes.push(code.to_string());
        }
    }
    Ok(if conds.is_empty() {
        format!("{v}.fillna(0).astype(\"uint8\")")
    } else {
        format!("np.select([{}], [{}], default=0).astype(\"uint8\")", conds.join(", "), codes.join(", "))
    })
}

/// A table of one row from the attributes of a result, by column.
fn row_of(var: &str, cols: &[(&str, &str)]) -> String {
    let items: Vec<String> = cols.iter().map(|(c, a)| format!("\"{c}\": {var}.{a}")).collect();
    format!("_pd.DataFrame([{{{}}}])", items.join(", "))
}

/// A class, a region or a transition's end as Python writes it (1, not 1.0).
fn class_literal(v: &Value) -> String {
    match v.as_f64() {
        Some(c) => class_number(c),
        None => literal(v),
    }
}

/// A setting of the node written in the model (not a `$name`, whose value is not known here).
fn raw<'n>(n: &'n Node, name: &str) -> Result<Option<&'n Value>, String> {
    match n.params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(v) if terraflux_graph::setting_ref(v).is_some() => {
            Err(format!("{name} given by a name: not in the script yet"))
        }
        Some(v) => Ok(Some(v)),
    }
}

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let code = match n.op.as_str() {
        "change.sample_to_population" => {
            // the library's matrix has the classes as its index and columns (numbers), Terraflux's a class column
            let m = must(n, "matrix", sc)?;
            let p = must(n, "population", sc)?;
            let code = format!(
                "{var} = tf.simulate.sample_to_population({m}.set_index({m}.columns[0]).rename(columns=lambda c: int(float(c)) if float(c).is_integer() else float(c)), {p})
"
            );
            let table = format!("{var}.rename_axis(index=\"from\", columns=None).reset_index().rename(columns=str)");
            (code, outs_of(&[("table", table)]), Need::Nothing)
        }
        "markov.matrix" => {
            // the library takes the principal root of `single` per step: a `multi` given cannot be passed
            if raw(n, "multi")?.is_some() || n.params.get("multi").is_some_and(|v| !v.is_null()) {
                return Err("its multi (the probabilities per step): tf.simulate.Markov takes the root of single and has no way to be given them"
                    .into());
            }
            let mut args =
                vec![format!("matrix={}", must(n, "single", sc)?), format!("classes={}", must(n, "classes", sc)?)];
            args.extend(given(n, &[("steps", "steps")], sc)?);
            (
                format!(
                    "{var} = tf.simulate.Markov({})
",
                    args.join(", ")
                ),
                outs_of(&[("markov", var.to_string())]),
                Need::Nothing,
            )
        }
        "change.intensity" => {
            let gain = must(n, "gain", sc)?;
            let loss = must(n, "loss", sc)?;
            let area = value_of(n, "area", sc)?.map(|a| format!(", area={a}")).unwrap_or_default();
            let code = format!("{var} = {}.intensity({gain}, {loss}{area})\n", changes_call(n, sc)?);
            let ports = [
                "interval",
                "category_gain",
                "category_loss",
                "transition_gain",
                "transition_loss",
                "category_gain_stationarity",
                "category_loss_stationarity",
                "transition_gain_stationarity",
                "transition_loss_stationarity",
            ];
            let outs = ports.iter().map(|p| (p.to_string(), format!("{var}.{p}"))).collect();
            (code, outs, Need::Nothing)
        }
        "change.count" => {
            let code = format!("{var} = {}.change_count()\n", changes_call(n, sc)?);
            (code, outs_of(&[("map", format!("{var}[0]")), ("table", format!("{var}[1]"))]), Need::Nothing)
        }
        "change.components" => {
            let matrix = match (value_of(n, "from", sc)?, value_of(n, "to", sc)?) {
                (Some(a), Some(b)) => format!("tf.simulate.crosstab({a}, {b})"),
                _ => format!("{}.matrix()", changes_call(n, sc).map_err(|_| "give from and to, or maps")?),
            };
            let analysis = value_of(n, "analysis", sc)?.unwrap_or_else(|| "\"change\"".into());
            let outs = outs_of(&[
                ("table", format!("tf.simulate.difference_components({var}, analysis={analysis})")),
                ("overall", format!("_pd.DataFrame([tf.simulate.overall_difference({var})])")),
            ]);
            (format!("{var} = {matrix}\n"), outs, Need::Flow)
        }
        "change.simple" => {
            let code =
                format!("{var} = tf.simple_change({}, {})\n", gtb_map(n, "before", sc)?, gtb_map(n, "after", sc)?);
            let stats = row_of(
                var,
                &[
                    ("stable_background", "stable_bg"),
                    ("gain", "gain"),
                    ("loss", "loss"),
                    ("stable_foreground", "stable_fg"),
                    ("missing", "missing"),
                    ("data", "total_valid"),
                ],
            );
            (code, outs_of(&[("map", format!("{var}.change_map")), ("stats", stats)]), Need::Flow)
        }
        "change.morphological" => {
            let code = format!(
                "{var} = tf.morphological_change({}, {})\n",
                gtb_map(n, "before", sc)?,
                gtb_map(n, "after", sc)?
            );
            let stats = row_of(
                var,
                &[
                    ("foreground_before", "fg_area_t1"),
                    ("foreground_after", "fg_area_t2"),
                    ("net_change", "net_change"),
                    ("stable_background", "stable_bg"),
                    ("stable_foreground", "stable_fg"),
                    ("gain", "mcd_gain"),
                    ("loss", "mcd_loss"),
                    ("spurious", "spurious"),
                    ("missing", "missing"),
                    ("elasticity", "elasticity"),
                ],
            );
            (code, outs_of(&[("map", format!("{var}.change_map")), ("stats", stats)]), Need::Flow)
        }
        "change.fos" => {
            // (a cell without a value is GTB's 102)
            let (a, b) = (must(n, "before", sc)?, must(n, "after", sc)?);
            let code = format!("{var} = tf.fos_change({a}.fillna(102), {b}.fillna(102))\n");
            let stats = row_of(
                var,
                &[
                    ("gain", "fg_gain"),
                    ("loss", "fg_loss"),
                    ("stable_background", "stable_bg"),
                    ("common", "forcom"),
                    ("same_class", "diag_same_class"),
                    ("other_class", "diff_class"),
                    ("decreased_percent", "frag_decrease_pct"),
                    ("increased_percent", "frag_increase_pct"),
                ],
            );
            let outs = outs_of(&[
                ("map", format!("{var}.change_map")),
                ("stats", stats),
                ("classes", format!("{var}.matrix_6x6")),
            ]);
            (code, outs, Need::Flow)
        }
        "change.mosaic" => {
            let mut args = vec![mosaic_map(n, "before", sc)?, mosaic_map(n, "after", sc)?];
            args.extend(given(n, &[("window", "window_size")], sc)?);
            let code = format!("{var} = tf.landscape_mosaic_change({})\n", args.join(", "));
            let outs = outs_of(&[
                ("heatmap", format!("{var}.table")),
                ("triangle", format!("{var}.triangle")),
                ("heatmap_csv", format!("{var}.csv_text()")),
            ]);
            (code, outs, Need::Nothing)
        }
        "change.fad" => {
            return Err("its matrix, stats and report put together the tables of tf.fad_multiscale_change's scales \
                 (one per window) in Terraflux's layout: not in the script yet"
                .into());
        }
        "change.suggest" => {
            return Err("the Python library has no function that suggests the transitions to model".into());
        }
        "validate.agreement" => {
            let args = [must(n, "observed", sc)?, must(n, "simulated", sc)?, must(n, "start", sc)?];
            let code = format!("{var} = tf.simulate.agreement({})\n", args.join(", "));
            let cols = ["overall", "figure_of_merit", "hits", "misses", "false_alarms", "wrong_hits"];
            let items: Vec<String> = cols.iter().map(|c| format!("\"{c}\": {var}[\"{c}\"]")).collect();
            (code, outs_of(&[("table", format!("_pd.DataFrame([{{{}}}])", items.join(", ")))]), Need::Flow)
        }
        "validate.kappa" => {
            let code = format!(
                "{var} = tf.simulate.compare_maps({}, {})\n",
                must(n, "reference", sc)?,
                must(n, "simulated", sc)?
            );
            let cols = ["kappa", "kappa_location", "kappa_histogram", "correctness", "cramer", "jiu", "correlation"];
            let names: Vec<String> = cols.iter().map(|c| format!("\"{c}\"")).collect();
            (code, outs_of(&[("table", format!("_pd.DataFrame([{var}])[[{}]]", names.join(", ")))]), Need::Flow)
        }
        "validate.futures" => {
            let mut args = vec![must(n, "reference", sc)?, must(n, "simulated", sc)?];
            args.extend(value_of(n, "original", sc)?);
            let code = format!("{var} = tf.simulate.futures_validation({})\n", args.join(", "));
            let metrics = format!(
                "_pd.DataFrame([{{k: v for k, v in {var}.items() if k not in (\"classes\", \"quantity\", \"allocation\")}}])"
            );
            let classes = format!(
                "_pd.DataFrame({{\"class\": {var}[\"classes\"], \"quantity\": {var}[\"quantity\"], \"allocation\": {var}[\"allocation\"]}})"
            );
            (code, outs_of(&[("metrics", metrics), ("classes", classes)]), Need::Flow)
        }
        "woe.weights" => {
            return Err(
                "the Python library fits weights of evidence (tf.simulate.weights_of_evidence, from two maps) \
                 and has no way to give fitted weights"
                    .into(),
            );
        }
        "molusce.total" => {
            return Err("the Python library gives the total weights only of a fitted tf.simulate.MolusceWoE, which \
                 has no way to take weights given"
                .into());
        }
        "molusce.step" => {
            return Err("the Python library runs MOLUSCE's simulator only within a whole model: \
                 tf.simulate.presets.molusce(t0, t1, drivers, iterations=...) fits and simulates it"
                .into());
        }
        "clumondo.ages" | "clumondo.step" => {
            return Err("the Python library runs CLUMondo only within a whole model (tf.simulate.CLUMondo, \
                 tf.simulate.presets.clumondo), the ages of its patches inside"
                .into());
        }
        "plus.demand" => {
            return Err("the Python library makes CARS' demand inside tf.simulate.CARS (tf.simulate.presets.plus), \
                 not by a function of its own"
                .into());
        }
        "futures.simulation" => {
            return Err("the Python library runs FUTURES only as a whole model: \
                 tf.simulate.presets.futures(potential, demand, patch_sizes, ...)"
                .into());
        }
        "futures.calibrate" => {
            return Err("tf.simulate.calibrate_patches takes a whole model (tf.simulate.presets.futures) to simulate \
                 each combination with"
                .into());
        }
        "lulcc.history" => {
            return Err(
                "the Python library keeps the history of the classes inside tf.simulate.run's simulation".into()
            );
        }
        "dynamic.distance_to" => {
            if n.inputs.contains_key("cost") {
                return Err(
                    "its cost (a stack of conductances by direction): the library takes a tf.cost Transition".into()
                );
            }
            let mut args = vec![must(n, "classes", sc)?];
            args.extend(given(n, &[("unit", "unit")], sc)?);
            let code = format!("{var} = tf.simulate.distance_to({})({})\n", args.join(", "), must(n, "landscape", sc)?);
            (code, outs_of(&[("distance", var.to_string())]), Need::Nothing)
        }
        "dynamic.share_of" => {
            let mut args = vec![must(n, "classes", sc)?];
            args.extend(given(n, &[("window", "window")], sc)?);
            let code = format!("{var} = tf.simulate.share_of({})({})\n", args.join(", "), must(n, "landscape", sc)?);
            (code, outs_of(&[("share", var.to_string())]), Need::Nothing)
        }
        "demand.regional" => {
            let regions = must(n, "regions", sc)?;
            let ids = raw(n, "ids")?.and_then(Value::as_array).ok_or("its ids are not written in the model")?;
            let demands =
                raw(n, "demands")?.and_then(Value::as_array).ok_or("its demands are not written in the model")?;
            if ids.len() != demands.len() {
                return Err("a demand per id".into());
            }
            let mut parts = Vec::new();
            let mut flow = false;
            for (id, d) in ids.iter().zip(demands) {
                let id = class_literal(id);
                if let Some(t) = d.get("table") {
                    flow = true;
                    parts.push(format!("{id}: _demand_table({})", literal(t)));
                } else if let Some(m) = d.get("markov") {
                    let field = |k: &str| m.get(k).map(literal);
                    let classes = field("classes").ok_or("a chain without its classes")?;
                    let single = field("single").ok_or("a chain without its single")?;
                    let steps = field("steps").map(|s| format!(", steps={s}")).unwrap_or_default();
                    if m.get("multi").is_some_and(|v| !v.is_null()) {
                        return Err(format!(
                            "the multi of region {id}: tf.simulate.Markov takes the root of single and has no way to \
                             be given the probabilities per step"
                        ));
                    }
                    parts.push(format!("{id}: tf.simulate.Markov(matrix={single}, classes={classes}{steps})"));
                } else {
                    return Err(format!("the demand of region {id}: a table or a markov"));
                }
            }
            let code = format!("{var} = tf.simulate.Regional({regions}, {{\n    {},\n}})\n", parts.join(",\n    "));
            let need = if flow { Need::Flow } else { Need::Nothing };
            (code, outs_of(&[("regional", var.to_string()), ("steps", format!("{var}.steps"))]), need)
        }
        "patches.observe" => {
            let (t0, t1) = (must(n, "t0", sc)?, must(n, "t1", sc)?);
            let markov = must(n, "markov", sc)?;
            let iso = value_of(n, "isometry", sc)?.unwrap_or_else(|| "1.5".into());
            let obs = format!("_{var}_obs");
            let tr = format!("_{var}_tr");
            let rates = format!("_{var}_rates");
            let share = match value_of(n, "expansion", sc)? {
                Some(e) => format!("float({e})"),
                None => format!("{obs}.loc[t, \"expansion\"]"),
            };
            // the transitions with a rate per step, rows then columns, as Dinamica lists them
            let mut code = format!("{rates} = {markov}.multi\n");
            code.push_str(&format!(
                "{tr} = [(f, t) for f in {rates}.index for t in {rates}.columns if f != t and {rates}.loc[f, t] > 0]\n"
            ));
            code.push_str(&format!(
                "{obs} = tf.simulate.presets.observed_patches({t0}, {t1}, {tr}).set_index([\"from\", \"to\"])\n"
            ));
            code.push_str(&format!(
                "{var} = tf.simulate.ExpanderPatcher(\n    expansion={{t: {share} for t in {tr}}},\n    \
                 expander={{t: ({obs}.loc[t, \"expansion_mean\"], {obs}.loc[t, \"expansion_variance\"], {iso}) for t in {tr}}},\n    \
                 patcher={{t: ({obs}.loc[t, \"new_mean\"], {obs}.loc[t, \"new_variance\"], {iso}) for t in {tr}}},\n)\n"
            ));
            (code, outs_of(&[("patches", var.to_string())]), Need::Nothing)
        }
        "patches.settings" => {
            let list = raw(n, "transitions")?
                .and_then(Value::as_array)
                .ok_or("its transitions are not written in the model")?;
            let (mut expansion, mut expander, mut patcher) = (Vec::new(), Vec::new(), Vec::new());
            for t in list {
                let class =
                    |k: &str| t.get(k).map(class_literal).ok_or_else(|| format!("a transition without its {k}"));
                let key = format!("({}, {})", class("from")?, class("to")?);
                let triple = |k: &str| match t.get(k).and_then(Value::as_array) {
                    Some(a) if a.len() == 3 => {
                        Ok(format!("({})", a.iter().map(literal).collect::<Vec<_>>().join(", ")))
                    }
                    _ => Err(format!("transition {key}: {k} is [mean, variance, isometry]")),
                };
                let share = t.get("expansion").map(literal).ok_or("a transition without its expansion")?;
                expansion.push(format!("{key}: {share}"));
                expander.push(format!("{key}: {}", triple("expander")?));
                patcher.push(format!("{key}: {}", triple("patcher")?));
            }
            let code = format!(
                "{var} = tf.simulate.ExpanderPatcher(\n    expansion={{{}}},\n    expander={{{}}},\n    patcher={{{}}},\n)\n",
                expansion.join(", "),
                expander.join(", "),
                patcher.join(", ")
            );
            (code, outs_of(&[("patches", var.to_string())]), Need::Nothing)
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

    #[test]
    fn change_analysis_of_two_maps() {
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
id = "series"
op = "stack.of"
inputs = { t0 = "a.map", t1 = "b.map" }

[[nodes]]
id = "cross"
op = "change.crosstab"
inputs = { from = "a.map", to = "b.map" }

[[nodes]]
id = "parts"
op = "change.components"
inputs = { from = "a.map", to = "b.map" }

[[nodes]]
id = "rates"
op = "change.intensity"
inputs = { maps = "series.stack" }
params = { times = [2000, 2010], gain = 1, loss = 2 }

[[nodes]]
id = "counted"
op = "change.count"
inputs = { maps = "series.stack" }

[[nodes]]
id = "chain"
op = "markov.matrix"
params = { classes = [1, 2], single = [[0.9, 0.1], [0.2, 0.8]], steps = 10 }

[[nodes]]
id = "sample"
op = "change.sample_to_population"
inputs = { matrix = "cross.table", population = "cross.table" }

[[nodes]]
id = "seen"
op = "dynamic.share_of"
inputs = { landscape = "a.map" }
params = { classes = [1, 2], window = 5 }

[[nodes]]
id = "near"
op = "dynamic.distance_to"
inputs = { landscape = "a.map" }
params = { classes = 3 }

[[nodes]]
id = "pressure"
op = "futures.devpressure"
inputs = { developed = "a.map" }
params = { size = 5 }

[[nodes]]
id = "library"
op = "futures.patch_library"
inputs = { start = "a.map", end = "b.map" }

[[nodes]]
id = "obs"
op = "patches.observe"
inputs = { t0 = "a.map", t1 = "b.map", markov = "chain.markov" }
params = { isometry = 1.2 }

[[nodes]]
id = "given"
op = "patches.settings"
params = { transitions = [{ from = 1, to = 2, expansion = 0.5, expander = [4, 2, 1.5], patcher = [3, 1, 1.5] }] }

[[nodes]]
id = "zones"
op = "demand.regional"
inputs = { regions = "a.map" }
params = { ids = [1, 2], demands = [{ table = { classes = [1, 2], rows = [[5, 5], [4, 6]] } }, { markov = { classes = [1, 2], single = [[0.9, 0.1], [0.2, 0.8]], steps = 5 } }] }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        assert!(s.left.is_empty(), "{:?}", s.left);
        let t = &s.text;
        assert!(t.contains("cross = tf.simulate.crosstab(from_map=a, to_map=b)\n"), "{t}");
        assert!(t.contains("parts = tf.simulate.crosstab(a, b)\n"), "{t}");
        assert!(t.contains("rates = tf.simulate.changes(series, times=[2000, 2010]).intensity(1, 2)\n"), "{t}");
        assert!(t.contains("counted = tf.simulate.changes(series).change_count()\n"), "{t}");
        assert!(
            t.contains("chain = tf.simulate.Markov(matrix=[[0.9, 0.1], [0.2, 0.8]], classes=[1, 2], steps=10)\n"),
            "{t}"
        );
        assert!(t.contains("seen = tf.simulate.share_of([1, 2], window=5)(a)\n"), "{t}");
        assert!(t.contains("near = tf.simulate.distance_to(3)(a)\n"), "{t}");
        assert!(t.contains("pressure = tf.simulate.development_pressure(a, size=5)\n"), "{t}");
        assert!(t.contains("library = tf.simulate.patch_library(start=a, end=b)\n"), "{t}");
        assert!(t.contains("_obs_rates = chain.multi\n"), "{t}");
        assert!(t.contains("tf.simulate.presets.observed_patches(a, b, _obs_tr)"), "{t}");
        assert!(t.contains("given = tf.simulate.ExpanderPatcher(\n    expansion={(1, 2): 0.5},"), "{t}");
        assert!(
            t.contains(
                "zones = tf.simulate.Regional(a, {\n    1: _demand_table({\"classes\": [1, 2], \"rows\": [[5, 5], [4, 6]]}),\n    2: tf.simulate.Markov("
            ),
            "{t}"
        );
        parses(t);
    }

    #[test]
    fn validation_of_a_simulation() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "start"
op = "nlm.random"

[[nodes]]
id = "observed"
op = "nlm.random"

[[nodes]]
id = "simulated"
op = "nlm.random"

[[nodes]]
id = "fuzzy"
op = "validate.similarity"
inputs = { start = "start.map", observed = "observed.map", simulated = "simulated.map" }
params = { windows = [1, 3, 5] }

[[nodes]]
id = "agree"
op = "validate.agreement"
inputs = { start = "start.map", observed = "observed.map", simulated = "simulated.map" }

[[nodes]]
id = "budget"
op = "validate.error_budget"
inputs = { reference = "observed.map", simulated = "simulated.map" }
params = { iterations = 4 }

[[nodes]]
id = "kappa"
op = "validate.kappa"
inputs = { reference = "observed.map", simulated = "simulated.map" }

[[nodes]]
id = "futures"
op = "validate.futures"
inputs = { reference = "observed.map", simulated = "simulated.map", original = "start.map" }

[[nodes]]
id = "gain"
op = "change.simple"
inputs = { before = "start.map", after = "observed.map" }
params = { foreground = [1, 2] }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        assert!(s.left.is_empty(), "{:?}", s.left);
        let t = &s.text;
        assert!(
            t.contains("fuzzy = tf.simulate.similarity_of_differences(t0=start, observed=observed, simulated=simulated, windows=[1, 3, 5])\n"),
            "{t}"
        );
        assert!(t.contains("agree = tf.simulate.agreement(observed, simulated, start)\n"), "{t}");
        assert!(
            t.contains("budget = tf.simulate.error_budget(reference=observed, simulated=simulated, iterations=4)\n"),
            "{t}"
        );
        assert!(t.contains("kappa = tf.simulate.compare_maps(observed, simulated)\n"), "{t}");
        assert!(t.contains("futures = tf.simulate.futures_validation(observed, simulated, start)\n"), "{t}");
        assert!(
            t.contains(
                "gain = tf.simple_change(tf.reclassify(start, foreground=[1, 2]), tf.reclassify(observed, foreground=[1, 2]))\n"
            ),
            "{t}"
        );
        parses(t);
    }

    #[test]
    fn what_the_library_cannot_be_given_stays_a_comment() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "fitted"
op = "woe.weights"
params = { weights = { transitions = [], entries = [], rows = [], fix_abnormal = false } }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        assert_eq!(s.left.len(), 1, "{:?}", s.left);
        assert!(s.left[0].2.contains("no way to give fitted weights"), "{:?}", s.left);
    }
}
