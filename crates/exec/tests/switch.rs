//! `control.switch` (the branch of its case runs, the others do not; every branch gives the same outputs) and
//! `value.calc` (a number or a text from the inputs) feeding the settings that read values.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_exec::{Engine, Observer, Results};
use terraflux_graph::Model;
use terraflux_ops::{Map, Value};
use terraflux_raster::Grid;

/// The nodes run, by path.
#[derive(Default)]
struct Ran(Vec<String>);

impl Observer for Ran {
    fn node(&mut self, at: &str) {
        self.0.push(at.to_string());
    }
}

fn run(text: &str, case: Value) -> (Result<Results, String>, Vec<String>) {
    let m = Model::from_toml(&format!("format = \"terraflux/1\"\n{text}")).unwrap();
    let land = Value::Map(Map::new(Arc::new(Grid::new(2, 1)), vec![1.0, 2.0]));
    let inputs = BTreeMap::from([("land".to_string(), land), ("method".to_string(), case)]);
    let mut ran = Ran::default();
    let out = Engine::default().run(&m, &inputs, &mut ran).map_err(|e| e.to_string());
    (out, ran.0)
}

fn data(v: Option<&Value>) -> Vec<f64> {
    let Some(Value::Map(m)) = v else { panic!("{:?}", v.map(Value::ty)) };
    m.data.to_vec()
}

const SWITCH: &str = r#"
[inputs.land]
kind = "map"
path = "land.tif"

[inputs.method]
kind = "value"
value = "double"

[[nodes]]
id = "sw"
op = "control.switch"
inputs = { case = "$method", map = "$land" }
[[nodes.body.nodes]]
id = "double"
op = "control.group"
inputs = { x = "@map" }
[nodes.body.nodes.body]
outputs = { map = "d.map" }
[[nodes.body.nodes.body.nodes]]
id = "d"
op = "map.calc"
inputs = { x = "@x" }
params = { expression = "x * 2" }
[[nodes.body.nodes]]
id = "case_3"
op = "control.group"
inputs = { x = "@map" }
[nodes.body.nodes.body]
outputs = { map = "t.map" }
[[nodes.body.nodes.body.nodes]]
id = "t"
op = "map.calc"
inputs = { x = "@x" }
params = { expression = "x * 3" }
[[nodes.body.nodes]]
id = "otherwise"
op = "control.group"
inputs = { x = "@map" }
[nodes.body.nodes.body]
outputs = { map = "o.map" }
[[nodes.body.nodes.body.nodes]]
id = "o"
op = "map.calc"
inputs = { x = "@x" }
params = { expression = "x" }

[[nodes]]
id = "after"
op = "map.calc"
inputs = { y = "sw.map" }
params = { expression = "y + 1" }
"#;

#[test]
fn the_branch_of_the_case_runs_alone() {
    let (out, ran) = run(SWITCH, Value::Text("double".into()));
    let out = out.unwrap();
    assert_eq!(data(out.get("sw.map")), [2.0, 4.0]);
    assert_eq!(data(out.get("after.map")), [3.0, 5.0], "what reads the switch reads the branch's output");
    assert!(matches!(out.get("sw.branch"), Some(Value::Text(t)) if t == "double"));
    assert!(ran.iter().any(|a| a == "sw/double/d") && !ran.iter().any(|a| a.starts_with("sw/case_3")), "{ran:?}");
    // a number: the branch case_<number>; anything else: otherwise
    let (out, _) = run(SWITCH, Value::Integer(3));
    assert_eq!(data(out.unwrap().get("sw.map")), [3.0, 6.0]);
    let (out, _) = run(SWITCH, Value::Text("glm".into()));
    let out = out.unwrap();
    assert!(matches!(out.get("sw.branch"), Some(Value::Text(t)) if t == "otherwise"));
    assert_eq!(data(out.get("sw.map")), [1.0, 2.0]);
}

#[test]
fn what_a_switch_cannot_do_is_refused() {
    // no branch for the case, and none otherwise
    let without = SWITCH.split("[[nodes.body.nodes]]\nid = \"otherwise\"").next().unwrap().to_string()
        + "\n[[nodes]]\nid = \"after\"\nop = \"map.calc\"\ninputs = { y = \"sw.map\" }\nparams = { expression = \"y + 1\" }\n";
    let (out, _) = run(&without, Value::Text("glm".into()));
    let e = out.err().unwrap();
    assert!(e.contains("no branch for the case \"glm\" (its branches: double, case_3"), "{e}");
    // branches that give different outputs
    let (out, _) = run(
        &SWITCH.replacen("outputs = { map = \"t.map\" }", "outputs = { other = \"t.map\" }", 1),
        Value::Text("double".into()),
    );
    let e = out.err().unwrap();
    assert!(e.contains("every branch gives the same outputs"), "{e}");
    // a node of the body that is not a group
    let (out, _) = run(
        &SWITCH.replacen("id = \"otherwise\"\nop = \"control.group\"", "id = \"otherwise\"\nop = \"map.calc\"", 1),
        Value::Text("double".into()),
    );
    assert!(out.err().unwrap().contains("a branch of control.switch is a group"));
}

#[test]
fn a_value_calculated_feeds_the_settings() {
    let (out, _) = run(
        r#"
[inputs.land]
kind = "map"
path = "land.tif"

[inputs.k]
kind = "value"
value = 4

[[nodes]]
id = "stats"
op = "raster.stats"
inputs = { map = "$land" }

[[nodes]]
id = "v"
op = "value.calc"
inputs = { k = "$k", s = "stats.table" }
params = { expression = "k / 2 + s.max", text = "x * {value} + {k}" }

[[nodes]]
id = "scaled"
op = "map.calc"
inputs = { x = "$land" }
params = { expression = { from = "v.text" } }
"#,
        Value::Text("unused".into()),
    );
    let out = out.unwrap();
    // k / 2 + the largest value (2): 4; the expression x * 4 + 4
    assert!(matches!(out.get("v.value"), Some(Value::Number(x)) if *x == 4.0));
    assert!(matches!(out.get("v.text"), Some(Value::Text(t)) if t == "x * 4 + 4"));
    assert_eq!(data(out.get("scaled.map")), [8.0, 12.0]);
}
