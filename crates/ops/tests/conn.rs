//! Habitat graphs against landschaft's (tests/data/reference/conn.npz and .json, tools/make_conn_reference.py):
//! graphs of distances (a matrix and a pair table), probabilities and links, with Makurhini's conventions and
//! Conefor's; the indices overall, the nodes' importances with the betweenness, the links' importance and
//! changes, nodes to restore, components and centralities. To the bit, but for what landschaft computes with
//! numpy's BLAS dot products, its SIMD exponentials or LAPACK's eigenvectors: those to rounding.

use std::path::{Path, PathBuf};

use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

fn table(columns: &[&str], rows: Vec<Vec<f64>>) -> Value {
    let mut t = Table::new(columns.iter().map(|c| c.to_string()).collect());
    t.rows = rows;
    Value::Table(t)
}

fn numbers(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

/// The columns computed through numpy's BLAS, SIMD exponentials or LAPACK in landschaft.
const ROUNDING: [&str; 15] = [
    "IICintra(%)",
    "IICstep(%)",
    "PCintra(%)",
    "PCstep(%)",
    "AWF",
    "dIICres",
    "dPCres",
    "dIICadd",
    "dPCadd",
    "dFadd",
    "dAWFadd",
    "dIIC_change",
    "dPC_change",
    "eigen",
    "strength",
];

#[test]
fn habitat_graphs_are_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/conn.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/conn.json")).unwrap()).unwrap();
    let mut rounded: Vec<String> = Vec::new();
    for c in meta["cases"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        if let Some(names) = c.get("graphs") {
            let series: Vec<Value> = names
                .as_array()
                .unwrap()
                .iter()
                .map(|g| {
                    let g = &meta["graphs"][g.as_str().unwrap()];
                    let ids = numbers(&g["nodes"]["id"]);
                    let attr = numbers(&g["nodes"]["attribute"]);
                    let nodes = table(&["id", "attribute"], (0..ids.len()).map(|k| vec![ids[k], attr[k]]).collect());
                    let names: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
                    let pairs = table(
                        &names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                        g["matrix"].as_array().unwrap().iter().map(numbers).collect(),
                    );
                    run("conn.graph", vec![("nodes", nodes), ("pairs", pairs)], serde_json::json!({}))
                        .remove("graph")
                        .unwrap()
                })
                .collect();
            let out = run("conn.eca_change", vec![("graphs", Value::Series(series))], c["params"].clone());
            let Value::Table(t) = &out["table"] else { panic!() };
            let got: Vec<f64> = t.rows.iter().flat_map(|r| r[1..].to_vec()).collect();
            let want = &z[name];
            assert_eq!(got.len(), want.data.len(), "{name}");
            for (k, (a, b)) in got.iter().zip(&want.data).enumerate() {
                assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "{name} at {k}: {a} {b}");
            }
            let Value::Json(r) = &out["readings"] else { panic!() };
            assert_eq!(r, &c["readings"], "{name}: readings");
            continue;
        }
        let g = &meta["graphs"][c["graph"].as_str().unwrap()];
        let ids = numbers(&g["nodes"]["id"]);
        let attr = numbers(&g["nodes"]["attribute"]);
        let mut cols = vec!["id", "attribute"];
        let rest = g["nodes"].get("restoration").map(numbers);
        if rest.is_some() {
            cols.push("restoration");
        }
        let nodes = table(
            &cols,
            (0..ids.len())
                .map(|k| [ids[k], attr[k]].into_iter().chain(rest.as_ref().map(|r| r[k])).collect())
                .collect(),
        );
        let pairs = match g.get("matrix") {
            Some(m) => {
                let names: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
                table(
                    &names.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
                    m.as_array().unwrap().iter().map(numbers).collect(),
                )
            }
            None => table(&["from", "to", "value"], g["pairs"].as_array().unwrap().iter().map(numbers).collect()),
        };
        let mut settings = c["settings"].clone();
        if rest.is_some() {
            settings["restoration"] = "restoration".into();
        }
        let graph = run("conn.graph", vec![("nodes", nodes), ("pairs", pairs)], settings).remove("graph").unwrap();
        let mut inputs = vec![("graph", graph)];
        if let Some(ch) = c.get("changes") {
            inputs.push((
                "changes",
                table(&["from", "to", "value"], ch.as_array().unwrap().iter().map(numbers).collect()),
            ));
        }
        let out = run(c["op"].as_str().unwrap(), inputs, c["params"].clone());
        let Value::Table(t) = &out["table"] else { panic!("{name}: no table") };
        let want_cols: Vec<String> =
            c["columns"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().into()).collect();
        assert_eq!(t.columns, want_cols, "{name}: columns");
        let want = &z[name];
        assert_eq!(t.rows.len(), want.shape[0], "{name}: rows");
        let m = want.shape[1];
        for (ci, col) in t.columns.iter().enumerate() {
            let mut exact = true;
            for (r, row) in t.rows.iter().enumerate() {
                let (a, b) = (row[ci], want.data[r * m + ci]);
                if a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) {
                    continue;
                }
                exact = false;
                // the centralities on probabilities go through numpy's exponentials too
                let loose = ROUNDING.contains(&col.as_str()) || (name.contains("centrality_") && col != "cluster");
                let tol = if col == "eigen" { 1e-7 } else { 1e-9 };
                assert!(
                    loose && (a - b).abs() <= tol * a.abs().max(b.abs()).max(1e-300),
                    "{name} {col} row {r}: {a} {b}"
                );
            }
            if !exact {
                rounded.push(format!("{name}.{col}"));
            }
        }
    }
    eprintln!("to rounding: {rounded:?}");

    // Conefor's files: read landschaft's, written back the same
    let dir = root().join("tests/data/reference");
    let read = run(
        "conn.read_conefor",
        vec![],
        serde_json::json!({
            "nodes": dir.join("conn_nodes.txt").to_str().unwrap(),
            "connections": dir.join("conn_connections.txt").to_str().unwrap(),
            "convention": "conefor",
        }),
    );
    let tmp = root().join("target/tmp/conn");
    std::fs::create_dir_all(&tmp).unwrap();
    run(
        "conn.write_conefor",
        vec![("graph", read["graph"].clone())],
        serde_json::json!({
            "nodes": tmp.join("nodes.txt").to_str().unwrap(),
            "connections": tmp.join("connections.txt").to_str().unwrap(),
        }),
    );
    for (ours, theirs) in [("nodes.txt", "conn_nodes.txt"), ("connections.txt", "conn_connections.txt")] {
        let (a, b) =
            (std::fs::read_to_string(tmp.join(ours)).unwrap(), std::fs::read_to_string(dir.join(theirs)).unwrap());
        assert_eq!(a.replace("\r\n", "\n"), b.replace("\r\n", "\n"), "{ours}");
    }
}
