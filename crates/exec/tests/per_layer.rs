//! Many layers where a node takes one map (a stack's, the bands of rasters on disk, a loop's series of maps): the
//! node runs once per layer, its maps a stack of the layers' names, its tables one with a column `layer`, its other
//! values a series; raster.write with {name} writes a file per layer. Where a port takes one map alone (an operator
//! taking its maps together), many bands are told, not dropped in silence.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_exec::{Engine, Observer};
use terraflux_graph::{Access, Body, Model, Node, OpSignature, PortSpec, Type};
use terraflux_ops::{Args, Ctx, Operator, Outputs, Registry, Table, Value};

struct Quiet;
impl Observer for Quiet {}

/// The warnings told, by node.
#[derive(Default)]
struct Told(Vec<(String, String)>);

impl Observer for Told {
    fn warning(&mut self, at: &str, message: &str) {
        self.0.push((at.to_string(), message.to_string()));
    }
}

/// `test.share`: the share of a map's cells above 1 (a number), the same as a table, and the map plus a second
/// map when one is given; a warning for maps of no cell above 1.
struct Share;

impl Operator for Share {
    fn signature(&self) -> OpSignature {
        OpSignature::new("test.share", "", Access::Global)
            .input(PortSpec::new("map", Type::Map, ""))
            .input(PortSpec::optional("plus", Type::Map, ""))
            .output(PortSpec::new("share", Type::Number, ""))
            .output(PortSpec::new("table", Type::Table, ""))
            .output(PortSpec::new("map", Type::Map, ""))
    }

    fn run(&self, ctx: &mut Ctx, a: &Args) -> terraflux_ops::Result<Outputs> {
        let m = a.map("map")?;
        let share = m.data.iter().filter(|v| **v > 1.0).count() as f64 / m.data.len() as f64;
        if share == 0.0 {
            ctx.warn("no cell above 1");
        }
        let plus = a.opt_map("plus")?;
        let data = m.data.iter().enumerate().map(|(i, v)| v + plus.map_or(0.0, |p| p.data[i])).collect();
        Ok([
            ("share".to_string(), Value::Number(share)),
            ("table".to_string(), Value::Table(Table { columns: vec!["share".into()], rows: vec![vec![share]] })),
            ("map".to_string(), Value::Map(terraflux_ops::Map::new(m.grid.clone(), data))),
        ]
        .into())
    }
}

fn engine() -> Engine {
    let mut r = Registry::standard();
    r.add(Arc::new(Share));
    Engine { registry: r }
}

fn land(id: &str, seed: u64) -> Node {
    Node::new(id, "nlm.random_cluster")
        .param("rows", 40)
        .param("columns", 50)
        .param("p", 0.55)
        .param("weights", serde_json::json!([0.4, 0.6]))
        .param("seed", seed)
}

/// Two maps of land as a stack of layers y2010 and y2020.
fn two_years(m: &mut Model) {
    m.nodes.push(land("a", 3));
    m.nodes.push(land("b", 4));
    m.nodes.push(Node::new("years", "stack.of").input("y2010", "a.map").input("y2020", "b.map"));
}

fn stack(v: &Value) -> &terraflux_ops::Stack {
    let Value::Stack(s) = v else { panic!("not a stack: {v:?}") };
    s
}

fn map(v: &Value) -> &terraflux_ops::Map {
    let Value::Map(m) = v else { panic!("not a map: {v:?}") };
    m
}

/// MSPA of a stack: a stack of each layer's MSPA, as MSPA of each map gives it.
#[test]
fn a_stack_is_run_once_per_layer() {
    let mut m = Model::new("t");
    two_years(&mut m);
    m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "years.stack").param("foreground", 2));
    m.nodes.push(Node::new("one", "landscape.mspa").input("map", "b.map").param("foreground", 2));
    let issues = terraflux_graph::check(&m, &engine().registry);
    assert!(issues.is_empty(), "{issues:?}");
    let r = engine().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
    let s = stack(r.get("mspa.map").unwrap());
    let names: Vec<String> = s.keys.iter().map(|k| k.to_string()).collect();
    assert_eq!(names, ["y2010", "y2020"]);
    assert_eq!(*s.layers[1], *map(r.get("one.map").unwrap()).data);
}

/// Numbers per layer a series, tables one table whose column `layer` is the year each layer's name ends with,
/// a warning once with the layers that gave it; a map given alone taken by every layer.
#[test]
fn outputs_are_gathered_by_layer() {
    let mut m = Model::new("t");
    two_years(&mut m);
    m.nodes.push(Node::new("none", "raster.reclassify").input("map", "a.map").param("rules", "* -> 0"));
    m.nodes.push(Node::new("mixed", "stack.of").input("y2010", "a.map").input("y2020", "none.map"));
    m.nodes.push(Node::new("s", "test.share").input("map", "mixed.stack").input("plus", "none.map"));
    let mut told = Told::default();
    let r = engine().run(&m, &BTreeMap::new(), &mut told).unwrap();
    let Value::Series(shares) = r.get("s.share").unwrap() else { panic!() };
    assert_eq!(shares.len(), 2);
    assert_eq!(shares[1].as_number(), Some(0.0));
    let Value::Table(t) = r.get("s.table").unwrap() else { panic!() };
    assert_eq!(t.columns, ["layer", "share"]);
    assert_eq!(t.column("layer").unwrap(), [2010.0, 2020.0]);
    assert_eq!(stack(r.get("s.map").unwrap()).layers.len(), 2);
    assert_eq!(told.0, [("s".to_string(), "no cell above 1 (layer y2020)".to_string())]);
}

/// Two stacks are paired layer by layer; stacks of different lengths are refused, saying so.
#[test]
fn stacks_are_paired() {
    let mut m = Model::new("t");
    two_years(&mut m);
    m.nodes.push(Node::new("s", "test.share").input("map", "years.stack").input("plus", "years.stack"));
    let r = engine().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
    let s = stack(r.get("s.map").unwrap());
    let a = map(r.get("a.map").unwrap());
    assert!(s.layers[0].iter().zip(a.data.iter()).all(|(x, y)| *x == 2.0 * y));
    m.nodes.push(Node::new("one", "stack.of").input("y2010", "a.map"));
    m.nodes[3] = Node::new("s", "test.share").input("map", "years.stack").input("plus", "one.stack");
    let e = engine().run(&m, &BTreeMap::new(), &mut Quiet).err().unwrap().to_string();
    assert!(e.contains("map has 2, plus has 1"), "{e}");
}

/// A loop's series of maps: a series again (its steps).
#[test]
fn a_series_stays_a_series() {
    let mut m = Model::new("t");
    m.nodes.push(land("a", 3));
    m.nodes.push(Node::new("loop", "control.repeat").literal("times", 3).body(Body {
        nodes: vec![Node::new("c", "raster.calc").input("x", "a.map").input("k", "@step").param("expression", "x * k")],
        collect: [("maps".to_string(), "c.map".to_string())].into(),
        ..Default::default()
    }));
    m.nodes.push(Node::new("s", "test.share").input("map", "loop.maps"));
    let r = engine().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
    let Value::Series(maps) = r.get("s.map").unwrap() else { panic!() };
    assert_eq!(maps.len(), 3);
}

/// The bands of a file: MSPA of each, written a file per band with {name}, each tagged MSPA; a port taking one
/// map alone (stack.of's) told that it takes the first band only.
#[test]
fn bands_on_disk_run_once_each_and_are_written_a_file_each() {
    let dir = std::env::temp_dir().join(format!("terraflux-per-layer-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let both = dir.join("years.tif");
    let mut m = Model::new("t");
    two_years(&mut m);
    m.nodes.push(Node::new("w", "raster.write").input("value", "years.stack").param("path", both.to_string_lossy()));
    engine().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();

    let mut m = Model::new("t");
    m.nodes.push(Node::new("read", "raster.read").param("path", both.to_string_lossy()));
    m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "read.raster").param("foreground", 2));
    let each = dir.join("out").join("{name}_mspa.tif");
    m.nodes.push(Node::new("w", "raster.write").input("value", "mspa.map").param("path", each.to_string_lossy()));
    m.nodes.push(Node::new("alone", "stack.of").input("first", "read.raster"));
    let mut told = Told::default();
    let r = engine().run(&m, &BTreeMap::new(), &mut told).unwrap();
    let Value::Series(files) = r.get("w.path").unwrap() else { panic!() };
    assert_eq!(files.len(), 2);
    for year in ["y2010", "y2020"] {
        let f = terraflux_raster::Raster::read(dir.join("out").join(format!("{year}_mspa.tif"))).unwrap();
        assert_eq!(f.grid.geo.item("landschaft_product"), Some("MSPA"));
    }
    assert_eq!(stack(r.get("alone.stack").unwrap()).layers.len(), 1);
    assert!(told.0.iter().any(|(at, w)| at == "alone" && w.contains("only \"y2010\" is used")), "{:?}", told.0);
    std::fs::remove_dir_all(&dir).ok();
}
