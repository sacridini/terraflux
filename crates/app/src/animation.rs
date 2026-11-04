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
    /// The maps drawn with it in the Viewer (under it or over it, at their opacity).
    pub rasters: Vec<crate::map_view::RasterLayer>,
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

/// What became of the animations asked for: the one being made, what the last one said, and the speed chosen
/// (frames a second; none: each animation's own).
#[derive(Default)]
pub struct Animations {
    pub job: Option<Job>,
    pub said: Option<String>,
    pub fps: Option<f32>,
}

/// The slowest and the fastest speeds offered, frames a second.
const SPEEDS: std::ops::RangeInclusive<f32> = 0.5..=25.0;

impl Animations {
    /// The time between two frames, milliseconds: as the speed chosen, else `default`.
    pub fn delay(&self, default: u16) -> u16 {
        self.fps.map_or(default, |f| (1000.0 / f.clamp(*SPEEDS.start(), *SPEEDS.end())).round() as u16)
    }

    /// The speed of the animations (playing and saved), a slider in frames a second, from `default` (the time
    /// between two frames, milliseconds) until one is chosen.
    pub fn speed(&mut self, ui: &mut Ui, default: u16) {
        let mut fps = 1000.0 / f32::from(self.delay(default).max(1));
        ui.horizontal(|ui| {
            ui.label(RichText::new("speed").weak());
            ui.spacing_mut().slider_width = (ui.available_width() - 110.0).clamp(40.0, 160.0);
            let s = egui::Slider::new(&mut fps, SPEEDS).logarithmic(true).max_decimals(1).suffix(" /s");
            if ui.add(s).on_hover_text("frames a second, when playing and in the GIF or video saved").changed() {
                self.fps = Some(fps);
            }
        });
    }

    /// The animation of the frames `make` gives (in the thread: their colours may take a while to work out)
    /// written to `path` by a thread, `delay` milliseconds a frame: a video when the path ends in .mp4, else a
    /// GIF.
    pub fn start(&mut self, make: impl FnOnce() -> Vec<Frame> + Send + 'static, delay: u16, path: PathBuf) {
        let (tx, rx) = channel();
        let to = path.clone();
        let video = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("mp4"));
        std::thread::spawn(move || {
            let frames = make();
            let made = if video { mp4(&frames, delay, MAX_SIDE, &to) } else { write(&frames, delay, MAX_SIDE, &to) };
            let _ = tx.send(made);
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

    /// The buttons that ask where to save the animation of the frames, as a GIF or a video
    /// (`prepare`, once clicked, gives what makes them in the thread), at the speed chosen (else `default`
    /// milliseconds a frame); and what became of the last one.
    pub fn button<F>(&mut self, ui: &mut Ui, name: &str, default: u16, prepare: impl FnOnce() -> F)
    where
        F: FnOnce() -> Vec<Frame> + Send + 'static,
    {
        let busy = self.busy();
        let delay = self.delay(default);
        ui.horizontal_wrapped(|ui| {
            let gif = ui
                .add_enabled(!busy, egui::Button::new("Save as GIF..."))
                .on_hover_text("an animation of the maps, each with its caption, for a talk or a class");
            let video = ui
                .add_enabled(!busy, egui::Button::new("Save as MP4..."))
                .on_hover_text("a video of the maps (H.264), for slides and players");
            let (ext, filter) = match (gif.clicked(), video.clicked()) {
                (true, _) => ("gif", "Animated GIF"),
                (_, true) => ("mp4", "Video (MP4)"),
                _ => ("", ""),
            };
            if !ext.is_empty()
                && let Some(path) = rfd::FileDialog::new()
                    .set_title("Save the animation")
                    .add_filter(filter, &[ext])
                    .set_file_name(format!("{name}.{ext}"))
                    .save_file()
            {
                self.start(prepare(), delay, path);
            }
            if busy {
                ui.spinner();
                ui.label(RichText::new("making the animation...").weak().size(11.0));
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
pub(crate) fn values(src: &Source, max_side: usize) -> Result<(Vec<f32>, usize, usize), String> {
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

/// A map's picture as [`values`] takes its cells, with the maps drawn with it: those `below` under it, the
/// others over it, each at its opacity and placed by its grid (one of another coordinate system carried to the
/// map's; one that cannot be carried left out); premultiplied colours, transparent where nothing is.
pub(crate) fn composed(
    src: &Source,
    coloring: &Coloring,
    layers: &[crate::map_view::RasterLayer],
    max_side: usize,
) -> Result<egui::ColorImage, String> {
    let (v, w, h) = values(src, max_side)?;
    let main = image_f32(&v, w, h, coloring);
    if layers.is_empty() {
        return Ok(main);
    }
    let (step, _, _) = fitted(src.size(), max_side);
    let over = |dst: Color32, top: Color32| -> Color32 {
        let k = 1.0 - f32::from(top.a()) / 255.0;
        let mix = |a: u8, b: u8| (f32::from(a) + f32::from(b) * k).round().min(255.0) as u8;
        Color32::from_rgba_premultiplied(
            mix(top.r(), dst.r()),
            mix(top.g(), dst.g()),
            mix(top.b(), dst.b()),
            mix(top.a(), dst.a()),
        )
    };
    let mut out = vec![Color32::TRANSPARENT; w * h];
    let draw = |out: &mut [Color32], l: &crate::map_view::RasterLayer| {
        let Some(link) = crate::reproject::Link::between(l.source.grid(), src.grid()) else { return };
        let side = (max_side * 2).min(4096);
        let Ok((lv, lw, lh)) = values(&l.source, side) else { return };
        let (lstep, _, _) = fitted(l.source.size(), side);
        let img = image_f32(&lv, lw, lh, &l.coloring);
        let opacity = l.opacity.clamp(0.0, 1.0);
        for r in 0..h {
            // a row's pixel centres on the layer: by its place, or each one carried through the systems' transformation
            // (a row at once)
            let carried = match &link {
                crate::reproject::Link::Warp(re) => {
                    let row: Vec<[f64; 2]> =
                        (0..w).map(|c| [((c * step) as f64) + 0.5, ((r * step) as f64) + 0.5]).collect();
                    Some(re.to_source_many(&row))
                }
                crate::reproject::Link::Place(_) => None,
            };
            for c in 0..w {
                // the cell's centre on the map, then on the layer
                let (x, y) = ((c * step) as f32 + 0.5, (r * step) as f32 + 0.5);
                let (lx, ly) = match (&link, &carried) {
                    (crate::reproject::Link::Place(p), _) => {
                        ((x - p.offset.x) / p.scale.x, (y - p.offset.y) / p.scale.y)
                    }
                    (_, Some(row)) => (row[c][0] as f32, row[c][1] as f32),
                    _ => continue,
                };
                if !(lx >= 0.0 && ly >= 0.0) {
                    continue;
                }
                let (pc, pr) = (lx as usize / lstep, ly as usize / lstep);
                if pc >= lw || pr >= lh {
                    continue;
                }
                let top = img.pixels[pr * lw + pc].gamma_multiply(opacity);
                out[r * w + c] = over(out[r * w + c], top);
            }
        }
    };
    for l in layers.iter().filter(|l| l.below) {
        draw(&mut out, l);
    }
    for (o, m) in out.iter_mut().zip(&main.pixels) {
        *o = over(*o, *m);
    }
    for l in layers.iter().filter(|l| !l.below) {
        draw(&mut out, l);
    }
    Ok(egui::ColorImage::new([w, h], out))
}

/// The font of the captions.
pub(crate) fn font() -> FontRef<'static> {
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
pub(crate) fn short(v: f64) -> String {
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
    let img = composed(&f.source, &f.coloring, &f.rasters, max_side)?;
    let w = img.size[0];
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

/// The GIF of the frames, `delay` milliseconds each (the last one held longer), looping, written to a file.
pub fn write(frames: &[Frame], delay: u16, max_side: usize, path: &Path) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    encode(frames, delay, max_side, std::io::BufWriter::new(file))
}

/// Pixels with even sides (as a video's colours, a value each two by two pixels, take them): a white column
/// and row added where a side is odd.
fn even(rgba: Vec<u8>, w: usize, h: usize) -> (Vec<u8>, usize, usize) {
    let (ew, eh) = (w + (w & 1), h + (h & 1));
    if (ew, eh) == (w, h) {
        return (rgba, w, h);
    }
    let mut out = vec![255u8; ew * eh * 4];
    for y in 0..h {
        out[y * ew * 4..(y * ew + w) * 4].copy_from_slice(&rgba[y * w * 4..(y + 1) * w * 4]);
    }
    (out, ew, eh)
}

/// A video of the frames (H.264 in an MP4, as slides and players take it), `delay` milliseconds each (the last one
/// held four times as long), encoded here (OpenH264, in the application: nothing to install), its sides made even.
pub fn mp4(frames: &[Frame], delay: u16, max_side: usize, path: &Path) -> Result<(), String> {
    use openh264::encoder::{BitRate, Encoder, EncoderConfig, FrameRate, UsageType};
    use openh264::formats::{RgbaSliceU8, YUVBuffer};
    let Some(first) = frames.first() else { return Err("no map to animate".into()) };
    let size = first.source.size();
    if let Some(f) = frames.iter().find(|f| f.source.size() != size) {
        let (a, b) = (size, f.source.size());
        return Err(format!("the maps are not of one size ({} x {}, then {} x {})", a.0, a.1, b.0, b.1));
    }
    let (_, zw, _, px) = layout_of(size, max_side);
    let lines = frames.iter().map(|f| legend_lines(&f.coloring, zw, px)).max().unwrap_or(1);
    let delay = u32::from(delay.max(1));
    let mut video = crate::mp4::Video::default();
    let mut encoder: Option<(Encoder, usize, usize)> = None;
    for f in frames {
        let (rgba, w, h) = picture(f, max_side, lines)?;
        let (rgba, w, h) = even(rgba, w, h);
        if encoder.is_none() {
            // (maps: sharp edges of flat colours, kept by bits enough: about two bits a pixel a second)
            let fps = 1000.0 / delay as f32;
            let bits = ((w * h) as f32 * fps * 2.0).clamp(500_000.0, 20_000_000.0) as u32;
            // (as a screen's content: flat colours, without what the encoder does not do for it; a frame it skips
            // lengthens the last one; 4 threads at most)
            let config = EncoderConfig::new()
                .usage_type(UsageType::ScreenContentRealTime)
                .adaptive_quantization(false)
                .background_detection(false)
                .bitrate(BitRate::from_bps(bits))
                .max_frame_rate(FrameRate::from_hz(fps))
                .num_threads(4);
            let e = Encoder::with_api_config(openh264::OpenH264API::from_source(), config)
                .map_err(|e| format!("the video encoder: {e}"))?;
            encoder = Some((e, w, h));
        }
        let (e, ..) = encoder.as_mut().unwrap();
        let yuv = YUVBuffer::from_rgb_source(RgbaSliceU8::new(&rgba, (w, h)));
        let stream = e.encode(&yuv).map_err(|e| format!("the video encoder: {e}"))?;
        video.add(&stream.to_vec(), delay);
    }
    // (the last frame held)
    video.add(&[], delay * 3);
    let (_, w, h) = encoder.unwrap();
    let bytes = video.finish(w as u16, h as u16)?;
    std::fs::write(path, bytes).map_err(|e| e.to_string())
}

/// The GIF of the frames as bytes (for a page that holds its images).
pub fn gif(frames: &[Frame], delay: u16, max_side: usize) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    encode(frames, delay, max_side, &mut out)?;
    Ok(out)
}

/// The GIF of the frames, `delay` milliseconds each (the last one held longer), looping.
fn encode<W: std::io::Write>(frames: &[Frame], delay: u16, max_side: usize, to: W) -> Result<(), String> {
    let Some(first) = frames.first() else { return Err("no map to animate".into()) };
    let size = first.source.size();
    if let Some(f) = frames.iter().find(|f| f.source.size() != size) {
        let (a, b) = (size, f.source.size());
        return Err(format!("the maps are not of one size ({} x {}, then {} x {})", a.0, a.1, b.0, b.1));
    }
    let (_, zw, _, px) = layout_of(size, max_side);
    let lines = frames.iter().map(|f| legend_lines(&f.coloring, zw, px)).max().unwrap_or(1);
    let mut out: Option<(gif::Encoder<W>, usize, usize)> = None;
    let mut file = Some(to);
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

    /// A map with the maps drawn with it: one under it (showing where it has no value), one over it at half its
    /// opacity, each placed by its grid.
    #[test]
    fn the_layers_are_drawn_with_the_map() {
        let (green, red, blue) =
            (Color32::from_rgb(0, 200, 0), Color32::from_rgb(200, 0, 0), Color32::from_rgb(0, 0, 200));
        // the map: its first row only; under it, the two columns at the left; over it, all of it
        let main = map(4, 4, |i| if i < 4 { 1.0 } else { f64::NAN });
        let layer = |source: Source, v: f64, c: Color32, opacity: f32, below: bool| crate::map_view::RasterLayer {
            source,
            coloring: Coloring::Classes(vec![(v, c)]),
            opacity,
            below,
            name: String::new(),
        };
        let under = layer(map(2, 4, |_| 2.0), 2.0, red, 1.0, true);
        let coloring = Coloring::Classes(vec![(1.0, green)]);
        let img = composed(&main, &coloring, std::slice::from_ref(&under), 100).unwrap();
        let at = |img: &egui::ColorImage, r: usize, c: usize| img.pixels[r * 4 + c];
        assert_eq!((at(&img, 0, 3), at(&img, 2, 0)), (green, red), "the map over the one under it");
        assert_eq!(at(&img, 2, 3).a(), 0, "nothing where neither has a value");
        let top = layer(map(4, 4, |_| 3.0), 3.0, blue, 0.5, false);
        let img = composed(&main, &coloring, &[under, top], 100).unwrap();
        let half = at(&img, 2, 3);
        assert!(half.a().abs_diff(128) <= 1 && half.b().abs_diff(100) <= 1 && half.r() == 0, "{half:?}");
        let mixed = at(&img, 2, 0);
        assert!(mixed.a() == 255 && mixed.r().abs_diff(100) <= 1 && mixed.b().abs_diff(100) <= 1, "{mixed:?}");
    }

    /// A layer in WGS 84 under a map in UTM 51N: its pixel found through the transformation, not by the cells' places.
    #[test]
    fn a_layer_of_another_coordinate_system_is_carried_to_the_map() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let (red, blue) = (Color32::from_rgb(200, 0, 0), Color32::from_rgb(0, 0, 200));
        // 400 x 300 cells of 10 m, about 122.54 E 11.76 N
        let mut g = terraflux_raster::Grid::new(400, 300);
        g.transform = [450_000.0, 10.0, 0.0, 1_300_000.0, 0.0, -10.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32651]);
        let main = Source::Map(Map::new(Arc::new(g), vec![f64::NAN; 400 * 300]));
        // 1000 x 800 cells of 0.0001 degrees from 122.5 E 11.8 N: 1 west of 122.55 E, 2 east of it
        let mut geo = terraflux_raster::Grid::new(1000, 800);
        geo.transform = [122.5, 0.0001, 0.0, 11.8, 0.0, -0.0001];
        geo.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        let data = (0..1000 * 800).map(|i| if i % 1000 < 500 { 1.0 } else { 2.0 }).collect();
        let layer = crate::map_view::RasterLayer {
            source: Source::Map(Map::new(Arc::new(geo), data)),
            coloring: Coloring::Classes(vec![(1.0, red), (2.0, blue)]),
            opacity: 1.0,
            below: true,
            name: String::new(),
        };
        let coloring = Coloring::Classes(vec![(1.0, Color32::GREEN)]);
        let img = composed(&main, &coloring, &[layer], 1000).unwrap();
        assert_eq!(img.size, [400, 300]);
        // (122.55 E is about 95 cells from the map's west edge)
        let at = |r: usize, c: usize| img.pixels[r * 400 + c];
        assert_eq!((at(10, 20), at(150, 30), at(280, 40)), (red, red, red));
        assert_eq!((at(10, 300), at(150, 200), at(280, 390)), (blue, blue, blue));
    }

    /// A layer turned a quarter under a map of the same system (none told: no GDAL): its pixels found by the full
    /// transform, not skipped.
    #[test]
    fn a_rotated_layer_is_carried_to_the_map() {
        let (red, blue) = (Color32::from_rgb(200, 0, 0), Color32::from_rgb(0, 0, 200));
        let grid = |t: [f64; 6]| {
            let mut g = terraflux_raster::Grid::new(3, 3);
            g.transform = t;
            Arc::new(g)
        };
        let main = Source::Map(Map::new(grid([0.0, 1.0, 0.0, 3.0, 0.0, -1.0]), vec![f64::NAN; 9]));
        // its cell (column c, row r) is at x = r, y = 3 - c: the map's cell (row i, column j) is its (c = i, r = j);
        // the red one is (c = 2, r = 0), so the map's cell of row 2, column 0
        let data = (0..9).map(|k| if k == 2 { 1.0 } else { 2.0 }).collect();
        let layer = crate::map_view::RasterLayer {
            source: Source::Map(Map::new(grid([0.0, 0.0, 1.0, 3.0, -1.0, 0.0]), data)),
            coloring: Coloring::Classes(vec![(1.0, red), (2.0, blue)]),
            opacity: 1.0,
            below: true,
            name: String::new(),
        };
        let coloring = Coloring::Classes(vec![(1.0, Color32::GREEN)]);
        let img = composed(&main, &coloring, &[layer], 100).unwrap();
        assert_eq!(img.size, [3, 3]);
        let want = |i: usize, j: usize| if (i, j) == (2, 0) { red } else { blue };
        for i in 0..3 {
            for j in 0..3 {
                assert_eq!(img.pixels[i * 3 + j], want(i, j), "row {i}, column {j}");
            }
        }
    }

    #[test]
    fn the_speed_chosen_and_a_video() {
        let mut a = Animations::default();
        assert_eq!(a.delay(200), 200, "each animation's own until one is chosen");
        a.fps = Some(10.0);
        assert_eq!((a.delay(200), a.delay(600)), (100, 100));
        a.fps = Some(1000.0);
        assert_eq!(a.delay(200), 40, "25 frames a second at most");
        // a video, nothing installed: two steps of a map of 31 x 17 cells (sides made even)
        let coloring = Coloring::Classes(vec![(1.0, Color32::from_rgb(0, 114, 178))]);
        let frames: Vec<Frame> = (0..2)
            .map(|k| Frame {
                source: map(31, 17, |_| 1.0),
                coloring: coloring.clone(),
                caption: format!("step {k}"),
                rasters: Vec::new(),
            })
            .collect();
        let path = std::env::temp_dir().join(format!("terraflux-anim-{}.mp4", std::process::id()));
        mp4(&frames, 250, MAX_SIDE, &path).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        let _ = std::fs::remove_file(&path);
        assert_eq!(&bytes[4..8], b"ftyp", "an MP4");
        assert!(bytes.windows(4).any(|w| w == b"avc1") && bytes.windows(4).any(|w| w == b"avcC"), "H.264");
        // its frames decoded again: the map's blue where the map is, the sides even
        let (rgba, pw, ph) = picture(&frames[0], MAX_SIDE, 1).unwrap();
        let (_, ew, eh) = even(rgba, pw, ph);
        assert!(ew % 2 == 0 && eh % 2 == 0 && ew >= pw && eh >= ph);
        let decoded = decode(&bytes);
        assert_eq!(decoded.len(), 2);
        let (img, dw, dh) = &decoded[1];
        assert_eq!((*dw, *dh), (ew, eh));
        // (a cell of the map, below the caption: its colour within the video's loss)
        let at = ((ph * 3 / 8) * dw + dw / 2) * 3;
        let near = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 12;
        assert!(near(img[at], 0) && near(img[at + 1], 114) && near(img[at + 2], 178), "{:?}", &img[at..at + 3]);
        // a map of another size refused before anything is encoded
        let mut odd = frames.clone();
        odd[1].source = map(10, 10, |_| 1.0);
        assert!(mp4(&odd, 250, MAX_SIDE, &path).unwrap_err().contains("not of one size"));
    }

    /// The frames of an MP4 written here, decoded (their pixels as RGB, their sides).
    fn decode(mp4: &[u8]) -> Vec<(Vec<u8>, usize, usize)> {
        use openh264::decoder::Decoder;
        use openh264::formats::YUVSource;
        // the parameter sets of avcC, then each frame's units (length-prefixed) back to a stream
        let at = mp4.windows(4).position(|w| w == b"avcC").unwrap() + 4;
        let sps_len = u16::from_be_bytes([mp4[at + 6], mp4[at + 7]]) as usize;
        let sps = &mp4[at + 8..at + 8 + sps_len];
        let p = at + 8 + sps_len + 1;
        let pps_len = u16::from_be_bytes([mp4[p], mp4[p + 1]]) as usize;
        let pps = &mp4[p + 2..p + 2 + pps_len];
        let num = |i: usize| u32::from_be_bytes(mp4[i..i + 4].try_into().unwrap()) as usize;
        let stsz = mp4.windows(4).position(|w| w == b"stsz").unwrap() + 4;
        let stco = mp4.windows(4).position(|w| w == b"stco").unwrap() + 4;
        let n = num(stsz + 8);
        let mut d = Decoder::new().unwrap();
        let mut out = Vec::new();
        let mut head = [&[0, 0, 0, 1][..], sps, &[0, 0, 0, 1], pps].concat();
        for k in 0..n {
            let (size, offset) = (num(stsz + 12 + 4 * k), num(stco + 8 + 4 * k));
            let mut stream = std::mem::take(&mut head);
            let mut i = offset;
            while i < offset + size {
                let len = num(i);
                stream.extend_from_slice(&[0, 0, 0, 1]);
                stream.extend_from_slice(&mp4[i + 4..i + 4 + len]);
                i += 4 + len;
            }
            if let Some(yuv) = d.decode(&stream).unwrap() {
                let (w, h) = yuv.dimensions();
                let mut rgb = vec![0u8; w * h * 3];
                yuv.write_rgb8(&mut rgb);
                out.push((rgb, w, h));
            }
        }
        out
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
                rasters: Vec::new(),
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
        let frames = vec![Frame {
            source: map(60, 20, |i| (i % 60) as f64 / 24.0),
            coloring,
            caption: "elevation".into(),
            rasters: Vec::new(),
        }];
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
            move || vec![Frame { source: map(8, 8, |_| 1.0), coloring: c, caption: "one".into(), rasters: Vec::new() }],
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
