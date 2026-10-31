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

/// The features drawn so far, and the one being drawn: their vertices in cells of the map's grid (x, y).
#[derive(Clone, Debug)]
pub struct Sketch {
    pub kind: Kind,
    pub features: Vec<Vec<[f64; 2]>>,
    pub current: Vec<[f64; 2]>,
}

impl Sketch {
    pub fn new(kind: Kind) -> Sketch {
        Sketch { kind, features: Vec::new(), current: Vec::new() }
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
        if self.current.pop().is_none() {
            self.features.pop();
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
        Vector { geometries, attributes: vec![("id".into(), Column::Numbers(ids))], crs }
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
    if response.clicked()
        && let Some(p) = response.interact_pointer_pos()
    {
        let close = sketch.current.first().is_some_and(|f| screen(f).distance(p) < 8.0);
        sketch.click(cell(p), close);
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
    if hover.is_some() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
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
