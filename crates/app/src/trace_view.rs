//! The Trace pane: why a cell of a result has its value ([`crate::trace`]). A cell clicked on a map (in the viewer),
//! then Trace this cell: the chain of values that made it, back through the graph, a card per node (its
//! value there, how it was made: map algebra's expression with the values in place of its names, the cells around,
//! what a model weighs, a loop step by step), its inputs a click away; a sentence of it at the top. Trace inside the
//! loops runs the model again keeping their bodies' values at the cell. Ask the assistant why sends the trace to the
//! assistant, which explains it (and may trace more: its tool trace_cell). Compare with another cell, then a click
//! in the viewer (or a right click's Compare with the traced cell), compares two cells ([`crate::trace::compare`]):
//! where the difference begins and why, then the chain with the nodes that differ, the two cells marked A and B.

use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};

use egui::{Margin, RichText, Ui};
use terraflux_ops::Registry;

use crate::app::Session;
use crate::theme;
use crate::trace::{At, Comparison, How, Kept, Kind, Trace, Why};

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

/// A node chosen in the pane: its place, id and port, and its step in the trace (of A, when compared): its map
/// shown, its patch drawn.
type Select = (Vec<String>, String, String, Option<usize>);

/// What a run keeping the loops' values at the places gives, when it ends.
type Keeping = Receiver<Result<Vec<Kept>, String>>;

/// The model run with another scenario, the other side (B) of a cell compared with itself.
pub struct Other {
    /// The scenario's name.
    pub name: String,
    /// The model with it, its run's values, and what its loops kept at the cell.
    pub model: terraflux_graph::Model,
    pub results: terraflux_exec::Results,
    pub kept: Kept,
    /// What the loops of the model as it is kept at the cell (A traced inside them as B is), when it has loops.
    kept_a: Option<Kept>,
    /// The model's revision it was run from (let go when the model changes).
    revision: u64,
}

/// The pane's state (the application's).
#[derive(Default)]
pub struct State {
    asked: Option<Ask>,
    pub trace: Option<Trace>,
    /// The other cell compared with the one traced (B), its trace, and the two compared.
    asked_b: Option<Ask>,
    pub trace_b: Option<Trace>,
    pub comparison: Option<Comparison>,
    /// Every node of the comparison shown, the same ones too (else those collapsed).
    show_all: bool,
    /// The other side when the cell is compared with itself in another scenario, and that run going.
    pub other: Option<Other>,
    running_other: Option<Receiver<Result<Other, String>>>,
    /// The model's revision and the nodes' states it was traced at (traced again when they change).
    made: Option<(u64, u64)>,
    /// What a run kept inside the loops, at each place (x, y) it was asked for (the cell traced, then B).
    pub kept: Vec<(f64, f64, Kept)>,
    /// A run keeping them: the places, and what it gives.
    running: Option<(Vec<(f64, f64)>, Keeping)>,
    pub error: Option<String>,
    /// The card to bring into view (an input's node clicked).
    focus: Option<usize>,
    /// Who explains the trace once it is made.
    explain: Option<Explain>,
}

impl State {
    /// Trace a cell of a node's map (a comparison let go).
    pub fn ask(&mut self, a: Ask) {
        self.explain = Some(a.4);
        self.asked = Some(a);
        self.made = None;
        self.error = None;
        self.focus = None;
        self.stop_comparing();
    }

    /// Compare the cell traced with another (B); with none traced, trace it.
    pub fn compare_with(&mut self, b: Ask) {
        if self.asked.is_none() {
            return self.ask(b);
        }
        self.stop_comparing();
        self.asked_b = Some(b);
        self.made = None;
        self.error = None;
        self.focus = None;
        self.show_all = false;
    }

    /// Back to the cell traced alone.
    pub fn stop_comparing(&mut self) {
        self.asked_b = None;
        self.trace_b = None;
        self.comparison = None;
        self.other = None;
        self.running_other = None;
        self.made = None;
    }

    /// Compare the cell traced with itself in another scenario: the model run with it in a thread (keeping what
    /// its loops give at the cell; the model as it is run again too when it has loops, for A to be traced inside
    /// them alike), then compared ([`crate::trace::compare_named`], the sides by their scenarios).
    pub fn compare_with_scenario(&mut self, s: &Session, name: &str, ctx: egui::Context) {
        let Some(t) = &self.trace else { return };
        let model = match s.source.with_scenario(name) {
            Ok(m) => m,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        let (name, base, place, source, revision) =
            (name.to_string(), s.base(), (t.x, t.y), s.source.clone(), s.revision);
        let loops = source.nodes.iter().any(has_loop);
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            let got = (|| {
                let (results, kept) = crate::trace::run_keeping(&model, &base, &[place])?;
                let kept_a =
                    if loops { Some(crate::trace::keep_loops(&source, &base, place.0, place.1)?) } else { None };
                let kept = kept.into_iter().next().unwrap_or_default();
                Ok(Other { name, model, results, kept, kept_a, revision })
            })();
            let _ = tx.send(got);
            ctx.request_repaint();
        });
        self.stop_comparing();
        self.error = None;
        self.running_other = Some(rx);
    }

    /// Whether the run of another scenario is going.
    pub fn running_other(&self) -> bool {
        self.running_other.is_some()
    }

    /// A and B the other way.
    pub fn swap(&mut self) {
        if self.asked_b.is_some() {
            std::mem::swap(&mut self.asked, &mut self.asked_b);
            self.made = None;
        }
    }

    /// The places of the cells traced: the one, or the two compared.
    /// (Compared with another scenario, the one cell: marked A alone.)
    pub fn places(&self) -> Vec<(f64, f64)> {
        let b = if self.other.is_some() { None } else { self.trace_b.as_ref() };
        self.trace.iter().chain(b).map(|t| (t.x, t.y)).collect()
    }

    pub fn running(&self) -> bool {
        self.running.is_some()
    }

    /// The trace made again when the model or its run changed (not while it runs).
    fn refresh(&mut self, s: &Session, r: &Registry) {
        let Some(a) = self.asked.clone() else { return };
        let now = (s.revision, s.status_rev);
        if self.made == Some(now) || s.running() {
            return;
        }
        self.made = Some(now);
        // (another scenario's run of a model since changed: let go)
        if self.other.as_ref().is_some_and(|o| o.revision != s.revision) {
            self.other = None;
        }
        let values = |n: &str, p: &str| s.value_of(n, p);
        let traced = |(node, port, grid, cell, _): &Ask| {
            let (x, y) = crate::trace::centre(grid, cell.0, cell.1);
            let kept =
                self.kept.iter().find(|(kx, ky, _)| *kx == x && *ky == y).map_or_else(Kept::new, |k| k.2.clone());
            crate::trace::trace(&s.source, r, &values, &kept, node, port, grid, *cell)
        };
        let t = traced(&a);
        // (the last trace, for an assistant through terraflux mcp: last_trace)
        crate::trace::save_last(s.path.as_deref(), &t);
        // the other side: another cell, or the same in another scenario's run
        self.trace_b = match &self.other {
            Some(o) => {
                let (node, port, grid, cell, _) = &a;
                let values = crate::trace::values_of(&o.results);
                Some(crate::trace::trace(&o.model, r, &values, &o.kept, node, port, grid, *cell))
            }
            None => self.asked_b.as_ref().map(traced),
        };
        let here = s.scenario_now().unwrap_or_else(|| "the model as it is".into());
        self.comparison = self.trace_b.as_ref().map(|b| match &self.other {
            Some(o) => crate::trace::compare_named(&t, b, [&here, &o.name]),
            None => crate::trace::compare(&t, b),
        });
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
    let got = st.running.as_ref().map(|(_, rx)| match rx.try_recv() {
        Ok(k) => Some(k),
        Err(std::sync::mpsc::TryRecvError::Empty) => None,
        Err(std::sync::mpsc::TryRecvError::Disconnected) => Some(Err("the run stopped".into())),
    });
    if let Some(Some(k)) = got
        && let Some((places, _)) = st.running.take()
    {
        match k {
            Ok(k) => {
                st.kept = places.into_iter().zip(k).map(|((x, y), k)| (x, y, k)).collect();
                st.made = None;
            }
            Err(e) => st.error = Some(e),
        }
    }
    // the run of another scenario, ended
    let other = st.running_other.as_ref().map(|rx| rx.try_recv());
    match other {
        Some(Ok(Ok(o))) => {
            if let (Some(k), Some(t)) = (&o.kept_a, &st.trace) {
                st.kept = vec![(t.x, t.y, k.clone())];
            }
            st.other = Some(o);
            st.running_other = None;
            st.made = None;
        }
        Some(Ok(Err(e))) => {
            st.error = Some(e);
            st.running_other = None;
        }
        Some(Err(std::sync::mpsc::TryRecvError::Disconnected)) => {
            st.error = Some("the run stopped".into());
            st.running_other = None;
        }
        _ => {}
    }
    st.refresh(s, r);
    s.trace_marks = st.places();
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
    let mut select: Option<Select> = None;
    egui::ScrollArea::vertical().id_salt("trace pane").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
            let Some(t) = st.trace.clone() else {
                theme::section(ui, "Trace of a cell");
                ui.label(weak(
                    "Why a cell has its value: click a cell of a map in the viewer, then Trace this cell. The \
                     chain of values that made it shows here, node by node.",
                ));
                return;
            };
            if let Some(c) = st.comparison.clone() {
                let title = if st.other.is_some() { "One cell in two scenarios" } else { "Two cells compared" };
                theme::section(ui, title);
                compared(ui, st, s, &c, &mut select, &mut asked);
                return;
            }
            theme::section(ui, "Trace of a cell");
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
                inside_loops(ui, st, s);
                if ui.button("Copy").on_hover_text("the trace as text").clicked() {
                    ui.ctx().copy_text(format!("{}\n\n{}", t.sentence(), t.text()));
                }
            });
            ui.horizontal_wrapped(|ui| {
                let choosing = s.trace_choosing.is_some();
                if ui
                    .add_enabled(!choosing, egui::Button::new("Compare with another cell"))
                    .on_hover_text(
                        "then click another cell of the map in the viewer: why the two differ, and where the \
                         difference begins",
                    )
                    .clicked()
                {
                    s.trace_choosing = Some(s.preview_view.clicks);
                    asked.get_or_insert(Asked::Viewer);
                }
                if choosing {
                    ui.label(weak("click another cell of the map in the viewer").small());
                    if ui.small_button("Cancel").clicked() {
                        s.trace_choosing = None;
                    }
                }
                // the same cell in another scenario's run
                let now = s.scenario_now();
                let others: Vec<String> =
                    s.source.scenarios.iter().map(|x| x.name.clone()).filter(|n| Some(n) != now.as_ref()).collect();
                let mut chosen: Option<String> = None;
                if !others.is_empty() {
                    ui.add_enabled_ui(!st.running_other() && !s.running(), |ui| {
                        ui.menu_button("Compare with a scenario", |ui| {
                            for n in &others {
                                if ui
                                    .button(n)
                                    .on_hover_text(
                                        "the model run with this scenario: this cell in its run, compared with this \
                                         run's",
                                    )
                                    .clicked()
                                {
                                    chosen = Some(n.clone());
                                    ui.close();
                                }
                            }
                        });
                    });
                }
                if st.running_other() {
                    ui.spinner();
                    ui.label(weak("running the scenario...").small());
                    if ui.small_button("Cancel").clicked() {
                        st.running_other = None;
                    }
                }
                if let Some(n) = chosen {
                    st.compare_with_scenario(s, &n, ui.ctx().clone());
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
                    .corner_radius(crate::theme::rounded(4.0))
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
                                select = Some((step.path.clone(), step.node.clone(), step.port.clone(), Some(k)));
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
    if let Some((path, id, port, k)) = select {
        s.editor.path = path.clone();
        crate::builder::select(s, r, &id);
        // its map in the viewer (a top-level node's, a group's body's), the cell traced outlined (and the patch
        // it is in, a patch's operator's)
        if let Some((node, port)) = crate::builder::shown_at(s, &path, &id, &port)
            && let Some(t) = &st.trace
        {
            let cell = s
                .value_of(&node, &port)
                .and_then(|v| crate::trace::grid_of(&v))
                .and_then(|g| crate::trace::cell_of(&g, t.x, t.y));
            s.preview = Some((node, port));
            s.preview_view.picked = cell;
            s.preview_view.patch =
                k.and_then(|k| t.steps.get(k)).and_then(|step| crate::trace::patch_outline(&step.how));
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

/// Trace inside the loops (a model with loops): the model run again keeping what the nodes of its loops give at the
/// cells traced (the two compared at once), step by step; a spinner while it runs.
fn inside_loops(ui: &mut Ui, st: &mut State, s: &Session) {
    if !s.source.nodes.iter().any(has_loop) {
        return;
    }
    let places = st.places();
    let tip = if places.len() > 1 {
        "run the model again keeping what the nodes of its loops give at the two cells, step by step: where their \
         series part"
    } else {
        "run the model again keeping what the nodes of its loops give at this cell, step by step"
    };
    if ui
        .add_enabled(!st.running() && !s.running(), egui::Button::new("Trace inside the loops"))
        .on_hover_text(tip)
        .clicked()
    {
        let (m, base, at) = (s.source.clone(), s.base(), places.clone());
        let (tx, rx) = channel();
        let ctx = ui.ctx().clone();
        std::thread::spawn(move || {
            let _ = tx.send(crate::trace::keep_loops_at(&m, &base, &at));
            ctx.request_repaint();
        });
        st.running = Some((places, rx));
    }
    if st.running() {
        ui.spinner();
        ui.label(weak("running again...").small());
    }
}

/// The question to the assistant of a comparison: why the two cells differ, from the comparison.
fn compare_question(c: &Comparison) -> String {
    let (node, port) = c.a.steps.first().map_or(("", ""), |s| (s.node.as_str(), s.port.as_str()));
    format!(
        "Why does {node}.{port} differ between these two cells? Explain it from their comparison, where the \
         difference begins and why, in words (trace_cell traces other cells, or compares them):\n\n{}\n\n{}",
        c.sentence(),
        c.text()
    )
}

/// A side's letter in its colour (A in the accent, B in a colour apart: as the viewer marks the two cells).
fn letter(ui: &mut Ui, k: usize) {
    let (text, colour, ink) =
        if k == 0 { ("A", theme::accent(), theme::on_accent()) } else { ("B", theme::second(), theme::on_second()) };
    egui::Frame::new().fill(colour).corner_radius(theme::rounded(3.0)).inner_margin(Margin::symmetric(4, 0)).show(
        ui,
        |ui| {
            ui.label(RichText::new(text).strong().color(ink));
        },
    );
}

/// The two cells compared: where they are and their values, the sentence, where the difference begins and why,
/// then the chain of nodes (those that differ shown, those the same collapsed).
fn compared(
    ui: &mut Ui,
    st: &mut State,
    s: &Session,
    c: &Comparison,
    select: &mut Option<Select>,
    asked: &mut Option<Asked>,
) {
    // (compared with another scenario: the sides by their scenarios, the same cell)
    let scenario = st.other.is_some();
    for (k, t) in [(0, &c.a), (1, &c.b)] {
        ui.horizontal_wrapped(|ui| {
            letter(ui, k);
            if scenario {
                ui.label(RichText::new(&c.names[k]).strong());
            }
            if let Some(first) = t.steps.first() {
                ui.label(RichText::new(format!("{}.{}", first.node, first.port)).strong());
            }
            ui.label(weak(format!(
                "at row {}, column {} (x {}, y {})",
                t.cell.0,
                t.cell.1,
                crate::report::cell(t.x),
                crate::report::cell(t.y)
            )));
            ui.label("=");
            ui.label(value(&t.steps.first().and_then(|s| s.value.clone())));
        });
    }
    ui.horizontal_wrapped(|ui| {
        if ui
            .add(crate::app::accent_button("Ask the assistant why", true))
            .on_hover_text("the comparison to the assistant, to explain in words why the two cells differ")
            .clicked()
        {
            *asked = Some(Asked::Assistant(compare_question(c)));
        }
        if !scenario {
            if ui.button("Swap A and B").clicked() {
                st.swap();
            }
            inside_loops(ui, st, s);
        }
        if ui.button("Copy").on_hover_text("the comparison as text").clicked() {
            ui.ctx().copy_text(format!("{}\n\n{}", c.sentence(), c.text()));
        }
        if ui.button("Trace A alone").on_hover_text("back to the trace of the cell A").clicked() {
            st.stop_comparing();
        }
    });
    if let Some(e) = &st.error {
        ui.colored_label(ui.visuals().error_fg_color, e);
    }
    ui.add_space(4.0);
    ui.label(RichText::new(c.sentence()).italics());
    ui.add_space(6.0);
    if !c.differs() {
        return;
    }
    if !c.roots.is_empty() {
        ui.label(RichText::new("Where the difference begins").strong());
        for root in &c.roots {
            pair_card(ui, c, root.pair, Some(&root.why), select);
            ui.add_space(4.0);
        }
        ui.add_space(4.0);
    }
    ui.label(RichText::new("The chain").strong());
    let mut k = 0;
    while k < c.pairs.len() {
        if !st.show_all && c.pairs[k].kind == Kind::Same {
            let from = k;
            while k < c.pairs.len() && c.pairs[k].kind == Kind::Same {
                k += 1;
            }
            let n = k - from;
            ui.horizontal(|ui| {
                ui.label(weak(if n == 1 { "1 node the same".to_string() } else { format!("{n} nodes the same") }));
                if ui.small_button("show all").clicked() {
                    st.show_all = true;
                }
            });
            continue;
        }
        pair_card(ui, c, k, None, select);
        ui.add_space(4.0);
        k += 1;
    }
    if st.show_all && ui.small_button("show only those that differ").clicked() {
        st.show_all = false;
    }
}

/// A node of a comparison: its name (a click selects it: its map in the viewer, the two cells marked), how it
/// stands, why when the difference begins there, and its value at A and at B (with the expression, a switch's
/// branch).
fn pair_card(ui: &mut Ui, c: &Comparison, k: usize, why: Option<&Why>, select: &mut Option<Select>) {
    let p = &c.pairs[k];
    let (sa, sb) = c.steps(p);
    egui::Frame::new().fill(theme::well()).corner_radius(theme::rounded(4.0)).inner_margin(Margin::same(6)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_wrapped(|ui| {
                if ui
                    .link(RichText::new(format!("{}.{}", p.node, p.port)).strong())
                    .on_hover_text("select it: its map in the viewer, the two cells marked")
                    .clicked()
                {
                    *select = Some((p.path.clone(), p.node.clone(), p.port.clone(), p.a));
                }
                ui.label(weak(&p.op).monospace().small());
                if !p.path.is_empty() {
                    ui.label(weak(format!("in {}", p.path.join("/"))).small());
                }
                let (said, colour) = match p.kind {
                    Kind::Same => ("the same", theme::weak()),
                    Kind::Different => ("differs", ui.visuals().warn_fg_color),
                    Kind::OnlyA => ("only at A", theme::accent()),
                    Kind::OnlyB => ("only at B", theme::second()),
                };
                ui.label(RichText::new(said).small().color(colour));
            });
            if let Some(w) = why {
                ui.label(RichText::new(w.text(&c.names)));
                for l in w.details(&c.names) {
                    ui.label(RichText::new(l).monospace().small());
                }
            }
            egui::Grid::new(ui.next_auto_id()).num_columns(2).spacing([8.0, 2.0]).show(ui, |ui| {
                for (side, step) in [(0, sa), (1, sb)] {
                    letter(ui, side);
                    ui.vertical(|ui| match step {
                        None => {
                            ui.label(weak("not traced here"));
                        }
                        Some(step) => {
                            ui.label(value(&step.value));
                            // (series that part: the value at that step)
                            if let (Some(at), Some(At::Steps(v))) = (c.step, &step.value)
                                && let Some(x) = v.get(at)
                            {
                                ui.label(weak(format!("at step {}: {}", at + 1, crate::report::cell(*x))).small());
                            }
                            if let How::Expression { with_values, .. } = &step.how
                                && p.kind != Kind::Same
                            {
                                ui.label(RichText::new(format!("here: {with_values}")).monospace().small());
                            }
                            if matches!(step.how, How::Branch { .. }) {
                                ui.label(weak(step.how.branch_text()).small());
                            }
                        }
                    });
                    ui.end_row();
                }
            });
        },
    );
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
                egui::Frame::new()
                    .fill(theme::bar())
                    .corner_radius(crate::theme::rounded(3.0))
                    .inner_margin(Margin::same(4))
                    .show(ui, |ui| {
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
        How::Branch { .. } => {
            ui.label(weak(how.branch_text()));
        }
        How::Local => {}
    }
}
