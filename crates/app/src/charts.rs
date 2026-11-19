//! Tables drawn as charts in the viewer, several metrics at once: bars, lines or points of some columns (a panel
//! each) against a column, the rows split by another (a class, a zone, a cluster: a colour each), as the library's
//! plot_metric and plot_cluster_metrics; the clustergram of metrics.clustergram's means (the library's
//! plot_clustergram); box plots of the columns by group (plot_cluster_metrics); the loadings of principal
//! components as arrows in the circle of correlations (metrics.pca's loadings); and a correlation matrix
//! (metrics.correlation) as a heatmap, as show_correlation; and ml.pca's elbow, ml.kmeans's choice of k and ml.som's
//! grid of nodes (squares or hexagons) in several views, whose picked node the Viewer links to the map.

use egui::{Color32, RichText, Ui, Vec2};
use egui_plot::{
    Arrows, Bar, BarChart, BoxElem, BoxPlot, BoxSpread, Heatmap, Line, Plot, PlotPoint, Points, Text, VLine,
};
use terraflux_ops::Table;

use crate::palette::{Ramp, class_colour};
use crate::table_view::{cell, cell_of};

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
    /// Box plots of the columns, a box per group.
    Boxes,
    /// The clustergram of a table of cluster means by k (columns k1, k2...).
    Clustergram,
    /// metrics.pca's loadings (a row per component): every metric an arrow in the circle of correlations.
    Loadings,
    /// A square table (a correlation matrix) as a heatmap.
    Matrix,
    /// segment.scale's table: LV and ROC against the scale, the suggested scales marked.
    Esp,
    /// ml.pca's explained variance (a row per component): the share of each as a bar, the cumulative share a line.
    Elbow,
    /// ml.kmeans's choice of k (a row per k): the inertia and the silhouette against k, the best silhouette marked.
    Choice,
    /// ml.som's codebook (a row per node): the grid of nodes as squares or hexagons, coloured by the U-matrix, a
    /// variable's weights, the records each holds or its group; a click on a node picks it.
    Som,
    /// segment.optimize's scores (a row per value): the objective, the variance and Moran's I scaled 0-1 against the
    /// value, the best marked, and the number of objects in a smaller panel.
    Scores,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::None => "none",
            Kind::Bars => "bars",
            Kind::Lines => "lines",
            Kind::Points => "points",
            Kind::Boxes => "box plots",
            Kind::Clustergram => "clustergram",
            Kind::Loadings => "loadings",
            Kind::Matrix => "matrix",
            Kind::Esp => "ESP",
            Kind::Elbow => "elbow",
            Kind::Choice => "choice of k",
            Kind::Som => "SOM",
            Kind::Scores => "scores",
        }
    }

    /// The kinds a table can be drawn as.
    pub fn of(t: &Table) -> Vec<Kind> {
        let mut v = vec![Kind::None];
        if t.columns.len() > 1 {
            v.extend([Kind::Bars, Kind::Lines, Kind::Points, Kind::Boxes]);
        }
        if is_loadings(t) {
            v.push(Kind::Loadings);
        }
        if !ks(t).is_empty() {
            v.push(Kind::Clustergram);
        }
        if is_matrix(t) {
            v.push(Kind::Matrix);
        }
        if is_esp(t) {
            v.push(Kind::Esp);
        }
        if is_elbow(t) {
            v.push(Kind::Elbow);
        }
        if is_choice(t) {
            v.push(Kind::Choice);
        }
        if is_som(t) {
            v.push(Kind::Som);
        }
        if is_scores(t) {
            v.push(Kind::Scores);
        }
        v
    }
}

/// Whether a table is segment.optimize's scores: a row per value, the columns value, objects, variance, moran and
/// objective.
pub fn is_scores(t: &Table) -> bool {
    t.columns == ["value", "objects", "variance", "moran", "objective"] && !t.rows.is_empty()
}

/// Whether a table is ml.pca's explained variance: a row per component, the columns component, variance, ratio and
/// cumulative.
pub fn is_elbow(t: &Table) -> bool {
    t.columns == ["component", "variance", "ratio", "cumulative"] && !t.rows.is_empty()
}

/// Whether a table is ml.kmeans's choice of k: a row per k, the columns k, records, inertia, silhouette and
/// calinski_harabasz.
pub fn is_choice(t: &Table) -> bool {
    t.columns == ["k", "records", "inertia", "silhouette", "calinski_harabasz"] && !t.rows.is_empty()
}

/// Whether a table is segment.scale's: a row per scale (columns scale, roc_mean and suggested, and an lv_<band> for
/// each band).
pub fn is_esp(t: &Table) -> bool {
    let has = |n: &str| t.columns.iter().any(|c| c == n);
    has("scale") && has("roc_mean") && has("suggested") && t.columns.iter().any(|c| c.starts_with("lv_"))
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

/// Whether a table is metrics.pca's loadings: a row per component (columns component and explained, then the
/// metrics), two at least.
pub fn is_loadings(t: &Table) -> bool {
    t.columns.len() > 2 && t.columns[0] == "component" && t.columns[1] == "explained" && t.rows.len() >= 2
}

/// A box plot's numbers of some values (NaN left out), as matplotlib's: the quartiles by linear interpolation, the
/// whiskers at the furthest values within 1.5 times the box's height, and the values beyond them.
pub fn box_stats(values: &[f64]) -> Option<([f64; 5], Vec<f64>)> {
    let mut v: Vec<f64> = values.iter().copied().filter(|x| !x.is_nan()).collect();
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let q = |p: f64| {
        let h = p * (v.len() - 1) as f64;
        let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
        v[lo] + (h - lo as f64) * (v[hi] - v[lo])
    };
    let (q1, median, q3) = (q(0.25), q(0.5), q(0.75));
    let (low, high) = (q1 - 1.5 * (q3 - q1), q3 + 1.5 * (q3 - q1));
    let lo = v.iter().copied().find(|&x| x >= low).unwrap_or(q1);
    let hi = v.iter().rev().copied().find(|&x| x <= high).unwrap_or(q3);
    let out = v.iter().copied().filter(|&x| x < lo || x > hi).collect();
    Some(([lo, q1, median, q3, hi], out))
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
/// the means of the clusters that share rows, as wide as the rows they share (the library's plot_clustergram).
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
    crate::palette::seen(if groups <= 1 { crate::theme::accent() } else { class_colour(g) })
}

/// A group's name in a legend.
fn group_name(t: &Table, by: Option<usize>, g: Option<f64>) -> String {
    match (by, g) {
        (Some(j), Some(v)) => match t.label(&t.columns[j], v) {
            Some(name) => name.to_string(),
            None => format!("{} {}", t.columns[j], cell(v)),
        },
        _ => String::new(),
    }
}

/// The settings of a chart: its kind, the columns drawn, the column along x and the one that splits the rows; for
/// loadings the two components (rows) drawn.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Chart {
    pub kind: Kind,
    pub columns: Vec<usize>,
    pub x: Option<usize>,
    pub by: Option<usize>,
    pub components: [usize; 2],
    /// For a SOM: what its nodes are coloured by.
    pub som: SomView,
}

impl Chart {
    /// The chart a table opens with: the SOM for a codebook (its grid is what it is for), the curves of the scores of
    /// segment.optimize, else none.
    pub fn first(t: &Table) -> Chart {
        if is_som(t) {
            Chart::suggested(t, Kind::Som)
        } else if is_scores(t) {
            Chart::suggested(t, Kind::Scores)
        } else {
            Chart::default()
        }
    }

    /// A chart of a kind with what a table suggests: the first metric (not a key), along the date, split by the
    /// class, zone or cluster.
    pub fn suggested(t: &Table, kind: Kind) -> Chart {
        let find = |names: &[&str]| t.columns.iter().position(|c| names.contains(&c.as_str()));
        let keys = terraflux_ops::sampling::KEYS;
        let metrics: Vec<usize> = (0..t.columns.len()).filter(|&j| !keys.contains(&t.columns[j].as_str())).collect();
        let first = metrics.first().copied().unwrap_or(1.min(t.columns.len().saturating_sub(1)));
        match kind {
            Kind::Bars => Chart { kind, columns: vec![first], x: Some(0), ..Chart::default() },
            Kind::Lines => Chart {
                kind,
                columns: vec![first],
                x: find(&["date", "layer"]).or(Some(0)),
                by: find(&["class", "zone", "cluster"]),
                ..Chart::default()
            },
            Kind::Points => {
                let second = metrics.get(1).copied().unwrap_or(first);
                Chart {
                    kind,
                    columns: vec![second],
                    x: Some(first),
                    by: find(&["cluster", "class"]),
                    ..Chart::default()
                }
            }
            Kind::Boxes => {
                Chart { kind, columns: vec![first], by: find(&["cluster", "class", "zone"]), ..Chart::default() }
            }
            Kind::Loadings => Chart { kind, components: [0, 1], ..Chart::default() },
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
        if chart.kind == Kind::Loadings {
            for (k, label) in ["x", "y"].iter().enumerate() {
                let c = &mut chart.components[k];
                egui::ComboBox::from_id_salt(id.with(("component", k)))
                    .selected_text(format!("{label}: PC{}", *c + 1))
                    .show_ui(ui, |ui| {
                        for r in 0..t.rows.len() {
                            ui.selectable_value(c, r, format!("PC{}", r + 1));
                        }
                    });
            }
        }
        if chart.kind == Kind::Som {
            let grouped = som_map(t).is_some_and(|m| m.grouped());
            for v in SomView::ALL.into_iter().filter(|v| grouped || *v != SomView::Groups) {
                ui.selectable_value(&mut chart.som, v, v.name());
            }
        }
        if !matches!(chart.kind, Kind::Bars | Kind::Lines | Kind::Points | Kind::Boxes) {
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
        if chart.kind != Kind::Boxes {
            let label = if chart.kind == Kind::Bars { "labels" } else { "along" };
            pick(ui, label, "x", &mut chart.x, false);
        }
        if chart.kind != Kind::Bars {
            pick(ui, "by", "by", &mut chart.by, true);
        }
    });
    match chart.kind {
        Kind::None => {}
        Kind::Bars | Kind::Lines | Kind::Points | Kind::Boxes => panels(ui, t, rows, chart, id),
        Kind::Clustergram => {
            if let Some(cg) = clustergram(t, rows) {
                draw_clustergram(ui, &cg, rows.len(), id);
            }
        }
        Kind::Matrix => matrix(ui, t, id),
        Kind::Esp => esp(ui, t, rows, id),
        Kind::Elbow => elbow(ui, t, rows, id),
        Kind::Choice => choice(ui, t, rows, id),
        Kind::Som => som(ui, t, chart.som, id),
        Kind::Scores => scores(ui, t, rows, id),
        Kind::Loadings => loadings(ui, t, chart.components, id),
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
                let plot = crate::theme::plot(Plot::new(id.with(("panel", j))))
                    .width(w)
                    .height(h)
                    .allow_scroll(false)
                    .y_axis_label(t.columns[j].clone());
                match chart.kind {
                    Kind::Bars => {
                        let labels: Vec<String> = rows.iter().map(|&i| cell_of(t, x, t.rows[i][x])).collect();
                        let bars: Vec<Bar> = rows
                            .iter()
                            .enumerate()
                            .filter(|(_, i)| !t.rows[**i][j].is_nan())
                            .map(|(k, &i)| {
                                let g = split.iter().position(|(_, r)| r.contains(&i)).unwrap_or(0);
                                Bar::new(k as f64, t.rows[i][j])
                                    .name(cell_of(t, x, t.rows[i][x]))
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
                            .show(ui, |p| {
                                wheel_zoom(p);
                                p.bar_chart(BarChart::new(t.columns[j].clone(), bars));
                            });
                    }
                    Kind::Boxes => {
                        let names: Vec<String> = split.iter().map(|(g, _)| group_name(t, chart.by, *g)).collect();
                        let label = chart.by.map_or(String::new(), |b| t.columns[b].clone());
                        plot.x_axis_label(label)
                            .x_axis_formatter(move |m, _| {
                                let k = m.value.round();
                                if (m.value - k).abs() < 1e-6 && k >= 0.0 {
                                    names.get(k as usize).cloned().unwrap_or_default()
                                } else {
                                    String::new()
                                }
                            })
                            .show(ui, |p| {
                                wheel_zoom(p);
                                for (g, (value, members)) in split.iter().enumerate() {
                                    let values: Vec<f64> = members.iter().map(|&i| t.rows[i][j]).collect();
                                    let Some((s, out)) = box_stats(&values) else { continue };
                                    let c = colour(g, many);
                                    let b = BoxElem::new(g as f64, BoxSpread::new(s[0], s[1], s[2], s[3], s[4]))
                                        .name(group_name(t, chart.by, *value))
                                        .box_width(0.6)
                                        .whisker_width(0.3)
                                        .fill(c.gamma_multiply(0.4))
                                        .stroke(egui::Stroke::new(1.5, c));
                                    p.box_plot(BoxPlot::new(t.columns[j].clone(), vec![b]));
                                    let pts: Vec<[f64; 2]> = out.iter().map(|&v| [g as f64, v]).collect();
                                    p.points(Points::new("", pts).color(c).radius(2.5));
                                }
                            });
                    }
                    Kind::Lines => {
                        plot.x_axis_label(t.columns[x].clone()).show(ui, |p| {
                            wheel_zoom(p);
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
                            wheel_zoom(p);
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

/// The wheel zooms a chart around the point under the pointer, as on a map (the page under it does not scroll
/// then); ctrl and the wheel, or dragging, as egui_plot does.
fn wheel_zoom(p: &mut egui_plot::PlotUi) {
    if !p.response().hovered() {
        return;
    }
    let scroll = p.ctx().input(|i| i.smooth_scroll_delta.y);
    if scroll != 0.0 {
        p.zoom_bounds_around_hovered(Vec2::splat((scroll / 200.0).exp()));
        p.ctx().input_mut(|i| i.smooth_scroll_delta = Vec2::ZERO);
    }
}

/// A clustergram: dots sized by their rows, lines as wide as the rows they share.
fn draw_clustergram(ui: &mut Ui, cg: &Clustergram, n: usize, id: egui::Id) {
    let n = n.max(1) as f32;
    let line = ui.visuals().text_color().gamma_multiply(0.7);
    crate::theme::plot(Plot::new(id.with("clustergram")))
        .height(260.0)
        .allow_scroll(false)
        .x_axis_label("number of clusters (k)")
        .y_axis_label("mean of the clusters")
        .x_axis_formatter(
            |m, _| if (m.value - m.value.round()).abs() < 1e-6 { format!("{}", m.value) } else { String::new() },
        )
        .show(ui, |p| {
            wheel_zoom(p);
            for &(k, a, b, c) in &cg.lines {
                p.line(Line::new("", vec![[k, a], [k + 1.0, b]]).color(line).width((c as f32 / n * 24.0).max(1.0)));
            }
            for &(k, m, c) in &cg.dots {
                p.points(
                    Points::new("", vec![[k, m]])
                        .color(crate::palette::seen(Color32::from_rgb(214, 39, 40)))
                        .radius((c as f32 / n).sqrt() * 18.0 + 2.0)
                        .filled(true),
                );
            }
        });
}

/// metrics.pca's loadings as the circle of correlations: every metric an arrow from the centre to its correlations
/// with two components.
fn loadings(ui: &mut Ui, t: &Table, [a, b]: [usize; 2], id: egui::Id) {
    if a >= t.rows.len() || b >= t.rows.len() {
        return;
    }
    let axis = |r: usize| format!("PC{} ({:.0}%)", r + 1, t.rows[r][1] * 100.0);
    let colour = crate::palette::seen(crate::theme::accent());
    let text = ui.visuals().text_color();
    let side = ui.available_width().clamp(240.0, 520.0);
    crate::theme::plot(Plot::new(id.with("loadings")))
        .width(side)
        .height(side)
        .data_aspect(1.0)
        .allow_scroll(false)
        .x_axis_label(axis(a))
        .y_axis_label(axis(b))
        .include_x(-1.1)
        .include_x(1.1)
        .include_y(-1.1)
        .include_y(1.1)
        .show(ui, |p| {
            wheel_zoom(p);
            let circle: Vec<[f64; 2]> = (0..=96)
                .map(|k| {
                    let t = k as f64 / 96.0 * std::f64::consts::TAU;
                    [t.cos(), t.sin()]
                })
                .collect();
            p.line(Line::new("", circle).color(text.gamma_multiply(0.4)).width(1.0));
            let metrics: Vec<usize> = (2..t.columns.len()).collect();
            // the correlation of a metric with a component: its loading times the component's deviation (the share
            // of the variance times the metrics, each of variance 1 as z-scores)
            let m = metrics.len() as f64;
            let (sa, sb) = ((t.rows[a][1] * m).sqrt(), (t.rows[b][1] * m).sqrt());
            let tips: Vec<[f64; 2]> = metrics.iter().map(|&j| [t.rows[a][j] * sa, t.rows[b][j] * sb]).collect();
            let origins = vec![[0.0, 0.0]; tips.len()];
            p.arrows(Arrows::new("loadings", origins, tips.clone()).color(colour).tip_length(8.0));
            for (&j, tip) in metrics.iter().zip(&tips) {
                p.text(Text::new("", PlotPoint::new(tip[0] * 1.08, tip[1] * 1.08), t.columns[j].clone()).color(text));
            }
        });
}

/// ml.pca's explained variance as an elbow: a bar per component with its share of the variance, and the cumulative
/// share as a line over it.
fn elbow(ui: &mut Ui, t: &Table, rows: &[usize], id: egui::Id) {
    let colour = crate::palette::seen(crate::theme::accent());
    let line = ui.visuals().text_color().gamma_multiply(0.6);
    let bars: Vec<Bar> = rows
        .iter()
        .map(|&i| {
            Bar::new(t.rows[i][0], t.rows[i][2] * 100.0).name(format!("PC{}", t.rows[i][0])).width(0.7).fill(colour)
        })
        .collect();
    let cum: Vec<[f64; 2]> = rows.iter().map(|&i| [t.rows[i][0], t.rows[i][3] * 100.0]).collect();
    crate::theme::plot(Plot::new(id.with("elbow")))
        .width(ui.available_width().clamp(240.0, 520.0))
        .height(220.0)
        .allow_scroll(false)
        .x_axis_label("component")
        .y_axis_label("% of the variance")
        .include_y(0.0)
        .include_y(100.0)
        .x_axis_formatter(|m, _| {
            let k = m.value.round();
            if (m.value - k).abs() < 1e-6 && k >= 1.0 { format!("PC{k}") } else { String::new() }
        })
        .show(ui, |p| {
            wheel_zoom(p);
            p.bar_chart(BarChart::new("share", bars));
            p.line(Line::new("cumulative", cum.clone()).color(line).width(1.5));
            p.points(Points::new("cumulative", cum).color(line).radius(3.0));
        });
}

/// ml.kmeans's choice of k: the inertia (to look for the elbow) and the silhouette (higher is better, the best marked
/// by a vertical line) against k, side by side.
fn choice(ui: &mut Ui, t: &Table, rows: &[usize], id: egui::Id) {
    let mut inertia: Vec<[f64; 2]> = Vec::new();
    let mut silhouette: Vec<[f64; 2]> = Vec::new();
    let mut best: Option<[f64; 2]> = None;
    for &i in rows {
        let r = &t.rows[i];
        if r[2].is_finite() {
            inertia.push([r[0], r[2]]);
        }
        if r[3].is_finite() {
            silhouette.push([r[0], r[3]]);
            if best.is_none_or(|b| r[3] > b[1]) {
                best = Some([r[0], r[3]]);
            }
        }
    }
    let colour = crate::palette::seen(crate::theme::accent());
    let mark = ui.visuals().text_color().gamma_multiply(0.5);
    let w = (ui.available_width() / 2.0 - 8.0).max(240.0);
    ui.horizontal_wrapped(|ui| {
        for (n, (label, pts)) in [("inertia", inertia), ("silhouette", silhouette)].into_iter().enumerate() {
            ui.allocate_ui(Vec2::new(w, 200.0), |ui| {
                crate::theme::plot(Plot::new(id.with(("choice", n))))
                    .width(w)
                    .height(200.0)
                    .allow_scroll(false)
                    .x_axis_label("k")
                    .y_axis_label(label)
                    .x_axis_formatter(|m, _| {
                        let k = m.value.round();
                        if (m.value - k).abs() < 1e-6 && k >= 1.0 { format!("{k}") } else { String::new() }
                    })
                    .show(ui, |p| {
                        wheel_zoom(p);
                        if n == 1
                            && let Some(b) = best
                        {
                            p.vline(VLine::new("best", b[0]).color(mark));
                        }
                        p.line(Line::new(label, pts.clone()).color(colour).width(2.0));
                        p.points(Points::new(label, pts).color(colour).radius(3.0));
                    });
            });
        }
    });
}

/// The value of the best row of a scores table (the highest objective, the earliest when tied).
pub fn best_value(t: &Table) -> Option<f64> {
    let mut best: Option<(f64, f64)> = None;
    for r in t.rows.iter().filter(|r| r.len() == 5 && r[4].is_finite()) {
        if best.is_none_or(|b| r[4] > b.1) {
            best = Some((r[0], r[4]));
        }
    }
    best.map(|b| b.0)
}

/// segment.optimize's scores: the objective and the variance and Moran's I scaled 0-1 as the operator scales them
/// (1 the lowest, the best) against the value, a vertical line at the best, and the objects each value made in a
/// smaller panel under it.
fn scores(ui: &mut Ui, t: &Table, rows: &[usize], id: egui::Id) {
    let col = |j: usize| -> Vec<f64> { rows.iter().map(|&i| t.rows[i][j]).collect() };
    let value = col(0);
    let (variance, moran) = (terraflux_ops::objects::scaled(&col(2)), terraflux_ops::objects::scaled(&col(3)));
    let line = |y: &[f64]| -> Vec<[f64; 2]> {
        value.iter().zip(y).filter(|(_, y)| y.is_finite()).map(|(&x, &y)| [x, y]).collect()
    };
    let curves =
        [("objective", line(&col(4))), ("variance (scaled)", line(&variance)), ("Moran's I (scaled)", line(&moran))];
    let objects: Vec<[f64; 2]> = rows.iter().map(|&i| [t.rows[i][0], t.rows[i][1]]).collect();
    let best = best_value(t);
    let mark = ui.visuals().text_color().gamma_multiply(0.5);
    let w = ui.available_width().clamp(240.0, 640.0);
    crate::theme::plot(Plot::new(id.with("scores")))
        .width(w)
        .height(220.0)
        .allow_scroll(false)
        .x_axis_label("value")
        .y_axis_label("score (1 is the best of the values)")
        .legend(egui_plot::Legend::default())
        .show(ui, |p| {
            wheel_zoom(p);
            if let Some(b) = best {
                p.vline(VLine::new("best", b).color(mark));
            }
            for (k, (label, pts)) in curves.into_iter().enumerate() {
                let c = colour(k, 3);
                p.line(Line::new(label, pts.clone()).color(c).width(if k == 0 { 2.5 } else { 1.5 }));
                p.points(Points::new(label, pts).color(c).radius(3.0));
            }
        });
    let accent = crate::palette::seen(crate::theme::accent());
    crate::theme::plot(Plot::new(id.with("scores objects")))
        .width(w)
        .height(100.0)
        .allow_scroll(false)
        .x_axis_label("value")
        .y_axis_label("objects")
        .include_y(0.0)
        .show(ui, |p| {
            wheel_zoom(p);
            if let Some(b) = best {
                p.vline(VLine::new("best", b).color(mark));
            }
            p.line(Line::new("objects", objects.clone()).color(accent).width(1.5));
            p.points(Points::new("objects", objects).color(accent).radius(2.5));
        });
}

/// segment.scale's table: the local variance (the mean of the bands' lv_) and the rate of change (roc_mean) against
/// the scale, side by side, a vertical line at each suggested scale.
fn esp(ui: &mut Ui, t: &Table, rows: &[usize], id: egui::Id) {
    let at = |n: &str| t.columns.iter().position(|c| c == n);
    let (Some(sc), Some(roc), Some(sg)) = (at("scale"), at("roc_mean"), at("suggested")) else { return };
    let lvs: Vec<usize> = (0..t.columns.len()).filter(|&j| t.columns[j].starts_with("lv_")).collect();
    let mut lv: Vec<[f64; 2]> = Vec::new();
    let mut rc: Vec<[f64; 2]> = Vec::new();
    let mut marks: Vec<f64> = Vec::new();
    for &i in rows {
        let r = &t.rows[i];
        let v: Vec<f64> = lvs.iter().map(|&j| r[j]).filter(|x| x.is_finite()).collect();
        if !v.is_empty() {
            lv.push([r[sc], v.iter().sum::<f64>() / v.len() as f64]);
        }
        if r[roc].is_finite() {
            rc.push([r[sc], r[roc]]);
        }
        if r[sg] == 1.0 {
            marks.push(r[sc]);
        }
    }
    let colour = crate::palette::seen(crate::theme::accent());
    let mark = ui.visuals().text_color().gamma_multiply(0.5);
    let w = (ui.available_width() / 2.0 - 8.0).max(240.0);
    ui.horizontal_wrapped(|ui| {
        for (k, (label, pts)) in [("LV (mean of the bands)", lv), ("ROC (%)", rc)].into_iter().enumerate() {
            ui.allocate_ui(Vec2::new(w, 200.0), |ui| {
                crate::theme::plot(Plot::new(id.with(("esp", k))))
                    .width(w)
                    .height(200.0)
                    .allow_scroll(false)
                    .x_axis_label("scale")
                    .y_axis_label(label)
                    .show(ui, |p| {
                        wheel_zoom(p);
                        for &x in &marks {
                            p.vline(VLine::new("suggested", x).color(mark));
                        }
                        p.line(Line::new(label, pts.clone()).color(colour).width(2.0));
                        p.points(Points::new(label, pts).color(colour).radius(3.0));
                    });
            });
        }
    });
}

/// The scales an ESP table suggests (its rows with `suggested` 1), by scale.
pub fn suggested_scales(t: &Table) -> Vec<f64> {
    let at = |n: &str| t.columns.iter().position(|c| c == n);
    let (Some(sc), Some(sg)) = (at("scale"), at("suggested")) else { return Vec::new() };
    t.rows.iter().filter(|r| r[sg] == 1.0).map(|r| r[sc]).collect()
}

/// The ROC of an ESP table as one small curve (the Viewer's live segmentation draws it beside the sliders), the
/// suggested scales marked; a click on it is the suggested scale nearest the click, which is returned.
pub fn esp_curve(ui: &mut Ui, t: &Table, id: egui::Id) -> Option<f64> {
    let at = |n: &str| t.columns.iter().position(|c| c == n);
    let (Some(sc), Some(roc)) = (at("scale"), at("roc_mean")) else { return None };
    let pts: Vec<[f64; 2]> = t.rows.iter().filter(|r| r[roc].is_finite()).map(|r| [r[sc], r[roc]]).collect();
    let marks = suggested_scales(t);
    let colour = crate::palette::seen(crate::theme::accent());
    let mark = ui.visuals().text_color().gamma_multiply(0.5);
    let mut picked = None;
    crate::theme::plot(Plot::new(id))
        .height(110.0)
        .allow_scroll(false)
        .allow_drag(false)
        .allow_zoom(false)
        .x_axis_label("scale")
        .y_axis_label("ROC (%)")
        .show(ui, |p| {
            for &x in &marks {
                p.vline(VLine::new("suggested", x).color(mark));
            }
            p.line(Line::new("ROC", pts.clone()).color(colour).width(2.0));
            p.points(Points::new("ROC", pts).color(colour).radius(2.5));
            if p.response().clicked()
                && let Some(c) = p.pointer_coordinate()
            {
                picked = marks.iter().copied().min_by(|a, b| (a - c.x).abs().total_cmp(&(b - c.x).abs()));
            }
        });
    picked
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
    crate::theme::plot(Plot::new(id.with("matrix")))
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
            wheel_zoom(p);
            p.heatmap(
                Heatmap::new(values, m)
                    .at(PlotPoint::new(-0.5, -0.5))
                    .custom_mapping(Box::new(move |v| {
                        if v.is_nan() {
                            Color32::GRAY
                        } else {
                            crate::palette::seen(Ramp::Diverging.colour(((v / top + 1.0) / 2.0) as f32))
                        }
                    }))
                    .formatter(Box::new(|v| if v.is_nan() { "NA".into() } else { format!("{v:.2}") }))
                    .show_labels(m <= 16),
            );
        });
}

/// What the nodes of a SOM are coloured by.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SomView {
    /// The mean distance of each node to its neighbours: large across the gaps between types of records.
    #[default]
    Umatrix,
    /// A grid per variable: its weight at each node.
    Planes,
    /// The records each node holds (the hits).
    Counts,
    /// The group of each node (with `clusters`).
    Groups,
}

impl SomView {
    pub const ALL: [SomView; 4] = [SomView::Umatrix, SomView::Planes, SomView::Counts, SomView::Groups];

    pub fn name(self) -> &'static str {
        match self {
            SomView::Umatrix => "U-matrix",
            SomView::Planes => "component planes",
            SomView::Counts => "counts",
            SomView::Groups => "groups",
        }
    }
}

/// A node or a group of a SOM picked in its view (or on the map), by the numbers of the codebook's `node` and
/// `group` (from 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SomPick {
    Node(usize),
    Group(usize),
}

/// A node of ml.som's codebook: its place (row and column from 1), the records it holds, its group (with clusters),
/// its U-matrix distance and its weights.
#[derive(Debug, Clone, PartialEq)]
pub struct SomNode {
    pub node: usize,
    pub row: usize,
    pub col: usize,
    pub count: f64,
    pub group: Option<usize>,
    pub umatrix: f64,
    pub weights: Vec<f64>,
}

/// ml.som's codebook read as a map: the size of the grid, its topology and the nodes.
#[derive(Debug, Clone, PartialEq)]
pub struct SomMap {
    pub rows: usize,
    pub cols: usize,
    pub hex: bool,
    /// The variables (the weights' columns).
    pub names: Vec<String>,
    pub nodes: Vec<SomNode>,
}

impl SomMap {
    /// Whether the nodes are grouped.
    pub fn grouped(&self) -> bool {
        self.nodes.iter().any(|n| n.group.is_some())
    }

    /// The group of a node (its number, from 1).
    pub fn group_of(&self, node: usize) -> Option<usize> {
        self.nodes.iter().find(|n| n.node == node).and_then(|n| n.group)
    }

    /// Whether a pick covers a node.
    pub fn lit(&self, pick: Option<SomPick>, n: &SomNode) -> bool {
        match pick {
            Some(SomPick::Node(k)) => n.node == k,
            Some(SomPick::Group(g)) => n.group == Some(g),
            None => false,
        }
    }

    /// The records of the nodes a pick covers.
    pub fn records(&self, pick: Option<SomPick>) -> f64 {
        self.nodes.iter().filter(|n| self.lit(pick, n)).map(|n| n.count).sum()
    }
}

/// Whether a table is ml.som's codebook: a row per node, the columns node, row, column, count, [group,] umatrix and
/// topology, then the weights.
pub fn is_som(t: &Table) -> bool {
    som_map(t).is_some()
}

/// ml.som's codebook as a map, if the table is one.
pub fn som_map(t: &Table) -> Option<SomMap> {
    let at = |n: &str| t.columns.iter().position(|c| c == n);
    let (node, row, col, count) = (at("node")?, at("row")?, at("column")?, at("count")?);
    let (umatrix, topology) = (at("umatrix")?, at("topology")?);
    let group = at("group");
    if t.rows.is_empty() {
        return None;
    }
    let whole = |v: f64| (v.is_finite() && v >= 1.0 && v.fract() == 0.0).then_some(v as usize);
    let mut nodes = Vec::with_capacity(t.rows.len());
    for r in &t.rows {
        nodes.push(SomNode {
            node: whole(r[node])?,
            row: whole(r[row])?,
            col: whole(r[col])?,
            count: r[count],
            group: group.and_then(|g| whole(r[g])),
            umatrix: r[umatrix],
            weights: r[topology + 1..].to_vec(),
        });
    }
    Some(SomMap {
        rows: nodes.iter().map(|n| n.row).max()?,
        cols: nodes.iter().map(|n| n.col).max()?,
        hex: t.rows[0][topology] == 2.0,
        names: t.columns[topology + 1..].to_vec(),
        nodes,
    })
}

/// The distance between the rows of a hexagonal map, with the nodes 1 apart along a row: (3 / 2) / sqrt(3).
pub const HEX_ROW: f64 = 0.866_025_403_784_438_6;

/// The centre of the node at `row` and `col` (from 1) of a grid of `rows`, in units of the distance between
/// neighbours, the first row at the top (y down). A hexagonal map's rows are shifted half a node as MiniSom does: the
/// rows counted from the last, every other one.
pub fn node_centre(row: usize, col: usize, rows: usize, hex: bool) -> [f64; 2] {
    let (r, c) = (row - 1, col - 1);
    if !hex {
        return [c as f64, r as f64];
    }
    let shift = if terraflux_ops::multivar::som::shifted(r, rows) { -0.5 } else { 0.0 };
    [c as f64 + shift, r as f64 * HEX_ROW]
}

/// The corners of a node drawn around its centre: a square of side 1, or a hexagon with a point up, 1 across its
/// flat sides (so that neighbours touch along their sides).
pub fn node_corners(centre: [f64; 2], hex: bool) -> Vec<[f64; 2]> {
    if !hex {
        return [(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)]
            .iter()
            .map(|(x, y)| [centre[0] + x, centre[1] + y])
            .collect();
    }
    let radius = 1.0 / 3.0_f64.sqrt();
    (0..6)
        .map(|k| {
            let a = (60.0 * k as f64 + 30.0).to_radians();
            [centre[0] + radius * a.cos(), centre[1] + radius * a.sin()]
        })
        .collect()
}

/// The box a grid's nodes cover: (min x, min y, max x, max y).
pub fn grid_extent(rows: usize, cols: usize, hex: bool) -> [f64; 4] {
    let mut e = [f64::MAX, f64::MAX, f64::MIN, f64::MIN];
    for r in [1, rows] {
        for c in [1, cols] {
            for p in node_corners(node_centre(r, c, rows, hex), hex) {
                e = [e[0].min(p[0]), e[1].min(p[1]), e[2].max(p[0]), e[3].max(p[1])];
            }
        }
    }
    // (a hexagonal map's rows are shifted by turns: the other column edge too)
    if hex {
        for r in 1..=rows.min(2) {
            for c in [1, cols] {
                for p in node_corners(node_centre(r, c, rows, hex), hex) {
                    e = [e[0].min(p[0]), e[1].min(p[1]), e[2].max(p[0]), e[3].max(p[1])];
                }
            }
        }
    }
    e
}

/// The node (row, column from 1) whose cell holds the point `p` (in the units of [`node_centre`]).
pub fn node_at(p: [f64; 2], rows: usize, cols: usize, hex: bool) -> Option<(usize, usize)> {
    let mut best: Option<(f64, usize, usize)> = None;
    for r in 1..=rows {
        for c in 1..=cols {
            let m = node_centre(r, c, rows, hex);
            let d = (p[0] - m[0]).powi(2) + (p[1] - m[1]).powi(2);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, r, c));
            }
        }
    }
    let (_, r, c) = best?;
    let m = node_centre(r, c, rows, hex);
    // (square cells: the square; hexagonal cells: the nearest centre's, which is the hexagon while inside the radius)
    let inside = if hex {
        (p[0] - m[0]).hypot(p[1] - m[1]) <= 1.0 / 3.0_f64.sqrt()
    } else {
        (p[0] - m[0]).abs() <= 0.5 && (p[1] - m[1]).abs() <= 0.5
    };
    inside.then_some((r, c))
}

/// Where a grid is on the screen: the units' origin, and the points per unit.
struct Place {
    min: [f64; 2],
    origin: egui::Pos2,
    scale: f32,
}

impl Place {
    fn screen(&self, p: [f64; 2]) -> egui::Pos2 {
        self.origin + Vec2::new((p[0] - self.min[0]) as f32 * self.scale, (p[1] - self.min[1]) as f32 * self.scale)
    }

    fn units(&self, s: egui::Pos2) -> [f64; 2] {
        let d = s - self.origin;
        [f64::from(d.x / self.scale) + self.min[0], f64::from(d.y / self.scale) + self.min[1]]
    }
}

/// The id a SOM view keeps its pick under in the context, for the table drawn with `table`.
pub fn pick_id(table: egui::Id) -> egui::Id {
    table.with("chart").with("som pick")
}

/// The node or group picked in the SOM view of the table drawn with `table`.
pub fn picked(ctx: &egui::Context, table: egui::Id) -> Option<SomPick> {
    ctx.data(|d| d.get_temp::<Option<SomPick>>(pick_id(table))).flatten()
}

/// Pick a node or group in the SOM view of the table drawn with `table` (none: let go).
pub fn set_picked(ctx: &egui::Context, table: egui::Id, pick: Option<SomPick>) {
    ctx.data_mut(|d| d.insert_temp(pick_id(table), pick));
}

/// Where the nodes of the SOM view of the table drawn with `table` were drawn, by node (from 1), at the last frame.
pub fn som_screen(ctx: &egui::Context, table: egui::Id, node: usize) -> Option<egui::Pos2> {
    let v: Vec<(usize, egui::Pos2)> = ctx.data(|d| d.get_temp(pick_id(table).with("screen")))?;
    v.iter().find(|(k, _)| *k == node).map(|(_, p)| *p)
}

/// The colour of a value in `lo` to `hi` on the viridis ramp (grey for none).
fn ramp(v: f64, lo: f64, hi: f64) -> Color32 {
    if !v.is_finite() {
        return Color32::from_gray(90);
    }
    let t = if hi > lo { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.5 };
    crate::palette::seen(Ramp::Viridis.colour(t as f32))
}

/// The least and greatest finite of some values.
fn span(values: impl Iterator<Item = f64>) -> (f64, f64) {
    values.filter(|v| v.is_finite()).fold((f64::MAX, f64::MIN), |(a, b), v| (a.min(v), b.max(v)))
}

/// A grid of the nodes of a SOM in a rectangle of `size`, each filled by `fill`, the picked ones outlined; returns the
/// node clicked (its place in `m.nodes`) and where each node was drawn (centres, by node).
fn draw_grid(
    ui: &mut Ui,
    m: &SomMap,
    size: Vec2,
    fill: &dyn Fn(&SomNode) -> Color32,
    pick: Option<SomPick>,
    tip: &dyn Fn(&SomNode) -> String,
) -> (Option<usize>, Vec<(usize, egui::Pos2)>) {
    let (resp, painter) = ui.allocate_painter(size, egui::Sense::click());
    let e = grid_extent(m.rows, m.cols, m.hex);
    let (w, h) = ((e[2] - e[0]) as f32, (e[3] - e[1]) as f32);
    let scale = (size.x / w).min(size.y / h);
    let origin = resp.rect.center() - Vec2::new(w, h) * scale / 2.0;
    let place = Place { min: [e[0], e[1]], origin, scale };
    let stroke = egui::Stroke::new(1.0, ui.visuals().panel_fill);
    let accent = egui::Stroke::new(2.5, crate::palette::seen(crate::theme::accent()));
    let mut centres = Vec::with_capacity(m.nodes.len());
    let mut outlined = Vec::new();
    for n in &m.nodes {
        let c = node_centre(n.row, n.col, m.rows, m.hex);
        let points: Vec<egui::Pos2> = node_corners(c, m.hex).into_iter().map(|p| place.screen(p)).collect();
        painter.add(egui::Shape::convex_polygon(points.clone(), fill(n), stroke));
        centres.push((n.node, place.screen(c)));
        if m.lit(pick, n) {
            outlined.push(points);
        }
    }
    for points in outlined {
        painter.add(egui::Shape::closed_line(points, accent));
    }
    let at = |s: egui::Pos2| {
        let p = place.units(s);
        node_at(p, m.rows, m.cols, m.hex).and_then(|(r, c)| m.nodes.iter().position(|n| n.row == r && n.col == c))
    };
    if let Some(k) = resp.hover_pos().and_then(at) {
        let _ = resp.clone().on_hover_text(tip(&m.nodes[k]));
    }
    let clicked = if resp.clicked() { resp.interact_pointer_pos().and_then(at) } else { None };
    (clicked, centres)
}

/// ml.som's codebook: the grid of nodes coloured by the U-matrix, the component planes, the records each holds or the
/// groups; a click on a node picks it (a click on a node of a grouped map in the groups view picks its group; the same
/// again lets go). The pick is kept in the context ([`picked`]).
fn som(ui: &mut Ui, t: &Table, view: SomView, id: egui::Id) {
    let Some(m) = som_map(t) else { return };
    let ctx = ui.ctx().clone();
    // (the id the table was drawn with is the chart's parent: see [`pick_id`])
    let key = id.with("som pick");
    let mut pick: Option<SomPick> = ctx.data(|d| d.get_temp::<Option<SomPick>>(key)).flatten();
    let by_group = view == SomView::Groups && m.grouped();
    let tip = |n: &SomNode| {
        let mut s = format!(
            "node {} (row {}, column {})\n{} records\nU-matrix {}",
            n.node,
            n.row,
            n.col,
            cell(n.count),
            cell(n.umatrix)
        );
        if let Some(g) = n.group {
            s.push_str(&format!("\ngroup {g}"));
        }
        for (name, w) in m.names.iter().zip(&n.weights) {
            s.push_str(&format!("\n{name} {}", cell(*w)));
        }
        s
    };
    let mut clicked: Option<usize> = None;
    let mut screen: Vec<(usize, egui::Pos2)> = Vec::new();
    let side = ui.available_width().clamp(240.0, 520.0);
    let main = Vec2::new(side, (side * 0.8).min(420.0));
    match view {
        SomView::Planes => {
            let across = ((ui.available_width() / 170.0).floor() as usize).clamp(1, m.names.len().max(1));
            let w = ui.available_width() / across as f32 - 10.0;
            ui.horizontal_wrapped(|ui| {
                for (j, name) in m.names.iter().enumerate() {
                    let (lo, hi) = span(m.nodes.iter().map(|n| n.weights[j]));
                    ui.allocate_ui(Vec2::new(w, w * 0.8 + 34.0), |ui| {
                        ui.vertical(|ui| {
                            ui.label(RichText::new(name).strong());
                            let fill = |n: &SomNode| ramp(n.weights[j], lo, hi);
                            let (c, centres) = draw_grid(ui, &m, Vec2::new(w, w * 0.8), &fill, pick, &tip);
                            clicked = clicked.or(c);
                            if j == 0 {
                                screen = centres;
                            }
                            ui.label(RichText::new(format!("{} to {}", cell(lo), cell(hi))).weak().size(11.0));
                        });
                    });
                }
            });
        }
        _ => {
            let (label, lo, hi) = match view {
                SomView::Counts => {
                    let (lo, hi) = span(m.nodes.iter().map(|n| n.count));
                    ("records per node", lo, hi)
                }
                SomView::Groups if by_group => ("group of each node", 1.0, 1.0),
                _ => {
                    let (lo, hi) = span(m.nodes.iter().map(|n| n.umatrix));
                    ("mean distance to the nodes next to it", lo, hi)
                }
            };
            let fill = |n: &SomNode| match (view, n.group) {
                (SomView::Counts, _) => ramp(n.count, lo, hi),
                (SomView::Groups, Some(g)) if by_group => crate::palette::seen(class_colour(g - 1)),
                _ => ramp(n.umatrix, lo, hi),
            };
            let (c, centres) = draw_grid(ui, &m, main, &fill, pick, &tip);
            clicked = c;
            screen = centres;
            if by_group {
                ui.label(RichText::new(label).weak().size(11.0));
            } else {
                ui.label(RichText::new(format!("{label}: {} to {}", cell(lo), cell(hi))).weak().size(11.0));
            }
        }
    }
    if let Some(k) = clicked {
        let n = &m.nodes[k];
        let new = match (by_group, n.group) {
            (true, Some(g)) => SomPick::Group(g),
            _ => SomPick::Node(n.node),
        };
        pick = if pick == Some(new) { None } else { Some(new) };
    }
    ctx.data_mut(|d| {
        d.insert_temp(key, pick);
        if !screen.is_empty() {
            d.insert_temp(key.with("screen"), screen);
        }
    });
    let note = match pick {
        Some(SomPick::Node(k)) => format!("node {k}: {} records", cell(m.records(pick))),
        Some(SomPick::Group(g)) => format!("group {g}: {} records", cell(m.records(pick))),
        None => "click a node to light its cells on the map (show the nodes or groups output)".to_string(),
    };
    ui.label(RichText::new(note).size(11.5));
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
            ..Default::default()
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
    fn box_numbers_and_loadings() {
        // matplotlib's: quartiles 2.25, 4.5, 6.75; whiskers within 1.5 boxes; 40 beyond them
        let (s, out) = box_stats(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 40.0, f64::NAN]).unwrap();
        assert_eq!(s, [1.0, 2.75, 4.5, 6.25, 7.0]);
        assert_eq!(out, [40.0]);
        assert!(box_stats(&[f64::NAN]).is_none());
        let t = Table {
            columns: vec!["component".into(), "explained".into(), "shdi".into(), "ed".into()],
            rows: vec![vec![1.0, 0.6, 0.7, 0.71], vec![2.0, 0.4, 0.71, -0.7]],
            ..Default::default()
        };
        assert!(Kind::of(&t).contains(&Kind::Loadings));
        assert_eq!(Chart::suggested(&t, Kind::Loadings).components, [0, 1]);
    }

    #[test]
    fn suggested_charts() {
        let t = Table {
            columns: vec!["class".into(), "date".into(), "ca".into(), "np".into()],
            rows: vec![vec![1.0, 2001.0, 3.0, 4.0]],
            ..Default::default()
        };
        let lines = Chart::suggested(&t, Kind::Lines);
        assert_eq!((lines.columns, lines.x, lines.by), (vec![2], Some(1), Some(0)));
        let points = Chart::suggested(&t, Kind::Points);
        assert_eq!((points.columns, points.x, points.by), (vec![3], Some(2), Some(0)));
        assert!(!Kind::of(&t).contains(&Kind::Matrix));
    }

    #[test]
    fn segment_scale_tables_have_the_esp_chart() {
        let esp = Table {
            columns: ["scale", "objects", "lv_b1", "roc_b1", "roc_mean", "suggested"].map(String::from).to_vec(),
            rows: vec![vec![1.0, 100.0, 0.5, f64::NAN, f64::NAN, 0.0]],
            ..Default::default()
        };
        assert!(is_esp(&esp) && Kind::of(&esp).contains(&Kind::Esp));
        let other = Table { columns: vec!["scale".into(), "x".into()], ..Default::default() };
        assert!(!is_esp(&other));
    }

    #[test]
    fn the_explained_variance_of_ml_pca_has_the_elbow() {
        let t = Table {
            columns: ["component", "variance", "ratio", "cumulative"].map(String::from).to_vec(),
            rows: vec![vec![1.0, 3.0, 0.6, 0.6], vec![2.0, 2.0, 0.4, 1.0]],
            ..Default::default()
        };
        assert!(is_elbow(&t) && Kind::of(&t).contains(&Kind::Elbow));
        assert_eq!(Kind::Elbow.name(), "elbow");
        assert!(!is_elbow(&Table { columns: vec!["component".into(), "variance".into()], ..Default::default() }));
    }

    #[test]
    fn the_choice_of_k_of_ml_kmeans_has_its_chart() {
        let t = Table {
            columns: ["k", "records", "inertia", "silhouette", "calinski_harabasz"].map(String::from).to_vec(),
            rows: vec![vec![2.0, 600.0, 900.0, 0.5, 300.0], vec![3.0, 600.0, 500.0, 0.6, 700.0]],
            ..Default::default()
        };
        assert!(is_choice(&t) && Kind::of(&t).contains(&Kind::Choice));
        assert_eq!(Kind::Choice.name(), "choice of k");
        assert!(!is_choice(&Table { columns: vec!["k".into(), "inertia".into()], ..Default::default() }));
    }

    #[test]
    fn the_scores_of_segment_optimize_have_their_chart() {
        let t = Table {
            columns: ["value", "objects", "variance", "moran", "objective"].map(String::from).to_vec(),
            rows: vec![
                vec![5.0, 90.0, 2.0, 0.1, 0.4],
                vec![10.0, 40.0, 3.0, -0.2, 1.7],
                vec![20.0, 9.0, 6.0, -0.3, 1.1],
            ],
            ..Default::default()
        };
        assert!(is_scores(&t) && Kind::of(&t).contains(&Kind::Scores));
        assert_eq!(Kind::Scores.name(), "scores");
        assert_eq!(Chart::first(&t).kind, Kind::Scores);
        assert_eq!(best_value(&t), Some(10.0));
        assert!(!is_scores(&Table { columns: vec!["value".into(), "objective".into()], ..Default::default() }));
        assert!(!is_scores(&Table { rows: vec![], ..t.clone() }));
        // (a NaN objective is never the best)
        let nan = Table { rows: vec![vec![1.0, 2.0, 1.0, 1.0, f64::NAN]], ..t };
        assert_eq!(best_value(&nan), None);
    }

    /// A codebook of `rows` x `cols` nodes as ml.som gives it (with a group column when `groups`).
    fn codebook(rows: usize, cols: usize, hex: bool, groups: bool) -> Table {
        let mut columns: Vec<String> = ["node", "row", "column", "count"].map(String::from).to_vec();
        if groups {
            columns.push("group".into());
        }
        columns.extend(["umatrix", "topology", "a", "b"].map(String::from));
        let mut t = Table { columns, ..Default::default() };
        for r in 0..rows {
            for c in 0..cols {
                let mut row = vec![(r * cols + c + 1) as f64, (r + 1) as f64, (c + 1) as f64, (r + c) as f64];
                if groups {
                    row.push((c * 2 / cols + 1) as f64);
                }
                row.extend([0.5, if hex { 2.0 } else { 1.0 }, r as f64, c as f64]);
                t.rows.push(row);
            }
        }
        t
    }

    #[test]
    fn the_codebook_of_ml_som_has_the_som_view() {
        let t = codebook(3, 4, false, true);
        assert!(is_som(&t) && Kind::of(&t).contains(&Kind::Som));
        assert_eq!(Kind::Som.name(), "SOM");
        assert_eq!(Chart::first(&t).kind, Kind::Som);
        let m = som_map(&t).unwrap();
        assert_eq!((m.rows, m.cols, m.hex, m.grouped()), (3, 4, false, true));
        assert_eq!(m.names, ["a", "b"]);
        assert_eq!(
            m.nodes[5],
            SomNode { node: 6, row: 2, col: 2, count: 2.0, group: Some(1), umatrix: 0.5, weights: vec![1.0, 1.0] }
        );
        assert_eq!(m.group_of(4), Some(2));
        assert_eq!(
            m.records(Some(SomPick::Group(2))),
            (0..3).flat_map(|r| [2, 3].map(|c| r + c)).sum::<usize>() as f64
        );
        assert!(som_map(&codebook(2, 2, true, false)).unwrap().hex);
        // not a codebook: a table of other columns, one without the topology (an older codebook), an empty one
        assert!(!is_som(&Table { columns: vec!["node".into(), "row".into()], ..Default::default() }));
        let mut old = codebook(2, 2, false, false);
        old.columns.retain(|c| c != "topology");
        assert!(!is_som(&old) && Chart::first(&old).kind == Kind::None);
        assert!(!is_som(&Table { rows: vec![], ..codebook(1, 1, false, false) }));
    }

    #[test]
    fn square_nodes_lie_in_rows_and_columns() {
        assert_eq!(node_centre(1, 1, 3, false), [0.0, 0.0]);
        assert_eq!(node_centre(3, 4, 3, false), [3.0, 2.0]);
        // a square's corners: the neighbours share them
        let a = node_corners(node_centre(1, 1, 3, false), false);
        let b = node_corners(node_centre(1, 2, 3, false), false);
        assert_eq!(a[1], b[0]);
        assert_eq!(a[2], b[3]);
        assert_eq!(grid_extent(3, 4, false), [-0.5, -0.5, 3.5, 2.5]);
        assert_eq!(node_at([2.2, 1.4], 3, 4, false), Some((2, 3)));
        assert_eq!(node_at([-0.6, 0.0], 3, 4, false), None);
        assert_eq!(node_at([3.4, 2.4], 3, 4, false), Some((3, 4)));
        assert_eq!(node_at([3.6, 2.4], 3, 4, false), None);
    }

    #[test]
    fn hexagonal_nodes_shift_every_other_row_as_minisom_does() {
        // 3 rows: counted from the last, the last and the first are shifted half a node
        let x = |r: usize| node_centre(r, 1, 3, true)[0];
        assert_eq!((x(1), x(2), x(3)), (-0.5, 0.0, -0.5));
        // 4 rows: the odd rows (the last is the 4th, shifted; the 2nd too)
        let x = |r: usize| node_centre(r, 1, 4, true)[0];
        assert_eq!((x(1), x(2), x(3), x(4)), (0.0, -0.5, 0.0, -0.5));
        assert!((node_centre(3, 1, 3, true)[1] - 2.0 * HEX_ROW).abs() < 1e-12);
        // every node has its neighbours at distance 1: the 2 along its row, and up to 4 in the rows around it
        let (rows, cols) = (5, 6);
        let d = |a: (usize, usize), b: (usize, usize)| {
            let (p, q) = (node_centre(a.0, a.1, rows, true), node_centre(b.0, b.1, rows, true));
            (p[0] - q[0]).hypot(p[1] - q[1])
        };
        for r in 2..rows {
            for c in 2..cols {
                let near = (r - 1..=r + 1)
                    .flat_map(|rr| (c - 1..=c + 1).map(move |cc| (rr, cc)))
                    .filter(|&p| p != (r, c) && (d((r, c), p) - 1.0).abs() < 1e-9)
                    .count();
                assert_eq!(near, 6, "node {r},{c}");
            }
        }
        // hexagons touch: the corners of a node's right side are the left side's of the one next to it
        let a = node_corners(node_centre(2, 3, rows, true), true);
        let b = node_corners(node_centre(2, 4, rows, true), true);
        let same = |p: [f64; 2], q: [f64; 2]| (p[0] - q[0]).abs() < 1e-9 && (p[1] - q[1]).abs() < 1e-9;
        let shared = a.iter().filter(|p| b.iter().any(|q| same(**p, *q))).count();
        assert_eq!(shared, 2);
        // and so do those of the next row's nodes (a shifted one under an unshifted one: two nodes touch it)
        let c = node_corners(node_centre(3, 3, rows, true), true);
        let c2 = node_corners(node_centre(3, 4, rows, true), true);
        let touching =
            [&c, &c2].iter().filter(|h| h.iter().filter(|p| a.iter().any(|q| same(**p, *q))).count() == 2).count();
        assert_eq!(touching, 2);
        // the extent holds every corner
        let e = grid_extent(rows, cols, true);
        for r in 1..=rows {
            for c in 1..=cols {
                for p in node_corners(node_centre(r, c, rows, true), true) {
                    assert!(p[0] >= e[0] - 1e-9 && p[0] <= e[2] + 1e-9 && p[1] >= e[1] - 1e-9 && p[1] <= e[3] + 1e-9);
                }
            }
        }
        // a point at a node's centre is its node, one in the gap beyond the map is none
        for (r, c) in [(1, 1), (2, 6), (5, 3)] {
            assert_eq!(node_at(node_centre(r, c, rows, true), rows, cols, true), Some((r, c)));
        }
        assert_eq!(node_at([-3.0, 0.0], rows, cols, true), None);
    }
}
