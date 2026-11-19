//! The feature space of objects in the Viewer: the separability of the classes of the samples, a rule made from a range
//! of an attribute (written into the model's objects.rules, one step of undo), and the list of the most uncertain
//! objects that selects an object and leads the map to it.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_app::objects_inspect::{Sample, sample_cell};
use terraflux_graph::{Model, Node, Source};
use terraflux_ops::objects::Objects;
use terraflux_ops::{Registry, Value};

/// Objects of a random map classed in three, and (with `rules`) classed by two rules on their cells.
fn model(rules: bool) -> Model {
    let mut m = Model::new("objects space");
    m.nodes.push(
        Node::new("land", "nlm.mpd")
            .param("rows", 40)
            .param("columns", 60)
            .param("seed", 3)
            .param("nlmr_compat", false),
    );
    m.nodes.push(Node::new("cls", "nlm.classify").param("n", 3).input("map", "land.map"));
    m.nodes.push(Node::new("seg", "segment.from_map").input("map", "cls.classes"));
    if rules {
        m.nodes.push(Node::new("rules", "objects.rules").input("objects", "seg.objects").param(
            "classes",
            serde_json::json!([
                { "name": "big", "rule": "above(cells, 1, 4)" },
                { "name": "small", "rule": "below(cells, 1, 4)" },
            ]),
        ));
    }
    m
}

fn harness(name: &str, rules: bool) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("model.toml");
    model(rules).save(&p).unwrap();
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

/// Step until `done`, a minute at most.
fn until(h: &mut Harness<TerrafluxApp>, what: &str, done: impl Fn(&Harness<TerrafluxApp>) -> bool) {
    let t = Instant::now();
    while !done(h) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "{what}: too long");
    }
}

/// The node run and its objects shown in the Viewer.
fn run_and_show(h: &mut Harness<TerrafluxApp>, node: &str) {
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), node);
    }
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(3);
    h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Enter);
    let t = Instant::now();
    loop {
        let s = h.state().session.as_ref().unwrap();
        if s.outputs.contains_key(node) {
            break;
        }
        assert!(t.elapsed() < Duration::from_secs(60), "the run: too long ({:?}, {:?})", s.status, s.editor.message);
        h.step();
        std::thread::sleep(Duration::from_millis(10));
    }
    h.state_mut().session.as_mut().unwrap().preview = Some((node.into(), "objects".into()));
    h.run_steps(3);
    until(h, "the objects drawn", |h| h.state().session.as_ref().unwrap().preview_view.objects_drawn() == 1);
}

fn objects_of(h: &Harness<TerrafluxApp>, node: &str) -> std::sync::Arc<Objects> {
    match h.state().session.as_ref().unwrap().value_of(node, "objects") {
        Some(Value::Objects(o)) => o,
        other => panic!("{node}.objects: {other:?}"),
    }
}

fn selected(h: &Harness<TerrafluxApp>) -> Vec<u32> {
    h.state().session.as_ref().unwrap().preview_objects.selected.iter().copied().collect()
}

fn objects_ui<'a>(h: &'a mut Harness<'_, TerrafluxApp>) -> &'a mut terraflux_app::objects_inspect::ObjectsUi {
    &mut h.state_mut().session.as_mut().unwrap().preview_objects
}

/// Click the (first) widget with this text, scrolled into view in the window first (the window scrolls).
fn reach_and_click(h: &mut Harness<TerrafluxApp>, text: &str) {
    h.query_all_by_label_contains(text).next().unwrap_or_else(|| panic!("no {text}")).scroll_to_me();
    h.run_steps(3);
    h.query_all_by_label_contains(text).next().unwrap().click();
    h.run_steps(4);
}

fn column(o: &Objects, name: &str) -> usize {
    o.table.columns.iter().position(|c| c == name).unwrap_or_else(|| panic!("no column {name}"))
}

/// Samples of two classes on the objects: the four smallest (small) and the four largest (big) by cells.
fn give_samples(h: &mut Harness<TerrafluxApp>, o: &Objects) -> Vec<u32> {
    let cells = column(o, "cells");
    let mut ids: Vec<u32> = (1..=o.count as u32).collect();
    ids.sort_by(|a, b| {
        o.table.rows[*a as usize - 1][cells].total_cmp(&o.table.rows[*b as usize - 1][cells]).then(a.cmp(b))
    });
    let small: Vec<u32> = ids.iter().copied().take(4).collect();
    let big: Vec<u32> = ids.iter().rev().copied().take(4).collect();
    let st = objects_ui(h);
    st.classes = vec!["big".into(), "small".into()];
    for (class, group) in [(0, &big), (1, &small)] {
        for &id in group {
            let (row, col) = sample_cell(o, id).unwrap();
            st.samples.push(Sample { row, col, class });
        }
    }
    small.into_iter().chain(big).collect()
}

/// The margins shown in the list of the most uncertain, in the order shown, with their objects.
fn listed(h: &Harness<TerrafluxApp>) -> Vec<(u32, f64)> {
    h.query_all_by_label_contains("(margin ")
        .filter_map(|n| {
            let t = n.accesskit_node().label()?;
            let id: u32 = t.split(':').next()?.parse().ok()?;
            let m: f64 = t.split("(margin ").nth(1)?.trim_end_matches(')').parse().ok()?;
            Some((id, m))
        })
        .collect()
}

#[test]
fn the_feature_space_tells_the_separability_and_the_most_uncertain_objects() {
    let mut h = harness("objects-space-uncertain", true);
    run_and_show(&mut h, "rules");
    let o = objects_of(&h, "rules");
    let sampled = give_samples(&mut h, &o);
    assert!(has(&h, "Feature space..."), "the button");
    h.get_by_label("Feature space...").click();
    h.run_steps(4);
    // the separability: the distance of the pair of classes by attribute, and the attributes that separate most
    until(&mut h, "the feature space", |h| has(h, "Separability"));
    assert!(has(&h, "big / small"), "a row for the pair of classes");
    assert!(has(&h, "The attributes that separate most"));
    assert!(has(&h, "least distance "), "the ranking");
    assert!(has(&h, "Most uncertain"));
    // the list: the objects without a sample of the smallest margin, the smallest first
    let table = match h.state().session.as_ref().unwrap().value_of("rules", "table") {
        Some(Value::Table(t)) => t,
        other => panic!("{other:?}"),
    };
    let margin = table.column("margin").expect("the margin column").to_vec();
    let mut want: Vec<(u32, f64)> =
        (1..=o.count as u32).filter(|i| !sampled.contains(i)).map(|i| (i, margin[i as usize - 1])).collect();
    want.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    want.truncate(20);
    let got = listed(&h);
    assert!(!got.is_empty() && got.len() <= 20, "{got:?}");
    assert!(got.windows(2).all(|w| w[0].1 <= w[1].1), "by margin: {got:?}");
    // (what is drawn is the start of the list, to the precision shown)
    assert!((got[0].1 - want[0].1).abs() < 0.006, "{got:?} against {want:?}");
    // a click on a row selects the object and takes the map to it
    let (id, _) = got[0];
    reach_and_click(&mut h, "(margin ");
    h.run_steps(4);
    assert_eq!(selected(&h), vec![id]);
    let s = h.state().session.as_ref().unwrap();
    let e = s.preview_view.view_extent().expect("the view");
    let (row, col) = sample_cell(&o, id).unwrap();
    let t = o.grid.transform;
    let (x, y) = (t[0] + (col as f64 + 0.5) * t[1], t[3] + (row as f64 + 0.5) * t[5]);
    assert!(e[0] <= x && x <= e[2] && e[1] <= y && y <= e[3], "the object {id} at {x},{y} is in view {e:?}");
}

#[test]
fn a_range_of_an_attribute_becomes_a_rule_in_one_step_of_undo() {
    let mut h = harness("objects-space-rule", true);
    run_and_show(&mut h, "rules");
    let o = objects_of(&h, "rules");
    let cells = column(&o, "cells");
    let (lo, hi): (f64, f64) = (5.0, 12.0);
    {
        let st = objects_ui(&mut h);
        st.space.open = true;
        st.space.hist = cells;
        st.space.lo = Some(lo);
        st.space.hi = Some(hi);
        st.space.class = "big".into();
    }
    until(&mut h, "the rule offered", |h| has(h, "objects inside: "));
    // the objects inside the range are lit on the map: they are the selection
    let inside: Vec<u32> =
        (1..=o.count as u32).filter(|&i| (lo..=hi).contains(&o.table.rows[i as usize - 1][cells])).collect();
    assert!(!inside.is_empty(), "objects of 5 to 12 cells");
    assert_eq!(selected(&h), inside);
    let values: Vec<f64> = o.table.rows.iter().map(|r| r[cells]).collect();
    let span = values.iter().copied().fold(f64::MIN, f64::max) - values.iter().copied().fold(f64::MAX, f64::min);
    let rule = terraflux_app::feature_space::range_rule("cells", Some(lo), Some(hi), span).unwrap();
    assert!(rule.starts_with("between(cells, "), "{rule}");
    let rule_of = |h: &Harness<TerrafluxApp>, class: &str| -> Option<String> {
        let s = h.state().session.as_ref().unwrap();
        let n = s.source.nodes.iter().find(|n| n.id == "rules")?;
        n.params["classes"]
            .as_array()?
            .iter()
            .find(|c| c["name"] == class)
            .and_then(|c| c["rule"].as_str().map(String::from))
    };
    assert_eq!(rule_of(&h, "big").as_deref(), Some("above(cells, 1, 4)"));
    reach_and_click(&mut h, "Make a rule");
    h.run_steps(4);
    // appended to the class's own rule with `and`
    assert_eq!(rule_of(&h, "big"), Some(format!("above(cells, 1, 4) and {rule}")));
    assert_eq!(rule_of(&h, "small").as_deref(), Some("below(cells, 1, 4)"), "the other class is as it was");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert!(s.history.undo(&mut s.source));
    }
    assert_eq!(rule_of(&h, "big").as_deref(), Some("above(cells, 1, 4)"), "one undo takes it back");
}

#[test]
fn with_no_rules_a_node_is_made_for_the_rule_and_undo_takes_it_away() {
    let mut h = harness("objects-space-new-rules", false);
    run_and_show(&mut h, "seg");
    let o = objects_of(&h, "seg");
    let cells = column(&o, "cells");
    {
        let st = objects_ui(&mut h);
        st.space.open = true;
        st.space.hist = cells;
        st.space.lo = Some(6.0);
        st.space.class = "big".into();
    }
    until(&mut h, "the rule offered", |h| has(h, "objects inside: "));
    reach_and_click(&mut h, "Make a rule");
    h.run_steps(4);
    {
        let s = h.state().session.as_ref().unwrap();
        let n = s.source.nodes.iter().find(|n| n.op == "objects.rules").expect("an objects.rules node");
        assert_eq!(n.inputs["objects"], Source::Ref("seg.objects".into()));
        let classes = n.params["classes"].as_array().unwrap();
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0]["name"], "big");
        let rule = classes[0]["rule"].as_str().unwrap();
        assert!(rule.starts_with("above(cells, ") && rule.ends_with(", 6)"), "{rule}");
    }
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert!(s.history.undo(&mut s.source));
        assert!(!s.source.nodes.iter().any(|n| n.op == "objects.rules"), "the node is gone");
    }
}
