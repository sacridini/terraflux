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
    until(&mut h, "the new map", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.status.contains_key("raster_reclassify")
    });
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Map(m)) = s.value_of("raster_reclassify", "map") else { panic!() };
    let ones = |m: &terraflux_ops::Map| m.data.iter().filter(|v| **v == 1.0).count();
    assert!(ones(&m) > ones(&forest), "class 2 joined the ones");
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
        s.preview = Some(("raster_read".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_read", "raster").is_some()
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
            Some(terraflux_app::builder::CompareWith::Output("raster_read".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("raster_reclassify", "map").is_some()
            && s.value_of("raster_read", "raster").is_some()
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
        let read = || terraflux_app::builder::CompareWith::Output("raster_read".into(), "raster".into());
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
    h.get_by_label("📌 Keep as a layer").click();
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        assert!(s.preview_compare.with.is_none(), "the comparison let go");
        assert_eq!(s.preview_layers.len(), 1);
        let l = &s.preview_layers[0];
        assert_eq!((l.node.as_str(), l.port.as_str()), ("raster_read", "raster"));
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
        assert_eq!(s.preview_layers[0].name(Some(&d)), "raster_reclassify.map to raster_read");
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
                    ..terraflux_graph::PlotLayer::new("raster_read.raster")
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
                with: "raster_read.raster".into(),
                how: Some("side by side".into()),
                also: vec!["raster_read.raster".into()],
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
        Some(terraflux_app::builder::CompareWith::Output("raster_read".into(), "raster".into()))
    );
    assert_eq!(s.preview_compare.how, terraflux_app::builder::How::SideBySide);
    // with its view more
    assert_eq!(s.preview_compare.more.len(), 1);
    assert_eq!(
        s.preview_compare.more[0].with,
        Some(terraflux_app::builder::CompareWith::Output("raster_read".into(), "raster".into()))
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
    assert!(h.query_by_label("LAYERS").is_some());
    shot(&mut h, "21-stacked");
    // the panel of the layers, at the map's left: hidden by its button, and shown again
    h.get_by_label("Layers").click();
    h.run_steps(20);
    assert!(h.query_by_label("LAYERS").is_none(), "the layers hidden");
    h.get_by_label("Layers").click();
    h.run_steps(20);
    assert!(h.query_by_label("LAYERS").is_some(), "the layers shown");
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
    // zoom to layer: the roads (the second row from the top), the view on their 30 km, inside the land
    {
        use egui_kittest::kittest::NodeT;
        let mut zooms: Vec<_> = h.query_all_by_label("zoom").collect();
        let y = |n: &egui_kittest::Node| n.accesskit_node().bounding_box().map_or(0.0, |r| r.y0);
        zooms.sort_by(|a, b| y(a).total_cmp(&y(b)));
        assert_eq!(zooms.len(), 3, "a zoom per layer");
        zooms[1].click();
    }
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
    // the drivers down, under the roads (the rows from the top: the drivers' first, the land's down disabled)
    h.query_all_by_label("⏷").find(|n| !n.accesskit_node().is_disabled()).unwrap().click();
    drawn(&mut h);
    assert_eq!(kept(&h)[1], pair("drivers", "raster"), "the drivers under the roads");
    // the land no longer a layer (its x the bottom one, below the node's inputs' x)
    let bottom = h.query_all_by_label("x").max_by(|a, b| {
        let y = |n: &egui_kittest::Node| n.accesskit_node().bounding_box().map_or(0.0, |r| r.y0);
        y(a).total_cmp(&y(b))
    });
    bottom.unwrap().click();
    h.run_steps(2);
    assert_eq!(kept(&h), [pair("drivers", "raster"), pair("roads", "vector")]);
    // a map not among them, kept with the pin: on top
    show(&mut h, "land", "raster");
    drawn(&mut h);
    h.get_by_label("📌").click();
    h.run_steps(2);
    assert_eq!(kept(&h).last(), Some(&pair("land", "raster")));
    // the land (on top) dragged by its name onto the drivers' row (at the bottom): it takes their place
    let name_at = |h: &Harness<TerrafluxApp>, name: &str| -> egui::Pos2 {
        let n = h
            .query_all_by_label(name)
            .find(|n| n.accesskit_node().role() == egui::accesskit::Role::Button)
            .unwrap_or_else(|| panic!("the layer {name}"));
        let b = n.accesskit_node().bounding_box().unwrap();
        egui::pos2(((b.x0 + b.x1) / 2.0) as f32, ((b.y0 + b.y1) / 2.0) as f32)
    };
    let (from, to) = (name_at(&h, "land"), name_at(&h, "drivers"));
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
    terraflux_app::settings::update(|s| {
        *s = terraflux_app::settings::Settings { theme: "Default".into(), ..Default::default() }
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
    assert!(h.state().shortcuts_open && h.query_by_label("Keyboard shortcuts").is_some());
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
    assert!(h.state().trace.trace.is_some());
    let last = terraflux_app::trace::last().expect("the last trace kept");
    assert!(last.contains("Here field.map is"), "{last}");
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
    assert!(h.query_by_label("raster_read (layer 1 of 2)").is_some(), "the map read listed");
    assert!(h.query_all_by_label("10").next().is_some() && h.query_all_by_label("1").next().is_some(), "both values");
    shot(&mut h, "14d-cell-everywhere");
    // the map read, shown by its name (the section's label, not the checkbox of the maps over it)
    // (the file has two bands: the one read, the first, said)
    h.get_by_role_and_label(egui::accesskit::Role::Button, "raster_read (layer 1 of 2)").click();
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().preview, Some(("raster_read".into(), "raster".into())));
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
        s.preview_layers = vec![terraflux_app::builder::Layer {
            style,
            ..terraflux_app::builder::Layer::new("vector_read", "vector")
        }];
        s.preview = Some(("raster_reclassify".into(), "map".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("raster_reclassify", "map").is_some()
            && s.value_of("vector_read", "vector").is_some()
    });
    h.state_mut().session.as_mut().unwrap().qgis.open = true;
    h.run_steps(3);
    // both maps of the run listed, the one shown chosen
    let names: Vec<String> =
        terraflux_app::qgis::candidates(h.state().session.as_ref().unwrap()).into_iter().map(|c| c.2).collect();
    assert_eq!(names, ["raster_read", "raster_reclassify.map", "vector_read.vector"]);
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
    let plots = terraflux_app::qgis::pick(s, "vector_read", "vector").unwrap();
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
        s.preview_layers = vec![terraflux_app::builder::Layer::new("vector_read", "vector")];
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
        !s.running() && s.value_of("vector_read", "vector").is_some()
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
        s.preview = Some(("raster_read".into(), "raster".into()));
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
        s.preview = Some(("raster_read".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_read", "raster").is_some()
    });
    drawn(&mut h);
    // (scrolled to, at the bottom of the panel beside the map)
    h.get_by_label("polygons").scroll_to_me();
    h.run_steps(3);
    h.get_by_label("polygons").click();
    until(&mut h, "drawing turned on", |a| a.session.as_ref().unwrap().preview_view.drawing.is_some());
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
    assert!(format!("{:?}", n.inputs).contains("vector_read.vector"), "the area connected: {:?}", n.inputs);
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
    assert_eq!(h.state().session.as_ref().unwrap().catalog.choices.like.as_deref(), Some("raster_read.raster"));
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
        matches!(n.inputs.get("like"), Some(terraflux_graph::Source::Ref(r)) if r == "raster_read.raster"),
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
        s.preview_layers = vec![terraflux_app::builder::Layer::new("vector_read", "vector")];
        s.preview = Some(("raster_read".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_read", "raster").is_some() && s.value_of("vector_read", "vector").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    let open = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().preview_attributes.clone();
    assert!(open(&h).is_none());
    assert!(h.query_by_label("Attributes of vector_read.vector").is_none());
    // the table button of the layer's row
    h.get_by_label("table").click();
    h.run_steps(3);
    assert!(open(&h).is_some_and(|o| o.is("vector_read", "vector") && o.pick.row.is_none()));
    assert!(h.query_by_label("Attributes of vector_read.vector").is_some());
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
    // the tab-separated copy: its columns named, a line a feature
    let v = h.state().session.as_ref().unwrap().value_of("vector_read", "vector");
    let Some(terraflux_ops::Value::Vector(v)) = v else { panic!("no features") };
    let text = terraflux_app::attribute_table::tsv(&terraflux_app::attribute_table::table_of(&v));
    assert_eq!(text.lines().next(), Some("feature\tarea\tuse"));
    assert_eq!(text.lines().nth(1), Some("1\t12.5\tforest"));
    // the button again closes the table, the outline goes
    h.get_by_label("table").click();
    h.run_steps(3);
    assert!(open(&h).is_none());
    assert!(h.query_by_label("Attributes of vector_read.vector").is_none());
    assert_eq!(h.state().session.as_ref().unwrap().preview_view.selected, None);
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
        let mut layer = terraflux_app::builder::Layer::new("vector_read", "vector");
        layer.style.labels = Some("use".into());
        s.preview_layers = vec![layer];
        s.preview = Some(("raster_read".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.value_of("raster_read", "raster").is_some() && s.value_of("vector_read", "vector").is_some()
    });
    h.run_steps(4);
    drawn(&mut h);
    let placed = |h: &Harness<TerrafluxApp>| h.state().session.as_ref().unwrap().preview_view.vectors.labels_placed;
    assert_eq!(placed(&h), 2, "the labels placed");
    shot(&mut h, "14g-labels");
    // by a number: the same two features, labelled
    h.state_mut().session.as_mut().unwrap().preview_layers[0].style.labels = Some("area".into());
    h.run_steps(3);
    assert_eq!(placed(&h), 2);
    // by nothing: none
    h.state_mut().session.as_mut().unwrap().preview_layers[0].style.labels = None;
    h.run_steps(3);
    assert_eq!(placed(&h), 0);
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
        s.preview_layers = vec![terraflux_app::builder::Layer::new("vector_read", "vector"), over];
        s.preview = Some(("raster_read".into(), "raster".into()));
    }
    h.state_mut().focus(Pane::Viewer);
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running()
            && s.value_of("vector_read", "vector").is_some()
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
    let read = || CompareWith::Output("raster_read".into(), "raster".into());
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
            && s.value_of("raster_read", "raster").is_some()
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
        assert_eq!(c.also, ["raster_read.raster"]);
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
