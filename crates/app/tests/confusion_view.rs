//! The confusion matrix of `objects.accuracy` and of `ml.fit`'s cross-validation drawn as a chart, and the cell picked in
//! the first lit on the objects it judges in the Viewer: the chart is chosen by the table's layout, its margins are the
//! accuracy table's numbers, and a click on a cell lights exactly the reference objects of that pair.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use terraflux_app::TerrafluxApp;
use terraflux_app::charts::{self, Kind};
use terraflux_app::confusion_link;
use terraflux_graph::Model;
use terraflux_ops::{Registry, Table, Value};

/// Five classes in an image of 120 x 120 cells cut into objects, a random forest learning 19 sample points, and
/// reference points (the other points of each class, two of them named wrongly on purpose) to judge the map with.
const MODEL: &str = r#"
format = "terraflux/1"
name = "confusion view"

[[nodes]]
id = "land"
op = "nlm.mosaic_gibbs"
params = { rows = 120, columns = 120, germs = 30, r = 12, patch_classes = 5, nlmr_compat = false, rescale = false, seed = 4 }

[[nodes]]
id = "image"
op = "nlm.image"
inputs = { classes = "land.map" }
params = { seed = 1 }

[[nodes]]
id = "segments"
op = "segment.multiresolution"
inputs = { image = "image.image" }

[[nodes]]
id = "features"
op = "objects.features"
inputs = { objects = "segments.objects", image = "image.image" }

[[nodes]]
id = "train_table"
op = "table.of"

[nodes.params]
columns = ["x", "y", "class"]
rows = [[9.5, 104.5, "water"], [85.5, 46.5, "water"], [5.5, 38.5, "water"], [9.5, 86.5, "water"], [10.5, 3.5, "forest"], [98.5, 30.5, "forest"], [29.5, 97.5, "forest"], [100.5, 19.5, "forest"], [65.5, 90.5, "grassland"], [92.5, 100.5, "grassland"], [87.5, 114.5, "grassland"], [115.5, 69.5, "cropland"], [96.5, 55.5, "cropland"], [114.5, 8.5, "cropland"], [114.5, 85.5, "cropland"], [71.5, 60.5, "urban"], [60.5, 70.5, "urban"], [26.5, 43.5, "urban"], [59.5, 44.5, "urban"]]

[[nodes]]
id = "train_points"
op = "vector.from_table"
inputs = { table = "train_table.table" }

[[nodes]]
id = "training"
op = "objects.samples"
inputs = { objects = "features.objects", samples = "train_points.vector" }

[[nodes]]
id = "model"
op = "ml.fit"
inputs = { table = "training.table" }
params = { column = "class", method = "random_forest", seed = 1, cv = 3 }

[[nodes]]
id = "classify"
op = "objects.classify"
inputs = { objects = "features.objects", model = "model.model", samples = "training.table" }

[[nodes]]
id = "ref_table"
op = "table.of"

[nodes.params]
columns = ["x", "y", "class"]
rows = [[68.5, 112.5, "water"], [84.5, 78.5, "water"], [10.5, 18.5, "water"], [83.5, 62.5, "forest"], [89.5, 22.5, "forest"], [23.5, 110.5, "forest"], [39.5, 91.5, "forest"], [38.5, 103.5, "forest"], [29.5, 20.5, "grassland"], [106.5, 111.5, "grassland"], [94.5, 7.5, "grassland"], [104.5, 75.5, "cropland"], [74.5, 7.5, "cropland"], [76.5, 16.5, "cropland"], [49.5, 110.5, "grassland"], [55.5, 58.5, "urban"], [20.5, 35.5, "urban"], [64.5, 61.5, "urban"], [50.5, 31.5, "urban"]]

[[nodes]]
id = "ref_points"
op = "vector.from_table"
inputs = { table = "ref_table.table" }

[[nodes]]
id = "accuracy"
op = "objects.accuracy"
inputs = { objects = "classify.objects", samples = "ref_points.vector" }
params = { design = "objects" }
"#;

fn harness(name: &str) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("model.toml");
    Model::from_toml(MODEL).unwrap().save(&p).unwrap();
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

fn table_of(h: &Harness<TerrafluxApp>, node: &str, port: &str) -> Table {
    match value(h, node, port) {
        Value::Table(t) => t,
        other => panic!("{other:?}"),
    }
}

fn table_id(node: &str, port: &str) -> egui::Id {
    egui::Id::new(("table", "viewer", node, port))
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

/// The chart of a table shown: its kind.
fn kind_of(h: &Harness<TerrafluxApp>, node: &str, port: &str) -> Kind {
    let state: terraflux_app::table_view::TableState =
        h.ctx.data(|d| d.get_temp(table_id(node, port))).expect("the table's state");
    state.chart.kind
}

/// The cells (row, column) of the matrix with a count, off the diagonal or on it.
fn cells_with_objects(m: &Table, diagonal: bool) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    for (i, r) in m.rows.iter().enumerate() {
        for j in 0..m.rows.len() {
            if (i == j) == diagonal && r[1 + j] > 0.0 {
                v.push((i, j));
            }
        }
    }
    v
}

/// The ids of the objects the `sampled` table says are mapped as `map` and referenced as `reference`.
fn ids_of(h: &Harness<TerrafluxApp>, map: &str, reference: &str) -> Vec<u32> {
    let t = table_of(h, "accuracy", "sampled");
    t.rows.iter().filter(|r| t.cell(1, r[1]) == map && t.cell(2, r[2]) == reference).map(|r| r[0] as u32).collect()
}

#[test]
fn the_matrix_of_objects_accuracy_is_a_chart_with_the_accuracies_in_its_margins() {
    let mut h = harness("confusion-view-chart");
    run_and_show(&mut h, "accuracy");
    show(&mut h, "accuracy", "matrix");
    let m = table_of(&h, "accuracy", "matrix");
    assert!(charts::is_confusion(&m) && Kind::of(&m).contains(&Kind::Confusion));
    until(&mut h, "the chart", |h| charts::confusion_screen(&h.ctx, table_id("accuracy", "matrix"), (0, 0)).is_some());
    assert_eq!(kind_of(&h, "accuracy", "matrix"), Kind::Confusion);
    // the margins are the accuracy table's numbers: the user's and the producer's of each class, the overall
    let c = charts::confusion_of(&m).unwrap();
    let t = table_of(&h, "accuracy", "table");
    let (user, producer) = (t.column("user_objects").unwrap(), t.column("producer_objects").unwrap());
    assert_eq!(c.user.len(), user.len());
    for k in 0..user.len() {
        assert!((c.user[k] - user[k]).abs() < 1e-12 || (c.user[k].is_nan() && user[k].is_nan()), "user {k}");
        assert!((c.producer[k] - producer[k]).abs() < 1e-12 || (c.producer[k].is_nan() && producer[k].is_nan()));
    }
    let o = table_of(&h, "accuracy", "overall");
    assert!((c.overall - o.column("overall_objects").unwrap()[0]).abs() < 1e-12);
    assert_eq!(c.total, o.column("objects").unwrap()[0]);
    // the names of the classes are the table's
    assert_eq!(c.map, c.reference);
}

#[test]
fn a_cell_clicked_lights_exactly_the_objects_of_its_pair_in_the_viewer() {
    let mut h = harness("confusion-view-link");
    run_and_show(&mut h, "accuracy");
    show(&mut h, "accuracy", "matrix");
    let m = table_of(&h, "accuracy", "matrix");
    let c = charts::confusion_of(&m).unwrap();
    let errors = table_of(&h, "accuracy", "errors");
    assert!(!errors.rows.is_empty(), "the reference was named wrongly on purpose");
    until(&mut h, "the chart", |h| charts::confusion_screen(&h.ctx, table_id("accuracy", "matrix"), (0, 0)).is_some());
    let o = match value(&h, "classify", "objects") {
        Value::Objects(o) => o,
        other => panic!("{other:?}"),
    };
    // an off-diagonal cell, then a cell of the diagonal
    for (diagonal, (i, j)) in [(false, cells_with_objects(&m, false)[0]), (true, cells_with_objects(&m, true)[0])] {
        show(&mut h, "accuracy", "matrix");
        let at = charts::confusion_screen(&h.ctx, table_id("accuracy", "matrix"), (i, j)).expect("the cell drawn");
        click_at(&mut h, at);
        let pick = (c.map[i].clone(), c.reference[j].clone());
        assert_eq!(charts::confusion_picked(&h.ctx, table_id("accuracy", "matrix")), Some(pick.clone()));
        assert_eq!(confusion_link::linked(&h.ctx, "accuracy"), Some(pick.clone()));
        // the ids of the pair: the misclassified ones are the errors' rows of it; those of the diagonal, the others
        let want = ids_of(&h, &pick.0, &pick.1);
        assert_eq!(want.len() as f64, m.rows[i][1 + j], "the cell's count");
        if !diagonal {
            let from_errors: Vec<u32> = errors
                .rows
                .iter()
                .filter(|r| errors.cell(1, r[1]) == pick.0 && errors.cell(2, r[2]) == pick.1)
                .map(|r| r[0] as u32)
                .collect();
            assert_eq!(want, from_errors);
        }
        assert_eq!(confusion_link::ids(h.state().session.as_ref().unwrap(), "accuracy", &pick), want);
        // the objects shown (their map too): those of the pair are lit, all of them and only them
        for port in ["objects", "map"] {
            show(&mut h, "classify", port);
            let lit = confusion_link::lit(&h.ctx, "accuracy").expect("the pick lit");
            assert_eq!(lit.data.len(), o.labels.len());
            for (v, &l) in lit.data.iter().zip(o.labels.iter()) {
                match (l, *v) {
                    (0, v) => assert!(v.is_nan()),
                    (l, v) => assert_eq!(v == 2.0, want.contains(&l), "object {l}, {port}"),
                }
            }
        }
        // the cell clicked again lets go
        // (the list of the pair's objects is above the chart now: the cell is somewhere else)
        show(&mut h, "accuracy", "matrix");
        let again = charts::confusion_screen(&h.ctx, table_id("accuracy", "matrix"), (i, j)).expect("the cell drawn");
        assert_ne!(again, at);
        click_at(&mut h, again);
        assert_eq!(confusion_link::linked(&h.ctx, "accuracy"), None);
    }
}

#[test]
fn the_cross_validations_matrix_is_the_same_chart_without_a_link_to_objects() {
    let mut h = harness("confusion-view-cv");
    run_and_show(&mut h, "model");
    show(&mut h, "model", "cv_matrix");
    let m = table_of(&h, "model", "cv_matrix");
    assert!(charts::is_confusion(&m) && Kind::of(&m).contains(&Kind::Confusion));
    until(&mut h, "the chart", |h| charts::confusion_screen(&h.ctx, table_id("model", "cv_matrix"), (0, 0)).is_some());
    assert_eq!(kind_of(&h, "model", "cv_matrix"), Kind::Confusion);
    // its margins are the matrix's own; a click picks the cell in the chart and links to no objects
    let c = charts::confusion_of(&m).unwrap();
    assert_eq!(c.total, 19.0);
    let at = charts::confusion_screen(&h.ctx, table_id("model", "cv_matrix"), (0, 0)).unwrap();
    click_at(&mut h, at);
    assert_eq!(
        charts::confusion_picked(&h.ctx, table_id("model", "cv_matrix")),
        Some((c.map[0].clone(), c.reference[0].clone()))
    );
    assert_eq!(confusion_link::linked(&h.ctx, "model"), None);
}
