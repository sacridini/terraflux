//! A calculator saved as a node of the user's library from its pane: its inputs typed and the layers it reads
//! found, the dialog's Save writing `ndvi@1` and making the node of the model that version, and the node added
//! again from the operators' pane. The dialog and the node's pane are written to `target/tmp/ui-my-node-*.png` to be
//! looked at.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_graph::{GROUP, Model, Node, Type};

fn model() -> Model {
    let mut m = Model::new("my nodes");
    m.nodes.push(Node::new("land", "nlm.random").param("rows", 20).param("columns", 30).param("seed", 3));
    m.nodes.push(Node::new("twice", "raster.calc").param("expression", "a * 2").input("a", "land.map"));
    m.nodes.push(Node::new("s", "stack.of").input("red", "land.map").input("nir", "twice.map"));
    m.nodes.push(
        Node::new("calc", "raster.calc")
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
    // its value k a setting in the Node pane, changed there
    h.state_mut().focus(Pane::Node);
    h.run_steps(3);
    assert!(h.query_by_label("Settings").is_some());
    let field = h
        .get_all_by_value("10")
        .find(|n| n.accesskit_node().role() == egui::accesskit::Role::TextInput)
        .expect("the field of k");
    field.focus();
    h.run_steps(2);
    h.key_press(egui::Key::End);
    h.event(egui::Event::Text("5".into()));
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    let k = &s.source.nodes.iter().find(|n| n.id == "ndvi").unwrap().inputs["k"];
    assert_eq!(*k, terraflux_graph::Source::Literal(105.into()));
    // the quick search (/) finds it as my:ndvi; enter adds it
    h.state_mut().session.as_mut().unwrap().editor.quick = Some(Default::default());
    h.run_steps(2);
    h.event(egui::Event::Text("ndvi".into()));
    h.run_steps(2);
    assert!(h.query_by_label_contains("my:ndvi").is_some(), "offered by the quick search");
    h.key_press(egui::Key::Enter);
    h.run_steps(2);
    let s = h.state().session.as_ref().unwrap();
    let of_it = s.source.nodes.iter().filter(|n| n.uses.as_ref().is_some_and(|u| u.name == "ndvi")).count();
    assert_eq!(of_it, 3, "added by the quick search");
    // the assistant's tools (and the MCP server's) list it, tell it and add it, its settings given
    let r = terraflux_ops::Registry::standard();
    let listed = terraflux_app::tools::list_operators(&r, Some("vegetation"));
    assert!(listed.contains("my:ndvi (version 1): the vegetation index"), "{listed}");
    let told = terraflux_app::tools::describe_operator(&r, "my:ndvi").unwrap();
    assert!(told.contains("Settings:\n  k (whole number = 10, optional)"), "{told}");
    assert!(told.contains("Inputs:\n  s (stack (nir, red))"), "{told}");
    let mut m = model();
    let a = serde_json::json!({ "op": "my:ndvi", "id": "veg", "inputs": { "s": "s.stack" }, "settings": { "k": 3 } });
    terraflux_app::tools::edit("add_node", &mut m, &r, &dir, &a).unwrap();
    let veg = m.nodes.iter().find(|n| n.id == "veg").unwrap();
    assert_eq!(veg.uses.as_ref().unwrap().label(), "ndvi@1");
    assert_eq!(veg.inputs["k"], terraflux_graph::Source::Literal(3.into()));
    assert!(
        terraflux_app::tools::edit("add_node", &mut m, &r, &dir, &serde_json::json!({ "op": "my:nothing" })).is_err()
    );
    // a library shared with others, set in the settings: its nodes offered beside the user's
    let shared = dir.join("shared");
    let mut u = terraflux_graph::user_node::Library::new(dir.join("nodes")).get("ndvi", None).unwrap();
    u.name = "group_ndvi".into();
    terraflux_graph::user_node::Library::new(&shared).save(&u).unwrap();
    terraflux_app::settings::update(|st| st.node_libraries = vec![shared.clone()]);
    terraflux_app::my_nodes::changed();
    let libs = terraflux_app::my_nodes::libraries();
    assert_eq!(libs.iter().map(|l| l.name.as_str()).collect::<Vec<_>>(), ["My nodes", "shared"]);
    assert_eq!(terraflux_app::my_nodes::library_of("group_ndvi").lib.dir, shared);
    h.state_mut().focus(Pane::Operators);
    h.run_steps(3);
    assert!(h.query_by_label_contains("group_ndvi@1").is_some(), "the shared library's node in the operators' pane");
    shot(&mut h, "shared");
    terraflux_app::settings::update(|st| st.node_libraries.clear());
    terraflux_app::my_nodes::changed();
}
