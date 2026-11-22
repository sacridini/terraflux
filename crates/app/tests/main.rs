//! The integration tests of the application in one program (each file a module), so that `target` keeps one
//! program instead of one per file. `ui`, `fit`, `keymap`, `vision` and `readme_images` are programs of their own: each points the
//! settings at a file of its own (`TERRAFLUX_SETTINGS`; `fit` changes the theme and the size of the text, which
//! are the whole program's).

mod calculator;
mod canvas;
mod charts;
mod class_tree;
mod cloud_public;
mod cluster_run;
mod confusion_view;
mod my_nodes;
mod objects_space;
mod objects_live;
mod objects_view;
mod objects_viewer;
mod optimize_view;
mod prepare;
mod reference_view;
mod sam_first_time;
mod segment_compare;
mod som_view;
mod time_estimate;
mod vector_calc;
mod vector_tiles;

/// The settings of this program: one file for all its tests (`TERRAFLUX_SETTINGS` set once). Each module pointing
/// them at a file of its own moved the others' data folder (`settings::data_dir`) under them while they ran, and a
/// test that pointed them nowhere wrote to the user's own settings. Called before any application is made.
pub fn point_settings() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("app-tests");
        std::fs::create_dir_all(&dir).unwrap();
        // (a run starts from the defaults, not from what the run before left; the theme "Classic", whose sizes
        // the canvas's tests measure by)
        std::fs::write(dir.join("settings.json"), r#"{ "theme": "Classic", "minimap": false }"#).unwrap();
        unsafe { std::env::set_var("TERRAFLUX_SETTINGS", dir.join("settings.json")) };
    });
}

/// Every file of `tests` is a module here (or a program of its own in Cargo.toml): with `autotests = false` a file
/// left out would not run.
#[test]
fn every_test_file_runs() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    // (a checkout with CRLF: the string below has LF, as Rust reads its literals)
    let main = include_str!("main.rs").replace("\r\n", "\n");
    // (or of `ui`, whose application and settings it shares: `web_layers`)
    let ui = include_str!("ui.rs").replace("\r\n", "\n");
    let apart: [&str; 5] = ["ui", "fit", "keymap", "vision", "readme_images"];
    for f in std::fs::read_dir(dir).unwrap() {
        let p = f.unwrap().path();
        let stem = p.file_stem().unwrap().to_string_lossy().into_owned();
        if p.extension().is_some_and(|e| e == "rs") && stem != "main" && !apart.contains(&stem.as_str()) {
            let line = format!(
                "
mod {stem};
"
            );
            assert!(
                main.contains(&line) || ui.contains(&line),
                "tests/{stem}.rs is not a module of tests/main.rs (nor of tests/ui.rs)"
            );
        }
    }
}
