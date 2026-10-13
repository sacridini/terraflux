//! Tables as the viewer shows them: their rows filtered (by text, or a condition on a column such as
//! `area > 1`) and sorted by a column, as many as they are (only those in view are laid out), and drawn as charts
//! of several columns at once ([`crate::charts`]).

use egui::{RichText, Ui, Vec2};
use terraflux_ops::Table;

use crate::charts::Chart;

/// How a table is shown: its sort (a column, ascending), its filter, and its chart.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TableState {
    pub sort: Option<(usize, bool)>,
    pub filter: String,
    pub chart: Chart,
}

/// A number as a table shows it: up to 6 decimals without trailing zeros, NA for none.
pub fn cell(x: f64) -> String {
    if x.is_nan() {
        return "NA".into();
    }
    let s = format!("{x:.6}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// A condition on a column: its index, the comparison and the number.
fn condition(t: &Table, filter: &str) -> Option<Result<(usize, &'static str, f64), String>> {
    // the longest operators first
    let op = ["<=", ">=", "!=", "==", "<", ">", "="].into_iter().find(|op| filter.contains(op))?;
    let (name, value) = filter.split_once(op)?;
    let name = name.trim();
    let Some(j) = t.columns.iter().position(|c| c.eq_ignore_ascii_case(name)) else {
        return Some(Err(format!("no column {name:?}")));
    };
    let value = value.trim();
    let v = if value.eq_ignore_ascii_case("na") {
        f64::NAN
    } else {
        match value.parse::<f64>() {
            Ok(v) => v,
            Err(_) => return Some(Err(format!("{value:?} is no number"))),
        }
    };
    Some(Ok((j, op, v)))
}

/// The rows a state shows, in its order: those the filter keeps (every word of a text in some cell, or a
/// condition on a column), sorted by its column (NA last).
pub fn rows_shown(t: &Table, st: &TableState) -> Result<Vec<usize>, String> {
    let filter = st.filter.trim();
    let mut rows: Vec<usize> = match condition(t, filter) {
        None if filter.is_empty() => (0..t.rows.len()).collect(),
        None => {
            let words: Vec<String> = filter.to_lowercase().split_whitespace().map(String::from).collect();
            (0..t.rows.len())
                .filter(|&i| {
                    let cells: Vec<String> = t.rows[i].iter().map(|&x| cell(x).to_lowercase()).collect();
                    words.iter().all(|w| cells.iter().any(|c| c == w || c.starts_with(w.as_str())))
                })
                .collect()
        }
        Some(Err(e)) => return Err(e),
        Some(Ok((j, op, v))) => (0..t.rows.len())
            .filter(|&i| {
                let x = t.rows[i][j];
                if v.is_nan() {
                    return match op {
                        "!=" => !x.is_nan(),
                        "=" | "==" => x.is_nan(),
                        _ => false,
                    };
                }
                match op {
                    "<=" => x <= v,
                    ">=" => x >= v,
                    "<" => x < v,
                    ">" => x > v,
                    "!=" => x != v,
                    _ => x == v,
                }
            })
            .collect(),
    };
    if let Some((j, ascending)) = st.sort
        && j < t.columns.len()
    {
        rows.sort_by(|&a, &b| {
            let (x, y) = (t.rows[a][j], t.rows[b][j]);
            match (x.is_nan(), y.is_nan()) {
                (true, true) => std::cmp::Ordering::Equal,
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ if ascending => x.total_cmp(&y),
                _ => y.total_cmp(&x),
            }
        });
    }
    Ok(rows)
}

/// A table: the filter and the column drawn above it, then its rows (headers sort them).
pub fn show(ui: &mut Ui, t: &Table, id: egui::Id) {
    let mut st: TableState = ui.data(|d| d.get_temp(id)).unwrap_or_default();
    let shown = rows_shown(t, &st);
    ui.horizontal(|ui| {
        ui.add(
            egui::TextEdit::singleline(&mut st.filter)
                .hint_text("filter: words, or a condition (area > 1)")
                .desired_width(220.0),
        );
        match &shown {
            Ok(r) if r.len() == t.rows.len() => ui.label(RichText::new(format!("{} rows", t.rows.len())).weak()),
            Ok(r) => ui.label(RichText::new(format!("{} of {} rows", r.len(), t.rows.len())).weak()),
            Err(e) => ui.label(RichText::new(e).color(ui.visuals().error_fg_color)),
        };
    });
    let rows = shown.unwrap_or_default();
    crate::charts::show(ui, t, &rows, &mut st.chart, id.with("chart"));
    // the columns as wide as their header and the values of the first rows
    let mono = egui::TextStyle::Monospace.resolve(ui.style());
    let char_w = ui.fonts_mut(|f| f.glyph_width(&mono, '0'));
    let widths: Vec<f32> = (0..t.columns.len())
        .map(|j| {
            let longest =
                t.rows.iter().take(500).map(|r| cell(r[j]).len()).max().unwrap_or(0).max(t.columns[j].len() + 2);
            (longest as f32 * char_w + 14.0).clamp(48.0, 220.0)
        })
        .collect();
    let row_h = ui.text_style_height(&egui::TextStyle::Monospace) + 4.0;
    egui::ScrollArea::horizontal().id_salt(id.with("h")).auto_shrink([false, true]).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (j, c) in t.columns.iter().enumerate() {
                let arrow = match st.sort {
                    Some((k, true)) if k == j => " ^",
                    Some((k, false)) if k == j => " v",
                    _ => "",
                };
                // drawn as the cells are, from the left
                let (rect, b) = ui.allocate_exact_size(Vec2::new(widths[j], row_h + 2.0), egui::Sense::click());
                let colour = if b.hovered() { crate::theme::accent() } else { ui.visuals().strong_text_color() };
                ui.painter().text(
                    egui::pos2(rect.left() + 4.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    format!("{c}{arrow}"),
                    egui::TextStyle::Body.resolve(ui.style()),
                    colour,
                );
                if b.on_hover_text("sort by it (again: the other way, then not)").clicked() {
                    st.sort = match st.sort {
                        Some((k, true)) if k == j => Some((j, false)),
                        Some((k, false)) if k == j => None,
                        _ => Some((j, true)),
                    };
                }
            }
        });
        ui.separator();
        egui::ScrollArea::vertical().id_salt(id.with("v")).auto_shrink([false, false]).show_rows(
            ui,
            row_h,
            rows.len(),
            |ui, range| {
                for k in range {
                    let r = &t.rows[rows[k]];
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(widths.iter().sum(), row_h), egui::Sense::hover());
                    if k % 2 == 1 {
                        ui.painter().rect_filled(rect, 0.0, ui.visuals().faint_bg_color);
                    }
                    let mut x = rect.left() + 4.0;
                    for (j, &w) in widths.iter().enumerate() {
                        ui.painter().text(
                            egui::pos2(x, rect.center().y),
                            egui::Align2::LEFT_CENTER,
                            cell(r[j]),
                            mono.clone(),
                            ui.visuals().text_color(),
                        );
                        x += w;
                    }
                }
            },
        );
    });
    ui.data_mut(|d| d.insert_temp(id, st));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table {
            columns: vec!["class".into(), "area".into(), "np".into()],
            rows: vec![vec![1.0, 2.5, 3.0], vec![2.0, f64::NAN, 1.0], vec![11.0, 0.5, 7.0]],
        }
    }

    #[test]
    fn filtered_and_sorted() {
        let t = table();
        let st = |filter: &str, sort| TableState { filter: filter.into(), sort, chart: Default::default() };
        assert_eq!(rows_shown(&t, &st("", None)).unwrap(), [0, 1, 2]);
        assert_eq!(rows_shown(&t, &st("area > 1", None)).unwrap(), [0]);
        assert_eq!(rows_shown(&t, &st("AREA <= 2.5", None)).unwrap(), [0, 2]);
        assert_eq!(rows_shown(&t, &st("area = NA", None)).unwrap(), [1]);
        assert_eq!(rows_shown(&t, &st("np != 1", Some((2, false)))).unwrap(), [2, 0]);
        // words: a cell that is it or starts with it
        assert_eq!(rows_shown(&t, &st("1", None)).unwrap(), [0, 1, 2]);
        assert_eq!(rows_shown(&t, &st("na", None)).unwrap(), [1]);
        // NA last whichever the order
        assert_eq!(rows_shown(&t, &st("", Some((1, true)))).unwrap(), [2, 0, 1]);
        assert_eq!(rows_shown(&t, &st("", Some((1, false)))).unwrap(), [0, 2, 1]);
        assert!(rows_shown(&t, &st("size > 1", None)).is_err());
        assert!(rows_shown(&t, &st("area > big", None)).is_err());
        assert_eq!(cell(2.0), "2");
        assert_eq!(cell(0.1234567), "0.123457");
        assert_eq!(cell(f64::NAN), "NA");
    }
}
