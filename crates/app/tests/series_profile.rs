//! The profile of an object in the inspector, when an `objects.series` made the objects (the template "Crops by their
//! seasons"): the dates of the object, the band of the samples of its class, and a second object to compare (ctrl+click).

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use terraflux_app::TerrafluxApp;
use terraflux_app::series_view;
use terraflux_ops::objects::Objects;
use terraflux_ops::{Registry, Value};

fn harness(name: &str) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("model.toml");
    terraflux_app::templates::TEMPLATES
        .iter()
        .find(|t| t.name == "Crops by their seasons")
        .expect("the template")
        .model()
        .save(&p)
        .unwrap();
    let mut app = TerrafluxApp::new(Some(p));
    app.session.as_mut().unwrap().wait();
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    terraflux_app::canvas_gpu::install(&rs);
    Harness::builder()
        .with_size((1400.0, 1000.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app)
}

/// Step until `done`, a minute at most.
fn until(h: &mut Harness<TerrafluxApp>, what: &str, done: impl Fn(&Harness<TerrafluxApp>) -> bool) {
    let t = Instant::now();
    while !done(h) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "{what}: too long");
    }
}

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

fn objects_of(h: &Harness<TerrafluxApp>, node: &str) -> std::sync::Arc<Objects> {
    match h.state().session.as_ref().unwrap().value_of(node, "objects") {
        Some(Value::Objects(o)) => o,
        other => panic!("{node}: {other:?}"),
    }
}

/// A click at the centre of a cell of the map shown, with the modifiers.
fn click_cell(h: &mut Harness<TerrafluxApp>, row: usize, col: usize, modifiers: egui::Modifiers) {
    let at = h.state().session.as_ref().unwrap().preview_view.cell_screen(col, row).expect("the map drawn");
    h.event(egui::Event::PointerMoved(at));
    h.run_steps(2);
    for pressed in [true, false] {
        h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers });
        h.run_steps(1);
    }
    h.run_steps(3);
}

fn selected(h: &Harness<TerrafluxApp>) -> Vec<u32> {
    h.state().session.as_ref().unwrap().preview_objects.selected.iter().copied().collect()
}

#[test]
fn the_inspector_draws_the_profile_of_the_object_its_samples_band_and_a_second_object() {
    let mut h = harness("series-profile");
    run_and_show(&mut h, "classify");
    h.state_mut().session.as_mut().unwrap().preview = Some(("classify".into(), "objects".into()));
    h.run_steps(2);
    until(&mut h, "the objects drawn", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() == 1);
    let o = objects_of(&h, "classify");
    assert!(series_view::drawn(&h.ctx).is_none(), "nothing selected, no profile");
    // two parcels of the same crop of a good size (the first of them is classed)
    let big: Vec<u32> =
        (1..=o.count as u32).filter(|&i| o.cells[i as usize - 1] >= 60 && o.class_of[i as usize - 1] > 0).collect();
    let a = big[0];
    let class = o.class_of[a as usize - 1];
    // (the largest, whose cell is well inside the map: one at its edge is under the map's own controls)
    let b = *big
        .iter()
        .filter(|&&i| i != a && o.class_of[i as usize - 1] == class)
        .max_by_key(|&&i| o.cells[i as usize - 1])
        .expect("a second parcel of the crop");
    let cell = |id| terraflux_app::objects_inspect::sample_cell(&o, id).expect("an object");
    let (row, col) = cell(a);
    click_cell(&mut h, row, col, egui::Modifiers::NONE);
    assert_eq!(selected(&h), vec![a]);
    until(&mut h, "the profile", |h| series_view::drawn(&h.ctx).is_some());
    let d = series_view::drawn(&h.ctx).unwrap();
    // the 24 dates of the year, some of them clouds for this object; the band of the samples of its class
    assert_eq!((d.id, d.second, d.dates), (a, None, 24));
    assert!(d.values > 12 && d.values <= 24, "{} dates with a value", d.values);
    let name = o.classes[class as usize - 1].name.clone();
    assert_eq!(d.class.as_deref(), Some(name.as_str()));
    assert_eq!(d.band, 12, "the twelve samples of {name}");
    // (the band is the median and the quartiles of the samples' series, by date)
    let samples = series_view::samples_of(h.state().session.as_ref().unwrap(), &o, &name);
    assert_eq!(samples.len(), 12);
    // ctrl+click another parcel: a second profile to compare
    let (row, col) = cell(b);
    click_cell(&mut h, row, col, egui::Modifiers::COMMAND);
    let mut both = selected(&h);
    both.sort_unstable();
    assert_eq!(both, vec![a.min(b), a.max(b)]);
    let d = series_view::drawn(&h.ctx).unwrap();
    let (first, second) = (a.max(b), a.min(b));
    assert_eq!((d.id, d.second), (first, Some(second)));
    // the profile is of the object that is told (the last), and a click elsewhere takes the second away
    let (row, col) = cell(a);
    click_cell(&mut h, row, col, egui::Modifiers::NONE);
    assert_eq!(selected(&h), vec![a]);
    assert_eq!(series_view::drawn(&h.ctx).unwrap().second, None);
}
