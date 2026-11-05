//! Finding one's way on the live map: the scale bar (a round length in metres or kilometres, in cells where the map
//! has no coordinate system), the coordinate grid (lines at round values of the map's own coordinates, their values
//! at its edges) and the history of the view (back and forward in the zoom, kept as a stack of the views that
//! settled). The arithmetic is here, as plain functions, and the map view draws it.

use std::sync::OnceLock;
use std::time::Instant;

use egui::Vec2;
use terraflux_raster::Grid;

use crate::figure::{coordinate, nice_step};
use crate::measure::{Frame, cells_apart};

/// The longest scale bar, in screen points (the nice length is the longest not above it: 60 to 150 points).
pub const BAR_MAX: f32 = 150.0;

/// The longest round length (1, 2 or 5 times a power of ten) not above `max`.
pub fn nice_length(max: f64) -> f64 {
    if !(max.is_finite() && max > 0.0) {
        return 1.0;
    }
    let p = 10f64.powf(max.log10().floor());
    let f = max / p;
    // (a float's rounding may leave f a hair under 10)
    p * if f >= 5.0 {
        5.0
    } else if f >= 2.0 {
        2.0
    } else {
        1.0
    }
}

/// A length in metres as text: kilometres from one, metres, centimetres below a metre (round values, no zeros at
/// the end).
pub fn length_label(m: f64) -> String {
    let plain = |v: f64| {
        let t = format!("{v:.6}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    };
    if m >= 1000.0 {
        format!("{} km", plain(m / 1000.0))
    } else if m >= 1.0 {
        format!("{} m", plain(m))
    } else {
        format!("{} cm", plain(m * 100.0))
    }
}

/// A scale bar: its length on the screen, its text, and what it stands for.
#[derive(Debug, Clone, PartialEq)]
pub struct Bar {
    pub px: f32,
    pub label: String,
    /// The length in metres (none with no coordinate system: the bar is in cells).
    pub metres: Option<f64>,
}

/// Metres on the ground per screen point at the view's centre: a cell's width in metres (the geodesic one along
/// the row at the centre's latitude on a geographic grid, as measuring does) over the points a cell takes. None
/// with no coordinate system.
pub fn metres_per_point(g: &Grid, center: Vec2, zoom: f32) -> Option<f64> {
    if zoom <= 0.0 || !zoom.is_finite() {
        return None;
    }
    let frame = Frame::of(g);
    let t = &g.transform;
    let cell = match frame {
        Frame::Plain => return None,
        Frame::Projected(u) => t[1].hypot(t[4]) * u,
        Frame::Geographic => {
            // (a centre off the map: the nearest row, and no farther than the poles)
            let y = f64::from(center.y).clamp(0.0, g.height as f64);
            let x = f64::from(center.x);
            let lat = t[3] + x * t[4] + y * t[5];
            let row = if lat.abs() > 89.0 {
                // (near a pole a row's width is nothing: the row at 89 degrees)
                let y89 = (lat.signum() * 89.0 - t[3] - x * t[4]) / t[5];
                y89.clamp(0.0, g.height as f64)
            } else {
                y
            };
            cells_apart(g, [x - 0.5, row], [x + 0.5, row])
        }
    };
    (cell.is_finite() && cell > 0.0).then(|| cell / f64::from(zoom))
}

/// The scale bar of a view: a round length of 60 to 150 points. In metres or kilometres on a grid with a
/// coordinate system, in cells ("10 cells") without one; none when the zoom is out of reach of any.
pub fn scale_bar(g: &Grid, center: Vec2, zoom: f32) -> Option<Bar> {
    if zoom <= 0.0 || !zoom.is_finite() {
        return None;
    }
    match metres_per_point(g, center, zoom) {
        Some(per_pt) => {
            let m = nice_length(per_pt * f64::from(BAR_MAX));
            Some(Bar { px: (m / per_pt) as f32, label: length_label(m), metres: Some(m) })
        }
        None if Frame::of(g) == Frame::Plain => {
            let cells = nice_length(f64::from(BAR_MAX / zoom));
            let label = if cells == 1.0 { "1 cell".to_string() } else { format!("{} cells", plain(cells)) };
            Some(Bar { px: (cells * f64::from(zoom)) as f32, label, metres: None })
        }
        None => None,
    }
}

/// A number without the zeros at its end.
fn plain(v: f64) -> String {
    let t = format!("{v:.6}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// The lines of a coordinate grid over a view: for each, where it is (screen points from the view's middle, along
/// the axis it crosses) and its value as a label.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Graticule {
    /// Vertical lines (constant x): the offset along x, and the label.
    pub xs: Vec<(f32, String)>,
    /// Horizontal lines (constant y).
    pub ys: Vec<(f32, String)>,
}

/// The grid's lines for a view of `size` points at `zoom` points per cell with `center` (a cell) at its middle, at
/// round values of the map's own coordinates (degrees, or its linear unit). None for a rotated grid (its lines
/// would not be straight on the screen).
pub fn graticule(g: &Grid, center: Vec2, zoom: f32, size: Vec2) -> Option<Graticule> {
    let t = &g.transform;
    if zoom <= 0.0 || !zoom.is_finite() || t[2] != 0.0 || t[4] != 0.0 || t[1] == 0.0 || t[5] == 0.0 {
        return None;
    }
    let half = size / 2.0;
    let geographic = g.geo.is_geographic() == Some(true);
    let (cx, cy) = (f64::from(center.x), f64::from(center.y));
    let z = f64::from(zoom);
    // an axis: its lines at round values between the view's edges, about one in every 130 points
    let axis = |origin: f64, step_of: f64, c: f64, half: f32, extent: f32, east: bool| -> Vec<(f32, String)> {
        let reach = f64::from(half) / z;
        let (a, b) = (origin + (c - reach) * step_of, origin + (c + reach) * step_of);
        let (lo, hi) = (a.min(b), a.max(b));
        let step = nice_step(hi - lo, f64::from((extent / 130.0).max(1.0)));
        if !(step.is_finite() && step > 0.0) {
            return Vec::new();
        }
        let first = (lo / step).ceil();
        let n = ((hi - lo) / step).floor() + 1.0;
        if n > 200.0 {
            return Vec::new();
        }
        (0..=(n as usize))
            .map(|k| (first + k as f64) * step)
            .filter(|v| *v <= hi + step * 1e-9)
            .map(|v| {
                let cell = (v - origin) / step_of;
                (((cell - c) * z) as f32, coordinate(v, step, geographic, east))
            })
            .collect()
    };
    Some(Graticule { xs: axis(t[0], t[1], cx, half.x, size.x, true), ys: axis(t[3], t[5], cy, half.y, size.y, false) })
}

/// A view of the map: the zoom (screen points per cell) and the cell at the centre.
pub type Cam = (f32, Vec2);

/// Whether two views are the same one to within half a point on the screen.
pub fn same_cam(a: Cam, b: Cam) -> bool {
    let near = (a.1 - b.1).length() * a.0.max(b.0);
    near < 0.5 && (a.0 / b.0).ln().abs() < 1e-3
}

/// Seconds since the first call: the clock the history's debounce goes by.
pub fn now() -> f64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_secs_f64()
}

/// How long a view stays as it is before it counts as settled (a pan or zoom ended), seconds.
pub const SETTLE: f64 = 0.4;
/// The views kept.
pub const CAP: usize = 50;

/// The views the map has settled on, a stack with a place in it: back and forward move along it, a new view after
/// going back cuts off what was ahead (as a browser's history).
#[derive(Debug, Clone, Default)]
pub struct History {
    stack: Vec<Cam>,
    at: usize,
    /// The view at the last frame and when it came to be: settled once it has not changed for [`SETTLE`].
    seen: Option<(Cam, f64)>,
}

impl History {
    /// The views kept.
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }

    /// The place in the stack (0: the oldest).
    pub fn at(&self) -> usize {
        self.at
    }

    /// Forget everything (another map is shown).
    pub fn clear(&mut self) {
        *self = History::default();
    }

    /// A settled view: after the one now (what was ahead of it is cut off); the same view again is not kept twice.
    pub fn push(&mut self, cam: Cam) {
        if self.stack.get(self.at).is_some_and(|&c| same_cam(c, cam)) {
            return;
        }
        if !self.stack.is_empty() {
            self.stack.truncate(self.at + 1);
        }
        self.stack.push(cam);
        if self.stack.len() > CAP {
            self.stack.remove(0);
        }
        self.at = self.stack.len() - 1;
    }

    /// Whether going back leads anywhere, with the view `now` on the screen: a view that has not settled yet
    /// goes back to the last one that has.
    pub fn can_back(&self, now: Cam) -> bool {
        match self.stack.get(self.at) {
            Some(&c) => self.at > 0 || !same_cam(c, now),
            None => false,
        }
    }

    pub fn can_forward(&self) -> bool {
        self.at + 1 < self.stack.len()
    }

    /// One view back: the one before the settled one, or the settled one itself when the view moved since. Going
    /// back pushes nothing.
    pub fn back(&mut self, now: Cam) -> Option<Cam> {
        let &c = self.stack.get(self.at)?;
        if !same_cam(c, now) {
            self.seen = None;
            return Some(c);
        }
        if self.at == 0 {
            return None;
        }
        self.at -= 1;
        self.seen = None;
        Some(self.stack[self.at])
    }

    /// One view forward, if there is one.
    pub fn forward(&mut self) -> Option<Cam> {
        if !self.can_forward() {
            return None;
        }
        self.at += 1;
        self.seen = None;
        Some(self.stack[self.at])
    }

    /// The view at this frame, at time `now` (seconds): once it has not changed for [`SETTLE`] it is pushed. True
    /// while it is waiting to settle (the next frame must come by itself).
    pub fn watch(&mut self, cam: Cam, now: f64) -> bool {
        if !cam.0.is_finite() || !cam.1.x.is_finite() || !cam.1.y.is_finite() || cam.0 <= 0.0 {
            return false;
        }
        let since = match self.seen {
            Some((c, since)) if same_cam(c, cam) => since,
            _ => {
                self.seen = Some((cam, now));
                now
            }
        };
        if self.stack.get(self.at).is_some_and(|&c| same_cam(c, cam)) {
            return false;
        }
        if now - since >= SETTLE {
            self.push(cam);
            false
        } else {
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projected(cell: f64) -> Grid {
        let mut g = Grid::new(100, 100);
        g.transform = [500_000.0, cell, 0.0, 4_500_000.0, 0.0, -cell];
        // (model type projected, ProjectedCSType 32633, metres)
        g.geo.geokeys = Some(vec![1, 1, 0, 3, 1024, 0, 1, 1, 3072, 0, 1, 32633, 3076, 0, 1, 9001]);
        g
    }

    fn degrees(top: f64, cell: f64) -> Grid {
        let mut g = Grid::new(400, 400);
        g.transform = [10.0, cell, 0.0, top, 0.0, -cell];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        g
    }

    #[test]
    fn nice_lengths() {
        for (max, want) in [
            (1.0, 1.0),
            (1.9, 1.0),
            (2.0, 2.0),
            (4.99, 2.0),
            (5.0, 5.0),
            (9.99, 5.0),
            (10.0, 10.0),
            (149.0, 100.0),
            (150.0, 100.0),
            (0.7, 0.5),
            (0.0123, 0.01),
            (1234.0, 1000.0),
            (25_000.0, 20_000.0),
        ] {
            assert_eq!(nice_length(max), want, "{max}");
        }
        assert_eq!(nice_length(0.0), 1.0);
        assert_eq!(nice_length(f64::NAN), 1.0);
        // always within 60..=150 points of a bar at the longest 150
        for k in 0..400 {
            let max = 10f64.powf(-3.0 + k as f64 * 0.02);
            let n = nice_length(max);
            assert!(n <= max * (1.0 + 1e-12) && n > max * 0.39, "{max}: {n}");
        }
    }

    #[test]
    fn labels() {
        assert_eq!(length_label(500.0), "500 m");
        assert_eq!(length_label(1000.0), "1 km");
        assert_eq!(length_label(2500.0), "2.5 km");
        assert_eq!(length_label(20.0), "20 m");
        assert_eq!(length_label(0.5), "50 cm");
        assert_eq!(length_label(200_000.0), "200 km");
    }

    #[test]
    fn the_scale_bar_on_a_projected_grid() {
        // 30 m cells, 2 points a cell: 15 m a point, the bar at most 150 points: 2250 m, so 2 km
        let g = projected(30.0);
        let bar = scale_bar(&g, Vec2::new(50.0, 50.0), 2.0).unwrap();
        assert_eq!(bar.metres, Some(2000.0));
        assert_eq!(bar.label, "2 km");
        assert!((bar.px - 2000.0 / 15.0).abs() < 1e-3, "{}", bar.px);
        // zoomed in 100 times: 0.15 m a point, 22.5 m at most: 20 m
        let bar = scale_bar(&g, Vec2::new(50.0, 50.0), 200.0).unwrap();
        assert_eq!(bar.label, "20 m");
        assert!((bar.px - 20.0 / 0.15).abs() < 1e-2, "{}", bar.px);
        // the bar's points stand for its metres whatever the zoom
        for z in [0.1f32, 0.37, 1.0, 5.0, 64.0] {
            let b = scale_bar(&g, Vec2::new(50.0, 50.0), z).unwrap();
            assert!((60.0 - 1.0..=150.0 + 1e-3).contains(&b.px), "zoom {z}: {} points", b.px);
            assert!((b.metres.unwrap() / (30.0 / f64::from(z)) - f64::from(b.px)).abs() < 0.05, "zoom {z}");
        }
    }

    #[test]
    fn the_scale_bar_on_a_geographic_grid_follows_the_latitude() {
        // 0.01 degree cells, 10 points a cell
        let at = |top: f64| {
            let g = degrees(top, 0.01);
            // the middle of the grid: 200 rows down
            let c = Vec2::new(200.0, 200.0);
            (metres_per_point(&g, c, 10.0).unwrap(), scale_bar(&g, c, 10.0).unwrap())
        };
        // centre latitude 0.0 +- 2: top 2.0 -> row 200 is the equator; a degree of longitude is 111.32 km there
        let (eq, bar) = at(2.0);
        let want = 111_319.49 * 0.01 / 10.0;
        assert!((eq - want).abs() / want < 1e-3, "{eq} against {want} m a point");
        assert_eq!(bar.label, "10 km");
        // at 60 N a degree of longitude is half as long
        let (north, bar) = at(62.0);
        assert!((north / eq - 0.5).abs() < 0.01, "{north} against {eq}");
        assert!(bar.metres.unwrap() <= 150.0 * north);
        // (the cell's width at the pole is nothing: not infinite or NaN, the scale of 89 degrees)
        let g = degrees(90.0, 0.01);
        let pole = metres_per_point(&g, Vec2::new(200.0, 0.0), 10.0).unwrap();
        assert!(pole.is_finite() && pole > 0.0 && pole < eq / 20.0, "{pole}");
    }

    #[test]
    fn without_a_crs_the_bar_is_in_cells() {
        let mut g = Grid::new(100, 100);
        g.transform = [0.0, 1.0, 0.0, 100.0, 0.0, -1.0];
        let bar = scale_bar(&g, Vec2::new(50.0, 50.0), 4.0).unwrap();
        assert_eq!(bar.metres, None);
        // 150 / 4 = 37.5 cells: 20 cells, 80 points
        assert_eq!(bar.label, "20 cells");
        assert!((bar.px - 80.0).abs() < 1e-3);
        let bar = scale_bar(&g, Vec2::new(50.0, 50.0), 100.0).unwrap();
        assert_eq!(bar.label, "1 cell");
        let bar = scale_bar(&g, Vec2::new(50.0, 50.0), 0.01).unwrap();
        assert_eq!(bar.label, "10000 cells");
    }

    #[test]
    fn the_coordinate_grid() {
        // a view of 400 x 300 points, 2 points a cell, 30 m cells from (500000, 4500000): the view's middle is cell
        // (50, 50): 1500 m from the west edge, 2 x 100 cells wide: x from 500 000 + 30 * (50 - 100) = 498 500 ...
        let g = projected(30.0);
        let gr = graticule(&g, Vec2::new(50.0, 50.0), 2.0, Vec2::new(400.0, 300.0)).unwrap();
        assert!(!gr.xs.is_empty() && !gr.ys.is_empty());
        // the lines are 1000 m or 2000 m apart: 100 or 200 points / 15 m... the span of 400 points is 6000 m: 2000 m
        let step_px = gr.xs[1].0 - gr.xs[0].0;
        assert!((step_px - 2000.0 / 15.0).abs() < 1e-2, "{step_px}");
        // each label is its line's value: a line at offset d is at cell 50 + d / 2, x = 500000 + 30 cell
        for (d, label) in &gr.xs {
            let x = 500_000.0 + 30.0 * (50.0 + f64::from(*d) / 2.0);
            let digits: String = label.chars().filter(|c| c.is_ascii_digit()).collect();
            assert_eq!(digits, format!("{x:.0}"), "{label}");
            assert!(d.abs() <= 200.0 + 1e-3);
        }
        // y lines run north up: a larger y is higher on the screen (a smaller offset)
        assert!(gr.ys[0].0 > gr.ys[1].0);
        // a rotated grid has none
        let mut r = g.clone();
        r.transform[2] = 5.0;
        assert!(graticule(&r, Vec2::new(50.0, 50.0), 2.0, Vec2::new(400.0, 300.0)).is_none());
        // degrees: labels with their hemispheres
        let d = degrees(2.0, 0.01);
        let gr = graticule(&d, Vec2::new(200.0, 200.0), 100.0, Vec2::new(400.0, 300.0)).unwrap();
        assert!(gr.xs.iter().all(|(_, l)| l.ends_with('E')), "{:?}", gr.xs);
        assert!(gr.ys.iter().any(|(_, l)| l.ends_with('N') || l.ends_with('S') || l.starts_with('0')));
    }

    fn cam(z: f32, x: f32) -> Cam {
        (z, Vec2::new(x, 10.0))
    }

    #[test]
    fn the_history_pushes_goes_back_and_forward() {
        let mut h = History::default();
        assert!(!h.can_back(cam(1.0, 0.0)) && !h.can_forward());
        for k in 0..4 {
            h.push(cam(1.0, k as f32 * 10.0));
        }
        assert_eq!((h.len(), h.at()), (4, 3));
        // the same view again is not kept twice
        h.push(cam(1.0, 30.0));
        assert_eq!(h.len(), 4);
        assert_eq!(h.back(cam(1.0, 30.0)), Some(cam(1.0, 20.0)));
        assert_eq!(h.back(cam(1.0, 20.0)), Some(cam(1.0, 10.0)));
        assert!(h.can_forward());
        assert_eq!(h.forward(), Some(cam(1.0, 20.0)));
        assert_eq!(h.forward(), Some(cam(1.0, 30.0)));
        assert_eq!(h.forward(), None);
        // back to the oldest, then no more
        for _ in 0..3 {
            assert!(h.back(h.stack[h.at]).is_some());
        }
        assert_eq!(h.back(cam(1.0, 0.0)), None);
        assert!(!h.can_back(cam(1.0, 0.0)));
        // (going back pushed nothing)
        assert_eq!(h.len(), 4);
    }

    #[test]
    fn a_view_that_moved_goes_back_to_the_settled_one() {
        let mut h = History::default();
        h.push(cam(1.0, 0.0));
        h.push(cam(1.0, 10.0));
        // moved since, not settled: back is the settled one, then the one before
        assert!(h.can_back(cam(1.0, 14.0)));
        assert_eq!(h.back(cam(1.0, 14.0)), Some(cam(1.0, 10.0)));
        assert_eq!(h.back(cam(1.0, 10.0)), Some(cam(1.0, 0.0)));
        assert_eq!(h.at(), 0);
        // a single view moved away from: back brings it
        let mut h = History::default();
        h.push(cam(1.0, 0.0));
        assert!(h.can_back(cam(1.0, 5.0)));
        assert_eq!(h.back(cam(1.0, 5.0)), Some(cam(1.0, 0.0)));
    }

    #[test]
    fn a_new_view_after_going_back_cuts_off_the_branch() {
        let mut h = History::default();
        for k in 0..5 {
            h.push(cam(1.0, k as f32 * 10.0));
        }
        h.back(cam(1.0, 40.0));
        h.back(cam(1.0, 30.0));
        assert_eq!(h.at(), 2);
        h.push(cam(2.0, 99.0));
        assert_eq!((h.len(), h.at()), (4, 3));
        assert!(!h.can_forward());
        assert_eq!(h.back(cam(2.0, 99.0)), Some(cam(1.0, 20.0)));
    }

    #[test]
    fn the_history_keeps_fifty() {
        let mut h = History::default();
        for k in 0..80 {
            h.push(cam(1.0, k as f32 * 10.0));
        }
        assert_eq!(h.len(), CAP);
        assert_eq!(h.at(), CAP - 1);
        // the oldest went: the first kept is the 30th
        let mut now = cam(1.0, 790.0);
        let mut last = now;
        while let Some(c) = h.back(now) {
            last = c;
            now = c;
        }
        assert_eq!(last, cam(1.0, 300.0));
    }

    #[test]
    fn a_view_settles_after_a_pause() {
        let mut h = History::default();
        // moving every frame: nothing pushed, but waiting
        for k in 0..10 {
            assert!(h.watch(cam(1.0, k as f32 * 3.0), 0.1 * f64::from(k)));
        }
        assert!(h.is_empty());
        // still at the last view: settled after SETTLE
        assert!(h.watch(cam(1.0, 27.0), 1.0 + 0.2));
        assert!(h.is_empty());
        assert!(!h.watch(cam(1.0, 27.0), 0.9 + SETTLE + 0.01));
        assert_eq!(h.len(), 1);
        // and not again
        assert!(!h.watch(cam(1.0, 27.0), 5.0));
        assert_eq!(h.len(), 1);
        // a move and a pause: a second one
        assert!(h.watch(cam(2.0, 27.0), 6.0));
        assert!(!h.watch(cam(2.0, 27.0), 6.0 + SETTLE));
        assert_eq!(h.len(), 2);
        // back: the view restored is the one on the stack, so it is not pushed again
        let c = h.back(cam(2.0, 27.0)).unwrap();
        assert!(!h.watch(c, 7.0));
        assert!(!h.watch(c, 8.0));
        assert_eq!((h.len(), h.at()), (2, 0));
    }
}
