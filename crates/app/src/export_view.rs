//! The Viewer's map written as a picture: what it shows (the map, the maps under and over it at their opacity, the
//! vectors and their labels), a cell to a pixel or magnified, as PNG or as a GeoTIFF of red, green, blue and alpha
//! that opens in QGIS over other data. No legend or decorations: the Figure has them.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use egui::RichText;

use crate::figure::Panel;

/// The longest side of a picture, pixels (a larger map is read every few cells).
pub const MAX_SIDE: usize = 8000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Png,
    GeoTiff,
}

/// The view being exported: the map as the Viewer draws it, the size chosen, and the file being written.
pub struct Export {
    pub panel: Panel,
    /// Pixels to a cell of the map.
    pub scale: usize,
    job: Option<(PathBuf, Receiver<Result<String, String>>)>,
    said: Option<String>,
}

impl Export {
    pub fn new(panel: Panel) -> Export {
        Export { panel, scale: 1, job: None, said: None }
    }
}

/// The size of the picture of a map of `w` x `h` cells at `scale`, and whether the map is read more coarsely to
/// keep it within [`MAX_SIDE`].
pub fn size_of((w, h): (usize, usize), scale: usize) -> ((usize, usize), bool) {
    let k = scale.max(1);
    let step = (w.max(h) * k).div_ceil(MAX_SIDE).max(1);
    ((w.div_ceil(step) * k, h.div_ceil(step) * k), step > 1)
}

/// The view as a picture: premultiplied RGBA, transparent where nothing is drawn.
pub fn picture(p: &Panel, scale: usize) -> Result<tiny_skia::Pixmap, String> {
    crate::figure::plain(p, scale, MAX_SIDE)?.clear_pixels()
}

/// The grid of the picture: the map's, its cells made smaller as the picture is larger.
fn grid_of(p: &Panel, w: usize, h: usize) -> terraflux_raster::Grid {
    let g = p.source.grid();
    let (fx, fy) = (g.width as f64 / w as f64, g.height as f64 / h as f64);
    let t = g.transform;
    terraflux_raster::Grid {
        width: w,
        height: h,
        transform: [t[0], t[1] * fx, t[2] * fy, t[3], t[4] * fx, t[5] * fy],
        geo: g.geo.clone(),
    }
}

/// The view written to `path`; what to tell of it.
pub fn save(p: &Panel, scale: usize, format: Format, path: &Path) -> Result<String, String> {
    let pm = picture(p, scale)?;
    let (w, h) = (pm.width() as usize, pm.height() as usize);
    match format {
        Format::Png => std::fs::write(path, pm.encode_png().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?,
        Format::GeoTiff => {
            let mut bytes = Vec::with_capacity(w * h * 4);
            for c in pm.pixels() {
                let c = c.demultiply();
                bytes.extend([c.red(), c.green(), c.blue(), c.alpha()]);
            }
            let grid = grid_of(p, w, h);
            terraflux_raster::tiff::write_rgba(&bytes, &grid, path, Some(6)).map_err(|e| e.to_string())?;
            // (a coordinate system that came from GDAL, not from GeoTIFF keys, is set by GDAL)
            if grid.geo.geokeys.is_none()
                && let Some(wkt) = &grid.geo.wkt
            {
                terraflux_gdal::set_crs(path, wkt)?;
            }
        }
    }
    let (gw, gh) = p.source.size();
    let mut said = format!("saved: {} ({w} x {h} px)", path.display());
    if (w, h) != (gw * scale.max(1), gh * scale.max(1)) {
        said.push_str(&format!(", the map read every few cells to stay within {MAX_SIDE} px"));
    }
    Ok(said)
}

/// The window, while a view is being exported.
pub fn window(ctx: &egui::Context, s: &mut crate::app::Session) {
    let Some(e) = &mut s.export else { return };
    if let Some((_, rx)) = &e.job
        && let Ok(r) = rx.try_recv()
    {
        e.said = Some(r.unwrap_or_else(|e| format!("not saved: {e}")));
        e.job = None;
    }
    let mut open = true;
    egui::Window::new("Export the view").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.label("The map as the Viewer shows it, with its layers, vectors and labels. No legend or decorations: the Figure has them.");
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.label("Size");
            for (k, name) in [(1, "1 pixel a cell"), (2, "x2"), (4, "x4")] {
                ui.radio_value(&mut e.scale, k, name);
            }
        });
        let ((w, h), coarse) = size_of(e.panel.source.size(), e.scale);
        let note = if coarse { format!("{w} x {h} px (the map is read every few cells)") } else { format!("{w} x {h} px") };
        ui.label(RichText::new(note).weak());
        ui.add_space(4.0);
        let busy = e.job.is_some();
        ui.horizontal(|ui| {
            for (label, format, ext, filter, tip) in [
                ("Save PNG...", Format::Png, "png", "PNG picture", "the picture, with its transparency"),
                (
                    "Save GeoTIFF...",
                    Format::GeoTiff,
                    "tif",
                    "GeoTIFF",
                    "the same picture as four bands of bytes (red, green, blue, alpha) placed on the map's grid, to open in QGIS",
                ),
            ] {
                if ui.add_enabled(!busy, egui::Button::new(label)).on_hover_text(tip).clicked()
                    && let Some(path) = rfd::FileDialog::new()
                        .set_title("Export the view")
                        .add_filter(filter, &[ext])
                        .set_file_name(format!("view.{ext}"))
                        .save_file()
                {
                    let (panel, scale) = (e.panel.clone(), e.scale);
                    let (tx, rx) = channel();
                    let (to, ctx) = (path.clone(), ui.ctx().clone());
                    std::thread::spawn(move || {
                        let _ = tx.send(save(&panel, scale, format, &to));
                        ctx.request_repaint();
                    });
                    e.job = Some((path, rx));
                    e.said = None;
                }
            }
        });
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("writing the picture...").weak());
            });
        } else if let Some(t) = &e.said {
            ui.label(RichText::new(t).size(11.0).weak());
        }
    });
    if !open {
        s.export = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::figure::Vectors;
    use crate::map_view::{RasterLayer, Source};
    use crate::palette::Coloring;
    use egui::Color32;
    use std::sync::Arc;
    use terraflux_ops::Map;

    fn classes(colours: &[(f64, Color32)]) -> Coloring {
        Coloring::Classes(colours.to_vec())
    }

    fn map(w: usize, h: usize, f: impl Fn(usize) -> f64) -> Map {
        let mut g = terraflux_raster::Grid::new(w, h);
        g.transform = [100.0, 10.0, 0.0, 500.0, 0.0, -10.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 1, 2048, 0, 1, 4326]);
        Map::new(Arc::new(g), (0..w * h).map(f).collect())
    }

    const RED: Color32 = Color32::from_rgb(200, 0, 0);
    const BLUE: Color32 = Color32::from_rgb(0, 0, 200);

    /// A 4 x 4 map of one class, a layer of another over its left half at half opacity, a polygon over its right.
    fn panel() -> Panel {
        let layer = map(4, 4, |i| if i % 4 < 2 { 1.0 } else { f64::NAN });
        let v = terraflux_raster::vector::Vector {
            geometries: vec![terraflux_raster::vector::Geometry::Polygon(vec![vec![
                [120.0, 480.0],
                [140.0, 480.0],
                [140.0, 460.0],
                [120.0, 460.0],
                [120.0, 480.0],
            ]])],
            attributes: Vec::new(),
            crs: None,
        };
        Panel {
            source: Source::Map(map(4, 4, |_| 1.0)),
            coloring: classes(&[(1.0, RED)]),
            names: Vec::new(),
            caption: String::new(),
            vectors: vec![Vectors {
                vector: Arc::new(v),
                colours: vec![Color32::from_rgb(0, 200, 0)],
                fill: 1.0,
                width: 0.0,
                labels: None,
                label_size: 6.0,
            }],
            rasters: vec![RasterLayer {
                source: Source::Map(layer),
                coloring: classes(&[(1.0, BLUE)]),
                opacity: 0.5,
                below: false,
                name: "layer".into(),
            }],
        }
    }

    fn at(pm: &tiny_skia::Pixmap, x: u32, y: u32) -> [u8; 4] {
        let c = pm.pixel(x, y).unwrap().demultiply();
        [c.red(), c.green(), c.blue(), c.alpha()]
    }

    #[test]
    fn the_view_composed() {
        let pm = picture(&panel(), 1).unwrap();
        assert_eq!((pm.width(), pm.height()), (4, 4));
        // the layer at half opacity over the map on the left, the map alone and the polygon's fill on the right
        let [r, g, b, a] = at(&pm, 0, 3);
        assert_eq!(a, 255);
        assert!((95..=105).contains(&r) && g == 0 && (95..=105).contains(&b), "{r} {g} {b}");
        assert_eq!(at(&pm, 3, 0), [200, 0, 0, 255]);
        // (the polygon covers the cells of columns 2 and 3 in the last two rows)
        assert_eq!(at(&pm, 2, 2), [0, 200, 0, 255]);
        assert_eq!(at(&pm, 3, 3), [0, 200, 0, 255]);
        // magnified: the same picture, a cell four pixels across
        let big = picture(&panel(), 4).unwrap();
        assert_eq!((big.width(), big.height()), (16, 16));
        assert_eq!(at(&big, 3, 15), at(&pm, 0, 3));
        assert_eq!(at(&big, 9, 9), [0, 200, 0, 255]);
    }

    #[test]
    fn no_value_is_transparent() {
        let mut p = panel();
        p.vectors.clear();
        p.rasters.clear();
        p.source = Source::Map(map(4, 4, |i| if i == 5 { f64::NAN } else { 1.0 }));
        let pm = picture(&p, 1).unwrap();
        assert_eq!(at(&pm, 1, 1)[3], 0);
        assert_eq!(at(&pm, 0, 0), [200, 0, 0, 255]);
    }

    #[test]
    fn the_size_is_kept_within_the_limit() {
        assert_eq!(size_of((100, 50), 2), ((200, 100), false));
        assert_eq!(size_of((20_000, 10_000), 1), ((6667, 3334), true));
        assert_eq!(size_of((3000, 3000), 4), ((6000, 6000), true));
    }

    #[test]
    fn a_png_and_a_geotiff() {
        let p = panel();
        let dir = std::env::temp_dir().join("terraflux-export-view-test");
        std::fs::create_dir_all(&dir).unwrap();
        let (png, tif) = (dir.join("view.png"), dir.join("view.tif"));
        save(&p, 2, Format::Png, &png).unwrap();
        let back = tiny_skia::Pixmap::load_png(&png).unwrap();
        assert_eq!((back.width(), back.height()), (8, 8));
        assert_eq!(at(&back, 7, 7), [0, 200, 0, 255]);
        assert_eq!(at(&back, 7, 1), [200, 0, 0, 255]);
        let said = save(&p, 2, Format::GeoTiff, &tif).unwrap();
        assert!(said.contains("8 x 8 px"), "{said}");
        // four bands of bytes on the map's grid, its cells half as large
        let r = terraflux_raster::tiff::read(&tif).unwrap();
        assert_eq!(r.bands.len(), 4);
        assert_eq!((r.grid.width, r.grid.height), (8, 8));
        assert_eq!(r.grid.transform, [100.0, 5.0, 0.0, 500.0, 0.0, -5.0]);
        assert_eq!(r.grid.geo.epsg(), Some(4326));
        let px = |x: usize, y: usize| -> Vec<u8> { r.bands.iter().map(|b| b.data[y * 8 + x] as u8).collect() };
        assert_eq!(px(7, 1), [200, 0, 0, 255]);
        assert_eq!(px(5, 6), [0, 200, 0, 255]);
        let [r0, g0, b0, a0] = px(0, 7)[..] else { panic!() };
        assert!((95..=105).contains(&r0) && g0 == 0 && (95..=105).contains(&b0) && a0 == 255);
    }
}
