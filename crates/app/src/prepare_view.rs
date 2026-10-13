//! The preparation pane: the reference grid of the model, what the data of the node selected is next to it, and
//! the steps that put it on it or make drivers of it, a click each (see [`crate::prepare`]).

use egui::{RichText, Ui};
use terraflux_ops::Registry;
use terraflux_raster::Grid;

use crate::app::{Session, accent_button};
use crate::prepare::{Data, Kind, Step, compare, crs_name, crs_of};
use crate::theme;

/// A grid in a line: its coordinate system, cells and size.
fn grid_line(g: &Grid) -> String {
    let t = g.transform;
    format!(
        "{} · cells of {} x {} · {} x {} cells",
        crs_name(crs_of(g).as_deref()),
        short(t[1].abs()),
        short(t[5].abs()),
        g.width,
        g.height
    )
}

fn short(v: f64) -> String {
    if v >= 1.0 { format!("{}", (v * 1000.0).round() / 1000.0) } else { format!("{v:.3e}") }
}

/// The data of an output, when it has run.
fn data_of(s: &Session, node: &str, port: &str) -> Option<Data> {
    s.value_of(node, port).as_ref().and_then(Data::of)
}

pub fn pane(ui: &mut Ui, s: &mut Session, r: &Registry) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        egui::Frame::new().inner_margin(egui::Margin::same(8)).show(ui, |ui| {
            if let Err(e) = panel(ui, s, r) {
                s.editor.message = Some(e);
            }
        });
    });
}

fn panel(ui: &mut Ui, s: &mut Session, r: &Registry) -> Result<(), String> {
    let top = s.editor.path.is_empty();
    // the node selected and its first output
    let selected: Option<(String, String)> = s
        .selected
        .clone()
        .filter(|_| top)
        .and_then(|id| s.source.nodes.iter().find(|n| n.id == id).cloned())
        .and_then(|n| crate::builder::first_output(&n, r).map(|p| (n.id.clone(), p)));
    let data = selected.as_ref().and_then(|(n, p)| data_of(s, n, p));
    // the reference grid
    theme::section(ui, "Reference grid");
    let reference = s.reference.clone();
    let ref_grid: Option<Grid> = reference.as_ref().and_then(|(n, p)| data_of(s, n, p)).and_then(|d| d.grid().cloned());
    match (&reference, &ref_grid) {
        (Some((n, p)), Some(g)) => {
            ui.label(RichText::new(format!("{n}.{p}")).monospace().strong());
            ui.label(RichText::new(grid_line(g)).size(11.5));
        }
        (Some((n, p)), None) => {
            ui.label(RichText::new(format!("{n}.{p}")).monospace().strong());
            ui.label(RichText::new("not run yet: run it to see its grid").weak());
            if s.can_run_to() && ui.button("Run it").clicked() {
                crate::builder::run_to(ui, s, Some(n.clone()));
            }
        }
        (None, _) => {
            ui.label(
                "None yet: the grid every map of the model is put on (its coordinate system, cells and extent), \
                 usually the land use map's.",
            );
            // a raster of classes read, suggested
            let suggested = s.source.nodes.iter().filter(|n| n.op == "raster.read").find_map(|n| {
                let port = crate::builder::first_output(n, r)?;
                matches!(data_of(s, &n.id, &port)?.kind(), Kind::Classes).then(|| (n.id.clone(), port))
            });
            if let Some((n, p)) = suggested
                && ui
                    .add(accent_button(&format!("Use {n} (classes)"), true))
                    .on_hover_text("a map of classes read: likely the land use, whose grid the others go on")
                    .clicked()
            {
                s.reference = Some((n, p));
            }
        }
    }
    let raster_selected = matches!(data, Some(Data::Raster { .. }));
    let is_reference = selected.is_some() && selected == reference;
    if ui
        .add_enabled(raster_selected && !is_reference, egui::Button::new("Use the node selected"))
        .on_hover_text("the grid of the node selected becomes the reference grid")
        .on_disabled_hover_text("select a node that gives rasters (run it first)")
        .clicked()
    {
        s.reference = selected.clone();
    }
    ui.add_space(8.0);
    // the node selected
    let Some((id, port)) = selected else {
        theme::section(ui, "Data");
        ui.label(
            "Select a node that reads or makes data (a raster, vectors) to put it on the grid or make drivers of it.",
        );
        return Ok(());
    };
    theme::section(ui, &format!("{id}.{port}"));
    let Some(data) = data else {
        ui.label(RichText::new("not run yet").weak());
        if s.can_run_to() && ui.add(accent_button("Run to here", true)).clicked() {
            crate::builder::run_to(ui, s, Some(id.clone()));
        }
        return Ok(());
    };
    let reference_ref = reference.as_ref().map(|(n, p)| (n.as_str(), p.as_str()));
    let mut chosen: Option<(Step, Vec<&'static str>)> = None;
    match &data {
        Data::Raster { grid, layers, classes, .. } => {
            ui.label(RichText::new(grid_line(grid)).size(11.5));
            if *layers > 1 {
                ui.label(RichText::new(format!("{layers} layers")).size(11.5));
            }
            let diff = ref_grid.as_ref().map(|g| compare(grid, g));
            match (&diff, is_reference) {
                (_, true) => {
                    ui.label(RichText::new("the reference grid").color(theme::accent()));
                }
                (Some(d), _) if d.any() => {
                    ui.colored_label(
                        ui.visuals().warn_fg_color,
                        format!("next to the grid: {}", d.describe().join(", ")),
                    );
                }
                (Some(_), _) => {
                    ui.label(RichText::new("on the reference grid").color(theme::accent()));
                }
                (None, _) => {}
            }
            // what its values are
            let guessed = data.kind();
            let mut kind = s.kinds.get(&id).copied().unwrap_or(guessed);
            ui.horizontal_wrapped(|ui| {
                ui.label("Its values are");
                for k in Kind::ALL {
                    ui.radio_value(&mut kind, k, k.label()).on_hover_text(k.why(false));
                }
            });
            if kind != guessed || s.kinds.contains_key(&id) {
                s.kinds.insert(id.clone(), kind);
            }
            let differences = diff.as_ref().map(|d| d.describe()).unwrap_or_default();
            let steps = crate::prepare::suggestions(&data, kind, ref_grid.as_ref());
            ui.add_space(4.0);
            for (k, step) in steps.iter().enumerate() {
                if matches!(step, Step::DistanceToClass { .. }) {
                    continue;
                }
                let first = k == 0 && matches!(step, Step::Match { .. });
                let title = step.title();
                let b = if first { accent_button(&title, true) } else { egui::Button::new(title.as_str()) };
                if ui.add(b).on_hover_text(step.why(&differences)).clicked() {
                    chosen = Some((step.clone(), differences.clone()));
                }
            }
            // a distance to classes of a map of classes
            if kind == Kind::Classes {
                let init = classes.first().map(|c| c.to_string()).unwrap_or_default();
                let mut text = s.fields.entry("prepare.classes".into()).or_insert(init).clone();
                ui.horizontal(|ui| {
                    let b = ui
                        .button("Distance to class")
                        .on_hover_text("the distance from every cell to the nearest cell of these classes: a driver");
                    ui.add(egui::TextEdit::singleline(&mut text).desired_width(70.0).hint_text("1, 3"));
                    let parsed: Vec<f64> = text.split([',', ' ']).filter_map(|c| c.trim().parse().ok()).collect();
                    if b.clicked() {
                        if parsed.is_empty() {
                            s.editor.message = Some("give the classes, e.g. 1 or 1, 3".into());
                        } else {
                            chosen = Some((Step::DistanceToClass { classes: parsed }, differences.clone()));
                        }
                    }
                });
                if !classes.is_empty() {
                    ui.label(
                        RichText::new(format!(
                            "classes: {}",
                            classes.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(", ")
                        ))
                        .size(11.0)
                        .weak(),
                    );
                }
                s.fields.insert("prepare.classes".into(), text);
            }
            // to another coordinate system
            let mut crs = s.fields.entry("prepare.crs".into()).or_insert_with(|| "EPSG:4326".into()).clone();
            ui.horizontal(|ui| {
                let b = ui.button("Reproject to").on_hover_text(
                    "another coordinate system (EPSG code, WKT or PROJ string), its cells resampled by the data",
                );
                ui.add(egui::TextEdit::singleline(&mut crs).desired_width(110.0));
                if b.clicked() && !crs.trim().is_empty() {
                    chosen = Some((Step::Reproject { crs: crs.trim().to_string(), kind }, differences.clone()));
                }
            });
            s.fields.insert("prepare.crs".into(), crs);
            // a series of maps of classes: the analysis of its changes
            if *layers > 1 && kind == Kind::Classes {
                ui.add_space(8.0);
                theme::section(ui, "Changes along the dates");
                let value = s.value_of(&id, &port);
                let key = egui::Id::new(("suggested changes", &id, &port, s.revision));
                let suggested: Option<Result<terraflux_ops::Table, String>> = match ui.data(|d| d.get_temp(key)) {
                    Some(t) => t,
                    None => {
                        let t = value.as_ref().and_then(crate::prepare::suggested_changes);
                        ui.data_mut(|d| d.insert_temp(key, t.clone()));
                        t
                    }
                };
                let label = |c: f64| terraflux_ops::value::class_label(c);
                let (mut gain0, mut loss0) = (f64::NAN, f64::NAN);
                match &suggested {
                    Some(Ok(t)) => {
                        if let Some(top) = t.rows.first() {
                            (loss0, gain0) = (top[0], top[1]);
                        }
                        ui.label(RichText::new("the transitions worth modelling:").size(11.5));
                        for r in t.rows.iter().filter(|r| r[6] == 1.0).take(8) {
                            ui.label(
                                RichText::new(format!(
                                    "{} -> {}: {:.1} % of the change ({:.1} % of {})",
                                    label(r[0]),
                                    label(r[1]),
                                    r[4],
                                    r[5],
                                    label(r[0])
                                ))
                                .size(11.5),
                            );
                        }
                        let small = t.rows.iter().filter(|r| r[6] == 0.0).count();
                        if small > 0 {
                            let others = if small == 1 { "1 other".to_string() } else { format!("{small} others") };
                            ui.label(
                                RichText::new(format!("{others} under 1 % of the change: too small to model"))
                                    .weak()
                                    .size(11.0),
                            );
                        }
                    }
                    Some(Err(e)) => {
                        ui.colored_label(ui.visuals().warn_fg_color, e);
                    }
                    None => {
                        ui.label(RichText::new("large: the nodes work it out when run").weak().size(11.0));
                    }
                }
                let init = |v: f64| if v.is_nan() { String::new() } else { crate::prepare::number_text(v) };
                let mut gain = s.fields.entry("prepare.gain".into()).or_insert_with(|| init(gain0)).clone();
                let mut loss = s.fields.entry("prepare.loss".into()).or_insert_with(|| init(loss0)).clone();
                ui.horizontal(|ui| {
                    ui.label("gains of");
                    ui.add(egui::TextEdit::singleline(&mut gain).desired_width(40.0));
                    ui.label("losses of");
                    ui.add(egui::TextEdit::singleline(&mut loss).desired_width(40.0));
                });
                let (g, l) = (gain.trim().parse::<f64>(), loss.trim().parse::<f64>());
                s.fields.insert("prepare.gain".into(), gain);
                s.fields.insert("prepare.loss".into(), loss);
                let ok = g.is_ok() && l.is_ok();
                if ui
                    .add_enabled(ok, accent_button("Analyse the changes", ok))
                    .on_hover_text(
                        "the transitions, those worth modelling, the intensity analysis (the gains and losses of the \
                         classes above), the components of the difference, the changes of each cell: a node each",
                    )
                    .clicked()
                {
                    let pos = crate::builder::beside(&s.source.nodes, Some(&id));
                    let ids =
                        crate::prepare::analyse_changes(&mut s.source, r, (&id, &port), g.unwrap(), l.unwrap(), pos)?;
                    s.edited();
                    crate::builder::select(s, r, &ids[0]);
                    return Ok(());
                }
            }
        }
        Data::Vector { crs, features, polygons, lines, points, numbers } => {
            let kinds: Vec<&str> = [(*polygons, "polygons"), (*lines, "lines"), (*points, "points")]
                .into_iter()
                .filter_map(|(on, k)| on.then_some(k))
                .collect();
            ui.label(
                RichText::new(format!("{features} features ({}) · {}", kinds.join(", "), crs_name(crs.as_deref())))
                    .size(11.5),
            );
            if let (Some(c), Some(g)) = (crs, &ref_grid)
                && crs_of(g).is_some_and(|r| &r != c && !terraflux_gdal::same_crs(c, &r).unwrap_or(false))
            {
                ui.label(
                    RichText::new("another coordinate system: reprojected to the grid by each step").size(11.0).weak(),
                );
            }
            if ref_grid.is_none() {
                ui.label(RichText::new("choose the reference grid to make maps of them").weak());
            }
            let mut field = s.fields.get("prepare.field").cloned().unwrap_or_default();
            for step in crate::prepare::suggestions(&data, Kind::Values, ref_grid.as_ref()) {
                let step = match step {
                    Step::Rasterize { .. } => Step::Rasterize { field: (!field.is_empty()).then(|| field.clone()) },
                    s => s,
                };
                let first = matches!(step, Step::DistanceToFeatures);
                let title = step.title();
                let b = if first { accent_button(&title, true) } else { egui::Button::new(title) };
                if ui.add(b).on_hover_text(step.why(&[])).clicked() {
                    chosen = Some((step.clone(), Vec::new()));
                }
                if matches!(step, Step::Rasterize { .. }) && !numbers.is_empty() {
                    egui::ComboBox::from_id_salt("prepare.field")
                        .selected_text(if field.is_empty() { "1 where they are" } else { field.as_str() })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut field, String::new(), "1 where they are");
                            for n in numbers {
                                ui.selectable_value(&mut field, n.clone(), n);
                            }
                        });
                }
            }
            s.fields.insert("prepare.field".into(), field);
        }
    }
    // every raster read onto the grid
    if let (Some((rn, _)), Some(g)) = (&reference, &ref_grid) {
        let off: Vec<(String, String, Kind, bool)> = crate::prepare::read_rasters(&s.source, rn)
            .into_iter()
            .filter(|n| n.id != id && !matched(s, &n.id))
            .filter_map(|n| {
                let port = crate::builder::first_output(n, r)?;
                let d = data_of(s, &n.id, &port)?;
                let diff = compare(d.grid()?, g);
                let kind = s.kinds.get(&n.id).copied().unwrap_or(d.kind());
                diff.any().then(|| (n.id.clone(), port, kind, diff.coarser))
            })
            .collect();
        if !off.is_empty() {
            ui.add_space(8.0);
            theme::section(ui, "Every raster");
            if ui
                .button(match off.len() {
                    1 => format!("Put {} on the grid as well", off[0].0),
                    n => format!("Put the {n} other rasters off the grid on it"),
                })
                .on_hover_text(
                    off.iter().map(|(n, _, k, _)| format!("{n} ({})", k.label())).collect::<Vec<_>>().join("\n"),
                )
                .clicked()
            {
                let mut last = None;
                for (n, p, kind, coarser) in &off {
                    let pos = crate::builder::beside(&s.source.nodes, Some(n));
                    let step = Step::Match { kind: *kind, coarser: *coarser };
                    last = Some(crate::prepare::apply(&mut s.source, r, &step, (n, p), reference_ref, &[], pos)?);
                }
                s.edited();
                if let Some(l) = last {
                    crate::builder::select(s, r, &l);
                }
            }
        }
    }
    if let Some((step, differences)) = chosen {
        let pos = crate::builder::beside(&s.source.nodes, Some(&id));
        let new = crate::prepare::apply(&mut s.source, r, &step, (&id, &port), reference_ref, &differences, pos)?;
        s.edited();
        crate::builder::select(s, r, &new);
    }
    Ok(())
}

/// Whether a node's rasters are already put on a grid by a raster.match.
fn matched(s: &Session, id: &str) -> bool {
    s.source.nodes.iter().any(|n| {
        n.op == "raster.match"
            && matches!(n.inputs.get("raster"), Some(terraflux_graph::Source::Ref(r)) if r.split('.').next() == Some(id))
    })
}
