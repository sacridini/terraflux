//! The start page: what the Model pane's canvas shows over its empty area while the model has no node, a way into
//! the first model (from the user's data, from a task's template, from a model of before) and into the
//! documentation. It is drawn only while the model is empty and the setting is on, so a canvas with nodes pays
//! nothing for it; what it asks (a command of the palette, a template of the user's) is returned for the
//! application to do, as the menus do.

use std::path::PathBuf;

use egui::{Rect, RichText, Ui, UiBuilder, Vec2};

use crate::commands::Do;

/// The widest the page is, and the most recent models it lists.
const WIDTH: f32 = 760.0;
const RECENT: usize = 6;

/// What the page asks of the application.
pub enum Act {
    /// A command of the palette (open data, the Wizard, a template, a recent model...).
    Do(Do),
    /// One of the user's own templates (a model file), started as a new model.
    User(PathBuf),
}

/// The lists kept between frames, so that no file is looked at on every frame: the recent models that exist and
/// the user's templates, looked at again when the list of recent models changes or after a second.
#[derive(Clone, Default)]
struct Cache {
    at: f64,
    asked: Vec<PathBuf>,
    recent: Vec<PathBuf>,
    mine: Vec<(String, PathBuf)>,
}

/// Where the page closed for this run is noted (in egui's memory, never saved: the next start shows it again).
fn closed_id() -> egui::Id {
    egui::Id::new("start page closed")
}

/// Whether the page was closed in this run (its *Close* button, Esc).
pub fn closed(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(closed_id())).unwrap_or(false)
}

/// The page closed until the application starts again, or shown again (Help > Start page).
pub fn set_closed(ctx: &egui::Context, closed: bool) {
    ctx.data_mut(|d| d.insert_temp(closed_id(), closed));
}

/// Whether the page shows for a model: no node, no input, at the top of the model, and the text view not on.
pub fn wanted(s: &crate::app::Session) -> bool {
    s.source.nodes.is_empty() && s.source.inputs.is_empty() && s.editor.path.is_empty() && !s.editor.text
}

/// The columns of the groups' grid for a width.
fn columns(width: f32) -> usize {
    if width >= 680.0 {
        3
    } else if width >= 440.0 {
        2
    } else {
        1
    }
}

/// A heading of a part of the page.
fn heading(ui: &mut Ui, text: &str) {
    ui.add_space(10.0);
    ui.label(RichText::new(text).strong().size(15.0));
    ui.add_space(3.0);
}

/// The page, over the `canvas` (its column centred; the sides stay the canvas's); what was asked, if anything.
pub fn show(ui: &mut Ui, canvas: Rect) -> Option<Act> {
    let st = crate::settings::get();
    let (mut on, now) = (st.start_page, ui.input(|i| i.time));
    if !on || closed(ui.ctx()) {
        return None;
    }
    // (Esc closes it, unless a text field has the keys)
    if !ui.ctx().egui_wants_keyboard_input() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
        set_closed(ui.ctx(), true);
        return None;
    }
    let id = egui::Id::new("start page cache");
    let raw = st.recent_models;
    let mut cache: Cache = ui.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    if cache.asked != raw || now - cache.at >= 1.0 || now < cache.at {
        cache = Cache {
            at: now,
            recent: raw.iter().filter(|p| p.is_file()).take(RECENT).cloned().collect(),
            asked: raw,
            mine: crate::templates::user_templates(),
        };
        ui.data_mut(|d| d.insert_temp(id, cache.clone()));
    }
    let width = (canvas.width() - 24.0).min(WIDTH);
    if width < 200.0 {
        return None;
    }
    let column = Rect::from_min_size(
        egui::pos2(canvas.center().x - width / 2.0, canvas.top() + 8.0),
        Vec2::new(width, (canvas.height() - 16.0).max(40.0)),
    );
    let mut act = None;
    let mut page = ui.new_child(UiBuilder::new().max_rect(column).layout(egui::Layout::top_down(egui::Align::Min)));
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .stroke(ui.visuals().window_stroke)
        .corner_radius(8)
        .inner_margin(egui::Margin::same(10))
        .show(&mut page, |ui| {
            // (the frame is as wide as the column: the scroll area inside it takes the height left)
            ui.set_width(width - 20.0);
            egui::ScrollArea::vertical()
                .id_salt("start page scroll")
                .max_height((ui.available_height() - 40.0).max(60.0))
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Start a model").heading());
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .button("Close")
                                .on_hover_text(
                                    "the bare canvas until Terraflux starts again (Esc); Help > Start page shows it",
                                )
                                .clicked()
                            {
                                set_closed(ui.ctx(), true);
                            }
                        });
                    });
                    ui.label(
                        RichText::new(
                            "Start from your data or from a task; every template runs as it opens, its data drawn \
                         by neutral landscape models.",
                        )
                        .weak(),
                    );
                    heading(ui, "Start from your data");
                    ui.horizontal_wrapped(|ui| {
                        if ui
                            .button("Open a raster or vectors...")
                            .on_hover_text("a node that reads the file you choose, in the model")
                            .clicked()
                        {
                            act = Some(Act::Do(Do::OpenData));
                        }
                        if ui
                            .button("Wizard: a land change model from your maps")
                            .on_hover_text("asks for the maps of two dates and the drivers, and makes the model")
                            .clicked()
                        {
                            act = Some(Act::Do(Do::Wizard));
                        }
                        if ui.button("Open a model...").on_hover_text("a model file (.toml or .json)").clicked() {
                            act = Some(Act::Do(Do::Open));
                        }
                    });
                    ui.label(RichText::new("or drop rasters and vectors on the window").weak().size(11.0));
                    if !cache.recent.is_empty() {
                        heading(ui, "Recent models");
                        for p in &cache.recent {
                            let name =
                                p.file_name().map_or(p.display().to_string(), |n| n.to_string_lossy().into_owned());
                            let folder = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
                            ui.horizontal(|ui| {
                                if ui
                                    .add(egui::Button::new(name).small())
                                    .on_hover_text(p.display().to_string())
                                    .clicked()
                                {
                                    act = Some(Act::Do(Do::OpenRecent(p.clone())));
                                }
                                ui.add(egui::Label::new(RichText::new(folder).weak().size(11.0)).truncate());
                            });
                        }
                    }
                    heading(ui, "Learn");
                    ui.horizontal_wrapped(|ui| {
                        for (name, path, about) in crate::docs::PAGES.iter().take(3) {
                            if ui.link(*name).on_hover_text(*about).clicked() {
                                crate::docs::open(&crate::docs::page(path));
                            }
                        }
                        if ui
                            .link("Tutorials with a model")
                            .on_hover_text("the tutorials that come with a model, to open here")
                            .clicked()
                        {
                            act = Some(Act::Do(Do::TutorialModels));
                        }
                    });
                    heading(ui, "Start from a task");
                    let cols = columns(ui.available_width());
                    for row in crate::templates::GROUPS.chunks(cols) {
                        // (the columns sit at equal widths, whatever the cards hold)
                        ui.columns(cols, |uis| {
                            for (ui, g) in uis.iter_mut().zip(row) {
                                crate::theme::card(ui, g, |ui| {
                                    for (k, t) in crate::templates::TEMPLATES.iter().enumerate() {
                                        if t.group != *g {
                                            continue;
                                        }
                                        ui.horizontal_wrapped(|ui| {
                                            if ui
                                                .add(egui::Button::new(t.name).small())
                                                .on_hover_text(t.about)
                                                .clicked()
                                            {
                                                act = Some(Act::Do(Do::Template(k)));
                                            }
                                            if let Some(tut) = crate::tutorials::for_template(t.name) {
                                                let link = egui::Label::new(
                                                    RichText::new("tutorial")
                                                        .small()
                                                        .color(ui.visuals().hyperlink_color),
                                                );
                                                if ui
                                                    .add(link)
                                                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                                                    .on_hover_text("read the tutorial of this template")
                                                    .interact(egui::Sense::click())
                                                    .clicked()
                                                {
                                                    crate::docs::open(&crate::tutorials::url(tut));
                                                }
                                            }
                                        });
                                    }
                                });
                            }
                        });
                    }
                    if !cache.mine.is_empty() {
                        crate::theme::card(ui, "Yours", |ui| {
                            ui.horizontal_wrapped(|ui| {
                                for (name, path) in &cache.mine {
                                    if ui
                                        .add(egui::Button::new(name).small())
                                        .on_hover_text(path.display().to_string())
                                        .clicked()
                                    {
                                        act = Some(Act::User(path.clone()));
                                    }
                                }
                            });
                        });
                    }
                });
            // (under the scrolling part, always in view)
            ui.separator();
            if ui
                .add(crate::theme::toggle(&mut on, "Show this page when the model is empty"))
                .on_hover_text("also in Settings > Model")
                .changed()
            {
                crate::settings::update(|s| s.start_page = on);
            }
        });
    act
}
