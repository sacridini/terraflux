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
        let r = spatial_analysis(&bd, &conds, &calcs, query.as_ref(), true).unwrap_or_else(|e| panic!("{what}: {e}"));
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
