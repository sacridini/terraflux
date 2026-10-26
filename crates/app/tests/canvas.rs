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
    // a double click on a node: its map in a tab of its own, run first
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
    assert_eq!(s.map_tabs.len(), 1);
    let (id, tab) = s.map_tabs.iter().next().unwrap();
    assert_eq!((tab.node.as_str(), tab.port.as_str()), ("read", "raster"));
    assert!(s.value_of("read", "raster").is_some());
    assert!(h.state().is_open(terraflux_app::app::Pane::Map(*id)));
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
    // files dropped on the window (a raster and vector features; a folder is left out): a node that reads each
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
    assert_eq!(read, [("raster.read", tif.as_str()), ("vector.read", geojson.to_str().unwrap())]);
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
    assert!(app.is_open(Pane::Node), "the layout read");
    let dock = app.dock.as_ref().unwrap();
    let at = dock.find_tab(&Pane::Model).unwrap();
    let leaf = dock.leaf(at.node_path()).unwrap();
    assert_eq!(leaf.tabs().get(leaf.active.0), Some(&Pane::Model));
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
