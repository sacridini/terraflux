//! What a map is travels with it to its file: raster.write of MSPA's map writes landschaft_product = MSPA in the
//! GeoTIFF (as landschaft does), read back with the file; a map made from it is no longer MSPA.

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
    assert_eq!(r.grid.geo.item("landschaft_product"), Some("MSPA"));
    let r = terraflux_raster::Raster::read(&other).unwrap();
    assert_eq!(r.grid.geo.item("landschaft_product"), None);
    std::fs::remove_dir_all(&dir).ok();
}
