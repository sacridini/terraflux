//! The application end to end, rendered off screen: a model opened, run with its maps arriving step by step,
//! its tabs drawn; and the wizard. The frames are written to `target/tmp/ui-*.png` to be looked at.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Tab;
use terraflux_app::wizard::{Picked, Wizard};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// A Dinamica model of Sibuyan written by the wizard.
fn model() -> PathBuf {
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    w.save(&tmp("ui-model.toml")).unwrap()
}

fn shot(h: &mut Harness<TerrafluxApp>, name: &str) {
    match h.render() {
        Ok(img) => img.save(tmp(&format!("ui-{name}.png"))).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
    }
}

fn harness(app: TerrafluxApp) -> Harness<'static, TerrafluxApp> {
    Harness::builder().with_size((1280.0, 820.0)).wgpu().build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app)
}

#[test]
fn a_model_runs_and_every_tab_draws() {
    let mut h = harness(TerrafluxApp::new(Some(model())));
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().can_run(), "{:?}", h.state().message);
    assert_eq!(h.state().session.as_ref().unwrap().maps.len(), 1, "the start map is shown");
    shot(&mut h, "1-open");
    h.get_by_label("Run").click();
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().running() || h.state().session.as_ref().unwrap().maps.len() < 2 {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
        assert!(t.elapsed() < Duration::from_secs(120), "the run did not end");
    }
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.error.is_none(), "{:?}", s.error);
    assert_eq!(s.maps.len(), 15, "the start and 14 steps");
    assert_eq!(s.tables.len(), 14);
    assert_eq!(s.shown, 14, "the view follows the run");
    shot(&mut h, "2-map");
    for (tab, name) in [(Tab::Charts, "3-charts"), (Tab::Graph, "4-graph"), (Tab::Model, "5-model")] {
        h.state_mut().tab = tab;
        h.run_steps(3);
        shot(&mut h, name);
    }
    // the graph: a node's note when it is selected
    h.state_mut().tab = Tab::Graph;
    h.state_mut().session.as_mut().unwrap().selected = Some("markov".into());
    h.run_steps(2);
    shot(&mut h, "4b-graph-node");
    assert!(h.query_by_label("Why").is_some(), "the selected node's note");
    // validation against the observed map of 2011
    h.state_mut().tab = Tab::Validation;
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.validation.observed = Some(data("sibuyan_landuse.tif"));
        s.validation.bands = vec!["1997".into(), "2011".into()];
        s.validation.band = 1;
        s.validate();
        assert!(s.validation.error.is_none(), "{:?}", s.validation.error);
        let a = s.validation.agreement.as_ref().unwrap();
        assert!(a.figure_of_merit > 0.0 && a.figure_of_merit < 1.0);
        assert_eq!(s.validation.similarity.as_ref().unwrap().rows.len(), 6);
    }
    h.run_steps(3);
    assert!(h.query_by_label("figure of merit").is_some());
    shot(&mut h, "6-validation");
}

#[test]
fn the_wizard_checks_the_data() {
    let mut h = harness(TerrafluxApp::new(None));
    h.run_steps(2);
    h.get_all_by_label("New model...").next().unwrap().click();
    h.run_steps(2);
    assert!(h.state().wizard.is_some());
    assert!(h.query_by_label_contains("Dinamica EGO's classic model").is_some());
    shot(&mut h, "7-wizard");
    h.get_by_label("Next").click();
    h.run_steps(2);
    {
        let w = h.state_mut().wizard.as_mut().unwrap();
        w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
        // a misaligned driver: Plum Island's
        w.drivers = Some(Picked::read(&data("pie_drivers.tif")).unwrap());
    }
    h.run_steps(2);
    h.get_by_label("Next").click();
    h.run_steps(3);
    let w = h.state().wizard.as_ref().unwrap();
    assert!(w.blocked(), "a driver on another grid must stop the wizard");
    assert!(h.query_by_label_contains("resample it to the reference grid").is_some());
    shot(&mut h, "8-wizard-check");
}

#[test]
fn the_editor_changes_the_graph() {
    let path = tmp("ui-edited.toml");
    std::fs::copy(model(), &path).unwrap();
    let mut h = harness(TerrafluxApp::new(Some(path.clone())));
    h.state_mut().tab = Tab::Graph;
    h.run_steps(3);
    shot(&mut h, "9-editor");
    // a node of map algebra, added from the bar and connected in the side panel's terms
    h.state_mut().session.as_mut().unwrap().editor.add = "map.calc".into();
    h.run_steps(2);
    h.get_by_label("Add node").click();
    h.run_steps(3);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert_eq!(s.selected.as_deref(), Some("calc"));
        let r = terraflux_ops::Registry::standard();
        terraflux_app::editor::connect(&mut s.source, &r, &[], "$t0", "calc", "land").unwrap();
        terraflux_app::editor::set_param(&mut s.source, &[], "calc", "expression", "land == 1").unwrap();
        s.edited();
        assert!(s.model.node("calc").is_some());
        assert!(!s.issues.iter().any(|i| i.at.starts_with("calc")), "{:?}", s.issues);
    }
    h.run_steps(3);
    assert!(h.query_by_label("Delete node").is_some(), "the selected node's panel");
    shot(&mut h, "9b-editor-node");
    // into the loop's body and back
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["simulation".into()];
    h.run_steps(3);
    assert!(h.query_by_label("simulation").is_some(), "the breadcrumb shows the container");
    {
        let s = h.state().session.as_ref().unwrap();
        let r = terraflux_ops::Registry::standard();
        let src = terraflux_app::editor::sources(&s.source, &r, &s.editor.path);
        assert!(src.iter().any(|(x, _)| x == "@landscape"), "the loop's carried value is a source: {src:?}");
    }
    shot(&mut h, "9c-editor-body");
    h.get_by_label("Top level").click();
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.path.is_empty());
    h.get_by_label("Save").click();
    h.run_steps(2);
    let saved = terraflux_graph::Model::load(&path).unwrap();
    assert_eq!(saved.node("calc").unwrap().params["expression"], "land == 1");
    assert!(saved.node("calc").unwrap().pos.is_some());
    assert!(!h.state().session.as_ref().unwrap().dirty);
}
