//! The Files pane: the computer's folders, to take data to the model from. A raster or vector features dragged to
//! the model's canvas is a node that reads it where it is let go (a double click: after the node selected); a
//! folder of rasters read as one stack; a model (`.toml`) opened. Only the data is listed, unless all the files
//! are asked for; the folder shown is kept between sessions.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use egui::{Color32, RichText, Ui, Vec2};
use terraflux_graph::Type;
use terraflux_ops::Registry;

use crate::app::Session;
use crate::builder::{DraggedFile, add_at, add_file, is_raster, is_vector};
use crate::theme;

/// What a file is, for the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Folder,
    Raster,
    Vector,
    Model,
    Other,
}

struct Entry {
    name: String,
    path: PathBuf,
    kind: Kind,
    size: u64,
}

/// The pane's state: the folder shown and what is in it (read again every few seconds, and when asked).
#[derive(Default)]
pub struct Files {
    dir: Option<PathBuf>,
    entries: Vec<Entry>,
    read_at: Option<Instant>,
    error: Option<String>,
    filter: String,
    /// every file, not only the data
    all: bool,
    /// the folder typed in the path's field
    typed: Option<String>,
    chosen: Option<PathBuf>,
    drives: Vec<PathBuf>,
}

/// What the pane asks of the application.
pub enum Asked {
    /// a model to open
    Open(PathBuf),
}

fn kind_of(p: &Path, dir: bool) -> Kind {
    if dir {
        Kind::Folder
    } else if is_vector(p) {
        Kind::Vector
    } else if is_raster(p) {
        Kind::Raster
    } else if p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")) {
        Kind::Model
    } else {
        Kind::Other
    }
}

/// The user's folder (the documents', if there is one).
fn home() -> Option<PathBuf> {
    let h = PathBuf::from(std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME"))?);
    let docs = h.join("Documents");
    Some(if docs.is_dir() { docs } else { h })
}

fn size_text(n: u64) -> String {
    match n {
        n if n >= 1 << 30 => format!("{:.1} GB", n as f64 / (1u64 << 30) as f64),
        n if n >= 1 << 20 => format!("{:.1} MB", n as f64 / (1u64 << 20) as f64),
        n if n >= 1 << 10 => format!("{:.0} KB", n as f64 / 1024.0),
        n => format!("{n} B"),
    }
}

impl Files {
    /// The folder shown: the one kept, else the model's, else the user's.
    fn dir(&mut self, model: Option<&Path>) -> PathBuf {
        if self.dir.is_none() {
            let kept = crate::settings::get().files_dir.filter(|d| d.is_dir());
            let of_model = model.and_then(|m| m.parent()).filter(|d| d.is_dir()).map(Path::to_path_buf);
            self.dir = kept.or(of_model).or_else(home).or_else(|| std::env::current_dir().ok());
        }
        self.dir.clone().unwrap_or_default()
    }

    /// Show another folder (kept for the next session).
    pub fn go(&mut self, dir: PathBuf) {
        crate::settings::update(|s| s.files_dir = Some(dir.clone()));
        self.dir = Some(dir);
        self.read_at = None;
        self.chosen = None;
        self.typed = None;
    }

    fn read(&mut self, dir: &Path) {
        self.read_at = Some(Instant::now());
        self.entries.clear();
        self.error = None;
        let list = match std::fs::read_dir(dir) {
            Ok(l) => l,
            Err(e) => {
                self.error = Some(e.to_string());
                return;
            }
        };
        for e in list.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let path = e.path();
            let meta = std::fs::metadata(&path).ok();
            let is_dir = meta.as_ref().is_some_and(|m| m.is_dir());
            let size = meta.as_ref().map_or(0, |m| if is_dir { 0 } else { m.len() });
            self.entries.push(Entry { kind: kind_of(&path, is_dir), name, path, size });
        }
        self.entries.sort_by(|a, b| {
            (a.kind != Kind::Folder)
                .cmp(&(b.kind != Kind::Folder))
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
    }
}

/// The Files pane.
pub fn pane(ui: &mut Ui, f: &mut Files, s: &mut Session, r: &Registry) -> Option<Asked> {
    let mut asked = None;
    let dir = f.dir(s.path.as_deref());
    if f.read_at.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
        f.read(&dir);
    }
    if cfg!(windows) && f.drives.is_empty() {
        f.drives = (b'C'..=b'Z').map(|d| PathBuf::from(format!("{}:\\", d as char))).filter(|p| p.is_dir()).collect();
    }
    egui::Frame::new().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        // where: up, the user's folder, the model's, the drives; the path, to type another
        ui.horizontal(|ui| {
            if ui.small_button("⏶").on_hover_text("the folder above").clicked()
                && let Some(up) = dir.parent()
            {
                f.go(up.to_path_buf());
            }
            if ui.small_button("~").on_hover_text("your documents").clicked()
                && let Some(h) = home()
            {
                f.go(h);
            }
            if let Some(m) = s.path.as_deref().and_then(Path::parent)
                && ui.small_button("model").on_hover_text("the model's folder").clicked()
            {
                f.go(m.to_path_buf());
            }
            if f.drives.len() > 1 {
                ui.menu_button("🖴", |ui| {
                    for d in f.drives.clone() {
                        if ui.button(d.to_string_lossy()).clicked() {
                            f.go(d);
                            ui.close();
                        }
                    }
                })
                .response
                .on_hover_text("the drives");
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("⟳").on_hover_text("read the folder again").clicked() {
                    f.read_at = None;
                }
            });
        });
        let mut typed = f.typed.clone().unwrap_or_else(|| dir.to_string_lossy().into_owned());
        let resp =
            ui.add(egui::TextEdit::singleline(&mut typed).desired_width(f32::INFINITY).font(egui::TextStyle::Small));
        if resp.changed() {
            f.typed = Some(typed.clone());
        }
        if resp.lost_focus() {
            let p = PathBuf::from(typed.trim());
            if ui.input(|i| i.key_pressed(egui::Key::Enter)) && p.is_dir() {
                f.go(p);
            } else {
                f.typed = None;
            }
        }
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut f.filter)
                    .hint_text("filter")
                    .desired_width(ui.available_width() - 70.0),
            );
            ui.checkbox(&mut f.all, "all").on_hover_text("every file, not only rasters, vectors and models");
        });
        ui.add_space(2.0);
        if let Some(e) = &f.error {
            ui.colored_label(ui.visuals().warn_fg_color, e);
        }
        let filter = f.filter.to_lowercase();
        let shown: Vec<usize> = (0..f.entries.len())
            .filter(|&i| {
                let e = &f.entries[i];
                (f.all || e.kind != Kind::Other) && (filter.is_empty() || e.name.to_lowercase().contains(&filter))
            })
            .collect();
        if shown.is_empty() && f.error.is_none() {
            ui.label(RichText::new("no data in this folder").weak());
        }
        let dark = ui.visuals().dark_mode;
        let colour = |k: Kind, ui: &Ui| -> Color32 {
            let ty = match k {
                Kind::Raster => Some(Type::Map),
                Kind::Vector => Some(Type::Vector),
                _ => None,
            };
            match k {
                Kind::Model => theme::accent(),
                Kind::Folder => ui.visuals().strong_text_color(),
                Kind::Other => ui.visuals().weak_text_color(),
                _ => ty.and_then(|t| crate::editor_view::type_colour(t, dark)).unwrap_or(ui.visuals().text_color()),
            }
        };
        let mut go = None;
        let row = 20.0;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show_rows(ui, row, shown.len(), |ui, range| {
            // a name too long for the pane cut, not wrapped (the rows of one height)
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Truncate);
            for &i in &shown[range] {
                let e = &f.entries[i];
                let (mark, note) = match e.kind {
                    Kind::Folder => ("🗀", String::new()),
                    Kind::Raster => ("🗺", size_text(e.size)),
                    Kind::Vector => ("■", size_text(e.size)),
                    Kind::Model => ("🖹", "model".to_string()),
                    Kind::Other => ("🖹", size_text(e.size)),
                };
                let text = RichText::new(format!("{mark}  {}", e.name)).color(colour(e.kind, ui));
                let data = matches!(e.kind, Kind::Raster | Kind::Vector);
                let sense = if data { egui::Sense::click_and_drag() } else { egui::Sense::click() };
                let resp = ui.add(
                    egui::Button::selectable(f.chosen.as_ref() == Some(&e.path), text)
                        .right_text(RichText::new(note).weak().size(10.5))
                        .min_size(Vec2::new(ui.available_width(), row - 2.0))
                        .sense(sense),
                );
                if data {
                    resp.dnd_set_drag_payload(DraggedFile(e.path.clone()));
                }
                let resp = match e.kind {
                    Kind::Folder => resp.on_hover_text("open the folder (right-click: its rasters as one stack)"),
                    Kind::Raster | Kind::Vector => {
                        resp.on_hover_text("drag it to the model, or double-click it, for a node that reads it")
                    }
                    Kind::Model => resp.on_hover_text("double-click to open the model"),
                    Kind::Other => resp,
                };
                if resp.clicked() {
                    match e.kind {
                        Kind::Folder => go = Some(e.path.clone()),
                        _ => f.chosen = Some(e.path.clone()),
                    }
                }
                if resp.double_clicked() {
                    match e.kind {
                        Kind::Raster | Kind::Vector | Kind::Other => {
                            if let Err(m) = add_file(s, r, &e.path, None) {
                                s.editor.message = Some(m);
                            }
                        }
                        Kind::Model => asked = Some(Asked::Open(e.path.clone())),
                        Kind::Folder => {}
                    }
                }
                resp.context_menu(|ui| match e.kind {
                    Kind::Folder => {
                        if ui.button("Read its rasters as one stack").clicked() {
                            let path = e.path.to_string_lossy().into_owned();
                            if let Err(m) = add_at(s, r, "raster.read", &[("path", path.into())], None) {
                                s.editor.message = Some(m);
                            }
                            ui.close();
                        }
                    }
                    Kind::Model => {
                        if ui.button("Open the model").clicked() {
                            asked = Some(Asked::Open(e.path.clone()));
                            ui.close();
                        }
                    }
                    _ => {
                        if ui.button("Add to the model").clicked() {
                            if let Err(m) = add_file(s, r, &e.path, None) {
                                s.editor.message = Some(m);
                            }
                            ui.close();
                        }
                    }
                });
            }
        });
        if let Some(d) = go {
            f.go(d);
        }
    });
    // the file carried, by the pointer
    if let Some(p) = egui::DragAndDrop::payload::<DraggedFile>(ui.ctx())
        && let Some(at) = ui.input(|i| i.pointer.latest_pos())
    {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
        let name = p.0.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        egui::Area::new(egui::Id::new("file carried"))
            .fixed_pos(at + Vec2::new(14.0, 10.0))
            .order(egui::Order::Tooltip)
            .show(ui.ctx(), |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.label(RichText::new(name).strong());
                    ui.label(RichText::new("to the model's canvas").weak().size(10.5));
                });
            });
    }
    asked
}
