//! The matrices and rasters around prioritizr's problems against landschaft's
//! (tests/data/reference/prioritize_tools.npz, the polygons in prioritize_tools.gpkg,
//! tools/make_prioritize_tools_reference.py): boundary, adjacency, connectivity and proximity matrices of cells and
//! polygons, the decay of distances, unit points, coarsen and refine, a binary stack and the protected cells; to
//! the bit (the decay, whose numpy has an exponential of its own, to 1e-15).

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::Geometry;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn check(what: &str, got: &[f64], want: &[f64], tol: f64) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        let same = a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) || (a - b).abs() <= tol * b.abs();
        assert!(same, "{what}: at {k}: {a} {b}");
    }
}

fn flat(v: &Value) -> Vec<f64> {
    match v {
        Value::Table(t) => t.rows.iter().flatten().copied().collect(),
        Value::Map(m) => m.data.to_vec(),
        Value::Stack(s) => s.layers.iter().flat_map(|l| l.iter().copied()).collect(),
        v => panic!("{v:?}"),
    }
}

#[test]
fn tools_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("prioritize_tools.npz")).unwrap();
    let t = &z["in/transform"].data;
    let mut g = Grid::new(90, 60);
    g.transform = [t[0], t[1], t[2], t[3], t[4], t[5]];
    let g = Arc::new(g);
    let map = |k: &str| Value::Map(Map::new(g.clone(), z[k].data.clone()));
    let n = 90 * 60;
    let fz = &z["in/features"];
    let features = Value::Stack(Stack {
        grid: g.clone(),
        keys: (0..fz.shape[0]).map(|k| Key::Name(format!("f{k}"))).collect(),
        layers: (0..fz.shape[0]).map(|k| Arc::new(fz.data[k * n..(k + 1) * n].to_vec())).collect(),
    });
    let layer = |k: usize| Value::Map(Map::new(g.clone(), fz.data[k * n..(k + 1) * n].to_vec()));
    let cost = map("in/cost");
    let polys =
        Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("prioritize_tools.gpkg"), Some("units")).unwrap()));

    for (key, op, inputs, params) in [
        ("boundary", "prioritize.boundary_matrix", vec![("units", cost.clone())], serde_json::json!({})),
        ("boundary_polygons", "prioritize.boundary_matrix", vec![("units", polys.clone())], serde_json::json!({})),
        ("adjacency", "prioritize.adjacency_matrix", vec![("units", cost.clone())], serde_json::json!({})),
        (
            "adjacency_8",
            "prioritize.adjacency_matrix",
            vec![("units", cost.clone())],
            serde_json::json!({"directions": 8}),
        ),
        ("adjacency_polygons", "prioritize.adjacency_matrix", vec![("units", polys.clone())], serde_json::json!({})),
        (
            "connectivity",
            "prioritize.connectivity_matrix",
            vec![("units", cost.clone()), ("y", layer(0))],
            serde_json::json!({}),
        ),
        (
            "connectivity_column",
            "prioritize.connectivity_matrix",
            vec![("units", polys.clone())],
            serde_json::json!({"column": "y"}),
        ),
        (
            "connectivity_raster",
            "prioritize.connectivity_matrix",
            vec![("units", polys.clone()), ("y", layer(2))],
            serde_json::json!({}),
        ),
        (
            "proximity",
            "prioritize.proximity_matrix",
            vec![("units", cost.clone())],
            serde_json::json!({"distance": 1500}),
        ),
        (
            "proximity_polygons",
            "prioritize.proximity_matrix",
            vec![("units", polys.clone())],
            serde_json::json!({"distance": 2500}),
        ),
    ] {
        let out = run(op, inputs, params);
        check(key, &flat(&out["table"]), &z[key].data, 0.0);
    }
    for (key, units) in [("points", cost.clone()), ("points_polygons", polys.clone())] {
        let out = run("prioritize.unit_points", vec![("units", units)], serde_json::json!({}));
        let Value::Vector(v) = &out["points"] else { panic!() };
        let got: Vec<f64> =
            v.geometries.iter().flat_map(|g| if let Geometry::Point(p) = g { p.to_vec() } else { vec![] }).collect();
        check(key, &got, &z[key].data, 0.0);
    }
    let d = &z["in/distances"];
    let mut dt = Table::new((0..d.shape[1]).map(|k| k.to_string()).collect());
    dt.rows = d.data.chunks(d.shape[1]).map(|r| r.to_vec()).collect();
    for (key, params) in [
        ("decay", serde_json::json!({"median": 3000})),
        ("decay_min", serde_json::json!({"median": 5000, "min_probability": 0.05})),
    ] {
        let out = run("prioritize.decay_connectivity", vec![("distances", Value::Table(dt.clone()))], params);
        check(key, &flat(&out["table"]), &z[key].data, 1e-15);
    }
    let out = run(
        "prioritize.coarsen",
        vec![("cost", cost.clone()), ("features", features)],
        serde_json::json!({"factor": 4}),
    );
    check("coarse cost", &flat(&out["cost"]), &z["coarse_cost"].data, 0.0);
    check("coarse features", &flat(&out["features"]), &z["coarse_features"].data, 0.0);
    let Value::Map(cc) = &out["cost"] else { panic!() };
    let sel = Value::Map(Map::new(cc.grid.clone(), z["in/selection"].data.clone()));
    let out = run("prioritize.refine", vec![("selection", sel), ("like", cost.clone())], serde_json::json!({}));
    check("refined", &flat(&out["map"]), &z["refined"].data, 0.0);
    let out =
        run("prioritize.binary_stack", vec![("zones", map("in/zones"))], serde_json::json!({"names": ["a", "b"]}));
    check("binary", &flat(&out["stack"]), &z["binary"].data, 0.0);
    let out = run("prioritize.protected_cells", vec![("solution", map("in/solution"))], serde_json::json!({}));
    check("protected", &flat(&out["map"]), &z["protected"].data, 0.0);
}
