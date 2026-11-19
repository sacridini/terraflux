//! A run of the application on the computers that work together: the tiles of `segment.multiresolution` and the
//! feature groups of `objects.features` go to the workers of the application's pool, and give what a local run gives;
//! a cluster that fails never fails the run.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_app::computers::Computers;
use terraflux_app::runner::engine_for;
use terraflux_exec::Quiet;
use terraflux_graph::{Model, Node};
use terraflux_ops::objects::{FeatureColumns, FeatureJob, TileCores, TileJob};
use terraflux_ops::{Table, Value};
use terraflux_raster::{Band, DataType, Grid, Raster};

/// A GeoTIFF of three f32 bands: blocks of two levels plus noise.
fn write_image(path: &Path, w: usize, h: usize) {
    let mut s = 5u64;
    let mut lcg = move || {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((s >> 11) as f64) / (1u64 << 53) as f64
    };
    let bands: Vec<Band> = (0..3)
        .map(|b| {
            let data = (0..w * h)
                .map(|i| {
                    let (r, c) = (i / w, i % w);
                    let level = if (r / 37 + c / 41) % 2 == 0 { 10.0 } else { 70.0 };
                    let noise = (lcg() + lcg() - 1.0) * 6.0;
                    (level + 3.0 * b as f64 + noise) as f32 as f64
                })
                .collect();
            Band { name: Some(format!("B{}", b + 2)), data }
        })
        .collect();
    let mut grid = Grid::new(w, h);
    grid.transform = [500_000.0, 10.0, 0.0, 5_000_000.0, 0.0, -10.0];
    Raster { grid, bands, dtype: DataType::F32, nodata: None }.write(path).unwrap();
}

fn model(path: &Path) -> Model {
    let mut m = Model::new("cluster run");
    m.nodes.push(Node::new("read", "raster.read").param("path", path.to_string_lossy().as_ref()));
    m.nodes.push(
        Node::new("seg", "segment.multiresolution")
            .input("image", "read.raster")
            .param("scale", 25)
            .param("shape", 0.1)
            .param("tile", 64)
            .param("margin", 12)
            .param("seed", 3),
    );
    m.nodes.push(
        Node::new("feat", "objects.features")
            .input("objects", "seg.objects")
            .input("image", "read.raster")
            .param("features", "all")
            .param("levels", 16),
    );
    m
}

fn dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// The labels and the table of a run.
fn of(results: &terraflux_exec::Results) -> (Vec<u32>, Table) {
    let Value::Objects(o) = &results.values["seg"]["objects"] else { panic!("objects") };
    let Value::Table(t) = &results.values["feat"]["table"] else { panic!("table") };
    (o.labels.to_vec(), t.clone())
}

fn same_table(a: &Table, b: &Table) {
    assert_eq!(a.columns, b.columns, "columns");
    assert_eq!(a.rows.len(), b.rows.len(), "rows");
    for (j, name) in a.columns.iter().enumerate() {
        for (i, (x, y)) in a.rows.iter().zip(&b.rows).enumerate() {
            assert!(x[j] == y[j] || (x[j].is_nan() && y[j].is_nan()), "{name} of object {}: {} {}", i + 1, x[j], y[j]);
        }
    }
}

#[test]
fn a_run_on_the_pool_gives_what_a_local_run_gives() {
    crate::point_settings();
    terraflux_ops::threads::ensure();
    let d = dir("cluster-run");
    let path = d.join("img.tif");
    write_image(&path, 256, 256);
    let m = model(&path);
    let want = engine_for(None, None).run(&m, &Default::default(), &mut Quiet).unwrap();
    assert!(want.warnings.iter().all(|w| !w.contains("cluster")), "{:?}", want.warnings);
    let (labels, table) = of(&want);
    assert!(table.rows.len() > 6, "{} objects", table.rows.len());

    // the application's pool with two workers of this computer, on 127.0.0.1; alone it is no cluster
    let mut c = Computers::default();
    c.dir = Some(d.join("pool"));
    c.network = Some(("127.0.0.1".parse().unwrap(), 17703));
    c.pool(2).unwrap();
    assert!(c.cluster().is_none(), "no other computer yet");
    let p = c.pool.as_ref().unwrap();
    let cluster = (p.url(), p.code().clone());
    let got = engine_for(None, Some(&cluster)).run(&m, &Default::default(), &mut Quiet).unwrap();
    assert!(got.warnings.iter().all(|w| !w.contains("cluster")), "{:?}", got.warnings);
    let (l, t) = of(&got);
    assert_eq!(l, labels, "labels");
    same_table(&t, &table);

    // a pool that is not there is no cluster: the run is local
    let gone = ("http://127.0.0.1:9".to_string(), p.code().clone());
    let local = engine_for(None, Some(&gone)).run(&m, &Default::default(), &mut Quiet).unwrap();
    assert_eq!(of(&local).0, labels);
}

/// A runner that always fails, as a computer that left.
#[derive(Debug)]
struct Failing;

impl TileCores for Failing {
    fn cores(&self, _: &TileJob) -> terraflux_ops::Result<Vec<Vec<u32>>> {
        Err(terraflux_ops::Error::msg("the computer left"))
    }

    fn features(&self, _: &FeatureJob) -> terraflux_ops::Result<Option<Vec<FeatureColumns>>> {
        Err(terraflux_ops::Error::msg("the computer left"))
    }
}

#[test]
fn a_failing_cluster_falls_back_to_this_computer_with_a_warning() {
    crate::point_settings();
    terraflux_ops::threads::ensure();
    let d = dir("cluster-fail");
    let path = d.join("img.tif");
    write_image(&path, 256, 256);
    let m = model(&path);
    let want = engine_for(None, None).run(&m, &Default::default(), &mut Quiet).unwrap();
    let got = terraflux_exec::Engine::default()
        .with_tile_runner(Some(Arc::new(Failing)))
        .run(&m, &Default::default(), &mut Quiet)
        .unwrap();
    let (l, t) = of(&got);
    let (labels, table) = of(&want);
    assert_eq!(l, labels, "labels");
    same_table(&t, &table);
    let said = |what: &str| got.warnings.iter().any(|w| w.contains(what) && w.contains("the computer left"));
    assert!(said("the tiles could not run on the cluster"), "{:?}", got.warnings);
    assert!(said("the features could not run on the cluster"), "{:?}", got.warnings);
}
