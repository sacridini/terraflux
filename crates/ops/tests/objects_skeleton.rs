//! The `skeleton` group of `objects.features`: a line, a rectangle, an L, a cross, a ring, the cell size, the `spur`
//! setting, a group left out by default, and the same numbers on 1 and 8 threads.

use std::sync::Arc;

use terraflux_ops::objects::{NO_REGION, Objects};
use terraflux_ops::{Args, Ctx, Map, Registry, Table, Value};
use terraflux_raster::Grid;

/// One object: the cells where `on` holds, on a grid of cells of side `cell`.
fn shape(w: usize, h: usize, cell: f64, on: impl Fn(usize, usize) -> bool) -> Arc<Objects> {
    let region: Vec<u64> = (0..w * h).map(|i| if on(i / w, i % w) { 1 } else { NO_REGION }).collect();
    let mut g = Grid::new(w, h);
    g.transform = [0.0, cell, 0.0, 0.0, 0.0, -cell];
    Arc::new(Objects::from_regions(Arc::new(g), &region, 8))
}

fn features(o: &Arc<Objects>, params: serde_json::Value) -> Table {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let image = Value::Map(Map::new(o.grid.clone(), vec![1.0; o.grid.cells()]));
    let inputs = [("objects".to_string(), Value::Objects(o.clone())), ("image".to_string(), image)];
    let args = Args { node: "n", inputs: inputs.into_iter().collect(), params: &params };
    let out = r.get("objects.features").unwrap().run(&mut Ctx::default(), &args).unwrap();
    let Value::Table(t) = &out["table"] else { panic!("table") };
    t.clone()
}

/// The skeleton columns of the first object.
fn skel(o: &Arc<Objects>) -> Table {
    features(o, serde_json::json!({ "features": ["skeleton"] }))
}

fn get(t: &Table, name: &str) -> f64 {
    t.column(name).unwrap_or_else(|| panic!("no column {name}: {:?}", t.columns))[0]
}

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} against {b}");
}

fn counts(t: &Table) -> (f64, f64, f64) {
    (get(t, "skel_branches"), get(t, "skel_ends"), get(t, "skel_junctions"))
}

#[test]
fn a_line_has_one_branch_and_no_curvature() {
    let o = shape(60, 11, 1.0, |r, c| r == 5 && (5..55).contains(&c));
    let t = skel(&o);
    near(get(&t, "skel_main_length"), 49.0, 1e-9, "length");
    assert_eq!(counts(&t), (1.0, 2.0, 0.0));
    near(get(&t, "skel_branch_length"), 49.0, 1e-9, "branch length");
    near(get(&t, "skel_curvature"), 0.0, 1e-12, "curvature");
    near(get(&t, "skel_curvature_sd"), 0.0, 1e-12, "curvature sd");
    near(get(&t, "skel_width"), 50.0 / 49.0, 1e-9, "width");
}

#[test]
fn a_rectangle_is_long_and_wide() {
    let o = shape(110, 20, 1.0, |r, c| (5..15).contains(&r) && (5..105).contains(&c));
    let t = skel(&o);
    let (len, width) = (get(&t, "skel_main_length"), get(&t, "skel_width"));
    assert!((85.0..=100.0).contains(&len), "length {len}");
    assert!((9.5..=11.5).contains(&width), "width {width}");
    near(get(&t, "skel_length_width"), len / width, 1e-9, "ratio");
    assert!(get(&t, "skel_curvature") < 0.01, "curvature {}", get(&t, "skel_curvature"));
}

#[test]
fn an_l_turns_by_a_right_angle() {
    let o = shape(60, 60, 1.0, |r, c| (c == 5 && (5..45).contains(&r)) || (r == 44 && (5..45).contains(&c)));
    let t = skel(&o);
    let len = get(&t, "skel_main_length");
    assert!((77.0..=79.0).contains(&len), "length {len}");
    near(get(&t, "skel_curvature") * len, std::f64::consts::FRAC_PI_2, 0.05, "total turning");
    assert_eq!(counts(&t), (1.0, 2.0, 0.0));
    assert!(get(&t, "skel_curvature_sd") > 0.0);
}

#[test]
fn a_cross_has_four_branches_and_a_junction() {
    for thick in [1usize, 5] {
        let o = shape(61, 61, 1.0, |r, c| {
            let (dr, dc) = (r.abs_diff(30), c.abs_diff(30));
            (dr <= thick / 2 && dc <= 25) || (dc <= thick / 2 && dr <= 25)
        });
        let t = skel(&o);
        assert_eq!(counts(&t), (4.0, 4.0, 1.0), "thickness {thick}");
        let len = get(&t, "skel_main_length");
        assert!((45.0..=52.0).contains(&len), "thickness {thick}: length {len}");
        near(get(&t, "skel_branch_length"), 25.0, 4.0, "mean branch");
    }
}

#[test]
fn a_ring_is_one_branch_with_no_ends() {
    let o = shape(61, 61, 1.0, |r, c| {
        let d = ((r as f64 - 30.0).powi(2) + (c as f64 - 30.0).powi(2)).sqrt();
        (12.0..=20.0).contains(&d)
    });
    let t = skel(&o);
    assert_eq!(counts(&t), (1.0, 0.0, 0.0));
    // the longest shortest path of a circle of radius 16 is half of it
    near(get(&t, "skel_main_length"), std::f64::consts::PI * 16.0, 6.0, "length");
}

#[test]
fn the_cell_size_counts() {
    let a = skel(&shape(60, 11, 1.0, |r, c| r == 5 && (5..55).contains(&c)));
    let b = skel(&shape(60, 11, 30.0, |r, c| r == 5 && (5..55).contains(&c)));
    near(get(&b, "skel_main_length"), 49.0 * 30.0, 1e-6, "length");
    near(get(&b, "skel_width"), 50.0 * 900.0 / (49.0 * 30.0), 1e-6, "width");
    // the diagonal step is sqrt 2 times the cell
    let d = skel(&shape(40, 40, 10.0, |r, c| r == c && r > 4 && r < 35));
    near(get(&d, "skel_main_length"), 29.0 * 2f64.sqrt() * 10.0, 1e-6, "diagonal");
    // the ratio does not depend on the unit
    near(get(&a, "skel_length_width"), get(&b, "skel_length_width"), 1e-6, "length / width");
}

#[test]
fn a_spur_is_pruned_and_spur_zero_keeps_it() {
    // a line with a two-cell stub in the middle
    let on = |r: usize, c: usize| (r == 20 && (5..45).contains(&c)) || (c == 25 && (18..20).contains(&r));
    let o = shape(50, 40, 1.0, on);
    let t = skel(&o);
    assert_eq!(counts(&t), (1.0, 2.0, 0.0));
    let t0 = features(&o, serde_json::json!({ "features": ["skeleton"], "spur": 0 }));
    assert_eq!(counts(&t0), (3.0, 3.0, 1.0));
}

#[test]
fn the_group_is_asked_for_and_a_tiny_object_still_has_numbers() {
    let o = shape(20, 20, 1.0, |r, c| (5..7).contains(&r) && (5..7).contains(&c));
    let basic = features(&o, serde_json::json!({}));
    assert!(basic.column("skel_main_length").is_none());
    let one = features(&o, serde_json::json!({ "features": ["skel_main_length", "skel_ends"] }));
    assert_eq!(one.columns.iter().filter(|c| c.starts_with("skel_")).count(), 2);
    near(get(&one, "skel_main_length"), 0.0, 1e-12, "a 2 by 2 square");
    let all = skel(&o);
    assert!(get(&all, "skel_width").is_nan());
    assert_eq!(get(&all, "skel_branches"), 1.0);
}

#[test]
fn the_same_numbers_on_one_and_eight_threads() {
    // many shapes of every kind, side by side
    let o = {
        let (w, h) = (300, 120);
        let region: Vec<u64> = (0..w * h)
            .map(|i| {
                let (r, c) = (i / w, i % w);
                let (br, bc) = (r / 60, c / 60);
                let (lr, lc) = (r % 60, c % 60);
                let (lr, lc): (usize, usize) = (lr, lc);
                let on = match (br + bc) % 4 {
                    0 => lr > 5 && lr < 50 && lc > 5 && lc < 50,
                    1 => (lr as i64 - lc as i64).abs() < 4 && lr > 3 && lr < 56,
                    2 => lr.abs_diff(30) < 3 || lc.abs_diff(30) < 3,
                    _ => (10.0..=25.0).contains(&((lr as f64 - 30.0).hypot(lc as f64 - 30.0))),
                };
                if on { (br * 5 + bc + 1) as u64 } else { NO_REGION }
            })
            .collect();
        Arc::new(Objects::from_regions(Arc::new(Grid::new(w, h)), &region, 8))
    };
    assert!(o.count >= 8);
    let run = |n: usize| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(n).build().unwrap();
        pool.install(|| skel(&o))
    };
    let (a, b) = (run(1), run(8));
    assert_eq!(a.columns, b.columns);
    for (x, y) in a.rows.iter().flatten().zip(b.rows.iter().flatten()) {
        assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{x} against {y}");
    }
    // (a solid square thins to a point or a short line, the other shapes to long ones)
    assert!(a.column("skel_main_length").unwrap().iter().filter(|l| **l > 20.0).count() >= 4);
}
