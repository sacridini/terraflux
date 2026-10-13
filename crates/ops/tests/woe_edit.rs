//! Weights of evidence edited as the viewer edits them: the ranges of one transition's variable given (the
//! others' as fitted), and weights set by hand (the others' as fitted, and in the probabilities).

use std::path::Path;

use terraflux_ops::files::Rasters;
use terraflux_ops::woe::{Settings, WoeModel, fit, probability};
use terraflux_ops::{Map, Stack};

fn inputs() -> (Map, Map, Stack) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs");
    let lu = Rasters::open(dir.join("sibuyan_landuse.tif").to_str().unwrap(), None, None).unwrap().stack().unwrap();
    let t0 = Map { grid: lu.grid.clone(), data: lu.layers[0].clone() };
    let t1 = Map { grid: lu.grid.clone(), data: lu.layers[1].clone() };
    let drivers =
        Rasters::open(dir.join("sibuyan_drivers.tif").to_str().unwrap(), None, None).unwrap().stack().unwrap();
    (t0, t1, drivers)
}

fn settings(v: serde_json::Value) -> Settings {
    serde_json::from_value(v).unwrap()
}

fn rows_of(m: &WoeModel, f: f64, t: f64, var: &str) -> Vec<(f64, f64, f64)> {
    m.rows
        .iter()
        .filter(|r| r.from == f && r.to == t && r.variable == var)
        .map(|r| (r.lower, r.upper, r.weight))
        .collect()
}

#[test]
fn ranges_of_a_transition_and_weights_by_hand() {
    let (t0, t1, drivers) = inputs();
    let base = fit(&t0, &t1, &drivers, &Settings::default(), None).unwrap();
    let (f, t) = base.transitions[0];
    let var = base.entries.iter().find(|e| e.from == f && e.to == t && !e.layout.categorical).unwrap().variable.clone();
    let fitted = rows_of(&base, f, t, &var);
    assert!(fitted.len() >= 2);
    // the first two ranges merged, for this transition only
    let mut limits: Vec<f64> = fitted.iter().map(|r| r.0).collect();
    limits.push(fitted.last().unwrap().1);
    limits.remove(1);
    let key = terraflux_ops::woe::key_of(f, t);
    let s = settings(serde_json::json!({"ranges": {key.clone(): {var.clone(): limits.clone()}}}));
    let edited = fit(&t0, &t1, &drivers, &s, None).unwrap();
    let mine = rows_of(&edited, f, t, &var);
    assert_eq!(mine.len(), fitted.len() - 1);
    for (k, r) in mine.iter().enumerate() {
        assert_eq!((r.0, r.1), (limits[k], limits[k + 1]));
    }
    for &(a, b) in base.transitions.iter().filter(|x| **x != (f, t)) {
        for e in base.entries.iter().filter(|e| e.from == a && e.to == b) {
            assert_eq!(rows_of(&base, a, b, &e.variable), rows_of(&edited, a, b, &e.variable), "the others as fitted");
        }
    }
    // as many weights as ranges, or refused
    let wrong: Vec<serde_json::Value> = vec![serde_json::Value::Null; fitted.len() + 1];
    let s = settings(serde_json::json!({"weights": {key.clone(): {var.clone(): wrong}}}));
    assert!(fit(&t0, &t1, &drivers, &s, None).is_err());
    // a weight by hand (the others as fitted)
    let mut ws = vec![serde_json::Value::Null; fitted.len()];
    ws[0] = serde_json::json!(2.5);
    let s2 = settings(serde_json::json!({"weights": {key.clone(): {var.clone(): ws}}}));
    let by_hand = fit(&t0, &t1, &drivers, &s2, None).unwrap();
    let r = rows_of(&by_hand, f, t, &var);
    assert_eq!(r[0].2, 2.5);
    assert_eq!(&r[1..], &fitted[1..]);
    let e = by_hand.entries.iter().find(|e| e.from == f && e.to == t && e.variable == var).unwrap();
    assert_eq!(e.weights[0], 2.5);
    // the probabilities follow the weights
    let p0 = probability(&base, &t0, &drivers).unwrap();
    let p1 = probability(&by_hand, &t0, &drivers).unwrap();
    assert_ne!(p0.layers[0], p1.layers[0]);
}
