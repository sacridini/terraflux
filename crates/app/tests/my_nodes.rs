//! A calculator saved as a node of the user's library from its pane: its inputs typed and the layers it reads
//! found, the dialog's Save writing `ndvi@1` and making the node of the model that version, and the node added
//! again from the operators' pane. The dialog and the node's pane are written to `target/tmp/ui-my-node-*.png` to be
//! looked at.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_graph::{GROUP, Model, Node, Type};

fn model() -> Model {
    let mut m = Model::new("my nodes");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 3));
    m.nodes.push(Node::new("twice", "map.calc").param("expression", "a * 2").input("a", "land.map"));
    m.nodes.push(Node::new("s", "stack.of").input("red", "land.map").input("nir", "twice.map"));
    m.nodes.push(
        Node::new("calc", "map.calc")
            .param("expression", "out ndvi = (s.nir - s.red) / (s.nir + s.red) * k")
            .input("s", "s.stack")
            .literal("k", 10)
            .note("the vegetation index"),
    );
    m
}

fn harness(dir: &std::path::Path) -> Harness<'static, TerrafluxApp> {
    unsafe {
        std::env::set_var("TERRAFLUX_SETTINGS", dir.join("settings.json"));
        std::env::set_var("TERRAFLUX_NODES", dir.join("nodes"));
    }
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("my-nodes.toml");
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

fn shot(h: &mut Harness<TerrafluxApp>, name: &str) {
    match h.render() {
        Ok(img) => img.save(PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("ui-my-node-{name}.png"))).unwrap(),
        Err(e) => eprintln!("no renderer ({e}): {name} not drawn"),
    }
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

#[test]
fn a_calculator_saved_as_my_node_and_used_again() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("my-nodes");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut h = harness(&dir);
    // run first: the outputs' types are known from what they gave
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |h| h.state().session.as_ref().unwrap().outputs.contains_key("calc"));
    h.state_mut().session.as_mut().unwrap().selected = Some("calc".into());
    h.state_mut().focus(Pane::Node);
    h.run_steps(3);
    h.get_by_label("Save as my node...").click();
    h.run_steps(3);
    {
        let d = h.state().session.as_ref().unwrap().editor.saving.clone().expect("the dialog");
        assert!(d.calc);
        assert_eq!((d.name.as_str(), d.description.as_str()), ("calc", "the vegetation index"));
        let s = d.inputs.iter().find(|p| p.name == "s").unwrap();
        assert_eq!((s.ty, s.bands.as_str()), (Type::Stack, "nir, red"));
        let k = d.inputs.iter().find(|p| p.name == "k").unwrap();
        assert_eq!((k.ty, k.default.as_str()), (Type::Integer, "10"));
        assert_eq!(d.outputs[0].ty, Type::Map);
    }
    assert!(h.query_by_label_contains("a new node: version 1").is_some());
    shot(&mut h, "dialog");
    h.state_mut().session.as_mut().unwrap().editor.saving.as_mut().unwrap().name = "ndvi".into();
    h.run_steps(2);
    h.get_by_label("Save").click();
    h.run_steps(3);
    assert!(h.state().session.as_ref().unwrap().editor.saving.is_none(), "the dialog closed");
    assert!(dir.join("nodes/ndvi@1.toml").is_file());
    let n = h.state().session.as_ref().unwrap().source.nodes.iter().find(|n| n.id == "calc").unwrap().clone();
    assert_eq!(n.op, GROUP);
    assert_eq!(n.uses.as_ref().unwrap().label(), "ndvi@1");
    // its pane tells what it is
    h.run_steps(2);
    assert!(h.query_by_label("ndvi@1 · a node of yours").is_some());
    assert!(h.query_by_label("s: stack (nir, red)").is_some());
    shot(&mut h, "pane");
    // it runs as before
    h.state_mut().session.as_mut().unwrap().start(Box::new(|| {}));
    until(&mut h, "the run", |h| {
        let s = h.state().session.as_ref().unwrap();
        !s.running() && s.outputs.get("calc").is_some_and(|o| o.contains_key("ndvi"))
    });
    // added again from the operators' pane, after the stack selected
    h.state_mut().session.as_mut().unwrap().selected = Some("s".into());
    h.state_mut().focus(Pane::Operators);
    h.run_steps(3);
    h.get_by_label("ndvi@1").click();
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    let again = s.source.nodes.iter().find(|n| n.id == "ndvi").expect("added");
    assert_eq!(again.inputs["s"], terraflux_graph::Source::Ref("s.stack".into()));
    assert!(s.issues.iter().all(|i| i.severity != terraflux_graph::Severity::Error), "{:?}", s.issues);
}
