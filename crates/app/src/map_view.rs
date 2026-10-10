//! A map on the screen: zoom with the wheel around the pointer, pan by dragging, a double click fits it; the
//! cell under the pointer is read out (row, column, map coordinates, value).

use egui::{Color32, Pos2, Rect, Sense, TextureHandle, TextureOptions, Ui, Vec2};
use terraflux_ops::Map;

use crate::palette::{Coloring, image};

/// The largest side of a map's texture (larger maps are drawn from every n-th cell).
pub const MAX_TEXTURE: usize = 4096;

/// The cell under the pointer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hover {
    pub row: usize,
    pub col: usize,
    pub x: f64,
    pub y: f64,
    pub value: f64,
}

#[derive(Default)]
pub struct MapView {
    key: Option<(usize, usize)>,
    texture: Option<TextureHandle>,
    /// cells per pixel of the texture
    step: usize,
    /// the texture's size, pixels
    size: Vec2,
    /// screen points per texture pixel
    zoom: f32,
    /// the texture pixel at the centre of the view
    center: Vec2,
    fitted: bool,
}

impl MapView {
    /// Fit the map in the view at the next frame.
    pub fn fit(&mut self) {
        self.fitted = false;
    }

    /// The texture pixel under a screen point.
    fn to_texture(&self, rect: Rect, p: Pos2) -> Vec2 {
        self.center + (p - rect.center()) / self.zoom
    }

    pub fn show(&mut self, ui: &mut Ui, map: &Map, coloring: &Coloring, coloring_key: usize) -> Option<Hover> {
        let key = (std::sync::Arc::as_ptr(&map.data) as usize, coloring_key);
        if self.key != Some(key) {
            let (img, step) = image(&map.data, map.grid.width, map.grid.height, MAX_TEXTURE, coloring);
            let size = Vec2::new(img.size[0] as f32, img.size[1] as f32);
            if self.size != size || self.step != step {
                self.fitted = false;
            }
            match &mut self.texture {
                Some(t) => t.set(img, TextureOptions::NEAREST),
                None => self.texture = Some(ui.ctx().load_texture("map", img, TextureOptions::NEAREST)),
            }
            self.key = Some(key);
            self.step = step;
            self.size = size;
        }
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());
        if !self.fitted || response.double_clicked() {
            self.zoom = (rect.width() / self.size.x).min(rect.height() / self.size.y).max(1e-3);
            self.center = self.size / 2.0;
            self.fitted = true;
        }
        if response.dragged() {
            self.center -= response.drag_delta() / self.zoom;
        }
        if let Some(p) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            let pinch = ui.input(|i| i.zoom_delta());
            let factor = pinch * (scroll / 400.0).exp();
            if factor != 1.0 {
                // keep the pixel under the pointer in place
                let under = self.to_texture(rect, p);
                self.zoom = (self.zoom * factor).clamp(1e-3, 256.0);
                self.center = under - (p - rect.center()) / self.zoom;
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        if let Some(t) = &self.texture {
            let min = rect.center() - self.center * self.zoom;
            let img_rect = Rect::from_min_size(min, self.size * self.zoom);
            painter.image(t.id(), img_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)), Color32::WHITE);
        }
        let p = response.hover_pos()?;
        let t = self.to_texture(rect, p);
        if t.x < 0.0 || t.y < 0.0 || t.x >= self.size.x || t.y >= self.size.y {
            return None;
        }
        let col = ((t.x * self.step as f32) as usize).min(map.grid.width - 1);
        let row = ((t.y * self.step as f32) as usize).min(map.grid.height - 1);
        let g = &map.grid.transform;
        let (cx, cy) = (col as f64 + 0.5, row as f64 + 0.5);
        Some(Hover {
            row,
            col,
            x: g[0] + cx * g[1] + cy * g[2],
            y: g[3] + cx * g[4] + cy * g[5],
            value: map.data[row * map.grid.width + col],
        })
    }
}

/// The legend of a colouring: a swatch and the name of each class (with its cells), or the range.
pub fn legend(ui: &mut Ui, coloring: &Coloring, counts: &[(f64, usize)], names: &dyn Fn(f64) -> String) {
    match coloring {
        Coloring::Classes(c) => {
            for (v, col) in c {
                ui.horizontal(|ui| {
                    let (r, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
                    ui.painter().rect_filled(r, 2.0, *col);
                    let n = counts.iter().find(|(k, _)| k == v).map_or(0, |(_, n)| *n);
                    ui.label(format!("{}  ({n} cells)", names(*v)));
                });
            }
        }
        Coloring::Continuous { min, max } => {
            let (r, _) = ui.allocate_exact_size(Vec2::new(160.0, 12.0), Sense::hover());
            let n = 32;
            for i in 0..n {
                let x0 = r.left() + r.width() * i as f32 / n as f32;
                let x1 = r.left() + r.width() * (i + 1) as f32 / n as f32;
                ui.painter().rect_filled(
                    Rect::from_min_max(Pos2::new(x0, r.top()), Pos2::new(x1, r.bottom())),
                    0.0,
                    crate::palette::sequential(i as f32 / (n - 1) as f32),
                );
            }
            ui.label(format!("{min:.4} to {max:.4}"));
        }
    }
}

/// The cells of each class of a map.
pub fn class_counts(values: &[f64]) -> Vec<(f64, usize)> {
    let mut out: Vec<(f64, usize)> = Vec::new();
    for &v in values {
        if v.is_nan() {
            continue;
        }
        match out.iter_mut().find(|(k, _)| *k == v) {
            Some(e) => e.1 += 1,
            None => {
                if out.len() < 256 {
                    out.push((v, 1));
                }
            }
        }
    }
    out.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts() {
        assert_eq!(class_counts(&[2.0, 1.0, f64::NAN, 2.0]), vec![(1.0, 1), (2.0, 2)]);
    }
}
