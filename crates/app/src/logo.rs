//! Terraflux's logo: two 3×3 blocks of cells, a landscape and the same one changed, joined by a wire of the
//! node editor. The window's icon (the application's icon in `assets/logo`) and the mark painted in the toolbar.

use std::sync::{Arc, OnceLock};

use egui::epaint::CubicBezierShape;
use egui::text::LayoutJob;
use egui::{Color32, FontId, Painter, Rect, Stroke, TextFormat, vec2};

/// The green of the land not yet changed (the changed land takes the theme's accent).
pub const LAND: Color32 = Color32::from_rgb(76, 159, 106);

const ICON_PNG: &[u8] = include_bytes!("../../../assets/logo/png/terraflux-icon-256.png");

/// The windows' icon: the application's icon, 256 px.
pub fn window_icon() -> Arc<egui::IconData> {
    static ICON: OnceLock<Arc<egui::IconData>> = OnceLock::new();
    ICON.get_or_init(|| {
        let pixmap = tiny_skia::Pixmap::decode_png(ICON_PNG).expect("the application's icon is a PNG");
        let rgba = pixmap
            .pixels()
            .iter()
            .flat_map(|p| {
                let c = p.demultiply();
                [c.red(), c.green(), c.blue(), c.alpha()]
            })
            .collect();
        Arc::new(egui::IconData { rgba, width: pixmap.width(), height: pixmap.height() })
    })
    .clone()
}

/// Paint the mark in `rect` (centred in it, as large as its shorter side): the changed cells in `accent`, the
/// wire in `wire`. Under 32 px the blocks are solid, the cells too small to be told apart.
pub fn paint_mark(painter: &Painter, rect: Rect, accent: Color32, wire: Color32) {
    let side = rect.width().min(rect.height());
    let s = side / 64.0;
    let origin = rect.center() - vec2(32.0 * s, 32.0 * s);
    let at = |x: f32, y: f32| origin + vec2(x * s, y * s);
    let block = |x: f32, y: f32, w: f32, h: f32, r: f32, c: Color32| {
        painter.rect_filled(Rect::from_min_size(at(x, y), vec2(w * s, h * s)), r * s, c);
    };
    let curve = [at(28.0, 15.0), at(40.0, 15.0), at(24.0, 49.0), at(36.0, 49.0)];
    let wire_of =
        |w: f32| CubicBezierShape::from_points_stroke(curve, false, Color32::TRANSPARENT, Stroke::new(w * s, wire));
    if side < 32.0 {
        block(2.0, 2.0, 26.0, 26.0, 3.0, LAND);
        block(36.0, 36.0, 26.0, 26.0, 3.0, accent);
        block(36.0, 45.0, 9.0, 17.0, 0.0, LAND);
        block(36.0, 54.0, 18.0, 8.0, 0.0, LAND);
        painter.add(wire_of(6.5));
        return;
    }
    for y in [2.0, 11.0, 20.0] {
        for x in [2.0, 11.0, 20.0] {
            block(x, y, 8.0, 8.0, 1.5, LAND);
        }
    }
    for (x, y) in [(36.0, 45.0), (36.0, 54.0), (45.0, 54.0)] {
        block(x, y, 8.0, 8.0, 1.5, LAND);
    }
    for (x, y) in [(36.0, 36.0), (45.0, 36.0), (54.0, 36.0), (45.0, 45.0), (54.0, 45.0), (54.0, 54.0)] {
        block(x, y, 8.0, 8.0, 1.5, accent);
    }
    painter.add(wire_of(5.0));
    painter.circle_filled(at(28.0, 15.0), 4.5 * s, wire);
    painter.circle_filled(at(36.0, 49.0), 4.5 * s, wire);
}

/// The name as the logo writes it: "terra" in `text`, "flux" in `accent`.
pub fn wordmark(size: f32, text: Color32, accent: Color32) -> LayoutJob {
    let mut job = LayoutJob::default();
    job.append("terra", 0.0, TextFormat::simple(FontId::proportional(size), text));
    job.append("flux", 0.0, TextFormat::simple(FontId::proportional(size), accent));
    job
}
