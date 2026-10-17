//! landschaft 1.50's surfaces and Dinamica functors against landschaft (tests/data/reference/landforms.npz,
//! tools/make_landforms_reference.py): the geomorphons of its r.geomorphon cases, hilltops, the gravitational
//! potential, the Patcher of a continuous state and the road functors, to the bit (the FFT's potential within
//! 1e-12 of the largest).

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::files::Rasters;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;

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

fn table_of(out: &Outputs, port: &str) -> Vec<f64> {
    let Value::Table(t) = &out[port] else { panic!("{port}") };
    t.rows.iter().flatten().copied().collect()
}

struct Data {
    z: std::collections::BTreeMap<String, terraflux_raster::npy::Array>,
    meta: serde_json::Value,
}

impl Data {
    fn load() -> Data {
        let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/landforms.npz")).unwrap();
        let text = std::fs::read_to_string(root().join("tests/data/reference/landforms.json")).unwrap();
        Data { z, meta: serde_json::from_str(&text).unwrap() }
    }

    fn want(&self, key: &str) -> &[f64] {
        &self.z.get(key).unwrap_or_else(|| panic!("{key}")).data
    }

    /// A map of a key on the grid of a geotransform.
    fn map(&self, key: &str, t: &serde_json::Value) -> Map {
        let a = &self.z[key];
        let mut g = Grid::new(a.shape[1], a.shape[0]);
        g.transform = serde_json::from_value(t.clone()).unwrap();
        Map::new(Arc::new(g), a.data.clone())
    }
}

#[test]
fn geomorphons_are_landschafts() {
    let d = Data::load();
    for (name, c) in d.meta["geomorphons"].as_object().unwrap() {
        let dem = d.map(&format!("geo/{name}/dem"), &c["transform"]);
        let out = run("surface.geomorphons", vec![("dem", Value::Map(dem))], c["settings"].clone());
        for k in [
            "forms",
            "ternary",
            "positive",
            "negative",
            "intensity",
            "exposition",
            "range",
            "variance",
            "elongation",
            "azimuth",
            "extend",
            "width",
        ] {
            check(&map_of(&out, k), d.want(&format!("geo/{name}/{k}")), &format!("{name} {k}"));
        }
        check(&table_of(&out, "share"), d.want(&format!("geo/{name}/share")), &format!("{name} share"));
    }
}

#[test]
fn hilltops_are_landschafts() {
    let d = Data::load();
    for h in d.meta["hilltop"].as_array().unwrap() {
        let name = h["name"].as_str().unwrap();
        let dem = if name == "lonlat" {
            let p = root().join("extern/landschaft/src/landschaft/surface/data/orelevation.tif");
            let m = Rasters::open(p.to_str().unwrap(), None, None).unwrap().map().unwrap();
            let want = &d.z["hill/lonlat/dem"];
            let (ny, nx) = (want.shape[0], want.shape[1]);
            let mut g = (*m.grid).clone();
            let w = g.width;
            (g.width, g.height) = (nx, ny);
            let data: Vec<f64> = (0..ny).flat_map(|i| m.data[i * w..i * w + nx].to_vec()).collect();
            check(&data, &want.data, "orelevation's corner");
            Map::new(Arc::new(g), data)
        } else {
            d.map(&format!("hill/{name}/dem"), &h["transform"])
        };
        let mut inputs = vec![("dem", Value::Map(dem.clone()))];
        if h["plain"].as_bool().unwrap() {
            inputs.push((
                "plain",
                Value::Map(Map::new(dem.grid.clone(), d.want(&format!("hill/{name}/plain")).to_vec())),
            ));
        }
        let out = run("surface.hilltop", inputs, h["settings"].clone());
        check(&map_of(&out, "app"), d.want(&format!("hill/{name}/app")), &format!("{name} app"));
        check(&map_of(&out, "hill"), d.want(&format!("hill/{name}/hill")), &format!("{name} hill"));
        check(&table_of(&out, "hills"), d.want(&format!("hill/{name}/hills")), &format!("{name} hills"));
    }
}

#[test]
fn gravity_is_landschafts() {
    let d = Data::load();
    let t = &d.meta["gravity_transform"];
    for g in d.meta["gravity"].as_array().unwrap() {
        let name = g["name"].as_str().unwrap();
        let mut inputs = vec![("sources", Value::Map(d.map("gravity/sources", t)))];
        if g["mask"].as_bool().unwrap() {
            inputs.push(("mask", Value::Map(d.map("gravity/mask", t))));
        }
        let got = map_of(&run("surface.gravity", inputs, g["settings"].clone()), "map");
        let want = d.want(&format!("gravity/{name}"));
        if name == "fft" {
            let top = want.iter().fold(0f64, |a, b| a.max(b.abs()));
            assert!(got.iter().zip(want).all(|(a, b)| (a - b).abs() <= 1e-12 * top), "gravity by FFT");
        } else {
            check(&got, want, &format!("gravity {name}"));
        }
    }
}

#[test]
fn the_continuous_patcher_is_landschafts() {
    let d = Data::load();
    let t = &d.meta["stock_transform"];
    let (stock, prob) = (d.map("stock/biomass", t), d.map("stock/probability", t));
    let out = run(
        "dinamica.continuous_patcher",
        vec![
            ("stock", Value::Map(stock.clone())),
            ("probability", Value::Map(prob.clone())),
            ("quota", Value::Number(40000.0)),
            ("seed", Value::Integer(11)),
        ],
        serde_json::json!({"mean": 1.5, "variance": 0.4, "isometry": 1.3}),
    );
    for k in ["stock", "probability", "taken", "patch"] {
        check(&map_of(&out, k), d.want(&format!("stock/allocate/{k}")), &format!("allocate {k}"));
    }
    check(&table_of(&out, "info"), d.want("stock/allocate/info"), "allocate info");
    // three steps of a run: the stock and the probabilities depleted from step to step
    let (mut s, mut p) = (Value::Map(stock), Value::Map(prob));
    let mut rows = Vec::new();
    for (k, q) in [500.0, 800.0, 300.0].into_iter().enumerate() {
        let out = run(
            "dinamica.continuous_patcher",
            vec![
                ("stock", s.clone()),
                ("probability", p.clone()),
                ("quota", Value::Number(q)),
                ("step", Value::Integer(k as i64 + 1)),
                ("seed", Value::Integer(4)),
            ],
            serde_json::json!({"mean": 6, "variance": 4, "isometry": 0.6, "unit": "cells", "window": [3, 5]}),
        );
        check(&map_of(&out, "stock"), d.want(&format!("stock/run/{}/stock", k + 1)), &format!("step {} stock", k + 1));
        check(&map_of(&out, "taken"), d.want(&format!("stock/run/{}/taken", k + 1)), &format!("step {} taken", k + 1));
        rows.push((k + 1) as f64);
        rows.extend(table_of(&out, "info"));
        (s, p) = (out["stock"].clone(), out["probability"].clone());
    }
    check(&rows, d.want("stock/run/table"), "the run's table");
}

#[test]
fn the_road_functors_are_landschafts() {
    let d = Data::load();
    let t = &d.meta["roads_transform"];
    let m = |k: &str| Value::Map(d.map(&format!("roads/{k}"), t));
    let out =
        run("dinamica.cost_map", vec![("sources", m("network")), ("friction", m("friction"))], serde_json::json!({}));
    check(&map_of(&out, "cost"), d.want("roads/cost"), "cost");
    check(&map_of(&out, "direction"), d.want("roads/direction"), "directions");
    let (cost, dirs) = (out["cost"].clone(), out["direction"].clone());
    let out = run(
        "dinamica.cost_map",
        vec![("sources", m("network")), ("friction", m("friction"))],
        serde_json::json!({"diagonal": false, "relative": true}),
    );
    check(&map_of(&out, "cost"), d.want("roads/cost_relative"), "relative cost");
    let dest = run(
        "dinamica.destinations",
        vec![("attraction", m("attraction")), ("seed", Value::Integer(9))],
        serde_json::json!({"quadrants": [3, 4], "per_quadrant": 2, "min_distance": 800.0}),
    );
    check(&map_of(&dest, "map"), d.want("roads/destinations"), "destinations");
    let dm = dest["map"].clone();
    let paths = |inputs: Vec<(&str, Value)>, params: serde_json::Value| -> Vec<f64> {
        let mut all = vec![("network", m("network")), ("sources", dm.clone())];
        all.extend(inputs);
        map_of(&run("dinamica.pathways", all, params), "map")
    };
    check(
        &paths(vec![("cost", cost.clone()), ("seed", Value::Integer(3))], serde_json::json!({})),
        d.want("roads/paths_cost"),
        "paths down the costs",
    );
    check(
        &paths(vec![("directions", dirs)], serde_json::json!({"merge": false, "value": 7.0})),
        d.want("roads/paths_dirs"),
        "paths along the directions",
    );
    check(
        &paths(vec![("cost", cost), ("seed", Value::Integer(21))], serde_json::json!({"lottery": true})),
        d.want("roads/paths_lottery"),
        "paths with the lottery",
    );
    let builder = d.meta["roads"]["builder"].clone();
    let step = |network: Value, step: Option<i64>, seed: i64| -> Outputs {
        let mut inputs = vec![
            ("network", network),
            ("friction", m("friction")),
            ("attraction", m("attraction")),
            ("seed", Value::Integer(seed)),
        ];
        if let Some(s) = step {
            inputs.push(("step", Value::Integer(s)));
        }
        run("dinamica.roads", inputs, builder.clone())
    };
    check(&map_of(&step(m("network"), None, 17), "network"), d.want("roads/step"), "a step of roads");
    let mut net = m("network");
    for k in 1..=2 {
        let out = step(net, Some(k), 8);
        check(&map_of(&out, "network"), d.want(&format!("roads/run/{k}")), &format!("roads, step {k}"));
        net = out["network"].clone();
    }
}
