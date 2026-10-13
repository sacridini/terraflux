//! Distances over a grid, landschaft's port of the R package gdistance through its C++: transitions between
//! neighbouring cells (4, 8 or 16 directions, a function of the two cells' values), corrected for the cells'
//! distances (geoCorrection), and over them the accumulated cost from origins (accCost), the least costs between
//! points (costDistance) and the least-cost paths (shortestPath).
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
        let mut table =
            Table::new(std::iter::once("point".to_string()).chain((1..=nt).map(|j| j.to_string())).collect());
        table.rows = (0..nf)
            .map(|i| std::iter::once((i + 1) as f64).chain(out[i * nt..(i + 1) * nt].iter().copied()).collect())
            .collect();
        Ok(outputs([("table", Value::Table(table))]))
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
