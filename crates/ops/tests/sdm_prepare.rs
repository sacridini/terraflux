//! The preparation of a species distribution model against landschaft's (tests/data/reference/sdm_prepare.npz and
//! .json, tools/make_sdm_prepare_reference.py): bradypus' records over the bioclimatic layers, with the steps of
//! landschaft's own test, the defaults, cleaning, an area given, every kind of folds and blocks of the
//! autocorrelation range; the records (coordinates, presence, predictors, fold) to the bit, the predictors kept
//! and the report.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Key, Outputs, Registry, Stack, Value};
use terraflux_raster::Grid;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

#[test]
fn preparation_is_landschafts() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let z = terraflux_raster::npy::read_npz(dir.join("sdm_prepare.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("sdm_prepare.json")).unwrap()).unwrap();
    let e = &z["in/env"];
    let (nb, ny, nx) = (e.shape[0], e.shape[1], e.shape[2]);
    let mut g = Grid::new(nx, ny);
    let t: Vec<f64> = meta["env"]["transform"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect();
    // rasterio's affine (a, b, c, d, e, f) as GDAL's geotransform
    g.transform = [t[2], t[0], t[1], t[5], t[3], t[4]];
    g.geo.wkt = Some("EPSG:4326".into());
    let names: Vec<&str> = meta["env"]["names"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    let env = Value::Stack(Stack {
        grid: Arc::new(g),
        keys: names.iter().map(|n| Key::Name(n.to_string())).collect(),
        layers: (0..nb).map(|b| Arc::new(e.data[b * ny * nx..(b + 1) * ny * nx].to_vec())).collect(),
    });
    let occ = Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("sdm_area_bradypus.csv"), None).unwrap()));
    let area = Value::Vector(Arc::new(terraflux_gdal::read_vector(dir.join("sdm_area.gpkg"), Some("buffer")).unwrap()));
    for (case, c) in meta["cases"].as_object().unwrap() {
        let mut settings = c["settings"].as_object().unwrap().clone();
        let mut inputs = vec![("occurrences", occ.clone()), ("env", env.clone())];
        if settings.remove("_area").is_some() {
            inputs.push(("area", area.clone()));
        }
        let out = run("sdm.prepare", inputs, serde_json::Value::Object(settings));
        let Value::Table(t) = &out["records"] else { panic!() };
        let predictors: Vec<&str> = c["predictors"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(&t.columns[3..3 + predictors.len()], &predictors[..], "{case}: predictors");
        let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
        let want = &z[case.as_str()].data;
        assert_eq!(got.len(), want.len(), "{case}: records {} columns {:?}", t.rows.len(), t.columns);
        if let Some(k) = got.iter().zip(want).position(|(a, b)| a.to_bits() != b.to_bits()) {
            let w = t.columns.len();
            panic!("{case}: record {} column {}: {} {}", k / w, t.columns[k % w], got[k], want[k]);
        }
        let Value::Json(report) = &out["report"] else { panic!() };
        for (r, w) in report.as_array().unwrap().iter().zip(c["report"].as_array().unwrap()) {
            assert_eq!(r["step"], w[0], "{case}");
            assert_eq!(r["presences"], w[1], "{case}: {}", w[0]);
            assert_eq!(r["removed"], w[2], "{case}: {}", w[0]);
            assert_eq!(r["detail"], w[3], "{case}: {}", w[0]);
        }
        assert_eq!(report.as_array().unwrap().len(), c["report"].as_array().unwrap().len(), "{case}: steps");
    }
}
