//! Vector features as the viewer shows them: polygons' outlines, lines and points fitted to the pane (scroll to
//! zoom, drag to move, double-click to fit again), the feature under the pointer named, and the attributes beside.

use egui::{Color32, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use terraflux_raster::vector::{Geometry, Vector, Xy};

use crate::theme;

/// The view: the zoom and the offset (in pane pixels) from the fitted view.
#[derive(Debug, Clone, Copy, PartialEq)]
struct View {
    zoom: f32,
    offset: Vec2,
}

impl Default for View {
    fn default() -> Self {
        View { zoom: 1.0, offset: Vec2::ZERO }
    }
}

/// The features drawn in the space left, their attributes in a panel at the right.
pub fn show(ui: &mut Ui, v: &Vector, id: egui::Id) {
    egui::Panel::right(id.with("attributes"))
        .resizable(true)
        .default_size(300.0)
        .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(egui::Margin::same(8)))
        .show(ui, |ui| {
            let kinds = |f: fn(&Geometry) -> bool| v.geometries.iter().filter(|g| f(g)).count();
            ui.label(format!(
                "{} features: {} polygons, {} lines, {} points",
                v.len(),
                kinds(|g| g.is_polygonal()),
                kinds(|g| matches!(g, Geometry::LineString(_) | Geometry::MultiLineString(_))),
                kinds(|g| g.is_point()),
            ));
            let texts: Vec<&str> = v
                .attributes
                .iter()
                .filter(|(_, c)| matches!(c, terraflux_raster::vector::Column::Texts(_)))
                .map(|(n, _)| n.as_str())
                .collect();
            if !texts.is_empty() {
                ui.label(egui::RichText::new(format!("texts: {}", texts.join(", "))).weak());
            }
            ui.separator();
            let t = terraflux_ops::vector::attribute_table(v);
            crate::table_view::show(ui, &t, id.with("table"));
        });
    let (rect, resp) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
    let Some(b) = v.bounds() else {
        painter.text(rect.center(), egui::Align2::CENTER_CENTER, "no geometry", egui::FontId::default(), Color32::GRAY);
        return;
    };
    let mut view: View = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    if resp.dragged() {
        view.offset += resp.drag_delta();
    }
    if resp.double_clicked() {
        view = View::default();
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let f = (scroll / 200.0).exp();
            // zoom about the pointer
            if let Some(p) = resp.hover_pos() {
                let c = rect.center() + view.offset;
                view.offset += (p - c) * (1.0 - f);
            }
            view.zoom *= f;
        }
    }
    ui.data_mut(|d| d.insert_temp(id, view));
    // the fitted scale (the same in x and y), the y axis up
    let (w, h) = ((b[2] - b[0]).max(1e-12), (b[3] - b[1]).max(1e-12));
    let fit = ((rect.width() as f64 - 24.0) / w).min((rect.height() as f64 - 24.0) / h).max(1e-12);
    let scale = fit * f64::from(view.zoom);
    let centre = rect.center() + view.offset;
    let (cx, cy) = ((b[0] + b[2]) / 2.0, (b[1] + b[3]) / 2.0);
    let at = |p: &Xy| Pos2::new(centre.x + ((p[0] - cx) * scale) as f32, centre.y - ((p[1] - cy) * scale) as f32);
    let stroke = Stroke::new(1.2, theme::ACCENT);
    let fill = theme::ACCENT.gamma_multiply(0.18);
    let mut hovered = None;
    let pointer = resp.hover_pos();
    for (k, g) in v.geometries.iter().enumerate() {
        let near = |pts: &[Pos2]| pointer.is_some_and(|p| Rect::from_points(pts).expand(3.0).contains(p));
        match g {
            Geometry::Empty => {}
            Geometry::Point(p) => {
                let q = at(p);
                painter.circle_filled(q, 3.0, theme::ACCENT);
                if near(&[q]) {
                    hovered = Some(k);
                }
            }
            Geometry::MultiPoint(ps) => {
                for p in ps {
                    let q = at(p);
                    painter.circle_filled(q, 3.0, theme::ACCENT);
                    if near(&[q]) {
                        hovered = Some(k);
                    }
                }
            }
            Geometry::LineString(l) => {
                let pts: Vec<Pos2> = l.iter().map(at).collect();
                painter.add(Shape::line(pts, stroke));
            }
            Geometry::MultiLineString(ls) => {
                for l in ls {
                    painter.add(Shape::line(l.iter().map(at).collect(), stroke));
                }
            }
            Geometry::Polygon(_) | Geometry::MultiPolygon(_) => {
                for poly in g.polygons() {
                    for (r, ring) in poly.iter().enumerate() {
                        let pts: Vec<Pos2> = ring.iter().map(at).collect();
                        if r == 0 && pts.len() > 2 {
                            // the outer ring lightly filled where egui can (convex), else its outline only
                            if pointer.is_some_and(|p| inside(&pts, p)) {
                                hovered = Some(k);
                            }
                            if convex(&pts) {
                                painter.add(Shape::convex_polygon(pts.clone(), fill, Stroke::NONE));
                            }
                        }
                        painter.add(Shape::closed_line(pts, stroke));
                    }
                }
            }
        }
    }
    // the coordinates under the pointer, in the view's corner
    if let Some(p) = pointer {
        let (x, y) = (cx + f64::from(p.x - centre.x) / scale, cy - f64::from(p.y - centre.y) / scale);
        let digits = if w.max(h) < 360.0 && b[0] >= -180.0 && b[2] <= 360.0 { 6 } else { 2 };
        let corner = rect.left_bottom() + egui::Vec2::new(6.0, -4.0);
        let galley = painter.layout_no_wrap(
            format!("x {x:.digits$}   y {y:.digits$}"),
            egui::FontId::monospace(11.0),
            Color32::WHITE,
        );
        let top = corner - egui::Vec2::new(0.0, galley.size().y);
        painter.rect_filled(
            Rect::from_min_size(top, galley.size()).expand2(egui::Vec2::new(4.0, 2.0)),
            2.0,
            Color32::from_black_alpha(150),
        );
        painter.galley(top, galley, Color32::WHITE);
    }
    if let Some(k) = hovered {
        let mut text = format!("feature {}", k + 1);
        for (n, c) in &v.attributes {
            let value = match c {
                terraflux_raster::vector::Column::Numbers(x) => crate::table_view::cell(x[k]),
                terraflux_raster::vector::Column::Texts(t) => t[k].clone(),
            };
            text.push_str(&format!("\n{n}: {value}"));
        }
        resp.on_hover_text_at_pointer(text);
    }
}

fn inside(pts: &[Pos2], p: Pos2) -> bool {
    let mut c = false;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            c = !c;
        }
    }
    c
}

fn convex(pts: &[Pos2]) -> bool {
    let n = pts.len();
    let mut sign = 0.0f32;
    for i in 0..n {
        let (a, b, c) = (pts[i], pts[(i + 1) % n], pts[(i + 2) % n]);
        let z = (b - a).x * (c - b).y - (b - a).y * (c - b).x;
        if z != 0.0 {
            if sign != 0.0 && z.signum() != sign {
                return false;
            }
            sign = z.signum();
        }
    }
    true
}
