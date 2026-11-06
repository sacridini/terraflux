//! The keyboard shortcuts changed in Settings (Keyboard): recorded from the keys pressed, they are the ones that
//! work and the ones every menu, the palette and F1 show; two actions given the same one are told at once; Reset
//! brings the default back. A program of its own: a shortcut changed is the whole program's, for every test
//! running beside it.

use std::path::PathBuf;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

fn shot(h: &mut Harness<TerrafluxApp>, name: &str) {
    match h.render() {
        Ok(img) => img.save(tmp(&format!("ui-{name}.png"))).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
    }
}

/// Settings of its own (not the user's), in the classic look; no computer shared or lent.
fn setup() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    unsafe { std::env::set_var("TERRAFLUX_SETTINGS", tmp("keymap-settings.json")) };
    ONCE.call_once(|| {
        let _ = std::fs::remove_file(tmp("keymap-settings.json"));
        terraflux_app::settings::update(|st| st.theme = "Default".into());
    });
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
}

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

/// Step until nothing runs.
fn idle(h: &mut Harness<TerrafluxApp>) {
    h.run_steps(3);
    let t = std::time::Instant::now();
    while h.state().session.as_ref().is_some_and(|s| s.running() || s.loading()) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(120), "the run took too long");
    }
    h.run_steps(2);
}

/// Whether a key pressed starts a run of the model.
fn runs(h: &mut Harness<TerrafluxApp>, m: egui::Modifiers, key: egui::Key) -> bool {
    let before = h.state().session.as_ref().unwrap().started;
    h.key_press_modifiers(m, key);
    h.run_steps(2);
    let ran = h.state().session.as_ref().unwrap().started != before;
    idle(h);
    ran
}

/// One test at a time: a shortcut changed is the whole program's.
static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[test]
fn a_shortcut_changed_in_the_settings_is_the_one_that_works() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    let mut h = harness(TerrafluxApp::new(None));
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    idle(&mut h);
    let ctrl = egui::Modifiers::COMMAND;
    let none = egui::Modifiers::NONE;
    // the defaults
    assert_eq!(terraflux_app::keymap::text("run"), "F5");
    assert!(runs(&mut h, none, egui::Key::F5), "F5 runs the model");
    assert!(!runs(&mut h, ctrl, egui::Key::R), "ctrl+R runs nothing");

    // Settings, Keyboard: Run is the first row
    h.state_mut().settings_open = true;
    h.run_steps(2);
    h.get_by_label("Keyboard").click();
    h.run_steps(2);
    assert!(h.state().settings_keys);
    assert!(h.query_by_label("Run the whole model").is_some());
    shot(&mut h, "40-keyboard-settings");
    // click its shortcut, then press the new one
    h.get_by_label("F5").click();
    h.run_steps(2);
    assert!(h.query_by_label("press the keys...").is_some());
    // (the keys of the application wait: nothing runs while the shortcut is recorded)
    let before = h.state().session.as_ref().unwrap().started;
    h.key_press_modifiers(ctrl, egui::Key::R);
    h.run_steps(3);
    assert_eq!(h.state().session.as_ref().unwrap().started, before, "recording does not run the model");
    assert_eq!(terraflux_app::settings::get().shortcuts["run"], vec!["Ctrl+R".to_string()]);
    assert_eq!(terraflux_app::keymap::text("run"), "ctrl+R");
    assert!(h.query_by_label("press the keys...").is_none());
    assert!(h.query_by_label("Reset").is_some());
    shot(&mut h, "41-keyboard-rebound");
    h.state_mut().settings_open = false;
    h.run_steps(2);

    // ctrl+R runs the model and F5 no longer does
    assert!(runs(&mut h, ctrl, egui::Key::R), "ctrl+R runs the model");
    assert!(!runs(&mut h, none, egui::Key::F5), "F5 runs nothing now");

    // the Run button, the palette and F1 show it
    h.get_by_label("Run").hover();
    h.run_steps(8);
    assert!(h.query_by_label_contains("run the whole model (ctrl+R)").is_some(), "the button's hover text");
    h.key_press_modifiers(ctrl, egui::Key::K);
    h.run_steps(2);
    h.event(egui::Event::Text("run the whole".into()));
    h.run_steps(2);
    assert!(h.query_by_label_contains("Run the whole model ctrl+R").is_some(), "the palette shows it");
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert!(h.state().commands.is_none());
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    assert!(h.state().shortcuts_open);
    assert!(h.query_all_by_label("ctrl+R").next().is_some(), "F1 lists it");
    assert!(h.query_all_by_label("F5").next().is_none(), "and not the old one");
    shot(&mut h, "42-keyboard-f1");
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    assert!(!h.state().shortcuts_open);

    // another action given the same one: both told, at once
    h.state_mut().settings_open = true;
    h.run_steps(2);
    h.state_mut().keyboard.filter = "New model".into();
    h.run_steps(2);
    h.get_by_label("ctrl+N").click();
    h.run_steps(2);
    h.key_press_modifiers(ctrl, egui::Key::R);
    h.run_steps(3);
    assert_eq!(terraflux_app::keymap::text("new"), "ctrl+R");
    let clashes = terraflux_app::keymap::conflicts();
    // (the error: Run and New model; and a warning each with the canvas' rename, which has ctrl+R too)
    let errors: Vec<_> = clashes.iter().filter(|c| c.error).collect();
    assert_eq!(errors.len(), 1);
    assert!((errors[0].a.id, errors[0].b.id) == ("run", "new") || (errors[0].a.id, errors[0].b.id) == ("new", "run"));
    assert_eq!(clashes.iter().filter(|c| !c.error && c.b.id == "canvas_rename").count(), 2);
    assert!(h.query_by_label_contains("also Run the whole model").is_some(), "the row of New model says it");
    shot(&mut h, "43-keyboard-conflict");
    // Backspace takes a shortcut away; Esc leaves it as it is
    h.get_by_label("ctrl+R").click();
    h.run_steps(2);
    h.key_press(egui::Key::Escape);
    h.run_steps(2);
    assert_eq!(terraflux_app::keymap::text("new"), "ctrl+R");
    h.get_by_label("ctrl+R").click();
    h.run_steps(2);
    h.key_press(egui::Key::Backspace);
    h.run_steps(2);
    assert_eq!(terraflux_app::keymap::text("new"), "");
    assert_eq!(terraflux_app::settings::get().shortcuts["new"], Vec::<String>::new());
    // (only the warning of Run against the canvas' rename is left)
    assert!(terraflux_app::keymap::conflicts().iter().all(|c| !c.error));
    // Reset: back to ctrl+N (the filter shows New model only, so one Reset)
    h.get_by_label("Reset").click();
    h.run_steps(2);
    assert_eq!(terraflux_app::keymap::text("new"), "ctrl+N");
    assert!(!terraflux_app::settings::get().shortcuts.contains_key("new"));

    // Reset all: F5 is back and ctrl+R is nothing
    h.get_by_label("Reset all").click();
    h.run_steps(2);
    assert!(terraflux_app::settings::get().shortcuts.is_empty());
    assert_eq!(terraflux_app::keymap::text("run"), "F5");
    h.state_mut().settings_open = false;
    h.run_steps(2);
    assert!(runs(&mut h, none, egui::Key::F5), "F5 runs the model again");
    assert!(!runs(&mut h, ctrl, egui::Key::R), "ctrl+R does not");
}

fn rebind(id: &str, keys: &[&str]) {
    terraflux_app::settings::update(|st| {
        st.shortcuts.insert(id.to_string(), keys.iter().map(|k| k.to_string()).collect());
    });
}

fn reset_all() {
    terraflux_app::settings::update(|st| st.shortcuts.clear());
}

/// The model of a template run, a node selected, the pointer over the canvas.
fn on_the_canvas(h: &mut Harness<TerrafluxApp>) -> String {
    h.run_steps(2);
    let ctx = h.ctx.clone();
    h.state_mut().start_template(&ctx, &terraflux_app::templates::TEMPLATES[0]);
    idle(h);
    let first = h.state().session.as_ref().unwrap().source.nodes[0].id.clone();
    let reg = terraflux_ops::Registry::standard();
    terraflux_app::builder::select(h.state_mut().session.as_mut().unwrap(), &reg, &first);
    h.run_steps(3);
    let canvas = h.state().session.as_ref().unwrap().editor.canvas().expect("the canvas is drawn");
    h.event(egui::Event::PointerMoved(canvas.center()));
    h.run_steps(3);
    first
}

fn zoom(h: &Harness<TerrafluxApp>) -> f32 {
    h.state().session.as_ref().unwrap().editor.zoom()
}

fn far(h: &mut Harness<TerrafluxApp>) {
    h.state_mut().session.as_mut().unwrap().editor.set_camera(egui::vec2(0.0, 0.0), 0.3);
    h.run_steps(2);
    assert!((zoom(h) - 0.3).abs() < 1e-3);
}

#[test]
fn the_canvas_keys_follow_the_table() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    reset_all();
    let mut h = harness(TerrafluxApp::new(None));
    on_the_canvas(&mut h);
    let none = egui::Modifiers::NONE;
    assert_eq!(terraflux_app::keymap::text("canvas_frame"), "F");

    // F frames the node selected
    far(&mut h);
    h.key_press(egui::Key::F);
    h.run_steps(3);
    assert!(zoom(&h) > 1.0, "F frames the node: {}", zoom(&h));

    // given J: J frames it and F does not
    rebind("canvas_frame", &["J"]);
    assert_eq!(terraflux_app::keymap::text("canvas_frame"), "J");
    far(&mut h);
    h.key_press(egui::Key::F);
    h.run_steps(3);
    assert!((zoom(&h) - 0.3).abs() < 1e-3, "F does nothing now");
    h.key_press(egui::Key::J);
    h.run_steps(3);
    assert!(zoom(&h) > 1.0, "J frames the node: {}", zoom(&h));

    // the sheet of shortcuts and the status bar's hints show the new keys
    rebind("canvas_run_to", &["Ctrl+J"]);
    h.state_mut().focus(terraflux_app::app::Pane::Model);
    h.run_steps(3);
    assert!(h.query_by_label_contains("ctrl+J: run to it").is_some(), "the status bar");
    assert!(h.query_by_label_contains("ctrl+Enter: run to it").is_none());
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    assert!(h.state().shortcuts_open);
    assert!(h.query_all_by_label("J").next().is_some(), "F1 lists the new key");
    assert!(h.query_all_by_label("F").next().is_none(), "and not the old one");
    assert!(h.query_all_by_label("ctrl+J").next().is_some());
    assert!(h.query_all_by_label("The nodes selected in view (none selected: the whole graph)").next().is_some());
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    assert!(!h.state().shortcuts_open);

    // the Keyboard settings list the scopes
    h.state_mut().settings_open = true;
    h.run_steps(2);
    h.get_by_label("Keyboard").click();
    h.run_steps(2);
    h.state_mut().keyboard.filter = "view".into();
    h.run_steps(3);
    assert!(h.query_by_label("THE MODEL'S CANVAS").is_some());
    assert!(h.query_by_label("MAPS").is_some());
    assert!(h.query_by_label("The nodes selected in view (none selected: the whole graph)").is_some());
    shot(&mut h, "44-keyboard-scopes");
    h.state_mut().settings_open = false;
    h.run_steps(2);

    // the canvas' rename against the everywhere Run given the same key: over the canvas, the canvas' wins
    reset_all();
    rebind("run", &["Ctrl+R"]);
    let ctrl = egui::Modifiers::COMMAND;
    let before = h.state().session.as_ref().unwrap().started;
    let conflicts = terraflux_app::keymap::conflicts();
    assert!(conflicts.iter().any(|c| !c.error && c.a.id == "run" && c.b.id == "canvas_rename"));
    h.event(egui::Event::PointerMoved(h.state().session.as_ref().unwrap().editor.canvas().unwrap().center()));
    h.run_steps(3);
    h.key_press_modifiers(ctrl, egui::Key::R);
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.started, before, "over the canvas ctrl+R does not run the model");
    assert!(s.editor.renaming.is_some(), "it renames the node selected");
    h.key_press_modifiers(none, egui::Key::Escape);
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.renaming.is_none());
    // the pointer elsewhere (the menu bar): the application's
    h.event(egui::Event::PointerMoved(egui::pos2(2.0, 2.0)));
    h.run_steps(6);
    assert!(runs(&mut h, ctrl, egui::Key::R), "off the canvas ctrl+R runs the model");
    reset_all();
}

#[test]
fn the_viewer_keys_follow_the_table() {
    let _one = ONE.lock().unwrap_or_else(|e| e.into_inner());
    reset_all();
    let mut h = harness(TerrafluxApp::new(None));
    let first = on_the_canvas(&mut h);
    // a map in the Viewer: the output of a node that gives one
    let id = {
        let s = h.state().session.as_ref().unwrap();
        s.source.nodes.iter().find(|n| n.op.starts_with("nlm.")).map(|n| n.id.clone()).unwrap_or(first)
    };
    let reg = terraflux_ops::Registry::standard();
    terraflux_app::builder::select(h.state_mut().session.as_mut().unwrap(), &reg, &id);
    h.state_mut().focus(terraflux_app::app::Pane::Viewer);
    h.run_steps(10);
    let t = std::time::Instant::now();
    let rect = loop {
        let s = h.state().session.as_ref().unwrap();
        if let Some(r) = s.view.rect().or(s.preview_view.rect()) {
            break r;
        }
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "no map in the Viewer");
    };
    let basemap = |h: &Harness<TerrafluxApp>| {
        let s = h.state().session.as_ref().unwrap();
        (s.view.basemap, s.preview_view.basemap)
    };
    h.event(egui::Event::PointerMoved(rect.center()));
    h.run_steps(3);
    let start = basemap(&h);
    assert_eq!(terraflux_app::keymap::text("viewer_satellite"), "B");

    // B puts the satellite under the map
    h.key_press(egui::Key::B);
    h.run_steps(3);
    let on = basemap(&h);
    assert_ne!(on, start, "B changes the basemap");
    h.key_press(egui::Key::B);
    h.run_steps(3);
    assert_eq!(basemap(&h), start);

    // given V: V does and B does not
    rebind("viewer_satellite", &["V"]);
    h.key_press(egui::Key::B);
    h.run_steps(3);
    assert_eq!(basemap(&h), start, "B does nothing now");
    h.key_press(egui::Key::V);
    h.run_steps(3);
    assert_eq!(basemap(&h), on, "V puts the satellite under the map");
    h.key_press(egui::Key::V);
    h.run_steps(3);

    // the status bar and F1 say V
    assert!(h.query_by_label_contains("V: satellite under the map").is_some(), "the status bar");
    assert!(h.query_by_label_contains("B: satellite under the map").is_none());
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    assert!(h.state().shortcuts_open);
    assert!(h.query_by_label("The satellite under the map").is_some());
    assert!(h.query_all_by_label("V").next().is_some(), "F1 lists V");
    h.key_press(egui::Key::F1);
    h.run_steps(2);
    reset_all();
}
