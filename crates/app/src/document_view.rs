//! The Document pane: the model read as the methods section of a paper ([`crate::document`]), for whoever reads,
//! checks and decides rather than builds. The stages in order, a step each (its note, why, inputs, settings in
//! words, expression, record), the containers' bodies within them, the plots as numbered figures; the maps and
//! tables of the last run beside the steps. A step's name selects its node (the Node pane shows all of it); the
//! settings of a single value are edited in place, and the model runs again as after any edit. Export writes the
//! document as a report (HTML or PDF).

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use egui::{Color32, Margin, RichText, TextureHandle, Ui};
use terraflux_ops::{Registry, Value};

use crate::app::Session;
use crate::document::{Body, Document, From, Section, Setting, Size, Stage, Step, data_rows, row_settings};
use crate::report::{Picture, Shown, Visual, visual};
use crate::theme;

/// The images of the values shown, made once each: by node and port, the value they were made of (its identity),
/// the texture and what of the value it is.
type Pics = HashMap<(String, String), (usize, TextureHandle, Option<String>)>;

thread_local! {
    static PICS: RefCell<Pics> = RefCell::new(HashMap::new());
}

/// What a value is, for the images made of it: the address of its cells (the same map gives the same image).
fn identity(v: &Value) -> usize {
    match v {
        Value::Map(m) => Arc::as_ptr(&m.data) as usize,
        Value::Stack(s) => s.layers.first().map_or(0, |l| Arc::as_ptr(l) as usize),
        Value::Raster(r) => Arc::as_ptr(r) as *const u8 as usize + r.chosen,
        Value::Series(items) => items.last().map_or(0, identity) ^ items.len(),
        _ => 0,
    }
}

/// The map a picture of a value shows (a stack's first layer, a series' last), in memory.
fn shown_map(v: &Value) -> Option<terraflux_ops::Map> {
    match v {
        Value::Map(m) => Some(m.clone()),
        Value::Stack(s) => Some(terraflux_ops::Map { grid: s.grid.clone(), data: s.layers.first()?.clone() }),
        Value::Series(items) => shown_map(items.last()?),
        _ => None,
    }
}

/// The image of a node's output (made the first time it is asked for), and what of the value it is.
fn picture(ui: &Ui, s: &Session, node: &str, port: &str) -> Option<Result<(TextureHandle, Option<String>), Visual>> {
    let v = s.value_of(node, port)?;
    if matches!(v, Value::Table(_)) {
        return visual(&v, 0).map(Err);
    }
    // (the palette and the legend as the viewer has them: a new picture when they change)
    let map = shown_map(&v);
    let id = identity(&v)
        ^ map.as_ref().map_or(0, |m| {
            use std::hash::{Hash, Hasher};
            let legend = crate::legend_view::legend_of(s, node, port, &crate::map_view::Source::Map(m.clone()));
            let mut h = std::collections::hash_map::DefaultHasher::new();
            format!("{:?}{legend:?}", crate::builder::palette_of(s, node, port)).hash(&mut h);
            h.finish() as usize
        });
    let key = (node.to_string(), port.to_string());
    if let Some((k, t, of)) = PICS.with(|p| p.borrow().get(&key).cloned())
        && k == id
        && id != 0
    {
        return Some(Ok((t, of)));
    }
    let mut vis = visual(&v, 256)?;
    if let (Some(m), Shown::Picture(_)) = (&map, &vis.shown) {
        let look = crate::builder::output_look(s, node, port, m);
        let (img, _) = crate::palette::image(&m.data, m.grid.width, m.grid.height, 256, &look.coloring);
        let rgba = img.pixels.iter().flat_map(|c| c.to_array()).collect();
        vis.shown = Shown::Picture(Picture { width: img.size[0], height: img.size[1], rgba });
    }
    let Shown::Picture(Picture { width, height, rgba }) = &vis.shown else { return Some(Err(vis)) };
    let img = egui::ColorImage::from_rgba_premultiplied([*width, *height], rgba);
    let t = ui.ctx().load_texture(format!("document-{node}-{port}"), img, egui::TextureOptions::NEAREST);
    PICS.with(|p| {
        let mut p = p.borrow_mut();
        if p.len() > 256 {
            p.clear();
        }
        p.insert(key, (id, t.clone(), vis.of.clone()));
    });
    Some(Ok((t, vis.of)))
}

/// What the pane asks of the session after drawing.
#[derive(Default)]
struct Asked {
    select: Option<(Vec<String>, String)>,
    /// a setting changed: the step's path and id, the setting and its text
    set: Option<(Vec<String>, String, String, String)>,
    /// the Parameters pane to the front
    parameters: bool,
}

fn link(ui: &mut Ui, s: &Session, r: &Registry, f: &From, path: &[String], asked: &mut Asked) {
    let text = f.text(|n| crate::document::only_output(&s.source, r, n));
    match f.node() {
        Some(n) => {
            // the node in the step's scope, else around it (a group's body reads the nodes around the group), or
            // in the body of the group that gives what is read
            let at = match f {
                From::Within { path, .. } => path.clone(),
                _ if s.source.scope(path).is_some_and(|sc| sc.iter().any(|x| x.id == n)) => path.to_vec(),
                _ => Vec::new(),
            };
            if ui.link(RichText::new(text).strong()).on_hover_text("select it").clicked() {
                asked.select = Some((at, n.to_string()));
            }
        }
        None => {
            ui.label(RichText::new(text).strong());
        }
    }
}

fn reference(ui: &mut Ui, s: &Session, r: &Registry, rf: &str, path: &[String], asked: &mut Asked) {
    match terraflux_graph::Ref::parse(rf) {
        Ok(terraflux_graph::Ref::Port { node, port }) => link(ui, s, r, &From::Node { node, port }, path, asked),
        _ => {
            ui.label(RichText::new(rf).strong());
        }
    }
}

fn weak(t: impl Into<String>) -> RichText {
    RichText::new(t.into()).color(theme::weak())
}

/// A value shown: its image `w` points wide at most, a table's first rows, or words.
fn shown(ui: &mut Ui, s: &Session, node: &str, port: &str, w: f32) {
    match picture(ui, s, node, port) {
        None => {
            ui.label(weak("Not run.").italics().small());
        }
        Some(Ok((t, of))) => {
            let size = t.size_vec2();
            let w = w.min(ui.available_width()).max(24.0);
            ui.add(egui::Image::new((t.id(), size * (w / size.x.max(1.0))))).on_hover_text(format!("{node}.{port}"));
            if let Some(of) = of {
                ui.label(weak(of).small());
            }
        }
        Some(Err(v)) => match v.shown {
            Shown::Table { columns, rows, more } => {
                egui::ScrollArea::horizontal().id_salt(("document table", node, port)).show(ui, |ui| {
                    egui::Grid::new(("document table grid", node, port)).striped(true).show(ui, |ui| {
                        for c in &columns {
                            ui.label(RichText::new(c).small().strong());
                        }
                        ui.end_row();
                        for row in &rows {
                            for c in row {
                                ui.label(RichText::new(c).small().monospace());
                            }
                            ui.end_row();
                        }
                    });
                });
                if more > 0 {
                    ui.label(weak(format!("and {more} more rows")).small());
                }
            }
            Shown::Text(t) => {
                ui.label(weak(t).small());
            }
            Shown::Picture(_) => {}
        },
    }
}

/// A setting given: its words and its value, a single value edited in place.
fn setting_ui(ui: &mut Ui, s: &mut Session, st: &Setting, step: &Step, asked: &mut Asked) {
    let key = format!("doc.{}/{}.{}", step.path.join("/"), step.id, st.name);
    // (a list or a table of plain values: its value as the model has it)
    let list = (!st.plain)
        .then(|| s.source.scope(&step.path)?.iter().find(|n| n.id == step.id)?.params.get(&st.name).cloned())
        .flatten()
        .filter(|v| crate::node_view::items(v).is_some());
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.label("•");
        ui.label(format!("{}:", st.label)).on_hover_text(format!("{}: {}", st.name, st.doc));
        if !st.choices.is_empty() && st.plain {
            let shown = match st.value.as_str() {
                "yes" => "true".to_string(),
                "no" => "false".to_string(),
                v => v.to_string(),
            };
            egui::ComboBox::from_id_salt(&key).selected_text(RichText::new(&st.value).strong()).show_ui(ui, |ui| {
                for c in &st.choices {
                    if ui.selectable_label(*c == shown, c).clicked() && *c != shown {
                        asked.set = Some((step.path.clone(), step.id.clone(), st.name.clone(), c.clone()));
                    }
                }
            });
        } else if st.plain {
            let mut text = s.fields.entry(key.clone()).or_insert_with(|| st.value.clone()).clone();
            let w = (text.chars().count() as f32 * 7.5 + 16.0).clamp(40.0, 240.0);
            let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(w).font(egui::TextStyle::Body));
            if resp.lost_focus() && text != st.value {
                asked.set = Some((step.path.clone(), step.id.clone(), st.name.clone(), text.clone()));
            }
            s.fields.insert(key.clone(), text);
        } else if list.is_none() {
            ui.label(RichText::new(&st.value).strong()).on_hover_text("edited in the Node pane (select the step)");
        }
        if let Some(d) = &st.default {
            ui.label(weak(format!("(default {d})")).small());
        }
    });
    // a list or a table of plain values: edited item by item, in place
    if let Some(v) = list {
        ui.indent(&key, |ui| {
            if let Some(text) = crate::node_view::values(ui, s, &format!("{key}."), &st.name, &v) {
                asked.set = Some((step.path.clone(), step.id.clone(), st.name.clone(), text));
            }
        });
    }
}

fn list(ui: &mut Ui, title: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    ui.label(RichText::new(format!("{title}.")).strong());
    for i in items {
        ui.horizontal_wrapped(|ui| {
            ui.label("•");
            ui.label(i);
        });
    }
}

fn body_ui(ui: &mut Ui, s: &mut Session, r: &Registry, b: &Body, num: &str, step: &Step, asked: &mut Asked) {
    let inner: Vec<String> = step.path.iter().cloned().chain([step.id.clone()]).collect();
    if let Some(t) = &b.times {
        ui.horizontal_wrapped(|ui| {
            ui.label("Repeated");
            link(ui, s, r, t, &step.path, asked);
            ui.label("times.");
        });
    }
    for (k, start, each) in &b.carry {
        ui.horizontal_wrapped(|ui| {
            ui.label("Carries");
            ui.label(RichText::new(k).strong());
            if let Some(f) = start {
                ui.label("from");
                link(ui, s, r, f, &step.path, asked);
            }
            ui.label(", then");
            reference(ui, s, r, each, &inner, asked);
        });
    }
    for (title, items) in [("Collects", &b.collect), ("Gives", &b.outputs)] {
        for (k, rf) in items {
            ui.horizontal_wrapped(|ui| {
                ui.label(title);
                ui.label(RichText::new(k).strong());
                ui.label("from");
                reference(ui, s, r, rf, &inner, asked);
            });
        }
    }
    if let Some(u) = &b.until {
        ui.horizontal_wrapped(|ui| {
            ui.label("Stops when");
            reference(ui, s, r, u, &inner, asked);
            ui.label("is true.");
        });
    }
    // the body's steps, set in
    ui.horizontal(|ui| {
        ui.add_space(6.0);
        let (line, _) = ui.allocate_exact_size(egui::vec2(2.0, 0.0), egui::Sense::hover());
        let top = line.top();
        ui.vertical(|ui| {
            for (k, inner_step) in b.steps.iter().enumerate() {
                step_ui(ui, s, r, inner_step, &format!("{num}.{}", k + 1), true, asked);
            }
            let bottom = ui.min_rect().bottom();
            ui.painter().line_segment(
                [egui::pos2(line.center().x, top), egui::pos2(line.center().x, bottom)],
                egui::Stroke::new(2.0, theme::weak().gamma_multiply(0.4)),
            );
        });
    });
}

fn title_ui(ui: &mut Ui, step: &Step, num: &str, size: f32, asked: &mut Asked) {
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(num).strong().size(size));
        if ui.link(RichText::new(&step.id).strong().size(size)).on_hover_text("select it").clicked() {
            asked.select = Some((step.path.clone(), step.id.clone()));
        }
        ui.label(weak(&step.op).monospace().small());
    });
}

/// A check's badge: how it fared in the last run.
fn check_ui(ui: &mut Ui, s: &Session, step: &Step) {
    if step.op != "check.assert" {
        return;
    }
    let Some(c) = crate::checks::of(s).into_iter().find(|c| c.id == step.id && c.path == step.path) else { return };
    let colour = c.verdict.colour(ui.visuals());
    ui.label(RichText::new(format!("{} {}", c.verdict.symbol(), c.verdict.words())).color(colour).strong());
}

/// A step: a line, or a card (`nested`: within its container's card, without a frame of its own).
fn step_ui(ui: &mut Ui, s: &mut Session, r: &Registry, step: &Step, num: &str, nested: bool, asked: &mut Asked) {
    let selected = s.selected.as_deref() == Some(step.id.as_str()) && s.editor.path == step.path;
    if step.size == Size::Line {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new(num).strong());
            if ui.link(RichText::new(&step.id).strong()).on_hover_text("select it").clicked() {
                asked.select = Some((step.path.clone(), step.id.clone()));
            }
            ui.label(weak(&step.op).monospace().small());
            if !step.inputs.is_empty() {
                ui.label("of");
                for (k, l) in step.inputs.iter().enumerate() {
                    if k > 0 {
                        ui.label(",");
                    }
                    link(ui, s, r, &l.from, &step.path, asked);
                }
            }
            if let Some(n) = &step.note {
                ui.label(RichText::new(format!("— {n}")).italics());
            }
            check_ui(ui, s, step);
        });
        return;
    }
    let stroke = if selected { egui::Stroke::new(1.5, theme::accent()) } else { egui::Stroke::NONE };
    let inner = nested;
    let frame = egui::Frame::new()
        .fill(if inner { Color32::TRANSPARENT } else { theme::well() })
        .stroke(stroke)
        .corner_radius(crate::theme::rounded(5.0))
        .inner_margin(if inner { Margin::symmetric(2, 4) } else { Margin::same(10) });
    let resp = frame.show(ui, |ui| {
        ui.set_width(ui.available_width());
        title_ui(ui, step, num, 13.5, asked);
        check_ui(ui, s, step);
        match &step.note {
            Some(n) => {
                ui.label(RichText::new(n).italics());
            }
            None => {
                ui.label(weak("No note.").italics().small());
            }
        }
        if let Some(w) = &step.why {
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new("Why.").strong());
                ui.label(w);
            });
        }
        if let Some(t) = &step.sentence {
            ui.label(t);
        }
        if let Some(f) = &step.file {
            ui.horizontal_wrapped(|ui| {
                ui.label("File");
                ui.label(RichText::new(f).monospace());
            });
        }
        if let Some(e) = &step.expression {
            egui::Frame::new()
                .fill(theme::bar())
                .corner_radius(crate::theme::rounded(3.0))
                .inner_margin(Margin::same(6))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new(e).monospace());
                });
            if !step.inputs.is_empty() {
                ui.horizontal_wrapped(|ui| {
                    ui.label("where");
                    for (k, l) in step.inputs.iter().enumerate() {
                        if k > 0 {
                            ui.label(",");
                        }
                        ui.label(RichText::new(&l.port).monospace());
                        ui.label("=");
                        link(ui, s, r, &l.from, &step.path, asked);
                    }
                });
            }
        } else if !step.inputs.is_empty() {
            ui.horizontal_wrapped(|ui| {
                ui.label("Takes");
                for (k, l) in step.inputs.iter().enumerate() {
                    if k > 0 {
                        ui.label(";");
                    }
                    let port = ui.label(l.port.replace('_', " "));
                    if !l.doc.is_empty() {
                        port.on_hover_text(&l.doc);
                    }
                    link(ui, s, r, &l.from, &step.path, asked);
                }
            });
        }
        for st in &step.settings {
            setting_ui(ui, s, st, step, asked);
        }
        if let Some(b) = &step.body {
            ui.add_space(2.0);
            body_ui(ui, s, r, b, num, step, asked);
        }
        list(ui, "References", &step.refs);
        list(ui, "Assumptions", &step.assumptions);
        list(ui, "Alternatives", &step.alternatives);
        if let Some((port, _)) = &step.shows
            && let Some((node, key)) = step.value_key(port)
        {
            ui.add_space(4.0);
            let w = if matches!(step.stage, Stage::Preparation) { 110.0 } else { 200.0 };
            shown(ui, s, &node, &key, w);
        }
        for f in &step.figures {
            figure_ui(ui, s, r, f, asked);
        }
    });
    if selected && scroll_to(&step.id) {
        resp.response.scroll_to_me(Some(egui::Align::Center));
    }
    ui.add_space(4.0);
}

thread_local! {
    /// The node last selected that the pane scrolled to.
    static SCROLLED: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// Whether to scroll to a step selected: the first time it is seen selected.
fn scroll_to(id: &str) -> bool {
    SCROLLED.with(|s| {
        let mut s = s.borrow_mut();
        if s.as_deref() == Some(id) {
            false
        } else {
            *s = Some(id.to_string());
            true
        }
    })
}

fn figure_ui(ui: &mut Ui, s: &mut Session, r: &Registry, f: &crate::document::Figure, asked: &mut Asked) {
    ui.add_space(6.0);
    shown(ui, s, &f.node, &f.port, 360.0);
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Figure {}.", f.number)).strong());
        ui.label(&f.caption);
        ui.label(weak("from").small());
        link(ui, s, r, &From::Node { node: f.node.clone(), port: f.port.clone() }, &[], asked);
    });
}

fn data_ui(ui: &mut Ui, s: &mut Session, r: &Registry, sec: &Section, num: &str, asked: &mut Asked) {
    for (k, row) in data_rows(&sec.steps).iter().enumerate() {
        egui::Frame::new()
            .fill(theme::well())
            .corner_radius(crate::theme::rounded(5.0))
            .inner_margin(Margin::same(8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    // the maps of the row, small
                    for st in row.iter().take(4) {
                        if let Some((port, _)) = &st.shows
                            && let Some((node, key)) = st.value_key(port)
                            && let Some(Ok((t, _))) = picture(ui, s, &node, &key)
                        {
                            let size = t.size_vec2();
                            ui.add(egui::Image::new((t.id(), size * (40.0 / size.x.max(1.0))))).on_hover_text(&st.id);
                        }
                    }
                    ui.vertical(|ui| {
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new(format!("{num}.{}", k + 1)).strong());
                            for (j, st) in row.iter().enumerate() {
                                if j > 0 {
                                    ui.label(",");
                                }
                                if ui.link(RichText::new(&st.id).strong()).on_hover_text("select it").clicked() {
                                    asked.select = Some((st.path.clone(), st.id.clone()));
                                }
                            }
                            ui.label(weak(&row[0].op).monospace().small());
                        });
                        if let Some(n) = &row[0].note {
                            ui.label(RichText::new(n).italics());
                        }
                        for f in row.iter().filter_map(|st| st.file.as_ref()) {
                            ui.label(RichText::new(f).monospace().small());
                        }
                        if row.len() == 1 {
                            if let Some(t) = &row[0].sentence {
                                ui.label(t);
                            }
                            for st in &row[0].settings {
                                setting_ui(ui, s, st, row[0], asked);
                            }
                        } else {
                            let set: Vec<String> = row_settings(row).iter().map(|(l, v)| format!("{l}: {v}")).collect();
                            if !set.is_empty() {
                                ui.label(weak(set.join("; ")).small());
                            }
                        }
                        if let Some(w) = &row[0].why {
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("Why.").strong());
                                ui.label(w);
                            });
                        }
                    });
                });
                for st in row.iter() {
                    for f in &st.figures {
                        figure_ui(ui, s, r, f, asked);
                    }
                }
            });
        ui.add_space(4.0);
    }
}

/// Write the report of the model open (a dialog asks where: HTML or PDF).
pub fn export(s: &mut Session, r: &Registry) {
    let name = s.source.name.clone().unwrap_or_else(|| "model".into());
    let Some(path) = rfd::FileDialog::new()
        .set_file_name(format!("{name}.html"))
        .add_filter("A web page (HTML)", &["html"])
        .add_filter("PDF", &["pdf"])
        .save_file()
    else {
        return;
    };
    let doc = Document::of(&s.source, r);
    let values = |n: &str, p: &str| s.value_of(n, p);
    let first = |n: &str| crate::document::only_output(&s.source, r, n);
    let said = match crate::report::write(&path, &doc, &values, &first) {
        Ok(()) => format!("wrote {}", path.display()),
        Err(e) => format!("{} not written: {e}", path.display()),
    };
    s.notice = Some((said, std::time::Instant::now()));
}

/// The Document pane; whether the Parameters pane is asked for.
/// The checks, a line each: how each fared.
pub fn check_list(checks: &[crate::checks::Check]) -> String {
    checks
        .iter()
        .map(|c| {
            let at = c.path.iter().chain(std::iter::once(&c.id)).cloned().collect::<Vec<_>>().join("/");
            format!("{} {at} ({}): {}", c.verdict.symbol(), c.expression, c.verdict.words())
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn pane(ui: &mut Ui, s: &mut Session, r: &Registry) -> bool {
    let doc = Document::of(&s.source, r);
    let mut asked = Asked::default();
    egui::Frame::new().inner_margin(Margin::symmetric(10, 6)).show(ui, |ui| {
        ui.horizontal(|ui| {
            let steps = doc.steps().iter().filter(|st| st.size != Size::Line).count();
            ui.label(weak(format!("{steps} steps")).small());
            // the model's health: how its checks fared
            let checks = crate::checks::of(s);
            if let (Some(h), Some(w)) = (crate::checks::health(&checks), crate::checks::worst(&checks)) {
                ui.label(RichText::new(format!("· {} {h}", w.symbol())).color(w.colour(ui.visuals())).small())
                    .on_hover_text(check_list(&checks));
            }
            let unexplained = doc.unexplained();
            if unexplained > 0 {
                ui.label(weak(format!("· {unexplained} without a note or a why")).small())
                    .on_hover_text("what each step is for and why it was chosen: the Node pane writes them");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Export report...").on_hover_text("the document as a web page (HTML) or a PDF").clicked() {
                    export(s, r);
                }
            });
        });
    });
    ui.separator();
    egui::ScrollArea::vertical().id_salt("document pane").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(Margin::symmetric(14, 8)).show(ui, |ui| {
            ui.set_max_width(820.0);
            ui.label(RichText::new(&doc.title).size(20.0).strong());
            if let Some(d) = &doc.description {
                ui.label(weak(d).italics());
            }
            for (label, v) in [("Request", &doc.request), ("Author", &doc.author), ("Summary", &doc.summary)] {
                if let Some(v) = v {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(format!("{label}.")).strong());
                        ui.label(v);
                    });
                }
            }
            if doc.sections.is_empty() && doc.inputs.is_empty() {
                ui.add_space(8.0);
                ui.label(weak(
                    "The model has no nodes yet. As it is built, it reads here as a document: its data, preparation, \
                     model, validation and results, step by step.",
                ));
                return;
            }
            let mut k = 0;
            let heading = |ui: &mut Ui, text: String| {
                ui.add_space(12.0);
                ui.label(RichText::new(text).size(16.0).strong());
                ui.separator();
            };
            if !doc.inputs.is_empty() {
                k += 1;
                heading(ui, format!("{k}. Inputs"));
                egui::Grid::new("document inputs").striped(true).show(ui, |ui| {
                    for i in &doc.inputs {
                        ui.label(RichText::new(format!("${}", i.name)).strong());
                        ui.label(weak(i.kind));
                        ui.label(RichText::new(&i.value).monospace().small());
                        ui.label(RichText::new(i.note.as_deref().unwrap_or("")).italics());
                        ui.end_row();
                    }
                });
            }
            if !doc.parameters.is_empty() {
                k += 1;
                heading(ui, format!("{k}. Parameters"));
                ui.horizontal_wrapped(|ui| {
                    ui.label(weak("The values to change a scenario with:").small());
                    if ui.link(weak("the Parameters pane").small()).clicked() {
                        asked.parameters = true;
                    }
                });
                egui::Grid::new("document parameters").striped(true).show(ui, |ui| {
                    for p in &doc.parameters {
                        ui.label(RichText::new(&p.label).strong()).on_hover_text(&p.key);
                        ui.label(format!(
                            "{}{}",
                            p.value,
                            p.unit.as_ref().map(|u| format!(" {u}")).unwrap_or_default()
                        ));
                        ui.label(weak(p.range.as_deref().unwrap_or("")).small());
                        ui.label(RichText::new(p.help.as_deref().unwrap_or("")).small());
                        ui.end_row();
                    }
                });
            }
            if !doc.scenarios.is_empty() {
                k += 1;
                heading(ui, format!("{k}. Scenarios"));
                for sc in &doc.scenarios {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(&sc.name).strong());
                        let v: Vec<String> = sc.values.iter().map(|(l, v)| format!("{l} {v}")).collect();
                        ui.label(v.join(" · "));
                    });
                    if let Some(n) = &sc.note {
                        ui.label(RichText::new(n).italics().small());
                    }
                }
            }
            for sec in &doc.sections {
                k += 1;
                let num = k.to_string();
                heading(ui, format!("{num}. {}", sec.title));
                // a group's section: its note, and the group a click away
                if let Some(g) = &sec.group {
                    ui.horizontal_wrapped(|ui| {
                        if let Some(n) = &sec.note {
                            ui.label(RichText::new(n).italics());
                        }
                        if ui.link(weak(format!("the group {g}")).small()).on_hover_text("select it").clicked() {
                            asked.select = Some((Vec::new(), g.clone()));
                        }
                    });
                    ui.add_space(4.0);
                }
                if sec.stage == Stage::Data {
                    data_ui(ui, s, r, sec, &num, &mut asked);
                } else {
                    for (j, st) in sec.steps.iter().enumerate() {
                        step_ui(ui, s, r, st, &format!("{num}.{}", j + 1), false, &mut asked);
                    }
                }
                if sec.stage == Stage::Results {
                    for f in &doc.figures {
                        figure_ui(ui, s, r, f, &mut asked);
                    }
                }
            }
            if !doc.figures.is_empty() && !doc.sections.iter().any(|sec| sec.stage == Stage::Results) {
                k += 1;
                heading(ui, format!("{k}. Results"));
                for f in &doc.figures {
                    figure_ui(ui, s, r, f, &mut asked);
                }
            }
            if !doc.outputs.is_empty() {
                k += 1;
                heading(ui, format!("{k}. Outputs"));
                egui::Grid::new("document outputs").striped(true).show(ui, |ui| {
                    for o in &doc.outputs {
                        ui.label(RichText::new(&o.name).strong());
                        reference(ui, s, r, &o.from, &[], &mut asked);
                        ui.label(RichText::new(&o.file).monospace().small());
                        ui.end_row();
                    }
                });
            }
        });
    });
    if let Some((path, id)) = asked.select {
        s.editor.path = path.clone();
        if path.is_empty() {
            crate::builder::select(s, r, &id);
        } else {
            s.fields.clear();
            s.selected = Some(id.clone());
        }
        s.editor.reveal(&id);
    }
    if let Some((path, id, name, text)) = asked.set {
        match crate::editor::set_param(&mut s.source, &path, &id, &name, &text) {
            Ok(()) => {
                s.edited();
                // the whole model again, as the document shows every step
                s.run_when_ready = true;
            }
            Err(e) => s.editor.message = Some(e),
        }
    }
    asked.parameters
}
