//! *Pack for sharing...* and *Open package...*: the model with its data in one `.tfpack` file (see
//! [`terraflux_exec::pack`]), to send to a supervisor or a reviewer, and the package opened: unpacked into a folder
//! (by default beside it, named like it), every file checked against the manifest, the model opened. A package
//! dropped on the window is unpacked at once, into that folder. A package with a file that does not check is not
//! opened: what differs is listed.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};

use egui::RichText;
use terraflux_exec::pack::{self, Manifest};

use crate::theme;

/// What the window is for.
pub enum State {
    Pack(Packing),
    Open(Opening),
}

/// A model being packed.
pub struct Packing {
    model: PathBuf,
    /// The package to write.
    pub dest: String,
    /// The outputs of the last run go in too.
    pub outputs: bool,
    /// The outputs of the last run (files that are there).
    found: Vec<PathBuf>,
    /// What would be packed, in words (or why it cannot be).
    pub plan: Result<String, String>,
    job: Option<Receiver<Result<(u64, String), String>>>,
    pub said: Option<String>,
}

/// A package being opened.
pub struct Opening {
    package: PathBuf,
    /// The folder to unpack into.
    pub folder: String,
    job: Option<Receiver<Result<Unpacked, String>>>,
    pub problems: Vec<String>,
    pub told: Vec<String>,
}

/// What an unpacking came to.
struct Unpacked {
    model: PathBuf,
    manifest: Manifest,
    problems: Vec<String>,
}

impl Packing {
    fn refresh(&mut self) {
        let extra = if self.outputs { self.found.clone() } else { Vec::new() };
        self.plan = pack::plan(&self.model, &extra).map(|p| pack::words(&p)).map_err(|e| e.to_string());
    }
}

impl State {
    /// The window to pack the model in a file (saved: the file is what is packed).
    pub fn pack(model: &Path) -> State {
        let found = pack::last_outputs(model);
        let mut p = Packing {
            model: model.to_path_buf(),
            dest: pack::default_package(model).to_string_lossy().into_owned(),
            outputs: false,
            found,
            plan: Ok(String::new()),
            job: None,
            said: None,
        };
        p.refresh();
        State::Pack(p)
    }

    /// The window to open a package; with `start`, the unpacking begins at once into the default folder (a package
    /// dropped on the window).
    pub fn open_package(package: &Path, start: bool) -> State {
        let mut o = Opening {
            package: package.to_path_buf(),
            folder: pack::default_folder(package).to_string_lossy().into_owned(),
            job: None,
            problems: Vec::new(),
            told: Vec::new(),
        };
        if start {
            o.start();
        }
        State::Open(o)
    }
}

impl Opening {
    fn start(&mut self) {
        let (tx, rx) = channel();
        let (package, folder) = (self.package.clone(), PathBuf::from(&self.folder));
        std::thread::spawn(move || {
            let r = pack::unpack(&package, &folder).map_err(|e| e.to_string()).map(|u| Unpacked {
                model: u.model,
                manifest: u.manifest,
                problems: u.problems,
            });
            let _ = tx.send(r);
        });
        self.job = Some(rx);
        self.problems.clear();
        self.told.clear();
    }
}

/// Draw the window (when there is one); the model to open once it is unpacked.
pub fn window(ctx: &egui::Context, state: &mut Option<State>) -> Option<PathBuf> {
    let mut st = state.take()?;
    let mut keep = true;
    let mut model = None;
    let title = if matches!(st, State::Pack(_)) { "Pack for sharing" } else { "Open package" };
    let mut open = true;
    egui::Window::new(title).open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        ui.set_min_width(460.0);
        match &mut st {
            State::Pack(p) => keep = packing(ui, p),
            State::Open(o) => match opening(ui, o) {
                Step::Stay => {}
                Step::Close => keep = false,
                Step::Model(m) => {
                    model = Some(m);
                    keep = false;
                }
            },
        }
    });
    if keep && open {
        *state = Some(st);
    }
    model
}

fn packing(ui: &mut egui::Ui, p: &mut Packing) -> bool {
    let (weak, warn) = (theme::weak(), ui.visuals().warn_fg_color);
    ui.label(
        RichText::new(
            "The model with the files its nodes read, their paths made relative, in one file to send: whoever \
             opens it runs the model without changing anything.",
        )
        .small()
        .color(weak),
    );
    if let Some(rx) = &p.job {
        match rx.try_recv() {
            Ok(r) => {
                p.job = None;
                p.said = Some(match r {
                    Ok((size, to)) => format!("wrote {to} ({})", pack::size_text(size)),
                    Err(e) => format!("not packed: {e}"),
                });
            }
            Err(_) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)),
        }
    }
    ui.add_space(4.0);
    match &p.plan {
        Ok(words) => {
            ui.label(words.as_str());
        }
        Err(e) => {
            ui.label(RichText::new(e.as_str()).color(warn));
        }
    }
    let n = p.found.len();
    let hover = "the files the last run wrote (from the journal, else the `output` folder beside the model)";
    let before = p.outputs;
    ui.add_enabled(n > 0, egui::Checkbox::new(&mut p.outputs, format!("Include the last outputs ({n} files)")))
        .on_hover_text(hover)
        .on_disabled_hover_text("the model has not run (no outputs found)");
    if p.outputs != before {
        p.refresh();
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        ui.label("Write to");
        ui.add(egui::TextEdit::singleline(&mut p.dest).desired_width(300.0));
        if ui.button("Browse...").clicked() {
            let start = Path::new(&p.dest);
            let mut d = rfd::FileDialog::new().add_filter("Terraflux package", &[pack::EXTENSION]);
            if let Some(dir) = start.parent().filter(|d| d.is_dir()) {
                d = d.set_directory(dir);
            }
            if let Some(f) =
                d.set_file_name(start.file_name().map_or(String::new(), |n| n.to_string_lossy().into())).save_file()
            {
                p.dest = f.to_string_lossy().into_owned();
            }
        }
    });
    if let Some(said) = &p.said {
        ui.label(said.as_str());
    }
    let mut keep = true;
    ui.horizontal(|ui| {
        let busy = p.job.is_some();
        let ok = p.plan.is_ok() && !p.dest.trim().is_empty() && !busy;
        if ui.add_enabled(ok, crate::app::accent_button("Pack", true)).clicked() {
            let (model, dest) = (p.model.clone(), PathBuf::from(p.dest.trim()));
            let extra = if p.outputs { p.found.clone() } else { Vec::new() };
            let (tx, rx) = channel();
            std::thread::spawn(move || {
                let r = pack::plan(&model, &extra)
                    .and_then(|plan| plan.write(&dest))
                    .map(|(_, size)| (size, dest.display().to_string()))
                    .map_err(|e| e.to_string());
                let _ = tx.send(r);
            });
            p.job = Some(rx);
            p.said = None;
        }
        if busy {
            ui.spinner();
            ui.label(RichText::new("packing...").color(weak));
        }
        if ui.button("Close").clicked() {
            keep = false;
        }
    });
    keep
}

enum Step {
    Stay,
    Close,
    Model(PathBuf),
}

fn opening(ui: &mut egui::Ui, o: &mut Opening) -> Step {
    let (weak, warn) = (theme::weak(), ui.visuals().warn_fg_color);
    ui.label(format!("Package: {}", o.package.display()));
    let mut step = Step::Stay;
    if let Some(rx) = &o.job {
        match rx.try_recv() {
            Ok(r) => {
                o.job = None;
                match r {
                    Ok(u) => {
                        o.told = u.manifest.missing.iter().map(|m| format!("{m}")).collect();
                        if u.problems.is_empty() {
                            step = Step::Model(u.model);
                        }
                        o.problems = u.problems;
                    }
                    Err(e) => o.problems = vec![e],
                }
            }
            Err(_) => ui.ctx().request_repaint_after(std::time::Duration::from_millis(100)),
        }
    }
    let busy = o.job.is_some();
    ui.add_enabled_ui(!busy, |ui| {
        ui.horizontal(|ui| {
            ui.label("Unpack into");
            ui.add(egui::TextEdit::singleline(&mut o.folder).desired_width(300.0));
            if ui.button("Browse...").clicked() {
                let mut d = rfd::FileDialog::new();
                if let Some(dir) = o.package.parent().filter(|d| d.is_dir()) {
                    d = d.set_directory(dir);
                }
                if let Some(f) = d.pick_folder() {
                    o.folder = f.to_string_lossy().into_owned();
                }
            }
        });
    });
    if busy {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(RichText::new("unpacking and checking the files...").color(weak));
        });
    }
    if !o.problems.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("The package is not as it was made, so it is not opened:").color(warn));
        for p in &o.problems {
            ui.label(RichText::new(p.as_str()).small().color(warn));
        }
    }
    if !o.told.is_empty() {
        ui.add_space(4.0);
        ui.label(RichText::new("Not found when the package was made:").small().color(weak));
        for t in &o.told {
            ui.label(RichText::new(t.as_str()).small().color(weak));
        }
    }
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let ok = !busy && !o.folder.trim().is_empty();
        if ui.add_enabled(ok, crate::app::accent_button("Unpack and open", true)).clicked() {
            o.start();
        }
        if ui.button("Close").clicked() {
            step = Step::Close;
        }
    });
    step
}
