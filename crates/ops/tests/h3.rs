//! H3 planning units against landschaft's h3_units (tests/data/reference/h3_units.json, tools/make_h3_reference.py):
//! the same hexagons over the cost map of landschaft's optimize cases, their costs to the bit; and a problem of them
//! solved. Skipped without GDAL (the coordinates go to longitude and latitude and back by PROJ).

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Value};
use terraflux_raster::vector::Column;

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

fn run(op: &str, ins: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let args = Args {
        node: "n",
        inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        params: params.as_object().unwrap(),
    };
    Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

#[test]
fn h3_units_are_landschafts() {
    if let Err(e) = terraflux_gdal::gdal() {
        eprintln!("skipped: {e}");
        return;
    }
    let reference: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/h3_units.json")).unwrap())
            .unwrap();
    let d = terraflux_raster::npy::read_npz(root().join("tests/data/reference/optimize_inputs.npz")).unwrap();
    let (ny, nx) = (d["pu_cost"].shape[0], d["pu_cost"].shape[1]);
    let mut grid = terraflux_raster::Grid::new(nx, ny);
    grid.transform = [0.0, 100.0, 0.0, ny as f64 * 100.0, 0.0, -100.0];
    grid.geo.wkt = Some(reference["crs"].as_str().unwrap().to_string());
    let grid = Arc::new(grid);
    let cost = Map::new(grid.clone(), d["pu_cost"].data.clone());
    for case in reference["cases"].as_array().unwrap() {
        let res = case["resolution"].as_i64().unwrap();
        let out = run("prioritize.h3_units", vec![("cost", Value::Map(cost.clone()))], json!({"resolution": res}));
        let Value::Vector(units) = &out["units"] else { panic!() };
        let Some(Column::Texts(ids)) = units.column("h3") else { panic!("no h3 column") };
        let want: Vec<String> =
            case["h3"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        assert_eq!(ids, &want, "resolution {res}: the hexagons");
        let costs = units.column("cost").unwrap().numbers();
        for (i, (a, b)) in costs.iter().zip(case["cost"].as_array().unwrap()).enumerate() {
            let b = b.as_f64().unwrap();
            assert!(a.to_bits() == b.to_bits(), "resolution {res}: hexagon {i}'s cost {a} != {b}");
        }
        // they tile the map: their costs add up to its cost
        let total: f64 = cost.data.iter().filter(|v| v.is_finite()).sum();
        assert!((costs.iter().sum::<f64>() - total).abs() <= 1e-5 * total);
    }
    // a problem of hexagons: every target met
    let f = &d["features"];
    let features = Value::Stack(Stack {
        grid: grid.clone(),
        keys: (1..=5).map(|k| Key::Name(format!("f{k}"))).collect(),
        layers: f.data.chunks(ny * nx).map(|c| Arc::new(c.to_vec())).collect(),
    });
    let units = run("prioritize.h3_units", vec![("cost", Value::Map(cost))], json!({"resolution": 9}));
    let out = run(
        "prioritize.optimize",
        vec![("features", features), ("polygons", units["units"].clone())],
        json!({"cost_column": "cost", "relative_targets": 0.2, "gap": 0.0}),
    );
    let Value::Table(coverage) = &out["coverage"] else { panic!("{:?}", out.keys().collect::<Vec<_>>()) };
    assert!(coverage.column("met").unwrap().iter().all(|&m| m == 1.0), "every target met");
}
