//! `segment.evaluate`: a segmentation measured against reference objects (polygons drawn, or objects made elsewhere).
//!
//! One pass over the cells builds the overlap table, in cells, of each reference and each object it shares cells with.
//! The measures are those of the R package `segmetric` (Costa et al.), which collects them from the literature, each
//! over the pairs it defines (a subset of the overlap table), and averaged over the pairs as `segmetric`'s `summary`
//! does:
//!
//! - the pairs `Y*` of Clinton et al. (2010): a reference and an object that overlap and either hold the other's
//!   centroid (on its edge too), or share more than half of the object or of the reference. Over them: OS (over
//!   segmentation) = 1 - overlap / reference, US (under) = 1 - overlap / object, D = sqrt((OS^2 + US^2) / 2) and QR (Weidner
//!   2008) = 1 - overlap / union;
//! - the pairs `Y'` of each reference and the object it shares the most cells with (all of them on a tie): AFI (Lucieer
//!   and Stein 2002) = (reference - object) / reference, and the recall, the sum of the overlaps over the sum of the
//!   references;
//! - the pairs `X'` of each object and the reference it shares the most with: the precision, the sum of the overlaps
//!   over the sum of the objects; and the F of the two (Zhang et al. 2015), 2 p r / (p + r).
//!
//! `segmetric` has no Liu et al. (2012) measures, so PSE, NSR and ED2 follow the paper as read here: the objects that
//! correspond to a reference are those with more than half of their cells in it (the object it shares the most with
//! when there are none); PSE is the cells of those objects outside the reference over all the reference cells, NSR is
//! |m - v| / m for m references and v corresponding objects, and ED2 = sqrt(PSE^2 + NSR^2).
//!
//! An area is a count of cells, which is exact for references made of whole cells (those a grid's segmentation gives); a
//! polygon is laid on the grid as `segment.from_vector` does, a cell to the polygon holding its centre.

use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;
use terraflux_raster::vector::Vector;

use super::{Objects, objects_in};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Evaluate));
}

/// The metrics a call can ask for, in the order of the summary.
pub const METRICS: [&str; 11] = ["os", "us", "d", "qr", "afi", "precision", "recall", "f_overlap", "pse", "nsr", "ed2"];

/// The reference objects laid on a grid: the reference (1..count, 0 for none) of each cell, its cells, and the sums of
/// the doubled coordinates of its cells' centres (to find its centroid exactly).
pub(crate) struct Reference {
    pub labels: Vec<u32>,
    pub count: usize,
    width: usize,
    height: usize,
    cells: Vec<usize>,
    sx: Vec<u64>,
    sy: Vec<u64>,
}

impl Reference {
    /// The reference whose cell `i` is `labels[i]` (0 none), `count` references.
    pub fn from_labels(labels: Vec<u32>, count: usize, grid: &Grid) -> Reference {
        let (width, height) = (grid.width, grid.height);
        let mut cells = vec![0usize; count];
        let (mut sx, mut sy) = (vec![0u64; count], vec![0u64; count]);
        for (i, &l) in labels.iter().enumerate() {
            if l > 0 {
                let k = l as usize - 1;
                cells[k] += 1;
                sx[k] += 2 * (i % width) as u64 + 1;
                sy[k] += 2 * (i / width) as u64 + 1;
            }
        }
        Reference { labels, count, width, height, cells, sx, sy }
    }

    /// The polygons laid on the grid of `grid` (the later polygon winning where they overlap): a reference each.
    pub fn from_vector(node: &str, v: &Arc<Vector>, grid: &Arc<Grid>) -> Result<Reference> {
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let v = crate::vector::on_grid(v, grid).map_err(|e| bad(e.0))?;
        if v.is_empty() {
            return Err(bad("the reference has no polygon".into()));
        }
        if let Some(k) = v.geometries.iter().position(|g| !g.is_polygonal()) {
            return Err(bad(format!("reference: feature {} is not a polygon", k + 1)));
        }
        let mut burnt = vec![f64::NAN; grid.cells()];
        let values: Vec<f64> = (1..=v.len()).map(|k| k as f64).collect();
        crate::vector::burn(&mut burnt, grid, &v, &values, false, false).map_err(|e| bad(e.0))?;
        let labels = burnt.iter().map(|&x| if x.is_nan() { 0 } else { x as u32 }).collect();
        Ok(Reference::from_labels(labels, v.len(), grid))
    }
}

/// The measures of one segmentation against a reference, averaged as `segmetric`'s summary does (NaN where a
/// measure has no pair).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Summary {
    pub os: f64,
    pub us: f64,
    pub d: f64,
    pub qr: f64,
    pub afi: f64,
    pub precision: f64,
    pub recall: f64,
    pub f_overlap: f64,
    pub pse: f64,
    pub nsr: f64,
    pub ed2: f64,
    /// The references that have an object they match (a pair of Clinton et al.'s).
    pub matched: usize,
}

impl Summary {
    /// A measure by its name.
    pub fn get(&self, name: &str) -> f64 {
        match name {
            "os" => self.os,
            "us" => self.us,
            "d" => self.d,
            "qr" => self.qr,
            "afi" => self.afi,
            "precision" => self.precision,
            "recall" => self.recall,
            "f_overlap" => self.f_overlap,
            "pse" => self.pse,
            "nsr" => self.nsr,
            "ed2" => self.ed2,
            _ => f64::NAN,
        }
    }
}

/// A reference and an object that share cells, with the subsets of `segmetric` they belong to.
pub(crate) struct Pair {
    pub reference: usize,
    pub object: usize,
    pub cells: usize,
    pub reference_cells: usize,
    pub object_cells: usize,
    /// In Clinton et al.'s `Y*` (OS, US, D and QR are over these).
    pub star: bool,
    /// The object the reference shares the most with (`Y'`: AFI and the recall).
    pub best_for_reference: bool,
    /// The reference the object shares the most with (`X'`: the precision).
    pub best_for_object: bool,
}

impl Pair {
    pub fn os(&self) -> f64 {
        1.0 - self.cells as f64 / self.reference_cells as f64
    }
    pub fn us(&self) -> f64 {
        1.0 - self.cells as f64 / self.object_cells as f64
    }
    pub fn d(&self) -> f64 {
        ((self.os().powi(2) + self.us().powi(2)) / 2.0).sqrt()
    }
    pub fn qr(&self) -> f64 {
        1.0 - self.cells as f64 / (self.reference_cells + self.object_cells - self.cells) as f64
    }
    pub fn afi(&self) -> f64 {
        (self.reference_cells as f64 - self.object_cells as f64) / self.reference_cells as f64
    }
}

/// What an evaluation gives: the pairs (those of any subset), the summary and, per object, its OS and US against the
/// reference it shares the most with (NaN for an object with none).
pub(crate) struct Evaluation {
    pub pairs: Vec<Pair>,
    pub summary: Summary,
    pub per_object: Vec<(f64, f64)>,
}

/// The labels of the cells that hold the point (`tx`, `ty` are its coordinates times `2n`, the sums of the doubled
/// coordinates of `n` cells): one cell, or two or four where it is on an edge or a corner. Cells outside the grid
/// are left out.
fn cells_at(tx: u64, ty: u64, n: u64, width: usize, height: usize, labels: &[u32]) -> Vec<u32> {
    let span = |t: u64| -> Vec<u64> {
        let d = 2 * n;
        if t.is_multiple_of(d) {
            let m = t / d;
            if m == 0 { vec![0] } else { vec![m - 1, m] }
        } else {
            vec![t / d]
        }
    };
    let mut out = Vec::new();
    for &r in &span(ty) {
        for &c in &span(tx) {
            if (c as usize) < width && (r as usize) < height {
                let l = labels[r as usize * width + c as usize];
                if l > 0 && !out.contains(&l) {
                    out.push(l);
                }
            }
        }
    }
    out
}

/// Evaluate the objects against the reference, laid on the same grid.
pub(crate) fn evaluate(r: &Reference, o: &Objects) -> Evaluation {
    let (width, height) = (r.width, r.height);
    let ns = o.count;
    // the overlap table, in one pass
    let mut overlap: HashMap<u64, usize> = HashMap::new();
    for (&a, &b) in r.labels.iter().zip(o.labels.iter()) {
        if a > 0 && b > 0 {
            *overlap.entry(a as u64 * (ns as u64 + 1) + b as u64).or_default() += 1;
        }
    }
    let mut table: Vec<(usize, usize, usize)> = overlap
        .into_iter()
        .map(|(k, n)| ((k / (ns as u64 + 1)) as usize - 1, (k % (ns as u64 + 1)) as usize - 1, n))
        .collect();
    table.sort_unstable();
    let seg_cells: Vec<usize> = o.cells.iter().map(|&c| c as usize).collect();
    // the centroids of the objects (sums of the doubled coordinates of their cells' centres)
    let (mut ox, mut oy) = (vec![0u64; ns], vec![0u64; ns]);
    for (i, &l) in o.labels.iter().enumerate() {
        if l > 0 {
            ox[l as usize - 1] += 2 * (i % width) as u64 + 1;
            oy[l as usize - 1] += 2 * (i / width) as u64 + 1;
        }
    }
    // the most each reference and each object share with another
    let mut best_r = vec![0usize; r.count];
    let mut best_s = vec![0usize; ns];
    for &(a, b, n) in &table {
        best_r[a] = best_r[a].max(n);
        best_s[b] = best_s[b].max(n);
    }
    // the objects that hold a reference's centroid (and the references that hold an object's), as the points are
    // on their edges too
    let holds_r: Vec<Vec<u32>> = (0..r.count)
        .map(|a| {
            if r.cells[a] == 0 {
                Vec::new()
            } else {
                cells_at(r.sx[a], r.sy[a], r.cells[a] as u64, width, height, &o.labels)
            }
        })
        .collect();
    let mut pairs = Vec::new();
    let mut first_best: Vec<Option<usize>> = vec![None; ns];
    for &(a, b, n) in &table {
        let (rc, sc) = (r.cells[a], seg_cells[b]);
        let by_ref = holds_r[a].contains(&(b as u32 + 1));
        let by_seg = {
            let seg = cells_at(ox[b], oy[b], sc as u64, width, height, &r.labels);
            seg.contains(&(a as u32 + 1))
        };
        let star = by_ref || by_seg || n * 2 > sc || n * 2 > rc;
        let best_for_reference = n == best_r[a];
        let best_for_object = n == best_s[b];
        if best_for_object && first_best[b].is_none() {
            first_best[b] = Some(pairs.len());
        }
        if star || best_for_reference || best_for_object {
            pairs.push(Pair {
                reference: a,
                object: b,
                cells: n,
                reference_cells: rc,
                object_cells: sc,
                star,
                best_for_reference,
                best_for_object,
            });
        }
    }
    // the averages over the pairs of each subset
    let mean = |f: &dyn Fn(&Pair) -> f64, keep: &dyn Fn(&Pair) -> bool| {
        let v: Vec<f64> = pairs.iter().filter(|p| keep(p)).map(f).collect();
        if v.is_empty() { f64::NAN } else { v.iter().sum::<f64>() / v.len() as f64 }
    };
    let star = |p: &Pair| p.star;
    let (os, us, d, qr) =
        (mean(&Pair::os, &star), mean(&Pair::us, &star), mean(&Pair::d, &star), mean(&Pair::qr, &star));
    let afi = mean(&Pair::afi, &|p| p.best_for_reference);
    let sum = |f: &dyn Fn(&Pair) -> usize, keep: &dyn Fn(&Pair) -> bool| -> f64 {
        pairs.iter().filter(|p| keep(p)).map(f).sum::<usize>() as f64
    };
    let precision = sum(&|p| p.cells, &|p| p.best_for_object) / sum(&|p| p.object_cells, &|p| p.best_for_object);
    let recall = sum(&|p| p.cells, &|p| p.best_for_reference) / sum(&|p| p.reference_cells, &|p| p.best_for_reference);
    let f_overlap = 1.0 / (0.5 / precision + 0.5 / recall);
    // Liu et al.: the objects that correspond to each reference
    let (mut outside, mut corresponding) = (0usize, 0usize);
    let mut with_overlap = vec![false; r.count];
    let mut mostly: Vec<Vec<(usize, usize, usize)>> = vec![Vec::new(); r.count];
    let mut best: Vec<Option<(usize, usize, usize)>> = vec![None; r.count];
    for &(a, b, n) in &table {
        with_overlap[a] = true;
        if n * 2 > seg_cells[b] {
            mostly[a].push((b, n, seg_cells[b]));
        }
        if best[a].is_none_or(|(_, m, _)| n > m) {
            best[a] = Some((b, n, seg_cells[b]));
        }
    }
    for a in 0..r.count {
        let chosen: Vec<(usize, usize, usize)> =
            if mostly[a].is_empty() { best[a].into_iter().collect() } else { mostly[a].clone() };
        for (_, n, sc) in chosen {
            outside += sc - n;
            corresponding += 1;
        }
    }
    let total: usize = r.cells.iter().sum();
    let pse = outside as f64 / total as f64;
    let nsr = (r.count as f64 - corresponding as f64).abs() / r.count as f64;
    let ed2 = (pse * pse + nsr * nsr).sqrt();
    let matched = {
        let mut seen = vec![false; r.count];
        pairs.iter().filter(|p| p.star).for_each(|p| seen[p.reference] = true);
        seen.iter().filter(|&&b| b).count()
    };
    // per object: against the reference it shares the most with (the first of them on a tie)
    let per_object = (0..ns)
        .map(|b| match first_best[b] {
            Some(k) => (pairs[k].os(), pairs[k].us()),
            None => (f64::NAN, f64::NAN),
        })
        .collect();
    let summary = Summary { os, us, d, qr, afi, precision, recall, f_overlap, pse, nsr, ed2, matched };
    Evaluation { pairs, summary, per_object }
}

/// The reference of a node: polygons laid on the grid of `grid`, or objects of that grid.
pub(crate) fn reference_of(node: &str, value: &Value, grid: &Arc<Grid>) -> Result<Reference> {
    match value {
        Value::Vector(v) => Reference::from_vector(node, v, grid),
        Value::Objects(r) => {
            if r.grid.width != grid.width || r.grid.height != grid.height {
                return Err(Error::msg(format!(
                    "node {node:?}: the reference objects are on a grid of {} x {} cells, the objects on one of {} x {}",
                    r.grid.width, r.grid.height, grid.width, grid.height
                )));
            }
            Ok(Reference::from_labels(r.labels.iter().copied().collect(), r.count, grid))
        }
        v => Err(Error::msg(format!("node {node:?}: reference: polygons or objects, not a {:?}", v.ty()))),
    }
}

/// `segment.evaluate`.
struct Evaluate;

impl Operator for Evaluate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.evaluate",
            "A segmentation measured against reference objects (polygons drawn by hand, or objects made another way), \
             as the R package segmetric measures it. The overlap of each reference with each object is counted in \
             cells in one pass. Over the pairs of Clinton et al. 2010 (a reference and an object that hold each \
             other's centroid or share more than half of one of them): os, the over-segmentation, 1 - overlap / \
             reference; us, the under-segmentation, 1 - overlap / object; d, sqrt((os^2 + us^2) / 2) and qr, \
             Weidner's quality rate, 1 - overlap / union. Over each reference and the object it shares the most with \
             (Lucieer and Stein 2002): afi, the area fit index (reference - object) / reference, and the recall; over \
             each object and its best reference, the precision; f_overlap is their F (Zhang et al. 2015). pse, nsr \
             and ed2 follow Liu et al. 2012 (their objects are those with over half their cells in the reference): \
             pse the cells of the corresponding objects outside the reference over those of the references, nsr the \
             difference between the numbers of references and corresponding objects over the first, and ed2 sqrt(pse^2 \
             + nsr^2). All are 0 for a segmentation equal to the reference, except f_overlap, which is 1. A polygon is \
             laid on the grid as segment.from_vector does it (a cell to the polygon holding its centre), so the areas \
             are exact for references made of whole cells. `summary` averages each measure over its pairs, as \
             segmetric's summary does.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the segmentation to measure"))
        .input(PortSpec::new(
            "reference",
            Type::Any,
            "the reference: polygons (a vector, in the objects' coordinate system or reprojected), or objects on the same grid",
        ))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per pair of a reference and an object that is in a subset of the measures: reference, object, cells \
             (the overlap), reference_cells, object_cells, matched (a pair of Clinton et al.: os, us, d and qr are \
             averaged over these), best_for_reference (afi and the recall), best_for_object (the precision), and os, us, d, \
             qr and afi of the pair",
        ))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "a row: references, objects, matched (references with a matching object) and the measures asked for",
        ))
        .output(PortSpec::new(
            "objects",
            Type::Objects,
            "the objects with the columns os and us against the reference each shares the most with (NaN where none), to \
             show on a map where the segmentation errs by excess (os) or by lack (us)",
        ))
        .param(PortSpec::optional(
            "metrics",
            Type::Any,
            "the measures (a list of: os, us, d, qr, afi, precision, recall, f_overlap, pse, nsr, ed2; default all)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let metrics: Vec<String> = match args.opt_param::<Vec<String>>("metrics")? {
            Some(m) => m,
            None => METRICS.iter().map(|s| s.to_string()).collect(),
        };
        if let Some(m) = metrics.iter().find(|m| !METRICS.contains(&m.as_str())) {
            return Err(bad(format!("metric {m:?}: one of {}", METRICS.join(", "))));
        }
        if metrics.is_empty() {
            return Err(bad("metrics: at least one".into()));
        }
        let value = args.get("reference").ok_or_else(|| bad("connect the reference (polygons or objects)".into()))?;
        let reference = reference_of(node, value, &o.grid)?;
        let e = evaluate(&reference, o);
        let want = |n: &str| metrics.iter().any(|m| m == n);
        // the pairs
        let mut head: Vec<String> = [
            "reference",
            "object",
            "cells",
            "reference_cells",
            "object_cells",
            "matched",
            "best_for_reference",
            "best_for_object",
        ]
        .map(String::from)
        .to_vec();
        let per_pair = ["os", "us", "d", "qr", "afi"];
        head.extend(per_pair.iter().filter(|n| want(n)).map(|n| n.to_string()));
        let mut table = Table::new(head);
        for p in &e.pairs {
            let mut row = vec![
                (p.reference + 1) as f64,
                (p.object + 1) as f64,
                p.cells as f64,
                p.reference_cells as f64,
                p.object_cells as f64,
                f64::from(p.star),
                f64::from(p.best_for_reference),
                f64::from(p.best_for_object),
            ];
            for n in per_pair.iter().filter(|n| want(n)) {
                row.push(match *n {
                    "os" => p.os(),
                    "us" => p.us(),
                    "d" => p.d(),
                    "qr" => p.qr(),
                    _ => p.afi(),
                });
            }
            table.rows.push(row);
        }
        // the summary
        let mut head = vec!["references".to_string(), "objects".into(), "matched".into()];
        head.extend(METRICS.iter().filter(|n| want(n)).map(|n| n.to_string()));
        let mut summary = Table::new(head);
        let mut row = vec![reference.count as f64, o.count as f64, e.summary.matched as f64];
        row.extend(METRICS.iter().filter(|n| want(n)).map(|n| e.summary.get(n)));
        summary.rows.push(row);
        // the objects with their os and us
        let mut t = o.table.clone();
        for (k, name) in ["os", "us"].iter().enumerate() {
            let values: Vec<f64> = e.per_object.iter().map(|v| if k == 0 { v.0 } else { v.1 }).collect();
            match t.columns.iter().position(|c| c == name) {
                Some(j) => t.rows.iter_mut().zip(&values).for_each(|(r, v)| r[j] = *v),
                None => {
                    t.columns.push(name.to_string());
                    t.rows.iter_mut().zip(&values).for_each(|(r, v)| r.push(*v));
                }
            }
        }
        let marked = o.as_ref().clone().with_table(t).map_err(|e| bad(e.0))?;
        let mut out = outputs([("table", Value::Table(table)), ("summary", Value::Table(summary))]);
        out.insert("objects".into(), Value::Objects(Arc::new(marked)));
        Ok(out)
    }
}
