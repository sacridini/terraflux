//! The Problems pane: what is wrong with the model, in one place, each a click away from its node. The last run's
//! failure first, explained ([`crate::diagnose`]): the node that failed and its message, what it was given side by
//! side (where each input comes from, what it is, its grid: size, cells, coordinate system, origin; what differs
//! from the first marked), what likely does not fit and what puts it right; then the checks of the graph (errors,
//! warnings), what was found in the data, the run's warnings. Ask the assistant sends a problem, explained, to the
//! assistant.

use egui::{Margin, RichText, Ui};
use terraflux_graph::Severity;
use terraflux_ops::Registry;

use crate::app::Session;
use crate::diagnose::{Diagnosis, GridInfo};
use crate::theme;

/// What the pane asks of the application.
pub enum Asked {
    /// A problem to the assistant, explained.
    Assistant(String),
}

/// The diagnosis of the last run's failure, if it failed in a node.
pub fn last_failure(s: &Session) -> Option<Diagnosis> {
    let e = s.error.as_ref()?;
    let (at, message) = crate::diagnose::failed_at(e)?;
    let values = |n: &str, p: &str| s.value_of(n, p);
    crate::diagnose::diagnose(&s.model, &at, &message, &values)
}

/// How many problems there are: errors (the run's failure, the graph's and the data's errors) and warnings.
pub fn counts(s: &Session) -> (usize, usize) {
    let errors = usize::from(s.error.is_some())
        + usize::from(s.load_error.is_some())
        + s.shown_issues().filter(|i| i.severity == Severity::Error).count()
        + s.findings.iter().filter(|f| f.level == terraflux_exec::lint::Level::Error).count();
    let warnings = s.warnings.len()
        + s.shown_issues().filter(|i| i.severity == Severity::Warning).count()
        + s.findings.iter().filter(|f| f.level == terraflux_exec::lint::Level::Warning).count();
    (errors, warnings)
}

fn weak(t: impl Into<String>) -> RichText {
    RichText::new(t.into()).color(theme::weak())
}

/// A node's path (`loop/node`) a click away: selected, the canvas in its scope.
fn node_link(ui: &mut Ui, at: &str, select: &mut Option<String>) {
    if ui.link(RichText::new(at).strong()).on_hover_text("select it").clicked() {
        *select = Some(at.to_string());
    }
}

/// What it was given, a row each; what differs from the first map in the colour of a warning.
fn inputs_ui(ui: &mut Ui, d: &Diagnosis, select: &mut Option<String>) {
    let first: Option<&GridInfo> = d.inputs.iter().find_map(|s| s.grid.as_ref());
    let warn = ui.visuals().warn_fg_color;
    let odd = |same: bool, t: String| if same { RichText::new(t) } else { RichText::new(t).color(warn).strong() };
    egui::ScrollArea::horizontal().id_salt(("diagnosis inputs", &d.at)).show(ui, |ui| {
        egui::Grid::new(("diagnosis grid", &d.at)).striped(true).spacing([10.0, 4.0]).show(ui, |ui| {
            for h in ["input", "from", "what", "size", "cells", "coordinate system"] {
                ui.label(weak(h).small());
            }
            ui.end_row();
            for s in &d.inputs {
                ui.label(RichText::new(&s.port).monospace());
                match &s.node {
                    Some(n) => {
                        let scope = d.at.rsplit_once('/').map(|(p, _)| format!("{p}/")).unwrap_or_default();
                        if ui.link(&s.from).on_hover_text("select it").clicked() {
                            *select = Some(format!("{scope}{n}"));
                        }
                    }
                    None => {
                        ui.label(&s.from);
                    }
                }
                let nothing = s.kind.starts_with("gave nothing");
                ui.label(if nothing {
                    RichText::new(&s.kind).color(ui.visuals().error_fg_color)
                } else {
                    RichText::new(&s.kind)
                });
                match (&s.grid, first) {
                    (Some(g), Some(f)) => {
                        ui.label(odd((g.width, g.height) == (f.width, f.height), g.size()));
                        ui.label(odd(g.cell == f.cell, g.cell_size()));
                        ui.label(odd(g.crs == f.crs, g.crs.clone()));
                    }
                    _ => {
                        ui.label("");
                        ui.label("");
                        ui.label("");
                    }
                }
                ui.end_row();
            }
        });
    });
}

/// The Problems pane.
pub fn pane(ui: &mut Ui, s: &mut Session, r: &Registry) -> Option<Asked> {
    let mut asked = None;
    let mut select: Option<String> = None;
    egui::ScrollArea::vertical().id_salt("problems pane").auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(Margin::same(8)).show(ui, |ui| {
            let (errors, warnings) = counts(s);
            theme::section(ui, "Problems");
            if errors + warnings == 0 {
                ui.label(weak("No problems: the graph is checked at every edit, the data when it is read, and a run's failure shows here, explained."));
                return;
            }
            let error = ui.visuals().error_fg_color;
            let warn = ui.visuals().warn_fg_color;
            // the last run's failure, explained
            if let Some(e) = s.error.clone() {
                egui::Frame::new().fill(theme::well()).corner_radius(4.0).inner_margin(Margin::same(8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.label(RichText::new("The last run failed").strong().color(error));
                    match last_failure(s) {
                        Some(d) => {
                            ui.horizontal_wrapped(|ui| {
                                ui.label("at");
                                node_link(ui, &d.at, &mut select);
                                ui.label(weak(&d.op).monospace());
                            });
                            ui.label(RichText::new(&d.message).color(error));
                            if !d.inputs.is_empty() {
                                ui.add_space(4.0);
                                ui.label(weak("What it was given:"));
                                inputs_ui(ui, &d, &mut select);
                            }
                            for h in &d.hints {
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Likely:").strong());
                                    ui.label(h);
                                });
                            }
                            ui.add_space(4.0);
                            ui.horizontal(|ui| {
                                if ui
                                    .add(crate::app::accent_button("Ask the assistant", true))
                                    .on_hover_text("this failure, explained, to the assistant: why, and how to put it right")
                                    .clicked()
                                {
                                    asked = Some(Asked::Assistant(format!(
                                        "The model does not run. Find why and put it right (look at the nodes it \
                                         names; trace_cell or inspect_file when it helps):\n\n{}",
                                        d.text()
                                    )));
                                }
                                if ui.button("Copy").clicked() {
                                    ui.ctx().copy_text(d.text());
                                }
                            });
                        }
                        None => {
                            ui.label(RichText::new(&e).color(error));
                            if ui.add(crate::app::accent_button("Ask the assistant", true)).clicked() {
                                asked = Some(Asked::Assistant(format!("The model does not run: {e}\nFind why and put it right.")));
                            }
                        }
                    }
                });
                ui.add_space(6.0);
            }
            if let Some(e) = &s.load_error {
                ui.label(RichText::new("The data").strong());
                ui.label(RichText::new(e).color(error));
                ui.add_space(6.0);
            }
            // the graph's checks
            let mut issues: Vec<&terraflux_graph::Issue> = s.shown_issues().collect();
            issues.sort_by_key(|i| std::cmp::Reverse(i.severity));
            if !issues.is_empty() {
                ui.label(RichText::new("The graph").strong());
                for i in issues {
                    ui.horizontal_wrapped(|ui| {
                        let colour = if i.severity == Severity::Error { error } else { warn };
                        ui.label(RichText::new("●").color(colour));
                        let node = i.at.split('.').next().unwrap_or("");
                        if !node.is_empty() {
                            node_link(ui, node, &mut select);
                        }
                        ui.label(&i.message);
                    });
                }
                ui.add_space(6.0);
            }
            // the data
            let found: Vec<_> = s.findings.iter().filter(|f| f.level != terraflux_exec::lint::Level::Info).collect();
            if !found.is_empty() {
                ui.label(RichText::new("The data").strong());
                for f in found {
                    ui.horizontal_wrapped(|ui| {
                        let colour = if f.level == terraflux_exec::lint::Level::Error { error } else { warn };
                        ui.label(RichText::new("●").color(colour));
                        ui.label(RichText::new(&f.at).strong());
                        ui.label(&f.message);
                    });
                }
                ui.add_space(6.0);
            }
            // the run's warnings
            if !s.warnings.is_empty() {
                ui.label(RichText::new("The last run's warnings").strong());
                for w in &s.warnings {
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new("●").color(warn));
                        ui.label(w);
                    });
                }
            }
        });
    });
    if let Some(at) = select {
        let mut path: Vec<String> = at.split('/').map(String::from).collect();
        let id = path.pop().unwrap_or_default();
        s.editor.path = path.clone();
        if path.is_empty() {
            crate::builder::select(s, r, &id);
        } else {
            s.fields.clear();
            s.selected = Some(id.clone());
        }
        s.editor.reveal(&id);
    }
    asked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_not_filled_in_is_told_once_a_run_is_asked_for() {
        let r = Registry::standard();
        let mut s = Session::blank();
        let a = crate::builder::add_at(&mut s, &r, "nlm.random", &[], Some([0.0, 0.0])).unwrap();
        let b = crate::builder::add_at(&mut s, &r, "raster.write", &[("path", "out.tif".into())], Some([300.0, 0.0]))
            .unwrap();
        // just added: its input not connected, an error not told yet
        assert!(s.issues.iter().any(|i| i.unfilled), "{:?}", s.issues);
        assert_eq!(counts(&s).0, 0);
        assert!(!s.ask_run());
        // a run asked for: told, until the node is filled in
        assert!(s.unfilled_shown);
        assert!(counts(&s).0 > 0);
        crate::editor::connect(&mut s.source, &r, &[], &format!("{a}.map"), &b, "value").unwrap();
        s.edited();
        assert!(!s.issues.iter().any(|i| i.unfilled), "{:?}", s.issues);
        assert!(!s.unfilled_shown);
    }
}
