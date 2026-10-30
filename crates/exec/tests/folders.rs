//! A folder of rasters given where a node takes one map: the node and the nodes after it that take its maps run as
//! a chain, file by file (not each node on all the files before the next); the files' names follow the maps (of
//! different grids too) to the files written and the tables gathered; a file that fails does not stop the others.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use terraflux_exec::{Engine, Observer};
use terraflux_graph::{Access, Model, Node, OpSignature, PortSpec, Type};
use terraflux_ops::{Args, Ctx, Operator, Outputs, Registry, Value};

/// The warnings told, by node.
#[derive(Default)]
struct Told(Vec<(String, String)>);

impl Observer for Told {
    fn warning(&mut self, at: &str, message: &str) {
        self.0.push((at.to_string(), message.to_string()));
    }
}

/// `test.log`: the map as it is, its width written down (with the node's name) when it runs; it fails on a map
/// narrower than `narrowest`.
struct Log(Arc<Mutex<Vec<String>>>);

impl Operator for Log {
    fn signature(&self) -> OpSignature {
        OpSignature::new("test.log", "", Access::Global)
            .input(PortSpec::new("map", Type::Map, ""))
            .output(PortSpec::new("map", Type::Map, ""))
            .param(PortSpec::optional("narrowest", Type::Integer, ""))
    }

    fn run(&self, ctx: &mut Ctx, a: &Args) -> terraflux_ops::Result<Outputs> {
        let m = a.map("map")?;
        self.0.lock().unwrap().push(format!("{} {}", a.node, m.grid.width));
        if m.grid.width < a.param("narrowest", 0usize)? {
            return Err(terraflux_ops::Error::msg(format!("{} columns: too narrow", m.grid.width)));
        }
        if m.grid.width == 30 {
            ctx.warn("thirty columns");
        }
        Ok([("map".to_string(), Value::Map(m.clone()))].into())
    }
}

fn engine(log: &Arc<Mutex<Vec<String>>>) -> Engine {
    let mut r = Registry::standard();
    r.add(Arc::new(Log(log.clone())));
    Engine { registry: r }
}

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("terraflux-folders-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("tiles")).unwrap();
    dir
}

/// A map of classes 1 (background) and 2 (forest) of `width` x 20 cells, written as `tiles/<name>.tif`.
fn tile(dir: &Path, name: &str, width: usize, x: f64) {
    let mut grid = terraflux_raster::Grid::new(width, 20);
    grid.transform[0] = x;
    let data: Vec<f64> = (0..width * 20).map(|i| if (i / 7 + i % 5) % 3 == 0 { 1.0 } else { 2.0 }).collect();
    let raster = terraflux_raster::Raster {
        grid,
        bands: vec![terraflux_raster::Band { name: None, data }],
        dtype: terraflux_raster::DataType::U8,
        nodata: None,
    };
    terraflux_ops::prepare::write_with_crs(&raster, &dir.join("tiles").join(format!("{name}.tif"))).unwrap();
}

fn read(dir: &Path) -> Node {
    Node::new("tiles", "raster.read").param("path", dir.join("tiles").to_string_lossy())
}

#[test]
fn a_chain_runs_file_by_file_and_writes_each_by_its_name() {
    let dir = folder("chain");
    tile(&dir, "basin_a", 30, 0.0);
    tile(&dir, "basin_b", 40, 100.0);
    tile(&dir, "basin_c", 50, 200.0);
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut m = Model::new("t");
    // a file at a time, to see the order
    m.nodes.push(read(&dir).param("workers", 1));
    m.nodes.push(Node::new("first", "test.log").input("map", "tiles.raster"));
    m.nodes.push(Node::new("second", "test.log").input("map", "first.map"));
    m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "second.map").param("foreground", 2));
    // without {name}: the maps are of different grids, a file each, its name before the file's
    let out = dir.join("out").join("mspa.tif");
    m.nodes.push(Node::new("w", "raster.write").input("value", "mspa.map").param("path", out.to_string_lossy()));
    // an operator of rasters on disk that takes one map runs once per file too
    m.nodes.push(
        Node::new("metrics", "metrics.class")
            .input("map", "tiles.raster")
            .param("metrics", serde_json::json!(["pland"])),
    );
    let mut told = Told::default();
    let r = engine(&log).run(&m, &BTreeMap::new(), &mut told).unwrap();

    // file by file: both nodes on a file before the next file
    assert_eq!(*log.lock().unwrap(), ["first 30", "second 30", "first 40", "second 40", "first 50", "second 50"]);
    // a warning of some files told once, with them
    assert!(told.0.iter().any(|(at, w)| at == "first" && w == "thirty columns (file basin_a)"), "{:?}", told.0);
    for name in ["basin_a", "basin_b", "basin_c"] {
        let f = terraflux_raster::Raster::read(dir.join("out").join(format!("{name}_mspa.tif"))).unwrap();
        assert_eq!(f.grid.geo.item("landschaft_product"), Some("MSPA"));
        assert_eq!(f.grid.geo.item(terraflux_ops::files::LAYER_ITEM), None, "the name is not written");
    }
    // the maps of different grids a series, each with its name
    let Value::Series(maps) = r.get("mspa.map").unwrap() else { panic!() };
    let names: Vec<_> = maps.iter().map(|v| terraflux_ops::files::layer_name(v).unwrap()).collect();
    assert_eq!(names, ["basin_a", "basin_b", "basin_c"]);
    // the tables one, a row per class of each file
    let Value::Table(t) = r.get("metrics.table").unwrap() else { panic!() };
    assert_eq!(t.columns[0], "layer");
    let layers: std::collections::BTreeSet<u64> = t.rows.iter().map(|r| r[0] as u64).collect();
    assert_eq!(layers.into_iter().collect::<Vec<_>>(), [1, 2, 3]);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn a_file_that_fails_does_not_stop_the_others() {
    let dir = folder("fails");
    tile(&dir, "a", 30, 0.0);
    tile(&dir, "b", 50, 100.0);
    tile(&dir, "c", 60, 200.0);
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut m = Model::new("t");
    m.nodes.push(read(&dir));
    m.nodes.push(Node::new("picky", "test.log").input("map", "tiles.raster").param("narrowest", 40));
    m.nodes.push(Node::new("after", "test.log").input("map", "picky.map"));
    let mut told = Told::default();
    let r = engine(&log).run(&m, &BTreeMap::new(), &mut told).unwrap();
    let Value::Series(maps) = r.get("after.map").unwrap() else { panic!() };
    let names: Vec<_> = maps.iter().map(|v| terraflux_ops::files::layer_name(v).unwrap()).collect();
    assert_eq!(names, ["b", "c"]);
    assert!(
        told.0.iter().any(|(at, w)| at == "picky"
            && w.starts_with("1 of the 3 files failed, the others went on: a (picky: 30 columns: too narrow)")),
        "{:?}",
        told.0
    );

    // all of them: an error
    let mut m = m.clone();
    m.nodes[1] = Node::new("picky", "test.log").input("map", "tiles.raster").param("narrowest", 100);
    let Err(e) = engine(&log).run(&m, &BTreeMap::new(), &mut Told::default()) else { panic!("it ran") };
    let e = e.to_string();
    assert!(e.contains("\"a\" (and every other of the 3 files)") && e.contains("too narrow"), "{e}");
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn files_of_one_grid_are_a_stack_written_as_one_file() {
    let dir = folder("grid");
    tile(&dir, "y2010", 30, 0.0);
    tile(&dir, "y2020", 30, 0.0);
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut m = Model::new("t");
    m.nodes.push(read(&dir));
    m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "tiles.raster").param("foreground", 2));
    let one = dir.join("mspa.tif");
    m.nodes.push(Node::new("w", "raster.write").input("value", "mspa.map").param("path", one.to_string_lossy()));
    let each = dir.join("{name}.tif");
    m.nodes.push(Node::new("w2", "raster.write").input("value", "mspa.map").param("path", each.to_string_lossy()));
    let r = engine(&log).run(&m, &BTreeMap::new(), &mut Told::default()).unwrap();
    let Value::Stack(s) = r.get("mspa.map").unwrap() else { panic!() };
    assert_eq!(s.keys.iter().map(|k| k.to_string()).collect::<Vec<_>>(), ["y2010", "y2020"]);
    assert_eq!(terraflux_raster::Raster::read(&one).unwrap().bands.len(), 2);
    for year in ["y2010", "y2020"] {
        assert!(dir.join(format!("{year}.tif")).exists());
    }
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn files_run_at_the_same_time_give_the_same() {
    let dir = folder("workers");
    for (i, name) in ["a", "b", "c", "d", "e"].iter().enumerate() {
        tile(&dir, name, 30 + 10 * i, 100.0 * i as f64);
    }
    let log = Arc::new(Mutex::new(Vec::new()));
    let run = |workers: u64, failing: bool| {
        let mut m = Model::new("t");
        m.nodes.push(read(&dir).param("workers", workers));
        let picky = Node::new("picky", "test.log").input("map", "tiles.raster");
        m.nodes.push(if failing { picky.param("narrowest", 50) } else { picky });
        m.nodes.push(Node::new("mspa", "landscape.mspa").input("map", "picky.map").param("foreground", 2));
        m.nodes.push(
            Node::new("metrics", "metrics.class")
                .input("map", "mspa.map")
                .param("metrics", serde_json::json!(["pland"])),
        );
        let mut told = Told::default();
        let r = engine(&log).run(&m, &BTreeMap::new(), &mut told).unwrap();
        let Value::Series(maps) = r.get("mspa.map").unwrap() else { panic!() };
        let maps: Vec<(String, Vec<f64>)> = maps
            .iter()
            .map(|v| {
                let Value::Map(m) = v else { panic!() };
                (terraflux_ops::files::layer_name(v).unwrap().to_string(), m.data.to_vec())
            })
            .collect();
        let Value::Table(t) = r.get("metrics.table").unwrap() else { panic!() };
        (maps, t.rows.clone(), told.0)
    };
    let one = run(1, false);
    assert_eq!(one.0.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["a", "b", "c", "d", "e"]);
    assert_eq!(run(3, false), one);
    // the failures and the warnings told the same, in the files' order
    let (maps, _, told) = run(3, true);
    assert_eq!(maps.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["c", "d", "e"]);
    assert_eq!(told, run(1, true).2);
    assert!(
        told.iter().any(|(_, w)| w.starts_with("2 of the 5 files failed, the others went on: a (picky")),
        "{told:?}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn files_done_before_are_not_run_again() {
    let dir = folder("resume");
    tile(&dir, "a", 30, 0.0);
    tile(&dir, "b", 40, 100.0);
    tile(&dir, "c", 50, 200.0);
    let log = Arc::new(Mutex::new(Vec::new()));
    let model = |edge: u64| {
        let mut m = Model::new("t");
        m.nodes.push(read(&dir));
        m.nodes.push(Node::new("seen", "test.log").input("map", "tiles.raster"));
        m.nodes.push(
            Node::new("mspa", "landscape.mspa")
                .input("map", "seen.map")
                .param("foreground", 2)
                .param("edge_width", edge),
        );
        let out = dir.join("out").join("{name}_mspa.tif");
        m.nodes.push(Node::new("w", "raster.write").input("value", "mspa.map").param("path", out.to_string_lossy()));
        m
    };
    let run = |m: &Model| {
        log.lock().unwrap().clear();
        let mut told = Told::default();
        let r = engine(&log).run(m, &BTreeMap::new(), &mut told).unwrap();
        let Value::Series(files) = r.get("w.path").unwrap() else { panic!() };
        let mut seen = log.lock().unwrap().clone();
        seen.sort();
        (files.len(), seen, told.0)
    };
    let (files, seen, _) = run(&model(1));
    assert_eq!((files, seen.len()), (3, 3));
    // the record of each file
    let rec: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("out").join("b_mspa.json")).unwrap()).unwrap();
    assert!(rec["input"].as_str().unwrap().ends_with("b.tif"), "{rec}");
    assert!(rec["outputs"]["w"].as_str().unwrap().ends_with("b_mspa.tif"), "{rec}");
    assert_eq!(rec["nodes"][1]["params"]["edge_width"], 1);
    // again: none runs, the files given
    let (files, seen, told) = run(&model(1));
    assert_eq!((files, seen.len()), (3, 0));
    assert!(told.iter().any(|(at, w)| at == "seen" && w.starts_with("3 of the 3 files were done before")), "{told:?}");
    // a file taken away: only it runs
    std::fs::remove_file(dir.join("out").join("c_mspa.tif")).unwrap();
    let (_, seen, _) = run(&model(1));
    assert_eq!(seen, ["seen 50"]);
    // another setting: all run
    let (_, seen, _) = run(&model(2));
    assert_eq!(seen.len(), 3);
    // something reads what the chain gives: every file runs, nothing taken from the records
    let mut m = model(2);
    m.nodes.push(
        Node::new("metrics", "metrics.class").input("map", "mspa.map").param("metrics", serde_json::json!(["pland"])),
    );
    let (_, seen, _) = run(&m);
    assert_eq!(seen.len(), 3);
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn the_first_files_of_a_folder_try_the_chain() {
    let dir = folder("first");
    tile(&dir, "a", 30, 0.0);
    tile(&dir, "b", 40, 100.0);
    tile(&dir, "c", 50, 200.0);
    let log = Arc::new(Mutex::new(Vec::new()));
    let mut m = Model::new("t");
    m.nodes.push(read(&dir).param("first", 2));
    m.nodes.push(Node::new("seen", "test.log").input("map", "tiles.raster"));
    let r = engine(&log).run(&m, &BTreeMap::new(), &mut Told::default()).unwrap();
    let Value::Series(maps) = r.get("seen.map").unwrap() else { panic!() };
    assert_eq!(maps.len(), 2);
    std::fs::remove_dir_all(&dir).ok();
}
