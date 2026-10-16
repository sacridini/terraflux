//! What a node's input gets from rasters on disk, by the port's type: the band asked for of `raster.read`'s
//! raster (also on ports of any value: map algebra, groups).

use std::collections::BTreeMap;

use terraflux_exec::{Engine, Quiet};
use terraflux_graph::Model;
use terraflux_ops::Value;

fn run(nodes: &str) -> terraflux_exec::Results {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs/sibuyan_landuse.tif");
    let path = path.to_string_lossy().replace('\\', "/");
    let model = Model::from_toml(&format!(
        "format = \"terraflux/1\"\n[[nodes]]\nid = \"lu\"\nop = \"raster.read\"\nparams = {{ path = \"{path}\", band = \"2011\" }}\n{nodes}"
    ))
    .unwrap();
    Engine::default().run(&model, &BTreeMap::new(), &mut Quiet).unwrap()
}

#[test]
fn the_map_of_a_multi_band_raster_is_its_band_on_any_port() {
    // map algebra's inputs take any value: a band asked for is that band, a map (`lu.map`, of a model written
    // before raster.read had one output, read as `lu.raster`)
    let r = run(
        "[[nodes]]\nid = \"plus\"\nop = \"map.calc\"\ninputs = { a = \"lu.map\" }\nparams = { expression = \"a + 1\" }\n",
    );
    let Some(Value::Map(m)) = r.get("plus.map") else { panic!("no map") };
    let Some(Value::Raster(read)) = r.get("lu.raster") else { panic!("no raster") };
    let band = read.choose(read.layers.iter().position(|l| l.name == "2011").unwrap()).map().unwrap();
    assert!(m.data.iter().zip(band.data.iter()).all(|(p, b)| p.is_nan() && b.is_nan() || *p == b + 1.0));
}
