//! Features selected by area on the map: shift+drag draws a rectangle, shift+alt+drag a lasso (a free-hand
//! polygon), and the features of a vector layer that meet the shape are selected on release (ctrl too: they are
//! added to the selection). The shape is kept in cells of the map's grid, as the layer's features are; a feature
//! meets it when a point of it is inside, or an edge of it crosses the shape's, or it holds the shape.

use egui::{Color32, Painter, Pos2, Stroke};

use crate::vector_tiles::Kind;

/// A shape being dragged on the map: its first point and the points since (a rectangle: its two corners; a lasso:
/// the path), in cells.
#[derive(Clone, Debug, PartialEq)]
pub struct Drag {
    pub lasso: bool,
    pub points: Vec<[f64; 2]>,
}

/// A shape finished: its polygon in cells, and whether it adds to the selection (else it replaces it).
#[derive(Clone, Debug, PartialEq)]
pub struct Done {
    pub shape: Vec<[f64; 2]>,
    pub add: bool,
}

impl Drag {
    pub fn new(lasso: bool, at: [f64; 2]) -> Drag {
        Drag { lasso, points: vec![at] }
    }

    /// The pointer at a cell: a lasso follows it (a point once it has moved `step` cells), a rectangle's corner
    /// is the last point.
    pub fn push(&mut self, at: [f64; 2], step: f64) {
        if self.lasso {
            let far = self.points.last().is_none_or(|l| (l[0] - at[0]).hypot(l[1] - at[1]) >= step);
            if far {
                self.points.push(at);
            }
        } else {
            self.points.truncate(1);
            self.points.push(at);
        }
    }

    /// The polygon it makes: a rectangle's four corners, a lasso's path; none while it is no area (a lasso of
    /// fewer than three points, a rectangle with no width or no height).
    pub fn polygon(&self) -> Option<Vec<[f64; 2]>> {
        if self.lasso {
            return (self.points.len() >= 3).then(|| self.points.clone());
        }
        let (a, b) = (*self.points.first()?, *self.points.get(1)?);
        (a[0] != b[0] && a[1] != b[1]).then(|| vec![a, [b[0], a[1]], b, [a[0], b[1]]])
    }

    /// The shape drawn translucent in the accent, its outline over it; `to_screen` carries a cell to a point.
    pub fn paint(&self, painter: &Painter, to_screen: impl Fn([f64; 2]) -> Pos2) {
        let accent = crate::theme::accent();
        let stroke = Stroke::new(1.5, accent);
        let fill = Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 45);
        let points: Vec<Pos2> = match self.polygon() {
            Some(p) => p.into_iter().map(&to_screen).collect(),
            None => self.points.iter().copied().map(&to_screen).collect(),
        };
        if points.len() > 2 && !self.lasso {
            painter.add(egui::epaint::PathShape::convex_polygon(points, fill, stroke));
        } else if points.len() > 2 {
            // (a lasso is seldom convex, and egui fills only convex shapes: its outline alone, closed)
            painter.add(egui::epaint::PathShape::closed_line(points, stroke));
        } else if points.len() == 2 {
            painter.line_segment([points[0], points[1]], stroke);
        }
    }
}

/// Whether a point is inside a polygon (even-odd rule).
pub fn point_in(shape: &[[f64; 2]], p: [f64; 2]) -> bool {
    let n = shape.len();
    let mut inside = false;
    for i in 0..n {
        let (a, b) = (shape[i], shape[(i + 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// Which side of the line `a`-`b` a point is on: positive left, negative right, zero on it.
fn side(a: [f64; 2], b: [f64; 2], p: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
}

/// Whether the segments `a`-`b` and `c`-`d` meet (touching counts).
fn meet(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> bool {
    let (d1, d2, d3, d4) = (side(c, d, a), side(c, d, b), side(a, b, c), side(a, b, d));
    if ((d1 > 0.0) != (d2 > 0.0)) && d1 != 0.0 && d2 != 0.0 && ((d3 > 0.0) != (d4 > 0.0)) && d3 != 0.0 && d4 != 0.0 {
        return true;
    }
    // (a point on the other's line: within its box)
    let within = |p: [f64; 2], q: [f64; 2], r: [f64; 2]| {
        r[0] >= p[0].min(q[0]) && r[0] <= p[0].max(q[0]) && r[1] >= p[1].min(q[1]) && r[1] <= p[1].max(q[1])
    };
    (d1 == 0.0 && within(c, d, a))
        || (d2 == 0.0 && within(c, d, b))
        || (d3 == 0.0 && within(a, b, c))
        || (d4 == 0.0 && within(a, b, d))
}

/// Whether a segment crosses an edge of the polygon.
fn crosses(a: [f64; 2], b: [f64; 2], shape: &[[f64; 2]]) -> bool {
    let n = shape.len();
    (0..n).any(|i| meet(a, b, shape[i], shape[(i + 1) % n]))
}

/// Whether a feature (its kind and parts, in cells) meets a polygon: a point of it inside, a line or a ring with a
/// vertex inside or an edge across the polygon's, a polygon that holds it (a hole holds nothing).
pub fn hits(kind: Kind, parts: &[&[[f64; 2]]], shape: &[[f64; 2]]) -> bool {
    if shape.len() < 3 {
        return false;
    }
    match kind {
        Kind::Point => parts.iter().flat_map(|p| p.iter()).any(|&p| point_in(shape, p)),
        Kind::Line | Kind::Polygon => {
            let closed = kind == Kind::Polygon;
            let edges = |part: &[[f64; 2]]| {
                let n = part.len();
                (0..n.saturating_sub(1) + usize::from(closed && n > 2))
                    .map(|i| (part[i], part[(i + 1) % n]))
                    .collect::<Vec<_>>()
            };
            if parts.iter().flat_map(|p| p.iter()).any(|&p| point_in(shape, p)) {
                return true;
            }
            if parts.iter().any(|part| edges(part).into_iter().any(|(a, b)| crosses(a, b, shape))) {
                return true;
            }
            // (the polygon around the shape, none of its edges crossing it: any corner of the shape is inside)
            closed && {
                let inside = parts.iter().flat_map(|ring| {
                    let n = ring.len();
                    (0..n).map(move |i| (ring[i], ring[(i + 1) % n]))
                });
                let q = shape[0];
                inside
                    .filter(|(a, b)| {
                        (a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0]
                    })
                    .count()
                    % 2
                    == 1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<[f64; 2]> {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }

    #[test]
    fn a_point_is_in_or_out() {
        let r = rect(0.0, 0.0, 4.0, 4.0);
        assert!(hits(Kind::Point, &[&[[1.0, 1.0]]], &r));
        assert!(!hits(Kind::Point, &[&[[5.0, 1.0]]], &r));
        // a multipoint: one of them in is enough
        assert!(hits(Kind::Point, &[&[[9.0, 9.0]], &[[2.0, 3.0]]], &r));
        // a lasso (a triangle): the box's corner is out
        let t = vec![[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
        assert!(hits(Kind::Point, &[&[[1.0, 1.0]]], &t));
        assert!(!hits(Kind::Point, &[&[[3.0, 3.0]]], &t));
    }

    #[test]
    fn a_line_in_across_or_beside() {
        let r = rect(0.0, 0.0, 4.0, 4.0);
        // inside, across (no vertex in), along outside, touching an edge
        assert!(hits(Kind::Line, &[&[[1.0, 1.0], [2.0, 3.0]]], &r));
        assert!(hits(Kind::Line, &[&[[-2.0, 2.0], [6.0, 2.0]]], &r));
        assert!(!hits(Kind::Line, &[&[[-2.0, 5.0], [6.0, 5.0]]], &r));
        assert!(!hits(Kind::Line, &[&[[5.0, -2.0], [5.0, 6.0]]], &r));
        assert!(hits(Kind::Line, &[&[[4.0, -2.0], [4.0, 6.0]]], &r));
        // a corner cut off a triangle lasso: the line near the hypotenuse
        let t = vec![[0.0, 0.0], [4.0, 0.0], [0.0, 4.0]];
        assert!(!hits(Kind::Line, &[&[[3.0, 3.0], [4.0, 4.0]]], &t));
        assert!(hits(Kind::Line, &[&[[3.0, 3.0], [1.0, 1.0]]], &t));
        // a part of several: one across
        assert!(hits(Kind::Line, &[&[[9.0, 9.0], [10.0, 9.0]], &[[-1.0, 1.0], [1.0, 1.0]]], &r));
    }

    #[test]
    fn a_polygon_in_across_around_or_beside() {
        let r = rect(0.0, 0.0, 4.0, 4.0);
        let ring = |x0, y0, x1, y1| rect(x0, y0, x1, y1);
        let inside = ring(1.0, 1.0, 2.0, 2.0);
        let across = ring(3.0, 3.0, 8.0, 8.0);
        let around = ring(-5.0, -5.0, 9.0, 9.0);
        let beside = ring(5.0, 5.0, 8.0, 8.0);
        assert!(hits(Kind::Polygon, &[&inside], &r));
        assert!(hits(Kind::Polygon, &[&across], &r));
        assert!(hits(Kind::Polygon, &[&around], &r), "the polygon holds the shape, none of its vertices in it");
        assert!(!hits(Kind::Polygon, &[&beside], &r));
        // a hole around the shape: the shape is in the hole, the polygon is not met
        let hole = ring(-1.0, -1.0, 5.0, 5.0);
        assert!(!hits(Kind::Polygon, &[&around, &hole], &r));
        // a lasso inside a big polygon
        let t = vec![[1.0, 1.0], [2.0, 1.0], [1.0, 2.0]];
        assert!(hits(Kind::Polygon, &[&around], &t));
        // a shape that is no area meets nothing
        assert!(!hits(Kind::Polygon, &[&inside], &[[0.0, 0.0], [4.0, 4.0]]));
    }

    #[test]
    fn the_shape_dragged() {
        let mut d = Drag::new(false, [1.0, 1.0]);
        assert!(d.polygon().is_none());
        d.push([3.0, 4.0], 0.5);
        d.push([5.0, 2.0], 0.5);
        assert_eq!(d.points.len(), 2, "a rectangle has two corners");
        assert_eq!(d.polygon().unwrap(), vec![[1.0, 1.0], [5.0, 1.0], [5.0, 2.0], [1.0, 2.0]]);
        // a lasso keeps the path (not points closer than a step)
        let mut l = Drag::new(true, [0.0, 0.0]);
        l.push([0.1, 0.0], 0.5);
        l.push([1.0, 0.0], 0.5);
        l.push([1.0, 1.0], 0.5);
        assert_eq!(l.points.len(), 3);
        assert_eq!(l.polygon().unwrap().len(), 3);
        l.points.pop();
        assert!(l.polygon().is_none());
        // a rectangle of no height is no shape
        let mut flat = Drag::new(false, [0.0, 2.0]);
        flat.push([4.0, 2.0], 0.5);
        assert!(flat.polygon().is_none());
    }
}
