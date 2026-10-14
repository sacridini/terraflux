//! diffeR's comparisons at coarser resolutions against landschaft's (tests/data/reference/differ.npz and .json,
//! tools/make_differ_reference.py): memberships, the composite cross-tabulation, differenceMR (factors, percent,
//! a population), MAD (strata, resolutions) and sample2pop, to the bit.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;

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

fn rows(v: &Value) -> Vec<f64> {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t.rows.iter().flatten().copied().collect()
}

fn table(cols: &[&str], data: &[f64]) -> Value {
    let mut t = Table::new(cols.iter().map(|s| s.to_string()).collect());
    t.rows = data.chunks(cols.len()).map(|r| r.to_vec()).collect();
    Value::Table(t)
}

#[test]
fn differ_is_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("differ.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("differ.json")).unwrap()).unwrap();
    let mut g = Grid::new(45, 37);
    g.transform = [1000.0, 30.0, 0.0, 5000.0, 0.0, -30.0];
    let g = Arc::new(g);
    let map = |k: &str| Value::Map(Map::new(g.clone(), z[k].data.clone()));
    let (a, b) = (map("in/a"), map("in/b"));
    for f in ["2", "3"] {
        let out = run(
            "change.memberships",
            vec![("map", a.clone())],
            serde_json::json!({"factor": f.parse::<u64>().unwrap()}),
        );
        let Value::Stack(s) = &out["stack"] else { panic!() };
        assert_eq!(s.layers.len(), meta["memberships"][f].as_array().unwrap().len());
        let got: Vec<f64> = s.layers.iter().flat_map(|l| l.iter().copied()).collect();
        check(&format!("memberships {f}"), &got, &z[format!("memberships/{f}").as_str()].data);
    }
    for f in ["2", "3", "5"] {
        let out = run(
            "change.composite",
            vec![("comp", a.clone()), ("ref", b.clone())],
            serde_json::json!({"factor": f.parse::<u64>().unwrap()}),
        );
        let Value::Table(t) = &out["table"] else { panic!() };
        let got: Vec<f64> = t.rows.iter().flat_map(|r| r[1..].iter().copied()).collect();
        check(&format!("composite {f}"), &got, &z[format!("composite/{f}").as_str()].data);
    }
    let pop = table(&["category", "size"], &z["in/population"].data);
    for (name, s) in meta["resolutions"].as_object().unwrap() {
        let mut p = s.clone();
        let mut inputs = vec![("comp", a.clone()), ("ref", b.clone())];
        if p.as_object_mut().unwrap().remove("population").is_some() {
            inputs.push(("population", pop.clone()));
        }
        let out = run("change.resolutions", inputs, p);
        check(&format!("resolutions {name}"), &rows(&out["table"]), &z[format!("resolutions/{name}").as_str()].data);
    }
    for (name, s) in meta["mad"].as_object().unwrap() {
        let mut inputs = vec![("grid1", map("in/c1")), ("grid2", map("in/c2"))];
        if s["strata"].as_bool().unwrap() {
            inputs.push(("strata", map("in/strata")));
        }
        let out = run("change.mad", inputs, serde_json::json!({"multiple": s["multiple"]}));
        check(&format!("mad {name}"), &rows(&out["table"]), &z[format!("mad/{name}").as_str()].data);
    }
    let ct = &z["in/crosstab"];
    let cols: Vec<String> =
        std::iter::once("from".to_string()).chain((1..ct.shape[1]).map(|k| k.to_string())).collect();
    let cols: Vec<&str> = cols.iter().map(|s| s.as_str()).collect();
    let out = run(
        "change.sample_to_population",
        vec![("matrix", table(&cols, &ct.data)), ("population", pop)],
        serde_json::json!({}),
    );
    let Value::Table(t) = &out["table"] else { panic!() };
    let got: Vec<f64> = t.rows.iter().flat_map(|r| r[1..].iter().copied()).collect();
    check("sample_to_population", &got, &z["sample_to_population"].data);
}
