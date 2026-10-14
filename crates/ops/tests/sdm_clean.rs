//! The cleaning of occurrence records against landschaft's (tests/data/reference/sdm_clean.npz, .json, .gpkg and
//! the records sdm_clean_<case>.csv, tools/make_sdm_clean_reference.py): CoordinateCleaner's tests with their
//! defaults and other options on records near each kind of reference, on the vertices of the circles, at the
//! antimeridian and the south pole, on acaule's records and on a species of more than 10 000 records, every flag
//! and count the same.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};
use terraflux_raster::vector::Column;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

#[test]
fn flags_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_clean.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_clean.json")).unwrap()).unwrap();
    let csv = |name: &str| {
        Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join(format!("sdm_clean_{name}.csv")), None).unwrap()))
    };
    let layer = |name: &str| {
        Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("sdm_clean.gpkg"), Some(name)).unwrap()))
    };
    let (land, urban, countries) = (layer("land"), layer("urban"), layer("countries"));
    for (case, c) in meta.as_object().unwrap() {
        let mut settings = c["settings"].as_object().unwrap().clone();
        let mut inputs = vec![
            ("occurrences", csv(c["records"].as_str().unwrap())),
            ("seas_ref", land.clone()),
            ("urban_ref", urban.clone()),
            ("countries_ref", countries.clone()),
        ];
        if let Some(r) = settings.remove("_capitals_ref") {
            inputs.push(("capitals_ref", csv(r.as_str().unwrap())));
        }
        settings.insert("action".into(), "flag".into());
        let out = run("sdm.clean_occurrences", inputs, serde_json::Value::Object(settings));
        let Value::Vector(v) = &out["occurrences"] else { panic!() };
        let mut checked = 0;
        for (name, col) in &v.attributes {
            if !name.starts_with("flag") {
                continue;
            }
            let Column::Numbers(got) = col else { panic!("{case}: {name}") };
            let want = &z[format!("{case}/{name}").as_str()].data;
            assert_eq!(got.len(), want.len(), "{case}: {name}");
            if let Some(k) = got.iter().zip(want).position(|(a, b)| a != b) {
                panic!("{case}: {name}: record {k}: {} {}", got[k], want[k]);
            }
            checked += 1;
        }
        let names: Vec<&str> = c["counts"].as_array().unwrap().iter().map(|n| n.as_str().unwrap()).collect();
        assert_eq!(checked, names.len(), "{case}: the tests run");
        let Value::Table(t) = &out["counts"] else { panic!() };
        assert_eq!(t.columns, names, "{case}: counts");
        assert_eq!(t.rows[0], z[format!("{case}/counts").as_str()].data, "{case}: counts");
    }
}

#[test]
fn drops_the_flagged_and_reads_the_tables() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let occ = Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("sdm_clean_synthetic.csv"), None).unwrap()));
    let off = serde_json::json!({"seas": false, "outliers": false});
    let out = run("sdm.clean_occurrences", vec![("occurrences", occ.clone())], off.clone());
    let Value::Vector(kept) = &out["occurrences"] else { panic!() };
    let Value::Table(t) = &out["counts"] else { panic!() };
    let Value::Vector(all) = &occ else { panic!() };
    assert_eq!(kept.len() as f64, all.len() as f64 - t.rows[0][t.columns.len() - 1]);
    for name in ["countryref", "institutions", "aohi"] {
        let out = run("sdm.reference_data", vec![], serde_json::json!({ "name": name }));
        let Value::Vector(v) = &out["features"] else { panic!() };
        assert!(v.len() > 300, "{name}: {}", v.len());
    }
}

/// The Natural Earth layers downloaded (network): the same flags as the polygons of the tests.
#[test]
#[ignore]
fn downloads_natural_earth() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_clean.npz")).unwrap();
    let occ = Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("sdm_clean_synthetic.csv"), None).unwrap()));
    let settings = serde_json::json!({"action": "flag", "urban": true, "countries": "iso3", "duplicates": true,
                                      "aohi": true});
    let out = run("sdm.clean_occurrences", vec![("occurrences", occ)], settings);
    let Value::Vector(v) = &out["occurrences"] else { panic!() };
    for (name, col) in &v.attributes {
        if let (true, Column::Numbers(got)) = (name.starts_with("flag"), col) {
            assert_eq!(got, &z[format!("synthetic/{name}").as_str()].data, "{name}");
        }
    }
}
