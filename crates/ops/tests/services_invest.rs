//! InVEST's nutrient delivery ratio, crop pollination and seasonal water yield against landschaft's
//! (tests/data/reference/services_invest.npz, tools/make_services_invest_reference.py, on the cases of
//! landschaft's InVEST references): their maps to the bit and their tables.

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

/// The differences found (every case is run, then they are reported together).
static FAILED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());

fn fail(m: String) {
    FAILED.lock().unwrap().push(m);
}

fn check(got: &[f64], want: &[f64], what: &str) {
    if got.len() != want.len() {
        return fail(format!("{what}: {} values, not {}", got.len(), want.len()));
    }
    let bad = got.iter().zip(want).filter(|(a, b)| !same(**a, **b)).count();
    let first = got.iter().zip(want).position(|(a, b)| !same(*a, *b));
    if bad > 0 {
        fail(format!("{what}: {bad} cells differ, the first {first:?}: {:?}", first.map(|i| (got[i], want[i]))));
    }
}

/// Numbers that agree to a relative tolerance (sums in another order).
fn close(got: &[f64], want: &[f64], rtol: f64, what: &str) {
    if got.len() != want.len() {
        return fail(format!("{what}: {} values, not {}", got.len(), want.len()));
    }
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        if !(same(*a, *b) || (a - b).abs() <= rtol * b.abs().max(1.0)) {
            return fail(format!("{what} {k}: {a} {b}"));
        }
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
        Value::Map(Map::new(self.grid.clone(), self.want(key).to_vec()))
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

    /// The twelve months of an input, as a stack.
    fn months(&self, key: &str) -> Value {
        Value::Stack(Stack {
            grid: self.grid.clone(),
            keys: (1..=12).map(|m| Key::Name(m.to_string())).collect(),
            layers: (1..=12).map(|m| Arc::new(self.want(&format!("in/{key}/{m}")).to_vec())).collect(),
        })
    }
}

fn ring(r: &serde_json::Value) -> Vec<[f64; 2]> {
    r.as_array().unwrap().iter().map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap()]).collect()
}

fn watersheds(rings: &serde_json::Value) -> Value {
    let geometries: Vec<Geometry> =
        rings.as_array().unwrap().iter().map(|r| Geometry::Polygon(vec![ring(r)])).collect();
    let ids = (1..=geometries.len()).map(|i| i as f64).collect();
    Value::Vector(Arc::new(Vector { geometries, attributes: vec![("ws_id".into(), Column::Numbers(ids))], crs: None }))
}

/// The farms: a ring and the attributes each (text ones as texts).
fn farms(list: &serde_json::Value) -> Value {
    let list = list.as_array().unwrap();
    let names: Vec<&String> = list[0].as_object().unwrap().keys().filter(|k| *k != "ring").collect();
    let attributes = names
        .iter()
        .map(|n| {
            let col = if list[0][n.as_str()].is_string() {
                Column::Texts(list.iter().map(|f| f[n.as_str()].as_str().unwrap().to_string()).collect())
            } else {
                Column::Numbers(list.iter().map(|f| f[n.as_str()].as_f64().unwrap()).collect())
            };
            ((*n).clone(), col)
        })
        .collect();
    Value::Vector(Arc::new(Vector {
        geometries: list.iter().map(|f| Geometry::Polygon(vec![ring(&f["ring"])])).collect(),
        attributes,
        crs: None,
    }))
}

fn out_map(out: &Outputs, port: &str) -> Vec<f64> {
    let Value::Map(m) = &out[port] else { panic!("{port}") };
    m.data.as_ref().clone()
}

fn out_layer(out: &Outputs, port: &str, key: &str) -> Vec<f64> {
    let Value::Stack(s) = &out[port] else { panic!("{port}") };
    s.by_name(key).unwrap_or_else(|| panic!("{port} {key}")).as_ref().clone()
}

fn out_table(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port}") };
    t.clone()
}

/// The table against the reference's, its columns from the `skip`th on.
fn same_table(got: &Table, want: &serde_json::Value, skip: usize, rtol: f64, what: &str) {
    let cols: Vec<String> = serde_json::from_value(want["columns"].clone()).unwrap();
    if got.columns[skip..] != cols[..] {
        return fail(format!("{what}: columns {:?}, not {cols:?}", got.columns));
    }
    let rows: Vec<Vec<Option<f64>>> = serde_json::from_value(want["rows"].clone()).unwrap();
    let rows: Vec<f64> = rows.concat().iter().map(|v| v.unwrap_or(f64::NAN)).collect();
    let got: Vec<f64> = got.rows.iter().flat_map(|r| r[skip..].to_vec()).collect();
    close(&got, &rows, rtol, what);
}

#[test]
fn invest_models_are_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/services_invest.npz")).unwrap();
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/data/reference/services_invest.json")).unwrap(),
    )
    .unwrap();
    let pixel = meta["pixel"].as_f64().unwrap();
    let origin = (meta["origin"][0].as_f64().unwrap(), meta["origin"][1].as_f64().unwrap());
    for (name, m) in meta["cases"].as_object().unwrap() {
        let key = if m["model"] == "seasonal_water_yield" { "in/dem" } else { "in/lulc" };
        let shape = &z[&format!("{name}/{key}")].shape;
        let mut g = Grid::new(shape[1], shape[0]);
        g.transform = [origin.0, pixel, 0.0, origin.1, 0.0, -pixel];
        let c = Case { name, z: &z, meta: m, grid: Arc::new(g) };
        match m["model"].as_str().unwrap() {
            "ndr" => {
                let inputs = vec![
                    ("dem", c.map("in/dem")),
                    ("lulc", c.map("in/lulc")),
                    ("runoff_proxy", c.map("in/runoff_proxy")),
                    ("biophysical", c.table("biophysical")),
                    ("watersheds", watersheds(&m["watersheds"])),
                ];
                let mut params = m["params"].clone();
                // the load types by class code
                let codes: Vec<i64> = m["biophysical"]["rows"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|r| r[0].as_f64().unwrap() as i64)
                    .collect();
                for x in ["load_type_n", "load_type_p"] {
                    let by: serde_json::Map<String, serde_json::Value> =
                        codes.iter().zip(m[x].as_array().unwrap()).map(|(c, t)| (c.to_string(), t.clone())).collect();
                    params[x] = serde_json::Value::Object(by);
                }
                let out = run("services.ndr", inputs, params);
                for k in [
                    "n_surface_export",
                    "n_subsurface_export",
                    "n_total_export",
                    "p_surface_export",
                    "ndr_n",
                    "ndr_p",
                    "effective_retention_n",
                    "effective_retention_p",
                    "ic",
                    "stream",
                    "drains_to_stream",
                ] {
                    if out.contains_key(k) != c.has(k) {
                        fail(format!("{name} {k}: given {}, in the reference {}", out.contains_key(k), c.has(k)));
                    }
                    if c.has(k) {
                        check(&out_map(&out, k), c.want(k), &format!("{name} {k}"));
                    }
                }
                let Value::Number(ic_0) = out["ic_0"] else { panic!() };
                check(&[ic_0], &[m["ic_0"].as_f64().unwrap()], &format!("{name} ic_0"));
                let Value::Number(mean) = out["runoff_proxy_mean"] else { panic!() };
                check(&[mean], &[m["runoff_proxy_mean"].as_f64().unwrap()], &format!("{name} proxy mean"));
                same_table(
                    &out_table(&out, "watershed_table"),
                    &m["watershed_table"],
                    0,
                    1e-12,
                    &format!("{name} sheds"),
                );
            }
            "pollination" => {
                let mut inputs = vec![("lulc", c.map("in/lulc")), ("biophysical", c.table("biophysical"))];
                if m.get("farms").is_some() {
                    inputs.push(("farms", farms(&m["farms"])));
                }
                let out = run("services.pollination", inputs, serde_json::json!({"guilds": m["guilds"]}));
                let species: Vec<String> = serde_json::from_value(m["species"].clone()).unwrap();
                let seasons: Vec<String> = serde_json::from_value(m["seasons"].clone()).unwrap();
                for sp in &species {
                    check(&out_layer(&out, "supply", sp), c.want(&format!("supply/{sp}")), &format!("{name} {sp}"));
                    for se in &seasons {
                        let k = format!("{sp}_{se}");
                        check(
                            &out_layer(&out, "abundance", &k),
                            c.want(&format!("abundance/{k}")),
                            &format!("{name} {k}"),
                        );
                    }
                }
                for se in &seasons {
                    let want = c.want(&format!("total_abundance/{se}"));
                    check(&out_layer(&out, "total_abundance", se), want, &format!("{name} total {se}"));
                }
                if m.get("farms").is_some() {
                    for k in ["farm_pollinators", "total_yield", "wild_yield"] {
                        check(&out_map(&out, k), c.want(k), &format!("{name} {k}"));
                    }
                    same_table(&out_table(&out, "farm_table"), &m["farm_table"], 1, 1e-12, &format!("{name} farms"));
                }
            }
            _ => {
                let mut inputs = vec![("dem", c.map("in/dem")), ("watersheds", watersheds(&m["watersheds"]))];
                let mut params = m["params"].clone();
                params["precision"] = m["precision"].clone();
                let recharge = c.has("in/recharge");
                if recharge {
                    inputs.push(("recharge", c.map("in/recharge")));
                } else {
                    inputs.push(("lulc", c.map("in/lulc")));
                    inputs.push(("biophysical", c.table("biophysical")));
                    inputs.push(("soil_group", c.map("in/soil_group")));
                    inputs.push(("precipitation", c.months("precipitation")));
                    inputs.push(("et0", c.months("et0")));
                    if c.has("in/climate_zones") {
                        inputs.push(("climate_zones", c.map("in/climate_zones")));
                        inputs.push(("climate_zone_table", c.table("climate_zone_table")));
                    } else {
                        params["rain_events"] = m["rain_events"].clone();
                    }
                }
                let out = run("services.seasonal_water_yield", inputs, params);
                let mut keys = vec!["local_recharge", "l_avail", "l_sum", "baseflow", "b_sum", "vri", "stream"];
                if !recharge {
                    keys.extend(["quickflow", "l_sum_avail", "aet"]);
                    check(&out_map(&out, "annual_precipitation"), c.want("precipitation"), &format!("{name} p"));
                    for mo in 1..=12 {
                        let k = format!("quickflow_monthly/{mo}");
                        check(
                            &out_layer(&out, "quickflow_monthly", &mo.to_string()),
                            c.want(&k),
                            &format!("{name} {k}"),
                        );
                    }
                    same_table(
                        &out_table(&out, "monthly_table"),
                        &m["monthly_table"],
                        0,
                        1e-12,
                        &format!("{name} months"),
                    );
                }
                for k in keys {
                    check(&out_map(&out, k), c.want(k), &format!("{name} {k}"));
                }
                same_table(
                    &out_table(&out, "watershed_table"),
                    &m["watershed_table"],
                    0,
                    1e-12,
                    &format!("{name} sheds"),
                );
            }
        }
    }
    let failed = FAILED.lock().unwrap();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
