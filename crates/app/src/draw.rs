//! Features drawn on a map: a study area, plots, a transect. Clicked on the map shown (points one a click; a
//! line or a polygon a vertex a click, ended by Enter, a double click or a click on its first vertex; a rectangle
//! by two corners), in the coordinates of the map's grid; written as a GeoPackage in its coordinate system and
//! read into the model by a `vector.read` node, so that the model keeps them (and QGIS opens them).

use std::path::Path;

use egui::{Color32, Pos2, Rect, Response, Stroke, Ui, Vec2};
use terraflux_raster::vector::{Column, Geometry, Vector};

/// What a click draws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Point,
    Line,
    Polygon,
    Rectangle,
}

impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Polygon, Kind::Rectangle, Kind::Point, Kind::Line];

    pub fn name(self) -> &'static str {
        match self {
            Kind::Point => "points",
            Kind::Line => "lines",
            Kind::Polygon => "polygons",
            Kind::Rectangle => "rectangles",
        }
    }

    /// How to draw it, said under the map's tools.
    pub fn how(self) -> &'static str {
        match self {
            Kind::Point => "a click: a point",
            Kind::Line => "a click a vertex; Enter or a double click ends the line",
            Kind::Polygon => "a click a vertex; Enter, a double click or a click on the first vertex closes it",
            Kind::Rectangle => "a click on a corner, then on the opposite one",
        }
    }
}

/// The features drawn so far, and the one being drawn: their vertices in cells of the map's grid (x, y); the
/// attributes typed (their names, and a row of values per feature); the vertex being dragged.
#[derive(Clone, Debug)]
pub struct Sketch {
    pub kind: Kind,
    pub features: Vec<Vec<[f64; 2]>>,
    pub current: Vec<[f64; 2]>,
    pub columns: Vec<String>,
    pub values: Vec<Vec<String>>,
    pub grabbed: Option<(usize, usize)>,
}

/// What is under the pointer of a feature drawn: a vertex, or the middle of an edge (after its vertex).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Vertex(usize, usize),
    Middle(usize, usize),
}

impl Sketch {
    pub fn new(kind: Kind) -> Sketch {
        Sketch {
            kind,
            features: Vec::new(),
            current: Vec::new(),
            columns: Vec::new(),
            values: Vec::new(),
            grabbed: None,
        }
    }

    /// A feature's vertices that can be moved (a ring's closing vertex is its first).
    fn movable(&self, f: usize) -> usize {
        let n = self.features[f].len();
        if matches!(self.kind, Kind::Polygon | Kind::Rectangle) { n - 1 } else { n }
    }

    /// A vertex moved to `to`; the vertex's index after (a rectangle stays one: the opposite corner kept).
    pub fn move_vertex(&mut self, f: usize, v: usize, to: [f64; 2]) -> usize {
        let closed = matches!(self.kind, Kind::Polygon | Kind::Rectangle);
        let ring = &mut self.features[f];
        if self.kind == Kind::Rectangle {
            let o = ring[(v + 2) % 4];
            let (x0, x1) = (o[0].min(to[0]), o[0].max(to[0]));
            let (y0, y1) = (o[1].min(to[1]), o[1].max(to[1]));
            *ring = vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]];
            return (0..4).find(|&k| ring[k] == to).unwrap_or(v);
        }
        ring[v] = to;
        if closed && v == 0 {
            let last = ring.len() - 1;
            ring[last] = to;
        }
        v
    }

    /// A vertex put in the middle of an edge (after vertex `after`), at `at`; its index.
    pub fn insert_vertex(&mut self, f: usize, after: usize, at: [f64; 2]) -> usize {
        self.features[f].insert(after + 1, at);
        after + 1
    }

    /// A vertex taken away; the feature too when it would be too small (a point, a rectangle's corner, a
    /// line's second last, a polygon's third last).
    pub fn remove_vertex(&mut self, f: usize, v: usize) {
        let few = match self.kind {
            Kind::Point | Kind::Rectangle => true,
            Kind::Line => self.features[f].len() <= 2,
            Kind::Polygon => self.movable(f) <= 3,
        };
        if few {
            self.remove_feature(f);
            return;
        }
        let ring = &mut self.features[f];
        ring.remove(v);
        if self.kind == Kind::Polygon && v == 0 {
            let first = ring[0];
            *ring.last_mut().unwrap() = first;
        }
    }

    /// A feature taken away, with its values.
    pub fn remove_feature(&mut self, f: usize) {
        self.features.remove(f);
        if f < self.values.len() {
            self.values.remove(f);
        }
        self.grabbed = None;
    }

    /// The vertex, or the middle of an edge, within `reach` of `p` on the screen (vertices first).
    pub fn hit(&self, p: Pos2, reach: f32, screen: &dyn Fn(&[f64; 2]) -> Pos2) -> Option<Hit> {
        let near = |c: &[f64; 2]| screen(c).distance(p) <= reach;
        for f in (0..self.features.len()).rev() {
            if let Some(v) = (0..self.movable(f)).find(|&v| near(&self.features[f][v])) {
                return Some(Hit::Vertex(f, v));
            }
        }
        if matches!(self.kind, Kind::Line | Kind::Polygon) {
            for f in (0..self.features.len()).rev() {
                let r = &self.features[f];
                if let Some(v) = (0..r.len() - 1).find(|&v| near(&mid(r[v], r[v + 1]))) {
                    return Some(Hit::Middle(f, v));
                }
            }
        }
        None
    }

    /// The values of the attributes typed for a feature (empty where none was typed).
    pub fn row(&self, f: usize) -> Vec<String> {
        let mut r = self.values.get(f).cloned().unwrap_or_default();
        r.resize(self.columns.len(), String::new());
        r
    }

    /// The attributes typed as the layer's columns: numbers when every value typed reads as one, else texts
    /// (named as typed, letters, digits and _; the empty names, `id` and those repeated skipped).
    fn typed(&self) -> Vec<(String, Column)> {
        let mut out: Vec<(String, Column)> = Vec::new();
        for (k, name) in self.columns.iter().enumerate() {
            let name: String =
                name.trim().chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
            if name.is_empty() || name == "id" || out.iter().any(|(n, _)| *n == name) {
                continue;
            }
            let texts: Vec<String> = (0..self.features.len()).map(|f| self.row(f)[k].trim().to_string()).collect();
            let numbers: Option<Vec<f64>> =
                texts.iter().map(|t| if t.is_empty() { Some(f64::NAN) } else { t.parse::<f64>().ok() }).collect();
            let any = texts.iter().any(|t| !t.is_empty());
            let column = match numbers {
                Some(n) if any => Column::Numbers(n),
                _ => Column::Texts(texts),
            };
            out.push((name, column));
        }
        out
    }

    /// A vertex clicked; `close` says whether it is on the first vertex of the feature (polygons close there).
    pub fn click(&mut self, at: [f64; 2], close: bool) {
        match self.kind {
            Kind::Point => self.features.push(vec![at]),
            Kind::Rectangle => {
                if let Some(&a) = self.current.first() {
                    let (x0, x1) = (a[0].min(at[0]), a[0].max(at[0]));
                    let (y0, y1) = (a[1].min(at[1]), a[1].max(at[1]));
                    if x1 > x0 && y1 > y0 {
                        self.features.push(vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]);
                    }
                    self.current.clear();
                } else {
                    self.current.push(at);
                }
            }
            Kind::Polygon if close && self.current.len() >= 3 => self.finish(),
            Kind::Line | Kind::Polygon => {
                // (the second click of a double click: the same vertex again)
                if self.current.last() != Some(&at) {
                    self.current.push(at);
                }
            }
        }
    }

    /// The feature being drawn ended (a line of two vertices or more, a polygon of three, closed); else dropped.
    pub fn finish(&mut self) {
        let mut v = std::mem::take(&mut self.current);
        v.dedup();
        match self.kind {
            Kind::Line if v.len() >= 2 => self.features.push(v),
            Kind::Polygon if v.len() >= 3 => {
                v.push(v[0]);
                self.features.push(v);
            }
            _ => {}
        }
    }

    /// The last vertex taken back (of the feature being drawn, else the last feature).
    pub fn undo(&mut self) {
        if self.current.pop().is_none() && !self.features.is_empty() {
            self.remove_feature(self.features.len() - 1);
        }
    }

    /// The features as a vector layer in the grid's coordinates (an `id` each, 1, 2...).
    pub fn vector(&self, grid: &terraflux_raster::Grid) -> Vector {
        let t = grid.transform;
        let xy = |c: &[f64; 2]| [t[0] + c[0] * t[1] + c[1] * t[2], t[3] + c[0] * t[4] + c[1] * t[5]];
        let geometries = self
            .features
            .iter()
            .map(|f| {
                let ring: Vec<[f64; 2]> = f.iter().map(xy).collect();
                match self.kind {
                    Kind::Point => Geometry::Point(ring[0]),
                    Kind::Line => Geometry::LineString(ring),
                    Kind::Polygon | Kind::Rectangle => Geometry::Polygon(vec![ring]),
                }
            })
            .collect();
        let crs = grid
            .geo
            .wkt
            .clone()
            .or_else(|| grid.geo.epsg().and_then(|c| terraflux_gdal::crs_wkt(&format!("EPSG:{c}")).ok()));
        let ids = (1..=self.features.len()).map(|i| i as f64).collect();
        let mut attributes = vec![("id".to_string(), Column::Numbers(ids))];
        attributes.extend(self.typed());
        Vector { geometries, attributes, crs }
    }
}

/// The features written as a GeoPackage (a layer named as the file).
pub fn write(sketch: &Sketch, grid: &terraflux_raster::Grid, path: &Path) -> Result<(), String> {
    if sketch.features.is_empty() {
        return Err("nothing drawn".into());
    }
    if let Some(d) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let layer = path.file_stem().map_or("drawn".into(), |s| s.to_string_lossy().into_owned());
    let _ = std::fs::remove_file(path);
    terraflux_gdal::write_vector(path, &layer, &sketch.vector(grid))
}

const INK: Color32 = Color32::from_rgb(255, 210, 0);

fn mid(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0]
}

/// How near a vertex the pointer grabs it, in points.
const REACH: f32 = 7.0;

/// A vertex of a feature drawn dragged (or a new one, from the middle of an edge): true while it is, the map
/// not moved then. Called before the map's own dragging.
pub fn drag(
    sketch: &mut Sketch,
    ui: &Ui,
    response: &Response,
    to_cell: impl Fn(Pos2) -> Vec2,
    to_screen: impl Fn(Vec2) -> Pos2,
) -> bool {
    let cell = |p: Pos2| {
        let c = to_cell(p);
        [f64::from(c.x), f64::from(c.y)]
    };
    let screen = |c: &[f64; 2]| to_screen(Vec2::new(c[0] as f32, c[1] as f32));
    if response.drag_started()
        && sketch.current.is_empty()
        && let Some(p) = ui.input(|i| i.pointer.press_origin())
    {
        sketch.grabbed = match sketch.hit(p, REACH, &screen) {
            Some(Hit::Vertex(f, v)) => Some((f, v)),
            Some(Hit::Middle(f, v)) => {
                let at = mid(sketch.features[f][v], sketch.features[f][v + 1]);
                Some((f, sketch.insert_vertex(f, v, at)))
            }
            None => None,
        };
    }
    let Some((f, v)) = sketch.grabbed else { return false };
    if f >= sketch.features.len() || v >= sketch.features[f].len() {
        sketch.grabbed = None;
        return false;
    }
    if let Some(p) = response.interact_pointer_pos() {
        let v = sketch.move_vertex(f, v, cell(p));
        sketch.grabbed = Some((f, v));
    }
    if response.drag_stopped() || !response.dragged() {
        sketch.grabbed = None;
    }
    ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    true
}

/// The clicks and keys over the map while drawing (taking the place of the cell picked), and the features drawn
/// over it: `to_cell` and `to_screen` between the screen and the map's cells.
pub fn interact(
    sketch: &mut Sketch,
    ui: &Ui,
    response: &Response,
    to_cell: impl Fn(Pos2) -> Vec2,
    to_screen: impl Fn(Vec2) -> Pos2,
) {
    let cell = |p: Pos2| {
        let c = to_cell(p);
        [f64::from(c.x), f64::from(c.y)]
    };
    let screen = |c: &[f64; 2]| to_screen(Vec2::new(c[0] as f32, c[1] as f32));
    // (a click on a vertex of a feature drawn: not a new vertex; a right click there takes it away)
    let on = |s: &Sketch, p: Pos2| if s.current.is_empty() { s.hit(p, REACH, &screen) } else { None };
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
        && on(sketch, p).is_none()
    {
        let close = sketch.current.first().is_some_and(|f| screen(f).distance(p) < 8.0);
        sketch.click(cell(p), close);
    }
    if response.secondary_clicked()
        && let Some(p) = response.interact_pointer_pos()
        && let Some(Hit::Vertex(f, v)) = on(sketch, p)
    {
        sketch.remove_vertex(f, v);
    }
    if response.double_clicked() {
        sketch.finish();
    }
    if response.hovered() || !sketch.current.is_empty() {
        ui.input_mut(|i| {
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Enter) {
                sketch.finish();
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Escape) {
                sketch.current.clear();
            }
            if i.consume_key(egui::Modifiers::NONE, egui::Key::Backspace) {
                sketch.undo();
            }
        });
    }
    // the features drawn, then the one being drawn up to the pointer
    let painter = ui.painter_at(response.rect);
    let halo = Stroke::new(4.0, Color32::from_black_alpha(140));
    let line = Stroke::new(2.0, INK);
    let fill = Color32::from_rgba_unmultiplied(255, 210, 0, 50);
    for f in &sketch.features {
        let pts: Vec<Pos2> = f.iter().map(screen).collect();
        match sketch.kind {
            Kind::Point => {
                painter.circle(pts[0], 5.0, INK, Stroke::new(1.5, Color32::BLACK));
            }
            Kind::Line => {
                painter.add(egui::Shape::line(pts.clone(), halo));
                painter.add(egui::Shape::line(pts, line));
            }
            Kind::Polygon | Kind::Rectangle => {
                // (not convex: its outline only, filled as a mesh would need a triangulation)
                let convex = matches!(sketch.kind, Kind::Rectangle);
                if convex {
                    painter.add(egui::Shape::convex_polygon(pts[..pts.len() - 1].to_vec(), fill, Stroke::NONE));
                }
                painter.add(egui::Shape::line(pts.clone(), halo));
                painter.add(egui::Shape::line(pts, line));
            }
        }
    }
    // the handles: a vertex to drag (a right click: away), the middle of an edge to drag a new one from
    if sketch.current.is_empty() && sketch.kind != Kind::Point {
        let lines = matches!(sketch.kind, Kind::Line | Kind::Polygon);
        for (f, r) in sketch.features.iter().enumerate() {
            for (v, c) in r[..sketch.movable(f)].iter().enumerate() {
                let big = sketch.grabbed == Some((f, v));
                let radius = if big { 5.0 } else { 3.5 };
                painter.circle(screen(c), radius, Color32::WHITE, Stroke::new(1.5, Color32::BLACK));
            }
            if lines {
                for w in r.windows(2) {
                    let dot = Color32::from_black_alpha(90);
                    painter.circle(screen(&mid(w[0], w[1])), 2.5, dot, Stroke::new(1.0, Color32::WHITE));
                }
            }
        }
    }
    let hover = response.hover_pos();
    if !sketch.current.is_empty() {
        let mut pts: Vec<Pos2> = sketch.current.iter().map(screen).collect();
        if sketch.kind == Kind::Rectangle {
            if let Some(h) = hover {
                let r = Rect::from_two_pos(pts[0], h);
                painter.rect_filled(r, 0.0, fill);
                painter.rect_stroke(r, 0.0, line, egui::StrokeKind::Middle);
            }
        } else {
            if let Some(h) = hover {
                pts.push(h);
            }
            let dashed = Stroke::new(2.0, Color32::WHITE);
            painter.add(egui::Shape::line(pts.clone(), halo));
            painter.add(egui::Shape::line(pts, dashed));
        }
        for v in &sketch.current {
            painter.circle(screen(v), 3.5, Color32::WHITE, Stroke::new(1.5, Color32::BLACK));
        }
        // the first vertex of a polygon ringed: a click there closes it
        if sketch.kind == Kind::Polygon && sketch.current.len() >= 3 {
            painter.circle_stroke(screen(&sketch.current[0]), 8.0, Stroke::new(1.5, INK));
        }
    }
    if let Some(h) = hover
        && sketch.grabbed.is_none()
    {
        let grab = on(sketch, h).is_some();
        ui.ctx().set_cursor_icon(if grab { egui::CursorIcon::Grab } else { egui::CursorIcon::Crosshair });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polygons_lines_points_and_rectangles() {
        let mut s = Sketch::new(Kind::Polygon);
        for v in [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [10.0, 10.0]] {
            s.click(v, false);
        }
        assert_eq!(s.current.len(), 3, "the second click of a double click is not a vertex");
        s.click([0.0, 0.0], true);
        assert_eq!(s.features, vec![vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 0.0]]]);
        // too few vertices: dropped
        s.click([1.0, 1.0], false);
        s.finish();
        assert_eq!(s.features.len(), 1);
        s.undo();
        assert!(s.features.is_empty());

        let mut s = Sketch::new(Kind::Rectangle);
        s.click([4.0, 6.0], false);
        s.click([2.0, 1.0], false);
        assert_eq!(s.features[0], vec![[2.0, 1.0], [4.0, 1.0], [4.0, 6.0], [2.0, 6.0], [2.0, 1.0]]);

        let mut s = Sketch::new(Kind::Line);
        s.click([0.0, 0.0], false);
        s.click([3.0, 4.0], false);
        s.finish();
        let mut p = Sketch::new(Kind::Point);
        p.click([1.5, 2.5], false);
        p.click([2.5, 2.5], false);
        assert_eq!((s.features.len(), p.features.len()), (1, 2));
    }

    #[test]
    fn vertices_moved_added_and_taken_away() {
        let mut s = Sketch::new(Kind::Polygon);
        for v in [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0]] {
            s.click(v, false);
        }
        s.finish();
        // the first vertex moved: the ring stays closed
        assert_eq!(s.move_vertex(0, 0, [-1.0, -1.0]), 0);
        assert_eq!(s.features[0], vec![[-1.0, -1.0], [10.0, 0.0], [10.0, 10.0], [-1.0, -1.0]]);
        // a vertex amid an edge, found on the screen (a point a cell here)
        let screen = |c: &[f64; 2]| Pos2::new(c[0] as f32, c[1] as f32);
        assert_eq!(s.hit(Pos2::new(10.0, 5.0), 2.0, &screen), Some(Hit::Middle(0, 1)));
        assert_eq!(s.hit(Pos2::new(10.5, 0.5), 2.0, &screen), Some(Hit::Vertex(0, 1)));
        assert_eq!(s.hit(Pos2::new(30.0, 30.0), 2.0, &screen), None);
        assert_eq!(s.insert_vertex(0, 1, [12.0, 5.0]), 2);
        assert_eq!(s.features[0].len(), 5);
        // taken away: the first (the ring closed on the next), then down to a triangle, then the polygon
        s.remove_vertex(0, 0);
        assert_eq!(s.features[0], vec![[10.0, 0.0], [12.0, 5.0], [10.0, 10.0], [10.0, 0.0]]);
        s.remove_vertex(0, 1);
        assert!(s.features.is_empty());

        // a rectangle's corner dragged past the opposite one: a rectangle still
        let mut r = Sketch::new(Kind::Rectangle);
        r.click([2.0, 2.0], false);
        r.click([4.0, 6.0], false);
        let v = r.move_vertex(0, 2, [0.0, 1.0]);
        assert_eq!(r.features[0], vec![[0.0, 1.0], [2.0, 1.0], [2.0, 2.0], [0.0, 2.0], [0.0, 1.0]]);
        assert_eq!(r.features[0][v], [0.0, 1.0]);
    }

    #[test]
    fn attributes_typed() {
        let mut s = Sketch::new(Kind::Point);
        for x in [1.0, 2.0, 3.0] {
            s.click([x, 1.0], false);
        }
        s.columns = vec!["name".into(), "area ha".into(), "".into(), "name".into()];
        s.values = vec![vec!["Mata".into(), "12.5".into()], vec!["".into(), "".into()], vec!["Rio".into(), "3".into()]];
        let v = s.vector(&terraflux_raster::Grid::new(10, 10));
        let names: Vec<&str> = v.attributes.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["id", "name", "area_ha"]);
        assert_eq!(v.column("name"), Some(&Column::Texts(vec!["Mata".into(), "".into(), "Rio".into()])));
        let Some(Column::Numbers(a)) = v.column("area_ha") else { panic!("{:?}", v.attributes) };
        assert!(a[0] == 12.5 && a[1].is_nan() && a[2] == 3.0);
        // a feature taken away takes its values
        s.remove_feature(0);
        assert_eq!(s.row(0), ["", "", "", ""]);
        assert_eq!(s.row(1)[0], "Rio");
    }

    #[test]
    fn written_in_the_maps_coordinates() {
        let mut g = terraflux_raster::Grid::new(100, 100);
        g.transform = [500000.0, 30.0, 0.0, 9000000.0, 0.0, -30.0];
        g.geo.wkt = Some(terraflux_gdal::crs_wkt("EPSG:32723").unwrap());
        let mut s = Sketch::new(Kind::Rectangle);
        s.click([10.0, 20.0], false);
        s.click([30.0, 40.0], false);
        let v = s.vector(&g);
        let Geometry::Polygon(rings) = &v.geometries[0] else { panic!() };
        assert_eq!(rings[0][0], [500300.0, 8999400.0]);
        assert_eq!(rings[0][2], [500900.0, 8998800.0]);
        // as a GeoPackage, read back by GDAL in its coordinate system
        let dir = std::env::temp_dir().join(format!("terraflux-draw-{}", std::process::id()));
        let path = dir.join("study_area.gpkg");
        write(&s, &g, &path).unwrap();
        let back = terraflux_gdal::read_vector(&path, None).unwrap();
        assert_eq!(back.len(), 1);
        assert!(back.crs.as_deref().is_some_and(|c| c.contains("UTM zone 23S")), "{:?}", back.crs);
        assert!(write(&Sketch::new(Kind::Point), &g, &path).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
