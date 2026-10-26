//! A node of the user's library in a model: a group with what it declares (its inputs typed, the layers it reads,
//! the values of its own of those not given), run from the copy the model holds.

use std::collections::BTreeMap;

use terraflux_exec::{Engine, Quiet};
use terraflux_graph::{Model, Severity, check};
use terraflux_ops::{Registry, Value};

const MODEL: &str = r#"format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random"
params = { rows = 20, columns = 30, seed = 3 }

[[nodes]]
id = "twice"
op = "map.calc"
inputs = { a = "land.map" }
params = { expression = "a * 2" }

[[nodes]]
id = "s"
op = "stack.of"
inputs = { red = "land.map", nir = "twice.map" }

[[nodes]]
id = "veg"
op = "control.group"
inputs = { img = "s.stack" }
[nodes.uses]
name = "ndvi"
version = 3
description = "the vegetation index, times k"
[nodes.uses.inputs.img]
type = "stack"
bands = ["nir", "red"]
[nodes.uses.inputs.k]
type = "number"
default = 10
[nodes.uses.outputs.ndvi]
type = "map"
[nodes.body]
outputs = { ndvi = "calc.ndvi" }
[[nodes.body.nodes]]
id = "calc"
op = "map.calc"
inputs = { img = "@img", k = "@k" }
params = { expression = "out ndvi = (img.nir - img.red) / (img.nir + img.red) * k" }
"#;

#[test]
fn a_node_of_the_library_runs_from_its_copy() {
    let model = Model::from_toml(MODEL).unwrap();
    let issues = check(&model, &Registry::standard());
    assert!(issues.iter().all(|i| i.severity != Severity::Error), "{issues:?}");
    let r = Engine::default().run(&model, &BTreeMap::new(), &mut Quiet).unwrap();
    // (2a - a) / (2a + a) * 10, k's value its own
    let Some(Value::Map(ndvi)) = r.get("veg.ndvi") else { panic!("no ndvi") };
    assert!(ndvi.data.iter().all(|x| x.is_nan() || (x - 10.0 / 3.0).abs() < 1e-9), "{:?}", &ndvi.data[..4]);
}

#[test]
fn a_layer_it_reads_and_is_not_given_is_told() {
    let model = Model::from_toml(&MODEL.replace("nir = \"twice.map\"", "b8 = \"twice.map\"")).unwrap();
    let e = Engine::default().run(&model, &BTreeMap::new(), &mut Quiet).err().unwrap().to_string();
    assert!(e.contains("ndvi@3 reads img's layers: it has no layer nir (it has b8, red)"), "{e}");
    let model = Model::from_toml(&MODEL.replace("img = \"s.stack\"", "img = \"land.map\"")).unwrap();
    // the check tells first: a map where it takes a stack
    let issues = check(&model, &Registry::standard());
    assert!(issues.iter().any(|i| i.message.contains("land.map gives a Map, the port takes a Stack")), "{issues:?}");
}
