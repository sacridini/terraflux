//! Find and replace in the whole model (ctrl+H): a text looked for in the settings of every node (inside groups
//! and loops too), the files they name, the scripts of the calculators and the rules of a reclassification, the
//! notes, and the names and files of the model's inputs and outputs. [`find`] lists every occurrence with what it
//! would become (the preview); [`replace`] applies the ones chosen to a copy of the model, all or nothing: the
//! copy is checked as an edit is, and when it would be invalid (a number turned into a text, an input that is not
//! there) nothing changes and the reason is told. The window ([`window`]) only calls them; one step of undo.

use std::collections::{BTreeSet, HashMap};

use egui::{RichText, Ui};
use regex::{Regex, RegexBuilder};
use serde_json::{Number, Value};
use terraflux_graph::{Input, Model, Node, Severity, Source};
use terraflux_ops::Registry;

use crate::app::Session;

/// What an occurrence is part of (what the window's filters choose among).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    /// A setting's value, a literal input, a value or a no-data value of an input.
    Setting,
    /// A file or a folder.
    Path,
    /// A calculator's script, a reclassification's rules.
    Script,
    /// A note, a reason, a reference, an assumption, an alternative.
    Note,
    /// The name of an input or an output of the model.
    Name,
}

/// What is looked for, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub text: String,
    pub with: String,
    /// Tell `A` from `a`.
    pub case: bool,
    /// Only where it is not part of a longer word.
    pub word: bool,
    /// `text` is a regular expression, `with` may use its groups (`$1`).
    pub regex: bool,
    pub settings: bool,
    pub paths: bool,
    pub scripts: bool,
    pub notes: bool,
    pub names: bool,
    /// Only the nodes of an operator whose name has this (none: every node, and the model's inputs and outputs).
    pub op: String,
}

impl Default for Query {
    fn default() -> Self {
        Query {
            text: String::new(),
            with: String::new(),
            case: false,
            word: false,
            regex: false,
            settings: true,
            paths: true,
            scripts: true,
            notes: true,
            names: true,
            op: String::new(),
        }
    }
}

impl Query {
    fn wants(&self, k: Kind) -> bool {
        match k {
            Kind::Setting => self.settings,
            Kind::Path => self.paths,
            Kind::Script => self.scripts,
            Kind::Note => self.notes,
            Kind::Name => self.names,
        }
    }
}

/// A step into a structured value (a table of rules, a list of files).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    Key(String),
    At(usize),
}

/// Which note of a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Slot {
    Note,
    Why,
    Ref(usize),
    Assumption(usize),
    Alternative(usize),
}

/// A place in the model that holds a text or a number.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Field {
    /// A setting of a node (and where in it, for a structured one).
    Param(String, Vec<Step>),
    /// A literal given to an input port of a node.
    Literal(String, Vec<Step>),
    Text(Slot),
    InputPath(String),
    InputNote(String),
    InputNodata(String),
    InputValue(String, Vec<Step>),
    InputName(String),
    OutputName(String),
    OutputPath(String),
}

/// One occurrence of what was looked for.
#[derive(Debug, Clone, PartialEq)]
pub struct Hit {
    /// The containers' ids from the top, then the node's (empty: the model's own inputs and outputs).
    pub at: Vec<String>,
    pub op: String,
    pub field: Field,
    pub kind: Kind,
    /// What the field is called, for the list (`path`, `rules`, `note`, `$land path`).
    pub setting: String,
    /// The whole text of the field, as it was found.
    pub text: String,
    /// Where in `text` the occurrence is (bytes).
    pub start: usize,
    pub end: usize,
    /// A little of the text before and after it (one line).
    pub pre: String,
    pub post: String,
    /// What it becomes.
    pub with: String,
}

impl Hit {
    /// Where the node is, `group > inner` (the model for its inputs and outputs).
    pub fn place(&self) -> String {
        if self.at.is_empty() { "the model".to_string() } else { self.at.join(" > ") }
    }

    /// The text matched.
    pub fn matched(&self) -> &str {
        &self.text[self.start..self.end]
    }

    /// What names the hit among the others (a checkbox's memory).
    pub fn key(&self) -> String {
        format!("{:?}|{:?}|{}", self.at, self.field, self.start)
    }
}

/// A field of the model: the node (its path) and the field of it.
type Place = (Vec<String>, Field);

/// A text of the model, before it is looked in.
struct Item {
    at: Vec<String>,
    op: String,
    field: Field,
    kind: Kind,
    setting: String,
    text: String,
}

/// A number as the user reads it: `-9999`, not `-9999.0`.
fn num_text(n: &Number) -> String {
    match n.as_f64() {
        Some(f) if n.is_f64() && f.fract() == 0.0 && f.abs() < 1e15 => format!("{}", f as i64),
        _ => n.to_string(),
    }
}

/// The texts and numbers of a value, with the way to each.
fn leaves(v: &Value, steps: &mut Vec<Step>, out: &mut Vec<(Vec<Step>, String)>) {
    match v {
        Value::String(s) => out.push((steps.clone(), s.clone())),
        Value::Number(n) => out.push((steps.clone(), num_text(n))),
        Value::Array(a) => {
            for (k, x) in a.iter().enumerate() {
                steps.push(Step::At(k));
                leaves(x, steps, out);
                steps.pop();
            }
        }
        Value::Object(o) => {
            for (k, x) in o {
                steps.push(Step::Key(k.clone()));
                leaves(x, steps, out);
                steps.pop();
            }
        }
        _ => {}
    }
}

fn leaf_mut<'a>(v: &'a mut Value, steps: &[Step]) -> Option<&'a mut Value> {
    let mut v = v;
    for s in steps {
        v = match s {
            Step::Key(k) => v.get_mut(k.as_str())?,
            Step::At(i) => v.get_mut(*i)?,
        };
    }
    Some(v)
}

fn steps_text(steps: &[Step]) -> String {
    steps
        .iter()
        .map(|s| match s {
            Step::Key(k) => format!(".{k}"),
            Step::At(i) => format!("[{i}]"),
        })
        .collect()
}

/// Whether a setting holds a script (map algebra) or the rules of a reclassification.
fn is_script(op: &str, name: &str) -> bool {
    (op == "raster.reclassify" && name == "rules")
        || terraflux_graph::old_algebra::SETTINGS.iter().any(|(o, s, _)| *o == op && *s == name)
}

/// Whether a setting of a node names a file: those the model's format knows, and the ones called path, file,
/// folder or dir.
fn is_path(name: &str, value: &Value, files: &[String]) -> bool {
    let n = name.to_lowercase();
    ["path", "file", "folder", "dir"].iter().any(|w| n.contains(w))
        || value.as_str().is_some_and(|s| files.iter().any(|f| f == s))
}

/// Every text and number the model holds that the query looks in.
fn items(m: &Model, q: &Query) -> Vec<Item> {
    let op = q.op.trim().to_lowercase();
    let mut out = Vec::new();
    for (at, n) in crate::find::walk(m) {
        if !op.is_empty() && !n.op.to_lowercase().contains(&op) {
            continue;
        }
        node_items(&at, n, q, &mut out);
    }
    if op.is_empty() {
        model_items(m, q, &mut out);
    }
    out
}

fn node_items(at: &[String], n: &Node, q: &Query, out: &mut Vec<Item>) {
    let mut push = |field: Field, kind: Kind, setting: String, text: String| {
        if q.wants(kind) {
            out.push(Item { at: at.to_vec(), op: n.op.clone(), field, kind, setting, text });
        }
    };
    let files: Vec<String> = terraflux_graph::file_settings(&mut n.clone()).map(|p| p.clone()).collect();
    for (name, v) in &n.params {
        let mut found = Vec::new();
        leaves(v, &mut Vec::new(), &mut found);
        let kind = if is_script(&n.op, name) {
            Kind::Script
        } else if is_path(name, v, &files) {
            Kind::Path
        } else {
            Kind::Setting
        };
        for (steps, text) in found {
            push(Field::Param(name.clone(), steps.clone()), kind, format!("{name}{}", steps_text(&steps)), text);
        }
    }
    for (port, s) in &n.inputs {
        if let Source::Literal(v) = s {
            let mut found = Vec::new();
            leaves(v, &mut Vec::new(), &mut found);
            for (steps, text) in found {
                push(
                    Field::Literal(port.clone(), steps.clone()),
                    Kind::Setting,
                    format!("{port}{}", steps_text(&steps)),
                    text,
                );
            }
        }
    }
    if let Some(t) = &n.note {
        push(Field::Text(Slot::Note), Kind::Note, "note".into(), t.clone());
    }
    if let Some(t) = &n.why {
        push(Field::Text(Slot::Why), Kind::Note, "why".into(), t.clone());
    }
    for (k, t) in n.refs.iter().enumerate() {
        push(Field::Text(Slot::Ref(k)), Kind::Note, format!("reference {}", k + 1), t.clone());
    }
    for (k, t) in n.assumptions.iter().enumerate() {
        push(Field::Text(Slot::Assumption(k)), Kind::Note, format!("assumption {}", k + 1), t.clone());
    }
    for (k, t) in n.alternatives.iter().enumerate() {
        push(Field::Text(Slot::Alternative(k)), Kind::Note, format!("alternative {}", k + 1), t.clone());
    }
}

fn model_items(m: &Model, q: &Query, out: &mut Vec<Item>) {
    let mut push = |field: Field, kind: Kind, setting: String, text: String| {
        if q.wants(kind) {
            out.push(Item { at: Vec::new(), op: String::new(), field, kind, setting, text });
        }
    };
    for (name, i) in &m.inputs {
        push(Field::InputName(name.clone()), Kind::Name, format!("${name} name"), name.clone());
        match i {
            Input::Map { path, nodata, .. } | Input::Stack { path, nodata, .. } => {
                push(Field::InputPath(name.clone()), Kind::Path, format!("${name} path"), path.clone());
                if let Some(x) = nodata.and_then(Number::from_f64) {
                    push(Field::InputNodata(name.clone()), Kind::Setting, format!("${name} nodata"), num_text(&x));
                }
            }
            Input::Value { value, .. } => {
                let mut found = Vec::new();
                leaves(value, &mut Vec::new(), &mut found);
                for (steps, text) in found {
                    let label = format!("${name}{}", steps_text(&steps));
                    push(Field::InputValue(name.clone(), steps), Kind::Setting, label, text);
                }
            }
        }
        if let Some(t) = i.note() {
            push(Field::InputNote(name.clone()), Kind::Note, format!("${name} note"), t.to_string());
        }
    }
    for (name, o) in &m.outputs {
        push(Field::OutputName(name.clone()), Kind::Name, format!("output {name} name"), name.clone());
        if let Some(p) = &o.path {
            push(Field::OutputPath(name.clone()), Kind::Path, format!("output {name} path"), p.clone());
        }
    }
}

/// The query's pattern, or what is wrong with it.
fn pattern(q: &Query) -> Result<Regex, String> {
    let p = if q.regex { q.text.clone() } else { regex::escape(&q.text) };
    RegexBuilder::new(&p).case_insensitive(!q.case).build().map_err(|e| format!("not a valid expression: {e}"))
}

/// What is wrong with the query's regular expression (None: nothing, or nothing typed).
pub fn bad_pattern(q: &Query) -> Option<String> {
    if q.text.is_empty() { None } else { pattern(q).err() }
}

fn is_word(c: Option<char>) -> bool {
    c.is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// A few characters of a text, on one line, from its end (`before`) or from its start.
fn snippet(s: &str, before: bool) -> String {
    const N: usize = 28;
    let one = |s: &str| s.chars().map(|c| if c == '\n' || c == '\r' { '↵' } else { c }).collect::<String>();
    let n = s.chars().count();
    if n <= N {
        return one(s);
    }
    if before {
        format!("…{}", one(&s.chars().skip(n - N).collect::<String>()))
    } else {
        format!("{}…", one(&s.chars().take(N).collect::<String>()))
    }
}

/// Every occurrence of the query in the model, in the model's order (a container before what it holds, a node's
/// settings by name, then its notes; the model's inputs and outputs last), each with what it would become. None
/// when nothing is typed, or the expression is not valid ([`bad_pattern`]).
pub fn find(m: &Model, q: &Query) -> Vec<Hit> {
    if q.text.is_empty() {
        return Vec::new();
    }
    let Ok(re) = pattern(q) else { return Vec::new() };
    let mut out = Vec::new();
    for it in items(m, q) {
        for caps in re.captures_iter(&it.text) {
            let Some(w) = caps.get(0) else { continue };
            if w.is_empty() {
                continue;
            }
            let (before, after) = (&it.text[..w.start()], &it.text[w.end()..]);
            if q.word && (is_word(before.chars().next_back()) || is_word(after.chars().next())) {
                continue;
            }
            let mut with = String::new();
            if q.regex {
                caps.expand(&q.with, &mut with);
            } else {
                with.push_str(&q.with);
            }
            out.push(Hit {
                at: it.at.clone(),
                op: it.op.clone(),
                field: it.field.clone(),
                kind: it.kind,
                setting: it.setting.clone(),
                text: it.text.clone(),
                start: w.start(),
                end: w.end(),
                pre: snippet(before, true),
                post: snippet(after, false),
                with,
            });
        }
    }
    out
}

/// Put a text where a value is: a text as it is, a number read from it and written back as a number (a float
/// stays one), refused when it is not one.
fn put(v: &mut Value, new: &str) -> Result<(), String> {
    match v {
        Value::String(s) => {
            *s = new.to_string();
            Ok(())
        }
        Value::Number(n) => {
            let not = || format!("{new:?} is not a number, and the value ({}) is one", num_text(n));
            let Value::Number(p) = serde_json::from_str::<Value>(new.trim()).map_err(|_| not())? else {
                return Err(not());
            };
            *n = if n.is_f64() { p.as_f64().and_then(Number::from_f64).ok_or_else(not)? } else { p };
            Ok(())
        }
        _ => Err("not a text nor a number".into()),
    }
}

fn node_at_mut<'a>(m: &'a mut Model, at: &[String]) -> Result<&'a mut Node, String> {
    let (id, path) = at.split_last().ok_or("no node")?;
    m.scope_mut(path)
        .and_then(|v| v.iter_mut().find(|n| n.id == *id))
        .ok_or_else(|| format!("no node {}", at.join("/")))
}

/// Write the new text of a field (the names of inputs and outputs are not: [`replace`] renames them after).
fn write(m: &mut Model, at: &[String], f: &Field, new: &str) -> Result<(), String> {
    let gone = || "it is no longer there".to_string();
    match f {
        Field::Param(name, steps) => {
            let n = node_at_mut(m, at)?;
            put(leaf_mut(n.params.get_mut(name).ok_or_else(gone)?, steps).ok_or_else(gone)?, new)
        }
        Field::Literal(port, steps) => {
            let n = node_at_mut(m, at)?;
            match n.inputs.get_mut(port) {
                Some(Source::Literal(v)) => put(leaf_mut(v, steps).ok_or_else(gone)?, new),
                _ => Err(gone()),
            }
        }
        Field::Text(slot) => {
            let n = node_at_mut(m, at)?;
            let some = (!new.trim().is_empty()).then(|| new.to_string());
            match *slot {
                Slot::Note => n.note = some,
                Slot::Why => n.why = some,
                Slot::Ref(k) => *n.refs.get_mut(k).ok_or_else(gone)? = new.to_string(),
                Slot::Assumption(k) => *n.assumptions.get_mut(k).ok_or_else(gone)? = new.to_string(),
                Slot::Alternative(k) => *n.alternatives.get_mut(k).ok_or_else(gone)? = new.to_string(),
            }
            Ok(())
        }
        Field::InputPath(name) => match m.inputs.get_mut(name) {
            Some(Input::Map { path, .. } | Input::Stack { path, .. }) => {
                *path = new.to_string();
                Ok(())
            }
            _ => Err(gone()),
        },
        Field::InputNote(name) => {
            let i = m.inputs.get_mut(name).ok_or_else(gone)?;
            i.set_note((!new.trim().is_empty()).then(|| new.to_string()));
            Ok(())
        }
        Field::InputNodata(name) => match m.inputs.get_mut(name) {
            Some(Input::Map { nodata, .. } | Input::Stack { nodata, .. }) => {
                let x: f64 =
                    new.trim().parse().map_err(|_| format!("{new:?} is not a number, and the nodata is one"))?;
                *nodata = Some(x);
                Ok(())
            }
            _ => Err(gone()),
        },
        Field::InputValue(name, steps) => match m.inputs.get_mut(name) {
            Some(Input::Value { value, .. }) => put(leaf_mut(value, steps).ok_or_else(gone)?, new),
            _ => Err(gone()),
        },
        Field::OutputPath(name) => {
            m.outputs.get_mut(name).ok_or_else(gone)?.path = Some(new.to_string());
            Ok(())
        }
        Field::InputName(_) | Field::OutputName(_) => Ok(()),
    }
}

fn errors(m: &Model, r: &Registry) -> Vec<String> {
    terraflux_graph::check(m, r).into_iter().filter(|i| i.severity == Severity::Error).map(|i| i.to_string()).collect()
}

/// The model with the hits replaced by what they became (`Hit::with`), all of them or none: the copy is checked as
/// an edit is, and when it has an error the model did not have, the reason is the error (and where the hit was
/// when it is the hit that is refused: a number made a text, a name that is not valid). The model given is not
/// changed.
pub fn replace(m: &Model, r: &Registry, hits: &[Hit]) -> Result<Model, String> {
    let mut c = m.clone();
    let mut now: HashMap<(Vec<String>, Field), String> = HashMap::new();
    for it in items(m, &Query::default()) {
        now.insert((it.at, it.field), it.text);
    }
    // (the hits of a field together, the last of its text first: the places of the others hold)
    let mut groups: Vec<(Place, Vec<&Hit>)> = Vec::new();
    for h in hits {
        match groups.iter_mut().find(|(k, _)| k.0 == h.at && k.1 == h.field) {
            Some((_, v)) => v.push(h),
            None => groups.push(((h.at.clone(), h.field.clone()), vec![h])),
        }
    }
    let mut inputs: Vec<(String, String)> = Vec::new();
    let mut outputs: Vec<(String, String)> = Vec::new();
    for ((at, field), mut group) in groups {
        let first = group[0];
        let who = format!("{} ({})", first.place(), first.setting);
        let text = now.get(&(at.clone(), field.clone())).ok_or_else(|| format!("{who}: it is no longer there"))?;
        if *text != first.text {
            return Err(format!("{who}: it changed since the search; search again"));
        }
        group.sort_by_key(|h| std::cmp::Reverse(h.start));
        let mut new = text.clone();
        let mut last = usize::MAX;
        for h in group {
            if h.end > last || h.end > new.len() || !new.is_char_boundary(h.start) || !new.is_char_boundary(h.end) {
                return Err(format!("{who}: the occurrences overlap"));
            }
            new.replace_range(h.start..h.end, &h.with);
            last = h.start;
        }
        match &field {
            Field::InputName(old) => inputs.push((old.clone(), new)),
            Field::OutputName(old) => outputs.push((old.clone(), new)),
            f => write(&mut c, &at, f, &new).map_err(|e| format!("{who}: {e}"))?,
        }
    }
    for (old, new) in inputs {
        crate::editor::rename_input(&mut c, &old, &new).map_err(|e| format!("input ${old}: {e}"))?;
    }
    for (old, new) in outputs {
        if old == new {
            continue;
        }
        if !terraflux_graph::valid_id(&new) {
            return Err(format!(
                "output {old}: {new:?} is not a valid name (letters, digits and _, not first a digit)"
            ));
        }
        if c.outputs.contains_key(&new) {
            return Err(format!("output {old}: the model has an output {new:?} already"));
        }
        let o = c.outputs.remove(&old).ok_or_else(|| format!("output {old}: it is no longer there"))?;
        c.outputs.insert(new, o);
    }
    let before = errors(m, r);
    if let Some(e) = errors(&c, r).into_iter().find(|e| !before.contains(e)) {
        return Err(format!("refused, nothing changed: {e}"));
    }
    Ok(c)
}

/// The window's state.
#[derive(Debug, Default)]
pub struct Dialog {
    pub open: bool,
    pub query: Query,
    hits: Vec<Hit>,
    /// What the hits were worked out for: the model's revision, the query.
    of: Option<(u64, Query)>,
    /// The hits unchecked ([`Hit::key`]).
    off: BTreeSet<String>,
    /// What came of the last Replace, or why it was refused.
    note: Option<(bool, String)>,
    focus: bool,
}

/// The files of the replaced paths that are not there, one per line (empty when all are).
fn not_found(s: &Session, hits: &[Hit]) -> Vec<String> {
    let new: BTreeSet<String> = hits.iter().filter(|h| h.kind == Kind::Path).map(after).collect();
    crate::relink::missing(&s.source, &s.base()).into_iter().map(|(p, _)| p).filter(|p| new.contains(p)).collect()
}

/// The field's text after the hit alone.
fn after(h: &Hit) -> String {
    format!("{}{}{}", &h.text[..h.start], h.with, &h.text[h.end..])
}

/// The window of find and replace.
pub fn window(ctx: &egui::Context, s: &mut Session, r: &Registry) {
    if !s.replace.open {
        return;
    }
    let mut d = std::mem::take(&mut s.replace);
    d.open = true;
    let mut open = true;
    if d.of.as_ref().is_none_or(|(rev, q)| *rev != s.revision || *q != d.query) {
        d.hits = find(&s.source, &d.query);
        d.of = Some((s.revision, d.query.clone()));
        let keys: BTreeSet<String> = d.hits.iter().map(Hit::key).collect();
        d.off.retain(|k| keys.contains(k));
    }
    let mut go: Option<Vec<String>> = None;
    let mut apply = false;
    egui::Window::new("Find and replace").open(&mut open).default_width(640.0).default_height(520.0).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label("Find");
            let e = ui.add(egui::TextEdit::singleline(&mut d.query.text).desired_width(f32::INFINITY));
            if !d.focus {
                e.request_focus();
                d.focus = true;
            }
        });
        ui.horizontal(|ui| {
            ui.label("Replace with");
            ui.add(egui::TextEdit::singleline(&mut d.query.with).desired_width(f32::INFINITY));
        });
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut d.query.case, "Match case");
            ui.checkbox(&mut d.query.word, "Whole word");
            ui.checkbox(&mut d.query.regex, "Regular expression")
                .on_hover_text("`$1` in the replacement is the first group");
            ui.separator();
            ui.label(RichText::new("Look in").weak());
            ui.checkbox(&mut d.query.settings, "Settings");
            ui.checkbox(&mut d.query.paths, "Paths");
            ui.checkbox(&mut d.query.scripts, "Scripts and rules");
            ui.checkbox(&mut d.query.notes, "Notes");
            ui.checkbox(&mut d.query.names, "Names");
        });
        ui.horizontal(|ui| {
            ui.label("Operator");
            ui.add(
                egui::TextEdit::singleline(&mut d.query.op)
                    .hint_text("only the nodes of an operator (raster.calc)")
                    .desired_width(260.0),
            );
        });
        if let Some(e) = bad_pattern(&d.query) {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
        ui.separator();
        let chosen = d.hits.iter().filter(|h| !d.off.contains(&h.key())).count();
        ui.horizontal(|ui| {
            let n = d.hits.len();
            ui.label(RichText::new(format!("{n} occurrence{}, {chosen} chosen", if n == 1 { "" } else { "s" })).weak());
            if ui.small_button("All").clicked() {
                d.off.clear();
            }
            if ui.small_button("None").clicked() {
                d.off = d.hits.iter().map(Hit::key).collect();
            }
        });
        let height = 22.0;
        let hits = d.hits.clone();
        egui::ScrollArea::vertical().max_height(300.0).auto_shrink([false, true]).show_rows(
            ui,
            height,
            hits.len(),
            |ui, rows| {
                for k in rows {
                    let h = &hits[k];
                    ui.horizontal(|ui| {
                        let mut on = !d.off.contains(&h.key());
                        if ui.checkbox(&mut on, "").changed() {
                            if on {
                                d.off.remove(&h.key());
                            } else {
                                d.off.insert(h.key());
                            }
                        }
                        let job = row(ui, h);
                        let resp = ui.add(egui::Button::selectable(false, job).frame(false));
                        if resp.clicked() && !h.at.is_empty() {
                            go = Some(h.at.clone());
                        }
                    });
                }
            },
        );
        ui.add_space(4.0);
        if let Some((ok, t)) = &d.note {
            let colour = if *ok { ui.visuals().text_color() } else { ui.visuals().error_fg_color };
            ui.add(egui::Label::new(RichText::new(t).color(colour)).wrap());
        }
        ui.horizontal(|ui| {
            apply = ui
                .add_enabled(chosen > 0, crate::app::accent_button(&format!("Replace selected ({chosen})"), chosen > 0))
                .on_hover_text("all of them in one step of undo, or none when the model would not be valid")
                .clicked();
        });
    });
    if apply {
        let chosen: Vec<Hit> = d.hits.iter().filter(|h| !d.off.contains(&h.key())).cloned().collect();
        match replace(&s.source, r, &chosen) {
            Ok(m) => {
                s.source = m;
                s.history.name(&format!(
                    "{} replaced ({} -> {})",
                    chosen.len(),
                    d.query.text.chars().take(24).collect::<String>(),
                    d.query.with.chars().take(24).collect::<String>()
                ));
                s.edited();
                s.read_data();
                let mut t = format!("{} replaced.", chosen.len());
                let lost = not_found(s, &chosen);
                if !lost.is_empty() {
                    t.push_str(&format!(" Not found now ({}):", lost.len()));
                    for p in lost.iter().take(6) {
                        t.push_str(&format!("\n  {p}"));
                    }
                    if lost.len() > 6 {
                        t.push_str("\n  ...");
                    }
                }
                d.note = Some((true, t));
                d.of = None;
            }
            Err(e) => d.note = Some((false, e)),
        }
    }
    if let Some(at) = go {
        crate::find::go(s, r, &at);
    }
    d.open = open;
    s.replace = d;
}

/// A row of the list: where it is, then the text before, the match struck out, what it becomes.
fn row(ui: &Ui, h: &Hit) -> egui::text::LayoutJob {
    let weak = ui.visuals().weak_text_color();
    let text = ui.visuals().text_color();
    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    let small = egui::FontId::proportional(11.5);
    let mut job = egui::text::LayoutJob::default();
    job.append(&format!("{} · {}   ", h.place(), h.setting), 0.0, egui::TextFormat::simple(small, weak));
    job.append(&h.pre, 0.0, egui::TextFormat::simple(mono.clone(), weak));
    let mut old = egui::TextFormat::simple(mono.clone(), text);
    old.background = ui.visuals().error_fg_color.gamma_multiply(0.25);
    old.strikethrough = egui::Stroke::new(1.0, ui.visuals().error_fg_color);
    job.append(&h.text[h.start..h.end].replace('\n', "↵"), 0.0, old);
    let mut new = egui::TextFormat::simple(mono.clone(), text);
    new.background = crate::review::ADDED.gamma_multiply(0.3);
    job.append(if h.with.is_empty() { "(nothing)" } else { &h.with }, 0.0, new);
    job.append(&h.post, 0.0, egui::TextFormat::simple(mono, weak));
    job
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Body;

    fn model() -> Model {
        let mut m = Model::new("m");
        m.inputs.insert(
            "land".into(),
            Input::Map { path: "D:/dados/land.tif".into(), band: None, nodata: Some(-9999.0), note: None, data: None },
        );
        let read = Node::new("dem", "raster.read").param("path", "D:/dados/dem.tif").param("nodata", -9999);
        let calc = Node::new("slope", "raster.calc").param("expression", "out s = a * 2");
        let rules = Node::new("rc", "raster.reclassify").param("rules", "1 2 3 -> 1\n* -> nan");
        let group = Node::new("prep", terraflux_graph::GROUP)
            .body(Body { nodes: vec![calc, rules], ..Default::default() })
            .note("terrain from D:/dados/");
        let each = Node::new("each", "control.each").body(Body {
            nodes: vec![Node::new("in_each", "raster.read").param("path", "D:/dados/each.tif")],
            ..Default::default()
        });
        m.nodes.extend([read, group, each]);
        m
    }

    fn q(text: &str, with: &str) -> Query {
        Query { text: text.into(), with: with.into(), ..Default::default() }
    }

    #[test]
    fn a_prefix_of_paths_is_found_in_every_scope_and_replaced() {
        let m = model();
        let r = Registry::standard();
        let hits = find(&m, &q("D:/dados/", "E:/projeto/dados/"));
        let at: Vec<String> = hits.iter().map(Hit::place).collect();
        // the node, the container of a loop, the model's input, the note of the group
        assert_eq!(at, ["dem", "prep", "each > in_each", "the model"]);
        assert_eq!(hits[0].kind, Kind::Path);
        assert_eq!(hits[1].kind, Kind::Note);
        let out = replace(&m, &r, &hits).unwrap();
        assert_eq!(out.nodes[0].params["path"], "E:/projeto/dados/dem.tif");
        assert_eq!(out.nodes[1].note.as_deref(), Some("terrain from E:/projeto/dados/"));
        assert_eq!(out.nodes[2].body.as_ref().unwrap().nodes[0].params["path"], "E:/projeto/dados/each.tif");
        assert!(matches!(&out.inputs["land"], Input::Map { path, .. } if path == "E:/projeto/dados/land.tif"));
        // the model given is as it was
        assert_eq!(m, model());
    }

    #[test]
    fn a_number_is_compared_as_text_and_written_back_as_a_number() {
        let m = model();
        let r = Registry::standard();
        let hits = find(&m, &q("-9999", "0"));
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].text, "-9999");
        let out = replace(&m, &r, &hits).unwrap();
        assert_eq!(out.nodes[0].params["nodata"], serde_json::json!(0));
        assert!(out.nodes[0].params["nodata"].is_number());
        // a float of an input stays one
        assert!(matches!(&out.inputs["land"], Input::Map { nodata: Some(x), .. } if *x == 0.0));
        // a number made a text is refused, with its place, and nothing changes
        let bad = find(&m, &q("-9999", "none"));
        let e = replace(&m, &r, &bad).unwrap_err();
        assert!(e.contains("is not a number"), "{e}");
        assert_eq!(m, model());
    }

    #[test]
    fn scripts_rules_and_notes_are_searched_by_kind_and_operator() {
        let m = model();
        let hits = find(&m, &q("2", "3"));
        let kinds: Vec<(String, Kind)> = hits.iter().map(|h| (h.setting.clone(), h.kind)).collect();
        assert!(kinds.contains(&("expression".into(), Kind::Script)));
        assert!(kinds.contains(&("rules".into(), Kind::Script)));
        // only the scripts
        let only = Query { settings: false, paths: false, notes: false, names: false, ..q("2", "3") };
        assert!(find(&m, &only).iter().all(|h| h.kind == Kind::Script));
        // only an operator's nodes
        let calc = Query { op: "raster.calc".into(), ..q("2", "3") };
        let h = find(&m, &calc);
        assert_eq!(h.iter().map(Hit::place).collect::<Vec<_>>(), ["prep > slope"]);
        // a rule changed
        let r = Registry::standard();
        let rules = find(&m, &q("-> 1", "-> 9"));
        let out = replace(&m, &r, &rules).unwrap();
        assert_eq!(out.nodes[1].body.as_ref().unwrap().nodes[1].params["rules"], "1 2 3 -> 9\n* -> nan");
    }

    #[test]
    fn case_and_whole_word_and_expressions() {
        let mut m = Model::new("m");
        m.nodes.push(Node::new("a", "raster.reclassify").note("Cat + concat + cat"));
        let n = |q: Query| find(&m, &q).len();
        assert_eq!(n(q("cat", "dog")), 3);
        assert_eq!(n(Query { case: true, ..q("cat", "dog") }), 2);
        assert_eq!(n(Query { word: true, ..q("cat", "dog") }), 2);
        assert_eq!(n(Query { word: true, case: true, ..q("cat", "dog") }), 1);
        // an expression with a group
        let re = Query { regex: true, ..q(r"(\w)at\b", "${1}ow") };
        let hits = find(&m, &re);
        assert_eq!(hits.len(), 3);
        let out = replace(&m, &Registry::standard(), &hits).unwrap();
        assert_eq!(out.nodes[0].note.as_deref(), Some("Cow + concow + cow"));
        // a wrong expression finds nothing and says why
        let bad = Query { regex: true, ..q("(", "") };
        assert!(find(&m, &bad).is_empty());
        assert!(bad_pattern(&bad).is_some());
    }

    #[test]
    fn only_the_hits_chosen_are_replaced_and_several_in_a_text_hold() {
        let m = model();
        let r = Registry::standard();
        let mut hits = find(&m, &q("a", "A"));
        assert!(hits.len() > 4);
        hits.retain(|h| h.setting == "expression");
        let out = replace(&m, &r, &hits).unwrap();
        // `out s = a * 2`: only the `a` the hits name, the rest of the model untouched
        let want = "out s = A * 2";
        assert_eq!(out.nodes[1].body.as_ref().unwrap().nodes[0].params["expression"], want);
        assert_eq!(out.nodes[0], m.nodes[0]);
    }

    #[test]
    fn an_invalid_replacement_refuses_everything() {
        let mut m = Model::new("m");
        m.inputs.insert(
            "land".into(),
            Input::Map { path: "a.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        m.inputs.insert(
            "other".into(),
            Input::Map { path: "b.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        m.nodes.push(Node::new("a", "raster.reclassify").input("map", "$land").param("rules", "1 -> 2"));
        m.nodes.push(Node::new("b", "raster.reclassify").input("map", "$other").param("rules", "1 -> 3"));
        let r = Registry::standard();
        // a name made one that is not valid: the input and what reads it are not renamed, the rules not changed
        let hits = find(&m, &q("land", "1 land"));
        assert!(!hits.is_empty());
        let mut all = find(&m, &q("1 ->", "9 ->"));
        all.extend(hits);
        let e = replace(&m, &r, &all).unwrap_err();
        assert!(e.contains("not a valid name") || e.contains("refused"), "{e}");
        assert_eq!(m.nodes[0].params["rules"], "1 -> 2");
        // renamed rightly, what reads it follows
        let ok = replace(&m, &r, &find(&m, &q("land", "terrain"))).unwrap();
        assert!(ok.inputs.contains_key("terrain") && !ok.inputs.contains_key("land"));
        assert_eq!(ok.nodes[0].inputs["map"], Source::Ref("$terrain".into()));
        // a name that exists already
        let e = replace(&m, &r, &find(&m, &q("land", "other"))).unwrap_err();
        assert!(e.contains("already"), "{e}");
    }

    #[test]
    fn a_setting_made_to_read_what_is_not_there_is_refused_by_the_check() {
        let mut m = Model::new("m");
        m.inputs.insert(
            "land".into(),
            Input::Map { path: "a.tif".into(), band: None, nodata: None, note: None, data: None },
        );
        m.nodes.push(Node::new("a", "raster.reclassify").input("map", "$land").param("rules", "1 -> 2"));
        m.nodes.push(Node::new("b", "raster.reclassify").input("map", "$land").param("rules", "$land"));
        let r = Registry::standard();
        let mut hits = find(&m, &q("land", "ghost"));
        hits.retain(|h| h.kind != Kind::Name);
        assert_eq!(hits.len(), 1);
        let mut all = find(&m, &q("1 ->", "9 ->"));
        all.extend(hits);
        let e = replace(&m, &r, &all).unwrap_err();
        assert!(e.starts_with("refused, nothing changed: "), "{e}");
        assert!(e.contains("ghost"), "{e}");
    }

    #[test]
    fn a_stale_hit_is_refused() {
        let mut m = model();
        let r = Registry::standard();
        let hits = find(&m, &q("dem.tif", "x.tif"));
        m.nodes[0].params.insert("path".into(), "other.tif".into());
        let e = replace(&m, &r, &hits).unwrap_err();
        assert!(e.contains("changed since"), "{e}");
    }
}
