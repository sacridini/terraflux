//! The profile of a map along a line (the line of the Measure tool): its values at points a fraction of a cell apart
//! (at most [`MOST`] of them), by their distance along the line, drawn as a chart under the legend; of a map of
//! classes, the classes crossed as a band and the length of the line in each.

use std::hash::{Hash, Hasher};

use egui::{Color32, Pos2, Rect, RichText, Sense, Ui, Vec2};

use crate::map_view::Source;
use crate::measure::Frame;
use crate::palette::Coloring;

/// The most points a profile is made of.
pub const MOST: usize = 2000;

/// A map's values along a line.
#[derive(Debug, Clone, PartialEq)]
pub struct Profile {
    /// The distance of each point from the line's start (metres; the grid's units with no coordinate system), its
    /// cell (column, row, fractional) and the map's value there (NaN: none, or outside the map).
    pub dist: Vec<f64>,
    pub cells: Vec<[f64; 2]>,
    pub values: Vec<f64>,
    /// The distance of each vertex of the line.
    pub vertices: Vec<f64>,
    pub frame: Frame,
}

/// The key of a line, to keep its profile by.
pub fn key(points: &[[f64; 2]]) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    points.iter().flatten().for_each(|v| v.to_bits().hash(&mut h));
    h.finish()
}

/// The points along a line of cells, about half a cell apart (fewer on a long line): their cells and distances.
fn along(grid: &terraflux_raster::Grid, line: &[[f64; 2]]) -> (Vec<[f64; 2]>, Vec<f64>, Vec<f64>) {
    let seg_cells: Vec<f64> = line.windows(2).map(|w| (w[1][0] - w[0][0]).hypot(w[1][1] - w[0][1])).collect();
    let seg_m: Vec<f64> = line.windows(2).map(|w| crate::measure::cells_apart(grid, w[0], w[1])).collect();
    let total: f64 = seg_cells.iter().sum();
    let mut vertices = vec![0.0];
    for m in &seg_m {
        vertices.push(vertices.last().unwrap() + m);
    }
    if line.len() < 2 || total <= 0.0 {
        return (line.first().map(|p| vec![*p]).unwrap_or_default(), vec![0.0; line.len().min(1)], vertices);
    }
    let n = ((total * 2.0).ceil() as usize + 1).clamp(2, MOST);
    let spacing = total / (n - 1) as f64;
    let (mut cells, mut dist) = (Vec::with_capacity(n), Vec::with_capacity(n));
    for k in 0..n {
        // the segment the k-th point is on, and how far along it
        let mut at = (k as f64 * spacing).min(total);
        let mut s = 0;
        while s + 1 < seg_cells.len() && at > seg_cells[s] {
            at -= seg_cells[s];
            s += 1;
        }
        let t = if seg_cells[s] > 0.0 { (at / seg_cells[s]).clamp(0.0, 1.0) } else { 0.0 };
        let (a, b) = (line[s], line[s + 1]);
        cells.push([a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]);
        dist.push(vertices[s] + seg_m[s] * t);
    }
    (cells, dist, vertices)
}

/// The profile of a map along a line of its cells (column, row).
pub fn of(source: &Source, line: &[[f64; 2]]) -> Profile {
    let grid = source.grid();
    let (cells, dist, vertices) = along(grid, line);
    let (w, h) = source.size();
    let inside = |c: &[f64; 2]| c[0] >= 0.0 && c[1] >= 0.0 && c[0] < w as f64 && c[1] < h as f64;
    let values: Vec<f64> = match source {
        Source::Map(m) => {
            cells.iter().map(|c| if inside(c) { m.data[c[1] as usize * w + c[0] as usize] } else { f64::NAN }).collect()
        }
        Source::Rasters(r, l) => {
            // the line's box read at once when it is not too large, else cell by cell
            let ins: Vec<&[f64; 2]> = cells.iter().filter(|c| inside(c)).collect();
            let (x0, y0) =
                ins.iter().fold((usize::MAX, usize::MAX), |a, c| (a.0.min(c[0] as usize), a.1.min(c[1] as usize)));
            let (x1, y1) = ins.iter().fold((0, 0), |a, c| (a.0.max(c[0] as usize + 1), a.1.max(c[1] as usize + 1)));
            let boxed = (!ins.is_empty() && (x1 - x0) * (y1 - y0) <= crate::histogram::SAMPLE)
                .then(|| {
                    let win = terraflux_raster::tiff::Window {
                        x0: x0 as i64,
                        y0: y0 as i64,
                        width: x1 - x0,
                        height: y1 - y0,
                    };
                    r.window(*l, win).ok()
                })
                .flatten();
            cells
                .iter()
                .map(|c| {
                    if !inside(c) {
                        return f64::NAN;
                    }
                    let (x, y) = (c[0] as usize, c[1] as usize);
                    match &boxed {
                        Some(v) => v.get((y - y0) * (x1 - x0) + (x - x0)).copied().unwrap_or(f64::NAN),
                        None => crate::builder::series_at(std::slice::from_ref(source), y, x)[0],
                    }
                })
                .collect()
        }
        Source::Blank(_) => vec![f64::NAN; cells.len()],
    };
    Profile { dist, cells, values, vertices, frame: Frame::of(grid) }
}

/// A distance as text in the profile's units.
fn length(p: &Profile, d: f64) -> String {
    match p.frame {
        Frame::Plain => format!("{} units", crate::map_view::cell_text(d)),
        _ => crate::measure::length_text(d),
    }
}

/// The length of the line in each class crossed (of a map of classes), the longest first.
pub fn class_lengths(p: &Profile) -> Vec<(f64, f64)> {
    let mut by: Vec<(f64, f64)> = Vec::new();
    for k in 1..p.values.len() {
        let v = p.values[k - 1];
        if !v.is_finite() {
            continue;
        }
        let d = p.dist[k] - p.dist[k - 1];
        match by.iter_mut().find(|(c, _)| *c == v) {
            Some(e) => e.1 += d,
            None => by.push((v, d)),
        }
    }
    by.sort_by(|a, b| b.1.total_cmp(&a.1));
    by
}

/// The profile drawn: of values a line by the distance, of classes a band in their colours with the length in each.
/// Returns the cell under the pointer when it is over the chart (to mark it on the map).
pub fn show(
    ui: &mut Ui,
    p: &Profile,
    coloring: &Coloring,
    name: &dyn Fn(f64) -> String,
    id: egui::Id,
) -> Option<[f64; 2]> {
    let total = p.dist.last().copied().unwrap_or(0.0);
    ui.label(RichText::new(format!("{} along the line, {} points", length(p, total), p.dist.len())).weak().size(11.0));
    let mut hovered = None;
    match coloring {
        Coloring::Classes(_) => {
            let (r, resp) =
                ui.allocate_exact_size(Vec2::new(ui.available_width().clamp(120.0, 260.0), 22.0), Sense::hover());
            ui.painter().rect_filled(r, 2.0, ui.visuals().extreme_bg_color);
            if total > 0.0 {
                for k in 1..p.values.len() {
                    let v = p.values[k - 1];
                    if !v.is_finite() {
                        continue;
                    }
                    let x0 = r.left() + r.width() * (p.dist[k - 1] / total) as f32;
                    let x1 = r.left() + r.width() * (p.dist[k] / total) as f32;
                    let bar = Rect::from_min_max(Pos2::new(x0, r.top()), Pos2::new(x1.max(x0 + 0.5), r.bottom()));
                    ui.painter().rect_filled(bar, 0.0, coloring.colour(v));
                }
                for d in &p.vertices[1..p.vertices.len().saturating_sub(1)] {
                    let x = r.left() + r.width() * (*d / total) as f32;
                    ui.painter().line_segment(
                        [Pos2::new(x, r.top()), Pos2::new(x, r.bottom())],
                        ui.visuals().widgets.noninteractive.fg_stroke,
                    );
                }
                if let Some(pos) = resp.hover_pos() {
                    let d = (f64::from((pos.x - r.left()) / r.width()) * total).clamp(0.0, total);
                    let k = p.dist.partition_point(|x| *x < d).min(p.dist.len() - 1);
                    hovered = Some(p.cells[k]);
                    let v = p.values[k];
                    let said = if v.is_finite() { name(v) } else { "no value".into() };
                    resp.on_hover_text(format!("{}: {said}", length(p, p.dist[k])));
                }
            }
            for (v, d) in class_lengths(p).into_iter().take(12) {
                ui.horizontal(|ui| {
                    let (s, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                    ui.painter().rect_filled(s, 2.0, coloring.colour(v));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let share = if total > 0.0 { 100.0 * d / total } else { 0.0 };
                        ui.label(RichText::new(format!("{} ({share:.0}%)", length(p, d))).monospace().size(11.0));
                        ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                            ui.add(egui::Label::new(name(v)).truncate());
                        });
                    });
                });
            }
        }
        Coloring::Continuous { .. } => {
            use egui_plot::{Line, Plot, VLine};
            let pts: Vec<[f64; 2]> =
                p.dist.iter().zip(&p.values).filter(|(_, v)| v.is_finite()).map(|(d, v)| [*d, *v]).collect();
            let weak = ui.visuals().weak_text_color();
            let colour: Color32 = crate::theme::accent();
            crate::theme::plot(Plot::new(id))
                .height(140.0)
                .allow_scroll(false)
                .allow_drag(false)
                .allow_zoom(false)
                .show_background(false)
                .x_axis_formatter({
                    let frame = p.frame;
                    move |m, _| match frame {
                        Frame::Plain => crate::map_view::cell_text(m.value),
                        _ if total >= 2000.0 => format!("{} km", crate::map_view::cell_text(m.value / 1000.0)),
                        _ => format!("{} m", crate::map_view::cell_text(m.value)),
                    }
                })
                .show(ui, |pl| {
                    pl.line(Line::new("values", pts).color(colour).width(1.6));
                    for d in &p.vertices[1..p.vertices.len().saturating_sub(1)] {
                        pl.vline(VLine::new("", *d).color(weak).width(0.5));
                    }
                    if pl.response().hovered()
                        && let Some(c) = pl.pointer_coordinate()
                        && !p.dist.is_empty()
                    {
                        let k = p.dist.partition_point(|x| *x < c.x).min(p.dist.len() - 1);
                        hovered = Some(p.cells[k]);
                        pl.vline(VLine::new("here", p.dist[k]).color(colour.gamma_multiply(0.6)));
                    }
                });
            let vals: Vec<f64> = p.values.iter().copied().filter(|v| v.is_finite()).collect();
            if vals.is_empty() {
                ui.label(RichText::new("no value along the line").weak());
            } else {
                let (lo, hi) = vals.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |a, v| (a.0.min(*v), a.1.max(*v)));
                let mean = vals.iter().sum::<f64>() / vals.len() as f64;
                let c = crate::map_view::cell_text;
                ui.label(RichText::new(format!("min {}, max {}, mean {}", c(lo), c(hi), c(mean))).size(11.0));
            }
        }
    }
    if ui.small_button("Copy").on_hover_text("the distance, the column, the row and the value of every point").clicked()
    {
        let unit = match p.frame {
            Frame::Plain => "distance",
            _ => "distance_m",
        };
        let mut t = format!("{unit}\tcolumn\trow\tvalue\n");
        for ((d, c), v) in p.dist.iter().zip(&p.cells).zip(&p.values) {
            t.push_str(&format!(
                "{d:.3}\t{:.2}\t{:.2}\t{}\n",
                c[0],
                c[1],
                if v.is_finite() { v.to_string() } else { String::new() }
            ));
        }
        ui.ctx().copy_text(t);
    }
    hovered
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use terraflux_ops::Map;
    use terraflux_raster::Grid;

    #[test]
    fn values_along_a_line() {
        // 10 x 1 cells of 30 m in UTM, the values the column numbers; the line across the row's middle
        let mut g = Grid::new(10, 1);
        g.transform = [0.0, 30.0, 0.0, 30.0, 0.0, -30.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32633]);
        let src = Source::Map(Map::new(Arc::new(g), (0..10).map(f64::from).collect()));
        let p = of(&src, &[[0.5, 0.5], [9.5, 0.5]]);
        assert_eq!(p.dist.len(), 19);
        assert_eq!(p.vertices, vec![0.0, 270.0]);
        assert_eq!((p.dist[0], *p.dist.last().unwrap()), (0.0, 270.0));
        assert_eq!((p.values[0], p.values[1], p.values[2], *p.values.last().unwrap()), (0.0, 1.0, 1.0, 9.0));
        // a line leaving the map: no value outside it
        let p = of(&src, &[[8.5, 0.5], [12.5, 0.5]]);
        assert_eq!(p.values[0], 8.0);
        assert!(p.values.last().unwrap().is_nan());
    }

    #[test]
    fn the_length_in_each_class() {
        let g = Grid::new(4, 1);
        let src = Source::Map(Map::new(Arc::new(g), vec![1.0, 1.0, 1.0, 2.0]));
        let p = of(&src, &[[0.0, 0.5], [4.0, 0.5]]);
        let l = class_lengths(&p);
        assert_eq!(l[0].0, 1.0);
        assert!((l[0].1 - 3.0).abs() < 0.3 && (l[1].1 - 1.0).abs() < 0.3, "{l:?}");
        assert_eq!(key(&[[1.0, 2.0]]), key(&[[1.0, 2.0]]));
    }
}
