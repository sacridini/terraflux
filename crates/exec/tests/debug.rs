//! Debugging a model: a node turned off (bypassed: its inputs passed on), how long every node took (inside loops
//! too, summed over the steps), and `check.assert` stopping a run whose result is not what it should be.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_exec::{Engine, Quiet};
use terraflux_graph::Model;
use terraflux_ops::{Map, Value};
use terraflux_raster::Grid;

fn model(text: &str) -> Model {
    Model::from_toml(&format!("format = \"terraflux/1\"\n{text}")).unwrap()
}

fn land() -> BTreeMap<String, Value> {
    BTreeMap::from([("land".to_string(), Value::Map(Map::new(Arc::new(Grid::new(3, 1)), vec![1.0, 2.0, 3.0])))])
}

fn data(v: Option<&Value>) -> Vec<f64> {
    let Some(Value::Map(m)) = v else { panic!("{:?}", v.map(Value::ty)) };
    m.data.to_vec()
}

const CHAIN: &str = r#"
[inputs.land]
kind = "map"
path = "land.tif"

[[nodes]]
id = "double"
op = "map.calc"
inputs = { x = "$land" }
params = { expression = "x * 2" }

[[nodes]]
id = "plus"
op = "map.calc"
inputs = { x = "double.map" }
params = { expression = "x + 10" }

[[nodes]]
id = "loop"
op = "control.repeat"
inputs = { times = 3, m = "plus.map" }
[nodes.body]
carry = { m = "inc.map" }
[[nodes.body.nodes]]
id = "inc"
op = "map.calc"
inputs = { x = "@m" }
params = { expression = "x + 1" }
"#;

#[test]
fn a_node_bypassed_passes_its_input_on() {
    let mut m = model(CHAIN);
    let out = Engine::default().run(&m, &land(), &mut Quiet).unwrap();
    assert_eq!(data(out.get("plus.map")), [12.0, 14.0, 16.0]);
    m.nodes[0].bypass = true;
    let out = Engine::default().run(&m, &land(), &mut Quiet).unwrap();
    assert_eq!(data(out.get("double.map")), [1.0, 2.0, 3.0], "the map it was given");
    assert_eq!(data(out.get("plus.map")), [11.0, 12.0, 13.0]);
    // written in the file only when on
    let text = m.to_toml().unwrap();
    assert!(text.contains("bypass = true"));
    m.nodes[0].bypass = false;
    assert!(!m.to_toml().unwrap().contains("bypass"));
}

#[test]
fn every_node_is_timed() {
    let out = Engine::default().run(&model(CHAIN), &land(), &mut Quiet).unwrap();
    let at: Vec<&str> = out.timings.iter().map(|t| t.at.as_str()).collect();
    for node in ["double", "plus", "loop", "loop/inc"] {
        assert!(at.contains(&node), "{node}: {at:?}");
    }
    assert_eq!(at.iter().filter(|a| **a == "loop/inc").count(), 1, "a body's node summed over the steps");
    let double = out.timings.iter().find(|t| t.at == "double").unwrap();
    assert_eq!(double.bytes, 3 * 8, "a map of three cells");
    assert!(out.timings.iter().all(|t| t.seconds >= 0.0));
}

#[test]
fn a_check_stops_the_run_when_it_fails() {
    let check = |expression: &str| {
        let text = format!(
            "{CHAIN}
[[nodes]]
id = \"check\"
op = \"check.assert\"
inputs = {{ m = \"loop.m\" }}
params = {{ expression = \"{expression}\", message = \"the land grew too much\" }}
"
        );
        Engine::default().run(&model(&text), &land(), &mut Quiet).map(|_| ()).map_err(|e| e.to_string())
    };
    assert!(check("m <= 19").is_ok());
    let e = check("m < 18").err().unwrap();
    assert!(e.contains("the land grew too much (m < 18: 1 of 3 cells fail)"), "{e}");
}
