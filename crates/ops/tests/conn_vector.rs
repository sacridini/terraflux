//! Habitat graphs of vector features against landschaft's (tests/data/reference/conn_vector.gpkg, .npz and
//! .json, tools/make_conn_vector_reference.py): the polygons' areas and weighted attributes, the distances
//! between centroids (C-shaped polygons and multipolygons included), edges and least-cost distances, a graph of
//! the features and a node importance joined back onto them. GEOS through GDAL.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
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

#[test]
fn vector_graphs_are_landschafts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("conn_vector.npz")).unwrap();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("conn_vector.json")).unwrap()).unwrap();
    let features =
        Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("conn_vector.gpkg"), Some("nodes")).unwrap()));
    let r = &z["in/resistance"];
    let mut g = Grid::new(r.shape[1], r.shape[0]);
    g.transform = [0.0, 30.0, 0.0, r.shape[0] as f64 * 30.0, 0.0, -30.0];
    let resistance = Value::Map(Map::new(Arc::new(g), r.data.clone()));
    let resistance_grid = |ny: usize, nx: usize| {
        let mut g = Grid::new(nx, ny);
        g.transform = [0.0, 30.0, 0.0, ny as f64 * 30.0, 0.0, -30.0];
        Arc::new(g)
    };
    let a = &z["in/absorption"];
    let absorption = Value::Map(Map::new(resistance_grid(r.shape[0], r.shape[1]), a.data.clone()));
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        if c["op"] == "conn.settlement_probabilities" || name == "samc_graph" {
            // Eigen's sparse LU against scipy's SuperLU: to rounding
            let out = if name == "samc_graph" {
                let graph = run(
                    "conn.graph_from_samc",
                    vec![("vector", features.clone()), ("resistance", resistance.clone())],
                    serde_json::json!({"dispersal_distance": 200.0, "area_unit": "ha"}),
                )
                .remove("graph")
                .unwrap();
                run("conn.overall", vec![("graph", graph)], c["params"].clone())
            } else {
                let mut params = c["params"].clone();
                let mut inputs = vec![("vector", features.clone()), ("resistance", resistance.clone())];
                if params.as_object_mut().unwrap().remove("absorption").is_some() {
                    inputs.push(("absorption", absorption.clone()));
                }
                run("conn.settlement_probabilities", inputs, params)
            };
            let Value::Table(t) = &out["table"] else { panic!("{name}: no table") };
            let cols: Vec<String> =
                c["columns"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
            assert_eq!(t.columns, cols, "{name}: columns");
            let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
            let want = &z[name].data;
            assert_eq!(got.len(), want.len(), "{name}");
            for (k, (a, b)) in got.iter().zip(want).enumerate() {
                assert!(
                    a.to_bits() == b.to_bits() || (a - b).abs() <= 1e-10 * a.abs().max(b.abs()),
                    "{name} at {k}: {a} {b}"
                );
            }
            continue;
        }
        let out = if c["op"] == "conn.node_importance" {
            let graph = run(
                "conn.graph_from_vector",
                vec![("vector", features.clone())],
                serde_json::json!({"id": "pid", "distance": "edge", "area_unit": "ha"}),
            )
            .remove("graph")
            .unwrap();
            let Value::HabitatGraph(hg) = &graph else { panic!() };
            check("habitat area", &[hg.habitat_area.unwrap()], &z["graph_habitat_area"].data);
            let out = run("conn.node_importance", vec![("graph", graph.clone())], c["params"].clone());
            let joined =
                run("conn.to_vector", vec![("graph", graph), ("table", out["table"].clone())], serde_json::json!({}));
            let Value::Vector(v) = &joined["vector"] else { panic!() };
            assert_eq!(v.len(), 14);
            out
        } else {
            let mut inputs = vec![("vector", features.clone())];
            if c["params"]["distance"] == "least-cost" {
                inputs.push(("resistance", resistance.clone()));
            }
            run(c["op"].as_str().unwrap(), inputs, c["params"].clone())
        };
        let Value::Table(t) = &out["table"] else { panic!("{name}: no table") };
        let cols: Vec<String> = c["columns"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
        assert_eq!(t.columns, cols, "{name}: columns");
        let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
        check(name, &got, &z[name].data);
    }
}
