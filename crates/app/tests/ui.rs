//! The application end to end, rendered off screen: a model opened (its data read in a thread), run with its
//! maps arriving step by step, its inputs' maps gone through, its tabs drawn; and the wizard, its data checked as
//! soon as it is chosen. The frames are written to `target/tmp/ui-*.png` to be looked at.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_app::editor_view::NodeStatus;
use terraflux_app::wizard::{Picked, Step, Wizard};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

/// A Dinamica model of Sibuyan written by the wizard.
fn model() -> PathBuf {
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    w.save(&tmp("ui-model.toml")).unwrap()
}

fn shot(h: &mut Harness<TerrafluxApp>, name: &str) {
    drawn(h);
    match h.render() {
        Ok(img) => img.save(tmp(&format!("ui-{name}.png"))).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
    }
}

/// The tests' own settings, not the user's; no computer shared or lent unless a test does it (on this computer
/// only).
fn setup() {
    unsafe { std::env::set_var("TERRAFLUX_SETTINGS", tmp("settings.json")) };
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
}

/// The application, with the tests' settings.
fn new_app(model: Option<PathBuf>) -> TerrafluxApp {
    setup();
    TerrafluxApp::new(model)
}

/// The application rendered off screen by wgpu, the builder's canvas pipelines installed as the application
/// does.
fn harness(app: TerrafluxApp) -> Harness<'static, TerrafluxApp> {
    setup();
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    terraflux_app::canvas_gpu::install(&rs);
    Harness::builder()
        .with_size((1280.0, 820.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app)
}

/// Step the application until `done`, two minutes at most.
fn until(h: &mut Harness<TerrafluxApp>, what: &str, done: impl Fn(&TerrafluxApp) -> bool) {
    let t = Instant::now();
    while !done(h.state()) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "{what}: too long");
    }
}

fn loaded(h: &mut Harness<TerrafluxApp>) {
    until(h, "reading the data", |a| a.session.as_ref().is_some_and(|s| !s.loading()));
}

/// Step until the maps shown are coloured (in a thread) and nothing runs.
fn drawn(h: &mut Harness<TerrafluxApp>) {
    h.run_steps(2);
    until(h, "drawing the maps", |a| {
        a.session.as_ref().is_none_or(|s| !s.view.busy() && !s.preview_view.busy() && !s.running())
    });
    h.run_steps(2);
}

#[test]
fn a_model_runs_and_every_tab_draws() {
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    h.state_mut().focus(Pane::Simulation);
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().can_run(), "{:?}", h.state().message);
    assert_eq!(h.state().session.as_ref().unwrap().maps.len(), 1, "the start map is shown");
    shot(&mut h, "1-open");
    // the inputs' maps: the drivers, a stack, gone through one map after the other
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let names: Vec<&str> = s.layers.iter().map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["$drivers", "$t0", "$t1"]);
        let d = &s.layers[0];
        assert!(d.stack && d.bands.len() > 1);
        s.layer = Some(0);
        s.set_frame(2);
        assert_eq!(s.layers[0].shown, 2);
    }
    h.run_steps(3);
    assert!(h.query_by_label("MAP OF THE STACK").is_some());
    shot(&mut h, "1b-stack");
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().layers[0].shown, 3, "right: the next map");
    h.state_mut().session.as_mut().unwrap().layer = None;
    h.run_steps(2);
    h.get_by_label("Run").click();
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().running() || h.state().session.as_ref().unwrap().maps.len() < 2 {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
        assert!(t.elapsed() < Duration::from_secs(120), "the run did not end");
    }
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.error.is_none(), "{:?}", s.error);
    assert_eq!(s.maps.len(), 15, "the start and 14 steps");
    assert_eq!(s.tables.len(), 14);
    assert_eq!(s.shown, 14, "the view follows the run");
    assert_eq!(s.counts.len(), 15, "the areas of every map, counted in the run's thread");
    shot(&mut h, "2-map");
    // run again: nothing changed, every node from the cache, the maps given back
    h.get_by_label("Run").click();
    h.run_steps(1);
    until(&mut h, "the run from the cache", |a| !a.session.as_ref().unwrap().running());
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.maps.len(), 15);
    assert!(s.status.values().all(|st| *st == NodeStatus::Done { cached: true }), "{:?}", s.status);
    for (pane, name) in [(Pane::Areas, "3-charts"), (Pane::Model, "4-graph")] {
        h.state_mut().focus(pane);
        h.run_steps(3);
        shot(&mut h, name);
    }
    // the model as text: a button of the model's tab
    h.get_by_label("Text").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.text);
    shot(&mut h, "5-model");
    h.get_by_label("Diagram").click();
    h.run_steps(2);
    // the graph: a node's note when it is selected
    h.state_mut().focus(Pane::Model);
    h.state_mut().session.as_mut().unwrap().selected = Some("markov".into());
    h.run_steps(2);
    shot(&mut h, "4b-graph-node");
    assert!(h.query_by_label("Why").is_some(), "the selected node's note");
    // validation against the observed map of 2011
    h.state_mut().focus(Pane::Validation);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.validation.observed = Some(data("sibuyan_landuse.tif"));
        s.validation.bands = vec!["1997".into(), "2011".into()];
        s.validation.band = 1;
        s.validate();
        assert!(s.validation.error.is_none(), "{:?}", s.validation.error);
        let a = s.validation.agreement.as_ref().unwrap();
        assert!(a.figure_of_merit > 0.0 && a.figure_of_merit < 1.0);
        assert_eq!(s.validation.similarity.as_ref().unwrap().rows.len(), 6);
    }
    h.run_steps(3);
    assert!(h.query_by_label("figure of merit").is_some());
    shot(&mut h, "6-validation");
}

#[test]
fn the_wizard_checks_the_data() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    h.get_by_label("Wizard...").click();
    h.run_steps(2);
    assert!(h.state().wizard.is_some());
    assert!(h.query_by_label_contains("Dinamica EGO's classic model").is_some());
    shot(&mut h, "7-wizard");
    h.get_by_label("Next").click();
    h.run_steps(2);
    {
        let w = h.state_mut().wizard.as_mut().unwrap();
        assert_eq!(w.step, Step::Data);
        w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
        // a misaligned driver: Plum Island's
        w.drivers = Some(Picked::header(&data("pie_drivers.tif")).unwrap());
    }
    // checked at once, in a thread, on the same page
    h.run_steps(1);
    until(&mut h, "the check", |a| a.wizard.as_ref().unwrap().is_checked());
    h.run_steps(3);
    let w = h.state().wizard.as_ref().unwrap();
    assert!(w.blocked(), "a driver on another grid must stop the wizard");
    assert!(h.query_by_label_contains("resample it to the reference grid").is_some());
    h.get_by_label("Next").click();
    h.run_steps(2);
    assert_eq!(h.state().wizard.as_ref().unwrap().step, Step::Data, "no next step with a driver on another grid");
    shot(&mut h, "8-wizard-check");
    // the right drivers: nothing to stop at, the settings next
    h.state_mut().wizard.as_mut().unwrap().drivers = Some(Picked::header(&data("sibuyan_drivers.tif")).unwrap());
    h.run_steps(1);
    until(&mut h, "the check", |a| a.wizard.as_ref().unwrap().is_checked());
    h.run_steps(2);
    assert!(!h.state().wizard.as_ref().unwrap().blocked());
    assert!(h.query_by_label_contains("nothing looks wrong").is_some());
    shot(&mut h, "8b-wizard-data");
    h.get_by_label("Next").click();
    h.run_steps(2);
    assert_eq!(h.state().wizard.as_ref().unwrap().step, Step::Settings);
    shot(&mut h, "8c-wizard-settings");
}

#[test]
fn the_editor_changes_the_graph() {
    let path = tmp("ui-edited.toml");
    std::fs::copy(model(), &path).unwrap();
    let mut h = harness(new_app(Some(path.clone())));
    loaded(&mut h);
    h.state_mut().focus(Pane::Model);
    h.run_steps(3);
    shot(&mut h, "9-editor");
    // a node of map algebra, added from the library and connected in the side panel's terms
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.selected = None;
        terraflux_app::builder::add(s, &terraflux_ops::Registry::standard(), "map.calc", &[]).unwrap();
    }
    h.run_steps(3);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert_eq!(s.selected.as_deref(), Some("map_calc"));
        let r = terraflux_ops::Registry::standard();
        terraflux_app::editor::connect(&mut s.source, &r, &[], "$t0", "map_calc", "land").unwrap();
        terraflux_app::editor::set_param(&mut s.source, &[], "map_calc", "expression", "land == 1").unwrap();
        s.edited();
        assert!(s.model.node("map_calc").is_some());
        assert!(!s.issues.iter().any(|i| i.at.starts_with("map_calc")), "{:?}", s.issues);
    }
    h.run_steps(3);
    assert!(h.query_by_label("Delete node").is_some(), "the selected node's panel");
    shot(&mut h, "9b-editor-node");
    // into the loop's body and back
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["simulation".into()];
    h.run_steps(3);
    assert!(h.query_by_label("simulation").is_some(), "the breadcrumb shows the container");
    {
        let s = h.state().session.as_ref().unwrap();
        let r = terraflux_ops::Registry::standard();
        let src = terraflux_app::editor::sources(&s.source, &r, &s.editor.path);
        assert!(src.iter().any(|(x, _)| x == "@landscape"), "the loop's carried value is a source: {src:?}");
    }
    shot(&mut h, "9c-editor-body");
    h.get_by_label("Top level").click();
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.path.is_empty());
    // File > Save
    h.get_by_label("File").click();
    h.run_steps(2);
    // (its label has its shortcut too; Save as is after it)
    h.query_all_by_label_contains("Save").next().expect("File's Save").click();
    h.run_steps(2);
    let saved = terraflux_graph::Model::load(&path).unwrap();
    assert_eq!(saved.node("map_calc").unwrap().params["expression"], "land == 1");
    assert!(saved.node("map_calc").unwrap().pos.is_some());
    assert!(!h.state().session.as_ref().unwrap().dirty);
}

/// The builder from nothing: a raster opened, MSPA on it shown at once, a setting changed (only MSPA runs
/// again), the result saved as a raster and the model as a file.
#[test]
fn the_builder_reads_analyses_and_writes() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    assert!(h.state().is_open(Pane::Model), "the application opens in the builder");
    for p in [Pane::Simulation, Pane::Areas, Pane::Validation] {
        assert!(!h.state().is_open(p), "{p:?} opens from the View menu only");
    }
    // the View menu: a pane off, then on again (beside the viewer)
    h.state_mut().focus(Pane::Areas);
    assert!(h.state().is_open(Pane::Areas));
    h.state_mut().close(Pane::Areas);
    assert!(!h.state().is_open(Pane::Areas));
    // a pane in a window of its own (another screen), then back
    h.state_mut().detach(Pane::Viewer);
    h.run_steps(3);
    assert_eq!(h.state().detached(), [Pane::Viewer]);
    assert!(h.state().is_open(Pane::Viewer));
    h.state_mut().attach(Pane::Viewer);
    h.run_steps(2);
    assert!(h.state().detached().is_empty() && h.state().is_open(Pane::Viewer));
    shot(&mut h, "10-builder-empty");
    let r = terraflux_ops::Registry::standard();
    let out = tmp("ui-mspa.tif");
    let _ = std::fs::remove_file(&out);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        let read = terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        assert_eq!(read, "raster_read");
        // forest (class 1) as the foreground, connected to the map read
        terraflux_app::builder::add(s, &r, "landscape.mspa", &[("foreground", 1.into())]).unwrap();
        let mspa = s.source.nodes.iter().find(|n| n.id == "landscape_mspa").unwrap();
        assert_eq!(
            mspa.inputs["map"],
            terraflux_graph::Source::Ref("raster_read.raster".into()),
            "fed by the node selected"
        );
        assert_eq!(s.preview, Some(("landscape_mspa".into(), "map".into())));
    }
    // by itself after the edit: the read and MSPA run, the result shown
    until(&mut h, "MSPA", |a| {
        a.session.as_ref().unwrap().status.get("landscape_mspa") == Some(&NodeStatus::Done { cached: false })
    });
    let first = {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(m)) = s.preview_value() else { panic!("{:?}", s.error) };
        assert!(m.data.contains(&17.0), "core");
        m
    };
    shot(&mut h, "11-builder-mspa");
    // a wider edge: the map read comes from the cache, MSPA runs again
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::editor::set_param(&mut s.source, &[], "landscape_mspa", "edge_width", "3").unwrap();
        s.edited();
    }
    until(&mut h, "MSPA again", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.status.get("landscape_mspa") == Some(&NodeStatus::Done { cached: false })
    });
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.status.get("raster_read"), Some(&NodeStatus::Done { cached: true }));
        let Some(terraflux_ops::Value::Map(m)) = s.preview_value() else { panic!() };
        assert_ne!(m.data, first.data, "a wider edge, fewer cores");
    }
    // saved as a raster: written as soon as it is added (the node shown)
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let p = out.to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.write", &[("path", p.into())]).unwrap();
    }
    until(&mut h, "the raster written", |a| {
        a.session.as_ref().unwrap().status.get("raster_write") == Some(&NodeStatus::Done { cached: false })
    });
    let written = terraflux_raster::Raster::read(&out).unwrap();
    assert_eq!(written.dtype, terraflux_raster::DataType::U8);
    shot(&mut h, "12-builder-saved");
    // the model saved: its files relative to its folder
    let model = tmp("ui-builder.toml");
    h.state_mut().session.as_mut().unwrap().save_as(&model).unwrap();
    let m = terraflux_graph::Model::load(&model).unwrap();
    let p = m.node("raster_read").unwrap().params["path"].as_str().unwrap().to_string();
    assert!(!std::path::Path::new(&p).is_absolute() && p.ends_with("sibuyan_landuse.tif"), "{p}");
    let again = terraflux_graph::Model::load_expanded(&model).unwrap();
    let r = terraflux_exec::Engine::default().run(&again, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
    assert!(r.get("landscape_mspa.map").is_some(), "the saved model runs from the command line's engine too");
}

/// Read, reclassify, plot: the plot's map tab opens by itself, nothing written; saved from the tab (a file, then
/// a write node in the model too); a rule changed, the tab shows the new map.
#[test]
fn a_plot_shows_without_writing_and_saves_from_its_tab() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "view.plot", &[("title", "forest".into())]).unwrap();
        let plot = s.source.nodes.iter().find(|n| n.id == "view_plot").unwrap();
        assert_eq!(plot.inputs["value"], terraflux_graph::Source::Ref("raster_reclassify.map".into()));
    }
    until(&mut h, "the plot's tab", |a| a.session.as_ref().unwrap().map_tabs.values().any(|t| t.node == "view_plot"));
    until(&mut h, "the plot", |a| !a.session.as_ref().unwrap().running());
    h.run_steps(3);
    let (id, forest) = {
        let s = h.state().session.as_ref().unwrap();
        let (id, _) = s.map_tabs.iter().find(|(_, t)| t.node == "view_plot").unwrap();
        let Some(terraflux_ops::Value::Map(m)) = s.value_of("view_plot", "shown") else { panic!("{:?}", s.error) };
        assert!(m.data.iter().all(|v| v.is_nan() || *v == 0.0 || *v == 1.0));
        (*id, m)
    };
    assert!(h.state().is_open(Pane::Map(id)));
    shot(&mut h, "14-plot");
    // saved from the tab: the same cells in the file
    let out = tmp("ui-forest.tif");
    let _ = std::fs::remove_file(&out);
    h.state_mut().session.as_mut().unwrap().save_output("view_plot", "shown", &out);
    until(&mut h, "the file", |a| !a.session.as_ref().unwrap().saving());
    let saved = terraflux_raster::Raster::read(&out).unwrap();
    let back = saved.bands[0].masked(saved.nodata);
    assert!(back.iter().zip(forest.data.iter()).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
    // and a write node in the model, that writes it at every run
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = terraflux_app::builder::add_write(s, &r, "view_plot", "shown", &out).unwrap();
        let w = s.source.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(w.inputs["value"], terraflux_graph::Source::Ref("view_plot.shown".into()));
    }
    // another rule: the tab shows the new map
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.selected = Some("view_plot".into());
        s.preview = Some(("view_plot".into(), "shown".into()));
        terraflux_app::editor::set_param(&mut s.source, &[], "raster_reclassify", "rules", "1 2 -> 1\n* -> 0").unwrap();
        s.edited();
    }
    h.run_steps(1);
    until(&mut h, "the new plot", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.status.contains_key("view_plot")
    });
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Map(m)) = s.value_of("view_plot", "shown") else { panic!() };
    let ones = |m: &terraflux_ops::Map| m.data.iter().filter(|v| **v == 1.0).count();
    assert!(ones(&m) > ones(&forest), "class 2 joined the ones");
}

/// A basemap under the data: Esri's imagery warped onto the map's grid (UTM 51N), the data half transparent.
/// Without the web, said and passed.
#[test]
fn a_basemap_shows_under_the_data() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        // the file has no CRS: Sibuyan is in UTM 51N
        terraflux_app::builder::add(
            s,
            &r,
            "raster.reproject",
            &[("crs", "EPSG:32651".into()), ("source_crs", "EPSG:32651".into())],
        )
        .unwrap();
    }
    until(&mut h, "the map with a CRS", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.preview_value().is_some()
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_view.basemap = terraflux_app::map_view::Basemap::EsriImagery;
        s.preview_view.transparency = 0.5;
    }
    h.run_steps(2);
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().preview_view.base_busy() && t.elapsed() < Duration::from_secs(60) {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    h.run_steps(3);
    let (tiles, error) = h.state().session.as_ref().unwrap().preview_view.base_tiles();
    match error {
        Some(e) if tiles == 0 => eprintln!("no basemap ({e}): not tested"),
        _ => {
            assert!(tiles > 0, "basemap tiles");
            shot(&mut h, "15-basemap");
        }
    }
}

/// A template opens in the builder and runs by itself: its plot's tab in MSPA's colours. A node's map asked for
/// twice is one tab, brought to the front. "/" searches the operators: enter adds the one chosen after the node
/// selected, escape closes the search.
#[test]
fn templates_run_and_the_keyboard_adds_operators() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    until(&mut h, "the template's plot", |a| a.session.as_ref().unwrap().map_tabs.values().any(|t| t.node == "plot"));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("plot", "shown").is_some()
    });
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(m)) = s.value_of("mspa", "map") else { panic!("{:?}", s.error) };
        assert!(m.data.contains(&17.0) && m.data.contains(&3.0), "core and edge");
    }
    until(&mut h, "the legend", |a| {
        let s = a.session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(m)) = s.value_of("plot", "shown") else { return false };
        s.looks
            .peek(&terraflux_app::map_view::Source::from(&m))
            .is_some_and(|l| l.product == Some(terraflux_ops::landscape::Product::Mspa))
    });
    shot(&mut h, "15-template");
    // the reclassification's rules as a table, the classes of the map it reads beside them
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "forest");
    }
    h.run_steps(4);
    shot(&mut h, "17-reclassify");
    // a map asked for again: the same tab
    let first = h.state_mut().session.as_mut().unwrap().open_map("mspa", "map");
    h.run_steps(2);
    let again = h.state_mut().session.as_mut().unwrap().open_map("mspa", "map");
    h.run_steps(2);
    assert_eq!(first, again);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.map_tabs.values().filter(|t| t.node == "mspa").count(), 1);
    assert!(h.state().is_open(Pane::Map(first)));
    // closed, it opens anew
    h.state_mut().close(Pane::Map(first));
    h.state_mut().session.as_mut().unwrap().map_tabs.remove(&first);
    let third = h.state_mut().session.as_mut().unwrap().open_map("mspa", "map");
    h.run_steps(2);
    assert_ne!(third, first);
    assert!(h.state().is_open(Pane::Map(third)));
    // "/", a few letters, enter: added after the node selected
    h.state_mut().session.as_mut().unwrap().selected = Some("mspa".into());
    h.event(egui::Event::Text("/".into()));
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.quick.is_some(), "the search opens");
    h.event(egui::Event::Text("write".into()));
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().editor.quick.as_ref().unwrap().query, "write");
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none(), "enter closes it");
    let w = s.source.nodes.iter().find(|n| n.op == "raster.write").expect("raster.write added");
    assert_eq!(w.inputs["value"], terraflux_graph::Source::Ref("mspa.map".into()));
    assert_eq!(s.selected.as_deref(), Some(w.id.as_str()));
    // and escape closes it, adding nothing
    let n = s.source.nodes.len();
    h.event(egui::Event::Text("/".into()));
    h.run_steps(2);
    shot(&mut h, "16-search");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.quick.is_none());
    assert_eq!(s.source.nodes.len(), n);
}

/// GuidosToolbox's fragmentation in the builder: FAD-MS's average shown in GTB's colours, its legend in the
/// classes of a density (rare to intact) rather than a line per value; FOS in 5 classes.
#[test]
fn fragmentation_is_shown_in_its_classes() {
    use terraflux_ops::landscape::Product;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "Fragmentation of a forest").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("plot_fad", "shown").is_some()
    });
    fn look(h: &mut Harness<TerrafluxApp>, node: &str) -> terraflux_app::app::Look {
        let of = |a: &TerrafluxApp| {
            let s = a.session.as_ref().unwrap();
            let Some(terraflux_ops::Value::Map(m)) = s.value_of(node, "shown") else { return None };
            s.looks.peek(&terraflux_app::map_view::Source::from(&m)).filter(|l| l.product.is_some()).cloned()
        };
        until(h, "the legend", |a| of(a).is_some());
        of(h.state()).unwrap()
    }
    // the template's two plots open in tabs: FAD's in front
    h.state_mut().session.as_mut().unwrap().open_map("plot_fad", "shown");
    let fad = look(&mut h, "plot_fad");
    assert_eq!(fad.product, Some(Product::Density { five: false }));
    let terraflux_app::palette::Coloring::Classes(c) = &fad.coloring else { panic!("classes") };
    let lines = terraflux_app::map_view::legend_lines(c, &fad.counts, &|v| terraflux_app::builder::class_name(&fad, v));
    assert!(c.len() > 20 && lines.len() <= 8, "{} values in {} lines: {lines:?}", c.len(), lines.len());
    assert!(lines.iter().any(|(_, n, _)| n == "Background"));
    shot(&mut h, "18-fragmentation");
    // FOS's tab: in 5 classes
    h.state_mut().session.as_mut().unwrap().open_map("plot_fos", "shown");
    let fos = look(&mut h, "plot_fos");
    assert_eq!(fos.product, Some(Product::Density { five: true }));
}

/// The landscape mosaic shown in GTB's 19 classes by name; the contagion's legend in bands of 10%.
#[test]
fn the_mosaic_and_contagion_are_named() {
    use terraflux_ops::landscape::Product;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "Landscape mosaic").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("plot_contagion", "shown").is_some()
    });
    let lines = |h: &mut Harness<TerrafluxApp>, node: &str| {
        h.state_mut().session.as_mut().unwrap().open_map(node, "shown");
        let of = |a: &TerrafluxApp| {
            let s = a.session.as_ref().unwrap();
            let Some(terraflux_ops::Value::Map(m)) = s.value_of(node, "shown") else { return None };
            s.looks.peek(&terraflux_app::map_view::Source::from(&m)).filter(|l| l.product.is_some()).cloned()
        };
        until(h, "the legend", |a| of(a).is_some());
        let look = of(h.state()).unwrap();
        let terraflux_app::palette::Coloring::Classes(c) = &look.coloring else { panic!("classes") };
        let names = |v| terraflux_app::builder::class_name(&look, v);
        (look.product, terraflux_app::map_view::legend_lines(c, &look.counts, &names))
    };
    let (product, mosaic) = lines(&mut h, "plot_mosaic");
    assert_eq!(product, Some(Product::Mosaic { classes: 19 }));
    assert!(mosaic.iter().any(|(_, n, _)| n.starts_with("Forest") || n.starts_with("Mosaic")), "{mosaic:?}");
    shot(&mut h, "19-mosaic");
    let (product, contagion) = lines(&mut h, "plot_contagion");
    assert_eq!(product, Some(Product::Connectivity));
    assert!(contagion.len() <= 12 && contagion.iter().any(|(_, n, _)| n == "90-99%" || n == "100%"), "{contagion:?}");
}

/// Landscape metrics in the builder: the table of the classes sorted by a column and drawn as bars, the map of a
/// metric in the plot's tab.
#[test]
fn metrics_show_as_tables_bars_and_maps() {
    use terraflux_app::charts::{Chart, Kind};
    use terraflux_app::table_view::{TableState, rows_shown};
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "Landscape metrics").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("plot_window", "shown").is_some()
    });
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Table(table)) = s.value_of("classes", "table") else { panic!("{:?}", s.error) };
    assert_eq!(table.rows.len(), 3);
    assert_eq!(table.columns[..3], ["class", "ca", "pland"]);
    let pland: f64 = table.rows.iter().map(|r| r[2]).sum();
    assert!((pland - 100.0).abs() < 1e-9, "the shares add up: {pland}");
    // sorted by the share, most first, drawn as bars
    let chart = Chart { kind: Kind::Bars, columns: vec![2], x: Some(0), by: None, ..Default::default() };
    let st = TableState { sort: Some((2, false)), filter: String::new(), chart };
    let order = rows_shown(&table, &st).unwrap();
    assert!(table.rows[order[0]][2] >= table.rows[order[2]][2]);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = Some(("classes".into(), "table".into()));
    }
    let id = egui::Id::new(("table", "viewer", &"classes".to_string(), &"table".to_string()));
    h.ctx.data_mut(|d| d.insert_temp(id, st.clone()));
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(4);
    assert_eq!(h.ctx.data(|d| d.get_temp::<TableState>(id)), Some(st), "the viewer keeps the table's state");
    shot(&mut h, "20-metrics");
}

/// Zonation's rank in its ramp (dark to red), the features built by stack.of.
#[test]
fn zonation_rank_in_its_colours() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "Zonation").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("plot_rank", "shown").is_some()
    });
    h.state_mut().session.as_mut().unwrap().open_map("plot_rank", "shown");
    until(&mut h, "the legend", |a| {
        let s = a.session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(m)) = s.value_of("plot_rank", "shown") else { return false };
        s.looks.peek(&terraflux_app::map_view::Source::from(&m)).is_some_and(|l| {
            matches!(
                l.coloring,
                terraflux_app::palette::Coloring::Continuous { ramp: terraflux_app::palette::Ramp::Rank, .. }
            )
        })
    });
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Table(curves)) = s.value_of("zonation", "curves") else { panic!("{:?}", s.error) };
    assert_eq!(curves.columns[2..], ["bird", "frog", "orchid", "wetland"]);
    shot(&mut h, "21-zonation");
}

/// Every template that needs no file runs from the command line's engine.
#[test]
fn templates_run() {
    for t in &terraflux_app::templates::TEMPLATES {
        let m = t.model();
        if m.nodes.iter().any(|n| n.op == "raster.read") {
            continue;
        }
        let r = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet);
        assert!(r.is_ok(), "{}: {:?}", t.name, r.err());
    }
}

/// Vector features read in the builder: drawn (polygons, lines, points) beside their attributes.
#[test]
fn vectors_are_drawn_with_their_attributes() {
    if terraflux_gdal::gdal().is_err() {
        eprintln!("skipped: no GDAL");
        return;
    }
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/vectors/mixed.gpkg");
        terraflux_app::builder::add(s, &r, "vector.read", &[("path", path.to_string_lossy().into_owned().into())])
            .unwrap();
    }
    until(&mut h, "reading the vectors", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && matches!(s.preview_value(), Some(terraflux_ops::Value::Vector(_)))
    });
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Vector(v)) = s.preview_value() else { panic!("{:?}", s.error) };
    assert_eq!(v.len(), 6);
    shot(&mut h, "22-vectors");
}

/// ctrl+D duplicates the node selected, ctrl+C and ctrl+V copy and paste the nodes selected (through the system's
/// clipboard): new nodes each time, of new ids, connected to each other as the originals were.
#[test]
fn keys_duplicate_copy_and_paste() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let r = terraflux_ops::Registry::standard();
        terraflux_app::builder::add(s, &r, "nlm.fbm", &[]).unwrap();
        terraflux_app::builder::add(s, &r, "landscape.mspa", &[]).unwrap();
    }
    h.state_mut().focus(Pane::Model);
    h.run_steps(3);
    // ctrl+D: a new node beside the one selected, reading what it read
    h.state_mut().session.as_mut().unwrap().selected = Some("landscape_mspa".into());
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::D);
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        let (a, b) = (s.source.node("landscape_mspa").unwrap(), s.source.node("landscape_mspa_2").unwrap());
        assert_eq!(a.inputs, b.inputs);
        assert_ne!(a.pos, b.pos);
        assert_eq!(s.selected.as_deref(), Some("landscape_mspa_2"));
    }
    // ctrl+C with both selected: their text in the clipboard
    h.state_mut().session.as_mut().unwrap().editor.chosen =
        ["nlm_fbm".to_string(), "landscape_mspa".to_string()].into();
    h.run_steps(1);
    h.event(egui::Event::Copy);
    h.step();
    let text = h
        .output()
        .platform_output
        .commands
        .iter()
        .find_map(|c| match c {
            egui::OutputCommand::CopyText(t) => Some(t.clone()),
            _ => None,
        })
        .expect("the nodes copied");
    // ctrl+V, twice: two new pairs, each connected within itself
    for (fbm, mspa) in [("nlm_fbm_2", "landscape_mspa_3"), ("nlm_fbm_3", "landscape_mspa_4")] {
        h.event(egui::Event::Paste(text.clone()));
        h.run_steps(2);
        let s = h.state().session.as_ref().unwrap();
        let m = s.source.node(mspa).unwrap_or_else(|| panic!("{mspa} pasted"));
        assert_eq!(m.inputs["map"], terraflux_graph::Source::Ref(format!("{fbm}.map")));
        assert!(s.source.node(fbm).is_some());
        assert_eq!(s.editor.chosen, [fbm.to_string(), mspa.to_string()].into());
    }
    assert_eq!(h.state().session.as_ref().unwrap().source.nodes.len(), 7);
    // ctrl+Z: the last paste undone, then the one before; ctrl+Y and ctrl+shift+Z: done again
    let ids = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().source.nodes.len();
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(2);
    assert_eq!(ids(&h), 5);
    assert!(h.state().session.as_ref().unwrap().source.node("nlm_fbm_3").is_none());
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(2);
    assert_eq!(ids(&h), 3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Y);
    h.run_steps(2);
    assert_eq!(ids(&h), 5);
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::Z);
    h.run_steps(2);
    assert_eq!(ids(&h), 7);
    // text not of nodes: nothing pasted
    h.event(egui::Event::Paste("hello".into()));
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().source.nodes.len(), 7);
}

/// The keyboard: Tab to the model and back to the tab it came from, ctrl+W closes the tab in focus, ctrl+R
/// renames the node selected; a right click on a connected input takes its connection away; the text of the
/// model is a button of the model's tab.
#[test]
fn keys_switch_close_rename_and_disconnect() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let r = terraflux_ops::Registry::standard();
        terraflux_app::builder::add(s, &r, "nlm.fbm", &[]).unwrap();
        terraflux_app::builder::add(s, &r, "landscape.mspa", &[]).unwrap();
        assert!(s.source.node("landscape_mspa").unwrap().inputs.contains_key("map"), "connected when added");
    }
    h.run_steps(3);
    // Tab: from the viewer to the model, and back
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(2);
    assert_eq!(h.state_mut().focused(), Some(Pane::Viewer));
    h.key_press(egui::Key::Tab);
    h.run_steps(2);
    assert_eq!(h.state_mut().focused(), Some(Pane::Model));
    h.key_press(egui::Key::Tab);
    h.run_steps(2);
    assert_eq!(h.state_mut().focused(), Some(Pane::Viewer));
    // ctrl+W closes it
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::W);
    h.run_steps(2);
    assert!(!h.state().is_open(Pane::Viewer));
    // ctrl+R: the node selected renamed
    h.state_mut().focus(Pane::Model);
    h.state_mut().session.as_mut().unwrap().selected = Some("landscape_mspa".into());
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::R);
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.renaming.is_some());
    shot(&mut h, "12-rename");
    h.state_mut().session.as_mut().unwrap().editor.renaming = Some(("landscape_mspa".into(), "patterns".into(), true));
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.source.node("patterns").is_some() && s.source.node("landscape_mspa").is_none());
        assert_eq!(s.selected.as_deref(), Some("patterns"));
    }
    // a right click on the connected input
    let at = h.state().session.as_ref().unwrap().editor.screen_of_input("patterns", "map").unwrap();
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
    assert!(!h.state().session.as_ref().unwrap().source.node("patterns").unwrap().inputs.contains_key("map"));
    // the model as text, and back to the diagram
    h.get_by_label("Text").click();
    h.run_steps(2);
    assert!(h.query_by_label_contains("problems").is_some() || h.query_by_label_contains("No problem").is_some());
    h.get_by_label("Diagram").click();
    h.run_steps(2);
    assert!(!h.state().session.as_ref().unwrap().editor.text);
    // the metrics of a level: all, by type or one by one, in a list
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.auto = false;
        terraflux_app::builder::add(s, &terraflux_ops::Registry::standard(), "metrics.class", &[]).unwrap();
    }
    h.run_steps(3);
    shot(&mut h, "13a-metrics");
    h.get_by_value("all").click();
    h.run_steps(2);
    h.get_by_label_contains("area and edge").click();
    h.run_steps(2);
    shot(&mut h, "13-metrics");
    let s = h.state().session.as_ref().unwrap();
    let chosen = s.source.node("metrics_class").unwrap().params["metrics"].as_array().unwrap().clone();
    assert!(chosen.iter().any(|m| m == "ca") && chosen.iter().any(|m| m == "pland"), "{chosen:?}");
}

/// The builder without the mouse: with the model's canvas in focus the arrows go from node to node (the first
/// node first), enter shows the output of the node selected in the viewer.
#[test]
fn nodes_are_gone_through_with_the_keyboard() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(4);
    h.state_mut().focus(Pane::Model);
    h.run_steps(2);
    // the pointer over the canvas
    let canvas = h.state().session.as_ref().unwrap().editor.canvas().expect("the canvas drawn");
    h.event(egui::Event::PointerMoved(canvas.center()));
    h.run_steps(2);
    let selected = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().selected.clone();
    // to the first node of the chain, leftwards
    let mut last = None;
    for _ in 0..8 {
        h.key_press(egui::Key::ArrowLeft);
        h.run_steps(2);
        if selected(&h) == last {
            break;
        }
        last = selected(&h);
    }
    let first = last.expect("a node selected");
    h.key_press(egui::Key::ArrowRight);
    h.run_steps(2);
    let second = selected(&h).unwrap();
    assert_ne!(first, second, "the arrow goes to the next node");
    h.key_press(egui::Key::ArrowLeft);
    h.run_steps(2);
    assert_eq!(selected(&h).as_ref(), Some(&first), "and back");
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert_eq!(h.state_mut().focused(), Some(Pane::Viewer), "enter shows the node's output");
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.preview.as_ref().map(|(n, _)| n), Some(&first));
}

/// The preparation pane: the land use suggested as the reference grid and chosen in a click, the drivers (in
/// another coordinate system) put on it in another, a raster.match with its note added after them.
#[test]
fn data_is_put_on_the_reference_grid_in_a_click() {
    let d = tmp("prepare");
    std::fs::create_dir_all(&d).unwrap();
    let (land, utm, geo) = (d.join("land.tif"), d.join("drivers_utm.tif"), d.join("drivers_geo.tif"));
    let srs = ["-a_srs".to_string(), "EPSG:32651".to_string()];
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &srs).unwrap();
    terraflux_gdal::translate(&data("sibuyan_drivers.tif"), &utm, &srs).unwrap();
    terraflux_gdal::warp(&[&utm], &geo, &["-t_srs".into(), "EPSG:4326".into()]).unwrap();
    let mut m = terraflux_graph::Model::new("prepare");
    for (id, p) in [("land", &land), ("drivers", &geo)] {
        m.nodes.push(terraflux_graph::Node::new(id, "raster.read").param("path", p.to_string_lossy()));
    }
    let path = d.join("prepare.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().focus(Pane::Prepare);
    let r = terraflux_ops::Registry::standard();
    for id in ["drivers", "land"] {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &r, id);
        s.start_to(Some(id.into()), Box::new(|| {}));
        until(&mut h, "the run", |a| {
            let s = a.session.as_ref().unwrap();
            !s.running() && s.value_of(id, "raster").is_some()
        });
    }
    h.run_steps(3);
    shot(&mut h, "18-prepare-reference");
    h.get_by_label("Use land (classes)").click();
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().reference, Some(("land".into(), "raster".into())));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &r, "drivers");
    }
    h.run_steps(3);
    assert!(h.query_by_label_contains("another coordinate system").is_some(), "what differs from the grid");
    shot(&mut h, "19-prepare-drivers");
    h.get_by_label("Put on the reference grid").click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.op == "raster.match").expect("raster.match added");
    assert_eq!(n.inputs["raster"], terraflux_graph::Source::Ref("drivers.raster".into()));
    assert_eq!(n.inputs["like"], terraflux_graph::Source::Ref("land.raster".into()));
    assert!(n.note.as_ref().is_some_and(|t| t.contains("Resampling")), "{:?}", n.note);
    assert_eq!(s.selected.as_deref(), Some(n.id.as_str()));
}

/// The panes' places: the model in most of the window, the node's pane at its right (the preparation and the files
/// in tabs beside it); the operators in a drawer the pointer opens at the window's left edge (and closes leaving
/// it) and in the toolbar, their pane, shown, at the left edge; backspace deletes the node selected; a tab dragged
/// out of the dock (egui_dock's floating window) becomes a window of its own.
#[test]
fn the_operators_drawer_and_windows_of_their_own() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(4);
    assert!(!h.state().is_open(Pane::Operators), "the operators in their drawer at first");
    assert!(h.state().is_open(Pane::Node) && h.state().is_open(Pane::Prepare));
    let left = |h: &Harness<TerrafluxApp>, pane: Pane| {
        let dock = h.state().dock.as_ref().unwrap();
        let path = dock.find_tab(&pane).unwrap();
        dock[path.surface][path.node].rect().unwrap().left()
    };
    let (model, node) = (left(&h, Pane::Model), left(&h, Pane::Node));
    assert!(model < node, "the model, then the node from the left: {model} {node}");
    // the node's pane a fifth of the window
    let width = h.ctx.content_rect().width();
    assert!(((width - node) / width - 0.2).abs() < 0.03, "the node's width: {}", width - node);
    // the files in a tab beside the node's: the data of a folder, a raster carried to the model from there
    {
        let dock = h.state().dock.as_ref().unwrap();
        let (a, b) = (dock.find_tab(&Pane::Node).unwrap(), dock.find_tab(&Pane::Files).unwrap());
        assert_eq!((a.surface, a.node), (b.surface, b.node), "the files beside the node");
    }
    h.state_mut().files.go(data(""));
    h.state_mut().focus(Pane::Files);
    h.run_steps(3);
    assert!(h.query_by_label_contains("sibuyan_landuse.tif").is_some(), "the folder's rasters");
    // a raster chosen: described below the list (once read, in a thread)
    h.get_by_label("sibuyan_landuse.tif").click();
    let t = Instant::now();
    while h.query_by_label("cell size").is_none() {
        assert!(t.elapsed() < Duration::from_secs(60), "the raster described");
        h.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    h.run_steps(2);
    shot(&mut h, "20-files");
    // another with ctrl: both chosen; enter adds a node for each
    h.get_by_label("pie_landuse.tif").click_modifiers(egui::Modifiers::COMMAND);
    h.run_steps(2);
    assert!(h.query_by_label_contains("2 chosen").is_some());
    let before = h.state().session.as_ref().unwrap().source.nodes.len();
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.nodes.len(), before + 2);
        let paths: Vec<&str> = s.source.nodes[before..].iter().filter_map(|n| n.params["path"].as_str()).collect();
        assert!(
            paths.iter().any(|p| p.ends_with("pie_landuse.tif"))
                && paths.iter().any(|p| p.ends_with("sibuyan_landuse.tif"))
        );
    }
    // alt+up: the folder above, the folder come from chosen (enter: into it again); alt+left: back
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::ArrowUp);
    h.run_steps(2);
    assert!(h.query_all_by_label("inputs").count() > 0 && h.query_by_label("sibuyan_landuse.tif").is_none());
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(h.query_by_label("sibuyan_landuse.tif").is_some(), "into the folder chosen");
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::ArrowLeft);
    h.run_steps(2);
    assert!(h.query_by_label("sibuyan_landuse.tif").is_none(), "back to the folder above");
    // the operators' pane shown: at the left edge
    h.state_mut().focus(Pane::Operators);
    h.run_steps(2);
    assert!(left(&h, Pane::Operators) < left(&h, Pane::Model), "the operators at the left");
    assert!(h.query_by_label("Open a raster...").is_some(), "the operators in their pane");
    h.event(egui::Event::PointerMoved(egui::pos2(1.0, 300.0)));
    h.run_steps(3);
    assert!(!h.state().drawer, "no drawer over the operators' pane");
    shot(&mut h, "20-panes");
    // the operators' pane closed: the drawer
    {
        let dock = h.state_mut().dock.as_mut().unwrap();
        let path = dock.find_tab(&Pane::Operators).unwrap();
        dock.remove_tab(path);
    }
    h.event(egui::Event::PointerMoved(egui::pos2(700.0, 300.0)));
    h.run_steps(4);
    assert!(!h.state().is_open(Pane::Operators));
    assert!(h.query_by_label("Operators").is_some(), "the toolbar's menu");
    assert!(h.query_by_label("Open a raster...").is_none());
    // the pointer at the left edge: the drawer
    h.event(egui::Event::PointerMoved(egui::pos2(1.0, 300.0)));
    h.run_steps(3);
    assert!(h.state().drawer);
    assert!(h.query_by_label("Open a raster...").is_some(), "the operators in the drawer");
    shot(&mut h, "20-drawer");
    h.event(egui::Event::PointerMoved(egui::pos2(700.0, 300.0)));
    h.run_steps(3);
    assert!(!h.state().drawer, "away from it, it goes in");
    // backspace deletes
    let n = h.state().session.as_ref().unwrap().source.nodes.len();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = s.source.nodes.last().unwrap().id.clone();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), &id);
    }
    h.run_steps(2);
    h.key_press(egui::Key::Backspace);
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().source.nodes.len(), n - 1, "backspace deletes the node");
    // a floating window of the dock: a window of its own
    {
        let a = h.state_mut();
        let dock = a.dock.as_mut().unwrap();
        let path = dock.find_tab(&Pane::Viewer).unwrap();
        dock.remove_tab(path);
        dock.add_window(vec![Pane::Viewer]);
    }
    h.run_steps(3);
    assert_eq!(h.state().detached(), vec![Pane::Viewer]);
    assert!(h.state().dock.as_ref().unwrap().iter_surfaces_indexed().all(|(i, s)| i.is_main() || s.is_empty()));
}

/// Maps stacked in the viewer: a plot of the land use with a plot of roads over it (its under), another map over
/// it chosen in the viewer; B puts the satellite under it; the zoom stops out at the world; a cell clicked in a
/// stack shows its values along the layers.
#[test]
fn maps_stack_and_cells_show_their_series() {
    let d = tmp("stack");
    std::fs::create_dir_all(&d).unwrap();
    let (land, drivers) = (d.join("land.tif"), d.join("drivers.tif"));
    let srs = ["-a_srs".to_string(), "EPSG:32651".to_string()];
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &srs).unwrap();
    terraflux_gdal::translate(&data("sibuyan_drivers.tif"), &drivers, &srs).unwrap();
    let roads = d.join("roads.geojson");
    let v = terraflux_raster::vector::Vector {
        geometries: vec![terraflux_raster::vector::Geometry::LineString(vec![
            [437000.0, 1370000.0],
            [452000.0, 1375000.0],
            [467000.0, 1360000.0],
        ])],
        attributes: vec![],
        crs: Some("EPSG:32651".into()),
    };
    terraflux_gdal::write_vector(&roads, "roads", &v).unwrap();
    use terraflux_graph::Node;
    let mut m = terraflux_graph::Model::new("stack");
    m.nodes.push(Node::new("land", "raster.read").param("path", land.to_string_lossy()));
    m.nodes.push(Node::new("drivers", "raster.read").param("path", drivers.to_string_lossy()));
    m.nodes.push(Node::new("roads", "vector.read").param("path", roads.to_string_lossy()));
    m.nodes.push(Node::new("base", "view.plot").input("value", "land.raster"));
    m.nodes.push(Node::new("top", "view.plot").input("features", "roads.vector").input("under", "base.shown"));
    assert_eq!(
        terraflux_app::builder::plot_stack(&m, "top"),
        vec![
            terraflux_app::builder::PlotLayer::Map("land".into(), "raster".into(), 1.0),
            terraflux_app::builder::PlotLayer::Features("roads".into(), "vector".into()),
        ]
    );
    let path = d.join("stack.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("top", "shown").is_some()
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "top");
        s.preview_rasters.push(("drivers".into(), "raster".into(), 0.5));
    }
    h.state_mut().focus(Pane::Viewer);
    drawn(&mut h);
    assert!(h.query_by_label("MAPS OVER IT").is_some());
    shot(&mut h, "21-stacked");
    // B over the map: the satellite under it (once the map is drawn: its colours come from a thread)
    until(&mut h, "the map drawn", |a| a.session.as_ref().unwrap().preview_view.rect().is_some());
    let rect = h.state().session.as_ref().unwrap().preview_view.rect().unwrap();
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(2);
    h.key_press(egui::Key::B);
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.basemap, terraflux_app::map_view::Basemap::EsriImagery);
    // space over the map: the map hidden, and shown again
    h.key_press(egui::Key::Space);
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().preview_view.hide_data);
    h.key_press(egui::Key::Space);
    h.run_steps(2);
    assert!(!h.state().session.as_ref().unwrap().preview_view.hide_data);
    // the wheel out, far: the view stops at about the world
    for _ in 0..60 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, -400.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(1);
    }
    let zoom = h.state().session.as_ref().unwrap().preview_view.zoom();
    // 250 m cells: the world (40,075 km) is about 160,000 cells; the view about 900 points
    assert!(zoom > 1e-3 && zoom < 0.05, "{zoom}");
    // a cell of the drivers' stack clicked: its values along the layers
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_rasters.clear();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "drivers");
        s.preview_view.picked = Some((50, 60));
    }
    drawn(&mut h);
    assert!(h.query_by_label("ROW 50, COLUMN 60").is_some(), "the series' chart");
    shot(&mut h, "22-series");
    let Some(terraflux_ops::Value::Raster(r)) = h.state().session.as_ref().unwrap().value_of("drivers", "raster")
    else {
        panic!()
    };
    let maps: Vec<_> = (0..r.layers.len()).map(|l| terraflux_app::map_view::Source::Rasters(r.clone(), l)).collect();
    let values = terraflux_app::builder::series_at(&maps, 50, 60);
    assert_eq!(values.len(), r.layers.len());
    for (l, v) in values.iter().enumerate() {
        let whole = r.read(l).unwrap();
        let want = whole[50 * r.grid.width + 60];
        assert!(v.to_bits() == want.to_bits() || (v.is_nan() && want.is_nan()));
    }
}

/// The settings: a window from the toolbar, the theme changed at once (each drawn); the favourites: an operator
/// in the red collection shown in the operators' pane when the collection is.
#[test]
fn settings_themes_and_favourites() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(4);
    // (the toolbar's gear)
    h.get_by_label("⚙").click();
    h.run_steps(2);
    assert!(h.state().settings_open);
    assert!(h.query_by_label("Theme").is_some() && h.query_by_label("Threads").is_some());
    for (k, t) in terraflux_app::theme::THEMES.iter().enumerate() {
        terraflux_app::settings::update(|s| s.theme = t.name.to_string());
        h.run_steps(3);
        assert_eq!(terraflux_app::theme::current().name, t.name);
        let theme = if t.dark { egui::Theme::Dark } else { egui::Theme::Light };
        assert_eq!(h.ctx.style_of(theme).visuals.panel_fill, t.panel);
        shot(&mut h, &format!("23-theme-{k}"));
    }
    terraflux_app::settings::update(|s| *s = Default::default());
    h.state_mut().settings_open = false;
    // red: raster.read
    terraflux_app::settings::toggle("raster.read", 0);
    assert_eq!(terraflux_app::settings::collections_of("raster.read"), vec![0]);
    assert_eq!(terraflux_app::settings::in_collection(0), vec!["raster.read".to_string()]);
    h.state_mut().focus(Pane::Operators);
    h.ctx.data_mut(|d| d.insert_temp(egui::Id::new("collection shown"), Some(0u8)));
    h.run_steps(3);
    shot(&mut h, "24-favourites");
    assert!(h.query_by_label_contains("read").is_some(), "the red collection's operator");
    assert!(h.query_by_label_contains("reclassify").is_none(), "only the collection's");
    terraflux_app::settings::toggle("raster.read", 0);
    assert!(terraflux_app::settings::in_collection(0).is_empty());
}

/// ctrl+R selects the whole name (what is typed replaces it); the Metadata button shows what the rasters are;
/// the Operators menu cascades as a classic menu, an operator chosen added after the node selected.
#[test]
fn rename_metadata_and_the_operators_menu() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
    }
    until(&mut h, "the raster read", |a| a.session.as_ref().unwrap().value_of("raster_read", "raster").is_some());
    h.state_mut().focus(Pane::Model);
    h.run_steps(2);
    let canvas = h.state().session.as_ref().unwrap().editor.canvas().expect("the canvas");
    h.event(egui::Event::PointerMoved(canvas.center()));
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::R);
    h.run_steps(3);
    h.event(egui::Event::Text("land".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(
        s.source.nodes.iter().any(|n| n.id == "land"),
        "{:?}",
        s.source.nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
    // the metadata of the raster (run again under its new name)
    until(&mut h, "the raster read again", |a| a.session.as_ref().unwrap().value_of("land", "raster").is_some());
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    h.get_by_label("Metadata").click();
    h.run_steps(3);
    shot(&mut h, "25-metadata");
    assert!(h.query_by_label("coordinate system").is_some());
    assert!(h.query_by_label("128 x 108 cells (13824 cells)").is_some());
    shot(&mut h, "25-metadata");
    h.get_by_label("Metadata").click();
    h.run_steps(2);
    // Operators > Data preparation > reclassify
    h.get_by_label("Operators").click();
    h.run_steps(2);
    shot(&mut h, "26-operators-menu");
    // the menu's, over the operators' pane's
    h.query_all_by_label_contains("Data preparation").last().expect("the subject's submenu").click();
    h.run_steps(2);
    shot(&mut h, "26-operators-menu");
    h.query_all_by_label_contains("reclassify").last().expect("the submenu's operator").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.op == "raster.reclassify").expect("added");
    assert_eq!(n.inputs["map"], terraflux_graph::Source::Ref("land.raster".into()));
}

/// A series of land use maps selected: the Prepare pane suggests the transitions worth modelling and analyses
/// the changes in a click (a node per analysis).
#[test]
fn the_changes_are_analysed_from_the_prepare_pane() {
    let d = tmp("changes");
    std::fs::create_dir_all(&d).unwrap();
    let mut m = terraflux_graph::Model::new("changes");
    m.nodes.push(
        terraflux_graph::Node::new("land", "raster.read").param("path", data("pie_landuse.tif").to_string_lossy()),
    );
    let path = d.join("changes.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().focus(Pane::Prepare);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "land");
        s.start_to(Some("land".into()), Box::new(|| {}));
    }
    until(&mut h, "the run", |a| a.session.as_ref().unwrap().value_of("land", "raster").is_some());
    h.run_steps(3);
    assert!(h.query_all_by_label_contains("% of the change").count() >= 2, "the transitions suggested");
    shot(&mut h, "27-changes");
    h.get_by_label("Analyse the changes").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    for op in ["change.transitions", "change.suggest", "change.intensity", "change.components", "change.count"] {
        assert!(s.source.nodes.iter().any(|n| n.op == op), "{op}");
    }
}

/// The weights of evidence in the viewer: bars of a transition's variable, two ranges merged in a click (the
/// fit's ranges of that transition), the fit run again with one range less.
#[test]
fn weights_of_evidence_are_edited_in_the_viewer() {
    let d = tmp("woe");
    std::fs::create_dir_all(&d).unwrap();
    use terraflux_graph::Node;
    let mut m = terraflux_graph::Model::new("woe");
    let lu = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
    m.nodes.push(Node::new("t0", "raster.read").param("path", lu.clone()).param("band", "0"));
    m.nodes.push(Node::new("t1", "raster.read").param("path", lu).param("band", "1"));
    m.nodes.push(Node::new("drivers", "raster.read").param("path", data("sibuyan_drivers.tif").to_string_lossy()));
    m.nodes.push(
        Node::new("fit", "woe.fit")
            .input("t0", "t0.raster")
            .input("t1", "t1.raster")
            .input("drivers", "drivers.raster"),
    );
    let path = d.join("woe.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "fit");
        s.start_to(Some("fit".into()), Box::new(|| {}));
    }
    until(&mut h, "the fit", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("fit", "model").is_some()
    });
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    let ranges = |h: &Harness<TerrafluxApp>| {
        let Some(terraflux_ops::Value::Woe(w)) = h.state().session.as_ref().unwrap().value_of("fit", "model") else {
            panic!()
        };
        let (f, t) = w.transitions[0];
        let e = w.entries.iter().find(|e| e.from == f && e.to == t).unwrap().clone();
        (w.rows.iter().filter(|r| r.from == f && r.to == t && r.variable == e.variable).count(), e.layout.categorical)
    };
    let (before, categorical) = ranges(&h);
    assert!(!categorical && before > 1);
    shot(&mut h, "28-woe");
    h.query_all_by_label("merge next").next().expect("a range to merge").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.id == "fit").unwrap();
    assert!(n.params.get("ranges").is_some_and(|r| r.as_object().is_some_and(|o| !o.is_empty())), "{:?}", n.params);
    let t = Instant::now();
    while ranges(&h).0 == before {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "the fit again: too long");
    }
    assert_eq!(ranges(&h).0, before - 1);
}

/// The legend edited in the viewer: a class named, applied as a raster.legend node after the node shown, its
/// name in the legend then.
#[test]
fn the_legend_is_edited_in_the_viewer() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
    }
    until(&mut h, "the raster read", |a| a.session.as_ref().unwrap().value_of("raster_read", "raster").is_some());
    h.state_mut().focus(Pane::Viewer);
    drawn(&mut h);
    h.run_steps(5);
    h.get_by_label("Edit the legend").click();
    h.run_steps(3);
    assert!(h.query_by_label("4723").is_none(), "the editor in the legend's place");
    // the first class's name field (the first text field of the editor): "1" replaced by "forest"
    let field = h.query_all_by_value("1").last().expect("the name of class 1");
    field.click();
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("forest".into()));
    h.run_steps(2);
    shot(&mut h, "29-legend-editor");
    h.get_by_label("Apply").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.op == "raster.legend").expect("raster.legend added");
    assert_eq!(n.inputs["map"], terraflux_graph::Source::Ref("raster_read.raster".into()));
    let classes = terraflux_ops::legend::parse(&n.params["classes"]).unwrap();
    assert_eq!(classes[0].name, "forest");
    assert_eq!(s.selected.as_deref(), Some(n.id.as_str()));
    until(&mut h, "the legend's node run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_legend", "map").is_some()
    });
    drawn(&mut h);
    h.run_steps(5);
    assert!(h.query_by_label("forest").is_some(), "the name in the legend");
    shot(&mut h, "30-legend");
}

/// A table plotted (in a tab of its own, as the chart its settings ask) and written to a file.
#[test]
fn a_table_is_plotted_and_written() {
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let out = tmp("written-table.csv");
    let _ = std::fs::remove_file(&out);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes = vec![
            Node::new("areas", "table.of")
                .param("columns", serde_json::json!(["year", "forest", "pasture"]))
                .param("rows", serde_json::json!([[2000, 50.5, 20], [2010, 42, 31], [2020, 35.25, 40]])),
            Node::new("plot", "view.plot")
                .input("table", "areas.table")
                .param("chart", "lines")
                .param("y", "forest, pasture"),
            Node::new("write", "table.write")
                .input("table", "areas.table")
                .param("path", out.to_string_lossy().as_ref())
                .param("separator", ";")
                .param("decimal", ","),
        ];
        s.edited();
        assert!(s.issues.is_empty(), "{:?}", s.issues);
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the plot's tab", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.map_tabs.values().any(|t| t.node == "plot")
    });
    let tab = *h.state().session.as_ref().unwrap().map_tabs.iter().find(|(_, t)| t.node == "plot").unwrap().0;
    h.state_mut().session.as_mut().unwrap().raised.push(tab);
    drawn(&mut h);
    h.run_steps(4);
    shot(&mut h, "31-table-plot");
    // the chart the plot asks for: lines of forest and pasture along the year
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Table(t)) = s.value_of("plot", "shown") else { panic!("{:?}", s.error) };
    let chart = terraflux_app::builder::plot_chart(s.model.node("plot").unwrap(), &t);
    assert_eq!(chart.kind, terraflux_app::charts::Kind::Lines);
    assert_eq!((chart.columns.as_slice(), chart.x), ([1, 2].as_slice(), Some(0)));
    assert_eq!(
        std::fs::read_to_string(&out).unwrap(),
        "year;forest;pasture\n2000;50,5;20\n2010;42;31\n2020;35,25;40\n"
    );
}

#[test]
fn the_model_runs_many_times_from_its_pane() {
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    {
        // two steps a run, for the test
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(2))].into()).unwrap();
        s.edited();
    }
    h.state_mut().focus(Pane::Batch);
    let out = tmp("ui-batch");
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_dir_all(tmp("ui-batch-pool"));
    h.state_mut().computers.pool = None;
    h.state_mut().computers.dir = Some(tmp("ui-batch-pool"));
    {
        let b = &mut h.state_mut().batch;
        b.runs = 3;
        b.workers = 3;
        b.out = Some(out.join("replicates"));
    }
    h.run_steps(3);
    shot(&mut h, "batch-1-replicates");
    h.get_by_label("Run 3 times").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the replicates", |a| !a.batch.running());
    h.run_steps(2);
    {
        let b = &h.state().batch;
        assert!(b.error.is_none(), "{:?}", b.error);
        let f = b.finished.as_ref().unwrap();
        assert_eq!(f.rows.iter().map(|r| r.label.as_str()).collect::<Vec<_>>(), ["seed_1", "seed_2", "seed_3"]);
        assert!(f.rows.iter().all(|r| r.ok), "{:?}", f.errors);
        assert!(!f.files.is_empty());
        assert!(out.join("replicates").join("tasks.csv").exists());
        assert!(out.join("replicates").join("seed_2").join(&f.files[0]).exists());
    }
    shot(&mut h, "batch-2-done");
    // the runs as one series in the model
    let name = h.state().batch.finished.as_ref().unwrap().files.iter().find(|n| n.ends_with(".tif")).unwrap().clone();
    let before = h.state().session.as_ref().unwrap().source.nodes.len();
    h.get_by_label(&format!("Show the runs of {name} in the model")).scroll_to_me();
    h.run_steps(30);
    h.get_by_label(&format!("Show the runs of {name} in the model")).click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes.len(), before + 1);
    assert_eq!(s.source.nodes.last().unwrap().op, "raster.read");

    // a calibration of the patches' isometry against the map of 2011, the best settings put in the model
    {
        let b = &mut h.state_mut().batch;
        b.kind = terraflux_app::batch_view::Kind::Calibration;
        b.ranges = vec![terraflux_app::batch_view::Range {
            key: "patches.isometry".into(),
            from: "0.5".into(),
            to: "2.5".into(),
            points: "3".into(),
        }];
        b.observed = Some(data("sibuyan_landuse.tif"));
        b.bands = vec!["1997".into(), "2011".into()];
        b.band = 1;
        b.out = Some(out.join("calibration"));
    }
    h.run_steps(3);
    shot(&mut h, "batch-3-calibration");
    h.get_by_label("Run 3 times").click();
    h.run_steps(1);
    assert!(
        h.state().batch.running(),
        "{:?} loading {}",
        h.state().batch.error,
        h.state().session.as_ref().unwrap().loading()
    );
    until(&mut h, "the calibration", |a| !a.batch.running());
    h.run_steps(2);
    let b = &h.state().batch;
    assert!(b.error.is_none(), "{:?}", b.error);
    let f = b.finished.as_ref().unwrap();
    assert_eq!(f.kind, terraflux_app::batch_view::Kind::Calibration, "{:?} {:?}", f.errors, f.rows);
    let (ranked, keys, metric) = f.ranked.clone().unwrap();
    assert_eq!(
        (ranked.len(), keys.as_slice(), metric.as_str()),
        (3, ["patches.isometry".to_string()].as_slice(), "similarity:5")
    );
    assert!(ranked[0].score >= ranked[2].score);
    assert!(out.join("calibration").join("calibration.csv").exists());
    shot(&mut h, "batch-4-calibrated");
    h.get_by_label("Apply the best settings to the model").scroll_to_me();
    h.run_steps(30);
    h.get_by_label("Apply the best settings to the model").click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.node("patches").unwrap().params.get("isometry"), ranked[0].settings.get("patches.isometry"));
    assert!(s.dirty);
}

#[test]
fn a_batch_stopped_half_way_goes_on() {
    // a batch of three runs submitted to the application's pool, closed before any ran: its job kept in the
    // pool's folder
    let pool_dir = tmp("ui-resume-pool");
    let out = tmp("ui-resume-out");
    let _ = std::fs::remove_dir_all(&pool_dir);
    let _ = std::fs::remove_dir_all(&out);
    let path = model();
    let mut m = terraflux_graph::Model::load_expanded(&path).unwrap();
    m.apply(&[("simulation.times".to_string(), serde_json::json!(1))].into()).unwrap();
    let code = terraflux_dist::secure::Code::generate();
    let pool =
        terraflux_dist::pool::Pool::start(pool_dir.clone(), tmp("ui-resume-cache"), code.clone(), false, 0).unwrap();
    let place = pool.place();
    let job = tokio::runtime::Runtime::new().unwrap().block_on(async {
        let terraflux_dist::run::Place::At { url, code } = place else { unreachable!() };
        let mut c = terraflux_dist::client::Client::connect_with(&url, code.as_ref()).await.unwrap();
        let b = terraflux_dist::run::Batch {
            name: "resume".into(),
            model: m,
            base: path.parent().unwrap().to_path_buf(),
            tasks: terraflux_dist::batch::replicates(3, 1, &["$seed".to_string()]),
            files: true,
            evaluation: None,
        };
        terraflux_dist::run::submit(&mut c, &b, &mut |_| {}).await.unwrap()
    });
    drop(pool);
    let record = terraflux_app::batch_view::Record {
        kind: terraflux_app::batch_view::Kind::Replicates,
        model: "Sibuyan".into(),
        out: out.clone(),
        files: true,
        job: Some(job),
        on: None,
        total: 3,
        plan: None,
        alive: 0,
    };
    // as the application finds it when it starts (the record of the user's data, shared by the tests that run
    // at the same time, given here)
    let mut h = harness(new_app(Some(path)));
    h.state_mut().batch.interrupted = Some(record);
    // the pool of this test (another test may have shared the user's, in the settings the tests share)
    h.state_mut().computers.pool = None;
    h.state_mut().computers.dir = Some(pool_dir);
    loaded(&mut h);
    h.state_mut().focus(Pane::Batch);
    h.run_steps(3);
    shot(&mut h, "batch-5-interrupted");
    h.get_by_label("Go on").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the batch taken up again", |a| !a.batch.running());
    h.run_steps(2);
    let b = &h.state().batch;
    assert!(b.error.is_none(), "{:?}", b.error);
    let f = b.finished.as_ref().unwrap();
    assert_eq!(f.rows.len(), 3);
    for seed in ["seed_1", "seed_2", "seed_3"] {
        assert!(out.join(seed).join("maps.tif").exists(), "{seed}");
    }
    assert!(b.interrupted.is_none());
}

#[test]
fn a_computer_lends_itself_to_another_by_its_code() {
    // two applications: one shares its pool (on this computer's loopback, beacons on a port of the test's own),
    // the other is lent to it by its code
    let beacons = 46_000 + (std::process::id() % 1000) as u16;
    let loopback = std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST);
    // neither shared nor lent at first (whatever a run stopped half way left in the tests' settings)
    setup();
    terraflux_app::settings::update(|s| {
        s.share = false;
        s.lend = false;
        s.lend_code.clear();
    });
    let mut a = harness(new_app(None));
    a.state_mut().computers.pool = None;
    a.state_mut().computers.dir = Some(tmp("ui-share-pool"));
    a.state_mut().computers.network = Some((loopback, beacons));
    a.state_mut().focus(Pane::Computers);
    a.run_steps(2);
    a.get_by_label("Let other computers help").click();
    a.run_steps(3);
    let code = a.state().computers.pool.as_ref().expect("the pool started, shared").code().clone();
    assert!(a.state().computers.pool.as_ref().unwrap().shared());
    assert!(a.query_by_label(&code.to_string()).is_some(), "the code shown");
    shot(&mut a, "computers-1-shared");

    let mut b = new_app(None);
    b.computers.network = Some((loopback, beacons));
    let mut b = harness(b);
    b.state_mut().focus(Pane::Computers);
    b.run_steps(2);
    b.get_by_role_and_label(egui::accesskit::Role::TextInput, "Code").click();
    b.run_steps(1);
    b.event(egui::Event::Text(code.to_string().to_lowercase()));
    b.run_steps(2);
    b.get_by_label("Lend this computer").click();
    b.run_steps(2);
    let t = Instant::now();
    while b.state().computers.lender.as_ref().is_none_or(|l| l.status().coordinator.is_none()) {
        b.step();
        std::thread::sleep(Duration::from_millis(50));
        let e = b.state().computers.error.clone();
        let l = b.state().computers.lender.as_ref().map(|l| (l.code().to_string(), l.status()));
        let st = terraflux_app::settings::get();
        assert!(
            t.elapsed() < Duration::from_secs(60),
            "the lent computer found the other: {e:?} {l:?} pool {code} settings {} {}",
            st.lend,
            st.lend_code
        );
    }
    b.run_steps(2);
    shot(&mut b, "computers-2-lent");
    // the sharing computer sees it among its helpers
    let t = Instant::now();
    while a.state_mut().computers.helpers().is_empty() {
        a.step();
        std::thread::sleep(Duration::from_millis(50));
        assert!(t.elapsed() < Duration::from_secs(30), "the helper seen");
    }
    a.run_steps(3);
    shot(&mut a, "computers-3-helped");
    // stop lending, stop sharing (the settings of the tests back as they were)
    b.get_by_label("Stop lending").click();
    b.run_steps(2);
    assert!(b.state().computers.lender.is_none());
    a.get_by_label("Let other computers help").click();
    a.run_steps(2);
    assert!(!a.state().computers.pool.as_ref().unwrap().shared());
}

/// The viewer opens beside the model when the first data comes; at the closest zoom the cells' values are written
/// in them.
#[test]
fn the_viewer_comes_with_the_data_and_the_cells_show_their_values() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    assert!(!h.state().is_open(Pane::Viewer), "no viewer before there is data");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let p = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &terraflux_ops::Registry::standard(), "raster.read", &[("path", p.into())])
            .unwrap();
    }
    h.run_steps(2);
    assert_eq!(h.state_mut().focused(), Some(Pane::Viewer), "the viewer, in front");
    {
        let dock = h.state().dock.as_ref().unwrap();
        let (a, b) = (dock.find_tab(&Pane::Model).unwrap(), dock.find_tab(&Pane::Viewer).unwrap());
        assert_eq!((a.surface, a.node), (b.surface, b.node), "in a tab beside the model's");
    }
    drawn(&mut h);
    until(&mut h, "the map drawn", |a| a.session.as_ref().unwrap().preview_view.rect().is_some());
    let rect = h.state().session.as_ref().unwrap().preview_view.rect().unwrap();
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(2);
    for _ in 0..40 {
        h.event(egui::Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 400.0),
            modifiers: Default::default(),
            phase: egui::TouchPhase::Move,
        });
        h.run_steps(1);
    }
    let zoom = h.state().session.as_ref().unwrap().preview_view.zoom();
    assert_eq!(zoom, terraflux_app::map_view::MAX_ZOOM);
    drawn(&mut h);
    shot(&mut h, "22-cell-values");
}

/// The palette of commands (ctrl+K): a node found by its name and gone to, an operator added by its name; F1 shows
/// the shortcuts.
#[test]
fn the_commands_find_nodes_and_operators() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(4);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    h.run_steps(2);
    assert!(h.state().commands.is_some());
    h.event(egui::Event::Text("go to forest".into()));
    h.run_steps(2);
    shot(&mut h, "30-commands");
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    assert!(h.state().commands.is_none());
    assert_eq!(h.state().session.as_ref().unwrap().selected.as_deref(), Some("forest"));
    // an operator, after the node selected
    let before = h.state().session.as_ref().unwrap().source.nodes.len();
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::SHIFT, egui::Key::P);
    h.run_steps(2);
    h.event(egui::Event::Text("add raster.reclassify".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes.len(), before + 1);
    assert!(s.source.nodes.iter().any(|n| n.op == "raster.reclassify" && n.id != "forest"));
    // escape closes it, doing nothing
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::K);
    h.run_steps(2);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.state().commands.is_none());
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    assert!(h.state().shortcuts_open && h.query_by_label("Keyboard shortcuts").is_some());
    shot(&mut h, "31-shortcuts");
}
