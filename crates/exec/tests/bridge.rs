//! Models written by landschaft's `Model.save` run here as `ls.simulate` runs them: the same maps, bit for
//! bit. The fixtures are written by `tools/make_reference.py`.

use std::collections::BTreeMap;
use std::path::PathBuf;

use terraflux_exec::{Engine, Quiet, load_inputs};
use terraflux_graph::Model;
use terraflux_ops::Value;
use terraflux_raster::Raster;

fn case(name: &str) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/bridge").join(name);
    let model = Model::load(dir.join("model.json")).unwrap();
    let inputs = load_inputs(&model, &dir, &BTreeMap::new()).unwrap();
    let r = Engine::default().run(&model, &inputs, &mut Quiet).unwrap();
    let Some(Value::Series(maps)) = r.get("simulation.maps") else { panic!("no maps") };
    let expected = Raster::read(dir.join("expected.tif")).unwrap();
    assert_eq!(maps.len(), expected.bands.len(), "{name}: steps");
    for (k, (m, b)) in maps.iter().zip(&expected.bands).enumerate() {
        let Value::Map(m) = m else { panic!() };
        let e = b.masked(expected.nodata);
        let differ =
            m.data.iter().zip(&e).filter(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())).count();
        assert_eq!(differ, 0, "{name}: step {}: {differ} cells differ", k + 1);
    }
}

#[test]
fn dinamica_saved_by_landschaft() {
    case("dinamica");
}

#[test]
fn clue_with_a_dynamic_variable_saved_by_landschaft() {
    case("clue_dynamic");
}

#[test]
fn cars_saved_by_landschaft() {
    case("cars");
}

#[test]
fn estimator_saved_by_landschaft_as_its_predictions() {
    case("estimator");
}

#[test]
fn regional_clue_saved_by_landschaft() {
    case("clue_regional");
}

#[test]
fn regional_cars_saved_by_landschaft() {
    case("cars_regional");
}

#[test]
fn regional_dinamica_saved_by_landschaft() {
    case("dinamica_regional");
}

#[test]
fn cost_distance_saved_by_landschaft() {
    case("clue_cost");
}

#[test]
fn molusce_saved_by_landschaft() {
    case("molusce");
}

#[test]
fn molusce_with_a_moving_driver_saved_by_landschaft() {
    case("molusce_dynamic");
}

#[test]
fn molusce_with_a_regression_saved_by_landschaft() {
    case("molusce_glm");
}
