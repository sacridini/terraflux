//! SWAT+ against landschaft's (tests/data/reference/swat.npz, tools/make_swat_reference.py): the Hubbard Brook W2
//! example run at every step, the tables SWAT+ prints and the daily weather, to the bit.

use std::path::Path;

use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: Default::default(), params: &params };
    r.get("swat.run").unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("swat.run: {e}"))
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn example() -> std::path::PathBuf {
    root().join("extern/landschaft/src/landschaft/hydro/swat/data/hubbard_brook.zip")
}

/// An operator of the family run with inputs and settings.
fn op(name: &str, inputs: &[(&str, &Value)], params: serde_json::Value) -> Result<Outputs, terraflux_ops::Error> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let inputs = inputs.iter().map(|(k, v)| (k.to_string(), (*v).clone())).collect();
    let args = Args { node: "n", inputs, params: &params };
    r.get(name).unwrap().run(&mut Ctx::default(), &args)
}

fn load() -> Value {
    op("swat.load", &[], serde_json::json!({ "path": example().to_str().unwrap() })).unwrap().remove("project").unwrap()
}

fn tables_equal(a: &Outputs, b: &Outputs, ports: &[&str]) -> usize {
    let mut n = 0;
    for p in ports {
        let (Value::Table(x), Value::Table(y)) = (&a[*p], &b[*p]) else { panic!("{p}") };
        assert_eq!(x.rows.len(), y.rows.len(), "{p}: rows");
        n += x.rows.iter().flatten().zip(y.rows.iter().flatten()).filter(|(u, v)| !same(**u, **v)).count();
    }
    n
}

#[test]
fn swat_is_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/swat.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/swat.json")).unwrap()).unwrap();
    let example = root().join("extern/landschaft/src/landschaft/hydro/swat/data/hubbard_brook.zip");
    for case in meta["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut params = case.as_object().unwrap().clone();
        params.remove("name");
        params.insert("path".into(), serde_json::json!(example.to_str().unwrap()));
        let out = run(serde_json::Value::Object(params));
        let mut checked = 0;
        for (key, want) in z.iter().filter(|(k, _)| k.starts_with(&format!("{name}/"))) {
            let port = &key[name.len() + 1..];
            let Value::Table(t) = &out[port] else { panic!("{key}") };
            assert_eq!(t.rows.len(), want.shape[0], "{key}: rows");
            assert_eq!(t.columns.len(), want.shape[1], "{key}: columns");
            let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
            let bad = got.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
            assert_eq!(bad, 0, "{key}: {bad} values differ");
            checked += 1;
        }
        assert!(checked > 1, "{name}");
    }
}

#[test]
fn a_loaded_project_runs_as_its_path_does() {
    let project = load();
    let Value::SwatProject(p) = &project else { panic!() };
    assert!(p.files.iter().any(|(n, _)| n == "file.cio"));
    let by_path = run(serde_json::json!({ "path": example().to_str().unwrap() }));
    let by_project = op("swat.run", &[("project", &project)], serde_json::json!({})).unwrap();
    let ports = ["basin_wb", "hru_wb", "hru_ls", "channel_sd", "aquifer", "weather"];
    assert_eq!(tables_equal(&by_path, &by_project, &ports), 0);
}

#[test]
fn a_run_takes_a_project_or_a_path_not_neither_nor_both() {
    let none = op("swat.run", &[], serde_json::json!({})).err().unwrap().to_string();
    assert!(none.contains("connect a project") && none.contains("TxtInOut"), "{none}");
    let both = op("swat.run", &[("project", &load())], serde_json::json!({ "path": example().to_str().unwrap() }))
        .err()
        .unwrap()
        .to_string();
    assert!(both.contains("not both"), "{both}");
    let lone = op("swat.load", &[], serde_json::json!({ "path": "no/such/folder" })).err().unwrap().to_string();
    assert!(lone.contains("not a folder or a zip"), "{lone}");
}

#[test]
fn set_is_landschafts_calibration() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/swat_set.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/swat_set.json")).unwrap())
            .unwrap();
    let base = load();
    let plain = op("swat.run", &[("project", &base)], serde_json::json!({})).unwrap();
    for case in meta["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        let mut project = base.clone();
        for c in case["changes"].as_array().unwrap() {
            let mut params = c.as_object().unwrap().clone();
            if let Some(o) = params.get("objects") {
                assert!(o.is_array());
            }
            params.entry("change").or_insert(serde_json::json!("absval"));
            project = op("swat.set", &[("project", &project)], serde_json::Value::Object(params))
                .unwrap_or_else(|e| panic!("{name}: {e}"))
                .remove("project")
                .unwrap();
        }
        let out = op("swat.run", &[("project", &project)], serde_json::json!({})).unwrap();
        let mut checked = 0;
        for (key, want) in z.iter().filter(|(k, _)| k.starts_with(&format!("{name}/"))) {
            let port = &key[name.len() + 1..];
            let Value::Table(t) = &out[port] else { panic!("{key}") };
            assert_eq!(t.rows.len(), want.shape[0], "{key}: rows");
            assert_eq!(t.columns.len(), want.shape[1], "{key}: columns");
            let got: Vec<f64> = t.rows.iter().flatten().copied().collect();
            let bad = got.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
            assert_eq!(bad, 0, "{key}: {bad} values differ");
            checked += 1;
        }
        assert!(checked > 5, "{name}");
        // the change reaches the tables
        assert!(tables_equal(&plain, &out, &["hru_wb", "basin_wb"]) > 0, "{name}: nothing changed");
    }
}

#[test]
fn set_leaves_the_project_it_was_given() {
    let base = load();
    let Value::SwatProject(before) = &base else { panic!() };
    let out = op(
        "swat.set",
        &[("project", &base)],
        serde_json::json!({ "parameter": "awc", "change": "pctchg", "value": 10 }),
    )
    .unwrap();
    let Value::SwatProject(after) = &out["project"] else { panic!() };
    let kept = before.files.clone();
    // the input is as it was; the copy differs in calibration.cal alone
    let Value::SwatProject(again) = &base else { panic!() };
    assert_eq!(again.files, kept);
    assert_eq!(before.files.len(), after.files.len());
    let differ: Vec<&str> =
        before.files.iter().zip(&after.files).filter(|(a, b)| a != b).map(|(a, _)| a.0.as_str()).collect();
    assert_eq!(differ, ["calibration.cal"]);
    // the row is added after the 12 the file has, and the count says so
    let text = String::from_utf8(after.files.iter().find(|(n, _)| n == "calibration.cal").unwrap().1.clone()).unwrap();
    assert_eq!(text.lines().nth(1).unwrap().trim(), "13");
    assert!(text.lines().nth(15).unwrap().starts_with("awc"), "{text}");
    // what the port does not calibrate is refused, with the reason
    let plant = op("swat.set", &[("project", &base)], serde_json::json!({ "parameter": "usle_c", "value": 0.1 }));
    assert!(plant.err().unwrap().to_string().contains("plants"));
    let soil = op(
        "swat.set",
        &[("project", &base)],
        serde_json::json!({ "parameter": "surlag", "value": 1, "first_layer": 2 }),
    );
    assert!(soil.err().unwrap().to_string().contains("no layers"));
}

#[test]
fn a_project_is_written_and_read_back_as_it_was() {
    let Value::SwatProject(p) = load() else { panic!() };
    let text = serde_json::to_string(&*p).unwrap();
    let back: terraflux_ops::swat::Project = serde_json::from_str(&text).unwrap();
    assert_eq!(back.files, p.files);
}
