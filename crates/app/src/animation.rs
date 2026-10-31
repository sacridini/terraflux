//! The steps of a simulation, or the maps of a stack, as an animated GIF: each map in the colours it is shown in,
//! enlarged when small, its caption ("step 12 of 50") written in its corner in the application's font and its
//! legend under it, the cells without a value white. Made by a thread, so that a long simulation does not stop
//! the application.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use egui::{Color32, RichText, Ui};

use crate::map_view::Source;
#[cfg(test)]
use crate::palette::Ramp;
use crate::palette::{Coloring, image_f32};

/// A frame: a map, its colours and what is written on it.
#[derive(Clone)]
pub struct Frame {
    pub source: Source,
    pub coloring: Coloring,
    pub caption: String,
}

/// The longest side of the GIF's frames, pixels (a larger map read every few cells).
pub const MAX_SIDE: usize = 800;
/// A small map is enlarged (cells as squares of pixels) until its longest side is at least this.
const MIN_SIDE: usize = 480;

/// A GIF being made: where, and its result when the thread ends.
pub struct Job {
    pub path: PathBuf,
    rx: Receiver<Result<(), String>>,
}

/// What became of the GIFs asked for: the one being made, and what the last one said.
#[derive(Default)]
pub struct Animations {
    pub job: Option<Job>,
    pub said: Option<String>,
}

impl Animations {
    /// The GIF of the frames `make` gives (in the thread: their colours may take a while to work out) written to
    /// `path` by a thread, `delay` milliseconds a frame.
    pub fn start(&mut self, make: impl FnOnce() -> Vec<Frame> + Send + 'static, delay: u16, path: PathBuf) {
        let (tx, rx) = channel();
        let to = path.clone();
        std::thread::spawn(move || {
            let _ = tx.send(write(&make(), delay, MAX_SIDE, &to));
        });
        self.job = Some(Job { path, rx });
        self.said = None;
    }

    /// Whether a GIF is being made (what it became said once it ends).
    pub fn busy(&mut self) -> bool {
        let Some(j) = &self.job else { return false };
        match j.rx.try_recv() {
            Ok(r) => {
                self.said = Some(match r {
                    Ok(()) => format!("saved: {}", j.path.display()),
                    Err(e) => format!("not saved: {e}"),
                });
                self.job = None;
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.said = Some("not saved: the thread stopped".into());
                self.job = None;
                false
            }
        }
    }

    /// A button that asks where to save the GIF of the frames (`prepare`, once clicked, gives what makes them in
    /// the thread), `delay` milliseconds a frame, and what became of the last one.
    pub fn button<F>(&mut self, ui: &mut Ui, name: &str, delay: u16, prepare: impl FnOnce() -> F)
    where
        F: FnOnce() -> Vec<Frame> + Send + 'static,
    {
        let busy = self.busy();
        ui.horizontal_wrapped(|ui| {
            let b = ui
                .add_enabled(!busy, egui::Button::new("Save as GIF..."))
                .on_hover_text("an animation of the maps, each with its caption, for a talk or a class");
            if b.clicked()
                && let Some(path) = rfd::FileDialog::new()
                    .set_title("Save the animation")
                    .add_filter("Animated GIF", &["gif"])
                    .set_file_name(format!("{name}.gif"))
                    .save_file()
            {
                self.start(prepare(), delay, path);
            }
            if busy {
                ui.spinner();
                ui.label(RichText::new("making the GIF...").weak().size(11.0));
                ui.ctx().request_repaint_after(std::time::Duration::from_millis(200));
            } else if let Some(t) = &self.said {
                ui.label(RichText::new(t).weak().size(11.0));
            }
        });
    }
}

/// The cells a map is read every (so that neither side passes `max_side`) and the size that makes.
fn fitted((w, h): (usize, usize), max_side: usize) -> (usize, usize, usize) {
    let step = w.max(h).div_ceil(max_side.max(1)).max(1);
    (step, w.div_ceil(step), h.div_ceil(step))
}

/// A map's values, every `step` cells, and the size they make.
fn values(src: &Source, max_side: usize) -> Result<(Vec<f32>, usize, usize), String> {
    let (w, h) = src.size();
    if w == 0 || h == 0 {
        return Err("an empty map".into());
    }
    let (step, ow, oh) = fitted((w, h), max_side);
    match src {
        Source::Map(m) => {
            let mut v = Vec::with_capacity(ow * oh);
            for r in 0..oh {
                let row = &m.data[(r * step) * w..];
                v.extend((0..ow).map(|c| row[c * step] as f32));
            }
            Ok((v, ow, oh))
        }
        Source::Rasters(r, l) => {
            let win = terraflux_raster::tiff::Window { x0: 0, y0: 0, width: w, height: h };
            let v = r.sample(*l, win, step).map_err(|e| e.to_string())?;
            Ok((v, ow, oh))
        }
        Source::Blank(_) => Err("no map".into()),
    }
}

/// The font of the captions.
fn font() -> FontRef<'static> {
    FontRef::try_from_slice(include_bytes!("../assets/fonts/SourceSans3-Regular.ttf")).expect("the bundled font")
}

/// An image being drawn: RGBA, `w` x `h` pixels, and the font of its text.
struct Canvas<'a> {
    rgba: &'a mut [u8],
    w: usize,
    h: usize,
    font: FontRef<'static>,
}

impl Canvas<'_> {
    /// The glyphs of `text` at `px` pixels on a line from x = 0, kerned, and its width.
    fn layout(&self, px: f32, text: &str) -> (Vec<ab_glyph::Glyph>, f32) {
        let scaled = self.font.as_scaled(PxScale::from(px));
        let (mut x, mut last, mut glyphs) = (0.0f32, None, Vec::new());
        for ch in text.chars() {
            let id = scaled.glyph_id(ch);
            if let Some(prev) = last {
                x += scaled.kern(prev, id);
            }
            glyphs.push(id.with_scale_and_position(px, ab_glyph::point(x, scaled.ascent())));
            x += scaled.h_advance(id);
            last = Some(id);
        }
        (glyphs, x)
    }

    fn width(&self, px: f32, text: &str) -> f32 {
        self.layout(px, text).1
    }

    /// `text` at `px` pixels, its line's top left corner at (x, y), in `colour`; its width.
    fn text(&mut self, x: f32, y: f32, px: f32, text: &str, colour: [u8; 3]) -> f32 {
        let (glyphs, width) = self.layout(px, text);
        for g in glyphs {
            let Some(o) = self.font.outline_glyph(g) else { continue };
            let b = o.px_bounds();
            let (w, h, rgba) = (self.w, self.h, &mut *self.rgba);
            o.draw(|gx, gy, cov| {
                let (px_, py_) = ((x + b.min.x) as i64 + gx as i64, (y + b.min.y) as i64 + gy as i64);
                if px_ < 0 || py_ < 0 || px_ as usize >= w || py_ as usize >= h {
                    return;
                }
                let i = (py_ as usize * w + px_ as usize) * 4;
                let cov = cov.clamp(0.0, 1.0);
                for (c, to) in rgba[i..i + 3].iter_mut().zip(colour) {
                    *c = (*c as f32 + (to as f32 - *c as f32) * cov).round() as u8;
                }
            });
        }
        width
    }

    /// The pixels of a rectangle: `f` gives each one's colour from its own and its place across (0 to 1).
    fn fill(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, f: impl Fn([u8; 3], f32) -> [u8; 3]) {
        let (x1, y1) = (x1.min(self.w), y1.min(self.h));
        for y in y0..y1 {
            for x in x0..x1 {
                let i = (y * self.w + x) * 4;
                let t = (x - x0) as f32 / (x1 - x0).saturating_sub(1).max(1) as f32;
                let c = f([self.rgba[i], self.rgba[i + 1], self.rgba[i + 2]], t);
                self.rgba[i..i + 3].copy_from_slice(&c);
            }
        }
    }
}

/// The size of the text of a frame `side` pixels across (its shorter side).
fn text_px(side: usize) -> f32 {
    (side as f32 / 22.0).clamp(14.0, 26.0)
}

/// A number of a legend, short.
fn short(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else if v.abs() >= 100.0 {
        format!("{v:.0}")
    } else {
        let t = format!("{v:.3}");
        t.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// The most classes a legend names (then "and k more").
const MAX_CLASSES: usize = 16;

/// What the legend of a colouring shows: each class (its colour and name), or the ramp and its two ends.
enum Key {
    Classes(Vec<([u8; 3], String)>),
    Ramp(Coloring, String, String),
}

fn key(coloring: &Coloring) -> Key {
    let rgb = |c: Color32| [c.r(), c.g(), c.b()];
    match coloring {
        Coloring::Classes(c) => {
            let mut v: Vec<_> = c
                .iter()
                .take(MAX_CLASSES)
                .map(|(k, col)| (rgb(crate::palette::seen(*col)), terraflux_ops::value::class_label(*k)))
                .collect();
            if c.len() > MAX_CLASSES {
                v.push(([255, 255, 255], format!("and {} more", c.len() - MAX_CLASSES)));
            }
            Key::Classes(v)
        }
        Coloring::Continuous { min, max, log, .. } => {
            let scale = if *log { " (log)" } else { "" };
            Key::Ramp(coloring.clone(), short(*min), format!("{}{scale}", short(*max)))
        }
    }
}

/// The lines of a legend `w` pixels wide at `px`: for each line, the classes on it (their index).
fn key_lines(c: &Canvas, px: f32, w: usize, key: &Key) -> Vec<Vec<usize>> {
    let Key::Classes(items) = key else { return vec![vec![]] };
    let (pad, gap) = (px * 0.6, px * 1.1);
    let mut lines = vec![Vec::new()];
    let mut x = pad;
    for (i, (_, name)) in items.iter().enumerate() {
        let wi = px + px * 0.35 + c.width(px, name);
        if x + wi > w as f32 - pad && !lines.last().unwrap().is_empty() {
            lines.push(Vec::new());
            x = pad;
        }
        lines.last_mut().unwrap().push(i);
        x += wi + gap;
    }
    lines
}

/// The legend drawn from the row `y0` down.
fn draw_key(c: &mut Canvas, y0: usize, px: f32, key: &Key, lines: &[Vec<usize>]) {
    let pad = px * 0.6;
    let line = px * 1.5;
    match key {
        Key::Classes(items) => {
            for (l, on) in lines.iter().enumerate() {
                let y = y0 as f32 + pad + l as f32 * line;
                let mut x = pad;
                for &i in on {
                    let (colour, name) = &items[i];
                    let s = px.round() as usize;
                    let (sx, sy) = (x.round() as usize, (y + (line - px) * 0.5 - px * 0.2).round() as usize);
                    c.fill(sx, sy, sx + s, sy + s, |_, _| [90, 90, 90]);
                    c.fill(sx + 1, sy + 1, sx + s - 1, sy + s - 1, |_, _| *colour);
                    x += px + px * 0.35;
                    x += c.text(x, y, px, name, [40, 40, 40]) + px * 1.1;
                }
            }
        }
        Key::Ramp(coloring, lo, hi) => {
            let y = y0 as f32 + pad;
            let mut x = pad;
            x += c.text(x, y, px, lo, [40, 40, 40]) + px * 0.5;
            let hi_w = c.width(px, hi);
            let bar = ((c.w as f32 - x - hi_w - px * 0.5 - pad).min(px * 16.0)).max(px * 3.0);
            let (bx, by) = (x.round() as usize, (y + px * 0.15).round() as usize);
            let (bx1, by1) = (bx + bar.round() as usize, by + (px * 0.85).round() as usize);
            let Coloring::Continuous { ramp, .. } = coloring else { return };
            let ramp = *ramp;
            c.fill(bx, by, bx1, by1, |_, t| {
                let k = crate::palette::seen(ramp.colour(t));
                [k.r(), k.g(), k.b()]
            });
            c.text(bx1 as f32 + px * 0.5, y, px, hi, [40, 40, 40]);
        }
    }
}

/// A frame as pixels: the map in its colours over white, enlarged when small, its caption in its top left corner
/// on a dark box, and the legend under it.
/// The pixels of a frame of a map of `size` cells (read as [`fitted`]): its zoom, the map's size enlarged, the
/// size of its text.
fn layout_of(size: (usize, usize), max_side: usize) -> (usize, usize, usize, f32) {
    let (_, w, h) = fitted(size, max_side);
    let zoom = MIN_SIDE.div_ceil(w.max(h).max(1)).max(1);
    let (zw, zh) = (w * zoom, h * zoom);
    (zoom, zw, zh, text_px(zw.min(zh).max(200)))
}

/// The lines of the legend of a colouring under a map `zw` pixels wide.
fn legend_lines(coloring: &Coloring, zw: usize, px: f32) -> usize {
    let mut none = [];
    let c = Canvas { rgba: &mut none, w: zw, h: 0, font: font() };
    key_lines(&c, px, zw, &key(coloring)).len()
}

/// A frame as pixels, its legend at least `lines_high` lines high (the most of all the frames, so that they are
/// of one size).
fn picture(f: &Frame, max_side: usize, lines_high: usize) -> Result<(Vec<u8>, usize, usize), String> {
    let (v, w, h) = values(&f.source, max_side)?;
    let img = image_f32(&v, w, h, &f.coloring);
    let (zoom, zw, zh, px) = layout_of(f.source.size(), max_side);
    let font = font();
    let key = key(&f.coloring);
    let lines = {
        let mut none = [];
        let c = Canvas { rgba: &mut none, w: zw, h: 0, font: font.clone() };
        key_lines(&c, px, zw, &key)
    };
    let kh = (px * 1.2 + lines.len().max(lines_high) as f32 * px * 1.5).round() as usize;
    let fh = zh + kh;
    let mut rgba = vec![255u8; zw * fh * 4];
    for y in 0..zh {
        for x in 0..zw {
            let c: Color32 = img.pixels[(y / zoom) * w + x / zoom];
            let a = c.a() as f32 / 255.0;
            let i = (y * zw + x) * 4;
            // (premultiplied: the colour plus white where it is transparent)
            for (k, ch) in [c.r(), c.g(), c.b()].into_iter().enumerate() {
                rgba[i + k] = (ch as f32 + 255.0 * (1.0 - a)).round().min(255.0) as u8;
            }
        }
    }
    let mut c = Canvas { rgba: &mut rgba, w: zw, h: fh, font };
    // a line between the map and its legend
    c.fill(0, zh, zw, zh + 1, |_, _| [200, 200, 200]);
    draw_key(&mut c, zh, px, &key, &lines);
    if !f.caption.is_empty() {
        let pad = px * 0.4;
        let tw = c.width(px, &f.caption);
        let (x1, y1) = ((pad * 3.0 + tw).round() as usize, (pad * 2.0 + px * 1.35).round() as usize);
        c.fill(pad.round() as usize, pad.round() as usize, x1, y1.min(zh), |p, _| p.map(|v| (v as f32 * 0.3) as u8));
        c.text(pad * 2.0, pad * 1.6, px, &f.caption, [255, 255, 255]);
    }
    Ok((rgba, zw, fh))
}

/// The GIF of the frames, `delay` milliseconds each (the last one held longer), looping.
pub fn write(frames: &[Frame], delay: u16, max_side: usize, path: &Path) -> Result<(), String> {
    let Some(first) = frames.first() else { return Err("no map to animate".into()) };
    let size = first.source.size();
    if let Some(f) = frames.iter().find(|f| f.source.size() != size) {
        let (a, b) = (size, f.source.size());
        return Err(format!("the maps are not of one size ({} x {}, then {} x {})", a.0, a.1, b.0, b.1));
    }
    let (_, zw, _, px) = layout_of(size, max_side);
    let lines = frames.iter().map(|f| legend_lines(&f.coloring, zw, px)).max().unwrap_or(1);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut out: Option<(gif::Encoder<std::io::BufWriter<std::fs::File>>, usize, usize)> = None;
    let mut file = Some(std::io::BufWriter::new(file));
    for (k, f) in frames.iter().enumerate() {
        let (mut rgba, w, h) = picture(f, max_side, lines)?;
        if out.is_none() {
            let mut e = gif::Encoder::new(file.take().unwrap(), w as u16, h as u16, &[]).map_err(|e| e.to_string())?;
            e.set_repeat(gif::Repeat::Infinite).map_err(|e| e.to_string())?;
            out = Some((e, w, h));
        }
        let (enc, w0, h0) = out.as_mut().unwrap();
        debug_assert_eq!((w, h), (*w0, *h0));
        let mut frame = gif::Frame::from_rgba_speed(w as u16, h as u16, &mut rgba, 10);
        let last = k + 1 == frames.len();
        frame.delay = if last { (delay / 10).saturating_mul(4).max(150) } else { (delay / 10).max(2) };
        enc.write_frame(&frame).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terraflux_ops::Map;

    fn map(w: usize, h: usize, f: impl Fn(usize) -> f64) -> Source {
        let grid = Arc::new(terraflux_raster::Grid::new(w, h));
        Source::Map(Map::new(grid, (0..w * h).map(f).collect()))
    }

    #[test]
    fn steps_as_a_gif() {
        let coloring =
            Coloring::Classes(vec![(1.0, Color32::from_rgb(0, 114, 178)), (2.0, Color32::from_rgb(230, 159, 0))]);
        // three steps of 40 x 30 cells, the second class growing; a cell without a value
        let frames: Vec<Frame> = (0..3)
            .map(|k| Frame {
                source: map(40, 30, move |i| {
                    if i == 0 {
                        f64::NAN
                    } else if i % 40 < 10 * (k + 1) {
                        2.0
                    } else {
                        1.0
                    }
                }),
                coloring: coloring.clone(),
                caption: format!("step {k} of 2"),
            })
            .collect();
        let path = std::env::temp_dir().join(format!("terraflux-anim-{}.gif", std::process::id()));
        write(&frames, 200, MAX_SIDE, &path).unwrap();
        // read back: three frames, the map enlarged to 480 x 360 pixels (12 per cell), its legend under it
        let mut d = gif::DecodeOptions::new();
        d.set_color_output(gif::ColorOutput::RGBA);
        let mut r = d.read_info(std::fs::File::open(&path).unwrap()).unwrap();
        assert_eq!(r.width(), 480);
        let h = r.height() as usize;
        assert!((380..440).contains(&h), "{h}");
        let mut n = 0;
        let mut first = Vec::new();
        while let Some(f) = r.read_next_frame().unwrap() {
            if n == 0 {
                first = f.buffer.to_vec();
            }
            n += 1;
        }
        assert_eq!(n, 3);
        // the cell without a value white, a cell of the map in its class's colour (far from the caption)
        let at = |x: usize, y: usize| &first[(y * 480 + x) * 4..(y * 480 + x) * 4 + 3];
        let near = |p: &[u8], c: [u8; 3]| p.iter().zip(c).all(|(a, b)| a.abs_diff(b) <= 12);
        assert!(near(at(470, 350), [0, 114, 178]), "{:?}", at(470, 350));
        assert!(near(at(6, 350), [230, 159, 0]), "{:?}", at(6, 350));
        assert!(near(at(2, 2), [255, 255, 255]), "{:?}", at(2, 2));
        // the caption darkened its corner and wrote in it, white
        let corner: Vec<&[u8]> = (8..30).flat_map(|y| (8..90).map(move |x| (x, y))).map(|(x, y)| at(x, y)).collect();
        assert!(corner.iter().any(|p| p.iter().all(|&c| c < 70)));
        assert!(corner.iter().any(|p| p.iter().all(|&c| c > 200)));
        // the legend: both classes' swatches, under the map
        let key: Vec<&[u8]> = (362..h).flat_map(|y| (0..480).map(move |x| (x, y))).map(|(x, y)| at(x, y)).collect();
        assert!(key.iter().any(|p| near(p, [0, 114, 178])) && key.iter().any(|p| near(p, [230, 159, 0])));
        if let Some(to) = std::env::var_os("TERRAFLUX_GIF_DEMO") {
            std::fs::copy(&path, to).unwrap();
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_ramp_in_the_legend() {
        let coloring = Coloring::Continuous { min: 0.0, max: 2.5, ramp: Default::default(), log: false };
        let frames =
            vec![Frame { source: map(60, 20, |i| (i % 60) as f64 / 24.0), coloring, caption: "elevation".into() }];
        let path = std::env::temp_dir().join(format!("terraflux-anim-ramp-{}.gif", std::process::id()));
        // maps of other sizes are not animated together
        let two = [frames[0].clone(), Frame { source: map(10, 10, |_| 1.0), ..frames[0].clone() }];
        assert!(write(&two, 600, MAX_SIDE, &path).unwrap_err().contains("not of one size"));
        write(&frames, 600, MAX_SIDE, &path).unwrap();
        let mut d = gif::DecodeOptions::new();
        d.set_color_output(gif::ColorOutput::RGBA);
        let mut r = d.read_info(std::fs::File::open(&path).unwrap()).unwrap();
        let (w, h) = (r.width() as usize, r.height() as usize);
        assert_eq!(w, 480);
        let f = r.read_next_frame().unwrap().unwrap().buffer.to_vec();
        // under the map (160 pixels high), the ramp's two ends
        let key: Vec<[u8; 3]> =
            (162..h).flat_map(|y| (0..w).map(move |x| (y * w + x) * 4)).map(|i| [f[i], f[i + 1], f[i + 2]]).collect();
        let (lo, hi) = (Ramp::default().colour(0.0), Ramp::default().colour(1.0));
        let near = |p: &[u8; 3], c: Color32| p.iter().zip([c.r(), c.g(), c.b()]).all(|(a, b)| a.abs_diff(b) <= 16);
        assert!(key.iter().any(|p| near(p, lo)) && key.iter().any(|p| near(p, hi)));
        assert_eq!(short(2.5), "2.5");
        assert_eq!(short(1.0 / 3.0), "0.333");
        assert_eq!(short(1234.56), "1235");
        if let Some(to) = std::env::var_os("TERRAFLUX_GIF_DEMO2") {
            std::fs::copy(&path, to).unwrap();
        }
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn made_by_a_thread() {
        let coloring = Coloring::Classes(vec![(1.0, Color32::from_rgb(0, 114, 178))]);
        let path = std::env::temp_dir().join(format!("terraflux-anim-thread-{}.gif", std::process::id()));
        let mut a = Animations::default();
        let c = coloring.clone();
        a.start(
            move || vec![Frame { source: map(8, 8, |_| 1.0), coloring: c, caption: "one".into() }],
            200,
            path.clone(),
        );
        let t = std::time::Instant::now();
        while a.busy() {
            assert!(t.elapsed().as_secs() < 30);
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(a.said.as_deref(), Some(format!("saved: {}", path.display()).as_str()));
        assert!(path.exists());
        // nothing to animate: said
        a.start(Vec::new, 200, path.clone());
        while a.busy() {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(a.said.as_deref(), Some("not saved: no map to animate"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_large_map_is_read_every_few_cells() {
        let (v, w, h) = values(&map(2000, 1000, |i| i as f64), 800).unwrap();
        assert_eq!((w, h), (667, 334));
        assert_eq!(v.len(), 667 * 334);
        assert_eq!(v[1], 3.0);
        assert_eq!(v[667], 6000.0);
    }
}
