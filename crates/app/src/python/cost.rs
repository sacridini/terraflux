//! The Python counterparts of the operators of distances and costs (cost, distance).
//!
//! A transition is `tf.cost.Transition`: the operators of combining and of maths are Python's own (`a + b`,
//! `tr ** 2`, `1 / tr`, `tr.map("sqrt")`), the tables of distances are DataFrames made from the matrices of the
//! library (a `point` column, then a column per point to).

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[
    Py {
        op: "cost.transition",
        call: "tf.cost.transition",
        args: &[("map", ""), ("function", "function"), ("directions", "directions"), ("symmetric", "symmetric")],
        defaults: &[],
        outs: &[("transition", "")],
        kind: Kind::Plain,
    },
    Py {
        // the layers of a stack (band) are the maps; the library's "mahal" reads them as layers
        op: "cost.mahal",
        call: "tf.cost.transition",
        args: &[("maps", ""), ("directions", "directions")],
        defaults: &[("function", "\"mahal\"")],
        outs: &[("transition", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "cost.areas",
        call: "tf.cost.areas",
        args: &[("map", ""), ("directions", "directions")],
        defaults: &[],
        outs: &[("transitions", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "cost.barriers",
        call: "tf.cost.barriers",
        args: &[("map", ""), ("directions", "directions"), ("breaks", "breaks"), ("symmetric", "symmetric")],
        defaults: &[],
        outs: &[("transitions", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "cost.geo_correction",
        call: "tf.cost.geo_correction",
        args: &[("transition", ""), ("type", "type"), ("scale", "scale")],
        defaults: &[],
        outs: &[("transition", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "cost.normalize",
        call: "tf.cost.normalize",
        args: &[("transition", ""), ("method", "method")],
        defaults: &[],
        outs: &[("transition", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "cost.sum_reciprocal",
        call: "tf.cost.sum_reciprocal",
        args: &[("a", ""), ("b", "x2")],
        defaults: &[],
        outs: &[("transition", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "cost.to_map",
        call: "tf.cost.Transition.to_raster",
        args: &[("transition", ""), ("reduce", "reduce")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Method,
    },
    Py {
        op: "cost.acc_cost",
        call: "tf.cost.acc_cost",
        args: &[("transition", ""), ("origins", "origins")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "cost.passage",
        call: "tf.cost.passage",
        args: &[
            ("transition", ""),
            ("origins", "origin"),
            ("goals", "goal"),
            ("theta", "theta"),
            ("total_net", "total_net"),
            ("gdistance_compat", "gdistance_compat"),
        ],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
];

/// A value written as an operand of an operator: in parentheses unless it is a name, a number or an item.
fn operand(v: &str) -> String {
    let simple = !v.is_empty()
        && !v.starts_with('-')
        && v.chars().all(|c| c.is_alphanumeric() || matches!(c, '_' | '.' | '[' | ']' | '"'));
    if simple { v.to_string() } else { format!("({v})") }
}

/// A matrix (a Python expression) as a table of Terraflux's shape: the `point` column (and `weight` when asked),
/// then a column per point to, named 1, 2...
fn matrix_table(var: &str, matrix: &str, weight: bool) -> String {
    let mut code = format!(
        "import pandas as _pd\n{var} = _pd.DataFrame({matrix}, columns=[str(j) for j in range(1, {matrix}.shape[1] + 1)])\n"
    );
    code.push_str(&format!("{var}.insert(0, \"point\", range(1, len({var}) + 1))\n"));
    if weight {
        code.push_str(&format!("{var}.insert(0, \"weight\", 1)\n"));
    }
    code
}

/// The map of a distance operator coded as GuidosToolbox's (the foreground's classes, else 0 missing, 1 background,
/// 2 foreground).
fn coded(n: &Node, sc: &Scope) -> Result<String, String> {
    let v = value_of(n, "map", sc)?.ok_or("its map is not connected")?;
    Ok(match value_of(n, "foreground", sc)? {
        Some(fg) => format!("tf.reclassify({v}, foreground={fg})"),
        None => format!("{v}.fillna(0).astype(\"uint8\")"),
    })
}

/// A required input.
fn need(n: &Node, name: &str, sc: &Scope) -> Result<String, String> {
    value_of(n, name, sc)?.ok_or_else(|| format!("its {name} is not connected"))
}

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let one = |p: &str, e: String| HashMap::from([(p.to_string(), e)]);
    let done = |code: String, outs: HashMap<String, String>| Ok(Some((code, outs, Need::Nothing)));
    match n.op.as_str() {
        // 1 / the function's value on the connections: the library's `1 / tr`
        "cost.transition" => {
            let r = match setting(n, "reciprocal", sc)? {
                None => return Ok(None),
                Some(r) if r == "False" => return Ok(None),
                Some(r) => r,
            };
            let mut args = vec![need(n, "map", sc)?];
            args.extend(given(
                n,
                &[("function", "function"), ("directions", "directions"), ("symmetric", "symmetric")],
                sc,
            )?);
            let call = format!("tf.cost.transition({})", args.join(", "));
            let code = if r == "True" {
                format!("{var} = 1 / {call}\n")
            } else {
                format!("{var} = {call}\nif {r}:\n    {var} = 1 / {var}\n")
            };
            done(code, one("transition", var.to_string()))
        }
        "cost.combine" => {
            let (a, b) = (operand(&need(n, "a", sc)?), operand(&need(n, "b", sc)?));
            let sign = match choice(n, "operation").unwrap_or("sum") {
                "sum" => "+",
                "difference" => "-",
                "product" => "*",
                o => return Err(format!("its operation {o:?}: not sum, difference or product")),
            };
            done(format!("{var} = {a} {sign} {b}\n"), one("transition", var.to_string()))
        }
        "cost.math" => {
            let t = need(n, "transition", sc)?;
            let value = value_of(n, "value", sc)?.unwrap_or_else(|| "1".into());
            let f = choice(n, "function").ok_or("its function is not a value of the model")?;
            let e = match f {
                "sqrt" | "exp" | "log" => format!("{}.map({})", operand(&t), text(f)),
                "pow" => format!("{} ** {}", operand(&t), operand(&value)),
                "multiply" => format!("{} * {}", operand(&t), operand(&value)),
                "divide" => format!("{} / {}", operand(&value), operand(&t)),
                f => return Err(format!("its function {f:?}: unknown")),
            };
            done(format!("{var} = {e}\n"), one("transition", var.to_string()))
        }
        "cost.distance" => {
            let (t, from) = (need(n, "transition", sc)?, need(n, "from", sc)?);
            let mut args = vec![t, from];
            args.extend(value_of(n, "to", sc)?);
            let m = format!("{var}_m");
            let code = format!("{m} = tf.cost.cost_distance({})\n{}", args.join(", "), matrix_table(var, &m, false));
            done(code, one("table", var.to_string()))
        }
        "cost.commute_distance" => {
            let (t, p) = (need(n, "transition", sc)?, need(n, "points", sc)?);
            let m = format!("{var}_m");
            let code = format!("{m} = tf.cost.commute_distance({t}, {p})\n{}", matrix_table(var, &m, false));
            done(code, one("table", var.to_string()))
        }
        "cost.rsp_distance" => {
            let (t, from) = (need(n, "transition", sc)?, need(n, "from", sc)?);
            let to = value_of(n, "to", sc)?.unwrap_or_else(|| from.clone());
            let theta = need(n, "theta", sc)?;
            let mut args = vec![t, from, to, theta];
            args.extend(given(n, &[("total_net", "total_net"), ("method", "method")], sc)?);
            let m = format!("{var}_m");
            let code = format!("{m} = tf.cost.rsp_distance({})\n{}", args.join(", "), matrix_table(var, &m, false));
            done(code, one("table", var.to_string()))
        }
        "cost.path_inc" => {
            if n.inputs.contains_key("weights") {
                return Err("its weights: not in the script yet (the library's weight, a result a layer)".into());
            }
            let (t, origin, from) = (need(n, "transition", sc)?, need(n, "origin", sc)?, need(n, "from", sc)?);
            let mut args = vec![t, origin, from];
            args.extend(value_of(n, "to", sc)?);
            args.extend(given(n, &[("theta", "theta"), ("gdistance_compat", "gdistance_compat")], sc)?);
            let r = format!("{var}_r");
            let (o, no) = (format!("{var}_overlap"), format!("{var}_nonoverlap"));
            let mut code = format!("{r} = tf.cost.path_inc({})\n", args.join(", "));
            code.push_str(&matrix_table(&o, &format!("{r}[\"overlap\"]"), true));
            code.push_str(&matrix_table(&no, &format!("{r}[\"nonoverlap\"]"), true));
            done(code, HashMap::from([("overlap".to_string(), o), ("nonoverlap".to_string(), no)]))
        }
        "cost.shortest_path" => {
            let (t, origin, goals) = (need(n, "transition", sc)?, need(n, "origin", sc)?, need(n, "goals", sc)?);
            let (lines, map, cells) = (format!("{var}_lines"), format!("{var}_map"), format!("{var}_cells"));
            // the share of the paths through each cell (a path counts a cell once), wrapped as the transition's maps
            let code = format!(
                "{lines} = tf.cost.shortest_path({t}, {origin}, {goals}, output=\"lines\")\n\
                 {cells} = tf.cost.shortest_path({t}, {origin}, {goals}, output=\"cells\")\n\
                 {map} = np.zeros({t}.n_cells)\n\
                 for _p in {cells}:\n    {map}[np.unique(_p)] += 1 / len({cells})\n\
                 {map} = {t}._wrap({map}.reshape({t}.shape), \"share\")\n"
            );
            done(code, HashMap::from([("lines".to_string(), lines), ("map".to_string(), map)]))
        }
        "distance.euclidean" => {
            let m = coded(n, sc)?;
            let stats = format!("{var}_stats");
            let code = format!(
                "{var} = tf.euclidean_distance({m})\nimport pandas as _pd\n\
                 {stats} = _pd.DataFrame({{\"foreground\": [1, 0], \"cells\": [{var}.fg_pixel_count, {var}.bg_pixel_count], \
                 \"mean\": [{var}.mean_fg_distance, {var}.mean_bg_distance], \
                 \"max\": [{var}.max_fg_distance, {var}.max_bg_distance]}})\n"
            );
            let outs = HashMap::from([
                ("map".to_string(), format!("{var}.signed_distance")),
                ("display".to_string(), format!("{var}.byte_map")),
                ("stats".to_string(), stats),
            ]);
            done(code, outs)
        }
        "distance.influence_zones" | "distance.objects_proximity" => {
            let proximity = n.op == "distance.objects_proximity";
            let m = coded(n, sc)?;
            let settings: &[(&str, &str)] = if proximity {
                &[("min_area", "min_area"), ("core_buffer", "core_buffer"), ("prox_max", "prox_max")]
            } else {
                &[("min_area", "min_area"), ("core_buffer", "core_buffer"), ("buffer", "buffer")]
            };
            let mut args = vec![m];
            args.extend(given(n, settings, sc)?);
            let f = if proximity { "tf.proximity" } else { "tf.influence_zones" };
            let objects = format!("{var}_objects");
            let code = format!(
                "{var} = {f}({})\nimport pandas as _pd\n{objects} = _pd.DataFrame({{\"id\": range(1, len({var}.object_area) + 1), \
                 \"cells\": {var}.object_area}})\n",
                args.join(", ")
            );
            let mut outs = HashMap::from([
                ("map".to_string(), format!("{var}.classes")),
                ("ids".to_string(), format!("{var}.ids")),
                ("zones".to_string(), format!("{var}.zones")),
                ("objects".to_string(), objects),
            ]);
            if proximity {
                outs.insert("distance".into(), format!("{var}.distance"));
                outs.insert("cag".into(), format!("{var}.cag"));
            }
            done(code, outs)
        }
        _ => Ok(None),
    }
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
    fn a_least_cost_model() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "res"
op = "raster.read"
params = { path = "C:/data/resistance.tif" }

[[nodes]]
id = "pts"
op = "vector.read"
params = { path = "C:/data/points.gpkg" }

[[nodes]]
id = "tr"
op = "cost.transition"
inputs = { map = "res.raster" }
params = { function = "mean", directions = 8, reciprocal = true }

[[nodes]]
id = "geo"
op = "cost.geo_correction"
inputs = { transition = "tr.transition" }
params = { type = "c" }

[[nodes]]
id = "root"
op = "cost.math"
inputs = { transition = "geo.transition" }
params = { function = "sqrt" }

[[nodes]]
id = "twice"
op = "cost.combine"
inputs = { a = "geo.transition", b = "root.transition" }
params = { operation = "difference" }

[[nodes]]
id = "acc"
op = "cost.acc_cost"
inputs = { transition = "twice.transition", origins = "pts.vector" }

[[nodes]]
id = "path"
op = "cost.shortest_path"
inputs = { transition = "geo.transition", origin = "pts.vector", goals = "pts.vector" }

[[nodes]]
id = "d"
op = "cost.distance"
inputs = { transition = "geo.transition", from = "pts.vector" }

[[nodes]]
id = "cover"
op = "raster.read"
params = { path = "C:/data/cover.tif" }

[[nodes]]
id = "eu"
op = "distance.euclidean"
inputs = { map = "cover.raster" }
params = { foreground = [1, 2] }

[[nodes]]
id = "zones"
op = "distance.influence_zones"
inputs = { map = "cover.raster" }
params = { min_area = 20 }
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        assert!(s.left.is_empty(), "{:?}\n{}", s.left, s.text);
        for line in [
            "tr = 1 / tf.cost.transition(res, function=\"mean\", directions=8)",
            "geo = tf.cost.geo_correction(tr, type=\"c\")",
            "root = geo.map(\"sqrt\")",
            "twice = geo - root",
            "acc = tf.cost.acc_cost(twice, origins=pts)",
            "path_map = np.zeros(geo.n_cells)",
            "d_m = tf.cost.cost_distance(geo, pts)",
            "eu = tf.euclidean_distance(tf.reclassify(cover, foreground=[1, 2]))",
            "zones = tf.influence_zones(cover.fillna(0).astype(\"uint8\"), min_area=20)",
        ] {
            assert!(s.text.contains(line), "{line}\n{}", s.text);
        }
        parses(&s.text);
    }

    #[test]
    fn the_walks_and_their_tables() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "res"
op = "raster.read"
params = { path = "C:/data/resistance.tif" }

[[nodes]]
id = "pts"
op = "vector.read"
params = { path = "C:/data/points.gpkg" }

[[nodes]]
id = "tr"
op = "cost.transition"
inputs = { map = "res.raster" }

[[nodes]]
id = "p"
op = "cost.passage"
inputs = { transition = "tr.transition", origins = "pts.vector", goals = "pts.vector" }
params = { theta = 2.5 }

[[nodes]]
id = "rsp"
op = "cost.rsp_distance"
inputs = { transition = "tr.transition", from = "pts.vector" }
params = { theta = 1 }

[[nodes]]
id = "pi"
op = "cost.path_inc"
inputs = { transition = "tr.transition", origin = "pts.vector", from = "pts.vector" }

[[nodes]]
id = "norm"
op = "cost.normalize"
inputs = { transition = "tr.transition" }

[[nodes]]
id = "back"
op = "cost.to_map"
inputs = { transition = "norm.transition" }
"#,
        )
        .unwrap();
        let s = script(&m, &r, None);
        assert!(s.left.is_empty(), "{:?}\n{}", s.left, s.text);
        for line in [
            "p = tf.cost.passage(tr, origin=pts, goal=pts, theta=2.5)",
            "rsp_m = tf.cost.rsp_distance(tr, pts, pts, 1)",
            "back = norm.to_raster()",
        ] {
            assert!(s.text.contains(line), "{line}\n{}", s.text);
        }
        parses(&s.text);
    }
}
