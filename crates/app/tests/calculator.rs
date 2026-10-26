//! The calculator: a raster.calc node's script edited in its pane, its inputs' layers completed as they are typed,
//! its outputs previewed once its inputs have run, a line computed in its console, and the script given to the
//! node. Its last frame is written to `target/tmp/ui-calculator.png` to be looked at.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_graph::{Model, Node};

fn model() -> Model {
    let mut m = Model::new("calculator");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 40).param("columns", 60).param("seed", 3));
    m.nodes.push(Node::new("twice", "raster.calc").param("expression", "a * 2").input("a", "land.map"));
    m.nodes.push(Node::new("s", "stack.of").input("low", "land.map").input("high", "twice.map"));
    m.nodes.push(
        Node::new("calc", "raster.calc")
            .param("expression", "out top = land > 0.5")
            .input("land", "land.map")
            .input("s", "s.stack"),
    );
    m
}

fn harness() -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    unsafe { std::env::set_var("TERRAFLUX_SETTINGS", dir.join("settings.json")) };
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("calculator.toml");
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

/// Step until `done`, a minute at most.
fn until(h: &mut Harness<TerrafluxApp>, what: &str, done: impl Fn(&Harness<TerrafluxApp>) -> bool) {
    let t = Instant::now();
    while !done(h) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "{what}: too long");
    }
}

fn typed(h: &mut Harness<TerrafluxApp>, text: &str) {
    h.event(egui::Event::Text(text.into()));
    h.run_steps(2);
}

#[test]
fn a_script_typed_completed_previewed_and_given_to_its_node() {
    let mut h = harness();
    h.state_mut().session.as_mut().unwrap().selected = Some("calc".into());
    h.state_mut().focus(Pane::Calculator);
    h.run_steps(3);
    assert!(h.state().is_open(Pane::Calculator));
    // not run yet: its inputs said to be run first
    assert!(h.query_by_label_contains("not run yet").is_some());
    h.get_by_label("Apply and run").click();
    until(&mut h, "the run", |h| h.query_by_label_contains("min 0").is_some());
    // the preview of the output, and the inputs' kinds
    assert!(h.query_by_label("top").is_some());
    assert!(h.query_by_label("s: stack · 2").is_some());
    // typing in the script: the layers of s completed after the dot
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("calculator script")));
    h.run_steps(2);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::End);
    typed(&mut h, "\nout m = s.h");
    assert!(h.query_by_label_contains("high  layer").is_some(), "the layer high offered");
    h.key_press(egui::Key::Tab);
    h.run_steps(2);
    // the keyboard goes through the list (high, low): down to the second, enter puts it in
    typed(&mut h, " - s.");
    assert!(h.query_by_label_contains("low  layer").is_some(), "the layers offered");
    h.key_press(egui::Key::ArrowDown);
    h.run_steps(2);
    if let Ok(img) = h.render() {
        img.save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ui-calculator-chosen.png")).unwrap();
    }
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    let text = h.state().calculator.text().to_string();
    assert!(text.ends_with("s.high - s.low"), "the second taken by the keyboard: {text:?}");
    // an error underlined and told; then mended
    typed(&mut h, " +");
    assert!(h.query_by_label_contains("✖ line 2").is_some());
    h.key_press(egui::Key::Backspace);
    h.key_press(egui::Key::Backspace);
    h.run_steps(2);
    assert!(h.query_by_label_contains("✔ gives top, m").is_some());
    // given to the node by ctrl+enter, which runs the model up to it
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    let calc = s.source.nodes.iter().find(|n| n.id == "calc").unwrap();
    assert_eq!(calc.params["expression"].as_str(), Some("out top = land > 0.5\nout m = s.high - s.low"));
    until(&mut h, "the run to the node", |h| {
        h.state().session.as_ref().unwrap().outputs.get("calc").is_some_and(|o| o.contains_key("m"))
    });
    // a line of the console, computed at once
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("calculator console input")));
    h.run_steps(2);
    typed(&mut h, "s.high.max() / s.low.max()");
    h.key_press(egui::Key::Enter);
    until(&mut h, "the console's answer", |h| h.query_by_label("= 2").is_some());
    // the pane, to be looked at
    typed(&mut h, "s.high");
    h.key_press(egui::Key::Enter);
    until(&mut h, "the console's stack", |h| h.query_by_label_contains("= map 60 x 40").is_some());
    h.ctx.memory_mut(|m| m.request_focus(egui::Id::new("calculator script")));
    h.run_steps(2);
    typed(
        &mut h,
        "
out n = s.",
    );
    match h.render() {
        Ok(img) => img.save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("ui-calculator.png")).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): the calculator not drawn"),
    }
}
