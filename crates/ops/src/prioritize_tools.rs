//! The matrices and rasters around prioritizr's problems (landschaft's prioritize/_milp_matrices.py,
//! _milp_integrate.py, _zones.py): the boundary, adjacency, connectivity and proximity matrices of planning units
//! (prioritizr's), connectivity from distances between units as a dispersal probability, a point per unit, a
//! problem's rasters on coarser blocks and a coarse selection back on the fine grid, a raster of zone numbers as a
//! 0/1 layer per zone, and a solution's selected cells.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::change::numpy_sum;
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(MatrixOp { kind: "boundary" }));
    r.add(Arc::new(MatrixOp { kind: "adjacency" }));
    r.add(Arc::new(MatrixOp { kind: "connectivity" }));
    r.add(Arc::new(MatrixOp { kind: "proximity" }));
    r.add(Arc::new(DecayConnectivity));
    r.add(Arc::new(UnitPoints));
    r.add(Arc::new(Coarsen));
    r.add(Arc::new(Refine));
    r.add(Arc::new(BinaryStack));
    r.add(Arc::new(ProtectedCells));
}

/// A sparse matrix of the units: n, and its entries (row, column, value).
struct Sparse {
    n: usize,
    entries: Vec<(usize, usize, f64)>,
}

impl Sparse {
    /// scipy's csc_matrix of the entries and their mirror, duplicates summed, sorted (by column, then row).
    fn symmetric(n: usize, pairs: Vec<(usize, usize, f64)>) -> Sparse {
        let mut all: Vec<(usize, usize, f64)> = pairs.iter().flat_map(|&(i, j, v)| [(i, j, v), (j, i, v)]).collect();
        Sparse { n, entries: Self::sorted(&mut all) }
    }

    /// The entries summed per (row, column) in the order given, sorted by column then row.
    fn sorted(all: &mut [(usize, usize, f64)]) -> Vec<(usize, usize, f64)> {
        // scipy's sum_duplicates: a stable sort by (column, row), then the duplicates added in that order
        let mut idx: Vec<usize> = (0..all.len()).collect();
        idx.sort_by_key(|&k| (all[k].1, all[k].0));
        let mut out: Vec<(usize, usize, f64)> = Vec::new();
        for k in idx {
            let (i, j, v) = all[k];
            match out.last_mut() {
                Some(last) if last.0 == i && last.1 == j => last.2 += v,
                _ => out.push((i, j, v)),
            }
        }
        out
    }

    fn table(&self) -> Table {
        let mut t = Table::new(vec!["i".into(), "j".into(), "x".into()]);
        t.rows = self.entries.iter().map(|&(i, j, x)| vec![(i + 1) as f64, (j + 1) as f64, x]).collect();
        t
    }
}

/// The planning units of a node: a map or a stack (its cells), or polygons.
enum Units {
    Raster { grid: Arc<Grid>, include: Vec<usize> },
    Polygons(Arc<Vector>),
}

fn units_of(args: &Args) -> Result<Units> {
    let layers: Vec<Arc<Vec<f64>>> = match args.get("units") {
        Some(Value::Vector(v)) => return Ok(Units::Polygons(v.clone())),
        Some(Value::Stack(s)) => {
            let n = s.grid.cells();
            let include = (0..n).filter(|&c| s.layers.iter().any(|l| l[c].is_finite())).collect();
            return Ok(Units::Raster { grid: s.grid.clone(), include });
        }
        Some(_) => vec![args.map("units")?.data.clone()],
        None => return Err(Error::msg(format!("node {:?}: connect the planning units", args.node))),
    };
    let m = args.map("units")?;
    let include = (0..m.grid.cells()).filter(|&c| layers[0][c].is_finite()).collect();
    Ok(Units::Raster { grid: m.grid.clone(), include })
}

/// terra's xres and yres: the extent over the cells.
pub(crate) fn res(g: &Grid) -> (f64, f64) {
    let t = g.transform;
    let (nx, ny) = (g.width as f64, g.height as f64);
    (((t[0] + nx * t[1]) - t[0]) / nx, (t[3] - (t[3] + ny * t[5])) / ny)
}

/// The pairs of included cells above and below, then left and right, of each other.
/// Pairs of cells.
type Pairs = Vec<(usize, usize)>;

fn pairs_4(grid: &Grid, include: &[usize]) -> (Pairs, Pairs) {
    let (ny, nx) = (grid.height, grid.width);
    let mut mask = vec![false; ny * nx];
    include.iter().for_each(|&c| mask[c] = true);
    let ud = (0..(ny.saturating_sub(1)) * nx).filter(|&c| mask[c] && mask[c + nx]).map(|c| (c, c + nx)).collect();
    let lr = (0..ny)
        .flat_map(|y| (0..nx.saturating_sub(1)).map(move |x| y * nx + x))
        .filter(|&c| mask[c] && mask[c + 1])
        .map(|c| (c, c + 1))
        .collect();
    (ud, lr)
}

fn raster_boundary(grid: &Grid, include: &[usize]) -> Sparse {
    let (xres, yres) = res(grid);
    let (ud, lr) = pairs_4(grid, include);
    let pairs = ud.iter().map(|&(a, b)| (a, b, xres)).chain(lr.iter().map(|&(a, b)| (a, b, yres))).collect();
    let mut m = Sparse::symmetric(grid.cells(), pairs);
    let mut all = std::mem::take(&mut m.entries);
    all.extend(include.iter().map(|&c| (c, c, (xres + yres) * 2.0)));
    m.entries = Sparse::sorted(&mut all);
    m
}

fn of_milp(m: crate::milp::Matrix) -> Sparse {
    let mut all: Vec<(usize, usize, f64)> = (0..m.i.len()).map(|k| (m.i[k], m.j[k], m.x[k])).collect();
    Sparse { n: m.n, entries: Sparse::sorted(&mut all) }
}

/// A point per unit: cell centres (every cell), or a point on the surface of every polygon.
fn points_of(u: &Units) -> Result<Vec<[f64; 2]>> {
    match u {
        Units::Raster { grid, .. } => {
            let t = grid.transform;
            Ok((0..grid.cells())
                .map(|k| {
                    let (r, c) = ((k / grid.width) as f64 + 0.5, (k % grid.width) as f64 + 0.5);
                    [t[0] + c * t[1] + r * t[2], t[3] + c * t[4] + r * t[5]]
                })
                .collect())
        }
        Units::Polygons(v) => {
            let geos = terraflux_gdal::Geos::new(&v.geometries).map_err(Error::msg)?;
            (0..v.len()).map(|i| geos.point_on_surface(i).map_err(Error::msg)).collect()
        }
    }
}

struct MatrixOp {
    kind: &'static str,
}

impl Operator for MatrixOp {
    fn signature(&self) -> OpSignature {
        let (doc, extra): (&str, Vec<PortSpec>) = match self.kind {
            "boundary" => (
                "The length of the boundary every two planning units share, and every unit's perimeter on the \
                 diagonal (prioritizr's boundary_matrix through landschaft): raster cells with data share xres \
                 with those above and below and yres with those left and right; polygons their common paths, \
                 rounded to 8 decimals.",
                vec![],
            ),
            "adjacency" => (
                "1 for every two adjacent planning units (prioritizr's adjacency_matrix through landschaft): \
                 cells with data sharing an edge (4 directions) or a corner too (8); polygons that meet.",
                vec![PortSpec::optional("directions", Type::Integer, "4 (default) or 8, for rasters")],
            ),
            "connectivity" => (
                "The connectivity of adjacent planning units (prioritizr's connectivity_matrix through \
                 landschaft): the length of their shared boundary times the mean of y in the two (a map on the \
                 units' grid; for polygons a map averaged over each, exactextract's, or an attribute).",
                vec![PortSpec::optional("column", Type::Text, "for polygons: the attribute of y")],
            ),
            _ => (
                "1 for every two planning units within a distance of each other (prioritizr's proximity_matrix \
                 through landschaft): cell centres, or polygons by their nearest points.",
                vec![PortSpec::new("distance", Type::Number, "the distance, in the units' coordinates")],
            ),
        };
        let name = match self.kind {
            "boundary" => "prioritize.boundary_matrix",
            "adjacency" => "prioritize.adjacency_matrix",
            "connectivity" => "prioritize.connectivity_matrix",
            _ => "prioritize.proximity_matrix",
        };
        let mut sig = OpSignature::new(name, doc, Access::Global)
            .input(PortSpec::new("units", Type::Any, "the planning units: a map or a stack (cells), or polygons"))
            .output(PortSpec::new("table", Type::Table, "i, j, x: the units (1..) and their value"));
        if self.kind == "connectivity" {
            sig = sig.input(PortSpec::optional("y", Type::Map, "the map of y"));
        }
        for p in extra {
            sig = sig.param(p);
        }
        sig
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let u = units_of(args)?;
        let m = match (self.kind, &u) {
            ("boundary", Units::Raster { grid, include }) => raster_boundary(grid, include),
            ("boundary", Units::Polygons(v)) => of_milp(crate::vector::boundary_matrix(&v.geometries)),
            ("adjacency", Units::Raster { grid, include }) => {
                let directions: i64 = args.param("directions", 4)?;
                if directions != 4 && directions != 8 {
                    return Err(fail("directions must be 4 or 8".into()));
                }
                let (ud, lr) = pairs_4(grid, include);
                let mut pairs: Vec<(usize, usize, f64)> = ud.into_iter().chain(lr).map(|(a, b)| (a, b, 1.0)).collect();
                if directions == 8 {
                    let (ny, nx) = (grid.height, grid.width);
                    let mut mask = vec![false; ny * nx];
                    include.iter().for_each(|&c| mask[c] = true);
                    for y in 0..ny.saturating_sub(1) {
                        for x in 0..nx.saturating_sub(1) {
                            let c = y * nx + x;
                            if mask[c] && mask[c + nx + 1] {
                                pairs.push((c, c + nx + 1, 1.0));
                            }
                        }
                    }
                    for y in 0..ny.saturating_sub(1) {
                        for x in 1..nx {
                            let c = y * nx + x;
                            if mask[c] && mask[c + nx - 1] {
                                pairs.push((c, c + nx - 1, 1.0));
                            }
                        }
                    }
                }
                let mut s = Sparse::symmetric(grid.cells(), pairs);
                s.entries.iter_mut().for_each(|e| e.2 = 1.0);
                s
            }
            ("adjacency", Units::Polygons(v)) => {
                let mut s = of_milp(crate::vector::adjacency_matrix(&v.geometries));
                s.entries.iter_mut().for_each(|e| e.2 = 1.0);
                s
            }
            ("connectivity", _) => {
                let bm = match &u {
                    Units::Raster { grid, include } => raster_boundary(grid, include),
                    Units::Polygons(v) => of_milp(crate::vector::boundary_matrix(&v.geometries)),
                };
                let n = bm.n;
                let y: Vec<f64> = match (&u, args.opt_param::<String>("column")?.filter(|c| !c.is_empty())) {
                    (Units::Polygons(v), Some(c)) => {
                        v.column(&c).ok_or_else(|| fail(format!("the polygons have no attribute {c:?}")))?.numbers()
                    }
                    (Units::Polygons(v), None) => {
                        let ym = args.map("y")?;
                        let count: Vec<f64> = ym.data.iter().map(|x| if x.is_finite() { 1.0 } else { 0.0 }).collect();
                        let geoms: Vec<&Geometry> = v.geometries.iter().collect();
                        let s = crate::vector::exact_sums(&ym.grid, &[ym.data.clone(), Arc::new(count)], &geoms)?;
                        (0..v.len())
                            .map(|f| {
                                let m = f64::from(s[2 * f] as f32 / s[2 * f + 1] as f32);
                                if m.abs() < 1e-10 { 0.0 } else { m }
                            })
                            .collect()
                    }
                    (Units::Raster { .. }, _) => {
                        let ym = args.map("y")?;
                        if ym.data.len() != n {
                            return Err(fail("y must be a map on the units' grid".into()));
                        }
                        ym.data.to_vec()
                    }
                };
                let pairs: Vec<(usize, usize, f64)> = bm
                    .entries
                    .iter()
                    .filter(|e| e.0 < e.1)
                    .map(|&(i, j, b)| (i, j, b * ((y[i] + y[j]) * 0.5)))
                    .filter(|e| e.2 > 0.0)
                    .collect();
                Sparse::symmetric(n, pairs)
            }
            (_, Units::Raster { grid, include }) => {
                let d: f64 = args.opt_param("distance")?.ok_or_else(|| fail("set the distance".into()))?;
                let pts = points_of(&u)?;
                let (xres, yres) = res(grid);
                let (ry, rx) = ((d / yres.abs()).ceil() as i64 + 1, (d / xres.abs()).ceil() as i64 + 1);
                let mut mask = vec![false; grid.cells()];
                include.iter().for_each(|&c| mask[c] = true);
                let nx = grid.width as i64;
                let ny = grid.height as i64;
                let mut pairs = Vec::new();
                for &a in include {
                    let (ya, xa) = (a as i64 / nx, a as i64 % nx);
                    for y in (ya - ry).max(0)..=(ya + ry).min(ny - 1) {
                        for x in (xa - rx).max(0)..=(xa + rx).min(nx - 1) {
                            let b = (y * nx + x) as usize;
                            if b <= a || !mask[b] {
                                continue;
                            }
                            let (dx, dy) = (pts[a][0] - pts[b][0], pts[a][1] - pts[b][1]);
                            if (dx * dx + dy * dy).sqrt() <= d {
                                pairs.push((a, b, 1.0));
                            }
                        }
                    }
                }
                Sparse::symmetric(grid.cells(), pairs)
            }
            (_, Units::Polygons(v)) => {
                let d: f64 = args.opt_param("distance")?.ok_or_else(|| fail("set the distance".into()))?;
                let geos = terraflux_gdal::Geos::new(&v.geometries).map_err(Error::msg)?;
                let mut pairs = Vec::new();
                for a in 0..v.len() {
                    for b in a + 1..v.len() {
                        if geos.distance(a, b) <= d {
                            pairs.push((a, b, 1.0));
                        }
                    }
                }
                Sparse::symmetric(v.len(), pairs)
            }
        };
        Ok(outputs([("table", Value::Table(m.table()))]))
    }
}

struct DecayConnectivity;

impl Operator for DecayConnectivity {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.decay_connectivity",
            "Connectivity from the distances between planning units as a dispersal probability (landschaft's \
             decay_connectivity, Conefor's negative exponential): exp(-ln 2 d / median), 0.5 at the median \
             dispersal distance, left out below a probability, 0 on the diagonal and for pairs out of reach. The \
             distances from cost.distance, cost.commute_distance or circuits between unit points.",
            Access::Global,
        )
        .input(PortSpec::new("distances", Type::Table, "a row per unit (its number first, or not), a column per unit"))
        .output(PortSpec::new("table", Type::Table, "i, j, x: the probabilities kept (connectivity_data)"))
        .param(PortSpec::new("median", Type::Number, "the median dispersal distance"))
        .param(PortSpec::optional("min_probability", Type::Number, "the least kept (default 0.001)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: &str| Error::msg(format!("node {:?}: {m}", args.node));
        let Some(Value::Table(t)) = args.get("distances") else { return Err(fail("connect the distances")) };
        let n = t.rows.len();
        let skip = match t.columns.len() {
            c if c == n => 0,
            c if c == n + 1 => 1,
            _ => return Err(fail("the distances must be a square matrix of units")),
        };
        let median: f64 = args.opt_param("median")?.ok_or_else(|| fail("set the median"))?;
        if median.is_nan() || median <= 0.0 {
            return Err(fail("median must be > 0"));
        }
        let min_p: f64 = args.param("min_probability", 0.001)?;
        let ln2 = 2f64.ln();
        let mut entries = Vec::new();
        for j in 0..n {
            for (i, row) in t.rows.iter().enumerate() {
                let d = row[skip + j];
                if i == j || !d.is_finite() {
                    continue;
                }
                let p = (-ln2 * d / median).exp();
                if p >= min_p && p != 0.0 {
                    entries.push((i, j, p));
                }
            }
        }
        Ok(outputs([("table", Value::Table(Sparse { n, entries }.table()))]))
    }
}

struct UnitPoints;

impl Operator for UnitPoints {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.unit_points",
            "A point per planning unit (landschaft's unit_points): the centre of every cell of a map, or a point \
             inside every polygon (GEOS's point on surface); to measure distances between units with cost.distance \
             or cost.commute_distance.",
            Access::Global,
        )
        .input(PortSpec::new("units", Type::Any, "the planning units: a map or a stack (cells), or polygons"))
        .output(PortSpec::new("points", Type::Vector, "a point per unit, with its number"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let u = units_of(args)?;
        let pts = points_of(&u)?;
        let crs = match &u {
            Units::Raster { grid, .. } => grid.geo.wkt.clone(),
            Units::Polygons(v) => v.crs.clone(),
        };
        let n = pts.len();
        let v = Vector {
            geometries: pts.into_iter().map(Geometry::Point).collect(),
            attributes: vec![("unit".into(), Column::Numbers((1..=n).map(|k| k as f64).collect()))],
            crs,
        };
        Ok(outputs([("points", Value::Vector(Arc::new(v)))]))
    }
}

/// The sums over blocks of f x f cells (numpy's nansum over the two axes of the blocks: each row of a block summed
/// pairwise, the rows added), NaN where a block has no value; on the grid of the blocks.
fn block_sum(v: &[f64], ny: usize, nx: usize, f: usize) -> Vec<f64> {
    let (by, bx) = (ny.div_ceil(f), nx.div_ceil(f));
    let mut out = vec![0.0; by * bx];
    let mut any = vec![false; by * bx];
    for b in 0..by * bx {
        let (r0, c0) = ((b / bx) * f, (b % bx) * f);
        for i in 0..f {
            let row: Vec<f64> = (0..f)
                .map(|j| {
                    let (y, x) = (r0 + i, c0 + j);
                    let val = if y < ny && x < nx { v[y * nx + x] } else { f64::NAN };
                    if val.is_finite() {
                        any[b] = true;
                    }
                    if val.is_nan() { 0.0 } else { val }
                })
                .collect();
            out[b] += numpy_sum(&row);
        }
    }
    out.iter().zip(any).map(|(&s, a)| if a { s } else { f64::NAN }).collect()
}

fn coarse_grid(g: &Grid, f: usize) -> Grid {
    let mut c = g.clone();
    c.width = g.width.div_ceil(f);
    c.height = g.height.div_ceil(f);
    let t = g.transform;
    c.transform = [t[0], t[1] * f as f64, t[2] * f as f64, t[3], t[4] * f as f64, t[5] * f as f64];
    c
}

struct Coarsen;

impl Operator for Coarsen {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.coarsen",
            "A problem's rasters on blocks of factor x factor cells (landschaft's coarsen, for continental \
             problems: solve the coarse problem, then prioritize.refine the selection): every block's cost and \
             amounts the sums of its cells with a cost (a block without any has none).",
            Access::Global,
        )
        .input(PortSpec::new("cost", Type::Map, "the cost of the planning units"))
        .input(PortSpec::new("features", Type::Any, "the features' amounts: a stack or a map"))
        .output(PortSpec::new("cost", Type::Map, "the cost of the blocks"))
        .output(PortSpec::new("features", Type::Stack, "the amounts of the blocks"))
        .param(PortSpec::new("factor", Type::Integer, "cells per side of a block"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: &str| Error::msg(format!("node {:?}: {m}", args.node));
        let c = args.map("cost")?;
        let f: usize = args.opt_param("factor")?.ok_or_else(|| fail("set the factor"))?;
        if f < 1 {
            return Err(fail("factor must be a whole number >= 1"));
        }
        let (keys, layers): (Vec<Key>, Vec<Arc<Vec<f64>>>) = match args.get("features") {
            Some(Value::Stack(s)) => (s.keys.clone(), s.layers.clone()),
            _ => (vec![Key::Name("feature".into())], vec![args.map("features")?.data.clone()]),
        };
        let (ny, nx) = (c.grid.height, c.grid.width);
        if layers.iter().any(|l| l.len() != ny * nx) {
            return Err(fail("the features and the cost differ in size"));
        }
        let grid = Arc::new(coarse_grid(&c.grid, f));
        let feats = layers
            .iter()
            .map(|l| {
                let v: Vec<f64> =
                    l.iter().zip(c.data.iter()).map(|(&x, &k)| if k.is_finite() { x } else { f64::NAN }).collect();
                Arc::new(block_sum(&v, ny, nx, f))
            })
            .collect();
        Ok(outputs([
            ("cost", Value::Map(Map::new(grid.clone(), block_sum(&c.data, ny, nx, f)))),
            ("features", Value::Stack(Stack { grid, keys, layers: feats })),
        ]))
    }
}

struct Refine;

impl Operator for Refine {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.refine",
            "A selection on coarse blocks (of a prioritize.coarsen-ed problem) back on the fine grid (landschaft's \
             refine): every cell its block's value, none where the fine map has none.",
            Access::Global,
        )
        .input(PortSpec::new("selection", Type::Map, "the coarse selection"))
        .input(PortSpec::new("like", Type::Map, "the fine cost map"))
        .output(PortSpec::new("map", Type::Map, "the selection on the fine grid"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (s, fine) = (args.map("selection")?, args.map("like")?);
        let (fy, fx, cy, cx) = (fine.grid.height, fine.grid.width, s.grid.height, s.grid.width);
        let (gy, gx) = (fy.div_ceil(cy), fx.div_ceil(cx));
        if cy * gy < fy || cx * gx < fx || (cy - 1) * gy >= fy || (cx - 1) * gx >= fx {
            return Err(Error::msg(format!(
                "node {:?}: the selection is not a coarsened version of this grid",
                args.node
            )));
        }
        let v = (0..fy * fx)
            .map(|k| {
                let (y, x) = (k / fx, k % fx);
                if fine.data[k].is_finite() { s.data[(y / gy) * cx + x / gx] } else { f64::NAN }
            })
            .collect();
        Ok(outputs([("map", Value::Map(Map::new(fine.grid.clone(), v)))]))
    }
}

struct BinaryStack;

impl Operator for BinaryStack {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.binary_stack",
            "A 0/1 layer per zone from a map of zone numbers (prioritizr's binary_stack through landschaft), for \
             the locked-in constraints of zones (an existing zoning plan); cells with 0 are in no zone, missing \
             ones stay missing.",
            Access::Global,
        )
        .input(PortSpec::new("zones", Type::Map, "zone numbers 1, 2... (0: none)"))
        .output(PortSpec::new("stack", Type::Stack, "a layer per zone, by name"))
        .param(PortSpec::optional(
            "names",
            Type::Any,
            "the zones' names in the order of their numbers (default 1, 2...)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("zones")?;
        let v = &m.data;
        let names: Vec<String> = match args.opt_param::<Vec<serde_json::Value>>("names")? {
            Some(n) => n.iter().map(|x| x.as_str().map(String::from).unwrap_or_else(|| x.to_string())).collect(),
            None => {
                let k = v.iter().filter(|x| x.is_finite()).fold(0.0f64, |a, &b| a.max(b)) as usize;
                (1..=k).map(|i| i.to_string()).collect()
            }
        };
        let bad = v.iter().any(|&x| x.is_finite() && x != 0.0 && (x < 1.0 || x > names.len() as f64 || x != x.round()));
        if bad {
            return Err(Error::msg(format!(
                "node {:?}: zone numbers must be whole numbers from 1 to {} (0: no zone)",
                args.node,
                names.len()
            )));
        }
        let layers = (0..names.len())
            .map(|k| {
                Arc::new(
                    v.iter()
                        .map(|&x| if x.is_finite() { if x == (k + 1) as f64 { 1.0 } else { 0.0 } } else { f64::NAN })
                        .collect(),
                )
            })
            .collect();
        Ok(outputs([(
            "stack",
            Value::Stack(Stack { grid: m.grid.clone(), keys: names.into_iter().map(Key::Name).collect(), layers }),
        )]))
    }
}

struct ProtectedCells;

impl Operator for ProtectedCells {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.protected_cells",
            "The selected cells of a solution on raster planning units (landschaft's protected_cells): 1 where \
             more than half selected, 0 elsewhere (missing too), for the restrictions of a land change \
             simulation.",
            Access::Global,
        )
        .input(PortSpec::new("solution", Type::Map, "a solution (prioritize.optimize's)"))
        .output(PortSpec::new("map", Type::Map, "1 protected, 0 not"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = args.map("solution")?;
        let v = s.data.iter().map(|&x| if !x.is_nan() && x > 0.5 { 1.0 } else { 0.0 }).collect();
        Ok(outputs([("map", Value::Map(Map::new(s.grid.clone(), v)))]))
    }
}
