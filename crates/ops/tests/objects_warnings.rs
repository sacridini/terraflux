//! The warnings of object-based classification: each fires on a crafted case and stays quiet on a clean one.

use std::sync::Arc;

use terraflux_ops::objects::Objects;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::vector::{Column, Geometry, Vector};
use terraflux_raster::{GeoTags, Grid};

/// The operator with the warnings it gave.
fn run(
    op: &str,
    inputs: Vec<(&str, Value)>,
    params: serde_json::Value,
) -> (terraflux_ops::Result<Outputs>, Vec<String>) {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    let mut ctx = Ctx::default();
    let out = r.get(op).unwrap().run(&mut ctx, &args);
    (out, ctx.warnings.clone())
}

fn said(w: &[String], part: &str) -> bool {
    w.iter().any(|m| m.contains(part))
}

/// A grid that knows its coordinate system.
fn georeferenced(w: usize, h: usize) -> Arc<Grid> {
    let mut g = Grid::new(w, h);
    g.geo = GeoTags { geokeys: Some(vec![1, 1, 0, 1, 2048, 0, 1, 4326]), ..Default::default() };
    Arc::new(g)
}

fn stack(grid: Arc<Grid>, layers: Vec<Vec<f64>>) -> Value {
    let keys = (0..layers.len()).map(|k| terraflux_ops::Key::Name(format!("b{}", k + 1))).collect();
    Value::Stack(Stack { grid, keys, layers: layers.into_iter().map(Arc::new).collect() })
}

fn ramp(n: usize, from: f64, to: f64) -> Vec<f64> {
    (0..n).map(|i| from + (to - from) * i as f64 / (n - 1) as f64).collect()
}

fn quadtree(image: Value) -> Vec<String> {
    let (out, w) = run("segment.quadtree", vec![("image", image)], serde_json::json!({ "threshold": 1.0e9 }));
    out.unwrap();
    w
}

#[test]
fn the_image_without_a_crs_is_said() {
    let n = 64;
    let bare = stack(Arc::new(Grid::new(8, 8)), vec![ramp(n, 0.1, 0.9)]);
    let w = quadtree(bare);
    assert!(said(&w, "no coordinate system"), "{w:?}");
    let w = quadtree(stack(georeferenced(8, 8), vec![ramp(n, 0.1, 0.9)]));
    assert!(!said(&w, "no coordinate system"), "{w:?}");
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn bands_of_very_different_scales_are_said() {
    let n = 64;
    let w = quadtree(stack(georeferenced(8, 8), vec![ramp(n, 0.1, 0.9), ramp(n, 100.0, 900.0)]));
    assert!(said(&w, "very different scales") && said(&w, "normalise the reflectance"), "{w:?}");
    let w = quadtree(stack(georeferenced(8, 8), vec![ramp(n, 0.1, 0.9), ramp(n, 0.2, 0.8)]));
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn reflectance_in_ten_thousands_is_said() {
    let n = 64;
    let w = quadtree(stack(georeferenced(8, 8), vec![ramp(n, 1000.0, 9000.0), ramp(n, 2000.0, 8000.0)]));
    assert!(said(&w, "divide it by 10 000"), "{w:?}");
    assert!(!said(&w, "very different scales"), "{w:?}");
    let w = quadtree(stack(georeferenced(8, 8), vec![ramp(n, 0.01, 0.6)]));
    assert!(w.is_empty(), "{w:?}");
}

#[test]
fn objects_features_reads_the_image_too() {
    let grid = georeferenced(8, 8);
    let o = Objects::from_labels(grid.clone(), (1..=64).collect(), 64).unwrap();
    let image = stack(grid, vec![ramp(64, 1000.0, 9000.0)]);
    let (out, w) = run(
        "objects.features",
        vec![("objects", Value::Objects(Arc::new(o))), ("image", image)],
        serde_json::json!({ "features": ["mean"] }),
    );
    out.unwrap();
    assert!(said(&w, "divide it by 10 000"), "{w:?}");
}

// ---- samples

/// 400 objects of 2 x 2 cells on a 40 x 40 grid (row by row, 20 to a row), the columns id, cells and area.
fn squares(size: usize) -> Arc<Objects> {
    let m = Map::new(georeferenced(40, 40), vec![1.0; 1600]);
    let out = run("segment.chessboard", vec![("image", Value::Map(m))], serde_json::json!({ "size": size })).0.unwrap();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

/// A point at the middle of object `k` (size 2: 20 to a row).
fn point(k: usize) -> Geometry {
    Geometry::Point([(k % 20) as f64 * 2.0 + 0.5, -((k / 20) as f64 * 2.0 + 0.5)])
}

fn samples(items: Vec<(Geometry, &str)>) -> Value {
    let (geometries, classes): (Vec<Geometry>, Vec<String>) =
        items.into_iter().map(|(g, c)| (g, c.to_string())).unzip();
    Value::Vector(Arc::new(Vector {
        geometries,
        attributes: vec![("class".to_string(), Column::Texts(classes))],
        ..Default::default()
    }))
}

fn sample_run(o: &Arc<Objects>, items: Vec<(Geometry, &str)>) -> Vec<String> {
    let (out, w) = run(
        "objects.samples",
        vec![("objects", Value::Objects(o.clone())), ("samples", samples(items))],
        serde_json::json!({}),
    );
    out.unwrap();
    w
}

/// `n` points of a class on objects from `first`.
fn of_class(class: &'static str, first: usize, n: usize) -> Vec<(Geometry, &'static str)> {
    (first..first + n).map(|k| (point(k), class)).collect()
}

#[test]
fn a_class_with_few_samples_is_said() {
    let o = squares(2);
    let mut items = of_class("a", 0, 12);
    items.extend(of_class("b", 100, 3));
    let w = sample_run(&o, items);
    assert!(said(&w, "class \"b\" has 3 sample object(s), under 10") && said(&w, "draw more samples"), "{w:?}");
    assert!(!said(&w, "class \"a\""), "{w:?}");
    let mut items = of_class("a", 0, 15);
    items.extend(of_class("b", 100, 12));
    assert!(sample_run(&o, items).is_empty());
}

#[test]
fn a_class_without_a_sample_object_is_said() {
    let o = squares(2);
    let mut items = of_class("a", 0, 12);
    items.push((Geometry::Point([500.0, -500.0]), "b"));
    let w = sample_run(&o, items);
    assert!(said(&w, "class \"b\" has no sample object"), "{w:?}");
}

#[test]
fn unbalanced_classes_are_said() {
    let o = squares(2);
    let mut items = of_class("a", 0, 110);
    items.extend(of_class("b", 200, 10));
    let w = sample_run(&o, items);
    assert!(said(&w, "very unbalanced") && said(&w, "\"a\" has 110") && said(&w, "\"b\" has 10"), "{w:?}");
    // 10 times is not more than 10 times
    let mut items = of_class("a", 0, 100);
    items.extend(of_class("b", 200, 10));
    assert!(sample_run(&o, items).is_empty());
}

#[test]
fn objects_of_another_size_than_the_polygons_are_said() {
    // 400 polygons' cells in a polygon over 100 objects of 4 cells
    let o = squares(2);
    let big = Geometry::Polygon(vec![vec![[0.0, 0.0], [20.0, 0.0], [20.0, -20.0], [0.0, -20.0], [0.0, 0.0]]]);
    let w = sample_run(&o, vec![(big, "a")]);
    assert!(said(&w, "much smaller than the sample polygons") && said(&w, "larger scale"), "{w:?}");
    // objects of 100 cells and a polygon of 4 cells (it holds no object by half)
    let o = squares(10);
    let mut items: Vec<(Geometry, &str)> = (0..16)
        .map(|k| (Geometry::Point([(k % 4) as f64 * 10.0 + 5.0, -((k / 4) as f64 * 10.0 + 5.0)]), "a"))
        .collect();
    items.push((Geometry::Polygon(vec![vec![[1.0, -1.0], [3.0, -1.0], [3.0, -3.0], [1.0, -3.0], [1.0, -1.0]]]), "a"));
    let w = sample_run(&o, items);
    assert!(said(&w, "much larger than the sample polygons") && said(&w, "smaller scale"), "{w:?}");
    // a polygon of the objects' size: nothing to say
    let o = squares(2);
    let one = Geometry::Polygon(vec![vec![[0.0, 0.0], [4.0, 0.0], [4.0, -4.0], [0.0, -4.0], [0.0, 0.0]]]);
    let mut items = vec![(one, "a")];
    items.extend(of_class("a", 10, 12));
    assert!(!said(&sample_run(&o, items), "than the sample polygons"));
}

// ---- classify and nearest

/// A table of the objects' attributes: `n` objects, a = i, and b = 2a + 1 (equal) or an unrelated one.
fn attributes(n: usize, equal: bool) -> Arc<Objects> {
    let o = Objects::from_labels(georeferenced(n, 1), (1..=n as u32).collect(), n).unwrap();
    let mut t = Table::new(["id", "cells", "area", "a", "b"].map(String::from).to_vec());
    t.rows = (0..n)
        .map(|i| {
            let (a, b) = (i as f64, if equal { 2.0 * i as f64 + 1.0 } else { ((i * 37) % 11) as f64 });
            vec![(i + 1) as f64, 1.0, 1.0, a, b]
        })
        .collect();
    Arc::new(o.with_table(t).unwrap())
}

/// A training table: `ones` objects of class one (a low) and `twos` of class two (a high).
fn training(ones: usize, twos: usize, equal: bool) -> Table {
    let mut t = Table::new(["class", "a", "b"].map(String::from).to_vec());
    for (class, n, base) in [(1.0, ones, 0.0), (2.0, twos, 100.0)] {
        for i in 0..n {
            let a = base + i as f64;
            t.rows.push(vec![class, a, if equal { 2.0 * a + 1.0 } else { ((a as usize * 37) % 11) as f64 }]);
        }
    }
    t.labels.insert("class".into(), vec!["low".into(), "high".into()]);
    t
}

fn classify(o: &Arc<Objects>, train: &Table) -> Vec<String> {
    let fit = run(
        "ml.fit",
        vec![("table", Value::Table(train.clone()))],
        serde_json::json!({ "column": "class", "columns": ["a", "b"], "method": "random_forest", "seed": 1, "min_samples_leaf": 2 }),
    )
    .0
    .unwrap();
    let (out, w) = run(
        "objects.classify",
        vec![
            ("objects", Value::Objects(o.clone())),
            ("model", fit["model"].clone()),
            ("samples", Value::Table(train.clone())),
        ],
        // (the objects reach far beyond the few samples: this test is about what the samples say, not about novelty)
        serde_json::json!({ "novelty": "none" }),
    );
    out.unwrap();
    w
}

fn nearest(o: &Arc<Objects>, train: &Table) -> Vec<String> {
    let (out, w) = run(
        "objects.nearest",
        vec![("objects", Value::Objects(o.clone())), ("samples", Value::Table(train.clone()))],
        serde_json::json!({ "columns": ["a", "b"] }),
    );
    out.unwrap();
    w
}

#[test]
fn classify_and_nearest_say_the_samples() {
    let o = attributes(200, false);
    // a clean case
    assert!(classify(&o, &training(15, 12, false)).is_empty());
    assert!(nearest(&o, &training(15, 12, false)).is_empty());
    for w in [classify(&o, &training(33, 3, false)), nearest(&o, &training(33, 3, false))] {
        assert!(said(&w, "class \"high\" has 3 sample object(s)"), "{w:?}");
        assert!(said(&w, "very unbalanced"), "{w:?}");
    }
    // a class of the labels without a row
    let w = nearest(&o, &training(15, 0, false));
    assert!(said(&w, "class \"high\" has no sample object"), "{w:?}");
}

#[test]
fn two_almost_equal_attributes_are_said() {
    let o = attributes(200, true);
    for w in [classify(&o, &training(15, 12, true)), nearest(&o, &training(15, 12, true))] {
        assert!(said(&w, "\"a\" and \"b\" are almost equal") && said(&w, "drop one"), "{w:?}");
    }
    let o = attributes(200, false);
    assert!(!said(&classify(&o, &training(15, 12, false)), "almost equal"));
    assert!(!said(&nearest(&o, &training(15, 12, false)), "almost equal"));
}

#[test]
fn multiresolution_without_a_scale_says_what_to_set() {
    let n = 16;
    let image = stack(georeferenced(n, n), vec![(0..n * n).map(|i| if i % n < n / 2 { 0.1 } else { 0.8 }).collect()]);
    let (out, w) = run("segment.multiresolution", vec![("image", image)], serde_json::json!({}));
    out.unwrap();
    assert!(said(&w, "no scale given") && said(&w, "set scale to choose it"), "{w:?}");
}
