//! Live segmentation, compared objects and the hypotheses of the classes in the Viewer: a slider of a
//! `segment.multiresolution` previews the part in view and *Apply* writes the node; the difference of two classified
//! objects counts the cells whose class differs; under the pointer an `objects.resolve` tells each class's membership.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_app::builder::{CompareWith, How};
use terraflux_app::live_segment;
use terraflux_graph::{Model, Node};
use terraflux_ops::objects::Objects;
use terraflux_ops::{Registry, Value};

fn model() -> Model {
    let mut m = Model::new("objects live");
    m.nodes.push(
        Node::new("land", "nlm.mpd")
            .param("rows", 60)
            .param("columns", 80)
            .param("seed", 3)
            .param("nlmr_compat", false),
    );
    m.nodes.push(Node::new("fine", "segment.multiresolution").input("image", "land.map").param("scale", 0.02));
    // other segmenters with sliders: SLIC, a chessboard and the merge of its squares by their difference
    m.nodes.push(Node::new("slic", "segment.slic").input("image", "land.map").param("size", 10));
    m.nodes.push(Node::new("chess", "segment.chessboard").input("image", "land.map").param("size", 6));
    m.nodes.push(
        Node::new("diff", "segment.spectral_difference")
            .input("objects", "chess.objects")
            .input("image", "land.map")
            .param("max_difference", 0.001),
    );
    // two classifications of the same objects (a different limit for "big")
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 4).input("map", "land.map"));
    m.nodes.push(Node::new("seg", "segment.from_map").input("map", "cls.classes"));
    for (id, limit) in [("rules_a", 4), ("rules_b", 12)] {
        m.nodes.push(Node::new(id, "objects.rules").input("objects", "seg.objects").param(
            "classes",
            serde_json::json!([
                { "name": "big", "rule": format!("above(cells, {limit}, {})", limit * 2) },
                { "name": "small", "rule": format!("below(cells, {limit}, {})", limit * 2) },
            ]),
        ));
    }
    // two hypotheses over the same cells, and their arbitration
    m.nodes.push(
        Node::new("wet", "nlm.mpd").param("rows", 60).param("columns", 80).param("seed", 5).param("nlmr_compat", false),
    );
    m.nodes.push(Node::new("st", "stack.of").input("trees", "land.map").input("grass", "wet.map"));
    m.nodes.push(Node::new("res", "objects.resolve").input("memberships", "st.stack").param("min", 0.1));
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

/// The model run (every node), and the output `node.port` shown in the Viewer.
fn run_and_show(h: &mut Harness<TerrafluxApp>, node: &str, port: &str) {
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), node);
    }
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    until(h, "the run", |h| {
        let s = h.state().session.as_ref().unwrap();
        s.outputs.get(node).is_some_and(|o| o.contains_key(port))
    });
    h.state_mut().session.as_mut().unwrap().preview = Some((node.into(), port.into()));
    h.run_steps(3);
}

fn objects_of(h: &Harness<TerrafluxApp>, node: &str, port: &str) -> Arc<Objects> {
    match h.state().session.as_ref().unwrap().value_of(node, port) {
        Some(Value::Objects(o)) => o,
        other => panic!("{node}.{port}: {other:?}"),
    }
}

fn param(h: &Harness<TerrafluxApp>, node: &str, name: &str) -> Option<f64> {
    let s = h.state().session.as_ref().unwrap();
    s.source.nodes.iter().find(|n| n.id == node)?.params.get(name)?.as_f64()
}

#[test]
fn a_slider_previews_the_part_in_view_and_apply_writes_the_node() {
    let mut h = harness("objects-live-segment");
    run_and_show(&mut h, "fine", "objects");
    until(&mut h, "the panel", |h| has(h, "LIVE SEGMENTATION"));
    let full = objects_of(&h, "fine", "objects");
    assert!(has(&h, "move a slider"), "nothing is previewed before a slider moves");
    let st = live_segment::state(&h.ctx, "viewer", "fine");
    assert!(st.lock().unwrap().preview.is_none());
    // the scale slider moved to a coarser scale: the part in view is segmented in a thread
    st.lock().unwrap().values[0] = 0.5;
    until(&mut h, "the preview", |h| {
        let _ = h;
        st.lock().unwrap().preview.is_some()
    });
    {
        let l = st.lock().unwrap();
        let p = l.preview.as_ref().unwrap();
        assert!(p.objects.count > 0 && p.objects.count < full.count, "{} against {}", p.objects.count, full.count);
        assert!(l.started >= 1 && l.touched());
        assert!(p.win.cells() <= 60 * 80, "the part in view, not more than the image");
    }
    // drawn over the map: the node's own objects and the preview's, marked as a preview
    until(&mut h, "the outlines", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() >= 2);
    until(&mut h, "its text", |h| has(h, "preview: "));
    // the ESP curve of the window, with the scales it suggests (or the model's segment.scale)
    until(&mut h, "the curve", |_| st.lock().unwrap().esp().is_some());
    assert!(terraflux_app::charts::is_esp(st.lock().unwrap().esp().unwrap()));
    // a suggested scale clicked sets the slider
    let marks = terraflux_app::charts::suggested_scales(st.lock().unwrap().esp().unwrap());
    if let Some(&m) = marks.first() {
        let text = format!("{m:.4}").trim_end_matches('0').trim_end_matches('.').to_string();
        h.run_steps(2);
        h.get_by_label(&text).click();
        h.run_steps(3);
        assert!((st.lock().unwrap().values[0] - m).abs() < 1e-12, "the slider is at {m}");
        st.lock().unwrap().values[0] = 0.5;
        h.run_steps(2);
    }
    // Apply: the node has the values, as one step of undo
    assert_eq!(param(&h, "fine", "scale"), Some(0.02));
    // (the button may move a frame as the curve settles: clicked again until it took)
    for _ in 0..6 {
        if param(&h, "fine", "scale") == Some(0.5) {
            break;
        }
        h.get_by_label("Apply").click();
        h.run_steps(4);
    }
    assert_eq!(param(&h, "fine", "scale"), Some(0.5));
    let s = h.state().session.as_ref().unwrap();
    assert!(s.history.can_undo());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::undo_redo(s, false);
    }
    assert_eq!(param(&h, "fine", "scale"), Some(0.02), "undo puts the scale back");
}

#[test]
fn a_slic_slider_previews_the_whole_small_image_and_apply_writes_the_size() {
    let mut h = harness("objects-live-slic");
    run_and_show(&mut h, "slic", "objects");
    until(&mut h, "the panel", |h| has(h, "LIVE SEGMENTATION"));
    let full = objects_of(&h, "slic", "objects");
    let st = live_segment::state(&h.ctx, "viewer", "slic");
    assert_eq!(st.lock().unwrap().values, [10.0, 1.0], "the node's size, and SLIC's own compactness");
    // a smaller size: more superpixels, of the whole (small) image whatever the view
    st.lock().unwrap().values[0] = 6.0;
    until(&mut h, "the preview", |_| st.lock().unwrap().preview.is_some());
    {
        let l = st.lock().unwrap();
        let p = l.preview.as_ref().unwrap();
        assert!(p.objects.count > full.count, "{} against {}", p.objects.count, full.count);
        assert_eq!((p.win.width, p.win.height, p.step), (80, 60, 1), "the whole image, read whole");
        assert!(l.touched());
    }
    until(&mut h, "the outlines", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() >= 2);
    until(&mut h, "its text", |h| has(h, "preview: "));
    // Apply writes the size as a whole number, one step of undo
    assert_eq!(param(&h, "slic", "size"), Some(10.0));
    for _ in 0..6 {
        if param(&h, "slic", "size") == Some(6.0) {
            break;
        }
        h.get_by_label("Apply").click();
        h.run_steps(4);
    }
    assert_eq!(param(&h, "slic", "size"), Some(6.0));
    {
        let s = h.state().session.as_ref().unwrap();
        let n = s.source.nodes.iter().find(|n| n.id == "slic").unwrap();
        assert!(n.params["size"].is_i64() || n.params["size"].is_u64(), "an integer: {:?}", n.params["size"]);
        assert!(!n.params.contains_key("compactness"), "the compactness at SLIC's own default is not written");
        assert!(s.history.can_undo());
    }
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::undo_redo(s, false);
    }
    assert_eq!(param(&h, "slic", "size"), Some(10.0), "undo puts the size back");
}

#[test]
fn a_spectral_difference_fed_by_a_chessboard_previews_its_merged_squares() {
    let mut h = harness("objects-live-difference-merge");
    run_and_show(&mut h, "diff", "objects");
    until(&mut h, "the panel", |h| has(h, "LIVE SEGMENTATION"));
    let squares = objects_of(&h, "chess", "objects");
    let st = live_segment::state(&h.ctx, "viewer", "diff");
    assert_eq!(st.lock().unwrap().values, [0.001]);
    // a looser limit merges neighbouring squares of alike tone
    st.lock().unwrap().values[0] = 0.3;
    until(&mut h, "the preview", |_| {
        let l = st.lock().unwrap();
        l.preview.is_some() || l.error.is_some()
    });
    let l = st.lock().unwrap();
    assert!(l.error.is_none(), "{:?}", l.error);
    let p = l.preview.as_ref().unwrap();
    assert!(p.objects.count > 0 && p.objects.count < squares.count, "{} against {}", p.objects.count, squares.count);
}

#[test]
fn the_difference_of_two_classified_objects_counts_the_cells_whose_class_differs() {
    let mut h = harness("objects-live-difference");
    // (both classifications run: the second run keeps the first's objects in the cache)
    run_and_show(&mut h, "rules_b", "objects");
    run_and_show(&mut h, "rules_a", "objects");
    let (a, b) = (objects_of(&h, "rules_a", "objects"), objects_of(&h, "rules_b", "objects"));
    assert_eq!(a.classes.len(), 2);
    // the cells with a class in both whose class differs, worked out from the objects
    let class = |o: &Objects, i: usize| match o.labels[i] {
        0 => 0,
        l => o.class_of[l as usize - 1],
    };
    let want =
        (0..a.labels.len()).filter(|&i| class(&a, i) != 0 && class(&b, i) != 0 && class(&a, i) != class(&b, i)).count();
    assert!(want > 0 && want < a.labels.len(), "the two classifications differ in some cells: {want}");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_compare.with = Some(CompareWith::Output("rules_b".into(), "objects".into()));
        s.preview_compare.how = How::Difference;
    }
    until(&mut h, "the difference", |h| {
        matches!(h.state().session.as_ref().unwrap().preview_compare.difference.shown(), Some(Ok(_)))
    });
    let s = h.state().session.as_ref().unwrap();
    let Some(Ok(d)) = s.preview_compare.difference.shown() else { panic!("a difference") };
    let m = d.matrix.as_ref().expect("classes: a matrix of transitions");
    let changed = (m.total(false) - m.kept(false)).round() as usize;
    assert_eq!(changed, want, "the cells that changed class");
    // (the classes are named, not numbered)
    assert!(m.from.iter().any(|(_, n)| n == "big") && m.to.iter().any(|(_, n)| n == "small"), "{:?}", m.from);
    // the curtain and side by side between the two draw (no panic, the other objects in its colours)
    for how in [How::Curtain, How::SideBySide] {
        h.state_mut().session.as_mut().unwrap().preview_compare.how = how;
        h.run_steps(4);
    }
}

#[test]
fn under_the_pointer_the_hypotheses_tell_each_classs_membership() {
    let mut h = harness("objects-live-hypotheses");
    run_and_show(&mut h, "res", "class");
    until(&mut h, "the panel", |h| has(h, "HYPOTHESES"));
    let (trees, grass, layers_before) = {
        let s = h.state().session.as_ref().unwrap();
        let Some(Value::Stack(st)) = s.value_of("st", "stack") else { panic!("the stack") };
        // (the layers are in the order of their names)
        assert_eq!(st.keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(), ["grass", "trees"]);
        (st.layers[1].clone(), st.layers[0].clone(), s.preview_view.layers_drawn().0)
    };
    // the toggle: each class's membership as a layer over the map
    let on = terraflux_app::hypotheses::state(&h.ctx, "viewer", "res", "class");
    on.lock().unwrap().on = true;
    h.run_steps(4);
    until(&mut h, "the layers", |h| {
        h.state().session.as_ref().unwrap().preview_view.layers_drawn().0 >= layers_before + 2
    });
    // the pointer over a cell: the classes' memberships, the winner, the doubt
    let (row, col) = (20, 30);
    let at = h.state().session.as_ref().unwrap().preview_view.cell_screen(col, row).expect("the map drawn");
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(3);
    until(&mut h, "the line", |h| has(h, "under the pointer"));
    let i = row * 80 + col;
    assert!(has(&h, &format!("trees {:.2}", trees[i])), "trees {:.2}", trees[i]);
    assert!(has(&h, &format!("grass {:.2}", grass[i])), "grass {:.2}", grass[i]);
    assert!(has(&h, "winner"), "the winner is told");
    assert!(has(&h, "doubt"), "and the doubt");
}
