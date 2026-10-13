//! Weights of evidence as the viewer shows them, editable as in Dinamica EGO: a transition and a variable chosen,
//! the W+ of each of its ranges in bars (significant ones in the accent), and the ranges in a table: two ranges
//! merged or one split (the ranges' limits given to the fit for that transition), a weight set by hand. Every
//! edit is a setting of the `woe.fit` node (`ranges` of the transition, `weights`), the fit run again.

use egui::{RichText, Ui};
use egui_plot::{Bar, BarChart, Plot};
use serde_json::{Map as JsonMap, Value as Json, json};
use terraflux_ops::woe::{Row, WoeModel, key_of};

use crate::app::Session;
use crate::theme;

/// What is asked of the fit node: the ranges of a variable for a transition (their limits), a range's weight
/// set by hand (none: the one fitted), or the transition's variable as fitted.
#[derive(Debug, Clone, PartialEq)]
pub enum Edit {
    Limits(Vec<f64>),
    Weight(usize, Option<f64>),
    Reset,
}

/// A `woe.fit` node's settings with an edit of a transition's variable: `ranges` and `weights` as JSON.
pub fn edited(params: &JsonMap<String, Json>, f: f64, t: f64, var: &str, ranges: usize, edit: &Edit) -> (Json, Json) {
    let key = key_of(f, t);
    let mut r = params.get("ranges").and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let mut w = params.get("weights").and_then(|v| v.as_object()).cloned().unwrap_or_default();
    let forget = |m: &mut JsonMap<String, Json>| {
        if let Some(Json::Object(o)) = m.get_mut(&key) {
            o.remove(var);
            if o.is_empty() {
                m.remove(&key);
            }
        }
    };
    match edit {
        Edit::Limits(limits) => {
            let own = r.entry(key.clone()).or_insert_with(|| json!({}));
            if let Json::Object(o) = own {
                o.insert(var.to_string(), json!(limits));
            }
            // the weights by hand were of the old ranges
            forget(&mut w);
        }
        Edit::Weight(i, value) => {
            let own = w.entry(key.clone()).or_insert_with(|| json!({}));
            if let Json::Object(o) = own {
                let mut list: Vec<Json> = o.get(var).and_then(|v| v.as_array()).cloned().unwrap_or_default();
                list.resize(ranges, Json::Null);
                list[*i] = value.map_or(Json::Null, |x| json!(x));
                if list.iter().all(|x| x.is_null()) {
                    o.remove(var);
                } else {
                    o.insert(var.to_string(), Json::Array(list));
                }
            }
            if w.get(&key).and_then(|o| o.as_object()).is_some_and(|o| o.is_empty()) {
                w.remove(&key);
            }
        }
        Edit::Reset => {
            forget(&mut r);
            forget(&mut w);
        }
    }
    (Json::Object(r), Json::Object(w))
}

/// The limits of ranges (each range's lower limit, then the last's upper).
pub fn limits_of(rows: &[&Row]) -> Vec<f64> {
    let mut l: Vec<f64> = rows.iter().map(|r| r.lower).collect();
    if let Some(last) = rows.last() {
        l.push(last.upper);
    }
    l
}

/// The weights of a model of a node, editable when the node is a top-level `woe.fit`.
pub fn show(ui: &mut Ui, s: &mut Session, node: &str, model: &WoeModel, salt: &str) {
    let id = egui::Id::new(("woe", salt, node));
    let (mut ti, mut vi): (usize, usize) = ui.data(|d| d.get_temp(id)).unwrap_or((0, 0));
    if model.transitions.is_empty() {
        ui.label("no transition");
        return;
    }
    ti = ti.min(model.transitions.len() - 1);
    let (f, t) = model.transitions[ti];
    let vars: Vec<String> =
        model.entries.iter().filter(|e| e.from == f && e.to == t).map(|e| e.variable.clone()).collect();
    if vars.is_empty() {
        return;
    }
    vi = vi.min(vars.len() - 1);
    let label = terraflux_ops::value::class_label;
    let fit = s.source.nodes.iter().find(|n| n.id == node && n.op == "woe.fit").cloned();
    let mut edit: Option<Edit> = None;
    egui::Frame::new().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.label("transition");
            egui::ComboBox::from_id_salt(id.with("t")).selected_text(format!("{} -> {}", label(f), label(t))).show_ui(
                ui,
                |ui| {
                    for (k, (a, b)) in model.transitions.iter().enumerate() {
                        ui.selectable_value(&mut ti, k, format!("{} -> {}", label(*a), label(*b)));
                    }
                },
            );
            ui.label("variable");
            egui::ComboBox::from_id_salt(id.with("v")).selected_text(vars[vi].clone()).show_ui(ui, |ui| {
                for (k, v) in vars.iter().enumerate() {
                    ui.selectable_value(&mut vi, k, v);
                }
            });
            if fit.is_none() {
                ui.label(RichText::new("weights given: read only").weak());
            }
        });
        let var = &vars[vi];
        let entry = model.entries.iter().find(|e| e.from == f && e.to == t && &e.variable == var).unwrap();
        let rows: Vec<&Row> = model.rows.iter().filter(|r| r.from == f && r.to == t && &r.variable == var).collect();
        let by_hand: Vec<Option<f64>> = fit
            .as_ref()
            .and_then(|n| n.params.get("weights"))
            .and_then(|w| w.get(key_of(f, t)))
            .and_then(|o| o.get(var))
            .and_then(|l| serde_json::from_value::<Vec<Option<f64>>>(l.clone()).ok())
            .unwrap_or_default();
        // the weights in bars
        let accent = theme::accent();
        let weak = ui.visuals().weak_text_color();
        let names: Vec<String> = rows
            .iter()
            .map(|r| {
                if entry.layout.categorical {
                    label(r.lower)
                } else {
                    format!("{}..{}", short(r.lower), short(r.upper))
                }
            })
            .collect();
        let bars: Vec<Bar> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| {
                Bar::new(i as f64, r.weight).width(0.85).name(&names[i]).fill(if r.significant {
                    accent
                } else {
                    weak.gamma_multiply(0.6)
                })
            })
            .collect();
        let labels = names.clone();
        Plot::new(id.with("bars"))
            .height(200.0)
            .allow_scroll(false)
            .y_axis_label("W+")
            .x_axis_formatter(move |m, _| {
                let k = m.value.round();
                if (m.value - k).abs() < 1e-6 && k >= 0.0 {
                    labels.get(k as usize).cloned().unwrap_or_default()
                } else {
                    String::new()
                }
            })
            .show(ui, |p| p.bar_chart(BarChart::new("W+", bars)));
        ui.label(
            RichText::new(
                "in the accent: significant (contrast); the others grey. Merge or split ranges, or set a weight:",
            )
            .weak()
            .size(11.0),
        );
        // the ranges
        let editable = fit.is_some();
        egui::ScrollArea::vertical().max_height(260.0).show(ui, |ui| {
            egui::Grid::new(id.with("rows")).striped(true).num_columns(8).show(ui, |ui| {
                for h in ["range", "cells", "changed", "W+", "contrast", "", "", ""] {
                    ui.label(RichText::new(h).strong().size(11.5));
                }
                ui.end_row();
                for (i, r) in rows.iter().enumerate() {
                    ui.label(&names[i]);
                    ui.label(r.possible.to_string());
                    ui.label(r.executed.to_string());
                    let mut w = r.weight;
                    let hand = by_hand.get(i).copied().flatten().is_some();
                    let resp = ui.add_enabled(editable, egui::DragValue::new(&mut w).speed(0.01).max_decimals(4));
                    if resp.changed() {
                        edit = Some(Edit::Weight(i, Some(w)));
                    }
                    ui.label(format!("{:.3}{}", r.contrast, if r.significant { "" } else { " (ns)" }));
                    if hand {
                        if ui.small_button("fitted").on_hover_text("back to the weight fitted").clicked() {
                            edit = Some(Edit::Weight(i, None));
                        }
                    } else {
                        ui.label("");
                    }
                    let continuous = !entry.layout.categorical;
                    if continuous
                        && i + 1 < rows.len()
                        && ui
                            .add_enabled(editable, egui::Button::new("merge next").small())
                            .on_hover_text("this range and the next as one")
                            .clicked()
                    {
                        let mut l = limits_of(&rows);
                        l.remove(i + 1);
                        edit = Some(Edit::Limits(l));
                    } else if !(continuous && i + 1 < rows.len()) {
                        ui.label("");
                    }
                    if continuous
                        && ui
                            .add_enabled(editable, egui::Button::new("split").small())
                            .on_hover_text("two ranges, at its middle")
                            .clicked()
                    {
                        let mut l = limits_of(&rows);
                        l.insert(i + 1, (r.lower + r.upper) / 2.0);
                        edit = Some(Edit::Limits(l));
                    } else if !continuous {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
        });
        let own = fit.as_ref().is_some_and(|n| {
            let k = key_of(f, t);
            ["ranges", "weights"]
                .iter()
                .any(|p| n.params.get(*p).and_then(|v| v.get(&k)).and_then(|o| o.get(var)).is_some())
        });
        if own && ui.button("Back to the fit").on_hover_text("this transition's variable as the fit makes it").clicked()
        {
            edit = Some(Edit::Reset);
        }
        if let (Some(e), Some(n)) = (&edit, &fit) {
            let (r, w) = edited(&n.params, f, t, var, rows.len(), e);
            let top: &[String] = &[];
            let mut res = crate::editor::set_param(&mut s.source, top, node, "ranges", &r.to_string());
            if res.is_ok() {
                res = crate::editor::set_param(&mut s.source, top, node, "weights", &w.to_string());
            }
            match res {
                Ok(()) => s.edited(),
                Err(e) => s.editor.message = Some(e),
            }
        }
    });
    ui.data_mut(|d| d.insert_temp(id, (ti, vi)));
}

fn short(v: f64) -> String {
    if v == v.trunc() && v.abs() < 1e12 { format!("{}", v as i64) } else { format!("{v:.4}") }
}
