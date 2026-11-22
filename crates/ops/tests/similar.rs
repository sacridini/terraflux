//! `objects.similar` against numpy (tests/data/reference/similar.json, tools/make_similar_reference.py).

use std::path::Path;
use std::sync::Arc;

use terraflux_ops::objects::{ObjectClass, Objects};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};
use terraflux_raster::Grid;

fn run(inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get("objects.similar").unwrap().run(&mut Ctx::default(), &args)
}

fn reference() -> serde_json::Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/similar.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn rows(v: &serde_json::Value) -> Vec<Vec<f64>> {
    v.as_array().unwrap().iter().map(|r| r.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()).collect()
}

fn numbers(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

/// 30 objects, each one cell of a 30 x 1 grid, with the columns id, cells, area and `names` (the values by row).
fn objects(names: &[&str], values: &[Vec<f64>]) -> Arc<Objects> {
    let n = values.len();
    let o = Objects::from_labels(Arc::new(Grid::new(n, 1)), (1..=n as u32).collect(), n).unwrap();
    let mut t =
        Table::new(["id", "cells", "area"].into_iter().chain(names.iter().copied()).map(String::from).collect());
    t.rows = values
        .iter()
        .enumerate()
        .map(|(i, v)| [(i + 1) as f64, 1.0, 1.0].into_iter().chain(v.iter().copied()).collect())
        .collect();
    Arc::new(o.with_table(t).unwrap())
}

fn close(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}");
    for (i, (a, b)) in got.iter().zip(want).enumerate() {
        assert!((a - b).abs() <= 1e-12, "{what}: object {}: {a} against {b}", i + 1);
    }
}

fn similarity_of(out: &Outputs) -> Vec<f64> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.table.column("similarity").expect("the column similarity")
}

fn selection(out: &Outputs) -> Table {
    let Value::Table(t) = &out["selection"] else { panic!("a table") };
    t.clone()
}

#[test]
fn similarity_is_numpys_cosine_and_euclidean_on_embeddings_and_on_standardised_attributes() {
    let z = reference();
    let (emb, attrs) = (rows(&z["emb"]), rows(&z["attrs"]));
    let examples = serde_json::json!(numbers(&z["examples"]));
    // the embeddings of objects.embed, as they are (the default when the objects have them)
    let o = objects(&["emb_1", "emb_2", "emb_3", "emb_4"], &emb);
    for (metric, key) in [("cosine", "emb_cosine"), ("euclidean", "emb_euclidean")] {
        let out = run(
            vec![("objects", Value::Objects(o.clone()))],
            serde_json::json!({ "examples": examples, "metric": metric }),
        )
        .unwrap();
        close(key, &similarity_of(&out), &numbers(&z[key]));
        let Value::Text(used) = &out["used"] else { panic!("text") };
        assert!(used.contains("emb_1") && used.contains("as they are"), "{used}");
    }
    // the attributes: standardised (the id, cells and area are not among them)
    let o = objects(&["ndvi", "mean_nir", "sd_nir", "compactness"], &attrs);
    for (metric, key) in [("cosine", "std_cosine"), ("euclidean", "std_euclidean")] {
        let out = run(
            vec![("objects", Value::Objects(o.clone()))],
            serde_json::json!({ "examples": examples, "metric": metric }),
        )
        .unwrap();
        close(key, &similarity_of(&out), &numbers(&z[key]));
        let Value::Text(used) = &out["used"] else { panic!("text") };
        assert!(used.contains("standardised") && used.contains("ndvi") && !used.contains("area"), "{used}");
    }
}

#[test]
fn top_and_threshold_choose_the_selection_and_the_examples_are_one() {
    let z = reference();
    let emb = rows(&z["emb"]);
    let o = objects(&["emb_1", "emb_2", "emb_3", "emb_4"], &emb);
    let want = numbers(&z["emb_cosine"]);
    let ask = |extra: serde_json::Value| {
        let mut p = serde_json::json!({ "examples": [3, 17] });
        p.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
        run(vec![("objects", Value::Objects(o.clone()))], p)
    };
    // the examples are 1 to themselves and are not in the selection
    let out = ask(serde_json::json!({ "top": 5 })).unwrap();
    let s = similarity_of(&out);
    assert!((s[2] - 1.0).abs() < 1e-12 && (s[16] - 1.0).abs() < 1e-12);
    let t = selection(&out);
    assert_eq!(t.rows.len(), 5);
    assert!(t.rows.iter().all(|r| r[0] != 3.0 && r[0] != 17.0));
    // the five most similar of the others, the most similar first
    let mut others: Vec<(usize, f64)> =
        (0..want.len()).filter(|&i| i != 2 && i != 16).map(|i| (i + 1, want[i])).collect();
    others.sort_by(|a, b| b.1.total_cmp(&a.1));
    assert_eq!(
        t.rows.iter().map(|r| r[0] as usize).collect::<Vec<_>>(),
        others[..5].iter().map(|x| x.0).collect::<Vec<_>>()
    );
    // a threshold: those at least as similar, none of the examples
    let out = ask(serde_json::json!({ "threshold": 0.5 })).unwrap();
    let t = selection(&out);
    let count = others.iter().filter(|x| x.1 >= 0.5).count();
    assert_eq!(t.rows.len(), count);
    assert!(t.rows.iter().all(|r| r[1] >= 0.5));
    // both is refused; neither is the 20 most similar
    let e = ask(serde_json::json!({ "top": 3, "threshold": 0.1 })).unwrap_err().to_string();
    assert!(e.contains("top or threshold"), "{e}");
    assert_eq!(selection(&ask(serde_json::json!({})).unwrap()).rows.len(), 20);
    // the examples may be a table with the ids, or the objects that have a class
    let mut ids = Table::new(vec!["id".into()]);
    ids.rows = vec![vec![3.0], vec![17.0]];
    let out = run(
        vec![("objects", Value::Objects(o.clone())), ("samples", Value::Table(ids))],
        serde_json::json!({ "top": 5 }),
    )
    .unwrap();
    assert_eq!(selection(&out).rows.len(), 5);
    close("a table of ids", &similarity_of(&out), &want);
    let mut classes = vec![0u32; o.count];
    classes[2] = 1;
    classes[16] = 1;
    let classed =
        o.as_ref().clone().with_classes(vec![ObjectClass { name: "a".into(), colour: None }], classes, vec![]).unwrap();
    let out = run(vec![("objects", Value::Objects(Arc::new(classed)))], serde_json::json!({ "top": 5 })).unwrap();
    close("the classed objects", &similarity_of(&out), &want);
    // no examples, an object that is not there, a column that is not
    assert!(
        run(vec![("objects", Value::Objects(o.clone()))], serde_json::json!({}))
            .unwrap_err()
            .to_string()
            .contains("no examples")
    );
    assert!(ask(serde_json::json!({ "examples": [99] })).unwrap_err().to_string().contains("no object's"));
    assert!(
        ask(serde_json::json!({ "columns": ["nothing"] })).unwrap_err().to_string().contains("no numeric attribute")
    );
}
