//! Terrain hydrology against landschaft's (tests/data/reference/hydro.npz, tools/make_hydro_reference.py): every
//! function of ls.hydro with some options on the DEMs of its WhiteboxTools references and on a longitude / latitude
//! corner of orelevation, to the bit.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::files::Rasters;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Geometry, Vector};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn check(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}");
    let bad = got.iter().zip(want).filter(|(a, b)| !same(**a, **b)).count();
    let first = got.iter().zip(want).position(|(a, b)| !same(*a, *b));
    assert_eq!(bad, 0, "{what}: {bad} cells differ, the first {first:?}: {:?}", first.map(|i| (got[i], want[i])));
}

fn map_of(out: &Outputs, port: &str) -> Vec<f64> {
    let Value::Map(m) = &out[port] else { panic!("{port}") };
    m.data.as_ref().clone()
}

fn table_of(out: &Outputs) -> Vec<f64> {
    let Value::Table(t) = &out["table"] else { panic!("table") };
    t.rows.iter().flatten().copied().collect()
}

/// The DEMs: whitebox's on grids from 0, 0 without a coordinate system; orelevation's corner as read.
fn dem_of(name: &str, extent: &[f64], z: &[f64], shape: (usize, usize)) -> Map {
    let (ny, nx) = shape;
    if name == "lonlat" {
        let p = root().join("extern/landschaft/src/landschaft/surface/data/orelevation.tif");
        let m = Rasters::open(p.to_str().unwrap(), None, None).unwrap().map().unwrap();
        let mut g = (*m.grid).clone();
        let w = g.width;
        (g.width, g.height) = (nx, ny);
        let data: Vec<f64> = (0..ny).flat_map(|i| m.data[i * w..i * w + nx].to_vec()).collect();
        check(&data, z, "orelevation's corner");
        return Map::new(Arc::new(g), data);
    }
    let mut g = Grid::new(nx, ny);
    let (dx, dy) = ((extent[1] - extent[0]) / nx as f64, (extent[3] - extent[2]) / ny as f64);
    g.transform = [extent[0], dx, 0.0, extent[3], 0.0, -dy];
    Map::new(Arc::new(g), z.to_vec())
}

#[test]
fn hydrology_is_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/hydro.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/hydro.json")).unwrap())
            .unwrap();
    for d in meta["dems"].as_array().unwrap() {
        let name = d["name"].as_str().unwrap();
        let extent: Vec<f64> = serde_json::from_value(d["extent"].clone()).unwrap();
        let raw = &z[&format!("{name}/dem")];
        let shape = (raw.shape[0], raw.shape[1]);
        let dem = dem_of(name, &extent, &raw.data, shape);
        let filled = Map::new(
            dem.grid.clone(),
            map_of(&run("hydro.fill", vec![("dem", Value::Map(dem.clone()))], serde_json::json!({})), "map"),
        );
        // numpy's linspace(1, 2, columns): j x step + 1, the last one 2
        let step = 1.0 / (shape.1 - 1) as f64;
        let row: Vec<f64> = (0..shape.1).map(|j| if j == shape.1 - 1 { 2.0 } else { j as f64 * step + 1.0 }).collect();
        let weights: Vec<f64> = (0..shape.0).flat_map(|_| row.iter().copied()).collect();
        let wmap = |scale: f64| Value::Map(Map::new(dem.grid.clone(), weights.iter().map(|w| w / scale).collect()));
        for case in meta["cases"].as_array().unwrap() {
            let (key, op) = (case[0].as_str().unwrap(), case[1].as_str().unwrap());
            if name == "lonlat" && op == "connectivity_index" {
                continue;
            }
            let mut params = case[2].as_object().unwrap().clone();
            let with_weights = params.remove("weights").and_then(|v| v.as_bool()).unwrap_or(false);
            let base = if op == "fill" || op == "breach" { &dem } else { &filled };
            let mut inputs = vec![("dem", Value::Map(base.clone()))];
            if with_weights {
                inputs.push(("weights", wmap(1.0)));
            }
            if op == "connectivity_index" {
                inputs.push(("weight", wmap(10.0)));
            }
            let out = run(&format!("hydro.{op}"), inputs, serde_json::Value::Object(params));
            check(&map_of(&out, "map"), &z[&format!("{name}/{key}")].data, &format!("{name} {key}"));
        }
        let fm = || Value::Map(filled.clone());
        let acc = run("hydro.flow_accumulation", vec![("dem", fm())], serde_json::json!({}));
        let rivers =
            run("hydro.streams", vec![("accumulation", acc["map"].clone())], serde_json::json!({"threshold": 20}));
        check(&map_of(&rivers, "map"), &z[&format!("{name}/streams")].data, &format!("{name} streams"));
        let mfd = run("hydro.flow_accumulation", vec![("dem", fm())], serde_json::json!({"method": "mfd"}));
        let traced = run(
            "hydro.streams",
            vec![("accumulation", mfd["map"].clone()), ("dem", fm())],
            serde_json::json!({"threshold": 25, "trace": 0.7}),
        );
        check(&map_of(&traced, "map"), &z[&format!("{name}/streams_trace")].data, &format!("{name} traced streams"));
        let st = || rivers["map"].clone();
        let out = run("hydro.hand", vec![("dem", fm()), ("streams", st())], serde_json::json!({}));
        check(&map_of(&out, "map"), &z[&format!("{name}/hand_streams")].data, &format!("{name} hand of streams"));
        let out = run("hydro.distance_to_channel", vec![("dem", fm()), ("streams", st())], serde_json::json!({}));
        check(&map_of(&out, "map"), &z[&format!("{name}/distance")].data, &format!("{name} distance"));
        let out = run(
            "hydro.distance_to_channel",
            vec![("dem", fm()), ("streams", st()), ("weights", wmap(1.0))],
            serde_json::json!({}),
        );
        check(&map_of(&out, "map"), &z[&format!("{name}/distance_w")].data, &format!("{name} weighted distance"));
        let links = run("hydro.stream_links", vec![("dem", fm()), ("streams", st())], serde_json::json!({}));
        check(&map_of(&links, "map"), &z[&format!("{name}/links")].data, &format!("{name} links"));
        check(&table_of(&links), &z[&format!("{name}/links_table")].data, &format!("{name} links' table"));
        let seeds: Vec<f64> = map_of(&links, "map").iter().map(|v| if v.is_nan() { 0.0 } else { *v }).collect();
        let out = run(
            "hydro.watersheds",
            vec![("dem", fm()), ("seeds", Value::Map(Map::new(dem.grid.clone(), seeds)))],
            serde_json::json!({}),
        );
        check(&map_of(&out, "map"), &z[&format!("{name}/watersheds")].data, &format!("{name} watersheds"));
        if let Some(pts) = d["outlets"].as_array() {
            let points = Vector {
                geometries: pts
                    .iter()
                    .map(|p| Geometry::Point([p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]))
                    .collect(),
                attributes: Vec::new(),
                crs: None,
            };
            let out = run(
                "hydro.stream_links",
                vec![("dem", fm()), ("streams", st()), ("outlets", Value::Vector(Arc::new(points)))],
                serde_json::json!({"snap": d["snap"]}),
            );
            check(&map_of(&out, "map"), &z[&format!("{name}/links_outlets")].data, &format!("{name} links to outlets"));
            check(&table_of(&out), &z[&format!("{name}/links_outlets_table")].data, &format!("{name} outlets' table"));
        }
    }
}
