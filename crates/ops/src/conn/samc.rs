//! Settlement probabilities between habitat nodes from a spatial absorbing Markov chain (the samc package's
//! model, as Makurhini's MK_dPCIIC with samc and landschaft's settlement_probabilities): a random walk over a
//! resistance that is absorbed at each cell with a probability (none inside the nodes, else from the dispersal
//! distance); the probability that a walker leaving one node settles in another, summed or averaged over the
//! cells of their boundary vertices. The chain's steps are landschaft's C++; its inverse is Eigen's sparse LU
//! (scipy's SuperLU in landschaft: the same to rounding).

use terraflux_ls_sys::ffi;
use terraflux_raster::vector::Geometry;

use super::raster::extent;
use crate::Map;
use crate::change::numpy_sum;

/// The coordinates of a geometry in shapely's order (exterior rings, then holes, part after part).
fn coordinates(g: &Geometry) -> Vec<[f64; 2]> {
    match g {
        Geometry::Empty => Vec::new(),
        Geometry::Point(p) => vec![*p],
        Geometry::LineString(l) | Geometry::MultiPoint(l) => l.clone(),
        Geometry::Polygon(rs) | Geometry::MultiLineString(rs) => rs.iter().flatten().copied().collect(),
        Geometry::MultiPolygon(ps) => ps.iter().flatten().flatten().copied().collect(),
    }
}

/// samc's boundary vertices of a node: n of them (or a fraction), evenly spaced along its coordinates (R's
/// round(seq(1, count, length.out = n))), the closing one and repeats left out.
pub fn boundary_vertices(g: &Geometry, n: Option<i64>, fraction: Option<f64>) -> Result<Vec<[f64; 2]>, String> {
    let mut xy = coordinates(g);
    if xy.len() > 1 && xy[0] == xy[xy.len() - 1] {
        xy.pop();
        let mut seen: Vec<[f64; 2]> = Vec::new();
        for p in xy {
            if !seen.iter().any(|q| q[0] == p[0] && q[1] == p[1]) {
                seen.push(p);
            }
        }
        xy = seen;
    }
    let count = xy.len() as i64;
    let n = match fraction {
        Some(f) => (count as f64 * f).round_ties_even() as i64,
        None => n.ok_or("give the number of vertices or their fraction")?,
    };
    if n >= count {
        return Ok(xy);
    }
    let seq: Vec<f64> = if n > 2 {
        let by = (count - 1) as f64 / (n - 1) as f64;
        std::iter::once(1.0)
            .chain((1..n - 1).map(|k| 1.0 + k as f64 * by))
            .chain(std::iter::once(count as f64))
            .collect()
    } else {
        [1.0, count as f64][..n.max(0) as usize].to_vec()
    };
    let mut idx: Vec<i64> = seq.iter().map(|v| v.round_ties_even() as i64 - 1).collect();
    idx.sort_unstable();
    idx.dedup();
    Ok(idx.iter().map(|&k| xy[k as usize]).collect())
}

/// The cell of each point (-1 outside) on a grid of extent xmin, xmax, ymin, ymax.
fn cells_of(xy: &[[f64; 2]], ny: usize, nx: usize, e: [f64; 4]) -> Vec<i64> {
    let [xmin, xmax, ymin, ymax] = e;
    let yres_inv = ny as f64 / (ymax - ymin);
    let xres_inv = nx as f64 / (xmax - xmin);
    xy.iter()
        .map(|&[x, y]| {
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

/// The settings of the chain.
pub struct Samc<'a> {
    /// The absorption of every cell (else from the dispersal distance).
    pub absorption: Option<&'a Map>,
    pub dispersal_distance: Option<f64>,
    pub directions: i32,
    /// makurhini (1 / the destination's resistance) or mean.
    pub cost_function: &'a str,
    pub vertices: Option<i64>,
    pub vertex_fraction: Option<f64>,
    /// sum or mean over the cells of two nodes.
    pub settle: &'a str,
}

/// landschaft's settlement_probabilities: the n x n matrix (row-major) of the probabilities of settling,
/// 1 on the diagonal.
pub fn settlement_probabilities(nodes: &[Geometry], resistance: &Map, s: &Samc) -> Result<Vec<f64>, String> {
    if s.settle != "sum" && s.settle != "mean" {
        return Err(format!("settle must be sum or mean, got {:?}", s.settle));
    }
    if s.directions != 4 && s.directions != 8 {
        return Err("directions must be 4 or 8".into());
    }
    let function = match s.cost_function {
        "makurhini" => 0,
        "mean" => 1,
        f => return Err(format!("cost_function must be makurhini or mean, got {f:?}")),
    };
    let n = nodes.len();
    let res = &resistance.data[..];
    if res.iter().any(|v| v.is_finite() && *v <= 0.0) {
        return Err("resistances must be positive".into());
    }
    let (ny, nx) = resistance.shape();
    let e = extent(&resistance.grid);
    let absorb: Vec<f64> = match s.absorption {
        Some(a) => {
            if a.shape() != resistance.shape() {
                return Err("the absorption raster must have the resistance's shape".into());
            }
            a.data.to_vec()
        }
        None => {
            let dd = s.dispersal_distance.ok_or("give the dispersal distance (or an absorption raster)")?;
            let mut inside = vec![0.0; ny * nx];
            let transform = [e[0], (e[1] - e[0]) / nx as f64, 0.0, e[3], 0.0, -(e[3] - e[2]) / ny as f64];
            terraflux_gdal::rasterize(&mut inside, nx, ny, transform, nodes, &vec![1.0; n], false, false)?;
            let rate = (-1.247 - 1.788 * (dd / ((e[1] - e[0]) / nx as f64)).ln()).exp();
            inside.iter().map(|&v| if v != 0.0 { 0.0 } else { rate }).collect()
        }
    };
    let finite: Vec<usize> = (0..absorb.len()).filter(|&k| absorb[k].is_finite() && res[k].is_finite()).collect();
    if finite.iter().any(|&k| absorb[k] < 0.0 || absorb[k] > 1.0) {
        return Err("absorption values must be between 0 and 1".into());
    }
    if !finite.iter().any(|&k| absorb[k] > 0.0) {
        return Err("at least one cell must absorb".into());
    }
    let t = ffi::conn_samc_transitions(res, &absorb, ny as i64, nx as i64, s.directions, function)
        .map_err(|e| e.to_string())?;
    let m = t.cell.len();
    let mut state_of = vec![-1i64; ny * nx];
    for (k, &c) in t.cell.iter().enumerate() {
        state_of[c as usize] = k as i64;
    }
    let mut own: Vec<Vec<i64>> = Vec::with_capacity(n);
    for g in nodes {
        let xy = boundary_vertices(g, s.vertices, s.vertex_fraction)?;
        let mut states: Vec<i64> = Vec::new();
        for c in cells_of(&xy, ny, nx, e) {
            let st = if c >= 0 { state_of[c as usize] } else { -1 };
            if st >= 0 && !states.contains(&st) {
                states.push(st);
            }
        }
        own.push(states);
    }
    let mut p = vec![0.0; n * n];
    (0..n).for_each(|k| p[k * n + k] = 1.0);
    let mut targets: Vec<i64> = own.iter().flatten().copied().collect();
    targets.sort_unstable();
    targets.dedup();
    if targets.is_empty() {
        return Ok(p);
    }
    let k = targets.len();
    let f = ffi::samc_inverse_block(m as i64, &t.from, &t.to, &t.q, &targets).map_err(|e| e.to_string())?;
    let scale: Vec<f64> = targets.iter().map(|&s| absorb[t.cell[s as usize] as usize]).collect();
    let where_: Vec<Vec<usize>> =
        own.iter().map(|o| o.iter().map(|s| targets.binary_search(s).unwrap()).collect()).collect();
    for y in 0..n {
        if where_[y].is_empty() {
            continue;
        }
        for x in 0..y {
            if where_[x].is_empty() {
                continue;
            }
            let (f, scale) = (&f, &scale);
            let block: Vec<f64> =
                where_[y].iter().flat_map(|&r| where_[x].iter().map(move |&c| f[r * k + c] * scale[c])).collect();
            let sum = numpy_sum(&block);
            let v = if s.settle == "sum" { sum } else { sum / block.len() as f64 };
            p[x * n + y] = v;
            p[y * n + x] = v;
        }
    }
    Ok(p)
}
