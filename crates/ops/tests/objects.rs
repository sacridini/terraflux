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
