//! Biodiverse's other imports against landschaft's (tests/data/reference/bio_imports.json and its inputs,
//! tools/make_bio_imports_reference.py): rasters (labels as bands, a threshold, larger cells, an origin, one label,
//! values as labels), vector features over a fishnet (the cells touched, areas, lengths, counts, binarised),
//! hexagons and H3 cells, and occurrence records of GBIF downloads and a Darwin Core Archive, cleaned and in
//! cells: the same groups, labels, counts and coordinates (areas and lengths of GEOS's pieces to 1e-12).

use std::path::Path;
use std::sync::Arc;

use serde_json::Value as Json;
use terraflux_ops::{Args, Ctx, Key, Outputs, Registry, Stack, Value};
use terraflux_raster::vector::Vector;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: Json) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn dir() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference")
}

fn strings(v: &Json) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect()
}

/// The basedata of a node against landschaft's: groups, labels and coordinates the same; counts to `tol`.
fn check(case: &str, out: &Outputs, want: &Json, tol: f64) {
    let Value::BaseData(bd) = &out["basedata"] else { panic!() };
    assert_eq!(bd.groups, strings(&want["groups"]), "{case}: groups");
    assert_eq!(bd.labels, strings(&want["labels"]), "{case}: labels");
    let coords: Vec<f64> = want["coords"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap())
        .map(|v| v.as_f64().unwrap())
        .collect();
    assert_eq!(bd.coords, coords, "{case}: coordinates");
    let sizes: Vec<f64> = want["cell_sizes"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    assert_eq!(bd.cell_sizes, sizes, "{case}: cell sizes");
    let dense = want["counts"].as_array().unwrap();
    for (g, want_row) in dense.iter().enumerate().take(bd.groups.len()) {
        let mut row = vec![0.0; bd.labels.len()];
        for e in bd.indptr[g] as usize..bd.indptr[g + 1] as usize {
            row[bd.indices[e] as usize] = bd.counts[e];
        }
        let w: Vec<f64> = want_row.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
        for (l, (a, b)) in row.iter().zip(&w).enumerate() {
            assert!(
                (a - b).abs() <= tol * b.abs().max(1.0),
                "{case}: group {} label {}: {a} {b}",
                bd.groups[g],
                bd.labels[l]
            );
        }
    }
    if let Some(grid) = want.get("grid").filter(|g| !g.is_null()) {
        assert_eq!(bd.grid.as_ref(), Some(grid), "{case}: grid");
    }
}

#[test]
fn rasters_are_landschafts() {
    let meta: Json = serde_json::from_str(&std::fs::read_to_string(dir().join("bio_imports.json")).unwrap()).unwrap();
    for (case, c) in meta["raster"].as_object().unwrap() {
        let path = dir().join(c["file"].as_str().unwrap());
        let read = run("raster.read", vec![], serde_json::json!({ "path": path.to_string_lossy() }));
        let Value::Raster(r) = &read["stack"] else { panic!() };
        let s = &r.stack().unwrap();
        // the labels from_raster gives the bands: their descriptions, else band<i>
        let keys = if s.layers.len() == 3 { vec!["sp_a", "sp_b", "band3"] } else { vec!["classes"] };
        let stack = Value::Stack(Stack {
            grid: s.grid.clone(),
            keys: keys.iter().map(|k| Key::Name(k.to_string())).collect(),
            layers: s.layers.clone(),
        });
        let out = run("bio.raster", vec![("raster", stack)], c["settings"].clone());
        check(case, &out, c, 0.0);
    }
}

#[test]
fn features_are_landschafts() {
    let meta: Json = serde_json::from_str(&std::fs::read_to_string(dir().join("bio_imports.json")).unwrap()).unwrap();
    let all = terraflux_gdal::read_vector(dir().join("bio_imports.gpkg"), Some("features")).unwrap();
    let part = |r: std::ops::Range<usize>| {
        let mut v =
            Vector { geometries: all.geometries[r.clone()].to_vec(), attributes: Vec::new(), crs: all.crs.clone() };
        for (n, c) in &all.attributes {
            let c = match c {
                terraflux_raster::vector::Column::Numbers(x) => {
                    terraflux_raster::vector::Column::Numbers(x[r.clone()].to_vec())
                }
                terraflux_raster::vector::Column::Texts(t) => {
                    terraflux_raster::vector::Column::Texts(t[r.clone()].to_vec())
                }
            };
            v.attributes.push((n.clone(), c));
        }
        Value::Vector(Arc::new(v))
    };
    for (case, c) in meta["vector"].as_object().unwrap() {
        let features = match c["part"].as_str() {
            Some("polygons") => part(0..3),
            Some("lines") => part(3..5),
            _ => part(0..all.len()),
        };
        let s = &c["settings"];
        let mut params = serde_json::json!({"label": "sp", "cell_sizes": s["cell_sizes"]});
        for (k, to) in [
            ("cell_origins", "cell_origins"),
            ("count_columns", "count"),
            ("measure", "measure"),
            ("binarise_counts", "binarise"),
        ] {
            if let Some(v) = s.get(k) {
                params[to] = v.clone();
            }
        }
        let out = run("bio.vector", vec![("features", features)], params);
        check(case, &out, c, 1e-12);
    }
}

#[test]
fn hexagons_and_h3_are_landschafts() {
    let meta: Json = serde_json::from_str(&std::fs::read_to_string(dir().join("bio_imports.json")).unwrap()).unwrap();
    let csv = |name: &str| Value::Vector(Arc::new(terraflux_gdal::read_vector(dir().join(name), None).unwrap()));
    for (case, c) in meta["grids"].as_object().unwrap() {
        let s = &c["settings"];
        let out = if case.starts_with("hex") {
            let mut p = serde_json::json!({"label": "sp", "size": s["size"], "x": "x", "y": "y"});
            for k in ["orientation", "origin"] {
                if let Some(v) = s.get(k) {
                    p[k] = v.clone();
                }
            }
            if let Some(v) = s.get("count_column") {
                p["count"] = v.clone();
            }
            run("bio.hex", vec![("records", csv("bio_imports_points.csv"))], p)
        } else {
            let p = serde_json::json!({"label": "sp", "resolution": s["resolution"], "lon": "lon", "lat": "lat"});
            run("bio.h3", vec![("records", csv("bio_imports_lonlat.csv"))], p)
        };
        check(case, &out, c, 0.0);
    }
}

#[test]
fn occurrences_are_landschafts() {
    let meta: Json = serde_json::from_str(&std::fs::read_to_string(dir().join("bio_imports.json")).unwrap()).unwrap();
    for (case, c) in meta["occurrences"].as_object().unwrap() {
        let mut p = c["settings"].clone();
        p["path"] = Json::String(dir().join(c["file"].as_str().unwrap()).to_string_lossy().into_owned());
        let out = run("bio.occurrences", vec![], p);
        check(case, &out, c, 0.0);
    }
    // records with Darwin Core columns (a table read by GDAL) give the same as the file
    let c = &meta["occurrences"]["csv"];
    let records =
        Value::Vector(Arc::new(terraflux_gdal::read_vector(dir().join("bio_imports_gbif.csv"), None).unwrap()));
    let out = run("bio.occurrences", vec![("occurrences", records)], c["settings"].clone());
    check("csv as records", &out, c, 0.0);
}
