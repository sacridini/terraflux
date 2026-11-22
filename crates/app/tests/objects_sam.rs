//! The SAM tool of the Viewer, with a fake model (no weights): the encoder runs once on the view, each click runs only the
//! decoder and the mask is drawn live; Enter writes the prompts into the model in one step of undo, Esc drops them, and a
//! class chosen goes with the object.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_app::sam_tool::{Backend, Image3};
use terraflux_graph::{Model, Node};
use terraflux_ops::objects::deep::{Embedded, Group, Prompt, Tile, read_prompts};
use terraflux_ops::objects::onnx::Tensor;
use terraflux_ops::{Registry, Value};

/// A drawn image of three classes, cut into objects.
fn model() -> Model {
    let mut m = Model::new("objects sam");
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

/// A model that cuts a square of 9 cells a side around the first point inside, less the 3 around each point outside,
/// or the box; the cells of the window.
struct Fake;

impl Backend for Fake {
    fn encode(&self, _: &Image3, tile: &Tile) -> Result<Embedded, String> {
        Ok(Embedded { emb: Tensor::new(vec![1], vec![0.0]), scale: 1.0, osize: (tile.h, tile.w), tile: *tile })
    }

    fn decode(&self, e: &Embedded, g: &Group, _: &Image3) -> Result<Option<Vec<bool>>, String> {
        let t = e.tile;
        let mut cells = vec![false; t.h * t.w];
        let near = |x: f64, y: f64, c: usize, r: usize, half: f64| {
            ((c as f64 + 0.5) - x).abs() <= half + 0.5 && ((r as f64 + 0.5) - y).abs() <= half + 0.5
        };
        let boxed = g.prompts.iter().find_map(|p| match p {
            Prompt::Box { x0, y0, x1, y1 } => Some((*x0, *y0, *x1, *y1)),
            _ => None,
        });
        let first = g.prompts.iter().find_map(|p| match p {
            Prompt::Point { x, y, positive: true } => Some((*x, *y)),
            _ => None,
        });
        for r in 0..t.h {
            for c in 0..t.w {
                let (cc, rr) = (t.c0 + c, t.r0 + r);
                let inside = match (boxed, first) {
                    (Some((x0, y0, x1, y1)), _) => {
                        (cc as f64 + 0.5) >= x0
                            && (cc as f64 + 0.5) <= x1
                            && (rr as f64 + 0.5) >= y0
                            && (rr as f64 + 0.5) <= y1
                    }
                    (None, Some((x, y))) => near(x, y, cc, rr, 4.0),
                    _ => false,
                };
                let out = g
                    .prompts
                    .iter()
                    .any(|p| matches!(p, Prompt::Point { x, y, positive: false } if near(*x, *y, cc, rr, 1.0)));
                cells[r * t.w + c] = inside && !out;
            }
        }
        Ok(Some(cells))
    }
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
        .with_size((1400.0, 900.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app);
    (h, dir)
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

fn sam<'a>(h: &'a Harness<TerrafluxApp>) -> &'a terraflux_app::sam_tool::SamTool {
    &h.state().session.as_ref().unwrap().preview_objects.sam
}

fn press(h: &mut Harness<TerrafluxApp>, at: egui::Pos2, button: egui::PointerButton, pressed: bool) {
    h.event(egui::Event::PointerButton { pos: at, button, pressed, modifiers: egui::Modifiers::NONE });
    h.run_steps(1);
}

fn click(h: &mut Harness<TerrafluxApp>, row: usize, col: usize, button: egui::PointerButton) {
    let at = h.state().session.as_ref().unwrap().preview_view.cell_screen(col, row).expect("the map drawn");
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    press(h, at, button, true);
    press(h, at, button, false);
    h.run_steps(3);
}

/// A box dragged from one cell to another.
fn drag(h: &mut Harness<TerrafluxApp>, from: (usize, usize), to: (usize, usize)) {
    let view = &h.state().session.as_ref().unwrap().preview_view;
    let (a, b) = (view.cell_screen(from.1, from.0).unwrap(), view.cell_screen(to.1, to.0).unwrap());
    h.event(egui::Event::PointerMoved(a));
    h.run_steps(2);
    press(h, a, egui::PointerButton::Primary, true);
    for k in 1..=6 {
        h.event(egui::Event::PointerMoved(a + (b - a) * (k as f32 / 6.0)));
        h.run_steps(1);
    }
    press(h, b, egui::PointerButton::Primary, false);
    h.run_steps(3);
}

fn shown() -> (Harness<'static, TerrafluxApp>, PathBuf) {
    let (mut h, dir) = harness("objects-sam");
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
    until(&mut h, "the inspector", |h| has(h, "SAM tool"));
    {
        let st = &mut h.state_mut().session.as_mut().unwrap().preview_objects;
        st.sam.backend = Some(Arc::new(Fake));
        st.sam.on = true;
        st.classes = vec!["tree".into()];
    }
    (h, dir)
}

#[test]
fn a_click_runs_the_decoder_only_and_shows_the_mask() {
    let (mut h, _) = shown();
    until(&mut h, "the encoder", |h| sam(h).ready());
    assert_eq!(sam(&h).encoded, 1, "the encoder runs once on the view");
    // a click inside: a point, the mask live
    click(&mut h, 20, 30, egui::PointerButton::Primary);
    assert_eq!(sam(&h).group.prompts.len(), 1);
    until(&mut h, "the mask", |h| sam(h).mask.is_some());
    let m = sam(&h).mask.clone().unwrap();
    assert!(m.cells[(20 - m.r0) * m.w + (30 - m.c0)], "the cell clicked is in the mask");
    assert_eq!(m.cells.iter().filter(|&&c| c).count(), 81, "a square of 9 x 9 cells");
    assert_eq!((sam(&h).encoded, sam(&h).decoded), (1, 1));
    // the objects are not selected by the click, and no cell is picked
    assert!(h.state().session.as_ref().unwrap().preview_objects.selected.is_empty());
    // a right click: a point outside, the mask is made again without the cells around it
    click(&mut h, 20, 33, egui::PointerButton::Secondary);
    assert!(matches!(sam(&h).group.prompts[1], Prompt::Point { positive: false, .. }));
    until(&mut h, "the mask without it", |h| {
        sam(h).mask.as_ref().is_some_and(|m| m.id != 0 && m.cells.iter().filter(|&&c| c).count() < 81)
    });
    assert_eq!(sam(&h).encoded, 1, "a click does not run the encoder again");
    assert_eq!(sam(&h).decoded, 2);
}

#[test]
fn a_dragged_box_is_the_prompt_and_esc_drops_it() {
    let (mut h, _) = shown();
    until(&mut h, "the encoder", |h| sam(h).ready());
    drag(&mut h, (10, 10), (18, 25));
    assert_eq!(sam(&h).group.prompts.len(), 1);
    assert!(matches!(sam(&h).group.prompts[0], Prompt::Box { .. }));
    until(&mut h, "the mask", |h| sam(h).mask.is_some());
    // a second box replaces the first
    drag(&mut h, (5, 5), (12, 12));
    assert_eq!(sam(&h).group.prompts.len(), 1);
    // Esc drops the prompt, and writes nothing
    let before = h.state().session.as_ref().unwrap().source.clone();
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(sam(&h).group.prompts.is_empty() && sam(&h).mask.is_none());
    assert_eq!(h.state().session.as_ref().unwrap().source, before);
}

#[test]
fn enter_writes_the_prompts_into_the_model_in_one_step_of_undo() {
    let (mut h, dir) = shown();
    until(&mut h, "the encoder", |h| sam(h).ready());
    h.state_mut().session.as_mut().unwrap().preview_objects.sam.class = Some("tree".into());
    click(&mut h, 20, 30, egui::PointerButton::Primary);
    click(&mut h, 20, 33, egui::PointerButton::Secondary);
    until(&mut h, "the mask", |h| sam(h).mask.is_some());
    let before = h.state().session.as_ref().unwrap().source.clone();
    h.key_press(egui::Key::Enter);
    until(&mut h, "the object kept", |h| sam(h).kept.len() == 1);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    // a SAM node of the image, its prompts read from a file beside the model
    let node = s.source.nodes.iter().find(|n| n.op == "segment.sam").expect("a segment.sam node");
    assert_eq!(node.inputs.get("image"), Some(&terraflux_graph::Source::Ref("img.image".into())));
    let reader = s.source.nodes.iter().find(|n| n.op == "vector.read").expect("the reader");
    assert_eq!(node.inputs.get("prompts"), Some(&terraflux_graph::Source::Ref(format!("{}.vector", reader.id))));
    let path = reader.params["path"].as_str().unwrap().to_string();
    assert!(std::path::Path::new(&path).starts_with(dir.join("drawn")), "{path}");
    // the file holds the prompts: a point inside and a point outside, a group, the class
    let v = terraflux_gdal::read_vector(&path, None).unwrap();
    let grid = match s.value_of("img", "image") {
        Some(Value::Stack(st)) => st.grid.clone(),
        other => panic!("{other:?}"),
    };
    let groups = read_prompts("n", &v, &grid).unwrap();
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].class.as_deref(), Some("tree"));
    assert_eq!(groups[0].prompts.len(), 2);
    assert!(matches!(groups[0].prompts[0], Prompt::Point { positive: true, .. }));
    assert!(matches!(groups[0].prompts[1], Prompt::Point { positive: false, .. }));
    // (the cell clicked: row 20, column 30)
    if let Prompt::Point { x, y, .. } = groups[0].prompts[0] {
        assert_eq!((x.floor() as usize, y.floor() as usize), (30, 20));
    }
    // the prompt is let go, the tool is ready for the next
    assert!(sam(&h).group.prompts.is_empty());
    // another object: the same node and file hold both
    drag(&mut h, (5, 5), (12, 12));
    until(&mut h, "the second mask", |h| sam(h).mask.is_some());
    h.key_press(egui::Key::Enter);
    until(&mut h, "the second kept", |h| sam(h).kept.len() == 2);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes.iter().filter(|n| n.op == "segment.sam").count(), 1);
    let reader = s.source.nodes.iter().find(|n| n.op == "vector.read").unwrap();
    let v = terraflux_gdal::read_vector(reader.params["path"].as_str().unwrap(), None).unwrap();
    let groups = read_prompts("n", &v, &grid).unwrap();
    assert_eq!(groups.len(), 2);
    assert!(matches!(groups[1].prompts[0], Prompt::Box { .. }));
    // undo takes the second object away, then the first (the nodes with it)
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert!(s.history.undo(&mut s.source));
        let reader = s.source.nodes.iter().find(|n| n.op == "vector.read").expect("the reader stays");
        let v = terraflux_gdal::read_vector(reader.params["path"].as_str().unwrap(), None).unwrap();
        assert_eq!(read_prompts("n", &v, &grid).unwrap().len(), 1, "one undo: the second object gone");
        assert!(s.history.undo(&mut s.source));
        assert_eq!(s.source, before, "the second undo: the model as it was");
    }
}

/// The tool at work, drawn in a dark and a light theme (the pictures go to the target's temporary folder, for a look).
#[test]
#[ignore = "writes two pictures and changes the global theme while it runs: cargo test -- --ignored the_tool_drawn"]
fn the_tool_drawn_in_both_themes() {
    let (mut h, dir) = shown();
    until(&mut h, "the encoder", |h| sam(h).ready());
    click(&mut h, 20, 30, egui::PointerButton::Primary);
    click(&mut h, 20, 34, egui::PointerButton::Secondary);
    until(&mut h, "the mask", |h| sam(h).mask.is_some());
    h.key_press(egui::Key::Enter);
    until(&mut h, "kept", |h| sam(h).kept.len() == 1);
    click(&mut h, 8, 12, egui::PointerButton::Primary);
    until(&mut h, "the next mask", |h| sam(h).mask.is_some());
    let was = terraflux_app::theme::current().name.to_string();
    for name in ["Slate", "Paper"] {
        terraflux_app::settings::update(|s| s.theme = name.to_string());
        h.run_steps(4);
        match h.render() {
            Ok(img) => img.save(dir.join(format!("sam-{name}.png"))).unwrap(),
            Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
        }
    }
    terraflux_app::settings::update(|s| s.theme = was);
}

/// The real model through the tool's backend, on a bright square over a dark ground: the mask of a click inside it is the
/// square (needs ONNX Runtime, ORT_DYLIB_PATH, and the SAM weights in the cache).
#[test]
#[ignore = "needs ONNX Runtime (ORT_DYLIB_PATH) and the SAM weights in the cache"]
fn the_real_model_cuts_the_square_clicked() {
    if std::env::var_os("ORT_DYLIB_PATH").is_none() {
        eprintln!("skipped: ORT_DYLIB_PATH is not set");
        return;
    }
    let Ok(real) = terraflux_app::sam_tool::Real::new() else {
        eprintln!("skipped: no weights in the cache");
        return;
    };
    let (w, h) = (160usize, 120usize);
    let band = |bright: f64| -> Arc<Vec<f64>> {
        Arc::new(
            (0..w * h)
                .map(|i| {
                    let (r, c) = (i / w, i % w);
                    let inside = (30..80).contains(&r) && (50..110).contains(&c);
                    let tex = ((r * 31 + c * 17) % 7) as f64;
                    (if inside { bright } else { 40.0 }) + tex
                })
                .collect(),
        )
    };
    let image = Image3::new(Arc::new(terraflux_raster::Grid::new(w, h)), [band(220.0), band(60.0), band(50.0)]);
    let tile = Tile { r0: 0, c0: 0, h, w };
    let e = real.encode(&image, &tile).unwrap();
    let g = Group { prompts: vec![Prompt::Point { x: 80.5, y: 55.5, positive: true }], class: None };
    let cells = real.decode(&e, &g, &image).unwrap().expect("a mask");
    let truth = |i: usize| (30..80).contains(&(i / w)) && (50..110).contains(&(i % w));
    let inter = (0..w * h).filter(|&i| cells[i] && truth(i)).count();
    let union = (0..w * h).filter(|&i| cells[i] || truth(i)).count();
    let iou = inter as f64 / union as f64;
    eprintln!("the square: IoU {iou:.3}");
    assert!(iou > 0.9, "IoU {iou}");
}
