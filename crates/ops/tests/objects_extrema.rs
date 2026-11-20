//! `objects.extrema`: the maximum at the centre of a 3 x 3 grid of objects, the radius against the steps, the classes
//! compared, ties (strict or not), objects without a value, an expression as the attribute, the radius against a brute
//! force, the errors, and the same result on 1 and 8 threads.

use std::collections::BTreeSet;
use std::sync::Arc;

use terraflux_ops::objects::{ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;

/// A grid of `nr` x `nc` objects of `size` x `size` cells, ids in row-major order, with the attribute `v` in the table.
fn lattice(nr: usize, nc: usize, size: usize, v: &[f64]) -> Arc<Objects> {
    let (w, h) = (nc * size, nr * size);
    let region: Vec<u64> = (0..w * h).map(|i| ((i / w / size) * nc + (i % w) / size) as u64).collect();
    let o = Objects::from_regions(Arc::new(Grid::new(w, h)), &region, 4);
    assert_eq!(o.count, nr * nc);
    let mut t = Table::new(vec!["id".into(), "v".into()]);
    for (k, &x) in v.iter().enumerate() {
        t.rows.push(vec![(k + 1) as f64, x]);
    }
    Arc::new(o.with_table(t).unwrap())
}

fn extrema_with(o: &Arc<Objects>, params: serde_json::Value) -> (terraflux_ops::Result<Outputs>, Vec<String>) {
    let r = Registry::standard();
    let mut p = params.as_object().cloned().unwrap_or_default();
    p.entry("attribute").or_insert(serde_json::json!("v"));
    let args = Args {
        node: "n",
        inputs: [("objects".to_string(), Value::Objects(o.clone()))].into_iter().collect(),
        params: &p,
    };
    let mut ctx = Ctx::default();
    let out = r.get("objects.extrema").unwrap().run(&mut ctx, &args);
    (out, ctx.warnings.clone())
}

fn extrema(o: &Arc<Objects>, params: serde_json::Value) -> Arc<Objects> {
    let (out, _) = extrema_with(o, params);
    let Value::Objects(n) = &out.unwrap()["objects"] else { panic!("objects") };
    n.clone()
}

/// The ids (1-based) of the objects of the class `name`.
fn marked(o: &Objects, name: &str) -> Vec<u32> {
    (1..=o.count as u32).filter(|&id| o.class_name(id) == Some(name)).collect()
}

fn with_classes(o: &Arc<Objects>, names: &[&str], class_of: Vec<u32>) -> Arc<Objects> {
    let classes = names.iter().map(|n| ObjectClass { name: n.to_string(), colour: None }).collect();
    Arc::new(o.as_ref().clone().with_classes(classes, class_of, Vec::new()).unwrap())
}

#[test]
fn the_maximum_at_the_centre_of_a_3_by_3_grid() {
    let o = lattice(3, 3, 3, &[1.0, 2.0, 3.0, 4.0, 100.0, 5.0, 6.0, 7.0, 0.5]);
    let (out, warnings) = extrema_with(&o, serde_json::json!({}));
    let out = out.unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let Value::Objects(n) = &out["objects"] else { panic!("objects") };
    assert_eq!(marked(n, "maximum"), vec![5]);
    // the table has the column
    let Value::Table(t) = &out["table"] else { panic!("table") };
    let j = t.columns.iter().position(|c| c == "extremum").unwrap();
    let col: Vec<f64> = t.rows.iter().map(|r| r[j]).collect();
    assert_eq!(col, vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0]);
    // the objects are the same cells, with the same table's other columns
    assert_eq!(&n.labels[..], &o.labels[..]);
    assert_eq!(n.table.rows[4][1], 100.0);
    // the minima: the objects lower than all their neighbours
    let m = extrema(&o, serde_json::json!({ "kind": "min" }));
    assert_eq!(marked(&m, "minimum"), vec![1, 9]);
    // the class given
    let c = extrema(&o, serde_json::json!({ "class": "crown top" }));
    assert_eq!(marked(&c, "crown top"), vec![5]);
}

#[test]
fn the_radius_reaches_the_diagonals_the_steps_do_not() {
    let o = lattice(3, 3, 3, &[5.0, 1.0, 2.0, 1.5, 3.0, 2.5, 0.1, 0.2, 0.3]);
    // one step: the objects that share a border
    assert_eq!(marked(&extrema(&o, serde_json::json!({})), "maximum"), vec![1, 5]);
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "steps": 1 })), "maximum"), vec![1, 5]);
    // a radius of 3.5 map units is the same four neighbours (centroids 3 apart); 4.5 takes the diagonals (4.24)
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "radius": 3.5 })), "maximum"), vec![1, 5]);
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "radius": 4.5 })), "maximum"), vec![1]);
    // two steps reach the diagonals as well, and the opposite corner of the row
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "steps": 2 })), "maximum"), vec![1]);
    // a radius that reaches everything: the one maximum
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "radius": 100.0 })), "maximum"), vec![1]);
}

#[test]
fn among_limits_the_classes_compared_and_marked() {
    let o = lattice(3, 3, 3, &[5.0, 1.0, 2.0, 1.5, 3.0, 2.5, 0.1, 0.2, 0.3]);
    // the centre is a road: it is not compared, so the tree at its right is a maximum too
    let o = with_classes(&o, &["tree", "road"], vec![1, 1, 1, 1, 2, 1, 1, 1, 1]);
    let n = extrema(&o, serde_json::json!({ "among": ["tree"] }));
    assert_eq!(marked(&n, "maximum"), vec![1, 6]);
    // the road keeps its class and is not marked
    assert_eq!(n.class_name(5), Some("road"));
    assert_eq!(n.class_name(1), Some("maximum"));
    assert_eq!(n.class_name(2), Some("tree"));
    let Some(j) = n.table.columns.iter().position(|c| c == "extremum") else { panic!("column") };
    assert!(n.table.rows[4][j].is_nan());
    assert_eq!(n.table.rows[5][j], 1.0);
    assert_eq!(n.table.rows[1][j], 0.0);
    // without among the road is compared (and marked, being the highest of its neighbours)
    let all = extrema(&o, serde_json::json!({}));
    assert_eq!(marked(&all, "maximum"), vec![1, 5]);
    // a class that does not exist is refused, with the ones there are
    let e = extrema_with(&o, serde_json::json!({ "among": ["water"] })).0.err().unwrap();
    assert!(e.0.contains("water") && e.0.contains("tree"), "{}", e.0);
}

#[test]
fn a_tie_is_no_extremum_when_strict() {
    // objects 1 and 2 are neighbours with the same value, the highest around
    let o = lattice(3, 3, 3, &[5.0, 5.0, 1.0, 2.0, 3.0, 4.0, 0.1, 0.2, 0.3]);
    assert_eq!(marked(&extrema(&o, serde_json::json!({})), "maximum"), vec![6]);
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "strict": true })), "maximum"), vec![6]);
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "strict": false })), "maximum"), vec![1, 2, 6]);
    // the same for the minima
    let o = lattice(3, 3, 3, &[0.0, 0.0, 9.0, 8.0, 7.0, 6.0, 5.0, 4.0, 3.0]);
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "kind": "min" })), "minimum"), vec![9]);
    assert_eq!(marked(&extrema(&o, serde_json::json!({ "kind": "min", "strict": false })), "minimum"), vec![1, 2, 9]);
}

#[test]
fn an_object_without_a_value_is_not_compared() {
    let o = lattice(3, 3, 3, &[1.0, 2.0, 3.0, 4.0, f64::NAN, 5.0, 6.0, 7.0, 0.5]);
    let n = extrema(&o, serde_json::json!({}));
    // the centre has no value: it is none, and does not stop its neighbours
    assert_eq!(marked(&n, "maximum"), vec![6, 8]);
    let Some(j) = n.table.columns.iter().position(|c| c == "extremum") else { panic!("column") };
    assert!(n.table.rows[4][j].is_nan());
    // all without a value: nothing, with a warning
    let o = lattice(3, 3, 3, &[f64::NAN; 9]);
    let (out, warnings) = extrema_with(&o, serde_json::json!({}));
    let Value::Objects(n) = &out.unwrap()["objects"] else { panic!("objects") };
    assert!(marked(n, "maximum").is_empty());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[test]
fn the_attribute_may_be_an_expression() {
    let o = lattice(3, 3, 3, &[1.0, 2.0, 3.0, 4.0, 100.0, 5.0, 6.0, 7.0, 0.5]);
    // minus v: the maxima of it are the minima of v
    let a = extrema(&o, serde_json::json!({ "attribute": "-v" }));
    let b = extrema(&o, serde_json::json!({ "kind": "min" }));
    assert_eq!(marked(&a, "maximum"), vec![1, 9]);
    assert_eq!(&a.class_of[..], &b.class_of[..]);
    // a neighbourhood mean: the context of objects.calc
    let m = extrema(&o, serde_json::json!({ "attribute": "nb.max(v)" }));
    assert_eq!(marked(&m, "maximum"), vec![2, 4, 6, 8]);
    // a column that is not there, a text, a bad expression
    let e = extrema_with(&o, serde_json::json!({ "attribute": "nothing" })).0.err().unwrap();
    assert!(e.0.contains("nothing"), "{}", e.0);
    let e = extrema_with(&o, serde_json::json!({ "attribute": "cls" })).0.err().unwrap();
    assert!(e.0.contains("text"), "{}", e.0);
}

#[test]
fn the_settings_are_checked() {
    let o = lattice(3, 3, 3, &[1.0; 9]);
    for p in [
        serde_json::json!({ "radius": 5.0, "steps": 2 }),
        serde_json::json!({ "radius": 0.0 }),
        serde_json::json!({ "radius": -1.0 }),
        serde_json::json!({ "steps": 0 }),
        serde_json::json!({ "kind": "mean" }),
    ] {
        assert!(extrema_with(&o, p.clone()).0.is_err(), "{p}");
    }
}

/// Values that are pseudo-random but fixed.
fn noise(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            // (a few levels, so that there are ties)
            ((s >> 40) % 400) as f64 / 4.0
        })
        .collect()
}

#[test]
fn the_radius_agrees_with_a_brute_force_search() {
    let (nr, nc, size) = (30usize, 40usize, 4usize);
    let o = lattice(nr, nc, size, &noise(nr * nc, 11));
    let v: Vec<f64> = o.table.rows.iter().map(|r| r[1]).collect();
    let centre = |k: usize| (((k % nc) * size) as f64 + 1.5, ((k / nc) * size) as f64 + 1.5);
    for radius in [4.0, 6.0, 9.5, 17.0] {
        for strict in [true, false] {
            let n = extrema(&o, serde_json::json!({ "radius": radius, "strict": strict }));
            let got: BTreeSet<u32> = marked(&n, "maximum").into_iter().collect();
            let mut want = BTreeSet::new();
            for i in 0..nr * nc {
                let ok = (0..nr * nc).filter(|&j| j != i).all(|j| {
                    let (a, b) = (centre(i), centre(j));
                    let d = ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
                    d > radius * (1.0 + 1e-9) || if strict { v[i] > v[j] } else { v[i] >= v[j] }
                });
                if ok {
                    want.insert(i as u32 + 1);
                }
            }
            assert!(!want.is_empty());
            assert_eq!(got, want, "radius {radius}, strict {strict}");
        }
    }
}

#[test]
fn the_same_objects_on_1_and_8_threads() {
    let (nr, nc) = (60usize, 70usize);
    let o = lattice(nr, nc, 3, &noise(nr * nc, 5));
    let cases = [
        serde_json::json!({}),
        serde_json::json!({ "steps": 3, "kind": "min" }),
        serde_json::json!({ "radius": 8.0 }),
        serde_json::json!({ "radius": 8.0, "strict": false, "kind": "min" }),
    ];
    for p in cases {
        let on = |n: usize| {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(n).build().unwrap();
            pool.install(|| extrema(&o, p.clone()))
        };
        let (a, b) = (on(1), on(8));
        assert_eq!(a.class_of, b.class_of, "{p}");
        assert_eq!(a.table.rows.len(), b.table.rows.len());
        for (x, y) in a.table.rows.iter().zip(&b.table.rows) {
            assert_eq!(format!("{x:?}"), format!("{y:?}"), "{p}");
        }
        let count = a.class_of.iter().filter(|&&k| k != 0).count();
        assert!(count > 0 && count < nr * nc / 2, "{count} extrema");
    }
}

#[test]
fn the_classes_and_levels_are_kept() {
    let o = lattice(3, 3, 3, &[5.0, 1.0, 2.0, 1.5, 3.0, 2.5, 0.1, 0.2, 0.3]);
    let o = with_classes(&o, &["tree", "road"], vec![1, 1, 1, 2, 2, 2, 0, 0, 0]);
    let n = extrema(&o, serde_json::json!({ "class": "road" }));
    // an existing class is used, none is added
    assert_eq!(n.classes.len(), 2);
    assert_eq!(marked(&n, "road"), vec![1, 4, 5, 6]);
    assert_eq!(n.class_name(2), Some("tree"));
    assert_eq!(n.class_name(7), None);
    // a new one is added after the others
    let n = extrema(&o, serde_json::json!({}));
    assert_eq!(n.classes.len(), 3);
    assert_eq!(n.classes[2].name, "maximum");
}
