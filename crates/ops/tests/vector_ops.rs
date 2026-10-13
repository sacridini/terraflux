//! vector.extract against landschaft's extract (to the bit: nearest, bilinear, statistics in buffers), and
//! vector.buffer, vector.clip and vector.dissolve against geopandas (the same features kept, their areas: GEOS
//! through GDAL, shapely's GEOS), on the grid and polygons of landschaft's optimize cases
//! (tests/data/reference/vector_ops.json, tools/make_vector_ops_reference.py). Skipped without GDAL.

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use terraflux_ops::{Args, Ctx, Key, Outputs, Registry, Stack, Value};
use terraflux_raster::vector::{Column, Geometry, Vector};

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

fn vector(out: &Outputs) -> Arc<Vector> {
    let Value::Vector(v) = &out["vector"] else { panic!() };
    v.clone()
}

fn floats(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()
}

/// Shoelace areas of polygons (holes subtracted).
fn area(g: &Geometry) -> f64 {
    g.polygons()
        .iter()
        .map(|poly| {
            poly.iter()
                .enumerate()
                .map(|(k, r)| {
                    let a: f64 = r.windows(2).map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1]).sum::<f64>().abs() / 2.0;
                    if k == 0 { a } else { -a }
                })
                .sum::<f64>()
        })
        .sum()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * b.abs().max(1.0)
}

#[test]
fn vector_operations_are_landschafts_and_geopandas() {
    if let Err(e) = terraflux_gdal::gdal() {
        eprintln!("skipped: {e}");
        return;
    }
    let r: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/vector_ops.json")).unwrap())
            .unwrap();
    let d = terraflux_raster::npy::read_npz(root().join("tests/data/reference/optimize_inputs.npz")).unwrap();
    let f = &d["features"];
    let (ny, nx) = (f.shape[1], f.shape[2]);
    let mut grid = terraflux_raster::Grid::new(nx, ny);
    grid.transform = [0.0, 100.0, 0.0, ny as f64 * 100.0, 0.0, -100.0];
    grid.geo.wkt = Some("EPSG:32723".into());
    let grid = Arc::new(grid);
    let bands: Vec<String> = r["bands"].as_array().unwrap().iter().map(|b| b.as_str().unwrap().to_string()).collect();
    let stack = Value::Stack(Stack {
        grid: grid.clone(),
        keys: bands.iter().map(|b| Key::Name(b.clone())).collect(),
        layers: f.data.chunks(ny * nx).map(|c| Arc::new(c.to_vec())).collect(),
    });
    let (x, y) = (floats(&r["x"]), floats(&r["y"]));
    let points = Arc::new(Vector {
        geometries: x.iter().zip(&y).map(|(a, b)| Geometry::Point([*a, *b])).collect(),
        attributes: vec![("site".into(), Column::Numbers((1..=x.len()).map(|i| i as f64).collect()))],
        crs: Some("EPSG:32723".into()),
    });
    // the values at the points, to the bit
    for method in ["nearest", "bilinear"] {
        let out = vector(&run(
            "vector.extract",
            vec![("vector", Value::Vector(points.clone())), ("map", stack.clone())],
            json!({"method": method}),
        ));
        for (b, want) in bands.iter().zip(r[format!("extract_{method}")].as_array().unwrap()) {
            let ours = out.column(b).unwrap().numbers();
            for (i, (a, w)) in ours.iter().zip(floats(want)).enumerate() {
                assert!(a.to_bits() == w.to_bits() || (a.is_nan() && w.is_nan()), "{method} {b} point {i}: {a} {w}");
            }
        }
    }
    // the statistics within 120 m of every point
    let Value::Stack(s) = &stack else { panic!() };
    let first = Value::Map(terraflux_ops::Map { grid: grid.clone(), data: s.layers[0].clone() });
    let out = vector(&run(
        "vector.extract",
        vec![("vector", Value::Vector(points.clone())), ("map", first)],
        json!({"buffer": 120.0, "stats": ["mean", "max", "count"]}),
    ));
    for stat in ["mean", "max", "count"] {
        let ours = out.column(stat).unwrap().numbers();
        for (i, (a, w)) in ours.iter().zip(floats(&r["extract_buffer"][stat])).enumerate() {
            assert!(close(*a, w) || (a.is_nan() && w.is_nan()), "buffer {stat} point {i}: {a} {w}");
        }
    }
    // buffers of polygons and of points
    let read = |name: &str| {
        let path = root().join(format!("tests/data/reference/vectors/{name}.gpkg"));
        vector(&run("vector.read", vec![], json!({"path": path.to_string_lossy()})))
    };
    let pu = read("pu_polygons");
    let b = vector(&run("vector.buffer", vec![("vector", Value::Vector(pu.clone()))], json!({"distance": 35.0})));
    for (i, (g, w)) in b.geometries.iter().zip(floats(&r["buffer_area"])).enumerate() {
        assert!(close(area(g), w), "buffer of polygon {i}: {} {w}", area(g));
    }
    let b = vector(&run("vector.buffer", vec![("vector", Value::Vector(points.clone()))], json!({"distance": 50.0})));
    for (i, (g, w)) in b.geometries.iter().zip(floats(&r["buffer_points_area"])).enumerate() {
        assert!(close(area(g), w), "buffer of point {i}: {} {w}", area(g));
    }
    // the polygons cut to a mask of two boxes
    let boxes = Arc::new(Vector {
        geometries: [[250.0, 150.0, 950.0, 850.0], [900.0, 800.0, 1300.0, 1100.0]]
            .iter()
            .map(|b| {
                Geometry::Polygon(vec![vec![[b[0], b[1]], [b[2], b[1]], [b[2], b[3]], [b[0], b[3]], [b[0], b[1]]]])
            })
            .collect(),
        attributes: Vec::new(),
        crs: Some("EPSG:32723".into()),
    });
    let mut numbered = (*pu).clone();
    numbered.attributes.push(("index".into(), Column::Numbers((0..pu.len()).map(|i| i as f64).collect())));
    let c = vector(&run(
        "vector.clip",
        vec![("vector", Value::Vector(Arc::new(numbered.clone()))), ("mask", Value::Vector(boxes))],
        json!({}),
    ));
    assert_eq!(c.column("index").unwrap().numbers(), floats(&r["clip_index"]));
    for (i, (g, w)) in c.geometries.iter().zip(floats(&r["clip_area"])).enumerate() {
        assert!(close(area(g), w), "clipped {i}: {} {w}", area(g));
    }
    // dissolved by a group, and all at once
    numbered.attributes.push(("group".into(), Column::Numbers((0..pu.len()).map(|i| (i % 3) as f64).collect())));
    let numbered = Value::Vector(Arc::new(numbered));
    let g = vector(&run("vector.dissolve", vec![("vector", numbered.clone())], json!({"by": "group"})));
    let groups = g.column("group").unwrap().numbers();
    for (want_group, want) in floats(&r["dissolve_group"]).iter().zip(floats(&r["dissolve_area"])) {
        let k = groups.iter().position(|v| v == want_group).unwrap();
        assert!(close(area(&g.geometries[k]), want), "group {want_group}");
    }
    let all = vector(&run("vector.dissolve", vec![("vector", numbered)], json!({})));
    assert_eq!(all.len(), 1);
    assert!(close(area(&all.geometries[0]), r["dissolve_all_area"].as_f64().unwrap()));
}
