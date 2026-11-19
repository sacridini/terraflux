//! `objects.resolve`: the arbitration of the hypotheses of two classes on a 4x4 map, against the winners worked out
//! by hand.

use std::sync::Arc;

use terraflux_ops::objects::Objects;
use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Table, Value};
use terraflux_raster::Grid;

const NAN: f64 = f64::NAN;

/// The membership of class A and of class B, row by row. They overlap on most cells of the top left.
const A: [f64; 16] = [0.9, 0.8, 0.3, 0.0, 0.7, 0.6, 0.5, 0.0, 0.0, 0.0, 0.0, 0.4, 0.0, 0.0, 0.2, 0.4];
const B: [f64; 16] = [0.0, 0.5, 0.3, 0.9, 0.0, 0.7, 0.2, 0.8, 0.6, 0.6, 0.0, 0.4, 0.05, 0.0, 0.3, 0.1];

fn grid() -> Arc<Grid> {
    Arc::new(Grid::new(4, 4))
}

fn memberships() -> Value {
    Value::Stack(Stack {
        grid: grid(),
        keys: vec![Key::Name("A".into()), Key::Name("B".into())],
        layers: vec![Arc::new(A.to_vec()), Arc::new(B.to_vec())],
    })
}

fn run(inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get("objects.resolve").unwrap().run(&mut Ctx::default(), &args)
}

fn resolve(params: serde_json::Value) -> Outputs {
    run(vec![("memberships", memberships())], params).unwrap()
}

fn map_of(out: &Outputs, name: &str) -> Vec<f64> {
    let Value::Map(m) = &out[name] else { panic!("{name} is not a map") };
    m.data.to_vec()
}

fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

fn table_of(out: &Outputs, name: &str) -> Table {
    let Value::Table(t) = &out[name] else { panic!("{name} is not a table") };
    t.clone()
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y || (x.is_nan() && y.is_nan()))
}

/// The classes by hand (A 1, B 2), ties to A: min 0.1.
#[rustfmt::skip]
const CLASS: [f64; 16] = [
    1.0, 1.0, 1.0, 2.0,
    1.0, 2.0, 1.0, 2.0,
    2.0, 2.0, NAN, 1.0,
    NAN, NAN, 2.0, 1.0,
];

#[test]
fn resolve_winner_per_cell() {
    let out = resolve(serde_json::json!({}));
    assert!(same(&map_of(&out, "class"), &CLASS), "{:?}", map_of(&out, "class"));
    let best = map_of(&out, "membership");
    // the winning membership: the larger of the two, none where no class won
    let want: Vec<f64> = (0..16).map(|i| if CLASS[i].is_nan() { NAN } else { A[i].max(B[i]) }).collect();
    assert!(same(&best, &want), "{best:?}");
    // the overlapped hypothesis keeps its part: A won 7 cells of its 9, B won 6 of its 11
    let won = |c: f64| CLASS.iter().filter(|&&x| x == c).count();
    assert_eq!((won(1.0), won(2.0)), (7, 6));
    let o = objects_of(&out);
    assert_eq!(o.classes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["A", "B"]);
    assert_eq!(o.class_name(o.id_at(0, 0)), Some("A"));
    assert_eq!(o.class_name(o.id_at(1, 1)), Some("B"));
    assert_eq!(o.id_at(2, 2), 0);
}

#[test]
fn resolve_min_leaves_weak_cells() {
    let out = resolve(serde_json::json!({ "min": 0.35 }));
    let class = map_of(&out, "class");
    // below 0.35: (0,2) 0.3 / 0.3 and (3,2) 0.3 have no class; (1,0) 0.7, (2,3) 0.4 and (3,3) 0.4 stay
    assert!(class[2].is_nan() && class[14].is_nan());
    assert_eq!((class[4], class[11], class[15]), (1.0, 1.0, 1.0));
    // the default 0.1 leaves only (3,0) and (3,1), (2,2) with none
    let default = map_of(&resolve(serde_json::json!({})), "class");
    assert_eq!(default.iter().filter(|v| v.is_nan()).count(), 3);
    assert!(run(vec![("memberships", memberships())], serde_json::json!({ "min": 2.0 })).is_err());
}

#[test]
fn resolve_tie_goes_by_order() {
    // (0,2) 0.3 / 0.3 and (2,3) 0.4 / 0.4 are ties
    let a = map_of(&resolve(serde_json::json!({})), "class");
    assert_eq!((a[2], a[11]), (1.0, 1.0));
    let b = map_of(&resolve(serde_json::json!({ "order": ["B", "A"] })), "class");
    assert_eq!((b[2], b[11]), (2.0, 2.0));
    // the other cells do not move
    assert!((0..16).filter(|i| *i != 2 && *i != 11).all(|i| a[i].is_nan() && b[i].is_nan() || a[i] == b[i]));
    assert!(run(vec![("memberships", memberships())], serde_json::json!({ "order": ["C"] })).is_err());
}

#[test]
fn resolve_merge_modes() {
    let count = |merge: &str, class: &str| {
        let o = objects_of(&resolve(serde_json::json!({ "merge": merge })));
        (0..o.count as u32).filter(|id| o.class_name(id + 1) == Some(class)).count()
    };
    // A has two patches (the top left, and (2,3) with (3,3)); B has three
    assert_eq!((count("connected", "A"), count("connected", "B")), (2, 3));
    assert_eq!(objects_of(&resolve(serde_json::json!({}))).count, 5);
    assert_eq!((count("global", "A"), count("global", "B")), (1, 1));
    let g = objects_of(&resolve(serde_json::json!({ "merge": "global" })));
    assert_eq!(g.cells, [7, 6]);
    assert_eq!((count("none", "A"), count("none", "B")), (7, 6));
    assert!(run(vec![("memberships", memberships())], serde_json::json!({ "merge": "x" })).is_err());
}

#[test]
fn resolve_object_membership_and_table() {
    let out = resolve(serde_json::json!({ "merge": "global" }));
    let o = objects_of(&out);
    // the mean of each class's membership over the object's cells: A's cells hold 0.9 0.8 0.3 0.7 0.5 0.4 0.4
    assert!((o.membership[0][0] - 4.0 / 7.0).abs() < 1e-12);
    // B's membership over A's cells: 0 0.5 0.3 0 0.2 0.4 0.1
    assert!((o.membership[1][0] - 1.5 / 7.0).abs() < 1e-12);
    let t = table_of(&out, "table");
    assert_eq!(t.columns, ["id", "cells", "area", "class", "membership"]);
    assert_eq!(t.cell(3, t.rows[0][3]), "A");
    assert_eq!(t.cell(3, t.rows[1][3]), "B");
    assert_eq!(t.rows[0][1], 7.0);
    assert!((t.rows[0][4] - 4.0 / 7.0).abs() < 1e-12);
}

#[test]
fn resolve_conflicts_and_doubt() {
    let out = resolve(serde_json::json!({}));
    let c = table_of(&out, "conflicts");
    assert_eq!(c.columns, ["class", "lost_to", "cells", "area"]);
    // A's 9 cells of hypotheses: 2 went to B ((1,1) and (3,2)); B's 11: 5 went to A
    assert_eq!(c.rows.len(), 2);
    let area = objects_of(&out).cell_area();
    assert_eq!((c.cell(0, c.rows[0][0]), c.cell(1, c.rows[0][1]), c.rows[0][2]), ("A".into(), "B".into(), 2.0));
    assert_eq!((c.cell(0, c.rows[1][0]), c.cell(1, c.rows[1][1]), c.rows[1][2]), ("B".into(), "A".into(), 5.0));
    assert!((c.rows[1][3] - 5.0 * area).abs() < 1e-12);
    // the doubt is the second highest, the smaller of the two here
    let d = map_of(&out, "doubt");
    let want: Vec<f64> = (0..16).map(|i| A[i].min(B[i])).collect();
    assert!(same(&d, &want), "{d:?}");
}

#[test]
fn resolve_region_restricts() {
    // only the left two columns
    let region: Vec<f64> = (0..16).map(|i| if i % 4 < 2 { 1.0 } else { 0.0 }).collect();
    let out = run(
        vec![("memberships", memberships()), ("region", Value::Map(Map::new(grid(), region)))],
        serde_json::json!({}),
    )
    .unwrap();
    let class = map_of(&out, "class");
    let doubt = map_of(&out, "doubt");
    for i in 0..16 {
        if i % 4 < 2 {
            assert!(same(&[class[i]], &[CLASS[i]]), "cell {i}");
            assert!(!doubt[i].is_nan());
        } else {
            assert!(class[i].is_nan() && doubt[i].is_nan(), "cell {i}");
        }
    }
    let o = objects_of(&out);
    assert_eq!(o.id_at(0, 3), 0);
    assert_ne!(o.id_at(0, 0), 0);
    // a lost hypothesis is counted only inside the region: (1,1) of A went to B and (0,1) of B to A
    let c = table_of(&out, "conflicts");
    assert_eq!(c.rows.iter().map(|r| r[2]).sum::<f64>(), 2.0);
}
