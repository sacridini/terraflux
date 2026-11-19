//! The Classes view of the Model pane: a card for each class of a model's tree (the groups with a `class` setting, or
//! the classes of `objects.rules`), a click on a card selecting its group, and the form of *Add a class...*.

use std::path::PathBuf;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::class_tree::{NewClass, Recipe, add_class};
use terraflux_graph::{Model, Node};

fn scene() -> Model {
    let mut m = Model::new("classes");
    m.nodes.push(Node::new("land", "nlm.fbm").param("rows", 30).param("columns", 40).param("seed", 5));
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("img", "nlm.image").input("classes", "cls.classes"));
    m.nodes.push(Node::new("ndsm", "nlm.random").param("rows", 30).param("columns", 40).param("seed", 9));
    m
}

fn tree() -> Model {
    let r = terraflux_ops::Registry::standard();
    let mut m = scene();
    for (name, parent, recipe) in [
        ("Urban", None, Recipe::Water),
        ("Buildings", Some("Urban"), Recipe::Buildings),
        ("Roads", Some("Urban"), Recipe::Buildings),
        ("Vegetation", None, Recipe::Vegetation),
        ("Water", None, Recipe::Water),
    ] {
        let c = NewClass { name: name.into(), colour: [60, 120, 200], parent: parent.map(String::from), recipe };
        add_class(&mut m, &[], &r, &c).unwrap();
    }
    m
}

fn rules() -> Model {
    let mut m = scene();
    m.nodes.push(Node::new("rules", "objects.rules").param(
        "classes",
        serde_json::json!([
            { "name": "Wet", "rule": "above(ndwi, 0, 0.2)" },
            { "name": "Lake", "parent": "Wet", "rule": "above(ndwi, 0.2, 0.4)" },
            { "name": "Dry" }
        ]),
    ));
    m
}

fn harness(model: Model, file: &str) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join(file);
    model.save(&p).unwrap();
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

fn has(h: &Harness<TerrafluxApp>, text: &str) -> bool {
    h.query_all_by_label_contains(text).next().is_some()
}

#[test]
fn the_classes_view_has_a_card_for_each_class_and_a_click_selects_its_group() {
    let mut h = harness(tree(), "class-tree.toml");
    h.state_mut().session.as_mut().unwrap().editor.classes = true;
    h.run_steps(3);
    for name in ["Urban", "Buildings", "Roads", "Vegetation", "Water"] {
        assert!(has(&h, name), "a card for {name}");
    }
    assert!(has(&h, "by NDWI") && has(&h, "by height (nDSM)") && has(&h, "by NDVI"));
    assert!(has(&h, "Add a class..."));
    // a click on a card: its group selected in the diagram
    h.get_by_label_contains("Buildings").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.selected.as_deref(), Some("class_Buildings"));
    assert!(!s.editor.classes, "back to the diagram");
}

#[test]
fn add_a_class_opens_its_form_and_adds_one_branch_in_one_step() {
    let mut h = harness(scene(), "class-tree-add.toml");
    h.state_mut().session.as_mut().unwrap().editor.classes = true;
    h.run_steps(3);
    assert!(has(&h, "No classes yet"));
    h.get_by_label("Add a class...").click();
    h.run_steps(3);
    assert!(has(&h, "Recipe") && has(&h, "Mother"), "the form");
    // what Add does with a name typed
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let r = terraflux_ops::Registry::standard();
        let c = NewClass { name: "Water".into(), colour: [0, 0, 255], parent: None, recipe: Recipe::Water };
        let mut m = s.source.clone();
        add_class(&mut m, &[], &r, &c).unwrap();
        s.source = m;
        s.history.name("added the class Water");
        s.edited();
    }
    h.run_steps(3);
    assert!(has(&h, "Water") && has(&h, "by NDWI"));
    let s = h.state_mut().session.as_mut().unwrap();
    let mut m = s.source.clone();
    assert!(s.history.undo(&mut m));
    assert!(terraflux_app::class_tree::read(&m.nodes).is_empty(), "undone with one step");
}

#[test]
fn a_model_with_objects_rules_shows_its_hierarchy_read_only() {
    let mut h = harness(rules(), "class-tree-rules.toml");
    h.state_mut().session.as_mut().unwrap().editor.classes = true;
    h.run_steps(3);
    for name in ["Wet", "Lake", "Dry"] {
        assert!(has(&h, name), "a card for {name}");
    }
    assert!(has(&h, "above(ndwi, 0, 0.2)"), "its rule");
    assert!(has(&h, "read only"));
    assert!(!has(&h, "Add a class..."), "the hierarchy is the setting's to change");
}
