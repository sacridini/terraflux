//! The scores of a `segment.optimize` in the Viewer: the chart, its best value and *Apply*, which writes the value in a
//! new node of the segmenter connected to the same image (one step of undo).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::charts::{self, Kind};
use terraflux_graph::{Model, Node, Source};
use terraflux_ops::{Registry, Value};

/// An image of three classes in three bands (30 x 40) and the scale of its multiresolution segmentation chosen among a
/// few, with a fixed shape.
fn model() -> Model {
    let mut m = Model::new("optimize view");
    m.nodes.push(Node::new("land", "nlm.fbm").param("rows", 30).param("columns", 40).param("seed", 5));
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("img", "nlm.image").input("classes", "cls.classes"));
    m.nodes.push(
        Node::new("opt", "segment.optimize")
            .input("image", "img.image")
            .param("segmenter", "segment.multiresolution")
            .param("values", serde_json::json!([0.5, 1, 2, 4]))
            .param("params", serde_json::json!({ "shape": 0.2 })),
    );
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

/// Step until `done`, a minute at most.
fn until(h: &mut Harness<TerrafluxApp>, what: &str, done: impl Fn(&Harness<TerrafluxApp>) -> bool) {
    let t = Instant::now();
    while !done(h) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "{what}: too long");
    }
}

#[test]
fn the_best_value_of_the_scores_is_applied_to_a_new_node_in_one_step() {
    let mut h = harness("optimize-view-apply");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), "opt");
    }
    h.state_mut().focus(terraflux_app::app::Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    until(&mut h, "the run", |h| h.state().session.as_ref().unwrap().outputs.contains_key("opt"));
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let Some(Value::Number(best)) = s.value_of("opt", "value") else { panic!("the best value") };
    let Some(Value::Table(scores)) = s.value_of("opt", "scores") else { panic!("the scores") };
    assert!(charts::is_scores(&scores) && Kind::of(&scores).contains(&Kind::Scores));
    assert_eq!(charts::best_value(&scores), Some(best));
    let before = s.source.clone();
    // the scores shown: their chart, the best value, and Apply
    h.state_mut().session.as_mut().unwrap().preview = Some(("opt".into(), "scores".into()));
    h.run_steps(4);
    until(&mut h, "the best value", |h| h.query_by_label_contains("best scale = ").is_some());
    let id = egui::Id::new(("table", "viewer", "opt", "scores"));
    let state: terraflux_app::table_view::TableState = h.ctx.data(|d| d.get_temp(id)).expect("the table's state");
    assert_eq!(state.chart.kind, Kind::Scores);
    h.get_by_label("Apply").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.op == "segment.multiresolution").expect("the new node");
    assert_eq!(n.inputs.get("image"), Some(&Source::Ref("img.image".into())));
    assert_eq!(n.params["scale"].as_f64(), Some(best));
    assert_eq!(n.params["shape"].as_f64(), Some(0.2));
    assert!(s.history.can_undo());
    // one undo takes the node away
    let s = h.state_mut().session.as_mut().unwrap();
    terraflux_app::builder::undo_redo(s, false);
    assert_eq!(s.source, before);
}
