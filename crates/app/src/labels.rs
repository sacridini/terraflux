//! Labels on vector layers: the text of an attribute over each feature, at one point worked out once with the layer
//! (a polygon's inner point, a line's middle, beside a point), drawn only for the features in view that are large
//! enough for it, a label that would cover one placed left out, so that a layer of a hundred thousand features
//! costs a frame what a couple of thousand labels do.

use std::collections::HashMap;

use egui::{Color32, Pos2, Rect, Vec2};
use terraflux_raster::vector::{Column, Vector};

use crate::vector_tiles::{Camera, Kind, Layer, Overlay};

/// The labels placed in a frame, at most.
pub const MAX: usize = 2000;
/// The features looked at in a frame, at most (the rest of a huge layer in view is left out).
const LOOKED: usize = 20_000;
/// The side of a square of the grid that finds the labels placed near a rectangle, points.
const BUCKET: f32 = 64.0;

/// The labels placed so far: a rectangle may be added only when it covers none of them.
#[derive(Default)]
pub struct Placed {
    boxes: Vec<Rect>,
    grid: HashMap<(i32, i32), Vec<u32>>,
}

impl Placed {
    fn squares(r: Rect) -> impl Iterator<Item = (i32, i32)> {
        let (x0, y0) = ((r.min.x / BUCKET).floor() as i32, (r.min.y / BUCKET).floor() as i32);
        let (x1, y1) = ((r.max.x / BUCKET).floor() as i32, (r.max.y / BUCKET).floor() as i32);
        (y0..=y1).flat_map(move |y| (x0..=x1).map(move |x| (x, y)))
    }

    /// How many are placed.
    pub fn len(&self) -> usize {
        self.boxes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.boxes.is_empty()
    }

    /// Whether a point is over one placed.
    pub fn covers(&self, p: Pos2) -> bool {
        self.grid
            .get(&((p.x / BUCKET).floor() as i32, (p.y / BUCKET).floor() as i32))
            .is_some_and(|v| v.iter().any(|&i| self.boxes[i as usize].contains(p)))
    }

    /// Place a rectangle if it covers none placed (those that only touch it do not): whether it was.
    pub fn take(&mut self, r: Rect) -> bool {
        let over = |a: &Rect| a.min.x < r.max.x && r.min.x < a.max.x && a.min.y < r.max.y && r.min.y < a.max.y;
        let free = Self::squares(r)
            .all(|s| self.grid.get(&s).is_none_or(|v| !v.iter().any(|&i| over(&self.boxes[i as usize]))));
        if free {
            let i = self.boxes.len() as u32;
            self.boxes.push(r);
            for s in Self::squares(r) {
                self.grid.entry(s).or_default().push(i);
            }
        }
        free
    }
}

/// Whether a point is inside the rings of a polygon, holes out (the even-odd rule).
fn inside(rings: &[&[[f64; 2]]], p: [f64; 2]) -> bool {
    let mut odd = false;
    for ring in rings {
        let n = ring.len();
        for i in 0..n {
            let (a, b) = (ring[i], ring[(i + 1) % n]);
            if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
                odd = !odd;
            }
        }
    }
    odd
}

/// A ring's signed area and its centroid.
fn centroid(ring: &[[f64; 2]]) -> (f64, [f64; 2]) {
    let Some(o) = ring.first().copied() else { return (0.0, [0.0, 0.0]) };
    let (mut a, mut cx, mut cy) = (0.0, 0.0, 0.0);
    for i in 0..ring.len() {
        let (p, q) = (ring[i], ring[(i + 1) % ring.len()]);
        let (px, py, qx, qy) = (p[0] - o[0], p[1] - o[1], q[0] - o[0], q[1] - o[1]);
        let w = px * qy - qx * py;
        a += w;
        cx += (px + qx) * w;
        cy += (py + qy) * w;
    }
    if a == 0.0 { (0.0, o) } else { (a / 2.0, [o[0] + cx / (3.0 * a), o[1] + cy / (3.0 * a)]) }
}

/// A polygon's inner point: the centroid of its largest ring when it is inside, else the middle of the longest
/// stretch inside along a few horizontal lines across it (near the pole of inaccessibility of a thin or bent one).
pub fn polygon_anchor(rings: &[&[[f64; 2]]]) -> Option<[f64; 2]> {
    let main =
        rings.iter().filter(|r| !r.is_empty()).max_by(|a, b| centroid(a).0.abs().total_cmp(&centroid(b).0.abs()))?;
    let (area, c) = centroid(main);
    if area != 0.0 && inside(rings, c) {
        return Some(c);
    }
    let (y0, y1) = main.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), p| (l.min(p[1]), h.max(p[1])));
    let mut best: Option<(f64, [f64; 2])> = None;
    for i in 1..=9 {
        let y = y0 + (y1 - y0) * (f64::from(i) / 10.0 + 0.00137);
        let mut xs = Vec::new();
        for ring in rings {
            let n = ring.len();
            for j in 0..n {
                let (a, b) = (ring[j], ring[(j + 1) % n]);
                if (a[1] > y) != (b[1] > y) {
                    xs.push(a[0] + (y - a[1]) * (b[0] - a[0]) / (b[1] - a[1]));
                }
            }
        }
        xs.sort_by(f64::total_cmp);
        for &[x0, x1] in xs.as_chunks::<2>().0 {
            let len = x1 - x0;
            if best.is_none_or(|(l, _)| len > l) {
                best = Some((len, [(x0 + x1) / 2.0, y]));
            }
        }
    }
    best.map(|(_, p)| p).or_else(|| main.first().copied())
}

/// A line's middle: the point half way along its longest part.
pub fn line_anchor(lines: &[&[[f64; 2]]]) -> Option<[f64; 2]> {
    let length = |l: &[[f64; 2]]| l.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum::<f64>();
    let line = lines.iter().filter(|l| !l.is_empty()).max_by(|a, b| length(a).total_cmp(&length(b)))?;
    let mut left = length(line) / 2.0;
    for w in line.windows(2) {
        let d = (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1]);
        if left <= d && d > 0.0 {
            let t = left / d;
            return Some([w[0][0] + t * (w[1][0] - w[0][0]), w[0][1] + t * (w[1][1] - w[0][1])]);
        }
        left -= d;
    }
    line.first().copied()
}

/// Where a feature is labelled: a polygon's inner point, a line's middle, a point's first vertex.
pub fn anchor(kind: Kind, parts: &[&[[f64; 2]]]) -> Option<[f64; 2]> {
    match kind {
        Kind::Polygon => polygon_anchor(parts),
        Kind::Line => line_anchor(parts),
        Kind::Point => parts.iter().find_map(|p| p.first().copied()),
    }
}

/// A feature's label: the value of an attribute (none when the layer has not the attribute, or the text is empty).
pub fn text(v: &Vector, attr: &str, k: usize) -> Option<String> {
    let t = match v.column(attr)? {
        Column::Numbers(x) => crate::table_view::cell(*x.get(k)?),
        Column::Texts(t) => t.get(k)?.clone(),
    };
    (!t.trim().is_empty()).then_some(t)
}

/// The labels of the layers over the view, the topmost layer's first: the number placed.
pub fn draw(painter: &egui::Painter, cam: Camera, layers: &[(&Overlay, &Layer)], placed: &mut Placed) -> usize {
    let (a, b) = (
        cam.center + (cam.rect.min - cam.rect.center()) / cam.zoom,
        cam.center + (cam.rect.max - cam.rect.center()) / cam.zoom,
    );
    let (mut looked, mut drawn) = (0, 0);
    for (o, layer) in layers.iter().rev() {
        let Some(attr) = &o.style.labels else { continue };
        let size = o.style.label_size.clamp(6.0, 40.0);
        let font = egui::FontId::proportional(size);
        for k in layer.within([f64::from(a.x), f64::from(a.y)], [f64::from(b.x), f64::from(b.y)]) {
            if placed.len() >= MAX || looked >= LOOKED {
                return drawn;
            }
            looked += 1;
            let (Some(c), Some(kind), Some(bx)) = (layer.anchor(k), layer.kind(k), layer.feature_box(k)) else {
                continue;
            };
            let at = cam.rect.center() + (Vec2::new(c[0] as f32, c[1] as f32) - cam.center) * cam.zoom;
            let (w, h) = ((bx[2] - bx[0]) as f32 * cam.zoom, (bx[3] - bx[1]) as f32 * cam.zoom);
            // a polygon too small for its label, a point under one placed, one off the view: no text laid out
            if (kind == Kind::Polygon && h < size) || !cam.rect.contains(at) || placed.covers(at) {
                continue;
            }
            let Some(t) = text(&o.vector, attr, k) else { continue };
            let galley = painter.layout_no_wrap(t, font.clone(), Color32::PLACEHOLDER);
            let s = galley.size();
            if kind == Kind::Polygon && w < s.x {
                continue;
            }
            let min = if kind == Kind::Point { at + Vec2::new(5.0, -s.y / 2.0) } else { at - s / 2.0 };
            if !placed.take(Rect::from_min_size(min, s).expand(1.5)) {
                continue;
            }
            // (a thin halo: the text a little to every side, light and half seen, then the text)
            let halo = Color32::from_white_alpha(150);
            for d in [
                (-0.75, 0.0),
                (0.75, 0.0),
                (0.0, -0.75),
                (0.0, 0.75),
                (-0.5, -0.5),
                (0.5, -0.5),
                (-0.5, 0.5),
                (0.5, 0.5),
            ] {
                painter.galley(min + Vec2::new(d.0, d.1), galley.clone(), halo);
            }
            painter.galley(min, galley, Color32::from_rgb(24, 24, 28));
            drawn += 1;
        }
    }
    drawn
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: f32, y: f32, w: f32, h: f32) -> Rect {
        Rect::from_min_size(Pos2::new(x, y), Vec2::new(w, h))
    }

    #[test]
    fn a_square_is_labelled_at_its_middle() {
        let sq = [[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0], [0.0, 0.0]];
        let p = anchor(Kind::Polygon, &[&sq]).unwrap();
        assert!((p[0] - 2.0).abs() < 1e-9 && (p[1] - 2.0).abs() < 1e-9, "{p:?}");
    }

    #[test]
    fn a_u_shaped_polygon_is_labelled_inside_it() {
        // a U: its centroid is in the notch, outside
        let u = [[0.0, 0.0], [9.0, 0.0], [9.0, 10.0], [6.0, 10.0], [6.0, 3.0], [3.0, 3.0], [3.0, 10.0], [0.0, 10.0]];
        let (_, c) = centroid(&u);
        assert!(!inside(&[&u], c), "the centroid {c:?} is not in the notch");
        let p = anchor(Kind::Polygon, &[&u]).unwrap();
        assert!(inside(&[&u], p), "{p:?} is outside");
    }

    #[test]
    fn a_polygon_with_a_hole_is_not_labelled_in_it() {
        let outer = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let hole = [[1.0, 1.0], [1.0, 9.0], [9.0, 9.0], [9.0, 1.0]];
        let p = anchor(Kind::Polygon, &[&outer, &hole]).unwrap();
        assert!(inside(&[&outer, &hole], p), "{p:?} is in the hole");
    }

    #[test]
    fn a_line_is_labelled_at_its_middle_and_a_point_at_itself() {
        let l = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]];
        assert_eq!(anchor(Kind::Line, &[&l]), Some([10.0, 0.0]));
        let short = [[0.0, 0.0], [1.0, 0.0]];
        // the longest part
        assert_eq!(anchor(Kind::Line, &[&short, &l]), Some([10.0, 0.0]));
        let p = [[3.0, 4.0]];
        assert_eq!(anchor(Kind::Point, &[&p]), Some([3.0, 4.0]));
        assert_eq!(anchor(Kind::Point, &[]), None);
    }

    #[test]
    fn a_label_over_one_placed_is_left_out() {
        let mut p = Placed::default();
        assert!(p.take(rect(0.0, 0.0, 40.0, 12.0)));
        assert!(!p.take(rect(30.0, 6.0, 40.0, 12.0)), "overlaps");
        assert!(p.take(rect(40.0, 0.0, 40.0, 12.0)), "touches only");
        assert!(p.take(rect(0.0, 20.0, 40.0, 12.0)), "below");
        // a label across several squares of the grid
        assert!(!p.take(rect(70.0, 0.0, 200.0, 12.0)));
        assert!(p.take(rect(100.0, 0.0, 200.0, 12.0)));
        assert_eq!(p.len(), 4);
        assert!(p.covers(Pos2::new(10.0, 5.0)) && !p.covers(Pos2::new(10.0, 15.0)));
    }

    #[test]
    fn the_text_of_a_label_is_an_attribute_s_value() {
        let v = Vector {
            geometries: vec![],
            attributes: vec![
                ("area".into(), Column::Numbers(vec![12.5, f64::NAN])),
                ("use".into(), Column::Texts(vec!["forest".into(), " ".into()])),
            ],
            crs: None,
        };
        assert_eq!(text(&v, "area", 0).as_deref(), Some("12.5"));
        assert_eq!(text(&v, "area", 1).as_deref(), Some("NA"));
        assert_eq!(text(&v, "use", 0).as_deref(), Some("forest"));
        assert_eq!(text(&v, "use", 1), None);
        assert_eq!(text(&v, "none", 0), None);
        assert_eq!(text(&v, "use", 5), None);
    }
}
