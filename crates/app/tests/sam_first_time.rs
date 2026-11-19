//! The question before SAM's weights are downloaded: a run of a `segment.sam` with no encoder and decoder, with the
//! weights not in the cache, is held back by a window that says the size, the licence, the source and the folder.
//! *Cancel* leaves the model as it was; the choice of the full weights is written to the node, one step of undo. The
//! cache is an empty folder of the test's own, and nothing is downloaded (the window's *Download and run* is not used).

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_graph::{Model, Node};
use terraflux_ops::Registry;

fn model() -> Model {
    let mut m = Model::new("sam first time");
    m.nodes.push(Node::new("land", "nlm.fbm").param("rows", 30).param("columns", 40).param("seed", 5));
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("img", "nlm.image").input("classes", "cls.classes"));
    m.nodes.push(Node::new("sam", "segment.sam").input("image", "img.image"));
    m
}

/// The cache of the test: empty, so that no weights are in it (set once for the program).
fn empty_cache() -> PathBuf {
    static ONCE: std::sync::Once = std::sync::Once::new();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("sam-first-time-cache");
    ONCE.call_once(|| {
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        unsafe { std::env::set_var("TERRAFLUX_CACHE", &dir) };
    });
    dir
}

fn harness(name: &str) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    empty_cache();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("model.toml");
    model().save(&p).unwrap();
    let mut app = TerrafluxApp::new(Some(p));
    app.session.as_mut().unwrap().wait();
    Harness::builder().with_size((1400.0, 900.0)).build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app)
}

/// A run of the model to the node asked for (Ctrl+Enter with the node selected).
fn ask_run(h: &mut Harness<TerrafluxApp>) {
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), "sam");
    }
    h.state_mut().focus(terraflux_app::app::Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    h.run_steps(4);
}

fn weights(h: &Harness<TerrafluxApp>) -> Option<String> {
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.id == "sam").unwrap();
    n.params.get("weights").and_then(|v| v.as_str()).map(String::from)
}

#[test]
fn a_run_of_sam_without_its_weights_asks_first_and_cancel_changes_nothing() {
    let mut h = harness("sam-first-time-cancel");
    let before = h.state().session.as_ref().unwrap().source.clone();
    assert!(!h.state().session.as_ref().unwrap().sam_ok(None));
    ask_run(&mut h);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.sam_ask.is_some(), "the window asks");
    assert!(!s.running(), "the run did not start");
    assert!(h.query_by_label("Apache-2.0").is_some());
    assert!(h.query_by_label_contains("huggingface.co").is_some());
    assert!(h.query_all_by_label_contains("75 MB").count() >= 1);
    assert!(h.query_by_label_contains("sam-first-time-cache").is_some());
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.sam_ask.is_none() && !s.running());
    assert!(s.source == before, "the model is as it was");
    assert!(!s.dirty);
    // nothing was agreed to
    assert!(terraflux_app::settings::get().sam_agreed.is_empty());
}

#[test]
fn the_full_weights_are_chosen_in_the_window_and_undone_in_one_step() {
    let mut h = harness("sam-first-time-full");
    assert_eq!(weights(&h), None);
    ask_run(&mut h);
    assert!(h.state().session.as_ref().unwrap().sam_ask.is_some());
    h.get_by_label_contains("Full precision").click();
    h.run_steps(3);
    assert_eq!(weights(&h).as_deref(), Some("full"));
    let s = h.state().session.as_ref().unwrap();
    assert!(s.sam_ask.is_some() && !s.running(), "still asking, now about the full weights");
    assert!(h.query_all_by_label_contains("348 MB").count() >= 2, "the choice and the size");
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    // one step of undo takes the choice back
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), false);
    assert_eq!(weights(&h), None);
}
