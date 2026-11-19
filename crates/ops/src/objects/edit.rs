//! `objects.edit`: objects corrected by hand, as eCognition's manual editing (merge, cut, delete). The edits are
//! kept as a list in the node's settings, by places (coordinates on the objects' grid) and not by object numbers,
//! which change when the segmentation above changes: the same list, run on another segmentation, finds its objects
//! again where they are. An edit that finds no object, or that changes nothing, is warned about and has a row in
//! the `report` table.
//!
//! - `{ "merge": [[x, y], ...] }`: the objects at the points become one, the first point's.
//! - `{ "cut": [[x, y], ...] }`: a line (two points or more) cuts the objects it crosses; each is split into the
//!   connected pieces it has without the cells of the line, and each cell of the line goes to a piece next to it
//!   (the smallest label of those around it, so that the result is the same every time).
//! - `{ "delete": [x, y], "into": "none" | "neighbour" }`: the object at the point is left with no object (the
//!   default), or taken by its most similar neighbour (the nearest band means with an image; else the longest
//!   shared border).
//!
//! The objects are numbered again after the edits and come with no classes and no level above (run the features
//! and the classification after this node).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{NO_REGION, Objects, big, objects_in, stats_table};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Edit));
}

/// An edit as the setting writes it.
#[derive(Debug, Clone, PartialEq)]
enum Step {
    Merge(Vec<[f64; 2]>),
    Cut(Vec<[f64; 2]>),
    Delete([f64; 2], bool),
}

impl Step {
    fn kind(&self) -> &'static str {
        match self {
            Step::Merge(_) => "merge",
            Step::Cut(_) => "cut",
            Step::Delete(..) => "delete",
        }
    }
}

fn point(v: &serde_json::Value) -> Option<[f64; 2]> {
    let a = v.as_array()?;
    match a.as_slice() {
        [x, y] => Some([x.as_f64()?, y.as_f64()?]),
        _ => None,
    }
}

fn points(v: &serde_json::Value) -> Option<Vec<[f64; 2]>> {
    v.as_array()?.iter().map(point).collect()
}

/// The list of edits of the setting `edits`, checked.
fn parse(v: &serde_json::Value) -> Result<Vec<Step>> {
    let forms = "edits: a list of { \"merge\": [[x, y], ...] }, { \"cut\": [[x, y], ...] } and { \"delete\": [x, y], \
                 \"into\": \"none\" | \"neighbour\" }";
    let list = match v {
        serde_json::Value::Null => return Ok(Vec::new()),
        serde_json::Value::Array(l) => l,
        _ => return Err(Error::msg(forms)),
    };
    let mut out = Vec::with_capacity(list.len());
    for (k, e) in list.iter().enumerate() {
        let at = |m: &str| Error::msg(format!("edits: edit {}: {m}", k + 1));
        let Some(m) = e.as_object() else { return Err(at(&format!("not an object; {forms}"))) };
        if let Some(bad) = m.keys().find(|k| !["merge", "cut", "delete", "into"].contains(&k.as_str())) {
            return Err(at(&format!("unknown key {bad:?}; {forms}")));
        }
        let kinds: Vec<&str> = ["merge", "cut", "delete"].into_iter().filter(|k| m.contains_key(*k)).collect();
        if kinds.len() != 1 {
            return Err(at(&format!("exactly one of merge, cut and delete; {forms}")));
        }
        if m.contains_key("into") && kinds[0] != "delete" {
            return Err(at("into goes with delete"));
        }
        out.push(match kinds[0] {
            "merge" => match points(&m["merge"]) {
                Some(p) if p.len() >= 2 => Step::Merge(p),
                _ => return Err(at("merge: a list of two points or more, [[x, y], ...]")),
            },
            "cut" => match points(&m["cut"]) {
                Some(p) if p.len() >= 2 => Step::Cut(p),
                _ => return Err(at("cut: a line of two points or more, [[x, y], ...]")),
            },
            _ => {
                let Some(p) = point(&m["delete"]) else { return Err(at("delete: a point, [x, y]")) };
                let into = match m.get("into").and_then(|i| i.as_str()) {
                    None if !m.contains_key("into") => false,
                    Some("none") => false,
                    Some("neighbour") | Some("neighbor") => true,
                    _ => return Err(at("into: \"none\" or \"neighbour\"")),
                };
                Step::Delete(p, into)
            }
        });
    }
    Ok(out)
}

/// What an edit did: `status` is "done" or what stopped it, `why` the words for a warning (none when it is done),
/// `cells` the cells that changed their object.
struct Outcome {
    status: &'static str,
    cells: usize,
    why: Option<String>,
}

impl Outcome {
    fn done(cells: usize) -> Outcome {
        Outcome { status: "done", cells, why: None }
    }

    fn stop(status: &'static str, why: impl Into<String>) -> Outcome {
        Outcome { status, cells: 0, why: Some(why.into()) }
    }
}

const STATUSES: [&str; 7] = ["done", "partly", "no object", "already one", "no split", "not touching", "no neighbour"];

/// The labels while the edits go on: 0 for none, ids not contiguous (new ones above all the old).
struct State<'a> {
    w: usize,
    h: usize,
    eight: bool,
    lab: Vec<u32>,
    next: u32,
    /// The grid (its geotransform places the points) and the image's bands, when there is one.
    grid: &'a terraflux_raster::Grid,
    bands: &'a [Arc<Vec<f64>>],
}

impl State<'_> {
    /// The cell of a place (row, column fractions), none when the grid has no inverse.
    fn frac(&self, p: [f64; 2]) -> Option<(f64, f64)> {
        let t = self.grid.transform;
        let det = t[1] * t[5] - t[2] * t[4];
        if det == 0.0 || !det.is_finite() {
            return None;
        }
        let (x, y) = (p[0] - t[0], p[1] - t[3]);
        let c = (t[5] * x - t[2] * y) / det;
        let r = (t[1] * y - t[4] * x) / det;
        (c.is_finite() && r.is_finite()).then_some((c, r))
    }

    /// The index of the cell at a place, none off the grid.
    fn cell(&self, p: [f64; 2]) -> Option<usize> {
        let (c, r) = self.frac(p)?;
        let (c, r) = (c.floor(), r.floor());
        (c >= 0.0 && r >= 0.0 && (c as usize) < self.w && (r as usize) < self.h)
            .then(|| r as usize * self.w + c as usize)
    }

    /// The object at a place: none off the grid or on a cell with no object.
    fn at(&self, p: [f64; 2]) -> Option<u32> {
        self.cell(p).map(|i| self.lab[i]).filter(|&l| l != 0)
    }

    /// The neighbours of a cell (4 or 8), in a fixed order.
    fn around(&self, i: usize, mut f: impl FnMut(usize)) {
        let (r, c) = (i / self.w, i % self.w);
        for dr in -1i64..=1 {
            for dc in -1i64..=1 {
                if (dr, dc) == (0, 0) || (!self.eight && dr != 0 && dc != 0) {
                    continue;
                }
                let (rr, cc) = (r as i64 + dr, c as i64 + dc);
                if rr >= 0 && cc >= 0 && (rr as usize) < self.h && (cc as usize) < self.w {
                    f(rr as usize * self.w + cc as usize);
                }
            }
        }
    }

    fn merge(&mut self, pts: &[[f64; 2]]) -> Outcome {
        let found: Vec<Option<u32>> = pts.iter().map(|&p| self.at(p)).collect();
        if let Some(k) = found.iter().position(Option::is_none) {
            return Outcome::stop("no object", format!("the point {} is on no object: not merged", k + 1));
        }
        let target = found[0].unwrap();
        let mut others: Vec<u32> = found.iter().flatten().copied().filter(|&l| l != target).collect();
        others.sort_unstable();
        others.dedup();
        if others.is_empty() {
            return Outcome::stop("already one", "the points are on one object already: nothing to merge");
        }
        let mut was: Vec<(usize, u32)> = Vec::new();
        for (i, l) in self.lab.iter_mut().enumerate() {
            if others.binary_search(l).is_ok() {
                was.push((i, *l));
                *l = target;
            }
        }
        // objects that do not touch the first are not one object: they stay apart
        let start = self.cell(pts[0]).unwrap();
        let mut seen = vec![false; self.lab.len()];
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            let mut next = Vec::new();
            self.around(i, |j| next.push(j));
            for j in next {
                if !seen[j] && self.lab[j] == target {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        let apart = was.iter().filter(|(i, _)| !seen[*i]).count();
        for &(i, l) in &was {
            if !seen[i] {
                self.lab[i] = l;
            }
        }
        if apart == was.len() {
            return Outcome::stop("not touching", "the objects do not touch the first: left apart");
        }
        if apart > 0 {
            return Outcome {
                status: "partly",
                cells: was.len() - apart,
                why: Some("some of the objects do not touch the first: those were left apart".into()),
            };
        }
        Outcome::done(was.len())
    }

    /// The cells a line crosses, 4-connected (a diagonal step gets the cell beside it), in row-major order.
    fn line(&self, pts: &[[f64; 2]]) -> Vec<usize> {
        let fr: Vec<Option<(f64, f64)>> = pts.iter().map(|&p| self.frac(p)).collect();
        let mut cells: Vec<(usize, usize)> = Vec::new();
        let mut last: Option<(i64, i64)> = None;
        let inside = |c: i64, r: i64| c >= 0 && r >= 0 && (c as usize) < self.w && (r as usize) < self.h;
        for pair in fr.windows(2) {
            let (Some(a), Some(b)) = (pair[0], pair[1]) else { continue };
            let steps = ((b.0 - a.0).abs().max((b.1 - a.1).abs()) * 2.0).ceil().max(1.0) as usize;
            for s in 0..=steps {
                let t = s as f64 / steps as f64;
                let (c, r) = ((a.0 + (b.0 - a.0) * t).floor() as i64, (a.1 + (b.1 - a.1) * t).floor() as i64);
                if let Some((lc, lr)) = last
                    && lc != c
                    && lr != r
                    && inside(lc, lr)
                    && inside(c, r)
                {
                    cells.push((lr as usize, c as usize));
                }
                if inside(c, r) {
                    cells.push((r as usize, c as usize));
                }
                last = Some((c, r));
            }
        }
        cells.sort_unstable();
        cells.dedup();
        cells.into_iter().map(|(r, c)| r * self.w + c).collect()
    }

    fn cut(&mut self, pts: &[[f64; 2]]) -> Outcome {
        let line = self.line(pts);
        let mut crossed: Vec<u32> = line.iter().map(|&i| self.lab[i]).filter(|&l| l != 0).collect();
        crossed.sort_unstable();
        crossed.dedup();
        if crossed.is_empty() {
            return Outcome::stop("no object", "the line crosses no object: nothing cut");
        }
        let mut on_line = vec![false; self.lab.len()];
        for &i in &line {
            on_line[i] = true;
        }
        // the cells of each crossed object, in row-major order
        let mut cells: BTreeMap<u32, Vec<usize>> = crossed.iter().map(|&l| (l, Vec::new())).collect();
        for (i, l) in self.lab.iter().enumerate() {
            if let Some(v) = cells.get_mut(l) {
                v.push(i);
            }
        }
        let mut piece = vec![0u32; self.lab.len()];
        let (mut split, mut changed) = (0usize, 0usize);
        for (&id, own) in &cells {
            // the pieces without the line's cells: the first keeps the id, the others get new ones
            let mut labels: Vec<u32> = Vec::new();
            for &s in own {
                if on_line[s] || piece[s] != 0 {
                    continue;
                }
                let label = if labels.is_empty() {
                    id
                } else {
                    self.next += 1;
                    self.next
                };
                labels.push(label);
                piece[s] = label;
                let mut stack = vec![s];
                while let Some(i) = stack.pop() {
                    let mut near = Vec::new();
                    self.around(i, |j| near.push(j));
                    for j in near {
                        if self.lab[j] == id && !on_line[j] && piece[j] == 0 {
                            piece[j] = label;
                            stack.push(j);
                        }
                    }
                }
            }
            if labels.len() < 2 {
                for &s in own {
                    piece[s] = 0;
                }
                continue;
            }
            split += 1;
            // the cells of the line go to the smallest piece around them, round after round
            let mut left: Vec<usize> = own.iter().copied().filter(|&s| on_line[s]).collect();
            loop {
                let mut gave: Vec<(usize, u32)> = Vec::new();
                for &s in &left {
                    let mut best = 0u32;
                    self.around(s, |j| {
                        if piece[j] != 0 && (best == 0 || piece[j] < best) {
                            best = piece[j];
                        }
                    });
                    if best != 0 {
                        gave.push((s, best));
                    }
                }
                if gave.is_empty() {
                    break;
                }
                // (all of a round at once, so the order of the cells does not matter)
                for &(s, p) in &gave {
                    piece[s] = p;
                }
                left.retain(|&s| piece[s] == 0);
            }
            for &s in own {
                if piece[s] != 0 {
                    if self.lab[s] != piece[s] {
                        changed += 1;
                    }
                    self.lab[s] = piece[s];
                }
            }
        }
        if split == 0 {
            return Outcome::stop("no split", "the line does not split any object it crosses: nothing cut");
        }
        Outcome::done(changed)
    }

    /// The mean of each band over the cells of some objects: per object, None when it has no value in a band.
    fn means(&self, ids: &[u32]) -> HashMap<u32, Vec<Option<f64>>> {
        let at: HashMap<u32, usize> = ids.iter().enumerate().map(|(k, &l)| (l, k)).collect();
        let mut sums = vec![vec![(0.0f64, 0usize); self.bands.len()]; ids.len()];
        for (i, l) in self.lab.iter().enumerate() {
            if let Some(&k) = at.get(l) {
                for (b, band) in self.bands.iter().enumerate() {
                    let v = band[i];
                    if !v.is_nan() {
                        sums[k][b].0 += v;
                        sums[k][b].1 += 1;
                    }
                }
            }
        }
        ids.iter()
            .zip(sums)
            .map(|(&l, s)| (l, s.into_iter().map(|(t, n)| (n > 0).then(|| t / n as f64)).collect()))
            .collect()
    }

    fn delete(&mut self, p: [f64; 2], neighbour: bool) -> Outcome {
        let Some(id) = self.at(p) else {
            return Outcome::stop("no object", "the point is on no object: nothing deleted");
        };
        let own: Vec<usize> = (0..self.lab.len()).filter(|&i| self.lab[i] == id).collect();
        let mut target = 0u32;
        let mut alone = false;
        if neighbour {
            // the neighbours (4 neighbourhood) and the length of the border shared with each
            let mut border: BTreeMap<u32, usize> = BTreeMap::new();
            let (w, h) = (self.w, self.h);
            for &i in &own {
                let (r, c) = (i / w, i % w);
                let mut see = |j: usize| {
                    let l = self.lab[j];
                    if l != 0 && l != id {
                        *border.entry(l).or_insert(0) += 1;
                    }
                };
                if r > 0 {
                    see(i - w);
                }
                if r + 1 < h {
                    see(i + w);
                }
                if c > 0 {
                    see(i - 1);
                }
                if c + 1 < w {
                    see(i + 1);
                }
            }
            if border.is_empty() {
                alone = true;
            } else {
                let ids: Vec<u32> = std::iter::once(id).chain(border.keys().copied()).collect();
                let means = if self.bands.is_empty() { HashMap::new() } else { self.means(&ids) };
                // (distance, then the longer border, then the smaller id)
                let mut best: Option<(f64, usize, u32)> = None;
                for (&l, &len) in &border {
                    let d = match (means.get(&id), means.get(&l)) {
                        (Some(a), Some(b)) => {
                            let (mut s, mut n) = (0.0, 0usize);
                            for (x, y) in a.iter().zip(b) {
                                if let (Some(x), Some(y)) = (x, y) {
                                    s += (x - y) * (x - y);
                                    n += 1;
                                }
                            }
                            if n > 0 { s } else { f64::INFINITY }
                        }
                        _ => f64::INFINITY,
                    };
                    let better = match best {
                        None => true,
                        Some((bd, bl, _)) => d < bd || (d == bd && len > bl),
                    };
                    if better {
                        best = Some((d, len, l));
                    }
                }
                target = best.map_or(0, |b| b.2);
            }
        }
        for &i in &own {
            self.lab[i] = target;
        }
        if alone {
            return Outcome {
                status: "no neighbour",
                cells: own.len(),
                why: Some("the object has no neighbour to join: left with no object".into()),
            };
        }
        Outcome::done(own.len())
    }
}

/// `objects.edit`.
struct Edit;

impl Operator for Edit {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.edit",
            "Objects corrected by hand: merge, cut and delete, kept as a list of edits by places (coordinates on the \
             objects' grid) and not by object numbers, so the same list finds its objects again when the segmentation \
             above changes. `edits` is applied in order: { \"merge\": [[x, y], ...] } (the objects at the points \
             become the first one's), { \"cut\": [[x, y], ...] } (a line of two points or more splits each object it \
             crosses into the pieces left without its cells; each cell of the line goes to a piece next to it) and \
             { \"delete\": [x, y], \"into\": \"none\" | \"neighbour\" } (the object is left with no object, or taken \
             by the most similar neighbour: the nearest band means with an image, else the longest shared border). An \
             edit that finds no object, merges objects that are one, or cuts nothing is warned about and has a row \
             in `report`. The objects are numbered again, with no classes and no level above: put this node right \
             after the segmenter, before objects.features and the classification.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects to edit"))
        .input(PortSpec::optional(
            "image",
            Type::Raster,
            "the image of the objects (on their grid): the similarity of a delete into a neighbour, and the table of \
             band means",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the edited objects, numbered again, without classes"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "the objects' table: id, cells, area (and the band means with an image)",
        ))
        .output(PortSpec::new("report", Type::Table, "a row per edit: its kind, how it ended and the cells it changed"))
        .param(PortSpec::optional(
            "edits",
            Type::Any,
            "the edits, in order: [{ \"merge\": [[x, y], ...] }, { \"cut\": [[x, y], ...] }, { \"delete\": [x, y], \
             \"into\": \"none\" | \"neighbour\" }, ...]",
        ))
        .param(PortSpec::optional(
            "neighbours",
            Type::Number,
            "4 or 8: the cells that make an object one (default: 4, or 8 when the objects need it to be whole)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let given: serde_json::Value = args.opt_param("edits")?.unwrap_or(serde_json::Value::Null);
        let steps = parse(&given).map_err(|e| bad(e.0))?;
        let nbs: usize = match args.opt_param::<usize>("neighbours")? {
            Some(n) => n,
            None => {
                // (8 when the objects are not whole with 4: the watershed's, say, so that nothing splits unasked)
                let region: Vec<u64> = o.labels.iter().map(|&l| if l == 0 { NO_REGION } else { l as u64 }).collect();
                if Objects::from_regions(o.grid.clone(), &region, 4).count == o.count { 4 } else { 8 }
            }
        };
        if nbs != 4 && nbs != 8 {
            return Err(bad(format!("neighbours {nbs}: 4 or 8")));
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
        let bands: &[Arc<Vec<f64>>] = image.as_ref().map_or(&[], |l| l.data.as_slice());
        let mut st = State {
            w: o.grid.width,
            h: o.grid.height,
            eight: nbs == 8,
            lab: o.labels.to_vec(),
            next: o.count as u32,
            grid: &o.grid,
            bands,
        };
        let mut report = Table::new(vec!["edit".into(), "kind".into(), "status".into(), "cells".into()]);
        let mut kinds: Vec<String> = Vec::new();
        for (k, step) in steps.iter().enumerate() {
            let out = match step {
                Step::Merge(p) => st.merge(p),
                Step::Cut(p) => st.cut(p),
                Step::Delete(p, into) => st.delete(*p, *into),
            };
            if let Some(why) = &out.why {
                ctx.warn(format!("node {node:?}: edit {} ({}): {why}", k + 1, step.kind()));
            }
            let kind = match kinds.iter().position(|n| n == step.kind()) {
                Some(j) => j,
                None => {
                    kinds.push(step.kind().to_string());
                    kinds.len() - 1
                }
            };
            let status = STATUSES.iter().position(|s| *s == out.status).unwrap_or(0);
            report.rows.push(vec![(k + 1) as f64, (kind + 1) as f64, (status + 1) as f64, out.cells as f64]);
        }
        report.labels.insert("kind".to_string(), kinds);
        report.labels.insert("status".to_string(), STATUSES.iter().map(|s| s.to_string()).collect());
        let region: Vec<u64> = st.lab.iter().map(|&l| if l == 0 { NO_REGION } else { l as u64 }).collect();
        let mut edited = Objects::from_regions(o.grid.clone(), &region, nbs);
        if let Some(l) = &image {
            let t = stats_table(&edited, &l.names, &l.data);
            edited = edited.with_table(t)?;
        }
        let table = edited.table.clone();
        let mut out = outputs([("table", Value::Table(table)), ("report", Value::Table(report))]);
        out.insert("objects".into(), Value::Objects(Arc::new(edited)));
        Ok(out)
    }
}
