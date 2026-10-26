//! Map algebra's scripts in a model: their outputs named by `out` (maps, stacks, numbers), read by other nodes.

use std::collections::BTreeMap;

use terraflux_exec::{Engine, Quiet};
use terraflux_graph::{Model, check};
use terraflux_ops::{Registry, Value};

const MODEL: &str = r#"format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random"
params = { rows = 20, columns = 30, seed = 3 }

[[nodes]]
id = "twice"
op = "raster.calc"
inputs = { a = "land.map" }
params = { expression = "a * 2" }

[[nodes]]
id = "s"
op = "stack.of"
inputs = { low = "land.map", high = "twice.map" }

[[nodes]]
id = "calc"
op = "raster.calc"
inputs = { s = "s.stack", land = "land.map" }
params = { expression = """
# the mean of the two layers, each layer over its largest value, and the top tenth of the land
out mean = mean(s)
out scaled = s / s.max()
out p90 = land.percentile(90)
out top = land > p90
""" }

[[nodes]]
id = "after"
op = "raster.calc"
inputs = { m = "calc.mean", k = "calc.p90" }
params = { expression = "m > k" }
"#;

#[test]
fn a_script_gives_maps_stacks_and_numbers() {
    let model = Model::from_toml(MODEL).unwrap();
    let issues = check(&model, &Registry::standard());
    assert!(issues.iter().all(|i| i.severity != terraflux_graph::Severity::Error), "{issues:?}");
    let r = Engine::default().run(&model, &BTreeMap::new(), &mut Quiet).unwrap();
    let Some(Value::Map(land)) = r.get("land.map") else { panic!("no land") };
    let Some(Value::Map(mean)) = r.get("calc.mean") else { panic!("no mean") };
    assert!(land.data.iter().zip(mean.data.iter()).all(|(l, m)| (l * 1.5 - m).abs() < 1e-12));
    let Some(Value::Stack(scaled)) = r.get("calc.scaled") else { panic!("no stack") };
    assert_eq!(scaled.keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(), ["high", "low"]);
    let largest = scaled.layers.iter().flat_map(|l| l.iter()).fold(f64::MIN, |a, b| a.max(*b));
    assert_eq!(largest, 1.0);
    let Some(Value::Number(p90)) = r.get("calc.p90") else { panic!("no number") };
    let Some(Value::Map(top)) = r.get("calc.top") else { panic!("no top") };
    let share = top.data.iter().sum::<f64>() / top.data.len() as f64;
    assert!((share - 0.1).abs() < 0.01, "{share}");
    assert!(*p90 > 0.0 && *p90 < 1.0);
    let Some(Value::Map(after)) = r.get("after.map") else { panic!("no after") };
    assert_eq!(after.data.len(), 600);
}

#[test]
fn a_stack_without_out_is_refused() {
    let model = Model::from_toml(&MODEL.replace("params = { expression = \"m > k\" }", "params = { expression = \"m > k\" }\n\n[[nodes]]\nid = \"bad\"\nop = \"raster.calc\"\ninputs = { s = \"s.stack\" }\nparams = { expression = \"s * 2\" }")).unwrap();
    let e = Engine::default().run(&model, &BTreeMap::new(), &mut Quiet).err().unwrap().to_string();
    assert!(e.contains("give it a name with out"), "{e}");
}
