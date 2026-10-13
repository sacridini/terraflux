//! Vectors over rasters read by strips (`block_rows`, and rasters on disk): the zones' statistics, the values at
//! points and the features burnt are those of the map in memory, to the bit.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_ops::files::Rasters;
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Table, Value};
use terraflux_raster::vector::{Column, Geometry, Vector, Xy};
use terraflux_raster::{Band, DataType, Grid, Raster};

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap()
}

fn table(out: &Outputs, port: &str) -> Table {
    let Value::Table(t) = &out[port] else { panic!("{port} is no table") };
    t.clone()
}

fn same_bits(a: &[f64], b: &[f64], what: &str) {
    assert_eq!(a.len(), b.len(), "{what}");
    for (i, (p, q)) in a.iter().zip(b).enumerate() {
        assert!(p.to_bits() == q.to_bits() || (p.is_nan() && q.is_nan()), "{what}: {i}: {p} {q}");
    }
}

fn same(a: &Table, b: &Table, what: &str) {
    assert_eq!(a.columns, b.columns, "{what}");
    assert_eq!(a.rows.len(), b.rows.len(), "{what}");
    for (i, (x, y)) in a.rows.iter().zip(&b.rows).enumerate() {
        same_bits(x, y, &format!("{what}: row {i}"));
    }
}

/// A deterministic draw in [0, 1) of a cell.
fn draw(seed: u64, i: usize, j: usize) -> f64 {
    let mut z = seed ^ ((i as u64) << 32 | j as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
}

/// A map of `ny` x `nx` cells of 10 x 10 from (1000, 5000) down, from a function of the row and column.
fn map_of(ny: usize, nx: usize, f: impl Fn(usize, usize) -> f64) -> Map {
    let mut grid = Grid::new(nx, ny);
    grid.transform = [1000.0, 10.0, 0.0, 5000.0, 0.0, -10.0];
    Map::new(Arc::new(grid), (0..ny * nx).map(|i| f(i / nx, i % nx)).collect())
}

/// Whole numbers in clumps, 3 % without a value.
fn classes(ny: usize, nx: usize) -> Map {
    map_of(
        ny,
        nx,
        |i, j| {
            if draw(1, i, j) < 0.03 { f64::NAN } else { (1.0 + (draw(2, i / 5, j / 5) * 9.0).floor()) * 10.0 }
        },
    )
}

/// Numbers with decimals, 3 % without a value.
fn decimals(ny: usize, nx: usize) -> Map {
    map_of(ny, nx, |i, j| if draw(3, i, j) < 0.03 { f64::NAN } else { draw(4, i, j) * 100.0 - 20.0 })
}

fn write(map: &Map, dtype: DataType, path: &Path) -> Value {
    let data = map.data.iter().map(|v| if v.is_nan() { -9999.0 } else { *v }).collect();
    Raster { grid: (*map.grid).clone(), bands: vec![Band { name: None, data }], dtype, nodata: Some(-9999.0) }
        .write(path)
        .unwrap();
    Value::Raster(Arc::new(Rasters::open(path.to_str().unwrap(), None, None).unwrap()))
}

fn dir(what: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("terraflux-vector-tiled-{what}-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Xy> {
    vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]
}

/// Zones over a map of `ny` x `nx` cells: small squares, a triangle, one over almost all of it, a ring with a
/// hole, two parts far apart, overlapping ones, and one outside it.
fn zones(ny: usize, nx: usize) -> Value {
    let (x1, y0) = (1000.0 + nx as f64 * 10.0, 5000.0 - ny as f64 * 10.0);
    let geometries = vec![
        Geometry::Polygon(vec![square(1105.0, 4405.0, 1287.0, 4633.0)]),
        Geometry::Polygon(vec![square(1200.0, 4500.0, 1450.0, 4800.0)]),
        Geometry::Polygon(vec![vec![[1500.0, 4000.0], [2200.0, 4100.0], [1700.0, 4900.0], [1500.0, 4000.0]]]),
        Geometry::Polygon(vec![square(1003.0, y0 + 7.0, x1 - 13.0, 4996.0)]),
        Geometry::Polygon(vec![square(1300.0, 3000.0, 2000.0, 3900.0), square(1500.0, 3200.0, 1800.0, 3600.0)]),
        Geometry::MultiPolygon(vec![
            vec![square(x1 - 300.0, y0 + 50.0, x1 - 100.0, y0 + 260.0)],
            vec![square(1020.0, 4700.0, 1140.0, 4950.0)],
        ]),
        Geometry::Polygon(vec![square(-500.0, -500.0, -100.0, -100.0)]),
        Geometry::Empty,
    ];
    let n = geometries.len();
    Value::Vector(Arc::new(Vector {
        geometries,
        attributes: vec![("id".into(), Column::Numbers((1..=n).map(|i| i as f64).collect()))],
        crs: None,
    }))
}

const STATS: [&str; 16] = [
    "count",
    "nodata",
    "sum",
    "mean",
    "min",
    "max",
    "range",
    "std",
    "var",
    "median",
    "majority",
    "minority",
    "unique",
    "percentile_10",
    "percentile_90",
    "percentile_100",
];

/// The statistics of zones by strips (from memory and from disk), of whole numbers and decimals: those of the
/// map in memory; the zone over almost all of the map read run after run.
#[test]
fn zonal_by_strips() {
    let d = dir("zonal");
    let (ny, nx) = (330, 270);
    for (name, map, dtype) in
        [("classes", classes(ny, nx), DataType::I16), ("decimals", decimals(ny, nx), DataType::F64)]
    {
        let disk = write(&map, dtype, &d.join(format!("{name}.tif")));
        for all_touched in [false, true] {
            let p = serde_json::json!({"stats": STATS, "all_touched": all_touched});
            let memory =
                run("vector.zonal", vec![("vector", zones(ny, nx)), ("map", Value::Map(map.clone()))], p.clone());
            for rows in [8, 40, 400] {
                let mut q = p.clone();
                q["block_rows"] = rows.into();
                let blocks =
                    run("vector.zonal", vec![("vector", zones(ny, nx)), ("map", Value::Map(map.clone()))], q.clone());
                same(&table(&blocks, "table"), &table(&memory, "table"), &format!("{name} {rows} in memory"));
                let blocks = run("vector.zonal", vec![("vector", zones(ny, nx)), ("map", disk.clone())], q);
                same(&table(&blocks, "table"), &table(&memory, "table"), &format!("{name} {rows} on disk"));
            }
        }
    }
    std::fs::remove_dir_all(&d).ok();
}

/// A zone of more than 2^20 cells, whose percentiles lsio selects from the raster: the same read run after run.
#[test]
fn zonal_of_a_large_zone() {
    let d = dir("large");
    let (ny, nx) = (1100, 1000);
    let map = decimals(ny, nx);
    let disk = write(&map, DataType::F64, &d.join("large.tif"));
    let p = serde_json::json!({"stats": ["count", "mean", "std", "median", "percentile_25", "percentile_99.5"]});
    let memory = run("vector.zonal", vec![("vector", zones(ny, nx)), ("map", Value::Map(map.clone()))], p.clone());
    let mut q = p.clone();
    q["block_rows"] = 64.into();
    let blocks = run("vector.zonal", vec![("vector", zones(ny, nx)), ("map", disk)], q);
    same(&table(&blocks, "table"), &table(&memory, "table"), "large zone");
    std::fs::remove_dir_all(&d).ok();
}

fn points(ny: usize, nx: usize) -> Value {
    let mut geometries: Vec<Geometry> = (0..400)
        .map(|k| {
            let x = 990.0 + draw(5, k, 0) * (nx as f64 * 10.0 + 20.0);
            let y = 5010.0 - draw(6, k, 0) * (ny as f64 * 10.0 + 20.0);
            Geometry::Point([x, y])
        })
        .collect();
    // on the cells' edges and centres, and the map's corners
    geometries.extend(
        [[1000.0, 5000.0], [1010.0, 4990.0], [1005.0, 4995.0], [1000.0 + nx as f64 * 10.0, 4000.0]]
            .map(Geometry::Point),
    );
    geometries.push(Geometry::Empty);
    let n = geometries.len();
    Value::Vector(Arc::new(Vector {
        geometries,
        attributes: vec![("id".into(), Column::Numbers((1..=n).map(|i| i as f64).collect()))],
        crs: None,
    }))
}

/// The values at points (nearest, bilinear, and the statistics of buffers) by strips: those of the map in memory.
#[test]
fn extract_by_strips() {
    let d = dir("extract");
    let (ny, nx) = (330, 270);
    let map = decimals(ny, nx);
    let disk = write(&map, DataType::F64, &d.join("decimals.tif"));
    for p in [
        serde_json::json!({}),
        serde_json::json!({"method": "bilinear"}),
        serde_json::json!({"buffer": 75, "stats": ["mean", "max", "median"]}),
    ] {
        let memory =
            run("vector.extract", vec![("vector", points(ny, nx)), ("map", Value::Map(map.clone()))], p.clone());
        for rows in [1, 8, 64] {
            let mut q = p.clone();
            q["block_rows"] = rows.into();
            let blocks = run("vector.extract", vec![("vector", points(ny, nx)), ("map", disk.clone())], q);
            same(&table(&blocks, "table"), &table(&memory, "table"), &format!("{p} {rows}"));
        }
    }
    std::fs::remove_dir_all(&d).ok();
}

/// Polygons, lines and points burnt by strips into a GeoTIFF: the cells of the map burnt in memory.
#[test]
fn rasterize_by_strips() {
    let d = dir("rasterize");
    let (ny, nx) = (330, 270);
    let like = classes(ny, nx);
    let disk = write(&like, DataType::I16, &d.join("like.tif"));
    let Value::Vector(z) = zones(ny, nx) else { panic!() };
    let Value::Vector(p) = points(ny, nx) else { panic!() };
    let mut v = (*z).clone();
    v.geometries.extend(p.geometries.iter().take(60).cloned());
    v.geometries.push(Geometry::LineString(vec![[1001.0, 4999.0], [3500.0, 2100.0], [1200.0, 1900.0]]));
    v.geometries.push(Geometry::LineString(vec![[1000.0, 4000.0], [3700.0, 4000.0]]));
    let n = v.geometries.len();
    v.attributes = vec![("w".into(), Column::Numbers((1..=n).map(|i| i as f64 * 0.5).collect()))];
    // all touched, without the lines: GDAL draws a line through cells' corners by where the strip starts
    let without_lines = v.select(&(0..n - 2).collect::<Vec<_>>());
    let (v, without_lines) = (Value::Vector(Arc::new(v)), Value::Vector(Arc::new(without_lines)));
    for (p, v) in [
        (serde_json::json!({"field": "w"}), &v),
        (serde_json::json!({"field": "w", "merge": "add"}), &v),
        (serde_json::json!({"field": "w", "merge": "add", "all_touched": true}), &without_lines),
        (serde_json::json!({"value": 3, "fill": null}), &v),
    ] {
        let Value::Map(memory) =
            &run("vector.rasterize", vec![("vector", v.clone()), ("like", Value::Map(like.clone()))], p.clone())["map"]
        else {
            panic!()
        };
        for rows in [64, 128] {
            let path = d.join(format!("burnt{rows}.tif"));
            let mut q = p.clone();
            q["block_rows"] = rows.into();
            q["path"] = path.to_string_lossy().into();
            let out = run("vector.rasterize", vec![("vector", v.clone()), ("like", disk.clone())], q);
            assert!(matches!(out["map"], Value::Raster(_)));
            let back = Raster::read(&path).unwrap();
            same_bits(&back.bands[0].masked(back.nodata), &memory.data, &format!("{p} {rows}"));
        }
    }
    std::fs::remove_dir_all(&d).ok();
}
