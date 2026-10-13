//! Habitat-graph nodes and distances from a raster of patches (landschaft's node_table and distance_table, as
//! Makurhini's nodesfile and distancefile): the patches' areas (or an attribute), the distances between their
//! centroids or edges, least-cost distances over a resistance (gdistance's) and commute times, with the
//! Euclidean distance for the pairs a resistance does not join.

use std::sync::Arc;

use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::Map;

pub const AREA_UNITS: [(&str, f64); 8] = [
    ("m2", 1.0),
    ("km2", 1e-06),
    ("ha", 0.0001),
    ("Dam2", 0.01),
    ("inch2", 1550.0),
    ("foot2", 10.7639),
    ("yard2", 1.19599),
    ("mile2", 3.861e-07),
];
pub const DISTANCE_UNITS: [(&str, f64); 6] =
    [("m", 1.0), ("km", 0.001), ("inch", 39.3701), ("foot", 3.28084), ("yard", 1.09361), ("mile", 0.000621371)];
/// The memory the threads of the least-cost search may take (landschaft's).
const COST_MEMORY: f64 = 8e9;

pub fn factor(table: &[(&str, f64)], unit: &str, what: &str) -> Result<f64, String> {
    table.iter().find(|(u, _)| *u == unit).map(|(_, f)| *f).ok_or_else(|| {
        format!(
            "unknown {what} unit {unit:?}; choose from {}",
            table.iter().map(|(u, _)| *u).collect::<Vec<_>>().join(", ")
        )
    })
}

/// The extent of a grid: xmin, xmax, ymin, ymax.
pub fn extent(g: &Grid) -> [f64; 4] {
    let t = g.transform;
    let (xmin, ymax) = (t[0], t[3]);
    [xmin, xmin + g.width as f64 * t[1], ymax + g.height as f64 * t[5], ymax]
}

/// A raster of patch ids: the ids, the missing cells (empty: none), its grid.
pub struct Patches {
    pub values: Vec<i32>,
    pub na: Vec<u8>,
    pub grid: Arc<Grid>,
}

/// Every cell's node (-1 none) on the patches' grid: what a graph from a raster maps its values back to.
#[derive(Debug)]
pub struct NodeRaster {
    pub grid: Arc<Grid>,
    pub node: Vec<i32>,
}

/// The patches of a map: its values as patch ids, or with `habitat` the patches (8 or 4 neighbours) of the
/// cells of those classes.
pub fn read_patches(map: &Map, habitat: Option<&[f64]>, directions: i32) -> Result<Patches, String> {
    let (values, na) = crate::metrics::classes_of(&map.data).map_err(|e| e.to_string())?;
    let Some(habitat) = habitat else { return Ok(Patches { values, na, grid: map.grid.clone() }) };
    let keep: Vec<bool> = values
        .iter()
        .enumerate()
        .map(|(k, &v)| habitat.contains(&f64::from(v)) && (na.is_empty() || na[k] == 0))
        .collect();
    if !keep.contains(&true) {
        return Err("no cell of the habitat classes".into());
    }
    let (ny, nx) = map.shape();
    let t = map.grid.transform;
    let mut options = crate::metrics::default_options();
    options.directions = directions;
    let ones: Vec<i32> = keep.iter().map(|&k| i32::from(k)).collect();
    let mask: Vec<u8> = keep.iter().map(|&k| u8::from(!k)).collect();
    let l = ffi::lsm_table(&ones, &mask, ny as i64, nx as i64, t[1].abs(), t[5].abs(), &options, &[], &[], true)
        .map_err(|e| e.to_string())?;
    Ok(Patches { values: l.labels.to_vec(), na: mask, grid: map.grid.clone() })
}

impl Patches {
    pub fn nodes(&self, centroids: bool, keep_index: bool) -> Result<ffi::ConnRasterNodes, String> {
        let g = &self.grid;
        ffi::conn_raster_nodes(
            &self.values,
            &self.na,
            g.height as i64,
            g.width as i64,
            extent(g),
            centroids,
            keep_index,
        )
        .map_err(|e| e.to_string())
    }

    pub fn node_raster(&self) -> Result<NodeRaster, String> {
        Ok(NodeRaster { grid: self.grid.clone(), node: self.nodes(false, true)?.node.to_vec() })
    }

    /// The ids and attributes of the nodes: their area (cells times the cell's area in the unit), the
    /// attributes given, or both multiplied (`weighted`).
    pub fn node_table(
        &self,
        attribute: Option<&[f64]>,
        weighted: bool,
        area_unit: &str,
    ) -> Result<(Vec<f64>, Vec<f64>), String> {
        let r = self.nodes(false, false)?;
        let e = extent(&self.grid);
        let res_x = (e[1] - e[0]) / self.grid.width as f64;
        let nres = res_x * res_x * factor(&AREA_UNITS, area_unit, "area")?;
        let ids: Vec<f64> = r.ids.iter().map(|&i| f64::from(i)).collect();
        let values = match attribute {
            None => r.cells.iter().map(|&c| c as f64 * nres).collect(),
            Some(a) => {
                if a.len() != ids.len() {
                    return Err(format!("attribute must have one value per patch ({})", ids.len()));
                }
                if weighted { r.cells.iter().zip(a).map(|(&c, x)| c as f64 * nres * x).collect() } else { a.to_vec() }
            }
        };
        Ok((ids, values))
    }
}

/// How to measure the distances between nodes.
pub struct DistanceOptions<'a> {
    /// centroid, edge, least-cost or commute-time.
    pub distance: &'a str,
    pub max_distance: Option<f64>,
    pub distance_unit: &'a str,
    pub resistance: Option<&'a Map>,
    pub cost_directions: i32,
    /// makurhini (1 / the destination's resistance) or mean.
    pub cost_function: &'a str,
    /// euclidean (the straight distance for pairs a resistance does not join) or drop.
    pub unreachable: &'a str,
}

/// The pairs' table: from, to, distance.
pub type PairTable = (Vec<f64>, Vec<f64>, Vec<f64>);

pub fn distance_table(p: &Patches, o: &DistanceOptions) -> Result<PairTable, String> {
    if !["centroid", "edge", "least-cost", "commute-time"].contains(&o.distance) {
        return Err(format!("distance must be centroid, edge, least-cost or commute-time, got {:?}", o.distance));
    }
    if o.distance == "least-cost" || o.distance == "commute-time" {
        return least_cost_table(p, o);
    }
    let f = factor(&DISTANCE_UNITS, o.distance_unit, "distance")?;
    let limit = o.max_distance.map_or(f64::INFINITY, |m| m / f * (1.0 + 1e-12));
    let nd = p.nodes(o.distance == "centroid", o.distance == "edge")?;
    let r = if o.distance == "centroid" {
        ffi::conn_point_distances(&nd.cx, &nd.cy, limit)
    } else {
        let e = extent(&p.grid);
        let (ny, nx) = (p.grid.height as i64, p.grid.width as i64);
        ffi::conn_raster_edge_distances(
            &nd.node,
            nd.ids.len() as i32,
            ny,
            nx,
            (e[1] - e[0]) / nx as f64,
            (e[3] - e[2]) / ny as f64,
            limit,
        )
    }
    .map_err(|e| e.to_string())?;
    let mut out: PairTable = (Vec::new(), Vec::new(), Vec::new());
    for k in 0..r.i.len() {
        let d = r.distance[k] * f;
        if o.max_distance.is_some_and(|m| d.is_nan() || d > m) {
            continue;
        }
        out.0.push(f64::from(nd.ids[r.i[k] as usize]));
        out.1.push(f64::from(nd.ids[r.j[k] as usize]));
        out.2.push(d);
    }
    Ok(out)
}

/// The cell of each point (-1 outside), as landschaft's cells_of.
fn cells_of(x: &[f64], y: &[f64], ny: usize, nx: usize, e: [f64; 4]) -> Vec<i64> {
    let [xmin, xmax, ymin, ymax] = e;
    let yres_inv = ny as f64 / (ymax - ymin);
    let xres_inv = nx as f64 / (xmax - xmin);
    x.iter()
        .zip(y)
        .map(|(&x, &y)| {
            let row = if y == ymin { (ny - 1) as f64 } else { ((ymax - y) * yres_inv).floor() };
            let col = if x == xmax { (nx - 1) as f64 } else { ((x - xmin) * xres_inv).floor() };
            if row.is_nan() || col.is_nan() || row < 0.0 || row >= ny as f64 || col < 0.0 || col >= nx as f64 {
                -1
            } else {
                (row * nx as f64 + col) as i64
            }
        })
        .collect()
}

/// scipy's ndimage.label of the cells with a finite value (4 or 8 neighbours): 0 none, else a component.
fn components(v: &[f64], ny: usize, nx: usize, directions: i32) -> Vec<i64> {
    let mut label = vec![0i64; v.len()];
    let mut next = 0;
    for s in 0..v.len() {
        if label[s] != 0 || !v[s].is_finite() {
            continue;
        }
        next += 1;
        label[s] = next;
        let mut stack = vec![s];
        while let Some(c) = stack.pop() {
            let (r, col) = ((c / nx) as i64, (c % nx) as i64);
            for dr in -1..=1i64 {
                for dc in -1..=1i64 {
                    if (dr == 0 && dc == 0) || (directions == 4 && dr != 0 && dc != 0) {
                        continue;
                    }
                    let (r2, c2) = (r + dr, col + dc);
                    if r2 < 0 || c2 < 0 || r2 >= ny as i64 || c2 >= nx as i64 {
                        continue;
                    }
                    let k = (r2 * nx as i64 + c2) as usize;
                    if label[k] == 0 && v[k].is_finite() {
                        label[k] = next;
                        stack.push(k);
                    }
                }
            }
        }
    }
    label
}

fn least_cost_table(p: &Patches, o: &DistanceOptions) -> Result<PairTable, String> {
    let Some(res) = o.resistance else { return Err(format!("{} distances need a resistance raster", o.distance)) };
    let function = match o.cost_function {
        "makurhini" => 0,
        "mean" => 1,
        f => return Err(format!("cost_function must be makurhini or mean, got {f:?}")),
    };
    if o.cost_directions != 4 && o.cost_directions != 8 {
        return Err("cost_directions must be 4 or 8".into());
    }
    if o.unreachable != "euclidean" && o.unreachable != "drop" {
        return Err("unreachable must be euclidean or drop".into());
    }
    let nd = p.nodes(true, false)?;
    let ids: Vec<f64> = nd.ids.iter().map(|&i| f64::from(i)).collect();
    let n = ids.len();
    let values = &res.data[..];
    let finite: Vec<f64> = values.iter().copied().filter(|v| v.is_finite()).collect();
    if finite.iter().any(|&v| v <= 0.0) {
        return Err("resistances must be positive".into());
    }
    let (ny, nx) = res.shape();
    let e = extent(&res.grid);
    let cells = cells_of(&nd.cx, &nd.cy, ny, nx, e);
    let limit = o.max_distance.unwrap_or(f64::INFINITY);
    let mut table: Vec<(i64, i64, f64)> = Vec::new();
    let mut on_graph = vec![true; n];
    if o.distance == "commute-time" {
        let core = [ny as f64, nx as f64, e[0], e[1], e[2], e[3], 0.0];
        let f = if function == 0 { "to" } else { "mean" };
        let mut v = ffi::cost_builtin(values, &core, o.cost_directions, true, f).map_err(|e| e.to_string())?;
        for x in v.iter_mut() {
            if *x != 0.0 {
                *x = 1.0 / *x;
            }
        }
        let v = ffi::cost_geo_correction(&v, &core, o.cost_directions, true, false, "c", false, false, false)
            .map_err(|e| e.to_string())?;
        let c = ffi::cost_commute(&v, &core, o.cost_directions, true, &cells).map_err(|e| e.to_string())?;
        for (k, g) in on_graph.iter_mut().enumerate() {
            *g = c.resistance[k * n + k] == 0.0;
        }
        for i in 0..n {
            for j in i + 1..n {
                let d = c.resistance[i * n + j] * c.volume;
                if d.is_finite() && d <= limit {
                    table.push((i as i64, j as i64, d));
                }
            }
        }
    } else {
        let cap = 1.max((COST_MEMORY / (9.0 * values.len() as f64 + 1.0)).floor() as i64) as i32;
        let r = ffi::conn_least_cost(values, ny as i64, nx as i64, e, &cells, o.cost_directions, function, limit, cap)
            .map_err(|e| e.to_string())?;
        table = (0..r.i.len()).map(|k| (r.i[k] as i64, r.j[k] as i64, r.distance[k])).collect();
    }
    if o.unreachable == "euclidean" {
        let labels = components(values, ny, nx, o.cost_directions);
        let component: Vec<i64> = cells.iter().map(|&c| if c >= 0 { labels[c as usize] } else { 0 }).collect();
        let ed = ffi::conn_point_distances(&nd.cx, &nd.cy, limit).map_err(|e| e.to_string())?;
        let found: std::collections::HashSet<(i64, i64)> = table.iter().map(|t| (t.0, t.1)).collect();
        let mut added = false;
        for k in 0..ed.i.len() {
            let (ei, ej) = (ed.i[k] as usize, ed.j[k] as usize);
            let mut cut_off = component[ei] == 0 || component[ei] != component[ej];
            if o.distance == "commute-time" {
                cut_off &= on_graph[ei] && on_graph[ej];
            }
            if cut_off && !found.contains(&(ei as i64, ej as i64)) {
                table.push((ei as i64, ej as i64, ed.distance[k]));
                added = true;
            }
        }
        if added {
            table.sort_by_key(|t| (t.0, t.1));
        }
    }
    let mut out: PairTable = (Vec::new(), Vec::new(), Vec::new());
    for (i, j, d) in table {
        if o.max_distance.is_some_and(|m| d.is_nan() || d > m) {
            continue;
        }
        out.0.push(ids[i as usize]);
        out.1.push(ids[j as usize]);
        out.2.push(d);
    }
    Ok(out)
}
