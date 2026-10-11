//! prioritize.proxirr against R's proxirr 0.5, to the bit: the outputs landschaft keeps
//! (extern/landschaft/tests/data/reference/optimize/proxirr.npz) of its inputs
//! (tests/data/reference/proxirr_inputs.json, tools/make_proxirr_inputs.py), as its test_prioritize_proxirr.py.

use std::collections::BTreeMap;
use std::path::Path;

use terraflux_ls_sys::ffi;
use terraflux_ops::{Args, Ctx, Registry, Table, Value};
use terraflux_raster::npy::Array;

fn data() -> (serde_json::Value, BTreeMap<String, Array>) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let inputs =
        serde_json::from_str(&std::fs::read_to_string(root.join("tests/data/reference/proxirr_inputs.json")).unwrap())
            .unwrap();
    let reference =
        terraflux_raster::npy::read_npz(root.join("extern/landschaft/tests/data/reference/optimize/proxirr.npz"))
            .unwrap();
    (inputs, reference)
}

/// The same bits, NaN where NaN.
fn bits(what: &str, a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len(), "{what}");
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{what}[{i}]: {x} != {y}");
    }
}

fn floats(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

/// The kernel: alpha with and without triage, and beta and gamma (R's prod() in 80 bits) of units of 5 and of
/// 25 features.
#[test]
fn kernel_is_proxirrs() {
    let (inputs, reference) = data();
    for (case, per) in [("proxirr_entries", 5), ("proxirr_many", 25)] {
        let e = &inputs[case];
        let (local, global, target) = (floats(&e["local"]), floats(&e["global"]), floats(&e["target"]));
        for triage in [false, true] {
            let suffix = if triage { "_triage" } else { "" };
            let Some(want) = reference.get(&format!("{case}/alpha{suffix}")) else { continue };
            let a = ffi::proxirr_alpha(&local, &global, &target, triage).unwrap();
            bits(&format!("{case} alpha{suffix}"), &a, &want.data);
            let start: Vec<i64> = (0..=a.len() as i64).step_by(per).collect();
            let bg = ffi::proxirr_beta_gamma(&start, &a).unwrap();
            bits(&format!("{case} beta{suffix}"), &bg.beta, &reference[&format!("{case}/beta{suffix}")].data);
            bits(&format!("{case} gamma{suffix}"), &bg.gamma, &reference[&format!("{case}/gamma{suffix}")].data);
        }
    }
}

fn run(tables: &serde_json::Value, triage: bool) -> terraflux_ops::Outputs {
    let r = Registry::standard();
    let params = serde_json::json!({ "triage": triage }).as_object().cloned().unwrap();
    let inputs = ["pu", "spec", "puvspr"]
        .into_iter()
        .map(|t| (t.to_string(), Value::Table(serde_json::from_value::<Table>(tables[t].clone()).unwrap())))
        .collect();
    let args = Args { node: "n", inputs, params: &params };
    r.get("prioritize.proxirr").unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn column(out: &terraflux_ops::Outputs, port: &str, col: &str) -> Vec<f64> {
    let Value::Table(t) = &out[port] else { panic!() };
    t.column(col).unwrap()
}

/// From Marxan's tables: proxirr's alphas, betas and gammas, with ids out of order, a feature missing from spec
/// and a unit without features (landschaft's corrected figures where proxirr as released gives NaN).
#[test]
fn marxan_tables_are_proxirrs() {
    let (inputs, reference) = data();
    let r = |k: &str| reference[&format!("proxirr_marxan/{k}")].data.clone();
    let out = run(&inputs["proxirr_marxan"], false);
    bits("alpha", &column(&out, "alpha", "alpha"), &r("alpha"));
    bits("beta", &column(&out, "units", "beta"), &r("beta"));
    bits("gamma", &column(&out, "units", "gamma"), &r("gamma"));
    for (case, triage, prefix) in [
        ("proxirr_marxan", false, "proxirr_marxan/fixed"),
        ("proxirr_marxan", true, "proxirr_marxan/fixed_triage"),
        ("proxirr_marxan_ids", false, "proxirr_marxan_ids/fixed"),
    ] {
        let out = run(&inputs[case], triage);
        let f = |k: &str| reference[&format!("{prefix}/{k}")].data.clone();
        bits(&format!("{prefix} global"), &column(&out, "alpha", "global"), &f("global"));
        bits(&format!("{prefix} target"), &column(&out, "alpha", "target"), &f("target"));
        bits(&format!("{prefix} alpha"), &column(&out, "alpha", "alpha"), &f("alpha"));
        bits(&format!("{prefix} beta"), &column(&out, "units", "beta"), &f("beta"));
        bits(&format!("{prefix} gamma"), &column(&out, "units", "gamma"), &f("gamma"));
    }
}
