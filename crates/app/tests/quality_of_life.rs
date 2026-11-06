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
