//! The Viewer's map written as a picture: what it shows (the map, the maps under and over it at their opacity, the
//! vectors and their labels), a cell to a pixel or magnified, as PNG or as a GeoTIFF of red, green, blue and alpha
//! that opens in QGIS over other data. No legend or decorations: the Figure has them.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use egui::RichText;

use crate::figure::Panel;
use crate::map_view::Basemap;

/// The longest side of a picture, pixels (a larger map is read every few cells).
pub const MAX_SIDE: usize = 8000;

/// The pixels between the views of a picture side by side (transparent).
pub const GUTTER: usize = 8;

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
    /// The views beside the map when the Viewer shows them side by side (the one compared, then those more), and
    /// whether the picture has them (PNG only: a GeoTIFF has one grid).
    pub beside: Vec<Panel>,
    pub together: bool,
    /// The basemap the Viewer shows under the map and how opaque the data is over it, and whether the picture
    /// has it.
    pub basemap: Basemap,
    pub opacity: f32,
    pub with_basemap: bool,
    job: Option<(PathBuf, Receiver<Result<String, String>>)>,
    said: Option<String>,
}

impl Export {
    pub fn new(panel: Panel) -> Export {
        Export {
            panel,
            scale: 1,
            beside: Vec::new(),
            together: true,
            basemap: Basemap::None,
            opacity: 1.0,
            with_basemap: true,
            job: None,
            said: None,
        }
    }

    /// The views beside the map shown (an option of the export when there are some).
    pub fn beside(mut self, views: Vec<Panel>) -> Export {
        self.beside = views;
        self
    }

    /// The basemap shown under the map, and how opaque the data is over it (an option when there is one).
    pub fn under(mut self, basemap: Basemap, opacity: f32) -> Export {
        (self.basemap, self.opacity) = (basemap, opacity);
        self
    }

    /// What the picture is made with: what the options chosen leave in.
    fn options(&self) -> Options {
        Options {
            scale: self.scale,
            basemap: if self.with_basemap { self.basemap } else { Basemap::None },
            opacity: self.opacity,
            beside: if self.together { self.beside.clone() } else { Vec::new() },
        }
    }
}

/// What a picture is made with besides the map: its size, the basemap under it (and how opaque the map is over
/// it), and the views beside it, side by side (PNG only: a GeoTIFF is of the map alone).
#[derive(Clone)]
pub struct Options {
    /// Pixels to a cell of the map.
    pub scale: usize,
    pub basemap: Basemap,
    pub opacity: f32,
    pub beside: Vec<Panel>,
}

impl Options {
    /// The map alone, at a scale.
    pub fn scaled(scale: usize) -> Options {
        Options { scale, basemap: Basemap::None, opacity: 1.0, beside: Vec::new() }
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
    picture_in(p, scale, MAX_SIDE)
}

/// One view as a picture within `max_side` pixels, over the basemap of the options when there is one (what could
/// not be had of it is said in `notes`: with no CRS the map has none, the picture is then the map alone).
fn one(p: &Panel, o: &Options, max_side: usize, notes: &mut Vec<String>) -> Result<tiny_skia::Pixmap, String> {
    if o.basemap == Basemap::None {
        return picture_in(p, o.scale, max_side);
    }
    let said = std::cell::RefCell::new(Vec::new());
    let grid = p.source.grid().clone();
    let make = |w: usize, h: usize| match crate::basemap::picture(o.basemap, &grid, w, h) {
        Ok(b) => {
            if b.missing > 0 {
                said.borrow_mut().push(format!("{} tiles of the basemap were not there", b.missing));
            }
            Ok(b.rgba)
        }
        Err(e) => {
            said.borrow_mut().push(format!("no basemap: {e}"));
            Ok(vec![0; w * h * 4])
        }
    };
    let under = crate::figure::Under { make: &make, opacity: o.opacity };
    let pm = crate::figure::plain_over(p, o.scale, max_side, Some(&under))?.clear_pixels();
    notes.extend(said.into_inner());
    pm
}

fn picture_in(p: &Panel, scale: usize, max_side: usize) -> Result<tiny_skia::Pixmap, String> {
    crate::figure::plain(p, scale, max_side)?.clear_pixels()
}

/// The views as one picture: the map first, those beside it to the right of it, [`GUTTER`] pixels apart, their
/// tops level (each as large as the whole is allowed: [`MAX_SIDE`] across).
pub fn pictures(p: &Panel, o: &Options, notes: &mut Vec<String>) -> Result<tiny_skia::Pixmap, String> {
    if o.beside.is_empty() {
        return one(p, o, MAX_SIDE, notes);
    }
    let n = 1 + o.beside.len();
    let each = (MAX_SIDE - GUTTER * (n - 1)) / n;
    let mut parts = vec![one(p, o, each, notes)?];
    for b in &o.beside {
        parts.push(one(b, o, each, notes)?);
    }
    let w: usize = parts.iter().map(|m| m.width() as usize).sum::<usize>() + GUTTER * (n - 1);
    let h = parts.iter().map(|m| m.height()).max().unwrap_or(1);
    let mut out = tiny_skia::Pixmap::new(w as u32, h).ok_or("no picture of this size")?;
    let mut x = 0;
    for m in &parts {
        out.draw_pixmap(x, 0, m.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
        x += m.width() as i32 + GUTTER as i32;
    }
    notes.sort();
    notes.dedup();
    Ok(out)
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
    save_with(p, &Options::scaled(scale), format, path)
}

/// [`save`] with the basemap under the map and the views beside it (a GeoTIFF is of the map alone, over its
/// basemap: it has one grid, and what is told says so).
pub fn save_with(p: &Panel, o: &Options, format: Format, path: &Path) -> Result<String, String> {
    let scale = o.scale;
    let mut notes = Vec::new();
    let tif_alone = format == Format::GeoTiff && !o.beside.is_empty();
    let o = &Options { beside: if format == Format::Png { o.beside.clone() } else { Vec::new() }, ..o.clone() };
    let pm = pictures(p, o, &mut notes)?;
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
    if o.beside.is_empty() && (w, h) != (gw * scale.max(1), gh * scale.max(1)) {
        said.push_str(&format!(", the map read every few cells to stay within {MAX_SIDE} px"));
    }
    if !o.beside.is_empty() {
        said.push_str(&format!(", {} views side by side", 1 + o.beside.len()));
    }
    if tif_alone {
        said.push_str(": a GeoTIFF has one grid, so it holds the map shown alone (the PNG has the views side by side)");
    }
    for n in notes {
        said.push_str(&format!("; {n}"));
    }
    Ok(said)
}

/// The size of the picture the options make, as the dialog tells it: the map's, or the views side by side.
fn size_with(e: &Export) -> ((usize, usize), bool) {
    let o = e.options();
    if o.beside.is_empty() {
        return size_of(e.panel.source.size(), e.scale);
    }
    let n = 1 + o.beside.len();
    let each = (MAX_SIDE - GUTTER * (n - 1)) / n;
    let (mut w, mut h, mut coarse) = (GUTTER * (n - 1), 0, false);
    for p in std::iter::once(&e.panel).chain(&o.beside) {
        let k = e.scale.max(1);
        let (gw, gh) = p.source.size();
        let step = (gw.max(gh) * k).div_ceil(each).max(1);
        w += gw.div_ceil(step) * k;
        h = h.max(gh.div_ceil(step) * k);
        coarse |= step > 1;
    }
    ((w, h), coarse)
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
        if !e.beside.is_empty() {
            ui.checkbox(&mut e.together, "with the views side by side")
                .on_hover_text("the map and the views beside it in one picture, as the Viewer shows them (PNG only: a GeoTIFF has one grid, so it holds the map shown alone)");
        }
        if e.basemap != Basemap::None {
            let name = e.basemap.name();
            ui.checkbox(&mut e.with_basemap, format!("with the basemap ({name})"))
                .on_hover_text("the web tiles under the map, as the Viewer shows them, from the disk or fetched; the map as opaque as it is in the Viewer");
        }
        let ((w, h), coarse) = size_with(e);
        let note = if coarse { format!("{w} x {h} px (the map is read every few cells)") } else { format!("{w} x {h} px") };
        ui.label(RichText::new(note).weak());
        if e.together && !e.beside.is_empty() {
            ui.label(RichText::new("the GeoTIFF holds the map shown alone").weak().size(11.0));
        }
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
                    let (panel, options) = (e.panel.clone(), e.options());
                    let (tx, rx) = channel();
                    let (to, ctx) = (path.clone(), ui.ctx().clone());
                    std::thread::spawn(move || {
                        let _ = tx.send(save_with(&panel, &options, format, &to));
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
                sizes: Vec::new(),
                symbol: Default::default(),
                legend: Default::default(),
            }],
            rasters: vec![RasterLayer {
                source: Source::Map(layer),
                coloring: classes(&[(1.0, BLUE)]),
                opacity: 0.5,
                below: false,
                name: "layer".into(),
                blend: Default::default(),
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

    /// A second view of the same map in green, for beside the first.
    fn green() -> Panel {
        let mut p = panel();
        p.vectors.clear();
        p.rasters.clear();
        p.coloring = classes(&[(1.0, Color32::from_rgb(0, 200, 0))]);
        p
    }

    #[test]
    fn the_views_side_by_side_in_one_picture() {
        let mut main = panel();
        main.vectors.clear();
        main.rasters.clear();
        let o = Options { beside: vec![green()], ..Options::scaled(1) };
        let mut notes = Vec::new();
        let pm = pictures(&main, &o, &mut notes).unwrap();
        // twice the width of one view (4 pixels) and the gutter between, the height of one
        assert_eq!((pm.width() as usize, pm.height() as usize), (4 + GUTTER + 4, 4));
        assert_eq!(at(&pm, 0, 0), [200, 0, 0, 255]);
        assert_eq!(at(&pm, 3, 3), [200, 0, 0, 255]);
        // (the gutter is transparent)
        assert!((4..4 + GUTTER as u32).all(|x| at(&pm, x, 1)[3] == 0));
        let x = (4 + GUTTER) as u32;
        assert_eq!(at(&pm, x, 0), [0, 200, 0, 255]);
        assert_eq!(at(&pm, x + 3, 3), [0, 200, 0, 255]);
        // magnified: each view is four times as wide
        let o = Options { scale: 4, ..o };
        let big = pictures(&main, &o, &mut notes).unwrap();
        assert_eq!((big.width() as usize, big.height() as usize), (16 + GUTTER + 16, 16));
        assert_eq!(at(&big, 15, 15), [200, 0, 0, 255]);
        assert_eq!(at(&big, (16 + GUTTER) as u32, 0), [0, 200, 0, 255]);
        // three views: the picture holds each in turn
        let o = Options { beside: vec![green(), green()], ..Options::scaled(1) };
        let three = pictures(&main, &o, &mut notes).unwrap();
        assert_eq!(three.width() as usize, 3 * 4 + 2 * GUTTER);
        assert!(notes.is_empty(), "{notes:?}");
    }

    #[test]
    fn a_geotiff_keeps_the_map_alone_and_says_so() {
        let mut main = panel();
        main.vectors.clear();
        main.rasters.clear();
        let o = Options { beside: vec![green()], ..Options::scaled(2) };
        let dir = std::env::temp_dir().join("terraflux-export-view-beside");
        std::fs::create_dir_all(&dir).unwrap();
        let (png, tif) = (dir.join("views.png"), dir.join("views.tif"));
        let said = save_with(&main, &o, Format::Png, &png).unwrap();
        assert!(said.contains("2 views side by side"), "{said}");
        let back = tiny_skia::Pixmap::load_png(&png).unwrap();
        assert_eq!((back.width() as usize, back.height() as usize), (8 + GUTTER + 8, 8));
        let said = save_with(&main, &o, Format::GeoTiff, &tif).unwrap();
        assert!(said.contains("one grid") && said.contains("8 x 8 px"), "{said}");
        let r = terraflux_raster::tiff::read(&tif).unwrap();
        assert_eq!((r.grid.width, r.grid.height), (8, 8));
        assert_eq!(r.bands[0].data[0] as u8, 200);
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A 4 x 4 map of the whole web Mercator world, the cells on the left of the first row of one class.
    fn world(f: impl Fn(usize) -> f64) -> Map {
        let e = 20037508.342789244;
        let mut g = terraflux_raster::Grid::new(4, 4);
        g.transform = [-e, e / 2.0, 0.0, e, 0.0, -e / 2.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 3857]);
        Map::new(Arc::new(g), (0..16).map(f).collect())
    }

    /// A tile of four quadrants: red, green (top), blue, yellow (bottom), as a PNG.
    fn quadrants() -> Vec<u8> {
        let mut pm = tiny_skia::Pixmap::new(256, 256).unwrap();
        for (i, c) in [[200, 0, 0], [0, 200, 0], [0, 0, 200], [200, 200, 0]].into_iter().enumerate() {
            let mut paint = tiny_skia::Paint::default();
            paint.set_color_rgba8(c[0], c[1], c[2], 255);
            paint.anti_alias = false;
            let rect = tiny_skia::Rect::from_xywh((i % 2) as f32 * 128.0, (i / 2) as f32 * 128.0, 128.0, 128.0);
            pm.fill_rect(rect.unwrap(), &paint, tiny_skia::Transform::identity(), None);
        }
        pm.encode_png().unwrap()
    }

    #[test]
    fn the_basemap_under_the_map_from_the_cached_tiles() {
        if terraflux_gdal::gdal().is_err() {
            eprintln!("no GDAL: skipped");
            return;
        }
        // (the address is a port nothing listens on: a tile not on the disk would be an error, not a download)
        let b = crate::basemap::known_for_test("http://127.0.0.1:9/export-view-test/{z}/{x}/{y}.png", 2);
        let cache = crate::basemap::cache_dir(b);
        let _ = std::fs::remove_dir_all(&cache);
        let tile = cache.join("0").join("0");
        std::fs::create_dir_all(&tile).unwrap();
        std::fs::write(tile.join("0"), quadrants()).unwrap();
        // the cell at the top left is the map's, the others have no value: the basemap shows through
        let mut p = panel();
        p.vectors.clear();
        p.rasters.clear();
        p.source = Source::Map(world(|i| if i == 0 { 1.0 } else { f64::NAN }));
        p.coloring = classes(&[(1.0, Color32::from_rgb(10, 10, 10))]);
        let mut o = Options::scaled(1);
        o.basemap = b;
        let mut notes = Vec::new();
        let pm = pictures(&p, &o, &mut notes).unwrap();
        assert_eq!((pm.width(), pm.height()), (4, 4));
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(at(&pm, 0, 0), [10, 10, 10, 255], "the map over the basemap");
        assert_eq!(at(&pm, 1, 0), [200, 0, 0, 255], "the basemap's top left");
        assert_eq!(at(&pm, 2, 1), [0, 200, 0, 255]);
        assert_eq!(at(&pm, 0, 2), [0, 0, 200, 255]);
        assert_eq!(at(&pm, 3, 3), [200, 200, 0, 255]);
        // the map as opaque as the Viewer has it: half over the basemap's blue
        p.coloring = classes(&[(1.0, Color32::from_rgb(200, 200, 200))]);
        p.source = Source::Map(world(|i| if i == 8 { 1.0 } else { f64::NAN }));
        o.opacity = 0.5;
        let pm = pictures(&p, &o, &mut notes).unwrap();
        let [r, g, b2, a] = at(&pm, 0, 2);
        assert!((98..=102).contains(&r) && (98..=102).contains(&g) && (198..=202).contains(&b2) && a == 255);
        // the basemap in a GeoTIFF too
        let dir = std::env::temp_dir().join("terraflux-export-view-basemap");
        std::fs::create_dir_all(&dir).unwrap();
        let tif = dir.join("base.tif");
        save_with(&p, &o, Format::GeoTiff, &tif).unwrap();
        let r = terraflux_raster::tiff::read(&tif).unwrap();
        let px = |x: usize, y: usize| -> Vec<u8> { r.bands.iter().map(|b| b.data[y * 4 + x] as u8).collect() };
        assert_eq!(px(3, 0), [0, 200, 0, 255]);
        std::fs::remove_dir_all(&dir).ok();
        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_basemap_that_cannot_be_had_is_said_and_the_map_alone_is_written() {
        if terraflux_gdal::gdal().is_err() {
            eprintln!("no GDAL: skipped");
            return;
        }
        // (the map of the tests is at latitudes beyond the poles: nowhere on the basemap's world)
        let b = crate::basemap::known_for_test("http://127.0.0.1:9/export-view-nowhere/{z}/{x}/{y}.png", 2);
        let mut o = Options::scaled(1);
        o.basemap = b;
        let mut notes = Vec::new();
        let pm = pictures(&panel(), &o, &mut notes).unwrap();
        assert_eq!(at(&pm, 3, 0), [200, 0, 0, 255]);
        assert!(notes.iter().any(|n| n.starts_with("no basemap")), "{notes:?}");
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
