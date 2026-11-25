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

/// The settings are global (one per process): a test that changes one (the minimap, the wires around boxes), or
/// reads what depends on them (the wires drawn), holds this while it runs, so no other test sees them half changed.
static SETTINGS: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn settings_lock() -> std::sync::MutexGuard<'static, ()> {
    let guard = SETTINGS.lock().unwrap_or_else(|e| e.into_inner());
    // (as every test of the canvas finds them: the minimap off, wires around boxes; a test sets what it tests)
    crate::point_settings();
    terraflux_app::settings::update(|s| {
        s.minimap = false;
        s.wires_around = true;
        s.node_pane_opens = false;
    });
    guard
}

/// An application without a model, its settings the tests' own (never the user's).
fn fresh() -> TerrafluxApp {
    crate::point_settings();
    TerrafluxApp::new(None)
}

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
    crate::point_settings();
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
        terraflux_graph::Input::Map {
            path: data.to_string_lossy().into_owned(),
            band: None,
            nodata: None,
            note: None,
            data: None,
        },
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
    let mut h = harness(fresh());
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

/// A double click brings the Viewer only for a node that has something to show, and only once it is there: a
/// random generator, a writer (its only output the path it wrote) and a node whose run fails do nothing at all; a
/// map is made, then shown.
#[test]
fn a_double_click_shows_the_viewer_only_with_something_in_it() {
    let _lock = settings_lock();
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-double-click-out.tif");
    let mut m = Model::new("double click");
    m.nodes.push(Node::new("gen", "rng.numpy").param("seed", 1));
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 20).param("columns", 20).param("seed", 3));
    m.nodes.push(Node::new("wr", "raster.write").input("raster", "land.map").param("path", out.to_string_lossy()));
    m.nodes.push(Node::new("bad", "raster.calc").input("a", "land.map").param("expression", "a +"));
    for (i, n) in m.nodes.iter_mut().enumerate() {
        n.pos = Some([(i % 2) as f32 * 320.0, (i / 2) as f32 * 220.0]);
    }
    let mut h = harness(open(&m, "canvas-double-click.toml"));
    h.run_steps(5);
    let viewer = |h: &mut Harness<TerrafluxApp>| h.state_mut().focused() == Some(Pane::Viewer);
    assert!(!viewer(&mut h));
    let settle = |h: &mut Harness<TerrafluxApp>| {
        let t = Instant::now();
        while h.state().session.as_ref().unwrap().running() {
            h.step();
            assert!(t.elapsed() < Duration::from_secs(30));
        }
        h.run_steps(5);
    };
    for id in ["gen", "wr", "bad"] {
        h.run_steps(60);
        let at = editor(&h).screen_of(id).unwrap();
        double_click(&mut h, at);
        settle(&mut h);
        assert!(!viewer(&mut h), "{id}: no Viewer");
        assert!(h.state().session.as_ref().unwrap().view_when_made.is_none(), "{id}: nothing waits to be shown");
    }
    // a map: made, then shown
    h.run_steps(60);
    let at = editor(&h).screen_of("land").unwrap();
    double_click(&mut h, at);
    settle(&mut h);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.value_of("land", "map").is_some());
    assert_eq!(s.preview, Some(("land".to_string(), "map".to_string())));
    assert!(viewer(&mut h), "the Viewer, with the map");
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
    let mut h = harness(fresh());
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
    let mut h = harness(fresh());
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    let dock = h.state().dock.clone().unwrap();
    let viewer = dock.find_tab(&Pane::Viewer).unwrap();
    assert_eq!(dock.find_tab(&Pane::Model).map(|p| p.node_path()), Some(viewer.node_path()));
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-layout.json");
    let text = serde_json::json!({ "version": 7, "main": dock, "windows": [] });
    std::fs::write(&path, text.to_string()).unwrap();
    let mut app = fresh();
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
    // (the pane that comes by itself is the one asked for in the settings)
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_pane_opens = true);
    let mut h = harness(fresh());
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
    // a template (of data): the node's pane, the viewer behind the graph
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(2);
    assert!(h.state().is_open(Pane::Node) && !h.state().is_open(Pane::Prepare) && h.state().is_open(Pane::Viewer));
    assert!(!h.state().is_open(Pane::Calculator), "the calculator only when a script is opened in it");
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
    // (on the header's height: lower down a box with settings has an output port near its middle)
    let edge = |h: &Harness<TerrafluxApp>, id: &str| {
        let r = editor(h).screen_rect_of(id).unwrap();
        egui::pos2(r.right(), r.top() + 28.0 * editor(h).zoom())
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
        terraflux_graph::Input::Map {
            path: data.to_string_lossy().into_owned(),
            band: None,
            nodata: None,
            note: None,
            data: None,
        },
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

/// Zoomed out under 0.6 a node is a card: 34 pixels tall at least, in the middle of its box, as wide as it; a click
/// on the card selects the node, one on the empty part of its box does not; its wires end at the card's sides.
#[test]
fn far_out_a_node_is_a_card_the_pointer_finds() {
    use egui::{Rect, pos2, vec2};
    use terraflux_app::editor_view::card;
    // the card: by arithmetic from the box and the zoom
    let big = Rect::from_min_size(pos2(10.0, 20.0), vec2(190.0, 300.0));
    let c = card(big, 0.5);
    assert_eq!((c.width(), c.height(), c.center()), (190.0, 68.0, big.center()), "34 pixels at 0.5");
    assert_eq!(card(big, 0.25).height(), 136.0);
    assert!((card(big, 0.59).height() - 34.0 / 0.59).abs() < 1e-3);
    assert_eq!(card(big, 0.3).left(), big.left());
    // (never taller than its box, never under a header's height)
    let small = Rect::from_min_size(pos2(0.0, 0.0), vec2(190.0, 50.0));
    assert_eq!(card(small, 0.25).height(), 50.0);
    assert_eq!(card(big, 0.9).height(), 38.0, "a header's height at least");

    let _lock = settings_lock();
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let mut m = Model::new("cards");
    m.inputs.insert(
        "t0".into(),
        terraflux_graph::Input::Map {
            path: data.to_string_lossy().into_owned(),
            band: None,
            nodata: None,
            note: None,
            data: None,
        },
    );
    let mut zon = Node::new("zon", "prioritize.zonation").input("features", "$t0");
    zon.pos = Some([0.0, 0.0]);
    let mut calc = Node::new("calc", "raster.calc").input("land", "zon.rank").param("expression", "land");
    calc.pos = Some([420.0, 0.0]);
    m.nodes.extend([zon, calc]);
    let mut h = harness(open(&m, "canvas-cards.toml"));
    h.run_steps(3);
    // whole at the zoom that fits: no card
    h.state_mut().session.as_mut().unwrap().editor.look(vec2(300.0, 300.0), 1.0);
    h.run_steps(3);
    assert!(editor(&h).screen_card_of("zon").is_none());
    h.state_mut().session.as_mut().unwrap().editor.look(vec2(300.0, 300.0), 0.5);
    h.run_steps(3);
    let (zoom, whole) = (editor(&h).zoom(), editor(&h).screen_rect_of("zon").unwrap());
    let card = editor(&h).screen_card_of("zon").expect("a card under 0.6");
    assert_eq!(zoom, 0.5);
    assert!(card.height() >= 34.0 - 0.01 && card.height() < whole.height() - 8.0, "{card:?} in {whole:?}");
    assert_eq!((card.left(), card.right(), card.center().y), (whole.left(), whole.right(), whole.center().y));
    // a wire ends at the middle of the card's left side (the node's input), the other's begins at its right
    let wire = editor(&h).wire("calc", "land").expect("the wire");
    let (end, begin) = (editor(&h).screen_at(*wire.last().unwrap()).unwrap(), editor(&h).screen_at(wire[0]).unwrap());
    assert!((begin - card.right_center()).length() < 0.5, "{begin:?} {card:?}");
    assert!((end - editor(&h).screen_card_of("calc").unwrap().left_center()).length() < 0.5);
    // on the empty part of its box (above the card, inside the box): the background
    let empty = pos2(card.center().x, (whole.top() + card.top()) / 2.0);
    assert!(whole.contains(empty) && !card.contains(empty));
    assert_eq!(editor(&h).node_at(empty), None);
    click(&mut h, empty);
    assert!(chosen_now(&h).is_empty(), "{:?}", chosen_now(&h));
    // on the card: selected
    click(&mut h, card.center());
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("zon"));
    assert_eq!(chosen_now(&h), ["zon"]);
    // zoomed in again: its box, as before (its middle is not its header)
    h.state_mut().session.as_mut().unwrap().editor.look(vec2(300.0, 300.0), 1.0);
    h.run_steps(3);
    assert!(editor(&h).screen_card_of("zon").is_none());
    let whole = editor(&h).screen_rect_of("zon").unwrap();
    assert!(editor(&h).node_at(pos2(whole.center().x, whole.top() + 19.0)).is_some());
}

/// A node's box shows its main settings in rows under its ports: those given first (a value, a default faint, a
/// reference as `← $name`), four at most, the others counted in a row (with how many were given) that unfolds
/// them; the box is taller by the rows; a setting the check has a problem with is marked; the preference turns
/// them off (the box as it was).
#[test]
fn a_nodes_box_shows_its_settings() {
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut m = Model::new("settings");
    let mut plan =
        Node::new("plan", "prioritize.optimize").param("gap", 0.2).param("seed", 3).param("time_limit", "$nothere");
    plan.pos = Some([0.0, 0.0]);
    m.nodes.push(plan);
    let mut many = Node::new("many", "prioritize.optimize");
    for (k, v) in [("gap", 0.2), ("seed", 3.0), ("pool_gap", 0.1), ("number_solutions", 5.0), ("edge_factor", 0.5)] {
        many = many.param(k, v);
    }
    many.pos = Some([500.0, 0.0]);
    m.nodes.push(many);
    let mut h = harness(open(&m, "canvas-settings.toml"));
    h.state_mut().session.as_mut().unwrap().editor.fit();
    h.run_steps(4);
    let rows = |h: &Harness<TerrafluxApp>, id: &str| editor(h).settings_of(id).unwrap().clone();
    let height = |h: &Harness<TerrafluxApp>, id: &str| {
        let r = editor(h).screen_rect_of(id).unwrap();
        r.height() / editor(h).zoom()
    };
    let plan = rows(&h, "plan");
    assert_eq!(plan.rows.len(), 4, "{plan:?}");
    assert!(plan.rows[..3].iter().all(|r| r.given), "the given ones first: {plan:?}");
    let gap = plan.rows.iter().find(|r| r.name == "gap").expect("gap");
    assert_eq!(gap.value, "0.2");
    let limit = plan.rows.iter().find(|r| r.name == "time_limit").expect("time_limit");
    assert_eq!(limit.value, "← $nothere");
    assert!(limit.reads && limit.problem == Some(true), "the check's error marks its row: {limit:?}");
    assert!(plan.rows.iter().all(|r| r.name == "gap" || r.name == "time_limit" || r.problem.is_none()));
    let default = plan.rows.iter().find(|r| !r.given).expect("an essential not given");
    assert!(!default.reads && default.problem.is_none());
    assert!(plan.folded > 0 && plan.folded_given == 0 && !plan.open);
    assert_eq!(plan.label, format!("+ {} settings", plan.folded));
    let many_rows = rows(&h, "many");
    assert_eq!(many_rows.rows.len(), 4);
    assert_eq!((many_rows.folded_given, many_rows.folded > many_rows.folded_given), (1, true));
    assert!(many_rows.label.ends_with("· 1 given"), "{}", many_rows.label);
    // the box is taller by the rows and the line over them (the row that unfolds the others counted)
    let (tall, before) = (height(&h, "plan"), height(&h, "many"));
    terraflux_app::settings::update(|s| s.node_settings = false);
    h.run_steps(3);
    assert!(editor(&h).settings_of("plan").unwrap().rows.is_empty(), "off: no rows");
    let (short, short_many) = (height(&h, "plan"), height(&h, "many"));
    assert!((tall - short - (6.0 + 18.0 * 5.0)).abs() < 0.5, "{tall} on, {short} off");
    assert!((before - short_many - (6.0 + 18.0 * 5.0)).abs() < 0.5, "{before} on, {short_many} off");
    terraflux_app::settings::update(|s| s.node_settings = true);
    h.run_steps(3);
    // a click on the row that counts the others unfolds them all (and the box is taller), another folds them
    let more = editor(&h).screen_of_more_settings("plan").expect("the row that unfolds");
    click(&mut h, more);
    let open = rows(&h, "plan");
    assert!(open.open && open.rows.len() > 4 && open.label == "- fewer", "{open:?}");
    assert!(height(&h, "plan") > tall + 17.0);
    h.render().unwrap().save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-settings.png")).unwrap();
    // (zoomed far out the node is a card, its rows not there to click: a click on it selects the node)
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(200.0, 80.0), 0.3);
    h.run_steps(2);
    let card = editor(&h).screen_card_of("plan").expect("a card");
    click(&mut h, card.center());
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("plan"));
    // (from 0.6 the rows are there again: the row of an unfolded operator of many settings is far below)
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(200.0, 80.0), 0.6);
    h.run_steps(2);
    let more = editor(&h).screen_of_more_settings("plan").unwrap();
    click(&mut h, more);
    assert_eq!(rows(&h, "plan").rows.len(), 4, "folded again");
    // a click on a row selects the node as a click on the box does
    let row = editor(&h).screen_of_setting("many", "edge_factor").expect("a row");
    click(&mut h, row);
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("many"));
}

/// A node's setting as the model has it.
fn param_now(h: &Harness<TerrafluxApp>, id: &str, name: &str) -> Option<serde_json::Value> {
    nodes_now(h, false).iter().find(|n| n.id == id).and_then(|n| n.params.get(name).cloned())
}

/// Where a node's row of a setting is, once the layout has settled (the side panel follows the node selected).
fn row_of(h: &mut Harness<TerrafluxApp>, node: &str, name: &str) -> egui::Pos2 {
    h.run_steps(12);
    editor(h).screen_of_setting(node, name).expect("a row")
}

/// Nodes with a yes or no (`batch.run`'s overwrite), a choice and numbers (`prioritize.optimize`'s solver and gap)
/// and a script (`raster.calc`'s expression), their boxes at a readable zoom.
fn settings_harness(name: &str) -> Harness<'static, TerrafluxApp> {
    let mut m = Model::new("edit");
    let mut opt = Node::new("opt", "prioritize.optimize").param("gap", 0.2).param("solver", "highs");
    opt.pos = Some([0.0, 0.0]);
    let mut job = Node::new("job", "batch.run").param("operator", "landscape.mspa").param("overwrite", true);
    job.pos = Some([400.0, 0.0]);
    let mut calc = Node::new("calc", "raster.calc").param("expression", "out r = 1");
    calc.pos = Some([400.0, 300.0]);
    let body = terraflux_graph::Body { nodes: vec![Node::new("inner", "nlm.random")], ..Default::default() };
    let mut group = Node::new("grp", terraflux_graph::GROUP).body(body);
    group.pos = Some([0.0, 600.0]);
    m.nodes = vec![opt, job, calc, group];
    let mut h = harness(open(&m, name));
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(20.0, 20.0), 0.8);
    h.run_steps(4);
    h
}

/// A click on a yes or no turns it over at once; on a choice it opens the list under the row, and the one picked
/// is set ("default" takes the setting away); each is one step of undo. A click that is not on a row opens nothing.
#[test]
fn a_yes_or_no_and_a_choice_are_set_on_the_box() {
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut h = settings_harness("canvas-edit-choice.toml");
    let kind = |h: &Harness<TerrafluxApp>, id: &str, name: &str| {
        editor(h).settings_of(id).unwrap().rows.iter().find(|r| r.name == name).map(|r| r.kind)
    };
    use terraflux_app::editor_view::Kind;
    assert_eq!(kind(&h, "job", "overwrite"), Some(Kind::Bool));
    assert_eq!(kind(&h, "opt", "solver"), Some(Kind::Choice));
    assert_eq!(kind(&h, "opt", "gap"), Some(Kind::Text));
    assert_eq!(kind(&h, "calc", "expression"), Some(Kind::Complex));
    let at = row_of(&mut h, "job", "overwrite");
    click(&mut h, at);
    assert_eq!(param_now(&h, "job", "overwrite"), Some(serde_json::json!(false)));
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("job"), "and the node is selected");
    let at = row_of(&mut h, "job", "overwrite");
    click(&mut h, at);
    assert_eq!(param_now(&h, "job", "overwrite"), Some(serde_json::json!(true)));
    // the choice: its list opens under the row, the current one marked
    use egui_kittest::kittest::Queryable;
    let at = row_of(&mut h, "opt", "solver");
    click(&mut h, at);
    assert_eq!(param_now(&h, "opt", "solver"), Some(serde_json::json!("highs")), "the click alone changes nothing");
    h.get_by_label_contains("gurobi").click();
    h.run_steps(3);
    assert_eq!(param_now(&h, "opt", "solver"), Some(serde_json::json!("gurobi")));
    click(&mut h, at);
    h.get_by_label_contains("default").click();
    h.run_steps(3);
    assert_eq!(param_now(&h, "opt", "solver"), None, "default takes the setting away");
    // Esc closes the list and changes nothing
    let at = row_of(&mut h, "opt", "gap");
    click(&mut h, at);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(0.2)));
    // each was one step of undo: the last one, "default", is undone
    undo(&mut h);
    assert_eq!(param_now(&h, "opt", "solver"), Some(serde_json::json!("gurobi")));
}

/// A click on a number opens a field over its value: typed in and Enter set it, Esc leaves it as it was, an empty
/// one takes it away, a click elsewhere sets what was typed; Delete typed in the field does not delete the node;
/// the keys of the canvas wait; a drag from the row still moves the node; a script is not edited on the box.
#[test]
fn a_number_is_typed_on_the_box() {
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut h = settings_harness("canvas-edit-text.toml");
    // typed in and Enter: set (the text there was selected, so typing replaces it)
    let at = row_of(&mut h, "opt", "gap");
    click(&mut h, at);
    h.event(egui::Event::Text("0.5".into()));
    h.run_steps(2);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(0.2)), "set when it is accepted");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(0.5)));
    // Esc gives up
    let at = row_of(&mut h, "opt", "gap");
    click(&mut h, at);
    h.event(egui::Event::Text("9".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(0.5)));
    // a script is not edited on the box: the node is selected, no field opens
    let at = row_of(&mut h, "calc", "expression");
    click(&mut h, at);
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("calc"));
    h.event(egui::Event::Text("x".into()));
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(param_now(&h, "calc", "expression"), Some(serde_json::json!("out r = 1")));
    // a drag from a row moves the node and edits nothing
    let before = nodes_now(&h, false).iter().find(|n| n.id == "job").unwrap().pos;
    let at = row_of(&mut h, "job", "overwrite");
    drag(&mut h, at, at + egui::vec2(60.0, 40.0));
    assert_ne!(nodes_now(&h, false).iter().find(|n| n.id == "job").unwrap().pos, before, "moved");
    assert_eq!(param_now(&h, "job", "overwrite"), Some(serde_json::json!(true)), "and not turned over");
    // a click elsewhere on the canvas sets what was typed (one step of undo, as each of these)
    let at = row_of(&mut h, "opt", "gap");
    click(&mut h, at);
    h.event(egui::Event::Text("4".into()));
    h.run_steps(2);
    click(&mut h, egui::pos2(900.0, 800.0));
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(4)));
    // Delete in the field is the field's, not the canvas's
    let at = row_of(&mut h, "opt", "gap");
    click(&mut h, at);
    h.event(egui::Event::Text("7".into()));
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    assert!(nodes_now(&h, false).iter().any(|n| n.id == "opt"), "the node stays");
    // an empty field takes the setting away
    h.key_press(egui::Key::Backspace);
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert_eq!(param_now(&h, "opt", "gap"), None);
    assert!(nodes_now(&h, false).iter().any(|n| n.id == "opt"));
    // one step of undo puts it back
    undo(&mut h);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(4)));
}

/// Readers and writers of files, some with their file and some without it, at a readable zoom.
fn files_harness(name: &str) -> Harness<'static, TerrafluxApp> {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let mut m = Model::new("files");
    let mut nodes = vec![
        Node::new("rd", "raster.read"),
        Node::new("wr", "raster.write"),
        Node::new("wr2", "raster.write").param("path", "out/given.tif"),
        Node::new("rd2", "raster.read").param("path", data.to_string_lossy().as_ref()),
        Node::new("wr3", "raster.write"),
    ];
    for (k, n) in nodes.iter_mut().enumerate() {
        n.pos = Some([(k % 3) as f32 * 450.0, (k / 3) as f32 * 420.0]);
    }
    m.nodes = std::mem::take(&mut nodes);
    let mut h = harness(open(&m, name));
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(20.0, 20.0), 0.8);
    h.run_steps(4);
    h
}

/// The answer the next dialog of a node's file gives (a dialog cannot open in a test).
fn answer(h: &mut Harness<TerrafluxApp>, path: &str) {
    h.state_mut().session.as_mut().unwrap().path_answer = Some(path.to_string());
}

/// The row of a file operator's `path` chooses the file in a dialog: the file is set as any edit of a setting (one
/// step of undo), its name then in the foot, where a click chooses another; the menu of the row offers the dialogs;
/// a box that asks for its file says so on the row and in the place of its picture, where a click chooses it too.
#[test]
fn the_path_of_a_file_operator_is_chosen_on_the_box() {
    use egui_kittest::kittest::Queryable;
    use terraflux_app::editor_view::Kind;
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut h = files_harness("canvas-files-path.toml");
    let row = |h: &Harness<TerrafluxApp>, id: &str| {
        editor(h).settings_of(id).unwrap().rows.iter().find(|r| r.name == "path").cloned()
    };
    let r = row(&h, "wr").expect("the path row");
    assert_eq!((r.kind, r.given, r.value.as_str()), (Kind::File, false, "choose a file…"));
    assert!(row(&h, "wr2").is_none(), "given: the foot names the file, no row");
    // a click on the row: the dialog's answer is the setting, as an edit that undo takes back
    answer(&mut h, "out/made.tif");
    let at = row_of(&mut h, "wr", "path");
    click(&mut h, at);
    assert_eq!(param_now(&h, "wr", "path"), Some(serde_json::json!("out/made.tif")));
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("wr"));
    assert!(h.state().session.as_ref().unwrap().path_answer.is_none(), "the dialog was asked");
    undo(&mut h);
    assert_eq!(param_now(&h, "wr", "path"), None);
    // a double click on the row chooses the file and does not open the node
    h.run_steps(60);
    let at = row_of(&mut h, "wr", "path");
    answer(&mut h, "twice.tif");
    double_click(&mut h, at);
    assert_eq!(param_now(&h, "wr", "path"), Some(serde_json::json!("twice.tif")));
    assert_ne!(h.state_mut().focused(), Some(Pane::Viewer), "not the Viewer");
    assert!(h.state().session.as_ref().unwrap().editor.panel.is_none(), "nor the settings");
    // a click on the foot that names the file chooses another
    h.run_steps(12);
    answer(&mut h, "other.tif");
    let foot = editor(&h).screen_of_foot("wr2").expect("a foot");
    click(&mut h, foot);
    assert_eq!(param_now(&h, "wr2", "path"), Some(serde_json::json!("other.tif")));
    // the menu on the row: a file; a folder too for a reader
    let foot = editor(&h).screen_of_foot("rd2").expect("a foot");
    right_click(&mut h, foot);
    assert!(h.query_by_label("Choose a file...").is_some() && h.query_by_label("Choose a folder...").is_some());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    let at = row_of(&mut h, "wr3", "path");
    right_click(&mut h, at);
    assert!(h.query_by_label("Choose a folder...").is_none(), "a writer writes a file");
    answer(&mut h, "menu.tif");
    h.get_by_label("Choose a file...").click();
    h.run_steps(3);
    assert_eq!(param_now(&h, "wr3", "path"), Some(serde_json::json!("menu.tif")));
    // a reader that asks for its file (the place of its picture asks for it: ChangeFile, tested with the pictures):
    // a double click on the box does nothing, there is nothing to show (no Viewer, no dialog)
    h.run_steps(60);
    answer(&mut h, "head.tif");
    let at = editor(&h).screen_of("rd").unwrap();
    double_click(&mut h, at);
    assert_eq!(param_now(&h, "rd", "path"), None, "the double click chooses nothing");
    assert!(h.state().session.as_ref().unwrap().editor.panel.is_none());
    assert!(!h.state().session.as_ref().unwrap().running(), "and runs nothing");
    assert_ne!(h.state_mut().focused(), Some(Pane::Viewer), "nor brings the Viewer");
    h.state_mut().session.as_mut().unwrap().path_answer = None;
}

/// Where the panel of a node's settings is on the screen (none: closed).
fn panel_rect(h: &Harness<TerrafluxApp>) -> Option<egui::Rect> {
    let open = h.state().session.as_ref().unwrap().editor.panel.is_some();
    open.then(|| h.ctx.memory(|m| m.area_rect(egui::Id::new("node settings panel")))).flatten()
}

fn panel_of(h: &Harness<TerrafluxApp>) -> bool {
    use egui_kittest::kittest::Queryable;
    h.query_by_label_contains("Rename").is_some()
}

/// A node's settings open in a panel beside its box, by the gear at the right end of its operator line; the panel
/// holds the Node pane's content, sits at the box's right (at its left with no room there) and follows it; another
/// node's gear moves it there; Esc and a click on the empty canvas close it.
#[test]
fn a_nodes_settings_open_in_a_panel_beside_it() {
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    // (the second node far enough from the first for its panel not to cover it)
    let mut m = Model::new("panel");
    let mut opt = Node::new("opt", "prioritize.optimize").param("gap", 0.2);
    opt.pos = Some([0.0, 0.0]);
    let mut job = Node::new("job", "batch.run").param("operator", "landscape.mspa");
    job.pos = Some([900.0, 0.0]);
    m.nodes = vec![opt, job];
    let mut h = harness(open(&m, "canvas-panel.toml"));
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(20.0, 20.0), 0.8);
    h.run_steps(8);
    assert!(panel_rect(&h).is_none() && !panel_of(&h), "closed: nothing drawn");
    assert!(!h.state().is_open(Pane::Node), "and the Node tab did not open by itself");
    let gear = editor(&h).screen_of_gear("opt").expect("a gear");
    click(&mut h, gear);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!((s.selected.as_deref(), s.editor.panel.is_some()), (Some("opt"), true));
    assert!(panel_of(&h), "the Node pane's content");
    let panel = panel_rect(&h).expect("the panel");
    let at = editor(&h).screen_rect_of("opt").unwrap();
    assert!(panel.left() >= at.right() && (panel.top() - at.top()).abs() < 2.0, "{panel:?} beside {at:?}");
    assert!(editor(&h).canvas().unwrap().contains_rect(panel), "inside the canvas");
    // the view moves: the panel follows the box
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(60.0, 120.0), 0.8);
    h.run_steps(4);
    let (moved, at) = (panel_rect(&h).unwrap(), editor(&h).screen_rect_of("opt").unwrap());
    assert!(moved.min != panel.min && (moved.left() - at.right()).abs() < 12.0, "{moved:?} beside {at:?}");
    // another node's gear: the panel moves to it
    let gear = editor(&h).screen_of_gear("job").unwrap();
    click(&mut h, gear);
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("job"));
    let (panel, at) = (panel_rect(&h).unwrap(), editor(&h).screen_rect_of("job").unwrap());
    assert!(panel.left() >= at.right() - 1.0 && (panel.top() - at.top()).abs() < 2.0, "{panel:?} beside {at:?}");
    // a box at the right edge: the panel at its left
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(630.0, 120.0), 0.8);
    h.run_steps(4);
    let (panel, at) = (panel_rect(&h).unwrap(), editor(&h).screen_rect_of("job").unwrap());
    assert!(panel.right() <= at.left() + 1.0, "{panel:?} at the left of {at:?}");
    // Esc closes it, the gear opens it again, a click on the empty canvas closes it
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.panel.is_none() && !panel_of(&h));
    let gear = editor(&h).screen_of_gear("job").unwrap();
    click(&mut h, gear);
    h.run_steps(3);
    assert!(panel_of(&h));
    click(&mut h, egui::pos2(300.0, 900.0));
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.panel.is_none() && !panel_of(&h), "the empty canvas");
}

/// A click on a setting the box does not edit (a script), shift+Enter on the node selected and "Settings..." in its
/// menu open the panel; the x closes it; a script is edited in it.
#[test]
fn the_panel_opens_from_a_row_enter_and_the_menu() {
    use egui_kittest::kittest::Queryable;
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut h = settings_harness("canvas-panel-open.toml");
    let at = row_of(&mut h, "calc", "expression");
    click(&mut h, at);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!((s.selected.as_deref(), s.editor.panel.is_some()), (Some("calc"), true), "a script's row");
    assert!(panel_of(&h));
    // the x
    h.get_by_label("Close window").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.panel.is_none() && !panel_of(&h), "closed by its x");
    // shift+Enter on the node selected, the pointer over the canvas
    let at = editor(&h).screen_of("job").unwrap();
    click(&mut h, at);
    assert!(h.state().session.as_ref().unwrap().editor.panel.is_none(), "a click on the box alone does not");
    h.hover_at(at);
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!((s.selected.as_deref(), s.editor.panel.is_some()), (Some("job"), true), "shift+Enter");
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.panel.is_none());
    // the node's menu
    menu_of(&mut h, &["opt"], "opt");
    pick(&mut h, "Settings...");
    let s = h.state().session.as_ref().unwrap();
    assert_eq!((s.selected.as_deref(), s.editor.panel.is_some()), (Some("opt"), true), "the menu");
    // a change of scope closes it
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.panel.is_none());
}

/// The Node tab no longer opens by itself with a model's first node; the preference brings it back.
#[test]
fn the_node_tab_opens_by_itself_only_when_asked() {
    let _lock = settings_lock();
    let mut m = Model::new("tab");
    m.nodes.push(reader("read", [0.0, 0.0]));
    let mut h = harness(open(&m, "canvas-tab-off.toml"));
    h.run_steps(4);
    assert!(!h.state().is_open(Pane::Node), "off by default");
    terraflux_app::settings::update(|s| s.node_pane_opens = true);
    let mut h = harness(open(&m, "canvas-tab-on.toml"));
    h.run_steps(4);
    assert!(h.state().is_open(Pane::Node), "asked for");
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

/// What a lift cuts is told: `read -> poly (vector) -> buf`, where what `poly` read (a map) does not fit what `buf`
/// reads (vectors), so `buf` is left unconnected and the message names it; a lift that cuts nothing says nothing.
#[test]
fn a_lift_that_cuts_a_connection_says_so() {
    use terraflux_graph::Source;
    let mut m = Model::new("lift-cut");
    let mut poly = Node::new("poly", "vector.polygonize").input("map", "read.raster");
    let mut buf = Node::new("buf", "vector.buffer").input("vector", "poly.vector").param("distance", 1.0);
    (poly.pos, buf.pos) = (Some([350.0, 0.0]), Some([700.0, 0.0]));
    m.nodes = vec![reader("read", [0.0, 0.0]), poly, buf];
    let mut h = harness(open(&m, "canvas-lift-cut.toml"));
    h.run_steps(3);
    assert!(editor(&h).message.is_none());
    let from = editor(&h).screen_of("poly").unwrap();
    let alt = |h: &mut Harness<TerrafluxApp>, on: bool| {
        let m = if on { egui::Modifiers::ALT } else { egui::Modifiers::NONE };
        h.event(egui::Event::ModifiersChanged(m));
        h.run_steps(1);
    };
    alt(&mut h, true);
    drag(&mut h, from, from + egui::vec2(0.0, 300.0));
    alt(&mut h, false);
    let v = nodes_now(&h, false);
    let find = |id: &str| v.iter().find(|n| n.id == id).cloned().unwrap();
    assert!(find("buf").inputs.get("vector").is_none_or(|s| !matches!(s, Source::Ref(_))), "{:?}", find("buf").inputs);
    assert!(find("poly").inputs.is_empty());
    let said = editor(&h).message.clone().expect("the cut is told");
    assert_eq!(said, "poly taken out; nothing it read fits buf.vector: left unconnected");
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

/// The palette lists *Find a node* (with the shortcut of ctrl+F) and choosing it opens the search; the camera then
/// glides to the node found (frames in between, farther and farther along), ending with the node at the centre.
#[test]
fn find_a_node_in_the_palette_and_the_camera_glides_to_it() {
    use egui_kittest::kittest::Queryable;
    let mut m = Model::new("gliding");
    m.nodes.push(reader("near", [0.0, 0.0]));
    m.nodes.push(reader("far", [3000.0, 2000.0]));
    let mut h = harness(open(&m, "canvas-glide.toml"));
    // (the harness has no animations: the style's, as a screen has it, a glide of 0.25 s)
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    h.run_steps(3);
    let centre = editor(&h).canvas().unwrap().center();
    h.hover_at(centre);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    h.run_steps(2);
    h.event(egui::Event::Text("find a node".into()));
    h.run_steps(2);
    assert!(h.query_by_label_contains("Find a node").is_some(), "listed");
    assert!(h.query_by_label_contains("ctrl+F").is_some(), "with its shortcut");
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(h.state().commands.is_none());
    assert!(editor(&h).find.is_some(), "the search is open");
    h.event(egui::Event::Text("far".into()));
    h.run_steps(2);
    let from = editor(&h).drawn_rect_of("far").unwrap().center();
    h.key_press(egui::Key::Enter);
    let mut seen = vec![];
    for _ in 0..60 {
        h.step();
        seen.push(editor(&h).drawn_rect_of("far").unwrap().center());
    }
    let to = editor(&h).canvas().unwrap().center();
    let (total, last) = ((to - from).length(), seen.last().unwrap());
    assert!((*last - to).length() < 1.0, "ends centred: {last:?} against {to:?}");
    let between = seen.iter().filter(|p| (**p - to).length() > 2.0 && (**p - from).length() > 2.0).count();
    assert!(between >= 3, "{between} frames in between, of {total}: {seen:?}");
    // (each frame no farther from the target than the one before)
    assert!(seen.windows(2).all(|w| (w[1] - to).length() <= (w[0] - to).length() + 1.0), "{seen:?}");
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
        nodes: vec![reader("a", [0.0, 0.0]), reader("b", [9000.0, 0.0]), reader("c", [0.0, 1800.0])],
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
    assert!(h.query_by_label("Open its documentation").is_some(), "its page, in one node's menu");
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
    // (held before c by a breakpoint: the queue is seen however fast c would run)
    h.state().session.as_ref().unwrap().gate.set_breakpoint("c", true);
    menu_of(&mut h, &["a", "c", "z"], "a");
    pick(&mut h, "Run to them");
    let t = Instant::now();
    while !h.state().session.as_ref().unwrap().gate.paused() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "the run did not pause before c");
    }
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.target.as_deref(), Some("c"), "{:?}", s.run_queue);
    assert_eq!(s.run_queue, ["z"]);
    // resume: c runs (these readers have no data, so it fails and drops the queue; with data, z would follow)
    h.state().session.as_ref().unwrap().gate.set_breakpoint("c", false);
    h.state().session.as_ref().unwrap().gate.resume();
    let t = Instant::now();
    loop {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        let s = h.state().session.as_ref().unwrap();
        if !s.running() && s.run_queue.is_empty() {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(120), "the queue did not run out: {:?} {:?}", s.target, s.run_queue);
    }
    assert!(!h.state().session.as_ref().unwrap().gate.paused());
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

/// The file a node reads, as the model says it.
fn path_of(h: &Harness<TerrafluxApp>, inside: bool, id: &str) -> String {
    nodes_now(h, inside).iter().find(|n| n.id == id).unwrap().params["path"].as_str().unwrap().to_string()
}

/// A read node's file changed (the menu's "Change file..." applies it through `change_file`; the dialog itself is the
/// system's): the id, the links and the other settings stay, one step of undo; the entry is in the menu of the
/// nodes that read a file only. At the top level and in a group.
#[test]
fn a_read_nodes_file_is_changed_keeping_its_links() {
    use egui_kittest::kittest::Queryable;
    use terraflux_graph::Source;
    let other = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_drivers.tif");
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-change-file.toml");
        let old = path_of(&h, inside, "a");
        // the menu: an entry on the reader, none on the node that calculates
        for (on, entry) in [("a", true), ("b", false)] {
            // (a node selected: escape closes the menu, it does not leave the group)
            menu_of(&mut h, &["z"], on);
            assert_eq!(h.query_by_label_contains("Change file...").is_some(), entry, "{on}");
            h.key_press(egui::Key::Escape);
            h.run_steps(2);
        }
        let r = h.state().registry.clone();
        let s = h.state_mut().session.as_mut().unwrap();
        let revision = s.revision;
        terraflux_app::builder::change_file(s, &r, "a", &other);
        h.run_steps(2);
        assert_ne!(h.state().session.as_ref().unwrap().revision, revision, "seen as an edit");
        assert_eq!(path_of(&h, inside, "a"), other.to_string_lossy());
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), 4, "inside {inside}");
        assert_eq!(v.iter().find(|n| n.id == "b").unwrap().inputs["map"], Source::Ref("a.raster".into()));
        assert_eq!(v.iter().find(|n| n.id == "a").unwrap().pos, Some([0.0, 0.0]));
        undo(&mut h);
        assert_eq!(path_of(&h, inside, "a"), old, "one step of undo");
    }
}

/// The pattern typed in the window of "Rename...".
fn pattern(h: &mut Harness<TerrafluxApp>, text: &str) {
    h.state_mut().session.as_mut().unwrap().editor.batch.as_mut().unwrap().0 = text.to_string();
    h.run_steps(2);
}

/// Whether some text on screen has `text` (the preview says each node's name twice: the label and its text run).
fn shows(h: &Harness<TerrafluxApp>, text: &str) -> bool {
    use egui_kittest::kittest::Queryable;
    h.query_all_by_label_contains(text).count() > 0
}

fn ids_now(h: &Harness<TerrafluxApp>, inside: bool) -> Vec<String> {
    nodes_now(h, inside).iter().map(|n| n.id.clone()).collect()
}

/// The selection's "Rename..." names the nodes by a pattern in the order of the canvas, each `old -> new` in a
/// preview; the links follow, one step of undo; a name that clashes (among the new ones or with a node outside the
/// selection) or is not an id blocks the button; a chain or a swap works. At the top level and in a group.
#[test]
fn the_selections_rename_follows_a_pattern() {
    use terraflux_graph::Source;
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-batch-rename.toml");
        let find = |v: &[Node], id: &str| v.iter().find(|n| n.id == id).cloned().unwrap();
        // `land_{n}`: a, b, c are in the order of the canvas (z is lower), the links follow
        menu_of(&mut h, &["c", "a", "b"], "b");
        pick(&mut h, "Rename...");
        pattern(&mut h, "land_{n}");
        for l in ["a -> land_1", "b -> land_2", "c -> land_3"] {
            assert!(shows(&h, l), "preview {l}, inside {inside}");
        }
        pick(&mut h, "OK");
        assert_eq!(ids_now(&h, inside), ["land_1", "land_2", "land_3", "z"], "inside {inside}");
        let v = nodes_now(&h, inside);
        assert_eq!(find(&v, "land_2").inputs["map"], Source::Ref("land_1.raster".into()));
        assert_eq!(find(&v, "land_3").inputs["map"], Source::Ref("land_2.map".into()));
        assert!(h.state().session.as_ref().unwrap().editor.batch.is_none(), "the window closed");
        assert_eq!(chosen_now(&h), ["land_1", "land_2", "land_3"]);
        undo(&mut h);
        assert_eq!(ids_now(&h, inside), ["a", "b", "c", "z"], "one step of undo");
        assert_eq!(find(&nodes_now(&h, inside), "c").inputs["map"], Source::Ref("b.map".into()));
        // a swap, and a chain (a to b, b to c, c to d): no name is met taken midway
        let chain = |h: &mut Harness<TerrafluxApp>, names: &[(&str, &str)]| {
            let s = h.state_mut().session.as_mut().unwrap();
            let pairs: Vec<(String, String)> = names.iter().map(|(o, n)| (o.to_string(), n.to_string())).collect();
            let path = s.editor.path.clone();
            terraflux_app::editor::rename_many(&mut s.source, &path, &pairs).unwrap();
            s.edited();
            h.run_steps(2);
        };
        chain(&mut h, &[("a", "b"), ("b", "a")]);
        let v = nodes_now(&h, inside);
        assert_eq!(find(&v, "a").op, "raster.reclassify", "the swap, inside {inside}");
        assert_eq!(find(&v, "b").op, "raster.read");
        assert_eq!(find(&v, "a").inputs["map"], Source::Ref("b.raster".into()), "links follow the swap");
        assert_eq!(find(&v, "c").inputs["map"], Source::Ref("a.map".into()));
        undo(&mut h);
        chain(&mut h, &[("a", "b"), ("b", "c"), ("c", "d")]);
        assert_eq!(ids_now(&h, inside), ["b", "c", "d", "z"], "the chain");
        let v = nodes_now(&h, inside);
        assert_eq!(find(&v, "c").inputs["map"], Source::Ref("b.raster".into()));
        assert_eq!(find(&v, "d").inputs["map"], Source::Ref("c.map".into()));
        undo(&mut h);
        assert_eq!(ids_now(&h, inside), ["a", "b", "c", "z"]);
    }
}

/// `{file}` is the stem of the file each node reads (as a node made by dropping the file is named); a node without a
/// file is an error under it; two nodes of one file, a name of a node outside, and a name that is no id block OK
/// with the reason, and nothing changes.
#[test]
fn the_selections_rename_checks_its_names() {
    let other = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_drivers.tif");
    let mut h = scoped_model(false, "canvas-batch-names.toml");
    let r = h.state().registry.clone();
    terraflux_app::builder::change_file(h.state_mut().session.as_mut().unwrap(), &r, "z", &other);
    h.run_steps(2);
    menu_of(&mut h, &["a", "z"], "a");
    pick(&mut h, "Rename...");
    pattern(&mut h, "{file}");
    assert!(shows(&h, "a -> sibuyan_landuse"));
    assert!(shows(&h, "z -> sibuyan_drivers"));
    pick(&mut h, "OK");
    assert_eq!(ids_now(&h, false), ["sibuyan_landuse", "b", "c", "sibuyan_drivers"]);
    undo(&mut h);
    // blocked: the same name twice, a node outside, no id, and a node without a file under `{file}`
    let before = ids_now(&h, false);
    for (ids, text, why) in [
        (vec!["a", "z"], "same", "another node of the selection"),
        (vec!["a", "z"], "b", "already named"),
        (vec!["a", "z"], "1{id}", "not a valid id"),
        (vec!["a", "b"], "{file}", "reads no file"),
    ] {
        menu_of(&mut h, &ids, "a");
        pick(&mut h, "Rename...");
        pattern(&mut h, text);
        assert!(shows(&h, why), "{text}: {why}");
        pick(&mut h, "OK");
        assert_eq!(ids_now(&h, false), before, "{text}: blocked");
        assert!(h.state().session.as_ref().unwrap().editor.batch.is_some(), "{text}: the window stays");
        h.key_press(egui::Key::Escape);
        h.run_steps(3);
        assert!(h.state().session.as_ref().unwrap().editor.batch.is_none(), "escape closes it");
    }
    // the names the pattern gives, without the window
    let nodes = h.state().session.as_ref().unwrap().source.nodes.clone();
    let ids: Vec<String> = ["b", "c"].map(String::from).to_vec();
    let names = terraflux_app::editor::batch_names(&nodes, &ids, "{op}_{id}");
    assert_eq!(names[0].1.as_deref(), Ok("raster_reclassify_b"));
    assert_eq!(names[1].1.as_deref(), Ok("landscape_mspa_c"));
}

/// A read node whose id is the one its file gave it follows the file when it changes (the menu's and the drop's
/// `change_file`), in the same step of undo; an id chosen by hand stays. At the top level and in a group.
#[test]
fn the_id_follows_the_file_when_it_changes() {
    use terraflux_graph::Source;
    let other = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_drivers.tif");
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-id-follows.toml");
        let r = h.state().registry.clone();
        let path = if inside { vec!["grp".to_string()] } else { vec![] };
        let old = path_of(&h, inside, "a");
        // named after its file: renamed to the new file's name
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::editor::rename_node(&mut s.source, &path, "a", "sibuyan_landuse").unwrap();
        s.edited();
        h.run_steps(2);
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::change_file(s, &r, "sibuyan_landuse", &other);
        h.run_steps(2);
        assert_eq!(ids_now(&h, inside), ["sibuyan_drivers", "b", "c", "z"], "inside {inside}");
        assert_eq!(path_of(&h, inside, "sibuyan_drivers"), other.to_string_lossy());
        let v = nodes_now(&h, inside);
        assert_eq!(v[1].inputs["map"], Source::Ref("sibuyan_drivers.raster".into()), "the link follows");
        undo(&mut h);
        assert_eq!(ids_now(&h, inside), ["sibuyan_landuse", "b", "c", "z"], "one step of undo");
        assert_eq!(path_of(&h, inside, "sibuyan_landuse"), old, "both restored");
        // a name chosen by hand stays (`z` reads the same file)
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::change_file(s, &r, "z", &other);
        h.run_steps(2);
        assert_eq!(ids_now(&h, inside), ["sibuyan_landuse", "b", "c", "z"], "inside {inside}");
        assert_eq!(path_of(&h, inside, "z"), other.to_string_lossy());
        // the name taken by another node: made unique
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::editor::rename_node(&mut s.source, &path, "z", "sibuyan_drivers").unwrap();
        s.edited();
        h.run_steps(2);
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::change_file(s, &r, "sibuyan_landuse", &other);
        h.run_steps(2);
        assert_eq!(ids_now(&h, inside), ["sibuyan_drivers_2", "b", "c", "sibuyan_drivers"], "inside {inside}");
    }
}

/// A single file let go on a read node of its kind replaces that node's file (connections kept), the node lit while
/// the file hovers; a file of another kind, or several, add nodes as before. At the top level and in a group.
#[test]
fn a_file_dropped_on_a_read_node_replaces_its_file() {
    use terraflux_graph::Source;
    let other = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_drivers.tif");
    let geojson = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-change-points.geojson");
    std::fs::write(&geojson, r#"{"type":"FeatureCollection","features":[]}"#).unwrap();
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-drop-on-node.toml");
        let old = path_of(&h, inside, "a");
        // the hit test: the node under a point, none on the background
        let at = editor(&h).screen_of("a").unwrap();
        assert_eq!(editor(&h).node_at(at), Some(("a".to_string(), "raster.read".to_string())));
        assert_eq!(editor(&h).node_at(editor(&h).screen_of("b").unwrap()).unwrap().0, "b");
        let canvas = editor(&h).canvas().unwrap();
        assert_eq!(editor(&h).node_at(canvas.left_bottom() + egui::vec2(5.0, -5.0)), None);
        // a file of another kind over the raster: a node more, the file of `a` as it was
        h.hover_at(at);
        h.run_steps(2);
        h.input_mut().dropped_files.push(std::sync::Arc::new(Dropped(geojson.clone())));
        h.run_steps(3);
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), 5, "inside {inside}: {v:?}");
        assert!(v.iter().any(|n| n.op == "vector.read"), "inside {inside}: {v:?}");
        assert_eq!(path_of(&h, inside, "a"), old);
        let after_wrong = nodes_now(&h, inside).len();
        // a raster over the raster, the file hovering first: the node lit, then its file replaced
        h.hover_at(at);
        h.run_steps(2);
        h.input_mut().hovered_files.push(egui::HoveredFile { path: Some(other.clone()), mime: String::new() });
        h.run_steps(2);
        assert_eq!(h.state().drop_lit.as_deref(), Some("a"), "the node lit, inside {inside}");
        // the pointer leaves the node: it goes out; back on it: lit again
        h.hover_at(canvas.left_bottom() + egui::vec2(5.0, -5.0));
        h.run_steps(2);
        assert_eq!(h.state().drop_lit, None, "pointer away, inside {inside}");
        h.hover_at(at);
        h.run_steps(2);
        assert_eq!(h.state().drop_lit.as_deref(), Some("a"), "back on it, inside {inside}");
        // a file of another kind hovering does not light it
        h.input_mut().hovered_files.clear();
        h.input_mut().hovered_files.push(egui::HoveredFile { path: Some(geojson.clone()), mime: String::new() });
        h.run_steps(2);
        assert_eq!(h.state().drop_lit, None, "a vector over a raster, inside {inside}");
        h.input_mut().hovered_files.clear();
        h.run_steps(2);
        assert_eq!(h.state().drop_lit, None, "no file hovering, inside {inside}");
        h.input_mut().dropped_files.push(std::sync::Arc::new(Dropped(other.clone())));
        h.run_steps(3);
        let v = nodes_now(&h, inside);
        assert_eq!(v.len(), after_wrong, "no node added, inside {inside}");
        assert_eq!(path_of(&h, inside, "a"), other.to_string_lossy());
        assert_eq!(v.iter().find(|n| n.id == "b").unwrap().inputs["map"], Source::Ref("a.raster".into()));
        undo(&mut h);
        assert_eq!(path_of(&h, inside, "a"), old, "one step of undo");
    }
}

/// The boxes of `ids` as the canvas has them now (graph units).
fn boxes_now(h: &Harness<TerrafluxApp>, ids: &[&str]) -> Vec<egui::Rect> {
    let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
    let v = editor(h).rects_of(&ids);
    ids.iter().map(|k| v.iter().find(|(i, _)| i == k).unwrap().1).collect()
}

fn places_now(h: &Harness<TerrafluxApp>, inside: bool, ids: &[&str]) -> Vec<[f32; 2]> {
    let v = nodes_now(h, inside);
    ids.iter().map(|k| v.iter().find(|n| n.id == *k).unwrap().pos.unwrap()).collect()
}

/// The selection's "Align" submenu: left, top and centre edges move only the coordinate concerned; distribute
/// leaves equal gaps with the extremes fixed (greyed with fewer than three); a column stacks with equal gaps, the
/// first one staying; each is one step of undo, in a group too.
#[test]
fn the_selections_menu_aligns_and_distributes() {
    use egui_kittest::kittest::{NodeT, Queryable};
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-align.toml");
        let go = |h: &mut Harness<TerrafluxApp>, ids: &[&str], on: &str, entry: &str| {
            menu_of(h, ids, on);
            pick(h, "Align");
            pick(h, entry);
        };
        let before = places_now(&h, inside, &["a", "b", "c", "z"]);
        // left edges: only x moves
        go(&mut h, &["b", "c", "z"], "b", "Left edges");
        let now = places_now(&h, inside, &["a", "b", "c", "z"]);
        assert_eq!(now, [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 300.0]], "inside {inside}");
        undo(&mut h);
        assert_eq!(places_now(&h, inside, &["a", "b", "c", "z"]), before, "one step of undo");
        // top edges: only y moves
        go(&mut h, &["a", "b", "z"], "a", "Top edges");
        assert_eq!(places_now(&h, inside, &["a", "b", "z"]), [[0.0, 0.0], [350.0, 0.0], [0.0, 0.0]]);
        undo(&mut h);
        // horizontal centres: one centre, y as it was
        go(&mut h, &["a", "b", "c"], "a", "Horizontal centres");
        let r = boxes_now(&h, &["a", "b", "c"]);
        assert!((r[0].center().x - r[1].center().x).abs() < 0.5 && (r[1].center().x - r[2].center().x).abs() < 0.5);
        assert!(places_now(&h, inside, &["a", "b", "c"]).iter().all(|p| p[1] == 0.0));
        undo(&mut h);
        // vertical centres and right edges and bottom edges
        go(&mut h, &["a", "z"], "a", "Vertical centres");
        let r = boxes_now(&h, &["a", "z"]);
        assert!((r[0].center().y - r[1].center().y).abs() < 0.5);
        assert!(places_now(&h, inside, &["a", "z"]).iter().all(|p| p[0] == 0.0));
        undo(&mut h);
        go(&mut h, &["a", "b", "c"], "a", "Right edges");
        let r = boxes_now(&h, &["a", "b", "c"]);
        let r0 = r[2];
        assert!(r.iter().all(|r| (r.max.x - r0.max.x).abs() < 0.5), "{r:?}");
        undo(&mut h);
        go(&mut h, &["a", "z"], "a", "Bottom edges");
        let r = boxes_now(&h, &["a", "z"]);
        assert!((r[0].max.y - r[1].max.y).abs() < 0.5);
        undo(&mut h);
        // distribute: the ends stay, the gaps are equal
        let r0 = boxes_now(&h, &["a", "b", "c"]);
        go(&mut h, &["a", "b", "c"], "a", "Distribute horizontally");
        let r = boxes_now(&h, &["a", "b", "c"]);
        assert!((r[0].min.x - r0[0].min.x).abs() < 0.01 && (r[2].min.x - r0[2].min.x).abs() < 0.01);
        assert!(((r[1].min.x - r[0].max.x) - (r[2].min.x - r[1].max.x)).abs() < 0.5, "{r:?}");
        undo(&mut h);
        assert_eq!(places_now(&h, inside, &["a", "b", "c", "z"]), before);
        // a column: by y, the first one (a) staying, a gap each
        go(&mut h, &["a", "c", "z"], "a", "In a column");
        let r = boxes_now(&h, &["a", "c", "z"]);
        assert_eq!(places_now(&h, inside, &["a"]), [[0.0, 0.0]], "the first stays");
        assert!((r[1].min.y - r[0].max.y - 20.0).abs() < 0.5 && (r[2].min.y - r[1].max.y - 20.0).abs() < 0.5, "{r:?}");
        assert!(places_now(&h, inside, &["c", "z"]).iter().all(|p| p[0] == 0.0), "the first one's x");
        undo(&mut h);
        assert_eq!(places_now(&h, inside, &["a", "b", "c", "z"]), before, "one step of undo");
        // two nodes: distribute greyed with its reason, the rest not
        menu_of(&mut h, &["a", "b"], "a");
        pick(&mut h, "Align");
        assert!(h.get_by_label_contains("Distribute horizontally").accesskit_node().is_disabled());
        assert!(h.get_by_label_contains("Distribute vertically").accesskit_node().is_disabled());
        assert!(!h.get_by_label_contains("Left edges").accesskit_node().is_disabled());
        assert!(!h.get_by_label_contains("In a row").accesskit_node().is_disabled());
    }
}

/// The Align menu's keys (alt+shift and the arrows, C, M, H, V, K, R) do what its entries do to the nodes selected, each
/// one step of undo, in a group too; with one node selected, or two to distribute, nothing; the menu shows them.
#[test]
fn the_align_keys_do_what_the_menu_does() {
    use egui_kittest::kittest::{NodeT, Queryable};
    let alt_shift = egui::Modifiers { alt: true, shift: true, ..egui::Modifiers::NONE };
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-align-keys.toml");
        let all = ["a", "b", "c", "z"];
        let before = places_now(&h, inside, &all);
        let select = |h: &mut Harness<TerrafluxApp>, ids: &[&str]| {
            let s = h.state_mut().session.as_mut().unwrap();
            s.selected = ids.first().map(|s| s.to_string());
            s.editor.chosen = ids.iter().map(|s| s.to_string()).collect();
            h.run_steps(2);
            let at = editor(h).screen_of(ids[0]).unwrap();
            h.hover_at(at);
            h.run_steps(1);
        };
        // alt+shift+left: the left edges, as "Left edges" does (the same places)
        select(&mut h, &["b", "c", "z"]);
        h.key_press_modifiers(alt_shift, egui::Key::ArrowLeft);
        h.run_steps(3);
        let by_key = places_now(&h, inside, &all);
        assert_eq!(by_key, [[0.0, 0.0], [0.0, 0.0], [0.0, 0.0], [0.0, 300.0]], "inside {inside}");
        undo(&mut h);
        assert_eq!(places_now(&h, inside, &all), before, "one step of undo");
        select(&mut h, &["b", "c", "z"]);
        menu_of(&mut h, &["b", "c", "z"], "b");
        pick(&mut h, "Align");
        pick(&mut h, "Left edges");
        assert_eq!(places_now(&h, inside, &all), by_key, "the menu's and the key's");
        undo(&mut h);
        // the top edges, the centres, the distribution: each one step
        select(&mut h, &["a", "b", "z"]);
        h.key_press_modifiers(alt_shift, egui::Key::ArrowUp);
        h.run_steps(3);
        assert_eq!(places_now(&h, inside, &["a", "b", "z"]), [[0.0, 0.0], [350.0, 0.0], [0.0, 0.0]]);
        undo(&mut h);
        assert_eq!(places_now(&h, inside, &all), before);
        select(&mut h, &["a", "b", "c"]);
        h.key_press_modifiers(alt_shift, egui::Key::C);
        h.run_steps(3);
        let r = boxes_now(&h, &["a", "b", "c"]);
        assert!((r[0].center().x - r[1].center().x).abs() < 0.5 && (r[1].center().x - r[2].center().x).abs() < 0.5);
        undo(&mut h);
        let r0 = boxes_now(&h, &["a", "b", "c"]);
        select(&mut h, &["a", "b", "c"]);
        h.key_press_modifiers(alt_shift, egui::Key::H);
        h.run_steps(3);
        let r = boxes_now(&h, &["a", "b", "c"]);
        assert!((r[0].min.x - r0[0].min.x).abs() < 0.01 && (r[2].min.x - r0[2].min.x).abs() < 0.01);
        assert!(((r[1].min.x - r[0].max.x) - (r[2].min.x - r[1].max.x)).abs() < 0.5, "{r:?}");
        undo(&mut h);
        assert_eq!(places_now(&h, inside, &all), before, "one step of undo");
        // alt+shift+K and R: a column and a row, as "In a column" and "In a row" do (the same places), one step of undo
        for (key, text) in [(egui::Key::K, "In a column"), (egui::Key::R, "In a row")] {
            let column = key == egui::Key::K;
            select(&mut h, &["a", "b", "c"]);
            h.key_press_modifiers(alt_shift, key);
            h.run_steps(3);
            let by_key = places_now(&h, inside, &all);
            assert_ne!(by_key, before, "{text}: moved");
            let mut r = boxes_now(&h, &["a", "b", "c"]);
            r.sort_by(|p, q| if column { p.min.y.total_cmp(&q.min.y) } else { p.min.x.total_cmp(&q.min.x) });
            for w in r.windows(2) {
                // (one after the other, none over another; across, all on the first one's line)
                if column {
                    assert!(w[1].min.y >= w[0].max.y && (w[1].min.x - w[0].min.x).abs() < 0.5, "{text}: {r:?}");
                } else {
                    assert!(w[1].min.x >= w[0].max.x && (w[1].min.y - w[0].min.y).abs() < 0.5, "{text}: {r:?}");
                }
            }
            undo(&mut h);
            assert_eq!(places_now(&h, inside, &all), before, "{text}: one step of undo");
            select(&mut h, &["a", "b", "c"]);
            menu_of(&mut h, &["a", "b", "c"], "a");
            pick(&mut h, "Align");
            pick(&mut h, text);
            assert_eq!(places_now(&h, inside, &all), by_key, "{text}: the menu's and the key's");
            undo(&mut h);
            assert_eq!(places_now(&h, inside, &all), before);
        }
        // two nodes cannot be distributed, one cannot be aligned: nothing moves
        select(&mut h, &["a", "b"]);
        h.key_press_modifiers(alt_shift, egui::Key::H);
        h.run_steps(3);
        select(&mut h, &["c"]);
        h.key_press_modifiers(alt_shift, egui::Key::ArrowLeft);
        h.run_steps(3);
        assert_eq!(places_now(&h, inside, &all), before, "inside {inside}");
        // the menu shows the keys
        menu_of(&mut h, &["a", "b", "c"], "a");
        pick(&mut h, "Align");
        assert!(!h.get_by_label_contains("Left edges    shift+alt+Left").accesskit_node().is_disabled());
        assert!(!h.get_by_label_contains("Distribute horizontally    shift+alt+H").accesskit_node().is_disabled());
        assert!(!h.get_by_label_contains("In a column    shift+alt+K").accesskit_node().is_disabled());
        assert!(!h.get_by_label_contains("In a row    shift+alt+R").accesskit_node().is_disabled());
    }
}

/// `r1` and `r2` into a `calc`'s `a` and `b`; a table and `r2` into a `fit`'s `table` and `target` (types that do
/// not swap); at the top level, or in a group `grp` that is shown.
fn wired_model(inside: bool, name: &str) -> Harness<'static, TerrafluxApp> {
    let mut calc = Node::new("calc", "raster.reclassify").input("map", "r1.raster");
    let mut other = Node::new("other", "raster.reclassify").input("map", "r2.raster");
    other.pos = Some([500.0, 150.0]);
    calc.pos = Some([500.0, 0.0]);
    let mut fit = Node::new("fit", "ml.fit").input("table", "tb.table").input("target", "r2.raster");
    fit.pos = Some([500.0, 400.0]);
    let mut tb = Node::new("tb", "table.read");
    tb.pos = Some([0.0, 600.0]);
    let nodes = vec![reader("r1", [0.0, 0.0]), reader("r2", [0.0, 300.0]), calc, other, fit, tb];
    let mut m = Model::new("wired");
    if inside {
        m.nodes
            .push(Node::new("grp", terraflux_graph::GROUP).body(terraflux_graph::Body { nodes, ..Default::default() }));
    } else {
        m.nodes = nodes;
    }
    let mut h = harness(open(&m, name));
    h.run_steps(3);
    if inside {
        h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
        h.run_steps(40);
    }
    h
}

fn source_of(h: &Harness<TerrafluxApp>, inside: bool, id: &str, port: &str) -> Option<String> {
    use terraflux_graph::Source;
    let n = nodes_now(h, inside).into_iter().find(|n| n.id == id)?;
    match n.inputs.get(port)? {
        Source::Ref(r) => Some(r.clone()),
        _ => None,
    }
}

/// A node's menu: "Disconnect all inputs" takes off every connection into it in one step of undo (greyed when it
/// has none). A connection picked up from an input and let go on another connected one swaps the two (one step of
/// undo); when the types do not fit both ways it replaces, as on any input, and the error says why. In a group too.
#[test]
fn a_nodes_menu_disconnects_all_and_wires_swap() {
    use egui_kittest::kittest::{NodeT, Queryable};
    for inside in [false, true] {
        let mut h = wired_model(inside, "canvas-swap.toml");
        // disconnect all: both inputs of fit, one undo
        let at = editor(&h).screen_of("fit").unwrap();
        right_click(&mut h, at);
        assert!(!h.get_by_label_contains("Disconnect all inputs").accesskit_node().is_disabled());
        pick(&mut h, "Disconnect all inputs");
        assert_eq!(source_of(&h, inside, "fit", "table"), None, "inside {inside}");
        assert_eq!(source_of(&h, inside, "fit", "target"), None);
        undo(&mut h);
        assert_eq!(source_of(&h, inside, "fit", "table").as_deref(), Some("tb.table"));
        assert_eq!(source_of(&h, inside, "fit", "target").as_deref(), Some("r2.raster"));
        // greyed: a node that reads nothing
        let at = editor(&h).screen_of("r1").unwrap();
        right_click(&mut h, at);
        assert!(h.get_by_label_contains("Disconnect all inputs").accesskit_node().is_disabled());
        // (a click on the background closes the menu; escape would leave the group)
        let canvas = editor(&h).canvas().unwrap();
        click(&mut h, canvas.left_bottom() + egui::vec2(60.0, -60.0));
        h.run_steps(2);
        // swap the two reclassify nodes' inputs
        let from = editor(&h).screen_of_input("calc", "map").unwrap();
        let to = editor(&h).screen_of_input("other", "map").unwrap();
        drag(&mut h, from, to);
        assert_eq!(source_of(&h, inside, "calc", "map").as_deref(), Some("r2.raster"), "inside {inside}");
        assert_eq!(source_of(&h, inside, "other", "map").as_deref(), Some("r1.raster"));
        undo(&mut h);
        assert_eq!(source_of(&h, inside, "calc", "map").as_deref(), Some("r1.raster"), "one undo restores both");
        assert_eq!(source_of(&h, inside, "other", "map").as_deref(), Some("r2.raster"));
        // a table over a map's input: no swap; refused, the target keeps what it had
        let from = editor(&h).screen_of_input("fit", "table").unwrap();
        let to = editor(&h).screen_of_input("fit", "target").unwrap();
        drag(&mut h, from, to);
        assert_eq!(source_of(&h, inside, "fit", "target").as_deref(), Some("r2.raster"), "inside {inside}");
        assert_eq!(source_of(&h, inside, "fit", "table"), None, "taken off, as when dropped nowhere");
        assert!(editor(&h).message.is_some(), "the refusal said why");
    }
}

/// Four readers far apart (a model that does not fit the view at 1:1), as nodes of the top level or of a group.
fn spread_model(inside: bool) -> Model {
    let nodes: Vec<Node> = [("a", [0.0, 0.0]), ("b", [5000.0, 0.0]), ("c", [0.0, 3000.0]), ("d", [5000.0, 3000.0])]
        .into_iter()
        .map(|(id, at)| reader(id, at))
        .collect();
    let mut m = Model::new("spread");
    if inside {
        m.nodes.push(reader("near", [0.0, 0.0]));
        m.nodes
            .push(Node::new("grp", terraflux_graph::GROUP).body(terraflux_graph::Body { nodes, ..Default::default() }));
    } else {
        m.nodes = nodes;
    }
    m
}

/// The graph point at the canvas's centre, and the minimap's mapping (the graph rectangle of the boxes, the
/// points per unit).
fn view_centre(h: &Harness<TerrafluxApp>) -> egui::Pos2 {
    let e = editor(h);
    e.graph_at(e.canvas().unwrap().center()).unwrap()
}

fn minimap_of(h: &Harness<TerrafluxApp>, ids: &[&str]) -> (egui::Rect, egui::Rect, f32) {
    let e = editor(h);
    let ids: Vec<String> = ids.iter().map(|s| s.to_string()).collect();
    let world = e.rects_of(&ids).into_iter().map(|(_, r)| r).reduce(|a, b| a.union(b)).unwrap();
    let scale = (180.0 / world.width()).min(120.0 / world.height());
    (e.minimap().expect("the minimap is shown"), world, scale)
}

/// Where a graph point is on the minimap.
fn on_minimap(m: egui::Rect, world: egui::Rect, scale: f32, g: egui::Pos2) -> egui::Pos2 {
    m.center() - world.size() * scale / 2.0 + (g - world.min) * scale
}

/// The minimap in the corner of a canvas the model does not fit: shown only then, a click on it centres the view
/// there, a drag pans it, the wheel over it does not zoom, a click on it keeps the selection, inside a group it maps
/// the group's body, and its button hides it (and another shows it again).
#[test]
fn the_minimap_shows_a_large_model_and_moves_the_view() {
    let _settings = settings_lock();
    use egui_kittest::kittest::Queryable;
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::settings::update(|s| s.minimap = true);
    let ids = ["a", "b", "c", "d"];
    let mut h = harness(open(&spread_model(false), "canvas-minimap.toml"));
    h.run_steps(3);
    assert!(editor(&h).minimap().is_none(), "the whole model fits the view");
    assert!(h.query_by_label("Minimap").is_none());
    // closer: it does not fit
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(60.0, 60.0), 1.0);
    h.run_steps(3);
    assert!(h.query_by_label("Minimap").is_some(), "found by its label");
    let (m, world, s) = minimap_of(&h, &ids);
    let canvas = editor(&h).canvas().unwrap();
    assert!(m.width() <= 190.0 && m.height() <= 130.0 && canvas.contains_rect(m), "{m:?} in {canvas:?}");
    assert!(m.right() > canvas.right() - 20.0 && m.bottom() > canvas.bottom() - 40.0, "in the bottom right corner");
    // a click centres the view on that point; the selection stays, and so does the zoom
    let s0 = h.state_mut().session.as_mut().unwrap();
    s0.selected = Some("a".into());
    s0.editor.chosen = ["a".to_string()].into();
    h.run_steps(2);
    let g = world.min + world.size() * 0.25;
    click(&mut h, on_minimap(m, world, s, g));
    h.run_steps(3);
    let c = view_centre(&h);
    assert!((c - g).length() < 2.0, "{c:?} not at {g:?}");
    assert_eq!(editor(&h).zoom(), 1.0);
    assert_eq!(chosen_now(&h), ["a"], "a click on the minimap does not reach the canvas");
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("a"));
    // a drag from the view's rectangle pans it by the pointer's way (in the minimap's scale)
    let from = on_minimap(m, world, s, c);
    drag(&mut h, from, from + egui::vec2(20.0, 10.0));
    let c2 = view_centre(&h);
    let want = c + egui::vec2(20.0, 10.0) / s;
    assert!((c2 - want).length() < 3.0, "{c2:?} not at {want:?}");
    assert_eq!(chosen_now(&h), ["a"], "nor does a drag");
    // the wheel over it does not zoom the canvas
    h.hover_at(m.center());
    h.run_steps(2);
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 120.0),
        phase: egui::TouchPhase::Move,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(10);
    assert_eq!(editor(&h).zoom(), 1.0);
    // its button hides it; the corner's button shows it again
    let close = egui::Rect::from_min_size(m.right_top() + egui::vec2(-15.0, 1.0), egui::Vec2::splat(14.0));
    click(&mut h, close.center());
    assert!(editor(&h).minimap().is_none(), "hidden");
    assert!(!terraflux_app::settings::get().minimap, "and kept");
    let reopen = canvas.right_bottom() - egui::vec2(10.0, 24.0) - egui::vec2(27.0, 9.0);
    click(&mut h, reopen);
    assert!(editor(&h).minimap().is_some(), "shown again");
    assert!(terraflux_app::settings::get().minimap);
    // fitted again (Home): nothing to show
    h.hover_at(canvas.center());
    h.run_steps(2);
    h.key_press(egui::Key::Home);
    h.run_steps(40);
    assert!(editor(&h).minimap().is_none());

    // inside a group: its body is what it maps (not the node of the top level)
    let mut h = harness(open(&spread_model(true), "canvas-minimap-in.toml"));
    h.run_steps(3);
    assert!(editor(&h).minimap().is_none(), "the top level fits");
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
    h.run_steps(40);
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(60.0, 60.0), 1.0);
    h.run_steps(3);
    let (m, world, s) = minimap_of(&h, &ids);
    assert!(world.width() > 5000.0, "the body's, {world:?}");
    let g = world.min + world.size() * 0.75;
    click(&mut h, on_minimap(m, world, s, g));
    h.run_steps(3);
    let c = view_centre(&h);
    assert!((c - g).length() < 2.0, "{c:?} not at {g:?}");
    assert_eq!(editor(&h).path, ["grp"]);
    terraflux_app::settings::update(|s| s.minimap = true);
}

/// The reroute points of a node's wires, by input.
fn points_of(h: &Harness<TerrafluxApp>, inside: bool, id: &str) -> std::collections::BTreeMap<String, Vec<[f32; 2]>> {
    nodes_now(h, inside).into_iter().find(|n| n.id == id).unwrap().wires
}

/// The graph point halfway along the wire into `node.port`, as it is drawn.
fn wire_mid(h: &Harness<TerrafluxApp>, node: &str, port: &str) -> egui::Pos2 {
    let w = editor(h).wire(node, port).expect("the wire is drawn");
    w[w.len() / 2]
}

fn graph_to_screen(h: &Harness<TerrafluxApp>, p: egui::Pos2) -> egui::Pos2 {
    editor(h).screen_at(p).unwrap()
}

/// A double click at a graph point.
fn double_click_graph(h: &mut Harness<TerrafluxApp>, p: egui::Pos2) {
    // (a pause first: clicks close to the last ones would be a triple click)
    h.run_steps(30);
    let at = graph_to_screen(h, p);
    double_click(h, at);
}

/// A double click halfway along the wire into `node.port`.
fn double_click_wire(h: &mut Harness<TerrafluxApp>, node: &str, port: &str) {
    let mid = wire_mid(h, node, port);
    double_click_graph(h, mid);
}

fn is_near(a: [f32; 2], b: egui::Pos2, within: f32) -> bool {
    egui::pos2(a[0], a[1]).distance(b) <= within
}

/// A double click on a wire adds a reroute point there, dragging it moves it, a double click on it (or right click,
/// "Remove point") takes it away: each one step of undo, and the wire is drawn through it. In a group too.
#[test]
fn a_wire_gets_reroute_points_that_are_added_moved_and_removed() {
    let _settings = settings_lock();
    use egui_kittest::kittest::Queryable;
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-points.toml");
        assert!(points_of(&h, inside, "b").is_empty());
        // added: a double click on the wire
        let mid = wire_mid(&h, "b", "map");
        double_click_graph(&mut h, mid);
        let pts = points_of(&h, inside, "b");
        assert_eq!(pts["map"].len(), 1, "inside {inside}: {pts:?}");
        let p = pts["map"][0];
        assert!(is_near(p, mid, 12.0), "{p:?} near {mid:?}");
        assert!(h.state().session.as_ref().unwrap().editor.message.is_none());
        assert!(
            editor(&h).wire("b", "map").unwrap().iter().any(|q| q.distance(egui::pos2(p[0], p[1])) < 1.0),
            "the wire goes through the point"
        );
        // one step of undo takes it away
        undo(&mut h);
        assert!(points_of(&h, inside, "b").is_empty(), "inside {inside}");
        double_click_wire(&mut h, "b", "map");
        let p = points_of(&h, inside, "b")["map"][0];
        // moved: dragged, one step of undo for the whole drag
        let z = editor(&h).zoom();
        let from = graph_to_screen(&h, egui::pos2(p[0], p[1]));
        drag(&mut h, from, from + egui::vec2(40.0, 60.0));
        let moved = points_of(&h, inside, "b")["map"][0];
        assert_eq!(points_of(&h, inside, "b")["map"].len(), 1);
        assert!(
            (moved[0] - p[0] - 40.0 / z).abs() < 2.0 && (moved[1] - p[1] - 60.0 / z).abs() < 2.0,
            "{p:?} -> {moved:?} at zoom {z}"
        );
        undo(&mut h);
        assert!(is_near(points_of(&h, inside, "b")["map"][0], egui::pos2(p[0], p[1]), 0.01), "one undo for the drag");
        // removed: a double click on it, then the menu's "Remove point"; each one step of undo
        double_click_graph(&mut h, egui::pos2(p[0], p[1]));
        assert!(points_of(&h, inside, "b").is_empty(), "inside {inside}");
        undo(&mut h);
        assert_eq!(points_of(&h, inside, "b")["map"].len(), 1);
        let at = graph_to_screen(&h, egui::pos2(p[0], p[1]));
        right_click(&mut h, at);
        pick(&mut h, "Remove point");
        assert!(points_of(&h, inside, "b").is_empty(), "inside {inside}");
        undo(&mut h);
        assert_eq!(points_of(&h, inside, "b")["map"].len(), 1);
        // (the menu is gone)
        assert!(h.query_by_label("Remove point").is_none());
    }
}

/// The points are saved with the model (the node's `wires`) and drawn again when it is opened; a model without
/// points has no `wires` in its text.
#[test]
fn reroute_points_are_saved_and_loaded() {
    let _settings = settings_lock();
    let mut m = selection_model();
    assert!(!m.to_toml().unwrap().contains("wires"));
    m.nodes[1].wires.insert("map".into(), vec![[250.0, -40.0], [280.0, 60.0]]);
    let text = m.to_toml().unwrap();
    assert!(text.contains("wires"), "{text}");
    let back = Model::from_toml(&text).unwrap();
    assert_eq!(back.nodes[1].wires["map"], vec![[250.0, -40.0], [280.0, 60.0]]);
    let mut h = harness(open(&back, "canvas-points-saved.toml"));
    h.run_steps(3);
    let path = editor(&h).wire("b", "map").unwrap();
    for p in [[250.0, -40.0], [280.0, 60.0]] {
        assert!(path.iter().any(|q| q.distance(egui::pos2(p[0], p[1])) < 1.0), "through {p:?}");
    }
}

/// A connection taken away, or a wire moved to another input, loses its points; so does a swap.
#[test]
fn a_wire_taken_off_loses_its_points() {
    let _settings = settings_lock();
    for inside in [false, true] {
        let mut h = scoped_model(inside, "canvas-points-gone.toml");
        double_click_wire(&mut h, "b", "map");
        assert_eq!(points_of(&h, inside, "b")["map"].len(), 1);
        // the connection taken off its input (a right click on the input): the points go with it
        let at = editor(&h).screen_of_input("b", "map").unwrap();
        right_click(&mut h, at);
        assert!(source_of(&h, inside, "b", "map").is_none());
        assert!(points_of(&h, inside, "b").is_empty(), "inside {inside}");
        // undone, the wire and its points are back (they are the model's)
        undo(&mut h);
        assert_eq!(points_of(&h, inside, "b")["map"].len(), 1);
    }
    // two wires swapped by dragging one over the other's input: neither keeps points it had
    for inside in [false, true] {
        let mut h = wired_model(inside, "canvas-points-swap.toml");
        double_click_wire(&mut h, "calc", "map");
        assert_eq!(points_of(&h, inside, "calc")["map"].len(), 1, "inside {inside}");
        let from = editor(&h).screen_of_input("calc", "map").unwrap();
        let to = editor(&h).screen_of_input("other", "map").unwrap();
        drag(&mut h, from, to);
        assert_eq!(source_of(&h, inside, "calc", "map").as_deref(), Some("r2.raster"));
        assert!(points_of(&h, inside, "calc").is_empty(), "inside {inside}");
        assert!(points_of(&h, inside, "other").is_empty());
    }
}

/// A duplicate (ctrl+D) carries the points of its wires, moved with the copy.
#[test]
fn a_duplicate_carries_the_reroute_points() {
    let _settings = settings_lock();
    for inside in [false, true] {
        let mut m = selection_model();
        m.nodes[1].wires.insert("map".into(), vec![[250.0, -40.0]]);
        let nodes = std::mem::take(&mut m.nodes);
        if inside {
            m.nodes.push(
                Node::new("grp", terraflux_graph::GROUP).body(terraflux_graph::Body { nodes, ..Default::default() }),
            );
        } else {
            m.nodes = nodes;
        }
        let mut h = harness(open(&m, "canvas-points-copy.toml"));
        h.run_steps(3);
        if inside {
            h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
            h.run_steps(40);
        }
        let s = h.state_mut().session.as_mut().unwrap();
        s.selected = Some("b".into());
        s.editor.chosen = ["b".to_string()].into();
        h.run_steps(2);
        let centre = editor(&h).canvas().unwrap().center();
        h.hover_at(centre);
        h.run_steps(2);
        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
        h.run_steps(3);
        let v = nodes_now(&h, inside);
        let (b, c) = (v.iter().find(|n| n.id == "b").unwrap(), v.iter().find(|n| n.id == "b_2").expect("the copy"));
        assert_eq!(b.wires["map"], vec![[250.0, -40.0]], "the original keeps its point");
        let (pb, pc) = (b.pos.unwrap(), c.pos.unwrap());
        let by = [pc[0] - pb[0], pc[1] - pb[1]];
        assert!(by != [0.0, 0.0], "the copy is moved");
        let got = c.wires["map"][0];
        assert!((got[0] - 250.0 - by[0]).abs() < 0.01 && (got[1] + 40.0 - by[1]).abs() < 0.01, "{got:?} by {by:?}");
    }
}

/// An input's box duplicated (ctrl+D) is another box of the same input, kept in the model under `#2`: each node
/// takes its wire from the nearest box, whichever is moved near it; one step of undo; the copy shows the input in
/// the pane; deleted, only its place goes; saved and read again, the copy is there.
#[test]
fn an_input_box_has_copies_and_each_reader_takes_the_nearest() {
    let _settings = settings_lock();
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let mut m = Model::new("copies");
    m.inputs.insert(
        "t0".into(),
        terraflux_graph::Input::Map {
            path: data.to_string_lossy().into_owned(),
            band: None,
            nodata: None,
            note: None,
            data: None,
        },
    );
    let mut low = Node::new("low", "raster.calc").input("land", "$t0").param("expression", "land");
    low.pos = Some([0.0, 0.0]);
    let mut high = Node::new("high", "raster.calc").input("land", "$t0").param("expression", "land");
    high.pos = Some([0.0, -700.0]);
    m.nodes.extend([low, high]);
    let mut h = harness(open(&m, "canvas-copies.toml"));
    h.run_steps(3);
    assert_eq!(editor(&h).wired_from("high", "land").as_deref(), Some("$t0"));
    // selected and duplicated: the copy is a box of its own, below, selected, with the input in the pane
    let at = editor(&h).screen_of("$t0").unwrap();
    click(&mut h, at);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.source.places.contains_key("|$t0#2"), "{:?}", s.source.places);
        assert_eq!(s.selected.as_deref(), Some("$t0#2"));
        assert_eq!(s.source.inputs.len(), 1);
    }
    let copy = editor(&h).screen_of("$t0#2").expect("the copy is on the canvas");
    assert!(copy.y > at.y, "below the first");
    // (the original is nearer to both: the copy has no wires yet, and shows anyway)
    assert_eq!(editor(&h).wired_from("high", "land").as_deref(), Some("$t0"));
    assert_eq!(editor(&h).wired_from("low", "land").as_deref(), Some("$t0"));
    // moved near the node above: that node takes its wire from it, the other keeps the first
    let port = editor(&h).screen_of_input("high", "land").unwrap();
    drag(&mut h, copy, port - egui::vec2(160.0, 0.0));
    let places = &h.state().session.as_ref().unwrap().source.places;
    assert_eq!(editor(&h).wired_from("high", "land").as_deref(), Some("$t0#2"), "{places:?} {copy:?} {port:?}");
    assert_eq!(editor(&h).wired_from("low", "land").as_deref(), Some("$t0"));
    // the model reads as it did: a place kept under the key, and saved and read again, the copy is there
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("canvas-copies-saved.toml");
    let saved = h.state().session.as_ref().unwrap().source.clone();
    assert_eq!(saved.nodes[0].inputs["land"], terraflux_graph::Source::Ref("$t0".into()));
    saved.save(&path).unwrap();
    let again = Model::load(&path).unwrap();
    assert_eq!(again.places, saved.places);
    assert!(again.places.contains_key("|$t0#2"));
    // one step of undo each: the move, then the duplicate
    undo(&mut h);
    undo(&mut h);
    assert!(!h.state().session.as_ref().unwrap().source.places.contains_key("|$t0#2"));
    assert!(editor(&h).screen_of("$t0#2").is_none());
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), true);
    h.run_steps(2);
    assert!(editor(&h).screen_of("$t0#2").is_some(), "redone");
    // the copy deleted: only its place; the readers keep the input
    let copy = editor(&h).screen_of("$t0#2").unwrap();
    click(&mut h, copy);
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(!s.source.places.contains_key("|$t0#2") && s.source.inputs.contains_key("t0"));
    assert_eq!(s.source.nodes[0].inputs["land"], terraflux_graph::Source::Ref("$t0".into()));
    assert_eq!(editor(&h).wired_from("high", "land").as_deref(), Some("$t0"));
    // the last box deleted: the input goes and what read it reads nothing, as it did
    let first = editor(&h).screen_of("$t0").unwrap();
    click(&mut h, first);
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.source.inputs.is_empty() && !s.source.nodes[0].inputs.contains_key("land"));
}

/// A box on a wire's way: `a -> b` far apart at the same height with `block` between.
fn blocked_model() -> Model {
    let mut b = Node::new("b", "raster.reclassify").input("map", "a.raster");
    b.pos = Some([900.0, 0.0]);
    let mut m = Model::new("blocked");
    m.nodes = vec![reader("a", [0.0, 0.0]), b, reader("block", [420.0, 0.0])];
    m
}

fn crosses(path: &[egui::Pos2], r: egui::Rect) -> bool {
    path.iter().any(|q| r.contains(*q))
}

/// A wire that would cross another node's box goes around it (above or below, by the shorter way) unless it has
/// points of its own or the setting is off, when it is the plain curve.
#[test]
fn a_wire_across_a_box_goes_around_it() {
    let _settings = settings_lock();
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::settings::update(|s| s.wires_around = true);
    let mut h = harness(open(&blocked_model(), "canvas-around.toml"));
    h.run_steps(3);
    let block = editor(&h).rects_of(&["block".to_string()])[0].1;
    // on: bent, and the path it is drawn along clears the box, from the output to the input
    assert!(editor(&h).bends("b", "map") > 0, "bent");
    let path = editor(&h).wire("b", "map").unwrap();
    assert!(!crosses(&path, block), "the path crosses {block:?}");
    let (a_out, b_in) = (path[0], *path.last().unwrap());
    assert!(a_out.x < block.left() && b_in.x > block.right());
    // off: the plain curve, straight across the box
    terraflux_app::settings::update(|s| s.wires_around = false);
    h.run_steps(3);
    assert_eq!(editor(&h).bends("b", "map"), 0);
    let plain = editor(&h).wire("b", "map").unwrap();
    assert!(crosses(&plain, block), "the plain wire crosses the box");
    assert!(plain.iter().all(|q| (q.y - plain[0].y).abs() < 1.0), "a straight wire");
    // a wire with a point of its own is left as drawn, even when the setting is on
    terraflux_app::settings::update(|s| s.wires_around = true);
    h.run_steps(3);
    assert!(editor(&h).bends("b", "map") > 0);
    double_click_wire(&mut h, "b", "map");
    let s = h.state().session.as_ref().unwrap();
    let own = s.source.nodes.iter().find(|n| n.id == "b").unwrap().wires.get("map").map_or(0, Vec::len);
    assert_eq!(own, 1, "a point of its own was added: {:?}", s.source.nodes[1].wires);
    assert_eq!(editor(&h).bends("b", "map"), 0, "its own points replace the bends");
    // the box dragged out of the way: the wire is straight again
    undo(&mut h);
    assert!(editor(&h).bends("b", "map") > 0);
    let from = editor(&h).screen_of("block").unwrap();
    drag(&mut h, from, from + egui::vec2(0.0, 400.0));
    assert_eq!(editor(&h).bends("b", "map"), 0, "nothing in the way now");
    terraflux_app::settings::update(|s| s.wires_around = true);
}

/// Run the model open and step until it is done.
fn run_model(h: &mut Harness<TerrafluxApp>) {
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().running() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "the run took too long");
    }
    h.run_steps(3);
}

/// Where a wire is on the screen: the middle of its two ends (a connection's curve goes through it).
fn on_wire(h: &Harness<TerrafluxApp>, from: (&str, &str), to: (&str, &str)) -> egui::Pos2 {
    let e = editor(h);
    let (a, b) = (e.screen_of_output(from.0, from.1).unwrap(), e.screen_of_input(to.0, to.1).unwrap());
    a + (b - a) / 2.0
}

/// The pointer rests on the wire for `steps` frames of a sixtieth of a second.
fn rest_on(h: &mut Harness<TerrafluxApp>, at: egui::Pos2, steps: usize) {
    h.hover_at(at);
    h.run_steps(steps);
}

fn map_model(inside: bool) -> Model {
    let mut calc = Node::new("calc", "raster.calc").param("expression", "a * 2").input("a", "read.raster");
    calc.pos = Some([450.0, 0.0]);
    let nodes = vec![reader("read", [0.0, 0.0]).param("band", "0"), calc];
    let mut m = Model::new("tip");
    if inside {
        let body = terraflux_graph::Body {
            nodes,
            outputs: [("map".to_string(), "calc.map".to_string())].into(),
            ..Default::default()
        };
        m.nodes.push(Node::new("grp", terraflux_graph::GROUP).body(body));
    } else {
        m.nodes = nodes;
    }
    m
}

/// The pointer resting on a connection: a tooltip with its type and a summary of its last value (a map's size, cells
/// and range); before a run it says it did not run, after an edit that it changed; in a group the same; not
/// while dragging.
#[test]
fn a_connection_tells_its_type_and_value() {
    let _settings = settings_lock();
    use egui_kittest::kittest::Queryable;
    for inside in [false, true] {
        let mut h = harness(open(&map_model(inside), "canvas-wire-tip.toml"));
        h.run_steps(3);
        if inside {
            h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
            h.run_steps(40);
        }
        let at = on_wire(&h, ("read", "raster"), ("calc", "a"));
        // before a run
        rest_on(&mut h, at, 3);
        assert!(h.query_by_label_contains("not run yet").is_none(), "not before the pointer rests");
        rest_on(&mut h, at, 60);
        assert!(h.query_by_label_contains("not run yet").is_some(), "inside {inside}: before a run");
        // after one: the type of the wire and the map's size
        h.hover_at(at + egui::vec2(0.0, 300.0));
        h.run_steps(2);
        run_model(&mut h);
        let s = h.state().session.as_ref().unwrap();
        let place = if inside { "grp/read" } else { "read" };
        let Some(terraflux_ops::Value::Raster(r)) = s.value_of(place, "raster") else {
            panic!("inside {inside}: the value of {place} is not kept")
        };
        let g = r.grid_of(r.chosen);
        let size = format!("{} × {}", g.width, g.height);
        rest_on(&mut h, at, 60);
        assert!(h.query_by_label_contains(&size).is_some(), "inside {inside}: the size {size}");
        // not while dragging (the pointer pressed and moved on the background)
        h.hover_at(at + egui::vec2(0.0, 300.0));
        h.run_steps(2);
        assert!(h.query_by_label_contains(&size).is_none(), "gone when the pointer leaves");
        h.hover_at(at);
        h.run_steps(1);
        h.drag_at(at);
        h.run_steps(1);
        for k in 1..=4 {
            h.hover_at(at + egui::vec2(0.0, 6.0 * k as f32));
            h.run_steps(1);
        }
        h.hover_at(at + egui::vec2(0.0, 24.0));
        h.run_steps(60);
        assert!(h.query_by_label_contains(&size).is_none(), "inside {inside}: no tooltip while dragging");
        h.drop_at(at + egui::vec2(0.0, 24.0));
        h.run_steps(2);
    }
}

/// A connection that carries a number shows the number; after an edit it says the value is out of date.
#[test]
fn a_connection_of_a_number_shows_the_number() {
    let _settings = settings_lock();
    use egui_kittest::kittest::Queryable;
    let mut a = Node::new("a", "value.calc").param("expression", "6 * 7");
    a.pos = Some([0.0, 0.0]);
    let mut b = Node::new("b", "value.calc").input("v", "a.value").param("expression", "v + 1");
    b.pos = Some([450.0, 0.0]);
    let mut m = Model::new("number");
    m.nodes = vec![a, b];
    let mut h = harness(open(&m, "canvas-wire-number.toml"));
    h.run_steps(3);
    run_model(&mut h);
    let at = on_wire(&h, ("a", "value"), ("b", "v"));
    rest_on(&mut h, at, 60);
    // (the whole label: the version in the bar, 1.42.0, has "42" too)
    assert!(h.query_by_label("42").is_some(), "the number");
    assert!(h.query_by_label_contains("Number").is_some(), "its kind");
    // an edit: what the nodes gave may be out of date
    h.hover_at(at + egui::vec2(0.0, 300.0));
    h.run_steps(2);
    // (an edit of what it reads; an edit elsewhere leaves it)
    let s = h.state_mut().session.as_mut().unwrap();
    s.source.nodes[0].params.insert("expression".into(), "6 * 8".into());
    s.edited();
    h.run_steps(3);
    rest_on(&mut h, at, 60);
    assert!(h.query_by_label_contains("changed since the last run").is_some());
    assert!(h.query_by_label("42").is_none());
}

/// A note put on the empty canvas and a frame around two nodes: the frame dragged by its title drags its nodes, a
/// node let go in it joins it, a note selected is deleted by Delete, each a step of undo.
#[test]
fn notes_and_frames_are_put_on_the_canvas_and_follow_their_nodes() {
    use egui_kittest::kittest::Queryable;
    let mut m = Model::new("noted");
    m.nodes = vec![reader("a", [0.0, 0.0]), reader("b", [0.0, 220.0]), reader("z", [700.0, 0.0])];
    let mut h = harness(open(&m, "canvas-notes.toml"));
    h.run_steps(3);
    let source = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().source.clone();
    let pos = |h: &Harness<TerrafluxApp>, id: &str| source(h).nodes.iter().find(|n| n.id == id).unwrap().pos.unwrap();
    let typed = |h: &mut Harness<TerrafluxApp>, text: &str| {
        let s = h.state_mut().session.as_mut().unwrap();
        s.editor.note_editing.as_mut().expect("the text asked for").1 = text.to_string();
        h.run_steps(1);
        h.get_by_label("OK").click();
        h.run_steps(3);
    };
    // a note on the empty canvas, its text typed at once
    let empty = editor(&h).screen_at(egui::pos2(300.0, 520.0)).unwrap();
    right_click(&mut h, empty);
    h.get_by_label("Add a note here").click();
    h.run_steps(3);
    assert!(h.query_by_label("For a frame, the first line is its title.").is_some());
    typed(&mut h, "Read the land first");
    let notes = source(&h).notes;
    assert_eq!(notes.len(), 1);
    assert_eq!(notes[0].text, "Read the land first");
    let at = notes[0].at.unwrap();
    assert!((at[0] - 300.0).abs() < 2.0 && (at[1] - 520.0).abs() < 2.0, "{at:?}");
    // a frame around a and b, from the selection's menu
    h.state_mut().session.as_mut().unwrap().editor.chosen = ["a".to_string(), "b".to_string()].into();
    h.run_steps(1);
    let a = editor(&h).screen_of("a").unwrap();
    right_click(&mut h, a);
    h.get_by_label("Frame them").click();
    h.run_steps(3);
    typed(&mut h, "Inputs");
    let frame = source(&h).notes.into_iter().find(|n| n.is_frame()).expect("a frame");
    assert_eq!((frame.title(), frame.nodes.clone()), ("Inputs", vec!["a".to_string(), "b".to_string()]));
    h.state_mut().session.as_mut().unwrap().editor.chosen.clear();
    h.run_steps(2);
    if let Ok(img) = h.render() {
        img.save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ui-notes.png")).unwrap();
    }
    // dragged by its title: a and b with it, z where it was
    let (a0, b0, z0) = (pos(&h, "a"), pos(&h, "b"), pos(&h, "z"));
    let title = editor(&h).screen_at(egui::pos2(a0[0] + 30.0, a0[1] - 28.0)).unwrap();
    drag(&mut h, title, title + egui::vec2(120.0, 60.0));
    let (a1, b1) = (pos(&h, "a"), pos(&h, "b"));
    assert!(a1[0] > a0[0] + 20.0 && a1[1] > a0[1] + 10.0, "{a0:?} -> {a1:?}");
    assert_eq!([a1[0] - a0[0], a1[1] - a0[1]], [b1[0] - b0[0], b1[1] - b0[1]], "the frame's nodes moved together");
    assert_eq!(pos(&h, "z"), z0);
    // z let go between a and b: in the frame
    let z = editor(&h).screen_of("z").unwrap();
    let into = editor(&h).screen_at(egui::pos2(a1[0] + 40.0, a1[1] + 120.0)).unwrap();
    drag(&mut h, z, into);
    let frame = source(&h).notes.into_iter().find(|n| n.is_frame()).unwrap();
    assert!(frame.nodes.contains(&"z".to_string()), "{:?} at {:?}", frame.nodes, pos(&h, "z"));
    // the note clicked, Delete: gone, not the nodes
    let note = editor(&h).screen_at(egui::pos2(at[0] + 40.0, at[1] + 12.0)).unwrap();
    click(&mut h, note);
    assert_eq!(editor(&h).note_chosen.as_deref(), Some("note1"));
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    assert_eq!(source(&h).notes.len(), 1);
    assert_eq!(source(&h).nodes.len(), 3);
    // undone: back
    undo(&mut h);
    assert_eq!(source(&h).notes.len(), 2);
}

/// Two independent chains (`a` to `b`, `c` to `d`) and a group of two nodes (`g`: `x` to `y`) read by `e`, all done
/// as after a run.
fn ran_session(name: &str) -> TerrafluxApp {
    use terraflux_app::editor_view::NodeStatus;
    let mut m = Model::new("stale");
    m.nodes.push(Node::new("a", "raster.calc").param("expression", "1"));
    m.nodes.push(Node::new("b", "raster.calc").param("expression", "a").input("a", "a.map"));
    m.nodes.push(Node::new("c", "raster.calc").param("expression", "2"));
    m.nodes.push(Node::new("d", "raster.calc").param("expression", "a").input("a", "c.map"));
    let body = terraflux_graph::Body {
        nodes: vec![
            Node::new("x", "raster.calc").param("expression", "3"),
            Node::new("y", "raster.calc").param("expression", "a").input("a", "x.map"),
            Node::new("z", "raster.calc").param("expression", "4"),
        ],
        ..Default::default()
    };
    m.nodes.push(Node::new("g", terraflux_graph::GROUP).body(body));
    m.nodes.push(Node::new("e", "raster.calc").param("expression", "a").input("a", "g.map"));
    let mut app = open(&m, name);
    let s = app.session.as_mut().unwrap();
    for id in ["a", "b", "c", "d", "g", "e"] {
        s.status.insert(id.into(), NodeStatus::Done { cached: false });
    }
    for id in ["g/x", "g/y", "g/z"] {
        s.inner_status.insert(id.into(), NodeStatus::Done { cached: false });
    }
    s.ran_model = Some(s.model.clone());
    app
}

fn kept(app: &TerrafluxApp) -> Vec<String> {
    let s = app.session.as_ref().unwrap();
    s.status.keys().cloned().chain(s.inner_status.keys().cloned()).collect()
}

/// An edit takes the status of the node it changes and of what reads it, all the way down; the other chain keeps
/// its own (and so its wires as fresh).
#[test]
fn an_edit_invalidates_only_what_it_reaches() {
    let mut app = ran_session("canvas-stale-reach.toml");
    let s = app.session.as_mut().unwrap();
    s.source.nodes[1].params.insert("expression".into(), "a + 1".into());
    s.edited();
    assert_eq!(kept(&app), ["a", "c", "d", "e", "g", "g/x", "g/y", "g/z"]);
    // upstream: every one downstream goes
    let s = app.session.as_mut().unwrap();
    s.source.nodes[2].params.insert("expression".into(), "5".into());
    s.edited();
    assert_eq!(kept(&app), ["a", "e", "g", "g/x", "g/y", "g/z"]);
}

/// Moving a node, or writing a note, changes nothing that was computed.
#[test]
fn moving_a_node_or_a_note_invalidates_nothing() {
    let mut app = ran_session("canvas-stale-visual.toml");
    let all = kept(&app);
    let s = app.session.as_mut().unwrap();
    s.source.nodes[0].pos = Some([40.0, 50.0]);
    s.source.nodes[1].note = Some("why".into());
    s.edited();
    assert_eq!(kept(&app), all);
}

/// An edit inside a group takes the group and what reads it, and what reads the node inside, not the others.
#[test]
fn an_edit_inside_a_group_invalidates_the_group_and_its_readers() {
    let mut app = ran_session("canvas-stale-group.toml");
    let s = app.session.as_mut().unwrap();
    s.source.nodes[4].body.as_mut().unwrap().nodes[0].params.insert("expression".into(), "9".into());
    s.edited();
    assert_eq!(kept(&app), ["a", "b", "c", "d", "g/z"]);
}

/// Inside an open group, a connection dragged from an inner node's output to the edge of the scope (a zone drawn
/// while it is dragged) makes it an output of the group, which shows on the group's box outside; the same port
/// again changes nothing; one step of undo takes it away.
#[test]
fn a_connection_let_go_on_the_edge_of_a_group_makes_an_output() {
    use terraflux_graph::Body;
    let mut m = Model::new("edge");
    let body = Body { nodes: vec![reader("r", [0.0, 0.0])], ..Default::default() };
    m.nodes.push(Node::new("grp", terraflux_graph::GROUP).body(body));
    let mut h = harness(open(&m, "canvas-edge.toml"));
    h.run_steps(3);
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
    h.run_steps(40);
    let outputs = |h: &Harness<TerrafluxApp>| {
        let s = h.state().session.as_ref().unwrap();
        s.source.node("grp").unwrap().body.as_ref().unwrap().outputs.clone()
    };
    assert!(outputs(&h).is_empty());
    let canvas = editor(&h).canvas().unwrap();
    let edge = egui::pos2(canvas.right() - 40.0, canvas.center().y);
    let from = editor(&h).screen_of_output("r", "raster").unwrap();
    drag(&mut h, from, edge);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none(), "no search: the edge takes it");
    assert_eq!(outputs(&h).get("raster").map(String::as_str), Some("r.raster"), "{:?}", s.editor.message);
    // the group's port is on its box outside
    h.state_mut().session.as_mut().unwrap().editor.path.clear();
    h.run_steps(40);
    assert!(editor(&h).screen_of_output("grp", "raster").is_some(), "the group's output port");
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["grp".into()];
    h.run_steps(40);
    // the same again: not twice
    let from = editor(&h).screen_of_output("r", "raster").unwrap();
    drag(&mut h, from, edge);
    assert_eq!(outputs(&h).len(), 1, "{:?}", outputs(&h));
    // one step of undo takes it away
    undo(&mut h);
    assert!(outputs(&h).is_empty(), "undone");
    // a wire let go short of the edge is the search, as before
    let from = editor(&h).screen_of_output("r", "raster").unwrap();
    drag(&mut h, from, from + egui::vec2(200.0, 120.0));
    assert!(h.state().session.as_ref().unwrap().editor.quick.is_some());
    assert!(outputs(&h).is_empty());
}

/// The panel of a group lists its outputs with their names to edit and arrows to order them: a name typed (Enter)
/// renames the output and what reads it follows, one step of undo; the arrows change the order its box shows its
/// ports in, one step of undo each.
#[test]
fn a_groups_outputs_are_renamed_and_ordered_in_its_panel() {
    use egui_kittest::kittest::{NodeT, Queryable};
    use terraflux_graph::{Body, Source};
    let _lock = settings_lock();
    let mut m = Model::new("outs");
    let mut grp = Node::new("grp", terraflux_graph::GROUP).body(Body {
        nodes: vec![reader("r", [0.0, 0.0]), reader("q", [0.0, 200.0])],
        outputs: [("alpha".to_string(), "r.raster".to_string()), ("beta".to_string(), "q.raster".to_string())].into(),
        ..Default::default()
    });
    grp.pos = Some([0.0, 0.0]);
    let mut rc = Node::new("rc", "raster.reclassify").input("map", "grp.alpha");
    rc.pos = Some([600.0, 0.0]);
    m.nodes = vec![grp, rc];
    let mut h = harness(open(&m, "canvas-group-outputs.toml"));
    h.run_steps(3);
    let state = |h: &Harness<TerrafluxApp>| {
        let s = h.state().session.as_ref().unwrap();
        let b = s.source.node("grp").unwrap().body.as_ref().unwrap();
        let names: Vec<String> = b.output_names().into_iter().cloned().collect();
        (names, s.source.node("rc").unwrap().inputs["map"].clone())
    };
    let s = h.state_mut().session.as_mut().unwrap();
    s.selected = Some("grp".into());
    s.editor.chosen = ["grp".to_string()].into();
    s.editor.panel = Some(Vec::new());
    h.run_steps(6);
    assert_eq!(state(&h).0, ["alpha", "beta"]);
    // the first one down: the order the box shows
    h.query_all_by_label("↓").next().expect("a down arrow").click();
    h.run_steps(3);
    assert_eq!(state(&h).0, ["beta", "alpha"]);
    let s = h.state().session.as_ref().unwrap();
    let ports =
        terraflux_app::editor::output_ports(s.source.node("grp").unwrap(), &terraflux_ops::Registry::standard());
    assert_eq!(ports.iter().map(|p| p.0.as_str()).collect::<Vec<_>>(), ["beta", "alpha"]);
    undo(&mut h);
    assert_eq!(state(&h).0, ["alpha", "beta"], "one step of undo");
    // renamed: the node that reads it follows; one step of undo takes both back
    let name = h.query_all_by_value("alpha").find(|n| n.accesskit_node().role() == egui::accesskit::Role::TextInput);
    name.expect("the name's field").click();
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("gamma".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(4);
    let (names, read) = state(&h);
    assert_eq!(names, ["gamma", "beta"], "its place kept");
    assert_eq!(read, Source::Ref("grp.gamma".into()));
    undo(&mut h);
    assert_eq!(state(&h), (vec!["alpha".to_string(), "beta".to_string()], Source::Ref("grp.alpha".into())));
}

/// A free input's wire dragged backwards: on an output that fits it connects (one step of undo), on one that does
/// not nothing changes; on the empty canvas the search lists only what gives its type, the one chosen is added
/// there and wired into the input; escape leaves the model as it was.
#[test]
fn a_free_inputs_wire_dragged_to_an_output_or_the_empty_canvas() {
    use egui_kittest::kittest::Queryable;
    let mut rc = Node::new("rc", "raster.reclassify");
    rc.pos = Some([500.0, 0.0]);
    let mut tb = Node::new("tb", "table.read");
    tb.pos = Some([0.0, 300.0]);
    let mut m = Model::new("feed");
    m.nodes = vec![reader("read", [0.0, 0.0]), tb, rc];
    let mut h = harness(open(&m, "canvas-feed.toml"));
    h.run_steps(3);
    let nodes = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().source.nodes.clone();
    let before = nodes(&h);
    // on an output that does not fit: nothing
    let from = editor(&h).screen_of_input("rc", "map").unwrap();
    let to = editor(&h).screen_of_output("tb", "table").unwrap();
    drag(&mut h, from, to);
    assert_eq!(nodes(&h), before, "a table does not fit a map");
    assert!(h.state().session.as_ref().unwrap().editor.quick.is_none());
    // on one that fits: connected
    let to = editor(&h).screen_of_output("read", "raster").unwrap();
    drag(&mut h, from, to);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.node("rc").unwrap().inputs["map"], terraflux_graph::Source::Ref("read.raster".into()));
    undo(&mut h);
    assert_eq!(nodes(&h), before, "one step of undo");
    // on the empty canvas: the search of what gives what it takes
    let drop = from + egui::vec2(-150.0, 380.0);
    drag(&mut h, from, drop);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.as_ref().is_some_and(|q| q.feed.is_some()), "the search opens where it was let go");
    assert_eq!(nodes(&h), before, "nothing added yet");
    h.run_steps(2);
    assert!(h.query_by_label_contains("for rc.map").is_some(), "the title says what for");
    assert!(h.query_by_label_contains("raster.read").is_some(), "an operator that gives a raster");
    assert!(h.query_by_label_contains("table.read").is_none(), "no operator that gives only a table");
    // escape: as it was
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none());
    assert_eq!(nodes(&h), before);
    // the one chosen is added there and wired in
    drag(&mut h, from, drop);
    h.event(egui::Event::Text("raster.read".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none());
    assert_eq!(s.source.nodes.len(), 4);
    let n = s.source.nodes.last().unwrap();
    assert_eq!(n.op, "raster.read");
    assert_eq!(
        s.source.node("rc").unwrap().inputs["map"],
        terraflux_graph::Source::Ref(format!("{}.raster", n.id)),
        "{:?}",
        s.editor.message
    );
    let (p, q) = (n.pos.unwrap(), editor(&h).graph_at(drop).unwrap());
    assert!((p[1] - q.y).abs() < 1.0 && p[0] < q.x && q.x - p[0] < 400.0, "{p:?} at {q:?}");
    assert!(s.editor.message.is_none(), "{:?}", s.editor.message);
    // one step of undo takes the node and the wire away
    undo(&mut h);
    assert_eq!(nodes(&h), before);
}

/// A node that gives a number (`ent.index`), a model's input of a value (`$rate`) and a node of many settings (`opt`:
/// `gap`, `seed` and `solver` given), their boxes in view.
fn pins_harness(name: &str) -> Harness<'static, TerrafluxApp> {
    let mut m = Model::new("pins");
    m.inputs.insert("rate".into(), terraflux_graph::Input::Value { value: 0.5.into(), note: None });
    let mut ent = Node::new("ent", "fragmentation.entropy");
    ent.pos = Some([0.0, 0.0]);
    let mut opt = Node::new("opt", "prioritize.optimize").param("gap", 0.2).param("seed", 3).param("solver", "highs");
    opt.pos = Some([700.0, 0.0]);
    m.nodes = vec![ent, opt];
    let mut h = harness(open(&m, name));
    h.state_mut().session.as_mut().unwrap().editor.fit();
    h.run_steps(6);
    h
}

/// A setting that may read a value has a socket on its row; a wire dragged from an output onto it (or anywhere on the
/// row) makes the setting read it (`{ from }` for a node's output, `"$name"` for a model's input), drawn as a wire
/// to the socket that follows the boxes; a wire that does not fit is refused with the reason; each is one step of
/// undo.
#[test]
fn a_wire_dropped_on_a_setting_makes_it_read_the_value() {
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut h = pins_harness("canvas-pins.toml");
    let before = nodes_now(&h, false);
    assert_eq!(editor(&h).setting_wires(), 0);
    // (a socket only for a setting that takes a number, a yes or no or a text)
    let feed = |h: &Harness<TerrafluxApp>, name: &str| {
        editor(h).settings_of("opt").unwrap().rows.iter().find(|r| r.name == name).and_then(|r| r.feed)
    };
    assert_eq!(feed(&h, "gap"), Some(terraflux_graph::Type::Number));
    assert_eq!(feed(&h, "solver"), Some(terraflux_graph::Type::Text));
    // a node's output let go on the socket
    let from = editor(&h).screen_of_output("ent", "index").unwrap();
    let socket = editor(&h).screen_of_socket("opt", "gap").unwrap();
    drag(&mut h, from, socket);
    assert_eq!(
        param_now(&h, "opt", "gap"),
        Some(serde_json::json!({ "from": "ent.index" })),
        "{:?}",
        editor(&h).message
    );
    let (source, a, z) = editor(&h).setting_wire("opt", "gap").expect("a wire to the socket");
    assert_eq!(source, "ent");
    assert!(a.distance(editor(&h).screen_of_output("ent", "index").unwrap()) < 1.0);
    assert!(z.distance(editor(&h).screen_of_socket("opt", "gap").unwrap()) < 1.0);
    assert_eq!(editor(&h).setting_wires(), 1);
    let row = editor(&h).settings_of("opt").unwrap().rows.iter().find(|r| r.name == "gap").unwrap().clone();
    assert!(row.reads && row.value == "← ent.index", "{row:?}");
    // the wire follows the box dragged
    let head = editor(&h).screen_of("ent").unwrap();
    drag(&mut h, head, head + egui::vec2(-20.0, 90.0));
    let (_, a2, z2) = editor(&h).setting_wire("opt", "gap").unwrap();
    assert!(
        (a2 - a).y > 50.0 && a2.distance(editor(&h).screen_of_output("ent", "index").unwrap()) < 1.0,
        "{a} -> {a2}"
    );
    assert!(z2.distance(z) < 1.0, "the socket stays");
    // one step of undo takes it back
    undo(&mut h);
    undo(&mut h);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(0.2)));
    let params = |h: &Harness<TerrafluxApp>| nodes_now(h, false).into_iter().map(|n| n.params).collect::<Vec<_>>();
    assert_eq!(params(&h), before.iter().map(|n| n.params.clone()).collect::<Vec<_>>());
    assert_eq!(editor(&h).setting_wires(), 0);
    // what does not fit is refused, with the reason, and the model stays
    let from = editor(&h).screen_of_output("ent", "pairs").unwrap();
    let socket = editor(&h).screen_of_socket("opt", "gap").unwrap();
    drag(&mut h, from, socket);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(0.2)));
    let why = editor(&h).message.clone().unwrap_or_default();
    assert!(why.contains("Table"), "{why:?}");
    // a model's input of a value, let go anywhere on the row: read as `"$rate"`
    let from = editor(&h).screen_of_output("$rate", "").unwrap();
    let on_row = editor(&h).screen_of_setting("opt", "seed").unwrap();
    drag(&mut h, from, on_row);
    assert_eq!(param_now(&h, "opt", "seed"), Some(serde_json::json!("$rate")), "{:?}", editor(&h).message);
    assert_eq!(editor(&h).setting_wire("opt", "seed").map(|w| w.0), Some("$rate".to_string()));
    // (a row that reads a value is edited in the settings, not on the box)
    let kind = editor(&h).settings_of("opt").unwrap().rows.iter().find(|r| r.name == "seed").unwrap().kind;
    assert_eq!(kind, terraflux_app::editor_view::Kind::Complex);
    // the wire taken off the socket: the setting has its own value again (the default), one step of undo
    let socket = editor(&h).screen_of_socket("opt", "seed").unwrap();
    drag(&mut h, socket, socket + egui::vec2(-150.0, 250.0));
    assert_eq!(param_now(&h, "opt", "seed"), None);
    assert_eq!(editor(&h).setting_wires(), 0);
    undo(&mut h);
    assert_eq!(param_now(&h, "opt", "seed"), Some(serde_json::json!("$rate")));
    // right click on the row: "Use a value of its own"
    let on_row = editor(&h).screen_of_setting("opt", "seed").unwrap();
    right_click(&mut h, on_row);
    pick(&mut h, "Use a value of its own");
    assert_eq!(param_now(&h, "opt", "seed"), None);
    undo(&mut h);
    assert_eq!(param_now(&h, "opt", "seed"), Some(serde_json::json!("$rate")));
}

/// A setting a node's box shows is chosen by the node too: "Hide from the node" on a row folds it (the row that
/// unfolds the others counts it), "Show on the node" in the node's menu puts a folded one back, always; both are
/// kept in the model, saved and loaded, one step of undo each. A setting that reads a value is always shown.
#[test]
fn a_setting_is_hidden_from_the_box_and_shown_on_it() {
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut h = pins_harness("canvas-hide.toml");
    let names = |h: &Harness<TerrafluxApp>| -> Vec<String> {
        editor(h).settings_of("opt").unwrap().rows.iter().map(|r| r.name.clone()).collect()
    };
    let folded = |h: &Harness<TerrafluxApp>| editor(h).settings_of("opt").unwrap().clone();
    assert!(names(&h).contains(&"seed".to_string()), "{:?}", names(&h));
    let (rows, count) = (names(&h).len(), folded(&h).folded);
    assert_eq!(folded(&h).folded_given, 0);
    let at = row_of(&mut h, "opt", "seed");
    right_click(&mut h, at);
    pick(&mut h, "Hide from the node");
    assert!(!names(&h).contains(&"seed".to_string()), "{:?}", names(&h));
    let set = folded(&h);
    // (the row it leaves is taken by the next essential one: as many folded as before, and the hidden one counted
    // among the given)
    assert_eq!((set.folded, set.folded_given), (count, 1), "{set:?}: a given one hidden is still counted");
    assert_eq!(nodes_now(&h, false).iter().find(|n| n.id == "opt").unwrap().hidden, ["seed"]);
    // saved with the model, loaded again
    let text = h.state().session.as_ref().unwrap().source.to_toml().unwrap();
    let back = Model::from_toml(&text).unwrap();
    assert_eq!(back.node("opt").unwrap().hidden, ["seed"]);
    assert!(back.node("ent").unwrap().hidden.is_empty() && !text.contains("shown"), "{text}");
    // the node's menu offers the folded ones: a click shows it, always
    let head = editor(&h).screen_of("opt").unwrap();
    right_click(&mut h, head);
    use egui_kittest::kittest::Queryable;
    h.get_by_label_contains("Show on the node").click();
    h.run_steps(3);
    h.get_by_label("seed").click();
    h.run_steps(3);
    assert!(names(&h).contains(&"seed".to_string()) && names(&h).len() == rows, "{:?}", names(&h));
    let opt = nodes_now(&h, false).into_iter().find(|n| n.id == "opt").unwrap();
    assert_eq!((opt.shown, opt.hidden), (vec!["seed".to_string()], Vec::<String>::new()));
    // one step of undo each
    undo(&mut h);
    assert!(!names(&h).contains(&"seed".to_string()));
    undo(&mut h);
    assert_eq!(nodes_now(&h, false).iter().find(|n| n.id == "opt").unwrap().hidden, Vec::<String>::new());
    assert!(names(&h).contains(&"seed".to_string()));
    // a setting that reads a value stays on the box, hidden or not
    let from = editor(&h).screen_of_output("$rate", "").unwrap();
    let socket = editor(&h).screen_of_socket("opt", "seed").unwrap();
    drag(&mut h, from, socket);
    assert!(names(&h).contains(&"seed".to_string()));
    h.state_mut().session.as_mut().unwrap().source.node_mut("opt").unwrap().hidden.push("seed".into());
    h.state_mut().session.as_mut().unwrap().editor.rebuild();
    h.run_steps(3);
    assert!(names(&h).contains(&"seed".to_string()), "a setting that reads a value is not hidden");
}

/// The panel opened from a setting the box does not edit is scrolled to it (a long list of settings: the one
/// clicked far down is in view, marked for a moment); opened from the gear it stays at the top.
#[test]
fn the_panel_opens_scrolled_to_the_setting_clicked() {
    use egui_kittest::kittest::Queryable;
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut m = Model::new("scrolled");
    // (a list given: not edited on the box, far down the operator's settings, in its folded part)
    let mut opt = Node::new("opt", "prioritize.optimize").param("num_temp", serde_json::json!([1, 2, 3]));
    opt.pos = Some([0.0, 0.0]);
    m.nodes = vec![opt];
    let mut h = harness(open(&m, "canvas-panel-scroll.toml"));
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(20.0, 20.0), 0.8);
    h.run_steps(8);
    let label = terraflux_app::node_view::label_of("num_temp");
    let field = |h: &Harness<TerrafluxApp>| h.query_all_by_label(&label).map(|n| n.rect()).next();
    // by the gear: the panel at its top, the setting far below the panel's end
    let gear = editor(&h).screen_of_gear("opt").expect("a gear");
    click(&mut h, gear);
    h.run_steps(6);
    let panel = panel_rect(&h).expect("the panel");
    assert!(field(&h).is_none_or(|r| !panel.contains_rect(r)), "{:?} not in {panel:?}", field(&h));
    assert!(h.state().session.as_ref().unwrap().editor.panel_focus.is_none());
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(panel_rect(&h).is_none());
    // by its row: scrolled to it
    let at = row_of(&mut h, "opt", "num_temp");
    click(&mut h, at);
    h.run_steps(30);
    let panel = panel_rect(&h).expect("the panel");
    let r = field(&h).expect("the setting's field is drawn");
    assert!(panel.contains_rect(r), "{r:?} not in {panel:?}");
    // (the scroll is done once, then the panel is the user's: scrolled up, it stays)
    assert!(editor(&h).panel_focus.is_none());
    h.hover_at(panel.center());
    h.event(egui::Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: egui::vec2(0.0, 4000.0),
        modifiers: egui::Modifiers::NONE,
        phase: egui::TouchPhase::Move,
    });
    h.run_steps(60);
    assert!(field(&h).is_none_or(|r| !panel.contains_rect(r)), "{:?}: left where the user scrolled it", field(&h));
}

/// A click on the row of a choice opens its list, a click on the same row again closes it (changing nothing), a
/// third opens it again; Esc and a click on an option close it as before.
#[test]
fn a_click_on_the_row_of_a_choice_toggles_its_list() {
    use egui_kittest::kittest::Queryable;
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut h = settings_harness("canvas-choice-toggle.toml");
    let listed = |h: &Harness<TerrafluxApp>| h.query_by_label_contains("gurobi").is_some();
    let at = row_of(&mut h, "opt", "solver");
    assert!(!listed(&h));
    click(&mut h, at);
    assert!(listed(&h), "opened");
    click(&mut h, at);
    assert!(!listed(&h), "closed by a click on its row");
    assert_eq!(param_now(&h, "opt", "solver"), Some(serde_json::json!("highs")));
    click(&mut h, at);
    assert!(listed(&h), "opened again");
    // (the press and the release in one frame, as a quick click)
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        });
    }
    h.run_steps(3);
    assert!(!listed(&h), "closed by a quick click too");
    // another row's click, with the list open, closes it
    click(&mut h, at);
    assert!(listed(&h));
    let gap = row_of(&mut h, "opt", "gap");
    click(&mut h, gap);
    assert!(!listed(&h));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // a click on an option sets it and closes the list
    click(&mut h, at);
    h.get_by_label_contains("gurobi").click();
    h.run_steps(3);
    assert!(!listed(&h));
    assert_eq!(param_now(&h, "opt", "solver"), Some(serde_json::json!("gurobi")));
}

/// A setting's wire: a tooltip says what it feeds, a wire dragged from a free setting's socket to an output makes the
/// setting read it, a double click adds a reroute point (kept under the setting's name, saved, moved, removed, gone
/// with the wire), each one step of undo.
#[test]
fn a_settings_wire_has_a_tip_points_and_can_be_drawn_from_the_socket() {
    use egui_kittest::kittest::Queryable;
    let _lock = settings_lock();
    terraflux_app::settings::update(|s| s.node_settings = true);
    let mut h = pins_harness("canvas-setting-wire.toml");
    // from the socket to an output that feeds it (one that does not is refused)
    let socket = editor(&h).screen_of_socket("opt", "gap").unwrap();
    let far = editor(&h).screen_of_output("ent", "pairs").unwrap();
    drag(&mut h, socket, far);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!(0.2)), "a table does not feed a number");
    let out = editor(&h).screen_of_output("ent", "index").unwrap();
    drag(&mut h, socket, out);
    assert_eq!(
        param_now(&h, "opt", "gap"),
        Some(serde_json::json!({ "from": "ent.index" })),
        "{:?}",
        editor(&h).message
    );
    assert_eq!(editor(&h).setting_wires(), 1);
    // the tooltip, once the pointer rests on the wire
    let mid = |h: &Harness<TerrafluxApp>| {
        let w = editor(h).setting_wire_path("opt", "gap").expect("the wire");
        w[w.len() / 2]
    };
    let at = graph_to_screen(&h, mid(&h));
    h.hover_at(at + egui::vec2(0.0, 300.0));
    h.run_steps(2);
    rest_on(&mut h, at, 3);
    assert!(h.query_by_label_contains("→").is_none(), "not before the pointer rests");
    rest_on(&mut h, at, 60);
    assert!(h.query_by_label_contains(".gap").is_some(), "the setting it feeds");
    h.hover_at(at + egui::vec2(0.0, 300.0));
    h.run_steps(2);
    // a point: added by a double click, the wire through it
    let key = "setting:gap".to_string();
    let points = |h: &Harness<TerrafluxApp>| points_of(h, false, "opt");
    assert!(points(&h).is_empty());
    let m = mid(&h);
    double_click_graph(&mut h, m);
    let pts = points(&h);
    assert_eq!(pts[&key].len(), 1, "{pts:?} {:?}", editor(&h).message);
    let p = pts[&key][0];
    assert!(is_near(p, m, 12.0), "{p:?} near {m:?}");
    let through = editor(&h).setting_wire_path("opt", "gap").unwrap();
    assert!(through.iter().any(|q| q.distance(egui::pos2(p[0], p[1])) < 1.0), "the wire goes through the point");
    // moved: dragged, one step of undo for the drag
    let from = graph_to_screen(&h, egui::pos2(p[0], p[1]));
    drag(&mut h, from, from + egui::vec2(30.0, 50.0));
    let moved = points(&h)[&key][0];
    assert!(moved[1] - p[1] > 20.0 && moved[0] - p[0] > 10.0, "{p:?} -> {moved:?}");
    undo(&mut h);
    assert_eq!(points(&h)[&key][0], p, "one undo for the drag");
    // saved with the model (under a key no port has), loaded again
    let text = h.state().session.as_ref().unwrap().source.to_toml().unwrap();
    let back = Model::from_toml(&text).unwrap();
    assert_eq!(back.node("opt").unwrap().wires[&key], vec![p], "{text}");
    // removed by a double click on the point, one step of undo
    double_click_graph(&mut h, egui::pos2(p[0], p[1]));
    assert!(points(&h).is_empty());
    undo(&mut h);
    assert_eq!(points(&h)[&key].len(), 1);
    // the wire taken off its socket: the setting has its own value again, and the points go with the wire
    let at = editor(&h).screen_of_socket("opt", "gap").unwrap();
    drag(&mut h, at, at + egui::vec2(-150.0, 250.0));
    assert_eq!(param_now(&h, "opt", "gap"), None);
    assert!(points(&h).is_empty(), "{:?}", points(&h));
    undo(&mut h);
    assert_eq!(param_now(&h, "opt", "gap"), Some(serde_json::json!({ "from": "ent.index" })));
}

/// A node that gives a map, a stack, a series or objects has an area for its picture under its last row (an option
/// of the settings, on): the box is taller by the space and the area; a click on it before a run only marks the node;
/// once run, the picture is made (never again for the same result); a click on it shows the output in the Viewer; off,
/// the area is gone.
#[test]
fn a_result_has_a_picture_in_its_box() {
    let _settings = settings_lock();
    let height = |h: &Harness<TerrafluxApp>, id: &str| editor(h).rects_of(&[id.to_string()])[0].1.height();
    let mut h = harness(open(&map_model(false), "canvas-thumbs.toml"));
    h.run_steps(3);
    // on (the default): the area of the map the node gives, inside the box's padding
    let on = height(&h, "calc");
    let (area, port) = editor(&h).thumb_of("calc").expect("the node has an area");
    assert_eq!(port, "map");
    assert_eq!(area.height(), 76.0);
    let rect = editor(&h).rects_of(&["calc".to_string()])[0].1;
    assert_eq!((area.left() - rect.left(), rect.right() - area.right()), (9.0, 9.0));
    // (6 units above it, the box's own 6 under it)
    assert_eq!(rect.bottom() - area.bottom(), 6.0);
    assert!(editor(&h).thumb_made("calc", "map").is_none(), "nothing before a run");
    // a click on the empty area: the node is marked, the Viewer is left alone (a reader would ask for its file)
    let at = graph_to_screen(&h, area.center());
    click(&mut h, at);
    assert_eq!(h.state().session.as_ref().unwrap().preview, None, "no empty Viewer");
    assert!(editor(&h).chosen.contains("calc"));
    // run: the picture is asked for and made
    run_model(&mut h);
    let t = Instant::now();
    while editor(&h).thumb_made("calc", "map").is_none() || editor(&h).thumbs_busy() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(20), "the picture was not made");
    }
    assert!(editor(&h).thumb_made("calc", "read").is_none());
    let made = editor(&h).thumb_made("calc", "map");
    h.run_steps(10);
    assert_eq!(editor(&h).thumb_made("calc", "map"), made, "the same result is not made again");
    // a click on the picture: what a click on its output does
    let at = graph_to_screen(&h, editor(&h).thumb_of("calc").unwrap().0.center());
    click(&mut h, at);
    assert_eq!(h.state().session.as_ref().unwrap().preview, Some(("calc".to_string(), "map".to_string())));
    // off: the box is shorter by the space and the area, and has none
    terraflux_app::settings::update(|s| s.node_thumbs = false);
    h.run_steps(3);
    assert!(editor(&h).thumb_of("calc").is_none());
    assert_eq!(on - height(&h, "calc"), 82.0);
    terraflux_app::settings::update(|s| s.node_thumbs = true);
    h.run_steps(3);
    assert_eq!(height(&h, "calc"), on);
}
