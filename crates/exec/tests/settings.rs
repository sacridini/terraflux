//! Settings that read values: an input (`"$x"`), a loop's step or carried value (`"@step"`), another node's output
//! (`{ from = "node.port" }`), and a submodel's input, so that a submodel shows its settings as inputs. The node
//! read runs first; a value that is not a number, true or false or a text is refused.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_exec::{Engine, Observer};
use terraflux_graph::{Access, Model, OpSignature, PortSpec, Type};
use terraflux_ops::{Args, Ctx, Map, Operator, Outputs, Registry, Value};
use terraflux_raster::Grid;

struct Quiet;
impl Observer for Quiet {}

fn outputs<const N: usize>(pairs: [(&str, Value); N]) -> Outputs {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// `test.scale`: a map times the setting factor.
struct Scale;

impl Operator for Scale {
    fn signature(&self) -> OpSignature {
        OpSignature::new("test.scale", "", Access::Local)
            .input(PortSpec::new("map", Type::Map, ""))
            .output(PortSpec::new("map", Type::Map, ""))
            .param(PortSpec::new("factor", Type::Number, ""))
    }

    fn run(&self, _ctx: &mut Ctx, a: &Args) -> terraflux_ops::Result<Outputs> {
        let m = a.map("map")?;
        let f: f64 = a.param("factor", f64::NAN)?;
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), m.data.iter().map(|x| x * f).collect())))]))
    }
}

/// `test.count`: how many cells a map has (a whole number).
struct Count;

impl Operator for Count {
    fn signature(&self) -> OpSignature {
        OpSignature::new("test.count", "", Access::Global)
            .input(PortSpec::new("map", Type::Map, ""))
            .output(PortSpec::new("n", Type::Integer, ""))
    }

    fn run(&self, _ctx: &mut Ctx, a: &Args) -> terraflux_ops::Result<Outputs> {
        Ok(outputs([("n", Value::Integer(a.map("map")?.data.len() as i64))]))
    }
}

fn engine() -> Engine {
    let mut r = Registry::standard();
    r.add(Arc::new(Scale));
    r.add(Arc::new(Count));
    Engine { registry: r }
}

fn ones() -> Value {
    Value::Map(Map::new(Arc::new(Grid::new(3, 2)), vec![1.0; 6]))
}

fn run(text: &str) -> Result<terraflux_exec::Results, String> {
    let m = Model::from_toml(&format!("format = \"terraflux/1\"\n{text}")).unwrap();
    let mut inputs = BTreeMap::from([("land".to_string(), ones())]);
    for (name, i) in &m.inputs {
        if let terraflux_graph::Input::Value { value, .. } = i {
            inputs.insert(name.clone(), Value::from_json(value));
        }
    }
    engine().run(&m, &inputs, &mut Quiet).map_err(|e| e.to_string())
}

fn data(v: &Value) -> Vec<f64> {
    let Value::Map(m) = v else { panic!("{v:?}") };
    m.data.to_vec()
}

#[test]
fn a_setting_reads_an_input_a_step_and_a_nodes_output() {
    let out = run(r#"
[inputs.land]
kind = "map"
path = "land.tif"

[inputs.factor]
kind = "value"
value = 2.5

[[nodes]]
id = "by_input"
op = "test.scale"
inputs = { map = "$land" }
params = { factor = "$factor" }

[[nodes]]
id = "cells"
op = "test.count"
inputs = { map = "$land" }

[[nodes]]
id = "by_node"
op = "test.scale"
inputs = { map = "$land" }
params = { factor = { from = "cells.n" } }

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 3 }
[nodes.body]
collect = { maps = "s.map" }
[[nodes.body.nodes]]
id = "s"
op = "test.scale"
inputs = { map = "$land" }
params = { factor = "@step" }

[outputs.by_input]
from = "by_input.map"
[outputs.by_node]
from = "by_node.map"
[outputs.years]
from = "years.maps"
"#)
    .unwrap();
    assert_eq!(data(out.get("by_input.map").unwrap()), [2.5; 6]);
    assert_eq!(data(out.get("by_node.map").unwrap()), [6.0; 6], "the cells counted first");
    let Some(Value::Series(steps)) = out.get("years.maps") else { panic!() };
    let firsts: Vec<f64> = steps.iter().map(|s| data(s)[0]).collect();
    assert_eq!(firsts, [1.0, 2.0, 3.0], "a factor that changes with the step");
}

#[test]
fn a_setting_refuses_what_is_not_a_value() {
    let e = run(r#"
[inputs.land]
kind = "map"
path = "land.tif"

[[nodes]]
id = "s"
op = "test.scale"
inputs = { map = "$land" }
params = { factor = "$land" }
"#)
    .err()
    .unwrap();
    assert!(e.contains("gives a Map: a setting takes a number"), "{e}");
}

#[test]
fn a_submodel_takes_its_settings_as_inputs() {
    let dir = std::env::temp_dir().join(format!("terraflux-settings-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("scaled.toml"),
        r#"format = "terraflux/1"

[inputs.map]
kind = "map"
path = "none.tif"

[inputs.factor]
kind = "value"
value = 1

[[nodes]]
id = "s"
op = "test.scale"
inputs = { map = "$map" }
params = { factor = "$factor" }

[outputs.map]
from = "s.map"
"#,
    )
    .unwrap();
    let outer = r#"format = "terraflux/1"

[inputs.land]
kind = "map"
path = "land.tif"

[[nodes]]
id = "default"
op = "control.submodel"
inputs = { map = "$land" }
params = { path = "scaled.toml" }

[[nodes]]
id = "tripled"
op = "control.submodel"
inputs = { map = "$land", factor = 3 }
params = { path = "scaled.toml" }

[outputs.default]
from = "default.map"
[outputs.tripled]
from = "tripled.map"
"#;
    let mut m = Model::from_toml(outer).unwrap();
    terraflux_graph::submodel::expand(&mut m, &dir).unwrap();
    let inputs = BTreeMap::from([("land".to_string(), ones())]);
    let out = engine().run(&m, &inputs, &mut Quiet).unwrap();
    assert_eq!(data(out.get("default.map").unwrap()), [1.0; 6], "the submodel's own value");
    assert_eq!(data(out.get("tripled.map").unwrap()), [3.0; 6], "the setting given from outside");
    let _ = std::fs::remove_dir_all(&dir);
}
