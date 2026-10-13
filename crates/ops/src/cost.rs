//! Distances over a grid, landschaft's port of the R package gdistance through its C++: transitions between
//! neighbouring cells (4, 8 or 16 directions, a function of the two cells' values), corrected for the cells'
//! distances (geoCorrection), and over them the accumulated cost from origins (accCost), the least costs between
//! points (costDistance) and the least-cost paths (shortestPath); random walks over them (commuteDistance,
//! passage, rSPDistance), and the matrix's own algebra (normalize, sumReciprocal, sums of transitions, R's
//! maths on the connections, the summary of each cell's).
//!
//! A transition is a stack of a layer per direction (the value of the connection from each cell in that
//! direction, 0 none; a symmetric one keeps the directions after (0, 0) only, as landschaft's `Transition`) whose
//! grid carries its layout in the metadata item `terraflux_transition`.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

/// The metadata item a transition's layout is kept in.
pub const ITEM: &str = "terraflux_transition";

pub fn register(r: &mut Registry) {
    r.add(Arc::new(TransitionOp));
    r.add(Arc::new(GeoCorrection));
    r.add(Arc::new(AccCost));
    r.add(Arc::new(CostDistance));
    r.add(Arc::new(ShortestPath));
    r.add(Arc::new(CommuteDistance));
    r.add(Arc::new(Passage));
    r.add(Arc::new(RspDistance));
    r.add(Arc::new(Normalize));
    r.add(Arc::new(SumReciprocal));
    r.add(Arc::new(Combine));
    r.add(Arc::new(TransitionMath));
    r.add(Arc::new(ToMap));
}

/// A transition's layout and grid (landschaft's GridInfo): what its stack's grid carries.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub directions: i32,
    pub symmetric: bool,
    /// "conductance" or "resistance".
    pub matrix_values: String,
    /// xmin, xmax, ymin, ymax.
    pub extent: [f64; 4],
    pub lonlat: bool,
    /// A global longitude/latitude grid: the columns wrap around.
    pub wrap: bool,
}

/// A transition: its values (directions x cells, row-major), layout and the grid of its map.
#[derive(Debug, Clone)]
pub struct Transition {
    pub values: Vec<f64>,
    pub layout: Layout,
    pub grid: Arc<Grid>,
}

impl Transition {
    pub fn cells(&self) -> usize {
        self.grid.width * self.grid.height
    }

    /// The grid as the C++ takes it: ny, nx, xmin, xmax, ymin, ymax, wrap.
    pub fn core(&self) -> [f64; 7] {
        let e = self.layout.extent;
        [self.grid.height as f64, self.grid.width as f64, e[0], e[1], e[2], e[3], f64::from(u8::from(self.layout.wrap))]
    }

    /// The directions' (row, column) offsets.
    pub fn offsets(&self) -> Result<Vec<(i32, i32)>> {
        offsets(self.layout.directions, self.layout.symmetric)
    }

    /// As a value: a stack of a layer per direction, its grid carrying the layout.
    pub fn value(self) -> Result<Value> {
        let offs = self.offsets()?;
        let n = self.cells();
        let mut g = (*self.grid).clone();
        g.geo.set_item(ITEM, Some(&serde_json::to_string(&self.layout).unwrap()));
        Ok(Value::Stack(Stack {
            grid: Arc::new(g),
            keys: offs.iter().map(|(r, c)| Key::Name(format!("{r},{c}"))).collect(),
            layers: self.values.chunks(n.max(1)).map(|c| Arc::new(c.to_vec())).collect(),
        }))
    }

    /// The transition of a node's port.
    pub fn of(args: &Args, port: &str) -> Result<Transition> {
        let fail = |m: &str| Error::msg(format!("node {:?}: {port}: {m}", args.node));
        let Some(Value::Stack(s)) = args.get(port) else { return Err(fail("connect a transition (cost.transition)")) };
        let layout: Layout = s
            .grid
            .geo
            .item(ITEM)
            .and_then(|t| serde_json::from_str(t).ok())
            .ok_or_else(|| fail("a transition (cost.transition), not any stack"))?;
        let mut g = (*s.grid).clone();
        g.geo.set_item(ITEM, None);
        let t =
            Transition { values: s.layers.iter().flat_map(|l| l.iter().copied()).collect(), layout, grid: Arc::new(g) };
        if t.values.len() != t.offsets()?.len() * t.cells() {
            return Err(fail("its layers do not match its directions"));
        }
        Ok(t)
    }

    /// The neighbour cell of every (direction, cell) of a layout of the grid (-1: none).
    fn neighbours(&self, directions: i32, symmetric: bool) -> Result<Vec<i64>> {
        Ok(ffi::cost_neighbours(&self.core(), directions, symmetric)?)
    }

    /// Row (0) or column (1) sums of the matrix, or counts of its connections.
    fn margin(&self, margin: i32, count: bool) -> Result<Vec<f64>> {
        let l = &self.layout;
        Ok(ffi::cost_margin_sums(&self.values, &self.core(), l.directions, l.symmetric, margin, count)?)
    }

    /// The values in another layout (more directions; asymmetric from symmetric), as landschaft's _relayout.
    pub fn relayout(&self, directions: i32, symmetric: bool) -> Result<Vec<f64>> {
        let l = &self.layout;
        if directions == l.directions && symmetric == l.symmetric {
            return Ok(self.values.clone());
        }
        if symmetric && !l.symmetric {
            return Err(Error::msg("an asymmetric transition cannot be made symmetric"));
        }
        if directions < l.directions {
            return Err(Error::msg("cannot drop directions"));
        }
        let target = offsets(directions, symmetric)?;
        let index = |o: (i32, i32)| target.iter().position(|&t| t == o).unwrap();
        let n = self.cells();
        let mut out = vec![0.0; target.len() * n];
        let src = self.offsets()?;
        if symmetric || !l.symmetric {
            for (k, &o) in src.iter().enumerate() {
                let j = index(o);
                out[j * n..(j + 1) * n].copy_from_slice(&self.values[k * n..(k + 1) * n]);
            }
            return Ok(out);
        }
        let nb = self.neighbours(l.directions, l.symmetric)?;
        for (k, &(dr, dc)) in src.iter().enumerate() {
            let (fwd, back) = (index((dr, dc)), index((-dr, -dc)));
            for cell in 0..n {
                let (v, to) = (self.values[k * n + cell], nb[k * n + cell]);
                if to >= 0 && v != 0.0 {
                    out[fwd * n + cell] = v;
                    out[back * n + to as usize] = v;
                }
            }
        }
        Ok(out)
    }

    /// A transition of other values on the same grid.
    fn like(&self, values: Vec<f64>, directions: i32, symmetric: bool) -> Transition {
        let layout = Layout { directions, symmetric, ..self.layout.clone() };
        Transition { values, layout, grid: self.grid.clone() }
    }

    /// Whether the matrix is symmetric (gdistance's isSymmetric): always for symmetric transitions, for
    /// asymmetric ones when every connection equals its reverse.
    pub fn is_symmetric(&self) -> Result<bool> {
        if self.layout.symmetric {
            return Ok(true);
        }
        let offs = self.offsets()?;
        let (ny, nx, n) = (self.grid.height as i64, self.grid.width as i64, self.cells());
        let index = |o: (i32, i32)| offs.iter().position(|&x| x == o);
        for (k, &(dr, dc)) in offs.iter().enumerate() {
            let back = index((-dr, -dc));
            for cell in 0..n {
                let v = self.values[k * n + cell];
                let (r, c) = (cell as i64 / nx, cell as i64 % nx);
                let (r2, mut c2) = (r + i64::from(dr), c + i64::from(dc));
                if self.layout.wrap {
                    c2 = c2.rem_euclid(nx);
                }
                if r2 < 0 || r2 >= ny || c2 < 0 || c2 >= nx {
                    continue;
                }
                let other = (r2 * nx + c2) as usize;
                let w = back.map_or(0.0, |b| self.values[b * n + other]);
                if v != w {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }
}

/// The directions of a layout: (row, column) offsets.
pub fn offsets(directions: i32, symmetric: bool) -> Result<Vec<(i32, i32)>> {
    let v = ffi::cost_offsets(directions, symmetric)?;
    Ok(v.chunks(2).map(|p| (p[0], p[1])).collect())
}

/// A map's grid as a transition's (landschaft's make_grid): its extent, whether its CRS is longitude/latitude
/// and whether it wraps around the globe.
pub fn grid_layout(g: &Grid) -> Result<([f64; 4], bool, bool)> {
    let t = g.transform;
    if t[2] != 0.0 || t[4] != 0.0 {
        return Err(Error::msg("rotated rasters are not supported"));
    }
    let (nx, ny) = (g.width, g.height);
    let (xmin, xmax, ymin, ymax) = (t[0], t[0] + nx as f64 * t[1], t[3] + ny as f64 * t[5], t[3]);
    if !(xmax > xmin && ymax > ymin) {
        return Err(Error::msg("the extent must have xmax > xmin and ymax > ymin"));
    }
    let crs = crate::prepare::crs_of(g);
    let lonlat =
        crs.as_deref().is_some_and(|c| c.contains("longlat") || terraflux_gdal::is_geographic(c).unwrap_or(false));
    let ext_ll = xmin > -365.0 && xmax < 365.0 && ymin > -90.1 && ymax < 90.1;
    let could_be_ll = lonlat || (crs.is_none() && ext_ll);
    let xres = (xmax - xmin) / nx as f64;
    let wrap = could_be_ll && (xmin + 180.0).abs() / xres <= 0.1 && (xmax - 180.0).abs() / xres <= 0.1;
    if wrap && nx < 5 {
        return Err(Error::msg("a global longitude/latitude grid needs at least 5 columns"));
    }
    Ok(([xmin, xmax, ymin, ymax], lonlat, wrap))
}

/// The cells of points (x, y) on a transition's grid (landschaft's cells_of_xy): -1 outside.
pub fn cells_of_xy(xy: &[[f64; 2]], t: &Transition) -> Vec<i64> {
    let (ny, nx) = (t.grid.height as f64, t.grid.width as f64);
    let [xmin, xmax, ymin, ymax] = t.layout.extent;
    let (yres_inv, xres_inv) = (ny / (ymax - ymin), nx / (xmax - xmin));
    xy.iter()
        .map(|&[x, y]| {
            let row = if y == ymin { ny - 1.0 } else { ((ymax - y) * yres_inv).floor() };
            let col = if x == xmax { nx - 1.0 } else { ((x - xmin) * xres_inv).floor() };
            if row.is_nan() || col.is_nan() || row < 0.0 || row >= ny || col < 0.0 || col >= nx {
                -1
            } else {
                (row * nx + col) as i64
            }
        })
        .collect()
}

/// Where of a node's port: the cells of a mask on the grid (its cells not 0), or of points (their
/// coordinates: features reprojected to the transition's grid, every point of multipoints).
fn cells_of(args: &Args, port: &str, t: &Transition) -> Result<Vec<i64>> {
    match args.get(port) {
        Some(Value::Map(m)) => {
            if m.data.len() != t.cells() {
                return Err(Error::msg(format!(
                    "node {:?}: {port}: the mask must have the transition's shape",
                    args.node
                )));
            }
            Ok(m.data.iter().enumerate().filter(|(_, v)| !v.is_nan() && **v != 0.0).map(|(k, _)| k as i64).collect())
        }
        Some(Value::Vector(v)) => {
            let v = crate::vector::on_grid(v, &t.grid)?;
            let mut xy = Vec::new();
            for g in &v.geometries {
                g.points(&mut |p| xy.push(p));
            }
            Ok(cells_of_xy(&xy, t))
        }
        Some(v) => Err(Error::msg(format!("node {:?}: {port}: points or a mask, not a {:?}", args.node, v.ty()))),
        None => Err(Error::msg(format!("node {:?}: connect the {port}", args.node))),
    }
}

/// landschaft's _clean: NaN connections left out (and said).
fn clean(values: &mut [f64]) {
    for v in values.iter_mut() {
        if v.is_nan() {
            *v = 0.0;
        }
    }
}

/// The cells of a node's port with those outside the grid left out.
fn inside(args: &Args, port: &str, t: &Transition) -> Result<Vec<i64>> {
    Ok(cells_of(args, port, t)?.into_iter().filter(|c| *c >= 0).collect())
}

/// A symmetric transition, or why not.
fn symmetric_of(args: &Args, what: &str) -> Result<Transition> {
    let t = Transition::of(args, "transition")?;
    if !t.is_symmetric()? {
        return Err(Error::msg(format!("node {:?}: {what} needs a symmetric transition", args.node)));
    }
    Ok(t)
}

/// A square or rectangular matrix as a table: a row per point from, its number, then a column per point to.
fn matrix_table(m: &[f64], nf: usize, nt: usize) -> Table {
    let mut table = Table::new(std::iter::once("point".to_string()).chain((1..=nt).map(|j| j.to_string())).collect());
    table.rows = (0..nf)
        .map(|i| std::iter::once((i + 1) as f64).chain(m[i * nt..(i + 1) * nt].iter().copied()).collect())
        .collect();
    table
}

fn theta_of(args: &Args) -> Result<f64> {
    let theta: f64 = args.param("theta", f64::NAN)?;
    if !(0.0..=20.0).contains(&theta) {
        return Err(Error::msg(format!("node {:?}: theta must be between 0 and 20 (gdistance's range)", args.node)));
    }
    Ok(theta)
}

fn net_of(args: &Args) -> Result<bool> {
    match args.param("total_net", "net".to_string())?.as_str() {
        "net" => Ok(true),
        "total" => Ok(false),
        _ => Err(Error::msg(format!("node {:?}: total_net should be either total or net", args.node))),
    }
}

const BUILTINS: [&str; 10] = ["mean", "min", "max", "sum", "prod", "from", "to", "diff", "harmonic", "geometric"];

fn transition_port(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new("transition", Type::Stack, "a transition (cost.transition)"))
}

struct TransitionOp;

impl Operator for TransitionOp {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "cost.transition",
            "A transition between neighbouring cells (gdistance's transition through landschaft's C++): the \
             connection of every cell with its 4, 8 or 16 neighbours, a function of the two cells' values (mean, \
             min, max, sum, prod, from, to, diff, harmonic, geometric), none where either has no value. Usually of \
             a conductance (1 / resistance), then corrected for the cells' distances (cost.geo_correction).",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the values (a conductance, usually)"))
        .output(PortSpec::new("transition", Type::Stack, "a layer per direction"))
        .param(
            PortSpec::optional("function", Type::Text, "of the two cells' values: mean (default)...").choices(BUILTINS),
        )
        .param(PortSpec::optional("directions", Type::Integer, "4, 8 (default) or 16").choices([4, 8, 16]))
        .param(PortSpec::optional("symmetric", Type::Bool, "one value per pair of cells (default true)"))
        .param(PortSpec::optional(
            "reciprocal",
            Type::Bool,
            "1 / the function's value (a conductance of a resistance; gdistance's 1 / transition(r, mean))",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let m = args.map("map")?;
        let function: String = args.param("function", "mean".to_string())?;
        let directions: i32 = args.param("directions", 8)?;
        let symmetric: bool = args.param("symmetric", true)?;
        if !BUILTINS.contains(&function.as_str()) {
            return Err(fail(Error::msg(format!("unknown function {function:?}; built-in: {}", BUILTINS.join(", ")))));
        }
        let (extent, lonlat, wrap) = grid_layout(&m.grid).map_err(fail)?;
        let layout = Layout { directions, symmetric, matrix_values: "conductance".into(), extent, lonlat, wrap };
        let mut t = Transition { values: Vec::new(), layout, grid: m.grid.clone() };
        t.values = ffi::cost_builtin(&m.data, &t.core(), directions, symmetric, &function)?;
        if args.param("reciprocal", false)? {
            // on the connections only, as landschaft's number / transition
            for v in t.values.iter_mut().filter(|v| **v != 0.0) {
                *v = 1.0 / *v;
            }
        }
        Ok(outputs([("transition", t.value()?)]))
    }
}

struct GeoCorrection;

impl Operator for GeoCorrection {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.geo_correction",
            "A transition corrected for the distances between cells' centres (gdistance's geoCorrection through \
             landschaft's C++): divided by them (type c, least-cost distances), or also by the cells' widths across \
             the connection (type r, random walks); great-circle distances on longitude/latitude grids.",
            Access::Global,
        ))
        .output(PortSpec::new("transition", Type::Stack, "the corrected transition"))
        .param(PortSpec::optional("type", Type::Text, "c (default) or r").choices(["c", "r"]))
        .param(PortSpec::optional(
            "scale",
            Type::Bool,
            "scaled by the distance between the grid's first cells",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let mut t = Transition::of(args, "transition")?;
        let kind: String = args.param("type", "c".to_string())?;
        let l = &t.layout;
        t.values = ffi::cost_geo_correction(
            &t.values,
            &t.core(),
            l.directions,
            l.symmetric,
            l.lonlat,
            &kind,
            l.matrix_values == "resistance",
            args.param("scale", false)?,
            false,
        )
        .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        clean(&mut t.values);
        Ok(outputs([("transition", t.value()?)]))
    }
}

struct AccCost;

impl Operator for AccCost {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.acc_cost",
            "The accumulated cost from the nearest origin over a transition of conductances (gdistance's accCost \
             through landschaft's C++): inf where no origin can be reached.",
            Access::Global,
        ))
        .input(PortSpec::new("origins", Type::Any, "points (vector features) or a mask (its cells not 0)"))
        .output(PortSpec::new("map", Type::Map, "the accumulated cost"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t = Transition::of(args, "transition")?;
        let cells: Vec<i64> = cells_of(args, "origins", &t)?.into_iter().filter(|c| *c >= 0).collect();
        let [xmin, xmax, ymin, ymax] = t.layout.extent;
        let out = ffi::cost_accumulated(
            &t.values,
            t.grid.height as i64,
            t.grid.width as i64,
            xmin,
            xmax,
            ymin,
            ymax,
            t.layout.wrap,
            t.layout.directions,
            t.layout.symmetric,
            &cells,
        )
        .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        Ok(outputs([("map", Value::Map(Map::new(t.grid.clone(), out)))]))
    }
}

/// The sorted unique cells of a list, those inside the grid.
fn unique(cells: &[i64]) -> Vec<i64> {
    let mut u: Vec<i64> = cells.iter().copied().filter(|c| *c >= 0).collect();
    u.sort_unstable();
    u.dedup();
    u
}

struct CostDistance;

impl Operator for CostDistance {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.distance",
            "The least costs between points over a transition of conductances (gdistance's costDistance through \
             landschaft's C++): from every point to every other (a symmetric matrix when the transition is), or to \
             every point of another set; no value for points outside the grid.",
            Access::Global,
        ))
        .input(PortSpec::new("from", Type::Vector, "the points"))
        .input(PortSpec::optional("to", Type::Vector, "other points (default: the same)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per point from: its number, then the cost to each point",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let t = Transition::of(args, "transition")?;
        let fc = cells_of(args, "from", &t)?;
        let to_given = args.get("to").is_some();
        let tc = if to_given { cells_of(args, "to", &t)? } else { fc.clone() };
        let (uf, ut) = (unique(&fc), if to_given { unique(&tc) } else { unique(&fc) });
        let l = &t.layout;
        let m = ffi::cost_matrix(&t.values, &t.core(), l.directions, l.symmetric, &uf, &ut)
            .map_err(|e| fail(Error::msg(e.to_string())))?;
        let (nf, nt) = (fc.len(), tc.len());
        let mut out = vec![f64::NAN; nf * nt];
        for (i, &a) in fc.iter().enumerate() {
            for (j, &b) in tc.iter().enumerate() {
                if a >= 0 && b >= 0 {
                    let (p, q) = (uf.binary_search(&a).unwrap(), ut.binary_search(&b).unwrap());
                    out[i * nt + j] = m[p * ut.len() + q];
                }
            }
        }
        // as.dist: the lower triangle, mirrored, 0 on the diagonal
        if !to_given && t.is_symmetric()? {
            for i in 0..nf {
                for j in 0..nf {
                    out[i * nt + j] = match i.cmp(&j) {
                        std::cmp::Ordering::Greater => out[i * nt + j] + 0.0,
                        std::cmp::Ordering::Less => out[j * nt + i] + 0.0,
                        std::cmp::Ordering::Equal => {
                            if fc[i] >= 0 {
                                0.0
                            } else {
                                f64::NAN
                            }
                        }
                    };
                }
            }
        }
        Ok(outputs([("table", Value::Table(matrix_table(&out, nf, nt)))]))
    }
}

struct ShortestPath;

impl Operator for ShortestPath {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.shortest_path",
            "The least-cost paths from an origin to goals over a transition of conductances (gdistance's \
             shortestPath through landschaft's C++, igraph's ties): lines through the cells' centres, and a map of \
             the share of the paths that cross each cell.",
            Access::Global,
        ))
        .input(PortSpec::new("origin", Type::Vector, "the origin (the first point)"))
        .input(PortSpec::new("goals", Type::Vector, "the goals"))
        .output(PortSpec::new("lines", Type::Vector, "a line per goal (id 1..)"))
        .output(PortSpec::new("map", Type::Map, "the share of the paths through each cell"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let t = Transition::of(args, "transition")?;
        let oc = cells_of(args, "origin", &t)?;
        let origin = *oc.first().filter(|c| **c >= 0).ok_or_else(|| fail("the origin is outside the grid".into()))?;
        let gc = cells_of(args, "goals", &t)?;
        let valid: Vec<i64> = gc.iter().copied().filter(|c| *c >= 0).collect();
        let l = &t.layout;
        let found = ffi::cost_paths(&t.values, &t.core(), l.directions, l.symmetric, origin, &valid)
            .map_err(|e| fail(e.to_string()))?;
        let mut paths: Vec<Vec<i64>> = vec![Vec::new(); gc.len()];
        let mut k = 0;
        for (i, &c) in gc.iter().enumerate() {
            if c >= 0 {
                paths[i] = found.cells[found.offsets[k] as usize..found.offsets[k + 1] as usize].to_vec();
                k += 1;
            }
        }
        let nx = t.grid.width as i64;
        let [xmin, xmax, ymin, ymax] = l.extent;
        let (xres, yres) = ((xmax - xmin) / nx as f64, (ymax - ymin) / t.grid.height as f64);
        let centre = |c: i64| [((c % nx) as f64 + 0.5) * xres + xmin, ymax - ((c / nx) as f64 + 0.5) * yres];
        let lines = Vector {
            geometries: paths
                .iter()
                .map(|p| {
                    if p.len() >= 2 {
                        Geometry::LineString(p.iter().map(|&c| centre(c)).collect())
                    } else {
                        Geometry::Empty
                    }
                })
                .collect(),
            attributes: vec![("id".into(), Column::Numbers((1..=paths.len()).map(|i| i as f64).collect()))],
            crs: crate::prepare::crs_of(&t.grid),
        };
        let mut share = vec![0.0; t.cells()];
        let w = 1.0 / paths.len().max(1) as f64;
        for p in &paths {
            let mut seen: Vec<i64> = p.clone();
            seen.sort_unstable();
            seen.dedup();
            for c in seen {
                share[c as usize] += w;
            }
        }
        Ok(outputs([("lines", Value::Vector(Arc::new(lines))), ("map", Value::Map(Map::new(t.grid.clone(), share)))]))
    }
}

struct CommuteDistance;

impl Operator for CommuteDistance {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.commute_distance",
            "The expected commute distances of a random walk between points (gdistance's commuteDistance through \
             landschaft's C++): how long a walk that moves to a neighbour with a probability proportional to the \
             conductance takes, on average, to reach the other point and come back; the effective resistance \
             times the graph's volume. inf between separate parts of the graph, no value for points outside it.",
            Access::Global,
        ))
        .input(PortSpec::new("points", Type::Vector, "the points"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per point: its number, then the distance to each point",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t = symmetric_of(args, "commute_distance")?;
        let cells = cells_of(args, "points", &t)?;
        let l = &t.layout;
        let c = ffi::cost_commute(&t.values, &t.core(), l.directions, l.symmetric, &cells)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let n = cells.len();
        let out: Vec<f64> = if c.part_volume.len() == 1 {
            c.resistance.iter().map(|r| r * c.volume).collect()
        } else {
            (0..n * n)
                .map(|k| {
                    let p = c.part[k / n];
                    c.resistance[k] * if p >= 0 { c.part_volume[p as usize] } else { f64::NAN }
                })
                .collect()
        };
        Ok(outputs([("table", Value::Table(matrix_table(&out, n, n)))]))
    }
}

struct Passage;

impl Operator for Passage {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.passage",
            "The passages of a random walk from origins to goals through every cell (gdistance's passage through \
             landschaft's C++). Without theta: the net number of times a random walk passes through each cell, the \
             current of a unit current from the origins to the goals (a symmetric transition; 1 at the origins and \
             goals, 0 off the graph). With theta (0 to 20): randomised shortest paths (Saerens et al. 2009), that \
             prefer cheap routes more as theta grows; no value off the graph.",
            Access::Global,
        ))
        .input(PortSpec::new("origins", Type::Any, "points (vector features) or a mask (its cells not 0)"))
        .input(PortSpec::new("goals", Type::Any, "points (vector features) or a mask (its cells not 0)"))
        .output(PortSpec::new("map", Type::Map, "the passages"))
        .param(PortSpec::optional("theta", Type::Number, "randomised shortest paths, 0 to 20 (default: none)"))
        .param(
            PortSpec::optional("total_net", Type::Text, "with theta: net (default) or total passages")
                .choices(["net", "total"]),
        )
        .param(PortSpec::optional(
            "gdistance_compat",
            Type::Bool,
            "without theta: gdistance's potential of the grounded cell (default true); false: the exact currents",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: cxx::Exception| Error::msg(format!("node {:?}: {e}", args.node));
        let t = Transition::of(args, "transition")?;
        let (oc, gc) = (inside(args, "origins", &t)?, inside(args, "goals", &t)?);
        let net = net_of(args)?;
        let l = &t.layout;
        let out = if args.params.get("theta").is_none_or(|v| v.is_null()) {
            let t = symmetric_of(args, "passage without theta")?;
            if !net {
                return Err(Error::msg(format!(
                    "node {:?}: without theta only net passages (gdistance: try a low value of theta instead)",
                    args.node
                )));
            }
            let quirk: bool = args.param("gdistance_compat", true)?;
            ffi::cost_rw_passages(&t.values, &t.core(), l.directions, l.symmetric, &oc, &gc, quirk).map_err(fail)?
        } else {
            let theta = theta_of(args)?;
            ffi::cost_rsp_passages(&t.values, &t.core(), l.directions, l.symmetric, &oc, &gc, theta, net, false)
                .map_err(fail)?
        };
        Ok(outputs([("map", Value::Map(Map::new(t.grid.clone(), out)))]))
    }
}

struct RspDistance;

impl Operator for RspDistance {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.rsp_distance",
            "Randomised-shortest-path distances between points (gdistance's rSPDistance through landschaft's C++): \
             the expected cost of the walk from each point to each other, near a random walk's with theta near 0, \
             near the least cost with large theta. No value for points off the grid.",
            Access::Global,
        ))
        .input(PortSpec::new("from", Type::Vector, "the points"))
        .input(PortSpec::optional("to", Type::Vector, "other points (default: the same)"))
        .output(PortSpec::new("table", Type::Table, "a row per point from: its number, then the cost to each point"))
        .param(PortSpec::new("theta", Type::Number, "0 to 20"))
        .param(
            PortSpec::optional("total_net", Type::Text, "the cost of the net (default) or total passages")
                .choices(["net", "total"]),
        )
        .param(
            PortSpec::optional("method", Type::Integer, "1 (default): exp(-theta cost) P; 2: P^theta").choices([1, 2]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t = Transition::of(args, "transition")?;
        let theta = theta_of(args)?;
        let method: i32 = args.param("method", 1)?;
        if method != 1 && method != 2 {
            return Err(Error::msg(format!("node {:?}: method should be either 1 or 2", args.node)));
        }
        let fc = cells_of(args, "from", &t)?;
        let tc = if args.get("to").is_some() { cells_of(args, "to", &t)? } else { fc.clone() };
        let l = &t.layout;
        let out = ffi::cost_rsp_distances(
            &t.values,
            &t.core(),
            l.directions,
            l.symmetric,
            &fc,
            &tc,
            theta,
            net_of(args)?,
            method,
        )
        .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        Ok(outputs([("table", Value::Table(matrix_table(&out, fc.len(), tc.len())))]))
    }
}

struct Normalize;

impl Operator for Normalize {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.normalize",
            "A transition normalised (gdistance's normalize): each row divided by its sum (the probabilities of a \
             random walk; asymmetric), each column, or by the square roots of the row and column sums (stays \
             symmetric).",
            Access::Global,
        ))
        .output(PortSpec::new("transition", Type::Stack, "the normalised transition"))
        .param(PortSpec::optional("method", Type::Text, "row (default), col or symm").choices(["row", "col", "symm"]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t = Transition::of(args, "transition")?;
        let method: String = args.param("method", "row".to_string())?;
        let (d, n) = (t.layout.directions, t.cells());
        let out = match method.as_str() {
            "row" | "col" => {
                let inv: Vec<f64> = t
                    .margin(if method == "row" { 0 } else { 1 }, false)?
                    .iter()
                    .map(|s| {
                        let v = 1.0 / s;
                        if v.is_infinite() { 0.0 } else { v }
                    })
                    .collect();
                let v = t.relayout(d, false)?;
                let out: Vec<f64> = if method == "row" {
                    v.iter().enumerate().map(|(k, x)| x * inv[k % n]).collect()
                } else {
                    let nb = t.neighbours(d, false)?;
                    v.iter().zip(&nb).map(|(x, &b)| if b >= 0 { x * inv[b as usize] } else { 0.0 }).collect()
                };
                t.like(out, d, false)
            }
            "symm" => {
                let rs = ffi::cost_rmath("pow", &t.margin(0, false)?, -0.5)?;
                let cs = ffi::cost_rmath("pow", &t.margin(1, false)?, -0.5)?;
                let s = t.layout.symmetric;
                let nb = t.neighbours(d, s)?;
                let out: Vec<f64> = t
                    .values
                    .iter()
                    .zip(&nb)
                    .enumerate()
                    .map(|(k, (&x, &b))| {
                        if b < 0 || x == 0.0 {
                            return 0.0;
                        }
                        let (cell, b) = ((k % n) as i64, b);
                        // forceSymmetric: the upper triangle, (x * rs[lower]) * cs[higher]
                        let (lo, hi) = if s { (cell.min(b), cell.max(b)) } else { (cell, b) };
                        (x * rs[lo as usize]) * cs[hi as usize]
                    })
                    .collect();
                t.like(out, d, s)
            }
            _ => return Err(Error::msg(format!("node {:?}: invalid method {method:?}: row, col or symm", args.node))),
        };
        Ok(outputs([("transition", out.value()?)]))
    }
}

/// Two transitions on the same grid in their common layout: the most directions, symmetric when both are.
fn common(a: &Transition, b: &Transition) -> Result<(Vec<f64>, Vec<f64>, i32, bool)> {
    let (ga, gb) = (&a.grid, &b.grid);
    if ga.width != gb.width
        || ga.height != gb.height
        || a.layout.extent != b.layout.extent
        || a.layout.lonlat != b.layout.lonlat
    {
        return Err(Error::msg("transitions on different grids"));
    }
    let d = a.layout.directions.max(b.layout.directions);
    let s = a.layout.symmetric && b.layout.symmetric;
    Ok((a.relayout(d, s)?, b.relayout(d, s)?, d, s))
}

fn two_transitions(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new("a", Type::Stack, "a transition"))
        .input(PortSpec::new("b", Type::Stack, "another, on the same grid"))
        .output(PortSpec::new("transition", Type::Stack, "the result"))
}

struct SumReciprocal;

impl Operator for SumReciprocal {
    fn signature(&self) -> OpSignature {
        two_transitions(OpSignature::new(
            "cost.sum_reciprocal",
            "Two transitions of conductances as resistances in series (gdistance's sumReciprocal): 1 / (1/a + 1/b) \
             on the connections.",
            Access::Global,
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let resist = |t: Transition| -> Transition {
            if t.layout.matrix_values != "conductance" {
                return t;
            }
            let v = t
                .values
                .iter()
                .map(|&x| if x != 0.0 { 1.0 / x } else { 0.0 })
                .map(|x| if x.is_infinite() { 0.0 } else { x })
                .collect();
            let (d, s) = (t.layout.directions, t.layout.symmetric);
            t.like(v, d, s)
        };
        let a = resist(Transition::of(args, "a")?);
        let b = resist(Transition::of(args, "b")?);
        let (va, vb, d, s) = common(&a, &b).map_err(fail)?;
        let out: Vec<f64> = va
            .iter()
            .zip(&vb)
            .map(|(x, y)| x + y)
            .map(|x| if x != 0.0 { 1.0 / x } else { 0.0 })
            .map(|x| if x.is_infinite() { 0.0 } else { x })
            .collect();
        let mut t = a.like(out, d, s);
        t.layout.matrix_values = "conductance".into();
        Ok(outputs([("transition", t.value()?)]))
    }
}

struct Combine;

impl Operator for Combine {
    fn signature(&self) -> OpSignature {
        two_transitions(OpSignature::new(
            "cost.combine",
            "Two transitions on the same grid added, subtracted or multiplied connection by connection \
             (gdistance's Arith on TransitionLayers), in their common layout: the most directions, symmetric when \
             both are.",
            Access::Global,
        ))
        .param(PortSpec::optional("operation", Type::Text, "sum (default), difference or product").choices([
            "sum",
            "difference",
            "product",
        ]))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let (a, b) = (Transition::of(args, "a")?, Transition::of(args, "b")?);
        let (va, vb, d, s) = common(&a, &b).map_err(fail)?;
        let op: String = args.param("operation", "sum".to_string())?;
        let f: fn(f64, f64) -> f64 = match op.as_str() {
            "sum" => |x, y| x + y,
            "difference" => |x, y| x - y,
            "product" => |x, y| x * y,
            _ => return Err(fail(Error::msg(format!("unknown operation {op:?}: sum, difference or product")))),
        };
        let out = va.iter().zip(&vb).map(|(x, y)| f(*x, *y)).collect();
        Ok(outputs([("transition", a.like(out, d, s).value()?)]))
    }
}

struct TransitionMath;

impl Operator for TransitionMath {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.math",
            "A function of a transition's values on its connections (landschaft's Transition.map, R's maths to the \
             bit): sqrt, exp, log, pow (to a value), multiply (by a value) or divide (a value by each, the \
             reciprocal for 1); NaN results leave the connection out.",
            Access::Global,
        ))
        .output(PortSpec::new("transition", Type::Stack, "the result"))
        .param(
            PortSpec::new("function", Type::Text, "sqrt, exp, log, pow, multiply or divide")
                .choices(["sqrt", "exp", "log", "pow", "multiply", "divide"]),
        )
        .param(PortSpec::optional("value", Type::Number, "the power, the factor or the dividend (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let mut t = Transition::of(args, "transition")?;
        let function: String = args.param("function", String::new())?;
        let value: f64 = args.param("value", 1.0)?;
        let on: Vec<usize> = (0..t.values.len()).filter(|&k| t.values[k] != 0.0).collect();
        let x: Vec<f64> = on.iter().map(|&k| t.values[k]).collect();
        let y: Vec<f64> = match function.as_str() {
            "sqrt" | "exp" | "log" | "pow" => ffi::cost_rmath(&function, &x, value)?,
            "multiply" => {
                // tr * number: every value
                t.values.iter_mut().for_each(|v| *v *= value);
                return Ok(outputs([("transition", t.value()?)]));
            }
            "divide" => x.iter().map(|v| value / v).collect(),
            _ => return Err(Error::msg(format!("node {:?}: unknown function {function:?}", args.node))),
        };
        let mut out = vec![0.0; t.values.len()];
        for (&k, v) in on.iter().zip(y) {
            out[k] = v;
        }
        clean(&mut out);
        t.values = out;
        Ok(outputs([("transition", t.value()?)]))
    }
}

const REDUCE: [&str; 6] = ["NZcolMeans", "NZrowMeans", "colSums", "rowSums", "colMeans", "rowMeans"];

struct ToMap;

impl Operator for ToMap {
    fn signature(&self) -> OpSignature {
        transition_port(OpSignature::new(
            "cost.to_map",
            "A map of each cell's connections (gdistance's raster() of a TransitionLayer): sums or means of the \
             matrix's columns (connections into the cell) or rows (out of it); NZ: the mean over the connections \
             only (no value without any).",
            Access::Global,
        ))
        .output(PortSpec::new("map", Type::Map, "a value per cell"))
        .param(PortSpec::optional("reduce", Type::Text, "NZcolMeans (default)...").choices(REDUCE))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let t = Transition::of(args, "transition")?;
        let reduce: String = args.param("reduce", "NZcolMeans".to_string())?;
        if !REDUCE.contains(&reduce.as_str()) {
            return Err(Error::msg(format!("node {:?}: reduce must be one of {}", args.node, REDUCE.join(", "))));
        }
        let margin = if reduce.contains("col") { 1 } else { 0 };
        let sums = t.margin(margin, false)?;
        let n = t.cells() as f64;
        let out: Vec<f64> = if reduce.starts_with("NZ") {
            sums.iter().zip(t.margin(margin, true)?).map(|(s, c)| s / c).collect()
        } else if reduce.ends_with("Means") {
            sums.iter().map(|s| s / n).collect()
        } else {
            sums
        };
        Ok(outputs([("map", Value::Map(Map::new(t.grid.clone(), out)))]))
    }
}
