//! Tables drawn as charts in the viewer, several metrics at once: bars, lines or points of some columns (a panel
//! each) against a column, the rows split by another (a class, a zone, a cluster: a colour each), as landschaft's
//! plot_metric and plot_cluster_metrics; the clustergram of metrics.clustergram's means (landschaft's
//! plot_clustergram); and a correlation matrix (metrics.correlation) as a heatmap, as show_correlation.

use egui::{Color32, Ui, Vec2};
use egui_plot::{Bar, BarChart, Heatmap, Line, Plot, PlotPoint, Points};
use terraflux_ops::Table;

use crate::palette::{Ramp, class_colour};
use crate::table_view::cell;

/// What a table is drawn as.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    None,
    /// A bar per row, a panel per column.
    Bars,
    /// The columns along another (a date), a line per group.
    Lines,
    /// The columns against another, a point per row coloured by its group.
    Points,
    /// The clustergram of a table of cluster means by k (columns k1, k2...).
    Clustergram,
    /// A square table (a correlation matrix) as a heatmap.
    Matrix,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::None => "none",
            Kind::Bars => "bars",
            Kind::Lines => "lines",
            Kind::Points => "points",
            Kind::Clustergram => "clustergram",
            Kind::Matrix => "matrix",
        }
    }

    /// The kinds a table can be drawn as.
    pub fn of(t: &Table) -> Vec<Kind> {
        let mut v = vec![Kind::None];
        if t.columns.len() > 1 {
            v.extend([Kind::Bars, Kind::Lines, Kind::Points]);
        }
        if !ks(t).is_empty() {
            v.push(Kind::Clustergram);
        }
        if is_matrix(t) {
            v.push(Kind::Matrix);
        }
        v
    }
}

/// The columns of a table named `k<number>` (a clustergram's): (k, column), by k.
pub fn ks(t: &Table) -> Vec<(usize, usize)> {
    let mut v: Vec<(usize, usize)> = t
        .columns
        .iter()
        .enumerate()
        .filter_map(|(j, c)| c.strip_prefix('k').and_then(|k| k.parse().ok()).map(|k| (k, j)))
        .collect();
    v.sort_unstable();
    v
}

/// Whether a table is a matrix: as many rows as columns (a correlation).
pub fn is_matrix(t: &Table) -> bool {
    t.columns.len() >= 2 && t.rows.len() == t.columns.len()
}

/// The rows split by the values of a column (in their first order), or all of them as one series.
pub fn groups(t: &Table, rows: &[usize], by: Option<usize>) -> Vec<(Option<f64>, Vec<usize>)> {
    let Some(j) = by.filter(|&j| j < t.columns.len()) else {
        return vec![(None, rows.to_vec())];
    };
    let mut out: Vec<(Option<f64>, Vec<usize>)> = Vec::new();
    for &i in rows {
        let v = t.rows[i][j];
        match out.iter_mut().find(|(g, _)| g.is_some_and(|g| g.to_bits() == v.to_bits())) {
            Some((_, r)) => r.push(i),
            None => out.push((Some(v), vec![i])),
        }
    }
    out
}

/// A clustergram: per k a dot at each cluster's mean sized by its rows, and between k and k + 1 a line between
/// the means of the clusters that share rows, as wide as the rows they share (landschaft's plot_clustergram).
#[derive(Debug, Clone, PartialEq)]
pub struct Clustergram {
    /// (k, mean, rows)
    pub dots: Vec<(f64, f64, usize)>,
    /// (k, mean at k, mean at k + 1, rows)
    pub lines: Vec<(f64, f64, f64, usize)>,
}

/// The clustergram of the rows of a table of means by k, if it has such columns.
pub fn clustergram(t: &Table, rows: &[usize]) -> Option<Clustergram> {
    let ks = ks(t);
    if ks.is_empty() {
        return None;
    }
    let count = |pairs: Vec<Vec<f64>>| -> Vec<(Vec<f64>, usize)> {
        let mut out: Vec<(Vec<f64>, usize)> = Vec::new();
        for p in pairs {
            match out.iter_mut().find(|(q, _)| q.iter().zip(&p).all(|(a, b)| a.to_bits() == b.to_bits())) {
                Some((_, n)) => *n += 1,
                None => out.push((p, 1)),
            }
        }
        out
    };
    let mut dots = Vec::new();
    let mut lines = Vec::new();
    for (n, &(k, j)) in ks.iter().enumerate() {
        for (m, c) in count(rows.iter().map(|&i| vec![t.rows[i][j]]).filter(|v| !v[0].is_nan()).collect()) {
            dots.push((k as f64, m[0], c));
        }
        if let Some(&(_, j2)) = ks.get(n + 1).filter(|(k2, _)| *k2 == k + 1) {
            let pairs = rows.iter().map(|&i| vec![t.rows[i][j], t.rows[i][j2]]);
            for (p, c) in count(pairs.filter(|p| !p[0].is_nan() && !p[1].is_nan()).collect()) {
                lines.push((k as f64, p[0], p[1], c));
            }
        }
    }
    Some(Clustergram { dots, lines })
}

/// The colour of the `g`-th group (or the accent for a single series).
fn colour(g: usize, groups: usize) -> Color32 {
    if groups <= 1 { crate::theme::ACCENT } else { class_colour(g) }
}

/// A group's name in a legend.
fn group_name(t: &Table, by: Option<usize>, g: Option<f64>) -> String {
    match (by, g) {
        (Some(j), Some(v)) => format!("{} {}", t.columns[j], cell(v)),
        _ => String::new(),
    }
}

/// The settings of a chart: its kind, the columns drawn, the column along x and the one that splits the rows.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chart {
    pub kind: Kind,
    pub columns: Vec<usize>,
    pub x: Option<usize>,
    pub by: Option<usize>,
}

impl Chart {
    /// A chart of a kind with what a table suggests: the first metric (not a key), along the date, split by the
    /// class, zone or cluster.
    pub fn suggested(t: &Table, kind: Kind) -> Chart {
        let find = |names: &[&str]| t.columns.iter().position(|c| names.contains(&c.as_str()));
        let keys = terraflux_ops::sampling::KEYS;
        let metrics: Vec<usize> = (0..t.columns.len()).filter(|&j| !keys.contains(&t.columns[j].as_str())).collect();
        let first = metrics.first().copied().unwrap_or(1.min(t.columns.len().saturating_sub(1)));
        match kind {
            Kind::Bars => Chart { kind, columns: vec![first], x: Some(0), by: None },
            Kind::Lines => Chart {
                kind,
                columns: vec![first],
                x: find(&["date", "layer"]).or(Some(0)),
                by: find(&["class", "zone", "cluster"]),
            },
            Kind::Points => {
                let second = metrics.get(1).copied().unwrap_or(first);
                Chart { kind, columns: vec![second], x: Some(first), by: find(&["cluster", "class"]) }
            }
            _ => Chart { kind, ..Chart::default() },
        }
    }
}

/// A table's chart: its settings above it (kind, columns, x, groups), then the panels.
pub fn show(ui: &mut Ui, t: &Table, rows: &[usize], chart: &mut Chart, id: egui::Id) {
    ui.horizontal_wrapped(|ui| {
        let kinds = Kind::of(t);
        egui::ComboBox::from_id_salt(id.with("kind")).selected_text(format!("chart: {}", chart.kind.name())).show_ui(
            ui,
            |ui| {
                for k in kinds {
                    if ui.selectable_label(chart.kind == k, k.name()).clicked() && chart.kind != k {
                        *chart = Chart::suggested(t, k);
                    }
                }
            },
        );
        if !matches!(chart.kind, Kind::Bars | Kind::Lines | Kind::Points) {
            return;
        }
        let shown = match chart.columns.len() {
            0 => "none".to_string(),
            1..=3 => chart.columns.iter().filter_map(|&j| t.columns.get(j)).cloned().collect::<Vec<_>>().join(", "),
            n => format!("{n} columns"),
        };
        egui::ComboBox::from_id_salt(id.with("columns"))
            .selected_text(format!("of: {shown}"))
            .height(400.0)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .show_ui(ui, |ui| {
                for (j, c) in t.columns.iter().enumerate() {
                    let mut on = chart.columns.contains(&j);
                    if ui.checkbox(&mut on, c).changed() {
                        chart.columns.retain(|&k| k != j);
                        if on {
                            chart.columns.push(j);
                            chart.columns.sort_unstable();
                        }
                    }
                }
            });
        let pick = |ui: &mut Ui, label: &str, salt: &str, value: &mut Option<usize>, none: bool| {
            let name = value.and_then(|j| t.columns.get(j)).map_or("none".to_string(), |c| c.clone());
            egui::ComboBox::from_id_salt(id.with(salt)).selected_text(format!("{label}: {name}")).show_ui(ui, |ui| {
                if none {
                    ui.selectable_value(value, None, "none");
                }
                for (j, c) in t.columns.iter().enumerate() {
                    ui.selectable_value(value, Some(j), c);
                }
            });
        };
        let label = if chart.kind == Kind::Bars { "labels" } else { "along" };
        pick(ui, label, "x", &mut chart.x, false);
        if chart.kind != Kind::Bars {
            pick(ui, "by", "by", &mut chart.by, true);
        }
    });
    match chart.kind {
        Kind::None => {}
        Kind::Bars | Kind::Lines | Kind::Points => panels(ui, t, rows, chart, id),
        Kind::Clustergram => {
            if let Some(cg) = clustergram(t, rows) {
                draw_clustergram(ui, &cg, rows.len(), id);
            }
        }
        Kind::Matrix => matrix(ui, t, id),
    }
}

/// A panel per column drawn, side by side as the width allows.
fn panels(ui: &mut Ui, t: &Table, rows: &[usize], chart: &Chart, id: egui::Id) {
    let columns: Vec<usize> = chart.columns.iter().copied().filter(|&j| j < t.columns.len()).collect();
    if columns.is_empty() {
        return;
    }
    let width = ui.available_width();
    let across = ((width / 340.0).floor() as usize).clamp(1, columns.len());
    let w = width / across as f32 - 8.0;
    let h = if columns.len() == 1 { 200.0 } else { 160.0 };
    let x = chart.x.filter(|&j| j < t.columns.len()).unwrap_or(0);
    let split = groups(t, rows, chart.by);
    let many = split.len();
    // one legend for every panel: the groups' colours
    if many > 1 {
        ui.horizontal_wrapped(|ui| {
            for (g, (value, _)) in split.iter().enumerate() {
                let (rect, _) = ui.allocate_exact_size(Vec2::splat(10.0), egui::Sense::hover());
                ui.painter().circle_filled(rect.center(), 4.5, colour(g, many));
                ui.label(group_name(t, chart.by, *value));
                ui.add_space(6.0);
            }
        });
    }
    ui.horizontal_wrapped(|ui| {
        for &j in &columns {
            ui.allocate_ui(Vec2::new(w, h), |ui| {
                let plot = Plot::new(id.with(("panel", j)))
                    .width(w)
                    .height(h)
                    .allow_scroll(false)
                    .y_axis_label(t.columns[j].clone());
                match chart.kind {
                    Kind::Bars => {
                        let labels: Vec<String> = rows.iter().map(|&i| cell(t.rows[i][x])).collect();
                        let bars: Vec<Bar> = rows
                            .iter()
                            .enumerate()
                            .filter(|(_, i)| !t.rows[**i][j].is_nan())
                            .map(|(k, &i)| {
                                let g = split.iter().position(|(_, r)| r.contains(&i)).unwrap_or(0);
                                Bar::new(k as f64, t.rows[i][j])
                                    .name(cell(t.rows[i][x]))
                                    .width(0.8)
                                    .fill(colour(g, many))
                            })
                            .collect();
                        plot.x_axis_label(t.columns[x].clone())
                            .x_axis_formatter(move |m, _| {
                                let k = m.value.round();
                                if (m.value - k).abs() < 1e-6 && k >= 0.0 {
                                    labels.get(k as usize).cloned().unwrap_or_default()
                                } else {
                                    String::new()
                                }
                            })
                            .show(ui, |p| p.bar_chart(BarChart::new(t.columns[j].clone(), bars)));
                    }
                    Kind::Lines => {
                        plot.x_axis_label(t.columns[x].clone()).show(ui, |p| {
                            for (g, (value, members)) in split.iter().enumerate() {
                                let mut pts: Vec<[f64; 2]> = members
                                    .iter()
                                    .map(|&i| [t.rows[i][x], t.rows[i][j]])
                                    .filter(|p| !p[0].is_nan() && !p[1].is_nan())
                                    .collect();
                                pts.sort_by(|a, b| a[0].total_cmp(&b[0]));
                                let name = group_name(t, chart.by, *value);
                                p.line(Line::new(name.clone(), pts.clone()).color(colour(g, many)).width(2.0));
                                p.points(Points::new(name, pts).color(colour(g, many)).radius(3.0));
                            }
                        });
                    }
                    _ => {
                        plot.x_axis_label(t.columns[x].clone()).show(ui, |p| {
                            for (g, (value, members)) in split.iter().enumerate() {
                                let pts: Vec<[f64; 2]> = members
                                    .iter()
                                    .map(|&i| [t.rows[i][x], t.rows[i][j]])
                                    .filter(|p| !p[0].is_nan() && !p[1].is_nan())
                                    .collect();
                                p.points(
                                    Points::new(group_name(t, chart.by, *value), pts)
                                        .color(colour(g, many))
                                        .radius(3.5)
                                        .filled(true),
                                );
                            }
                        });
                    }
                }
            });
        }
    });
}

/// A clustergram: dots sized by their rows, lines as wide as the rows they share.
fn draw_clustergram(ui: &mut Ui, cg: &Clustergram, n: usize, id: egui::Id) {
    let n = n.max(1) as f32;
    let line = ui.visuals().text_color().gamma_multiply(0.7);
    Plot::new(id.with("clustergram"))
        .height(260.0)
        .allow_scroll(false)
        .x_axis_label("number of clusters (k)")
        .y_axis_label("mean of the clusters")
        .x_axis_formatter(
            |m, _| if (m.value - m.value.round()).abs() < 1e-6 { format!("{}", m.value) } else { String::new() },
        )
        .show(ui, |p| {
            for &(k, a, b, c) in &cg.lines {
                p.line(Line::new("", vec![[k, a], [k + 1.0, b]]).color(line).width((c as f32 / n * 24.0).max(1.0)));
            }
            for &(k, m, c) in &cg.dots {
                p.points(
                    Points::new("", vec![[k, m]])
                        .color(Color32::from_rgb(214, 39, 40))
                        .radius((c as f32 / n).sqrt() * 18.0 + 2.0)
                        .filled(true),
                );
            }
        });
}

/// A square table as a heatmap: blue to red around 0 (a correlation), its values written.
fn matrix(ui: &mut Ui, t: &Table, id: egui::Id) {
    let m = t.columns.len();
    let top = t.rows.iter().flatten().filter(|v| v.is_finite()).fold(0.0f64, |a, v| a.max(v.abs()));
    let top = if top > 0.0 { top } else { 1.0 };
    // the first row on top
    let values: Vec<f64> = (0..m).rev().flat_map(|i| t.rows[i].iter().copied()).collect();
    let names = t.columns.clone();
    let names_y = t.columns.clone();
    let side = ui.available_width().clamp(200.0, 520.0);
    Plot::new(id.with("matrix"))
        .width(side)
        .height(side)
        .data_aspect(1.0)
        .allow_scroll(false)
        .allow_drag(false)
        .show_grid(false)
        .x_axis_formatter(move |g, _| {
            let k = g.value.round();
            if (g.value - k).abs() < 1e-6 && k >= 0.0 {
                names.get(k as usize).cloned().unwrap_or_default()
            } else {
                String::new()
            }
        })
        .y_axis_formatter(move |g, _| {
            let k = g.value.round();
            if (g.value - k).abs() < 1e-6 && k >= 0.0 && (k as usize) < m {
                names_y[m - 1 - k as usize].clone()
            } else {
                String::new()
            }
        })
        .show(ui, |p| {
            p.heatmap(
                Heatmap::new(values, m)
                    .at(PlotPoint::new(-0.5, -0.5))
                    .custom_mapping(Box::new(move |v| {
                        if v.is_nan() { Color32::GRAY } else { Ramp::Diverging.colour(((v / top + 1.0) / 2.0) as f32) }
                    }))
                    .formatter(Box::new(|v| if v.is_nan() { "NA".into() } else { format!("{v:.2}") }))
                    .show_labels(m <= 16),
            );
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_and_clustergram() {
        let t = Table {
            columns: vec!["zone".into(), "k1".into(), "k2".into(), "k3".into()],
            rows: vec![
                vec![1.0, 0.0, -1.0, -2.0],
                vec![2.0, 0.0, -1.0, -1.5],
                vec![3.0, 0.0, 1.0, 1.0],
                vec![4.0, 0.0, -1.0, -2.0],
            ],
        };
        assert_eq!(ks(&t), [(1, 1), (2, 2), (3, 3)]);
        assert!(Kind::of(&t).contains(&Kind::Clustergram) && Kind::of(&t).contains(&Kind::Matrix));
        let rows = [0, 1, 2, 3];
        let g = groups(&t, &rows, Some(2));
        assert_eq!(g, vec![(Some(-1.0), vec![0, 1, 3]), (Some(1.0), vec![2])]);
        assert_eq!(groups(&t, &rows, None), vec![(None, vec![0, 1, 2, 3])]);
        let cg = clustergram(&t, &rows).unwrap();
        assert_eq!(
            cg.dots,
            vec![(1.0, 0.0, 4), (2.0, -1.0, 3), (2.0, 1.0, 1), (3.0, -2.0, 2), (3.0, -1.5, 1), (3.0, 1.0, 1)]
        );
        assert_eq!(
            cg.lines,
            vec![
                (1.0, 0.0, -1.0, 3),
                (1.0, 0.0, 1.0, 1),
                (2.0, -1.0, -2.0, 2),
                (2.0, -1.0, -1.5, 1),
                (2.0, 1.0, 1.0, 1)
            ]
        );
    }

    #[test]
    fn suggested_charts() {
        let t = Table {
            columns: vec!["class".into(), "date".into(), "ca".into(), "np".into()],
            rows: vec![vec![1.0, 2001.0, 3.0, 4.0]],
        };
        let lines = Chart::suggested(&t, Kind::Lines);
        assert_eq!((lines.columns, lines.x, lines.by), (vec![2], Some(1), Some(0)));
        let points = Chart::suggested(&t, Kind::Points);
        assert_eq!((points.columns, points.x, points.by), (vec![3], Some(2), Some(0)));
        assert!(!Kind::of(&t).contains(&Kind::Matrix));
    }
}
