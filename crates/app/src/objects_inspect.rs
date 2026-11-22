//! Objects in the Viewer, selected and inspected, and samples given by clicking.
//!
//! A click on an objects layer selects the object (kept until another click or Esc; ctrl toggles); N adds the
//! neighbours of the selected, P goes to their parents (the level above is drawn then), C to their children (when
//! the level shown has a level beneath it in the chain of parents). The selection is the attribute table's: a row
//! selected is an object selected.
//!
//! The inspector tells the selected object: its attributes, its class and its membership to every class, the
//! margin, its parent and children, and what decided (the rules of `objects.rules` and their values here, the
//! probabilities of `objects.classify`, the distance to the nearest samples of `objects.nearest`).
//!
//! In samples mode a click gives an object the class chosen in the palette, a drag paints several, a right click
//! takes the sample away. A sample is a point at the object's interior cell nearest its centroid (so that it
//! survives another segmentation); [`apply`] writes them into the model: a `table.of` with the points' x, y and class
//! feeding a `vector.from_table` (more than [`IN_SETTINGS`] samples: a GeoPackage beside the model read by a
//! `vector.read`), feeding `objects.samples`, which is made, with `ml.fit` and `objects.classify`, when the model has
//! none for these objects.

use std::collections::{BTreeSet, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Ui};
use terraflux_graph::{Model, Node, Source};
use terraflux_ops::objects::Objects;
use terraflux_ops::{Registry, Value};
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::app::Session;
use crate::map_view::{ObjectEvent, ObjectEventKind};
use crate::objects_view::{at_level, class_colour_of, object_text};
use crate::palette::class_colour;
use crate::table_view::Pick;

/// The most samples kept in the model's settings: more go to a GeoPackage beside the model.
pub const IN_SETTINGS: usize = 500;
/// A class with fewer samples than this is warned about.
pub const FEW: usize = 10;
/// How long the samples stay unchanged before the live mode writes them.
const DEBOUNCE: Duration = Duration::from_millis(800);
/// The ids the nodes made are given (made unique when taken).
const POINTS: &str = "sample_points";

/// A sample: the cell (row, column) and the class (an index of the palette's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub row: usize,
    pub col: usize,
    pub class: usize,
}

/// What the Viewer keeps of the objects selected and the samples given.
#[derive(Default)]
pub struct ObjectsUi {
    /// The layer (node, output) these are of, and the address of the labels of its objects (the selection is of
    /// those objects, not of another level or another segmentation).
    pub key: (String, String),
    labels: usize,
    pub selected: BTreeSet<u32>,
    /// The selection as the table and the map last agreed on it.
    synced: BTreeSet<u32>,
    pub sampling: bool,
    /// The classes of the palette (names), the one chosen and the name being typed.
    pub classes: Vec<String>,
    pub active: usize,
    new_class: String,
    pub samples: Vec<Sample>,
    /// Whether the samples are written as they are given (after a pause).
    pub live: bool,
    /// When the samples last changed and were not written.
    pub changed: Option<Instant>,
    pub message: Option<String>,
    /// The most samples kept in the model's settings (0: [`IN_SETTINGS`]); more go to a GeoPackage.
    pub most: usize,
    /// The values of the rules of an `objects.rules` node (by class), and what they were made from.
    rules: Option<(u64, Vec<Option<Vec<f64>>>)>,
    /// The feature space window (see [`crate::feature_space`]).
    pub space: crate::feature_space::Space,
    /// An edit asked for in the *Edit* block (the Viewer writes it into the model), whether a cut line is being
    /// drawn, and whether a delete goes into the most similar neighbour.
    pub pending: Option<crate::objects_edit::Edit>,
    pub cutting: bool,
    /// Whether the Viewer put the sketch of the cut line on the map (so that it takes it away when the cut ends).
    pub cut_sketch: bool,
    into_neighbour: bool,
    /// *Find similar*: the objects like the selected ones, lit on the map as the threshold moves.
    pub finding: Finding,
    /// The samples are to be written now (*Label all as...* gave them): the Viewer applies them, as it does for *Apply*.
    pub write_now: bool,
    /// The SAM tool: clicks on the map cut objects by Segment Anything (see [`crate::sam_tool`]).
    pub sam: crate::sam_tool::SamTool,
}

/// What *Find similar* keeps: whether it is on, the metric and the threshold, and the similarity of every object to the
/// objects selected when it was made (kept while the selection, the objects and the metric stay).
pub struct Finding {
    pub on: bool,
    pub cosine: bool,
    pub threshold: f64,
    made: Option<Made>,
    error: Option<String>,
}

impl Default for Finding {
    fn default() -> Finding {
        Finding { on: false, cosine: true, threshold: 0.9, made: None, error: None }
    }
}

struct Made {
    labels: usize,
    cosine: bool,
    examples: Vec<u32>,
    similarity: Vec<f64>,
    used: String,
}

/// What a key pressed over the objects asks of what is drawn.
pub enum Go {
    /// Another level of the chain of parents (the parents or the children are selected there).
    Level(usize),
    /// The `below` output of the node (a layer's port is changed to it; the children are selected there).
    Below,
}

impl ObjectsUi {
    /// The layer this is for, and its objects: other objects let the selection go, another layer the samples too.
    pub fn target(&mut self, key: &(String, String), o: &Objects) {
        let labels = Arc::as_ptr(&o.labels) as usize;
        if self.key != *key || self.labels != labels {
            self.selected.clear();
            self.synced.clear();
        }
        if self.key != *key {
            *self = ObjectsUi { key: key.clone(), labels, ..Default::default() };
        }
        self.labels = labels;
    }

    /// The most samples kept in the model's settings.
    pub fn most(&self) -> usize {
        if self.most == 0 { IN_SETTINGS } else { self.most }
    }

    /// The colour of a class of the palette: the standard palette's by the place of its name in the alphabet (the
    /// order `objects.samples` numbers the classes in, so that the colours are those the result is drawn in).
    pub fn colour(&self, k: usize) -> Color32 {
        let name = &self.classes[k];
        class_colour(self.classes.iter().filter(|c| *c < name).count())
    }

    /// The samples as marks on the map (the cell and the class's colour).
    pub fn marks(&self) -> Vec<((usize, usize), Color32)> {
        self.samples
            .iter()
            .filter(|s| s.class < self.classes.len())
            .map(|s| ((s.row, s.col), self.colour(s.class)))
            .collect()
    }

    /// What the pointer did on the objects of the layer drawn `layer`th; whether something changed.
    pub fn pointer(&mut self, events: &[ObjectEvent], layer: usize, o: &Objects) -> bool {
        let mut changed = false;
        for e in events.iter().filter(|e| e.layer == layer) {
            if self.sampling {
                match e.kind {
                    ObjectEventKind::Click | ObjectEventKind::Drag if e.id != 0 => changed |= self.give(o, e.id),
                    ObjectEventKind::RightClick if e.id != 0 => changed |= self.take(o, e.id),
                    _ => {}
                }
            } else if e.kind == ObjectEventKind::Click {
                if e.id == 0 {
                    if !e.ctrl {
                        changed |= !self.selected.is_empty();
                        self.selected.clear();
                    }
                } else if e.ctrl {
                    if !self.selected.remove(&e.id) {
                        self.selected.insert(e.id);
                    }
                    changed = true;
                } else {
                    self.selected = BTreeSet::from([e.id]);
                    changed = true;
                }
            }
        }
        changed
    }

    /// The samples inside object `id` (indexes).
    fn in_object(&self, o: &Objects, id: u32) -> Vec<usize> {
        let w = o.grid.width;
        (0..self.samples.len())
            .filter(|&i| {
                let s = &self.samples[i];
                s.row < o.grid.height && s.col < w && o.labels[s.row * w + s.col] == id
            })
            .collect()
    }

    /// Give the object the class chosen (a sample at its interior cell nearest its centroid); whether that changed.
    fn give(&mut self, o: &Objects, id: u32) -> bool {
        if self.active >= self.classes.len() {
            self.message = Some("add a class to the palette first (a name, then Add)".into());
            return false;
        }
        let in_it = self.in_object(o, id);
        let mut changed = false;
        if in_it.is_empty() {
            let Some((row, col)) = sample_cell(o, id) else { return false };
            self.samples.push(Sample { row, col, class: self.active });
            changed = true;
        }
        for i in in_it {
            if self.samples[i].class != self.active {
                self.samples[i].class = self.active;
                changed = true;
            }
        }
        if changed {
            self.changed = Some(Instant::now());
            self.message = None;
        }
        changed
    }

    /// Take the object's samples away; whether there were any.
    fn take(&mut self, o: &Objects, id: u32) -> bool {
        let in_it: HashSet<usize> = self.in_object(o, id).into_iter().collect();
        if in_it.is_empty() {
            return false;
        }
        let mut i = 0;
        self.samples.retain(|_| {
            i += 1;
            !in_it.contains(&(i - 1))
        });
        self.changed = Some(Instant::now());
        true
    }

    /// Samples per class: the objects of the current level that have one.
    pub fn counts(&self, o: &Objects) -> Vec<usize> {
        let w = o.grid.width;
        let mut seen: HashSet<(usize, u32)> = HashSet::new();
        for s in &self.samples {
            if s.row < o.grid.height && s.col < w {
                let id = o.labels[s.row * w + s.col];
                if id != 0 {
                    seen.insert((s.class, id));
                }
            }
        }
        (0..self.classes.len()).map(|k| seen.iter().filter(|(c, _)| *c == k).count()).collect()
    }

    /// The selection and the attribute table's agreed on: a row picked in the table is an object selected, an object
    /// selected on the map is its row picked (the table scrolls to it).
    pub fn sync(&mut self, pick: &mut Pick, count: usize) {
        let table: BTreeSet<u32> = pick.rows.iter().filter(|&&r| r < count).map(|&r| r as u32 + 1).collect();
        if table != self.synced {
            self.selected = table;
        } else if self.selected != self.synced {
            pick.rows = self.selected.iter().map(|&i| i as usize - 1).collect();
            pick.row = self.selected.iter().next_back().map(|&i| i as usize - 1);
            pick.anchor = pick.row;
            pick.scroll = true;
            pick.plain = false;
        }
        self.synced = self.selected.clone();
    }

    /// The keys over the objects `o` of `level` (`base`'s, counted up the chain of parents): N adds the neighbours of
    /// the selected, P selects their parents, C their children, Esc lets go. C goes down the chain of parents when the
    /// level shown has a level beneath it there (`level` above 0), else to `below_out`, the `below` output of the
    /// node that made the objects when it has one. What to go to, when the key asks.
    pub fn keys(&mut self, ui: &Ui, base: &Arc<Objects>, level: usize, below_out: Option<&Arc<Objects>>) -> Option<Go> {
        if self.selected.is_empty() || ui.ctx().egui_wants_keyboard_input() {
            return None;
        }
        let pressed = |k: egui::Key| ui.input(|i| i.key_pressed(k) && !i.modifiers.any());
        let o = at_level(base, level);
        if pressed(egui::Key::Escape) {
            self.selected.clear();
        } else if pressed(egui::Key::N) {
            let more: Vec<u32> =
                self.selected.iter().flat_map(|&id| o.neighbours_of(id).iter().map(|&(n, _)| n)).collect();
            self.selected.extend(more);
        } else if pressed(egui::Key::P) {
            let up = o.parent.clone()?;
            let parents: BTreeSet<u32> =
                self.selected.iter().filter_map(|&id| o.parent_id(id)).filter(|&p| p != 0).collect();
            self.selected = parents;
            self.labels = Arc::as_ptr(&up.labels) as usize;
            return Some(Go::Level(level + 1));
        } else if pressed(egui::Key::C) {
            let below = if level > 0 { at_level(base, level - 1) } else { below_out?.clone() };
            let children: BTreeSet<u32> = (1..=below.count as u32)
                .filter(|&c| below.parent_id(c).is_some_and(|p| self.selected.contains(&p)))
                .collect();
            self.selected = children;
            self.labels = Arc::as_ptr(&below.labels) as usize;
            if level == 0 {
                self.key.1 = "below".into();
                return Some(Go::Below);
            }
            return Some(Go::Level(level - 1));
        }
        None
    }

    /// The inspector: the object selected (the last of them) and what decided its class. `below` is the level
    /// beneath the one shown, when the chain of parents has it.
    pub fn inspector(&mut self, ui: &mut Ui, s: &Session, o: &Objects, below: Option<&Objects>) {
        let Some(&id) = self.selected.iter().next_back().filter(|&&i| i != 0 && i as usize <= o.count) else {
            ui.label(
                RichText::new("click an object to inspect it (N: its neighbours, P: its parent, C: its children, Esc)")
                    .weak()
                    .size(11.0),
            );
            crate::series_view::clear(ui.ctx());
            self.edit_block(ui);
            return;
        };
        let k = id as usize - 1;
        ui.label(RichText::new(object_text(o, id)).strong());
        if self.selected.len() > 1 {
            ui.label(
                RichText::new(format!("{} objects selected, the last one told", self.selected.len())).weak().size(11.0),
            );
        }
        let margin = margin_of(s, &self.key, o, id);
        if let Some(m) = margin {
            ui.label(format!("margin {m:.3} (the largest membership minus the next)"));
        }
        let mut family = Vec::new();
        if let Some(p) = o.parent_id(id) {
            family.push(format!("parent {p}"));
        }
        if let Some(b) = below.filter(|b| b.parent.is_some()) {
            let n = (1..=b.count as u32).filter(|&c| b.parent_id(c) == Some(id)).count();
            family.push(format!("{n} children"));
        }
        family.push(format!("{} neighbours", o.neighbours_of(id).len()));
        ui.label(RichText::new(family.join(" · ")).weak().size(11.0));
        // the membership to every class, as bars in the class's colour
        if !o.membership.is_empty() {
            ui.add_space(4.0);
            ui.label(RichText::new("Membership").size(11.5));
            for (c, class) in o.classes.iter().enumerate() {
                let m = o.membership[c][k].clamp(0.0, 1.0);
                ui.horizontal(|ui| {
                    let won = o.class_of[k] as usize == c + 1;
                    let (r, _) = ui.allocate_exact_size(egui::vec2(120.0, 11.0), egui::Sense::hover());
                    ui.painter().rect_filled(r, 2.0, ui.visuals().faint_bg_color);
                    let mut part = r;
                    part.set_width(r.width() * m as f32);
                    ui.painter().rect_filled(part, 2.0, class_colour_of(o, c + 1));
                    let text = RichText::new(format!("{} {m:.2}", class.name)).size(11.0);
                    ui.label(if won { text.strong() } else { text });
                });
            }
        }
        self.decided(ui, s, o, id);
        // the profile of the object (and of a second one, ctrl+click) when the objects carry a time series
        let second = self.selected.iter().rev().nth(1).copied().filter(|&i| i != 0 && i as usize <= o.count);
        crate::series_view::profile(ui, s, o, id, second);
        // the attributes (the table's row)
        ui.add_space(4.0);
        egui::CollapsingHeader::new(RichText::new("Attributes").size(11.5)).default_open(true).show(ui, |ui| {
            egui::Grid::new("object attributes").num_columns(2).spacing([10.0, 2.0]).show(ui, |ui| {
                for (j, (name, v)) in o.table.columns.iter().zip(&o.table.rows[k]).enumerate().skip(1) {
                    ui.label(RichText::new(name).weak().size(11.0));
                    ui.label(RichText::new(crate::table_view::cell_of(&o.table, j + 1, *v)).size(11.0));
                    ui.end_row();
                }
            });
        });
        self.edit_block(ui);
        self.similar_block(ui, s, o);
    }

    /// The objects the threshold of *Find similar* lights (not the selected ones): none while it is off, or the
    /// selection is not the one it was made for.
    pub fn lit(&self) -> Vec<u32> {
        let f = &self.finding;
        match &f.made {
            Some(m) if f.on && m.examples.iter().copied().eq(self.selected.iter().copied()) => {
                terraflux_ops::objects::similar::above(&m.similarity, &m.examples, f.threshold)
            }
            _ => Vec::new(),
        }
    }

    /// *Find similar*: the similarity of every object to the selected ones (the same function as objects.similar:
    /// the embeddings when the objects have them, else the standardised attributes), a threshold that lights the
    /// objects on the map as it moves, and *Label all as <class>*, which gives the lit objects and the selected ones the
    /// class as samples and writes them in one step of undo (as *Apply* does).
    fn similar_block(&mut self, ui: &mut Ui, s: &Session, o: &Objects) {
        ui.add_space(4.0);
        let was = self.finding.on;
        ui.checkbox(&mut self.finding.on, "Find similar").on_hover_text(
            "the objects whose attributes look like those of the selected ones, lit on the map above a threshold",
        );
        if !self.finding.on {
            if was {
                self.finding.made = None;
            }
            return;
        }
        let labels = Arc::as_ptr(&o.labels) as usize;
        let examples: Vec<u32> = self.selected.iter().copied().filter(|&i| i != 0 && i as usize <= o.count).collect();
        let stale = !matches!(&self.finding.made, Some(m)
            if m.labels == labels && m.cosine == self.finding.cosine && m.examples == examples);
        if stale && !examples.is_empty() {
            let metric = if self.finding.cosine {
                terraflux_ops::objects::similar::Metric::Cosine
            } else {
                terraflux_ops::objects::similar::Metric::Euclidean
            };
            match terraflux_ops::objects::similar::similarity(o, &examples, None, metric) {
                Ok(f) => {
                    self.finding.error = None;
                    self.finding.made = Some(Made {
                        labels,
                        cosine: self.finding.cosine,
                        examples: examples.clone(),
                        similarity: f.similarity,
                        used: f.used,
                    });
                }
                Err(e) => {
                    self.finding.made = None;
                    self.finding.error = Some(e);
                }
            }
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.finding.cosine, true, "cosine");
            ui.selectable_value(&mut self.finding.cosine, false, "euclidean");
            ui.add(egui::Slider::new(&mut self.finding.threshold, 0.0..=1.0).text("threshold").fixed_decimals(2));
        });
        if let Some(e) = &self.finding.error {
            ui.label(RichText::new(e).color(ui.visuals().warn_fg_color).size(11.0));
        }
        let lit = self.lit();
        if let Some(m) = &self.finding.made {
            ui.label(
                RichText::new(format!("{} objects like the {} selected, on {}", lit.len(), m.examples.len(), m.used))
                    .weak()
                    .size(11.0),
            );
        }
        ui.horizontal(|ui| {
            let typed = self.new_class.trim().to_string();
            ui.add(egui::TextEdit::singleline(&mut self.new_class).hint_text("class").desired_width(90.0));
            let name = if typed.is_empty() { self.classes.get(self.active).cloned() } else { Some(typed.clone()) };
            let text = format!("Label all as {}", name.as_deref().unwrap_or("..."));
            let go = ui
                .add_enabled(name.is_some() && !lit.is_empty(), egui::Button::new(text))
                .on_hover_text(
                    "the lit objects and the selected ones become samples of the class, written into the model",
                )
                .clicked();
            if go && let Some(name) = name {
                self.label_all(s, o, &name, &lit);
            }
        });
    }

    /// The class `name` (made when the palette lacks it) given as samples to the objects `lit` and the selected ones;
    /// the samples are written at the next frame (one step of undo, as *Apply*).
    pub fn label_all(&mut self, s: &Session, o: &Objects, name: &str, lit: &[u32]) {
        if !self.sampling {
            self.prefill(s, o);
        }
        self.active = match self.classes.iter().position(|c| c == name) {
            Some(k) => k,
            None => {
                self.classes.push(name.to_string());
                self.classes.len() - 1
            }
        };
        self.new_class.clear();
        let mut ids: Vec<u32> = self.selected.iter().copied().collect();
        ids.extend_from_slice(lit);
        ids.sort_unstable();
        ids.dedup();
        let mut given = 0;
        for id in ids {
            given += usize::from(self.give(o, id));
        }
        self.write_now = given > 0;
        self.message = Some(format!("{given} objects given the class {name}"));
    }

    /// The *Edit* block: Merge (two objects selected or more) and Delete act on the selection, Cut asks for a line
    /// drawn on the map (clicks, then Enter). What is asked is left in `pending` (or `cutting`) for the Viewer.
    fn edit_block(&mut self, ui: &mut Ui) {
        ui.add_space(4.0);
        ui.label(RichText::new("Edit").size(11.5)).on_hover_text(
            "corrections kept in an objects.edit node of the model, as places on the map, so that they hold when the segmentation above changes",
        );
        let n = self.selected.len();
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(n >= 2, egui::Button::new("Merge"))
                .on_hover_text("the selected objects become one (select two or more: ctrl+click)")
                .clicked()
            {
                self.pending = Some(crate::objects_edit::Edit::Merge(self.selected.iter().copied().collect()));
            }
            if ui
                .add_enabled(n >= 1, egui::Button::new("Delete"))
                .on_hover_text("the selected objects are left with no object, or taken by the most similar neighbour")
                .clicked()
            {
                self.pending = Some(crate::objects_edit::Edit::Delete(
                    self.selected.iter().copied().collect(),
                    self.into_neighbour,
                ));
            }
            ui.checkbox(&mut self.into_neighbour, "into a neighbour")
                .on_hover_text("a deleted object joins its most similar neighbour (else it has no object)");
            let text = if self.cutting { "Cancel cut" } else { "Cut" };
            if ui
                .add(egui::Button::selectable(self.cutting, text))
                .on_hover_text(
                    "draw a line on the map (a click a vertex, Enter ends it): the objects it crosses are split",
                )
                .clicked()
            {
                self.cutting = !self.cutting;
            }
        });
        if self.cutting {
            ui.label(RichText::new("click the vertices of the line on the map, then Enter").weak().size(11.0));
        }
        if !self.sampling
            && let Some(m) = &self.message
        {
            ui.label(RichText::new(m).color(ui.visuals().warn_fg_color).size(11.0));
        }
    }

    /// What decided the class: by the node that made the objects, when it is a node of the top level whose inputs can
    /// be read. Nothing when it is not known.
    fn decided(&mut self, ui: &mut Ui, s: &Session, o: &Objects, id: u32) {
        let Some(node) = s.source.nodes.iter().find(|n| n.id == self.key.0) else { return };
        let k = id as usize - 1;
        let small = |t: String| RichText::new(t).size(11.0);
        match node.op.as_str() {
            "objects.rules" if !o.membership.is_empty() => {
                let Some(given) = node.params.get("classes") else { return };
                let Ok(tree) = terraflux_ops::objects::tree(given) else { return };
                let Some(Value::Objects(input)) = input_value(s, node, "objects") else { return };
                let below = match input_value(s, node, "below") {
                    Some(Value::Objects(b)) => Some(b),
                    _ => None,
                };
                let key = {
                    let mut h = std::collections::hash_map::DefaultHasher::new();
                    given.to_string().hash(&mut h);
                    (Arc::as_ptr(&input.labels) as usize, below.as_ref().map(|b| Arc::as_ptr(&b.labels) as usize))
                        .hash(&mut h);
                    h.finish()
                };
                if self.rules.as_ref().is_none_or(|r| r.0 != key) {
                    self.rules = Some((key, rule_values(&input, below.as_deref(), &tree)));
                }
                ui.add_space(4.0);
                ui.label(RichText::new("What decided").size(11.5));
                let values = &self.rules.as_ref().unwrap().1;
                for (c, class) in tree.iter().enumerate() {
                    let won = o.class_of[k] as usize == c + 1;
                    let own = values.get(c).and_then(|v| v.as_ref()).map(|v| v[k]);
                    let text = match (&class.rule, own) {
                        (Some(rule), Some(v)) => format!("{}: {rule} = {v:.3}", class.name),
                        _ => format!("{}: no rule", class.name),
                    };
                    let total = o.membership.get(c).map_or(f64::NAN, |m| m[k]);
                    let up = class.parent.map(|p| format!(", inside {}", tree[p].name)).unwrap_or_default();
                    let line = format!("{text} (membership {total:.3}{up}){}", if won { "  <- decided" } else { "" });
                    ui.label(if won { small(line).strong() } else { small(line) });
                }
            }
            "objects.classify" if !o.membership.is_empty() => {
                let mut top: Vec<(usize, f64)> = (0..o.classes.len()).map(|c| (c, o.membership[c][k])).collect();
                top.sort_by(|a, b| b.1.total_cmp(&a.1));
                ui.add_space(4.0);
                ui.label(RichText::new("What decided").size(11.5));
                let said: Vec<String> =
                    top.iter().take(3).map(|(c, p)| format!("{} {:.0}%", o.classes[*c].name, p * 100.0)).collect();
                ui.label(small(format!("the forest's votes: {}", said.join(", "))));
            }
            "objects.nearest" if !o.membership.is_empty() => {
                // (the membership of a class is exp(-k d^2), k = ln(1 / slope): the distance of its nearest sample)
                let slope = node.params.get("slope").and_then(|v| v.as_f64()).unwrap_or(0.2);
                if !(slope > 0.0 && slope < 1.0) {
                    return;
                }
                let kk = (1.0 / slope).ln();
                ui.add_space(4.0);
                ui.label(RichText::new("What decided").size(11.5));
                for (c, class) in o.classes.iter().enumerate() {
                    let m = o.membership[c][k];
                    if m > 0.0 {
                        let d = (-m.ln() / kk).max(0.0).sqrt();
                        let won = o.class_of[k] as usize == c + 1;
                        let line = format!("nearest sample of {}: distance {d:.2}", class.name);
                        ui.label(if won { small(line).strong() } else { small(line) });
                    }
                }
            }
            _ => {}
        }
    }

    /// The samples mode's controls: the palette (with the counts and the warnings), a new class, Apply and Live.
    /// Whether *Apply* was clicked, or the live mode has waited long enough.
    pub fn samples_ui(&mut self, ui: &mut Ui, s: &Session, o: &Objects) -> bool {
        let was = self.sampling;
        ui.checkbox(&mut self.sampling, "Samples").on_hover_text(
            "click an object to give it the class chosen, drag to paint several, right click to take it away",
        );
        if self.sampling && !was {
            self.prefill(s, o);
        }
        if !self.sampling {
            return false;
        }
        let counts = self.counts(o);
        for (k, &n) in counts.iter().enumerate() {
            ui.horizontal(|ui| {
                let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                ui.painter().rect_filled(r, 2.0, self.colour(k));
                let text = format!("{} ({})", self.classes[k], n);
                if ui.add(egui::Button::selectable(self.active == k, text)).clicked() {
                    self.active = k;
                }
                if n < FEW {
                    ui.label(RichText::new("few").color(ui.visuals().warn_fg_color).size(11.0))
                        .on_hover_text(format!("fewer than {FEW} samples: a classifier needs more of each class"));
                }
            });
        }
        let add = ui
            .horizontal(|ui| {
                let e =
                    ui.add(egui::TextEdit::singleline(&mut self.new_class).hint_text("new class").desired_width(110.0));
                let go = ui.add_enabled(!self.new_class.trim().is_empty(), egui::Button::new("Add")).clicked();
                go || (e.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            })
            .inner;
        if add && !self.new_class.trim().is_empty() {
            let name = self.new_class.trim().to_string();
            if let Some(k) = self.classes.iter().position(|c| *c == name) {
                self.active = k;
            } else {
                self.classes.push(name);
                self.active = self.classes.len() - 1;
            }
            self.new_class.clear();
        }
        let mut apply = false;
        ui.horizontal(|ui| {
            apply = ui
                .add_enabled(!self.samples.is_empty(), egui::Button::new("Apply"))
                .on_hover_text("the samples written into the model: a table of points feeding objects.samples")
                .clicked();
            ui.checkbox(&mut self.live, "Live").on_hover_text("written after a pause, as they are given");
        });
        if let Some(m) = &self.message {
            ui.label(RichText::new(m).color(ui.visuals().warn_fg_color).size(11.0));
        }
        if let Some(t) = self.changed.filter(|_| self.live && !self.samples.is_empty()) {
            if t.elapsed() >= DEBOUNCE {
                apply = true;
            } else {
                ui.ctx().request_repaint_after(DEBOUNCE - t.elapsed());
            }
        }
        apply
    }

    /// Entering samples mode: the classes of the `objects.samples` node of these objects, and (when its points are
    /// ours) its samples.
    fn prefill(&mut self, s: &Session, o: &Objects) {
        let Some(sn) = samples_node(&s.source, &self.key) else { return };
        let node = &s.source.nodes[sn];
        if self.classes.is_empty()
            && let Some(Value::Objects(r)) = s.value_of(&node.id, "objects")
        {
            self.classes = r.classes.iter().map(|c| c.name.clone()).collect();
        }
        if !self.samples.is_empty() {
            return;
        }
        let Some((points, Some(table))) = ours_of(&s.source, node) else { return };
        let _ = points;
        let Some(t) = s.source.nodes.iter().find(|n| n.id == table) else { return };
        let rows: Vec<Vec<serde_json::Value>> =
            t.params.get("rows").and_then(|r| serde_json::from_value(r.clone()).ok()).unwrap_or_default();
        for r in rows {
            let (Some(x), Some(y), Some(name)) = (
                r.first().and_then(|v| v.as_f64()),
                r.get(1).and_then(|v| v.as_f64()),
                r.get(2).and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            let Some((row, col)) = cell_at(&o.grid, x, y) else { continue };
            let class = match self.classes.iter().position(|c| c == name) {
                Some(k) => k,
                None => {
                    self.classes.push(name.to_string());
                    self.classes.len() - 1
                }
            };
            self.samples.push(Sample { row, col, class });
        }
    }
}

/// The cell (row, column) of a place on a grid (north up).
fn cell_at(g: &terraflux_raster::Grid, x: f64, y: f64) -> Option<(usize, usize)> {
    let t = g.transform;
    if t[2] != 0.0 || t[4] != 0.0 || t[1] == 0.0 || t[5] == 0.0 {
        return None;
    }
    let (c, r) = (((x - t[0]) / t[1]).floor(), ((y - t[3]) / t[5]).floor());
    (c >= 0.0 && r >= 0.0 && (c as usize) < g.width && (r as usize) < g.height).then_some((r as usize, c as usize))
}

/// The place of the centre of a cell.
pub(crate) fn centre_of(g: &terraflux_raster::Grid, row: usize, col: usize) -> [f64; 2] {
    let t = g.transform;
    let (c, r) = (col as f64 + 0.5, row as f64 + 0.5);
    [t[0] + c * t[1] + r * t[2], t[3] + c * t[4] + r * t[5]]
}

/// The cell where an object gets its sample: of the cells of the object that are not on its border (all four
/// neighbours are the object's), the one nearest its centroid; an object with no such cell, its cell nearest the
/// centroid. None for no object.
pub fn sample_cell(o: &Objects, id: u32) -> Option<(usize, usize)> {
    let b = *o.boxes.get((id as usize).checked_sub(1)?)?;
    let (w, h) = (o.grid.width, o.grid.height);
    let is = |r: usize, c: usize| o.labels[r * w + c] == id;
    let (mut n, mut sr, mut sc) = (0.0f64, 0.0f64, 0.0f64);
    for r in b[0] as usize..=b[2] as usize {
        for c in b[1] as usize..=b[3] as usize {
            if is(r, c) {
                n += 1.0;
                sr += r as f64;
                sc += c as f64;
            }
        }
    }
    if n == 0.0 {
        return None;
    }
    let (mr, mc) = (sr / n, sc / n);
    // (interior, any): the nearest of each
    let mut best: [Option<(f64, usize, usize)>; 2] = [None, None];
    for r in b[0] as usize..=b[2] as usize {
        for c in b[1] as usize..=b[3] as usize {
            if !is(r, c) {
                continue;
            }
            let d = (r as f64 - mr).powi(2) + (c as f64 - mc).powi(2);
            let inside = r > 0
                && c > 0
                && r + 1 < h
                && c + 1 < w
                && is(r - 1, c)
                && is(r + 1, c)
                && is(r, c - 1)
                && is(r, c + 1);
            for (k, ok) in [(0, inside), (1, true)] {
                if ok && best[k].is_none_or(|b| d < b.0) {
                    best[k] = Some((d, r, c));
                }
            }
        }
    }
    best[0].or(best[1]).map(|(_, r, c)| (r, c))
}

/// The value a node's input reads (`node.port`), when it is one of the model's.
fn input_value(s: &Session, n: &Node, port: &str) -> Option<Value> {
    match n.inputs.get(port)? {
        Source::Ref(r) => {
            let (node, p) = r.split_once('.')?;
            s.value_of(node, p)
        }
        Source::Literal(_) => None,
    }
}

/// The value of each class's rule over the objects (0 to 1, a missing value 0), none for a class with no rule.
fn rule_values(
    input: &Objects,
    below: Option<&Objects>,
    tree: &[terraflux_ops::objects::RuleClass],
) -> Vec<Option<Vec<f64>>> {
    tree.iter()
        .map(|c| {
            let script = terraflux_ops::algebra::parse_script_objects(c.rule.as_ref()?).ok()?;
            let (_, v) = terraflux_ops::objects::evaluate_numbers(input, below, &script).ok()?.pop()?;
            Some(v.into_iter().map(|x| if x.is_nan() { 0.0 } else { x.clamp(0.0, 1.0) }).collect())
        })
        .collect()
}

/// The margin of an object: the one the node that made the classes wrote in its table when it is there, else the
/// largest membership minus the next one.
pub fn margin_of(s: &Session, key: &(String, String), o: &Objects, id: u32) -> Option<f64> {
    if o.membership.is_empty() {
        return None;
    }
    let k = id as usize - 1;
    if let Some(Value::Table(t)) = s.value_of(&key.0, "table")
        && t.rows.len() == o.count
        && let Some(m) = t.column("margin")
    {
        return Some(m[k]);
    }
    Some(margin_in(o, id))
}

/// The largest membership of an object minus the next one (over every class).
pub fn margin_in(o: &Objects, id: u32) -> f64 {
    let k = id as usize - 1;
    let mut m: Vec<f64> = o.membership.iter().map(|c| c[k]).collect();
    m.sort_by(|a, b| b.total_cmp(a));
    m.first().copied().unwrap_or(0.0) - m.get(1).copied().unwrap_or(0.0)
}

/// The `objects.samples` node of the top level that reads the objects `key` (its place).
pub fn samples_node(m: &Model, key: &(String, String)) -> Option<usize> {
    let want = format!("{}.{}", key.0, key.1);
    m.nodes.iter().position(|n| {
        n.op == "objects.samples" && matches!(n.inputs.get("objects"), Some(Source::Ref(r)) if *r == want)
    })
}

/// The nodes of ours that feed a node of `objects.samples`: the points' (`vector.from_table`, or `vector.read` of a
/// GeoPackage) and the table's (`table.of`) when there is one. None when its samples come from elsewhere.
fn ours_of(m: &Model, samples: &Node) -> Option<(String, Option<String>)> {
    let Some(Source::Ref(r)) = samples.inputs.get("samples") else { return None };
    let (points, _) = r.split_once('.')?;
    let p = m.nodes.iter().find(|n| n.id == points)?;
    match (p.op.as_str(), p.inputs.get("table")) {
        // points written in the model as a table of x, y and class (ours, or a template's: the same form)
        ("vector.from_table", Some(Source::Ref(t))) => {
            let (table, _) = t.split_once('.')?;
            let xyc = serde_json::json!(["x", "y", "class"]);
            m.nodes
                .iter()
                .any(|n| {
                    n.id == table
                        && n.op == "table.of"
                        && (n.id.starts_with("sample_table") || n.params.get("columns") == Some(&xyc))
                })
                .then(|| (p.id.clone(), Some(table.to_string())))
        }
        // a file only when it is ours (a GeoPackage written beside the model): never the user's own
        ("vector.read", _) if p.id.starts_with(POINTS) => Some((p.id.clone(), None)),
        _ => None,
    }
}

/// The samples as the points written: x, y and the class's name, one for each object (a cell of no object, or off
/// the grid, is left out).
fn points(st: &ObjectsUi, o: &Objects) -> Vec<(f64, f64, String)> {
    let w = o.grid.width;
    let mut seen: HashSet<u32> = HashSet::new();
    let mut out = Vec::new();
    for s in &st.samples {
        if s.row >= o.grid.height || s.col >= w || s.class >= st.classes.len() {
            continue;
        }
        let id = o.labels[s.row * w + s.col];
        if id == 0 || !seen.insert(id) {
            continue;
        }
        let [x, y] = centre_of(&o.grid, s.row, s.col);
        out.push((x, y, st.classes[s.class].clone()));
    }
    out
}

/// *Apply*: the samples written into the model, as one step of undo. The points are a `table.of` (columns x, y and
/// class) feeding a `vector.from_table` of the objects' coordinate system, or (more than [`IN_SETTINGS`] points) a
/// GeoPackage `samples/<name>.gpkg` beside the model read by a `vector.read`; they feed the `samples` port of the
/// model's `objects.samples` of these objects, which is made with `ml.fit` (column class) and `objects.classify`
/// after it when the model has none. A message of what was done.
pub fn apply(s: &mut Session, r: &Registry, st: &ObjectsUi, o: &Objects) -> Result<String, String> {
    let (node, port) = st.key.clone();
    if !s.source.nodes.iter().any(|n| n.id == node) {
        return Err(format!("{node} is not a node of the model's top level: no samples can feed from it"));
    }
    let pts = points(st, o);
    if pts.is_empty() {
        return Err("no samples on an object".into());
    }
    let before = s.source.clone();
    match write(s, r, (&node, &port), &pts, &o.grid, st.most()) {
        Ok(msg) => {
            s.history.name(&format!("{} samples of objects", pts.len()));
            s.edited();
            Ok(msg)
        }
        Err(e) => {
            s.source = before;
            Err(e)
        }
    }
}

fn node_of<'a>(m: &'a mut Model, id: &str) -> Result<&'a mut Node, String> {
    m.nodes.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("no node {id}"))
}

fn write(
    s: &mut Session,
    r: &Registry,
    (node, port): (&str, &str),
    pts: &[(f64, f64, String)],
    grid: &terraflux_raster::Grid,
    most: usize,
) -> Result<String, String> {
    let top: &[String] = &[];
    let from = format!("{node}.{port}");
    let big = pts.len() > most;
    let crs = terraflux_ops::prepare::crs_of(grid);
    let sn = samples_node(&s.source, &(node.to_string(), port.to_string())).map(|k| s.source.nodes[k].id.clone());
    let ours =
        sn.as_ref().and_then(|id| s.source.nodes.iter().find(|n| n.id == *id)).and_then(|n| ours_of(&s.source, n));
    let pos = |s: &Session| crate::builder::beside(&s.source.nodes, Some(node));
    let mut made: Vec<String> = Vec::new();
    // the GeoPackage of a large set: the file the points node reads already, else a new one beside the model
    let mut file: Option<std::path::PathBuf> = None;
    if big {
        let base = s.base();
        if base.as_os_str().is_empty() {
            return Err(format!(
                "{} samples are more than {} (kept in the model): save the model first, they go to a GeoPackage beside it",
                pts.len(),
                most
            ));
        }
        let old = ours.as_ref().and_then(|(p, _)| s.source.nodes.iter().find(|n| n.id == *p)).and_then(|n| {
            (n.op == "vector.read")
                .then(|| n.params.get("path").and_then(|p| p.as_str()).map(std::path::PathBuf::from))?
        });
        let path = old.unwrap_or_else(|| {
            let dir = base.join("samples");
            let mut p = dir.join(format!("{node}_samples.gpkg"));
            let mut k = 2;
            while p.exists() {
                p = dir.join(format!("{node}_samples_{k}.gpkg"));
                k += 1;
            }
            p
        });
        if let Some(d) = path.parent() {
            std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
        }
        let v = Vector {
            geometries: pts.iter().map(|p| Geometry::Point([p.0, p.1])).collect(),
            attributes: vec![("class".to_string(), Column::Texts(pts.iter().map(|p| p.2.clone()).collect()))],
            crs: crs.as_ref().map(|c| terraflux_gdal::crs_wkt(c).unwrap_or_else(|_| c.clone())),
        };
        let layer = path.file_stem().map_or("samples".into(), |x| x.to_string_lossy().into_owned());
        let _ = std::fs::remove_file(&path);
        terraflux_gdal::write_vector(&path, &layer, &v)?;
        file = Some(path);
    }
    // the points' node
    let p_id = match &ours {
        Some((p, _)) => p.clone(),
        None => {
            let at = pos(s);
            let op = if big { "vector.read" } else { "vector.from_table" };
            let id = crate::editor::add_node_named(&mut s.source, r, top, op, Some(POINTS), at)?;
            made.push(id.clone());
            id
        }
    };
    {
        let p = node_of(&mut s.source, &p_id)?;
        p.params.clear();
        p.inputs.remove("table");
        p.wires.remove("table");
        match &file {
            Some(path) => {
                p.op = "vector.read".into();
                p.params.insert("path".into(), path.to_string_lossy().into_owned().into());
            }
            None => {
                p.op = "vector.from_table".into();
                p.params.insert("x".into(), "x".into());
                p.params.insert("y".into(), "y".into());
                if let Some(c) = &crs {
                    p.params.insert("crs".into(), c.clone().into());
                }
                p.note = Some("The samples given by clicking on objects in the Viewer: a point in each object.".into());
            }
        }
    }
    // the table in the settings (small sets), or none (the file instead)
    let old_table = ours.as_ref().and_then(|(_, t)| t.clone());
    if file.is_none() {
        let t_id = match old_table {
            Some(t) => t,
            None => {
                let at = pos(s);
                let id = crate::editor::add_node_named(&mut s.source, r, top, "table.of", Some("sample_table"), at)?;
                made.push(id.clone());
                id
            }
        };
        let t = node_of(&mut s.source, &t_id)?;
        t.params.insert("columns".into(), serde_json::json!(["x", "y", "class"]));
        let rows: Vec<serde_json::Value> = pts.iter().map(|p| serde_json::json!([p.0, p.1, p.2])).collect();
        t.params.insert("rows".into(), rows.into());
        t.note = Some(format!(
            "{} samples given by clicking on objects in the Viewer (the cell centre of each, and its class).",
            pts.len()
        ));
        crate::editor::connect(&mut s.source, r, top, &format!("{t_id}.table"), &p_id, "table")?;
    } else if let Some(t) = old_table {
        s.source.nodes.retain(|n| n.id != t);
    }
    let points_ref = format!("{p_id}.vector");
    match sn {
        Some(sn) => {
            crate::editor::connect(&mut s.source, r, top, &points_ref, &sn, "samples")?;
        }
        None => {
            let at = pos(s);
            let sn = crate::editor::add_node_named(&mut s.source, r, top, "objects.samples", Some("samples"), at)?;
            crate::editor::connect(&mut s.source, r, top, &from, &sn, "objects")?;
            crate::editor::connect(&mut s.source, r, top, &points_ref, &sn, "samples")?;
            let at = pos(s);
            let fit = crate::editor::add_node_named(&mut s.source, r, top, "ml.fit", Some("fit"), at)?;
            crate::editor::connect(&mut s.source, r, top, &format!("{sn}.table"), &fit, "table")?;
            crate::editor::set_param(&mut s.source, top, &fit, "column", "class")?;
            let at = pos(s);
            let cls = crate::editor::add_node_named(&mut s.source, r, top, "objects.classify", Some("classified"), at)?;
            crate::editor::connect(&mut s.source, r, top, &from, &cls, "objects")?;
            crate::editor::connect(&mut s.source, r, top, &format!("{fit}.model"), &cls, "model")?;
            crate::editor::connect(&mut s.source, r, top, &format!("{sn}.table"), &cls, "samples")?;
            made.extend([sn, fit, cls]);
        }
    }
    let kind = match &file {
        Some(p) => format!("a GeoPackage ({})", p.display()),
        None => "the model's settings".to_string(),
    };
    Ok(format!(
        "{} samples written in {kind}{}",
        pts.len(),
        if made.is_empty() { String::new() } else { format!("; new nodes: {}", made.join(", ")) }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_raster::Grid;

    /// 8 x 4 cells: the left half object 1 (4 columns), the right half object 2.
    fn two() -> Objects {
        let labels: Vec<u32> = (0..32).map(|i| if i % 8 < 4 { 1 } else { 2 }).collect();
        Objects::from_labels(Arc::new(Grid::new(8, 4)), labels, 2).unwrap()
    }

    #[test]
    fn a_sample_is_an_interior_cell_near_the_centroid() {
        let o = two();
        // object 1: columns 0 to 3, rows 0 to 3; its interior cells are rows 1-2, columns 1-2
        let (r, c) = sample_cell(&o, 1).unwrap();
        assert!((1..=2).contains(&r) && (1..=2).contains(&c), "{r},{c}");
        let (r, c) = sample_cell(&o, 2).unwrap();
        assert!((1..=2).contains(&r) && (5..=6).contains(&c), "{r},{c}");
        assert!(sample_cell(&o, 3).is_none());
    }

    #[test]
    fn the_samples_are_given_changed_and_taken_away() {
        let o = two();
        let mut st = ObjectsUi { classes: vec!["b".into(), "a".into()], ..Default::default() };
        let click = |id, kind| ObjectEvent { layer: 0, id, kind, ctrl: false };
        st.sampling = true;
        assert!(st.pointer(&[click(1, ObjectEventKind::Click)], 0, &o));
        // (the same class again: nothing changed)
        assert!(!st.pointer(&[click(1, ObjectEventKind::Drag)], 0, &o));
        st.active = 1;
        assert!(st.pointer(&[click(2, ObjectEventKind::Drag), click(1, ObjectEventKind::Click)], 0, &o));
        assert_eq!(st.samples.len(), 2, "one sample an object");
        assert_eq!(st.counts(&o), vec![0, 2]);
        assert!(st.pointer(&[click(2, ObjectEventKind::RightClick)], 0, &o));
        assert_eq!(st.samples.len(), 1);
        // the colours follow the alphabet: a is the first of the palette
        assert_eq!(st.colour(1), class_colour(0));
        assert_eq!(st.colour(0), class_colour(1));
    }

    #[test]
    fn the_table_and_the_map_select_alike() {
        let mut st = ObjectsUi::default();
        let mut pick = Pick::default();
        st.selected.insert(2);
        st.sync(&mut pick, 2);
        assert_eq!(pick.rows.iter().copied().collect::<Vec<_>>(), vec![1]);
        // a row clicked in the table: the object selected
        pick.rows = [0usize].into();
        st.sync(&mut pick, 2);
        assert_eq!(st.selected.iter().copied().collect::<Vec<_>>(), vec![1]);
    }

    #[test]
    fn a_templates_points_are_the_samples_of_the_viewer() {
        let m = crate::templates::TEMPLATES
            .iter()
            .find(|t| t.name == "Classify an image by clicking samples")
            .expect("the template")
            .model();
        let sn = m.nodes.iter().find(|n| n.op == "objects.samples").expect("objects.samples");
        assert_eq!(ours_of(&m, sn), Some(("samples".to_string(), Some("sample_table".to_string()))));
    }
}
