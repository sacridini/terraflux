//! Comparing segmenters in the Viewer: *Compare...* in the live segmentation's panel opens a window with a thumbnail for
//! each segmenter, and *Use* puts one in place of the segmenter shown.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_app::segment_compare;
use terraflux_graph::{Model, Node, Source};
use terraflux_ops::Registry;

fn model() -> Model {
    let mut m = Model::new("segment compare");
    m.nodes.push(
        Node::new("land", "nlm.mpd")
            .param("rows", 60)
            .param("columns", 80)
            .param("seed", 3)
            .param("nlmr_compat", false),
    );
    m.nodes.push(Node::new("fine", "segment.multiresolution").input("image", "land.map").param("scale", 0.02));
    // two readers of the objects shown
    for id in ["rules_a", "rules_b"] {
        m.nodes.push(Node::new(id, "objects.rules").input("objects", "fine.objects").param(
            "classes",
            serde_json::json!([
                { "name": "big", "rule": "above(cells, 4, 8)" },
                { "name": "small", "rule": "below(cells, 4, 8)" },
            ]),
        ));
    }
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
fn four_thumbnails_and_use_puts_the_slic_in_place_of_the_shown_segmenter() {
    let mut h = harness("segment-compare");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), "fine");
    }
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    until(&mut h, "the run", |h| {
        let s = h.state().session.as_ref().unwrap();
        s.outputs.get("fine").is_some_and(|o| o.contains_key("objects"))
    });
    h.state_mut().session.as_mut().unwrap().preview = Some(("fine".into(), "objects".into()));
    h.run_steps(3);
    until(&mut h, "the panel", |h| has(h, "LIVE SEGMENTATION"));
    // Compare... opens the window: the four segmenters, one after another in a thread
    let st = segment_compare::state(&h.ctx, "viewer", "fine");
    assert!(!st.lock().unwrap().open);
    h.get_by_label("Compare...").click();
    h.run_steps(3);
    assert!(st.lock().unwrap().open);
    until(&mut h, "the thumbnails", |_| st.lock().unwrap().ready() >= 4);
    {
        let c = st.lock().unwrap();
        assert!(c.error.is_none(), "{:?}", c.error);
        assert!(c.started >= 1);
        for (op, _) in segment_compare::OPS.iter().take(4) {
            assert!(c.count(op).is_some_and(|n| n > 0), "{op}");
        }
        assert!(c.count("segment.quickshift").is_none(), "the ones not chosen are not run");
    }
    until(&mut h, "its text", |h| has(h, " objects, "));
    // Use on SLIC (the thumbnails are in order and the shown multiresolution has no button: SLIC has the first)
    let before_readers = ["rules_a", "rules_b"];
    for _ in 0..6 {
        let made = h.state().session.as_ref().unwrap().source.nodes.iter().any(|n| n.op == "segment.slic");
        if made {
            break;
        }
        h.get_all_by_label("Use").next().expect("a Use button").click();
        h.run_steps(4);
    }
    let s = h.state().session.as_ref().unwrap();
    let slic = s.source.nodes.iter().find(|n| n.op == "segment.slic").expect("a segment.slic");
    assert_eq!(slic.inputs.get("image"), Some(&Source::Ref("land.map".into())));
    for id in before_readers {
        let n = s.source.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(n.inputs.get("objects"), Some(&Source::Ref(format!("{}.objects", slic.id))), "{id}");
    }
    assert!(s.history.can_undo());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::undo_redo(s, false);
    }
    let s = h.state().session.as_ref().unwrap();
    assert!(!s.source.nodes.iter().any(|n| n.op == "segment.slic"), "one undo takes it away");
}
