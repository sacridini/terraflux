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

mod quality_of_life;
mod web_layers;

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

/// A Plot of the model opened in the Viewer (in front); what it shows first.
fn open_plot(h: &mut Harness<TerrafluxApp>, id: &str) -> (String, String) {
    let s = h.state_mut().session.as_mut().unwrap();
    terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), id);
    let shown = s.preview.clone().expect("the plot shows something");
    h.state_mut().focus(Pane::Viewer);
    shown
}

/// What a Plot of the model shows first, once run.
fn plotted(s: &terraflux_app::app::Session, id: &str) -> Option<terraflux_ops::Value> {
    let p = s.source.plot(id)?;
    let (n, q) = terraflux_app::builder::plot_figure(&s.source, &terraflux_ops::Registry::standard(), p)?;
    s.value_of(&n, &q)
}

/// An output shown in the Viewer, brought to the front (as a click on its node shows it).
fn show(h: &mut Harness<TerrafluxApp>, node: &str, port: &str) {
    let s = h.state_mut().session.as_mut().unwrap();
    terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), node);
    s.preview = Some((node.to_string(), port.to_string()));
    h.state_mut().focus(Pane::Viewer);
}

fn shot(h: &mut Harness<TerrafluxApp>, name: &str) {
    drawn(h);
    match h.render() {
        Ok(img) => img.save(tmp(&format!("ui-{name}.png"))).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
    }
}

/// The tests' own settings, not the user's; no computer shared or lent unless a test does it (on this computer
/// only). The tests look for the classic theme's labels and places ("Run", "Wizard...", the panes edge to edge):
/// their settings start in it (a new installation opens in Slate since 1.31.0).
fn setup() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    unsafe { std::env::set_var("TERRAFLUX_SETTINGS", tmp("settings.json")) };
    ONCE.call_once(|| {
        let _ = std::fs::remove_file(tmp("settings.json"));
        terraflux_app::settings::update(|st| st.theme = "Default".into());
    });
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
    // (and the numbers of the side panel worked out in threads, which make it longer when they come)
    let t = Instant::now();
    loop {
        let s = h.state_mut().session.as_mut();
        let busy = s.is_some_and(|s| s.regions.busy() || s.profiles.busy() || s.steps_areas.busy());
        if !busy {
            break;
        }
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "the side panel's numbers: too long");
    }
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
    assert!(h.query_by_label("Save as GIF...").is_some(), "the stack's maps as an animation");
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
    assert!(h.query_by_label("Save as GIF...").is_some(), "the steps as an animation");
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
        terraflux_app::builder::add(s, &terraflux_ops::Registry::standard(), "raster.calc", &[]).unwrap();
    }
    h.run_steps(3);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert_eq!(s.selected.as_deref(), Some("raster_calc"));
        let r = terraflux_ops::Registry::standard();
        terraflux_app::editor::connect(&mut s.source, &r, &[], "$t0", "raster_calc", "land").unwrap();
        terraflux_app::editor::set_param(&mut s.source, &[], "raster_calc", "expression", "land == 1").unwrap();
        s.edited();
        assert!(s.model.node("raster_calc").is_some());
        assert!(!s.issues.iter().any(|i| i.at.starts_with("raster_calc")), "{:?}", s.issues);
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
    // told inside: the canvas's corner and the bar say which container, and how to come out
    assert!(h.query_by_label_contains("Inside simulation, a loop").is_none(), "(painted, not a widget)");
    assert!(h.query_by_label("(a loop, each step)").is_some(), "the kind of the container in the bar");
    shot(&mut h, "9c-editor-body");
    h.get_by_label("‹ Back").click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.editor.path.is_empty(), "back up a level");
    assert_eq!(s.selected.as_deref(), Some("simulation"), "the container left, selected");
    // escape on the canvas, nothing selected, comes out too; the trail goes to any level
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["simulation".into()];
    h.run_steps(3);
    let canvas = h.state().session.as_ref().unwrap().editor.canvas().expect("the canvas");
    h.hover_at(canvas.center());
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.path.is_empty(), "escape: back up");
    h.state_mut().session.as_mut().unwrap().editor.path = vec!["simulation".into()];
    h.run_steps(3);
    h.get_by_label("Whole model").click();
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.path.is_empty());
    // File > Save
    h.get_by_label("File").click();
    h.run_steps(2);
    // (its label has its shortcut too; Save as is after it)
    h.query_all_by_label_contains("Save").next().expect("File's Save").click();
    h.run_steps(2);
    let saved = terraflux_graph::Model::load(&path).unwrap();
    assert_eq!(saved.node("raster_calc").unwrap().params["expression"], "land == 1");
    assert!(saved.node("raster_calc").unwrap().pos.is_some());
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
        // (its first band: the file has two, and MSPA would run on each)
        let read =
            terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        assert_eq!(read, "sibuyan_landuse");
        // forest (class 1) as the foreground, connected to the map read
        terraflux_app::builder::add(s, &r, "landscape.mspa", &[("foreground", 1.into())]).unwrap();
        let mspa = s.source.nodes.iter().find(|n| n.id == "landscape_mspa").unwrap();
        assert_eq!(
            mspa.inputs["map"],
            terraflux_graph::Source::Ref("sibuyan_landuse.raster".into()),
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
        assert_eq!(s.status.get("sibuyan_landuse"), Some(&NodeStatus::Done { cached: true }));
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
    let p = m.node("sibuyan_landuse").unwrap().params["path"].as_str().unwrap().to_string();
    assert!(!std::path::Path::new(&p).is_absolute() && p.ends_with("sibuyan_landuse.tif"), "{p}");
    let again = terraflux_graph::Model::load_expanded(&model).unwrap();
    let r = terraflux_exec::Engine::default().run(&again, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
    assert!(r.get("landscape_mspa.map").is_some(), "the saved model runs from the command line's engine too");
}

/// The setting "write the model's Python script beside it": off, nothing is written; on, `<model>.py` appears.
#[test]
fn python_script_beside_the_model() {
    let r = terraflux_ops::Registry::standard();
    let model = tmp("ui-python-beside.toml");
    let py = model.with_extension("py");
    let _ = std::fs::remove_file(&py);
    let mut s = terraflux_app::app::Session::blank();
    s.save_as(&model).unwrap();
    terraflux_app::settings::update(|st| st.python_beside = false);
    terraflux_app::app::python_beside(&mut s, &r);
    assert!(!py.exists(), "off: no script");
    terraflux_app::settings::update(|st| st.python_beside = true);
    terraflux_app::app::python_beside(&mut s, &r);
    terraflux_app::settings::update(|st| st.python_beside = false);
    assert!(py.exists(), "on: the script is written beside the model");
    assert!(s.editor.message.is_none(), "{:?}", s.editor.message);
}

/// Read, reclassify, keep it as a Plot from the Viewer's Plots, nothing written; saved (a file, then a write node in
/// the model too); a rule changed, the Viewer shows the new map.
#[test]
fn a_map_kept_as_a_plot_without_writing_and_saved() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        // (its first band: the file has two, and the Viewer would show a stack)
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 0".into())]).unwrap();
        terraflux_app::builder::select(s, &r, "raster_reclassify");
    }
    until(&mut h, "the map", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_reclassify", "map").is_some()
    });
    h.state_mut().focus(Pane::Viewer);
    drawn(&mut h);
    // kept as a Plot: the Viewer's Plots, Save as a new plot
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let forest = {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.plots.len(), 1);
        let p = &s.source.plots[0];
        assert_eq!(p.id, "plot_raster_reclassify");
        assert_eq!(p.layers, [terraflux_graph::PlotLayer::new("raster_reclassify.map")]);
        assert_eq!(s.plot_open.as_deref(), Some("plot_raster_reclassify"));
        let Some(terraflux_ops::Value::Map(m)) = plotted(s, "plot_raster_reclassify") else { panic!("{:?}", s.error) };
        assert!(m.data.iter().all(|v| v.is_nan() || *v == 0.0 || *v == 1.0));
        m
    };
    assert!(h.query_by_label("Plot: plot_raster_reclassify").is_some(), "the Plot open named in the Viewer");
    shot(&mut h, "14-plot");
    // saved from the Viewer: the same cells in the file
    let out = tmp("ui-forest.tif");
    let _ = std::fs::remove_file(&out);
    h.state_mut().session.as_mut().unwrap().save_output("raster_reclassify", "map", &out);
    until(&mut h, "the file", |a| !a.session.as_ref().unwrap().saving());
    let saved = terraflux_raster::Raster::read(&out).unwrap();
    let back = saved.bands[0].masked(saved.nodata);
    assert!(back.iter().zip(forest.data.iter()).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
    // and a write node in the model, that writes it at every run
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = terraflux_app::builder::add_write(s, &r, "raster_reclassify", "map", &out).unwrap();
        let w = s.source.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(w.inputs["value"], terraflux_graph::Source::Ref("raster_reclassify.map".into()));
    }
    // another rule: the Viewer shows the new map
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = Some(("raster_reclassify".into(), "map".into()));
        terraflux_app::editor::set_param(&mut s.source, &[], "raster_reclassify", "rules", "1 2 -> 1\n* -> 0").unwrap();
        s.edited();
    }
    h.run_steps(1);
    let ones = |m: &terraflux_ops::Map| m.data.iter().filter(|v| **v == 1.0).count();
    // (the map of the run before stays until the new run is done: waited for by its cells, not by the status)
    until(&mut h, "class 2 joined the ones", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && matches!(s.value_of("raster_reclassify", "map"), Some(terraflux_ops::Value::Map(m)) if ones(&m) > ones(&forest))
    });
    // the node renamed: the Plot follows it
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::editor::rename_node(&mut s.source, &[], "raster_reclassify", "forest").unwrap();
        s.edited();
        assert_eq!(s.source.plots[0].layers[0].from, "forest.map");
    }
}

/// Two maps compared in the viewer: the land use read and the forest reclassified from it, by a curtain, side by
/// side (the two views moving together) and by their difference (the transitions).
#[test]
fn measuring_on_the_map_by_clicks() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        s.preview = Some(("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("sibuyan_landuse", "raster").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    assert!(h.state().session.as_ref().unwrap().preview_view.measured().is_none());
    // by the API: the points of the cells, and the numbers they tell
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        v.set_measuring(true);
        assert!(v.measuring.is_some());
        v.measure_click([0.0, 0.0]);
        v.measure_click([30.0, 0.0]);
        v.measure_click([30.0, 40.0]);
    }
    h.run_steps(2);
    shot(&mut h, "14d-measure");
    let r = h.state().session.as_ref().unwrap().preview_view.measured().expect("measured");
    assert_eq!(r.segments.len(), 2);
    assert!((r.cells_length - 70.0).abs() < 1e-9, "{}", r.cells_length);
    assert_eq!(r.cells_area, Some(600.0));
    assert!(r.area.is_some() && r.perimeter.is_some() && r.text().contains("Area:"), "{}", r.text());
    // drawing and measuring exclude each other
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        v.drawing = Some(terraflux_app::draw::Sketch::new(terraflux_app::draw::Kind::Line));
        v.set_measuring(true);
        assert!(v.measuring.is_none(), "not while drawing");
        v.drawing = None;
        v.set_measuring(true);
    }
    // by the mouse: clicks add points (no cell picked), a right click takes the last away, esc ends
    h.run_steps(2);
    let (rect, zoom) = {
        let v = &h.state().session.as_ref().unwrap().preview_view;
        (v.rect().expect("drawn"), v.zoom())
    };
    let click = |h: &mut Harness<TerrafluxApp>, p: egui::Pos2, button: egui::PointerButton| {
        h.event(egui::Event::PointerMoved(p));
        h.run_steps(1);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: p, button, pressed, modifiers: egui::Modifiers::NONE });
        }
        h.run_steps(2);
    };
    let c = rect.center();
    let a = c + egui::Vec2::new(-60.0, -20.0);
    let b = c + egui::Vec2::new(60.0, -20.0);
    click(&mut h, a, egui::PointerButton::Primary);
    click(&mut h, b, egui::PointerButton::Primary);
    {
        let v = &h.state().session.as_ref().unwrap().preview_view;
        assert!(v.picked.is_none(), "no cell picked while measuring");
        let r = v.measured().expect("two points");
        assert_eq!(r.segments.len(), 1);
        assert!((r.cells_length - f64::from(120.0 / zoom)).abs() < 0.05, "{} cells at zoom {zoom}", r.cells_length);
    }
    click(&mut h, c, egui::PointerButton::Secondary);
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.measured().unwrap().segments.len(), 0);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().preview_view.measuring.is_none(), "esc ends measuring");
    // M over the map turns it on again
    h.event(egui::Event::PointerMoved(c));
    h.run_steps(1);
    h.key_press(egui::Key::M);
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().preview_view.measuring.is_some(), "M turns measuring on");
}

/// The test data given a place in degrees by GDAL (`gdal_translate -a_srs EPSG:4326 -a_ullr 122.30 12.60 122.62
/// 12.33`: 128 x 108 cells of 0.0025 degrees) in a file of its own.
fn in_degrees(name: &str) -> PathBuf {
    let out = tmp(name);
    let args = ["-a_srs", "EPSG:4326", "-a_ullr", "122.30", "12.60", "122.62", "12.33"];
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &out, &args.map(String::from)).unwrap();
    out
}

/// A real file in degrees, read and measured: the Viewer's numbers against references of other tools (not from this
/// code). The corners of the cells A = (10, 20), B = (110, 90), C = (10, 90) are (122.325 E, 12.55 N), (122.575 E,
/// 12.375 N), (122.325 E, 12.375 N). pyproj 3.7.2 `Geod(ellps="WGS84")` (GeographicLib's exact geodesics) gives
/// A-B 33368.4751 m; for the rectangle A, (110, 20), B, C the area 526,161,498.514 m2 (the same from GDAL,
/// `ogrinfo -dialect SQLite -sql "SELECT ST_Area(GeomFromText(wkt, 4326), 1)"`: 526161498.514046) and the perimeter
/// 93,075.771 m. The viewer's area is on the authalic sphere with great-circle edges: its difference from the
/// ellipsoid's (here under a millionth, `measure.rs` checks it) is far below the 0.5% allowed.
#[test]
fn measuring_a_file_in_degrees_matches_the_references() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let file = in_degrees("measure_degrees.tif");
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = file.to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        s.preview = Some(("measure_degrees".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("measure_degrees", "raster").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        v.set_measuring(true);
        v.measure_click([10.0, 20.0]);
        v.measure_click([110.0, 90.0]);
    }
    h.run_steps(2);
    let m = h.state().session.as_ref().unwrap().preview_view.measured().expect("measured");
    assert_eq!(m.frame, terraflux_app::measure::Frame::Geographic);
    assert!((m.length - 33_368.475_1).abs() / 33_368.475_1 < 0.001, "A-B {} m", m.length);
    assert!(m.text().contains("33.368 km"), "{}", m.text());
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        v.measuring.as_mut().unwrap().points.clear();
        for c in [[10.0, 20.0], [110.0, 20.0], [110.0, 90.0], [10.0, 90.0]] {
            v.measure_click(c);
        }
    }
    h.run_steps(2);
    let m = h.state().session.as_ref().unwrap().preview_view.measured().expect("measured");
    let (area, perimeter) = (m.area.expect("an area"), m.perimeter.expect("a perimeter"));
    let off = (area - 526_161_498.514) / 526_161_498.514;
    assert!(off.abs() < 0.005, "the area {area} m2: {:.4}% from the ellipsoid's", off * 100.0);
    assert!((perimeter - 93_075.771).abs() / 93_075.771 < 0.001, "the perimeter {perimeter} m");
    assert!(m.text().contains("526.16"), "{}", m.text());
    // by the mouse: two clicks; the line against a haversine on a sphere (a mean radius: within 0.5% of the ellipsoid)
    h.state_mut().session.as_mut().unwrap().preview_view.measuring.as_mut().unwrap().points.clear();
    h.run_steps(2);
    let rect = h.state().session.as_ref().unwrap().preview_view.rect().expect("drawn");
    let click = |h: &mut Harness<TerrafluxApp>, p: egui::Pos2| {
        h.event(egui::Event::PointerMoved(p));
        h.run_steps(1);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: p,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
        }
        h.run_steps(2);
    };
    click(&mut h, rect.center() + egui::vec2(-80.0, -50.0));
    click(&mut h, rect.center() + egui::vec2(90.0, 40.0));
    let v = &h.state().session.as_ref().unwrap().preview_view;
    let pts = v.measuring.as_ref().unwrap().points.clone();
    assert_eq!(pts.len(), 2);
    let ll = |c: [f64; 2]| ((122.30 + 0.0025 * c[0]).to_radians(), (12.60 - 0.0025 * c[1]).to_radians());
    let ((l1, p1), (l2, p2)) = (ll(pts[0]), ll(pts[1]));
    let hav = ((p2 - p1) / 2.0).sin().powi(2) + p1.cos() * p2.cos() * ((l2 - l1) / 2.0).sin().powi(2);
    let sphere = 6_371_008.8 * 2.0 * hav.sqrt().asin();
    let got = v.measured().unwrap().length;
    assert!((got - sphere).abs() / sphere < 0.005, "clicked {got} m against {sphere} m on a sphere");
}

/// The line measured on the map shown is drawn in the view beside it too, carried to its own grid when that is of
/// another coordinate system (here the map in UTM 51N measured, the one in WGS 84 beside it): the line's colour is
/// in both views, at the same place.
#[test]
fn the_measure_line_follows_into_the_views_side_by_side() {
    use terraflux_app::builder::{CompareWith, How};
    use terraflux_graph::Node;
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let d = tmp("measure_side_by_side");
    std::fs::create_dir_all(&d).unwrap();
    let (land, geo) = (d.join("land.tif"), d.join("land_geo.tif"));
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &["-a_srs".to_string(), "EPSG:32651".to_string()])
        .unwrap();
    terraflux_gdal::warp(&[&land], &geo, &["-t_srs".into(), "EPSG:4326".into(), "-r".into(), "near".into()]).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = |id: &str, p: &std::path::Path| {
            Node::new(id, "raster.read").param("path", p.to_string_lossy().as_ref()).param("band", "0")
        };
        s.source.nodes = vec![read("land", &land), read("geo", &geo)];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "geo"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = Some(("land".into(), "raster".into()));
        s.preview_compare.with = Some(CompareWith::Output("geo".into(), "raster".into()));
        s.preview_compare.how = How::SideBySide;
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "both views drawn", |a| {
        let s = a.session.as_ref().unwrap();
        s.preview_view.shown_grid().is_some() && s.preview_compare.view.shown_grid().is_some()
    });
    drawn(&mut h);
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        v.set_measuring(true);
        // the cells (30, 20) to (100, 80) of the map in UTM
        v.measure_click([30.0, 20.0]);
        v.measure_click([100.0, 80.0]);
    }
    h.run_steps(3);
    // the pixels of the line's colour (0, 220, 255) in a view's rectangle
    let ink = |h: &mut Harness<TerrafluxApp>, rect: egui::Rect| {
        let img = h.render().expect("the window drawn by wgpu");
        let mut n = 0;
        for y in rect.top() as u32..rect.bottom() as u32 {
            for x in rect.left() as u32..rect.right() as u32 {
                let p = img.get_pixel(x, y).0;
                if p[0] < 40 && p[1] > 200 && p[2] > 230 {
                    n += 1;
                }
            }
        }
        n
    };
    let (shown, beside) = {
        let s = h.state().session.as_ref().unwrap();
        (s.preview_view.rect().unwrap(), s.preview_compare.view.rect().unwrap())
    };
    let (a, b) = (ink(&mut h, shown), ink(&mut h, beside));
    assert!(a > 100, "the line in the view measured on: {a} pixels");
    assert!(b > 100, "and in the view beside, of another coordinate system: {b} pixels");
    // the vertex of the first point lands on the same ground: the view beside's centre is the shown one's, carried
    let s = h.state().session.as_ref().unwrap();
    let link = terraflux_app::reproject::Link::between(
        s.preview_compare.view.shown_grid().unwrap(),
        s.preview_view.shown_grid().unwrap(),
    )
    .unwrap();
    let (zoom, centre) = s.preview_compare.view.camera();
    let at = beside.center() + (link.to_own(egui::vec2(30.0, 20.0)) - centre) * zoom;
    let img = h.render().expect("the window drawn by wgpu");
    // (the vertex: a white disc with a black ring, at that screen point)
    let p = img.get_pixel(at.x as u32, at.y as u32).0;
    assert!(p[0] > 200 && p[1] > 200 && p[2] > 200, "the first vertex at {at:?} in the view beside: {p:?}");
    shot(&mut h, "14d-measure-side-by-side");
}

/// Measuring in views side by side: every view measures on its own (a click in the one beside makes its line there,
/// in its own grid), the line of one followed into the others, and the box of numbers only in the view worked on
/// last; Esc over any view stops all of them.
#[test]
fn measuring_in_the_views_side_by_side_each_with_its_own_line() {
    use terraflux_app::builder::{CompareWith, How};
    use terraflux_graph::Node;
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let d = tmp("measure_each_view");
    std::fs::create_dir_all(&d).unwrap();
    let (land, geo) = (d.join("land.tif"), d.join("land_geo.tif"));
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &["-a_srs".to_string(), "EPSG:32651".to_string()])
        .unwrap();
    terraflux_gdal::warp(&[&land], &geo, &["-t_srs".into(), "EPSG:4326".into(), "-r".into(), "near".into()]).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = |id: &str, p: &std::path::Path| {
            Node::new(id, "raster.read").param("path", p.to_string_lossy().as_ref()).param("band", "0")
        };
        s.source.nodes = vec![read("land", &land), read("geo", &geo)];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "geo"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = Some(("land".into(), "raster".into()));
        s.preview_compare.with = Some(CompareWith::Output("geo".into(), "raster".into()));
        s.preview_compare.how = How::SideBySide;
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "both views drawn", |a| {
        let s = a.session.as_ref().unwrap();
        s.preview_view.shown_grid().is_some() && s.preview_compare.view.shown_grid().is_some()
    });
    drawn(&mut h);
    // measuring on in the map shown: the view beside measures too, and only the first has the box
    h.state_mut().session.as_mut().unwrap().preview_view.set_measuring(true);
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.preview_compare.view.measuring.is_some(), "the view beside measures with it");
        assert!(!s.preview_view.hide_measure_box && s.preview_compare.view.hide_measure_box);
    }
    assert_eq!(h.query_all_by_label("click the map to add points").count(), 1, "one box, in the map shown");
    // a line in the map shown (cells of the map in UTM): its numbers in its box, the line in the view beside
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        v.measure_click([30.0, 20.0]);
        v.measure_click([100.0, 80.0]);
    }
    h.run_steps(3);
    assert_eq!(h.query_all_by_label_contains("Length:").count(), 1);
    // a click in the view beside: a line of its own, in its own grid (cells of the map in WGS 84)
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_compare.view;
        v.measure_click([20.0, 15.0]);
        v.measure_click([60.0, 50.0]);
    }
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        let (a, b) = (s.preview_view.measuring.as_ref().unwrap(), s.preview_compare.view.measuring.as_ref().unwrap());
        assert_eq!((a.points.len(), b.points.len()), (2, 2), "each view its own line");
        assert_eq!(b.points, [[20.0, 15.0], [60.0, 50.0]]);
        // (the one worked on last has the box)
        assert!(s.preview_view.hide_measure_box && !s.preview_compare.view.hide_measure_box);
        let (ra, rb) = (s.preview_view.measured().unwrap(), s.preview_compare.view.measured().unwrap());
        assert!(
            ra.length > 0.0 && rb.length > 0.0 && (ra.length - rb.length).abs() > 1.0,
            "{} {}",
            ra.length,
            rb.length
        );
    }
    assert_eq!(h.query_all_by_label_contains("Length:").count(), 1, "the box only in the view worked on");
    // both lines are drawn in both views (the line's colour, in each view's rectangle)
    let ink = |h: &mut Harness<TerrafluxApp>, rect: egui::Rect| {
        let img = h.render().expect("the window drawn by wgpu");
        let mut n = 0;
        for y in rect.top() as u32..rect.bottom() as u32 {
            for x in rect.left() as u32..rect.right() as u32 {
                let p = img.get_pixel(x, y).0;
                if p[0] < 40 && p[1] > 200 && p[2] > 230 {
                    n += 1;
                }
            }
        }
        n
    };
    let (shown, beside) = {
        let s = h.state().session.as_ref().unwrap();
        (s.preview_view.rect().unwrap(), s.preview_compare.view.rect().unwrap())
    };
    let (a, b) = (ink(&mut h, shown), ink(&mut h, beside));
    assert!(a > 150 && b > 150, "the lines in both views: {a} {b} pixels");
    // Esc over a view stops them all
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_compare.view;
        v.measure_stopped = true;
    }
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.preview_view.measuring.is_none() && s.preview_compare.view.measuring.is_none());
    shot(&mut h, "14e-measure-each-view");
}

#[test]
fn maps_compare_by_a_curtain_and_side_by_side() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 0".into())]).unwrap();
        s.preview = Some(("raster_reclassify".into(), "map".into()));
        s.preview_compare.with =
            Some(terraflux_app::builder::CompareWith::Output("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("raster_reclassify", "map").is_some()
            && s.value_of("sibuyan_landuse", "raster").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.preview_compare.with.is_some(), "the map compared kept");
    shot(&mut h, "14b-compare-curtain");
    // side by side: the second view drawn, at the first one's place
    h.state_mut().session.as_mut().unwrap().preview_compare.how = terraflux_app::builder::How::SideBySide;
    h.run_steps(4);
    drawn(&mut h);
    shot(&mut h, "14c-compare-side-by-side");
    let s = h.state().session.as_ref().unwrap();
    let (a, b) = (s.preview_view.camera(), s.preview_compare.view.camera());
    assert!(s.preview_compare.view.rect().is_some(), "the view beside drawn");
    assert!((a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).length() < 1e-3, "the same grid, the same view: {a:?} {b:?}");
    // four views: two more, in two rows, all moving with the one moved
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = || terraflux_app::builder::CompareWith::Output("sibuyan_landuse".into(), "raster".into());
        s.preview_compare.more.push(terraflux_app::builder::Beside::new(read()));
        s.preview_compare.more.push(terraflux_app::builder::Beside::new(read()));
    }
    h.run_steps(4);
    drawn(&mut h);
    shot(&mut h, "14c1-compare-four-views");
    {
        let s = h.state().session.as_ref().unwrap();
        let rect = |v: &terraflux_app::map_view::MapView| v.rect().expect("drawn");
        let (one, two) = (rect(&s.preview_view), rect(&s.preview_compare.view));
        let (three, four) = (rect(&s.preview_compare.more[0].view), rect(&s.preview_compare.more[1].view));
        assert!(one.left() < two.left() && (one.top() - two.top()).abs() < 1.0, "{one:?} {two:?}");
        assert!(three.top() > one.bottom() && four.top() > two.bottom(), "{three:?} {four:?}");
        assert!((three.left() - one.left()).abs() < 1.0 && (four.left() - two.left()).abs() < 1.0);
    }
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let (zoom, center) = s.preview_compare.view.camera();
        s.preview_compare.view.set_camera(zoom * 2.0, center + egui::Vec2::new(5.0, 3.0));
    }
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        let moved = s.preview_compare.view.camera();
        for v in [&s.preview_view, &s.preview_compare.more[0].view, &s.preview_compare.more[1].view] {
            let c = v.camera();
            assert!((c.0 - moved.0).abs() < 1e-4 && (c.1 - moved.1).length() < 1e-3, "{c:?} {moved:?}");
        }
    }
    h.state_mut().session.as_mut().unwrap().preview_compare.more.clear();
    // their difference: both maps of classes, the transitions from the forest map to the land use, named
    h.state_mut().session.as_mut().unwrap().preview_compare.how = terraflux_app::builder::How::Difference;
    until(&mut h, "the difference", |a| a.session.as_ref().unwrap().preview_compare.difference.made.is_some());
    h.run_steps(4);
    drawn(&mut h);
    shot(&mut h, "14c2-compare-difference");
    let s = h.state().session.as_ref().unwrap();
    let d = s.preview_compare.difference.made.as_ref().unwrap().as_ref().expect("the difference made");
    assert_eq!(d.step, 1);
    assert_eq!(d.names[0].1, "unchanged");
    assert!(d.names.len() > 1 && d.names[1..].iter().all(|(_, n)| n.contains(" -> ")), "{:?}", d.names);
    assert!(d.summary.contains("cells changed class"), "{}", d.summary);
    let (a, b) = (s.preview_view.camera(), s.preview_compare.view.camera());
    assert!((a.0 - b.0).abs() < 1e-4 && (a.1 - b.1).length() < 1e-3, "drawn where the map is: {a:?} {b:?}");
    // the difference kept as a layer: over the map shown, the comparison let go
    // (a taller window: the side panel, with the choice of the compared map's band, is longer than the default one)
    h.set_size(egui::vec2(1280.0, 1100.0));
    h.run_steps(2);
    h.get_by_label("📌 Keep as a layer").scroll_to_me();
    h.run_steps(30);
    h.get_by_label("📌 Keep as a layer").click();
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.preview_compare.with.is_none(), "the comparison let go");
        assert_eq!(s.preview_layers.len(), 1);
        let l = &s.preview_layers[0];
        assert_eq!((l.node.as_str(), l.port.as_str()), ("sibuyan_landuse", "raster"));
        assert_eq!(l.minus, Some(("raster_reclassify".to_string(), "map".to_string())));
    }
    until(&mut h, "the difference layer", |a| {
        let s = a.session.as_ref().unwrap();
        s.layer_differences.values().any(|d| matches!(d.made, Some(Ok(_))))
    });
    h.run_steps(4);
    drawn(&mut h);
    shot(&mut h, "14c3-difference-layer");
    {
        let s = h.state().session.as_ref().unwrap();
        let d = s.layer_differences.values().find_map(|d| d.made.clone()).unwrap().unwrap();
        assert_eq!(s.preview_layers[0].name(Some(&d)), "raster_reclassify.map to sibuyan_landuse");
    }
    // in a Plot: opened again as a difference
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_layers.clear();
        s.source.plots.push(terraflux_graph::Plot {
            id: "changed".into(),
            layers: vec![
                terraflux_graph::PlotLayer::new("raster_reclassify.map"),
                terraflux_graph::PlotLayer {
                    minus: Some("raster_reclassify.map".into()),
                    ..terraflux_graph::PlotLayer::new("sibuyan_landuse.raster")
                },
            ],
            ..Default::default()
        });
        s.edited();
        s.plot_to_open = Some("changed".into());
    }
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        let minus: Vec<_> = s.preview_layers.iter().map(|l| l.minus.clone()).collect();
        assert_eq!(minus, [None, Some(("raster_reclassify".to_string(), "map".to_string()))]);
    }
    // a Plot that compares: opened with its comparison, as it asks
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_compare.with = None;
        s.preview_compare.how = terraflux_app::builder::How::Curtain;
        s.source.plots.push(terraflux_graph::Plot {
            id: "compared".into(),
            layers: vec![terraflux_graph::PlotLayer::new("raster_reclassify.map")],
            compare: Some(terraflux_graph::PlotCompare {
                with: "sibuyan_landuse.raster".into(),
                how: Some("side by side".into()),
                also: vec!["sibuyan_landuse.raster".into()],
                ..Default::default()
            }),
            ..Default::default()
        });
        s.edited();
        s.plot_to_open = Some("compared".into());
    }
    until(&mut h, "the plot", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.preview_compare.with.is_some()
    });
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(
        s.preview_compare.with,
        Some(terraflux_app::builder::CompareWith::Output("sibuyan_landuse".into(), "raster".into()))
    );
    assert_eq!(s.preview_compare.how, terraflux_app::builder::How::SideBySide);
    // with its view more
    assert_eq!(s.preview_compare.more.len(), 1);
    assert_eq!(
        s.preview_compare.more[0].with,
        Some(terraflux_app::builder::CompareWith::Output("sibuyan_landuse".into(), "raster".into()))
    );
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
    h.run_steps(2);
    h.state_mut().focus(Pane::Viewer);
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

/// A template opens in the builder and runs by itself: its plot in the Viewer in MSPA's colours. A node's map asked for
/// twice is one tab, brought to the front. "/" searches the operators: enter adds the one chosen after the node
/// selected, escape closes the search.
#[test]
fn templates_run_and_the_keyboard_adds_operators() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    // (the viewer opens behind the graph)
    h.state_mut().focus(Pane::Viewer);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.plot_open.as_deref(), Some("plot"), "its Plot open");
        assert_eq!(s.preview, Some(("mspa".to_string(), "map".to_string())), "the map it shows");
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && plotted(s, "plot").is_some()
    });
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(m)) = s.value_of("mspa", "map") else { panic!("{:?}", s.error) };
        assert!(m.data.contains(&17.0) && m.data.contains(&3.0), "core and edge");
    }
    until(&mut h, "the legend", |a| {
        let s = a.session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(m)) = plotted(s, "plot") else { return false };
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

/// The model read as a document: its stages, the steps with their notes and settings in words, the loop's body
/// within it, the plot as a figure; a step's name selects its node, a setting edited in place runs the model again.
#[test]
fn the_model_reads_as_a_document() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "CLUE-S").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("simulation", "maps").is_some()
    });
    h.state_mut().focus(Pane::Document);
    h.run_steps(4);
    for label in ["1. Data", "2. Preparation", "3. Model", "Figure 1.", "Repeated", "3.4.1"] {
        assert!(h.query_by_label_contains(label).is_some(), "{label}");
    }
    assert!(h.query_by_label_contains("the range of the autocorrelation, in cells").is_some(), "settings in words");
    shot(&mut h, "40-document");
    h.get_all_by_label("regression").next().unwrap().click_accesskit();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.selected.as_deref(), Some("regression"), "a step's name selects its node");
    assert!(s.editor.path.is_empty());
    // a setting changed in place: an edit of the model (undone by ctrl+Z), the model run again
    assert!(!s.history.can_undo(), "a template just opened");
    fn range<'a>(h: &'a Harness<'static, TerrafluxApp>) -> egui_kittest::Node<'a> {
        use egui_kittest::kittest::NodeT;
        let mut fields =
            h.get_all_by_value("25").filter(|n| n.accesskit_node().role() == egui::accesskit::Role::TextInput);
        let f = fields.next().expect("the range's field");
        assert!(fields.next().is_none());
        f
    }
    range(&h).scroll_to_me();
    h.run_steps(2);
    let field = range(&h);
    field.focus();
    field.type_text("0");
    h.run_steps(1);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let aptitude = s.source.nodes.iter().find(|n| n.id == "aptitude").unwrap();
    assert_eq!(aptitude.params["range"], 250, "{:?}", aptitude.params);
    assert!(s.history.can_undo());
    until(&mut h, "the run again", |a| !a.session.as_ref().unwrap().running());
}

/// The checks (check.assert) after a run: one passed, one only warned, one turned off; their badges' verdicts, the
/// model's health at the top of the document and in the status bar, each check's step with its badge.
#[test]
fn checks_show_how_they_fared() {
    use terraflux_app::checks::{self, Verdict};
    use terraflux_graph::{Model, Node};
    let mut m = Model::new("checks");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 30).param("columns", 40).param("seed", 1));
    m.nodes.push(Node::new("whole", "check.assert").param("expression", "land >= 0").input("land", "land.map"));
    m.nodes.push(
        Node::new("half", "check.assert")
            .param("expression", "land > 0.5")
            .param("message", "not all of it is above a half")
            .param("warn", true)
            .input("land", "land.map"),
    );
    let mut off = Node::new("off", "check.assert").param("expression", "land < 0").input("land", "land.map");
    off.bypass = true;
    m.nodes.push(off);
    let path = tmp("checks.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    {
        let s = h.state().session.as_ref().unwrap();
        let before: Vec<Verdict> = checks::of(s).into_iter().map(|c| c.verdict).collect();
        assert_eq!(before, [Verdict::NotRun, Verdict::NotRun, Verdict::Off], "before a run");
    }
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("whole", "ok").is_some()
    });
    let found = checks::of(h.state().session.as_ref().unwrap());
    let verdict = |id: &str| found.iter().find(|c| c.id == id).unwrap().verdict.clone();
    assert_eq!(verdict("whole"), Verdict::Passed);
    let Verdict::Warned(said) = verdict("half") else { panic!("{:?}", verdict("half")) };
    assert!(said.starts_with("not all of it is above a half (land > 0.5: "), "{said}");
    assert_eq!(verdict("off"), Verdict::Off);
    let badges = checks::badges(&found, &[]);
    assert_eq!(badges.len(), 3);
    h.run_steps(2);
    assert!(h.query_by_label_contains("3 checks: 1 passed, 1 warned, 1 off").is_some(), "in the status bar");
    h.state_mut().focus(Pane::Document);
    h.run_steps(4);
    assert!(h.query_all_by_label_contains("3 checks: 1 passed, 1 warned, 1 off").count() >= 2, "in the document");
    assert!(h.query_by_label_contains("✔ passed").is_some(), "the step of the check that passed");
    assert!(h.query_by_label_contains("! failed, warning only: not all of it").is_some());
    shot(&mut h, "45-checks");
    h.state_mut().focus(Pane::Model);
    h.run_steps(4);
    shot(&mut h, "46-checks-canvas");
}

/// A node's result frozen: written next to the model, used in a later session instead of running it (what only it
/// reads not run either), marked on the canvas; thawed, it runs again. A whole run of the model saved writes its
/// manifest.
#[test]
fn a_frozen_result_is_kept_between_sessions() {
    use terraflux_app::editor_view::NodeStatus;
    use terraflux_graph::{Model, Node};
    let mut m = Model::new("frozen");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 3));
    m.nodes.push(Node::new("twice", "raster.calc").param("expression", "land * 2").input("land", "land.map"));
    m.nodes.push(Node::new("half", "raster.calc").param("expression", "twice / 2").input("twice", "twice.map"));
    let path = tmp("frozen.toml");
    let folder = terraflux_exec::frozen::folder(&path);
    let _ = std::fs::remove_dir_all(&folder);
    m.save(&path).unwrap();
    let run = |h: &mut Harness<TerrafluxApp>| {
        h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
        until(h, "the run", |a| {
            let s = a.session.as_ref().unwrap();
            !s.running() && s.value_of("half", "map").is_some()
        });
    };
    let mut h = harness(new_app(Some(path.clone())));
    loaded(&mut h);
    run(&mut h);
    let given = h.state().session.as_ref().unwrap().value_of("twice", "map");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert!(matches!(&s.recorded, Some(Ok(p)) if p.ends_with("manifest.json")), "{:?}", s.recorded);
        s.freeze(&[], "twice", true).unwrap();
        s.edited();
        s.save().unwrap();
    }
    assert!(folder.join("twice").join("frozen.json").is_file());
    // a later session: twice from disk, land not run
    let mut h = harness(new_app(Some(path.clone())));
    loaded(&mut h);
    run(&mut h);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.status.get("twice"), Some(&NodeStatus::Done { cached: true }));
        assert_eq!(s.status.get("land"), None, "only twice reads land");
        assert_eq!(s.status.get("half"), Some(&NodeStatus::Done { cached: false }));
        let (Some(terraflux_ops::Value::Map(a)), Some(terraflux_ops::Value::Map(b))) =
            (&given, s.value_of("twice", "map"))
        else {
            panic!()
        };
        assert_eq!(a.data, b.data);
    }
    h.state_mut().session.as_mut().unwrap().selected = Some("twice".into());
    h.run_steps(4);
    shot(&mut h, "47-frozen");
    // thawed: land and twice run again, the files gone
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.freeze(&[], "twice", false).unwrap();
        s.edited();
    }
    run(&mut h);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.status.get("land"), Some(&NodeStatus::Done { cached: false }));
    assert!(!folder.exists());
}

/// A node of a group's body frozen: written next to the model by its place (`group.node`), used in a later session
/// whatever it reads; and the viewer shows a node of a loop's body (its last step) and of a switch's branch.
#[test]
fn a_node_of_a_group_is_frozen_and_bodies_are_shown() {
    use terraflux_graph::{Body, Model, Node};
    let mut m = Model::new("frozen in a group");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 3));
    m.nodes.push(Node::new("g", terraflux_graph::GROUP).input("x", "land.map").body(Body {
        nodes: vec![
            Node::new("twice", "raster.calc").param("expression", "x * 2").input("x", "@x"),
            Node::new("half", "raster.calc").param("expression", "twice / 2").input("twice", "twice.map"),
        ],
        outputs: [("map".to_string(), "half.map".to_string())].into(),
        ..Default::default()
    }));
    m.nodes.push(Node::new("years", "control.repeat").literal("times", 3).literal("v", 0).body(Body {
        nodes: vec![
            Node::new("step", "value.calc").input("v", "@v").param("expression", "v + 1"),
            Node::new("grow", "nlm.random").param("rows", 10).param("columns", 10).param("seed", 5),
        ],
        carry: [("v".to_string(), "step.value".to_string())].into(),
        ..Default::default()
    }));
    m.nodes.push(Node::new("which", "value.calc").param("text", "a"));
    m.nodes.push(Node::new("sw", terraflux_graph::SWITCH).input("case", "which.text").input("map", "land.map").body(
        Body {
            nodes: vec![
                Node::new("a", terraflux_graph::GROUP).input("x", "@map").body(Body {
                    nodes: vec![Node::new("inv", "raster.calc").param("expression", "1 - x").input("x", "@x")],
                    outputs: [("map".to_string(), "inv.map".to_string())].into(),
                    ..Default::default()
                }),
                Node::new("otherwise", terraflux_graph::GROUP).input("x", "@map").body(Body {
                    nodes: vec![Node::new("same", "raster.calc").param("expression", "x").input("x", "@x")],
                    outputs: [("map".to_string(), "same.map".to_string())].into(),
                    ..Default::default()
                }),
            ],
            ..Default::default()
        },
    ));
    let path = tmp("frozen_group.toml");
    let folder = terraflux_exec::frozen::folder(&path);
    let _ = std::fs::remove_dir_all(&folder);
    m.save(&path).unwrap();
    let run = |h: &mut Harness<TerrafluxApp>| {
        h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
        until(h, "the run", |a| {
            let s = a.session.as_ref().unwrap();
            !s.running() && s.value_of("g", "map").is_some()
        });
    };
    let map = |v: Option<terraflux_ops::Value>| match v {
        Some(terraflux_ops::Value::Map(m)) => m.data.to_vec(),
        v => panic!("{:?}", v.map(|v| v.ty())),
    };
    let mut h = harness(new_app(Some(path.clone())));
    loaded(&mut h);
    run(&mut h);
    let r = terraflux_ops::Registry::standard();
    let given = map(h.state().session.as_ref().unwrap().value_of("g", "twice.map"));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        // a loop's body: the node's map at the last step, named so in the viewer
        s.editor.path = vec!["years".into()];
        terraflux_app::builder::select(s, &r, "grow");
        assert_eq!(s.preview, Some(("years".to_string(), "grow.map".to_string())));
        assert!(s.preview_value().is_some(), "the loop gives what its body gave at the last step");
        // a switch's branch that ran
        s.editor.path = vec!["sw".into(), "a".into()];
        terraflux_app::builder::select(s, &r, "inv");
        assert_eq!(s.preview, Some(("sw".to_string(), "a.inv.map".to_string())));
        assert!(s.preview_value().is_some(), "the switch gives what its branch gave");
        s.editor.path = vec!["sw".into(), "otherwise".into()];
        terraflux_app::builder::select(s, &r, "same");
        assert!(s.preview_value().is_none(), "a branch that did not run gives nothing");
        // back to the loop's node, for the viewer
        s.editor.path = vec!["years".into()];
        terraflux_app::builder::select(s, &r, "grow");
    }
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(4);
    assert!(h.query_by_label("years/grow (the last step)").is_some(), "the viewer says it is the last step");
    shot(&mut h, "51-loop-body-viewer");
    // twice, in the group, frozen
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.editor.path = vec!["g".into()];
        s.freeze(&["g".to_string()], "twice", true).unwrap();
        s.edited();
        s.save().unwrap();
    }
    assert!(folder.join("g.twice").join("frozen.json").is_file());
    // a later session, land drawn otherwise: twice from disk, half on it
    let mut saved = Model::load(&path).unwrap();
    assert!(saved.node("g").unwrap().body.as_ref().unwrap().nodes[0].frozen);
    saved.nodes[0].params.insert("seed".into(), 4.into());
    saved.save(&path).unwrap();
    let mut h = harness(new_app(Some(path.clone())));
    loaded(&mut h);
    run(&mut h);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(map(s.value_of("g", "twice.map")), given, "twice frozen, whatever land is now");
        let half: Vec<f64> = given.iter().map(|x| x / 2.0).collect();
        assert_eq!(map(s.value_of("g", "map")), half);
    }
    // thawed: twice runs again on land as it is now, the files gone
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.freeze(&["g".to_string()], "twice", false).unwrap();
        s.edited();
    }
    run(&mut h);
    let s = h.state().session.as_ref().unwrap();
    assert_ne!(map(s.value_of("g", "twice.map")), given);
    assert!(!folder.exists());
}

/// A cell of a patch's operator traced: its card selected, the patch it is in drawn on the viewer's map; another
/// cell picked lets it go.
#[test]
fn the_patch_of_a_traced_cell_is_drawn() {
    use terraflux_graph::{Model, Node};
    let mut m = Model::new("patches");
    m.nodes.push(
        Node::new("land", "nlm.random_cluster").param("rows", 40).param("columns", 40).param("seed", 2).param("p", 0.5),
    );
    m.nodes.push(Node::new("habitat", "raster.calc").param("expression", "land >= 2").input("land", "land.map"));
    m.nodes.push(Node::new("patches", "raster.label").input("map", "habitat.map"));
    let path = tmp("patches.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("patches", "map").is_some()
    });
    // a cell of a patch (labelled), traced from its map's tab
    let cell = {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(p)) = s.value_of("patches", "map") else { panic!() };
        let i = p.data.iter().position(|v| v.is_finite() && *v > 0.0).expect("a patch");
        (i / p.grid.width, i % p.grid.width)
    };
    show(&mut h, "patches", "map");
    h.run_steps(3);
    h.state_mut().session.as_mut().unwrap().preview_view.picked = Some(cell);
    h.run_steps(3);
    h.get_by_label("Trace this cell").click();
    h.run_steps(4);
    let t = h.state().trace.trace.clone().expect("traced");
    assert!(matches!(t.steps[0].how, terraflux_app::trace::How::Patch { .. }), "{:?}", t.steps[0].how);
    // (its name is also an input's link, which only brings the card into view)
    let links = h.get_all_by_label("patches.map").count();
    for k in 0..links {
        h.state_mut().focus(Pane::Trace);
        h.run_steps(2);
        if let Some(n) = h.get_all_by_label("patches.map").nth(k) {
            n.click_accesskit();
        }
        h.run_steps(3);
        // (the map is in the Viewer already: the link taken when it draws the patch)
        if h.state().session.as_ref().unwrap().preview_view.patch.is_some() {
            break;
        }
    }
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.preview, Some(("patches".to_string(), "map".to_string())));
        let o = s.preview_view.patch.as_ref().expect("the patch drawn");
        assert!(!o.edges.is_empty());
        let (x0, y0, x1, y1) = o.bounds;
        let (r, c) = (cell.0 as f32, cell.1 as f32);
        assert!(x0 <= c && c < x1 && y0 <= r && r < y1, "the cell in its patch's rows and columns");
    }
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(4);
    shot(&mut h, "53-trace-patch");
}

/// Step by step: the run pauses after a step of a loop, the status bar says where with Next and Continue.
#[test]
fn a_loop_pauses_step_by_step() {
    use std::sync::atomic::Ordering;
    use terraflux_graph::{Body, Model, Node};
    let mut m = Model::new("steps");
    m.nodes.push(Node::new("years", "control.repeat").literal("times", 3).body(Body {
        nodes: vec![Node::new("land", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 1)],
        collect: [("maps".to_string(), "land.map".to_string())].into(),
        ..Default::default()
    }));
    let path = tmp("steps.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.gate.stepping.store(true, Ordering::Relaxed);
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the first step", |a| a.session.as_ref().unwrap().gate.paused());
    h.run_steps(3);
    assert!(h.query_by_label_contains("paused at years: step 1 of 3").is_some());
    // (drawn while the run waits: not `shot`, which waits for its end)
    h.run_steps(4);
    if let Ok(img) = h.render() {
        img.save(tmp("ui-48-step.png")).unwrap();
    }
    h.get_by_label("Next").click();
    h.run_steps(2);
    until(&mut h, "the second step", |a| {
        let s = a.session.as_ref().unwrap();
        s.gate.paused() && s.progress.as_ref().is_some_and(|p| p.1 == 2)
    });
    h.get_by_label("Continue").click();
    until(&mut h, "the run", |a| !a.session.as_ref().unwrap().running());
    let s = h.state().session.as_ref().unwrap();
    assert!(s.error.is_none(), "{:?}", s.error);
    assert!(!s.gate.stepping.load(Ordering::Relaxed));
}

/// A breakpoint on a node: the run pauses before it (what ran before it done and shown), the status bar says so;
/// Next node pauses before the next one, Continue runs on to the next breakpoint (in a loop's body, every step).
#[test]
fn a_breakpoint_pauses_before_its_node() {
    use terraflux_graph::{Body, Model, Node};
    let mut m = Model::new("breakpoints");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 1));
    m.nodes.push(Node::new("twice", "raster.calc").param("expression", "land * 2").input("land", "land.map"));
    m.nodes.push(Node::new("half", "raster.calc").param("expression", "twice / 2").input("twice", "twice.map"));
    m.nodes.push(Node::new("years", "control.repeat").literal("times", 2).body(Body {
        nodes: vec![Node::new("grow", "nlm.random").param("rows", 5).param("columns", 5).param("seed", 1)],
        collect: [("maps".to_string(), "grow.map".to_string())].into(),
        ..Default::default()
    }));
    let path = tmp("breakpoints.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        // from the Node pane: twice selected, its breakpoint ticked
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "twice");
    }
    h.state_mut().focus(Pane::Node);
    h.run_steps(3);
    h.get_by_label("Breakpoint").click();
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().gate.breaks_at("twice"));
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.gate.set_breakpoint("years/grow", true);
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the breakpoint", |a| a.session.as_ref().unwrap().gate.paused());
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.gate.paused_at().as_deref(), Some("twice"));
        assert!(s.value_of("land", "map").is_some(), "what ran before it is there");
        assert!(s.cache.lock().unwrap().outputs("twice").is_none(), "it has not run");
    }
    assert!(h.query_by_label_contains("paused before twice").is_some());
    h.state_mut().focus(Pane::Model);
    h.run_steps(4);
    if let Ok(img) = h.render() {
        img.save(tmp("ui-54-breakpoint.png")).unwrap();
    }
    // Next node: twice runs, the run pauses before half
    h.get_by_label("Next node").click();
    until(&mut h, "the next node", |a| a.session.as_ref().unwrap().gate.paused_at().as_deref() == Some("half"));
    assert!(!h.state().session.as_ref().unwrap().gate.breaks_at("half"));
    // (the bar drawn again: its buttons where they are now)
    h.run_steps(3);
    assert!(h.query_by_label_contains("paused before half").is_some());
    // Continue: on to the breakpoint in the loop's body, at each step
    h.get_by_label("Continue").click();
    for step in 1..=2 {
        // (paused before grow at this step: the steps before it done)
        until(&mut h, "the loop's breakpoint", |a| {
            let s = a.session.as_ref().unwrap();
            s.gate.paused_at().as_deref() == Some("years/grow") && s.progress.as_ref().map_or(0, |p| p.1) == step - 1
        });
        h.run_steps(3);
        assert!(h.query_by_label_contains("paused before years/grow").is_some(), "step {step}");
        h.get_by_label("Continue").click();
        h.run_steps(2);
    }
    until(&mut h, "the run", |a| !a.session.as_ref().unwrap().running());
    let s = h.state().session.as_ref().unwrap();
    assert!(s.error.is_none(), "{:?}", s.error);
    assert!(s.value_of("years", "maps").is_some());
}

/// The document says what each step does in a sentence, edits a list and a table in place, and draws the maps in the viewer's
/// palettes (MSPA's classes in GuidosToolbox's colours).
#[test]
fn the_document_reads_and_edits_in_place() {
    use terraflux_graph::{Model, Node};
    let mut m = Model::new("sentences");
    m.nodes.push(
        Node::new("patches", "nlm.random_cluster")
            .param("rows", 60)
            .param("columns", 80)
            .param("seed", 2)
            .param("weights", serde_json::json!([0.3, 0.7])),
    );
    m.nodes.push(Node::new("habitat", "nlm.binarize").input("map", "patches.map").param("breaks", 0.4));
    m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "habitat.map").param("foreground", 1));
    let classes = serde_json::json!([[0, "matrix", "#cccccc"], [1, "habitat", "#228833"]]);
    m.nodes.push(Node::new("legend", "raster.legend").input("map", "habitat.map").param("classes", classes));
    let path = tmp("sentences.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("mspa", "map").is_some()
    });
    h.state_mut().focus(Pane::Document);
    h.run_steps(4);
    shot(&mut h, "49-document-sentences");
    assert!(
        h.query_by_label_contains("Random clusters on 60 × 80 cells, a share 0.5 of the cells in clusters, the classes weighted 0.3, 0.7; seed 2.")
            .is_some()
    );
    assert!(h.query_by_label_contains("Patches split into habitat and matrix, a share 0.4 of it habitat.").is_some());
    assert!(h.query_by_label_contains("MSPA's patterns of habitat, its foreground 1").is_some());
    // the list of weights, one more in place
    h.get_by_label("+ add").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let weights = s.source.nodes[0].params.get("weights").cloned();
    assert_eq!(weights, Some(serde_json::json!([0.3, 0.7, 0.7])));
    // a table of several columns (the legend's value, name and colour), cell by cell: a row more in place
    h.get_by_label("+ row").scroll_to_me();
    h.run_steps(20);
    h.get_by_label("+ row").click();
    h.run_steps(3);
    shot(&mut h, "49-document-table");
    let s = h.state().session.as_ref().unwrap();
    let classes = s.source.nodes.iter().find(|n| n.id == "legend").and_then(|n| n.params.get("classes")).cloned();
    let three = serde_json::json!([[0, "matrix", "#cccccc"], [1, "habitat", "#228833"], [1, "habitat", "#228833"]]);
    assert_eq!(classes, Some(three));
}

/// A model in stages: zoomed out, the canvas frames and names each stage; grouped by stage, it shows a block per
/// stage with its note, and the document a section per group, the maps of the groups' steps kept from the run.
#[test]
fn the_model_reads_in_stages() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "A species in a warmer climate").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("mess", "result").is_some()
    });
    h.state_mut().focus(Pane::Model);
    h.run_steps(2);
    let zoom_out = |h: &mut Harness<TerrafluxApp>| {
        let s = h.state_mut().session.as_mut().unwrap();
        s.editor.look(egui::vec2(60.0, 120.0), 0.25);
        h.run_steps(3);
    };
    zoom_out(&mut h);
    assert!(h.query_by_label("Preparation").is_none(), "the frames' names are painted, not widgets");
    shot(&mut h, "42-stages");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::group_by_stage(s, &terraflux_ops::Registry::standard());
        assert!(s.editor.message.is_none(), "{:?}", s.editor.message);
        let groups: Vec<&str> =
            s.source.nodes.iter().filter(|n| n.op == terraflux_graph::GROUP).map(|n| n.id.as_str()).collect();
        // (the template has a node named data; the results are a node alone, out of groups)
        assert_eq!(groups, ["data_stage", "preparation", "model", "validation"]);
        assert!(s.history.can_undo());
        s.run_when_ready = true;
    }
    h.run_steps(2);
    until(&mut h, "the grouped model's run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("model", "fit.model").is_some()
    });
    h.state_mut().session.as_mut().unwrap().editor.look(egui::vec2(30.0, 120.0), 0.7);
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.message.is_none());
    shot(&mut h, "43-groups");
    h.state_mut().focus(Pane::Document);
    h.run_steps(4);
    for label in ["the group model", "The model: what is fitted, simulated or optimised.", "Figure 1."] {
        assert!(h.query_by_label_contains(label).is_some(), "{label}");
    }
    assert!(h.query_by_label_contains("Not run.").is_none(), "the groups' steps shown from their run");
    shot(&mut h, "44-document-groups");
    // inside a group: a node of its body selected, its map in the viewer
    h.state_mut().focus(Pane::Model);
    let r = terraflux_ops::Registry::standard();
    let inner = {
        let s = h.state_mut().session.as_mut().unwrap();
        s.editor.path = vec!["preparation".into()];
        let body = &s.source.node("preparation").unwrap().body.as_ref().unwrap().nodes;
        let inner = body
            .iter()
            .find(|n| {
                terraflux_app::editor::output_ports(n, &r)
                    .first()
                    .is_some_and(|(_, t)| *t == terraflux_graph::Type::Map)
            })
            .map(|n| n.id.clone())
            .expect("a node of the preparation that gives a map");
        terraflux_app::builder::select(s, &r, &inner);
        let (node, port) = s.preview.clone().unwrap();
        assert_eq!(node, "preparation");
        assert!(port.starts_with(&format!("{inner}.")), "{port}");
        assert!(s.preview_value().is_some(), "the group gives what its nodes gave");
        inner
    };
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(4);
    assert!(h.query_by_label(&format!("preparation/{inner}")).is_some(), "the viewer names the node by its place");
    assert!(h.query_by_label_contains("Not run yet").is_none());
    shot(&mut h, "50-group-viewer");
}

/// The model's parameters as a form: a setting offered with its range and help, its values kept as scenarios,
/// a scenario applied (an edit, the model run again) and two compared side by side with their difference.
#[test]
fn parameters_and_scenarios_from_their_pane() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "Neutral landscapes").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("field", "map").is_some()
    });
    {
        let r = terraflux_ops::Registry::standard();
        let s = h.state_mut().session.as_mut().unwrap();
        let mut p = terraflux_app::params_view::parameter(&s.source, &r, "field.range");
        (p.min, p.max, p.step, p.unit) = (Some(2.0), Some(60.0), Some(1.0), Some("cells".into()));
        s.source.parameters.push(p);
        s.source.scenarios.push(terraflux_graph::Scenario {
            name: "fine".into(),
            note: Some("short patches".into()),
            values: [("field.range".to_string(), serde_json::json!(5))].into(),
        });
        s.edited();
    }
    h.state_mut().focus(Pane::Parameters);
    h.run_steps(3);
    assert!(h.query_all_by_label_contains("The range of the autocorrelation, in cells").next().is_some());
    // the values now, kept as a scenario
    h.state_mut().params.name = "coarse".into();
    h.get_by_label("Save the values").click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.scenarios.len(), 2);
    assert_eq!(s.source.scenarios[1].values["field.range"], 20);
    // both side by side, the field's map
    {
        let a = h.state_mut();
        a.params.chosen = ["fine".to_string(), "coarse".to_string()].into();
        a.params.outputs = vec!["field.map".into()];
    }
    h.run_steps(1);
    h.get_by_label("Compare").click();
    until(&mut h, "the comparison", |a| !a.params.compared.is_empty());
    h.run_steps(3);
    let compared = &h.state().params.compared[0].compared;
    assert_eq!(compared.len(), 2);
    assert!(
        compared.iter().all(|(_, v)| v.is_ok()),
        "{:?}",
        compared.iter().map(|(n, v)| (n, v.is_ok())).collect::<Vec<_>>()
    );
    assert!(h.query_by_label_contains("the others less this one").is_some());
    shot(&mut h, "45-parameters");
    // the same through Many runs: the scenarios as a batch on this computer's pool, their maps back here
    let runs = tmp("ui-params");
    let _ = std::fs::remove_dir_all(&runs);
    let _ = std::fs::remove_dir_all(tmp("ui-params-pool"));
    {
        let a = h.state_mut();
        a.computers.pool = None;
        a.computers.dir = Some(tmp("ui-params-pool"));
        a.batch.workers = 2;
        let s = a.session.as_mut().unwrap();
        std::fs::create_dir_all(&runs).unwrap();
        s.source.save(runs.join("neutral.toml")).unwrap();
        s.path = Some(runs.join("neutral.toml"));
        a.params.compared.clear();
    }
    h.run_steps(1);
    h.get_by_label("Compare in Many runs").click();
    h.run_steps(2);
    assert!(h.state().batch.running(), "{:?}", h.state().params.error);
    assert!(h.state().params.in_batch);
    until(&mut h, "the comparison in Many runs", |a| !a.params.compared.is_empty());
    h.run_steps(3);
    {
        let a = h.state();
        let (output, compared) = (&a.params.compared[0].output, &a.params.compared[0].compared);
        assert_eq!(output, "field.map");
        let names: Vec<&str> = compared.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["fine", "coarse"], "in the scenarios' order");
        for (n, v) in compared {
            assert!(matches!(v, Ok(terraflux_ops::Value::Map(_))), "{n}: {:?}", v.as_ref().err());
        }
        assert!(!a.params.in_batch);
        assert!(a.batch.finished.is_some(), "Many runs shows its runs too");
    }
    // maps of classes: their differences are the cells that changed class
    {
        let a = h.state_mut();
        let s = a.session.as_mut().unwrap();
        s.source.scenarios[0].values.insert("clusters.seed".into(), serde_json::json!(9));
        s.edited();
        // two outputs at once: the clusters' classes and the field's values
        a.params.outputs = vec!["clusters.map".into(), "field.map".into()];
        a.params.compared.clear();
    }
    h.run_steps(2);
    h.get_by_label("Compare").click();
    until(&mut h, "the comparison of classes", |a| !a.params.compared.is_empty());
    h.run_steps(3);
    {
        let c = &h.state().params.compared;
        let outputs: Vec<&str> = c.iter().map(|c| c.output.as_str()).collect();
        assert_eq!(outputs, ["clusters.map", "field.map"]);
        assert!(c[0].classes, "the clusters are classes, as the viewer shows them");
        assert!(!c[1].classes);
        assert!(c.iter().all(|c| c.compared.iter().all(|(_, v)| v.is_ok())));
    }
    assert!(h.query_by_label_contains("where the others' classes differ").is_some());
    assert!(h.query_by_label_contains("the others less this one").is_some(), "the field's block too");
    assert!(h.query_by_label_contains("cells changed class").is_some());
    // the whole transition matrix offered (params_view's tests check its counts)
    assert!(h.query_by_label("the whole transition matrix").is_some());
    shot(&mut h, "51-scenario-classes");
    // a scenario applied: an edit of the model, run again
    h.get_all_by_label("Apply").next().unwrap().click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.node("field").unwrap().params["range"], 5);
    assert!(s.history.can_undo());
    until(&mut h, "the run again", |a| !a.session.as_ref().unwrap().running());
    // the document tells them
    h.state_mut().focus(Pane::Document);
    h.run_steps(3);
    assert!(h.query_by_label_contains("Scenarios").is_some());
    assert!(h.query_all_by_label_contains("short patches").count() >= 2, "in the pane and the document");
}

/// Why a cell has its value: a cell of a map's tab clicked, then Trace this cell: its pane with the chain of
/// values, map algebra's expression with the values in place; inside the loop when asked; to the assistant.
#[test]
fn a_cell_is_traced_from_its_map() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "CLUE-S").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("simulation", "maps").is_some()
    });
    show(&mut h, "simulation", "maps");
    h.run_steps(3);
    h.state_mut().session.as_mut().unwrap().preview_view.picked = Some((40, 60));
    h.run_steps(3);
    h.get_by_label("Trace this cell").click();
    h.run_steps(4);
    assert!(h.state().is_open(Pane::Trace));
    let t = h.state().trace.trace.clone().expect("traced");
    assert_eq!(t.cell, (40, 60));
    assert!(h.query_by_label_contains("Here simulation.maps is").is_some());
    assert!(h.query_all_by_label_contains("here: 2 if ").next().is_some(), "the expression with the values");
    // inside the loop
    h.get_by_label("Trace inside the loops").click_accesskit();
    until(&mut h, "the run kept", |a| a.trace.kept.is_some() || a.trace.error.is_some());
    assert!(h.state().trace.error.is_none(), "{:?}", h.state().trace.error);
    h.run_steps(4);
    assert!(h.query_all_by_label_contains("in the loop, allocate.landscape").next().is_some());
    shot(&mut h, "46-trace");
    // a card selected: its node's map in the viewer, the cell traced outlined (its name is also an input's link,
    // which only brings the card into view)
    let links = h.get_all_by_label("land_2010.map").count();
    for k in 0..links {
        h.state_mut().focus(Pane::Trace);
        h.run_steps(2);
        if let Some(n) = h.get_all_by_label("land_2010.map").nth(k) {
            n.click_accesskit();
        }
        h.run_steps(3);
        let s = h.state().session.as_ref().unwrap();
        if s.preview.as_ref().is_some_and(|(n, _)| n == "land_2010") {
            break;
        }
    }
    h.run_steps(2);
    {
        let a = h.state();
        let s = a.session.as_ref().unwrap();
        assert_eq!(s.preview, Some(("land_2010".to_string(), "map".to_string())));
        assert_eq!(s.preview_view.picked, Some((40, 60)));
        assert!(s.preview_view.patch.is_none(), "no patch: not a patch's operator");
        assert!(a.is_open(Pane::Viewer));
    }
    shot(&mut h, "52-trace-card");
    h.state_mut().focus(Pane::Trace);
    h.run_steps(3);
    h.get_by_label("Ask the assistant why").scroll_to_me();
    h.run_steps(30);
    // to the assistant (no server chosen: its settings shown)
    h.get_by_label("Ask the assistant why").click();
    h.run_steps(3);
    assert!(h.state().is_open(Pane::Assistant));
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
        !s.running() && plotted(s, "plot_fad").is_some()
    });
    fn look(h: &mut Harness<TerrafluxApp>, node: &str) -> terraflux_app::app::Look {
        let of = |a: &TerrafluxApp| {
            let s = a.session.as_ref().unwrap();
            let Some(terraflux_ops::Value::Map(m)) = plotted(s, node) else { return None };
            s.looks.peek(&terraflux_app::map_view::Source::from(&m)).filter(|l| l.product.is_some()).cloned()
        };
        until(h, "the legend", |a| of(a).is_some());
        of(h.state()).unwrap()
    }
    // the template's two plots in the Viewer: FAD's first
    open_plot(&mut h, "plot_fad");
    let fad = look(&mut h, "plot_fad");
    assert_eq!(fad.product, Some(Product::Density { five: false }));
    let terraflux_app::palette::Coloring::Classes(c) = &fad.coloring else { panic!("classes") };
    let lines = terraflux_app::map_view::legend_lines(c, &fad.counts, &|v| terraflux_app::builder::class_name(&fad, v));
    assert!(c.len() > 20 && lines.len() <= 8, "{} values in {} lines: {lines:?}", c.len(), lines.len());
    assert!(lines.iter().any(|(_, n, _)| n == "Background"));
    shot(&mut h, "18-fragmentation");
    // FOS's: in 5 classes
    open_plot(&mut h, "plot_fos");
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
        !s.running() && plotted(s, "plot_contagion").is_some()
    });
    let lines = |h: &mut Harness<TerrafluxApp>, node: &str| {
        open_plot(h, node);
        let of = |a: &TerrafluxApp| {
            let s = a.session.as_ref().unwrap();
            let Some(terraflux_ops::Value::Map(m)) = plotted(s, node) else { return None };
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
        !s.running() && plotted(s, "plot_window").is_some()
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
        !s.running() && plotted(s, "plot_rank").is_some()
    });
    open_plot(&mut h, "plot_rank");
    until(&mut h, "the legend", |a| {
        let s = a.session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(m)) = plotted(s, "plot_rank") else { return false };
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

/// The keyboard: Tab to the model and to the last map seen, ctrl+W closes the tab in focus, ctrl+R
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
    // from a pane of another kind: to the model, then to the last map seen (not back to it)
    h.state_mut().focus(Pane::Document);
    h.run_steps(2);
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
    assert!(
        h.query_all_by_label_contains("problems").next().is_some() || h.query_by_label_contains("No problem").is_some()
    );
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

/// The panes' places: the model in most of the window, the node's pane at its left (the preparation in a tab beside
/// it), nothing at the right; the operators and the files in drawers the pointer opens at the window's left edge
/// (and closes leaving it), the operators in the toolbar too, their panes, shown, at the left edge; the assistant
/// at the right; backspace deletes the node selected; a tab dragged
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
    assert!(!h.state().is_open(Pane::Files) && !h.state().is_open(Pane::Assistant), "nothing more at first");
    let rect = |h: &Harness<TerrafluxApp>, pane: Pane| {
        let dock = h.state().dock.as_ref().unwrap();
        let path = dock.find_tab(&pane).unwrap();
        dock[path.surface][path.node].rect().unwrap()
    };
    let left = |h: &Harness<TerrafluxApp>, pane: Pane| rect(h, pane).left();
    let (model, node) = (rect(&h, Pane::Model), rect(&h, Pane::Node));
    assert!(node.left() < model.left(), "the node, then the model from the left: {node:?} {model:?}");
    // the node's pane a fifth of the window, the model to the right edge
    let width = h.ctx.content_rect().width();
    assert!((node.width() / width - 0.2).abs() < 0.03, "the node's width: {}", node.width());
    assert!(width - model.right() < 12.0, "nothing at the right: {}", width - model.right());
    // the files' drawer: the pointer over its tab of the strip
    h.state_mut().files.go(data(""));
    let files_tab = egui::pos2(8.0, rect(&h, Pane::Model).top() + 180.0);
    h.event(egui::Event::PointerMoved(files_tab));
    h.run_steps(3);
    assert_eq!(h.state().drawer, Some(Pane::Files), "the files' drawer");
    assert!(h.query_by_label_contains("sibuyan_landuse.tif").is_some(), "the folder's rasters in it");
    shot(&mut h, "20-files-drawer");
    h.event(egui::Event::PointerMoved(egui::pos2(700.0, 300.0)));
    h.run_steps(3);
    assert_eq!(h.state().drawer, None);
    // the files' pane: the data of a folder, a raster carried to the model from there
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
    assert!(h.query_by_label("FAVOURITES").is_some(), "the operators in their pane, from the favourites");
    assert!(h.query_by_label("Open a raster...").is_none(), "no buttons of data");
    h.event(egui::Event::PointerMoved(egui::pos2(1.0, 300.0)));
    h.run_steps(3);
    assert!(h.state().drawer.is_none(), "no drawer over the operators' pane");
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
    assert!(h.query_by_label("FAVOURITES").is_none());
    // the pointer at the left edge: the drawer
    h.event(egui::Event::PointerMoved(egui::pos2(1.0, 300.0)));
    h.run_steps(3);
    assert_eq!(h.state().drawer, Some(Pane::Operators));
    assert!(h.query_by_label("FAVOURITES").is_some(), "the operators in the drawer");
    shot(&mut h, "20-drawer");
    h.event(egui::Event::PointerMoved(egui::pos2(700.0, 300.0)));
    h.run_steps(3);
    assert!(h.state().drawer.is_none(), "away from it, it goes in");
    // the assistant: at the right, alone
    h.state_mut().focus(Pane::Assistant);
    h.run_steps(2);
    assert!(left(&h, Pane::Assistant) > left(&h, Pane::Model), "the assistant at the right");
    {
        let dock = h.state().dock.as_ref().unwrap();
        let (a, b) = (dock.find_tab(&Pane::Assistant).unwrap(), dock.find_tab(&Pane::Node).unwrap());
        assert_ne!((a.surface, a.node), (b.surface, b.node), "not with the node");
    }
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
    // a click on the strip at the left edge: the operators' pane, the strip gone
    h.event(egui::Event::PointerMoved(egui::pos2(700.0, 300.0)));
    h.run_steps(2);
    h.drag_at(egui::pos2(18.0, 100.0));
    h.run_steps(1);
    h.drop_at(egui::pos2(18.0, 100.0));
    h.run_steps(3);
    assert!(h.state().is_open(Pane::Operators), "the strip opens the operators' pane");
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
        !s.running() && s.value_of("land", "raster").is_some() && s.value_of("roads", "vector").is_some()
    });
    // the plots made Plots: the top one's layers, the land under the roads
    {
        let s = h.state().session.as_ref().unwrap();
        let top = s.source.plot("top").expect("the Plot top");
        let from: Vec<&str> = top.layers.iter().map(|l| l.from.as_str()).collect();
        assert_eq!(from, ["land.raster", "roads.vector"]);
    }
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), "top");
        let drivers =
            terraflux_app::builder::Layer { opacity: 0.5, ..terraflux_app::builder::Layer::new("drivers", "raster") };
        s.preview_layers.push(drivers);
    }
    h.state_mut().focus(Pane::Viewer);
    drawn(&mut h);
    assert!(h.query_by_label("show drivers").is_some());
    shot(&mut h, "21-stacked");
    // the panel of the layers, at the map's left: hidden by its button, and shown again
    use egui::accesskit::Role;
    h.get_by_role_and_label(Role::Button, "Layers").click();
    h.run_steps(20);
    assert!(h.query_by_label("show drivers").is_none(), "the layers hidden");
    h.get_by_role_and_label(Role::Button, "Layers").click();
    h.run_steps(20);
    assert!(h.query_by_label("show drivers").is_some(), "the layers shown");
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
    // zoom to layer: the roads (their row selected, then the zoom of the bar), the view on their 30 km, inside the
    // land
    h.get_by_label("roads.vector").click();
    h.run_steps(2);
    h.get_by_label("Zoom to layer").click();
    h.run_steps(2);
    let (zoom, center) = h.state().session.as_ref().unwrap().preview_view.camera();
    // (the roads span 120 columns of 250 m: the view about 900 points wide)
    assert!(zoom > 2.0, "{zoom}");
    let g = h.state().session.as_ref().unwrap().preview_view.shown_grid().unwrap().clone();
    let (x, y) = (g.transform[0] + center.x as f64 * g.transform[1], g.transform[3] + center.y as f64 * g.transform[5]);
    assert!((x - 452000.0).abs() < 500.0 && (y - 1367500.0).abs() < 500.0, "{x} {y}");
    // the layers: the Plot's (the land, the roads) and the drivers on top; the drivers' name clicked: their map shown
    let kept = |h: &Harness<TerrafluxApp>| -> Vec<(String, String)> {
        let s = h.state().session.as_ref().unwrap();
        s.preview_layers.iter().map(|l| (l.node.clone(), l.port.clone())).collect()
    };
    let pair = |n: &str, p: &str| (n.to_string(), p.to_string());
    assert_eq!(kept(&h), [pair("land", "raster"), pair("roads", "vector"), pair("drivers", "raster")]);
    assert_eq!(h.state().session.as_ref().unwrap().preview, Some(pair("land", "raster")), "the Plot's map shown");
    h.get_by_label("drivers").click();
    drawn(&mut h);
    assert_eq!(h.state().session.as_ref().unwrap().preview, Some(pair("drivers", "raster")));
    use egui_kittest::kittest::NodeT;
    // the drivers (selected by the click) down, under the roads, by the bar's down
    h.get_by_label("Down").click();
    drawn(&mut h);
    assert_eq!(kept(&h)[1], pair("drivers", "raster"), "the drivers under the roads");
    // the land no longer a layer: removed from its menu (a right click on its row)
    h.get_by_label("land").click_secondary();
    h.run_steps(2);
    h.get_by_label("🗑 Remove layer").click();
    h.run_steps(2);
    assert_eq!(kept(&h), [pair("drivers", "raster"), pair("roads", "vector")]);
    // a map not among them, kept with the pin: on top
    show(&mut h, "land", "raster");
    drawn(&mut h);
    h.get_by_label("Keep as a layer").click();
    h.run_steps(2);
    assert_eq!(kept(&h).last(), Some(&pair("land", "raster")));
    // the land (on top) dragged by its row under the drivers' (at the bottom): a line where it goes, and there
    // (the row: the widest of what is so named, not the node's name in the Node pane)
    let name_at = |h: &Harness<TerrafluxApp>, name: &str| -> egui::Pos2 {
        let width = |n: &egui_kittest::Node| n.accesskit_node().bounding_box().map_or(0.0, |b| b.x1 - b.x0);
        let n = h.query_all_by_label(name).max_by(|a, b| width(a).total_cmp(&width(b)));
        let b = n.unwrap_or_else(|| panic!("the layer {name}")).accesskit_node().bounding_box().unwrap();
        egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
    };
    let from = name_at(&h, "land");
    // (below the drivers' row and its legend, the last: the place under it)
    let to = name_at(&h, "drivers") + egui::vec2(0.0, 150.0);
    h.event(egui::Event::PointerMoved(from));
    h.run_steps(1);
    let press = |pos, pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Default::default(),
    };
    h.event(press(from, true));
    h.run_steps(1);
    for k in 1..=8 {
        h.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 8.0)));
        h.run_steps(1);
    }
    shot(&mut h, "21-layer-dragged");
    h.event(press(to, false));
    h.run_steps(2);
    assert_eq!(kept(&h), [pair("land", "raster"), pair("drivers", "raster"), pair("roads", "vector")]);
    // a cell of the drivers' stack clicked: its values along the layers
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_layers.clear();
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
    // (the run over: the bar at the top as it stays)
    drawn(&mut h);
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
    // (only what this test changed: the settings are global, other tests share and lend at the same time)
    terraflux_app::settings::update(|s| {
        s.theme = "Default".into();
        s.favourites.clear();
        s.collection_names.clear();
    });
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
    until(&mut h, "the raster read", |a| a.session.as_ref().unwrap().value_of("sibuyan_landuse", "raster").is_some());
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
    // (again while the menu, laid out among what threads still bring, missed the click)
    let added = |h: &Harness<TerrafluxApp>| {
        h.state().session.as_ref().unwrap().source.nodes.iter().any(|n| n.op == "raster.reclassify")
    };
    let t = Instant::now();
    while !added(&h) {
        h.get_by_label("Operators").click();
        h.run_steps(2);
        shot(&mut h, "26-operators-menu");
        // the menu's, over the operators' pane's
        h.query_all_by_label_contains("Data preparation").last().expect("the subject's submenu").click();
        h.run_steps(2);
        shot(&mut h, "26-operators-menu");
        h.query_all_by_label_contains("reclassify").last().expect("the submenu's operator").click();
        h.run_steps(3);
        assert!(t.elapsed() < Duration::from_secs(120), "the operator from the menu: too long");
    }
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
    until(&mut h, "the raster read", |a| a.session.as_ref().unwrap().value_of("sibuyan_landuse", "raster").is_some());
    h.state_mut().focus(Pane::Viewer);
    drawn(&mut h);
    h.run_steps(5);
    // the legend under the map's row in the Layers, its editor there
    assert!(h.query_by_label("Apply").is_none());
    h.get_by_label("Edit legend").click();
    h.run_steps(3);
    assert!(h.query_by_label("Edit legend").is_none(), "the editor in the legend's place");
    assert!(h.query_by_label("Apply").is_some(), "the editor in the legend's place");
    // the first class's name field (the first text field of the editor): "1" replaced by "forest"
    fn field<'a>(h: &'a Harness<'static, TerrafluxApp>) -> egui_kittest::Node<'a> {
        use egui_kittest::kittest::NodeT;
        h.query_all_by_value("1")
            .rfind(|n| n.accesskit_node().role() == egui::accesskit::Role::TextInput)
            .expect("the name of class 1")
    }
    field(&h).scroll_to_me();
    h.run_steps(30);
    field(&h).click();
    h.run_steps(1);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    h.event(egui::Event::Text("forest".into()));
    h.run_steps(2);
    shot(&mut h, "29-legend-editor");
    h.get_by_label("Apply").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.op == "raster.legend").expect("raster.legend added");
    assert_eq!(n.inputs["map"], terraflux_graph::Source::Ref("sibuyan_landuse.raster".into()));
    let classes = terraflux_ops::legend::parse(&n.params["classes"]).unwrap();
    assert_eq!(classes[0].name, "forest");
    assert_eq!(s.selected.as_deref(), Some(n.id.as_str()));
    until(&mut h, "the legend's node run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_legend", "map").is_some()
    });
    drawn(&mut h);
    h.run_steps(5);
    assert!(h.query_all_by_label_contains("forest").count() > 0, "the name in the legend");
    shot(&mut h, "30-legend");
}

/// The layers change the picture (the harness's pixels at the middle of the map): a map over the one shown, half
/// opaque, hidden, put under it.
#[test]
fn the_layers_change_the_picture() {
    use terraflux_app::builder::Layer;
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes = vec![
            Node::new("land", "raster.read")
                .param("path", data("sibuyan_landuse.tif").to_string_lossy().as_ref())
                .param("band", "0"),
            Node::new("drivers", "raster.read").param("path", data("sibuyan_drivers.tif").to_string_lossy().as_ref()),
        ];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("land", "raster").is_some() && s.value_of("drivers", "raster").is_some()
    });
    show(&mut h, "land", "raster");
    // the pixel at the middle of the map, once drawn and faded in
    let pixel = |h: &mut Harness<TerrafluxApp>| -> [u8; 4] {
        drawn(h);
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(500) {
            h.step();
            std::thread::sleep(Duration::from_millis(10));
        }
        drawn(h);
        let at = h.state().session.as_ref().unwrap().preview_view.rect().expect("the map drawn").center();
        let img = h.render().expect("the window drawn by wgpu");
        img.get_pixel(at.x as u32, at.y as u32).0
    };
    let layers =
        |h: &mut Harness<TerrafluxApp>, l: Vec<Layer>| h.state_mut().session.as_mut().unwrap().preview_layers = l;
    let alone = pixel(&mut h);
    // the drivers over the land (both layers, the drivers on top)
    layers(&mut h, vec![Layer::new("land", "raster"), Layer::new("drivers", "raster")]);
    let over = pixel(&mut h);
    assert_ne!(over, alone, "the drivers over the land");
    layers(&mut h, vec![Layer::new("land", "raster"), Layer { opacity: 0.5, ..Layer::new("drivers", "raster") }]);
    let half = pixel(&mut h);
    assert!(half != over && half != alone, "half opaque: between the two ({half:?}, {over:?}, {alone:?})");
    layers(&mut h, vec![Layer::new("land", "raster"), Layer { visible: false, ..Layer::new("drivers", "raster") }]);
    assert_eq!(pixel(&mut h), alone, "hidden");
    layers(&mut h, vec![Layer::new("drivers", "raster"), Layer::new("land", "raster")]);
    assert_eq!(pixel(&mut h), alone, "under the land");
}

/// The Layers as a GIS has them: a layer of the run added over the map shown when there are layers already (the
/// map shown kept under it), the map shown hidden by its box, its palette changed by a click on the palette, the
/// range of its colours set under its histogram and kept in a Plot, a layer shown alone from its menu.
#[test]
fn the_layers_as_a_gis_has_them() {
    use egui_kittest::kittest::NodeT;
    use terraflux_app::builder::{Layer, Palette};
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let drivers = data("sibuyan_drivers.tif").to_string_lossy().into_owned();
        s.source.nodes = vec![
            Node::new("land", "raster.read")
                .param("path", data("sibuyan_landuse.tif").to_string_lossy().as_ref())
                .param("band", "0"),
            Node::new("drivers", "raster.read").param("path", drivers.as_str()),
            Node::new("slope", "raster.read").param("path", drivers.as_str()).param("band", "1"),
        ];
        s.edited();
        assert!(s.issues.is_empty(), "{:?}", s.issues);
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "drivers", "slope"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    h.state_mut().session.as_mut().unwrap().preview_layers = vec![Layer::new("land", "raster")];
    show(&mut h, "drivers", "raster");
    drawn(&mut h);
    let kept = |h: &Harness<TerrafluxApp>| -> Vec<String> {
        let s = h.state().session.as_ref().unwrap();
        s.preview_layers.iter().map(|l| l.node.clone()).collect()
    };
    // a layer added while there are layers: on top, the map shown kept under it (it would hide it else)
    h.get_by_label("Add a layer").click();
    h.run_steps(2);
    h.get_by_label("slope").click();
    drawn(&mut h);
    assert_eq!(kept(&h), ["land", "drivers", "slope"]);
    // (the row of a name: the widest of what is so named)
    let row = |h: &Harness<TerrafluxApp>, name: &str| -> egui::Pos2 {
        let width = |n: &egui_kittest::Node| n.accesskit_node().bounding_box().map_or(0.0, |b| b.x1 - b.x0);
        let n = h.query_all_by_label(name).max_by(|a, b| width(a).total_cmp(&width(b)));
        let b = n.unwrap_or_else(|| panic!("{name}")).accesskit_node().bounding_box().unwrap();
        egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
    };
    // the slope hidden by its box: the map shown seen again
    h.get_by_label("show slope").click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert!(!s.preview_layers[2].visible && s.preview_layers[1].visible);
    // the map shown hidden by its own box: the view draws it as hidden
    h.get_by_label("show drivers").click();
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.opacity, Some(0.0));
    h.get_by_label("show drivers").click();
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.opacity, Some(1.0));
    // the palette of the map shown changed by a click on it (the one at the right, by its legend)
    let palette = |h: &Harness<TerrafluxApp>| -> egui::Pos2 {
        let right = h
            .query_all_by_label("palette")
            .filter_map(|n| n.accesskit_node().bounding_box())
            .max_by(|a, b| a.x0.total_cmp(&b.x0))
            .expect("the palette at the right");
        egui::pos2(((right.x0 + right.x1) / 2.0) as f32, ((right.y0 + right.y1) / 2.0) as f32)
    };
    let at = palette(&h);
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        });
    }
    h.run_steps(2);
    shot(&mut h, "layers-palettes");
    h.get_by_label("terrain").click();
    drawn(&mut h);
    let key = ("drivers".to_string(), "raster".to_string());
    let terrain = Palette::Chosen(terraflux_app::palette::Ramp::Terrain, false);
    assert_eq!(h.state().session.as_ref().unwrap().palettes.get(&key), Some(&terrain));
    // the range of its colours: over all its values, then back to the percentiles
    let x = |n: &egui_kittest::Node| n.accesskit_node().bounding_box().map_or(0.0, |b| b.x0);
    h.query_all_by_label("least to most").max_by(|a, b| x(a).total_cmp(&x(b))).expect("at the right").scroll_to_me();
    h.run_steps(30);
    h.query_all_by_label("least to most").max_by(|a, b| x(a).total_cmp(&x(b))).expect("at the right").click();
    drawn(&mut h);
    let range = h.state().session.as_ref().unwrap().ranges.get(&key).copied().expect("the range chosen");
    assert!(range.1 > range.0, "{range:?}");
    // kept in a Plot with the palette
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        let p = s.source.plot("plot_drivers").expect("the Plot");
        let l = p.layers.iter().find(|l| l.from == "drivers.raster").expect("the map shown");
        assert_eq!((l.palette.as_deref(), l.range), (Some("terrain"), Some([range.0, range.1])));
    }
    h.query_all_by_label("2% to 98%").max_by(|a, b| x(a).total_cmp(&x(b))).expect("at the right").click();
    drawn(&mut h);
    assert!(h.state().session.as_ref().unwrap().ranges.is_empty());
    // the land shown alone, from its menu (a right click on its row)
    let at = row(&h, "land");
    h.event(egui::Event::PointerMoved(at));
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Default::default(),
        });
    }
    h.run_steps(2);
    h.get_by_label("Show only this layer").click();
    h.run_steps(2);
    let visible: Vec<bool> = h.state().session.as_ref().unwrap().preview_layers.iter().map(|l| l.visible).collect();
    assert_eq!(visible, [true, false, false]);
    shot(&mut h, "layers-gis");
}

/// A layer's menu *Show its node*: the canvas to the front, the groups on the way opened, the node selected and
/// the camera on it; greyed out (with its reason) for a layer no node of the model makes.
#[test]
fn a_layers_menu_shows_its_node_on_the_canvas() {
    use egui_kittest::kittest::NodeT;
    use terraflux_app::builder::Layer;
    use terraflux_graph::{Body, Node};
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let land = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        let read = |id: &str, pos: [f32; 2]| {
            let mut n = Node::new(id, "raster.read").param("path", land.as_str()).param("band", "0");
            n.pos = Some(pos);
            n
        };
        s.source.nodes = vec![read("land", [0.0, 0.0]), read("drivers", [300.0, 0.0])];
        s.edited();
        s.start(Box::new(|| {}));
        // (a loop added after the run: the layers of its body are named by the loop and the port `inner.raster`; the
        // layers are keyed by the nodes at the top)
        let body = Body { nodes: vec![read("inner", [900.0, 700.0])], ..Default::default() };
        s.source.nodes.push(Node::new("lp", "control.repeat").body(body));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("land", "raster").is_some()
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        // (a difference of two maps: no one node makes it)
        let diff = Layer { minus: Some(("drivers".into(), "raster".into())), ..Layer::new("land", "raster") };
        s.preview_layers = vec![Layer::new("land", "raster"), Layer::new("lp", "inner.raster"), diff];
    }
    show(&mut h, "land", "raster");
    drawn(&mut h);
    let row = |h: &Harness<TerrafluxApp>, name: &str| -> egui::Pos2 {
        let width = |n: &egui_kittest::Node| n.accesskit_node().bounding_box().map_or(0.0, |b| b.x1 - b.x0);
        let n = h.query_all_by_label(name).max_by(|a, b| width(a).total_cmp(&width(b)));
        let b = n.unwrap_or_else(|| panic!("{name}")).accesskit_node().bounding_box().unwrap();
        egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
    };
    let menu = |h: &mut Harness<TerrafluxApp>, name: &str| {
        let at = row(h, name);
        h.event(egui::Event::PointerMoved(at));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Default::default(),
            });
        }
        h.run_steps(2);
    };
    let centred = |h: &Harness<TerrafluxApp>, id: &str| {
        let e = &h.state().session.as_ref().unwrap().editor;
        e.screen_rect_of(id).is_some_and(|r| r.contains(e.canvas().unwrap().center()))
    };
    // a layer of a node at the top: the canvas in front, the node selected and framed
    menu(&mut h, "land");
    h.get_by_label("Show its node").click();
    h.run_steps(40);
    assert_eq!(h.state_mut().focused(), Some(Pane::Model));
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.editor.path.is_empty());
        assert_eq!(s.selected.as_deref(), Some("land"));
        assert!(s.editor.chosen.contains("land"));
    }
    assert!(centred(&h, "land"));
    // a layer of a node inside a loop: the loop opened, the node selected, the camera on it
    show(&mut h, "land", "raster");
    drawn(&mut h);
    menu(&mut h, "lp.inner.raster");
    h.get_by_label("Show its node").click();
    h.run_steps(40);
    assert_eq!(h.state_mut().focused(), Some(Pane::Model));
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.editor.path, ["lp"]);
        assert_eq!(s.selected.as_deref(), Some("inner"));
        assert!(s.editor.chosen.contains("inner"));
    }
    assert!(centred(&h, "inner"));
    // a layer no node makes (the Viewer keeps layers of the nodes at the top only, so a node removed takes its layers
    // with it; `of_layer` has its unit test): the entry greyed out, and nothing happens
    show(&mut h, "land", "raster");
    drawn(&mut h);
    menu(&mut h, "drivers to land");
    let entry = h.get_by_label("Show its node");
    assert!(entry.accesskit_node().is_disabled(), "greyed out");
    entry.click();
    h.run_steps(2);
    assert_eq!(h.state_mut().focused(), Some(Pane::Viewer), "the Viewer stays in front");
}

/// A layer's menu *Same colours for all similar layers*: its palette and range given to the other maps of values;
/// a map of classes is left as it is; greyed out when no other layer is of its kind.
#[test]
fn a_layers_colours_are_given_to_all_similar_layers() {
    use egui_kittest::kittest::NodeT;
    use terraflux_app::builder::{Layer, Palette};
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let drivers = data("sibuyan_drivers.tif").to_string_lossy().into_owned();
        s.source.nodes = vec![
            Node::new("land", "raster.read")
                .param("path", data("sibuyan_landuse.tif").to_string_lossy().as_ref())
                .param("band", "0"),
            Node::new("a", "raster.read").param("path", drivers.as_str()).param("band", "0"),
            Node::new("b", "raster.read").param("path", drivers.as_str()).param("band", "1"),
            Node::new("c", "raster.read").param("path", drivers.as_str()).param("band", "1"),
        ];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "a", "b", "c"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    let key = |n: &str| (n.to_string(), "raster".to_string());
    let terrain = Palette::Chosen(terraflux_app::palette::Ramp::Terrain, false);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_layers = vec![Layer::new("land", "raster"), Layer::new("b", "raster"), Layer::new("c", "raster")];
        s.palettes.insert(key("a"), terrain);
        s.ranges.insert(key("a"), (10.0, 20.0));
    }
    show(&mut h, "a", "raster");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_layers.push(Layer::new("a", "raster"));
    }
    drawn(&mut h);
    let menu = |h: &mut Harness<TerrafluxApp>, name: &str| {
        let width = |n: &egui_kittest::Node| n.accesskit_node().bounding_box().map_or(0.0, |b| b.x1 - b.x0);
        let n = h.query_all_by_label(name).max_by(|a, b| width(a).total_cmp(&width(b)));
        let b = n.unwrap_or_else(|| panic!("{name}")).accesskit_node().bounding_box().unwrap();
        let at = egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32);
        h.event(egui::Event::PointerMoved(at));
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Secondary,
                pressed,
                modifiers: Default::default(),
            });
        }
        h.run_steps(2);
    };
    menu(&mut h, "a");
    let entry = h.get_by_label("Same colours for all similar layers");
    assert!(!entry.accesskit_node().is_disabled());
    entry.click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    for n in ["b", "c"] {
        assert_eq!(s.palettes.get(&key(n)), Some(&terrain), "{n}");
        assert_eq!(s.ranges.get(&key(n)), Some(&(10.0, 20.0)), "{n}");
    }
    assert_eq!(s.palettes.get(&key("land")), None, "the classes are left as they are");
    assert_eq!(s.ranges.get(&key("land")), None);
    // a single layer: nothing to give the colours to
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_layers.retain(|l| l.node == "a");
    }
    drawn(&mut h);
    menu(&mut h, "a");
    let entry = h.get_by_label("Same colours for all similar layers");
    assert!(entry.accesskit_node().is_disabled(), "greyed out");
}

/// A layer's band and palette kept in a Plot, and given back when it opens; the Document's colours by the Plot.
#[test]
fn a_layers_band_and_palette_are_kept_in_its_plot() {
    use terraflux_app::builder::{Layer, Palette, palette_of};
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes = vec![
            Node::new("land", "raster.read")
                .param("path", data("sibuyan_landuse.tif").to_string_lossy().as_ref())
                .param("band", "0"),
            Node::new("drivers", "raster.read").param("path", data("sibuyan_drivers.tif").to_string_lossy().as_ref()),
        ];
        s.edited();
        assert!(s.issues.is_empty(), "{:?}", s.issues);
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("land", "raster").is_some() && s.value_of("drivers", "raster").is_some()
    });
    let terrain = Palette::Chosen(terraflux_app::palette::Ramp::Terrain, false);
    let third = {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_layers = vec![Layer { band: Some(2), ..Layer::new("drivers", "raster") }];
        s.palettes.insert(("drivers".into(), "raster".into()), terrain);
        let Some(terraflux_ops::Value::Raster(r)) = s.value_of("drivers", "raster") else { panic!("rasters") };
        r.layers[2].name.clone()
    };
    show(&mut h, "land", "raster");
    drawn(&mut h);
    h.run_steps(3);
    // the layer's map, chosen in the Layers: its third band
    assert!(h.query_by_value(&format!("map: {third}")).is_some(), "the band of the layer");
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let s = h.state_mut().session.as_mut().unwrap();
    let p = s.source.plot("plot_land").expect("the Plot").clone();
    let drivers = p.layers.iter().find(|l| l.from == "drivers.raster").expect("the drivers layer");
    assert_eq!((drivers.band, drivers.palette.as_deref()), (Some(2), Some("terrain")));
    assert_eq!(p.layers.last().unwrap().palette, None, "the land by what it is");
    // the Viewer cleared: the Document's colours by the Plot, and opening it gives them back
    s.preview_layers.clear();
    s.palettes.clear();
    assert_eq!(palette_of(s, "drivers", "raster"), terrain);
    terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), "plot_land");
    assert_eq!(s.preview_layers[0].band, Some(2));
    assert_eq!(s.palettes.get(&("drivers".to_string(), "raster".to_string())), Some(&terrain));
    // the layer shown: its band is the Viewer's
    s.preview = Some(("drivers".into(), "raster".into()));
    s.preview_band = 4;
    s.preview_chosen = true;
    drawn(&mut h);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.preview_layers[0].band, Some(4));
    // the Document lists the Plots, each opened in the Viewer a click away
    h.state_mut().session.as_mut().unwrap().plot_open = None;
    h.state_mut().focus(Pane::Document);
    h.run_steps(3);
    assert!(h.query_by_label("Figure 1").is_some(), "the Plot's figure");
    h.get_by_label("open").click();
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().plot_open.as_deref(), Some("plot_land"));
}

/// A stack of three bands shown as colour: the box "RGB" of its layer takes the natural colour of the bands' names
/// (B4, B3, B2), the legend tells each channel's band and range, the picture of the Viewer (its export) has the
/// colours the ranges give, the Plot keeps it and opens again with it, and `plot_export` draws it the same.
#[test]
fn a_stack_shown_as_rgb() {
    use terraflux_app::builder::Layer;
    use terraflux_app::export_view::{Format, save};
    use terraflux_app::rgb::{Rgb, channel};
    use terraflux_graph::Node;
    // bands of one random map: B4 twice it, B3 itself, B2 its complement (the names order the stack: B2, B3, B4)
    let mut m = terraflux_graph::Model::new("rgb");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 6).param("columns", 8).param("seed", 1));
    for (id, e) in [("red", "land * 2"), ("green", "land"), ("blue", "1 - land")] {
        m.nodes.push(Node::new(id, "raster.calc").param("expression", e).input("land", "land.map"));
    }
    m.nodes
        .push(Node::new("scene", "stack.of").input("B2", "blue.map").input("B3", "green.map").input("B4", "red.map"));
    let path = tmp("ui-rgb.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("scene", "stack").is_some()
    });
    h.state_mut().session.as_mut().unwrap().preview_layers = vec![Layer::new("scene", "stack")];
    show(&mut h, "scene", "stack");
    drawn(&mut h);
    let wait = |h: &mut Harness<TerrafluxApp>, text: &str| {
        let t = Instant::now();
        while h.query_by_label_contains(text).is_none() {
            h.step();
            std::thread::sleep(Duration::from_millis(10));
            assert!(t.elapsed() < Duration::from_secs(120), "{text}: not shown");
        }
    };
    // the box: the natural colour of the names (the red channel is B4, the third map)
    assert!(h.query_by_label("RGB").is_some(), "the box of a stack of three maps");
    h.get_by_label("RGB").click();
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().preview_layers[0].rgb.as_ref().unwrap().bands, [2, 1, 0]);
    // (the ranges by the percentiles come from a thread: the legend when they are there)
    wait(&mut h, "R B4 ");
    wait(&mut h, "B B2 ");
    assert!(h.query_by_label_contains("G B3 ").is_some());
    // ranges chosen by hand, the gamma 1: the legend tells them
    let ranges = [Some((0.0, 2.0)), Some((0.0, 1.0)), Some((0.25, 0.75))];
    let rgb = Rgb { bands: [2, 1, 0], ranges, gamma: 1.0 };
    h.state_mut().session.as_mut().unwrap().preview_layers[0].rgb = Some(rgb.clone());
    wait(&mut h, "R B4 0–2");
    assert!(h.query_by_label("G B3 0–1").is_some() && h.query_by_label("B B2 0.25–0.75").is_some());
    drawn(&mut h);
    // the picture the Viewer shows: its export, the cell at column 3, row 2 in the colours of the formula
    for _ in 0..400 {
        if h.query_by_label("Export the view...").is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
        h.run_steps(1);
    }
    h.get_by_label("Export the view...").click();
    h.run_steps(4);
    let (col, row) = (3usize, 2usize);
    let expected = {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Stack(st)) = s.value_of("scene", "stack") else { panic!("a stack") };
        let at = row * st.grid.width + col;
        // (the stack's order: B2, B3, B4; the channels R, G, B are B4, B3, B2)
        let v = |band: usize| st.layers[band][at];
        let c = |i: usize| channel(v(rgb.bands[i]), ranges[i].unwrap().0, ranges[i].unwrap().1, 1.0).unwrap();
        [c(0), c(1), c(2)]
    };
    let pixel = |p: &std::path::Path| {
        let pm = tiny_skia::Pixmap::load_png(p).unwrap();
        let c = pm.pixel(col as u32, row as u32).unwrap();
        [c.red(), c.green(), c.blue(), c.alpha()]
    };
    let png = tmp("rgb-view.png");
    {
        let e = h.state().session.as_ref().unwrap().export.as_ref().expect("the export's window");
        assert!(matches!(e.panel.coloring, terraflux_app::palette::Coloring::Packed), "drawn as colour");
        save(&e.panel, 1, Format::Png, &png).unwrap();
    }
    assert_eq!(pixel(&png), [expected[0], expected[1], expected[2], 255], "a cell of the picture");
    // the Plot keeps it, and opening it gives it back
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let s = h.state_mut().session.as_mut().unwrap();
    let id = s.source.plots.last().expect("the Plot").id.clone();
    let kept = s.source.plot(&id).unwrap().layers.iter().find(|l| l.from == "scene.stack").unwrap().rgb.clone();
    let kept = kept.expect("the composite kept in the Plot");
    assert_eq!((kept.bands, kept.red, kept.blue, kept.gamma), ([2, 1, 0], Some([0.0, 2.0]), Some([0.25, 0.75]), None));
    // (written to the model's file and read back: the plot's `rgb` is a table of its layer)
    let text = s.source.to_toml().unwrap();
    assert!(text.contains("rgb"), "{text}");
    assert_eq!(terraflux_graph::Model::from_toml(&text).unwrap().plots, s.source.plots);
    s.preview_layers.clear();
    terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), &id);
    let back = s.preview_layers.iter().find(|l| l.node == "scene").expect("the layer");
    assert_eq!(back.rgb.as_ref(), Some(&rgb));
    // the CLI's and the MCP tool's picture of the Plot: the same colours
    let made = {
        let s = h.state().session.as_ref().unwrap();
        let value = |from: &str| {
            let (n, p) = from.split_once('.')?;
            s.value_of(n, p)
        };
        terraflux_app::plot_export::panel_of(&s.source, &id, &value).unwrap()
    };
    assert!(matches!(made.panel.coloring, terraflux_app::palette::Coloring::Packed));
    let png2 = tmp("rgb-plot.png");
    save(&made.panel, 1, Format::Png, &png2).unwrap();
    assert_eq!(pixel(&png2), pixel(&png), "the plot's picture is the Viewer's");
    shot(&mut h, "14r-rgb");
}

/// The Viewer's picture of the map shown with its layers (the button "Export the view..."): the pixel at `at`.
fn view_pixel(h: &mut Harness<TerrafluxApp>, at: (u32, u32)) -> [u8; 4] {
    for _ in 0..400 {
        if h.query_by_label("Export the view...").is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
        h.run_steps(1);
    }
    h.get_by_label("Export the view...").click();
    h.run_steps(4);
    let e = h.state().session.as_ref().unwrap().export.as_ref().expect("the export's window");
    let pm = terraflux_app::export_view::picture(&e.panel, 1).unwrap();
    let c = pm.pixel(at.0, at.1).unwrap();
    [c.red(), c.green(), c.blue(), c.alpha()]
}

/// An elevation shaded with its own relief: the box "Shade with this elevation" on its layer (the sun at 315 and 45
/// degrees, the exaggeration 1, 0.6 strong), a legend line that says so, the picture of the Viewer with each cell's
/// colour times `1 - strength + strength * shade` (the shade made of the whole map, so the tiles join); the Plot
/// keeps it and opens again with it, and `plot_export` draws the same.
#[test]
fn a_map_shaded_with_its_own_relief() {
    use terraflux_app::builder::Layer;
    use terraflux_app::export_view::{Format, save};
    use terraflux_app::shade::{Shade, cell_metres, shade_all};
    use terraflux_graph::Node;
    // a terrain of the midpoint displacement
    let mut m = terraflux_graph::Model::new("relief");
    m.nodes.push(Node::new("dem", "nlm.mpd").param("rows", 40).param("columns", 56).param("seed", 3));
    let path = tmp("ui-relief.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("dem", "map").is_some()
    });
    h.state_mut().session.as_mut().unwrap().preview_layers = vec![Layer::new("dem", "map")];
    show(&mut h, "dem", "map");
    drawn(&mut h);
    // the box of the layer of values; the sun's controls come with it
    assert!(h.query_by_label("Shade with this elevation").is_some(), "the box of a map of values");
    assert!(h.state().session.as_ref().unwrap().preview_layers[0].shade.is_none());
    h.get_by_label("Shade with this elevation").click();
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().preview_layers[0].shade, Some(Shade::default()));
    drawn(&mut h);
    for text in ["azimuth", "sun altitude", "exaggeration", "strength"] {
        assert!(h.query_by_label_contains(text).is_some(), "{text}");
    }
    // the legend says where the sun is
    assert!(h.query_by_label("shaded: 315°, 45°, ×1").is_some(), "the legend line");
    // the sun and the strength by hand
    let shade = Shade { azimuth: 250.0, altitude: 35.0, z: 2.0, strength: 0.7 };
    h.state_mut().session.as_mut().unwrap().preview_layers[0].shade = Some(shade);
    drawn(&mut h);
    assert!(h.query_by_label("shaded: 250°, 35°, ×2").is_some(), "the legend line follows the sun");
    // the picture of the Viewer: a cell's colour times the shade worked out from the whole map
    let (col, row) = (30usize, 17usize);
    let (base, bright) = {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(dem)) = s.value_of("dem", "map") else { panic!("a map") };
        let all: Vec<f32> = dem.data.iter().map(|v| *v as f32).collect();
        let (w, ht) = (dem.grid.width, dem.grid.height);
        let b = shade_all(&all, w, ht, &shade.sun(), cell_metres(&dem.grid));
        (dem.data[row * w + col], b[row * w + col])
    };
    let pixel = view_pixel(&mut h, (col as u32, row as u32));
    let png = tmp("relief-view.png");
    let colour = {
        let e = h.state().session.as_ref().unwrap().export.as_ref().unwrap();
        assert_eq!(e.panel.rasters.len(), 1, "the shade is a layer over the map");
        assert!(matches!(e.panel.rasters[0].source, terraflux_app::map_view::Source::Shade(_)));
        save(&e.panel, 1, Format::Png, &png).unwrap();
        e.panel.coloring.colour(base)
    };
    let k = 1.0 - shade.strength + shade.strength * bright;
    let want = |c: u8| (f32::from(c) * k).round();
    for (got, c) in [(pixel[0], colour.r()), (pixel[1], colour.g()), (pixel[2], colour.b())] {
        assert!((f32::from(got) - want(c)).abs() <= 1.0, "{pixel:?} vs {colour:?} x {k}");
    }
    assert_eq!(pixel[3], 255);
    // the Plot keeps it, and opening it gives it back
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let s = h.state_mut().session.as_mut().unwrap();
    let id = s.source.plots.last().expect("the Plot").id.clone();
    let kept = s.source.plot(&id).unwrap().layers.iter().find(|l| l.from == "dem.map").unwrap().shade.clone();
    let kept = kept.expect("the relief kept in the Plot");
    assert_eq!(Shade::of_plot(&kept), shade);
    let text = s.source.to_toml().unwrap();
    assert!(text.contains("shade") && text.contains("azimuth"), "{text}");
    assert_eq!(terraflux_graph::Model::from_toml(&text).unwrap().plots, s.source.plots);
    s.preview_layers.clear();
    terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), &id);
    let back = s.preview_layers.iter().find(|l| l.node == "dem").expect("the layer");
    assert_eq!(back.shade, Some(shade));
    // the CLI's and the MCP tool's picture of the Plot: the same colours
    let made = {
        let s = h.state().session.as_ref().unwrap();
        let value = |from: &str| {
            let (n, p) = from.split_once('.')?;
            s.value_of(n, p)
        };
        terraflux_app::plot_export::panel_of(&s.source, &id, &value).unwrap()
    };
    let png2 = tmp("relief-plot.png");
    save(&made.panel, 1, Format::Png, &png2).unwrap();
    let at = |p: &std::path::Path| {
        let c = tiny_skia::Pixmap::load_png(p).unwrap().pixel(col as u32, row as u32).unwrap();
        [c.red(), c.green(), c.blue()]
    };
    assert_eq!(at(&png2), at(&png), "the plot's picture is the Viewer's");
    shot(&mut h, "14s-relief");
}

/// A layer set to multiply over another darkens it (each channel the product of the two), screen lightens it, in
/// the picture of the Viewer and in the live view as the GPU draws it; the Plot keeps the mode (`blend = "multiply"`)
/// and opens again with it.
#[test]
fn a_layer_blended_with_the_one_under_it() {
    use terraflux_app::blend::Blend;
    use terraflux_app::builder::Layer;
    use terraflux_graph::Node;
    // two maps of one class each: the same colour, so that a mix of it with itself is told at once
    let mut m = terraflux_graph::Model::new("blend");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 12).param("columns", 16).param("seed", 1));
    m.nodes.push(Node::new("under", "raster.calc").param("expression", "land * 0 + 1").input("land", "land.map"));
    m.nodes.push(Node::new("top", "raster.calc").param("expression", "land * 0 + 1").input("land", "land.map"));
    let path = tmp("ui-blend.toml");
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("top", "map").is_some() && s.value_of("under", "map").is_some()
    });
    let set = |h: &mut Harness<TerrafluxApp>, blend: Blend| {
        h.state_mut().session.as_mut().unwrap().preview_layers =
            vec![Layer::new("under", "map"), Layer { blend, ..Layer::new("top", "map") }];
    };
    set(&mut h, Blend::Normal);
    show(&mut h, "under", "map");
    drawn(&mut h);
    let normal = view_pixel(&mut h, (5, 5));
    assert_eq!(normal[3], 255);
    let c = [normal[0], normal[1], normal[2]];
    let product = |a: [u8; 3], b: [u8; 3]| [0, 1, 2].map(|i| (f32::from(a[i]) * f32::from(b[i]) / 255.0).round() as u8);
    let screen = |a: [u8; 3], b: [u8; 3]| {
        [0, 1, 2].map(|i| {
            let (x, y) = (f32::from(a[i]) / 255.0, f32::from(b[i]) / 255.0);
            ((x + y - x * y) * 255.0).round() as u8
        })
    };
    let near = |a: [u8; 3], b: [u8; 3], by: i32| (0..3).all(|i| (i32::from(a[i]) - i32::from(b[i])).abs() <= by);
    // multiplied: the colour times itself, darker than it was
    set(&mut h, Blend::Multiply);
    drawn(&mut h);
    let multiplied = view_pixel(&mut h, (5, 5));
    assert!(near([multiplied[0], multiplied[1], multiplied[2]], product(c, c), 1), "{multiplied:?} vs {c:?}");
    assert!(multiplied[0] < normal[0] || multiplied[1] < normal[1] || multiplied[2] < normal[2]);
    // the live view: the pixels the GPU drew (the colour times itself somewhere on the window)
    let live = |h: &mut Harness<TerrafluxApp>, want: [u8; 3]| -> Option<bool> {
        drawn(h);
        let img = h.render().ok()?;
        Some(img.pixels().any(|p| near([p[0], p[1], p[2]], want, 2)))
    };
    match live(&mut h, product(c, c)) {
        Some(found) => assert!(found, "the multiplied colour {:?} is on the screen", product(c, c)),
        None => eprintln!("no renderer: the live view is not looked at"),
    }
    // screened: lighter
    set(&mut h, Blend::Screen);
    drawn(&mut h);
    let screened = view_pixel(&mut h, (5, 5));
    assert!(near([screened[0], screened[1], screened[2]], screen(c, c), 1), "{screened:?} vs {c:?}");
    match live(&mut h, screen(c, c)) {
        Some(found) => assert!(found, "the screened colour {:?} is on the screen", screen(c, c)),
        None => eprintln!("no renderer: the live view is not looked at"),
    }
    // the Plot keeps the mode, and opening it gives it back
    set(&mut h, Blend::Multiply);
    drawn(&mut h);
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let s = h.state_mut().session.as_mut().unwrap();
    let id = s.source.plots.last().expect("the Plot").id.clone();
    let layers = s.source.plot(&id).unwrap().layers.clone();
    assert_eq!(layers.iter().find(|l| l.from == "top.map").unwrap().blend.as_deref(), Some("multiply"));
    assert!(layers.iter().find(|l| l.from == "under.map").unwrap().blend.is_none());
    let text = s.source.to_toml().unwrap();
    assert!(text.contains("blend = \"multiply\""), "{text}");
    assert_eq!(terraflux_graph::Model::from_toml(&text).unwrap().plots, s.source.plots);
    s.preview_layers.clear();
    terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), &id);
    let back = s.preview_layers.iter().find(|l| l.node == "top").expect("the layer");
    assert_eq!(back.blend, Blend::Multiply);
    // the CLI's and the MCP tool's picture of the Plot: the same colours
    let made = {
        let s = h.state().session.as_ref().unwrap();
        let value = |from: &str| {
            let (n, p) = from.split_once('.')?;
            s.value_of(n, p)
        };
        terraflux_app::plot_export::panel_of(&s.source, &id, &value).unwrap()
    };
    let png = tmp("blend-plot.png");
    terraflux_app::export_view::save(&made.panel, 1, terraflux_app::export_view::Format::Png, &png).unwrap();
    let p = tiny_skia::Pixmap::load_png(&png).unwrap().pixel(5, 5).unwrap();
    assert!(near([p.red(), p.green(), p.blue()], product(c, c), 1), "the plot's picture is multiplied too");
    shot(&mut h, "14b-blend");
}

/// A Plot's saved view: kept from what the Viewer has in view, left alone by Update the plot, brought back when the
/// Plot is opened, and forgotten.
#[test]
fn a_plot_keeps_the_frame_of_its_map() {
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes = vec![
            Node::new("land", "raster.read").param("path", data("sibuyan_landuse.tif").to_string_lossy().as_ref()),
        ];
        s.source.plots = vec![terraflux_graph::Plot {
            id: "framed".into(),
            layers: vec![terraflux_graph::PlotLayer::new("land.raster")],
            ..Default::default()
        }];
        s.edited();
        assert!(s.issues.is_empty(), "{:?}", s.issues);
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("land", "raster").is_some()
    });
    open_plot(&mut h, "framed");
    drawn(&mut h);
    h.run_steps(3);
    // a part of the map in view
    let (w, ht) = {
        let s = h.state().session.as_ref().unwrap();
        let g = s.preview_view.shown_grid().expect("the map is drawn");
        (g.width as f32, g.height as f32)
    };
    let part = [w * 0.25, ht * 0.25, w * 0.5, ht * 0.5];
    h.state_mut().session.as_mut().unwrap().preview_view.zoom_to(part);
    h.run_steps(3);
    let seen =
        |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().preview_view.visible().map(|(_, b)| b).unwrap();
    let before = seen(&h);
    assert!(before[2] - before[0] < w * 0.6, "zoomed in: {before:?}");
    // kept in the Plot
    h.get_by_label("Plot: framed").click();
    h.run_steps(2);
    h.get_by_label("Keep this view in the plot").click();
    h.run_steps(2);
    let saved = {
        let s = h.state().session.as_ref().unwrap();
        s.source.plot("framed").unwrap().view.clone().expect("the saved view")
    };
    let [x0, y0, x1, y1] = saved.extent;
    assert!(x1 > x0 && y1 > y0, "{saved:?}");
    // the map fitted again, the Plot opened: the view is back on the frame
    h.state_mut().session.as_mut().unwrap().preview_view.fit();
    h.run_steps(3);
    assert!(seen(&h)[2] - seen(&h)[0] > w * 0.9, "the whole map");
    open_plot(&mut h, "framed");
    h.run_steps(4);
    let after = seen(&h);
    for (a, b) in after.iter().zip(&before) {
        assert!((a - b).abs() <= 1.5, "back on the frame: {after:?} was {before:?}");
    }
    // Update the plot leaves the frame as it is
    h.get_by_label("Plot: framed").click();
    h.run_steps(2);
    h.get_by_label("Update the plot").click();
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().source.plot("framed").unwrap().view, Some(saved));
    // forgotten
    h.get_by_label("Plot: framed").click();
    h.run_steps(2);
    h.get_by_label("Forget the saved view").click();
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().source.plot("framed").unwrap().view.is_none());
}

/// The features selected of a vector layer stay selected when its table is closed: *Update the plot* keeps them in the
/// Plot, a layer's table opens with them again.
#[test]
fn the_selection_outlives_the_table() {
    use terraflux_graph::{Node, Plot, PlotLayer};
    use terraflux_raster::vector::{Column, Geometry, Vector};
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let g = terraflux_gdal::Dataset::open(data("sibuyan_landuse.tif")).unwrap().meta.grid.clone();
    let t = g.transform;
    let (w, ht) = (g.width as f64 * t[1], g.height as f64 * -t[5]);
    let square = |a: f64, b: f64| {
        let p = [t[0] + w * a, t[3] - ht * b];
        let q = [t[0] + w * (a + 0.2), t[3] - ht * (b + 0.2)];
        Geometry::Polygon(vec![vec![p, [q[0], p[1]], q, [p[0], q[1]], p]])
    };
    let v = Vector {
        geometries: vec![square(0.1, 0.1), square(0.4, 0.4), square(0.7, 0.7)],
        attributes: vec![("area".into(), Column::Numbers(vec![1.0, 2.0, 3.0]))],
        crs: g.geo.wkt.clone(),
    };
    let gpkg = tmp("kept-selection.gpkg");
    let _ = std::fs::remove_file(&gpkg);
    terraflux_gdal::write_vector(&gpkg, "features", &v).unwrap();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes = vec![
            Node::new("land", "raster.read").param("path", data("sibuyan_landuse.tif").to_string_lossy().as_ref()),
            Node::new("feat", "vector.read").param("path", gpkg.to_string_lossy().as_ref()),
        ];
        s.source.plots = vec![Plot {
            id: "sel".into(),
            layers: vec![
                PlotLayer::new("land.raster"),
                PlotLayer { selected: vec![1, 3], ..PlotLayer::new("feat.vector") },
            ],
            ..Default::default()
        }];
        s.edited();
        assert!(s.issues.is_empty(), "{:?}", s.issues);
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("land", "raster").is_some() && s.value_of("feat", "vector").is_some()
    });
    open_plot(&mut h, "sel");
    drawn(&mut h);
    h.run_steps(3);
    let rows = |h: &Harness<TerrafluxApp>| -> Option<Vec<usize>> {
        let o = h.state().session.as_ref().unwrap().preview_attributes.clone()?;
        Some(o.pick.rows.iter().copied().collect())
    };
    assert_eq!(rows(&h), Some(vec![0, 2]), "the table opens with the selection");
    // closed: the selection is not lost
    h.get_by_label("x").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().preview_attributes.is_none(), "the table is closed");
    assert!(h.query_by_label("Attributes of feat.vector").is_none());
    // (what the Plot had is taken away, to see Update the plot write it back from what is kept)
    h.state_mut().session.as_mut().unwrap().source.plots[0].layers[1].selected.clear();
    h.get_by_label("Plot: sel").click();
    h.run_steps(2);
    h.get_by_label("Update the plot").click();
    h.run_steps(2);
    let p = h.state().session.as_ref().unwrap().source.plot("sel").unwrap().clone();
    assert!(p.layers[0].selected.is_empty());
    assert_eq!(p.layers[1].selected, [1, 3], "the selection is in the Plot, the table closed");
    // (kept in the session for the table to show again: `Shown::open_table` is tested in the builder)
    let s = h.state().session.as_ref().unwrap();
    let kept = s.preview_kept.get(&("feat".to_string(), "vector".to_string())).cloned();
    assert_eq!(kept.map(|k| k.into_iter().collect::<Vec<_>>()), Some(vec![0, 2]));
}

/// A table plotted (in the Viewer, as the chart its settings ask) and written to a file.
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
            Node::new("write", "table.write")
                .input("table", "areas.table")
                .param("path", out.to_string_lossy().as_ref())
                .param("separator", ";")
                .param("decimal", ","),
        ];
        s.source.plots = vec![terraflux_graph::Plot {
            id: "plot".into(),
            layers: vec![terraflux_graph::PlotLayer::new("areas.table")],
            chart: Some(terraflux_graph::PlotChart {
                kind: Some("lines".into()),
                y: Some("forest, pasture".into()),
                ..Default::default()
            }),
            ..Default::default()
        }];
        s.edited();
        assert!(s.issues.is_empty(), "{:?}", s.issues);
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the plot", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && plotted(s, "plot").is_some()
    });
    // (in the Viewer)
    open_plot(&mut h, "plot");
    drawn(&mut h);
    h.run_steps(4);
    shot(&mut h, "31-table-plot");
    // the chart the plot asks for: lines of forest and pasture along the year
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Table(t)) = plotted(s, "plot") else { panic!("{:?}", s.error) };
    let chart = terraflux_app::builder::plot_chart(s.source.plot("plot").unwrap().chart.as_ref().unwrap(), &t);
    assert_eq!(chart.kind, terraflux_app::charts::Kind::Lines);
    assert_eq!((chart.columns.as_slice(), chart.x), ([1, 2].as_slice(), Some(0)));
    // another chart chosen in the Viewer, kept by Update the plot
    h.get_by_value("chart: lines").click();
    h.run_steps(2);
    h.get_by_label("bars").click();
    h.run_steps(2);
    h.get_by_label("Plot: plot").click();
    h.run_steps(2);
    h.get_by_label("Update the plot").click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    let kept = s.source.plot("plot").unwrap().chart.clone().unwrap();
    assert_eq!(kept.kind.as_deref(), Some("bars"));
    let chart = terraflux_app::builder::plot_chart(&kept, &t);
    assert_eq!(chart.kind, terraflux_app::charts::Kind::Bars);
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
        // the batch is one entry of the model's journal
        let file = h.state().session.as_ref().unwrap().path.clone().unwrap();
        let last = terraflux_exec::journal::Journal::of(&file).unwrap().entries().pop().unwrap();
        let terraflux_exec::journal::Kind::Batch(j) = &last.kind else { panic!("{:?}", last.kind) };
        assert_eq!((j.kind.as_str(), j.runs, j.failed, last.from.as_str()), ("replicates", 3, 0, "app"));
        assert!(j.out.ends_with("replicates"));
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
fn a_sensitivity_analysis_runs_from_its_pane() {
    use terraflux_app::batch_view::{Kind, Range};
    use terraflux_dist::sensitivity::Method;
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    {
        // one step a run, for the test
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(1))].into()).unwrap();
        s.edited();
    }
    h.state_mut().focus(Pane::Batch);
    let out = tmp("ui-sensitivity");
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_dir_all(tmp("ui-sensitivity-pool"));
    h.state_mut().computers.pool = None;
    h.state_mut().computers.dir = Some(tmp("ui-sensitivity-pool"));
    {
        let b = &mut h.state_mut().batch;
        b.kind = Kind::Sensitivity;
        b.workers = 2;
        b.out = Some(out.clone());
        b.sens.method = Method::Oat;
        b.sens.output = Some("simulation.maps".into());
        b.sens.factors = ["patches.isometry:0.5:2.5", "simulation/allocate.prune_factor:5:15"]
            .iter()
            .map(|f| {
                let p: Vec<&str> = f.splitn(2, ':').collect();
                let (from, to) = p[1].rsplit_once(':').unwrap();
                Range { key: p[0].into(), from: from.into(), to: to.into(), points: String::new() }
            })
            .collect();
    }
    h.run_steps(3);
    shot(&mut h, "batch-6-sensitivity");
    h.get_by_label("Run 5 times").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the sensitivity analysis", |a| !a.batch.running());
    h.run_steps(2);
    let b = &h.state().batch;
    assert!(b.error.is_none(), "{:?}", b.error);
    let f = b.finished.as_ref().unwrap();
    assert_eq!(f.kind, Kind::Sensitivity, "{:?} {:?}", f.errors, f.rows);
    assert_eq!(f.rows.len(), 5);
    let i = f.sensitivity.as_ref().unwrap();
    assert_eq!(i.factors, ["patches.isometry", "simulation/allocate.prune_factor"]);
    assert_eq!(i.rows.len(), 2);
    assert_eq!(i.columns, ["low", "high", "swing"]);
    assert!(i.base.is_some_and(f64::is_finite));
    let indices = std::fs::read_to_string(out.join("indices.csv")).unwrap();
    assert!(indices.starts_with("factor,low,high,swing,base\npatches.isometry,"), "{indices}");
    let design = std::fs::read_to_string(out.join("design.csv")).unwrap();
    assert_eq!(design.lines().count(), 6, "{design}");
    // the chart, as files (OAT has no convergence check)
    let svg = std::fs::read_to_string(out.join("sensitivity.svg")).unwrap();
    assert!(svg.contains("<svg") && svg.contains("patches.isometry"), "{svg}");
    assert!(out.join("sensitivity.png").metadata().unwrap().len() > 500);
    assert!(i.convergence.is_none() && !out.join("convergence.csv").exists());
    // the methods paragraph, as a file and as a button to copy it
    let methods = std::fs::read_to_string(out.join("methods.md")).unwrap();
    assert!(methods.contains("one-at-a-time") && methods.contains("the mean of simulation.maps"), "{methods}");
    assert!(methods.contains("patches.isometry [0.5\u{2013}2.5]") && methods.contains("## References"), "{methods}");
    assert_eq!(f.methods.as_deref(), Some(methods.as_str()));
    h.get_by_label("Copy the methods text");
    shot(&mut h, "batch-7-sensitivity-done");

    // the share of a land use class of the simulated maps: a number in 0..1 in every run
    let r = terraflux_raster::Raster::read(data("sibuyan_landuse.tif")).unwrap();
    let class = r.bands[0].masked(r.nodata).iter().copied().find(|x| !x.is_nan()).unwrap();
    let out = tmp("ui-sensitivity-share");
    let _ = std::fs::remove_dir_all(&out);
    {
        let b = &mut h.state_mut().batch;
        b.out = Some(out.clone());
        b.sens.factors.truncate(1);
        b.sens.statistic = "share".into();
        b.sens.class = format!("{class}");
    }
    h.run_steps(3);
    h.get_by_label("Run 3 times").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the share of a class", |a| !a.batch.running());
    h.run_steps(2);
    let b = &h.state().batch;
    assert!(b.error.is_none(), "{:?}", b.error);
    let f = b.finished.as_ref().unwrap();
    assert_eq!(f.kind, Kind::Sensitivity, "{:?} {:?}", f.errors, f.rows);
    assert!(f.sensitivity.as_ref().unwrap().base.is_some_and(|v| (0.0..=1.0).contains(&v)));
    let design = std::fs::read_to_string(out.join("design.csv")).unwrap();
    let values: Vec<f64> = design
        .lines()
        .skip(1)
        .map(|l| l.rsplit(',').next().unwrap().parse().unwrap_or_else(|_| panic!("{design}")))
        .collect();
    assert_eq!(values.len(), 3, "{design}");
    assert!(values.iter().all(|v| (0.0..=1.0).contains(v)), "{design}");
    assert!(values.iter().any(|v| *v > 0.0), "{design}");
}

#[test]
fn a_sensitivity_analysis_runs_replicates_at_each_point() {
    use terraflux_app::batch_view::{Kind, Range};
    use terraflux_dist::sensitivity::Method;
    let r = terraflux_raster::Raster::read(data("sibuyan_landuse.tif")).unwrap();
    let class = r.bands[0].masked(r.nodata).iter().copied().find(|x| !x.is_nan()).unwrap();
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(1))].into()).unwrap();
        s.edited();
    }
    h.state_mut().focus(Pane::Batch);
    let out = tmp("ui-sensitivity-replicates");
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_dir_all(tmp("ui-sensitivity-replicates-pool"));
    h.state_mut().computers.pool = None;
    h.state_mut().computers.dir = Some(tmp("ui-sensitivity-replicates-pool"));
    {
        let b = &mut h.state_mut().batch;
        b.kind = Kind::Sensitivity;
        b.workers = 2;
        b.out = Some(out.clone());
        b.sens.method = Method::Oat;
        b.sens.output = Some("simulation.maps".into());
        b.sens.replicates = 2;
        // (the mean of the map would not change with the seed: the number of patches does)
        b.sens.statistic = "metric".into();
        b.sens.metric = "np".into();
        b.sens.class = format!("{class}");
        b.sens.factors =
            vec![Range { key: "patches.isometry".into(), from: "0.5".into(), to: "2.5".into(), points: String::new() }];
    }
    h.run_steps(3);
    assert!(h.query_by_label_contains("a simulation that draws at random").is_some());
    // 3 points of the design, 2 replicates each
    assert!(h.query_by_label_contains("One at a time of 1 factor, 2 replicates: ").is_some());
    h.get_by_label("Run 6 times").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the replicates of a sensitivity analysis", |a| !a.batch.running());
    h.run_steps(2);
    let b = &h.state().batch;
    assert!(b.error.is_none(), "{:?}", b.error);
    let f = b.finished.as_ref().unwrap();
    assert_eq!(f.rows.len(), 6, "{:?} {:?}", f.errors, f.rows);
    let i = f.sensitivity.as_ref().unwrap();
    assert_eq!(i.replicates, 2);
    assert!(i.chance.is_some_and(|c| (0.0..=1.0).contains(&c)), "{:?}", i.chance);
    let design = std::fs::read_to_string(out.join("design.csv")).unwrap();
    assert!(design.starts_with("run,patches.isometry,simulation.maps,sd,n\n"), "{design}");
    assert_eq!(design.lines().count(), 4, "{design}");
    assert!(out.join("sensitivity.svg").exists() && out.join("sensitivity.png").exists());
    h.get_by_label("Save the chart...").scroll_to_me();
    h.run_steps(30);
    shot(&mut h, "batch-9-oat-chart");
}

#[test]
fn a_morris_analysis_checks_its_convergence() {
    use terraflux_app::batch_view::{Kind, Range};
    use terraflux_dist::sensitivity::Method;
    let r = terraflux_raster::Raster::read(data("sibuyan_landuse.tif")).unwrap();
    let class = r.bands[0].masked(r.nodata).iter().copied().find(|x| !x.is_nan()).unwrap();
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(1))].into()).unwrap();
        s.edited();
    }
    h.state_mut().focus(Pane::Batch);
    let out = tmp("ui-sensitivity-morris");
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_dir_all(tmp("ui-sensitivity-morris-pool"));
    h.state_mut().computers.pool = None;
    h.state_mut().computers.dir = Some(tmp("ui-sensitivity-morris-pool"));
    {
        let b = &mut h.state_mut().batch;
        b.kind = Kind::Sensitivity;
        b.workers = 2;
        b.out = Some(out.clone());
        b.sens.method = Method::Morris;
        b.sens.design.n = 4;
        b.sens.output = Some("simulation.maps".into());
        // (the mean of the map would not change: the number of patches does)
        b.sens.statistic = "metric".into();
        b.sens.metric = "np".into();
        b.sens.class = format!("{class}");
        b.sens.factors = vec![
            Range { key: "patches.isometry".into(), from: "0.5".into(), to: "2.5".into(), points: String::new() },
            Range {
                key: "simulation/allocate.prune_factor".into(),
                from: "5".into(),
                to: "15".into(),
                points: String::new(),
            },
        ];
    }
    h.run_steps(3);
    // 4 trajectories of 3 runs
    h.get_by_label("Run 12 times").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the Morris analysis", |a| !a.batch.running());
    h.run_steps(2);
    let b = &h.state().batch;
    assert!(b.error.is_none(), "{:?}", b.error);
    let f = b.finished.as_ref().unwrap();
    assert_eq!(f.rows.len(), 12, "{:?} {:?}", f.errors, f.rows);
    let i = f.sensitivity.as_ref().unwrap();
    let c = i.convergence.as_ref().expect("the convergence of 4 trajectories");
    assert_eq!((c.n.clone(), c.by.as_str(), c.values.len()), (vec![2, 4], "mu_star", 2));
    let csv = std::fs::read_to_string(out.join("convergence.csv")).unwrap();
    assert!(csv.starts_with("factor,mu_star_n2,mu_star_n4\npatches.isometry,"), "{csv}");
    assert!(out.join("sensitivity.svg").exists() && out.join("sensitivity.png").exists());
    h.get_by_label("Save the chart...").scroll_to_me();
    h.run_steps(30);
    shot(&mut h, "batch-8-morris-done");
}

#[test]
fn a_sensitivity_analysis_makes_maps_of_sensitivity() {
    use terraflux_app::batch_view::{Kind, Range};
    use terraflux_dist::sensitivity::Method;
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(1))].into()).unwrap();
        s.edited();
    }
    h.state_mut().focus(Pane::Batch);
    let out = tmp("ui-sensitivity-maps");
    let _ = std::fs::remove_dir_all(&out);
    let _ = std::fs::remove_dir_all(tmp("ui-sensitivity-maps-pool"));
    h.state_mut().computers.pool = None;
    h.state_mut().computers.dir = Some(tmp("ui-sensitivity-maps-pool"));
    {
        let b = &mut h.state_mut().batch;
        b.kind = Kind::Sensitivity;
        b.workers = 2;
        b.out = Some(out.clone());
        b.sens.method = Method::Oat;
        b.sens.output = Some("simulation.maps".into());
        b.sens.factors = vec![
            Range { key: "patches.isometry".into(), from: "0.5".into(), to: "2.5".into(), points: String::new() },
            Range {
                key: "simulation/allocate.prune_factor".into(),
                from: "5".into(),
                to: "15".into(),
                points: String::new(),
            },
        ];
    }
    h.run_steps(3);
    // the choice is offered for a series, off; on, it says what the runs keep
    assert!(h.query_by_label("Maps of sensitivity, cell by cell").is_some());
    assert!(h.query_by_label("Keep each run's map").is_none() && !h.state().batch.sens.maps);
    h.get_by_label("Maps of sensitivity, cell by cell").click();
    h.run_steps(3);
    assert!(h.state().batch.sens.maps);
    assert!(h.query_by_label("Keep each run's map").is_some());
    assert!(h.query_by_label_contains("Each run keeps the last map of simulation.maps (5 maps").is_some());
    shot(&mut h, "batch-10-sensitivity-maps");
    h.get_by_label("Run 5 times").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the sensitivity analysis and its maps", |a| !a.batch.running() && a.batch.making.is_none());
    h.run_steps(3);
    let b = &h.state().batch;
    assert!(b.error.is_none(), "{:?}", b.error);
    let f = b.finished.as_ref().unwrap();
    assert!(f.sensitivity.is_some(), "{:?}", f.errors);
    // the indices are the numbers as before; the maps are the bands of one file
    let made = f.maps.as_ref().expect("the maps of sensitivity");
    assert_eq!(made.path, out.join("sensitivity_maps.tif"));
    assert_eq!(made.bands, ["mean", "sd", "swing_patches.isometry", "swing_simulation/allocate.prune_factor"]);
    assert_eq!((made.read, made.missing), (5, 0));
    let stack = terraflux_raster::Raster::read(&made.path).unwrap();
    assert_eq!(stack.band_names(), made.bands);
    assert!(stack.bands[0].data.iter().any(|v| v.is_finite()), "cells with a mean");
    // the sentence of the methods text, in the file and on screen
    let methods = std::fs::read_to_string(out.join("methods.md")).unwrap();
    assert!(
        methods.contains("Cell by cell, the standard deviation of the output across the design and the swing"),
        "{methods}"
    );
    assert_eq!(f.methods.as_deref(), Some(methods.as_str()));
    // the runs' maps are gone: only the maps of sensitivity are left as rasters
    let tifs: Vec<_> = walk(&out).into_iter().filter(|p| p.extension().is_some_and(|e| e == "tif")).collect();
    assert_eq!(tifs, [out.join("sensitivity_maps.tif")], "{tifs:?}");
    h.get_by_label("Show in the Viewer").scroll_to_me();
    h.run_steps(30);
    assert!(h.query_by_label_contains("Maps of sensitivity: sensitivity_maps.tif").is_some());
    assert!(h.query_by_label_contains("Bands: mean, sd, swing_patches.isometry").is_some());
    let layers = h.state().session.as_ref().unwrap().preview_layers.len();
    h.get_by_label("Show in the Viewer").click();
    h.run_steps(2);
    until(&mut h, "the maps shown", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.show_queue.is_empty()
    });
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().preview_layers.len() > layers, "the maps are a layer of the Viewer");
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    shot(&mut h, "batch-11-sensitivity-maps-viewer");

    // with the runs' maps kept, they stay
    let out = tmp("ui-sensitivity-maps-kept");
    let _ = std::fs::remove_dir_all(&out);
    h.state_mut().focus(Pane::Batch);
    {
        let b = &mut h.state_mut().batch;
        b.out = Some(out.clone());
        b.sens.keep_run_maps = true;
        b.sens.factors.truncate(1);
    }
    h.run_steps(3);
    h.get_by_label("Run 3 times").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the maps with the runs' maps kept", |a| !a.batch.running() && a.batch.making.is_none());
    h.run_steps(2);
    let b = &h.state().batch;
    assert!(b.error.is_none(), "{:?}", b.error);
    assert_eq!(b.finished.as_ref().unwrap().maps.as_ref().unwrap().bands.len(), 3);
    let tifs = walk(&out).into_iter().filter(|p| p.extension().is_some_and(|e| e == "tif")).count();
    assert_eq!(tifs, 1 + 3, "the maps and the 3 runs' maps");
}

/// Every file under a folder.
fn walk(dir: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out
}

#[test]
fn the_model_suggests_the_factors_it_assumes() {
    use terraflux_app::batch_view::Kind;
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let patches = s.source.nodes.iter_mut().find(|n| n.id == "patches").unwrap();
        patches.assumptions = vec!["The isometry of the patches is a guess".into()];
        s.edited();
    }
    h.state_mut().focus(Pane::Batch);
    h.state_mut().batch.kind = Kind::Sensitivity;
    h.run_steps(3);
    assert!(h.query_by_label("Suggested").is_some(), "the assumed setting is suggested");
    assert!(h.query_by_label_contains("patches.isometry").is_some());
    assert!(h.state().batch.sens.factors.is_empty());
    h.get_by_label("+").click();
    h.run_steps(3);
    let f = &h.state().batch.sens.factors;
    assert_eq!(f.len(), 1, "{f:?}");
    assert_eq!(f[0].key, "patches.isometry");
    assert!(!f[0].from.is_empty() && !f[0].to.is_empty(), "with a range: {f:?}");
    assert!(h.query_by_label("Suggested").is_none(), "what is taken is not suggested again");
    // the parameters with a range, all at once
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.parameters = ["simulation.times", "patches.rows"]
            .iter()
            .map(|k| terraflux_graph::Parameter {
                key: k.to_string(),
                min: Some(1.0),
                max: Some(9.0),
                ..Default::default()
            })
            .collect();
        s.edited();
    }
    h.run_steps(3);
    h.get_by_label("Add all").click();
    h.run_steps(3);
    let f = &h.state().batch.sens.factors;
    assert!(f.iter().any(|x| x.key == "simulation.times" && x.from == "1.0" && x.to == "9.0"), "{f:?}");
}

#[test]
fn a_setting_is_varied_from_its_menu_and_the_parameters_pane() {
    use terraflux_app::batch_view::Kind;
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "patches");
    }
    h.state_mut().focus(Pane::Node);
    h.run_steps(4);
    h.get_by_label("Isometry").click_secondary();
    h.run_steps(2);
    h.get_by_label("Vary in a sensitivity analysis").click();
    h.run_steps(4);
    let b = &h.state().batch;
    assert_eq!(b.kind, Kind::Sensitivity);
    assert!(b.sens.factors.iter().any(|f| f.key == "patches.isometry" && !f.from.is_empty()), "{:?}", b.sens.factors);
    assert_eq!(h.state_mut().focused(), Some(Pane::Batch), "Many runs to the front");
    // a parameter of the model: its own range
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.parameters = vec![terraflux_graph::Parameter {
            key: "simulation.times".into(),
            min: Some(2.0),
            max: Some(6.0),
            ..Default::default()
        }];
        s.edited();
    }
    h.state_mut().batch.sens.factors.clear();
    h.state_mut().focus(Pane::Parameters);
    h.run_steps(4);
    h.get_by_label("Sensitivity of these parameters...").click();
    h.run_steps(4);
    let b = &h.state().batch;
    assert_eq!(b.kind, Kind::Sensitivity);
    assert_eq!(b.sens.factors.len(), 1, "{:?}", b.sens.factors);
    assert_eq!((b.sens.factors[0].from.as_str(), b.sens.factors[0].to.as_str()), ("2.0", "6.0"));
    assert_eq!(h.state_mut().focused(), Some(Pane::Batch));
    // a whole analysis set up by the assistant (the tool `sensitivity`): in place of the factors there, to be run
    {
        let ask = terraflux_app::tools::sensitivity_ask(&serde_json::json!({
            "factors": ["patches.isometry"], "method": "sobol", "n": 8, "replicates": 2, "output": "simulation.maps",
        }))
        .unwrap();
        let s = h.state_mut().session.as_mut().unwrap();
        let ranges =
            terraflux_app::batch_view::asked_ranges(&s.model, &terraflux_ops::Registry::standard(), &ask.factors)
                .unwrap();
        s.ask_sensitivity = Some((ranges, ask));
    }
    h.state_mut().focus(Pane::Model);
    h.run_steps(4);
    let b = &h.state().batch;
    assert_eq!(b.kind, Kind::Sensitivity);
    assert_eq!(b.sens.factors.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(), ["patches.isometry"]);
    assert_eq!(b.sens.method, terraflux_dist::sensitivity::Method::Sobol);
    assert_eq!((b.sens.design.n, b.sens.replicates, b.sens.output.as_deref()), (8, 2, Some("simulation.maps")));
    assert!(!b.running(), "set up, not run");
    assert_eq!(h.state_mut().focused(), Some(Pane::Batch));
}

#[test]
fn the_cost_of_a_batch_is_told_before_it_runs() {
    use terraflux_app::batch_view::{Kind, Range};
    use terraflux_dist::sensitivity::Method;
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    h.state_mut().focus(Pane::Batch);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.times_before.clear();
        s.timings.clear();
        let b = &mut h.state_mut().batch;
        b.kind = Kind::Sensitivity;
        b.workers = 2;
        b.on = None;
        b.sens.method = Method::Oat;
        b.sens.output = Some("simulation.maps".into());
        b.sens.factors =
            vec![Range { key: "patches.isometry".into(), from: "0.5".into(), to: "2.5".into(), points: String::new() }];
    }
    h.run_steps(3);
    assert!(h.query_by_label_contains("known once the model has run").is_some(), "no times yet");
    assert!(h.query_by_label_contains("That is long").is_none());
    // a run of 4 s: 3 runs at 2 at once are 2 rounds
    h.state_mut().session.as_mut().unwrap().times_before.insert("simulation".into(), 4.0);
    h.run_steps(3);
    assert!(h.query_by_label_contains("3 runs, about 8.0 s here (2 at once; a run took about 4.0 s)").is_some());
    assert!(h.query_by_label_contains("That is long").is_none());
    // a run of 3 h: the warning, with the hint of a sensitivity analysis
    h.state_mut().session.as_mut().unwrap().times_before.insert("simulation".into(), 10800.0);
    h.run_steps(3);
    assert!(h.query_by_label_contains("about 6 h here").is_some());
    assert!(h.query_by_label_contains("to keep only the factors that matter").is_some());
    h.state_mut().batch.kind = Kind::Replicates;
    h.run_steps(3);
    assert!(h.query_by_label_contains("more computers (Computers)").is_some());
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
        sensitivity: None,
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
    assert_eq!(h.state_mut().focused(), Some(Pane::Model), "the viewer behind the model's graph");
    {
        let dock = h.state().dock.as_ref().unwrap();
        let (a, b) = (dock.find_tab(&Pane::Model).unwrap(), dock.find_tab(&Pane::Viewer).unwrap());
        assert_eq!((a.surface, a.node), (b.surface, b.node), "in a tab beside the model's");
    }
    h.state_mut().focus(Pane::Viewer);
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
    // (the window's title, and the action of that name in its list)
    assert!(h.state().shortcuts_open && h.query_all_by_label("Keyboard shortcuts").count() == 2);
    shot(&mut h, "31-shortcuts");
}

/// An operator of many settings: its essential ones shown, the others folded as advanced (the count of those the
/// model gives in the fold's title, open then); a setting given marked, and back to its default by its menu.
#[test]
fn many_settings_fold_as_advanced() {
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes = vec![Node::new("plan", "prioritize.optimize").param("objective", "min_set")];
        s.edited();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "plan");
    }
    h.state_mut().focus(Pane::Node);
    h.run_steps(3);
    shot(&mut h, "32-essentials");
    assert!(h.query_by_label("Objective").is_some() && h.query_by_label("Solver").is_some(), "essential");
    assert!(h.query_by_label("Number solutions").is_none(), "advanced, folded");
    assert!(h.query_by_label("Advanced (39)").is_some(), "48 settings, 9 essential");
    // a node with an advanced setting given: counted, its fold open when it is selected
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes.push(Node::new("plan2", "prioritize.optimize").param("number_solutions", 5));
        s.edited();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "plan2");
    }
    h.run_steps(3);
    assert!(h.query_by_label("Advanced (39 · 1 given)").is_some());
    assert!(h.query_by_label("Number solutions").is_some(), "open with a setting given");
    shot(&mut h, "32b-essentials-given");
}

/// The assistant's answers wrap at the pane's sides: long lines, a list, code and a tool's call.
#[test]
fn the_assistants_text_wraps() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    terraflux_app::settings::update(|s| s.provider = Some(0));
    h.state_mut().focus(Pane::Assistant);
    {
        let long = "The model reads the land use of 2000 and 2010, fits a logistic regression of every class on its drivers and allocates the demand of each step by the suitability plus an iteration variable, which is a long sentence that has to wrap at the side of the pane rather than run out of it.";
        let c = &mut h.state_mut().chat;
        c.items.push(terraflux_app::chat::Item::User("what does it do? ".repeat(12)));
        c.items.push(terraflux_app::chat::Item::Assistant {
            text: format!("{long}\n\n- **first**: {long}\n- `code` {long}\n\n```\n{}\n```", "x".repeat(300)),
            thinking: String::new(),
        });
        c.items.push(terraflux_app::chat::Item::Tool {
            name: "edit_node".into(),
            args: "{\"node\": \"a\", \"settings\": {\"expression\": \"".to_string() + &"a + ".repeat(60) + "1\"}}",
            result: Some(("done ".repeat(60), false)),
        });
        c.items.push(terraflux_app::chat::Item::Tool {
            name: "run_model".into(),
            args: "{}".into(),
            result: Some(("node b failed\nits input a: 10 x 10 cells".into(), true)),
        });
        c.items.push(terraflux_app::chat::Item::Note("no changes to review".into()));
    }
    h.run_steps(4);
    let pane = {
        let dock = h.state().dock.as_ref().unwrap();
        let path = dock.find_tab(&Pane::Assistant).unwrap();
        dock[path.surface][path.node].rect().unwrap()
    };
    // every label within the pane
    for n in h.query_all_by_label_contains("wrap at the side") {
        let r = n.rect();
        assert!(r.right() <= pane.right() + 1.0, "a line out of the pane: {r:?} {pane:?}");
    }
    shot(&mut h, "47-assistant-wrap");
    terraflux_app::settings::update(|s| s.provider = None);
}

/// A right click on a cell of a map: its menu traces the cell, or has the assistant explain it (the trace kept for
/// an assistant through terraflux mcp too).
#[test]
fn a_right_click_traces_a_cell() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    let t = terraflux_app::templates::TEMPLATES.iter().find(|t| t.name == "Neutral landscapes").unwrap();
    h.state_mut().start_template(&ctx, t);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("field", "map").is_some()
    });
    show(&mut h, "field", "map");
    h.run_steps(3);
    until(&mut h, "the map drawn", |a| a.session.as_ref().unwrap().preview_view.rect().is_some());
    drawn(&mut h);
    let rect = h.state().session.as_ref().unwrap().preview_view.rect().unwrap();
    let at = rect.center();
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
    assert!(h.state().session.as_ref().unwrap().preview_view.picked.is_some(), "the cell picked");
    shot(&mut h, "48-right-click");
    assert!(h.query_by_label("Explain with an MCP assistant").is_some(), "the option for an assistant through mcp");
    // the pointer to the menu, a few frames, then a click: the menu stays while the pointer goes to it
    let button = h.get_by_label("Ask the assistant why").rect().center();
    for k in 1..=4 {
        h.event(egui::Event::PointerMoved(at + (button - at) * (k as f32 / 4.0)));
        h.run_steps(2);
    }
    assert!(h.query_by_label("Ask the assistant why").is_some(), "the menu still open over it");
    h.get_by_label("Ask the assistant why").click();
    h.run_steps(4);
    assert!(h.state().is_open(Pane::Trace) && h.state().is_open(Pane::Assistant));
    // (the app's own trace: the file of the last one is shared by the tests that trace at the same time)
    let t = h.state().trace.trace.as_ref().expect("the cell traced");
    let words = format!("{}\n\n{}", t.sentence(), t.text());
    assert!(words.contains("Here field.map is"), "{words}");
    assert!(terraflux_app::trace::last().is_some(), "the last trace kept");
}

/// A run that fails: the status bar says so, the Problems pane explains it (what the node was given side by side,
/// the maps of different sizes), the node's pane asks why.
#[test]
fn a_failure_is_explained() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source = terraflux_graph::Model::from_toml(
            r#"
format = "terraflux/1"
[[nodes]]
id = "small"
op = "nlm.random"
params = { rows = 10, columns = 10, seed = 1 }
[[nodes]]
id = "big"
op = "nlm.random"
params = { rows = 20, columns = 10, seed = 2 }
[[nodes]]
id = "sum"
op = "raster.calc"
inputs = { a = "small.map", b = "big.map" }
params = { expression = "a + b" }
"#,
        )
        .unwrap();
        s.edited();
        s.run_when_ready = true;
    }
    h.run_steps(2);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.error.is_some()
    });
    h.run_steps(2);
    h.get_by_label("the run failed: why?").click();
    h.run_steps(3);
    assert!(h.state().is_open(Pane::Problems));
    assert!(h.query_by_label("The last run failed").is_some());
    assert!(h.query_all_by_label_contains("different sizes").next().is_some(), "what does not fit");
    assert!(h.query_all_by_label_contains("10 x 20 cells").next().is_some(), "the grids side by side");
    shot(&mut h, "49-problems");
    // the node's pane: why it failed
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &terraflux_ops::Registry::standard(), "sum");
    }
    h.state_mut().focus(Pane::Node);
    h.run_steps(3);
    h.get_by_label("Why it failed").click();
    h.run_steps(3);
    assert!(h.state().is_open(Pane::Problems));
}

/// The box of an input of the model is moved, selected and edited in the Node pane, made a node and back by its
/// menus, and taken away with Delete, as a node is.
#[test]
fn an_input_of_the_model_is_a_box_like_the_nodes() {
    let path = tmp("ui-inputs.toml");
    let mut m = terraflux_graph::Model::new("inputs");
    m.inputs.insert(
        "land".into(),
        terraflux_graph::Input::Map {
            path: data("sibuyan_landuse.tif").to_string_lossy().into_owned(),
            band: None,
            nodata: None,
            note: None,
        },
    );
    let mut n = terraflux_graph::Node::new("mspa", "landscape.mspa").input("map", "$land").param("foreground", 1);
    n.pos = Some([300.0, 0.0]);
    m.nodes.push(n);
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().focus(Pane::Model);
    h.run_steps(3);
    // dragged: moved, kept in the model, and selected
    let at = h.state().session.as_ref().unwrap().editor.screen_of("$land").unwrap();
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    for k in 1..=5 {
        h.event(egui::Event::PointerMoved(at + egui::vec2(12.0 * k as f32, 6.0 * k as f32)));
        h.run_steps(1);
    }
    let to = at + egui::vec2(60.0, 30.0);
    h.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.source.places.contains_key("|$land"), "{:?}", s.source.places);
        assert_eq!(s.selected.as_deref(), Some("$land"));
        let now = s.editor.screen_of("$land").unwrap();
        assert!((now - to).length() < (at - to).length(), "{at:?} {now:?} {to:?}");
    }
    shot(&mut h, "14-input");
    // its pane: made a node
    h.get_by_label("Make it a node").click();
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.node("land").unwrap().op, "raster.read");
        assert_eq!(s.source.node("mspa").unwrap().inputs["map"], terraflux_graph::Source::Ref("land.raster".into()));
        assert!(s.source.inputs.is_empty());
    }
    // the node's menu: an input again
    let at = h.state().session.as_ref().unwrap().editor.screen_of("land").unwrap();
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
    h.get_by_label("Make it an input of the model").click();
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.source.inputs.contains_key("land") && s.source.node("land").is_none());
        assert_eq!(s.selected.as_deref(), Some("$land"));
    }
    // Delete: taken away, what read it reads nothing
    h.state_mut().focus(Pane::Model);
    h.run_steps(2);
    h.key_press(egui::Key::Delete);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.source.inputs.is_empty());
    assert!(!s.source.node("mspa").unwrap().inputs.contains_key("map"));
}

#[test]
fn the_files_that_failed_are_listed_in_the_node_pane() {
    let path = tmp("ui-failed.toml");
    let mut m = terraflux_graph::Model::new("failed");
    let mut n = terraflux_graph::Node::new("mspa", "landscape.mspa").param("foreground", 1);
    n.pos = Some([300.0, 0.0]);
    m.nodes.push(n);
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().focus(Pane::Model);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.failures.insert(
            "mspa".into(),
            vec![("basin_07".into(), "mspa: no forest".into()), ("basin_12".into(), "mspa: no forest".into())],
        );
        s.selected = Some("mspa".into());
    }
    h.run_steps(3);
    assert!(h.query_by_label("Files that failed (2)").is_some());
    assert!(h.query_by_label("basin_12").is_some());
    assert!(h.query_by_label("Run again").is_some());
}

/// The themes side by side: the same model, run, in each (screenshots `ui-theme-<name>-<pane>.png` in the tests'
/// folder). Run by hand: `cargo test -p terraflux-app --test ui theme_gallery -- --ignored`.
#[test]
#[ignore]
fn theme_gallery() {
    for name in ["Default", "Immaterial", "Slate", "Paper", "Canopy"] {
        terraflux_app::settings::update(|s| s.theme = name.into());
        let mut h = harness(new_app(Some(model())));
        loaded(&mut h);
        h.state_mut().focus(Pane::Simulation);
        h.run_steps(3);
        // (the modern themes' button with its sign)
        let modern = ["Slate", "Paper", "Canopy"].contains(&name);
        h.get_by_label(if modern { "▶  Run" } else { "Run" }).click();
        h.run_steps(1);
        until(&mut h, "the run", |a| !a.session.as_ref().unwrap().running());
        h.run_steps(3);
        shot(&mut h, &format!("theme-{name}-map"));
        h.state_mut().focus(Pane::Model);
        h.state_mut().session.as_mut().unwrap().selected = Some("markov".into());
        h.run_steps(3);
        shot(&mut h, &format!("theme-{name}-graph"));
        // the other panes, the menus and the windows: of the default and two modern themes
        if !["Default", "Slate", "Paper"].contains(&name) {
            continue;
        }
        for (pane, what) in [
            (Pane::Operators, "operators"),
            (Pane::Files, "files"),
            (Pane::Areas, "areas"),
            (Pane::Problems, "problems"),
            (Pane::Assistant, "assistant"),
            (Pane::Batch, "batch"),
        ] {
            // (each in the layout of a session begun)
            h.state_mut().dock = Some(terraflux_app::app::default_layout());
            h.state_mut().focus(pane);
            h.run_steps(3);
            shot(&mut h, &format!("theme-{name}-{what}"));
        }
        h.state_mut().dock = Some(terraflux_app::app::default_layout());
        let s = h.state_mut().session.as_mut().unwrap();
        s.selected = Some("woe".into());
        s.fields.clear();
        s.preview = Some(("woe".into(), "table".into()));
        h.state_mut().focus(Pane::Viewer);
        h.run_steps(3);
        shot(&mut h, &format!("theme-{name}-table"));
        h.state_mut().settings_open = true;
        h.run_steps(3);
        shot(&mut h, &format!("theme-{name}-settings"));
        h.state_mut().settings_open = false;
        h.state_mut().commands = Some(Default::default());
        h.run_steps(3);
        shot(&mut h, &format!("theme-{name}-commands"));
        h.state_mut().commands = None;
        h.run_steps(2);
        h.get_by_label("File").click();
        h.run_steps(3);
        shot(&mut h, &format!("theme-{name}-menu"));
    }
    terraflux_app::settings::update(|s| s.theme = "Default".into());
}

/// The methods of a template as a paper's, in their window: MSPA cited with its papers and GuidosToolbox's.
#[test]
fn the_methods_and_citations_of_a_model() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(2);
    h.state_mut().session.as_mut().unwrap().methods_open = true;
    h.run_steps(3);
    assert!(h.query_by_label_contains("(MSPA; Soille & Vogt, 2009)").is_some(), "the paragraph");
    assert!(h.query_by_label_contains("Vogt, P., & Riitters, K. (2017)").is_some(), "the references");
    shot(&mut h, "18-methods");
    // in Portuguese: the paragraph, the methods' names, the headings
    h.get_by_label("Português").click();
    h.run_steps(3);
    assert!(h.query_by_label_contains("A análise foi feita no Terraflux").is_some(), "the paragraph in Portuguese");
    assert!(
        h.query_by_label_contains("análise morfológica de padrões espaciais (MSPA; Soille & Vogt, 2009)").is_some()
    );
    assert!(h.query_by_label("REFERÊNCIAS").is_some(), "the headings (in capitals)");
    shot(&mut h, "18-methods-pt");
    // closed by its button: the session says so
    h.get_by_label("Close window").click();
    h.run_steps(2);
    assert!(!h.state().session.as_ref().unwrap().methods_open);
}

/// A cell clicked in the viewer: its value in every map of the run, at the same place; a name clicked shows that
/// map.
#[test]
fn a_cell_in_every_map() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 10\n* -> 0".into())]).unwrap();
        s.preview = Some(("raster_reclassify".into(), "map".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_reclassify", "map").is_some()
    });
    // a cell of class 1 (forest): 10 after the rules
    let (row, col) = {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Map(m)) = s.value_of("raster_reclassify", "map") else { panic!() };
        let i = m.data.iter().position(|v| *v == 10.0).unwrap();
        (i / m.grid.width, i % m.grid.width)
    };
    h.state_mut().session.as_mut().unwrap().preview_view.picked = Some((row, col));
    h.run_steps(4);
    assert!(
        h.query_by_label("This cell in every map").is_some() || h.query_by_label("THIS CELL IN EVERY MAP").is_some()
    );
    assert!(h.query_by_label("sibuyan_landuse (layer 1 of 2)").is_some(), "the map read listed");
    assert!(h.query_all_by_label("10").next().is_some() && h.query_all_by_label("1").next().is_some(), "both values");
    shot(&mut h, "14d-cell-everywhere");
    // the map read, shown by its name (the section's label, not the checkbox of the maps over it)
    // (the file has two bands: the one read, the first, said)
    h.get_by_role_and_label(egui::accesskit::Role::Button, "sibuyan_landuse (layer 1 of 2)").click();
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().preview, Some(("sibuyan_landuse".into(), "raster".into())));
    // a map of two layers: they make an animation
    assert!(h.query_by_label("Save as GIF...").is_some());
}

/// File > Export to QGIS: the run's maps listed, the one shown chosen, the export button counting them; the
/// features drawn over the map in the Viewer's colours (by an attribute), the project zipped (.qgz).
#[test]
fn the_maps_of_a_run_to_qgis() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 10\n* -> 0".into())]).unwrap();
        // two plots over the map, drawn by their use
        let g = terraflux_gdal::Dataset::open(data("sibuyan_landuse.tif")).unwrap().meta.grid.clone();
        let t = g.transform;
        let (w, ht) = (g.width as f64 * t[1], g.height as f64 * -t[5]);
        let square = |a: f64, b: f64| {
            let (x0, y0, x1, y1) = (t[0] + w * a, t[3] - ht * b, t[0] + w * (a + 0.2), t[3] - ht * (b - 0.2));
            terraflux_raster::vector::Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
        };
        let v = terraflux_raster::vector::Vector {
            geometries: vec![square(0.2, 0.6), square(0.5, 0.9)],
            attributes: vec![(
                "use".into(),
                terraflux_raster::vector::Column::Texts(vec!["forest".into(), "pasture".into()]),
            )],
            crs: g.geo.wkt.clone(),
        };
        let gpkg = tmp("qgis-plots.gpkg");
        let _ = std::fs::remove_file(&gpkg);
        terraflux_gdal::write_vector(&gpkg, "plots", &v).unwrap();
        terraflux_app::builder::add(s, &r, "vector.read", &[("path", gpkg.to_string_lossy().into_owned().into())])
            .unwrap();
        let style = terraflux_app::vector_tiles::Style { by: Some("use".into()), ..Default::default() };
        s.preview_layers =
            vec![terraflux_app::builder::Layer { style, ..terraflux_app::builder::Layer::new("qgis_plots", "vector") }];
        s.preview = Some(("raster_reclassify".into(), "map".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_reclassify", "map").is_some() && s.value_of("qgis_plots", "vector").is_some()
    });
    h.state_mut().session.as_mut().unwrap().qgis.open = true;
    h.run_steps(3);
    // both maps of the run listed, the one shown chosen
    let names: Vec<String> =
        terraflux_app::qgis::candidates(h.state().session.as_ref().unwrap()).into_iter().map(|c| c.2).collect();
    assert_eq!(names, ["sibuyan_landuse", "raster_reclassify.map", "qgis_plots.vector"]);
    assert!(h.query_by_label(".qgz").is_some() && h.query_by_label(".qgs").is_some());
    // the map shown and the features drawn over it
    assert!(h.query_by_label("Export 2 layers...").is_some());
    shot(&mut h, "14e-qgis");
    h.get_by_label("all").click();
    h.run_steps(2);
    assert!(h.query_by_label("Export 3 layers...").is_some());
    // what is exported of the map shown: its palette, the classes of its look
    let s = h.state().session.as_ref().unwrap();
    let p = terraflux_app::qgis::pick(s, "raster_reclassify", "map").unwrap();
    assert_eq!(p.name, "raster_reclassify.map");
    assert!(terraflux_app::qgis::exportable(&p.value));
    // the features as the Viewer draws them; all of it as a .qgz
    let plots = terraflux_app::qgis::pick(s, "qgis_plots", "vector").unwrap();
    assert_eq!(plots.style.as_ref().and_then(|st| st.by.as_deref()), Some("use"));
    let dir = tmp("qgis-export");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let picks = vec![p, plots];
    let qgz = terraflux_app::qgis::export(&picks, &dir, "sibuyan", false).unwrap();
    assert_eq!(qgz, dir.join("sibuyan.qgz"));
    assert!(std::fs::read(&qgz).unwrap().starts_with(b"PK\x03\x04"));
}

/// The Viewer's Figure...: the map shown as a figure (its preview drawn), and every layer of a raster of several.
#[test]
fn a_map_as_a_figure() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("pie_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 2".into())]).unwrap();
        // a polygon over the middle of the map, drawn over it in the Viewer (and so in the figure)
        let g = terraflux_gdal::Dataset::open(data("pie_landuse.tif")).unwrap().meta.grid.clone();
        let t = g.transform;
        let (w, h) = (g.width as f64 * t[1], g.height as f64 * -t[5]);
        let (x0, y0) = (t[0] + w * 0.3, t[3] - h * 0.7);
        let (x1, y1) = (t[0] + w * 0.6, t[3] - h * 0.4);
        let v = terraflux_raster::vector::Vector {
            geometries: vec![terraflux_raster::vector::Geometry::Polygon(vec![vec![
                [x0, y0],
                [x1, y0],
                [x1, y1],
                [x0, y1],
                [x0, y0],
            ]])],
            attributes: Vec::new(),
            crs: g.geo.wkt.clone(),
        };
        let gpkg = tmp("figure-area.gpkg");
        let _ = std::fs::remove_file(&gpkg);
        terraflux_gdal::write_vector(&gpkg, "area", &v).unwrap();
        terraflux_app::builder::add(s, &r, "vector.read", &[("path", gpkg.to_string_lossy().into_owned().into())])
            .unwrap();
        s.preview_layers = vec![terraflux_app::builder::Layer::new("figure_area", "vector")];
        s.preview = Some(("raster_reclassify".into(), "map".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_reclassify", "map").is_some()
    });
    // (the whole model: the vectors too)
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the whole run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("figure_area", "vector").is_some()
    });
    drawn(&mut h);
    h.get_by_label("Figure...").click();
    h.run_steps(4);
    {
        let f = h.state().session.as_ref().unwrap().figure.as_ref().expect("the figure's window");
        // the map shown first (of the three the rules made of the raster's bands), a column wide
        assert_eq!(f.choices[0].0, "this map");
        assert_eq!(f.choices[0].1.len(), 1);
        assert_eq!((f.layout.columns, f.layout.width_mm), (1, 85.0));
        // the polygon over the map goes with it; the inset on by default; written as PDF and PNG
        assert_eq!(f.choices[0].1[0].vectors.len(), 1);
        assert!(f.layout.inset);
        let pdf = tmp("figure.pdf");
        terraflux_app::figure::save(&f.choices[0].1, &f.layout, &pdf).unwrap();
        assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-1.4"));
        terraflux_app::figure::save(&f.choices[0].1, &f.layout, &tmp("figure.png")).unwrap();
    }
    assert!(h.query_by_label("Save PNG...").is_some() && h.query_by_label("Save PDF...").is_some());
    shot(&mut h, "14f-figure");
    // the raster read, of three bands: each of them a panel
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.figure = None;
        s.preview = Some(("pie_landuse".into(), "raster".into()));
    }
    drawn(&mut h);
    // (its legend worked out in the background first)
    for _ in 0..400 {
        if h.query_by_label("Figure...").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
        h.run_steps(1);
    }
    h.get_by_label("Figure...").click();
    h.run_steps(3);
    let f = h.state().session.as_ref().unwrap().figure.as_ref().unwrap();
    let names: Vec<&str> = f.choices.iter().map(|c| c.0.as_str()).collect();
    assert_eq!(names, ["this map", "every layer (3)"]);
    assert_eq!(f.choices[1].1.len(), 3);
}

/// Draw on the map: a polygon clicked on the Viewer's map, closed on its first vertex, added to the model as a
/// GeoPackage beside it read by a vector.read node.
#[test]
fn a_study_area_drawn_on_the_map() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    let dir = tmp("drawn-model");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    // a model saved (its folder takes what is drawn), then edited
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.save(dir.join("model.toml")).unwrap();
        s.path = Some(dir.join("model.toml"));
    }
    h.run_steps(3);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("pie_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        s.preview = Some(("pie_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("pie_landuse", "raster").is_some()
    });
    drawn(&mut h);
    // (scrolled to, at the bottom of the panel beside the map; again while what threads make, a histogram, still
    // moves it between the scroll and the click)
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().preview_view.drawing.is_none() {
        // (a frame without the button, the panel still laid out: the next one)
        if let Some(b) = h.query_by_label("polygons") {
            b.scroll_to_me();
        }
        h.run_steps(3);
        if let Some(b) = h.query_by_label("polygons") {
            b.click();
        }
        h.run_steps(2);
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "drawing turned on: too long");
    }
    until(&mut h, "the map drawn", |a| a.session.as_ref().unwrap().preview_view.rect().is_some());
    let rect = h.state().session.as_ref().unwrap().preview_view.rect().unwrap();
    let c = rect.center();
    let click = |h: &mut Harness<TerrafluxApp>, at: egui::Pos2| {
        h.event(egui::Event::PointerMoved(at));
        h.run_steps(1);
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton {
                pos: at,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: egui::Modifiers::NONE,
            });
            h.run_steps(1);
        }
        // (apart: not a double click)
        h.run_steps(30);
    };
    for v in [egui::vec2(-60.0, -40.0), egui::vec2(70.0, -30.0), egui::vec2(20.0, 60.0)] {
        click(&mut h, c + v);
    }
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.drawing.as_ref().unwrap().current.len(), 3);
    shot(&mut h, "14g-drawing");
    // a click on the first vertex closes it
    click(&mut h, c + egui::vec2(-60.0, -40.0));
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.drawing.as_ref().unwrap().features.len(), 1);
    let ring = |h: &Harness<TerrafluxApp>| {
        h.state().session.as_ref().unwrap().preview_view.drawing.as_ref().unwrap().features[0].clone()
    };
    let before = ring(&h);
    let camera = h.state().session.as_ref().unwrap().preview_view.camera();
    // its second vertex dragged (the map not moved)
    let (from, to) = (c + egui::vec2(70.0, -30.0), c + egui::vec2(100.0, 0.0));
    h.event(egui::Event::PointerMoved(from));
    h.run_steps(1);
    let button = |pressed, pos| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    h.event(button(true, from));
    h.run_steps(1);
    for k in 1..=10 {
        h.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 10.0)));
        h.run_steps(1);
    }
    h.event(button(false, to));
    h.run_steps(30);
    let after = ring(&h);
    assert_eq!((after.len(), after[0], after[2]), (before.len(), before[0], before[2]), "only it moved");
    assert!((after[1][0] - before[1][0]).abs() > 1.0, "{before:?} -> {after:?}");
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.camera(), camera, "the map not moved");
    // an attribute typed
    h.get_by_label("+ an attribute").scroll_to_me();
    h.run_steps(2);
    h.get_by_label("+ an attribute").click_accesskit();
    h.run_steps(2);
    {
        let d = h.state_mut().session.as_mut().unwrap().preview_view.drawing.as_mut().unwrap();
        assert_eq!(d.columns, ["name"]);
        d.values[0][0] = "Pie".into();
    }
    h.run_steps(2);
    shot(&mut h, "14g2-drawing-edited");
    // added, and the catalog opened to fetch data cut to it
    h.get_by_label("Add, and data for it...").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("Add, and data for it...").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.preview_view.drawing.is_none(), "{:?}", s.notice);
    let file = dir.join("drawn").join("study_area.gpkg");
    assert!(file.exists(), "{:?}", s.notice);
    let v = terraflux_gdal::read_vector(&file, None).unwrap();
    assert_eq!(v.len(), 1);
    assert_eq!(v.column("name"), Some(&terraflux_raster::vector::Column::Texts(vec!["Pie".into()])));
    let read = s.source.nodes.iter().find(|n| n.op == "vector.read").expect("the node that reads it");
    assert!(s.catalog.open && s.catalog.choices.on_area);
    assert_eq!(s.catalog.choices.area, Some(format!("{}.vector", read.id)));
    h.run_steps(3);
    shot(&mut h, "14g3-catalog-area");
    // the download added: the area connected, its grid the area's (no run: it would go to the servers)
    h.state_mut().session.as_mut().unwrap().auto = false;
    h.get_by_label("Add to the model").click_accesskit();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.op == "data.download").expect("the download");
    assert!(n.params.contains_key("cellsize") && !n.params.contains_key("extent"), "{:?}", n.params);
    assert!(format!("{:?}", n.inputs).contains("study_area.vector"), "the area connected: {:?}", n.inputs);
}

/// The data catalog: a dataset chosen by its card (CHELSA), its variables ticked, added to the model as a
/// data.download node on the grid of the model's map, with its reference.
#[test]
fn a_dataset_from_the_catalog() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        // (no run: the download would go to the servers)
        s.auto = false;
        let path = data("pie_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        s.catalog.open = true;
    }
    h.run_steps(3);
    // the grid of the model's map by default; the datasets as cards
    assert_eq!(h.state().session.as_ref().unwrap().catalog.choices.like.as_deref(), Some("pie_landuse.raster"));
    h.get_by_label("CHELSA 2.1").click();
    h.run_steps(2);
    assert_eq!(h.state().session.as_ref().unwrap().catalog.chosen, 2);
    h.get_by_label("precipitation").click();
    h.run_steps(2);
    // what is in the cache: the map not run, nothing to know; on a box, the layers to download
    let said = |h: &Harness<TerrafluxApp>| {
        h.state().session.as_ref().unwrap().catalog.status.as_ref().map(|(_, s, _)| s.clone()).unwrap()
    };
    assert!(said(&h).is_err_and(|e| e.contains("has not run")), "{:?}", said(&h));
    let like = h.state_mut().session.as_mut().unwrap().catalog.choices.like.take();
    h.run_steps(2);
    assert!(said(&h).is_ok_and(|m| m.contains("120 × 120 cells")), "{:?}", said(&h));
    h.state_mut().session.as_mut().unwrap().catalog.choices.like = like;
    h.run_steps(2);
    shot(&mut h, "14h-catalog");
    h.get_by_label("Add to the model").scroll_to_me();
    drawn(&mut h);
    h.get_by_label("Add to the model").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.nodes.iter().find(|n| n.op == "data.download").expect("the download node");
    assert_eq!(n.params.get("source"), Some(&serde_json::json!("chelsa")));
    assert_eq!(n.params.get("layers"), Some(&serde_json::json!([12, 13, 14, 15, 16, 17, 18, 19])));
    assert!(n.params.get("extent").is_none(), "on the map's grid, not a box");
    assert!(
        matches!(n.inputs.get("like"), Some(terraflux_graph::Source::Ref(r)) if r == "pie_landuse.raster"),
        "{:?}",
        n.inputs
    );
    assert!(n.refs[0].contains("10.1038/sdata.2017.122"));
    assert!(matches!(&s.catalog.said, Some(Ok(m)) if m.starts_with("added: data_download")), "{:?}", s.catalog.said);
}

/// File > Python script (landschaft)...: the model as a script, a call of landschaft a node, in its window.
#[test]
fn the_model_as_a_python_script() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.auto = false;
        terraflux_app::builder::add(
            s,
            &r,
            "nlm.random_cluster",
            &[("p", "0.55".into()), ("weights", "[0.3, 0.7]".into())],
        )
        .unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "2 -> 2\n* -> 1".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "landscape.mspa", &[("edge_width", "2".into())]).unwrap();
    }
    h.run_steps(2);
    h.get_by_label("File").click();
    h.run_steps(3);
    h.get_by_label("Python script (landschaft)...").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let v = s.python.as_ref().expect("the script's window");
    assert_eq!((v.script.translated, v.script.left.len()), (3, 0));
    let t = &v.script.text;
    assert!(t.contains("nlm_random_cluster = ls.nlm.random_cluster(ncol=256, nrow=256, p=0.55, ai=[0.3, 0.7], seed=1, rescale=False)"), "{t}");
    assert!(t.contains("raster_reclassify = ls.reclassify(nlm_random_cluster, {2: 2}, others=1)"), "{t}");
    assert!(t.contains("landscape_mspa = ls.mspa(raster_reclassify.fillna(0).astype(\"uint8\"), edge_width=2)"), "{t}");
    assert!(h.query_by_label("Copy").is_some() && h.query_by_label("Save as...").is_some());
    shot(&mut h, "14i-python");
}

/// The catalog's preview: ESA WorldCover over a box (its first layer, coarse, fetched in a thread) and GBIF's
/// count of a species' records. Needs the internet: `cargo test -p terraflux-app --test ui a_preview -- --ignored`.
#[test]
#[ignore = "downloads from ESA WorldCover and asks GBIF"]
fn a_preview_from_the_catalog() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.auto = false;
        s.catalog.open = true;
        s.catalog.seen = true;
        s.catalog.chosen = terraflux_app::catalog::DATASETS.iter().position(|d| d.source == "esa_worldcover").unwrap();
        s.catalog.choices.extent = [-47.2, -23.0, -46.8, -22.7];
        s.catalog.choices.cellsize = 1.0 / 1200.0;
    }
    h.run_steps(3);
    let wait = |h: &mut Harness<TerrafluxApp>| {
        h.get_by_label("Preview").click_accesskit();
        h.run_steps(2);
        for _ in 0..600 {
            if h.state().session.as_ref().unwrap().catalog.preview.made.is_some()
                && h.query_by_label("fetching the preview...").is_none()
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
            h.run_steps(1);
        }
        h.run_steps(3);
    };
    wait(&mut h);
    {
        let p = &h.state().session.as_ref().unwrap().catalog.preview;
        let made = p.made.as_ref().unwrap().as_ref().expect("the preview");
        assert!(made.map.is_some() && made.said.starts_with("worldcover2021"), "{}", made.said);
    }
    shot(&mut h, "14h2-catalog-preview");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.catalog.chosen = terraflux_app::catalog::DATASETS.iter().position(|d| d.source == "gbif").unwrap();
        s.catalog.choices.species = "Puma concolor".into();
        s.catalog.choices.country = "BR".into();
        s.catalog.preview.made = None;
    }
    h.run_steps(3);
    wait(&mut h);
    let p = &h.state().session.as_ref().unwrap().catalog.preview;
    let made = p.made.as_ref().unwrap().as_ref().expect("the count");
    assert!(made.said.contains("records with coordinates at GBIF"), "{}", made.said);
    shot(&mut h, "14h3-catalog-gbif");
}

/// A file dropped, as the window gives it.
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

/// Files let go at a place of the window (the pointer there).
fn drop_files(h: &mut Harness<TerrafluxApp>, at: egui::Pos2, files: &[PathBuf]) {
    h.input_mut().events.push(egui::Event::PointerMoved(at));
    h.run_steps(1);
    h.input_mut().dropped_files = files.iter().map(|p| std::sync::Arc::new(Dropped(p.clone())) as _).collect();
    h.run_steps(1);
    h.input_mut().dropped_files.clear();
}

#[test]
fn over_the_viewer_is_told_by_where_it_was_drawn() {
    use egui::{Rect, pos2};
    let r = Rect::from_min_max(pos2(100.0, 100.0), pos2(400.0, 300.0));
    assert!(terraflux_app::app::over_viewer(Some(r), Some(pos2(200.0, 200.0))));
    assert!(!terraflux_app::app::over_viewer(Some(r), Some(pos2(50.0, 200.0))));
    assert!(!terraflux_app::app::over_viewer(Some(r), None));
    assert!(!terraflux_app::app::over_viewer(None, Some(pos2(200.0, 200.0))));
}

/// Rasters dropped at once, a layer each, joined as one stack when asked: of one folder, a node reading a pattern of
/// their names in place of theirs; of two folders, a stack.of they feed.
#[test]
fn rasters_dropped_at_once_joined_as_one_stack() {
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    h.state_mut().focus(Pane::Viewer);
    drawn(&mut h);
    let rect = h.state().viewer_rect.expect("the Viewer is drawn");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = None;
        s.preview_layers.clear();
    }
    let nodes = h.state().session.as_ref().unwrap().source.nodes.len();
    let folder = tmp("joined-series");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(folder.join("later")).unwrap();
    let years = ["1997", "2005", "2011"];
    for y in years {
        std::fs::copy(data("pie_restrictions.tif"), folder.join(format!("landuse_{y}.tif"))).unwrap();
    }
    std::fs::copy(data("pie_restrictions.tif"), folder.join("later").join("landuse_2020.tif")).unwrap();
    let files: Vec<PathBuf> = years.iter().map(|y| folder.join(format!("landuse_{y}.tif"))).collect();
    drop_files(&mut h, rect.center(), &files);
    drawn(&mut h);
    // (each node named by its file)
    assert_eq!(h.state().session.as_ref().unwrap().dropped_rasters, ["landuse_1997", "landuse_2005", "landuse_2011"]);
    shot(&mut h, "layers-dropped-rasters");
    h.get_by_label("As one stack (a time series)").click();
    h.run_steps(2);
    let id = {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.nodes.len(), nodes + 1, "one node in place of three");
        let n = s.source.nodes.last().unwrap();
        let want = folder.join("landuse_*.tif").to_string_lossy().into_owned();
        assert_eq!((n.op.as_str(), n.params["path"].as_str()), ("raster.read", Some(want.as_str())));
        assert_eq!(n.id, "landuse", "named by the pattern");
        let layers: Vec<&str> = s.preview_layers.iter().map(|l| l.node.as_str()).collect();
        assert_eq!(layers, [n.id.as_str()]);
        assert!(s.dropped_rasters.is_empty());
        n.id.clone()
    };
    until(&mut h, "the stack read", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of(&id, "raster").is_some()
    });
    drawn(&mut h);
    let Some(terraflux_ops::Value::Raster(r)) = h.state().session.as_ref().unwrap().value_of(&id, "raster") else {
        panic!("a stack")
    };
    assert_eq!(r.images.len(), 3, "a file a year");
    assert!(h.query_by_label("Play").is_some(), "its steps played");
    // (each step by the year in its file's name: a map a file)
    assert!(h.query_by_label("1997").is_some(), "the step shown by its year");
    // in two folders: a stack.of fed by them, the nodes kept
    let two = [folder.join("landuse_1997.tif"), folder.join("later").join("landuse_2020.tif")];
    drop_files(&mut h, rect.center(), &two);
    drawn(&mut h);
    h.get_by_label("As one stack (a time series)").click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes.len(), nodes + 4);
    let n = s.source.nodes.last().unwrap();
    assert_eq!((n.op.as_str(), n.inputs.len()), ("stack.of", 2));
    assert_eq!(s.preview.as_ref().map(|(p, _)| p), Some(&n.id));
}

/// Files of the Files pane let go over the Viewer: a node each, all of them layers of it.
#[test]
fn files_of_the_files_pane_let_go_over_the_viewer_are_its_layers() {
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    h.state_mut().focus(Pane::Viewer);
    drawn(&mut h);
    let rect = h.state().viewer_rect.expect("the Viewer is drawn");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = None;
        s.preview_layers.clear();
    }
    let nodes = h.state().session.as_ref().unwrap().source.nodes.len();
    let files = vec![data("sibuyan_landuse.tif"), data("sibuyan_drivers.tif"), data("pie_landuse.tif")];
    egui::DragAndDrop::set_payload(&h.ctx, terraflux_app::builder::DraggedFile(files));
    let at = rect.center();
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Default::default(),
    });
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes.len(), nodes + 3);
    assert_eq!(s.preview_layers.len(), 3, "every file a layer");
}

#[test]
fn files_dropped_on_the_viewer_become_layers_of_it() {
    let mut h = harness(new_app(Some(model())));
    loaded(&mut h);
    h.state_mut().focus(Pane::Viewer);
    drawn(&mut h);
    let rect = h.state().viewer_rect.expect("the Viewer is drawn");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = None;
        s.preview_layers.clear();
    }
    let nodes = h.state().session.as_ref().unwrap().source.nodes.len();
    // outside the Viewer: nodes only
    drop_files(&mut h, egui::pos2(2.0, 2.0), &[data("pie_landuse.tif")]);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.nodes.len(), nodes + 1);
        assert!(s.preview_layers.is_empty() && s.show_queue.is_empty());
    }
    h.state_mut().session.as_mut().unwrap().preview = None;
    // over it: nodes, layers in the order dropped, the first shown, and drawn after the runs
    drop_files(&mut h, rect.center(), &[data("sibuyan_landuse.tif"), data("sibuyan_drivers.tif")]);
    let ids: Vec<String> = {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.nodes.len(), nodes + 3);
        let ids: Vec<String> = s.preview_layers.iter().map(|l| l.node.clone()).collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(s.preview.as_ref().map(|(n, _)| n), Some(&ids[0]));
        ids
    };
    until(&mut h, "the runs", |a| {
        let s = a.session.as_ref().unwrap();
        s.show_queue.is_empty() && !s.running() && ids.iter().all(|id| s.value_of(id, "raster").is_some())
    });
    drawn(&mut h);
    // several outside the Viewer: all of them layers too (not only the last one, shown), over the map shown
    let shown = h.state().session.as_ref().unwrap().preview.clone();
    drop_files(&mut h, egui::pos2(2.0, 2.0), &[data("pie_landuse.tif"), data("sibuyan_landuse.tif")]);
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.nodes.len(), nodes + 5);
        assert_eq!(s.preview_layers.len(), 4, "the two before and the two dropped");
        assert_eq!(s.preview, shown, "the map shown stays");
    }
    // a folder: its rasters as one stack (a time series), a layer of the Viewer, shown with its steps; its vector
    // features as one layer over it
    let folder = tmp("dropped-series");
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    for year in ["1997", "2011"] {
        std::fs::copy(data("sibuyan_landuse.tif"), folder.join(format!("landuse_{year}.tif"))).unwrap();
    }
    let vectors = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference/vectors");
    std::fs::copy(vectors.join("mixed.gpkg"), folder.join("roads.gpkg")).unwrap();
    std::fs::copy(vectors.join("shapes.gpkg"), folder.join("plots.gpkg")).unwrap();
    h.state_mut().session.as_mut().unwrap().preview = None;
    drop_files(&mut h, rect.center(), std::slice::from_ref(&folder));
    let id = {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.nodes.len(), nodes + 7);
        let n = &s.source.nodes[s.source.nodes.len() - 2];
        assert_eq!((n.op.as_str(), n.id.as_str()), ("raster.read", "dropped_series"));
        let v = s.source.nodes.last().unwrap();
        assert_eq!((v.op.as_str(), v.id.as_str()), ("vector.read", "dropped_series_features"));
        assert_eq!(v.params["path"].as_str(), Some(folder.to_string_lossy().as_ref()), "one node of the folder");
        assert!(s.preview_layers.iter().any(|l| l.node == v.id), "the features a layer");
        n.id.clone()
    };
    until(&mut h, "the folder read", |a| {
        let s = a.session.as_ref().unwrap();
        s.show_queue.is_empty()
            && !s.running()
            && s.value_of(&id, "raster").is_some()
            && s.value_of("dropped_series_features", "vector").is_some()
    });
    {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Vector(v)) = s.value_of("dropped_series_features", "vector") else {
            panic!("the features")
        };
        assert!(v.column("file").is_some(), "the file of each feature: {:?}", v.attributes);
    }
    drawn(&mut h);
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Raster(r)) = s.value_of(&id, "raster") else { panic!("a stack") };
    // (each file's two bands, 1997 and 2011: a layer each)
    assert_eq!((r.images.len(), r.layers.len()), (2, 4), "the files and their layers");
    assert_eq!(s.preview.as_ref().map(|(n, _)| n), Some(&id));
    assert!(h.query_by_label("Play").is_some(), "its steps played");
}

/// The attribute table of a vector layer pinned over a map: opened from the layer's row (its table button), a row
/// clicked selects its feature (outlined, the view zoomed to it), a feature clicked on the map selects its row, the
/// button again closes it.
#[test]
fn the_attribute_table_of_a_vector_layer() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    let g = terraflux_gdal::Dataset::open(data("sibuyan_landuse.tif")).unwrap().meta.grid.clone();
    let t = g.transform;
    let (w, ht) = (g.width as f64 * t[1], g.height as f64 * -t[5]);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        // (another map, to compare the first with side by side)
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 0".into())]).unwrap();
        // two plots over the map, the second lower right
        let square = |a: f64, b: f64| {
            let (x0, y0, x1, y1) = (t[0] + w * a, t[3] - ht * b, t[0] + w * (a + 0.2), t[3] - ht * (b - 0.2));
            terraflux_raster::vector::Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
        };
        let v = terraflux_raster::vector::Vector {
            geometries: vec![square(0.2, 0.4), square(0.5, 0.9)],
            attributes: vec![
                ("area".into(), terraflux_raster::vector::Column::Numbers(vec![12.5, 3.0])),
                ("use".into(), terraflux_raster::vector::Column::Texts(vec!["forest".into(), "pasture".into()])),
            ],
            crs: g.geo.wkt.clone(),
        };
        let gpkg = tmp("attributes-plots.gpkg");
        let _ = std::fs::remove_file(&gpkg);
        terraflux_gdal::write_vector(&gpkg, "plots", &v).unwrap();
        terraflux_app::builder::add(s, &r, "vector.read", &[("path", gpkg.to_string_lossy().into_owned().into())])
            .unwrap();
        s.preview_layers = vec![terraflux_app::builder::Layer::new("attributes_plots", "vector")];
        s.preview = Some(("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("sibuyan_landuse", "raster").is_some()
            && s.value_of("raster_reclassify", "map").is_some()
            && s.value_of("attributes_plots", "vector").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    let open = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().preview_attributes.clone();
    assert!(open(&h).is_none());
    assert!(h.query_by_label("Attributes of attributes_plots.vector").is_none());
    // the table, from the menu of the layer's row
    use egui_kittest::kittest::NodeT;
    let dots = h
        .query_all_by_label_contains(": its menu")
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.contains("attributes_plots")))
        .expect("the menu of the features' row");
    dots.click();
    h.run_steps(2);
    h.get_by_label("Attribute table").click();
    h.run_steps(3);
    assert!(open(&h).is_some_and(|o| o.is("attributes_plots", "vector") && o.pick.row.is_none()));
    assert!(h.query_by_label("Attributes of attributes_plots.vector").is_some());
    // (the map drawn first: its view fitted, whatever the other tests keep the machine busy with)
    until(&mut h, "the map drawn", |a| a.session.as_ref().unwrap().preview_view.camera().0 > 0.0);
    let before = h.state().session.as_ref().unwrap().preview_view.camera().0;
    // a row clicked: the second feature selected, outlined on the map, the view on it (once the layer is ready)
    h.get_by_label("2 3 pasture").click();
    until(&mut h, "the view on the feature", |a| {
        let s = a.session.as_ref().unwrap();
        s.preview_attributes.as_ref().is_some_and(|o| !o.zoom) && s.preview_view.camera().0 > before * 2.0
    });
    let o = open(&h).unwrap();
    assert_eq!(o.pick.row, Some(1));
    assert!(!o.zoom, "zoomed");
    let view = &h.state().session.as_ref().unwrap().preview_view;
    assert_eq!(view.selected, Some((0, vec![1])));
    let (zoom, center) = view.camera();
    assert!(zoom > before * 2.0, "{before} -> {zoom}");
    let shown = view.shown_grid().unwrap().clone();
    let (x, y) = (
        shown.transform[0] + center.x as f64 * shown.transform[1],
        shown.transform[3] + center.y as f64 * shown.transform[5],
    );
    let (ex, ey) = (t[0] + w * 0.6, t[3] - ht * 0.8);
    assert!((x - ex).abs() < 2.0 * t[1] && (y - ey).abs() < 2.0 * t[1], "{x} {y} against {ex} {ey}");
    shot(&mut h, "14e-attribute-table");
    // a feature clicked on the map: its row selected, the table scrolled to it
    h.state_mut().session.as_mut().unwrap().preview_view.feature_clicked = vec![(0, 0)];
    h.run_steps(2);
    assert_eq!(open(&h).unwrap().pick.row, Some(0));
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.selected, Some((0, vec![0])));
    // ctrl+click on the other row: both selected and outlined, the view where it was, the header tells how many
    let selected = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().preview_view.selected.clone();
    let camera = h.state().session.as_ref().unwrap().preview_view.camera();
    h.get_by_label("2 3 pasture").click_modifiers(egui::Modifiers::COMMAND);
    until(&mut h, "both rows selected", |a| a.session.as_ref().unwrap().preview_view.selected == Some((0, vec![0, 1])));
    h.run_steps(2);
    assert!(h.query_by_label("2 selected").is_some());
    assert!(h.query_by_label("Zoom to selection").is_some() && h.query_by_label("Copy selected").is_some());
    assert!(!open(&h).unwrap().zoom, "zoomed by a ctrl+click");
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.camera(), camera);
    shot(&mut h, "14f-attribute-table-two");
    // ctrl+click on the map toggles a feature: the second one out, then in again
    for want in [vec![0], vec![0, 1]] {
        let view = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        (view.feature_clicked, view.feature_toggle) = (vec![(0, 1)], true);
        h.run_steps(2);
        assert_eq!(selected(&h), Some((0, want)));
    }
    // clear selection lets them all go
    h.get_by_label("Clear selection").click();
    h.run_steps(2);
    assert_eq!(selected(&h), None);
    assert!(open(&h).unwrap().pick.rows.is_empty() && open(&h).unwrap().pick.row.is_none());
    assert!(h.query_by_label("Clear selection").is_none());
    // a click and a shift+click: the range; escape lets it go
    h.get_by_label("1 12.5 forest").click();
    h.run_steps(2);
    h.get_by_label("2 3 pasture").click_modifiers(egui::Modifiers::SHIFT);
    until(&mut h, "the range selected", |a| a.session.as_ref().unwrap().preview_view.selected == Some((0, vec![0, 1])));
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert_eq!(selected(&h), None);
    // select all shown: the button (offered with nothing selected), and ctrl+A with the pointer on the table
    h.get_by_label("Select all shown").click();
    until(&mut h, "all selected by the button", |a| {
        a.session.as_ref().unwrap().preview_view.selected == Some((0, vec![0, 1]))
    });
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert_eq!(selected(&h), None);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::A);
    until(&mut h, "all selected by ctrl+A", |a| {
        a.session.as_ref().unwrap().preview_view.selected == Some((0, vec![0, 1]))
    });
    assert!(!open(&h).unwrap().zoom, "ctrl+A is no click on a row");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    // a ctrl+click that leaves one row selected does not zoom (a plain click does)
    h.get_by_label("1 12.5 forest").click();
    until(&mut h, "the view on the feature", |a| {
        let s = a.session.as_ref().unwrap();
        s.preview_attributes.as_ref().is_some_and(|o| !o.zoom && o.pick.plain)
            && s.preview_view.selected == Some((0, vec![0]))
    });
    let camera = h.state().session.as_ref().unwrap().preview_view.camera();
    h.get_by_label("2 3 pasture").click_modifiers(egui::Modifiers::COMMAND);
    until(&mut h, "both rows selected", |a| a.session.as_ref().unwrap().preview_view.selected == Some((0, vec![0, 1])));
    h.get_by_label("2 3 pasture").click_modifiers(egui::Modifiers::COMMAND);
    until(&mut h, "one row left", |a| a.session.as_ref().unwrap().preview_view.selected == Some((0, vec![0])));
    h.run_steps(3);
    assert!(!open(&h).unwrap().zoom, "zoomed by a ctrl+click");
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.camera(), camera);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert_eq!(selected(&h), None);
    // a feature clicked in a view side by side (drawing the Viewer's layers) selects its row; in one with layers of
    // its own it does not
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_compare.with =
            Some(terraflux_app::builder::CompareWith::Output("raster_reclassify".into(), "map".into()));
        s.preview_compare.how = terraflux_app::builder::How::SideBySide;
        s.preview_compare.more.push(terraflux_app::builder::Beside::new(terraflux_app::builder::CompareWith::Output(
            "raster_reclassify".into(),
            "map".into(),
        )));
    }
    drawn(&mut h);
    h.run_steps(3);
    for (view, want) in [(0, 1), (1, 0)] {
        let s = h.state_mut().session.as_mut().unwrap();
        let v = if view == 0 { &mut s.preview_compare.view } else { &mut s.preview_compare.more[0].view };
        v.feature_clicked = vec![(0, want)];
        h.run_steps(2);
        assert_eq!(open(&h).unwrap().pick.row, Some(want), "the feature clicked in view {}", view + 2);
        assert_eq!(selected(&h), Some((0, vec![want])));
    }
    // (a view with layers of its own: its clicks are of other layers)
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_compare.layers = Some(vec![terraflux_app::builder::Layer::new("attributes_plots", "vector")]);
        s.preview_compare.view.feature_clicked = vec![(0, 1)];
    }
    h.run_steps(3);
    assert_eq!(open(&h).unwrap().pick.row, Some(0), "the click in a view with layers of its own is not a row's");
    assert!(h.state().session.as_ref().unwrap().preview_compare.view.feature_clicked.is_empty());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_compare.layers = None;
        s.preview_compare.more.clear();
        s.preview_compare.with = None;
    }
    h.run_steps(3);
    // the tab-separated copy: its columns named, a line a feature
    let v = h.state().session.as_ref().unwrap().value_of("attributes_plots", "vector");
    let Some(terraflux_ops::Value::Vector(v)) = v else { panic!("no features") };
    let text = terraflux_app::attribute_table::tsv(&terraflux_app::attribute_table::table_of(&v));
    assert_eq!(text.lines().next(), Some("feature\tarea\tuse"));
    assert_eq!(text.lines().nth(1), Some("1\t12.5\tforest"));
    // the button again (in the row's menu) closes the table, the outline goes
    h.query_all_by_label_contains(": its menu")
        .find(|n| n.accesskit_node().label().is_some_and(|l| l.contains("attributes_plots")))
        .expect("the menu of the features' row")
        .click();
    h.run_steps(2);
    h.get_by_label("Attribute table").click();
    h.run_steps(3);
    assert!(open(&h).is_none());
    assert!(h.query_by_label("Attributes of attributes_plots.vector").is_none());
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.selected, None);
}

/// Features selected by area on the map: shift+drag a rectangle (ctrl too: added), shift+alt a lasso, escape lets a
/// shape go; the table follows and the view stays. Then *Filter the selection* makes a `vector.filter` node, and
/// ctrl+A selects the rows with the keyboard focus in the table.
#[test]
fn features_selected_by_area_on_the_map() {
    use egui::{Modifiers, Pos2};
    use egui_kittest::kittest::NodeT;
    use terraflux_raster::vector::{Column, Geometry, Vector};
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    let g = terraflux_gdal::Dataset::open(data("sibuyan_landuse.tif")).unwrap().meta.grid.clone();
    let t = g.transform;
    let (w, ht) = (g.width as f64 * t[1], g.height as f64 * -t[5]);
    // the world at a share of the map's width from the left and of its height from the top
    let world = |fx: f64, fy: f64| [t[0] + w * fx, t[3] - ht * fy];
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        let square = |a: f64, b: f64| {
            let (p, q) = (world(a, b), world(a + 0.2, b + 0.2));
            Geometry::Polygon(vec![vec![p, [q[0], p[1]], q, [p[0], q[1]], p]])
        };
        // 1 and 2: squares (upper left, lower right); 3: a point (upper right); 4: a line along the lower part,
        // reaching into the second square
        let v = Vector {
            geometries: vec![
                square(0.2, 0.2),
                square(0.5, 0.7),
                Geometry::Point(world(0.8, 0.2)),
                Geometry::LineString(vec![world(0.1, 0.8), world(0.55, 0.8)]),
            ],
            attributes: vec![
                ("area".into(), Column::Numbers(vec![12.5, 3.0, 7.0, 1.5])),
                ("use".into(), Column::Texts(vec!["forest".into(), "pasture".into(), "urban".into(), "road".into()])),
            ],
            crs: g.geo.wkt.clone(),
        };
        let gpkg = tmp("area-features.gpkg");
        let _ = std::fs::remove_file(&gpkg);
        terraflux_gdal::write_vector(&gpkg, "features", &v).unwrap();
        terraflux_app::builder::add(s, &r, "vector.read", &[("path", gpkg.to_string_lossy().into_owned().into())])
            .unwrap();
        s.preview_layers = vec![terraflux_app::builder::Layer::new("area_features", "vector")];
        s.preview = Some(("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("sibuyan_landuse", "raster").is_some()
            && s.value_of("area_features", "vector").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    let open = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().preview_attributes.clone();
    let rows = |h: &Harness<TerrafluxApp>| -> Vec<usize> {
        open(h).map(|o| o.pick.rows.iter().copied().collect()).unwrap_or_default()
    };
    let selected = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().preview_view.selected.clone();
    // a shape dragged with no table open opens the table of the topmost vector layer, with what it met
    let at = |h: &Harness<TerrafluxApp>, fx: f64, fy: f64| -> Pos2 {
        let v = &h.state().session.as_ref().unwrap().preview_view;
        let (zoom, center) = v.camera();
        let cell = egui::vec2((fx * g.width as f64) as f32, (fy * g.height as f64) as f32);
        v.rect().expect("the map is drawn").center() + (cell - center) * zoom
    };
    let drag = |h: &mut Harness<TerrafluxApp>, path: &[(f64, f64)], keys: Modifiers| {
        let pts: Vec<Pos2> = path.iter().map(|&(x, y)| at(h, x, y)).collect();
        h.event(egui::Event::PointerMoved(pts[0]));
        h.run_steps(1);
        h.event(egui::Event::PointerButton {
            pos: pts[0],
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: keys,
        });
        h.run_steps(1);
        for w in pts.windows(2) {
            for k in 1..=4 {
                h.event(egui::Event::PointerMoved(w[0] + (w[1] - w[0]) * (k as f32 / 4.0)));
                h.run_steps(1);
            }
        }
        h.event(egui::Event::PointerButton {
            pos: *pts.last().unwrap(),
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: keys,
        });
        h.run_steps(3);
    };
    assert!(open(&h).is_none());
    // (a rectangle is its first corner and where the pointer is: two corners are enough)
    drag(&mut h, &[(0.1, 0.1), (0.45, 0.45)], Modifiers::SHIFT);
    until(&mut h, "the table opened by the shape", |a| {
        a.session.as_ref().unwrap().preview_attributes.as_ref().is_some_and(|o| o.pick.rows.len() == 1)
    });
    h.run_steps(3);
    assert!(open(&h).is_some_and(|o| o.is("area_features", "vector")));
    assert_eq!(rows(&h), [0], "the first square");
    assert!(h.query_by_label("Attributes of area_features.vector").is_some());
    assert!(h.query_by_label("1 selected").is_some());
    // (the table took room: the view of the map is fitted again before the next shapes)
    h.state_mut().session.as_mut().unwrap().preview_view.fit();
    drawn(&mut h);
    h.run_steps(3);
    let camera2 = h.state().session.as_ref().unwrap().preview_view.camera();
    assert_ne!(camera2.0, 0.0);
    assert_eq!(selected(&h), Some((0, vec![0])));
    // shift+ctrl adds: the rectangle around the point
    drag(&mut h, &[(0.7, 0.1), (0.9, 0.3)], Modifiers::SHIFT | Modifiers::COMMAND);
    until(&mut h, "the point added", |a| {
        a.session.as_ref().unwrap().preview_attributes.as_ref().is_some_and(|o| o.pick.rows.len() == 2)
    });
    assert_eq!(rows(&h), [0, 2]);
    assert_eq!(selected(&h), Some((0, vec![0, 2])));
    assert!(h.query_by_label("2 selected").is_some());
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.camera(), camera2, "the map did not move");
    shot(&mut h, "14g-selected-by-area");
    // *Filter the selection*: a vector.filter node after the layer that keeps the features 1 and 3
    let nodes = h.state().session.as_ref().unwrap().source.nodes.len();
    let before = h.state().session.as_ref().unwrap().preview.clone();
    assert!(!h.get_by_label("Filter the selection").accesskit_node().is_disabled());
    h.get_by_label("Filter the selection").click();
    h.run_steps(3);
    let id = {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.source.nodes.len(), nodes + 1, "{:?}", s.editor.message);
        let n = s.source.nodes.last().unwrap();
        assert_eq!(n.op, "vector.filter");
        assert_eq!(n.params["expression"].as_str(), Some("feature == 1 or feature == 3"));
        assert!(
            n.inputs
                .get("vector")
                .is_some_and(|i| matches!(i, terraflux_graph::Source::Ref(r) if r == "area_features.vector"))
        );
        assert!(n.note.as_deref().is_some_and(|t| t.contains("2 features")) && n.why.is_some());
        assert_eq!(s.selected.as_deref(), Some(n.id.as_str()), "the node is selected");
        assert_eq!(s.preview, before, "the map shown stays");
        assert!(s.preview_layers.iter().any(|l| l.node == n.id), "shown in the Viewer, over the map");
        n.id.clone()
    };
    until(&mut h, "the filter run", |a| {
        let s = a.session.as_ref().unwrap();
        s.show_queue.is_empty() && !s.running() && s.value_of(&id, "vector").is_some()
    });
    {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Vector(v)) = s.value_of(&id, "vector") else { panic!("the features kept") };
        assert_eq!(v.len(), 2, "exactly the selected features");
        assert_eq!(v.column("use").map(|c| c.numbers().len()), Some(2));
        let Some(Column::Texts(uses)) = v.column("use") else { panic!("texts") };
        assert_eq!(uses, &["forest".to_string(), "urban".to_string()]);
    }
    drawn(&mut h);
    // one undo step takes the node away
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::undo_redo(s, false);
        assert_eq!(s.source.nodes.len(), nodes);
    }
    h.run_steps(3);
    drawn(&mut h);
    // shift alone replaces: the rectangle around the second square and the end of the line
    drag(&mut h, &[(0.45, 0.75), (0.6, 0.85)], Modifiers::SHIFT);
    until(&mut h, "the rectangle's features", |a| {
        a.session
            .as_ref()
            .unwrap()
            .preview_attributes
            .as_ref()
            .is_some_and(|o| o.pick.rows.len() == 2 && o.pick.rows.contains(&3))
    });
    assert_eq!(rows(&h), [1, 3], "the square and the line that reaches into the rectangle");
    // a lasso (shift+alt) round the point: a triangle of whose box the point is in, but not of the triangle: none
    drag(&mut h, &[(0.6, 0.4), (0.95, 0.4), (0.95, 0.1), (0.61, 0.4)], Modifiers::SHIFT | Modifiers::ALT);
    h.run_steps(3);
    assert_eq!(rows(&h), Vec::<usize>::new(), "the point is in the lasso's box, not in the lasso");
    assert_eq!(selected(&h), None);
    assert!(h.get_by_label("Filter the selection").accesskit_node().is_disabled(), "nothing to filter by");
    // a lasso that holds it
    drag(&mut h, &[(0.65, 0.1), (0.95, 0.1), (0.95, 0.3), (0.7, 0.3), (0.66, 0.11)], Modifiers::SHIFT | Modifiers::ALT);
    until(&mut h, "the lasso's feature", |a| {
        a.session.as_ref().unwrap().preview_attributes.as_ref().is_some_and(|o| o.pick.rows.len() == 1)
    });
    assert_eq!(rows(&h), [2]);
    // escape lets a shape go: the selection stays, whatever the pointer does after it
    h.event(egui::Event::PointerMoved(at(&h, 0.1, 0.1)));
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: at(&h, 0.1, 0.1),
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::SHIFT,
    });
    h.run_steps(1);
    for k in 1..=4 {
        h.event(egui::Event::PointerMoved(at(&h, 0.1 + 0.1 * k as f64, 0.1 + 0.1 * k as f64)));
        h.run_steps(1);
    }
    assert!(h.state().session.as_ref().unwrap().preview_view.area.is_some(), "the shape is being dragged");
    h.key_press(egui::Key::Escape);
    h.run_steps(1);
    assert!(h.state().session.as_ref().unwrap().preview_view.area.is_none());
    h.event(egui::Event::PointerMoved(at(&h, 0.8, 0.8)));
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: at(&h, 0.8, 0.8),
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::SHIFT,
    });
    h.run_steps(3);
    assert_eq!(rows(&h), [2], "the selection stays");
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.camera(), camera2, "the map did not move");
    // ctrl+A with the keyboard focus in the table, the pointer on the map; not with a box focused
    h.get_by_label("2 3 pasture").click();
    h.run_steps(2);
    assert_eq!(rows(&h), [1]);
    let below = h.state().session.as_ref().unwrap().preview_view.rect().unwrap().bottom();
    let filter = h
        .query_all_by_role(egui::accesskit::Role::TextInput)
        .find(|n| n.accesskit_node().bounding_box().is_some_and(|b| b.y0 as f32 > below))
        .expect("the table's filter box");
    filter.click();
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    h.run_steps(3);
    assert_eq!(rows(&h), [1], "ctrl+A in a box is the box's");
    h.get_by_label("2 3 pasture").click();
    h.run_steps(2);
    let centre = h.state().session.as_ref().unwrap().preview_view.rect().unwrap().center();
    h.event(egui::Event::PointerMoved(centre));
    h.run_steps(2);
    h.key_press_modifiers(Modifiers::COMMAND, egui::Key::A);
    until(&mut h, "all selected by ctrl+A with the focus in the table", |a| {
        a.session.as_ref().unwrap().preview_attributes.as_ref().is_some_and(|o| o.pick.rows.len() == 4)
    });
    assert_eq!(rows(&h), [0, 1, 2, 3]);
}

/// The histogram of the map shown: bars of its values under the legend, the numbers under them; a map of classes
/// has bars for its classes in the legend.
#[test]
fn the_histogram_of_a_map() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::add(
            s,
            &r,
            "nlm.random",
            &[("rows", 60.into()), ("columns", 80.into()), ("seed", 4.into())],
        )
        .unwrap();
        s.preview = Some(("nlm_random".into(), "map".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("nlm_random", "map").is_some()
    });
    let t = Instant::now();
    while h.query_by_label_contains("cells with a value").is_none() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "the histogram: too long");
    }
    assert!(h.query_by_label_contains("4800 cells with a value, 0 with none").is_some());
    assert!(h.query_by_label_contains("mean").is_some());
    assert!(h.query_by_label_contains("from a sample").is_none());
    shot(&mut h, "14f-histogram");
}

/// The labels of a vector layer pinned over a map: by an attribute of its features, drawn over them (two plots,
/// two labels), none when the layer is labelled by nothing, and kept in the layer's style.
#[test]
fn the_labels_of_a_vector_layer() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    let g = terraflux_gdal::Dataset::open(data("sibuyan_landuse.tif")).unwrap().meta.grid.clone();
    let t = g.transform;
    let (w, ht) = (g.width as f64 * t[1], g.height as f64 * -t[5]);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        let square = |a: f64, b: f64| {
            let (x0, y0, x1, y1) = (t[0] + w * a, t[3] - ht * b, t[0] + w * (a + 0.3), t[3] - ht * (b - 0.3));
            terraflux_raster::vector::Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
        };
        let v = terraflux_raster::vector::Vector {
            geometries: vec![square(0.1, 0.5), square(0.55, 0.9)],
            attributes: vec![
                ("area".into(), terraflux_raster::vector::Column::Numbers(vec![12.5, 3.0])),
                ("use".into(), terraflux_raster::vector::Column::Texts(vec!["forest".into(), "pasture".into()])),
            ],
            crs: g.geo.wkt.clone(),
        };
        let gpkg = tmp("labels-plots.gpkg");
        let _ = std::fs::remove_file(&gpkg);
        terraflux_gdal::write_vector(&gpkg, "plots", &v).unwrap();
        terraflux_app::builder::add(s, &r, "vector.read", &[("path", gpkg.to_string_lossy().into_owned().into())])
            .unwrap();
        let mut layer = terraflux_app::builder::Layer::new("labels_plots", "vector");
        layer.style.labels = Some("use".into());
        s.preview_layers = vec![layer];
        s.preview = Some(("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("sibuyan_landuse", "raster").is_some()
            && s.value_of("labels_plots", "vector").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    // (the layer's features are prepared for the map in a worker: waited for)
    let placed = |a: &TerrafluxApp| a.session.as_ref().unwrap().preview_view.vectors.labels_placed;
    until(&mut h, "the labels placed", |a| placed(a) == 2);
    shot(&mut h, "14g-labels");
    // by a number: the same two features, labelled
    h.state_mut().session.as_mut().unwrap().preview_layers[0].style.labels = Some("area".into());
    h.run_steps(3);
    until(&mut h, "the labels by a number", |a| placed(a) == 2);
    // by nothing: none
    h.state_mut().session.as_mut().unwrap().preview_layers[0].style.labels = None;
    until(&mut h, "no labels", |a| placed(a) == 0);
}

/// The symbology of a vector layer: six squares coloured by a number in three classes of quantiles, the legend under
/// the layer's row naming them, the exported view with the class colours, and the style kept in a Plot and back.
#[test]
fn the_symbology_of_a_vector_layer() {
    use terraflux_app::vector_tiles::{Breaks, Style, Swatches, class_of_ramp};
    use terraflux_raster::vector::{Column, Geometry, Vector};
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    let g = terraflux_gdal::Dataset::open(data("sibuyan_landuse.tif")).unwrap().meta.grid.clone();
    let t = g.transform;
    let (w, ht) = (g.width as f64 * t[1], g.height as f64 * -t[5]);
    // the squares' places: a share of the width from the left, of the height from the top (0.25 across)
    let at = [(0.05, 0.1), (0.37, 0.1), (0.69, 0.1), (0.05, 0.55), (0.37, 0.55), (0.69, 0.55)];
    let pop = vec![5.0, 8.0, 12.0, 15.0, 30.0, 50.0];
    let style = Style {
        by: Some("pop".into()),
        breaks: Breaks::Quantiles,
        classes: 3,
        fill: 1.0,
        width: 0.0,
        ..Style::default()
    };
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        let square = |(a, b): (f64, f64)| {
            let (x0, y0, x1, y1) = (t[0] + w * a, t[3] - ht * b, t[0] + w * (a + 0.25), t[3] - ht * (b + 0.25));
            Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])
        };
        let v = Vector {
            geometries: at.iter().map(|p| square(*p)).collect(),
            attributes: vec![("pop".into(), Column::Numbers(pop.clone()))],
            crs: g.geo.wkt.clone(),
        };
        let gpkg = tmp("symbology-squares.gpkg");
        let _ = std::fs::remove_file(&gpkg);
        terraflux_gdal::write_vector(&gpkg, "squares", &v).unwrap();
        terraflux_app::builder::add(s, &r, "vector.read", &[("path", gpkg.to_string_lossy().into_owned().into())])
            .unwrap();
        let mut layer = terraflux_app::builder::Layer::new("symbology_squares", "vector");
        layer.style = style.clone();
        s.preview_layers = vec![layer];
        s.preview = Some(("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("sibuyan_landuse", "raster").is_some()
            && s.value_of("symbology_squares", "vector").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    // the legend under the layer's row: the attribute and a range each class
    h.run_steps(3);
    assert!(h.query_by_label("by pop").is_some());
    for range in ["5 \u{2013} 8", "8 \u{2013} 15", "15 \u{2013} 50"] {
        assert!(h.query_by_label(range).is_some(), "the class {range} in the legend");
    }
    shot(&mut h, "14s-symbology-classes");
    // the layer selected: its properties have the style's editor (the ramp, the breaks, the classes)
    {
        use egui_kittest::kittest::NodeT;
        let row = h
            .query_all_by_label_contains("symbology_squares")
            .find(|n| n.accesskit_node().label().is_some_and(|l| !l.contains("its menu")))
            .expect("the layer's row");
        row.click();
    }
    h.run_steps(3);
    assert!(h.query_all_by_label("ramp").count() >= 1, "the ramp bar");
    shot(&mut h, "14s-symbology-editor");
    // the classes' colours by the legend's own look
    let look = {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Vector(v)) = s.value_of("symbology_squares", "vector") else { panic!() };
        terraflux_app::vector_tiles::look(&v, &style)
    };
    let Swatches::Classes { bounds, .. } = &look.legend.swatches else { panic!("{:?}", look.legend.swatches) };
    assert_eq!(bounds, &vec![5.0, 8.0, 15.0, 50.0]);
    let class = |i: usize| class_of_ramp(style.ramp, i, 3);
    assert_eq!((look.colours[0], look.colours[3], look.colours[5]), (class(0), class(1), class(2)));
    // the exported view: a pixel in the middle of the first and the last square is the class colour
    for _ in 0..400 {
        if h.query_by_label("Export the view...").is_some() {
            break;
        }
        std::thread::sleep(Duration::from_millis(25));
        h.run_steps(1);
    }
    h.get_by_label("Export the view...").click();
    h.run_steps(4);
    let png = tmp("symbology-view.png");
    {
        let e = h.state().session.as_ref().unwrap().export.as_ref().expect("the export's window");
        assert_eq!(e.panel.vectors.len(), 1);
        terraflux_app::export_view::save(&e.panel, 1, terraflux_app::export_view::Format::Png, &png).unwrap();
    }
    let pm = tiny_skia::Pixmap::load_png(&png).unwrap();
    let pixel = |k: usize| {
        let (fx, fy) = at[k];
        let (col, row) = (((fx + 0.125) * g.width as f64) as u32, ((fy + 0.125) * g.height as f64) as u32);
        let c = pm.pixel(col, row).unwrap();
        [c.red(), c.green(), c.blue()]
    };
    let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
    assert_eq!(pixel(0), rgb(class(0)), "the least class");
    assert_eq!(pixel(3), rgb(class(1)), "the middle class");
    assert_eq!(pixel(5), rgb(class(2)), "the greatest class");
    // kept in a Plot: the style with its breaks and classes, and back from it
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let s = h.state_mut().session.as_mut().unwrap();
    let id = s.source.plots.last().expect("the Plot").id.clone();
    let kept =
        s.source.plot(&id).unwrap().layers.iter().find(|l| l.from == "symbology_squares.vector").unwrap().clone();
    let json = kept.style.expect("the style kept in the Plot");
    assert_eq!(
        (json["breaks"].as_str(), json["classes"].as_u64(), json["by"].as_str()),
        (Some("quantiles"), Some(3), Some("pop"))
    );
    // (written to the model's file and read back)
    let text = s.source.to_toml().unwrap();
    assert_eq!(terraflux_graph::Model::from_toml(&text).unwrap().plots, s.source.plots);
    s.preview_layers.clear();
    terraflux_app::builder::open_plot(s, &r, &id);
    let back = s.preview_layers.iter().find(|l| l.node == "symbology_squares").expect("the layer");
    assert_eq!(back.style, style);
    // the CLI's and the MCP tool's picture of the Plot: the same colours (a figure's vectors take them too)
    let made = {
        let s = h.state().session.as_ref().unwrap();
        let value = |from: &str| {
            let (n, p) = from.split_once('.')?;
            s.value_of(n, p)
        };
        terraflux_app::plot_export::panel_of(&s.source, &id, &value).unwrap()
    };
    assert_eq!(made.panel.vectors.len(), 1);
    let drawn_by = &made.panel.vectors[0];
    assert_eq!((drawn_by.colours[0], drawn_by.colours[5]), (class(0), class(2)));
    assert!(matches!(drawn_by.legend.swatches, Swatches::Classes { .. }));
}

/// The Viewer's Export the view...: the map shown with a layer over it and a polygon, written as PNG and as GeoTIFF
/// on the map's grid, a cell to a pixel and magnified.
#[test]
fn the_view_exported_as_a_picture() {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    let g = terraflux_gdal::Dataset::open(data("sibuyan_landuse.tif")).unwrap().meta.grid.clone();
    let t = g.transform;
    let (w, ht) = (g.width as f64 * t[1], g.height as f64 * -t[5]);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 2".into())]).unwrap();
        let (x0, y0, x1, y1) = (t[0] + w * 0.3, t[3] - ht * 0.7, t[0] + w * 0.6, t[3] - ht * 0.4);
        let v = terraflux_raster::vector::Vector {
            geometries: vec![terraflux_raster::vector::Geometry::Polygon(vec![vec![
                [x0, y0],
                [x1, y0],
                [x1, y1],
                [x0, y1],
                [x0, y0],
            ]])],
            attributes: Vec::new(),
            crs: g.geo.wkt.clone(),
        };
        let gpkg = tmp("export-area.gpkg");
        let _ = std::fs::remove_file(&gpkg);
        terraflux_gdal::write_vector(&gpkg, "area", &v).unwrap();
        terraflux_app::builder::add(s, &r, "vector.read", &[("path", gpkg.to_string_lossy().into_owned().into())])
            .unwrap();
        let mut over = terraflux_app::builder::Layer::new("raster_reclassify", "map");
        over.opacity = 0.5;
        s.preview_layers = vec![terraflux_app::builder::Layer::new("export_area", "vector"), over];
        s.preview = Some(("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("export_area", "vector").is_some()
            && s.value_of("raster_reclassify", "map").is_some()
    });
    drawn(&mut h);
    for _ in 0..400 {
        if h.query_by_label("Export the view...").is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
        h.run_steps(1);
    }
    h.get_by_label("Export the view...").click();
    h.run_steps(4);
    assert!(h.query_by_label("Save PNG...").is_some() && h.query_by_label("Save GeoTIFF...").is_some());
    let e = h.state().session.as_ref().unwrap().export.as_ref().expect("the export's window");
    assert_eq!((e.panel.vectors.len(), e.panel.rasters.len()), (1, 1));
    let (png, tif, big) = (tmp("view.png"), tmp("view.tif"), tmp("view-x2.png"));
    let said = terraflux_app::export_view::save(&e.panel, 1, terraflux_app::export_view::Format::Png, &png).unwrap();
    assert!(said.starts_with("saved: "), "{said}");
    terraflux_app::export_view::save(&e.panel, 2, terraflux_app::export_view::Format::Png, &big).unwrap();
    terraflux_app::export_view::save(&e.panel, 1, terraflux_app::export_view::Format::GeoTiff, &tif).unwrap();
    let size = |p: &std::path::Path| {
        let pm = tiny_skia::Pixmap::load_png(p).unwrap();
        (pm.width() as usize, pm.height() as usize)
    };
    assert_eq!(size(&png), (g.width, g.height));
    assert_eq!(size(&big), (g.width * 2, g.height * 2));
    // the GeoTIFF: four bands on the map's grid, with something drawn
    let back = terraflux_raster::tiff::read(&tif).unwrap();
    assert_eq!(back.bands.len(), 4);
    assert_eq!((back.grid.width, back.grid.height), (g.width, g.height));
    assert_eq!(back.grid.transform, g.transform);
    assert!(back.bands[3].data.contains(&255.0), "cells drawn");
    shot(&mut h, "14h-export");
}

/// A layer of another coordinate system pinned over the map is drawn reprojected, tile by tile, where it lies: the
/// land in geographic coordinates over the drivers in UTM looks as the land in UTM does (the picture of the map's
/// middle, cell by cell of the screen), and not as the drivers do.
#[test]
fn a_layer_of_another_coordinate_system_is_drawn_reprojected() {
    use terraflux_app::builder::Layer;
    use terraflux_graph::Node;
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    // the files have no CRS: Sibuyan is in UTM 51N; the land again in geographic coordinates (nearest cell)
    let d = tmp("reprojected");
    std::fs::create_dir_all(&d).unwrap();
    let (land, drivers, geo) = (d.join("land.tif"), d.join("drivers.tif"), d.join("land_geo.tif"));
    let srs = ["-a_srs".to_string(), "EPSG:32651".to_string()];
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &srs).unwrap();
    terraflux_gdal::translate(&data("sibuyan_drivers.tif"), &drivers, &srs).unwrap();
    terraflux_gdal::warp(&[&land], &geo, &["-t_srs".into(), "EPSG:4326".into(), "-r".into(), "near".into()]).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = |id: &str, p: &std::path::Path| {
            Node::new(id, "raster.read").param("path", p.to_string_lossy().as_ref()).param("band", "0")
        };
        s.source.nodes = vec![read("land", &land), read("drivers", &drivers), read("geo", &geo)];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "drivers", "geo"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    // the picture of the middle of the map shown (the cells from 20% to 80% across, a lattice of screen points)
    let picture = |h: &mut Harness<TerrafluxApp>| -> Vec<[u8; 4]> {
        drawn(h);
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(500) {
            h.step();
            std::thread::sleep(Duration::from_millis(10));
        }
        drawn(h);
        let s = h.state().session.as_ref().unwrap();
        let (rect, (zoom, centre)) = (s.preview_view.rect().expect("the map drawn"), s.preview_view.camera());
        let (w, ht) = s.preview_view.shown_grid().map(|g| (g.width as f32, g.height as f32)).unwrap();
        let img = h.render().expect("the window drawn by wgpu");
        let mut out = Vec::new();
        for i in 0..40 {
            for j in 0..40 {
                let cell = egui::Vec2::new(w * (0.2 + 0.6 * j as f32 / 39.0), ht * (0.2 + 0.6 * i as f32 / 39.0));
                let at = rect.center() + (cell - centre) * zoom;
                out.push(img.get_pixel(at.x as u32, at.y as u32).0);
            }
        }
        out
    };
    let same = |a: &[[u8; 4]], b: &[[u8; 4]]| {
        let near = |p: &[u8; 4], q: &[u8; 4]| (0..3).all(|k| p[k].abs_diff(q[k]) <= 8);
        a.iter().zip(b).filter(|(p, q)| near(p, q)).count() as f32 / a.len() as f32
    };
    show(&mut h, "land", "raster");
    let land_alone = picture(&mut h);
    show(&mut h, "drivers", "raster");
    let drivers_alone = picture(&mut h);
    assert!(same(&land_alone, &drivers_alone) < 0.9, "the two maps differ");
    // the land in geographic coordinates over the drivers in UTM
    h.state_mut().session.as_mut().unwrap().preview_layers =
        vec![Layer::new("drivers", "raster"), Layer::new("geo", "raster")];
    let over = picture(&mut h);
    shot(&mut h, "14h-reprojected");
    let (like_land, like_drivers) = (same(&over, &land_alone), same(&over, &drivers_alone));
    assert!(like_land > 0.9, "the layer is there and lines up: {like_land} of the lattice is the land's");
    assert!(like_drivers < 0.9, "the layer hides the drivers: {like_drivers}");
    // hidden: the drivers again
    let hidden = Layer { visible: false, ..Layer::new("geo", "raster") };
    h.state_mut().session.as_mut().unwrap().preview_layers = vec![Layer::new("drivers", "raster"), hidden];
    assert!(same(&picture(&mut h), &drivers_alone) > 0.99, "hidden");
}

/// A layer of another coordinate system drawn as a mesh honours its blend mode in the live view: the land in
/// geographic coordinates multiplied over the land in UTM is each colour times itself (the layer is the land's own
/// picture, so the product is told), screened it is lighter, and the normal way it is the land again.
#[test]
fn a_reprojected_layer_is_blended_in_the_live_view() {
    use terraflux_app::blend::Blend;
    use terraflux_app::builder::Layer;
    use terraflux_graph::Node;
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let d = tmp("reprojected_blend");
    std::fs::create_dir_all(&d).unwrap();
    let (land, geo) = (d.join("land.tif"), d.join("land_geo.tif"));
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &["-a_srs".to_string(), "EPSG:32651".to_string()])
        .unwrap();
    terraflux_gdal::warp(&[&land], &geo, &["-t_srs".into(), "EPSG:4326".into(), "-r".into(), "near".into()]).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = |id: &str, p: &std::path::Path| {
            Node::new(id, "raster.read").param("path", p.to_string_lossy().as_ref()).param("band", "0")
        };
        s.source.nodes = vec![read("land", &land), read("geo", &geo)];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "geo"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    // the middle of the map shown (a lattice of screen points), as the GPU drew it
    let picture = |h: &mut Harness<TerrafluxApp>| -> Vec<[u8; 4]> {
        drawn(h);
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(500) {
            h.step();
            std::thread::sleep(Duration::from_millis(10));
        }
        drawn(h);
        let s = h.state().session.as_ref().unwrap();
        let (rect, (zoom, centre)) = (s.preview_view.rect().expect("the map drawn"), s.preview_view.camera());
        let (w, ht) = s.preview_view.shown_grid().map(|g| (g.width as f32, g.height as f32)).unwrap();
        let img = h.render().expect("the window drawn by wgpu");
        let mut out = Vec::new();
        for i in 0..40 {
            for j in 0..40 {
                let cell = egui::Vec2::new(w * (0.2 + 0.6 * j as f32 / 39.0), ht * (0.2 + 0.6 * i as f32 / 39.0));
                let at = rect.center() + (cell - centre) * zoom;
                out.push(img.get_pixel(at.x as u32, at.y as u32).0);
            }
        }
        out
    };
    show(&mut h, "land", "raster");
    let alone = picture(&mut h);
    let blended = |h: &mut Harness<TerrafluxApp>, blend: Blend| {
        h.state_mut().session.as_mut().unwrap().preview_layers =
            vec![Layer::new("land", "raster"), Layer { blend, ..Layer::new("geo", "raster") }];
        picture(h)
    };
    // normal: the layer's own colours over the land's (the same warp as the blended ones, so they line up)
    let top = blended(&mut h, Blend::Normal);
    // the share of the lattice where the picture is `f` of the land's colour and the layer's, within 6 per channel
    let share = |got: &[[u8; 4]], f: &dyn Fn(f32, f32) -> f32| {
        let hit = |(p, (a, n)): (&[u8; 4], (&[u8; 4], &[u8; 4]))| {
            (0..3).all(|k| {
                let want = f(f32::from(a[k]) / 255.0, f32::from(n[k]) / 255.0) * 255.0;
                (f32::from(p[k]) - want).abs() <= 6.0
            })
        };
        // (the cells with no value, the grey of the panel behind the map, have nothing to blend: left out)
        let cells = || got.iter().zip(alone.iter().zip(&top)).filter(|(_, (a, _))| a[..3] != [40, 40, 40]);
        cells().filter(|(p, e)| hit((*p, (e.0, e.1)))).count() as f32 / cells().count().max(1) as f32
    };
    // (the layer covers the land: most of the lattice is the layer's colours, and these are not white or black)
    assert!(share(&top, &|_, n| n) > 0.99, "normal is the layer's colours");
    let multiplied = blended(&mut h, Blend::Multiply);
    let got = share(&multiplied, &|a, n| a * n);
    assert!(got > 0.9, "the product of the two is on the lattice: {got}");
    assert!(share(&multiplied, &|_, n| n) < 0.5, "darker than the layer");
    let lighter = blended(&mut h, Blend::Screen);
    let got = share(&lighter, &|a, n| a + n - a * n);
    assert!(got > 0.9, "the screen of the two is on the lattice: {got}");
    assert!(share(&lighter, &|_, n| n) < 0.5, "lighter than the layer");
}

/// Two maps of two coordinate systems side by side: each in a view of its own grid, the views following each other
/// through the transformation (the centre of one is the other's, carried) when one is zoomed.
#[test]
fn views_of_maps_of_two_coordinate_systems_follow_each_other() {
    use terraflux_app::builder::{CompareWith, How};
    use terraflux_app::reproject::Link;
    use terraflux_graph::Node;
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let d = tmp("side_by_side_crs");
    std::fs::create_dir_all(&d).unwrap();
    let (land, geo) = (d.join("land.tif"), d.join("land_geo.tif"));
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &["-a_srs".to_string(), "EPSG:32651".to_string()])
        .unwrap();
    terraflux_gdal::warp(&[&land], &geo, &["-t_srs".into(), "EPSG:4326".into(), "-r".into(), "near".into()]).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = |id: &str, p: &std::path::Path| {
            Node::new(id, "raster.read").param("path", p.to_string_lossy().as_ref()).param("band", "0")
        };
        s.source.nodes = vec![read("land", &land), read("geo", &geo)];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "geo"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = Some(("land".into(), "raster".into()));
        s.preview_compare.with = Some(CompareWith::Output("geo".into(), "raster".into()));
        s.preview_compare.how = How::SideBySide;
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "both views drawn", |a| {
        let s = a.session.as_ref().unwrap();
        s.preview_view.shown_grid().is_some() && s.preview_compare.view.shown_grid().is_some()
    });
    drawn(&mut h);
    shot(&mut h, "14c5-side-by-side-two-systems");
    // the compared view's centre is the shown view's, carried; its cells are of its own grid (degrees)
    let carried = |h: &Harness<TerrafluxApp>| {
        let s = h.state().session.as_ref().unwrap();
        let (a, b) = (s.preview_view.camera(), s.preview_compare.view.camera());
        let link = Link::between(s.preview_compare.view.shown_grid().unwrap(), s.preview_view.shown_grid().unwrap())
            .expect("a link");
        assert!(matches!(link, Link::Warp(_)), "two coordinate systems");
        (a, b, link.to_base(b.1))
    };
    let (a, b, base) = carried(&h);
    assert!((base - a.1).length() < 1.0, "the compared view's centre carried is the shown one's: {a:?} {b:?} {base:?}");
    // zoom the shown view with the wheel over it: the other follows
    let rect = h.state().session.as_ref().unwrap().preview_view.rect().expect("drawn");
    h.event(egui::Event::PointerMoved(rect.center() + egui::vec2(30.0, 20.0)));
    h.run_steps(2);
    h.event(egui::Event::Zoom(1.6));
    h.run_steps(2);
    until(&mut h, "the other view following", |app| {
        let s = app.session.as_ref().unwrap();
        s.preview_view.camera().0 > 1.3 * a.0 && s.preview_compare.view.camera().0 > 1.3 * b.0
    });
    h.run_steps(2);
    let (a2, b2, base2) = carried(&h);
    assert!(a2.0 > a.0 * 1.3, "the shown view zoomed in: {a:?} -> {a2:?}");
    assert!((base2 - a2.1).length() < 1.0, "the compared view's centre carried is the shown one's: {a2:?} {b2:?}");
    assert!(b2.0 > b.0 * 1.3, "and the other zoomed in with it: {b:?} -> {b2:?}");
    shot(&mut h, "14c5-side-by-side-two-systems-zoomed");
}

/// The difference of two bands of one stack kept as a layer: the band of each side in the layer, the Plot and the
/// cache apart from another pair's, back from the Plot.
#[test]
fn the_difference_of_two_bands_of_a_stack_is_a_layer() {
    use terraflux_app::builder::{CompareWith, How, Layer};
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes = vec![
            Node::new("drivers", "raster.read").param("path", data("sibuyan_drivers.tif").to_string_lossy().as_ref()),
        ];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("drivers", "raster").is_some()
    });
    show(&mut h, "drivers", "raster");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_compare.with = Some(CompareWith::Layer(2));
        s.preview_compare.how = How::Difference;
    }
    until(&mut h, "the difference", |a| a.session.as_ref().unwrap().preview_compare.difference.made.is_some());
    drawn(&mut h);
    // kept as a layer: the third band minus the first, of the same stack
    h.get_by_label("📌 Keep as a layer").scroll_to_me();
    h.run_steps(30);
    h.get_by_label("📌 Keep as a layer").click();
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.preview_compare.with.is_none(), "the comparison let go");
        assert_eq!(s.preview_layers.len(), 1);
        let l = &s.preview_layers[0];
        assert_eq!((l.node.as_str(), l.port.as_str(), l.band), ("drivers", "raster", Some(2)));
        assert_eq!(l.minus, Some(("drivers".to_string(), "raster".to_string())));
        assert_eq!(l.minus_band, Some(0));
        assert_ne!(
            l.difference_key(),
            Layer::difference_of_bands("drivers", "raster", Some(2), ("drivers", "raster", Some(1))).difference_key()
        );
    }
    until(&mut h, "the difference layer", |a| {
        let s = a.session.as_ref().unwrap();
        s.layer_differences.values().any(|d| matches!(d.made, Some(Ok(_))))
    });
    drawn(&mut h);
    shot(&mut h, "14i-difference-of-bands");
    // the band names of the stack in its name, the output once
    {
        let s = h.state().session.as_ref().unwrap();
        let Some(terraflux_ops::Value::Raster(r)) = s.value_of("drivers", "raster") else { panic!("rasters") };
        let (third, first) = (r.layers[2].name.clone(), r.layers[0].name.clone());
        assert_eq!(s.layer_differences.len(), 1);
        let names: Vec<String> = r.layers.iter().map(|l| l.name.clone()).collect();
        let name = s.preview_layers[0].name_by(None, |_, _, b| names[b].clone());
        assert_eq!(name, format!("drivers {third} minus {first}"));
    }
    // kept in a Plot, and back from it
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let s = h.state_mut().session.as_mut().unwrap();
    let p = s.source.plot("plot_drivers").expect("the Plot").clone();
    let l = p.layers.iter().find(|l| l.minus.is_some()).expect("the difference");
    assert_eq!(
        (l.from.as_str(), l.band, l.minus.as_deref(), l.minus_band),
        ("drivers.raster", Some(2), Some("drivers.raster"), Some(0))
    );
    let before = s.preview_layers[0].difference_key();
    s.preview_layers.clear();
    terraflux_app::builder::open_plot(s, &terraflux_ops::Registry::standard(), "plot_drivers");
    let back = s.preview_layers.iter().find(|l| l.minus.is_some()).expect("the difference back");
    assert_eq!(back.difference_key(), before);
}

/// A view side by side with layers of its own: drawn there and not in the others, kept in a Plot and back.
#[test]
fn a_view_side_by_side_has_layers_of_its_own() {
    use terraflux_app::builder::{Beside, CompareWith, How, Layer};
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    let read = || CompareWith::Output("sibuyan_landuse".into(), "raster".into());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 0".into())]).unwrap();
        s.preview = Some(("raster_reclassify".into(), "map".into()));
        s.preview_compare.with = Some(read());
        s.preview_compare.how = How::SideBySide;
        s.preview_compare.more.push(Beside::new(read()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("raster_reclassify", "map").is_some()
            && s.value_of("sibuyan_landuse", "raster").is_some()
    });
    // the views draw the Viewer's layers (here none), then the compared one has a layer of its own
    let drawn_in = |h: &Harness<TerrafluxApp>| {
        let s = h.state().session.as_ref().unwrap();
        [
            s.preview_view.layers_drawn(),
            s.preview_compare.view.layers_drawn(),
            s.preview_compare.more[0].view.layers_drawn(),
        ]
    };
    drawn(&mut h);
    assert_eq!(drawn_in(&h), [(0, 0); 3]);
    h.state_mut().session.as_mut().unwrap().preview_compare.layers = Some(vec![Layer::new("raster_reclassify", "map")]);
    until(&mut h, "the layer of the view", |a| {
        a.session.as_ref().unwrap().preview_compare.view.layers_drawn() == (1, 0)
    });
    drawn(&mut h);
    shot(&mut h, "14c4-view-with-its-own-layers");
    assert_eq!(drawn_in(&h), [(0, 0), (1, 0), (0, 0)], "drawn in that view only");
    // kept in a Plot with the layers of the view
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let id = "plot_raster_reclassify";
    {
        let s = h.state().session.as_ref().unwrap();
        let c = s.source.plot(id).expect("the Plot").compare.clone().expect("its comparison");
        assert_eq!(c.also, ["sibuyan_landuse.raster"]);
        assert_eq!(c.layers.len(), 1, "the view more draws the Viewer's layers: none kept for it");
        assert_eq!(c.layers[0].iter().map(|l| l.from.as_str()).collect::<Vec<_>>(), ["raster_reclassify.map"]);
    }
    // let go, and opened again: the view has its layers back
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_compare.layers = None;
        s.preview_compare.more.clear();
        s.preview_compare.with = None;
        s.plot_to_open = Some(id.into());
    }
    until(&mut h, "the Plot", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.preview_compare.with.is_some() && s.preview_compare.layers.is_some()
    });
    let s = h.state().session.as_ref().unwrap();
    let own = s.preview_compare.layers.as_ref().unwrap();
    assert_eq!(
        own.iter().map(|l| (l.node.as_str(), l.port.as_str())).collect::<Vec<_>>(),
        [("raster_reclassify", "map")]
    );
    assert!(s.preview_compare.more[0].layers.is_none());
    until(&mut h, "drawn again", |a| a.session.as_ref().unwrap().preview_compare.view.layers_drawn() == (1, 0));
    drawn(&mut h);
    assert_eq!(drawn_in(&h), [(0, 0), (1, 0), (0, 0)]);
}

/// The band of another stack compared: the one the difference kept as a layer is of, then chosen for the compared
/// view and for a view more, kept in the Plot and back from it.
#[test]
fn comparing_with_a_band_of_another_stack() {
    use terraflux_app::builder::{Beside, CompareWith, How};
    use terraflux_graph::Node;
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let drivers = || CompareWith::Output("drivers".into(), "raster".into());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.nodes = vec![
            Node::new("land", "raster.read").param("path", data("sibuyan_landuse.tif").to_string_lossy().as_ref()),
            Node::new("drivers", "raster.read").param("path", data("sibuyan_drivers.tif").to_string_lossy().as_ref()),
        ];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("land", "raster").is_some() && s.value_of("drivers", "raster").is_some()
    });
    show(&mut h, "land", "raster");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let Some(terraflux_ops::Value::Raster(r)) = s.value_of("drivers", "raster") else { panic!("rasters") };
        assert!(r.layers.len() > 2, "a stack");
        s.preview_compare.with = Some(drivers());
        s.preview_compare.band = Some(2);
        s.preview_compare.how = How::Difference;
    }
    until(&mut h, "the difference", |a| a.session.as_ref().unwrap().preview_compare.difference.made.is_some());
    drawn(&mut h);
    // the difference kept as a layer: the band compared is the layer's own
    // (a taller window: the side panel with the band choice is longer than the default one)
    h.set_size(egui::vec2(1280.0, 1100.0));
    h.run_steps(2);
    h.get_by_label("📌 Keep as a layer").scroll_to_me();
    h.run_steps(30);
    h.get_by_label("📌 Keep as a layer").click();
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let l = s.preview_layers.iter().find(|l| l.minus.is_some()).expect("the difference layer");
        assert_eq!((l.node.as_str(), l.port.as_str(), l.band), ("drivers", "raster", Some(2)));
        assert_eq!(l.minus, Some(("land".to_string(), "raster".to_string())));
        assert!(s.preview_compare.with.is_none(), "the comparison let go");
        s.preview_layers.clear();
        // side by side, the compared map and a view more each a band of its stack
        s.preview_compare.with = Some(drivers());
        s.preview_compare.band = Some(2);
        s.preview_compare.how = How::SideBySide;
        s.preview_compare.more.push(Beside { band: Some(1), ..Beside::new(drivers()) });
    }
    drawn(&mut h);
    h.run_steps(3);
    // kept in a Plot: the band of the compared map and of the view more
    h.get_by_label("Plots").click();
    h.run_steps(2);
    h.get_by_label("Save as a new plot").click();
    h.run_steps(2);
    let id = "plot_land";
    {
        let s = h.state().session.as_ref().unwrap();
        let c = s.source.plot(id).expect("the Plot").compare.clone().expect("its comparison");
        assert_eq!((c.with.as_str(), c.band), ("drivers.raster", Some(2)));
        assert_eq!((c.also, c.also_bands), (vec!["drivers.raster".to_string()], vec![1]));
    }
    // let go, and opened again: the bands are back
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_compare.more.clear();
        s.preview_compare.band = None;
        s.preview_compare.with = None;
        s.plot_to_open = Some(id.into());
    }
    until(&mut h, "the Plot", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.preview_compare.with.is_some() && !s.preview_compare.more.is_empty()
    });
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.preview_compare.band, Some(2));
    assert_eq!(s.preview_compare.more[0].band, Some(1));
}

/// The difference of two maps of different coordinate systems: the other map carried to this one's grid, made and
/// kept as a layer too.
#[test]
fn the_difference_of_maps_of_two_coordinate_systems_is_made() {
    use terraflux_app::builder::{CompareWith, How};
    use terraflux_graph::Node;
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let d = tmp("difference_crs");
    std::fs::create_dir_all(&d).unwrap();
    let (land, geo) = (d.join("land.tif"), d.join("land_geo.tif"));
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &land, &["-a_srs".to_string(), "EPSG:32651".to_string()])
        .unwrap();
    terraflux_gdal::warp(&[&land], &geo, &["-t_srs".into(), "EPSG:4326".into(), "-r".into(), "near".into()]).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = |id: &str, p: &std::path::Path| {
            Node::new(id, "raster.read").param("path", p.to_string_lossy().as_ref()).param("band", "0")
        };
        s.source.nodes = vec![read("land", &land), read("geo", &geo)];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "geo"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = Some(("land".into(), "raster".into()));
        s.preview_compare.with = Some(CompareWith::Output("geo".into(), "raster".into()));
        s.preview_compare.how = How::Difference;
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the difference", |a| a.session.as_ref().unwrap().preview_compare.difference.made.is_some());
    h.run_steps(4);
    drawn(&mut h);
    shot(&mut h, "14c6-difference-two-systems");
    {
        let s = h.state().session.as_ref().unwrap();
        let d = s.preview_compare.difference.made.as_ref().unwrap().as_ref().expect("the difference made");
        let terraflux_app::map_view::Source::Map(m) = &d.source else { panic!("a map") };
        assert!(m.data.iter().any(|v| v.is_finite()), "cells with a value in both maps: {}", d.summary);
    }
    // (the legend of many classes pushes the button down the side panel)
    h.get_by_label("📌 Keep as a layer").scroll_to_me();
    h.run_steps(20);
    h.get_by_label("📌 Keep as a layer").scroll_to_me();
    h.run_steps(30);
    h.get_by_label("📌 Keep as a layer").click();
    h.run_steps(2);
    until(&mut h, "the difference layer", |a| {
        let s = a.session.as_ref().unwrap();
        s.layer_differences.values().any(|d| matches!(d.made, Some(Ok(_))))
    });
    h.run_steps(4);
    drawn(&mut h);
}

/// The median and the 95th percentile of frame times, in milliseconds.
fn median_p95(mut t: Vec<Duration>) -> (f64, f64) {
    t.sort();
    let ms = |d: Duration| d.as_secs_f64() * 1000.0;
    (ms(t[t.len() / 2]), ms(t[(t.len() * 95 / 100).min(t.len() - 1)]))
}

/// The Viewer's frame times with 1 and with 5 layers on rasters of 4000 x 4000 cells (the graph's are timed by
/// `canvas::timings`): the first drawing (the layers shown until their tiles are made), the steady frames, and the
/// frames while panning and zooming. The 5 layers are four maps (the last three at half opacity) and a layer of 300
/// polygons. A frame is `Harness::step` (the interface and the tiles); the GPU's painting is `render`, timed apart.
///
/// `cargo test --release -p terraflux-app --test ui viewer_timings -- --ignored --nocapture`
#[test]
#[ignore = "timings: run with --release --ignored --nocapture"]
fn viewer_timings() {
    use terraflux_app::builder::Layer;
    use terraflux_graph::Node;
    const SIDE: usize = 4000;
    // 300 squares of 20 to 220 cells over the map's extent (the cells are of one unit, its origin at the bottom left)
    let mut seed = 12345_u64;
    let mut next = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };
    let geometries = (0..300)
        .map(|_| {
            let (x, y, w) = (next() * 3700.0, next() * 3700.0, 20.0 + next() * 200.0);
            terraflux_raster::vector::Geometry::Polygon(vec![vec![
                [x, y],
                [x + w, y],
                [x + w, y + w],
                [x, y + w],
                [x, y],
            ]])
        })
        .collect();
    let gpkg = tmp("viewer-timings.gpkg");
    let _ = std::fs::remove_file(&gpkg);
    let v = terraflux_raster::vector::Vector { geometries, attributes: Vec::new(), crs: None };
    terraflux_gdal::write_vector(&gpkg, "squares", &v).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let mut nodes: Vec<Node> = (0..4)
            .map(|i| {
                Node::new(&format!("m{i}"), "nlm.random")
                    .param("rows", SIDE as i64)
                    .param("columns", SIDE as i64)
                    .param("seed", i + 1)
            })
            .collect();
        nodes.push(Node::new("squares", "vector.read").param("path", gpkg.to_string_lossy().as_ref()));
        s.source.nodes = nodes;
        s.edited();
        s.start(Box::new(|| {}));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && (0..4).all(|i| s.value_of(&format!("m{i}"), "map").is_some())
            && s.value_of("squares", "vector").is_some()
    });
    println!(
        "{:<10} {:>12} {:>16} {:>16} {:>16} {:>14}",
        "layers", "first (ms)", "steady med/p95", "pan med/p95", "zoom med/p95", "render (ms)"
    );
    for layers in [1, 5] {
        {
            let s = h.state_mut().session.as_mut().unwrap();
            s.preview = Some(("m0".into(), "map".into()));
            s.preview_layers = if layers == 1 {
                Vec::new()
            } else {
                let half = |n: &str| Layer { opacity: 0.5, ..Layer::new(n, "map") };
                vec![Layer::new("m0", "map"), half("m1"), half("m2"), half("m3"), Layer::new("squares", "vector")]
            };
            // (a view fitted again, its tiles made again)
            s.preview_view = Default::default();
        }
        // the first drawing: from the layers chosen until the maps are coloured and nothing is busy
        // (stepping with no pause, three frames at least, the view shown and fitted)
        let t = Instant::now();
        let mut frames = 0;
        while frames < 3 || {
            let v = &h.state().session.as_ref().unwrap().preview_view;
            v.busy() || v.shown_grid().is_none()
        } {
            h.step();
            frames += 1;
            assert!(t.elapsed() < Duration::from_secs(120), "the first drawing: too long");
        }
        let first = t.elapsed();
        drawn(&mut h);
        let g = h.state().session.as_ref().unwrap().preview_view.shown_grid().cloned().unwrap();
        assert_eq!((g.width, g.height), (SIDE, SIDE), "the map shown");
        shot(&mut h, &format!("viewer-timings-{layers}"));
        // steady: nothing moves
        let steady: Vec<Duration> = (0..60)
            .map(|_| {
                let t = Instant::now();
                h.step();
                t.elapsed()
            })
            .collect();
        let rect = h.state().session.as_ref().unwrap().preview_view.rect().expect("drawn");
        let c = rect.center();
        // panning: the primary button held and the pointer moved a few points a frame, across and back
        h.event(egui::Event::PointerMoved(c));
        h.run_steps(1);
        h.event(egui::Event::PointerButton {
            pos: c,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: egui::Modifiers::NONE,
        });
        h.run_steps(1);
        let mut pan = Vec::new();
        for k in 1..=60 {
            let dx = if k <= 30 { k as f32 * 6.0 } else { (60 - k) as f32 * 6.0 };
            h.event(egui::Event::PointerMoved(c + egui::vec2(dx, dx * 0.5)));
            let t = Instant::now();
            h.step();
            pan.push(t.elapsed());
        }
        h.event(egui::Event::PointerButton {
            pos: c,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: egui::Modifiers::NONE,
        });
        drawn(&mut h);
        // zooming: the wheel over the map, in and out
        h.event(egui::Event::PointerMoved(c + egui::vec2(40.0, 20.0)));
        h.run_steps(1);
        let mut zoom = Vec::new();
        for k in 0..60 {
            h.event(egui::Event::Zoom(if k < 30 { 1.08 } else { 1.0 / 1.08 }));
            let t = Instant::now();
            h.step();
            zoom.push(t.elapsed());
        }
        drawn(&mut h);
        let mut render = Vec::new();
        for _ in 0..5 {
            let t = Instant::now();
            h.render().expect("the window drawn by wgpu");
            render.push(t.elapsed());
        }
        let (render, _) = median_p95(render);
        let pair = |t: Vec<Duration>| {
            let (m, p) = median_p95(t);
            format!("{m:.1} / {p:.1}")
        };
        println!(
            "{:<10} {:>12.0} {:>16} {:>16} {:>16} {:>14.0}",
            layers,
            first.as_secs_f64() * 1000.0,
            pair(steady),
            pair(pan),
            pair(zoom),
            render
        );
    }
}

/// A map turned by 30 degrees drawn over the map shown, in the same coordinate system (none told): its tiles are
/// meshes on the view's cells, not skipped; the pixels of the cells it covers change, those it does not cover
/// do not, and the pointer's cell is carried back to the map's by the inverse of its transform.
#[test]
fn a_rotated_map_layer_is_drawn() {
    use terraflux_app::builder::Layer;
    use terraflux_app::reproject::Link;
    use terraflux_graph::Node;
    let d = tmp("rotated");
    std::fs::create_dir_all(&d).unwrap();
    let (land, rot) = (data("sibuyan_landuse.tif"), d.join("rotated.tif"));
    // the layer: a square of 60% of the land's smaller side, turned 30 degrees about the land's middle;
    // its values a ramp (classes would share colours with the land's)
    let base = terraflux_raster::Raster::read(&land).unwrap().grid;
    let n = ((base.width.min(base.height) as f64) * 0.6) as usize;
    let (c, s) = (30f64.to_radians().cos() * base.transform[1], 30f64.to_radians().sin() * base.transform[1]);
    let mut grid = terraflux_raster::Grid::new(n, n);
    let half = n as f64 / 2.0;
    let middle = [
        base.transform[0] + base.width as f64 / 2.0 * base.transform[1],
        base.transform[3] + base.height as f64 / 2.0 * base.transform[5],
    ];
    grid.transform = [middle[0] - half * (c + s), c, s, middle[1] - half * (s - c), s, -c];
    let values = (0..n * n).map(|k| (k % n) as f64 * 0.37 + 0.123).collect();
    terraflux_raster::Raster::single(grid.clone(), values, terraflux_raster::DataType::F32, None).write(&rot).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = |id: &str, p: &std::path::Path| {
            Node::new(id, "raster.read").param("path", p.to_string_lossy().as_ref()).param("band", "0")
        };
        s.source.nodes = vec![read("land", &land), read("rot", &rot)];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "rot"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    // the colours of the middle of the map shown (a lattice of cells from 20% to 80% across), with their cells
    let picture = |h: &mut Harness<TerrafluxApp>| -> Vec<(egui::Vec2, [u8; 4])> {
        drawn(h);
        let t = Instant::now();
        while t.elapsed() < Duration::from_millis(500) {
            h.step();
            std::thread::sleep(Duration::from_millis(10));
        }
        drawn(h);
        let s = h.state().session.as_ref().unwrap();
        let (rect, (zoom, centre)) = (s.preview_view.rect().expect("the map drawn"), s.preview_view.camera());
        let (w, ht) = s.preview_view.shown_grid().map(|g| (g.width as f32, g.height as f32)).unwrap();
        let img = h.render().expect("the window drawn by wgpu");
        let mut out = Vec::new();
        for i in 0..40 {
            for j in 0..40 {
                let cell = egui::Vec2::new(w * (0.2 + 0.6 * j as f32 / 39.0), ht * (0.2 + 0.6 * i as f32 / 39.0));
                let at = rect.center() + (cell - centre) * zoom;
                out.push((cell, img.get_pixel(at.x as u32, at.y as u32).0));
            }
        }
        out
    };
    show(&mut h, "land", "raster");
    let alone = picture(&mut h);
    h.state_mut().session.as_mut().unwrap().preview_layers =
        vec![Layer::new("land", "raster"), Layer::new("rot", "raster")];
    let over = picture(&mut h);
    shot(&mut h, "14i-rotated");
    // the layer is not skipped: no refusal, and the shown grid's link to it is a warp
    let s = h.state().session.as_ref().unwrap();
    let shown = s.preview_view.shown_grid().unwrap().clone();
    let link = Link::between(&grid, &shown).expect("a rotated grid is put on the map's");
    assert!(matches!(link, Link::Warp(_)));
    // the cells inside the layer (by the inverse of its transform) are the pixels that changed
    let inside = |cell: egui::Vec2| {
        let o = link.to_own(cell);
        o.x >= 0.0 && o.y >= 0.0 && o.x < n as f32 && o.y < n as f32
    };
    let changed = |p: &[u8; 4], q: &[u8; 4]| (0..3).any(|k| p[k].abs_diff(q[k]) > 8);
    let (mut agree, mut covered) = (0, 0);
    for ((cell, a), (_, b)) in alone.iter().zip(&over) {
        covered += usize::from(inside(*cell));
        agree += usize::from(inside(*cell) == changed(a, b));
    }
    assert!((160..1400).contains(&covered), "the layer covers part of the lattice: {covered} of {}", alone.len());
    assert!(agree as f32 / alone.len() as f32 > 0.9, "the pixels changed are the layer's: {agree} of {}", alone.len());
}

#[test]
fn the_transition_matrix_of_a_difference() {
    let mut h = harness(new_app(None));
    h.set_size(egui::vec2(1280.0, 1000.0));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 0".into())]).unwrap();
        s.preview = Some(("raster_reclassify".into(), "map".into()));
        s.preview_compare.with =
            Some(terraflux_app::builder::CompareWith::Output("sibuyan_landuse".into(), "raster".into()));
        s.preview_compare.how = terraflux_app::builder::How::Difference;
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the difference", |a| {
        matches!(a.session.as_ref().unwrap().preview_compare.difference.made, Some(Ok(_)))
    });
    h.run_steps(4);
    // the matrix: rows the classes of the map shown (0 and 1), columns the land use's; the cells add up
    let (m, total) = {
        let s = h.state().session.as_ref().unwrap();
        let d = s.preview_compare.difference.made.clone().unwrap().unwrap();
        let m = d.matrix.clone().expect("maps of classes: a matrix");
        let total: usize = d.counts.iter().map(|(_, n)| n).sum();
        (m, total)
    };
    assert_eq!(m.from.iter().map(|(v, _)| *v).collect::<Vec<_>>(), vec![0, 1]);
    assert!(m.to.len() > 2, "{:?}", m.to);
    assert_eq!(m.total(false) as usize, total);
    assert!(m.area.is_none(), "a map without a coordinate system: no area, the cells");
    // (forest stays forest: class 1 of both)
    let one = m.to.iter().position(|(v, _)| *v == 1).unwrap();
    assert_eq!(m.cells[0][one], 0.0, "no cell of 0 is forest in the land use");
    h.get_by_label("Transition matrix...").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().preview_compare.difference.matrix.open);
    h.get_by_label("Gains and losses").click();
    h.run_steps(2);
    shot(&mut h, "14c4-transition-budget");
    h.get_by_label("Flows").click();
    h.run_steps(2);
    shot(&mut h, "14c5-transition-flows");
    h.get_by_label("Matrix").click();
    h.run_steps(2);
    // a transition singled out: only its cells on the map
    let pair = (0, m.to.iter().find(|(v, _)| *v != 1).unwrap().0);
    h.state_mut().session.as_mut().unwrap().preview_compare.difference.focus = Some(pair);
    until(&mut h, "the transition singled out", |a| {
        a.session.as_ref().unwrap().preview_compare.difference.focused.as_ref().is_some_and(|(p, _)| *p == pair)
    });
    h.run_steps(4);
    until(&mut h, "the transition drawn", |a| !a.session.as_ref().unwrap().preview_compare.view.busy());
    h.run_steps(30);
    shot(&mut h, "14c6-transition-singled-out");
    let s = h.state().session.as_ref().unwrap();
    let d = s.preview_compare.difference.shown().unwrap().as_ref().unwrap();
    let i = m.from.iter().position(|(v, _)| *v == pair.0).unwrap();
    let j = m.to.iter().position(|(v, _)| *v == pair.1).unwrap();
    assert_eq!(d.counts[1], (1.0, m.cells[i][j] as usize));
    h.get_by_label("Show all").scroll_to_me();
    h.run_steps(30);
    h.get_by_label("Show all").click();
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().preview_compare.difference.focus.is_none());
}

#[test]
fn the_area_of_the_classes_in_a_region_and_a_profile() {
    use terraflux_app::region::{Part, of};
    let mut h = harness(new_app(None));
    h.set_size(egui::vec2(1280.0, 1100.0));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        s.preview = Some(("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("sibuyan_landuse", "raster").is_some()
    });
    drawn(&mut h);
    let src = {
        let s = h.state().session.as_ref().unwrap();
        terraflux_app::builder::first_source(&s.value_of("sibuyan_landuse", "raster").unwrap()).unwrap()
    };
    // (a number as the legend writes it: its thousands apart by a thin space)
    fn grouped(v: f64) -> String {
        let t = format!("{}", v.round());
        let n = t.len();
        let mut out = String::new();
        for (i, c) in t.chars().enumerate() {
            if i > 0 && (n - i) % 3 == 0 {
                out.push('\u{2009}');
            }
            out.push(c);
        }
        out
    }
    // the whole map: every class, its cells (no coordinate system: no area)
    let whole = of(&src, &Part::Map, true);
    assert_eq!(whole.classes.len(), 5);
    let said = format!("total: {} cells with a value", grouped(whole.cells));
    let t = Instant::now();
    while h.query_by_label_contains(&said).is_none() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "{said}: not shown");
    }
    // a polygon drawn with Measure: only the cells inside it
    let square = vec![[0.0, 0.0], [64.0, 0.0], [64.0, 54.0], [0.0, 54.0]];
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        v.set_measuring(true);
        for p in &square {
            v.measure_click(*p);
        }
    }
    h.run_steps(2);
    h.get_by_label("polygon").click();
    let inside = of(&src, &Part::Polygons(vec![square.clone()]), true);
    assert!(inside.cells > 0.0 && inside.cells < whole.cells, "{} of {}", inside.cells, whole.cells);
    let said = format!("total: {} cells with a value", grouped(inside.cells));
    let t = Instant::now();
    while h.query_by_label_contains(&said).is_none() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "{said}: not shown");
    }
    // the profile along the line (its four points), the classes crossed
    let t = Instant::now();
    while h.query_by_label_contains("along the line").is_none() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "no profile");
    }
    shot(&mut h, "14d1-area-and-profile");
    let p = terraflux_app::profile::of(&src, &square);
    assert!(p.dist.len() > 100 && p.values.iter().any(|v| v.is_finite()));
    // the part in view
    h.get_by_label("view").click();
    let t = Instant::now();
    while h.query_all_by_label_contains("cells with a value").count() == 0 {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "the view's classes not counted");
    }
}

#[test]
fn the_area_of_the_classes_along_the_steps_and_the_flows_from_the_first() {
    let mut h = harness(new_app(None));
    h.set_size(egui::vec2(1280.0, 1300.0));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        s.preview = Some(("sibuyan_landuse".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("sibuyan_landuse", "raster").is_some()
    });
    drawn(&mut h);
    // the two years (1997 and 2011): the classes counted at each, then the window of the flows from 1997 to 2011
    let t = Instant::now();
    while h.query_by_label("1997 to 2011...").is_none() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "the steps' classes not counted");
    }
    h.get_by_label("1997 to 2011...").click();
    let t = Instant::now();
    while h.query_by_label_contains("Transitions: 1997 to 2011").is_none() {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "no window of the transitions");
    }
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.steps_window.open);
    assert_eq!(s.steps_window.tab, terraflux_app::transition_view::Tab::Sankey, "the flows first");
    shot(&mut h, "14k-steps-flows");
}

/// Rasters selected together go into a stack.of at once: a connection drawn from one of them takes the others
/// with it (each a new input, in their order on the canvas), and the menu of the stack.of connects them too.
#[test]
fn the_nodes_selected_are_connected_together() {
    let path = tmp("ui-connect-many.toml");
    let mut m = terraflux_graph::Model::new("many");
    for (k, id) in ["a", "b", "c"].iter().enumerate() {
        let mut n = terraflux_graph::Node::new(id, "raster.read")
            .param("path", data("sibuyan_landuse.tif").to_string_lossy().into_owned());
        n.pos = Some([0.0, 160.0 * k as f32]);
        m.nodes.push(n);
    }
    let mut n = terraflux_graph::Node::new("stack", "stack.of");
    n.pos = Some([420.0, 120.0]);
    m.nodes.push(n);
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().focus(Pane::Model);
    h.run_steps(3);
    let choose = |h: &mut Harness<TerrafluxApp>, ids: &[&str]| {
        h.state_mut().session.as_mut().unwrap().editor.chosen = ids.iter().map(|s| s.to_string()).collect();
        h.run_steps(2);
    };
    // a connection drawn from b's output, the three selected, let go on the stack: all three, in order
    choose(&mut h, &["a", "b", "c"]);
    let (from, to) = {
        let e = &h.state().session.as_ref().unwrap().editor;
        (e.screen_of_output("b", "raster").unwrap(), e.screen_of("stack").unwrap())
    };
    h.event(egui::Event::PointerMoved(from));
    h.run_steps(1);
    h.event(egui::Event::PointerButton {
        pos: from,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(1);
    for k in 1..=6 {
        h.event(egui::Event::PointerMoved(from + (to - from) * (k as f32 / 6.0)));
        h.run_steps(1);
    }
    shot(&mut h, "14l-connect-many");
    h.event(egui::Event::PointerButton {
        pos: to,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        let n = s.source.node("stack").unwrap();
        let read: Vec<(&str, &terraflux_graph::Source)> = n.inputs.iter().map(|(k, v)| (k.as_str(), v)).collect();
        assert_eq!(read.len(), 3, "{read:?}");
        for id in ["a", "b", "c"] {
            assert_eq!(n.inputs[id], terraflux_graph::Source::Ref(format!("{id}.raster")), "{read:?}");
        }
        assert!(s.editor.message.is_none(), "{:?}", s.editor.message);
    }
    // one step of undo takes the three away
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), false);
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().source.node("stack").unwrap().inputs.is_empty());
    // the stack's menu, a and c selected: both connected
    choose(&mut h, &["a", "c"]);
    let at = h.state().session.as_ref().unwrap().editor.screen_of("stack").unwrap();
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
    h.get_by_label("Connect the 2 nodes selected here").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let n = s.source.node("stack").unwrap();
    assert_eq!(n.inputs.keys().map(String::as_str).collect::<Vec<_>>(), ["a", "c"]);
}

/// A group looks like one: two cards behind its box, an accent border and header, a stack icon and the count of
/// its nodes (a node alone has none of it). Going in zooms into its box, going out back (the frames of the
/// transition are drawn, each a little further), and it is over soon, with no frame requested after it.
#[test]
fn a_group_looks_like_one_and_going_in_and_out_is_a_zoom() {
    use terraflux_graph::{Body, Model, Node};
    let path = tmp("ui-group-look.toml");
    let mut m = Model::new("group look");
    let mut land = Node::new("land", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 3);
    land.pos = Some([0.0, 0.0]);
    m.nodes.push(land);
    let mut g = Node::new("smooth", terraflux_graph::GROUP).input("x", "land.map").body(Body {
        nodes: (0..5)
            .map(|k| {
                let mut n = Node::new(&format!("step{k}"), "raster.calc").param("expression", "x + 1");
                n = if k == 0 { n.input("x", "@x") } else { n.input("x", &format!("step{}.map", k - 1)) };
                n.pos = Some([260.0 * (k % 3) as f32, 130.0 * (k / 3) as f32]);
                n
            })
            .collect(),
        outputs: [("map".to_string(), "step4.map".to_string())].into(),
        ..Default::default()
    });
    g.pos = Some([300.0, 0.0]);
    m.nodes.push(g);
    let mut after = Node::new("after", "raster.calc").param("expression", "x * 2").input("x", "smooth.map");
    after.pos = Some([620.0, 0.0]);
    m.nodes.push(after);
    m.save(&path).unwrap();
    setup();
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    terraflux_app::canvas_gpu::install(&rs);
    // (short steps, to stop inside the transition)
    let mut h = Harness::builder()
        .with_size((1280.0, 820.0))
        .with_step_dt(0.04)
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), new_app(Some(path)));
    // (the harness turns egui's animations off)
    h.ctx.all_styles_mut(|s| s.animation_time = 1.0 / 12.0);
    loaded(&mut h);
    h.state_mut().focus(Pane::Model);
    h.run_steps(20);
    shot(&mut h, "14m-group-box");
    // going in: the first frames, a middle one, the end
    h.state_mut().session.as_mut().unwrap().editor.path.push("smooth".into());
    h.run_steps(2);
    assert!(h.state().session.as_ref().unwrap().editor.moving(), "the zoom is going on");
    h.render().unwrap().save(tmp("ui-14n-group-enter-start.png")).unwrap();
    h.run_steps(3);
    h.render().unwrap().save(tmp("ui-14n-group-enter-middle.png")).unwrap();
    h.run_steps(20);
    assert!(!h.state().session.as_ref().unwrap().editor.moving(), "and over");
    shot(&mut h, "14n-group-inside");
    let before = h.state().session.as_ref().unwrap().editor.zoom();
    assert!(before > 0.0);
    // going out
    terraflux_app::builder::go_up(h.state_mut().session.as_mut().unwrap(), 0);
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.moving());
    h.render().unwrap().save(tmp("ui-14o-group-leave-middle.png")).unwrap();
    h.run_steps(20);
    shot(&mut h, "14o-group-left");
    assert!(h.state().session.as_ref().unwrap().editor.path.is_empty());
}

/// A raster file read by a node (named as its file) and shown in the Viewer, drawn.
fn viewed(file: &std::path::Path, node: &str) -> Harness<'static, TerrafluxApp> {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = file.to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into()), ("band", "0".into())]).unwrap();
        s.preview = Some((node.to_string(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of(node, "raster").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    h
}

/// The Viewer's map view in a test.
fn map<'a>(h: &'a mut Harness<'_, TerrafluxApp>) -> &'a mut terraflux_app::map_view::MapView {
    &mut h.state_mut().session.as_mut().unwrap().preview_view
}

/// Step for a while (real time: the view's history counts settled views by the clock).
fn wait(h: &mut Harness<TerrafluxApp>, seconds: f64) {
    let t = Instant::now();
    while t.elapsed().as_secs_f64() < seconds {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn near(a: egui::Vec2, b: egui::Vec2, tol: f32) -> bool {
    (a - b).length() <= tol
}

/// The test data given a place in UTM 51N by GDAL (128 x 108 cells of 30 m) in a file of its own.
fn in_utm(name: &str) -> PathBuf {
    let out = tmp(name);
    let args = ["-a_srs", "EPSG:32651", "-a_ullr", "450000", "1380000", "453840", "1376760"];
    terraflux_gdal::translate(&data("sibuyan_landuse.tif"), &out, &args.map(String::from)).unwrap();
    out
}

/// The scale bar of the live map: a label in km or m on a projected map, its length in points standing for those
/// metres at the zoom; shorter when zoomed in; gone when turned off. The coordinate grid draws over it.
#[test]
fn the_scale_bar_on_the_live_map() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let mut h = viewed(&in_utm("nav_utm.tif"), "nav_utm");
    h.run_steps(2);
    let (g, zoom, center) = {
        let v = map(&mut h);
        let (zoom, center) = v.camera();
        (v.shown_grid().unwrap().clone(), zoom, center)
    };
    assert!(
        matches!(terraflux_app::measure::Frame::of(&g), terraflux_app::measure::Frame::Projected(_)),
        "the test map is projected"
    );
    let bar = map(&mut h).scale_bar().cloned().expect("a scale bar");
    assert!(bar.label.ends_with(" km") || bar.label.ends_with(" m"), "{}", bar.label);
    assert!((60.0..=150.0).contains(&bar.px), "{} points", bar.px);
    let cell = g.transform[1].abs() * g.geo.linear_unit();
    let per_point = bar.metres.unwrap() / f64::from(bar.px);
    assert!((per_point - cell / f64::from(zoom)).abs() / per_point < 1e-3, "{per_point} m a point, zoom {zoom}");
    shot(&mut h, "14p-scale-bar");
    // zoomed in eight times: a shorter bar, in the same range of points
    map(&mut h).set_camera(zoom * 8.0, center);
    h.run_steps(2);
    let closer = map(&mut h).scale_bar().cloned().expect("a scale bar");
    assert!(closer.metres.unwrap() < bar.metres.unwrap(), "{} then {}", bar.label, closer.label);
    assert!((60.0..=150.0).contains(&closer.px), "{} points", closer.px);
    // the grid over the map, and the bar still there
    map(&mut h).aids.grid = true;
    h.run_steps(2);
    shot(&mut h, "14q-coordinate-grid");
    assert!(map(&mut h).scale_bar().is_some());
    map(&mut h).aids.grid = false;
    // off: none
    map(&mut h).aids.scale_bar = false;
    h.run_steps(2);
    assert!(map(&mut h).scale_bar().is_none());
    map(&mut h).aids.scale_bar = true;
    h.run_steps(2);
    assert!(map(&mut h).scale_bar().is_some());
}

/// On a map in degrees the bar is in metres at the centre's latitude (a degree of longitude is 111.3 km times the
/// cosine of the latitude on a sphere), whatever the cell size says.
#[test]
fn the_scale_bar_of_a_map_in_degrees_is_in_metres() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let mut h = viewed(&in_degrees("nav_degrees.tif"), "nav_degrees");
    h.run_steps(2);
    let (zoom, center) = map(&mut h).camera();
    let bar = map(&mut h).scale_bar().cloned().expect("a scale bar");
    assert!(bar.label.ends_with(" m") || bar.label.ends_with(" km"), "{}", bar.label);
    assert!((60.0..=150.0).contains(&bar.px), "{} points", bar.px);
    // the cell of the middle: 0.0025 degrees of longitude at its latitude, on the sphere of a mean radius
    let lat = (12.60 - 0.0025 * f64::from(center.y)).to_radians();
    let cell = 6_371_008.8 * 0.0025f64.to_radians() * lat.cos();
    let per_point = bar.metres.unwrap() / f64::from(bar.px);
    let want = cell / f64::from(zoom);
    assert!((per_point - want).abs() / want < 0.005, "{per_point} m a point against {want}");
}

/// Back and forward along the views: a view that has been still for a moment is kept; alt+left and alt+right bring
/// the one before and the one after (going back keeps nothing new), as do the mouse's buttons and the header's
/// arrows.
#[test]
fn back_and_forward_along_the_views() {
    let mut h = viewed(&data("sibuyan_landuse.tif"), "sibuyan_landuse");
    until(&mut h, "the first view settling", |a| a.session.as_ref().unwrap().preview_view.history_len() >= 1);
    let (z0, c0) = map(&mut h).camera();
    assert!(!map(&mut h).can_back(), "one view, none before it");
    // a zoom that settles
    map(&mut h).set_camera(z0 * 3.0, c0 + egui::vec2(5.0, 4.0));
    let (z1, c1) = map(&mut h).camera();
    until(&mut h, "the zoomed view settling", |a| a.session.as_ref().unwrap().preview_view.history_len() >= 2);
    assert!(map(&mut h).can_back() && !map(&mut h).can_forward());
    let rect = map(&mut h).rect().expect("drawn");
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::ArrowLeft);
    h.run_steps(2);
    let (z, c) = map(&mut h).camera();
    assert!((z / z0 - 1.0).abs() < 1e-4 && near(c, c0, 1e-3), "alt+left: back to {z0} {c0:?}, at {z} {c:?}");
    // (it settles on the view kept: nothing is pushed, the forward stays)
    wait(&mut h, 0.7);
    assert_eq!(map(&mut h).history_len(), 2, "going back pushes nothing");
    assert!(map(&mut h).can_forward() && !map(&mut h).can_back());
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::ArrowRight);
    h.run_steps(2);
    let (z, c) = map(&mut h).camera();
    assert!((z / z1 - 1.0).abs() < 1e-4 && near(c, c1, 1e-3), "alt+right: forward to {z1} {c1:?}, at {z} {c:?}");
    // the mouse's back button, and the header's arrows
    let button = |h: &mut Harness<TerrafluxApp>, b: egui::PointerButton| {
        for pressed in [true, false] {
            let pos = rect.center();
            h.event(egui::Event::PointerButton { pos, button: b, pressed, modifiers: egui::Modifiers::NONE });
            h.run_steps(1);
        }
        h.run_steps(1);
    };
    button(&mut h, egui::PointerButton::Extra1);
    assert!((map(&mut h).zoom() / z0 - 1.0).abs() < 1e-4, "the mouse's back button");
    button(&mut h, egui::PointerButton::Extra2);
    assert!((map(&mut h).zoom() / z1 - 1.0).abs() < 1e-4, "the mouse's forward button");
    h.get_by_label("‹").click();
    h.run_steps(2);
    assert!((map(&mut h).zoom() / z0 - 1.0).abs() < 1e-4, "the back arrow");
    h.get_by_label("›").click();
    h.run_steps(2);
    assert!((map(&mut h).zoom() / z1 - 1.0).abs() < 1e-4, "the forward arrow");
    // a new view after going back cuts off the one ahead (the pointer back on the map: the arrows had it)
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::ALT, egui::Key::ArrowLeft);
    h.run_steps(2);
    map(&mut h).set_camera(z0 * 6.0, c0);
    wait(&mut h, 0.6);
    assert!(!map(&mut h).can_forward(), "the branch is cut");
    assert_eq!(map(&mut h).history_len(), 2);
}

/// Places kept with the model: ctrl+1 keeps the Viewer's view in slot 1 and 1 goes back to it (the pointer over the
/// map); the Places menu keeps one under a name; they are in the model's file; ctrl+Z takes a saved place back; the
/// canvas has its own with the same keys; a digit typed in a field is the field's.
#[test]
fn places_are_kept_with_the_model_and_gone_back_to() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT;
    let mut h = viewed(&data("sibuyan_landuse.tif"), "sibuyan_landuse");
    until(&mut h, "the first view settling", |a| a.session.as_ref().unwrap().preview_view.history_len() >= 1);
    let (z0, c0) = map(&mut h).camera();
    let rect = map(&mut h).rect().expect("drawn");
    // zoomed in somewhere, kept in slot 1 with ctrl+1
    map(&mut h).set_camera(z0 * 4.0, c0 + egui::vec2(8.0, -6.0));
    h.run_steps(2);
    let (z1, c1) = map(&mut h).camera();
    let e1 = map(&mut h).view_extent().expect("a view");
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num1);
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        let p = s.source.bookmark_at(true, 1).expect("the place of slot 1");
        assert_eq!(p.name, "Place 1");
        let terraflux_graph::PlaceView::Map { extent, layers, .. } = &p.view else { panic!("{p:?}") };
        assert!(layers.is_none());
        for k in 0..4 {
            assert!((extent[k] - e1[k]).abs() < 1e-6 * e1[k].abs().max(1.0), "{extent:?} against {e1:?}");
        }
        assert!(s.dirty, "a place is an edit of the model");
    }
    // away, then 1 brings the view back
    map(&mut h).set_camera(z0, c0);
    h.run_steps(2);
    h.key_press(egui::Key::Num1);
    h.run_steps(3);
    let (z, c) = map(&mut h).camera();
    assert!((z / z1 - 1.0).abs() < 1e-3 && near(c, c1, 0.05), "back to {z1} {c1:?}, at {z} {c:?}");
    // a digit typed in a field is the field's: the map stays where it is (a text field clicked, the pointer back
    // over the map)
    map(&mut h).set_camera(z0, c0);
    h.run_steps(2);
    let field = h
        .query_all_by_role(Role::TextInput)
        .find(|n| {
            n.accesskit_node().bounding_box().is_some_and(|b| b.x1 as f32 <= rect.left() || b.y0 as f32 > rect.bottom())
        })
        .expect("a text field outside the map");
    field.click();
    h.run_steps(2);
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(2);
    h.key_press(egui::Key::Num1);
    h.event(egui::Event::Text("1".into()));
    h.run_steps(3);
    assert_eq!(map(&mut h).camera(), (z0, c0), "1 typed in a field went to the field");
    // (the field let go)
    h.event(egui::Event::PointerMoved(egui::pos2(rect.center().x, rect.top() + 40.0)));
    h.key_press(egui::Key::Escape);
    h.run_steps(3);
    // the Places menu of the Viewer's header keeps one under a name
    let button = h.get_all_by_role_and_label(Role::Button, "Places").last().expect("the Places menu").rect();
    h.get_all_by_role_and_label(Role::Button, "Places").last().unwrap().click();
    h.run_steps(3);
    assert!(h.query_by_label("Place 1   [1]").is_some(), "slot 1's place listed with its slot");
    let name = h
        .query_all_by_role(Role::TextInput)
        .find(|n| {
            n.accesskit_node()
                .bounding_box()
                .is_some_and(|b| b.y0 as f32 > button.bottom() && b.x0 as f32 > button.left() - 200.0)
        })
        .expect("the name field of the menu");
    name.click();
    h.run_steps(1);
    h.event(egui::Event::Text("Lake".into()));
    h.run_steps(2);
    shot(&mut h, "places-menu");
    h.get_by_label("Save this place...").click();
    h.run_steps(4);
    {
        let s = h.state().session.as_ref().unwrap();
        let p = s.source.bookmarks_of(true).find(|p| p.name == "Lake").expect("the place Lake");
        assert_eq!(p.slot, None);
        assert_eq!(s.source.bookmarks_of(true).count(), 2);
        assert!(s.history.steps().0.iter().any(|n| n == "Save the place Lake"), "{:?}", s.history.steps().0);
    }
    // in the model's file, read back
    let file = tmp("places-model.toml");
    h.state_mut().session.as_mut().unwrap().save_as(&file).unwrap();
    let back = terraflux_graph::Model::load(&file).unwrap();
    assert_eq!(back.bookmarks, h.state().session.as_ref().unwrap().source.bookmarks);
    assert!(std::fs::read_to_string(&file).unwrap().contains("[[bookmarks]]"));
    // ctrl+Z takes Lake back
    h.event(egui::Event::PointerMoved(egui::pos2(rect.center().x, rect.center().y)));
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.source.bookmarks_of(true).all(|p| p.name != "Lake") && s.source.bookmark_at(true, 1).is_some());

    // the canvas: ctrl+2 keeps where it looks, 2 goes back; its places are apart from the Viewer's
    h.state_mut().focus(Pane::Model);
    h.run_steps(4);
    let canvas = h.state().session.as_ref().unwrap().editor.canvas().expect("the canvas drawn");
    let cam = h.state().session.as_ref().unwrap().editor.camera().expect("a camera");
    let look = |h: &mut Harness<TerrafluxApp>, pan: egui::Vec2, zoom: f32| {
        h.state_mut().session.as_mut().unwrap().editor.set_camera(pan, zoom);
        h.run_steps(2);
    };
    look(&mut h, cam.pan + egui::vec2(60.0, 25.0), cam.zoom * 1.5);
    let moved = h.state().session.as_ref().unwrap().editor.camera().unwrap();
    h.event(egui::Event::PointerMoved(canvas.center()));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Num2);
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.source.bookmark_at(false, 2).is_some() && s.source.bookmark_at(true, 2).is_none());
    }
    look(&mut h, cam.pan, cam.zoom);
    h.event(egui::Event::PointerMoved(canvas.center()));
    h.key_press(egui::Key::Num2);
    h.run_steps(3);
    let now = h.state().session.as_ref().unwrap().editor.camera().unwrap();
    assert!(near(now.pan, moved.pan, 0.5) && (now.zoom / moved.zoom - 1.0).abs() < 1e-3, "{moved:?} against {now:?}");
    // 1 has no place on the canvas: nothing moves (the Viewer's slot 1 is not the canvas')
    h.key_press(egui::Key::Num1);
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().editor.camera().unwrap(), now);
    // the right-click menu of the empty canvas lists them
    h.event(egui::Event::PointerButton {
        pos: canvas.left_bottom() + egui::vec2(30.0, -30.0),
        button: egui::PointerButton::Secondary,
        pressed: true,
        modifiers: egui::Modifiers::NONE,
    });
    h.event(egui::Event::PointerButton {
        pos: canvas.left_bottom() + egui::vec2(30.0, -30.0),
        button: egui::PointerButton::Secondary,
        pressed: false,
        modifiers: egui::Modifiers::NONE,
    });
    h.run_steps(3);
    h.get_by_label_contains("Places").click();
    h.run_steps(3);
    assert!(h.query_by_label("Place 2   [2]").is_some());
    shot(&mut h, "places-canvas-menu");
}

/// Go to at a zoom: a scale (chosen or typed after an `@`) or a width of view sets the metres per screen point;
/// without one the zoom is kept; the jump is one view of the history.
#[test]
fn go_to_at_a_scale_or_a_width() {
    use terraflux_app::goto::Zoom;
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    // cells of 30 m (UTM 51N): the cell (64, 54) is at (451935, 1378365)
    let mut h = viewed(&in_utm("goto_zoom.tif"), "goto_zoom");
    h.run_steps(2);
    let per_point = |h: &mut Harness<TerrafluxApp>| {
        let v = map(h);
        let (z, c) = v.camera();
        terraflux_app::nav::metres_per_point(v.shown_grid().unwrap(), c, z).unwrap()
    };
    let zoom0 = map(&mut h).zoom();
    // keep: the zoom is the same
    assert_eq!(map(&mut h).go_to_zoom("451935 1378365", false, Zoom::Keep), Ok(true));
    assert_eq!(map(&mut h).zoom(), zoom0, "keep the zoom");
    assert!(near(map(&mut h).camera().1, egui::vec2(64.5, 54.5), 1e-3));
    // a scale chosen: 1:25 000, a screen point being 1/96 inch
    assert_eq!(map(&mut h).go_to_zoom("451935 1378365", false, Zoom::Scale(25_000.0)), Ok(true));
    let want = 25_000.0 * 0.0254 / 96.0;
    let got = per_point(&mut h);
    assert!((got / want - 1.0).abs() < 0.01, "{got} m per point, wanted {want}");
    // typed: it wins over the one chosen
    assert_eq!(map(&mut h).go_to_zoom("451935 1378365 @ 1:5000", false, Zoom::Scale(100_000.0)), Ok(true));
    let want = 5_000.0 * 0.0254 / 96.0;
    let got = per_point(&mut h);
    assert!((got / want - 1.0).abs() < 0.01, "{got} m per point, wanted {want}");
    // a width of view: 1 km across the view
    assert_eq!(map(&mut h).go_to("451935 1378365 @ 1 km", false), Ok(true));
    let width = map(&mut h).rect().unwrap().width();
    let want = 1000.0 / f64::from(width);
    let got = per_point(&mut h);
    assert!((got / want - 1.0).abs() < 0.01, "{got} m per point, wanted {want}");
    // (an error in the zoom is told, the view stays)
    let before = map(&mut h).camera();
    assert!(map(&mut h).go_to("451935 1378365 @ 50000", false).unwrap_err().contains("scale"));
    assert_eq!(map(&mut h).camera(), before);
    // the box offers it too: Ctrl+G, the text, and a scale in the box
    wait(&mut h, 0.7);
    let zoom_before = map(&mut h).zoom();
    let rect = map(&mut h).rect().expect("drawn");
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::G);
    h.run_steps(3);
    {
        let b = map(&mut h).goto.as_mut().expect("the box");
        (b.zoom, b.degrees) = (Zoom::Scale(100_000.0), false);
    }
    h.event(egui::Event::Text("451935 1378365".into()));
    h.run_steps(2);
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(map(&mut h).goto.is_none(), "enter goes, the box closes");
    let want = 100_000.0 * 0.0254 / 96.0;
    let got = per_point(&mut h);
    assert!((got / want - 1.0).abs() < 0.01, "{got} m per point, wanted {want}");
    // one view of the history: the jump (centre and zoom), settled; back goes to the view before it
    wait(&mut h, 0.7);
    assert!(map(&mut h).can_back());
    map(&mut h).back();
    assert!((map(&mut h).zoom() / zoom_before - 1.0).abs() < 1e-3, "back to {zoom_before}");
}

/// Go to in views side by side (two coordinate systems): the mark is drawn in the other view too, at the same
/// ground, and escape lets it go from both.
#[test]
fn the_go_to_mark_is_in_the_views_side_by_side() {
    use terraflux_app::builder::{CompareWith, How};
    use terraflux_app::reproject::Link;
    use terraflux_graph::Node;
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let d = tmp("goto_side_by_side");
    std::fs::create_dir_all(&d).unwrap();
    // (cells of 30 m: a scale has metres to use)
    let (land, geo) = (in_utm("goto_sbs_land.tif"), d.join("land_geo.tif"));
    terraflux_gdal::warp(&[&land], &geo, &["-t_srs".into(), "EPSG:4326".into(), "-r".into(), "near".into()]).unwrap();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let read = |id: &str, p: &std::path::Path| {
            Node::new(id, "raster.read").param("path", p.to_string_lossy().as_ref()).param("band", "0")
        };
        s.source.nodes = vec![read("land", &land), read("geo", &geo)];
        s.edited();
        s.start(Box::new(|| {}));
    }
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && ["land", "geo"].iter().all(|n| s.value_of(n, "raster").is_some())
    });
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview = Some(("land".into(), "raster".into()));
        s.preview_compare.with = Some(CompareWith::Output("geo".into(), "raster".into()));
        s.preview_compare.how = How::SideBySide;
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "both views drawn", |a| {
        let s = a.session.as_ref().unwrap();
        s.preview_view.shown_grid().is_some() && s.preview_compare.view.shown_grid().is_some()
    });
    drawn(&mut h);
    // go to a cell of the UTM map (in map units), at a scale: the view beside follows
    let (x, y) = {
        let g = map(&mut h).shown_grid().unwrap().clone();
        (g.transform[0] + 60.5 * g.transform[1], g.transform[3] + 40.5 * g.transform[5])
    };
    assert_eq!(map(&mut h).go_to_zoom(&format!("{x} {y}"), false, terraflux_app::goto::Zoom::Scale(5_000.0)), Ok(true));
    h.run_steps(4);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.preview_view.mark.is_some() && s.preview_compare.view.mark.is_none());
    let link = Link::between(s.preview_compare.view.shown_grid().unwrap(), s.preview_view.shown_grid().unwrap())
        .expect("a link");
    assert!(matches!(link, Link::Warp(_)), "two coordinate systems");
    // the view beside is centred on the place, carried: its mark is drawn at its centre
    let (zoom, centre) = s.preview_compare.view.camera();
    let at = link.to_own(egui::vec2(60.5, 40.5));
    assert!((at - centre).length() < 1.0, "the place carried {at:?} is the view's centre {centre:?}");
    let beside = s.preview_compare.view.rect().unwrap();
    assert!(zoom > 0.0);
    // the mark in the picture: pixels near the centre that are not the map's (a plain class there)
    let img = h.render().expect("the window drawn by wgpu");
    let ink = |img: &image::RgbaImage, c: egui::Pos2| {
        let mut n = 0;
        for y in c.y as u32 - 16..c.y as u32 + 16 {
            for x in c.x as u32 - 16..c.x as u32 + 16 {
                let p = img.get_pixel(x, y).0;
                if p[0].abs_diff(0) > 40 || p[1].abs_diff(114) > 40 || p[2].abs_diff(178) > 40 {
                    n += 1;
                }
            }
        }
        n
    };
    let shown = h.state().session.as_ref().unwrap().preview_view.rect().unwrap();
    let (a, b) = (ink(&img, shown.center()), ink(&img, beside.center()));
    assert!(a > 40, "the mark in the view it was made in: {a} pixels");
    assert!(b > 40, "and in the view beside, of another coordinate system: {b} pixels");
    shot(&mut h, "14t-go-to-side-by-side");
    // escape over the view beside (it has no mark of its own) lets it go from both
    h.event(egui::Event::PointerMoved(beside.center() + egui::vec2(80.0, 60.0)));
    h.run_steps(2);
    h.key_press(egui::Key::Escape);
    h.run_steps(4);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.preview_view.mark.is_none() && s.preview_compare.view.mark.is_none(), "no mark in either");
    let img = h.render().expect("the window drawn by wgpu");
    let (a2, b2) = (ink(&img, shown.center()), ink(&img, beside.center()));
    assert!(a2 == 0 && b2 == 0, "the marks are gone: {a2} {b2}");
}

/// Go to a coordinate: degrees reach a map in degrees as they are and a projected one through GDAL; the view
/// centres there, a mark stays until escape, a place off the map is gone to all the same; the box opens with ctrl+G,
/// takes the text, and enter goes.
#[test]
fn go_to_a_coordinate_with_a_mark() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    // a map in degrees: cell (40.5, 30.5) is at 12.60 - 0.0025 * 30.5 north, 122.30 + 0.0025 * 40.5 east
    let mut h = viewed(&in_degrees("nav_goto.tif"), "nav_goto");
    h.run_steps(2);
    let zoom = map(&mut h).zoom();
    assert_eq!(map(&mut h).go_to("12.52375, 122.40125", true), Ok(true));
    h.run_steps(2);
    let (z, c) = map(&mut h).camera();
    assert!(near(c, egui::vec2(40.5, 30.5), 1e-3) && z == zoom, "the view at {c:?}, zoom {z}");
    let mark = map(&mut h).mark.clone().expect("a mark");
    assert!(mark.inside && mark.text.contains("12.52375") && mark.text.contains("122.40125"), "{mark:?}");
    shot(&mut h, "14r-go-to");
    // (hemispheres and DMS name the same place; an error is told)
    assert_eq!(map(&mut h).go_to("122.40125E 12.52375N", true), Ok(true));
    assert!(map(&mut h).go_to("12\u{b0}31'25.5\"N 122\u{b0}24'4.5\"E", true).is_ok());
    assert!(near(map(&mut h).camera().1, egui::vec2(40.5, 30.5), 0.01));
    assert!(map(&mut h).go_to("200, 12", true).unwrap_err().contains("latitude"));
    // escape over the map lets the mark go
    let rect = map(&mut h).rect().expect("drawn");
    h.event(egui::Event::PointerMoved(rect.center() + egui::vec2(60.0, 60.0)));
    h.run_steps(2);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(map(&mut h).mark.is_none(), "escape lets the mark go");
    // outside the map: gone there, said
    assert_eq!(map(&mut h).go_to("20, 130", true), Ok(false));
    assert!(!map(&mut h).mark.clone().unwrap().inside);
    // map units: the cell (10.5, 20.5) by the grid's own coordinates
    let units = format!("{} {}", 122.30 + 0.0025 * 10.5, 12.60 - 0.0025 * 20.5);
    assert_eq!(map(&mut h).go_to(&units, false), Ok(true));
    assert!(near(map(&mut h).camera().1, egui::vec2(10.5, 20.5), 1e-3));
    // the box: ctrl+G with the pointer over the map, the text typed, enter
    map(&mut h).mark = None;
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::G);
    h.run_steps(3);
    assert!(map(&mut h).goto.is_some(), "ctrl+G opens the box");
    h.event(egui::Event::Text("12.52375, 122.40125".into()));
    h.run_steps(2);
    shot(&mut h, "14s-go-to-box");
    h.key_press(egui::Key::Enter);
    h.run_steps(3);
    assert!(map(&mut h).goto.is_none(), "enter goes, the box closes");
    assert!(near(map(&mut h).camera().1, egui::vec2(40.5, 30.5), 0.01), "{:?}", map(&mut h).camera());
    assert!(map(&mut h).mark.is_some());

    // a projected map: the degrees go through GDAL (EPSG:4326 to the map's system, x first)
    let mut h = viewed(&in_utm("nav_utm_goto.tif"), "nav_utm_goto");
    h.run_steps(2);
    let g = map(&mut h).shown_grid().unwrap().clone();
    let crs = format!("EPSG:{}", g.geo.epsg().expect("an EPSG code"));
    let t = g.transform;
    let (cx, cy) = (100.5, 80.5);
    let (mut xs, mut ys) = ([t[0] + cx * t[1]], [t[3] + cy * t[5]]);
    terraflux_gdal::Transform::new(&crs, "EPSG:4326").unwrap().points(&mut xs, &mut ys);
    let (lon, lat) = (xs[0], ys[0]);
    assert!((121.0..124.0).contains(&lon) && (12.0..13.0).contains(&lat), "{lon} {lat}: Sibuyan");
    assert_eq!(map(&mut h).go_to(&format!("{lat:.7}, {lon:.7}"), true), Ok(true));
    assert!(near(map(&mut h).camera().1, egui::vec2(cx as f32, cy as f32), 0.05), "{:?}", map(&mut h).camera());
    // the same, lon lat by the ranges
    assert_eq!(map(&mut h).go_to(&format!("{lon:.7} {lat:.7}"), true), Ok(true));
    assert!(near(map(&mut h).camera().1, egui::vec2(cx as f32, cy as f32), 0.05));
    // far away: outside the map, marked all the same
    assert_eq!(map(&mut h).go_to("0, 0", true), Ok(false));
    assert!(map(&mut h).mark.as_ref().is_some_and(|m| !m.inside));
}

/// A map with no coordinate system has its bar in cells.
#[test]
fn the_scale_bar_without_a_crs_is_in_cells() {
    let mut h = viewed(&data("sibuyan_landuse.tif"), "sibuyan_landuse");
    h.run_steps(2);
    let g = map(&mut h).shown_grid().unwrap().clone();
    assert_eq!(terraflux_app::measure::Frame::of(&g), terraflux_app::measure::Frame::Plain, "the test map has no CRS");
    let bar = map(&mut h).scale_bar().cloned().expect("a scale bar");
    assert!(bar.label.ends_with(" cells") || bar.label == "1 cell", "{}", bar.label);
    assert_eq!(bar.metres, None);
    assert!((60.0..=150.0).contains(&bar.px), "{} points", bar.px);
    // go to by the map's own units on it
    assert_eq!(
        map(&mut h).go_to(
            &format!("{} {}", g.transform[0] + 10.5 * g.transform[1], g.transform[3] + 20.5 * g.transform[5]),
            false
        ),
        Ok(true)
    );
    assert!(near(map(&mut h).camera().1, egui::vec2(10.5, 20.5), 1e-3));
    assert!(map(&mut h).go_to("12.5, 122.5", true).unwrap_err().contains("no coordinate system"));
}

/// The app with a template started, its Viewer open in the dock.
fn with_viewer() -> Harness<'static, TerrafluxApp> {
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(4);
    assert!(h.state().is_open(Pane::Viewer) && h.state().detached().is_empty(), "the Viewer in the dock");
    h
}

fn in_the_dock(h: &Harness<TerrafluxApp>, pane: Pane) -> bool {
    h.state().dock.as_ref().unwrap().find_tab(&pane).is_some()
}

/// The Viewer taken out of the dock by its button into a window of its own, shown there, brought to the front when
/// asked for (no tab again), and back into the dock by the button.
#[test]
fn the_viewer_pops_out_into_a_window_and_back() {
    let mut h = with_viewer();
    let tab = h.state().dock.as_ref().unwrap().find_tab(&Pane::Viewer).unwrap();
    // the button of the Viewer's header (its tab in front)
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    h.get_by_label("Pop out").click();
    h.run_steps(4);
    assert_eq!(h.state().detached(), [Pane::Viewer]);
    assert!(!in_the_dock(&h, Pane::Viewer), "no tab of it in the dock");
    assert!(h.state().viewer_out, "it opens out from now on");
    h.run_steps(2);
    assert!(h.state().viewer_rect.is_some(), "the Viewer is drawn, in its window");
    assert!(h.query_by_label("Back into the window").is_some(), "its header's button is the way back");
    // asked for (a node double-clicked): its window to the front, not a tab in the dock
    let raised = h.state().raised.1;
    h.state_mut().session.as_mut().unwrap().ask_viewer = true;
    h.run_steps(3);
    assert!(h.state().raised.1 > raised, "its window brought to the front");
    assert_eq!(h.state().detached(), [Pane::Viewer]);
    assert!(!in_the_dock(&h, Pane::Viewer));
    // back into the dock, where it was
    h.get_by_label("Back into the window").click();
    h.run_steps(4);
    assert!(h.state().detached().is_empty() && !h.state().viewer_out);
    let back = h.state().dock.as_ref().unwrap().find_tab(&Pane::Viewer).expect("its tab is back");
    assert_eq!((back.surface, back.node), (tab.surface, tab.node), "in the leaf it was in");
}

/// The View menu's item does the same; closing the window by its own close puts the Viewer back in the dock.
#[test]
fn the_viewer_out_by_the_view_menu_and_in_by_closing_its_window() {
    let mut h = with_viewer();
    h.get_by_label("View").click();
    h.run_steps(2);
    h.get_by_label("Viewer in a window of its own").click();
    h.run_steps(4);
    assert_eq!(h.state().detached(), [Pane::Viewer]);
    assert!(!in_the_dock(&h, Pane::Viewer));
    // the window's tab, taken back by its menu (the window closed: the same way back)
    h.state_mut().attach(Pane::Viewer);
    h.run_steps(3);
    assert!(h.state().detached().is_empty() && in_the_dock(&h, Pane::Viewer));
    assert!(!h.state().viewer_out);
}

/// The Viewer left out comes out when it comes, and a layout kept says so: on the next start, out again; an old
/// layout (without that) opens it in the dock.
#[test]
fn the_viewer_out_is_kept_between_sessions() {
    let file = tmp("viewer-out-layout.json");
    let _ = std::fs::remove_file(&file);
    let mut a = new_app(None);
    a.restore_layout(file.clone());
    let mut h = harness(a);
    h.run_steps(2);
    h.state_mut().set_viewer_out(true);
    h.run_steps(2);
    // (the layout is kept once a second at most)
    std::thread::sleep(Duration::from_millis(1100));
    h.run_steps(2);
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains(r#""viewer_out":true"#), "{text}");
    drop(h);
    // the next start: nothing yet, then the data comes and the Viewer with it, out
    let mut b = new_app(None);
    b.restore_layout(file.clone());
    assert!(b.viewer_out);
    let mut h = harness(b);
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(4);
    assert_eq!(h.state().detached(), [Pane::Viewer], "the Viewer comes in its window");
    assert!(!in_the_dock(&h, Pane::Viewer));
    drop(h);
    // a layout of the version before: the Viewer in the dock
    let dock = serde_json::to_string(&terraflux_app::app::default_layout())
        .unwrap()
        .replace(r#""x":null"#, r#""x":0.0"#)
        .replace(r#""y":null"#, r#""y":0.0"#);
    std::fs::write(&file, format!(r#"{{"version":11,"main":{dock},"windows":[]}}"#)).unwrap();
    let mut c = new_app(None);
    c.restore_layout(file.clone());
    assert!(!c.viewer_out);
    let mut h = harness(c);
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    h.run_steps(4);
    assert!(h.state().detached().is_empty() && in_the_dock(&h, Pane::Viewer), "an old layout: the Viewer in the dock");
    let _ = std::fs::remove_file(&file);
}

/// Files let go over the Viewer in its window of its own: layers of it, as over the dock's.
#[test]
fn files_dropped_on_the_viewer_in_its_window() {
    let mut h = with_viewer();
    h.state_mut().set_viewer_out(true);
    drawn(&mut h);
    let rect = h.state().viewer_rect.expect("the Viewer is drawn");
    let layers = h.state().session.as_ref().unwrap().preview_layers.len();
    let nodes = h.state().session.as_ref().unwrap().source.nodes.len();
    drop_files(&mut h, rect.center(), &[data("pie_restrictions.tif")]);
    drawn(&mut h);
    let s = h.state().session.as_ref().unwrap();
    // (a layer of the Viewer only comes of a drop on it. The test's window shows the other window embedded, so the
    // main one reads the same files too: a node more than in two real windows)
    assert!(s.source.nodes.len() > nodes, "a node that reads it");
    assert!(s.preview_layers.len() > layers, "and a layer of the Viewer");
}

/// A setting changed in several nodes of one operator at once: the pane of the nodes selected shows what they have in
/// common ("several values" where they differ), a change goes into all of them in one step of undo, and one the
/// checker refuses on any of them goes into none. Nodes of several operators are listed.
#[test]
fn a_setting_is_changed_in_the_nodes_selected_at_once() {
    use terraflux_graph::{Model, Node};
    let path = tmp("ui-set-many.toml");
    let mut m = Model::new("set many");
    let file = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
    for (k, (id, nodata)) in [("a", Some(0)), ("b", Some(255)), ("c", None)].into_iter().enumerate() {
        let mut n = Node::new(id, "raster.read").param("path", file.clone());
        if let Some(v) = nodata {
            n = n.param("nodata", v);
        }
        n.pos = Some([0.0, 160.0 * k as f32]);
        m.nodes.push(n);
    }
    let mut rc = Node::new("rc", "raster.reclassify").input("raster", "a.raster");
    rc.pos = Some([420.0, 0.0]);
    m.nodes.push(rc);
    m.save(&path).unwrap();
    let mut h = harness(new_app(Some(path)));
    loaded(&mut h);
    h.state_mut().focus(Pane::Model);
    h.run_steps(3);
    let choose = |h: &mut Harness<TerrafluxApp>, ids: &[&str]| {
        h.state_mut().session.as_mut().unwrap().editor.chosen = ids.iter().map(|s| s.to_string()).collect();
        h.run_steps(3);
    };
    let nodata = |h: &Harness<TerrafluxApp>| -> Vec<Option<serde_json::Value>> {
        let s = h.state().session.as_ref().unwrap();
        ["a", "b", "c"].iter().map(|id| s.source.node(id).unwrap().params.get("nodata").cloned()).collect()
    };
    choose(&mut h, &["a", "b", "c"]);
    assert!(h.query_by_label("3 raster.read selected").is_some());
    // (the nodata differs: said so, the values listed; the path is the same: its value shown)
    assert!(h.query_by_label_contains("several values: 0, 255, default").is_some());
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.fields.get("many.nodata").map(String::as_str), Some(""));
    assert_eq!(s.fields.get("many.path"), Some(&file));
    assert_eq!(h.get_all_by_label_contains("several values").count(), 1, "only the setting that differs");
    // a value for all of them: one step of undo
    {
        let r = terraflux_ops::Registry::standard();
        let s = h.state_mut().session.as_mut().unwrap();
        let ids: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        terraflux_app::editor::set_param_many(&mut s.source, &r, &[], &ids, "nodata", "-9999").unwrap();
        s.edited();
    }
    h.run_steps(3);
    assert_eq!(nodata(&h), vec![Some(serde_json::json!(-9999)); 3]);
    assert!(h.query_by_label_contains("several values: ").is_none());
    assert_eq!(h.state().session.as_ref().unwrap().fields.get("many.nodata"), Some(&"-9999".to_string()));
    terraflux_app::builder::undo_redo(h.state_mut().session.as_mut().unwrap(), false);
    h.run_steps(3);
    assert_eq!(nodata(&h), vec![Some(serde_json::json!(0)), Some(serde_json::json!(255)), None]);
    // a value the checker refuses on any of them: on none
    {
        let r = terraflux_ops::Registry::standard();
        let s = h.state_mut().session.as_mut().unwrap();
        let ids: Vec<String> = ["a", "b", "c"].iter().map(|s| s.to_string()).collect();
        let e = terraflux_app::editor::set_param_many(&mut s.source, &r, &[], &ids, "nodata", "$nothing");
        assert!(e.is_err(), "{e:?}");
    }
    assert_eq!(nodata(&h), vec![Some(serde_json::json!(0)), Some(serde_json::json!(255)), None]);
    // nodes of two operators: listed, a click on one selecting it alone
    choose(&mut h, &["a", "b", "rc"]);
    assert!(h.query_by_label("2 raster.read, 1 raster.reclassify").is_some());
    h.get_by_label("rc").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.selected.as_deref(), Some("rc"));
    assert_eq!(s.editor.chosen.iter().map(String::as_str).collect::<Vec<_>>(), ["rc"]);
}

/// A run to its end (the journal is written as it ends).
fn run_all(h: &mut Harness<TerrafluxApp>, to: Option<&str>) {
    let s = h.state_mut().session.as_mut().unwrap();
    match to {
        Some(n) => s.start_to(Some(n.to_string()), Box::new(|| {})),
        None => s.start(Box::new(|| {})),
    }
    until(h, "the run", |a| !a.session.as_ref().unwrap().running());
    h.run_steps(2);
}

/// Each run asked for is kept in a journal beside the model, and the Runs pane lists them, the newest first, with
/// what changed since the run before; the application's own runs are not kept.
#[test]
fn the_runs_are_kept_and_listed() {
    let dir = tmp("runs-journal");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    let file = w.save(&dir.join("ui-runs.toml")).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    loaded(&mut h);
    let journal = terraflux_exec::journal::Journal::of(&file).unwrap();
    {
        // a parameter to change between the runs, and two steps a run
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.parameters.push(terraflux_graph::Parameter { key: "patches.isometry".into(), ..Default::default() });
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(2))].into()).unwrap();
        s.edited();
        s.save().unwrap();
    }
    h.state_mut().focus(Pane::Runs);
    h.run_steps(3);
    assert!(h.query_by_label_contains("No run yet").is_some());
    run_all(&mut h, None);
    let before = h.state().session.as_ref().unwrap().source.setting("patches.isometry").cloned();
    let now = serde_json::json!(before.as_ref().and_then(|v| v.as_f64()).unwrap_or(1.0) + 0.5);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("patches.isometry".to_string(), now.clone())].into()).unwrap();
        s.edited();
        s.save().unwrap();
    }
    run_all(&mut h, None);
    run_all(&mut h, Some("patches"));
    // the application's own run to show a node: not kept
    assert!(h.state_mut().session.as_mut().unwrap().run_to_show("patches", Box::new(|| {})));
    until(&mut h, "the quiet run", |a| !a.session.as_ref().unwrap().running());
    h.run_steps(3);
    let all = journal.entries();
    assert_eq!(all.len(), 3, "{all:?}");
    assert_eq!(all[2].kind, terraflux_exec::journal::Kind::To("patches".into()));
    assert!(all.iter().all(|e| e.status == terraflux_exec::journal::Status::Done && e.from == "app"));
    assert_eq!(all[0].values["patches.isometry"], before.unwrap_or(serde_json::Value::Null));
    assert_eq!(all[1].values["patches.isometry"], now);
    assert_ne!(all[0].model, all[1].model);
    assert_eq!(all[1].model, all[2].model);
    // a whole run of the model as saved wrote its outputs and its manifest
    assert!(!all[0].outputs.is_empty() && all[0].manifest.is_some());
    assert!(all[1].outputs.iter().all(|w| w.unchanged() && w.sha256.is_some()));
    // (the second run wrote over the files of the first)
    assert!(all[0].outputs.iter().any(|w| !w.unchanged()));
    assert!(all[2].outputs.is_empty());
    assert_eq!(journal.model(&all[0].model).unwrap().name, h.state().session.as_ref().unwrap().source.name);
    // the pane: three rows, the last run first, the change told, the model's change marked
    h.run_steps(3);
    shot(&mut h, "runs-1");
    assert_eq!(h.state().runs.entries.len(), 3);
    let top = h.get_by_label("Run to patches").rect().min.y;
    // (the second run, the one that changed the model, is under the third)
    assert_eq!(h.get_all_by_label("model changed").count(), 1);
    assert!(h.get_by_label("model changed").rect().min.y > top, "the newest run first");
    assert!(h.query_by_label_contains("patches.isometry").is_some());
    // selecting: a click, then ctrl+click
    let ids: Vec<u64> = all.iter().map(|e| e.id).collect();
    h.state_mut().runs.select(ids[0], false);
    h.state_mut().runs.select(ids[2], true);
    assert_eq!(h.state().runs.selected, vec![ids[0], ids[2]]);
    // the outputs the second run wrote over are told in the first
    assert!(h.state().runs.over.contains_key(&ids[0]));
    assert!(!h.state().runs.over.contains_key(&ids[1]));
    let _ = std::fs::remove_dir_all(&dir);
}

/// A check run, from the Run button's menu: the model on the whole map at a quarter of the resolution; its results
/// are marked in the window, its files go to `<model> runs/check` and the real outputs are left alone, the journal
/// has it as a check run, and the next normal run clears the mark and takes the cache of the normal runs back.
#[test]
fn a_check_run_gives_the_model_at_a_coarser_resolution_apart_from_the_real_outputs() {
    use terraflux_exec::journal::{Journal, Kind};
    let dir = tmp("check-run");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    let file = w.save(&dir.join("ui-check.toml")).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(2))].into()).unwrap();
        s.edited();
        s.save().unwrap();
    }
    // a normal run first: the real outputs, and its results in the cache
    run_all(&mut h, None);
    let journal = Journal::of(&file).unwrap();
    let real = journal.entries().pop().unwrap();
    assert_eq!(real.kind, Kind::Run);
    assert!(!real.outputs.is_empty());
    let stamps = |e: &terraflux_exec::journal::Entry| -> Vec<(String, std::time::SystemTime)> {
        e.outputs.iter().map(|w| (w.path.clone(), std::fs::metadata(&w.path).unwrap().modified().unwrap())).collect()
    };
    let before = stamps(&real);
    assert!(h.state().session.as_ref().unwrap().check.is_none());
    h.run_steps(3);
    assert!(h.query_by_label_contains("check run").is_none(), "no mark after a normal run");
    // the menu next to Run
    h.get_by_label("Check run").click();
    h.run_steps(3);
    assert!(h.query_by_label("Check run at 1/4 resolution").is_some());
    shot(&mut h, "check-menu");
    h.get_by_label("Check run at 1/4 resolution").click();
    h.run_steps(2);
    until(&mut h, "the check run", |a| !a.session.as_ref().unwrap().running());
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    let check = s.check.clone().expect("the results are marked");
    assert_eq!((check.coarsen, check.window.is_none()), (4, true));
    assert!(s.error.is_none(), "{:?}", s.error);
    // the files of the check run are in their own folder, a quarter of the cells a side
    let entries = journal.entries();
    assert_eq!(entries.len(), 2, "{entries:?}");
    let Kind::Check(k) = &entries[1].kind else { panic!("{:?}", entries[1].kind) };
    assert_eq!(k.coarsen, 4);
    assert_eq!(entries[1].status, terraflux_exec::journal::Status::Done);
    assert!(!entries[1].outputs.is_empty() && entries[1].manifest.is_some());
    let folder = terraflux_exec::journal::check_dir(&std::path::absolute(&file).unwrap());
    assert!(entries[1].outputs.iter().all(|w| std::path::Path::new(&w.path).starts_with(&folder)), "{entries:?}");
    let (a, b) = (&real.outputs[0], entries[1].outputs.iter().find(|w| w.name == real.outputs[0].name).unwrap());
    let (big, small) =
        (terraflux_raster::Raster::read(&a.path).unwrap().grid, terraflux_raster::Raster::read(&b.path).unwrap().grid);
    assert_eq!((small.width, small.height), (big.width.div_ceil(4), big.height.div_ceil(4)));
    assert_eq!(small.transform[1], big.transform[1] * 4.0);
    // the real outputs untouched
    assert_eq!(stamps(&real), before, "the real outputs are as they were");
    // the mark, in the window and in the Viewer, over the map the run gave
    assert!(h.query_by_label_contains("check run, 1/4 resolution").is_some());
    h.state_mut().session.as_mut().unwrap().preview = Some(("simulation".into(), "landscape".into()));
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    shot(&mut h, "check-marked");
    assert!(h.query_all_by_label_contains("check run, 1/4 resolution").count() >= 2, "the Viewer and the status bar");
    assert_eq!(Journal::of(&file).unwrap().entries().len(), 2);
    // the list of what the runs wrote tells the check run's files and does not take them for the newest run's
    let items = terraflux_app::clean_view::items(&entries);
    assert!(items.iter().any(|i| i.check && !i.latest) && items.iter().any(|i| i.latest && !i.check));
    // the next normal run clears the mark and has the normal cache back (the nodes it holds are not run again)
    run_all(&mut h, None);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.check.is_none());
    h.run_steps(3);
    assert!(h.query_by_label_contains("check run").is_none(), "the mark is gone");
    let all = journal.entries();
    assert_eq!(all.len(), 3);
    assert_eq!(all[2].kind, Kind::Run);
    // on the view: the part of the map the Viewer shows (zoomed in), of the real resolution
    h.state_mut().session.as_mut().unwrap().preview = Some(("simulation".into(), "landscape".into()));
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(4);
    let (cols, rows) = (big.width as f32, big.height as f32);
    h.state_mut().session.as_mut().unwrap().preview_view.zoom_to([cols * 0.25, rows * 0.25, cols * 0.75, rows * 0.75]);
    h.run_steps(4);
    assert!(h.state().session.as_ref().unwrap().view_window().is_some(), "the Viewer shows a map");
    h.get_by_label("Check run").click();
    h.run_steps(3);
    h.get_by_label("Check run on the view").click();
    h.run_steps(2);
    until(&mut h, "the check run on the view", |a| !a.session.as_ref().unwrap().running());
    h.run_steps(3);
    let all = journal.entries();
    assert_eq!(all.len(), 4);
    let Kind::Check(k) = &all[3].kind else { panic!("{:?}", all[3].kind) };
    assert!(k.window.is_some() && k.coarsen == 1, "{k:?}");
    let part = terraflux_raster::Raster::read(&all[3].outputs[0].path).unwrap().grid;
    assert!(part.width < big.width && part.width * 3 > big.width, "{} of {}", part.width, big.width);
    assert!(part.height < big.height, "{} of {}", part.height, big.height);
    assert!(h.query_all_by_label_contains("check run, on the view").count() >= 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The Runs pane's actions: a scenario applied in the Parameters pane is recorded with its name; a run selected
/// shows its outputs in the Viewer and restores its model (one step of undo); two runs selected are compared (their
/// values, their models through the Review pane, their maps in the Viewer's comparison).
#[test]
fn the_runs_are_shown_restored_and_compared() {
    use terraflux_exec::journal::{Entry, Journal, Kind, Written};
    let dir = tmp("runs-actions");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    let file = w.save(&dir.join("ui-runs-actions.toml")).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    loaded(&mut h);
    let journal = Journal::of(&file).unwrap();
    let isometry = |h: &Harness<TerrafluxApp>| {
        h.state().session.as_ref().unwrap().source.setting("patches.isometry").cloned().unwrap()
    };
    let set = |h: &mut Harness<TerrafluxApp>, v: f64| {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("patches.isometry".to_string(), serde_json::json!(v))].into()).unwrap();
        s.edited();
        s.save().unwrap();
    };
    {
        // a parameter to change between the runs, a scenario to apply, and two steps a run
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.parameters.push(terraflux_graph::Parameter { key: "patches.isometry".into(), ..Default::default() });
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(2))].into()).unwrap();
        s.source.scenarios.push(terraflux_graph::Scenario {
            name: "wet".into(),
            note: None,
            values: [("patches.isometry".to_string(), serde_json::json!(2.5))].into(),
        });
        s.edited();
        s.save().unwrap();
    }
    set(&mut h, 1.5);
    run_all(&mut h, None);
    // the scenario applied from the Parameters pane: the run that follows is recorded with its name
    h.state_mut().focus(Pane::Parameters);
    h.run_steps(3);
    h.get_by_label("Apply").click();
    until(&mut h, "the run of the scenario", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && journal.entries().len() == 2
    });
    h.run_steps(2);
    assert_eq!(isometry(&h), serde_json::json!(2.5));
    // (a value it set changed: the scenario no more)
    set(&mut h, 3.5);
    run_all(&mut h, None);
    let all = journal.entries();
    assert_eq!(all.len(), 3, "{all:?}");
    assert_eq!(all.iter().map(|e| e.scenario.as_deref()).collect::<Vec<_>>(), [None, Some("wet"), None]);
    assert_eq!(all[1].values["patches.isometry"], serde_json::json!(2.5));
    let ids: Vec<u64> = all.iter().map(|e| e.id).collect();
    h.state_mut().focus(Pane::Runs);
    h.run_steps(3);
    h.state_mut().runs.select(ids[2], false);
    h.run_steps(3);
    // the detail of the run: its values, outputs and what can be done with them
    assert!(h.query_all_by_label_contains("Run of ").next().is_some());
    assert!(h.query_by_label("Show its outputs").is_some() && h.query_by_label("Restore this model").is_some());
    assert!(h.query_all_by_label_contains("patches.isometry").next().is_some());
    shot(&mut h, "runs-detail");
    // the model of the first run restored: one step of undo, named
    let steps = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().history.steps();
    let done = steps(&h).0.len();
    h.state_mut().runs.select(ids[0], false);
    h.run_steps(3);
    h.get_by_label("Restore this model").click();
    h.run_steps(3);
    assert_eq!(isometry(&h), serde_json::json!(1.5));
    let (names, at) = steps(&h);
    assert_eq!((names.len(), at), (done + 1, done + 1), "{names:?}");
    assert!(names[done].starts_with("Restore the model of the run of "), "{names:?}");
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(3);
    assert_eq!(isometry(&h), serde_json::json!(3.5));
    // (the model open is the one of the last run: nothing to restore from it)
    h.state_mut().runs.select(ids[2], false);
    h.run_steps(3);
    assert!(h.query_all_by_label_contains("already the one of this run").next().is_some());
    // the outputs of the last run, still as written, are layers of the Viewer
    let (nodes, layers) = {
        let s = h.state().session.as_ref().unwrap();
        (s.source.nodes.len(), s.preview_layers.len())
    };
    h.get_by_label("Show its outputs").click();
    h.run_steps(2);
    until(&mut h, "the outputs shown", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.show_queue.is_empty()
    });
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.source.nodes.len() > nodes, "a node reads each output");
        assert!(s.preview_layers.len() > layers, "and keeps it as a layer");
        assert!(s.notice.as_ref().is_some_and(|(t, _)| t.contains("opened in the Viewer")), "{:?}", s.notice);
    }
    // the first run's outputs were written over by the next: the copies kept with the run are what it opens
    h.state_mut().focus(Pane::Runs);
    h.state_mut().runs.select(ids[0], false);
    h.run_steps(3);
    assert!(h.query_all_by_label_contains("overwritten since, a copy kept").next().is_some());
    assert!(h.query_all_by_label_contains("left out: ").next().is_none());
    // two selected: the values side by side, then the models' differences in the Review pane
    h.state_mut().runs.select(ids[2], true);
    h.run_steps(3);
    assert!(h.query_all_by_label_contains("Compare the runs of").next().is_some());
    assert!(h.query_by_label("1.5").is_some() && h.query_by_label("3.5").is_some());
    assert_eq!(h.state().runs.selected, vec![ids[0], ids[2]]);
    shot(&mut h, "runs-compare");
    h.get_by_label("Review the model's differences").click();
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        let r = s.review.as_ref().expect("a review of the two models");
        assert!(r.after.is_some());
        assert!(!terraflux_app::review::changes(s).is_empty());
    }
    shot(&mut h, "runs-review");
    // the newer model against the open one is a review of the open model (it has the nodes the outputs added)
    h.state_mut().focus(Pane::Runs);
    h.run_steps(3);
    h.get_by_label("Newer against the model open").click();
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        let r = s.review.as_ref().unwrap();
        assert!(r.after.is_none() && !terraflux_app::review::changes(s).is_empty());
    }
    // two runs that wrote a map each, in files of their own: the Viewer compares them
    let maps = ["map-a.tif", "map-b.tif"].map(|n| {
        let p = dir.join(n);
        std::fs::copy(data("sibuyan_landuse.tif"), &p).unwrap();
        p
    });
    {
        let source = h.state().session.as_ref().unwrap().source.clone();
        for (k, p) in maps.iter().enumerate() {
            let mut e = Entry::new(Kind::Run, "app", 1.0);
            e.id = ids[2] + 1000 * (k as u64 + 1);
            e.when = e.id / 1000;
            e.outputs = vec![Written::of("land", p, None, None).unwrap()];
            journal.record(e, &source).unwrap();
        }
    }
    h.state_mut().focus(Pane::Runs);
    h.run_steps(2);
    std::thread::sleep(Duration::from_millis(2100));
    h.run_steps(3);
    let all = journal.entries();
    h.state_mut().runs.select(all[3].id, false);
    h.state_mut().runs.select(all[4].id, true);
    h.run_steps(3);
    h.get_by_label("Compare the maps (land)").click();
    h.run_steps(2);
    until(&mut h, "the maps compared", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.compare_asked.is_none() && s.show_queue.is_empty()
    });
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(matches!(s.preview_compare.with, Some(terraflux_app::builder::CompareWith::Output(..))));
        assert_eq!(s.preview_compare.how, terraflux_app::builder::How::SideBySide);
        assert!(s.preview.is_some());
    }
    shot(&mut h, "runs-maps");
    // two runs that wrote the same file: the second wrote over the first's, which is compared from its copy
    let same = dir.join("same.tif");
    {
        let source = h.state().session.as_ref().unwrap().source.clone();
        for (k, from) in ["sibuyan_landuse.tif", "sibuyan_drivers.tif"].into_iter().enumerate() {
            std::fs::copy(data(from), &same).unwrap();
            let mut e = Entry::new(Kind::Run, "app", 1.0);
            e.id = ids[2] + 10_000 * (k as u64 + 1);
            e.when = e.id / 1000;
            e.outputs = vec![Written::of("land", &same, None, None).unwrap()];
            journal.record(e, &source).unwrap();
        }
    }
    h.state_mut().focus(Pane::Runs);
    h.run_steps(2);
    std::thread::sleep(Duration::from_millis(2100));
    h.run_steps(3);
    let all = journal.entries();
    assert_eq!(all.len(), 7);
    let copy = journal.kept(&all[5].outputs[0]).expect("the first run's copy");
    assert!(copy.is_file() && journal.kept(&all[6].outputs[0]).is_some());
    h.state_mut().runs.select(all[5].id, false);
    h.state_mut().runs.select(all[6].id, true);
    h.run_steps(3);
    assert!(h.query_all_by_label_contains("overwritten since, copies kept").next().is_some());
    h.get_by_label("Compare the maps (land)").click();
    h.run_steps(2);
    until(&mut h, "the copy and the file compared", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.compare_asked.is_none() && s.show_queue.is_empty()
    });
    h.run_steps(3);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(matches!(s.preview_compare.with, Some(terraflux_app::builder::CompareWith::Output(..))));
        assert!(s.preview.is_some());
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Named versions of a model: *Save a version...* keeps the model as it is on screen; the Versions tab lists them
/// newest first with what changed from the one before; one is reviewed against the model open (a change rejected
/// brings the version's value back), restored (one named step of undo) or deleted after asking; a run's detail
/// names the version of its model.
#[test]
fn the_versions_are_saved_listed_reviewed_restored_and_deleted() {
    use terraflux_app::review::{self, Target};
    use terraflux_app::runs_view::Tab;
    use terraflux_app::versions_view::Dialog;
    use terraflux_exec::journal::Journal;
    let dir = tmp("versions");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    let file = w.save(&dir.join("ui-versions.toml")).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    loaded(&mut h);
    let journal = Journal::of(&file).unwrap();
    let isometry = |h: &Harness<TerrafluxApp>| {
        h.state().session.as_ref().unwrap().source.setting("patches.isometry").cloned().unwrap()
    };
    let set = |h: &mut Harness<TerrafluxApp>, v: f64, save: bool| {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("patches.isometry".to_string(), serde_json::json!(v))].into()).unwrap();
        s.edited();
        if save {
            s.save().unwrap();
        }
    };
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.parameters.push(terraflux_graph::Parameter { key: "patches.isometry".into(), ..Default::default() });
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(2))].into()).unwrap();
        s.edited();
        s.save().unwrap();
    }
    set(&mut h, 1.5, true);
    // the shortcut opens the window; a label is required
    h.key_press_modifiers(egui::Modifiers::COMMAND | egui::Modifiers::ALT, egui::Key::S);
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().version_dialog.is_some(), "ctrl+alt+S opens the window");
    h.state_mut().session.as_mut().unwrap().version_dialog =
        Some(Dialog { label: "before".into(), note: "the first isometry".into() });
    h.run_steps(3);
    assert!(h.query_by_label_contains("edits not saved").is_none());
    h.get_by_label("Save version").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().version_dialog.is_none());
    assert_eq!(journal.versions().len(), 1);
    run_all(&mut h, None);
    // the second, of what is on screen with an edit not saved: the window says so
    set(&mut h, 3.5, false);
    h.state_mut().session.as_mut().unwrap().version_dialog =
        Some(Dialog { label: "after".into(), note: String::new() });
    h.run_steps(3);
    assert!(h.query_by_label_contains("edits not saved").is_some());
    h.get_by_label("Save version").click();
    h.run_steps(3);
    h.state_mut().session.as_mut().unwrap().save().unwrap();
    let versions = journal.versions();
    assert_eq!(versions.iter().map(|v| v.label.as_str()).collect::<Vec<_>>(), ["before", "after"]);
    assert_eq!(versions[0].note.as_deref(), Some("the first isometry"));
    assert_eq!(versions[1].from, "app");
    assert_eq!(journal.model(&versions[1].model).unwrap().setting("patches.isometry"), Some(&serde_json::json!(3.5)));
    // the tab: two rows, the newest first, the change told
    h.state_mut().focus(Pane::Runs);
    h.run_steps(3);
    h.get_by_label("Versions").click();
    h.run_steps(3);
    assert_eq!(h.state().runs.tab, Tab::Versions);
    assert_eq!(h.state().runs.versions.list.len(), 2);
    assert!(h.get_by_label("after").rect().min.y < h.get_by_label("before").rect().min.y, "the newest first");
    assert!(h.query_by_label("1 node changed").is_some());
    shot(&mut h, "versions");
    // the run's detail names the version of its model
    let run = journal.entries()[0].id;
    h.state_mut().runs.tab = Tab::Runs;
    h.state_mut().runs.select(run, false);
    h.run_steps(3);
    assert!(h.query_by_label("The model of version before").is_some());
    h.state_mut().runs.tab = Tab::Versions;
    h.run_steps(3);
    // review against the model open: rejecting the change brings the version's value back
    let ids: Vec<u64> = versions.iter().map(|v| v.id).collect();
    h.state_mut().runs.versions.select(ids[0], false);
    h.run_steps(3);
    h.get_by_label("Review against the model open").click();
    h.run_steps(3);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let r = s.review.as_ref().expect("a review of the version against the model open");
        assert!(r.after.is_none());
        let target = Target::Node("patches".into());
        assert!(review::changes(s).iter().any(|c| c.target == target), "{:?}", review::changes(s));
        review::reject(s, &target);
        review::follow(s);
    }
    assert_eq!(isometry(&h), serde_json::json!(1.5));
    // the version "after" restored: one named step, undone by ctrl+Z
    h.state_mut().runs.versions.select(ids[1], false);
    h.run_steps(3);
    let steps = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().history.steps();
    let done = steps(&h).0.len();
    h.get_by_label("Restore this version").click();
    h.run_steps(3);
    assert_eq!(isometry(&h), serde_json::json!(3.5));
    let (names, at) = steps(&h);
    assert_eq!((names.len(), at), (done + 1, done + 1), "{names:?}");
    assert_eq!(names[done], "Restore the version after");
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
    h.run_steps(3);
    assert_eq!(isometry(&h), serde_json::json!(1.5));
    // two selected: their differences to read
    h.state_mut().runs.versions.select(ids[0], false);
    h.state_mut().runs.versions.select(ids[1], true);
    h.run_steps(3);
    h.get_by_label("Review the differences").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().review.as_ref().is_some_and(|r| r.after.is_some()));
    // "before" deleted, after asking
    h.state_mut().runs.versions.select(ids[0], false);
    h.run_steps(3);
    h.get_by_label("Delete...").click();
    h.run_steps(3);
    assert_eq!(journal.versions().len(), 2, "not before the answer");
    h.get_by_label("Delete the version").click();
    h.run_steps(3);
    assert_eq!(journal.versions().iter().map(|v| v.id).collect::<Vec<_>>(), vec![ids[1]]);
    assert_eq!(h.state().runs.versions.list.len(), 1);
    // (a run ran the model of "before": its snapshot stays)
    assert!(journal.model(&versions[0].model).is_some());
    let _ = std::fs::remove_dir_all(&dir);
}

/// *Clean outputs...* lists what the runs wrote, from the journal: the batch folders and the outputs of the runs
/// before the newest are chosen, the newest run's are not; deleting asks first, takes the files away and leaves the
/// journal, whose runs then show them missing.
#[test]
fn the_outputs_of_the_runs_are_cleaned_after_asking() {
    use terraflux_exec::journal::{Entry, Journal, Kind, Written};
    let dir = tmp("clean-outputs");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("output")).unwrap();
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    let file = w.save(&dir.join("ui-clean.toml")).unwrap();
    let journal = Journal::of(&file).unwrap();
    // a run from long ago, whose output is still there
    let old = dir.join("output").join("old.tif");
    std::fs::write(&old, vec![0u8; 4096]).unwrap();
    let past = Entry {
        id: 1000,
        when: 1,
        kind: Kind::Run,
        outputs: vec![Written::of("old", &old, None, None).unwrap()],
        ..Default::default()
    };
    journal.record(past, &terraflux_graph::Model::load(&file).unwrap()).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(2))].into()).unwrap();
        s.edited();
        s.save().unwrap();
    }
    run_all(&mut h, None);
    // a batch of replicates, in its own folder
    let batch = dir.join("batch");
    h.state_mut().focus(Pane::Batch);
    h.state_mut().computers.pool = None;
    h.state_mut().computers.dir = Some(tmp("clean-outputs-pool"));
    {
        let b = &mut h.state_mut().batch;
        b.runs = 2;
        b.workers = 2;
        b.out = Some(batch.clone());
    }
    h.run_steps(3);
    h.get_by_label("Run 2 times").click();
    h.run_steps(1);
    assert!(h.state().batch.running(), "{:?}", h.state().batch.error);
    until(&mut h, "the replicates", |a| !a.batch.running());
    h.run_steps(2);
    assert!(batch.join("tasks.csv").exists());
    // the window, from the Runs pane's button
    h.state_mut().focus(Pane::Runs);
    h.run_steps(3);
    h.get_by_label("Clean outputs...").click();
    h.run_steps(3);
    until(&mut h, "the sizes", |a| {
        a.session.as_ref().unwrap().clean.as_ref().is_some_and(|c| c.items.iter().all(|i| i.size.is_some()))
    });
    h.run_steps(2);
    let items = h.state().session.as_ref().unwrap().clean.as_ref().unwrap().items.clone();
    let by = |name: &str| items.iter().find(|i| i.path.ends_with(name)).unwrap_or_else(|| panic!("{name} {items:?}"));
    assert!(by("batch").checked && by("batch").batch, "the batch folder is chosen");
    assert!(by("old.tif").checked && !by("old.tif").latest, "an output of a run before is chosen");
    let latest: Vec<_> = items.iter().filter(|i| i.latest).collect();
    assert!(!latest.is_empty() && latest.iter().all(|i| !i.checked), "the newest run's outputs are not: {latest:?}");
    let chosen_bytes: u64 = items.iter().filter(|i| i.checked).map(|i| i.size.unwrap()).sum();
    assert!(chosen_bytes >= 4096);
    shot(&mut h, "clean-1");
    // asking first: nothing is deleted until it is confirmed
    h.get_by_label_contains("Delete 2 items").click();
    h.run_steps(2);
    assert!(h.query_by_label_contains("for good").is_some());
    assert!(batch.exists() && old.exists());
    shot(&mut h, "clean-2-asked");
    h.get_by_label("Delete").click();
    h.run_steps(3);
    assert!(!batch.exists() && !old.exists(), "the folder and the file are gone");
    for i in latest {
        assert!(i.path.exists(), "{:?} is the newest run's: kept", i.path);
    }
    // the journal keeps its runs, which now tell the file missing
    let entries = journal.entries();
    assert_eq!(entries.len(), 3);
    assert_eq!(terraflux_app::runs_view::state_of(&entries[0].outputs[0], None), "missing");
    // (the copy kept with the run stays: the cleaning leaves it unless asked)
    let state = terraflux_app::runs_view::state_of(&entries[0].outputs[0], Some(&journal));
    assert_eq!(state, "missing, a copy kept");
    h.state_mut().session.as_mut().unwrap().clean = None;
    h.state_mut().runs.select(entries[0].id, false);
    h.run_steps(3);
    assert!(h.get_all_by_label_contains("missing").count() >= 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// A run whose outputs may not fit the disk asks first (the disk made small for the test): *Cancel* does not run it,
/// *Run anyway* does, *Clean outputs...* opens the cleaning; a batch that keeps files asks the same, and its cost
/// line tells the room.
#[test]
fn a_run_that_may_not_fit_the_disk_asks_first() {
    use terraflux_app::disk::Space;
    let dir = tmp("disk-asks");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut w = Wizard::default();
    w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
    w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
    w.increment = Some(250.0);
    let file = w.save(&dir.join("ui-disk.toml")).unwrap();
    let mut h = harness(new_app(Some(file.clone())));
    loaded(&mut h);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.source.apply(&[("simulation.times".to_string(), serde_json::json!(2))].into()).unwrap();
        s.edited();
        s.save().unwrap();
        // a disk with 1 GB free of 10: the margin leaves nothing for any run
        s.disk_space = Some(Space { free: 1 << 30, total: 10 << 30 });
        assert!(s.run_bytes() > 0, "the model's maps give an estimate");
    }
    let journal = terraflux_exec::journal::Journal::of(&file).unwrap();
    h.run_steps(3);
    h.get_by_label("Run").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.disk_ask.is_some() && !s.running(), "the run waits for the answer");
    assert!(h.query_by_label_contains("The outputs may take about").is_some());
    assert!(h.query_by_label_contains("1.0 GB free").is_some());
    shot(&mut h, "disk-1");
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.disk_ask.is_none() && !s.running() && journal.entries().is_empty(), "cancelled: nothing ran");
    // Clean outputs... opens the cleaning
    h.get_by_label("Run").click();
    h.run_steps(3);
    h.get_by_label("Clean outputs...").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.disk_ask.is_none() && s.clean.is_some() && !s.running());
    h.state_mut().session.as_mut().unwrap().clean = None;
    h.run_steps(2);
    // run anyway
    h.get_by_label("Run").click();
    h.run_steps(3);
    h.get_by_label("Run anyway").click();
    h.run_steps(1);
    until(&mut h, "the run", |a| !a.session.as_ref().unwrap().running());
    h.run_steps(3);
    assert_eq!(journal.entries().len(), 1, "run anyway ran");
    // a batch that keeps files asks too, and the estimate now comes from that run
    h.state_mut().focus(Pane::Batch);
    h.state_mut().computers.pool = None;
    h.state_mut().computers.dir = Some(tmp("disk-asks-pool"));
    {
        let b = &mut h.state_mut().batch;
        b.runs = 2;
        b.workers = 2;
        b.out = Some(dir.join("batch"));
    }
    h.run_steps(3);
    assert!(h.query_by_label_contains("on disk (1.0 GB free)").is_some(), "the cost line tells the room");
    shot(&mut h, "disk-2-batch");
    h.get_by_label("Run 2 times").click();
    h.run_steps(3);
    assert!(h.state().batch.disk_ask.is_some() && !h.state().batch.running());
    h.get_by_label("Cancel").click();
    h.run_steps(3);
    assert!(h.state().batch.disk_ask.is_none() && !h.state().batch.running());
    // with room, nothing is asked
    h.state_mut().session.as_mut().unwrap().disk_space = Some(Space { free: 500 << 30, total: 1000 << 30 });
    h.run_steps(2);
    h.get_by_label("Run 2 times").click();
    h.run_steps(2);
    assert!(h.state().batch.running() && h.state().batch.disk_ask.is_none());
    until(&mut h, "the replicates", |a| !a.batch.running());
    let _ = std::fs::remove_dir_all(&dir);
}
