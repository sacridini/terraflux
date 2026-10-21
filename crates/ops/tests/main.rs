//! The integration tests of the operators in one program (each file a module): every test program links
//! landschaft's C++ whole, so one program instead of one per file keeps `target` and the links small.
//! `circuit_ini` is a program of its own: it changes the process's folder (Circuitscape's INI files name theirs
//! relative to it).

mod accuracy;
mod actions;
mod actions_raster;
mod batch;
mod bio;
mod bio_imports;
mod bio_shapes;
mod carbon;
mod change_analysis;
mod circuit;
mod conn;
mod conn_raster;
mod conn_vector;
mod cost;
mod data;
mod differ;
mod dispersal;
mod extinction;
mod forest_code;
mod futures_tools;
mod h3;
mod hydro;
mod interpolation;
mod landforms;
mod legend;
mod map_validation;
mod marxan;
mod metapopulation;
mod metrics;
mod ml;
mod mspa_tiled;
mod nlm;
mod ops;
mod optimize;
mod prioritize_tools;
mod protconn;
mod proxirr;
mod redd;
mod restoration;
mod rivers;
mod sampling;
mod sdm_area;
mod sdm_clean;
mod sdm_evaluate;
mod sdm_extrapolation;
mod sdm_folds;
mod sdm_gbif;
mod sdm_maxnet;
mod sdm_models;
mod sdm_points;
mod sdm_predictors;
mod sdm_prepare;
mod sdm_workflow;
mod sensitivity;
mod services;
mod signatures;
mod stats;
mod surface;
mod swat;
mod tiled_metrics;
mod uncertainty;
mod vector;
mod vector_ops;
mod vector_tiled;
mod woe_edit;
mod zonation;
mod zonation_project;
mod zones;

/// Every file of `tests` is a module here (or a program of its own in Cargo.toml): with `autotests = false` a file
/// left out would not run.
#[test]
fn every_test_file_runs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let main = include_str!("main.rs");
    let apart: [&str; 1] = ["circuit_ini"];
    for f in std::fs::read_dir(dir).unwrap() {
        let p = f.unwrap().path();
        let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
        if p.extension().is_some_and(|e| e == "rs") && stem != "main" && !apart.contains(&stem.as_str()) {
            assert!(
                main.contains(&format!(
                    "
mod {stem};
"
                )),
                "tests/{stem}.rs is not a module of tests/main.rs"
            );
        }
    }
}
