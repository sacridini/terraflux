//! Objects in the Viewer, selected and inspected, and samples given by clicking: a click selects an object (N adds its
//! neighbours, P goes to the level above), the inspector tells the membership to each class and what decided, the
//! samples mode writes the clicks into the model as a chain feeding objects.samples, and the trace of a cell of
//! objects tells the memberships.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_graph::{Model, Node, Source};
use terraflux_ops::objects::Objects;
use terraflux_ops::{Registry, Value};

/// Objects of a random map classed in three; a fine and a coarse level (the fine one inside the coarse); and the
/// objects classed by two rules on their cells.
fn model() -> Model {
    let mut m = Model::new("objects viewer");
    m.nodes.push(
        Node::new("land", "nlm.mpd")
            .param("rows", 40)
            .param("columns", 60)
            .param("seed", 3)
            .param("nlmr_compat", false),
    );
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("seg", "segment.from_map").input("map", "cls.classes"));
    m.nodes.push(Node::new("fine", "segment.multiresolution").input("image", "land.map").param("scale", 0.02));
    m.nodes.push(
        Node::new("coarse", "segment.multiresolution")
            .input("image", "land.map")
            .input("from", "fine.objects")
            .param("scale", 0.15),
    );
    m.nodes.push(Node::new("rules", "objects.rules").input("objects", "seg.objects").param(
        "classes",
        serde_json::json!([
            { "name": "big", "rule": "above(cells, 1, 4)" },
            { "name": "small", "rule": "below(cells, 1, 4)" },
        ]),
    ));
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

/// The model run, and the node's output shown (the map of a node that gives one, else its objects).
fn run_and_show(h: &mut Harness<TerrafluxApp>, node: &str) {
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), node);
    }
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    let t = Instant::now();
    loop {
        let s = h.state().session.as_ref().unwrap();
        if s.outputs.contains_key(node) {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(60), "the run: too long ({:?}, {:?})", s.status, s.editor.message);
        h.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    h.run_steps(3);
}

fn objects_of(h: &Harness<TerrafluxApp>, node: &str, port: &str) -> std::sync::Arc<Objects> {
    match h.state().session.as_ref().unwrap().value_of(node, port) {
        Some(Value::Objects(o)) => o,
        other => panic!("{node}.{port}: {other:?}"),
    }
}

/// A click of the primary button at the centre of a cell of the map shown.
fn click_cell(h: &mut Harness<TerrafluxApp>, row: usize, col: usize) {
    let at = h.state().session.as_ref().unwrap().preview_view.cell_screen(col, row).expect("the map drawn");
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

fn selected(h: &Harness<TerrafluxApp>) -> Vec<u32> {
    h.state().session.as_ref().unwrap().preview_objects.selected.iter().copied().collect()
}

/// A cell of an object that is not on its border (a click there is on it at any zoom).
fn cell_of(o: &Objects, id: u32) -> (usize, usize) {
    terraflux_app::objects_inspect::sample_cell(o, id).expect("an object")
}

#[test]
fn a_click_selects_an_object_n_adds_its_neighbours_and_p_goes_to_its_parent() {
    let mut h = harness("objects-viewer-select");
    run_and_show(&mut h, "coarse");
    // the fine level as a layer over the map: it has the coarse level as its parent
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), "land");
        s.preview_layers = vec![terraflux_app::builder::Layer::new("coarse", "below")];
    }
    h.run_steps(2);
    until(&mut h, "the outlines", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() == 1);
    let fine = objects_of(&h, "coarse", "below");
    assert!(fine.parent.is_some(), "the objects of `below` have a parent level");
    // an object with neighbours
    let id = (1..=fine.count as u32).find(|&i| !fine.neighbours_of(i).is_empty()).expect("neighbours");
    let (row, col) = cell_of(&fine, id);
    click_cell(&mut h, row, col);
    assert_eq!(selected(&h), vec![id], "a click selects the object under the pointer");
    // N: the neighbours join it
    h.key_press(egui::Key::N);
    h.run_steps(3);
    let mut want: Vec<u32> = std::iter::once(id).chain(fine.neighbours_of(id).iter().map(|n| n.0)).collect();
    want.sort_unstable();
    want.dedup();
    assert_eq!(selected(&h), want);
    // a click elsewhere selects that object alone (and Esc lets go)
    click_cell(&mut h, row, col);
    assert_eq!(selected(&h), vec![id]);
    // P: the parent is selected, and the level above is the one drawn
    h.key_press(egui::Key::P);
    h.run_steps(3);
    let parent = fine.parent_id(id).expect("a parent");
    assert_eq!(selected(&h), vec![parent]);
    assert_eq!(h.state().session.as_ref().unwrap().preview_layers[0].style.level, 1);
    // C: back to the children (the layer's level beneath)
    h.key_press(egui::Key::C);
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().preview_layers[0].style.level, 0);
    assert!(selected(&h).contains(&id));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    assert!(selected(&h).is_empty());
}

#[test]
fn the_inspector_tells_the_membership_and_the_rules_of_the_object() {
    let mut h = harness("objects-viewer-inspect");
    run_and_show(&mut h, "rules");
    // (the output shown is the objects: a node with a map shows that first)
    h.state_mut().session.as_mut().unwrap().preview = Some(("rules".into(), "objects".into()));
    h.run_steps(2);
    until(&mut h, "the objects drawn", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() == 1);
    let o = objects_of(&h, "rules", "objects");
    assert_eq!(o.classes.len(), 2);
    assert!(has(&h, "click an object to inspect it"));
    let (row, col) = cell_of(&o, 1);
    click_cell(&mut h, row, col);
    assert_eq!(selected(&h), vec![1]);
    until(&mut h, "the inspector", |h| has(h, "Membership"));
    // a bar for each class, the rules and their values for this object
    assert!(has(&h, "big "), "the membership of big");
    assert!(has(&h, "small "), "the membership of small");
    assert!(has(&h, "What decided"));
    assert!(has(&h, "big: above(cells, 1, 4) = "), "the value of the rule of big");
    assert!(has(&h, "small: below(cells, 1, 4) = "), "the value of the rule of small");
    assert!(has(&h, "margin "), "the margin");
}

#[test]
fn the_trace_of_a_cell_of_objects_tells_the_memberships() {
    let mut h = harness("objects-viewer-trace");
    run_and_show(&mut h, "rules");
    let o = objects_of(&h, "rules", "objects");
    let (row, col) = cell_of(&o, 1);
    let t = o.grid.transform;
    let (x, y) = (t[0] + (col as f64 + 0.5) * t[1], t[3] + (row as f64 + 0.5) * t[5]);
    let at = terraflux_app::trace::at(&Value::Objects(o.clone()), x, y).expect("a cell");
    let text = match at {
        terraflux_app::trace::At::Other(t) => t,
        other => panic!("{other:?}"),
    };
    assert!(text.starts_with("object 1, class "), "{text}");
    assert!(text.contains("membership big ") && text.contains(" small "), "{text}");
    assert!(text.contains("margin "), "{text}");
}

/// Samples mode on the objects of `seg`: the classes A and B, three objects clicked (two A, one B), and what the
/// model has after *Apply*.
fn give_samples(h: &mut Harness<TerrafluxApp>, most: usize) -> (std::sync::Arc<Objects>, [u32; 3]) {
    run_and_show(h, "seg");
    until(h, "the objects drawn", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() == 1);
    until(h, "the inspector", |h| has(h, "Samples"));
    let o = objects_of(h, "seg", "objects");
    // three objects of several cells (their interior cell has the object's id)
    let interior = |i: u32| {
        let (r, c) = cell_of(&o, i);
        let w = o.grid.width;
        r > 0 && c > 0 && r + 1 < o.grid.height && c + 1 < w && {
            let same = |r: usize, c: usize| o.labels[r * w + c] == i;
            same(r - 1, c) && same(r + 1, c) && same(r, c - 1) && same(r, c + 1)
        }
    };
    let ids: Vec<u32> = (1..=o.count as u32).filter(|&i| interior(i)).take(3).collect();
    assert_eq!(ids.len(), 3, "objects with an interior cell");
    {
        let st = &mut h.state_mut().session.as_mut().unwrap().preview_objects;
        st.sampling = true;
        st.classes = vec!["A".into(), "B".into()];
        st.most = most;
    }
    h.run_steps(2);
    for (k, &id) in ids.iter().enumerate() {
        h.state_mut().session.as_mut().unwrap().preview_objects.active = usize::from(k == 2);
        let (row, col) = cell_of(&o, id);
        click_cell(h, row, col);
    }
    let st = &h.state().session.as_ref().unwrap().preview_objects;
    assert_eq!(st.samples.len(), 3, "a sample for each object clicked");
    assert!(selected(h).is_empty(), "no selection in samples mode");
    h.get_by_label("Apply").click();
    h.run_steps(4);
    (o, [ids[0], ids[1], ids[2]])
}

#[test]
fn samples_by_clicking_are_written_into_the_model_and_undone() {
    let mut h = harness("objects-viewer-samples");
    let (o, ids) = give_samples(&mut h, 0);
    let s = h.state().session.as_ref().unwrap();
    let node = |id: &str| s.source.nodes.iter().find(|n| n.id == id).unwrap_or_else(|| panic!("no node {id}"));
    // the points: a table written in the settings, read as points
    let table = node("sample_table");
    assert_eq!(table.op, "table.of");
    let rows = table.params["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    let classes: Vec<&str> = rows.iter().map(|r| r[2].as_str().unwrap()).collect();
    assert_eq!(classes, ["A", "A", "B"]);
    assert_eq!(table.params["columns"], serde_json::json!(["x", "y", "class"]));
    let points = node("sample_points");
    assert_eq!(points.op, "vector.from_table");
    assert_eq!(points.inputs["table"], Source::Ref("sample_table.table".into()));
    // each point is the centre of a cell of its object, not on its border
    let t = o.grid.transform;
    for (r, id) in rows.iter().zip(ids) {
        let (x, y) = (r[0].as_f64().unwrap(), r[1].as_f64().unwrap());
        let (col, row) = (((x - t[0]) / t[1]).floor() as usize, ((y - t[3]) / t[5]).floor() as usize);
        assert_eq!(o.id_at(row, col), id);
        let w = o.grid.width;
        let same = |r: usize, c: usize| o.labels[r * w + c] == id;
        assert!(same(row - 1, col) && same(row + 1, col) && same(row, col - 1) && same(row, col + 1), "interior");
    }
    // the chain: objects.samples -> ml.fit (column class) -> objects.classify
    let samples = node("samples");
    assert_eq!(samples.op, "objects.samples");
    assert_eq!(samples.inputs["objects"], Source::Ref("seg.objects".into()));
    assert_eq!(samples.inputs["samples"], Source::Ref("sample_points.vector".into()));
    let fit = node("fit");
    assert_eq!(fit.op, "ml.fit");
    assert_eq!(fit.inputs["table"], Source::Ref("samples.table".into()));
    assert_eq!(fit.params["column"], "class");
    let classify = node("classified");
    assert_eq!(classify.op, "objects.classify");
    assert_eq!(classify.inputs["model"], Source::Ref("fit.model".into()));
    // one step of undo takes the whole chain away
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert!(s.history.undo(&mut s.source));
        for id in ["sample_table", "sample_points", "samples", "fit", "classified"] {
            assert!(!s.source.nodes.iter().any(|n| n.id == id), "{id} is gone");
        }
    }
}

#[test]
fn many_samples_go_to_a_geopackage_beside_the_model() {
    let mut h = harness("objects-viewer-gpkg");
    // (a smaller most than the 500 of the model: 3 samples are more than 2)
    give_samples(&mut h, 2);
    let s = h.state().session.as_ref().unwrap();
    let points = s.source.nodes.iter().find(|n| n.id == "sample_points").expect("the points' node");
    assert_eq!(points.op, "vector.read");
    let path = PathBuf::from(points.params["path"].as_str().unwrap());
    assert!(path.exists(), "{}", path.display());
    assert_eq!(path.extension().unwrap(), "gpkg");
    assert!(path.starts_with(s.base().join("samples")), "{}", path.display());
    assert!(!s.source.nodes.iter().any(|n| n.id == "sample_table"), "no table in the settings");
    let v = terraflux_gdal::read_vector(&path, None).unwrap();
    assert_eq!(v.geometries.len(), 3);
    assert!(v.attributes.iter().any(|(n, _)| n == "class"));
    assert!(s.source.nodes.iter().any(|n| n.op == "objects.samples"));
}
