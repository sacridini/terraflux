//! The node editor's canvas: the nodes of a scope with their ports (inputs on the left, outputs on the right),
//! what the scope may read on the left (the model's inputs, a container's values), the connections. Drag a
//! node to move it, drag from an output to an input to connect them, click a node to edit it in the side
//! panel, double-click a container to edit its body.

use std::collections::BTreeMap;

use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Stroke, Ui, Vec2};
use terraflux_graph::{Model, Node, Ref, Source, Type};
use terraflux_ops::Registry;

use crate::editor;

pub const WIDTH: f32 = 190.0;
const HEAD: f32 = 38.0;
const ROW: f32 = 18.0;
const PORT: f32 = 5.0;

/// What the canvas reports of a frame.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    Select(String),
    Enter(String),
    Move(String, [f32; 2]),
    Connect { reference: String, node: String, port: String },
}

/// The editor's state between frames.
#[derive(Debug, Default)]
pub struct EditorState {
    /// The scope edited: the containers' ids from the top.
    pub path: Vec<String>,
    /// A connection being drawn: the reference and where it starts.
    dragging: Option<(String, Pos2)>,
    /// A node being moved: its id and the offset of the pointer.
    moving: Option<(String, Vec2)>,
    /// The operator chosen to add.
    pub add: String,
    pub message: Option<String>,
}

/// A box of the canvas.
struct BoxAt {
    key: String,
    rect: Rect,
    inputs: Vec<(String, Type, bool)>,
    outputs: Vec<(String, Type)>,
}

impl BoxAt {
    fn input_at(&self, i: usize) -> Pos2 {
        Pos2::new(self.rect.left(), self.rect.top() + HEAD + (i as f32 + 0.5) * ROW)
    }

    fn output_at(&self, i: usize) -> Pos2 {
        Pos2::new(self.rect.right(), self.rect.top() + HEAD + (i as f32 + 0.5) * ROW)
    }
}

/// Where a scope's nodes go when they have no place yet: a column per depth.
pub fn auto_positions(nodes: &[Node]) -> BTreeMap<String, [f32; 2]> {
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    let mut depth: BTreeMap<&str, usize> = BTreeMap::new();
    for _ in 0..=nodes.len() {
        for n in nodes {
            let d = 1 + terraflux_graph::references(n)
                .iter()
                .filter_map(|r| match Ref::parse(r) {
                    Ok(Ref::Port { node, .. }) if ids.contains(&node.as_str()) && node != n.id => {
                        depth.get(node.as_str()).copied()
                    }
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            depth.insert(&n.id, d);
        }
    }
    let mut column: BTreeMap<usize, f32> = BTreeMap::new();
    let mut out = BTreeMap::new();
    for n in nodes {
        let d = depth[n.id.as_str()];
        let y = column.entry(d).or_insert(0.0);
        out.insert(n.id.clone(), [d as f32 * (WIDTH + 60.0), *y]);
        *y += HEAD + 6.0 * ROW + 20.0;
    }
    out
}

/// Draw the scope; what the user did.
pub fn show(ui: &mut Ui, model: &Model, r: &Registry, st: &mut EditorState, selected: Option<&str>) -> Vec<Action> {
    let mut actions = Vec::new();
    let Some(nodes) = model.scope(&st.path) else {
        st.path.clear();
        return actions;
    };
    let mut sources = editor::sources(model, r, &st.path);
    // in a body, what its nodes read from outside it
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    for n in nodes {
        for reference in terraflux_graph::references(n) {
            if let Ok(Ref::Port { node, .. }) = Ref::parse(&reference)
                && !ids.contains(&node.as_str())
                && !sources.iter().any(|(s, _)| *s == reference)
            {
                sources.push((reference, Type::Any));
            }
        }
    }
    let auto = auto_positions(nodes);
    let left = WIDTH + 80.0;
    let mut boxes: Vec<BoxAt> = Vec::new();
    // what the scope reads, in a column on the left
    let mut y = 0.0;
    for (s, t) in &sources {
        let rect = Rect::from_min_size(Pos2::new(0.0, y), Vec2::new(WIDTH, HEAD + ROW));
        boxes.push(BoxAt { key: s.clone(), rect, inputs: Vec::new(), outputs: vec![(String::new(), *t)] });
        y += HEAD + ROW + 12.0;
    }
    for n in nodes {
        let p = n.pos.unwrap_or(auto[&n.id]);
        let inputs = editor::input_ports(n, r);
        let outputs = editor::output_ports(n, r);
        let rows = inputs.len().max(outputs.len()).max(1);
        let rect = Rect::from_min_size(Pos2::new(left + p[0], p[1]), Vec2::new(WIDTH, HEAD + rows as f32 * ROW + 6.0));
        boxes.push(BoxAt { key: n.id.clone(), rect, inputs, outputs });
    }
    let size = boxes.iter().fold(Vec2::new(600.0, 400.0), |s, b| s.max(b.rect.max.to_vec2() + Vec2::splat(40.0)));
    egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
        let (area, _) = ui.allocate_exact_size(size, Sense::hover());
        let origin = area.min.to_vec2() + Vec2::splat(10.0);
        let painter = ui.painter_at(area);
        let v = ui.visuals().clone();
        let place = |r: Rect| r.translate(origin);
        // the place of a reference's output port
        let from_port = |reference: &str| -> Option<Pos2> {
            if reference.starts_with('$')
                || reference.starts_with('@')
                || boxes.iter().any(|b| b.key == reference && b.inputs.is_empty())
            {
                return boxes
                    .iter()
                    .find(|b| b.key == reference)
                    .map(|b| place(b.rect).right_center() + Vec2::new(0.0, HEAD / 2.0 - ROW / 2.0));
            }
            let Ok(Ref::Port { node, port }) = Ref::parse(reference) else { return None };
            let b = boxes.iter().find(|b| b.key == node)?;
            let i = b.outputs.iter().position(|(p, _)| *p == port)?;
            Some(b.output_at(i) + origin)
        };
        // connections
        for n in nodes {
            let Some(b) = boxes.iter().find(|b| b.key == n.id) else { continue };
            for (port, src) in &n.inputs {
                let Source::Ref(reference) = src else { continue };
                let Some(i) = b.inputs.iter().position(|(p, _, _)| p == port) else { continue };
                if let Some(p0) = from_port(reference) {
                    curve(&painter, p0, b.input_at(i) + origin, Stroke::new(1.4, v.weak_text_color()));
                }
            }
        }
        let pointer = ui.input(|i| i.pointer.interact_pos());
        // boxes
        let mut hovered_input: Option<(String, String)> = None;
        for b in &boxes {
            let rect = place(b.rect);
            let source = !nodes.iter().any(|n| n.id == b.key);
            let fill = if source { v.faint_bg_color } else { v.widgets.inactive.bg_fill };
            painter.rect_filled(rect, 6.0, fill);
            let stroke = if selected == Some(b.key.as_str()) {
                Stroke::new(2.0, v.selection.stroke.color)
            } else {
                Stroke::new(1.0, v.widgets.noninteractive.bg_stroke.color)
            };
            painter.rect_stroke(rect, 6.0, stroke, egui::StrokeKind::Inside);
            painter.text(
                rect.left_top() + Vec2::new(8.0, 5.0),
                Align2::LEFT_TOP,
                &b.key,
                FontId::proportional(14.0),
                v.strong_text_color(),
            );
            if let Some(n) = nodes.iter().find(|n| n.id == b.key) {
                painter.text(
                    rect.left_top() + Vec2::new(8.0, 21.0),
                    Align2::LEFT_TOP,
                    &n.op,
                    FontId::monospace(10.5),
                    v.weak_text_color(),
                );
            }
            for (i, (name, _, required)) in b.inputs.iter().enumerate() {
                let p = b.input_at(i) + origin;
                let connected = nodes.iter().find(|n| n.id == b.key).is_some_and(|n| n.inputs.contains_key(name));
                let color = if connected {
                    v.text_color()
                } else if *required {
                    v.warn_fg_color
                } else {
                    v.weak_text_color()
                };
                painter.circle_filled(p, PORT, color);
                painter.text(
                    p + Vec2::new(9.0, 0.0),
                    Align2::LEFT_CENTER,
                    name,
                    FontId::monospace(11.0),
                    v.text_color(),
                );
                if pointer.is_some_and(|q| q.distance(p) < 9.0) {
                    hovered_input = Some((b.key.clone(), name.clone()));
                }
            }
            for (i, (name, _)) in b.outputs.iter().enumerate() {
                let p = if source {
                    rect.right_center() + Vec2::new(0.0, HEAD / 2.0 - ROW / 2.0)
                } else {
                    b.output_at(i) + origin
                };
                painter.circle_filled(p, PORT, Color32::from_rgb(90, 160, 220));
                if !name.is_empty() {
                    painter.text(
                        p - Vec2::new(9.0, 0.0),
                        Align2::RIGHT_CENTER,
                        name,
                        FontId::monospace(11.0),
                        v.text_color(),
                    );
                }
                let resp = ui.interact(
                    Rect::from_center_size(p, Vec2::splat(14.0)),
                    ui.id().with(("out", &b.key, name)),
                    Sense::drag(),
                );
                if resp.drag_started() {
                    let reference = if source { b.key.clone() } else { format!("{}.{name}", b.key) };
                    st.dragging = Some((reference, p));
                }
            }
            if !source {
                // the header moves the node, a click selects it, a double click enters a container
                let head = Rect::from_min_size(rect.min, Vec2::new(rect.width(), HEAD));
                let resp = ui.interact(head, ui.id().with(("node", &b.key)), Sense::click_and_drag());
                if resp.drag_started()
                    && let Some(q) = pointer
                {
                    st.moving = Some((b.key.clone(), q - rect.min));
                }
                if resp.clicked() {
                    actions.push(Action::Select(b.key.clone()));
                }
                if resp.double_clicked() && nodes.iter().any(|n| n.id == b.key && n.body.is_some()) {
                    actions.push(Action::Enter(b.key.clone()));
                }
                if let Some(n) = nodes.iter().find(|n| n.id == b.key)
                    && let Some(note) = &n.note
                {
                    resp.on_hover_text(note);
                }
            }
        }
        // a connection being drawn
        if let Some((reference, p0)) = &st.dragging {
            if let Some(q) = pointer {
                curve(&painter, *p0, q, Stroke::new(2.0, v.selection.stroke.color));
            }
            if ui.input(|i| i.pointer.any_released()) {
                if let Some((node, port)) = hovered_input {
                    actions.push(Action::Connect { reference: reference.clone(), node, port });
                }
                st.dragging = None;
            }
        }
        if let Some((id, off)) = &st.moving {
            if let Some(q) = pointer {
                let p = q - *off - origin - Vec2::new(left, 0.0);
                actions.push(Action::Move(id.clone(), [p.x.max(0.0), p.y.max(0.0)]));
            }
            if ui.input(|i| i.pointer.any_released()) {
                st.moving = None;
            }
        }
    });
    actions
}

fn curve(painter: &egui::Painter, p0: Pos2, p3: Pos2, stroke: Stroke) {
    let dx = ((p3.x - p0.x).abs() * 0.5).max(30.0);
    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
        [p0, p0 + Vec2::new(dx, 0.0), p3 - Vec2::new(dx, 0.0), p3],
        false,
        Color32::TRANSPARENT,
        stroke,
    ));
}
