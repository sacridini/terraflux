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

/// Included and excluded pairs (Circuitscape's list and matrix files) on rasters, pairwise, one-to-all and
/// all-to-one, with and without focal regions, and Circuitscape's network test cases (advanced and pairwise,
/// included pairs, the pairs' maps), against landschaft's (tests/data/reference/circuit_networks.npz and .json),
/// to the bit.
#[test]
fn included_pairs_and_networks_are_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/circuitscape_landschaft.npz")).unwrap();
    let w = terraflux_raster::npy::read_npz(root().join("tests/data/reference/circuit_networks.npz")).unwrap();
    let meta: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root().join("tests/data/reference/circuit_networks.json")).unwrap(),
    )
    .unwrap();
    let table = |v: &Value| -> Vec<f64> {
        let Value::Table(t) = v else { panic!() };
        t.rows.iter().flatten().copied().collect()
    };
    let bits = |a: &[f64], b: &[f64], what: &str| {
        assert_eq!(a.len(), b.len(), "{what}: length");
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{what}: {i}: {x} {y}");
        }
    };
    let without_header = |a: &terraflux_raster::npy::Array| -> Vec<f64> {
        let n = a.shape[1];
        (1..a.shape[0]).flat_map(|i| a.data[i * n..(i + 1) * n].to_vec()).collect()
    };
    for key in ["include_list", "exclude_list", "include_matrix"] {
        let path = root().join(format!("tests/data/reference/circuit_pairs_{key}.txt"));
        for name in ["plain", "regions"] {
            let inputs =
                vec![("habitat", map(&z[&format!("{name}/habitat")])), ("points", map(&z[&format!("{name}/points")]))];
            let what = format!("{key} {name}");
            let out = run(
                "circuit.pairwise",
                inputs.clone(),
                serde_json::json!({"cum_map": true, "included_pairs": path.to_str().unwrap()}),
            );
            bits(
                &table(&out["resistances"]),
                &without_header(&w[&format!("{key}/{name}/pairwise/resistances")]),
                &format!("{what} pairwise"),
            );
            same(&out["cum"], &w[&format!("{key}/{name}/pairwise/cum")], &format!("{what} pairwise cum"));
            for (mode, all_to_one) in [("one_to_all", false), ("all_to_one", true)] {
                let out = run(
                    "circuit.one_to_all",
                    inputs.clone(),
                    serde_json::json!({"all_to_one": all_to_one, "cum_map": true, "max_map": true, "batch": 4,
                        "included_pairs": path.to_str().unwrap()}),
                );
                let k = format!("{key}/{name}/{mode}");
                bits(&table(&out["resistances"]), &w[&format!("{k}/resistances")].data, &format!("{what} {mode}"));
                same(&out["cum"], &w[&format!("{k}/cum")], &format!("{what} {mode} cum"));
                same(&out["max"], &w[&format!("{k}/max")], &format!("{what} {mode} max"));
            }
        }
    }
    let cs = root().join("extern/landschaft/tests/data/reference/circuit/circuitscape");
    let path = |v: &serde_json::Value| v.as_str().map(|p| cs.join(p).to_str().unwrap().to_string());
    for case in meta["networks"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        if case["scenario"] == "advanced" {
            let out = run(
                "circuit.network_advanced",
                vec![],
                serde_json::json!({"graph_file": path(&case["graph"]), "sources_file": path(&case["sources"]),
                    "grounds_file": path(&case["grounds"]), "graph_is_resistance": case["graph_is_resistance"],
                    "ground_is_resistance": case["ground_is_resistance"], "remove_src_or_gnd": case["policy"]}),
            );
            for k in ["voltages", "node_currents", "branch_currents"] {
                bits(&table(&out[k]), &w[&format!("net/{name}/{k}")].data, &format!("{name} {k}"));
            }
        } else {
            let mut p = serde_json::json!({"graph_file": path(&case["graph"]), "focal_nodes": path(&case["points"]),
                "graph_is_resistance": case["graph_is_resistance"], "voltage_maps": case["voltage_maps"],
                "current_maps": case["current_maps"]});
            if let Some(pairs) = path(&case["included_pairs"]) {
                p["included_pairs"] = serde_json::json!(pairs);
            }
            let out = run("circuit.network_pairwise", vec![], p);
            bits(
                &table(&out["resistances"]),
                &without_header(&w[&format!("net/{name}/resistances")]),
                &format!("{name} resistances"),
            );
            for k in ["cum_node_currents", "cum_branch_currents"] {
                if let Some(want) = w.get(&format!("net/{name}/{k}")) {
                    bits(&table(&out[k]), &want.data, &format!("{name} {k}"));
                }
            }
            for m in case["maps"].as_array().unwrap() {
                let m = m.as_str().unwrap();
                let parts: Vec<&str> = m.split('/').collect();
                let Value::Json(maps) = &out["maps"] else { panic!("{name}: no maps") };
                let entry = &maps[format!("{}_{}", parts[1], parts[2])];
                let got: Vec<f64> = match parts[0] {
                    "branch_currents" => entry["branch_currents"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .flat_map(|r| r.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()))
                        .collect(),
                    k => {
                        let nodes = entry["nodes"].as_array().unwrap();
                        let vals = entry[k].as_array().unwrap();
                        nodes
                            .iter()
                            .zip(vals)
                            .flat_map(|(n, v)| [n.as_f64().unwrap(), v.as_f64().unwrap_or(f64::NAN)])
                            .collect()
                    }
                };
                bits(&got, &w[&format!("net/{name}/maps/{m}")].data, &format!("{name} {m}"));
            }
        }
    }
}
