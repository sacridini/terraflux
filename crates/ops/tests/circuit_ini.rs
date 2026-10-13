//! Circuitscape's and Omniscape's INI files against landschaft's (tests/data/reference/circuit_ini.npz and
//! .json, tools/make_circuit_reference.py): Circuitscape's raster test cases run from its folder (advanced,
//! pairwise, one-to-all and all-to-one, with their polygons, masks, reclassifications, included pairs and
//! variable strengths; ASCII grids, gzipped ones, GeoTIFFs and text lists) and the Omniscape INI files of
//! landschaft's references, to the bit. (Its own test binary: it changes the working directory.)

use std::path::{Path, PathBuf};

use terraflux_ops::{Args, Ctx, Outputs, Registry, Value};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(op: &str, params: serde_json::Value) -> std::result::Result<Outputs, String> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args = Args { node: "n", inputs: Default::default(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

fn same(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan())
}

fn bits(got: &[f64], want: &[f64], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}: length");
    if let Some(i) = got.iter().zip(want).position(|(a, b)| !same(*a, *b)) {
        panic!("{what}: at {i}: {} {}", got[i], want[i]);
    }
}

fn values(v: &Value) -> Vec<f64> {
    match v {
        Value::Map(m) => m.data.to_vec(),
        Value::Table(t) => t.rows.iter().flatten().copied().collect(),
        v => panic!("{v:?}"),
    }
}

#[test]
fn ini_files_are_landschafts() {
    let z = terraflux_raster::npy::read_npz(root().join("tests/data/reference/circuit_ini.npz")).unwrap();
    let meta: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root().join("tests/data/reference/circuit_ini.json")).unwrap())
            .unwrap();
    let omni = root().join("extern/landschaft/tests/data/reference/circuit/omniscape_ini");
    for case in meta["omniscape"].as_array().unwrap() {
        let ini = case["ini"].as_str().unwrap();
        let out = run(
            "circuit.omniscape_ini",
            serde_json::json!({"path": omni.join(ini).to_str().unwrap(), "write_outputs": false}),
        );
        if let Some(e) = case.get("error") {
            assert!(out.is_err(), "{ini}: expected {e}");
            continue;
        }
        let out = out.unwrap_or_else(|e| panic!("{ini}: {e}"));
        for k in case["keys"].as_array().unwrap() {
            let k = k.as_str().unwrap();
            let port = match k {
                "cum_currmap" => "current",
                "normalized_cum_currmap" => "normalized",
                k => k,
            };
            bits(&values(&out[port]), &z[&format!("om/{ini}/{k}")].data, &format!("{ini} {k}"));
        }
    }

    let cs = root().join("extern/landschaft/tests/data/reference/circuit/circuitscape");
    std::env::set_current_dir(&cs).unwrap();
    for case in meta["circuitscape"].as_array().unwrap() {
        let ini = case["ini"].as_str().unwrap();
        let out = run("circuit.circuitscape_ini", serde_json::json!({"path": ini, "write_outputs": false, "batch": 4}));
        if let Some(e) = case.get("error") {
            assert!(out.is_err(), "{ini}: expected {e}");
            continue;
        }
        let out = out.unwrap_or_else(|e| panic!("{ini}: {e}"));
        for k in case["keys"].as_array().unwrap() {
            let k = k.as_str().unwrap();
            let want = &z[&format!("cs/{ini}/{k}")];
            match k {
                "resistances" if want.shape.len() == 2 && want.shape[1] > 2 => {
                    // ids in the first row and column; ours a row per id, its id first
                    let n = want.shape[1];
                    let rows: Vec<f64> =
                        (1..want.shape[0]).flat_map(|i| want.data[i * n..(i + 1) * n].to_vec()).collect();
                    bits(&values(&out["resistances"]), &rows, &format!("{ini} resistances"));
                }
                "resistances" => bits(&values(&out["resistances"]), &want.data, &format!("{ini} resistances")),
                "cum_current" => bits(&values(&out["cum"]), &want.data, &format!("{ini} cum")),
                "max_current" => bits(&values(&out["max"]), &want.data, &format!("{ini} max")),
                k => bits(&values(&out[k]), &want.data, &format!("{ini} {k}")),
            }
        }
        let maps = case["maps"].as_array().unwrap();
        if maps.is_empty() {
            continue;
        }
        let Value::Stack(s) = &out["maps"] else { panic!("{ini}: no maps") };
        let mut keys: Vec<String> = s.keys.iter().map(|k| k.to_string()).collect();
        keys.sort();
        let want_keys: Vec<String> = maps.iter().map(|m| m.as_str().unwrap().to_string()).collect();
        assert_eq!(keys, want_keys, "{ini}: maps");
        for (k, layer) in s.keys.iter().zip(&s.layers) {
            bits(layer, &z[&format!("cs/{ini}/maps/{k}")].data, &format!("{ini} {k}"));
        }
    }
}

/// The files written for three of Circuitscape's INI files (pairwise with every map, advanced, one-to-all) are
/// landschaft's, byte for byte (tests/data/reference/circuit_ini_out): the ASCII grids, the resistances and the
/// configuration.
#[test]
fn written_outputs_are_landschafts() {
    let cs = root().join("extern/landschaft/tests/data/reference/circuit/circuitscape");
    let want = root().join("tests/data/reference/circuit_ini_out");
    let tmp = root().join("target/tmp/circuit_ini_out");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::env::set_current_dir(&cs).unwrap();
    for rel in [
        "input/raster/pairwise/1/sgVerify1.ini",
        "input/raster/advanced/1/mgVerify1.ini",
        "input/raster/one_to_all/1/oneToAllVerify1.ini",
    ] {
        let name = Path::new(rel).file_stem().unwrap().to_str().unwrap();
        let out_file = tmp.join(name).join(format!("{name}.out"));
        let text: String = std::fs::read_to_string(cs.join(rel))
            .unwrap()
            .lines()
            .map(|l| {
                if l.starts_with("output_file") {
                    format!("output_file = {}\n", out_file.to_str().unwrap().replace(std::path::MAIN_SEPARATOR, "/"))
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        let ini = tmp.join(format!("{name}.ini"));
        std::fs::write(&ini, text).unwrap();
        run("circuit.circuitscape_ini", serde_json::json!({"path": ini.to_str().unwrap(), "batch": 4})).unwrap();
        let mut files: Vec<String> = std::fs::read_dir(want.join(name))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        files.sort();
        let mut ours: Vec<String> = std::fs::read_dir(tmp.join(name))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        ours.sort();
        assert_eq!(ours, files, "{name}: the files written");
        for f in files {
            let (a, b) = (
                std::fs::read_to_string(tmp.join(name).join(&f)).unwrap(),
                std::fs::read_to_string(want.join(name).join(&f)).unwrap(),
            );
            let strip = |t: &str| t.lines().filter(|l| !l.starts_with("output_file")).collect::<Vec<_>>().join("\n");
            assert!(strip(&a) == strip(&b), "{name}/{f} differs");
        }
    }
}
