//! A large test case: Sibuyan's land use and drivers repeated `rep x rep` times (real patterns, any size),
//! written tile by tile (never whole in memory), and a Dinamica model of it.
//!
//!     cargo run --release -p terraflux-engine --example big -- 64 C:/data/big
//!     terraflux run C:/data/big/model.toml --tiled --disk C:/data/big/layers

use std::path::PathBuf;

use terraflux_raster::tiff::TiledWriter;
use terraflux_raster::{DataType, Raster};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let rep: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(16);
    let out = PathBuf::from(args.get(2).cloned().unwrap_or_else(|| "big".into()));
    std::fs::create_dir_all(&out).unwrap();
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs");
    let lu = Raster::read(data.join("sibuyan_landuse.tif")).unwrap();
    let dr = Raster::read(data.join("sibuyan_drivers.tif")).unwrap();
    let (sw, sh) = (lu.grid.width, lu.grid.height);
    let (w, h) = (sw * rep, sh * rep);
    let mut grid = lu.grid.clone();
    grid.width = w;
    grid.height = h;
    println!("{w} x {h} = {} cells", w * h);
    let tile = 256;
    for (name, src, dtype) in [("landuse.tif", &lu, DataType::U8), ("drivers.tif", &dr, DataType::F32)] {
        let names = src.bands.iter().map(|b| b.name.clone()).collect();
        let mut tw = TiledWriter::create(out.join(name), &grid, names, dtype, src.nodata, tile, Some(1)).unwrap();
        for row in 0..h.div_ceil(tile) {
            for col in 0..w.div_ceil(tile) {
                let (x0, y0) = (col * tile, row * tile);
                let (tw_, th) = (tile.min(w - x0), tile.min(h - y0));
                let bands: Vec<Vec<f64>> = src
                    .bands
                    .iter()
                    .map(|b| {
                        let mut v = Vec::with_capacity(tw_ * th);
                        for y in y0..y0 + th {
                            for x in x0..x0 + tw_ {
                                v.push(b.data[(y % sh) * sw + x % sw]);
                            }
                        }
                        v
                    })
                    .collect();
                let refs: Vec<&[f64]> = bands.iter().map(|v| v.as_slice()).collect();
                tw.write_tile(col, row, &refs).unwrap();
            }
        }
        tw.finish().unwrap();
        println!("wrote {}", out.join(name).display());
    }
    let model = r#"format = "terraflux/1"
name = "Sibuyan repeated"

[inputs.t0]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "landuse.tif"
band = "2011"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"

[inputs.seed]
kind = "value"
value = 1

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = { t0 = "$t0", t1 = "$t1", steps = 14 }

[[nodes]]
id = "woe"
op = "woe.fit"
inputs = { t0 = "$t0", t1 = "$t1", drivers = "$drivers", markov = "markov.markov" }
params = { increment = 250.0 }

[[nodes]]
id = "patches"
op = "patches.observe"
inputs = { t0 = "$t0", t1 = "$t1", markov = "markov.markov" }
params = { isometry = 1.5 }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 3, landscape = "$t0" }

[nodes.body]
carry = { landscape = "allocate.landscape" }
collect = { maps = "allocate.landscape", info = "allocate.info" }

[[nodes.body.nodes]]
id = "probability"
op = "woe.probability"
inputs = { model = "woe.model", landscape = "@landscape", drivers = "$drivers" }

[[nodes.body.nodes]]
id = "allocate"
op = "dinamica.step"
inputs = { landscape = "@landscape", probability = "probability.probability", rates = "markov.markov", patches = "patches.patches", step = "@step", seed = "$seed" }
params = { prune_factor = 10.0, window = [3, 3] }

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
"#;
    std::fs::write(out.join("model.toml"), model).unwrap();
    println!("wrote {}", out.join("model.toml").display());
}
