//! Global sensitivity analysis against SALib 1.6 and scipy.stats.qmc (tests/data/reference/sensitivity.npz,
//! tools/make_sensitivity_reference.py): scipy's scrambled and plain Sobol' points, Morris's trajectories and
//! analysis at 4 and 6 levels, Saltelli's runs of the Ishigami function and Sobol's indices with and without the
//! second order; to the bit.

use std::path::Path;

use terraflux_ops::sensitivity::sobol_points;
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};
use terraflux_raster::npy::Array;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

fn table(v: &Value) -> &Table {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t
}

fn reference() -> std::collections::BTreeMap<String, Array> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    terraflux_raster::npy::read_npz(dir.join("sensitivity.npz")).unwrap()
}

/// The design's values without the run column, row by row.
fn values(t: &Table) -> Vec<f64> {
    t.rows.iter().flat_map(|r| r[1..].to_vec()).collect()
}

fn results(y: &[f64]) -> Value {
    Value::Table(Table {
        columns: vec!["output".into()],
        rows: y.iter().map(|v| vec![*v]).collect(),
        ..Default::default()
    })
}

#[test]
fn scipys_sobol_points() {
    let z = reference();
    let flat = |p: Vec<Vec<f64>>| p.into_iter().flatten().collect::<Vec<_>>();
    check("scrambled", &flat(sobol_points(6, 16, true, 7).unwrap()), &z["qmc/scrambled"].data);
    check("plain", &flat(sobol_points(6, 8, false, 0).unwrap()), &z["qmc/plain"].data);
}

#[test]
fn morris_is_salibs() {
    let z = reference();
    let factors = serde_json::json!([["a", 0, 1], ["b", -1, 2], ["c", 0, 10], ["d", 1, 3], ["e", -5, 5]]);
    for levels in [4, 6] {
        let key = |k: &str| &z[format!("morris/{levels}/{k}").as_str()].data;
        let d = run(
            "sensitivity.design",
            vec![],
            serde_json::json!({"method": "morris", "factors": factors, "n": 10, "levels": levels, "seed": 3}),
        );
        check(&format!("morris {levels} design"), &values(table(&d["table"])), key("x"));
        let a = run(
            "sensitivity.analyze",
            vec![("design", d["table"].clone()), ("results", results(key("y")))],
            serde_json::json!({"method": "morris", "levels": levels, "resamples": 50, "seed": 5}),
        );
        let t = table(&a["indices"]);
        for (c, k) in ["mu", "mu_star", "sigma", "mu_star_conf"].iter().enumerate() {
            let got: Vec<f64> = t.rows.iter().map(|r| r[1 + c]).collect();
            check(&format!("morris {levels} {k}"), &got, key(k));
        }
    }
}

#[test]
fn sobol_is_salibs() {
    let z = reference();
    let pi = std::f64::consts::PI;
    let factors = serde_json::json!([["x1", -pi, pi], ["x2", -pi, pi], ["x3", -pi, pi]]);
    for (tag, second) in [("second", true), ("first", false)] {
        let key = |k: &str| &z[format!("sobol/{tag}/{k}").as_str()].data;
        let d = run(
            "sensitivity.design",
            vec![],
            serde_json::json!({"method": "sobol", "factors": factors, "n": 64, "second_order": second, "seed": 11}),
        );
        check(&format!("sobol {tag} design"), &values(table(&d["table"])), key("x"));
        let a = run(
            "sensitivity.analyze",
            vec![("design", d["table"].clone()), ("results", results(key("y")))],
            serde_json::json!({"method": "sobol", "second_order": second, "resamples": 40, "seed": 13}),
        );
        let t = table(&a["indices"]);
        for (c, k) in ["S1", "S1_conf", "ST", "ST_conf"].iter().enumerate() {
            let got: Vec<f64> = t.rows.iter().map(|r| r[1 + c]).collect();
            check(&format!("sobol {tag} {k}"), &got, key(k));
        }
        if second {
            for k in ["S2", "S2_conf"] {
                let got: Vec<f64> =
                    table(&a[k.to_lowercase().as_str()]).rows.iter().flat_map(|r| r[1..].to_vec()).collect();
                check(&format!("sobol {k}"), &got, key(k));
            }
        }
    }
}

#[test]
fn scipys_normal_quantile() {
    let z = reference();
    let got: Vec<f64> = z["ndtri/p"].data.iter().map(|p| terraflux_ops::sensitivity::ndtri(*p)).collect();
    check("ndtri", &got, &z["ndtri/x"].data);
}
