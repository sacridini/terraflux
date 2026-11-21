//! vector.rasterize on degenerate grids (no cells, a zero or non-finite cell size, a size beyond what a cell count
//! holds): an error naming the problem, never a crash. Needs no GDAL files (the polygon is built here).

use std::sync::Arc;

use terraflux_ops::{Args, Ctx, Map, Registry, Value};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Geometry, Vector};

/// A polygon, a line and a point (or only the kind TF_KIND names: poly, line, point).
fn polygon() -> Value {
    let ring = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]];
    let mut geometries = vec![
        Geometry::Polygon(vec![ring]),
        Geometry::LineString(vec![[0.0, 0.0], [4.0, 4.0]]),
        Geometry::Point([2.0, 2.0]),
    ];
    match std::env::var("TF_KIND").as_deref() {
        Ok("poly") => geometries.truncate(1),
        Ok("line") => geometries = geometries.split_off(1).drain(..1).collect(),
        Ok("point") => geometries = geometries.split_off(2),
        _ => {}
    }
    Value::Vector(Arc::new(Vector { geometries, attributes: vec![], crs: None }))
}

/// The error of burning the polygon into a grid (a map of `data` cells on it), or the burnt cells.
fn burn(
    width: usize,
    height: usize,
    transform: [f64; 6],
    data: Vec<f64>,
    params: serde_json::Value,
) -> Result<usize, String> {
    let mut grid = Grid::new(width, height);
    grid.transform = transform;
    let like = Value::Map(Map::new(Arc::new(grid), data));
    let args = Args {
        node: "n",
        inputs: [("vector".to_string(), polygon()), ("like".to_string(), like)].into_iter().collect(),
        params: params.as_object().unwrap(),
    };
    let out = Registry::standard()
        .get("vector.rasterize")
        .unwrap()
        .run(&mut Ctx::default(), &args)
        .map_err(|e| e.to_string())?;
    let Value::Map(m) = &out["map"] else { panic!("a map") };
    Ok(m.data.iter().filter(|&&v| v == 1.0).count())
}

const NORTH_UP: [f64; 6] = [0.0, 1.0, 0.0, 4.0, 0.0, -1.0];

/// One case by name, in this process (a crash kills it: run through the subprocess test below).
fn case(name: &str) -> Result<usize, String> {
    let p = serde_json::json!({});
    match name {
        "ok" => burn(4, 4, NORTH_UP, vec![0.0; 16], p),
        "width0" => burn(0, 4, NORTH_UP, vec![], p),
        "height0" => burn(4, 0, NORTH_UP, vec![], p),
        "both0" => burn(0, 0, NORTH_UP, vec![], p),
        "res0" => burn(4, 4, [0.0, 0.0, 0.0, 4.0, 0.0, -1.0], vec![0.0; 16], p),
        "resy0" => burn(4, 4, [0.0, 1.0, 0.0, 4.0, 0.0, 0.0], vec![0.0; 16], p),
        "nan" => burn(4, 4, [0.0, f64::NAN, 0.0, 4.0, 0.0, -1.0], vec![0.0; 16], p),
        "inf" => burn(4, 4, [0.0, f64::INFINITY, 0.0, 4.0, 0.0, -1.0], vec![0.0; 16], p),
        "origin_nan" => burn(4, 4, [f64::NAN, 1.0, 0.0, 4.0, 0.0, -1.0], vec![0.0; 16], p),
        "huge_width" => burn(3_000_000_000, 1, NORTH_UP, vec![0.0; 4], p),
        "huge_both" => burn(usize::MAX, 2, NORTH_UP, vec![0.0; 4], p),
        "strips0" => burn(0, 4, NORTH_UP, vec![], serde_json::json!({"block_rows": 2, "path": "unused"})),
        other => panic!("no case {other}"),
    }
}

/// The case named by TF_CASE (for the subprocess).
#[test]
fn one_case() {
    let Ok(name) = std::env::var("TF_CASE") else { return };
    match case(&name) {
        Ok(n) => println!("RESULT ok {n}"),
        Err(e) => println!("RESULT err {e}"),
    }
}

/// Every degenerate case in its own process: an error, not a crash.
#[test]
fn degenerate_grids_give_errors() {
    let exe = std::env::current_exe().unwrap();
    for kind in ["poly", "line", "point", "all"] {
        for name in [
            "width0",
            "height0",
            "both0",
            "res0",
            "resy0",
            "nan",
            "inf",
            "origin_nan",
            "huge_width",
            "huge_both",
            "strips0",
        ] {
            let out = std::process::Command::new(&exe)
                .args(["one_case", "--exact", "--nocapture", "--test-threads=1"])
                .env("TF_CASE", name)
                .env("TF_KIND", kind)
                .env("RAYON_NUM_THREADS", "4")
                .env("OMP_NUM_THREADS", "4")
                .output()
                .unwrap();
            let text = String::from_utf8_lossy(&out.stdout);
            let line = text.lines().find_map(|l| l.split_once("RESULT ").map(|x| x.1)).unwrap_or("");
            let who = format!("{kind} {name}");
            assert!(out.status.success() && line.starts_with("err "), "{who}: {:?} {line:?}", out.status);
            assert!(line.len() > 12, "{who}: the error says nothing: {line:?}");
        }
    }
}

/// A sound grid still burns.
#[test]
fn sound_grid_burns() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    assert_eq!(case("ok").unwrap(), 16);
}
