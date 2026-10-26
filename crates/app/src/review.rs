//! The review of a model's changes, as a pull request's: what an assistant changed (in the application, or
//! through `terraflux mcp` on the model's file), or the differences with another version of the model (its file
//! on disk, another file, its last commit). A semantic diff of the graph: the nodes added, removed and changed
//! (their operator, connections, settings and record, before and after), the model's inputs, outputs and record;
//! each change accepted or rejected (rejected: an edit that ctrl+Z undoes), or all at once; the maps the changed
//! nodes give, before and after, side by side. The nodes changed are marked on the canvas.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use egui::{Color32, Margin, RichText, Ui};
use serde_json::Value;
use terraflux_graph::{Body, Model, Node, Source};
use terraflux_ops::Registry;

use crate::app::Session;
use crate::theme;

/// The colours of what was added, removed and changed.
pub const ADDED: Color32 = Color32::from_rgb(92, 196, 92);
pub const REMOVED: Color32 = Color32::from_rgb(232, 90, 80);
pub const CHANGED: Color32 = Color32::from_rgb(236, 190, 52);

/// The texts shown of a value, at most this many characters.
const SHORT: usize = 140;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Added,
    Removed,
    Changed,
}

/// What a change is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// The model's record: its name, description, request, summary, author.
    About,
    Input(String),
    Output(String),
    /// A node of the top level (with its body).
    Node(String),
}

/// A change: what it is of, and what changed in it (a line each: before -> after).
#[derive(Debug, Clone, PartialEq)]
pub struct Change {
    pub kind: Kind,
    pub target: Target,
    pub title: String,
    pub details: Vec<String>,
}

/// A review going on: the model the changes are counted from, who made them, what the nodes gave before.
pub struct Review {
    pub base: Model,
    pub by: String,
    pub before: BTreeMap<String, terraflux_ops::Outputs>,
    /// The Changes pane to be brought to the front (a review begun by another program, or asked for).
    pub show: bool,
    /// The changes, counted at the model's revision.
    seen: Option<u64>,
    changes: Vec<Change>,
}

/// A value in a line.
fn short(v: &Value) -> String {
    let t = match v {
        Value::String(s) => format!("{s:?}"),
        other => other.to_string(),
    };
    cut(&t)
}

fn cut(t: &str) -> String {
    let t = t.replace('\n', " ");
    if t.chars().count() > SHORT { format!("{}…", t.chars().take(SHORT).collect::<String>()) } else { t }
}

fn source(s: &Source) -> String {
    match s {
        Source::Ref(r) => r.clone(),
        Source::Literal(v) => short(v),
    }
}

/// A text before and after: (added, removed, changed) in words.
fn text_change(what: &str, before: Option<&str>, after: Option<&str>, out: &mut Vec<String>) {
    match (before, after) {
        (None, Some(a)) => out.push(format!("{what}: {}", cut(a))),
        (Some(b), None) => out.push(format!("{what}: removed (was {})", cut(b))),
        (Some(b), Some(a)) if a != b => out.push(format!("{what}: {} → {}", cut(b), cut(a))),
        _ => {}
    }
}

/// A list before and after: the items added and removed.
fn list_change(what: &str, before: &[String], after: &[String], out: &mut Vec<String>) {
    for a in after.iter().filter(|a| !before.contains(a)) {
        out.push(format!("{what}: + {}", cut(a)));
    }
    for b in before.iter().filter(|b| !after.contains(b)) {
        out.push(format!("{what}: − {}", cut(b)));
    }
}

/// What changed in a node (its place on the canvas aside); `at`: the container it is in (for the lines of a
/// body's nodes).
fn node_details(b: &Node, a: &Node, at: &str, out: &mut Vec<String>) {
    let p = |t: String| if at.is_empty() { t } else { format!("{at}: {t}") };
    let mut own = Vec::new();
    if a.op != b.op {
        own.push(format!("operator: {} → {}", b.op, a.op));
    }
    let ports: BTreeSet<&String> = b.inputs.keys().chain(a.inputs.keys()).collect();
    for port in ports {
        match (b.inputs.get(port), a.inputs.get(port)) {
            (None, Some(x)) => own.push(format!("input {port}: {}", source(x))),
            (Some(x), None) => own.push(format!("input {port}: disconnected (was {})", source(x))),
            (Some(x), Some(y)) if x != y => own.push(format!("input {port}: {} → {}", source(x), source(y))),
            _ => {}
        }
    }
    let names: BTreeSet<&String> = b.params.keys().chain(a.params.keys()).collect();
    for name in names {
        match (b.params.get(name), a.params.get(name)) {
            (None, Some(x)) => own.push(format!("setting {name}: {} (was the default)", short(x))),
            (Some(x), None) => own.push(format!("setting {name}: the default (was {})", short(x))),
            (Some(x), Some(y)) if x != y => own.push(format!("setting {name}: {} → {}", short(x), short(y))),
            _ => {}
        }
    }
    text_change("note", b.note.as_deref(), a.note.as_deref(), &mut own);
    text_change("why", b.why.as_deref(), a.why.as_deref(), &mut own);
    list_change("refs", &b.refs, &a.refs, &mut own);
    list_change("assumptions", &b.assumptions, &a.assumptions, &mut own);
    list_change("alternatives", &b.alternatives, &a.alternatives, &mut own);
    out.extend(own.into_iter().map(p));
    let empty = Body::default();
    let (bb, ab) = (b.body.as_ref().unwrap_or(&empty), a.body.as_ref().unwrap_or(&empty));
    let inside = if at.is_empty() { a.id.clone() } else { format!("{at}/{}", a.id) };
    for (what, x, y) in [("carries", &bb.carry, &ab.carry), ("collects", &bb.collect, &ab.collect)] {
        if x != y {
            out.push(format!("{inside} {what}: {x:?} → {y:?}"));
        }
    }
    if bb.outputs != ab.outputs {
        out.push(format!("{inside} gives: {:?} → {:?}", bb.outputs, ab.outputs));
    }
    if bb.until != ab.until {
        out.push(format!("{inside} ends: {:?} → {:?}", bb.until, ab.until));
    }
    for n in &ab.nodes {
        match bb.nodes.iter().find(|m| m.id == n.id) {
            None => out.push(format!("{inside}: node {} added ({})", n.id, n.op)),
            Some(m) => node_details(m, n, &inside, out),
        }
    }
    for m in bb.nodes.iter().filter(|m| !ab.nodes.iter().any(|n| n.id == m.id)) {
        out.push(format!("{inside}: node {} removed (was {})", m.id, m.op));
    }
}

/// What a node added is: its connections, settings and record.
fn node_added(n: &Node) -> Vec<String> {
    let mut out = Vec::new();
    node_details(&Node::new(&n.id, &n.op), n, "", &mut out);
    out
}

/// The changes from `before` to `after`.
pub fn diff(before: &Model, after: &Model) -> Vec<Change> {
    let mut all = Vec::new();
    let mut about = Vec::new();
    for (what, b, a) in [
        ("name", &before.name, &after.name),
        ("description", &before.description, &after.description),
        ("request", &before.request, &after.request),
        ("summary", &before.summary, &after.summary),
        ("author", &before.author, &after.author),
    ] {
        text_change(what, b.as_deref(), a.as_deref(), &mut about);
    }
    if !about.is_empty() {
        all.push(Change {
            kind: Kind::Changed,
            target: Target::About,
            title: "the model's record".into(),
            details: about,
        });
    }
    let names: BTreeSet<&String> = before.inputs.keys().chain(after.inputs.keys()).collect();
    for name in names {
        let (b, a) = (before.inputs.get(name), after.inputs.get(name));
        if b == a {
            continue;
        }
        let kind = match (b, a) {
            (None, _) => Kind::Added,
            (_, None) => Kind::Removed,
            _ => Kind::Changed,
        };
        let mut details = Vec::new();
        text_change("is", b.map(line).as_deref(), a.map(line).as_deref(), &mut details);
        all.push(Change { kind, target: Target::Input(name.clone()), title: format!("input ${name}"), details });
    }
    let names: BTreeSet<&String> = before.outputs.keys().chain(after.outputs.keys()).collect();
    for name in names {
        let (b, a) = (before.outputs.get(name), after.outputs.get(name));
        if b == a {
            continue;
        }
        let kind = match (b, a) {
            (None, _) => Kind::Added,
            (_, None) => Kind::Removed,
            _ => Kind::Changed,
        };
        let mut details = Vec::new();
        text_change("writes", b.map(line).as_deref(), a.map(line).as_deref(), &mut details);
        all.push(Change { kind, target: Target::Output(name.clone()), title: format!("output {name}"), details });
    }
    for n in &after.nodes {
        match before.nodes.iter().find(|m| m.id == n.id) {
            None => all.push(Change {
                kind: Kind::Added,
                target: Target::Node(n.id.clone()),
                title: format!("{} ({})", n.id, n.op),
                details: node_added(n),
            }),
            Some(m) => {
                let mut details = Vec::new();
                node_details(m, n, "", &mut details);
                if !details.is_empty() {
                    all.push(Change {
                        kind: Kind::Changed,
                        target: Target::Node(n.id.clone()),
                        title: format!("{} ({})", n.id, n.op),
                        details,
                    });
                }
            }
        }
    }
    for m in before.nodes.iter().filter(|m| !after.nodes.iter().any(|n| n.id == m.id)) {
        all.push(Change {
            kind: Kind::Removed,
            target: Target::Node(m.id.clone()),
            title: format!("{} ({})", m.id, m.op),
            details: Vec::new(),
        });
    }
    all
}

/// A part of the model (an input, an output) as one line of JSON.
fn line<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_string(v).unwrap_or_default()
}

/// What a change is of, as `from` has it, put in `into` (taken out of it when `from` has none).
pub fn take(target: &Target, from: &Model, into: &mut Model) {
    match target {
        Target::About => {
            into.name = from.name.clone();
            into.description = from.description.clone();
            into.request = from.request.clone();
            into.summary = from.summary.clone();
            into.author = from.author.clone();
        }
        Target::Input(name) => match from.inputs.get(name) {
            Some(i) => {
                into.inputs.insert(name.clone(), i.clone());
            }
            None => {
                into.inputs.remove(name);
            }
        },
        Target::Output(name) => match from.outputs.get(name) {
            Some(o) => {
                into.outputs.insert(name.clone(), o.clone());
            }
            None => {
                into.outputs.remove(name);
            }
        },
        Target::Node(id) => {
            match (from.nodes.iter().find(|n| n.id == *id), into.nodes.iter().position(|n| n.id == *id)) {
                (Some(n), Some(k)) => into.nodes[k] = n.clone(),
                (Some(n), None) => into.nodes.push(n.clone()),
                (None, Some(k)) => {
                    into.nodes.remove(k);
                }
                (None, None) => {}
            }
        }
    }
}

/// Begin a review of the changes about to be made to the model open (by `by`), unless one goes on (then `by` is
/// one more of its authors).
pub fn begin(s: &mut Session, by: &str, show: bool) {
    match &mut s.review {
        Some(r) => {
            if !r.by.contains(by) {
                r.by = format!("{}; {by}", r.by);
            }
            r.show |= show;
        }
        None => {
            s.review = Some(Review {
                base: s.source.clone(),
                by: by.into(),
                before: s.outputs.clone(),
                show,
                seen: None,
                changes: Vec::new(),
            })
        }
    }
}

/// Compare the model open with another version of it: the review of the differences (from that version).
pub fn compare(s: &mut Session, other: Model, what: &str) {
    s.review = Some(Review {
        base: other,
        by: what.into(),
        before: BTreeMap::new(),
        show: true,
        seen: None,
        changes: Vec::new(),
    });
}

/// Once a frame: the changes counted again after an edit (the review ends when none is left), and the nodes
/// changed marked on the canvas.
pub fn follow(s: &mut Session) {
    let Some(r) = &mut s.review else {
        s.editor.marks.clear();
        return;
    };
    if r.seen != Some(s.revision) {
        r.changes = diff(&r.base, &s.source);
        r.seen = Some(s.revision);
        s.editor.marks = r
            .changes
            .iter()
            .filter_map(|c| match (&c.target, c.kind) {
                (Target::Node(id), Kind::Added) => Some((id.clone(), ADDED)),
                (Target::Node(id), Kind::Changed) => Some((id.clone(), CHANGED)),
                _ => None,
            })
            .collect::<HashMap<_, _>>();
        if r.changes.is_empty() {
            s.review = None;
            s.editor.marks.clear();
        }
    }
}

/// The changes of the review going on.
pub fn changes(s: &Session) -> &[Change] {
    s.review.as_ref().map_or(&[], |r| &r.changes)
}

/// Accept a change: it stays, and leaves the review.
pub fn accept(s: &mut Session, target: &Target) {
    if let Some(r) = &mut s.review {
        take(target, &s.source, &mut r.base);
        r.seen = None;
    }
}

/// Reject a change: the model as it was there (an edit that ctrl+Z undoes).
pub fn reject(s: &mut Session, target: &Target) {
    let Some(r) = &s.review else { return };
    let mut m = s.source.clone();
    take(target, &r.base, &mut m);
    put(s, m);
}

/// Accept every change: the review ends.
pub fn accept_all(s: &mut Session) {
    s.review = None;
    s.editor.marks.clear();
}

/// Reject every change: the model as it was when the review began.
pub fn reject_all(s: &mut Session) {
    let Some(r) = &s.review else { return };
    let m = r.base.clone();
    put(s, m);
}

/// The model changed by the review: an edit of its own, its data read again when its inputs changed.
fn put(s: &mut Session, m: Model) {
    let inputs = m.inputs != s.source.inputs;
    s.source = m;
    crate::builder::forget_missing(s);
    s.edited();
    if inputs {
        s.read_data();
    }
}

/// The versions the model open may be compared with: its file on disk, another file, its last commit. Whether a
/// comparison began.
pub fn compare_menu(ui: &mut Ui, s: &mut Session) -> bool {
    let mut begun = false;
    let file = s.path.clone();
    if ui
        .add_enabled(file.is_some(), egui::Button::new("Compare with the saved file"))
        .on_hover_text("the changes not saved, to keep or undo one by one")
        .clicked()
        && let Some(p) = &file
    {
        ui.close();
        match Model::load(p) {
            Ok(m) => {
                compare(s, m, "the changes since the file was saved");
                begun = true;
            }
            Err(e) => s.notice = Some((format!("{}: {e}", p.display()), std::time::Instant::now())),
        }
    }
    if ui.add_enabled(file.is_some(), egui::Button::new("Compare with the last commit (git)")).clicked()
        && let Some(p) = &file
    {
        ui.close();
        match committed(p) {
            Ok(m) => {
                compare(s, m, "the changes since the last commit");
                begun = true;
            }
            Err(e) => s.notice = Some((e, std::time::Instant::now())),
        }
    }
    if ui.button("Compare with a file...").clicked() {
        ui.close();
        if let Some(p) = rfd::FileDialog::new().add_filter("model", &["toml", "json"]).pick_file() {
            match Model::load(&p) {
                Ok(m) => {
                    let name = p.file_name().map_or(String::new(), |n| n.to_string_lossy().into_owned());
                    compare(s, m, &format!("the differences with {name}"));
                    begun = true;
                }
                Err(e) => s.notice = Some((format!("{}: {e}", p.display()), std::time::Instant::now())),
            }
        }
    }
    begun
}

/// The model's file as its repository's last commit has it.
fn committed(path: &Path) -> Result<Model, String> {
    let dir = path.parent().ok_or("the model has no folder")?;
    let name = path.file_name().ok_or("the model has no file")?.to_string_lossy().into_owned();
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .arg("show")
        .arg(format!("HEAD:./{name}"))
        .output()
        .map_err(|e| format!("git did not run: {e}"))?;
    if !out.status.success() {
        return Err(format!("git: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let m = if name.to_lowercase().ends_with(".json") { Model::from_json(&text) } else { Model::from_toml(&text) };
    m.map_err(|e| format!("{name} in the last commit: {e}"))
}

/// The Changes pane: the review going on, its changes to accept or reject.
pub fn pane(ui: &mut Ui, s: &mut Session, r: &Registry) {
    egui::ScrollArea::vertical().id_salt("changes pane").show(ui, |ui| {
        egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
            theme::section(ui, "Changes");
            let Some(by) = s.review.as_ref().map(|r| r.by.clone()) else {
                ui.label(
                    RichText::new(
                        "No changes to review. What an assistant changes (here, or through terraflux mcp) is listed \
                         here to accept or reject, and File, Compare lists the differences with another version.",
                    )
                    .weak(),
                );
                return;
            };
            let list = changes(s).to_vec();
            ui.label(format!("{} change(s): {by}", list.len()));
            ui.horizontal(|ui| {
                if ui.add(crate::app::accent_button("Accept all", true)).on_hover_text("keep every change").clicked() {
                    accept_all(s);
                }
                if ui.button("Reject all").on_hover_text("the model as it was (ctrl+Z goes back)").clicked() {
                    reject_all(s);
                }
            });
            ui.add_space(6.0);
            for (k, c) in list.iter().enumerate() {
                change_ui(ui, s, r, k, c);
                ui.add_space(4.0);
            }
        });
    });
}

fn change_ui(ui: &mut Ui, s: &mut Session, r: &Registry, k: usize, c: &Change) {
    let (mark, colour) = match c.kind {
        Kind::Added => ("+", ADDED),
        Kind::Removed => ("−", REMOVED),
        Kind::Changed => ("~", CHANGED),
    };
    egui::Frame::new().fill(theme::well()).corner_radius(4.0).inner_margin(Margin::same(6)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(RichText::new(mark).strong().color(colour));
            ui.label(RichText::new(&c.title).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("✖").on_hover_text("reject: as it was").clicked() {
                    reject(s, &c.target);
                }
                if ui.small_button("✔").on_hover_text("accept: keep it").clicked() {
                    accept(s, &c.target);
                }
                if let Target::Node(id) = &c.target
                    && c.kind != Kind::Removed
                    && ui.small_button("Show").on_hover_text("select it on the canvas").clicked()
                {
                    s.editor.path.clear();
                    crate::builder::select(s, r, id);
                    s.editor.reveal(id);
                }
            });
        });
        for d in &c.details {
            ui.label(RichText::new(d).small().monospace());
        }
        if let Target::Node(id) = &c.target
            && c.kind != Kind::Removed
        {
            before_after(ui, s, r, k, id);
        }
    });
}

/// What a changed node gives, before the changes and now: its maps side by side, else its values in words.
fn before_after(ui: &mut Ui, s: &mut Session, r: &Registry, k: usize, id: &str) {
    let Some(n) = s.source.nodes.iter().find(|n| n.id == id) else { return };
    let ports = crate::editor::output_ports(n, r);
    let Some((port, _)) = ports.first() else { return };
    let before = s.review.as_ref().and_then(|rv| rv.before.get(id)).and_then(|o| o.get(port)).cloned();
    let after = s.value_of(id, port);
    if before.is_none() && after.is_none() {
        return;
    }
    egui::CollapsingHeader::new(RichText::new(format!("{id}.{port}, before and after")).small())
        .id_salt(("change values", k, id))
        .default_open(true)
        .show(ui, |ui| {
            use terraflux_ops::Value as V;
            match (&before, &after) {
                (b @ (Some(V::Map(_)) | None), a @ (Some(V::Map(_)) | None)) => {
                    // the same colours for both: those of the map now (else of the one before)
                    let base = match (a, b) {
                        (Some(V::Map(m)), _) | (None, Some(V::Map(m))) => Some(m.data.clone()),
                        _ => None,
                    };
                    let Some(base) = base else { return };
                    let coloring = crate::app::Look::of(&base, None).coloring;
                    ui.horizontal(|ui| {
                        for (label, v) in [("before", b), ("now", a)] {
                            ui.vertical(|ui| {
                                ui.label(RichText::new(label).small().weak());
                                match v {
                                    Some(V::Map(m)) => {
                                        let (tex, size) = s.thumbs.get(ui.ctx(), m, &coloring);
                                        let w = (ui.available_width() / 2.0 - 8.0).clamp(60.0, 160.0);
                                        ui.image((tex, size * (w / size.x.max(1.0))));
                                    }
                                    _ => {
                                        ui.label(RichText::new("not run").small().weak());
                                    }
                                }
                            });
                        }
                    });
                }
                _ => {
                    let say = |v: &Option<V>| v.as_ref().map_or("not run".into(), crate::tools::summarise);
                    ui.label(RichText::new(format!("before: {}", say(&before))).small());
                    ui.label(RichText::new(format!("now: {}", say(&after))).small());
                }
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        let mut m = Model::new("m");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 10).param("columns", 10));
        m.nodes.push(Node::new("high", "raster.calc").input("x", "land.map").param("expression", "x > 0.5"));
        m
    }

    #[test]
    fn the_changes_are_told_and_taken_back() {
        let before = model();
        let mut after = before.clone();
        after.nodes[1].params.insert("expression".into(), "x > 0.7".into());
        after.nodes[1].why = Some("a stricter threshold".into());
        after.nodes[0].pos = Some([10.0, 20.0]);
        after.nodes.push(Node::new("low", "raster.calc").input("x", "land.map").param("expression", "x < 0.2"));
        after.request = Some("the high and the low cells".into());
        let all = diff(&before, &after);
        let kinds: Vec<(Kind, &Target)> = all.iter().map(|c| (c.kind, &c.target)).collect();
        assert_eq!(
            kinds,
            [
                (Kind::Changed, &Target::About),
                (Kind::Changed, &Target::Node("high".into())),
                (Kind::Added, &Target::Node("low".into())),
            ],
            "a node only moved is not a change"
        );
        let high = &all[1].details;
        assert!(high.contains(&"setting expression: \"x > 0.5\" → \"x > 0.7\"".to_string()), "{high:?}");
        assert!(high.iter().any(|d| d.starts_with("why: a stricter")), "{high:?}");
        assert!(all[2].details.iter().any(|d| d == "input x: land.map"), "{:?}", all[2].details);
        // the threshold taken back: the high node as it was, the rest kept
        let mut m = after.clone();
        take(&Target::Node("high".into()), &before, &mut m);
        assert_eq!(m.node("high"), before.node("high"));
        assert!(m.node("low").is_some());
        // the node added taken back: gone; one removed taken back: there again
        take(&Target::Node("low".into()), &before, &mut m);
        assert!(m.node("low").is_none());
        let mut gone = before.clone();
        gone.nodes.remove(0);
        let removed = diff(&before, &gone);
        assert_eq!((removed[0].kind, &removed[0].target), (Kind::Removed, &Target::Node("land".into())));
        take(&Target::Node("land".into()), &before, &mut gone);
        assert!(gone.node("land").is_some());
    }

    #[test]
    fn a_review_is_accepted_and_rejected_change_by_change() {
        let r = Registry::standard();
        let mut s = Session::from_model(model()).unwrap();
        begin(&mut s, "the assistant", false);
        let mut m = s.source.clone();
        m.nodes[1].params.insert("expression".into(), "x > 0.9".into());
        m.nodes.push(Node::new("low", "raster.calc").input("x", "land.map").param("expression", "x < 0.2"));
        s.source = m;
        s.edited();
        follow(&mut s);
        assert_eq!(changes(&s).len(), 2);
        assert_eq!(s.editor.marks.get("low"), Some(&ADDED));
        assert_eq!(s.editor.marks.get("high"), Some(&CHANGED));
        // the new node kept, the threshold rejected (an edit undone by ctrl+Z)
        accept(&mut s, &Target::Node("low".into()));
        follow(&mut s);
        assert_eq!(changes(&s).len(), 1);
        reject(&mut s, &Target::Node("high".into()));
        follow(&mut s);
        assert!(s.review.is_none(), "nothing left to review");
        assert_eq!(s.source.node("high").unwrap().params["expression"], "x > 0.5");
        assert!(s.source.node("low").is_some());
        assert!(s.history.can_undo());
        // all rejected at once: the model as it was
        begin(&mut s, "terraflux mcp", true);
        let at = s.source.clone();
        crate::tools::edit("remove_node", &mut s.source, &r, Path::new(""), &serde_json::json!({ "node": "low" }))
            .unwrap();
        s.edited();
        follow(&mut s);
        assert!(s.review.as_ref().is_some_and(|rv| rv.show));
        reject_all(&mut s);
        follow(&mut s);
        assert_eq!(s.source, at);
        assert!(s.review.is_none());
    }
}
