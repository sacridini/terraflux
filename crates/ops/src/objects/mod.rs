//! Objects: an image divided into objects (segments), as object-based image analysis (eCognition's) works on it.
//! [`Objects`] holds each cell's object, the objects' cells, boxes and neighbours, their attributes (a table, a row
//! per object), their classes and the level above. `segment.*` make objects, `objects.*` read and change them.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use terraflux_raster::Grid;

use crate::{Args, Error, Registry, Result, Table, Value};

mod convert;
mod esp;
mod features;
mod meanshift;
mod multiresolution;
mod segment;
mod store;
mod superpixels;
mod watershed;

pub fn register(r: &mut Registry) {
    segment::register(r);
    convert::register(r);
    r.add(Arc::new(features::Features));
}

/// The region of a cell that is in none ([`Objects::from_regions`]).
pub const NO_REGION: u64 = u64::MAX;

/// A class of objects: its name and, optionally, the colour it is drawn in.
#[derive(Debug, Clone, PartialEq)]
pub struct ObjectClass {
    pub name: String,
    pub colour: Option<[u8; 3]>,
}

/// An image divided into objects.
#[derive(Clone)]
pub struct Objects {
    pub grid: Arc<Grid>,
    /// Each cell's object: 0 none (no data), 1..=count; ids contiguous, numbered in the order of their first cell
    /// row by row.
    pub labels: Arc<Vec<u32>>,
    pub count: usize,
    /// Per object (index id - 1): its cells.
    pub cells: Vec<u32>,
    /// Per object: row min, column min, row max, column max (inclusive).
    pub boxes: Vec<[u32; 4]>,
    /// Per object: (neighbour id, length of the shared border in cell edges, 4-neighbourhood), sorted by id.
    pub adjacency: Vec<Vec<(u32, u32)>>,
    /// The level above (each object of this level inside exactly one of it) and each object's parent id.
    pub parent: Option<Arc<Objects>>,
    pub parent_of: Vec<u32>,
    /// Attributes: one row per object in id order, the first column `id`.
    pub table: Table,
    /// The classes; `class_of` per object, 0 unclassified, k the class `classes[k - 1]`.
    pub classes: Vec<ObjectClass>,
    pub class_of: Vec<u32>,
    /// The membership of each object to each class (outer: class, inner: object), empty when none.
    pub membership: Vec<Vec<f64>>,
}

/// The root of a label's set (its path halved on the way).
fn find(parent: &mut [u32], mut x: u32) -> u32 {
    while parent[x as usize] != x {
        parent[x as usize] = parent[parent[x as usize] as usize];
        x = parent[x as usize];
    }
    x
}

impl Objects {
    /// Objects from regions: the cells of each region split into their connected components (4 or 8 neighbours), so
    /// each object is connected; ids in the order of an object's first cell, row by row. `region` holds a number per
    /// cell, [`NO_REGION`] where there is none. One pass of union-find, no scan per object.
    pub fn from_regions(grid: Arc<Grid>, region: &[u64], neighbours: usize) -> Objects {
        let (w, h) = (grid.width, grid.height);
        assert_eq!(region.len(), w * h, "the regions are not one a cell of the grid");
        let eight = neighbours == 8;
        let mut prov = vec![0u32; region.len()];
        // the sets of provisional labels (label 0 is none)
        let mut parent: Vec<u32> = vec![0];
        for r in 0..h {
            for c in 0..w {
                let i = r * w + c;
                let reg = region[i];
                if reg == NO_REGION {
                    continue;
                }
                let mut label = 0u32;
                let mut see = |j: usize, parent: &mut Vec<u32>| {
                    if region[j] != reg {
                        return;
                    }
                    let l = find(parent, prov[j]);
                    if label == 0 {
                        label = l;
                    } else if l != label {
                        // the smaller label stays the root
                        let (lo, hi) = (l.min(label), l.max(label));
                        parent[hi as usize] = lo;
                        label = lo;
                    }
                };
                if c > 0 {
                    see(i - 1, &mut parent);
                }
                if r > 0 {
                    see(i - w, &mut parent);
                    if eight && c > 0 {
                        see(i - w - 1, &mut parent);
                    }
                    if eight && c + 1 < w {
                        see(i - w + 1, &mut parent);
                    }
                }
                if label == 0 {
                    label = parent.len() as u32;
                    parent.push(label);
                }
                prov[i] = label;
            }
        }
        // the final ids, in the order of the first cell of each set
        let mut id_of = vec![0u32; parent.len()];
        let mut count = 0u32;
        for p in prov.iter_mut() {
            if *p == 0 {
                continue;
            }
            let root = find(&mut parent, *p) as usize;
            if id_of[root] == 0 {
                count += 1;
                id_of[root] = count;
            }
            *p = id_of[root];
        }
        Objects::indexed(grid, prov, count as usize)
    }

    /// Objects from labels already made: 0 for none, ids 1..=count each on at least a cell. The labels are taken as
    /// they are (connected or not: that is the caller's).
    pub fn from_labels(grid: Arc<Grid>, labels: Vec<u32>, count: usize) -> Result<Objects> {
        if labels.len() != grid.cells() {
            return Err(Error::msg(format!("objects: {} labels for {} cells", labels.len(), grid.cells())));
        }
        let mut seen = vec![false; count + 1];
        for &l in &labels {
            let l = l as usize;
            if l > count {
                return Err(Error::msg(format!("objects: the id {l} is above the count {count}")));
            }
            seen[l] = true;
        }
        if let Some(missing) = seen.iter().skip(1).position(|s| !s) {
            return Err(Error::msg(format!("objects: the id {} has no cell", missing + 1)));
        }
        Ok(Objects::indexed(grid, labels, count))
    }

    /// The cells, boxes, adjacency and the table of ids of labels that are right.
    fn indexed(grid: Arc<Grid>, labels: Vec<u32>, count: usize) -> Objects {
        let (w, h) = (grid.width, grid.height);
        let mut cells = vec![0u32; count];
        let mut boxes = vec![[u32::MAX, u32::MAX, 0, 0]; count];
        let mut edges: Vec<u64> = Vec::new();
        for r in 0..h {
            for c in 0..w {
                let i = r * w + c;
                let a = labels[i];
                if a == 0 {
                    continue;
                }
                let k = a as usize - 1;
                cells[k] += 1;
                let b = &mut boxes[k];
                b[0] = b[0].min(r as u32);
                b[1] = b[1].min(c as u32);
                b[2] = b[2].max(r as u32);
                b[3] = b[3].max(c as u32);
                let mut edge = |j: usize| {
                    let o = labels[j];
                    if o != 0 && o != a {
                        edges.push(((a.min(o) as u64) << 32) | a.max(o) as u64);
                    }
                };
                if c + 1 < w {
                    edge(i + 1);
                }
                if r + 1 < h {
                    edge(i + w);
                }
            }
        }
        edges.sort_unstable();
        let mut adjacency: Vec<Vec<(u32, u32)>> = vec![Vec::new(); count];
        let mut e = 0;
        while e < edges.len() {
            let mut end = e;
            while end < edges.len() && edges[end] == edges[e] {
                end += 1;
            }
            let (a, b) = ((edges[e] >> 32) as u32, edges[e] as u32);
            let length = (end - e) as u32;
            // (a < b: the keys come sorted, so each list stays sorted by id)
            adjacency[a as usize - 1].push((b, length));
            adjacency[b as usize - 1].push((a, length));
            e = end;
        }
        let mut table = Table::new(vec!["id".to_string()]);
        table.rows = (1..=count).map(|id| vec![id as f64]).collect();
        Objects {
            grid,
            labels: Arc::new(labels),
            count,
            cells,
            boxes,
            adjacency,
            parent: None,
            parent_of: Vec::new(),
            table,
            classes: Vec::new(),
            class_of: vec![0; count],
            membership: Vec::new(),
        }
    }

    /// The object at a cell: 0 none (or off the grid).
    pub fn id_at(&self, row: usize, col: usize) -> u32 {
        if row >= self.grid.height || col >= self.grid.width {
            return 0;
        }
        self.labels[row * self.grid.width + col]
    }

    /// The id of an object's parent in the level above, if there is a level and the object is in one of its objects.
    pub fn parent_id(&self, id: u32) -> Option<u32> {
        self.parent.as_ref()?;
        let p = *self.parent_of.get((id as usize).checked_sub(1)?)?;
        (p != 0).then_some(p)
    }

    /// The name of an object's class: none when it is unclassified (or there is no such object).
    pub fn class_name(&self, id: u32) -> Option<&str> {
        let k = *self.class_of.get((id as usize).checked_sub(1)?)? as usize;
        self.classes.get(k.checked_sub(1)?).map(|c| c.name.as_str())
    }

    /// The neighbours of an object: (id, shared border in cell edges), by id.
    pub fn neighbours_of(&self, id: u32) -> &[(u32, u32)] {
        (id as usize).checked_sub(1).and_then(|k| self.adjacency.get(k)).map_or(&[], |a| a.as_slice())
    }

    /// The same objects with another table of attributes: a row per object, the first column `id`.
    pub fn with_table(mut self, table: Table) -> Result<Objects> {
        if table.rows.len() != self.count {
            return Err(Error::msg(format!(
                "objects: a table of {} rows for {} objects",
                table.rows.len(),
                self.count
            )));
        }
        if table.columns.first().map(String::as_str) != Some("id") {
            return Err(Error::msg("objects: the first column of the table is id"));
        }
        self.table = table;
        Ok(self)
    }

    /// The same objects with classes: a class per object (0 none, k the class k), and the memberships (empty, or a
    /// list per class of a value per object).
    pub fn with_classes(
        mut self,
        classes: Vec<ObjectClass>,
        class_of: Vec<u32>,
        membership: Vec<Vec<f64>>,
    ) -> Result<Objects> {
        if class_of.len() != self.count {
            return Err(Error::msg(format!("objects: {} classes for {} objects", class_of.len(), self.count)));
        }
        if let Some(k) = class_of.iter().find(|&&k| k as usize > classes.len()) {
            return Err(Error::msg(format!("objects: the class {k} of {} classes", classes.len())));
        }
        if !membership.is_empty()
            && (membership.len() != classes.len() || membership.iter().any(|m| m.len() != self.count))
        {
            return Err(Error::msg("objects: the memberships are one a class and an object"));
        }
        self.classes = classes;
        self.class_of = class_of;
        self.membership = membership;
        Ok(self)
    }

    /// The same objects inside those of a level above: `parent_of` gives each object's parent (0 none).
    pub fn with_parent(mut self, parent: Arc<Objects>, parent_of: Vec<u32>) -> Result<Objects> {
        if parent_of.len() != self.count {
            return Err(Error::msg(format!("objects: {} parents for {} objects", parent_of.len(), self.count)));
        }
        if let Some(p) = parent_of.iter().find(|&&p| p as usize > parent.count) {
            return Err(Error::msg(format!("objects: the parent {p} of {} objects", parent.count)));
        }
        self.parent = Some(parent);
        self.parent_of = parent_of;
        Ok(self)
    }

    /// The area of one cell in map units squared.
    pub fn cell_area(&self) -> f64 {
        let (rw, rh) = self.grid.resolution();
        rw * rh
    }

    /// About how much memory the objects hold, the levels above too.
    pub fn bytes(&self) -> usize {
        let table: usize = self.table.rows.iter().map(|r| r.len() * 8).sum();
        let adjacency: usize = self.adjacency.iter().map(|a| a.len() * 8).sum();
        let membership: usize = self.membership.iter().map(|m| m.len() * 8).sum();
        self.labels.len() * 4
            + self.count * (4 + 16 + 4 + 4)
            + table
            + adjacency
            + membership
            + self.parent.as_ref().map_or(0, |p| p.bytes())
    }

    /// Feed a hash with what the objects hold (labels, table, classes, memberships, the levels above): equal objects
    /// give the same, whatever their Arcs. What is made from the labels (cells, boxes, adjacency) is left out.
    pub fn content_hash<H: Hasher>(&self, h: &mut H) {
        format!("{:?}", self.grid).hash(h);
        words(&self.labels).hash(h);
        self.count.hash(h);
        self.table.columns.hash(h);
        self.table.rows.len().hash(h);
        for r in &self.table.rows {
            r.len().hash(h);
            for x in r {
                x.to_bits().hash(h);
            }
        }
        self.table.labels.hash(h);
        self.classes.len().hash(h);
        for c in &self.classes {
            (&c.name, c.colour).hash(h);
        }
        self.class_of.hash(h);
        self.membership.len().hash(h);
        for m in &self.membership {
            m.len().hash(h);
            for x in m {
                x.to_bits().hash(h);
            }
        }
        self.parent_of.hash(h);
        match &self.parent {
            Some(p) => {
                1u8.hash(h);
                p.content_hash(h);
            }
            None => 0u8.hash(h),
        }
    }

    /// The text of the objects' summary for people: how many, on what grid, the attributes and classes.
    pub fn describe(&self) -> String {
        let mut s = format!("{} objects on a {}x{} grid", self.count, self.grid.height, self.grid.width);
        let attrs: Vec<&str> = self.table.columns.iter().map(String::as_str).filter(|c| *c != "id").collect();
        if !attrs.is_empty() {
            s += &format!(", attributes: {}", attrs.join(", "));
        }
        if !self.classes.is_empty() {
            let names: Vec<&str> = self.classes.iter().map(|c| c.name.as_str()).collect();
            s += &format!(", classes: {}", names.join(", "));
        }
        if let Some(p) = &self.parent {
            s += &format!(", inside {} objects of the level above", p.count);
        }
        s
    }
}

/// The ids of a slice hashed four lanes at a time (not for anything but telling two values apart).
fn words(data: &[u32]) -> u64 {
    const K: u64 = 0x9E37_79B9_7F4A_7C15;
    let mut lanes = [K, K.rotate_left(16), K.rotate_left(32), K.rotate_left(48)];
    let (chunks, rest) = data.as_chunks::<4>();
    for c in chunks {
        for (l, x) in lanes.iter_mut().zip(c) {
            *l = (l.rotate_left(23) ^ *x as u64).wrapping_mul(K);
        }
    }
    for x in rest {
        lanes[0] = (lanes[0].rotate_left(23) ^ *x as u64).wrapping_mul(K);
    }
    let mut h = data.len() as u64;
    for l in lanes {
        h = (h.rotate_left(29) ^ l).wrapping_mul(K);
    }
    h ^ (h >> 32)
}

impl std::fmt::Debug for Objects {
    /// Cheap: the count, the grid's shape and a hash of the labels (not every label).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Objects({} objects, {}x{}, labels {:016x}, {} classes{})",
            self.count,
            self.grid.height,
            self.grid.width,
            words(&self.labels),
            self.classes.len(),
            if self.parent.is_some() { ", with a parent level" } else { "" }
        )
    }
}

/// The names of the bands of an image: an empty name is `b<k>`.
pub(crate) fn band_names(names: &[String]) -> Vec<String> {
    names.iter().enumerate().map(|(k, s)| if s.is_empty() { format!("b{}", k + 1) } else { s.clone() }).collect()
}

/// The table of segmenters over an image: id, cells, area (map units squared), and mean_<band> and sd_<band> (the
/// population standard deviation) over the cells of each object that have a value in the band (NaN for none).
pub(crate) fn stats_table(o: &Objects, names: &[String], data: &[Arc<Vec<f64>>]) -> Table {
    let names = band_names(names);
    let nb = data.len();
    let mut columns: Vec<String> = ["id", "cells", "area"].map(String::from).to_vec();
    columns.extend(names.iter().map(|s| format!("mean_{s}")));
    columns.extend(names.iter().map(|s| format!("sd_{s}")));
    // sums of the values less the band's mean (the rounding error stays small): count, sum, sum of squares
    let mut st = vec![0.0f64; o.count * nb * 3];
    for (j, b) in data.iter().enumerate() {
        let (s, c) = b.iter().filter(|v| !v.is_nan()).fold((0.0, 0usize), |(s, c), v| (s + v, c + 1));
        let centre = if c > 0 { s / c as f64 } else { 0.0 };
        for (&l, &v) in o.labels.iter().zip(b.iter()) {
            if l != 0 && !v.is_nan() {
                let (d, k) = (v - centre, ((l as usize - 1) * nb + j) * 3);
                st[k] += 1.0;
                st[k + 1] += d;
                st[k + 2] += d * d;
            }
        }
        for k in 0..o.count {
            let at = (k * nb + j) * 3;
            // the sum becomes the mean and the sum of squares the deviation
            if st[at] > 0.0 {
                let m = st[at + 1] / st[at];
                st[at + 2] = (st[at + 2] / st[at] - m * m).max(0.0).sqrt();
                st[at + 1] = centre + m;
            } else {
                st[at + 1] = f64::NAN;
                st[at + 2] = f64::NAN;
            }
        }
    }
    let area = o.cell_area();
    let mut t = Table::new(columns);
    t.rows = (0..o.count)
        .map(|k| {
            let mut row = vec![(k + 1) as f64, o.cells[k] as f64, o.cells[k] as f64 * area];
            row.extend((0..nb).map(|j| st[(k * nb + j) * 3 + 1]));
            row.extend((0..nb).map(|j| st[(k * nb + j) * 3 + 2]));
            row
        })
        .collect();
    t
}

/// The objects an input holds, or why it holds none.
pub(crate) fn objects_in<'a>(args: &'a Args, port: &str) -> Result<&'a Arc<Objects>> {
    match args.get(port) {
        Some(Value::Objects(o)) => Ok(o),
        Some(v) => Err(Error::msg(format!("node {:?}: the input {port:?} is a {:?}, not objects", args.node, v.ty()))),
        None => Err(Error::msg(format!("node {:?}: no input {port:?}", args.node))),
    }
}
