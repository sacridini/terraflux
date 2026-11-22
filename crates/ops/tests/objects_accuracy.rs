//! `objects.accuracy` and `objects.sample`: six objects of known mapped and reference classes against the confusion
//! matrix and accuracies worked out by hand, and the sample's allocation, determinism and points.

use std::sync::Arc;

use terraflux_ops::accuracy::{olofsson, olofsson_objects};
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
    // `area`: each sampled object counts once, as Olofsson's pixels
    let out = run(
        "objects.accuracy",
        vec![("objects", Value::Objects(objects(MAPPED))), ("samples", points(&REFERENCE))],
        serde_json::json!({ "design": "area" }),
    )
    .unwrap();
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

/// 600 objects of one cell and 400 of nine on a row of 4200 cells, all mapped A; the reference is water for the small
/// objects of even id and the big ones of id divisible by 10: 300 + 360 = 660 of 4200 cells, 340 of 1000 objects.
fn mixed() -> (Arc<Objects>, Vec<usize>, Vec<bool>) {
    let mut labels = Vec::new();
    let mut size = Vec::new();
    for id in 1..=1000u32 {
        let n = if id <= 600 { 1 } else { 9 };
        labels.extend(std::iter::repeat_n(id, n));
        size.push(n);
    }
    let o = Objects::from_labels(Arc::new(Grid::new(4200, 1)), labels, 1000).unwrap();
    let classes = vec![ObjectClass { name: "A".into(), colour: None }];
    let o = Arc::new(o.with_classes(classes, vec![1; 1000], Vec::new()).unwrap());
    let water = (1..=1000).map(|id| if id <= 600 { id % 2 == 0 } else { id % 10 == 0 }).collect();
    (o, size, water)
}

const TRUE_WATER: f64 = 660.0 / 4200.0;

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545F4914F6CDD1D)
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() >> 11) as usize % n
    }
}

/// The reference table of the objects of `ids` (1-based), with their true class.
fn reference_of(ids: &[usize], water: &[bool]) -> Value {
    let mut r = Table::new(vec!["id".into(), "class".into()]);
    r.rows = ids.iter().map(|&i| vec![i as f64, if water[i - 1] { 2.0 } else { 1.0 }]).collect();
    r.labels.insert("class".into(), ["land", "water"].map(String::from).to_vec());
    Value::Table(r)
}

/// The estimated share of water (`area_share`) and the half-width of its interval, by `design`.
fn water_share(o: &Arc<Objects>, reference: Value, design: &str) -> (f64, f64) {
    let out = run(
        "objects.accuracy",
        vec![("objects", Value::Objects(o.clone())), ("reference", reference)],
        serde_json::json!({ "design": design }),
    )
    .unwrap();
    let t = table_of(&out, "table");
    let k = t.rows.iter().position(|r| t.cell(0, r[0]) == "water").unwrap();
    (t.column("area_share").unwrap()[k], t.column("area_cells_ci").unwrap()[k] / 4200.0)
}

#[test]
fn objects_drawn_with_the_same_chance_need_the_design_objects() {
    let (o, _, water) = mixed();
    let (mut by_objects, mut by_area, mut covered) = (0.0, 0.0, 0);
    let runs = 300;
    for seed in 0..runs {
        let mut rng = Rng(seed as u64 * 7919 + 12345);
        // 60 distinct objects, each with the same chance
        let mut ids: Vec<usize> = (1..=1000).collect();
        for k in 0..60 {
            let j = k + rng.below(1000 - k);
            ids.swap(k, j);
        }
        let (a, ci) = water_share(&o, reference_of(&ids[..60], &water), "objects");
        by_objects += a;
        covered += usize::from((a - TRUE_WATER).abs() <= ci);
        by_area += water_share(&o, reference_of(&ids[..60], &water), "area").0;
    }
    let runs = runs as f64;
    assert!((by_objects / runs - TRUE_WATER).abs() < 0.01, "objects: {}", by_objects / runs);
    // each object counted once says about a third of the area is water (340 of 1000 objects)
    assert!((by_area / runs - 0.34).abs() < 0.02 && by_area / runs - TRUE_WATER > 0.1, "area: {}", by_area / runs);
    let cover = covered as f64 / runs;
    assert!((0.89..=0.995).contains(&cover), "the 95% interval covers the truth {cover} of the time");
}

#[test]
fn objects_drawn_by_area_need_the_design_area() {
    let (o, size, water) = mixed();
    let (mut by_objects, mut by_area) = (0.0, 0.0);
    let runs = 300;
    for seed in 0..runs {
        let mut rng = Rng(seed as u64 * 104729 + 99);
        // points at random over the cells until 60 distinct objects are hit: a chance proportional to the area
        let mut ids: Vec<usize> = Vec::new();
        while ids.len() < 60 {
            let cell = rng.below(4200);
            let id = if cell < 600 { cell + 1 } else { 601 + (cell - 600) / 9 };
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        assert!(ids.iter().all(|&i| size[i - 1] > 0));
        by_objects += water_share(&o, reference_of(&ids, &water), "objects").0;
        by_area += water_share(&o, reference_of(&ids, &water), "area").0;
    }
    let runs = runs as f64;
    assert!((by_area / runs - TRUE_WATER).abs() < 0.02, "area: {}", by_area / runs);
    assert!((by_objects / runs - TRUE_WATER).abs() > 0.03, "objects: {}", by_objects / runs);
}

#[test]
fn the_design_objects_is_olofsson_for_equal_units_and_zero_for_all_of_them() {
    let (map, reference) = ([1.0, 1.0, 1.0, 2.0, 2.0, 2.0], [1.0, 1.0, 2.0, 2.0, 2.0, 1.0]);
    let a = olofsson(&map, &reference, &[1.0, 2.0], &[3e12, 1e12]).unwrap();
    // equal units of area 1, strata of trillions of them: no finite population correction to speak of
    let b = olofsson_objects(&map, &reference, &[1.0; 6], &[1.0, 2.0], &[3e12, 1e12], &[3e12, 1e12]).unwrap();
    for j in 0..2 {
        assert!(near(a.area[j], b.area[j]) && near(a.user[j], b.user[j]) && near(a.producer[j], b.producer[j]));
        assert!((a.area_se[j] - b.area_se[j]).abs() < 1e-9, "area se {j}");
        assert!((a.user_se[j] - b.user_se[j]).abs() < 1e-9, "user se {j}");
        assert!((a.producer_se[j] - b.producer_se[j]).abs() < 1e-9, "producer se {j}");
    }
    assert!((a.overall_se - b.overall_se).abs() < 1e-9);
    // all the objects of both strata sampled: nothing left to estimate
    let c = olofsson_objects(&map, &reference, &[1.0; 6], &[1.0, 2.0], &[3.0, 3.0], &[3.0, 3.0]).unwrap();
    assert_eq!((c.overall_se, c.area_se[0], c.producer_se[0]), (0.0, 0.0, 0.0));
}

#[test]
fn the_design_is_checked_and_points_are_guessed() {
    let objs = || ("objects", Value::Objects(objects(MAPPED)));
    assert!(
        run(
            "objects.accuracy",
            vec![objs(), ("samples", points(&REFERENCE))],
            serde_json::json!({ "design": "pixels" })
        )
        .is_err()
    );
    // one point per object: taken as objects, with a warning
    let mut ctx = Ctx::default();
    let params = serde_json::Map::new();
    let args = Args {
        node: "n",
        inputs: vec![objs(), ("samples", points(&REFERENCE))].into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
        params: &params,
    };
    Registry::standard().get("objects.accuracy").unwrap().run(&mut ctx, &args).unwrap();
    assert!(ctx.warnings.iter().any(|w| w.contains("design not set")));
}

#[test]
fn training_objects_and_their_neighbours_among_the_reference_are_counted_and_warned_of() {
    let go = |training: Value| {
        let mut ctx = Ctx::default();
        let r = Registry::standard();
        let params = serde_json::Map::new();
        let args = Args {
            node: "n",
            inputs: [
                ("objects".to_string(), Value::Objects(objects(MAPPED))),
                ("samples".to_string(), points(&REFERENCE)),
                ("training".to_string(), training),
            ]
            .into_iter()
            .collect(),
            params: &params,
        };
        let out = r.get("objects.accuracy").unwrap().run(&mut ctx, &args).unwrap();
        (table_of(&out, "overall"), ctx.warnings)
    };
    // object 1 is trained on: it is a reference object, and objects 2 and 4 touch it
    let (o, warnings) = go(Value::Objects(objects([1, 0, 0, 0, 0, 0])));
    assert_eq!(o.column("training_objects").unwrap(), [1.0]);
    assert_eq!(o.column("touching_training").unwrap(), [2.0]);
    assert!(
        warnings.iter().any(|w| w.contains("1 of the 6 reference objects are training objects and 2 more touch")),
        "{warnings:?}"
    );
    // objects 1 and 6 by a table of ids: 2 and 4 touch 1, 3 and 5 touch 6
    let mut ids = Table::new(vec!["id".into(), "class".into()]);
    ids.rows = vec![vec![1.0, 1.0], vec![6.0, 3.0]];
    let (o, _) = go(Value::Table(ids));
    assert_eq!(o.column("training_objects").unwrap(), [2.0]);
    assert_eq!(o.column("touching_training").unwrap(), [4.0]);
    // without `training` the columns are not there
    let out = accuracy(vec![("objects", Value::Objects(objects(MAPPED))), ("samples", points(&REFERENCE))]);
    assert!(!table_of(&out, "overall").columns.contains(&"training_objects".to_string()));
    // an id that is no object is refused
    let mut bad = Table::new(vec!["id".into()]);
    bad.rows = vec![vec![9.0]];
    let r = run(
        "objects.accuracy",
        vec![
            ("objects", Value::Objects(objects(MAPPED))),
            ("samples", points(&REFERENCE)),
            ("training", Value::Table(bad)),
        ],
        serde_json::json!({}),
    );
    assert!(r.map(|_| ()).unwrap_err().to_string().contains("no object"));
}
