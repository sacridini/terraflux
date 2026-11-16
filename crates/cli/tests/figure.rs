//! `terraflux figure`: a model's graph written as SVG, PDF and PNG, at the width asked for.

use std::path::PathBuf;
use std::process::Command;

use terraflux_graph::{Model, Node};

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn the_graph_is_drawn_in_each_format() {
    let dir = tmp("figure");
    let mut m = Model::new("figure");
    let mut a = Node::new("land", "nlm.random_cluster").param("rows", 20).param("columns", 20);
    a.pos = Some([0.0, 0.0]);
    let mut b = Node::new("forest", "raster.reclassify").param("rules", "2 -> 2\n* -> 1").input("map", "land.map");
    b.pos = Some([320.0, 0.0]);
    m.nodes = vec![a, b];
    let model = dir.join("m.toml");
    m.save(&model).unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_terraflux")).arg("figure").arg(&model).args(args).output().unwrap()
    };

    let svg = dir.join("g.svg");
    let out = run(&["-o", svg.to_str().unwrap(), "--width", "85mm"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let text = std::fs::read_to_string(&svg).unwrap();
    assert!(
        text.contains("width=\"85.00mm\"")
            && text.contains(">forest</text>")
            && text.contains(">raster.reclassify</text>")
    );
    let said = String::from_utf8_lossy(&out.stderr).to_string();
    assert!(said.contains("2 nodes") && said.contains("85 mm wide"), "{said}");

    let pdf = dir.join("g.pdf");
    assert!(run(&["-o", pdf.to_str().unwrap()]).status.success());
    assert!(std::fs::read(&pdf).unwrap().starts_with(b"%PDF-"));
    let png = dir.join("g.png");
    assert!(run(&["-o", png.to_str().unwrap(), "--width", "natural"]).status.success());
    assert_eq!(&std::fs::read(&png).unwrap()[..4], b"\x89PNG");

    // a selection, and what is wrong said
    let one = dir.join("one.svg");
    assert!(run(&["-o", one.to_str().unwrap(), "--nodes", "forest"]).status.success());
    let text = std::fs::read_to_string(&one).unwrap();
    assert!(
        text.contains(">forest</text>") && !text.contains(">raster.read</text>") && text.contains(">land.map</text>")
    );
    let bad = run(&["-o", svg.to_str().unwrap(), "--width", "wide"]);
    assert!(!bad.status.success() && String::from_utf8_lossy(&bad.stderr).contains("not a width"));
    let bad = run(&["-o", svg.to_str().unwrap(), "--group", "nope"]);
    assert!(!bad.status.success() && String::from_utf8_lossy(&bad.stderr).contains("not in the model"));
}
