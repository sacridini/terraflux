//! `objects.split`: touching discs cut at the neck, a disc left whole, the neck and size limits, the classes, a surface
//! with two peaks, cells without a value, an object that fills the image, the classes kept and the level dropped, and
//! the same objects on 1 and 8 threads.

use std::sync::Arc;

use terraflux_ops::objects::{NO_REGION, ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;

fn grid(w: usize, h: usize) -> Arc<Grid> {
    Arc::new(Grid::new(w, h))
}

/// Objects: the connected sets of the cells where `on` holds (4 neighbours).
fn shape(w: usize, h: usize, on: impl Fn(usize, usize) -> bool) -> Arc<Objects> {
    let region: Vec<u64> = (0..w * h).map(|i| if on(i / w, i % w) { 1 } else { NO_REGION }).collect();
    Arc::new(Objects::from_regions(grid(w, h), &region, 4))
}

/// Whether a cell is in the disc of radius `r` around (row, col) `c`.
fn disc(c: (f64, f64), r: f64, row: usize, col: usize) -> bool {
    (row as f64 - c.0).powi(2) + (col as f64 - c.1).powi(2) <= r * r
}

fn split_with(
    o: &Arc<Objects>,
    surface: Option<Vec<f64>>,
    params: serde_json::Value,
) -> (terraflux_ops::Result<Outputs>, Vec<String>) {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let mut inputs = vec![("objects".to_string(), Value::Objects(o.clone()))];
    if let Some(s) = surface {
        inputs.push(("surface".to_string(), Value::Map(Map::new(o.grid.clone(), s))));
    }
    let args = Args { node: "n", inputs: inputs.into_iter().collect(), params: &params };
    let mut ctx = Ctx::default();
    let out = r.get("objects.split").unwrap().run(&mut ctx, &args);
    (out, ctx.warnings.clone())
}

fn split(o: &Arc<Objects>, params: serde_json::Value) -> Arc<Objects> {
    objects_of(&split_with(o, None, params).0.unwrap())
}

fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

/// Two discs of radius 10 whose centres are 16 cells apart.
fn pair() -> Arc<Objects> {
    shape(40, 24, |r, c| disc((12.0, 12.0), 10.0, r, c) || disc((12.0, 28.0), 10.0, r, c))
}

/// Two discs of radius 8, 12 apart: a wide neck.
fn dumbbell() -> Arc<Objects> {
    shape(32, 20, |r, c| disc((10.0, 10.0), 8.0, r, c) || disc((10.0, 22.0), 8.0, r, c))
}

#[test]
fn two_touching_discs_are_cut_at_the_neck() {
    let o = pair();
    assert_eq!(o.count, 1);
    let (out, warnings) = split_with(&o, None, serde_json::json!({}));
    let out = out.unwrap();
    let s = objects_of(&out);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(s.count, 2);
    // the cut is in the middle (column 20), each disc in its piece, and every cell of the object kept
    assert_ne!(s.id_at(12, 12), s.id_at(12, 28));
    assert_ne!(s.id_at(12, 12), 0);
    assert_ne!(s.id_at(12, 28), 0);
    assert!(s.id_at(12, 19) == s.id_at(12, 12) && s.id_at(12, 21) == s.id_at(12, 28));
    assert_eq!(s.cells.iter().sum::<u32>(), o.cells[0]);
    for i in 0..o.grid.cells() {
        assert_eq!(o.labels[i] != 0, s.labels[i] != 0);
    }
    // the report: object 1 in two pieces
    let Value::Table(t) = &out["report"] else { panic!("report") };
    assert_eq!(t.columns, ["object", "pieces"]);
    assert_eq!(t.rows, vec![vec![1.0, 2.0]]);
    // the table has a row per piece
    let Value::Table(tab) = &out["table"] else { panic!("table") };
    assert_eq!(tab.rows.len(), 2);
}

#[test]
fn a_single_disc_is_not_split() {
    let o = shape(30, 30, |r, c| disc((15.0, 15.0), 11.0, r, c));
    let (out, warnings) = split_with(&o, None, serde_json::json!({}));
    let out = out.unwrap();
    assert!(warnings.is_empty());
    let s = objects_of(&out);
    assert_eq!(s.count, 1);
    assert!(Arc::ptr_eq(&s, &o), "nothing split: the same objects");
    let Value::Table(t) = &out["report"] else { panic!("report") };
    assert!(t.rows.is_empty());
}

#[test]
fn max_neck_holds_a_dumbbell_with_a_wide_neck() {
    let o = dumbbell();
    assert_eq!(split(&o, serde_json::json!({})).count, 2, "without the limit it splits");
    assert_eq!(split(&o, serde_json::json!({ "max_neck": 0.5 })).count, 1, "a neck as wide as the parts stays");
    assert_eq!(split(&o, serde_json::json!({ "max_neck": 1.0 })).count, 2, "a neck narrower than the part's width cuts");
    // the narrow neck of the pair cuts even at 0.5
    assert_eq!(split(&pair(), serde_json::json!({ "max_neck": 0.9 })).count, 2);
}

#[test]
fn min_size_sends_a_small_piece_back_to_its_neighbour() {
    // a big disc with a small one on its side
    let o = shape(40, 26, |r, c| disc((13.0, 12.0), 10.0, r, c) || disc((13.0, 26.0), 4.0, r, c));
    let s = split(&o, serde_json::json!({ "h": 0.3 }));
    assert_eq!(s.count, 2);
    let small = s.cells.iter().min().copied().unwrap();
    assert!(small < 60);
    let s = split(&o, serde_json::json!({ "h": 0.3, "min_size": 60 }));
    assert_eq!(s.count, 1, "the small piece went back");
    let s = split(&o, serde_json::json!({ "h": 0.3, "min_size": small as i64 }));
    assert_eq!(s.count, 2, "a piece of min_size cells is not smaller");
}

#[test]
fn classes_limits_which_objects_split() {
    // two objects, each a pair of discs: the first a class "crown", the second "roof"
    let region: Vec<u64> = (0..40 * 50)
        .map(|i| {
            let (r, c) = (i / 40, i % 40);
            if r < 24 && (disc((12.0, 12.0), 10.0, r, c) || disc((12.0, 28.0), 10.0, r, c)) {
                1
            } else if r >= 26 && (disc((38.0, 12.0), 10.0, r, c) || disc((38.0, 28.0), 10.0, r, c)) {
                2
            } else {
                NO_REGION
            }
        })
        .collect();
    let o = Objects::from_regions(grid(40, 50), &region, 4);
    assert_eq!(o.count, 2);
    let names = |n: &[&str]| n.iter().map(|n| ObjectClass { name: n.to_string(), colour: None }).collect::<Vec<_>>();
    let o = Arc::new(o.with_classes(names(&["crown", "roof"]), vec![1, 2], Vec::new()).unwrap());
    assert_eq!(split(&o, serde_json::json!({})).count, 4);
    let s = split(&o, serde_json::json!({ "classes": ["roof"] }));
    assert_eq!(s.count, 3);
    assert_eq!(s.cells[0], o.cells[0], "the crown is whole");
    // the pieces keep the class of their object
    assert_eq!(s.class_name(1), Some("crown"));
    assert_eq!(s.class_name(2), Some("roof"));
    assert_eq!(s.class_name(3), Some("roof"));
    assert_eq!(s.classes, o.classes);
    // a name that is no class
    let (e, _) = split_with(&o, None, serde_json::json!({ "classes": ["road"] }));
    assert!(e.unwrap_err().0.contains("no class"));
}

/// A rectangle of 40 x 20 cells and a surface of two bumps in it (peaks 10 at column 10 and 8 at column 30).
fn two_peaks() -> (Arc<Objects>, Vec<f64>) {
    let o = shape(40, 20, |_, _| true);
    let z: Vec<f64> = (0..40 * 20)
        .map(|i| {
            let (r, c) = ((i / 40) as f64, (i % 40) as f64);
            let a = 10.0 * (-((r - 10.0).powi(2) + (c - 10.0).powi(2)) / 40.0).exp();
            let b = 8.0 * (-((r - 10.0).powi(2) + (c - 30.0).powi(2)) / 40.0).exp();
            a + b
        })
        .collect();
    (o, z)
}

#[test]
fn a_surface_with_two_peaks_gives_two_pieces() {
    let (o, z) = two_peaks();
    // by distance a rectangle is one piece
    assert_eq!(split(&o, serde_json::json!({})).count, 1);
    let (out, _) = split_with(&o, Some(z.clone()), serde_json::json!({ "by": "surface" }));
    let s = objects_of(&out.unwrap());
    assert_eq!(s.count, 2);
    assert_ne!(s.id_at(10, 10), s.id_at(10, 30));
    assert_eq!(s.id_at(10, 10), s.id_at(0, 0));
    // the dark centres: the same surface upside down
    let neg: Vec<f64> = z.iter().map(|v| -v).collect();
    let (out, _) = split_with(&o, Some(neg.clone()), serde_json::json!({ "by": "surface", "invert": true }));
    let t = objects_of(&out.unwrap());
    assert_eq!(t.count, 2);
    assert_eq!(&t.labels[..], &s.labels[..]);
    // without invert the negated surface has one centre (its peak is the flat corner): not the same pieces
    let (out, _) = split_with(&o, Some(neg), serde_json::json!({ "by": "surface" }));
    assert_ne!(&objects_of(&out.unwrap()).labels[..], &s.labels[..]);
    // a height above the valley's depth keeps it whole (peak 8 over a saddle of about 1.9)
    let (out, _) = split_with(&o, Some(z.clone()), serde_json::json!({ "by": "surface", "h": 7.0 }));
    assert_eq!(objects_of(&out.unwrap()).count, 1);
    // smoothed, the same two
    let (out, _) = split_with(&o, Some(z), serde_json::json!({ "by": "surface", "sigma": 1.5 }));
    assert_eq!(objects_of(&out.unwrap()).count, 2);
    // by surface needs the surface
    let (e, _) = split_with(&o, None, serde_json::json!({ "by": "surface" }));
    assert!(e.unwrap_err().0.contains("surface"));
}

/// Many touching blobs on a grid of 200 x 160 cells.
fn crowd() -> Arc<Objects> {
    let (w, h) = (200, 160);
    let mut centres = Vec::new();
    for gr in 0..8 {
        for gc in 0..5 {
            let (r, c) = (10.0 + gr as f64 * 19.0, 12.0 + gc as f64 * 38.0 + (gr % 3) as f64 * 3.0);
            centres.push((r, c, 7.0 + ((gr * 5 + gc * 3) % 4) as f64));
            centres.push((r + 2.0, c + 14.0 + (gc % 2) as f64 * 2.0, 6.0 + ((gr + gc) % 3) as f64));
        }
    }
    shape(w, h, |r, c| centres.iter().any(|&(cr, cc, rad)| disc((cr, cc), rad, r, c)))
}

#[test]
fn the_same_objects_on_1_and_8_threads() {
    let o = crowd();
    assert!(o.count >= 30, "{} blobs", o.count);
    let on = |n: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(n).build().unwrap();
        pool.install(|| split(&o, serde_json::json!({ "min_size": 20, "max_neck": 1.2 })))
    };
    let (a, b) = (on(1), on(8));
    assert!(a.count > o.count, "the blobs split ({} from {})", a.count, o.count);
    assert_eq!(a.count, b.count);
    assert_eq!(&a.labels[..], &b.labels[..]);
    // and by surface
    let z: Vec<f64> = (0..o.grid.cells()).map(|i| ((i / 200) as f64 * 0.7).sin() + ((i % 200) as f64 * 0.45).cos()).collect();
    let by = |n: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(n).build().unwrap();
        pool.install(|| objects_of(&split_with(&o, Some(z.clone()), serde_json::json!({ "by": "surface", "h": 0.3 })).0.unwrap()))
    };
    let (a, b) = (by(1), by(8));
    assert!(a.count > o.count);
    assert_eq!(&a.labels[..], &b.labels[..]);
}

#[test]
fn cells_without_a_value_or_an_object_stay_as_they_are() {
    // the pair with a hole of cells that belong to no object, at the neck's side
    let o = shape(40, 24, |r, c| {
        (disc((12.0, 12.0), 10.0, r, c) || disc((12.0, 28.0), 10.0, r, c)) && !((10..13).contains(&r) && (19..22).contains(&c))
    });
    assert_eq!(o.count, 1);
    let s = split(&o, serde_json::json!({}));
    assert_eq!(s.count, 2);
    for i in 0..o.grid.cells() {
        assert_eq!(o.labels[i] != 0, s.labels[i] != 0, "cell {i}");
    }
    assert_eq!(s.id_at(11, 20), 0);
    // a surface with NaN on a few cells of the rectangle: they join a piece, and nothing is lost
    let (o, mut z) = two_peaks();
    for c in 18..22 {
        z[10 * 40 + c] = f64::NAN;
    }
    z[5 * 40 + 10] = f64::NAN;
    let (out, _) = split_with(&o, Some(z), serde_json::json!({ "by": "surface" }));
    let s = objects_of(&out.unwrap());
    assert_eq!(s.count, 2);
    assert!(s.labels.iter().all(|&l| l != 0));
    assert_eq!(s.id_at(5, 10), s.id_at(10, 10));
    // a surface of NaN only: nothing to split by
    let (out, _) = split_with(&o, Some(vec![f64::NAN; 800]), serde_json::json!({ "by": "surface" }));
    assert_eq!(objects_of(&out.unwrap()).count, 1);
}

#[test]
fn an_object_that_covers_the_whole_image() {
    let o = shape(30, 30, |_, _| true);
    assert_eq!(o.count, 1);
    // its border is the image's edge: a square has no neck
    assert_eq!(split(&o, serde_json::json!({})).count, 1);
    // a surface still splits it
    let (o, z) = two_peaks();
    let (out, _) = split_with(&o, Some(z), serde_json::json!({ "by": "surface" }));
    assert_eq!(objects_of(&out.unwrap()).count, 2);
    // a long strip with an object all over it: the edges are its border, and its ridge is one centre
    let o = shape(60, 20, |_, _| true);
    assert_eq!(split(&o, serde_json::json!({})).count, 1);
}

#[test]
fn the_level_above_is_dropped_with_a_warning_and_the_classes_are_kept() {
    let o = pair();
    let parent = shape(40, 24, |_, _| true);
    let o = Arc::new(
        (*o).clone()
            .with_classes(vec![ObjectClass { name: "crown".into(), colour: Some([0, 200, 0]) }], vec![1], vec![vec![0.8]])
            .unwrap()
            .with_parent(parent, vec![1])
            .unwrap(),
    );
    let (out, warnings) = split_with(&o, None, serde_json::json!({}));
    let s = objects_of(&out.unwrap());
    assert_eq!(s.count, 2);
    assert!(s.parent.is_none());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("level above"), "{warnings:?}");
    assert_eq!(s.classes, o.classes);
    assert_eq!(s.class_of, vec![1, 1]);
    assert_eq!(s.membership, vec![vec![0.8, 0.8]]);
    // nothing split: the level stays and there is no warning
    let (out, warnings) = split_with(&o, None, serde_json::json!({ "min_size": 1000 }));
    assert!(warnings.is_empty());
    assert!(objects_of(&out.unwrap()).parent.is_some());
}
