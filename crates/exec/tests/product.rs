//! What a map is travels with it to its file: raster.write of MSPA's map writes terraflux_product = MSPA in the
//! GeoTIFF (the old landschaft_product is still read), read back with the file; a map made from it is no longer MSPA.

use std::collections::BTreeMap;

use terraflux_exec::Engine;
use terraflux_graph::{Model, Node};

struct Quiet;
impl terraflux_exec::Observer for Quiet {}

#[test]
fn the_product_is_written_with_the_map() {
    let dir = std::env::temp_dir().join(format!("terraflux-product-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (mspa, other) = (dir.join("mspa.tif"), dir.join("other.tif"));
    let mut m = Model::new("t");
    m.nodes.push(
        Node::new("land", "nlm.random_cluster")
            .param("rows", 60)
            .param("columns", 60)
            .param("p", 0.55)
            .param("weights", serde_json::json!([0.4, 0.6]))
            .param("seed", 3),
    );
    m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "land.map").param("foreground", 2));
    m.nodes.push(Node::new("write", "raster.write").input("value", "mspa.map").param("path", mspa.to_string_lossy()));
    m.nodes.push(Node::new("again", "raster.reclassify").input("map", "mspa.map").param("rules", "* -> 1"));
    m.nodes
        .push(Node::new("write2", "raster.write").input("value", "again.map").param("path", other.to_string_lossy()));
    Engine::default().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
    let r = terraflux_raster::Raster::read(&mspa).unwrap();
    assert_eq!(r.grid.geo.item("terraflux_product"), Some("MSPA"));
    let r = terraflux_raster::Raster::read(&other).unwrap();
    assert_eq!(r.grid.geo.item("terraflux_product"), None);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_file_with_the_old_product_tag_is_still_recognised() {
    let dir = std::env::temp_dir().join(format!("terraflux-product-old-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("old.tif");
    let mut m = Model::new("t");
    m.nodes.push(
        Node::new("land", "nlm.random_cluster")
            .param("rows", 20)
            .param("columns", 20)
            .param("p", 0.55)
            .param("weights", serde_json::json!([0.4, 0.6]))
            .param("seed", 3),
    );
    m.nodes.push(Node::new("write", "raster.write").input("value", "land.map").param("path", path.to_string_lossy()));
    Engine::default().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
    // a GeoTIFF as the Python library wrote it before it was renamed: only landschaft_product
    let mut r = terraflux_raster::Raster::read(&path).unwrap();
    r.grid.geo.set_item("landschaft_product", Some("MSPA"));
    r.write(&path).unwrap();
    let r = terraflux_raster::Raster::read(&path).unwrap();
    assert_eq!(r.grid.geo.item("terraflux_product"), None);
    assert_eq!(r.grid.geo.item("landschaft_product"), Some("MSPA"));
    assert_eq!(r.grid.geo.product(), Some("MSPA"));
    // the new tag wins when both are there
    let mut geo = r.grid.geo.clone();
    geo.set_item("terraflux_product", Some("FAD"));
    assert_eq!(geo.product(), Some("FAD"));
    std::fs::remove_dir_all(&dir).ok();
}
