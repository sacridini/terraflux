//! ProtConn against landschaft's (tests/data/reference/protconn.gpkg, .npz and .json,
//! tools/make_protconn_reference.py): two distances, transboundary nodes around the nodes and around the
//! region, the region's parts, the probability exp(-1), least-cost distances, a single protected area, every
//! protected area's importance and ProtConn per region. To the bit, but for what goes through numpy's
//! exponentials (the probabilities of the paths): those to rounding.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

/// Bit for bit, or within 1e-12 of each other; how many were not the same bits.
fn close(what: &str, got: &[f64], want: &[f64]) -> usize {
    assert_eq!(got.len(), want.len(), "{what}: length");
    let mut rounded = 0;
    for (k, (a, b)) in got.iter().zip(want).enumerate() {
        if a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) {
            continue;
        }
        rounded += 1;
        assert!((a - b).abs() <= 1e-12 * a.abs().max(b.abs()), "{what} at {k}: {a} {b}");
    }
    rounded
}

#[test]
fn protconn_is_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("protconn.npz")).unwrap();
    let cases: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("protconn.json")).unwrap()).unwrap();
    let layer = |name: &str| terraflux_gdal::read_vector(dir.join("protconn.gpkg"), Some(name)).unwrap();
    let pas = layer("protected");
    let region = Value::Vector(Arc::new(layer("region")));
    let regions = Value::Vector(Arc::new(layer("regions")));
    let r = &z["in/resistance"];
    let mut g = Grid::new(r.shape[1], r.shape[0]);
    g.transform = [0.0, 30.0, 0.0, r.shape[0] as f64 * 30.0, 0.0, -30.0];
    let resistance = Value::Map(Map::new(Arc::new(g), r.data.clone()));
    let mut rounded = Vec::new();
    let raster = |name: &str| {
        let a = &z[name];
        let mut g = Grid::new(a.shape[1], a.shape[0]);
        g.transform = [0.0, 30.0, 0.0, a.shape[0] as f64 * 30.0, 0.0, -30.0];
        Value::Map(Map::new(Arc::new(g), a.data.clone()))
    };
    for c in cases["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        if c["op"] == "conn.protconn_raster" {
            let mut inputs = vec![("protected", raster("in/pa_raster"))];
            if c.get("region_map").is_some() {
                inputs.push(("region_map", raster("in/region_raster")));
            } else {
                inputs.push(("region", Value::Vector(Arc::new(layer("raster_region")))));
            }
            let out = run("conn.protconn_raster", inputs, c["params"].clone());
            let Value::Table(t) = &out["table"] else { panic!("{name}: no table") };
            let cols: Vec<String> =
                c["columns"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
            assert_eq!(t.columns, cols, "{name}: columns");
            let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
            let n = close(name, &got, &z[name].data);
            if n > 0 {
                rounded.push(format!("{name}: {n}"));
            }
            continue;
        }
        let mut params = c["params"].clone();
        let protected = match params.get("only") {
            Some(only) => {
                let keep: Vec<usize> = only.as_array().unwrap().iter().map(|k| k.as_u64().unwrap() as usize).collect();
                Vector {
                    geometries: keep.iter().map(|&k| pas.geometries[k].clone()).collect(),
                    attributes: vec![],
                    crs: pas.crs.clone(),
                }
            }
            None => pas.clone(),
        };
        params.as_object_mut().unwrap().remove("only");
        let mut inputs = vec![("protected", Value::Vector(Arc::new(protected)))];
        if c["op"] == "conn.protconn_regions" {
            inputs.push(("regions", regions.clone()));
        } else {
            inputs.push(("region", region.clone()));
        }
        if c.get("resistance").is_some() {
            inputs.push(("resistance", resistance.clone()));
        }
        let out = run(c["op"].as_str().unwrap(), inputs, params);
        let Value::Table(t) = &out["table"] else { panic!("{name}: no table") };
        let cols: Vec<String> = c["columns"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
        assert_eq!(t.columns, cols, "{name}: columns");
        let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
        let n = close(name, &got, &z[name].data);
        if n > 0 {
            rounded.push(format!("{name}: {n}"));
        }
        if name == "delta_table" {
            let Value::Vector(d) = &out["delta"] else { panic!("no delta") };
            let cols: Vec<f64> =
                (0..d.len()).flat_map(|k| d.attributes.iter().map(move |(_, c)| c.numbers()[k])).collect();
            let n = close("delta", &cols, &z["delta"].data);
            if n > 0 {
                rounded.push(format!("delta: {n}"));
            }
        }
    }

    // the grids' cells, coordinate by coordinate
    let coords = |g: &Geometry| -> Vec<f64> {
        match g {
            Geometry::Polygon(rs) => rs.iter().flatten().flat_map(|c| [c[0], c[1]]).collect(),
            Geometry::MultiPolygon(ps) => ps.iter().flatten().flatten().flat_map(|c| [c[0], c[1]]).collect(),
            g => panic!("{g:?}"),
        }
    };
    for c in cases["grids"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let out = run("conn.make_grid", vec![("region", region.clone())], c["params"].clone());
        let Value::Vector(v) = &out["grid"] else { panic!() };
        let n: Vec<f64> = v.geometries.iter().map(|g| (coords(g).len() / 2) as f64).collect();
        let xy: Vec<f64> = v.geometries.iter().flat_map(coords).collect();
        close(&format!("grid {name} counts"), &n, &z[&format!("grid_{name}_n")].data);
        assert_eq!(close(&format!("grid {name}"), &xy, &z[&format!("grid_{name}")].data), 0, "{name}");
    }
    let times = Value::Series((1..=3).map(|k| Value::Vector(Arc::new(layer(&format!("time{k}"))))).collect());
    for c in cases["vectors"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let mut inputs = vec![("region", region.clone())];
        if c["op"] == "conn.connectivity_grid" {
            inputs.push(("protected", Value::Vector(Arc::new(pas.clone()))));
        } else {
            inputs.push(("times", times.clone()));
        }
        let out = run(c["op"].as_str().unwrap(), inputs, c["params"].clone());
        let Value::Vector(v) = &out["grid"] else { panic!() };
        let cols: Vec<&str> = c["columns"].as_array().unwrap().iter().map(|s| s.as_str().unwrap()).collect();
        let col =
            |name: &str| v.attributes.iter().find(|(n, _)| n == name).unwrap_or_else(|| panic!("{name}")).1.clone();
        let got: Vec<f64> =
            (0..v.len()).flat_map(|k| cols.iter().map(|c| col(c).numbers()[k]).collect::<Vec<_>>()).collect();
        let n = close(name, &got, &z[name].data);
        if n > 0 {
            rounded.push(format!("{name}: {n}"));
        }
        if let Some(texts) = c.get("texts") {
            for (t, want) in texts.as_object().unwrap() {
                let Column::Texts(got) = col(t) else { panic!("{t}") };
                let want: Vec<String> = want.as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
                assert_eq!(got, want, "{name} {t}");
            }
        }
    }
    eprintln!("to rounding: {rounded:?}");
}
