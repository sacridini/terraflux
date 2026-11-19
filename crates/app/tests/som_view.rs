//! The SOM view of an `ml.som` node's codebook linked to the maps in the Viewer (a node or group clicked lights its
//! cells on the map of nodes or groups, a cell clicked on the map picks its node in the view), and the scores of
//! `ml.pca` shown as a colour composite of pc1, pc2 and pc3 with the box "RGB".

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::charts::{self, Kind, SomPick};
use terraflux_app::som_link;
use terraflux_graph::{Model, Node};
use terraflux_ops::{Registry, Table, Value};

/// An image of three classes in six bands (30 x 40), its self-organizing map of 3 x 4 nodes in 3 groups, and its
/// principal components.
fn model(topology: &str) -> Model {
    let mut m = Model::new("som view");
    m.nodes.push(Node::new("land", "nlm.fbm").param("rows", 30).param("columns", 40).param("seed", 5));
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("img", "nlm.image").input("classes", "cls.classes"));
    m.nodes.push(
        Node::new("som", "ml.som")
            .input("predictors", "img.image")
            .param("rows", 3)
            .param("cols", 4)
            .param("clusters", 3)
            .param("topology", topology)
            .param("seed", 1),
    );
    m.nodes.push(Node::new("pca", "ml.pca").input("predictors", "img.image").param("components", 4));
    m
}

fn harness(name: &str, topology: &str) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("model.toml");
    model(topology).save(&p).unwrap();
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

fn value(h: &Harness<TerrafluxApp>, node: &str, port: &str) -> Value {
    h.state().session.as_ref().unwrap().value_of(node, port).unwrap_or_else(|| panic!("{node}.{port}"))
}

fn table_id(port: &str) -> egui::Id {
    egui::Id::new(("table", "viewer", "som", port))
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

fn click_cell(h: &mut Harness<TerrafluxApp>, row: usize, col: usize) {
    let at = h.state().session.as_ref().unwrap().preview_view.cell_screen(col, row).expect("the map drawn");
    click_at(h, at);
}

/// The first cell (row, column) of a map whose value is `v`.
fn cell_with(map: &terraflux_ops::Map, v: f64) -> (usize, usize) {
    let i = map.data.iter().position(|&x| x == v).expect("a cell");
    (i / map.grid.width, i % map.grid.width)
}

fn map_of(v: Value) -> terraflux_ops::Map {
    match v {
        Value::Map(m) => m,
        other => panic!("{other:?}"),
    }
}

fn table_of(v: Value) -> Table {
    match v {
        Value::Table(t) => t,
        other => panic!("{other:?}"),
    }
}

/// The cells a lighting map covers.
fn lit_cells(h: &Harness<TerrafluxApp>, port: &str) -> usize {
    let m = som_link::lit(&h.ctx, "som", port).expect("the pick lit");
    m.data.iter().filter(|&&v| v == 2.0).count()
}

#[test]
fn a_node_clicked_in_the_som_view_lights_its_cells_and_a_cell_clicked_picks_its_node() {
    for topology in ["rectangular", "hexagonal"] {
        let mut h = harness(&format!("som-view-link-{topology}"), topology);
        run_and_show(&mut h, "som");
        let nodes = map_of(value(&h, "som", "nodes"));
        let groups = map_of(value(&h, "som", "groups"));
        let book = table_of(value(&h, "som", "codebook"));
        let col = |n: &str| book.columns.iter().position(|c| c == n).unwrap();
        let (c_node, c_count, c_group) = (col("node"), col("count"), col("group"));
        assert_eq!(book.label("topology", book.rows[0][col("topology")]), Some(topology));
        // the codebook is drawn as a SOM at once (a codebook opens with it), every node on the screen
        show(&mut h, "som", "codebook");
        until(&mut h, "the SOM view", |h| charts::som_screen(&h.ctx, table_id("codebook"), 1).is_some());
        let state: terraflux_app::table_view::TableState =
            h.ctx.data(|d| d.get_temp(table_id("codebook"))).expect("the table's state");
        assert_eq!(state.chart.kind, Kind::Som);
        assert!(Kind::of(&book).contains(&Kind::Som));
        assert_eq!(charts::picked(&h.ctx, table_id("codebook")), None);
        // a node with cells, clicked: it is the pick of the view and of the node's link
        let row = book.rows.iter().find(|r| r[c_count] > 0.0).unwrap().clone();
        let (k, g) = (row[c_node] as usize, row[c_group] as usize);
        let at = charts::som_screen(&h.ctx, table_id("codebook"), k).unwrap();
        click_at(&mut h, at);
        assert_eq!(charts::picked(&h.ctx, table_id("codebook")), Some(SomPick::Node(k)), "{topology}");
        assert_eq!(som_link::linked(&h.ctx, "som"), Some(SomPick::Node(k)));
        // its cells lit on the map of nodes: as many as the node holds, those of its node
        show(&mut h, "som", "nodes");
        assert_eq!(lit_cells(&h, "nodes") as f64, row[c_count], "{topology}");
        let lit = som_link::lit(&h.ctx, "som", "nodes").unwrap();
        assert!(lit.data.iter().zip(nodes.data.iter()).all(|(l, n)| match (l.is_nan(), n.is_nan()) {
            (true, true) => true,
            (false, false) => (*l == 2.0) == (*n == k as f64),
            _ => false,
        }));
        // and the group's on the map of groups (the node's group)
        show(&mut h, "som", "groups");
        let in_group = groups.data.iter().filter(|&&v| v == g as f64).count();
        assert_eq!(lit_cells(&h, "groups"), in_group);
        // a cell clicked on the map of nodes: its node is picked, in the view too
        show(&mut h, "som", "nodes");
        let other = book.rows.iter().find(|r| r[c_count] > 0.0 && r[c_node] as usize != k).unwrap()[c_node];
        let (r2, c2) = cell_with(&nodes, other);
        click_cell(&mut h, r2, c2);
        assert_eq!(som_link::linked(&h.ctx, "som"), Some(SomPick::Node(other as usize)), "{topology}");
        assert_eq!(lit_cells(&h, "nodes") as f64, book.rows[other as usize - 1][c_count]);
        show(&mut h, "som", "codebook");
        assert_eq!(charts::picked(&h.ctx, table_id("codebook")), Some(SomPick::Node(other as usize)));
        // a cell of the map of groups picks its group, which lights its cells (and its nodes in the view)
        show(&mut h, "som", "groups");
        let (r3, c3) = cell_with(&groups, 2.0);
        click_cell(&mut h, r3, c3);
        assert_eq!(som_link::linked(&h.ctx, "som"), Some(SomPick::Group(2)), "{topology}");
        assert_eq!(lit_cells(&h, "groups"), groups.data.iter().filter(|&&v| v == 2.0).count());
        show(&mut h, "som", "nodes");
        let want: f64 = book.rows.iter().filter(|r| r[c_group] == 2.0).map(|r| r[c_count]).sum();
        assert_eq!(lit_cells(&h, "nodes") as f64, want);
        show(&mut h, "som", "codebook");
        assert_eq!(charts::picked(&h.ctx, table_id("codebook")), Some(SomPick::Group(2)));
    }
}

#[test]
fn the_scores_of_a_pca_are_one_click_from_a_colour_composite() {
    use terraflux_app::builder::Layer;
    let mut h = harness("som-view-pca-rgb", "rectangular");
    run_and_show(&mut h, "pca");
    let Value::Stack(scores) = value(&h, "pca", "scores") else { panic!("a stack") };
    assert_eq!(scores.keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(), ["pc1", "pc2", "pc3", "pc4"]);
    h.state_mut().session.as_mut().unwrap().preview_layers = vec![Layer::new("pca", "scores")];
    show(&mut h, "pca", "scores");
    until(&mut h, "the box", |h| h.query_by_label("RGB").is_some());
    h.get_by_label("RGB").click();
    h.run_steps(3);
    let rgb = h.state().session.as_ref().unwrap().preview_layers[0].rgb.clone().expect("the composite on");
    // red, green and blue are the first three components (not the first three by luck: the composite is named so)
    assert_eq!(rgb.bands, [0, 1, 2]);
    until(&mut h, "the legend", |h| h.query_by_label_contains("R pc1 ").is_some());
    assert!(h.query_by_label_contains("G pc2 ").is_some() && h.query_by_label_contains("B pc3 ").is_some());
}
