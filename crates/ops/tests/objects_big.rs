//! Big images: rasters on disk read by windows (the tiles of `segment.multiresolution`) and by strips
//! (`objects.features`), with the results of the same image held in memory.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::Ordering;

use terraflux_ops::files::Rasters;
use terraflux_ops::objects::{LABELS_DISK_BYTES, Objects};
use terraflux_ops::objects::big::{STRIP_BYTES, STRIP_VALUES};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Table, Value};
use terraflux_raster::{Band, DataType, Grid, Raster};

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> terraflux_ops::Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let inputs = inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    let args = Args { node: "n", inputs, params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn objects_of(out: &Outputs) -> Arc<Objects> {
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

fn table_of(out: &Outputs) -> Table {
    let Value::Table(t) = &out["table"] else { panic!("table") };
    t.clone()
}

fn lcg(s: &mut u64) -> f64 {
    *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    ((*s >> 11) as f64) / (1u64 << 53) as f64
}

/// A scratch folder of the test.
fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("terraflux-objects-big-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// A GeoTIFF of f32 bands: blocks of two levels plus noise, a few cells without data (-9999) in the first band.
fn write_image(path: &Path, w: usize, h: usize, bands: usize, seed: u64, nodata: bool) {
    let mut s = seed;
    let layers: Vec<Band> = (0..bands)
        .map(|b| {
            let data = (0..w * h)
                .map(|i| {
                    let (r, c) = (i / w, i % w);
                    let level = if (r / 37 + c / 41) % 2 == 0 { 10.0 } else { 70.0 };
                    let noise = (lcg(&mut s) + lcg(&mut s) - 1.0) * 6.0;
                    if nodata && b == 0 && i % 211 == 0 {
                        -9999.0
                    } else {
                        (level + 3.0 * b as f64 + noise) as f32 as f64
                    }
                })
                .collect();
            Band { name: Some(format!("B{}", b + 2)), data }
        })
        .collect();
    let mut grid = Grid::new(w, h);
    grid.transform = [500_000.0, 10.0, 0.0, 5_000_000.0, 0.0, -10.0];
    Raster { grid, bands: layers, dtype: DataType::F32, nodata: Some(-9999.0) }.write(path).unwrap();
}

/// The same file as rasters on disk and read whole as a stack (or a map for one band).
fn open(path: &Path) -> (Value, Value) {
    let r = Arc::new(Rasters::open(&path.to_string_lossy(), None, None).unwrap());
    let mem = if r.layers.len() > 1 { Value::Stack(r.stack().unwrap()) } else { Value::Map(r.map().unwrap()) };
    (Value::Raster(r), mem)
}

/// Strips of a few blocks and features by strips whatever the size, for the small images of the tests.
fn small_strips() {
    STRIP_BYTES.store(0, Ordering::Relaxed);
    STRIP_VALUES.store(1, Ordering::Relaxed);
}

#[test]
fn tiles_read_from_disk_are_the_tiles_read_from_memory() {
    small_strips();
    let d = dir("tiles");
    let path = d.join("img.tif");
    write_image(&path, 150, 130, 3, 5, true);
    let (disk, mem) = open(&path);
    for (tile, margin, shape) in [(48, 12, 0.1), (64, 0, 0.3), (40, 8, 0.0)] {
        let p = serde_json::json!({ "scale": 25, "shape": shape, "tile": tile, "margin": margin, "seed": 3 });
        let a = run("segment.multiresolution", vec![("image", mem.clone())], p.clone()).unwrap();
        let b = run("segment.multiresolution", vec![("image", disk.clone())], p).unwrap();
        let (oa, ob) = (objects_of(&a), objects_of(&b));
        assert!(oa.count > 4, "tile {tile}: {} objects", oa.count);
        assert_eq!(oa.labels, ob.labels, "tile {tile} margin {margin} shape {shape}");
        let (ta, tb) = (table_of(&a), table_of(&b));
        assert_eq!(ta.columns, tb.columns);
        for (ra, rb) in ta.rows.iter().zip(&tb.rows) {
            for (x, y) in ra.iter().zip(rb) {
                assert!(x == y || (x.is_nan() && y.is_nan()), "tile {tile}: {x} against {y}");
            }
        }
    }
    // the scale of ESP needs the image in memory: asked for, from disk, with tiles
    let p = serde_json::json!({ "tile": 48 });
    let e = run("segment.multiresolution", vec![("image", disk.clone())], p).expect_err("no scale");
    assert!(e.to_string().contains("scale"), "{e}");
    // a single band of a file, chosen
    let one = d.join("one.tif");
    write_image(&one, 100, 90, 1, 9, false);
    let (disk, mem) = open(&one);
    let p = serde_json::json!({ "scale": 25, "tile": 40, "margin": 10 });
    let a = objects_of(&run("segment.multiresolution", vec![("image", mem)], p.clone()).unwrap());
    let b = objects_of(&run("segment.multiresolution", vec![("image", disk)], p).unwrap());
    assert_eq!(a.labels, b.labels);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn labels_in_a_file_are_the_labels_in_memory() {
    small_strips();
    let d = dir("labels");
    let path = d.join("img.tif");
    write_image(&path, 150, 130, 3, 5, true);
    let (disk, mem) = open(&path);
    let p = serde_json::json!({ "scale": 25, "shape": 0.1, "tile": 48, "margin": 12, "seed": 3 });
    // (a threshold far above any labels, then 0: every labels in a file)
    LABELS_DISK_BYTES.store(1 << 40, Ordering::Relaxed);
    let a = run("segment.multiresolution", vec![("image", mem.clone())], p.clone()).unwrap();
    let a_disk = run("segment.multiresolution", vec![("image", disk.clone())], p.clone()).unwrap();
    let af = run(
        "objects.features",
        vec![("objects", Value::Objects(objects_of(&a))), ("image", disk.clone())],
        serde_json::json!({ "features": "basic" }),
    )
    .unwrap();
    LABELS_DISK_BYTES.store(0, Ordering::Relaxed);
    let b = run("segment.multiresolution", vec![("image", mem)], p.clone()).unwrap();
    let b_disk = run("segment.multiresolution", vec![("image", disk.clone())], p).unwrap();
    let (oa, ob) = (objects_of(&a), objects_of(&b));
    assert!(oa.count > 4 && !oa.labels.is_mapped(), "{} objects, {:?}", oa.count, oa.labels);
    assert!(ob.labels.is_mapped() && objects_of(&b_disk).labels.is_mapped(), "{:?}", ob.labels);
    assert_eq!(oa.labels, ob.labels);
    assert_eq!(objects_of(&a_disk).labels, objects_of(&b_disk).labels);
    assert_eq!((&oa.cells, &oa.boxes, &oa.adjacency), (&ob.cells, &ob.boxes, &ob.adjacency));
    let (ta, tb) = (table_of(&a), table_of(&b));
    assert_eq!(ta.columns, tb.columns);
    assert_eq!(ta.rows.len(), tb.rows.len());
    for (ra, rb) in ta.rows.iter().zip(&tb.rows) {
        assert!(ra.iter().zip(rb).all(|(x, y)| x == y || (x.is_nan() && y.is_nan())));
    }
    // the features over labels in a file
    let bf = run(
        "objects.features",
        vec![("objects", Value::Objects(ob.clone())), ("image", disk)],
        serde_json::json!({ "features": "basic" }),
    )
    .unwrap();
    same_tables(&table_of(&af), &table_of(&bf), 0.0);
    // the store round trip, read back into a file too
    let back = Objects::from_bytes(&ob.to_bytes().unwrap()).unwrap();
    assert!(back.labels.is_mapped());
    assert_eq!(back.labels, oa.labels);
    assert_eq!(back.table.columns, tb.columns);
    LABELS_DISK_BYTES.store(u64::MAX, Ordering::Relaxed);
    std::fs::remove_dir_all(&d).ok();
}

/// A runner that does the tiles in this process, through the job a worker gets, and counts its calls.
struct InProcess(std::sync::atomic::AtomicUsize);

impl terraflux_ops::objects::TileCores for InProcess {
    fn cores(&self, job: &terraflux_ops::objects::TileJob) -> terraflux_ops::Result<Vec<Vec<u32>>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        // through a wire's form, as a worker would get it
        let back: terraflux_ops::objects::TileJob = serde_json::from_str(&serde_json::to_string(job).unwrap()).unwrap();
        let all: Vec<usize> = (0..back.tiles()).collect();
        back.cores(&all)
    }
}

#[test]
fn tiles_given_to_a_runner_are_the_tiles_run_here() {
    small_strips();
    let d = dir("runner");
    let path = d.join("img.tif");
    write_image(&path, 150, 130, 3, 5, true);
    let (disk, mem) = open(&path);
    let runner = Arc::new(InProcess(Default::default()));
    for (tile, margin, shape) in [(48, 12, 0.1), (64, 0, 0.3)] {
        let p = serde_json::json!({ "scale": 25, "shape": shape, "tile": tile, "margin": margin, "seed": 3 });
        let params = p.as_object().cloned().unwrap();
        let args = |v: &Value| Args { node: "n", inputs: [("image".to_string(), v.clone())].into(), params: &params };
        let r = Registry::standard();
        let op = r.get("segment.multiresolution").unwrap();
        let here = objects_of(&op.run(&mut Ctx::default(), &args(&disk)).unwrap());
        let before = runner.0.load(Ordering::Relaxed);
        let mut ctx = Ctx::default().with_tile_runner(Some(runner.clone()));
        let there = objects_of(&op.run(&mut ctx, &args(&disk)).unwrap());
        assert_eq!(runner.0.load(Ordering::Relaxed), before + 1, "the runner was not asked");
        assert!(here.count > 4);
        assert_eq!(here.labels, there.labels, "tile {tile} margin {margin}");
        assert!(ctx.warnings.is_empty(), "{:?}", ctx.warnings);
        // an image in memory runs here, with a warning
        let before = runner.0.load(Ordering::Relaxed);
        let mut ctx = Ctx::default().with_tile_runner(Some(runner.clone()));
        let mem_out = objects_of(&op.run(&mut ctx, &args(&mem)).unwrap());
        assert_eq!(runner.0.load(Ordering::Relaxed), before);
        assert_eq!(mem_out.labels, here.labels);
        assert!(ctx.warnings.iter().any(|w| w.contains("raster.read")), "{:?}", ctx.warnings);
    }
    std::fs::remove_dir_all(&d).ok();
}

/// Every column of two tables equal: the ones from sums within `tol`, the rest exactly.
fn same_tables(a: &Table, b: &Table, tol: f64) -> usize {
    assert_eq!(a.columns, b.columns);
    assert_eq!(a.rows.len(), b.rows.len());
    for (j, name) in a.columns.iter().enumerate() {
        for (k, (ra, rb)) in a.rows.iter().zip(&b.rows).enumerate() {
            let (x, y) = (ra[j], rb[j]);
            let limit = if name.starts_with("median") || name.starts_with("min") || name.starts_with("max") {
                0.0
            } else {
                tol
            };
            assert!(
                (x.is_nan() && y.is_nan()) || (x - y).abs() <= limit * (1.0 + x.abs()),
                "column {name}, object {}: {x} against {y}",
                k + 1
            );
        }
    }
    a.columns.len()
}

#[test]
fn features_by_strips_are_the_features_from_memory() {
    small_strips();
    let d = dir("features");
    let path = d.join("img.tif");
    write_image(&path, 150, 130, 4, 7, true);
    let (disk, mem) = open(&path);
    let seg = run(
        "segment.multiresolution",
        vec![("image", mem.clone())],
        serde_json::json!({ "scale": 20, "shape": 0.2, "seed": 1 }),
    )
    .unwrap();
    let objects = Value::Objects(objects_of(&seg));
    // a second file, one band, as `more`
    let more = d.join("more.tif");
    write_image(&more, 150, 130, 1, 11, false);
    let (more_disk, more_mem) = open(&more);
    for (features, levels) in [("all", 16), ("basic", 32)] {
        let p = serde_json::json!({ "features": features, "levels": levels });
        let a = run(
            "objects.features",
            vec![("objects", objects.clone()), ("image", mem.clone()), ("more", more_mem.clone())],
            p.clone(),
        )
        .unwrap();
        let b = run(
            "objects.features",
            vec![("objects", objects.clone()), ("image", disk.clone()), ("more", more_disk.clone())],
            p,
        )
        .unwrap();
        let columns = same_tables(&table_of(&a), &table_of(&b), 1e-9);
        if features == "all" {
            assert!(columns > 60, "{columns} columns");
        }
    }
    // the image from disk and `more` in memory, the texture of chosen layers
    let p =
        serde_json::json!({ "features": ["spectral", "texture", "nb_diff"], "texture_bands": ["B3", "brightness"] });
    let a = run(
        "objects.features",
        vec![("objects", objects.clone()), ("image", mem), ("more", more_mem.clone())],
        p.clone(),
    )
    .unwrap();
    let b = run("objects.features", vec![("objects", objects), ("image", disk), ("more", more_mem)], p).unwrap();
    same_tables(&table_of(&a), &table_of(&b), 1e-9);
    std::fs::remove_dir_all(&d).ok();
}

#[test]
fn the_grey_levels_cut_by_strips_are_the_percentiles_of_the_whole() {
    // constant, two values, and a band with a spread: the radix passes find the order statistics exactly
    small_strips();
    let d = dir("cut");
    let path = d.join("img.tif");
    write_image(&path, 90, 70, 2, 21, true);
    let (disk, mem) = open(&path);
    let seg = run("segment.multiresolution", vec![("image", mem.clone())], serde_json::json!({ "scale": 15 })).unwrap();
    let objects = Value::Objects(objects_of(&seg));
    for levels in [2, 7, 256] {
        let p = serde_json::json!({ "features": ["contrast", "entropy", "correlation"], "levels": levels });
        let a = run("objects.features", vec![("objects", objects.clone()), ("image", mem.clone())], p.clone()).unwrap();
        let b = run("objects.features", vec![("objects", objects.clone()), ("image", disk.clone())], p).unwrap();
        same_tables(&table_of(&a), &table_of(&b), 1e-9);
    }
    std::fs::remove_dir_all(&d).ok();
}

#[cfg(windows)]
fn peak_bytes() -> Option<u64> {
    #[repr(C)]
    struct Counters {
        cb: u32,
        faults: u32,
        peak: usize,
        working: usize,
        rest: [usize; 6],
    }
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(p: isize, c: *mut Counters, cb: u32) -> i32;
    }
    // SAFETY: `c` is zeroed memory of the right layout with its size set, for the call to fill
    let mut c: Counters = unsafe { std::mem::zeroed() };
    c.cb = std::mem::size_of::<Counters>() as u32;
    let ok = unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut c, c.cb) };
    (ok != 0).then_some(c.peak as u64)
}

#[cfg(all(unix, not(target_os = "macos")))]
fn peak_bytes() -> Option<u64> {
    let text = std::fs::read_to_string("/proc/self/status").ok()?;
    let line = text.lines().find(|l| l.starts_with("VmHWM:"))?;
    line.split_whitespace().nth(1)?.parse::<u64>().ok().map(|k| k * 1024)
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
fn peak_bytes() -> Option<u64> {
    None
}

/// 4000 x 4000 x 4 bands: the time and the process's peak memory of the tiles and the features from disk, then from
/// memory (the peak only grows: the second figure is the larger of both). `cargo test -- --ignored --nocapture big_image`.
#[test]
#[ignore = "a 1 GB file and a minute: run by hand with --nocapture"]
fn big_image_from_disk_and_from_memory() {
    // (the defaults of the strips)
    STRIP_BYTES.store(256 << 20, Ordering::Relaxed);
    STRIP_VALUES.store(1 << 23, Ordering::Relaxed);
    let (w, h, nb) = (4000usize, 4000usize, 4usize);
    let d = dir("big");
    let path = d.join("big.tif");
    let mb = |b: Option<u64>| b.map_or("?".to_string(), |b| format!("{:.0} MB", b as f64 / 1e6));
    let t = std::time::Instant::now();
    write_image(&path, w, h, nb, 3, true);
    println!(
        "written in {:.1} s, the file {:.0} MB, peak {}",
        t.elapsed().as_secs_f64(),
        std::fs::metadata(&path).unwrap().len() as f64 / 1e6,
        mb(peak_bytes())
    );
    let r = Arc::new(Rasters::open(&path.to_string_lossy(), None, None).unwrap());
    let disk = Value::Raster(r.clone());
    let p = serde_json::json!({ "scale": 25, "tile": 1024, "margin": 64, "seed": 1 });
    let t = std::time::Instant::now();
    let seg = run("segment.multiresolution", vec![("image", disk.clone())], p.clone()).unwrap();
    let objects = Value::Objects(objects_of(&seg));
    println!(
        "tiles from disk: {:.1} s, {} objects, peak {} (the image whole as numbers: {:.0} MB)",
        t.elapsed().as_secs_f64(),
        objects_of(&seg).count,
        mb(peak_bytes()),
        (w * h * nb * 8) as f64 / 1e6
    );
    let t = std::time::Instant::now();
    let f = run(
        "objects.features",
        vec![("objects", objects.clone()), ("image", disk.clone())],
        serde_json::json!({ "features": ["spectral", "shape", "texture", "context"] }),
    )
    .unwrap();
    println!(
        "features from disk: {:.1} s, {} columns, peak {}",
        t.elapsed().as_secs_f64(),
        table_of(&f).columns.len(),
        mb(peak_bytes())
    );
    // from memory
    let t = std::time::Instant::now();
    let mem = Value::Stack(r.stack().unwrap());
    println!("read whole: {:.1} s, peak {}", t.elapsed().as_secs_f64(), mb(peak_bytes()));
    let t = std::time::Instant::now();
    let seg2 = run("segment.multiresolution", vec![("image", mem.clone())], p).unwrap();
    println!("tiles from memory: {:.1} s, peak {}", t.elapsed().as_secs_f64(), mb(peak_bytes()));
    assert_eq!(objects_of(&seg).labels, objects_of(&seg2).labels);
    let t = std::time::Instant::now();
    let f2 = run(
        "objects.features",
        vec![("objects", objects), ("image", mem)],
        serde_json::json!({ "features": ["spectral", "shape", "texture", "context"] }),
    )
    .unwrap();
    println!("features from memory: {:.1} s, peak {}", t.elapsed().as_secs_f64(), mb(peak_bytes()));
    same_tables(&table_of(&f), &table_of(&f2), 1e-9);
    std::fs::remove_dir_all(&d).ok();
}

/// A hash of (cell, band, seed) as a number in 0..1: noise that does not depend on the order cells are made in.
fn noise(i: u64, b: u64, seed: u64) -> f64 {
    let mut x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (b + 1).wrapping_mul(0xBF58_476D_1CE4_E5B9) ^ seed;
    x ^= x >> 30;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 27;
    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    (x >> 11) as f64 / (1u64 << 53) as f64
}

/// A tiled, deflated u16 GeoTIFF of `bands` bands written tile by tile (never whole in memory): blocks of two levels
/// plus noise, as Sentinel-2 reflectance (0-10000).
fn write_scene(path: &Path, w: usize, h: usize, bands: usize, seed: u64) {
    use rayon::prelude::*;
    let mut grid = Grid::new(w, h);
    grid.transform = [500_000.0, 10.0, 0.0, 5_000_000.0, 0.0, -10.0];
    let names = (0..bands).map(|b| Some(format!("B{}", b + 2))).collect();
    let t = 256;
    let mut out =
        terraflux_raster::tiff::TiledWriter::create(path, &grid, names, DataType::U16, Some(0.0), t, Some(1)).unwrap();
    let (nx, ny) = (w.div_ceil(t), h.div_ceil(t));
    for row in 0..ny {
        // a row of tiles made in parallel, written in order
        let tiles: Vec<Vec<Vec<f64>>> = (0..nx)
            .into_par_iter()
            .map(|col| {
                let (tw, th) = (t.min(w - col * t), t.min(h - row * t));
                (0..bands)
                    .map(|b| {
                        let mut v = Vec::with_capacity(tw * th);
                        for y in 0..th {
                            for x in 0..tw {
                                let (r, c) = (row * t + y, col * t + x);
                                let level = if (r / 37 + c / 41) % 2 == 0 { 1000.0 } else { 4000.0 };
                                let n = noise((r * w + c) as u64, b as u64, seed) + noise((r * w + c) as u64, 99, seed);
                                v.push((level + 300.0 * b as f64 + (n - 1.0) * 300.0).round().max(1.0));
                            }
                        }
                        v
                    })
                    .collect()
            })
            .collect();
        for (col, tile) in tiles.iter().enumerate() {
            let refs: Vec<&[f64]> = tile.iter().map(|b| b.as_slice()).collect();
            out.write_tile(col, row, &refs).unwrap();
        }
    }
    out.finish().unwrap();
}

/// A whole Sentinel-2 scene, 10 980 x 10 980 x 10 bands of u16 on disk: `segment.multiresolution` by tiles and
/// `objects.features` basic, with the time and the process's peak working set of each step.
/// `cargo test -p terraflux-ops --release --test objects_big -- --ignored --nocapture whole_scene`.
#[test]
#[ignore = "a 2 GB file and many minutes: run by hand with --nocapture"]
fn whole_scene_from_disk() {
    STRIP_BYTES.store(256 << 20, Ordering::Relaxed);
    STRIP_VALUES.store(1 << 23, Ordering::Relaxed);
    let (w, h, nb) = (10_980usize, 10_980usize, 10usize);
    let d = dir("scene");
    let path = d.join("scene.tif");
    let mb = |b: Option<u64>| b.map_or("?".to_string(), |b| format!("{:.0} MB", b as f64 / 1e6));
    let t = std::time::Instant::now();
    write_scene(&path, w, h, nb, 3);
    println!(
        "written in {:.1} s, the file {:.0} MB, peak {}",
        t.elapsed().as_secs_f64(),
        std::fs::metadata(&path).unwrap().len() as f64 / 1e6,
        mb(peak_bytes())
    );
    let r = Arc::new(Rasters::open(&path.to_string_lossy(), None, None).unwrap());
    let disk = Value::Raster(r);
    let scale: f64 = std::env::var("SCENE_SCALE").ok().and_then(|s| s.parse().ok()).unwrap_or(300.0);
    let p = serde_json::json!({ "scale": scale, "tile": 512, "margin": 64, "seed": 1 });
    let t = std::time::Instant::now();
    let seg = run("segment.multiresolution", vec![("image", disk.clone())], p).unwrap();
    let objects = objects_of(&seg);
    println!(
        "tiles from disk: {:.1} s, {} objects ({:.0} cells each), peak {}",
        t.elapsed().as_secs_f64(),
        objects.count,
        (w * h) as f64 / objects.count as f64,
        mb(peak_bytes())
    );
    let t = std::time::Instant::now();
    let f = run(
        "objects.features",
        vec![("objects", Value::Objects(objects)), ("image", disk)],
        serde_json::json!({ "features": "basic" }),
    )
    .unwrap();
    println!(
        "features from disk: {:.1} s, {} columns, peak {}",
        t.elapsed().as_secs_f64(),
        table_of(&f).columns.len(),
        mb(peak_bytes())
    );
    std::fs::remove_dir_all(&d).ok();
}
