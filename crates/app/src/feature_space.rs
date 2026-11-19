//! The feature space of objects: where the samples of each class lie among the attributes, which attributes tell the
//! classes apart, a rule from a threshold, and where to give a sample next.
//!
//! A window opened from the Viewer's side panel (*Feature space...*) for the objects shown:
//! - a scatter of two attributes: the objects with a sample in the colour of their class, the others grey and faint,
//!   the selected ringed; a click on a point selects its object (on the map too);
//! - the histogram of one attribute per class, overlaid, where a range dragged lights up the objects inside it (a
//!   selection) and *Make a rule* writes it into the model's `objects.rules` as a fuzzy rule of the class;
//! - the separability of the classes: the Jeffries-Matusita distance of each pair of classes by attribute, and the
//!   attributes that separate most (each a button to use it as the axes);
//! - after a classification, the objects whose class is least certain (*Most uncertain*), and the errors the
//!   validation found, each leading the map to the object.

use std::collections::{BTreeSet, HashSet};

use egui::{Color32, Id, RichText, Ui};
use egui_plot::{Bar, BarChart, Plot, PlotPoint, Points, Polygon, VLine};
use terraflux_graph::{Model, Source};
use terraflux_ops::Table;
use terraflux_ops::objects::Objects;
use terraflux_ops::{Registry, Value};

use crate::app::Session;
use crate::objects_inspect::{ObjectsUi, margin_in, samples_node};
use crate::palette::class_colour;

/// The attributes listed as separating most.
pub const TOP: usize = 5;
/// The objects the list of the most uncertain has.
pub const UNCERTAIN: usize = 20;
/// The bins of the histogram.
const BINS: usize = 20;
/// The most objects without a sample the scatter draws (the others are thinned out).
const MAX_GREY: usize = 20_000;
/// The fuzzy border of a rule made from a range: this part of the attribute's range on each side.
const BORDER: f64 = 0.1;
/// How near (in points of the screen) a click must be to a point of the scatter.
const NEAR: f32 = 9.0;

/// What the feature space keeps.
#[derive(Default)]
pub struct Space {
    pub open: bool,
    /// The attributes of the scatter and the histogram (columns of the objects' table; 0 none yet).
    pub x: usize,
    pub y: usize,
    pub hist: usize,
    /// The range of the histogram's attribute (no limit on the side that is none).
    pub lo: Option<f64>,
    pub hi: Option<f64>,
    /// The class a rule is made for.
    pub class: String,
    /// Where a drag on the histogram began.
    anchor: Option<f64>,
    /// The range whose objects are selected (the attribute and the limits).
    applied: Option<(usize, Option<f64>, Option<f64>)>,
    pub message: Option<String>,
}

/// What the window asks of the Viewer: the rule to write (class, text), the object to go to on the map.
#[derive(Default)]
pub struct SpaceAction {
    pub rule: Option<(String, String)>,
    pub go: Option<u32>,
}

/// A class with samples: its name, colour and the objects that hold them (ids).
struct Group {
    name: String,
    colour: Color32,
    ids: Vec<u32>,
}

/// The separability of the classes (pairs of groups) by attribute.
pub struct Separability {
    /// The attributes (columns of the table).
    pub columns: Vec<usize>,
    /// The pairs of groups.
    pub pairs: Vec<(usize, usize)>,
    /// The distance by attribute (as `columns`) and pair (as `pairs`); none where a class has fewer than two values.
    pub jm: Vec<Vec<Option<f64>>>,
}

impl Separability {
    /// The attributes (their place in `columns`) with the least distance over their pairs, the largest first, at
    /// most `n`; an attribute with no pair measured is left out.
    pub fn best(&self, n: usize) -> Vec<(usize, f64)> {
        let mut v: Vec<(usize, f64)> = self
            .jm
            .iter()
            .enumerate()
            .filter_map(|(a, row)| row.iter().flatten().copied().reduce(f64::min).map(|m| (a, m)))
            .collect();
        v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        v.truncate(n);
        v
    }
}

/// The Jeffries-Matusita distance of two classes in one attribute, as Gaussians of the means and variances given:
/// B = (m1 - m2)^2 / (4 (v1 + v2)) + 1/2 ln((v1 + v2) / (2 s1 s2)), JM = 2 (1 - e^-B), from 0 (the same) to 2 (apart).
pub fn jeffries_matusita(m1: f64, v1: f64, m2: f64, v2: f64) -> f64 {
    // (a class whose values are all alike has no variance: a floor keeps the formula finite)
    let (v1, v2) = (v1.max(1e-12), v2.max(1e-12));
    let b = (m1 - m2).powi(2) / (4.0 * (v1 + v2)) + 0.5 * ((v1 + v2) / (2.0 * (v1 * v2).sqrt())).ln();
    2.0 * (1.0 - (-b.max(0.0)).exp())
}

/// The mean and variance (of a sample) of a column over some objects; none under two values.
fn stats(t: &Table, col: usize, ids: &[u32]) -> Option<(f64, f64)> {
    let v: Vec<f64> =
        ids.iter().filter_map(|&i| t.rows.get(i as usize - 1)).map(|r| r[col]).filter(|x| x.is_finite()).collect();
    if v.len() < 2 {
        return None;
    }
    let n = v.len() as f64;
    let mean = v.iter().sum::<f64>() / n;
    Some((mean, v.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0)))
}

/// The separability of the groups of objects (ids) over the columns of a table.
pub fn separability(t: &Table, columns: &[usize], groups: &[Vec<u32>]) -> Separability {
    let pairs: Vec<(usize, usize)> =
        (0..groups.len()).flat_map(|a| (a + 1..groups.len()).map(move |b| (a, b))).collect();
    let jm = columns
        .iter()
        .map(|&c| {
            let st: Vec<Option<(f64, f64)>> = groups.iter().map(|g| stats(t, c, g)).collect();
            pairs
                .iter()
                .map(|&(a, b)| {
                    let ((m1, v1), (m2, v2)) = (st[a]?, st[b]?);
                    Some(jeffries_matusita(m1, v1, m2, v2))
                })
                .collect()
        })
        .collect();
    Separability { columns: columns.to_vec(), pairs, jm }
}

/// The columns of the objects' table that hold numbers to look at (not the id, not a text, not the class).
pub fn numeric(o: &Objects) -> Vec<usize> {
    (1..o.table.columns.len())
        .filter(|&j| !o.table.is_text(&o.table.columns[j]) && o.table.columns[j] != "class")
        .collect()
}

/// A number written short, 6 digits: 0.1, 12.5, -3, 0.000123457.
fn num(x: f64) -> String {
    if x == 0.0 || !x.is_finite() {
        return "0".into();
    }
    let d = (5 - x.abs().log10().floor() as i32).clamp(0, 15) as usize;
    let s = format!("{x:.d$}");
    let s = if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s };
    if s == "-0" { "0".into() } else { s }
}

/// The rule for a range of an attribute: `between(attr, lo - w, lo, hi, hi + w)` (1 inside the range, 0 beyond a
/// border of width `w`, a tenth of the attribute's range `span`), `above(attr, lo - w, lo)` with no upper limit,
/// `below(attr, hi, hi + w)` with no lower one; none with neither.
pub fn range_rule(attr: &str, lo: Option<f64>, hi: Option<f64>, span: f64) -> Option<String> {
    let w = if span > 0.0 && span.is_finite() { span * BORDER } else { 1.0 };
    let (lo, hi) = match (lo, hi) {
        (Some(a), Some(b)) => (Some(a.min(b)), Some(a.max(b))),
        other => other,
    };
    Some(match (lo, hi) {
        (Some(l), Some(h)) => {
            format!("between({attr}, {}, {}, {}, {})", num(l - w), num(l), num(h), num(h + w))
        }
        (Some(l), None) => format!("above({attr}, {}, {})", num(l - w), num(l)),
        (None, Some(h)) => format!("below({attr}, {}, {})", num(h), num(h + w)),
        (None, None) => return None,
    })
}

/// A rule joined to the class's own: `old and new` (the old one in parentheses when it has an `or`; of a rule of
/// several lines, the last, which is its value).
pub fn append_rule(old: Option<&str>, new: &str) -> String {
    let Some(old) = old.map(str::trim).filter(|o| !o.is_empty()) else { return new.to_string() };
    let (head, last) = match old.rfind('\n') {
        Some(i) => (&old[..=i], old[i + 1..].trim()),
        None => ("", old),
    };
    if last.contains(" or ") { format!("{head}({last}) and {new}") } else { format!("{head}{last} and {new}") }
}

/// An object whose class is uncertain: the two most likely classes (index, membership) and the margin.
#[derive(Debug, Clone, PartialEq)]
pub struct Uncertain {
    pub id: u32,
    pub first: (usize, f64),
    pub second: Option<(usize, f64)>,
    pub margin: f64,
}

/// The `n` objects of the smallest margin (the margins as the table gave them, else the largest membership minus the
/// next), the smallest first, ties by id. An object with no membership to any class and those in `skip` are left out.
pub fn uncertain(o: &Objects, margins: Option<&[f64]>, skip: &HashSet<u32>, n: usize) -> Vec<Uncertain> {
    let mut v: Vec<Uncertain> = (1..=o.count as u32)
        .filter(|id| !skip.contains(id))
        .filter_map(|id| {
            let k = id as usize - 1;
            let mut top: Vec<(usize, f64)> = (0..o.membership.len()).map(|c| (c, o.membership[c][k])).collect();
            top.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
            let first = *top.first().filter(|t| t.1 > 0.0)?;
            let margin = margins.map_or_else(|| margin_in(o, id), |m| m[k]);
            Some(Uncertain { id, first, second: top.get(1).copied(), margin })
        })
        .collect();
    v.sort_by(|a, b| a.margin.total_cmp(&b.margin).then(a.id.cmp(&b.id)));
    v.truncate(n);
    v
}

/// The `objects.rules` node a rule is made in: the node of the objects shown when it is one, else the one that reads
/// them.
fn rules_node(m: &Model, key: &(String, String)) -> Option<usize> {
    let want = format!("{}.{}", key.0, key.1);
    m.nodes.iter().position(|n| {
        n.op == "objects.rules"
            && (n.id == key.0 || matches!(n.inputs.get("objects"), Some(Source::Ref(r)) if *r == want))
    })
}

/// The margins the node that made the objects wrote in its table, when they are there.
fn margins(s: &Session, key: &(String, String), o: &Objects) -> Option<Vec<f64>> {
    match s.value_of(&key.0, "table")? {
        Value::Table(t) if t.rows.len() == o.count => t.column("margin").map(|m| m.to_vec()),
        _ => None,
    }
}

/// *Make a rule*: the rule `text` joined to the class's own in the `objects.rules` node of the objects `key` (one fed
/// by them is made when the model has none), as one step of undo. A message of what was done.
pub fn make_rule(
    s: &mut Session,
    r: &Registry,
    key: &(String, String),
    class: &str,
    text: &str,
) -> Result<String, String> {
    let class = class.trim();
    if class.is_empty() {
        return Err("choose the class the rule is for".into());
    }
    if !s.source.nodes.iter().any(|n| n.id == key.0) {
        return Err(format!("{} is not a node of the model's top level: no rules can read from it", key.0));
    }
    let before = s.source.clone();
    match write_rule(s, r, key, class, text) {
        Ok(msg) => {
            s.history.name(&format!("rule of {class}"));
            s.edited();
            Ok(msg)
        }
        Err(e) => {
            s.source = before;
            Err(e)
        }
    }
}

fn write_rule(
    s: &mut Session,
    r: &Registry,
    key: &(String, String),
    class: &str,
    text: &str,
) -> Result<String, String> {
    let top: &[String] = &[];
    let (id, made) = match rules_node(&s.source, key) {
        Some(k) => (s.source.nodes[k].id.clone(), false),
        None => {
            let at = crate::builder::beside(&s.source.nodes, Some(&key.0));
            let id = crate::editor::add_node_named(&mut s.source, r, top, "objects.rules", Some("rules"), at)?;
            crate::editor::connect(&mut s.source, r, top, &format!("{}.{}", key.0, key.1), &id, "objects")?;
            (id, true)
        }
    };
    let node = s.source.nodes.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("no node {id}"))?;
    let mut classes = match node.params.get("classes") {
        None => Vec::new(),
        Some(serde_json::Value::Array(a)) => a.clone(),
        Some(_) => return Err(format!("the classes of {id} are not a list: no rule is added")),
    };
    let rule = match classes.iter_mut().find(|c| c.get("name").and_then(|n| n.as_str()) == Some(class)) {
        Some(c) => {
            let joined = append_rule(c.get("rule").and_then(|r| r.as_str()), text);
            c["rule"] = joined.clone().into();
            joined
        }
        None => {
            classes.push(serde_json::json!({ "name": class, "rule": text }));
            text.to_string()
        }
    };
    node.params.insert("classes".into(), classes.into());
    Ok(format!("rule of {class} in {id}{}: {rule}", if made { " (a new node)" } else { "" }))
}

/// A column chosen among the numeric ones.
fn column_combo(ui: &mut Ui, id: &str, t: &Table, cols: &[usize], cur: &mut usize) {
    egui::ComboBox::from_id_salt(id).selected_text(t.columns[*cur].clone()).show_ui(ui, |ui| {
        for &j in cols {
            ui.selectable_value(cur, j, &t.columns[j]);
        }
    });
}

/// The value of an attribute for an object, when it is a number.
fn value(o: &Objects, id: u32, col: usize) -> Option<f64> {
    o.table.rows.get(id as usize - 1).map(|r| r[col]).filter(|x| x.is_finite())
}

impl ObjectsUi {
    /// The button that opens the feature space.
    pub fn space_button(&mut self, ui: &mut Ui) {
        ui.toggle_value(&mut self.space.open, "Feature space...")
            .on_hover_text("where the samples of each class lie among the attributes, which attributes separate them");
    }

    /// The classes with samples: those given in the Viewer, else those the model's `objects.samples` marked.
    fn groups(&self, s: &Session, o: &Objects) -> Vec<Group> {
        let w = o.grid.width;
        let mut out: Vec<Group> = Vec::new();
        for (k, name) in self.classes.iter().enumerate() {
            let ids: BTreeSet<u32> = self
                .samples
                .iter()
                .filter(|m| m.class == k && m.row < o.grid.height && m.col < w)
                .map(|m| o.labels[m.row * w + m.col])
                .filter(|&i| i != 0)
                .collect();
            if !ids.is_empty() {
                out.push(Group { name: name.clone(), colour: self.colour(k), ids: ids.into_iter().collect() });
            }
        }
        if !out.is_empty() {
            return out;
        }
        // (the objects.samples node of these objects, run: the objects it marked, the classes numbered by name)
        if let Some(sn) = samples_node(&s.source, &self.key)
            && let Some(Value::Objects(m)) = s.value_of(&s.source.nodes[sn].id, "objects")
            && m.count == o.count
        {
            for (c, class) in m.classes.iter().enumerate() {
                let ids: Vec<u32> =
                    (1..=m.count as u32).filter(|&i| m.class_of[i as usize - 1] == c as u32 + 1).collect();
                if !ids.is_empty() {
                    out.push(Group { name: class.name.clone(), colour: class_colour(c), ids });
                }
            }
        }
        out
    }

    /// The feature space, when it is open. What it asks of the Viewer.
    pub fn space_window(&mut self, ctx: &egui::Context, s: &Session, o: &Objects) -> SpaceAction {
        let mut act = SpaceAction::default();
        if !self.space.open {
            return act;
        }
        let cols = numeric(o);
        let mut open = true;
        egui::Window::new("Feature space")
            .id(Id::new("feature space window"))
            .open(&mut open)
            .default_size([560.0, 680.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| self.space_body(ui, s, o, &cols, &mut act));
            });
        if !open {
            self.space.open = false;
        }
        act
    }

    fn space_body(&mut self, ui: &mut Ui, s: &Session, o: &Objects, cols: &[usize], act: &mut SpaceAction) {
        let groups = self.groups(s, o);
        let ids: Vec<Vec<u32>> = groups.iter().map(|g| g.ids.clone()).collect();
        if cols.is_empty() {
            ui.label("these objects have no numeric attributes (objects.features gives them)");
        } else {
            let sp = &mut self.space;
            if !cols.contains(&sp.x) {
                sp.x = cols[0];
            }
            if !cols.contains(&sp.y) {
                sp.y = *cols.get(1).unwrap_or(&cols[0]);
            }
            if !cols.contains(&sp.hist) {
                sp.hist = cols[0];
            }
            if sp.class.is_empty() && !self.classes.is_empty() {
                self.space.class = self.classes[self.active.min(self.classes.len() - 1)].clone();
            }
            self.separability_ui(ui, o, cols, &groups, &ids);
            ui.separator();
            self.scatter_ui(ui, o, &groups);
            ui.separator();
            self.histogram_ui(ui, s, o, &groups, act);
            ui.separator();
        }
        self.uncertain_ui(ui, s, o, act);
    }

    /// The table of the distances by pair and attribute, and the attributes that separate most.
    fn separability_ui(&mut self, ui: &mut Ui, o: &Objects, cols: &[usize], groups: &[Group], ids: &[Vec<u32>]) {
        ui.label(RichText::new("Separability").strong()).on_hover_text(
            "Jeffries-Matusita distance of two classes in an attribute: 0 the same, 2 apart; the samples of each class \
             give a Gaussian",
        );
        if groups.len() < 2 {
            ui.label(
                RichText::new("give samples to two classes at least (Samples, in the panel beside the map)")
                    .weak()
                    .size(11.0),
            );
            return;
        }
        let sep = separability(&o.table, cols, ids);
        egui::ScrollArea::horizontal().id_salt("separability scroll").show(ui, |ui| {
            egui::Grid::new("separability").num_columns(cols.len() + 1).spacing([10.0, 2.0]).show(ui, |ui| {
                ui.label("");
                for &c in cols {
                    ui.label(RichText::new(&o.table.columns[c]).weak().size(11.0));
                }
                ui.end_row();
                for (p, &(a, b)) in sep.pairs.iter().enumerate() {
                    ui.label(RichText::new(format!("{} / {}", groups[a].name, groups[b].name)).size(11.0));
                    for row in &sep.jm {
                        match row[p] {
                            Some(v) => {
                                let t = RichText::new(format!("{v:.2}")).size(11.0);
                                ui.label(if v >= 1.9 {
                                    t.color(Color32::from_rgb(60, 160, 80))
                                } else if v < 1.0 {
                                    t.color(ui.visuals().warn_fg_color)
                                } else {
                                    t
                                });
                            }
                            None => {
                                ui.label(RichText::new("-").weak().size(11.0)).on_hover_text("under two samples");
                            }
                        }
                    }
                    ui.end_row();
                }
            });
        });
        let best = sep.best(TOP);
        if best.is_empty() {
            return;
        }
        ui.add_space(4.0);
        ui.label(RichText::new("The attributes that separate most").size(11.5))
            .on_hover_text("by the smallest distance over the pairs of classes");
        for &(a, m) in &best {
            ui.horizontal(|ui| {
                ui.label(
                    RichText::new(format!("{}: least distance {m:.2}", o.table.columns[sep.columns[a]])).size(11.0),
                );
                if ui.small_button("use").on_hover_text("the axes of the scatter, and the histogram").clicked() {
                    let other = best.iter().map(|b| b.0).find(|&b| b != a).unwrap_or(a);
                    self.space.x = sep.columns[a];
                    self.space.y = sep.columns[other];
                    self.space.hist = sep.columns[a];
                }
            });
        }
    }

    /// The scatter of two attributes.
    fn scatter_ui(&mut self, ui: &mut Ui, o: &Objects, groups: &[Group]) {
        ui.label(RichText::new("Scatter").strong());
        let cols = numeric(o);
        let t = &o.table;
        ui.horizontal(|ui| {
            ui.label("x");
            column_combo(ui, "scatter x", t, &cols, &mut self.space.x);
            ui.label("y");
            column_combo(ui, "scatter y", t, &cols, &mut self.space.y);
        });
        ui.horizontal_wrapped(|ui| {
            for g in groups {
                ui.label(RichText::new(format!("{} ({})", g.name, g.ids.len())).color(g.colour).size(11.0));
            }
        });
        let (x, y) = (self.space.x, self.space.y);
        let at = |id: u32| Some([value(o, id, x)?, value(o, id, y)?]);
        let with: HashSet<u32> = groups.iter().flat_map(|g| g.ids.iter().copied()).collect();
        let step = (o.count / MAX_GREY).max(1);
        let grey: Vec<(u32, [f64; 2])> = (1..=o.count as u32)
            .step_by(step)
            .filter(|i| !with.contains(i))
            .filter_map(|i| Some((i, at(i)?)))
            .collect();
        let coloured: Vec<Vec<(u32, [f64; 2])>> =
            groups.iter().map(|g| g.ids.iter().filter_map(|&i| Some((i, at(i)?))).collect()).collect();
        let ring: Vec<[f64; 2]> = self.selected.iter().filter_map(|&i| at(i)).collect();
        let resp = crate::theme::plot(Plot::new("feature space scatter"))
            .height(260.0)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(false)
            .x_axis_label(t.columns[x].clone())
            .y_axis_label(t.columns[y].clone())
            .show(ui, |p| {
                let faint = ui_grey();
                p.points(
                    Points::new("other objects", grey.iter().map(|g| g.1).collect::<Vec<_>>()).color(faint).radius(2.0),
                );
                for (g, pts) in groups.iter().zip(&coloured) {
                    p.points(
                        Points::new(g.name.clone(), pts.iter().map(|g| g.1).collect::<Vec<_>>())
                            .color(g.colour)
                            .radius(3.5),
                    );
                }
                if !ring.is_empty() {
                    p.points(Points::new("selected", ring).color(Color32::YELLOW).filled(false).radius(7.0));
                }
            });
        // the point nearest the pointer (among those drawn): told on hover, selected by a click
        let Some(pos) = resp.response.hover_pos() else { return };
        let near = grey
            .iter()
            .chain(coloured.iter().flatten())
            .map(|&(id, [px, py])| (id, resp.transform.position_from_point(&PlotPoint::new(px, py)).distance(pos)))
            .filter(|d| d.1 <= NEAR)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((id, _)) = near {
            resp.response.clone().on_hover_text(format!("object {id}"));
            if resp.response.clicked() {
                if ui.input(|i| i.modifiers.command) {
                    if !self.selected.remove(&id) {
                        self.selected.insert(id);
                    }
                } else {
                    self.selected = BTreeSet::from([id]);
                }
            }
        }
    }

    /// The histogram of one attribute per class, the range dragged on it, and *Make a rule*.
    fn histogram_ui(&mut self, ui: &mut Ui, s: &Session, o: &Objects, groups: &[Group], act: &mut SpaceAction) {
        ui.label(RichText::new("Histogram and rule").strong());
        let cols = numeric(o);
        let t = &o.table;
        ui.horizontal(|ui| {
            ui.label("attribute");
            column_combo(ui, "histogram attribute", t, &cols, &mut self.space.hist);
        });
        let col = self.space.hist;
        let all: Vec<f64> = (1..=o.count as u32).filter_map(|i| value(o, i, col)).collect();
        if all.is_empty() {
            ui.label("no values");
            return;
        }
        let mn = all.iter().copied().fold(f64::INFINITY, f64::min);
        let mut mx = all.iter().copied().fold(f64::NEG_INFINITY, f64::max);
        if mx <= mn {
            mx = mn + 1.0;
        }
        let span = mx - mn;
        let bw = span / BINS as f64;
        let bin = |v: f64| (((v - mn) / bw) as usize).min(BINS - 1);
        // (a fraction of each class's objects, so that a class with few samples shows as well as one with many)
        let mut series: Vec<(String, Color32, Vec<f64>)> = Vec::new();
        for g in groups {
            let vals: Vec<f64> = g.ids.iter().filter_map(|&i| value(o, i, col)).collect();
            if vals.is_empty() {
                continue;
            }
            let mut f = vec![0.0; BINS];
            for v in &vals {
                f[bin(*v)] += 1.0 / vals.len() as f64;
            }
            series.push((g.name.clone(), g.colour, f));
        }
        if series.is_empty() {
            let mut f = vec![0.0; BINS];
            for v in &all {
                f[bin(*v)] += 1.0 / all.len() as f64;
            }
            series.push(("all objects".into(), ui_grey(), f));
        }
        let top = series.iter().flat_map(|s| s.2.iter().copied()).fold(0.0, f64::max).max(1e-9) * 1.05;
        let range = (self.space.lo, self.space.hi);
        let resp = crate::theme::plot(Plot::new("feature space histogram"))
            .height(200.0)
            .allow_drag(false)
            .allow_scroll(false)
            .allow_zoom(false)
            .allow_boxed_zoom(false)
            .x_axis_label(t.columns[col].clone())
            .show(ui, |p| {
                for (name, colour, f) in &series {
                    let bars: Vec<Bar> = f
                        .iter()
                        .enumerate()
                        .map(|(b, &v)| {
                            Bar::new(mn + (b as f64 + 0.5) * bw, v).width(bw).fill(colour.gamma_multiply(0.45))
                        })
                        .collect();
                    p.bar_chart(BarChart::new(name.clone(), bars).color(*colour));
                }
                if range.0.is_some() || range.1.is_some() {
                    let (a, b) = (range.0.unwrap_or(mn).clamp(mn, mx), range.1.unwrap_or(mx).clamp(mn, mx));
                    let (a, b) = (a.min(b), a.max(b));
                    p.polygon(
                        Polygon::new("range", vec![[a, 0.0], [b, 0.0], [b, top], [a, top]])
                            .fill_color(Color32::from_rgba_unmultiplied(255, 200, 0, 50))
                            .stroke(egui::Stroke::NONE),
                    );
                    for e in [range.0, range.1].into_iter().flatten() {
                        p.vline(VLine::new("limit", e).color(Color32::from_rgb(255, 200, 0)));
                    }
                }
            });
        // a drag on the plot is the range
        if let Some(pos) = resp.response.interact_pointer_pos() {
            let x = resp.transform.value_from_position(pos).x.clamp(mn, mx);
            if resp.response.drag_started() {
                self.space.anchor = Some(x);
            }
            if resp.response.dragged()
                && let Some(a) = self.space.anchor
            {
                self.space.lo = Some(a.min(x));
                self.space.hi = Some(a.max(x));
            }
        }
        // the limits, by number too (and a side left open)
        ui.horizontal(|ui| {
            for (label, limit, at) in [("from", &mut self.space.lo, mn), ("to", &mut self.space.hi, mx)] {
                let mut on = limit.is_some();
                if ui.checkbox(&mut on, label).changed() {
                    *limit = on.then_some(at);
                }
                if let Some(v) = limit {
                    ui.add(egui::DragValue::new(v).speed(span / 200.0).max_decimals(4));
                }
            }
            if ui.button("Clear").clicked() {
                self.space.lo = None;
                self.space.hi = None;
            }
        });
        // the objects inside the range, selected (once for each range)
        let want = (col, self.space.lo, self.space.hi);
        if self.space.applied != Some(want) {
            let had = self.space.applied.is_some_and(|a| a.1.is_some() || a.2.is_some());
            self.space.applied = Some(want);
            if want.1.is_some() || want.2.is_some() {
                let (lo, hi) = (want.1.unwrap_or(f64::NEG_INFINITY), want.2.unwrap_or(f64::INFINITY));
                self.selected =
                    (1..=o.count as u32).filter(|&i| value(o, i, col).is_some_and(|v| v >= lo && v <= hi)).collect();
            } else if had {
                self.selected.clear();
            }
        }
        let name = t.columns[col].clone();
        let rule = range_rule(&name, self.space.lo, self.space.hi, span);
        if let Some(r) = &rule {
            ui.label(RichText::new(format!("{} objects inside: {r}", self.selected.len())).weak().size(11.0));
        } else {
            ui.label(RichText::new("drag a range on the plot to light the objects inside it").weak().size(11.0));
        }
        // the class the rule is for: typed, or one of the classes the model and the palette have
        let mut known: Vec<String> = Vec::new();
        if let Some(k) = rules_node(&s.source, &self.key)
            && let Some(serde_json::Value::Array(a)) = s.source.nodes[k].params.get("classes")
        {
            known.extend(a.iter().filter_map(|c| c.get("name")?.as_str().map(String::from)));
        }
        known.extend(self.classes.iter().cloned());
        known.extend(o.classes.iter().map(|c| c.name.clone()));
        let mut seen = HashSet::new();
        known.retain(|c| seen.insert(c.clone()));
        ui.horizontal(|ui| {
            ui.label("class");
            ui.add(egui::TextEdit::singleline(&mut self.space.class).desired_width(110.0).hint_text("class"));
            if !known.is_empty() {
                ui.menu_button("v", |ui| {
                    for k in &known {
                        if ui.button(k).clicked() {
                            self.space.class = k.clone();
                            ui.close();
                        }
                    }
                });
            }
            let ready = rule.is_some() && !self.space.class.trim().is_empty();
            if ui
                .add_enabled(ready, egui::Button::new("Make a rule"))
                .on_hover_text(
                    "the range as a fuzzy rule of the class in the model's objects.rules (joined by `and` to the \
                     class's own rule)",
                )
                .clicked()
                && let Some(r) = rule
            {
                act.rule = Some((self.space.class.trim().to_string(), r));
            }
        });
        if let Some(m) = &self.space.message {
            ui.label(RichText::new(m).size(11.0).color(ui.visuals().warn_fg_color));
        }
    }

    /// Where to give a sample next: the objects of the least certain class, and the errors of the validation.
    fn uncertain_ui(&mut self, ui: &mut Ui, s: &Session, o: &Objects, act: &mut SpaceAction) {
        if !o.membership.is_empty() {
            ui.label(RichText::new("Most uncertain").strong())
                .on_hover_text("the objects with the smallest margin: the largest membership minus the next one");
            let marks: HashSet<u32> = self.groups(s, o).iter().flat_map(|g| g.ids.iter().copied()).collect();
            let m = margins(s, &self.key, o);
            let list = uncertain(o, m.as_deref(), &marks, UNCERTAIN);
            ui.label(
                RichText::new(
                    "click one to see it on the map; give it its class with Samples (the palette beside the map)",
                )
                .weak()
                .size(11.0),
            );
            for u in &list {
                let name = |c: usize| o.classes.get(c).map_or("?", |c| c.name.as_str());
                let mut text = format!("{}: {} {:.2}", u.id, name(u.first.0), u.first.1);
                if let Some((c, p)) = u.second {
                    text.push_str(&format!(", {} {p:.2}", name(c)));
                }
                text.push_str(&format!(" (margin {:.2})", u.margin));
                if ui.selectable_label(self.selected.len() == 1 && self.selected.contains(&u.id), text).clicked() {
                    self.selected = BTreeSet::from([u.id]);
                    act.go = Some(u.id);
                }
            }
        }
        // the errors the model's validation found
        let want = format!("{}.{}", self.key.0, self.key.1);
        let acc = s.source.nodes.iter().find(|n| {
            n.op == "objects.accuracy" && matches!(n.inputs.get("objects"), Some(Source::Ref(r)) if *r == want)
        });
        if let Some(Value::Table(t)) = acc.and_then(|n| s.value_of(&n.id, "errors")) {
            ui.add_space(6.0);
            ui.label(RichText::new("Errors of the validation").strong())
                .on_hover_text("the reference objects the map got wrong (objects.accuracy)");
            if t.rows.is_empty() {
                ui.label(RichText::new("none: every reference object is mapped as its reference").weak().size(11.0));
            }
            egui::ScrollArea::vertical().id_salt("validation errors").max_height(160.0).show(ui, |ui| {
                for r in t.rows.iter().take(500) {
                    let id = r[0] as u32;
                    let text = format!("object {id}: mapped {}, reference {}", t.cell(1, r[1]), t.cell(2, r[2]));
                    if id as usize <= o.count
                        && ui.selectable_label(self.selected.len() == 1 && self.selected.contains(&id), text).clicked()
                    {
                        self.selected = BTreeSet::from([id]);
                        act.go = Some(id);
                    }
                }
            });
        }
    }
}

/// The grey of the objects without a sample: faint.
fn ui_grey() -> Color32 {
    Color32::from_gray(150).gamma_multiply(0.4)
}

/// The box of an object as a place (west, south, east, north), grown to a few times its size (at least `least` cells
/// across): what the map is taken to when an object is gone to. None for no object or a grid turned.
pub fn extent_of_object(o: &Objects, id: u32, least: f64) -> Option<[f64; 4]> {
    let b = *o.boxes.get((id as usize).checked_sub(1)?)?;
    let t = o.grid.transform;
    if t[2] != 0.0 || t[4] != 0.0 || t[1] == 0.0 || t[5] == 0.0 {
        return None;
    }
    let (c0, c1) = (b[1] as f64, b[3] as f64 + 1.0);
    let (r0, r1) = (b[0] as f64, b[2] as f64 + 1.0);
    let half = ((c1 - c0).max(r1 - r0).max(least) * 1.5) / 2.0;
    let (cc, cr) = ((c0 + c1) / 2.0, (r0 + r1) / 2.0);
    let (xa, xb) = (t[0] + (cc - half) * t[1], t[0] + (cc + half) * t[1]);
    let (ya, yb) = (t[3] + (cr - half) * t[5], t[3] + (cr + half) * t[5]);
    Some([xa.min(xb), ya.min(yb), xa.max(xb), ya.max(yb)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terraflux_raster::Grid;

    #[test]
    fn jm_is_zero_for_the_same_class_and_two_for_classes_far_apart() {
        assert!(jeffries_matusita(3.0, 1.5, 3.0, 1.5).abs() < 1e-12);
        let far = jeffries_matusita(0.0, 1.0, 100.0, 1.0);
        assert!((far - 2.0).abs() < 1e-9, "{far}");
        // (known value: means 1 apart, variance 1 each: B = 1/8, JM = 2 (1 - e^-1/8))
        let one = jeffries_matusita(0.0, 1.0, 1.0, 1.0);
        assert!((one - 2.0 * (1.0 - (-0.125f64).exp())).abs() < 1e-12, "{one}");
        // (the same mean, other spread: only the log term)
        let spread = jeffries_matusita(0.0, 1.0, 0.0, 4.0);
        let b = 0.5 * (5.0f64 / (2.0 * 2.0)).ln();
        assert!((spread - 2.0 * (1.0 - (-b).exp())).abs() < 1e-12, "{spread}");
        // (no variance at all: finite, 0 for the same value and 2 for two)
        assert!(jeffries_matusita(1.0, 0.0, 1.0, 0.0).abs() < 1e-9);
        assert!((jeffries_matusita(1.0, 0.0, 2.0, 0.0) - 2.0).abs() < 1e-9);
    }

    fn table() -> Table {
        // id, a (separates a from b), b (the same for both)
        let mut t = Table::new(vec!["id".into(), "a".into(), "b".into()]);
        for i in 0..8 {
            let class_b = i >= 4;
            t.rows.push(vec![i as f64 + 1.0, if class_b { 10.0 } else { 0.0 } + (i % 4) as f64 * 0.1, (i % 4) as f64]);
        }
        t
    }

    #[test]
    fn the_separability_ranks_the_attributes_by_their_worst_pair() {
        let t = table();
        let sep = separability(&t, &[1, 2], &[vec![1, 2, 3, 4], vec![5, 6, 7, 8]]);
        assert_eq!(sep.pairs, vec![(0, 1)]);
        assert!((sep.jm[0][0].unwrap() - 2.0).abs() < 1e-6, "a separates");
        assert!(sep.jm[1][0].unwrap().abs() < 1e-9, "b does not");
        assert_eq!(sep.best(5).iter().map(|b| b.0).collect::<Vec<_>>(), vec![0, 1]);
        // (a class of one object has no spread: no distance)
        let one = separability(&t, &[1], &[vec![1], vec![5, 6]]);
        assert_eq!(one.jm[0][0], None);
        assert!(one.best(5).is_empty());
    }

    #[test]
    fn a_range_is_a_fuzzy_rule() {
        // (a range of 10: a border of 1)
        assert_eq!(range_rule("ndvi", Some(2.0), Some(5.0), 10.0).unwrap(), "between(ndvi, 1, 2, 5, 6)");
        assert_eq!(range_rule("ndvi", Some(5.0), Some(2.0), 10.0).unwrap(), "between(ndvi, 1, 2, 5, 6)");
        assert_eq!(range_rule("ndvi", Some(2.5), None, 10.0).unwrap(), "above(ndvi, 1.5, 2.5)");
        assert_eq!(range_rule("ndvi", None, Some(0.25), 1.0).unwrap(), "below(ndvi, 0.25, 0.35)");
        assert_eq!(range_rule("ndvi", None, None, 1.0), None);
        // (a rule joined to the class's own)
        assert_eq!(append_rule(None, "above(a, 1, 2)"), "above(a, 1, 2)");
        assert_eq!(append_rule(Some("  "), "above(a, 1, 2)"), "above(a, 1, 2)");
        assert_eq!(append_rule(Some("below(b, 1, 2)"), "above(a, 1, 2)"), "below(b, 1, 2) and above(a, 1, 2)");
        assert_eq!(append_rule(Some("x > 1 or y > 1"), "above(a, 1, 2)"), "(x > 1 or y > 1) and above(a, 1, 2)");
        assert_eq!(append_rule(Some("w = a > 1\nw"), "above(a, 1, 2)"), "w = a > 1\nw and above(a, 1, 2)");
        assert_eq!(num(-0.0), "0");
        assert_eq!(num(12.5), "12.5");
        assert_eq!(num(0.1 + 0.2), "0.3");
    }

    #[test]
    fn the_most_uncertain_come_first_by_margin() {
        let labels: Vec<u32> = (0..40).map(|i| i as u32 / 4 + 1).collect();
        let o = Objects::from_labels(Arc::new(Grid::new(10, 4)), labels, 10).unwrap();
        // 10 objects: class A's membership 0.0, 0.1 .. 0.9 (B has the rest): margins 1, 0.8, .., 0, .., 0.8
        let a: Vec<f64> = (0..10).map(|i| i as f64 / 10.0).collect();
        let b: Vec<f64> = a.iter().map(|x| 1.0 - x).collect();
        let classes = vec![
            terraflux_ops::objects::ObjectClass { name: "A".into(), colour: None },
            terraflux_ops::objects::ObjectClass { name: "B".into(), colour: None },
        ];
        let o = o.with_classes(classes, vec![1; 10], vec![a, b]).unwrap();
        let list = uncertain(&o, None, &HashSet::new(), 3);
        // id 6 is the 0.5 (margin 0), ids 5 and 7 the 0.4 and 0.6 (margin 0.2 each)
        let mut ids: Vec<u32> = list.iter().map(|u| u.id).collect();
        assert_eq!(ids[0], 6, "{list:?}");
        ids.sort_unstable();
        assert_eq!(ids, vec![5, 6, 7], "{list:?}");
        assert!(list[0].margin.abs() < 1e-12);
        assert!(list.windows(2).all(|w| w[0].margin <= w[1].margin));
        let five = list.iter().find(|u| u.id == 5).unwrap();
        assert_eq!(five.first.0, 1, "B is the likelier at 0.4");
        assert_eq!(five.second.map(|s| s.0), Some(0));
        // (the objects that have a sample are left out; a table's margins rule)
        let skip: HashSet<u32> = [6].into();
        assert!([5, 7].contains(&uncertain(&o, None, &skip, 1)[0].id));
        let m: Vec<f64> = (0..10).map(|i| if i == 9 { 0.0 } else { 1.0 }).collect();
        assert_eq!(uncertain(&o, Some(&m), &HashSet::new(), 2)[0].id, 10);
    }

    #[test]
    fn an_object_is_gone_to_by_its_box() {
        let o = Objects::from_labels(
            Arc::new(Grid::new(8, 4)),
            (0..32).map(|i| if i % 8 < 4 { 1 } else { 2 }).collect(),
            2,
        )
        .unwrap();
        let e = extent_of_object(&o, 2, 2.0).unwrap();
        assert!(e[0] < e[2] && e[1] < e[3]);
        // (the object's centre is the box's: column 6 of 8)
        let t = o.grid.transform;
        let cx = t[0] + 6.0 * t[1];
        assert!(((e[0] + e[2]) / 2.0 - cx).abs() < 1e-9);
        assert!(extent_of_object(&o, 3, 2.0).is_none());
    }
}
