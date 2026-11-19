//! The calculators over features (`vector.calc`, `vector.filter`) written and explained by the assistants' tools
//! (write_calc, explain_calc): checked, and tried on the features, before anything changes.

use std::path::Path;
use std::sync::Arc;

use serde_json::json;
use terraflux_app::tools::{explain_calc, write_calc};
use terraflux_graph::{Model, Ref};
use terraflux_ops::{Registry, Value};
use terraflux_raster::vector::{Column, Geometry, Vector};

fn model() -> Model {
    Model::from_toml(
        r#"
format = "terraflux/1"

[[nodes]]
id = "parcels"
op = "vector.read"
params = { path = "parcels.gpkg" }
"#,
    )
    .unwrap()
}

/// Three squares of sides 1, 2 and 3, of a land use (one without) and people.
fn parcels() -> Value {
    let square = |x: f64, s: f64| Geometry::Polygon(vec![vec![[x, 0.0], [x + s, 0.0], [x + s, s], [x, s], [x, 0.0]]]);
    Value::Vector(Arc::new(Vector {
        geometries: vec![square(0.0, 1.0), square(10.0, 2.0), square(20.0, 3.0)],
        attributes: vec![
            ("uso".into(), Column::Texts(vec!["floresta".into(), "pasto".into(), String::new()])),
            ("pop".into(), Column::Numbers(vec![10.0, 20.0, 90.0])),
        ],
        crs: None,
    }))
}

fn known(r: &Ref) -> Option<Value> {
    matches!(r, Ref::Port { node, port } if node == "parcels" && port == "vector").then(parcels)
}

#[test]
fn a_script_over_features_is_tried_on_them() {
    let (r, dir) = (Registry::standard(), Path::new("."));
    let mut m = model();
    let script = "# the people per unit of area, and a class of land use\n\
                  density = pop / geometry.area\n\
                  out density\n\
                  out cover = \"natural\" if uso == \"floresta\" else \"other\"\n\
                  cover[isnull(uso)] = None";
    let a = json!({ "node": "attrs", "op": "vector.calc", "script": script, "inputs": { "vector": "parcels.vector" } });
    let reply = write_calc(&mut m, &r, dir, &a, &known).unwrap();
    assert!(reply.contains("Its attributes, on a sample of every feature (3):"), "{reply}");
    assert!(reply.contains("- density: numbers: classes 5 (33.3 %), 10 (66.7 %)"), "{reply}");
    assert!(
        reply.contains("- cover: texts \"natural\" (50.0 %), \"other\" (50.0 %); 1 of 3 features without a value"),
        "{reply}"
    );
    let n = m.node("attrs").unwrap();
    assert_eq!(n.op, "vector.calc");
    assert!(!n.params.contains_key("column"));

    // explained: its inputs and what each name holds
    let t = explain_calc(&m, dir, &json!({ "node": "attrs" }), &known).unwrap();
    assert!(t.contains("- vector <- parcels.vector: 3 features, their attributes: uso (texts), pop (numbers)"), "{t}");
    assert!(t.contains("- density (an attribute given): numbers:"), "{t}");

    // a filter: what it keeps, a text input compared
    let a = json!({
        "node": "kept",
        "op": "vector.filter",
        "script": "dense = pop / geometry.area > 6\ndense and uso == wanted",
        "inputs": { "vector": "parcels.vector", "wanted": "pasto" },
    });
    let reply = write_calc(&mut m, &r, dir, &a, &known).unwrap();
    assert!(reply.contains("On a sample of every feature (3), it keeps 0 of 3 features (0.0 %)"), "{reply}");
    let t = explain_calc(&m, dir, &json!({ "node": "kept" }), &known).unwrap();
    assert!(t.contains("- wanted <- \"pasto\": the text \"pasto\""), "{t}");
    assert!(t.contains("- the condition: it keeps 0 of 3 features"), "{t}");
    assert!(t.contains("- dense: numbers: classes 0 (33.3 %), 1 (66.7 %)"), "{t}");
}

#[test]
fn a_script_over_features_that_fails_is_not_written() {
    let (r, dir) = (Registry::standard(), Path::new("."));
    let mut m = model();
    let before = m.clone();
    let write = |m: &mut Model, op: &str, script: &str| {
        let a = json!({ "node": "c", "op": op, "script": script, "inputs": { "vector": "parcels.vector" } });
        write_calc(m, &r, dir, &a, &known).unwrap_err()
    };
    let e = write(&mut m, "vector.calc", "out big = uso > 3");
    assert!(e.contains("on a sample of its features it fails: a text compared with a number (uso > a number)"), "{e}");
    let e = write(&mut m, "vector.calc", "pop * 2");
    assert!(e.contains("gives each attribute with out"), "{e}");
    let e = write(&mut m, "vector.filter", "out k = pop > 1");
    assert!(e.contains("ends with the condition"), "{e}");
    let e = write(&mut m, "vector.filter", "uso");
    assert!(e.contains("its condition gives texts"), "{e}");
    let e = write(&mut m, "vector.calc", "out x = usos == 'a'");
    assert!(e.contains("neither an attribute (the features': uso, pop) nor an input"), "{e}");
    let e = write(&mut m, "raster.filter", "x");
    assert!(e.contains("a calculator is one of raster.calc, vector.calc, vector.filter"), "{e}");
    assert_eq!(m.nodes.len(), before.nodes.len(), "the model as it was");

    // the features not known: written, without a preview
    let reply = write_calc(
        &mut m,
        &r,
        dir,
        &json!({ "node": "c", "op": "vector.calc", "script": "out d = pop / geometry.area", "inputs": { "vector": "parcels.vector" } }),
        &|_| None,
    )
    .unwrap();
    assert!(reply.contains("No preview: the features"), "{reply}");
    // an existing calculator keeps its operator
    let e = write_calc(&mut m, &r, dir, &json!({ "node": "c", "op": "vector.filter", "script": "pop > 1" }), &known)
        .unwrap_err();
    assert!(e.contains("c is a vector.calc, not a vector.filter"), "{e}");
}

/// Three objects in a row, A, B and C (mean_red 2, 6, 12), classed water, field and water.
fn tiles() -> Value {
    use terraflux_ops::objects::{ObjectClass, Objects};
    let o = Objects::from_labels(Arc::new(terraflux_raster::Grid::new(3, 1)), vec![1, 2, 3], 3).unwrap();
    let mut t = terraflux_ops::Table::new(vec!["id".into(), "mean_red".into()]);
    t.rows = vec![vec![1.0, 2.0], vec![2.0, 6.0], vec![3.0, 12.0]];
    let classes = ["water", "field"].map(|name| ObjectClass { name: name.into(), colour: None }).to_vec();
    Value::Objects(Arc::new(o.with_table(t).unwrap().with_classes(classes, vec![1, 2, 1], vec![]).unwrap()))
}

fn objects_known(r: &Ref) -> Option<Value> {
    matches!(r, Ref::Port { node, port } if node == "tiles" && port == "objects").then(tiles)
}

fn objects_model() -> Model {
    Model::from_toml(
        r#"
format = "terraflux/1"

[[nodes]]
id = "tiles"
op = "segment.chessboard"
params = { size = 1 }
"#,
    )
    .unwrap()
}

#[test]
fn a_script_over_objects_is_tried_on_them() {
    let (r, dir) = (Registry::standard(), Path::new("."));
    let mut m = objects_model();
    let script = "# the neighbours' red, a membership and a text\n\
                  out around = nb.mean(mean_red)\n\
                  out wet = above(mean_red, 2, 12) and not (cls == \"field\")\n\
                  out kind = \"water\" if cls == \"water\" else \"land\"";
    let a =
        json!({ "node": "rules", "op": "objects.calc", "script": script, "inputs": { "objects": "tiles.objects" } });
    let reply = write_calc(&mut m, &r, dir, &a, &objects_known).unwrap();
    assert!(reply.contains("Its attributes, on a sample of every object (3):"), "{reply}");
    // A 6, B (2 + 12) / 2 = 7, C 6
    assert!(reply.contains("- around: numbers: classes 6 (66.7 %), 7 (33.3 %)"), "{reply}");
    assert!(reply.contains("- wet: numbers:"), "{reply}");
    assert!(reply.contains("- kind: texts \"water\" (66.7 %), \"land\" (33.3 %)"), "{reply}");
    let n = m.node("rules").unwrap();
    assert_eq!(n.op, "objects.calc");
    assert!(!n.params.contains_key("column"));

    // explained: its inputs and what each name holds
    let t = explain_calc(&m, dir, &json!({ "node": "rules" }), &objects_known).unwrap();
    assert!(t.contains("- objects <- tiles.objects: 3 objects"), "{t}");
    assert!(t.contains("- around (an attribute given): numbers:"), "{t}");

    // refused: no out, a level that is not there, a function the others have not; the model as it was
    let before = m.clone();
    let write = |m: &mut Model, script: &str| {
        let a = json!({ "node": "rules", "op": "objects.calc", "script": script, "inputs": { "objects": "tiles.objects" } });
        write_calc(m, &r, dir, &a, &objects_known).unwrap_err()
    };
    let e = write(&mut m, "nb.mean(mean_red)");
    assert!(e.contains("gives each attribute with out"), "{e}");
    let e = write(&mut m, "out c = children.count(cls == \"water\")");
    assert!(e.contains("on a sample of its objects it fails") && e.contains("below"), "{e}");
    let e = write(&mut m, "out c = nb.median(mean_red)");
    assert!(e.contains("no \"nb.median\""), "{e}");
    assert_eq!(m.nodes.len(), before.nodes.len());
    // the fuzzy functions are for objects.calc only
    let a = json!({ "node": "v", "op": "vector.calc", "script": "out m = above(pop, 0, 1)", "inputs": { "vector": "tiles.objects" } });
    let e = write_calc(&mut m, &r, dir, &a, &objects_known).unwrap_err();
    assert!(e.contains("above"), "{e}");
    // the objects not known: written, without a preview
    let a = json!({ "node": "other", "op": "objects.calc", "script": "out d = nb.sum(mean_red)", "inputs": { "objects": "tiles.objects" } });
    let reply = write_calc(&mut m, &r, dir, &a, &|_| None).unwrap();
    assert!(reply.contains("No preview: the objects"), "{reply}");
}
