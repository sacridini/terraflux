//! `objects.accuracy` and `objects.sample`: six objects of known mapped and reference classes against the confusion
//! matrix and accuracies worked out by hand, and the sample's allocation, determinism and points.

use std::sync::Arc;

use terraflux_ops::accuracy::olofsson;
use terraflux_ops::objects::{ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

const NAN: f64 = f64::NAN;

/// Six objects on a 6 x 4 grid:
///
/// ```text
/// 1 1 2 2 3 3
/// 1 1 2 2 3 3
/// 4 4 4 5 5 6
/// 4 4 4 5 5 6
/// ```
#[rustfmt::skip]
const LABELS: [u32; 24] = [
    1, 1, 2, 2, 3, 3,
    1, 1, 2, 2, 3, 3,
    4, 4, 4, 5, 5, 6,
    4, 4, 4, 5, 5, 6,
];

/// The objects with the classes of `class_of` (A 1, B 2, C 3, 0 none).
fn objects(class_of: [u32; 6]) -> Arc<Objects> {
    let o = Objects::from_labels(Arc::new(Grid::new(6, 4)), LABELS.to_vec(), 6).unwrap();
    let classes = ["A", "B", "C"].map(|n| ObjectClass { name: n.into(), colour: None }).to_vec();
    Arc::new(o.with_classes(classes, class_of.to_vec(), Vec::new()).unwrap())
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn table_of(out: &Outputs, name: &str) -> Table {
    let Value::Table(t) = &out[name] else { panic!("{name} is not a table") };
    t.clone()
}

fn points_of(out: &Outputs) -> Arc<Vector> {
    let Value::Vector(v) = &out["points"] else { panic!("points") };
    v.clone()
}

fn near(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || (a - b).abs() < 1e-12
}

/// A point at the centre of a cell (column, row) of the 6 x 4 grid.
fn at(c: usize, r: usize) -> Geometry {
    Geometry::Point([c as f64 + 0.5, -(r as f64 + 0.5)])
}

/// Points over objects 1 to 6 (one cell each) with a class each.
fn points(classes: &[&str]) -> Value {
    let cells = [(0, 0), (2, 0), (4, 0), (0, 2), (3, 2), (5, 2)];
    let geometries = cells.iter().take(classes.len()).map(|&(c, r)| at(c, r)).collect();
    let texts = classes.iter().map(|s| s.to_string()).collect();
    Value::Vector(Arc::new(Vector { geometries, attributes: vec![("class".into(), Column::Texts(texts))], crs: None }))
}

/// The reference classes by hand: A A B B C C for objects 1 to 6, mapped as A A A B B C.
const MAPPED: [u32; 6] = [1, 1, 1, 2, 2, 3];
const REFERENCE: [&str; 6] = ["A", "A", "B", "B", "C", "C"];

fn accuracy(inputs: Vec<(&str, Value)>) -> Outputs {
    run("objects.accuracy", inputs, serde_json::json!({})).unwrap()
}

#[test]
fn the_matrix_and_accuracies_by_objects() {
    let out = accuracy(vec![("objects", Value::Objects(objects(MAPPED))), ("samples", points(&REFERENCE))]);
    // rows the map's class, columns the reference's: A [2 1 0], B [0 1 1], C [0 0 1]
    let m = table_of(&out, "matrix");
    assert_eq!(m.columns, ["map", "A", "B", "C"]);
    assert_eq!(m.cell(0, m.rows[1][0]), "B");
    let counts: Vec<Vec<f64>> = m.rows.iter().map(|r| r[1..].to_vec()).collect();
    assert_eq!(counts, [[2.0, 1.0, 0.0], [0.0, 1.0, 1.0], [0.0, 0.0, 1.0]]);
    let t = table_of(&out, "table");
    let get = |c: &str, row: usize| t.column(c).unwrap()[row];
    // the user's accuracy 2/3, 1/2, 1; the producer's 1, 1/2, 1/2
    for (i, (u, p)) in [(2.0 / 3.0, 1.0), (0.5, 0.5), (1.0, 0.5)].into_iter().enumerate() {
        assert!(near(get("user_objects", i), u), "user {i}");
        assert!(near(get("producer_objects", i), p), "producer {i}");
    }
    assert_eq!(t.column("map_cells").unwrap(), [12.0, 10.0, 2.0]);
    assert_eq!(t.column("reference_objects").unwrap(), [2.0, 2.0, 2.0]);
    // overall 4/6; kappa: chance (3 * 2 + 2 * 2 + 1 * 2) / 36 = 1/3, so (2/3 - 1/3) / (2/3) = 1/2
    let o = table_of(&out, "overall");
    assert!(near(o.column("overall_objects").unwrap()[0], 4.0 / 6.0));
    assert!(near(o.column("kappa").unwrap()[0], 0.5));
    assert_eq!(o.column("objects").unwrap()[0], 6.0);
    assert_eq!(o.column("cells").unwrap()[0], 24.0);
    // the misclassified: object 3 (A, reference B) and object 5 (B, reference C)
    let e = table_of(&out, "errors");
    let texts: Vec<(f64, String, String)> = e.rows.iter().map(|r| (r[0], e.cell(1, r[1]), e.cell(2, r[2]))).collect();
    assert_eq!(texts, [(3.0, "A".into(), "B".into()), (5.0, "B".into(), "C".into())]);
}

#[test]
fn the_adjusted_estimates_are_olofssons() {
    let out = accuracy(vec![("objects", Value::Objects(objects(MAPPED))), ("samples", points(&REFERENCE))]);
    let e = olofsson(
        &[1.0, 1.0, 1.0, 2.0, 2.0, 3.0],
        &[1.0, 1.0, 2.0, 2.0, 3.0, 3.0],
        &[1.0, 2.0, 3.0],
        &[12.0, 10.0, 2.0],
    )
    .unwrap();
    let t = table_of(&out, "table");
    let z = 1.959963984540054;
    for i in 0..3 {
        let col = |c: &str| t.column(c).unwrap()[i];
        assert!(near(col("user_adjusted"), e.user[i]), "user {i}");
        assert!(near(col("producer_adjusted"), e.producer[i]), "producer {i}");
        assert!(near(col("area_share"), e.area[i]), "area {i}");
        assert!(near(col("area_cells"), e.area[i] * 24.0), "cells {i}");
        assert!((col("area_cells_ci") - z * e.area_se[i] * 24.0).abs() < 1e-6, "ci {i}");
    }
    let o = table_of(&out, "overall");
    assert!(near(o.column("overall_adjusted").unwrap()[0], e.overall));
    assert!(near(o.column("overall_se").unwrap()[0], e.overall_se));
}

#[test]
fn the_reference_as_a_table_and_as_polygons_agree() {
    let want = accuracy(vec![("objects", Value::Objects(objects(MAPPED))), ("samples", points(&REFERENCE))]);
    // a table: id and class (text)
    let mut r = Table::new(vec!["id".into(), "class".into()]);
    r.rows = (0..6).map(|i| vec![(i + 1) as f64, [1.0, 1.0, 2.0, 2.0, 3.0, 3.0][i]]).collect();
    r.labels.insert("class".into(), ["A", "B", "C"].map(String::from).to_vec());
    let by_table = accuracy(vec![("objects", Value::Objects(objects(MAPPED))), ("reference", Value::Table(r))]);
    // polygons: each over the cells of its object
    let sq = |c0: f64, r0: f64, c1: f64, r1: f64| {
        Geometry::Polygon(vec![vec![[c0, -r0], [c1, -r0], [c1, -r1], [c0, -r1], [c0, -r0]]])
    };
    let v = Vector {
        geometries: vec![
            sq(0.0, 0.0, 2.0, 2.0),
            sq(2.0, 0.0, 4.0, 2.0),
            sq(4.0, 0.0, 6.0, 2.0),
            sq(0.0, 2.0, 3.0, 4.0),
            sq(3.0, 2.0, 5.0, 4.0),
            sq(5.0, 2.0, 6.0, 4.0),
        ],
        attributes: vec![("class".into(), Column::Texts(REFERENCE.iter().map(|s| s.to_string()).collect()))],
        crs: None,
    };
    let by_poly = accuracy(vec![("objects", Value::Objects(objects(MAPPED))), ("samples", Value::Vector(Arc::new(v)))]);
    for out in [&by_table, &by_poly] {
        for name in ["table", "overall", "matrix", "errors"] {
            let (a, b) = (table_of(&want, name), table_of(out, name));
            assert_eq!(a.columns, b.columns, "{name}");
            assert!(
                a.rows.len() == b.rows.len()
                    && a.rows.iter().flatten().zip(b.rows.iter().flatten()).all(|(x, y)| near(*x, *y)),
                "{name}: {:?} vs {:?}",
                a.rows,
                b.rows
            );
        }
    }
}

#[test]
fn an_unclassified_object_is_a_row_of_the_matrix() {
    // object 6 is left unclassified and its sample says C
    let out = accuracy(vec![("objects", Value::Objects(objects([1, 1, 1, 2, 2, 0]))), ("samples", points(&REFERENCE))]);
    let m = table_of(&out, "matrix");
    let rows: Vec<String> = m.rows.iter().map(|r| m.cell(0, r[0])).collect();
    assert_eq!(rows, ["A", "B", "C", "unclassified"]);
    assert_eq!(m.rows[3][1..], [0.0, 0.0, 1.0, 0.0]);
}

#[test]
fn a_class_with_no_sample_is_left_out_and_the_inputs_are_checked() {
    // object 6 (the only C) has no sample
    let out = accuracy(vec![("objects", Value::Objects(objects(MAPPED))), ("samples", points(&REFERENCE[..5]))]);
    assert_eq!(table_of(&out, "table").rows.len(), 3); // C is a reference class, with no cells in the map
    let objs = || ("objects", Value::Objects(objects(MAPPED)));
    assert!(run("objects.accuracy", vec![objs()], serde_json::json!({})).is_err());
    assert!(run("objects.accuracy", vec![objs(), ("samples", points(&[]))], serde_json::json!({})).is_err());
    assert!(
        run(
            "objects.accuracy",
            vec![("objects", Value::Objects(objects([0; 6]))), ("samples", points(&REFERENCE))],
            serde_json::json!({})
        )
        .is_err()
    );
}

fn sample(params: serde_json::Value) -> Outputs {
    run("objects.sample", vec![("objects", Value::Objects(objects(MAPPED)))], params).unwrap()
}

/// The ids and classes of a sample's points.
fn drawn(out: &Outputs) -> Vec<(f64, String)> {
    let v = points_of(out);
    let Some(Column::Numbers(ids)) = v.column("id") else { panic!("id") };
    let Some(Column::Texts(mapped)) = v.column("mapped") else { panic!("mapped") };
    ids.iter().copied().zip(mapped.iter().cloned()).collect()
}

#[test]
fn a_sample_is_stratified_deterministic_and_inside_its_objects() {
    let a = sample(serde_json::json!({ "per_class": 1, "seed": 7 }));
    // one per class (A has 3 objects, B 2, C 1); each point in its object, the reference empty
    let d = drawn(&a);
    assert_eq!(d.len(), 3);
    assert_eq!(d.iter().map(|(_, c)| c.as_str()).collect::<Vec<_>>(), ["A", "B", "C"]);
    let v = points_of(&a);
    let o = objects(MAPPED);
    for (k, (id, _)) in d.iter().enumerate() {
        let Geometry::Point(p) = v.geometries[k] else { panic!("a point") };
        assert_eq!(o.id_at((-p[1]).floor() as usize, p[0].floor() as usize) as f64, *id);
    }
    let Some(Column::Texts(r)) = v.column("reference") else { panic!("reference") };
    assert!(r.iter().all(String::is_empty));
    // the same seed, the same objects
    assert_eq!(drawn(&sample(serde_json::json!({ "per_class": 1, "seed": 7 }))), d);
    // every seed draws one object of each class: object 6 is the only C
    assert!((1..20).all(|s| drawn(&sample(serde_json::json!({ "per_class": 1, "seed": s })))[2].0 == 6.0));
    // the objects drawn from A vary with the seed
    let firsts: std::collections::BTreeSet<u64> =
        (1..20).map(|s| drawn(&sample(serde_json::json!({ "per_class": 1, "seed": s })))[0].0 as u64).collect();
    assert!(firsts.len() > 1);
}

#[test]
fn the_allocation_shares_n_equally_with_caps() {
    // n 4 over three classes of 3, 2 and 1 objects: 4 / 3 each, C capped at 1, the rest to A and B
    let t = table_of(&sample(serde_json::json!({ "n": 4 })), "table");
    let total: f64 = t.column("sample").unwrap().iter().sum();
    assert_eq!(total, 4.0);
    assert_eq!(t.column("objects").unwrap(), [3.0, 2.0, 1.0]);
    assert_eq!(t.column("sample").unwrap()[2], 1.0);
    // more than there are objects: all of them
    let all = sample(serde_json::json!({ "n": 100 }));
    assert_eq!(drawn(&all).len(), 6);
    // per_class over the objects of a class: all of its objects
    assert_eq!(
        table_of(&sample(serde_json::json!({ "per_class": 2 })), "table").column("sample").unwrap(),
        [2.0, 2.0, 1.0]
    );
}

#[test]
fn a_point_is_the_object_cell_nearest_its_centroid() {
    // object 4 is a 3 x 2 block: its centroid (row 2.5, column 1) is nearest the cell (row 2, column 1)
    let all = sample(serde_json::json!({ "per_class": 3 }));
    let v = points_of(&all);
    let d = drawn(&all);
    let k = d.iter().position(|(id, _)| *id == 4.0).unwrap();
    let Geometry::Point(p) = v.geometries[k] else { panic!("a point") };
    assert_eq!(p, [1.5, -2.5]);
    // a table of the sample with the reference still empty is refused, whatever the ids
    let mut r = Table::new(vec!["id".into(), "class".into()]);
    r.rows = d.iter().map(|(id, _)| vec![*id, NAN]).collect();
    assert!(
        run(
            "objects.accuracy",
            vec![("objects", Value::Objects(objects(MAPPED))), ("reference", Value::Table(r))],
            serde_json::json!({})
        )
        .is_err()
    );
}
