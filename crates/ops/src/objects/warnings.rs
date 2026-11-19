//! The warnings of object-based classification that need the data: the image (no coordinate system, bands in very
//! different scales, reflectance in 0 to 10 000), the samples (few, missing, unbalanced, objects of another size than
//! the polygons) and the attributes (two almost equal). Each says what to do; the static advice is the app's.

use std::sync::Arc;

use terraflux_raster::Grid;

use crate::Ctx;

/// The least samples of a class a classifier should have.
const FEW: usize = 10;

/// Values taken from a band to tell its scale (an even stride keeps the order of the cells).
const SAMPLE: usize = 50_000;

fn quantile(sorted: &[f64], q: f64) -> f64 {
    sorted[((sorted.len() - 1) as f64 * q).round() as usize]
}

/// The finite values of a band (at most `SAMPLE` of them, evenly), sorted.
fn sorted_sample(v: &[f64]) -> Vec<f64> {
    let stride = (v.len() / SAMPLE).max(1);
    let mut s: Vec<f64> = v.iter().step_by(stride).copied().filter(|x| x.is_finite()).collect();
    s.sort_by(f64::total_cmp);
    s
}

/// Whether a grid knows its coordinate system.
fn has_crs(g: &Grid) -> bool {
    g.geo.geokeys.is_some() || g.geo.wkt.is_some()
}

/// The image of a segmentation or of the attributes: no coordinate system, bands of very different scales (when
/// `standardised` is false: a standardised image is on one scale already) and reflectance as 0 to 10 000.
pub(super) fn image(
    ctx: &mut Ctx,
    node: &str,
    grid: &Grid,
    names: &[String],
    data: &[Arc<Vec<f64>>],
    standardised: bool,
) {
    if !has_crs(grid) {
        ctx.warn(format!(
            "node {node:?}: the image has no coordinate system: areas and distances are in cell units and samples in \
             map coordinates will not fall on it; read the image from a georeferenced file"
        ));
    }
    let bands: Vec<(usize, Vec<f64>)> =
        data.iter().enumerate().map(|(i, d)| (i, sorted_sample(d))).filter(|(_, s)| !s.is_empty()).collect();
    if bands.is_empty() {
        return;
    }
    if !standardised {
        let p99: Vec<(usize, f64)> =
            bands.iter().map(|(i, s)| (*i, quantile(s, 0.99))).filter(|(_, p)| *p > 0.0).collect();
        let hi = p99.iter().max_by(|a, b| a.1.total_cmp(&b.1));
        let lo = p99.iter().min_by(|a, b| a.1.total_cmp(&b.1));
        if let (Some(&(h, ph)), Some(&(l, pl))) = (hi, lo)
            && ph / pl > 100.0
        {
            ctx.warn(format!(
                "node {node:?}: the bands are in very different scales (99th percentile {ph} in {:?} against {pl} in \
                 {:?}): normalise the reflectance (divide each band by its scale, or raster.standardize) so that no \
                 band weighs more than the others",
                names[h], names[l]
            ));
        }
    }
    // most of the cells of most of the bands above 1.5: not a 0 to 1 reflectance
    let high = bands.iter().filter(|(_, s)| quantile(s, 0.5) > 1.5 && s[0] >= 0.0).count();
    if 2 * high > bands.len() {
        ctx.warn(format!(
            "node {node:?}: the image looks like reflectance in 0 to 10 000 (most cells are above 1.5): if it is, \
             divide it by 10 000 (raster.calc) so that the thresholds and the indices mean what they should"
        ));
    }
}

/// The class names with the number of sample objects of each (zero included): few samples, none, very unbalanced.
pub(super) fn counts(ctx: &mut Ctx, op: &str, node: &str, counts: &[(String, usize)]) {
    for (name, n) in counts {
        if *n == 0 {
            ctx.warn(format!(
                "{op}: node {node:?}: class {name:?} has no sample object: draw samples of it, or leave it out of the classes"
            ));
        } else if *n < FEW {
            ctx.warn(format!(
                "{op}: node {node:?}: class {name:?} has {n} sample object(s), under {FEW}: draw more samples of it \
                 (or segment with a smaller scale for more objects)"
            ));
        }
    }
    let have: Vec<&(String, usize)> = counts.iter().filter(|c| c.1 > 0).collect();
    if let (Some(big), Some(small)) = (have.iter().max_by_key(|c| c.1), have.iter().min_by_key(|c| c.1))
        && big.1 > FEW * small.1
    {
        ctx.warn(format!(
            "{op}: node {node:?}: the classes are very unbalanced ({:?} has {} sample objects, {:?} has {}): draw more \
             samples of {:?}, or fewer of {:?}",
            big.0, big.1, small.0, small.1, small.0, big.0
        ));
    }
}

fn median(v: &mut [f64]) -> f64 {
    v.sort_by(f64::total_cmp);
    let n = v.len();
    if n % 2 == 1 { v[n / 2] } else { (v[n / 2 - 1] + v[n / 2]) / 2.0 }
}

/// The sample objects (their cells) against the sample polygons (the cells inside each): much larger objects mix
/// what the polygons say apart, much smaller ones are pieces of what the polygons cover.
pub(super) fn sizes(ctx: &mut Ctx, node: &str, objects: &mut [f64], polygons: &mut [f64]) {
    if objects.is_empty() || polygons.is_empty() {
        return;
    }
    let (o, p) = (median(objects), median(polygons));
    if o > 4.0 * p {
        ctx.warn(format!(
            "objects.samples: node {node:?}: the sample objects are much larger than the sample polygons (median {o} \
             cells against {p}): they mix what the polygons tell apart; segment with a smaller scale"
        ));
    } else if o < p / 4.0 {
        ctx.warn(format!(
            "objects.samples: node {node:?}: the sample objects are much smaller than the sample polygons (median {o} \
             cells against {p}): they are pieces of the samples; segment with a larger scale"
        ));
    }
}

/// Pairs of columns with |Pearson r| above 0.98 over the objects: the names, said for one to be dropped.
pub(super) fn correlated(ctx: &mut Ctx, op: &str, node: &str, names: &[String], columns: &[Vec<f64>]) {
    let mut pairs: Vec<(usize, usize, f64)> = Vec::new();
    // (cells and area are the same number by a constant: always, and no news)
    let size = |a: &str, b: &str| (a == "cells" && b == "area") || (a == "area" && b == "cells");
    for a in 0..columns.len() {
        for b in a + 1..columns.len() {
            if size(&names[a], &names[b]) {
                continue;
            }
            if let Some(r) = pearson(&columns[a], &columns[b])
                && r.abs() > 0.98
            {
                pairs.push((a, b, r));
            }
        }
    }
    for &(a, b, r) in pairs.iter().take(5) {
        ctx.warn(format!(
            "{op}: node {node:?}: the attributes {:?} and {:?} are almost equal (r = {r:.3}): drop one",
            names[a], names[b]
        ));
    }
    if pairs.len() > 5 {
        ctx.warn(format!(
            "{op}: node {node:?}: {} more pairs of almost equal attributes: drop one of each",
            pairs.len() - 5
        ));
    }
}

/// Pearson's r over the pairs with both values (an even sample of them when many); none for under 3 pairs or a
/// column that does not vary.
fn pearson(a: &[f64], b: &[f64]) -> Option<f64> {
    let stride = (a.len() / 20_000).max(1);
    let (mut n, mut sa, mut sb) = (0.0, 0.0, 0.0);
    let pairs: Vec<(f64, f64)> = a
        .iter()
        .zip(b)
        .step_by(stride)
        .filter(|(x, y)| x.is_finite() && y.is_finite())
        .map(|(x, y)| (*x, *y))
        .collect();
    for (x, y) in &pairs {
        n += 1.0;
        sa += x;
        sb += y;
    }
    if n < 3.0 {
        return None;
    }
    let (ma, mb) = (sa / n, sb / n);
    let (mut saa, mut sbb, mut sab) = (0.0, 0.0, 0.0);
    for (x, y) in &pairs {
        saa += (x - ma) * (x - ma);
        sbb += (y - mb) * (y - mb);
        sab += (x - ma) * (y - mb);
    }
    if saa <= 0.0 || sbb <= 0.0 {
        return None;
    }
    Some(sab / (saa * sbb).sqrt())
}
