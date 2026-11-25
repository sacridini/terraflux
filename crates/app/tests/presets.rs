//! The palette's shortcuts: "within" typed into the quick add finds the preset first, enter adds
//! `vector.select_by_location` with its predicate filled, and the node's title on the canvas says it. The quick add,
//! the title and the library pane's search ("dentro") are written to `target/tmp/ui-presets-*.png` to be looked at.

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_app::builder::Quick;
use terraflux_graph::{Model, Node};

fn model() -> Model {
    let mut m = Model::new("presets");
    let mut a = Node::new("parcels", "vector.read").param("path", "parcels.gpkg");
    a.pos = Some([0.0, 0.0]);
    let mut b = Node::new("park", "vector.read").param("path", "park.gpkg");
    b.pos = Some([0.0, 160.0]);
    m.nodes.push(a);
    m.nodes.push(b);
    m
}

fn harness(dir: &std::path::Path) -> Harness<'static, TerrafluxApp> {
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("presets.toml");
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

fn shot(h: &mut Harness<TerrafluxApp>, name: &str) {
    match h.render() {
        Ok(img) => img.save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("ui-presets-{name}.png"))).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
    }
}

#[test]
fn a_preset_typed_in_the_quick_add_adds_the_operator_with_its_setting() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("presets");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = harness(&dir);
    h.state_mut().focus(Pane::Model);
    h.run_steps(3);
    // the quick add ("/"), "within" typed
    h.state_mut().session.as_mut().unwrap().editor.quick = Some(Quick::default());
    h.run_steps(2);
    h.event(egui::Event::Text("within".into()));
    h.run_steps(3);
    // (the rows in order: the preset first, then the operators that match)
    let first = h.query_all_by_label_contains("Within").next().expect("the preset is listed");
    assert!(first.accesskit_node().label().is_some_and(|l| l.ends_with("Within  · select by location")));
    shot(&mut h, "quick-within");
    // the first row is the preset: enter adds it
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none(), "enter closes the search");
    let n = s.source.nodes.iter().find(|n| n.op == "vector.select_by_location").expect("the node");
    assert_eq!(n.params.get("predicate").and_then(|v| v.as_str()), Some("within"));
    assert_eq!(terraflux_app::editor_view::title(n), "Select by location · within");
    let id = n.id.clone();
    h.state_mut().session.as_mut().unwrap().selected = Some(id);
    h.state_mut().focus(Pane::Model);
    h.run_steps(5);
    shot(&mut h, "node-title");
    // the library pane's search finds it by a Portuguese word
    h.state_mut().session.as_mut().unwrap().editor.search = "dentro".into();
    h.state_mut().focus(Pane::Operators);
    h.run_steps(4);
    shot(&mut h, "library-dentro");
}
