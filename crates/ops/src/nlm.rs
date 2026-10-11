//! Neutral landscape models from landschaft's port of NLMR (`nlm.random_cluster`, `nlm.gaussian_field`,
//! `nlm.fbm`): maps of known structure, drawn with R's generator, to try a model on before real data (the
//! templates of the application start from them) or to compare a landscape with what chance would give.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(RandomCluster));
    r.add(Arc::new(GaussianField));
    r.add(Arc::new(Fbm));
}

/// The settings every model has: its size (or another map's grid), cell size and seed.
fn sized(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::optional("like", Type::Map, "a map whose grid to draw on (else rows x columns)"))
        .param(PortSpec::optional("rows", Type::Integer, "rows (default 256)"))
        .param(PortSpec::optional("columns", Type::Integer, "columns (default 256)"))
        .param(PortSpec::optional("resolution", Type::Number, "the cells' side (default 1)"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of R's generator (default 1): NLMR's map for it"))
}

/// The grid to draw on and the seed.
fn grid(args: &Args) -> Result<(Arc<Grid>, i32)> {
    let seed: i64 = args.param("seed", 1)?;
    let seed = i32::try_from(seed).map_err(|_| Error::msg(format!("the seed {seed} does not fit 32 bits")))?;
    if let Some(Value::Map(m)) = args.get("like") {
        return Ok((m.grid.clone(), seed));
    }
    let rows: usize = args.param("rows", 256)?;
    let columns: usize = args.param("columns", 256)?;
    if rows == 0 || columns == 0 || rows * columns > 100_000_000 {
        return Err(Error::msg(format!("node {:?}: from 1 to 100 million cells", args.node)));
    }
    let res: f64 = args.param("resolution", 1.0)?;
    let mut g = Grid::new(columns, rows);
    g.transform = [0.0, res, 0.0, rows as f64 * res, 0.0, -res];
    Ok((Arc::new(g), seed))
}

/// (x - min) / (max - min) over the values (NLMR's `util_rescale`): 0 when they are all the same.
fn rescale(v: &mut [f64]) {
    let (lo, hi) =
        v.iter().filter(|x| !x.is_nan()).fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
    for x in v.iter_mut().filter(|x| !x.is_nan()) {
        *x = if hi > lo { (*x - lo) / (hi - lo) } else { 0.0 };
    }
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
        .param(PortSpec::optional("neighbourhood", Type::Integer, "of a cluster's cells: 4 (default) or 8"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (grid, seed) = grid(args)?;
        let p: f64 = args.param("p", 0.5)?;
        let weights: Vec<f64> = args.param("weights", vec![0.5, 0.5])?;
        let neighbourhood: i32 = args.param("neighbourhood", 4)?;
        let v = ffi::nlm_random_cluster(seed, grid.height as i64, grid.width as i64, p, &weights, neighbourhood)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        Ok(outputs([("map", Value::Map(Map::new(grid, v)))]))
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
        let mut v = ffi::nlm_gaussian_field(seed, grid.height as i64, grid.width as i64, range, var, nug, 0.5)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        rescale(&mut v);
        Ok(outputs([("map", Value::Map(Map::new(grid, v)))]))
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
        let mut v = ffi::nlm_fbm(seed, grid.height as i64, grid.width as i64, dim)
            .map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        rescale(&mut v);
        Ok(outputs([("map", Value::Map(Map::new(grid, v)))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rescaled() {
        let mut v = vec![2.0, 4.0, f64::NAN, 3.0];
        rescale(&mut v);
        assert_eq!(&v[..2], &[0.0, 1.0]);
        assert!(v[2].is_nan());
        assert_eq!(v[3], 0.5);
    }
}
