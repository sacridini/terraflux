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
    // a constant variable: the correlation cannot standardise it, the covariance can
    let mut flat = x.clone();
    flat.iter_mut().for_each(|r| r[3] = 4.0);
    fails(vec![("table", Value::Table(pca_table(&flat)))], serde_json::json!({}), "\"d\" is constant");
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
