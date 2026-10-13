//! Biodiverse's analyses against landschaft's (tests/data/reference/bio.json and bio_spatial.npz,
//! tools/make_bio_reference.py): the basedata of its example sites read five ways, and the spatial analyses of
//! every calculation without trees, matrices or properties under twenty spatial conditions, scalar and list
//! indices, to the bit.

use std::path::{Path, PathBuf};

use terraflux_ops::bio::basedata::{BaseData, TableImport};
use terraflux_ops::bio::conditions::Condition;
use terraflux_ops::bio::spatial::spatial_analysis;
use terraflux_ops::{Args, Ctx, Registry, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn sites() -> PathBuf {
    root().join("extern/landschaft/src/landschaft/bio/data/example_sites.csv")
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn check(what: &str, got: &[f64], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(k) = got.iter().zip(want).position(|(a, b)| !same(*a, *b)) {
        panic!("{what}: at {k}: {} {}", got[k], want[k]);
    }
}

fn strings(v: &serde_json::Value) -> Vec<String> {
    v.as_array()
        .map_or(Vec::new(), |a| a.iter().map(|s| s.as_str().map_or_else(|| s.to_string(), String::from)).collect())
}

fn numbers(v: &serde_json::Value) -> Option<Vec<f64>> {
    v.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap()).collect())
}

fn import(o: &serde_json::Value) -> TableImport {
    TableImport {
        group_columns: strings(&o["group_columns"]),
        label_columns: strings(&o["label_columns"]),
        sample_count_columns: strings(&o["sample_count_columns"]),
        cell_sizes: numbers(&o["cell_sizes"]),
        cell_origins: numbers(&o["cell_origins"]),
        ..TableImport::default()
    }
}

#[test]
fn basedata_and_spatial_analyses_are_landschafts() {
    let reference: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/bio.json")).unwrap()).unwrap();
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/bio_spatial.npz")).unwrap();
    let text = std::fs::read_to_string(sites()).unwrap();
    for (key, want) in reference["basedata"].as_object().unwrap() {
        let bd = BaseData::from_table(&text, &import(&want["options"])).unwrap();
        assert_eq!(bd.groups, strings(&want["groups"]), "{key}: groups");
        assert_eq!(bd.labels, strings(&want["labels"]), "{key}: labels");
        let f = |v: &[i64]| v.iter().map(|x| *x as f64).collect::<Vec<_>>();
        check(&format!("{key}: indptr"), &f(&bd.indptr), &z[&format!("bd/{key}/indptr")].data);
        let idx: Vec<f64> = bd.indices.iter().map(|x| f64::from(*x)).collect();
        check(&format!("{key}: indices"), &idx, &z[&format!("bd/{key}/indices")].data);
        check(&format!("{key}: counts"), &bd.counts, &z[&format!("bd/{key}/counts")].data);
        check(&format!("{key}: coords"), &bd.coords, &z[&format!("bd/{key}/coords")].data);
    }

    let bd = BaseData::from_table(&text, &import(&reference["basedata"]["cells"]["options"])).unwrap();
    let calcs = strings(&reference["calculations"]);
    for (k, case) in reference["spatial"].as_array().unwrap().iter().enumerate() {
        let conds: Vec<Condition> = strings(&case["conditions"]).iter().map(|c| Condition::new(c).unwrap()).collect();
        let query = case["query"].as_str().map(|q| Condition::new(q).unwrap());
        let what = format!("case {k} {:?}", strings(&case["conditions"]));
        let r = spatial_analysis(&bd, &conds, &calcs, query.as_ref(), None, 1, true)
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        let groups: Vec<String> = r.groups.iter().map(|&g| bd.groups[g as usize].clone()).collect();
        assert_eq!(groups, strings(&case["groups"]), "{what}: groups");
        assert_eq!(r.names, strings(&case["columns"]), "{what}: columns");
        check(&format!("{what}: table"), &r.rows.concat(), &z[&format!("sp/{k}/table")].data);
        let lists = case["lists"].as_array().unwrap();
        assert_eq!(r.lists.len(), lists.len(), "{what}: lists");
        for l in lists {
            let name = l[0].as_str().unwrap();
            let got = r.lists.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("{what}: no list {name}"));
            assert_eq!(i64::from(got.kind), l[1].as_i64().unwrap(), "{what}: {name}: kind");
            let mut indptr = vec![0.0];
            for row in &got.rows {
                indptr.push(indptr.last().unwrap() + row.len() as f64);
            }
            let p = format!("sp/{k}/list/{name}");
            check(&format!("{what}: {name}: indptr"), &indptr, &z[&format!("{p}/indptr")].data);
            let keys: Vec<f64> = got.rows.iter().flatten().map(|e| e.0).collect();
            let values: Vec<f64> = got.rows.iter().flatten().map(|e| e.1).collect();
            check(&format!("{what}: {name}: keys"), &keys, &z[&format!("{p}/keys")].data);
            check(&format!("{what}: {name}: values"), &values, &z[&format!("{p}/values")].data);
        }
        assert_eq!(r.texts, strings(&case["texts"]), "{what}: texts");
    }
}

/// The operators: a basedata of the example sites, and richness and endemism over circles of 200 km as a table
/// and maps.
#[test]
fn the_operators_read_and_analyse() {
    let r = Registry::standard();
    let params = serde_json::json!({"path": sites().to_str().unwrap(), "group_columns": ["x", "y"],
        "label_columns": ["genus", "species"], "cell_sizes": [100000, 100000]});
    let params = params.as_object().unwrap().clone();
    let args = Args { node: "bd", inputs: Default::default(), params: &params };
    let out = r.get("bio.table").unwrap().run(&mut Ctx::default(), &args).unwrap();
    let Value::BaseData(bd) = &out["basedata"] else { panic!() };
    assert_eq!((bd.n_groups(), bd.n_labels()), (127, 31));
    let params = serde_json::json!({"conditions": "sp_circle (radius => 200000)",
        "calculations": ["richness", "endemism_whole"]});
    let params = params.as_object().unwrap().clone();
    let args = Args {
        node: "sp",
        inputs: [("basedata".to_string(), Value::BaseData(bd.clone()))].into_iter().collect(),
        params: &params,
    };
    let out = r.get("bio.spatial").unwrap().run(&mut Ctx::default(), &args).unwrap();
    let Value::Table(t) = &out["table"] else { panic!() };
    assert_eq!(t.rows.len(), 127);
    assert!(t.columns.contains(&"RICHNESS_ALL".to_string()) && t.columns.contains(&"ENDW_WE".to_string()));
    let Value::Stack(s) = &out["maps"] else { panic!() };
    assert_eq!((s.grid.width, s.grid.height), (21, 31));
    let rich = s.keys.iter().position(|k| k.to_string() == "RICHNESS_ALL").unwrap();
    assert_eq!(s.layers[rich].iter().filter(|v| !v.is_nan()).count(), 127);
}

/// The feature import (bio.points): the example sites as points, cells of 150 km from an origin, counts from an
/// attribute.
#[test]
fn points_are_read_like_landschaft() {
    use std::sync::Arc;
    use terraflux_raster::vector::{Column, Geometry, Vector};
    let reference: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/bio.json")).unwrap()).unwrap();
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/bio_spatial.npz")).unwrap();
    let text = std::fs::read_to_string(sites()).unwrap();
    let rows: Vec<Vec<String>> =
        text.lines().skip(1).map(|l| l.split(',').map(|f| f.trim_matches('"').to_string()).collect()).collect();
    let v = Vector {
        geometries: rows.iter().map(|r| Geometry::Point([r[3].parse().unwrap(), r[4].parse().unwrap()])).collect(),
        attributes: vec![
            ("num".into(), Column::Numbers(rows.iter().map(|r| r[0].parse().unwrap()).collect())),
            ("genus".into(), Column::Texts(rows.iter().map(|r| r[1].clone()).collect())),
            ("species".into(), Column::Texts(rows.iter().map(|r| r[2].clone()).collect())),
        ],
        crs: None,
    };
    let params = serde_json::json!({"label": ["genus", "species"], "count": "num", "cell_sizes": [150000, 150000],
        "cell_origins": [10000, 20000]});
    let params = params.as_object().unwrap().clone();
    let args = Args {
        node: "pts",
        inputs: [("points".to_string(), Value::Vector(Arc::new(v)))].into_iter().collect(),
        params: &params,
    };
    let out = Registry::standard().get("bio.points").unwrap().run(&mut Ctx::default(), &args).unwrap();
    let Value::BaseData(bd) = &out["basedata"] else { panic!() };
    assert_eq!(bd.groups, strings(&reference["basedata_points"]["groups"]));
    assert_eq!(bd.labels, strings(&reference["basedata_points"]["labels"]));
    check("points: counts", &bd.counts, &z["bd/points/counts"].data);
    let idx: Vec<f64> = bd.indices.iter().map(|x| f64::from(*x)).collect();
    check("points: indices", &idx, &z["bd/points/indices"].data);
    check("points: coords", &bd.coords, &z["bd/points/coords"].data);
}

fn same_tree(what: &str, t: &terraflux_ops::bio::tree::Tree, want: &serde_json::Value) {
    assert_eq!(t.name, want["name"].as_str().unwrap(), "{what}: name");
    assert_eq!(t.names, strings(&want["names"]), "{what}: names");
    let parent: Vec<i64> = want["parent"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    assert_eq!(t.parent, parent, "{what}: parent");
    check(&format!("{what}: length"), &t.length, &numbers(&want["length"]).unwrap());
    let children: Vec<Vec<usize>> = serde_json::from_value(want["children"].clone()).unwrap();
    assert_eq!(t.children, children, "{what}: children");
}

/// Trees read as Biodiverse reads them (landschaft's test trees and some Newick), and the phylogenetic indices
/// over them (trimmed trees, polytomies, names that do not match, PhyloCom's random tips), to the bit
/// (tests/data/reference/bio_trees.json and bio_phylo.npz).
#[test]
fn trees_and_phylogenetic_indices_are_landschafts() {
    use terraflux_ops::bio::tree::read_trees;
    let reference: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/bio_trees.json")).unwrap())
            .unwrap();
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/bio_phylo.npz")).unwrap();
    let data = root().join("extern/landschaft/tests/data/bio");
    for (f, want) in reference["files"].as_object().unwrap() {
        let text = String::from_utf8_lossy(&std::fs::read(data.join(f)).unwrap()).into_owned();
        let got = read_trees(&text, None).unwrap();
        let want = want.as_array().unwrap();
        assert_eq!(got.len(), want.len(), "{f}: trees");
        for (k, (t, w)) in got.iter().zip(want).enumerate() {
            same_tree(&format!("{f} #{k}"), t, w);
        }
    }
    for case in reference["newicks"].as_array().unwrap() {
        let nwk = case[0].as_str().unwrap();
        let got = read_trees(nwk, None).unwrap();
        for (t, w) in got.iter().zip(case[1].as_array().unwrap()) {
            same_tree(nwk, t, w);
        }
    }

    let text = std::fs::read_to_string(sites()).unwrap();
    let bd_ref: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/bio.json")).unwrap()).unwrap();
    let bd = BaseData::from_table(&text, &import(&bd_ref["basedata"]["cells"]["options"])).unwrap();
    let calcs = strings(&reference["calculations"]);
    for (k, case) in reference["phylo"].as_array().unwrap().iter().enumerate() {
        let f = case["tree"].as_str().unwrap();
        let tree = read_trees(&String::from_utf8_lossy(&std::fs::read(data.join(f)).unwrap()), None).unwrap().remove(0);
        let conds: Vec<Condition> = strings(&case["conditions"]).iter().map(|c| Condition::new(c).unwrap()).collect();
        let what = format!("{f} {:?}", strings(&case["conditions"]));
        let r = spatial_analysis(&bd, &conds, &calcs, None, Some(&tree), 12345, true)
            .unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(r.names, strings(&case["columns"]), "{what}: columns");
        check(&format!("{what}: table"), &r.rows.concat(), &z[&format!("ph/{k}/table")].data);
        assert_eq!(r.nodes, strings(&case["nodes"]), "{what}: nodes");
        let lists = case["lists"].as_array().unwrap();
        assert_eq!(r.lists.len(), lists.len(), "{what}: lists");
        for l in lists {
            let name = l[0].as_str().unwrap();
            let got = r.lists.iter().find(|x| x.name == name).unwrap_or_else(|| panic!("{what}: no list {name}"));
            let p = format!("ph/{k}/list/{name}");
            let keys: Vec<f64> = got.rows.iter().flatten().map(|e| e.0).collect();
            let values: Vec<f64> = got.rows.iter().flatten().map(|e| e.1).collect();
            check(&format!("{what}: {name}: keys"), &keys, &z[&format!("{p}/keys")].data);
            check(&format!("{what}: {name}: values"), &values, &z[&format!("{p}/values")].data);
        }
        assert_eq!(r.texts, strings(&case["texts"]), "{what}: texts");
    }
}

/// bio.tree and bio.spatial with a tree: phylogenetic diversity per group, and the list indices as JSON.
#[test]
fn the_tree_operator_feeds_the_analysis() {
    let r = Registry::standard();
    let run = |op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value| {
        let params = params.as_object().unwrap().clone();
        let args =
            Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
    };
    let nex = root().join("extern/landschaft/src/landschaft/bio/data/example_tree.nex");
    let tree = run("bio.tree", vec![], serde_json::json!({"path": nex.to_str().unwrap()})).remove("tree").unwrap();
    let Value::Tree(t) = &tree else { panic!() };
    assert_eq!(t.terminal_names().len(), 31);
    let bd = run(
        "bio.table",
        vec![],
        serde_json::json!({"path": sites().to_str().unwrap(), "group_columns": ["x", "y"],
        "label_columns": ["genus", "species"], "cell_sizes": [100000, 100000]}),
    )
    .remove("basedata")
    .unwrap();
    let out = run(
        "bio.spatial",
        vec![("basedata", bd), ("tree", tree)],
        serde_json::json!({"conditions": "sp_self_only()", "calculations": ["pd", "pe", "pd_node_list"]}),
    );
    let Value::Table(t) = &out["table"] else { panic!() };
    let pd = t.columns.iter().position(|c| c == "PD").unwrap();
    assert!(t.rows.iter().all(|row| row[pd] > 0.0));
    let Value::Json(lists) = &out["lists"] else { panic!() };
    assert!(lists["PD_INCLUDED_NODE_LIST"]["1950000:1350000"].as_object().is_some_and(|m| !m.is_empty()));
}

/// Randomisations of an analysis with a tree against landschaft's (tests/data/reference/bio_rand.json and
/// bio_rand.npz): every function, with their options, subsets, definition queries, labels kept in place and
/// shuffled trees; the counters, p-ranks, z-scores and CANAPE, scalar and list indices, to the bit.
#[test]
fn randomisations_are_landschafts() {
    use terraflux_ops::bio::rand::{Options, Randomisation};
    use terraflux_ops::bio::spatial::prepare;
    use terraflux_ops::bio::tree::read_trees;
    let reference: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/bio_rand.json")).unwrap())
            .unwrap();
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/bio_rand.npz")).unwrap();
    let bd_ref: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/bio.json")).unwrap()).unwrap();
    let text = std::fs::read_to_string(sites()).unwrap();
    let bd = BaseData::from_table(&text, &import(&bd_ref["basedata"]["cells"]["options"])).unwrap();
    let nex = root().join("extern/landschaft/src/landschaft/bio/data/example_tree.nex");
    let tree = read_trees(&std::fs::read_to_string(nex).unwrap(), None).unwrap().remove(0);
    let calcs = strings(&reference["calculations"]);
    let conds = [Condition::new("sp_circle (radius => 200000)").unwrap()];
    for (k, case) in reference["cases"].as_array().unwrap().iter().enumerate() {
        let a = &case["args"];
        let o = Options {
            richness_multiplier: a["richness_multiplier"].as_f64(),
            richness_addition: a["richness_addition"].as_f64(),
            spatial_conditions_for_label_allocation: a["spatial_conditions_for_label_allocation"]
                .as_str()
                .map(String::from),
            spatial_conditions_for_seed_location: a["spatial_conditions_for_seed_location"].as_str().map(String::from),
            swap_count: a["swap_count"].as_i64(),
            use_max_swap: a["use_max_swap"].as_bool().unwrap_or(false),
            spatial_conditions_for_subset: a["spatial_conditions_for_subset"].as_str().map(String::from),
            definition_query: a["definition_query"].as_str().map(String::from),
            labels_not_to_randomise: strings(&a["labels_not_to_randomise"]),
            randomise_trees_by: a["randomise_trees_by"].as_str().map(String::from),
            ..Options::default()
        };
        let function = case["function"].as_str().unwrap();
        let what = format!("case {k} {function} {a}");
        let run = prepare(&bd, &conds, &calcs, None, Some(&tree), 1, true).unwrap();
        let mut rnd = Randomisation::new(&bd, run, function, case["seed"].as_u64().unwrap(), o).unwrap();
        rnd.run(5).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(rnd.result.names, strings(&reference["columns"]), "{what}: columns");
        let (n, kk) = (rnd.result.groups.len(), rnd.result.names.len());
        let s = &rnd.scalars;
        let p = s.p();
        let mut comparison = Vec::new();
        for g in 0..n {
            for part in [&s.c, &p, &s.q, &s.sumx, &s.sumxx, &s.t] {
                comparison.extend_from_slice(&part[g * kk..(g + 1) * kk]);
            }
        }
        check(&format!("{what}: comparison"), &comparison, &z[&format!("r/{k}/comparison")].data);
        check(&format!("{what}: p_rank"), &s.p_rank(), &z[&format!("r/{k}/p_rank")].data);
        check(&format!("{what}: z"), &s.z_scores(), &z[&format!("r/{k}/z")].data);
        check(&format!("{what}: canape"), &rnd.canape(false, false).unwrap(), &z[&format!("r/{k}/canape")].data);
        let lists = case["lists"].as_array().unwrap();
        assert_eq!(rnd.lists.len(), lists.len(), "{what}: lists");
        for l in lists {
            let name = l[0].as_str().unwrap();
            let (_, keys, c) = rnd.lists.iter().find(|x| x.0 == name).unwrap_or_else(|| panic!("{what}: no {name}"));
            let want_keys: Vec<(String, String)> = strings(&l[1]).into_iter().zip(strings(&l[2])).collect();
            assert_eq!(keys, &want_keys, "{what}: {name}: keys");
            let mut got = Vec::new();
            for i in 0..keys.len() {
                got.extend([c.c[i], c.q[i], c.t[i], c.sumx[i], c.sumxx[i]]);
            }
            check(&format!("{what}: {name}"), &got, &z[&format!("r/{k}/list/{name}")].data);
        }
    }
}

/// bio.randomise: p-ranks, z-scores, the counters and CANAPE as tables and maps.
#[test]
fn the_randomise_operator_gives_tables_and_maps() {
    let r = Registry::standard();
    let run = |op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value| {
        let params = params.as_object().unwrap().clone();
        let args =
            Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
    };
    let nex = root().join("extern/landschaft/src/landschaft/bio/data/example_tree.nex");
    let tree = run("bio.tree", vec![], serde_json::json!({"path": nex.to_str().unwrap()})).remove("tree").unwrap();
    let bd = run(
        "bio.table",
        vec![],
        serde_json::json!({"path": sites().to_str().unwrap(), "group_columns": ["x", "y"],
        "label_columns": ["genus", "species"], "cell_sizes": [100000, 100000]}),
    )
    .remove("basedata")
    .unwrap();
    let out = run(
        "bio.randomise",
        vec![("basedata", bd), ("tree", tree)],
        serde_json::json!({"conditions": "sp_circle (radius => 200000)", "calculations": ["pe", "phylo_rpe2"],
            "iterations": 9, "rand_seed": 3}),
    );
    for t in ["table", "p_rank", "z_scores", "comparison", "canape"] {
        let Value::Table(t) = &out[t] else { panic!("{t}") };
        assert_eq!(t.rows.len(), 127);
    }
    let Value::Stack(s) = &out["maps"] else { panic!() };
    assert!(s.keys.iter().any(|k| k.to_string() == "CANAPE_CODE"));
}

/// Clusterings and region growers against landschaft's (tests/data/reference/bio_cluster.json and .npz): every
/// linkage, direct and pairwise indices, phylogenetic ones, link_recalculate, several conditions, a definition
/// query, the calculations of each node, tie breakers, singletons; the dendrogram, its node values and its cuts,
/// to the bit.
#[test]
fn clusters_are_landschafts() {
    use terraflux_ops::bio::cluster::{ClusterOptions, cluster};
    use terraflux_ops::bio::tree::read_trees;
    let reference: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/bio_cluster.json")).unwrap())
            .unwrap();
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/bio_cluster.npz")).unwrap();
    let bd_ref: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/bio.json")).unwrap()).unwrap();
    let text = std::fs::read_to_string(sites()).unwrap();
    let bd = BaseData::from_table(&text, &import(&bd_ref["basedata"]["cells"]["options"])).unwrap();
    let nex = root().join("extern/landschaft/src/landschaft/bio/data/example_tree.nex");
    let tree = read_trees(&std::fs::read_to_string(nex).unwrap(), None).unwrap().remove(0);
    for (k, entry) in reference["cases"].as_array().unwrap().iter().enumerate() {
        let c = &entry["case"];
        let rg = c["region_grower"].as_bool().unwrap_or(false);
        let o = ClusterOptions {
            index: c["index"].as_str().unwrap_or(if rg { "RICHNESS_ALL" } else { "SORENSON" }).to_string(),
            linkage: c["linkage"].as_str().unwrap_or(if rg { "link_recalculate" } else { "link_average" }).to_string(),
            conditions: strings(&c["conditions"]),
            definition_query: c["definition_query"].as_str().map(String::from),
            calculations: strings(&c["calculations"]),
            seed: 7 + k as u64,
            tie_breaker: strings(&c["tie_breaker"]),
            clear_singletons: c["clear_singletons"].as_bool().unwrap_or(false),
            region_grower: rg,
        };
        let what = format!("case {k} {c}");
        let t = c["tree"].as_bool().unwrap_or(false).then_some(&tree);
        let r = cluster(&bd, t, &o, true).unwrap_or_else(|e| panic!("{what}: {e}"));
        assert_eq!(r.tree.names, strings(&entry["names"]), "{what}: names");
        let parent: Vec<i64> = entry["parent"].as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
        assert_eq!(r.tree.parent, parent, "{what}: parent");
        let children: Vec<Vec<usize>> = serde_json::from_value(entry["children"].clone()).unwrap();
        assert_eq!(r.tree.children, children, "{what}: children");
        check(&format!("{what}: lengths"), &r.tree.length, &z[&format!("c/{k}/length")].data);
        let values: Vec<f64> = r.nodes.iter().flat_map(|n| n.values.map(|v| v.unwrap_or(f64::NAN))).collect();
        check(&format!("{what}: values"), &values, &z[&format!("c/{k}/values")].data);
        if let Some(cols) = entry.get("columns") {
            let t = r.table.as_ref().unwrap();
            assert_eq!(t.names, strings(cols), "{what}: columns");
            check(&format!("{what}: table"), &t.rows.concat(), &z[&format!("c/{k}/table")].data);
        }
        for (kk, want) in entry["cut"].as_object().unwrap() {
            let got = r.cut(kk.parse().unwrap()).unwrap();
            let want: Vec<(String, usize)> =
                want.as_object().unwrap().iter().map(|(g, v)| (g.clone(), v.as_u64().unwrap() as usize)).collect();
            let mut want = want;
            want.sort();
            assert_eq!(got, want, "{what}: cut {kk}");
        }
    }
}

/// bio.cluster and bio.region_grower: the dendrogram, its nodes, a node's calculations and k clusters as a map.
#[test]
fn the_cluster_operators_cut_maps() {
    let r = Registry::standard();
    let run = |op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value| {
        let params = params.as_object().unwrap().clone();
        let args =
            Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
        r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
    };
    let bd = run(
        "bio.table",
        vec![],
        serde_json::json!({"path": sites().to_str().unwrap(), "group_columns": ["x", "y"],
        "label_columns": ["genus", "species"], "cell_sizes": [100000, 100000]}),
    )
    .remove("basedata")
    .unwrap();
    for op in ["bio.cluster", "bio.region_grower"] {
        let out = run(op, vec![("basedata", bd.clone())], serde_json::json!({"k": 6, "calculations": ["richness"]}));
        let Value::Tree(t) = &out["tree"] else { panic!() };
        assert_eq!(t.terminal_names().len(), 127, "{op}");
        let Value::Table(n) = &out["nodes"] else { panic!() };
        assert_eq!(n.rows.len(), t.n_nodes(), "{op}");
        let Value::Map(m) = &out["map"] else { panic!() };
        let mut ks: Vec<i64> = m.data.iter().filter(|v| !v.is_nan()).map(|v| *v as i64).collect();
        ks.sort_unstable();
        ks.dedup();
        if op == "bio.cluster" {
            assert_eq!(ks, (1..=6).collect::<Vec<_>>(), "{op}");
        } else {
            // the region grower stops once a region holds every label: the regions left join under one root,
            // and undoing that join gives them all
            assert!(ks.len() >= 6 && ks[0] == 1, "{op}");
        }
        assert!(matches!(&out["table"], Value::Table(t) if t.columns.contains(&"RICHNESS_ALL".to_string())));
    }
}
