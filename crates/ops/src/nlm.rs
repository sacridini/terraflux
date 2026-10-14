//! Neutral landscape models from landschaft's port of NLMR: maps of known structure, drawn with R's generator
//! (or NLMR's C++ models' std::mt19937), to try a model on before real data (the templates of the application
//! start from them) or to compare a landscape with what chance would give; and NLMR's utilities to classify,
//! binarize, merge and rescale them. Landschaft's maps, to the bit.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(RandomCluster));
    r.add(Arc::new(GaussianField));
    r.add(Arc::new(Fbm));
    r.add(Arc::new(Random));
    r.add(Arc::new(Percolation));
    r.add(Arc::new(Gradient { edge: false }));
    r.add(Arc::new(Gradient { edge: true }));
    r.add(Arc::new(DistanceGradient));
    r.add(Arc::new(Curds));
    r.add(Arc::new(Mpd));
    r.add(Arc::new(Neigh));
    r.add(Arc::new(RectangularCluster));
    r.add(Arc::new(MosaicField));
    r.add(Arc::new(Tess { gibbs: false }));
    r.add(Arc::new(Tess { gibbs: true }));
    r.add(Arc::new(Classify));
    r.add(Arc::new(Binarize));
    r.add(Arc::new(Merge));
    r.add(Arc::new(Rescale));
}

/// The settings every model has: its size (or another map's grid), cell size and seed.
fn sized(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::optional("like", Type::Map, "a map whose grid to draw on (else rows x columns)"))
        .param(PortSpec::optional("rows", Type::Integer, "rows (default 256)"))
        .param(PortSpec::optional("columns", Type::Integer, "columns (default 256)"))
        .param(PortSpec::optional("resolution", Type::Number, "the cells' side (default 1)"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of R's generator (default 1): NLMR's map for it"))
}

/// The `rescale` setting.
fn rescaled(sig: OpSignature) -> OpSignature {
    sig.param(PortSpec::optional("rescale", Type::Bool, "rescale the values to 0-1 (default true)"))
}

/// The seed of the models drawn with R's generator.
fn seed(args: &Args) -> Result<i32> {
    let seed: i64 = args.param("seed", 1)?;
    i32::try_from(seed).map_err(|_| Error::msg(format!("the seed {seed} does not fit 32 bits")))
}

/// A grid of rows x columns of cells of side `res`, its origin at the bottom left (landschaft's).
fn plain(rows: usize, columns: usize, res: f64) -> Grid {
    let mut g = Grid::new(columns, rows);
    g.transform = [0.0, res, 0.0, rows as f64 * res, 0.0, -res];
    g
}

/// The grid to draw on and the seed.
fn grid(args: &Args) -> Result<(Arc<Grid>, i32)> {
    let seed = seed(args)?;
    if let Some(Value::Map(m)) = args.get("like") {
        return Ok((m.grid.clone(), seed));
    }
    let rows: usize = args.param("rows", 256)?;
    let columns: usize = args.param("columns", 256)?;
    if rows == 0 || columns == 0 || rows * columns > 100_000_000 {
        return Err(Error::msg(format!("node {:?}: from 1 to 100 million cells", args.node)));
    }
    let res: f64 = args.param("resolution", 1.0)?;
    if !(res.is_finite() && res > 0.0) {
        return Err(Error::msg(format!("node {:?}: the resolution must be positive", args.node)));
    }
    Ok((Arc::new(plain(rows, columns, res)), seed))
}

/// The grid of values that do not have the size asked for: `like`'s cells from its top-left corner, else
/// landschaft's grid of their size.
fn resized(args: &Args, g: &Grid, rows: usize, columns: usize) -> Arc<Grid> {
    let mut out = g.clone();
    out.width = columns;
    out.height = rows;
    if args.get("like").is_none() {
        out.transform[3] = rows as f64 * g.transform[1];
    }
    Arc::new(out)
}

/// The rows and columns of a grid, as the models take them.
fn dims(g: &Grid) -> (i64, i64) {
    (g.height as i64, g.width as i64)
}

/// A landschaft error, on the node.
fn ls<T, E: std::fmt::Display>(args: &Args, r: std::result::Result<T, E>) -> Result<T> {
    r.map_err(|e| bad(args, e))
}

fn bad(args: &Args, m: impl std::fmt::Display) -> Error {
    Error::msg(format!("node {:?}: {m}", args.node))
}

fn map_out(grid: Arc<Grid>, v: Vec<f64>) -> Outputs {
    outputs([("map", Value::Map(Map::new(grid, v)))])
}

/// NLMR's util_rescale when `rescale` is on (the default).
fn maybe_rescale(args: &Args, v: Vec<f64>) -> Result<Vec<f64>> {
    Ok(if args.param("rescale", true)? { ffi::nlm_rescale(&v) } else { v })
}

/// A required setting.
fn needed<T: serde::de::DeserializeOwned>(args: &Args, name: &str) -> Result<T> {
    args.opt_param(name)?.ok_or_else(|| bad(args, format!("set {name}")))
}

struct RandomCluster;

impl Operator for RandomCluster {
    fn signature(&self) -> OpSignature {
        sized(
            OpSignature::new(
                "nlm.random_cluster",
                "A map of patches of classes 1, 2... (NLMR's nlm_randomcluster, Saura & Martínez-Millán 2000): a \
                 share p of the cells grows into clusters, each of a class drawn with the weights, and the rest \
                 takes its neighbours' class. A forest map to try MSPA or a land change model on.",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "the classes 1, 2...")),
        )
        .param(PortSpec::optional("p", Type::Number, "the share of cells in clusters, up to 1 (default 0.5)"))
        .param(PortSpec::optional(
            "weights",
            Type::Any,
            "the classes' weights, their share of the clusters (default [0.5, 0.5]: two classes)",
        ))
        .param(
            PortSpec::optional("neighbourhood", Type::Integer, "of a cluster's cells: 4 (default) or 8")
                .choices([4, 8]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let p: f64 = args.param("p", 0.5)?;
        let weights: Vec<f64> = args.param("weights", vec![0.5, 0.5])?;
        let neighbourhood: i32 = args.param("neighbourhood", 4)?;
        let (ny, nx) = dims(&grid);
        let v = ls(args, ffi::nlm_random_cluster(seed, ny, nx, p, &weights, neighbourhood))?;
        Ok(map_out(grid, v))
    }
}

struct GaussianField;

impl Operator for GaussianField {
    fn signature(&self) -> OpSignature {
        sized(
            OpSignature::new(
                "nlm.gaussian_field",
                "A smooth random surface from 0 to 1 (NLMR's nlm_gaussianfield): a Gaussian field whose \
                 autocorrelation fades over the range given. A driver, a suitability or an elevation to try a \
                 model on; reclassified, a map of classes.",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "the field, 0 to 1")),
        )
        .param(PortSpec::optional("range", Type::Number, "the range of the autocorrelation, in cells (default 10)"))
        .param(PortSpec::optional("variance", Type::Number, "of the autocorrelated part (default 5)"))
        .param(PortSpec::optional(
            "nugget",
            Type::Number,
            "the variance of the noise without structure (default 0.2)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let range: f64 = args.param("range", 10.0)?;
        let var: f64 = args.param("variance", 5.0)?;
        let nug: f64 = args.param("nugget", 0.2)?;
        let (ny, nx) = dims(&grid);
        let v = ls(args, ffi::nlm_gaussian_field(seed, ny, nx, range, var, nug, 0.5))?;
        Ok(map_out(grid, ffi::nlm_rescale(&v)))
    }
}

struct Fbm;

impl Operator for Fbm {
    fn signature(&self) -> OpSignature {
        sized(
            OpSignature::new(
                "nlm.fbm",
                "A fractal surface from 0 to 1 (NLMR's nlm_fbm, fractional Brownian motion): rough for a small \
                 fractal dimension, smooth for a large one (2 is a plane).",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "the surface, 0 to 1")),
        )
        .param(PortSpec::optional("dimension", Type::Number, "the fractal dimension, in (0, 2] (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let dim: f64 = args.param("dimension", 1.0)?;
        let (ny, nx) = dims(&grid);
        let v = ls(args, ffi::nlm_fbm(seed, ny, nx, dim))?;
        Ok(map_out(grid, ffi::nlm_rescale(&v)))
    }
}

struct Random;

impl Operator for Random {
    fn signature(&self) -> OpSignature {
        rescaled(sized(
            OpSignature::new(
                "nlm.random",
                "Spatially random values (NLMR's nlm_random): a uniform draw for every cell, by column as R \
                 fills a matrix. The null model of a landscape without structure.",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "the values (0 to 1)")),
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let (ny, nx) = dims(&grid);
        let v = ls(args, ffi::nlm_random(seed, ny, nx))?;
        Ok(map_out(grid, maybe_rescale(args, v)?))
    }
}

struct Percolation;

impl Operator for Percolation {
    fn signature(&self) -> OpSignature {
        sized(
            OpSignature::new(
                "nlm.percolation",
                "A random binary map (NLMR's nlm_percolation): each cell is 1 with the probability given (a uniform \
                 draw below it, by column), else 0.",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "1 and 0")),
        )
        .param(PortSpec::optional("prob", Type::Number, "the probability of a 1, in [0, 1] (default 0.5)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let prob: f64 = args.param("prob", 0.5)?;
        if !(0.0..=1.0).contains(&prob) {
            return Err(bad(args, format!("prob must be in [0, 1], got {prob}")));
        }
        let (ny, nx) = dims(&grid);
        Ok(map_out(grid, ls(args, ffi::nlm_percolation(seed, ny, nx, prob))?))
    }
}

struct Gradient {
    edge: bool,
}

impl Operator for Gradient {
    fn signature(&self) -> OpSignature {
        let sig = if self.edge {
            OpSignature::new(
                "nlm.edge_gradient",
                "A gradient that peaks in the middle of the map and falls to its edges along a direction (NLMR's \
                 nlm_edgegradient): the planar gradient rescaled and folded around 0.5.",
                Access::Global,
            )
        } else {
            OpSignature::new(
                "nlm.planar_gradient",
                "A linear gradient across the map along a direction (NLMR's nlm_planargradient): eastness times \
                 the column plus southness times the row.",
                Access::Global,
            )
        };
        rescaled(sized(sig.output(PortSpec::new("map", Type::Map, "the gradient")).output(PortSpec::new(
            "direction",
            Type::Number,
            "its direction, in degrees",
        ))))
        .param(PortSpec::optional(
            "direction",
            Type::Number,
            "in degrees, 0 north to 90 east (default: a uniform draw from 0 to 360 with the seed)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let direction = match args.opt_param::<f64>("direction")? {
            Some(d) if !d.is_nan() => d,
            // runif(1, 0, 360) after set.seed(seed)
            _ => 0.0 + 360.0 * ffi::rrng_runif(ffi::rrng_new(seed).pin_mut(), 1)[0],
        };
        let rescale: bool = args.param("rescale", true)?;
        let (ny, nx) = dims(&grid);
        let v = ls(args, ffi::nlm_planar_gradient(ny, nx, direction, rescale, self.edge))?;
        Ok(outputs([("map", Value::Map(Map::new(grid, v))), ("direction", Value::Number(direction))]))
    }
}

struct DistanceGradient;

impl Operator for DistanceGradient {
    fn signature(&self) -> OpSignature {
        rescaled(sized(
            OpSignature::new(
                "nlm.distance_gradient",
                "The distance of every cell to a rectangle of cells (NLMR's nlm_distancegradient). As NLMR's (the \
                 default): terra's distance, on a lon/lat grid of 1 degree cells for maps up to 360 x 90 (metres \
                 on the sphere), else GDAL's proximity in cells; otherwise the exact distance between the cells' \
                 centres, in map units.",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "the distances")),
        ))
        .param(PortSpec::new(
            "origin",
            Type::Any,
            "the rectangle, 1-based and inclusive: NLMR's [row from, row to, column from, column to] (as its \
             checks: the second at most the columns and the fourth at most the rows); without NLMR's, [column \
             from, column to, row from, row to]",
        ))
        .param(PortSpec::optional("nlmr_compat", Type::Bool, "NLMR's distances and origin (default true)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, _) = grid(args)?;
        let o: Vec<i64> = needed(args, "origin")?;
        if o.len() != 4 {
            return Err(bad(args, "origin must have 4 values"));
        }
        let compat: bool = args.param("nlmr_compat", true)?;
        let (ny, nx) = dims(&grid);
        let (r0, r1, c0, c1) = if compat {
            if !(o[1] <= nx && o[3] <= ny) {
                return Err(bad(args, "origin[2] must be <= the columns and origin[4] <= the rows"));
            }
            (o[0], o[1], o[2], o[3])
        } else {
            (o[2], o[3], o[0], o[1])
        };
        if !(1 <= r0 && r0 <= r1 && r1 <= ny && 1 <= c0 && c0 <= c1 && c1 <= nx) {
            return Err(bad(args, format!("origin {o:?} is outside the {ny} x {nx} raster")));
        }
        let mut mask = vec![0u8; (ny * nx) as usize];
        for r in r0 - 1..r1 {
            for c in c0 - 1..c1 {
                mask[(r * nx + c) as usize] = 1;
            }
        }
        let v = if !compat {
            ffi::nlm_distance(&mask, ny, nx, 2, grid.transform[1], 0.0, 0.0, 1.0, 1.0)
        } else if nx <= 360 && ny <= 90 {
            ffi::nlm_distance(&mask, ny, nx, 1, 1.0, 0.0, ny as f64, 1.0, 1.0)
        } else {
            ffi::nlm_distance(&mask, ny, nx, 0, 1.0, 0.0, 0.0, 1.0, 1.0)
        };
        let v = ls(args, v)?;
        Ok(map_out(grid, maybe_rescale(args, v)?))
    }
}

struct Curds;

impl Operator for Curds {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "nlm.curds",
            "A hierarchical binary map (NLMR's nlm_curds, curdling): at each level every cell splits into steps x \
             steps cells and a share of them becomes matrix (0), the rest habitat (1); with wheyes, a share of the \
             matrix turns back to habitat (wheyed curdling). Its side is the product of the steps.",
            Access::Global,
        )
        .output(PortSpec::new("map", Type::Map, "1 habitat, 0 matrix"))
        .param(PortSpec::new("curds", Type::Any, "each level's share of matrix cells, e.g. [0.5, 0.3, 0.6]"))
        .param(PortSpec::new("steps", Type::Any, "each level's split, e.g. [32, 6, 2]"))
        .param(PortSpec::optional("wheyes", Type::Any, "each level's share of matrix turned to habitat"))
        .param(PortSpec::optional("resolution", Type::Number, "the cells' side (default 1)"))
        .param(PortSpec::optional(
            "seed",
            Type::Integer,
            "the seed of R's generator (default 1): NLMR's map for it",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let seed = seed(args)?;
        let p: Vec<f64> = needed(args, "curds")?;
        let steps: Vec<i64> = needed(args, "steps")?;
        if p.len() != steps.len() {
            return Err(bad(args, "curds and steps differ in length"));
        }
        if steps.iter().any(|&s| s < 1) {
            return Err(bad(args, "the steps must be positive integers"));
        }
        let side = steps.iter().try_fold(1i64, |a, &s| a.checked_mul(s)).filter(|&s| s * s <= 100_000_000);
        if side.is_none() {
            return Err(bad(args, "the map would have more than 100 million cells"));
        }
        let wheyes: Option<Vec<f64>> = args.opt_param("wheyes")?;
        let res: f64 = args.param("resolution", 1.0)?;
        let v = ls(args, ffi::nlm_curds(seed, &p, &steps, wheyes.as_deref().unwrap_or(&[]), wheyes.is_some()))?;
        let side = (v.len() as f64).sqrt().round() as usize;
        Ok(map_out(Arc::new(plain(side, side, res)), v))
    }
}

/// The seed of NLMR's C++ models (std::mt19937): landschaft's, modulo 2^32.
fn cxx_seed(seed: i32) -> u32 {
    seed as u32
}

struct Mpd;

impl Operator for Mpd {
    fn signature(&self) -> OpSignature {
        rescaled(sized(
            OpSignature::new(
                "nlm.mpd",
                "A fractal surface by midpoint displacement (NLMR's nlm_mpd, the diamond-square algorithm): each \
                 step halves the cells and displaces the midpoints by a deviation that shrinks with the roughness. \
                 NLMR's grid (the default) is the odd side above the larger dimension, less its border: it may \
                 differ from the size asked for (a warning says so).",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "the surface")),
        ))
        .param(PortSpec::optional("roughness", Type::Number, "in [0, 1]: how fast the deviation shrinks (0.5)"))
        .param(PortSpec::optional("deviation", Type::Number, "the first step's deviation (default 1)"))
        .param(PortSpec::optional("torus", Type::Bool, "wrap the edges around (default false)"))
        .param(PortSpec::optional("nlmr_compat", Type::Bool, "NLMR's grid (default true)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let roughness: f64 = args.param("roughness", 0.5)?;
        if !(0.0..=1.0).contains(&roughness) {
            return Err(bad(args, format!("roughness must be in [0, 1], got {roughness}")));
        }
        let deviation: f64 = args.param("deviation", 1.0)?;
        let torus: bool = args.param("torus", false)?;
        let compat: bool = args.param("nlmr_compat", true)?;
        let (nrow, ncol) = (grid.height, grid.width);
        let size = if compat {
            let s = (nrow + 1).max(ncol + 1);
            if s % 2 == 0 { s + 1 } else { s }
        } else {
            let mut s = 3;
            while s - 1 < nrow.max(ncol) {
                s = 2 * (s - 1) + 1;
            }
            s
        };
        let n_steps = ((size - 1) as f64).log2().ceil() as usize;
        let mut devs = vec![deviation];
        for _ in 1..n_steps {
            devs.push(devs[devs.len() - 1] * roughness);
        }
        let full = ls(args, ffi::nlm_mpd(size as i64, &devs, cxx_seed(seed), torus))?;
        let (rows, cols, r0, c0) = if compat { (size - 2, size - 2, 1, 1) } else { (nrow, ncol, 0, 0) };
        let mut v = Vec::with_capacity(rows * cols);
        for r in r0..r0 + rows {
            v.extend_from_slice(&full[r * size + c0..r * size + c0 + cols]);
        }
        let grid = if (rows, cols) != (nrow, ncol) {
            ctx.warn(format!(
                "node {:?}: NLMR's grid gives a {rows} x {cols} map instead of {nrow} x {ncol}; set nlmr_compat to \
                 false for the size asked for",
                args.node
            ));
            resized(args, &grid, rows, cols)
        } else {
            grid
        };
        Ok(map_out(grid, maybe_rescale(args, v)?))
    }
}

struct Neigh;

impl Operator for Neigh {
    fn signature(&self) -> OpSignature {
        rescaled(sized(
            OpSignature::new(
                "nlm.neigh",
                "A map of categories grown by neighbourhood (NLMR's nlm_neigh): cells are filled one at a time, \
                 with the category of their neighbours (probability p_neigh) or, around empty ones, at random \
                 (p_empty), until each category has its share. Categories 0, 1...",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "the categories")),
        ))
        .param(PortSpec::new("p_neigh", Type::Number, "the probability of a cell with filled neighbours, [0, 1]"))
        .param(PortSpec::new("p_empty", Type::Number, "the probability of a cell without, [0, 1]"))
        .param(PortSpec::optional("categories", Type::Integer, "how many (default 3)"))
        .param(PortSpec::optional("neighbourhood", Type::Integer, "4 (default) or 8 neighbours").choices([4, 8]))
        .param(PortSpec::optional(
            "proportions",
            Type::Any,
            "each category's share of the cells, summing to 1 (default equal shares)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let p_neigh: f64 = needed(args, "p_neigh")?;
        let p_empty: f64 = needed(args, "p_empty")?;
        for (name, p) in [("p_neigh", p_neigh), ("p_empty", p_empty)] {
            if !(0.0..=1.0).contains(&p) {
                return Err(bad(args, format!("{name} must be in [0, 1], got {p}")));
            }
        }
        if p_neigh == 0.0 && p_empty == 0.0 {
            return Err(bad(args, "p_neigh and p_empty cannot both be 0: no cell would ever be filled"));
        }
        let categories: i64 = args.param("categories", 3)?;
        if categories < 1 {
            return Err(bad(args, "categories must be a positive integer"));
        }
        let neighbourhood: i32 = args.param("neighbourhood", 4)?;
        if neighbourhood != 4 && neighbourhood != 8 {
            return Err(bad(args, "the neighbourhood is 4 or 8"));
        }
        let (nrow, ncol) = (grid.height as f64, grid.width as f64);
        let cells: Vec<i64> = match args.opt_param::<Vec<f64>>("proportions")? {
            Some(props) => {
                if props.len() as i64 != categories {
                    return Err(bad(args, "proportions must have one value per category"));
                }
                if ffi::r_sum(&props) != 1.0 {
                    return Err(bad(args, "proportions must sum to 1"));
                }
                props.iter().map(|p| (p * nrow * ncol) as i64).collect()
            }
            None => vec![(nrow * ncol / categories as f64).floor() as i64; categories as usize],
        };
        let (ny, nx) = dims(&grid);
        let v = ls(
            args,
            ffi::nlm_neigh(
                ny,
                nx,
                (categories - 1) as i32,
                &cells,
                neighbourhood,
                p_neigh as f32,
                p_empty as f32,
                cxx_seed(seed),
            ),
        )?;
        Ok(map_out(grid, maybe_rescale(args, v)?))
    }
}

struct RectangularCluster;

impl Operator for RectangularCluster {
    fn signature(&self) -> OpSignature {
        rescaled(sized(
            OpSignature::new(
                "nlm.random_rectangular_cluster",
                "Overlapping rectangles of random values (NLMR's nlm_randomrectangularcluster): rectangles with \
                 sides from minl to maxl cells, each of a uniform value, until the map is covered.",
                Access::Global,
            )
            .output(PortSpec::new("map", Type::Map, "the values")),
        ))
        .param(PortSpec::new("minl", Type::Integer, "the shortest side, in cells"))
        .param(PortSpec::new("maxl", Type::Integer, "the longest, at most the rows and the columns"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let minl: i64 = needed(args, "minl")?;
        let maxl: i64 = needed(args, "maxl")?;
        let (ny, nx) = dims(&grid);
        if !(1 <= minl && minl <= maxl && maxl <= nx && maxl <= ny) {
            return Err(bad(args, "need 1 <= minl <= maxl <= min(columns, rows)"));
        }
        let v = ls(args, ffi::nlm_random_rectangular_cluster(ny, nx, minl as i32, maxl as i32, cxx_seed(seed)))?;
        Ok(map_out(grid, maybe_rescale(args, v)?))
    }
}

struct MosaicField;

impl Operator for MosaicField {
    fn signature(&self) -> OpSignature {
        rescaled(sized(
            OpSignature::new(
                "nlm.mosaic_field",
                "A mosaic random field (NLMR's nlm_mosaicfield, spatstat's rMosaicField): the sum of n mosaics, \
                 each a Poisson line tessellation with a normal value per tile; optionally every step's sum and \
                 the infinite field (a log-Gaussian Cox process).",
                Access::Global,
            )
            .output(PortSpec::optional("map", Type::Map, "the sum of the n mosaics"))
            .output(PortSpec::optional("steps", Type::Stack, "the sum after each step (`collect`)"))
            .output(PortSpec::optional("infinit", Type::Map, "the infinite field (`infinit`)")),
        ))
        .param(PortSpec::optional("n", Type::Integer, "the mosaics (default 20; 0 for only the infinite field)"))
        .param(PortSpec::optional("mean", Type::Number, "of the tiles' values (default 0.5)"))
        .param(PortSpec::optional("sd", Type::Number, "of the tiles' values (default 0.5)"))
        .param(PortSpec::optional("collect", Type::Bool, "every step's sum too (default false)"))
        .param(PortSpec::optional("infinit", Type::Bool, "the infinite field too (default false)"))
        .param(PortSpec::optional(
            "nlmr_compat",
            Type::Bool,
            "NLMR's quirks (default true): its steps rescaled with the first's range",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let n: i64 = args.param("n", 20)?;
        let infinit: bool = args.param("infinit", false)?;
        if n < 0 || (n == 0 && !infinit) {
            return Err(bad(args, "n must be a positive integer (0, only the infinite field, needs infinit)"));
        }
        let compat: bool = args.param("nlmr_compat", true)?;
        let collect: bool = args.param("collect", false)?;
        let rescale: bool = args.param("rescale", true)?;
        let (ny, nx) = dims(&grid);
        let r = ls(
            args,
            ffi::nlm_mosaic_field(
                seed,
                ny,
                nx,
                n as i32,
                compat,
                args.param("mean", 0.5)?,
                args.param("sd", 0.5)?,
                collect && n > 0,
                infinit,
            ),
        )?;
        let scale = |v: Vec<f64>| if rescale { ffi::nlm_rescale(&v) } else { v };
        let mut out = Outputs::new();
        if !r.sum.is_empty() {
            out.insert("map".into(), Value::Map(Map::new(grid.clone(), scale(r.sum))));
        }
        if collect && n > 0 {
            let cells = (ny * nx) as usize;
            let mut layers: Vec<Vec<f64>> = r.steps.chunks(cells).map(|c| c.to_vec()).collect();
            if rescale {
                layers = rescale_layers(layers, compat);
            }
            out.insert(
                "steps".into(),
                Value::Stack(Stack {
                    grid: grid.clone(),
                    keys: (1..=layers.len()).map(|i| Key::Name(format!("step_{i}"))).collect(),
                    layers: layers.into_iter().map(Arc::new).collect(),
                }),
            );
        }
        if infinit {
            out.insert("infinit".into(), Value::Map(Map::new(grid, scale(r.infinit))));
        }
        Ok(out)
    }
}

/// nlm_mosaicfield's steps rescaled: each on its own, or (NLMR's) all with the first's range.
fn rescale_layers(layers: Vec<Vec<f64>>, compat: bool) -> Vec<Vec<f64>> {
    if layers.is_empty() {
        return layers;
    }
    if !compat {
        return layers.iter().map(|v| ffi::nlm_rescale(v)).collect();
    }
    let (lo, hi) = layers[0].iter().filter(|x| !x.is_nan()).fold((f64::NAN, f64::NAN), |(a, b), &x| {
        (if a.is_nan() || x < a { x } else { a }, if b.is_nan() || x > b { x } else { b })
    });
    if ffi::nlm_rescale(&[lo, hi])[1] == 0.0 {
        return layers.iter().map(|v| vec![0.0; v.len()]).collect();
    }
    layers.into_iter().map(|v| v.into_iter().map(|x| (x - lo) / (hi - lo)).collect()).collect()
}

struct Tess {
    gibbs: bool,
}

impl Operator for Tess {
    fn signature(&self) -> OpSignature {
        let sig = if self.gibbs {
            OpSignature::new(
                "nlm.mosaic_gibbs",
                "A Voronoi mosaic of classes from germs kept apart (NLMR's nlm_mosaicgibbs): spatstat's rSSI germs \
                 at least R apart, each with a class 1..patch_classes, every cell the class of its nearest germ. \
                 As NLMR (nlmr_compat, the default) the k-th class goes to the k-th Voronoi cell in the order GEOS \
                 lists them (GEOS 3.11 or later, next to GDAL); else each germ keeps its own class.",
                Access::Global,
            )
        } else {
            OpSignature::new(
                "nlm.mosaic_tess",
                "A Voronoi mosaic of random values (NLMR's nlm_mosaictess): random germs (sf's st_sample), a \
                 uniform value each, every cell the value of its nearest germ. As NLMR (nlmr_compat, the default) \
                 the k-th value goes to the k-th Voronoi cell in the order GEOS lists them (GEOS 3.11 or later, \
                 next to GDAL) and the values are floats, as terra::rasterize writes them; else each germ keeps \
                 its own value, in double precision.",
                Access::Global,
            )
        };
        let sig = rescaled(sized(sig.output(PortSpec::new("map", Type::Map, "the mosaic"))))
            .param(PortSpec::new("germs", Type::Integer, "how many germs"))
            .param(PortSpec::optional(
                "nlmr_compat",
                Type::Bool,
                "NLMR's assignment of the values to the Voronoi cells by GEOS's order (default true)",
            ));
        if self.gibbs {
            sig.param(PortSpec::new("r", Type::Number, "the shortest distance between germs, in cells"))
                .param(PortSpec::new("patch_classes", Type::Integer, "the classes"))
        } else {
            sig
        }
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let germs: i64 = needed(args, "germs")?;
        let (ny, nx) = dims(&grid);
        let (x, y, v) = if self.gibbs {
            let r: f64 = needed(args, "r")?;
            let classes: i64 = needed(args, "patch_classes")?;
            if germs < 1 || classes < 1 {
                return Err(bad(args, "germs and patch_classes must be positive integers"));
            }
            if r < 0.0 {
                return Err(bad(args, "r must be non-negative"));
            }
            let g = ls(args, ffi::nlm_gibbs_germs(seed, germs, r, classes as i32, nx as f64, ny as f64, 1000))?;
            if (g.x.len() as i64) < germs {
                ctx.warn(format!(
                    "node {:?}: only {} of {germs} germs fit at distance {r} (rSSI gave up after 1000 attempts)",
                    args.node,
                    g.x.len()
                ));
            }
            let n = g.x.len();
            (g.x, g.y, g.v[..n].to_vec())
        } else {
            if germs < 1 {
                return Err(bad(args, "germs must be a positive integer"));
            }
            let g = ls(args, ffi::nlm_tess_germs(seed, germs, nx as f64, ny as f64))?;
            (g.x, g.y, g.v)
        };
        let compat: bool = args.param("nlmr_compat", true)?;
        let v = if compat { by_cell_order(ctx, args, &x, &y, v, if self.gibbs { 0.1 } else { 0.0 })? } else { v };
        let mut v = ls(args, ffi::nlm_nearest_germ_raster(&x, &y, &v, ny, nx))?;
        if compat {
            // terra::rasterize() writes floats
            v.iter_mut().for_each(|x| *x = f64::from(*x as f32));
        }
        Ok(map_out(grid, maybe_rescale(args, v)?))
    }
}

/// NLMR gives the k-th value to the k-th Voronoi cell in the order GEOS lists them (sf::st_voronoi of the germs'
/// union): the values reassigned so (landschaft's _values_by_cell_order); without GEOS each germ keeps its own,
/// with a warning.
fn by_cell_order(
    ctx: &mut Ctx,
    args: &Args,
    x: &[f64],
    y: &[f64],
    values: Vec<f64>,
    tolerance: f64,
) -> Result<Vec<f64>> {
    let g = match terraflux_gdal::geos::geos() {
        Ok(g) => g,
        Err(e) => {
            ctx.warn(format!(
                "node {:?}: GEOS is needed for NLMR's assignment of values to the Voronoi cells ({e}); each germ gets \
                 its own value instead (or set nlmr_compat to false)",
                args.node
            ));
            return Ok(values);
        }
    };
    if g.version_number() < (3, 11) {
        ctx.warn(format!(
            "node {:?}: GEOS {} orders the Voronoi cells differently from GEOS 3.11 and later, so the values do not \
             follow NLMR's",
            args.node, g.version
        ));
    }
    let germ_of_cell = g.voronoi_cell_order(x, y, tolerance).map_err(|e| bad(args, e))?;
    let mut out = values.clone();
    for (k, &germ) in germ_of_cell.iter().enumerate() {
        out[germ] = values[k];
    }
    Ok(out)
}

/// NLMR's landscape_classify: the classes 1, 2... of the values (NaN none), by the weights (each class's share of
/// the cells; nulls share what the others leave) or in n classes by Jenks's natural breaks.
fn classify(values: &[f64], n: Option<i64>, weights: Option<&[Option<f64>]>) -> std::result::Result<Vec<f64>, String> {
    if let Some(weights) = weights {
        let mut w: Vec<f64> = weights.iter().map(|v| v.unwrap_or(f64::NAN)).collect();
        let na = w.iter().filter(|v| v.is_nan()).count();
        if na > 0 {
            let known: Vec<f64> = w.iter().copied().filter(|v| !v.is_nan()).collect();
            let fill = (1.0 - ffi::r_sum(&known)) / na as f64;
            w.iter_mut().filter(|v| v.is_nan()).for_each(|v| *v = fill);
        }
        if w.iter().any(|&v| v < 0.0) {
            return Err("the weights must be non-negative".into());
        }
        let total = ffi::r_sum(&w);
        if total.abs() <= 1.5e-8 {
            return Err("the weights must sum to a positive value".into());
        }
        let cp = ffi::nlm_r_cumsum(&w.iter().map(|v| v / total).collect::<Vec<_>>());
        let mut x: Vec<f64> = values.iter().copied().filter(|v| !v.is_nan()).collect();
        if x.is_empty() {
            return Err("the map has no values".into());
        }
        x.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let len = x.len();
        let boundaries: Vec<f64> =
            cp.iter().map(|&p| x[((p * len as f64).trunc() as i64).clamp(1, len as i64) as usize - 1]).collect();
        if boundaries.iter().all(|&b| b == boundaries[0]) {
            return Ok(vec![1.0; values.len()]);
        }
        let mut breaks = vec![0.0];
        breaks.extend(boundaries);
        return cut(values, breaks);
    }
    let k = n.ok_or("set the classes or the weights (or connect a real landscape)")?;
    if k < 1 {
        return Err("the classes must be a positive integer".into());
    }
    let mut v: Vec<f64> = values.iter().copied().filter(|v| !v.is_nan()).collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let distinct = 1 + v.windows(2).filter(|w| w[0] != w[1]).count();
    let k = k.min(if v.is_empty() { 0 } else { distinct as i64 });
    let breaks = ffi::nlm_jenks_breaks(&v, k as i32).map_err(|e| e.to_string())?;
    cut(values, breaks)
}

/// base::cut with include.lowest (the breaks sorted, distinct).
fn cut(values: &[f64], mut breaks: Vec<f64>) -> std::result::Result<Vec<f64>, String> {
    breaks.sort_by(|a, b| a.partial_cmp(b).unwrap());
    if breaks.windows(2).any(|w| w[0] == w[1]) {
        return Err("the breaks are not unique (base::cut): classes would be empty".into());
    }
    Ok(ffi::nlm_cut(values, &breaks))
}

struct Classify;

impl Operator for Classify {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "nlm.classify",
            "Classes 1, 2... of a map (NLMR's util_classify): by the weights, each class's share of the cells (the \
             cells sorted, the boundaries at the cumulative shares); by a real landscape's shares of its values; \
             or in n classes by Jenks's natural breaks.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the values (a neutral landscape)"))
        .input(PortSpec::optional("real", Type::Map, "a real landscape whose classes' shares to take"))
        .output(PortSpec::new("classes", Type::Map, "the classes 1, 2..."))
        .param(PortSpec::optional("n", Type::Integer, "the classes, by natural breaks"))
        .param(PortSpec::optional(
            "weights",
            Type::Any,
            "each class's share of the cells (null for an even share of what the others leave)",
        ))
        .param(PortSpec::optional("mask_value", Type::Number, "the real landscape's value to leave out"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let mut values = m.data.to_vec();
        let mut weights: Option<Vec<Option<f64>>> = args.opt_param("weights")?;
        if let Some(real) = args.opt_map("real")? {
            if real.shape() != m.shape() {
                return Err(bad(args, "the map and the real landscape must have the same grid"));
            }
            let mask: Option<f64> = args.opt_param("mask_value")?;
            let mut kept: Vec<f64> = Vec::new();
            for (v, &r) in values.iter_mut().zip(real.data.iter()) {
                if mask == Some(r) {
                    *v = f64::NAN;
                } else if !r.is_nan() {
                    kept.push(r);
                }
            }
            if kept.is_empty() {
                return Err(bad(args, "the real landscape has no values after masking"));
            }
            kept.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let mut counts: Vec<u64> = vec![1];
            for w in kept.windows(2) {
                if w[0] == w[1] {
                    *counts.last_mut().unwrap() += 1;
                } else {
                    counts.push(1);
                }
            }
            let total: u64 = counts.iter().sum();
            weights = Some(counts.iter().map(|&c| Some(c as f64 / total as f64)).collect());
        }
        let classes = classify(&values, args.opt_param("n")?, weights.as_deref()).map_err(|e| bad(args, e))?;
        Ok(outputs([("classes", Value::Map(Map::new(m.grid.clone(), classes)))]))
    }
}

struct Binarize;

impl Operator for Binarize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "nlm.binarize",
            "Habitat (2) and matrix (1) of a map (NLMR's util_binarize): the share of habitat given, the cells \
             with the highest values. Several shares give a stack, a map each.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the values (a neutral landscape)"))
        .output(PortSpec::new("map", Type::Any, "1 matrix, 2 habitat: a map, or a stack by share"))
        .param(PortSpec::new("breaks", Type::Any, "the share of habitat, in [0, 1], or several"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let breaks: serde_json::Value = needed(args, "breaks")?;
        let (multi, bs): (bool, Vec<f64>) = match &breaks {
            serde_json::Value::Array(_) => (true, args.param("breaks", vec![])?),
            _ => (false, vec![args.param("breaks", 0.0)?]),
        };
        if bs.iter().any(|b| !(0.0..=1.0).contains(b)) {
            return Err(bad(args, "the breaks must be between 0 and 1"));
        }
        let mut layers = Vec::new();
        for &b in &bs {
            layers.push(classify(&m.data, None, Some(&[Some(1.0 - b), Some(b)])).map_err(|e| bad(args, e))?);
        }
        if !multi {
            return Ok(map_out(m.grid.clone(), layers.remove(0)));
        }
        Ok(outputs([(
            "map",
            Value::Stack(Stack {
                grid: m.grid.clone(),
                keys: bs.iter().map(|&b| Key::Name(crate::bio::names::py_float(b))).collect(),
                layers: layers.into_iter().map(Arc::new).collect(),
            }),
        )]))
    }
}

struct Merge;

impl Operator for Merge {
    fn signature(&self) -> OpSignature {
        rescaled(
            OpSignature::new(
                "nlm.merge",
                "Landscapes merged (NLMR's util_merge): the primary plus the sum of the secondary ones times the \
                 scaling, e.g. a gradient with noise on it.",
                Access::Global,
            )
            .input(PortSpec::new("primary", Type::Map, "the primary landscape"))
            .input(PortSpec::new("secondary", Type::Any, "a map or a stack of maps on its grid"))
            .output(PortSpec::new("map", Type::Map, "the merged landscape")),
        )
        .param(PortSpec::optional("scaling", Type::Number, "of the secondary landscapes (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let p = args.map("primary")?;
        let layers: Vec<&[f64]> = match args.get("secondary") {
            Some(Value::Map(s)) => {
                if s.shape() != p.shape() {
                    return Err(bad(args, "the landscapes must have the same shape"));
                }
                vec![&s.data[..]]
            }
            Some(Value::Stack(s)) => {
                if (s.grid.height, s.grid.width) != p.shape() {
                    return Err(bad(args, "the landscapes must have the same shape"));
                }
                if s.layers.is_empty() {
                    return Err(bad(args, "secondary must hold at least one landscape"));
                }
                s.layers.iter().map(|l| &l[..]).collect()
            }
            _ => return Err(bad(args, "connect a map or a stack as the secondary landscapes")),
        };
        let mut total = layers[0].to_vec();
        for l in &layers[1..] {
            total.iter_mut().zip(l.iter()).for_each(|(t, v)| *t += v);
        }
        let scaling: f64 = args.param("scaling", 1.0)?;
        let out: Vec<f64> = p.data.iter().zip(&total).map(|(a, t)| a + t * scaling).collect();
        Ok(map_out(p.grid.clone(), maybe_rescale(args, out)?))
    }
}

struct Rescale;

impl Operator for Rescale {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "nlm.rescale",
            "A map rescaled to 0-1 (NLMR's util_rescale): (x - min) / (max - min), all 0 when the minimum and \
             maximum are all.equal.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the values"))
        .output(PortSpec::new("map", Type::Map, "rescaled"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        Ok(map_out(m.grid.clone(), ffi::nlm_rescale(&m.data)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescaled() {
        let v = ffi::nlm_rescale(&[2.0, 4.0, f64::NAN, 3.0]);
        assert_eq!(&v[..2], &[0.0, 1.0]);
        assert!(v[2].is_nan());
        assert_eq!(v[3], 0.5);
        assert_eq!(ffi::nlm_rescale(&[1.0, 1.0 + 1e-12]), vec![0.0, 0.0]);
    }

    #[test]
    fn classified_by_weights() {
        let v: Vec<f64> = (0..10).map(f64::from).collect();
        let c = classify(&v, None, Some(&[Some(0.3), None])).unwrap();
        assert_eq!(c, vec![1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0]);
    }
}
