//! The accessible area against landschaft's (tests/data/reference/sdm_area.gpkg and .json, the records
//! sdm_area_<case>.csv, tools/make_sdm_area_reference.py): buffers, the convex hull, the regions that hold the
//! records (widened or not), a buffer cut to the land and one across the antimeridian. GEOS through GDAL is not
//! shapely's GEOS: the same polygons (their areas, planar and geodesic, their boxes, and next to nothing in one
//! that is not in the other), not the same vertices.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};
use terraflux_raster::vector::{Geometry, Vector, Xy};

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn ring_area(r: &[Xy]) -> f64 {
    r.windows(2).map(|w| w[0][0] * w[1][1] - w[1][0] * w[0][1]).sum::<f64>().abs() / 2.0
}

fn area(g: &Geometry) -> f64 {
    let poly = |rings: &Vec<Vec<Xy>>| {
        rings.first().map_or(0.0, |s| ring_area(s)) - rings.iter().skip(1).map(|h| ring_area(h)).sum::<f64>()
    };
    match g {
        Geometry::Polygon(r) => poly(r),
        Geometry::MultiPolygon(p) => p.iter().map(poly).sum(),
        _ => 0.0,
    }
}

fn bounds(g: &Geometry) -> [f64; 4] {
    let rings: Vec<&Vec<Xy>> = match g {
        Geometry::Polygon(r) => r.iter().collect(),
        Geometry::MultiPolygon(p) => p.iter().flatten().collect(),
        _ => Vec::new(),
    };
    let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
    for p in rings.into_iter().flatten() {
        b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
    }
    b
}

fn close(a: f64, b: f64, tol: f64) -> bool {
    (a - b).abs() <= tol * b.abs().max(1.0)
}

#[test]
fn areas_are_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_area.json")).unwrap()).unwrap();
    let read = |path: &str, layer: Option<&str>| Arc::new(terraflux_gdal::read_vector(dir.join(path), layer).unwrap());
    let countries = Value::Vector(read("sdm_clean.gpkg", Some("countries")));
    let land = Value::Vector(read("sdm_clean.gpkg", Some("land")));
    for (case, c) in meta.as_object().unwrap() {
        let mut settings = c["settings"].as_object().unwrap().clone();
        let records = format!("sdm_area_{}.csv", c["records"].as_str().unwrap());
        let mut inputs = vec![("occurrences", Value::Vector(read(&records, None)))];
        if settings.remove("_regions").is_some() {
            inputs.push(("regions", countries.clone()));
        }
        if settings.remove("_within").is_some() {
            inputs.push(("within", land.clone()));
        }
        let out = run("sdm.accessible_area", inputs, serde_json::Value::Object(settings));
        let Value::Vector(v) = &out["area"] else { panic!() };
        let ours = &v.geometries[0];
        let theirs = &read("sdm_area.gpkg", Some(case)).geometries[0];
        let a = c["area"].as_f64().unwrap();
        assert!(close(area(ours), a, 1e-9), "{case}: area {} {a}", area(ours));
        let geodesic = terraflux_gdal::geodesic_areas(v).unwrap()[0];
        let g = c["geodesic"].as_f64().unwrap();
        assert!(close(geodesic, g, 1e-9), "{case}: geodesic area {geodesic} {g}");
        let want: Vec<f64> = c["bounds"].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        let b = bounds(ours);
        assert!((0..4).all(|k| close(b[k], want[k], 1e-12)), "{case}: bounds {b:?} {want:?}");
        for (x, y) in [(ours, theirs), (theirs, ours)] {
            let d = area(&terraflux_gdal::difference(x, y).unwrap());
            assert!(d <= 1e-9 * a, "{case}: {d} of difference");
        }
        let Vector { attributes, crs, .. } = &**v;
        assert_eq!(crs.as_deref(), Some("EPSG:4326"));
        assert_eq!(attributes[0].0, "method");
    }
}
