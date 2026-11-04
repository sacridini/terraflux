//! The legend of a map of classes, one table: a row per class (its colour, its name, its area, a bar of its
//! share and the share), of the whole map, the part in view, the polygon measured or the
//! polygons selected, in hectares, km² or cells; the palette and the legend's editor in its header. It is both the
//! legend and the area of the classes (they were two lists, the same classes twice).

use super::*;
use egui::{FontId, Pos2, Sense};

/// A line of the legend: the classes of a run of the same name in one (the values of a density in its classes:
/// Rare (0-9%)...), in the colour of the first.
struct Line {
    colour: Color32,
    name: String,
    values: Vec<f64>,
}

fn lines_of(look: &Look, legend: Option<&[terraflux_ops::legend::Class]>) -> Vec<Line> {
    let Coloring::Classes(classes) = &look.coloring else { return Vec::new() };
    let mut lines: Vec<Line> = Vec::new();
    for (v, col) in classes {
        let name = crate::legend_view::name(look, legend, *v);
        match lines.last_mut() {
            Some(l) if l.name == name => l.values.push(*v),
            _ => lines.push(Line { colour: *col, name, values: vec![*v] }),
        }
    }
    lines
}

/// The amounts of a line: its cells, its area (m²) and its share (%).
#[derive(Clone, Copy, Default)]
struct Amount {
    cells: f64,
    area: Option<f64>,
    share: f64,
}

/// A number with its thousands apart (a thin space), as many decimals as asked.
fn grouped(v: f64, decimals: usize) -> String {
    let text = format!("{:.*}", decimals, v.abs());
    let (int, frac) = text.split_once('.').map_or((text.as_str(), None), |(a, b)| (a, Some(b)));
    let mut out = String::new();
    for (i, ch) in int.chars().enumerate() {
        if i > 0 && (int.len() - i) % 3 == 0 {
            out.push('\u{2009}');
        }
        out.push(ch);
    }
    if let Some(f) = frac {
        out.push('.');
        out.push_str(f);
    }
    if v < 0.0 { format!("-{out}") } else { out }
}

/// An amount in a unit, the number alone (the unit is in the column's head).
fn number_in(a: &Amount, unit: AreaUnit) -> String {
    match (unit, a.area) {
        (AreaUnit::Hectares, Some(m2)) => {
            let ha = m2 / 1e4;
            grouped(ha, if ha >= 100.0 { 0 } else { 2 })
        }
        (AreaUnit::SquareKm, Some(m2)) => {
            let km = m2 / 1e6;
            grouped(km, if km >= 100.0 { 1 } else { 3 })
        }
        _ => grouped(a.cells.round(), 0),
    }
}

fn unit_name(unit: AreaUnit) -> &'static str {
    match unit {
        AreaUnit::Hectares => "ha",
        AreaUnit::SquareKm => "km²",
        AreaUnit::Cells => "cells",
    }
}

/// The width of the bar of a class's share.
const BAR: f32 = 34.0;

/// A row of the table: the colour, the name (cut short when long), the area, a bar
/// of the share and the share at the right in columns of their own.
#[allow(clippy::too_many_arguments)]
fn row(
    ui: &mut Ui,
    line: &Line,
    amount: Option<Amount>,
    top_share: f64,
    unit: AreaUnit,
    (area_w, share_w): (f32, f32),
    striped: bool,
) -> egui::Response {
    let height = 20.0;
    let width = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let p = ui.painter_at(r);
    let visuals = ui.visuals();
    let text = visuals.text_color();
    let weak = visuals.weak_text_color();
    let colour = crate::palette::seen(line.colour);
    if striped {
        p.rect_filled(r, 0.0, visuals.faint_bg_color);
    }
    if resp.hovered() {
        p.rect_filled(r, 2.0, visuals.widgets.hovered.weak_bg_fill.gamma_multiply(0.5));
    }
    // the colour
    let sw = Rect::from_center_size(Pos2::new(r.left() + 9.0, r.center().y - 1.0), Vec2::splat(12.0));
    p.rect_filled(sw, 2.0, colour);
    p.rect_stroke(sw, 2.0, egui::Stroke::new(1.0, weak.gamma_multiply(0.5)), egui::StrokeKind::Inside);
    // the numbers at the right, in columns
    let font = FontId::proportional(12.5);
    let mono = FontId::monospace(11.5);
    let (share_text, area_text, faded) = match amount {
        Some(a) if a.cells > 0.0 => (format!("{:.1}%", a.share), number_in(&a, unit), false),
        Some(_) => ("0%".to_string(), "0".to_string(), true),
        None => (String::new(), String::new(), false),
    };
    let y = r.center().y - 1.0;
    p.text(Pos2::new(r.right() - 2.0, y), egui::Align2::RIGHT_CENTER, &share_text, mono.clone(), weak);
    // the share as a bar in a track of its own, against the largest class's (a small class still seen)
    let track = Rect::from_min_size(Pos2::new(r.right() - 2.0 - share_w - 4.0 - BAR, y - 3.0), Vec2::new(BAR, 6.0));
    p.rect_filled(track, 2.0, visuals.extreme_bg_color);
    if let Some(a) = amount
        && top_share > 0.0
        && a.share > 0.0
    {
        let frac = (a.share / top_share).clamp(0.0, 1.0) as f32;
        let fill = Rect::from_min_size(track.min, Vec2::new((BAR * frac).max(1.5), track.height()));
        p.rect_filled(fill, 2.0, text.gamma_multiply(0.55));
    }
    p.text(
        Pos2::new(r.right() - 2.0 - share_w - 4.0 - BAR - 8.0, y),
        egui::Align2::RIGHT_CENTER,
        &area_text,
        mono,
        if faded { weak } else { text },
    );
    // the name in what is left, cut short with an ellipsis
    let left = r.left() + 20.0;
    let room = (r.right() - 2.0 - share_w - 4.0 - BAR - 8.0 - area_w - 8.0 - left).max(20.0);
    let mut job = egui::text::LayoutJob::single_section(
        line.name.clone(),
        egui::TextFormat { font_id: font, color: if faded { weak } else { text }, ..Default::default() },
    );
    job.wrap =
        egui::text::TextWrapping {
            max_width: room, max_rows: 1, break_anywhere: true, overflow_character: Some('…')
        };
    let galley = ui.fonts_mut(|f| f.layout_job(job));
    p.galley(Pos2::new(left, y - galley.size().y / 2.0), galley, text);
    let label = format!("{} {} {} {}", line.name, area_text, unit_name(unit), share_text);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &label));
    resp
}

/// The legend of a map of classes with the area of each, as one table (see the module).
#[allow(clippy::too_many_arguments)]
pub(super) fn section(
    ui: &mut Ui,
    s: &mut Session,
    r: &Registry,
    sh: &mut Shown,
    src: &Source,
    look: &Look,
    legend: Option<&[terraflux_ops::legend::Class]>,
    (node, port): (&str, &str),
    salt: &str,
) {
    let key = (node.to_string(), port.to_string());
    // the head: the title, the palette and the editor
    ui.horizontal(|ui| {
        theme::section(ui, "Legend");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            theme::quiet(ui);
            if !crate::legend_view::editing(ui, node, port)
                && ui
                    .small_button("Edit legend")
                    .on_hover_text("the names and colours of the classes, kept in the model (a raster.legend node)")
                    .clicked()
            {
                crate::legend_view::begin(ui, node, port, look, legend);
            }
            let colours = ui.small_button("Colours ⏷").on_hover_text("another palette");
            egui::Popup::menu(&colours)
                .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                .show(|ui| layers_panel::palette_menu(ui, s, &key, &look.coloring));
        });
    });
    if crate::legend_view::editing(ui, node, port) {
        crate::legend_view::editor(ui, s, r, node, port, look, legend);
        return;
    }
    // the part of the map, and the unit
    let id = egui::Id::new(("region scope", salt));
    let mut scope: Scope = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let drawn = sh.view.measuring.as_ref().filter(|m| m.points.len() >= 3).map(|m| vec![m.points.clone()]);
    let selected = selected_rings(s, sh, ui.ctx(), src.grid());
    if scope == Scope::Drawn && drawn.is_none() || scope == Scope::Selected && selected.is_none() {
        scope = Scope::Map;
    }
    let uid = egui::Id::new(("region unit", salt));
    let mut unit: AreaUnit = ui.data(|d| d.get_temp(uid)).unwrap_or_default();
    let with_area = crate::measure::cell_area(src.grid(), 0).is_some();
    if !with_area {
        unit = AreaUnit::Cells;
    }
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        ui.selectable_value(&mut scope, Scope::Map, "map").on_hover_text("the whole map");
        ui.selectable_value(&mut scope, Scope::View, "view").on_hover_text("the part of the map in view");
        ui.add_enabled_ui(drawn.is_some(), |ui| {
            ui.selectable_value(&mut scope, Scope::Drawn, "polygon")
                .on_hover_text("the polygon of the line measured (Measure, three points or more)")
                .on_disabled_hover_text("draw a polygon with Measure (M): three points or more");
        });
        ui.add_enabled_ui(selected.is_some(), |ui| {
            ui.selectable_value(&mut scope, Scope::Selected, "selected")
                .on_hover_text("the polygons selected in the table of attributes")
                .on_disabled_hover_text("select polygons in the table of attributes of a vector layer");
        });
        if with_area {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.selectable_value(&mut unit, AreaUnit::Cells, "cells");
                ui.selectable_value(&mut unit, AreaUnit::SquareKm, "km²");
                ui.selectable_value(&mut unit, AreaUnit::Hectares, "ha");
            });
        }
    });
    ui.data_mut(|d| {
        d.insert_temp(id, scope);
        d.insert_temp(uid, unit);
    });
    let part = match scope {
        Scope::Map => Some(crate::region::Part::Map),
        Scope::View => settled_view(ui, &sh.view, salt).map(crate::region::Part::Window),
        Scope::Drawn => drawn.map(crate::region::Part::Polygons),
        Scope::Selected => selected.map(crate::region::Part::Polygons),
    };
    let stats = part.as_ref().and_then(|p| s.regions.get(ui.ctx(), src, p, true));
    let lines = lines_of(look, legend);
    // each line's amount: of the part counted, else (while it is counted) the map's cells as the look has them
    let amounts: Vec<Option<Amount>> = match &stats {
        Some(st) => lines
            .iter()
            .map(|l| {
                let mut a = Amount::default();
                for c in st.classes.iter().filter(|c| l.values.contains(&c.value)) {
                    a.cells += c.cells;
                    a.area = match (a.area, c.area) {
                        (x, Some(b)) => Some(x.unwrap_or(0.0) + b),
                        (x, None) => x,
                    };
                    a.share += st.share(c);
                }
                Some(a)
            })
            .collect(),
        None if scope == Scope::Map => {
            let all: usize = look.counts.iter().map(|(_, n)| n).sum();
            lines
                .iter()
                .map(|l| {
                    let cells: usize = look.counts.iter().filter(|(v, _)| l.values.contains(v)).map(|(_, n)| n).sum();
                    let share = if all > 0 { 100.0 * cells as f64 / all as f64 } else { 0.0 };
                    Some(Amount { cells: cells as f64, area: None, share })
                })
                .collect()
        }
        None => lines.iter().map(|_| None).collect(),
    };
    // (cells until the areas come)
    let shown_unit = if amounts.iter().flatten().any(|a| a.area.is_some()) { unit } else { AreaUnit::Cells };
    let top_share = amounts.iter().flatten().map(|a| a.share).fold(0.0, f64::max);
    // the columns as wide as their widest number
    let mono = FontId::monospace(11.5);
    let width_of = |t: &str| ui.fonts_mut(|f| f.layout_no_wrap(t.to_string(), mono.clone(), Color32::WHITE).size().x);
    let area_w = amounts
        .iter()
        .flatten()
        .map(|a| width_of(&number_in(a, shown_unit)))
        .fold(width_of(unit_name(shown_unit)), f32::max);
    let share_w = width_of("100.0%");
    // the columns' heads
    {
        let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 16.0), Sense::hover());
        let p = ui.painter_at(hr);
        let weak = ui.visuals().weak_text_color();
        let f = FontId::proportional(10.5);
        let y = hr.center().y;
        p.text(Pos2::new(hr.left() + 20.0, y), egui::Align2::LEFT_CENTER, "class", f.clone(), weak);
        p.text(
            Pos2::new(hr.right() - 2.0 - share_w - 4.0 - BAR - 8.0, y),
            egui::Align2::RIGHT_CENTER,
            unit_name(shown_unit),
            f.clone(),
            weak,
        );
        p.text(Pos2::new(hr.right() - 2.0, y), egui::Align2::RIGHT_CENTER, "share", f, weak);
        let line = ui.visuals().widgets.noninteractive.bg_stroke;
        p.hline(hr.x_range(), hr.bottom() - 0.5, line);
    }
    ui.spacing_mut().item_spacing.y = 0.0;
    let mut text = String::from("class\tcells\tm2\tha\tkm2\tpercent\n");
    for (k, (l, a)) in lines.iter().zip(&amounts).enumerate() {
        let resp = row(ui, l, *a, top_share, shown_unit, (area_w, share_w), k % 2 == 1);
        if let Some(a) = a {
            let (m2, ha, km2) = match a.area {
                Some(m) => (format!("{m:.1}"), format!("{:.4}", m / 1e4), format!("{:.6}", m / 1e6)),
                None => Default::default(),
            };
            text.push_str(&format!("{}\t{}\t{m2}\t{ha}\t{km2}\t{:.3}\n", l.name, a.cells.round(), a.share));
            resp.on_hover_ui(|ui| {
                ui.label(RichText::new(&l.name).strong());
                let codes: Vec<String> = l.values.iter().map(|v| terraflux_ops::value::class_label(*v)).collect();
                ui.label(RichText::new(format!("value {}", codes.join(", "))).weak());
                ui.label(format!("{} cells", grouped(a.cells.round(), 0)));
                if let Some(m) = a.area {
                    ui.label(format!("{} ha · {} km²", grouped(m / 1e4, 2), grouped(m / 1e6, 3)));
                }
                ui.label(format!("{:.2}% of the cells with a value", a.share));
            });
        }
    }
    ui.spacing_mut().item_spacing.y = 4.0;
    {
        let (hr, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), Sense::hover());
        ui.painter().hline(hr.x_range(), hr.center().y, ui.visuals().widgets.noninteractive.bg_stroke);
    }
    // the total, and what the numbers are of
    match (&part, &stats) {
        (None, _) => {
            let why = match scope {
                Scope::View => "the map not drawn yet",
                Scope::Drawn => "no polygon drawn: Measure (M), three points or more",
                _ => "no polygon selected in the table of attributes",
            };
            ui.label(RichText::new(why).weak().size(11.0));
        }
        (Some(_), None) => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(RichText::new("counting the cells...").weak().size(11.0));
            });
        }
        (Some(_), Some(st)) => {
            ui.horizontal(|ui| {
                let total = Amount { cells: st.cells, area: st.area, share: 100.0 };
                ui.label(
                    RichText::new(format!(
                        "total: {} {} with a value",
                        number_in(&total, shown_unit),
                        unit_name(shown_unit)
                    ))
                    .size(11.0),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    theme::quiet(ui);
                    if st.cells > 0.0 && ui.small_button("Copy").on_hover_text("the table, tab-separated").clicked() {
                        ui.ctx().copy_text(text.clone());
                    }
                });
            });
            let mut notes = Vec::new();
            if st.empty > 0.0 {
                notes.push(format!("{} cells without a value", grouped(st.empty.round(), 0)));
            }
            if st.step > 1 {
                notes.push(format!("from a sample: a cell in every {} rows and columns", st.step));
            }
            if !notes.is_empty() {
                ui.label(RichText::new(notes.join(" · ")).weak().size(10.5));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::grouped;

    #[test]
    fn numbers_grouped_by_thousands() {
        assert_eq!(grouped(33894.0, 0), "33\u{2009}894");
        assert_eq!(grouped(1234567.891, 2), "1\u{2009}234\u{2009}567.89");
        assert_eq!(grouped(98.0, 0), "98");
        assert_eq!(grouped(-4500.5, 1), "-4\u{2009}500.5");
    }
}
