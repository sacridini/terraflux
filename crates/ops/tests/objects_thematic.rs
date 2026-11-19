//! Thematic layers, the rest of them (`bounds` in every segmenter) and the thematic attributes: no object crosses a
//! zone for each segmenter, native or by the step after it; one zone is no bounds, bit for bit; the live window is
//! the run's; `objects.features` with `thematic` gives `th_mode` and `th_share`.

use std::sync::Arc;

use terraflux_ops::objects::{Objects, Whole, Window, segment_window_op};
use terraflux_ops::{Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::objects_zones::{
    assert_no_crossing, blocks_map, blocks_objects, crosses, features, lcg, map, objects_of, run, stack,
};

const W: usize = 60;
const H: usize = 48;

/// Three bands of noise: segmenters give many small objects, of which many cross the zones' edges.
fn busy() -> (Arc<Grid>, Vec<Arc<Vec<f64>>>) {
    let mut s = 17u64;
    let data = (0..3).map(|_| Arc::new((0..W * H).map(|_| 40.0 * lcg(&mut s)).collect::<Vec<f64>>())).collect();
    (Arc::new(Grid::new(W, H)), data)
}

/// The segmenters that got `bounds`: the operator, its settings, and whether it works natively (else a step after it).
fn cases() -> Vec<(&'static str, serde_json::Value, bool)> {
    vec![
        ("segment.slic", serde_json::json!({ "size": 9 }), false),
        ("segment.snic", serde_json::json!({ "size": 9 }), false),
        ("segment.felzenszwalb", serde_json::json!({ "k": 2.0, "min_size": 12 }), true),
        ("segment.watershed", serde_json::json!({ "sigma": 1.0, "compactness": 0.02 }), false),
        ("segment.quickshift", serde_json::json!({ "kernel_size": 2.0, "max_dist": 6.0 }), false),
        ("segment.meanshift", serde_json::json!({ "spatial_radius": 3.0, "range_radius": 0.5 }), false),
        ("segment.threshold", serde_json::json!({ "classes": 2, "neighbours": 8 }), false),
        ("segment.chessboard", serde_json::json!({ "size": 13 }), false),
        ("segment.quadtree", serde_json::json!({ "threshold": 36.0, "min_size": 4 }), false),
        ("segment.spectral_difference", serde_json::json!({ "max_difference": 1.2 }), true),
    ]
}

/// The inputs of a case besides `bounds`.
fn inputs(op: &str, img: &(Arc<Grid>, Vec<Arc<Vec<f64>>>)) -> Vec<(&'static str, Value)> {
    let mut v = vec![("image", stack(img))];
    if op == "segment.spectral_difference" {
        let board = run("segment.chessboard", vec![("image", stack(img))], serde_json::json!({ "size": 7 })).unwrap();
        v.push(("objects", Value::Objects(objects_of(&board))));
    }
    v
}

fn segment(
    op: &str,
    img: &(Arc<Grid>, Vec<Arc<Vec<f64>>>),
    bounds: Option<Value>,
    p: &serde_json::Value,
) -> Arc<Objects> {
    let mut i = inputs(op, img);
    i.extend(bounds.map(|b| ("bounds", b)));
    objects_of(&run(op, i, p.clone()).unwrap())
}

#[test]
fn no_object_crosses_a_zone_for_every_segmenter() {
    let img = busy();
    for (op, p, _) in cases() {
        let free = segment(op, &img, None, &p);
        for (name, (bounds, zone)) in [("a map", blocks_map()), ("vectors", features()), ("objects", blocks_objects())]
        {
            assert!(crosses(&free, &zone), "{op}, {name}: the objects alone must cross the zones ({})", free.count);
            let out = run(
                op,
                {
                    let mut i = inputs(op, &img);
                    i.push(("bounds", bounds));
                    i
                },
                p.clone(),
            )
            .unwrap();
            let o = objects_of(&out);
            assert_no_crossing(&o, &zone, &format!("{op}, {name}"));
            // the table is of the objects made
            let Value::Table(t) = &out["table"] else { panic!("table") };
            assert_eq!(t.rows.len(), o.count, "{op}, {name}: a row per object");
            let cells: Vec<f64> = o.cells.iter().map(|&c| c as f64).collect();
            let col = t.column("cells").unwrap();
            assert_eq!(col, cells, "{op}, {name}: the cells of the table");
        }
    }
}

#[test]
fn one_zone_is_no_bounds_for_every_segmenter() {
    let img = busy();
    let one = map(W, H, vec![7.0; W * H]);
    for (op, p, _) in cases() {
        let want = segment(op, &img, None, &p);
        assert!(want.count > 1, "{op}: {} objects", want.count);
        let got = segment(op, &img, Some(one.clone()), &p);
        assert_eq!(got.labels[..], want.labels[..], "{op}: one zone");
        assert_eq!(got.table.rows, want.table.rows, "{op}: one zone, the table");
    }
}

#[test]
fn the_live_window_on_the_whole_image_is_the_run_with_bounds() {
    let img = busy();
    let names = ["a".to_string(), "b".to_string(), "c".to_string()];
    let whole = Whole::of(&img.1);
    let full = Window { x0: 0, y0: 0, width: W, height: H };
    // a native one and ones by the step after
    for (op, p, _) in
        cases().into_iter().filter(|c| ["segment.felzenszwalb", "segment.slic", "segment.threshold"].contains(&c.0))
    {
        for (name, (bounds, zone)) in [("a map", blocks_map()), ("vectors", features()), ("objects", blocks_objects())]
        {
            let want = segment(op, &img, Some(bounds.clone()), &p);
            assert_no_crossing(&want, &zone, &format!("{op}, {name}"));
            let params = p.as_object().cloned().unwrap();
            let extra = [("bounds".to_string(), bounds)];
            let got =
                segment_window_op(&Registry::standard(), op, &params, &img.0, &img.1, &names, &whole, full, 1, &extra)
                    .unwrap();
            assert_eq!(got.labels[..], want.labels[..], "{op}, {name}: the window is the run with bounds");
        }
    }
}

// ---- the thematic attributes

/// The grid and the bands of an image, and objects on it.
type Squares = (Arc<Grid>, Vec<Arc<Vec<f64>>>, Arc<Objects>);

/// The image of `objects.features`, and the objects: squares of 12 cells.
fn squares() -> Squares {
    let img = busy();
    let o = objects_of(
        &run("segment.chessboard", vec![("image", stack(&img))], serde_json::json!({ "size": 12 })).unwrap(),
    );
    (img.0, img.1, o)
}

/// `objects.features` with a thematic layer: the objects it gives.
fn with_thematic(s: &Squares, thematic: Value, field: Option<&str>) -> terraflux_ops::Result<terraflux_ops::Outputs> {
    let img = (s.0.clone(), s.1.clone());
    let mut p = serde_json::json!({ "features": ["area"] });
    if let Some(f) = field {
        p["field"] = serde_json::json!(f);
    }
    run(
        "objects.features",
        vec![("objects", Value::Objects(s.2.clone())), ("image", stack(&img)), ("thematic", thematic)],
        p,
    )
}

/// The mode (the smallest on a tie) and its share, over the cells that have a value, of each object, the long way.
fn expected(o: &Objects, value: &[f64]) -> Vec<(f64, f64)> {
    (1..=o.count as u32)
        .map(|id| {
            let mut counts: Vec<(f64, usize)> = Vec::new();
            for (i, _) in o.labels.iter().enumerate().filter(|(_, l)| **l == id) {
                let v = value[i];
                if v.is_nan() {
                    continue;
                }
                match counts.iter_mut().find(|c| c.0 == v) {
                    Some(c) => c.1 += 1,
                    None => counts.push((v, 1)),
                }
            }
            let total: usize = counts.iter().map(|c| c.1).sum();
            if total == 0 {
                return (f64::NAN, f64::NAN);
            }
            counts.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.total_cmp(&b.0)));
            (counts[0].0, counts[0].1 as f64 / total as f64)
        })
        .collect()
}

fn same(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b
}

/// Two polygons (the left half, the top right quarter; the bottom right quarter is in none) with a number and a
/// text attribute.
fn vector() -> (Vector, Vec<f64>) {
    let square = |c0: f64, r0: f64, c1: f64, r1: f64| {
        Geometry::Polygon(vec![vec![[c0, -r0], [c1, -r0], [c1, -r1], [c0, -r1], [c0, -r0]]])
    };
    let v = Vector {
        geometries: vec![square(0.0, 0.0, 30.0, 48.0), square(30.0, 0.0, 60.0, 24.0)],
        attributes: vec![
            ("land".into(), Column::Numbers(vec![20.0, 10.0])),
            ("use".into(), Column::Texts(vec!["forest".into(), "crop".into()])),
        ],
        crs: None,
    };
    // the feature (0..) of each cell, NaN for none
    let cell = (0..W * H)
        .map(|i| {
            if i % W < 30 {
                0.0
            } else if i / W < 24 {
                1.0
            } else {
                f64::NAN
            }
        })
        .collect();
    (v, cell)
}

#[test]
fn th_mode_and_th_share_of_a_map_are_the_most_common_value_and_its_share() {
    let s = squares();
    let (layer, zone) = blocks_map();
    let value: Vec<f64> = zone.iter().map(|&z| if z < 0 { f64::NAN } else { z as f64 }).collect();
    let out = with_thematic(&s, layer, None).unwrap();
    let Value::Table(t) = &out["table"] else { panic!("table") };
    let (mode, share) = (t.column("th_mode").unwrap(), t.column("th_share").unwrap());
    let want = expected(&s.2, &value);
    assert!(want.iter().any(|w| w.1 < 1.0 && w.1 > 0.0), "some squares must cross the zones");
    for (k, w) in want.iter().enumerate() {
        assert!(same(mode[k], w.0), "object {}: mode {} for {}", k + 1, mode[k], w.0);
        assert!(same(share[k], w.1), "object {}: share {} for {}", k + 1, share[k], w.1);
    }
    // the table keeps its other columns, and the objects carry it
    assert!(t.columns.contains(&"area".to_string()));
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    assert!(o.table.columns.contains(&"th_mode".to_string()));
    assert!(!t.is_text("th_mode"));
}

#[test]
fn the_smallest_value_wins_a_tie() {
    // a square of 12 cells at columns 24..36 holds 6 cells of zone 1 and 6 of zone 2
    let s = squares();
    let value: Vec<f64> = (0..W * H).map(|i| if i % W < 30 { 5.0 } else { 3.0 }).collect();
    let out = with_thematic(&s, map(W, H, value), None).unwrap();
    let Value::Table(t) = &out["table"] else { panic!("table") };
    let at = s.2.id_at(0, 24) as usize - 1;
    assert_eq!(t.column("th_mode").unwrap()[at], 3.0);
    assert_eq!(t.column("th_share").unwrap()[at], 0.5);
}

#[test]
fn th_mode_of_vectors_is_the_attribute_or_the_feature_number() {
    let s = squares();
    let (v, cell) = vector();
    let layer = Value::Vector(Arc::new(v));
    // no field: the feature's number from 1; cells in no feature have no value
    let out = with_thematic(&s, layer.clone(), None).unwrap();
    let Value::Table(t) = &out["table"] else { panic!("table") };
    let value: Vec<f64> = cell.iter().map(|&f| f + 1.0).collect();
    for (k, w) in expected(&s.2, &value).iter().enumerate() {
        assert!(same(t.column("th_mode").unwrap()[k], w.0), "number, object {}", k + 1);
        assert!(same(t.column("th_share").unwrap()[k], w.1), "number, object {}", k + 1);
    }
    // a numeric attribute
    let out = with_thematic(&s, layer.clone(), Some("land")).unwrap();
    let Value::Table(t) = &out["table"] else { panic!("table") };
    let land = [20.0, 10.0];
    let value: Vec<f64> = cell.iter().map(|&f| if f.is_nan() { f } else { land[f as usize] }).collect();
    for (k, w) in expected(&s.2, &value).iter().enumerate() {
        assert!(same(t.column("th_mode").unwrap()[k], w.0), "land, object {}", k + 1);
        assert!(same(t.column("th_share").unwrap()[k], w.1), "land, object {}", k + 1);
    }
    // the tie of the square at columns 24..36, rows 0..12: 10 (the smaller) wins
    let at = s.2.id_at(0, 24) as usize - 1;
    assert_eq!(t.column("th_mode").unwrap()[at], 10.0);
    // a text attribute: codes in the order the texts come (forest 1, crop 2), named by the table's labels
    let out = with_thematic(&s, layer.clone(), Some("use")).unwrap();
    let Value::Table(t) = &out["table"] else { panic!("table") };
    assert!(t.is_text("th_mode"));
    assert_eq!(t.labels["th_mode"], ["forest", "crop"]);
    let codes = [1.0, 2.0];
    let value: Vec<f64> = cell.iter().map(|&f| if f.is_nan() { f } else { codes[f as usize] }).collect();
    for (k, w) in expected(&s.2, &value).iter().enumerate() {
        assert!(same(t.column("th_mode").unwrap()[k], w.0), "text, object {}", k + 1);
    }
    assert_eq!(t.label("th_mode", t.column("th_mode").unwrap()[s.2.id_at(0, 0) as usize - 1]), Some("forest"));
    // the errors say what is wrong
    let e = with_thematic(&s, layer.clone(), Some("nope")).err().unwrap().to_string();
    assert!(e.contains("nope") && e.contains("land") && e.contains("use"), "{e}");
    let e = with_thematic(&s, blocks_map().0, Some("land")).err().unwrap().to_string();
    assert!(e.contains("field") && e.contains("vectors"), "{e}");
    let e = with_thematic(&s, Value::Number(1.0), None).err().unwrap().to_string();
    assert!(e.contains("thematic"), "{e}");
}

#[test]
fn with_bounds_of_the_same_layer_th_share_is_one_and_th_mode_the_zone() {
    let img = busy();
    let (layer, zone) = blocks_map();
    let o = segment("segment.slic", &img, Some(layer.clone()), &serde_json::json!({ "size": 9 }));
    let s = (img.0.clone(), img.1.clone(), o.clone());
    let out = with_thematic(&s, layer, None).unwrap();
    let Value::Table(t) = &out["table"] else { panic!("table") };
    let (mode, share) = (t.column("th_mode").unwrap(), t.column("th_share").unwrap());
    let mut checked = 0;
    for (i, &l) in o.labels.iter().enumerate() {
        let k = l as usize - 1;
        if zone[i] < 0 {
            // (cells without a value: the objects of that zone have no mode)
            assert!(mode[k].is_nan() && share[k].is_nan());
        } else {
            assert_eq!(share[k], 1.0, "object {l}");
            assert_eq!(mode[k], zone[i] as f64, "object {l}");
            checked += 1;
        }
    }
    assert!(checked > W * H / 2);
}

#[test]
fn rules_and_the_table_read_th_mode_by_name() {
    let s = squares();
    let (v, _) = vector();
    let out = with_thematic(&s, Value::Vector(Arc::new(v)), Some("land")).unwrap();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    let classes = serde_json::json!([{ "name": "forest", "rule": "above(th_mode, 10, 15)" }]);
    let r =
        run("objects.rules", vec![("objects", Value::Objects(o.clone()))], serde_json::json!({ "classes": classes }))
            .unwrap();
    let Value::Objects(c) = &r["objects"] else { panic!("objects") };
    let t = o.table.column("th_mode").unwrap();
    let want = t.iter().filter(|&&m| m == 20.0).count();
    assert!(want > 0 && want < o.count);
    assert_eq!(c.class_of.iter().filter(|&&k| k == 1).count(), want);
    assert_eq!(terraflux_ops::objects::features_give(None, "th_mode"), Some(("thematic", true)));
}
