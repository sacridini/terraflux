//! futures.calibrate against landschaft's calibrate_patches (r.futures.calib): its case (a FUTURES model saved by
//! landschaft, the development observed three steps later, every combination of two compactness means, ranges and
//! discount factors run twice; tools/make_futures_calib_reference.py), run as a graph: the same table, bit for
//! bit, and the same patch library.

use std::collections::BTreeMap;
use std::path::PathBuf;

use terraflux_exec::{Engine, Quiet, load_inputs};
use terraflux_graph::Model;
use terraflux_ops::Value;

#[test]
fn calibration_is_landschafts() {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/futures_calib");
    let model = Model::load(dir.join("calibrate.json")).unwrap();
    let inputs = load_inputs(&model, &dir, &BTreeMap::new()).unwrap();
    let r = Engine::default().run(&model, &inputs, &mut Quiet).unwrap();
    let want: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("calib.json")).unwrap()).unwrap();
    let Some(Value::Table(t)) = r.get("simulation.table") else { panic!("no table") };
    let columns: Vec<String> = serde_json::from_value(want["columns"].clone()).unwrap();
    assert_eq!(t.columns, columns);
    let rows: Vec<Vec<f64>> = serde_json::from_value(want["rows"].clone()).unwrap();
    assert_eq!(t.rows, rows);
    let Some(Value::Json(lib)) = r.get("simulation.library") else { panic!("no library") };
    assert_eq!(lib["lists"][0], want["library"]);
}
