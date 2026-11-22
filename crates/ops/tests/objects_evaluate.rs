//! `segment.evaluate` and `segment.optimize`'s reference: the measures equal the R package segmetric's (cases in
//! `tests/data/reference/segmetric`, made by `tools/make_segmetric_reference.R`), a segmentation equal to its reference
//! is perfect, a reference cut in two or two references merged move the measures that say so, and the best size of a
//! chessboard of blocks is the blocks' own.

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::objects::Objects;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn table(out: &Outputs, name: &str) -> Table {
    let Value::Table(t) = &out[name] else { panic!("{name} is not a table") };
    t.clone()
}

fn reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/segmetric/segmetric.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn gpkg(name: &str) -> String {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/data/reference/segmetric")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

/// The labels of a case (a matrix of rows) as objects on a grid of its size.
fn objects_of(rows: &serde_json::Value, w: usize, h: usize) -> Arc<Objects> {
    let labels: Vec<u32> = rows
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|r| r.as_array().unwrap().iter().map(|x| x.as_u64().unwrap() as u32))
        .collect();
    let count = labels.iter().copied().max().unwrap() as usize;
    Arc::new(Objects::from_labels(Arc::new(Grid::new(w, h)), labels, count).unwrap())
}

fn numbers(v: &serde_json::Value) -> Vec<f64> {
    match v {
        serde_json::Value::Array(a) => a.iter().map(|x| x.as_f64().unwrap()).collect(),
        x => vec![x.as_f64().unwrap()],
    }
}

fn pairs_of(v: &serde_json::Value) -> Vec<(u64, u64)> {
    match v.as_array().unwrap().first() {
        None => Vec::new(),
        Some(serde_json::Value::Array(_)) => {
            v.as_array().unwrap().iter().map(|p| (p[0].as_u64().unwrap(), p[1].as_u64().unwrap())).collect()
        }
        // (a single pair is a plain array of two numbers in the JSON)
        Some(_) => vec![(v[0].as_u64().unwrap(), v[1].as_u64().unwrap())],
    }
}

fn close(what: &str, got: f64, want: f64) {
    assert!((got - want).abs() <= 1e-9 * want.abs().max(1.0), "{what}: {got} {want}");
}

/// The summary and the pairs of a case equal segmetric's.
fn check_case(name: &str, out: &Outputs, case: &serde_json::Value) {
    let s = table(out, "summary");
    let at = |c: &str| s.column(c).unwrap_or_else(|| panic!("no column {c}"))[0];
    let want = &case["summary"];
    for (mine, theirs) in [
        ("os", "OS1"),
        ("us", "US1"),
        ("d", "D_index"),
        ("qr", "QR"),
        ("afi", "AFI"),
        ("precision", "precision"),
        ("recall", "recall"),
        ("f_overlap", "F_measure"),
    ] {
        close(&format!("{name} {mine}"), at(mine), want[theirs].as_f64().unwrap());
    }
    // the pairs of each subset, and each pair's own values
    let t = table(out, "table");
    let col = |c: &str| t.column(c).unwrap();
    let (r, o) = (col("reference"), col("object"));
    let set = |flag: &str| -> Vec<(u64, u64)> {
        let f = col(flag);
        let mut v: Vec<(u64, u64)> =
            (0..r.len()).filter(|&i| f[i] == 1.0).map(|i| (r[i] as u64, o[i] as u64)).collect();
        v.sort_unstable();
        v
    };
    let sorted = |v: &serde_json::Value| {
        let mut p = pairs_of(v);
        p.sort_unstable();
        p
    };
    assert_eq!(set("matched"), sorted(&case["star"]), "{name}: Y*");
    assert_eq!(set("best_for_reference"), sorted(&case["prime"]), "{name}: Y'");
    assert_eq!(set("best_for_object"), sorted(&case["xprime"]), "{name}: X'");
    let by_pair = |c: &str, pairs: &[(u64, u64)], values: Vec<f64>| {
        let v = col(c);
        for (k, p) in pairs.iter().enumerate() {
            let i = (0..r.len()).find(|&i| (r[i] as u64, o[i] as u64) == *p).unwrap();
            close(&format!("{name} {c} of {p:?}"), v[i], values[k]);
        }
    };
    let star = pairs_of(&case["star"]);
    for (mine, theirs) in [("os", "os"), ("us", "us"), ("d", "d"), ("qr", "qr")] {
        by_pair(mine, &star, numbers(&case[theirs]));
    }
    by_pair("afi", &pairs_of(&case["prime"]), numbers(&case["afi"]));
}

#[test]
fn the_measures_are_segmetrics_by_labels_and_by_polygons() {
    let z = reference();
    let (w, h) = (z["width"].as_u64().unwrap() as usize, z["height"].as_u64().unwrap() as usize);
    for (name, case) in z["cases"].as_object().unwrap() {
        let seg = objects_of(&case["segmentation"], w, h);
        // the reference as objects of the same cells
        let reference = objects_of(&case["reference"], w, h);
        let out = run(
            "segment.evaluate",
            vec![("objects", Value::Objects(seg.clone())), ("reference", Value::Objects(reference))],
            serde_json::json!({}),
        )
        .unwrap();
        check_case(name, &out, case);
        // and as the polygons segmetric was given, read from their GeoPackage and laid on the grid
        let read =
            run("vector.read", vec![], serde_json::json!({"path": gpkg(&format!("{name}_reference.gpkg"))})).unwrap();
        let out = run(
            "segment.evaluate",
            vec![("objects", Value::Objects(seg)), ("reference", read["vector"].clone())],
            serde_json::json!({}),
        )
        .unwrap();
        check_case(&format!("{name} (polygons)"), &out, case);
    }
}

#[test]
fn a_segmentation_equal_to_its_reference_is_perfect() {
    let z = reference();
    let (w, h) = (30, 24);
    let case = &z["cases"]["equal"];
    let o = objects_of(&case["segmentation"], w, h);
    let out = run(
        "segment.evaluate",
        vec![("objects", Value::Objects(o.clone())), ("reference", Value::Objects(o.clone()))],
        serde_json::json!({}),
    )
    .unwrap();
    let s = table(&out, "summary");
    for m in ["os", "us", "d", "qr", "afi", "pse", "nsr", "ed2"] {
        assert_eq!(s.column(m).unwrap()[0], 0.0, "{m}");
    }
    for m in ["precision", "recall", "f_overlap"] {
        assert_eq!(s.column(m).unwrap()[0], 1.0, "{m}");
    }
    // no object errs: os and us are 0 on every object
    let Value::Objects(marked) = &out["objects"] else { panic!() };
    assert!(
        marked.table.column("os").unwrap().iter().chain(marked.table.column("us").unwrap().iter()).all(|&v| v == 0.0)
    );
}

/// 9 blocks of 20 x 20 cells in a grid of 60 x 60, as objects.
fn blocks(side: usize) -> Arc<Objects> {
    let n = 60 / side;
    let labels: Vec<u32> = (0..3600).map(|i| (((i / 60) / side) * n + (i % 60) / side + 1) as u32).collect();
    Arc::new(Objects::from_labels(Arc::new(Grid::new(60, 60)), labels, n * n).unwrap())
}

fn summary(seg: &Arc<Objects>, reference: &Arc<Objects>) -> Table {
    let out = run(
        "segment.evaluate",
        vec![("objects", Value::Objects(seg.clone())), ("reference", Value::Objects(reference.clone()))],
        serde_json::json!({}),
    )
    .unwrap();
    table(&out, "summary")
}

#[test]
fn cutting_the_references_raises_the_under_segmentation_and_merging_them_the_over() {
    // the segmentation is the 20 x 20 blocks. The reference cut in two (10 wide): each object holds two references
    let (seg, whole) = (blocks(20), blocks(20));
    let halves = {
        let labels: Vec<u32> = (0..3600).map(|i| (((i / 60) / 20) * 6 + (i % 60) / 10 + 1) as u32).collect();
        Arc::new(Objects::from_labels(Arc::new(Grid::new(60, 60)), labels, 18).unwrap())
    };
    let (a, b) = (summary(&seg, &whole), summary(&seg, &halves));
    let at = |t: &Table, c: &str| t.column(c).unwrap()[0];
    assert_eq!((at(&a, "os"), at(&a, "us")), (0.0, 0.0));
    // (an object holds the two halves: the references are inside it, none is left out of it)
    assert_eq!(at(&b, "os"), 0.0);
    assert!(at(&b, "us") > 0.4, "{}", at(&b, "us"));
    // the reference merged by pairs (40 x 20, a ninth left alone... as 60 x 20 rows of the blocks): each object is
    // a third of a reference, so the over-segmentation rises and the under does not
    let rows = {
        let labels: Vec<u32> = (0..3600).map(|i| ((i / 60) / 20 + 1) as u32).collect();
        Arc::new(Objects::from_labels(Arc::new(Grid::new(60, 60)), labels, 3).unwrap())
    };
    let c = summary(&seg, &rows);
    assert!(at(&c, "os") > 0.6 && at(&c, "us") == 0.0, "{} {}", at(&c, "os"), at(&c, "us"));
    // the same, the segmentation cut and the reference whole: the objects are halves of the references
    let d = summary(&halves, &whole);
    assert!(at(&d, "os") > 0.4 && at(&d, "us") == 0.0, "{} {}", at(&d, "os"), at(&d, "us"));
    assert!(at(&d, "ed2") > 0.0 && at(&d, "nsr") > 0.0);
}

#[test]
fn the_objects_output_gives_the_os_and_us_of_each_object_and_the_metrics_choose_the_columns() {
    let (seg, rows) = (blocks(20), {
        let labels: Vec<u32> = (0..3600).map(|i| ((i / 60) / 20 + 1) as u32).collect();
        Arc::new(Objects::from_labels(Arc::new(Grid::new(60, 60)), labels, 3).unwrap())
    });
    let out = run(
        "segment.evaluate",
        vec![("objects", Value::Objects(seg)), ("reference", Value::Objects(rows))],
        serde_json::json!({"metrics": ["os", "ed2"]}),
    )
    .unwrap();
    let Value::Objects(o) = &out["objects"] else { panic!() };
    // each object is a third of its row: its os is 2/3 and it is whole inside it (us 0)
    for (&os, &us) in o.table.column("os").unwrap().iter().zip(&o.table.column("us").unwrap()) {
        assert!((os - 2.0 / 3.0).abs() < 1e-12 && us == 0.0);
    }
    let s = table(&out, "summary");
    assert_eq!(s.columns, ["references", "objects", "matched", "os", "ed2"]);
    assert_eq!(
        table(&out, "table").columns.iter().filter(|c| ["os", "us", "d", "qr", "afi"].contains(&c.as_str())).count(),
        1
    );
}

#[test]
fn evaluate_refuses_what_it_cannot_measure() {
    let seg = blocks(20);
    let go = |inputs: Vec<(&str, Value)>, params: serde_json::Value| {
        run("segment.evaluate", inputs, params).map(|_| ()).expect_err("refused").to_string()
    };
    let e = go(vec![("objects", Value::Objects(seg.clone()))], serde_json::json!({}));
    assert!(e.contains("connect the reference"), "{e}");
    let e =
        go(vec![("objects", Value::Objects(seg.clone())), ("reference", Value::Number(3.0))], serde_json::json!({}));
    assert!(e.contains("polygons or objects"), "{e}");
    let small = Arc::new(Objects::from_labels(Arc::new(Grid::new(30, 30)), vec![1; 900], 1).unwrap());
    let e =
        go(vec![("objects", Value::Objects(seg.clone())), ("reference", Value::Objects(small))], serde_json::json!({}));
    assert!(e.contains("grid of 30 x 30"), "{e}");
    let e = go(
        vec![("objects", Value::Objects(seg.clone())), ("reference", Value::Objects(seg.clone()))],
        serde_json::json!({"metrics": ["os", "kappa"]}),
    );
    assert!(e.contains("metric \"kappa\""), "{e}");
    let e = go(
        vec![("objects", Value::Objects(seg.clone())), ("reference", Value::Objects(seg))],
        serde_json::json!({"metrics": []}),
    );
    assert!(e.contains("at least one"), "{e}");
}

// -- segment.optimize with a reference -----------------------------------------------------------------------------

/// The image of the blocks: each of its 9 blocks of 20 x 20 cells a value of its own, with a little noise.
fn flat_image() -> Value {
    let data = (0..3600)
        .map(|i| (((i / 60) / 20) * 3 + (i % 60) / 20) as f64 * 3.0 + ((i * 7919) % 13) as f64 / 13.0)
        .collect();
    Value::Map(Map::new(Arc::new(Grid::new(60, 60)), data))
}

fn optimize(reference: Option<Value>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let mut inputs = vec![("image", flat_image())];
    inputs.extend(reference.map(|r| ("reference", r)));
    run("segment.optimize", inputs, params)
}

#[test]
fn optimize_with_a_reference_picks_the_size_of_the_blocks_by_each_measure() {
    let reference = Value::Objects(blocks(20));
    let base = serde_json::json!({"segmenter": "segment.chessboard", "parameter": "size", "values": [5, 10, 15, 20, 25, 30, 40]});
    for objective in ["d", "ed2", "f_overlap"] {
        let mut p = base.clone();
        p["objective"] = serde_json::json!(objective);
        let out = optimize(Some(reference.clone()), p).unwrap();
        let Value::Number(best) = out["value"] else { panic!() };
        assert_eq!(best, 20.0, "{objective}");
        let Value::Objects(o) = &out["objects"] else { panic!() };
        assert_eq!(o.count, 9);
        let s = table(&out, "scores");
        assert_eq!(
            s.columns,
            ["value", "objects", "variance", "moran", "objective", "os", "us", "d", "ed2", "f_overlap"]
        );
        // the perfect size: no error, the F of 1, and the objective (1 the best) is 1 there only
        let at = s.rows.iter().position(|r| r[0] == 20.0).unwrap();
        assert_eq!((s.rows[at][7], s.rows[at][8], s.rows[at][9]), (0.0, 0.0, 1.0));
        assert_eq!(s.rows[at][4], 1.0);
        assert!(s.rows.iter().enumerate().all(|(k, r)| k == at || r[4] < 1.0));
        assert!(s.rows.iter().enumerate().all(|(k, r)| k == at || (r[8] > 0.0 && r[9] < 1.0)));
    }
    // the same by the default objective (the reference only adds the columns): the scores' first five are as without
    let with = optimize(Some(reference), base.clone()).unwrap();
    let without = optimize(None, base).unwrap();
    let (a, b) = (table(&with, "scores"), table(&without, "scores"));
    assert_eq!(b.columns.len(), 5);
    assert_eq!(a.columns[..5], b.columns[..]);
    for (x, y) in a.rows.iter().zip(&b.rows) {
        assert_eq!(
            x[..5].iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
            y.iter().map(|v| v.to_bits()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn optimize_refuses_a_measure_without_its_reference() {
    let p = serde_json::json!({"segmenter": "segment.chessboard", "parameter": "size", "values": [10, 20], "objective": "ed2"});
    let e = optimize(None, p.clone()).map(|_| ()).expect_err("refused").to_string();
    assert!(e.contains("connect the reference"), "{e}");
    let e = optimize(Some(Value::Number(1.0)), p).map(|_| ()).expect_err("refused").to_string();
    assert!(e.contains("polygons or objects"), "{e}");
    let mut other = serde_json::json!({"segmenter": "segment.chessboard", "parameter": "size", "values": [10, 20]});
    other["objective"] = serde_json::json!("nope");
    let e = optimize(None, other).map(|_| ()).expect_err("refused").to_string();
    assert!(e.contains("sum, f, d, ed2 or f_overlap"), "{e}");
}
