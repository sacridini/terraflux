//! Objects in the Viewer: the output of a node of objects shown with its summary and table, and kept as a layer over
//! another map (its fill through the tiles, its outlines over them), in each way of filling.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_graph::{Model, Node};

fn model() -> Model {
    let mut m = Model::new("objects");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 40).param("columns", 60).param("seed", 3));
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("seg", "segment.from_map").input("map", "cls.classes"));
    m
}

fn harness() -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("objects-view.toml");
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

/// Whether a label with the text is on the screen (the summary is in the side and over the table).
fn has(h: &Harness<TerrafluxApp>, text: &str) -> bool {
    h.query_all_by_label_contains(text).next().is_some()
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
fn objects_are_shown_with_their_summary_and_kept_as_a_layer() {
    let mut h = harness();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let r = terraflux_ops::Registry::standard();
        terraflux_app::builder::select(s, &r, "seg");
    }
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    until(&mut h, "the run", |h| {
        h.state().session.as_ref().unwrap().outputs.get("seg").is_some_and(|o| o.contains_key("objects"))
    });
    // the summary in the Viewer's side, the attribute table under the map
    until(&mut h, "the summary", |h| has(h, "objects on a 40x60 grid"));
    assert!(has(&h, "Fill"), "its fill and outline controls");
    h.run_steps(3);
    // kept as a layer, in each way of filling, over another map: no panic, the layer's row told
    for fill in [None, Some("random"), Some("none")] {
        {
            let s = h.state_mut().session.as_mut().unwrap();
            let r = terraflux_ops::Registry::standard();
            terraflux_app::builder::select(s, &r, "cls");
            let mut l = terraflux_app::builder::Layer::new("seg", "objects");
            l.style.by = fill.map(String::from);
            l.style.width = 2.5;
            s.preview_layers = vec![l];
        }
        h.run_steps(2);
        // (the colours of the map shown are worked out in a thread: drawn when they are)
        until(&mut h, "the outlines", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() == 1);
        assert!(has(&h, "objects"));
        // (the fill a map over the one shown: only when each object has a colour, these have no classes)
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.preview_view.layers_drawn().0, usize::from(fill == Some("random")), "{fill:?}");
    }
    // the objects shown as a layer of their own (its properties in the Layers)
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let r = terraflux_ops::Registry::standard();
        terraflux_app::builder::select(s, &r, "seg");
        s.preview_layers = vec![terraflux_app::builder::Layer::new("seg", "objects")];
    }
    h.run_steps(2);
    until(&mut h, "the outlines", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() == 1);
    assert!(has(&h, "objects on a 40x60 grid"));
    // (the map shown is the objects' fill itself: its outlines are drawn, no map over it)
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.preview_view.layers_drawn().0, 0);
}
