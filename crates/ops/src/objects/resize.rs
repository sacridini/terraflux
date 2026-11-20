//! `objects.resize`: objects grown, shrunk and smoothed cell by cell, as eCognition's pixel-based object resizing, its
//! morphology (opening and closing), its filling of holes and its border optimization.
//!
//! Without a condition on the cells (a `mask`, or classes whose cells may be taken) growing and shrinking are exact and
//! Euclidean, in one pass: growing is the transform of `linkage::edt` with the chosen objects' cells as sites (each free
//! cell takes the object nearest to it when that is within the distance); shrinking is the same transform per object,
//! in its box, from the cells that are not its own (running the objects in parallel, [`super::each`]). With a condition
//! the growth is geodesic, ring by ring: a cell goes to the object with most cells around it, a tie to the lower id, all
//! the cells of a ring at once (two buffers, rows in parallel). `open` is shrink and then grow back over the cells the
//! object had; `close` is grow and then shrink only the cells that were added; `fill` takes the holes of free cells
//! that one object encloses; `smooth` is the mode of the labels in a disc; `refine` moves each border cell to the
//! adjacent object whose band means are closest, in passes that read the means of the pass before. An object cut in
//! two stays two objects. The same objects on any number of threads.

use std::cmp::Reverse;
use std::collections::BTreeSet;
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::each::{Frame, each_with};
use super::levels::class_numbers;
use super::{NO_REGION, Objects, big, objects_in, stats_table};
use crate::linkage::{Lattice, edt};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Resize));
}

const N4: [(i64, i64); 4] = [(-1, 0), (0, -1), (0, 1), (1, 0)];
const N8: [(i64, i64); 8] = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)];

fn around(eight: bool) -> &'static [(i64, i64)] {
    if eight { &N8 } else { &N4 }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Grow,
    Shrink,
    Open,
    Close,
    Fill,
    Smooth,
    Refine,
}

/// The grid the labels are on.
#[derive(Clone, Copy)]
struct Geo {
    w: usize,
    h: usize,
    eight: bool,
    lat: Lattice,
}

/// How far an exact transform reaches: a hair more than the distance, for the rounding of the square roots.
fn limit(d: f64) -> f64 {
    d * (1.0 + 1e-9)
}

/// The cells of the chosen objects as the sites of an exact transform: each free cell within `d` of one takes the
/// object nearest to it.
fn grow_exact(lab: &mut [u32], chosen: &[bool], d: f64, g: &Geo) {
    let sites: Vec<u32> = lab.iter().map(|&l| if l != 0 && chosen[l as usize] { l } else { 0 }).collect();
    let (dist, near) = edt(&sites, &g.lat);
    let lim = limit(d);
    for i in 0..lab.len() {
        if lab[i] == 0 && near[i] != 0 && dist[i] <= lim {
            lab[i] = near[i];
        }
    }
}

/// Growth ring by ring over the candidate cells (`cand`, which loses each cell as it is taken): a cell takes the
/// chosen object with most cells around it (a tie, the lower id). Rows in parallel, reading one buffer and writing the
/// other.
fn grow_geo(lab: &mut [u32], chosen: &[bool], cand: &mut [bool], steps: usize, g: &Geo) {
    let (w, h) = (g.w, g.h);
    let offs = around(g.eight);
    let mut next = lab.to_vec();
    for _ in 0..steps {
        let cur: &[u32] = lab;
        let changed: usize = next
            .par_chunks_mut(w)
            .zip(cand.par_chunks_mut(w))
            .enumerate()
            .map(|(r, (row, cd))| {
                let mut n = 0;
                for c in 0..w {
                    if !cd[c] {
                        continue;
                    }
                    let mut seen = [(0u32, 0u8); 8];
                    let mut k = 0;
                    for &(dr, dc) in offs {
                        let (rr, cc) = (r as i64 + dr, c as i64 + dc);
                        if rr < 0 || cc < 0 || rr >= h as i64 || cc >= w as i64 {
                            continue;
                        }
                        let l = cur[rr as usize * w + cc as usize];
                        if l == 0 || !chosen[l as usize] {
                            continue;
                        }
                        match seen[..k].iter_mut().find(|s| s.0 == l) {
                            Some(s) => s.1 += 1,
                            None => {
                                seen[k] = (l, 1);
                                k += 1;
                            }
                        }
                    }
                    if k > 0 {
                        row[c] = seen[..k].iter().max_by_key(|&&(l, n)| (n, Reverse(l))).map_or(0, |s| s.0);
                        cd[c] = false;
                        n += 1;
                    }
                }
                n
            })
            .sum();
        if changed == 0 {
            break;
        }
        lab.copy_from_slice(&next);
    }
}

/// The cells of the chosen objects that lie within `d` of a cell that is not their object's (another object, no
/// object): each object's own transform in its box, the objects in parallel. The image's edge is a border only when
/// `edge` says so (the box then has a pad as wide as the distance, for the cells that far from the edge).
fn shrink_cells(lab: &[u32], chosen: &[bool], d: f64, edge: bool, g: &Geo) -> Vec<usize> {
    let count = chosen.len() - 1;
    let mut boxes = vec![[u32::MAX, u32::MAX, 0, 0]; count + 1];
    for (i, &l) in lab.iter().enumerate() {
        if l != 0 && chosen[l as usize] {
            let (r, c) = ((i / g.w) as u32, (i % g.w) as u32);
            let b = &mut boxes[l as usize];
            *b = [b[0].min(r), b[1].min(c), b[2].max(r), b[3].max(c)];
        }
    }
    let ids: Vec<u32> = (1..=count as u32).filter(|&id| boxes[id as usize][0] != u32::MAX).collect();
    let pad = if edge { (d / g.lat.dx.min(g.lat.dy)).ceil() as i64 + 1 } else { 1 };
    let frames: Vec<Frame> = ids
        .iter()
        .map(|&id| {
            let [r0, c0, r1, c1] = boxes[id as usize];
            Frame {
                top: r0 as i64 - pad,
                left: c0 as i64 - pad,
                w: (c1 - c0) as usize + 1 + 2 * pad as usize,
                h: (r1 - r0) as usize + 1 + 2 * pad as usize,
            }
        })
        .collect();
    let lim = limit(d);
    let found: Vec<Vec<usize>> = each_with(&frames, |k, f| {
        let id = ids[k];
        let n = f.cells();
        let mut inside = vec![false; n];
        let mut sites = vec![0u32; n];
        let mut global = vec![usize::MAX; n];
        for i in 0..n {
            match f.index(g.w, g.h, i) {
                Some(gl) => {
                    global[i] = gl;
                    if lab[gl] == id {
                        inside[i] = true;
                    } else {
                        sites[i] = 1;
                    }
                }
                None => sites[i] = u32::from(edge),
            }
        }
        let lat = Lattice { w: f.w, h: f.h, dx: g.lat.dx, dy: g.lat.dy };
        let (dist, _) = edt(&sites, &lat);
        (0..n).filter(|&i| inside[i] && dist[i] <= lim).map(|i| global[i]).collect()
    });
    found.into_iter().flatten().collect()
}

/// The holes of the cells that may be filled (`free`) that one chosen object encloses: the connected sets of free
/// cells (by the neighbourhood the objects are not connected by) that do not touch the image's edge, have only that
/// object around them, and have at most `max_hole` cells.
fn fill(lab: &mut [u32], chosen: &[bool], free: &[bool], max_hole: usize, g: &Geo) {
    let (w, h) = (g.w, g.h);
    let offs = around(!g.eight);
    let mut seen = vec![false; w * h];
    let mut stack: Vec<usize> = Vec::new();
    let mut hole: Vec<usize> = Vec::new();
    for s in 0..w * h {
        if !free[s] || seen[s] {
            continue;
        }
        hole.clear();
        stack.push(s);
        seen[s] = true;
        let mut owner = 0u32;
        let mut open = false;
        while let Some(i) = stack.pop() {
            hole.push(i);
            let (r, c) = ((i / w) as i64, (i % w) as i64);
            for &(dr, dc) in offs {
                let (rr, cc) = (r + dr, c + dc);
                if rr < 0 || cc < 0 || rr >= h as i64 || cc >= w as i64 {
                    open = true;
                    continue;
                }
                let j = rr as usize * w + cc as usize;
                if free[j] {
                    if !seen[j] {
                        seen[j] = true;
                        stack.push(j);
                    }
                } else {
                    let l = lab[j];
                    if l == 0 || !chosen[l as usize] || (owner != 0 && owner != l) {
                        open = true;
                    } else {
                        owner = l;
                    }
                }
            }
        }
        if !open && owner != 0 && hole.len() <= max_hole {
            for &i in &hole {
                lab[i] = owner;
            }
        }
    }
}

/// The mode of the labels in a disc of radius `r` around each cell of the chosen objects (no object counts as a
/// label; a tie keeps the cell's own, else the lower id): spikes and notches go, corners round. Cells of no object stay.
fn smooth(lab: &mut [u32], chosen: &[bool], r: f64, g: &Geo) {
    let (w, h) = (g.w, g.h);
    let (rr, rc) = ((r / g.lat.dy).floor() as i64, (r / g.lat.dx).floor() as i64);
    let lim = limit(r) * limit(r);
    let mut disc: Vec<(i64, i64)> = Vec::new();
    for dr in -rr..=rr {
        for dc in -rc..=rc {
            if (dr as f64 * g.lat.dy).powi(2) + (dc as f64 * g.lat.dx).powi(2) <= lim {
                disc.push((dr, dc));
            }
        }
    }
    let cur = lab.to_vec();
    lab.par_chunks_mut(w).enumerate().for_each(|(r, row)| {
        let mut tally: Vec<(u32, u32)> = Vec::new();
        for c in 0..w {
            let own = cur[r * w + c];
            if own == 0 || !chosen[own as usize] {
                continue;
            }
            tally.clear();
            for &(dr, dc) in &disc {
                let (y, x) = (r as i64 + dr, c as i64 + dc);
                if y < 0 || x < 0 || y >= h as i64 || x >= w as i64 {
                    continue;
                }
                let l = cur[y as usize * w + x as usize];
                match tally.iter_mut().find(|t| t.0 == l) {
                    Some(t) => t.1 += 1,
                    None => tally.push((l, 1)),
                }
            }
            let top = tally.iter().map(|t| t.1).max().unwrap_or(0);
            if tally.iter().any(|t| t.0 == own && t.1 == top) {
                continue;
            }
            row[c] = tally.iter().filter(|t| t.1 == top).map(|t| t.0).min().unwrap_or(own);
        }
    });
}

/// Border optimization: in each pass a cell of a chosen object that touches another object goes to the one (its own
/// included) whose band means are closest, in z-scores of the whole image; the means are those of the pass before. Stops
/// when a pass moves nothing. A cell with a band without a value stays.
fn refine(lab: &mut [u32], chosen: &[bool], layers: &[Arc<Vec<f64>>], steps: usize, g: &Geo) {
    let (w, h) = (g.w, g.h);
    let nb = layers.len();
    let count = chosen.len() - 1;
    let offs = around(g.eight);
    let scale: Vec<(f64, f64)> = layers
        .iter()
        .map(|b| {
            let (mut s, mut s2, mut n) = (0.0f64, 0.0f64, 0usize);
            for &v in b.iter().filter(|v| v.is_finite()) {
                s += v;
                s2 += v * v;
                n += 1;
            }
            let mean = if n > 0 { s / n as f64 } else { 0.0 };
            let var = if n > 0 { (s2 / n as f64 - mean * mean).max(0.0) } else { 0.0 };
            (mean, if var > 0.0 { 1.0 / var.sqrt() } else { 0.0 })
        })
        .collect();
    let z = |b: usize, i: usize| (layers[b][i] - scale[b].0) * scale[b].1;
    let ok: Vec<bool> = (0..w * h).map(|i| layers.iter().all(|b| b[i].is_finite())).collect();
    for _ in 0..steps {
        let mut mean = vec![0.0f64; (count + 1) * nb];
        let mut n = vec![0u32; count + 1];
        for (i, &l) in lab.iter().enumerate() {
            if l == 0 || !ok[i] {
                continue;
            }
            n[l as usize] += 1;
            for b in 0..nb {
                mean[l as usize * nb + b] += z(b, i);
            }
        }
        for l in 1..=count {
            if n[l] > 0 {
                for b in 0..nb {
                    mean[l * nb + b] /= n[l] as f64;
                }
            }
        }
        let cur = lab.to_vec();
        let dist = |l: u32, i: usize| -> f64 { (0..nb).map(|b| (z(b, i) - mean[l as usize * nb + b]).powi(2)).sum() };
        let moved: usize = lab
            .par_chunks_mut(w)
            .enumerate()
            .map(|(r, row)| {
                let mut n_moved = 0;
                for (c, cell) in row.iter_mut().enumerate() {
                    let i = r * w + c;
                    let own = cur[i];
                    if own == 0 || !chosen[own as usize] || !ok[i] {
                        continue;
                    }
                    let own_d = dist(own, i);
                    let mut best: Option<(f64, u32)> = None;
                    for &(dr, dc) in offs {
                        let (y, x) = (r as i64 + dr, c as i64 + dc);
                        if y < 0 || x < 0 || y >= h as i64 || x >= w as i64 {
                            continue;
                        }
                        let m = cur[y as usize * w + x as usize];
                        if m == 0 || m == own || n[m as usize] == 0 {
                            continue;
                        }
                        let d = dist(m, i);
                        // (the closest neighbour, the lower id on a tie)
                        if best.is_none_or(|(bd, bm)| d < bd || (d == bd && m < bm)) {
                            best = Some((d, m));
                        }
                    }
                    // (it moves only when strictly closer than to its own object)
                    if let Some((_, m)) = best.filter(|&(d, _)| d < own_d) {
                        *cell = m;
                        n_moved += 1;
                    }
                }
                n_moved
            })
            .sum();
        if moved == 0 {
            break;
        }
    }
}

/// What the operator does with the labels.
struct Job<'a> {
    mode: Mode,
    chosen: &'a [bool],
    /// Per object: its cells may be taken by growth or filled over (a class of `into`, not a chosen one).
    taken: &'a [bool],
    /// The cells that may be taken (the mask), none = all.
    allowed: Option<&'a [bool]>,
    /// The distance in map units and in cells.
    d: f64,
    steps: usize,
    max_hole: usize,
    layers: Option<&'a [Arc<Vec<f64>>]>,
    g: Geo,
}

/// Grow the chosen objects: exact when no condition is on the cells, ring by ring over the candidate cells when there is.
fn grow(lab: &mut [u32], j: &Job) {
    if j.allowed.is_none() && !j.taken.iter().any(|&t| t) {
        grow_exact(lab, j.chosen, j.d, &j.g);
    } else {
        let mut cand: Vec<bool> = (0..lab.len())
            .map(|i| (lab[i] == 0 || j.taken[lab[i] as usize]) && j.allowed.is_none_or(|a| a[i]))
            .collect();
        grow_geo(lab, j.chosen, &mut cand, j.steps, &j.g);
    }
}

fn apply(lab: &mut [u32], j: &Job) {
    match j.mode {
        Mode::Grow => grow(lab, j),
        Mode::Shrink => {
            for i in shrink_cells(lab, j.chosen, j.d, false, &j.g) {
                lab[i] = 0;
            }
        }
        Mode::Open => {
            let orig = lab.to_vec();
            for i in shrink_cells(lab, j.chosen, j.d, false, &j.g) {
                lab[i] = 0;
            }
            // grown back over the cells the object had: those within the distance of what is left of it
            let sites: Vec<u32> = lab.iter().map(|&l| if l != 0 && j.chosen[l as usize] { l } else { 0 }).collect();
            let (dist, near) = edt(&sites, &j.g.lat);
            let lim = limit(j.d);
            for i in 0..lab.len() {
                if lab[i] == 0 && orig[i] != 0 && j.chosen[orig[i] as usize] && near[i] == orig[i] && dist[i] <= lim {
                    lab[i] = orig[i];
                }
            }
        }
        Mode::Close => {
            let orig = lab.to_vec();
            grow(lab, j);
            // (the image's edge is outside here, so growth that reached it does not stay)
            for i in shrink_cells(lab, j.chosen, j.d, true, &j.g) {
                // (only a cell that was added goes back)
                if lab[i] != orig[i] {
                    lab[i] = orig[i];
                }
            }
        }
        Mode::Fill => {
            let free: Vec<bool> = (0..lab.len())
                .map(|i| (lab[i] == 0 || j.taken[lab[i] as usize]) && j.allowed.is_none_or(|a| a[i]))
                .collect();
            fill(lab, j.chosen, &free, j.max_hole, &j.g);
        }
        Mode::Smooth => smooth(lab, j.chosen, j.d, &j.g),
        Mode::Refine => refine(lab, j.chosen, j.layers.unwrap_or(&[]), j.steps, &j.g),
    }
}

/// The table of objects without an image: id, cells and area (the class is the objects', which objects.to_vector
/// writes as an attribute of its own).
fn basic_table(o: &Objects) -> Table {
    let area = o.cell_area();
    let mut t = Table::new(["id", "cells", "area"].map(String::from).to_vec());
    t.rows = (0..o.count).map(|k| vec![(k + 1) as f64, o.cells[k] as f64, o.cells[k] as f64 * area]).collect();
    t
}

/// `objects.resize`.
struct Resize;

impl Operator for Resize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.resize",
            "Objects grown, shrunk and smoothed cell by cell, as eCognition's pixel-based object resizing, its \
             morphology, its filling of holes and its border optimization (Serra 1982, Soille 2004). mode grow takes \
             the free cells within distance (map units) or steps (cells) of the chosen objects, each to the nearest; \
             shrink removes the cells of the chosen objects that lie within that distance of a cell that is not their \
             object's (another object, no object; the image's edge is not a border); open is shrink and then grow back \
             over the cells the object had (it cuts thin necks and drops specks); close is grow and then shrink only \
             the cells that were added (it fills slits and gaps inside an object; the image's edge is outside, so what grew              up to it goes back); fill gives the holes of free cells \
             that one object encloses, not touching the image's edge, to it (max_hole limits their cells); smooth \
             gives each cell the most common label in a disc of that radius (spikes and notches go, corners round; \
             nothing is added to the outside); refine is the border optimization: a border cell goes to the adjacent \
             object whose band means are closest (z-scores of the whole image), in steps passes that read the means \
             of the pass before. Without a mask or into, grow and shrink are exact Euclidean (Felzenszwalb & \
             Huttenlocher) in one pass. A mask (a map: cells that are not 0 may be taken, say ndvi > 0.3 from \
             raster.calc) or into (class names whose cells grow may take, eCognition's candidates) makes the growth \
             geodesic, ring by ring: a cell takes the object with most cells around it, a tie the lower id. classes \
             limits the objects that change. An object cut in two becomes two objects of its class; the level above \
             is dropped (with a warning), as are the other attributes (objects.features again, or connect the \
             image). report has a row per chosen object: its cells before and after and the objects it ended in. The \
             same result on every run, on any number of threads.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects to resize"))
        .input(PortSpec::optional(
            "mask",
            Type::Map,
            "a map on the objects' grid: only cells that are not 0 may be taken (grow, close) or filled",
        ))
        .input(PortSpec::optional("image", Type::Raster, "the image of the objects: needed by refine; the table of band means"))
        .output(PortSpec::new("objects", Type::Objects, "the resized objects"))
        .output(PortSpec::new("table", Type::Table, "id, cells, area (and the band means and deviations with an image)"))
        .output(PortSpec::new("report", Type::Table, "a row per chosen object: its id in the input, its cells before and after, the objects it ended in"))
        .param(
            PortSpec::optional("mode", Type::Text, "grow (the default), shrink, open, close, fill, smooth or refine")
                .choices(["grow", "shrink", "open", "close", "fill", "smooth", "refine"]),
        )
        .param(PortSpec::optional("classes", Type::Any, "the class names whose objects change (default: all)"))
        .param(PortSpec::optional("distance", Type::Number, "how far, in map units (grow, shrink, open, close; the radius of smooth)"))
        .param(PortSpec::optional("steps", Type::Integer, "how far in cells, or the passes of refine (default 1)"))
        .param(PortSpec::optional(
            "into",
            Type::Any,
            "grow, fill: none (the default: only cells of no object) or the class names whose cells may be taken",
        ))
        .param(PortSpec::optional("max_hole", Type::Integer, "fill: the most cells of a hole (default: every enclosed hole)"))
        .param(PortSpec::optional("neighbours", Type::Integer, "4 or 8: the cells an object is connected by, and a ring grows by (default 8)").choices([4, 8]))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        if o.grid.cells() >= 1 << 28 {
            return Err(bad(format!(
                "{} × {} cells, more than objects.resize works in memory",
                o.grid.width, o.grid.height
            )));
        }
        let mode = match args.param("mode", "grow".to_string())?.as_str() {
            "grow" => Mode::Grow,
            "shrink" => Mode::Shrink,
            "open" => Mode::Open,
            "close" => Mode::Close,
            "fill" => Mode::Fill,
            "smooth" => Mode::Smooth,
            "refine" => Mode::Refine,
            other => {
                return Err(bad(format!("mode {other:?}: grow, shrink, open, close, fill, smooth or refine")));
            }
        };
        let nbs: i64 = args.param("neighbours", 8)?;
        if nbs != 4 && nbs != 8 {
            return Err(bad(format!("neighbours {nbs}: 4 or 8")));
        }
        let lat = Lattice::of(&o.grid);
        let cell = lat.dx.max(lat.dy);
        let distance: Option<f64> = args.opt_param("distance")?;
        let steps: Option<i64> = args.opt_param("steps")?;
        if distance.is_some() && steps.is_some() {
            return Err(bad("give distance or steps, not both".into()));
        }
        if let Some(d) = distance
            && (!d.is_finite() || d <= 0.0)
        {
            return Err(bad(format!("distance {d}: above 0")));
        }
        if let Some(s) = steps
            && s < 1
        {
            return Err(bad(format!("steps {s}: 1 or more")));
        }
        if mode == Mode::Refine && distance.is_some() {
            return Err(bad("refine takes steps (its passes), not distance".into()));
        }
        // the distance in map units and in cells (a distance is rounded to whole rings where the growth is ring by ring)
        let d = distance.unwrap_or(steps.unwrap_or(1) as f64 * cell);
        let nsteps = match (distance, steps) {
            (Some(d), _) => ((d / cell).round() as usize).max(1),
            (None, Some(s)) => s as usize,
            (None, None) => 1,
        };
        if matches!(mode, Mode::Fill) && (distance.is_some() || steps.is_some()) {
            ctx.warn(format!("node {node:?}: distance and steps are not used by fill (max_hole limits it)"));
        }
        let max_hole: Option<i64> = args.opt_param("max_hole")?;
        if let Some(m) = max_hole {
            if m < 1 {
                return Err(bad(format!("max_hole {m}: 1 or more")));
            }
            if mode != Mode::Fill {
                ctx.warn(format!("node {node:?}: max_hole is for fill: not used by this mode"));
            }
        }
        let only = match args.opt_param::<Vec<String>>("classes")? {
            Some(names) if !names.is_empty() => Some(class_numbers(o, &names).map_err(|e| bad(e.0))?),
            _ => None,
        };
        let chosen: Vec<bool> = std::iter::once(false)
            .chain((0..o.count).map(|k| only.as_ref().is_none_or(|s| s.contains(&o.class_of[k]))))
            .collect();
        // the classes whose cells may be taken
        let into: BTreeSet<u32> = match args.opt_param::<serde_json::Value>("into")? {
            None => BTreeSet::new(),
            Some(serde_json::Value::String(s)) if s == "none" => BTreeSet::new(),
            Some(serde_json::Value::String(s)) => class_numbers(o, &[s]).map_err(|e| bad(e.0))?,
            Some(v) => {
                let names: Vec<String> =
                    serde_json::from_value(v).map_err(|e| bad(format!("into: none or class names ({e})")))?;
                class_numbers(o, &names).map_err(|e| bad(e.0))?
            }
        };
        if !into.is_empty() && !matches!(mode, Mode::Grow | Mode::Close | Mode::Fill) {
            ctx.warn(format!("node {node:?}: into is for grow, close and fill: not used by this mode"));
        }
        let taken: Vec<bool> = std::iter::once(false)
            .chain((0..o.count).map(|k| into.contains(&o.class_of[k]) && !chosen[k + 1]))
            .collect();
        let allowed: Option<Vec<bool>> = match args.get("mask") {
            Some(Value::Map(m)) => {
                if !m.grid.same_as(&o.grid) {
                    return Err(bad(
                        "the mask is not on the grid of the objects: put it there with raster.match".into()
                    ));
                }
                Some(m.data.iter().map(|&v| v.is_finite() && v != 0.0).collect())
            }
            Some(v) => return Err(bad(format!("the mask is a {:?}, not a map", v.ty()))),
            None => None,
        };
        if allowed.is_some() && !matches!(mode, Mode::Grow | Mode::Close | Mode::Fill) {
            ctx.warn(format!("node {node:?}: the mask is for grow, close and fill: not used by this mode"));
        }
        let image = match args.get("image") {
            Some(_) => {
                let l = big::layers(args, "image")?;
                if !l.grid.same_as(&o.grid) {
                    return Err(bad(format!(
                        "the image is on a {}x{} grid, the objects' is {}x{}: put it on the objects' grid with \
                         raster.match",
                        l.grid.height, l.grid.width, o.grid.height, o.grid.width
                    )));
                }
                Some(l)
            }
            None => None,
        };
        if mode == Mode::Refine && image.is_none() {
            return Err(bad("refine needs the input image (the bands whose means decide)".into()));
        }
        let job = Job {
            mode,
            chosen: &chosen,
            taken: &taken,
            allowed: allowed.as_deref(),
            d,
            steps: nsteps,
            max_hole: max_hole.map_or(usize::MAX, |m| m as usize),
            layers: image.as_ref().map(|l| l.data.as_slice()),
            g: Geo { w: o.grid.width, h: o.grid.height, eight: nbs == 8, lat },
        };
        let before: Vec<u32> = o.labels.iter().copied().collect();
        let mut lab = before.clone();
        if o.count > 0 && chosen.iter().any(|&c| c) {
            apply(&mut lab, &job);
        }
        let table_of = |objs: &Objects| match &image {
            Some(l) => stats_table(objs, &l.names, &l.data),
            None => objs.table.clone(),
        };
        // the chosen objects' cells before and after, and the objects each ended in
        let mut after = vec![0u32; o.count + 1];
        for &l in &lab {
            after[l as usize] += 1;
        }
        let mut report = Table::new(vec!["object".into(), "before".into(), "after".into(), "pieces".into()]);
        let make_report = |report: &mut Table, pieces: &[u32]| {
            for id in 1..=o.count {
                if chosen[id] {
                    report.rows.push(vec![id as f64, o.cells[id - 1] as f64, after[id] as f64, pieces[id] as f64]);
                }
            }
        };
        if lab == before {
            let mut pieces = vec![0u32; o.count + 1];
            for (p, &c) in pieces.iter_mut().zip(std::iter::once(&0).chain(o.cells.iter())) {
                *p = u32::from(c > 0);
            }
            make_report(&mut report, &pieces);
            let mut out = outputs([("table", Value::Table(table_of(o))), ("report", Value::Table(report))]);
            out.insert("objects".into(), Value::Objects(o.clone()));
            return Ok(out);
        }
        // the new objects: the connected sets of each old id's cells
        let region: Vec<u64> = lab.iter().map(|&l| if l == 0 { NO_REGION } else { l as u64 }).collect();
        let cut = Objects::from_regions(o.grid.clone(), &region, nbs as usize);
        let mut old_of = vec![0u32; cut.count];
        for (&n, &l) in cut.labels.iter().zip(lab.iter()) {
            if n != 0 && old_of[n as usize - 1] == 0 {
                old_of[n as usize - 1] = l;
            }
        }
        let mut pieces = vec![0u32; o.count + 1];
        for &l in &old_of {
            pieces[l as usize] += 1;
        }
        make_report(&mut report, &pieces);
        // each piece has the class (and memberships) of its object
        let class_of: Vec<u32> = old_of.iter().map(|&l| o.class_of[l as usize - 1]).collect();
        let membership: Vec<Vec<f64>> =
            o.membership.iter().map(|m| old_of.iter().map(|&l| m[l as usize - 1]).collect()).collect();
        let mut cut = cut.with_classes(o.classes.clone(), class_of, membership)?;
        if o.parent.is_some() {
            ctx.warn(format!(
                "node {node:?}: the resized objects replace the objects, so the level above is dropped (objects.merge \
                 with level makes one again)"
            ));
        }
        // the band statistics with an image, else the plain table of the new objects (id, cells, area)
        let t = if image.is_some() { table_of(&cut) } else { basic_table(&cut) };
        cut = cut.with_table(t)?;
        let table = cut.table.clone();
        let mut out = outputs([("table", Value::Table(table)), ("report", Value::Table(report))]);
        out.insert("objects".into(), Value::Objects(Arc::new(cut)));
        Ok(out)
    }
}
