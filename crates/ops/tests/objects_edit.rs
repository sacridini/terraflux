//! `objects.edit`: merge, cut and delete by places on a small grid of known objects; the same edits on another
//! segmentation; the warnings and the report; cells with no object; determinism.

use std::sync::Arc;

use terraflux_ops::objects::{ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;

/// Six objects on a 6 x 4 grid:
///
/// ```text
/// 1 1 2 2 3 3
/// 1 1 2 2 3 3
/// 4 4 4 5 5 6
/// 4 4 4 5 5 6
/// ```
#[rustfmt::skip]
const SIX: [u32; 24] = [
    1, 1, 2, 2, 3, 3,
    1, 1, 2, 2, 3, 3,
    4, 4, 4, 5, 5, 6,
    4, 4, 4, 5, 5, 6,
];

fn objects(labels: &[u32], w: usize, h: usize) -> Arc<Objects> {
    let count = labels.iter().copied().max().unwrap_or(0) as usize;
    Arc::new(Objects::from_labels(Arc::new(Grid::new(w, h)), labels.to_vec(), count).unwrap())
}

/// The place of the centre of a cell (the grid has cells of side 1, the origin at the upper left, y down).
fn at(row: usize, col: usize) -> [f64; 2] {
    [col as f64 + 0.5, -(row as f64 + 0.5)]
}

/// The operator on objects (and perhaps an image) with the edits and settings given; the warnings it gave.
fn edit(
    o: &Arc<Objects>,
    image: Option<Value>,
    edits: serde_json::Value,
    extra: serde_json::Value,
) -> (terraflux_ops::Result<Outputs>, Vec<String>) {
    let r = Registry::standard();
    let mut params = extra.as_object().cloned().unwrap_or_default();
    params.insert("edits".into(), edits);
    let mut inputs = vec![("objects".to_string(), Value::Objects(o.clone()))];
    if let Some(i) = image {
        inputs.push(("image".to_string(), i));
    }
    let args = Args { node: "n", inputs: inputs.into_iter().collect(), params: &params };
    let mut ctx = Ctx::default();
    let out = r.get("objects.edit").unwrap().run(&mut ctx, &args);
    (out, ctx.warnings.clone())
}

fn run(o: &Arc<Objects>, edits: serde_json::Value) -> (Outputs, Vec<String>) {
    let (out, warnings) = edit(o, None, edits, serde_json::json!({}));
    (out.unwrap(), warnings)
}

fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

fn report_of(out: &Outputs) -> Table {
    let Value::Table(t) = &out["report"] else { panic!("a report") };
    t.clone()
}

/// The status of the edit `k` (from 0) in the report.
fn status(out: &Outputs, k: usize) -> String {
    let t = report_of(out);
    t.cell(2, t.rows[k][2])
}

fn id(o: &Objects, row: usize, col: usize) -> u32 {
    o.id_at(row, col)
}

#[test]
fn a_merge_gives_the_first_points_object_the_others() {
    let o = objects(&SIX, 6, 4);
    let (out, warnings) = run(&o, serde_json::json!([{ "merge": [at(0, 0), at(0, 2)] }]));
    let m = objects_of(&out);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(m.count, 5);
    assert_eq!(id(&m, 0, 0), id(&m, 1, 3));
    assert_eq!(m.cells[id(&m, 0, 0) as usize - 1], 8);
    assert_ne!(id(&m, 0, 0), id(&m, 0, 4));
    // numbered again by the first cell: the merged object is the first
    assert_eq!(id(&m, 0, 0), 1);
    assert_eq!(status(&out, 0), "done");
    // three at once
    let (out, _) = run(&o, serde_json::json!([{ "merge": [at(0, 0), at(0, 2), at(2, 0)] }]));
    assert_eq!(objects_of(&out).count, 4);
    // the report row: edit, kind, status, cells changed
    let t = report_of(&run(&o, serde_json::json!([{ "merge": [at(0, 0), at(0, 2)] }])).0);
    assert_eq!(t.columns, ["edit", "kind", "status", "cells"]);
    assert_eq!(t.cell(1, t.rows[0][1]), "merge");
    assert_eq!(t.rows[0][3], 4.0);
}

#[test]
fn a_merge_that_finds_no_object_is_already_one_or_not_touching_is_warned_about() {
    let o = objects(&SIX, 6, 4);
    // the second point off the grid, then two points on one object, then two objects that do not touch (1 and 3)
    let (out, warnings) = run(
        &o,
        serde_json::json!([
            { "merge": [at(0, 0), [50.0, -50.0]] },
            { "merge": [at(0, 0), at(1, 1)] },
            { "merge": [at(0, 0), at(0, 4)] },
        ]),
    );
    assert_eq!(objects_of(&out).count, 6, "nothing changed");
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    assert!(warnings[0].contains("edit 1 (merge)") && warnings[0].contains("no object"), "{warnings:?}");
    assert!(warnings[1].contains("edit 2 (merge)") && warnings[1].contains("one object already"), "{warnings:?}");
    assert!(warnings[2].contains("edit 3 (merge)") && warnings[2].contains("do not touch"), "{warnings:?}");
    assert_eq!((0..3).map(|k| status(&out, k)).collect::<Vec<_>>(), ["no object", "already one", "not touching"]);
}

#[test]
fn a_cut_splits_an_object_and_gives_the_line_to_a_piece_next_to_it() {
    let o = objects(&SIX, 6, 4);
    // down the middle column of object 4 (rows 2-3, columns 0-2), a little through object 1 above and off the grid
    let (out, warnings) = run(&o, serde_json::json!([{ "cut": [[1.5, -1.9], [1.5, -4.1]] }]));
    let m = objects_of(&out);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(m.count, 7);
    // the line's cells went to the piece with the smaller label (the first, which kept the id)
    let left = id(&m, 2, 0);
    assert!([(2, 1), (3, 0), (3, 1)].iter().all(|&(r, c)| id(&m, r, c) == left));
    let right = id(&m, 2, 2);
    assert_ne!(left, right);
    assert_eq!(id(&m, 3, 2), right);
    // object 1, crossed at one cell, is whole: that did not split it
    assert_eq!(m.cells[id(&m, 0, 0) as usize - 1], 4);
    assert_eq!(status(&out, 0), "done");
}

#[test]
fn a_cut_that_splits_nothing_or_crosses_nothing_is_warned_about() {
    let o = objects(&SIX, 6, 4);
    // a corner cell of object 1 only; then a line off the grid
    let (out, warnings) = run(
        &o,
        serde_json::json!([
            { "cut": [[0.2, -0.2], [0.8, -0.4]] },
            { "cut": [[20.0, -1.0], [30.0, -1.0]] },
        ]),
    );
    assert_eq!(objects_of(&out).count, 6);
    assert_eq!(warnings.len(), 2, "{warnings:?}");
    assert!(warnings[0].contains("edit 1 (cut)") && warnings[0].contains("does not split"), "{warnings:?}");
    assert!(warnings[1].contains("edit 2 (cut)") && warnings[1].contains("crosses no object"), "{warnings:?}");
    assert_eq!(status(&out, 0), "no split");
    assert_eq!(status(&out, 1), "no object");
}

#[test]
fn a_diagonal_cut_through_one_object_gives_two_connected_pieces_for_both_neighbourhoods() {
    let o = objects(&[1; 36], 6, 6);
    for n in [4, 8] {
        let (out, warnings) = edit(
            &o,
            None,
            serde_json::json!([{ "cut": [[0.0, 0.0], [6.0, -6.0]] }]),
            serde_json::json!({ "neighbours": n }),
        );
        let m = objects_of(&out.unwrap());
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(m.count, 2, "{n} neighbours");
        assert_eq!(m.cells.iter().sum::<u32>(), 36, "every cell has an object");
        assert_ne!(id(&m, 0, 5), id(&m, 5, 0));
    }
}

#[test]
fn a_delete_leaves_no_object_or_joins_the_neighbour() {
    let o = objects(&SIX, 6, 4);
    let (out, warnings) = run(&o, serde_json::json!([{ "delete": at(3, 5) }]));
    let m = objects_of(&out);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(m.count, 5);
    assert_eq!(id(&m, 2, 5), 0);
    assert_eq!(id(&m, 3, 5), 0);
    // into a neighbour without an image: the longest shared border (object 5, two edges; object 3, one)
    let (out, _) = run(&o, serde_json::json!([{ "delete": at(3, 5), "into": "neighbour" }]));
    let m = objects_of(&out);
    assert_eq!(m.count, 5);
    assert_eq!(id(&m, 2, 5), id(&m, 2, 3), "joined object 5");
    assert_eq!(m.cells[id(&m, 2, 3) as usize - 1], 6);
    // with an image, the most similar one (the band's mean): object 6 is as 3, not as 5
    let mut v = vec![0.0; 24];
    for (i, l) in SIX.iter().enumerate() {
        v[i] = match l {
            3 => 10.0,
            6 => 10.1,
            5 => 50.0,
            _ => 100.0,
        };
    }
    let image = Value::Map(Map::new(Arc::new(Grid::new(6, 4)), v));
    let (out, _) =
        edit(&o, Some(image), serde_json::json!([{ "delete": at(3, 5), "into": "neighbour" }]), serde_json::json!({}));
    let out = out.unwrap();
    let m = objects_of(&out);
    assert_eq!(id(&m, 3, 5), id(&m, 0, 5), "joined object 3");
    assert_eq!(m.cells[id(&m, 0, 5) as usize - 1], 6);
    // the table is the segmenters': id, cells, area and the band's mean and deviation
    let Value::Table(t) = &out["table"] else { panic!("a table") };
    assert_eq!(t.columns, ["id", "cells", "area", "mean_image", "sd_image"]);
    assert_eq!(t.rows.len(), 5);
    assert_eq!(m.table.columns, t.columns);
}

#[test]
fn a_delete_with_no_object_or_no_neighbour_is_warned_about() {
    let o = objects(&SIX, 6, 4);
    let (out, warnings) = run(&o, serde_json::json!([{ "delete": [90.0, 90.0] }]));
    assert_eq!(objects_of(&out).count, 6);
    assert!(warnings[0].contains("edit 1 (delete)") && warnings[0].contains("no object"), "{warnings:?}");
    // an object alone on the grid: nothing to join
    let one = objects(&[1; 4], 2, 2);
    let (out, warnings) = run(&one, serde_json::json!([{ "delete": at(0, 0), "into": "neighbour" }]));
    assert_eq!(objects_of(&out).count, 0);
    assert!(warnings[0].contains("no neighbour"), "{warnings:?}");
    assert_eq!(status(&out, 0), "no neighbour");
}

#[test]
fn the_same_edits_find_their_objects_in_another_segmentation() {
    // the same places, objects numbered and shaped otherwise: four blocks
    #[rustfmt::skip]
    let blocks = [
        1, 1, 1, 2, 2, 2,
        1, 1, 1, 2, 2, 2,
        3, 3, 3, 4, 4, 4,
        3, 3, 3, 4, 4, 4,
    ];
    let edits = serde_json::json!([{ "merge": [at(0, 0), at(3, 0)] }, { "delete": at(0, 5) }]);
    let a = objects(&SIX, 6, 4);
    let b = objects(&blocks, 6, 4);
    let (oa, wa) = run(&a, edits.clone());
    let (ob, wb) = run(&b, edits);
    assert!(wa.is_empty() && wb.is_empty(), "{wa:?} {wb:?}");
    let (ma, mb) = (objects_of(&oa), objects_of(&ob));
    // (6 objects: 1 and 4 merged, 3 deleted) and (4 blocks: 1 and 3 merged, 2 deleted)
    assert_eq!((ma.count, mb.count), (4, 2));
    for m in [&ma, &mb] {
        assert_eq!(id(m, 0, 0), id(m, 3, 0), "merged");
        assert_eq!(id(m, 0, 5), 0, "deleted");
    }
}

#[test]
fn cells_with_no_object_stay_so_and_stop_a_merge() {
    // a column of no data between two objects
    #[rustfmt::skip]
    let l = [
        1, 1, 0, 2, 2,
        1, 1, 0, 2, 2,
    ];
    let o = objects(&l, 5, 2);
    let (out, warnings) = run(
        &o,
        serde_json::json!([
            { "merge": [at(0, 0), at(0, 3)] },
            { "merge": [at(0, 0), at(0, 2)] },
            { "delete": at(1, 2) },
            { "cut": [[2.5, 0.0], [2.5, -2.0]] },
        ]),
    );
    let m = objects_of(&out);
    assert_eq!(m.count, 2);
    assert!((0..2).all(|r| id(&m, r, 2) == 0), "still no object");
    assert_eq!(warnings.len(), 4, "{warnings:?}");
    assert!(warnings[0].contains("do not touch"), "{warnings:?}");
    assert!(warnings[1].contains("no object"), "{warnings:?}");
    // (a line over the no-data column crosses no object)
    assert!(warnings[3].contains("edit 4 (cut)") && warnings[3].contains("crosses no object"), "{warnings:?}");
}

#[test]
fn the_same_edits_give_the_same_objects_every_time_and_no_classes() {
    let classed = Arc::new(
        (*objects(&SIX, 6, 4))
            .clone()
            .with_classes(vec![ObjectClass { name: "a".into(), colour: None }], vec![1; 6], Vec::new())
            .unwrap(),
    );
    let edits = serde_json::json!([
        { "cut": [[1.5, -1.9], [1.5, -4.1]] },
        { "merge": [at(0, 0), at(0, 2)] },
        { "delete": at(3, 5), "into": "neighbour" },
    ]);
    let (a, _) = run(&classed, edits.clone());
    let (b, _) = run(&classed, edits);
    let (a, b) = (objects_of(&a), objects_of(&b));
    assert_eq!(a.labels, b.labels);
    assert_eq!(format!("{a:?}"), format!("{b:?}"));
    assert!(a.classes.is_empty() && a.parent.is_none(), "no classes or levels");
}

#[test]
fn bad_edits_are_refused() {
    let o = objects(&SIX, 6, 4);
    for e in [
        serde_json::json!("merge"),
        serde_json::json!([1]),
        serde_json::json!([{}]),
        serde_json::json!([{ "merge": [at(0, 0)] }]),
        serde_json::json!([{ "cut": [at(0, 0)] }]),
        serde_json::json!([{ "merge": [at(0, 0), at(0, 2)], "cut": [at(0, 0), at(0, 2)] }]),
        serde_json::json!([{ "delete": [1.0] }]),
        serde_json::json!([{ "delete": at(0, 0), "into": "far" }]),
        serde_json::json!([{ "merge": [at(0, 0), at(0, 2)], "into": "none" }]),
        serde_json::json!([{ "merge": [at(0, 0), at(0, 2)], "extra": 1 }]),
    ] {
        assert!(edit(&o, None, e.clone(), serde_json::json!({})).0.is_err(), "{e}");
    }
    assert!(edit(&o, None, serde_json::json!([]), serde_json::json!({ "neighbours": 6 })).0.is_err());
    // no edits: the same objects
    let (out, _) = run(&o, serde_json::json!([]));
    assert_eq!(objects_of(&out).labels, o.labels);
}

#[test]
fn objects_whole_only_with_eight_neighbours_are_not_split_by_an_edit_that_changes_nothing() {
    // object 1 is two cells that touch at a corner
    #[rustfmt::skip]
    let l = [
        1, 2, 2,
        2, 1, 2,
        2, 2, 2,
    ];
    let o = objects(&l, 3, 3);
    let (out, _) = run(&o, serde_json::json!([]));
    assert_eq!(objects_of(&out).labels, o.labels, "8 neighbours chosen, nothing split");
    let (out, _) = edit(&o, None, serde_json::json!([]), serde_json::json!({ "neighbours": 4 }));
    assert_eq!(objects_of(&out.unwrap()).count, 3, "with 4 neighbours the corner pair are two objects");
}
