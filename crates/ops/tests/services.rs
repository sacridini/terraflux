//! InVEST's models against landschaft's (tests/data/reference/services.npz, tools/make_services_reference.py, on
//! the cases of landschaft's InVEST references): carbon, habitat quality, annual water yield and the sediment
//! delivery ratio, their maps to the bit and their tables.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;
use terraflux_raster::vector::{Column, Geometry, Vector};

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

/// Numbers that agree to a relative tolerance (sums in another order).
fn close(got: &[f64], want: &[f64], rtol: f64, what: &str) {
    assert_eq!(got.len(), want.len(), "{what}");
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        assert!(same(*a, *b) || (a - b).abs() <= rtol * b.abs().max(1.0), "{what} {k}: {a} {b}");
    }
}

struct Case<'a> {
    name: &'a str,
    z: &'a std::collections::BTreeMap<String, Array>,
    meta: &'a serde_json::Value,
    grid: Arc<Grid>,
}

impl Case<'_> {
    fn map(&self, key: &str) -> Value {
        Value::Map(Map::new(self.grid.clone(), self.z[&format!("{}/{key}", self.name)].data.clone()))
    }

    fn has(&self, key: &str) -> bool {
        self.z.contains_key(&format!("{}/{key}", self.name))
    }

    fn want(&self, key: &str) -> &[f64] {
        &self.z[&format!("{}/{key}", self.name)].data
    }

    fn table(&self, key: &str) -> Value {
        let t = &self.meta[key];
        let mut out = Table::new(serde_json::from_value(t["columns"].clone()).unwrap());
        out.rows = serde_json::from_value(t["rows"].clone()).unwrap();
        Value::Table(out)
    }

    fn stack(&self, key: &str, names: &[String]) -> Value {
        Value::Stack(Stack {
            grid: self.grid.clone(),
            keys: names.iter().map(|n| Key::Name(n.clone())).collect(),
            layers: names.iter().map(|n| Arc::new(self.want(&format!("in/{key}/{n}")).to_vec())).collect(),
        })
    }
}

fn ring(r: &serde_json::Value) -> Vec<[f64; 2]> {
    r.as_array().unwrap().iter().map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]).collect()
}

fn polygons(rings: &serde_json::Value, field: &str, values: Vec<f64>) -> Value {
    Value::Vector(Arc::new(Vector {
        geometries: rings.as_array().unwrap().iter().map(|r| Geometry::Polygon(vec![ring(r)])).collect(),
        attributes: vec![(field.to_string(), Column::Numbers(values))],
        crs: None,
    }))
}

fn out_map(out: &Outputs, port: &str) -> Vec<f64> {
    let Value::Map(m) = &out[port] else { panic!("{port}") };
    m.data.as_ref().clone()
}

fn out_table(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port}") };
    t.clone()
}

fn same_table(got: &Table, want: &serde_json::Value, rtol: f64, what: &str) {
    let cols: Vec<String> = serde_json::from_value(want["columns"].clone()).unwrap();
    assert_eq!(got.columns, cols, "{what}");
    let rows: Vec<Vec<f64>> = serde_json::from_value(want["rows"].clone()).unwrap();
    close(&got.rows.concat(), &rows.concat(), rtol, what);
}

#[test]
fn invest_models_are_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/services.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/services.json")).unwrap())
            .unwrap();
    let pixel = meta["pixel"].as_f64().unwrap();
    let origin = (meta["origin"][0].as_f64().unwrap(), meta["origin"][1].as_f64().unwrap());
    for (name, m) in meta["cases"].as_object().unwrap() {
        let shape = &z[&format!("{name}/in/lulc")].shape;
        let mut g = Grid::new(shape[1], shape[0]);
        g.transform = [origin.0, pixel, 0.0, origin.1, 0.0, -pixel];
        let c = Case { name, z: &z, meta: m, grid: Arc::new(g) };
        match m["model"].as_str().unwrap() {
            "carbon" => {
                let mut inputs = vec![("lulc", c.map("in/lulc")), ("pools", c.table("pools"))];
                if c.has("in/future") {
                    inputs.push(("future", c.map("in/future")));
                }
                let out = run("services.carbon", inputs, m.get("params").cloned().unwrap_or_default());
                check(&out_map(&out, "storage"), c.want("storage"), &format!("{name} storage"));
                let Value::Stack(pools) = &out["pools"] else { panic!() };
                for (k, p) in ["c_above", "c_below", "c_soil", "c_dead"].iter().enumerate() {
                    check(&pools.layers[k], c.want(p), &format!("{name} {p}"));
                }
                for k in ["storage_future", "change", "npv"] {
                    if c.has(k) {
                        check(&out_map(&out, k), c.want(k), &format!("{name} {k}"));
                    }
                }
                let totals: Vec<f64> = serde_json::from_value(m["totals"].clone()).unwrap();
                check(&out_table(&out, "totals").rows[0], &totals, &format!("{name} totals"));
            }
            "habitat_quality" => {
                let names: Vec<String> = m["threats"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|t| t["threat"].as_str().unwrap().to_string())
                    .collect();
                let mut inputs = vec![
                    ("lulc", c.map("in/lulc")),
                    ("sensitivity", c.table("sensitivity")),
                    ("threat_maps", c.stack("threat_maps", &names)),
                ];
                if c.has("in/future") {
                    inputs.push(("future", c.map("in/future")));
                    inputs.push(("future_threat_maps", c.stack("future_threat_maps", &names)));
                }
                if c.has("in/base") {
                    inputs.push(("base", c.map("in/base")));
                }
                let polys = m["polygons"].as_array().unwrap();
                if !polys.is_empty() {
                    let rings = serde_json::Value::Array(polys.iter().map(|p| p["ring"].clone()).collect());
                    let access = polys.iter().map(|p| p["access"].as_f64().unwrap()).collect();
                    inputs.push(("access", polygons(&rings, "ACCESS", access)));
                }
                let out = run(
                    "services.habitat_quality",
                    inputs,
                    serde_json::json!({"threats": m["threats"], "half_saturation": m["k"]}),
                );
                for k in ["quality", "degradation", "quality_future", "degradation_future", "rarity", "rarity_future"] {
                    if c.has(k) {
                        check(&out_map(&out, k), c.want(k), &format!("{name} {k}"));
                    }
                }
                let Value::Stack(threats) = &out["threats"] else { panic!() };
                for (k, t) in names.iter().enumerate() {
                    check(&threats.layers[k], c.want(&format!("threat/{t}")), &format!("{name} {t}"));
                }
                if m.get("rarity_table").is_some() {
                    same_table(&out_table(&out, "rarity_table"), &m["rarity_table"], 0.0, &format!("{name} rarity"));
                }
            }
            "water_yield" => {
                let mut inputs = vec![("lulc", c.map("in/lulc")), ("biophysical", c.table("biophysical"))];
                for k in ["precipitation", "eto", "depth", "pawc"] {
                    inputs.push((k, c.map(&format!("in/{k}"))));
                }
                inputs.push(("watersheds", polygons(&m["watersheds"], "ws_id", vec![1.0, 2.0])));
                if m.get("subwatersheds").is_some() {
                    inputs.push((
                        "subwatersheds",
                        polygons(&m["subwatersheds"], "subws_id", vec![10.0, 11.0, 12.0, 13.0]),
                    ));
                }
                if m.get("demand").is_some() {
                    inputs.push(("demand", c.table("demand")));
                }
                if m.get("valuation").is_some() {
                    inputs.push(("valuation", c.table("valuation")));
                }
                let out = run(
                    "services.water_yield",
                    inputs,
                    serde_json::json!({"seasonality": m["z"], "precision": m["precision"]}),
                );
                for k in ["wyield", "aet", "pet", "fractp", "demand_map"] {
                    if c.has(k) {
                        check(&out_map(&out, k), c.want(k), &format!("{name} {k}"));
                    }
                }
                same_table(&out_table(&out, "watershed_table"), &m["watershed_table"], 1e-12, &format!("{name} sheds"));
                if m.get("subwatershed_table").is_some() {
                    same_table(
                        &out_table(&out, "subwatershed_table"),
                        &m["subwatershed_table"],
                        1e-12,
                        &format!("{name} subwatersheds"),
                    );
                }
            }
            _ => {
                let mut inputs = vec![("lulc", c.map("in/lulc")), ("biophysical", c.table("biophysical"))];
                for k in ["dem", "erosivity", "erodibility"] {
                    inputs.push((k, c.map(&format!("in/{k}"))));
                }
                if c.has("in/drainage") {
                    inputs.push(("drainage", c.map("in/drainage")));
                }
                inputs.push(("watersheds", polygons(&m["watersheds"], "ws_id", vec![1.0, 2.0])));
                let out = run("services.sdr", inputs, m["params"].clone());
                for k in [
                    "usle",
                    "sed_export",
                    "sed_deposition",
                    "avoided_export",
                    "avoided_erosion",
                    "rkls",
                    "sdr",
                    "ic",
                    "stream",
                    "drains_to_stream",
                ] {
                    check(&out_map(&out, k), c.want(k), &format!("{name} {k}"));
                }
                same_table(&out_table(&out, "watershed_table"), &m["watershed_table"], 1e-12, &format!("{name} sheds"));
            }
        }
    }
}
