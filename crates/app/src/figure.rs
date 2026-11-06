//! A map as a figure for a paper: one map or several side by side (a), (b)..., each with its caption, its
//! coordinates along its edges, a scale bar, a north arrow, a location inset (where in the world the map is), the
//! vector layers drawn over it in the Viewer and the legend, at the width of a journal's column and the resolution
//! it asks for. Laid out once in points (1/72 inch) as shapes, then written as SVG (text as text, to edit in
//! Inkscape or Illustrator; the maps as pictures in it), as PDF (vectors and text, the maps as pictures) or drawn
//! as PNG at the DPI chosen (the same letters, as the font's outlines).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use ab_glyph::{Font, FontRef, OutlineCurve};
use egui::{Color32, RichText};

use crate::map_view::Source;
use crate::palette::Coloring;

/// A map of the figure: what it shows, its colours, the names of its classes, its caption, and the vector layers
/// drawn over it.
#[derive(Clone)]
pub struct Panel {
    pub source: Source,
    pub coloring: Coloring,
    pub names: Vec<(f64, String)>,
    pub caption: String,
    pub vectors: Vec<Vectors>,
    /// The maps drawn with it in the Viewer (under it or over it, at their opacity).
    pub rasters: Vec<crate::map_view::RasterLayer>,
}

/// Features drawn over a map, in its coordinate system: a colour each, the polygons' fill (0 to 1) and the
/// lines' width (points).
#[derive(Clone)]
pub struct Vectors {
    pub vector: Arc<terraflux_raster::vector::Vector>,
    pub colours: Vec<Color32>,
    pub fill: f32,
    pub width: f32,
    /// The attribute whose values label the features (none: no labels), and their size, points.
    pub labels: Option<String>,
    pub label_size: f32,
    /// A point's diameter by feature, points (empty: all of the width's size), the points' shape.
    pub sizes: Vec<f32>,
    pub symbol: crate::vector_tiles::Symbol,
    /// What the colours and sizes are by, for the legend.
    pub legend: crate::vector_tiles::Legend,
}

impl Vectors {
    /// Features in the colours and sizes of a look in the Viewer ([`crate::vector_tiles::look`]), widths and sizes
    /// in the figure's points (0.6 of the Viewer's pixels).
    pub fn of(
        vector: Arc<terraflux_raster::vector::Vector>,
        look: crate::vector_tiles::Look,
        style: &crate::vector_tiles::Style,
    ) -> Vectors {
        Vectors {
            vector,
            colours: look.colours,
            fill: style.fill,
            width: style.width * 0.6,
            labels: style.labels.clone(),
            label_size: style.label_size * 0.6,
            sizes: look.sizes.iter().map(|d| d * 0.6).collect(),
            symbol: style.symbol,
            legend: look.legend,
        }
    }
}

/// How the figure is laid out.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    pub title: String,
    /// The width of the figure, millimetres (a journal's column: about 85; the page: about 180).
    pub width_mm: f32,
    pub dpi: u32,
    pub columns: usize,
    pub legend: bool,
    pub scale_bar: bool,
    pub north: bool,
    pub grid: bool,
    /// A small map of where in the world the map is.
    pub inset: bool,
}

impl Default for Layout {
    fn default() -> Self {
        Layout {
            title: String::new(),
            width_mm: 180.0,
            dpi: 300,
            columns: 2,
            legend: true,
            scale_bar: true,
            north: true,
            grid: true,
            inset: true,
        }
    }
}

/// Where a text is from its point: its start or its middle.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Anchor {
    Start,
    Middle,
}

/// A thing drawn, in points from the figure's top left corner.
#[derive(Clone)]
enum Shape {
    /// A map's picture (premultiplied RGBA), stretched over its rectangle, its cells kept square-edged.
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        rgba: Arc<Vec<u8>>,
        iw: usize,
        ih: usize,
    },
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Option<Color32>,
        stroke: Option<(Color32, f32)>,
    },
    Line {
        points: Vec<(f32, f32)>,
        colour: Color32,
        width: f32,
    },
    Polygon {
        points: Vec<(f32, f32)>,
        fill: Color32,
        stroke: Option<(Color32, f32)>,
    },
    /// Rings (closed: filled by the even-odd rule, a polygon's holes left out) or lines.
    Path {
        rings: Vec<Vec<(f32, f32)>>,
        closed: bool,
        fill: Option<Color32>,
        stroke: Option<(Color32, f32)>,
    },
    /// What follows drawn only inside a rectangle (x, y, w, h), up to `Clip(None)`.
    Clip(Option<(f32, f32, f32, f32)>),
    /// A line of text: its point on the baseline, its size, read upwards when `up`.
    Text {
        x: f32,
        y: f32,
        size: f32,
        text: String,
        anchor: Anchor,
        colour: Color32,
        up: bool,
    },
}

/// A figure laid out: its size in points and what is drawn.
pub struct Drawing {
    pub w: f32,
    pub h: f32,
    shapes: Vec<Shape>,
}

const INK: Color32 = Color32::from_rgb(34, 34, 34);
const GRID: Color32 = Color32::from_rgba_premultiplied(120, 120, 120, 120);
const HALO: Color32 = Color32::from_rgba_premultiplied(230, 230, 230, 230);

/// The font's measures: a text's width at a size (points).
struct Type {
    font: FontRef<'static>,
    upem: f32,
}

impl Type {
    fn new() -> Type {
        let font = crate::animation::font();
        let upem = font.units_per_em().unwrap_or(1000.0);
        Type { font, upem }
    }

    fn width(&self, size: f32, text: &str) -> f32 {
        let mut w = 0.0;
        let mut last = None;
        for c in text.chars() {
            let id = self.font.glyph_id(c);
            if let Some(p) = last {
                w += self.font.kern_unscaled(p, id);
            }
            w += self.font.h_advance_unscaled(id);
            last = Some(id);
        }
        w * size / self.upem
    }
}

/// A nice step for about `n` ticks over `span`: 1, 2 or 5 times a power of ten.
pub(crate) fn nice_step(span: f64, n: f64) -> f64 {
    if span <= 0.0 || !span.is_finite() {
        return 1.0;
    }
    let raw = span / n;
    let p = 10f64.powf(raw.log10().floor());
    let f = raw / p;
    p * if f < 1.5 {
        1.0
    } else if f < 3.5 {
        2.0
    } else if f < 7.5 {
        5.0
    } else {
        10.0
    }
}

/// A coordinate as a label: degrees with their hemisphere, or map units (thousands spaced).
pub(crate) fn coordinate(v: f64, step: f64, geographic: bool, east: bool) -> String {
    let decimals = if step >= 1.0 { 0 } else { (-step.log10()).ceil() as usize };
    if geographic {
        let h = match (east, v < 0.0) {
            (true, false) => "E",
            (true, true) => "W",
            (false, false) => "N",
            (false, true) => "S",
        };
        let t = format!("{:.*}°{h}", decimals, v.abs());
        if v == 0.0 { format!("{:.*}°", decimals, 0.0) } else { t }
    } else {
        let t = format!("{v:.decimals$}");
        // 4500000 as 4 500 000 (a thin space)
        let (int, frac) = t.split_once('.').map_or((t.as_str(), None), |(a, b)| (a, Some(b)));
        let (sign, digits) = int.strip_prefix('-').map_or(("", int), |d| ("-", d));
        let mut grouped = String::new();
        for (i, c) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                grouped.push('\u{2009}');
            }
            grouped.push(c);
        }
        match frac {
            Some(f) => format!("{sign}{grouped}.{f}"),
            None => format!("{sign}{grouped}"),
        }
    }
}

/// The extent of a grid in its coordinates: (west, south, east, north).
fn extent(g: &terraflux_raster::Grid) -> (f64, f64, f64, f64) {
    let t = g.transform;
    let (x0, x1) = (t[0], t[0] + g.width as f64 * t[1]);
    let (y0, y1) = (t[3], t[3] + g.height as f64 * t[5]);
    (x0.min(x1), y0.min(y1), x0.max(x1), y0.max(y1))
}

/// Whether a grid is georeferenced (a coordinate system, or coordinates other than cell numbers).
fn georeferenced(g: &terraflux_raster::Grid) -> bool {
    g.geo.wkt.is_some() || g.geo.epsg().is_some() || g.geo.is_geographic().is_some()
}

/// The legend of a colouring: its height at `x`, `y` (top) when drawn, and its width; under it the legends of the
/// vector layers' colours and sizes by an attribute.
fn legend(t: &Type, out: &mut Vec<Shape>, p: &Panel, x: f32, y: f32) -> (f32, f32) {
    let (mut w, mut h) = raster_legend(t, out, p, x, y);
    for v in p.vectors.iter().filter(|v| !v.legend.is_plain()) {
        let gap = if h > 0.0 { 6.0 } else { 0.0 };
        let (vw, vh) = vector_legend(t, out, v, x, y + h + gap);
        w = w.max(vw);
        h += gap + vh;
    }
    (w, h)
}

/// A ramp upwards from the least to the greatest, labelled with `values` (the greatest, the middle, the least):
/// its width and height.
fn ramp_bar(
    t: &Type,
    out: &mut Vec<Shape>,
    x: f32,
    y: f32,
    ramp: crate::palette::Ramp,
    values: [f64; 3],
) -> (f32, f32) {
    const SIZE: f32 = 7.5;
    let (bw, bh) = (8.0, 72.0);
    let n = 64;
    for k in 0..n {
        let t0 = k as f32 / n as f32;
        out.push(Shape::Rect {
            x,
            y: y + bh * (1.0 - t0 - 1.0 / n as f32),
            w: bw,
            // (each a little over the one under it: no seam between them where they are smoothed)
            h: bh / n as f32 + if k == 0 { 0.0 } else { 0.6 },
            fill: Some(ramp.colour(t0 + 0.5 / n as f32)),
            stroke: None,
        });
    }
    out.push(Shape::Rect { x, y, w: bw, h: bh, fill: None, stroke: Some((Color32::from_gray(110), 0.4)) });
    let mut w: f32 = 0.0;
    for (v, at) in [(values[0], 0.0), (values[1], 0.5), (values[2], 1.0)] {
        let label = crate::animation::short(v);
        out.push(Shape::Line {
            points: vec![(x + bw, y + bh * at), (x + bw + 2.5, y + bh * at)],
            colour: INK,
            width: 0.4,
        });
        out.push(Shape::Text {
            x: x + bw + 4.0,
            y: y + bh * at + 2.6,
            size: SIZE,
            text: label.clone(),
            anchor: Anchor::Start,
            colour: INK,
            up: false,
        });
        w = w.max(bw + 4.0 + t.width(SIZE, &label));
    }
    (w, bh)
}

/// The legend of a vector layer's colours and sizes: the attribute that colours it over a line each category or
/// class (at most 12, then "and k more") or the ramp; the attribute that sizes its points over a few of the sizes.
fn vector_legend(t: &Type, out: &mut Vec<Shape>, v: &Vectors, x: f32, y: f32) -> (f32, f32) {
    use crate::vector_tiles::Swatches;
    const SIZE: f32 = 7.5;
    const MAX: usize = 12;
    let row = 11.0;
    let (mut w, mut h): (f32, f32) = (0.0, 0.0);
    let text = |out: &mut Vec<Shape>, s: String, x: f32, y: f32| {
        out.push(Shape::Text { x, y, size: SIZE, text: s.clone(), anchor: Anchor::Start, colour: INK, up: false });
        t.width(SIZE, &s)
    };
    let l = &v.legend;
    if let (Some(a), false) = (&l.attribute, matches!(l.swatches, Swatches::One)) {
        w = w.max(text(out, a.clone(), x, y + 7.0));
        h += row;
        match &l.swatches {
            Swatches::Ramp { ramp, min, max } => {
                let (rw, rh) = ramp_bar(t, out, x, y + h, *ramp, [*max, (min + max) / 2.0, *min]);
                w = w.max(rw);
                h += rh + 2.0;
            }
            _ => {
                let rows = l.rows();
                for (c, name) in rows.iter().take(MAX) {
                    out.push(Shape::Rect {
                        x,
                        y: y + h,
                        w: 8.0,
                        h: 8.0,
                        fill: Some(*c),
                        stroke: Some((Color32::from_gray(110), 0.4)),
                    });
                    w = w.max(12.0 + text(out, name.clone(), x + 12.0, y + h + 7.0));
                    h += row;
                }
                let more = rows.len().saturating_sub(MAX) + l.more();
                if more > 0 {
                    w = w.max(text(out, format!("and {more} more"), x, y + h + 7.0));
                    h += row;
                }
            }
        }
    }
    if let (Some(a), false) = (&l.size_by, l.sizes.is_empty()) {
        if h > 0.0 {
            h += 4.0;
        }
        w = w.max(text(out, format!("size: {a}"), x, y + h + 7.0));
        h += row;
        // (the diameters in points, 0.6 of the Viewer's pixels)
        let biggest = l.sizes.iter().map(|(d, _)| d * 0.6).fold(0.0, f32::max);
        for (d, label) in &l.sizes {
            let r = d * 0.6 / 2.0;
            let line = (r * 2.0 + 2.0).max(row);
            let (cx, cy) = (x + biggest / 2.0, y + h + line / 2.0);
            let ring = v.symbol.outline(r).into_iter().map(|(dx, dy)| (cx + dx, cy + dy)).collect();
            out.push(Shape::Path {
                rings: vec![ring],
                closed: true,
                fill: Some(Color32::from_gray(150)),
                stroke: Some((Color32::from_gray(60), 0.4)),
            });
            w = w.max(biggest + 4.0 + text(out, label.clone(), x + biggest + 4.0, cy + 2.6));
            h += line;
        }
    }
    (w, h)
}

/// The legend of a map's colouring: its height at `x`, `y` (top) when drawn, and its width.
fn raster_legend(t: &Type, out: &mut Vec<Shape>, p: &Panel, x: f32, y: f32) -> (f32, f32) {
    const SIZE: f32 = 7.5;
    const MAX: usize = 20;
    match &p.coloring {
        Coloring::Classes(classes) => {
            let mut w: f32 = 0.0;
            let row = 11.0;
            let shown = classes.len().min(MAX);
            for (i, (v, c)) in classes.iter().take(MAX).enumerate() {
                let name = p
                    .names
                    .iter()
                    .find(|(k, _)| k == v)
                    .map_or_else(|| terraflux_ops::value::class_label(*v), |(_, n)| n.clone());
                let yy = y + i as f32 * row;
                out.push(Shape::Rect {
                    x,
                    y: yy,
                    w: 8.0,
                    h: 8.0,
                    fill: Some(*c),
                    stroke: Some((Color32::from_gray(110), 0.4)),
                });
                out.push(Shape::Text {
                    x: x + 12.0,
                    y: yy + 7.0,
                    size: SIZE,
                    text: name.clone(),
                    anchor: Anchor::Start,
                    colour: INK,
                    up: false,
                });
                w = w.max(12.0 + t.width(SIZE, &name));
            }
            let mut h = shown as f32 * row;
            if classes.len() > MAX {
                let more = format!("and {} more", classes.len() - MAX);
                out.push(Shape::Text {
                    x,
                    y: y + h + 7.0,
                    size: SIZE,
                    text: more,
                    anchor: Anchor::Start,
                    colour: INK,
                    up: false,
                });
                h += row;
            }
            (w, h)
        }
        // a colour composite: a line for each channel, its band and the range it is drawn over
        Coloring::Shade { .. } => (0.0, 0.0),
        Coloring::Packed => {
            let Source::Rgb(r) = &p.source else { return (0.0, 0.0) };
            let (mut w, row): (f32, f32) = (0.0, 11.0);
            for (i, (c, text)) in r.legend().into_iter().enumerate() {
                let yy = y + i as f32 * row;
                out.push(Shape::Rect {
                    x,
                    y: yy,
                    w: 8.0,
                    h: 8.0,
                    fill: Some(c),
                    stroke: Some((Color32::from_gray(110), 0.4)),
                });
                out.push(Shape::Text {
                    x: x + 12.0,
                    y: yy + 7.0,
                    size: SIZE,
                    text: text.clone(),
                    anchor: Anchor::Start,
                    colour: INK,
                    up: false,
                });
                w = w.max(12.0 + t.width(SIZE, &text));
            }
            (w, 3.0 * row)
        }
        Coloring::Continuous { min, max, ramp, log } => {
            // the ramp upwards, from the minimum to the maximum, labelled at both ends and the middle
            let mid = if *log && *min > 0.0 { (min * max).sqrt() } else { (min + max) / 2.0 };
            let (w, bh) = ramp_bar(t, out, x, y, *ramp, [*max, mid, *min]);
            if *log {
                out.push(Shape::Text {
                    x,
                    y: y + bh + 9.0,
                    size: 6.5,
                    text: "logarithmic".into(),
                    anchor: Anchor::Start,
                    colour: INK,
                    up: false,
                });
                return (w.max(t.width(6.5, "logarithmic")), bh + 10.0);
            }
            (w, bh + 2.0)
        }
    }
}

/// The width of a legend without drawing it.
fn legend_size(t: &Type, p: &Panel) -> (f32, f32) {
    let mut none = Vec::new();
    legend(t, &mut none, p, 0.0, 0.0)
}

/// A map's picture at most `px` pixels across: its cells in their colours (premultiplied RGBA), transparent
/// without a value.
fn picture(p: &Panel, px: usize) -> Result<(Arc<Vec<u8>>, usize, usize), String> {
    let img = crate::animation::composed(&p.source, &p.coloring, &p.rasters, px.max(16))?;
    let [w, h] = img.size;
    let mut rgba = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        rgba.extend_from_slice(&c.to_array());
    }
    Ok((Arc::new(rgba), w, h))
}

/// The map as the Viewer draws it and nothing else, each cell `scale` pixels across (the map read more coarsely
/// when that passes `max_side` pixels): the cells coloured with the layers under and over them, the vectors and
/// their labels. A point of the drawing is a pixel.
pub fn plain(p: &Panel, scale: usize, max_side: usize) -> Result<Drawing, String> {
    plain_over(p, scale, max_side, None)
}

/// What a map is drawn over: a picture made for the size asked (width and height in pixels; premultiplied RGBA,
/// the map's own extent), and how opaque the map is over it (0 to 1).
pub struct Under<'a> {
    pub make: &'a dyn Fn(usize, usize) -> Result<Vec<u8>, String>,
    pub opacity: f32,
}

/// [`plain`] over a picture (a basemap): the cells at `under.opacity`, the vectors and labels as they are.
pub fn plain_over(p: &Panel, scale: usize, max_side: usize, under: Option<&Under>) -> Result<Drawing, String> {
    let k = scale.max(1);
    let (mut rgba, iw, ih) = picture(p, max_side / k)?;
    let (w, h) = ((iw * k) as f32, (ih * k) as f32);
    let mut shapes = Vec::new();
    if let Some(u) = under {
        let (pw, ph) = (iw * k, ih * k);
        let base = (u.make)(pw, ph)?;
        if base.len() != pw * ph * 4 {
            return Err("the picture under the map is not of its size".into());
        }
        shapes.push(Shape::Image { x: 0.0, y: 0.0, w, h, rgba: Arc::new(base), iw: pw, ih: ph });
        let o = u.opacity.clamp(0.0, 1.0);
        if o < 1.0 {
            // (premultiplied: every channel scales with the alpha)
            let faded: Vec<u8> = rgba.iter().map(|&c| (f32::from(c) * o).round() as u8).collect();
            rgba = Arc::new(faded);
        }
    }
    shapes.push(Shape::Image { x: 0.0, y: 0.0, w, h, rgba, iw, ih });
    let g = p.source.grid();
    let (west, south, east, north) = extent(g);
    let north_up = g.transform[2] == 0.0 && g.transform[4] == 0.0 && g.transform[5] < 0.0;
    if north_up && east > west && north > south {
        let at = |x: f64, y: f64| (((x - west) / (east - west)) as f32 * w, ((north - y) / (north - south)) as f32 * h);
        // (a panel keeps the widths and sizes in the figure's points, 0.6 of the Viewer's pixels)
        let to_px = k as f32 / 0.6;
        for v in &p.vectors {
            let v = Vectors {
                width: v.width * to_px,
                label_size: v.label_size * to_px,
                sizes: v.sizes.iter().map(|d| d * to_px).collect(),
                ..v.clone()
            };
            vectors(&mut shapes, &v, &at);
        }
    }
    Ok(Drawing { w, h, shapes })
}

/// Features as shapes: polygons filled (and outlined), lines, points as small discs; `at` from the map's
/// coordinates to the figure's points.
fn vectors(out: &mut Vec<Shape>, v: &Vectors, at: &dyn Fn(f64, f64) -> (f32, f32)) {
    use terraflux_raster::vector::Geometry;
    let ring = |r: &[[f64; 2]]| -> Vec<(f32, f32)> { r.iter().map(|p| at(p[0], p[1])).collect() };
    for (k, g) in v.vector.geometries.iter().enumerate() {
        let c = v.colours.get(k).copied().unwrap_or(INK);
        let stroke = (v.width > 0.0).then_some((c, v.width));
        match g {
            Geometry::Polygon(_) | Geometry::MultiPolygon(_) => {
                let rings: Vec<Vec<(f32, f32)>> = g.polygons().into_iter().flatten().map(|r| ring(r)).collect();
                let fill = (v.fill > 0.0).then(|| c.gamma_multiply(v.fill.clamp(0.0, 1.0)));
                out.push(Shape::Path { rings, closed: true, fill, stroke: stroke.or(Some((c, 0.6))) });
            }
            Geometry::LineString(l) => {
                out.push(Shape::Path {
                    rings: vec![ring(l)],
                    closed: false,
                    fill: None,
                    stroke: Some((c, v.width.max(0.3))),
                });
            }
            Geometry::MultiLineString(m) => {
                let rings = m.iter().map(|l| ring(l)).collect();
                out.push(Shape::Path { rings, closed: false, fill: None, stroke: Some((c, v.width.max(0.3))) });
            }
            Geometry::Point(_) | Geometry::MultiPoint(_) => {
                let r = v.sizes.get(k).map_or(1.2 + v.width / 2.0, |d| d / 2.0);
                let shape = v.symbol.outline(r);
                let mut rings = Vec::new();
                g.points(&mut |p| {
                    let (x, y) = at(p[0], p[1]);
                    rings.push(shape.iter().map(|(dx, dy)| (x + dx, y + dy)).collect());
                });
                out.push(Shape::Path { rings, closed: true, fill: Some(c), stroke: Some((Color32::WHITE, 0.3)) });
            }
            Geometry::Empty => {}
        }
    }
    if let Some(attr) = &v.labels {
        labels(out, v, attr, at);
    }
}

/// The labels of features: their attribute's values, dark on a light halo, where [`crate::labels::anchor`] puts them,
/// one that would cover another left out.
fn labels(out: &mut Vec<Shape>, v: &Vectors, attr: &str, at: &dyn Fn(f64, f64) -> (f32, f32)) {
    use crate::vector_tiles::Kind;
    use terraflux_raster::vector::Geometry;
    let size = v.label_size.max(2.0);
    let mut placed = crate::labels::Placed::default();
    // the features, the larger first (so that when labels collide the big ones win), as the Viewer places them
    type Candidate<'a> = (usize, Kind, Vec<&'a [[f64; 2]]>, crate::labels::Rank);
    let mut order: Vec<Candidate> = Vec::new();
    for (k, g) in v.vector.geometries.iter().enumerate() {
        let (kind, parts): (Kind, Vec<&[[f64; 2]]>) = match g {
            Geometry::Polygon(_) | Geometry::MultiPolygon(_) => {
                (Kind::Polygon, g.polygons().into_iter().flatten().map(|r| &r[..]).collect())
            }
            Geometry::LineString(l) => (Kind::Line, vec![&l[..]]),
            Geometry::MultiLineString(m) => (Kind::Line, m.iter().map(|l| &l[..]).collect()),
            Geometry::Point(p) => (Kind::Point, vec![std::slice::from_ref(p)]),
            Geometry::MultiPoint(ps) => (Kind::Point, ps.iter().map(std::slice::from_ref).collect()),
            Geometry::Empty => continue,
        };
        let mut bx = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
        for p in parts.iter().flat_map(|part| part.iter()) {
            bx = [bx[0].min(p[0]), bx[1].min(p[1]), bx[2].max(p[0]), bx[3].max(p[1])];
        }
        let rank = crate::labels::rank(kind, bx);
        order.push((k, kind, parts, rank));
    }
    order.sort_by(|a, b| crate::labels::by_rank(&a.3, &b.3));
    for (k, kind, parts, _) in order {
        if placed.len() >= crate::labels::MAX {
            break;
        }
        let (Some(c), Some(text)) = (crate::labels::anchor(kind, &parts), crate::labels::text(&v.vector, attr, k))
        else {
            continue;
        };
        let (x, y) = at(c[0], c[1]);
        let w = text.chars().count() as f32 * size * 0.55;
        let point = kind == Kind::Point;
        let x0 = if point { x + 2.5 } else { x - w / 2.0 };
        if !placed.take(egui::Rect::from_min_size(egui::pos2(x0, y - size / 2.0), egui::vec2(w, size))) {
            continue;
        }
        let (tx, anchor) = if point { (x + 2.5, Anchor::Start) } else { (x, Anchor::Middle) };
        let ty = y + size * 0.35;
        let halo = Color32::from_white_alpha(230);
        let d = size * 0.07;
        for (dx, dy) in [(-d, -d), (d, -d), (-d, d), (d, d)] {
            out.push(Shape::Text { x: tx + dx, y: ty + dy, size, text: text.clone(), anchor, colour: halo, up: false });
        }
        out.push(Shape::Text { x: tx, y: ty, size, text, anchor, colour: INK, up: false });
    }
}

/// The world's land (Natural Earth's at 1:110 million, public domain): rings of longitudes and latitudes.
fn land() -> &'static [Vec<(f32, f32)>] {
    static LAND: std::sync::OnceLock<Vec<Vec<(f32, f32)>>> = std::sync::OnceLock::new();
    LAND.get_or_init(|| {
        let b: &[u8] = include_bytes!("../assets/ne_110m_land.bin");
        let u16_at = |i: usize| u16::from_le_bytes([b[i], b[i + 1]]) as usize;
        let i16_at = |i: usize| i16::from_le_bytes([b[i], b[i + 1]]) as f32 / 100.0;
        let mut at = 6;
        let mut rings = Vec::new();
        for _ in 0..u16_at(4) {
            let n = u16_at(at);
            at += 2;
            rings.push((0..n).map(|k| (i16_at(at + 4 * k), i16_at(at + 4 * k + 2))).collect());
            at += 4 * n;
        }
        rings
    })
}

/// A grid's longitudes and latitudes: west, south, east, north (its edges sampled, in WGS 84).
fn lonlat(g: &terraflux_raster::Grid) -> Option<[f64; 4]> {
    let (west, south, east, north) = extent(g);
    if g.geo.is_geographic() == Some(true) {
        return Some([west, south, east, north]);
    }
    let crs = g.geo.wkt.clone().or_else(|| g.geo.epsg().map(|c| format!("EPSG:{c}")))?;
    let tr = terraflux_gdal::Transform::new(&crs, "EPSG:4326").ok()?;
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for k in 0..=10 {
        let f = k as f64 / 10.0;
        for (x, y) in [
            (west + f * (east - west), south),
            (west + f * (east - west), north),
            (west, south + f * (north - south)),
            (east, south + f * (north - south)),
        ] {
            xs.push(x);
            ys.push(y);
        }
    }
    tr.points(&mut xs, &mut ys);
    let ok = |v: &f64| v.is_finite();
    let lo = |v: &[f64]| v.iter().copied().filter(ok).fold(f64::INFINITY, f64::min);
    let hi = |v: &[f64]| v.iter().copied().filter(ok).fold(f64::NEG_INFINITY, f64::max);
    let b = [lo(&xs), lo(&ys), hi(&xs), hi(&ys)];
    (b.iter().all(|v| v.is_finite()) && b[2] >= b[0] && b[3] >= b[1]).then_some(b)
}

/// The location inset in the map's bottom right corner: the land around (several times the map's span, 24
/// degrees at least) and the map's extent in red.
fn inset(out: &mut Vec<Shape>, g: &terraflux_raster::Grid, (mx, my, mw, mh): (f32, f32, f32, f32)) {
    let Some([w, s, e, n]) = lonlat(g) else { return };
    let (iw, ih) = ((mw * 0.3).clamp(42.0, 110.0), (mw * 0.3).clamp(42.0, 110.0) * 0.72);
    let (x0, y0) = (mx + mw - iw - 4.0, my + mh - ih - 4.0);
    // longitudes shrunk by the cosine of the middle latitude, as the eye expects them there
    let (cx, cy) = ((w + e) / 2.0, (s + n) / 2.0);
    let k = cy.to_radians().cos().max(0.2);
    let span_x = (((e - w) * k).max(n - s) * 6.0).max(24.0 * k).min(360.0 * k);
    let span_y = span_x * f64::from(ih / iw);
    let at = |lon: f64, lat: f64| -> (f32, f32) {
        (x0 + (((lon - cx) * k / span_x + 0.5) as f32) * iw, y0 + ((0.5 - (lat - cy) / span_y) as f32) * ih)
    };
    out.push(Shape::Rect { x: x0, y: y0, w: iw, h: ih, fill: Some(Color32::from_rgb(232, 240, 247)), stroke: None });
    out.push(Shape::Clip(Some((x0, y0, iw, ih))));
    let rings: Vec<Vec<(f32, f32)>> =
        land().iter().map(|r| r.iter().map(|&(lon, lat)| at(f64::from(lon), f64::from(lat))).collect()).collect();
    let grey = Color32::from_rgb(205, 205, 200);
    out.push(Shape::Path {
        rings,
        closed: true,
        fill: Some(grey),
        stroke: Some((Color32::from_rgb(150, 150, 145), 0.25)),
    });
    let (a, b) = (at(w, n), at(e, s));
    let red = Color32::from_rgb(214, 39, 40);
    if b.0 - a.0 < 3.0 || b.1 - a.1 < 3.0 {
        // too small to see as a box: a ring around its middle
        let (px, py) = at(cx, cy);
        let ring = (0..20).map(|i| {
            let t = i as f32 / 20.0 * std::f32::consts::TAU;
            (px + 3.0 * t.cos(), py + 3.0 * t.sin())
        });
        out.push(Shape::Path { rings: vec![ring.collect()], closed: true, fill: None, stroke: Some((red, 0.9)) });
    } else {
        out.push(Shape::Rect { x: a.0, y: a.1, w: b.0 - a.0, h: b.1 - a.1, fill: None, stroke: Some((red, 0.9)) });
    }
    out.push(Shape::Clip(None));
    out.push(Shape::Rect { x: x0, y: y0, w: iw, h: ih, fill: None, stroke: Some((INK, 0.5)) });
}

/// The figure of the panels laid out: its shapes, its size. `px_per_pt` sets how finely the maps are read.
pub fn lay_out(panels: &[Panel], l: &Layout, px_per_pt: f32) -> Result<Drawing, String> {
    if panels.is_empty() {
        return Err("no map".into());
    }
    let t = Type::new();
    let mut out = Vec::new();
    let width = l.width_mm / 25.4 * 72.0;
    let (margin, gap) = (8.0, 12.0);
    let mut y = margin;
    if !l.title.trim().is_empty() {
        out.push(Shape::Text {
            x: margin,
            y: y + 10.0,
            size: 11.0,
            text: l.title.trim().to_string(),
            anchor: Anchor::Start,
            colour: INK,
            up: false,
        });
        y += 18.0;
    }
    // one legend for all when they share their colours, at the right; else each map's under it
    let shared = panels.iter().all(|p| p.coloring.key() == panels[0].coloring.key());
    let side = if l.legend && shared { legend_size(&t, &panels[0]).0 + gap } else { 0.0 };
    let n = panels.len();
    let cols = l.columns.clamp(1, n);
    let (left, bottom) = if l.grid { (13.0, 11.0) } else { (0.0, 0.0) };
    let cell = (width - 2.0 * margin - side - (cols - 1) as f32 * gap) / cols as f32;
    let map_w = cell - left;
    if map_w < 30.0 {
        return Err("the figure is too narrow for its maps: widen it or take fewer columns".into());
    }
    let letters = n > 1;
    let captions = letters || panels.iter().any(|p| !p.caption.is_empty());
    let top_of_grid = y;
    for (row, chunk) in panels.chunks(cols).enumerate() {
        let mut row_h: f32 = 0.0;
        for (c, p) in chunk.iter().enumerate() {
            let i = row * cols + c;
            let x0 = margin + c as f32 * (cell + gap);
            let mut yy = y;
            if captions {
                let text = if letters {
                    format!("({}) {}", (b'a' + (i % 26) as u8) as char, p.caption)
                } else {
                    p.caption.clone()
                };
                out.push(Shape::Text {
                    x: x0 + left,
                    y: yy + 8.0,
                    size: 8.5,
                    text: text.trim().to_string(),
                    anchor: Anchor::Start,
                    colour: INK,
                    up: false,
                });
                yy += 12.0;
            }
            let g = p.source.grid();
            let (west, south, east, north) = extent(g);
            let aspect = if east > west { ((north - south) / (east - west)) as f32 } else { 1.0 };
            let map_h = map_w * aspect.clamp(0.05, 20.0);
            let mx = x0 + left;
            let (rgba, iw, ih) = picture(p, (map_w * px_per_pt).ceil() as usize)?;
            out.push(Shape::Image { x: mx, y: yy, w: map_w, h: map_h, rgba, iw, ih });
            let geo = georeferenced(g);
            let geographic = g.geo.is_geographic() == Some(true);
            let at_x = |v: f64| mx + ((v - west) / (east - west)) as f32 * map_w;
            let at_y = |v: f64| yy + ((north - v) / (north - south)) as f32 * map_h;
            if l.grid && east > west && north > south {
                // lines across the map at round coordinates, their values outside it
                let step = nice_step((east - west).max(north - south), 4.0);
                let mut v = (west / step).ceil() * step;
                while v <= east + 1e-9 * step {
                    let px = at_x(v);
                    out.push(Shape::Line { points: vec![(px, yy), (px, yy + map_h)], colour: GRID, width: 0.3 });
                    out.push(Shape::Text {
                        x: px,
                        y: yy + map_h + 8.0,
                        size: 6.0,
                        text: coordinate(v, step, geographic, true),
                        anchor: Anchor::Middle,
                        colour: INK,
                        up: false,
                    });
                    v += step;
                }
                let mut v = (south / step).ceil() * step;
                while v <= north + 1e-9 * step {
                    let py = at_y(v);
                    out.push(Shape::Line { points: vec![(mx, py), (mx + map_w, py)], colour: GRID, width: 0.3 });
                    out.push(Shape::Text {
                        x: mx - 3.0,
                        y: py,
                        size: 6.0,
                        text: coordinate(v, step, geographic, false),
                        anchor: Anchor::Middle,
                        colour: INK,
                        up: true,
                    });
                    v += step;
                }
            }
            // a north-up map: the vectors over it, in its coordinates, inside it
            let north_up = g.transform[2] == 0.0 && g.transform[4] == 0.0 && g.transform[5] < 0.0;
            if north_up && east > west && north > south && !p.vectors.is_empty() {
                out.push(Shape::Clip(Some((mx, yy, map_w, map_h))));
                for v in &p.vectors {
                    vectors(&mut out, v, &|x, y| (at_x(x), at_y(y)));
                }
                out.push(Shape::Clip(None));
            }
            out.push(Shape::Rect { x: mx, y: yy, w: map_w, h: map_h, fill: None, stroke: Some((INK, 0.5)) });
            // where in the world: bottom right
            if l.inset && geo && map_w > 90.0 && map_h > 60.0 {
                inset(&mut out, g, (mx, yy, map_w, map_h));
            }
            // a north arrow, top right, on a north-up map
            if l.north && north_up && map_h > 40.0 {
                let (cx, top) = (mx + map_w - 10.0, yy + 4.0);
                out.push(Shape::Rect { x: cx - 7.0, y: top, w: 14.0, h: 24.0, fill: Some(HALO), stroke: None });
                out.push(Shape::Text {
                    x: cx,
                    y: top + 9.0,
                    size: 8.0,
                    text: "N".into(),
                    anchor: Anchor::Middle,
                    colour: INK,
                    up: false,
                });
                let (tip, base, notch) = (top + 10.5, top + 22.0, top + 19.0);
                out.push(Shape::Polygon {
                    points: vec![(cx, tip), (cx - 4.5, base), (cx, notch)],
                    fill: INK,
                    stroke: Some((INK, 0.4)),
                });
                out.push(Shape::Polygon {
                    points: vec![(cx, tip), (cx + 4.5, base), (cx, notch)],
                    fill: Color32::WHITE,
                    stroke: Some((INK, 0.4)),
                });
            }
            // a scale bar, bottom left: about a quarter of the map, in metres or kilometres
            if l.scale_bar && geo && east > west && map_h > 30.0 {
                let metres_per_unit = if geographic {
                    let lat = ((north + south) / 2.0).to_radians();
                    111_320.0 * lat.cos().max(0.01)
                } else {
                    g.geo.linear_unit()
                };
                let map_m = (east - west) * metres_per_unit;
                let len_m = nice_step(map_m / 4.0, 1.0);
                let len = (len_m / map_m) as f32 * map_w;
                if len > 8.0 {
                    let km = len_m >= 1000.0;
                    let f = |m: f64| crate::animation::short(if km { m / 1000.0 } else { m });
                    let unit = if km { "km" } else { "m" };
                    let (bx, by) = (mx + 6.0, yy + map_h - 9.0);
                    let label = format!("{} {unit}", f(len_m));
                    let lw = t.width(6.5, &label);
                    out.push(Shape::Rect {
                        x: bx - 3.0,
                        y: by - 9.0,
                        w: len + lw / 2.0 + 7.0,
                        h: 15.0,
                        fill: Some(HALO),
                        stroke: None,
                    });
                    for k in 0..4 {
                        let fill = if k % 2 == 0 { INK } else { Color32::WHITE };
                        out.push(Shape::Rect {
                            x: bx + len * k as f32 / 4.0,
                            y: by,
                            w: len / 4.0,
                            h: 2.6,
                            fill: Some(fill),
                            stroke: Some((INK, 0.35)),
                        });
                    }
                    for (at, text) in [(0.0, "0".to_string()), (0.5, f(len_m / 2.0)), (1.0, label)] {
                        out.push(Shape::Text {
                            x: bx + len * at,
                            y: by - 2.0,
                            size: 6.5,
                            text,
                            anchor: Anchor::Middle,
                            colour: INK,
                            up: false,
                        });
                    }
                }
            }
            let mut h = yy - y + map_h + bottom;
            if l.legend && !shared {
                let (_, lh) = legend(&t, &mut out, p, mx, y + h + 4.0);
                h += lh + 6.0;
            }
            row_h = row_h.max(h);
        }
        y += row_h + gap;
    }
    if l.legend && shared {
        let x = width - margin - side + gap;
        legend(&t, &mut out, &panels[0], x, top_of_grid + if captions { 12.0 } else { 0.0 });
    }
    let h = (y - gap + margin).max(top_of_grid + 80.0);
    Ok(Drawing { w: width, h, shapes: out })
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// A colour as SVG takes it, and its opacity.
fn svg_colour(c: Color32) -> (String, f32) {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    (format!("#{r:02x}{g:02x}{b:02x}"), a as f32 / 255.0)
}

fn paint_attrs(kind: &str, c: Color32) -> String {
    let (hex, a) = svg_colour(c);
    if a < 1.0 { format!(" {kind}=\"{hex}\" {kind}-opacity=\"{a:.3}\"") } else { format!(" {kind}=\"{hex}\"") }
}

/// A picture of a PDF: width, height, RGB deflated, alpha deflated (when not all opaque).
type PdfImage = (usize, usize, Vec<u8>, Option<Vec<u8>>);

impl Drawing {
    /// The figure as SVG, `width_mm` wide: its text as text (in the application's font, else a sans serif).
    pub fn svg(&self, width_mm: f32) -> String {
        let h_mm = width_mm * self.h / self.w;
        let mut s = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" \
             xmlns:xlink=\"http://www.w3.org/1999/xlink\" width=\"{width_mm:.2}mm\" height=\"{h_mm:.2}mm\" viewBox=\"0 0 {:.3} {:.3}\" \
             font-family=\"'Source Sans 3', 'Source Sans Pro', Arial, sans-serif\">\n\
             <rect width=\"100%\" height=\"100%\" fill=\"#ffffff\"/>\n",
            self.w, self.h
        );
        let mut clips = 0;
        for sh in &self.shapes {
            match sh {
                Shape::Clip(Some((x, y, w, h))) => {
                    clips += 1;
                    s.push_str(&format!(
                        "<clipPath id=\"clip{clips}\"><rect x=\"{x:.3}\" y=\"{y:.3}\" width=\"{w:.3}\" height=\"{h:.3}\"/></clipPath>\n<g clip-path=\"url(#clip{clips})\">\n"
                    ));
                }
                Shape::Clip(None) => s.push_str("</g>\n"),
                Shape::Path { rings, closed, fill, stroke } => {
                    let mut d = String::new();
                    for r in rings.iter().filter(|r| r.len() >= 2) {
                        for (i, (x, y)) in r.iter().enumerate() {
                            d.push_str(&format!("{}{x:.2},{y:.2} ", if i == 0 { "M" } else { "L" }));
                        }
                        if *closed {
                            d.push_str("Z ");
                        }
                    }
                    let f = fill.map_or(" fill=\"none\"".to_string(), |c| paint_attrs("fill", c));
                    let st = stroke.map_or(String::new(), |(c, w)| {
                        format!("{} stroke-width=\"{w}\" stroke-linejoin=\"round\"", paint_attrs("stroke", c))
                    });
                    s.push_str(&format!("<path d=\"{}\" fill-rule=\"evenodd\"{f}{st}/>\n", d.trim_end()));
                }
                Shape::Image { x, y, w, h, rgba, iw, ih } => {
                    let png = png_of(rgba, *iw, *ih).unwrap_or_default();
                    // (xlink:href too: SVG Tiny and older programs read only it)
                    let data = format!("data:image/png;base64,{}", crate::report::base64(&png));
                    s.push_str(&format!(
                        "<image x=\"{x:.3}\" y=\"{y:.3}\" width=\"{w:.3}\" height=\"{h:.3}\" preserveAspectRatio=\"none\" \
                         style=\"image-rendering:pixelated\" href=\"{data}\" xlink:href=\"{data}\"/>\n"
                    ));
                }
                Shape::Rect { x, y, w, h, fill, stroke } => {
                    let f = fill.map_or(" fill=\"none\"".to_string(), |c| paint_attrs("fill", c));
                    let st = stroke
                        .map_or(String::new(), |(c, w)| format!("{} stroke-width=\"{w}\"", paint_attrs("stroke", c)));
                    s.push_str(&format!(
                        "<rect x=\"{x:.3}\" y=\"{y:.3}\" width=\"{w:.3}\" height=\"{h:.3}\"{f}{st}/>\n"
                    ));
                }
                Shape::Line { points, colour, width } => {
                    let p: Vec<String> = points.iter().map(|(x, y)| format!("{x:.3},{y:.3}")).collect();
                    s.push_str(&format!(
                        "<polyline points=\"{}\" fill=\"none\"{} stroke-width=\"{width}\"/>\n",
                        p.join(" "),
                        paint_attrs("stroke", *colour)
                    ));
                }
                Shape::Polygon { points, fill, stroke } => {
                    let p: Vec<String> = points.iter().map(|(x, y)| format!("{x:.3},{y:.3}")).collect();
                    let st = stroke
                        .map_or(String::new(), |(c, w)| format!("{} stroke-width=\"{w}\"", paint_attrs("stroke", c)));
                    s.push_str(&format!("<polygon points=\"{}\"{}{st}/>\n", p.join(" "), paint_attrs("fill", *fill)));
                }
                Shape::Text { x, y, size, text, anchor, colour, up } => {
                    let a = match anchor {
                        Anchor::Start => "start",
                        Anchor::Middle => "middle",
                    };
                    let rot = if *up { format!(" transform=\"rotate(-90 {x:.3} {y:.3})\"") } else { String::new() };
                    s.push_str(&format!(
                        "<text x=\"{x:.3}\" y=\"{y:.3}\" font-size=\"{size}\" text-anchor=\"{a}\"{}{rot}>{}</text>\n",
                        paint_attrs("fill", *colour),
                        esc(text)
                    ));
                }
            }
        }
        s.push_str("</svg>\n");
        s
    }

    /// The figure as a PDF of its own size: the lines, shapes and text as they are (the text in Helvetica), the
    /// maps as pictures (their transparent cells kept so, by a soft mask).
    pub fn pdf(&self, title: &str) -> Vec<u8> {
        use std::fmt::Write as _;
        let page_h = self.h;
        let mut c = String::new();
        // the opacities used, of fills and of strokes (ExtGStates /Fa.. and /Sa..)
        let (mut fills, mut strokes): (Vec<u8>, Vec<u8>) = (Vec::new(), Vec::new());
        // the pictures: width, height, RGB deflated, alpha deflated (when not all opaque)
        let mut images: Vec<PdfImage> = Vec::new();
        let colour = |c: Color32| {
            let [r, g, b, _] = c.to_srgba_unmultiplied();
            format!("{:.3} {:.3} {:.3}", r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0)
        };
        let mut paint = |c: &mut String, fill: Option<Color32>, stroke: Option<(Color32, f32)>| {
            if let Some(f) = fill {
                let a = f.to_srgba_unmultiplied()[3];
                if a < 255 {
                    if !fills.contains(&a) {
                        fills.push(a);
                    }
                    let _ = write!(c, "/Fa{a} gs ");
                }
                let _ = write!(c, "{} rg ", colour(f));
            }
            if let Some((s, w)) = stroke {
                let a = s.to_srgba_unmultiplied()[3];
                if a < 255 {
                    if !strokes.contains(&a) {
                        strokes.push(a);
                    }
                    let _ = write!(c, "/Sa{a} gs ");
                }
                let _ = write!(c, "{} RG {w:.2} w 1 j ", colour(s));
            }
            match (fill.is_some(), stroke.is_some()) {
                (true, true) => "B",
                (true, false) => "f",
                (false, true) => "S",
                (false, false) => "n",
            }
        };
        let text = |t: &str| {
            let mut s = String::new();
            for b in t.chars().map(|c| if c == '\u{2009}' { ' ' } else { c }).flat_map(crate::report::ansi) {
                if matches!(b, b'(' | b')' | b'\\') {
                    s.push('\\');
                }
                if b < 128 {
                    s.push(b as char);
                } else {
                    let _ = write!(s, "\\{b:03o}");
                }
            }
            s
        };
        for sh in &self.shapes {
            match sh {
                Shape::Clip(Some((x, y, w, h))) => {
                    let _ = writeln!(c, "q {x:.2} {:.2} {w:.2} {h:.2} re W n", page_h - y - h);
                }
                Shape::Clip(None) => c.push_str("Q\n"),
                Shape::Image { x, y, w, h, rgba, iw, ih } => {
                    let (mut rgb, mut alpha) = (Vec::with_capacity(iw * ih * 3), Vec::with_capacity(iw * ih));
                    for px in rgba.chunks(4) {
                        let a = px[3];
                        let un = |v: u8| {
                            if a == 0 { 255 } else { ((v as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 }
                        };
                        rgb.extend([un(px[0]), un(px[1]), un(px[2])]);
                        alpha.push(a);
                    }
                    let mask =
                        alpha.iter().any(|a| *a < 255).then(|| miniz_oxide::deflate::compress_to_vec_zlib(&alpha, 6));
                    images.push((*iw, *ih, miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6), mask));
                    let _ =
                        writeln!(c, "q {w:.3} 0 0 {h:.3} {x:.3} {:.3} cm /Im{} Do Q", page_h - y - h, images.len() - 1);
                }
                Shape::Rect { x, y, w, h, fill, stroke } => {
                    c.push_str("q ");
                    let op = paint(&mut c, *fill, *stroke);
                    let _ = writeln!(c, "{x:.3} {:.3} {w:.3} {h:.3} re {op} Q", page_h - y - h);
                }
                Shape::Line { points, colour: col, width } => {
                    c.push_str("q ");
                    let op = paint(&mut c, None, Some((*col, *width)));
                    for (i, (x, y)) in points.iter().enumerate() {
                        let _ = write!(c, "{x:.3} {:.3} {} ", page_h - y, if i == 0 { "m" } else { "l" });
                    }
                    let _ = writeln!(c, "{op} Q");
                }
                Shape::Polygon { points, fill, stroke } => {
                    c.push_str("q ");
                    let op = paint(&mut c, Some(*fill), *stroke);
                    for (i, (x, y)) in points.iter().enumerate() {
                        let _ = write!(c, "{x:.3} {:.3} {} ", page_h - y, if i == 0 { "m" } else { "l" });
                    }
                    let _ = writeln!(c, "h {op} Q");
                }
                Shape::Path { rings, closed, fill, stroke } => {
                    c.push_str("q ");
                    let op = paint(&mut c, *fill, *stroke);
                    for r in rings.iter().filter(|r| r.len() >= 2) {
                        for (i, (x, y)) in r.iter().enumerate() {
                            let _ = write!(c, "{x:.2} {:.2} {} ", page_h - y, if i == 0 { "m" } else { "l" });
                        }
                        if *closed {
                            c.push_str("h ");
                        }
                    }
                    // (the even-odd rule: a polygon's holes left out)
                    let op = match op {
                        "f" => "f*",
                        "B" => "B*",
                        o => o,
                    };
                    let _ = writeln!(c, "{op} Q");
                }
                Shape::Text { x, y, size, text: t, anchor, colour: col, up } => {
                    let w = crate::report::helvetica_width(*size, t);
                    let back = match anchor {
                        Anchor::Start => 0.0,
                        Anchor::Middle => w / 2.0,
                    };
                    let (px, py) = if *up { (*x, page_h - y - back) } else { (x - back, page_h - y) };
                    let m = if *up { format!("0 1 -1 0 {px:.3} {py:.3}") } else { format!("1 0 0 1 {px:.3} {py:.3}") };
                    let _ = writeln!(c, "q {} rg BT /F1 {size} Tf {m} Tm ({}) Tj ET Q", colour(*col), text(t));
                }
            }
        }
        // objects: 1 catalog, 2 pages, 3 the page, 4 its content, 5 the font, 6 the information, then the pictures
        // (and their masks)
        let mut objects: Vec<Vec<u8>> = Vec::new();
        objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        objects.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
        let mut xo = String::new();
        let mut next = 7;
        let mut image_objects = Vec::new();
        for (k, im) in images.iter().enumerate() {
            let _ = write!(xo, "/Im{k} {next} 0 R ");
            image_objects.push(next);
            next += if im.3.is_some() { 2 } else { 1 };
        }
        let mut gs = String::new();
        for a in &fills {
            let _ = write!(gs, "/Fa{a} << /ca {:.3} >> ", *a as f32 / 255.0);
        }
        for a in &strokes {
            let _ = write!(gs, "/Sa{a} << /CA {:.3} >> ", *a as f32 / 255.0);
        }
        objects.push(
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.3} {:.3}] /Resources << /Font << /F1 5 0 R >> \
                 /XObject << {xo}>> /ExtGState << {gs}>> >> /Contents 4 0 R >>",
                self.w, self.h
            )
            .into_bytes(),
        );
        let mut content = format!("<< /Length {} >>\nstream\n", c.len()).into_bytes();
        content.extend(c.as_bytes());
        content.extend(b"\nendstream");
        objects.push(content);
        objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".to_vec());
        objects.push(
            format!("<< /Title ({}) /Producer (Terraflux {}) >>", text(title), env!("CARGO_PKG_VERSION")).into_bytes(),
        );
        for (k, (w, h, data, mask)) in images.iter().enumerate() {
            let smask = if mask.is_some() { format!("/SMask {} 0 R ", image_objects[k] + 1) } else { String::new() };
            let mut o = format!(
                "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 \
                 {smask}/Filter /FlateDecode /Length {} >>\nstream\n",
                data.len()
            )
            .into_bytes();
            o.extend(data);
            o.extend(b"\nendstream");
            objects.push(o);
            if let Some(m) = mask {
                let mut o = format!(
                    "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceGray \
                     /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>\nstream\n",
                    m.len()
                )
                .into_bytes();
                o.extend(m);
                o.extend(b"\nendstream");
                objects.push(o);
            }
        }
        let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::new();
        for (k, o) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n", k + 1).as_bytes());
            out.extend(o);
            out.extend(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for o in offsets {
            out.extend(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend(
            format!("trailer\n<< /Size {} /Root 1 0 R /Info 6 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1)
                .as_bytes(),
        );
        out
    }

    /// The figure as pixels at `dpi` (premultiplied RGBA), on white.
    pub fn pixels(&self, dpi: u32) -> Result<tiny_skia::Pixmap, String> {
        self.pixels_on(dpi, tiny_skia::Color::WHITE)
    }

    /// The figure as pixels at `dpi` over a background (premultiplied RGBA).
    fn pixels_on(&self, dpi: u32, background: tiny_skia::Color) -> Result<tiny_skia::Pixmap, String> {
        let k = dpi as f32 / 72.0;
        let (w, h) = ((self.w * k).round() as u32, (self.h * k).round() as u32);
        if w as u64 * h as u64 > 40_000 * 40_000 {
            return Err("the figure is too large at this resolution".into());
        }
        let mut pm = tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("no picture of this size")?;
        pm.fill(background);
        let tr = tiny_skia::Transform::from_scale(k, k);
        let t = Type::new();
        let paint = |c: Color32| {
            let mut p = tiny_skia::Paint::default();
            let [r, g, b, a] = c.to_srgba_unmultiplied();
            p.set_color_rgba8(r, g, b, a);
            p.anti_alias = true;
            p
        };
        let mut clip: Option<tiny_skia::Mask> = None;
        for sh in &self.shapes {
            let mask = clip.as_ref();
            match sh {
                Shape::Clip(Some((x, y, w, h))) => {
                    let mut m = tiny_skia::Mask::new(pm.width(), pm.height()).ok_or("no mask of this size")?;
                    if let Some(r) = tiny_skia::Rect::from_xywh(*x, *y, *w, *h) {
                        m.fill_path(&tiny_skia::PathBuilder::from_rect(r), tiny_skia::FillRule::Winding, true, tr);
                    }
                    clip = Some(m);
                }
                Shape::Clip(None) => clip = None,
                Shape::Path { rings, closed, fill, stroke } => {
                    let mut pb = tiny_skia::PathBuilder::new();
                    for r in rings.iter().filter(|r| r.len() >= 2) {
                        for (i, (x, y)) in r.iter().enumerate() {
                            if i == 0 { pb.move_to(*x, *y) } else { pb.line_to(*x, *y) }
                        }
                        if *closed {
                            pb.close();
                        }
                    }
                    if let Some(path) = pb.finish() {
                        if let Some(c) = fill {
                            pm.fill_path(&path, &paint(*c), tiny_skia::FillRule::EvenOdd, tr, mask);
                        }
                        if let Some((c, sw)) = stroke {
                            let st = tiny_skia::Stroke {
                                width: *sw,
                                line_join: tiny_skia::LineJoin::Round,
                                ..Default::default()
                            };
                            pm.stroke_path(&path, &paint(*c), &st, tr, mask);
                        }
                    }
                }
                Shape::Image { x, y, w, h, rgba, iw, ih } => {
                    let Some(size) = tiny_skia::IntSize::from_wh(*iw as u32, *ih as u32) else { continue };
                    let Some(img) = tiny_skia::Pixmap::from_vec(rgba.to_vec(), size) else { continue };
                    let pp =
                        tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Nearest, ..Default::default() };
                    let place = tiny_skia::Transform::from_row(w / *iw as f32, 0.0, 0.0, h / *ih as f32, *x, *y);
                    pm.draw_pixmap(0, 0, img.as_ref(), &pp, place.post_concat(tr), mask);
                }
                Shape::Rect { x, y, w, h, fill, stroke } => {
                    let Some(r) = tiny_skia::Rect::from_xywh(*x, *y, *w, *h) else { continue };
                    let path = tiny_skia::PathBuilder::from_rect(r);
                    if let Some(c) = fill {
                        let mut p = paint(*c);
                        p.anti_alias = false;
                        pm.fill_path(&path, &p, tiny_skia::FillRule::Winding, tr, mask);
                    }
                    if let Some((c, sw)) = stroke {
                        let st = tiny_skia::Stroke { width: *sw, ..Default::default() };
                        pm.stroke_path(&path, &paint(*c), &st, tr, mask);
                    }
                }
                Shape::Line { points, colour, width } => {
                    let mut pb = tiny_skia::PathBuilder::new();
                    for (i, (x, y)) in points.iter().enumerate() {
                        if i == 0 { pb.move_to(*x, *y) } else { pb.line_to(*x, *y) }
                    }
                    if let Some(path) = pb.finish() {
                        let st = tiny_skia::Stroke { width: *width, ..Default::default() };
                        pm.stroke_path(&path, &paint(*colour), &st, tr, None);
                    }
                }
                Shape::Polygon { points, fill, stroke } => {
                    let mut pb = tiny_skia::PathBuilder::new();
                    for (i, (x, y)) in points.iter().enumerate() {
                        if i == 0 { pb.move_to(*x, *y) } else { pb.line_to(*x, *y) }
                    }
                    pb.close();
                    if let Some(path) = pb.finish() {
                        pm.fill_path(&path, &paint(*fill), tiny_skia::FillRule::Winding, tr, None);
                        if let Some((c, sw)) = stroke {
                            let st = tiny_skia::Stroke { width: *sw, ..Default::default() };
                            pm.stroke_path(&path, &paint(*c), &st, tr, None);
                        }
                    }
                }
                Shape::Text { x, y, size, text, anchor, colour, up } => {
                    if let Some(path) = text_path(&t, *x, *y, *size, text, *anchor, *up) {
                        pm.fill_path(&path, &paint(*colour), tiny_skia::FillRule::Winding, tr, None);
                    }
                }
            }
        }
        Ok(pm)
    }
}

/// A line of text as the outlines of its letters (points), from its point on the baseline.
fn text_path(t: &Type, x: f32, y: f32, size: f32, text: &str, anchor: Anchor, up: bool) -> Option<tiny_skia::Path> {
    let k = size / t.upem;
    let width = t.width(size, text);
    let start = match anchor {
        Anchor::Start => 0.0,
        Anchor::Middle => -width / 2.0,
    };
    // (u along the baseline, v downwards) to the page: as written, or turned to read upwards
    let place = |u: f32, v: f32| if up { (x + v, y - u) } else { (x + u, y + v) };
    let mut pb = tiny_skia::PathBuilder::new();
    let mut pen = start;
    let mut last_id = None;
    for c in text.chars() {
        let id = t.font.glyph_id(c);
        if let Some(p) = last_id {
            pen += t.font.kern_unscaled(p, id) * k;
        }
        if let Some(o) = t.font.outline(id) {
            let mut at: Option<ab_glyph::Point> = None;
            let pt = |p: ab_glyph::Point| place(pen + p.x * k, -p.y * k);
            for curve in &o.curves {
                let (p0, end) = match curve {
                    OutlineCurve::Line(a, b) => (*a, *b),
                    OutlineCurve::Quad(a, _, b) => (*a, *b),
                    OutlineCurve::Cubic(a, _, _, b) => (*a, *b),
                };
                if at != Some(p0) {
                    if at.is_some() {
                        pb.close();
                    }
                    let (mx, my) = pt(p0);
                    pb.move_to(mx, my);
                }
                match curve {
                    OutlineCurve::Line(_, b) => {
                        let (bx, by) = pt(*b);
                        pb.line_to(bx, by);
                    }
                    OutlineCurve::Quad(_, c1, b) => {
                        let ((cx, cy), (bx, by)) = (pt(*c1), pt(*b));
                        pb.quad_to(cx, cy, bx, by);
                    }
                    OutlineCurve::Cubic(_, c1, c2, b) => {
                        let ((ax, ay), (cx, cy), (bx, by)) = (pt(*c1), pt(*c2), pt(*b));
                        pb.cubic_to(ax, ay, cx, cy, bx, by);
                    }
                }
                at = Some(end);
            }
            if at.is_some() {
                pb.close();
            }
        }
        pen += t.font.h_advance_unscaled(id) * k;
        last_id = Some(id);
    }
    pb.finish()
}

impl Drawing {
    /// A drawing of a map with nothing behind it (premultiplied RGBA, transparent where nothing is drawn), a
    /// point to a pixel.
    pub fn clear_pixels(&self) -> Result<tiny_skia::Pixmap, String> {
        self.pixels_on(72, tiny_skia::Color::TRANSPARENT)
    }

    /// An empty drawing of `w` x `h` points, for a chart laid out by hand.
    pub(crate) fn blank(w: f32, h: f32) -> Drawing {
        Drawing { w, h, shapes: Vec::new() }
    }

    pub(crate) fn rect(&mut self, (x, y, w, h): (f32, f32, f32, f32), fill: Option<Color32>) {
        self.shapes.push(Shape::Rect { x, y, w, h, fill, stroke: None });
    }

    pub(crate) fn polyline(&mut self, points: Vec<(f32, f32)>, colour: Color32, width: f32) {
        self.shapes.push(Shape::Line { points, colour, width });
    }

    /// A filled disc.
    pub(crate) fn dot(&mut self, x: f32, y: f32, r: f32, fill: Color32) {
        let ring = (0..16)
            .map(|i| {
                let a = i as f32 / 16.0 * std::f32::consts::TAU;
                (x + r * a.cos(), y + r * a.sin())
            })
            .collect();
        self.shapes.push(Shape::Path { rings: vec![ring], closed: true, fill: Some(fill), stroke: None });
    }

    /// A line of text at a point on its baseline: from there, or centred on it; `up` reads it upwards.
    pub(crate) fn label(&mut self, (x, y): (f32, f32), size: f32, text: &str, middle: bool, colour: Color32, up: bool) {
        let anchor = if middle { Anchor::Middle } else { Anchor::Start };
        self.shapes.push(Shape::Text { x, y, size, text: text.into(), anchor, colour, up });
    }
}

/// A text's width in points at a size, in the figures' font.
pub(crate) fn text_width(size: f32, text: &str) -> f32 {
    Type::new().width(size, text)
}

/// A picture as PNG (from premultiplied RGBA).
fn png_of(rgba: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    let size = tiny_skia::IntSize::from_wh(w as u32, h as u32)?;
    tiny_skia::Pixmap::from_vec(rgba.to_vec(), size)?.encode_png().ok()
}

/// The figure written: as SVG or PDF when the path says so, else as PNG at the layout's DPI (which the file
/// records).
pub fn save(panels: &[Panel], l: &Layout, path: &std::path::Path) -> Result<(), String> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let (svg, pdf) = (ext == "svg", ext == "pdf");
    // the maps read as finely as the resolution shows them (SVG and PDF: as for the DPI chosen, 300 at least)
    let dpi = if svg || pdf { l.dpi.max(300) } else { l.dpi };
    let d = lay_out(panels, l, dpi as f32 / 72.0)?;
    if svg {
        return std::fs::write(path, d.svg(l.width_mm)).map_err(|e| e.to_string());
    }
    if pdf {
        return std::fs::write(path, d.pdf(&l.title)).map_err(|e| e.to_string());
    }
    let pm = d.pixels(l.dpi)?;
    let bytes = pm.encode_png().map_err(|e| e.to_string())?;
    // the resolution, so that a word processor places the figure at its width
    std::fs::write(path, with_dpi(&bytes, l.dpi)).map_err(|e| e.to_string())
}

/// A PNG with its resolution (a `pHYs` chunk after the header), so that it is placed at its size.
fn with_dpi(png: &[u8], dpi: u32) -> Vec<u8> {
    // signature (8) and IHDR (4 + 4 + 13 + 4)
    let at = 8 + 25;
    if png.len() < at {
        return png.to_vec();
    }
    let ppm = (dpi as f64 / 0.0254).round() as u32;
    let mut data = Vec::with_capacity(13);
    data.extend_from_slice(b"pHYs");
    data.extend_from_slice(&ppm.to_be_bytes());
    data.extend_from_slice(&ppm.to_be_bytes());
    data.push(1);
    let crc = crc32(&data);
    let mut out = Vec::with_capacity(png.len() + 21);
    out.extend_from_slice(&png[..at]);
    out.extend_from_slice(&9u32.to_be_bytes());
    out.extend_from_slice(&data);
    out.extend_from_slice(&crc.to_be_bytes());
    out.extend_from_slice(&png[at..]);
    out
}

pub(crate) fn crc32(bytes: &[u8]) -> u32 {
    let mut c = 0xffff_ffffu32;
    for &b in bytes {
        c ^= b as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

/// The window of a figure: the maps it may show, how it is laid out, its preview, and the file being written.
pub struct Composer {
    /// The maps it may show: by a name ("this map", "this map and the compared one", "every layer").
    pub choices: Vec<(String, Vec<Panel>)>,
    pub chosen: usize,
    pub layout: Layout,
    preview: Option<(Layout, usize, egui::TextureHandle, egui::Vec2, Option<String>)>,
    job: Option<(PathBuf, Receiver<Result<(), String>>)>,
    said: Option<String>,
}

impl Composer {
    pub fn new(choices: Vec<(String, Vec<Panel>)>, title: String) -> Composer {
        let one = choices.first().is_some_and(|c| c.1.len() == 1);
        let layout = Layout {
            title,
            columns: if one { 1 } else { 2 },
            width_mm: if one { 85.0 } else { 180.0 },
            ..Default::default()
        };
        Composer { choices, chosen: 0, layout, preview: None, job: None, said: None }
    }
}

/// The window, while a figure is being composed.
pub fn window(ctx: &egui::Context, s: &mut crate::app::Session) {
    let Some(c) = &mut s.figure else { return };
    if let Some((path, rx)) = &c.job
        && let Ok(r) = rx.try_recv()
    {
        c.said = Some(match r {
            Ok(()) => format!("saved: {}", path.display()),
            Err(e) => format!("not saved: {e}"),
        });
        c.job = None;
    }
    let mut open = true;
    egui::Window::new("Figure").open(&mut open).default_size([760.0, 560.0]).resizable(true).collapsible(false).show(
        ctx,
        |ui| {
            egui::Panel::left("figure settings").resizable(false).default_size(230.0).show(ui, |ui| {
                let l = &mut c.layout;
                ui.label("Title");
                ui.add(egui::TextEdit::singleline(&mut l.title).hint_text("none").desired_width(210.0));
                if c.choices.len() > 1 {
                    ui.label("Maps");
                    for (i, (name, _)) in c.choices.iter().enumerate() {
                        ui.radio_value(&mut c.chosen, i, name);
                    }
                }
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut l.width_mm).range(40.0..=400.0).speed(1.0).suffix(" mm"));
                });
                ui.horizontal(|ui| {
                    for (w, name, tip) in
                        [(85.0, "1 column", "a journal's column"), (180.0, "page", "the page's width")]
                    {
                        if ui.small_button(name).on_hover_text(tip).clicked() {
                            l.width_mm = w;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Resolution");
                    egui::ComboBox::from_id_salt("figure dpi").selected_text(format!("{} DPI", l.dpi)).show_ui(
                        ui,
                        |ui| {
                            for d in [150, 300, 600, 1200] {
                                ui.selectable_value(&mut l.dpi, d, format!("{d} DPI"));
                            }
                        },
                    );
                });
                let n = c.choices.get(c.chosen).map_or(1, |p| p.1.len());
                if n > 1 {
                    ui.horizontal(|ui| {
                        ui.label("Columns");
                        ui.add(egui::DragValue::new(&mut l.columns).range(1..=n.min(6)));
                    });
                }
                ui.add_space(4.0);
                ui.checkbox(&mut l.legend, "Legend");
                ui.checkbox(&mut l.scale_bar, "Scale bar");
                ui.checkbox(&mut l.north, "North arrow");
                ui.checkbox(&mut l.grid, "Coordinates");
                ui.checkbox(&mut l.inset, "Location inset")
                    .on_hover_text("a small map of where in the world the map is, in its bottom right corner");
                ui.add_space(8.0);
                let busy = c.job.is_some();
                ui.horizontal(|ui| {
                    for (label, ext, filter) in [
                        ("Save PNG...", "png", "PNG picture"),
                        ("Save SVG...", "svg", "SVG drawing"),
                        ("Save PDF...", "pdf", "PDF document"),
                    ] {
                        if ui.add_enabled(!busy, egui::Button::new(label)).clicked()
                            && let Some(path) = rfd::FileDialog::new()
                                .set_title("Save the figure")
                                .add_filter(filter, &[ext])
                                .set_file_name(format!("figure.{ext}"))
                                .save_file()
                        {
                            let panels = c.choices.get(c.chosen).map(|p| p.1.clone()).unwrap_or_default();
                            let layout = c.layout.clone();
                            let (tx, rx) = channel();
                            let (to, ctx) = (path.clone(), ui.ctx().clone());
                            std::thread::spawn(move || {
                                let _ = tx.send(save(&panels, &layout, &to));
                                ctx.request_repaint();
                            });
                            c.job = Some((path, rx));
                            c.said = None;
                        }
                    }
                });
                if busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new("writing the figure...").weak());
                    });
                } else if let Some(t) = &c.said {
                    ui.label(RichText::new(t).size(11.0).weak());
                }
                ui.label(
                    RichText::new(
                        "SVG and PDF keep the text as text and the lines as lines (SVG to change in Inkscape or \
                         Illustrator); PNG records its resolution.",
                    )
                    .size(10.5)
                    .weak(),
                );
            });
            // the preview: drawn again when the layout or the maps change
            let fresh = c.preview.as_ref().is_some_and(|p| p.0 == c.layout && p.1 == c.chosen);
            if !fresh {
                let panels = c.choices.get(c.chosen).map(|p| p.1.as_slice()).unwrap_or_default();
                let dpi = (560.0 / (c.layout.width_mm / 25.4)).clamp(30.0, 300.0);
                let made = lay_out(panels, &c.layout, dpi / 72.0).and_then(|d| d.pixels(dpi.round() as u32));
                let (img, err) = match made {
                    Ok(pm) => (
                        egui::ColorImage::from_rgba_premultiplied(
                            [pm.width() as usize, pm.height() as usize],
                            pm.data(),
                        ),
                        None,
                    ),
                    Err(e) => (egui::ColorImage::new([1, 1], vec![Color32::WHITE]), Some(e)),
                };
                let size = egui::vec2(img.size[0] as f32, img.size[1] as f32);
                let tex = ui.ctx().load_texture("figure preview", img, egui::TextureOptions::LINEAR);
                c.preview = Some((c.layout.clone(), c.chosen, tex, size, err));
            }
            if let Some((_, _, tex, size, err)) = &c.preview {
                egui::ScrollArea::both().show(ui, |ui| {
                    if let Some(e) = err {
                        ui.colored_label(ui.visuals().error_fg_color, e);
                    } else {
                        let w = ui.available_width().min(size.x);
                        let sz = *size * (w / size.x);
                        egui::Frame::new().stroke(egui::Stroke::new(1.0, Color32::from_gray(90))).show(ui, |ui| {
                            ui.add(egui::Image::new((tex.id(), sz)));
                        });
                    }
                });
            }
        },
    );
    if !open {
        s.figure = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Map;

    fn panel(caption: &str, f: impl Fn(usize) -> f64, coloring: Coloring) -> Panel {
        let mut g = terraflux_raster::Grid::new(60, 40);
        g.transform = [500000.0, 100.0, 0.0, 9000000.0, 0.0, -100.0];
        g.geo.wkt = Some("PROJCS[\"WGS 84 / UTM zone 23S\",GEOGCS[\"WGS 84\"]]".into());
        let m = Map::new(Arc::new(g), (0..2400).map(f).collect());
        Panel {
            source: Source::Map(m),
            coloring,
            names: vec![(1.0, "forest".into())],
            caption: caption.into(),
            vectors: Vec::new(),
            rasters: Vec::new(),
        }
    }

    fn classes() -> Coloring {
        Coloring::Classes(vec![(1.0, Color32::from_rgb(0, 158, 115)), (2.0, Color32::from_rgb(230, 159, 0))])
    }

    #[test]
    fn steps_and_labels() {
        assert_eq!(nice_step(6000.0, 4.0), 2000.0);
        assert_eq!(nice_step(0.9, 4.0), 0.2);
        assert_eq!(coordinate(9_002_000.0, 2000.0, false, false), "9\u{2009}002\u{2009}000");
        assert_eq!(coordinate(-47.5, 0.5, true, true), "47.5°W");
        assert_eq!(coordinate(-15.0, 5.0, true, false), "15°S");
    }

    #[test]
    fn a_figure_of_two_maps() {
        let a = panel("2000", |i| if i % 60 < 30 { 1.0 } else { 2.0 }, classes());
        let b = panel("2020", |i| if i % 60 < 20 { 1.0 } else { 2.0 }, classes());
        let l = Layout { title: "Land use".into(), width_mm: 180.0, ..Default::default() };
        let d = lay_out(&[a.clone(), b.clone()], &l, 300.0 / 72.0).unwrap();
        // 180 mm wide; two maps of 3 to 2 side by side
        assert!((d.w - 510.236).abs() < 0.01, "{}", d.w);
        assert!(d.h > 150.0 && d.h < 260.0, "{}", d.h);
        let svg = d.svg(180.0);
        assert!(svg.contains("width=\"180.00mm\""));
        assert!(svg.contains(">(a) 2000</text>") && svg.contains(">(b) 2020</text>"), "{svg}");
        assert!(svg.contains(">Land use</text>"));
        // one legend (the same colours): its classes named once
        assert_eq!(svg.matches(">forest</text>").count(), 1);
        assert_eq!(svg.matches("<image ").count(), 2);
        assert_eq!(svg.matches(" xlink:href=\"data:image/png;base64,").count(), 2);
        // the scale bar (6 km: a quarter of a map is 1.5 km), the north arrow, the coordinates
        assert!(svg.contains(">2 km</text>") || svg.contains(">1 km</text>"), "{svg}");
        assert_eq!(svg.matches(">N</text>").count(), 2);
        assert!(svg.contains("rotate(-90"));
        // drawn at 300 DPI: 2126 pixels wide, white around, the forest's green on the map
        let pm = d.pixels(300).unwrap();
        assert_eq!(pm.width(), 2126);
        let px = pm.pixel(5, 5).unwrap();
        assert_eq!((px.red(), px.green(), px.blue()), (255, 255, 255));
        let green = pm.pixels().iter().filter(|p| (p.red(), p.green(), p.blue()) == (0, 158, 115)).count();
        assert!(green > 100_000, "{green}");
        // the letters drawn: dark pixels in the title's line
        let dark = (0..60).flat_map(|y| (0..600).map(move |x| (x, y))).filter(|&(x, y)| {
            let p = pm.pixel(x, y).unwrap();
            p.red() < 100
        });
        assert!(dark.count() > 200);
    }

    #[test]
    fn maps_in_their_own_colours_have_their_own_legends() {
        let ramp = Coloring::Continuous { min: 0.0, max: 100.0, ramp: Default::default(), log: false };
        let a = panel("classes", |i| 1.0 + (i % 2) as f64, classes());
        let b = panel("values", |i| (i % 100) as f64, ramp);
        let l = Layout { columns: 1, grid: false, ..Default::default() };
        let svg = lay_out(&[a, b], &l, 1.0).unwrap().svg(180.0);
        assert!(svg.contains(">forest</text>") && svg.contains(">100</text>") && svg.contains(">50</text>"));
        assert!(!svg.contains("rotate(-90"), "no coordinates");
    }

    #[test]
    fn the_legend_of_vectors_colours_and_sizes() {
        use crate::vector_tiles::{Breaks, Style, Symbol, look};
        use terraflux_raster::vector::{Column, Geometry, Vector};
        let n = [1.5, 2.5, 3.5, 4.5, 8.0, 9.0, 10.0, 12.0];
        let v = Arc::new(Vector {
            geometries: (0..8).map(|i| Geometry::Point([500000.0 + 300.0 * i as f64, 8999000.0])).collect(),
            attributes: vec![
                ("area".into(), Column::Numbers(n.to_vec())),
                ("use".into(), Column::Texts((0..8).map(|i| ["wood", "farm"][i % 2].to_string()).collect())),
            ],
            crs: None,
        });
        let with = |style: Style| {
            let mut p = panel("map", |i| 1.0 + (i % 2) as f64, classes());
            p.vectors = vec![Vectors::of(v.clone(), look(&v, &style), &style)];
            p
        };
        let l = Layout { columns: 1, grid: false, ..Default::default() };
        // classes of a number: a range each, in its colour
        let s = Style { by: Some("area".into()), breaks: Breaks::Quantiles, classes: 4, ..Style::default() };
        let p = with(s.clone());
        let svg = lay_out(std::slice::from_ref(&p), &l, 1.0).unwrap().svg(180.0);
        assert!(svg.contains(">area</text>") && svg.contains(">1.5 \u{2013} 2.5</text>"), "{svg}");
        assert!(svg.contains(">forest</text>"), "the map's own legend stays");
        let c0 = crate::vector_tiles::class_of_ramp(s.ramp, 0, 4);
        assert!(svg.contains(&format!("#{:02x}{:02x}{:02x}", c0.r(), c0.g(), c0.b())));
        // categories of a text, sizes by an attribute: circles with their values
        let s =
            Style { by: Some("use".into()), size_by: Some("area".into()), symbol: Symbol::Square, ..Style::default() };
        let svg = lay_out(&[with(s)], &l, 1.0).unwrap().svg(180.0);
        assert!(
            svg.contains(">farm</text>") && svg.contains(">wood</text>") && svg.contains(">size: area</text>"),
            "{svg}"
        );
        assert!(svg.contains(">1.5</text>") && svg.contains(">12</text>"), "{svg}");
        // the ramp: its ends
        let r = Style { by: Some("area".into()), ..Style::default() };
        let svg = lay_out(&[with(r)], &l, 1.0).unwrap().svg(180.0);
        assert!(svg.contains(">area</text>") && svg.contains(">12</text>") && svg.contains(">1.5</text>"), "{svg}");
        // one colour and one size: nothing but the map's legend
        let svg = lay_out(&[with(Style::default())], &l, 1.0).unwrap().svg(180.0);
        assert!(!svg.contains("size:") && !svg.contains(">area</text>"));
        // and as a PNG too (no panic: drawn)
        assert!(lay_out(&[p], &l, 1.0).unwrap().pixels(72).is_ok());
    }

    #[test]
    fn vectors_over_the_map_an_inset_and_a_pdf() {
        use terraflux_raster::vector::{Geometry, Vector};
        let mut g = terraflux_raster::Grid::new(60, 40);
        g.transform = [500000.0, 100.0, 0.0, 9000000.0, 0.0, -100.0];
        g.geo.wkt = Some(terraflux_gdal::crs_wkt("EPSG:32723").unwrap());
        let m = Map::new(Arc::new(g), (0..2400).map(|i| 1.0 + (i % 2) as f64).collect());
        let magenta = Color32::from_rgb(255, 0, 255);
        // a polygon with a hole, a line beyond the map (cut at its edge), a point
        let ring = |x0: f64, y0: f64, x1: f64, y1: f64| vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]];
        let v = Vector {
            geometries: vec![
                Geometry::Polygon(vec![
                    ring(501000.0, 8997000.0, 505000.0, 8999000.0),
                    ring(502000.0, 8997500.0, 504000.0, 8998500.0),
                ]),
                Geometry::LineString(vec![[499000.0, 8996500.0], [507000.0, 8996500.0]]),
                Geometry::Point([500500.0, 8999500.0]),
            ],
            attributes: Vec::new(),
            crs: None,
        };
        let a = Panel {
            source: Source::Map(m),
            coloring: classes(),
            names: Vec::new(),
            caption: String::new(),
            vectors: vec![Vectors {
                vector: Arc::new(v),
                colours: vec![magenta; 3],
                fill: 0.2,
                width: 1.0,
                labels: None,
                label_size: 6.0,
                sizes: Vec::new(),
                symbol: Default::default(),
                legend: crate::vector_tiles::Look::default().legend,
            }],
            rasters: Vec::new(),
        };
        let l = Layout { width_mm: 120.0, columns: 1, legend: false, ..Default::default() };
        let d = lay_out(std::slice::from_ref(&a), &l, 2.0).unwrap();
        let svg = d.svg(120.0);
        // labelled by an attribute: its values drawn, each over its halo
        let mut named = a.clone();
        let mut v = (*named.vectors[0].vector).clone();
        v.attributes = vec![(
            "name".into(),
            terraflux_raster::vector::Column::Texts(vec!["plot".into(), "road".into(), "well".into()]),
        )];
        named.vectors[0].vector = Arc::new(v);
        named.vectors[0].labels = Some("name".into());
        let labelled = lay_out(std::slice::from_ref(&named), &l, 2.0).unwrap().svg(120.0);
        for t in ["plot", "road", "well"] {
            assert_eq!(labelled.matches(&format!(">{t}</text>")).count(), 5, "{t} and its halo: {labelled}");
        }
        assert_eq!(svg.matches("</text>").count() + 15, labelled.matches("</text>").count());
        // clipped to the map, and the inset to its box; the hole by the even-odd rule
        assert_eq!(svg.matches("<clipPath ").count(), 2, "{svg}");
        assert!(svg.contains("fill-rule=\"evenodd\"") && svg.contains("stroke=\"#ff00ff\""));
        // the inset: the land, and the map's place in red (São Paulo's zone: a ring, the map is small)
        assert!(svg.contains("fill=\"#cdcdc8\"") && svg.contains("stroke=\"#d62728\""));
        let pm = d.pixels(300).unwrap();
        let count = |c: (u8, u8, u8)| pm.pixels().iter().filter(|p| (p.red(), p.green(), p.blue()) == c).count();
        assert!(count((255, 0, 255)) > 300, "the outlines");
        assert!(count((205, 205, 200)) > 300, "the land");
        // nothing drawn left of the map: the line cut at its edge
        let left = (0..pm.height()).filter(|&y| {
            let p = pm.pixel(30, y).unwrap();
            (p.red(), p.green(), p.blue()) == (255, 0, 255)
        });
        assert_eq!(left.count(), 0);
        // as PDF: one page of the figure's size, vectors and text
        let pdf = d.pdf("A figure");
        let text = String::from_utf8_lossy(&pdf);
        assert!(text.starts_with("%PDF-1.4") && text.contains("/MediaBox [0 0 340.15"), "{}", &text[..400]);
        assert!(text.contains(" re W n") && text.contains("B*") && text.contains("(N) Tj"));
        assert!(text.contains("/Title (A figure)") && text.contains("/Subtype /Image"));
        // without the inset: no land
        let l = Layout { inset: false, ..l };
        assert!(!lay_out(&[a], &l, 2.0).unwrap().svg(120.0).contains("#cdcdc8"));
    }

    /// Where labels collide the larger feature's wins, whatever the order of the layer: a polygon before a line before
    /// a point, the larger first, points in their order.
    #[test]
    fn a_label_collision_keeps_the_larger_feature() {
        use terraflux_raster::vector::{Column, Geometry, Vector};
        let square = |r: f64| Geometry::Polygon(vec![vec![[-r, -r], [r, -r], [r, r], [-r, r], [-r, -r]]]);
        let v = Vector {
            // the small ones first in the layer, all labelled at the same point (0, 0)
            geometries: vec![
                Geometry::Point([0.0, 0.0]),
                square(1.0),
                Geometry::LineString(vec![[-2.0, 0.0], [2.0, 0.0]]),
                square(8.0),
                square(4.0),
            ],
            attributes: vec![(
                "name".into(),
                Column::Texts(vec!["point".into(), "small".into(), "line".into(), "big".into(), "middle".into()]),
            )],
            crs: None,
        };
        let layer = Vectors {
            vector: Arc::new(v),
            colours: Vec::new(),
            fill: 0.0,
            width: 1.0,
            labels: Some("name".into()),
            label_size: 6.0,
            sizes: Vec::new(),
            symbol: Default::default(),
            legend: Default::default(),
        };
        let mut out = Vec::new();
        // (one unit is 10 points: the labels of a few characters are wider than a unit, so they cover each other)
        labels(&mut out, &layer, "name", &|x, y| ((x * 10.0) as f32, (y * 10.0) as f32));
        let drawn: Vec<&str> = out
            .iter()
            .filter_map(|s| match s {
                Shape::Text { text, colour, .. } if *colour == INK => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(drawn, ["big"], "the larger polygon's label, the others covered");
        // the order of the others when the big one is not there: the larger polygon, then the line, then the point
        let order = |names: &[&str]| {
            let mut ranks: Vec<(usize, crate::labels::Rank)> = vec![
                (0, crate::labels::rank(crate::vector_tiles::Kind::Point, [0.0; 4])),
                (1, crate::labels::rank(crate::vector_tiles::Kind::Polygon, [-1.0, -1.0, 1.0, 1.0])),
                (2, crate::labels::rank(crate::vector_tiles::Kind::Line, [-2.0, 0.0, 2.0, 0.0])),
                (3, crate::labels::rank(crate::vector_tiles::Kind::Polygon, [-8.0, -8.0, 8.0, 8.0])),
                (4, crate::labels::rank(crate::vector_tiles::Kind::Polygon, [-4.0, -4.0, 4.0, 4.0])),
            ];
            ranks.sort_by(|a, b| crate::labels::by_rank(&a.1, &b.1));
            ranks.iter().map(|(i, _)| names[*i].to_string()).collect::<Vec<_>>()
        };
        assert_eq!(order(&["point", "small", "line", "big", "middle"]), ["big", "middle", "small", "line", "point"]);
    }

    #[test]
    fn a_png_records_its_resolution() {
        let dir = std::env::temp_dir().join(format!("terraflux-figure-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = panel("", |i| 1.0 + (i % 2) as f64, classes());
        let l = Layout { width_mm: 85.0, dpi: 300, ..Default::default() };
        let png = dir.join("f.png");
        save(std::slice::from_ref(&a), &l, &png).unwrap();
        let bytes = std::fs::read(&png).unwrap();
        let at = bytes.windows(4).position(|w| w == b"pHYs").unwrap();
        assert_eq!(u32::from_be_bytes(bytes[at + 4..at + 8].try_into().unwrap()), 11811);
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!(img.width(), 1004);
        save(&[a], &l, &dir.join("f.svg")).unwrap();
        assert!(std::fs::read_to_string(dir.join("f.svg")).unwrap().starts_with("<?xml"));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
