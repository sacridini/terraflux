//! The integration tests of the application in one program (each file a module), so that `target` keeps one
//! program instead of one per file. `ui` and `readme_images` are programs of their own: each points the
//! settings at a file of its own (`TERRAFLUX_SETTINGS`).

mod calculator;
mod canvas;
mod charts;
mod prepare;
mod vector_tiles;

/// Every file of `tests` is a module here (or a program of its own in Cargo.toml): with `autotests = false` a file
/// left out would not run.
#[test]
fn every_test_file_runs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    // (a checkout with CRLF: the string below has LF, as Rust reads its literals)
    let main = include_str!("main.rs").replace("\r\n", "\n");
    let apart: [&str; 2] = ["ui", "readme_images"];
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
