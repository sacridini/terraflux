//! Vector features drawn over a grid as tiles, so that layers of millions of features draw and move as smoothly as
//! a map: a layer is prepared once on a thread of its own (reprojected to the grid's coordinate system, its
//! vertices put in the grid's cells, an R-tree of its features' boxes, the box of every run of 64 vertices), then
//! threads rasterize the tiles in view (256 x 256 pixels, antialiased by tiny-skia) from the features the tree finds
//! over each, every vertex closer than a pixel to the last drawn skipped, a run off the tile or smaller than a pixel
//! drawn as one line (a tile deep in a feature of a hundred thousand vertices draws the few over it, not them all)
//! and features smaller than a pixel drawn as a dot. The screen only draws
//! textures; until a tile comes the finest coarser one kept is shown, stretched. The feature under the pointer is
//! found by the tree too.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};

use egui::{Color32, Pos2, Rect, TextureHandle, TextureOptions, Vec2};
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{AABB, Envelope, RTree, RTreeObject};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::palette::Ramp;

/// The side of a tile, pixels.
const TILE: usize = 256;
/// The tiles kept, textures of 256 KB: 128 MB at most.
const KEPT: usize = 512;
/// The finest level: a tile's pixel 1/256 of a cell.
const FINEST: i32 = -8;
/// The vertices of a run of a part, boxed: a tile draws a run away from it, or smaller than a pixel, as the line
/// from its first vertex to its last.
const RUN: usize = 64;

/// How the numbers of an attribute become colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Breaks {
    /// The colour follows the value along the ramp (a few whole numbers: a class each).
    #[default]
    Continuous,
    /// Classes as wide as each other.
    Equal,
    /// Classes of as many features each.
    Quantiles,
    /// Classes where the values gather (Jenks, Fisher's optimal split).
    Natural,
}

impl Breaks {
    pub const ALL: [Breaks; 4] = [Breaks::Continuous, Breaks::Equal, Breaks::Quantiles, Breaks::Natural];

    /// As a Plot keeps it.
    pub fn key(self) -> &'static str {
        match self {
            Breaks::Continuous => "continuous",
            Breaks::Equal => "equal",
            Breaks::Quantiles => "quantiles",
            Breaks::Natural => "natural",
        }
    }

    /// As the editor names it.
    pub fn name(self) -> &'static str {
        match self {
            Breaks::Continuous => "continuous",
            Breaks::Equal => "equal intervals",
            Breaks::Quantiles => "quantiles",
            Breaks::Natural => "natural (Jenks)",
        }
    }

    /// By its key or its name (or a usual other: "jenks", "quantile", "equal intervals").
    pub fn named(text: &str) -> Option<Breaks> {
        let t = text.trim().to_lowercase();
        Breaks::ALL.into_iter().find(|b| b.key() == t || b.name() == t).or(match t.as_str() {
            "jenks" => Some(Breaks::Natural),
            "equal_interval" | "equal interval" => Some(Breaks::Equal),
            "quantile" => Some(Breaks::Quantiles),
            _ => None,
        })
    }
}

/// The shape of a point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Symbol {
    #[default]
    Circle,
    Square,
    Triangle,
    Diamond,
    Cross,
}

impl Symbol {
    pub const ALL: [Symbol; 5] = [Symbol::Circle, Symbol::Square, Symbol::Triangle, Symbol::Diamond, Symbol::Cross];

    /// As a Plot keeps it (and the editor names it).
    pub fn key(self) -> &'static str {
        match self {
            Symbol::Circle => "circle",
            Symbol::Square => "square",
            Symbol::Triangle => "triangle",
            Symbol::Diamond => "diamond",
            Symbol::Cross => "cross",
        }
    }

    pub fn named(text: &str) -> Option<Symbol> {
        let t = text.trim().to_lowercase();
        Symbol::ALL.into_iter().find(|s| s.key() == t)
    }

    /// The outline of the symbol around (0, 0), `r` its radius: the corners in order (a circle as 16 of them).
    pub fn outline(self, r: f32) -> Vec<(f32, f32)> {
        let polar = |n: usize, first: f32| -> Vec<(f32, f32)> {
            (0..n)
                .map(|i| {
                    let a = first + i as f32 / n as f32 * std::f32::consts::TAU;
                    (r * a.cos(), r * a.sin())
                })
                .collect()
        };
        match self {
            Symbol::Circle => polar(16, 0.0),
            Symbol::Diamond => polar(4, 0.0),
            // (about the circle's area: 0.886 r a half side)
            Symbol::Square => {
                let h = r * 0.886;
                vec![(-h, -h), (h, -h), (h, h), (-h, h)]
            }
            // (pointing up)
            Symbol::Triangle => polar(3, -std::f32::consts::FRAC_PI_2),
            Symbol::Cross => {
                let (l, t) = (r, r * 0.38);
                vec![
                    (-t, -l),
                    (t, -l),
                    (t, -t),
                    (l, -t),
                    (l, t),
                    (t, t),
                    (t, l),
                    (-t, l),
                    (-t, t),
                    (-l, t),
                    (-l, -t),
                    (-t, -t),
                ]
            }
        }
    }
}

/// The most distinct values Jenks' classes are worked out from (a layer of more is sampled, evenly).
pub const JENKS_SAMPLE: usize = 1500;
/// The most classes of numbers.
pub const MAX_CLASSES: usize = 12;

/// How a layer is drawn: a colour, or the colours of an attribute's values (categories, classes of numbers, or a
/// ramp), the points' symbol and their size by an attribute.
#[derive(Debug, Clone, PartialEq)]
pub struct Style {
    pub colour: Color32,
    pub by: Option<String>,
    /// The polygons' fill, 0 (none) to 1.
    pub fill: f32,
    /// The lines' and the polygons' outlines' width, pixels (0: polygons without outline); points are a circle of
    /// radius 2 + width pixels.
    pub width: f32,
    /// The attribute whose values label the features (none: no labels), and the labels' size, points.
    pub labels: Option<String>,
    pub label_size: f32,
    /// The colours of a number attribute: the ramp they run along.
    pub ramp: Ramp,
    /// How a number attribute is split into colours.
    pub breaks: Breaks,
    /// The number of classes of `Equal`, `Quantiles` and `Natural` breaks (2 to 12).
    pub classes: usize,
    /// A number attribute whose every value is a category (a text attribute always is).
    pub categories: bool,
    /// The points' size by a number attribute: between `size_min` and `size_max` pixels across, the area as the
    /// value (none: all of one size, the width's).
    pub size_by: Option<String>,
    pub size_min: f32,
    pub size_max: f32,
    pub symbol: Symbol,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            colour: crate::theme::accent(),
            by: None,
            fill: 0.2,
            width: 1.2,
            labels: None,
            label_size: 11.0,
            ramp: Ramp::Viridis,
            breaks: Breaks::Continuous,
            classes: 5,
            categories: false,
            size_by: None,
            size_min: 3.0,
            size_max: 18.0,
            symbol: Symbol::Circle,
        }
    }
}

impl Style {
    /// As a Plot keeps it: `{ colour = "#rrggbbaa", by, fill, width, labels, label_size, ramp, breaks, classes,
    /// categories, size_by, size_min, size_max, symbol }`, what is as by default left out.
    pub fn to_json(&self) -> serde_json::Value {
        let d = Style::default();
        let [r, g, b, a] = self.colour.to_array();
        let mut o = serde_json::json!({
            "colour": format!("#{r:02x}{g:02x}{b:02x}{a:02x}"),
            "fill": self.fill,
            "width": self.width,
        });
        if let Some(by) = &self.by {
            o["by"] = by.clone().into();
        }
        if let Some(l) = &self.labels {
            o["labels"] = l.clone().into();
            o["label_size"] = self.label_size.into();
        }
        if self.ramp != d.ramp
            && let Some(name) = crate::builder::palette_name(crate::builder::Palette::Chosen(self.ramp, false))
        {
            o["ramp"] = name.into();
        }
        if self.breaks != d.breaks {
            o["breaks"] = self.breaks.key().into();
        }
        if self.classes != d.classes {
            o["classes"] = self.classes.into();
        }
        if self.categories {
            o["categories"] = true.into();
        }
        if let Some(s) = &self.size_by {
            o["size_by"] = s.clone().into();
            o["size_min"] = self.size_min.into();
            o["size_max"] = self.size_max.into();
        }
        if self.symbol != d.symbol {
            o["symbol"] = self.symbol.key().into();
        }
        o
    }

    /// A style kept in a Plot ([`Style::to_json`]); what it does not say, as by default.
    pub fn from_json(v: &serde_json::Value) -> Style {
        let d = Style::default();
        let colour = v
            .get("colour")
            .and_then(|c| c.as_str())
            .and_then(|c| c.strip_prefix('#'))
            .filter(|h| h.len() == 8 && h.is_ascii())
            .and_then(|h| {
                let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
                Some(Color32::from_rgba_premultiplied(byte(0)?, byte(2)?, byte(4)?, byte(6)?))
            })
            .unwrap_or(d.colour);
        let number = |k: &str, or: f32| v.get(k).and_then(|x| x.as_f64()).map_or(or, |x| x as f32);
        let text = |k: &str| v.get(k).and_then(|b| b.as_str());
        let (size_min, size_max) = (number("size_min", d.size_min).max(0.5), number("size_max", d.size_max).max(0.5));
        Style {
            colour,
            by: text("by").map(String::from),
            fill: number("fill", d.fill),
            width: number("width", d.width),
            labels: text("labels").map(String::from),
            label_size: number("label_size", d.label_size),
            ramp: text("ramp")
                .and_then(crate::builder::palette_named)
                .and_then(|p| match p {
                    crate::builder::Palette::Chosen(r, _) => Some(r),
                    _ => None,
                })
                .unwrap_or(d.ramp),
            breaks: text("breaks").and_then(Breaks::named).unwrap_or(d.breaks),
            classes: v
                .get("classes")
                .and_then(|x| x.as_u64())
                .map_or(d.classes, |x| (x as usize).clamp(2, MAX_CLASSES)),
            categories: v.get("categories").and_then(|x| x.as_bool()).unwrap_or(false),
            size_by: text("size_by").map(String::from),
            size_min: size_min.min(size_max),
            size_max,
            symbol: text("symbol").and_then(Symbol::named).unwrap_or(d.symbol),
        }
    }

    /// What the drawing of the tiles depends on, hashed (not the labels).
    pub fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.colour.to_array().hash(&mut h);
        self.by.hash(&mut h);
        self.fill.to_bits().hash(&mut h);
        self.width.to_bits().hash(&mut h);
        self.ramp.hash(&mut h);
        (self.breaks as u8, self.classes, self.categories).hash(&mut h);
        self.size_by.hash(&mut h);
        self.size_min.to_bits().hash(&mut h);
        self.size_max.to_bits().hash(&mut h);
        (self.symbol as u8).hash(&mut h);
        h.finish()
    }
}

/// What a feature is drawn as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Point,
    Line,
    Polygon,
}

/// A feature: its kind, its parts (rings, lines or points: `parts[k]..parts[k + 1]` of the vertices) and box.
#[derive(Debug, Clone)]
struct Feature {
    kind: Kind,
    first: u32,
    last: u32,
    bbox: [f64; 4],
}

/// A layer ready to draw over a grid: its features in the grid's cells (x the column, y the row, from the corner).
pub struct Layer {
    pub vector: Arc<Vector>,
    features: Vec<Feature>,
    parts: Vec<u32>,
    xy: Vec<[f64; 2]>,
    /// The box of every run of `RUN` vertices of the parts (the last vertex of one the first of the next), and
    /// the first run of each part: `runs[run_of[k]..run_of[k + 1]]` the runs of part k.
    runs: Vec<[f64; 4]>,
    run_of: Vec<u32>,
    tree: RTree<GeomWithData<Rectangle<[f64; 2]>, u32>>,
    /// Where each feature is labelled, cells ([`crate::labels::anchor`]).
    anchors: Vec<Option<[f64; 2]>>,
    /// The box of every feature, cells.
    pub bounds: Option<[f64; 4]>,
}

/// A layer's features in a grid's cells: reprojected to its coordinate system when both are known, through the
/// inverse of its geotransform.
pub fn prepare(v: &Arc<Vector>, grid: &Grid) -> Result<Layer, String> {
    let projected = match (&v.crs, crs_of(grid)) {
        (Some(from), Some(to)) if !terraflux_gdal::same_crs(from, &to).unwrap_or(true) => {
            Arc::new(terraflux_gdal::reproject_vector(v, &to)?)
        }
        _ => v.clone(),
    };
    let t = grid.transform;
    let det = t[1] * t[5] - t[2] * t[4];
    if det == 0.0 {
        return Err("the grid's geotransform cannot be inverted".into());
    }
    let to_cell = |p: &[f64; 2]| {
        let (dx, dy) = (p[0] - t[0], p[1] - t[3]);
        [(dx * t[5] - dy * t[2]) / det, (dy * t[1] - dx * t[4]) / det]
    };
    let mut features = Vec::with_capacity(projected.len());
    let mut parts: Vec<u32> = vec![0];
    let mut xy: Vec<[f64; 2]> = Vec::new();
    let mut boxes = Vec::with_capacity(projected.len());
    for g in &projected.geometries {
        let first = (parts.len() - 1) as u32;
        let push = |pts: &[[f64; 2]], xy: &mut Vec<[f64; 2]>, parts: &mut Vec<u32>| {
            xy.extend(pts.iter().map(to_cell));
            parts.push(xy.len() as u32);
        };
        let kind = match g {
            Geometry::Empty => Kind::Point,
            Geometry::Point(p) => {
                push(&[*p], &mut xy, &mut parts);
                Kind::Point
            }
            Geometry::MultiPoint(ps) => {
                for p in ps {
                    push(&[*p], &mut xy, &mut parts);
                }
                Kind::Point
            }
            Geometry::LineString(l) => {
                push(l, &mut xy, &mut parts);
                Kind::Line
            }
            Geometry::MultiLineString(ls) => {
                for l in ls {
                    push(l, &mut xy, &mut parts);
                }
                Kind::Line
            }
            Geometry::Polygon(_) | Geometry::MultiPolygon(_) => {
                for poly in g.polygons() {
                    for ring in poly {
                        push(ring, &mut xy, &mut parts);
                    }
                }
                Kind::Polygon
            }
        };
        let last = (parts.len() - 1) as u32;
        let a = parts[first as usize] as usize;
        let b = parts[last as usize] as usize;
        let mut bbox = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for p in &xy[a..b] {
            bbox = [bbox[0].min(p[0]), bbox[1].min(p[1]), bbox[2].max(p[0]), bbox[3].max(p[1])];
        }
        let k = features.len() as u32;
        if a < b && bbox.iter().all(|v| v.is_finite()) {
            boxes.push(GeomWithData::new(Rectangle::from_corners([bbox[0], bbox[1]], [bbox[2], bbox[3]]), k));
        }
        features.push(Feature { kind, first, last, bbox });
    }
    let bounds = boxes.iter().map(|b| b.geom().envelope()).reduce(|a, b| a.merged(&b)).map(|e| {
        let (l, u) = (e.lower(), e.upper());
        [l[0], l[1], u[0], u[1]]
    });
    let tree = RTree::bulk_load(boxes);
    let mut runs = Vec::with_capacity(xy.len() / RUN + parts.len());
    let mut run_of = Vec::with_capacity(parts.len());
    for w in parts.windows(2) {
        run_of.push(runs.len() as u32);
        let (mut i, b) = (w[0] as usize, w[1] as usize);
        while i + 1 < b {
            let j = (i + RUN).min(b - 1);
            let mut bbox = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
            for p in &xy[i..=j] {
                bbox = [bbox[0].min(p[0]), bbox[1].min(p[1]), bbox[2].max(p[0]), bbox[3].max(p[1])];
            }
            runs.push(bbox);
            i = j;
        }
    }
    run_of.push(runs.len() as u32);
    let mut layer = Layer { vector: v.clone(), features, parts, xy, runs, run_of, tree, anchors: Vec::new(), bounds };
    layer.anchors = (0..layer.features.len())
        .map(|k| {
            let (kind, parts) = layer.shape(k)?;
            crate::labels::anchor(kind, &parts)
        })
        .collect();
    Ok(layer)
}

impl Layer {
    /// A feature's kind and its parts in cells (rings, lines or points), as it is drawn; none for a number it has
    /// not.
    pub fn shape(&self, k: usize) -> Option<(Kind, Vec<&[[f64; 2]]>)> {
        let f = self.features.get(k)?;
        let parts =
            (f.first..f.last).map(|i| &self.xy[self.parts[i as usize] as usize..self.parts[i as usize + 1] as usize]);
        Some((f.kind, parts.collect()))
    }

    /// A feature's kind.
    pub fn kind(&self, k: usize) -> Option<Kind> {
        self.features.get(k).map(|f| f.kind)
    }

    /// Where a feature is labelled, cells.
    pub fn anchor(&self, k: usize) -> Option<[f64; 2]> {
        self.anchors.get(k).copied().flatten()
    }

    /// The features whose boxes meet a rectangle of cells (corners), in their order.
    pub fn within(&self, a: [f64; 2], b: [f64; 2]) -> Vec<usize> {
        let env = AABB::from_corners(a, b);
        let mut found: Vec<usize> = self.tree.locate_in_envelope_intersecting(&env).map(|g| g.data as usize).collect();
        found.sort_unstable();
        found
    }

    /// The features that meet a polygon of cells (see [`crate::area::hits`]), in their order: those whose boxes
    /// meet its box first, then the exact test.
    pub fn in_shape(&self, shape: &[[f64; 2]]) -> Vec<usize> {
        let Some(b) = crate::attribute_table::union_box(shape.iter().map(|p| [p[0], p[1], p[0], p[1]])) else {
            return Vec::new();
        };
        self.within([b[0], b[1]], [b[2], b[3]])
            .into_iter()
            .filter(|&k| self.shape(k).is_some_and(|(kind, parts)| crate::area::hits(kind, &parts, shape)))
            .collect()
    }

    /// The box of a feature in cells (`[x0, y0, x1, y1]`); none when it has no vertex.
    pub fn feature_box(&self, k: usize) -> Option<[f64; 4]> {
        let b = self.features.get(k)?.bbox;
        b.iter().all(|v| v.is_finite()).then_some(b)
    }
}

fn crs_of(g: &Grid) -> Option<String> {
    g.geo.wkt.clone().or_else(|| g.geo.epsg().map(|c| format!("EPSG:{c}")))
}

/// What a legend shows of the colours: the colours of the values of an attribute.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Swatches {
    /// One colour: nothing to tell.
    #[default]
    One,
    /// A colour for each value (the first `LEGEND_CATEGORIES` of them), `total` the number of distinct values.
    Categories { items: Vec<(Color32, String)>, total: usize },
    /// A colour for each class of numbers: class `i` from `bounds[i]` to `bounds[i + 1]` (the first from its
    /// least value, the others above their lower bound).
    Classes { bounds: Vec<f64>, colours: Vec<Color32> },
    /// The ramp, from the least value to the greatest.
    Ramp { ramp: Ramp, min: f64, max: f64 },
}

/// The distinct values a legend names at most.
const LEGEND_CATEGORIES: usize = 200;

/// What the legend of a layer tells: how its colours and its points' sizes come from their attributes.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Legend {
    /// The attribute that colours the features (none: one colour).
    pub attribute: Option<String>,
    pub swatches: Swatches,
    /// The attribute that sizes the points (none: one size), and a few sizes with their values: (diameter in
    /// pixels, the value as text).
    pub size_by: Option<String>,
    pub sizes: Vec<(f32, String)>,
}

impl Legend {
    /// Whether there is anything to tell besides what the features are.
    pub fn is_plain(&self) -> bool {
        matches!(self.swatches, Swatches::One) && self.sizes.is_empty()
    }

    /// The lines of categories or classes: a colour and its text ("0.2 – 0.5" for a class).
    pub fn rows(&self) -> Vec<(Color32, String)> {
        match &self.swatches {
            Swatches::Categories { items, .. } => items.clone(),
            Swatches::Classes { bounds, colours } => colours
                .iter()
                .enumerate()
                .map(|(i, c)| {
                    let text = |v: f64| crate::map_view::cell_text(v);
                    (*c, format!("{} \u{2013} {}", text(bounds[i]), text(bounds[i + 1])))
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    /// How many distinct values the categories have beyond those listed.
    pub fn more(&self) -> usize {
        match &self.swatches {
            Swatches::Categories { items, total } => total.saturating_sub(items.len()),
            _ => 0,
        }
    }
}

/// How every feature of a layer is drawn in a style: its colour and, when the points' size is by an attribute,
/// its size; and what the legend tells. The one place the Viewer's tiles, the figure, the picture of the view
/// and QGIS take their colours and sizes from.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Look {
    pub colours: Vec<Color32>,
    /// A point's diameter, pixels, by feature; empty when the size is not by an attribute.
    pub sizes: Vec<f32>,
    pub legend: Legend,
}

impl Look {
    /// The radius of a feature's point, pixels.
    pub fn radius(&self, k: usize, style: &Style) -> f32 {
        self.sizes.get(k).map_or(2.0 + style.width, |d| d / 2.0)
    }
}

/// Every feature's colour in a style: one, or by the categories, the classes or the range of an attribute.
pub fn colours(v: &Vector, style: &Style) -> Vec<Color32> {
    look(v, style).colours
}

/// The colour of class `i` of `n` along a ramp: the middle of its share of it.
pub fn class_of_ramp(ramp: Ramp, i: usize, n: usize) -> Color32 {
    ramp.colour(if n <= 1 { 0.5 } else { i as f32 / (n - 1) as f32 })
}

/// A point's diameter for a value (pixels): between `min` and `max` by the square root of the value's place
/// between `lo` and `hi`, so the area goes as the value; `min` without a value.
pub fn size_of(v: f64, (lo, hi): (f64, f64), (min, max): (f32, f32)) -> f32 {
    if v.is_nan() || hi <= lo {
        return min;
    }
    min + (max - min) * (((v - lo) / (hi - lo)).clamp(0.0, 1.0) as f32).sqrt()
}

/// The upper bounds of `n` classes of equal width between `lo` and `hi`, but the last's.
pub fn equal_breaks(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    (1..n.max(1)).map(|k| lo + (hi - lo) * k as f64 / n as f64).collect()
}

/// The upper bounds of `n` classes of as many values each, but the last's, for the values sorted (a bound the
/// values take more than once is told once: fewer classes).
pub fn quantile_breaks(sorted: &[f64], n: usize) -> Vec<f64> {
    let m = sorted.len();
    let mut out: Vec<f64> = Vec::new();
    for k in 1..n.max(1) {
        // (the nearest rank: the value the first k / n of them are at most)
        let at = (k * m).div_ceil(n).clamp(1, m.max(1)) - 1;
        if let Some(&v) = sorted.get(at)
            && out.last() != Some(&v)
        {
            out.push(v);
        }
    }
    if let (Some(&last), Some(&top)) = (out.last(), sorted.last())
        && last >= top
    {
        out.pop();
    }
    out
}

/// The upper bounds of `n` classes by Jenks' natural breaks (Fisher's optimal split: the least variance within the
/// classes), but the last's, for values sorted; a few thousand at most are worked on (`JENKS_SAMPLE`): more are
/// sampled, evenly.
pub fn natural_breaks(sorted: &[f64], n: usize) -> Vec<f64> {
    let sample: Vec<f64>;
    let x: &[f64] = if sorted.len() > JENKS_SAMPLE {
        sample = (0..JENKS_SAMPLE).map(|i| sorted[i * (sorted.len() - 1) / (JENKS_SAMPLE - 1)]).collect();
        &sample
    } else {
        sorted
    };
    let m = x.len();
    let k = n.min(m);
    if k < 2 {
        return Vec::new();
    }
    // (the sums of the values and of their squares from the first, as differences of the first: steadier)
    let base = x[0];
    let (mut s1, mut s2) = (vec![0.0; m + 1], vec![0.0; m + 1]);
    for (i, v) in x.iter().enumerate() {
        let d = v - base;
        s1[i + 1] = s1[i] + d;
        s2[i + 1] = s2[i] + d * d;
    }
    // the squares' sum around the mean of x[i..j]
    let cost = |i: usize, j: usize| {
        let (c, a) = ((j - i) as f64, s1[j] - s1[i]);
        (s2[j] - s2[i] - a * a / c).max(0.0)
    };
    // best[j]: the least cost of x[..j] in `c` classes; back[c][j] where the last of them begins
    let mut best: Vec<f64> = (0..=m).map(|j| if j == 0 { f64::INFINITY } else { cost(0, j) }).collect();
    let mut back = vec![vec![0usize; m + 1]; k + 1];
    for (c, row) in back.iter_mut().enumerate().take(k + 1).skip(2) {
        let mut next = vec![f64::INFINITY; m + 1];
        for j in c..=m {
            let (mut least, mut at) = (f64::INFINITY, c - 1);
            for (i, b) in best.iter().enumerate().take(j).skip(c - 1) {
                let t = b + cost(i, j);
                if t < least {
                    (least, at) = (t, i);
                }
            }
            next[j] = least;
            row[j] = at;
        }
        best = next;
    }
    let mut out = Vec::with_capacity(k - 1);
    let mut j = m;
    for c in (2..=k).rev() {
        let i = back[c][j];
        out.push(x[i - 1]);
        j = i;
    }
    out.reverse();
    out.dedup();
    out
}

/// A style's colours and sizes of a layer's features, and what its legend tells.
pub fn look(v: &Vector, style: &Style) -> Look {
    let n = v.len();
    let column = style.by.as_ref().and_then(|b| v.column(b));
    let (colours, swatches) = match column {
        None => (vec![style.colour; n], Swatches::One),
        Some(Column::Numbers(x)) => numbers(x, style),
        Some(Column::Texts(t)) => {
            let mut distinct: Vec<&String> = t.iter().collect();
            distinct.sort();
            distinct.dedup();
            let colours =
                t.iter().map(|s| crate::palette::class_colour(distinct.binary_search(&s).unwrap_or(0))).collect();
            let items = distinct
                .iter()
                .take(LEGEND_CATEGORIES)
                .enumerate()
                .map(|(i, d)| (crate::palette::class_colour(i), (*d).clone()))
                .collect();
            (colours, Swatches::Categories { items, total: distinct.len() })
        }
    };
    let (sizes, size_legend) = match style.size_by.as_ref().and_then(|b| v.column(b)) {
        Some(Column::Numbers(x)) => {
            let finite = x.iter().copied().filter(|v| v.is_finite());
            let lo = finite.clone().fold(f64::INFINITY, f64::min);
            let hi = finite.fold(f64::NEG_INFINITY, f64::max);
            let range = (style.size_min, style.size_max.max(style.size_min));
            let sizes = x.iter().map(|&v| size_of(v, (lo, hi), range)).collect();
            let shown: Vec<f64> = if hi > lo {
                vec![lo, lo + (hi - lo) / 2.0, hi]
            } else if lo.is_finite() {
                vec![lo]
            } else {
                Vec::new()
            };
            (sizes, shown.into_iter().map(|a| (size_of(a, (lo, hi), range), crate::map_view::cell_text(a))).collect())
        }
        _ => (Vec::new(), Vec::new()),
    };
    let legend = Legend {
        attribute: column.and(style.by.clone()),
        swatches,
        size_by: style.size_by.clone().filter(|_| !sizes.is_empty() || n == 0),
        sizes: size_legend,
    };
    Look { colours, sizes, legend }
}

/// The colours of a number attribute: a category each (a few whole numbers, or when asked), the classes of the
/// breaks, or the ramp along the values.
fn numbers(x: &[f64], style: &Style) -> (Vec<Color32>, Swatches) {
    let mut sorted: Vec<f64> = x.iter().copied().filter(|v| !v.is_nan()).collect();
    sorted.sort_by(f64::total_cmp);
    let mut distinct = sorted.clone();
    distinct.dedup();
    let whole = distinct.len() <= 24 && distinct.iter().all(|v| v.fract() == 0.0);
    if style.categories || (style.breaks == Breaks::Continuous && whole) {
        let colours = x
            .iter()
            .map(|v| match distinct.binary_search_by(|d| d.total_cmp(v)) {
                Ok(i) => crate::palette::class_colour(i),
                Err(_) => Color32::GRAY,
            })
            .collect();
        let items = distinct
            .iter()
            .take(LEGEND_CATEGORIES)
            .enumerate()
            .map(|(i, d)| (crate::palette::class_colour(i), crate::map_view::cell_text(*d)))
            .collect();
        return (colours, Swatches::Categories { items, total: distinct.len() });
    }
    let (lo, hi) = (distinct.first().copied().unwrap_or(0.0), distinct.last().copied().unwrap_or(1.0));
    if style.breaks == Breaks::Continuous {
        let span = if hi > lo { hi - lo } else { 1.0 };
        let colours = x
            .iter()
            .map(|v| if v.is_nan() { Color32::GRAY } else { style.ramp.colour(((v - lo) / span) as f32) })
            .collect();
        return (colours, Swatches::Ramp { ramp: style.ramp, min: lo, max: hi });
    }
    let k = style.classes.clamp(2, MAX_CLASSES);
    let inner = match style.breaks {
        Breaks::Equal => equal_breaks(lo, hi, k),
        Breaks::Quantiles => quantile_breaks(&sorted, k),
        _ => natural_breaks(&distinct, k),
    };
    let mut inner: Vec<f64> = inner.into_iter().filter(|b| *b >= lo && *b < hi).collect();
    inner.dedup();
    let classes = inner.len() + 1;
    let palette: Vec<Color32> = (0..classes).map(|i| class_of_ramp(style.ramp, i, classes)).collect();
    let colours =
        x.iter().map(|v| if v.is_nan() { Color32::GRAY } else { palette[inner.partition_point(|b| v > b)] }).collect();
    let bounds = std::iter::once(lo).chain(inner).chain(std::iter::once(hi)).collect();
    (colours, Swatches::Classes { bounds, colours: palette })
}

/// A layer over a grid: its features' and the grid's keys.
type LayerKey = (usize, u64);

/// A tile of a layer: (layer, style, level, column, row), at level `l` a pixel is 2^l cells.
type Key = (LayerKey, u64, i32, i64, i64);

/// The cells a tile spans at a level.
fn span(level: i32) -> f64 {
    TILE as f64 * 2f64.powi(level)
}

/// A tile's picture: the features over it, drawn; none when no feature is.
pub fn rasterize(layer: &Layer, look: &Look, style: &Style, level: i32, tx: i64, ty: i64) -> Option<egui::ColorImage> {
    let s = span(level);
    let px = 2f64.powi(-level); // pixels per cell
    let (ox, oy) = (tx as f64 * s, ty as f64 * s);
    // the features near, a few pixels around for the strokes and dots
    let big = if look.sizes.is_empty() { 0.0 } else { f64::from(style.size_max) / 2.0 - 2.0 };
    let margin = (4.0 + 2.0 * f64::from(style.width) + big.max(0.0)) / px;
    let env = AABB::from_corners([ox - margin, oy - margin], [ox + s + margin, oy + s + margin]);
    let mut found: Vec<u32> = layer.tree.locate_in_envelope_intersecting(&env).map(|g| g.data).collect();
    if found.is_empty() {
        return None;
    }
    // in their order, the later over the earlier
    found.sort_unstable();
    let mut pixmap = tiny_skia::Pixmap::new(TILE as u32, TILE as u32)?;
    let at = |p: &[f64; 2]| (((p[0] - ox) * px) as f32, ((p[1] - oy) * px) as f32);
    let paint = |c: Color32, alpha: f32| {
        let mut p = tiny_skia::Paint::default();
        p.set_color_rgba8(c.r(), c.g(), c.b(), (f32::from(c.a()) * alpha).round() as u8);
        p.anti_alias = true;
        p
    };
    let stroke = tiny_skia::Stroke { width: style.width, ..tiny_skia::Stroke::default() };
    let id = tiny_skia::Transform::identity();
    for k in found {
        let f = &layer.features[k as usize];
        let c = look.colours.get(k as usize).copied().unwrap_or(style.colour);
        let w = (f.bbox[2] - f.bbox[0]) * px;
        let h = (f.bbox[3] - f.bbox[1]) * px;
        // smaller than a pixel: a dot
        if f.kind != Kind::Point && w < 1.0 && h < 1.0 {
            let (x, y) = at(&[(f.bbox[0] + f.bbox[2]) / 2.0, (f.bbox[1] + f.bbox[3]) / 2.0]);
            if let Some(r) = tiny_skia::Rect::from_xywh(x - 0.5, y - 0.5, 1.0, 1.0) {
                pixmap.fill_rect(r, &paint(c, 1.0), id, None);
            }
            continue;
        }
        let mut pb = tiny_skia::PathBuilder::new();
        for part in f.first..f.last {
            let (a, b) = (layer.parts[part as usize] as usize, layer.parts[part as usize + 1] as usize);
            let pts = &layer.xy[a..b];
            if pts.is_empty() {
                continue;
            }
            if f.kind == Kind::Point {
                let (x, y) = at(&pts[0]);
                push_symbol(&mut pb, style.symbol, x, y, look.radius(k as usize, style));
                continue;
            }
            // the vertices a pixel apart at least; a run off the tile or smaller than a pixel the line from its
            // first vertex to its last, within its box: off the tile still, the fill over the tile the same
            let (x, y) = at(&pts[0]);
            pb.move_to(x, y);
            let mut last = (x, y);
            let runs = &layer.runs[layer.run_of[part as usize] as usize..layer.run_of[part as usize + 1] as usize];
            for (k, r) in runs.iter().enumerate() {
                let (i, j) = (a + k * RUN, (a + (k + 1) * RUN).min(b - 1));
                let off =
                    r[2] < env.lower()[0] || r[0] > env.upper()[0] || r[3] < env.lower()[1] || r[1] > env.upper()[1];
                let tiny = (r[2] - r[0]) * px < 0.5 && (r[3] - r[1]) * px < 0.5;
                let from = if off || tiny { j } else { i + 1 };
                for (n, p) in layer.xy[from..=j].iter().enumerate() {
                    let (x, y) = at(p);
                    if (x - last.0).abs() >= 0.5 || (y - last.1).abs() >= 0.5 || from + n == b - 1 {
                        pb.line_to(x, y);
                        last = (x, y);
                    }
                }
            }
            if f.kind == Kind::Polygon {
                pb.close();
            }
        }
        let Some(path) = pb.finish() else { continue };
        match f.kind {
            Kind::Point => {
                pixmap.fill_path(&path, &paint(c, 1.0), tiny_skia::FillRule::Winding, id, None);
                let ring = tiny_skia::Stroke { width: 1.0, ..tiny_skia::Stroke::default() };
                pixmap.stroke_path(&path, &paint(Color32::from_black_alpha(160), 1.0), &ring, id, None);
            }
            Kind::Line => pixmap.stroke_path(&path, &paint(c, 1.0), &stroke, id, None),
            Kind::Polygon if style.width <= 0.0 => {
                pixmap.fill_path(&path, &paint(c, style.fill), tiny_skia::FillRule::EvenOdd, id, None);
            }
            Kind::Polygon => {
                if style.fill > 0.0 {
                    pixmap.fill_path(&path, &paint(c, style.fill), tiny_skia::FillRule::EvenOdd, id, None);
                }
                pixmap.stroke_path(&path, &paint(c, 1.0), &stroke, id, None);
            }
        }
    }
    Some(egui::ColorImage::from_rgba_premultiplied([TILE, TILE], pixmap.data()))
}

/// A symbol of radius `r` pixels at (`x`, `y`) added to a path.
fn push_symbol(pb: &mut tiny_skia::PathBuilder, symbol: Symbol, x: f32, y: f32, r: f32) {
    if symbol == Symbol::Circle {
        pb.push_circle(x, y, r);
        return;
    }
    for (i, (dx, dy)) in symbol.outline(r).into_iter().enumerate() {
        if i == 0 {
            pb.move_to(x + dx, y + dy);
        } else {
            pb.line_to(x + dx, y + dy);
        }
    }
    pb.close();
}

/// The feature under a point (cells) within `tolerance` cells: the last drawn there.
pub fn pick(layer: &Layer, p: [f64; 2], tolerance: f64) -> Option<usize> {
    let env = AABB::from_corners([p[0] - tolerance, p[1] - tolerance], [p[0] + tolerance, p[1] + tolerance]);
    let mut found: Vec<u32> = layer.tree.locate_in_envelope_intersecting(&env).map(|g| g.data).collect();
    found.sort_unstable();
    let near_segment = |a: &[f64; 2], b: &[f64; 2]| {
        let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
        let l2 = dx * dx + dy * dy;
        let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
        let (x, y) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
        x * x + y * y <= tolerance * tolerance
    };
    found.into_iter().rev().map(|k| k as usize).find(|&k| {
        let f = &layer.features[k];
        let parts = (f.first..f.last).map(|part| {
            let (a, b) = (layer.parts[part as usize] as usize, layer.parts[part as usize + 1] as usize);
            &layer.xy[a..b]
        });
        match f.kind {
            Kind::Point => parts.flatten().any(|q| (q[0] - p[0]).hypot(q[1] - p[1]) <= tolerance),
            Kind::Line => parts.into_iter().any(|l| l.windows(2).any(|w| near_segment(&w[0], &w[1]))),
            Kind::Polygon => {
                let mut inside = false;
                for ring in parts {
                    let n = ring.len();
                    for i in 0..n {
                        let (a, b) = (ring[i], ring[(i + 1) % n]);
                        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
                        {
                            inside = !inside;
                        }
                        if near_segment(&a, &b) {
                            return true;
                        }
                    }
                }
                inside
            }
        }
    })
}

struct Job {
    key: Key,
    layer: Arc<Layer>,
    look: Arc<Look>,
    style: Style,
}

struct Made {
    key: Key,
    image: Option<egui::ColorImage>,
}

/// The threads that rasterize tiles: a queue replaced at every frame (the tiles in view, the nearest the centre
/// first), the tiles being made.
struct Workers {
    queue: Arc<(Mutex<VecDeque<Job>>, Condvar)>,
    started: Arc<Mutex<HashSet<Key>>>,
    done: Receiver<Made>,
}

impl Workers {
    fn new(ctx: egui::Context) -> Workers {
        let queue: Arc<(Mutex<VecDeque<Job>>, Condvar)> = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
        let started: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::new()));
        let (tx, done): (Sender<Made>, _) = channel();
        let threads = terraflux_ops::threads::most().min(6);
        for k in 0..threads {
            let (queue, started, tx, ctx) = (queue.clone(), started.clone(), tx.clone(), ctx.clone());
            let _ = std::thread::Builder::new().name(format!("terraflux-vector-tiles-{k}")).spawn(move || {
                loop {
                    let job = {
                        let (lock, cv) = &*queue;
                        let mut q = lock.lock().unwrap();
                        loop {
                            if let Some(j) = q.pop_front() {
                                break j;
                            }
                            q = cv.wait(q).unwrap();
                        }
                    };
                    started.lock().unwrap().insert(job.key);
                    let (_, _, level, tx_, ty) = job.key;
                    let image = rasterize(&job.layer, &job.look, &job.style, level, tx_, ty);
                    started.lock().unwrap().remove(&job.key);
                    if tx.send(Made { key: job.key, image }).is_err() {
                        return;
                    }
                    ctx.request_repaint();
                }
            });
        }
        Workers { queue, started, done }
    }
}

/// A layer being prepared, ready, or failed.
enum Prep {
    Pending(Receiver<Result<Layer, String>>),
    Ready(Arc<Layer>),
    Failed(String),
}

/// The vector layers of a view: prepared once per grid, their tiles made by threads and kept (the least used let
/// go), each style's colours.
#[derive(Default)]
pub struct VectorTiles {
    layers: HashMap<LayerKey, Prep>,
    colours: HashMap<(LayerKey, u64), Arc<Look>>,
    /// a tile's texture, or none (no feature over it); when last shown
    tiles: HashMap<Key, (Option<TextureHandle>, u64)>,
    workers: Option<Workers>,
    asked: HashSet<Key>,
    clock: u64,
    /// The labels placed by the last frame.
    pub labels_placed: usize,
}

/// A grid's key: its geotransform and coordinate system.
fn grid_key(g: &Grid) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    g.transform.iter().for_each(|v| v.to_bits().hash(&mut h));
    crs_of(g).hash(&mut h);
    h.finish()
}

/// A layer to draw: the features and their style.
#[derive(Clone)]
pub struct Overlay {
    pub vector: Arc<Vector>,
    pub style: Style,
    /// Its name, the feature under the pointer told by it (none: the only one).
    pub name: String,
}

/// How the view maps cells to the screen: the cell at the centre, screen points per cell, the rectangle.
#[derive(Clone, Copy)]
pub struct Camera {
    pub center: Vec2,
    pub zoom: f32,
    pub rect: Rect,
    pub pixels_per_point: f32,
}

impl VectorTiles {
    /// Whether layers are being prepared or tiles made.
    pub fn busy(&self) -> bool {
        !self.asked.is_empty() || self.layers.values().any(|p| matches!(p, Prep::Pending(_)))
    }

    /// A layer over a grid, prepared (none while it is, or when it failed: why).
    pub fn layer(&mut self, ctx: &egui::Context, v: &Arc<Vector>, grid: &Grid) -> Result<Option<Arc<Layer>>, String> {
        let key = (Arc::as_ptr(v) as usize, grid_key(grid));
        let prep = self.layers.entry(key).or_insert_with(|| {
            let (tx, rx) = channel();
            let (v, grid, ctx) = (v.clone(), grid.clone(), ctx.clone());
            let _ = std::thread::Builder::new().name("terraflux-vector-index".into()).spawn(move || {
                let _ = tx.send(prepare(&v, &grid));
                ctx.request_repaint();
            });
            Prep::Pending(rx)
        });
        if let Prep::Pending(rx) = prep {
            match rx.try_recv() {
                Ok(Ok(l)) => *prep = Prep::Ready(Arc::new(l)),
                Ok(Err(e)) => *prep = Prep::Failed(e),
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(_) => *prep = Prep::Failed("the layer could not be prepared".into()),
            }
        }
        match prep {
            Prep::Ready(l) => Ok(Some(l.clone())),
            Prep::Failed(e) => Err(e.clone()),
            Prep::Pending(_) => Ok(None),
        }
    }

    /// Draw the layers over the view, asking the threads for the tiles missing.
    pub fn draw(
        &mut self,
        ctx: &egui::Context,
        painter: &egui::Painter,
        cam: Camera,
        grid: &Grid,
        overlays: &[Overlay],
    ) -> Vec<String> {
        let workers = self.workers.get_or_insert_with(|| Workers::new(ctx.clone()));
        for m in workers.done.try_iter().collect::<Vec<_>>() {
            self.asked.remove(&m.key);
            self.clock += 1;
            let tex = m.image.map(|img| ctx.load_texture(format!("vtile-{}", self.clock), img, TextureOptions::LINEAR));
            self.tiles.insert(m.key, (tex, self.clock));
        }
        let mut errors = Vec::new();
        let ppp = cam.pixels_per_point;
        let cells_per_pixel = 1.0 / (f64::from(cam.zoom) * f64::from(ppp));
        let (a, b) = (
            cam.center + (cam.rect.min - cam.rect.center()) / cam.zoom,
            cam.center + (cam.rect.max - cam.rect.center()) / cam.zoom,
        );
        self.clock += 1;
        let clock = self.clock;
        let full = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        let mut wanted: Vec<(f32, Job)> = Vec::new();
        for o in overlays {
            let layer = match self.layer(ctx, &o.vector, grid) {
                Ok(Some(l)) => l,
                Ok(None) => continue,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };
            let Some(bounds) = layer.bounds else { continue };
            let lkey = (Arc::as_ptr(&o.vector) as usize, grid_key(grid));
            let skey = o.style.key();
            // (a style dragged, a size or a colour, makes one a frame: the old ones let go, those of the layers now
            // shown made again once)
            if self.colours.len() > 64 && !self.colours.contains_key(&(lkey, skey)) {
                self.colours.clear();
            }
            let look = self.colours.entry((lkey, skey)).or_insert_with(|| Arc::new(look(&o.vector, &o.style))).clone();
            // the level: a tile's pixel about a pixel of the screen; the coarsest that holds the layer in a tile
            let mut top = FINEST;
            while span(top) < (bounds[2] - bounds[0]).max(bounds[3] - bounds[1]).max(1e-9) && top < 40 {
                top += 1;
            }
            let level = (cells_per_pixel.log2().floor() as i32).clamp(FINEST, top.max(FINEST));
            let screen = |l: i32, tx: i64, ty: i64| -> Rect {
                let s = span(l) as f32;
                let min = cam.rect.center() + (Vec2::new(tx as f32 * s, ty as f32 * s) - cam.center) * cam.zoom;
                Rect::from_min_size(min, Vec2::splat(s * cam.zoom))
            };
            let s = span(level);
            // the tiles in view over the layer's box
            let x0 = (f64::from(a.x).max(bounds[0] - 1.0) / s).floor() as i64;
            let y0 = (f64::from(a.y).max(bounds[1] - 1.0) / s).floor() as i64;
            let x1 = (f64::from(b.x).min(bounds[2] + 1.0) / s).floor() as i64;
            let y1 = (f64::from(b.y).min(bounds[3] + 1.0) / s).floor() as i64;
            if x1 < x0 || y1 < y0 || (x1 - x0 + 1) * (y1 - y0 + 1) > 1024 {
                continue;
            }
            // the top tiles first: something at once
            let ts = span(top);
            for ty in (bounds[1] / ts).floor() as i64..=(bounds[3] / ts).floor() as i64 {
                for tx in (bounds[0] / ts).floor() as i64..=(bounds[2] / ts).floor() as i64 {
                    let k = (lkey, skey, top, tx, ty);
                    if !self.tiles.contains_key(&k) {
                        wanted.push((
                            -1.0,
                            Job { key: k, layer: layer.clone(), look: look.clone(), style: o.style.clone() },
                        ));
                    }
                }
            }
            for ty in y0..=y1 {
                for tx in x0..=x1 {
                    let k = (lkey, skey, level, tx, ty);
                    let r = screen(level, tx, ty);
                    if let Some((tex, used)) = self.tiles.get_mut(&k) {
                        *used = clock;
                        if let Some(t) = tex {
                            painter.image(t.id(), r, full, Color32::WHITE);
                        }
                        continue;
                    }
                    wanted.push((
                        (r.center() - cam.rect.center()).length(),
                        Job { key: k, layer: layer.clone(), look: look.clone(), style: o.style.clone() },
                    ));
                    // meanwhile the finest coarser tile kept, stretched
                    for up in level + 1..=top.max(level + 1) {
                        let shift = (up - level) as u32;
                        let (ax, ay) = (tx >> shift, ty >> shift);
                        if let Some((tex, used)) = self.tiles.get_mut(&(lkey, skey, up, ax, ay)) {
                            *used = clock;
                            if let Some(t) = tex {
                                let f = 1.0 / (1u32 << shift) as f32;
                                let (u, v) = ((tx - (ax << shift)) as f32 * f, (ty - (ay << shift)) as f32 * f);
                                painter.image(
                                    t.id(),
                                    r,
                                    Rect::from_min_size(Pos2::new(u, v), Vec2::splat(f)),
                                    Color32::WHITE,
                                );
                            }
                            break;
                        }
                    }
                }
            }
        }
        // the labels over the tiles, the topmost layer's placed first
        let mut ready: Vec<(&Overlay, Arc<Layer>)> = Vec::new();
        for o in overlays.iter().filter(|o| o.style.labels.is_some()) {
            if let Ok(Some(l)) = self.layer(ctx, &o.vector, grid) {
                ready.push((o, l));
            }
        }
        let layers: Vec<(&Overlay, &Layer)> = ready.iter().map(|(o, l)| (*o, &**l)).collect();
        self.labels_placed = crate::labels::draw(painter, cam, &layers, &mut crate::labels::Placed::default());
        wanted.sort_by(|a, b| a.0.total_cmp(&b.0));
        let workers = self.workers.as_ref().unwrap();
        let started = workers.started.lock().unwrap().clone();
        let (lock, cv) = &*workers.queue;
        let mut q = lock.lock().unwrap();
        q.clear();
        self.asked.retain(|k| started.contains(k));
        for (_, j) in wanted {
            if !started.contains(&j.key) && self.asked.insert(j.key) {
                q.push_back(j);
            } else if started.contains(&j.key) {
                self.asked.insert(j.key);
            }
        }
        cv.notify_all();
        drop(q);
        if self.tiles.len() > KEPT {
            let mut by_age: Vec<(u64, Key)> = self.tiles.iter().map(|(k, (_, u))| (*u, *k)).collect();
            by_age.sort_unstable_by_key(|(u, _)| *u);
            for (_, k) in by_age.into_iter().take(self.tiles.len() - KEPT) {
                self.tiles.remove(&k);
            }
        }
        errors
    }

    /// The feature under a cell position, of the topmost layer that has one there: (layer, feature).
    pub fn pick(
        &mut self,
        ctx: &egui::Context,
        overlays: &[Overlay],
        grid: &Grid,
        cell: [f64; 2],
        tolerance: f64,
    ) -> Option<(usize, usize)> {
        self.pick_all(ctx, overlays, grid, cell, tolerance).into_iter().next()
    }

    /// The feature under a cell position of every layer that has one there, the topmost first: (layer, feature).
    pub fn pick_all(
        &mut self,
        ctx: &egui::Context,
        overlays: &[Overlay],
        grid: &Grid,
        cell: [f64; 2],
        tolerance: f64,
    ) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        for (i, o) in overlays.iter().enumerate().rev() {
            if let Ok(Some(l)) = self.layer(ctx, &o.vector, grid)
                && let Some(k) = pick(&l, cell, tolerance)
            {
                out.push((i, k));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_style_keeps_its_labels_in_a_plot() {
        let s = Style { by: Some("use".into()), labels: Some("name".into()), label_size: 14.0, ..Style::default() };
        let json = s.to_json();
        assert_eq!(json["labels"], "name");
        assert_eq!(Style::from_json(&json), s);
        // no labels: none said, none read
        let plain = Style::default();
        assert!(plain.to_json().get("labels").is_none());
        assert_eq!(Style::from_json(&plain.to_json()), plain);
    }

    #[test]
    fn a_style_of_an_older_plot_has_no_labels() {
        let old = serde_json::json!({ "colour": "#112233ff", "fill": 0.5, "width": 2.0, "by": "use" });
        let s = Style::from_json(&old);
        assert_eq!((s.labels, s.label_size), (None, Style::default().label_size));
        assert_eq!((s.fill, s.width, s.by.as_deref()), (0.5, 2.0, Some("use")));
    }

    /// Points with a number attribute `n` and a text one `t`.
    fn points(n: &[f64], t: &[&str]) -> Vector {
        Vector {
            geometries: (0..n.len()).map(|i| Geometry::Point([i as f64, 0.0])).collect(),
            attributes: vec![
                ("n".into(), Column::Numbers(n.to_vec())),
                ("t".into(), Column::Texts(t.iter().map(|s| s.to_string()).collect())),
            ],
            crs: None,
        }
    }

    const TEN: [f64; 9] = [1.0, 2.0, 3.0, 10.0, 11.0, 12.0, 20.0, 21.0, 22.0];

    #[test]
    fn equal_intervals_and_quantiles() {
        assert_eq!(equal_breaks(0.0, 10.0, 5), vec![2.0, 4.0, 6.0, 8.0]);
        assert_eq!(equal_breaks(0.0, 10.0, 1), Vec::<f64>::new());
        // a quarter of the values in each: the nearest rank
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(quantile_breaks(&v, 4), vec![25.0, 50.0, 75.0]);
        // skewed values: the quantiles follow them, the equal intervals do not
        let skew = [1.0, 1.0, 1.0, 2.0, 2.0, 3.0, 4.0, 5.0, 100.0, 1000.0];
        assert_eq!(quantile_breaks(&skew, 5), vec![1.0, 2.0, 3.0, 5.0]);
        assert!((equal_breaks(1.0, 1000.0, 5)[0] - 200.8).abs() < 1e-9);
        // ties: a bound once, so fewer classes
        assert_eq!(quantile_breaks(&[1.0, 1.0, 1.0, 1.0, 1.0, 2.0], 3), vec![1.0]);
    }

    #[test]
    fn natural_breaks_are_where_the_values_gather() {
        assert_eq!(natural_breaks(&TEN, 3), vec![3.0, 12.0]);
        assert_eq!(natural_breaks(&TEN, 2).len(), 1);
        // as many classes as values at most
        assert_eq!(natural_breaks(&[1.0, 5.0], 5), vec![1.0]);
        assert!(natural_breaks(&[7.0], 3).is_empty());
        // a layer of thousands: sampled, the gap between its two clusters still found
        let mut big: Vec<f64> = (0..6000).map(f64::from).collect();
        big.extend((0..6000).map(|i| 1e6 + f64::from(i)));
        let b = natural_breaks(&big, 2);
        assert_eq!(b.len(), 1);
        assert!((5000.0..=5999.0).contains(&b[0]), "{b:?}");
    }

    #[test]
    fn numbers_in_classes_and_as_categories() {
        let n = [0.1, 0.2, 0.3, 0.45, 0.5, 0.8, 0.9, 1.0];
        let v = points(&n, &["b", "a", "b", "c", "a", "a", "b", "c"]);
        // as today: a ramp along the values
        let ramp = look(&v, &Style { by: Some("n".into()), ..Style::default() });
        assert!(matches!(ramp.legend.swatches, Swatches::Ramp { min, max, .. } if min == 0.1 && max == 1.0));
        assert_eq!(ramp.colours[0], crate::palette::sequential(0.0));
        assert_eq!(ramp.colours[7], crate::palette::sequential(1.0));
        // quantiles in 4 classes: two values each
        let s = Style { by: Some("n".into()), breaks: Breaks::Quantiles, classes: 4, ..Style::default() };
        let l = look(&v, &s);
        let Swatches::Classes { bounds, colours } = &l.legend.swatches else { panic!("{:?}", l.legend.swatches) };
        assert_eq!(bounds, &vec![0.1, 0.2, 0.45, 0.8, 1.0]);
        assert_eq!(colours.len(), 4);
        assert_eq!(colours[0], class_of_ramp(Ramp::Viridis, 0, 4));
        assert_eq!(l.colours[0], l.colours[1]);
        assert_eq!(l.colours[2], l.colours[3]);
        assert_ne!(l.colours[1], l.colours[2]);
        assert_eq!(l.colours[7], colours[3]);
        let rows = l.legend.rows();
        assert_eq!(rows[0].1, "0.1 \u{2013} 0.2");
        assert_eq!(rows[1].1, "0.2 \u{2013} 0.45");
        // another ramp: its colours
        let s2 = Style { ramp: Ramp::Cividis, ..s.clone() };
        assert_eq!(look(&v, &s2).colours[7], Ramp::Cividis.colour(1.0));
        // equal intervals
        let e = Style { breaks: Breaks::Equal, classes: 3, ..s.clone() };
        let Swatches::Classes { bounds, .. } = look(&v, &e).legend.swatches else { panic!() };
        assert_eq!(bounds.len(), 4);
        assert!((bounds[1] - 0.4).abs() < 1e-9 && (bounds[2] - 0.7).abs() < 1e-9);
        // numbers as categories when asked: a colour each value
        let c = Style { by: Some("n".into()), categories: true, ..Style::default() };
        let l = look(&v, &c);
        assert!(matches!(&l.legend.swatches, Swatches::Categories { total: 8, .. }));
        assert_eq!(l.colours[3], crate::palette::class_colour(3));
        assert_eq!(l.legend.rows()[3].1, "0.45");
        // texts always: a colour each, sorted
        let t = look(&v, &Style { by: Some("t".into()), breaks: Breaks::Natural, ..Style::default() });
        assert_eq!(t.legend.rows().iter().map(|r| r.1.as_str()).collect::<Vec<_>>(), ["a", "b", "c"]);
        assert_eq!(t.colours[1], crate::palette::class_colour(0));
        assert_eq!(t.colours[0], crate::palette::class_colour(1));
        // a few whole numbers: categories as before
        let w = points(&[1.0, 2.0, 2.0, 5.0], &["", "", "", ""]);
        let l = look(&w, &Style { by: Some("n".into()), ..Style::default() });
        assert!(matches!(&l.legend.swatches, Swatches::Categories { total: 3, .. }));
        assert_eq!(l.colours[3], crate::palette::class_colour(2));
        // no attribute: one colour, nothing to tell
        let one = look(&v, &Style::default());
        assert!(one.legend.is_plain() && one.colours.iter().all(|c| *c == Style::default().colour));
        // the classes of a colour are the same wherever they are drawn: `colours` is the look's
        assert_eq!(colours_of(&v, &s), look(&v, &s).colours);
    }

    fn colours_of(v: &Vector, s: &Style) -> Vec<Color32> {
        colours(v, s)
    }

    #[test]
    fn many_categories_are_listed_up_to_a_limit() {
        let t: Vec<String> = (0..500).map(|i| format!("name {i:03}")).collect();
        let v = Vector {
            geometries: vec![Geometry::Empty; 500],
            attributes: vec![("t".into(), Column::Texts(t))],
            crs: None,
        };
        let l = look(&v, &Style { by: Some("t".into()), ..Style::default() }).legend;
        assert_eq!(l.rows().len(), 200);
        assert_eq!(l.more(), 300);
    }

    #[test]
    fn the_size_of_points_by_an_attribute() {
        // the area as the value: the middle value a diameter between by the square root
        assert_eq!(size_of(0.0, (0.0, 100.0), (3.0, 18.0)), 3.0);
        assert_eq!(size_of(100.0, (0.0, 100.0), (3.0, 18.0)), 18.0);
        assert!((size_of(25.0, (0.0, 100.0), (3.0, 18.0)) - 10.5).abs() < 1e-5);
        assert_eq!(size_of(f64::NAN, (0.0, 100.0), (3.0, 18.0)), 3.0);
        assert_eq!(size_of(500.0, (0.0, 100.0), (3.0, 18.0)), 18.0);
        assert_eq!(size_of(5.0, (5.0, 5.0), (3.0, 18.0)), 3.0);
        let v = points(&[10.0, 20.0, 30.0], &["", "", ""]);
        let s = Style { size_by: Some("n".into()), size_min: 4.0, size_max: 20.0, ..Style::default() };
        let l = look(&v, &s);
        assert_eq!(l.sizes, vec![4.0, size_of(20.0, (10.0, 30.0), (4.0, 20.0)), 20.0]);
        assert_eq!(l.radius(2, &s), 10.0);
        // the legend: three sizes with their values
        let shown: Vec<&str> = l.legend.sizes.iter().map(|(_, t)| t.as_str()).collect();
        assert_eq!(shown, ["10", "20", "30"]);
        assert_eq!(l.legend.size_by.as_deref(), Some("n"));
        // not by an attribute: the width's radius, no sizes
        let plain = look(&v, &Style::default());
        assert!(plain.sizes.is_empty() && plain.legend.sizes.is_empty());
        assert_eq!(plain.radius(0, &Style::default()), 2.0 + Style::default().width);
        // a text attribute sizes nothing
        let t = look(&v, &Style { size_by: Some("t".into()), ..Style::default() });
        assert!(t.sizes.is_empty());
    }

    #[test]
    fn the_symbols_have_their_shapes() {
        let corners: Vec<usize> = Symbol::ALL.iter().map(|s| s.outline(5.0).len()).collect();
        assert_eq!(corners, [16, 4, 3, 4, 12]);
        for s in Symbol::ALL {
            let o = s.outline(5.0);
            assert!(o.iter().all(|(x, y)| x.abs() <= 5.001 && y.abs() <= 5.001), "{s:?}");
            assert_eq!(Symbol::named(s.key()), Some(s));
        }
        // the triangle points up (the screen's y goes down)
        let tri = Symbol::Triangle.outline(5.0);
        assert!(tri[0].1 < 0.0 && tri[1].1 > 0.0);
        assert_eq!(Symbol::named("star"), None);
    }

    #[test]
    fn a_symbol_is_drawn_by_the_tile() {
        let mut v = points(&[1.0, 9.0], &["", ""]);
        v.geometries = vec![Geometry::Point([0.0, 0.0]), Geometry::Point([12.0, 0.0])];
        let v = std::sync::Arc::new(v);
        let mut g = Grid::new(20, 20);
        g.transform = [-5.0, 1.0, 0.0, 5.0, 0.0, -1.0];
        let layer = prepare(&v, &g).unwrap();
        let drawn = |s: &Style| {
            let l = look(&v, s);
            let img = rasterize(&layer, &l, s, 0, 0, 0).expect("the points in a tile");
            img.pixels.iter().filter(|p| p.a() > 0).count()
        };
        let circle = drawn(&Style { width: 4.0, ..Style::default() });
        let cross = drawn(&Style { width: 4.0, symbol: Symbol::Cross, ..Style::default() });
        assert!(circle > 0 && cross > 0 && cross < circle, "{circle} {cross}");
        // by size: the bigger value covers more
        let sized = drawn(&Style { size_by: Some("n".into()), ..Style::default() });
        let small = drawn(&Style { size_by: Some("n".into()), size_min: 3.0, size_max: 3.0, ..Style::default() });
        assert!(sized > small, "{sized} {small}");
    }

    #[test]
    fn a_style_keeps_its_symbology_in_a_plot() {
        let s = Style {
            by: Some("n".into()),
            ramp: Ramp::Cividis,
            breaks: Breaks::Natural,
            classes: 7,
            categories: false,
            size_by: Some("pop".into()),
            size_min: 4.0,
            size_max: 24.0,
            symbol: Symbol::Diamond,
            ..Style::default()
        };
        let json = s.to_json();
        assert_eq!(
            (json["breaks"].as_str(), json["classes"].as_u64(), json["symbol"].as_str()),
            (Some("natural"), Some(7), Some("diamond"))
        );
        assert_eq!(json["size_by"], "pop");
        assert!(json["ramp"].as_str().is_some_and(|r| r.starts_with("cividis")));
        assert_eq!(Style::from_json(&json), s);
        let c = Style { categories: true, by: Some("n".into()), ..Style::default() };
        assert_eq!(c.to_json()["categories"], true);
        assert_eq!(Style::from_json(&c.to_json()), c);
        // by default: none of it said
        let plain = Style::default().to_json();
        for k in ["ramp", "breaks", "classes", "categories", "size_by", "size_min", "symbol"] {
            assert!(plain.get(k).is_none(), "{k}");
        }
        // an older Plot, and one with nonsense: as by default
        let old = serde_json::json!({ "colour": "#112233ff", "by": "n" });
        let (o, d) = (Style::from_json(&old), Style::default());
        assert_eq!((o.ramp, o.breaks, o.classes, o.symbol, o.size_by), (d.ramp, d.breaks, d.classes, d.symbol, None));
        let odd = serde_json::json!({ "breaks": "wavy", "classes": 99, "symbol": "star", "ramp": "rainbow" });
        let o = Style::from_json(&odd);
        assert_eq!(
            (o.breaks, o.classes, o.symbol, o.ramp),
            (Breaks::Continuous, MAX_CLASSES, Symbol::Circle, Ramp::Viridis)
        );
        // the keys of the tiles tell the symbology apart
        assert_ne!(Style::default().key(), Style { symbol: Symbol::Square, ..Style::default() }.key());
        assert_ne!(Style::default().key(), Style { breaks: Breaks::Equal, ..Style::default() }.key());
        assert_ne!(Style::default().key(), Style { ramp: Ramp::Cividis, ..Style::default() }.key());
        assert_eq!(Breaks::named("Jenks"), Some(Breaks::Natural));
        assert_eq!(Breaks::named("equal intervals"), Some(Breaks::Equal));
    }
}
