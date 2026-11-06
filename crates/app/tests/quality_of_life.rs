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
    // a new model with nothing done to it: no question
    h.state_mut().ask_new();
    assert!(h.state().confirm.is_none());
    add_node(&mut h, "land");
    h.run_steps(2);
    // a new model asked for: the question, the model kept until it is answered
    h.state_mut().ask_new();
    h.run_steps(3);
    assert_eq!(h.state().confirm, Some(Then::New));
    assert!(h.query_by_label_contains("Save the changes to").is_some());
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    assert!(h.state().confirm.is_none());
    assert!(has_node(&h, "land"), "Cancel keeps the model");
    // Don't save: the new model
    h.state_mut().ask_new();
    h.run_steps(3);
    h.get_by_label("Don't save").click();
    h.run_steps(3);
    assert!(h.state().confirm.is_none());
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
    let path = tmp("eta.toml");
    let _ = std::fs::remove_file(terraflux_app::settings::data_dir().join("run-times.json"));
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
    let h = harness(new_app(Some(path)));
    assert!(h.state().session.as_ref().unwrap().times_before.contains_key("years"));
}
