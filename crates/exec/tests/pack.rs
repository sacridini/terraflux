//! A model packed with its data (`.tfpack`) and unpacked in another folder runs by the engine and gives the same
//! outputs as the original: rasters of the same name from two folders, a table, the rasters of a pattern and a
//! submodel with data of its own.

use std::path::{Path, PathBuf};

use terraflux_exec::pack::{self, Role};
use terraflux_exec::{Engine, Quiet, load_inputs, write_outputs};
use terraflux_graph::Model;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terraflux-pack-run-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A raster of 12 x 9 cells whose values start at `start`, written to `file`.
fn raster(file: &Path, start: f64) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    let grid = terraflux_raster::Grid::new(12, 9);
    let data: Vec<f64> = (0..12 * 9).map(|i| start + (i % 7) as f64).collect();
    let r = terraflux_raster::Raster {
        grid,
        bands: vec![terraflux_raster::Band { name: None, data }],
        dtype: terraflux_raster::DataType::F32,
        nodata: None,
    };
    terraflux_ops::prepare::write_with_crs(&r, file).unwrap();
}

fn write(file: &Path, text: &str) {
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

const MODEL: &str = r#"
format = "terraflux/1"

[[nodes]]
id = "a"
op = "raster.read"
params = { path = "a/land.tif" }

[[nodes]]
id = "b"
op = "raster.read"
params = { path = "b/land.tif" }

[[nodes]]
id = "t"
op = "table.read"
params = { path = "tables/rates.csv" }

[[nodes]]
id = "runs"
op = "raster.read"
params = { path = "runs/*/out.tif" }

[[nodes]]
id = "s"
op = "control.submodel"
params = { path = "sub/part.toml" }

[outputs.a]
from = "a.raster"

[outputs.b]
from = "b.raster"

[outputs.t]
from = "t.table"

[outputs.s]
from = "s.m"
"#;

const SUB: &str = r#"
format = "terraflux/1"

[[nodes]]
id = "l"
op = "raster.read"
params = { path = "land.tif" }

[outputs.m]
from = "l.raster"
"#;

/// Run a model file and write its outputs to `out`.
fn run(model: &Path, out: &Path) {
    let m = Model::load_expanded(model).unwrap();
    let inputs = load_inputs(&m, model.parent().unwrap(), &Default::default()).unwrap();
    let results = Engine::default().run(&m, &inputs, &mut Quiet).unwrap();
    write_outputs(&m, &results, out).unwrap();
}

fn values(file: &Path) -> Vec<f64> {
    terraflux_raster::Raster::read(file).unwrap().bands[0].data.clone()
}

#[test]
fn a_model_packed_and_unpacked_elsewhere_gives_the_same_outputs() {
    let d = folder("same");
    raster(&d.join("a/land.tif"), 1.0);
    raster(&d.join("b/land.tif"), 20.0);
    raster(&d.join("runs/x/out.tif"), 3.0);
    raster(&d.join("runs/y/out.tif"), 5.0);
    raster(&d.join("sub/land.tif"), 100.0);
    write(&d.join("tables/rates.csv"), "class,rate\n1,0.5\n2,0.25\n");
    write(&d.join("sub/part.toml"), SUB);
    write(&d.join("model.toml"), MODEL);
    run(&d.join("model.toml"), &d.join("out1"));

    let plan = pack::plan(&d.join("model.toml"), &[]).unwrap();
    assert!(plan.missing.is_empty(), "{:?}", plan.missing);
    let package = d.join("study.tfpack");
    let (manifest, _) = plan.write(&package).unwrap();
    assert_eq!(manifest.files.iter().filter(|f| f.role == Role::Data).count(), 6);
    assert!(manifest.files.iter().filter(|f| f.role == Role::Data).all(|f| f.sha256.len() == 64 && f.bytes > 0));

    // unpacked somewhere else, the original data gone: nothing it reads is outside the folder
    let there = folder("same-there");
    let done = pack::unpack(&package, &there.join("study")).unwrap();
    assert!(done.problems.is_empty(), "{:?}", done.problems);
    for gone in ["a", "b", "runs", "sub", "tables"] {
        std::fs::remove_dir_all(d.join(gone)).unwrap();
    }
    run(&done.model, &there.join("out2"));
    for name in ["a", "b", "s"] {
        let (one, two) = (
            values(&d.join("out1").join(format!("{name}.tif"))),
            values(&there.join("out2").join(format!("{name}.tif"))),
        );
        assert_eq!(one, two, "{name}");
    }
    assert_eq!(values(&there.join("out2/a.tif"))[0], 1.0);
    assert_eq!(values(&there.join("out2/b.tif"))[0], 20.0, "the second land.tif is not the first");
    assert_eq!(values(&there.join("out2/s.tif"))[0], 100.0);
    assert_eq!(
        std::fs::read_to_string(d.join("out1/t.csv")).unwrap(),
        std::fs::read_to_string(there.join("out2/t.csv")).unwrap()
    );
    let _ = std::fs::remove_dir_all(&d);
    let _ = std::fs::remove_dir_all(&there);
}

#[test]
fn a_file_the_model_names_that_is_not_there_is_listed() {
    let d = folder("missing");
    raster(&d.join("a/land.tif"), 1.0);
    write(&d.join("model.toml"), MODEL);
    let plan = pack::plan(&d.join("model.toml"), &[]).unwrap();
    let said: Vec<&str> = plan.missing.iter().map(|m| m.at.as_str()).collect();
    assert_eq!(said, ["b", "t", "runs", "s"]);
    let (manifest, _) = plan.write(&d.join("m.tfpack")).unwrap();
    assert_eq!(manifest.missing, plan.missing);
    let _ = std::fs::remove_dir_all(&d);
}
