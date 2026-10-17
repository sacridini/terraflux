//! The side panel of the node selected (the Node pane): its name and operator, the problems found in it, what it
//! does (its first sentence, the rest a click away), then what is edited: its inputs (those it needs and those
//! given first, the optional others folded away), its settings (named in words, lists and tables of values edited
//! item by item), a container's body, and the note of why it is there. Every field is editable.

use std::collections::BTreeMap;

use egui::{RichText, Ui};
use terraflux_graph::{Issue, Node, Severity, Source};
use terraflux_ops::Registry;

use crate::app::Session;

/// A setting's name in words: `gray_threshold` is "Gray threshold".
pub fn label_of(name: &str) -> String {
    let words = name.replace('_', " ");
    let mut c = words.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// The first sentence of what an operator does (up to a full stop followed by a capital, at most about 140
/// characters, cut at a word), and whether there is more.
pub fn first_sentence(doc: &str) -> (String, bool) {
    let doc = doc.trim();
    let chars: Vec<char> = doc.chars().collect();
    let mut end = chars.len();
    for k in 1..chars.len().saturating_sub(2) {
        if chars[k] == '.' && chars[k + 1] == ' ' && chars[k + 2].is_uppercase() {
            end = k + 1;
            break;
        }
    }
    const MOST: usize = 140;
    if end > MOST + 40 {
        let cut = chars[..MOST].iter().rposition(|c| *c == ' ').unwrap_or(MOST);
        let short: String = chars[..cut].iter().collect();
        return (format!("{}…", short.trim_end_matches([',', ';', ':'])), true);
    }
    (chars[..end].iter().collect(), end < chars.len())
}

/// The issues of a node of a scope (those of its ports and, for a container, of its body).
fn issues_of<'a>(s: &'a Session, path: &[String], id: &str) -> Vec<&'a Issue> {
    let at = if path.is_empty() { id.to_string() } else { format!("{}/{id}", path.join("/")) };
    s.issues
        .iter()
        .filter(|i| {
            let head = i.at.split('.').next().unwrap_or("");
            head == at || head.strip_prefix(&at).is_some_and(|rest| rest.starts_with('/'))
        })
        .collect()
}

pub fn panel(ui: &mut Ui, s: &mut Session, r: &Registry) {
    let path = s.editor.path.clone();
    let Some(id) = s.selected.clone() else {
        ui.label(
            RichText::new(
                "Click a node to edit it; drag from an output (right) to an input (left) to connect them; \
                 double-click a container to edit its body.",
            )
            .color(ui.visuals().weak_text_color()),
        );
        return;
    };
    let Some(n) = s.source.scope(&path).and_then(|v| v.iter().find(|n| n.id == id)).cloned() else {
        s.selected = None;
        return;
    };
    let mut change: Option<Result<(), String>> = None;
    ui.horizontal(|ui| {
        let mut name = field(s, "id", n.id.clone());
        // (as wide as the pane lets it, beside its button)
        ui.add(egui::TextEdit::singleline(&mut name).desired_width((ui.available_width() - 70.0).clamp(60.0, 140.0)));
        s.fields.insert("id".into(), name.clone());
        if ui.button("Rename").clicked() {
            change = Some(crate::editor::rename_node(&mut s.source, &path, &n.id, &name).map(|_| {
                s.selected = Some(name.clone());
            }));
        }
    });
    ui.label(RichText::new(&n.op).monospace().color(ui.visuals().weak_text_color()));
    // what is wrong with it, first
    for i in issues_of(s, &path, &id) {
        let colour =
            if i.severity == Severity::Error { ui.visuals().error_fg_color } else { ui.visuals().warn_fg_color };
        ui.colored_label(colour, &i.message);
    }
    let sig = r.sig(&n.op).cloned();
    // what it does: the first sentence, the rest a click away
    if let Some(sig) = &sig {
        let (short, more) = first_sentence(&sig.doc);
        let open_id = ui.make_persistent_id("node doc open");
        let mut open = ui.data(|d| d.get_temp::<bool>(open_id)).unwrap_or(false);
        ui.add_space(2.0);
        ui.label(if open { sig.doc.as_str() } else { short.as_str() });
        if more
            && ui
                .add(egui::Label::new(
                    RichText::new(if open { "less" } else { "more..." }).small().color(ui.visuals().hyperlink_color),
                ))
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .interact(egui::Sense::click())
                .clicked()
        {
            open = !open;
            ui.data_mut(|d| d.insert_temp(open_id, open));
        }
    }
    ui.separator();
    ui.label(RichText::new("Inputs").strong());
    // those it needs and those given, then the optional others folded away
    let ports = crate::editor::input_ports(&n, r);
    let (shown, folded): (Vec<_>, Vec<_>) =
        ports.into_iter().partition(|(port, _, required)| *required || n.inputs.contains_key(port));
    let mut input_rows = |ui: &mut Ui, rows: &[(String, terraflux_graph::Type, bool)], salt: &str| {
        egui::Grid::new(("inputs", salt)).num_columns(3).show(ui, |ui| {
            for (port, ty, required) in rows {
                ui.label(if *required { format!("{port} *") } else { port.clone() }).on_hover_text(format!(
                    "{} ({})",
                    ty.name(),
                    if *required { "needed" } else { "optional" }
                ));
                match n.inputs.get(port) {
                    Some(Source::Ref(reference)) => {
                        ui.label(RichText::new(format!("<- {reference}")).monospace());
                        if ui.small_button("x").on_hover_text("disconnect").clicked() {
                            change = Some(crate::editor::disconnect(&mut s.source, &path, &n.id, port));
                        }
                    }
                    other => {
                        let init = match other {
                            Some(Source::Literal(v)) => v.to_string(),
                            _ => String::new(),
                        };
                        let key = format!("in.{port}");
                        let mut text = field(s, &key, init.clone());
                        let resp =
                            ui.add(egui::TextEdit::singleline(&mut text).desired_width(120.0).hint_text("value"));
                        s.fields.insert(key, text.clone());
                        if resp.lost_focus() && text != init {
                            change = Some(crate::editor::set_literal(&mut s.source, r, &path, &n.id, port, &text));
                        }
                    }
                }
                ui.end_row();
            }
        });
    };
    if shown.is_empty() && folded.is_empty() {
        ui.label(RichText::new("none").color(ui.visuals().weak_text_color()));
    }
    input_rows(ui, &shown, "shown");
    if !folded.is_empty() {
        egui::CollapsingHeader::new(RichText::new(format!("Optional inputs ({})", folded.len())).size(11.5))
            .id_salt(("optional inputs", &n.op))
            .default_open(false)
            .show(ui, |ui| input_rows(ui, &folded, "folded"));
    }
    if sig.as_ref().is_some_and(|g| g.extra.is_some() || g.container) {
        ui.horizontal_wrapped(|ui| {
            let mut name = field(s, "new_input", String::new());
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(100.0).hint_text("name"));
            let mut value = field(s, "new_value", String::new());
            ui.add(egui::TextEdit::singleline(&mut value).desired_width(100.0).hint_text("$input, node.port or value"));
            s.fields.insert("new_input".into(), name.clone());
            s.fields.insert("new_value".into(), value.clone());
            if ui.button("Add input").clicked() && !name.is_empty() {
                let v = value.trim();
                change = Some(if v.starts_with('$') || v.starts_with('@') || terraflux_graph::Ref::parse(v).is_ok() {
                    crate::editor::connect(&mut s.source, r, &path, v, &n.id, &name)
                } else {
                    crate::editor::set_literal(&mut s.source, r, &path, &n.id, &name, v)
                });
            }
        });
    }
    let mut params: Vec<(String, String, Option<terraflux_graph::PortSpec>)> = sig
        .as_ref()
        .map(|g| g.params.iter().map(|p| (p.name.clone(), p.doc.clone(), Some(p.clone()))).collect())
        .unwrap_or_default();
    for k in n.params.keys() {
        if !params.iter().any(|(p, _, _)| p == k) {
            params.push((k.clone(), String::new(), None));
        }
    }
    if !params.is_empty() {
        ui.separator();
        ui.label(RichText::new("Settings").strong());
        for (name, doc, spec) in params {
            if let Some(c) = setting(ui, s, &n, &path, &name, &doc, spec.as_ref()) {
                change = Some(c);
            }
        }
    }
    if let Some(b) = &n.body {
        ui.separator();
        ui.label(RichText::new("Body").strong());
        let maps: Vec<(&str, &BTreeMap<String, String>)> = if n.op == terraflux_graph::GROUP {
            vec![("outputs", &b.outputs)]
        } else {
            vec![("carry", &b.carry), ("collect", &b.collect)]
        };
        for (which, map) in &maps {
            for (k, v) in map.iter() {
                ui.label(RichText::new(format!("{which}: {k} <- {v}")).monospace());
            }
        }
        ui.horizontal(|ui| {
            let mut which = field(s, "body_which", maps[0].0.to_string());
            egui::ComboBox::from_id_salt("which").selected_text(&which).show_ui(ui, |ui| {
                for (w, _) in &maps {
                    ui.selectable_value(&mut which, w.to_string(), *w);
                }
            });
            s.fields.insert("body_which".into(), which.clone());
            let mut name = field(s, "body_name", String::new());
            ui.add(egui::TextEdit::singleline(&mut name).desired_width(80.0).hint_text("name"));
            let mut reference = field(s, "body_ref", String::new());
            ui.add(egui::TextEdit::singleline(&mut reference).desired_width(110.0).hint_text("node.port inside"));
            s.fields.insert("body_name".into(), name.clone());
            s.fields.insert("body_ref".into(), reference.clone());
            if ui.button("Set").clicked() {
                let mut inner = path.clone();
                inner.push(n.id.clone());
                change = Some(crate::editor::set_body_map(&mut s.source, r, &inner, &which, &name, reference.trim()));
            }
        });
        if ui.button("Edit the body").clicked() {
            s.editor.path.push(n.id.clone());
            s.selected = None;
            s.fields.clear();
        }
    }
    // why the node is there, written by whoever built the model (or the wizard): edited in place
    ui.separator();
    ui.label(RichText::new("Why").strong());
    let init = n.note.clone().unwrap_or_default();
    let mut text = field(s, "note", init.clone());
    let resp = ui.add(
        egui::TextEdit::multiline(&mut text)
            .desired_rows(2)
            .desired_width(f32::INFINITY)
            .hint_text("why this node is here (a note for whoever reads the model)"),
    );
    s.fields.insert("note".into(), text.clone());
    if resp.lost_focus() && text != init {
        change = Some(crate::editor::set_note(&mut s.source, &path, &n.id, &text));
    }
    ui.separator();
    if ui.button("Delete node").clicked() {
        change = Some(crate::editor::remove_node(&mut s.source, &path, &n.id).map(|_| s.selected = None));
    }
    match change {
        Some(Ok(())) => {
            s.fields.clear();
            s.editor.message = None;
            s.edited();
        }
        Some(Err(e)) => s.editor.message = Some(e),
        None => {}
    }
}

/// A field's text as typed so far (`init` until it is typed in).
fn field(s: &mut Session, key: &str, init: String) -> String {
    s.fields.entry(key.to_string()).or_insert(init).clone()
}

/// A setting: its name in words (its own name and what it does on hover), and its value: a list to choose from,
/// the metrics of landscape metrics, a reclassification's table of rules, a list or a table of values edited item
/// by item, else a text; a file's path chosen in a dialog. What it changed, if anything.
fn setting(
    ui: &mut Ui,
    s: &mut Session,
    n: &Node,
    path: &[String],
    name: &str,
    doc: &str,
    spec: Option<&terraflux_graph::PortSpec>,
) -> Option<Result<(), String>> {
    // the metrics of the landscape metrics: all, by type, or one by one
    if ["metrics", "landscape_metrics", "class_metrics"].contains(&name) && n.op.starts_with("metrics.") {
        return crate::builder::metrics_param(ui, s, n, path, name);
    }
    // a reclassification's rules: a table of them, from the values of the map it reads
    if n.op == "raster.reclassify" && name == "rules" {
        return crate::builder::reclassify_rules(ui, s, n, path);
    }
    let init = n
        .params
        .get(name)
        .map(|v| match v {
            serde_json::Value::String(t) => t.clone(),
            v => v.to_string(),
        })
        .unwrap_or_default();
    // a few values to choose from (a yes or no among them)
    if let Some(spec) = spec.filter(|p| p.ty == terraflux_graph::Type::Bool || !p.choices.is_empty()) {
        return crate::builder::choice_param(ui, s, n, path, spec, &init);
    }
    let hover = if doc.is_empty() { name.to_string() } else { format!("{name}: {doc}") };
    // a list or a table of plain values, item by item (or as text, when asked)
    let raw_key = format!("raw.{name}");
    let raw = s.fields.get(&raw_key).is_some_and(|v| v == "1");
    if let Some(v) = n.params.get(name).filter(|v| items(v).is_some()) {
        ui.horizontal(|ui| {
            ui.label(label_of(name)).on_hover_text(&hover);
            let toggle = ui
                .add(egui::Button::new(RichText::new("{ }").monospace().size(10.0)).small().selected(raw))
                .on_hover_text(if raw { "item by item" } else { "as text (JSON)" });
            if toggle.clicked() {
                s.fields.insert(raw_key.clone(), if raw { "0" } else { "1" }.into());
            }
        });
        if !raw {
            return values(ui, s, n, path, name, v);
        }
    } else {
        ui.label(label_of(name)).on_hover_text(&hover);
    }
    let mut change = None;
    let key = format!("param.{name}");
    let mut text = field(s, &key, init.clone());
    let resp = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(1).desired_width(f32::INFINITY).hint_text(doc));
    s.fields.insert(key, text.clone());
    if resp.lost_focus() && text != init {
        change = Some(crate::editor::set_param(&mut s.source, path, &n.id, name, &text));
    }
    // a file: chosen in a dialog
    if name == "path" && terraflux_graph::FILE_OPS.contains(&n.op.as_str()) && ui.button("Choose...").clicked() {
        let dialog = match n.op.as_str() {
            terraflux_graph::PLUGIN => rfd::FileDialog::new().add_filter("WebAssembly", &["wasm", "wat"]),
            "vector.read" | "vector.write" => rfd::FileDialog::new()
                .add_filter("Vectors", &crate::builder::VECTOR_EXTENSIONS)
                .add_filter("Every file", &["*"]),
            "table.read" | "table.write" => rfd::FileDialog::new()
                .add_filter("Tables", &["csv", "tsv", "txt", "json"])
                .add_filter("Every file", &["*"]),
            _ => rfd::FileDialog::new()
                .add_filter("Rasters", &terraflux_ops::files::RASTER_EXTENSIONS)
                .add_filter("Every file", &["*"]),
        };
        let writes = ["raster.write", "landscape.mspa_tiled", "vector.write", "table.write"].contains(&n.op.as_str());
        let picked = if writes { dialog.save_file() } else { dialog.pick_file() };
        if let Some(p) = picked {
            let p = p.to_string_lossy().into_owned();
            change = Some(crate::editor::set_param(&mut s.source, path, &n.id, name, &p));
        }
    }
    // a folder: every raster in it, as one stack
    if name == "path"
        && n.op == "raster.read"
        && ui.button("A folder...").on_hover_text("every raster of a folder, as one stack").clicked()
        && let Some(p) = rfd::FileDialog::new().pick_folder()
    {
        let p = p.to_string_lossy().into_owned();
        change = Some(crate::editor::set_param(&mut s.source, path, &n.id, name, &p));
    }
    change
}

/// The items of a value edited item by item: a list of numbers or texts, or a table of them by name (an object
/// of plain values); none for others (lists of tables, nested values: edited as text).
fn items(v: &serde_json::Value) -> Option<Vec<(Option<String>, serde_json::Value)>> {
    let plain = |v: &serde_json::Value| v.is_number() || v.is_string();
    match v {
        serde_json::Value::Array(a) if !a.is_empty() && a.iter().all(plain) => {
            Some(a.iter().map(|v| (None, v.clone())).collect())
        }
        serde_json::Value::Object(o) if !o.is_empty() && o.values().all(plain) => {
            Some(o.iter().map(|(k, v)| (Some(k.clone()), v.clone())).collect())
        }
        _ => None,
    }
}

/// A value typed in an item: a number when it reads as one (an integer kept an integer), else a text.
fn typed(text: &str) -> serde_json::Value {
    match serde_json::from_str::<serde_json::Value>(text.trim()) {
        Ok(v @ serde_json::Value::Number(_)) => v,
        _ => serde_json::Value::String(text.to_string()),
    }
}

/// A list or a table of values, a row per item: its number or name, its value, a button that takes it away; and a
/// row to add one (a list's: a copy of its last). What it changed.
fn values(
    ui: &mut Ui,
    s: &mut Session,
    n: &Node,
    path: &[String],
    name: &str,
    v: &serde_json::Value,
) -> Option<Result<(), String>> {
    let rows = items(v)?;
    let table = rows.first().is_some_and(|(k, _)| k.is_some());
    let mut next: Option<Vec<(Option<String>, serde_json::Value)>> = None;
    let text_of = |v: &serde_json::Value| match v {
        serde_json::Value::String(t) => t.clone(),
        v => v.to_string(),
    };
    egui::Grid::new(("values", &n.id, name)).num_columns(3).spacing([6.0, 3.0]).show(ui, |ui| {
        for (k, (key, value)) in rows.iter().enumerate() {
            let label = key.clone().unwrap_or_else(|| format!("{}", k + 1));
            ui.label(RichText::new(label).monospace().color(ui.visuals().weak_text_color()));
            let init = text_of(value);
            let f = format!("item.{name}.{k}");
            let mut text = field(s, &f, init.clone());
            let resp = ui.add(egui::TextEdit::singleline(&mut text).desired_width(ui.available_width().min(140.0)));
            s.fields.insert(f, text.clone());
            if resp.lost_focus() && text != init {
                let mut r = rows.clone();
                r[k].1 = typed(&text);
                next = Some(r);
            }
            if ui.small_button("x").on_hover_text("take it away").clicked() {
                let mut r = rows.clone();
                r.remove(k);
                next = Some(r);
            }
            ui.end_row();
        }
    });
    ui.horizontal(|ui| {
        if table {
            let mut key = field(s, &format!("newkey.{name}"), String::new());
            ui.add(egui::TextEdit::singleline(&mut key).desired_width(80.0).hint_text("name"));
            s.fields.insert(format!("newkey.{name}"), key.clone());
            let mut value = field(s, &format!("newvalue.{name}"), String::new());
            ui.add(egui::TextEdit::singleline(&mut value).desired_width(80.0).hint_text("value"));
            s.fields.insert(format!("newvalue.{name}"), value.clone());
            if ui.button("Add").clicked() && !key.trim().is_empty() {
                let mut r = rows.clone();
                r.retain(|(k, _)| k.as_deref() != Some(key.trim()));
                r.push((Some(key.trim().to_string()), typed(&value)));
                next = Some(r);
            }
        } else if ui.small_button("+ add").on_hover_text("one more, as the last").clicked() {
            let mut r = rows.clone();
            r.push(rows.last().cloned().unwrap_or((None, serde_json::Value::from(0))));
            next = Some(r);
        }
    });
    let r = next?;
    let value = if table {
        serde_json::Value::Object(r.into_iter().filter_map(|(k, v)| Some((k?, v))).collect())
    } else {
        serde_json::Value::Array(r.into_iter().map(|(_, v)| v).collect())
    };
    // (none left: the setting's default)
    let text = match &value {
        serde_json::Value::Array(a) if a.is_empty() => String::new(),
        serde_json::Value::Object(o) if o.is_empty() => String::new(),
        v => v.to_string(),
    };
    Some(crate::editor::set_param(&mut s.source, path, &n.id, name, &text))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_in_words_and_first_sentences() {
        assert_eq!(label_of("gray_threshold"), "Gray threshold");
        assert_eq!(label_of("rule"), "Rule");
        let (short, more) = first_sentence("Zonation's ranking (Moilanen et al.). The rank goes from 0 to 1.");
        assert_eq!(short, "Zonation's ranking (Moilanen et al.).");
        assert!(more);
        assert_eq!(first_sentence("One sentence only."), ("One sentence only.".to_string(), false));
        let long = "word ".repeat(80);
        let (short, more) = first_sentence(&long);
        assert!(more && short.ends_with('…') && short.chars().count() <= 142);
    }

    #[test]
    fn values_typed_in_items() {
        assert_eq!(typed("2"), serde_json::json!(2));
        assert_eq!(typed("1.5"), serde_json::json!(1.5));
        assert_eq!(typed("forest"), serde_json::json!("forest"));
        assert!(items(&serde_json::json!([1, 2.5])).is_some());
        assert!(items(&serde_json::json!({"food": "food_sum"})).is_some());
        assert!(items(&serde_json::json!([{"name": "farmer"}])).is_none());
        assert!(items(&serde_json::json!([])).is_none());
    }
}
