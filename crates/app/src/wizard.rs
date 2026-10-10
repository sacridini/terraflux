//! The wizard of a new model: which model, which data, what is wrong with the data, which settings; it writes a
//! model whose every choice is a node with a note, ready to read and change.

use std::path::{Path, PathBuf};

use egui::{RichText, Ui};
use serde_json::json;
use terraflux_exec::lint::{Action, Finding, Level, lint};
use terraflux_graph::{Input, Model, relative_path};
use terraflux_ops::presets;
use terraflux_raster::Raster;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dinamica,
    Clue,
    Ordered,
}

impl Kind {
    pub fn title(self) -> &'static str {
        match self {
            Kind::Dinamica => "Dinamica EGO's classic model",
            Kind::Clue => "CLUE-S",
            Kind::Ordered => "Ordered allocation",
        }
    }

    pub fn about(self) -> &'static str {
        match self {
            Kind::Dinamica => {
                "Fitted on two maps: how much changes comes from the transition rates between them (a Markov chain), \
                 where from weights of evidence of the drivers, and the shape of the new patches from the patches \
                 observed (Expander and Patcher). A good start when you have two dates of land use."
            }
            Kind::Clue => {
                "lulcc's CLUE-S: the suitability of each class from logistic regressions on the drivers, the cells \
                 of each class at each step interpolated from observed maps, and an allocation where classes compete \
                 for cells until the demand is met."
            }
            Kind::Ordered => {
                "lulcc's ordered allocation: the same suitability and demand as CLUE-S, the classes served one after \
                 the other in a given order, each taking the cells it suits best."
            }
        }
    }
}

/// A raster picked: its path, band names and the classes of its first band (when whole values).
#[derive(Debug, Clone, PartialEq)]
pub struct Picked {
    pub path: PathBuf,
    pub bands: Vec<String>,
    pub classes: Vec<f64>,
}

impl Picked {
    pub fn read(path: &Path) -> Result<Picked, String> {
        let r = Raster::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut classes: Vec<f64> = Vec::new();
        for v in r.bands[0].masked(r.nodata) {
            if v.is_nan() || v.fract() != 0.0 {
                continue;
            }
            if !classes.contains(&v) {
                classes.push(v);
                if classes.len() > 100 {
                    break;
                }
            }
        }
        classes.sort_by(|a, b| a.partial_cmp(b).unwrap());
        Ok(Picked { path: path.to_path_buf(), bands: r.band_names(), classes })
    }

    /// The band names as numbers (years), when they all are.
    pub fn times(&self) -> Option<Vec<f64>> {
        self.bands.iter().map(|b| b.trim().parse::<f64>().ok()).collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Kind,
    Data,
    Check,
    Settings,
}

pub enum Outcome {
    Open,
    Cancel,
    Created(PathBuf),
}

/// The wizard's state.
pub struct Wizard {
    pub step: Step,
    pub kind: Kind,
    pub landuse: Option<Picked>,
    pub t0: usize,
    pub t1: usize,
    pub drivers: Option<Picked>,
    pub times: String,
    pub steps: usize,
    pub simulate: usize,
    pub seed: i64,
    pub increment: Option<f64>,
    pub expansion: Option<f64>,
    pub isometry: f64,
    pub elasticity: f64,
    pub order: String,
    pub findings: Vec<Finding>,
    pub error: Option<String>,
    /// What was done to the data (the inputs' notes), by input.
    pub notes: std::collections::BTreeMap<String, String>,
}

impl Default for Wizard {
    fn default() -> Self {
        Wizard {
            step: Step::Kind,
            kind: Kind::Dinamica,
            landuse: None,
            t0: 0,
            t1: 1,
            drivers: None,
            times: String::new(),
            steps: 1,
            simulate: 1,
            seed: 1,
            increment: None,
            expansion: None,
            isometry: 1.5,
            elasticity: 0.2,
            order: String::new(),
            findings: Vec::new(),
            error: None,
            notes: Default::default(),
        }
    }
}

fn pick(title: &str) -> Option<PathBuf> {
    rfd::FileDialog::new().set_title(title).add_filter("GeoTIFF", &["tif", "tiff"]).pick_file()
}

fn path_text(p: &Path, dir: Option<&Path>) -> String {
    match dir {
        Some(d) => relative_path(p, d),
        None => p.to_string_lossy().replace('\\', "/"),
    }
}

impl Wizard {
    /// Defaults from the land use map: the two dates and the steps between them, the observed dates.
    pub fn landuse_picked(&mut self, p: Picked) {
        self.t0 = 0;
        self.t1 = p.bands.len().saturating_sub(1).max(usize::from(p.bands.len() > 1));
        if let Some(t) = p.times() {
            self.times = t.iter().map(|v| format!("{v}")).collect::<Vec<_>>().join(", ");
            if t.len() >= 2 {
                let years = t[self.t1] - t[self.t0];
                if years >= 1.0 && years.fract() == 0.0 {
                    self.steps = years as usize;
                    self.simulate = self.steps;
                }
            }
        } else {
            self.times = (0..p.bands.len()).map(|i| i.to_string()).collect::<Vec<_>>().join(", ");
        }
        self.landuse = Some(p);
    }

    fn parse_times(&self) -> Result<Vec<f64>, String> {
        self.times
            .split(',')
            .map(|s| s.trim().parse::<f64>().map_err(|_| format!("{:?} is not a date", s.trim())))
            .collect()
    }

    /// The model of the wizard's choices; paths relative to `dir` (the model file's folder) when given.
    pub fn build(&self, dir: Option<&Path>) -> Result<Model, String> {
        let lu = self.landuse.as_ref().ok_or("choose the land use map")?;
        let dr = self.drivers.as_ref().ok_or("choose the drivers")?;
        let mut m = Model::new(self.kind.title());
        let lu_path = path_text(&lu.path, dir);
        let dr_path = path_text(&dr.path, dir);
        m.inputs.insert(
            "drivers".into(),
            Input::Stack {
                path: dr_path,
                bands: None,
                nodata: None,
                note: Some(match self.notes.get("drivers") {
                    Some(n) => format!("the explanatory variables, {n}"),
                    None => "the explanatory variables".into(),
                }),
            },
        );
        m.inputs.insert(
            "seed".into(),
            Input::Value { value: self.seed.into(), note: Some("the seed of the simulation".into()) },
        );
        match self.kind {
            Kind::Dinamica => {
                if lu.bands.len() < 2 || self.t0 == self.t1 {
                    return Err("Dinamica's model needs two dates of land use: a map with two bands at least".into());
                }
                let band = |i: usize| Input::Map {
                    path: lu_path.clone(),
                    band: Some(lu.bands[i].clone()),
                    nodata: None,
                    note: Some(format!("land use in {}", lu.bands[i])),
                };
                m.inputs.insert("t0".into(), band(self.t0));
                m.inputs.insert("t1".into(), band(self.t1));
                presets::dinamica(
                    &mut m,
                    &presets::Dinamica {
                        steps: self.steps,
                        simulate: self.simulate,
                        increment: self.increment,
                        expansion: self.expansion,
                        isometry: self.isometry,
                        ..Default::default()
                    },
                );
            }
            Kind::Clue | Kind::Ordered => {
                let times = self.parse_times()?;
                if times.len() != lu.bands.len() {
                    return Err(format!("{} dates for {} maps", times.len(), lu.bands.len()));
                }
                m.inputs.insert(
                    "observed".into(),
                    Input::Stack {
                        path: lu_path.clone(),
                        bands: None,
                        nodata: None,
                        note: Some("the observed maps, one per date".into()),
                    },
                );
                m.inputs.insert(
                    "start".into(),
                    Input::Map {
                        path: lu_path,
                        band: Some(lu.bands[0].clone()),
                        nodata: None,
                        note: Some("the map the simulation starts from".into()),
                    },
                );
                let allocation = if self.kind == Kind::Clue {
                    json!({"method": "clue"})
                } else {
                    let order: Vec<f64> = self
                        .order
                        .split(',')
                        .map(|s| s.trim().parse::<f64>().map_err(|_| format!("{:?} is not a class", s.trim())))
                        .collect::<Result<_, _>>()?;
                    json!({"method": "ordered", "order": order})
                };
                presets::lulcc(
                    &mut m,
                    &presets::Lulcc {
                        observed: "observed".into(),
                        drivers: "drivers".into(),
                        seed: "seed".into(),
                        start: "start".into(),
                        restrictions: None,
                        history: None,
                        times,
                        formulas: None,
                        allocation,
                        elasticity: (self.kind == Kind::Clue).then(|| json!(self.elasticity)),
                        allowed: None,
                        neighbourhood: None,
                    },
                );
            }
        }
        Ok(m)
    }

    /// Read the data of the model and check it.
    pub fn check(&mut self) {
        self.findings.clear();
        self.error = None;
        match self.build(None) {
            Err(e) => self.error = Some(e),
            Ok(m) => match terraflux_exec::load_inputs(&m, Path::new(""), &Default::default()) {
                Err(e) => self.error = Some(e.to_string()),
                Ok(inputs) => self.findings = lint(&m, &inputs),
            },
        }
    }

    /// Resample an input's raster to the land use map's grid: a new file next to it, the method chosen by the
    /// kind of data; the wizard then uses it and checks again.
    pub fn resample(&mut self, input: &str) -> Result<(), String> {
        use terraflux_raster::resample::{Method, resample};
        let lu = self.landuse.as_ref().ok_or("choose the land use map")?;
        let reference = Raster::read(&lu.path).map_err(|e| e.to_string())?.grid;
        let picked = match input {
            "drivers" => self.drivers.as_ref().ok_or("choose the drivers")?,
            _ => return Err(format!("${input} cannot be resampled here")),
        };
        let src = Raster::read(&picked.path).map_err(|e| e.to_string())?;
        let (out, methods) = resample(&src, &reference, Method::Auto).map_err(|e| e.to_string())?;
        let stem = picked.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let ref_stem = lu.path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let path = picked.path.with_file_name(format!("{stem}_on_{ref_stem}.tif"));
        out.write(&path).map_err(|e| e.to_string())?;
        let mut names: Vec<&str> = methods
            .iter()
            .map(|m| match m {
                Method::Nearest => "nearest",
                Method::Mode => "mode",
                Method::Bilinear => "bilinear",
                Method::Average => "average",
                Method::Auto => "auto",
            })
            .collect();
        names.dedup();
        self.notes.insert(
            input.to_string(),
            format!("resampled from {} to the grid of the land use map ({})", picked.path.display(), names.join(", ")),
        );
        let p = Picked::read(&path)?;
        self.drivers = Some(p);
        self.check();
        Ok(())
    }

    pub fn blocked(&self) -> bool {
        self.error.is_some() || self.findings.iter().any(|f| f.level == Level::Error)
    }

    /// Write the model next to where the user says.
    pub fn save(&self, path: &Path) -> Result<PathBuf, String> {
        let dir = path.parent().ok_or("no folder")?;
        let m = self.build(Some(dir))?;
        m.save(path).map_err(|e| e.to_string())?;
        Ok(path.to_path_buf())
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<Outcome> {
        let mut out = None;
        ui.heading("New model");
        ui.label(match self.step {
            Step::Kind => "1 of 4: the model",
            Step::Data => "2 of 4: the data",
            Step::Check => "3 of 4: what the data looks like",
            Step::Settings => "4 of 4: the settings",
        });
        ui.separator();
        match self.step {
            Step::Kind => {
                for k in [Kind::Dinamica, Kind::Clue, Kind::Ordered] {
                    ui.radio_value(&mut self.kind, k, RichText::new(k.title()).strong());
                    ui.indent(k.title(), |ui| ui.label(k.about()));
                    ui.add_space(6.0);
                }
            }
            Step::Data => {
                ui.horizontal(|ui| {
                    if ui.button("Land use map...").clicked()
                        && let Some(p) = pick("The land use map (one band per date)")
                    {
                        match Picked::read(&p) {
                            Ok(x) => self.landuse_picked(x),
                            Err(e) => self.error = Some(e),
                        }
                    }
                    ui.label(self.landuse.as_ref().map_or("none".into(), |p| p.path.display().to_string()));
                });
                if let Some(lu) = &self.landuse {
                    if self.kind == Kind::Dinamica {
                        let bands = lu.bands.clone();
                        egui::ComboBox::from_label("the start of the calibration period")
                            .selected_text(&bands[self.t0.min(bands.len() - 1)])
                            .show_ui(ui, |ui| {
                                for (i, b) in bands.iter().enumerate() {
                                    ui.selectable_value(&mut self.t0, i, b);
                                }
                            });
                        egui::ComboBox::from_label("its end")
                            .selected_text(&bands[self.t1.min(bands.len() - 1)])
                            .show_ui(ui, |ui| {
                                for (i, b) in bands.iter().enumerate() {
                                    ui.selectable_value(&mut self.t1, i, b);
                                }
                            });
                    } else {
                        ui.horizontal(|ui| {
                            ui.label("the date of each band:");
                            ui.text_edit_singleline(&mut self.times);
                        });
                    }
                }
                ui.horizontal(|ui| {
                    if ui.button("Drivers...").clicked()
                        && let Some(p) = pick("The drivers (one band per variable)")
                    {
                        match Picked::read(&p) {
                            Ok(x) => self.drivers = Some(x),
                            Err(e) => self.error = Some(e),
                        }
                    }
                    ui.label(
                        self.drivers
                            .as_ref()
                            .map_or("none".into(), |p| format!("{} ({})", p.path.display(), p.bands.join(", "))),
                    );
                });
            }
            Step::Check => {
                if let Some(e) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                } else if self.findings.is_empty() {
                    ui.label("Every map is on the same grid and nothing looks wrong.");
                }
                let mut act = None;
                for f in &self.findings {
                    let colour = match f.level {
                        Level::Error => ui.visuals().error_fg_color,
                        Level::Warning => ui.visuals().warn_fg_color,
                        Level::Info => ui.visuals().text_color(),
                    };
                    ui.colored_label(colour, format!("{}: {}", f.at, f.message));
                    if let Some(fix) = &f.fix {
                        ui.indent(&f.message, |ui| {
                            ui.label(format!("what to do: {fix}"));
                            if let Some(Action::Resample { input, .. }) = &f.action
                                && input == "drivers"
                                && ui.button("Resample it to the grid of the land use map").clicked()
                            {
                                act = Some(input.clone());
                            }
                        });
                    }
                }
                if let Some(input) = act
                    && let Err(e) = self.resample(&input)
                {
                    self.error = Some(e);
                }
                for (input, note) in &self.notes {
                    ui.label(format!("${input}: {note}"));
                }
            }
            Step::Settings => {
                egui::Grid::new("settings").num_columns(2).show(ui, |ui| {
                    if self.kind == Kind::Dinamica {
                        ui.label("time steps between the two maps");
                        ui.add(egui::DragValue::new(&mut self.steps).range(1..=1000));
                        ui.end_row();
                        ui.label("steps to simulate");
                        ui.add(egui::DragValue::new(&mut self.simulate).range(1..=10000));
                        ui.end_row();
                        let mut auto = self.increment.is_none();
                        ui.label("buffer increment of the weights");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut auto, "automatic (a hundredth of each variable's range)");
                            if auto {
                                self.increment = None;
                            } else {
                                let mut v = self.increment.unwrap_or(1.0);
                                ui.add(egui::DragValue::new(&mut v).range(1e-9..=f64::MAX));
                                self.increment = Some(v);
                            }
                        });
                        ui.end_row();
                        let mut observed = self.expansion.is_none();
                        ui.label("share of the changes that expand patches");
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut observed, "as observed");
                            if observed {
                                self.expansion = None;
                            } else {
                                let mut v = self.expansion.unwrap_or(0.5);
                                ui.add(egui::Slider::new(&mut v, 0.0..=1.0));
                                self.expansion = Some(v);
                            }
                        });
                        ui.end_row();
                        ui.label("isometry of the patches");
                        ui.add(egui::Slider::new(&mut self.isometry, 0.0..=3.0));
                        ui.end_row();
                    } else if self.kind == Kind::Clue {
                        ui.label("elasticity (resistance to change, 0-1)");
                        ui.add(egui::Slider::new(&mut self.elasticity, 0.0..=1.0));
                        ui.end_row();
                    } else {
                        ui.label("the order classes are served in");
                        ui.text_edit_singleline(&mut self.order);
                        ui.end_row();
                    }
                    ui.label("seed");
                    ui.add(egui::DragValue::new(&mut self.seed));
                    ui.end_row();
                });
                if let Some(e) = &self.error {
                    ui.colored_label(ui.visuals().error_fg_color, e);
                }
            }
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Cancel").clicked() {
                out = Some(Outcome::Cancel);
            }
            if self.step != Step::Kind && ui.button("Back").clicked() {
                self.step = match self.step {
                    Step::Data => Step::Kind,
                    Step::Check => Step::Data,
                    _ => Step::Check,
                };
            }
            match self.step {
                Step::Kind | Step::Data => {
                    let ready = self.step == Step::Kind || (self.landuse.is_some() && self.drivers.is_some());
                    if ui.add_enabled(ready, egui::Button::new("Next")).clicked() {
                        if self.step == Step::Kind {
                            self.step = Step::Data;
                        } else {
                            self.check();
                            self.step = Step::Check;
                        }
                    }
                }
                Step::Check => {
                    if ui.add_enabled(!self.blocked(), egui::Button::new("Next")).clicked() {
                        if self.order.is_empty()
                            && let Some(lu) = &self.landuse
                        {
                            self.order = lu.classes.iter().map(|c| format!("{c}")).collect::<Vec<_>>().join(", ");
                        }
                        self.step = Step::Settings;
                    }
                }
                Step::Settings => {
                    if ui.button("Create the model...").clicked()
                        && let Some(p) = rfd::FileDialog::new()
                            .set_title("Save the model")
                            .add_filter("Terraflux model", &["toml", "json"])
                            .set_file_name("model.toml")
                            .save_file()
                    {
                        match self.save(&p) {
                            Ok(p) => out = Some(Outcome::Created(p)),
                            Err(e) => self.error = Some(e),
                        }
                    }
                }
            }
        });
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
    }

    #[test]
    fn dinamica_from_two_dates() {
        let mut w = Wizard::default();
        w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
        assert_eq!((w.t0, w.t1, w.steps), (0, 1, 14), "14 years between 1997 and 2011");
        w.drivers = Some(Picked::read(&data("sibuyan_drivers.tif")).unwrap());
        w.check();
        assert!(!w.blocked(), "{:?} {:?}", w.error, w.findings);
        let dir = std::env::temp_dir().join("terraflux-wizard-test");
        std::fs::create_dir_all(&dir).unwrap();
        let p = w.save(&dir.join("m.toml")).unwrap();
        let m = Model::load(&p).unwrap();
        assert!(m.node("woe").is_some() && m.node("simulation").is_some());
        let Some(Input::Map { path, band, .. }) = m.inputs.get("t1") else { panic!() };
        assert!(path.ends_with("sibuyan_landuse.tif") && band.as_deref() == Some("2011"));
    }

    #[test]
    fn misaligned_drivers_are_resampled() {
        let dir = std::env::temp_dir().join("terraflux-wizard-resample");
        std::fs::create_dir_all(&dir).unwrap();
        // the drivers of Sibuyan on a grid twice as fine, shifted: not aligned with the land use map
        let mut r = Raster::read(data("sibuyan_drivers.tif")).unwrap();
        let g = r.grid.clone();
        let mut fine = g.clone();
        fine.width *= 2;
        fine.height *= 2;
        fine.transform[1] /= 2.0;
        fine.transform[5] /= 2.0;
        let (moved, _) =
            terraflux_raster::resample::resample(&r, &fine, terraflux_raster::resample::Method::Nearest).unwrap();
        r = moved;
        let p = dir.join("drivers_fine.tif");
        r.write(&p).unwrap();
        let mut w = Wizard::default();
        w.landuse_picked(Picked::read(&data("sibuyan_landuse.tif")).unwrap());
        w.drivers = Some(Picked::read(&p).unwrap());
        w.check();
        assert!(w.blocked());
        assert!(
            w.findings.iter().any(|f| matches!(&f.action, Some(Action::Resample { input, .. }) if input == "drivers"))
        );
        w.resample("drivers").unwrap();
        assert!(!w.blocked(), "{:?}", w.findings);
        assert!(w.drivers.as_ref().unwrap().path.ends_with("drivers_fine_on_sibuyan_landuse.tif"));
        let m = w.build(None).unwrap();
        let Some(Input::Stack { note: Some(n), .. }) = m.inputs.get("drivers") else { panic!() };
        assert!(n.contains("resampled") && n.contains("average"), "{n}");
    }

    #[test]
    fn ordered_needs_an_order() {
        let mut w = Wizard { kind: Kind::Ordered, ..Default::default() };
        w.landuse_picked(Picked::read(&data("pie_landuse.tif")).unwrap());
        assert_eq!(w.times, "1985, 1991, 1999");
        w.drivers = Some(Picked::read(&data("pie_drivers.tif")).unwrap());
        assert!(w.build(None).is_err());
        w.order = "2, 1, 3".into();
        let m = w.build(None).unwrap();
        assert!(m.node("allocate").is_none() && m.node("simulation").is_some());
    }
}
