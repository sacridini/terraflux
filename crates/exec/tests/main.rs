//! The integration tests of the runs in one program (each file a module): every test program links landschaft's
//! C++ whole, so one program instead of one per file keeps `target` and the links small.

mod abm;
mod bridge;
mod cache;
mod calc;
mod debug;
mod each;
mod futures_calib;
mod parity;
mod per_layer;
mod ports;
mod product;
mod settings;
mod switch;

/// Every file of `tests` is a module here (or a program of its own in Cargo.toml): with `autotests = false` a file
/// left out would not run.
#[test]
fn every_test_file_runs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let main = include_str!("main.rs");
    let apart: [&str; 0] = [];
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
