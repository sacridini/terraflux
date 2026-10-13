//! Circuits against landschaft's (tests/data/reference/circuit_landschaft.npz and .json,
//! tools/make_circuit_reference.py): Omniscape on the cases of landschaft's references (themselves checked
//! against Omniscape.jl), one with conditions, a reclassification and single precision, and the advanced mode on
//! its cases (checked against Circuitscape.jl): the same maps, to the bit.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::npy::Array;

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn reference() -> (BTreeMap<String, Array>, serde_json::Value) {
    let dir = root().join("tests/data/reference");
    let npz = terraflux_raster::npy::read_npz(dir.join("circuit_landschaft.npz")).unwrap();
    let json = serde_json::from_str(&std::fs::read_to_string(dir.join("circuit_landschaft.json")).unwrap()).unwrap();
    (npz, json)
}

fn map(a: &Array) -> Value {
    let (ny, nx) = (a.shape[0], a.shape[1]);
    let mut g = Grid::new(nx, ny);
    g.transform = [0.0, 1.0, 0.0, ny as f64, 0.0, -1.0];
    Value::Map(Map::new(Arc::new(g), a.data.clone()))
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn same(got: &Value, want: &Array, what: &str) {
    let Value::Map(m) = got else { panic!("{what}") };
    assert_eq!(m.data.len(), want.data.len(), "{what}");
    for (i, (a, b)) in m.data.iter().zip(&want.data).enumerate() {
        assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "{what}: cell {i}: {a} {b}");
    }
}

const OUTPUTS: [(&str, &str); 3] =
    [("cum_currmap", "current"), ("flow_potential", "flow_potential"), ("normalized_cum_currmap", "normalized")];

#[test]
fn omniscape_is_landschafts() {
    let (z, json) = reference();
    for (name, case) in json["omniscape"].as_object().unwrap() {
        let mut inputs = vec![("resistance", map(&z[&format!("omniscape/{name}/resistance")]))];
        if case["source"].as_bool().unwrap() {
            inputs.push(("source", map(&z[&format!("omniscape/{name}/source")])));
        }
        let out = run("circuit.omniscape", inputs, case["options"].clone());
        let mut checked = 0;
        for (theirs, ours) in OUTPUTS {
            match z.get(&format!("omniscape/{name}/{theirs}")) {
                Some(want) => {
                    same(&out[ours], want, &format!("{name} {theirs}"));
                    checked += 1;
                }
                None => assert!(!out.contains_key(ours), "{name}: {ours} not asked for"),
            }
        }
        assert!(checked > 0);
    }
    // conditions, a reclassification and single precision
    let inputs = vec![("resistance", map(&z["extra/resistance"])), ("condition1", map(&z["extra/condition1"]))];
    let out = run("circuit.omniscape", inputs, json["extra"].clone());
    for (theirs, ours) in OUTPUTS {
        same(&out[ours], &z[&format!("extra/{theirs}")], &format!("extra {theirs}"));
    }
}

#[test]
fn the_advanced_mode_is_landschafts() {
    let (z, _) = reference();
    let names: std::collections::BTreeSet<String> = z
        .keys()
        .filter_map(|k| k.strip_prefix("advanced/"))
        .filter_map(|k| k.split('/').next())
        .map(String::from)
        .collect();
    assert!(names.len() >= 5);
    for name in names {
        let a = |k: &str| map(&z[&format!("advanced/{name}/{k}")]);
        let four = z[&format!("advanced/{name}/four")].data[0] != 0.0;
        let out = run(
            "circuit.advanced",
            vec![("conductance", a("conductance")), ("source", a("source")), ("ground", a("ground"))],
            serde_json::json!({"four_neighbors": four}),
        );
        same(&out["current"], &z[&format!("advanced/{name}/current")], &format!("{name} current"));
        same(&out["voltage"], &z[&format!("advanced/{name}/voltage")], &format!("{name} voltage"));
    }
}

/// landschaft's example: one amp injected at the left of a 3 x 5 grid, grounded at the right.
#[test]
fn one_amp_across_a_small_grid() {
    let a = |v: Vec<f64>| Array { shape: vec![3, 5], data: v };
    let mut s = vec![0.0; 15];
    s[5] = 1.0;
    let mut g = vec![0.0; 15];
    g[9] = f64::INFINITY;
    let out = run(
        "circuit.advanced",
        vec![("conductance", map(&a(vec![1.0; 15]))), ("source", map(&a(s))), ("ground", map(&a(g)))],
        serde_json::json!({}),
    );
    let Value::Map(v) = &out["voltage"] else { panic!() };
    let r = |x: f64| (x * 1000.0).round() / 1000.0;
    assert_eq!(v.data.iter().map(|x| r(*x)).collect::<Vec<_>>()[5..10], [0.887, 0.615, 0.444, 0.272, 0.0]);
    let Value::Map(c) = &out["current"] else { panic!() };
    assert_eq!(r(c.data[5]), 1.0);
    assert_eq!(r(c.data[9]), 1.0);
}

/// Circuitscape's pairwise and one-to-all (all-to-one) modes against landschaft's, on rasters made by
/// tools/make_circuit_reference.py (focal regions, short-circuit polygons, a mask, strengths): the resistances and
/// the cumulative and largest currents, to the bit (batches of 4 problems, as landschaft on one thread).
#[test]
fn circuitscape_modes_are_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/circuitscape_landschaft.npz")).unwrap();
    let table = |v: &Value| -> Vec<f64> {
        let Value::Table(t) = v else { panic!() };
        t.rows.iter().flatten().copied().collect()
    };
    let bits = |a: &[f64], b: &[f64], what: &str| {
        assert_eq!(a.len(), b.len(), "{what}");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{what}: {i}: {x} {y}");
        }
    };
    for name in ["plain", "regions", "polygons"] {
        let a = |k: &str| map(&z[&format!("{name}/{k}")]);
        let mut inputs = vec![("habitat", a("habitat")), ("points", a("points"))];
        for k in ["polygons", "mask"] {
            if z.contains_key(&format!("{name}/{k}")) {
                inputs.push((k, a(k)));
            }
        }
        let out = run("circuit.pairwise", inputs.clone(), serde_json::json!({"cum_map": true, "max_map": true}));
        // the reference: ids in the first row and column; ours a row per id, its id first
        let want = &z[&format!("{name}/pairwise/resistances")];
        let n = want.shape[0];
        let ours: Vec<f64> = (1..n).flat_map(|i| want.data[i * n..(i + 1) * n].to_vec()).collect();
        bits(&table(&out["resistances"]), &ours, &format!("{name} pairwise resistances"));
        same(&out["cum"], &z[&format!("{name}/pairwise/cum")], &format!("{name} pairwise cum"));
        same(&out["max"], &z[&format!("{name}/pairwise/max")], &format!("{name} pairwise max"));
        let strengths: Vec<[f64; 2]> = z[&format!("{name}/strengths")].data.chunks(2).map(|c| [c[0], c[1]]).collect();
        for (mode, all_to_one) in [("one_to_all", false), ("all_to_one", true)] {
            for with in [false, true] {
                let key = format!("{name}/{mode}") + if with { "_strengths" } else { "" };
                if !z.contains_key(&format!("{key}/resistances")) {
                    continue;
                }
                let mut p = serde_json::json!({"all_to_one": all_to_one, "cum_map": true, "max_map": true,
                                               "focal_zero": true, "batch": 4});
                if with {
                    p["strengths"] = serde_json::json!(strengths);
                }
                let out = run("circuit.one_to_all", inputs.clone(), p);
                bits(
                    &table(&out["resistances"]),
                    &z[&format!("{key}/resistances")].data,
                    &format!("{key} resistances"),
                );
                same(&out["cum"], &z[&format!("{key}/cum")], &format!("{key} cum"));
                same(&out["max"], &z[&format!("{key}/max")], &format!("{key} max"));
            }
        }
    }
}
