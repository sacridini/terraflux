//! The document of a model written as a report ([`crate::document`]): an HTML file (one file, its images in it,
//! read without Terraflux) or a PDF of the same content. The values of the last run give the images (maps, the
//! last step of a series, the first layer of a stack) and the tables' first rows; a step not run says so.

use std::fmt::Write as _;

use terraflux_ops::Value;

use crate::animation::Frame;
use crate::document::{Body, Document, From, Section, Size, Stage, Step, data_rows, row_settings};
use crate::map_view::Source;
use crate::palette::{Coloring, image, image_f32};

/// The largest side of the images of a report, pixels: a figure's, a step's.
const FIGURE: usize = 640;
const STEP: usize = 320;

/// An image: its pixels, RGBA premultiplied, row by row.
#[derive(Debug, Clone)]
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// What a value is shown as.
#[derive(Debug, Clone)]
pub enum Shown {
    Picture(Picture),
    /// A table's first rows.
    Table {
        columns: Vec<String>,
        rows: Vec<Vec<String>>,
        more: usize,
    },
    Text(String),
}

/// A value shown, and what of it (the step of a series, the layer of a stack).
#[derive(Debug, Clone)]
pub struct Visual {
    pub shown: Shown,
    pub of: Option<String>,
}

/// The rows of a table shown, at most.
pub const ROWS: usize = 6;
/// Its columns, at most.
const COLUMNS: usize = 8;

fn picture(values: &[f64], width: usize, height: usize, side: usize) -> Picture {
    let coloring = Coloring::for_values(values);
    let (img, _) = image(values, width, height, side, &coloring);
    let rgba = img.pixels.iter().flat_map(|c| c.to_array()).collect();
    Picture { width: img.size[0], height: img.size[1], rgba }
}

/// A number in a table: whole numbers as they are, others with four significant digits or so.
pub fn cell(x: f64) -> String {
    if x.is_nan() {
        "–".into()
    } else if x.fract() == 0.0 && x.abs() < 1e15 {
        format!("{x:.0}")
    } else if x.abs() >= 1e7 || x.abs() < 0.001 {
        format!("{x:.4e}")
    } else {
        // four significant digits or so, not fewer than one decimal
        let decimals = if x.abs() >= 1000.0 { 1 } else { 4 };
        format!("{x:.decimals$}").trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// What a value is shown as, its images at most `side` pixels on a side: a map, rasters on disk (sampled), a
/// stack's first layer, a series' last value; a table's first rows; else a line of words.
pub fn visual(v: &Value, side: usize) -> Option<Visual> {
    match v {
        Value::Map(m) => {
            Some(Visual { shown: Shown::Picture(picture(&m.data, m.grid.width, m.grid.height, side)), of: None })
        }
        Value::Stack(s) => {
            let first = s.layers.first()?;
            let key = s.keys.first().map(|k| k.to_string()).unwrap_or_default();
            Some(Visual {
                shown: Shown::Picture(picture(first, s.grid.width, s.grid.height, side)),
                of: (s.layers.len() > 1).then(|| format!("layer 1 of {}: {key}", s.layers.len())),
            })
        }
        Value::Raster(r) => {
            let (w, h) = (r.grid.width, r.grid.height);
            let step = w.max(h).div_ceil(side.max(1)).max(1);
            let win = terraflux_raster::tiff::Window { x0: 0, y0: 0, width: w, height: h };
            let cells = r.sample(r.chosen, win, step).ok()?;
            let (sw, sh) = (w.div_ceil(step), h.div_ceil(step));
            if cells.len() < sw * sh {
                return None;
            }
            let values: Vec<f64> = cells.iter().map(|&x| f64::from(x)).collect();
            let img = image_f32(&cells, sw, sh, &Coloring::for_values(&values));
            let rgba = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            let n = r.layers.len();
            Some(Visual {
                shown: Shown::Picture(Picture { width: sw, height: sh, rgba }),
                of: (n > 1).then(|| format!("layer {} of {n}", r.chosen + 1)),
            })
        }
        Value::Series(items) => {
            let last = items.last()?;
            let mut out = visual(last, side)?;
            out.of = Some(match out.of {
                Some(o) => format!("step {} of {}, {o}", items.len(), items.len()),
                None => format!("step {} of {}", items.len(), items.len()),
            });
            Some(out)
        }
        Value::Table(t) => {
            let columns: Vec<String> = t.columns.iter().take(COLUMNS).cloned().collect();
            let rows = t.rows.iter().take(ROWS).map(|r| r.iter().take(COLUMNS).map(|&x| cell(x)).collect()).collect();
            Some(Visual {
                shown: Shown::Table { columns, rows, more: t.rows.len().saturating_sub(ROWS) },
                of: (t.columns.len() > COLUMNS).then(|| format!("{} of {} columns", COLUMNS, t.columns.len())),
            })
        }
        other => Some(Visual {
            shown: Shown::Text(crate::tools::summarise(other).lines().next().unwrap_or_default().to_string()),
            of: None,
        }),
    }
}

/// The most steps the animation of a series shows (a longer series every few steps, its first and last always),
/// and the steps of one a PDF lays in a row.
const FRAMES: usize = 40;
const STRIP: usize = 4;

/// `most` of `n` steps, evenly spaced, the first and the last among them.
fn picks(n: usize, most: usize) -> Vec<usize> {
    if n <= most { (0..n).collect() } else { (0..most).map(|k| k * (n - 1) / (most - 1).max(1)).collect() }
}

/// The maps of a series (a loop's steps) to animate or lay in a row: `most` of its steps, in one colouring (a
/// value the same colour at every step), each with its step written; none unless a series of two maps of one
/// size or more.
fn series_frames(v: &Value, most: usize) -> Option<Vec<Frame>> {
    let Value::Series(items) = v else { return None };
    let n = items.len();
    let steps: Vec<(usize, Source)> =
        picks(n, most).into_iter().filter_map(|i| Some((i, crate::builder::first_source(&items[i])?))).collect();
    let size = steps.first()?.1.size();
    if steps.len() < 2 || steps.iter().any(|(_, s)| s.size() != size) {
        return None;
    }
    let coloring = steps
        .iter()
        .filter_map(|(_, s)| crate::animation::values(s, 256).ok())
        .map(|(v, _, _)| Coloring::for_values(&v.iter().map(|&x| f64::from(x)).collect::<Vec<_>>()))
        .reduce(|a, b| a.shared(&b))?;
    let frame = |(i, source): (usize, Source)| Frame {
        source,
        coloring: coloring.clone(),
        caption: format!("step {} of {n}", i + 1),
    };
    Some(steps.into_iter().map(frame).collect())
}

/// A frame as a picture at most `side` pixels on a side, in its colouring.
fn frame_picture(f: &Frame, side: usize) -> Option<Picture> {
    let (v, w, h) = crate::animation::values(&f.source, side).ok()?;
    let img = image_f32(&v, w, h, &f.coloring);
    Some(Picture { width: w, height: h, rgba: img.pixels.iter().flat_map(|c| c.to_array()).collect() })
}

/// A plot's comparison as pictures: the map shown and the one compared, in the colours they share, each with its
/// name; and their difference when asked: its picture, what it says, and its key (a colour and words each).
struct Compared {
    pics: Vec<(Picture, String)>,
    said: Option<String>,
    key: Vec<([u8; 3], String)>,
}

/// The last map of a value (a series' last step) as a source.
fn last_source(v: &Value) -> Option<Source> {
    match v {
        Value::Series(items) => items.iter().rev().find_map(crate::builder::first_source),
        v => crate::builder::first_source(v),
    }
}

/// Two values compared (`names` theirs), pictures at most `side` pixels on a side; their difference too when
/// `difference` (between maps of classes, the transitions).
fn compared(a: &Value, b: &Value, names: [String; 2], difference: bool, side: usize) -> Option<Compared> {
    let (sa, sb) = (last_source(a)?, last_source(b)?);
    let colours = |s: &Source| {
        let (v, _, _) = crate::animation::values(s, 256).ok()?;
        Some(Coloring::for_values(&v.iter().map(|&x| f64::from(x)).collect::<Vec<_>>()))
    };
    let (ca, cb) = (colours(&sa)?, colours(&sb)?);
    let shared = ca.shared(&cb);
    let picture = |source: &Source, coloring: &Coloring| {
        frame_picture(&Frame { source: source.clone(), coloring: coloring.clone(), caption: String::new() }, side)
    };
    let [na, nb] = names;
    let mut pics = vec![(picture(&sa, &shared)?, na), (picture(&sb, &shared)?, nb)];
    let (mut said, mut key) = (None, Vec::new());
    if difference {
        let classes = matches!((&ca, &cb), (Coloring::Classes(_), Coloring::Classes(_)));
        let none = crate::difference::Names::new();
        match crate::difference::make(&sa, &sb, classes.then_some((&none, &none))) {
            Ok(d) => {
                pics.push((picture(&d.source, &d.coloring)?, "difference".into()));
                said = Some(d.summary.clone());
                let rgb = |c: egui::Color32| [c.r(), c.g(), c.b()];
                key = match &d.coloring {
                    Coloring::Classes(c) => c
                        .iter()
                        .map(|(v, colour)| {
                            let n = d.counts.iter().find(|(k, _)| k == v).map_or(0, |(_, n)| *n);
                            (rgb(*colour), format!("{}: {n} cells", d.name(*v)))
                        })
                        .collect(),
                    Coloring::Continuous { min, max, ramp, .. } => vec![
                        (rgb(ramp.colour(0.0)), format!("lower, to {}", cell(*min))),
                        (rgb(ramp.colour(0.5)), "the same".into()),
                        (rgb(ramp.colour(1.0)), format!("higher, to {}", cell(*max))),
                    ],
                };
            }
            Err(e) => said = Some(e),
        }
    }
    Some(Compared { pics, said, key })
}

/// The values of a run, by node and port.
pub type Values<'a> = &'a dyn Fn(&str, &str) -> Option<Value>;

/// What a report adds to the model's document: its methods and the papers to cite, and how its checks fared in
/// the run.
#[derive(Default)]
pub struct Extras {
    pub methods: Option<crate::methods::Methods>,
    pub checks: Vec<crate::checks::Check>,
}

impl Extras {
    /// Of a model (`file` its file's name, if it has one) and the checks of its run, for this Terraflux.
    pub fn of(
        m: &terraflux_graph::Model,
        r: &terraflux_ops::Registry,
        file: Option<&str>,
        checks: Vec<crate::checks::Check>,
    ) -> Extras {
        Extras { methods: Some(crate::methods::of(m, r, env!("CARGO_PKG_VERSION"), file)), checks }
    }

    /// The badge of a step of a scope (`path`): a check's verdict, a container's worst of the checks it holds.
    fn badge(&self, path: &[String], id: &str) -> Option<(crate::checks::Verdict, bool)> {
        let v = crate::checks::badges(&self.checks, path).remove(id)?;
        let own = self.checks.iter().any(|c| c.path == path && c.id == id);
        Some((v, own))
    }

    /// The references cited, in the APA style and in alphabetical order, Terraflux's among them.
    fn references(&self) -> Vec<String> {
        let Some(m) = &self.methods else { return Vec::new() };
        let mut refs: Vec<String> = m.references.iter().map(|r| r.apa()).collect();
        refs.push(m.software.clone());
        refs.sort_by_key(|r| r.to_lowercase());
        refs
    }
}

/// What a badge says beside a step: a check's verdict in words (not again the badge's word), a container's worst.
fn badge_words(v: &crate::checks::Verdict, own: bool) -> String {
    use crate::checks::Verdict;
    match (v, own) {
        (_, false) => format!("the worst of the checks inside: {}", v.words()),
        (Verdict::Passed, true) => "it held in the run".into(),
        (Verdict::NotRun, true) => "the run did not come to it".into(),
        (Verdict::Off, true) => "turned off (bypassed)".into(),
        (_, true) => v.words(),
    }
}

/// The numbers of the sections and steps: "2", "2.3", "2.3.1".
fn number(prefix: &str, k: usize) -> String {
    if prefix.is_empty() { format!("{}", k + 1) } else { format!("{prefix}.{}", k + 1) }
}

/// Today's date (UTC), as 2026-10-18.
pub fn today() -> String {
    let days =
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() / 86_400) as i64;
    // (Howard Hinnant's civil_from_days)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------------------------------------------
// HTML

fn esc(t: &str) -> String {
    let mut s = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '&' => s.push_str("&amp;"),
            '<' => s.push_str("&lt;"),
            '>' => s.push_str("&gt;"),
            '"' => s.push_str("&quot;"),
            _ => s.push(c),
        }
    }
    s
}

pub(crate) fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n =
            (u32::from(c[0]) << 16) | (u32::from(*c.get(1).unwrap_or(&0)) << 8) | u32::from(*c.get(2).unwrap_or(&0));
        for k in 0..4 {
            if k <= c.len() {
                s.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

/// A picture as a PNG (none when it cannot be made).
pub fn png(p: &Picture) -> Option<Vec<u8>> {
    let size = tiny_skia::IntSize::from_wh(p.width as u32, p.height as u32)?;
    tiny_skia::Pixmap::from_vec(p.rgba.clone(), size)?.encode_png().ok()
}

const CSS: &str = r#"
:root { --bg: #ffffff; --fg: #1d2125; --weak: #5f6870; --line: #dde1e5; --well: #f4f6f8; --accent: #1f7a5a; --code: #eef1f4; }
@media (prefers-color-scheme: dark) {
  :root { --bg: #16191c; --fg: #e3e6e9; --weak: #9aa3ab; --line: #30363c; --well: #1e2226; --accent: #5cc49a; --code: #23282d; }
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--bg); color: var(--fg); font: 15px/1.55 system-ui, -apple-system, "Segoe UI", Roboto, sans-serif; }
main { max-width: 860px; margin: 0 auto; padding: 32px 16px 64px; }
h1 { font-size: 1.7rem; margin: 0 0 4px; line-height: 1.25; }
h2 { font-size: 1.25rem; margin: 36px 0 10px; padding-bottom: 4px; border-bottom: 1px solid var(--line); }
h3 { font-size: 1rem; margin: 0 0 4px; }
.lead { color: var(--weak); margin: 0 0 16px; font-style: italic; }
.meta { margin: 0 0 6px; }
.meta b { font-weight: 600; }
.step { background: var(--well); border-radius: 6px; padding: 12px 14px; margin: 10px 0; }
.step .body { border-left: 2px solid var(--line); margin: 10px 0 0 4px; padding-left: 12px; }
.step .body .step { background: transparent; padding: 6px 0; margin: 4px 0; }
.op { color: var(--weak); font-weight: normal; font-family: ui-monospace, Consolas, monospace; font-size: 0.85em; }
.note { margin: 2px 0 6px; }
.missing { color: var(--weak); font-style: italic; font-size: 0.9em; }
.line { margin: 6px 0; }
ul { margin: 4px 0; padding-left: 20px; }
code, pre { font-family: ui-monospace, Consolas, monospace; font-size: 0.88em; background: var(--code); border-radius: 4px; }
code { padding: 1px 4px; }
pre { padding: 6px 8px; white-space: pre-wrap; margin: 6px 0; }
.default { color: var(--weak); }
table { border-collapse: collapse; margin: 8px 0; font-size: 0.9em; }
th, td { border-bottom: 1px solid var(--line); padding: 4px 8px; text-align: left; vertical-align: top; }
th { font-weight: 600; }
td.num { text-align: right; font-variant-numeric: tabular-nums; }
figure { margin: 12px 0; }
figure img, .thumb img { image-rendering: pixelated; border: 1px solid var(--line); max-width: 100%; height: auto; }
img.anim { image-rendering: auto; }
.pair { display: flex; flex-wrap: wrap; gap: 10px; }
.key { list-style: none; padding-left: 0; font-size: 0.85em; margin: 6px 0; }
.sw { display: inline-block; width: 10px; height: 10px; margin-right: 6px; border: 1px solid var(--line); }
figcaption, .of { color: var(--weak); font-size: 0.88em; }
.thumb { margin: 8px 0 2px; }
.badge { display: inline-block; color: #fff; border-radius: 9px; padding: 0 8px; font-size: 0.8em; font-weight: 600; margin-right: 6px; }
.checks { margin: 4px 0 8px; padding-left: 0; list-style: none; }
.checks li { margin: 3px 0; }
.refs li { margin: 4px 0; }
footer { margin-top: 48px; color: var(--weak); font-size: 0.85em; }
a { color: var(--accent); text-decoration: none; }
@media print { body { background: #fff; color: #000; } .step { break-inside: avoid; } }
"#;

/// The HTML of a value shown (an image at most `side` pixels wide on the page).
fn html_visual(v: &Visual, alt: &str, width: usize) -> String {
    let mut s = String::new();
    match &v.shown {
        Shown::Picture(p) => {
            if let Some(png) = png(p) {
                let w = width.min(p.width.max(1) * (width / p.width.max(1)).max(1));
                let _ =
                    write!(s, r#"<img alt="{}" width="{w}" src="data:image/png;base64,{}">"#, esc(alt), base64(&png));
            }
        }
        Shown::Table { columns, rows, more } => {
            s.push_str("<table><tr>");
            for c in columns {
                let _ = write!(s, "<th>{}</th>", esc(c));
            }
            s.push_str("</tr>");
            for r in rows {
                s.push_str("<tr>");
                for c in r {
                    let _ = write!(s, r#"<td class="num">{}</td>"#, esc(c));
                }
                s.push_str("</tr>");
            }
            s.push_str("</table>");
            if *more > 0 {
                let _ = write!(s, r#"<div class="of">and {more} more rows</div>"#);
            }
        }
        Shown::Text(t) => {
            let _ = write!(s, r#"<div class="of">{}</div>"#, esc(t));
        }
    }
    if let Some(of) = &v.of {
        let _ = write!(s, r#"<div class="of">{}</div>"#, esc(of));
    }
    s
}

/// A series of maps as an animated GIF (the step written on each, the legend under them), `width` pixels wide on
/// the page, and how many steps it shows.
fn html_animation(v: &Value, alt: &str, width: usize) -> Option<String> {
    let Value::Series(items) = v else { return None };
    let frames = series_frames(v, FRAMES)?;
    let gif = crate::animation::gif(&frames, 250, width).ok()?;
    let n = items.len();
    let shown = if frames.len() < n {
        format!("the {n} steps, {} of them animated", frames.len())
    } else {
        format!("the {n} steps, animated")
    };
    Some(format!(
        r#"<img class="anim" alt="{}" width="{width}" src="data:image/gif;base64,{}"><div class="of">{shown}</div>"#,
        esc(alt),
        base64(&gif)
    ))
}

/// The comparison a figure asks for (its plot's `compared`), from the values of the run; none without one.
fn figure_compared(
    values: Values,
    first: &dyn Fn(&str) -> Option<String>,
    f: &crate::document::Figure,
) -> Option<Compared> {
    let (n, p, how) = f.compared.as_ref()?;
    let (a, b) = (values(&f.node, &f.port)?, values(n, p)?);
    let name = |node: &str, port: &str| From::Node { node: node.into(), port: port.into() }.text(first);
    compared(&a, &b, [name(&f.node, &f.port), name(n, p)], how == "difference", FIGURE / 2)
}

/// A comparison on the page: the maps in a row, each named under it, what their difference says and its key.
fn html_compared(c: &Compared) -> String {
    let width = if c.pics.len() > 2 { 260 } else { 340 };
    let mut s = String::from(r#"<div class="pair">"#);
    for (p, name) in &c.pics {
        let img = html_visual(&Visual { shown: Shown::Picture(p.clone()), of: None }, name, width);
        let _ = write!(s, r#"<div>{img}<div class="of">{}</div></div>"#, esc(name));
    }
    s.push_str("</div>");
    if let Some(t) = &c.said {
        let _ = write!(s, r#"<div class="of">{}</div>"#, esc(t));
    }
    if !c.key.is_empty() {
        s.push_str(r#"<ul class="key">"#);
        for ([r, g, b], words) in &c.key {
            let _ =
                write!(s, r#"<li><span class="sw" style="background: rgb({r}, {g}, {b})"></span>{}</li>"#, esc(words));
        }
        s.push_str("</ul>");
    }
    s
}

/// A value on the page: a series of maps animated, else as [`visual`] shows it; none when there is none.
fn html_value(v: Option<Value>, alt: &str, side: usize, width: usize) -> Option<String> {
    let v = v?;
    html_animation(&v, alt, width).or_else(|| visual(&v, side).map(|x| html_visual(&x, alt, width)))
}

struct Html<'a> {
    out: String,
    values: Values<'a>,
    extras: &'a Extras,
    /// the containers the steps written are in
    path: Vec<String>,
    /// the only output of a node, to name the node alone when it is what is taken
    first: &'a dyn Fn(&str) -> Option<String>,
}

impl Html<'_> {
    fn from(&self, f: &From) -> String {
        let t = esc(&f.text(self.first));
        match f.node() {
            Some(n) => format!(r##"<a href="#{}"><b>{t}</b></a>"##, esc(n)),
            None => format!("<b>{t}</b>"),
        }
    }

    fn reference(&self, r: &str) -> String {
        match terraflux_graph::Ref::parse(r) {
            Ok(terraflux_graph::Ref::Port { node, port }) => self.from(&From::Node { node, port }),
            _ => format!("<b>{}</b>", esc(r)),
        }
    }

    fn list(&mut self, title: &str, items: &[String]) {
        if items.is_empty() {
            return;
        }
        let _ = write!(self.out, "<div><b>{title}.</b><ul>");
        for i in items {
            let _ = write!(self.out, "<li>{}</li>", esc(i));
        }
        self.out.push_str("</ul></div>");
    }

    fn step(&mut self, s: &Step, num: &str) {
        let anchor = if s.value.is_some() { format!(r#" id="{}""#, esc(&s.id)) } else { String::new() };
        if s.size == Size::Line {
            let _ = write!(
                self.out,
                r#"<div class="line"{anchor}><b>{num} {}</b> <span class="op">{}</span>"#,
                esc(&s.id),
                esc(&s.op)
            );
            let of: Vec<String> = s.inputs.iter().map(|l| self.from(&l.from)).collect();
            if !of.is_empty() {
                let _ = write!(self.out, " of {}", of.join(", "));
            }
            if let Some(n) = &s.note {
                let _ = write!(self.out, " — <i>{}</i>", esc(n));
            }
            if let Some((v, own)) = self.extras.badge(&self.path, &s.id) {
                let _ = write!(self.out, " {}", html_badge(&v, &badge_words(&v, own)));
            }
            self.out.push_str("</div>");
            return;
        }
        let _ = write!(
            self.out,
            r#"<div class="step"{anchor}><h3>{num} {} <span class="op">{}</span></h3>"#,
            esc(&s.id),
            esc(&s.op)
        );
        if let Some((v, own)) = self.extras.badge(&self.path, &s.id) {
            let words = badge_words(&v, own);
            let _ = write!(self.out, "<p>{}{}</p>", html_badge(&v, &words), esc(&words));
        }
        match &s.note {
            Some(n) => {
                let _ = write!(self.out, r#"<p class="note"><i>{}</i></p>"#, esc(n));
            }
            None => self.out.push_str(r#"<p class="missing">No note.</p>"#),
        }
        if let Some(w) = &s.why {
            let _ = write!(self.out, "<p><b>Why.</b> {}</p>", esc(w));
        }
        if let Some(t) = &s.sentence {
            let _ = write!(self.out, "<p>{}</p>", esc(t));
        }
        if let Some(f) = &s.file {
            let _ = write!(self.out, "<p>File <code>{}</code></p>", esc(f));
        }
        if let Some(e) = &s.expression {
            let _ = write!(self.out, "<pre>{}</pre>", esc(e));
            let vars: Vec<String> =
                s.inputs.iter().map(|l| format!("<code>{}</code> = {}", esc(&l.port), self.from(&l.from))).collect();
            if !vars.is_empty() {
                let _ = write!(self.out, "<p>where {}</p>", vars.join(", "));
            }
        } else if !s.inputs.is_empty() {
            let of: Vec<String> = s
                .inputs
                .iter()
                .map(|l| {
                    format!(
                        r#"<span title="{}">{}</span> {}"#,
                        esc(&l.doc),
                        esc(&l.port.replace('_', " ")),
                        self.from(&l.from)
                    )
                })
                .collect();
            let _ = write!(self.out, "<p>Takes {}.</p>", of.join("; "));
        }
        if !s.settings.is_empty() {
            self.out.push_str("<ul>");
            for st in &s.settings {
                let default = st
                    .default
                    .as_ref()
                    .map(|d| format!(r#" <span class="default">(default {})</span>"#, esc(d)))
                    .unwrap_or_default();
                let _ = write!(
                    self.out,
                    r#"<li><span title="{}">{}</span>: <b>{}</b>{default}</li>"#,
                    esc(&format!("{}: {}", st.name, st.doc)),
                    esc(&st.label),
                    esc(&st.value)
                );
            }
            self.out.push_str("</ul>");
        }
        if let Some(b) = &s.body {
            self.path.push(s.id.clone());
            self.body(b, num);
            self.path.pop();
        }
        self.list("References", &s.refs);
        self.list("Assumptions", &s.assumptions);
        self.list("Alternatives", &s.alternatives);
        if let Some((port, _)) = &s.shows
            && let Some((node, key)) = s.value_key(port)
        {
            let side = if matches!(s.stage, Stage::Data | Stage::Preparation) { STEP / 2 } else { STEP };
            match html_value((self.values)(&node, &key), &format!("{}.{port}", s.id), side, side) {
                Some(h) => {
                    let _ = write!(self.out, r#"<div class="thumb">{h}</div>"#);
                }
                None => self.out.push_str(r#"<p class="missing">Not run.</p>"#),
            }
        }
        for f in &s.figures {
            self.figure(f);
        }
        self.out.push_str("</div>");
    }

    fn figure(&mut self, f: &crate::document::Figure) {
        let img = match figure_compared(self.values, self.first, f) {
            Some(c) => html_compared(&c),
            None => html_value((self.values)(&f.node, &f.port), &f.caption, FIGURE, FIGURE * 3 / 4)
                .unwrap_or_else(|| r#"<p class="missing">Not run.</p>"#.to_string()),
        };
        let with = f
            .compared
            .as_ref()
            .map(|(n, p, _)| format!(", compared with {}", self.from(&From::Node { node: n.clone(), port: p.clone() })))
            .unwrap_or_default();
        let _ = write!(
            self.out,
            "<figure>{img}<figcaption><b>Figure {}.</b> {} (from {}{with})</figcaption></figure>",
            f.number,
            esc(&f.caption),
            self.from(&From::Node { node: f.node.clone(), port: f.port.clone() })
        );
    }

    fn body(&mut self, b: &Body, num: &str) {
        let mut head = Vec::new();
        if let Some(t) = &b.times {
            head.push(format!("Repeated {} times.", self.from(t)));
        }
        if !b.carry.is_empty() {
            let c: Vec<String> = b
                .carry
                .iter()
                .map(|(k, start, each)| {
                    let start = start.as_ref().map(|f| format!(" from {}", self.from(f))).unwrap_or_default();
                    format!("<b>{}</b>{start}, then {}", esc(k), self.reference(each))
                })
                .collect();
            head.push(format!("Carries {}.", c.join("; ")));
        }
        if !b.collect.is_empty() {
            let c: Vec<String> =
                b.collect.iter().map(|(k, r)| format!("<b>{}</b> ({})", esc(k), self.reference(r))).collect();
            head.push(format!("Collects {}.", c.join(", ")));
        }
        if !b.outputs.is_empty() {
            let c: Vec<String> =
                b.outputs.iter().map(|(k, r)| format!("<b>{}</b> ({})", esc(k), self.reference(r))).collect();
            head.push(format!("Gives {}.", c.join(", ")));
        }
        if let Some(u) = &b.until {
            head.push(format!("Stops when {} is true.", self.reference(u)));
        }
        if !head.is_empty() {
            let _ = write!(self.out, "<p>{}</p>", head.join(" "));
        }
        self.out.push_str(r#"<div class="body">"#);
        for (k, s) in b.steps.iter().enumerate() {
            self.step(s, &number(num, k));
        }
        self.out.push_str("</div>");
    }

    fn data(&mut self, sec: &Section, num: &str) {
        self.out.push_str("<table><tr><th>Step</th><th>What it is</th><th></th></tr>");
        for (k, row) in data_rows(&sec.steps).iter().enumerate() {
            let ids: Vec<String> = row.iter().map(|s| format!(r#"<b id="{0}">{0}</b>"#, esc(&s.id))).collect();
            let _ = write!(
                self.out,
                r#"<tr><td>{} {} <span class="op">{}</span></td><td>"#,
                number(num, k),
                ids.join(", "),
                esc(&row[0].op)
            );
            if let Some(n) = &row[0].note {
                let _ = write!(self.out, "<i>{}</i><br>", esc(n));
            }
            if let (1, Some(t)) = (row.len(), &row[0].sentence) {
                let _ = write!(self.out, "{}<br>", esc(t));
            }
            let files: Vec<String> =
                row.iter().filter_map(|s| s.file.as_ref()).map(|f| format!("<code>{}</code>", esc(f))).collect();
            if !files.is_empty() {
                let _ = write!(self.out, "{}<br>", files.join(", "));
            }
            let set: Vec<String> =
                row_settings(row).iter().map(|(l, v)| format!("{}: <b>{}</b>", esc(l), esc(v))).collect();
            self.out.push_str(&set.join("; "));
            if let Some(w) = &row[0].why {
                let _ = write!(self.out, "<br><b>Why.</b> {}", esc(w));
            }
            self.out.push_str("</td><td>");
            for s in row.iter().take(4) {
                if let Some((port, _)) = &s.shows
                    && let Some((node, key)) = s.value_key(port)
                    && let Some(v) = (self.values)(&node, &key).and_then(|v| visual(&v, 96))
                    && matches!(v.shown, Shown::Picture(_))
                {
                    self.out.push_str(&html_visual(&v, &s.id, 72));
                }
            }
            self.out.push_str("</td></tr>");
            for s in row.iter() {
                for f in &s.figures {
                    self.out.push_str("<tr><td colspan=3>");
                    self.figure(f);
                    self.out.push_str("</td></tr>");
                }
            }
        }
        self.out.push_str("</table>");
    }
}

/// A check's verdict as a badge, in its colour, what it says shown over it.
fn html_badge(v: &crate::checks::Verdict, title: &str) -> String {
    let [r, g, b] = v.rgb();
    format!(r#"<span class="badge" style="background: rgb({r}, {g}, {b})" title="{}">{}</span>"#, esc(title), v.label())
}

/// The report as an HTML page.
pub fn html(doc: &Document, values: Values, first: &dyn Fn(&str) -> Option<String>, extras: &Extras) -> String {
    let mut h = Html { out: String::new(), values, extras, path: Vec::new(), first };
    let _ = write!(
        h.out,
        r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>{}</title><style>{CSS}</style></head><body><main>"#,
        esc(&doc.title)
    );
    let _ = write!(h.out, "<h1>{}</h1>", esc(&doc.title));
    if let Some(d) = &doc.description {
        let _ = write!(h.out, r#"<p class="lead">{}</p>"#, esc(d));
    }
    for (label, v) in [("Request", &doc.request), ("Author", &doc.author), ("Summary", &doc.summary)] {
        if let Some(v) = v {
            let _ = write!(h.out, r#"<p class="meta"><b>{label}.</b> {}</p>"#, esc(v));
        }
    }
    // the model's health: how its checks fared, each linked to its step
    if let Some(health) = crate::checks::health(&extras.checks) {
        let _ = write!(h.out, r#"<p class="meta"><b>Checks.</b> {}</p><ul class="checks">"#, esc(&health));
        for c in &extras.checks {
            let at = c.path.iter().chain(std::iter::once(&c.id)).cloned().collect::<Vec<_>>().join("/");
            let _ = write!(
                h.out,
                r##"<li>{}<a href="#{}"><b>{}</b></a> <code>{}</code>: {}</li>"##,
                html_badge(&c.verdict, &c.verdict.words()),
                esc(c.path.first().unwrap_or(&c.id)),
                esc(&at),
                esc(&c.expression),
                esc(&c.verdict.words())
            );
        }
        h.out.push_str("</ul>");
    }
    let mut k = 0;
    if !doc.inputs.is_empty() {
        k += 1;
        let _ =
            write!(h.out, "<h2>{k}. Inputs</h2><table><tr><th>Input</th><th>Kind</th><th>Value</th><th>Note</th></tr>");
        for i in &doc.inputs {
            let _ = write!(
                h.out,
                "<tr><td><b>${}</b></td><td>{}</td><td><code>{}</code></td><td>{}</td></tr>",
                esc(&i.name),
                i.kind,
                esc(&i.value),
                esc(i.note.as_deref().unwrap_or(""))
            );
        }
        h.out.push_str("</table>");
    }
    if !doc.parameters.is_empty() {
        k += 1;
        let _ = write!(
            h.out,
            "<h2>{k}. Parameters</h2><table><tr><th>Parameter</th><th>Value</th><th>Range</th><th>What it is</th></tr>"
        );
        for p in &doc.parameters {
            let unit = p.unit.as_ref().map(|u| format!(" {}", esc(u))).unwrap_or_default();
            let _ = write!(
                h.out,
                r#"<tr><td><b title="{}">{}</b></td><td>{}{unit}</td><td>{}</td><td>{}</td></tr>"#,
                esc(&p.key),
                esc(&p.label),
                esc(&p.value),
                esc(p.range.as_deref().unwrap_or("")),
                esc(p.help.as_deref().unwrap_or(""))
            );
        }
        h.out.push_str("</table>");
    }
    if !doc.scenarios.is_empty() {
        k += 1;
        let _ = write!(h.out, "<h2>{k}. Scenarios</h2><table><tr><th>Scenario</th><th>Values</th><th>Note</th></tr>");
        for sc in &doc.scenarios {
            let v: Vec<String> = sc.values.iter().map(|(l, v)| format!("{} <b>{}</b>", esc(l), esc(v))).collect();
            let _ = write!(
                h.out,
                "<tr><td><b>{}</b></td><td>{}</td><td><i>{}</i></td></tr>",
                esc(&sc.name),
                v.join("; "),
                esc(sc.note.as_deref().unwrap_or(""))
            );
        }
        h.out.push_str("</table>");
    }
    for sec in &doc.sections {
        k += 1;
        let num = k.to_string();
        let _ = write!(h.out, "<h2>{num}. {}</h2>", esc(&sec.title));
        if let Some(n) = &sec.note {
            let _ = write!(h.out, r#"<p class="lead">{}</p>"#, esc(n));
        }
        if sec.stage == Stage::Data {
            h.data(sec, &num);
        } else {
            for (j, s) in sec.steps.iter().enumerate() {
                h.step(s, &number(&num, j));
            }
        }
        if sec.stage == Stage::Results {
            for f in &doc.figures {
                h.figure(f);
            }
        }
    }
    if !doc.figures.is_empty() && !doc.sections.iter().any(|s| s.stage == Stage::Results) {
        k += 1;
        let _ = write!(h.out, "<h2>{k}. Results</h2>");
        for f in &doc.figures {
            h.figure(f);
        }
    }
    if !doc.outputs.is_empty() {
        k += 1;
        let _ = write!(h.out, "<h2>{k}. Outputs</h2><table><tr><th>Output</th><th>From</th><th>File</th></tr>");
        for o in &doc.outputs {
            let file = if o.file.is_empty() { String::new() } else { format!("<code>{}</code>", esc(&o.file)) };
            let _ = write!(
                h.out,
                "<tr><td><b>{}</b></td><td>{}</td><td>{file}</td></tr>",
                esc(&o.name),
                h.reference(&o.from)
            );
        }
        h.out.push_str("</table>");
    }
    // the methods as a paper tells them, and the papers to cite
    if let Some(m) = &extras.methods {
        k += 1;
        let _ = write!(h.out, "<h2>{k}. Methods and citations</h2><p>{}</p>", esc(&m.paragraph));
        h.out.push_str(r#"<h3>References</h3><ul class="refs">"#);
        for r in extras.references() {
            let _ = write!(h.out, "<li>{}</li>", esc(&r));
        }
        h.out.push_str("</ul>");
        if !m.own.is_empty() {
            h.out.push_str(r#"<h3>Given in the model's notes</h3><ul class="refs">"#);
            for r in &m.own {
                let _ = write!(h.out, "<li>{}</li>", esc(r));
            }
            h.out.push_str("</ul>");
        }
    }
    let _ = write!(
        h.out,
        "<footer>Made by Terraflux {} on {} from the model's graph: every step is a node, in the order of what it \
         depends on.</footer></main></body></html>",
        env!("CARGO_PKG_VERSION"),
        today()
    );
    h.out
}

// ---------------------------------------------------------------------------------------------------------------
// PDF

/// The widths of Helvetica's and Helvetica-Bold's characters 32 to 126, thousandths of the font's size (the
/// Adobe metrics of the standard fonts).
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556, 833,
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556,
    556, 278, 556, 556, 222, 222, 500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334,
    260, 334, 584,
];
const HELVETICA_BOLD: [u16; 95] = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556, 556, 556, 556, 556,
    556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667, 611, 778, 722, 278, 556, 722, 611, 833,
    722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611,
    556, 333, 611, 611, 278, 278, 556, 278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389,
    280, 389, 584,
];

#[derive(Clone, Copy, PartialEq)]
enum Font {
    Regular,
    Bold,
    Italic,
    Mono,
}

impl Font {
    fn name(self) -> &'static str {
        match self {
            Font::Regular => "F1",
            Font::Bold => "F2",
            Font::Italic => "F3",
            Font::Mono => "F4",
        }
    }
}

/// A character in the PDF's encoding (WinAnsi: Latin-1 and a few more); others as near as they come.
fn ansi(c: char) -> Vec<u8> {
    match c {
        '\u{20}'..='\u{7e}' => vec![c as u8],
        '\u{a0}'..='\u{ff}' => vec![c as u32 as u8],
        '€' => vec![0x80],
        '…' => vec![0x85],
        '‘' => vec![0x91],
        '’' => vec![0x92],
        '“' => vec![0x93],
        '”' => vec![0x94],
        '•' => vec![0x95],
        '–' => vec![0x96],
        '—' => vec![0x97],
        '−' => vec![b'-'],
        '→' => b"->".to_vec(),
        '←' => b"<-".to_vec(),
        '≤' => b"<=".to_vec(),
        '≥' => b">=".to_vec(),
        '\t' | '\n' | '\r' => vec![b' '],
        _ => vec![b'?'],
    }
}

fn width(font: Font, size: f32, text: &str) -> f32 {
    let w: u32 = text
        .chars()
        .flat_map(ansi)
        .map(|b| match font {
            Font::Mono => 600,
            _ if (32..127).contains(&b) => {
                let t = if font == Font::Bold { &HELVETICA_BOLD } else { &HELVETICA };
                u32::from(t[(b - 32) as usize])
            }
            _ => 556,
        })
        .sum();
    w as f32 * size / 1000.0
}

/// A run of text in one font.
#[derive(Clone)]
struct Run {
    font: Font,
    text: String,
}

fn run(font: Font, text: impl Into<String>) -> Run {
    Run { font, text: text.into() }
}

const PAGE_W: f32 = 595.0;
const PAGE_H: f32 = 842.0;
const MARGIN: f32 = 56.0;

/// A PDF laid out from the top of the first page down.
struct Pdf {
    pages: Vec<String>,
    page: String,
    y: f32,
    /// the images (XObjects): width, height, RGB deflated
    images: Vec<(usize, usize, Vec<u8>)>,
    /// the images each page uses
    page_images: Vec<Vec<usize>>,
    used: Vec<usize>,
}

impl Pdf {
    fn new() -> Pdf {
        Pdf {
            pages: Vec::new(),
            page: String::new(),
            y: PAGE_H - MARGIN,
            images: Vec::new(),
            page_images: Vec::new(),
            used: Vec::new(),
        }
    }

    fn break_page(&mut self) {
        self.pages.push(std::mem::take(&mut self.page));
        self.page_images.push(std::mem::take(&mut self.used));
        self.y = PAGE_H - MARGIN;
    }

    fn room(&mut self, h: f32) {
        if self.y - h < MARGIN {
            self.break_page();
        }
    }

    fn text_at(&mut self, x: f32, y: f32, font: Font, size: f32, grey: f32, text: &str) {
        let mut bytes = Vec::new();
        for b in text.chars().flat_map(ansi) {
            if matches!(b, b'(' | b')' | b'\\') {
                bytes.push(b'\\');
            }
            bytes.push(b);
        }
        // (the bytes above 127 as octal escapes: the content stream stays ASCII)
        let mut s = String::new();
        for b in bytes {
            if b < 128 {
                s.push(b as char);
            } else {
                let _ = write!(s, "\\{b:03o}");
            }
        }
        let _ = writeln!(self.page, "{grey:.2} g BT /{} {size} Tf {x:.2} {y:.2} Td ({s}) Tj ET", font.name());
    }

    /// Runs of text wrapped in the width from `indent`, a line `size * 1.35` high.
    fn paragraph(&mut self, runs: &[Run], size: f32, indent: f32, grey: f32) {
        let max = PAGE_W - 2.0 * MARGIN - indent;
        let lead = size * 1.35;
        // words with their fonts (a word across two runs is two, without a space between), then lines of them
        let mut words: Vec<(Font, String, bool)> = Vec::new(); // the word, and whether a space comes before it
        let mut space = false;
        for r in runs {
            let mut word = String::new();
            for c in r.text.chars() {
                if c == ' ' {
                    if !word.is_empty() {
                        words.push((r.font, std::mem::take(&mut word), space));
                    }
                    space = true;
                } else {
                    word.push(c);
                }
            }
            if !word.is_empty() {
                words.push((r.font, word, space));
                space = false;
            }
        }
        let mut line: Vec<(Font, String, bool)> = Vec::new();
        let mut used = 0.0;
        let flush = |pdf: &mut Pdf, line: &mut Vec<(Font, String, bool)>| {
            pdf.room(lead);
            pdf.y -= lead;
            let mut x = MARGIN + indent;
            for (k, (f, w, space)) in line.iter().enumerate() {
                if k > 0 && *space {
                    x += width(*f, size, " ");
                }
                pdf.text_at(x, pdf.y, *f, size, grey, w);
                x += width(*f, size, w);
            }
            line.clear();
        };
        // the words without a space between (a name and its full stop) kept on one line
        let mut units: Vec<Vec<(Font, String, bool)>> = Vec::new();
        for w in words {
            match units.last_mut() {
                Some(u) if !w.2 => u.push(w),
                _ => units.push(vec![w]),
            }
        }
        for unit in units {
            let space = unit[0].2;
            let lead_space = if space && !line.is_empty() { width(unit[0].0, size, " ") } else { 0.0 };
            let uw: f32 = unit.iter().map(|(f, w, _)| width(*f, size, w)).sum();
            if !line.is_empty() && used + lead_space + uw > max {
                flush(self, &mut line);
                used = 0.0;
            }
            let lead_space = if space && !line.is_empty() { lead_space } else { 0.0 };
            used += lead_space;
            for (f, mut w, sp) in unit {
                // a word longer than the line: cut
                while width(f, size, &w) > max && w.chars().count() > 1 {
                    let mut cut = w.chars().count() - 1;
                    while cut > 1 && width(f, size, &w.chars().take(cut).collect::<String>()) > max {
                        cut -= 1;
                    }
                    line.push((f, w.chars().take(cut).collect(), sp));
                    flush(self, &mut line);
                    used = 0.0;
                    w = w.chars().skip(cut).collect();
                }
                used += width(f, size, &w);
                line.push((f, w, sp));
            }
        }
        if !line.is_empty() {
            flush(self, &mut line);
        }
    }

    fn space(&mut self, h: f32) {
        self.y -= h;
    }

    /// A badge (its label in white on its colour) and runs of text after it, wrapped beside it.
    fn badge(&mut self, rgb: [u8; 3], label: &str, rest: &[Run], size: f32, indent: f32) {
        let lead = size * 1.35;
        self.room(lead);
        let y = self.y - lead;
        let small = size * 0.85;
        let bw = width(Font::Bold, small, label) + 8.0;
        let x = MARGIN + indent;
        let [r, g, b] = rgb.map(|c| f32::from(c) / 255.0);
        let _ = writeln!(
            self.page,
            "{r:.3} {g:.3} {b:.3} rg {x:.2} {:.2} {bw:.2} {:.2} re f",
            y - size * 0.28,
            size * 1.15
        );
        self.text_at(x + 4.0, y, Font::Bold, small, 1.0, label);
        self.paragraph(rest, size, indent + bw + 4.0, 0.0);
    }

    fn rule(&mut self) {
        let _ = writeln!(
            self.page,
            "0.8 G 0.5 w {MARGIN} {:.2} m {:.2} {:.2} l S",
            self.y - 3.0,
            PAGE_W - MARGIN,
            self.y - 3.0
        );
        self.y -= 6.0;
    }

    /// A picture as an image of the PDF (composed on white: the pixels are premultiplied), used on this page.
    fn image(&mut self, p: &Picture) -> usize {
        let mut rgb = Vec::with_capacity(p.width * p.height * 3);
        for px in p.rgba.chunks(4) {
            let a = 255 - px[3];
            rgb.extend([px[0].saturating_add(a), px[1].saturating_add(a), px[2].saturating_add(a)]);
        }
        let k = self.images.len();
        self.images.push((p.width, p.height, miniz_oxide::deflate::compress_to_vec_zlib(&rgb, 6)));
        self.used.push(k);
        k
    }

    /// Pictures in a row, each at most `each` points wide (narrower when they do not fit), their tops in line,
    /// each with its caption under it.
    fn strip(&mut self, pics: &[(Picture, String)], each: f32, indent: f32) {
        if pics.is_empty() {
            return;
        }
        let (gap, size) = (8.0, 8.0);
        let avail = PAGE_W - 2.0 * MARGIN - indent;
        let w = each.min((avail - gap * (pics.len() - 1) as f32) / pics.len() as f32);
        let high = |p: &Picture| w * p.height as f32 / p.width.max(1) as f32;
        let h = pics.iter().map(|(p, _)| high(p)).fold(0.0, f32::max);
        self.room(h + size * 1.6 + 8.0);
        self.y -= h + 4.0;
        for (k, (p, caption)) in pics.iter().enumerate() {
            let x = MARGIN + indent + k as f32 * (w + gap);
            let id = self.image(p);
            let ph = high(p);
            let _ = writeln!(self.page, "q {w:.2} 0 0 {ph:.2} {x:.2} {:.2} cm /Im{id} Do Q", self.y + h - ph);
            self.text_at(x, self.y - size * 1.3, Font::Italic, size, 0.4, caption);
        }
        self.y -= size * 1.6 + 2.0;
    }

    fn picture(&mut self, p: &Picture, w: f32, indent: f32) {
        let h = w * p.height as f32 / p.width.max(1) as f32;
        self.room(h + 6.0);
        let k = self.image(p);
        self.y -= h + 4.0;
        let _ = writeln!(self.page, "q {w:.2} 0 0 {h:.2} {:.2} {:.2} cm /Im{k} Do Q", MARGIN + indent, self.y);
        self.y -= 2.0;
    }

    fn visual(&mut self, v: &Visual, w: f32, indent: f32) {
        match &v.shown {
            Shown::Picture(p) => self.picture(p, w, indent),
            Shown::Table { columns, rows, more } => {
                let size = 8.0;
                let cols = columns.len().max(1);
                let cw = ((PAGE_W - 2.0 * MARGIN - indent) / cols as f32).min(80.0);
                let line = |pdf: &mut Pdf, cells: &[String], font: Font| {
                    pdf.room(size * 1.5);
                    pdf.y -= size * 1.5;
                    for (j, c) in cells.iter().enumerate() {
                        let mut t = c.clone();
                        while width(font, size, &t) > cw - 4.0 && t.chars().count() > 1 {
                            t = t.chars().take(t.chars().count() - 1).collect();
                        }
                        pdf.text_at(MARGIN + indent + j as f32 * cw, pdf.y, font, size, 0.0, &t);
                    }
                };
                line(self, columns, Font::Bold);
                for r in rows {
                    line(self, r, Font::Regular);
                }
                if *more > 0 {
                    self.paragraph(&[run(Font::Italic, format!("and {more} more rows"))], 8.0, indent, 0.4);
                }
            }
            Shown::Text(t) => self.paragraph(&[run(Font::Italic, t.clone())], 9.0, indent, 0.4),
        }
        if let Some(of) = &v.of {
            self.paragraph(&[run(Font::Italic, of.clone())], 8.0, indent, 0.4);
        }
    }

    fn finish(mut self, title: &str) -> Vec<u8> {
        if !self.page.is_empty() || self.pages.is_empty() {
            self.break_page();
        }
        let n = self.pages.len();
        let images = self.images.len();
        // objects: 1 catalog, 2 pages, 3-6 fonts, then a page and its content each, then the images, the info
        let mut objects: Vec<Vec<u8>> = Vec::new();
        let page_obj = |k: usize| 7 + 2 * k;
        let image_obj = |k: usize| 7 + 2 * n + k;
        let info = 7 + 2 * n + images;
        objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        let kids: Vec<String> = (0..n).map(|k| format!("{} 0 R", page_obj(k))).collect();
        objects.push(format!("<< /Type /Pages /Kids [{}] /Count {n} >>", kids.join(" ")).into_bytes());
        for base in ["Helvetica", "Helvetica-Bold", "Helvetica-Oblique", "Courier"] {
            objects.push(
                format!("<< /Type /Font /Subtype /Type1 /BaseFont /{base} /Encoding /WinAnsiEncoding >>").into_bytes(),
            );
        }
        for (k, content) in self.pages.iter().enumerate() {
            let mut xo = String::new();
            for i in &self.page_images[k] {
                let _ = write!(xo, "/Im{i} {} 0 R ", image_obj(*i));
            }
            objects.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {PAGE_W} {PAGE_H}] /Resources << /Font << /F1 3 0 R \
                     /F2 4 0 R /F3 5 0 R /F4 6 0 R >> /XObject << {xo}>> >> /Contents {} 0 R >>",
                    page_obj(k) + 1
                )
                .into_bytes(),
            );
            let mut c = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            c.extend(content.as_bytes());
            c.extend(b"\nendstream");
            objects.push(c);
        }
        for (w, h, data) in &self.images {
            let mut c = format!(
                "<< /Type /XObject /Subtype /Image /Width {w} /Height {h} /ColorSpace /DeviceRGB /BitsPerComponent 8 \
                 /Filter /FlateDecode /Length {} >>\nstream\n",
                data.len()
            )
            .into_bytes();
            c.extend(data);
            c.extend(b"\nendstream");
            objects.push(c);
        }
        let mut t = String::new();
        for b in title.chars().flat_map(ansi) {
            if matches!(b, b'(' | b')' | b'\\') {
                t.push('\\');
            }
            if b < 128 {
                t.push(b as char);
            } else {
                let _ = write!(t, "\\{b:03o}");
            }
        }
        objects.push(format!("<< /Title ({t}) /Producer (Terraflux {}) >>", env!("CARGO_PKG_VERSION")).into_bytes());
        let mut out = b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n".to_vec();
        let mut offsets = Vec::new();
        for (k, o) in objects.iter().enumerate() {
            offsets.push(out.len());
            out.extend(format!("{} 0 obj\n", k + 1).as_bytes());
            out.extend(o);
            out.extend(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
        for o in offsets {
            out.extend(format!("{o:010} 00000 n \n").as_bytes());
        }
        out.extend(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R /Info {info} 0 R >>\nstartxref\n{xref}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        out
    }
}

/// A series of maps laid in a row in a PDF: its first, last and steps between, in one colouring, each `each`
/// points wide at most; whether it was one.
fn pdf_series(pdf: &mut Pdf, v: &Value, each: f32, indent: f32) -> bool {
    let Some(frames) = series_frames(v, STRIP) else { return false };
    let pics: Vec<(Picture, String)> =
        frames.iter().filter_map(|f| Some((frame_picture(f, STEP)?, f.caption.clone()))).collect();
    pdf.strip(&pics, each, indent);
    true
}

struct PdfReport<'a> {
    pdf: Pdf,
    values: Values<'a>,
    extras: &'a Extras,
    /// the containers the steps written are in
    path: Vec<String>,
    first: &'a dyn Fn(&str) -> Option<String>,
}

impl PdfReport<'_> {
    fn from(&self, f: &From) -> Run {
        run(Font::Bold, f.text(self.first))
    }

    fn reference(&self, r: &str) -> Run {
        match terraflux_graph::Ref::parse(r) {
            Ok(terraflux_graph::Ref::Port { node, port }) => self.from(&From::Node { node, port }),
            _ => run(Font::Bold, r),
        }
    }

    fn list(&mut self, title: &str, items: &[String], indent: f32) {
        if items.is_empty() {
            return;
        }
        self.pdf.paragraph(&[run(Font::Bold, format!("{title}."))], 9.5, indent, 0.0);
        for i in items {
            self.pdf.paragraph(&[run(Font::Regular, format!("• {i}"))], 9.5, indent + 8.0, 0.0);
        }
    }

    fn joined(&self, parts: Vec<Vec<Run>>, sep: &str) -> Vec<Run> {
        let mut out = Vec::new();
        for (k, p) in parts.into_iter().enumerate() {
            if k > 0 {
                out.push(run(Font::Regular, sep.to_string()));
            }
            out.extend(p);
        }
        out
    }

    fn step(&mut self, s: &Step, num: &str, indent: f32) {
        if s.size == Size::Line {
            let mut runs = vec![run(Font::Bold, format!("{num} {} ", s.id)), run(Font::Mono, s.op.clone())];
            let of: Vec<Vec<Run>> = s.inputs.iter().map(|l| vec![self.from(&l.from)]).collect();
            if !of.is_empty() {
                runs.push(run(Font::Regular, " of "));
                runs.extend(self.joined(of, ", "));
            }
            if let Some(n) = &s.note {
                runs.push(run(Font::Italic, format!(" — {n}")));
            }
            self.pdf.space(3.0);
            self.pdf.paragraph(&runs, 9.5, indent, 0.0);
            if let Some((v, own)) = self.extras.badge(&self.path, &s.id) {
                self.pdf.badge(v.rgb(), v.label(), &[run(Font::Regular, badge_words(&v, own))], 9.0, indent + 8.0);
            }
            return;
        }
        self.pdf.space(8.0);
        self.pdf.room(40.0);
        self.pdf.paragraph(
            &[run(Font::Bold, format!("{num} {}  ", s.id)), run(Font::Mono, s.op.clone())],
            11.0,
            indent,
            0.0,
        );
        if let Some((v, own)) = self.extras.badge(&self.path, &s.id) {
            self.pdf.badge(v.rgb(), v.label(), &[run(Font::Regular, badge_words(&v, own))], 9.5, indent);
        }
        match &s.note {
            Some(n) => self.pdf.paragraph(&[run(Font::Italic, n.clone())], 9.5, indent, 0.0),
            None => self.pdf.paragraph(&[run(Font::Italic, "No note.")], 9.0, indent, 0.45),
        }
        if let Some(w) = &s.why {
            self.pdf.paragraph(&[run(Font::Bold, "Why. "), run(Font::Regular, w.clone())], 9.5, indent, 0.0);
        }
        if let Some(t) = &s.sentence {
            self.pdf.paragraph(&[run(Font::Regular, t.clone())], 9.5, indent, 0.0);
        }
        if let Some(f) = &s.file {
            self.pdf.paragraph(&[run(Font::Regular, "File "), run(Font::Mono, f.clone())], 9.5, indent, 0.0);
        }
        if let Some(e) = &s.expression {
            self.pdf.paragraph(&[run(Font::Mono, e.clone())], 9.0, indent + 8.0, 0.0);
            let vars: Vec<Vec<Run>> = s
                .inputs
                .iter()
                .map(|l| vec![run(Font::Mono, l.port.clone()), run(Font::Regular, " = "), self.from(&l.from)])
                .collect();
            if !vars.is_empty() {
                let mut runs = vec![run(Font::Regular, "where ")];
                runs.extend(self.joined(vars, ", "));
                self.pdf.paragraph(&runs, 9.5, indent, 0.0);
            }
        } else if !s.inputs.is_empty() {
            let of: Vec<Vec<Run>> = s
                .inputs
                .iter()
                .map(|l| vec![run(Font::Regular, format!("{} ", l.port.replace('_', " "))), self.from(&l.from)])
                .collect();
            let mut runs = vec![run(Font::Regular, "Takes ")];
            runs.extend(self.joined(of, "; "));
            runs.push(run(Font::Regular, "."));
            self.pdf.paragraph(&runs, 9.5, indent, 0.0);
        }
        for st in &s.settings {
            let mut runs = vec![run(Font::Regular, format!("• {}: ", st.label)), run(Font::Bold, st.value.clone())];
            if let Some(d) = &st.default {
                runs.push(run(Font::Italic, format!(" (default {d})")));
            }
            self.pdf.paragraph(&runs, 9.5, indent + 8.0, 0.0);
        }
        if let Some(b) = &s.body {
            let mut head: Vec<Vec<Run>> = Vec::new();
            if let Some(t) = &b.times {
                head.push(vec![run(Font::Regular, "Repeated "), self.from(t), run(Font::Regular, " times.")]);
            }
            for (k, start, each) in &b.carry {
                let mut r = vec![run(Font::Regular, "Carries "), run(Font::Bold, k.clone())];
                if let Some(f) = start {
                    r.push(run(Font::Regular, " from "));
                    r.push(self.from(f));
                }
                r.push(run(Font::Regular, ", then "));
                r.push(self.reference(each));
                r.push(run(Font::Regular, "."));
                head.push(r);
            }
            for (k, r) in &b.collect {
                head.push(vec![
                    run(Font::Regular, "Collects "),
                    run(Font::Bold, k.clone()),
                    run(Font::Regular, " ("),
                    self.reference(r),
                    run(Font::Regular, ")."),
                ]);
            }
            for (k, r) in &b.outputs {
                head.push(vec![
                    run(Font::Regular, "Gives "),
                    run(Font::Bold, k.clone()),
                    run(Font::Regular, " ("),
                    self.reference(r),
                    run(Font::Regular, ")."),
                ]);
            }
            if let Some(u) = &b.until {
                head.push(vec![run(Font::Regular, "Stops when "), self.reference(u), run(Font::Regular, " is true.")]);
            }
            for h in head {
                self.pdf.paragraph(&h, 9.5, indent, 0.0);
            }
            self.path.push(s.id.clone());
            for (k, inner) in b.steps.iter().enumerate() {
                self.step(inner, &number(num, k), indent + 14.0);
            }
            self.path.pop();
        }
        self.list("References", &s.refs, indent);
        self.list("Assumptions", &s.assumptions, indent);
        self.list("Alternatives", &s.alternatives, indent);
        if let Some((port, _)) = &s.shows
            && let Some((node, key)) = s.value_key(port)
        {
            let small = matches!(s.stage, Stage::Data | Stage::Preparation);
            let value = (self.values)(&node, &key);
            if !value.as_ref().is_some_and(|v| pdf_series(&mut self.pdf, v, 110.0, indent)) {
                match value.and_then(|v| visual(&v, STEP)) {
                    Some(v) => self.pdf.visual(&v, if small { 110.0 } else { 180.0 }, indent),
                    None => self.pdf.paragraph(&[run(Font::Italic, "Not run.")], 9.0, indent, 0.45),
                }
            }
        }
        for f in &s.figures {
            self.figure(f, indent);
        }
    }

    fn figure(&mut self, f: &crate::document::Figure, indent: f32) {
        self.pdf.space(4.0);
        let value = (self.values)(&f.node, &f.port);
        if let Some(c) = figure_compared(self.values, self.first, f) {
            // the maps compared in a row, what their difference says and its key
            self.pdf.strip(&c.pics, if c.pics.len() > 2 { 150.0 } else { 200.0 }, indent);
            if let Some(t) = &c.said {
                self.pdf.paragraph(&[run(Font::Italic, t.clone())], 8.5, indent, 0.35);
            }
            for (rgb, words) in &c.key {
                self.pdf.badge(*rgb, "", &[run(Font::Regular, words.clone())], 8.0, indent + 4.0);
            }
        } else if !value.as_ref().is_some_and(|v| pdf_series(&mut self.pdf, v, 150.0, indent)) {
            match value.and_then(|v| visual(&v, FIGURE)) {
                Some(v) => self.pdf.visual(&v, 300.0, indent),
                None => self.pdf.paragraph(&[run(Font::Italic, "Not run.")], 9.0, indent, 0.45),
            }
        }
        let mut caption = vec![
            run(Font::Bold, format!("Figure {}. ", f.number)),
            run(Font::Regular, format!("{} (from ", f.caption)),
            self.from(&From::Node { node: f.node.clone(), port: f.port.clone() }),
        ];
        if let Some((n, p, _)) = &f.compared {
            caption.push(run(Font::Regular, ", compared with "));
            caption.push(self.from(&From::Node { node: n.clone(), port: p.clone() }));
        }
        caption.push(run(Font::Regular, ")"));
        self.pdf.paragraph(&caption, 9.0, indent, 0.25);
    }

    fn heading(&mut self, text: &str) {
        self.pdf.space(14.0);
        self.pdf.room(60.0);
        self.pdf.paragraph(&[run(Font::Bold, text)], 14.0, 0.0, 0.0);
        self.pdf.rule();
    }
}

/// The report as a PDF.
pub fn pdf(doc: &Document, values: Values, first: &dyn Fn(&str) -> Option<String>, extras: &Extras) -> Vec<u8> {
    let mut p = PdfReport { pdf: Pdf::new(), values, extras, path: Vec::new(), first };
    p.pdf.paragraph(&[run(Font::Bold, doc.title.clone())], 20.0, 0.0, 0.0);
    if let Some(d) = &doc.description {
        p.pdf.paragraph(&[run(Font::Italic, d.clone())], 10.5, 0.0, 0.35);
    }
    for (label, v) in [("Request", &doc.request), ("Author", &doc.author), ("Summary", &doc.summary)] {
        if let Some(v) = v {
            p.pdf.space(2.0);
            p.pdf.paragraph(&[run(Font::Bold, format!("{label}. ")), run(Font::Regular, v.clone())], 10.0, 0.0, 0.0);
        }
    }
    // the model's health: how its checks fared
    if let Some(health) = crate::checks::health(&extras.checks) {
        p.pdf.space(2.0);
        p.pdf.paragraph(&[run(Font::Bold, "Checks. "), run(Font::Regular, health)], 10.0, 0.0, 0.0);
        for c in &extras.checks {
            let at = c.path.iter().chain(std::iter::once(&c.id)).cloned().collect::<Vec<_>>().join("/");
            let rest = [
                run(Font::Bold, format!("{at} ")),
                run(Font::Mono, c.expression.clone()),
                run(Font::Regular, format!(": {}", c.verdict.words())),
            ];
            p.pdf.badge(c.verdict.rgb(), c.verdict.label(), &rest, 9.0, 8.0);
        }
    }
    let mut k = 0;
    if !doc.inputs.is_empty() {
        k += 1;
        p.heading(&format!("{k}. Inputs"));
        for i in &doc.inputs {
            let mut runs = vec![
                run(Font::Bold, format!("${} ", i.name)),
                run(Font::Regular, format!("{}: ", i.kind)),
                run(Font::Mono, i.value.clone()),
            ];
            if let Some(n) = &i.note {
                runs.push(run(Font::Italic, format!(" — {n}")));
            }
            p.pdf.paragraph(&runs, 9.5, 0.0, 0.0);
        }
    }
    if !doc.parameters.is_empty() {
        k += 1;
        p.heading(&format!("{k}. Parameters"));
        for x in &doc.parameters {
            let mut runs = vec![
                run(Font::Bold, format!("{}: ", x.label)),
                run(
                    Font::Regular,
                    format!("{}{}", x.value, x.unit.as_ref().map(|u| format!(" {u}")).unwrap_or_default()),
                ),
            ];
            if let Some(r) = &x.range {
                runs.push(run(Font::Italic, format!(" ({r})")));
            }
            if let Some(h) = &x.help {
                runs.push(run(Font::Regular, format!(" — {h}")));
            }
            p.pdf.paragraph(&runs, 9.5, 0.0, 0.0);
        }
    }
    if !doc.scenarios.is_empty() {
        k += 1;
        p.heading(&format!("{k}. Scenarios"));
        for sc in &doc.scenarios {
            let v: Vec<String> = sc.values.iter().map(|(l, v)| format!("{l} {v}")).collect();
            let mut runs = vec![run(Font::Bold, format!("{}: ", sc.name)), run(Font::Regular, v.join("; "))];
            if let Some(n) = &sc.note {
                runs.push(run(Font::Italic, format!(" — {n}")));
            }
            p.pdf.paragraph(&runs, 9.5, 0.0, 0.0);
        }
    }
    for sec in &doc.sections {
        k += 1;
        let num = k.to_string();
        p.heading(&format!("{num}. {}", sec.title));
        if let Some(n) = &sec.note {
            p.pdf.paragraph(&[run(Font::Italic, n.clone())], 10.0, 0.0, 0.35);
        }
        if sec.stage == Stage::Data {
            for (j, row) in data_rows(&sec.steps).iter().enumerate() {
                let ids: Vec<&str> = row.iter().map(|s| s.id.as_str()).collect();
                p.pdf.space(4.0);
                p.pdf.paragraph(
                    &[
                        run(Font::Bold, format!("{} {}  ", number(&num, j), ids.join(", "))),
                        run(Font::Mono, row[0].op.clone()),
                    ],
                    10.0,
                    0.0,
                    0.0,
                );
                if let Some(n) = &row[0].note {
                    p.pdf.paragraph(&[run(Font::Italic, n.clone())], 9.5, 0.0, 0.0);
                }
                if let (1, Some(t)) = (row.len(), &row[0].sentence) {
                    p.pdf.paragraph(&[run(Font::Regular, t.clone())], 9.5, 0.0, 0.0);
                }
                for f in row.iter().filter_map(|s| s.file.as_ref()) {
                    p.pdf.paragraph(&[run(Font::Mono, f.clone())], 9.0, 8.0, 0.0);
                }
                let set: Vec<String> = row_settings(row).iter().map(|(l, v)| format!("{l}: {v}")).collect();
                if !set.is_empty() {
                    p.pdf.paragraph(&[run(Font::Regular, set.join("; "))], 9.0, 0.0, 0.25);
                }
                if let Some(w) = &row[0].why {
                    p.pdf.paragraph(&[run(Font::Bold, "Why. "), run(Font::Regular, w.clone())], 9.5, 0.0, 0.0);
                }
                for s in row.iter() {
                    for f in &s.figures {
                        p.figure(f, 0.0);
                    }
                }
            }
        } else {
            for (j, s) in sec.steps.iter().enumerate() {
                p.step(s, &number(&num, j), 0.0);
            }
        }
        if sec.stage == Stage::Results {
            for f in &doc.figures {
                p.figure(f, 0.0);
            }
        }
    }
    if !doc.figures.is_empty() && !doc.sections.iter().any(|s| s.stage == Stage::Results) {
        k += 1;
        p.heading(&format!("{k}. Results"));
        for f in &doc.figures {
            p.figure(f, 0.0);
        }
    }
    if !doc.outputs.is_empty() {
        k += 1;
        p.heading(&format!("{k}. Outputs"));
        for o in &doc.outputs {
            let mut runs =
                vec![run(Font::Bold, format!("{} ", o.name)), run(Font::Regular, "from "), p.reference(&o.from)];
            if !o.file.is_empty() {
                runs.push(run(Font::Regular, ", "));
                runs.push(run(Font::Mono, o.file.clone()));
            }
            p.pdf.paragraph(&runs, 9.5, 0.0, 0.0);
        }
    }
    // the methods as a paper tells them, and the papers to cite
    if let Some(m) = &extras.methods {
        k += 1;
        p.heading(&format!("{k}. Methods and citations"));
        p.pdf.paragraph(&[run(Font::Regular, m.paragraph.clone())], 10.0, 0.0, 0.0);
        p.pdf.space(6.0);
        p.pdf.paragraph(&[run(Font::Bold, "References")], 10.5, 0.0, 0.0);
        for r in extras.references() {
            p.pdf.space(2.0);
            p.pdf.paragraph(&[run(Font::Regular, r)], 9.0, 0.0, 0.0);
        }
        if !m.own.is_empty() {
            p.pdf.space(6.0);
            p.pdf.paragraph(&[run(Font::Bold, "Given in the model's notes")], 10.5, 0.0, 0.0);
            for r in &m.own {
                p.pdf.space(2.0);
                p.pdf.paragraph(&[run(Font::Regular, r.clone())], 9.0, 0.0, 0.0);
            }
        }
    }
    p.pdf.space(18.0);
    p.pdf.paragraph(
        &[run(
            Font::Italic,
            format!("Made by Terraflux {} on {} from the model's graph.", env!("CARGO_PKG_VERSION"), today()),
        )],
        8.5,
        0.0,
        0.45,
    );
    p.pdf.finish(&doc.title)
}

/// The report of a model as HTML or PDF (by the file's extension), its images from the values given.
pub fn write(
    path: &std::path::Path,
    doc: &Document,
    values: Values,
    first: &dyn Fn(&str) -> Option<String>,
    extras: &Extras,
) -> std::io::Result<()> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("pdf")) {
        std::fs::write(path, pdf(doc, values, first, extras))
    } else {
        std::fs::write(path, html(doc, values, first, extras))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Registry;

    fn template(name: &str) -> terraflux_graph::Model {
        crate::templates::TEMPLATES.iter().find(|t| t.name == name).expect("a template").model()
    }

    #[test]
    fn base64_and_dates() {
        assert_eq!(base64(b"Man"), "TWFu");
        assert_eq!(base64(b"Ma"), "TWE=");
        assert_eq!(base64(b"M"), "TQ==");
        let d = today();
        assert_eq!(d.len(), 10);
        assert!(d.starts_with("20"));
        assert_eq!(cell(3.0), "3");
        assert_eq!(cell(0.123456), "0.1235");
        assert_eq!(cell(1916.43), "1916.4");
        assert_eq!(cell(f64::NAN), "–");
    }

    #[test]
    fn a_report_without_a_run_says_so() {
        let r = Registry::standard();
        let m = template("CLUE-S");
        let d = Document::of(&m, &r);
        let none = |_: &str, _: &str| None;
        let first = |_: &str| None;
        let h = html(&d, &none, &first, &Extras::default());
        assert!(h.starts_with("<!doctype html>"));
        assert!(h.contains("<h2>1. Data</h2>"));
        assert!(h.contains("<h2>3. Model</h2>"));
        assert!(h.contains("3.4.1 allocate"), "the loop's body numbered within it");
        assert!(h.contains("Not run."));
        assert!(h.contains("Figure 1.</b> Simulated land use"));
        assert!(h.contains("the range of the autocorrelation, in cells"));
        let p = pdf(&d, &none, &first, &Extras::default());
        assert!(p.starts_with(b"%PDF-1.4"));
        assert!(p.ends_with(b"%%EOF\n"));
    }

    #[test]
    fn a_report_tells_the_methods_and_how_the_checks_fared() {
        use crate::checks::{Check, Verdict};
        let r = Registry::standard();
        let mut m = template("Neutral landscapes");
        let first_node = m.nodes[0].id.clone();
        m.nodes.push(
            terraflux_graph::Node::new("enough", "check.assert")
                .input("value", &format!("{first_node}.raster"))
                .param("expression", "value >= 0"),
        );
        let d = Document::of(&m, &r);
        let none = |_: &str, _: &str| None;
        let first = |_: &str| None;
        let checks = vec![Check {
            path: Vec::new(),
            id: "enough".into(),
            expression: "value >= 0".into(),
            verdict: Verdict::Warned("some cells below".into()),
        }];
        let extras = Extras::of(&m, &r, Some("neutral.toml"), checks);
        let h = html(&d, &none, &first, &extras);
        assert!(h.contains("<b>Checks.</b> 1 check: 1 warned"), "the health at the top");
        assert!(h.contains(">warned</span>"), "the badge");
        assert!(h.contains("failed, warning only: some cells below"), "what it said, on its step");
        assert!(h.contains(". Methods and citations</h2><p>The analysis was run in Terraflux"));
        assert!(h.contains("(neutral.toml)"));
        assert!(h.contains("<li>Lacerda, E. (2026). Terraflux"), "Terraflux among the references");
        let p = pdf(&d, &none, &first, &extras);
        let text = String::from_utf8_lossy(&p);
        assert!(text.contains("(warned) Tj"));
        assert!(text.contains("(citations) Tj") && text.contains("(Lacerda,) Tj"));
        // without them, neither
        let plain = html(&d, &none, &first, &Extras::default());
        assert!(!plain.contains("Methods and citations") && !plain.contains("Checks."));
    }

    #[test]
    fn a_series_is_animated_in_html_and_laid_in_a_row_in_a_pdf() {
        use std::sync::Arc;
        let map = |k: usize| {
            let grid = Arc::new(terraflux_raster::Grid::new(20, 10));
            Value::Map(terraflux_ops::Map::new(grid, (0..200).map(|i| f64::from(u8::from(i < k * 4))).collect()))
        };
        let series = Value::Series((0..50).map(map).collect());
        assert_eq!(picks(50, 4), [0, 16, 32, 49]);
        assert_eq!(picks(3, 4), [0, 1, 2]);
        let frames = series_frames(&series, FRAMES).unwrap();
        assert_eq!(frames.len(), FRAMES);
        assert_eq!((frames[0].caption.as_str(), frames[39].caption.as_str()), ("step 1 of 50", "step 50 of 50"));
        assert!(frames.iter().all(|f| f.coloring == frames[0].coloring), "one colouring");
        let h = html_value(Some(series.clone()), "sim", STEP, STEP).unwrap();
        assert!(h.contains("data:image/gif;base64,R0lGOD"), "a GIF");
        assert!(h.contains("the 50 steps, 40 of them animated"));
        let mut pdf = Pdf::new();
        assert!(pdf_series(&mut pdf, &series, 110.0, 0.0));
        assert_eq!(pdf.used.len(), 4, "four steps in a row");
        assert!(pdf.page.contains("(of) Tj") || pdf.page.contains("(step 50 of 50) Tj"), "{}", pdf.page);
        // a single map is not a series
        assert!(series_frames(&map(3), FRAMES).is_none());
    }

    #[test]
    fn a_plot_compares_two_maps_and_their_difference() {
        let r = Registry::standard();
        let m = terraflux_graph::Model::from_toml(
            r#"
            format = "terraflux/1"
            [[nodes]]
            id = "before"
            op = "nlm.random"
            params = { rows = 20, columns = 30, seed = 1 }
            [[nodes]]
            id = "after"
            op = "raster.calc"
            inputs = { a = "before.map" }
            params = { expression = "out x = 1 if a > 0.5 else 0" }
            [[nodes]]
            id = "was"
            op = "raster.calc"
            inputs = { a = "before.map" }
            params = { expression = "out x = 1 if a > 0.3 else 0" }
            [[nodes]]
            id = "plot"
            op = "view.plot"
            inputs = { value = "was.x", compared = "after.x" }
            params = { title = "Forest lost", compare = "difference" }
            "#,
        )
        .unwrap();
        let d = Document::of(&m, &r);
        let f = d.steps().iter().flat_map(|s| s.figures.clone()).next().unwrap();
        assert_eq!(f.compared, Some(("after".into(), "x".into(), "difference".into())));
        let results =
            terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let values = |n: &str, p: &str| results.values.get(n)?.get(p).cloned();
        let first = |n: &str| crate::document::only_output(&m, &r, n);
        let c = figure_compared(&values, &first, &f).unwrap();
        assert_eq!(c.pics.iter().map(|(_, n)| n.as_str()).collect::<Vec<_>>(), ["was", "after", "difference"]);
        assert!(c.said.as_deref().is_some_and(|t| t.contains("cells changed class")), "{:?}", c.said);
        assert_eq!(c.key[0].1.split(':').next(), Some("unchanged"));
        assert!(c.key.iter().any(|(_, w)| w.starts_with("1 -> 0: ")), "{:?}", c.key);
        let h = html(&d, &values, &first, &Extras::default());
        assert!(h.contains(r#"<div class="pair">"#) && h.contains("compared with <a"));
        let p = pdf(&d, &values, &first, &Extras::default());
        assert!(String::from_utf8_lossy(&p).contains("(compared) Tj"));
        // side by side: the two maps, no difference
        let mut f2 = f.clone();
        f2.compared = Some(("after".into(), "x".into(), "side by side".into()));
        let c = figure_compared(&values, &first, &f2).unwrap();
        assert_eq!((c.pics.len(), c.said, c.key.len()), (2, None, 0));
    }

    #[test]
    fn a_report_shows_the_values_run() {
        let r = Registry::standard();
        let m = template("Neutral landscapes");
        let results =
            terraflux_exec::Engine::default().run(&m, &Default::default(), &mut terraflux_exec::Quiet).unwrap();
        let d = Document::of(&m, &r);
        let values = |n: &str, p: &str| results.values.get(n)?.get(p).cloned();
        let first = |n: &str| crate::document::only_output(&m, &r, n);
        let h = html(&d, &values, &first, &Extras::default());
        assert!(h.contains("data:image/png;base64,"), "the maps as images");
        assert!(!h.contains("Not run."));
        let p = pdf(&d, &values, &first, &Extras::default());
        let text = String::from_utf8_lossy(&p);
        assert!(text.contains("/Subtype /Image"));
        // the cross-reference table points at the objects
        let xref: usize = text.rsplit("startxref\n").next().unwrap().lines().next().unwrap().parse().unwrap();
        assert!(p[xref..].starts_with(b"xref"));
    }
}
