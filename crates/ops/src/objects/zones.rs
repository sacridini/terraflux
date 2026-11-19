//! The zones of a thematic layer (`bounds`): a map, vectors or objects whose zones no object crosses. One number per
//! cell names its zone; cells in the same zone have the same number.

use terraflux_graph::{PortSpec, Type};
use terraflux_raster::Grid;

use super::Objects;
use crate::{Args, Error, Result, Value};

/// The optional input `bounds` of a segmenter: `native` when the segmenter keeps to the zones as it works, else the
/// objects it makes are cut where they cross a zone.
pub(super) fn port(native: bool) -> PortSpec {
    PortSpec::optional(
        "bounds",
        Type::Any,
        if native {
            "a thematic layer whose zones no object crosses (none is made across a zone): a map (a zone for each              value, the cells without a value one zone together), vectors (each feature a zone, the cells in none one              zone) or objects (each object a zone, id 0 one zone)"
        } else {
            "a thematic layer whose zones no object crosses (the objects made are cut where they cross a zone): a              map (a zone for each value, the cells without a value one zone together), vectors (each feature a zone,              the cells in none one zone) or objects (each object a zone, id 0 one zone)"
        },
    )
}

/// The objects cut by the zones of the input `bounds` (as they are without it).
pub(super) fn bounded(args: &Args, o: Objects, neighbours: usize) -> Result<Objects> {
    let zone = zones(args, "bounds", &o.grid)?;
    Ok(if zone.is_empty() { o } else { split_by_zones(&o, &zone, neighbours) })
}

/// The objects renumbered by the connected components (4 or 8 neighbours) of (object, zone): an object that crosses
/// zones becomes one object for each piece in a zone, ids in the order of the first cell as `Objects::from_regions`
/// numbers them, in parallel strips. The same objects, untouched, when none crosses a zone. The table, classes and
/// levels are not kept (the callers make their tables from the result).
pub(super) fn split_by_zones(o: &Objects, zone: &[u64], neighbours: usize) -> Objects {
    assert_eq!(zone.len(), o.grid.cells(), "the zones are not one a cell of the grid");
    let mut first = vec![u64::MAX; o.count];
    let mut crossed = false;
    for (&l, &z) in o.labels.iter().zip(zone) {
        if l != 0 {
            let f = &mut first[l as usize - 1];
            if *f == u64::MAX {
                *f = z;
            } else if *f != z {
                crossed = true;
                break;
            }
        }
    }
    if !crossed {
        return o.clone();
    }
    let labels = &o.labels;
    Objects::from_fn(
        o.grid.clone(),
        |i| if labels[i] == 0 { (0u32, 0u64) } else { (labels[i], zone[i]) },
        (0u32, 0u64),
        neighbours,
    )
}

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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;

    fn lcg(s: &mut u64) -> u64 {
        *s = s.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        *s >> 33
    }

    /// The connected components of (label, zone) by a flood fill, ids in the order of the first cell.
    fn reference(labels: &[u32], zone: &[u64], w: usize, h: usize, eight: bool) -> Vec<u32> {
        let mut out = vec![0u32; w * h];
        let mut next = 0;
        for start in 0..w * h {
            if labels[start] == 0 || out[start] != 0 {
                continue;
            }
            next += 1;
            let mut stack = vec![start];
            out[start] = next;
            while let Some(i) = stack.pop() {
                let (r, c) = ((i / w) as i64, (i % w) as i64);
                for dr in -1i64..=1 {
                    for dc in -1i64..=1 {
                        if (dr, dc) == (0, 0) || (!eight && dr != 0 && dc != 0) {
                            continue;
                        }
                        let (rr, cc) = (r + dr, c + dc);
                        if rr < 0 || cc < 0 || rr >= h as i64 || cc >= w as i64 {
                            continue;
                        }
                        let j = rr as usize * w + cc as usize;
                        if out[j] == 0 && labels[j] == labels[start] && zone[j] == zone[start] {
                            out[j] = next;
                            stack.push(j);
                        }
                    }
                }
            }
        }
        out
    }

    fn crossing_objects(w: usize, h: usize) -> (Objects, Vec<u64>) {
        let mut s = 5u64;
        // big blobs (cells of 7 x 5 blocks joined at random) that the zones cut
        let region: Vec<u64> = (0..w * h)
            .map(|i| {
                let (r, c) = (i / w, i % w);
                if lcg(&mut s) % 40 < 1 { NO_REGION_FOR_TEST } else { ((r / 7 + c / 5) % 3) as u64 }
            })
            .collect();
        let o = Objects::from_regions(Arc::new(Grid::new(w, h)), &region, 4);
        let zone: Vec<u64> = (0..w * h).map(|i| (((i % w) / 9 + ((i / w) / 11) * 7) % 4) as u64).collect();
        (o, zone)
    }

    const NO_REGION_FOR_TEST: u64 = super::super::NO_REGION;

    #[test]
    fn split_by_zones_makes_the_components_of_object_and_zone() {
        let (w, h) = (53, 41);
        let (o, zone) = crossing_objects(w, h);
        for eight in [false, true] {
            let got = split_by_zones(&o, &zone, if eight { 8 } else { 4 });
            let want = reference(&o.labels, &zone, w, h, eight);
            assert_eq!(got.labels[..], want[..], "{} neighbours", if eight { 8 } else { 4 });
        }
        // no object crosses a zone, and there are more objects than before
        let got = split_by_zones(&o, &zone, 4);
        assert!(got.count > o.count);
        let mut of = vec![u64::MAX; got.count];
        for (&l, &z) in got.labels.iter().zip(&zone) {
            if l != 0 {
                let k = l as usize - 1;
                assert!(of[k] == u64::MAX || of[k] == z);
                of[k] = z;
            }
        }
        // cells with no object stay with none
        assert!(o.labels.iter().zip(got.labels.iter()).all(|(&a, &b)| (a == 0) == (b == 0)));
    }

    #[test]
    fn split_by_zones_is_the_same_on_one_and_eight_threads_and_on_every_run() {
        let (o, zone) = crossing_objects(97, 83);
        let on = |n: usize| {
            let pool = rayon::ThreadPoolBuilder::new().num_threads(n).build().unwrap();
            pool.install(|| split_by_zones(&o, &zone, 4))
        };
        let (a, b, c) = (on(1), on(8), on(8));
        assert_eq!(a.labels[..], b.labels[..]);
        assert_eq!(b.labels[..], c.labels[..]);
        assert_eq!(a.count, b.count);
    }

    #[test]
    fn split_by_zones_leaves_objects_that_cross_nothing_as_they_are() {
        let (o, _) = crossing_objects(30, 20);
        // one zone, and zones that follow the objects
        let one = vec![3u64; 30 * 20];
        assert_eq!(split_by_zones(&o, &one, 4).labels[..], o.labels[..]);
        let own: Vec<u64> = o.labels.iter().map(|&l| l as u64).collect();
        assert_eq!(split_by_zones(&o, &own, 4).labels[..], o.labels[..]);
    }
}
