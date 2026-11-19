//! The zones of a thematic layer (`bounds`): a map, vectors or objects whose zones no object crosses. One number per
//! cell names its zone; cells in the same zone have the same number.

use terraflux_raster::Grid;

use crate::{Args, Error, Result, Value};

/// The bits of a number as a zone or a seed: the same for equal numbers (-0 and 0, any NaN).
pub(super) fn bits(v: f64) -> u64 {
    if v.is_nan() {
        0x7ff8_0000_0000_0000
    } else if v == 0.0 {
        0
    } else {
        v.to_bits()
    }
}

/// The zone of every cell for the input `port` (empty: the input is not given, no bounds), on `grid`:
/// - a map: one zone for each value, the cells without a value one zone together;
/// - vectors: each feature a zone (the later one winning where they overlap), the cells in no feature one zone;
/// - objects: each object a zone, the cells in none (id 0) one zone.
pub(super) fn zones(args: &Args, port: &str, grid: &Grid) -> Result<Vec<u64>> {
    match args.get(port) {
        None => Ok(Vec::new()),
        Some(v) => zones_of(v, grid, port).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node))),
    }
}

/// The zones of a value on `grid` (`what` names the input in the messages).
pub(crate) fn zones_of(v: &Value, grid: &Grid, what: &str) -> Result<Vec<u64>> {
    let bad = |m: String| Error::msg(format!("{what}: {m}"));
    match v {
        Value::Map(m) => {
            if !m.grid.same_as(grid) {
                return Err(bad("a map of another grid than the image".into()));
            }
            Ok(m.data.iter().map(|&v| bits(v)).collect())
        }
        Value::Objects(o) => {
            if !o.grid.same_as(grid) {
                return Err(bad("objects of another grid than the image".into()));
            }
            Ok(o.labels.iter().map(|&id| id as u64).collect())
        }
        Value::Vector(f) => {
            let f = crate::vector::on_grid(f, grid)?;
            // the feature (0..) of each cell, the later winning; NaN for none
            let mut burnt = vec![f64::NAN; grid.cells()];
            let values: Vec<f64> = (0..f.len()).map(|k| k as f64).collect();
            if !f.is_empty() {
                crate::vector::burn(&mut burnt, grid, &f, &values, false, false).map_err(|e| bad(e.to_string()))?;
            }
            Ok(burnt.iter().map(|&x| bits(x)).collect())
        }
        v => Err(bad(format!("a {:?}, not a map, vectors or objects", v.ty()))),
    }
}

/// The zones renumbered 0, 1, 2... in the order of first appearance (equal numbers stay equal), as the narrower
/// numbers a tile job carries.
pub(crate) fn compact(zone: &[u64]) -> Vec<u32> {
    let mut seen = std::collections::HashMap::new();
    zone.iter()
        .map(|&z| {
            let n = seen.len() as u32;
            *seen.entry(z).or_insert(n)
        })
        .collect()
}
