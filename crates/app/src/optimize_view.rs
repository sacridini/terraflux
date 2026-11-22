//! The best value of a `segment.optimize` node, and *Apply to...*: the value written into a segmenter of the same
//! image (its setting and the fixed settings the optimize scored it with), or into a new node connected to that
//! image. The panel is under the Viewer's table of scores and in the node's pane. One step of undo.

use std::time::Instant;

use egui::{RichText, Ui};
use terraflux_graph::{Node, Source};
use terraflux_ops::{Registry, Value};

use crate::app::Session;

/// The op the panel is for.
pub const OP: &str = "segment.optimize";

/// The segmenter an optimize node runs and the setting it varies (the defaults when not given).
fn varied(n: &Node) -> Option<(String, String)> {
    let text = |k: &str| n.params.get(k).and_then(|v| v.as_str()).filter(|t| !t.is_empty()).map(str::to_string);
    let segmenter = text("segmenter").unwrap_or_else(|| "segment.multiresolution".into());
    let parameter =
        text("parameter").or_else(|| terraflux_ops::objects::default_parameter(&segmenter).map(str::to_string))?;
    Some((segmenter, parameter))
}

/// A number as the setting of an operator: a whole one is an integer (the settings that are counts read no other).
fn number(x: f64) -> serde_json::Value {
    if x.fract() == 0.0 && x.abs() < 9e15 { serde_json::Value::from(x as i64) } else { serde_json::Value::from(x) }
}

/// The segmenters of the model's top level that an optimize node's value fits: the same operator on the same image.
pub fn targets(s: &Session, node: &str) -> Vec<String> {
    let Some(o) = s.source.nodes.iter().find(|n| n.id == node && n.op == OP) else { return Vec::new() };
    let Some((segmenter, _)) = varied(o) else { return Vec::new() };
    let Some(image) = o.inputs.get("image") else { return Vec::new() };
    s.source
        .nodes
        .iter()
        .filter(|n| n.op == segmenter && n.id != node && n.inputs.get("image") == Some(image))
        .map(|n| n.id.clone())
        .collect()
}

/// What *Apply* writes, for the button's hint: the setting and the fixed settings of the optimize node.
fn writes(o: &Node, parameter: &str, value: f64) -> String {
    let mut text = format!("{parameter} = {value}");
    if let Some(fixed) = o.params.get("params").and_then(|p| p.as_object()) {
        for (k, v) in fixed {
            text.push_str(&format!(", {k} = {v}"));
        }
    }
    text
}

/// *Apply*: the best `value` of the node `node` written into the segmenter `target` (a node of `targets`), or into a
/// new node of the segmenter connected to the optimize node's image when none is given. The setting varied and the
/// fixed `params` are written. One step of undo (named by them); the model is put back on an error.
pub fn apply(s: &mut Session, r: &Registry, node: &str, value: f64, target: Option<&str>) -> Result<String, String> {
    apply_as(s, r, node, value, target, true)
}

/// [`apply`] of a value that is the best (`best`) or another one chosen on the chart (the step is named so).
pub fn apply_as(
    s: &mut Session,
    r: &Registry,
    node: &str,
    value: f64,
    target: Option<&str>,
    best: bool,
) -> Result<String, String> {
    let before = s.source.clone();
    match write(s, r, node, value, target) {
        Ok((msg, parameter, id)) => {
            if best {
                s.history.name(&format!("Best {parameter} of {node}: {value} in {id}"));
            } else {
                let mut name = parameter.chars();
                let name: String = name.next().into_iter().flat_map(char::to_uppercase).chain(name).collect();
                s.history.name(&format!("{name} {value} of {node} in {id}"));
            }
            s.edited();
            Ok(msg)
        }
        Err(e) => {
            s.source = before;
            Err(e)
        }
    }
}

fn write(
    s: &mut Session,
    r: &Registry,
    node: &str,
    value: f64,
    target: Option<&str>,
) -> Result<(String, String, String), String> {
    let o = s
        .source
        .nodes
        .iter()
        .find(|n| n.id == node && n.op == OP)
        .cloned()
        .ok_or_else(|| format!("{node} is not a segment.optimize of the model's top level"))?;
    let (segmenter, parameter) = varied(&o).ok_or_else(|| format!("{node} has no setting to vary"))?;
    if !value.is_finite() {
        return Err(format!("{node} has no best {parameter} (its scores gave none)"));
    }
    let top: &[String] = &[];
    let (id, made) = match target {
        Some(id) => {
            if !s.source.nodes.iter().any(|n| n.id == id && n.op == segmenter) {
                return Err(format!("{id} is not a {segmenter} of the model's top level"));
            }
            (id.to_string(), false)
        }
        None => {
            let Some(Source::Ref(image)) = o.inputs.get("image") else {
                return Err(format!("the image of {node} is not a node's output: no new node can read it"));
            };
            let at = crate::builder::beside(&s.source.nodes, Some(node));
            let name = segmenter.strip_prefix("segment.").unwrap_or(&segmenter);
            let id = crate::editor::add_node_named(&mut s.source, r, top, &segmenter, Some(name), at)?;
            crate::editor::connect(&mut s.source, r, top, image, &id, "image")?;
            (id, true)
        }
    };
    let n = s.source.nodes.iter_mut().find(|n| n.id == id).ok_or_else(|| format!("no node {id}"))?;
    if let Some(fixed) = o.params.get("params").and_then(|p| p.as_object()) {
        for (k, v) in fixed {
            n.params.insert(k.clone(), v.clone());
        }
    }
    n.params.insert(parameter.clone(), number(value));
    let msg = format!("{id}{}: {}", if made { " (a new node)" } else { "" }, writes(&o, &parameter, value));
    Ok((msg, parameter, id))
}

/// The panel: "best size = 12", where to apply it, and *Apply*. `node` is the optimize node shown; nothing when it
/// has not run (no best value yet). `table` is the id of the table of scores shown beside it (if any): a value chosen
/// by a click on its chart is the one *Apply* writes ("Chosen: 8", with the best and *Back to the best*).
pub fn panel(ui: &mut Ui, s: &mut Session, r: &Registry, node: &str, salt: &str, table: Option<egui::Id>) {
    let Some(o) = s.source.nodes.iter().find(|n| n.id == node && n.op == OP).cloned() else { return };
    let Some((_, parameter)) = varied(&o) else { return };
    let Some(Value::Number(best)) = s.value_of(node, "value") else { return };
    let pick = table.zip(s.value_of(node, "scores")).and_then(|(id, t)| match t {
        Value::Table(t) => crate::charts::score_pick(ui.ctx(), id, &t).map(|v| (id, t, v)),
        _ => None,
    });
    let value = pick.as_ref().map_or(best, |p| p.2);
    let targets = targets(s, node);
    let can_make = matches!(o.inputs.get("image"), Some(Source::Ref(_)));
    // the choice kept between frames: a node's id, or "" for a new node
    let key = egui::Id::new(("optimize apply", salt, node));
    let mut chosen: String = ui.data(|d| d.get_temp(key)).unwrap_or_default();
    if !targets.contains(&chosen) && !(can_make && chosen.is_empty()) {
        chosen = targets.first().cloned().unwrap_or_default();
    }
    let err_key = key.with("error");
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("best {parameter} = {best}")).strong());
        if let Some((id, t, v)) = &pick {
            ui.label(
                RichText::new(format!("Chosen: {v}")).strong().color(crate::palette::seen(crate::theme::accent())),
            );
            if ui.button("Back to the best").clicked() {
                crate::charts::set_score_pick(ui.ctx(), *id, t, None);
            }
        }
        if targets.is_empty() && !can_make {
            return;
        }
        let shown = if chosen.is_empty() { "a new node".to_string() } else { chosen.clone() };
        egui::ComboBox::from_id_salt(key.with("to")).selected_text(format!("Apply to: {shown}")).show_ui(ui, |ui| {
            for t in &targets {
                ui.selectable_value(&mut chosen, t.clone(), t);
            }
            if can_make {
                ui.selectable_value(&mut chosen, String::new(), "a new node");
            }
        });
        let hint = format!(
            "write {} into {} (one step of undo, in the Changes)",
            writes(&o, &parameter, value),
            if chosen.is_empty() {
                "a new node of the same segmenter, connected to the same image".into()
            } else {
                chosen.clone()
            }
        );
        if ui.button("Apply").on_hover_text(hint).clicked() {
            let to = (!chosen.is_empty()).then_some(chosen.as_str());
            match apply_as(s, r, node, value, to, pick.is_none()) {
                Ok(m) => {
                    s.notice = Some((m, Instant::now()));
                    ui.data_mut(|d| d.remove::<String>(err_key));
                }
                Err(e) => ui.data_mut(|d| {
                    d.insert_temp(err_key, e);
                }),
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(key, chosen));
    if let Some(e) = ui.data(|d| d.get_temp::<String>(err_key)) {
        ui.label(RichText::new(e).color(ui.visuals().error_fg_color).size(11.0));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An image, an optimize of its scale with a fixed shape, and a multiresolution of the same image.
    fn session() -> Session {
        let mut m = terraflux_graph::Model::new("optimize");
        m.nodes.push(Node::new("land", "nlm.random").param("rows", 10).param("columns", 10).param("seed", 1));
        m.nodes.push(
            Node::new("opt", OP)
                .input("image", "land.map")
                .param("values", serde_json::json!([4, 8]))
                .param("params", serde_json::json!({ "shape": 0.3 })),
        );
        m.nodes.push(Node::new("seg", "segment.multiresolution").input("image", "land.map").param("scale", 5.0));
        m.nodes.push(Node::new("other", "segment.multiresolution").input("image", "opt.objects"));
        Session::from_model(m).unwrap()
    }

    fn node<'a>(s: &'a Session, id: &str) -> &'a Node {
        s.source.nodes.iter().find(|n| n.id == id).unwrap()
    }

    #[test]
    fn the_targets_are_the_segmenters_of_the_same_image() {
        let s = session();
        assert_eq!(targets(&s, "opt"), ["seg"]);
        assert!(targets(&s, "land").is_empty());
        assert_eq!(varied(node(&s, "opt")), Some(("segment.multiresolution".into(), "scale".into())));
    }

    #[test]
    fn applying_to_a_node_writes_the_value_and_the_fixed_settings_and_undo_puts_them_back() {
        let r = Registry::standard();
        let mut s = session();
        let before = s.source.clone();
        let msg = apply(&mut s, &r, "opt", 12.0, Some("seg")).unwrap();
        assert!(msg.contains("scale = 12") && msg.contains("shape = 0.3"), "{msg}");
        let n = node(&s, "seg");
        assert_eq!(n.params["scale"], serde_json::json!(12));
        assert_eq!(n.params["shape"], serde_json::json!(0.3));
        assert!(s.history.can_undo());
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, before, "one undo puts the node back");
        // (not a node of the segmenter on that image)
        assert!(apply(&mut s, &r, "opt", 12.0, Some("land")).is_err());
        assert!(apply(&mut s, &r, "opt", f64::NAN, Some("seg")).is_err());
        assert_eq!(s.source, before, "an error leaves the model as it was");
    }

    #[test]
    fn applying_to_a_new_node_connects_it_to_the_image_and_undo_removes_it() {
        let r = Registry::standard();
        let mut s = session();
        let before = s.source.clone();
        apply(&mut s, &r, "opt", 7.5, None).unwrap();
        assert_eq!(s.source.nodes.len(), before.nodes.len() + 1);
        let n = node(&s, "multiresolution");
        assert_eq!(n.op, "segment.multiresolution");
        assert_eq!(n.inputs.get("image"), Some(&Source::Ref("land.map".into())));
        assert_eq!(n.params["scale"], serde_json::json!(7.5));
        assert_eq!(n.params["shape"], serde_json::json!(0.3));
        assert!(n.pos.is_some());
        // (it is a target of the next value too)
        assert!(targets(&s, "opt").contains(&"multiresolution".to_string()));
        crate::builder::undo_redo(&mut s, false);
        assert_eq!(s.source, before, "one undo takes the new node away");
        crate::builder::undo_redo(&mut s, true);
        assert!(s.source.nodes.iter().any(|n| n.id == "multiresolution"), "and redo brings it back");
    }
}
