//! The application's quality of life (a part of the `ui` tests): the edits not saved asked about before they are
//! put aside, copied now and then, recovered after a crash.

use super::*;
use terraflux_app::app::Then;
use terraflux_app::recovery::{Keeper, Note};

/// A node added to the model open, as an edit.
fn add_node(h: &mut Harness<TerrafluxApp>, id: &str) {
    let s = h.state_mut().session.as_mut().unwrap();
    s.source.nodes.push(terraflux_graph::Node::new(id, "nlm.random"));
    s.edited();
}

fn has_node(h: &Harness<TerrafluxApp>, id: &str) -> bool {
    h.state().session.as_ref().unwrap().source.nodes.iter().any(|n| n.id == id)
}

#[test]
fn edits_not_saved_are_asked_about_before_they_are_put_aside() {
    let mut h = harness(new_app(None));
    h.run_steps(3);
    // a new model with nothing done to it: replaced, no question, one tab
    h.state_mut().ask_new();
    assert!(h.state().confirm.is_none());
    assert_eq!(h.state().tab_count(), 1);
    add_node(&mut h, "land");
    h.run_steps(2);
    // a new model asked for: in a tab of its own, the edited one kept in its tab
    h.state_mut().ask_new();
    h.run_steps(3);
    assert!(h.state().confirm.is_none());
    assert_eq!((h.state().tab_count(), h.state().front), (2, 1));
    assert!(h.state().tab(0).unwrap().source.nodes.iter().any(|n| n.id == "land"));
    // its tab closed: the question, the model kept until it is answered
    h.state_mut().close_tab(0);
    h.run_steps(3);
    assert_eq!(h.state().confirm, Some(Then::Close));
    assert!(h.query_by_label_contains("Save the changes to").is_some());
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    assert!(h.state().confirm.is_none());
    assert_eq!(h.state().tab_count(), 2);
    assert!(has_node(&h, "land"), "Cancel keeps the model, in front");
    // Don't save: closed, the other in front
    h.state_mut().close_tab(0);
    h.run_steps(3);
    h.get_by_label("Don't save").click();
    h.run_steps(3);
    assert!(h.state().confirm.is_none());
    assert_eq!(h.state().tab_count(), 1);
    assert!(!has_node(&h, "land"), "the edits thrown away");
    assert!(!h.state().session.as_ref().unwrap().dirty);
}

#[test]
fn a_template_only_started_is_not_asked_about() {
    let mut h = harness(new_app(None));
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.dirty && !s.worth_keeping(), "a template is not saved, but nothing was done to it");
    h.state_mut().ask_new();
    assert!(h.state().confirm.is_none());
    assert!(h.state().session.as_ref().unwrap().source.nodes.is_empty(), "the new model at once");
    // the template's tab closed: no question either
    h.state_mut().close_tab(0);
    h.run_steps(2);
    assert!(h.state().confirm.is_none());
    assert_eq!(h.state().tab_count(), 1);
}

/// Two models open in tabs: switched by the strip and ctrl+Tab, each with its own panes' states, a model open
/// already gone to its tab (by the MCP's open_in_app too), one behind running to its end, closed by its ×.
#[test]
fn models_open_in_tabs_run_behind_and_close() {
    let dir = tmp("tabs");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = |name: &str, node: &str| {
        let mut m = terraflux_graph::Model::new(name);
        m.nodes.push(terraflux_graph::Node::new(node, "nlm.random").param("rows", 40).param("cols", 40));
        let p = dir.join(format!("{name}.toml"));
        m.save(&p).unwrap();
        p
    };
    let (first, second) = (file("first", "a"), file("second", "b"));
    let mut h = harness(new_app(Some(first.clone())));
    h.run_steps(3);
    h.state_mut().ask_open(&second);
    h.run_steps(3);
    assert_eq!((h.state().tab_count(), h.state().front), (2, 1));
    assert!(has_node(&h, "b"));
    assert!(h.query_by_label("first").is_some(), "the strip of the tabs");
    assert_eq!(h.query_all_by_label("×").count(), 2);
    if let Ok(img) = h.render() {
        img.save(tmp("ui-tabs.png")).unwrap();
    }
    // the first by its tab; the second by ctrl+Tab; the first open again: its tab, not a third
    h.get_by_label("first").click();
    h.run_steps(3);
    assert!(has_node(&h, "a") && h.state().front == 0);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Tab);
    h.run_steps(3);
    assert!(has_node(&h, "b"));
    h.state_mut().ask_open(&first);
    h.run_steps(2);
    assert_eq!(h.state().tab_count(), 2);
    assert!(has_node(&h, "a"));
    // the panes' states go with their models
    h.state_mut().params.filter = "only the first's".into();
    h.state_mut().switch_to(1);
    assert_eq!(h.state().params.filter, "");
    h.state_mut().switch_to(0);
    assert_eq!(h.state().params.filter, "only the first's");
    // the first run behind the second: it runs to its end
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.outputs.clear();
        s.start(Box::new(|| {}));
    }
    h.state_mut().switch_to(1);
    let t = Instant::now();
    while h.state().tab(0).unwrap().running() || h.state().tab(0).unwrap().outputs.is_empty() {
        h.step();
        assert!(t.elapsed() < Duration::from_secs(60), "the run behind did not end");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(h.state().tab(0).unwrap().outputs.contains_key("a"), "ran behind");
    // the MCP's open_in_app of a model open: its tab
    let (tx, rx) = std::sync::mpsc::channel();
    h.state_mut().handle_handoff(terraflux_app::handoff::Request { path: Some(first.clone()), answer: tx });
    h.run_steps(2);
    assert_eq!(rx.try_recv().unwrap(), Ok(()));
    assert_eq!((h.state().tab_count(), h.state().front), (2, 0));
    // closed by its ×: the other in front, the strip gone
    h.get_all_by_label("×").next().unwrap().click();
    h.run_steps(3);
    assert_eq!(h.state().tab_count(), 1);
    assert!(has_node(&h, "b"));
    assert_eq!(h.query_all_by_label("×").count(), 0, "no strip for one model");
}

#[test]
fn the_edits_of_a_terraflux_that_ended_are_recovered() {
    let dir = tmp("recovery");
    let _ = std::fs::remove_dir_all(&dir);
    // a model saved, then edited
    let file = tmp("recovered.toml");
    let mut m = terraflux_graph::Model::new("recovered");
    m.nodes.push(terraflux_graph::Node::new("first", "nlm.random"));
    m.save(&file).unwrap();
    let mut app = new_app(Some(file.clone()));
    app.recovery = Some(Keeper::new(dir.clone()));
    let mut h = harness(app);
    h.run_steps(2);
    add_node(&mut h, "second");
    // its copy written (at once: none written yet), with its file
    let t = Instant::now();
    let key = loop {
        h.step();
        let key = h.state().session.as_ref().unwrap().copy.as_ref().map(|(k, _)| k.clone());
        if let Some(k) = key.filter(|k| dir.join(format!("{k}.json")).is_file()) {
            break k;
        }
        assert!(t.elapsed() < Duration::from_secs(20), "no copy written");
        std::thread::sleep(Duration::from_millis(20));
    };
    let note: Note = serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{key}.json"))).unwrap()).unwrap();
    assert_eq!(note.path.as_deref(), Some(std::path::absolute(&file).unwrap().as_path()));
    assert_eq!(note.pid, std::process::id());
    // as if of another Terraflux, whose pulse stopped ten minutes ago: offered when the application opens
    let text = std::fs::read_to_string(dir.join(format!("{key}.toml"))).unwrap();
    let mut other = note.clone();
    other.pid = 1;
    terraflux_app::recovery::write(&dir, "left-1", &text, &other).unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(600);
    std::fs::File::options().write(true).open(dir.join("left-1.json")).unwrap().set_modified(old).unwrap();
    drop(h);
    let mut app = new_app(None);
    app.recovery = Some(Keeper::new(dir.clone()));
    let mut h = harness(app);
    h.run_steps(3);
    assert!(h.query_by_label_contains("Terraflux closed with edits not saved in recovered").is_some());
    h.get_by_label("Recover").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.path.as_deref(), Some(std::path::absolute(&file).unwrap().as_path()));
    assert!(s.dirty && has_node(&h, "second") && has_node(&h, "first"), "the edits back, not saved");
    assert!(!dir.join("left-1.toml").exists(), "the copy recovered is deleted");
    assert!(h.query_by_label_contains("Terraflux closed with edits not saved").is_none());
    // a step of undo: the file as it is on disk
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), false);
    h.run_steps(2);
    assert!(!has_node(&h, "second") && has_node(&h, "first"));
}

#[test]
fn the_computer_is_kept_awake_while_a_model_runs() {
    let mut app = new_app(None);
    app.awake = Some(terraflux_app::awake::Awake::default());
    let mut h = harness(app);
    let ctx = h.ctx.clone();
    // a template: run once its data is read
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    let (mut ran, mut held) = (false, false);
    let t = Instant::now();
    loop {
        h.step();
        let running = h.state().session.as_ref().unwrap().running();
        if running {
            ran = true;
            held |= h.state().awake.as_ref().unwrap().held();
        } else if ran {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(120), "the run did not end");
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(held, "held while it ran");
    h.run_steps(2);
    assert!(!h.state().awake.as_ref().unwrap().held(), "let go once it ended");
    // turned off in the settings: never held
    terraflux_app::settings::update(|st| st.keep_awake = false);
    h.state_mut().session.as_mut().unwrap().run_when_ready = true;
    let mut held = false;
    for _ in 0..200 {
        h.step();
        held |= h.state().awake.as_ref().unwrap().held();
        if !h.state().session.as_ref().unwrap().running() && !h.state().session.as_ref().unwrap().run_when_ready {
            break;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    terraflux_app::settings::update(|st| st.keep_awake = true);
    assert!(!held, "not held when turned off");
}

/// Step until the run going on (or about to start) ends.
fn run_to_the_end(h: &mut Harness<TerrafluxApp>) {
    let t = Instant::now();
    let mut ran = false;
    loop {
        h.step();
        let s = h.state().session.as_ref().unwrap();
        ran |= s.running();
        if ran && !s.running() {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(120), "the run did not end");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(2);
}

#[test]
fn the_end_of_a_run_is_told_when_terraflux_is_not_in_front() {
    terraflux_app::settings::update(|st| st.notify_after = 0.0);
    let mut h = harness(new_app(None));
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    // in front: nothing told
    run_to_the_end(&mut h);
    assert!(h.state().told.is_empty(), "{:?}", h.state().told);
    // in the background: a run asked for is told, with the model's name
    // (as the system tells it: the window lost the focus)
    h.input_mut().focused = false;
    h.run_steps(1);
    let ctx = h.ctx.clone();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.cache.lock().unwrap().clear();
        s.start(Box::new(move || ctx.request_repaint()));
    }
    run_to_the_end(&mut h);
    let title = h.state().session.as_ref().unwrap().title();
    assert_eq!(h.state().told.len(), 1, "{:?}", h.state().told);
    assert_eq!(h.state().told[0].0, format!("{title}: done"));
    assert!(h.state().told[0].1.starts_with("The run ended in "));
    // turned off: not told
    terraflux_app::settings::update(|st| st.notify_runs = false);
    let ctx = h.ctx.clone();
    h.state_mut().session.as_mut().unwrap().start(Box::new(move || ctx.request_repaint()));
    run_to_the_end(&mut h);
    terraflux_app::settings::update(|st| {
        st.notify_runs = true;
        st.notify_after = 10.0;
    });
    assert_eq!(h.state().told.len(), 1);
}

#[test]
fn the_time_left_comes_from_the_runs_before() {
    use std::sync::atomic::Ordering;
    use terraflux_graph::{Body, Model, Node};
    let mut m = Model::new("eta");
    m.nodes.push(Node::new("first", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 2));
    m.nodes.push(Node::new("years", "control.repeat").literal("times", 4).body(Body {
        nodes: vec![Node::new("land", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 1)],
        collect: [("maps".to_string(), "land.map".to_string())].into(),
        ..Default::default()
    }));
    // (a path of this run alone: the times kept are shared by the tests, and by the runs before of this one)
    let path = tmp(&format!("eta-{}.toml", std::process::id()));
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path.clone())));
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().times_before.is_empty(), "never run");
    // a whole run: the nodes' times known
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    run_to_the_end(&mut h);
    // again from nothing, paused after the loop's first step: the time left by the times of the run before
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.cache.lock().unwrap().clear();
        s.gate.stepping.store(true, Ordering::Relaxed);
        s.start(Box::new(|| {}));
    }
    let t = Instant::now();
    while !h.state().session.as_ref().unwrap().gate.paused() {
        h.step();
        assert!(t.elapsed() < Duration::from_secs(60), "never paused");
        std::thread::sleep(Duration::from_millis(5));
    }
    let left = h.state().session.as_ref().unwrap().time_left().expect("a time left");
    assert!(left.by_times && left.seconds >= 0.0, "{left:?}");
    h.state_mut().session.as_mut().unwrap().gate.resume();
    run_to_the_end(&mut h);
    assert!(h.state().session.as_ref().unwrap().time_left().is_none(), "nothing left once it ended");
    // kept for the next session of the model
    let t = Instant::now();
    while terraflux_app::eta::times_of(&path).is_empty() {
        assert!(t.elapsed() < Duration::from_secs(10), "the times not kept");
        std::thread::sleep(Duration::from_millis(20));
    }
    let times = terraflux_app::eta::times_of(&path);
    assert!(times.contains_key("first") && times.contains_key("years"), "{times:?}");
    drop(h);
    let mut h = harness(new_app(Some(path)));
    assert!(h.state().session.as_ref().unwrap().times_before.contains_key("years"));
    // and on the canvas before running (fainter): the nodes of the scope shown, by their ids
    h.run_steps(3);
    let e = &h.state().session.as_ref().unwrap().editor;
    assert!(e.times.is_empty(), "nothing run in this session");
    assert!(e.times_before.contains_key("first") && e.times_before.contains_key("years"), "{:?}", e.times_before);
    assert!(!e.times_before.contains_key("years/land"), "only the scope shown");
    if let Ok(img) = h.render() {
        img.save(tmp("ui-times-before.png")).unwrap();
    }
}

#[test]
fn the_canvas_tells_how_long_each_node_took() {
    use terraflux_graph::{Model, Node};
    let mut m = Model::new("times");
    let mut big = Node::new("big", "nlm.random").param("rows", 600).param("columns", 600).param("seed", 3);
    big.pos = Some([0.0, 0.0]);
    let mut small = Node::new("small", "nlm.random").param("rows", 5).param("columns", 5).param("seed", 3);
    small.pos = Some([0.0, 200.0]);
    m.nodes = vec![big, small];
    let path = tmp("times.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    h.run_steps(3);
    // (nothing run yet: Times does nothing)
    h.get_by_label("Times").click();
    h.run_steps(2);
    assert!(!h.state().session.as_ref().unwrap().editor.times_shown);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    run_to_the_end(&mut h);
    let times = h.state().session.as_ref().unwrap().editor.times.clone();
    assert!(times.contains_key("big") && times.contains_key("small"), "{times:?}");
    h.get_by_label("Times").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.times_shown);
    assert!(h.query_by_label("The slowest").is_some());
    if let Ok(img) = h.render() {
        img.save(tmp("ui-times.png")).unwrap();
    }
    let first = terraflux_app::editor_view::slowest_of(&times)[0].0.clone();
    h.get_by_label(&format!("1. {first}")).click();
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some(first.as_str()));
}

#[test]
fn the_history_goes_back_and_forward_to_any_step() {
    let mut h = harness(new_app(None));
    h.run_steps(3);
    for id in ["a", "b", "c"] {
        add_node(&mut h, id);
        h.run_steps(2);
    }
    h.get_by_label("History").click();
    h.run_steps(3);
    assert!(h.query_by_label("the model as it was opened").is_some());
    assert!(h.query_by_label("3. added c (nlm.random)").is_some());
    if let Ok(img) = h.render() {
        img.save(tmp("ui-history.png")).unwrap();
    }
    // back to the first step: one node
    h.get_by_label("1. added a (nlm.random)").click();
    h.run_steps(3);
    let ids = |h: &Harness<TerrafluxApp>| -> Vec<String> {
        h.state().session.as_ref().unwrap().source.nodes.iter().map(|n| n.id.clone()).collect()
    };
    assert_eq!(ids(&h), ["a"]);
    assert_eq!(h.state().session.as_ref().unwrap().history.steps().1, 1);
    // forward to the last again
    h.get_by_label("3. added c (nlm.random)").click();
    h.run_steps(3);
    assert_eq!(ids(&h), ["a", "b", "c"]);
    // compared with the model as it was opened: the Changes
    h.get_by_label("1. added a (nlm.random)").click();
    h.run_steps(2);
    h.get_by_label("3. added c (nlm.random)").click();
    h.run_steps(2);
    let first = h.get_all_by_label("compare").next().unwrap();
    first.click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(terraflux_app::review::changes(s).len(), 3, "three nodes added since it was opened");
}

#[test]
fn the_files_not_found_are_located_again() {
    // a model written on another computer (Windows paths), its data now in a folder far from it (not around it:
    // found by the folder chosen)
    let top = tmp("relink");
    let _ = std::fs::remove_dir_all(&top);
    let dir = top.join("elsewhere");
    std::fs::create_dir_all(dir.join("copied").join("inputs")).unwrap();
    std::fs::write(dir.join("copied").join("inputs").join("land.tif"), b"x").unwrap();
    std::fs::write(dir.join("copied").join("inputs").join("roads.tif"), b"x").unwrap();
    let models = top.join("a").join("b");
    std::fs::create_dir_all(&models).unwrap();
    let file = models.join("model.toml");
    let mut m = terraflux_graph::Model::new("from another computer");
    m.nodes.push(terraflux_graph::Node::new("land", "raster.read").param("path", r"D:\work\inputs\land.tif"));
    m.nodes.push(terraflux_graph::Node::new("roads", "raster.read").param("path", r"D:\work\inputs\roads.tif"));
    m.nodes.push(terraflux_graph::Node::new("out", "raster.write").param("path", r"D:\work\out.tif"));
    m.save(&file).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    h.run_steps(3);
    assert!(h.query_by_label("2 files of the model are not found.").is_some(), "the file written is not asked for");
    h.get_all_by_label("Locate them...").next().unwrap().click();
    h.run_steps(3);
    assert!(h.query_by_label("Files not found").is_some());
    // the folder chosen (the dialog of the system stands for it here)
    let s = h.state_mut().session.as_mut().unwrap();
    let olds: Vec<String> = s.relink.missing.iter().map(|(p, _)| p.clone()).collect();
    let found = terraflux_app::relink::locate(&olds, &dir);
    assert_eq!(found.len(), 2);
    s.relink.dialog.found = found;
    h.run_steps(2);
    if let Ok(img) = h.render() {
        img.save(tmp("ui-relink.png")).unwrap();
    }
    h.get_by_label("Apply (2 found)").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes[0].params["path"], serde_json::json!("../../elsewhere/copied/inputs/land.tif"));
    assert_eq!(s.source.nodes[1].params["path"], serde_json::json!("../../elsewhere/copied/inputs/roads.tif"));
    assert_eq!(s.source.nodes[2].params["path"], serde_json::json!(r"D:\work\out.tif"), "the file written left");
    assert!(s.dirty);
    assert!(h.query_by_label_contains("not found.").is_none(), "the strip gone");
    // one step of undo, named
    assert_eq!(s.history.steps().0.last().map(String::as_str), Some("2 files located again"));
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), false);
    h.run_steps(3);
    assert!(h.query_by_label("2 files of the model are not found.").is_some());
}

/// The models open when Terraflux closed opened again, each with its view: the camera, the node selected, what the
/// Viewer showed (run the quiet way once the data is read), the pane in front.
#[test]
fn the_last_session_comes_back_with_each_models_view() {
    let dir = tmp("restore");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let store = terraflux_app::restore::Store::new(dir.join("data"));
    let file = |name: &str| {
        let mut m = terraflux_graph::Model::new(name);
        for id in ["a", "b"] {
            m.nodes.push(terraflux_graph::Node::new(id, "nlm.random").param("rows", 30).param("cols", 30));
        }
        let p = dir.join(format!("{name}.toml"));
        m.save(&p).unwrap();
        p
    };
    let (one, two) = (file("one"), file("two"));
    let mut app = new_app(Some(one.clone()));
    app.restore = Some(store.clone());
    let mut h = harness(app);
    h.run_steps(3);
    h.state_mut().ask_open(&two);
    h.run_steps(3);
    // in the first: b selected, the camera moved, a's map in the Viewer; the first in front
    h.state_mut().switch_to(0);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.selected = Some("b".into());
        s.editor.set_camera(egui::vec2(123.0, -45.0), 1.75);
        s.preview = Some(("a".into(), "map".into()));
    }
    h.run_steps(3);
    h.state_mut().remember();
    drop(h);
    // Terraflux again with a model given, or with the setting off: only that model, or a new one
    let mut app = new_app(Some(two.clone()));
    app.start_restoring(store.clone(), true, true);
    assert_eq!(app.tab_count(), 1, "a model given: it alone");
    let mut app = new_app(None);
    app.start_restoring(store.clone(), false, false);
    assert_eq!(app.tab_count(), 1, "turned off: a new model");
    assert!(app.session.as_ref().unwrap().path.is_none());
    // and without either: both open, the first in front with its view
    let mut app = new_app(None);
    app.start_restoring(store.clone(), false, true);
    let mut h = harness(app);
    h.run_steps(2);
    assert_eq!((h.state().tab_count(), h.state().front), (2, 0));
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.path.as_deref().map(|p| p.file_name().unwrap().to_owned()), Some("one.toml".into()));
    assert_eq!(s.selected.as_deref(), Some("b"));
    let c = s.editor.camera().unwrap();
    assert_eq!(([c.pan.x, c.pan.y], c.zoom), ([123.0, -45.0], 1.75));
    assert_eq!(s.preview, Some(("a".to_string(), "map".to_string())));
    // what the Viewer showed: run the quiet way, not the whole model
    let t = Instant::now();
    while !h.state().session.as_ref().unwrap().outputs.contains_key("a") {
        h.step();
        assert!(t.elapsed() < Duration::from_secs(60), "what the Viewer showed did not run");
        std::thread::sleep(Duration::from_millis(5));
    }
    run_to_the_end_quietly(&mut h);
    assert!(!h.state().session.as_ref().unwrap().outputs.contains_key("b"), "the whole model not run");
    // a model opened with no view kept: as before
    assert!(store.view_of(&dir.join("never.toml")).is_none());
}

/// Step until no run goes on.
fn run_to_the_end_quietly(h: &mut Harness<TerrafluxApp>) {
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().running() {
        h.step();
        assert!(t.elapsed() < Duration::from_secs(60), "the run did not end");
        std::thread::sleep(Duration::from_millis(5));
    }
    h.run_steps(2);
}

/// A model's project copied whole to another computer: the files it reads looked for around it at once (its folder,
/// the one above), those found offered in the strip, Apply.
#[test]
fn the_files_not_found_are_looked_for_around_the_model() {
    let top = tmp("relink-around");
    let _ = std::fs::remove_dir_all(&top);
    std::fs::create_dir_all(top.join("data").join("2020")).unwrap();
    std::fs::write(top.join("data").join("2020").join("cover.tif"), b"x").unwrap();
    std::fs::create_dir_all(top.join("models")).unwrap();
    let file = top.join("models").join("model.toml");
    let mut m = terraflux_graph::Model::new("copied");
    m.nodes.push(terraflux_graph::Node::new("cover", "raster.read").param("path", r"E:\study\data\2020\cover.tif"));
    m.nodes.push(terraflux_graph::Node::new("gone", "raster.read").param("path", r"E:\study\data\gone.tif"));
    m.save(&file).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    let t = Instant::now();
    while h.query_by_label("1 found beside the model.").is_none() {
        h.step();
        assert!(t.elapsed() < Duration::from_secs(30), "not looked for around the model");
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(h.query_by_label("2 files of the model are not found.").is_some());
    h.get_by_label("Apply (1)").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes[0].params["path"], serde_json::json!("../data/2020/cover.tif"));
    assert_eq!(s.source.nodes[1].params["path"], serde_json::json!(r"E:\study\data\gone.tif"));
    assert!(h.query_by_label("1 file of the model is not found.").is_some(), "the other still told");
    assert!(h.query_by_label_contains("found beside the model").is_none(), "not looked for twice");
}

/// The frame selected copied (with its nodes) and pasted: a new frame around the copies; a note selected
/// duplicated (ctrl+D): a new note beside it.
#[test]
fn notes_and_frames_are_copied_with_their_nodes() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let r = terraflux_ops::Registry::standard();
        terraflux_app::builder::add(s, &r, "nlm.fbm", &[]).unwrap();
        terraflux_app::builder::add(s, &r, "landscape.mspa", &[]).unwrap();
        let note = |id: &str, nodes: Vec<String>, at| terraflux_graph::Note {
            id: id.into(),
            text: id.into(),
            nodes,
            at,
            ..Default::default()
        };
        s.source.notes.push(note("frame1", vec!["nlm_fbm".into(), "landscape_mspa".into()], None));
        s.source.notes.push(note("note1", Vec::new(), Some([0.0, -200.0])));
        s.editor.chosen.clear();
        s.selected = None;
        s.editor.note_chosen = Some("frame1".into());
    }
    h.state_mut().focus(Pane::Model);
    h.run_steps(3);
    h.event(egui::Event::Copy);
    h.step();
    let text = h
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .expect("the frame copied");
    h.event(egui::Event::Paste(text));
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.source.node("nlm_fbm_2").is_some() && s.source.node("landscape_mspa_2").is_some());
        let f = s.source.notes.iter().find(|n| n.id == "frame2").expect("the frame pasted");
        assert_eq!(f.nodes, ["nlm_fbm_2", "landscape_mspa_2"]);
        assert_eq!(f.text, "frame1");
        let m = s.source.node("landscape_mspa_2").unwrap();
        assert_eq!(m.inputs["map"], terraflux_graph::Source::Ref("nlm_fbm_2.map".into()));
    }
    // the note selected, ctrl+D
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.editor.chosen.clear();
        s.selected = None;
        s.editor.note_chosen = Some("note1".into());
    }
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.notes.iter().find(|n| n.id == "note2").expect("the note duplicated");
    assert_eq!((n.text.as_str(), n.at), ("note1", Some([40.0, -160.0])));
    assert_eq!(s.editor.note_chosen.as_deref(), Some("note2"));
    assert_eq!(s.source.nodes.len(), 4);
}

/// The model's file changed by an agent through the MCP, which told its tool: the step that takes it again named by
/// the tool, not by the difference.
#[test]
fn the_steps_of_an_agent_are_named_by_its_tools() {
    use terraflux_graph::{Model, Node};
    let path = tmp(&format!("agent-{}.toml", std::process::id()));
    let mut m = Model::new("agent");
    m.nodes.push(Node::new("a", "nlm.random"));
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path.clone())));
    h.run_steps(3);
    // what terraflux mcp does: the tool told, then the file written
    m.nodes.push(Node::new("b", "nlm.random"));
    terraflux_app::agent_edits::tell(&path, "add_node");
    m.save(&path).unwrap();
    let t = Instant::now();
    while !has_node(&h, "b") {
        h.step();
        assert!(t.elapsed() < Duration::from_secs(20), "the file not read again");
        std::thread::sleep(Duration::from_millis(20));
    }
    h.run_steps(3);
    let (names, at) = h.state().session.as_ref().unwrap().history.steps();
    assert_eq!(names.last().map(String::as_str), Some("the MCP: add_node"), "{names:?}");
    assert_eq!(at, names.len());
    assert!(terraflux_app::agent_edits::take(&path).is_empty(), "taken");
}
