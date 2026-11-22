//! Reference objects drawn in the Viewer: the polygons become, in one step of undo, a node that reads them and a
//! `segment.evaluate` of the objects shown; its numbers are shown, and where it errs (os less us) is a diverging layer
//! over the objects; a `segment.optimize` with no reference is offered the same reference.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::draw::{Kind, Sketch};
use terraflux_app::reference_link;
use terraflux_graph::{Model, Node};
use terraflux_ops::{Registry, Value};

/// An image of three regions of 60 x 60 cells, cut into objects by the multiresolution segmentation, and an optimizer
/// of its scale that has no reference.
fn model() -> Model {
    let mut m = Model::new("reference view");
    m.nodes.push(Node::new("land", "nlm.fbm").param("rows", 60).param("columns", 60).param("seed", 5));
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("img", "nlm.image").input("classes", "cls.classes"));
    m.nodes.push(Node::new("segments", "segment.multiresolution").input("image", "img.image").param("scale", 6));
    m.nodes.push(
        Node::new("scales", "segment.optimize")
            .input("image", "img.image")
            .param("from", 4)
            .param("to", 12)
            .param("step", 4),
    );
    m
}

fn harness(name: &str) -> (Harness<'static, TerrafluxApp>, PathBuf) {
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
    let h = Harness::builder()
        .with_size((1400.0, 1500.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app);
    (h, dir)
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

/// The model run to `node`, which is shown.
fn run_and_show(h: &mut Harness<TerrafluxApp>, node: &str) {
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), node);
    }
    h.state_mut().focus(terraflux_app::app::Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    until(h, "the run", |h| h.state().session.as_ref().unwrap().outputs.contains_key(node));
    h.run_steps(3);
}

fn show(h: &mut Harness<TerrafluxApp>, node: &str, port: &str) {
    h.state_mut().session.as_mut().unwrap().preview = Some((node.into(), port.into()));
    h.run_steps(4);
}

/// A click of the primary button at a point.
fn click_at(h: &mut Harness<TerrafluxApp>, at: egui::Pos2) {
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

fn ids(h: &Harness<TerrafluxApp>) -> Vec<String> {
    h.state().session.as_ref().unwrap().source.nodes.iter().map(|n| n.id.clone()).collect()
}

/// Three rectangles drawn on the cells of the image (in cells), written as a GeoPackage.
fn drawn(h: &Harness<TerrafluxApp>, dir: &std::path::Path) -> PathBuf {
    let mut sketch = Sketch::new(Kind::Polygon);
    for (x0, y0, x1, y1) in [(2.0, 2.0, 24.0, 22.0), (30.0, 4.0, 56.0, 26.0), (6.0, 34.0, 30.0, 56.0)] {
        sketch.features.push(vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]);
    }
    let grid = match h.state().session.as_ref().unwrap().value_of("segments", "objects") {
        Some(Value::Objects(o)) => o.grid.clone(),
        other => panic!("{other:?}"),
    };
    let path = dir.join("drawn").join("reference.gpkg");
    terraflux_app::draw::write(&sketch, &grid, &path).unwrap();
    path
}

#[test]
fn polygons_drawn_become_a_reference_and_an_evaluation_in_one_step_of_undo() {
    let (mut h, dir) = harness("reference-view-add");
    run_and_show(&mut h, "segments");
    let before = ids(&h);
    let path = drawn(&h, &dir);
    let key = ("segments".to_string(), "objects".to_string());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let reader = reference_link::add(s, &Registry::standard(), &key, &path).unwrap();
        assert_eq!(reader, "reference");
    }
    // a reader of the file and an evaluation of the objects with it
    let after = ids(&h);
    assert_eq!(after.len(), before.len() + 2, "{after:?}");
    {
        let s = h.state().session.as_ref().unwrap();
        let evaluate = s.source.nodes.iter().find(|n| n.op == "segment.evaluate").expect("an evaluation");
        assert!(
            evaluate.inputs.get("objects").map(|i| format!("{i:?}")).unwrap_or_default().contains("segments.objects")
        );
        assert!(format!("{:?}", evaluate.inputs.get("reference")).contains("reference.vector"));
        let reader = s.source.nodes.iter().find(|n| n.id == "reference").unwrap();
        assert_eq!(reader.op, "vector.read");
    }
    // one step of undo takes both away, and redo brings them back
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::undo_redo(s, false);
    }
    assert_eq!(ids(&h), before);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::undo_redo(s, true);
    }
    assert_eq!(ids(&h), after);
    // drawn again over the same objects: the reader is replaced in the evaluation, not another evaluation made
    let second = dir.join("drawn").join("again.gpkg");
    std::fs::copy(&path, &second).unwrap();
    let s = h.state_mut().session.as_mut().unwrap();
    reference_link::add(s, &Registry::standard(), &key, &second).unwrap();
    assert_eq!(s.source.nodes.iter().filter(|n| n.op == "segment.evaluate").count(), 1);
}

#[test]
fn the_numbers_are_shown_and_the_errors_are_a_layer_of_os_less_us() {
    let (mut h, dir) = harness("reference-view-layer");
    run_and_show(&mut h, "segments");
    let path = drawn(&h, &dir);
    let key = ("segments".to_string(), "objects".to_string());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        reference_link::add(s, &Registry::standard(), &key, &path).unwrap();
    }
    run_and_show(&mut h, "evaluate");
    // the summary's numbers
    let n = reference_link::numbers(h.state().session.as_ref().unwrap(), "evaluate");
    let names: Vec<&str> = n.iter().map(|(k, _)| *k).collect();
    assert_eq!(names, ["ed2", "d", "f_overlap", "os", "us"]);
    assert!(n.iter().all(|(_, v)| v.is_finite()), "{n:?}");
    // the objects shown (the segmentation): asked for, the errors are a layer of os less us on each object
    show(&mut h, "segments", "objects");
    reference_link::show_errors(&h.ctx, "evaluate", true);
    h.run_steps(4);
    let lit = reference_link::lit(&h.ctx, "evaluate").expect("the errors drawn");
    let Some(Value::Objects(o)) = h.state().session.as_ref().unwrap().value_of("evaluate", "objects") else { panic!() };
    let (os, us) = (o.table.column("os").unwrap(), o.table.column("us").unwrap());
    assert_eq!(lit.data.len(), o.labels.len());
    let mut finite = 0;
    for (v, &l) in lit.data.iter().zip(o.labels.iter()) {
        if l == 0 {
            assert!(v.is_nan());
        } else {
            let want = os[l as usize - 1] - us[l as usize - 1];
            assert!((v.is_nan() && want.is_nan()) || (v - want).abs() < 1e-12, "object {l}: {v} {want}");
            finite += usize::from(v.is_finite());
        }
    }
    // (the objects that touch a reference have a value, the others none)
    assert!(finite > 0, "{finite} of {}", lit.data.len());
    // and not asked for, no layer
    reference_link::show_errors(&h.ctx, "evaluate", false);
    let s = h.state().session.as_ref().unwrap();
    assert!(reference_link::layers(&h.ctx, s, "segments", "objects").is_empty());
}

#[test]
fn an_optimizer_without_a_reference_is_offered_the_one_drawn_in_one_step_of_undo() {
    let (mut h, dir) = harness("reference-view-optimizer");
    run_and_show(&mut h, "segments");
    let path = drawn(&h, &dir);
    let key = ("segments".to_string(), "objects".to_string());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        reference_link::add(s, &Registry::standard(), &key, &path).unwrap();
    }
    show(&mut h, "segments", "objects");
    h.run_steps(4);
    let before = h.state().session.as_ref().unwrap().source.clone();
    assert!(!before.nodes.iter().find(|n| n.id == "scales").unwrap().inputs.contains_key("reference"));
    let at = h.get_by_label_contains("Use as the reference of scales").rect().center();
    click_at(&mut h, at);
    let s = h.state().session.as_ref().unwrap();
    let scales = s.source.nodes.iter().find(|n| n.id == "scales").unwrap();
    assert!(
        format!("{:?}", scales.inputs.get("reference")).contains("reference.vector"),
        "{:?} {:?}",
        scales.inputs,
        s.notice
    );
    // one step of undo takes the reference away again
    let s = h.state_mut().session.as_mut().unwrap();
    terraflux_app::builder::undo_redo(s, false);
    assert!(!s.source.nodes.iter().find(|n| n.id == "scales").unwrap().inputs.contains_key("reference"));
    assert!(s.source.nodes.iter().any(|n| n.op == "segment.evaluate"));
}
