//! The Python counterparts of operators the library has no function for, written into the script as helpers
//! (python_tools.py) that do what Terraflux does: the statistics of maps and stacks (raster.stats, stack.reduce,
//! raster.correlation, raster.standardize), their windows and patches (raster.focal, majority, sieve, label, expand,
//! shrink), raster.classify (the breaks but Jenks's), raster.terrain (gdaldem's, as GDAL 3.12 computes it), a map's
//! legend, a stack's layers (stack.classes, extract_layer, set), the checks (a Python `AssertionError`, or a warning
//! with `warn`), value.calc, numpy's generator (rng.numpy), batch.run (the library's `tf.batch`) and the agents (a
//! table of attributes).
//!
//! Left without a counterpart: raster.fill (gdal_fillnodata's search and weights are GDAL's), raster.contour
//! (gdal_contour's tracing of the lines), Jenks's breaks of raster.classify (the library's own routine is not in its
//! functions), rng.r (R's generator is the library's private `_core.RRandom`) and plugin.wasm.

use super::*;

fn outs<const N: usize>(ports: [(&str, String); N]) -> HashMap<String, String> {
    ports.into_iter().map(|(p, e)| (p.to_string(), e)).collect()
}

/// An input that must be connected.
fn must(n: &Node, name: &str, sc: &Scope) -> Result<String, String> {
    value_of(n, name, sc)?.ok_or_else(|| format!("its {name} is not connected"))
}

/// The inputs and settings given among `names` (ours, theirs), as `theirs=value`.
fn kwargs(n: &Node, names: &[(&str, &str)], sc: &Scope) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for (ours, theirs) in names {
        if let Some(v) = value_of(n, ours, sc)? {
            out.push(format!("{theirs}={v}"));
        }
    }
    Ok(out)
}

/// A setting that is a literal text, not one given by a name (`$name`): None when it is absent or by a name.
fn literal_text<'n>(n: &'n Node, name: &str) -> Option<&'n str> {
    n.params
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| terraflux_graph::setting_ref(&Value::String(s.to_string())).is_none())
}

/// The call `helper(args)` of a node, its outputs the result or its parts (`[0]`, `[1]`...).
fn call(var: &str, helper: &str, args: Vec<String>, notes: String, outs: HashMap<String, String>) -> Code {
    (format!("{notes}{var} = {helper}({})\n", args.join(", ")), outs, Need::Tools)
}

/// The operators of statistics, windows and patches with one map in (and `known` the names of what they read).
fn windowed(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let map = |k: &str| must(n, k, sc);
    let one = |helper: &str, first: &str, known: &[&str], named: &[(&str, &str)]| -> Result<Option<Code>, String> {
        refuse_others(n, known)?;
        let mut args = vec![map(first)?];
        args.extend(kwargs(n, named, sc)?);
        Ok(Some(call(var, helper, args, String::new(), outs([("map", var.to_string())]))))
    };
    match n.op.as_str() {
        "raster.focal" => one(
            "_raster_focal",
            "map",
            &["map", "stat", "shape", "radius", "inner", "sigma", "fill"],
            &[
                ("stat", "stat"),
                ("shape", "shape"),
                ("radius", "radius"),
                ("inner", "inner"),
                ("sigma", "sigma"),
                ("fill", "fill"),
            ],
        ),
        "raster.majority" => one(
            "_raster_majority",
            "map",
            &["map", "neighbours", "radius", "replace", "iterations"],
            &[("neighbours", "neighbours"), ("radius", "radius"), ("replace", "replace"), ("iterations", "iterations")],
        ),
        "raster.sieve" => {
            if !n.params.contains_key("threshold") && !n.inputs.contains_key("threshold") {
                return Err("its threshold is not given".into());
            }
            one(
                "_raster_sieve",
                "map",
                &["map", "threshold", "neighbours"],
                &[("threshold", "threshold"), ("neighbours", "neighbours")],
            )
        }
        _ => Ok(None),
    }
}

/// The operators of the stack's layers and of a map's classes.
fn layers_and_classes(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    Ok(Some(match n.op.as_str() {
        "raster.stats" => {
            refuse_others(n, &["map", "stats", "bins", "range"])?;
            let mut args = vec![must(n, "map", sc)?];
            args.extend(kwargs(n, &[("stats", "stats"), ("bins", "bins"), ("range", "range")], sc)?);
            call(
                var,
                "_raster_stats",
                args,
                String::new(),
                outs([("table", format!("{var}[0]")), ("histogram", format!("{var}[1]"))]),
            )
        }
        "stack.reduce" => {
            refuse_others(n, &["maps", "stat"])?;
            let mut args = vec![must(n, "maps", sc)?];
            args.extend(kwargs(n, &[("stat", "stat")], sc)?);
            call(var, "_stack_reduce", args, String::new(), outs([("map", var.to_string())]))
        }
        "raster.correlation" => {
            refuse_others(n, &["x", "y", "method"])?;
            let mut args = vec![must(n, "x", sc)?];
            args.extend(kwargs(n, &[("y", "y"), ("method", "method")], sc)?);
            let o = outs([
                ("matrix", format!("{var}[0]")),
                ("covariance", format!("{var}[1]")),
                ("pairs", format!("{var}[2]")),
            ]);
            call(var, "_raster_correlation", args, String::new(), o)
        }
        "raster.standardize" => {
            refuse_others(n, &["map", "method", "low", "high"])?;
            let mut args = vec![must(n, "map", sc)?];
            args.extend(kwargs(n, &[("method", "method"), ("low", "low"), ("high", "high")], sc)?);
            call(
                var,
                "_raster_standardize",
                args,
                String::new(),
                outs([("map", format!("{var}[0]")), ("table", format!("{var}[1]"))]),
            )
        }
        "raster.label" => {
            refuse_others(n, &["map", "neighbours", "classes"])?;
            let mut args = vec![must(n, "map", sc)?];
            args.extend(kwargs(n, &[("neighbours", "neighbours"), ("classes", "classes")], sc)?);
            call(
                var,
                "_raster_label",
                args,
                String::new(),
                outs([("map", format!("{var}[0]")), ("table", format!("{var}[1]"))]),
            )
        }
        "raster.expand" | "raster.shrink" => {
            refuse_others(n, &["map", "classes", "cells", "neighbours", "into_missing"])?;
            let classes = value_of(n, "classes", sc)?.ok_or("choose the classes")?;
            let grow = if n.op == "raster.expand" { "True" } else { "False" };
            let mut args = vec![must(n, "map", sc)?, classes, grow.to_string()];
            let mut named = vec![("cells", "cells"), ("neighbours", "neighbours")];
            if n.op == "raster.expand" {
                named.push(("into_missing", "into_missing"));
            }
            args.extend(kwargs(n, &named, sc)?);
            call(var, "_grow", args, String::new(), outs([("map", var.to_string())]))
        }
        "raster.classify" => {
            refuse_others(n, &["map", "method", "n", "breaks", "sd_width", "sample"])?;
            if literal_text(n, "method") == Some("jenks") {
                return Err(
                    "method jenks: not in the script yet (the natural breaks are computed by Terraflux's own routine)"
                        .into(),
                );
            }
            let mut args = vec![must(n, "map", sc)?];
            args.extend(kwargs(
                n,
                &[("method", "method"), ("n", "n"), ("breaks", "breaks"), ("sd_width", "sd_width")],
                sc,
            )?);
            let notes = if n.params.contains_key("sample") {
                "# (not passed: sample, only the natural breaks sample the values)\n".to_string()
            } else {
                String::new()
            };
            call(
                var,
                "_raster_classify",
                args,
                notes,
                outs([("map", format!("{var}[0]")), ("table", format!("{var}[1]"))]),
            )
        }
        "raster.terrain" => {
            refuse_others(n, &["raster", "measure", "options", "path"])?;
            if let Some(o) = n.params.get("options").and_then(Value::as_str).filter(|o| !o.trim().is_empty()) {
                return Err(format!("its option {o:?} (gdaldem's): not in the script yet"));
            }
            if n.params.get("options").is_some_and(|o| !o.is_string() && !o.is_null()) {
                return Err("its options given by a name: not in the script yet".into());
            }
            let mut args = vec![must(n, "raster", sc)?];
            args.extend(kwargs(n, &[("measure", "measure")], sc)?);
            let notes = n.params.get("path").map_or(String::new(), |p| {
                format!("# (not passed: path = {}; the measure is not written)\n", literal(p))
            });
            call(var, "_raster_terrain", args, notes, outs([("raster", var.to_string())]))
        }
        "raster.legend" => {
            refuse_others(n, &["map", "classes"])?;
            let classes = value_of(n, "classes", sc)?.ok_or("its classes are not given")?;
            call(
                var,
                "_raster_legend",
                vec![must(n, "map", sc)?, classes],
                String::new(),
                outs([("map", var.to_string())]),
            )
        }
        "stack.classes" => {
            refuse_others(n, &["stack"])?;
            call(var, "_stack_classes", vec![must(n, "stack", sc)?], String::new(), outs([("stack", var.to_string())]))
        }
        "stack.extract_layer" => {
            refuse_others(n, &["stack", "layer"])?;
            let layer = value_of(n, "layer", sc)?.ok_or("give the layer's name or number")?;
            call(
                var,
                "_stack_layer",
                vec![must(n, "stack", sc)?, layer],
                String::new(),
                outs([("map", var.to_string())]),
            )
        }
        "stack.set" => {
            refuse_others(n, &["stack", "map", "name"])?;
            let name = value_of(n, "name", sc)?.ok_or("give the layer's name")?;
            let args = vec![must(n, "stack", sc)?, must(n, "map", sc)?, name];
            call(var, "_stack_set", args, String::new(), outs([("stack", var.to_string())]))
        }
        _ => return Ok(None),
    }))
}

/// The checks: a function that raises `AssertionError` (or warns with `warn`) and gives whether it passed.
fn checks(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let id = text(&n.id);
    let ok = |args: Vec<String>, helper: &str| -> Code {
        let mut all = vec![id.clone()];
        all.extend(args);
        call(var, helper, all, String::new(), outs([("ok", var.to_string())]))
    };
    let common = |named: &mut Vec<(&'static str, &'static str)>| {
        named.extend([("message", "message"), ("warn", "warn")]);
    };
    Ok(Some(match n.op.as_str() {
        "check.range" => {
            refuse_others(n, &["value", "min", "max", "message", "warn"])?;
            let mut named = vec![("min", "minimum"), ("max", "maximum")];
            common(&mut named);
            let mut args = vec![must(n, "value", sc)?];
            args.extend(kwargs(n, &named, sc)?);
            ok(args, "_check_range")
        }
        "check.no_nodata" => {
            refuse_others(n, &["map", "within", "max_share", "message", "warn"])?;
            let mut named = vec![("within", "within"), ("max_share", "max_share")];
            common(&mut named);
            let mut args = vec![must(n, "map", sc)?];
            args.extend(kwargs(n, &named, sc)?);
            ok(args, "_check_no_nodata")
        }
        "check.area_kept" => {
            refuse_others(n, &["before", "after", "within", "class", "tolerance", "message", "warn"])?;
            let mut named = vec![("within", "within"), ("class", "cls"), ("tolerance", "tolerance")];
            common(&mut named);
            let mut args = vec![must(n, "before", sc)?, must(n, "after", sc)?];
            args.extend(kwargs(n, &named, sc)?);
            ok(args, "_check_area_kept")
        }
        "check.accuracy" => {
            refuse_others(n, &["map", "reference", "min", "measure", "message", "warn"])?;
            let min = value_of(n, "min", sc)?.ok_or("give min")?;
            let mut named = vec![("measure", "measure")];
            common(&mut named);
            let mut args = vec![must(n, "map", sc)?, must(n, "reference", sc)?, min];
            args.extend(kwargs(n, &named, sc)?);
            ok(args, "_check_accuracy")
        }
        "check.demand" => {
            refuse_others(n, &["landscape", "demand", "step", "tolerance", "relative", "message", "warn"])?;
            let mut named = vec![("tolerance", "tolerance"), ("relative", "relative")];
            common(&mut named);
            let mut args = vec![must(n, "landscape", sc)?, must(n, "demand", sc)?, must(n, "step", sc)?];
            args.extend(kwargs(n, &named, sc)?);
            ok(args, "_check_demand")
        }
        "check.assert" => {
            refuse_others_but(n, &["expression", "message", "warn", "row"])?;
            let expression = value_of(n, "expression", sc)?.ok_or("give the expression that must be true")?;
            let mut args = vec![expression];
            args.extend(kwargs(n, &[("message", "message"), ("warn", "warn"), ("row", "row")], sc)?);
            for k in n.inputs.keys() {
                args.push(format!("{k}={}", must(n, k, sc)?));
            }
            ok(args, "_check_assert")
        }
        _ => return Ok(None),
    }))
}

/// A node that takes any inputs (named as the expression names them) and these settings: none of the inputs may be
/// named as an argument of the helper.
fn refuse_others_but(n: &Node, settings: &[&str]) -> Result<(), String> {
    if let Some(k) = n.params.keys().find(|k| !settings.contains(&k.as_str())) {
        return Err(format!("its setting {k}: not in the script yet"));
    }
    let taken = ["node", "expression", "message", "warn", "row", "text", "script"];
    if let Some(k) = n.inputs.keys().find(|k| taken.contains(&k.as_str())) {
        return Err(format!("its input {k} is named as the helper's argument: not in the script yet"));
    }
    Ok(())
}

/// The files of batch.run's inputs as strings (a folder, a pattern, or a list of them), paths of the model's folder.
fn files(v: &Value) -> Result<String, String> {
    let one = |s: &str| format!("str({})", path(s));
    match v {
        Value::String(s) => Ok(one(s)),
        Value::Array(a) => {
            let items: Option<Vec<String>> = a.iter().map(|x| x.as_str().map(one)).collect();
            Ok(format!("[{}]", items.ok_or("its inputs: texts")?.join(", ")))
        }
        _ => Err("its inputs: a folder, a pattern or a list of rasters".into()),
    }
}

/// The operators of the landscape the library's `tf.batch` runs on many rasters, by their names there.
const BATCH_TOOLS: &[(&str, &str)] = &[
    ("landscape.mspa", "mspa"),
    ("landscape.spa", "spa"),
    ("landscape.accounting", "accounting"),
    ("fragmentation.fad", "fad"),
    ("fragmentation.fad_multiscale", "fad_multiscale"),
    ("fragmentation.fos", "fos"),
];

fn batch(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["operator", "inputs", "output_dir", "params", "input", "workers", "overwrite", "recursive"])?;
    let operator = literal_text(n, "operator").ok_or("its operator is given by a name: not in the script yet")?;
    let (_, tool) = BATCH_TOOLS
        .iter()
        .find(|(op, _)| *op == operator)
        .ok_or_else(|| format!("its operator {operator}: not in the library's batch"))?;
    let py = mapped().find(|p| p.op == operator).ok_or("its operator is not in the script")?;
    let first = py.args.iter().find(|(_, t)| t.is_empty()).map(|(o, _)| *o).unwrap_or("map");
    if let Some(i) = n.params.get("input").and_then(Value::as_str).filter(|i| !i.is_empty() && *i != first) {
        return Err(format!("its input {i}: not in the script yet"));
    }
    let raw = n.params.get("inputs").ok_or("give the inputs")?;
    if terraflux_graph::setting_ref(raw).is_some() {
        return Err("its inputs given by a name: not in the script yet".into());
    }
    let recursive = matches!(n.params.get("recursive"), Some(Value::Bool(true)));
    let mut inputs = files(raw)?;
    if recursive {
        inputs = format!("tf.collection.expand_sources({inputs}, recursive=True)");
    }
    let out_dir = match n.params.get("output_dir") {
        Some(Value::String(s)) if !s.is_empty() => path(s),
        _ => return Err("give the output folder".into()),
    };
    let mut args = vec![text(tool), inputs, out_dir];
    if let Some(w) = value_of(n, "workers", sc)? {
        args.push(format!("n_workers={w}"));
    }
    if let Some(o) = value_of(n, "overwrite", sc)? {
        args.push(format!("overwrite={o}"));
    }
    // the operator's parameters, by their names in the library
    if let Some(p) = n.params.get("params") {
        let Value::Object(o) = p else { return Err("its params: an object of the operator's parameters".into()) };
        for (k, v) in o {
            if k == "foreground" {
                return Err("its param foreground: not in the script yet".into());
            }
            let theirs = py
                .args
                .iter()
                .find(|(ours, t)| ours == k && !t.is_empty())
                .map(|(_, t)| *t)
                .ok_or_else(|| format!("its param {k}: not in the script yet"))?;
            args.push(format!("{theirs}={}", literal(v)));
        }
    }
    let code = format!(
        "# (the files are named and written as the library's batch does)\n{var} = tf.batch({})\n",
        args.join(", ")
    );
    let o = outs([("table", format!("_batch_table({var})[0]")), ("report", format!("_batch_table({var})[1]"))]);
    Ok((code, o, Need::Tools))
}

pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    if let Some(code) = windowed(n, var, sc)? {
        return Ok(Some(code));
    }
    if let Some(code) = layers_and_classes(n, var, sc)? {
        return Ok(Some(code));
    }
    if let Some(code) = checks(n, var, sc)? {
        return Ok(Some(code));
    }
    Ok(Some(match n.op.as_str() {
        "value.calc" => {
            refuse_others_but(n, &["expression", "text", "row"])?;
            let mut args = kwargs(n, &[("expression", "expression"), ("text", "text"), ("row", "row")], sc)?;
            for k in n.inputs.keys() {
                args.push(format!("{k}={}", must(n, k, sc)?));
            }
            if !n.params.contains_key("expression") && !n.params.contains_key("text") {
                return Err("give an expression, a text or both".into());
            }
            call(
                var,
                "_value_calc",
                args,
                String::new(),
                outs([("value", format!("{var}[\"value\"]")), ("text", format!("{var}[\"text\"]"))]),
            )
        }
        "rng.numpy" => {
            let seed = value_of(n, "seed", sc)?.ok_or("its seed is not given")?;
            (
                format!(
                    "{var} = np.random.RandomState(int({seed}) % 2**32)  # numpy's legacy generator, carried along\n"
                ),
                outs([("rng", var.to_string())]),
                Need::Nothing,
            )
        }
        "rng.r" => {
            return Err(
                "R's generator (set.seed) is the library's private _core.RRandom: its functions take a seed instead"
                    .into(),
            );
        }
        "raster.fill" => {
            return Err(
                "gdal_fillnodata's search of the values and their weights are GDAL's: not in the library".into()
            );
        }
        "raster.contour" => {
            return Err(
                "gdal_contour's tracing of the lines (their order and direction) is GDAL's: not in the library".into(),
            );
        }
        "plugin.wasm" => return Err("a WebAssembly plugin has no counterpart in Python".into()),
        terraflux_graph::SUBMODEL => {
            return Err(
                "its model file was not read (the script needs the path of the model, to find the file beside it)"
                    .into(),
            );
        }
        "batch.run" => batch(n, var, sc)?,
        "agents.table" => {
            refuse_others(n, &["table", "agents"])?;
            let args = kwargs(n, &[("agents", "agents"), ("table", "table")], sc)?;
            if args.is_empty() {
                return Err("give the agents or a table".into());
            }
            call(var, "_agents_table", args, String::new(), outs([("agents", var.to_string())]))
        }
        "agents.parcels" => {
            let mut args = vec![must(n, "parcels", sc)?];
            if let Some(k) = n.params.keys().next() {
                return Err(format!("its setting {k}: not in the script yet"));
            }
            for k in n.inputs.keys().filter(|k| *k != "parcels") {
                args.push(format!("{k}={}", must(n, k, sc)?));
            }
            call(var, "_agents_parcels", args, String::new(), outs([("agents", var.to_string())]))
        }
        "agents.update" => {
            refuse_others_but(n, &["rules"])?;
            let rules = value_of(n, "rules", sc)?.ok_or("no rules")?;
            let mut args = vec![must(n, "agents", sc)?, rules];
            for k in n.inputs.keys().filter(|k| *k != "agents") {
                args.push(format!("{k}={}", must(n, k, sc)?));
            }
            call(var, "_agents_update", args, String::new(), outs([("agents", var.to_string())]))
        }
        "agents.map" => {
            refuse_others(n, &["agents", "parcels", "column"])?;
            let column = value_of(n, "column", sc)?.ok_or("no column")?;
            let args = vec![must(n, "agents", sc)?, must(n, "parcels", sc)?, column];
            call(var, "_agents_map", args, String::new(), outs([("map", var.to_string())]))
        }
        "agents.summary" => {
            refuse_others(n, &["agents"])?;
            call(
                var,
                "_agents_summary",
                vec![must(n, "agents", sc)?],
                String::new(),
                outs([("table", var.to_string())]),
            )
        }
        _ => return Ok(None),
    }))
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

    fn script_of(toml: &str) -> super::super::Script {
        let m = Model::from_toml(toml).unwrap();
        super::super::script(&m, &Registry::standard(), None)
    }

    #[test]
    fn statistics_windows_and_patches() {
        let s = script_of(
            r##"
format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random"
params = { columns = 20, rows = 10 }

[[nodes]]
id = "two"
op = "nlm.random"
params = { columns = 20, rows = 10, seed = 2 }

[[nodes]]
id = "cls"
op = "raster.classify"
inputs = { map = "land.map" }
params = { method = "equal", n = 4 }

[[nodes]]
id = "med"
op = "raster.focal"
inputs = { map = "land.map" }
params = { stat = "median", shape = "circle", radius = 2, fill = true }

[[nodes]]
id = "maj"
op = "raster.majority"
inputs = { map = "cls.map" }
params = { neighbours = 4, replace = "half", iterations = 2 }

[[nodes]]
id = "sv"
op = "raster.sieve"
inputs = { map = "cls.map" }
params = { threshold = 4, neighbours = 8 }

[[nodes]]
id = "lab"
op = "raster.label"
inputs = { map = "cls.map" }
params = { neighbours = 8, classes = [2, 3] }

[[nodes]]
id = "grown"
op = "raster.expand"
inputs = { map = "cls.map" }
params = { classes = [2], cells = 2, into_missing = true }

[[nodes]]
id = "shrunk"
op = "raster.shrink"
inputs = { map = "cls.map" }
params = { classes = 3, neighbours = 4 }

[[nodes]]
id = "stats"
op = "raster.stats"
inputs = { map = "land.map" }
params = { stats = ["mean", "median", "percentile_10"], bins = 5, range = [0, 1] }

[[nodes]]
id = "layers"
op = "stack.of"
inputs = { a = "land.map", b = "two.map" }

[[nodes]]
id = "red"
op = "stack.reduce"
inputs = { maps = "layers.stack" }
params = { stat = "percentile_90" }

[[nodes]]
id = "cor"
op = "raster.correlation"
inputs = { x = "layers.stack" }
params = { method = "spearman" }

[[nodes]]
id = "z"
op = "raster.standardize"
inputs = { map = "land.map" }
params = { method = "percentile", low = 5, high = 95 }

[[nodes]]
id = "rough"
op = "raster.terrain"
inputs = { raster = "land.map" }
params = { measure = "TRI", path = "tri.tif" }

[[nodes]]
id = "legend"
op = "raster.legend"
inputs = { map = "cls.map" }
params = { classes = [[1, "water", "#0000ff"], [2, "forest", "#00aa00"]] }

[[nodes]]
id = "pick"
op = "stack.extract_layer"
inputs = { stack = "layers.stack" }
params = { layer = "a" }

[[nodes]]
id = "more"
op = "stack.set"
inputs = { stack = "layers.stack", map = "cls.map" }
params = { name = "cls" }

[[nodes]]
id = "named"
op = "stack.classes"
inputs = { stack = "more.stack" }

[outputs.low]
from = "stats.table"
"##,
        );
        assert!(s.left.is_empty(), "left: {:?}", s.left);
        let t = &s.text;
        assert!(t.contains("cls = _raster_classify(land, method=\"equal\", n=4)\n"), "{t}");
        assert!(
            t.contains("med = _raster_focal(land, stat=\"median\", shape=\"circle\", radius=2, fill=True)\n"),
            "{t}"
        );
        assert!(t.contains("_raster_majority(cls[0], neighbours=4, replace=\"half\", iterations=2)"), "{t}");
        assert!(t.contains("_raster_sieve(cls[0], threshold=4, neighbours=8)"), "{t}");
        assert!(t.contains("_raster_label(cls[0], neighbours=8, classes=[2, 3])"), "{t}");
        assert!(t.contains("_grow(cls[0], [2], True, cells=2, into_missing=True)"), "{t}");
        assert!(t.contains("_grow(cls[0], 3, False, neighbours=4)"), "{t}");
        assert!(
            t.contains(
                "stats = _raster_stats(land, stats=[\"mean\", \"median\", \"percentile_10\"], bins=5, range=[0, 1])"
            ),
            "{t}"
        );
        assert!(t.contains("_stack_reduce(layers, stat=\"percentile_90\")"), "{t}");
        assert!(t.contains("_raster_correlation(layers, method=\"spearman\")"), "{t}");
        assert!(t.contains("_raster_standardize(land, method=\"percentile\", low=5, high=95)"), "{t}");
        assert!(
            t.contains("# (not passed: path = \"tri.tif\"; the measure is not written)\nrough = _raster_terrain(land, measure=\"TRI\")"),
            "{t}"
        );
        assert!(
            t.contains("_raster_legend(cls[0], [[1, \"water\", \"#0000ff\"], [2, \"forest\", \"#00aa00\"]])"),
            "{t}"
        );
        assert!(
            t.contains("pick = _stack_layer(layers, \"a\")")
                && t.contains("more = _stack_set(layers, cls[0], \"cls\")"),
            "{t}"
        );
        assert!(t.contains("named = _stack_classes(more)"), "{t}");
        // the outputs of a node of two: its parts
        assert!(t.contains("tf.save(stats[0], OUT / \"low\")"), "{t}");
        // the helpers, written once, with the calculator's they run
        for def in ["_raster_focal", "_raster_sieve", "_grow", "_raster_terrain", "_stack_set", "_calc"] {
            assert_eq!(t.matches(&format!("def {def}(")).count(), 1, "{def}\n{t}");
        }
        parses(t);
    }

    #[test]
    fn checks_values_and_agents() {
        let s = script_of(
            r##"
format = "terraflux/1"

[inputs.year]
kind = "value"
value = 2010

[inputs.seed]
kind = "value"
value = 7

[[nodes]]
id = "land"
op = "nlm.random"

[[nodes]]
id = "after"
op = "nlm.random"
params = { seed = 3 }

[[nodes]]
id = "demand"
op = "demand.table"
params = { table = { classes = [1, 2], rows = [[5, 5], [4, 6]] } }

[[nodes]]
id = "inrange"
op = "check.range"
inputs = { value = "land.map" }
params = { min = 0, max = 1, message = "out", warn = true }

[[nodes]]
id = "filled"
op = "check.no_nodata"
inputs = { map = "land.map", within = "after.map" }
params = { max_share = 0.1 }

[[nodes]]
id = "kept"
op = "check.area_kept"
inputs = { before = "land.map", after = "after.map" }
params = { class = 2, tolerance = 3 }

[[nodes]]
id = "agree"
op = "check.accuracy"
inputs = { map = "land.map", reference = "after.map" }
params = { min = 0.5, measure = "kappa" }

[[nodes]]
id = "met"
op = "check.demand"
inputs = { landscape = "land.map", demand = "demand.demand", step = 1 }
params = { tolerance = 0.1, relative = true }

[[nodes]]
id = "later"
op = "value.calc"
inputs = { year = "$year" }
params = { expression = "year + 1", text = "out/{year}_{value}.tif" }

[[nodes]]
id = "late"
op = "check.assert"
inputs = { y = "later.value" }
params = { expression = "y > 2000", message = "too early" }

[[nodes]]
id = "gen"
op = "rng.numpy"
inputs = { seed = "$seed" }

[[nodes]]
id = "farms"
op = "agents.table"
params = { agents = [{ id = 1, wealth = 10 }, { id = 2, wealth = 90 }] }

[[nodes]]
id = "rules"
op = "agents.update"
inputs = { agents = "farms.agents", price = "later.value" }
params = { rules = "wealth = wealth * price / 2000" }

[[nodes]]
id = "total"
op = "agents.summary"
inputs = { agents = "rules.agents" }

[[nodes]]
id = "plots"
op = "agents.parcels"
inputs = { parcels = "land.map", elevation = "after.map" }

[[nodes]]
id = "painted"
op = "agents.map"
inputs = { agents = "rules.agents", parcels = "land.map" }
params = { column = "wealth" }
"##,
        );
        assert!(s.left.is_empty(), "left: {:?}", s.left);
        let t = &s.text;
        assert!(
            t.contains("inrange = _check_range(\"inrange\", land, minimum=0, maximum=1, message=\"out\", warn=True)\n"),
            "{t}"
        );
        assert!(t.contains("filled = _check_no_nodata(\"filled\", land, within=after, max_share=0.1)\n"), "{t}");
        assert!(t.contains("kept = _check_area_kept(\"kept\", land, after, cls=2, tolerance=3)\n"), "{t}");
        assert!(t.contains("agree = _check_accuracy(\"agree\", land, after, 0.5, measure=\"kappa\")\n"), "{t}");
        assert!(t.contains("met = _check_demand(\"met\", land, demand, 1, tolerance=0.1, relative=True)\n"), "{t}");
        assert!(
            t.contains("later = _value_calc(expression=\"year + 1\", text=\"out/{year}_{value}.tif\", year=year)\n"),
            "{t}"
        );
        assert!(
            t.contains("late = _check_assert(\"late\", \"y > 2000\", message=\"too early\", y=later[\"value\"])\n"),
            "{t}"
        );
        assert!(t.contains("gen = np.random.RandomState(int(seed) % 2**32)"), "{t}");
        assert!(
            t.contains("farms = _agents_table(agents=[{\"id\": 1, \"wealth\": 10}, {\"id\": 2, \"wealth\": 90}])\n"),
            "{t}"
        );
        assert!(
            t.contains("rules = _agents_update(farms, \"wealth = wealth * price / 2000\", price=later[\"value\"])\n"),
            "{t}"
        );
        assert!(
            t.contains("total = _agents_summary(rules)\n")
                && t.contains("plots = _agents_parcels(land, elevation=after)\n"),
            "{t}"
        );
        assert!(t.contains("painted = _agents_map(rules, land, \"wealth\")\n"), "{t}");
        for def in ["_check_range", "_value_calc", "_agents_update", "_verdict"] {
            assert_eq!(t.matches(&format!("def {def}(")).count(), 1, "{def}\n{t}");
        }
        parses(t);
    }

    #[test]
    fn what_stays_a_comment() {
        let s = script_of(
            r##"
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "nlm.mpd"

[[nodes]]
id = "holes"
op = "raster.fill"
inputs = { raster = "dem.map" }

[[nodes]]
id = "lines"
op = "raster.contour"
inputs = { map = "dem.map" }
params = { interval = 10 }

[[nodes]]
id = "natural"
op = "raster.classify"
inputs = { map = "dem.map" }
params = { method = "jenks" }

[[nodes]]
id = "slope"
op = "raster.terrain"
inputs = { raster = "dem.map" }
params = { options = "-p" }

[[nodes]]
id = "r"
op = "rng.r"
inputs = { seed = 1 }

[[nodes]]
id = "wasm"
op = "plugin.wasm"
params = { path = "p.wasm" }
"##,
        );
        let why = |id: &str| s.left.iter().find(|(i, _, _)| i == id).map(|(_, _, w)| w.clone()).unwrap_or_default();
        assert_eq!(s.left.len(), 6, "{:?}", s.left);
        assert!(why("holes").contains("gdal_fillnodata"), "{}", why("holes"));
        assert!(why("lines").contains("gdal_contour"), "{}", why("lines"));
        assert!(why("natural").contains("jenks"), "{}", why("natural"));
        assert!(why("slope").contains("-p"), "{}", why("slope"));
        assert!(why("r").contains("RRandom"), "{}", why("r"));
        assert!(why("wasm").contains("plugin"), "{}", why("wasm"));
        assert!(!s.text.contains("def _raster_focal("), "{}", s.text);
        parses(&s.text);
    }

    #[test]
    fn batches_and_submodels() {
        let s = script_of(
            r##"
format = "terraflux/1"

[[nodes]]
id = "tiles"
op = "batch.run"
params = { operator = "landscape.mspa", inputs = "tiles/*.tif", output_dir = "out", params = { edge_width = 2, connectivity = 4 }, workers = 2, overwrite = true }

[[nodes]]
id = "other"
op = "batch.run"
params = { operator = "raster.focal", inputs = ["a.tif"], output_dir = "out" }
"##,
        );
        let t = &s.text;
        assert!(
            t.contains("tiles = tf.batch(\"mspa\", str(MODEL_DIR / \"tiles/*.tif\"), MODEL_DIR / \"out\", n_workers=2, overwrite=True, connectivity=4, edge_width=2)\n"),
            "{t}"
        );
        assert!(
            s.left.iter().any(|(id, _, why)| id == "other" && why.contains("not in the library's batch")),
            "{:?}",
            s.left
        );
        parses(t);
        // a submodel: its file read beside the model, its nodes a group's
        let dir = std::env::temp_dir().join(format!("terraflux-python-submodel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("inner.toml"),
            r#"format = "terraflux/1"
[inputs.x]
kind = "value"
value = 1
[[nodes]]
id = "plus"
op = "value.calc"
inputs = { x = "$x" }
params = { expression = "x + 1" }
[outputs.y]
from = "plus.value"
"#,
        )
        .unwrap();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[inputs.v]
kind = "value"
value = 3

[[nodes]]
id = "sub"
op = "control.submodel"
inputs = { x = "$v" }
params = { path = "inner.toml" }

[[nodes]]
id = "check"
op = "check.range"
inputs = { value = "sub.y" }
params = { max = 10 }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &Registry::standard(), Some(&dir.join("outer.toml")));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(s.left.is_empty(), "left: {:?}", s.left);
        let t = &s.text;
        assert!(t.contains("# sub (control.group): from inner.toml"), "{t}");
        assert!(t.contains("plus = _value_calc(expression=\"x + 1\", x=sub_x)\n"), "{t}");
        assert!(t.contains("check = _check_range(\"check\", plus[\"value\"], maximum=10)\n"), "{t}");
        parses(t);
        // unread, a comment saying so
        let s = super::super::script(&m, &Registry::standard(), None);
        assert!(
            s.left.iter().any(|(id, _, why)| id == "sub" && why.contains("model file was not read")),
            "{:?}",
            s.left
        );
    }

    #[test]
    fn the_library_calls_exist() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../website/data/python_api.json");
        let api: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let (names, kw) = super::super::tests::api_params(&api, "tf.batch").unwrap();
        assert!(kw, "{names:?}");
        for k in ["tool", "inputs", "output_dir", "n_workers", "overwrite"] {
            assert!(names.iter().any(|n| n == k), "{k}: {names:?}");
        }
        assert!(super::super::tests::api_params(&api, "tf.collection.expand_sources").is_ok());
        for (op, tool) in BATCH_TOOLS {
            let py = mapped().find(|p| p.op == *op).unwrap();
            let (names, kw) = super::super::tests::api_params(&api, &format!("tf.{tool}")).unwrap();
            assert!(
                kw || py.args.iter().filter(|(_, t)| !t.is_empty()).all(|(_, t)| names.iter().any(|n| n == t)),
                "{op}"
            );
        }
    }

    /// The helpers run in Python (numpy, scipy, pandas and xarray there) give what Terraflux's operators give: the
    /// cases of the operators' own tests, and gdaldem's terrain of a small surface.
    #[test]
    fn helpers_give_terraflux_s_numbers() {
        let have = std::process::Command::new("python")
            .args(["-I", "-c", "import numpy, scipy, pandas, xarray"])
            .output()
            .is_ok_and(|o| o.status.success());
        if !have {
            return;
        }
        let driver =
            format!("import numpy as np\n{}\n{}\n{}", super::super::CALC_HELPER, super::super::TOOLS_HELPER, CHECKS);
        let o = std::process::Command::new("python")
            .args(["-I", "-c", "import sys; exec(compile(sys.stdin.read(), 'helpers', 'exec'))"])
            .env("OMP_NUM_THREADS", "1")
            .env("OPENBLAS_NUM_THREADS", "1")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(driver.as_bytes())?;
                c.wait_with_output()
            })
            .unwrap();
        assert!(o.status.success(), "{}\n{}", String::from_utf8_lossy(&o.stdout), String::from_utf8_lossy(&o.stderr));
    }

    /// What the helpers must give (after the operators' tests of Terraflux, and gdaldem 3.12 for the terrain).
    const CHECKS: &str = r##"
nan = float("nan")


def M(rows, resolution=(1.0, 1.0)):
    a = np.asarray(rows, dtype="float64")
    h, w = a.shape
    return _xr.DataArray(a, dims=("y", "x"), coords={"y": -resolution[1] * (np.arange(h) + 0.5),
                                                     "x": resolution[0] * (np.arange(w) + 0.5)})


def same(got, want, tol=1e-9):
    got, want = np.asarray(got, dtype="float64"), np.asarray(want, dtype="float64")
    assert got.shape == want.shape, (got.shape, want.shape)
    assert np.allclose(got, want, atol=tol, rtol=0, equal_nan=True), (got, want)


# a window
m = M([[1, 2, 3], [4, 5, 6], [7, 8, nan]])
mean = _raster_focal(m).values
assert mean[0, 0] == 3.0 and mean[1, 1] == 4.5 and np.isnan(mean[2, 2])
assert abs(_raster_focal(m, fill=True).values[2, 2] - 19 / 3) < 1e-12
assert _raster_focal(m, stat="max").values[1, 1] == 8 and _raster_focal(m, stat="variety").values[0, 0] == 4
assert _raster_focal(m, stat="nodata").values[1, 1] == 1
assert _raster_focal(m, stat="sum", shape="circle").values[1, 1] == 25
assert _raster_focal(m, stat="sum", shape="annulus", inner=0).values[1, 1] == 20
same(_raster_focal(M(np.full((5, 5), 2.0)), shape="gaussian", radius=2).values, np.full((5, 5), 2.0))

# specks: the majority filter, the sieve
specks = M([[1, 1, 1, 1], [1, 2, 1, 1], [1, 1, 3, 3], [1, 1, 3, 3]])
f = _raster_majority(specks).values
assert f[1, 1] == 1 and f[2, 2] == 3
s = _raster_sieve(specks, 2).values
assert s[1, 1] == 1 and s[3, 3] == 3
assert (_raster_sieve(specks, 5).values == 1).all()

# patches: numbered by their first cell, only the classes given
ids, table = _raster_label(M([[1, 1, 2], [2, nan, 2], [2, 1, 1]]))
same(ids.values, [[1, 1, 2], [3, nan, 2], [3, 4, 4]])
assert table.loc[1].tolist() == [2.0, 2.0, 2.0, 2.0]
ids, _ = _raster_label(M([[1, 1, 2], [2, nan, 2], [2, 1, 1]]), 8, [2])
same(ids.values, [[nan, nan, 1], [2, nan, 1], [2, nan, nan]])

# a class grown into the others, shrunk
grown = _grow(M([[1, 1, 1], [1, 2, 1], [1, 1, 1]]), [2], True).values
assert (grown == 2).all(), grown
shrunk = _grow(M([[2, 2, 2], [2, 2, 2], [2, 2, 1]]), [2], False).values
assert shrunk[1, 1] == 1 and shrunk[0, 0] == 2

# statistics of a map: 1 2 3 4 10 (one cell without a value)
stats, hist = _raster_stats(M([[1, 2, 3], [4, nan, 10]]),
                            ["count", "nodata", "sum", "mean", "std", "median", "percentile_25", "majority", "unique"], 3)
same(stats.iloc[0].tolist(), [1, 5, 1, 20, 4, 10 ** 0.5, 3, 2, 1, 5])
same(hist["count"].tolist(), [3, 1, 1])

# by breaks
classes, table = _raster_classify(M([[1, 2, 3], [4, 5, 6]]), "equal", 2)
same(classes.values, [[1, 1, 1], [2, 2, 2]])
same(table["cells"].tolist(), [3, 3])
same(_raster_classify(M([[1, 2, 3], [4, 5, 6]]), "breaks", breaks=[2.0, 4.0])[0].values, [[1, 1, 2], [2, 3, 3]])

# gdaldem's terrain of a surface (cells 10 x 20, one without a value), as GDAL 3.12 computes it
dem = M([[10, 12, 15, 19, 24, 30], [11, 14, 18, 23, 29, 36], [12, 16, 21, nan, 35, 43],
         [14, 19, 25, 32, 40, 49], [30, 23, 30, 38, 47, 57]], (10.0, 20.0))
want = {
    "slope": [[6.726, 15.07, 20.846, 26.218, 31.138, 22.564], [16.917, 20.002, 19.304, 30.276, 34.868, 37.66],
              [22.145, 25.034, 23.586, nan, 32.513, 40.81], [25.773, 24.918, 33.767, 38.305, 36.326, 43.999],
              [36.436, 19.926, 38.329, 42.031, 45.354, 32.317]],
    "aspect": [[321.34, 308.66, 310.601, 311.634, 312.274, 332.447], [288.435, 299.745, 299.055, 299.055, 311.424, 312.879],
               [290.556, 299.055, 311.186, nan, 313.727, 309.094], [348.996, 322.907, 307.235, 298.413, 302.735, 307.875],
               [15.068, 0.0, 303.69, 305.218, 306.384, 327.171]],
    "hillshade": [[200, 217, 228, 236, 242, 236], [215, 223, 222, 235, 244, 246], [223, 230, 233, nan, 243, 245],
                  [234, 238, 241, 238, 240, 244], [176, 213, 242, 243, 243, 248]],
    "TRI": [[4.899, 8.185, 11.533, 14.933, 18.358, 17.378], [8.0, 10.149, 10.0, 17.029, 21.048, 23.516],
            [10.724, 12.961, 15.1, nan, 21.794, 25.357], [30.984, 19.519, 19.494, 22.539, 22.825, 28.107],
            [34.684, 29.799, 22.204, 25.593, 29.0, 25.573]],
    "TPI": [[-0.75, -0.375, -0.375, -0.375, -0.375, 2.25], [0.0, -0.375, 0.75, 0.0, -0.875, -0.375],
            [-0.375, -0.75, 0.0, nan, -0.875, 0.375], [-5.25, -2.375, -0.5, -1.5, -2.625, -0.75],
            [2.625, -5.25, -0.375, -0.375, -0.375, 3.75]],
    "roughness": [[5, 9, 13, 17, 21, 17], [8, 11, 11, 20, 24, 27], [11, 14, 18, nan, 26, 29],
                  [29, 18, 22, 26, 25, 32], [32, 32, 25, 29, 33, 25]],
}
for measure, rows in want.items():
    same(_raster_terrain(dem, measure).values, rows, 2e-3)

# the checks: a failure raises, a warning is only said
import warnings

try:
    _check_range("n", M([[0.2, 1.5, -1.0]]), minimum=0, maximum=1)
    raise SystemExit("a range that fails must raise")
except AssertionError as e:
    assert "2 of 3 cells not between 0 and 1, from -1 to 1.5" in str(e), e
with warnings.catch_warnings(record=True) as w:
    warnings.simplefilter("always")
    assert _check_range("n", 3.0, maximum=2, warn=True) is False
    assert "3 is not at most 2" in str(w[0].message)
assert _check_assert("n", "a > 2", a=3.0) is True
try:
    _check_assert("n", "a > 2", message="too few", a=1.0)
    raise SystemExit("an assertion that fails must raise")
except AssertionError as e:
    assert "too few (a > 2: it is 0)" in str(e), e
assert _check_assert("n", "p >= 0 and p <= 1", p=M([[0.2, 1.5, nan]]), warn=True) is False

# value.calc: a table's value of a row, a number, a text
t = _pd.DataFrame({"year": [2000.0, 2010.0], "area": [4.0, 6.0]})
assert _value_calc(expression="t.area * k + max(1, 2) if on else 0", t=t, k=3, on=True)["value"] == 14.0
assert _value_calc(expression="t.year", row=-1, t=t)["value"] == 2010.0
out = _value_calc(expression="x * 2 + 1", text="out/{species}_{year}_{value}_{{ok}}.tif", year=2010.0, species="oak", x=0.25)
assert out["value"] == 1.5 and out["text"] == "out/oak_2010_1.5_{ok}.tif", out

# agents: rules in order, the parcels' means, a summary
agents = _agents_table(agents=[{"id": 1, "wealth": 10.0}, {"id": 2, "wealth": 90.0}])
agents = _agents_update(agents, "wealth = wealth + step; farmer = wealth > 50", step=5.0)
same(agents["wealth"], [15.0, 95.0])
same(agents["farmer"], [0.0, 1.0])
same(_agents_summary(agents).iloc[0].tolist(), [2, 3, 1.5, 110.0, 55.0, 1.0, 0.5])
parcels = _agents_parcels(M([[1, 1, 2], [2, 2, nan]]), height=M([[1, 3, 5], [nan, 7, 9]]))
same(parcels.to_numpy(), [[1, 2, 2], [2, 3, 6]])
same(_agents_map(agents, M([[1, 2, 3]]), "wealth").values, [[15.0, 95.0, nan]])
"##;
}
