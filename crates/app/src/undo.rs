//! Undo and redo of a model's edits (ctrl+Z, ctrl+shift+Z or ctrl+Y): the model as it was before each edit, kept
//! whole. An edit is a step of its own (a node added, deleted, pasted, a connection, a setting), but the changes
//! made while a button of the mouse is held (nodes dragged, a slider moved) are one step. Each step has a name, made
//! when it ends from what changed ("added suit (raster.calc)", "reclass: setting threshold: 3 → 4", "3 nodes
//! moved"), and the History window lists them: a click goes back or forward to any of them.

use egui::{RichText, Ui};
use terraflux_graph::Model;

use crate::app::Session;
use crate::review::{Kind, Target};

/// The steps kept.
const KEPT: usize = 200;
/// A step's name, at most this many characters.
const NAME: usize = 90;

#[derive(Default)]
pub struct History {
    /// The models before each step done, the oldest first, and the steps' names.
    undo: Vec<Model>,
    undo_names: Vec<String>,
    /// The models after each step undone, the one undone last at the end, and the steps' names.
    redo: Vec<Model>,
    redo_names: Vec<String>,
    /// The model as it is after the last step (none while a step is being made).
    last: Option<Model>,
    /// The name the step being made is to have (given by whoever made it), instead of one made from what changed.
    named: Option<String>,
}

impl History {
    pub fn new(m: &Model) -> History {
        History { last: Some(m.clone()), ..Default::default() }
    }

    /// After a change of the model: the model as it was before it kept, once for the changes of a step; what was
    /// undone cannot be done again.
    pub fn changed(&mut self, m: &Model) {
        if self.last.as_ref() == Some(m) {
            return;
        }
        if let Some(before) = self.last.take() {
            self.undo.push(before);
            self.undo_names.push(String::new());
            if self.undo.len() > KEPT {
                self.undo.remove(0);
                self.undo_names.remove(0);
            }
        }
        self.redo.clear();
        self.redo_names.clear();
    }

    /// The name of the step being made (an assistant's edit, a rename of many), instead of one made from what changed.
    pub fn name(&mut self, name: &str) {
        self.named = Some(name.to_string());
    }

    /// The last step joined to the one before it, when `m` (the model the last step began from) is where the one
    /// before ended: a connection taken off an input and let go on another is one step, not two.
    pub fn join(&mut self, m: &Model) {
        if self.undo.len() >= 2 && self.undo.last() == Some(m) {
            self.undo.pop();
            self.undo_names.pop();
        }
    }

    /// Once a frame, before the edits: a step ends unless a button of the mouse is `held` (and is named then).
    pub fn tick(&mut self, m: &Model, held: bool) {
        if self.last.is_none() && !held {
            self.last = Some(m.clone());
            if let (Some(before), Some(name)) = (self.undo.last(), self.undo_names.last_mut()) {
                *name = self.named.take().unwrap_or_else(|| describe(before, m));
            }
        }
    }

    /// The model as it was before the last step; whether there was one.
    pub fn undo(&mut self, m: &mut Model) -> bool {
        let Some(before) = self.undo.pop() else { return false };
        let name = self.undo_names.pop().unwrap_or_default();
        self.redo.push(std::mem::replace(m, before));
        self.redo_names.push(name);
        self.last = Some(m.clone());
        true
    }

    /// The step undone last made again; whether there was one.
    pub fn redo(&mut self, m: &mut Model) -> bool {
        let Some(after) = self.redo.pop() else { return false };
        let name = self.redo_names.pop().unwrap_or_default();
        self.undo.push(std::mem::replace(m, after));
        self.undo_names.push(name);
        self.last = Some(m.clone());
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// The steps by name, the oldest first: those done, then those undone (the next to do again first); how many
    /// are done (the model now is after the step of that number, 0: before them all).
    pub fn steps(&self) -> (Vec<String>, usize) {
        let mut all = self.undo_names.clone();
        // (a step still being made: its name once it ends)
        if let Some(n) = all.last_mut().filter(|n| n.is_empty()) {
            *n = "an edit being made".into();
        }
        all.extend(self.redo_names.iter().rev().cloned());
        (all, self.undo.len())
    }

    /// The model after step `k` (0: before them all), `now` being the model now.
    pub fn model_at(&self, k: usize, now: &Model) -> Option<Model> {
        let done = self.undo.len();
        if k < done {
            return self.undo.get(k).cloned();
        }
        if k == done {
            return Some(now.clone());
        }
        let back = k - done;
        self.redo.len().checked_sub(back).and_then(|i| self.redo.get(i)).cloned()
    }
}

/// A step's name from what changed: one node added, deleted or changed (its change when it is one), several counted;
/// else what moved.
pub fn describe(before: &Model, after: &Model) -> String {
    let changes = crate::review::diff(before, after);
    let name = match changes.as_slice() {
        [] => moved(before, after),
        [c] => {
            let what = match &c.target {
                Target::Node(_) => c.title.clone(),
                Target::Input(n) => format!("input ${n}"),
                Target::Output(n) => format!("output {n}"),
                Target::About => "the model's record".into(),
            };
            match (c.kind, c.details.as_slice()) {
                (Kind::Added, _) => format!("added {what}"),
                (Kind::Removed, _) => format!("deleted {what}"),
                (Kind::Changed, [one]) => format!("{}: {one}", short_title(c)),
                (Kind::Changed, many) => format!("{}: {} changes", short_title(c), many.len()),
            }
        }
        many => {
            let count = |k: Kind| many.iter().filter(|c| c.kind == k).count();
            let (added, removed, changed) = (count(Kind::Added), count(Kind::Removed), count(Kind::Changed));
            let names = |k: Kind| {
                let n: Vec<String> = many.iter().filter(|c| c.kind == k).map(short_title).collect();
                n.join(", ")
            };
            match (added, removed, changed) {
                (a, 0, 0) => format!("added {a}: {}", names(Kind::Added)),
                (0, r, 0) => format!("deleted {r}: {}", names(Kind::Removed)),
                (0, 0, c) => format!("changed {c}: {}", names(Kind::Changed)),
                (a, r, c) => [(a, "added"), (r, "deleted"), (c, "changed")]
                    .iter()
                    .filter(|(n, _)| *n > 0)
                    .map(|(n, w)| format!("{w} {n}"))
                    .collect::<Vec<_>>()
                    .join(", "),
            }
        }
    };
    if name.chars().count() > NAME { format!("{}…", name.chars().take(NAME).collect::<String>()) } else { name }
}

/// A change's node by its id alone (not its operator), else its title.
fn short_title(c: &crate::review::Change) -> String {
    match &c.target {
        Target::Node(id) => id.clone(),
        _ => c.title.clone(),
    }
}

/// What moved between two models that differ in no node, input or output: nodes on the canvas, boxes of what a
/// scope reads, the views kept, the parameters.
fn moved(before: &Model, after: &Model) -> String {
    let mut n = 0;
    count_moved(&before.nodes, &after.nodes, &mut n);
    if n == 1 {
        "a node moved".into()
    } else if n > 1 {
        format!("{n} nodes moved")
    } else if before.places != after.places {
        "a box moved".into()
    } else if before.plots != after.plots {
        "the Plots changed".into()
    } else if before.parameters != after.parameters || before.scenarios != after.scenarios {
        "the parameters changed".into()
    } else {
        "an edit".into()
    }
}

fn count_moved(b: &[terraflux_graph::Node], a: &[terraflux_graph::Node], n: &mut usize) {
    for x in a {
        if let Some(y) = b.iter().find(|y| y.id == x.id) {
            if x.pos != y.pos || x.width != y.width {
                *n += 1;
            }
            if let (Some(xb), Some(yb)) = (&x.body, &y.body) {
                count_moved(&yb.nodes, &xb.nodes, n);
            }
        }
    }
}

/// Go to the model after step `k` (0: before them all), undoing or doing again the steps between, in one go.
pub fn go_to(s: &mut Session, k: usize) {
    let (_, done) = s.history.steps();
    let redo = k > done;
    for _ in 0..k.abs_diff(done) {
        crate::builder::undo_redo(s, redo);
    }
}

/// The History window: the model as it was opened, then its steps, the one it is at marked, those undone faint; a
/// click goes to one, "compare" opens the Changes with the model as it was then.
pub fn window(ctx: &egui::Context, s: &mut Session) {
    if !s.history_open {
        return;
    }
    let mut open = true;
    let (names, done) = s.history.steps();
    let mut go = None;
    let mut compare = None;
    let at = ctx.content_rect().right_top() + egui::vec2(-400.0, 90.0);
    egui::Window::new("History").open(&mut open).default_pos(at).default_width(360.0).default_height(420.0).show(
        ctx,
        |ui| {
            ui.label(
                RichText::new(format!(
                    "{} step{} done, {} undone",
                    done,
                    if done == 1 { "" } else { "s" },
                    names.len() - done
                ))
                .weak()
                .size(11.0),
            );
            egui::ScrollArea::vertical().auto_shrink([false, true]).show(ui, |ui| {
                let mut row = |ui: &mut Ui, k: usize, text: &str| {
                    let here = k == done;
                    let undone = k > done;
                    ui.horizontal(|ui| {
                        let text = if undone { RichText::new(text).weak() } else { RichText::new(text) };
                        let r = ui.add(egui::Button::selectable(here, text).truncate());
                        if r.on_hover_text(if here { "the model now" } else { "go back or forward to here" }).clicked()
                            && !here
                        {
                            go = Some(k);
                        }
                        if !here
                            && ui
                                .small_button("compare")
                                .on_hover_text("the changes from then to now, in Changes")
                                .clicked()
                        {
                            compare = Some(k);
                        }
                    });
                };
                row(ui, 0, "the model as it was opened");
                for (k, n) in names.iter().enumerate() {
                    row(ui, k + 1, &format!("{}. {n}", k + 1));
                }
            });
        },
    );
    s.history_open = open;
    if let Some(k) = compare
        && let Some(m) = s.history.model_at(k, &s.source)
    {
        crate::review::compare(s, m, &format!("the changes since step {k} of the History"));
    }
    if let Some(k) = go {
        go_to(s, k);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_graph::Node;

    #[test]
    fn steps_are_undone_and_done_again() {
        let mut m = Model::new("t");
        let mut h = History::new(&m);
        // a step: a node added
        m.nodes.push(Node::new("a", "raster.calc"));
        h.changed(&m);
        h.tick(&m, false);
        let one = m.clone();
        // another: the node dragged, frame after frame with the button held
        for k in 0..5 {
            m.nodes[0].pos = Some([k as f32, 0.0]);
            h.changed(&m);
            h.tick(&m, true);
        }
        h.tick(&m, false);
        let two = m.clone();
        // nothing changed: no step
        h.changed(&m);
        h.tick(&m, false);
        assert!(h.undo(&mut m));
        assert_eq!(m, one);
        assert!(h.undo(&mut m));
        assert!(m.nodes.is_empty());
        assert!(!h.undo(&mut m));
        assert!(h.redo(&mut m) && h.redo(&mut m));
        assert_eq!(m, two);
        assert!(!h.redo(&mut m));
        // a change after an undo: what was undone is gone
        assert!(h.undo(&mut m));
        m.nodes.push(Node::new("c", "raster.calc"));
        h.changed(&m);
        assert!(!h.can_redo());
    }

    /// A step made: the model changed by `f`, the step ended.
    fn step(h: &mut History, m: &mut Model, f: impl FnOnce(&mut Model)) {
        f(m);
        h.changed(m);
        h.tick(m, false);
    }

    #[test]
    fn steps_are_named_by_what_changed() {
        let mut m = Model::new("t");
        let mut h = History::new(&m);
        step(&mut h, &mut m, |m| m.nodes.push(Node::new("suit", "raster.calc")));
        step(&mut h, &mut m, |m| {
            m.nodes[0].params.insert("expression".into(), serde_json::json!("a + 1"));
        });
        step(&mut h, &mut m, |m| m.nodes[0].pos = Some([10.0, 0.0]));
        step(&mut h, &mut m, |m| {
            m.nodes.push(Node::new("a", "nlm.random"));
            m.nodes.push(Node::new("b", "nlm.random"));
        });
        h.name("renamed by a pattern");
        step(&mut h, &mut m, |m| m.nodes[1].id = "c".into());
        step(&mut h, &mut m, |m| m.nodes.clear());
        let (names, done) = h.steps();
        assert_eq!(done, 6);
        assert_eq!(names[0], "added suit (raster.calc)");
        assert_eq!(names[1], "suit: setting expression: \"a + 1\" (was the default)");
        assert_eq!(names[2], "a node moved");
        assert_eq!(names[3], "added 2: a, b");
        assert_eq!(names[4], "renamed by a pattern");
        assert_eq!(names[5], "deleted 3: suit, c, b");
        // undone: listed after the steps done, the next to do again first
        h.undo(&mut m);
        h.undo(&mut m);
        let (names, done) = h.steps();
        assert_eq!((names.len(), done), (6, 4));
        assert_eq!(names[4], "renamed by a pattern");
        assert_eq!(names[5], "deleted 3: suit, c, b");
    }

    #[test]
    fn the_model_after_any_step() {
        let mut m = Model::new("t");
        let mut h = History::new(&m);
        let mut at = vec![m.clone()];
        for id in ["a", "b", "c"] {
            step(&mut h, &mut m, |m| m.nodes.push(Node::new(id, "nlm.random")));
            at.push(m.clone());
        }
        h.undo(&mut m);
        for (k, want) in at.iter().enumerate() {
            assert_eq!(h.model_at(k, &m).as_ref(), Some(want), "after step {k}");
        }
        assert!(h.model_at(4, &m).is_none());
    }
}
