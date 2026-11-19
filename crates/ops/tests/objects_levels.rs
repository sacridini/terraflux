//! The levels and operations on objects (`objects.merge`, `objects.grow`, `objects.remove_small`,
//! `objects.reclassify`) on small objects made by hand, against the results worked out by hand.

use std::sync::Arc;

use terraflux_ops::objects::{ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;

/// Objects on a grid of `w` x `h` from their labels, with the table id, cells, area and `attr` (`values` per object),
/// and the classes `names` (`class_of` per object, 0 none).
fn objs(w: usize, h: usize, labels: Vec<u32>, attr: &str, values: &[f64], names: &[&str], class_of: Vec<u32>) -> Value {
    let count = values.len();
    let o = Objects::from_labels(Arc::new(Grid::new(w, h)), labels, count).unwrap();
    let area = o.cell_area();
    let mut t = Table::new(["id", "cells", "area", attr].map(String::from).to_vec());
    t.rows = (0..count).map(|k| vec![(k + 1) as f64, o.cells[k] as f64, o.cells[k] as f64 * area, values[k]]).collect();
    let classes = names.iter().map(|n| ObjectClass { name: n.to_string(), colour: None }).collect();
    let o = o.with_table(t).unwrap().with_classes(classes, class_of, Vec::new()).unwrap();
    Value::Objects(Arc::new(o))
}

/// A cell an object: ids 1..=w*h row by row.
fn cells_as_objects(w: usize, h: usize, attr: &str, values: &[f64], names: &[&str], class_of: Vec<u32>) -> Value {
    objs(w, h, (1..=(w * h) as u32).collect(), attr, values, names, class_of)
}

fn run(op: &str, o: Value, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: [("objects".to_string(), o)].into_iter().collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn objects_of(out: &Outputs, port: &str) -> Arc<Objects> {
    let Value::Objects(o) = &out[port] else { panic!("{port} is not objects") };
    o.clone()
}

fn count_of(out: &Outputs) -> i64 {
    let Value::Integer(n) = &out["count"] else { panic!("count is not an integer") };
    *n
}

fn done_of(out: &Outputs) -> bool {
    let Value::Bool(b) = &out["done"] else { panic!("done is not a bool") };
    *b
}

fn column(o: &Objects, name: &str) -> Vec<f64> {
    o.table.column(name).unwrap()
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

/// 3 x 3 cells, each an object, of the classes a (1) and b (2):
///   a a b
///   a b b
///   b b a
fn nine() -> Value {
    let values: Vec<f64> = (0..9).map(|i| i as f64).collect();
    cells_as_objects(3, 3, "mean_red", &values, &["a", "b"], vec![1, 1, 2, 1, 2, 2, 2, 2, 1])
}

#[test]
fn merge_by_class() {
    let out = run("objects.merge", nine(), serde_json::json!({})).unwrap();
    let o = objects_of(&out, "objects");
    assert_eq!(o.count, 3);
    assert_eq!(count_of(&out), 3);
    assert!(!done_of(&out));
    assert_eq!(*o.labels, vec![1, 1, 2, 1, 2, 2, 2, 2, 3]);
    assert_eq!(o.cells, vec![3, 5, 1]);
    assert_eq!(o.class_of, vec![1, 2, 1]);
    assert_eq!(column(&o, "id"), vec![1.0, 2.0, 3.0]);
    assert_eq!(column(&o, "cells"), vec![3.0, 5.0, 1.0]);
    assert!(close(&column(&o, "area"), &[3.0, 5.0, 1.0]));
    // the cells' weighted mean: (0 + 1 + 3) / 3, (2 + 4 + 5 + 6 + 7) / 5, 8
    assert!(close(&column(&o, "mean_red"), &[4.0 / 3.0, 4.8, 8.0]));
    // each object is connected: as many objects as connected areas of a label
    let region: Vec<u64> = o.labels.iter().map(|&l| l as u64).collect();
    assert_eq!(Objects::from_regions(o.grid.clone(), &region, 4).count, 3);
    // nothing left to merge
    let again = run("objects.merge", Value::Objects(o), serde_json::json!({})).unwrap();
    assert!(done_of(&again));
}

#[test]
fn merge_only_some_classes() {
    let out = run("objects.merge", nine(), serde_json::json!({ "classes": ["a"] })).unwrap();
    let o = objects_of(&out, "objects");
    // the b cells stay apart: a {0,1,3}, b 2, b 4, b 5, b 6, b 7, a 8 -> 7 objects
    assert_eq!(o.count, 7);
    assert!(run("objects.merge", nine(), serde_json::json!({ "classes": ["zzz"] })).is_err());
}

#[test]
fn merge_into_a_level() {
    let out = run("objects.merge", nine(), serde_json::json!({ "level": true })).unwrap();
    let merged = objects_of(&out, "objects");
    let below = objects_of(&out, "below");
    assert_eq!(merged.count, 3);
    assert_eq!(below.count, 9);
    // each below object has one parent, the object that contains it
    assert_eq!(below.parent_of, vec![1, 1, 2, 1, 2, 2, 2, 2, 3]);
    for (k, &p) in below.parent_of.iter().enumerate() {
        assert_eq!(merged.labels[k], p);
        assert_eq!(below.parent_id(k as u32 + 1), Some(p));
    }
    // the level above is the merged objects themselves
    assert_eq!(*below.parent.as_ref().unwrap().labels, *merged.labels);
}

#[test]
fn merge_by_condition_is_transitive() {
    // six objects in a row; a pair merges when the means differ by less than 0.1
    let values = [0.10, 0.15, 0.20, 0.50, 0.55, 0.95];
    let o = cells_as_objects(6, 1, "mean_red", &values, &[], vec![0; 6]);
    let out =
        run("objects.merge", o, serde_json::json!({ "condition": "abs(a.mean_red - b.mean_red) < 0.1" })).unwrap();
    let m = objects_of(&out, "objects");
    assert_eq!(*m.labels, vec![1, 1, 1, 2, 2, 3]);
    assert_eq!(m.cells, vec![3, 2, 1]);
    assert!(close(&column(&m, "mean_red"), &[0.15, 0.525, 0.95]));
}

#[test]
fn merge_by_condition_with_border() {
    // objects: 1 = cells (0,0),(1,0); 2 = (0,1); 3 = (1,1). 1-2 and 1-3 share one edge, 2-3 share one edge.
    let o = objs(2, 2, vec![1, 2, 1, 3], "mean_red", &[1.0, 2.0, 3.0], &[], vec![0; 3]);
    let out = run("objects.merge", o, serde_json::json!({ "condition": "border >= 2 or b.mean_red == 3" })).unwrap();
    // 1-3 merges (b is 3); 2-3 has b = 3 too (a = 2, b = 3) and merges: all one object
    let m = objects_of(&out, "objects");
    assert_eq!(m.count, 1);
    let o = objs(2, 2, vec![1, 2, 1, 3], "mean_red", &[1.0, 2.0, 3.0], &[], vec![0; 3]);
    let out = run("objects.merge", o, serde_json::json!({ "condition": "border >= 2" })).unwrap();
    assert_eq!(objects_of(&out, "objects").count, 3);
    assert!(done_of(&out));
}

#[test]
fn grow_along_a_chain() {
    // a road then candidates 0.3, 0.4, 0.1 (stops), 0.5 and 0.5
    let nir = [0.9, 0.3, 0.4, 0.1, 0.5, 0.5];
    let o = cells_as_objects(6, 1, "mean_nir", &nir, &["road"], vec![1, 0, 0, 0, 0, 0]);
    let cond = "mean_nir > 0.2 and cls == \"unclassified\"";
    let out = run("objects.grow", o.clone(), serde_json::json!({ "class": "road", "condition": cond })).unwrap();
    let g = objects_of(&out, "objects");
    assert_eq!(*g.labels, vec![1, 1, 1, 2, 3, 4]);
    assert_eq!(g.class_of, vec![1, 0, 0, 0]);
    assert_eq!(g.cells[0], 3);
    assert_eq!(count_of(&out), 4);
    assert!(!done_of(&out));
    // max_steps
    let out =
        run("objects.grow", o, serde_json::json!({ "class": "road", "condition": cond, "max_steps": 1 })).unwrap();
    assert_eq!(*objects_of(&out, "objects").labels, vec![1, 1, 2, 3, 4, 5]);
    // grown again: nothing changes
    let again =
        run("objects.grow", Value::Objects(g), serde_json::json!({ "class": "road", "condition": cond })).unwrap();
    assert!(done_of(&again));
}

/// Two rows of four cells: object 1 (3 cells), 2 (4 cells) and 3 (1 cell, at (1, 1)).
///   1 1 2 2
///   1 3 2 2
fn small_one(class_of: Vec<u32>) -> Value {
    objs(4, 2, vec![1, 1, 2, 2, 1, 3, 2, 2], "mean_red", &[0.2, 0.8, 0.7], &["x", "y"], class_of)
}

#[test]
fn remove_small_into_the_most_similar() {
    let out = run("objects.remove_small", small_one(vec![1, 1, 1]), serde_json::json!({ "min_cells": 2 })).unwrap();
    let o = objects_of(&out, "objects");
    // the mean 0.7 is nearer 0.8 (object 2) than 0.2
    assert_eq!(*o.labels, vec![1, 1, 2, 2, 1, 2, 2, 2]);
    assert_eq!(o.cells, vec![3, 5]);
    assert_eq!(count_of(&out), 2);
}

#[test]
fn remove_small_into_the_longest_border() {
    let out =
        run("objects.remove_small", small_one(vec![1, 1, 1]), serde_json::json!({ "min_cells": 2, "into": "border" }))
            .unwrap();
    let o = objects_of(&out, "objects");
    // two edges with object 1, one with object 2
    assert_eq!(*o.labels, vec![1, 1, 2, 2, 1, 1, 2, 2]);
    assert_eq!(o.cells, vec![4, 4]);
    // by area: the cells are 1 x 1, so an area of 2 is as 2 cells
    let out = run("objects.remove_small", small_one(vec![1, 1, 1]), serde_json::json!({ "min_area": 2.0 })).unwrap();
    assert_eq!(objects_of(&out, "objects").count, 2);
}

#[test]
fn remove_small_until_none_is_small() {
    // 1 (4 cells, 0.0), 2 (1 cell, 0.5), 3 (1 cell, 0.5), 4 (4 cells, 1.2): the two small go to each other, are still
    // small (2 cells < 3) and then go to object 1
    let labels = vec![1, 1, 1, 1, 2, 3, 4, 4, 4, 4];
    let o = objs(10, 1, labels, "mean_red", &[0.0, 0.5, 0.5, 1.2], &[], vec![0; 4]);
    let out = run("objects.remove_small", o, serde_json::json!({ "min_cells": 3 })).unwrap();
    let m = objects_of(&out, "objects");
    assert_eq!(m.count, 2);
    assert_eq!(m.cells, vec![6, 4]);
    assert!(m.cells.iter().all(|&c| c >= 3));
}

#[test]
fn remove_small_respects_the_classes() {
    // the small object 3 is of class y: with classes = [x] it stays
    let out =
        run("objects.remove_small", small_one(vec![1, 1, 2]), serde_json::json!({ "min_cells": 2, "classes": ["x"] }))
            .unwrap();
    assert_eq!(objects_of(&out, "objects").count, 3);
    assert!(done_of(&out));
    let out =
        run("objects.remove_small", small_one(vec![1, 1, 2]), serde_json::json!({ "min_cells": 2, "classes": ["y"] }))
            .unwrap();
    let o = objects_of(&out, "objects");
    assert_eq!(o.count, 2);
    // the merged object keeps the class of the larger part
    assert_eq!(o.class_of, vec![1, 1]);
}

fn strip() -> Value {
    cells_as_objects(4, 1, "mean_red", &[1.0, 2.0, 3.0, 4.0], &["marsh", "water", "forest"], vec![1, 2, 3, 2])
}

#[test]
fn reclassify_joins_names_and_colours() {
    let params = serde_json::json!({ "classes": { "marsh": "wet", "water": "wet" }, "colours": { "wet": "#0000ff" } });
    let out = run("objects.reclassify", strip(), params.clone()).unwrap();
    let o = objects_of(&out, "objects");
    let names: Vec<&str> = o.classes.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["wet", "forest"]);
    assert_eq!(o.classes[0].colour, Some([0, 0, 255]));
    assert_eq!(o.class_of, vec![1, 1, 2, 1]);
    // neighbours that share a class stay apart without merge
    assert_eq!(o.count, 4);
    assert!(done_of(&out));
    let mut with = params;
    with["merge"] = serde_json::json!(true);
    let out = run("objects.reclassify", strip(), with).unwrap();
    let o = objects_of(&out, "objects");
    assert_eq!(*o.labels, vec![1, 1, 2, 3]);
    assert_eq!(o.class_of, vec![1, 2, 1]);
    assert_eq!(count_of(&out), 3);
    assert!(run("objects.reclassify", strip(), serde_json::json!({ "classes": { "nope": "x" } })).is_err());
}
