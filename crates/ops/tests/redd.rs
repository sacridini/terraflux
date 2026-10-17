//! Verra's VT0007 against Clark Labs' UDef-ARP (tests/data/reference/redd.npz and .json,
//! tools/make_redd_reference.py, which runs UDef-ARP's own functions): the negligible risk threshold, the benchmark
//! and alternative vulnerability classes, the relative frequency table and the fitted density, the allocation to a
//! period's deforestation and to an expected area (a region the fit lacks filled from its class), to the bit (the
//! adjustment ratio to 1e-14: UDef-ARP reads its table back from a CSV); the
//! assessment grid's cells and agreement to float32's rounding (UDef-ARP sums and measures in float32).

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

fn near(what: &str, got: f64, want: f64, tol: f64) {
    assert!((got - want).abs() <= tol * want.abs().max(1.0), "{what}: {got} {want}");
}

fn map(v: &Value) -> &Map {
    let Value::Map(m) = v else { panic!("{v:?}") };
    m
}

fn table(v: &Value) -> &Table {
    let Value::Table(t) = v else { panic!("{v:?}") };
    t
}

#[test]
fn vt0007_is_udef_arps() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("redd.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("redd.json")).unwrap()).unwrap();
    let shape = &z["in/distance"].shape;
    let mut g = Grid::new(shape[1], shape[0]);
    g.transform = serde_json::from_value(meta["transform"].clone()).unwrap();
    let g = Arc::new(g);
    let m = |k: &str| Value::Map(Map::new(g.clone(), z[k].data.clone()));

    let out = run(
        "redd.nrt",
        vec![("distance", m("in/distance")), ("deforestation", m("in/cal")), ("mask", m("in/mask"))],
        serde_json::json!({}),
    );
    let Value::Number(nrt) = out["nrt"] else { panic!() };
    assert_eq!(nrt, meta["nrt"].as_f64().unwrap());

    let out = run(
        "redd.vulnerability",
        vec![("distance", m("in/distance")), ("nrt", Value::Number(nrt)), ("mask", m("in/mask"))],
        serde_json::json!({}),
    );
    let classes = map(&out["classes"]).clone();
    check("benchmark", &classes.data, &z["benchmark"].data);
    let out = run(
        "redd.vulnerability",
        vec![("vulnerability", m("in/vulnerability")), ("forest", m("in/forest")), ("mask", m("in/mask"))],
        serde_json::json!({}),
    );
    check("alternative", &map(&out["classes"]).data, &z["alternative"].data);

    let fit = run(
        "redd.fit",
        vec![
            ("classes", Value::Map(classes.clone())),
            ("deforestation", m("in/cal")),
            ("subdivisions", m("in/regions")),
        ],
        serde_json::json!({}),
    );
    // UDef-ARP's table: ID, deforested, cells, frequency
    let t = table(&fit["table"]);
    let got: Vec<f64> = t.rows.iter().flat_map(|r| [r[0], r[3], r[4], r[5]]).collect();
    check("fit table", &got, &z["fit/table"].data);
    check("fit density", &map(&fit["density"]).data, &z["fit/density"].data);

    // the prediction's table lacks a region (class 3 of subdivision 4): it is filled from the class's others
    let dropped = meta["dropped"][0].as_f64().unwrap();
    let mut lacking = t.clone();
    lacking.rows.retain(|r| r[0] != dropped);
    for (name, target) in [("cnf", Some(m("in/cnf"))), ("vp", None)] {
        let mut inputs = vec![
            ("classes", Value::Map(classes.clone())),
            ("table", Value::Table(lacking.clone())),
            ("subdivisions", m("in/regions")),
        ];
        match target {
            Some(d) => inputs.push(("deforestation", d)),
            None => inputs.push(("expected", Value::Number(meta["expected"].as_f64().unwrap()))),
        }
        let out = run("redd.allocate", inputs, serde_json::json!({"max_iterations": meta["max_iterations"]}));
        check(&format!("{name} density"), &map(&out["density"]).data, &z[format!("{name}/density").as_str()].data);
        let info = table(&out["info"]);
        // UDef-ARP reads its table back from a CSV with pandas' fast parser (a last bit off at times)
        near(&format!("{name} adjustment ratio"), info.rows[0][0], meta["ar"][name].as_f64().unwrap(), 1e-14);
        assert_eq!(info.rows[0][1], meta["iterations"][name].as_f64().unwrap(), "{name} iterations");
        assert_eq!(info.rows[0][4], 1.0, "{name} filled");
    }

    // the assessment of the confirmation
    let a = &meta["assess"];
    let out = run(
        "redd.assess",
        vec![("density", m("cnf/density")), ("deforestation", m("in/cnf")), ("mask", m("in/mask"))],
        serde_json::json!({"grid_area": a["grid_area"]}),
    );
    let t = table(&out["table"]);
    let cells = &z["assess/cells"];
    assert_eq!(t.rows.len(), cells.shape[0]);
    for (k, r) in t.rows.iter().enumerate() {
        let w = &cells.data[k * 4..k * 4 + 4];
        assert_eq!([r[1], r[2]], [w[0], w[1]], "cell {k}: its centre");
        assert_eq!(r[3], w[2], "cell {k}: actual");
        near(&format!("cell {k}: predicted"), r[4], w[3], 1e-6);
    }
    let mt = table(&out["metrics"]);
    let col = |n: &str| mt.column(n).unwrap()[0];
    assert_eq!(col("samples"), a["samples"].as_f64().unwrap());
    for (ours, theirs) in [
        ("medae", "medae"),
        ("mae", "mae"),
        ("iou_percent", "iou"),
        ("agreement", "agreement"),
        ("difference", "difference"),
    ] {
        near(ours, col(ours), a[theirs].as_f64().unwrap(), 1e-5);
    }
    near("r2", col("r2"), a["r2"].as_f64().unwrap(), 1e-5);
    near("ols slope", col("ols_slope"), a["ols"][0].as_f64().unwrap(), 1e-5);
    near("ols intercept", col("ols_intercept"), a["ols"][1].as_f64().unwrap(), 1e-5);
    near("theil-sen slope", col("theil_sen_slope"), a["theil_sen"][0].as_f64().unwrap(), 1e-5);
    near("theil-sen intercept", col("theil_sen_intercept"), a["theil_sen"][1].as_f64().unwrap(), 1e-5);
}
