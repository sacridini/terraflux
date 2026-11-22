//! `objects.split`: objects that touch divided into their pieces, by the watershed of the distance to the object's
//! border (the necks are the saddles) or of a surface (peaks are the centres). What eCognition's watershed
//! transformation and CellProfiler's declumping do, for tree crowns, buildings, cells and mounds.
//!
//! Per object, in its box with a pad of one cell ([`super::each`]): the Euclidean distance from each cell to the
//! nearest cell that is not the object's (the exact transform of `linkage::edt`, in map units), or the surface; the
//! markers are the peaks that stand `h` or more above the saddle where they meet a higher one (the h-minima of
//! `segment.watershed` over the landscape turned upside down); the flooding is its queue. A piece smaller than
//! `min_size`, or cut from a bigger one by a neck longer than `max_neck` times its width, goes back to the neighbour
//! piece it shares the longest border with. The objects are independent: they run in parallel and the result is
//! assembled in the order of the ids.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::each::{Frame, each};
use super::levels::class_numbers;
use super::superpixels::{Prep, smooth};
use super::watershed::{flood_in, minima_in, order_of};
use super::{NO_REGION, Objects, big, objects_in, stats_table};
use crate::linkage::{Lattice, edt};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Split));
}

/// What the settings ask.
struct Setup<'a> {
    eight: bool,
    /// The peaks' least height over the saddle (map units, or the surface's).
    h: f64,
    min_size: usize,
    max_neck: Option<f64>,
    invert: bool,
    /// The (smoothed) surface when the centres are its peaks.
    surface: Option<&'a Prep>,
    lat: Lattice,
}

/// One object's pieces: the label of each cell of its frame (0 off the object, 1..=pieces) and how many pieces.
struct Pieces {
    frame: Frame,
    lab: Vec<u32>,
    count: u32,
}

/// The cells of the frame that are in the mask but have no label yet, taken by the smallest label around them, round
/// after round (8 neighbours); what stays out of reach, if anything, goes to piece 1.
fn fill(lab: &mut [u32], mask: &[bool], w: usize, h: usize) {
    let mut left: Vec<usize> = (0..w * h).filter(|&i| mask[i] && lab[i] == 0).collect();
    while !left.is_empty() {
        let mut gave: Vec<(usize, u32)> = Vec::new();
        for &i in &left {
            let (r, c) = ((i / w) as i64, (i % w) as i64);
            let mut best = 0u32;
            for dr in -1i64..=1 {
                for dc in -1i64..=1 {
                    let (rr, cc) = (r + dr, c + dc);
                    if (dr, dc) == (0, 0) || rr < 0 || cc < 0 || rr >= h as i64 || cc >= w as i64 {
                        continue;
                    }
                    let l = lab[rr as usize * w + cc as usize];
                    if l != 0 && (best == 0 || l < best) {
                        best = l;
                    }
                }
            }
            if best != 0 {
                gave.push((i, best));
            }
        }
        if gave.is_empty() {
            for &i in &left {
                lab[i] = 1;
            }
            return;
        }
        // (all of a round at once, so the order of the cells does not matter)
        for &(i, l) in &gave {
            lab[i] = l;
        }
        left.retain(|&i| lab[i] == 0);
    }
}

/// The pieces as the merging sees them: cells and largest distance to the border per piece (index = label), and the
/// border between pairs as (edges in the 4-neighbourhood, corner contacts).
struct Graph {
    cells: Vec<u32>,
    reach: Vec<f64>,
    live: Vec<bool>,
    /// The piece each label ended in after the merges.
    alias: Vec<u32>,
    border: BTreeMap<(u32, u32), (u32, u32)>,
}

impl Graph {
    fn new(lab: &[u32], dist: &[f64], w: usize, k: usize) -> Graph {
        let mut g = Graph {
            cells: vec![0; k + 1],
            reach: vec![0.0; k + 1],
            live: vec![true; k + 1],
            alias: (0..k as u32 + 1).collect(),
            border: BTreeMap::new(),
        };
        g.live[0] = false;
        let n = lab.len();
        for i in 0..n {
            let a = lab[i];
            if a == 0 {
                continue;
            }
            g.cells[a as usize] += 1;
            if let Some(d) = dist.get(i).filter(|d| d.is_finite()) {
                g.reach[a as usize] = g.reach[a as usize].max(*d);
            }
            let c = i % w;
            let mut see = |j: usize, corner: bool| {
                let b = lab[j];
                if b != 0 && b != a {
                    let e = g.border.entry((a.min(b), a.max(b))).or_insert((0, 0));
                    if corner {
                        e.1 += 1;
                    } else {
                        e.0 += 1;
                    }
                }
            };
            if c + 1 < w {
                see(i + 1, false);
            }
            if i + w < n {
                see(i + w, false);
                if c + 1 < w {
                    see(i + w + 1, true);
                }
                if c > 0 {
                    see(i + w - 1, true);
                }
            }
        }
        g
    }

    /// The piece `from` taken into `to`.
    fn merge(&mut self, from: u32, to: u32) {
        let old = std::mem::take(&mut self.border);
        for ((a, b), (e, d)) in old {
            let (a, b) = (if a == from { to } else { a }, if b == from { to } else { b });
            if a == b {
                continue;
            }
            let s = self.border.entry((a.min(b), a.max(b))).or_insert((0, 0));
            s.0 += e;
            s.1 += d;
        }
        self.cells[to as usize] += self.cells[from as usize];
        self.reach[to as usize] = self.reach[to as usize].max(self.reach[from as usize]);
        self.live[from as usize] = false;
        for a in self.alias.iter_mut().filter(|a| **a == from) {
            *a = to;
        }
    }
}

/// The length of a border in map units (its edges, or its corner contacts when it has no edge).
fn cut_length(b: (u32, u32), cell: f64) -> f64 {
    (if b.0 > 0 { b.0 } else { b.1 }) as f64 * cell
}

/// The pieces of one object, none when it stays whole.
fn split_one(o: &Objects, id: u32, f: &Frame, s: &Setup) -> Option<Pieces> {
    let (w, h) = (f.w, f.h);
    let mask = f.mask(o, id);
    let mut lat = s.lat;
    (lat.w, lat.h) = (w, h);
    // the distance of each cell to the nearest cell off the object (the pad is off it): the landscape, or the width
    let dist = if s.surface.is_none() || s.max_neck.is_some() {
        let sites: Vec<u32> = mask.iter().map(|&m| u32::from(!m)).collect();
        edt(&sites, &lat).0
    } else {
        Vec::new()
    };
    // the height of each cell, on the cells that count
    let (valid, height): (Vec<bool>, Vec<f64>) = match s.surface {
        None => (mask.clone(), dist.clone()),
        Some(p) => {
            let mut v = vec![false; w * h];
            let mut z = vec![0.0; w * h];
            for i in 0..w * h {
                if let Some(g) = f.global(o, i).filter(|_| mask[i]).filter(|&g| p.valid[g]) {
                    v[i] = true;
                    z[i] = if s.invert { -p.x[g] } else { p.x[g] };
                }
            }
            (v, z)
        }
    };
    let top = (0..w * h).filter(|&i| valid[i]).map(|i| height[i]).fold(f64::NEG_INFINITY, f64::max);
    if top == f64::NEG_INFINITY {
        return None;
    }
    // upside down and never negative: the peaks are the minima, as the watershed's queue wants them
    let low: Vec<f64> = (0..w * h).map(|i| if valid[i] { top - height[i] } else { 0.0 }).collect();
    let p = Prep { w, h, nb: 1, valid, x: Vec::new() };
    let order = order_of(&p, &low);
    let markers = minima_in(&p, &order, s.eight, s.h);
    if markers.len() < 2 {
        return None;
    }
    let mut lab = flood_in(&p, &low, &order, &markers, s.eight, 0.0);
    fill(&mut lab, &mask, w, h);
    let mut g = Graph::new(&lab, &dist, w, markers.len());
    let cell = (s.lat.dx + s.lat.dy) / 2.0;
    loop {
        // a neck too long for the smaller piece: the worst one first
        if let Some(mn) = s.max_neck {
            let mut worst: Option<(f64, u32, u32)> = None;
            for (&(a, b), &bd) in &g.border {
                let small = if g.cells[a as usize] <= g.cells[b as usize] { a } else { b };
                let room = mn * 2.0 * g.reach[small as usize];
                let len = cut_length(bd, cell);
                if len >= room && worst.is_none_or(|(r, ..)| len / room > r) {
                    worst = Some((len / room, small, a + b - small));
                }
            }
            if let Some((_, from, to)) = worst {
                g.merge(from, to);
                continue;
            }
        }
        // a piece too small: the smallest first, to the neighbour of the longest border
        if s.min_size > 0 {
            let mut small: Vec<u32> =
                (1..g.live.len() as u32).filter(|&l| g.live[l as usize] && (g.cells[l as usize] as usize) < s.min_size).collect();
            small.sort_by_key(|&l| (g.cells[l as usize], l));
            let mut took = false;
            for l in small {
                let mut best: Option<((u32, u32), u32)> = None;
                for (&(a, b), &bd) in &g.border {
                    let other = if a == l {
                        b
                    } else if b == l {
                        a
                    } else {
                        continue;
                    };
                    if best.is_none_or(|(k, n)| bd > k || (bd == k && other < n)) {
                        best = Some((bd, other));
                    }
                }
                if let Some((_, to)) = best {
                    g.merge(l, to);
                    took = true;
                    break;
                }
            }
            if took {
                continue;
            }
        }
        break;
    }
    // the pieces left, numbered in the order of their first cell
    let mut number = vec![0u32; g.live.len()];
    let mut next = 0u32;
    for l in lab.iter_mut().filter(|l| **l != 0) {
        let at = g.alias[*l as usize] as usize;
        if number[at] == 0 {
            next += 1;
            number[at] = next;
        }
        *l = number[at];
    }
    (next > 1).then_some(Pieces { frame: *f, lab, count: next })
}

/// `objects.split`.
struct Split;

impl Operator for Split {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.split",
            "Objects that touch divided into their pieces, as eCognition's watershed transformation and \
             CellProfiler's declumping (Vincent & Soille 1991): tree crowns, buildings, cells, mounds. by = distance \
             (the default) floods the distance from each cell to the object's border, so a piece grows from each \
             rounded part and the cut is at the neck; by = surface floods a map (a canopy height model, the brightness) \
             whose peaks are the centres (invert for dark ones). A centre needs to stand h above the saddle where it \
             meets a higher one (h-minima, as segment.watershed's markers): a lower bump is noise and joins its \
             neighbour. min_size sends a piece of fewer cells to the neighbour piece it shares the longest border \
             with; max_neck (eCognition's length factor) keeps a cut only when it is shorter than max_neck times the \
             width of the smaller piece (twice its largest distance to the border), so two parts joined by a wide \
             neck stay one object. classes limits which objects split. The distance is computed inside the operator, \
             per object and in parallel, exactly (Felzenszwalb & Huttenlocher); the cells off the object, the \
             image's edge and the cells without a surface value are its border or are filled from the pieces around. \
             The pieces take the class of their object and replace it in the same level: the level above is \
             dropped (with a warning), as are the other attributes (objects.features again). report has a row per \
             object that split. The same result on every run, on any number of threads.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects to split"))
        .input(PortSpec::optional(
            "surface",
            Type::Map,
            "by surface: a map on the objects' grid whose peaks are the centres (a canopy height model, the brightness)",
        ))
        .input(PortSpec::optional("image", Type::Raster, "the image of the objects, for the table of band means"))
        .output(PortSpec::new("objects", Type::Objects, "the objects with the pieces in place of those that split"))
        .output(PortSpec::new("table", Type::Table, "id, cells, area (and the band means and deviations with an image)"))
        .output(PortSpec::new("report", Type::Table, "a row per object that split: its id in the input and its pieces"))
        .param(PortSpec::optional("by", Type::Text, "distance (the default) or surface"))
        .param(PortSpec::optional("invert", Type::Bool, "by surface: the centres are the lowest values (default false)"))
        .param(PortSpec::optional("classes", Type::Any, "the class names whose objects split (default: all)"))
        .param(PortSpec::optional(
            "h",
            Type::Number,
            "how high a centre must stand over the saddle: map units by distance (default one cell), the surface's \
             units by surface (default 0.05 of its range)",
        ))
        .param(PortSpec::optional("min_size", Type::Integer, "a piece of fewer cells goes to the neighbour piece (default 0)"))
        .param(PortSpec::optional(
            "max_neck",
            Type::Number,
            "a cut stays only when shorter than max_neck times the width of the smaller piece (default: no limit)",
        ))
        .param(PortSpec::optional("sigma", Type::Number, "by surface: the smoothing's Gaussian in cells (default 0)"))
        .param(PortSpec::optional("neighbours", Type::Integer, "4 or 8: the cells a piece is connected by (default 8)").choices([4, 8]))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        if o.grid.cells() >= 1 << 28 {
            return Err(bad(format!(
                "{} × {} cells, more than objects.split works in memory",
                o.grid.width, o.grid.height
            )));
        }
        let by_surface = match args.param("by", "distance".to_string())?.as_str() {
            "distance" => false,
            "surface" => true,
            other => return Err(bad(format!("by {other:?}: distance or surface"))),
        };
        let invert: bool = args.param("invert", false)?;
        let sigma: f64 = args.param("sigma", 0.0)?;
        if !sigma.is_finite() || sigma < 0.0 {
            return Err(bad(format!("sigma {sigma}: 0 or more")));
        }
        let hp: Option<f64> = args.opt_param("h")?;
        if let Some(h) = hp
            && (!h.is_finite() || h < 0.0)
        {
            return Err(bad(format!("h {h}: 0 or more")));
        }
        let min_size: i64 = args.param("min_size", 0)?;
        if min_size < 0 {
            return Err(bad(format!("min_size {min_size}: 0 or more")));
        }
        let max_neck: Option<f64> = args.opt_param("max_neck")?;
        if let Some(m) = max_neck
            && (!m.is_finite() || m <= 0.0)
        {
            return Err(bad(format!("max_neck {m}: above 0")));
        }
        let nbs: i64 = args.param("neighbours", 8)?;
        if nbs != 4 && nbs != 8 {
            return Err(bad(format!("neighbours {nbs}: 4 or 8")));
        }
        let only = match args.opt_param::<Vec<String>>("classes")? {
            Some(names) if !names.is_empty() => Some(class_numbers(o, &names).map_err(|e| bad(e.0))?),
            _ => None,
        };
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
        let mut surface: Option<Prep> = None;
        match args.get("surface") {
            Some(Value::Map(m)) => {
                if !m.grid.same_as(&o.grid) {
                    return Err(bad("the surface is not on the grid of the objects: put it there with raster.match".into()));
                }
                if by_surface {
                    let mut p = Prep {
                        w: o.grid.width,
                        h: o.grid.height,
                        nb: 1,
                        valid: m.data.iter().map(|v| !v.is_nan()).collect(),
                        x: m.data.iter().map(|&v| if v.is_nan() { 0.0 } else { v }).collect(),
                    };
                    if sigma > 0.0 {
                        smooth(&mut p, sigma);
                    }
                    surface = Some(p);
                } else {
                    ctx.warn(format!("node {node:?}: a surface is given but by is distance: the surface is not used"));
                }
            }
            Some(v) => return Err(bad(format!("the surface is a {:?}, not a map", v.ty()))),
            None if by_surface => return Err(bad("by surface needs the input surface".into())),
            None => {}
        }
        if !by_surface && (sigma > 0.0 || invert) {
            ctx.warn(format!("node {node:?}: sigma and invert are for by surface: not used by distance"));
        }
        let lat = Lattice::of(&o.grid);
        let h = match (hp, &surface) {
            (Some(h), _) => h,
            (None, None) => lat.dx.max(lat.dy),
            (None, Some(p)) => {
                let (lo, hi) = (0..p.w * p.h)
                    .filter(|&i| p.valid[i])
                    .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), i| (a.min(p.x[i]), b.max(p.x[i])));
                if lo <= hi { 0.05 * (hi - lo) } else { 0.0 }
            }
        };
        let setup = Setup {
            eight: nbs == 8,
            h,
            min_size: min_size as usize,
            max_neck,
            invert,
            surface: surface.as_ref(),
            lat,
        };
        let ids: Vec<u32> = (1..=o.count as u32)
            .filter(|&id| o.cells[id as usize - 1] > 1 && only.as_ref().is_none_or(|s| s.contains(&o.class_of[id as usize - 1])))
            .collect();
        let found: Vec<Option<Pieces>> = each(o, &ids, |id, f| split_one(o, id, f, &setup));
        let mut report = Table::new(vec!["object".into(), "pieces".into()]);
        let split: Vec<(u32, &Pieces)> =
            ids.iter().zip(&found).filter_map(|(&id, p)| p.as_ref().map(|p| (id, p))).collect();
        for (id, p) in &split {
            report.rows.push(vec![*id as f64, p.count as f64]);
        }
        if split.is_empty() {
            let table = match &image {
                Some(l) => stats_table(o, &l.names, &l.data),
                None => o.table.clone(),
            };
            let mut out = outputs([("table", Value::Table(table)), ("report", Value::Table(report))]);
            out.insert("objects".into(), Value::Objects(o.clone()));
            return Ok(out);
        }
        // the regions: an object's id, and above the count a number per piece of each object that split
        let mut region: Vec<u64> = o.labels.iter().map(|&l| if l == 0 { NO_REGION } else { l as u64 }).collect();
        let mut base = o.count as u64;
        for (_, p) in &split {
            let f = &p.frame;
            for (i, &l) in p.lab.iter().enumerate().filter(|(_, l)| **l != 0) {
                if let Some(g) = f.global(o, i) {
                    region[g] = base + l as u64;
                }
            }
            base += p.count as u64;
        }
        let cut = Objects::from_regions(o.grid.clone(), &region, nbs as usize);
        // each piece has the class (and memberships) of its object
        let mut old_of = vec![0u32; cut.count];
        for (&n, &l) in cut.labels.iter().zip(o.labels.iter()) {
            if n != 0 {
                old_of[n as usize - 1] = l;
            }
        }
        let class_of: Vec<u32> = old_of.iter().map(|&l| o.class_of[l as usize - 1]).collect();
        let membership: Vec<Vec<f64>> =
            o.membership.iter().map(|m| old_of.iter().map(|&l| m[l as usize - 1]).collect()).collect();
        let mut cut = cut.with_classes(o.classes.clone(), class_of, membership)?;
        if o.parent.is_some() {
            ctx.warn(format!(
                "node {node:?}: the pieces replace the objects, so the level above is dropped (objects.merge with \
                 level makes one again)"
            ));
        }
        if let Some(l) = &image {
            let t = stats_table(&cut, &l.names, &l.data);
            cut = cut.with_table(t)?;
        }
        let table = cut.table.clone();
        let mut out = outputs([("table", Value::Table(table)), ("report", Value::Table(report))]);
        out.insert("objects".into(), Value::Objects(Arc::new(cut)));
        Ok(out)
    }
}
