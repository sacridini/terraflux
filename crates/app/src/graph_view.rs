//! The model's graph on the screen: the inputs on the left, each node in the column of its depth (one more
//! than the deepest node it reads), containers with their body inside; the note of a node on hover.

use std::collections::BTreeMap;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use terraflux_graph::{Model, Node, Ref, Source, references};

pub const WIDTH: f32 = 180.0;
const GAP_X: f32 = 46.0;
const GAP_Y: f32 = 22.0;
const LINE: f32 = 17.0;

/// A box of the diagram.
#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// `$input` or a node's id.
    pub key: String,
    pub title: String,
    pub subtitle: String,
    pub note: Option<String>,
    /// the nodes of a container: (id, operator)
    pub body: Vec<(String, String)>,
    pub rect: Rect,
}

/// Boxes and edges (from key, to key) of a model's top level.
pub fn layout(model: &Model) -> (Vec<Placed>, Vec<(String, String)>) {
    let ids: Vec<&str> = model.nodes.iter().map(|n| n.id.as_str()).collect();
    let sources = |n: &Node| -> Vec<String> {
        let mut out = Vec::new();
        for r in references(n) {
            match Ref::parse(&r) {
                Ok(Ref::Input(name)) => out.push(format!("${name}")),
                Ok(Ref::Port { node, .. }) if ids.contains(&node.as_str()) && node != n.id => out.push(node),
                _ => {}
            }
        }
        out.sort();
        out.dedup();
        out
    };
    // depth: inputs 0, a node 1 + its deepest source
    let mut depth: BTreeMap<String, usize> = model.inputs.keys().map(|k| (format!("${k}"), 0)).collect();
    for _ in 0..=model.nodes.len() {
        for n in &model.nodes {
            let d = 1 + sources(n).iter().map(|s| depth.get(s).copied().unwrap_or(0)).max().unwrap_or(0);
            depth.insert(n.id.clone(), d);
        }
    }
    let mut columns: BTreeMap<usize, f32> = BTreeMap::new();
    let mut boxes = Vec::new();
    let mut place =
        |key: String, title: String, subtitle: String, note: Option<String>, body: Vec<(String, String)>| {
            let d = depth[&key];
            let lines = 2 + body.len() + usize::from(!body.is_empty());
            let h = lines as f32 * LINE + 12.0;
            let y = columns.entry(d).or_insert(0.0);
            let rect = Rect::from_min_size(Pos2::new(d as f32 * (WIDTH + GAP_X), *y), Vec2::new(WIDTH, h));
            *y += h + GAP_Y;
            boxes.push(Placed { key, title, subtitle, note, body, rect });
        };
    let file = |p: &str| p.rsplit(['/', '\\']).next().unwrap_or(p).to_string();
    for (name, input) in &model.inputs {
        let (kind, note) = match input {
            terraflux_graph::Input::Map { path, band, note, .. } => (
                format!("{}{}", file(path), band.as_ref().map(|b| format!(" [{b}]")).unwrap_or_default()),
                Some(format!("{}\n{path}", note.clone().unwrap_or_default())),
            ),
            terraflux_graph::Input::Stack { path, note, .. } => {
                (file(path), Some(format!("{}\n{path}", note.clone().unwrap_or_default())))
            }
            terraflux_graph::Input::Value { value, note } => (format!("= {value}"), note.clone()),
        };
        place(format!("${name}"), format!("${name}"), kind, note, Vec::new());
    }
    let mut edges = Vec::new();
    for n in &model.nodes {
        let body: Vec<(String, String)> =
            n.body.as_ref().map(|b| b.nodes.iter().map(|x| (x.id.clone(), x.op.clone())).collect()).unwrap_or_default();
        place(n.id.clone(), n.id.clone(), n.op.clone(), n.note.clone(), body);
        for s in sources(n) {
            edges.push((s, n.id.clone()));
        }
    }
    (boxes, edges)
}

/// The literal settings of a node, one per line.
pub fn settings(n: &Node) -> String {
    let mut lines: Vec<String> = n
        .inputs
        .iter()
        .map(|(k, s)| match s {
            Source::Literal(v) => format!("{k} = {v}"),
            Source::Ref(r) => format!("{k} <- {r}"),
        })
        .collect();
    lines.extend(n.params.iter().map(|(k, v)| {
        let t = v.to_string();
        format!("{k} = {}", if t.len() > 80 { format!("{}...", &t[..80]) } else { t })
    }));
    lines.join("\n")
}

/// Draw the diagram in a scrolling area; returns the key of the box clicked.
pub fn show(ui: &mut Ui, model: &Model) -> Option<String> {
    let (boxes, edges) = layout(model);
    let size = boxes.iter().fold(Vec2::ZERO, |s, b| s.max(b.rect.max.to_vec2())) + Vec2::splat(20.0);
    let mut clicked = None;
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        let (area, _) = ui.allocate_exact_size(size, Sense::hover());
        let origin = area.min.to_vec2() + Vec2::splat(10.0);
        let painter = ui.painter_at(area);
        let visuals = ui.visuals().clone();
        let find = |k: &str| boxes.iter().find(|b| b.key == k).map(|b| b.rect.translate(origin));
        for (from, to) in &edges {
            if let (Some(a), Some(b)) = (find(from), find(to)) {
                let (p0, p3) = (a.right_center(), b.left_center());
                let dx = (p3.x - p0.x).abs() * 0.5;
                let curve = egui::epaint::CubicBezierShape::from_points_stroke(
                    [p0, p0 + Vec2::new(dx, 0.0), p3 - Vec2::new(dx, 0.0), p3],
                    false,
                    Color32::TRANSPARENT,
                    Stroke::new(1.2, visuals.weak_text_color()),
                );
                painter.add(curve);
            }
        }
        for b in &boxes {
            let r = b.rect.translate(origin);
            let input = b.key.starts_with('$');
            let fill = if input { visuals.faint_bg_color } else { visuals.widgets.inactive.bg_fill };
            painter.rect_filled(r, 6.0, fill);
            painter.rect_stroke(
                r,
                6.0,
                Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
                egui::StrokeKind::Inside,
            );
            let mut y = r.top() + 6.0;
            painter.text(
                Pos2::new(r.left() + 8.0, y),
                Align2::LEFT_TOP,
                &b.title,
                FontId::proportional(14.0),
                visuals.strong_text_color(),
            );
            y += LINE;
            let sub: String = b.subtitle.chars().take(25).collect();
            painter.text(
                Pos2::new(r.left() + 8.0, y),
                Align2::LEFT_TOP,
                sub,
                FontId::monospace(11.0),
                visuals.text_color(),
            );
            if !b.body.is_empty() {
                y += LINE;
                painter.line_segment(
                    [Pos2::new(r.left() + 6.0, y + 4.0), Pos2::new(r.right() - 6.0, y + 4.0)],
                    Stroke::new(1.0, visuals.weak_text_color()),
                );
                for (id, op) in &b.body {
                    y += LINE;
                    painter.text(
                        Pos2::new(r.left() + 14.0, y - 6.0),
                        Align2::LEFT_TOP,
                        format!("{id}: {op}"),
                        FontId::monospace(11.0),
                        visuals.text_color(),
                    );
                }
            }
            let resp = ui.interact(r, ui.id().with(&b.key), Sense::click());
            if resp.clicked() {
                clicked = Some(b.key.clone());
            }
            if let Some(note) = &b.note {
                resp.on_hover_text(note);
            }
        }
    });
    clicked
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::{Body, Input};

    #[test]
    fn columns_follow_dependencies() {
        let mut m = Model::new("t");
        m.inputs.insert("t0".into(), Input::Map { path: "a.tif".into(), band: None, nodata: None, note: None });
        m.nodes.push(Node::new("b", "x").input("m", "a.out"));
        m.nodes.push(Node::new("a", "x").input("t", "$t0"));
        m.nodes.push(
            Node::new("loop", "control.repeat")
                .input("land", "$t0")
                .body(Body { nodes: vec![Node::new("s", "x").input("p", "b.out")], ..Default::default() }),
        );
        let (boxes, edges) = layout(&m);
        let x = |k: &str| boxes.iter().find(|b| b.key == k).unwrap().rect.left();
        assert!(x("$t0") < x("a") && x("a") < x("b") && x("b") < x("loop"));
        assert!(edges.contains(&("b".to_string(), "loop".to_string())));
        assert_eq!(boxes.iter().find(|b| b.key == "loop").unwrap().body, vec![("s".to_string(), "x".to_string())]);
    }
}
