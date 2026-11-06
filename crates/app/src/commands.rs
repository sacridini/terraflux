//! The palette of commands (ctrl+K, ctrl+shift+P): everything the application does, found by typing: its actions,
//! the templates, the models opened last, the nodes of the scope shown (to go to one) and the operators (to add
//! one). The arrows choose, enter does it, escape closes it. And the sheet of the keyboard's shortcuts (F1).

use std::path::PathBuf;

use egui::{Margin, RichText, Vec2};

use crate::app::Pane;

/// The palette, while it is open.
#[derive(Debug, Default)]
pub struct Commands {
    pub query: String,
    /// The command chosen among those found.
    pub at: usize,
    /// Whether the search was given the keyboard's focus.
    focused: bool,
}

/// What a command does.
#[derive(Debug, Clone, PartialEq)]
pub enum Do {
    New,
    Open,
    OpenRecent(PathBuf),
    Save,
    SaveAs,
    /// Save a version...: the model kept under a name.
    Version,
    /// Clean outputs...: delete the files the runs wrote.
    Clean,
    OpenData,
    /// Process a folder...: every raster of a folder through an operator.
    ProcessFolder,
    Run,
    /// A check run: on the Viewer's view or the whole map, one cell in every `coarsen`.
    Check {
        view: bool,
        coarsen: u32,
    },
    Stop,
    RunToHere,
    Arrange,
    /// Group the nodes selected, take the group selected apart, group the model by stage.
    Group,
    Ungroup,
    GroupByStage,
    Undo,
    Redo,
    /// The History window: the model's edits by name, to go back or forward to any.
    History,
    /// The next model open (its tab), and the model in front closed.
    NextTab,
    CloseTab,
    Text,
    Settings,
    Shortcuts,
    /// The window of the model's methods and citations.
    Methods,
    /// The window of the export to QGIS.
    Qgis,
    /// The data catalog's window.
    Catalog,
    /// The model as a Python script.
    Python,
    Wizard,
    Pane(Pane),
    ResetLayout,
    /// A template, by its index in [`crate::templates::TEMPLATES`].
    Template(usize),
    /// Add an operator after the node selected.
    Add(String),
    /// Save the node selected (a group or a calculator) as a node of the user's library; add one of the library's
    /// (its name) after the node selected.
    SaveNode,
    AddMine(String),
    /// Select a node (the containers' ids from the top, then its own), opening its containers, and look at it.
    GoTo(Vec<String>),
    /// Go to a place kept in the model, of the Viewer's map or of the canvas, by its name.
    Place {
        map: bool,
        name: String,
    },
}

/// A command: what it is called, what it is (an action, a template, a node, an operator), its key or what it does.
#[derive(Debug, Clone)]
pub struct Command {
    pub label: String,
    pub kind: &'static str,
    pub detail: String,
    pub act: Do,
}

impl Command {
    pub fn new(label: impl Into<String>, kind: &'static str, detail: impl Into<String>, act: Do) -> Command {
        Command { label: label.into(), kind, detail: detail.into(), act }
    }
}

/// The modifier of the shortcuts as the system names it (⌘ on macOS).
pub fn cmd() -> &'static str {
    if cfg!(target_os = "macos") { "⌘" } else { "ctrl+" }
}

/// The commands a search finds, the best first: every word of it in the label (or in what an operator does);
/// those whose label starts with it, then whose label has it, then the others; within a rank, in their order.
pub fn find<'a>(all: &'a [Command], query: &str) -> Vec<&'a Command> {
    let query = query.trim().to_lowercase();
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut found: Vec<(u8, &Command)> = all
        .iter()
        .filter_map(|c| {
            let label = c.label.to_lowercase();
            let detail = c.detail.to_lowercase();
            let kind = c.kind.to_lowercase();
            let has = |w: &str| {
                label.contains(w)
                    || kind.contains(w)
                    || (matches!(c.kind, "operator" | "my node") && detail.contains(w))
            };
            if !words.iter().all(|w| has(w)) {
                return None;
            }
            let rank = if query.is_empty() || label.starts_with(&query) {
                0
            } else if label.contains(&query) {
                1
            } else if words.iter().all(|w| label.contains(w)) {
                2
            } else {
                3
            };
            Some((rank, c))
        })
        .collect();
    found.sort_by_key(|(rank, _)| *rank);
    found.into_iter().map(|(_, c)| c).collect()
}

/// The palette over the panes: the command chosen, if one was; `None` in `open` when it closed.
pub fn show(ctx: &egui::Context, open: &mut Option<Commands>, all: &[Command]) -> Option<Do> {
    let mut q = open.take()?;
    let found = find(all, &q.query);
    let (down, up, enter, escape) = ctx.input_mut(|i| {
        (
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown),
            i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Enter),
            i.consume_key(egui::Modifiers::NONE, egui::Key::Escape),
        )
    });
    if down {
        q.at = (q.at + 1).min(found.len().saturating_sub(1));
    }
    if up {
        q.at = q.at.saturating_sub(1);
    }
    let mut chosen = (enter && !found.is_empty()).then(|| found[q.at.min(found.len() - 1)].act.clone());
    let mut close = escape || chosen.is_some();
    let screen = ctx.content_rect();
    let width = 580.0_f32.min(screen.width() - 40.0);
    let weak = crate::theme::weak();
    let area = egui::Area::new(egui::Id::new("commands"))
        .order(egui::Order::Foreground)
        .fixed_pos(egui::pos2(screen.center().x - width / 2.0, screen.top() + 70.0))
        .show(ctx, |ui| {
            egui::Frame::popup(ui.style()).inner_margin(Margin::same(10)).show(ui, |ui| {
                ui.set_width(width);
                let edit = ui.add(
                    egui::TextEdit::singleline(&mut q.query)
                        .hint_text("a command, a template, a node or an operator...")
                        .desired_width(f32::INFINITY),
                );
                if !q.focused {
                    edit.request_focus();
                    q.focused = true;
                }
                if edit.changed() {
                    q.at = 0;
                }
                ui.add_space(4.0);
                if found.is_empty() {
                    ui.label(RichText::new("nothing found").color(weak));
                }
                let at = q.at.min(found.len().saturating_sub(1));
                egui::ScrollArea::vertical().max_height(400.0).auto_shrink([false, true]).show(ui, |ui| {
                    // (a few hundred at most: the operators are many)
                    for (k, c) in found.iter().enumerate().take(300) {
                        let on = k == at;
                        let detail = if c.kind == "operator" { String::new() } else { c.detail.clone() };
                        let resp = ui.add(
                            egui::Button::selectable(on, RichText::new(&c.label))
                                .right_text(RichText::new(format!("{detail}   {}", c.kind)).size(11.0).color(weak))
                                .min_size(Vec2::new(ui.available_width(), 22.0)),
                        );
                        if on && (up || down) {
                            resp.scroll_to_me(None);
                        }
                        let resp = if c.kind == "operator" { resp.on_hover_text(&c.detail) } else { resp };
                        if resp.clicked() {
                            chosen = Some(c.act.clone());
                            close = true;
                        }
                    }
                });
                // what the operator chosen does
                if let Some(c) = found.get(at).filter(|c| c.kind == "operator") {
                    ui.separator();
                    ui.label(RichText::new(&c.detail).size(11.5).color(weak));
                }
                ui.add_space(2.0);
                ui.label(RichText::new("up, down: choose · enter: do it · esc: close").size(10.5).color(weak));
            });
        });
    // a click elsewhere closes it
    if ctx.input(|i| i.pointer.any_pressed()) && !area.response.contains_pointer() {
        close = true;
    }
    if close {
        ctx.memory_mut(|m| {
            if let Some(f) = m.focused() {
                m.surrender_focus(f);
            }
        });
    } else {
        *open = Some(q);
    }
    chosen
}

/// The keyboard's shortcuts, by where they work: the mouse's first (not in the table), then the table's with the
/// shortcuts the user gave them ([`crate::keymap`]).
pub fn shortcuts() -> Vec<(&'static str, Vec<(String, &'static str)>)> {
    crate::keymap::Scope::ALL
        .into_iter()
        .map(|scope| {
            let mouse: &[(&str, &'static str)] = match scope {
                crate::keymap::Scope::Canvas => &[
                    ("click", "select a node (shift or ctrl: one more)"),
                    ("drag", "select the nodes in a box; on a node: move the nodes selected"),
                    ("right or middle drag", "move the view"),
                    ("wheel", "zoom"),
                    ("double click", "a node's output in the Viewer; a container: its body"),
                ],
                crate::keymap::Scope::Viewer => {
                    &[("wheel, drag", "zoom, move"), ("double click", "the whole map in view")]
                }
                _ => &[],
            };
            let mut rows: Vec<(String, &'static str)> = mouse.iter().map(|(k, w)| (k.to_string(), *w)).collect();
            rows.extend(crate::keymap::of_scope(scope));
            (scope.title(), rows)
        })
        .collect()
}

/// The window of the shortcuts, while `open`.
pub fn shortcuts_window(ctx: &egui::Context, open: &mut bool) {
    let weak = crate::theme::weak();
    let high = (ctx.content_rect().height() - 120.0).max(200.0);
    egui::Window::new("Keyboard shortcuts").open(open).resizable(false).collapsible(false).show(ctx, |ui| {
        egui::ScrollArea::vertical().max_height(high).auto_shrink([true, true]).show(ui, |ui| {
            for (where_, keys) in shortcuts() {
                crate::theme::section(ui, where_);
                egui::Grid::new(("shortcuts", where_)).num_columns(2).spacing([18.0, 3.0]).show(ui, |ui| {
                    for (k, what) in keys {
                        ui.label(RichText::new(k).monospace().strong());
                        ui.label(RichText::new(what).color(weak));
                        ui.end_row();
                    }
                });
                ui.add_space(4.0);
            }
        });
    });
}
