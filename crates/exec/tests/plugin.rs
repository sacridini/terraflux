//! A WebAssembly plugin of the `map` interface in a model: two outputs, read by the nodes after it.

use std::collections::BTreeMap;

use terraflux_exec::{Engine, Quiet};
use terraflux_graph::{Model, Severity, check};
use terraflux_ops::{Registry, Value};

/// `map` gives the sum of its two inputs in the first output and their difference in the second.
const MODULE: &str = r#"(module
  (memory (export "memory") 40)
  (global $top (mut i32) (i32.const 8))
  (func (export "alloc") (param $n i32) (result i32) (local $p i32)
    (local.set $p (global.get $top))
    (global.set $top (i32.add (local.get $p) (local.get $n)))
    (local.get $p))
  (func $at (param $base i32) (param $i i32) (result i32)
    (i32.add (local.get $base) (i32.shl (local.get $i) (i32.const 3))))
  (func (export "map") (param $rows i32) (param $cols i32) (param $nin i32) (param $in i32)
        (param $nout i32) (param $out i32) (result i32)
    (local $i i32) (local $n i32) (local $a i32) (local $b i32) (local $s i32) (local $d i32)
    (local.set $a (i32.load (local.get $in)))
    (local.set $b (i32.load offset=4 (local.get $in)))
    (local.set $s (i32.load (local.get $out)))
    (local.set $d (i32.load offset=4 (local.get $out)))
    (local.set $n (i32.mul (local.get $rows) (local.get $cols)))
    (block $done (loop $next
      (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
      (f64.store (call $at (local.get $s) (local.get $i))
        (f64.add (f64.load (call $at (local.get $a) (local.get $i))) (f64.load (call $at (local.get $b) (local.get $i)))))
      (f64.store (call $at (local.get $d) (local.get $i))
        (f64.sub (f64.load (call $at (local.get $a) (local.get $i))) (f64.load (call $at (local.get $b) (local.get $i)))))
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br $next)))
    (i32.const 0)))"#;

fn model(params: &str) -> Model {
    let path = std::env::temp_dir().join(format!("terraflux-exec-plugin-{}.wat", std::process::id()));
    std::fs::write(&path, MODULE).unwrap();
    let path = path.to_string_lossy().replace('\\', "/");
    Model::from_toml(&format!(
        r#"format = "terraflux/1"

[[nodes]]
id = "land"
op = "nlm.random"
params = {{ rows = 20, columns = 30, seed = 3 }}

[[nodes]]
id = "twice"
op = "raster.calc"
inputs = {{ a = "land.map" }}
params = {{ expression = "a * 2" }}

[[nodes]]
id = "p"
op = "plugin.wasm"
inputs = {{ a = "land.map", b = "twice.map" }}
params = {{ path = "{path}", args = ["a", "b"]{params} }}

[[nodes]]
id = "after"
op = "raster.calc"
inputs = {{ s = "p.sum", d = "p.diff" }}
params = {{ expression = "s + d" }}
"#
    ))
    .unwrap()
}

#[test]
fn a_map_plugin_gives_two_outputs_to_the_nodes_after_it() {
    let m = model(r#", interface = "map", outputs = ["sum", "diff"]"#);
    let issues = check(&m, &Registry::standard());
    assert!(issues.iter().all(|i| i.severity != Severity::Error), "{issues:?}");
    let r = Engine::default().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
    let get = |k: &str| match r.get(k) {
        Some(Value::Map(m)) => m.data.to_vec(),
        _ => panic!("no map {k}"),
    };
    let (land, sum, diff, after) = (get("land.map"), get("p.sum"), get("p.diff"), get("after.map"));
    assert_eq!(land.len(), 600);
    for i in 0..land.len() {
        assert_eq!(sum[i], land[i] + land[i] * 2.0);
        assert_eq!(diff[i], land[i] - land[i] * 2.0);
        assert!((after[i] - land[i] * 2.0).abs() < 1e-12);
    }
}

#[test]
fn the_check_tells_a_plugin_misconfigured_before_it_runs() {
    let errors = |params: &str| {
        check(&model(params), &Registry::standard())
            .into_iter()
            .filter(|i| i.severity == Severity::Error)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
    };
    // (the nodes after it read p.sum and p.diff: they are there only when the plugin lists them)
    assert!(errors(r#", interface = "map", outputs = ["sum", "diff"]"#).is_empty());
    let e = errors(r#", interface = "tiles", outputs = ["sum", "diff"]"#);
    assert!(e.iter().any(|x| x.contains("p.interface")), "{e:?}");
    let e = errors(r#", interface = "window", radius = 0"#);
    assert!(e.iter().any(|x| x.contains("p.radius")), "{e:?}");
    let e = errors(r#", interface = "map", outputs = ["sum", "sum"]"#);
    assert!(e.iter().any(|x| x.contains("two outputs are named")), "{e:?}");
    // without `outputs` the plugin gives `map`, and the nodes after it read what is not there
    let e = errors("");
    assert!(e.iter().any(|x| x.contains("no output \"sum\"")), "{e:?}");
}
