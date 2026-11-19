//! The hypotheses of the classes of an `objects.resolve` node in the Viewer: each class's membership map as a layer
//! over the map (in the class's colour, the deeper the greater the membership), and under the pointer each class's
//! membership at that cell, the winner, the doubt (the second highest) and "conflict" when the doubt reaches the node's
//! `min`. Shown for any output of the node, and for the stack of memberships it reads when that is what is shown.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use egui::{Color32, RichText, Ui};
use terraflux_graph::Source as Wire;
use terraflux_ops::objects::{arbitrate, explain_cell};
use terraflux_ops::{Map, Value};
use terraflux_raster::Grid;

use crate::app::Session;
use crate::map_view::{RasterLayer, Source};
use crate::palette::{Coloring, class_colour};

/// The levels a membership is drawn in (a membership of 0.1 to 0.2 the first, 0.8 to 1 the last).
const LEVELS: usize = 5;

/// What the Viewer shows of the hypotheses: whether, which classes, and the cell last under the pointer.
#[derive(Default)]
pub struct Hyp {
    pub on: bool,
    pub hidden: BTreeSet<usize>,
    pub cursor: Option<(usize, usize)>,
    /// The maps drawn, made for the layers of this address.
    cache: Option<(usize, Vec<Map>)>,
}

/// The state of an output's hypotheses (kept in the context, by the view's name and the output).
pub fn state(ctx: &egui::Context, salt: &str, node: &str, port: &str) -> Arc<Mutex<Hyp>> {
    let id = egui::Id::new(("hypotheses", salt, node, port));
    ctx.data_mut(|d| d.get_temp_mut_or_insert_with::<Arc<Mutex<Hyp>>>(id, Default::default).clone())
}

/// The cell under the pointer (the last one, when it leaves the map).
pub fn set_cursor(ctx: &egui::Context, salt: &str, node: &str, port: &str, cell: Option<(usize, usize)>) {
    if let Some(c) = cell {
        state(ctx, salt, node, port).lock().unwrap_or_else(|e| e.into_inner()).cursor = Some(c);
    }
}

/// The memberships an output is of: the stack an `objects.resolve` node reads and its settings.
pub struct Found {
    /// The node of `objects.resolve`.
    pub resolve: String,
    pub grid: Arc<Grid>,
    pub names: Vec<String>,
    pub layers: Vec<Arc<Vec<f64>>>,
    pub min: f64,
    pub order: Option<Vec<String>>,
}

/// The hypotheses behind the output `node.port`: when it is of an `objects.resolve` node, that node's `memberships`;
/// when it is the stack such a node reads, itself.
pub fn find(s: &Session, node: &str, port: &str) -> Option<Found> {
    let stack_of = |n: &terraflux_graph::Node| match n.inputs.get("memberships")? {
        Wire::Ref(r) => {
            let (id, p) = r.split_once('.')?;
            match s.value_of(id, p)? {
                Value::Stack(st) => Some(st),
                _ => None,
            }
        }
        Wire::Literal(_) => None,
    };
    let resolve = s.source.nodes.iter().find(|n| n.op == "objects.resolve" && n.id == node).or_else(|| {
        let want = format!("{node}.{port}");
        s.source.nodes.iter().find(|n| {
            n.op == "objects.resolve" && matches!(n.inputs.get("memberships"), Some(Wire::Ref(r)) if *r == want)
        })
    })?;
    let st = stack_of(resolve)?;
    Some(Found {
        resolve: resolve.id.clone(),
        grid: st.grid.clone(),
        names: st.keys.iter().map(|k| k.to_string()).collect(),
        layers: st.layers.clone(),
        min: resolve.params.get("min").and_then(|v| v.as_f64()).unwrap_or(0.1),
        order: resolve
            .params
            .get("order")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(String::from)).collect()),
    })
}

/// The classes' colour: the standard palette's, by the layer's place (as the classes of the resolved objects are).
fn colour(k: usize) -> Color32 {
    class_colour(k)
}

/// A membership map as levels 1 to [`LEVELS`] (NaN under 0.1 and where there is none), to be drawn in a class's colour.
fn levels(layer: &[f64]) -> Vec<f64> {
    layer
        .iter()
        .map(|&m| if m.is_nan() || m < 0.1 { f64::NAN } else { (m * LEVELS as f64).ceil().clamp(1.0, LEVELS as f64) })
        .collect()
}

/// The layers to draw over the map for an output: a map of each class that is shown, in its colour (none when the
/// toggle is off or the output has no hypotheses).
pub fn layers(ctx: &egui::Context, s: &Session, node: &str, port: &str, salt: &str) -> Vec<RasterLayer> {
    let st = state(ctx, salt, node, port);
    let mut h = st.lock().unwrap_or_else(|e| e.into_inner());
    if !h.on {
        return Vec::new();
    }
    let Some(f) = find(s, node, port) else { return Vec::new() };
    let key = f.layers.first().map_or(0, |l| Arc::as_ptr(l) as usize);
    if h.cache.as_ref().is_none_or(|(k, m)| *k != key || m.len() != f.layers.len()) {
        let maps = f.layers.iter().map(|l| Map::new(f.grid.clone(), levels(l))).collect();
        h.cache = Some((key, maps));
    }
    let Some((_, maps)) = &h.cache else { return Vec::new() };
    maps.iter()
        .enumerate()
        .filter(|(k, _)| !h.hidden.contains(k))
        .map(|(k, m)| {
            let c = colour(k);
            let steps =
                (1..=LEVELS).map(|l| (l as f64, c.gamma_multiply(0.25 + 0.75 * l as f32 / LEVELS as f32))).collect();
            RasterLayer {
                source: Source::from(m),
                coloring: Coloring::Classes(steps),
                opacity: 0.85,
                below: false,
                name: format!("hypothesis {}", f.names.get(k).map_or("?", |n| n.as_str())),
                blend: Default::default(),
            }
        })
        .collect()
}

/// The line under the pointer: each class's membership at the cell, the winner, the doubt and the conflict. One
/// string per line (the sentence of [`explain_cell`] last).
pub fn cell_lines(f: &Found, row: usize, col: usize) -> Option<Vec<String>> {
    let (w, h) = (f.grid.width, f.grid.height);
    if row >= h || col >= w {
        return None;
    }
    let values: Vec<f64> = f.layers.iter().map(|l| l[row * w + col]).collect();
    let order: Vec<usize> = f
        .order
        .as_deref()
        .map(|o| {
            let first: Vec<usize> = o.iter().filter_map(|n| f.names.iter().position(|x| x == n)).collect();
            first.iter().copied().chain((0..f.names.len()).filter(|c| !first.contains(c))).collect()
        })
        .unwrap_or_default();
    let v = arbitrate(&values, &order, f.min);
    let mut lines: Vec<String> = f
        .names
        .iter()
        .zip(&values)
        .enumerate()
        .map(|(k, (n, m))| {
            let m = if m.is_nan() { 0.0 } else { *m };
            let tag = if v.winner == Some(k) {
                "  winner"
            } else if v.second == Some(k) && v.doubt > 0.0 {
                "  doubt"
            } else {
                ""
            };
            format!("{n} {m:.2}{tag}")
        })
        .collect();
    if v.conflict {
        lines.push(format!("conflict: the second reaches the minimum {}", f.min));
    }
    lines.push(explain_cell(&f.names, &values, f.order.as_deref(), f.min));
    Some(lines)
}

/// The side panel's section: the toggle, the classes, and what is under the pointer.
pub fn panel(ui: &mut Ui, s: &Session, node: &str, port: &str, salt: &str) {
    let Some(f) = find(s, node, port) else { return };
    let st = state(ui.ctx(), salt, node, port);
    let mut h = st.lock().unwrap_or_else(|e| e.into_inner());
    crate::theme::section(ui, "Hypotheses");
    ui.checkbox(&mut h.on, "hypotheses").on_hover_text(
        "each class's membership as a layer over the map, and under the pointer the winner and the doubt",
    );
    if h.on {
        for (k, n) in f.names.iter().enumerate() {
            let mut shown = !h.hidden.contains(&k);
            ui.horizontal(|ui| {
                ui.label(RichText::new("\u{25a0}").color(colour(k)));
                ui.checkbox(&mut shown, n.as_str());
            });
            if shown {
                h.hidden.remove(&k);
            } else {
                h.hidden.insert(k);
            }
        }
    }
    let lines = h.cursor.and_then(|(r, c)| cell_lines(&f, r, c).map(|l| (r, c, l)));
    match lines {
        Some((r, c, lines)) => {
            ui.label(RichText::new(format!("under the pointer (row {}, column {})", r + 1, c + 1)).weak().size(11.0));
            let last = lines.len() - 1;
            for (i, l) in lines.iter().enumerate() {
                let text = RichText::new(l).size(11.0);
                ui.label(if i == last { text.weak() } else { text });
            }
        }
        None => {
            ui.label(RichText::new("point at the map to see the hypotheses of a cell").weak().size(11.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found() -> Found {
        Found {
            resolve: "r".into(),
            grid: Arc::new(Grid::new(2, 1)),
            names: vec!["Trees".into(), "Grass".into()],
            layers: vec![Arc::new(vec![0.82, 0.0]), Arc::new(vec![0.64, 0.05])],
            min: 0.1,
            order: None,
        }
    }

    #[test]
    fn a_cell_tells_each_class_the_winner_and_the_conflict() {
        let f = found();
        let l = cell_lines(&f, 0, 0).unwrap();
        assert_eq!(l[0], "Trees 0.82  winner");
        assert_eq!(l[1], "Grass 0.64  doubt");
        assert!(l[2].starts_with("conflict"), "{l:?}");
        assert!(l[3].contains("Trees 0.82 won over Grass 0.64"), "{l:?}");
        // (a cell no class reaches the minimum in: no winner, no conflict)
        let l = cell_lines(&f, 0, 1).unwrap();
        assert!(l.iter().all(|x| !x.starts_with("conflict")), "{l:?}");
        assert!(l.last().unwrap().contains("no class"), "{l:?}");
        assert!(cell_lines(&f, 3, 0).is_none(), "off the grid");
    }

    #[test]
    fn memberships_are_drawn_in_levels() {
        let l = levels(&[f64::NAN, 0.05, 0.1, 0.5, 1.0]);
        assert!(l[0].is_nan() && l[1].is_nan());
        assert_eq!(&l[2..], &[1.0, 3.0, 5.0]);
    }
}
