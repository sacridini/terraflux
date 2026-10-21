//! `control.each`: a body run once for each element of a collection (the layers of a stack, the rows of a table, the
//! features of vectors, the values of a list), `@item`, `@index` and `@name` inside, what it collects gathered: maps a
//! stack of the elements' names, tables one table with a column `item`, the rest a series.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_exec::{Engine, Observer, Results};
use terraflux_graph::Model;
use terraflux_ops::{Key, Map, Stack, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

struct Quiet;
impl Observer for Quiet {}

fn run(text: &str, inputs: Vec<(&str, Value)>) -> Result<Results, String> {
    let m = Model::from_toml(&format!("format = \"terraflux/1\"\n{text}")).unwrap();
    let mut given: BTreeMap<String, Value> = inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    for (name, i) in &m.inputs {
        if let terraflux_graph::Input::Value { value, .. } = i {
            given.entry(name.clone()).or_insert_with(|| Value::from_json(value));
        }
    }
    Engine::default().run(&m, &given, &mut Quiet).map_err(|e| e.to_string())
}

fn years() -> Value {
    let g = Arc::new(Grid::new(2, 1));
    Value::Stack(Stack {
        grid: g,
        keys: ["forest_2000", "forest_2010", "forest_2020"].iter().map(|k| Key::Name(k.to_string())).collect(),
        layers: vec![Arc::new(vec![1.0, 2.0]), Arc::new(vec![3.0, 4.0]), Arc::new(vec![5.0, 6.0])],
    })
}

const STACK_INPUT: &str = r#"
[inputs.years]
kind = "stack"
path = "years.tif"
"#;

#[test]
fn a_body_runs_for_each_layer_and_its_maps_are_a_stack() {
    let out = run(
        &format!(
            r#"{STACK_INPUT}
[[nodes]]
id = "each"
op = "control.each"
inputs = {{ over = "$years", plus = 10 }}
[nodes.body]
collect = {{ maps = "calc.map", stats = "stats.table", names = "@name" }}
[[nodes.body.nodes]]
id = "calc"
op = "map.calc"
inputs = {{ x = "@item", k = "@index", p = "@plus" }}
params = {{ expression = "x * k + p" }}
[[nodes.body.nodes]]
id = "stats"
op = "raster.stats"
inputs = {{ map = "calc.map" }}
"#
        ),
        vec![("years", years())],
    )
    .unwrap();
    let Some(Value::Stack(s)) = out.get("each.maps") else { panic!("{:?}", out.get("each.maps").map(Value::ty)) };
    assert_eq!(s.keys, ["forest_2000", "forest_2010", "forest_2020"].map(|k| Key::Name(k.into())));
    assert_eq!(*s.layers[0], [11.0, 12.0]);
    assert_eq!(*s.layers[2], [25.0, 28.0], "x x 3 + 10");
    let Some(Value::Table(t)) = out.get("each.stats") else { panic!() };
    assert_eq!(t.columns[0], "item");
    assert_eq!(t.column("item").unwrap(), [2000.0, 2010.0, 2020.0], "the year of each layer's name");
    let Some(Value::Series(names)) = out.get("each.names") else { panic!() };
    assert!(matches!(&names[1], Value::Text(t) if t == "forest_2010"));
}

#[test]
fn rows_features_and_values_are_elements_too() {
    // the rows of a table, each a table of one row named by its first column
    let table = Value::Table(Table {
        columns: vec!["code".into(), "area".into()],
        rows: vec![vec![31.0, 2.5], vec![35.0, 4.0]],
    });
    let features = Value::Vector(Arc::new(Vector {
        geometries: vec![Geometry::Point([0.0, 0.0]), Geometry::Point([1.0, 1.0])],
        attributes: vec![("name".into(), Column::Texts(vec!["north".into(), "south".into()]))],
        crs: None,
    }));
    let out = run(
        r#"
[inputs.table]
kind = "value"
value = "a table, given when it runs"

[inputs.places]
kind = "value"
value = "vectors, given when it runs"

[[nodes]]
id = "rows"
op = "control.each"
inputs = { over = "$table" }
[nodes.body]
collect = { rows = "@item", names = "@name" }

[[nodes]]
id = "places"
op = "control.each"
inputs = { over = "$places" }
[nodes.body]
collect = { one = "@item", names = "@name" }

[[nodes]]
id = "species"
op = "control.each"
inputs = { over = ["Ara macao", "Tapirus terrestris"] }
[nodes.body]
collect = { names = "@item", at = "@index" }
"#,
        vec![("table", table), ("places", features)],
    );
    let out = out.unwrap();
    let Some(Value::Table(t)) = out.get("rows.rows") else { panic!() };
    assert_eq!(t.columns, ["item", "code", "area"]);
    assert_eq!(t.rows, [vec![31.0, 31.0, 2.5], vec![35.0, 35.0, 4.0]]);
    let Some(Value::Series(n)) = out.get("places.names") else { panic!() };
    assert!(matches!((&n[0], &n[1]), (Value::Text(a), Value::Text(b)) if a == "north" && b == "south"));
    let Some(Value::Series(one)) = out.get("places.one") else { panic!() };
    assert!(matches!(&one[1], Value::Vector(v) if v.len() == 1));
    let Some(Value::Series(sp)) = out.get("species.names") else { panic!() };
    assert!(matches!(&sp[1], Value::Text(t) if t == "Tapirus terrestris"));
    let Some(Value::Series(at)) = out.get("species.at") else { panic!() };
    assert!(matches!((&at[0], &at[1]), (Value::Integer(1), Value::Integer(2))));
}

#[test]
fn what_each_cannot_do_is_refused() {
    // nothing carried, no until
    let e = run(
        &format!(
            r#"{STACK_INPUT}
[[nodes]]
id = "each"
op = "control.each"
inputs = {{ over = "$years", total = 0 }}
[nodes.body]
carry = {{ total = "@item" }}
"#
        ),
        vec![("years", years())],
    )
    .err()
    .unwrap();
    assert!(e.contains("carries nothing"), "{e}");
    // not a collection
    let e = run(
        r#"
[[nodes]]
id = "each"
op = "control.each"
inputs = { over = 3 }
[nodes.body]
collect = { x = "@item" }
"#,
        vec![],
    )
    .err()
    .unwrap();
    assert!(e.contains("goes over a stack"), "{e}");
    // an @name the loop does not have
    let e = run(
        &format!(
            r#"{STACK_INPUT}
[[nodes]]
id = "each"
op = "control.each"
inputs = {{ over = "$years" }}
[nodes.body]
collect = {{ x = "@step" }}
"#
        ),
        vec![("years", years())],
    )
    .err()
    .unwrap();
    assert!(e.contains("@step is not an input"), "{e}");
}

#[test]
fn a_maps_stack_feeds_what_takes_a_stack() {
    // the gathered maps are a stack to the checker too (a series would not fit a port of a stack)
    let model = |collect: &str| {
        Model::from_toml(&format!(
            r#"format = "terraflux/1"
{STACK_INPUT}
[inputs.land]
kind = "map"
path = "land.tif"

[[nodes]]
id = "each"
op = "control.each"
inputs = {{ over = "$years" }}
[nodes.body]
collect = {{ out = "{collect}" }}

[[nodes]]
id = "after"
op = "stack.set"
inputs = {{ stack = "each.out", map = "$land" }}
params = {{ name = "extra" }}
"#
        ))
        .unwrap()
    };
    let errors = |m: &Model| -> Vec<String> {
        terraflux_graph::check(m, &Engine::default().registry)
            .into_iter()
            .filter(|i| i.severity == terraflux_graph::Severity::Error)
            .map(|i| i.to_string())
            .collect()
    };
    assert!(errors(&model("@item")).is_empty(), "{:?}", errors(&model("@item")));
    assert!(errors(&model("@name")).iter().any(|e| e.contains("gives a Series")), "{:?}", errors(&model("@name")));
    let land = Value::Map(Map::new(Arc::new(Grid::new(2, 1)), vec![0.0, 0.0]));
    let m = model("@item");
    let mut given = BTreeMap::from([("years".to_string(), years()), ("land".to_string(), land)]);
    given.retain(|_, _| true);
    let out = Engine::default().run(&m, &given, &mut Quiet).unwrap();
    let Some(Value::Stack(s)) = out.get("after.stack") else { panic!() };
    assert_eq!(s.layers.len(), 4);
}
