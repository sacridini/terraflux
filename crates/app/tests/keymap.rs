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

#[test]
fn a_shortcut_changed_in_the_settings_is_the_one_that_works() {
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
    assert_eq!(clashes.len(), 1);
    assert!(
        clashes[0].error && (clashes[0].a.id, clashes[0].b.id) == ("run", "new")
            || (clashes[0].a.id, clashes[0].b.id) == ("new", "run")
    );
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
    assert!(terraflux_app::keymap::conflicts().is_empty());
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
