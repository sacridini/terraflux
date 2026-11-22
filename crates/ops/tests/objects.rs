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
fn felzenszwalb_enforces_min_size_except_in_cut_off_areas() {
    let (w, h) = (90, 70);
    let image = quadrants(w, h, 3, 8.0, 4);
    for nb in [4, 8] {
        for min_size in [20, 60] {
            let p = serde_json::json!({ "k": 2, "min_size": min_size, "neighbours": nb });
            let a = run_on(1, "segment.felzenszwalb", vec![("image", image.clone())], p.clone());
            let b = run_on(8, "segment.felzenszwalb", vec![("image", image.clone())], p);
            let o = objects_of(&a);
            assert_eq!(o.labels, objects_of(&b).labels, "threads, {nb} neighbours");
            connected(&o);
            assert!(o.count > 1);
            assert!(
                o.cells.iter().all(|&c| c as usize >= min_size),
                "{nb} nb, min {min_size}: {:?}",
                o.cells.iter().min()
            );
        }
    }
    // zones of 17 columns: the last, 5 columns of 70 cells, is above min_size 20; zones of 3 columns (210 cells) too,
    // and a zone of 1 column x 8 rows (8 cells) stays one object under it
    let zones =
        map(w, h, (0..w * h).map(|i| if i % w == 0 && i / w < 8 { 9.0 } else { ((i % w) / 17) as f64 }).collect());
    let out = run(
        "segment.felzenszwalb",
        vec![("image", image), ("bounds", Value::Map(zones))],
        serde_json::json!({ "k": 2 }),
    )
    .unwrap();
    let o = objects_of(&out);
    connected(&o);
    let small: Vec<u32> = o.cells.iter().copied().filter(|&c| c < 20).collect();
    assert_eq!(small, [8], "only the cut-off zone is under min_size");
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
    assert!(
        ctx.warnings.iter().filter(|w| w.contains("no scale given")).count() == 1
            && named_number(&with, "scale") == suggested
    );
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

// ---------------------------------------------------------------------------
// objects.features: spectral and shape

fn feats(
    o: &Arc<Objects>,
    image: Value,
    more: Option<Value>,
    params: serde_json::Value,
) -> terraflux_ops::Result<(Arc<Objects>, Table)> {
    let mut inputs = vec![("objects", Value::Objects(o.clone())), ("image", image)];
    inputs.extend(more.map(|m| ("more", m)));
    let out = run("objects.features", inputs, params)?;
    let (Value::Objects(o), Value::Table(t)) = (&out["objects"], &out["table"]) else { panic!("types") };
    assert_eq!(o.table.columns, t.columns);
    assert!(o.table.rows.iter().zip(&t.rows).all(|(a, b)| same(a, b)), "the objects' table is the table");
    Ok((o.clone(), t.clone()))
}

fn col(t: &Table, name: &str) -> Vec<f64> {
    t.column(name).unwrap_or_else(|| panic!("no column {name}: {:?}", t.columns))
}

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} against {b}");
}

/// One object of the cells (row, column) in a w x h grid, the rest no data (8 neighbours: one object even when the
/// cells touch by a corner).
fn blob(w: usize, h: usize, cells: &[(usize, usize)]) -> Arc<Objects> {
    let mut data = vec![NAN; w * h];
    for &(r, c) in cells {
        data[r * w + c] = 1.0;
    }
    let (o, _) = segment(&map(w, h, data), 8);
    assert_eq!(o.count, 1);
    o
}

fn shape_table(o: &Arc<Objects>) -> Table {
    let (w, h) = (o.grid.width, o.grid.height);
    feats(o, Value::Map(map(w, h, vec![1.0; w * h])), None, serde_json::json!({ "features": ["shape"] })).unwrap().1
}

fn block(r: std::ops::Range<usize>, c: std::ops::Range<usize>) -> Vec<(usize, usize)> {
    r.flat_map(|r| c.clone().map(move |c| (r, c))).collect()
}

#[test]
fn shape_of_a_rectangle_by_hand() {
    // 2 rows x 3 columns at rows 1-2, columns 2-4 of a 6 x 8 grid
    let t = shape_table(&blob(8, 6, &block(1..3, 2..5)));
    let g = |n: &str| col(&t, n)[0];
    let tol = 1e-9;
    near(g("area"), 6.0, tol, "area");
    near(g("perimeter"), 10.0, tol, "perimeter");
    near(g("length"), 3.0, tol, "length");
    near(g("width"), 2.0, tol, "width");
    near(g("length_width"), 1.5, tol, "length_width");
    near(g("compactness"), 1.0, tol, "compactness");
    near(g("shape_index"), 10.0 / (4.0 * 6f64.sqrt()), tol, "shape_index");
    near(g("border_index"), 1.0, tol, "border_index");
    // axes of the area: variances 9/12 and 4/12, so minor / major = 2 / 3
    near(g("asymmetry"), 1.0 - 2.0 / 3.0, tol, "asymmetry");
    near(g("direction"), 0.0, tol, "direction");
    near(g("rectangular_fit"), 1.0, tol, "rectangular_fit");
    near(g("elliptic_fit"), 1.0, tol, "elliptic_fit");
    // the centres' variances are 2/3 (x) and 1/4 (y)
    near(g("density"), 6f64.sqrt() / (1.0 + (2.0f64 / 3.0 + 0.25).sqrt()), tol, "density");
    // x east: the columns' mean 3 + 0.5; y north: the rows' mean 1.5 + 0.5 down from the top (y = 0)
    near(g("centroid_x"), 3.5, tol, "centroid_x");
    near(g("centroid_y"), -2.0, tol, "centroid_y");
}

#[test]
fn shape_of_a_tall_rectangle_points_up() {
    // 4 rows x 1 column: the major axis is vertical, 90 degrees
    let t = shape_table(&blob(5, 6, &block(1..5, 2..3)));
    near(col(&t, "direction")[0], 90.0, 1e-9, "direction");
    near(col(&t, "length")[0], 4.0, 1e-9, "length");
    near(col(&t, "width")[0], 1.0, 1e-9, "width");
    // variances 16/12 and 1/12: minor / major = 1 / 4
    near(col(&t, "asymmetry")[0], 0.75, 1e-9, "asymmetry");
}

#[test]
fn shape_of_an_l_by_hand() {
    // column 0 rows 0-2 and the cell (2, 1), in the corner of the image: the image's edge is border
    let t = shape_table(&blob(4, 4, &[(0, 0), (1, 0), (2, 0), (2, 1)]));
    let g = |n: &str| col(&t, n)[0];
    near(g("area"), 4.0, 1e-9, "area");
    near(g("perimeter"), 10.0, 1e-9, "perimeter");
    // the smallest rectangle is the 3 x 2 around it (the one along the slanted side of the hull is 8)
    near(g("length"), 3.0, 1e-9, "length");
    near(g("width"), 2.0, 1e-9, "width");
    near(g("compactness"), 6.0 / 4.0, 1e-9, "compactness");
    near(g("border_index"), 1.0, 1e-9, "border_index");
    // the cells' covariance (columns x, rows down so y = -row), as an area (+ 1/12)
    let (vx, vy, cxy) = (0.1875 + 1.0 / 12.0, 0.6875 + 1.0 / 12.0, -0.1875);
    let half = (((vx - vy) / 2.0f64).powi(2) + cxy * cxy).sqrt();
    let (l1, l2) = ((vx + vy) / 2.0 + half, (vx + vy) / 2.0 - half);
    near(g("asymmetry"), 1.0 - (l2 / l1).sqrt(), 1e-9, "asymmetry");
    near(g("asymmetry"), 0.5, 1e-9, "asymmetry by hand");
    let theta = 0.5 * (2.0 * cxy).atan2(vx - vy);
    near(g("direction"), theta.to_degrees().rem_euclid(180.0), 1e-9, "direction");
    near(g("direction"), 108.43494882292201, 1e-9, "direction by hand");
    let fit = g("rectangular_fit");
    assert!(fit > 0.3 && fit <= 1.0, "rectangular_fit {fit}");
    let fit = g("elliptic_fit");
    assert!(fit > 0.3 && fit <= 1.0, "elliptic_fit {fit}");
}

#[test]
fn shape_of_a_single_cell() {
    let t = shape_table(&blob(3, 3, &[(1, 1)]));
    let g = |n: &str| col(&t, n)[0];
    for (name, want) in [
        ("area", 1.0),
        ("perimeter", 4.0),
        ("length", 1.0),
        ("width", 1.0),
        ("length_width", 1.0),
        ("compactness", 1.0),
        ("border_index", 1.0),
        ("asymmetry", 0.0),
        ("direction", 0.0),
        ("rectangular_fit", 1.0),
        ("elliptic_fit", 1.0),
        ("density", 1.0),
        ("centroid_x", 1.5),
        ("centroid_y", -1.5),
    ] {
        near(g(name), want, 1e-9, name);
    }
}

#[test]
fn shape_is_in_map_units() {
    // cells 10 wide and 5 tall, origin (1000, 2000): a 2 x 3 rectangle is 30 x 10 map units
    let mut grid = Grid::new(8, 6);
    grid.transform = [1000.0, 10.0, 0.0, 2000.0, 0.0, -5.0];
    let grid = Arc::new(grid);
    let mut data = vec![NAN; 48];
    for (r, c) in block(1..3, 2..5) {
        data[r * 8 + c] = 1.0;
    }
    let (o, _) = segment(&Map::new(grid.clone(), data), 4);
    let image = Value::Map(Map::new(grid, vec![1.0; 48]));
    let (_, t) = feats(&o, image, None, serde_json::json!({ "features": ["shape"] })).unwrap();
    let g = |n: &str| col(&t, n)[0];
    near(g("area"), 6.0 * 50.0, 1e-9, "area");
    near(g("perimeter"), 2.0 * (3.0 * 10.0 + 2.0 * 5.0), 1e-9, "perimeter");
    near(g("length"), 30.0, 1e-9, "length");
    near(g("width"), 10.0, 1e-9, "width");
    near(g("direction"), 0.0, 1e-9, "direction");
    near(g("rectangular_fit"), 1.0, 1e-9, "rectangular_fit");
    near(g("centroid_x"), 1000.0 + 10.0 * 3.5, 1e-9, "centroid_x");
    near(g("centroid_y"), 2000.0 - 5.0 * 2.0, 1e-9, "centroid_y");
}

#[test]
fn shape_of_a_diagonal_rectangle() {
    // 40 x 12 cells long along the north-east diagonal (x east, y north): direction 45
    let (w, h) = (90, 90);
    let mut cells = Vec::new();
    for r in 0..h {
        for c in 0..w {
            let (x, y) = (c as f64 - 45.0, -(r as f64 - 45.0));
            let (u, v) = ((x + y) / 2f64.sqrt(), (y - x) / 2f64.sqrt());
            if u.abs() <= 20.0 && v.abs() <= 6.0 {
                cells.push((r, c));
            }
        }
    }
    let t = shape_table(&blob(w, h, &cells));
    let g = |n: &str| col(&t, n)[0];
    println!("diagonal: length {} width {} direction {}", g("length"), g("width"), g("direction"));
    near(g("direction"), 45.0, 5.0, "direction");
    // the corners of the cells add up to 1.4 cells to each side
    near(g("length"), 40.0, 2.0, "length");
    near(g("width"), 12.0, 2.0, "width");
    near(g("length_width"), 40.0 / 12.0, 0.6, "length_width");
    // the mirror image (north-west) is 135
    let mirrored: Vec<(usize, usize)> = cells.iter().map(|&(r, c)| (r, w - 1 - c)).collect();
    near(col(&shape_table(&blob(w, h, &mirrored)), "direction")[0], 135.0, 5.0, "mirrored direction");
}

fn named_stack(w: usize, h: usize, bands: Vec<(&str, Vec<f64>)>) -> Value {
    let keys = bands.iter().map(|(n, _)| terraflux_ops::Key::Name(n.to_string())).collect();
    let layers = bands.into_iter().map(|(_, d)| Arc::new(d)).collect();
    Value::Stack(terraflux_ops::Stack { grid: Arc::new(Grid::new(w, h)), keys, layers })
}

/// One object of 5 cells in a row; the first band has 4 values and a cell without one.
fn five(names: [&str; 3]) -> (Arc<Objects>, Value) {
    let (o, _) = segment(&map(5, 1, vec![1.0; 5]), 4);
    let image = named_stack(
        5,
        1,
        vec![
            (names[0], vec![1.0, 2.0, NAN, 6.0, 7.0]),
            (names[1], vec![5.0, 7.0, 8.0, 9.0, 11.0]),
            (names[2], vec![2.0, 2.0, 3.0, 4.0, 4.0]),
        ],
    );
    (o, image)
}

#[test]
fn spectral_of_an_object_by_hand() {
    let (o, image) = five(["B4", "B8", "B3"]);
    let (_, t) = feats(&o, image, None, serde_json::json!({ "features": "spectral" })).unwrap();
    let g = |n: &str| col(&t, n)[0];
    let tol = 1e-12;
    // B4 1 2 6 7: the population sd, and the median of an even count is the mean of the two in the middle
    near(g("mean_B4"), 4.0, tol, "mean_B4");
    near(g("sd_B4"), 6.5f64.sqrt(), tol, "sd_B4");
    near(g("min_B4"), 1.0, tol, "min_B4");
    near(g("max_B4"), 7.0, tol, "max_B4");
    near(g("median_B4"), 4.0, tol, "median_B4");
    // B8 5 7 8 9 11
    near(g("mean_B8"), 8.0, tol, "mean_B8");
    near(g("median_B8"), 8.0, tol, "median_B8");
    near(g("sd_B8"), (20.0f64 / 5.0).sqrt(), tol, "sd_B8");
    // B3 2 2 3 4 4
    near(g("mean_B3"), 3.0, tol, "mean_B3");
    near(g("median_B3"), 3.0, tol, "median_B3");
    // the means 4, 8, 3
    near(g("brightness"), 5.0, tol, "brightness");
    near(g("max_diff"), (8.0 - 3.0) / 5.0, tol, "max_diff");
    near(g("ratio_B4"), 4.0 / 15.0, tol, "ratio_B4");
    near(g("ratio_B8"), 8.0 / 15.0, tol, "ratio_B8");
    near(g("ratio_B3"), 3.0 / 15.0, tol, "ratio_B3");
    // Sentinel-2's names: red B4, nir B8, green B3
    near(g("ndvi"), (8.0 - 4.0) / (8.0 + 4.0), tol, "ndvi");
    near(g("ndwi"), (3.0 - 8.0) / (3.0 + 8.0), tol, "ndwi");
    // (no short-wave infrared band: no ndmi from a group)
    assert!(t.column("ndmi").is_none());
}

#[test]
fn bands_are_found_by_their_names() {
    let ndvi = |names: [&str; 3], params: serde_json::Value| {
        let (o, image) = five(names);
        feats(&o, image, None, params).map(|(_, t)| t.column("ndvi").map(|c| c[0]))
    };
    let basic = serde_json::json!({});
    // Landsat 8/9: red SR_B4 (the second band, mean 8), nir SR_B5 (the first, mean 4)
    near(ndvi(["SR_B5", "SR_B4", "SR_B3"], basic.clone()).unwrap().unwrap(), (4.0 - 8.0) / (4.0 + 8.0), 1e-12, "l8");
    // the words red and nir
    near(ndvi(["NIR", "Red", "b3"], basic.clone()).unwrap().unwrap(), (4.0 - 8.0) / (4.0 + 8.0), 1e-12, "words");
    // not found: basic leaves ndvi out, naming it is an error that says what to do
    assert_eq!(ndvi(["x", "y", "z"], basic).unwrap(), None);
    let e = ndvi(["x", "y", "z"], serde_json::json!({ "features": ["ndvi"] })).unwrap_err().to_string();
    assert!(e.contains("red") && e.contains("nir"), "{e}");
    // given by the settings, any case: red y (mean 8), nir X (mean 4)
    let given = serde_json::json!({ "features": ["ndvi"], "red": "y", "nir": "X" });
    near(ndvi(["x", "y", "z"], given).unwrap().unwrap(), (4.0 - 8.0) / (4.0 + 8.0), 1e-12, "given");
    let e = ndvi(["x", "y", "z"], serde_json::json!({ "red": "nope" })).unwrap_err().to_string();
    assert!(e.contains("nope") && e.contains("x, y, z"), "{e}");
}

#[test]
fn features_basic_and_all_and_names() {
    let (o, image) = five(["B4", "B8", "B3"]);
    let before: Vec<String> = o.table.columns.clone();
    let (o2, t) = feats(&o, image.clone(), None, serde_json::json!({})).unwrap();
    let new: Vec<&str> = t.columns.iter().map(String::as_str).filter(|c| !before.iter().any(|b| b == c)).collect();
    assert_eq!(
        new,
        ["mean_B4", "mean_B8", "mean_B3", "sd_B4", "sd_B8", "sd_B3", "ndvi", "compactness", "glcm_homogeneity"]
    );
    assert_eq!(o2.table.columns, t.columns);
    // the old columns come first and are kept
    assert_eq!(&t.columns[..before.len()], &before[..]);
    // all: every spectral and shape column
    let (_, all) = feats(&o, image.clone(), None, serde_json::json!({ "features": "all" })).unwrap();
    for c in [
        "median_B8",
        "brightness",
        "ratio_B3",
        "ndwi",
        "perimeter",
        "centroid_y",
        "rectangular_fit",
        "glcm_contrast",
        "glcm_entropy_B4",
        "glcm_asm_B3",
        "nb_diff_B8",
        "nb_count",
        "border_image",
    ] {
        assert!(all.column(c).is_some(), "{c}");
    }
    // a group: its columns and the old ones (area was one of them)
    let (_, t) = feats(&o, image.clone(), None, serde_json::json!({ "features": ["shape"] })).unwrap();
    assert_eq!(t.columns.len(), before.len() + 15 - 1, "{:?}", t.columns);
    assert!(t.columns.iter().all(|c| !c.contains("B4")));
    let (_, t) = feats(&o, image.clone(), None, serde_json::json!({ "features": ["brightness", "length"] })).unwrap();
    assert!(t.column("brightness").is_some() && t.column("length").is_some() && t.column("mean_B4").is_none());
    // an unknown name lists the groups and the features
    let e = feats(&o, image, None, serde_json::json!({ "features": ["shape", "nope"] })).unwrap_err().to_string();
    assert!(
        e.contains("nope") && e.contains("spectral") && e.contains("shape") && e.contains("rectangular_fit"),
        "{e}"
    );
}

#[test]
fn a_recomputed_column_replaces_the_old_one() {
    let (o, image) = five(["B4", "B8", "B3"]);
    let (o1, t1) = feats(&o, image.clone(), None, serde_json::json!({ "features": ["mean", "area"] })).unwrap();
    // area was a column of the segmenter's table: replaced in place, not repeated
    assert_eq!(t1.columns.iter().filter(|c| *c == "area").count(), 1);
    let pos = t1.columns.iter().position(|c| c == "area").unwrap();
    assert_eq!(pos, o.table.columns.iter().position(|c| c == "area").unwrap());
    // a second run keeps the first's columns and does not repeat the ones it gives again
    let (_, t2) = feats(&o1, image, None, serde_json::json!({ "features": ["sd", "mean"] })).unwrap();
    assert_eq!(t2.columns.iter().filter(|c| *c == "mean_B4").count(), 1);
    assert!(t2.column("sd_B4").is_some());
    assert_eq!(t2.columns.len(), t1.columns.len() + 3);
}

#[test]
fn more_layers_keep_their_names_and_a_clash_gets_a_suffix() {
    let (o, image) = five(["B4", "B8", "B3"]);
    let dem = named_stack(5, 1, vec![("B4", vec![10.0; 5]), ("dem", vec![0.0, 1.0, 2.0, 3.0, 4.0])]);
    let p = serde_json::json!({ "features": ["mean", "brightness"] });
    let (_, t) = feats(&o, image.clone(), Some(dem), p).unwrap();
    near(col(&t, "mean_B4")[0], 4.0, 1e-12, "image's B4");
    near(col(&t, "mean_B4_2")[0], 10.0, 1e-12, "more's B4");
    near(col(&t, "mean_dem")[0], 2.0, 1e-12, "dem");
    // the brightness is the image's bands only
    near(col(&t, "brightness")[0], 5.0, 1e-12, "brightness");
    // a single map is named by its port
    let one = Value::Map(map(5, 1, vec![1.0; 5]));
    let (_, t) = feats(&o, image, Some(one), serde_json::json!({ "features": ["mean"] })).unwrap();
    near(col(&t, "mean_more")[0], 1.0, 1e-12, "a map");
}

#[test]
fn features_on_another_grid_say_to_use_raster_match() {
    let (o, image) = five(["B4", "B8", "B3"]);
    let small = Value::Map(map(4, 1, vec![1.0; 4]));
    for (image, more) in [(small.clone(), None), (image, Some(small))] {
        let e = feats(&o, image, more, serde_json::json!({})).unwrap_err().to_string();
        assert!(e.contains("raster.match") && e.contains("\"n\""), "{e}");
    }
}

/// Objects over noise, and a three-band image with a cell now and then missing.
fn noisy_scene(w: usize, h: usize) -> (Arc<Objects>, Value) {
    let (o, _) = segment(&noisy(w, h), 4);
    let mut s = 4321u64;
    let bands = (0..3)
        .map(|b| {
            let data = (0..w * h)
                .map(|_| if lcg(&mut s) < 0.02 { NAN } else { 100.0 * (b + 1) as f64 + 50.0 * lcg(&mut s) })
                .collect();
            (["B2", "B3", "B4"][b], data)
        })
        .collect();
    (o, named_stack(w, h, bands))
}

#[test]
fn means_equal_the_zonal_means() {
    let (o, image) = noisy_scene(40, 30);
    assert!(o.count > 30);
    let (_, t) = feats(&o, image.clone(), None, serde_json::json!({ "features": ["mean"] })).unwrap();
    let ids = run("objects.to_map", vec![("objects", Value::Objects(o.clone()))], serde_json::json!({})).unwrap();
    let inputs = vec![("map", image), ("zones", ids["map"].clone())];
    let zonal = run("raster.zonal", inputs, serde_json::json!({ "stats": ["mean"] })).unwrap();
    let Value::Table(z) = &zonal["table"] else { panic!("table") };
    assert_eq!(z.rows.len(), o.count);
    for b in ["B2", "B3", "B4"] {
        let (mine, theirs) = (col(&t, &format!("mean_{b}")), col(z, &format!("{b}_mean")));
        for (k, (a, b)) in mine.iter().zip(&theirs).enumerate() {
            assert!((a - b).abs() <= 1e-12 || (a.is_nan() && b.is_nan()), "object {}: {a} against {b}", k + 1);
        }
    }
}

#[test]
fn features_are_the_same_on_one_thread_and_four() {
    let (o, image) = noisy_scene(60, 40);
    let more = Value::Map(map(60, 40, (0..2400).map(|i| (i % 17) as f64).collect()));
    let run_in = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| {
            feats(&o, image.clone(), Some(more.clone()), serde_json::json!({ "features": "all" })).unwrap().1
        })
    };
    let (one, four) = (run_in(1), run_in(4));
    assert_eq!(one.columns, four.columns);
    assert!(one.columns.len() > 40);
    for (a, b) in one.rows.iter().zip(&four.rows) {
        assert!(same(a, b), "{a:?} against {b:?}");
    }
}

/// 1000 x 1000 cells, 4 bands, 10 000 objects, features = all (spectral and shape): the time
/// (cargo test --release ... --ignored --nocapture).
#[test]
#[ignore]
fn features_timing_on_a_million_cells() {
    let (w, h) = (1000, 1000);
    let mut s = 7u64;
    let bands = (0..4).map(|b| (["B2", "B3", "B4", "B8"][b], (0..w * h).map(|_| lcg(&mut s)).collect())).collect();
    let image = named_stack(w, h, bands);
    let out = run("segment.chessboard", vec![("image", image.clone())], serde_json::json!({ "size": 10 })).unwrap();
    let o = objects_of(&out);
    let t = std::time::Instant::now();
    let (_, table) = feats(&o, image, None, serde_json::json!({ "features": "all" })).unwrap();
    println!(
        "objects.features all, 1000 x 1000 x 4, {} objects, {} columns: {:.2?}",
        o.count,
        table.columns.len(),
        t.elapsed()
    );
}

// ---------------------------------------------------------------------------
// objects.features: texture and context

/// The texture columns of one object over a single band (the brightness), from `levels` grey levels.
fn glcm_of(o: &Arc<Objects>, values: Vec<f64>, levels: u32) -> Table {
    let (w, h) = (o.grid.width, o.grid.height);
    let p = serde_json::json!({ "features": ["texture"], "texture_bands": ["brightness"], "levels": levels });
    feats(o, Value::Map(map(w, h, values)), None, p).unwrap().1
}

#[test]
fn glcm_of_a_2x2_object_by_hand() {
    let (o, _) = segment(&map(2, 2, vec![1.0; 4]), 4);
    // grey levels [[0, 1], [1, 0]]: the pairs {0,1} four times, {0,0} and {1,1} once each, 6 in all, so the
    // symmetric matrix over 12 is p00 = 1/6, p01 = p10 = 1/3, p11 = 1/6
    let t = glcm_of(&o, vec![0.0, 1.0, 1.0, 0.0], 2);
    let g = |n: &str| col(&t, n)[0];
    let tol = 1e-12;
    near(g("glcm_homogeneity"), 2.0 / 3.0, tol, "homogeneity");
    near(g("glcm_contrast"), 2.0 / 3.0, tol, "contrast");
    near(g("glcm_entropy"), 6f64.ln() / 3.0 + 2.0 * 3f64.ln() / 3.0, tol, "entropy");
    near(g("glcm_asm"), 5.0 / 18.0, tol, "asm");
    // mean 1/2, variance 1/4, covariance -1/12
    near(g("glcm_correlation"), -1.0 / 3.0, tol, "correlation");
}

#[test]
fn glcm_levels_cut_between_the_percentiles_and_a_constant_object_is_smooth() {
    // 3 levels over 0 to 1: 0.5 is level 1, 1 is the last level
    let (o, _) = segment(&map(3, 1, vec![1.0; 3]), 4);
    let t = glcm_of(&o, vec![0.0, 0.5, 1.0], 3);
    // pairs {0,1} and {1,2}: p01 = p10 = p12 = p21 = 1/4
    near(col(&t, "glcm_contrast")[0], 1.0, 1e-12, "contrast");
    near(col(&t, "glcm_homogeneity")[0], 0.5, 1e-12, "homogeneity");
    // a constant object: one level, homogeneity and asm 1, contrast and entropy 0, correlation taken as 1
    let (o, _) = segment(&map(3, 3, vec![1.0; 9]), 4);
    let t = glcm_of(&o, vec![5.0; 9], 8);
    let g = |n: &str| col(&t, n)[0];
    near(g("glcm_homogeneity"), 1.0, 1e-12, "homogeneity");
    near(g("glcm_contrast"), 0.0, 1e-12, "contrast");
    near(g("glcm_asm"), 1.0, 1e-12, "asm");
    near(g("glcm_entropy"), 0.0, 1e-12, "entropy");
    near(g("glcm_correlation"), 1.0, 1e-12, "correlation");
}

#[test]
fn glcm_of_one_cell_is_nan_and_pairs_stay_inside_the_object() {
    let t = glcm_of(&blob(3, 3, &[(1, 1)]), (0..9).map(f64::from).collect(), 4);
    for n in ["homogeneity", "contrast", "entropy", "correlation", "asm"] {
        assert!(col(&t, &format!("glcm_{n}"))[0].is_nan(), "{n}");
    }
    // two objects side by side: no pair crosses the border, so each is constant
    let (o, _) = segment(&map(4, 1, vec![1.0, 1.0, 2.0, 2.0]), 4);
    let t = glcm_of(&o, vec![0.0, 0.0, 9.0, 9.0], 4);
    assert_eq!(col(&t, "glcm_contrast"), [0.0, 0.0]);
}

#[test]
fn texture_bands_and_basic() {
    let (o, image) = five(["B4", "B8", "B3"]);
    // basic: the brightness only
    let (_, t) = feats(&o, image.clone(), None, serde_json::json!({})).unwrap();
    assert!(t.column("glcm_homogeneity").is_some());
    assert!(t.columns.iter().all(|c| !c.starts_with("glcm_") || c == "glcm_homogeneity"));
    // the group: five statistics of the brightness and of each band
    let (_, t) = feats(&o, image.clone(), None, serde_json::json!({ "features": "texture" })).unwrap();
    let glcm: Vec<&String> = t.columns.iter().filter(|c| c.starts_with("glcm_")).collect();
    assert_eq!(glcm.len(), 5 * 4, "{glcm:?}");
    assert!(t.column("glcm_asm").is_some() && t.column("glcm_asm_B8").is_some());
    // a list of layers, one named, and an unknown one
    let p = serde_json::json!({ "features": ["contrast"], "texture_bands": ["B3", "brightness"] });
    let (_, t) = feats(&o, image.clone(), None, p).unwrap();
    assert!(t.column("glcm_contrast_B3").is_some() && t.column("glcm_contrast").is_some());
    assert!(t.column("glcm_contrast_B4").is_none());
    let p = serde_json::json!({ "features": ["contrast"], "texture_bands": ["nope"] });
    assert!(feats(&o, image.clone(), None, p).unwrap_err().to_string().contains("nope"));
    let e = feats(&o, image, None, serde_json::json!({ "levels": 1 })).unwrap_err().to_string();
    assert!(e.contains("levels"), "{e}");
}

/// Three objects in a row, A | B | C (one cell each), over two bands.
fn row3() -> (Arc<Objects>, Value) {
    let (o, _) = segment(&map(3, 1, vec![1.0, 5.0, 9.0]), 4);
    assert_eq!(o.count, 3);
    let image = named_stack(3, 1, vec![("B4", vec![2.0, 6.0, 12.0]), ("B8", vec![10.0, 10.0, 40.0])]);
    (o, image)
}

fn classed_row() -> (Arc<Objects>, Value) {
    let (o, image) = row3();
    let classes = vec![
        ObjectClass { name: "Wood land".into(), colour: None },
        ObjectClass { name: "field".into(), colour: None },
    ];
    (Arc::new((*o).clone().with_classes(classes, vec![1, 2, 1], vec![]).unwrap()), image)
}

fn features_run(
    o: &Arc<Objects>,
    image: Value,
    below: Option<&Arc<Objects>>,
    features: &[&str],
) -> terraflux_ops::Result<Table> {
    let mut inputs = vec![("objects", Value::Objects(o.clone())), ("image", image)];
    inputs.extend(below.map(|b| ("below", Value::Objects(b.clone()))));
    run("objects.features", inputs, serde_json::json!({ "features": features })).map(|out| table_of(&out))
}

fn context_of(o: &Arc<Objects>, image: Value, below: Option<&Arc<Objects>>, features: &[&str]) -> Table {
    features_run(o, image, below, features).unwrap()
}

#[test]
fn context_neighbours_by_hand() {
    let (o, image) = row3();
    let t = context_of(&o, image, None, &["context"]);
    let tol = 1e-12;
    // B4 means 2, 6, 12: A (2 - 6) / 1, B ((6 - 2) + (6 - 12)) / 2, C (12 - 6) / 1
    for (a, b) in col(&t, "nb_diff_B4").iter().zip([-4.0, -1.0, 6.0]) {
        near(*a, b, tol, "nb_diff_B4");
    }
    // B8 means 10, 10, 40
    for (a, b) in col(&t, "nb_diff_B8").iter().zip([0.0, -15.0, 30.0]) {
        near(*a, b, tol, "nb_diff_B8");
    }
    assert_eq!(col(&t, "nb_count"), [1.0, 2.0, 1.0]);
    // each object is one cell: 4 edges; A and C have one shared, B has two
    for (a, b) in col(&t, "border_image").iter().zip([0.75, 0.5, 0.75]) {
        near(*a, b, tol, "border_image");
    }
    // no classes, no parent, no below: nothing else came with the group
    assert!(t.columns.iter().all(|c| !c.starts_with("rel_border") && !c.starts_with("parent") && c != "children"));
}

#[test]
fn context_of_the_classes_by_hand() {
    let (o, image) = classed_row();
    let t = context_of(&o, image, None, &["context"]);
    let tol = 1e-12;
    let eq = |name: &str, want: [f64; 3]| {
        let got = col(&t, name);
        for (a, b) in got.iter().zip(want) {
            assert!((a.is_nan() && b.is_nan()) || (a - b).abs() <= tol, "{name}: {got:?} against {want:?}");
        }
    };
    // (the class names made safe for a column: Wood land is Wood_land)
    eq("rel_border_Wood_land", [0.0, 0.5, 0.0]);
    eq("rel_border_field", [0.25, 0.0, 0.25]);
    eq("nb_Wood_land", [0.0, 2.0, 0.0]);
    eq("nb_field", [1.0, 0.0, 1.0]);
    // the centroids are one cell apart: the nearest of the class other than itself
    eq("dist_Wood_land", [2.0, 1.0, 2.0]);
    eq("dist_field", [1.0, NAN, 1.0]);
    // naming one of them without classes is an error
    let (o, image) = row3();
    let e = features_run(&o, image, None, &["dist"]).unwrap_err().to_string();
    assert!(e.contains("dist") && e.contains("classes"), "{e}");
}

#[test]
fn nearest_class_centroid_agrees_with_brute_force() {
    // 5 x 5 squares in 12 columns and 8 rows, a class by chance: the grid search against all pairs
    let (w, h) = (60, 40);
    let image = Value::Map(map(w, h, vec![1.0; w * h]));
    let out = run("segment.chessboard", vec![("image", image.clone())], serde_json::json!({ "size": 5 })).unwrap();
    let o = objects_of(&out);
    assert_eq!(o.count, 96);
    let mut s = 3u64;
    let class_of: Vec<u32> = (0..o.count).map(|_| if lcg(&mut s) < 0.1 { 1 } else { 2 }).collect();
    let classes =
        vec![ObjectClass { name: "rare".into(), colour: None }, ObjectClass { name: "common".into(), colour: None }];
    let o = Arc::new((*o).clone().with_classes(classes, class_of.clone(), vec![]).unwrap());
    let t = context_of(&o, image, None, &["dist"]);
    let at = |k: usize| ((k % 12) as f64 * 5.0, (k / 12) as f64 * 5.0);
    for (c, name) in [(1u32, "rare"), (2, "common")] {
        let got = col(&t, &format!("dist_{name}"));
        for (k, &g) in got.iter().enumerate() {
            let best = (0..o.count)
                .filter(|&j| j != k && class_of[j] == c)
                .map(|j| (at(j).0 - at(k).0).hypot(at(j).1 - at(k).1))
                .fold(f64::INFINITY, f64::min);
            if best.is_finite() {
                near(g, best, 1e-9, &format!("{name} {k}"));
            } else {
                assert!(g.is_nan());
            }
        }
    }
}

#[test]
fn context_of_the_level_above_and_beneath() {
    // six cells in a row: objects of two cells (o), and above them two objects of four and two cells
    let (o, _) = segment(&map(6, 1, vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0]), 4);
    let image = named_stack(6, 1, vec![("B4", vec![1.0; 6])]);
    let (above, _) = segment(&map(6, 1, vec![1.0, 1.0, 1.0, 1.0, 2.0, 2.0]), 4);
    let nested = Arc::new((*o).clone().with_parent(above, vec![1, 1, 2]).unwrap());
    let t = context_of(&nested, image.clone(), None, &["parent"]);
    assert_eq!(col(&t, "parent_id"), [1.0, 1.0, 2.0]);
    assert_eq!(col(&t, "parent_cells"), [4.0, 4.0, 2.0]);
    assert_eq!(col(&t, "parent_value"), [1.0, 1.0, 2.0]);
    assert!(t.column("parent_id").is_some() && t.column("parent_parent_id").is_none());
    // without a level above: left out by the group, an error when named
    let t = context_of(&o, image.clone(), None, &["context"]);
    assert!(t.column("parent_id").is_none());
    let e = features_run(&o, image.clone(), None, &["parent"]).unwrap_err().to_string();
    assert!(e.contains("parent"), "{e}");
    // beneath: the six cells, in classes tree, shrub, none
    let (cells, _) = segment(&map(6, 1, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), 4);
    let classes =
        vec![ObjectClass { name: "tree".into(), colour: None }, ObjectClass { name: "shrub".into(), colour: None }];
    let below = (*cells).clone().with_parent(o.clone(), vec![1, 1, 2, 2, 3, 3]).unwrap();
    let below = Arc::new(below.with_classes(classes, vec![1, 2, 1, 1, 2, 0], vec![]).unwrap());
    let t = context_of(&o, image.clone(), Some(&below), &["children"]);
    assert_eq!(col(&t, "children"), [2.0, 2.0, 2.0]);
    assert_eq!(col(&t, "children_tree"), [1.0, 2.0, 0.0]);
    assert_eq!(col(&t, "children_shrub"), [1.0, 0.0, 1.0]);
    assert_eq!(col(&t, "children_area_tree"), [0.5, 1.0, 0.0]);
    assert_eq!(col(&t, "children_area_shrub"), [0.5, 0.0, 0.5]);
    // naming children without below is an error; a below of other objects is refused
    let e = features_run(&o, image.clone(), None, &["children"]).unwrap_err().to_string();
    assert!(e.contains("below"), "{e}");
    let e = features_run(&nested, image, Some(&cells), &["children"]).unwrap_err().to_string();
    assert!(e.contains("below"), "{e}");
}

/// 1000 x 1000 cells, 4 bands, about 10 000 irregular objects (SLIC) in three classes, features = all: the time
/// (cargo test --release ... --ignored --nocapture).
#[test]
#[ignore]
fn features_all_timing_on_irregular_objects() {
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
    let o = objects_of(&superpixels("segment.slic", image.clone(), serde_json::json!({ "count": 10000 })));
    let class_of: Vec<u32> = (0..o.count).map(|_| 1 + (lcg(&mut s) * 3.0) as u32).collect();
    let classes = ["a", "b", "c"].iter().map(|n| ObjectClass { name: n.to_string(), colour: None }).collect();
    let o = Arc::new((*o).clone().with_classes(classes, class_of, vec![]).unwrap());
    let t = std::time::Instant::now();
    let (_, table) = feats(&o, image, None, serde_json::json!({ "features": "all" })).unwrap();
    println!(
        "objects.features all, 1000 x 1000 x 4, {} objects, {} columns: {:.2?}",
        o.count,
        table.columns.len(),
        t.elapsed()
    );
}

// objects.samples and objects.classify

/// The operator with the warnings it gave.
fn run_warn(
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

/// Four squares of 5 x 5 cells on a 10 x 10 grid (ids 1 top-left, 2 top-right, 3 bottom-left, 4 bottom-right), with
/// the columns id, cells and area.
fn four_squares() -> Arc<Objects> {
    let m = map(10, 10, vec![1.0; 100]);
    let out = run("segment.chessboard", vec![("image", Value::Map(m))], serde_json::json!({ "size": 5 })).unwrap();
    objects_of(&out)
}

/// Samples: a geometry and a class each.
fn sample_set(items: Vec<(Geometry, &str)>) -> Value {
    let (geometries, classes): (Vec<Geometry>, Vec<String>) =
        items.into_iter().map(|(g, c)| (g, c.to_string())).unzip();
    Value::Vector(Arc::new(Vector {
        geometries,
        attributes: vec![("class".to_string(), Column::Texts(classes))],
        ..Default::default()
    }))
}

fn samples_of(o: &Arc<Objects>, s: Value) -> (terraflux_ops::Result<Outputs>, Vec<String>) {
    run_warn("objects.samples", vec![("objects", Value::Objects(o.clone())), ("samples", s)], serde_json::json!({}))
}

/// The ids and class names of the rows of a training table.
fn rows_of(out: &Outputs) -> Vec<(u32, String)> {
    let t = table_of(out);
    let marked = objects_of(out);
    let ids = marked.class_of.iter().enumerate().filter(|(_, k)| **k != 0).map(|(i, _)| i as u32 + 1);
    ids.zip(t.rows.iter()).map(|(id, r)| (id, t.cell(0, r[0]))).collect()
}

#[test]
fn samples_points_pick_the_objects_under_them() {
    let o = four_squares();
    let s = sample_set(vec![
        (Geometry::Point([1.5, -1.5]), "water"),
        (Geometry::Point([7.5, -1.5]), "forest"),
        (Geometry::Point([2.5, -7.5]), "forest"),
        (Geometry::Point([50.0, -50.0]), "forest"),
    ]);
    let (out, warnings) = samples_of(&o, s);
    let out = out.unwrap();
    let t = table_of(&out);
    // classes numbered by name: forest 1, water 2
    assert_eq!(rows_of(&out), vec![(1, "water".to_string()), (2, "forest".to_string()), (3, "forest".to_string())]);
    assert_eq!(t.columns, ["class", "cells", "area"]);
    assert_eq!(t.rows[0][0], 2.0);
    assert!(warnings.iter().any(|w| w.contains("1 sample") && w.contains("outside")), "{warnings:?}");
    let Value::Table(c) = &out["counts"] else { panic!("counts") };
    assert_eq!(c.rows, vec![vec![1.0, 2.0, 50.0], vec![2.0, 1.0, 25.0]]);
    assert_eq!(c.cell(0, 1.0), "forest");
    let m = objects_of(&out);
    assert_eq!(m.class_of, vec![2, 1, 1, 0]);
    assert_eq!(m.classes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["forest", "water"]);
}

#[test]
fn samples_polygons_pick_the_objects_they_cover_by_half() {
    let o = four_squares();
    // rows 0 to 5: the top squares whole, the bottom ones by one row of five (20%)
    let (out, w) = samples_of(&o, sample_set(vec![(cells_square(0.0, 0.0, 10.0, 6.0), "a")]));
    assert_eq!(rows_of(&out.unwrap()), vec![(1, "a".to_string()), (2, "a".to_string())]);
    assert!(!w.iter().any(|m| m.contains("outside")), "{w:?}");
    // rows 0 to 7: the bottom ones by three rows (60%)
    let (out, _) = samples_of(&o, sample_set(vec![(cells_square(0.0, 0.0, 10.0, 8.0), "a")]));
    assert_eq!(table_of(&out.unwrap()).rows.len(), 4);
    // a polygon holding no object by half is ignored, with a warning
    let (out, w) = samples_of(&o, sample_set(vec![(cells_square(0.0, 4.0, 10.0, 6.0), "a")]));
    assert!(table_of(&out.unwrap()).rows.is_empty());
    assert!(w.iter().any(|w| w.contains("outside")), "{w:?}");
}

#[test]
fn samples_of_two_classes_on_an_object_leave_it_out() {
    let o = four_squares();
    let s = sample_set(vec![
        (Geometry::Point([6.5, -6.5]), "a"),
        (Geometry::Point([8.5, -8.5]), "b"),
        (Geometry::Point([8.5, -8.5]), "b"),
        (Geometry::Point([1.5, -1.5]), "a"),
    ]);
    let (out, warnings) = samples_of(&o, s);
    let out = out.unwrap();
    assert_eq!(rows_of(&out), vec![(1, "a".to_string())]);
    assert!(warnings.iter().any(|w| w.contains("1 object") && w.contains("ids 4")), "{warnings:?}");
    assert_eq!(objects_of(&out).class_of, vec![1, 0, 0, 0]);
    // a samples attribute that is not there is named
    let e = run_warn(
        "objects.samples",
        vec![("objects", Value::Objects(o.clone())), ("samples", sample_set(vec![]))],
        serde_json::json!({ "class": "kind" }),
    )
    .0
    .unwrap_err()
    .to_string();
    assert!(e.contains("kind"), "{e}");
}

/// The blocky nlm image cut in objects, their basic features, and the truth.
fn obia_case(segmenter: &str, params: serde_json::Value) -> (Arc<Objects>, Map) {
    let image = run("nlm.image", vec![("classes", blocky())], serde_json::json!({})).unwrap();
    let (stack, truth) = (image["image"].clone(), image["truth"].clone());
    let Value::Map(truth) = truth else { panic!("truth") };
    let o = objects_of(&run(segmenter, vec![("image", stack.clone())], params).unwrap());
    let out =
        run("objects.features", vec![("objects", Value::Objects(o)), ("image", stack)], serde_json::json!({})).unwrap();
    (objects_of(&out), truth)
}

/// Sample points: up to `per` random objects of each truth class (by their main class), at the cell of the object
/// closest to its box centre.
fn truth_points(o: &Objects, truth: &Map, per: usize, seed: u64) -> Value {
    let w = o.grid.width;
    let mut by_class: Vec<Vec<usize>> = vec![Vec::new(); 8];
    for k in 0..o.count {
        let mut n = [0u32; 8];
        for (&l, &c) in o.labels.iter().zip(truth.data.iter()) {
            if l as usize == k + 1 && !c.is_nan() {
                n[c as usize] += 1;
            }
        }
        let main = (0..8).max_by_key(|&c| (n[c], std::cmp::Reverse(c))).unwrap();
        by_class[main].push(k);
    }
    let mut s = seed;
    let mut items = Vec::new();
    for (c, list) in by_class.iter_mut().enumerate() {
        for _ in 0..per.min(list.len()) {
            let k = list.remove((lcg(&mut s) * list.len() as f64) as usize % list.len());
            let b = o.boxes[k];
            let (cr, cc) = ((b[0] + b[2]) as f64 / 2.0, (b[1] + b[3]) as f64 / 2.0);
            let d = |i: usize| ((i / w) as f64 - cr).powi(2) + ((i % w) as f64 - cc).powi(2);
            let best = (0..o.labels.len())
                .filter(|&i| o.labels[i] as usize == k + 1)
                .min_by(|&i, &j| d(i).partial_cmp(&d(j)).unwrap())
                .unwrap();
            let (r, col) = (best / w, best % w);
            items.push((Geometry::Point([col as f64 + 0.5, -(r as f64 + 0.5)]), format!("c{c}")));
        }
    }
    let (geometries, classes): (Vec<_>, Vec<_>) = items.into_iter().unzip();
    Value::Vector(Arc::new(Vector {
        geometries,
        attributes: vec![("class".to_string(), Column::Texts(classes))],
        ..Default::default()
    }))
}

/// Samples, a fit and a classification: (the samples' table, the model, the classification's outputs).
fn classify_case(o: &Arc<Objects>, truth: &Map, per: usize, method: &str, min: f64) -> (Table, Value, Outputs) {
    let s = run(
        "objects.samples",
        vec![("objects", Value::Objects(o.clone())), ("samples", truth_points(o, truth, per, 7))],
        serde_json::json!({}),
    )
    .unwrap();
    let train = table_of(&s);
    let columns: Vec<&String> = train.columns.iter().filter(|c| *c != "id" && *c != "class").collect();
    let fit = run(
        "ml.fit",
        vec![("table", Value::Table(train.clone()))],
        // (the boosting splits nodes of 20 records or more: its default, with 25 training objects, learns nothing)
        serde_json::json!({ "column": "class", "columns": columns, "method": method, "seed": 1, "min_samples_leaf": 2 }),
    )
    .unwrap();
    let model = fit["model"].clone();
    let out = run(
        "objects.classify",
        vec![
            ("objects", Value::Objects(o.clone())),
            ("model", model.clone()),
            ("samples", Value::Table(train.clone())),
        ],
        serde_json::json!({ "min": min }),
    )
    .unwrap();
    (train, model, out)
}

/// The share of the cells whose class (the name's number) is the truth's.
fn accuracy(out: &Outputs, truth: &Map) -> f64 {
    let Value::Map(m) = &out["map"] else { panic!("map") };
    let names = &table_of(out).labels["class"];
    let hit = m
        .data
        .iter()
        .zip(truth.data.iter())
        .filter(|(p, t)| !p.is_nan() && names[**p as usize - 1] == format!("c{}", **t as usize))
        .count();
    hit as f64 / truth.data.len() as f64
}

#[test]
fn classify_on_nlm_image_by_samples_is_accurate() {
    for (segmenter, params, method) in [
        ("segment.slic", serde_json::json!({}), "random_forest"),
        ("segment.multiresolution", serde_json::json!({ "scale": 5 }), "random_forest"),
        ("segment.slic", serde_json::json!({}), "gradient_boosting"),
    ] {
        let (o, truth) = obia_case(segmenter, params);
        let (train, _, out) = classify_case(&o, &truth, 5, method, 0.0);
        let acc = accuracy(&out, &truth);
        println!("{segmenter} {method}: {} objects, {} training, accuracy {acc:.4}", o.count, train.rows.len());
        assert!(acc >= 0.9, "{segmenter} {method}: accuracy {acc}");
        let t = table_of(&out);
        assert_eq!(t.rows.len(), o.count);
        assert!(t.columns.contains(&"margin".to_string()));
        let c = objects_of(&out);
        assert_eq!(c.classes.len(), 5);
        assert_eq!(c.membership.len(), 5);
        assert!(c.class_of.iter().all(|&k| k > 0));
        // the probabilities add up to 1, the margin is the first minus the second
        for r in &t.rows {
            let p = &r[2..r.len() - 1];
            assert!((p.iter().sum::<f64>() - 1.0).abs() < 1e-9);
            let mut q = p.to_vec();
            q.sort_by(|a, b| b.partial_cmp(a).unwrap());
            assert!((r[r.len() - 1] - (q[0] - q[1])).abs() < 1e-9);
        }
        // the same again, the same result
        let (_, _, again) = classify_case(&o, &truth, 5, method, 0.0);
        assert!(same(&table_of(&out).rows.concat(), &table_of(&again).rows.concat()));
    }
}

#[test]
fn classify_min_leaves_the_unsure_unclassified_and_missing_attributes_are_named() {
    let (o, truth) = obia_case("segment.slic", serde_json::json!({}));
    let (train, model, out) = classify_case(&o, &truth, 5, "random_forest", 0.9);
    let none = objects_of(&out).class_of.iter().filter(|&&k| k == 0).count();
    println!("min 0.9: {none} of {} objects unclassified", o.count);
    assert!(none > 0 && none < o.count, "{none} unclassified");
    let t = table_of(&out);
    assert!(t.rows.iter().all(|r| (r[1].is_nan()) == (r[2..r.len() - 1].iter().all(|&p| p < 0.9))));
    let Value::Map(m) = &out["map"] else { panic!("map") };
    assert!(m.data.iter().any(|v| v.is_nan()));
    // the objects without the attributes the model reads
    let mut bare = o.as_ref().clone();
    bare.table = Table { rows: bare.table.rows.iter().map(|r| vec![r[0]]).collect(), ..Table::new(vec!["id".into()]) };
    let e = run(
        "objects.classify",
        vec![("objects", Value::Objects(Arc::new(bare))), ("model", model), ("samples", Value::Table(train))],
        serde_json::json!({}),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("objects.features"), "{e}");
}

/// The share of the cells whose cluster is the commonest truth class of the cluster.
fn purity(out: &Outputs, truth: &Map) -> f64 {
    let Value::Map(m) = &out["map"] else { panic!("map") };
    let mut count = std::collections::BTreeMap::<(u32, u32), usize>::new();
    for (p, t) in m.data.iter().zip(truth.data.iter()) {
        if !p.is_nan() && !t.is_nan() {
            *count.entry((*p as u32, *t as u32)).or_default() += 1;
        }
    }
    let mut best = std::collections::BTreeMap::<u32, usize>::new();
    for (&(p, _), &n) in &count {
        let b = best.entry(p).or_default();
        *b = (*b).max(n);
    }
    best.values().sum::<usize>() as f64 / truth.data.len() as f64
}

#[test]
fn classify_by_an_unsupervised_model_gives_the_objects_their_clusters() {
    let (o, truth) = obia_case("segment.slic", serde_json::json!({}));
    let table = Value::Table(o.table.clone());
    let columns: Vec<&String> = o.table.columns.iter().filter(|c| c.starts_with("mean_")).collect();
    assert!(columns.len() >= 4, "{columns:?}");
    let classify = |model: &Value| {
        run(
            "objects.classify",
            vec![("objects", Value::Objects(o.clone())), ("model", model.clone())],
            serde_json::json!({}),
        )
        .unwrap()
    };

    // k-means: k clusters named "cluster 1"..., a one-hot membership, the cells' map
    let fit =
        run("ml.kmeans", vec![("table", table.clone())], serde_json::json!({ "k": 5, "columns": columns })).unwrap();
    let out = classify(&fit["model"]);
    let c = objects_of(&out);
    let names: Vec<&str> = c.classes.iter().map(|k| k.name.as_str()).collect();
    assert_eq!(names, ["cluster 1", "cluster 2", "cluster 3", "cluster 4", "cluster 5"]);
    assert!(c.class_of.iter().all(|&k| (1..=5).contains(&k)));
    assert_eq!(c.membership.len(), 5);
    let t = table_of(&out);
    assert_eq!(t.rows.len(), o.count);
    assert!(t.rows.iter().all(|r| r[2..r.len() - 1].iter().sum::<f64>() == 1.0));
    // (the objects' clusters are the table's: the same model on the same rows)
    let cluster = fit_table_clusters(&fit, &o);
    assert_eq!(c.class_of.iter().map(|&k| k as f64).collect::<Vec<_>>(), cluster);
    let p = purity(&out, &truth);
    println!("k-means on the objects: purity {p:.4}");
    assert!(p >= 0.9, "purity {p}");

    // a map with groups: the groups are the classes; without them, the nodes
    let som = |clusters: usize| {
        run(
            "ml.som",
            vec![("table", table.clone())],
            serde_json::json!({ "columns": columns, "rows": 4, "cols": 4, "clusters": clusters, "seed": 1 }),
        )
        .unwrap()
    };
    let out = classify(&som(5)["model"]);
    let c = objects_of(&out);
    assert_eq!(c.classes.len(), 5);
    assert_eq!(c.classes[0].name, "group 1");
    assert!(c.class_of.iter().all(|&k| (1..=5).contains(&k)));
    let p = purity(&out, &truth);
    println!("SOM groups on the objects: purity {p:.4}");
    assert!(p >= 0.75, "purity {p}");
    let c = objects_of(&classify(&som(0)["model"]));
    assert_eq!(c.classes.len(), 16);
    assert_eq!(c.classes[15].name, "node 16");

    // the attributes the model reads are named when the objects lack them
    let mut bare = o.as_ref().clone();
    bare.table = Table { rows: bare.table.rows.iter().map(|r| vec![r[0]]).collect(), ..Table::new(vec!["id".into()]) };
    let e = run(
        "objects.classify",
        vec![("objects", Value::Objects(Arc::new(bare))), ("model", fit["model"].clone())],
        serde_json::json!({}),
    )
    .unwrap_err()
    .to_string();
    assert!(e.contains("mean_") && e.contains("columns"), "{e}");
}

/// The cluster of each object in the table `ml.kmeans` gave (its rows are the objects').
fn fit_table_clusters(fit: &Outputs, o: &Objects) -> Vec<f64> {
    let t = table_of(fit);
    assert_eq!(t.rows.len(), o.count);
    t.column("cluster").unwrap()
}

#[test]
fn join_adds_the_columns_of_a_table_to_the_objects_by_id() {
    let (o, truth) = obia_case("segment.slic", serde_json::json!({}));
    let columns: Vec<String> = o.table.columns.iter().filter(|c| c.starts_with("mean_")).cloned().collect();
    let fit = run(
        "ml.pca",
        vec![("table", Value::Table(o.table.clone()))],
        serde_json::json!({ "columns": columns, "components": 2 }),
    )
    .unwrap();
    let pcs = table_of(&fit);
    let join = |objects: &Arc<Objects>, t: Table, params: serde_json::Value| {
        run("objects.join", vec![("objects", Value::Objects(objects.clone())), ("table", Value::Table(t))], params)
    };
    // the rows in another order: the id decides, not the position
    let mut shuffled = pcs.clone();
    shuffled.rows.reverse();
    let j = join(&o, shuffled, serde_json::json!({ "columns": ["pc1", "pc2"] })).unwrap();
    let joined = objects_of(&j);
    assert_eq!(joined.count, o.count);
    assert_eq!(&joined.table.columns[..o.table.columns.len()], &o.table.columns[..]);
    assert_eq!(&joined.table.columns[o.table.columns.len()..], ["pc1", "pc2"]);
    let (own, want) = (joined.table.column("pc1").unwrap(), pcs.column("pc1").unwrap());
    assert!(own.iter().zip(&want).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
    // (the default: the numeric columns the objects lack, so not the pcs' copies of the attributes)
    let d = objects_of(&join(&o, pcs.clone(), serde_json::json!({})).unwrap());
    assert_eq!(&d.table.columns[o.table.columns.len()..], ["pc1", "pc2"]);

    // downstream: the samples' table has the components, and a forest on them classifies the objects
    let s = run(
        "objects.samples",
        vec![("objects", Value::Objects(joined.clone())), ("samples", truth_points(&o, &truth, 5, 7))],
        serde_json::json!({}),
    )
    .unwrap();
    let train = table_of(&s);
    assert!(train.columns.contains(&"pc1".to_string()) && train.columns.contains(&"pc2".to_string()));
    let model = run(
        "ml.fit",
        vec![("table", Value::Table(train.clone()))],
        serde_json::json!({ "column": "class", "columns": ["pc1", "pc2"], "seed": 1 }),
    )
    .unwrap();
    let out = run(
        "objects.classify",
        vec![
            ("objects", Value::Objects(joined.clone())),
            ("model", model["model"].clone()),
            ("samples", Value::Table(train)),
        ],
        serde_json::json!({}),
    )
    .unwrap();
    assert_eq!(objects_of(&out).class_of.len(), o.count);

    // refusals name the ids
    let refuse = |t: Table, params: serde_json::Value| join(&o, t, params).map(|_| ()).expect_err("refused").to_string();
    let mut less = pcs.clone();
    less.rows.remove(0);
    let e = refuse(less, serde_json::json!({ "columns": ["pc1"] }));
    assert!(e.contains("objects with no row: 1"), "{e}");
    let mut twice = pcs.clone();
    twice.rows[1][0] = 1.0;
    let e = refuse(twice, serde_json::json!({ "columns": ["pc1"] }));
    assert!(e.contains("ids twice: 1") && e.contains("objects with no row: 2"), "{e}");
    let mut unknown = pcs.clone();
    unknown.rows[0][0] = (o.count + 5) as f64;
    let e = refuse(unknown, serde_json::json!({ "columns": ["pc1"] }));
    assert!(e.contains(&format!("{}", o.count + 5)) && e.contains("no object's"), "{e}");
    let e = refuse(pcs.clone(), serde_json::json!({ "columns": ["nothing"] }));
    assert!(e.contains("no column \"nothing\""), "{e}");
    let no_id = Table {
        columns: vec!["pc1".into()],
        rows: pcs.column("pc1").unwrap().into_iter().map(|v| vec![v]).collect(),
        ..Default::default()
    };
    assert!(refuse(no_id, serde_json::json!({})).contains("no column id"));

    // a column the objects have is refused unless replace is true
    let e = join(&joined, joined.table.clone(), serde_json::json!({ "columns": ["pc1"] })).map(|_| ()).expect_err("refused");
    assert!(e.to_string().contains("replace"), "{e}");
    let mut changed = joined.table.clone();
    let k = changed.columns.iter().position(|c| c == "pc1").unwrap();
    for r in changed.rows.iter_mut() {
        r[k] = 7.0;
    }
    let r = objects_of(&join(&joined, changed, serde_json::json!({ "columns": ["pc1"], "replace": true })).unwrap());
    assert_eq!(r.table.columns.len(), joined.table.columns.len());
    assert!(r.table.column("pc1").unwrap().iter().all(|&v| v == 7.0));
}

#[test]
fn samples_table_fits_without_the_id_as_a_predictor() {
    let (o, truth) = obia_case("segment.slic", serde_json::json!({}));
    let s = run(
        "objects.samples",
        vec![("objects", Value::Objects(o.clone())), ("samples", truth_points(&o, &truth, 5, 7))],
        serde_json::json!({}),
    )
    .unwrap();
    let train = table_of(&s);
    assert_eq!(train.columns[0], "class");
    assert!(!train.columns.contains(&"id".to_string()));
    // ml.fit with only the target: every other column is a predictor
    let fit = run(
        "ml.fit",
        vec![("table", Value::Table(train.clone()))],
        serde_json::json!({ "column": "class", "seed": 1, "min_samples_leaf": 2 }),
    )
    .unwrap();
    let Value::MlModel(m) = &fit["model"] else { panic!("a model") };
    assert!(m.predictors().iter().all(|p| p != "id" && p != "class"), "{:?}", m.predictors());
    assert_eq!(m.predictors().len(), train.columns.len() - 1);
}

// ---- objects.calc

/// Three objects on a 3 x 2 grid: A the first column, B the cell right of it on the top row, C the other three cells
/// (ids 1, 2, 3; borders A-B 1, A-C 1, B-C 2; perimeters 6, 4, 8). Columns id, mean_red (2, 6, 12) and ndwi; classes
/// water, field and road, A and C water, B field, none of road.
fn calc_row() -> Arc<Objects> {
    let (o, _) = segment(&map(3, 2, vec![1.0, 2.0, 3.0, 1.0, 3.0, 3.0]), 4);
    assert_eq!(o.count, 3);
    let mut t = Table::new(["id", "mean_red", "ndwi"].map(String::from).to_vec());
    t.rows = vec![vec![1.0, 2.0, -0.2], vec![2.0, 6.0, 0.1], vec![3.0, 12.0, 0.5]];
    let classes = ["water", "field", "road"].map(|name| ObjectClass { name: name.into(), colour: None }).to_vec();
    Arc::new((*o).clone().with_table(t).unwrap().with_classes(classes, vec![1, 2, 1], vec![]).unwrap())
}

fn calc_out(o: &Arc<Objects>, below: Option<&Arc<Objects>>, script: &str) -> terraflux_ops::Result<Outputs> {
    let mut inputs = vec![("objects", Value::Objects(o.clone()))];
    inputs.extend(below.map(|b| ("below", Value::Objects(b.clone()))));
    run("objects.calc", inputs, serde_json::json!({ "expression": script }))
}

/// The column `v` that `out v = <expr>` gives.
fn calc_col(o: &Arc<Objects>, below: Option<&Arc<Objects>>, expr: &str) -> Vec<f64> {
    let out = calc_out(o, below, &format!("out v = {expr}")).unwrap_or_else(|e| panic!("{expr}: {e}"));
    col(&table_of(&out), "v")
}

fn close(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: {got:?} against {want:?}");
    for (a, b) in got.iter().zip(want) {
        assert!((a.is_nan() && b.is_nan()) || (a - b).abs() <= 1e-12, "{what}: {got:?} against {want:?}");
    }
}

/// Seven objects in a row with the columns x and y.
fn fuzzy_row() -> Arc<Objects> {
    let (o, _) = segment(&map(7, 1, (1..=7).map(f64::from).collect()), 4);
    let mut t = Table::new(["id", "x", "y"].map(String::from).to_vec());
    let x = [-1.0, 0.0, 0.25, 0.5, 1.0, 2.0, NAN];
    let y = [0.2, 0.9, 0.5, 0.5, 0.1, 0.3, 0.4];
    t.rows = (0..7).map(|k| vec![(k + 1) as f64, x[k], y[k]]).collect();
    Arc::new((*o).clone().with_table(t).unwrap())
}

#[test]
fn calc_membership_functions_by_hand() {
    let o = fuzzy_row();
    close(&calc_col(&o, None, "above(x, 0, 1)"), &[0.0, 0.0, 0.25, 0.5, 1.0, 1.0, NAN], "above");
    close(&calc_col(&o, None, "below(x, 0, 1)"), &[1.0, 1.0, 0.75, 0.5, 0.0, 0.0, NAN], "below");
    // rises from 0 to 0.5, 1 up to 1, falls to 0 at 2
    close(&calc_col(&o, None, "between(x, 0, 0.5, 1, 2)"), &[0.0, 0.0, 0.5, 1.0, 1.0, 0.0, NAN], "between");
    close(&calc_col(&o, None, "about(x, 0.5, 1)"), &[0.0, 0.5, 0.75, 1.0, 0.5, 0.0, NAN], "about");
    // a bound that is a value of the objects
    close(&calc_col(&o, None, "above(x, 0, y)"), &[0.0, 0.0, 0.5, 1.0, 1.0, 1.0, NAN], "above with y");
    // fuzzy and, or, not: the smaller, the larger, one minus
    let a = [0.0, 0.0, 0.25, 0.5, 1.0, 1.0, NAN];
    close(&calc_col(&o, None, "above(x, 0, 1) and below(y, 0, 1)"), &[0.0, 0.0, 0.25, 0.5, 0.9, 0.7, NAN], "and");
    close(&calc_col(&o, None, "above(x, 0, 1) or below(y, 0, 1)"), &[0.8, 0.1, 0.5, 0.5, 1.0, 1.0, NAN], "or");
    close(&calc_col(&o, None, "not above(x, 0, 1)"), &a.map(|v| 1.0 - v), "not");
    // with 0 and 1 it is logic as usual
    close(&calc_col(&o, None, "(x > 0 and y > 0.3) or not (y > 0.2)"), &[1.0, 0.0, 1.0, 1.0, 1.0, 0.0, NAN], "logic");
    // the other calculators do not know these
    for text in ["above(x, 0, 1)", "nb.mean(x)", "border_to(\"a\")", "children.count(x)"] {
        assert!(terraflux_ops::algebra::parse_script(text).is_err(), "{text}");
        assert!(terraflux_ops::algebra::parse_script_objects(text).is_ok(), "{text}");
    }
    // a wrong number of values is told
    let e = calc_out(&o, None, "above(x, 0)").unwrap_err().to_string();
    assert!(e.contains("above takes 3"), "{e}");
    let e = calc_out(&o, None, "nb.median(x)").unwrap_err().to_string();
    assert!(e.contains("no \"nb.median\""), "{e}");
}

#[test]
fn calc_neighbours_and_classes_by_hand() {
    let o = calc_row();
    // border-weighted: A (6 x 1 + 12 x 1) / 2, B (2 x 1 + 12 x 2) / 3, C (2 x 1 + 6 x 2) / 3
    close(&calc_col(&o, None, "nb.mean(mean_red)"), &[9.0, 26.0 / 3.0, 14.0 / 3.0], "nb.mean");
    close(&calc_col(&o, None, "nb.min(mean_red)"), &[6.0, 2.0, 2.0], "nb.min");
    close(&calc_col(&o, None, "nb.max(mean_red)"), &[12.0, 12.0, 6.0], "nb.max");
    close(&calc_col(&o, None, "nb.sum(mean_red)"), &[18.0, 14.0, 8.0], "nb.sum");
    close(&calc_col(&o, None, "nb.count(cls == \"water\")"), &[1.0, 2.0, 1.0], "nb.count");
    // the share of the perimeter (6, 4, 8 edges) shared with water: C 1, A 1 and C 2, A 1
    close(&calc_col(&o, None, "border_to(\"water\")"), &[1.0 / 6.0, 0.75, 0.125], "border_to");
    close(&calc_col(&o, None, "border_to(\"road\")"), &[0.0, 0.0, 0.0], "border_to road");
    // centroids (0.5, -1), (1.5, -0.5), (13/6, -7/6): water is 0 for itself, B is 2/3 x sqrt 2 from C
    let b = (2.0f64 / 3.0).hypot(2.0 / 3.0);
    close(&calc_col(&o, None, "distance_to(\"water\")"), &[0.0, b, 0.0], "distance_to");
    close(&calc_col(&o, None, "distance_to(\"road\")"), &[NAN, NAN, NAN], "distance_to none");
    // the argument is any expression over the objects, names the script gave included
    close(&calc_col(&o, None, "nb.mean(mean_red * 2)"), &[18.0, 52.0 / 3.0, 28.0 / 3.0], "nb.mean of an expression");
    let out = calc_out(&o, None, "d = mean_red - 1\nout v = nb.max(d)").unwrap();
    close(&col(&table_of(&out), "v"), &[11.0, 11.0, 5.0], "nb.max of a name");
    // a call inside a call, and in a condition
    close(&calc_col(&o, None, "nb.mean(nb.count(cls == \"water\"))"), &[1.5, 1.0, 5.0 / 3.0], "nested");
    close(&calc_col(&o, None, "1 if nb.count(cls == \"water\") > 1 else 0"), &[0.0, 1.0, 0.0], "in a condition");
    // a class that is not there is told, with the classes
    let e = calc_out(&o, None, "border_to(\"sea\")").unwrap_err().to_string();
    assert!(e.contains("sea") && e.contains("water, field, road"), "{e}");
    let (bare, _) = segment(&map(3, 1, vec![1.0, 2.0, 3.0]), 4);
    let e = calc_out(&bare, None, "distance_to(\"water\")").unwrap_err().to_string();
    assert!(e.contains("no classes"), "{e}");
}

#[test]
fn calc_parent_and_children_by_hand() {
    let o = calc_row();
    // the level above: P1 holds A and B, P2 holds C
    let (up, _) = segment(&map(3, 2, vec![1.0, 1.0, 2.0, 1.0, 2.0, 2.0]), 4);
    let mut t = Table::new(["id", "mean_red"].map(String::from).to_vec());
    t.rows = vec![vec![1.0, 100.0], vec![2.0, 200.0]];
    let classes = ["urban", "rural"].map(|name| ObjectClass { name: name.into(), colour: None }).to_vec();
    let up = (*up).clone().with_table(t).unwrap().with_classes(classes, vec![1, 2], vec![]).unwrap();
    let nested = Arc::new((*o).clone().with_parent(Arc::new(up), vec![1, 1, 2]).unwrap());
    close(&calc_col(&nested, None, "parent.mean_red"), &[100.0, 100.0, 200.0], "parent.mean_red");
    close(&calc_col(&nested, None, "mean_red / parent.mean_red"), &[0.02, 0.06, 0.06], "ratio to the parent");
    close(&calc_col(&nested, None, "parent.cls == \"urban\""), &[1.0, 1.0, 0.0], "parent.cls");
    // A: (100 + 200) / 2, B: (100 x 1 + 200 x 2) / 3, C: (100 x 1 + 100 x 2) / 3
    close(&calc_col(&nested, None, "nb.mean(parent.mean_red)"), &[150.0, 500.0 / 3.0, 100.0], "nb of parent");
    // no level above, or no such attribute: told
    let e = calc_out(&o, None, "parent.mean_red").unwrap_err().to_string();
    assert!(e.contains("parent level"), "{e}");
    let e = calc_out(&nested, None, "parent.nope").unwrap_err().to_string();
    assert!(e.contains("nope") && e.contains("mean_red"), "{e}");

    // the level beneath: the six cells, in classes tree, shrub, none; parents A, B, C, A, C, C
    let (cells, _) = segment(&map(3, 2, (1..=6).map(f64::from).collect()), 4);
    let classes = ["tree", "shrub"].map(|name| ObjectClass { name: name.into(), colour: None }).to_vec();
    let below = (*cells).clone().with_parent(o.clone(), vec![1, 2, 3, 1, 3, 3]).unwrap();
    let below = Arc::new(below.with_classes(classes, vec![1, 2, 1, 1, 2, 0], vec![]).unwrap());
    let b = Some(&below);
    close(&calc_col(&o, b, "children.count(cls == \"tree\")"), &[2.0, 0.0, 1.0], "children.count");
    close(&calc_col(&o, b, "children.area(cls == \"tree\")"), &[1.0, 0.0, 1.0 / 3.0], "children.area");
    close(&calc_col(&o, b, "children.mean(id)"), &[2.5, 2.0, 14.0 / 3.0], "children.mean");
    // the argument is read over the children: their parent's attributes too
    close(
        &calc_col(&o, b, "children.count(cls == \"tree\" and parent.mean_red > 5)"),
        &[0.0, 0.0, 1.0],
        "children of a parent",
    );
    // without below, or with a level that is not beneath: told
    let e = calc_out(&o, None, "children.count(cls == \"tree\")").unwrap_err().to_string();
    assert!(e.contains("below"), "{e}");
    let e = calc_out(&nested, Some(&cells), "1").unwrap_err().to_string();
    assert!(e.contains("below"), "{e}");
}

#[test]
fn calc_outputs_text_and_table() {
    let o = calc_row();
    let out = calc_out(&o, None, "out kind = \"wet\" if ndwi > 0 else \"dry\"\nout wet = above(ndwi, 0, 0.5)").unwrap();
    let t = table_of(&out);
    assert_eq!(t.columns, ["id", "mean_red", "ndwi", "kind", "wet"]);
    assert!(t.is_text("kind"));
    assert_eq!(t.rows.iter().map(|r| t.cell(3, r[3])).collect::<Vec<_>>(), ["dry", "wet", "wet"]);
    close(&col(&t, "wet"), &[0.0, 0.2, 1.0], "wet");
    // the objects carry the same table, and a column of the same name is replaced
    let carried = objects_of(&out).table.clone();
    assert!(carried.columns == t.columns && carried.rows == t.rows && carried.labels == t.labels);
    let out = calc_out(&o, None, "out ndwi = ndwi * 10\nout cls_here = cls").unwrap();
    let t = table_of(&out);
    close(&col(&t, "ndwi"), &[-2.0, 1.0, 5.0], "ndwi replaced");
    assert_eq!(t.columns.len(), 4);
    assert_eq!(t.rows.iter().map(|r| t.cell(3, r[3])).collect::<Vec<_>>(), ["water", "field", "water"]);
    // a script without out needs the column; with out, not
    let e = calc_out(&o, None, "ndwi * 2").unwrap_err().to_string();
    assert!(e.contains("name the column"), "{e}");
    let out = run(
        "objects.calc",
        vec![("objects", Value::Objects(o.clone()))],
        serde_json::json!({ "expression": "ndwi * 2", "column": "twice" }),
    )
    .unwrap();
    close(&col(&table_of(&out), "twice"), &[-0.4, 0.2, 1.0], "twice");
    let e = calc_out(&o, None, "out id = 1").unwrap_err().to_string();
    assert!(e.contains("id"), "{e}");
}

// ---- objects.rules and objects.nearest

/// Three objects in a row (ids 1 to 3) with the columns ndwi (0.125, 0.5, -0.25) and ndvi (0.5, 0.25, 1), values
/// that make the memberships below exact.
fn rules_row() -> Arc<Objects> {
    let (o, _) = segment(&map(3, 1, vec![1.0, 2.0, 3.0]), 4);
    let mut t = Table::new(["id", "ndwi", "ndvi"].map(String::from).to_vec());
    t.rows = vec![vec![1.0, 0.125, 0.5], vec![2.0, 0.5, 0.25], vec![3.0, -0.25, 1.0]];
    Arc::new((*o).clone().with_table(t).unwrap())
}

fn rules_out(o: &Arc<Objects>, classes: serde_json::Value, min: Option<f64>) -> terraflux_ops::Result<Outputs> {
    let mut p = serde_json::json!({ "classes": classes });
    if let Some(m) = min {
        p["min"] = serde_json::json!(m);
    }
    run("objects.rules", vec![("objects", Value::Objects(o.clone()))], p)
}

/// wet (water, marsh) and land: the hierarchy of the hand calculation.
fn wet_land() -> serde_json::Value {
    serde_json::json!([
        { "name": "wet", "rule": "above(ndwi, 0, 0.25)", "colour": "#3366cc" },
        { "name": "water", "rule": "below(ndvi, 0.25, 0.75)", "parent": "wet" },
        { "name": "marsh", "rule": "above(ndvi, 0.25, 0.75)", "parent": "wet" },
        { "name": "land", "rule": "below(ndwi, 0, 0.25)" }
    ])
}

#[test]
fn rules_inherit_the_mothers_membership_and_leaves_decide() {
    let o = rules_row();
    let out = rules_out(&o, wet_land(), None).unwrap();
    let t = table_of(&out);
    assert_eq!(t.columns, ["id", "class", "m_wet", "m_water", "m_marsh", "m_land", "margin"]);
    // wet 0.5, 1, 0; own water 0.5 (ndvi .5), 1, 0; own marsh 0.5, 0, 1; land 0.5, 0, 1; the child is the smaller
    close(&col(&t, "m_wet"), &[0.5, 1.0, 0.0], "wet");
    close(&col(&t, "m_water"), &[0.5, 1.0, 0.0], "water = min(wet, own)");
    close(&col(&t, "m_marsh"), &[0.5, 0.0, 0.0], "marsh = min(wet, own)");
    close(&col(&t, "m_land"), &[0.5, 0.0, 1.0], "land");
    // leaves are water, marsh and land: object 1 ties at 0.5 (the first of the list, water), 2 water, 3 land
    let c = objects_of(&out);
    assert_eq!(c.classes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["wet", "water", "marsh", "land"]);
    assert_eq!(c.classes[0].colour, Some([0x33, 0x66, 0xcc]));
    assert_eq!(c.class_of, vec![2, 2, 4]);
    assert_eq!(c.membership.len(), 4);
    assert_eq!(t.cell(1, t.rows[2][1]), "land");
    // the margin: best leaf less the second best
    close(&col(&t, "margin"), &[0.0, 1.0, 1.0], "margin");
    // the map holds the leaf's class (1..) of each cell
    let Value::Map(m) = &out["map"] else { panic!("map") };
    assert_eq!(m.data.to_vec(), vec![2.0, 2.0, 4.0]);
}

#[test]
fn rules_min_leaves_the_unsure_unclassified() {
    let o = rules_row();
    let out = rules_out(&o, wet_land(), Some(0.6)).unwrap();
    assert_eq!(objects_of(&out).class_of, vec![0, 2, 4]);
    let Value::Map(m) = &out["map"] else { panic!("map") };
    assert!(m.data[0].is_nan() && m.data[1] == 2.0);
    let t = table_of(&out);
    assert!(t.rows[0][1].is_nan());
    // the default is 0.1: an object at 0.05 is left out, at 0.1 it is not
    let (o, _) = segment(&map(3, 1, vec![1.0, 2.0, 3.0]), 4);
    let mut t = Table::new(["id", "x"].map(String::from).to_vec());
    t.rows = vec![vec![1.0, 0.05], vec![2.0, 0.1], vec![3.0, 0.5]];
    let o = Arc::new((*o).clone().with_table(t).unwrap());
    let out = rules_out(&o, serde_json::json!([{ "name": "a", "rule": "x" }]), None).unwrap();
    assert_eq!(objects_of(&out).class_of, vec![0, 1, 1]);
    // min must be 0 to 1
    let e = rules_out(&o, serde_json::json!([{ "name": "a", "rule": "x" }]), Some(2.0)).unwrap_err().to_string();
    assert!(e.contains("min"), "{e}");
}

#[test]
fn rules_clamp_missing_values_and_a_class_without_a_rule_is_its_mothers() {
    let o = fuzzy_row();
    let classes = serde_json::json!([
        { "name": "any" },
        { "name": "c", "rule": "x", "parent": "any" },
        { "name": "d", "rule": "2 - x * 0 - 2 + 0.5", "parent": "any" }
    ]);
    let t = table_of(&rules_out(&o, classes, Some(0.0)).unwrap());
    // the value is cut to 0..1 and a missing one is 0; the root with no rule is 1
    close(&col(&t, "m_any"), &[1.0; 7], "any");
    close(&col(&t, "m_c"), &[0.0, 0.0, 0.25, 0.5, 1.0, 1.0, 0.0], "c");
    // x * 0 is missing for the missing x: 0
    close(&col(&t, "m_d"), &[0.5, 0.5, 0.5, 0.5, 0.5, 0.5, 0.0], "d");
    // a script of several lines: the last value counts
    let classes = serde_json::json!([{ "name": "s", "rule": "a = above(x, 0, 1)\nb = 1 - a\nb" }]);
    let t = table_of(&rules_out(&o, classes, Some(0.0)).unwrap());
    close(&col(&t, "m_s"), &[1.0, 1.0, 0.75, 0.5, 0.0, 0.0, 0.0], "script");
}

#[test]
fn rules_errors_name_the_class() {
    let o = rules_row();
    let e = |classes: serde_json::Value| rules_out(&o, classes, None).unwrap_err().to_string();
    let m = e(serde_json::json!([
        { "name": "a", "parent": "b", "rule": "ndwi" },
        { "name": "b", "parent": "c" },
        { "name": "c", "parent": "a" }
    ]));
    assert!(m.contains("cycle") && m.contains("a -> b -> c -> a"), "{m}");
    let m = e(serde_json::json!([{ "name": "a", "parent": "a" }]));
    assert!(m.contains("cycle"), "{m}");
    let m = e(serde_json::json!([{ "name": "a", "parent": "nope" }]));
    assert!(m.contains("\"nope\"") && m.contains("\"a\""), "{m}");
    let m = e(serde_json::json!([{ "name": "a" }, { "name": "a" }]));
    assert!(m.contains("\"a\""), "{m}");
    let m = e(serde_json::json!([{ "name": "a", "colour": "blue" }]));
    assert!(m.contains("colour") && m.contains("blue"), "{m}");
    let m = e(serde_json::json!([{ "name": "a", "rule": "nothing > 1" }]));
    assert!(m.contains("class \"a\"") && m.contains("nothing"), "{m}");
    let m = e(serde_json::json!([{ "name": "a", "rule": "above(ndwi, 0)" }]));
    assert!(m.contains("class \"a\"") && m.contains("above takes 3"), "{m}");
    let m = e(serde_json::json!([{ "name": "a", "rune": "ndwi" }]));
    assert!(m.contains("rune"), "{m}");
    let m = e(serde_json::json!([]));
    assert!(m.contains("empty"), "{m}");
    // no classes at all
    let r = run("objects.rules", vec![("objects", Value::Objects(o.clone()))], serde_json::json!({}));
    assert!(r.unwrap_err().to_string().contains("no classes"));
    // the hierarchy a stage that shows it reads
    let t = terraflux_ops::objects::tree(&wet_land()).unwrap();
    assert_eq!(t.iter().map(|c| c.parent).collect::<Vec<_>>(), [None, Some(0), Some(0), None]);
    assert_eq!(terraflux_ops::objects::leaves(&t), vec![1, 2, 3]);
    assert_eq!(t[1].rule.as_deref(), Some("below(ndvi, 0.25, 0.75)"));
}

#[test]
fn rules_read_the_classes_the_objects_have_before_the_node() {
    // calc_row: A and C water, B field, borders A-B 1, A-C 1, B-C 2, perimeters 6, 4, 8
    let o = calc_row();
    let classes = serde_json::json!([{ "name": "near", "rule": r#"above(border_to("water"), 0, 0.5)"# }]);
    let out = rules_out(&o, classes, Some(0.3)).unwrap();
    // the border to water: A 1/6, B 3/4, C 1/8; the membership is twice that, at most 1
    close(&col(&table_of(&out), "m_near"), &[1.0 / 3.0, 1.0, 0.25], "near");
    // (the classes of the input, not those the node gives, are read: the output classes are only `near`)
    assert_eq!(objects_of(&out).class_of, vec![1, 1, 0]);
    // objects without classes cannot be asked about them
    let e = rules_out(&rules_row(), serde_json::json!([{ "name": "a", "rule": r#"border_to("water")"# }]), None)
        .unwrap_err()
        .to_string();
    assert!(e.contains("class \"a\"") && e.contains("no classes"), "{e}");
    // a second pass reads the first's classes
    let first = rules_out(&rules_row(), wet_land(), None).unwrap();
    let c1 = objects_of(&first);
    let second =
        rules_out(&c1, serde_json::json!([{ "name": "by_land", "rule": r#"above(border_to("land"), 0, 0.5)"# }]), None)
            .unwrap();
    // land is object 3, and only object 2 touches it: 1 of its 4 edges, a membership of 0.5
    close(&col(&table_of(&second), "m_by_land"), &[0.0, 0.5, 0.0], "second pass");
}

/// Objects in a row with the columns f1 and f2, and the samples' table (class a, b).
fn nearest_case(f1: &[f64], f2: &[f64], samples: &[(f64, f64, f64)]) -> (Arc<Objects>, Table) {
    let n = f1.len();
    let (o, _) = segment(&map(n, 1, (1..=n).map(|v| v as f64).collect()), 4);
    let mut t = Table::new(["id", "f1", "f2"].map(String::from).to_vec());
    t.rows = (0..n).map(|k| vec![(k + 1) as f64, f1[k], f2[k]]).collect();
    let mut s = Table::new(["class", "f1", "f2"].map(String::from).to_vec());
    s.rows = samples.iter().map(|&(c, a, b)| vec![c, a, b]).collect();
    s.labels.insert("class".into(), vec!["a".into(), "b".into()]);
    (Arc::new((*o).clone().with_table(t).unwrap()), s)
}

fn nearest_out(o: &Arc<Objects>, s: &Table, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    run("objects.nearest", vec![("objects", Value::Objects(o.clone())), ("samples", Value::Table(s.clone()))], params)
}

fn sd(v: &[f64]) -> f64 {
    let m = v.iter().sum::<f64>() / v.len() as f64;
    (v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64).sqrt()
}

#[test]
fn nearest_takes_the_class_of_the_nearest_samples() {
    let f1 = [0.0, 0.1, 0.2, 10.0, 10.1, 10.2];
    let f2 = [1.0, 1.1, 0.9, 5.0, 5.2, 4.9];
    let (o, s) = nearest_case(&f1, &f2, &[(1.0, 0.0, 1.0), (1.0, 0.3, 1.0), (2.0, 10.0, 5.0)]);
    let out = nearest_out(&o, &s, serde_json::json!({})).unwrap();
    let c = objects_of(&out);
    assert_eq!(c.class_of, vec![1, 1, 1, 2, 2, 2]);
    assert_eq!(c.classes.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    let t = table_of(&out);
    assert_eq!(t.columns, ["id", "class", "m_a", "m_b", "margin"]);
    // an object that is a sample has the membership 1 to its class
    near(col(&t, "m_a")[0], 1.0, 1e-12, "the sample itself");
    near(col(&t, "m_b")[3], 1.0, 1e-12, "the sample itself");
    assert!(col(&t, "m_a")[5] < 1e-3 && col(&t, "m_b")[0] < 1e-3);
    near(col(&t, "margin")[0], 1.0 - col(&t, "m_b")[0], 1e-12, "margin");
    let Value::Map(m) = &out["map"] else { panic!("map") };
    assert_eq!(m.data.to_vec(), vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0]);
    // the columns can be chosen: f1 alone, and a missing one is named
    assert_eq!(
        objects_of(&nearest_out(&o, &s, serde_json::json!({ "columns": ["f1"] })).unwrap()).class_of,
        c.class_of
    );
    let e = nearest_out(&o, &s, serde_json::json!({ "columns": ["f9"] })).unwrap_err().to_string();
    assert!(e.contains("f9") && e.contains("objects"), "{e}");
    let e = nearest_out(&o, &s, serde_json::json!({ "slope": 1.0 })).unwrap_err().to_string();
    assert!(e.contains("slope"), "{e}");
}

#[test]
fn nearest_membership_at_a_distance_of_one_is_the_slope() {
    let f1 = [0.0, 1.0, 4.0, 5.0];
    let f2 = [0.0, 3.0, 1.0, 2.0];
    let (s1, s2) = (sd(&f1), sd(&f2));
    // a sample one deviation of f1 from the object 2, and one far away
    let (o, s) = nearest_case(&f1, &f2, &[(1.0, f1[1] - s1, f2[1]), (2.0, f1[3] + 10.0 * s1, f2[3])]);
    let t = table_of(&nearest_out(&o, &s, serde_json::json!({ "columns": ["f1"] })).unwrap());
    near(col(&t, "m_a")[1], 0.2, 1e-9, "d = 1 gives the slope (0.2)");
    // by hand: exp(-ln(5) * d^2) for the others
    let d = (f1[2] - (f1[1] - s1)) / s1;
    near(col(&t, "m_a")[2], 0.2f64.powf(d * d), 1e-9, "d by hand");
    let t = table_of(&nearest_out(&o, &s, serde_json::json!({ "columns": ["f1"], "slope": 0.5 })).unwrap());
    near(col(&t, "m_a")[1], 0.5, 1e-9, "slope 0.5");
    // min: 0.2 passes the default 0.1, not 0.3
    let out = nearest_out(&o, &s, serde_json::json!({ "columns": ["f1"] })).unwrap();
    assert_eq!(objects_of(&out).class_of[1], 1);
    let out = nearest_out(&o, &s, serde_json::json!({ "columns": ["f1"], "min": 0.3 })).unwrap();
    assert_eq!(objects_of(&out).class_of[1], 0);
    // two attributes: the distances add as squares, d^2 = 2 gives slope^2 (0.04)
    let (o, s) = nearest_case(&f1, &f2, &[(1.0, f1[1] - s1, f2[1] - s2), (2.0, f1[3] + 10.0 * s1, f2[3] + 10.0 * s2)]);
    let t = table_of(&nearest_out(&o, &s, serde_json::json!({ "columns": ["f1", "f2"] })).unwrap());
    near(col(&t, "m_a")[1], 0.04, 1e-9, "d = sqrt(2)");
    // without colours (no mean_*, no index) the default is every numeric attribute common to both tables but id, class,
    // cells, area and the centroid: the same
    let t2 = table_of(&nearest_out(&o, &s, serde_json::json!({})).unwrap());
    close(&col(&t2, "m_a"), &col(&t, "m_a"), "default columns");
}

/// The share of the cells whose class (by name, `names[truth - 1]`) is the truth's.
fn named_accuracy(out: &Outputs, truth: &Map, names: &[&str]) -> f64 {
    let Value::Map(m) = &out["map"] else { panic!("map") };
    let o = objects_of(out);
    let hit = m
        .data
        .iter()
        .zip(truth.data.iter())
        .filter(|(p, t)| !p.is_nan() && o.classes[**p as usize - 1].name == names[**t as usize - 1])
        .count();
    hit as f64 / truth.data.len() as f64
}

/// Sharp blocks (nlm.image without blur), segmented and with spectral attributes (mean per band, ndvi, ndwi).
fn sharp_case(segmenter: &str, params: serde_json::Value) -> (Arc<Objects>, Map) {
    let image = run("nlm.image", vec![("classes", blocky())], serde_json::json!({ "blur": 0 })).unwrap();
    let (stack, truth) = (image["image"].clone(), image["truth"].clone());
    let Value::Map(truth) = truth else { panic!("truth") };
    let o = objects_of(&run(segmenter, vec![("image", stack.clone())], params).unwrap());
    let out = run(
        "objects.features",
        vec![("objects", Value::Objects(o)), ("image", stack)],
        serde_json::json!({ "features": ["mean", "ndvi", "ndwi"] }),
    )
    .unwrap();
    (objects_of(&out), truth)
}

#[test]
fn rules_on_nlm_image_by_hand_written_thresholds_are_accurate() {
    // signatures (blue green red nir swir1 swir2): ndwi (green - nir) / (green + nir) is 0.43 for water and under -0.15
    // for the rest; ndvi is 0.82 forest, 0.62 grassland, 0.35 cropland, 0.125 urban; swir1 is 0.15 forest, 0.25
    // grassland
    let classes = serde_json::json!([
        { "name": "water", "rule": "above(ndwi, 0, 0.2)", "colour": "#2b6cb0" },
        { "name": "vegetation", "rule": "above(ndvi, 0.45, 0.55)" },
        { "name": "forest", "rule": "below(mean_swir1, 0.18, 0.22)", "parent": "vegetation" },
        { "name": "grassland", "rule": "above(mean_swir1, 0.18, 0.22)", "parent": "vegetation" },
        { "name": "cropland", "rule": "between(ndvi, 0.2, 0.28, 0.45, 0.5)" },
        { "name": "urban", "rule": "below(ndvi, 0.2, 0.28) and below(ndwi, 0, 0.2)" }
    ]);
    let names = ["water", "forest", "grassland", "cropland", "urban"];
    for (segmenter, params) in
        [("segment.slic", serde_json::json!({})), ("segment.multiresolution", serde_json::json!({ "scale": 5 }))]
    {
        let (o, truth) = sharp_case(segmenter, params);
        let out = rules_out(&o, classes.clone(), None).unwrap();
        let acc = named_accuracy(&out, &truth, &names);
        println!("rules, {segmenter}: {} objects, accuracy {acc:.4}", o.count);
        assert!(acc >= 0.8, "{segmenter}: accuracy {acc}");
        // the parent is not a class an object becomes
        let c = objects_of(&out);
        assert!(c.class_of.iter().all(|&k| k != 2));
        assert_eq!(c.classes.len(), 6);
    }
}

#[test]
fn nearest_on_nlm_image_by_samples_is_accurate() {
    for (segmenter, params) in
        [("segment.slic", serde_json::json!({})), ("segment.multiresolution", serde_json::json!({ "scale": 5 }))]
    {
        let (o, truth) = sharp_case(segmenter, params);
        let s = run(
            "objects.samples",
            vec![("objects", Value::Objects(o.clone())), ("samples", truth_points(&o, &truth, 3, 7))],
            serde_json::json!({}),
        )
        .unwrap();
        let train = table_of(&s);
        let bands: Vec<&str> = ["blue", "green", "red", "nir", "swir1", "swir2"].to_vec();
        let columns: Vec<String> = bands.iter().map(|b| format!("mean_{b}")).collect();
        let out = nearest_out(&o, &train, serde_json::json!({ "columns": columns })).unwrap();
        let acc = accuracy(&out, &truth);
        println!("nearest, {segmenter}: {} objects, {} training, accuracy {acc:.4}", o.count, train.rows.len());
        assert!(acc >= 0.9, "{segmenter}: accuracy {acc}");
        assert_eq!(objects_of(&out).classes.len(), 5);
        // the default feature space (the colours: means and indices) is as good
        let all = nearest_out(&o, &train, serde_json::json!({})).unwrap();
        let default = accuracy(&all, &truth);
        println!("nearest, {segmenter}, default columns: accuracy {default:.4}");
        assert!(default >= 0.9, "{segmenter}: default columns, accuracy {default}");
        assert_eq!(table_of(&all).rows.len(), o.count);
    }
}

// segment.threshold, segment.spectral_difference

fn threshold(image: Value, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    run("segment.threshold", vec![("image", image)], params)
}

fn column(t: &Table, name: &str) -> Vec<f64> {
    let k = t.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no column {name}"));
    t.rows.iter().map(|r| r[k]).collect()
}

/// The same operator on a pool of `threads`.
fn run_on(threads: usize, op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    pool.install(|| run(op, inputs, params).unwrap())
}

#[test]
fn threshold_cuts_a_band_into_classes_and_their_pieces() {
    #[rustfmt::skip]
    let data = vec![
        5.0,  5.0,  15.0, 15.0, 25.0, 5.0,
        5.0,  NAN,  15.0, 25.0, 25.0, 5.0,
        5.0,  5.0,  15.0, 15.0, 15.0, 5.0,
    ];
    let m = Value::Map(map(6, 3, data.clone()));
    let out = threshold(m.clone(), serde_json::json!({ "thresholds": [10, 20] })).unwrap();
    let (o, t) = (objects_of(&out), table_of(&out));
    assert_eq!(t.columns, ["id", "cells", "area", "class", "mean_image", "sd_image"]);
    assert_eq!(o.count, 4);
    assert_eq!(column(&t, "cells"), [5.0, 6.0, 3.0, 3.0]);
    assert_eq!(column(&t, "class"), [1.0, 2.0, 3.0, 1.0]);
    for (got, want) in column(&t, "mean_image").iter().zip([5.0, 15.0, 25.0, 5.0]) {
        assert!((got - want).abs() < 1e-9, "{got} against {want}");
    }
    assert!(column(&t, "sd_image").iter().all(|&x| x < 1e-6));
    assert_eq!(o.labels[7], 0, "a cell without a value is no object");
    connected(&o);
    let Value::Table(th) = &out["thresholds"] else { panic!("thresholds") };
    assert_eq!(column(th, "threshold"), [10.0, 20.0]);
    // a value at a threshold is in the class above it
    let edge = Value::Map(map(3, 1, vec![9.0, 10.0, 11.0]));
    let o = objects_of(&threshold(edge, serde_json::json!({ "thresholds": 10 })).unwrap());
    assert_eq!(o.count, 2);
    assert_eq!(o.cells, [1, 2]);
    // the same with a single number or with a list of one
    let one = Value::Map(map(3, 1, vec![9.0, 10.0, 11.0]));
    let o2 = objects_of(&threshold(one, serde_json::json!({ "thresholds": [10] })).unwrap());
    assert_eq!(o2.labels, o.labels);
}

#[test]
fn threshold_neighbours_join_classes_by_a_corner_with_8() {
    let m = Value::Map(map(2, 2, vec![1.0, 0.0, 0.0, 1.0]));
    for (n, count) in [(4, 4), (8, 2)] {
        let o = objects_of(&threshold(m.clone(), serde_json::json!({ "thresholds": [0.5], "neighbours": n })).unwrap());
        assert_eq!(o.count, count, "{n} neighbours");
    }
}

#[test]
fn threshold_takes_the_band_chosen() {
    let (w, h) = (4, 1);
    let s = stack(w, h, vec![vec![0.0, 0.0, 1.0, 1.0], vec![0.0, 1.0, 1.0, 0.0]]);
    let cells = |p: serde_json::Value| objects_of(&threshold(s.clone(), p).unwrap()).cells.clone();
    let base = serde_json::json!({ "thresholds": [0.5] });
    assert_eq!(cells(base.clone()), [2, 2]);
    assert_eq!(cells(serde_json::json!({ "thresholds": [0.5], "band": "b1" })), [2, 2]);
    assert_eq!(cells(serde_json::json!({ "thresholds": [0.5], "band": "b2" })), [1, 2, 1]);
    assert_eq!(cells(serde_json::json!({ "thresholds": [0.5], "band": 2 })), [1, 2, 1]);
    let t = table_of(&threshold(s.clone(), serde_json::json!({ "thresholds": [0.5], "band": "b2" })).unwrap());
    assert_eq!(t.columns, ["id", "cells", "area", "class", "mean_b2", "sd_b2"]);
    for bad in [serde_json::json!(0), serde_json::json!(3), serde_json::json!("nir"), serde_json::json!(1.5)] {
        assert!(threshold(s.clone(), serde_json::json!({ "thresholds": [0.5], "band": bad })).is_err());
    }
}

#[test]
fn otsu_splits_a_bimodal_image_in_the_gap() {
    let (w, h) = (40, 20);
    let image = step_image(w, h);
    let out = threshold(image, serde_json::json!({ "classes": 2 })).unwrap();
    let (o, t) = (objects_of(&out), table_of(&out));
    assert_eq!(o.count, 2);
    assert_eq!(column(&t, "class"), [1.0, 2.0]);
    assert_eq!(o.cells, [400, 400]);
    let Value::Table(th) = &out["thresholds"] else { panic!("thresholds") };
    let cut = column(th, "threshold");
    assert_eq!(cut.len(), 1);
    // the modes are at 10 and 90: the gap's middle
    assert!((cut[0] - 50.0).abs() < 5.0, "{cut:?}");
    // classes 2 is the default without thresholds
    let again = objects_of(&threshold(step_image(w, h), serde_json::json!({})).unwrap());
    assert_eq!(again.labels, o.labels);
}

#[test]
fn otsu_finds_the_levels_of_four_quadrants() {
    let image = quadrants(60, 40, 1, 2.0, 5);
    let out = threshold(image.clone(), serde_json::json!({ "classes": 4 })).unwrap();
    let o = objects_of(&out);
    assert_eq!(o.count, 4);
    assert_eq!(column(&table_of(&out), "class"), [1.0, 2.0, 3.0, 4.0]);
    let Value::Table(th) = &out["thresholds"] else { panic!("thresholds") };
    let cut = column(th, "threshold");
    for (c, want) in cut.iter().zip([25.0, 55.0, 85.0]) {
        assert!((c - want).abs() < 8.0, "{cut:?}");
    }
    // more bins, the same classes here
    let fine = objects_of(&threshold(image, serde_json::json!({ "classes": 4, "bins": 1024 })).unwrap());
    assert_eq!(fine.labels, o.labels);
}

#[test]
fn threshold_skips_cells_without_a_value() {
    let (w, h) = (40, 20);
    let Value::Stack(mut s) = step_image(w, h) else { panic!("stack") };
    let mut v = (*s.layers[0]).clone();
    for r in 0..h {
        v[r * w + 5] = NAN;
    }
    v[3 * w + 30] = NAN;
    s.layers[0] = Arc::new(v.clone());
    let o = objects_of(&threshold(Value::Stack(s), serde_json::json!({ "classes": 2 })).unwrap());
    for (i, x) in v.iter().enumerate() {
        assert_eq!(o.labels[i] == 0, x.is_nan(), "cell {i}");
    }
    // the column without a value cuts the left class in two; the one cell in the right class is a hole
    assert_eq!(o.count, 3);
}

#[test]
fn threshold_is_the_same_on_one_thread_and_eight() {
    let image = quadrants(130, 90, 2, 9.0, 3);
    for p in [serde_json::json!({ "classes": 3, "bins": 600 }), serde_json::json!({ "thresholds": [30, 60, 90] })] {
        let a = run_on(1, "segment.threshold", vec![("image", image.clone())], p.clone());
        let b = run_on(8, "segment.threshold", vec![("image", image.clone())], p.clone());
        assert_eq!(objects_of(&a).labels, objects_of(&b).labels);
        assert_eq!(table_of(&a).rows, table_of(&b).rows);
        let (Value::Table(ta), Value::Table(tb)) = (&a["thresholds"], &b["thresholds"]) else { panic!("thresholds") };
        assert_eq!(ta.rows, tb.rows);
    }
}

#[test]
fn threshold_checks_its_settings() {
    let image = || stack(4, 1, vec![vec![1.0, 2.0, 3.0, 4.0]]);
    for p in [
        serde_json::json!({ "thresholds": [] }),
        serde_json::json!({ "thresholds": [3, 2] }),
        serde_json::json!({ "thresholds": [2, 2] }),
        serde_json::json!({ "thresholds": ["a"] }),
        serde_json::json!({ "thresholds": [2], "classes": 3 }),
        serde_json::json!({ "classes": 1 }),
        serde_json::json!({ "classes": 9 }),
        serde_json::json!({ "bins": 4 }),
        serde_json::json!({ "bins": 100000 }),
        serde_json::json!({ "neighbours": 6 }),
        // more classes than levels
        serde_json::json!({ "classes": 5 }),
    ] {
        assert!(threshold(image(), p.clone()).is_err(), "{p}");
    }
    // Otsu on a constant band has nothing to cut
    let flat = Value::Map(map(3, 3, vec![2.0; 9]));
    assert!(threshold(flat.clone(), serde_json::json!({})).is_err());
    // given thresholds need no range
    assert_eq!(objects_of(&threshold(flat, serde_json::json!({ "thresholds": [1] })).unwrap()).count, 1);
}

fn spectral(
    labels: &Map,
    image: Value,
    params: serde_json::Value,
) -> terraflux_ops::Result<Outputs> {
    let (o, _) = segment(labels, 4);
    run("segment.spectral_difference", vec![("objects", Value::Objects(o)), ("image", image)], params)
}

/// A row of objects 1..=n, one cell each.
fn row_objects(n: usize) -> Map {
    map(n, 1, (1..=n).map(|k| k as f64).collect())
}

#[test]
fn spectral_difference_merges_the_closest_pairs_with_updated_means() {
    // 10 12 | 20 21 | 40: (3,4) goes first (1), then (1,2) (2); 11 against 20.5 is 9.5, too far
    let image = Value::Map(map(5, 1, vec![10.0, 12.0, 20.0, 21.0, 40.0]));
    let out = spectral(&row_objects(5), image, serde_json::json!({ "max_difference": 3 })).unwrap();
    let (o, t) = (objects_of(&out), table_of(&out));
    assert_eq!(o.count, 3);
    assert_eq!(o.cells, [2, 2, 1]);
    assert_eq!(column(&t, "mean_image"), [11.0, 20.5, 40.0]);
    assert_eq!(t.columns, ["id", "cells", "area", "mean_image", "sd_image"]);
    assert_eq!((0..5).map(|i| o.labels[i]).collect::<Vec<_>>(), [1, 1, 2, 2, 3]);
    // each step is 2: all the pairs tie, the lower ids go first (1, 2 -> mean 11), and the mean of the
    // merged object is then 3 from the next, so a chain of 2s does not run through
    let image = Value::Map(map(4, 1, vec![10.0, 12.0, 14.0, 16.0]));
    let o = objects_of(&spectral(&row_objects(4), image, serde_json::json!({ "max_difference": 2.5 })).unwrap());
    assert_eq!((0..4).map(|i| o.labels[i]).collect::<Vec<_>>(), [1, 1, 2, 2]);
}

#[test]
fn spectral_difference_merges_only_below_the_limit() {
    let image = Value::Map(map(2, 1, vec![10.0, 12.0]));
    let count = |limit: f64| {
        objects_of(&spectral(&row_objects(2), image.clone(), serde_json::json!({ "max_difference": limit })).unwrap())
            .count
    };
    assert_eq!(count(2.0), 2);
    assert_eq!(count(2.0001), 1);
}

#[test]
fn spectral_difference_weights_the_bands() {
    // the bands differ by 10 and 0: the weighted mean of the differences
    let image = stack(2, 1, vec![vec![0.0, 10.0], vec![0.0, 0.0]]);
    let count = |w: serde_json::Value, limit: f64| {
        let p = serde_json::json!({ "max_difference": limit, "weights": w });
        objects_of(&spectral(&row_objects(2), image.clone(), p).unwrap()).count
    };
    assert_eq!(count(serde_json::json!([1, 1]), 6.0), 1);
    assert_eq!(count(serde_json::json!([1, 1]), 5.0), 2);
    assert_eq!(count(serde_json::json!([1, 0]), 6.0), 2);
    assert_eq!(count(serde_json::json!([0, 1]), 1.0), 1);
}

#[test]
fn spectral_difference_never_merges_an_object_without_a_value() {
    let image = Value::Map(map(3, 1, vec![10.0, NAN, 10.0]));
    let o = objects_of(&spectral(&row_objects(3), image, serde_json::json!({ "max_difference": 100 })).unwrap());
    assert_eq!(o.count, 3);
}

#[test]
fn spectral_difference_joins_the_squares_of_a_chessboard_into_the_areas() {
    let (w, h) = (64, 64);
    let image = quadrants(w, h, 2, 3.0, 11);
    let squares = run("segment.chessboard", vec![("image", image.clone())], serde_json::json!({ "size": 4 })).unwrap();
    let before = objects_of(&squares);
    assert_eq!(before.count, 256);
    let inputs = || vec![("objects", Value::Objects(before.clone())), ("image", image.clone())];
    let out = run("segment.spectral_difference", inputs(), serde_json::json!({ "max_difference": 15 })).unwrap();
    let o = objects_of(&out);
    assert_eq!(o.count, 4);
    assert_eq!(table_of(&out).rows.len(), 4);
    connected(&o);
    // every object of the start is inside one of the result
    let mut parent = vec![0u32; before.count];
    for (&a, &b) in before.labels.iter().zip(o.labels.iter()) {
        let p = &mut parent[a as usize - 1];
        assert!(*p == 0 || *p == b, "object {a} split");
        *p = b;
    }
    // the same on one thread and on eight
    let (a, b) = (
        run_on(1, "segment.spectral_difference", inputs(), serde_json::json!({ "max_difference": 15 })),
        run_on(8, "segment.spectral_difference", inputs(), serde_json::json!({ "max_difference": 15 })),
    );
    assert_eq!(objects_of(&a).labels, objects_of(&b).labels);
    assert_eq!(table_of(&a).rows, table_of(&b).rows);
    // nothing under the limit: the objects stay
    let tiny = run("segment.spectral_difference", inputs(), serde_json::json!({ "max_difference": 0.001 })).unwrap();
    assert_eq!(objects_of(&tiny).labels, before.labels);
}

#[test]
fn spectral_difference_checks_its_inputs_and_settings() {
    let image = Value::Map(map(3, 1, vec![1.0, 2.0, 3.0]));
    for p in [
        serde_json::json!({}),
        serde_json::json!({ "max_difference": 0 }),
        serde_json::json!({ "max_difference": -1 }),
        serde_json::json!({ "max_difference": 1, "weights": [1, 1] }),
        serde_json::json!({ "max_difference": 1, "weights": [-1] }),
        serde_json::json!({ "max_difference": 1, "weights": [0] }),
    ] {
        assert!(spectral(&row_objects(3), image.clone(), p.clone()).is_err(), "{p}");
    }
    // objects of another grid
    let other = Value::Map(map(4, 1, vec![1.0, 2.0, 3.0, 4.0]));
    assert!(spectral(&row_objects(3), other, serde_json::json!({ "max_difference": 1 })).is_err());
    // something other than objects
    let e = run(
        "segment.spectral_difference",
        vec![("objects", image.clone()), ("image", image)],
        serde_json::json!({ "max_difference": 1 }),
    );
    assert!(e.is_err());
}

fn grow(image: Value, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    run("segment.region_growing", vec![("image", image)], params)
}

fn grow_labels(m: Vec<f64>, params: serde_json::Value) -> Vec<u32> {
    let n = m.len();
    let o = objects_of(&grow(Value::Map(map(n, 1, m)), params).unwrap());
    o.labels.to_vec()
}

#[test]
fn region_growing_merges_the_neighbours_under_the_threshold() {
    // scaled by the range 11: the gaps are 1/11 inside the pairs, 9/11 between them
    let m = vec![0.0, 1.0, 10.0, 11.0];
    assert_eq!(grow_labels(m.clone(), serde_json::json!({ "threshold": 0.1 })), [1, 1, 2, 2]);
    assert_eq!(grow_labels(m.clone(), serde_json::json!({ "threshold": 0.05 })), [1, 2, 3, 4]);
    assert_eq!(grow_labels(m.clone(), serde_json::json!({ "threshold": 1.0 })), [1, 1, 1, 1]);
    // 0 joins only the identical
    assert_eq!(grow_labels(vec![3.0, 3.0, 5.0, 5.0, 9.0], serde_json::json!({ "threshold": 0 })), [1, 1, 2, 2, 3]);
}

#[test]
fn region_growing_joins_the_mutually_closest_first() {
    // 0 4 7 (range 7): 4 and 7 are each other's closest, 0 is not 4's: the means 0 and 5.5 stay apart at 0.6
    let labels = grow_labels(vec![0.0, 4.0, 7.0], serde_json::json!({ "threshold": 0.6 }));
    assert_eq!(labels, [1, 2, 2]);
    // without the mutual condition everything under the threshold at the time of the visit joins
    let all = grow_labels(vec![0.0, 4.0, 7.0], serde_json::json!({ "threshold": 1.0, "mutual": false }));
    assert_eq!(all, [1, 1, 1]);
}

#[test]
fn region_growing_euclidean_and_manhattan_differ() {
    // bands scaled to 0-1: A (0, 0), B (0.6, 0.2), C (1, 1): A-B is 0.4 by Manhattan, 0.447 by Euclid
    let image = stack(3, 1, vec![vec![0.0, 0.6, 1.0], vec![0.0, 0.2, 1.0]]);
    let n = |sim: &str| {
        objects_of(&grow(image.clone(), serde_json::json!({ "threshold": 0.42, "similarity": sim })).unwrap()).count
    };
    assert_eq!(n("manhattan"), 2);
    assert_eq!(n("euclidean"), 3);
    assert!(grow(image, serde_json::json!({ "threshold": 0.42, "similarity": "cosine" })).is_err());
}

#[test]
fn region_growing_leaves_the_cells_without_a_value_out() {
    let labels = grow_labels(vec![3.0, 3.0, NAN, 3.0, 4.0], serde_json::json!({ "threshold": 0 }));
    assert_eq!(labels, [1, 1, 0, 2, 3]);
    // a band with no value in a cell counts in the others
    let image = stack(3, 1, vec![vec![0.0, NAN, 1.0], vec![0.0, 0.0, 1.0]]);
    let o = objects_of(&grow(image, serde_json::json!({ "threshold": 0.01 })).unwrap());
    assert_eq!(o.labels.to_vec(), [1, 1, 2]);
}

#[test]
fn region_growing_minsize_joins_the_small_to_the_most_similar() {
    let m = vec![0.0, 0.0, 4.0, 10.0, 10.0];
    assert_eq!(grow_labels(m.clone(), serde_json::json!({ "threshold": 0 })), [1, 1, 2, 3, 3]);
    assert_eq!(grow_labels(m.clone(), serde_json::json!({ "threshold": 0, "minsize": 2 })), [1, 1, 1, 2, 2]);
    // a fragment still small after a join is joined again
    assert_eq!(grow_labels(m, serde_json::json!({ "threshold": 0, "minsize": 5 })), [1, 1, 1, 1, 1]);
}

#[test]
fn region_growing_iterations_stop_the_passes() {
    // a gradient of equal steps: a pass joins the mutual pairs only, so one pass cannot make one object
    let m: Vec<f64> = (0..16).map(|k| k as f64).collect();
    let one = grow_labels(m.clone(), serde_json::json!({ "threshold": 0.2, "iterations": 1 }));
    let many = grow_labels(m, serde_json::json!({ "threshold": 0.2 }));
    let count = |v: &[u32]| *v.iter().max().unwrap();
    assert!(count(&one) > count(&many), "{one:?} {many:?}");
}

#[test]
fn region_growing_never_crosses_the_bounds() {
    let image = Value::Map(map(6, 1, vec![5.0; 6]));
    let bounds = Value::Map(map(6, 1, vec![1.0, 1.0, 2.0, 2.0, 2.0, NAN]));
    let out = run(
        "segment.region_growing",
        vec![("image", image.clone()), ("bounds", bounds)],
        serde_json::json!({ "threshold": 1 }),
    )
    .unwrap();
    assert_eq!(objects_of(&out).labels.to_vec(), [1, 1, 2, 2, 2, 3]);
    assert_eq!(objects_of(&grow(image.clone(), serde_json::json!({ "threshold": 1 })).unwrap()).count, 1);
    // bounds of another grid
    let other = Value::Map(map(5, 1, vec![1.0; 5]));
    let e = run(
        "segment.region_growing",
        vec![("image", image), ("bounds", other)],
        serde_json::json!({ "threshold": 1 }),
    );
    assert!(e.is_err());
}

#[test]
fn region_growing_starts_from_the_seeds_and_keeps_them_whole() {
    // the seeds' cells differ in value, yet each seed is one start region
    let image = Value::Map(map(6, 1, vec![0.0, 10.0, 0.0, 10.0, 0.0, 10.0]));
    let seeds = map(6, 1, vec![1.0, 1.0, NAN, NAN, 2.0, 2.0]);
    let run_seeds = |s: Value, p: serde_json::Value| {
        objects_of(&run("segment.region_growing", vec![("image", image.clone()), ("seeds", s)], p).unwrap())
    };
    let o = run_seeds(Value::Map(seeds.clone()), serde_json::json!({ "threshold": 0 }));
    assert_eq!(o.labels.to_vec(), [1, 1, 2, 3, 4, 4]);
    // the same from objects
    let (objects, _) = segment(&seeds, 4);
    let o = run_seeds(Value::Objects(objects), serde_json::json!({ "threshold": 0 }));
    assert_eq!(o.labels.to_vec(), [1, 1, 2, 3, 4, 4]);
    // two groups of a value apart are two seeds; seeds that merge on stay whole
    let (w, h) = (40, 30);
    let img = quadrants(w, h, 1, 2.0, 5);
    let sd = map(w, h, (0..w * h).map(|i| if i % w < 3 && i / w < 3 { 1.0 } else { NAN }).collect());
    let o = objects_of(
        &run("segment.region_growing", vec![("image", img), ("seeds", Value::Map(sd))], serde_json::json!({ "threshold": 0.1 }))
            .unwrap(),
    );
    let id = o.labels[0];
    assert!((0..w * h).filter(|&i| i % w < 3 && i / w < 3).all(|i| o.labels[i] == id));
    connected(&o);
    // seeds of another grid or a text
    let e = run(
        "segment.region_growing",
        vec![("image", image.clone()), ("seeds", Value::Map(map(5, 1, vec![1.0; 5])))],
        serde_json::json!({ "threshold": 0.1 }),
    );
    assert!(e.is_err());
}

#[test]
fn region_growing_finds_the_areas_of_a_noisy_image() {
    let (w, h) = (64, 64);
    let image = quadrants(w, h, 2, 3.0, 11);
    let out = grow(image.clone(), serde_json::json!({ "threshold": 0.1, "minsize": 40, "neighbours": 8 })).unwrap();
    let o = objects_of(&out);
    assert_eq!(o.count, 4, "{}", o.count);
    assert_eq!(table_of(&out).columns, ["id", "cells", "area", "mean_b1", "mean_b2", "sd_b1", "sd_b2"]);
    connected(&o);
    // with minsize every object has that many cells
    assert!(o.cells.iter().all(|&c| c >= 40));
    // the seed changes the order of the visits, not what the objects are
    let b = objects_of(&grow(image, serde_json::json!({ "threshold": 0.1, "minsize": 40, "seed": 7 })).unwrap());
    assert_eq!(b.count, 4);
}

#[test]
fn region_growing_is_the_same_on_one_thread_and_eight() {
    let image = quadrants(130, 90, 3, 6.0, 3);
    let seeds = map(130, 90, (0..130 * 90).map(|i| if i % 130 < 20 && (i / 130) % 7 == 0 { 1.0 } else { NAN }).collect());
    for p in [
        serde_json::json!({ "threshold": 0.05 }),
        serde_json::json!({ "threshold": 0.08, "neighbours": 8, "similarity": "manhattan", "minsize": 12 }),
        serde_json::json!({ "threshold": 0.04, "mutual": false, "seed": 3 }),
    ] {
        for s in [false, true] {
            let mut inputs = vec![("image", image.clone())];
            if s {
                inputs.push(("seeds", Value::Map(seeds.clone())));
            }
            let a = run_on(1, "segment.region_growing", inputs.clone(), p.clone());
            let b = run_on(8, "segment.region_growing", inputs, p.clone());
            assert_eq!(objects_of(&a).labels, objects_of(&b).labels, "{p} seeds {s}");
            assert_eq!(table_of(&a).rows, table_of(&b).rows);
        }
    }
}

#[test]
fn region_growing_checks_its_settings() {
    let image = || Value::Map(map(4, 1, vec![1.0, 2.0, 3.0, 4.0]));
    for p in [
        serde_json::json!({}),
        serde_json::json!({ "threshold": -0.1 }),
        serde_json::json!({ "threshold": 1.5 }),
        serde_json::json!({ "threshold": 0.1, "minsize": 0 }),
        serde_json::json!({ "threshold": 0.1, "iterations": 0 }),
        serde_json::json!({ "threshold": 0.1, "neighbours": 6 }),
        serde_json::json!({ "threshold": 0.1, "similarity": "chebyshev" }),
    ] {
        assert!(grow(image(), p.clone()).is_err(), "{p}");
    }
    assert!(grow(Value::Number(1.0), serde_json::json!({ "threshold": 0.1 })).is_err());
}

/// `region_growing` params with a tile side and a margin added.
fn tiled(p: serde_json::Value, tile: usize, margin: usize) -> serde_json::Value {
    let mut p = p;
    p["tile"] = tile.into();
    p["margin"] = margin.into();
    p
}

#[test]
fn region_growing_by_tiles_is_the_same_on_one_thread_and_eight() {
    let image = quadrants(130, 90, 3, 6.0, 3);
    let seeds = map(130, 90, (0..130 * 90).map(|i| if i % 130 < 60 && (i / 130) % 7 == 0 { 1.0 } else { NAN }).collect());
    let bounds = map(130, 90, (0..130 * 90).map(|i| ((i % 130) / 45) as f64).collect());
    for p in [
        serde_json::json!({ "threshold": 0.05 }),
        serde_json::json!({ "threshold": 0.08, "neighbours": 8, "similarity": "manhattan", "minsize": 12 }),
        serde_json::json!({ "threshold": 0.04, "mutual": false, "seed": 3 }),
    ] {
        for (s, b) in [(false, false), (true, false), (false, true), (true, true)] {
            let mut inputs = vec![("image", image.clone())];
            if s {
                inputs.push(("seeds", Value::Map(seeds.clone())));
            }
            if b {
                inputs.push(("bounds", Value::Map(bounds.clone())));
            }
            let p = tiled(p.clone(), 40, 8);
            let a = run_on(1, "segment.region_growing", inputs.clone(), p.clone());
            let c = run_on(8, "segment.region_growing", inputs, p.clone());
            assert_eq!(objects_of(&a).labels, objects_of(&c).labels, "{p} seeds {s} bounds {b}");
            assert_eq!(table_of(&a).rows, table_of(&c).rows);
            connected(&objects_of(&a));
        }
    }
}

#[test]
fn region_growing_by_tiles_equals_the_whole_on_blocks_of_one_value() {
    // 10 x 10 blocks, each constant and unlike its neighbours: tiles cut through them, the stitching joins them again
    let (w, h) = (95, 70);
    let data: Vec<f64> = (0..w * h).map(|i| (((i % w) / 10 + 2 * ((i / w) / 10)) % 5) as f64 * 10.0).collect();
    let image = Value::Map(map(w, h, data));
    let p = serde_json::json!({ "threshold": 0.05 });
    let whole = objects_of(&grow(image.clone(), p.clone()).unwrap());
    assert_eq!(whole.count, 10 * 7);
    for (t, m) in [(32, 8), (24, 4), (50, 20), (17, 1)] {
        let o = objects_of(&grow(image.clone(), tiled(p.clone(), t, m)).unwrap());
        assert_eq!(o.labels.to_vec(), whole.labels.to_vec(), "tile {t} margin {m}");
    }
}

#[test]
fn region_growing_by_tiles_finds_about_the_objects_of_the_whole() {
    let image = quadrants(160, 120, 3, 4.0, 5);
    for p in [serde_json::json!({ "threshold": 0.08 }), serde_json::json!({ "threshold": 0.12, "minsize": 6 })] {
        let whole = objects_of(&grow(image.clone(), p.clone()).unwrap()).count as f64;
        let o = objects_of(&grow(image.clone(), tiled(p.clone(), 64, 16)).unwrap());
        let gap = (o.count as f64 - whole).abs() / whole;
        assert!(gap < 0.06, "{p}: {} objects by tiles, {whole} whole", o.count);
        connected(&o);
    }
}

#[test]
fn region_growing_by_tiles_applies_minsize_at_the_stitching() {
    let image = quadrants(120, 100, 2, 5.0, 9);
    let p = tiled(serde_json::json!({ "threshold": 0.05, "minsize": 25, "neighbours": 8 }), 40, 10);
    let o = objects_of(&grow(image, p).unwrap());
    assert!(o.cells.iter().all(|&c| c >= 25), "{:?}", o.cells.iter().min());
    connected(&o);
}

#[test]
fn region_growing_with_a_tile_over_the_image_is_the_whole() {
    let image = quadrants(50, 40, 2, 4.0, 2);
    for p in [serde_json::json!({ "threshold": 0.07 }), serde_json::json!({ "threshold": 0.03, "minsize": 9, "seed": 4 })] {
        let a = grow(image.clone(), p.clone()).unwrap();
        for t in [50, 64, 1000] {
            let b = grow(image.clone(), tiled(p.clone(), t, 0)).unwrap();
            assert_eq!(objects_of(&a).labels, objects_of(&b).labels, "tile {t}");
            assert_eq!(table_of(&a).rows, table_of(&b).rows);
        }
    }
}

#[test]
fn region_growing_by_tiles_keeps_the_bounds_and_the_seeds_across_tiles() {
    let (w, h) = (100, 80);
    let image = quadrants(w, h, 2, 3.0, 4);
    // zones of 17 columns, cut by the tile borders of 30
    let zones = map(w, h, (0..w * h).map(|i| ((i % w) / 17) as f64).collect());
    let out = run(
        "segment.region_growing",
        vec![("image", image.clone()), ("bounds", Value::Map(zones))],
        tiled(serde_json::json!({ "threshold": 1 }), 30, 6),
    )
    .unwrap();
    let o = objects_of(&out);
    assert_eq!(o.count, 6);
    for i in 0..w * h {
        assert_eq!(o.labels[i], o.labels[(i / w) * w + (i % w) / 17 * 17], "cell {i}");
    }
    // a seed that crosses every tile border stays one object, whatever its values (threshold 0 merges nothing else)
    let bar = |i: usize| (i / w == 12 || i / w == 13) && (4..96).contains(&(i % w));
    let seeds = map(w, h, (0..w * h).map(|i| if bar(i) { 1.0 } else { NAN }).collect());
    for sd in [Value::Map(seeds.clone()), Value::Objects(segment(&seeds, 4).0)] {
        let out = run(
            "segment.region_growing",
            vec![("image", image.clone()), ("seeds", sd)],
            tiled(serde_json::json!({ "threshold": 0 }), 30, 6),
        )
        .unwrap();
        let o = objects_of(&out);
        let id = o.labels[12 * w + 4];
        assert!((0..w * h).filter(|&i| bar(i)).all(|i| o.labels[i] == id));
        assert_eq!(o.cells[id as usize - 1], 2 * 92);
        connected(&o);
    }
}

#[test]
fn region_growing_checks_its_tile_settings() {
    let image = || quadrants(20, 20, 1, 1.0, 1);
    for p in [serde_json::json!({ "tile": 3 }), serde_json::json!({ "tile": -1 }), serde_json::json!({ "tile": 8, "margin": -2 })] {
        let mut q = serde_json::json!({ "threshold": 0.1 });
        q.as_object_mut().unwrap().extend(p.as_object().unwrap().clone());
        assert!(grow(image(), q.clone()).is_err(), "{q}");
    }
}

fn quick(image: Value, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    run("segment.quickshift", vec![("image", image)], params)
}

/// Two flat halves, 0 and 10, of a 20 x 10 image.
fn halves() -> Value {
    stack(20, 10, vec![(0..200).map(|i| if i % 20 < 10 { 0.0 } else { 10.0 }).collect()])
}

#[test]
fn quickshift_cuts_two_flat_halves_apart_by_colour() {
    let o = objects_of(&quick(halves(), serde_json::json!({ "kernel_size": 2, "max_dist": 6, "ratio": 10 })).unwrap());
    assert_eq!(o.count, 2);
    for (i, &l) in o.labels.iter().enumerate() {
        assert_eq!(l, if i % 20 < 10 { 1 } else { 2 }, "cell {i}");
    }
    // colour is ignored with a ratio of 0: one mode for the whole image
    let o = objects_of(&quick(halves(), serde_json::json!({ "kernel_size": 2, "max_dist": 12, "ratio": 0 })).unwrap());
    assert_eq!(o.count, 1);
}

#[test]
fn quickshift_with_a_short_link_leaves_every_cell_alone() {
    let out = quick(halves(), serde_json::json!({ "kernel_size": 2, "max_dist": 0.5 })).unwrap();
    assert_eq!(objects_of(&out).count, 200);
    assert_eq!(table_of(&out).rows.len(), 200);
}

#[test]
fn quickshift_skips_the_cells_without_a_value() {
    let mut v: Vec<f64> = (0..200).map(|i| if i % 20 < 10 { 0.0 } else { 10.0 }).collect();
    v[5] = NAN;
    v[17 + 40] = NAN;
    let o = objects_of(&quick(stack(20, 10, vec![v]), serde_json::json!({ "kernel_size": 2, "max_dist": 6, "ratio": 10 })).unwrap());
    assert_eq!(o.labels[5], 0);
    assert_eq!(o.labels[57], 0);
    assert!(o.labels[6] != 0 && o.labels[56] != 0);
    connected(&o);
}

/// Quick shift by its definition in f64, with exact exponentials: the segments of cells by the roots (the trees cut into
/// 4-connected pieces).
fn quickshift_by_hand(v: &[f64], w: usize, h: usize, ratio: f64, sigma: f64, max_dist: f64) -> Vec<usize> {
    let rad = (3.0 * sigma).ceil() as i64;
    let d2 = |i: usize, j: usize| {
        let (dr, dc) = ((i / w) as f64 - (j / w) as f64, (i % w) as f64 - (j % w) as f64);
        (dr * dr + dc * dc, ratio * ratio * (v[i] - v[j]) * (v[i] - v[j]))
    };
    let dens: Vec<f64> = (0..w * h)
        .map(|i| {
            let mut s = 0.0;
            for (j, x) in v.iter().enumerate() {
                let (sp, co) = d2(i, j);
                if !x.is_nan() && sp <= (rad * rad) as f64 {
                    s += (-(sp + co) / (2.0 * sigma * sigma)).exp();
                }
            }
            s
        })
        .collect();
    let mut root: Vec<usize> = (0..w * h).collect();
    for i in 0..w * h {
        let mut best: Option<(f64, usize)> = None;
        for j in 0..w * h {
            let (sp, co) = d2(i, j);
            let higher = dens[j] > dens[i] || (dens[j] == dens[i] && j < i);
            if j != i && higher && sp + co <= max_dist * max_dist && best.is_none_or(|(d, _)| sp + co < d) {
                best = Some((sp + co, j));
            }
        }
        if let Some((_, j)) = best {
            root[i] = j;
        }
    }
    let top = |mut i: usize| {
        while root[i] != i {
            i = root[i];
        }
        i
    };
    let tops: Vec<usize> = (0..w * h).map(top).collect();
    // 4-connected pieces of each tree, numbered by the first cell
    let mut piece = vec![usize::MAX; w * h];
    let mut next = 0;
    for s in 0..w * h {
        if piece[s] != usize::MAX {
            continue;
        }
        let mut stack = vec![s];
        piece[s] = next;
        while let Some(i) = stack.pop() {
            let (r, c) = (i / w, i % w);
            let mut near = vec![];
            if r > 0 {
                near.push(i - w);
            }
            if r + 1 < h {
                near.push(i + w);
            }
            if c > 0 {
                near.push(i - 1);
            }
            if c + 1 < w {
                near.push(i + 1);
            }
            for j in near {
                if piece[j] == usize::MAX && tops[j] == tops[s] {
                    piece[j] = next;
                    stack.push(j);
                }
            }
        }
        next += 1;
    }
    piece
}

#[test]
fn quickshift_is_the_definition() {
    let (w, h) = (18, 14);
    let mut s = 77u64;
    let v: Vec<f64> = (0..w * h)
        .map(|i| (if i % w < 9 { 0.0 } else { 6.0 }) + gauss(&mut s, 1.0))
        .collect();
    for (ratio, sigma, max_dist) in [(1.0, 1.5, 5.0), (3.0, 2.0, 7.0), (0.5, 1.0, 3.0)] {
        let p = serde_json::json!({ "ratio": ratio, "kernel_size": sigma, "max_dist": max_dist, "standardise": false });
        let o = objects_of(&quick(stack(w, h, vec![v.clone()]), p).unwrap());
        let want = quickshift_by_hand(&v, w, h, ratio, sigma, max_dist);
        // the same partition: ids by the first cell on both sides
        assert_eq!(o.labels.iter().map(|&l| l as usize - 1).collect::<Vec<_>>(), want, "{ratio} {sigma} {max_dist}");
    }
}

#[test]
fn quickshift_is_the_same_on_one_thread_and_eight() {
    let image = quadrants(70, 50, 2, 5.0, 9);
    for p in [
        serde_json::json!({ "kernel_size": 2, "max_dist": 6, "ratio": 3 }),
        serde_json::json!({ "kernel_size": 3, "max_dist": 8, "sigma": 1, "standardise": false, "ratio": 0.1 }),
    ] {
        let a = run_on(1, "segment.quickshift", vec![("image", image.clone())], p.clone());
        let b = run_on(8, "segment.quickshift", vec![("image", image.clone())], p.clone());
        assert_eq!(objects_of(&a).labels, objects_of(&b).labels, "{p}");
        assert_eq!(table_of(&a).rows, table_of(&b).rows);
    }
}

#[test]
fn quickshift_finds_the_areas_of_a_noisy_image() {
    let out = quick(quadrants(64, 64, 2, 3.0, 11), serde_json::json!({ "kernel_size": 3, "max_dist": 12, "ratio": 8 })).unwrap();
    let o = objects_of(&out);
    connected(&o);
    assert_eq!(table_of(&out).rows.len(), o.count);
    // the four areas, each in a few objects at most, none across two areas
    assert!(o.count >= 4 && o.count < 40, "{}", o.count);
    let area = |i: usize| (i / 64 >= 32) as usize * 2 + (i % 64 >= 32) as usize;
    let mut of_area = vec![usize::MAX; o.count];
    for (i, &l) in o.labels.iter().enumerate() {
        let a = &mut of_area[l as usize - 1];
        assert!(*a == usize::MAX || *a == area(i), "object {l} in two areas");
        *a = area(i);
    }
}

#[test]
fn quickshift_checks_its_settings() {
    for p in [
        serde_json::json!({ "ratio": -1 }),
        serde_json::json!({ "kernel_size": 0 }),
        serde_json::json!({ "kernel_size": -2 }),
        serde_json::json!({ "max_dist": 0 }),
        serde_json::json!({ "sigma": -1 }),
    ] {
        assert!(quick(halves(), p.clone()).is_err(), "{p}");
    }
    assert!(quick(Value::Number(1.0), serde_json::json!({})).is_err());
}

fn optimize(image: Value, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    run("segment.optimize", vec![("image", image)], params)
}

fn scores_of(out: &Outputs) -> Table {
    let Value::Table(t) = &out["scores"] else { panic!("scores") };
    t.clone()
}

fn value_of(out: &Outputs) -> f64 {
    let Value::Number(x) = &out["value"] else { panic!("value") };
    *x
}

/// The lowest and highest of the numbers that are finite.
fn range_of(x: &[f64]) -> (f64, f64) {
    let g = x.iter().copied().filter(|y| y.is_finite());
    (g.clone().fold(f64::INFINITY, f64::min), g.fold(f64::NEG_INFINITY, f64::max))
}

#[test]
fn optimize_scores_the_runs_by_hand() {
    // 4 x 2 cells, one band: size 2 makes two objects {1 3 1 3} and {5 9 5 9}, size 4 one object
    let image = Value::Map(map(4, 2, vec![1.0, 3.0, 5.0, 9.0, 1.0, 3.0, 5.0, 9.0]));
    let out = optimize(
        image,
        serde_json::json!({ "segmenter": "segment.chessboard", "parameter": "size", "values": [2, 4] }),
    )
    .unwrap();
    let s = scores_of(&out);
    assert_eq!(s.columns, ["value", "objects", "variance", "moran", "objective"]);
    assert_eq!(col(&s, "value"), [2.0, 4.0]);
    assert_eq!(col(&s, "objects"), [2.0, 1.0]);
    // the variances 1 and 4 over four cells each; the means 2 and 7 that touch: Moran's I is -1
    assert_eq!(s.rows[0][2], 2.5);
    assert!((s.rows[0][3] + 1.0).abs() < 1e-12);
    // one object: the variance of the eight values, no Moran's I
    assert!((s.rows[1][2] - 8.75).abs() < 1e-12);
    assert!(s.rows[1][3].is_nan() && s.rows[1][4].is_nan());
    // only one run has scores: it is the best
    assert_eq!(value_of(&out), 2.0);
    assert_eq!(objects_of(&out).count, 2);
    assert_eq!(table_of(&out).columns, ["id", "cells", "area"]);
}

#[test]
fn optimize_picks_the_scale_that_finds_the_four_areas() {
    let image = quadrants(48, 48, 2, 3.0, 5);
    let values = [2.0, 4.0, 8.0, 16.0, 32.0, 64.0, 128.0, 400.0, 1000.0];
    let out = optimize(image.clone(), serde_json::json!({ "values": values, "params": { "shape": 0.1 } })).unwrap();
    let s = scores_of(&out);
    assert_eq!(s.rows.len(), values.len());
    assert_eq!(col(&s, "value"), values);
    let objects = col(&s, "objects");
    assert!(objects[0] > 100.0, "the finest scale over-segments: {objects:?}");
    assert_eq!(*objects.last().unwrap(), 1.0);
    assert_eq!(objects_of(&out).count, 4, "{s:?}");
    // the objects are those of a run of the segmenter at the chosen value
    let alone = multires(image, serde_json::json!({ "scale": value_of(&out), "shape": 0.1 }));
    assert_eq!(objects_of(&out).labels, objects_of(&alone).labels);
    assert_eq!(table_of(&out).rows, table_of(&alone).rows);
    // the best has the highest objective
    let obj = col(&s, "objective");
    let at = col(&s, "value").iter().position(|&v| v == value_of(&out)).unwrap();
    assert!(obj.iter().filter(|x| !x.is_nan()).all(|&x| x <= obj[at]));
}

#[test]
fn optimize_f_measure_and_the_range_of_values() {
    let image = quadrants(40, 40, 1, 3.0, 9);
    let a = optimize(image.clone(), serde_json::json!({ "from": 10, "to": 70, "step": 20 })).unwrap();
    let b = optimize(image.clone(), serde_json::json!({ "values": [10, 30, 50, 70] })).unwrap();
    assert_eq!(scores_of(&a).rows.len(), 4);
    for (x, y) in scores_of(&a).rows.iter().zip(&scores_of(&b).rows) {
        assert!(same(x, y));
    }
    assert_eq!(col(&scores_of(&a), "value"), [10.0, 30.0, 50.0, 70.0]);
    let f = optimize(image, serde_json::json!({ "values": [5, 10, 30, 50, 90], "objective": "f", "alpha": 2 })).unwrap();
    let s = scores_of(&f);
    let (v, m, obj) = (col(&s, "variance"), col(&s, "moran"), col(&s, "objective"));
    let ((lo, hi), (mlo, mhi)) = (range_of(&v), range_of(&m));
    for k in 0..v.len() {
        if m[k].is_nan() {
            assert!(obj[k].is_nan());
            continue;
        }
        let (sv, sm) = ((hi - v[k]) / (hi - lo), (mhi - m[k]) / (mhi - mlo));
        let want = if 4.0 * sm + sv > 0.0 { 5.0 * sv * sm / (4.0 * sm + sv) } else { 0.0 };
        assert!((obj[k] - want).abs() < 1e-12, "{k}: {} {want}", obj[k]);
    }
}

#[test]
fn optimize_runs_other_segmenters_and_keeps_their_fixed_settings() {
    let image = quadrants(40, 40, 2, 3.0, 2);
    let out = optimize(image.clone(), serde_json::json!({ "segmenter": "segment.slic", "values": [4, 8, 16] })).unwrap();
    assert_eq!(col(&scores_of(&out), "value"), [4.0, 8.0, 16.0]);
    let alone = run("segment.slic", vec![("image", image.clone())], serde_json::json!({ "size": value_of(&out) as i64 })).unwrap();
    assert_eq!(objects_of(&out).labels, objects_of(&alone).labels);
    let out = optimize(
        image.clone(),
        serde_json::json!({ "segmenter": "segment.slic", "values": [8], "params": { "compactness": 5 } }),
    )
    .unwrap();
    let alone = run("segment.slic", vec![("image", image)], serde_json::json!({ "size": 8, "compactness": 5 })).unwrap();
    assert_eq!(objects_of(&out).labels, objects_of(&alone).labels);
}

#[test]
fn optimize_is_the_same_on_one_thread_and_eight() {
    let image = quadrants(70, 50, 3, 4.0, 3);
    for p in [
        serde_json::json!({ "values": [3, 6, 12, 25, 50, 100, 300] }),
        serde_json::json!({ "values": [3, 6, 12, 25, 50, 100, 300], "objective": "f" }),
        serde_json::json!({ "segmenter": "segment.region_growing", "values": [0.02, 0.05, 0.1, 0.3] }),
    ] {
        let a = run_on(1, "segment.optimize", vec![("image", image.clone())], p.clone());
        let b = run_on(8, "segment.optimize", vec![("image", image.clone())], p.clone());
        assert_eq!(scores_of(&a).rows.len(), scores_of(&b).rows.len());
        for (x, y) in scores_of(&a).rows.iter().zip(&scores_of(&b).rows) {
            assert!(same(x, y), "{p}: {x:?} {y:?}");
        }
        assert_eq!(value_of(&a), value_of(&b), "{p}");
        assert_eq!(objects_of(&a).labels, objects_of(&b).labels, "{p}");
    }
}

#[test]
fn optimize_checks_its_settings() {
    let image = quadrants(16, 16, 1, 3.0, 1);
    for p in [
        serde_json::json!({}),
        serde_json::json!({ "values": [] }),
        serde_json::json!({ "values": "a" }),
        serde_json::json!({ "values": [10, "x"] }),
        serde_json::json!({ "values": [10, 20], "from": 1, "to": 5, "step": 1 }),
        serde_json::json!({ "from": 1, "to": 5 }),
        serde_json::json!({ "from": 5, "to": 1, "step": 1 }),
        serde_json::json!({ "from": 1, "to": 5, "step": 0 }),
        serde_json::json!({ "from": 1, "to": 5000, "step": 1 }),
        serde_json::json!({ "values": [10], "segmenter": "segment.nothing" }),
        serde_json::json!({ "values": [10], "segmenter": "segment.optimize" }),
        serde_json::json!({ "values": [10], "segmenter": "segment.from_map", "parameter": "neighbours" }),
        serde_json::json!({ "values": [10], "segmenter": "raster.read" }),
        serde_json::json!({ "values": [10], "segmenter": "segment.threshold" }),
        serde_json::json!({ "values": [10], "parameter": "nothing" }),
        serde_json::json!({ "values": [10], "params": [1] }),
        serde_json::json!({ "values": [10], "params": { "scale": 20 } }),
        serde_json::json!({ "values": [10], "objective": "g" }),
        serde_json::json!({ "values": [10], "objective": "f", "alpha": 0 }),
        serde_json::json!({ "values": [-10] }),
        serde_json::json!({ "values": [10], "params": { "shape": 5 } }),
    ] {
        assert!(optimize(image.clone(), p.clone()).is_err(), "{p}");
    }
    assert!(optimize(Value::Number(1.0), serde_json::json!({ "values": [10] })).is_err());
    // one object for every value: no score to choose by
    let flat = Value::Map(map(8, 8, vec![1.0; 64]));
    assert!(optimize(flat, serde_json::json!({ "values": [10, 20] })).is_err());
}

/// The marked objects of objects.samples' output and its table: the samples of a case, `per` objects of each class.
fn folds_case(per: usize, seed: u64) -> (Arc<Objects>, Value, Table) {
    let (o, truth) = obia_case("segment.slic", serde_json::json!({}));
    let s = run(
        "objects.samples",
        vec![("objects", Value::Objects(o.clone())), ("samples", truth_points(&o, &truth, per, seed))],
        serde_json::json!({}),
    )
    .unwrap();
    (objects_of(&s), Value::Table(table_of(&s)), table_of(&s))
}

fn folds_of(marked: &Arc<Objects>, table: &Value, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    run("objects.folds", vec![("objects", Value::Objects(marked.clone())), ("samples", table.clone())], params)
}

#[test]
fn folds_keep_neighbours_together_balance_the_classes_and_follow_the_seed() {
    let (marked, table, t) = folds_case(12, 5);
    for (per, steps) in [(12, 1usize), (3, 2)] {
        let (marked, table, t) = if per == 12 { (marked.clone(), table.clone(), t.clone()) } else { folds_case(per, 5) };
        let ids: Vec<usize> = (0..marked.count).filter(|&i| marked.class_of[i] != 0).collect();
        assert_eq!(ids.len(), t.rows.len());
        let class = t.column("class").unwrap();
        let out = folds_of(&marked, &table, serde_json::json!({"method": "neighbours", "k": 3, "steps": steps})).unwrap();
        let folded = table_of(&out);
        assert_eq!(folded.columns.last().unwrap(), "fold");
        // (the samples' own columns are as they were)
        assert_eq!(&folded.columns[..t.columns.len()], &t.columns[..]);
        let fold = folded.column("fold").unwrap();
        assert!(fold.iter().all(|f| (1.0..=3.0).contains(f)));
        for f in 1..=3 {
            assert!(fold.contains(&(f as f64)), "steps {steps}: an empty fold {f}");
        }
        // two sample objects within `steps` steps of each other are in one fold
        let at: std::collections::HashMap<usize, usize> = ids.iter().enumerate().map(|(p, &i)| (i, p)).collect();
        for (p, &i) in ids.iter().enumerate() {
            let mut frontier = vec![i];
            let mut seen = std::collections::HashSet::from([i]);
            for _ in 0..steps {
                let mut next = Vec::new();
                for &a in &frontier {
                    for &(nb, _) in marked.neighbours_of(a as u32 + 1) {
                        let b = nb as usize - 1;
                        if seen.insert(b) {
                            next.push(b);
                            if let Some(&q) = at.get(&b) {
                                assert_eq!(fold[p], fold[q], "steps {steps}: objects {} and {} apart", i + 1, b + 1);
                            }
                        }
                    }
                }
                frontier = next;
            }
        }
        // the counts table: the objects of each fold add up, and so do those of each class
        let counts = table_of_port(&out, "counts");
        assert_eq!(counts.rows.iter().map(|r| r[1]).sum::<f64>() as usize, ids.len());
        // the classes are spread over the folds: no class sits in a single fold
        let mut codes = class.clone();
        codes.sort_by(f64::total_cmp);
        codes.dedup();
        for (c, &code) in codes.iter().enumerate() {
            let held = counts.rows.iter().filter(|r| r[3 + c] > 0.0).count();
            println!("steps {steps}: class {code} in {held} folds");
            // (with fewer samples and wider groups, a class can sit in one group: the op warns of it)
            if steps == 1 {
                assert!(held >= 2, "class {code} in {held} folds");
            }
        }
    }
    // the same seed, the same folds; another seed another split; any number of threads
    let p = serde_json::json!({"method": "neighbours", "k": 3, "seed": 7});
    let a = table_of(&folds_of(&marked, &table, p.clone()).unwrap()).column("fold").unwrap();
    let on = |threads: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        pool.install(|| table_of(&folds_of(&marked, &table, p.clone()).unwrap()).column("fold").unwrap())
    };
    assert_eq!(a, on(1));
    assert_eq!(a, on(8));
    let others: Vec<Vec<f64>> = (1..6)
        .map(|s| {
            table_of(&folds_of(&marked, &table, serde_json::json!({"method": "neighbours", "k": 3, "seed": s})).unwrap())
                .column("fold")
                .unwrap()
        })
        .collect();
    assert!(others.iter().any(|f| *f != a), "five seeds gave one split");
}

fn table_of_port(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port} is not a table") };
    t.clone()
}

#[test]
fn folds_by_random_blocks_and_clusters_fill_every_fold() {
    let (marked, table, t) = folds_case(12, 5);
    let n = t.rows.len();
    // random: stratified, so the folds hold the same number of objects within one, and each class within one
    let out = folds_of(&marked, &table, serde_json::json!({"method": "random", "k": 4, "seed": 2})).unwrap();
    let counts = table_of_port(&out, "counts");
    let sizes: Vec<f64> = counts.rows.iter().map(|r| r[1]).collect();
    assert!(sizes.iter().all(|s| (s - n as f64 / 4.0).abs() < 1.0 + 1e-9), "{sizes:?}");
    for c in 3..counts.columns.len() {
        let per: Vec<f64> = counts.rows.iter().map(|r| r[c]).collect();
        let (lo, hi) = (per.iter().cloned().fold(f64::INFINITY, f64::min), per.iter().cloned().fold(0.0, f64::max));
        assert!(hi - lo <= 1.0, "class column {c}: {per:?}");
    }
    // blocks and clusters of the centroids: a fold each, and the same numbers twice
    for method in ["blocks", "clusters"] {
        let p = serde_json::json!({"method": method, "k": 3, "seed": 4});
        let a = table_of(&folds_of(&marked, &table, p.clone()).unwrap());
        let fold = a.column("fold").unwrap();
        for f in 1..=3 {
            assert!(fold.contains(&(f as f64)), "{method}: an empty fold {f}");
        }
        assert_eq!(fold, table_of(&folds_of(&marked, &table, p).unwrap()).column("fold").unwrap());
    }
    // refusals
    let refused = |params: serde_json::Value| folds_of(&marked, &table, params).map(|_| ()).expect_err("refused").to_string();
    assert!(refused(serde_json::json!({"k": 1})).contains("k 1"));
    assert!(refused(serde_json::json!({"method": "hexagons"})).contains("neighbours, blocks, clusters or random"));
    assert!(refused(serde_json::json!({"k": 500})).contains("k 500"));
    // a table of another size is not these objects' samples
    let mut short = t.clone();
    short.rows.pop();
    let e = folds_of(&marked, &Value::Table(short), serde_json::json!({})).map(|_| ()).expect_err("refused").to_string();
    assert!(e.contains("marked"), "{e}");
}
