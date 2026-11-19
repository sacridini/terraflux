//! The time of a segmenter told before it runs: a node over an image of 4000 x 4000 cells (a synthetic `nlm.mpd`, not
//! run: the size comes from the model) shows the estimate, and *Use tiles* writes `tile`.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use terraflux_app::TerrafluxApp;
use terraflux_app::app::Pane;
use terraflux_graph::{Model, Node};
use terraflux_ops::Registry;

fn model() -> Model {
    let mut m = Model::new("time estimate");
    m.nodes.push(Node::new("land", "nlm.mpd").param("rows", 4000).param("columns", 4000).param("seed", 3));
    m.nodes.push(Node::new("grow", "segment.region_growing").input("image", "land.map").param("threshold", 0.05));
    m
}

fn harness(name: &str) -> Harness<'static, TerrafluxApp> {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    crate::point_settings();
    terraflux_app::computers::AUTOSTART.store(false, std::sync::atomic::Ordering::Relaxed);
    let p = dir.join("model.toml");
    model().save(&p).unwrap();
    let mut app = TerrafluxApp::new(Some(p));
    app.session.as_mut().unwrap().wait();
    Harness::builder().with_size((1400.0, 900.0)).build_ui_state(|ui, app: &mut TerrafluxApp| app.frame(ui), app)
}

fn has(h: &Harness<TerrafluxApp>, text: &str) -> bool {
    h.query_all_by_label_contains(text).next().is_some()
}

fn until(h: &mut Harness<TerrafluxApp>, what: &str, done: impl Fn(&Harness<TerrafluxApp>) -> bool) {
    let t = Instant::now();
    while !done(h) {
        h.step();
        std::thread::sleep(Duration::from_millis(10));
        assert!(t.elapsed() < Duration::from_secs(60), "{what}: too long");
    }
}

#[test]
fn a_big_image_shows_its_time_and_use_tiles_writes_the_tile() {
    let mut h = harness("time-estimate");
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::select(s, &Registry::standard(), "grow");
    }
    h.state_mut().focus(Pane::Node);
    until(&mut h, "the estimate", |h| has(h, "by tiles of 512"));
    assert!(has(&h, "about "), "the time of the whole image");
    let s = h.state().session.as_ref().unwrap();
    assert!(s.outputs.is_empty(), "nothing ran: the size came from the model");
    assert!(s.source.nodes[1].params.get("tile").is_none());
    h.get_by_label("Use tiles").click();
    h.run_steps(4);
    let s = h.state().session.as_ref().unwrap();
    assert_eq!(s.source.nodes[1].params.get("tile").and_then(|v| v.as_u64()), Some(512));
    assert!(s.history.can_undo());
    // with tiles written the estimate is of the tiles, and the button is gone
    until(&mut h, "the estimate by tiles", |h| has(h, "(tiles of 512)"));
    assert!(h.query_by_label("Use tiles").is_none());
    {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::undo_redo(s, false);
    }
    let s = h.state().session.as_ref().unwrap();
    assert!(s.source.nodes[1].params.get("tile").is_none(), "one undo takes it away");
}
