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

/// The application rendered off screen by wgpu, the builder's canvas pipelines installed as the application
/// does.
fn harness(app: TerrafluxApp) -> Harness<'static, TerrafluxApp> {
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
    let mut h = harness(TerrafluxApp::new(Some(model())));
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
    for (pane, name) in [(Pane::Areas, "3-charts"), (Pane::Model, "4-graph"), (Pane::Text, "5-model")] {
        h.state_mut().focus(pane);
        h.run_steps(3);
        shot(&mut h, name);
    }
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
    let mut h = harness(TerrafluxApp::new(None));
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
    let mut h = harness(TerrafluxApp::new(Some(path.clone())));
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
        assert_eq!(s.selected.as_deref(), Some("calc"));
        let r = terraflux_ops::Registry::standard();
        terraflux_app::editor::connect(&mut s.source, &r, &[], "$t0", "calc", "land").unwrap();
        terraflux_app::editor::set_param(&mut s.source, &[], "calc", "expression", "land == 1").unwrap();
        s.edited();
        assert!(s.model.node("calc").is_some());
        assert!(!s.issues.iter().any(|i| i.at.starts_with("calc")), "{:?}", s.issues);
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
    h.get_by_label("Save").click();
    h.run_steps(2);
    let saved = terraflux_graph::Model::load(&path).unwrap();
    assert_eq!(saved.node("calc").unwrap().params["expression"], "land == 1");
    assert!(saved.node("calc").unwrap().pos.is_some());
    assert!(!h.state().session.as_ref().unwrap().dirty);
}

/// The builder from nothing: a raster opened, MSPA on it shown at once, a setting changed (only MSPA runs
/// again), the result saved as a raster and the model as a file.
#[test]
fn the_builder_reads_analyses_and_writes() {
    let mut h = harness(TerrafluxApp::new(None));
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
        assert_eq!(read, "read");
        // forest (class 1) as the foreground, connected to the map read
        terraflux_app::builder::add(s, &r, "landscape.mspa", &[("foreground", 1.into())]).unwrap();
        let mspa = s.source.nodes.iter().find(|n| n.id == "mspa").unwrap();
        assert_eq!(mspa.inputs["map"], terraflux_graph::Source::Ref("read.raster".into()), "fed by the node selected");
        assert_eq!(s.preview, Some(("mspa".into(), "map".into())));
    }
    // by itself after the edit: the read and MSPA run, the result shown
    until(&mut h, "MSPA", |a| {
        a.session.as_ref().unwrap().status.get("mspa") == Some(&NodeStatus::Done { cached: false })
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
        terraflux_app::editor::set_param(&mut s.source, &[], "mspa", "edge_width", "3").unwrap();
        s.edited();
    }
    until(&mut h, "MSPA again", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.status.get("mspa") == Some(&NodeStatus::Done { cached: false })
    });
    {
        let s = h.state().session.as_ref().unwrap();
        assert_eq!(s.status.get("read"), Some(&NodeStatus::Done { cached: true }));
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
        a.session.as_ref().unwrap().status.get("write") == Some(&NodeStatus::Done { cached: false })
    });
    let written = terraflux_raster::Raster::read(&out).unwrap();
    assert_eq!(written.dtype, terraflux_raster::DataType::U8);
    shot(&mut h, "12-builder-saved");
    // the model saved: its files relative to its folder
    let model = tmp("ui-builder.toml");
    h.state_mut().session.as_mut().unwrap().save_as(&model).unwrap();
    let m = terraflux_graph::Model::load(&model).unwrap();
    let p = m.node("read").unwrap().params["path"].as_str().unwrap().to_string();
    assert!(!std::path::Path::new(&p).is_absolute() && p.ends_with("sibuyan_landuse.tif"), "{p}");
    let again = terraflux_graph::Model::load_expanded(&model).unwrap();
    let r = terraflux_exec::Engine::default().run(&again, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
    assert!(r.get("mspa.map").is_some(), "the saved model runs from the command line's engine too");
}

/// Read, reclassify, plot: the plot's map tab opens by itself, nothing written; saved from the tab (a file, then
/// a write node in the model too); a rule changed, the tab shows the new map.
#[test]
fn a_plot_shows_without_writing_and_saves_from_its_tab() {
    let mut h = harness(TerrafluxApp::new(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        terraflux_app::builder::add(s, &r, "raster.reclassify", &[("rules", "1 -> 1\n* -> 0".into())]).unwrap();
        terraflux_app::builder::add(s, &r, "view.plot", &[("title", "forest".into())]).unwrap();
        let plot = s.source.nodes.iter().find(|n| n.id == "plot").unwrap();
        assert_eq!(plot.inputs["value"], terraflux_graph::Source::Ref("reclassify.map".into()));
    }
    until(&mut h, "the plot's tab", |a| a.session.as_ref().unwrap().map_tabs.values().any(|t| t.node == "plot"));
    until(&mut h, "the plot", |a| !a.session.as_ref().unwrap().running());
    h.run_steps(3);
    let (id, forest) = {
        let s = h.state().session.as_ref().unwrap();
        let (id, _) = s.map_tabs.iter().find(|(_, t)| t.node == "plot").unwrap();
        let Some(terraflux_ops::Value::Map(m)) = s.value_of("plot", "shown") else { panic!("{:?}", s.error) };
        assert!(m.data.iter().all(|v| v.is_nan() || *v == 0.0 || *v == 1.0));
        (*id, m)
    };
    assert!(h.state().is_open(Pane::Map(id)));
    shot(&mut h, "14-plot");
    // saved from the tab: the same cells in the file
    let out = tmp("ui-forest.tif");
    let _ = std::fs::remove_file(&out);
    h.state_mut().session.as_mut().unwrap().save_output("plot", "shown", &out);
    until(&mut h, "the file", |a| !a.session.as_ref().unwrap().saving());
    let saved = terraflux_raster::Raster::read(&out).unwrap();
    let back = saved.bands[0].masked(saved.nodata);
    assert!(back.iter().zip(forest.data.iter()).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
    // and a write node in the model, that writes it at every run
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = terraflux_app::builder::add_write(s, &r, "plot", "shown", &out).unwrap();
        let w = s.source.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(w.inputs["value"], terraflux_graph::Source::Ref("plot.shown".into()));
    }
    // another rule: the tab shows the new map
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.selected = Some("plot".into());
        s.preview = Some(("plot".into(), "shown".into()));
        terraflux_app::editor::set_param(&mut s.source, &[], "reclassify", "rules", "1 2 -> 1\n* -> 0").unwrap();
        s.edited();
    }
    h.run_steps(1);
    until(&mut h, "the new plot", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.status.contains_key("plot")
    });
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Map(m)) = s.value_of("plot", "shown") else { panic!() };
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
    let mut h = harness(TerrafluxApp::new(None));
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
    let mut h = harness(TerrafluxApp::new(None));
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
    let mut h = harness(TerrafluxApp::new(None));
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
    let mut h = harness(TerrafluxApp::new(None));
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
    use terraflux_app::table_view::{TableState, rows_shown};
    let mut h = harness(TerrafluxApp::new(None));
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
    let st = TableState { sort: Some((2, false)), filter: String::new(), chart: Some(2) };
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
    let mut h = harness(TerrafluxApp::new(None));
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
    let mut h = harness(TerrafluxApp::new(None));
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
