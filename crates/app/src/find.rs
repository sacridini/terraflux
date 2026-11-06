//! Find a node of the model (ctrl+F on the canvas): by its id, its operator, its note or a file its settings name,
//! at every depth (inside groups, loops, switches and each); the camera goes to the node chosen, the containers on
//! the way opened. The commands of the palette ("Go to group/inner") reach nodes the same way ([`go`]).

use egui::{RichText, Vec2};
use terraflux_graph::{Model, Node};
use terraflux_ops::Registry;

use crate::app::Session;

/// How many matches the list shows.
const MOST: usize = 12;

/// The search box while it is open.
#[derive(Debug, Default)]
pub struct Find {
    pub query: String,
    /// The match chosen.
    pub at: usize,
    /// Whether the box was given the keyboard's focus.
    focused: bool,
}

/// A node found.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The containers' ids from the top, then the node's.
    pub at: Vec<String>,
    pub op: String,
    /// The file its settings name, when that is what matched.
    pub file: Option<String>,
}

impl Hit {
    /// The node's id.
    pub fn id(&self) -> &str {
        self.at.last().map_or("", |s| s.as_str())
    }

    /// Where it is, `group > inner` (none for a node of the top level).
    pub fn place(&self) -> Option<String> {
        (self.at.len() > 1).then(|| self.at.join(" > "))
    }
}

/// The files a node's settings name: a text setting called path, file, folder or dir (or ending in one).
fn files(n: &Node) -> Vec<&str> {
    n.params
        .iter()
        .filter(|(k, _)| {
            let k = k.to_lowercase();
            ["path", "file", "folder", "dir"].iter().any(|w| k.contains(w))
        })
        .filter_map(|(_, v)| v.as_str())
        .collect()
}

/// Every node of the model at every depth, in the model's order (a container before what it holds): the path to
/// it (its own id last) and the node.
pub fn walk(m: &Model) -> Vec<(Vec<String>, &Node)> {
    fn into<'a>(nodes: &'a [Node], path: &mut Vec<String>, out: &mut Vec<(Vec<String>, &'a Node)>) {
        for n in nodes {
            path.push(n.id.clone());
            out.push((path.clone(), n));
            if let Some(b) = &n.body {
                into(&b.nodes, path, out);
            }
            path.pop();
        }
    }
    let mut out = Vec::new();
    into(&m.nodes, &mut Vec::new(), &mut out);
    out
}

/// The nodes a search finds, the best first: every word of it in the id, the operator, the note or a file; the id
/// starting with it, then having it, then the operator, then a file, then the note. Nothing typed: the first nodes.
pub fn hits(m: &Model, query: &str) -> Vec<Hit> {
    let query = query.trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut found: Vec<(u8, Hit)> = Vec::new();
    for (at, n) in walk(m) {
        let (id, op) = (n.id.to_lowercase(), n.op.to_lowercase());
        let note = n.note.as_deref().unwrap_or("").to_lowercase();
        let files: Vec<(&str, String)> = files(n).into_iter().map(|f| (f, f.to_lowercase())).collect();
        let has =
            |w: &&str| id.contains(w) || op.contains(w) || note.contains(w) || files.iter().any(|(_, f)| f.contains(w));
        if !words.iter().all(has) {
            continue;
        }
        let rank = if id.starts_with(&query) {
            0
        } else if id.contains(&query) {
            1
        } else if words.iter().all(|w| id.contains(w) || op.contains(w)) {
            2
        } else if words.iter().all(|w| id.contains(w) || op.contains(w) || files.iter().any(|(_, f)| f.contains(w))) {
            3
        } else {
            4
        };
        let file = files
            .iter()
            .find(|(_, f)| words.iter().any(|w| f.contains(w)))
            .map(|(f, _)| std::path::Path::new(f).file_name().map_or(f.to_string(), |n| n.to_string_lossy().into()));
        found.push((rank, Hit { at, op: n.op.clone(), file }));
    }
    // (the model's order within a rank)
    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, h)| h).take(MOST).collect()
}

/// Go to a node: `at` is the containers' ids from the top, then the node's. The scope becomes the node's (the
/// group's zoom when it is one level in, set directly for more), the node selected and the camera centred on it.
pub fn go(s: &mut Session, r: &Registry, at: &[String]) {
    let Some((id, path)) = at.split_last() else { return };
    if s.source.scope(path).is_none_or(|v| !v.iter().any(|n| n.id == *id)) {
        return;
    }
    s.editor.text = false;
    s.editor.path = path.to_vec();
    crate::builder::select(s, r, id);
    s.editor.chosen = [id.clone()].into();
    s.editor.centre_on(id);
}

/// The search box over the canvas's top, while it is open; its keys taken before the panes'.
pub fn show(ctx: &egui::Context, s: &mut Session, r: &Registry) {
    let Some(mut f) = s.editor.find.take() else { return };
    let found = hits(&s.source, &f.query);
    let (down, up, enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if down {
        f.at = (f.at + 1).min(found.len().saturating_sub(1));
    }
    if up {
        f.at = f.at.saturating_sub(1);
    }
    let mut chosen = (enter && !found.is_empty()).then(|| found[f.at.min(found.len() - 1)].at.clone());
    let mut close = escape || chosen.is_some();
    let screen = ctx.content_rect();
    let canvas = s.editor.canvas().unwrap_or(screen);
    let width = 480.0_f32.min(canvas.width() - 24.0);
    let place = egui::pos2(canvas.center().x - width / 2.0, canvas.top() + 10.0);
    let area =
        egui::Area::new(egui::Id::new("find node")).order(egui::Order::Foreground).fixed_pos(place).show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
                ui.set_width(width);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut f.query)
                        .hint_text("find a node: its id, operator, note or file")
                        .desired_width(f32::INFINITY),
                );
                if !f.focused {
                    edit.request_focus();
                    f.focused = true;
                }
                if edit.changed() {
                    f.at = 0;
                }
                ui.add_space(4.0);
                if found.is_empty() {
                    ui.label(RichText::new("no node has it").weak());
                }
                let at = f.at.min(found.len().saturating_sub(1));
                for (k, h) in found.iter().enumerate() {
                    let weak = ui.visuals().weak_text_color();
                    let mut job = egui::text::LayoutJob::default();
                    let font = egui::TextStyle::Monospace.resolve(ui.style());
                    let text = ui.visuals().text_color();
                    job.append(&format!("{} · {}", h.id(), h.op), 0.0, egui::TextFormat::simple(font, text));
                    let small = egui::FontId::proportional(11.0);
                    for part in h.place().into_iter().chain(h.file.clone()) {
                        job.append(&format!("   {part}"), 0.0, egui::TextFormat::simple(small.clone(), weak));
                    }
                    let resp =
                        ui.add(egui::Button::selectable(k == at, job).min_size(Vec2::new(ui.available_width(), 22.0)));
                    if k == at && (up || down) {
                        resp.scroll_to_me(None);
                    }
                    if resp.clicked() {
                        chosen = Some(h.at.clone());
                        close = true;
                    }
                }
                ui.add_space(2.0);
                ui.label(
                    RichText::new("up, down: choose · enter: go to it · esc: close")
                        .size(10.5)
                        .color(ui.visuals().weak_text_color()),
                );
            });
        });
    // a click elsewhere closes it
    if ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer() {
        close = true;
    }
    if let Some(at) = chosen {
        go(s, r, &at);
    }
    if close {
        ctx.memory_mut(|m| {
            if let Some(id) = m.focused() {
                m.surrender_focus(id);
            }
        });
    } else {
        s.editor.find = Some(f);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Body;

    fn model() -> Model {
        let mut m = Model::new("m");
        let read = Node::new("dem", "raster.read").param("path", "data/Sibuyan_DEM.tif");
        let inner = Node::new("slope", "raster.calc").param("expression", "a");
        let mut group =
            Node::new("prep", terraflux_graph::GROUP).body(Body { nodes: vec![inner], ..Default::default() });
        group.note = Some("terrain steps".into());
        m.nodes.extend([read, group]);
        m
    }

    #[test]
    fn walks_into_groups_and_searches_every_field() {
        let m = model();
        assert_eq!(walk(&m).len(), 3);
        let h = hits(&m, "SLO");
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].at, ["prep", "slope"]);
        assert_eq!(h[0].place().as_deref(), Some("prep > slope"));
        // a file's name, whatever its case; the file shown
        let h = hits(&m, "sibuyan");
        assert_eq!((h.len(), h[0].id()), (1, "dem"));
        assert_eq!(h[0].file.as_deref(), Some("Sibuyan_DEM.tif"));
        // the operator, the note; the id first
        assert_eq!(hits(&m, "raster.").len(), 2);
        assert_eq!(hits(&m, "terrain")[0].id(), "prep");
        assert_eq!(hits(&m, "p")[0].id(), "prep");
        assert!(hits(&m, "nothing like it").is_empty());
        assert_eq!(hits(&m, "").len(), 3);
    }
}
