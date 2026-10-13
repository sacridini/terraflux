//! Habitat graphs of raster patches against landschaft's (tests/data/reference/conn_raster.npz and .json,
//! tools/make_conn_raster_reference.py): the nodes, the distances between centroids and edges, least-cost and
//! commute-time distances over a resistance with a barrier, a graph of the patches and a node importance mapped
//! back onto them, to the bit.

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
fn raster_graphs_are_landschafts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let z = terraflux_raster::npy::read_npz(root.join("tests/data/reference/conn_raster.npz")).unwrap();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("tests/data/reference/conn_raster.json")).unwrap())
            .unwrap();
    let shape = &z["in/classes"].shape;
    let (ny, nx) = (shape[0], shape[1]);
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, 30.0, 0.0, ny as f64 * 30.0, 0.0, -30.0];
    let grid = Arc::new(g);
    let map = |name: &str| Value::Map(Map::new(grid.clone(), z[&format!("in/{name}")].data.clone()));
    for c in cases.as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let inputs: Vec<(&str, Value)> =
            c["inputs"].as_object().unwrap().iter().map(|(k, v)| (k.as_str(), map(v.as_str().unwrap()))).collect();
        let t = if c["op"] == "conn.node_importance" {
            let graph = run("conn.graph_from_raster", inputs, serde_json::json!({"habitat": 1, "area_unit": "ha"}))
                .remove("graph")
                .unwrap();
            let Value::HabitatGraph(hg) = &graph else { panic!() };
            check("habitat area", &[hg.habitat_area.unwrap()], &z["graph_habitat_area"].data);
            let out = run(c["op"].as_str().unwrap(), vec![("graph", graph.clone())], c["params"].clone());
            let mapped = run(
                "conn.to_map",
                vec![("graph", graph), ("table", out["table"].clone())],
                serde_json::json!({"column": "dPC"}),
            );
            let Value::Map(m) = &mapped["map"] else { panic!() };
            check("to_map", &m.data, &z["graph_map"].data);
            out
        } else {
            run(c["op"].as_str().unwrap(), inputs, c["params"].clone())
        };
        let Value::Table(t) = &t["table"] else { panic!("{name}: no table") };
        let cols: Vec<String> = c["columns"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
        assert_eq!(t.columns, cols, "{name}: columns");
        let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
        check(name, &got, &z[name].data);
    }
}
