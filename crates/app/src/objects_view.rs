//! Objects (see `terraflux_ops::objects`) on the screen: a layer of the Viewer whose fill is the class colour of each
//! object (or a colour of its own, or none) and whose outlines are drawn over it at a constant width on the screen,
//! the object under the pointer told and outlined, and the objects' table.
//!
//! The fill is a map (a number per cell, made once and kept) that goes through the tiles as any other map does. The
//! outlines are made for the part of the map in view only, as an image at the screen's resolution (a pixel is
//! coloured where the cell under it is another object's than the cell under the pixel right of it or below it): see
//! [`outline_image`], redone when the view, the objects or the style change and not otherwise.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use egui::{Color32, Ui};
use terraflux_ops::Map;
use terraflux_ops::legend::Class;
use terraflux_ops::objects::Objects;

use crate::map_view::Outline;
use crate::palette::{Coloring, class_colour};
use crate::vector_tiles::Style;

/// The outlines' colour when the style has not been touched (the style's own default is the theme's accent, which is
/// the colour of what is selected).
pub const OUTLINE: Color32 = Color32::from_rgb(255, 214, 10);
/// The most pixels of the image of the outlines (a side of 4096): a larger window is drawn at a coarser resolution.
pub const MOST_PIXELS: usize = 4096 * 4096;
/// The most cells of an object's box looked through to outline it; a larger one is drawn as its box.
const OUTLINE_CELLS: usize = 4_000_000;

/// How the objects are filled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Fill {
    /// Not at all: the outlines alone.
    None,
    /// By the colour of each object's class (objects without a class are not filled).
    Classes,
    /// Each object in a colour of its own, distinct from its neighbours'.
    Random,
}

impl Fill {
    pub const ALL: [Fill; 3] = [Fill::Classes, Fill::Random, Fill::None];

    pub fn name(self) -> &'static str {
        match self {
            Fill::None => "no fill",
            Fill::Classes => "by class",
            Fill::Random => "a colour each",
        }
    }
}

/// How a layer's objects are filled: the style's `by` says (none: by class when the objects have classes, else not
/// at all), so that a Plot keeps it with the rest of the style.
pub fn fill_of(style: &Style, o: &Objects) -> Fill {
    match style.by.as_deref() {
        Some("none") => Fill::None,
        Some("random") => Fill::Random,
        _ if o.classes.is_empty() => Fill::None,
        _ => Fill::Classes,
    }
}

/// Choose how the objects are filled (written in the style's `by`).
pub fn set_fill(style: &mut Style, fill: Fill) {
    style.by = match fill {
        Fill::Classes => None,
        Fill::Random => Some("random".into()),
        Fill::None => Some("none".into()),
    };
}

/// How many levels the objects have above them (the chain of `parent`).
pub fn levels_above(o: &Objects) -> usize {
    std::iter::successors(o.parent.as_ref(), |p| p.parent.as_ref()).count()
}

/// The objects `level` levels up the chain of parents (0: these; more than there are: the topmost).
pub fn at_level(o: &Arc<Objects>, level: usize) -> Arc<Objects> {
    let mut at = o.clone();
    for _ in 0..level {
        let Some(p) = at.parent.clone() else { break };
        at = p;
    }
    at
}

/// The outlines' colour: the style's, yellow while it is the untouched default.
pub fn outline_colour(style: &Style) -> Color32 {
    if style.colour == Style::default().colour { OUTLINE } else { style.colour }
}

/// What the fill of objects is: its map (a number per cell, NaN where there is none to colour), how it is coloured,
/// its legend (the classes' names and colours) and the cells of each class.
pub struct Filled {
    pub map: Map,
    pub coloring: Coloring,
    pub legend: Vec<Class>,
    pub counts: Vec<(f64, usize)>,
}

type Kept = ((usize, Fill, u64), Arc<Objects>, Arc<Filled>);

/// The fills made last (the map of a large image is big: a few are kept, the objects held with them so that their
/// labels' address is no other's).
static KEPT: Mutex<Vec<Kept>> = Mutex::new(Vec::new());

/// What tells two sets of classes apart.
fn classes_key(o: &Objects) -> u64 {
    let mut h = DefaultHasher::new();
    o.classes.len().hash(&mut h);
    for c in &o.classes {
        (&c.name, c.colour).hash(&mut h);
    }
    o.class_of.hash(&mut h);
    novel_flags(o).hash(&mut h);
    h.finish()
}

/// The objects objects.classify flagged outside what the model was trained on (the attribute `novel`, 1): None when
/// there are none. Their class is drawn hatched.
fn novel_flags(o: &Objects) -> Option<Vec<bool>> {
    let j = o.table.columns.iter().position(|c| c == "novel")?;
    let flags: Vec<bool> = o.table.rows.iter().map(|r| r[j] == 1.0).collect();
    flags.iter().any(|&f| f).then_some(flags)
}

/// The fill of objects in a way, made once while the objects are the same.
pub fn fill(o: &Arc<Objects>, how: Fill) -> Arc<Filled> {
    let key = (Arc::as_ptr(&o.labels) as usize, how, if how == Fill::Classes { classes_key(o) } else { 0 });
    let mut kept = KEPT.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(at) = kept.iter().position(|k| k.0 == key) {
        // (the last used last)
        let k = kept.remove(at);
        let f = k.2.clone();
        kept.push(k);
        return f;
    }
    let f = Arc::new(made(o, how));
    kept.push((key, o.clone(), f.clone()));
    while kept.len() > 6 {
        kept.remove(0);
    }
    f
}

/// The colour of an object's class: the one it was given, else the standard class palette's.
pub fn class_colour_of(o: &Objects, k: usize) -> Color32 {
    match o.classes[k - 1].colour {
        Some([r, g, b]) => Color32::from_rgb(r, g, b),
        None => class_colour(k - 1),
    }
}

/// A colour for an object, by its id: the hue steps by the golden ratio so that the ids next to each other are far
/// apart; the saturation and the lightness vary a little.
pub fn id_colour(id: u32) -> Color32 {
    let h = (f64::from(id) * 0.618_033_988_749_895).fract();
    let s = 0.55 + 0.3 * (f64::from(id % 3) / 2.0);
    let v = 0.75 + 0.2 * (f64::from(id % 5) / 4.0);
    let (r, g, b) = hsv(h, s, v);
    Color32::from_rgb(r, g, b)
}

fn hsv(h: f64, s: f64, v: f64) -> (u8, u8, u8) {
    let i = (h * 6.0).floor();
    let f = h * 6.0 - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - f * s), v * (1.0 - (1.0 - f) * s));
    let (r, g, b) = match i as i32 % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    ((r * 255.0).round() as u8, (g * 255.0).round() as u8, (b * 255.0).round() as u8)
}

fn made(o: &Objects, how: Fill) -> Filled {
    let n = o.grid.cells();
    let mut legend = Vec::new();
    let mut counts = Vec::new();
    let (data, coloring) = match how {
        Fill::None => (vec![f64::NAN; n], Coloring::Classes(Vec::new())),
        Fill::Classes => {
            let mut cells = vec![0usize; o.classes.len() + 1];
            for (k, &c) in o.class_of.iter().enumerate() {
                cells[c as usize] += o.cells[k] as usize;
            }
            let mut colours = Vec::new();
            for (k, &n) in cells.iter().enumerate().skip(1) {
                if n == 0 {
                    continue;
                }
                let col = class_colour_of(o, k);
                colours.push((k as f64, col));
                counts.push((k as f64, n));
                legend.push(Class {
                    value: k as f64,
                    name: o.classes[k - 1].name.clone(),
                    colour: format!("#{:02x}{:02x}{:02x}", col.r(), col.g(), col.b()),
                });
            }
            let mut data: Vec<f64> = o
                .labels
                .iter()
                .map(|&l| match l {
                    0 => f64::NAN,
                    l => match o.class_of[l as usize - 1] {
                        0 => f64::NAN,
                        k => f64::from(k),
                    },
                })
                .collect();
            // (the novel objects hatched: diagonal stripes of two cells, the image showing through the others)
            if let Some(flags) = novel_flags(o) {
                let w = o.grid.width.max(1);
                for (i, &l) in o.labels.iter().enumerate() {
                    if l != 0 && flags[l as usize - 1] && (i / w + i % w) % 4 < 2 {
                        data[i] = f64::NAN;
                    }
                }
            }
            (data, Coloring::Classes(colours))
        }
        Fill::Random => {
            let colours: Vec<f64> = (1..=o.count as u32)
                .map(|id| {
                    let c = id_colour(id);
                    f64::from(crate::rgb::pack(c.r(), c.g(), c.b()))
                })
                .collect();
            let data = o.labels.iter().map(|&l| if l == 0 { f64::NAN } else { colours[l as usize - 1] }).collect();
            (data, Coloring::Packed)
        }
    };
    // (the classes' names go with the map, as a map of classes carries its legend: a difference of two of them names
    // the transitions)
    let grid = if legend.is_empty() {
        o.grid.clone()
    } else {
        Arc::new(terraflux_ops::legend::with_legend(&o.grid, Some(&legend)))
    };
    Filled { map: Map::new(grid, data), coloring, legend, counts }
}

/// The text of an object under the pointer: `object 12 · class water · area 340 m²` (the cells when the cells are in
/// degrees). `id` 0 is no object.
pub fn object_text(o: &Objects, id: u32) -> String {
    if id == 0 || id as usize > o.count {
        return "no object".into();
    }
    let mut text = format!("object {id}");
    if let Some(c) = o.class_name(id) {
        text.push_str(&format!(" · class {c}"));
    }
    let cells = o.cells[id as usize - 1];
    // (cells of hundredths of a unit or less are degrees: no area in metres)
    if o.grid.transform[1].abs() < 0.01 {
        text.push_str(&format!(" · {cells} cells"));
    } else {
        let area = f64::from(cells) * o.cell_area();
        text.push_str(&format!(" · area {} m²", crate::map_view::cell_text(area.round())));
    }
    text
}

/// An object's outline: the sides of its cells on its border, or (an object whose box has too many cells to look
/// through, or that has too many sides) its box, as the map draws a patch.
pub fn object_outline(o: &Objects, id: u32) -> Option<Outline> {
    let b = *o.boxes.get((id as usize).checked_sub(1)?)?;
    let bounds = (b[1] as f32, b[0] as f32, b[3] as f32 + 1.0, b[2] as f32 + 1.0);
    let (rows, cols) = ((b[2] - b[0] + 1) as usize, (b[3] - b[1] + 1) as usize);
    if rows * cols > OUTLINE_CELLS {
        return Some(Outline { edges: Vec::new(), bounds });
    }
    let w = o.grid.width;
    let is = |r: i64, c: i64| {
        r >= 0
            && c >= 0
            && (r as usize) < o.grid.height
            && (c as usize) < w
            && o.labels[r as usize * w + c as usize] == id
    };
    let mut edges = Vec::new();
    for r in b[0]..=b[2] {
        for c in b[1]..=b[3] {
            let (ri, ci) = (i64::from(r), i64::from(c));
            if !is(ri, ci) {
                continue;
            }
            let (x, y) = (c as f32, r as f32);
            if !is(ri - 1, ci) {
                edges.push([(x, y), (x + 1.0, y)]);
            }
            if !is(ri + 1, ci) {
                edges.push([(x, y + 1.0), (x + 1.0, y + 1.0)]);
            }
            if !is(ri, ci - 1) {
                edges.push([(x, y), (x, y + 1.0)]);
            }
            if !is(ri, ci + 1) {
                edges.push([(x + 1.0, y), (x + 1.0, y + 1.0)]);
            }
        }
    }
    Some(Outline { edges, bounds })
}

/// The size in pixels of the outlines' image of a window of `w` x `h` pixels, and how many of its pixels make one of
/// the screen's: smaller than 1 when the window has more than `MOST_PIXELS`.
pub fn image_size(w: f32, h: f32, ppp: f32) -> ((usize, usize), f32) {
    let (pw, ph) = ((w * ppp).ceil().max(1.0), (h * ppp).ceil().max(1.0));
    // (and no side above 8192, a texture's most)
    let scale = if pw * ph > MOST_PIXELS as f32 { (MOST_PIXELS as f32 / (pw * ph)).sqrt() } else { 1.0 };
    let scale = scale.min(8192.0 / pw.max(ph));
    (((pw * scale).ceil() as usize, (ph * scale).ceil() as usize), ppp * scale)
}

/// The outlines of objects as an image of `cols.len()` x `rows.len()` pixels: `cols[x]` is the column of the objects'
/// cells under the pixel column x, `rows[y]` the row under the pixel row y (-1: none, off the grid). A pixel is
/// `colour` where the cell under it is another's than the one under the pixel right of it or below it (and not both
/// without an object), as wide as `width` screen points at `per_point` pixels to a point (a width under one pixel
/// is drawn fainter). Where a pixel covers many cells, the one cell under it is what is compared.
pub fn outline_image(
    labels: &[u32],
    w: usize,
    (cols, rows): (&[i32], &[i32]),
    colour: Color32,
    width: f32,
    per_point: f32,
) -> egui::ColorImage {
    let (pw, ph) = (cols.len(), rows.len());
    let at = |y: usize, x: usize| -> u32 {
        let (r, c) = (rows[y], cols[x]);
        if r < 0 || c < 0 { 0 } else { labels[r as usize * w + c as usize] }
    };
    let mut mask = vec![false; pw * ph];
    // two rows of labels at a time: each pixel's own, and the row below's
    let mut next: Vec<u32> = (0..pw).map(|x| if ph > 0 { at(0, x) } else { 0 }).collect();
    for y in 0..ph {
        let cur = std::mem::take(&mut next);
        next = if y + 1 < ph { (0..pw).map(|x| at(y + 1, x)).collect() } else { cur.clone() };
        let row = &mut mask[y * pw..(y + 1) * pw];
        for x in 0..pw {
            let a = cur[x];
            let right = if x + 1 < pw { cur[x + 1] } else { a };
            row[x] = a != right || a != next[x];
        }
    }
    // a line wider than a pixel: the marks spread over the pixels around
    let t = (width * per_point).max(0.1);
    let n = t.ceil().max(1.0) as usize;
    if n > 1 {
        let (before, after) = ((n - 1) / 2, n / 2);
        spread(&mut mask, pw, ph, (before, after));
    }
    let alpha = (t / n as f32).min(1.0) * f32::from(colour.a()) / 255.0;
    let paint = Color32::from_rgba_unmultiplied(colour.r(), colour.g(), colour.b(), (alpha * 255.0).round() as u8);
    let pixels = mask.iter().map(|&m| if m { paint } else { Color32::TRANSPARENT }).collect();
    egui::ColorImage::new([pw, ph], pixels)
}

/// Each marked pixel marks `before` pixels before it and `after` after it, along the rows and then the columns (one
/// pass each way over each, whatever the widths).
fn spread(mask: &mut [bool], w: usize, h: usize, (before, after): (usize, usize)) {
    const FAR: i64 = i64::MAX / 4;
    let (before, after) = (before as i64, after as i64);
    let mut out = vec![false; mask.len()];
    for y in 0..h {
        let (row, o) = (&mask[y * w..(y + 1) * w], &mut out[y * w..(y + 1) * w]);
        let mut last = -FAR;
        for x in 0..w {
            if row[x] {
                last = x as i64;
            }
            o[x] = x as i64 - last <= after;
        }
        let mut next = FAR;
        for x in (0..w).rev() {
            if row[x] {
                next = x as i64;
            }
            o[x] |= next - x as i64 <= before;
        }
    }
    let mut res = vec![false; mask.len()];
    let mut last = vec![-FAR; w];
    for y in 0..h {
        for x in 0..w {
            if out[y * w + x] {
                last[x] = y as i64;
            }
            res[y * w + x] = y as i64 - last[x] <= after;
        }
    }
    let mut next = vec![FAR; w];
    for y in (0..h).rev() {
        for x in 0..w {
            if out[y * w + x] {
                next[x] = y as i64;
            }
            res[y * w + x] |= next[x] - y as i64 <= before;
        }
    }
    mask.copy_from_slice(&res);
}

/// The lines of a layer's legend: a colour, a name and the cells of each class (none when it is not filled by class).
pub fn legend_lines(f: &Filled) -> Vec<(Color32, String, usize)> {
    match &f.coloring {
        Coloring::Classes(c) => crate::map_view::legend_lines(c, &f.counts, &|v| {
            f.legend.iter().find(|k| k.value == v).map(|k| k.name.clone()).unwrap_or_default()
        }),
        _ => Vec::new(),
    }
}

/// The controls of a layer of objects: how they are filled, the outlines' colour and width.
pub fn controls(ui: &mut Ui, style: &mut Style, o: &Objects, id: egui::Id) {
    // (objects inside those of a level above: the level drawn)
    let above = levels_above(o);
    if above > 0 {
        style.level = style.level.min(above);
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Level").size(11.5));
            let count = |k: usize| {
                let mut at = o;
                for _ in 0..k {
                    at = at.parent.as_deref().unwrap_or(at);
                }
                at.count
            };
            let name = |k: usize| match k {
                0 => format!("this level ({} objects)", count(0)),
                1 => format!("the level above ({} objects)", count(1)),
                k => format!("{k} levels above ({} objects)", count(k)),
            };
            egui::ComboBox::from_id_salt(id.with("level"))
                .selected_text(name(style.level))
                .show_ui(ui, |ui| {
                    for k in 0..=above {
                        if ui.add(egui::Button::selectable(style.level == k, name(k))).clicked() {
                            style.level = k;
                        }
                    }
                })
                .response
                .on_hover_text("the objects of this node, or of the level above them (their parents)");
        });
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Fill").size(11.5));
        let now = fill_of(style, o);
        egui::ComboBox::from_id_salt(id.with("fill"))
            .selected_text(now.name())
            .show_ui(ui, |ui| {
                for f in Fill::ALL {
                    let enabled = f != Fill::Classes || !o.classes.is_empty();
                    if ui.add_enabled(enabled, egui::Button::selectable(now == f, f.name())).clicked() {
                        set_fill(style, f);
                    }
                }
            })
            .response
            .on_hover_text("each object in the colour of its class, in a colour of its own, or not at all");
    });
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Outline").size(11.5));
        let was = outline_colour(style);
        let mut colour = was;
        ui.color_edit_button_srgba(&mut colour);
        for (name, c) in [("white", Color32::WHITE), ("black", Color32::BLACK)] {
            if ui.small_button(name).clicked() {
                colour = c;
            }
        }
        if colour != was {
            style.colour = colour;
        }
    });
    ui.spacing_mut().slider_width = (ui.available_width() - 130.0).clamp(40.0, 160.0);
    ui.add(egui::Slider::new(&mut style.width, 0.5..=4.0).step_by(0.1).text("width"))
        .on_hover_text("pixels on the screen, the same at every zoom");
}

/// The objects' summary and attribute table, in the space given (the table scrolls inside it); the row picked is
/// kept in `pick` for the map to outline it.
pub fn table_panel(ui: &mut Ui, o: &Objects, name: &str, id: egui::Id, pick: &mut crate::table_view::Pick) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(name).strong());
        ui.label(egui::RichText::new(o.describe()).weak());
    });
    crate::table_view::show_picking(ui, &o.table, id, Some(pick));
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::objects::ObjectClass;
    use terraflux_raster::Grid;

    /// Objects on a grid of 6 x 4: the left three columns object 1, the right three object 2, the last row none.
    fn two() -> Arc<Objects> {
        let grid = Arc::new(Grid::new(6, 4));
        let labels: Vec<u32> = (0..24)
            .map(|i| match (i / 6, i % 6) {
                (3, _) => 0,
                (_, c) if c < 3 => 1,
                _ => 2,
            })
            .collect();
        Arc::new(Objects::from_labels(grid, labels, 2).unwrap())
    }

    fn classed() -> Arc<Objects> {
        let o = (*two()).clone();
        let classes = vec![
            ObjectClass { name: "water".into(), colour: Some([0, 0, 255]) },
            ObjectClass { name: "forest".into(), colour: None },
        ];
        Arc::new(o.with_classes(classes, vec![1, 2], Vec::new()).unwrap())
    }

    #[test]
    fn the_fill_is_by_class_when_there_are_classes() {
        let (plain, classed) = (two(), classed());
        let style = Style::default();
        assert_eq!(fill_of(&style, &plain), Fill::None);
        assert_eq!(fill_of(&style, &classed), Fill::Classes);
        let mut s = style.clone();
        set_fill(&mut s, Fill::Random);
        assert_eq!(fill_of(&s, &plain), Fill::Random);
        set_fill(&mut s, Fill::None);
        assert_eq!(fill_of(&s, &classed), Fill::None);
        // the choice is kept with the style
        assert_eq!(fill_of(&Style::from_json(&s.to_json()), &classed), Fill::None);
        let f = fill(&classed, Fill::Classes);
        // (the first class's own colour, the second's from the palette; the last row is not filled)
        let Coloring::Classes(c) = &f.coloring else { panic!("classes") };
        assert_eq!(c, &vec![(1.0, Color32::from_rgb(0, 0, 255)), (2.0, class_colour(1))]);
        assert_eq!(f.map.data[0], 1.0);
        assert_eq!(f.map.data[5], 2.0);
        assert!(f.map.data[18].is_nan());
        assert_eq!(f.counts, vec![(1.0, 9), (2.0, 9)]);
        assert_eq!(f.legend[0].name, "water");
        assert_eq!(f.legend[0].colour, "#0000ff");
        // (made once while the objects are the same)
        assert!(Arc::ptr_eq(&f, &fill(&classed, Fill::Classes)));
        // each in a colour of its own
        let r = fill(&plain, Fill::Random);
        assert_eq!(r.coloring, Coloring::Packed);
        assert_ne!(r.map.data[0], r.map.data[5]);
        assert_ne!(id_colour(1), id_colour(2));
    }

    #[test]
    fn the_object_under_the_pointer_is_told() {
        let o = classed();
        // (the grid of the tests has unit cells)
        let t = object_text(&o, 1);
        assert!(t.starts_with("object 1 · class water · "), "{t}");
        assert_eq!(object_text(&o, 0), "no object");
        let mut grid = Grid::new(6, 4);
        grid.transform = [0.0, 10.0, 0.0, 40.0, 0.0, -10.0];
        let metres = Objects::from_labels(Arc::new(grid), o.labels.to_vec(), 2).unwrap();
        // 9 cells of 100 m²
        assert_eq!(object_text(&metres, 2), "object 2 · area 900 m²");
    }

    #[test]
    fn an_objects_outline_is_the_sides_of_its_cells_on_its_border() {
        let o = two();
        let out = object_outline(&o, 1).unwrap();
        // 3 columns by 3 rows: 3 + 3 + 3 + 3 sides
        assert_eq!(out.edges.len(), 12);
        assert_eq!(out.bounds, (0.0, 0.0, 3.0, 3.0));
        assert!(object_outline(&o, 3).is_none());
    }

    #[test]
    fn the_outlines_are_where_the_object_changes() {
        let o = two();
        // one pixel a cell
        let cols: Vec<i32> = (0..6).collect();
        let rows: Vec<i32> = (0..4).collect();
        let red = Color32::from_rgb(255, 0, 0);
        let img = outline_image(&o.labels, 6, (&cols, &rows), red, 1.0, 1.0);
        let on = |x: usize, y: usize| img.pixels[y * 6 + x] != Color32::TRANSPARENT;
        // the border between the two objects: the column 2 (left of the change)
        assert!(on(2, 0) && on(2, 2));
        assert!(!on(1, 0) && !on(4, 1));
        // the border with no object: the row 2 (above the change); the row 3 has nothing under it
        assert!(on(0, 2) && on(5, 2));
        assert!(!on(0, 3) && !on(0, 0));
        assert_eq!(img.pixels[2], red);
        // a window off the grid on a side: those pixels are none, and the first column's border is drawn
        let cols: Vec<i32> = vec![-1, 0, 1, 2, 3, 4];
        let img = outline_image(&o.labels, 6, (&cols, &rows[..3]), red, 1.0, 1.0);
        assert!(img.pixels[0] != Color32::TRANSPARENT, "the grid's edge");
        // wider: more pixels
        let narrow = img.pixels.iter().filter(|p| **p != Color32::TRANSPARENT).count();
        let wide = outline_image(&o.labels, 6, (&cols, &rows[..3]), red, 3.0, 1.0);
        assert!(wide.pixels.iter().filter(|p| **p != Color32::TRANSPARENT).count() > narrow);
        // thinner than a pixel: fainter
        let faint = outline_image(&o.labels, 6, (&cols, &rows[..3]), red, 0.5, 1.0);
        assert!(faint.pixels.iter().any(|p| p.a() > 0 && p.a() < 255));
    }

    #[test]
    fn a_large_window_is_made_coarser() {
        let ((w, h), per) = image_size(1000.0, 800.0, 2.0);
        assert_eq!((w, h), (2000, 1600));
        assert_eq!(per, 2.0);
        let ((w, h), per) = image_size(10_000.0, 10_000.0, 2.0);
        assert!(w * h <= MOST_PIXELS + w + h, "{w} x {h}");
        assert!(per < 2.0);
    }

    #[test]
    fn the_novel_objects_are_drawn_hatched_over_their_class() {
        // object 2 is flagged novel by objects.classify's column `novel`: its cells are left out in diagonal stripes
        let mut t = terraflux_ops::Table::new(vec!["id".into(), "novel".into()]);
        t.rows = vec![vec![1.0, 0.0], vec![2.0, 1.0]];
        let hatched = Arc::new((*classed()).clone().with_table(t).unwrap());
        let (plain, f) = (fill(&classed(), Fill::Classes), fill(&hatched, Fill::Classes));
        let w = 6;
        for i in 0..24 {
            let (r, c) = (i / w, i % w);
            let (a, b) = (plain.map.data[i], f.map.data[i]);
            if r == 3 || c < 3 {
                // (no object, or the object that is not novel)
                assert!(a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()), "cell {r},{c}");
            } else if (r + c) % 4 < 2 {
                assert!(b.is_nan() && a == 2.0, "cell {r},{c}: the stripe lets the image through");
            } else {
                assert_eq!(b, 2.0, "cell {r},{c}: the class between the stripes");
            }
        }
        // (the legend and the cells of each class are those of the classes: the hatching is only how it is drawn)
        assert_eq!(f.counts, plain.counts);
        // another set of flags is another fill (the fill is kept by the objects and what they say)
        assert!(!Arc::ptr_eq(&plain, &f));
    }
}
