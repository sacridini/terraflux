//! The operators on small cases worked by hand (the examples of landschaft's documentation).

use std::sync::Arc;

use terraflux_ops::dynamic::{distance_to, share_of};
use terraflux_ops::{Args, Ctx, Demand, Key, Map, Registry, Stack, Value};
use terraflux_raster::Grid;

fn map(ny: usize, nx: usize, v: &[f64]) -> Map {
    Map::new(Arc::new(Grid::new(nx, ny)), v.to_vec())
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Result<terraflux_ops::Outputs, String> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

#[test]
fn distance_to_a_class() {
    let land = map(2, 4, &[1.0, 1.0, 1.0, 3.0, 1.0, 1.0, 1.0, 1.0]);
    let d = distance_to(&land, &[3.0], 1.0).unwrap();
    let want = [3.0, 2.0, 1.0, 0.0, 10f64.sqrt(), 5f64.sqrt(), 2f64.sqrt(), 1.0];
    for (a, b) in d.data.iter().zip(want) {
        assert!((a - b).abs() < 1e-12, "{a} != {b}");
    }
    // no cell of the class: the diagonal everywhere; missing cells stay missing
    let land = map(1, 3, &[1.0, f64::NAN, 1.0]);
    let d = distance_to(&land, &[9.0], 2.0).unwrap();
    assert_eq!(d.data[0], 3f64.hypot(1.0) * 2.0);
    assert!(d.data[1].is_nan());
}

#[test]
fn share_of_a_class() {
    let land = map(3, 3, &[1.0, 2.0, 2.0, 1.0, 1.0, 2.0, 1.0, 1.0, 1.0]);
    let s = share_of(&land, &[2.0], &vec![vec![1.0; 3]; 3]).unwrap();
    assert!((s.data[4] - 1.0 / 3.0).abs() < 1e-15);
    assert!(share_of(&land, &[2.0], &vec![vec![1.0; 2]; 2]).is_err());
}

#[test]
fn stacks_by_name_class_and_transition() {
    let g = Arc::new(Grid::new(2, 1));
    let s = Stack {
        grid: g.clone(),
        keys: vec![Key::Name("1".into()), Key::Name("2_to_3".into())],
        layers: vec![Arc::new(vec![0.1, 0.2]), Arc::new(vec![0.3, 0.4])],
    };
    let out = run("stack.classes", vec![("stack", Value::Stack(s.clone()))], serde_json::json!({})).unwrap();
    let Value::Stack(c) = &out["stack"] else { panic!() };
    assert_eq!(c.keys, vec![Key::Class(1.0), Key::Transition(2.0, 3.0)]);
    let out = run(
        "stack.set",
        vec![("stack", Value::Stack(s)), ("map", Value::Map(Map::new(g, vec![9.0, 9.0])))],
        serde_json::json!({"name": "slope"}),
    )
    .unwrap();
    let Value::Stack(c) = &out["stack"] else { panic!() };
    assert_eq!(c.by_name("slope").unwrap().as_slice(), &[9.0, 9.0]);
    assert_eq!(c.layers.len(), 3);
}

#[test]
fn cars_demand_from_an_area_demand() {
    let land = map(1, 4, &[1.0, 1.0, 2.0, f64::NAN]);
    let ok = Demand { classes: vec![1.0, 2.0], times: vec![0.0, 1.0], table: vec![vec![2.0, 1.0], vec![1.0, 2.0]] };
    let out = run(
        "plus.demand",
        vec![("start", Value::Map(land.clone())), ("demand", Value::Demand(Arc::new(ok)))],
        serde_json::json!({}),
    )
    .unwrap();
    assert!(matches!(out["steps"], Value::Integer(1)));
    let bad = Demand { classes: vec![1.0, 2.0], times: vec![0.0, 1.0], table: vec![vec![2.0, 1.0], vec![1.0, 1.0]] };
    let e = run(
        "plus.demand",
        vec![("start", Value::Map(land)), ("demand", Value::Demand(Arc::new(bad)))],
        serde_json::json!({}),
    )
    .unwrap_err();
    assert!(e.contains("adds up to"), "{e}");
}

#[test]
fn fitted_pieces_from_the_model_file() {
    let out = run(
        "markov.matrix",
        vec![],
        serde_json::json!({"classes": [1, 2], "single": [[0.81, 0.19], [0.0, 1.0]], "steps": 2}),
    )
    .unwrap();
    let Value::Markov(m) = &out["markov"] else { panic!() };
    assert!((m.multi[0] - 0.9).abs() < 1e-14 && (m.multi[1] - 0.1).abs() < 1e-14);
    let out = run(
        "glm.coefficients",
        vec![],
        serde_json::json!({"fits": [{"class": 1, "drivers": ["a", "b"], "coefficients": [0.5, null, 2.0],
                                     "pivot": [1, 2, 3], "rank": 2}]}),
    )
    .unwrap();
    let Value::Glm(g) = &out["model"] else { panic!() };
    assert!(g.fits[0].coefficients[1].is_nan());
    assert!(
        run(
            "glm.coefficients",
            vec![],
            serde_json::json!({"fits": [{"class": 1, "drivers": ["a"], "coefficients": [1.0],
        "pivot": [1], "rank": 1}]})
        )
        .is_err()
    );
}

#[test]
fn agreement_and_figure_of_merit() {
    use terraflux_ops::validate::agreement;
    let start = map(1, 4, &[1.0, 1.0, 1.0, 1.0]);
    let obs = map(1, 4, &[2.0, 2.0, 1.0, 1.0]);
    let sim = map(1, 4, &[2.0, 3.0, 2.0, 1.0]);
    let a = agreement(&obs, &sim, &start).unwrap();
    assert_eq!((a.hits, a.wrong_hits, a.misses, a.false_alarms), (1, 1, 0, 1));
    assert!((a.figure_of_merit - 1.0 / 3.0).abs() < 1e-15 && a.overall == 0.5);
    assert_eq!(a.quantity, vec![(1.0, -1), (2.0, 0), (3.0, 1)]);
}

#[test]
fn similarity_of_differences() {
    // landschaft's example: one cell changed, simulated one cell away
    let t0 = map(6, 6, &[1.0; 36]);
    let mut o = vec![1.0; 36];
    o[2 * 6 + 2] = 2.0;
    let mut s = vec![1.0; 36];
    s[2 * 6 + 3] = 2.0;
    let t =
        terraflux_ops::validate::similarity_of_differences(&t0, &map(6, 6, &o), &map(6, 6, &s), &[1, 3], true).unwrap();
    assert_eq!(t.rows[0][3], 0.0);
    assert!((t.rows[1][3] - 0.5f64.sqrt()).abs() < 1e-6, "{}", t.rows[1][3]);
}
