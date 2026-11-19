//! `objects.temporal` and `objects.transition_matrix`: a transition of possibility 0 never happens, a matrix of ones
//! changes nothing, the max-min composition by hand, an accuracy gain against a known truth, and the matrix
//! estimated from samples.

use std::sync::Arc;

use terraflux_ops::objects::{ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn names(n: &[&str]) -> Vec<ObjectClass> {
    n.iter().map(|s| ObjectClass { name: s.to_string(), colour: None }).collect()
}

/// Objects on a grid from their labels, classified with memberships (class, object): the class is the highest.
fn objs(w: usize, h: usize, labels: Vec<u32>, classes: &[&str], mem: Vec<Vec<f64>>) -> Arc<Objects> {
    let count = *labels.iter().max().unwrap() as usize;
    let o = Objects::from_labels(Arc::new(Grid::new(w, h)), labels, count).unwrap();
    let class_of = (0..count)
        .map(|j| {
            let mut best = (0u32, 0.0);
            for (c, m) in mem.iter().enumerate() {
                if m[j] > best.1 {
                    best = (c as u32 + 1, m[j]);
                }
            }
            best.0
        })
        .collect();
    Arc::new(o.with_classes(names(classes), class_of, mem).unwrap())
}

/// A matrix: a row per class of origin (`class` text, then a column per class).
fn matrix(classes: &[&str], m: &[&[f64]]) -> Table {
    let n: Vec<String> = classes.iter().map(|s| s.to_string()).collect();
    let mut t = Table::new(std::iter::once("class".to_string()).chain(n.iter().cloned()).collect());
    t.labels.insert("class".into(), n);
    t.rows =
        m.iter().enumerate().map(|(k, r)| std::iter::once((k + 1) as f64).chain(r.iter().copied()).collect()).collect();
    t
}

fn temporal(
    before: &Arc<Objects>,
    after: &Arc<Objects>,
    m: Table,
    params: serde_json::Value,
) -> terraflux_ops::Result<Outputs> {
    run(
        "objects.temporal",
        vec![
            ("before", Value::Objects(before.clone())),
            ("after", Value::Objects(after.clone())),
            ("matrix", Value::Table(m)),
        ],
        params,
    )
}

fn objects_of(out: &Outputs, port: &str) -> Arc<Objects> {
    let Value::Objects(o) = &out[port] else { panic!("types") };
    o.clone()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn temporal_impossible_transition_never_happens() {
    let before = objs(2, 1, vec![1, 1], &["A", "B"], vec![vec![1.0], vec![0.0]]);
    let after = objs(2, 1, vec![1, 1], &["A", "B"], vec![vec![0.2], vec![0.9]]);
    assert_eq!(after.class_of, vec![2]);
    let m = matrix(&["A", "B"], &[&[1.0, 0.0], &[1.0, 1.0]]);
    let out = temporal(&before, &after, m, serde_json::json!({})).unwrap();
    let o = objects_of(&out, "objects");
    assert_eq!(o.class_of, vec![1]);
    assert!(close(o.membership[1][0], 0.0));
    assert!(close(o.membership[0][0], 0.2));
}

#[test]
fn temporal_matrix_of_ones_is_the_after_date_alone() {
    // different segmentations, 3 objects each; the first date is sure (a membership of 1)
    let before = objs(
        6,
        1,
        vec![1, 1, 2, 2, 3, 3],
        &["A", "B", "C"],
        vec![vec![1.0, 1.0, 0.0], vec![0.0, 0.0, 1.0], vec![0.0, 0.0, 0.0]],
    );
    let after = objs(
        6,
        1,
        vec![1, 2, 2, 2, 3, 3],
        &["A", "B", "C"],
        vec![vec![0.6, 0.2, 0.05], vec![0.3, 0.7, 0.05], vec![0.1, 0.1, 0.9]],
    );
    let m = matrix(&["A", "B", "C"], &[&[1.0; 3], &[1.0; 3], &[1.0; 3]]);
    let out = temporal(&before, &after, m, serde_json::json!({})).unwrap();
    let o = objects_of(&out, "objects");
    assert_eq!(o.class_of, after.class_of);
    for c in 0..3 {
        for j in 0..3 {
            assert!(close(o.membership[c][j], after.membership[c][j].min(1.0)));
        }
    }
    // the first date as given
    assert_eq!(objects_of(&out, "before").class_of, before.class_of);
}

/// The two-by-two case by hand, the dates on one segmentation: left and right halves of a 4x1 grid.
fn hand() -> (Arc<Objects>, Arc<Objects>, Table) {
    let labels = vec![1, 1, 2, 2];
    let before = objs(4, 1, labels.clone(), &["A", "B"], vec![vec![0.8, 0.3], vec![0.2, 0.6]]);
    let after = objs(4, 1, labels, &["A", "B"], vec![vec![0.9, 0.7], vec![0.7, 0.8]]);
    (before, after, matrix(&["A", "B"], &[&[1.0, 0.4], &[0.5, 0.9]]))
}

#[test]
fn temporal_max_min_by_hand() {
    let (before, after, m) = hand();
    let out = temporal(&before, &after, m, serde_json::json!({})).unwrap();
    let o = objects_of(&out, "objects");
    // object 1: before (0.8, 0.2): A = min(0.9, max(min(0.8,1), min(0.2,0.5))) = 0.8, B = min(0.7, max(0.4, 0.2)) = 0.4
    // object 2: before (0.3, 0.6): A = min(0.7, max(0.3, min(0.6,0.5))) = 0.5, B = min(0.8, max(0.3, 0.6)) = 0.6
    let want = [[0.8, 0.5], [0.4, 0.6]];
    for (c, row) in want.iter().enumerate() {
        for (j, &w) in row.iter().enumerate() {
            assert!(close(o.membership[c][j], w), "class {c} object {j}: {}", o.membership[c][j]);
        }
    }
    assert_eq!(o.class_of, vec![1, 2]);
    // the map: the class number of each cell
    let Value::Map(map) = &out["map"] else { panic!("types") };
    assert_eq!(*map.data, vec![1.0, 1.0, 2.0, 2.0]);
    // the change table: A -> A and B -> B, an object of 2 cells each
    let Value::Table(t) = &out["change"] else { panic!("types") };
    assert_eq!(t.columns, ["from", "to", "objects", "cells", "area"]);
    assert_eq!(t.rows.len(), 2);
    assert_eq!((t.cell(0, t.rows[0][0]), t.cell(1, t.rows[0][1])), ("A".to_string(), "A".to_string()));
    assert_eq!((t.cell(0, t.rows[1][0]), t.cell(1, t.rows[1][1])), ("B".to_string(), "B".to_string()));
    assert_eq!(t.rows[0][2..4], [1.0, 2.0]);
}

#[test]
fn temporal_min_leaves_weak_objects_unclassified() {
    let (before, after, m) = hand();
    let out = temporal(&before, &after, m, serde_json::json!({ "min": 0.55 })).unwrap();
    // the adjusted memberships 0.8 / 0.4 and 0.5 / 0.6: object 2 is B at 0.6 (>= 0.55), object 1 is A
    assert_eq!(objects_of(&out, "objects").class_of, vec![1, 2]);
    let out = temporal(&before, &after, hand().2, serde_json::json!({ "min": 0.7 })).unwrap();
    assert_eq!(objects_of(&out, "objects").class_of, vec![1, 0]);
    let Value::Map(map) = &out["map"] else { panic!("types") };
    assert!(map.data[2].is_nan());
}

#[test]
fn temporal_both_adjusts_the_first_date() {
    let before = objs(2, 1, vec![1, 1], &["A", "B"], vec![vec![0.9], vec![0.8]]);
    let after = objs(2, 1, vec![1, 1], &["A", "B"], vec![vec![0.1], vec![0.9]]);
    // A -> A 0.2, A -> B 0.3, B -> anything 1
    let m = matrix(&["A", "B"], &[&[0.2, 0.3], &[1.0, 1.0]]);
    let out = temporal(&before, &after, m.clone(), serde_json::json!({})).unwrap();
    assert_eq!(objects_of(&out, "before").class_of, vec![1]);
    let out = temporal(&before, &after, m, serde_json::json!({ "both": true })).unwrap();
    let b = objects_of(&out, "before");
    // A = min(0.9, max(min(0.1,0.2), min(0.9,0.3))) = 0.3; B = min(0.8, max(0.1, 0.9)) = 0.8
    assert!(close(b.membership[0][0], 0.3) && close(b.membership[1][0], 0.8));
    assert_eq!(b.class_of, vec![2]);
}

#[test]
fn temporal_names_missing_from_the_matrix() {
    let (before, after, _) = hand();
    let m = matrix(&["A"], &[&[1.0]]);
    let e = temporal(&before, &after, m, serde_json::json!({})).err().unwrap().to_string();
    assert!(e.contains('B'), "{e}");
}

/// Cell accuracy of a classification of the cells against the truth (class numbers).
fn accuracy(map: &[f64], truth: &[u32]) -> f64 {
    map.iter().zip(truth).filter(|(m, t)| **m == **t as f64).count() as f64 / truth.len() as f64
}

#[test]
fn temporal_gains_accuracy_against_a_known_truth() {
    let n = 20;
    // the truth: quadrants; date 1 A B / C A, date 2 the same but the quadrant B becomes C
    let quad = |r: usize, c: usize| (r >= n / 2) as usize * 2 + (c >= n / 2) as usize;
    let truth1: Vec<u32> = (0..n * n).map(|i| [1, 2, 3, 1][quad(i / n, i % n)]).collect();
    let truth2: Vec<u32> = (0..n * n).map(|i| [1, 3, 3, 1][quad(i / n, i % n)]).collect();
    // the classes 1 A, 2 B, 3 C: A stays, B stays or becomes C, C stays; every other change is impossible
    let m = matrix(&["A", "B", "C"], &[&[1.0, 0.0, 0.0], &[0.0, 1.0, 1.0], &[0.0, 0.0, 1.0]]);
    // the blocks of a side `s`, as objects
    let blocks = |s: usize| -> (Vec<u32>, usize) {
        let per = n / s;
        ((0..n * n).map(|i| ((i / n / s) * per + (i % n) / s + 1) as u32).collect(), per * per)
    };
    // the memberships of objects: the truth of the object's first cell (noise: another class in some objects)
    let make = |s: usize, truth: &[u32], noisy: bool, m: &Table| {
        let (labels, count) = blocks(s);
        let mut first = vec![0usize; count];
        for (i, &l) in labels.iter().enumerate().rev() {
            first[l as usize - 1] = i;
        }
        let mut mem = vec![vec![0.05; count]; 3];
        for j in 0..count {
            let t = truth[first[j]] as usize;
            mem[t - 1][j] = if noisy { 0.5 } else { 0.9 };
            if noisy && j % 3 == 0 {
                // an impossible class from the object's class at date 1, stronger than its truth
                let o = truth1[first[j]] as usize;
                if let Some(bad) = (1..=3).find(|&c| c != t && m.rows[o - 1][c] == 0.0) {
                    mem[bad - 1][j] = 0.7;
                }
            }
        }
        objs(n, n, labels, &["A", "B", "C"], mem)
    };
    let before = make(5, &truth1, false, &m);
    let after = make(2, &truth2, true, &m);
    let out = temporal(&before, &after, m, serde_json::json!({})).unwrap();
    let to_map =
        |o: &Arc<Objects>| -> Vec<f64> {
            o.labels
                .iter()
                .map(|&l| {
                    if l == 0 || o.class_of[l as usize - 1] == 0 { f64::NAN } else { o.class_of[l as usize - 1] as f64 }
                })
                .collect()
        };
    let raw = accuracy(&to_map(&after), &truth2);
    let Value::Map(adj) = &out["map"] else { panic!("types") };
    let fixed = accuracy(&adj.data, &truth2);
    assert!(fixed > raw, "adjusted {fixed} against raw {raw}");
    assert!(fixed > 0.95, "adjusted {fixed}");
}

fn samples(rows: &[(&str, &str)]) -> Table {
    let mut t = Table::new(vec!["before".into(), "after".into()]);
    let mut names: Vec<String> = Vec::new();
    for (a, b) in rows {
        let mut code = |s: &str| match names.iter().position(|x| x == s) {
            Some(k) => (k + 1) as f64,
            None => {
                names.push(s.to_string());
                names.len() as f64
            }
        };
        let row = vec![code(a), code(b)];
        t.rows.push(row);
    }
    t.labels.insert("before".into(), names.clone());
    t.labels.insert("after".into(), names);
    t
}

#[test]
fn transition_matrix_from_samples_by_hand() {
    let s = samples(&[("A", "A"), ("A", "A"), ("A", "A"), ("A", "B"), ("B", "B"), ("B", "B"), ("C", "C")]);
    let out =
        run("objects.transition_matrix", vec![("samples", Value::Table(s.clone()))], serde_json::json!({})).unwrap();
    let Value::Table(m) = &out["matrix"] else { panic!("types") };
    assert_eq!(m.columns, ["class", "A", "B", "C"]);
    assert_eq!(m.labels["class"], ["A", "B", "C"]);
    // A: 3 stay, 1 to B; B: 2 stay; C: 1 stay
    let want = [[1.0, 1.0 / 3.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for (k, row) in want.iter().enumerate() {
        for (c, &w) in row.iter().enumerate() {
            assert!(close(m.rows[k][c + 1], w), "{k} {c}: {}", m.rows[k][c + 1]);
        }
    }
    let out =
        run("objects.transition_matrix", vec![("samples", Value::Table(s))], serde_json::json!({ "floor": 0.05 }))
            .unwrap();
    let Value::Table(m) = &out["matrix"] else { panic!("types") };
    assert!(close(m.rows[1][1], 0.05) && close(m.rows[0][3], 0.05) && close(m.rows[0][1], 1.0));
    // and the matrix is the one objects.temporal reads
    let (before, after, _) = hand();
    let m2 = matrix(&["A", "B"], &[&[1.0, 0.0], &[0.0, 1.0]]);
    assert!(temporal(&before, &after, m2, serde_json::json!({})).is_ok());
}

#[test]
fn transition_matrix_from_change_transitions() {
    let mut t =
        Table::new(["year_from", "year_to", "from", "to", "km2", "cells", "interval"].map(String::from).to_vec());
    t.rows = vec![
        vec![2000.0, 2010.0, 1.0, 1.0, 6.0, 6.0, 10.0],
        vec![2000.0, 2010.0, 1.0, 2.0, 2.0, 2.0, 10.0],
        vec![2000.0, 2010.0, 2.0, 2.0, 4.0, 4.0, 10.0],
    ];
    let out = run("objects.transition_matrix", vec![("transitions", Value::Table(t))], serde_json::json!({})).unwrap();
    let Value::Table(m) = &out["matrix"] else { panic!("types") };
    assert_eq!(m.columns, ["class", "1", "2"]);
    assert!(close(m.rows[0][1], 1.0) && close(m.rows[0][2], 1.0 / 3.0));
    assert!(close(m.rows[1][1], 0.0) && close(m.rows[1][2], 1.0));
}
