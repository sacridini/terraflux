//! The random forests and the potential of each class against scikit-learn's and the library's
//! (tests/data/reference/ml.npz and .json, tools/make_ml_reference.py): a classifier's probabilities of three
//! classes and a regressor's values with their importances, and the library's Estimator's maps of PLUS's land
//! expansion analysis and of the presence of each class with the drivers' importances; to the bit.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn reference() -> (BTreeMap<String, Array>, serde_json::Value) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("ml.npz")).unwrap();
    let meta = serde_json::from_str(&std::fs::read_to_string(dir.join("ml.json")).unwrap()).unwrap();
    (z, meta)
}

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

/// The records of the table: five predictors x0..x4 and the target, or the test's predictors.
fn records(x: &Array, y: Option<&[f64]>) -> Table {
    let (n, k) = (x.shape[0], x.shape[1]);
    let mut t = Table::new((0..k).map(|j| format!("x{j}")).chain(y.map(|_| "target".to_string())).collect());
    for i in 0..n {
        t.rows.push(x.data[i * k..(i + 1) * k].iter().copied().chain(y.map(|y| y[i])).collect());
    }
    t
}

fn table(v: &Value) -> &Table {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t
}

#[test]
fn forests_are_scikit_learns() {
    let (z, meta) = reference();
    let x = &z["table/x"];
    let test = records(&z["table/test"], None);
    for (task, target) in [("classifier", "table/class"), ("regressor", "table/value")] {
        for name in ["default", "shaped"] {
            let mut params = meta[format!("{task}/{name}")].clone();
            params["task"] = serde_json::json!(if task == "classifier" { "classes" } else { "values" });
            let fit = run("ml.fit", vec![("table", Value::Table(records(x, Some(&z[target].data))))], params);
            check(
                &format!("{task} {name} importance"),
                &table(&fit["importance"]).rows[0],
                &z[format!("{task}/{name}/importance").as_str()].data,
            );
            let out = run(
                "ml.predict",
                vec![("model", fit["model"].clone()), ("table", Value::Table(test.clone()))],
                serde_json::json!({}),
            );
            let t = table(&out["table"]);
            if task == "classifier" {
                // the probabilities of the classes 1, 2 and 5, row by row
                let got: Vec<f64> = t.rows.iter().flat_map(|r| r[6..9].to_vec()).collect();
                check(&format!("{task} {name}"), &got, &z[format!("{task}/{name}/proba").as_str()].data);
                assert_eq!(t.columns[6..9], ["p_1", "p_2", "p_5"]);
            } else {
                let got: Vec<f64> = t.rows.iter().map(|r| r[5]).collect();
                check(&format!("{task} {name}"), &got, &z[format!("{task}/{name}/predict").as_str()].data);
            }
        }
    }
}

#[test]
fn the_potential_matches_the_library() {
    let (z, meta) = reference();
    let t0 = &z["pie/t0"];
    let (ny, nx) = (t0.shape[0], t0.shape[1]);
    let g = Arc::new(Grid::new(nx, ny));
    let map = |k: &str| Value::Map(Map::new(g.clone(), z[k].data.clone()));
    let names: Vec<String> =
        meta["pie/drivers"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    let drivers = Stack {
        grid: g.clone(),
        keys: names.iter().map(|n| Key::Name(n.clone())).collect(),
        layers: names.iter().map(|n| Arc::new(z[format!("pie/driver/{n}").as_str()].data.clone())).collect(),
    };
    for name in ["expansion", "stratified", "presence", "boosting"] {
        let m = &meta[format!("pie/{name}")];
        let mut params =
            serde_json::json!({"n_estimators": 10, "random_state": 0, "target": m["target"], "seed": m["seed"]});
        if name == "boosting" {
            params["method"] = serde_json::json!("gradient_boosting");
            params["max_iter"] = serde_json::json!(20);
        }
        for k in ["sample", "stratify", "balance"] {
            if !m[k].is_null() {
                params[k] = m[k].clone();
            }
        }
        let mut inputs = vec![("landscape", map("pie/t0")), ("drivers", Value::Stack(drivers.clone()))];
        if m["target"] == "expansion" {
            inputs.push(("later", map("pie/t1")));
        }
        let out = run("ml.potential", inputs, params);
        let Value::Stack(p) = &out["potential"] else { panic!() };
        let classes: Vec<f64> = m["classes"].as_array().unwrap().iter().map(|c| c.as_f64().unwrap()).collect();
        assert_eq!(p.keys, classes.iter().map(|&c| Key::Class(c)).collect::<Vec<_>>());
        for (k, c) in classes.iter().enumerate() {
            check(&format!("{name} {c}"), &p.layers[k], &z[format!("pie/{name}/{c}").as_str()].data);
        }
        // The library's importance: a row per driver (sorted), a column per class (scikit-learn's boosting has none)
        let imp = table(&out["importance"]);
        let mut sorted = names.clone();
        sorted.sort();
        assert_eq!(imp.columns[1..], sorted[..]);
        if name != "boosting" {
            let want = &z[format!("pie/{name}/importance").as_str()];
            let got: Vec<f64> = (0..sorted.len()).flat_map(|d| imp.rows.iter().map(move |r| r[1 + d])).collect();
            check(&format!("{name} importance"), &got, &want.data);
        }

        // the model again on the drivers: the same maps
        let again = run(
            "ml.predict",
            vec![("model", out["model"].clone()), ("predictors", Value::Stack(drivers.clone()))],
            serde_json::json!({}),
        );
        let Value::Stack(q) = &again["probability"] else { panic!() };
        for (a, b) in q.layers.iter().zip(&p.layers) {
            check(&format!("{name} again"), a, b);
        }
    }
}

#[test]
fn boosting_is_scikit_learns() {
    let (z, meta) = reference();
    let x = &z["table/x"];
    let test = records(&z["table/test"], None);
    for name in ["binary", "multiclass", "shaped", "regression"] {
        let m = &meta[format!("boosting/{name}")];
        let mut params = m.clone();
        let target = params.as_object_mut().unwrap().remove("target").unwrap();
        let regression = target == "table/value";
        params["method"] = serde_json::json!("gradient_boosting");
        params["task"] = serde_json::json!(if regression { "values" } else { "classes" });
        let key = if target == "binary" { "table/binary" } else { target.as_str().unwrap() };
        let y = &z[key].data;
        let fit = run("ml.fit", vec![("table", Value::Table(records(x, Some(y))))], params);
        let out = run(
            "ml.predict",
            vec![("model", fit["model"].clone()), ("table", Value::Table(test.clone()))],
            serde_json::json!({}),
        );
        let t = table(&out["table"]);
        let want = &z[format!("boosting/{name}/predict").as_str()];
        let got: Vec<f64> = if regression {
            t.rows.iter().map(|r| r[5]).collect()
        } else {
            let k = want.shape[1];
            t.rows.iter().flat_map(|r| r[6..6 + k].to_vec()).collect()
        };
        check(&format!("boosting {name}"), &got, &want.data);
    }

    // 210,000 records: the bins from a subsample of 200,000
    let n = 210_000u64;
    let x: Vec<[f64; 2]> =
        (0..n).map(|i| [(i * 7919 % 100003) as f64 / 100003.0, (i * 104729 % 99991) as f64 / 99991.0]).collect();
    let mut t = Table::new(vec!["x0".into(), "x1".into(), "target".into()]);
    for p in &x {
        t.rows.push(vec![p[0], p[1], p[0] * 3.0 + f64::from(u8::from(p[1] > 0.5)) - p[0] * p[1]]);
    }
    let fit = run(
        "ml.fit",
        vec![("table", Value::Table(t))],
        serde_json::json!({"method": "gradient_boosting", "task": "values", "max_iter": 3, "random_state": 2}),
    );
    let mut rows = Table::new(vec!["x0".into(), "x1".into()]);
    rows.rows = x.iter().step_by(2100).map(|p| p.to_vec()).collect();
    let out =
        run("ml.predict", vec![("model", fit["model"].clone()), ("table", Value::Table(rows))], serde_json::json!({}));
    let got: Vec<f64> = table(&out["table"]).rows.iter().map(|r| r[2]).collect();
    check("boosting of 210,000 records", &got, &z["boosting/big/predict"].data);
}

/// A forest on maps: the cells of a target learnt from a stack, and predicted back.
#[test]
fn a_forest_of_maps() {
    let g = Arc::new(Grid::new(40, 30));
    let n = g.cells();
    let a: Vec<f64> = (0..n).map(|i| (i % 40) as f64).collect();
    let b: Vec<f64> = (0..n).map(|i| (i / 40) as f64).collect();
    let target: Vec<f64> = (0..n).map(|i| if a[i] + b[i] > 35.0 { 2.0 } else { 1.0 }).collect();
    let stack = Stack {
        grid: g.clone(),
        keys: vec![Key::Name("a".into()), Key::Name("b".into())],
        layers: vec![Arc::new(a), Arc::new(b)],
    };
    let fit = run(
        "ml.fit",
        vec![("predictors", Value::Stack(stack.clone())), ("target", Value::Map(Map::new(g.clone(), target.clone())))],
        serde_json::json!({"n_estimators": 20, "sample": 600, "stratify": true}),
    );
    let out = run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("predictors", Value::Stack(stack))],
        serde_json::json!({}),
    );
    let Value::Map(m) = &out["map"] else { panic!() };
    let right = m.data.iter().zip(&target).filter(|(a, b)| a == b).count();
    assert!(right as f64 > 0.97 * n as f64, "{right} of {n}");
    let Value::Stack(p) = &out["probability"] else { panic!() };
    assert_eq!(p.keys, vec![Key::Class(1.0), Key::Class(2.0)]);
}

/// A model of the reference fitted again by ml.fit: classifier/…, regressor/… or boosting/….
fn refit(z: &BTreeMap<String, Array>, meta: &serde_json::Value, model: &str) -> Value {
    let mut params = meta[model].clone();
    let target = match params.as_object_mut().unwrap().remove("target") {
        Some(t) if t == "binary" => "table/binary".to_string(),
        Some(t) => t.as_str().unwrap().to_string(),
        None if model.starts_with("regressor") => "table/value".to_string(),
        None => "table/class".to_string(),
    };
    let regression = target == "table/value";
    params["task"] = serde_json::json!(if regression { "values" } else { "classes" });
    if model.starts_with("boosting") {
        params["method"] = serde_json::json!("gradient_boosting");
    }
    let fit =
        run("ml.fit", vec![("table", Value::Table(records(&z["table/x"], Some(&z[target.as_str()].data))))], params);
    fit["model"].clone()
}

#[test]
fn permutation_importance_is_scikit_learns() {
    let (z, meta) = reference();
    for name in ["classifier", "regressor", "boosting"] {
        let m = &meta[format!("permutation/{name}")];
        let model = refit(&z, &meta, m["model"].as_str().unwrap());
        let y = if name == "regressor" { "table/test_value" } else { "table/test_class" };
        let out = run(
            "ml.importance",
            vec![("model", model), ("table", Value::Table(records(&z["table/test"], Some(&z[y].data))))],
            serde_json::json!({"n_repeats": m["n_repeats"], "random_state": m["random_state"]}),
        );
        let key = |k: &str| &z[format!("permutation/{name}/{k}").as_str()].data;
        let Value::Number(score) = out["score"] else { panic!() };
        check(&format!("{name} score"), &[score], key("score"));
        check(&format!("{name} mean"), &table(&out["importance"]).rows[0], key("mean"));
        check(&format!("{name} sd"), &table(&out["sd"]).rows[0], key("sd"));
        // repeats: a row per shuffle here, a row per predictor in scikit-learn
        let rep = table(&out["repeats"]);
        let r = rep.rows.len();
        let got: Vec<f64> = (0..5).flat_map(|f| rep.rows.iter().map(move |row| row[1 + f])).collect();
        assert_eq!(got.len(), 5 * r);
        check(&format!("{name} repeats"), &got, key("repeats"));
    }
}

#[test]
fn partial_dependence_is_scikit_learns() {
    let (z, meta) = reference();
    for name in ["regressor", "regressor_few", "binary", "multiclass"] {
        let m = &meta[format!("dependence/{name}")];
        let model = refit(&z, &meta, m["model"].as_str().unwrap());
        let feature = m["feature"].as_u64().unwrap();
        let out = run(
            "ml.partial_dependence",
            vec![("model", model), ("table", Value::Table(records(&z["table/test"], None)))],
            serde_json::json!({"predictor": format!("x{feature}"), "grid_resolution": m["grid_resolution"]}),
        );
        let t = table(&out["table"]);
        let grid: Vec<f64> = t.rows.iter().map(|r| r[0]).collect();
        check(&format!("{name} grid"), &grid, &z[format!("dependence/{name}/grid").as_str()].data);
        let want = &z[format!("dependence/{name}/average").as_str()];
        // scikit-learn: a row per class, a column per value of the grid
        let got: Vec<f64> = (1..t.columns.len()).flat_map(|c| t.rows.iter().map(move |r| r[c])).collect();
        check(&format!("{name} average"), &got, &want.data);
    }
}

// -- ml.pca -------------------------------------------------------------------------------------------------------
// (scikit-learn's PCA, tools/make_pca_reference.py: tests/data/reference/pca.json)

fn pca_reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/pca.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn rows_of(v: &serde_json::Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()).collect()
}

fn pca_table(rows: &[Vec<f64>]) -> Table {
    let mut t = Table::new(["a", "b", "c", "d", "e"].map(String::from).to_vec());
    t.rows = rows.to_vec();
    t
}

/// A stack of the columns of some rows, as a grid of `nx` cells across.
fn pca_stack(rows: &[Vec<f64>], nx: usize) -> Stack {
    let g = Arc::new(Grid::new(nx, rows.len() / nx));
    let names = ["a", "b", "c", "d", "e"];
    Stack {
        grid: g,
        keys: names.iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..5).map(|j| Arc::new(rows.iter().map(|r| r[j]).collect())).collect(),
    }
}

fn try_run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value, ctx: &mut Ctx) -> Result<Outputs, String> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(ctx, &args).map_err(|e| e.to_string())
}

/// `got` against `want` to a relative tolerance of the larger absolute value, the sign of `got` flipped to `want`'s.
fn close_up_to_sign(what: &str, got: &[f64], want: &[f64], tol: f64) {
    let dot: f64 = got.iter().zip(want).map(|(a, b)| a * b).sum();
    let sign = if dot < 0.0 { -1.0 } else { 1.0 };
    let big = want.iter().fold(0.0f64, |m, v| m.max(v.abs()));
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        assert!((sign * a - b).abs() <= tol * big.max(1e-300), "{what} at {k}: {} {b}", sign * a);
    }
    assert_eq!(got.len(), want.len(), "{what}: length");
}

fn columns_of(t: &Table, from: usize, count: usize) -> Vec<Vec<f64>> {
    (from..from + count).map(|j| t.rows.iter().map(|r| r[j]).collect()).collect()
}

#[test]
fn pca_is_scikit_learns() {
    let z = pca_reference();
    let (x, other) = (rows_of(&z["x"]), rows_of(&z["other"]));
    for (mode, standardise) in [("correlation", true), ("covariance", false)] {
        let m = &z[mode];
        let fit = run(
            "ml.pca",
            vec![("table", Value::Table(pca_table(&x)))],
            serde_json::json!({"components": 5, "standardise": standardise}),
        );
        // the loadings: component, explained, then a column per variable
        let loadings = table(&fit["loadings"]);
        assert_eq!(loadings.columns, ["component", "explained", "a", "b", "c", "d", "e"]);
        let comps = rows_of(&m["components"]);
        let ratio: Vec<f64> = m["ratio"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let var: Vec<f64> = m["variance"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        let mut signs = Vec::new();
        for c in 0..5 {
            let row = &loadings.rows[c];
            assert_eq!(row[0], (c + 1) as f64);
            assert!((row[1] - ratio[c]).abs() < 1e-12, "{mode} explained {c}");
            close_up_to_sign(&format!("{mode} component {c}"), &row[2..], &comps[c], 1e-8);
            // the largest loading is positive, as metrics.pca's
            let big = (0..5).fold(0, |b, j| if row[2 + j].abs() > row[2 + b].abs() { j } else { b });
            assert!(row[2 + big] > 0.0);
            signs.push(if row[2..].iter().zip(&comps[c]).map(|(a, b)| a * b).sum::<f64>() < 0.0 { -1.0 } else { 1.0 });
        }
        // the variance table: component, variance, ratio, cumulative
        let ex = table(&fit["explained"]);
        assert_eq!(ex.columns, ["component", "variance", "ratio", "cumulative"]);
        let mut cum = 0.0;
        for c in 0..5 {
            cum += ratio[c];
            assert!((ex.rows[c][1] - var[c]).abs() <= 1e-9 * var[0], "{mode} variance {c}");
            assert!((ex.rows[c][2] - ratio[c]).abs() < 1e-12 && (ex.rows[c][3] - cum).abs() < 1e-12);
        }
        // the scores of the records fitted, and of another set with the fit's means and scales (ml.predict)
        let t = table(&fit["table"]);
        assert_eq!(t.columns[5..], ["pc1", "pc2", "pc3", "pc4", "pc5"]);
        let scores = columns_of(t, 5, 5);
        let want = rows_of(&m["scores"]);
        let out = run(
            "ml.predict",
            vec![("model", fit["model"].clone()), ("table", Value::Table(pca_table(&other)))],
            serde_json::json!({}),
        );
        let applied = columns_of(table(&out["table"]), 5, 5);
        let want_other = rows_of(&m["other_scores"]);
        for c in 0..5 {
            let w: Vec<f64> = want.iter().map(|r| r[c] * signs[c]).collect();
            close_up_to_sign(&format!("{mode} scores {c}"), &scores[c], &w, 1e-8);
            let w: Vec<f64> = want_other.iter().map(|r| r[c] * signs[c]).collect();
            close_up_to_sign(&format!("{mode} other scores {c}"), &applied[c], &w, 1e-8);
        }
    }
}

#[test]
fn a_share_of_the_variance_picks_the_components_as_scikit_learn() {
    let z = pca_reference();
    let x = rows_of(&z["x"]);
    for (mode, standardise) in [("correlation", true), ("covariance", false)] {
        for (share, k) in z[mode]["k_share"].as_object().unwrap() {
            let fit = run(
                "ml.pca",
                vec![("table", Value::Table(pca_table(&x)))],
                serde_json::json!({"components": share.parse::<f64>().unwrap(), "standardise": standardise}),
            );
            assert_eq!(table(&fit["loadings"]).rows.len() as u64, k.as_u64().unwrap(), "{mode} {share}");
            assert_eq!(table(&fit["table"]).columns.len(), 5 + k.as_u64().unwrap() as usize);
            // the variance table lists every component
            assert_eq!(table(&fit["explained"]).rows.len(), 5);
        }
    }
    // the default is 0.95
    let fit = run("ml.pca", vec![("table", Value::Table(pca_table(&x)))], serde_json::json!({}));
    assert_eq!(table(&fit["loadings"]).rows.len() as u64, z["correlation"]["k_share"]["0.95"].as_u64().unwrap());
}

// two dates (scikit-learn's PCA of the six layers stacked and of each band's pair, tools/make_pca_reference.py:
// tests/data/reference/pca_dates.json)

fn dates_reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/pca_dates.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The layers a, b, c of some rows (20 cells across), named so.
fn dates_stack(rows: &[Vec<f64>], names: &[&str]) -> Stack {
    Stack {
        grid: Arc::new(if rows.len().is_multiple_of(20) {
            Grid::new(20, rows.len() / 20)
        } else {
            Grid::new(rows.len(), 1)
        }),
        keys: names.iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..names.len()).map(|j| Arc::new(rows.iter().map(|r| r[j]).collect())).collect(),
    }
}

/// The layers of a stack value as columns.
fn layers_of(v: &Value) -> Vec<Vec<f64>> {
    let Value::Stack(s) = v else { panic!("{v:?}") };
    s.layers.iter().map(|l| l.to_vec()).collect()
}

#[test]
fn pca_of_two_dates_stacked_is_scikit_learns_and_tells_the_change() {
    let z = dates_reference();
    let (first, later) = (rows_of(&z["first"]), rows_of(&z["later"]));
    let names = ["a", "b", "c"];
    let mut ctx = Ctx::default();
    let fit = try_run(
        "ml.pca",
        vec![
            ("predictors", Value::Stack(dates_stack(&first, &names))),
            ("later", Value::Stack(dates_stack(&later, &names))),
        ],
        serde_json::json!({"components": 6}),
        &mut ctx,
    )
    .unwrap();
    let m = &z["stacked"];
    let loadings = table(&fit["loadings"]);
    assert_eq!(loadings.columns, ["component", "explained", "a_1", "b_1", "c_1", "a_2", "b_2", "c_2"]);
    let (comps, var, change) = (rows_of(&m["components"]), numbers(&m["variance"]), numbers(&m["change"]));
    let want = rows_of(&m["scores"]);
    let scores = layers_of(&fit["scores"]);
    let ex = table(&fit["explained"]);
    assert_eq!(ex.columns, ["component", "variance", "ratio", "cumulative", "change"]);
    for c in 0..6 {
        close_up_to_sign(&format!("component {c}"), &loadings.rows[c][2..], &comps[c], 1e-8);
        assert!((ex.rows[c][1] - var[c]).abs() <= 1e-9 * var[0], "variance {c}");
        assert!((ex.rows[c][4] - change[c]).abs() < 1e-9, "change {c}: {} {}", ex.rows[c][4], change[c]);
        let w: Vec<f64> = want.iter().map(|r| r[c]).collect();
        close_up_to_sign(&format!("scores {c}"), &scores[c], &w, 1e-8);
    }
    // the component most like a change among those with 1% of the variance: the third
    assert_eq!(ctx.warnings.len(), 1, "{:?}", ctx.warnings);
    assert!(ctx.warnings[0].contains("pc3 is the component most like a change"), "{:?}", ctx.warnings);
    // a share of the variance keeps fewer components, the change column still tells all of them
    let few = run(
        "ml.pca",
        vec![
            ("predictors", Value::Stack(dates_stack(&first, &names))),
            ("later", Value::Stack(dates_stack(&later, &names))),
        ],
        serde_json::json!({"components": 0.9}),
    );
    assert_eq!(table(&few["loadings"]).rows.len(), 3);
    assert_eq!(table(&few["explained"]).rows.len(), 6);
    // the model scores a stack of the six layers by their names
    let both = Stack {
        grid: Arc::new(Grid::new(20, 15)),
        keys: ["a_1", "b_1", "c_1", "a_2", "b_2", "c_2"].iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..6)
            .map(|j| Arc::new((0..300).map(|i| if j < 3 { first[i][j] } else { later[i][j - 3] }).collect()))
            .collect(),
    };
    let out = run(
        "ml.predict",
        vec![("model", few["model"].clone()), ("predictors", Value::Stack(both))],
        serde_json::json!({}),
    );
    let applied = layers_of(&out["scores"]);
    let kept = layers_of(&few["scores"]);
    for c in 0..3 {
        check(&format!("predict {c}"), &applied[c], &kept[c]);
    }
}

#[test]
fn pca_of_two_dates_selective_is_scikit_learns_for_each_pair() {
    let z = dates_reference();
    let (first, later) = (rows_of(&z["first"]), rows_of(&z["later"]));
    let names = ["a", "b", "c"];
    let fit = run(
        "ml.pca",
        vec![
            ("predictors", Value::Stack(dates_stack(&first, &names))),
            ("later", Value::Stack(dates_stack(&later, &names))),
        ],
        serde_json::json!({"change": "selective"}),
    );
    assert!(!fit.contains_key("model"));
    let Value::Stack(s) = &fit["scores"] else { panic!() };
    assert_eq!(s.names(), ["a_pc1", "a_pc2", "b_pc1", "b_pc2", "c_pc1", "c_pc2"]);
    let (loadings, ex) = (table(&fit["loadings"]), table(&fit["explained"]));
    assert_eq!(loadings.columns, ["band", "component", "explained", "first", "later"]);
    assert_eq!(loadings.labels["band"], ["a", "b", "c"]);
    assert_eq!(ex.columns, ["band", "component", "variance", "ratio", "cumulative"]);
    for (b, per) in z["selective"].as_array().unwrap().iter().enumerate() {
        let (comps, var, want) = (rows_of(&per["components"]), numbers(&per["variance"]), rows_of(&per["scores"]));
        for c in 0..2 {
            let row = &loadings.rows[2 * b + c];
            assert_eq!((row[0], row[1]), ((b + 1) as f64, (c + 1) as f64));
            close_up_to_sign(&format!("band {b} component {c}"), &row[3..], &comps[c], 1e-8);
            assert!((ex.rows[2 * b + c][2] - var[c]).abs() <= 1e-9 * var[0], "variance {b} {c}");
            let w: Vec<f64> = want.iter().map(|r| r[c]).collect();
            close_up_to_sign(&format!("band {b} scores {c}"), &s.layers[2 * b + c], &w, 1e-8);
        }
    }
}

#[test]
fn pca_of_two_dates_finds_a_pure_difference_and_leaves_out_missing_cells() {
    // a layer and its negative: the one component is the difference, with all the variance; the same layer twice, the sum
    let mut s = 12345u64;
    let u: Vec<Vec<f64>> = (0..60)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            vec![(s >> 33) as f64 / 1e6]
        })
        .collect();
    let minus: Vec<Vec<f64>> = u.iter().map(|r| vec![-r[0]]).collect();
    let change_of = |later: &[Vec<f64>]| {
        let fit = run(
            "ml.pca",
            vec![
                ("predictors", Value::Stack(dates_stack(&u, &["x"]))),
                ("later", Value::Stack(dates_stack(later, &["x"]))),
            ],
            serde_json::json!({"standardise": false}),
        );
        let ex = table(&fit["explained"]);
        (ex.rows[0][4], ex.rows[0][2])
    };
    let (change, ratio) = change_of(&minus);
    assert!((change - 1.0).abs() < 1e-12 && (ratio - 1.0).abs() < 1e-12, "{change} {ratio}");
    let (change, _) = change_of(&u);
    assert!(change.abs() < 1e-12, "{change}");

    // a cell with none at either date has no scores and does not take part in the fit
    let z = dates_reference();
    let (first, later) = (rows_of(&z["first"]), rows_of(&z["later"]));
    let names = ["a", "b", "c"];
    let mut hole = dates_stack(&later, &names);
    let mut l = hole.layers[1].to_vec();
    l[7] = f64::NAN;
    hole.layers[1] = Arc::new(l);
    let fit = run(
        "ml.pca",
        vec![("predictors", Value::Stack(dates_stack(&first, &names))), ("later", Value::Stack(hole.clone()))],
        serde_json::json!({"components": 6}),
    );
    let scores = layers_of(&fit["scores"]);
    assert!(scores.iter().all(|l| l[7].is_nan() && l.iter().filter(|v| v.is_nan()).count() == 1));
    let (kept_first, kept_later): (Vec<Vec<f64>>, Vec<Vec<f64>>) =
        (0..300).filter(|&i| i != 7).map(|i| (first[i].clone(), later[i].clone())).unzip();
    let mut ctx = Ctx::default();
    let without = try_run(
        "ml.pca",
        vec![
            ("predictors", Value::Stack(dates_stack(&kept_first, &names))),
            ("later", Value::Stack(dates_stack(&kept_later, &names))),
        ],
        serde_json::json!({"components": 6}),
        &mut ctx,
    )
    .unwrap();
    check("loadings", &table(&fit["loadings"]).rows.concat(), &table(&without["loadings"]).rows.concat());
    let selective = run(
        "ml.pca",
        vec![("predictors", Value::Stack(dates_stack(&first, &names))), ("later", Value::Stack(hole))],
        serde_json::json!({"change": "selective"}),
    );
    // (only the band with the hole loses the cell)
    let layers = layers_of(&selective["scores"]);
    assert!(layers[2][7].is_nan() && layers[0][7].is_finite() && layers[4][7].is_finite());
}

#[test]
fn pca_of_two_dates_refuses_what_does_not_fit() {
    let z = dates_reference();
    let (first, later) = (rows_of(&z["first"]), rows_of(&z["later"]));
    let names = ["a", "b", "c"];
    let refused = |inputs: Vec<(&str, Value)>, params: serde_json::Value| {
        try_run("ml.pca", inputs, params, &mut Ctx::default()).map(|_| ()).expect_err("refused")
    };
    let one = Value::Stack(dates_stack(&first, &names));
    // the names differ
    let e = refused(
        vec![("predictors", one.clone()), ("later", Value::Stack(dates_stack(&later, &["a", "b", "z"])))],
        serde_json::json!({}),
    );
    assert!(e.contains("missing in later: \"c\"") && e.contains("not in the first: \"z\""), "{e}");
    // the grids differ
    let mut other = dates_stack(&later, &names);
    other.grid = Arc::new(Grid::new(15, 20));
    let e = refused(vec![("predictors", one.clone()), ("later", Value::Stack(other))], serde_json::json!({}));
    assert!(e.contains("another grid"), "{e}");
    // later goes with maps, not a table
    let e = refused(
        vec![("table", Value::Table(pca_table(&first))), ("later", Value::Stack(dates_stack(&later, &names)))],
        serde_json::json!({}),
    );
    assert!(e.contains("not with a table"), "{e}");
    // later without the first date, change without later, a change that is none
    let e = refused(vec![("later", Value::Stack(dates_stack(&later, &names)))], serde_json::json!({}));
    assert!(e.contains("first date"), "{e}");
    let e = refused(vec![("predictors", one.clone())], serde_json::json!({"change": "selective"}));
    assert!(e.contains("two dates"), "{e}");
    let e = refused(
        vec![("predictors", one), ("later", Value::Stack(dates_stack(&later, &names)))],
        serde_json::json!({"change": "mixed"}),
    );
    assert!(e.contains("stacked or selective"), "{e}");
}

#[test]
fn pca_of_maps_scores_other_maps_with_the_fits_mean() {
    let z = pca_reference();
    let x = rows_of(&z["x"]);
    let stack = pca_stack(&x, 20);
    let n = 400;
    // covariance: the scale is 1, so a shift of one variable moves each score by its loading times the shift
    let fit = run(
        "ml.pca",
        vec![("predictors", Value::Stack(stack.clone()))],
        serde_json::json!({"components": 3, "standardise": false}),
    );
    let Value::Stack(s) = &fit["scores"] else { panic!() };
    assert_eq!(s.keys, vec![Key::Name("pc1".into()), Key::Name("pc2".into()), Key::Name("pc3".into())]);
    // the same scores as the table's
    let by_table = run(
        "ml.pca",
        vec![("table", Value::Table(pca_table(&x)))],
        serde_json::json!({"components": 3, "standardise": false}),
    );
    let tab = columns_of(table(&by_table["table"]), 5, 3);
    for (c, col) in tab.iter().enumerate() {
        check(&format!("pc{}", c + 1), &s.layers[c], col);
    }
    let mut later = stack.clone();
    later.layers[0] = Arc::new(stack.layers[0].iter().map(|v| v + 50.0).collect());
    let out = run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("predictors", Value::Stack(later))],
        serde_json::json!({}),
    );
    let Value::Stack(p) = &out["scores"] else { panic!() };
    let loadings = table(&fit["loadings"]);
    for c in 0..3 {
        let load = loadings.rows[c][2];
        for i in 0..n {
            let d = p.layers[c][i] - s.layers[c][i];
            assert!((d - 50.0 * load).abs() < 1e-9, "pc{} cell {i}: {d} {}", c + 1, 50.0 * load);
        }
    }
}

#[test]
fn pca_leaves_out_nan_and_the_cells_not_asked_for() {
    let z = pca_reference();
    let x = rows_of(&z["x"]);
    let mut stack = pca_stack(&x, 20);
    let mut b = (*stack.layers[1]).clone();
    b[7] = f64::NAN;
    stack.layers[1] = Arc::new(b);
    // the fit on the cells of the first half
    let g = stack.grid.clone();
    let mask = Map::new(g.clone(), (0..400).map(|i| if i < 200 { 1.0 } else { 0.0 }).collect());
    let half = run(
        "ml.pca",
        vec![("predictors", Value::Stack(stack.clone())), ("cells", Value::Map(mask))],
        serde_json::json!({"components": 2}),
    );
    let all = run("ml.pca", vec![("predictors", Value::Stack(stack.clone()))], serde_json::json!({"components": 2}));
    let (Value::Stack(h), Value::Stack(a)) = (&half["scores"], &all["scores"]) else { panic!() };
    // a NaN in any variable is NaN in every score, in the cells off the mask too
    for layer in h.layers.iter().chain(&a.layers) {
        assert!(layer[7].is_nan());
        assert_eq!(layer.iter().filter(|v| v.is_nan()).count(), 1);
    }
    // another mean and scale, so other numbers
    assert!(h.layers[0].iter().zip(a.layers[0].iter()).any(|(p, q)| (p - q).abs() > 1e-3));
    // the half fit equals the fit on the first 200 rows alone
    let mut first = x[..200].to_vec();
    first.remove(7);
    let own = run("ml.pca", vec![("table", Value::Table(pca_table(&first)))], serde_json::json!({"components": 2}));
    check("loadings", &table(&half["loadings"]).rows.concat(), &table(&own["loadings"]).rows.concat());
}

#[test]
fn pca_is_the_same_on_one_thread_or_eight() {
    // more cells than the blocks of the covariance and the pieces of the scores
    let n: usize = 300 * 300;
    let rows: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let f = ((i * 7919) % 1009) as f64 / 1009.0;
            let g = ((i * 104729) % 997) as f64 / 997.0;
            let v = vec![f * 10.0, f * 3.0 + g, 100.0 * g - f, f * f, (f - g) * 1000.0];
            if i % 61 == 0 {
                v.into_iter().enumerate().map(|(j, x)| if j == 2 { f64::NAN } else { x }).collect()
            } else {
                v
            }
        })
        .collect();
    let stack = pca_stack(&rows, 300);
    let on = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let fit = run("ml.pca", vec![("predictors", Value::Stack(stack.clone()))], serde_json::json!({}));
            let again = run(
                "ml.predict",
                vec![("model", fit["model"].clone()), ("predictors", Value::Stack(stack.clone()))],
                serde_json::json!({}),
            );
            (fit, again)
        })
    };
    let ((a, a2), (b, b2)) = (on(1), on(8));
    let layers = |o: &Outputs| match &o["scores"] {
        Value::Stack(s) => s.layers.iter().flat_map(|l| l.iter().copied()).collect::<Vec<f64>>(),
        _ => panic!(),
    };
    check("scores", &layers(&a), &layers(&b));
    check("applied", &layers(&a2), &layers(&b2));
    check("applied as fitted", &layers(&a), &layers(&a2));
    for name in ["loadings", "explained"] {
        check(name, &table(&a[name]).rows.concat(), &table(&b[name]).rows.concat());
    }
}

#[test]
fn pca_on_a_sample_says_so() {
    let z = pca_reference();
    let x = rows_of(&z["x"]);
    let stack = pca_stack(&x, 20);
    let go = |seed: u64| {
        let mut ctx = Ctx::default();
        let out = try_run(
            "ml.pca",
            vec![("predictors", Value::Stack(stack.clone()))],
            serde_json::json!({"sample": 100, "seed": seed, "components": 2}),
            &mut ctx,
        )
        .unwrap();
        (out, ctx.warnings)
    };
    let (a, note) = go(3);
    assert_eq!(note.len(), 1, "{note:?}");
    assert!(note[0].contains("sample of 100 of the 400 records"), "{}", note[0]);
    let (b, _) = go(3);
    let (c, _) = go(4);
    let rows = |o: &Outputs| table(&o["loadings"]).rows.concat();
    check("the same seed", &rows(&a), &rows(&b));
    assert_ne!(rows(&a), rows(&c));
    // a share
    let mut ctx = Ctx::default();
    try_run("ml.pca", vec![("predictors", Value::Stack(stack.clone()))], serde_json::json!({"sample": 0.25}), &mut ctx)
        .unwrap();
    assert!(ctx.warnings[0].contains("sample of 100 of the 400"), "{:?}", ctx.warnings);
    // all of them: no note
    let mut ctx = Ctx::default();
    try_run("ml.pca", vec![("predictors", Value::Stack(stack))], serde_json::json!({}), &mut ctx).unwrap();
    assert!(ctx.warnings.is_empty());
}

#[test]
fn a_pca_model_is_saved_as_json_and_only_scores() {
    let z = pca_reference();
    let (x, other) = (rows_of(&z["x"]), rows_of(&z["other"]));
    let fit = run("ml.pca", vec![("table", Value::Table(pca_table(&x)))], serde_json::json!({"components": 2}));
    let Value::MlModel(model) = &fit["model"] else { panic!() };
    let json = serde_json::to_string(&**model).unwrap();
    assert!(json.starts_with("{\"kind\":\"pca\""), "{}", &json[..40]);
    let back: terraflux_ops::ml::Model = serde_json::from_str(&json).unwrap();
    let apply = |m: Value| {
        let out =
            run("ml.predict", vec![("model", m), ("table", Value::Table(pca_table(&other)))], serde_json::json!({}));
        table(&out["table"]).rows.concat()
    };
    check("the model read back", &apply(Value::MlModel(Arc::new(back))), &apply(fit["model"].clone()));
    // it is not a forest: the inspection of ml.importance does not take it
    let mut t = pca_table(&x);
    t.columns[4] = "target".into();
    let mut ctx = Ctx::default();
    let e = try_run(
        "ml.importance",
        vec![("model", fit["model"].clone()), ("table", Value::Table(t))],
        serde_json::json!({}),
        &mut ctx,
    )
    .unwrap_err();
    assert!(e.contains("a model of ml.fit"), "{e}");
}

#[test]
fn pca_tells_what_is_wrong() {
    let z = pca_reference();
    let x = rows_of(&z["x"]);
    let table_in = || vec![("table", Value::Table(pca_table(&x)))];
    let fails = |inputs: Vec<(&str, Value)>, params: serde_json::Value, want: &str| {
        let e = try_run("ml.pca", inputs, params, &mut Ctx::default()).unwrap_err();
        assert!(e.contains(want), "{want}: {e}");
    };
    fails(table_in(), serde_json::json!({"components": 0}), "components");
    fails(table_in(), serde_json::json!({"components": -2}), "components");
    fails(table_in(), serde_json::json!({"components": 2.5}), "whole number");
    fails(table_in(), serde_json::json!({"components": 6}), "1 to 5");
    fails(table_in(), serde_json::json!({"components": "many"}), "components");
    fails(table_in(), serde_json::json!({"standardise": "yes"}), "standardise");
    fails(table_in(), serde_json::json!({"columns": ["a", "zz"]}), "no column \"zz\"");
    fails(table_in(), serde_json::json!({"columns": ["a", "a"]}), "twice");
    fails(table_in(), serde_json::json!({"columns": []}), "no numeric column");
    fails(table_in(), serde_json::json!({"sample": 0}), "sample");
    fails(table_in(), serde_json::json!({"sample": -1}), "sample");
    fails(vec![], serde_json::json!({}), "connect the records");
    let stack = pca_stack(&x, 20);
    fails(
        vec![("table", Value::Table(pca_table(&x))), ("predictors", Value::Stack(stack.clone()))],
        serde_json::json!({}),
        "not both",
    );
    let g = stack.grid.clone();
    fails(
        vec![("table", Value::Table(pca_table(&x))), ("cells", Value::Map(Map::new(g.clone(), vec![1.0; 400])))],
        serde_json::json!({}),
        "cells goes with maps",
    );
    fails(
        vec![("predictors", Value::Stack(stack.clone())), ("cells", Value::Map(Map::new(g.clone(), vec![0.0; 400])))],
        serde_json::json!({}),
        "no record has every variable",
    );
    fails(
        vec![
            ("predictors", Value::Stack(stack.clone())),
            ("cells", Value::Map(Map::new(Arc::new(Grid::new(2, 2)), vec![1.0; 4]))),
        ],
        serde_json::json!({}),
        "differs in size",
    );
    // a constant variable: standardised or not, it fits
    let mut flat = x.clone();
    flat.iter_mut().for_each(|r| r[3] = 4.0);
    run("ml.pca", vec![("table", Value::Table(pca_table(&flat)))], serde_json::json!({"standardise": false}));
    // a table with a column pc1 already
    let mut t = pca_table(&x);
    t.columns.push("pc1".into());
    t.rows.iter_mut().for_each(|r| r.push(0.0));
    fails(
        vec![("table", Value::Table(t))],
        serde_json::json!({"columns": ["a", "b", "c", "d", "e"]}),
        "already has a column \"pc1\"",
    );
    // ml.predict: a layer or a column the model reads is missing
    let fit = run("ml.pca", table_in(), serde_json::json!({}));
    let mut few = pca_stack(&x, 20);
    few.keys.pop();
    few.layers.pop();
    let e = try_run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("predictors", Value::Stack(few))],
        serde_json::json!({}),
        &mut Ctx::default(),
    )
    .unwrap_err();
    assert!(e.contains("no layer \"e\""), "{e}");
    let mut narrow = pca_table(&x);
    narrow.columns.pop();
    narrow.rows.iter_mut().for_each(|r| {
        r.pop();
    });
    let e = try_run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("table", Value::Table(narrow))],
        serde_json::json!({}),
        &mut Ctx::default(),
    )
    .unwrap_err();
    assert!(e.contains("no column \"e\""), "{e}");
}

// -- ml.kmeans ----------------------------------------------------------------------------------------------------
// (scikit-learn's KMeans, tools/make_kmeans_reference.py: tests/data/reference/kmeans.json)

fn km_reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/kmeans.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn numbers(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

fn km_table(rows: &[Vec<f64>]) -> Table {
    let mut t = Table::new(["a", "b", "c"].map(String::from).to_vec());
    t.rows = rows.to_vec();
    t
}

/// A stack of the three variables of some rows, as a grid of `nx` cells across.
fn km_stack(rows: &[Vec<f64>], nx: usize) -> Stack {
    Stack {
        grid: Arc::new(Grid::new(nx, rows.len() / nx)),
        keys: ["a", "b", "c"].iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..3).map(|j| Arc::new(rows.iter().map(|r| r[j]).collect())).collect(),
    }
}

/// `a` against `b` to a relative tolerance.
fn near(what: &str, a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() <= tol * a.abs().max(b.abs()).max(1e-300), "{what}: {a} {b}");
}

/// The clusters of a column of labels as groups of rows: the same partition whatever the numbers.
fn same_partition(a: &[f64], b: &[f64]) -> bool {
    let mut map = BTreeMap::new();
    let mut back = BTreeMap::new();
    a.len() == b.len()
        && a.iter().zip(b).all(|(x, y)| {
            let (kx, ky) = (x.to_bits(), y.to_bits());
            *map.entry(kx).or_insert(ky) == ky && *back.entry(ky).or_insert(kx) == kx
        })
}

/// The share of records whose cluster in `a` is the one most of that cluster's records have in `b`.
fn agreement(a: &[f64], b: &[f64]) -> f64 {
    let mut count: BTreeMap<(u64, u64), usize> = BTreeMap::new();
    for (x, y) in a.iter().zip(b) {
        *count.entry((x.to_bits(), y.to_bits())).or_insert(0) += 1;
    }
    let mut best: BTreeMap<u64, usize> = BTreeMap::new();
    for ((x, _), c) in &count {
        let e = best.entry(*x).or_insert(0);
        *e = (*e).max(*c);
    }
    best.values().sum::<usize>() as f64 / a.len() as f64
}

fn cluster_column(t: &Table) -> Vec<f64> {
    let j = t.columns.iter().position(|c| c == "cluster").unwrap();
    t.rows.iter().map(|r| r[j]).collect()
}

fn map_data(v: &Value) -> Vec<f64> {
    let Value::Map(m) = v else { panic!("{v:?}") };
    m.data.to_vec()
}

#[test]
fn lloyds_iterations_are_scikit_learns_from_the_same_centres() {
    use terraflux_ops::multivar::kmeans::kmeans_from;
    let z = km_reference();
    let x: Vec<Vec<f64>> = z["x"].as_array().unwrap().iter().map(numbers).collect();
    let (mean, scale) = (numbers(&z["mean"]), numbers(&z["scale"]));
    let flat: Vec<f64> = x.iter().flat_map(|r| (0..3).map(|j| (r[j] - mean[j]) / scale[j])).collect();
    for which in ["plus_plus", "random"] {
        let w = &z[which];
        let init: Vec<f64> = w["init"].as_array().unwrap().iter().flat_map(numbers).collect();
        let km = kmeans_from(&flat, x.len(), 3, init, 300, 1e-4);
        near(&format!("{which} inertia"), km.inertia, w["inertia"].as_f64().unwrap(), 1e-9);
        let labels: Vec<f64> = km.labels.iter().map(|&l| l as f64).collect();
        assert_eq!(labels, numbers(&w["labels"]), "{which}: labels");
        for (a, b) in km.centres.iter().zip(w["centres"].as_array().unwrap().iter().flat_map(numbers)) {
            near(&format!("{which} centre"), *a, b, 1e-9);
        }
        println!("{which}: {} iterations (scikit-learn: {})", km.iterations, w["n_iter"]);
    }
}

#[test]
fn kmeans_of_a_table_is_scikit_learns() {
    let z = km_reference();
    let x: Vec<Vec<f64>> = z["x"].as_array().unwrap().iter().map(numbers).collect();
    let truth = numbers(&z["fits"][2]["labels"]);
    let mut ctx = Ctx::default();
    let fit = try_run(
        "ml.kmeans",
        vec![("table", Value::Table(km_table(&x)))],
        serde_json::json!({"k": [2, 3, 4, 5, 6]}),
        &mut ctx,
    )
    .unwrap();
    // the choice: the inertia, silhouette and index of k = 2 to 4 are scikit-learn's (k above the groups have
    // several optima: only a bound)
    let choice = table(&fit["choice"]);
    assert_eq!(choice.columns, ["k", "records", "inertia", "silhouette", "calinski_harabasz"]);
    assert_eq!(choice.rows.len(), 5);
    for (row, want) in choice.rows.iter().zip(z["fits"].as_array().unwrap()) {
        let k = want["k"].as_f64().unwrap();
        assert_eq!((row[0], row[1]), (k, 600.0));
        if k <= 4.0 {
            near(&format!("k {k} inertia"), row[2], want["inertia"].as_f64().unwrap(), 1e-9);
            near(&format!("k {k} silhouette"), row[3], want["silhouette"].as_f64().unwrap(), 1e-9);
            near(&format!("k {k} index"), row[4], want["calinski_harabasz"].as_f64().unwrap(), 1e-9);
        } else {
            assert!(row[2] <= want["inertia"].as_f64().unwrap() * 1.05, "k {k}: {}", row[2]);
        }
    }
    // the best silhouette is k = 4, said in a note
    assert_eq!(table(&fit["centres"]).rows.len(), 4);
    assert!(ctx.warnings.len() == 1 && ctx.warnings[0].contains("k = 4 has the best silhouette"), "{:?}", ctx.warnings);
    // the clusters are scikit-learn's of k = 4 (the groups overlap a little: not exactly the ones drawn)
    let t = table(&fit["table"]);
    assert_eq!(t.columns, ["a", "b", "c", "cluster"]);
    let labels = cluster_column(t);
    assert!(same_partition(&labels, &truth));
    // the centres: cluster, size, inertia, then the variables in their own units: the mean of the cluster's records
    let centres = table(&fit["centres"]);
    assert_eq!(centres.columns, ["cluster", "size", "inertia", "a", "b", "c"]);
    for (c, row) in centres.rows.iter().enumerate() {
        assert_eq!(row[0], (c + 1) as f64);
        let members: Vec<&Vec<f64>> = x.iter().zip(&labels).filter(|(_, l)| **l == row[0]).map(|(r, _)| r).collect();
        assert_eq!(row[1], members.len() as f64);
        for j in 0..3 {
            let m = members.iter().map(|r| r[j]).sum::<f64>() / members.len() as f64;
            near(&format!("centre {c} variable {j}"), row[3 + j], m, 1e-9);
        }
    }
    assert_eq!(centres.rows.iter().map(|r| r[1]).sum::<f64>(), 600.0);
    let fits = z["fits"].as_array().unwrap();
    near(
        "the inertias add up",
        centres.rows.iter().map(|r| r[2]).sum::<f64>(),
        fits[2]["inertia"].as_f64().unwrap(),
        1e-9,
    );
    // without standardising: the same groups, the inertia of the variables as they are
    let raw = run(
        "ml.kmeans",
        vec![("table", Value::Table(km_table(&x)))],
        serde_json::json!({"k": 4, "standardise": false}),
    );
    assert!(raw.get("choice").is_some_and(|c| table(c).rows.len() == 1));
    // (in these units the variable with the largest numbers decides, and the groups overlap in it: ours is the same
    // optimum or a better one)
    let inertia: f64 = table(&raw["centres"]).rows.iter().map(|r| r[2]).sum();
    assert!(inertia <= z["raw"]["inertia"].as_f64().unwrap() * (1.0 + 1e-9), "{inertia}");
    assert!(inertia > 0.99 * z["raw"]["inertia"].as_f64().unwrap(), "{inertia}");
}

#[test]
fn the_clusters_of_a_synthetic_image_are_its_classes() {
    // five classes in blocks (columns in fifths), the six bands of nlm.image with its noise, no blur
    let v: Vec<f64> = (0..100 * 100).map(|i| ((i % 100) / 20 + 1) as f64).collect();
    let classes = Value::Map(Map::new(Arc::new(Grid::new(100, 100)), v.clone()));
    let image = run("nlm.image", vec![("classes", classes)], serde_json::json!({"blur": 0}));
    let fit = run("ml.kmeans", vec![("predictors", image["image"].clone())], serde_json::json!({"k": 5}));
    let labels = map_data(&fit["clusters"]);
    // each class falls (nearly) in one cluster of its own
    let mut taken = Vec::new();
    for class in 1..=5 {
        let mut count: BTreeMap<u64, f64> = BTreeMap::new();
        let mut n = 0.0f64;
        for (l, t) in labels.iter().zip(&v) {
            if *t == class as f64 {
                *count.entry(l.to_bits()).or_insert(0.0) += 1.0;
                n += 1.0;
            }
        }
        let (best, share) = count.iter().map(|(k, c)| (*k, c / n)).max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        println!("class {class}: cluster {} holds {share:.3}", f64::from_bits(best));
        assert!(share > 0.95, "class {class}: {share}");
        taken.push(best);
    }
    taken.sort();
    taken.dedup();
    assert_eq!(taken.len(), 5, "two classes in one cluster");
    // the table of centres has the bands' names
    assert_eq!(table(&fit["centres"]).columns[3..], ["blue", "green", "red", "nir", "swir1", "swir2"]);
}

#[test]
fn the_cluster_numbers_do_not_depend_on_the_run_or_the_order_of_the_records() {
    let z = km_reference();
    let x: Vec<Vec<f64>> = z["x"].as_array().unwrap().iter().map(numbers).collect();
    let go = |rows: &[Vec<f64>], seed: u64| {
        let fit = run(
            "ml.kmeans",
            vec![("table", Value::Table(km_table(rows)))],
            serde_json::json!({"k": 4, "seed": seed, "n_init": 3}),
        );
        (
            cluster_column(table(&fit["table"])),
            table(&fit["centres"]).rows.iter().map(|r| r[3..].to_vec()).collect::<Vec<_>>(),
        )
    };
    let (first, centres) = go(&x, 1);
    for seed in 2..6 {
        let (labels, c) = go(&x, seed);
        assert_eq!(labels, first, "seed {seed}");
        for (a, b) in c.concat().iter().zip(centres.concat()) {
            near("centre", *a, b, 1e-9);
        }
    }
    // the same records the other way round: each record has the same number
    let back: Vec<Vec<f64>> = x.iter().rev().cloned().collect();
    let (labels, _) = go(&back, 1);
    assert_eq!(labels.iter().rev().copied().collect::<Vec<_>>(), first);
    // numbered by the first principal component of the centres (standardised), its largest loading positive, from
    // the least score: power iteration on the centres' covariance gives it here
    let (mean, scale) = (numbers(&z["mean"]), numbers(&z["scale"]));
    let std: Vec<Vec<f64>> = centres.iter().map(|r| (0..3).map(|j| (r[j] - mean[j]) / scale[j]).collect()).collect();
    let m: Vec<f64> = (0..3).map(|j| std.iter().map(|r| r[j]).sum::<f64>() / 4.0).collect();
    let mut v = vec![1.0, 0.7, 0.3];
    for _ in 0..500 {
        let mut w = [0.0; 3];
        for r in &std {
            let dot: f64 = (0..3).map(|j| (r[j] - m[j]) * v[j]).sum();
            for j in 0..3 {
                w[j] += (r[j] - m[j]) * dot;
            }
        }
        let norm = w.iter().map(|x| x * x).sum::<f64>().sqrt();
        v = w.iter().map(|x| x / norm).collect();
    }
    let big = (0..3).fold(0, |b, j| if v[j].abs() > v[b].abs() { j } else { b });
    let sign = v[big].signum();
    let score: Vec<f64> = std.iter().map(|r| sign * (0..3).map(|j| (r[j] - m[j]) * v[j]).sum::<f64>()).collect();
    assert!(score.windows(2).all(|w| w[0] < w[1]), "{score:?}");
}

#[test]
fn kmeans_is_the_same_on_one_thread_or_eight() {
    // more cells than the blocks of the sums and the pieces of the assignment, four groups in a 3-D space and a few NaN
    let n: usize = 150 * 150;
    let rows: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let g = (i * 7919) % 4;
            let f = ((i * 104729) % 1009) as f64 / 1009.0;
            let h = ((i * 31) % 997) as f64 / 997.0;
            let v = vec![g as f64 * 10.0 + f * 3.0, (g % 2) as f64 * 40.0 + h * 5.0, 100.0 - g as f64 * 7.0 + (f - h)];
            if i % 61 == 0 {
                v.into_iter().enumerate().map(|(j, x)| if j == 1 { f64::NAN } else { x }).collect()
            } else {
                v
            }
        })
        .collect();
    let on = |stack: &Stack, threads: usize, params: serde_json::Value| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            let fit = run("ml.kmeans", vec![("predictors", Value::Stack(stack.clone()))], params.clone());
            let again = run(
                "ml.predict",
                vec![("model", fit["model"].clone()), ("predictors", Value::Stack(stack.clone()))],
                serde_json::json!({}),
            );
            (fit, again)
        })
    };
    // (the silhouette is quadratic in its records, at most 10 000: the k to choose among on fewer cells)
    for (cells, nx, params) in
        [(n, 150, serde_json::json!({"k": 4})), (4900, 70, serde_json::json!({"k": [3, 4, 5], "n_init": 3}))]
    {
        let stack = km_stack(&rows[..cells], nx);
        let ((a, a2), (b, b2)) = (on(&stack, 1, params.clone()), on(&stack, 8, params));
        check("clusters", &map_data(&a["clusters"]), &map_data(&b["clusters"]));
        check("applied", &map_data(&a2["map"]), &map_data(&b2["map"]));
        check("applied as fitted", &map_data(&a["clusters"]), &map_data(&a2["map"]));
        for name in ["centres", "choice"] {
            check(name, &table(&a[name]).rows.concat(), &table(&b[name]).rows.concat());
        }
        // the cells with a NaN have no cluster
        let labels = map_data(&a["clusters"]);
        assert!(labels.iter().enumerate().all(|(i, l)| l.is_nan() == (i % 61 == 0)));
        assert_eq!(labels.len(), cells);
    }
}

#[test]
fn k_auto_finds_the_groups() {
    let z = km_reference();
    let x: Vec<Vec<f64>> = z["x"].as_array().unwrap().iter().map(numbers).collect();
    // as a table and as maps, with no k: 2 to 10, the best silhouette is the four groups
    let mut ctx = Ctx::default();
    let fit =
        try_run("ml.kmeans", vec![("predictors", Value::Stack(km_stack(&x, 30)))], serde_json::json!({}), &mut ctx)
            .unwrap();
    let choice = table(&fit["choice"]);
    assert_eq!(choice.rows.iter().map(|r| r[0]).collect::<Vec<_>>(), (2..=10).map(f64::from).collect::<Vec<_>>());
    let best = choice.rows.iter().max_by(|a, b| a[3].total_cmp(&b[3])).unwrap();
    assert_eq!(best[0], 4.0);
    assert_eq!(table(&fit["centres"]).rows.len(), 4);
    assert!(ctx.warnings[0].contains("k = 4") && ctx.warnings[0].contains("2 to 10"), "{:?}", ctx.warnings);
    let labels = map_data(&fit["clusters"]);
    assert!(same_partition(&labels, &numbers(&z["fits"][2]["labels"])));
    // "auto" said so is the same
    let again = run("ml.kmeans", vec![("table", Value::Table(km_table(&x)))], serde_json::json!({"k": "auto"}));
    assert!(same_partition(&cluster_column(table(&again["table"])), &labels));
    // a list of one has no choice to make, and no note
    let mut ctx = Ctx::default();
    try_run("ml.kmeans", vec![("table", Value::Table(km_table(&x)))], serde_json::json!({"k": [3]}), &mut ctx).unwrap();
    assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
}

#[test]
fn kmeans_on_a_sample_says_so() {
    let z = km_reference();
    let x: Vec<Vec<f64>> = z["x"].as_array().unwrap().iter().map(numbers).collect();
    let stack = km_stack(&x, 30);
    let go = |seed: u64| {
        let mut ctx = Ctx::default();
        let out = try_run(
            "ml.kmeans",
            vec![("predictors", Value::Stack(stack.clone()))],
            serde_json::json!({"k": 4, "sample": 200, "seed": seed}),
            &mut ctx,
        )
        .unwrap();
        (out, ctx.warnings)
    };
    let (a, note) = go(3);
    assert_eq!(note.len(), 1, "{note:?}");
    assert!(note[0].contains("sample of 200 of the 600 records"), "{}", note[0]);
    // every cell has a cluster, from a fit on the sample: still the groups
    let labels = map_data(&a["clusters"]);
    assert!(labels.iter().all(|l| l.is_finite()));
    let share = agreement(&labels, &numbers(&z["fits"][2]["labels"]));
    assert!(share > 0.97, "{share}");
    let (b, _) = go(3);
    check("the same seed", &map_data(&a["clusters"]), &map_data(&b["clusters"]));
    check("the same seed's centres", &table(&a["centres"]).rows.concat(), &table(&b["centres"]).rows.concat());
    // the sizes are of all the cells
    assert_eq!(table(&a["centres"]).rows.iter().map(|r| r[1]).sum::<f64>(), 600.0);
}

#[test]
fn kmeans_leaves_out_nan_and_the_cells_not_asked_for() {
    let z = km_reference();
    let x: Vec<Vec<f64>> = z["x"].as_array().unwrap().iter().map(numbers).collect();
    let mut stack = km_stack(&x, 30);
    let mut b = stack.layers[1].to_vec();
    b[7] = f64::NAN;
    stack.layers[1] = Arc::new(b);
    let g = stack.grid.clone();
    let mut cells = vec![1.0; 600];
    cells[0] = 0.0;
    cells[1] = f64::NAN;
    let fit = run(
        "ml.kmeans",
        vec![("predictors", Value::Stack(stack)), ("cells", Value::Map(Map::new(g, cells)))],
        serde_json::json!({"k": 4}),
    );
    let labels = map_data(&fit["clusters"]);
    for i in [0, 1, 7] {
        assert!(labels[i].is_nan(), "cell {i}");
    }
    assert_eq!(labels.iter().filter(|l| l.is_finite()).count(), 597);
    assert_eq!(table(&fit["centres"]).rows.iter().map(|r| r[1]).sum::<f64>(), 597.0);
    // a table: the row with a NaN has no cluster
    let mut rows = x.clone();
    rows[2][0] = f64::NAN;
    let fit = run("ml.kmeans", vec![("table", Value::Table(km_table(&rows)))], serde_json::json!({"k": 4}));
    let labels = cluster_column(table(&fit["table"]));
    assert!(labels[2].is_nan() && labels.iter().filter(|l| l.is_nan()).count() == 1);
}

#[test]
fn a_kmeans_model_is_saved_as_json_and_gives_the_nearest_centre() {
    let z = km_reference();
    let x: Vec<Vec<f64>> = z["x"].as_array().unwrap().iter().map(numbers).collect();
    let fit = run("ml.kmeans", vec![("table", Value::Table(km_table(&x)))], serde_json::json!({"k": 4}));
    let Value::MlModel(model) = &fit["model"] else { panic!() };
    let json = serde_json::to_string(&**model).unwrap();
    assert!(json.starts_with("{\"kind\":\"kmeans\""), "{}", &json[..40]);
    let back: terraflux_ops::ml::Model = serde_json::from_str(&json).unwrap();
    let apply = |m: Value, rows: &[Vec<f64>]| {
        let out = run("ml.predict", vec![("model", m), ("table", Value::Table(km_table(rows)))], serde_json::json!({}));
        cluster_column(table(&out["table"]))
    };
    // the fitted records get the numbers they had; the model read back is the same
    let as_fitted = cluster_column(table(&fit["table"]));
    assert_eq!(apply(fit["model"].clone(), &x), as_fitted);
    assert_eq!(apply(Value::MlModel(Arc::new(back)), &x), as_fitted);
    // other records: the nearest centre, in the fit's units (its mean and scales, not the new data's). A record
    // at each cluster's centre has that cluster's number.
    let centres = table(&fit["centres"]);
    let at_centres: Vec<Vec<f64>> = centres.rows.iter().map(|r| r[3..].to_vec()).collect();
    assert_eq!(apply(fit["model"].clone(), &at_centres), vec![1.0, 2.0, 3.0, 4.0]);
    // one record alone, far from the fit's mean: still assigned by the centres of the fit
    let far = vec![vec![at_centres[3][0], at_centres[3][1], at_centres[3][2] + 50.0]];
    assert_eq!(apply(fit["model"].clone(), &far), vec![4.0]);
    // maps: a `map`
    let out = run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("predictors", Value::Stack(km_stack(&x, 30)))],
        serde_json::json!({}),
    );
    assert_eq!(map_data(&out["map"]), as_fitted);
    // it is not a forest: the inspection of ml.importance does not take it
    let mut t = km_table(&x);
    t.columns[2] = "target".into();
    let e = try_run(
        "ml.importance",
        vec![("model", fit["model"].clone()), ("table", Value::Table(t))],
        serde_json::json!({}),
        &mut Ctx::default(),
    )
    .unwrap_err();
    assert!(e.contains("a model of ml.fit"), "{e}");
}

#[test]
fn kmeans_tells_what_is_wrong() {
    let z = km_reference();
    let x: Vec<Vec<f64>> = z["x"].as_array().unwrap().iter().map(numbers).collect();
    let table_in = || vec![("table", Value::Table(km_table(&x)))];
    let fails = |inputs: Vec<(&str, Value)>, params: serde_json::Value, want: &str| {
        let e = try_run("ml.kmeans", inputs, params, &mut Ctx::default()).unwrap_err();
        assert!(e.contains(want), "{want}: {e}");
    };
    fails(table_in(), serde_json::json!({"k": 0}), "whole number");
    fails(table_in(), serde_json::json!({"k": -2}), "whole number");
    fails(table_in(), serde_json::json!({"k": 2.5}), "whole number");
    fails(table_in(), serde_json::json!({"k": "many"}), "\"auto\"");
    fails(table_in(), serde_json::json!({"k": []}), "\"auto\"");
    fails(table_in(), serde_json::json!({"k": [1, 3]}), "2 or more");
    fails(table_in(), serde_json::json!({"k": [3, "x"]}), "\"x\"");
    fails(table_in(), serde_json::json!({"k": 601}), "more than the 600 records");
    fails(table_in(), serde_json::json!({"k": 4, "sample": 50, "seed": 1.5}), "seed");
    fails(table_in(), serde_json::json!({"k": 80, "sample": 50}), "more than the 50 records");
    fails(table_in(), serde_json::json!({"n_init": 0}), "n_init");
    fails(table_in(), serde_json::json!({"n_init": -1}), "n_init");
    fails(table_in(), serde_json::json!({"max_iter": 0}), "max_iter");
    fails(table_in(), serde_json::json!({"tol": -1}), "tol");
    fails(table_in(), serde_json::json!({"tol": "small"}), "tol");
    fails(table_in(), serde_json::json!({"standardise": "yes"}), "standardise");
    fails(table_in(), serde_json::json!({"columns": ["a", "zz"]}), "no column \"zz\"");
    fails(table_in(), serde_json::json!({"columns": ["a", "a"]}), "twice");
    fails(table_in(), serde_json::json!({"columns": []}), "no numeric column");
    fails(table_in(), serde_json::json!({"sample": 0}), "sample");
    fails(vec![], serde_json::json!({}), "connect the records");
    let stack = km_stack(&x, 30);
    fails(
        vec![("table", Value::Table(km_table(&x))), ("predictors", Value::Stack(stack.clone()))],
        serde_json::json!({}),
        "not both",
    );
    let g = stack.grid.clone();
    fails(
        vec![("table", Value::Table(km_table(&x))), ("cells", Value::Map(Map::new(g.clone(), vec![1.0; 600])))],
        serde_json::json!({}),
        "cells goes with maps",
    );
    fails(
        vec![("predictors", Value::Stack(stack.clone())), ("cells", Value::Map(Map::new(g, vec![0.0; 600])))],
        serde_json::json!({}),
        "no record has every variable",
    );
    // auto needs three records at least
    fails(vec![("table", Value::Table(km_table(&x[..2])))], serde_json::json!({}), "too few");
    // a constant variable: standardising cannot divide by its deviation, in its own units it is only a null direction
    let mut flat = x.clone();
    flat.iter_mut().for_each(|r| r[1] = 4.0);
    run("ml.kmeans", vec![("table", Value::Table(km_table(&flat)))], serde_json::json!({"standardise": false, "k": 3}));
    // a table with a column cluster already
    let mut t = km_table(&x);
    t.columns.push("cluster".into());
    t.rows.iter_mut().for_each(|r| r.push(0.0));
    fails(
        vec![("table", Value::Table(t))],
        serde_json::json!({"columns": ["a", "b", "c"]}),
        "already has a column \"cluster\"",
    );
    // ml.predict: a layer or a column the model reads is missing
    let fit = run("ml.kmeans", table_in(), serde_json::json!({"k": 3}));
    let mut few = km_stack(&x, 30);
    few.keys.pop();
    few.layers.pop();
    let e = try_run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("predictors", Value::Stack(few))],
        serde_json::json!({}),
        &mut Ctx::default(),
    )
    .unwrap_err();
    assert!(e.contains("no layer \"c\""), "{e}");
    let mut narrow = km_table(&x);
    narrow.columns.pop();
    narrow.rows.iter_mut().for_each(|r| {
        r.pop();
    });
    let e = try_run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("table", Value::Table(narrow))],
        serde_json::json!({}),
        &mut Ctx::default(),
    )
    .unwrap_err();
    assert!(e.contains("no column \"c\""), "{e}");
}

// -- ml.som -------------------------------------------------------------------------------------------------------
// (MiniSom's trainings, tools/make_som_reference.py: tests/data/reference/som.json; the records of kmeans.json)

fn som_reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/som.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The floats a list of their 64 bits stands for.
fn from_bits(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| f64::from_bits(x.as_u64().unwrap())).collect()
}

fn pool(threads: usize) -> rayon::ThreadPool {
    rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap()
}

/// `got` against `want` to MiniSom's tolerance where the C runtime's exp and NumPy's may differ by a unit in the last
/// place (a relative 1e-12 and an absolute 1e-10, as Zeit's tests), else to the bit.
fn same_weights(what: &str, got: &[f64], want: &[f64], exp: bool) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if !exp {
        check(what, got, want);
        return;
    }
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        assert!((a - b).abs() <= 1e-10 + 1e-12 * b.abs(), "{what} at {k}: {a} {b}");
    }
}

/// A map of the weights MiniSom trained (its `x` by `y` nodes, `i * y + j`), as the rows and columns of a `Som`: `y`
/// rows of `x` columns.
fn som_of(trained: &[f64], x: usize, y: usize, d: usize, hex: bool) -> terraflux_ops::multivar::codebook::Som {
    use terraflux_ops::multivar::som::Topology;
    let weights = (0..y * x)
        .map(|k| {
            let (r, col) = (k / x, k % x);
            let at = (col * y + r) * d;
            trained[at..at + d].to_vec()
        })
        .collect();
    terraflux_ops::multivar::codebook::Som {
        predictors: (0..d).map(|j| format!("v{j}")).collect(),
        rows: y,
        columns: x,
        topology: if hex { Topology::Hexagonal } else { Topology::Rectangular },
        mean: vec![0.0; d],
        scale: vec![1.0; d],
        weights,
        groups: vec![],
    }
}

#[test]
fn the_som_engine_is_minisoms() {
    use terraflux_ops::multivar::som::{Grid, Schedule, Topology, predict_bmus, train_batch, train_online};
    let z = som_reference();
    let cases = z["cases"].as_object().unwrap();
    assert!(cases.len() >= 12);
    let usize_of = |v: &serde_json::Value| v.as_u64().unwrap() as usize;
    for (name, c) in cases {
        let k = &c["som"];
        let (x, y, d) = (usize_of(&k["x"]), usize_of(&k["y"]), usize_of(&k["input_len"]));
        let hex = k["topology"].as_str() == Some("hexagonal");
        let name_of = |key: &str| k[key].as_str().unwrap_or("").to_string();
        let (lr_decay, sigma_decay, neighbourhood) =
            (name_of("decay_function"), name_of("sigma_decay_function"), name_of("neighborhood_function"));
        let code = |n: &str| match n {
            s if s.starts_with("inverse") => 1,
            s if s.starts_with("linear") => 2,
            _ => 0,
        };
        let s = Schedule {
            learning_rate: k["learning_rate"].as_f64().unwrap(),
            sigma: k["sigma"].as_f64().unwrap(),
            lr_decay: code(&lr_decay),
            sigma_decay: code(&sigma_decay),
            neighborhood: match neighbourhood.as_str() {
                "mexican_hat" => 1,
                "bubble" => 2,
                "triangle" => 3,
                _ => 0,
            },
        };
        // MiniSom's coordinates of the nodes, and ours
        let (xx, yy) = (from_bits(&c["xx"]), from_bits(&c["yy"]));
        let topology = if hex { Topology::Hexagonal } else { Topology::Rectangular };
        let mine = Grid::minisom(x, y, topology);
        check(&format!("{name}: xx"), mine.coordinates().0, &xx);
        check(&format!("{name}: yy"), mine.coordinates().1, &yy);
        let grid = Grid::new(x, y, &xx, &yy).unwrap();
        let data = from_bits(&c["data"]);
        let start = from_bits(&c["init"]);
        let want = from_bits(&c["weights"]);
        // (the reference was made on Windows, where NumPy's exp is the C runtime's: the same bits there; elsewhere the
        // exp may differ in the last place)
        let exp = !cfg!(windows) && !matches!(neighbourhood.as_str(), "bubble" | "triangle");
        let iterations = usize_of(&c["train"]["num_iteration"]);
        if c["method"] == "train" {
            let order: Vec<usize> = c["order"].as_array().unwrap().iter().map(usize_of).collect();
            let epochs = c["train"]["use_epochs"].as_bool().unwrap_or(false);
            let mut w = start.clone();
            train_online(&mut w, &data, d, &order, iterations, epochs, &s, &grid);
            same_weights(&format!("{name}: online"), &w, &want, exp);
        } else {
            // the batch on one, two and eight threads: the same bits as each other, and as MiniSom's
            let on = |threads: usize| {
                let mut w = start.clone();
                pool(threads).install(|| train_batch(&mut w, &data, d, iterations, &s, &grid));
                w
            };
            let w = on(1);
            same_weights(&format!("{name}: batch"), &w, &want, exp);
            for threads in [2, 8] {
                check(&format!("{name}: {threads} threads"), &on(threads), &w);
            }
        }
        // the winners of the trained map: MiniSom's (the index i * y + j)
        let want_bmus: Vec<usize> = c["winners"].as_array().unwrap().iter().map(usize_of).collect();
        assert_eq!(predict_bmus(&data, d, &want, x * y), want_bmus, "{name}: winners");
        // the errors and the U-matrix of the map MiniSom trained, from its weights
        let som = som_of(&want, x, y, d, hex);
        let (qe, te) = som.quality(&data);
        near(&format!("{name}: quantization error"), qe, c["quantization_error"].as_f64().unwrap(), 1e-9);
        // (MiniSom's is 1 - mean of the neighbours: off by the last bits)
        near(&format!("{name}: topographic error"), te, c["topographic_error"].as_f64().unwrap(), 1e-9);
        // the U-matrix: MiniSom's distance map for a rectangular map (its hexagonal one counts the neighbours of
        // the other way of shifting the rows than its own coordinates do), and for any from MiniSom's coordinates:
        // the nodes at the distance 1 (hexagonal) or at most 1.42 (rectangular, as its topographic error)
        let u = som.umatrix();
        let top = u.iter().fold(0.0f64, |m, v| m.max(*v));
        let dm: Vec<f64> = c["distance_map"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        for r in 0..y {
            for col in 0..x {
                if !hex {
                    near(&format!("{name}: umatrix at {r} {col}"), u[r * x + col] / top, dm[col * y + r], 1e-9);
                }
                let e = col * y + r;
                let dists: Vec<f64> = (0..x * y)
                    .filter(|&f| {
                        let d = ((xx[e] - xx[f]).powi(2) + (yy[e] - yy[f]).powi(2)).sqrt();
                        f != e && if hex { (d - 1.0).abs() <= 1e-8 + 1e-5 } else { d <= 1.42 }
                    })
                    .map(|f| (0..d).map(|j| (want[e * d + j] - want[f * d + j]).powi(2)).sum::<f64>().sqrt())
                    .collect();
                near(
                    &format!("{name}: umatrix at {r} {col} by coordinates"),
                    u[r * x + col],
                    dists.iter().sum::<f64>() / dists.len() as f64,
                    1e-9,
                );
            }
        }
    }
}

/// The three variables of the records of kmeans.json as a stack of a grid `nx` across (the records of four groups).
fn som_records() -> (Vec<Vec<f64>>, Vec<f64>) {
    let z = km_reference();
    (z["x"].as_array().unwrap().iter().map(numbers).collect(), numbers(&z["truth"]))
}

#[test]
fn a_map_of_four_groups_groups_its_nodes_into_them() {
    let (x, truth) = som_records();
    let truth: Vec<f64> = truth.iter().map(|t| t + 1.0).collect();
    for (grouping, topology) in [("ward", "rectangular"), ("kmeans", "hexagonal"), ("ward", "hexagonal")] {
        let mut ctx = Ctx::default();
        let fit = try_run(
            "ml.som",
            vec![("predictors", Value::Stack(km_stack(&x, 30)))],
            serde_json::json!({"rows": 7, "cols": 6, "clusters": 4, "grouping": grouping, "topology": topology}),
            &mut ctx,
        )
        .unwrap();
        // the size is given: no note of it
        assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
        let nodes = map_data(&fit["nodes"]);
        let groups = map_data(&fit["groups"]);
        assert!(nodes.iter().all(|v| (1.0..=42.0).contains(v)));
        assert!(agreement(&groups, &truth) > 0.93, "{grouping} {topology}: {}", agreement(&groups, &truth));
        assert!(agreement(&truth, &groups) > 0.93, "{grouping} {topology}");
        // the codebook: 42 nodes in row-major order, the counts adding to the cells, the groups from 1 to 4
        let book = table(&fit["codebook"]);
        assert_eq!(
            book.columns[..7],
            ["node", "row", "column", "count", "group", "umatrix", "topology"].map(String::from)
        );
        assert_eq!(&book.columns[7..], &["a", "b", "c"].map(String::from));
        let want = if topology == "hexagonal" { 2.0 } else { 1.0 };
        assert!(book.rows.iter().all(|r| r[6] == want));
        assert_eq!(book.label("topology", want), Some(topology));
        assert_eq!(book.rows.len(), 42);
        for (k, r) in book.rows.iter().enumerate() {
            assert_eq!((r[0], r[1], r[2]), ((k + 1) as f64, (k / 6 + 1) as f64, (k % 6 + 1) as f64));
            assert!((1.0..=4.0).contains(&r[4]) && r[5] > 0.0, "{r:?}");
        }
        assert_eq!(book.rows.iter().map(|r| r[3]).sum::<f64>(), 600.0);
        // a cell's group is its node's group
        for (n, g) in nodes.iter().zip(&groups) {
            assert_eq!(book.rows[*n as usize - 1][4], *g);
        }
        // the weights are in the variables' own units: the nodes of the cells near the origin of a (about 100)
        let a_range = book.rows.iter().fold((f64::MAX, f64::MIN), |m, r| (m.0.min(r[7]), m.1.max(r[7])));
        assert!(a_range.0 > -50.0 && a_range.1 < 300.0 && a_range.1 - a_range.0 > 50.0, "{a_range:?}");
        // the quality: 600 records, a small error, the topology mostly kept
        let q = table(&fit["quality"]);
        assert_eq!(q.columns, ["records", "rows", "columns", "quantisation_error", "topographic_error", "empty_nodes"]);
        assert_eq!(q.rows[0][..3], [600.0, 7.0, 6.0]);
        assert!(q.rows[0][3] > 0.0 && q.rows[0][3] < 1.5, "{:?}", q.rows[0]);
        assert!((0.0..=0.6).contains(&q.rows[0][4]), "{:?}", q.rows[0]);
    }
}

#[test]
fn without_a_size_the_map_has_vesantos() {
    let (x, _) = som_records();
    let mut ctx = Ctx::default();
    let fit =
        try_run("ml.som", vec![("table", Value::Table(km_table(&x)))], serde_json::json!({"iterations": 5}), &mut ctx)
            .unwrap();
    // 5 sqrt(600) = 122.5, 123 nodes in a roughly square map
    assert_eq!(ctx.warnings.len(), 1, "{:?}", ctx.warnings);
    assert!(
        ctx.warnings[0].contains("Vesanto's rule") && ctx.warnings[0].contains("= 123 nodes"),
        "{}",
        ctx.warnings[0]
    );
    let q = &table(&fit["quality"]).rows[0];
    let (rows, cols) = (q[1], q[2]);
    assert!(rows >= cols && rows * cols >= 100.0 && rows * cols <= 140.0, "{rows} x {cols}");
    assert_eq!(table(&fit["codebook"]).rows.len() as f64, rows * cols);
    // with a table: the node as a column, and no groups without clusters
    let t = table(&fit["table"]);
    assert_eq!(&t.columns[3..], &["node".to_string()]);
    assert_eq!(t.rows.len(), 600);
    assert!(!fit.contains_key("groups"));
    // a size given says nothing
    let mut ctx = Ctx::default();
    try_run(
        "ml.som",
        vec![("table", Value::Table(km_table(&x)))],
        serde_json::json!({"rows": 3, "cols": 3, "iterations": 2}),
        &mut ctx,
    )
    .unwrap();
    assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
}

#[test]
fn som_is_the_same_on_one_thread_or_eight() {
    // more cells than a piece of the assignment and a chunk of winners, four groups in a 3-D space and a few NaN
    let n: usize = 20_000;
    let rows: Vec<Vec<f64>> = (0..n)
        .map(|i| {
            let g = (i * 7919) % 4;
            let f = ((i * 104729) % 1009) as f64 / 1009.0;
            let h = ((i * 31) % 997) as f64 / 997.0;
            let v = vec![g as f64 * 10.0 + f * 3.0, (g % 2) as f64 * 40.0 + h * 5.0, 100.0 - g as f64 * 7.0 + (f - h)];
            if i % 61 == 0 {
                v.into_iter().enumerate().map(|(j, x)| if j == 1 { f64::NAN } else { x }).collect()
            } else {
                v
            }
        })
        .collect();
    let stack = km_stack(&rows, 200);
    let on = |threads: usize, params: serde_json::Value| {
        pool(threads).install(|| {
            let fit = run("ml.som", vec![("predictors", Value::Stack(stack.clone()))], params.clone());
            let again = run(
                "ml.predict",
                vec![("model", fit["model"].clone()), ("predictors", Value::Stack(stack.clone()))],
                serde_json::json!({}),
            );
            (fit, again)
        })
    };
    for params in [
        serde_json::json!({"rows": 6, "cols": 5, "iterations": 3, "clusters": 4}),
        serde_json::json!({"rows": 5, "cols": 5, "iterations": 1, "training": "online", "topology": "hexagonal", "init": "random", "clusters": 3, "grouping": "kmeans", "sample": 3000}),
    ] {
        let ((a, a2), (b, b2)) = (on(1, params.clone()), on(8, params));
        check("nodes", &map_data(&a["nodes"]), &map_data(&b["nodes"]));
        check("groups", &map_data(&a["groups"]), &map_data(&b["groups"]));
        check("applied", &map_data(&a2["map"]), &map_data(&b2["map"]));
        check("applied groups", &map_data(&a2["groups"]), &map_data(&b2["groups"]));
        check("applied as fitted", &map_data(&a["nodes"]), &map_data(&a2["map"]));
        for name in ["codebook", "quality"] {
            check(name, &table(&a[name]).rows.concat(), &table(&b[name]).rows.concat());
        }
        let nodes = map_data(&a["nodes"]);
        assert!(nodes.iter().enumerate().all(|(i, l)| l.is_nan() == (i % 61 == 0)));
    }
}

#[test]
fn the_same_seed_gives_the_same_map_and_the_groups_are_reproducible() {
    let (x, truth) = som_records();
    let go = |seed: u64, training: &str, init: &str| {
        run(
            "ml.som",
            vec![("table", Value::Table(km_table(&x)))],
            serde_json::json!({"rows": 5, "cols": 5, "iterations": 4, "clusters": 4, "seed": seed, "training": training, "init": init}),
        )
    };
    for (training, init) in [("batch", "random"), ("online", "random"), ("online", "pca")] {
        let (a, b) = (go(3, training, init), go(3, training, init));
        check("nodes", &table(&a["table"]).rows.concat(), &table(&b["table"]).rows.concat());
        check("codebook", &table(&a["codebook"]).rows.concat(), &table(&b["codebook"]).rows.concat());
        let groups = |t: &Value| -> Vec<f64> { table(t).rows.iter().map(|r| r[4]).collect() };
        assert_eq!(groups(&a["table"]), groups(&b["table"]), "{training} {init}");
        if init == "random" {
            // another seed starts elsewhere
            let c = go(4, training, init);
            assert_ne!(table(&a["codebook"]).rows.concat(), table(&c["codebook"]).rows.concat(), "{training}");
        }
    }
    // the groups are numbered by the first principal component of their centres: the same records in another order
    // get the same numbers (a batch's weights do not depend on the order but in the last bits)
    let reversed: Vec<Vec<f64>> = x.iter().rev().cloned().collect();
    let a = run(
        "ml.som",
        vec![("table", Value::Table(km_table(&x)))],
        serde_json::json!({"rows": 5, "cols": 5, "iterations": 20, "clusters": 4}),
    );
    let b = run(
        "ml.som",
        vec![("table", Value::Table(km_table(&reversed)))],
        serde_json::json!({"rows": 5, "cols": 5, "iterations": 20, "clusters": 4}),
    );
    let groups = |t: &Value| -> Vec<f64> { table(t).rows.iter().map(|r| r[4]).collect() };
    let mut back = groups(&b["table"]);
    back.reverse();
    assert_eq!(groups(&a["table"]), back);
    assert!(agreement(&groups(&a["table"]), &truth.iter().map(|t| t + 1.0).collect::<Vec<_>>()) > 0.95);
}

/// By hand: four records of two variables on a line, two near the origin and two near (10, 10), on a map of one row of
/// two nodes with a neighbourhood too narrow to share: the nodes end at the means of the pairs, (0.1, 0.1) and
/// (10.1, 10.1), the distance of each record to its node is sqrt(0.02), the two nodes are next to each other
/// (topographic error 0). A map of one node is the mean: the distances 4, 2, 0 and 6 of 0, 2, 4 and 10 to their
/// mean 4, mean 3 (and no topographic error).
#[test]
fn the_quality_by_hand() {
    let mut t = Table::new(["u", "v"].map(String::from).to_vec());
    t.rows = vec![vec![0.0, 0.0], vec![0.2, 0.2], vec![10.0, 10.0], vec![10.2, 10.2]];
    let fit = run(
        "ml.som",
        vec![("table", Value::Table(t))],
        serde_json::json!({"rows": 1, "cols": 2, "sigma": 0.1, "standardise": false, "iterations": 40}),
    );
    let book = table(&fit["codebook"]);
    // node, row, column, count, umatrix, topology, u, v
    assert_eq!(book.columns[3..], ["count", "umatrix", "topology", "u", "v"].map(String::from));
    for (r, mean) in book.rows.iter().zip([0.1, 10.1]) {
        assert_eq!(r[3], 2.0);
        near("u", r[6], mean, 1e-9);
        near("v", r[7], mean, 1e-9);
    }
    // the U-matrix of two nodes: each one's distance to the other, sqrt(2) * 10
    near("umatrix", book.rows[0][4], 200.0f64.sqrt(), 1e-9);
    let q = &table(&fit["quality"]).rows[0];
    near("quantisation error", q[3], 0.02f64.sqrt(), 1e-9);
    assert_eq!((q[4], q[5]), (0.0, 0.0));
    // one node
    let mut one = Table::new(vec!["u".to_string()]);
    one.rows = vec![vec![0.0], vec![2.0], vec![4.0], vec![10.0]];
    let fit = run(
        "ml.som",
        vec![("table", Value::Table(one))],
        serde_json::json!({"rows": 1, "cols": 1, "standardise": false, "iterations": 3, "init": "random"}),
    );
    let q = &table(&fit["quality"]).rows[0];
    near("one node's error", q[3], 3.0, 1e-9);
    assert!(q[4].is_nan());
    assert!(table(&fit["codebook"]).rows[0][4].is_nan());
}

#[test]
fn som_on_a_sample_says_so() {
    let (x, truth) = som_records();
    let stack = km_stack(&x, 30);
    let mut ctx = Ctx::default();
    let out = try_run(
        "ml.som",
        vec![("predictors", Value::Stack(stack))],
        serde_json::json!({"rows": 5, "cols": 5, "sample": 300, "seed": 2, "clusters": 4}),
        &mut ctx,
    )
    .unwrap();
    assert_eq!(ctx.warnings.len(), 1, "{:?}", ctx.warnings);
    assert!(ctx.warnings[0].contains("sample of 300 of the 600 records"), "{}", ctx.warnings[0]);
    // every cell has its node, from a fit on the sample; the records fitted are 300
    assert!(map_data(&out["nodes"]).iter().all(|v| v.is_finite()));
    assert_eq!(table(&out["quality"]).rows[0][0], 300.0);
    assert_eq!(table(&out["codebook"]).rows.iter().map(|r| r[3]).sum::<f64>(), 600.0);
    assert!(agreement(&map_data(&out["groups"]), &truth.iter().map(|t| t + 1.0).collect::<Vec<_>>()) > 0.9);
}

#[test]
fn som_leaves_out_nan_and_the_cells_not_asked_for() {
    let (x, _) = som_records();
    let mut stack = km_stack(&x, 30);
    let mut b = stack.layers[1].to_vec();
    b[7] = f64::NAN;
    stack.layers[1] = Arc::new(b);
    let g = stack.grid.clone();
    let mut cells = vec![1.0; 600];
    cells[0] = 0.0;
    cells[1] = f64::NAN;
    let fit = run(
        "ml.som",
        vec![("predictors", Value::Stack(stack)), ("cells", Value::Map(Map::new(g, cells)))],
        serde_json::json!({"rows": 4, "cols": 4, "iterations": 3, "clusters": 3}),
    );
    for name in ["nodes", "groups"] {
        let labels = map_data(&fit[name]);
        for i in [0, 1, 7] {
            assert!(labels[i].is_nan(), "{name}: cell {i}");
        }
        assert_eq!(labels.iter().filter(|l| l.is_finite()).count(), 597);
    }
    assert_eq!(table(&fit["codebook"]).rows.iter().map(|r| r[3]).sum::<f64>(), 597.0);
    assert_eq!(table(&fit["quality"]).rows[0][0], 597.0);
    // a table: the row with a NaN has no node, nor group
    let mut rows = x.clone();
    rows[2][0] = f64::NAN;
    let fit = run(
        "ml.som",
        vec![("table", Value::Table(km_table(&rows)))],
        serde_json::json!({"rows": 4, "cols": 4, "iterations": 3, "clusters": 3}),
    );
    let t = table(&fit["table"]);
    assert_eq!(&t.columns[3..], &["node".to_string(), "group".to_string()]);
    assert!(t.rows[2][3].is_nan() && t.rows[2][4].is_nan());
    assert_eq!(t.rows.iter().filter(|r| r[3].is_nan()).count(), 1);
}

#[test]
fn a_som_model_is_saved_as_json_and_gives_the_nearest_node_and_group() {
    let (x, _) = som_records();
    for topology in ["rectangular", "hexagonal"] {
        let fit = run(
            "ml.som",
            vec![("table", Value::Table(km_table(&x)))],
            serde_json::json!({"rows": 5, "cols": 4, "iterations": 5, "clusters": 3, "topology": topology}),
        );
        let Value::MlModel(model) = &fit["model"] else { panic!() };
        let json = serde_json::to_string(&**model).unwrap();
        assert!(json.starts_with("{\"kind\":\"som\""), "{}", &json[..40]);
        assert!(json.contains(&format!("\"topology\":\"{topology}\"")));
        let back: terraflux_ops::ml::Model = serde_json::from_str(&json).unwrap();
        // to the bit: the same JSON again
        assert_eq!(serde_json::to_string(&back).unwrap(), json);
        let apply = |m: Value, rows: &[Vec<f64>]| {
            let out =
                run("ml.predict", vec![("model", m), ("table", Value::Table(km_table(rows)))], serde_json::json!({}));
            table(&out["table"]).clone()
        };
        let as_fitted = table(&fit["table"]).clone();
        assert_eq!(apply(fit["model"].clone(), &x), as_fitted);
        assert_eq!(apply(Value::MlModel(Arc::new(back)), &x), as_fitted);
        // a record at a node's weights has that node, and its group; the weights are the codebook's, own units
        let book = table(&fit["codebook"]);
        let at_nodes: Vec<Vec<f64>> = book.rows.iter().map(|r| r[7..].to_vec()).collect();
        let t = apply(fit["model"].clone(), &at_nodes);
        for (k, r) in t.rows.iter().enumerate() {
            assert_eq!((r[3], r[4]), ((k + 1) as f64, book.rows[k][4]), "node {k}");
        }
        // maps: `map` and `groups`
        let out = run(
            "ml.predict",
            vec![("model", fit["model"].clone()), ("predictors", Value::Stack(km_stack(&x, 30)))],
            serde_json::json!({}),
        );
        assert_eq!(map_data(&out["map"]), as_fitted.rows.iter().map(|r| r[3]).collect::<Vec<_>>());
        assert_eq!(map_data(&out["groups"]), as_fitted.rows.iter().map(|r| r[4]).collect::<Vec<_>>());
        // it is not a forest
        let mut tt = km_table(&x);
        tt.columns[2] = "target".into();
        let e = try_run(
            "ml.importance",
            vec![("model", fit["model"].clone()), ("table", Value::Table(tt))],
            serde_json::json!({}),
            &mut Ctx::default(),
        )
        .unwrap_err();
        assert!(e.contains("a model of ml.fit"), "{e}");
    }
    // without groups, no groups output nor column
    let fit = run(
        "ml.som",
        vec![("predictors", Value::Stack(km_stack(&x, 30)))],
        serde_json::json!({"rows": 3, "cols": 3, "iterations": 2}),
    );
    assert!(!fit.contains_key("groups"));
    let out = run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("predictors", Value::Stack(km_stack(&x, 30)))],
        serde_json::json!({}),
    );
    assert!(!out.contains_key("groups"));
    assert_eq!(map_data(&out["map"]), map_data(&fit["nodes"]));
}

#[test]
fn som_tells_what_is_wrong() {
    let (x, _) = som_records();
    let table_in = || vec![("table", Value::Table(km_table(&x)))];
    let fails = |inputs: Vec<(&str, Value)>, params: serde_json::Value, want: &str| {
        let e = try_run("ml.som", inputs, params, &mut Ctx::default()).unwrap_err();
        assert!(e.contains(want), "{want}: {e}");
    };
    fails(table_in(), serde_json::json!({"rows": 3}), "rows and cols go together");
    fails(table_in(), serde_json::json!({"cols": 3}), "rows and cols go together");
    fails(table_in(), serde_json::json!({"rows": 0, "cols": 3}), "1 or more");
    fails(table_in(), serde_json::json!({"rows": -2, "cols": 3}), "rows");
    fails(table_in(), serde_json::json!({"rows": 70, "cols": 70}), "at most 4096");
    fails(table_in(), serde_json::json!({"topology": "square"}), "topology \"square\": one of rectangular, hexagonal");
    fails(table_in(), serde_json::json!({"neighbourhood": "mexican_hat"}), "neighbourhood");
    fails(table_in(), serde_json::json!({"decay": "fast"}), "decay");
    fails(table_in(), serde_json::json!({"training": "slow"}), "training");
    fails(table_in(), serde_json::json!({"init": "zero"}), "init");
    fails(table_in(), serde_json::json!({"grouping": "average"}), "grouping");
    fails(table_in(), serde_json::json!({"iterations": 0}), "iterations");
    fails(table_in(), serde_json::json!({"iterations": -1}), "iterations");
    fails(table_in(), serde_json::json!({"learning_rate": 0}), "learning_rate");
    fails(table_in(), serde_json::json!({"learning_rate": 1.5}), "learning_rate");
    fails(table_in(), serde_json::json!({"sigma": 0}), "sigma");
    fails(table_in(), serde_json::json!({"sigma": -1}), "sigma");
    fails(table_in(), serde_json::json!({"sigma": "wide"}), "sigma");
    fails(table_in(), serde_json::json!({"rows": 2, "cols": 2, "clusters": 5}), "more than the 4 nodes");
    fails(table_in(), serde_json::json!({"clusters": -1}), "clusters");
    fails(table_in(), serde_json::json!({"standardise": "yes"}), "standardise");
    fails(table_in(), serde_json::json!({"columns": ["a", "zz"]}), "no column \"zz\"");
    fails(table_in(), serde_json::json!({"columns": ["a", "a"]}), "twice");
    fails(table_in(), serde_json::json!({"sample": 0}), "sample");
    fails(vec![], serde_json::json!({}), "connect the records");
    let stack = km_stack(&x, 30);
    fails(
        vec![("table", Value::Table(km_table(&x))), ("predictors", Value::Stack(stack.clone()))],
        serde_json::json!({}),
        "not both",
    );
    let g = stack.grid.clone();
    fails(
        vec![("table", Value::Table(km_table(&x))), ("cells", Value::Map(Map::new(g.clone(), vec![1.0; 600])))],
        serde_json::json!({}),
        "cells goes with maps",
    );
    fails(
        vec![("predictors", Value::Stack(stack.clone())), ("cells", Value::Map(Map::new(g, vec![0.0; 600])))],
        serde_json::json!({}),
        "no record has every variable",
    );
    // a constant variable: standardising cannot divide by its deviation
    let mut flat = x.clone();
    flat.iter_mut().for_each(|r| r[1] = 4.0);
    run(
        "ml.som",
        vec![("table", Value::Table(km_table(&flat)))],
        serde_json::json!({"standardise": false, "rows": 3, "cols": 3, "iterations": 2}),
    );
    // a table with a column node already
    let mut t = km_table(&x);
    t.columns.push("node".into());
    t.rows.iter_mut().for_each(|r| r.push(0.0));
    fails(
        vec![("table", Value::Table(t))],
        serde_json::json!({"columns": ["a", "b", "c"]}),
        "already has a column \"node\"",
    );
    // one variable: a note that the pca start is not possible
    let mut one = Table::new(vec!["u".to_string()]);
    one.rows = (0..50).map(|i| vec![i as f64]).collect();
    let mut ctx = Ctx::default();
    try_run(
        "ml.som",
        vec![("table", Value::Table(one))],
        serde_json::json!({"rows": 3, "cols": 3, "iterations": 2}),
        &mut ctx,
    )
    .unwrap();
    assert!(ctx.warnings.iter().any(|w| w.contains("init pca needs two variables")), "{:?}", ctx.warnings);
    // ml.predict: a layer or a column the model reads is missing
    let fit = run("ml.som", table_in(), serde_json::json!({"rows": 3, "cols": 3, "iterations": 2}));
    let mut few = km_stack(&x, 30);
    few.keys.pop();
    few.layers.pop();
    let e = try_run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("predictors", Value::Stack(few))],
        serde_json::json!({}),
        &mut Ctx::default(),
    )
    .unwrap_err();
    assert!(e.contains("no layer \"c\""), "{e}");
    let mut narrow = km_table(&x);
    narrow.columns.pop();
    narrow.rows.iter_mut().for_each(|r| {
        r.pop();
    });
    let e = try_run(
        "ml.predict",
        vec![("model", fit["model"].clone()), ("table", Value::Table(narrow))],
        serde_json::json!({}),
        &mut Ctx::default(),
    )
    .unwrap_err();
    assert!(e.contains("no column \"c\""), "{e}");
}

/// The records without their second column, a variable of its own kept constant in `flat`.
fn without_b(rows: &[Vec<f64>]) -> Vec<Vec<f64>> {
    rows.iter().map(|r| vec![r[0], r[2]]).collect()
}

fn two_table(rows: &[Vec<f64>]) -> Table {
    let mut t = Table::new(["a", "c"].map(String::from).to_vec());
    t.rows = rows.to_vec();
    t
}

fn constant_b(x: &[Vec<f64>]) -> Vec<Vec<f64>> {
    let mut flat = x.to_vec();
    flat.iter_mut().for_each(|r| r[1] = 4.0);
    flat
}

fn two_stack(rows: &[Vec<f64>], nx: usize) -> Stack {
    Stack {
        grid: Arc::new(Grid::new(nx, rows.len() / nx)),
        keys: ["a", "c"].iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..2).map(|j| Arc::new(rows.iter().map(|r| r[j]).collect())).collect(),
    }
}

fn says_constant(ctx: &Ctx) {
    let w = ctx.warnings.iter().filter(|w| w.contains("\"b\" constant: they weigh nothing")).count();
    assert_eq!(w, 1, "{:?}", ctx.warnings);
}

#[test]
fn kmeans_standardises_a_constant_column_as_scale_one_and_says_it() {
    let x = som_records().0;
    let (flat, params) = (constant_b(&x), serde_json::json!({"k": 3}));
    let mut ctx = Ctx::default();
    let a = try_run("ml.kmeans", vec![("table", Value::Table(km_table(&flat)))], params.clone(), &mut ctx).unwrap();
    says_constant(&ctx);
    let mut ctx = Ctx::default();
    let b =
        try_run("ml.kmeans", vec![("table", Value::Table(two_table(&without_b(&flat))))], params, &mut ctx).unwrap();
    assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
    assert!(same_partition(&cluster_column(table(&a["table"])), &cluster_column(table(&b["table"]))));
}

#[test]
fn som_standardises_a_constant_column_as_scale_one_and_says_it() {
    let x = som_records().0;
    let flat = constant_b(&x);
    let params = serde_json::json!({"rows": 4, "cols": 4, "clusters": 3, "iterations": 2});
    let mut ctx = Ctx::default();
    let a =
        try_run("ml.som", vec![("predictors", Value::Stack(km_stack(&flat, 30)))], params.clone(), &mut ctx).unwrap();
    says_constant(&ctx);
    let mut ctx = Ctx::default();
    let b = try_run("ml.som", vec![("predictors", Value::Stack(two_stack(&without_b(&flat), 30)))], params, &mut ctx)
        .unwrap();
    assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
    assert_eq!(map_data(&a["nodes"]), map_data(&b["nodes"]));
    assert_eq!(map_data(&a["groups"]), map_data(&b["groups"]));
}

#[test]
fn pca_standardises_a_constant_column_as_scale_one_and_says_it() {
    let x = som_records().0;
    let flat = constant_b(&x);
    let params = serde_json::json!({"components": 2});
    let mut ctx = Ctx::default();
    let a = try_run("ml.pca", vec![("table", Value::Table(km_table(&flat)))], params.clone(), &mut ctx).unwrap();
    says_constant(&ctx);
    let b = run("ml.pca", vec![("table", Value::Table(two_table(&without_b(&flat))))], params);
    let (la, lb) = (table(&a["loadings"]), table(&b["loadings"]));
    for (ra, rb) in la.rows.iter().zip(&lb.rows) {
        // component, explained, then a, b, c: the constant b loads nothing
        assert_eq!(ra[3], 0.0);
        near("explained", ra[1], rb[1], 1e-12);
        near("a", ra[2].abs(), rb[2].abs(), 1e-9);
        near("c", ra[4].abs(), rb[3].abs(), 1e-9);
    }
}
