//! *Find similar* in the objects inspector: the objects like the selected one lit on the map above a threshold (the
//! function of objects.similar), and *Label all as <class>* writing them, and the selected one, as samples in one step
//! of undo.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_graph::{Model, Node};
use terraflux_ops::objects::Objects;
use terraflux_ops::{Registry, Value};

/// A drawn image of three classes, cut into objects, with their attributes.
fn model() -> Model {
    let mut m = Model::new("objects similar");
    m.nodes.push(
        Node::new("land", "nlm.mpd")
            .param("rows", 40)
            .param("columns", 60)
            .param("seed", 3)
            .param("nlmr_compat", false),
    );
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("img", "nlm.image").input("classes", "cls.classes"));
    m.nodes.push(Node::new("seg", "segment.slic").input("image", "img.image").param("size", 6));
    m.nodes.push(Node::new("feat", "objects.features").input("objects", "seg.objects").input("image", "img.image"));
    m
}

fn harness(name: &str) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("model.toml");
    model().save(&p).unwrap();
    let mut app = TerrafluxApp::new(Some(p));
    app.session.as_mut().unwrap().wait();
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    terraflux_app::canvas_gpu::install(&rs);
    Harness::builder()
        .with_size((1400.0, 900.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app)
}

fn has(h: &Harness<TerrafluxApp>, text: &str) -> bool {
    h.query_all_by_label_contains(text).next().is_some()
}

fn until(h: &mut Harness<TerrafluxApp>, what: &str, done: impl Fn(&Harness<TerrafluxApp>) -> bool) {
    let t = Instant::now();
    while !done(h) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "{what}: too long");
    }
}

fn click_cell(h: &mut Harness<TerrafluxApp>, row: usize, col: usize) {
    let at = h.state().session.as_ref().unwrap().preview_view.cell_screen(col, row).expect("the map drawn");
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(1);
    }
    h.run_steps(3);
}

#[test]
fn the_objects_like_the_selected_one_are_lit_and_labelled_in_one_step() {
    let mut h = harness("objects-similar");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), "feat");
    }
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    until(&mut h, "the run", |h| h.state().session.as_ref().unwrap().outputs.contains_key("feat"));
    h.run_steps(3);
    until(&mut h, "the objects drawn", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() == 1);
    until(&mut h, "the inspector", |h| has(h, "Samples"));
    let o: std::sync::Arc<Objects> = match h.state().session.as_ref().unwrap().value_of("feat", "objects") {
        Some(Value::Objects(o)) => o,
        other => panic!("{other:?}"),
    };
    // an object of several cells, selected by a click
    let id = (1..=o.count as u32).find(|&i| o.cells[i as usize - 1] >= 12).expect("an object");
    let (row, col) = terraflux_app::objects_inspect::sample_cell(&o, id).unwrap();
    click_cell(&mut h, row, col);
    assert_eq!(h.state().session.as_ref().unwrap().preview_objects.selected.iter().copied().collect::<Vec<_>>(), [id]);
    until(&mut h, "Find similar", |h| has(h, "Find similar"));
    {
        let st = &mut h.state_mut().session.as_mut().unwrap().preview_objects;
        st.most = 100_000;
        st.classes = vec!["grass".into()];
        st.finding.threshold = 0.5;
    }
    h.get_by_label("Find similar").click();
    h.run_steps(3);
    // the lit objects are those objects.similar finds above the threshold from the same object
    let st = &h.state().session.as_ref().unwrap().preview_objects;
    let lit = st.lit();
    let found =
        terraflux_ops::objects::similar::similarity(&o, &[id], None, terraflux_ops::objects::similar::Metric::Cosine)
            .unwrap();
    assert_eq!(lit, terraflux_ops::objects::similar::above(&found.similarity, &[id], 0.5));
    assert!(!lit.is_empty() && lit.len() < o.count - 1, "{} of {} objects lit", lit.len(), o.count);
    // a higher threshold lights fewer
    h.state_mut().session.as_mut().unwrap().preview_objects.finding.threshold = 0.95;
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().preview_objects.lit().len() <= lit.len());
    h.state_mut().session.as_mut().unwrap().preview_objects.finding.threshold = 0.5;
    h.run_steps(2);
    // Label all: the lit objects and the selected one are samples written into the model, in one step of undo
    let before = h.state().session.as_ref().unwrap().source.clone();
    // (the button is far down the panel, past what a click reaches in a short window: the function it calls)
    assert!(has(&h, "Label all as grass"));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut st = std::mem::take(&mut s.preview_objects);
        st.label_all(s, &o, "grass", &lit);
        s.preview_objects = st;
    }
    h.run_steps(5);
    let s = h.state().session.as_ref().unwrap();
    let table = s.source.nodes.iter().find(|n| n.op == "table.of").expect("the samples' table");
    let rows = table.params["rows"].as_array().unwrap();
    assert_eq!(rows.len(), lit.len() + 1, "a sample for each lit object and the selected one");
    assert!(rows.iter().all(|r| r[2] == "grass"));
    assert!(s.source.nodes.iter().any(|n| n.op == "objects.samples"));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert!(s.history.undo(&mut s.source));
        assert_eq!(s.source, before, "one undo takes the samples away");
    }
}
