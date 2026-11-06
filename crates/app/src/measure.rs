//! Measuring on a map: a line clicked point by point, its length (each segment and the total) and, from three
//! points, the area and perimeter of the polygon it closes. The points are cells of the map's grid, so they
//! stay in place when the map is moved or zoomed.
//!
//! The units follow the grid: metres and square metres on a projected grid (its linear unit taken into account);
//! on a geographic grid (degrees) the distances are geodesic on the WGS84 ellipsoid (Vincenty's inverse
//! formula) and the area is the one of the polygon with great-circle edges on the sphere of the same area as
//! the ellipsoid (the authalic sphere), within a fraction of a percent for polygons up to a few hundred
//! kilometres; with no coordinate system, the grid's own units and the cells.

use egui::{Color32, Pos2, Response, Stroke, Ui, Vec2};
use terraflux_raster::Grid;

/// WGS84: the equatorial radius (m) and the flattening.
const A: f64 = 6_378_137.0;
const F: f64 = 1.0 / 298.257_223_563;

/// A number of the grid's own units: to the thousandth, without the zeros at its end.
fn plain(v: f64) -> String {
    let t = format!("{v:.3}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The units a grid is measured in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Frame {
    /// A projected grid: metres per unit of its coordinates.
    Projected(f64),
    /// Longitudes and latitudes, in degrees.
    Geographic,
    /// No coordinate system known: the grid's units.
    Plain,
}

impl Frame {
    pub fn of(g: &Grid) -> Frame {
        match g.geo.is_geographic() {
            Some(true) => Frame::Geographic,
            Some(false) => Frame::Projected(g.geo.linear_unit()),
            None => Frame::Plain,
        }
    }
}

/// A grid's cell (column, row) as a coordinate of its map.
fn world(g: &Grid, c: [f64; 2]) -> [f64; 2] {
    let t = &g.transform;
    [t[0] + c[0] * t[1] + c[1] * t[2], t[3] + c[0] * t[4] + c[1] * t[5]]
}

/// The distance between two points of the map's coordinates, in metres (map units with no coordinate system).
fn distance(f: Frame, a: [f64; 2], b: [f64; 2]) -> f64 {
    match f {
        Frame::Projected(u) => (b[0] - a[0]).hypot(b[1] - a[1]) * u,
        Frame::Plain => (b[0] - a[0]).hypot(b[1] - a[1]),
        Frame::Geographic => geodesic(a, b),
    }
}

/// The geodesic distance in metres between two points (longitude, latitude in degrees) on the WGS84 ellipsoid:
/// Vincenty's inverse formula, a haversine on a sphere where it does not converge (nearly antipodal points).
pub fn geodesic(a: [f64; 2], b: [f64; 2]) -> f64 {
    let bb = A * (1.0 - F);
    let (p1, p2) = (a[1].to_radians(), b[1].to_radians());
    let mut l = (b[0] - a[0]).to_radians();
    l = (l + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI;
    let (u1, u2) = (((1.0 - F) * p1.tan()).atan(), ((1.0 - F) * p2.tan()).atan());
    let (s1, c1, s2, c2) = (u1.sin(), u1.cos(), u2.sin(), u2.cos());
    let mut lambda = l;
    for _ in 0..200 {
        let (sl, cl) = lambda.sin_cos();
        let ss = ((c2 * sl).powi(2) + (c1 * s2 - s1 * c2 * cl).powi(2)).sqrt();
        if ss == 0.0 {
            return 0.0;
        }
        let cs = s1 * s2 + c1 * c2 * cl;
        let sigma = ss.atan2(cs);
        let sa = c1 * c2 * sl / ss;
        let c2a = 1.0 - sa * sa;
        let c2sm = if c2a != 0.0 { cs - 2.0 * s1 * s2 / c2a } else { 0.0 };
        let c = F / 16.0 * c2a * (4.0 + F * (4.0 - 3.0 * c2a));
        let next = l + (1.0 - c) * F * sa * (sigma + c * ss * (c2sm + c * cs * (-1.0 + 2.0 * c2sm * c2sm)));
        if (next - lambda).abs() < 1e-12 {
            let u_sq = c2a * (A * A - bb * bb) / (bb * bb);
            let ka = 1.0 + u_sq / 16384.0 * (4096.0 + u_sq * (-768.0 + u_sq * (320.0 - 175.0 * u_sq)));
            let kb = u_sq / 1024.0 * (256.0 + u_sq * (-128.0 + u_sq * (74.0 - 47.0 * u_sq)));
            let ds = kb
                * ss
                * (c2sm
                    + kb / 4.0
                        * (cs * (-1.0 + 2.0 * c2sm * c2sm)
                            - kb / 6.0 * c2sm * (-3.0 + 4.0 * ss * ss) * (-3.0 + 4.0 * c2sm * c2sm)));
            return bb * ka * (sigma - ds);
        }
        lambda = next;
    }
    // (haversine)
    let h = ((p2 - p1) / 2.0).sin().powi(2) + p1.cos() * p2.cos() * (l / 2.0).sin().powi(2);
    6_371_008.8 * 2.0 * h.sqrt().asin()
}

/// The area in square metres of the polygon of these points (longitude, latitude in degrees) on the WGS84
/// ellipsoid, as told in the module's doc.
pub fn geodesic_area(pts: &[[f64; 2]]) -> f64 {
    let e2 = F * (2.0 - F);
    let e = e2.sqrt();
    // the authalic latitude: sin(beta) = q(phi) / q(pole)
    let q = |s: f64| (1.0 - e2) * (s / (1.0 - e2 * s * s) - ((1.0 - e * s) / (1.0 + e * s)).ln() / (2.0 * e));
    let qp = q(1.0);
    let radius2 = A * A * qp / 2.0;
    let beta = |lat: f64| (q(lat.to_radians().sin()) / qp).clamp(-1.0, 1.0).asin();
    let mut sum = 0.0;
    for i in 0..pts.len() {
        let (p, n) = (pts[i], pts[(i + 1) % pts.len()]);
        let mut dl = (n[0] - p[0]).to_radians();
        dl = (dl + std::f64::consts::PI).rem_euclid(2.0 * std::f64::consts::PI) - std::f64::consts::PI;
        let (t1, t2) = ((beta(p[1]) / 2.0).tan(), (beta(n[1]) / 2.0).tan());
        // the signed area between the edge and the pole
        sum += 2.0 * ((dl / 2.0).tan() * (t1 + t2)).atan2(1.0 + t1 * t2);
    }
    sum.abs() * radius2
}

/// The area of a polygon of planar points (shoelace).
fn shoelace(pts: &[[f64; 2]]) -> f64 {
    let mut s = 0.0;
    for i in 0..pts.len() {
        let (p, n) = (pts[i], pts[(i + 1) % pts.len()]);
        s += p[0] * n[1] - n[0] * p[1];
    }
    s.abs() / 2.0
}

/// The area in square metres of a cell of a grid in a row (the same along a row of an axis-aligned grid): its
/// width by its height on a projected grid, the cell's polygon on the ellipsoid on a geographic one; none with no
/// coordinate system.
pub fn cell_area(g: &Grid, row: usize) -> Option<f64> {
    match Frame::of(g) {
        Frame::Projected(u) => {
            let t = &g.transform;
            Some((t[1] * t[5] - t[2] * t[4]).abs() * u * u)
        }
        Frame::Geographic => {
            let r = row as f64;
            let corners = [[0.0, r], [1.0, r], [1.0, r + 1.0], [0.0, r + 1.0]].map(|c| world(g, c));
            Some(geodesic_area(&corners))
        }
        Frame::Plain => None,
    }
}

/// The distance in metres between two cells (column, row) of a grid along a straight line (the grid's units with
/// no coordinate system).
pub fn cells_apart(g: &Grid, a: [f64; 2], b: [f64; 2]) -> f64 {
    distance(Frame::of(g), world(g, a), world(g, b))
}

/// What a line measures.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    pub frame: Frame,
    /// Each segment's length, in metres (the grid's units with no coordinate system).
    pub segments: Vec<f64>,
    pub length: f64,
    /// The perimeter and area of the polygon closed (three points or more), in metres and square metres.
    pub perimeter: Option<f64>,
    pub area: Option<f64>,
    /// The line's length in cells and the polygon's area in cells.
    pub cells_length: f64,
    pub cells_area: Option<f64>,
}

/// A length in metres as text: metres below a kilometre.
pub fn length_text(m: f64) -> String {
    if m.abs() >= 1000.0 { format!("{:.3} km", m / 1000.0) } else { format!("{m:.2} m") }
}

/// An area in square metres as text: m², hectares from one, km² from a square kilometre.
pub fn area_text(m2: f64) -> String {
    if m2 >= 1e6 {
        format!("{:.3} km²", m2 / 1e6)
    } else if m2 >= 1e4 {
        format!("{:.2} ha", m2 / 1e4)
    } else {
        format!("{m2:.2} m²")
    }
}

impl Report {
    pub fn of(g: &Grid, cells: &[[f64; 2]]) -> Report {
        let frame = Frame::of(g);
        let pts: Vec<[f64; 2]> = cells.iter().map(|&c| world(g, c)).collect();
        let segments: Vec<f64> = pts.windows(2).map(|w| distance(frame, w[0], w[1])).collect();
        let length = segments.iter().sum();
        let cells_length = cells.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).sum();
        let closed = pts.len() >= 3;
        let perimeter = closed.then(|| length + distance(frame, pts[pts.len() - 1], pts[0]));
        let area = closed.then(|| match frame {
            Frame::Projected(u) => shoelace(&pts) * u * u,
            Frame::Plain => shoelace(&pts),
            Frame::Geographic => geodesic_area(&pts),
        });
        let cells_area = closed.then(|| shoelace(cells));
        Report { frame, segments, length, perimeter, area, cells_length, cells_area }
    }

    fn len(&self, v: f64) -> String {
        match self.frame {
            Frame::Plain => format!("{} units, {:.1} cells", plain(v), v / self.unit_cells()),
            _ => length_text(v),
        }
    }

    // (a length in map units over the same in cells: the line's own ratio, for a grid with no coordinate system)
    fn unit_cells(&self) -> f64 {
        if self.length > 0.0 && self.cells_length > 0.0 { self.length / self.cells_length } else { 1.0 }
    }

    fn sq(&self, v: f64) -> String {
        match self.frame {
            Frame::Plain => format!("{} units²", plain(v)),
            _ => area_text(v),
        }
    }

    /// The numbers as lines of text (what the box shows and Copy puts on the clipboard).
    pub fn text(&self) -> String {
        let mut t = String::new();
        if self.segments.is_empty() {
            return t;
        }
        t.push_str(&format!("Length: {}\n", self.len(self.length)));
        if self.frame != Frame::Plain {
            t.push_str(&format!("Length in cells: {:.1}\n", self.cells_length));
        }
        if self.segments.len() > 1 {
            for (k, s) in self.segments.iter().enumerate() {
                t.push_str(&format!("  segment {}: {}\n", k + 1, self.len(*s)));
            }
        }
        if let (Some(p), Some(a), Some(c)) = (self.perimeter, self.area, self.cells_area) {
            t.push_str(&format!("Perimeter: {}\n", self.len(p)));
            t.push_str(&format!("Area: {}\n", self.sq(a)));
            t.push_str(&format!("Area in cells: {c:.1}\n"));
        }
        t
    }
}

/// A line being measured.
#[derive(Clone, Debug, Default)]
pub struct Measure {
    /// The points, as cells (column, row) of the map's grid.
    pub points: Vec<[f64; 2]>,
    /// Ended by a double click: no more points until one is taken away.
    pub ended: bool,
    /// When it was last changed (a counter over every line: the greatest is the line last worked on, the one
    /// whose view shows the numbers among views side by side).
    pub touched: u64,
}

/// A number that grows with every change of any line.
fn stamp() -> u64 {
    static CLOCK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    CLOCK.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// What the keys and clicks over the map ask of the measuring.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wish {
    Keep,
    /// Esc: measuring ended and cleared.
    Stop,
}

const INK: Color32 = Color32::from_rgb(0, 220, 255);

/// The line over a map: its segments (closed by a faint one once ended, from three points) and its vertices, at
/// these screen points. Also drawn over the views beside the one measured on.
pub fn paint_line(painter: &egui::Painter, pts: &[Pos2], ended: bool) {
    let halo = Stroke::new(4.0, Color32::from_black_alpha(140));
    let line = Stroke::new(2.0, INK);
    if pts.len() >= 3 && ended {
        painter.line_segment([pts[pts.len() - 1], pts[0]], Stroke::new(1.5, INK.gamma_multiply(0.6)));
    }
    if pts.len() >= 2 {
        painter.add(egui::Shape::line(pts.to_vec(), halo));
        painter.add(egui::Shape::line(pts.to_vec(), line));
    }
    for p in pts {
        painter.circle(*p, 4.0, Color32::WHITE, Stroke::new(1.5, Color32::BLACK));
    }
}

/// Views side by side measure together: when one is measuring (and none was stopped by Esc or M, or the button) all
/// are, each with its own line in its own grid; when one was stopped all stop. The box of numbers is only in the
/// view worked on last (the first while there is no point yet); the others show the lines only.
pub fn together(views: &mut [&mut crate::map_view::MapView]) {
    let stopped = views.iter().any(|v| v.measure_stopped);
    for v in views.iter_mut() {
        v.measure_stopped = false;
    }
    if stopped {
        for v in views.iter_mut() {
            v.measuring = None;
            v.hide_measure_box = false;
        }
        return;
    }
    if views.iter().any(|v| v.measuring.is_some()) {
        for v in views.iter_mut() {
            if v.measuring.is_none() && v.drawing.is_none() {
                v.measuring = Some(Measure::default());
            }
        }
    }
    let last = views
        .iter()
        .enumerate()
        .filter_map(|(i, v)| v.measuring.as_ref().filter(|m| !m.points.is_empty()).map(|m| (m.touched, i)))
        .max()
        .map_or(0, |(_, i)| i);
    for (i, v) in views.iter_mut().enumerate() {
        v.hide_measure_box = i != last;
    }
}

impl Measure {
    pub fn click(&mut self, cell: [f64; 2]) {
        if !self.ended {
            self.points.push(cell);
            self.touched = stamp();
        }
    }

    /// Takes the last point away (and lets the line go on if it was ended).
    pub fn undo(&mut self) {
        self.points.pop();
        self.ended = false;
        self.touched = stamp();
    }

    /// Ends the line (a double click: its second click added a point on the first one's place, taken away).
    pub fn finish(&mut self) {
        if !self.ended {
            if self.points.len() >= 2 && self.points[self.points.len() - 1] == self.points[self.points.len() - 2] {
                self.points.pop();
            }
            self.ended = true;
            self.touched = stamp();
        }
    }

    pub fn report(&self, g: &Grid) -> Report {
        Report::of(g, &self.points)
    }

    /// The clicks and keys over the map while measuring (taking the place of the cell picked), the line over
    /// it and the box of numbers at its corner (only with `boxed`: of views side by side, the one being measured
    /// has it): `to_cell` and `to_screen` between the screen and the grid's cells.
    #[allow(clippy::too_many_arguments)]
    pub fn interact(
        &mut self,
        ui: &Ui,
        response: &Response,
        grid: &Grid,
        typing: bool,
        boxed: bool,
        to_cell: impl Fn(Pos2) -> Vec2,
        to_screen: impl Fn(Vec2) -> Pos2,
    ) -> Wish {
        let cell = |p: Pos2| {
            let c = to_cell(p);
            [f64::from(c.x), f64::from(c.y)]
        };
        let screen = |c: &[f64; 2]| to_screen(Vec2::new(c[0] as f32, c[1] as f32));
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.click(cell(p));
        }
        if response.double_clicked() {
            self.finish();
        }
        if response.secondary_clicked() {
            self.undo();
        }
        let mut wish = Wish::Keep;
        if response.hovered() && !typing {
            ui.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace) {
                    self.undo();
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                    wish = Wish::Stop;
                }
            });
        }
        // the line, its vertices, and (not ended) the next segment up to the pointer
        let painter = ui.painter_at(response.rect);
        let line = Stroke::new(2.0, INK);
        let pts: Vec<Pos2> = self.points.iter().map(screen).collect();
        paint_line(&painter, &pts, self.ended);
        if !self.ended
            && let (Some(&last), Some(h)) = (pts.last(), response.hover_pos())
        {
            painter.add(egui::Shape::dashed_line(&[last, h], line, 6.0, 4.0));
            let mut next = self.points.clone();
            next.push(cell(h));
            let r = Report::of(grid, &next);
            if let Some(s) = r.segments.last() {
                let text = r.len(*s);
                let at = h + Vec2::new(12.0, -14.0);
                let font = egui::FontId::proportional(12.0);
                let galley = painter.layout_no_wrap(text, font, Color32::WHITE);
                painter.rect_filled(
                    egui::Rect::from_min_size(at, galley.size()).expand(3.0),
                    3.0,
                    Color32::from_black_alpha(170),
                );
                painter.galley(at, galley, Color32::WHITE);
            }
        }
        // the numbers, in a box at the map's lower corner (it takes the clicks over it: no point under it)
        if !boxed {
            return wish;
        }
        let report = self.report(grid);
        let mut clear = false;
        egui::Area::new(response.id.with("measure box"))
            .order(egui::Order::Foreground)
            .pivot(egui::Align2::LEFT_BOTTOM)
            .fixed_pos(response.rect.left_bottom() + Vec2::new(8.0, -8.0))
            .constrain_to(response.rect)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.strong("Measure");
                    if self.points.is_empty() {
                        ui.weak("click the map to add points");
                    } else {
                        ui.label(report.text());
                        ui.horizontal(|ui| {
                            if ui.small_button("Copy").on_hover_text("the numbers, as text").clicked() {
                                ui.ctx().copy_text(report.text());
                            }
                            if ui.small_button("Clear").on_hover_text("the line away; measuring goes on").clicked() {
                                clear = true;
                            }
                        });
                    }
                    ui.weak("right click, backspace: undo   double click: end   esc: stop");
                });
            });
        if clear {
            self.points.clear();
            self.ended = false;
            self.touched = stamp();
        }
        wish
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projected(unit_m: f64) -> Grid {
        let mut g = Grid::new(100, 100);
        // 10 m cells, north-up
        g.transform = [500_000.0, 10.0, 0.0, 4_000_000.0, 0.0, -10.0];
        // ProjectedCSType (3072) = 32633, model type (1024) projected, linear unit (3076)
        let unit = if unit_m == 1.0 { 9001 } else { 9003 };
        g.geo.geokeys = Some(vec![1, 1, 0, 3, 1024, 0, 1, 1, 3072, 0, 1, 32633, 3076, 0, 1, unit]);
        g
    }

    #[test]
    fn the_area_of_a_cell() {
        // 10 m cells: 100 m²; in feet, a tenth of it squared
        assert_eq!(cell_area(&projected(1.0), 0), Some(100.0));
        let ft = cell_area(&projected(0.3048), 0).unwrap();
        assert!((ft / (100.0 * 0.3048 * 0.3048) - 1.0).abs() < 1e-5, "{ft}");
        // a cell of 0.01 degrees at the equator: about 1.1132 km by 1.1057 km; smaller towards the poles
        let g = geographic();
        let at_equator = cell_area(&g, 99).unwrap();
        assert!((at_equator - 1_113_195.0 * 1_105.74 / 1000.0).abs() / at_equator < 0.005, "{at_equator}");
        let mut north = geographic();
        north.transform[3] = 61.0;
        assert!((cell_area(&north, 99).unwrap() / at_equator - 60f64.to_radians().cos()).abs() < 0.01);
        assert_eq!(cell_area(&Grid::new(2, 2), 0), None);
    }

    fn geographic() -> Grid {
        let mut g = Grid::new(100, 100);
        g.transform = [0.0, 0.01, 0.0, 1.0, 0.0, -0.01];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        g
    }

    #[test]
    fn planar_length_and_area() {
        let g = projected(1.0);
        assert_eq!(Frame::of(&g), Frame::Projected(1.0));
        // a 30 x 40 cells (300 m x 400 m) triangle's two legs, then the closing hypotenuse
        let r = Report::of(&g, &[[0.0, 0.0], [30.0, 0.0], [30.0, 40.0]]);
        assert_eq!(r.segments, vec![300.0, 400.0]);
        assert_eq!(r.length, 700.0);
        assert_eq!(r.perimeter, Some(1200.0));
        assert_eq!(r.area, Some(60_000.0));
        assert_eq!(r.cells_area, Some(600.0));
        assert!((r.cells_length - 70.0).abs() < 1e-9);
        // a line has no area
        let r = Report::of(&g, &[[0.0, 0.0], [10.0, 0.0]]);
        assert!(r.area.is_none() && r.perimeter.is_none());
        assert_eq!(r.text().lines().next(), Some("Length: 100.00 m"));
        // the units
        assert_eq!(length_text(1500.0), "1.500 km");
        assert_eq!(area_text(50_000.0), "5.00 ha");
        assert_eq!(area_text(2_500_000.0), "2.500 km²");
        assert_eq!(area_text(12.0), "12.00 m²");
    }

    #[test]
    fn the_linear_unit_counts() {
        // US survey feet: 1200/3937 m
        let g = projected(0.0);
        let r = Report::of(&g, &[[0.0, 0.0], [10.0, 0.0]]);
        assert!((r.length - 100.0 * 1200.0 / 3937.0).abs() < 1e-9);
    }

    #[test]
    fn geodesic_distances() {
        // a degree of longitude at the equator: a * pi / 180
        let d = geodesic([0.0, 0.0], [1.0, 0.0]);
        assert!((d - 111_319.49).abs() < 0.01, "{d}");
        // a degree of latitude at the equator
        let d = geodesic([0.0, 0.0], [0.0, 1.0]);
        assert!((d - 110_574.39).abs() < 1.0, "{d}");
        // Berlin to Paris: 877 km on a sphere, about 879.7 km on the ellipsoid
        let d = geodesic([13.405, 52.52], [2.3522, 48.8566]);
        assert!((d / 1000.0 - 879.7).abs() < 1.0, "{d}");
        // across the antimeridian
        let d = geodesic([179.5, 0.0], [-179.5, 0.0]);
        assert!((d - 111_319.49).abs() < 0.01, "{d}");
        assert_eq!(geodesic([5.0, 5.0], [5.0, 5.0]), 0.0);
        // Vincenty's own test case (Flinders Peak to Buninyong): 54972.271 m
        let d = geodesic([144.424_867_9, -37.951_033_4], [143.926_495_5, -37.652_821_1]);
        assert!((d - 54_972.271).abs() < 0.01, "{d}");
    }

    #[test]
    fn geodesic_areas() {
        // one degree by one degree at the equator: about 12,308 km2
        let a = geodesic_area(&[[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]]);
        assert!((a / 1e6 - 12_308.0).abs() < 15.0, "{a}");
        // the other way round: the same
        let b = geodesic_area(&[[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]]);
        assert!((a - b).abs() < 1.0);
        // an octant of the earth (from the equator to the pole, over 90 degrees of longitude): an eighth of the
        // ellipsoid's 510,065,622 km2
        let octant = geodesic_area(&[[0.0, 0.0], [90.0, 0.0], [90.0, 90.0], [0.0, 90.0]]);
        assert!((octant / 1e6 - 63_758_203.0).abs() < 1000.0, "{octant}");
        // through a grid: cells of 0.01 degrees
        let g = geographic();
        let r = Report::of(&g, &[[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]]);
        assert_eq!(r.frame, Frame::Geographic);
        assert!((r.area.unwrap() / 1e6 - 12_308.0).abs() < 15.0);
        assert!((r.perimeter.unwrap() / 1000.0 - 443.0).abs() < 2.0, "{:?}", r.perimeter);
    }

    /// A GeoTIFF in WGS 84 written and read back: 128 x 108 cells of 0.0025 degrees, its corner at 122.30 E, 12.60 N
    /// (the island of Sibuyan, as the test data). GDAL reads it as EPSG:4326 (checked with gdalinfo).
    fn degrees_file() -> Grid {
        let mut g = Grid::new(128, 108);
        g.transform = [122.30, 0.0025, 0.0, 12.60, 0.0, -0.0025];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        let data = (0..128 * 108).map(|i| f64::from(i % 5)).collect();
        let r = terraflux_raster::Raster::single(g, data, terraflux_raster::DataType::U8, None);
        let path = std::env::temp_dir().join(format!("terraflux-measure-{}.tif", std::process::id()));
        r.write(&path).unwrap();
        let back = terraflux_raster::Raster::read(&path).unwrap();
        std::fs::remove_file(&path).ok();
        back.grid
    }

    #[test]
    fn a_file_in_degrees_measures_as_the_references() {
        let g = degrees_file();
        assert_eq!(Frame::of(&g), Frame::Geographic);
        // The references (independent of this code): pyproj 3.7.2 `Geod(ellps="WGS84")`, that is GeographicLib's
        // exact geodesics, for the corners of cells A = (10, 20), B = (110, 90) and C = (10, 90), that is
        // (122.325 E, 12.55 N), (122.575 E, 12.375 N) and (122.325 E, 12.375 N):
        //   A-B 33368.4751 m, A-C 19359.5509 m, C-B 27187.4490 m;
        // the rectangle A, (110, 20), B, C: `polygon_area_perimeter` 526,161,498.514 m2 and 93,075.771 m, and the
        // same area from GDAL, `ogrinfo -dialect SQLite -sql "SELECT ST_Area(GeomFromText(wkt, 4326), 1)"`,
        // 526161498.514046 m2 (its geodesic edges on the ellipsoid).
        let r = Report::of(&g, &[[10.0, 20.0], [110.0, 90.0]]);
        let ab = 33_368.475_1;
        assert!((r.length - ab).abs() / ab < 0.001, "{}", r.length);
        // (Vincenty's formula is good to a millimetre: far tighter than the 0.1% asked)
        assert!((r.length - ab).abs() < 0.01, "{}", r.length);
        let r = Report::of(&g, &[[10.0, 20.0], [10.0, 90.0], [110.0, 90.0]]);
        assert!((r.segments[0] - 19_359.550_9).abs() < 0.01 && (r.segments[1] - 27_187.449_0).abs() < 0.01);
        // the rectangle: the polygon's edges are great circles on the authalic sphere, the reference's geodesics on
        // the ellipsoid; the area is within 0.5% as asked (the authalic sphere has the ellipsoid's area)
        let rect = [[10.0, 20.0], [110.0, 20.0], [110.0, 90.0], [10.0, 90.0]];
        let r = Report::of(&g, &rect);
        let (area, perimeter) = (526_161_498.514, 93_075.771);
        let a = r.area.unwrap();
        let off = (a - area) / area;
        assert!(off.abs() < 0.005, "{a} against {area}");
        // (in fact the difference is 8e-9: the authalic sphere keeps the ellipsoid's areas)
        assert!(off.abs() < 1e-6, "{a} against {area}: {:.5}%", off * 100.0);
        assert!((r.perimeter.unwrap() - perimeter).abs() / perimeter < 0.001, "{:?}", r.perimeter);
        assert!(r.text().contains("km²"), "{}", r.text());
    }

    #[test]
    fn no_crs_is_in_units_and_cells() {
        let mut g = Grid::new(100, 100);
        g.transform = [0.0, 2.0, 0.0, 0.0, 0.0, -2.0];
        assert_eq!(Frame::of(&g), Frame::Plain);
        let r = Report::of(&g, &[[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]]);
        assert_eq!(r.length, 40.0);
        assert_eq!(r.cells_length, 20.0);
        assert_eq!(r.area, Some(200.0));
        assert_eq!(r.cells_area, Some(50.0));
        let t = r.text();
        assert!(t.contains("40 units, 20.0 cells") && t.contains("200 units²"), "{t}");
    }

    #[test]
    fn clicks_undo_and_end() {
        let mut m = Measure::default();
        m.click([1.0, 1.0]);
        m.click([2.0, 2.0]);
        // a double click: its second click on the same place
        m.click([2.0, 2.0]);
        m.finish();
        assert!(m.ended && m.points.len() == 2);
        m.click([9.0, 9.0]);
        assert_eq!(m.points.len(), 2, "no point once ended");
        m.undo();
        assert!(!m.ended && m.points.len() == 1);
    }
}
