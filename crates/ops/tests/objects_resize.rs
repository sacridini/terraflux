//! `objects.resize`: a square grown by 2 cells, the exact Euclidean disc, a gap shared by the nearest (and its
//! ties), the mask and the classes that may be taken, opening and closing, filling holes, smoothing, the border
//! optimization, cells without a value or an object, the classes kept and the level dropped, and the same objects on
//! 1 and 8 threads.

use std::sync::Arc;

use terraflux_ops::objects::{NO_REGION, ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Key, Map, Outputs, Registry, Stack, Value};
use terraflux_raster::Grid;

fn grid(w: usize, h: usize) -> Arc<Grid> {
    Arc::new(Grid::new(w, h))
}

/// Objects from a region number per cell (0 = none): the connected sets of each region (4 neighbours).
fn regions(w: usize, h: usize, at: impl Fn(usize, usize) -> u64) -> Arc<Objects> {
    let region: Vec<u64> = (0..w * h)
        .map(|i| match at(i / w, i % w) {
            0 => NO_REGION,
            k => k,
        })
        .collect();
    Arc::new(Objects::from_regions(grid(w, h), &region, 4))
}

/// One object: the cells where `on` holds.
fn shape(w: usize, h: usize, on: impl Fn(usize, usize) -> bool) -> Arc<Objects> {
    regions(w, h, |r, c| u64::from(on(r, c)))
}

fn image(o: &Objects, bands: Vec<Vec<f64>>) -> Value {
    let keys = (0..bands.len()).map(|k| Key::Name(format!("b{}", k + 1))).collect();
    Value::Stack(Stack { grid: o.grid.clone(), keys, layers: bands.into_iter().map(Arc::new).collect() })
}

fn resize_with(
    o: &Arc<Objects>,
    mask: Option<Vec<f64>>,
    img: Option<Value>,
    params: serde_json::Value,
) -> (terraflux_ops::Result<Outputs>, Vec<String>) {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let mut inputs = vec![("objects".to_string(), Value::Objects(o.clone()))];
    if let Some(m) = mask {
        inputs.push(("mask".to_string(), Value::Map(Map::new(o.grid.clone(), m))));
    }
    if let Some(i) = img {
        inputs.push(("image".to_string(), i));
    }
    let args = Args { node: "n", inputs: inputs.into_iter().collect(), params: &params };
    let mut ctx = Ctx::default();
    let out = r.get("objects.resize").unwrap().run(&mut ctx, &args);
    (out, ctx.warnings.clone())
}

fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

fn resize(o: &Arc<Objects>, params: serde_json::Value) -> Arc<Objects> {
    objects_of(&resize_with(o, None, None, params).0.unwrap())
}

fn cells_of(o: &Objects) -> usize {
    o.labels.iter().filter(|&&l| l != 0).count()
}

fn names(n: &[&str]) -> Vec<ObjectClass> {
    n.iter().map(|n| ObjectClass { name: n.to_string(), colour: None }).collect()
}

#[test]
fn a_square_grows_by_two_cells() {
    // a 4 x 4 square at rows and columns 8..12
    let o = shape(20, 20, |r, c| (8..12).contains(&r) && (8..12).contains(&c));
    let (out, warnings) = resize_with(&o, None, None, serde_json::json!({ "mode": "grow", "steps": 2 }));
    let out = out.unwrap();
    let g = objects_of(&out);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(g.count, 1);
    // the sides reach 2 cells out; the corners are rounded (the exact disc of radius 2: (2,1) is out)
    assert_ne!(g.id_at(6, 9), 0);
    assert_eq!(g.id_at(5, 9), 0);
    assert_ne!(g.id_at(7, 7), 0);
    assert_eq!(g.id_at(6, 7), 0);
    assert_eq!(g.cells[0], 16 + 4 * 8 + 4);
    // the report: 16 cells before, 52 after, in one object
    let Value::Table(t) = &out["report"] else { panic!("report") };
    assert_eq!(t.columns, ["object", "before", "after", "pieces"]);
    assert_eq!(t.rows, vec![vec![1.0, 16.0, 52.0, 1.0]]);
    // the table has a row per object
    let Value::Table(tab) = &out["table"] else { panic!("table") };
    assert_eq!(tab.rows.len(), 1);
    // distance in map units on a unit cell, exact: 2.1 takes the same cells as 2 (the next ones are at 2.24)
    assert_eq!(&resize(&o, serde_json::json!({ "distance": 2.0 })).labels[..], &g.labels[..]);
    assert_eq!(&resize(&o, serde_json::json!({ "distance": 2.1 })).labels[..], &g.labels[..]);
    assert_ne!(&resize(&o, serde_json::json!({ "distance": 2.3 })).labels[..], &g.labels[..]);
    // nothing grown, nothing changed: the same objects
    let full = shape(6, 6, |_, _| true);
    assert!(Arc::ptr_eq(&resize(&full, serde_json::json!({ "steps": 3 })), &full));
}

#[test]
fn the_euclidean_disc_is_exact() {
    let o = shape(41, 41, |r, c| r == 20 && c == 20);
    for d in [3.0f64, 5.5, 9.0, 12.2] {
        let g = resize(&o, serde_json::json!({ "distance": d }));
        for r in 0..41 {
            for c in 0..41 {
                let inside = ((r as f64 - 20.0).powi(2) + (c as f64 - 20.0).powi(2)).sqrt() <= d + 1e-9;
                assert_eq!(g.id_at(r, c) != 0, inside, "distance {d} at ({r}, {c})");
            }
        }
    }
}

#[test]
fn two_objects_share_a_gap_by_the_nearest() {
    // A at columns 0..3, B at columns 8..11, five free columns between them (3 to 7), rows 1..4
    let o = regions(11, 5, |r, c| {
        if !(1..4).contains(&r) {
            0
        } else if c < 3 {
            1
        } else if c >= 8 {
            2
        } else {
            0
        }
    });
    assert_eq!(o.count, 2);
    let g = resize(&o, serde_json::json!({ "steps": 3 }));
    assert_eq!(g.count, 2);
    for r in 1..4 {
        assert_eq!(g.id_at(r, 3), 1);
        assert_eq!(g.id_at(r, 4), 1);
        assert_eq!(g.id_at(r, 6), 2);
        assert_eq!(g.id_at(r, 7), 2);
        // the middle one is 3 from each: deterministic, the same on every run
        assert_ne!(g.id_at(r, 5), 0);
        assert_eq!(g.id_at(r, 5), resize(&o, serde_json::json!({ "steps": 3 })).id_at(r, 5));
    }
    // ring by ring (a mask that allows everything): the tie goes to the lower id
    let (out, _) = resize_with(&o, Some(vec![1.0; 55]), None, serde_json::json!({ "steps": 3 }));
    let g = objects_of(&out.unwrap());
    for r in 1..4 {
        assert_eq!(g.id_at(r, 5), 1);
        assert_eq!(g.id_at(r, 6), 2);
    }
    // the neighbours of each side count: a cell between two objects takes the one with more cells around it
    let o = regions(9, 3, |r, c| match (r, c) {
        (0, 0) | (1, 0) | (2, 0) | (0, 1) => 1,
        (_, 8) | (_, 7) => 2,
        _ => 0,
    });
    let (out, _) = resize_with(&o, Some(vec![1.0; 27]), None, serde_json::json!({ "steps": 1 }));
    let g = objects_of(&out.unwrap());
    // (1, 1) has A at (0,0), (1,0), (2,0) and (0,1): A; (2, 1) has A at (1,0) and (2,0)
    assert_eq!(g.id_at(1, 1), g.id_at(1, 0));
    assert_eq!(g.id_at(2, 1), g.id_at(1, 0));
    assert_eq!(g.id_at(1, 6), g.id_at(1, 7));
}

#[test]
fn the_mask_stops_growth() {
    let o = shape(24, 16, |r, c| (6..10).contains(&r) && (4..8).contains(&c));
    // the mask: 0 at the column 10 (a wall) and 1 elsewhere
    let mask: Vec<f64> = (0..24 * 16).map(|i| if i % 24 == 10 { 0.0 } else { 1.0 }).collect();
    let (out, _) = resize_with(&o, Some(mask.clone()), None, serde_json::json!({ "steps": 5 }));
    let g = objects_of(&out.unwrap());
    for (i, &allowed) in mask.iter().enumerate() {
        if allowed == 0.0 {
            assert_eq!(g.labels[i], 0, "cell {i} is not allowed");
        }
    }
    // the cells before the wall are taken, those behind it (5 steps do not go round) are not
    assert_ne!(g.id_at(7, 9), 0);
    assert_eq!(g.id_at(7, 11), 0);
    assert_ne!(g.id_at(7, 0), 0);
    // without the mask the growth crosses
    assert_ne!(resize(&o, serde_json::json!({ "steps": 5 })).id_at(7, 11), 0);
    // a distance is rounded to cells
    let (out, _) = resize_with(&o, Some(mask), None, serde_json::json!({ "distance": 5.0 }));
    assert_eq!(&objects_of(&out.unwrap()).labels[..], &g.labels[..]);
    // a map that is not 0 and not NaN is allowed; NaN is not
    let mut m = vec![1.0; 24 * 16];
    m[7 * 24 + 9] = f64::NAN;
    let (out, _) = resize_with(&o, Some(m), None, serde_json::json!({ "steps": 1 }));
    let g = objects_of(&out.unwrap());
    assert_eq!(g.id_at(7, 8), 1);
    assert_eq!(g.id_at(7, 9), 0, "NaN is not allowed");
}

#[test]
fn into_takes_the_cells_of_a_class() {
    // forest at columns 0..4, scrub at columns 4..8, free from column 8, rows 0..5
    let o = regions(12, 5, |_, c| {
        if c < 4 {
            1
        } else if c < 8 {
            2
        } else {
            0
        }
    });
    let o = Arc::new(o.as_ref().clone().with_classes(names(&["forest", "scrub"]), vec![1, 2], Vec::new()).unwrap());
    // into none: the forest touches no free cell, nothing changes
    let g = resize(&o, serde_json::json!({ "classes": ["forest"], "steps": 2 }));
    assert!(Arc::ptr_eq(&g, &o));
    // into scrub: two columns of it go to the forest
    let (out, _) =
        resize_with(&o, None, None, serde_json::json!({ "classes": ["forest"], "into": ["scrub"], "steps": 2 }));
    let out = out.unwrap();
    let g = objects_of(&out);
    assert_eq!(g.count, 2);
    for r in 0..5 {
        assert_eq!(g.id_at(r, 5), g.id_at(r, 0));
        assert_ne!(g.id_at(r, 6), g.id_at(r, 0));
        assert_eq!(g.class_name(g.id_at(r, 6)), Some("scrub"));
        assert_eq!(g.class_name(g.id_at(r, 5)), Some("forest"));
    }
    let Value::Table(t) = &out["report"] else { panic!("report") };
    assert_eq!(t.rows, vec![vec![1.0, 20.0, 30.0, 1.0]]);
    // a single name is a list of one, and "none" is none
    assert_eq!(
        &resize(&o, serde_json::json!({ "classes": ["forest"], "into": "scrub", "steps": 2 })).labels[..],
        &g.labels[..]
    );
    assert!(Arc::ptr_eq(&resize(&o, serde_json::json!({ "classes": ["forest"], "into": "none", "steps": 2 })), &o));
    // the free cells of the classes that may be taken too: all of the scrub's neighbours go on
    let (e, _) = resize_with(&o, None, None, serde_json::json!({ "into": ["road"] }));
    assert!(e.unwrap_err().0.contains("no class"));
}

#[test]
fn open_cuts_a_one_cell_bridge_into_two_objects() {
    // two 6 x 6 squares joined by a bridge of one cell and six long
    let o = shape(30, 14, |r, c| {
        ((4..10).contains(&r) && ((2..8).contains(&c) || (14..20).contains(&c))) || (r == 6 && (8..14).contains(&c))
    });
    assert_eq!(o.count, 1);
    let o = Arc::new(o.as_ref().clone().with_classes(names(&["urban"]), vec![1], vec![vec![0.7]]).unwrap());
    let (out, warnings) = resize_with(&o, None, None, serde_json::json!({ "mode": "open", "steps": 1 }));
    let out = out.unwrap();
    let g = objects_of(&out);
    assert!(warnings.is_empty());
    assert_eq!(g.count, 2);
    assert_ne!(g.id_at(6, 7), 0, "the body is back");
    for c in 9..13 {
        assert_eq!(g.id_at(6, c), 0, "the bridge is gone at {c}");
    }
    // both pieces have the class (and membership) of the object
    assert_eq!(g.class_of, vec![1, 1]);
    assert_eq!(g.membership, vec![vec![0.7, 0.7]]);
    let Value::Table(t) = &out["report"] else { panic!("report") };
    assert_eq!(t.rows.len(), 1);
    assert_eq!(t.rows[0][3], 2.0, "one object ended in two");
    // shrink alone cuts it too, and a speck of one cell is dropped by the opening
    assert_eq!(resize(&o, serde_json::json!({ "mode": "shrink", "steps": 1 })).count, 2);
    let speck = shape(10, 10, |r, c| (r, c) == (5, 5));
    assert_eq!(resize(&speck, serde_json::json!({ "mode": "open", "steps": 1 })).count, 0);
}

#[test]
fn close_fills_a_gap() {
    // a block of 10 x 10 with a slit of two columns (8 and 9) open at the top, six rows deep
    let o = shape(24, 20, |r, c| (3..13).contains(&r) && (3..13).contains(&c) && !(r < 9 && (8..10).contains(&c)));
    assert_eq!(o.count, 1);
    let before = o.cells[0] as usize;
    let g = resize(&o, serde_json::json!({ "mode": "close", "steps": 1 }));
    assert_eq!(g.count, 1);
    // the slit is filled but for its mouth (the closing of the cross: the cells above it were not grown), and
    // nothing else was added
    assert_eq!(g.cells[0] as usize, before + 10);
    assert_ne!(g.id_at(5, 8), 0);
    assert_ne!(g.id_at(4, 9), 0);
    assert_eq!(g.id_at(3, 9), 0);
    assert_eq!(g.id_at(2, 8), 0);
    // two objects that face each other across a gap stay apart: close works inside an object
    let two = regions(16, 6, |r, c| {
        if !(1..5).contains(&r) {
            0
        } else if c < 4 {
            1
        } else if c >= 8 {
            2
        } else {
            0
        }
    });
    let g = resize(&two, serde_json::json!({ "mode": "close", "steps": 2 }));
    assert_eq!(g.count, 2);
    assert_eq!(g.id_at(2, 5), 0);
    assert_eq!(g.cells, two.cells);
}

#[test]
fn fill_takes_the_holes_by_size_and_not_those_at_the_edge() {
    // object 1: a block with a hole of 2 x 2 and one of 5 x 5; object 2: a U open to the bottom edge
    let o = regions(26, 14, |r, c| {
        let block = (1..13).contains(&r) && (1..13).contains(&c);
        let hole_a = (3..5).contains(&r) && (3..5).contains(&c);
        let hole_b = (6..11).contains(&r) && (6..11).contains(&c);
        let u = (6..14).contains(&r) && (15..22).contains(&c);
        let u_in = (9..14).contains(&r) && (17..20).contains(&c);
        if block && !hole_a && !hole_b {
            1
        } else if u && !u_in {
            2
        } else {
            0
        }
    });
    assert_eq!(o.count, 2);
    let small = resize(&o, serde_json::json!({ "mode": "fill", "max_hole": 10 }));
    assert_eq!(small.id_at(3, 3), 1, "the small hole is filled");
    assert_eq!(small.id_at(8, 8), 0, "the big one is not");
    assert_eq!(small.cells[0], o.cells[0] + 4);
    let all = resize(&o, serde_json::json!({ "mode": "fill" }));
    assert_eq!(all.id_at(3, 3), 1);
    assert_eq!(all.id_at(8, 8), 1);
    assert_eq!(all.cells[0], o.cells[0] + 4 + 25);
    // the U's inside touches the image's edge: no hole
    assert_eq!(all.id_at(11, 18), 0);
    assert_eq!(all.cells[1], o.cells[1]);
    // a hole with two objects around it is not filled
    let two = regions(12, 12, |r, c| {
        let ring = (2..10).contains(&r) && (2..10).contains(&c) && !((4..8).contains(&r) && (4..8).contains(&c));
        if ring && c < 6 {
            1
        } else if ring {
            2
        } else {
            0
        }
    });
    assert_eq!(two.count, 2);
    assert!(Arc::ptr_eq(&resize(&two, serde_json::json!({ "mode": "fill" })), &two));
    // only the classes that change are the ones that fill
    let o = Arc::new(o.as_ref().clone().with_classes(names(&["a", "b"]), vec![1, 2], Vec::new()).unwrap());
    assert!(Arc::ptr_eq(&resize(&o, serde_json::json!({ "mode": "fill", "classes": ["b"] })), &o));
}

#[test]
fn smooth_removes_a_spike_and_a_notch() {
    // a body with a one-cell spike into free space
    let o = shape(20, 20, |r, c| ((5..15).contains(&r) && (5..15).contains(&c)) || (r, c) == (4, 9));
    let g = resize(&o, serde_json::json!({ "mode": "smooth", "distance": 1.5 }));
    assert_eq!(g.id_at(4, 9), 0, "the spike is gone");
    assert_ne!(g.id_at(5, 9), 0, "the flat side stays");
    assert_ne!(g.id_at(10, 10), 0);
    assert_eq!(g.id_at(3, 3), 0, "nothing is added to the outside");
    assert_eq!(g.count, 1);
    // a spike of an object into the one beside it goes to that one
    let o = regions(20, 14, |r, c| {
        if !(2..12).contains(&r) {
            0
        } else if c < 10 || (r, c) == (7, 10) {
            1
        } else {
            2
        }
    });
    assert_eq!(o.count, 2);
    let g = resize(&o, serde_json::json!({ "mode": "smooth", "steps": 1 }));
    assert_eq!(g.id_at(7, 10), g.id_at(7, 12));
    assert_ne!(g.id_at(7, 10), g.id_at(7, 8));
    // classes limit the objects that change: the first one's spike is not touched when only the second changes
    let o = Arc::new(o.as_ref().clone().with_classes(names(&["a", "b"]), vec![1, 2], Vec::new()).unwrap());
    let g = resize(&o, serde_json::json!({ "mode": "smooth", "steps": 1, "classes": ["b"] }));
    assert_eq!(g.id_at(7, 10), g.id_at(7, 8));
}

/// A band of 0 to the left of column 10 and 10 from it on, 24 columns by 12 rows, and objects that cut at column 12.
fn two_regions() -> (Arc<Objects>, Value, Vec<f64>) {
    let o = regions(24, 12, |_, c| if c < 12 { 1 } else { 2 });
    let band: Vec<f64> = (0..24 * 12).map(|i| if i % 24 < 10 { 0.0 } else { 10.0 }).collect();
    let img = image(&o, vec![band.clone()]);
    (o, img, band)
}

#[test]
fn refine_moves_a_wrong_border_to_the_true_edge() {
    let (o, img, _) = two_regions();
    // one pass moves the cells of the border column
    let (out, _) = resize_with(&o, None, Some(img.clone()), serde_json::json!({ "mode": "refine", "steps": 1 }));
    let g = objects_of(&out.unwrap());
    assert_eq!(g.id_at(5, 11), 2);
    assert_eq!(g.id_at(5, 10), 1);
    // enough passes (the means move between them) reach the true edge, then nothing moves
    let (out, _) = resize_with(&o, None, Some(img.clone()), serde_json::json!({ "mode": "refine", "steps": 6 }));
    let out = out.unwrap();
    let g = objects_of(&out);
    assert_eq!(g.count, 2);
    for r in 0..12 {
        assert_eq!(g.id_at(r, 9), 1);
        assert_eq!(g.id_at(r, 10), 2);
    }
    // the table is of the new objects, with the band means
    let Value::Table(t) = &out["table"] else { panic!("table") };
    assert_eq!(t.rows.len(), 2);
    assert!(t.columns.iter().any(|c| c == "mean_b1"));
    let mean = t.columns.iter().position(|c| c == "mean_b1").unwrap();
    assert!(t.rows[0][mean].abs() < 1e-9 && (t.rows[1][mean] - 10.0).abs() < 1e-9, "{:?}", t.rows);
    // a correct border stays, and the same objects come back
    let right = regions(24, 12, |_, c| if c < 10 { 1 } else { 2 });
    let (out, _) = resize_with(&right, None, Some(img.clone()), serde_json::json!({ "mode": "refine", "steps": 6 }));
    assert!(Arc::ptr_eq(&objects_of(&out.unwrap()), &right));
    // refine needs the image and takes passes, not a distance
    let (e, _) = resize_with(&o, None, None, serde_json::json!({ "mode": "refine" }));
    assert!(e.unwrap_err().0.contains("image"));
    let (e, _) = resize_with(&o, None, Some(img), serde_json::json!({ "mode": "refine", "distance": 2.0 }));
    assert!(e.unwrap_err().0.contains("steps"));
}

#[test]
fn cells_without_a_value_or_an_object_stay_as_they_are() {
    let (o, img, mut band) = two_regions();
    // a cell of the border column that has no value stays with its object; its neighbours go
    band[5 * 24 + 11] = f64::NAN;
    let nan_img = image(&o, vec![band]);
    let (out, _) = resize_with(&o, None, Some(nan_img), serde_json::json!({ "mode": "refine", "steps": 6 }));
    let out = out.unwrap();
    let g = objects_of(&out);
    // the cell stays object 1's, now an island of its own: object 1 ended in two objects
    assert_ne!(g.id_at(5, 11), 0);
    assert_ne!(g.id_at(5, 11), g.id_at(5, 5));
    assert_eq!(g.id_at(4, 11), g.id_at(5, 12));
    assert_eq!(g.id_at(6, 11), g.id_at(5, 12));
    let Value::Table(t) = &out["report"] else { panic!("report") };
    assert_eq!(t.rows[0][3], 2.0);
    // cells of no object are not moved by refine or smooth, and are the free cells of grow and the holes of fill
    let o2 = shape(14, 14, |r, c| ((3..11).contains(&r) && (3..11).contains(&c)) && (r, c) != (7, 7));
    assert_eq!(o2.id_at(7, 7), 0);
    let s = resize(&o2, serde_json::json!({ "mode": "smooth", "steps": 1 }));
    assert_eq!(s.id_at(7, 7), 0);
    assert_eq!(resize(&o2, serde_json::json!({ "mode": "fill" })).id_at(7, 7), 1);
    let grown = resize(&o2, serde_json::json!({ "steps": 1 }));
    assert_eq!(grown.id_at(7, 7), 1);
    assert_eq!(grown.id_at(2, 6), 1);
    let (out, _) =
        resize_with(&o2, None, Some(image(&o2, vec![vec![1.0; 196]])), serde_json::json!({ "mode": "refine" }));
    assert!(Arc::ptr_eq(&objects_of(&out.unwrap()), &o2));
    // an image with no value anywhere moves nothing
    let (out, _) =
        resize_with(&o, None, Some(image(&o, vec![vec![f64::NAN; 24 * 12]])), serde_json::json!({ "mode": "refine" }));
    assert!(Arc::ptr_eq(&objects_of(&out.unwrap()), &o));
    let _ = img;
}

#[test]
fn the_classes_are_kept_and_the_level_above_is_dropped_with_a_warning() {
    let o = shape(20, 20, |r, c| (8..12).contains(&r) && (8..12).contains(&c));
    let parent = shape(20, 20, |_, _| true);
    let o = Arc::new(
        o.as_ref()
            .clone()
            .with_classes(
                vec![ObjectClass { name: "crown".into(), colour: Some([0, 200, 0]) }],
                vec![1],
                vec![vec![0.8]],
            )
            .unwrap()
            .with_parent(parent, vec![1])
            .unwrap(),
    );
    let (out, warnings) = resize_with(&o, None, None, serde_json::json!({ "steps": 2 }));
    let g = objects_of(&out.unwrap());
    assert!(g.cells[0] > 16);
    assert!(g.parent.is_none());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("level above"), "{warnings:?}");
    assert_eq!(g.classes, o.classes);
    assert_eq!(g.class_of, vec![1]);
    assert_eq!(g.membership, vec![vec![0.8]]);
    // a shrunk object is a change too: the level goes
    let (out, warnings) =
        resize_with(&o, None, None, serde_json::json!({ "mode": "shrink", "steps": 1, "classes": ["crown"] }));
    assert_eq!(objects_of(&out.unwrap()).count, 1);
    assert_eq!(warnings.len(), 1);
    // classes limits the objects that change
    let two = regions(24, 12, |r, c| {
        if !(3..9).contains(&r) {
            0
        } else if c < 8 {
            1
        } else if (14..22).contains(&c) {
            2
        } else {
            0
        }
    });
    let two = Arc::new(two.as_ref().clone().with_classes(names(&["a", "b"]), vec![1, 2], Vec::new()).unwrap());
    let g = resize(&two, serde_json::json!({ "steps": 1, "classes": ["a"] }));
    assert_eq!(g.cells[1], two.cells[1], "b is as it was");
    assert!(g.cells[0] > two.cells[0]);
    assert_eq!(g.class_name(1), Some("a"));
    assert_eq!(g.class_name(2), Some("b"));
    let (e, _) = resize_with(&two, None, None, serde_json::json!({ "classes": ["road"] }));
    assert!(e.unwrap_err().0.contains("no class"));
}

#[test]
fn the_settings_are_checked() {
    let o = shape(10, 10, |r, c| (r, c) == (5, 5));
    let err = |p: serde_json::Value| resize_with(&o, None, None, p).0.unwrap_err().0;
    assert!(err(serde_json::json!({ "distance": 2.0, "steps": 2 })).contains("not both"));
    assert!(err(serde_json::json!({ "distance": -1.0 })).contains("above 0"));
    assert!(err(serde_json::json!({ "steps": 0 })).contains("1 or more"));
    assert!(err(serde_json::json!({ "mode": "dilate" })).contains("mode"));
    assert!(err(serde_json::json!({ "neighbours": 6 })).contains("4 or 8"));
    let (out, warnings) =
        resize_with(&o, Some(vec![1.0; 100]), None, serde_json::json!({ "mode": "shrink", "max_hole": 3 }));
    assert!(out.is_ok());
    assert_eq!(warnings.len(), 2, "{warnings:?}");
}

/// Discs on a lattice, each its own object, and a smooth two-band image with their intensities.
fn field() -> (Arc<Objects>, Value) {
    let (w, h) = (200usize, 160usize);
    let mut centres = Vec::new();
    for gr in 0..8 {
        for gc in 0..10 {
            let (r, c) = (10.0 + gr as f64 * 19.0, 10.0 + gc as f64 * 19.0 + (gr % 3) as f64 * 2.0);
            centres.push((r, c, 6.0 + ((gr * 5 + gc * 3) % 4) as f64));
        }
    }
    let region = |r: usize, c: usize| -> u64 {
        for (k, &(cr, cc, rad)) in centres.iter().enumerate() {
            if (r as f64 - cr).powi(2) + (c as f64 - cc).powi(2) <= rad * rad {
                // a spike and a hole in some discs, for the filling and the smoothing
                let hole = (r as f64 - cr).abs() <= 1.0 && (c as f64 - cc).abs() <= 1.0 && k % 3 == 0;
                return if hole { 0 } else { k as u64 + 1 };
            }
        }
        0
    };
    let o = regions(w, h, region);
    let band = |a: f64, b: f64| -> Vec<f64> {
        (0..w * h).map(|i| ((i / w) as f64 * a).sin() * 20.0 + ((i % w) as f64 * b).cos() * 20.0).collect()
    };
    let img = image(&o, vec![band(0.21, 0.17), band(0.13, 0.29)]);
    (o, img)
}

#[test]
fn the_same_objects_on_1_and_8_threads() {
    let (o, img) = field();
    assert!(o.count >= 70, "{} blobs", o.count);
    let mask: Vec<f64> = (0..200 * 160).map(|i| if (i % 200 + i / 200) % 11 == 0 { 0.0 } else { 1.0 }).collect();
    let touching = resize(&o, serde_json::json!({ "distance": 6.0 }));
    let cases: Vec<(&str, serde_json::Value, bool)> = vec![
        ("grow", serde_json::json!({ "mode": "grow", "distance": 4.5 }), false),
        ("grow, masked", serde_json::json!({ "mode": "grow", "steps": 5 }), true),
        ("shrink", serde_json::json!({ "mode": "shrink", "distance": 2.0 }), false),
        ("open", serde_json::json!({ "mode": "open", "steps": 2 }), false),
        ("close", serde_json::json!({ "mode": "close", "steps": 3 }), false),
        ("close, masked", serde_json::json!({ "mode": "close", "steps": 3 }), true),
        ("fill", serde_json::json!({ "mode": "fill", "max_hole": 20 }), false),
        ("smooth", serde_json::json!({ "mode": "smooth", "distance": 2.5 }), false),
        ("refine", serde_json::json!({ "mode": "refine", "steps": 4 }), false),
    ];
    for (name, params, masked) in cases {
        // (refine needs objects that touch)
        let input = if name == "refine" { &touching } else { &o };
        let on = |n: usize| {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(n).build().unwrap();
            pool.install(|| {
                let m = masked.then(|| mask.clone());
                objects_of(&resize_with(input, m, Some(img.clone()), params.clone()).0.unwrap())
            })
        };
        let (a, b) = (on(1), on(8));
        assert_ne!(&a.labels[..], &input.labels[..], "{name} changed nothing");
        assert_eq!(a.count, b.count, "{name}");
        assert_eq!(&a.labels[..], &b.labels[..], "{name}");
        assert!(cells_of(&a) > 0);
    }
}
