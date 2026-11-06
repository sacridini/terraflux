//! A check run: the model on a part of the map or at a coarser resolution. The rasters a model reads come cut to the
//! window and one cell in every few, on the same grid when they were on one; the ones it draws come smaller.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use terraflux_exec::{Engine, Quiet};
use terraflux_graph::Model;
use terraflux_ops::Value;
use terraflux_ops::check::Check;
use terraflux_raster::{DataType, Grid, Raster};

/// A raster of 40 x 32 cells of 10 m, the corner at (1000, 5000), each cell its column number (classes).
fn file(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terraflux-check-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut grid = Grid::new(40, 32);
    grid.transform = [1000.0, 10.0, 0.0, 5000.0, 0.0, -10.0];
    let data = (0..40 * 32).map(|i| (i % 40) as f64).collect();
    let path = dir.join(name);
    Raster::single(grid, data, DataType::U8, None).write(&path).unwrap();
    path
}

fn model(path: &Path) -> Model {
    let p = path.to_string_lossy().replace('\\', "/");
    Model::from_toml(&format!(
        "format = \"terraflux/1\"\n\
         [[nodes]]\nid = \"land\"\nop = \"nlm.random\"\nparams = {{ rows = 40, columns = 32, resolution = 10, seed = 3 }}\n\
         [[nodes]]\nid = \"a\"\nop = \"raster.read\"\nparams = {{ path = \"{p}\" }}\n\
         [[nodes]]\nid = \"b\"\nop = \"raster.read\"\nparams = {{ path = \"{p}\" }}\n\
         [[nodes]]\nid = \"sum\"\nop = \"raster.calc\"\ninputs = {{ a = \"a.map\", b = \"b.map\" }}\nparams = {{ expression = \"a + b\" }}\n"
    ))
    .unwrap()
}

fn map(r: &terraflux_exec::Results, at: &str) -> terraflux_ops::Map {
    match r.get(at) {
        Some(Value::Map(m)) => m.clone(),
        other => panic!("{at}: {other:?}"),
    }
}

#[test]
fn a_coarser_check_reads_and_draws_fewer_cells_on_the_same_grid() {
    let path = file("coarse.tif");
    let r = Engine::checked(Check::new(None, 2)).run(&model(&path), &BTreeMap::new(), &mut Quiet).unwrap();
    let land = map(&r, "land.map");
    assert_eq!((land.grid.width, land.grid.height), (16, 20));
    assert_eq!(land.grid.transform[1], 20.0);
    // the rasters on the same grid stay on it: they meet in a map algebra node
    let sum = map(&r, "sum.map");
    assert_eq!((sum.grid.width, sum.grid.height), (20, 16));
    assert_eq!(sum.grid.transform, [1000.0, 20.0, 0.0, 5000.0, 0.0, -20.0]);
    // each cell the file's nearest (the column 1, 3, 5... of the block), doubled by the sum: classes stay whole
    assert_eq!(&sum.data[..4], &[2.0, 6.0, 10.0, 14.0]);
    // the normal run is the whole
    let whole = Engine::default().run(&model(&path), &BTreeMap::new(), &mut Quiet).unwrap();
    assert_eq!(map(&whole, "sum.map").grid.width, 40);
    assert_eq!(map(&whole, "land.map").grid.width, 32);
}

#[test]
fn a_window_cuts_the_files_to_the_cells_it_touches() {
    let path = file("window.tif");
    // x 1105..1245 is the columns 10 to 24, y 4715..4885 the rows 11 to 28 (moved out to the cells)
    let check = Check::new(Some(([1105.0, 4715.0, 1245.0, 4885.0], None)), 1);
    let r = Engine::checked(check).run(&model(&path), &BTreeMap::new(), &mut Quiet).unwrap();
    let sum = map(&r, "sum.map");
    assert_eq!((sum.grid.width, sum.grid.height), (15, 18));
    assert_eq!(sum.grid.transform, [1100.0, 10.0, 0.0, 4890.0, 0.0, -10.0]);
    assert_eq!(sum.data[0], 20.0);
    assert_eq!(sum.data[14], 48.0);
}

#[test]
fn a_window_that_misses_the_files_stops_the_run_with_the_file() {
    let path = file("miss.tif");
    let check = Check::new(Some(([0.0, 0.0, 10.0, 10.0], None)), 1);
    let e = Engine::checked(check).run(&model(&path), &BTreeMap::new(), &mut Quiet).err().unwrap().to_string();
    assert!(e.contains("miss.tif") && e.contains("does not meet"), "{e}");
}

#[test]
fn the_files_a_model_writes_go_to_the_checks_folder_and_not_over_the_real_ones() {
    let path = file("write.tif");
    let dir = path.parent().unwrap().join("real");
    let apart = path.parent().unwrap().join("apart");
    let real = dir.join("sum.tif");
    let m = Model::from_toml(&format!(
        "format = \"terraflux/1\"\n\
         [[nodes]]\nid = \"a\"\nop = \"raster.read\"\nparams = {{ path = \"{}\" }}\n\
         [[nodes]]\nid = \"out\"\nop = \"raster.write\"\ninputs = {{ value = \"a.map\" }}\nparams = {{ path = \"{}\" }}\n",
        path.to_string_lossy().replace('\\', "/"),
        real.to_string_lossy().replace('\\', "/")
    ))
    .unwrap();
    let check = Check::new(None, 4).writing_to(&apart);
    Engine::checked(check).run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
    assert!(!real.exists(), "the real file untouched");
    let small = Raster::read(apart.join("sum.tif")).unwrap();
    assert_eq!((small.grid.width, small.grid.height), (10, 8));
    // a normal run writes where it is said
    Engine::default().run(&m, &BTreeMap::new(), &mut Quiet).unwrap();
    assert_eq!(Raster::read(&real).unwrap().grid.width, 40);
}

#[test]
fn the_inputs_of_maps_are_cut_and_coarsened_too() {
    let path = file("input.tif");
    let p = path.to_string_lossy().replace('\\', "/");
    let m = Model::from_toml(&format!(
        "format = \"terraflux/1\"\n[inputs.cover]\nkind = \"map\"\npath = \"{p}\"\n\
         [[nodes]]\nid = \"plus\"\nop = \"raster.calc\"\ninputs = {{ a = \"$cover\" }}\nparams = {{ expression = \"a + 1\" }}\n"
    ))
    .unwrap();
    let check = Check::new(Some(([1000.0, 4680.0, 1200.0, 5000.0], None)), 4);
    let inputs =
        terraflux_exec::load_inputs_checked(&m, std::path::Path::new("."), &BTreeMap::new(), Some(&check)).unwrap();
    let Some(Value::Map(cover)) = inputs.get("cover") else { panic!("no map") };
    assert_eq!((cover.grid.width, cover.grid.height), (5, 8));
    assert_eq!(cover.grid.transform[1], 40.0);
    // the middle cell of each block of four columns: 2, 6, 10...
    assert_eq!(&cover.data[..3], &[2.0, 6.0, 10.0]);
    let r = Engine::checked(check).run(&m, &inputs, &mut Quiet).unwrap();
    assert_eq!(map(&r, "plus.map").data[0], 3.0);
}
