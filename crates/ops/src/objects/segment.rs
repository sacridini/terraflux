//! `segment.*`: making objects. `segment.from_map` takes the result of any segmenter run elsewhere.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::Column;

use super::{NO_REGION, Objects};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(FromMap));
    r.add(Arc::new(Chessboard));
    r.add(Arc::new(Quadtree));
    r.add(Arc::new(FromVector));
    r.add(Arc::new(super::multiresolution::Multiresolution));
    super::superpixels::register(r);
    super::watershed::register(r);
    super::threshold::register(r);
    super::spectral::register(r);
    super::growing::register(r);
    super::quickshift::register(r);
    super::meanshift::register(r);
    r.add(Arc::new(super::esp::Scale));
    super::optimize::register(r);
}

/// A table of id, cells and area for objects, set on them: the objects and the table as outputs.
fn square_outputs(o: Objects) -> Result<Outputs> {
    let area = o.cell_area();
    let mut t = Table::new(["id", "cells", "area"].map(String::from).to_vec());
    t.rows = (0..o.count).map(|k| vec![(k + 1) as f64, o.cells[k] as f64, o.cells[k] as f64 * area]).collect();
    let o = o.with_table(t.clone())?;
    Ok(outputs([("objects", Value::Objects(Arc::new(o))), ("table", Value::Table(t))]))
}

/// The cells of an image where at least a band has a value.
fn valid_cells(data: &[Arc<Vec<f64>>], n: usize) -> Vec<bool> {
    (0..n).map(|i| data.iter().any(|b| !b[i].is_nan())).collect()
}

/// `segment.chessboard`.
struct Chessboard;

impl Operator for Chessboard {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.chessboard",
            "Objects as a chessboard: the image cut into squares of a size in cells, from its top-left corner (the \
             squares at the right and bottom edges are clipped). The simplest segmentation, with no regard to what \
             the image holds, to start from or to give the scale of a first level. A cell without a value in every \
             band is no object, and a square cut in parts by such cells is an object for each part.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands: only its grid and its empty cells count"))
        .output(PortSpec::new("objects", Type::Objects, "the squares as objects"))
        .output(PortSpec::new("table", Type::Table, "a row per object: id, cells, area (map units squared)"))
        .param(PortSpec::optional(
            "size",
            Type::Integer,
            "the side of a square in cells, 1 or more (default 10)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let size: i64 = args.param("size", 10)?;
        if size < 1 {
            return Err(Error::msg(format!("node {:?}: size {size}: 1 cell or more", args.node)));
        }
        let size = size as usize;
        let l = crate::raster_stats::layers(args, "image")?;
        let (w, h) = (l.grid.width, l.grid.height);
        let valid = valid_cells(&l.data, w * h);
        let across = w.div_ceil(size);
        let region: Vec<u64> = (0..w * h)
            .map(|i| if valid[i] { ((i / w / size) * across + (i % w) / size) as u64 } else { NO_REGION })
            .collect();
        square_outputs(Objects::from_regions(l.grid.clone(), &region, 4))
    }
}

/// `segment.quadtree`.
struct Quadtree;

/// The state of a quadtree's split: the bands, the stopping rules and the leaves made.
struct Split<'a> {
    bands: &'a [Arc<Vec<f64>>],
    w: usize,
    h: usize,
    threshold: f64,
    min_size: usize,
    region: Vec<u64>,
    next: u64,
}

impl Split<'_> {
    /// A square (clipped at the edges): split in four while its colour difference is above the threshold and the
    /// halves are not under the smallest size, else one leaf.
    fn square(&mut self, r0: usize, c0: usize, size: usize) {
        let (r1, c1) = ((r0 + size).min(self.h), (c0 + size).min(self.w));
        let (mut range, mut any) = (0.0f64, false);
        for b in self.bands {
            let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
            for r in r0..r1 {
                for &v in &b[r * self.w + c0..r * self.w + c1] {
                    if !v.is_nan() {
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            }
            if lo <= hi {
                any = true;
                range = range.max(hi - lo);
            }
        }
        if !any {
            return;
        }
        let half = size / 2;
        if half >= self.min_size.max(1) && range > self.threshold {
            for (r, c) in [(r0, c0), (r0, c0 + half), (r0 + half, c0), (r0 + half, c0 + half)] {
                if r < self.h && c < self.w {
                    self.square(r, c, half);
                }
            }
            return;
        }
        let id = self.next;
        self.next += 1;
        for r in r0..r1 {
            for c in c0..c1 {
                let i = r * self.w + c;
                if self.bands.iter().any(|b| !b[i].is_nan()) {
                    self.region[i] = id;
                }
            }
        }
    }
}

impl Operator for Quadtree {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.quadtree",
            "Objects by a quadtree: the image is one square (or squares of the largest size), split into four while \
             the colour difference inside is above a threshold, down to a smallest size. Even areas stay in big \
             squares and varied ones in small squares. The colour difference is the largest, over the bands, of the \
             highest value minus the lowest among a square's cells (eCognition's quadtree-based segmentation uses \
             the colour difference the same way). A cell without a value in every band is no object. Squares are of \
             sizes of a power of two, clipped at the right and bottom edges; the result is always the same.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, the colour"))
        .output(PortSpec::new("objects", Type::Objects, "the squares as objects"))
        .output(PortSpec::new("table", Type::Table, "a row per object: id, cells, area (map units squared)"))
        .param(PortSpec::new(
            "threshold",
            Type::Number,
            "a square is split while, in any band, its highest value minus its lowest is above this (in the image's units)",
        ))
        .param(PortSpec::optional("min_size", Type::Integer, "the smallest square's side in cells (default 1)"))
        .param(PortSpec::optional(
            "max_size",
            Type::Integer,
            "the largest square's side in cells, taken down to a power of two (default: the smallest power of two \
             covering the image)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let threshold = args
            .opt_param::<f64>("threshold")?
            .ok_or_else(|| Error::msg(format!("node {node:?}: the setting threshold is needed")))?;
        if !threshold.is_finite() || threshold < 0.0 {
            return Err(Error::msg(format!("node {node:?}: threshold {threshold}: 0 or more")));
        }
        let min_size: i64 = args.param("min_size", 1)?;
        if min_size < 1 {
            return Err(Error::msg(format!("node {node:?}: min_size {min_size}: 1 cell or more")));
        }
        let l = crate::raster_stats::layers(args, "image")?;
        super::warnings::image(ctx, args.node, &l.grid, &l.names, &l.data, false);
        let (w, h) = (l.grid.width, l.grid.height);
        let cover = w.max(h).max(1).next_power_of_two();
        let start = match args.opt_param::<i64>("max_size")? {
            None => cover,
            Some(m) if m >= 1 => ((m as usize + 1).next_power_of_two() / 2).min(cover),
            Some(m) => return Err(Error::msg(format!("node {node:?}: max_size {m}: 1 cell or more"))),
        };
        let mut s = Split {
            bands: &l.data,
            w,
            h,
            threshold,
            min_size: min_size as usize,
            region: vec![NO_REGION; w * h],
            next: 0,
        };
        for r in (0..h).step_by(start) {
            for c in (0..w).step_by(start) {
                s.square(r, c, start);
            }
        }
        square_outputs(Objects::from_regions(l.grid.clone(), &s.region, 4))
    }
}

/// `segment.from_vector`.
struct FromVector;

impl Operator for FromVector {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.from_vector",
            "Objects from polygons: each polygon is laid on the grid of a map (a cell belongs to the polygon holding \
             its centre) and becomes an object, or several when it falls in parts that do not touch (a multipolygon \
             of two areas, or a polygon cut by the map's edge). Where polygons overlap, the later one wins the \
             cells. Cells in no polygon are no object. The way in of parcels, stands or segments drawn elsewhere \
             (the polygons of another software's segmentation); their numeric attributes come to the table, and \
             their text attributes too, as names.",
            Access::Global,
        )
        .input(PortSpec::new("vector", Type::Vector, "the polygons (in the map's coordinate system, or reprojected)"))
        .input(PortSpec::new("like", Type::Map, "a map whose grid the objects take"))
        .output(PortSpec::new("objects", Type::Objects, "the objects"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, feature (the polygon, 1..), cells, area (map units squared) and the attributes",
        ))
        .param(
            PortSpec::optional("neighbours", Type::Integer, "which cells touch: 4 (default, sharing an edge) or 8")
                .choices([4, 8]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let n: usize = args.param("neighbours", 4)?;
        if n != 4 && n != 8 {
            return Err(Error::msg(format!("node {node:?}: neighbours {n}: 4 or 8")));
        }
        let like = args.map("like")?;
        let v = crate::vector::on_grid(crate::vector::vector_of(args, "vector")?, &like.grid)?;
        if let Some(k) = v.geometries.iter().position(|g| !g.is_polygonal()) {
            return Err(Error::msg(format!("node {node:?}: feature {} is not a polygon", k + 1)));
        }
        // the feature (1..) of each cell, the later winning
        let mut burnt = vec![f64::NAN; like.grid.cells()];
        let values: Vec<f64> = (1..=v.len()).map(|k| k as f64).collect();
        if !v.is_empty() {
            crate::vector::burn(&mut burnt, &like.grid, &v, &values, false, false)
                .map_err(|e| Error::msg(format!("node {node:?}: {e}")))?;
        }
        let region: Vec<u64> = burnt.iter().map(|&x| if x.is_nan() { NO_REGION } else { x as u64 }).collect();
        let o = Objects::from_regions(like.grid.clone(), &region, n);
        // each object's feature: the one of its first cell
        let mut feature = vec![0usize; o.count];
        let mut seen = vec![false; o.count];
        for (&l, &x) in o.labels.iter().zip(burnt.iter()) {
            if l != 0 && !seen[l as usize - 1] {
                seen[l as usize - 1] = true;
                feature[l as usize - 1] = x as usize - 1;
            }
        }
        let area = o.cell_area();
        let mut columns: Vec<String> = ["id", "feature", "cells", "area"].map(String::from).to_vec();
        let mut t = Table::new(columns.clone());
        t.rows = (0..o.count)
            .map(|k| vec![(k + 1) as f64, (feature[k] + 1) as f64, o.cells[k] as f64, o.cells[k] as f64 * area])
            .collect();
        for (name, col) in &v.attributes {
            let mut name = name.clone();
            while columns.contains(&name) {
                name.push('_');
            }
            columns.push(name.clone());
            t.columns.push(name.clone());
            match col {
                Column::Numbers(x) => {
                    for (k, row) in t.rows.iter_mut().enumerate() {
                        row.push(x[feature[k]]);
                    }
                }
                Column::Texts(x) => {
                    // a code per distinct text, in the order they come (an empty text has none)
                    let mut names: Vec<String> = Vec::new();
                    for (k, row) in t.rows.iter_mut().enumerate() {
                        let s = &x[feature[k]];
                        if s.is_empty() {
                            row.push(f64::NAN);
                            continue;
                        }
                        let code = names.iter().position(|q| q == s).unwrap_or_else(|| {
                            names.push(s.clone());
                            names.len() - 1
                        });
                        row.push((code + 1) as f64);
                    }
                    t.labels.insert(name, names);
                }
            }
        }
        let o = o.with_table(t.clone())?;
        Ok(outputs([("objects", Value::Objects(Arc::new(o))), ("table", Value::Table(t))]))
    }
}

/// `segment.from_map`.
struct FromMap;

impl Operator for FromMap {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.from_map",
            "Objects from a map: every group of connected cells of one value becomes an object (ids 1.., in the order \
             of their first cell, row by row); the cells without a value are no object. The way in of any segmenter's \
             result (a map of segments or of classes made elsewhere), with a table of each object's value, cells and \
             area.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the segments or classes: a value for each region"))
        .output(PortSpec::new("objects", Type::Objects, "the objects"))
        .output(PortSpec::new("table", Type::Table, "a row per object: id, value, cells, area (map units squared)"))
        .param(
            PortSpec::optional("neighbours", Type::Integer, "which cells touch: 4 (default, sharing an edge) or 8")
                .choices([4, 8]),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let n: usize = args.param("neighbours", 4)?;
        if n != 4 && n != 8 {
            return Err(Error::msg(format!("node {:?}: neighbours {n}: 4 or 8", args.node)));
        }
        // a region by the bits of its value (-0 as 0)
        let region: Vec<u64> =
            m.data.iter().map(|&v| if v.is_nan() { NO_REGION } else { (v + 0.0).to_bits() }).collect();
        let o = Objects::from_regions(m.grid.clone(), &region, n);
        // each object's value: the map's at its first cell
        let mut value = vec![f64::NAN; o.count];
        let mut seen = vec![false; o.count];
        for (&l, &v) in o.labels.iter().zip(m.data.iter()) {
            if l != 0 && !seen[l as usize - 1] {
                seen[l as usize - 1] = true;
                value[l as usize - 1] = v;
            }
        }
        let area = o.cell_area();
        let mut t = Table::new(["id", "value", "cells", "area"].map(String::from).to_vec());
        t.rows =
            (0..o.count).map(|k| vec![(k + 1) as f64, value[k], o.cells[k] as f64, o.cells[k] as f64 * area]).collect();
        let o = o.with_table(t.clone())?;
        Ok(outputs([("objects", Value::Objects(Arc::new(o))), ("table", Value::Table(t))]))
    }
}
