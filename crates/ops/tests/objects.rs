//! Objects: `segment.from_map` and `objects.to_map`, and what [`Objects`] holds: each object connected, ids
//! contiguous and in the order of their first cell, the cells, boxes and borders against a map made by hand.

use std::collections::VecDeque;
use std::sync::Arc;

use terraflux_ops::objects::{NO_REGION, ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

const NAN: f64 = f64::NAN;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn map(w: usize, h: usize, data: Vec<f64>) -> Map {
    Map::new(Arc::new(Grid::new(w, h)), data)
}

fn segment(m: &Map, neighbours: u32) -> (Arc<Objects>, Table) {
    let out =
        run("segment.from_map", vec![("map", Value::Map(m.clone()))], serde_json::json!({ "neighbours": neighbours }))
            .unwrap();
    let (Value::Objects(o), Value::Table(t)) = (&out["objects"], &out["table"]) else { panic!("types") };
    (o.clone(), t.clone())
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y || (x.is_nan() && y.is_nan()))
}

/// A map of values 0..3 from a fixed pseudo-random sequence, a few cells without a value.
fn noisy(w: usize, h: usize) -> Map {
    let mut s = 12345u64;
    let data = (0..w * h)
        .map(|_| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            match (s >> 33) % 10 {
                0 => NAN,
                k => (k % 3) as f64,
            }
        })
        .collect();
    map(w, h, data)
}

/// Each object of a map is connected, of one value, and the ids are contiguous in the order of the first cell.
fn check(m: &Map, o: &Objects, neighbours: u32) {
    let (w, h) = (m.grid.width, m.grid.height);
    let around: Vec<(i64, i64)> = (-1..=1)
        .flat_map(|a| (-1..=1).map(move |b| (a, b)))
        .filter(|&(a, b)| (a, b) != (0, 0) && (neighbours == 8 || a == 0 || b == 0))
        .collect();
    let mut next = 1;
    for (i, &l) in o.labels.iter().enumerate() {
        assert_eq!(l == 0, m.data[i].is_nan(), "no data is no object, cell {i}");
        if l == next {
            next += 1;
        } else {
            assert!(l < next, "the ids go in the order of the first cell: {l} at {i}, next {next}");
        }
    }
    assert_eq!(next as usize - 1, o.count);
    for id in 1..=o.count as u32 {
        let cells: Vec<usize> = (0..w * h).filter(|&i| o.labels[i] == id).collect();
        assert_eq!(cells.len() as u32, o.cells[id as usize - 1]);
        let mut seen = vec![false; w * h];
        let mut queue = VecDeque::from([cells[0]]);
        seen[cells[0]] = true;
        let mut reached = 0;
        while let Some(i) = queue.pop_front() {
            reached += 1;
            assert_eq!(m.data[i], m.data[cells[0]], "one value in an object");
            let (r, c) = ((i / w) as i64, (i % w) as i64);
            for &(dr, dc) in &around {
                let (rr, cc) = (r + dr, c + dc);
                if rr < 0 || cc < 0 || rr >= h as i64 || cc >= w as i64 {
                    continue;
                }
                let j = rr as usize * w + cc as usize;
                if o.labels[j] == id && !seen[j] {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        assert_eq!(reached, cells.len(), "object {id} is connected ({neighbours} neighbours)");
    }
}

#[test]
fn every_object_is_connected_and_ids_are_contiguous() {
    let m = noisy(23, 17);
    for n in [4, 8] {
        let (o, t) = segment(&m, n);
        check(&m, &o, n);
        assert_eq!(t.rows.len(), o.count);
        assert_eq!(t.columns, ["id", "value", "cells", "area"]);
    }
    let (four, _) = segment(&m, 4);
    let (eight, _) = segment(&m, 8);
    assert!(eight.count < four.count, "{} {}", eight.count, four.count);
}

/// 4 x 4: the value 1 in an L around the corner, 2 on the right, 3 in the middle.
fn hand() -> Map {
    #[rustfmt::skip]
    let data = vec![
        1.0, 1.0, 2.0, 2.0,
        1.0, 3.0, 3.0, 2.0,
        1.0, 3.0, 3.0, 2.0,
        1.0, 1.0, 2.0, 2.0,
    ];
    map(4, 4, data)
}

#[test]
fn cells_boxes_and_borders_of_a_small_map() {
    let m = hand();
    let (o, t) = segment(&m, 4);
    assert_eq!(o.count, 3);
    assert_eq!(o.cells, vec![6, 6, 4]);
    assert_eq!(o.boxes, vec![[0, 0, 3, 1], [0, 2, 3, 3], [1, 1, 2, 2]]);
    assert_eq!(o.adjacency[0], vec![(2, 2), (3, 4)]);
    assert_eq!(o.adjacency[1], vec![(1, 2), (3, 4)]);
    assert_eq!(o.adjacency[2], vec![(1, 4), (2, 4)]);
    assert_eq!(t.rows, vec![vec![1.0, 1.0, 6.0, 6.0], vec![2.0, 2.0, 6.0, 6.0], vec![3.0, 3.0, 4.0, 4.0]]);
    assert_eq!((o.id_at(0, 0), o.id_at(1, 1), o.id_at(3, 3), o.id_at(9, 9)), (1, 3, 2, 0));
    assert_eq!(o.neighbours_of(3), &[(1, 4), (2, 4)]);
}

#[test]
fn a_region_in_two_parts_is_two_objects() {
    let m = map(3, 1, vec![5.0, 7.0, 5.0]);
    let (o, t) = segment(&m, 4);
    assert_eq!(o.count, 3);
    assert_eq!(t.column("value").unwrap(), vec![5.0, 7.0, 5.0]);
    // the same region by diagonals: 4 neighbours two objects, 8 one
    let d = map(2, 2, vec![1.0, 0.0, 0.0, 1.0]);
    assert_eq!(segment(&d, 4).0.count, 4);
    let (eight, t) = segment(&d, 8);
    assert_eq!(eight.count, 2);
    assert_eq!(t.column("cells").unwrap(), vec![2.0, 2.0]);
    // and the border is by edges only: the two share 4
    assert_eq!(eight.adjacency[0], vec![(2, 4)]);
}

#[test]
fn cells_without_a_value_are_no_object() {
    let m = map(3, 2, vec![1.0, NAN, 1.0, 1.0, 1.0, NAN]);
    let (o, _) = segment(&m, 4);
    assert_eq!(*o.labels, vec![1, 0, 2, 1, 1, 0]);
    assert_eq!(o.count, 2);
    // an all-empty map has no object
    let (none, t) = segment(&map(2, 2, vec![NAN; 4]), 4);
    assert_eq!((none.count, t.rows.len()), (0, 0));
    // from regions: the sentinel
    let o = Objects::from_regions(Arc::new(Grid::new(2, 1)), &[NO_REGION, 3], 4);
    assert_eq!(*o.labels, vec![0, 1]);
}

#[test]
fn map_to_objects_to_map_is_the_map() {
    for m in [hand(), noisy(19, 13)] {
        let (o, _) = segment(&m, 4);
        let out = run("objects.to_map", vec![("objects", Value::Objects(o))], serde_json::json!({ "what": "value" }))
            .unwrap();
        let Value::Map(back) = &out["map"] else { panic!("a map") };
        assert!(same(&back.data, &m.data));
    }
}

#[test]
fn objects_to_map_by_id_class_parent_and_column() {
    let m = hand();
    let (o, _) = segment(&m, 4);
    let to_map = |o: &Arc<Objects>, what: Option<&str>| {
        let params = what.map_or(serde_json::json!({}), |w| serde_json::json!({ "what": w }));
        run("objects.to_map", vec![("objects", Value::Objects(o.clone()))], params).map(|out| {
            let Value::Map(m) = &out["map"] else { panic!("a map") };
            m.data.to_vec()
        })
    };
    // the id is the default
    assert_eq!(to_map(&o, None).unwrap()[..4], [1.0, 1.0, 2.0, 2.0]);
    assert_eq!(to_map(&o, Some("cells")).unwrap()[5], 4.0);
    // classes: object 1 is class 2, object 2 none, object 3 class 1
    let classes = vec![
        ObjectClass { name: "forest".into(), colour: Some([0, 128, 0]) },
        ObjectClass { name: "field".into(), colour: None },
    ];
    let classed = Arc::new((*o).clone().with_classes(classes, vec![2, 0, 1], vec![]).unwrap());
    let got = to_map(&classed, Some("class")).unwrap();
    assert_eq!(got[0], 2.0);
    assert!(got[2].is_nan());
    assert_eq!(got[5], 1.0);
    assert_eq!(classed.class_name(1), Some("field"));
    assert_eq!(classed.class_name(2), None);
    assert_eq!(classed.class_name(3), Some("forest"));
    // a parent level: the objects of `hand` inside two halves
    let halves = map(4, 4, (0..16).map(|i| if i % 4 < 2 { 1.0 } else { 2.0 }).collect());
    let (above, _) = segment(&halves, 4);
    let nested = Arc::new((*o).clone().with_parent(above, vec![1, 2, 1]).unwrap());
    let got = to_map(&nested, Some("parent")).unwrap();
    assert_eq!((got[0], got[2], got[5]), (1.0, 2.0, 1.0));
    assert_eq!(nested.parent_id(2), Some(2));
    // no level above: an error naming the node; an unknown name lists the columns
    let e = to_map(&o, Some("parent")).unwrap_err().to_string();
    assert!(e.contains("\"n\"") && e.contains("no level above"), "{e}");
    let e = to_map(&o, Some("nope")).unwrap_err().to_string();
    assert!(e.contains("\"n\"") && e.contains("id, value, cells, area"), "{e}");
    // a parent that does not exist, and a table of another size, are refused
    assert!((*o).clone().with_parent(nested.clone(), vec![9, 1, 1]).is_err());
    assert!((*o).clone().with_table(Table::new(vec!["id".into()])).is_err());
}

#[test]
fn bytes_and_content_hash_tell_objects_apart() {
    use std::hash::{DefaultHasher, Hasher};
    let print = |o: &Objects| {
        let mut h = DefaultHasher::new();
        o.content_hash(&mut h);
        h.finish()
    };
    let (a, _) = segment(&hand(), 4);
    let (b, _) = segment(&hand(), 4);
    assert_eq!(print(&a), print(&b));
    let (c, _) = segment(&noisy(4, 4), 4);
    assert_ne!(print(&a), print(&c));
    let classed =
        (*a).clone().with_classes(vec![ObjectClass { name: "x".into(), colour: None }], vec![1, 0, 0], vec![]).unwrap();
    assert_ne!(print(&a), print(&classed));
    assert!(format!("{a:?}").starts_with("Objects(3 objects, 4x4"));
    assert!(a.bytes() >= 64);
    assert!(a.describe().starts_with("3 objects on a 4x4 grid, attributes: value, cells, area"));
}

#[test]
fn bytes_round_trip() {
    let (o, _) = segment(&noisy(11, 7), 8);
    let above = Arc::new(Objects::from_regions(o.grid.clone(), &vec![1; 77], 4));
    let parent_of = vec![1; o.count];
    let classes = vec![
        ObjectClass { name: "a".into(), colour: Some([1, 2, 3]) },
        ObjectClass { name: "b, c".into(), colour: None },
    ];
    let class_of: Vec<u32> = (0..o.count).map(|k| (k % 3) as u32).collect();
    let membership = vec![vec![NAN; o.count], vec![0.25; o.count]];
    let mut t = o.table.clone();
    t.labels.insert("value".into(), vec!["x".into()]);
    t.rows[0][1] = NAN;
    let full = (*o)
        .clone()
        .with_table(t)
        .unwrap()
        .with_classes(classes, class_of, membership)
        .unwrap()
        .with_parent(above, parent_of)
        .unwrap();
    let back = Objects::from_bytes(&full.to_bytes().unwrap()).unwrap();
    assert_eq!(*back.labels, *full.labels);
    assert_eq!(
        (back.cells.clone(), back.boxes.clone(), back.adjacency.clone()),
        (full.cells.clone(), full.boxes.clone(), full.adjacency.clone())
    );
    assert_eq!(back.classes, full.classes);
    assert_eq!(back.class_of, full.class_of);
    assert_eq!(back.parent_of, full.parent_of);
    assert_eq!(back.table.labels, full.table.labels);
    assert!(back.table.rows[0][1].is_nan() && same(&back.table.rows[3], &full.table.rows[3]));
    assert!(back.membership[0][0].is_nan() && back.membership[1] == full.membership[1]);
    assert_eq!(back.parent.as_ref().unwrap().count, 1);
    assert_eq!(back.grid, full.grid);
    // a damaged file is an error, not a crash
    let bytes = full.to_bytes().unwrap();
    assert!(Objects::from_bytes(&bytes[..bytes.len() / 2]).is_err());
    assert!(Objects::from_bytes(b"nothing").is_err());
}

fn table_of(out: &Outputs) -> Table {
    let Value::Table(t) = &out["table"] else { panic!("a table") };
    t.clone()
}

fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

fn vector_of(out: &Outputs) -> Arc<Vector> {
    let Value::Vector(v) = &out["vector"] else { panic!("a vector") };
    v.clone()
}

/// The area of a polygon geometry: its outer rings minus its holes.
fn area_of(g: &Geometry) -> f64 {
    let ring = |r: &Vec<[f64; 2]>| -> f64 {
        let n = r.len();
        (0..n).map(|k| r[k][0] * r[(k + 1) % n][1] - r[(k + 1) % n][0] * r[k][1]).sum::<f64>().abs() / 2.0
    };
    g.polygons().iter().map(|p| ring(&p[0]) - p[1..].iter().map(ring).sum::<f64>()).sum()
}

/// A square polygon on the 10 x 10 grid (x to the right from 0, y down from 0): cells from (c0, r0) to (c1, r1).
fn cells_square(c0: f64, r0: f64, c1: f64, r1: f64) -> Geometry {
    Geometry::Polygon(vec![vec![[c0, -r0], [c1, -r0], [c1, -r1], [c0, -r1], [c0, -r0]]])
}

fn from_vector(v: Vector, like: &Map) -> terraflux_ops::Result<Outputs> {
    run(
        "segment.from_vector",
        vec![("vector", Value::Vector(Arc::new(v))), ("like", Value::Map(like.clone()))],
        serde_json::json!({}),
    )
}

fn to_vector(o: &Arc<Objects>, params: serde_json::Value) -> Arc<Vector> {
    vector_of(&run("objects.to_vector", vec![("objects", Value::Objects(o.clone()))], params).unwrap())
}

#[test]
fn chessboard_cuts_squares_clipped_at_the_edges() {
    let m = map(25, 23, vec![1.0; 25 * 23]);
    let out = run("segment.chessboard", vec![("image", Value::Map(m))], serde_json::json!({ "size": 10 })).unwrap();
    let (o, t) = (objects_of(&out), table_of(&out));
    assert_eq!(o.count, 9);
    assert_eq!(o.cells, vec![100, 100, 50, 100, 100, 50, 30, 30, 15]);
    assert_eq!(t.columns, ["id", "cells", "area"]);
    // a column of no data cuts a square in two
    let mut data = vec![1.0; 10 * 10];
    (0..10).for_each(|r| data[r * 10 + 4] = NAN);
    let image = Value::Map(map(10, 10, data));
    let out = run("segment.chessboard", vec![("image", image)], serde_json::json!({ "size": 10 })).unwrap();
    assert_eq!(objects_of(&out).count, 2);
    // a bad size names the node
    let image = Value::Map(map(3, 3, vec![1.0; 9]));
    let e = run("segment.chessboard", vec![("image", image)], serde_json::json!({ "size": 0 })).err().unwrap();
    assert!(e.to_string().contains("\"n\""), "{e}");
}

#[test]
fn quadtree_keeps_even_areas_in_big_squares() {
    // the left half 5.0, the right half noise
    let (w, h) = (32, 32);
    let mut s = 99u64;
    let data: Vec<f64> = (0..w * h)
        .map(|i| {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            if i % w < 16 { 5.0 } else { ((s >> 33) % 100) as f64 }
        })
        .collect();
    let q = |params: serde_json::Value| {
        let out = run("segment.quadtree", vec![("image", Value::Map(map(w, h, data.clone())))], params).unwrap();
        objects_of(&out)
    };
    let o = q(serde_json::json!({ "threshold": 1.0 }));
    for k in 0..o.count {
        let b = o.boxes[k];
        let (bh, bw) = (b[2] - b[0] + 1, b[3] - b[1] + 1);
        assert_eq!(bh, bw, "object {} is a square", k + 1);
        assert_eq!(o.cells[k], bh * bw);
        if b[1] < 16 {
            assert_eq!(bw, 16, "the even half stays in a big square");
        }
    }
    // the noisy half goes down to small squares
    let right = (0..o.count).filter(|&k| o.boxes[k][1] >= 16).count();
    assert!(right > 200, "{right}");
    // the smallest size is kept
    let o = q(serde_json::json!({ "threshold": 1.0, "min_size": 4 }));
    assert!(o.cells.iter().all(|&c| c >= 16), "{:?}", o.cells);
    // a huge threshold: the one square covering the image, or the squares of the largest size
    assert_eq!(q(serde_json::json!({ "threshold": 1e9 })).count, 1);
    assert_eq!(q(serde_json::json!({ "threshold": 1e9, "max_size": 16 })).cells, vec![256; 4]);
    // clipped at the edges: 25 x 23 in squares of 16
    let image = Value::Map(map(25, 23, vec![1.0; 25 * 23]));
    let params = serde_json::json!({ "threshold": 5.0, "max_size": 16 });
    let out = run("segment.quadtree", vec![("image", image)], params).unwrap();
    assert_eq!(objects_of(&out).cells, vec![256, 144, 112, 63]);
    // the threshold is needed
    let image = Value::Map(map(2, 2, vec![1.0; 4]));
    let e = run("segment.quadtree", vec![("image", image)], serde_json::json!({})).err().unwrap().to_string();
    assert!(e.contains("\"n\"") && e.contains("threshold"), "{e}");
}

#[test]
fn from_vector_makes_objects_with_the_attributes() {
    let like = map(10, 10, vec![0.0; 100]);
    let v = Vector {
        geometries: vec![cells_square(0.0, 0.0, 4.0, 4.0), cells_square(5.0, 5.0, 9.0, 9.0)],
        attributes: vec![
            ("height".into(), Column::Numbers(vec![12.5, 3.0])),
            ("kind".into(), Column::Texts(vec!["stand".into(), "meadow".into()])),
        ],
        ..Default::default()
    };
    let out = from_vector(v, &like).unwrap();
    let (o, t) = (objects_of(&out), table_of(&out));
    assert_eq!(o.count, 2);
    assert_eq!(t.columns, ["id", "feature", "cells", "area", "height", "kind"]);
    assert_eq!(t.rows[0][..5], [1.0, 1.0, 16.0, 16.0, 12.5]);
    assert_eq!(t.rows[1][4], 3.0);
    assert_eq!(t.label("kind", t.rows[0][5]), Some("stand"));
    assert_eq!(t.label("kind", t.rows[1][5]), Some("meadow"));
    assert_eq!(o.id_at(9, 9), 0);
    // later features win where they overlap
    let v = Vector {
        geometries: vec![cells_square(0.0, 0.0, 6.0, 6.0), cells_square(4.0, 4.0, 10.0, 10.0)],
        ..Default::default()
    };
    assert_eq!(objects_of(&from_vector(v, &like).unwrap()).cells, vec![32, 36]);
    // a multipolygon of two parts: two objects, one feature
    let two = Geometry::MultiPolygon(vec![
        vec![vec![[0.0, 0.0], [3.0, 0.0], [3.0, -3.0], [0.0, -3.0], [0.0, 0.0]]],
        vec![vec![[6.0, -6.0], [9.0, -6.0], [9.0, -9.0], [6.0, -9.0], [6.0, -6.0]]],
    ]);
    let out = from_vector(Vector { geometries: vec![two], ..Default::default() }, &like).unwrap();
    assert_eq!(objects_of(&out).count, 2);
    assert_eq!(table_of(&out).column("feature").unwrap(), vec![1.0, 1.0]);
    // a line is not a polygon
    let line = Geometry::LineString(vec![[0.0, 0.0], [5.0, -5.0]]);
    let e = from_vector(Vector { geometries: vec![line], ..Default::default() }, &like).err().unwrap().to_string();
    assert!(e.contains("\"n\"") && e.contains("not a polygon"), "{e}");
}

#[test]
fn vector_to_objects_to_vector_keeps_the_areas() {
    let like = map(10, 10, vec![0.0; 100]);
    let v = Vector {
        geometries: vec![cells_square(1.0, 1.0, 5.0, 4.0), cells_square(5.0, 4.0, 9.0, 8.0)],
        ..Default::default()
    };
    let o = objects_of(&from_vector(v, &like).unwrap());
    let back = to_vector(&o, serde_json::json!({}));
    assert_eq!(back.len(), 2);
    for k in 0..2 {
        assert_eq!(area_of(&back.geometries[k]), o.cells[k] as f64, "polygon {}", k + 1);
    }
    assert_eq!(back.column("cells").unwrap().numbers(), vec![12.0, 16.0]);
    // objects -> vector -> objects: the same labels, holes and parts included
    for m in [hand(), noisy(15, 11)] {
        let (o, _) = segment(&m, 4);
        let v = to_vector(&o, serde_json::json!({}));
        assert_eq!(v.len(), o.count);
        for k in 0..o.count {
            assert_eq!(area_of(&v.geometries[k]), o.cells[k] as f64, "polygon {}", k + 1);
        }
        let like = Map::new(o.grid.clone(), vec![0.0; m.data.len()]);
        let again = from_vector((*v).clone(), &like).unwrap();
        assert_eq!(*objects_of(&again).labels, *o.labels);
    }
}

#[test]
fn to_vector_has_the_attributes_and_smooths() {
    // a disc of radius 10 on a background: the background has the disc as a hole
    let n = 25usize;
    let data: Vec<f64> = (0..n * n)
        .map(|i| {
            let (r, c) = ((i / n) as f64 - 12.0, (i % n) as f64 - 12.0);
            if r * r + c * c <= 100.0 { 2.0 } else { 1.0 }
        })
        .collect();
    let (o, _) = segment(&map(n, n, data), 4);
    let classes = vec![ObjectClass { name: "water".into(), colour: None }];
    let o = Arc::new((*o).clone().with_classes(classes, vec![0, 1], vec![]).unwrap());
    let raw = to_vector(&o, serde_json::json!({}));
    assert_eq!(raw.len(), 2);
    let names: Vec<&str> = raw.attributes.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["class", "id", "value", "cells", "area"]);
    let Column::Texts(class) = raw.column("class").unwrap() else { panic!("text") };
    assert_eq!(class, &vec!["".to_string(), "water".to_string()]);
    assert_eq!(raw.geometries[0].polygons()[0].len(), 2);
    let smooth = to_vector(&o, serde_json::json!({ "smooth": 2 }));
    for k in 0..2 {
        let (a, b) = (area_of(&raw.geometries[k]), area_of(&smooth.geometries[k]));
        // (the background's outer ring is the image's square: its corners are cut too, so only the disc is held to it)
        assert!(k == 0 || (a - b).abs() < 0.1 * a, "object {}: {a} {b}", k + 1);
        for p in smooth.geometries[k].polygons() {
            for ring in p {
                assert_eq!(ring.first(), ring.last(), "closed");
                assert!(ring.len() > 8);
            }
        }
    }
}

// segment.multiresolution

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*s >> 11) as f64) / (1u64 << 53) as f64
}

/// A cheap normal-ish noise (sum of 4 uniforms, standard deviation about `sd`).
fn gauss(s: &mut u64, sd: f64) -> f64 {
    ((lcg(s) + lcg(s) + lcg(s) + lcg(s)) - 2.0) * sd * 3f64.sqrt()
}

fn stack(w: usize, h: usize, layers: Vec<Vec<f64>>) -> Value {
    let keys = (0..layers.len()).map(|k| terraflux_ops::Key::Name(format!("b{}", k + 1))).collect();
    let layers = layers.into_iter().map(Arc::new).collect();
    Value::Stack(terraflux_ops::Stack { grid: Arc::new(Grid::new(w, h)), keys, layers })
}

/// Four quadrants of levels 10, 40, 70 and 100 (in each band, shifted by the band) with noise of the given sd.
fn quadrants(w: usize, h: usize, bands: usize, sd: f64, seed: u64) -> Value {
    let mut s = seed;
    let layers = (0..bands)
        .map(|b| {
            (0..w * h)
                .map(|i| {
                    let q = (i / w >= h / 2) as usize * 2 + (i % w >= w / 2) as usize;
                    10.0 + 30.0 * q as f64 + 3.0 * b as f64 + gauss(&mut s, sd)
                })
                .collect()
        })
        .collect();
    stack(w, h, layers)
}

fn multires(image: Value, params: serde_json::Value) -> Outputs {
    run("segment.multiresolution", vec![("image", image)], params).unwrap()
}

/// Each object is one connected set of cells (4 neighbours).
fn connected(o: &Objects) {
    let (w, h) = (o.grid.width, o.grid.height);
    let mut seen = vec![false; w * h];
    let mut comps = vec![0usize; o.count];
    for start in 0..w * h {
        let l = o.labels[start];
        if l == 0 || seen[start] {
            continue;
        }
        comps[l as usize - 1] += 1;
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        while let Some(i) = queue.pop_front() {
            let (r, c) = (i / w, i % w);
            let mut go = |j: usize| {
                if !seen[j] && o.labels[j] == l {
                    seen[j] = true;
                    queue.push_back(j);
                }
            };
            if r > 0 {
                go(i - w);
            }
            if r + 1 < h {
                go(i + w);
            }
            if c > 0 {
                go(i - 1);
            }
            if c + 1 < w {
                go(i + 1);
            }
        }
    }
    assert!(comps.iter().all(|&c| c == 1), "an object in parts");
}

#[test]
fn multiresolution_a_homogeneous_image_is_one_object_per_area() {
    // two areas apart (a column of no data), constant values
    let (w, h) = (31, 20);
    let data: Vec<f64> = (0..w * h).map(|i| if i % w == 15 { NAN } else { 7.0 }).collect();
    for scale in [0.5, 5.0, 100.0] {
        for mutual in [true, false] {
            let out = multires(
                Value::Map(map(w, h, data.clone())),
                serde_json::json!({ "scale": scale, "shape": 0.0, "mutual": mutual }),
            );
            let o = objects_of(&out);
            assert_eq!(o.count, 2, "scale {scale} mutual {mutual}");
            assert_eq!(o.labels.iter().filter(|&&l| l == 0).count(), h);
            assert_eq!(table_of(&out).columns, ["id", "cells", "area", "mean_image", "sd_image"]);
        }
    }
}

#[test]
fn multiresolution_more_scale_gives_fewer_objects() {
    let image = quadrants(60, 60, 3, 12.0, 5);
    let mut last = usize::MAX;
    for scale in [5.0, 10.0, 20.0, 40.0] {
        let o = objects_of(&multires(image.clone(), serde_json::json!({ "scale": scale })));
        println!("scale {scale}: {} objects", o.count);
        assert!(o.count <= last, "scale {scale}: {} after {last}", o.count);
        assert!(o.count >= 1);
        connected(&o);
        last = o.count;
    }
    assert!(last < 60 * 60 / 20, "scale 40 left {last} objects");
}

#[test]
fn multiresolution_is_deterministic_for_a_seed() {
    let image = quadrants(50, 40, 2, 3.0, 9);
    let params = |seed: i64| serde_json::json!({ "scale": 15, "seed": seed });
    let a = objects_of(&multires(image.clone(), params(3)));
    let b = objects_of(&multires(image.clone(), params(3)));
    assert_eq!(a.labels, b.labels);
    assert_eq!(a.table.rows, b.table.rows);
}

#[test]
fn multiresolution_a_step_is_kept_until_the_scale_is_huge() {
    let (w, h) = (20, 20);
    let mut s = 3u64;
    let data: Vec<f64> = (0..w * h).map(|i| if i % w < w / 2 { 10.0 } else { 90.0 } + gauss(&mut s, 0.1)).collect();
    let image = Value::Map(map(w, h, data));
    let two = objects_of(&multires(image.clone(), serde_json::json!({ "scale": 20 })));
    assert_eq!(two.count, 2);
    assert_eq!(two.cells, vec![200, 200]);
    let one = objects_of(&multires(image, serde_json::json!({ "scale": 1000 })));
    assert_eq!(one.count, 1);
}

#[test]
fn multiresolution_from_a_level_makes_the_level_above() {
    let image = quadrants(60, 60, 3, 6.0, 5);
    let low = objects_of(&multires(image.clone(), serde_json::json!({ "scale": 4 })));
    let out = run(
        "segment.multiresolution",
        vec![("image", image), ("from", Value::Objects(low.clone()))],
        serde_json::json!({ "scale": 6 }),
    )
    .unwrap();
    let high = objects_of(&out);
    let Value::Objects(below) = &out["below"] else { panic!("below") };
    println!("from {} objects to {}", low.count, high.count);
    assert!(high.count < low.count, "{} not under {}", high.count, low.count);
    connected(&high);
    assert_eq!(below.count, low.count);
    assert!(below.parent.is_some());
    // each from-object is inside one new object, which is the union of its from-objects
    let mut parent = vec![0u32; low.count];
    for (&a, &b) in low.labels.iter().zip(high.labels.iter()) {
        let p = &mut parent[a as usize - 1];
        assert!(*p == 0 || *p == b, "an object in two");
        *p = b;
    }
    for k in 1..=low.count as u32 {
        assert_eq!(below.parent_id(k), Some(parent[k as usize - 1]));
    }
    let mut cells = vec![0u32; high.count];
    for k in 0..low.count {
        cells[parent[k] as usize - 1] += low.cells[k];
    }
    assert_eq!(cells, high.cells);
}

#[test]
fn multiresolution_needs_a_scale_and_checks_its_settings() {
    let image = || Value::Map(map(4, 4, vec![1.0; 16]));
    let e = run("segment.multiresolution", vec![("image", image())], serde_json::json!({})).err().unwrap();
    assert!(e.to_string().contains("give a scale"), "{e}");
    for p in [
        serde_json::json!({ "scale": 0 }),
        serde_json::json!({ "scale": 5, "shape": 0.95 }),
        serde_json::json!({ "scale": 5, "compactness": 2 }),
        serde_json::json!({ "scale": 5, "weights": [1, 2] }),
    ] {
        assert!(run("segment.multiresolution", vec![("image", image())], p).is_err());
    }
}

/// The under segmentation (share of objects whose main class covers under 90% of them) and the over one (objects
/// per region of the truth) of objects against a map of classes.
fn judge(o: &Objects, truth: &Map) -> (f64, f64) {
    let (tr, _) = segment(truth, 4);
    let classes = truth.data.iter().filter(|v| !v.is_nan()).fold(0.0f64, |a, &b| a.max(b)) as usize + 1;
    let mut count = vec![vec![0u32; classes]; o.count];
    for (&l, &c) in o.labels.iter().zip(truth.data.iter()) {
        if l != 0 && !c.is_nan() {
            count[l as usize - 1][c as usize] += 1;
        }
    }
    let impure = (0..o.count).filter(|&k| (*count[k].iter().max().unwrap() as f64) < 0.9 * o.cells[k] as f64).count();
    (impure as f64 / o.count as f64, o.count as f64 / tr.count as f64)
}

/// Blocks of 30 x 30 cells of five classes, none next to one of its own class.
fn blocky() -> Value {
    let v = (0..120 * 120).map(|i| (((i / 120 / 30) + 2 * ((i % 120) / 30)) % 5 + 1) as f64).collect();
    Value::Map(map(120, 120, v))
}

#[test]
fn multiresolution_on_nlm_image_finds_the_blocks() {
    // (the scale is in reflectance, 0-1, summed over six bands.) Sharp edges: the blocks are found, nothing mixed
    // (measured at scales 4 to 5.5: 16 objects, none mixed); with the default blur the two cells along every edge
    // are a mix of two classes, so some objects are mixed whatever the scale (measured at 5: 20 objects, 4 mixed).
    for (blur, scale, max_under) in [(0, 5.0, 0.02), (1, 5.0, 0.3)] {
        let image = run("nlm.image", vec![("classes", blocky())], serde_json::json!({ "blur": blur })).unwrap();
        let (stack, truth) = (image["image"].clone(), image["truth"].clone());
        let Value::Map(truth) = truth else { panic!("truth") };
        let o = objects_of(&multires(stack, serde_json::json!({ "scale": scale })));
        let (under, over) = judge(&o, &truth);
        println!("blur {blur} scale {scale}: {} objects, under {under:.3}, over {over:.1}", o.count);
        connected(&o);
        assert!(under <= max_under, "blur {blur}: under segmentation {under}");
        assert!(over < 3.0, "blur {blur}: over segmentation {over}");
    }
}

/// 1000 x 1000 cells, 4 bands: the time of a run (cargo test ... --ignored --nocapture).
#[test]
#[ignore]
fn multiresolution_timing_on_a_million_cells() {
    let (w, h) = (1000, 1000);
    let mut s = 1u64;
    let layers = (0..4)
        .map(|b| {
            (0..w * h)
                .map(|i| {
                    let block = (i / w / 50 + 3 * ((i % w) / 50)) % 7;
                    0.05 + 0.1 * block as f64 + 0.01 * b as f64 + gauss(&mut s, 0.02)
                })
                .collect()
        })
        .collect();
    let image = stack(w, h, layers);
    for scale in [0.3, 1.0] {
        let t = std::time::Instant::now();
        let o = objects_of(&multires(image.clone(), serde_json::json!({ "scale": scale })));
        println!("1000 x 1000 x 4 bands, scale {scale}: {} objects in {:.2?}", o.count, t.elapsed());
    }
}

// segment.slic, segment.snic, segment.felzenszwalb

const SUPERPIXELS: [&str; 3] = ["segment.slic", "segment.snic", "segment.felzenszwalb"];

fn superpixels(op: &str, image: Value, params: serde_json::Value) -> Outputs {
    run(op, vec![("image", image)], params).unwrap()
}

/// Two areas, left 10 and right 90, in two bands, a little noise.
fn step_image(w: usize, h: usize) -> Value {
    let mut s = 21u64;
    let layers = (0..2)
        .map(|b| {
            (0..w * h).map(|i| if i % w < w / 2 { 10.0 } else { 90.0 } + 5.0 * b as f64 + gauss(&mut s, 0.5)).collect()
        })
        .collect();
    stack(w, h, layers)
}

#[test]
fn superpixels_are_deterministic_connected_and_keep_the_table() {
    let image = quadrants(70, 50, 3, 8.0, 4);
    for op in SUPERPIXELS {
        let a = superpixels(op, image.clone(), serde_json::json!({}));
        let b = superpixels(op, image.clone(), serde_json::json!({}));
        let (oa, ob) = (objects_of(&a), objects_of(&b));
        assert_eq!(oa.labels, ob.labels, "{op}");
        assert_eq!(table_of(&a).rows, table_of(&b).rows, "{op}");
        connected(&oa);
        let t = table_of(&a);
        assert_eq!(t.columns, ["id", "cells", "area", "mean_b1", "mean_b2", "mean_b3", "sd_b1", "sd_b2", "sd_b3"]);
        assert_eq!(t.rows.len(), oa.count);
        assert_eq!(oa.cells.iter().sum::<u32>(), 70 * 50, "{op}");
        println!("{op}: {} objects", oa.count);
    }
}

#[test]
fn superpixels_leave_cells_without_a_value_out() {
    let (w, h) = (40, 30);
    let Value::Stack(mut s) = quadrants(w, h, 2, 6.0, 8) else { panic!("stack") };
    for layer in s.layers.iter_mut() {
        let mut v = (**layer).clone();
        for r in 0..h {
            v[r * w + 17] = NAN;
        }
        *layer = Arc::new(v);
    }
    // a cell with a value in one band only is still an object
    let mut v = (*s.layers[0]).clone();
    v[5 * w + 17] = 30.0;
    s.layers[0] = Arc::new(v);
    for op in SUPERPIXELS {
        let o = objects_of(&superpixels(op, Value::Stack(s.clone()), serde_json::json!({})));
        for i in 0..w * h {
            assert_eq!(o.labels[i] == 0, i % w == 17 && i / w != 5, "{op}: cell {i}");
        }
        connected(&o);
    }
}

#[test]
fn a_constant_image_is_regular_superpixels_or_one_object() {
    let image = Value::Map(map(60, 60, vec![7.0; 3600]));
    for op in ["segment.slic", "segment.snic"] {
        let o = objects_of(&superpixels(op, image.clone(), serde_json::json!({ "size": 10 })));
        println!("{op} constant: {} objects", o.count);
        assert!((25..=47).contains(&o.count), "{op}: {} objects, 36 expected", o.count);
        let o = objects_of(&superpixels(op, image.clone(), serde_json::json!({ "count": 16 })));
        assert!((11..=21).contains(&o.count), "{op} count 16: {} objects", o.count);
    }
    let o = objects_of(&superpixels("segment.felzenszwalb", image, serde_json::json!({})));
    assert_eq!(o.count, 1);
}

#[test]
fn no_superpixel_straddles_a_step() {
    let (w, h) = (48, 36);
    let image = step_image(w, h);
    for op in SUPERPIXELS {
        let o = objects_of(&superpixels(op, image.clone(), serde_json::json!({})));
        let mut side = vec![None; o.count];
        for (i, &l) in o.labels.iter().enumerate() {
            let left = i % w < w / 2;
            let s = side[l as usize - 1].get_or_insert(left);
            assert_eq!(*s, left, "{op}: object {l} on both sides of the step");
        }
        println!("{op} step: {} objects", o.count);
    }
    // Felzenszwalb finds just the two areas (with the smoothing the blurred columns along the step can stand alone)
    let o = objects_of(&superpixels("segment.felzenszwalb", image, serde_json::json!({ "sigma": 0 })));
    assert_eq!(o.count, 2);
}

#[test]
fn superpixels_check_their_settings() {
    let image = || Value::Map(map(8, 8, vec![1.0; 64]));
    for (op, p) in [
        ("segment.slic", serde_json::json!({ "size": 0 })),
        ("segment.slic", serde_json::json!({ "size": 4, "count": 4 })),
        ("segment.slic", serde_json::json!({ "iterations": 0 })),
        ("segment.snic", serde_json::json!({ "compactness": -1 })),
        ("segment.felzenszwalb", serde_json::json!({ "k": -1 })),
        ("segment.felzenszwalb", serde_json::json!({ "neighbours": 6 })),
        ("segment.felzenszwalb", serde_json::json!({ "min_size": 0 })),
    ] {
        assert!(run(op, vec![("image", image())], p).is_err(), "{op}");
    }
}

#[test]
fn superpixels_on_nlm_image_do_not_mix_classes() {
    let image = run("nlm.image", vec![("classes", blocky())], serde_json::json!({ "blur": 0 })).unwrap();
    let (stack, truth) = (image["image"].clone(), image["truth"].clone());
    let Value::Map(truth) = truth else { panic!("truth") };
    for (op, max_under) in [("segment.felzenszwalb", 0.05), ("segment.slic", 0.1), ("segment.snic", 0.1)] {
        let o = objects_of(&superpixels(op, stack.clone(), serde_json::json!({})));
        let (under, over) = judge(&o, &truth);
        println!("{op}: {} objects, under {under:.3}, over {over:.1}", o.count);
        connected(&o);
        assert!(under <= max_under, "{op}: under segmentation {under}");
    }
}

/// 1000 x 1000 cells, 4 bands: the time of each (cargo test --release ... --ignored --nocapture).
#[test]
#[ignore]
fn superpixels_timing_on_a_million_cells() {
    let (w, h) = (1000, 1000);
    let mut s = 1u64;
    let layers = (0..4)
        .map(|b| {
            (0..w * h)
                .map(|i| {
                    let block = (i / w / 50 + 3 * ((i % w) / 50)) % 7;
                    0.05 + 0.1 * block as f64 + 0.01 * b as f64 + gauss(&mut s, 0.02)
                })
                .collect()
        })
        .collect();
    let image = stack(w, h, layers);
    for op in SUPERPIXELS {
        let t = std::time::Instant::now();
        let o = objects_of(&superpixels(op, image.clone(), serde_json::json!({})));
        println!("{op}, 1000 x 1000 x 4 bands: {} objects in {:.2?}", o.count, t.elapsed());
    }
}

// segment.watershed, segment.meanshift

const OBIA: [&str; 2] = ["segment.watershed", "segment.meanshift"];

#[test]
fn watershed_and_meanshift_are_deterministic_connected_and_keep_the_table() {
    let image = quadrants(70, 50, 3, 8.0, 4);
    for op in OBIA {
        let a = superpixels(op, image.clone(), serde_json::json!({}));
        let b = superpixels(op, image.clone(), serde_json::json!({}));
        let (oa, ob) = (objects_of(&a), objects_of(&b));
        assert_eq!(oa.labels, ob.labels, "{op}");
        assert_eq!(table_of(&a).rows, table_of(&b).rows, "{op}");
        connected(&oa);
        let t = table_of(&a);
        assert_eq!(t.columns, ["id", "cells", "area", "mean_b1", "mean_b2", "mean_b3", "sd_b1", "sd_b2", "sd_b3"]);
        assert_eq!(t.rows.len(), oa.count);
        assert_eq!(oa.cells.iter().sum::<u32>(), 70 * 50, "{op}");
        println!("{op}: {} objects", oa.count);
    }
}

#[test]
fn watershed_and_meanshift_leave_cells_without_a_value_out() {
    let (w, h) = (40, 30);
    let Value::Stack(mut s) = quadrants(w, h, 2, 6.0, 8) else { panic!("stack") };
    for layer in s.layers.iter_mut() {
        let mut v = (**layer).clone();
        for r in 0..h {
            v[r * w + 17] = NAN;
        }
        *layer = Arc::new(v);
    }
    let mut v = (*s.layers[0]).clone();
    v[5 * w + 17] = 30.0;
    s.layers[0] = Arc::new(v);
    for op in OBIA {
        let o = objects_of(&superpixels(op, Value::Stack(s.clone()), serde_json::json!({})));
        for i in 0..w * h {
            assert_eq!(o.labels[i] == 0, i % w == 17 && i / w != 5, "{op}: cell {i}");
        }
        connected(&o);
    }
}

#[test]
fn watershed_and_meanshift_find_the_two_sides_of_a_step() {
    let (w, h) = (48, 36);
    let image = step_image(w, h);
    for op in OBIA {
        let o = objects_of(&superpixels(op, image.clone(), serde_json::json!({})));
        let mut side = vec![None; o.count];
        for (i, &l) in o.labels.iter().enumerate() {
            let left = i % w < w / 2;
            let s = side[l as usize - 1].get_or_insert(left);
            assert_eq!(*s, left, "{op}: object {l} on both sides of the step");
        }
        println!("{op} step: {} objects", o.count);
        assert_eq!(o.count, 2, "{op}");
    }
}

#[test]
fn a_constant_image_is_one_object_for_watershed_and_meanshift() {
    let image = Value::Map(map(60, 50, vec![7.0; 3000]));
    for op in OBIA {
        let o = objects_of(&superpixels(op, image.clone(), serde_json::json!({})));
        assert_eq!(o.count, 1, "{op}");
    }
}

#[test]
fn watershed_with_markers_has_one_object_a_marker() {
    let (w, h) = (60, 40);
    let image = quadrants(w, h, 2, 4.0, 3);
    let mut m = vec![NAN; w * h];
    for (k, &(r, c)) in [(5usize, 5usize), (30, 10), (20, 50)].iter().enumerate() {
        // a marker of 3 cells, one value
        for d in 0..3 {
            m[r * w + c + d] = 10.0 + k as f64;
        }
    }
    let marks = Value::Map(map(w, h, m));
    let o = objects_of(
        &run("segment.watershed", vec![("image", image.clone()), ("markers", marks.clone())], serde_json::json!({}))
            .unwrap(),
    );
    assert_eq!(o.count, 3);
    assert_eq!(o.cells.iter().sum::<u32>() as usize, w * h);
    connected(&o);
    // and with compactness, 4 neighbours
    let o = objects_of(
        &run(
            "segment.watershed",
            vec![("image", image), ("markers", marks)],
            serde_json::json!({ "compactness": 0.05, "neighbours": 4 }),
        )
        .unwrap(),
    );
    assert_eq!(o.count, 3);
    assert_eq!(o.cells.iter().sum::<u32>() as usize, w * h);
}

/// The mean of perimeter² / area (in cells) over the objects.
fn roughness(o: &Objects) -> f64 {
    let (w, h) = (o.grid.width, o.grid.height);
    let mut perimeter = vec![0u32; o.count];
    for i in 0..w * h {
        let l = o.labels[i];
        if l == 0 {
            continue;
        }
        let (r, c) = (i / w, i % w);
        let nb =
            [(r > 0).then(|| i - w), (r + 1 < h).then(|| i + w), (c > 0).then(|| i - 1), (c + 1 < w).then(|| i + 1)];
        perimeter[l as usize - 1] += 4 - nb.iter().flatten().filter(|&&j| o.labels[j] == l).count() as u32;
    }
    (0..o.count).map(|k| (perimeter[k] as f64).powi(2) / o.cells[k] as f64).sum::<f64>() / o.count as f64
}

#[test]
fn watershed_compactness_gives_more_regular_shapes() {
    let (w, h) = (80, 80);
    let mut s = 5u64;
    let image = stack(w, h, vec![(0..w * h).map(|_| gauss(&mut s, 1.0)).collect()]);
    // 25 markers on a lattice: the plain watershed's basins follow the noise, the compact ones the lattice
    let mut m = vec![NAN; w * h];
    for a in 0..5 {
        for b in 0..5 {
            m[(8 + 16 * a) * w + 8 + 16 * b] = (a * 5 + b) as f64;
        }
    }
    let marks = Value::Map(map(w, h, m));
    let rough = |c: f64| {
        let o = objects_of(
            &run(
                "segment.watershed",
                vec![("image", image.clone()), ("markers", marks.clone())],
                serde_json::json!({ "compactness": c, "neighbours": 4 }),
            )
            .unwrap(),
        );
        assert_eq!(o.count, 25, "compactness {c}");
        connected(&o);
        println!("compactness {c}: {} objects, perimeter² / area {:.1}", o.count, roughness(&o));
        roughness(&o)
    };
    let (plain, compact) = (rough(0.0), rough(0.5));
    assert!(compact < plain, "compact {compact} against plain {plain}");
}

#[test]
fn watershed_and_meanshift_on_nlm_image_do_not_mix_classes() {
    let image = run("nlm.image", vec![("classes", blocky())], serde_json::json!({ "blur": 0 })).unwrap();
    let (stack, truth) = (image["image"].clone(), image["truth"].clone());
    let Value::Map(truth) = truth else { panic!("truth") };
    for op in OBIA {
        let o = objects_of(&superpixels(op, stack.clone(), serde_json::json!({})));
        let (under, over) = judge(&o, &truth);
        println!("{op}: {} objects, under {under:.3}, over {over:.1}", o.count);
        connected(&o);
        assert!(under <= 0.1, "{op}: under segmentation {under}");
    }
}

#[test]
fn watershed_and_meanshift_check_their_settings() {
    let image = || Value::Map(map(8, 8, vec![1.0; 64]));
    for (op, p) in [
        ("segment.watershed", serde_json::json!({ "sigma": -1 })),
        ("segment.watershed", serde_json::json!({ "h": -1 })),
        ("segment.watershed", serde_json::json!({ "compactness": -1 })),
        ("segment.watershed", serde_json::json!({ "neighbours": 6 })),
        ("segment.meanshift", serde_json::json!({ "spatial_radius": 0 })),
        ("segment.meanshift", serde_json::json!({ "range_radius": 0 })),
        ("segment.meanshift", serde_json::json!({ "max_iterations": 0 })),
        ("segment.meanshift", serde_json::json!({ "min_size": 0 })),
    ] {
        assert!(run(op, vec![("image", image())], p).is_err(), "{op}");
    }
}

/// 1000 x 1000 cells, 4 bands: the time of each (cargo test --release ... --ignored --nocapture).
#[test]
#[ignore]
fn watershed_and_meanshift_timing_on_a_million_cells() {
    let (w, h) = (1000, 1000);
    let mut s = 1u64;
    let layers = (0..4)
        .map(|b| {
            (0..w * h)
                .map(|i| {
                    let block = (i / w / 50 + 3 * ((i % w) / 50)) % 7;
                    0.05 + 0.1 * block as f64 + 0.01 * b as f64 + gauss(&mut s, 0.02)
                })
                .collect()
        })
        .collect();
    let image = stack(w, h, layers);
    for op in OBIA {
        let t = std::time::Instant::now();
        let o = objects_of(&superpixels(op, image.clone(), serde_json::json!({})));
        println!("{op}, 1000 x 1000 x 4 bands: {} objects in {:.2?}", o.count, t.elapsed());
    }
}

// segment.scale (ESP)

/// Blocks of 10 cells (contrast 12) inside blocks of 40 (contrast 40), noise of sd 2: two sizes of object.
fn two_sizes(w: usize, h: usize, bands: usize) -> Value {
    let mut s = 21u64;
    let layers = (0..bands)
        .map(|b| {
            (0..w * h)
                .map(|i| {
                    let (r, c) = (i / w, i % w);
                    let big = ((r / 40 + c / 40) % 2) as f64 * 40.0;
                    let small = ((r / 10 + c / 10) % 2) as f64 * 12.0;
                    big + small + 2.0 * b as f64 + gauss(&mut s, 2.0)
                })
                .collect()
        })
        .collect();
    stack(w, h, layers)
}

fn esp(image: Value, params: serde_json::Value) -> Outputs {
    run("segment.scale", vec![("image", image)], params).unwrap()
}

fn named_table(out: &Outputs, name: &str) -> Table {
    let Value::Table(t) = &out[name] else { panic!("{name}") };
    t.clone()
}

fn named_number(out: &Outputs, name: &str) -> f64 {
    let Value::Number(x) = &out[name] else { panic!("{name}") };
    *x
}

#[test]
fn scale_peaks_where_the_blocks_are_whole() {
    let out = esp(two_sizes(120, 120, 2), serde_json::json!({ "start": 1, "step": 0.5, "levels": 90 }));
    let t = named_table(&out, "table");
    let sc = named_table(&out, "scales");
    let col = |n: &str| t.columns.iter().position(|c| c == n).unwrap();
    // the suggested scales are rows of the table
    let flagged: Vec<f64> = t.rows.iter().filter(|r| r[col("suggested")] == 1.0).map(|r| r[col("scale")]).collect();
    assert_eq!(flagged, sc.rows.iter().map(|r| r[1]).collect::<Vec<_>>());
    assert!(!flagged.is_empty() && flagged.len() <= 3);
    // the first one is a level where the small blocks (144) are whole, near that count
    let first = t.rows.iter().find(|r| r[col("suggested")] == 1.0).unwrap();
    let n = first[col("objects")];
    assert!((100.0..=250.0).contains(&n), "{n} objects at the first suggestion");
    // and the big blocks (9) are whole at a later one
    assert!(t.rows.iter().any(|r| r[col("suggested")] == 1.0 && r[col("objects")] == 9.0));
    assert_eq!(named_number(&out, "scale"), flagged[0]);
    // the ROC of the first level is none, and the objects only decrease
    assert!(t.rows[0][col("roc_mean")].is_nan());
    assert!(t.rows.windows(2).all(|p| p[1][col("objects")] <= p[0][col("objects")]));
}

#[test]
fn scale_is_deterministic_and_the_window_limits_the_cells() {
    let image = two_sizes(120, 120, 1);
    let a = named_table(&esp(image.clone(), serde_json::json!({ "levels": 8 })), "table");
    let b = named_table(&esp(image.clone(), serde_json::json!({ "levels": 8 })), "table");
    assert_eq!(a.rows.len(), b.rows.len());
    assert!(a.rows.iter().zip(&b.rows).all(|(x, y)| same(x, y)));
    let w = named_table(&esp(image.clone(), serde_json::json!({ "levels": 8, "window": 30 })), "table");
    assert!(w.rows[0][1] <= 900.0, "{} objects on a window of 900 cells", w.rows[0][1]);
    assert!(a.rows[0][1] > 900.0, "{} objects on the whole image", a.rows[0][1]);
    // a window of 0 is the whole image
    let all = named_table(&esp(image, serde_json::json!({ "levels": 8, "window": 0 })), "table");
    assert!(same(&all.rows[0], &a.rows[0]));
}

#[test]
fn multiresolution_without_a_scale_uses_the_one_of_segment_scale() {
    let image = two_sizes(80, 80, 2);
    let suggested = named_number(&esp(image.clone(), serde_json::json!({})), "scale");
    let r = Registry::standard();
    let params = serde_json::Map::new();
    let args = Args { node: "n", inputs: [("image".to_string(), image.clone())].into(), params: &params };
    let mut ctx = Ctx::default();
    let out = r.get("segment.multiresolution").unwrap().run(&mut ctx, &args).unwrap();
    assert_eq!(named_number(&out, "scale"), suggested);
    assert!(ctx.warnings.iter().any(|w| w.contains("no scale given")), "{:?}", ctx.warnings);
    let with = multires(image, serde_json::json!({ "scale": suggested }));
    assert_eq!(objects_of(&out).labels, objects_of(&with).labels);
    assert!(ctx.warnings.len() == 1 && named_number(&with, "scale") == suggested);
}

#[test]
fn scale_checks_its_settings() {
    let image = || two_sizes(20, 20, 1);
    for p in [
        serde_json::json!({ "start": 0 }),
        serde_json::json!({ "step": -1 }),
        serde_json::json!({ "levels": 2 }),
        serde_json::json!({ "shape": 1 }),
        serde_json::json!({ "window": -1 }),
        serde_json::json!({ "weights": [1, 2] }),
    ] {
        assert!(run("segment.scale", vec![("image", image())], p).is_err());
    }
    // an image with no variation has no default start, but a given one works
    let flat = Value::Map(map(12, 12, vec![3.0; 144]));
    assert!(run("segment.scale", vec![("image", flat.clone())], serde_json::json!({})).is_err());
    assert!(run("segment.scale", vec![("image", flat)], serde_json::json!({ "start": 1 })).is_ok());
}

/// 1000 x 1000 cells, 4 bands, window 512: the time of ESP (cargo test --release ... --ignored --nocapture).
#[test]
#[ignore]
fn scale_timing_on_a_million_cells() {
    let image = two_sizes(1000, 1000, 4);
    let t = std::time::Instant::now();
    let out = esp(image, serde_json::json!({ "window": 512 }));
    println!(
        "segment.scale, 1000 x 1000 x 4 bands, window 512: scale {} in {:.2?}",
        named_number(&out, "scale"),
        t.elapsed()
    );
}
