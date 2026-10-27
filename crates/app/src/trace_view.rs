//! The Trace pane: why a cell of a result has its value ([`crate::trace`]). A cell clicked on a map (the viewer, a
//! map's tab), then Trace this cell: the chain of values that made it, back through the graph, a card per node (its
//! value there, how it was made: map algebra's expression with the values in place of its names, the cells around,
//! what a model weighs, a loop step by step), its inputs a click away; a sentence of it at the top. Trace inside the
//! loops runs the model again keeping their bodies' values at the cell. Ask the assistant why sends the trace to the
//! assistant, which explains it (and may trace more: its tool trace_cell).

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use egui::{Margin, RichText, Ui};
use terraflux_ops::Registry;

use crate::app::Session;
use crate::theme;
use crate::trace::{At, How, Kept, Trace};

/// Who explains a trace asked for: nobody (the Trace pane), the application's assistant, or an assistant through
/// `terraflux mcp` (the trace kept for it, a request to paste copied).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Explain {
    Here,
    Assistant,
    Mcp,
}

/// A trace asked for: the node and port of a map, its grid, the cell (row, column), and who explains it.
pub type Ask = (String, String, Arc<terraflux_raster::Grid>, (usize, usize), Explain);

/// The pane's state (the application's).
#[derive(Default)]
pub struct State {
    asked: Option<Ask>,
    pub trace: Option<Trace>,
    /// The model's revision and the nodes' states it was traced at (traced again when they change).
    made: Option<(u64, u64)>,
    /// What a run kept inside the loops, at the place (x, y) it was asked for.
    pub kept: Option<(f64, f64, Kept)>,
    running: Option<Receiver<Result<Kept, String>>>,
    pub error: Option<String>,
    /// The card to bring into view (an input's node clicked).
    focus: Option<usize>,
    /// Who explains the trace once it is made.
    explain: Option<Explain>,
}

impl State {
    /// Trace a cell of a node's map.
    pub fn ask(&mut self, a: Ask) {
        self.explain = Some(a.4);
        self.asked = Some(a);
        self.made = None;
        self.error = None;
        self.focus = None;
    }

    pub fn running(&self) -> bool {
        self.running.is_some()
    }

    /// The trace made again when the model or its run changed (not while it runs).
    fn refresh(&mut self, s: &Session, r: &Registry) {
        let Some((node, port, grid, cell, _)) = self.asked.clone() else { return };
        let now = (s.revision, s.status_rev);
        if self.made == Some(now) || s.running() {
            return;
        }
        self.made = Some(now);
        let values = |n: &str, p: &str| s.value_of(n, p);
        let (x, y) = crate::trace::centre(&grid, cell.0, cell.1);
        let kept = match &self.kept {
            Some((kx, ky, k)) if *kx == x && *ky == y => k.clone(),
            _ => Kept::new(),
        };
        let t = crate::trace::trace(&s.source, r, &values, &kept, &node, &port, &grid, cell);
        // (the last trace, for an assistant through terraflux mcp: last_trace)
        crate::trace::save_last(s.path.as_deref(), &t);
        self.trace = Some(t);
    }
}

/// What the pane asks of the application.
pub enum Asked {
    /// The trace to the assistant, with the question.
    Assistant(String),
    /// The viewer in front (a step's map shown, the cell outlined).
    Viewer,
}

fn weak(t: impl Into<String>) -> RichText {
    RichText::new(t.into()).color(theme::weak())
}

/// A value at the cell, strong.
fn value(v: &Option<At>) -> RichText {
    match v {
        Some(a) => RichText::new(a.text()).strong(),
        None => weak("no value kept"),
    }
}

/// The Trace pane.
pub fn pane(ui: &mut Ui, st: &mut State, s: &mut Session, r: &Registry) -> Option<Asked> {
    let got = st.running.as_ref().map(|rx| match rx.try_recv() {
        Ok(k) => Some(k),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("the run stopped".into())),
    });
    if let Some(Some(k)) = got {
        st.running = None;
        match k {
            Ok(k) => {
                if let Some(t) = &st.trace {
                    st.kept = Some((t.x, t.y, k));
                }
                st.made = None;
            }
            Err(e) => st.error = Some(e),
        }
    }
    st.refresh(s, r);
    let mut asked = None;
    // asked for with the assistant (a right click on a cell): to it, once traced
    if let Some(e) = st.explain
        && let Some(t) = &st.trace
    {
        st.explain = None;
        match e {
            Explain::Assistant => asked = Some(Asked::Assistant(question(t))),
            // (kept already for last_trace: a request to paste in the assistant)
            Explain::Mcp => {
                ui.ctx().copy_text(MCP_REQUEST.to_string());
                s.notice = Some((
                    "traced, and a request copied: paste it in your assistant (or run its prompt explain_cell)".into(),
                    std::time::Instant::now(),
                ));
            }
            Explain::Here => {}
        }
    }
    let mut select: Option<(Vec<String>, String, String)> = None;
    egui::ScrollArea::vertical().id_salt("trace pane").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
            theme::section(ui, "Trace of a cell");
            let Some(t) = st.trace.clone() else {
                ui.label(weak(
                    "Why a cell has its value: click a cell of a map (in the viewer or a map's tab), then Trace this \
                     cell. The chain of values that made it shows here, node by node.",
                ));
                return;
            };
            let Some(first) = t.steps.first() else { return };
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{}.{}", first.node, first.port)).strong());
                ui.label(weak(format!(
                    "at row {}, column {} (x {}, y {})",
                    t.cell.0,
                    t.cell.1,
                    crate::report::cell(t.x),
                    crate::report::cell(t.y)
                )));
            });
            ui.horizontal_wrapped(|ui| {
                if ui
                    .add(crate::app::accent_button("Ask the assistant why", true))
                    .on_hover_text("the trace to the assistant, to explain it in words (and trace more if it needs)")
                    .clicked()
                {
                    asked = Some(Asked::Assistant(question(&t)));
                }
                let loops = s.source.nodes.iter().any(has_loop);
                if loops
                    && ui
                        .add_enabled(!st.running() && !s.running(), egui::Button::new("Trace inside the loops"))
                        .on_hover_text(
                            "run the model again keeping what the nodes of its loops give at this cell, step by step",
                        )
                        .clicked()
                {
                    let (m, base, x, y) = (s.source.clone(), s.base(), t.x, t.y);
                    let (tx, rx) = channel();
                    let ctx = ui.ctx().clone();
                    std::thread::spawn(move || {
                        let _ = tx.send(crate::trace::keep_loops(&m, &base, x, y));
                        ctx.request_repaint();
                    });
                    st.running = Some(rx);
                }
                if st.running() {
                    ui.spinner();
                    ui.label(weak("running again...").small());
                }
                if ui.button("Copy").on_hover_text("the trace as text").clicked() {
                    ui.ctx().copy_text(format!("{}\n\n{}", t.sentence(), t.text()));
                }
            });
            if let Some(e) = &st.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
            ui.add_space(4.0);
            ui.label(RichText::new(t.sentence()).italics());
            ui.add_space(6.0);
            for (k, step) in t.steps.iter().enumerate() {
                let resp = egui::Frame::new()
                    .fill(theme::well())
                    .corner_radius(4.0)
                    .inner_margin(Margin::same(6))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{}.", k + 1)).strong());
                            if ui
                                .link(RichText::new(format!("{}.{}", step.node, step.port)).strong())
                                .on_hover_text("select it: its map in the viewer, this cell outlined")
                                .clicked()
                            {
                                select = Some((step.path.clone(), step.node.clone(), step.port.clone()));
                            }
                            ui.label(weak(&step.op).monospace().small());
                            if !step.path.is_empty() {
                                ui.label(weak(format!("in {}", step.path.join("/"))).small());
                            }
                            ui.label("=");
                            ui.label(value(&step.value));
                        });
                        if let Some(n) = &step.note {
                            ui.label(RichText::new(n).italics());
                        }
                        how_ui(ui, &step.how);
                        for i in &step.inputs {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 4.0;
                                ui.label(weak(&i.port));
                                ui.label(weak("from"));
                                match i.step {
                                    Some(j) => {
                                        if ui.link(RichText::new(&i.from)).on_hover_text("to its card").clicked() {
                                            st.focus = Some(j);
                                        }
                                    }
                                    None => {
                                        ui.label(RichText::new(&i.from));
                                    }
                                }
                                ui.label(weak("="));
                                ui.label(value(&i.value));
                            });
                        }
                        for (what, values) in &step.body {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(weak(format!("in the loop, {what}:")));
                                ui.label(RichText::new(crate::trace::steps_text(values)));
                            });
                        }
                    });
                if st.focus == Some(k) {
                    resp.response.scroll_to_me(Some(egui::Align::TOP));
                    st.focus = None;
                }
                ui.add_space(4.0);
            }
            if t.cut {
                ui.label(weak("(the trace stops here: the graph goes on)").small());
            }
        });
    });
    if let Some((path, id, port)) = select {
        s.editor.path = path.clone();
        crate::builder::select(s, r, &id);
        // its map in the viewer (a top-level node's, a group's body's), the cell traced outlined
        if let Some((node, port)) = crate::builder::shown_at(s, &path, &id, &port)
            && let Some(t) = &st.trace
        {
            let cell = s
                .value_of(&node, &port)
                .and_then(|v| crate::trace::grid_of(&v))
                .and_then(|g| crate::trace::cell_of(&g, t.x, t.y));
            s.preview = Some((node, port));
            s.preview_view.picked = cell;
            asked = asked.or(Some(Asked::Viewer));
        }
        s.editor.reveal(&id);
    }
    asked
}

/// What is copied for an assistant through `terraflux mcp` to explain the cell traced.
pub const MCP_REQUEST: &str = "Explain why the cell I traced in Terraflux has its value: read its trace with the terraflux tool last_trace, then \
go through it step by step, in words.";

/// The question to the assistant of a trace: why the cell has its value, from the trace.
fn question(t: &Trace) -> String {
    let (node, port) = t.steps.first().map_or(("", ""), |s| (s.node.as_str(), s.port.as_str()));
    format!(
        "Why does {node}.{port} have this value at row {}, column {}? Explain it from its trace, step by step, in \
         words (trace_cell traces other cells or nodes):\n\n{}",
        t.cell.0,
        t.cell.1,
        t.text()
    )
}

/// Whether a node is, or holds, a loop.
fn has_loop(n: &terraflux_graph::Node) -> bool {
    n.op == "control.repeat"
        || n.op == terraflux_graph::EACH
        || n.body.as_ref().is_some_and(|b| b.nodes.iter().any(has_loop))
}

/// How a node made its value, in a few lines.
fn how_ui(ui: &mut Ui, how: &How) {
    match how {
        How::Data => {
            ui.label(weak("data: read or drawn, from no other node"));
        }
        How::Expression { text, with_values } => {
            for (label, t) in [("", text), ("here: ", with_values)] {
                egui::Frame::new().fill(theme::bar()).corner_radius(3.0).inner_margin(Margin::same(4)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(format!("{label}{t}")).monospace());
                });
            }
        }
        How::Window { radius, windows } => {
            ui.label(weak(format!("from the cells within {radius} of this one (it at the centre):")));
            for (port, rows) in windows {
                ui.label(RichText::new(port));
                egui::Grid::new(ui.next_auto_id()).spacing([6.0, 1.0]).show(ui, |ui| {
                    let mid = rows.len() / 2;
                    for (i, row) in rows.iter().enumerate() {
                        for (j, v) in row.iter().enumerate() {
                            let t = RichText::new(crate::report::cell(*v)).monospace();
                            ui.label(if i == mid && j == mid { t.strong().color(theme::accent()) } else { t });
                        }
                        ui.end_row();
                    }
                });
            }
        }
        How::Whole(what) => {
            ui.label(weak(format!("from the whole map: {what}")));
        }
        How::Patch { .. } => {
            ui.label(weak(how.patch_text()));
        }
        How::Allocation { what, lines } => {
            ui.label(weak(what));
            for l in lines {
                ui.label(RichText::new(l).small());
            }
        }
        How::Loop => {
            ui.label(weak("a loop: its value at every step"));
        }
        How::Group => {
            ui.label(weak("a group: what its body gave"));
        }
        How::Local => {}
    }
}
