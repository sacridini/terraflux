//! `objects.fuse`: an L-shaped building split in three pieces joined by compactness, the threshold on a mean, the best
//! candidate of each seed against all of them, the pass limit, the refusal of what cannot be derived, the seeds'
//! class kept and the level dropped, the table made again, and the same objects on 1 and 8 threads.

use std::sync::Arc;

use terraflux_ops::objects::{ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Key, Outputs, Registry, Stack, Value};
use terraflux_raster::Grid;

fn grid(w: usize, h: usize) -> Arc<Grid> {
    Arc::new(Grid::new(w, h))
}

/// Objects from a region number per cell (0 = none): the connected sets of each region (4 neighbours).
fn regions(w: usize, h: usize, at: impl Fn(usize, usize) -> u64) -> Arc<Objects> {
    let region: Vec<u64> = (0..w * h)
        .map(|i| match at(i / w, i % w) {
            0 => terraflux_ops::objects::NO_REGION,
            k => k,
        })
        .collect();
    Arc::new(Objects::from_regions(grid(w, h), &region, 4))
}

fn image(o: &Objects, bands: Vec<Vec<f64>>) -> Value {
    let keys = (0..bands.len()).map(|k| Key::Name(format!("b{}", k + 1))).collect();
    Value::Stack(Stack { grid: o.grid.clone(), keys, layers: bands.into_iter().map(Arc::new).collect() })
}

fn classes(o: &Objects, names: &[&str], of: Vec<u32>) -> Arc<Objects> {
    let list = names.iter().map(|n| ObjectClass { name: n.to_string(), colour: None }).collect();
    Arc::new(o.clone().with_classes(list, of, Vec::new()).unwrap())
}

fn fuse_with(
    o: &Arc<Objects>,
    img: Option<Value>,
    params: serde_json::Value,
) -> (terraflux_ops::Result<Outputs>, Vec<String>) {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let mut inputs = vec![("objects".to_string(), Value::Objects(o.clone()))];
    if let Some(i) = img {
        inputs.push(("image".to_string(), i));
    }
    let args = Args { node: "n", inputs: inputs.into_iter().collect(), params: &params };
    let mut ctx = Ctx::default();
    let out = r.get("objects.fuse").unwrap().run(&mut ctx, &args);
    (out, ctx.warnings.clone())
}

fn fuse(o: &Arc<Objects>, img: Option<Value>, params: serde_json::Value) -> Arc<Objects> {
    objects_of(&fuse_with(o, img, params).0.unwrap())
}

fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

/// An L: the upright (rows 1..=10, columns 1..=3) in two pieces and the foot (rows 8..=10, columns 4..=10) the third.
fn building() -> Arc<Objects> {
    regions(14, 14, |r, c| match (r, c) {
        (1..=4, 1..=3) => 1,
        (5..=10, 1..=3) => 2,
        (8..=10, 4..=10) => 3,
        _ => 0,
    })
}

#[test]
fn an_l_shaped_building_in_three_pieces_joins_where_the_result_is_as_compact() {
    let o = building();
    assert_eq!(o.count, 3);
    // the two pieces of the upright make a rectangle (compactness 1, no worse than either); the foot would make an L
    // of 1.96, worse than the foot's own 1: it stays apart
    let f = fuse(&o, None, serde_json::json!({ "fitting": "compactness", "goal": "min" }));
    assert_eq!(f.count, 2);
    assert_eq!(f.id_at(2, 2), f.id_at(8, 2));
    assert_ne!(f.id_at(8, 2), f.id_at(9, 8));
    assert_eq!(f.cells.iter().map(|&c| c as usize).sum::<usize>(), 12 + 18 + 21);
    // not asking for an improvement, but for a compactness of at most 2: the whole building
    let f = fuse(
        &o,
        None,
        serde_json::json!({ "fitting": "compactness", "goal": "min", "improve": false, "threshold": 2.0 }),
    );
    assert_eq!(f.count, 1);
    assert_eq!(f.cells[0], 51);
    // at most 1.5: the L (1.96) is out, and so is the foot with the upright's lower piece (1.54)
    let f = fuse(
        &o,
        None,
        serde_json::json!({ "fitting": "compactness", "goal": "min", "improve": false, "threshold": 1.5 }),
    );
    assert_eq!(f.count, 2);
    assert_ne!(f.id_at(8, 2), f.id_at(9, 8));
}

#[test]
fn the_attributes_of_the_joined_object_are_those_of_its_cells() {
    // the joined L's compactness, as the fitting reads it, is its box (10 x 10) over its 51 cells: the threshold
    // sits between 100/51 = 1.9608 and the next
    let o = building();
    let ok = fuse(
        &o,
        None,
        serde_json::json!({ "fitting": "compactness", "goal": "min", "improve": false, "threshold": 1.961 }),
    );
    let no = fuse(
        &o,
        None,
        serde_json::json!({ "fitting": "compactness", "goal": "min", "improve": false, "threshold": 1.96 }),
    );
    assert_eq!(ok.count, 1);
    assert_eq!(no.count, 2);
    // the perimeter of the L: 2 * (10 + 10) = 40 edges, its shape index 40 / (4 sqrt 51)
    let p = 40.0 / (4.0 * 51f64.sqrt());
    let ok = fuse(
        &o,
        None,
        serde_json::json!({ "fitting": "shape_index", "goal": "min", "improve": false, "threshold": p + 1e-9 }),
    );
    let no = fuse(
        &o,
        None,
        serde_json::json!({ "fitting": "shape_index", "goal": "min", "improve": false, "threshold": p - 1e-9 }),
    );
    assert!(ok.count < no.count || ok.count == 1, "{} {}", ok.count, no.count);
    assert_eq!(ok.count, 1);
    assert_eq!(no.count, 2);
}

/// Three squares of 3 x 3 cells in a row (the middle one the neighbour of both).
fn strip(values: &[f64]) -> (Arc<Objects>, Value) {
    let n = values.len();
    let o = regions(3 * n, 3, |_, c| (c / 3 + 1) as u64);
    let band: Vec<f64> = (0..3 * n * 3).map(|i| values[(i % (3 * n)) / 3]).collect();
    let img = image(&o, vec![band]);
    (o, img)
}

#[test]
fn the_threshold_on_a_mean_decides_which_pairs_join() {
    let (o, img) = strip(&[10.0, 11.0, 50.0]);
    // 10 + 11 make a mean of 10.5, 11 + 50 of 30.5: only the second reaches 25 (and 10 with both makes 23.7: not)
    let f =
        fuse(&o, Some(img.clone()), serde_json::json!({ "fitting": "mean_b1", "improve": false, "threshold": 25.0 }));
    assert_eq!(f.count, 2);
    assert_ne!(f.id_at(1, 1), f.id_at(1, 4));
    assert_eq!(f.id_at(1, 4), f.id_at(1, 7));
    // the table is made again from the image: the joined object's mean is exact
    let t = &f.table;
    let j = t.columns.iter().position(|c| c == "mean_b1").unwrap();
    assert!((t.rows[1][j] - 30.5).abs() < 1e-9 && (t.rows[0][j] - 10.0).abs() < 1e-9, "{:?}", t.rows);
    // a deviation too: the joined 11 and 50 have a standard deviation of 19.5
    let g = fuse(
        &o,
        Some(img.clone()),
        serde_json::json!({ "fitting": "sd_b1", "goal": "min", "improve": false, "threshold": 5.0 }),
    );
    assert_eq!(g.count, 2);
    assert_eq!(g.id_at(1, 1), g.id_at(1, 4));
    // below the threshold for min of mean: with a goal min and a threshold of 10.6, 10 + 11 join, 11 + 50 do not
    let h = fuse(
        &o,
        Some(img),
        serde_json::json!({ "fitting": "mean_b1", "goal": "min", "improve": false, "threshold": 10.6 }),
    );
    assert_eq!(h.count, 2);
    assert_eq!(h.id_at(1, 1), h.id_at(1, 4));
}

/// Two seeds and three candidates in a row: C3 S2 C1 S1 C2.
fn row_of_five() -> (Arc<Objects>, Value) {
    let o = regions(15, 3, |_, c| (c / 3 + 1) as u64);
    let o = classes(&o, &["seed", "cand"], vec![2, 1, 2, 1, 2]);
    let values = [2.0, 2.0, 20.0, 20.0, 10.0];
    let band: Vec<f64> = (0..45).map(|i| values[(i % 15) / 3]).collect();
    let img = image(&o, vec![band]);
    (o, img)
}

#[test]
fn each_seed_takes_its_best_candidate_or_all_that_qualify() {
    let (o, img) = row_of_five();
    let base = serde_json::json!({ "fitting": "mean_b1", "improve": false, "seeds": ["seed"], "candidates": ["cand"], "max_passes": 1 });
    // S1 takes C1 (the mean 20, the best); S2's best is C1 too (11), already taken: S2 waits for the next pass
    let f = fuse(&o, Some(img.clone()), base.clone());
    assert_eq!(f.count, 4);
    assert_eq!(f.id_at(1, 7), f.id_at(1, 10));
    assert_ne!(f.id_at(1, 4), f.id_at(1, 1));
    // `all`: S2 also has C3 (2), free, and takes it in the same pass
    let mut all = base.clone();
    all["mode"] = serde_json::json!("all");
    let f = fuse(&o, Some(img.clone()), all);
    assert_eq!(f.count, 3);
    assert_eq!(f.id_at(1, 1), f.id_at(1, 4));
    assert_eq!(f.id_at(1, 7), f.id_at(1, 10));
    // the passes go on: S1 (with C1) takes C2, S2 takes C3, and the seeds' class is the joined objects'
    let mut more = base.clone();
    more["max_passes"] = serde_json::json!(5);
    let f = fuse(&o, Some(img), more);
    assert_eq!(f.count, 2);
    assert!(f.class_of.iter().all(|&c| c == 1), "{:?}", f.class_of);
    assert_eq!(f.classes.len(), 2);
}

#[test]
fn two_objects_that_are_each_others_best_join_first() {
    // 1 2 3 of equal size, means 0, 10, 30: 2 and 3 are the best of each other (20); 1 would take 2 (5) but 2 is gone
    let (o, img) = strip(&[0.0, 10.0, 30.0]);
    let f = fuse(&o, Some(img), serde_json::json!({ "fitting": "mean_b1", "improve": false, "max_passes": 1 }));
    assert_eq!(f.count, 2);
    assert_eq!(f.id_at(1, 4), f.id_at(1, 7));
    assert_ne!(f.id_at(1, 1), f.id_at(1, 4));
}

#[test]
fn the_passes_go_on_until_nothing_joins_or_the_limit() {
    // eight equal squares, the fitting their cells: every pair qualifies and each pass halves them
    let o = regions(24, 3, |_, c| (c / 3 + 1) as u64);
    assert_eq!(o.count, 8);
    let n = |p: serde_json::Value| fuse(&o, None, p).count;
    let base = |passes: u32| serde_json::json!({ "fitting": "cells", "improve": false, "max_passes": passes });
    assert_eq!(n(base(1)), 4);
    assert_eq!(n(base(2)), 2);
    assert_eq!(n(base(3)), 1);
    assert_eq!(n(serde_json::json!({ "fitting": "cells", "improve": false })), 1);
    // with improve, a bigger object is always better: the same, and the report says what joined
    let (out, _) = fuse_with(&o, None, base(1));
    let out = out.unwrap();
    let Value::Table(r) = &out["report"] else { panic!("report") };
    assert_eq!(r.rows.len(), 4);
    assert!(r.rows.iter().all(|row| row[1] == 2.0));
    assert!(matches!(out["count"], Value::Integer(4)));
    assert!(matches!(out["done"], Value::Bool(false)));
    // a threshold nobody reaches: the same objects, done
    let (out, _) =
        fuse_with(&o, None, serde_json::json!({ "fitting": "cells", "improve": false, "threshold": 1000.0 }));
    let out = out.unwrap();
    assert!(Arc::ptr_eq(&objects_of(&out), &o));
    assert!(matches!(out["done"], Value::Bool(true)));
}

#[test]
fn what_cannot_be_derived_is_refused_with_what_can() {
    let (o, img) = strip(&[1.0, 2.0, 3.0]);
    for (fitting, has_image) in
        [("glcm_homogeneity", true), ("skel_width", true), ("rel_border_water", true), ("nb.mean(mean_b1)", true)]
    {
        let (out, _) = fuse_with(&o, has_image.then(|| img.clone()), serde_json::json!({ "fitting": fitting }));
        let e = out.err().unwrap_or_else(|| panic!("{fitting} was accepted")).to_string();
        assert!(e.contains("cannot be derived") || e.contains("not an attribute"), "{fitting}: {e}");
        assert!(e.contains("compactness") && e.contains("mean_b1"), "{fitting}: {e}");
    }
    // a band's mean without the image: it says to give the image
    let (out, _) = fuse_with(&o, None, serde_json::json!({ "fitting": "mean_b1" }));
    let e = out.err().unwrap().to_string();
    assert!(e.contains("image"), "{e}");
    // no fitting, a bad goal, a bad mode, an unknown class
    for p in [
        serde_json::json!({}),
        serde_json::json!({ "fitting": "cells", "goal": "up" }),
        serde_json::json!({ "fitting": "cells", "mode": "some" }),
        serde_json::json!({ "fitting": "cells", "seeds": ["roof"] }),
        serde_json::json!({ "fitting": "cells", "max_passes": 0 }),
    ] {
        assert!(fuse_with(&o, None, p.clone()).0.is_err(), "{p}");
    }
}

#[test]
fn the_class_of_the_seed_is_kept_and_the_level_above_dropped() {
    let (o, img) = row_of_five();
    let parent = regions(15, 3, |_, _| 1);
    let o = Arc::new(o.as_ref().clone().with_parent(parent, vec![1; 5]).unwrap());
    let (out, warnings) = fuse_with(
        &o,
        Some(img),
        serde_json::json!({ "fitting": "mean_b1", "improve": false, "threshold": -1e9, "seeds": ["seed"], "candidates": ["cand"] }),
    );
    let f = objects_of(&out.unwrap());
    assert!(f.parent.is_none());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("level above"));
    // S1, S2 and the candidates they took: two objects, both of the seeds' class
    assert_eq!(f.count, 2);
    assert_eq!(f.class_name(1), Some("seed"));
    assert_eq!(f.class_name(2), Some("seed"));
    // the candidates alone, none to take them (no seeds of their class): nothing changes
    let (out, warnings) =
        fuse_with(&o, None, serde_json::json!({ "fitting": "cells", "seeds": ["cand"], "candidates": ["cand"] }));
    let same = objects_of(&out.unwrap());
    assert!(Arc::ptr_eq(&same, &o));
    assert!(warnings.is_empty());
}

fn textured(side: usize) -> (Arc<Objects>, Value) {
    let o = regions(side, side, |r, c| ((r / 3) * (side / 3) + c / 3 + 1) as u64);
    let band = |k: usize| -> Vec<f64> {
        (0..side * side)
            .map(|i| {
                let (r, c) = ((i / side) as f64, (i % side) as f64);
                ((r / 9.0).floor() * 7.0 + (c / 12.0).floor() * 5.0) + ((r * 0.7 + c * 1.3 + k as f64).sin() * 2.0)
            })
            .collect()
    };
    let img = image(&o, vec![band(0), band(1)]);
    (o, img)
}

#[test]
fn the_same_objects_on_one_and_eight_threads() {
    let (o, img) = textured(120);
    let run = |threads: usize, params: serde_json::Value| {
        let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
        let out = pool.install(|| fuse_with(&o, Some(img.clone()), params).0.unwrap());
        let f = objects_of(&out);
        let Value::Table(r) = &out["report"] else { panic!("report") };
        (f.labels.to_vec(), f.count, f.table.rows.clone(), r.rows.clone())
    };
    for params in [
        serde_json::json!({ "fitting": "sd_b1 + sd_b2", "goal": "min" }),
        serde_json::json!({ "fitting": "compactness", "goal": "min", "improve": false, "threshold": 1.3, "mode": "all", "max_passes": 4 }),
        serde_json::json!({ "fitting": "mean_b1 - mean_b2", "improve": false, "threshold": 0.0, "max_passes": 3 }),
    ] {
        let one = run(1, params.clone());
        let eight = run(8, params.clone());
        assert!(one.1 < 1600, "{params}: nothing joined");
        assert_eq!(one, eight, "{params}");
    }
}

#[test]
fn improve_off_without_a_threshold_warns_that_nothing_limits_the_joins() {
    let o = building();
    let (out, warnings) =
        fuse_with(&o, None, serde_json::json!({ "fitting": "compactness", "goal": "min", "improve": false }));
    assert_eq!(objects_of(&out.unwrap()).count, 1);
    assert!(warnings.iter().any(|w| w.contains("nothing limits the joins")), "{warnings:?}");
    // with a threshold, or with improve on, no warning
    for p in [
        serde_json::json!({ "fitting": "compactness", "goal": "min", "improve": false, "threshold": 2.0 }),
        serde_json::json!({ "fitting": "compactness", "goal": "min" }),
    ] {
        assert!(fuse_with(&o, None, p).1.is_empty());
    }
}
