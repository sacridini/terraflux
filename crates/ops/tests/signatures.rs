//! Spatial signatures against landschaft's SpatialSignatureAnalysis (tests/data/reference/lsm_signatures.json,
//! tools/make_signature_reference.py): the table of square zones of augusta.tif to the bit, numpy's generator to
//! the bit, the clusters label by label, the components, factors and means to LAPACK's rounding.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::signatures::NumpyRng;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};

fn reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/lsm_signatures.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn num(v: &serde_json::Value) -> f64 {
    v.as_f64().unwrap_or(f64::NAN)
}

fn matrix(v: &serde_json::Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(num).collect()).collect()
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn table(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port} is no table") };
    t.clone()
}

/// landschaft's signature table, its zones as keys.
fn signatures() -> Table {
    let r = reference();
    let mut columns = vec!["zone".to_string()];
    columns.extend(r["columns"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()));
    let rows = r["zones"]
        .as_array()
        .unwrap()
        .iter()
        .zip(matrix(&r["table"]))
        .map(|(z, row)| std::iter::once(num(z)).chain(row).collect())
        .collect();
    Table { columns, rows }
}

/// Values equal up to a relative tolerance (and an absolute one for those near zero).
fn assert_close(ours: &[Vec<f64>], theirs: &[Vec<f64>], rtol: f64, atol: f64, what: &str) {
    assert_eq!(ours.len(), theirs.len(), "{what}: rows");
    for (i, (o, t)) in ours.iter().zip(theirs).enumerate() {
        assert_eq!(o.len(), t.len(), "{what}: row {i}");
        for (j, (a, b)) in o.iter().zip(t).enumerate() {
            assert!((a - b).abs() <= atol + rtol * b.abs(), "{what}[{i}][{j}]: {a} against {b}");
        }
    }
}

/// The columns of a table but the keys.
fn values(t: &Table, keys: usize) -> Vec<Vec<f64>> {
    t.rows.iter().map(|r| r[keys..].to_vec()).collect()
}

#[test]
fn numpy_generator_is_numpys() {
    let r = &reference()["rng"];
    let seed = r["seed"].as_u64().unwrap();
    let mut g = NumpyRng::new(seed);
    for raw in r["raw"].as_array().unwrap() {
        assert_eq!(g.next_u64(), raw.as_str().unwrap().parse::<u64>().unwrap());
    }
    let mut g = NumpyRng::new(seed);
    for (n, want) in [1u64, 5, 100, 7, (1 << 31) + 5, 3].iter().zip(r["integers"].as_array().unwrap()) {
        assert_eq!(g.integers(*n), want.as_u64().unwrap(), "integers({n})");
    }
    for want in r["random"].as_array().unwrap() {
        assert_eq!(g.random(), num(want));
    }
    let p = [0.1, 0.0, 0.25, 0.4, 0.25];
    for want in r["choice"].as_array().unwrap() {
        assert_eq!(g.choice(&p) as u64, want.as_u64().unwrap());
    }
}

/// The signatures of square zones of augusta.tif: landscape metrics and pland per class, to the bit.
#[test]
fn signature_table_is_landschafts() {
    let r = reference();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/tests/data/lsm/augusta.tif");
    let raster = terraflux_raster::Raster::read(&path).unwrap();
    let map = Map::new(Arc::new(raster.grid.clone()), raster.bands[0].masked(raster.nodata));
    let out = run(
        "metrics.signatures",
        vec![("maps", Value::Map(map))],
        serde_json::json!({
            "grid": r["grid"],
            "landscape_metrics": r["landscape_metrics"],
            "class_metrics": r["class_metrics"],
        }),
    );
    let t = table(&out, "table");
    assert_eq!(t, signatures());
}

#[test]
fn pca_is_landschafts() {
    let r = reference();
    let out = run("metrics.pca", vec![("table", Value::Table(signatures()))], serde_json::json!({}));
    let scores = table(&out, "scores");
    assert_eq!(scores.columns[0], "zone");
    assert_close(&values(&scores, 1), &matrix(&r["pca"]["scores"]), 1e-10, 1e-12, "scores");
    let loadings = table(&out, "loadings");
    assert_eq!(
        loadings.columns[2..],
        *r["pca"]["columns"].as_array().unwrap().iter().map(|c| c.as_str().unwrap().to_string()).collect::<Vec<_>>()
    );
    let explained: Vec<Vec<f64>> = loadings.rows.iter().map(|row| vec![row[1]]).collect();
    let want: Vec<Vec<f64>> = r["pca"]["explained"].as_array().unwrap().iter().map(|v| vec![num(v)]).collect();
    assert_close(&explained, &want, 1e-12, 1e-15, "explained");
    // a metric per row in landschaft, a component per row here
    let ours: Vec<Vec<f64>> = values(&loadings, 2);
    let theirs = matrix(&r["pca"]["loadings"]);
    let theirs: Vec<Vec<f64>> = (0..ours.len()).map(|c| theirs.iter().map(|row| row[c]).collect()).collect();
    assert_close(&ours, &theirs, 1e-10, 1e-12, "loadings");
}

#[test]
fn nmf_is_landschafts() {
    for case in reference()["nmf"].as_array().unwrap() {
        let k = case["k"].as_u64().unwrap();
        let out = run("metrics.nmf", vec![("table", Value::Table(signatures()))], serde_json::json!({"factors": k}));
        assert_close(&values(&table(&out, "scores"), 1), &matrix(&case["scores"]), 1e-12, 1e-14, "scores");
        let theirs = matrix(&case["components"]);
        let theirs: Vec<Vec<f64>> = (0..k as usize).map(|c| theirs.iter().map(|row| row[c]).collect()).collect();
        assert_close(&values(&table(&out, "components"), 1), &theirs, 1e-12, 1e-14, "components");
        let Value::Number(err) = out["error"] else { panic!() };
        assert!((err - num(&case["error"])).abs() <= 1e-13 * err, "error {err}");
        let Value::Integer(it) = out["iterations"] else { panic!() };
        assert_eq!(it as u64, case["n_iter"].as_u64().unwrap(), "k = {k}");
    }
}

#[test]
fn clusters_are_landschafts() {
    for case in reference()["cluster"].as_array().unwrap() {
        let mut p = serde_json::json!({"k": case["k"], "method": case["method"]});
        for (from, to) in [("seed", "seed"), ("n_init", "n_init"), ("n_components", "components")] {
            if !case[from].is_null() {
                p[to] = case[from].clone();
            }
        }
        let out = run("metrics.cluster", vec![("table", Value::Table(signatures()))], p.clone());
        let t = table(&out, "table");
        let labels: Vec<u64> = t.rows.iter().map(|r| *r.last().unwrap() as u64).collect();
        let want: Vec<u64> = case["labels"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
        assert_eq!(labels, want, "{p}");
    }
}

#[test]
fn clustergrams_are_landschafts() {
    for case in reference()["clustergram"].as_array().unwrap() {
        let p = serde_json::json!({
            "method": case["method"], "pca_weighted": case["pca_weighted"], "k_min": 1, "k_max": 6,
        });
        let out = run("metrics.clustergram", vec![("table", Value::Table(signatures()))], p.clone());
        assert_eq!(values(&table(&out, "labels"), 1), matrix(&case["labels"]), "{p}");
        assert_close(&values(&table(&out, "means"), 1), &matrix(&case["means"]), 1e-10, 1e-12, &p.to_string());
    }
}
