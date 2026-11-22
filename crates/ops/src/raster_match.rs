//! `raster.match_histogram`: a scene brought to the radiometry of another, band by band. Each value of the source goes
//! to the value of the reference with the same cumulative share (scikit-image's `exposure.match_histograms`: the
//! source's cumulative distribution mapped onto the reference's, by linear interpolation between the distinct values),
//! so the source keeps its order and takes the reference's histogram. A `mask` limits the cells whose values make the
//! two distributions to the area the scenes share. A relative normalisation, the simple kind: it does not tell a
//! change of the ground from a change of the light (the IR-MAD of Canty et al. 2004 does, and is not here).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use crate::raster_stats::layers;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Stack, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(MatchHistogram));
}

/// numpy's `interp`: `x` on the line between the points `(xp, fp)` (`xp` increasing), the ends beyond them.
fn interp(x: f64, xp: &[f64], fp: &[f64]) -> f64 {
    let n = xp.len();
    if x.is_nan() {
        return f64::NAN;
    }
    if x < xp[0] {
        return fp[0];
    }
    if x > xp[n - 1] {
        return fp[n - 1];
    }
    if x == xp[n - 1] {
        return fp[n - 1];
    }
    let j = xp.partition_point(|&v| v <= x) - 1;
    let slope = (fp[j + 1] - fp[j]) / (xp[j + 1] - xp[j]);
    slope * (x - xp[j]) + fp[j]
}

/// The distinct values of a sample, increasing, and the cumulative share of the sample at each (`np.unique` with the
/// counts, then `cumsum(counts) / size`).
fn distribution(mut v: Vec<f64>) -> (Vec<f64>, Vec<f64>) {
    v.sort_by(f64::total_cmp);
    let n = v.len() as f64;
    let (mut values, mut counts): (Vec<f64>, Vec<f64>) = (Vec::new(), Vec::new());
    for x in v {
        match values.last() {
            Some(&l) if l == x => *counts.last_mut().unwrap() += 1.0,
            _ => {
                values.push(x);
                counts.push(1.0);
            }
        }
    }
    let mut acc = 0.0;
    let quantiles = counts
        .iter()
        .map(|c| {
            acc += c;
            acc / n
        })
        .collect();
    (values, quantiles)
}

/// One layer of the source put on the distribution of one of the reference: `source_in` and `reference_in` are the
/// values (finite) that make the two distributions; every finite value of `source` is mapped (a value the sample has
/// not got by the line between its neighbours'), the missing ones stay missing.
pub fn match_layer(source: &[f64], source_in: Vec<f64>, reference_in: Vec<f64>) -> Vec<f64> {
    if source_in.is_empty() || reference_in.is_empty() {
        return source.to_vec();
    }
    let (sv, sq) = distribution(source_in);
    let (tv, tq) = distribution(reference_in);
    source.iter().map(|&x| if x.is_finite() { interp(interp(x, &sv, &sq), &tq, &tv) } else { f64::NAN }).collect()
}

struct MatchHistogram;

impl Operator for MatchHistogram {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.match_histogram",
            "A scene brought to the radiometry of another, band by band (scikit-image's match_histograms): each \
             value of the raster takes the value of the reference that has the same cumulative share, so the raster \
             keeps its order and takes the reference's histogram. For a second date, sensor or processing before \
             a model fitted on the first classes the second: a model reads the light as it reads the ground. The \
             bands go by their names (a stack) or by their order. `mask` limits the cells that make the \
             distributions to the area both scenes show, when they are on one grid (clouds, the part of one that \
             the other lacks, what changed). A relative normalisation: it does not tell a change of the ground from \
             a change of the light.",
            Access::None,
        )
        .input(PortSpec::new("raster", Type::Any, "the scene to change: a map or a stack"))
        .input(PortSpec::new("reference", Type::Any, "the scene whose histogram to take: a map or a stack"))
        .input(PortSpec::optional(
            "mask",
            Type::Map,
            "on the raster's grid: the cells that make the distributions (non-zero), the area the two scenes share",
        ))
        .output(PortSpec::new("raster", Type::Any, "the raster with the reference's histogram, as it came"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let src = layers(args, "raster")?;
        let reference = layers(args, "reference")?;
        let mask = match args.get("mask") {
            None => None,
            Some(Value::Map(m)) if m.grid.same_as(&src.grid) => Some(m),
            Some(Value::Map(_)) => {
                return Err(Error::msg(format!("node {node:?}: the mask is not on the raster's grid")));
            }
            Some(v) => return Err(Error::msg(format!("node {node:?}: mask: a map, not a {:?}", v.ty()))),
        };
        // (the pairs: by name, else by order)
        let pairs: Vec<usize> = if src.names.iter().all(|n| reference.names.contains(n)) && src.names.len() > 1 {
            src.names.iter().map(|n| reference.names.iter().position(|r| r == n).unwrap()).collect()
        } else if src.names.len() == reference.names.len() || reference.names.len() == 1 {
            (0..src.names.len()).map(|i| if reference.names.len() == 1 { 0 } else { i }).collect()
        } else {
            return Err(Error::msg(format!(
                "node {node:?}: the raster has {} layers and the reference {}: give the layers the same names, or the \
                 same number",
                src.names.len(),
                reference.names.len()
            )));
        };
        let same = reference.grid.same_as(&src.grid);
        let inside = |i: usize| mask.is_none_or(|m| m.data[i].is_finite() && m.data[i] != 0.0);
        let mut out = Vec::new();
        for (k, &p) in pairs.iter().enumerate() {
            let (s, r) = (&src.data[k], &reference.data[p]);
            let source_in: Vec<f64> = (0..s.len()).filter(|&i| s[i].is_finite() && inside(i)).map(|i| s[i]).collect();
            let reference_in: Vec<f64> = if same {
                (0..r.len()).filter(|&i| r[i].is_finite() && inside(i)).map(|i| r[i]).collect()
            } else {
                r.iter().copied().filter(|x| x.is_finite()).collect()
            };
            if source_in.is_empty() || reference_in.is_empty() {
                return Err(Error::msg(format!(
                    "node {node:?}: layer {:?} has no cell with a value (inside the mask) to take a histogram from",
                    src.names[k]
                )));
            }
            out.push(Arc::new(match_layer(s, source_in, reference_in)));
        }
        let raster = match args.get("raster") {
            Some(Value::Stack(s)) => Value::Stack(Stack { grid: s.grid.clone(), keys: s.keys.clone(), layers: out }),
            Some(Value::Raster(r)) if r.layers.len() > 1 && !r.picked => {
                let s = r.stack()?;
                Value::Stack(Stack { grid: s.grid, keys: s.keys, layers: out })
            }
            _ => Value::Map(Map { grid: src.grid.clone(), data: out.remove(0) }),
        };
        Ok(outputs([("raster", raster)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interp_is_numpys() {
        let (xp, fp) = ([1.0, 2.0, 4.0], [10.0, 20.0, 40.0]);
        assert_eq!(interp(0.0, &xp, &fp), 10.0);
        assert_eq!(interp(3.0, &xp, &fp), 30.0);
        assert_eq!(interp(4.0, &xp, &fp), 40.0);
        assert_eq!(interp(9.0, &xp, &fp), 40.0);
    }

    #[test]
    fn the_source_takes_the_references_distribution() {
        let source = [1.0, 2.0, 3.0, 4.0, f64::NAN];
        let reference = vec![10.0, 20.0, 30.0, 40.0];
        let m = match_layer(&source, vec![1.0, 2.0, 3.0, 4.0], reference);
        assert_eq!(&m[..4], &[10.0, 20.0, 30.0, 40.0]);
        assert!(m[4].is_nan());
        // a constant source goes to the reference's largest, as scikit-image does
        assert_eq!(match_layer(&[5.0, 5.0], vec![5.0, 5.0], vec![1.0, 2.0, 9.0])[0], 9.0);
    }
}
