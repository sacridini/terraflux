//! Legends: given by raster.legend, carried with the map to what keeps its classes and to the GeoTIFF it is
//! written to; a reclassification leaves them behind.

use std::sync::Arc;

use terraflux_ops::legend::{ITEM, of_grid};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::Grid;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Result<Outputs, String> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).map_err(|e| e.to_string())
}

#[test]
fn legends_go_with_their_maps() {
    let mut grid = Grid::new(4, 2);
    grid.transform = [0.0, 10.0, 0.0, 20.0, 0.0, -10.0];
    let map = Map::new(Arc::new(grid), vec![1.0, 1.0, 2.0, 2.0, 3.0, 3.0, f64::NAN, 1.0]);
    let classes = serde_json::json!([[1, "forest", "#1a7f37"], [2, "pasture", "#e0c341"], [3, "", ""]]);
    let out =
        run("raster.legend", vec![("map", Value::Map(map.clone()))], serde_json::json!({"classes": classes})).unwrap();
    let Value::Map(m) = &out["map"] else { panic!() };
    assert!(m.data.iter().zip(map.data.iter()).all(|(a, b)| a.to_bits() == b.to_bits()));
    let l = of_grid(&m.grid).expect("the legend carried");
    assert_eq!((l[0].name.as_str(), l[0].colour.as_str()), ("forest", "#1a7f37"));
    assert_eq!(l.len(), 3);
    // written to a GeoTIFF and read back
    let dir = std::env::temp_dir().join(format!("terraflux-legend-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("legend.tif");
    run("raster.write", vec![("value", out["map"].clone())], serde_json::json!({"path": path.to_string_lossy()}))
        .unwrap();
    let back = terraflux_raster::Raster::read(&path).unwrap();
    assert!(back.grid.geo.item(ITEM).is_some());
    assert_eq!(of_grid(&back.grid).unwrap(), l);
    // a reclassification: other classes, no legend
    let re =
        run("raster.reclassify", vec![("map", out["map"].clone())], serde_json::json!({"rules": "1 -> 9"})).unwrap();
    let Value::Map(rm) = &re["map"] else { panic!() };
    assert!(of_grid(&rm.grid).is_none());
    // a colour not #rrggbb refused
    let bad = serde_json::json!({"classes": [[1, "x", "green"]]});
    assert!(run("raster.legend", vec![("map", Value::Map(map))], bad).is_err());
    std::fs::remove_dir_all(&dir).ok();
}
