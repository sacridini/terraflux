//! The Parameters pane: the values of the model offered as a form (`parameters` in the model: an input of a value
//! or a node's setting, with its label, unit, range, step, choices and help), to change a scenario without
//! touching the graph: a slider for a number in a range, a list for a few choices, a box for yes or no, a field for
//! the others; each change an edit of the model (ctrl+Z undoes it), the model run again. The values kept as named
//! scenarios (`scenarios` in the model), applied in a click, sent to Many runs, or compared side by side: each run
//! to an output chosen, its maps, tables or numbers in a column per scenario, with their differences from the
//! first.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::mpsc::{Receiver, channel};

use egui::{RichText, TextureHandle, Ui};
use serde_json::Value as Json;
use terraflux_graph::{Model, Parameter, Scenario};
use terraflux_ops::{Registry, Value};

use crate::app::Session;
use crate::document::{setting_label, words};
use crate::report::{Picture, Shown, visual};
use crate::theme;

/// What a comparison gave of each scenario: its value, or why it failed.
pub type Compared = Vec<(String, Result<Value, String>)>;

/// The image of a value compared (none: not a map), and what of the value it is.
type Image = Option<(TextureHandle, Option<String>)>;

/// The pane's state (the application's).
#[derive(Default)]
pub struct State {
    /// The name of the scenario to save the values as.
    pub name: String,
    /// The scenarios checked to compare, and the output compared (`node.port`).
    pub chosen: BTreeSet<String>,
    pub output: Option<String>,
    /// A comparison running, and the last one done (the output it is of, the scenarios' values and images).
    running: Option<Receiver<Compared>>,
    pub compared: Option<(String, Compared)>,
    images: Vec<(String, Image)>,
    /// The parameter whose description is edited, by key.
    pub editing: Option<String>,
    pub filter: String,
    pub error: Option<String>,
    /// A comparison running in Many runs (on this computer and those helping it, or a cluster), and what it gave
    /// when it ends (the application passes it on).
    pub in_batch: bool,
    pub from_batch: Option<(String, Compared)>,
    /// Whether the maps compared are of classes, as the viewer shows them (their differences: their changes).
    pub classes: bool,
}

impl State {
    pub fn running(&self) -> bool {
        self.running.is_some()
    }
}

/// The value a parameter has now: what the model gives (an input's value, a node's setting or literal input), else
/// none (the operator's default).
pub fn current(m: &Model, key: &str) -> Option<Json> {
    m.setting(key).cloned()
}

/// What a key is: its operator's description of the setting (the input's note for `$name`).
fn doc_of(m: &Model, r: &Registry, key: &str) -> (String, Option<terraflux_graph::PortSpec>) {
    if let Some(name) = key.strip_prefix('$') {
        let note = match m.inputs.get(name) {
            Some(terraflux_graph::Input::Value { note, .. }) => note.clone().unwrap_or_default(),
            _ => String::new(),
        };
        return (note, None);
    }
    let Some((path, name)) = key.rsplit_once('.') else { return (String::new(), None) };
    let mut nodes = &m.nodes;
    let mut node = None;
    for id in path.split('/') {
        let Some(n) = nodes.iter().find(|n| n.id == id) else { return (String::new(), None) };
        node = Some(n);
        if let Some(b) = &n.body {
            nodes = &b.nodes;
        }
    }
    let Some(sig) = node.and_then(|n| r.sig(&n.op)) else { return (String::new(), None) };
    let spec = sig.params.iter().find(|p| p.name == name).or_else(|| sig.input_spec(name)).cloned();
    (spec.as_ref().map(|p| p.doc.clone()).unwrap_or_default(), spec)
}

/// A new parameter of a key: its label from its description (or its name), its help the description; a few
/// choices or yes or no from the operator.
pub fn parameter(m: &Model, r: &Registry, key: &str) -> Parameter {
    let (doc, spec) = doc_of(m, r, key);
    let name = key.strip_prefix('$').unwrap_or_else(|| key.rsplit_once('.').map_or(key, |(_, n)| n));
    let mut label = setting_label(name, &doc);
    if let Some(f) = label.get(..1) {
        label = f.to_uppercase() + &label[1..];
    }
    Parameter {
        key: key.to_string(),
        label: Some(label),
        help: (!doc.is_empty()).then_some(doc),
        choices: spec.map(|s| s.choices.iter().map(|c| editor_value(c)).collect()).unwrap_or_default(),
        ..Default::default()
    }
}

/// A value typed: a number, yes or no, a list or table as JSON, else the text.
fn editor_value(text: &str) -> Json {
    crate::editor::value_of(text)
}

/// The label of a parameter in the form.
pub fn label(p: &Parameter) -> String {
    p.label.clone().unwrap_or_else(|| p.key.clone())
}

/// Whether a parameter's values are whole numbers: its value is one and its step (if any) too.
fn whole(p: &Parameter, v: &Json) -> bool {
    v.is_i64() && p.step.is_none_or(|s| s.fract() == 0.0)
}

/// Set parameters' values in the source (an edit: the model runs again).
fn set(s: &mut Session, values: &BTreeMap<String, Json>) -> Result<(), String> {
    s.source.apply(values).map_err(|e| e.to_string())?;
    s.edited();
    s.run_when_ready = true;
    Ok(())
}

/// A parameter's control; the value it was changed to.
fn control(ui: &mut Ui, s: &mut Session, p: &Parameter, now: Option<&Json>, hint: &str) -> Option<Json> {
    let key = format!("parameter.{}", p.key);
    if !p.choices.is_empty() {
        let shown = now.map(words).unwrap_or_else(|| hint.to_string());
        let mut out = None;
        egui::ComboBox::from_id_salt(&key).selected_text(shown).show_ui(ui, |ui| {
            for c in &p.choices {
                if ui.selectable_label(Some(c) == now, words(c)).clicked() && Some(c) != now {
                    out = Some(c.clone());
                }
            }
        });
        return out;
    }
    match now {
        Some(Json::Bool(b)) => {
            let mut b = *b;
            ui.checkbox(&mut b, "").changed().then_some(Json::Bool(b))
        }
        Some(v @ Json::Number(n)) => {
            let mut x = n.as_f64().unwrap_or(0.0);
            let whole = whole(p, v);
            let resp = match (p.min, p.max) {
                (Some(lo), Some(hi)) if hi > lo => {
                    let mut slider = egui::Slider::new(&mut x, lo..=hi).clamping(egui::SliderClamping::Never);
                    if let Some(st) = p.step.filter(|s| *s > 0.0) {
                        slider = slider.step_by(st);
                    } else if whole {
                        slider = slider.step_by(1.0);
                    }
                    if whole || p.step.is_some_and(|s| s.fract() == 0.0) {
                        slider = slider.max_decimals(0);
                    }
                    if let Some(u) = &p.unit {
                        slider = slider.suffix(format!(" {u}"));
                    }
                    ui.add(slider)
                }
                _ => {
                    let speed = p.step.unwrap_or(if whole { 1.0 } else { (x.abs() * 0.01).max(0.001) });
                    let mut drag = egui::DragValue::new(&mut x).speed(speed);
                    if let Some(lo) = p.min {
                        drag = drag.range(lo..=p.max.unwrap_or(f64::INFINITY));
                    }
                    ui.add(drag)
                }
            };
            // (a slider dragged: once let go, not at every step of the drag)
            let done = resp.drag_stopped() || (resp.changed() && !resp.dragged());
            if done && x != n.as_f64().unwrap_or(0.0) {
                return Some(if whole {
                    Json::from(x.round() as i64)
                } else {
                    serde_json::Number::from_f64(x).map_or(Json::Null, Json::Number)
                });
            }
            None
        }
        _ => {
            let init = now.map(words).unwrap_or_default();
            let mut text = s.fields.entry(key.clone()).or_insert_with(|| init.clone()).clone();
            let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(160.0).hint_text(hint));
            s.fields.insert(key, text.clone());
            (resp.lost_focus() && text != init).then(|| editor_value(&text))
        }
    }
}

/// The description of a parameter edited: its label, unit, range, step, choices and help.
fn describe(ui: &mut Ui, s: &mut Session, k: usize) -> Option<Parameter> {
    let mut p = s.source.parameters[k].clone();
    let before = p.clone();
    let field = |ui: &mut Ui, s: &mut Session, name: &str, init: String, hint: &str| -> Option<String> {
        let key = format!("parameter-edit.{}.{name}", before.key);
        let mut text = s.fields.entry(key.clone()).or_insert_with(|| init.clone()).clone();
        ui.label(RichText::new(name).small());
        let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(150.0).hint_text(hint));
        s.fields.insert(key, text.clone());
        ui.end_row();
        (resp.lost_focus() && text != init).then_some(text)
    };
    let num = |x: Option<f64>| x.map(|v| v.to_string()).unwrap_or_default();
    let parse = |t: &str| t.trim().parse::<f64>().ok();
    let opt = |t: String| if t.trim().is_empty() { None } else { Some(t.trim().to_string()) };
    egui::Grid::new(("parameter edit", &before.key)).num_columns(2).show(ui, |ui| {
        if let Some(t) = field(ui, s, "label", label(&before), "its name in the form") {
            p.label = opt(t);
        }
        if let Some(t) = field(ui, s, "unit", before.unit.clone().unwrap_or_default(), "cells, mm, %...") {
            p.unit = opt(t);
        }
        if let Some(t) = field(ui, s, "min", num(before.min), "the least") {
            p.min = parse(&t);
        }
        if let Some(t) = field(ui, s, "max", num(before.max), "the most") {
            p.max = parse(&t);
        }
        if let Some(t) = field(ui, s, "step", num(before.step), "between values") {
            p.step = parse(&t);
        }
        let choices = before.choices.iter().map(words).collect::<Vec<_>>().join(", ");
        if let Some(t) = field(ui, s, "choices", choices, "a, b, c (none: any value)") {
            p.choices = t.split(',').map(str::trim).filter(|c| !c.is_empty()).map(editor_value).collect();
        }
        if let Some(t) = field(ui, s, "help", before.help.clone().unwrap_or_default(), "what it is, how it acts") {
            p.help = opt(t);
        }
    });
    (p != before).then_some(p)
}

/// The outputs a comparison may be of: the model's outputs, the values its plots show, else its top-level nodes'
/// first outputs.
fn comparable(m: &Model, r: &Registry) -> Vec<String> {
    let mut out: Vec<String> = m.outputs.values().map(|o| o.from.clone()).collect();
    for n in m.nodes.iter().filter(|n| n.op == "view.plot") {
        if let Some(terraflux_graph::Source::Ref(rf)) = n.inputs.get("value").or_else(|| n.inputs.get("table"))
            && !out.contains(rf)
        {
            out.push(rf.clone());
        }
    }
    if out.is_empty() {
        for n in &m.nodes {
            if let Some(p) = crate::builder::first_output(n, r) {
                out.push(format!("{}.{p}", n.id));
            }
        }
    }
    out
}

/// Run each scenario (the source with its values) up to the node of `output` in a thread, its value sent back.
fn compare(st: &mut State, s: &Session, output: &str, ctx: &egui::Context) {
    let Ok(terraflux_graph::Ref::Port { node, port }) = terraflux_graph::Ref::parse(output) else { return };
    let base = s.path.as_ref().and_then(|p| p.parent()).map(std::path::Path::to_path_buf).unwrap_or_default();
    let runs: Vec<(String, Result<Model, String>)> = s
        .source
        .scenarios
        .iter()
        .filter(|sc| st.chosen.contains(&sc.name))
        .map(|sc| (sc.name.clone(), s.source.with_scenario(&sc.name).map_err(|e| e.to_string())))
        .collect();
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let engine = terraflux_exec::Engine::default();
        let mut out = Vec::new();
        for (name, m) in runs {
            let value = m.and_then(|mut m| {
                terraflux_graph::submodel::expand(&mut m, &base).map_err(|e| e.to_string())?;
                let inputs = terraflux_exec::load_inputs(&m, &base, &BTreeMap::new()).map_err(|e| e.to_string())?;
                let res = engine
                    .run_with(&m, &inputs, &mut terraflux_exec::Quiet, None, Some(&node))
                    .map_err(|e| e.to_string())?;
                res.values
                    .get(&node)
                    .and_then(|o| o.get(&port))
                    .cloned()
                    .ok_or_else(|| format!("{node} gave no {port}"))
            });
            out.push((name, value));
        }
        let _ = tx.send(out);
        ctx.request_repaint();
    });
    st.running = Some(rx);
    st.compared = None;
    st.images.clear();
}

/// Whether the maps of a comparison of `output` (`node.port`) are of classes, as the viewer shows that output
/// (its operator's palette, the model's legend, else its values).
fn classes_of(s: &Session, output: &str, c: &Compared) -> bool {
    let Ok(terraflux_graph::Ref::Port { node, port }) = terraflux_graph::Ref::parse(output) else { return false };
    match c.iter().find_map(|(_, v)| v.as_ref().ok().and_then(last)) {
        Some(Value::Map(m)) => {
            matches!(crate::builder::output_look(s, &node, &port, &m).coloring, crate::palette::Coloring::Classes(_))
        }
        _ => false,
    }
}

/// A value as compared: a series' last step.
fn last(v: &Value) -> Option<Value> {
    match v {
        Value::Series(s) => s.last().cloned(),
        v => Some(v.clone()),
    }
}

/// Whether a map holds classes: whole numbers, a few of them.
fn is_classes(data: &[f64]) -> bool {
    let mut seen = BTreeSet::new();
    for v in data.iter().filter(|v| v.is_finite()) {
        if v.fract() != 0.0 || (seen.insert(*v as i64) && seen.len() > 32) {
            return false;
        }
    }
    !seen.is_empty()
}

/// The cells that changed class, of the cells both maps have, and each change (from, to) with its cells.
pub type Changes = (usize, usize, Vec<((i64, i64), usize)>);

/// How the classes of two maps of the same grid differ, the changes of the most cells first; none unless both are
/// maps of classes.
pub fn class_changes(a: &Value, b: &Value) -> Option<Changes> {
    let (Value::Map(x), Value::Map(y)) = (last(a)?, last(b)?) else { return None };
    if x.data.len() != y.data.len() || !is_classes(&x.data) || !is_classes(&y.data) {
        return None;
    }
    let mut changes: BTreeMap<(i64, i64), usize> = BTreeMap::new();
    let mut both = 0;
    for (p, q) in x.data.iter().zip(y.data.iter()).filter(|(p, q)| p.is_finite() && q.is_finite()) {
        both += 1;
        if p != q {
            *changes.entry((*p as i64, *q as i64)).or_default() += 1;
        }
    }
    let mut list: Vec<((i64, i64), usize)> = changes.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Some((list.iter().map(|(_, n)| n).sum(), both, list))
}

/// A value's difference from another's: maps of classes (`classes`: as the viewer shows them) the class each cell
/// changed to (no data where it did not change), other maps of the same grid cell by cell, tables of the same
/// shape cell by cell, numbers; none otherwise.
pub fn difference(a: &Value, b: &Value, classes: bool) -> Option<Value> {
    match (last(a)?, last(b)?) {
        (Value::Map(x), Value::Map(y))
            if classes && x.data.len() == y.data.len() && is_classes(&x.data) && is_classes(&y.data) =>
        {
            let d = x.data.iter().zip(y.data.iter()).map(|(p, q)| if p == q { f64::NAN } else { *q }).collect();
            Some(Value::Map(terraflux_ops::Map::new(y.grid.clone(), d)))
        }
        (Value::Map(x), Value::Map(y)) if x.data.len() == y.data.len() => {
            let d = x.data.iter().zip(y.data.iter()).map(|(p, q)| q - p).collect();
            Some(Value::Map(terraflux_ops::Map::new(y.grid.clone(), d)))
        }
        (Value::Table(x), Value::Table(y)) if x.columns == y.columns && x.rows.len() == y.rows.len() => {
            let mut t = terraflux_ops::Table::new(y.columns.clone());
            t.rows = x.rows.iter().zip(&y.rows).map(|(p, q)| p.iter().zip(q).map(|(a, b)| b - a).collect()).collect();
            Some(Value::Table(t))
        }
        (x, y) => Some(Value::Number(y.as_number()? - x.as_number()?)),
    }
}

/// A value of a comparison: its image, a table's first rows, or words.
fn shown_value(ui: &mut Ui, img: &Image, v: &Value, w: f32) {
    if let Some((t, of)) = img {
        let size = t.size_vec2();
        ui.add(egui::Image::new((t.id(), size * (w / size.x.max(1.0)))));
        if let Some(of) = of {
            ui.label(RichText::new(of).small().weak());
        }
        return;
    }
    match visual(v, 0).map(|v| v.shown) {
        Some(Shown::Table { columns, rows, .. }) => {
            egui::Grid::new(ui.next_auto_id()).striped(true).show(ui, |ui| {
                for c in &columns {
                    ui.label(RichText::new(c).small().strong());
                }
                ui.end_row();
                for r in &rows {
                    for c in r {
                        ui.label(RichText::new(c).small().monospace());
                    }
                    ui.end_row();
                }
            });
        }
        Some(Shown::Text(t)) => {
            ui.label(RichText::new(t).monospace());
        }
        _ => {}
    }
}

/// The images of a comparison's values and differences (made once, when it comes).
fn images(ctx: &egui::Context, compared: &Compared, classes: bool) -> Vec<(String, Image)> {
    let pic = |name: &str, v: &Value| {
        let vis = visual(v, 256)?;
        let Shown::Picture(Picture { width, height, rgba }) = &vis.shown else { return None };
        let img = egui::ColorImage::from_rgba_premultiplied([*width, *height], rgba);
        Some((ctx.load_texture(format!("scenario-{name}"), img, egui::TextureOptions::NEAREST), vis.of))
    };
    let mut out = Vec::new();
    let first = compared.first().and_then(|(_, v)| v.as_ref().ok());
    for (name, v) in compared {
        let Ok(v) = v else {
            out.push((name.clone(), None));
            continue;
        };
        out.push((name.clone(), pic(name, v)));
        let d = first.and_then(|f| difference(f, v, classes));
        out.push((format!("{name} - first"), d.as_ref().and_then(|d| pic(&format!("{name}-diff"), d))));
    }
    out
}

/// The scenarios as Many runs' table: a row per scenario, a column per value it changes.
pub fn to_batch(st: &mut crate::batch_view::BatchState, m: &Model) {
    let mut columns: Vec<String> = Vec::new();
    for sc in &m.scenarios {
        for k in sc.values.keys() {
            if !columns.contains(k) {
                columns.push(k.clone());
            }
        }
    }
    st.kind = crate::batch_view::Kind::Scenarios;
    st.scenarios = m
        .scenarios
        .iter()
        .map(|sc| {
            let values = columns.iter().map(|c| sc.values.get(c).map(|v| v.to_string()).unwrap_or_default()).collect();
            (sc.name.clone(), values)
        })
        .collect();
    st.columns = columns;
}

/// The parameters of a range as Many runs' calibration: each tried from its least to its most (five points, or
/// one per step when they are fewer).
pub fn to_calibration(st: &mut crate::batch_view::BatchState, m: &Model) {
    st.kind = crate::batch_view::Kind::Calibration;
    st.ranges = m
        .parameters
        .iter()
        .filter_map(|p| {
            let (lo, hi) = (p.min?, p.max?);
            let points = p.step.filter(|s| *s > 0.0).map_or(5, |s| (((hi - lo) / s).round() as usize + 1).clamp(2, 5));
            Some(crate::batch_view::Range {
                key: p.key.clone(),
                from: words(&serde_json::json!(lo)),
                to: words(&serde_json::json!(hi)),
                points: points.to_string(),
            })
        })
        .collect();
}

/// What the pane asks of the application.
pub enum Asked {
    /// The scenarios to Many runs.
    ManyRuns,
    /// The parameters of a range to Many runs' calibration.
    Calibrate,
    /// The scenarios checked compared through Many runs, on the output (`node.port`).
    CompareInManyRuns(String),
}

/// The scenarios checked as a batch of Many runs that keeps the output compared, its runs sent back to the pane.
pub fn to_batch_compare(st: &mut crate::batch_view::BatchState, m: &Model, chosen: &BTreeSet<String>, output: &str) {
    to_batch(st, m);
    st.scenarios.retain(|(name, _)| chosen.contains(name));
    st.keep = Some([output.to_string()].into());
    st.out = None;
    st.compare = Some(output.to_string());
}

/// The Parameters pane.
pub fn pane(ui: &mut Ui, st: &mut State, s: &mut Session, r: &Registry) -> Option<Asked> {
    let mut asked = None;
    if let Some(rx) = &st.running
        && let Ok(c) = rx.try_recv()
    {
        st.running = None;
        st.from_batch = Some((st.output.clone().unwrap_or_default(), c));
    }
    if let Some((output, c)) = st.from_batch.take() {
        st.classes = classes_of(s, &output, &c);
        st.images = images(ui.ctx(), &c, st.classes);
        st.compared = Some((output, c));
    }
    egui::ScrollArea::vertical().id_salt("parameters pane").show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            theme::section(ui, "Parameters");
            if s.source.parameters.is_empty() {
                ui.label(
                    RichText::new(
                        "The values to change a scenario with, as a form: add an input of the model or a node's \
                         setting below (a range, a unit and a help each), without touching the graph.",
                    )
                    .weak(),
                );
            }
            parameters_ui(ui, st, s, r);
            ui.add_space(8.0);
            theme::section(ui, "Scenarios");
            scenarios_ui(ui, st, s, r, &mut asked);
            if let Some(e) = &st.error {
                ui.colored_label(ui.visuals().error_fg_color, e);
            }
        });
    });
    asked
}

fn parameters_ui(ui: &mut Ui, st: &mut State, s: &mut Session, r: &Registry) {
    let mut change: Option<(String, Json)> = None;
    let mut described: Option<(usize, Parameter)> = None;
    let mut removed = None;
    let mut moved = None;
    let n = s.source.parameters.len();
    for k in 0..n {
        let p = s.source.parameters[k].clone();
        let now = current(&s.source, &p.key);
        let (doc, _) = doc_of(&s.source, r, &p.key);
        let default = crate::node_view::default_of(&doc);
        let hint = if default.is_empty() { "default".to_string() } else { default };
        egui::Frame::new()
            .fill(theme::well())
            .corner_radius(crate::theme::rounded(4.0))
            .inner_margin(egui::Margin::same(6))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new(label(&p)).strong()).on_hover_text(&p.key);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .small_button("x")
                            .on_hover_text("take it out of the form (the model keeps its value)")
                            .clicked()
                        {
                            removed = Some(k);
                        }
                        let editing = st.editing.as_deref() == Some(p.key.as_str());
                        if ui.small_button("✏").on_hover_text("its label, unit, range, choices and help").clicked() {
                            st.editing = if editing { None } else { Some(p.key.clone()) };
                        }
                        if k > 0 && ui.small_button("⏶").on_hover_text("up").clicked() {
                            moved = Some((k, k - 1));
                        }
                    });
                });
                ui.horizontal_wrapped(|ui| {
                    if let Some(v) = control(ui, s, &p, now.as_ref(), &hint) {
                        change = Some((p.key.clone(), v));
                    }
                    let slider = p.min.is_some() && p.max.is_some() && now.as_ref().is_some_and(Json::is_number);
                    if let Some(u) = p.unit.as_ref().filter(|_| !slider) {
                        ui.label(RichText::new(u).weak());
                    }
                    if now.is_none() {
                        ui.label(RichText::new("(the operator's default)").small().weak());
                    }
                });
                if let Some(h) = &p.help {
                    ui.label(RichText::new(h).small().weak());
                }
                if st.editing.as_deref() == Some(p.key.as_str())
                    && let Some(d) = describe(ui, s, k)
                {
                    described = Some((k, d));
                }
            });
        ui.add_space(3.0);
    }
    // a setting of the model to the form
    let knobs: Vec<crate::batch_view::Knob> = crate::batch_view::knobs(&s.source, r)
        .into_iter()
        .filter(|kn| !s.source.parameters.iter().any(|p| p.key == kn.key))
        .collect();
    let mut key = String::new();
    if crate::batch_view::knob_picker(ui, "add parameter", &knobs, &mut key, &mut st.filter, "+ Parameter") {
        let p = parameter(&s.source, r, &key);
        s.source.parameters.push(p);
        s.edited();
    }
    if let Some((key, v)) = change
        && let Err(e) = set(s, &[(key, v)].into())
    {
        st.error = Some(e);
    }
    if let Some((k, p)) = described {
        s.source.parameters[k] = p;
        s.edited();
    }
    if let Some((a, b)) = moved {
        s.source.parameters.swap(a, b);
        s.edited();
    }
    if let Some(k) = removed {
        s.source.parameters.remove(k);
        s.edited();
    }
}

fn scenarios_ui(ui: &mut Ui, st: &mut State, s: &mut Session, r: &Registry, asked: &mut Option<Asked>) {
    // the values now, kept as a scenario
    ui.horizontal_wrapped(|ui| {
        ui.add(egui::TextEdit::singleline(&mut st.name).desired_width(150.0).hint_text("a scenario's name"));
        let can = !st.name.trim().is_empty() && !s.source.parameters.is_empty();
        if ui
            .add_enabled(can, egui::Button::new("Save the values"))
            .on_hover_text("the parameters' values now, as a scenario of this name (replaced if it is one)")
            .clicked()
        {
            let values: BTreeMap<String, Json> =
                s.source.parameters.iter().filter_map(|p| Some((p.key.clone(), current(&s.source, &p.key)?))).collect();
            let name = st.name.trim().to_string();
            let note = s.source.scenarios.iter().find(|sc| sc.name == name).and_then(|sc| sc.note.clone());
            s.source.scenarios.retain(|sc| sc.name != name);
            s.source.scenarios.push(Scenario { name: name.clone(), note, values });
            st.chosen.insert(name);
            st.name.clear();
            s.edited();
        }
    });
    if s.source.scenarios.is_empty() {
        ui.label(RichText::new("No scenario yet: set the parameters, then save their values under a name.").weak());
        return;
    }
    let labels: BTreeMap<String, String> = s.source.parameters.iter().map(|p| (p.key.clone(), label(p))).collect();
    let mut apply = None;
    let mut delete = None;
    for sc in &s.source.scenarios {
        ui.horizontal_wrapped(|ui| {
            let mut on = st.chosen.contains(&sc.name);
            if ui.checkbox(&mut on, RichText::new(&sc.name).strong()).on_hover_text("compare it").changed() {
                if on {
                    st.chosen.insert(sc.name.clone());
                } else {
                    st.chosen.remove(&sc.name);
                }
            }
            let values: Vec<String> = sc
                .values
                .iter()
                .map(|(k, v)| format!("{} {}", labels.get(k).cloned().unwrap_or_else(|| k.clone()), words(v)))
                .collect();
            ui.label(RichText::new(values.join(" · ")).small().weak());
            if ui.small_button("Apply").on_hover_text("its values in the model (ctrl+Z undoes it)").clicked() {
                apply = Some(sc.values.clone());
            }
            if ui.small_button("x").on_hover_text("forget it").clicked() {
                delete = Some(sc.name.clone());
            }
        });
        if let Some(n) = &sc.note {
            ui.label(RichText::new(n).small().italics());
        }
    }
    if let Some(v) = apply
        && let Err(e) = set(s, &v)
    {
        st.error = Some(e);
    }
    if let Some(name) = delete {
        s.source.scenarios.retain(|sc| sc.name != name);
        st.chosen.remove(&name);
        s.edited();
    }
    ui.add_space(4.0);
    // side by side
    let outputs = comparable(&s.source, r);
    if st.output.as_ref().is_none_or(|o| !outputs.contains(o)) {
        st.output = outputs.first().cloned();
    }
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("compare output")
            .selected_text(st.output.clone().unwrap_or_else(|| "an output".into()))
            .show_ui(ui, |ui| {
                for o in &outputs {
                    if ui.selectable_label(st.output.as_ref() == Some(o), o).clicked() {
                        st.output = Some(o.clone());
                    }
                }
            });
        let chosen = s.source.scenarios.iter().filter(|sc| st.chosen.contains(&sc.name)).count();
        let can = chosen >= 1 && st.output.is_some() && !st.running() && !st.in_batch;
        if ui
            .add_enabled(can, crate::app::accent_button("Compare", true))
            .on_hover_text("run each scenario checked up to the output, side by side, with its difference from the first")
            .clicked()
            && let Some(o) = st.output.clone()
        {
            compare(st, s, &o, ui.ctx());
        }
        if ui
            .add_enabled(can, egui::Button::new("Compare in Many runs"))
            .on_hover_text(
                "the same, the scenarios run at once as a batch: on this computer and those helping it, or a cluster \
                 (Many runs' choice)",
            )
            .clicked()
            && let Some(o) = st.output.clone()
        {
            *asked = Some(Asked::CompareInManyRuns(o));
        }
        if st.running() || st.in_batch {
            ui.spinner();
        }
        if st.in_batch {
            ui.label(RichText::new("running in Many runs").small().weak());
        }
        if ui.button("To Many runs").on_hover_text("the scenarios as Many runs' table, to run them all at once (on other computers too)").clicked() {
            *asked = Some(Asked::ManyRuns);
        }
        let ranged = s.source.parameters.iter().any(|p| p.min.is_some() && p.max.is_some());
        if ui
            .add_enabled(ranged, egui::Button::new("Calibrate"))
            .on_hover_text("the parameters of a range as Many runs' calibration: tried in their ranges, scored against an observed map")
            .clicked()
        {
            *asked = Some(Asked::Calibrate);
        }
    });
    if let Some((output, compared)) = &st.compared {
        ui.label(RichText::new(output).monospace().weak());
        let cols = compared.len().max(1);
        let w = ((ui.available_width() - 8.0 * cols as f32) / cols as f32).clamp(60.0, 220.0);
        egui::ScrollArea::horizontal().id_salt("compared").show(ui, |ui| {
            egui::Grid::new("compared grid").spacing([8.0, 4.0]).show(ui, |ui| {
                for (name, _) in compared {
                    ui.label(RichText::new(name).strong());
                }
                ui.end_row();
                for (name, v) in compared {
                    ui.vertical(|ui| match v {
                        Ok(v) => {
                            let img = st.images.iter().find(|(n, _)| n == name).and_then(|(_, i)| i.clone());
                            shown_value(ui, &img, v, w);
                        }
                        Err(e) => {
                            ui.colored_label(ui.visuals().error_fg_color, e);
                        }
                    });
                }
                ui.end_row();
                // the differences from the first
                let first = compared.first().and_then(|(_, v)| v.as_ref().ok());
                for (k, (name, v)) in compared.iter().enumerate() {
                    ui.vertical(|ui| {
                        if k == 0 {
                            let text = if st.classes {
                                "where the others' classes differ from this one's:"
                            } else {
                                "the others less this one:"
                            };
                            ui.label(RichText::new(text).small().weak());
                            return;
                        }
                        let (Some(f), Ok(v)) = (first, v) else { return };
                        match difference(f, v, st.classes) {
                            Some(d) => {
                                let key = format!("{name} - first");
                                let img = st.images.iter().find(|(n, _)| *n == key).and_then(|(_, i)| i.clone());
                                shown_value(ui, &img, &d, w);
                                // a map of classes: how many cells changed, and the changes the most first
                                if let Some((changed, of, list)) = class_changes(f, v).filter(|_| st.classes) {
                                    let share = 100.0 * changed as f64 / of.max(1) as f64;
                                    ui.label(
                                        RichText::new(format!("{changed} cells changed class ({share:.1}%)")).small(),
                                    );
                                    for ((from, to), n) in list.iter().take(5) {
                                        ui.label(
                                            RichText::new(format!("{from} → {to}: {n}")).small().monospace().weak(),
                                        );
                                    }
                                    if list.len() > 5 {
                                        ui.label(
                                            RichText::new(format!("and {} more changes", list.len() - 5))
                                                .small()
                                                .weak(),
                                        );
                                    }
                                }
                            }
                            None => {
                                ui.label(RichText::new("not comparable").small().weak());
                            }
                        }
                    });
                }
                ui.end_row();
            });
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        let mut m = crate::templates::TEMPLATES.iter().find(|t| t.name == "Neutral landscapes").unwrap().model();
        let r = Registry::standard();
        let mut p = parameter(&m, &r, "field.range");
        (p.min, p.max, p.step) = (Some(2.0), Some(60.0), Some(5.0));
        m.parameters.push(p);
        m.parameters.push(parameter(&m, &r, "clusters.seed"));
        m.scenarios.push(Scenario { name: "a".into(), note: None, values: [("field.range".into(), 5.into())].into() });
        m.scenarios.push(Scenario {
            name: "b".into(),
            note: Some("both".into()),
            values: [("field.range".into(), 40.into()), ("clusters.seed".into(), 9.into())].into(),
        });
        m
    }

    #[test]
    fn a_parameter_is_told_by_its_operator() {
        let m = model();
        assert_eq!(label(&m.parameters[0]), "The range of the autocorrelation, in cells");
        assert_eq!(current(&m, "field.range"), Some(20.into()));
        assert_eq!(current(&m, "field.nugget"), None, "the operator's default");
        let d = crate::document::Document::of(&m, &Registry::standard());
        assert_eq!(d.parameters[0].value, "20");
        assert_eq!(d.parameters[0].range.as_deref(), Some("2 to 60, by 5"));
        assert_eq!(d.scenarios[1].values[1], ("The range of the autocorrelation, in cells".into(), "40".into()));
        assert_eq!(d.scenarios[1].note.as_deref(), Some("both"));
    }

    #[test]
    fn scenarios_go_to_many_runs() {
        let mut st = crate::batch_view::BatchState::new();
        to_calibration(&mut st, &model());
        assert_eq!(st.kind, crate::batch_view::Kind::Calibration);
        assert_eq!(
            st.ranges,
            [crate::batch_view::Range {
                key: "field.range".into(),
                from: "2".into(),
                to: "60".into(),
                points: "5".into()
            }]
        );
        to_batch(&mut st, &model());
        assert_eq!(st.kind, crate::batch_view::Kind::Scenarios);
        assert_eq!(st.columns, ["field.range", "clusters.seed"]);
        assert_eq!(st.scenarios[0], ("a".to_string(), vec!["5".to_string(), String::new()]));
        assert_eq!(st.scenarios[1], ("b".to_string(), vec!["40".to_string(), "9".to_string()]));
    }

    #[test]
    fn differences_of_maps_tables_and_numbers() {
        let grid = std::sync::Arc::new(terraflux_raster::Grid::new(2, 1));
        let a = Value::Map(terraflux_ops::Map::new(grid.clone(), vec![1.0, 2.0]));
        let b = Value::Series(vec![Value::Map(terraflux_ops::Map::new(grid, vec![4.0, 0.0]))]);
        let Some(Value::Map(d)) = difference(&a, &b, false) else { panic!() };
        assert_eq!(d.data.as_ref(), &vec![3.0, -2.0], "the last step of a series, less the first");
        let mut t = terraflux_ops::Table::new(vec!["x".into()]);
        t.rows = vec![vec![1.0]];
        let mut u = t.clone();
        u.rows = vec![vec![1.5]];
        let Some(Value::Table(d)) = difference(&Value::Table(t), &Value::Table(u), false) else { panic!() };
        assert_eq!(d.rows, vec![vec![0.5]]);
        assert!(matches!(difference(&Value::Number(1.0), &Value::Number(3.0), false), Some(Value::Number(2.0))));
        assert!(difference(&Value::Number(1.0), &Value::Text("x".into()), false).is_none());
    }

    #[test]
    fn maps_of_classes_differ_by_their_changes() {
        let grid = std::sync::Arc::new(terraflux_raster::Grid::new(3, 2));
        let map = |v: Vec<f64>| Value::Map(terraflux_ops::Map::new(grid.clone(), v));
        let a = map(vec![1.0, 1.0, 2.0, 2.0, 3.0, f64::NAN]);
        let b = map(vec![1.0, 2.0, 2.0, 3.0, 2.0, 1.0]);
        // the class each cell changed to, no data where it stayed
        let Some(Value::Map(d)) = difference(&a, &b, true) else { panic!() };
        let shown: Vec<String> = d.data.iter().map(|v| v.to_string()).collect();
        assert_eq!(shown, ["NaN", "2", "NaN", "3", "2", "1"]);
        // three changes of a cell each, of the five cells both have
        let (changed, of, list) = class_changes(&a, &b).unwrap();
        assert_eq!((changed, of), (3, 5));
        assert_eq!(list, [((1, 2), 1), ((2, 3), 1), ((3, 2), 1)]);
        // continuous values: subtracted, no changes of class
        let fine = map(vec![0.5, 1.0, 2.0, 2.0, 3.0, 1.0]);
        assert!(class_changes(&fine, &b).is_none());
        let Some(Value::Map(d)) = difference(&fine, &b, true) else { panic!() };
        assert_eq!(d.data[0], 0.5);
        // classes the viewer does not show as classes (counts...): subtracted
        let Some(Value::Map(d)) = difference(&a, &b, false) else { panic!() };
        assert_eq!(d.data[1], 1.0);
    }
}
