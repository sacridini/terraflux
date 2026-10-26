//! The calculator: a console for the scripts of map algebra (`map.calc`, [`terraflux_ops::algebra`]) under the
//! model. The script of the node selected with its words in colour and its first error underlined where it is;
//! the names completed as they are typed (the node's inputs, a stack's layers after `.`, the functions, the
//! methods); its outputs computed at once on a sample of the cells, beside it; and a line under it that computes
//! anything in the script's scope (`img.mean()` gives the number at once) without changing the node.
//!
//! The inputs are what the node is connected to: their values from the last run, or the headers of rasters on
//! disk (a stack's layers are known before anything runs). The script goes to the node with ctrl+enter (which
//! runs the model up to it), Apply, or when the editor is left.

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::ops::Range;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TryRecvError, channel};
use std::time::{Duration, Instant};

use egui::text::{CCursor, CCursorRange, LayoutJob};
use egui::{Color32, Key, Margin, RichText, TextFormat, TextureHandle, Ui};
use terraflux_graph::{Node, Ref, Source};
use terraflux_ops::Value;
use terraflux_ops::algebra::Out;
use terraflux_ops::algebra_tools::{
    self, Completion, FUNCTION_HELP, Input, Inputs, METHOD_HELP, Part, Preview, Problem,
};

use crate::app::Session;
use crate::palette::Coloring;
use crate::theme;

/// The cells of a preview, at most.
const SAMPLE: usize = 160_000;

/// The pause after typing before a preview is computed.
const PAUSE: Duration = Duration::from_millis(300);

/// The side of an image of an output, pixels at most.
const SIDE: usize = 220;

/// The editor's id (the model's ctrl+enter leaves it to the calculator).
fn editor_id() -> egui::Id {
    egui::Id::new("calculator script")
}

/// Whether the calculator's script is being typed in.
pub fn editing(ctx: &egui::Context) -> bool {
    ctx.memory(|m| m.has_focus(editor_id()))
}

/// A line of the console.
enum Line {
    /// What was typed.
    Asked(String),
    /// A number, a map's or a stack's summary (an image of the map, of a stack's first layer).
    Answer(String, Option<TextureHandle>),
    Error(String),
    Note(String),
}

/// What a line typed gave (computed in a thread): the grid's size and the value.
type Evaluated = Result<(usize, usize, Out), String>;

/// An output of a preview as shown: its name, what it is, its numbers and its images (a stack's first layers).
struct Shown {
    name: String,
    what: String,
    numbers: String,
    images: Vec<(String, TextureHandle)>,
}

/// The calculator's state.
#[derive(Default)]
pub struct Calculator {
    /// The node edited: its container's path and its id.
    node: Option<(Vec<String>, String)>,
    /// The script as typed, and as the node has it.
    text: String,
    applied: String,
    /// The completions shown: the bytes of the word they replace, the list, the one chosen.
    popup: Option<(Range<usize>, Vec<Completion>, usize)>,
    /// When the script was last typed in (its preview waits for a pause).
    typed_at: Option<Instant>,
    /// The preview asked for (what it was of), and the thread computing it.
    asked: Option<u64>,
    computing: Option<Receiver<(u64, Result<Preview, String>)>>,
    /// The last preview: what it was of, its outputs (or why there are none), the sample's size and step.
    shown: Option<(u64, Result<Vec<Shown>, String>, String)>,
    /// What the inputs connected to rasters not run yet are, by their headers (by path and band).
    headers: HashMap<(String, String), Input>,
    /// The console's lines, its prompt, what was typed in it (up and down bring it back), a line being computed.
    lines: Vec<Line>,
    input: String,
    typed: Vec<String>,
    recalled: Option<usize>,
    evaluating: Option<Receiver<Evaluated>>,
    images: usize,
}

/// What the calculator asks of the application.
pub enum Asked {
    /// The model run up to the node.
    RunTo(String),
}

/// The node a calculator edits: the node selected when it is a `map.calc`, else the one it edited (while it is
/// still there).
fn target(c: &Calculator, s: &Session) -> Option<(Vec<String>, Node)> {
    let path = s.editor.path.clone();
    if let Some(id) = &s.selected
        && let Some(n) = s.source.scope(&path).and_then(|v| v.iter().find(|n| &n.id == id))
        && n.op == "map.calc"
    {
        return Some((path, n.clone()));
    }
    let (path, id) = c.node.as_ref()?;
    let n = s.source.scope(path)?.iter().find(|n| &n.id == id && n.op == "map.calc")?;
    Some((path.clone(), n.clone()))
}

/// The script a node has.
fn script_of(n: &Node) -> String {
    match n.params.get("expression") {
        Some(serde_json::Value::String(t)) => t.clone(),
        Some(v) => v.to_string(),
        None => String::new(),
    }
}

/// What each input of a node is, and the values known (none for those not run yet).
fn connected(c: &mut Calculator, s: &Session, path: &[String], n: &Node) -> (Inputs, BTreeMap<String, Value>) {
    let (mut inputs, mut values) = (Inputs::new(), BTreeMap::new());
    for (port, src) in &n.inputs {
        let value = match src {
            Source::Literal(v) => v.as_f64().map(Value::Number),
            Source::Ref(r) => match Ref::parse(r) {
                // (the outputs of the nodes in a container are not kept)
                Ok(Ref::Port { node, port: p }) if path.is_empty() => s.value_of(&node, &p),
                Ok(Ref::Input(name)) => s.inputs.as_ref().and_then(|i| i.get(&name)).cloned(),
                _ => None,
            },
        };
        let input = match &value {
            Some(v) => algebra_tools::describe(v),
            None => header(c, s, path, src),
        };
        inputs.insert(port.clone(), input);
        if let Some(v) = value {
            values.insert(port.clone(), v);
        }
    }
    (inputs, values)
}

/// What an input connected to a raster read and not run yet is, by the raster's header (read once).
fn header(c: &mut Calculator, s: &Session, path: &[String], src: &Source) -> Input {
    let Source::Ref(r) = src else { return Input::Unknown };
    let Ok(Ref::Port { node, .. }) = Ref::parse(r) else { return Input::Unknown };
    let Some(n) = s.source.scope(path).and_then(|v| v.iter().find(|n| n.id == node)) else { return Input::Unknown };
    let Some(file) = n.params.get("path").and_then(|p| p.as_str()).filter(|_| n.op == "raster.read") else {
        return Input::Unknown;
    };
    let band = n.params.get("band").map(|b| b.as_str().map_or(b.to_string(), String::from));
    let key = (file.to_string(), band.clone().unwrap_or_default());
    if let Some(i) = c.headers.get(&key) {
        return i.clone();
    }
    let at = s.base().join(file).to_string_lossy().into_owned();
    let input = match terraflux_ops::files::Rasters::open(&at, band.as_deref(), None) {
        Ok(r) => algebra_tools::describe(&Value::Raster(Arc::new(r.picking(band.is_some())))),
        Err(_) => Input::Unknown,
    };
    c.headers.insert(key, input.clone());
    input
}

/// What a preview is of: the script and the values it reads (the same data: the same preview).
fn key_of(text: &str, values: &BTreeMap<String, Value>) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    for (name, v) in values {
        name.hash(&mut h);
        match v {
            Value::Map(m) => (Arc::as_ptr(&m.data) as usize).hash(&mut h),
            Value::Stack(st) => st.layers.iter().for_each(|l| (Arc::as_ptr(l) as usize).hash(&mut h)),
            Value::Raster(r) => (Arc::as_ptr(r) as usize).hash(&mut h),
            v => v.as_number().map(f64::to_bits).hash(&mut h),
        }
    }
    h.finish()
}

/// The colour of a part of a script.
fn colour(ui: &Ui, part: Part) -> Color32 {
    let dark = theme::current().dark;
    let pick = |d: (u8, u8, u8), l: (u8, u8, u8)| {
        let (r, g, b) = if dark { d } else { l };
        Color32::from_rgb(r, g, b)
    };
    match part {
        Part::Number => pick((181, 206, 168), (9, 120, 80)),
        Part::Input => theme::accent(),
        Part::Name => ui.visuals().text_color(),
        Part::Layer | Part::Method => pick((156, 210, 250), (0, 100, 175)),
        Part::Function => pick((220, 220, 170), (120, 90, 30)),
        Part::Word => pick((205, 150, 200), (160, 0, 200)),
        Part::Operator => ui.visuals().text_color(),
        Part::Comment => theme::weak(),
    }
}

/// The script's text laid out: its parts in their colours, the error underlined.
fn layout(ui: &Ui, text: &str, inputs: &Inputs, problem: Option<&Problem>, wrap: f32) -> LayoutJob {
    let font = egui::TextStyle::Monospace.resolve(ui.style());
    let plain = TextFormat::simple(font.clone(), ui.visuals().text_color());
    // (the spaces at the end of a line kept: hiding them is odd when typing)
    let mut job = LayoutJob { keep_trailing_whitespace: true, ..Default::default() };
    job.wrap.max_width = wrap;
    if text.is_empty() {
        job.append("", 0.0, plain);
        return job;
    }
    let mut colours = vec![None; text.len()];
    for (r, part) in algebra_tools::parts(text, inputs) {
        for c in &mut colours[r] {
            *c = Some(part);
        }
    }
    let bad =
        problem.map(|p| if p.at.is_empty() { p.at.start..(p.at.start + 1).min(text.len()) } else { p.at.clone() });
    let error = ui.visuals().error_fg_color;
    let mut start = 0;
    // (runs of the same look; a run ends at a character's boundary)
    let look = |i: usize| (colours[i], bad.as_ref().is_some_and(|b| b.contains(&i)));
    for (i, _) in text.char_indices().skip(1).chain([(text.len(), ' ')]) {
        if i < text.len() && look(i) == look(start) {
            continue;
        }
        let (part, wrong) = look(start);
        let mut f = plain.clone();
        if let Some(p) = part {
            f.color = colour(ui, p);
            f.italics = p == Part::Comment;
        }
        if wrong {
            f.underline = egui::Stroke::new(1.5, error);
        }
        job.append(&text[start..i], 0.0, f);
        start = i;
    }
    job
}

/// The byte at a character of a text.
fn byte_at(text: &str, chars: usize) -> usize {
    text.char_indices().nth(chars).map_or(text.len(), |(i, _)| i)
}

/// The text in the terminal's font.
fn mono(t: impl Into<String>) -> RichText {
    RichText::new(t).monospace()
}

/// A number as the console writes it.
fn number(x: f64) -> String {
    if x.is_nan() {
        "null".into()
    } else if x == x.round() && x.abs() < 1e15 {
        format!("{x}")
    } else {
        let s = format!("{x:.6}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// A map's numbers: the least, the mean, the largest, the cells missing.
fn numbers(cells: &[f64]) -> String {
    let (lo, mean, hi, missing) = algebra_tools::summary(cells);
    let missing = if missing > 0 { format!(" · {missing} null") } else { String::new() };
    format!("min {} · mean {} · max {}{missing}", number(lo), number(mean), number(hi))
}

impl Calculator {
    /// The image of a map of `width` x `height` cells (none without cells).
    fn image(&mut self, ctx: &egui::Context, cells: &[f64], width: usize, height: usize) -> Option<TextureHandle> {
        if width * height == 0 || cells.len() < width * height {
            return None;
        }
        let coloring = Coloring::for_values(cells);
        let (img, _) = crate::palette::image(cells, width, height, SIDE, &coloring);
        self.images += 1;
        Some(ctx.load_texture(format!("calculator {}", self.images), img, egui::TextureOptions::NEAREST))
    }

    /// A preview as shown: each output's images and numbers.
    fn show_preview(&mut self, ctx: &egui::Context, p: Preview) -> (Vec<Shown>, String) {
        let mut out = Vec::new();
        for (name, o) in p.outputs {
            let shown = match o {
                Out::Number(x) => Shown { name, what: "number".into(), numbers: number(x), images: Vec::new() },
                Out::Map(cells) => Shown {
                    name,
                    what: "map".into(),
                    numbers: numbers(&cells),
                    images: self
                        .image(ctx, &cells, p.width, p.height)
                        .map(|t| (String::new(), t))
                        .into_iter()
                        .collect(),
                },
                Out::Stack(layers) => {
                    let n = layers.len();
                    let all: Vec<f64> = layers.iter().flat_map(|(_, c)| c.iter().copied()).collect();
                    let images = layers
                        .iter()
                        .take(4)
                        .filter_map(|(k, c)| Some((k.to_string(), self.image(ctx, c, p.width, p.height)?)))
                        .collect();
                    Shown { name, what: format!("stack of {n} layers"), numbers: numbers(&all), images }
                }
            };
            out.push(shown);
        }
        let sample = if p.step > 1 {
            format!("on one cell in {} ({} x {})", p.step * p.step, p.width, p.height)
        } else {
            format!("on every cell ({} x {})", p.width, p.height)
        };
        (out, sample)
    }

    /// Take what the threads computed.
    fn poll(&mut self, ctx: &egui::Context) {
        if let Some(rx) = &self.computing {
            match rx.try_recv() {
                Ok((key, r)) => {
                    self.computing = None;
                    let shown = match r {
                        Ok(p) => {
                            let (outs, sample) = self.show_preview(ctx, p);
                            (key, Ok(outs), sample)
                        }
                        Err(e) => (key, Err(e), String::new()),
                    };
                    self.shown = Some(shown);
                }
                Err(TryRecvError::Disconnected) => self.computing = None,
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
            }
        }
        if let Some(rx) = &self.evaluating {
            match rx.try_recv() {
                Ok(r) => {
                    self.evaluating = None;
                    let line = match r {
                        Ok((_, _, Out::Number(x))) => Line::Answer(number(x), None),
                        Ok((w, h, Out::Map(cells))) => {
                            let t = self.image(ctx, &cells, w, h);
                            Line::Answer(format!("map {w} x {h} · {}", numbers(&cells)), t)
                        }
                        Ok((w, h, Out::Stack(layers))) => {
                            let names: Vec<String> = layers.iter().take(12).map(|(k, _)| k.to_string()).collect();
                            let more = if layers.len() > 12 { ", ..." } else { "" };
                            let t = layers.first().and_then(|(_, c)| self.image(ctx, c, w, h));
                            Line::Answer(
                                format!("stack of {} layers {w} x {h}: {}{more}", layers.len(), names.join(", ")),
                                t,
                            )
                        }
                        Err(e) => Line::Error(e),
                    };
                    self.lines.push(line);
                }
                Err(TryRecvError::Disconnected) => self.evaluating = None,
                Err(TryRecvError::Empty) => ctx.request_repaint_after(Duration::from_millis(50)),
            }
        }
    }

    /// A preview of the script asked for, when it changed, after a pause in the typing, when it has no error and
    /// what it reads has values.
    fn ask_preview(&mut self, ctx: &egui::Context, values: &BTreeMap<String, Value>, problem: bool) {
        if problem || self.text.trim().is_empty() || self.computing.is_some() {
            return;
        }
        if let Some(t) = self.typed_at
            && t.elapsed() < PAUSE
        {
            ctx.request_repaint_after(PAUSE - t.elapsed());
            return;
        }
        let reads = algebra_tools::reads(&self.text);
        let wanted: BTreeMap<String, Value> =
            values.iter().filter(|(k, _)| reads.contains(k)).map(|(k, v)| (k.clone(), v.clone())).collect();
        if reads.iter().any(|r| !wanted.contains_key(r) && !algebra_tools::names_given(&self.text).contains(r)) {
            return;
        }
        let key = key_of(&self.text, &wanted);
        if self.asked == Some(key) {
            return;
        }
        self.asked = Some(key);
        let (tx, rx) = channel();
        let (text, ctx) = (self.text.clone(), ctx.clone());
        std::thread::spawn(move || {
            let r = algebra_tools::preview(&text, &wanted, SAMPLE).map_err(|e| e.0);
            let _ = tx.send((key, r));
            ctx.request_repaint();
        });
        self.computing = Some(rx);
    }

    /// A line typed in the console computed (in a thread), or a command done (`help`, `clear`).
    fn evaluate(&mut self, ctx: &egui::Context, line: String, values: &BTreeMap<String, Value>, inputs: &Inputs) {
        if self.typed.last() != Some(&line) {
            self.typed.push(line.clone());
        }
        self.recalled = None;
        match line.as_str() {
            "clear" => {
                self.lines.clear();
                return;
            }
            "help" | "?" => {
                self.lines.push(Line::Asked(line));
                self.lines.push(Line::Note(help()));
                return;
            }
            _ => {}
        }
        self.lines.push(Line::Asked(line.clone()));
        let reads = algebra_tools::reads(&line);
        let missing: Vec<&String> =
            reads.iter().filter(|r| inputs.contains_key(*r) && !values.contains_key(*r)).collect();
        if !missing.is_empty() {
            let names: Vec<&str> = missing.iter().map(|s| s.as_str()).collect();
            self.lines.push(Line::Error(format!(
                "{} {} no value yet: run the model up to this node (ctrl+enter)",
                names.join(", "),
                if names.len() == 1 { "has" } else { "have" }
            )));
            return;
        }
        let (tx, rx) = channel();
        let (script, values, ctx) = (self.text.clone(), values.clone(), ctx.clone());
        std::thread::spawn(move || {
            let r = algebra_tools::evaluate_line(&script, &line, &values).map_err(|e| e.0);
            let _ = tx.send(r);
            ctx.request_repaint();
        });
        self.evaluating = Some(rx);
    }
}

/// The reference of the language, for the console's `help`.
fn help() -> String {
    let mut t = String::from(
        "name = value · name[where] = value · s.layer = value · out name = value · # a comment\n\
         + - * / % ** · < <= > >= == != (5 <= x < 8) · and or not · a if c else b · null True False pi\n",
    );
    for (_, written, what) in FUNCTION_HELP {
        t.push_str(&format!("{written:<26}{what}\n"));
    }
    for (_, written, what) in METHOD_HELP {
        t.push_str(&format!("{written:<26}{what}\n"));
    }
    t.push_str("clear empties the console");
    t
}

/// The calculator's pane.
pub fn pane(ui: &mut Ui, c: &mut Calculator, s: &mut Session) -> Option<Asked> {
    c.poll(ui.ctx());
    let Some((path, n)) = target(c, s) else {
        c.node = None;
        egui::Frame::new().inner_margin(Margin::same(10)).show(ui, |ui| {
            ui.label(mono("terraflux calculator").strong().color(theme::accent()));
            ui.add_space(4.0);
            ui.label(
                mono(
                    "Select a map.calc node to write its script here: maps, stacks and numbers in a language close \
                     to Python's, its inputs' names and layers completed as you type, its outputs shown at once.",
                )
                .weak(),
            );
        });
        return None;
    };
    // another node, or its script changed elsewhere (undo, the node's pane) while not typed in here
    let here = Some((path.clone(), n.id.clone()));
    let script = script_of(&n);
    if c.node != here {
        c.node = here;
        c.text = script.clone();
        c.applied = script;
        c.popup = None;
        c.shown = None;
        c.asked = None;
    } else if script != c.applied && c.text == c.applied {
        c.text = script.clone();
        c.applied = script;
    }
    let (inputs, values) = connected(c, s, &path, &n);
    let problem = algebra_tools::problem(&c.text, &inputs);
    c.ask_preview(ui.ctx(), &values, problem.is_some());
    let mut asked = None;
    let mut apply = false;
    let fill = theme::well();
    egui::Panel::top("calculator bar")
        .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::symmetric(8, 4)))
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label(mono(&n.id).strong());
                for (name, input) in &inputs {
                    let what = match input {
                        Input::Map => "map".to_string(),
                        Input::Number(Some(x)) => number(*x),
                        Input::Number(None) => "number".into(),
                        Input::Stack(Some(l)) => format!("stack · {}", l.len()),
                        Input::Stack(None) => "stack".into(),
                        Input::Unknown => "?".into(),
                    };
                    let tip = match input {
                        Input::Stack(Some(l)) => {
                            let names: Vec<String> = l
                                .iter()
                                .map(|(n, raw)| if n == raw { n.clone() } else { format!("{n} ({raw})") })
                                .collect();
                            format!("its layers: {}", names.join(", "))
                        }
                        Input::Unknown => "not run yet: run the model up to this node (ctrl+enter)".into(),
                        _ => String::new(),
                    };
                    let r = ui.label(mono(format!("{name}: {what}")).color(theme::accent()).small());
                    if !tip.is_empty() {
                        r.on_hover_text(tip);
                    }
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let runnable = path.is_empty() && s.can_run_to();
                    if ui
                        .add_enabled(runnable, egui::Button::new("Apply and run"))
                        .on_hover_text("the script given to the node, and the model run up to it (ctrl+enter)")
                        .clicked()
                    {
                        apply = true;
                        asked = Some(Asked::RunTo(n.id.clone()));
                    }
                    if ui
                        .add_enabled(c.text != c.applied, egui::Button::new("Apply"))
                        .on_hover_text("the script given to the node (also when the editor is left)")
                        .clicked()
                    {
                        apply = true;
                    }
                    if c.text != c.applied {
                        ui.label(mono("●").color(theme::accent())).on_hover_text("not given to the node yet");
                    }
                });
            });
        });
    egui::Panel::bottom("calculator console")
        .resizable(true)
        .default_size(160.0)
        .min_size(70.0)
        .frame(egui::Frame::new().fill(fill).inner_margin(Margin::symmetric(10, 6)))
        .show(ui, |ui| console(ui, c, &values, &inputs));
    egui::Panel::right("calculator preview")
        .resizable(true)
        .default_size(260.0)
        .min_size(140.0)
        .frame(egui::Frame::new().fill(ui.visuals().panel_fill).inner_margin(Margin::same(8)))
        .show(ui, |ui| preview(ui, c, &inputs, &values, problem.is_some()));
    egui::CentralPanel::default().frame(egui::Frame::new().fill(fill).inner_margin(Margin::symmetric(6, 6))).show(
        ui,
        |ui| {
            let (left, run) = editor(ui, c, &inputs, problem.as_ref());
            apply |= left || run;
            if run && path.is_empty() && s.can_run_to() {
                asked = Some(Asked::RunTo(n.id.clone()));
            }
        },
    );
    if apply && c.text != c.applied {
        match crate::editor::set_text_param(&mut s.source, &path, &n.id, "expression", &c.text) {
            Ok(()) => {
                c.applied = c.text.clone();
                s.editor.message = None;
                s.edited();
            }
            Err(e) => s.editor.message = Some(e),
        }
    }
    asked
}

/// The script's editor, its line numbers at its left, the completions under the cursor, the error under it.
/// Whether it was left (its script to be given to the node) and whether ctrl+enter was pressed.
fn editor(ui: &mut Ui, c: &mut Calculator, inputs: &Inputs, problem: Option<&Problem>) -> (bool, bool) {
    let id = editor_id();
    let focused = ui.memory(|m| m.has_focus(id));
    let none = egui::Modifiers::NONE;
    // the keys of the completions, and ctrl+enter, taken before the editor sees them
    let (mut accept, mut run, mut explicit) = (None, false, false);
    if focused {
        ui.input_mut(|i| {
            run = i.consume_key(egui::Modifiers::COMMAND, Key::Enter);
            explicit = i.consume_key(egui::Modifiers::COMMAND, Key::Space);
            if let Some((_, list, chosen)) = &mut c.popup {
                if i.consume_key(none, Key::ArrowDown) {
                    *chosen = (*chosen + 1) % list.len();
                }
                if i.consume_key(none, Key::ArrowUp) {
                    *chosen = (*chosen + list.len() - 1) % list.len();
                }
                if i.consume_key(none, Key::Tab) || i.consume_key(none, Key::Enter) {
                    accept = Some(*chosen);
                }
                if i.consume_key(none, Key::Escape) {
                    c.popup = None;
                }
            }
        });
    }
    if let Some(k) = accept
        && let Some((range, list, _)) = c.popup.take()
    {
        let chosen = &list[k];
        c.text.replace_range(range.clone(), &chosen.text);
        let end = c.text[..range.start + chosen.text.len()].chars().count();
        if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
            state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(end))));
            state.store(ui.ctx(), id);
        }
        c.typed_at = Some(Instant::now());
    }
    let mut left = false;
    egui::ScrollArea::vertical().id_salt("calculator script").auto_shrink(false).show(ui, |ui| {
        let font = egui::TextStyle::Monospace.resolve(ui.style());
        let digits = c.text.lines().count().max(1).to_string().len().max(2);
        let gutter = font.size * 0.62 * (digits as f32 + 1.0);
        ui.horizontal_top(|ui| {
            let (rect, _) = ui.allocate_exact_size(egui::vec2(gutter, 0.0), egui::Sense::hover());
            let mut layouter = |ui: &Ui, text: &dyn egui::TextBuffer, wrap: f32| {
                ui.fonts_mut(|f| f.layout_job(layout(ui, text.as_str(), inputs, problem, wrap)))
            };
            let out = egui::TextEdit::multiline(&mut c.text)
                .id(id)
                .font(egui::TextStyle::Monospace)
                .frame(egui::Frame::NONE)
                .margin(Margin::ZERO)
                .lock_focus(true)
                .desired_rows(8)
                .desired_width(f32::INFINITY)
                .hint_text(mono("out ndvi = (img.nir - img.red) / (img.nir + img.red)").weak())
                .layouter(&mut layouter)
                .show(ui);
            // the numbers of the lines (of the rows that begin one)
            let painter = ui.painter();
            let mut line = 1;
            for (k, row) in out.galley.rows.iter().enumerate() {
                if k == 0 || out.galley.rows[k - 1].ends_with_newline {
                    let at = egui::pos2(rect.right() - font.size * 0.5, out.galley_pos.y + row.pos.y);
                    let wrong = problem.is_some_and(|p| p.line == line);
                    let colour = if wrong { ui.visuals().error_fg_color } else { theme::weak() };
                    painter.text(at, egui::Align2::RIGHT_TOP, line.to_string(), font.clone(), colour);
                    line += 1;
                }
            }
            if out.response.response.changed() {
                c.typed_at = Some(Instant::now());
            }
            left = out.response.response.lost_focus();
            // the completions: as a name is typed, after a dot, or asked for (ctrl+space)
            let cursor = out.cursor_range.map(|r| byte_at(&c.text, r.primary.index.into()));
            if let Some(at) = cursor
                && (out.response.response.changed() || explicit)
            {
                let before = c.text[..at].chars().next_back();
                let typing = before.is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.');
                let (range, list) = algebra_tools::complete(&c.text, at, inputs);
                c.popup = if (typing || explicit)
                    && !list.is_empty()
                    && (explicit || !range.is_empty() || before == Some('.'))
                {
                    Some((range, list, 0))
                } else {
                    None
                };
            }
            if !focused || cursor.is_none() {
                c.popup = c.popup.take().filter(|_| out.response.response.has_focus());
            }
            let mut clicked = None;
            if let (Some(at), Some((range, list, chosen))) = (cursor, &c.popup) {
                // under the cursor's line, at the word's beginning
                let ch = c.text[..at].chars().count();
                let start = c.text[..range.start].chars().count();
                let pos = egui::pos2(
                    out.galley_pos.x + out.galley.pos_from_cursor(CCursor::new(start)).left().max(0.0),
                    out.galley_pos.y + out.galley.pos_from_cursor(CCursor::new(ch)).bottom() + 2.0,
                );
                clicked = completions(ui, pos, list, *chosen);
            }
            if let Some(k) = clicked {
                accept_now(ui, c, k);
            }
        });
    });
    match problem {
        Some(p) => {
            let colour = ui.visuals().error_fg_color;
            ui.label(mono(format!("✖ line {}: {}", p.line, p.message)).color(colour));
        }
        None if !c.text.trim().is_empty() => {
            let outs = terraflux_graph::declared_outputs(&c.text);
            let outs = if outs.is_empty() { "map".to_string() } else { outs.join(", ") };
            ui.label(mono(format!("✔ gives {outs}")).weak())
                .on_hover_text("ctrl+space: the names that fit · ctrl+enter: apply and run up to here");
        }
        None => {}
    }
    (left && c.popup.is_none(), run)
}

/// The completion clicked, put in at once.
fn accept_now(ui: &Ui, c: &mut Calculator, k: usize) {
    let Some((range, list, _)) = c.popup.take() else { return };
    let chosen = &list[k];
    c.text.replace_range(range.clone(), &chosen.text);
    let end = c.text[..range.start + chosen.text.len()].chars().count();
    if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), editor_id()) {
        state.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(end))));
        state.store(ui.ctx(), editor_id());
    }
    ui.memory_mut(|m| m.request_focus(editor_id()));
    c.typed_at = Some(Instant::now());
}

/// The completions under the cursor, the one chosen marked, its help beside: the one clicked.
fn completions(ui: &Ui, at: egui::Pos2, list: &[Completion], chosen: usize) -> Option<usize> {
    let mut clicked = None;
    egui::Area::new(egui::Id::new("calculator completions")).order(egui::Order::Foreground).fixed_pos(at).show(
        ui.ctx(),
        |ui| {
            egui::Frame::popup(ui.style()).inner_margin(Margin::same(4)).show(ui, |ui| {
                ui.set_max_width(460.0);
                // (a window of the list around the one chosen)
                let first = chosen.saturating_sub(9).min(list.len().saturating_sub(10));
                for (k, item) in list.iter().enumerate().skip(first).take(10) {
                    let mut job = LayoutJob::default();
                    let style = ui.style().clone();
                    mono(&item.label).color(colour(ui, item.part)).append_to(
                        &mut job,
                        &style,
                        egui::FontSelection::Default,
                        egui::Align::Center,
                    );
                    mono(format!("  {}", item.detail)).weak().small().append_to(
                        &mut job,
                        &style,
                        egui::FontSelection::Default,
                        egui::Align::Center,
                    );
                    if ui.add(egui::Button::selectable(k == chosen, job).frame_when_inactive(false)).clicked() {
                        clicked = Some(k);
                    }
                }
                if list.len() > 10 {
                    ui.label(mono(format!("{} in all · ↑↓ tab", list.len())).weak().small());
                }
            });
        },
    );
    clicked
}

/// The preview of the outputs, on a sample of the cells.
fn preview(ui: &mut Ui, c: &Calculator, inputs: &Inputs, values: &BTreeMap<String, Value>, problem: bool) {
    egui::ScrollArea::vertical().id_salt("calculator preview").auto_shrink(false).show(ui, |ui| {
        let reads = algebra_tools::reads(&c.text);
        let given = algebra_tools::names_given(&c.text);
        let missing: Vec<&str> = reads
            .iter()
            .filter(|r| inputs.contains_key(*r) && !values.contains_key(*r) && !given.contains(r))
            .map(String::as_str)
            .collect();
        if !missing.is_empty() {
            ui.label(
                RichText::new(format!(
                    "{} not run yet: run the model up to this node to see the outputs here (ctrl+enter)",
                    missing.join(", ")
                ))
                .weak(),
            );
            return;
        }
        if c.computing.is_some() {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("computing...").weak());
            });
        }
        let Some((_, shown, sample)) = &c.shown else {
            if !problem && c.computing.is_none() {
                ui.label(RichText::new("The outputs show here as the script is typed.").weak());
            }
            return;
        };
        if problem {
            ui.label(RichText::new("the script has an error: its outputs as they last were").weak().small());
        }
        match shown {
            Err(e) => {
                ui.label(RichText::new(format!("✖ {e}")).color(ui.visuals().error_fg_color));
            }
            Ok(outs) => {
                for o in outs {
                    ui.horizontal(|ui| {
                        ui.label(mono(&o.name).strong());
                        ui.label(mono(&o.what).weak().small());
                    });
                    ui.label(mono(&o.numbers).small());
                    for (name, t) in &o.images {
                        if !name.is_empty() {
                            ui.label(mono(name).weak().small());
                        }
                        // (as wide as the column, its cells as squares)
                        let size = t.size_vec2() * (ui.available_width() / t.size_vec2().x.max(1.0));
                        ui.image((t.id(), size));
                    }
                    ui.add_space(8.0);
                }
                ui.label(RichText::new(sample).weak().small()).on_hover_text(
                    "the outputs computed on a sample of the cells (the neighbourhoods' radii and the numbers of \
                     whole maps the sample's); the node computes every cell",
                );
            }
        }
    });
}

/// The console: what was computed, and a line to compute in the scope of the script.
fn console(ui: &mut Ui, c: &mut Calculator, values: &BTreeMap<String, Value>, inputs: &Inputs) {
    let id = egui::Id::new("calculator console input");
    let typing = ui.memory(|m| m.has_focus(id));
    let none = egui::Modifiers::NONE;
    if typing {
        let up = ui.input_mut(|i| i.consume_key(none, Key::ArrowUp));
        let down = !up && ui.input_mut(|i| i.consume_key(none, Key::ArrowDown));
        if up || down {
            let n = c.typed.len();
            let recalled = match (c.recalled, up) {
                (None, true) if n > 0 => Some(Some(n - 1)),
                (Some(k), true) => Some(Some(k.saturating_sub(1))),
                (Some(k), false) if k + 1 < n => Some(Some(k + 1)),
                (Some(_), false) => Some(None),
                _ => None,
            };
            if let Some(r) = recalled {
                c.recalled = r;
                c.input = r.map_or(String::new(), |k| c.typed[k].clone());
                if let Some(mut state) = egui::TextEdit::load_state(ui.ctx(), id) {
                    let end = CCursor::new(c.input.chars().count());
                    state.cursor.set_char_range(Some(CCursorRange::one(end)));
                    state.store(ui.ctx(), id);
                }
            }
        }
    }
    let enter = typing && ui.input_mut(|i| i.consume_key(none, Key::Enter));
    egui::Panel::bottom("calculator prompt").frame(egui::Frame::new().inner_margin(Margin::symmetric(0, 4))).show(
        ui,
        |ui| {
            ui.horizontal(|ui| {
                ui.label(mono("❯").color(theme::accent()));
                ui.add(
                    egui::TextEdit::singleline(&mut c.input)
                        .id(id)
                        .frame(egui::Frame::NONE)
                        .margin(Margin::ZERO)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .hint_text(mono("img.mean() · ndvi > 0.5 · help").weak()),
                );
            });
        },
    );
    if enter && !c.input.trim().is_empty() && c.evaluating.is_none() {
        let line = std::mem::take(&mut c.input).trim().to_string();
        c.evaluate(ui.ctx(), line, values, inputs);
        ui.memory_mut(|m| m.request_focus(id));
    }
    egui::ScrollArea::vertical().id_salt("calculator console").stick_to_bottom(true).auto_shrink(false).show(
        ui,
        |ui| {
            if c.lines.is_empty() {
                ui.label(
                    mono("Compute anything with the script's names: img.mean(), ndvi.percentile(90), land == 3 · help")
                        .weak(),
                );
            }
            for line in &c.lines {
                match line {
                    Line::Asked(t) => {
                        ui.label(mono(format!("❯ {t}")).color(ui.visuals().strong_text_color()));
                    }
                    Line::Answer(t, image) => {
                        ui.horizontal_top(|ui| {
                            if let Some(img) = image {
                                let size = img.size_vec2() * (48.0 / img.size_vec2().y.max(1.0));
                                ui.image((img.id(), size));
                            }
                            ui.label(mono(format!("= {t}")).color(theme::accent()));
                        });
                    }
                    Line::Error(e) => {
                        ui.label(mono(format!("✖ {e}")).color(ui.visuals().error_fg_color));
                    }
                    Line::Note(t) => {
                        ui.label(mono(t).weak().small());
                    }
                }
            }
            if c.evaluating.is_some() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(mono("computing...").weak());
                });
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use terraflux_ops::Map;

    use super::*;

    #[test]
    fn numbers_as_written() {
        assert_eq!(number(3.0), "3");
        assert_eq!(number(0.123456789), "0.123457");
        assert_eq!(number(2.5), "2.5");
        assert_eq!(number(f64::NAN), "null");
        assert_eq!(numbers(&[1.0, f64::NAN, 2.0]), "min 1 · mean 1.5 · max 2 · 1 null");
        assert_eq!(byte_at("aé b", 2), 3);
        assert!(help().contains("percentile(s, p)") && help().contains("x.mean()"));
    }

    #[test]
    fn a_map_is_kept_by_its_data() {
        let m = Map::new(Arc::new(terraflux_raster::Grid::new(1, 1)), vec![1.0]);
        let v: BTreeMap<String, Value> = [("a".to_string(), Value::Map(m.clone()))].into();
        let w: BTreeMap<String, Value> = [("a".to_string(), Value::Map(m))].into();
        assert_eq!(key_of("a + 1", &v), key_of("a + 1", &w));
        assert_ne!(key_of("a + 1", &v), key_of("a + 2", &v));
    }
}
