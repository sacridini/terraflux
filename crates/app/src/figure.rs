//! A map as a figure for a paper: one map or several side by side (a), (b)..., each with its caption, its
//! coordinates along its edges, a scale bar, a north arrow and the legend, at the width of a journal's column and
//! the resolution it asks for. Laid out once in points (1/72 inch) as shapes, then written as SVG (text as text,
//! to edit in Inkscape or Illustrator; the maps as pictures in it) or drawn as PNG at the DPI chosen (the same
//! letters, as the font's outlines).

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use ab_glyph::{Font, FontRef, OutlineCurve};
use egui::{Color32, RichText};

use crate::map_view::Source;
use crate::palette::Coloring;

/// A map of the figure: what it shows, its colours, the names of its classes, its caption.
#[derive(Clone)]
pub struct Panel {
    pub source: Source,
    pub coloring: Coloring,
    pub names: Vec<(f64, String)>,
    pub caption: String,
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
fn nice_step(span: f64, n: f64) -> f64 {
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
fn coordinate(v: f64, step: f64, geographic: bool, east: bool) -> String {
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

/// The legend of a colouring: its height at `x`, `y` (top) when drawn, and its width.
fn legend(t: &Type, out: &mut Vec<Shape>, p: &Panel, x: f32, y: f32) -> (f32, f32) {
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
        Coloring::Continuous { min, max, ramp, log } => {
            // the ramp upwards, from the minimum to the maximum, labelled at both ends and the middle
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
            let mid = if *log && *min > 0.0 { (min * max).sqrt() } else { (min + max) / 2.0 };
            let mut w: f32 = 0.0;
            for (v, at) in [(*max, 0.0), (mid, 0.5), (*min, 1.0)] {
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
    let (v, w, h) = crate::animation::values(&p.source, px.max(16))?;
    let img = crate::palette::image_f32(&v, w, h, &p.coloring);
    let mut rgba = Vec::with_capacity(w * h * 4);
    for c in &img.pixels {
        rgba.extend_from_slice(&c.to_array());
    }
    Ok((Arc::new(rgba), w, h))
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
            out.push(Shape::Rect { x: mx, y: yy, w: map_w, h: map_h, fill: None, stroke: Some((INK, 0.5)) });
            // a north arrow, top right, on a north-up map
            let north_up = g.transform[2] == 0.0 && g.transform[4] == 0.0 && g.transform[5] < 0.0;
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
        for sh in &self.shapes {
            match sh {
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

    /// The figure as pixels at `dpi` (premultiplied RGBA): its width and height.
    pub fn pixels(&self, dpi: u32) -> Result<tiny_skia::Pixmap, String> {
        let k = dpi as f32 / 72.0;
        let (w, h) = ((self.w * k).round() as u32, (self.h * k).round() as u32);
        if w as u64 * h as u64 > 40_000 * 40_000 {
            return Err("the figure is too large at this resolution".into());
        }
        let mut pm = tiny_skia::Pixmap::new(w.max(1), h.max(1)).ok_or("no picture of this size")?;
        pm.fill(tiny_skia::Color::WHITE);
        let tr = tiny_skia::Transform::from_scale(k, k);
        let t = Type::new();
        let paint = |c: Color32| {
            let mut p = tiny_skia::Paint::default();
            let [r, g, b, a] = c.to_srgba_unmultiplied();
            p.set_color_rgba8(r, g, b, a);
            p.anti_alias = true;
            p
        };
        for sh in &self.shapes {
            match sh {
                Shape::Image { x, y, w, h, rgba, iw, ih } => {
                    let Some(size) = tiny_skia::IntSize::from_wh(*iw as u32, *ih as u32) else { continue };
                    let Some(img) = tiny_skia::Pixmap::from_vec(rgba.to_vec(), size) else { continue };
                    let pp =
                        tiny_skia::PixmapPaint { quality: tiny_skia::FilterQuality::Nearest, ..Default::default() };
                    let place = tiny_skia::Transform::from_row(w / *iw as f32, 0.0, 0.0, h / *ih as f32, *x, *y);
                    pm.draw_pixmap(0, 0, img.as_ref(), &pp, place.post_concat(tr), None);
                }
                Shape::Rect { x, y, w, h, fill, stroke } => {
                    let Some(r) = tiny_skia::Rect::from_xywh(*x, *y, *w, *h) else { continue };
                    let path = tiny_skia::PathBuilder::from_rect(r);
                    if let Some(c) = fill {
                        let mut p = paint(*c);
                        p.anti_alias = false;
                        pm.fill_path(&path, &p, tiny_skia::FillRule::Winding, tr, None);
                    }
                    if let Some((c, sw)) = stroke {
                        let st = tiny_skia::Stroke { width: *sw, ..Default::default() };
                        pm.stroke_path(&path, &paint(*c), &st, tr, None);
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

/// A picture as PNG (from premultiplied RGBA).
fn png_of(rgba: &[u8], w: usize, h: usize) -> Option<Vec<u8>> {
    let size = tiny_skia::IntSize::from_wh(w as u32, h as u32)?;
    tiny_skia::Pixmap::from_vec(rgba.to_vec(), size)?.encode_png().ok()
}

/// The figure written: as SVG when the path says so, else as PNG at the layout's DPI (which the file records).
pub fn save(panels: &[Panel], l: &Layout, path: &std::path::Path) -> Result<(), String> {
    let svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    // the maps read as finely as the resolution shows them (an SVG: as for 300 DPI)
    let dpi = if svg { 300 } else { l.dpi };
    let d = lay_out(panels, l, dpi as f32 / 72.0)?;
    if svg {
        return std::fs::write(path, d.svg(l.width_mm)).map_err(|e| e.to_string());
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

fn crc32(bytes: &[u8]) -> u32 {
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
                ui.add_space(8.0);
                let busy = c.job.is_some();
                ui.horizontal(|ui| {
                    for (label, ext, filter) in
                        [("Save PNG...", "png", "PNG picture"), ("Save SVG...", "svg", "SVG drawing")]
                    {
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
                        "SVG keeps the text as text, to change in Inkscape or Illustrator; PNG records its resolution.",
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
        Panel { source: Source::Map(m), coloring, names: vec![(1.0, "forest".into())], caption: caption.into() }
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
