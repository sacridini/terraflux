//! The model's graph as a figure for a paper: SVG (text as text, to edit in Inkscape), PDF (vectors and text) or
//! PNG. The boxes, ports, wires, groups, notes and frames are the canvas's own (the same layout, the wires around
//! the boxes), always in the light theme on white, with ink that reads on paper (4.5:1 for text, 3:1 for lines and
//! edges). Written through [`crate::figure::Drawing`], the same as the maps' figures, so the three formats come
//! from one drawing, sized in points to fit a column (85 mm, 180 mm, or the natural size).
//!
//! Pure: the model and the measures of its texts, no window.

use egui::{Color32, Pos2, Rect, Vec2};
use terraflux_graph::{Model, Type};

use crate::editor_view::{FOOT, FigBox, HEAD, PAD, PORT, figure_scene, file_label, type_colour};
use crate::figure::{Drawing, text_width};

/// A figure's width: its own size (a graph unit is [`NATURAL`] points) or fitted to a width in millimetres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Width {
    Natural,
    Mm(f32),
}

impl Width {
    /// One column of an article, two columns, the figure's own.
    pub const CHOICES: [(Width, &'static str); 3] = [
        (Width::Mm(85.0), "85 mm (a column)"),
        (Width::Mm(180.0), "180 mm (a page)"),
        (Width::Natural, "natural size"),
    ];

    /// `85mm`, `18cm`, `4in`, `240pt`, a bare number of millimetres, or `natural`.
    pub fn parse(text: &str) -> Result<Width, String> {
        let t = text.trim().to_lowercase();
        if t == "natural" || t == "auto" {
            return Ok(Width::Natural);
        }
        let (num, unit) = match t.find(|c: char| c.is_ascii_alphabetic()) {
            Some(i) => t.split_at(i),
            None => (t.as_str(), "mm"),
        };
        let v: f32 = num.trim().parse().map_err(|_| format!("`{text}` is not a width (try 85mm, 180mm or natural)"))?;
        let mm = match unit.trim() {
            "mm" => v,
            "cm" => v * 10.0,
            "in" => v * 25.4,
            "pt" => v * 25.4 / 72.0,
            u => return Err(format!("`{u}` is not a unit of width (mm, cm, in, pt)")),
        };
        if !mm.is_finite() || !(10.0..=2000.0).contains(&mm) {
            return Err(format!("`{text}` is not a width between 10 mm and 2000 mm"));
        }
        Ok(Width::Mm(mm))
    }
}

/// What the figure draws.
#[derive(Debug, Clone)]
pub struct Options {
    pub width: Width,
    /// The notes and frames of the canvas.
    pub notes: bool,
    /// The rows of settings the canvas shows on the boxes.
    pub settings: bool,
    /// What is inside the groups and other containers, framed under the graph.
    pub groups: bool,
    /// The scope drawn: the containers' ids from the top (none: the whole model's top level).
    pub path: Vec<String>,
    /// Only these nodes of the scope (a selection).
    pub only: Option<Vec<String>>,
}

impl Default for Options {
    fn default() -> Self {
        Options { width: Width::Mm(180.0), notes: true, settings: true, groups: true, path: Vec::new(), only: None }
    }
}

/// A graph unit in points at the natural size: the canvas's 14 px name is 8.4 pt.
pub const NATURAL: f32 = 0.6;
/// The room around the figure, points.
const MARGIN: f32 = 6.0;
/// The smallest text, in points, under which a warning is given (a journal's usual limit is 5 to 6 pt).
pub const MIN_TEXT_PT: f32 = 5.0;

const WHITE: Color32 = Color32::WHITE;
/// The ink: names and values (14:1 on white), the quieter ink of the ports and the other texts (7.5:1).
const INK: Color32 = Color32::from_rgb(34, 34, 34);
const WEAK: Color32 = Color32::from_rgb(85, 85, 85);
/// The edge of a box (4.5:1 on white), the rules inside it (decoration), a wire of a type without a colour.
const EDGE: Color32 = Color32::from_rgb(118, 118, 118);
const RULE: Color32 = Color32::from_rgb(205, 205, 205);
const GREY_WIRE: Color32 = Color32::from_rgb(110, 110, 110);
/// A port the node needs and nothing feeds, a node turned off: a dark amber.
const WARN: Color32 = Color32::from_rgb(166, 90, 0);
const SOURCE_FILL: Color32 = Color32::from_rgb(244, 244, 244);
/// The accent of a group, fixed (the figure is the same whatever the theme).
const ACCENT: Color32 = Color32::from_rgb(46, 110, 190);

/// The sizes of the texts, graph units (the canvas's).
const NAME: f32 = 14.0;
const SMALL: f32 = 11.0;
const FILE: f32 = 10.5;
const FRAME_TITLE: f32 = 16.0;
const NOTE_TEXT: f32 = 13.0;
const NOTE_LINE: f32 = 16.0;
/// How deep the containers are opened.
const DEEP: usize = 3;

/// A thing drawn, in graph units from the canvas's origin.
#[derive(Debug, Clone)]
enum Item {
    Box { r: Rect, radius: f32, fill: Color32, edge: Option<(Color32, f32)> },
    Wire { pts: Vec<Pos2>, col: Color32, w: f32 },
    Disc { c: Pos2, r: f32, fill: Option<Color32>, edge: Option<(Color32, f32)> },
    Text { at: Pos2, size: f32, text: String, col: Color32 },
}

impl Item {
    fn bounds(&self) -> Rect {
        match self {
            Item::Box { r, .. } => *r,
            Item::Wire { pts, .. } => pts.iter().fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(*p, *p))),
            Item::Disc { c, r, .. } => Rect::from_center_size(*c, Vec2::splat(2.0 * r)),
            Item::Text { at, size, text, .. } => {
                Rect::from_min_size(Pos2::new(at.x, at.y - size), Vec2::new(text_width(*size, text), *size * 1.25))
            }
        }
    }

    fn shift(&mut self, d: Vec2) {
        match self {
            Item::Box { r, .. } => *r = r.translate(d),
            Item::Wire { pts, .. } => pts.iter_mut().for_each(|p| *p += d),
            Item::Disc { c, .. } => *c += d,
            Item::Text { at, .. } => *at += d,
        }
    }
}

/// The figure: its drawing (in points, `w` x `h`), how many nodes it draws and the size of its smallest text.
pub struct Figure {
    pub drawing: Drawing,
    pub nodes: usize,
    /// Points: the smallest text drawn (below [`MIN_TEXT_PT`] it is hard to read on paper).
    pub min_text_pt: f32,
    pub width_mm: f32,
}

impl Figure {
    pub fn svg(&self) -> String {
        self.drawing.svg(self.width_mm)
    }

    pub fn pdf(&self, title: &str) -> Vec<u8> {
        self.drawing.pdf(title)
    }

    /// As PNG at `dpi` (the file records it, so a word processor places the figure at its width).
    pub fn png(&self, dpi: u32) -> Result<Vec<u8>, String> {
        self.drawing.png(dpi)
    }

    /// A word of advice when the text comes out too small to read on paper.
    pub fn warning(&self) -> Option<String> {
        (self.min_text_pt < MIN_TEXT_PT).then(|| {
            format!(
                "the smallest text is {:.1} pt: a wider figure, fewer nodes (a selection, or a group) or only the \
                 top level would read better",
                self.min_text_pt
            )
        })
    }
}

/// The three formats of the figure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Svg,
    Pdf,
    Png,
}

impl Format {
    pub const ALL: [Format; 3] = [Format::Svg, Format::Pdf, Format::Png];

    pub fn ext(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Pdf => "pdf",
            Format::Png => "png",
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Format::Svg => "SVG (text as text)",
            Format::Pdf => "PDF (vectors and text)",
            Format::Png => "PNG (300 dpi)",
        }
    }

    /// The format a file name asks for (SVG when it says nothing).
    pub fn of_path(path: &std::path::Path) -> Format {
        match path.extension().map(|e| e.to_string_lossy().to_lowercase()).as_deref() {
            Some("pdf") => Format::Pdf,
            Some("png") => Format::Png,
            _ => Format::Svg,
        }
    }
}

/// The ink a type's wires and ports are drawn in on white: the canvas's colour for it (its light theme's), dark
/// enough to read as a line (3:1), grey for the types with none.
pub fn wire_ink(t: Option<Type>) -> Color32 {
    t.and_then(|t| type_colour(t, false)).map_or(GREY_WIRE, |c| crate::theme::readable(c, WHITE, 3.0))
}

/// A box's header tint over a fill: the type's colour washed in.
fn tint_of(fill: Color32, t: Type) -> Option<Color32> {
    type_colour(t, false).map(|c| fill.lerp_to_gamma(c, 0.18))
}

/// A note's fill: white washed in its colour; a frame's.
fn note_fill(c: Color32) -> Color32 {
    WHITE.lerp_to_gamma(c, 0.22)
}

fn frame_fill(c: Color32) -> Color32 {
    WHITE.lerp_to_gamma(c, 0.07)
}

/// A frame's title: its colour darkened until it reads over the frame's own fill (4.5:1).
fn title_ink(c: Color32) -> Color32 {
    crate::theme::readable(c, frame_fill(c), 4.5)
}

fn group_colours() -> crate::editor_view::GroupLook {
    crate::editor_view::group_look(false, WHITE, EDGE, ACCENT, WHITE, Color32::from_rgb(17, 17, 17))
}

/// A text cut to `room` graph units, with an ellipsis when it does not fit.
fn fit(text: &str, size: f32, room: f32) -> String {
    if text_width(size, text) <= room {
        return text.to_string();
    }
    let mut out: String = text.to_string();
    while !out.is_empty() && text_width(size, &format!("{out}\u{2026}")) > room {
        out.pop();
    }
    format!("{}\u{2026}", out.trim_end())
}

/// A text wrapped to a width (graph units), its own lines kept.
fn wrap(text: &str, size: f32, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in text.lines() {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let next = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && text_width(size, &next) > width {
                lines.push(std::mem::take(&mut line));
                line = word.to_string();
            } else {
                line = next;
            }
        }
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// A rounded rectangle as a ring of points.
fn rounded(r: Rect, radius: f32) -> Vec<(f32, f32)> {
    let k = radius.min(r.width() / 2.0).min(r.height() / 2.0).max(0.0);
    if k < 0.01 {
        return vec![(r.min.x, r.min.y), (r.max.x, r.min.y), (r.max.x, r.max.y), (r.min.x, r.max.y)];
    }
    let mut out = Vec::new();
    // each corner: its centre and the angle it starts at
    for (c, a0) in [
        (Pos2::new(r.max.x - k, r.min.y + k), -90.0f32),
        (Pos2::new(r.max.x - k, r.max.y - k), 0.0),
        (Pos2::new(r.min.x + k, r.max.y - k), 90.0),
        (Pos2::new(r.min.x + k, r.min.y + k), 180.0),
    ] {
        for i in 0..=4 {
            let a = (a0 + 90.0 * i as f32 / 4.0).to_radians();
            out.push((c.x + k * a.cos(), c.y + k * a.sin()));
        }
    }
    out
}

fn ring(c: Pos2, r: f32) -> Vec<(f32, f32)> {
    (0..16)
        .map(|i| {
            let a = i as f32 / 16.0 * std::f32::consts::TAU;
            (c.x + r * a.cos(), c.y + r * a.sin())
        })
        .collect()
}

/// What a node's box is made of: its card, header, texts, ports and rows of settings.
fn draw_box(b: &FigBox, items: &mut Vec<Item>) {
    let radius = 6.0;
    let r = b.rect;
    let look = group_colours();
    let fill = if b.source || b.bypass { SOURCE_FILL } else { WHITE };
    let (edge, ew) = if b.group {
        (look.border, 1.5)
    } else if b.bypass {
        (WARN, 1.5)
    } else {
        (EDGE, 1.0)
    };
    if b.group {
        // two cards behind it: a stack of nodes
        for k in [2.0f32, 1.0] {
            let card = Rect::from_min_size(
                Pos2::new(r.min.x + 7.0 * k, r.min.y + 5.0 * k),
                Vec2::new(r.width() - 14.0 * k, r.height()),
            );
            items.push(Item::Box {
                r: card,
                radius,
                fill: WHITE.lerp_to_gamma(ACCENT, 0.05 * k),
                edge: Some((edge.gamma_multiply(0.35 + 0.25 * (3.0 - k)), 1.0)),
            });
        }
    }
    items.push(Item::Box { r, radius, fill, edge: Some((edge, ew)) });
    let head = Rect::from_min_size(r.min + Vec2::splat(2.0), Vec2::new(r.width() - 4.0, HEAD - 4.0));
    if b.group {
        items.push(Item::Box { r: head, radius: radius - 1.0, fill: look.head, edge: None });
    } else if let Some(c) = b.tint.and_then(|t| tint_of(fill, t)) {
        items.push(Item::Box { r: head, radius: radius - 1.0, fill: c, edge: None });
    }
    // its name, its operator under it (a model's input: its name alone)
    let text = |items: &mut Vec<Item>, x: f32, y: f32, size: f32, t: &str, col: Color32| {
        items.push(Item::Text { at: Pos2::new(x, y), size, text: t.to_string(), col });
    };
    let strong = if b.group { Color32::from_rgb(17, 17, 17) } else { INK };
    if b.source {
        text(items, r.min.x + 8.0, r.min.y + 24.0, NAME, &fit(&b.key, NAME, r.width() - 16.0), strong);
    } else {
        text(items, r.min.x + 8.0, r.min.y + 17.0, NAME, &fit(&b.key, NAME, r.width() - 16.0), strong);
        let mut op = b.label.clone();
        if b.container {
            op = format!("{op} \u{b7} {} node{}", b.inside, if b.inside == 1 { "" } else { "s" });
        }
        if b.bypass {
            op.push_str(" \u{b7} off");
        }
        if b.frozen {
            op.push_str(" \u{b7} frozen");
        }
        text(
            items,
            r.min.x + 8.0,
            r.min.y + 31.0,
            SMALL,
            &fit(&op, SMALL, r.width() - 16.0),
            if b.group { strong } else { WEAK },
        );
    }
    // the ports and their names
    let port = |items: &mut Vec<Item>, c: Pos2, fill: Option<Color32>| {
        items.push(Item::Disc { c, r: PORT * 0.8, fill, edge: Some((if fill.is_some() { WHITE } else { EDGE }, 1.0)) });
    };
    let mut left_end = r.min.x;
    for (name, ty, required, connected, at) in &b.inputs {
        let fill = if *connected {
            Some(wire_ink(Some(*ty)))
        } else if *required {
            Some(WARN)
        } else {
            None
        };
        port(items, *at, fill);
        text(items, r.min.x + PAD, at.y + 4.0, SMALL, name, WEAK);
        left_end = left_end.max(r.min.x + PAD + text_width(SMALL, name));
    }
    for (name, ty, at) in &b.outputs {
        port(items, *at, Some(wire_ink(Some(*ty))));
        if !name.is_empty() {
            let w = text_width(SMALL, name);
            // (not over the name of an input on the same row, the box being as wide as both need)
            text(items, (r.max.x - PAD - w).max(left_end + 8.0), at.y + 4.0, SMALL, name, WEAK);
        }
    }
    // the rows of settings, a name and its value, under a line
    if let Some(y) = b.settings_line {
        items.push(Item::Wire {
            pts: vec![Pos2::new(r.min.x + 6.0, y), Pos2::new(r.max.x - 6.0, y)],
            col: RULE,
            w: 1.0,
        });
    }
    for (row, at) in &b.settings {
        text(items, r.min.x + PAD, at.y + 4.0, SMALL, &row.name, WEAK);
        let room = r.width() - 2.0 * PAD - text_width(SMALL, &row.name) - 14.0;
        let value = fit(&row.value, SMALL, room.max(20.0));
        let w = text_width(SMALL, &value);
        text(items, r.max.x - PAD - w, at.y + 4.0, SMALL, &value, if row.given { INK } else { WEAK });
        if let Some(t) = row.feed {
            if row.reads {
                items.push(Item::Disc {
                    c: *at,
                    r: PORT - 1.0,
                    fill: Some(wire_ink(Some(t))),
                    edge: Some((WHITE, 1.0)),
                });
            } else {
                items.push(Item::Disc { c: *at, r: PORT - 1.5, fill: Some(WHITE), edge: Some((EDGE, 1.0)) });
            }
        }
    }
    if let Some((label, at)) = &b.more {
        text(items, r.min.x + PAD, at.y + 4.0, SMALL, label, WEAK);
    }
    if let Some(f) = &b.file {
        let y = r.max.y - FOOT;
        items.push(Item::Wire {
            pts: vec![Pos2::new(r.min.x + 6.0, y), Pos2::new(r.max.x - 6.0, y)],
            col: RULE,
            w: 1.0,
        });
        let name = file_label(f, 40);
        text(items, r.min.x + PAD, r.max.y - 6.0, FILE, &fit(&name, FILE, r.width() - 2.0 * PAD), WEAK);
    }
}

/// A scope drawn: its items, the nodes in it, and the containers it has (to open them under it).
struct Scope {
    items: Vec<Item>,
    nodes: usize,
}

/// One scope as the canvas draws it: frames and notes behind, then the wires, then the boxes.
fn scope(model: &Model, o: &Options, path: &[String], only: Option<&[String]>) -> Option<Scope> {
    let (boxes, wires) = figure_scene(model, path, only, o.settings)?;
    let mut items: Vec<Item> = Vec::new();
    let nodes = boxes.iter().filter(|b| !b.source).count();
    let bounds = boxes.iter().fold(Rect::NOTHING, |r, b| r.union(b.rect));
    if o.notes {
        let boxed = |id: &str| boxes.iter().find(|b| b.key == id).map(|b| b.rect);
        let measure = |text: &str, w: f32| wrap(text, NOTE_TEXT, w).len() as f32 * NOTE_LINE;
        for l in crate::notes::layout(model, path, &boxed, &measure, None) {
            // (a selection: the notes beside it only)
            if only.is_some() && !l.frame && !l.rect.intersects(bounds.expand(60.0)) {
                continue;
            }
            let edge = crate::notes::inks(l.colour, false, WHITE).0;
            if l.frame {
                items.push(Item::Box { r: l.rect, radius: 8.0, fill: frame_fill(l.colour), edge: Some((edge, 1.2)) });
                let title = if l.text.is_empty() { "(a frame)" } else { l.text.as_str() };
                let at = Pos2::new(l.rect.min.x + 10.0, l.grip.center().y + 5.0);
                items.push(Item::Text {
                    at,
                    size: FRAME_TITLE,
                    text: fit(title, FRAME_TITLE, l.rect.width() - 20.0),
                    col: title_ink(l.colour),
                });
            } else {
                items.push(Item::Box { r: l.rect, radius: 8.0, fill: note_fill(l.colour), edge: Some((edge, 1.2)) });
                items.push(Item::Box {
                    r: Rect::from_min_size(l.rect.min, Vec2::new(4.0, l.rect.height())),
                    radius: 0.0,
                    fill: edge,
                    edge: None,
                });
                for (k, line) in wrap(&l.text, NOTE_TEXT, crate::notes::NOTE_WIDTH - 20.0).into_iter().enumerate() {
                    let at = Pos2::new(l.rect.min.x + 10.0, l.rect.min.y + 9.0 + NOTE_TEXT + k as f32 * NOTE_LINE);
                    items.push(Item::Text { at, size: NOTE_TEXT, text: line, col: INK });
                }
            }
        }
    }
    for w in &wires {
        let col = if w.colour.a() == 0 { GREY_WIRE } else { crate::theme::readable(w.colour, WHITE, 3.0) };
        items.push(Item::Wire { pts: w.points.clone(), col, w: if w.setting { 1.2 } else { 1.7 } });
    }
    for b in &boxes {
        draw_box(b, &mut items);
    }
    if items.is_empty() {
        return None;
    }
    Some(Scope { items, nodes })
}

fn bounds_of(items: &[Item]) -> Rect {
    items.iter().fold(Rect::NOTHING, |r, i| r.union(i.bounds()))
}

/// A scope with what is inside its containers framed under it, each with a title (a group's frame, the stack of
/// cards of its box standing for it above).
fn scope_with_groups(
    model: &Model,
    o: &Options,
    path: &[String],
    only: Option<&[String]>,
    depth: usize,
) -> Option<Scope> {
    let mut main = scope(model, o, path, only)?;
    if !o.groups || depth >= DEEP {
        return Some(main);
    }
    let ids: Vec<(String, String)> = model
        .scope(path)?
        .iter()
        .filter(|n| n.body.is_some() && only.is_none_or(|k| k.contains(&n.id)))
        .map(|n| (n.id.clone(), crate::editor_view::op_label(n)))
        .collect();
    let look = group_colours();
    let left = bounds_of(&main.items).min.x;
    let mut y = bounds_of(&main.items).max.y + 70.0;
    for (id, label) in ids {
        let mut inner_path = path.to_vec();
        inner_path.push(id.clone());
        let Some(inner) = scope_with_groups(model, o, &inner_path, None, depth + 1) else { continue };
        let b = bounds_of(&inner.items);
        let frame = b.expand2(Vec2::new(18.0, 18.0));
        let frame = Rect::from_min_max(Pos2::new(frame.min.x, frame.min.y - 30.0), frame.max);
        let d = Vec2::new(left - frame.min.x, y - frame.min.y);
        let fill = WHITE.lerp_to_gamma(ACCENT, 0.04);
        main.items.push(Item::Box { r: frame.translate(d), radius: 8.0, fill, edge: Some((look.border, 1.5)) });
        let title = format!("Inside {id} ({label})");
        main.items.push(Item::Text {
            at: Pos2::new(frame.min.x + 12.0 + d.x, frame.min.y + 21.0 + d.y),
            size: FRAME_TITLE,
            text: title,
            col: look.ink,
        });
        for mut it in inner.items {
            it.shift(d);
            main.items.push(it);
        }
        main.nodes += inner.nodes;
        y = frame.translate(d).max.y + 50.0;
    }
    Some(main)
}

/// The model's graph as a figure.
pub fn build(model: &Model, o: &Options) -> Result<Figure, String> {
    let only = o.only.as_deref();
    let sc = scope_with_groups(model, o, &o.path, only, 0).ok_or("that group is not in the model")?;
    if sc.nodes == 0 && !sc.items.iter().any(|i| matches!(i, Item::Box { .. })) {
        return Err("the model has no nodes to draw".into());
    }
    let b = bounds_of(&sc.items);
    let (w_units, h_units) = (b.width().max(1.0), b.height().max(1.0));
    // the scale: a unit in points
    let (scale, width_mm) = match o.width {
        Width::Natural => (NATURAL, (w_units * NATURAL + 2.0 * MARGIN) * 25.4 / 72.0),
        Width::Mm(mm) => (((mm * 72.0 / 25.4 - 2.0 * MARGIN) / w_units).max(1e-4), mm),
    };
    let total_w = w_units * scale + 2.0 * MARGIN;
    let total_h = h_units * scale + 2.0 * MARGIN;
    let mut d = Drawing::blank(total_w, total_h);
    let at = |p: Pos2| ((p.x - b.min.x) * scale + MARGIN, (p.y - b.min.y) * scale + MARGIN);
    let line = |w: f32| (w * scale).max(0.3);
    for it in &sc.items {
        match it {
            Item::Box { r, radius, fill, edge } => {
                let pts: Vec<(f32, f32)> = rounded(*r, *radius).into_iter().map(|(x, y)| at(Pos2::new(x, y))).collect();
                d.path(vec![pts], true, Some(*fill), edge.map(|(c, w)| (c, line(w))));
            }
            Item::Wire { pts, col, w } => {
                let pts: Vec<(f32, f32)> = pts.iter().map(|p| at(*p)).collect();
                d.path(vec![pts], false, None, Some((*col, line(*w))));
            }
            Item::Disc { c, r, fill, edge } => {
                let pts: Vec<(f32, f32)> = ring(*c, *r).into_iter().map(|(x, y)| at(Pos2::new(x, y))).collect();
                d.path(vec![pts], true, *fill, edge.map(|(c, w)| (c, line(w))));
            }
            Item::Text { at: p, size, text, col } => d.label(at(*p), size * scale, text, false, *col, false),
        }
    }
    Ok(Figure { drawing: d, nodes: sc.nodes, min_text_pt: FILE * scale, width_mm })
}

/// The figure written to a file in the format its extension asks for (SVG when it says none). The figure, to tell
/// of its size and warnings.
pub fn save(model: &Model, o: &Options, path: &std::path::Path) -> Result<Figure, String> {
    let f = build(model, o)?;
    let title = model.name.clone().unwrap_or_else(|| "Terraflux model".to_string());
    let bytes = match Format::of_path(path) {
        Format::Svg => f.svg().into_bytes(),
        Format::Pdf => f.pdf(&title),
        Format::Png => f.png(300)?,
    };
    std::fs::write(path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(f)
}

/// The window of the export: the format, the width, what to draw; the nodes it is of.
#[derive(Debug, Default)]
pub struct Window {
    pub open: bool,
    pub format: Option<Format>,
    pub width: Option<Width>,
    pub no_notes: bool,
    pub no_settings: bool,
    pub no_groups: bool,
    /// The scope and the nodes it is of (a selection, a group).
    pub path: Vec<String>,
    pub only: Option<Vec<String>>,
    pub said: Option<Result<String, String>>,
}

/// Open the window for the whole model, or a selection or a group of it.
pub fn open(s: &mut crate::app::Session, path: Vec<String>, only: Option<Vec<String>>) {
    let w = &mut s.graph_figure;
    w.open = true;
    w.path = path;
    w.only = only;
    w.said = None;
}

pub fn window(ctx: &egui::Context, s: &mut crate::app::Session) {
    if !s.graph_figure.open {
        return;
    }
    let mut open = true;
    let mut save_as: Option<(std::path::PathBuf, Options)> = None;
    {
        let w = &mut s.graph_figure;
        let format = *w.format.get_or_insert(Format::Svg);
        let width = *w.width.get_or_insert(Width::Mm(180.0));
        egui::Window::new("Export the model as a figure")
            .open(&mut open)
            .default_size([380.0, 300.0])
            .resizable(false)
            .collapsible(false)
            .show(ctx, |ui| {
                let what = match (&w.only, w.path.last()) {
                    (Some(k), _) => format!("the {} nodes selected", k.len()),
                    (None, Some(g)) => format!("the inside of {g}"),
                    (None, None) => "the whole model".to_string(),
                };
                ui.label(
                    egui::RichText::new(format!("The graph of {what}, as the canvas draws it, in the light theme."))
                        .size(11.5)
                        .weak(),
                );
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Format");
                    egui::ComboBox::from_id_salt("graph figure format").selected_text(format.name()).show_ui(
                        ui,
                        |ui| {
                            for f in Format::ALL {
                                ui.selectable_value(&mut w.format, Some(f), f.name());
                            }
                        },
                    );
                });
                ui.horizontal(|ui| {
                    ui.label("Width");
                    egui::ComboBox::from_id_salt("graph figure width")
                        .selected_text(Width::CHOICES.iter().find(|c| c.0 == width).map_or("custom", |c| c.1))
                        .show_ui(ui, |ui| {
                            for (v, name) in Width::CHOICES {
                                ui.selectable_value(&mut w.width, Some(v), name);
                            }
                        });
                });
                let mut notes = !w.no_notes;
                let mut settings = !w.no_settings;
                let mut groups = !w.no_groups;
                ui.checkbox(&mut notes, "Include the notes and frames");
                ui.checkbox(&mut settings, "Include the settings shown on the boxes");
                ui.checkbox(&mut groups, "Show what is inside the groups, framed under the graph");
                (w.no_notes, w.no_settings, w.no_groups) = (!notes, !settings, !groups);
                ui.add_space(6.0);
                if ui.button("Save the figure...").clicked() {
                    let (ext, kind) = match format {
                        Format::Svg => ("svg", "SVG drawing"),
                        Format::Pdf => ("pdf", "PDF document"),
                        Format::Png => ("png", "PNG picture"),
                    };
                    if let Some(p) = rfd::FileDialog::new()
                        .set_title("Save the figure")
                        .add_filter(kind, &[ext])
                        .set_file_name(format!("graph.{ext}"))
                        .save_file()
                    {
                        let o = Options {
                            width: w.width.unwrap_or(Width::Mm(180.0)),
                            notes,
                            settings,
                            groups,
                            path: w.path.clone(),
                            only: w.only.clone(),
                        };
                        save_as = Some((p, o));
                    }
                }
                match &w.said {
                    Some(Ok(t)) => ui.label(egui::RichText::new(t).size(11.0).weak()),
                    Some(Err(t)) => ui.label(egui::RichText::new(t).size(11.0).color(ui.visuals().error_fg_color)),
                    None => ui.label(
                        egui::RichText::new("SVG and PDF keep the text as text and the lines as lines.")
                            .size(11.0)
                            .weak(),
                    ),
                };
            });
    }
    if let Some((p, o)) = save_as {
        s.graph_figure.said = Some(match save(&s.source, &o, &p) {
            Ok(f) => {
                let mut t = format!("saved {} ({} nodes, {:.0} mm wide)", p.display(), f.nodes, f.width_mm);
                if let Some(w) = f.warning() {
                    t.push_str(&format!("; {w}"));
                }
                Ok(t)
            }
            Err(e) => Err(e),
        });
    }
    if !open {
        s.graph_figure.open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::{Node, Note};

    /// Three nodes (a chain), a group with two inside, a frame and a note.
    fn model() -> Model {
        let mut m = Model::new("figure test");
        let mut a = Node::new("dem", "nlm.random_cluster").param("rows", 40).param("columns", 40);
        a.pos = Some([0.0, 0.0]);
        let mut b = Node::new("slope", "raster.reclassify")
            .param(
                "rules",
                "2 -> 2
* -> 1",
            )
            .input("map", "dem.map");
        b.pos = Some([320.0, 0.0]);
        let mut c = Node::new("calc", "raster.calc").param("expression", "a * 2").input("a", "slope.map");
        c.pos = Some([640.0, 0.0]);
        m.nodes = vec![a, b, c];
        m.notes.push(Note {
            id: "note1".into(),
            text: "The terrain & its slope".into(),
            at: Some([0.0, 300.0]),
            ..Default::default()
        });
        m.notes.push(Note {
            id: "frame1".into(),
            text: "Terrain".into(),
            nodes: vec!["dem".into(), "slope".into()],
            ..Default::default()
        });
        m
    }

    fn with_group() -> Model {
        let mut m = model();
        let mut inner = Node::new("inner", "raster.reclassify");
        inner.pos = Some([0.0, 0.0]);
        let mut g = Node::new("grp", terraflux_graph::GROUP);
        g.pos = Some([960.0, 0.0]);
        g.body = Some(terraflux_graph::Body { nodes: vec![inner], ..Default::default() });
        m.nodes.push(g);
        m
    }

    #[test]
    fn a_graph_as_svg() {
        let m = model();
        let f = build(&m, &Options::default()).unwrap();
        let svg = f.svg();
        assert!(svg.starts_with("<?xml") && svg.trim_end().ends_with("</svg>"));
        // the ids and operators as the boxes name them
        for t in ["dem", "slope", "calc", "raster.reclassify", "raster.calc", "nlm.random_cluster"] {
            assert!(svg.contains(&format!(">{t}</text>")), "{t}\n{svg}");
        }
        // a path for each wire (the rules over the settings and the files left out: two connections, two paths
        // that are not closed)
        let plain =
            build(&m, &Options { settings: false, notes: false, groups: false, ..Options::default() }).unwrap().svg();
        let wires = plain
            .lines()
            .filter(|l| l.starts_with("<path") && l.contains("fill=\"none\"") && !l.contains(" Z"))
            .count();
        assert_eq!(wires, 2, "{plain}");
        // the frame's title and the note's text (escaped)
        assert!(svg.contains(">Terrain</text>"));
        assert!(svg.contains("The terrain &amp; its slope"));
        // the settings rows: a name and its value
        assert!(svg.contains(">expression</text>") && svg.contains(">a * 2</text>"));
        assert_eq!(f.nodes, 3);
        // the width in millimetres and a viewBox as the content
        assert!(svg.contains("width=\"180.00mm\""));
        let vb = svg.split("viewBox=\"").nth(1).unwrap().split('"').next().unwrap();
        let v: Vec<f32> = vb.split(' ').map(|x| x.parse().unwrap()).collect();
        assert_eq!((v[0], v[1]), (0.0, 0.0));
        assert!((v[2] - 180.0 * 72.0 / 25.4).abs() < 0.01);
        assert!((v[2] / v[3] - f.drawing.w / f.drawing.h).abs() < 1e-3);
        assert!(svg.contains("sans-serif"));
    }

    #[test]
    fn it_parses_as_xml() {
        let svg = build(&with_group(), &Options::default()).unwrap().svg();
        // well formed: tags balance, no stray `&` or `<` in the texts
        let mut depth = 0i32;
        let mut rest = svg.as_str();
        while let Some(i) = rest.find('<') {
            let end = rest[i..].find('>').expect("a tag left open") + i;
            let tag = &rest[i + 1..end];
            if tag.starts_with('?') || tag.ends_with('/') {
            } else if tag.starts_with('/') {
                depth -= 1;
            } else {
                depth += 1;
            }
            assert!(depth >= 0);
            rest = &rest[end + 1..];
            // the text before the next tag has no `<` or loose `&`
            let text = &rest[..rest.find('<').unwrap_or(rest.len())];
            assert!(!text.contains('>'));
            assert!(text.split('&').skip(1).all(|p| ["amp;", "lt;", "gt;", "quot;"].iter().any(|e| p.starts_with(e))));
        }
        assert_eq!(depth, 0);
    }

    #[test]
    fn groups_notes_and_the_selection() {
        let m = with_group();
        let svg = build(&m, &Options::default()).unwrap().svg();
        // the group's box and its body framed with a title
        assert!(svg.contains(">grp</text>") && svg.contains(">control.group \u{b7} 1 node</text>"), "{svg}");
        assert!(svg.contains(">Inside grp (control.group)</text>") && svg.contains(">inner</text>"));
        let closed = build(&m, &Options { groups: false, ..Options::default() }).unwrap().svg();
        assert!(!closed.contains("Inside grp") && !closed.contains(">inner</text>"));
        let bare =
            build(&m, &Options { notes: false, settings: false, groups: false, ..Options::default() }).unwrap().svg();
        assert!(!bare.contains("Terrain") && !bare.contains(">expression</text>"));
        assert!(bare.len() < closed.len());
        // a selection: only those, what they read from the others as an input
        let one = build(&m, &Options { only: Some(vec!["slope".into()]), ..Options::default() }).unwrap();
        let s = one.svg();
        assert_eq!(one.nodes, 1);
        assert!(s.contains(">slope</text>") && !s.contains(">calc</text>") && !s.contains(">grp</text>"));
        assert!(s.contains(">dem.map</text>"), "what it reads, as an input: {s}");
        // the inside of a group alone
        let inside = build(&m, &Options { path: vec!["grp".into()], ..Options::default() }).unwrap();
        assert_eq!(inside.nodes, 1);
        assert!(build(&m, &Options { path: vec!["nope".into()], ..Options::default() }).is_err());
        assert!(build(&Model::new("empty"), &Options::default()).is_err());
    }

    #[test]
    fn the_width_in_millimetres_and_the_other_formats() {
        let m = model();
        let w85 = build(&m, &Options { width: Width::Mm(85.0), ..Options::default() }).unwrap();
        assert!(w85.svg().contains("width=\"85.00mm\""));
        assert!((w85.drawing.w - 85.0 * 72.0 / 25.4).abs() < 0.01);
        let nat = build(&m, &Options { width: Width::Natural, ..Options::default() }).unwrap();
        assert!(nat.min_text_pt > w85.min_text_pt && nat.warning().is_none());
        assert!(w85.warning().is_some_and(|w| w.contains("pt")));
        // PDF of the size of the figure, PNG with its signature
        let pdf = String::from_utf8_lossy(&w85.pdf("A model")).to_string();
        assert!(pdf.starts_with("%PDF-1.4") && pdf.contains("/MediaBox [0 0 240.9"), "{}", &pdf[..300]);
        let png = w85.png(150).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        let dir = std::env::temp_dir().join(format!("tf-graph-figure-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["svg", "pdf", "png"] {
            let p = dir.join(format!("g.{ext}"));
            save(&m, &Options::default(), &p).unwrap();
            assert!(std::fs::metadata(&p).unwrap().len() > 500);
        }
        std::fs::remove_dir_all(&dir).ok();
        assert_eq!(Width::parse("85mm"), Ok(Width::Mm(85.0)));
        assert_eq!(Width::parse("18 cm"), Ok(Width::Mm(180.0)));
        assert_eq!(Width::parse("natural"), Ok(Width::Natural));
        assert_eq!(Width::parse("120"), Ok(Width::Mm(120.0)));
        assert!(Width::parse("wide").is_err() && Width::parse("5mm").is_err() && Width::parse("3furlong").is_err());
    }

    /// The first template drawn, to look at: `TF_FIGURE_EXAMPLE=<folder> cargo test -p terraflux-app --lib
    /// graph_example -- --ignored`.
    #[test]
    #[ignore = "writes files for a person to look at"]
    fn graph_example() {
        let dir = std::path::PathBuf::from(std::env::var("TF_FIGURE_EXAMPLE").expect("a folder"));
        let t = &crate::templates::TEMPLATES[0];
        let mut m = t.model();
        m.notes.push(Note {
            id: "note1".into(),
            text: "Drawn by chance: in its place, a map of yours.".into(),
            at: Some([0.0, -170.0]),
            ..Default::default()
        });
        m.save(dir.join("template.toml")).unwrap();
        for (name, width) in [("85", Width::Mm(85.0)), ("180", Width::Mm(180.0)), ("natural", Width::Natural)] {
            let f = build(&m, &Options { width, ..Options::default() }).unwrap();
            std::fs::write(dir.join(format!("graph_{name}.svg")), f.svg()).unwrap();
            std::fs::write(dir.join(format!("graph_{name}.png")), f.png(200).unwrap()).unwrap();
            std::fs::write(dir.join(format!("graph_{name}.pdf")), f.pdf(t.name)).unwrap();
            eprintln!("{name}: {} nodes, {:.1} pt", f.nodes, f.min_text_pt);
        }
    }

    /// The inks read on paper: text 4.5:1 over every fill it is drawn on, wires and box edges 3:1 over white.
    #[test]
    fn the_inks_read_on_paper() {
        use crate::theme::contrast;
        let mut fills = vec![WHITE, SOURCE_FILL, WHITE.lerp_to_gamma(ACCENT, 0.04)];
        for t in
            [Type::Map, Type::Raster, Type::Stack, Type::Series, Type::Table, Type::Number, Type::Vector, Type::Agents]
        {
            fills.push(tint_of(WHITE, t).unwrap());
            fills.push(tint_of(SOURCE_FILL, t).unwrap());
        }
        for n in terraflux_graph::NOTE_COLOURS {
            let c = crate::notes::colour(Some(n));
            fills.push(note_fill(c));
            fills.push(frame_fill(c));
            let edge = crate::notes::inks(c, false, WHITE).0;
            assert!(contrast(title_ink(c), frame_fill(c)) >= 4.5, "{n} title");
            assert!(contrast(edge, WHITE) >= 3.0, "{n} edge");
        }
        for fill in &fills {
            for ink in [INK, WEAK] {
                assert!(contrast(ink, *fill) >= 4.5, "{ink:?} on {fill:?}: {}", contrast(ink, *fill));
            }
        }
        let g = group_colours();
        assert!(contrast(Color32::from_rgb(17, 17, 17), g.head) >= 4.5, "the group's title");
        assert!(contrast(g.ink, WHITE.lerp_to_gamma(ACCENT, 0.04)) >= 4.5, "the inside's title");
        assert!(contrast(g.border, WHITE) >= 3.0 && contrast(EDGE, WHITE) >= 4.5 && contrast(WARN, WHITE) >= 4.5);
        for t in [Some(Type::Map), Some(Type::Table), Some(Type::Number), Some(Type::Vector), Some(Type::Agents), None]
        {
            assert!(contrast(wire_ink(t), WHITE) >= 3.0, "{t:?}");
        }
        assert!(contrast(GREY_WIRE, WHITE) >= 3.0);
    }
}
