//! prioritize.zonation_project against Zonation 5 v2.4: the projects of landschaft's 65 cases (settings.z5, the
//! feature list, link files, rasters; tests/data/reference/zonation_projects, tools/make_zonation_projects.py) read
//! and run, their rank maps as the z5 binary's (extern/landschaft/tests/data/reference/prioritize) bit for bit;
//! a post-processing file's calls one after another.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use terraflux_ops::{Args, Ctx, Registry, Value};
use terraflux_raster::npy::Array;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn projects() -> serde_json::Value {
    let p = root().join("tests/data/reference/zonation_projects/projects.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

/// A case's project written into a directory of its own.
fn write_case(name: &str, case: &serde_json::Value) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terraflux-z5-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (file, text) in case["files"].as_object().unwrap() {
        std::fs::write(dir.join(file), text.as_str().unwrap()).unwrap();
    }
    let src = root().join("tests/data/reference/zonation_projects");
    for r in case["rasters"].as_array().unwrap() {
        let r = r.as_str().unwrap();
        std::fs::copy(src.join(r), dir.join(r)).unwrap();
    }
    dir
}

fn run(params: serde_json::Value) -> Result<terraflux_ops::Outputs, String> {
    let r = Registry::standard();
    let params = params.as_object().unwrap().clone();
    let args = Args { node: "n", inputs: BTreeMap::new(), params: &params };
    r.get("prioritize.zonation_project").unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

fn run_case(name: &str, case: &serde_json::Value) -> (terraflux_ops::Outputs, PathBuf) {
    let dir = write_case(name, case);
    let out = run(serde_json::json!({
        "path": dir.join("settings.z5").to_string_lossy(),
        "flags": case["flags"],
        "mode": case["mode"],
        "bits": case["bits"],
    }))
    .unwrap_or_else(|e| panic!("{name}: {e}"));
    (out, dir)
}

fn same(a: f64, b: f64) -> bool {
    (a as f32).to_bits() == (b as f32).to_bits() || (a.is_nan() && b.is_nan())
}

fn reference(group: &str) -> (BTreeMap<String, Array>, serde_json::Value) {
    let refdir = root().join("extern/landschaft/tests/data/reference/prioritize");
    let arrays = terraflux_raster::npy::read_npz(refdir.join(format!("{group}.npz"))).unwrap();
    let meta = serde_json::from_str(&std::fs::read_to_string(refdir.join(format!("{group}.json"))).unwrap()).unwrap();
    (arrays, meta)
}

/// Every case's project (every rule, 16 bits, weights and weight groups, thresholds, zero modes, value
/// extraction, costs, hierarchic masks, analysis areas, LOAD; conditions, retention and the transforms;
/// connectivity kernels, interactions and matrices; impact scenarios; post-processing): the rank and the
/// iterations as the z5 binary's.
#[test]
fn projects_rank_as_zonation() {
    let all = projects();
    let mut refs: BTreeMap<String, (BTreeMap<String, Array>, serde_json::Value)> = BTreeMap::new();
    for (name, case) in all.as_object().unwrap() {
        let group = case["group"].as_str().unwrap();
        let (arrays, meta) = refs.entry(group.to_string()).or_insert_with(|| reference(group));
        let (out, dir) = run_case(name, case);
        let Value::Map(rank) = &out["rank"] else { panic!() };
        let want = &arrays[&format!("{name}/rank")];
        let differ = rank.data.iter().zip(&want.data).filter(|(a, b)| !same(**a, **b)).count();
        assert_eq!(differ, 0, "{name}: {differ} cells differ from Zonation's rank");
        let stats = &meta["cases"][format!("{name}/stats")];
        let iterations: f64 =
            stats.get("Number of iterations").and_then(|v| v.as_str()).map_or(0.0, |v| v.parse().unwrap());
        let Value::Number(it) = out["iterations"] else { panic!() };
        assert_eq!(it, iterations, "{name}: iterations");
        std::fs::remove_dir_all(dir).ok();
    }
    assert_eq!(all.as_object().unwrap().len(), 65);
}

/// A post-processing file's calls (statistics of external areas, of a range of ranks and of its separate areas,
/// a comparison, two histograms, two flexibility maps) one after another: the tables numbered by call, the masks
/// as Zonation's files bit for bit, the maps named as its files.
#[test]
fn post_processing_calls() {
    let all = projects();
    let name = "post_caz2";
    let (out, dir) = run_case(name, &all[name]);
    let (arrays, _) = reference("post");
    let Value::Table(stats) = &out["area_statistics"] else { panic!("no area statistics") };
    assert_eq!(stats.columns[0], "call");
    let calls: std::collections::BTreeSet<i64> = stats.rows.iter().map(|r| r[0] as i64).collect();
    assert_eq!(calls.into_iter().collect::<Vec<_>>(), [1, 2, 3]);
    let Value::Table(h) = &out["histogram"] else { panic!("no histogram") };
    assert!(h.rows.iter().any(|r| r[0] == 5.0) && h.rows.iter().any(|r| r[0] == 6.0));
    let Value::Table(c) = &out["comparison"] else { panic!("no comparison") };
    assert_eq!(c.rows.len(), 1);
    assert_eq!(c.rows[0][0], 4.0);
    let Value::Stack(maps) = &out["post_maps"] else { panic!("no maps") };
    let keys: Vec<String> = maps.keys.iter().map(|k| k.to_string()).collect();
    assert_eq!(
        keys,
        [
            "area_statistics_rank_range.mask.top",
            "area_statistics_separate_areas.mask.sep",
            "solution_comparison.mask.cmp",
            "flexibilitymap.flex.up",
            "flexibilitymap.flex.down",
            "flexibilitymap.flex",
            "flexibilitymap.wide",
        ]
    );
    for (k, layer) in keys.iter().zip(&maps.layers).take(3) {
        let want = &arrays[&format!("{name}/load/post/{k}.tif")];
        assert_eq!(layer.as_slice(), want.data.as_slice(), "{k}");
    }
    std::fs::remove_dir_all(dir).ok();
}

/// The settings file's and the feature list's errors, and the switches' forms.
#[test]
fn errors_and_forms() {
    let all = projects();
    let case = &all["caz2_weights"];
    let dir = write_case("forms", case);
    let path = dir.join("settings.z5").to_string_lossy().into_owned();
    // the z5 command line pasted: --mode= among the switches, the words of the files left out
    let a = run(serde_json::json!({"path": path, "flags": format!("{} --mode=CAZ2 settings.z5 out", case["flags"].as_str().unwrap())}))
        .unwrap();
    let b = run(serde_json::json!({"path": path, "flags": case["flags"], "mode": "CAZ2"})).unwrap();
    let (Value::Map(ra), Value::Map(rb)) = (&a["rank"], &b["rank"]) else { panic!() };
    assert!(ra.data.iter().zip(rb.data.iter()).all(|(x, y)| same(*x, *y)));
    // without -w the weights are left out
    let c = run(serde_json::json!({"path": path, "mode": "CAZ2"})).unwrap();
    let Value::Table(f) = &c["features"] else { panic!() };
    assert!(f.rows.iter().all(|r| r[1] == 1.0));
    assert!(run(serde_json::json!({"path": dir.join("none.z5").to_string_lossy()})).is_err());
    std::fs::write(dir.join("empty.z5"), "# nothing\n").unwrap();
    let e = run(serde_json::json!({"path": dir.join("empty.z5").to_string_lossy()})).unwrap_err();
    assert!(e.contains("feature list file"), "{e}");
    std::fs::write(dir.join("bad.txt"), "weight filename\n1.0\n").unwrap();
    std::fs::write(dir.join("bad.z5"), "feature list file = bad.txt\n").unwrap();
    let e = run(serde_json::json!({"path": dir.join("bad.z5").to_string_lossy()})).unwrap_err();
    assert!(e.contains("1 fields for 2 columns"), "{e}");
    std::fs::remove_dir_all(dir).ok();
}
