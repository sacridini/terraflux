//! Terraflux against landschaft: the same models give the same bits (maps of every step and the fitted
//! pieces). The references are written by `tools/make_reference.py` with landschaft's `ls.simulate`, whose
//! allocations reproduce lulcc bit for bit.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::json;
use terraflux_exec::{Engine, Quiet, Results, load_inputs};
use terraflux_graph::{Input, Model};
use terraflux_ops::{Value, presets};
use terraflux_raster::npy;

fn data() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..").join("..").join("tests").join("data")
}

fn reference(case: &str, name: &str) -> npy::Array {
    npy::read(data().join("reference").join(case).join(format!("{name}.npy")))
        .unwrap_or_else(|e| panic!("{case}/{name}: {e} (run tools/make_reference.py)"))
}

/// The maps of every step, as the reference GeoTIFF holds them (255: no class).
fn reference_maps(case: &str) -> Vec<f64> {
    let r = terraflux_raster::Raster::read(data().join("reference").join(case).join("maps.tif")).unwrap();
    r.bands.iter().flat_map(|b| b.masked(r.nodata)).collect()
}

fn same_bits(what: &str, ours: &[f64], theirs: &[f64]) {
    assert_eq!(ours.len(), theirs.len(), "{what}: lengths");
    let differ =
        ours.iter().zip(theirs).filter(|(a, b)| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan())).count();
    if differ > 0 {
        let (i, (a, b)) = ours
            .iter()
            .zip(theirs)
            .enumerate()
            .find(|(_, (a, b))| a.to_bits() != b.to_bits() && !(a.is_nan() && b.is_nan()))
            .unwrap();
        panic!("{what}: {differ} values differ, the first at {i}: {a} != {b}");
    }
}

fn map(path: &str, band: &str) -> Input {
    Input::Map { path: path.into(), band: Some(band.into()), nodata: None, note: None }
}

fn stack(path: &str) -> Input {
    Input::Stack { path: path.into(), bands: None, nodata: None, note: None }
}

fn run(m: &Model) -> Results {
    let inputs = load_inputs(m, &data().join("inputs"), &BTreeMap::new()).unwrap();
    Engine::default().run(m, &inputs, &mut Quiet).unwrap()
}

fn series_maps(r: &Results, reference: &str) -> Vec<f64> {
    let Some(Value::Series(s)) = r.get(reference) else { panic!("{reference} is not a series") };
    s.iter()
        .flat_map(|v| match v {
            Value::Map(m) => m.data.iter().copied().collect::<Vec<_>>(),
            _ => panic!("not a map"),
        })
        .collect()
}

#[test]
fn dinamica_on_sibuyan() {
    let mut m = Model::new("dinamica");
    m.inputs.insert("t0".into(), map("sibuyan_landuse.tif", "1997"));
    m.inputs.insert("t1".into(), map("sibuyan_landuse.tif", "2011"));
    m.inputs.insert("drivers".into(), stack("sibuyan_drivers.tif"));
    m.inputs.insert("seed".into(), Input::Value { value: 1.into(), note: None });
    presets::dinamica(
        &mut m,
        &presets::Dinamica { steps: 14, simulate: 14, increment: Some(250.0), ..Default::default() },
    );
    let r = run(&m);
    let case = "dinamica_sibuyan";
    let Some(Value::Markov(mk)) = r.get("markov.markov") else { panic!() };
    same_bits("single", &mk.single, &reference(case, "single").data);
    same_bits("multi", &mk.multi, &reference(case, "multi").data);
    let Some(Value::Table(w)) = r.get("woe.table") else { panic!() };
    same_bits("weights", &w.rows.concat(), &reference(case, "weights").data);
    let Some(Value::Patches(p)) = r.get("patches.patches") else { panic!() };
    let obs = reference(case, "observed");
    let k = obs.shape[1];
    for (i, t) in p.transitions.iter().enumerate() {
        let row = &obs.data[i * k..(i + 1) * k];
        same_bits(
            "observed patches",
            &[t.from, t.to, t.expansion, t.expander[0], t.expander[1], t.patcher[0], t.patcher[1]],
            &[row[0], row[1], row[3], row[5], row[6], row[8], row[9]],
        );
    }
    same_bits("maps", &series_maps(&r, "simulation.maps"), &reference_maps(case));
}

fn lulcc(
    case: &str,
    allocation: serde_json::Value,
    seed: i64,
    elasticity: Option<serde_json::Value>,
    rules: bool,
    nb: bool,
) {
    let mut m = Model::new(case);
    m.inputs.insert("observed".into(), stack("pie_landuse.tif"));
    m.inputs.insert("drivers".into(), stack("pie_drivers.tif"));
    m.inputs.insert("start".into(), map("pie_landuse.tif", "1985"));
    m.inputs.insert("seed".into(), Input::Value { value: seed.into(), note: None });
    if rules {
        m.inputs.insert("history".into(), map("pie_history.tif", "0"));
        m.inputs.insert("restrictions".into(), map("pie_restrictions.tif", "0"));
    }
    presets::lulcc(
        &mut m,
        &presets::Lulcc {
            observed: "observed".into(),
            drivers: "drivers".into(),
            seed: "seed".into(),
            start: "start".into(),
            restrictions: rules.then(|| "restrictions".into()),
            history: rules.then(|| "history".into()),
            times: vec![0.0, 6.0, 14.0],
            formulas: Some(
                json!({"2": ["elevation", "slope", "dist_built_1985"], "1": ["elevation", "slope"], "3": ["elevation", "slope"]}),
            ),
            allocation,
            elasticity,
            allowed: rules.then(|| json!([[1, 1008, 105], [0, 1, 0], [-1, -2, 1]])),
            neighbourhood: nb.then(|| json!({"rules": {"1": 0.3, "2": 0.2, "3": 0.3}})),
        },
    );
    let r = run(&m);
    let Some(Value::Demand(d)) = r.get("demand.demand") else { panic!() };
    same_bits(&format!("{case}: demand"), &d.table.concat(), &reference(case, "demand").data);
    let Some(Value::Glm(g)) = r.get("regression.model") else { panic!() };
    // landschaft's coefficients: rows 1, 2, 3; columns the intercept, elevation, slope, dist_built_1985
    let cols = ["elevation", "slope", "dist_built_1985"];
    let mut coef = Vec::new();
    for f in &g.fits {
        coef.push(f.coefficients[0]);
        for c in cols {
            coef.push(f.drivers.iter().position(|d| d == c).map_or(f64::NAN, |j| f.coefficients[j + 1]));
        }
    }
    same_bits(&format!("{case}: coefficients"), &coef, &reference(case, "coefficients").data);
    same_bits(&format!("{case}: maps"), &series_maps(&r, "simulation.maps"), &reference_maps(case));
}

fn clue() -> serde_json::Value {
    json!({"method": "clue", "jitter": 0.0002, "scale": 0.000001, "max_iter": 1000, "max_diff": 50, "mean_diff": 50})
}

#[test]
fn clue_s_vignette() {
    lulcc("clues_pie", clue(), 1, Some(json!([0.2, 0.2, 0.2])), false, false);
}

#[test]
fn clue_s_with_every_rule() {
    lulcc("clues_pie_rules", clue(), 11, Some(json!([0.2, 0.6, 0.1])), true, false);
}

#[test]
fn clue_s_with_neighbourhoods() {
    lulcc("clues_pie_neighb", clue(), 13, Some(json!([0.2, 0.2, 0.2])), false, true);
}

#[test]
fn ordered_stochastic() {
    lulcc("ordered_pie", json!({"method": "ordered", "order": [2, 1, 3]}), 3, None, false, false);
}

#[test]
fn ordered_deterministic() {
    lulcc(
        "ordered_pie_det",
        json!({"method": "ordered", "order": [2, 1, 3], "stochastic": false}),
        3,
        None,
        false,
        false,
    );
}

#[test]
fn ordered_with_rules_and_neighbourhoods() {
    lulcc("ordered_pie_rules", json!({"method": "ordered", "order": [2, 1, 3]}), 5, None, true, true);
}

#[test]
fn models_survive_the_file() {
    // a preset written as TOML and read back runs the same
    let mut m = Model::new("dinamica");
    m.inputs.insert("t0".into(), map("sibuyan_landuse.tif", "1997"));
    m.inputs.insert("t1".into(), map("sibuyan_landuse.tif", "2011"));
    m.inputs.insert("drivers".into(), stack("sibuyan_drivers.tif"));
    m.inputs.insert("seed".into(), Input::Value { value: 1.into(), note: None });
    presets::dinamica(
        &mut m,
        &presets::Dinamica { steps: 14, simulate: 3, increment: Some(250.0), ..Default::default() },
    );
    let back = Model::from_toml(&m.to_toml().unwrap()).unwrap();
    assert_eq!(back, m);
    same_bits("maps", &series_maps(&run(&back), "simulation.maps"), &series_maps(&run(&m), "simulation.maps"));
}
