//! The builder's canvas with large graphs: laid out in time proportional to its nodes, drawn by the GPU, its
//! frames short whatever the size of the graph.
//!
//! The timings: `cargo test --release -p terraflux-app --test app canvas:: -- --ignored --nocapture`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_graph::{Model, Node};

/// A model of `n` nodes of map algebra, each reading the one before and another earlier one; placed in a grid
/// (`placed`) or left to the automatic layout.
fn graph(n: usize, placed: bool) -> Model {
    let mut m = Model::new("big");
    for i in 0..n {
        let mut node = Node::new(&format!("n{i}"), "raster.calc").param("expression", "a + b");
        if i > 0 {
            node = node.input("a", &format!("n{}.map", i - 1));
        }
        if i > 7 {
            node = node.input("b", &format!("n{}.map", i * 7 % i));
        }
        if placed {
            node.pos = Some([(i % 40) as f32 * 260.0, (i / 40) as f32 * 120.0]);
        }
        m.nodes.push(node);
    }
    m
}

fn open(m: &Model, name: &str) -> TerrafluxApp {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    m.save(&p).unwrap();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut app = TerrafluxApp::new(Some(p));
    app.session.as_mut().unwrap().wait();
    app
}

fn harness(app: TerrafluxApp) -> Harness<'static, TerrafluxApp> {
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    terraflux_app::canvas_gpu::install(&rs);
    // frames of a sixtieth of a second, as on a screen (two clicks a frame apart: a double click)
    Harness::builder()
        .with_size((1600.0, 1000.0))
        .with_step_dt(1.0 / 60.0)
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app)
}

/// The median and the worst of `n` frames.
fn frames(h: &mut Harness<TerrafluxApp>, n: usize, each: impl Fn(&mut TerrafluxApp)) -> (Duration, Duration) {
    let mut all: Vec<Duration> = (0..n)
        .map(|_| {
            each(h.state_mut());
            let t = Instant::now();
            h.step();
            t.elapsed()
        })
        .collect();
    all.sort();
    (all[n / 2], all[n - 1])
}

#[test]
fn large_graphs_are_laid_out_and_drawn() {
    // 5000 nodes without places: the automatic layout, every node in the scene
    let mut h = harness(open(&graph(5000, false), "canvas-auto.toml"));
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.editor.boxes(), 5000);
    h.render().expect("the canvas drawn by wgpu");
    // the whole graph in view (zoomed out: boxes and connections, no texts), then close (texts of the nodes in
    // view only)
    h.state_mut().session.as_mut().unwrap().editor.fit();
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.zoom() < 0.1);
}

#[test]
#[ignore = "timings: run with --release --ignored --nocapture"]
fn timings() {
    for n in [100, 1000, 10_000] {
        let mut h = harness(open(&graph(n, true), &format!("canvas-{n}.toml")));
        h.run_steps(3);
        // the whole graph in view: boxes and connections, by the GPU
        let all = frames(&mut h, 30, |_| {});
        // close: the texts of the nodes in view
        h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(40.0, 40.0), 1.0);
        h.run_steps(2);
        let close = frames(&mut h, 30, |_| {});
        // a node dragged by the pointer: it alone moves in the scene
        let at = editor(&h).screen_of("n41").unwrap();
        h.hover_at(at);
        h.run_steps(1);
        h.drag_at(at);
        h.run_steps(1);
        let mut moved = Vec::new();
        for k in 1..=30 {
            h.hover_at(at + egui::vec2(k as f32 * 4.0, k as f32 * 2.0));
            let t = Instant::now();
            h.step();
            moved.push(t.elapsed());
        }
        h.drop_at(at + egui::vec2(120.0, 60.0));
        h.run_steps(2);
        moved.sort();
        // an edit: the model checked again and the scene laid out again
        let t = Instant::now();
        h.state_mut().session.as_mut().unwrap().edited();
        let checked = t.elapsed();
        let t = Instant::now();
        h.step();
        let laid = t.elapsed();
        let t = Instant::now();
        h.render().unwrap();
        let rendered = t.elapsed();
        println!(
            "{n:>6} nodes: frames (median, worst) all in view {all:?}, close {close:?}, dragging a node ({:?}, {:?}); an edit: checked {checked:?}, laid out {laid:?}; rendered and read back by wgpu {rendered:?}",
            moved[15], moved[29]
        );
    }
}

fn editor<'a>(h: &'a Harness<TerrafluxApp>) -> &'a terraflux_app::editor_view::EditorState {
    &h.state().session.as_ref().unwrap().editor
}

/// Press, move in steps and release the primary button (a drag), a frame per event.
fn drag(h: &mut Harness<TerrafluxApp>, from: egui::Pos2, to: egui::Pos2) {
    h.hover_at(from);
    h.run_steps(1);
    h.drag_at(from);
    h.run_steps(1);
    for k in 1..=6 {
        h.hover_at(from + (to - from) * (k as f32 / 6.0));
        h.run_steps(1);
    }
    h.drop_at(to);
    h.run_steps(2);
}

/// Press and release the primary button at a point (a click).
fn click(h: &mut Harness<TerrafluxApp>, at: egui::Pos2) {
    h.hover_at(at);
    h.run_steps(1);
    h.drag_at(at);
    h.run_steps(1);
    h.drop_at(at);
    h.run_steps(2);
}

#[test]
fn the_canvas_answers_the_pointer() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let mut m = Model::new("pointer");
    m.inputs.insert(
        "t0".into(),
        terraflux_graph::Input::Map { path: data.to_string_lossy().into_owned(), band: None, nodata: None, note: None },
    );
    let mut dist = Node::new("dist", "dynamic.distance_to").param("classes", 1);
    dist.pos = Some([0.0, 0.0]);
    let mut calc = Node::new("calc", "raster.calc").input("land", "$t0").param("expression", "land");
    calc.pos = Some([0.0, 220.0]);
    m.nodes.extend([dist, calc]);
    let mut h = harness(open(&m, "canvas-pointer.toml"));
    h.run_steps(3);
    // a click on a node selects it and shows its output
    let at = editor(&h).screen_of("calc").unwrap();
    click(&mut h, at);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.selected.as_deref(), Some("calc"));
    assert_eq!(s.preview, Some(("calc".into(), "map".into())));
    // a node dragged moves by the pointer's way (in the graph's units)
    let zoom = editor(&h).zoom();
    let at = editor(&h).screen_of("dist").unwrap();
    drag(&mut h, at, at + egui::vec2(120.0, 40.0));
    let p = h.state().session.as_ref().unwrap().source.nodes[0].pos.unwrap();
    assert!((p[0] - 120.0 / zoom).abs() < 2.0 && (p[1] - 40.0 / zoom).abs() < 2.0, "{p:?} at zoom {zoom}");
    assert_eq!(editor(&h).screen_of("dist").unwrap(), at + egui::vec2(120.0, 40.0), "the scene follows");
    // a connection drawn from the model's input to the node's input
    let from = editor(&h).screen_of_output("$t0", "").unwrap();
    let to = editor(&h).screen_of_input("dist", "landscape").unwrap();
    drag(&mut h, from, to);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes[0].inputs["landscape"], terraflux_graph::Source::Ref("$t0".into()));
    // a click on an output shows it
    let out = editor(&h).screen_of_output("dist", "distance").unwrap();
    click(&mut h, out);
    assert_eq!(h.state().session.as_ref().unwrap().preview, Some(("dist".into(), "distance".into())));
    // the wheel zooms around the pointer
    h.hover_at(at);
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 120.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(10);
    assert!(editor(&h).zoom() > zoom * 1.2, "{} after {zoom}", editor(&h).zoom());
    // Home fits the graph again
    h.key_press(egui::Key::Home);
    h.run_steps(3);
    assert!((editor(&h).zoom() - zoom).abs() < 0.05);
}

/// A raster of any size shown at once: `TERRAFLUX_BIG_RASTER=path cargo test --release -p terraflux-app --test
/// canvas big_raster -- --ignored --nocapture`.
#[test]
#[ignore = "a large raster of one's own: TERRAFLUX_BIG_RASTER"]
fn big_raster_shows_at_once() {
    let Ok(path) = std::env::var("TERRAFLUX_BIG_RASTER") else { return };
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut h = harness(TerrafluxApp::new(None));
    h.run_steps(2);
    let t = Instant::now();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::add(s, &terraflux_ops::Registry::standard(), "raster.read", &[("path", path.into())])
            .unwrap();
    }
    let mut frames = Vec::new();
    let mut first = None;
    loop {
        let f = Instant::now();
        h.step();
        frames.push(f.elapsed());
        let s = h.state().session.as_ref().unwrap();
        if first.is_none() && s.preview_view.textures() > 0 {
            first = Some(t.elapsed());
        }
        if s.preview_value().is_some() && !s.preview_view.busy() && s.preview_view.textures() > 0 && !s.running() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120));
    }
    let all = t.elapsed();
    frames.sort();
    println!(
        "read and shown: first tile {:?}, the whole view {all:?}; {} frames meanwhile, median {:?} worst {:?}",
        first.unwrap(),
        frames.len(),
        frames[frames.len() / 2],
        frames.last().unwrap()
    );
    // zoomed in 64 times around the centre: the finer tiles stream in
    let before = h.state().session.as_ref().unwrap().preview_view.zoom();
    let centre = egui::pos2(672.0, 820.0);
    h.hover_at(centre);
    let t = Instant::now();
    for _ in 0..12 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 140.0),
            phase: egui::TouchPhase::Move,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    let mut frames = Vec::new();
    loop {
        let f = Instant::now();
        h.step();
        frames.push(f.elapsed());
        if !h.state().session.as_ref().unwrap().preview_view.busy() {
            break;
        }
        std::thread::sleep(Duration::from_millis(2));
        assert!(t.elapsed() < Duration::from_secs(120));
    }
    frames.sort();
    let after = h.state().session.as_ref().unwrap().preview_view.zoom();
    assert!(after > before * 30.0, "zoomed {before} -> {after}");
    println!(
        "zoomed in {:.0} times: the finer view complete in {:?}; frames median {:?} worst {:?}",
        after / before,
        t.elapsed(),
        frames[frames.len() / 2],
        frames.last().unwrap()
    );
    h.render().unwrap().save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ui-big-raster.png")).unwrap();
}

/// A double click: two clicks a frame apart.
fn double_click(h: &mut Harness<TerrafluxApp>, at: egui::Pos2) {
    h.hover_at(at);
    h.run_steps(1);
    for pressed in [true, false, true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.step();
    }
    h.run_steps(3);
}

#[test]
fn nodes_are_selected_moved_deleted_arranged_and_opened() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let mut m = Model::new("select");
    let mut read = Node::new("read", "raster.read").param("path", data.to_string_lossy().as_ref());
    read.pos = Some([0.0, 0.0]);
    m.nodes.push(read);
    for (k, x) in [(1usize, 300.0f32), (2, 600.0), (3, 900.0)] {
        let from = if k == 1 { "read.raster".to_string() } else { format!("n{}.map", k - 1) };
        let mut n = Node::new(&format!("n{k}"), "raster.calc").input("a", &from).param("expression", "a");
        n.pos = Some([x, 0.0]);
        m.nodes.push(n);
    }
    let mut h = harness(open(&m, "canvas-select.toml"));
    h.run_steps(3);
    let pos = |h: &Harness<TerrafluxApp>, id: &str| {
        h.state().session.as_ref().unwrap().source.nodes.iter().find(|n| n.id == id).unwrap().pos.unwrap()
    };
    // a box around n1 and n2, from the empty canvas above them
    let (a, b) = (editor(&h).screen_of("n1").unwrap(), editor(&h).screen_of("n2").unwrap());
    drag(&mut h, a - egui::vec2(60.0, 40.0), b + egui::vec2(60.0, 60.0));
    let chosen: Vec<String> = editor(&h).chosen.iter().cloned().collect();
    assert_eq!(chosen, ["n1", "n2"]);
    // dragging one moves both, the same way
    let (p1, p2) = (pos(&h, "n1"), pos(&h, "n2"));
    let at = editor(&h).screen_of("n1").unwrap();
    drag(&mut h, at, at + egui::vec2(40.0, 30.0));
    let (q1, q2) = (pos(&h, "n1"), pos(&h, "n2"));
    assert!(
        (q1[0] - p1[0] - (q2[0] - p2[0])).abs() < 0.01
            && q1[0] > p1[0]
            && (q1[1] - p1[1] - (q2[1] - p2[1])).abs() < 0.01
    );
    // no wall: a node goes left and up of the graph's origin as freely as anywhere
    let at = editor(&h).screen_of("read").unwrap();
    drag(&mut h, at, at - egui::vec2(500.0, 300.0));
    let p = pos(&h, "read");
    assert!(p[0] < -100.0 && p[1] < -100.0, "{p:?}");
    // (the selection back to n1 and n2)
    h.state_mut().session.as_mut().unwrap().editor.chosen = ["n1".to_string(), "n2".to_string()].into_iter().collect();
    h.run_steps(1);
    // shift and a click: one less
    h.hover_at(editor(&h).screen_of("n2").unwrap());
    h.event_modifiers(
        egui::Event::PointerButton {
            pos: editor(&h).screen_of("n2").unwrap(),
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::SHIFT,
        },
        egui::Modifiers::SHIFT,
    );
    h.step();
    h.event_modifiers(
        egui::Event::PointerButton {
            pos: editor(&h).screen_of("n2").unwrap(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::SHIFT,
        },
        egui::Modifiers::SHIFT,
    );
    h.run_steps(2);
    assert_eq!(editor(&h).chosen.iter().cloned().collect::<Vec<_>>(), ["n1"]);
    // ctrl+A: all; del: all gone
    h.hover_at(editor(&h).screen_of("n3").unwrap() + egui::vec2(0.0, 200.0));
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.run_steps(2);
    assert_eq!(editor(&h).chosen.len(), 4);
    {
        // keep two: select n2 and n3 only, then delete them
        let s = h.state_mut().session.as_mut().unwrap();
        s.editor.chosen = ["n2".to_string(), "n3".to_string()].into_iter().collect();
    }
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    let ids: Vec<String> = h.state().session.as_ref().unwrap().source.nodes.iter().map(|n| n.id.clone()).collect();
    assert_eq!(ids, ["read", "n1"]);
    // arranged: by what they read, left to right, none over another
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes[1].pos = Some([0.0, 0.0]);
        terraflux_app::builder::arrange(s, &terraflux_ops::Registry::standard());
    }
    h.run_steps(3);
    let (r, n1) = (pos(&h, "read"), pos(&h, "n1"));
    assert!(n1[0] > r[0], "{r:?} {n1:?}");
    // a double click on a node: its map in the Viewer (no tab of its own), run first
    // (the clicks before far enough in time not to make a triple click)
    h.run_steps(60);
    let at = editor(&h).screen_of("read").unwrap();
    double_click(&mut h, at);
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().running() {
        h.step();
        assert!(t.elapsed() < Duration::from_secs(30));
    }
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.selected.as_deref(), Some("read"));
    assert_eq!(s.preview, Some(("read".to_string(), "raster".to_string())));
    assert!(s.value_of("read", "raster").is_some());
    assert!(s.preview_layers.iter().any(|l| l.is("read", "raster")), "kept as a layer");
    assert_eq!(h.state_mut().focused(), Some(terraflux_app::app::Pane::Viewer), "the Viewer in front");
    h.render().unwrap().save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ui-13-map-tab.png")).unwrap();
}

/// A file dropped on a window from the system's file manager.
#[derive(Debug)]
struct Dropped(PathBuf);

impl egui::DroppedFile for Dropped {
    fn path(&self) -> &std::path::Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|e| e.to_string())
    }
}

#[test]
fn files_become_nodes_and_rasters_join_a_stack_by_a_connection() {
    let inputs = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs");
    let mut m = Model::new("stack");
    let mut stack = Node::new("drivers", "stack.of");
    stack.pos = Some([600.0, 0.0]);
    m.nodes.push(stack);
    let mut h = harness(open(&m, "canvas-files.toml"));
    h.run_steps(3);
    assert!(!h.state().is_open(Pane::Viewer), "no viewer before there is data");
    // files dropped on the window (a raster, vector features and a folder of rasters, read as one stack): a node
    // that reads each
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let geojson = dir.join("canvas-points.geojson");
    std::fs::write(&geojson, r#"{"type":"FeatureCollection","features":[]}"#).unwrap();
    for p in [inputs.join("sibuyan_landuse.tif"), geojson.clone(), inputs.clone()] {
        h.input_mut().dropped_files.push(std::sync::Arc::new(Dropped(p)));
    }
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let read: Vec<(&str, &str)> =
        s.source.nodes.iter().skip(1).map(|n| (n.op.as_str(), n.params["path"].as_str().unwrap())).collect();
    let tif = inputs.join("sibuyan_landuse.tif").to_string_lossy().into_owned();
    let folder = inputs.to_string_lossy().into_owned();
    assert_eq!(
        read,
        [("raster.read", tif.as_str()), ("vector.read", geojson.to_str().unwrap()), ("raster.read", folder.as_str())]
    );
    let raster = s.source.nodes[1].id.clone();
    // the first data: the viewer, in a tab beside the model's, behind it
    assert!(h.state().is_open(Pane::Viewer));
    assert_eq!(h.state_mut().focused(), Some(Pane::Model));
    // stack.of takes any maps: a connection let go on its place of a new input, or on the node, is one more
    // input, named as the node it comes from
    h.run_steps(3);
    let from = editor(&h).screen_of_output(&raster, "raster").unwrap();
    let to = editor(&h).screen_of_input("drivers", "").unwrap();
    drag(&mut h, from, to);
    let ins = &h.state().session.as_ref().unwrap().source.nodes[0].inputs;
    assert_eq!(ins.get(&raster), Some(&terraflux_graph::Source::Ref(format!("{raster}.raster"))), "{ins:?}");
    let to = editor(&h).screen_of("drivers").unwrap();
    drag(&mut h, from, to);
    let ins = &h.state().session.as_ref().unwrap().source.nodes[0].inputs;
    assert_eq!(ins.get(&format!("{raster}_2")), Some(&terraflux_graph::Source::Ref(format!("{raster}.raster"))));
    // a raster of the Files pane carried to the canvas: a node where it is let go
    let canvas = editor(&h).canvas().unwrap();
    let at = canvas.center() + egui::vec2(0.0, 150.0);
    h.hover_at(at);
    h.run_steps(1);
    egui::DragAndDrop::set_payload(
        &h.ctx,
        terraflux_app::builder::DraggedFile(vec![inputs.join("sibuyan_drivers.tif")]),
    );
    h.run_steps(1);
    h.drag_at(at);
    h.run_steps(1);
    h.drop_at(at);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    let last = s.source.nodes.last().unwrap();
    assert_eq!(last.op, "raster.read");
    assert!(last.params["path"].as_str().unwrap().ends_with("sibuyan_drivers.tif"));
    let p = last.pos.unwrap();
    let q = editor(&h).graph_at(at).unwrap();
    assert!((p[0] + terraflux_app::editor_view::WIDTH / 2.0 - q.x).abs() < 1.0, "{p:?} {q:?}");
}

#[test]
fn new_inputs_are_named_after_what_they_read() {
    use terraflux_app::editor_view::new_input;
    assert_eq!(new_input("elevation.map", &[]), "elevation");
    assert_eq!(new_input("elevation.map", &["elevation", "elevation_2"]), "elevation_3");
    assert_eq!(new_input("$t0", &[""]), "t0");
    assert_eq!(new_input("@step", &[]), "step");
}

/// A model opened after another (a template after the blank model the application starts with): its shapes reach
/// the GPU, whose canvas keeps the shapes it was given last until their generation changes (both editors' first
/// generation was 1: the template's boxes were never drawn, only their texts).
#[test]
fn another_model_is_drawn() {
    let mut h = harness(TerrafluxApp::new(None));
    h.run_steps(3);
    let before = editor(&h).generation();
    assert!(before > 0, "the blank model's canvas drawn");
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.state_mut().focus(Pane::Model);
    h.run_steps(3);
    assert!(editor(&h).boxes() > 0);
    assert_ne!(editor(&h).generation(), before, "new shapes for the GPU");
}

/// A layout kept with the viewer in front of the model's tab: the viewer is let go when the application opens
/// (it comes with the data), and the model's tab is the one in front (egui_dock left the leaf's index past its tabs:
/// the model's pane was blank).
#[test]
fn a_layout_kept_with_the_viewer_in_front_shows_the_model() {
    // a layout drawn (its rectangles known, as the application keeps it), the viewer in front of the model
    let mut h = harness(TerrafluxApp::new(None));
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    let dock = h.state().dock.clone().unwrap();
    let viewer = dock.find_tab(&Pane::Viewer).unwrap();
    assert_eq!(dock.find_tab(&Pane::Model).map(|p| p.node_path()), Some(viewer.node_path()));
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-layout.json");
    let text = serde_json::json!({ "version": 7, "main": dock, "windows": [] });
    std::fs::write(&path, text.to_string()).unwrap();
    let mut app = TerrafluxApp::new(None);
    app.restore_layout(path);
    assert!(!app.is_open(Pane::Viewer), "the viewer comes with the data");
    assert!(!app.is_open(Pane::Node) && !app.is_open(Pane::Prepare), "the node's pane comes with the first node");
    let dock = app.dock.as_ref().unwrap();
    let at = dock.find_tab(&Pane::Model).unwrap();
    let leaf = dock.leaf(at.node_path()).unwrap();
    assert_eq!(leaf.tabs().get(leaf.active.0), Some(&Pane::Model));
}

/// The panes that only make sense with something in the model are not there when the application opens (nor
/// from a layout kept, which opens on the model): the node's pane comes with the first node (at the model's left,
/// the graph keeping the focus) and the model's document behind the model's tab, the preparation of the data behind it with the first data, the calculator only when a script is opened
/// in it.
#[test]
fn the_node_prepare_and_calculator_come_when_they_make_sense() {
    let mut h = harness(TerrafluxApp::new(None));
    h.run_steps(2);
    for p in [Pane::Node, Pane::Document, Pane::Prepare, Pane::Calculator, Pane::Viewer] {
        assert!(!h.state().is_open(p), "{p:?} at the start");
    }
    // a layout kept with them all (the document's tab in front, beside the model's): not read back, the model
    // in front
    let mut kept = h.state().dock.clone().unwrap();
    kept.push_to_first_leaf(Pane::Node);
    kept.push_to_first_leaf(Pane::Prepare);
    kept.push_to_first_leaf(Pane::Calculator);
    kept.push_to_first_leaf(Pane::Areas);
    kept.push_to_first_leaf(Pane::Document);
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("due-layout.json");
    let text = serde_json::json!({ "version": 11, "main": kept, "windows": [[Pane::Calculator]] });
    std::fs::write(&path, text.to_string()).unwrap();
    h.state_mut().restore_layout(path);
    h.run_steps(2);
    for p in [Pane::Node, Pane::Document, Pane::Prepare, Pane::Calculator] {
        assert!(!h.state().is_open(p), "{p:?} from the layout kept");
    }
    assert!(h.state().is_open(Pane::Areas), "the others kept");
    assert_eq!(h.state_mut().focused(), Some(Pane::Model), "the application opens on the model");
    // the first node: the node's pane, at the model's left; no data yet, no preparation
    {
        let app = h.state_mut();
        let r = app.registry.clone();
        terraflux_app::builder::add(app.session.as_mut().unwrap(), &r, "nlm.random", &[]).unwrap();
    }
    h.run_steps(2);
    assert!(h.state().is_open(Pane::Node), "the node's pane with the first node");
    assert!(!h.state().is_open(Pane::Prepare) && !h.state().is_open(Pane::Calculator));
    let focused = h.state_mut().focused();
    assert_eq!(focused, Some(Pane::Model), "the model's tab keeps the front and the focus");
    // the model's document with it: behind the model's tab
    {
        let dock = h.state().dock.as_ref().unwrap();
        let model = dock.find_tab(&Pane::Model).unwrap();
        assert_eq!(dock.find_tab(&Pane::Document).map(|p| p.node_path()), Some(model.node_path()));
        let leaf = dock.leaf(model.node_path()).unwrap();
        assert_eq!(leaf.tabs().get(leaf.active.0), Some(&Pane::Model));
    }
    // closed by the user: not back with the next node
    {
        let d = h.state_mut().dock.as_mut().unwrap();
        let at = d.find_tab(&Pane::Node).unwrap();
        d.remove_tab(at);
    }
    {
        let app = h.state_mut();
        let r = app.registry.clone();
        terraflux_app::builder::add(app.session.as_mut().unwrap(), &r, "nlm.random", &[]).unwrap();
    }
    h.run_steps(2);
    assert!(!h.state().is_open(Pane::Node), "closed, it stays closed");
    // a template (of data): the node's pane and the preparation behind it, the viewer behind the graph
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(2);
    assert!(h.state().is_open(Pane::Node) && h.state().is_open(Pane::Prepare) && h.state().is_open(Pane::Viewer));
    assert!(!h.state().is_open(Pane::Calculator), "the calculator only when a script is opened in it");
    let dock = h.state().dock.as_ref().unwrap();
    let node = dock.find_tab(&Pane::Node).unwrap();
    assert_eq!(dock.find_tab(&Pane::Prepare).map(|p| p.node_path()), Some(node.node_path()), "together");
    let leaf = dock.leaf(node.node_path()).unwrap();
    assert_eq!(leaf.tabs().get(leaf.active.0), Some(&Pane::Node), "the node's pane in front");
}

/// A box is as wide as its texts need (an input's and an output's names side by side never overlap), wider when
/// its right edge is dragged, never narrower than its texts, and as wide as they need again after a double click
/// on the edge; the columns of Arrange are as wide as their widest box.
#[test]
fn nodes_fit_their_texts_and_are_resized_by_their_edge() {
    let mut m = Model::new("widths");
    let mut calc = Node::new("calc", "raster.calc").param("expression", "1");
    calc.pos = Some([0.0, 0.0]);
    m.nodes.push(calc);
    let mut plan = Node::new("plan", "prioritize.optimize").input("cost", "calc.map");
    plan.pos = Some([300.0, 0.0]);
    m.nodes.push(plan);
    let mut h = harness(open(&m, "canvas-widths.toml"));
    h.run_steps(3);
    let width = |h: &Harness<TerrafluxApp>, id: &str| editor(h).width_of(id).unwrap();
    let saved = |h: &Harness<TerrafluxApp>, id: &str| {
        h.state().session.as_ref().unwrap().source.nodes.iter().find(|n| n.id == id).unwrap().width
    };
    let (calc_w, calc_need) = width(&h, "calc");
    assert_eq!((calc_w, calc_need), (terraflux_app::editor_view::WIDTH, terraflux_app::editor_view::WIDTH));
    // its optional inputs folded: as wide as the rows shown need, which the estimate of the layout follows
    let (folded_w, folded_need) = width(&h, "plan");
    assert_eq!(folded_w, folded_need);
    let r = terraflux_ops::Registry::standard();
    let estimated = terraflux_app::editor_view::node_width(&m.nodes[1], &r);
    assert!((estimated - folded_need).abs() <= 40.0, "the estimate {estimated} near the measure {folded_need}");
    // unfolded, prioritize.optimize's rows (asym_connectivity_data across from extinction_risk...) need more
    h.state_mut().session.as_mut().unwrap().editor.toggle_fold("plan");
    h.run_steps(2);
    let (plan_w, plan_need) = width(&h, "plan");
    assert!(plan_w > terraflux_app::editor_view::WIDTH + 40.0 && plan_w == plan_need, "{plan_w}");
    assert!(plan_w > folded_w, "{plan_w} unfolded, {folded_w} folded");
    h.render().unwrap().save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-widths.png")).unwrap();
    // its right edge dragged 100 points to the right: 100 / zoom wider, kept in the model
    let edge = |h: &Harness<TerrafluxApp>, id: &str| {
        let r = editor(h).screen_rect_of(id).unwrap();
        egui::pos2(r.right(), r.center().y + 7.0)
    };
    let zoom = editor(&h).zoom();
    let at = edge(&h, "calc");
    drag(&mut h, at, at + egui::vec2(100.0, 0.0));
    let w = saved(&h, "calc").expect("a width");
    assert!((w - (calc_w + 100.0 / zoom)).abs() <= 2.0, "{w} {calc_w} {zoom}");
    assert!((width(&h, "calc").0 - w).abs() < 0.01, "the scene follows");
    // dragged far to the left: no narrower than its texts
    let at = edge(&h, "plan");
    drag(&mut h, at, at - egui::vec2(300.0, 0.0));
    assert_eq!(saved(&h, "plan"), Some(plan_need));
    assert_eq!(width(&h, "plan").0, plan_need);
    // a double click on the edge: as wide as its texts need, no width kept
    let at = edge(&h, "calc");
    double_click(&mut h, at);
    assert_eq!(saved(&h, "calc"), None);
    assert_eq!(width(&h, "calc").0, terraflux_app::editor_view::WIDTH);
    // Arrange: the next column begins after the widest box of the one before
    let mut wide = m.clone();
    wide.nodes.push(Node::new("after", "raster.calc").input("a", "plan.solution").param("expression", "a"));
    let places = terraflux_app::editor_view::arrange(&wide.nodes, &r);
    assert!(places["after"][0] >= places["plan"][0] + estimated + 60.0, "{places:?}");
}

/// A node's optional inputs are folded in a row a click unfolds (and folds again); a connection let go on the node
/// rather than on an input offers the free inputs it fits, and goes to the one chosen.
#[test]
fn optional_inputs_fold_and_a_connection_dropped_on_a_node_picks_one() {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let mut m = Model::new("fold");
    m.inputs.insert(
        "t0".into(),
        terraflux_graph::Input::Map { path: data.to_string_lossy().into_owned(), band: None, nodata: None, note: None },
    );
    let mut zon = Node::new("zon", "prioritize.zonation");
    zon.pos = Some([0.0, 0.0]);
    m.nodes.push(zon);
    let mut h = harness(open(&m, "canvas-fold.toml"));
    h.run_steps(3);
    // folded: the input it needs and its essential ones shown, the other optional ones not
    assert!(editor(&h).screen_of_input("zon", "features").is_some());
    assert!(editor(&h).screen_of_input("zon", "cost").is_some(), "an essential input");
    assert!(editor(&h).screen_of_input("zon", "areas").is_none());
    let fold = editor(&h).screen_of_fold("zon").expect("the row of the optional inputs");
    click(&mut h, fold);
    assert!(editor(&h).screen_of_input("zon", "areas").is_some(), "unfolded");
    h.render().unwrap().save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-unfolded.png")).unwrap();
    let fold = editor(&h).screen_of_fold("zon").unwrap();
    click(&mut h, fold);
    assert!(editor(&h).screen_of_input("zon", "areas").is_none(), "folded again");
    // the model's map let go on the node's header: the inputs it fits, in a list
    let from = editor(&h).screen_of_output("$t0", "").unwrap();
    let to = editor(&h).screen_of("zon").unwrap();
    drag(&mut h, from, to);
    h.run_steps(2);
    h.render().unwrap().save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-pick.png")).unwrap();
    use egui_kittest::kittest::Queryable;
    h.query_all_by_label_contains("areas  ·").next().expect("areas in the list").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes[0].inputs.get("areas"), Some(&terraflux_graph::Source::Ref("$t0".into())));
    // given, it is shown though its others stay folded
    assert!(editor(&h).screen_of_input("zon", "areas").is_some());
    // zoomed far out: the names still read, in the middle of their boxes
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(300.0, 300.0), 0.25);
    h.run_steps(2);
    h.render().unwrap().save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-far.png")).unwrap();
}

/// A raster node at a place: the models of the wires let go on the empty canvas.
fn reader(id: &str, at: [f32; 2]) -> Node {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let mut n = Node::new(id, "raster.read").param("path", data.to_string_lossy().as_ref());
    n.pos = Some(at);
    n
}

/// A wire dragged to empty canvas: the search, only of the operators that take its type, titled by what it is
/// for; typing narrows it, enter adds the one chosen where the wire was let go, connected, in one step of undo.
/// Escape and a click elsewhere add nothing.
#[test]
fn a_wire_let_go_on_empty_canvas_asks_for_an_operator_that_takes_it() {
    use egui_kittest::kittest::Queryable;
    let mut m = Model::new("wire");
    m.nodes.push(reader("read", [0.0, 0.0]));
    let mut h = harness(open(&m, "canvas-wire.toml"));
    h.run_steps(3);
    let from = editor(&h).screen_of_output("read", "raster").unwrap();
    let to = from + egui::vec2(260.0, 150.0);
    drag(&mut h, from, to);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.as_ref().is_some_and(|q| q.wire.is_some()), "the search opens where it was let go");
    assert_eq!(s.source.nodes.len(), 1, "nothing added yet");
    h.run_steps(2);
    assert!(h.query_by_label_contains("an operator that takes a raster").is_some(), "the title says what for");
    assert!(h.query_by_label_contains("raster.reclassify").is_some(), "an operator that takes a raster");
    assert!(h.query_by_label_contains("vector.join").is_none(), "no operator that takes only vectors and tables");
    // typing narrows it, enter adds the node there, connected and selected
    h.event(egui::Event::Text("reclass".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none(), "enter closes it");
    assert_eq!(s.source.nodes.len(), 2);
    let n = s.source.nodes.last().unwrap();
    assert_eq!(n.op, "raster.reclassify");
    assert_eq!(n.inputs["map"], terraflux_graph::Source::Ref("read.raster".into()));
    assert_eq!(s.selected.as_deref(), Some(n.id.as_str()));
    let (p, q) = (n.pos.unwrap(), editor(&h).graph_at(to).unwrap());
    assert!((p[0] - q.x).abs() < 1.0 && (p[1] - q.y).abs() < 1.0, "{p:?} at {q:?}");
    assert!(s.editor.message.is_none(), "{:?}", s.editor.message);
    // one step of undo takes both away
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), false);
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().source.nodes.len(), 1);
    // escape adds nothing
    let from = editor(&h).screen_of_output("read", "raster").unwrap();
    drag(&mut h, from, to);
    assert!(h.state().session.as_ref().unwrap().editor.quick.is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none());
    assert_eq!(s.source.nodes.len(), 1);
    // so does a click elsewhere
    drag(&mut h, from, to);
    assert!(h.state().session.as_ref().unwrap().editor.quick.is_some());
    click(&mut h, to + egui::vec2(-600.0, -300.0));
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none());
    assert_eq!(s.source.nodes.len(), 1);
    // a connection taken off an input only stays off: no search
    let mut m = Model::new("carried");
    m.nodes.push(reader("read", [0.0, 0.0]));
    let mut calc = Node::new("calc", "raster.calc").input("a", "read.raster").param("expression", "a");
    calc.pos = Some([400.0, 0.0]);
    m.nodes.push(calc);
    let mut h = harness(open(&m, "canvas-carried.toml"));
    h.run_steps(3);
    let from = editor(&h).screen_of_input("calc", "a").unwrap();
    drag(&mut h, from, from + egui::vec2(100.0, 200.0));
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none());
    assert!(s.source.node("calc").unwrap().inputs.is_empty(), "taken off");
}

/// Several nodes selected, their wires let go together: the node chosen takes them all (a stack.of: an input of
/// each), else the first fitting inputs, the wire dragged among them.
#[test]
fn wires_dragged_together_go_into_the_operator_chosen() {
    let mut m = Model::new("wires");
    for (k, id) in ["a", "b", "c"].iter().enumerate() {
        m.nodes.push(reader(id, [0.0, 160.0 * k as f32]));
    }
    let mut h = harness(open(&m, "canvas-wires.toml"));
    h.run_steps(3);
    let choose = |h: &mut Harness<TerrafluxApp>, ids: &[&str]| {
        h.state_mut().session.as_mut().unwrap().editor.chosen = ids.iter().map(|s| s.to_string()).collect();
        h.run_steps(2);
    };
    choose(&mut h, &["a", "b", "c"]);
    let from = editor(&h).screen_of_output("b", "raster").unwrap();
    drag(&mut h, from, from + egui::vec2(300.0, 40.0));
    h.event(egui::Event::Text("stack.of".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.last().unwrap();
    assert_eq!(n.op, "stack.of");
    for id in ["a", "b", "c"] {
        assert_eq!(n.inputs[id], terraflux_graph::Source::Ref(format!("{id}.raster")), "{:?}", n.inputs);
    }
    assert_eq!(n.inputs.len(), 3);
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), false);
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().source.nodes.len(), 3);
    // an operator with map inputs of its own (raster.calc): the wire dragged and another, in two of them
    choose(&mut h, &["a", "b", "c"]);
    let from = editor(&h).screen_of_output("c", "raster").unwrap();
    drag(&mut h, from, from + egui::vec2(300.0, 40.0));
    h.event(egui::Event::Text("raster.calc".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.last().unwrap();
    assert_eq!(n.op, "raster.calc");
    let refs: Vec<String> = n
        .inputs
        .values()
        .filter_map(|v| match v {
            terraflux_graph::Source::Ref(r) => Some(r.clone()),
            _ => None,
        })
        .collect();
    assert!(refs.contains(&"c.raster".to_string()), "the wire dragged is connected: {:?}", n.inputs);
    assert!(refs.len() >= 2, "{:?}", n.inputs);
    assert!(s.editor.message.is_none(), "{:?}", s.editor.message);
}

/// Inside a group's body the same: the node is added in that scope, connected to what is there.
#[test]
fn a_wire_let_go_inside_a_group_adds_the_node_to_its_body() {
    use terraflux_graph::Body;
    let mut m = Model::new("inside");
    let body = Body { nodes: vec![reader("r", [0.0, 0.0])], ..Default::default() };
    m.nodes.push(Node::new("grp", terraflux_graph::GROUP).body(body));
    let mut h = harness(open(&m, "canvas-inside.toml"));
    h.run_steps(3);
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
    h.run_steps(40);
    let from = editor(&h).screen_of_output("r", "raster").unwrap();
    drag(&mut h, from, from + egui::vec2(260.0, 120.0));
    h.event(egui::Event::Text("reclass".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes.len(), 1, "the top level is as it was");
    let body = s.source.scope(&["grp".to_string()]).unwrap();
    assert_eq!(body.len(), 2, "{body:?}");
    let n = &body[1];
    assert_eq!(n.op, "raster.reclassify");
    assert_eq!(n.inputs["map"], terraflux_graph::Source::Ref("r.raster".into()));
    assert!(s.editor.message.is_none(), "{:?}", s.editor.message);
}

/// `read -> mspa` with a free node (`op`) under the connection, and the screen points to drag it by (its header)
/// and to (its box centred on the connection).
fn over_a_wire(
    name: &str,
    op: &str,
    inside: bool,
    wired: bool,
) -> (Harness<'static, TerrafluxApp>, egui::Pos2, egui::Pos2) {
    use terraflux_graph::Body;
    let mspa = Node::new("mspa", "landscape.mspa").input("map", "read.raster");
    let mut mspa = mspa;
    mspa.pos = Some([700.0, 0.0]);
    let mut free = Node::new("free", op);
    free.pos = Some([350.0, 300.0]);
    if wired {
        free = free.input("map", "read.raster");
    }
    let mut nodes = vec![reader("read", [0.0, 0.0]), mspa, free];
    let mut m = Model::new("over");
    if inside {
        m.nodes.push(Node::new("grp", terraflux_graph::GROUP).body(Body { nodes, ..Default::default() }));
    } else {
        m.nodes.append(&mut nodes);
    }
    let mut h = harness(open(&m, name));
    h.run_steps(3);
    if inside {
        h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
        h.run_steps(40);
    }
    let e = editor(&h);
    let (a, b) = (e.screen_of_output("read", "raster").unwrap(), e.screen_of_input("mspa", "map").unwrap());
    let r = e.screen_rect_of("free").unwrap();
    let grab = e.screen_of("free").unwrap();
    (h, grab, grab + ((a + b.to_vec2()) / 2.0 - r.center()))
}

fn nodes_now(h: &Harness<TerrafluxApp>, inside: bool) -> Vec<Node> {
    let s = h.state().session.as_ref().unwrap();
    s.source.scope(&if inside { vec!["grp".to_string()] } else { vec![] }).unwrap().to_vec()
}

fn undo(h: &mut Harness<TerrafluxApp>) {
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), false);
    h.run_steps(2);
}

/// A node dragged over a connection and let go goes in the middle of it, by the input and output that fit, in one
/// step of undo; refused when its types do not fit, when it is wired already, and inside a group it is the same.
#[test]
fn a_node_let_go_over_a_connection_goes_in_the_middle() {
    use terraflux_graph::Source;
    let find = |v: &[Node], id: &str| v.iter().find(|n| n.id == id).cloned().unwrap();
    for inside in [false, true] {
        let (mut h, from, to) = over_a_wire("canvas-insert.toml", "raster.reclassify", inside, false);
        drag(&mut h, from, to);
        let v = nodes_now(&h, inside);
        assert_eq!(find(&v, "free").inputs["map"], Source::Ref("read.raster".into()), "inside {inside}: {v:?}");
        assert_eq!(find(&v, "mspa").inputs["map"], Source::Ref("free.map".into()), "inside {inside}");
        assert!(h.state().session.as_ref().unwrap().editor.message.is_none());
        // one step of undo puts the connection back
        undo(&mut h);
        let v = nodes_now(&h, inside);
        assert_eq!(find(&v, "mspa").inputs["map"], Source::Ref("read.raster".into()), "inside {inside}");
        assert!(find(&v, "free").inputs.is_empty(), "inside {inside}");
    }
    // a node of vectors does not fit a connection of maps
    let (mut h, from, to) = over_a_wire("canvas-insert-vector.toml", "vector.buffer", false, false);
    drag(&mut h, from, to);
    let v = nodes_now(&h, false);
    assert_eq!(find(&v, "mspa").inputs["map"], Source::Ref("read.raster".into()));
    assert!(find(&v, "free").inputs.is_empty());
    // a node that reads something at the input that fits is only moved
    let (mut h, from, to) = over_a_wire("canvas-insert-wired.toml", "raster.reclassify", false, true);
    drag(&mut h, from, to);
    let v = nodes_now(&h, false);
    assert_eq!(find(&v, "mspa").inputs["map"], Source::Ref("read.raster".into()));
    assert_eq!(find(&v, "free").inputs["map"], Source::Ref("read.raster".into()));
}

/// A node dragged with alt is lifted out of its connections: what it read feeds what it fed, in one step of undo;
/// what its types leave unconnected is named in the message.
#[test]
fn a_node_dragged_with_alt_is_lifted_out_of_its_connections() {
    use terraflux_graph::Source;
    let find = |v: &[Node], id: &str| v.iter().find(|n| n.id == id).cloned().unwrap();
    let alt = |h: &mut Harness<TerrafluxApp>, on: bool| {
        let m = if on { egui::Modifiers::ALT } else { egui::Modifiers::NONE };
        h.event(egui::Event::ModifiersChanged(m));
        h.run_steps(1);
    };
    for inside in [false, true] {
        let mut m = Model::new("lift");
        let (mut mspa, mut mid) = (Node::new("mspa", "landscape.mspa"), Node::new("mid", "raster.reclassify"));
        mid = mid.input("map", "read.raster");
        mspa = mspa.input("map", "mid.map");
        (mid.pos, mspa.pos) = (Some([350.0, 0.0]), Some([700.0, 0.0]));
        let nodes = vec![reader("read", [0.0, 0.0]), mid, mspa];
        if inside {
            let body = terraflux_graph::Body { nodes, ..Default::default() };
            m.nodes.push(Node::new("grp", terraflux_graph::GROUP).body(body));
        } else {
            m.nodes = nodes;
        }
        let mut h = harness(open(&m, "canvas-lift.toml"));
        h.run_steps(3);
        if inside {
            h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
            h.run_steps(40);
        }
        let from = editor(&h).screen_of("mid").unwrap();
        alt(&mut h, true);
        drag(&mut h, from, from + egui::vec2(0.0, 300.0));
        alt(&mut h, false);
        let v = nodes_now(&h, inside);
        assert_eq!(find(&v, "mspa").inputs["map"], Source::Ref("read.raster".into()), "inside {inside}: {v:?}");
        assert!(find(&v, "mid").inputs.is_empty(), "inside {inside}");
        let moved = find(&v, "mid").pos.unwrap();
        assert!(moved[1] > 150.0, "it stays where it was dropped: {moved:?}");
        assert!(h.state().session.as_ref().unwrap().editor.message.is_none());
        undo(&mut h);
        let v = nodes_now(&h, inside);
        assert_eq!(find(&v, "mspa").inputs["map"], Source::Ref("mid.map".into()), "inside {inside}");
        assert_eq!(find(&v, "mid").inputs["map"], Source::Ref("read.raster".into()), "inside {inside}");
    }
}

/// `top` and a group `grp` holding `inner` (at the far side of its scope) and a second group `deep` with `core`.
fn finding_model() -> Model {
    use terraflux_graph::Body;
    let mut m = Model::new("finding");
    m.nodes.push(reader("top", [0.0, 0.0]));
    let deep = Node::new("deep", terraflux_graph::GROUP)
        .body(Body { nodes: vec![reader("core", [0.0, 0.0])], ..Default::default() });
    let body = Body { nodes: vec![reader("inner", [0.0, 0.0]), deep], ..Default::default() };
    m.nodes.push(Node::new("grp", terraflux_graph::GROUP).body(body));
    m
}

/// The canvas's centre is inside a node's box.
fn centred_on(h: &Harness<TerrafluxApp>, id: &str) -> bool {
    let e = editor(h);
    e.screen_rect_of(id).is_some_and(|r| r.contains(e.canvas().unwrap().center()))
}

/// Ctrl+F over the canvas: typing finds nodes at every depth by id, operator or file, with their place; enter
/// selects the one chosen, opens its groups and centres the camera on it; escape changes nothing; the palette's
/// *Go to group/inner* does the same.
#[test]
fn a_node_of_the_model_is_found_inside_groups_and_the_camera_goes_to_it() {
    use egui_kittest::kittest::Queryable;
    let mut h = harness(open(&finding_model(), "canvas-find.toml"));
    h.run_steps(3);
    let centre = editor(&h).canvas().unwrap().center();
    h.hover_at(centre);
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    h.run_steps(2);
    assert!(editor(&h).find.is_some(), "ctrl+F opens the search");
    h.event(egui::Event::Text("inne".into()));
    h.run_steps(2);
    assert!(h.query_by_label_contains("inner · raster.read").is_some(), "the node, with its operator");
    assert!(h.query_by_label_contains("grp > inner").is_some(), "and its place");
    assert!(h.query_by_label_contains("top · raster.read").is_none(), "the others are not listed");
    h.key_press(egui::Key::Enter);
    h.run_steps(40);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.find.is_none(), "enter closes it");
    assert_eq!(s.editor.path, ["grp"]);
    assert_eq!(s.selected.as_deref(), Some("inner"));
    assert!(s.editor.chosen.contains("inner"));
    assert!(centred_on(&h, "inner"), "{:?}", editor(&h).screen_rect_of("inner"));
    // a node of a group two levels in: the scope set directly
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    h.run_steps(2);
    h.event(egui::Event::Text("core".into()));
    h.run_steps(2);
    assert!(h.query_by_label_contains("grp > deep > core").is_some());
    h.key_press(egui::Key::Enter);
    h.run_steps(40);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.editor.path, ["grp", "deep"]);
    assert_eq!(s.selected.as_deref(), Some("core"));
    assert!(centred_on(&h, "core"));
    // by the file's name: the node at the top, from inside a group
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    h.run_steps(2);
    h.event(egui::Event::Text("SIBUYAN_land".into()));
    h.run_steps(2);
    assert_eq!(h.query_all_by_label_contains("sibuyan_landuse.tif").count(), 3, "every reader, the file shown");
    h.key_press(egui::Key::Enter);
    h.run_steps(40);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.find.is_none());
    assert_eq!(s.selected.as_deref(), Some("top"), "the first of the list: {:?}", s.selected);
    assert!(s.editor.path.is_empty());
    assert!(centred_on(&h, "top"));
    // escape closes it, changing nothing
    let before = (editor(&h).path.clone(), h.state().session.as_ref().unwrap().selected.clone());
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::F);
    h.run_steps(2);
    h.event(egui::Event::Text("core".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.find.is_none());
    assert_eq!((s.editor.path.clone(), s.selected.clone()), before);
    // the palette: Go to group/inner
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    h.run_steps(2);
    h.event(egui::Event::Text("go to grp/inner".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(40);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.editor.path, ["grp"]);
    assert_eq!(s.selected.as_deref(), Some("inner"));
    assert!(centred_on(&h, "inner"));
}

/// Frames: F over the canvas fits the nodes selected (their boxes whole on the canvas, closer than Home); nothing
/// selected, it is Home; a lone small node is not zoomed in past 1.75; inside a group it frames within the scope.
#[test]
fn f_frames_the_nodes_selected_and_home_the_whole_model() {
    use terraflux_graph::Body;
    let inside = |h: &Harness<TerrafluxApp>, ids: &[&str]| {
        let e = editor(h);
        let canvas = e.canvas().unwrap();
        ids.iter().all(|id| e.screen_rect_of(id).is_some_and(|r| canvas.contains_rect(r)))
    };
    let choose = |h: &mut Harness<TerrafluxApp>, ids: &[&str]| {
        let s = h.state_mut().session.as_mut().unwrap();
        s.selected = ids.first().map(|s| s.to_string());
        s.editor.chosen = ids.iter().map(|s| s.to_string()).collect();
        h.run_steps(2);
    };
    let press = |h: &mut Harness<TerrafluxApp>, key: egui::Key| {
        h.key_press(key);
        h.run_steps(40);
    };
    let mut h = harness(open(&graph(200, true), "canvas-frame.toml"));
    h.run_steps(3);
    let centre = editor(&h).canvas().unwrap().center();
    h.hover_at(centre);
    h.run_steps(2);
    press(&mut h, egui::Key::Home);
    let (home_zoom, home_at) = (editor(&h).zoom(), editor(&h).screen_of("n3"));
    choose(&mut h, &["n0", "n10"]);
    press(&mut h, egui::Key::F);
    assert!(inside(&h, &["n0", "n10"]), "{:?} {:?}", editor(&h).screen_rect_of("n0"), editor(&h).screen_rect_of("n10"));
    assert!(editor(&h).zoom() > home_zoom * 2.0, "{} against {home_zoom}", editor(&h).zoom());
    // a lone node: not closer than 1.75
    choose(&mut h, &["n5"]);
    press(&mut h, egui::Key::F);
    assert!(editor(&h).zoom() <= 1.75 + 1e-3 && editor(&h).zoom() > home_zoom);
    assert!(inside(&h, &["n5"]));
    // nothing selected: the whole graph, as Home did
    let s = h.state_mut().session.as_mut().unwrap();
    s.selected = None;
    s.editor.chosen.clear();
    h.run_steps(2);
    press(&mut h, egui::Key::F);
    assert!((editor(&h).zoom() - home_zoom).abs() < 1e-4);
    let (a, b) = (editor(&h).screen_of("n3").unwrap(), home_at.unwrap());
    assert!((a - b).length() < 1.0, "{a:?} {b:?}");
    // inside a group
    let mut m = Model::new("frame-in");
    let body = Body {
        nodes: vec![reader("a", [0.0, 0.0]), reader("b", [3000.0, 0.0]), reader("c", [0.0, 1800.0])],
        ..Default::default()
    };
    m.nodes.push(Node::new("grp", terraflux_graph::GROUP).body(body));
    let mut h = harness(open(&m, "canvas-frame-in.toml"));
    h.run_steps(3);
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
    h.run_steps(40);
    let centre = editor(&h).canvas().unwrap().center();
    h.hover_at(centre);
    h.run_steps(2);
    let home_zoom = editor(&h).zoom();
    choose(&mut h, &["a", "c"]);
    press(&mut h, egui::Key::F);
    assert!(inside(&h, &["a", "c"]));
    assert!(editor(&h).zoom() > home_zoom, "{} against {home_zoom}", editor(&h).zoom());
    assert_eq!(editor(&h).path, ["grp"]);
}

/// A right click (press and release of the secondary button) at a point.
fn right_click(h: &mut Harness<TerrafluxApp>, at: egui::Pos2) {
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(1);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(1);
    }
    h.run_steps(2);
}

/// `a -> b -> c` (a chain) and a free `z`, as the selection's menu finds them.
fn selection_model() -> Model {
    let mut m = Model::new("many");
    let mut b = Node::new("b", "raster.reclassify").input("map", "a.raster");
    let mut c = Node::new("c", "landscape.mspa").input("map", "b.map");
    (b.pos, c.pos) = (Some([350.0, 0.0]), Some([700.0, 0.0]));
    m.nodes = vec![reader("a", [0.0, 0.0]), b, c, reader("z", [0.0, 300.0])];
    m
}

/// Select `ids` (the first one in the inspector), right click `on`, the menu open.
fn menu_of(h: &mut Harness<TerrafluxApp>, ids: &[&str], on: &str) {
    let s = h.state_mut().session.as_mut().unwrap();
    s.selected = ids.first().map(|s| s.to_string());
    s.editor.chosen = ids.iter().map(|s| s.to_string()).collect();
    h.run_steps(2);
    let at = editor(h).screen_of(on).unwrap();
    right_click(h, at);
}

fn pick(h: &mut Harness<TerrafluxApp>, label: &str) {
    use egui_kittest::kittest::Queryable;
    h.get_by_label_contains(label).click();
    h.run_steps(3);
}

/// Right click with several nodes selected: the selection's menu, headed by their number (a right click on a
/// node outside them keeps the node's own menu). Duplicate copies them with the links among them, selected;
/// delete, disconnect, freeze and group are each one step of undo; run to them runs the last ones one by one.
#[test]
fn the_selections_menu_acts_on_all_the_nodes_selected() {
    use egui_kittest::kittest::Queryable;
    use terraflux_graph::Source;
    let mut h = harness(open(&selection_model(), "canvas-selection-menu.toml"));
    h.run_steps(3);
    let count = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().source.nodes.len();
    // the heading (the bar under the canvas says it too) and the entries
    menu_of(&mut h, &["a", "b", "c"], "b");
    assert_eq!(h.query_all_by_label("3 nodes selected").count(), 2, "the bar's and the menu's");
    for l in ["Delete", "Duplicate    ", "Disconnect all their inputs", "Group    ", "Run to them", "Freeze", "Bypass"]
    {
        assert!(h.query_by_label_contains(l).is_some(), "{l}");
    }
    // a node outside them: its own menu, no heading
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    menu_of(&mut h, &["a", "b", "c"], "z");
    assert!(h.query_by_label_contains("Run to them").is_none());
    assert!(h.query_by_label_contains("Run on a folder").is_some(), "one node's menu");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    // duplicate: three copies, linked among themselves, selected, one step of undo
    menu_of(&mut h, &["a", "b", "c"], "c");
    pick(&mut h, "Duplicate    ");
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes.len(), 7);
    let (a2, b2) = (s.source.node("a_2").unwrap(), s.source.node("b_2").unwrap());
    assert!(a2.inputs.is_empty());
    assert_eq!(b2.inputs["map"], Source::Ref("a_2.raster".into()));
    assert_eq!(s.source.node("c_2").unwrap().inputs["map"], Source::Ref("b_2.map".into()));
    let want: std::collections::BTreeSet<String> = ["a_2", "b_2", "c_2"].map(String::from).into();
    assert_eq!(s.editor.chosen, want);
    undo(&mut h);
    assert_eq!(count(&h), 4);
    // delete: three nodes, one step of undo
    menu_of(&mut h, &["a", "b", "c"], "a");
    pick(&mut h, "Delete");
    assert_eq!(count(&h), 1);
    assert!(h.state().session.as_ref().unwrap().editor.chosen.is_empty());
    undo(&mut h);
    assert_eq!(count(&h), 4);
    // the background of the canvas with them selected: the same menu
    // (escape would let the nodes selected go first)
    h.state_mut().session.as_mut().unwrap().editor.chosen = ["b", "c"].map(String::from).into();
    h.run_steps(2);
    let canvas = editor(&h).canvas().unwrap();
    right_click(&mut h, canvas.left_bottom() + egui::vec2(60.0, -60.0));
    assert!(h.query_by_label_contains("Run to them").is_some(), "the background's menu");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    // disconnect their inputs (b reads a, c reads b), one step of undo
    menu_of(&mut h, &["b", "c"], "b");
    pick(&mut h, "Disconnect all their inputs");
    let s = h.state().session.as_ref().unwrap();
    assert!(s.source.node("b").unwrap().inputs.is_empty() && s.source.node("c").unwrap().inputs.is_empty());
    undo(&mut h);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.node("b").unwrap().inputs["map"], Source::Ref("a.raster".into()));
    assert_eq!(s.source.node("c").unwrap().inputs["map"], Source::Ref("b.map".into()));
    // freeze toggles them all (a, then all frozen: unfreeze)
    menu_of(&mut h, &["a", "b", "z"], "a");
    pick(&mut h, "Freeze");
    let frozen = |h: &Harness<TerrafluxApp>| {
        let s = h.state().session.as_ref().unwrap();
        ["a", "b", "c", "z"].map(|id| s.source.node(id).unwrap().frozen)
    };
    assert_eq!(frozen(&h), [true, true, false, true]);
    menu_of(&mut h, &["a", "b", "z"], "a");
    pick(&mut h, "Unfreeze");
    assert_eq!(frozen(&h), [false; 4]);
    // group: one group of the three
    menu_of(&mut h, &["a", "b", "c"], "b");
    pick(&mut h, "Group    ");
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes.len(), 2, "{:?}", s.source.nodes.iter().map(|n| &n.id).collect::<Vec<_>>());
    assert_eq!(s.source.nodes.iter().filter(|n| n.op == terraflux_graph::GROUP).count(), 1);
    undo(&mut h);
    assert_eq!(count(&h), 4);
    // run to them: the two that nothing else selected needs, one after the other (b and c: only c)
    menu_of(&mut h, &["a", "c", "z"], "a");
    pick(&mut h, "Run to them");
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.target.as_deref(), Some("c"), "{:?}", s.run_queue);
    assert_eq!(s.run_queue, ["z"]);
}

/// `selection_model`'s nodes at the top level, or in a group `grp` that is shown.
fn scoped_model(inside: bool, name: &str) -> Harness<'static, TerrafluxApp> {
    let mut m = selection_model();
    if inside {
        let nodes = std::mem::take(&mut m.nodes);
        m.nodes
            .push(Node::new("grp", terraflux_graph::GROUP).body(terraflux_graph::Body { nodes, ..Default::default() }));
    }
    let mut h = harness(open(&m, name));
    h.run_steps(3);
    if inside {
        h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
        h.run_steps(40);
    }
    h
}

fn holding(h: &mut Harness<TerrafluxApp>, m: egui::Modifiers) {
    h.event(egui::Event::ModifiersChanged(m));
    h.run_steps(1);
}

fn chosen_now(h: &Harness<TerrafluxApp>) -> Vec<String> {
    editor(h).chosen.iter().cloned().collect()
}

/// Ctrl+drag copies the nodes: outlines follow the pointer, the originals stay, and where it is let go the copies
/// appear (the links among them kept, what they read kept), selected, in one step of undo; escape during the drag
/// makes none; a ctrl+click, no drag, still toggles the selection. In a group too.
#[test]
fn nodes_dragged_with_ctrl_are_copied() {
    use terraflux_graph::Source;
    let find = |v: &[Node], id: &str| v.iter().find(|n| n.id == id).cloned();
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-copy-drag.toml");
        let select = |h: &mut Harness<TerrafluxApp>, ids: &[&str]| {
            let s = h.state_mut().session.as_mut().unwrap();
            s.selected = ids.first().map(|s| s.to_string());
            s.editor.chosen = ids.iter().map(|s| s.to_string()).collect();
            h.run_steps(2);
        };
        // one node, selected or not: a copy at the drop point
        select(&mut h, &[]);
        let (from, to) =
            (editor(&h).screen_of("z").unwrap(), editor(&h).screen_of("z").unwrap() + egui::vec2(220.0, 120.0));
        let by = editor(&h).graph_at(to).unwrap() - editor(&h).graph_at(from).unwrap();
        holding(&mut h, egui::Modifiers::COMMAND);
        drag(&mut h, from, to);
        holding(&mut h, egui::Modifiers::NONE);
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), 5, "inside {inside}: {v:?}");
        assert_eq!(find(&v, "z").unwrap().pos, Some([0.0, 300.0]), "the original stays");
        let p = find(&v, "z_2").expect("the copy").pos.unwrap();
        assert!((p[0] - by.x).abs() < 1.0 && (p[1] - 300.0 - by.y).abs() < 1.0, "{p:?} by {by:?}");
        assert_eq!(chosen_now(&h), ["z_2"]);
        undo(&mut h);
        assert_eq!(nodes_now(&h, inside).len(), 4, "one step of undo");
        // two linked nodes selected: both copied, the copy of the second reads the copy of the first
        select(&mut h, &["b", "c"]);
        let from = editor(&h).screen_of("b").unwrap();
        holding(&mut h, egui::Modifiers::COMMAND);
        drag(&mut h, from, from + egui::vec2(0.0, 200.0));
        holding(&mut h, egui::Modifiers::NONE);
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), 6, "inside {inside}");
        assert_eq!(find(&v, "b").unwrap().pos, Some([350.0, 0.0]));
        assert_eq!(find(&v, "b_2").unwrap().inputs["map"], Source::Ref("a.raster".into()));
        assert_eq!(find(&v, "c_2").unwrap().inputs["map"], Source::Ref("b_2.map".into()));
        assert_eq!(chosen_now(&h), ["b_2", "c_2"]);
        undo(&mut h);
        assert_eq!(nodes_now(&h, inside).len(), 4);
        // escape during the drag: nothing copied
        select(&mut h, &["z"]);
        let from = editor(&h).screen_of("z").unwrap();
        holding(&mut h, egui::Modifiers::COMMAND);
        h.hover_at(from);
        h.run_steps(1);
        h.drag_at(from);
        h.run_steps(1);
        for k in 1..=4 {
            h.hover_at(from + egui::vec2(40.0 * k as f32, 20.0 * k as f32));
            h.run_steps(1);
        }
        h.key_press(egui::Key::Escape);
        h.run_steps(1);
        h.drop_at(from + egui::vec2(160.0, 80.0));
        h.run_steps(2);
        holding(&mut h, egui::Modifiers::NONE);
        assert_eq!(nodes_now(&h, inside).len(), 4, "escape: no copies");
        // a ctrl+click toggles the selection, copies nothing
        select(&mut h, &["a"]);
        let at = editor(&h).screen_of("b").unwrap();
        holding(&mut h, egui::Modifiers::COMMAND);
        click(&mut h, at);
        holding(&mut h, egui::Modifiers::NONE);
        assert_eq!(chosen_now(&h), ["a", "b"], "inside {inside}");
        assert_eq!(nodes_now(&h, inside).len(), 4);
    }
}

/// Ctrl+D duplicates with the inputs: the copy reads what the original read, the links among the nodes copied go
/// to the copies; so do the entries in the menus. Inside a group too.
#[test]
fn a_duplicate_reads_what_the_original_read() {
    use terraflux_graph::Source;
    let find = |v: &[Node], id: &str| v.iter().find(|n| n.id == id).cloned().unwrap();
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-copy-inputs.toml");
        let select = |h: &mut Harness<TerrafluxApp>, ids: &[&str]| {
            let s = h.state_mut().session.as_mut().unwrap();
            s.selected = ids.first().map(|s| s.to_string());
            s.editor.chosen = ids.iter().map(|s| s.to_string()).collect();
            h.run_steps(2);
            let centre = editor(h).canvas().unwrap().center();
            h.hover_at(centre);
            h.run_steps(2);
        };
        let press = |h: &mut Harness<TerrafluxApp>| {
            h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
            h.run_steps(3);
        };
        select(&mut h, &["b"]);
        press(&mut h);
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), 5, "inside {inside}");
        assert_eq!(find(&v, "b_2").inputs["map"], Source::Ref("a.raster".into()));
        assert_eq!(chosen_now(&h), ["b_2"]);
        undo(&mut h);
        assert_eq!(nodes_now(&h, inside).len(), 4);
        select(&mut h, &["b", "c"]);
        press(&mut h);
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), 6);
        assert_eq!(find(&v, "b_2").inputs["map"], Source::Ref("a.raster".into()));
        assert_eq!(find(&v, "c_2").inputs["map"], Source::Ref("b_2.map".into()));
        assert_eq!(find(&v, "c").inputs["map"], Source::Ref("b.map".into()), "the originals as they were");
        undo(&mut h);
        // the entries of the one-node menu and of the selection's
        select(&mut h, &["z"]);
        let at = editor(&h).screen_of("c").unwrap();
        right_click(&mut h, at);
        pick(&mut h, "Duplicate    ");
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), 5);
        assert_eq!(find(&v, "c_2").inputs["map"], Source::Ref("b.map".into()), "the copy reads what c read");
        undo(&mut h);
        menu_of(&mut h, &["b", "c"], "b");
        pick(&mut h, "Duplicate    ");
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), 6);
        assert_eq!(find(&v, "c_2").inputs["map"], Source::Ref("b_2.map".into()));
    }
}
