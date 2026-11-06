//! The charts of a sensitivity analysis: in the batch pane (egui_plot) and written as SVG and PNG files (laid out
//! by hand with `figure::Drawing`). One at a time: a tornado. Morris: μ* against σ. Sobol: S1 and ST with their
//! intervals. And whether the indices have settled (`Indices::convergence`).

use std::path::Path;

use egui::{Align2, Color32, Ui};
use egui_plot::{Bar, BarChart, Legend, Line, Plot, PlotPoint, Points, Text, VLine};
use terraflux_dist::sensitivity::{Indices, Method, TOLERANCE};

use crate::figure::{Drawing, nice_step, text_width};
use crate::theme;

/// What Garcia Sanchez et al. (2014) read from σ/μ*, told on the Morris chart.
const MORRIS_HINT: &str = "The lines are σ/μ* = 0.1 and 1 (Garcia Sanchez et al. 2014). Below 0.1 a factor acts \
    linearly, from 0.1 to 0.5 monotonically, above 1 non-linearly or through interactions with other factors.";

/// A piece of a bar: from `a` to `b`, of the legend's series `series`, with its interval when it has one.
struct Seg {
    series: usize,
    a: f64,
    b: f64,
    whisker: Option<(f64, f64)>,
}

/// A factor's row: bands of segments (a band is a bar; Sobol has two, S1 and ST).
struct Row {
    name: String,
    bands: Vec<Vec<Seg>>,
}

/// The bars of a chart: the factors from the most important, what the colours are, and the axis.
struct Bars {
    title: String,
    rows: Vec<Row>,
    legend: Vec<(Color32, String)>,
    /// The output at the base (a line through the bars).
    base: Option<f64>,
    x_label: String,
    /// The axis starts at zero (the indices).
    zero: bool,
    /// The intervals are drawn.
    interval: bool,
}

fn col(ind: &Indices, name: &str) -> Option<usize> {
    ind.columns.iter().position(|c| c == name)
}

/// OAT's and Sobol's bars, none for Morris or an analysis without its columns.
fn bars_of(ind: &Indices) -> Option<Bars> {
    let order = ind.order();
    match ind.method {
        Method::Oat => {
            let (lo, hi, base) = (col(ind, "low")?, col(ind, "high")?, ind.base?);
            let rows = order
                .iter()
                .map(|&i| Row {
                    name: ind.factors[i].clone(),
                    bands: vec![vec![
                        Seg { series: 0, a: base, b: ind.rows[i][lo], whisker: None },
                        Seg { series: 1, a: base, b: ind.rows[i][hi], whisker: None },
                    ]],
                })
                .collect();
            Some(Bars {
                title: "One at a time: the output with each factor at its minimum and at its maximum".into(),
                rows,
                legend: vec![
                    (crate::palette::class_colour(0), "factor at its minimum".into()),
                    (crate::palette::class_colour(1), "factor at its maximum".into()),
                ],
                base: Some(base),
                x_label: "the output measured".into(),
                zero: false,
                interval: false,
            })
        }
        Method::Sobol => {
            let (s1, s1c, st, stc) = (col(ind, "s1")?, col(ind, "s1_conf")?, col(ind, "st")?, col(ind, "st_conf")?);
            let seg = |series: usize, v: f64, c: f64| Seg {
                series,
                a: 0.0,
                b: v,
                whisker: (c.is_finite() && v.is_finite()).then_some((v - c, v + c)),
            };
            let rows = order
                .iter()
                .map(|&i| {
                    let r = &ind.rows[i];
                    Row {
                        name: ind.factors[i].clone(),
                        bands: vec![vec![seg(0, r[s1], r[s1c])], vec![seg(1, r[st], r[stc])]],
                    }
                })
                .collect();
            Some(Bars {
                title: "Sobol: the share of the output's variance of each factor".into(),
                rows,
                legend: vec![
                    (crate::palette::class_colour(0), "S1, alone".into()),
                    (crate::palette::class_colour(1), "ST, with its interactions".into()),
                ],
                base: None,
                x_label: "index (share of the variance)".into(),
                zero: true,
                interval: true,
            })
        }
        Method::Morris => None,
    }
}

/// Morris's factors as (name, μ*, σ), the most important first.
fn dots_of(ind: &Indices) -> Option<Vec<(String, f64, f64)>> {
    if ind.method != Method::Morris {
        return None;
    }
    let (m, s) = (col(ind, "mu_star")?, col(ind, "sigma")?);
    Some(ind.order().iter().map(|&i| (ind.factors[i].clone(), ind.rows[i][m], ind.rows[i][s])).collect())
}

/// What the convergence check says, and whether it is a warning: none for OAT.
pub fn verdict(ind: &Indices) -> Option<(String, bool)> {
    let (unit, more) = match ind.method {
        Method::Oat => return None,
        Method::Morris => ("trajectories", "more trajectories (r)"),
        Method::Sobol => ("base points", "more base points (N)"),
    };
    let Some(c) = &ind.convergence else {
        let least = if ind.method == Method::Morris { 4 } else { 8 };
        return Some((format!("Convergence not checked: it needs {least} {unit} at least."), false));
    };
    Some(if c.stable {
        (
            format!(
                "Stable: the indices with half the runs are within {:.0} % of those with all of them.",
                TOLERANCE * 100.0
            ),
            false,
        )
    } else {
        (format!("Not stable yet: the indices still move when the runs double. Try {more}."), true)
    })
}

fn range_of(b: &Bars) -> (f64, f64) {
    let mut v: Vec<f64> = b.base.into_iter().collect();
    for s in b.rows.iter().flat_map(|r| &r.bands).flatten() {
        v.extend([s.a, s.b]);
        v.extend(s.whisker.into_iter().flat_map(|(a, b)| [a, b]));
    }
    let v: Vec<f64> = v.into_iter().filter(|x| x.is_finite()).collect();
    let lo = v.iter().cloned().fold(f64::INFINITY, f64::min);
    let hi = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if !lo.is_finite() {
        return (0.0, 1.0);
    }
    let (lo, hi) = if b.zero { (lo.min(0.0), hi.max(0.0)) } else { (lo, hi) };
    let pad = if hi > lo { (hi - lo) * 0.05 } else { 1.0 };
    (if b.zero && lo == 0.0 { 0.0 } else { lo - pad }, hi + pad)
}

// -- In the pane ----------------------------------------------------------------------------------------------------

/// The charts of an analysis, and the convergence check under them.
pub fn show(ui: &mut Ui, ind: &Indices) {
    if let Some(b) = bars_of(ind) {
        bars_ui(ui, &b);
    } else if let Some(d) = dots_of(ind) {
        dots_ui(ui, &d);
    }
    if let Some((text, warn)) = verdict(ind) {
        if warn {
            let c = ui.visuals().warn_fg_color;
            ui.label(egui::RichText::new(text).color(c));
        } else {
            ui.label(text);
        }
    }
    if let Some(c) = &ind.convergence {
        let unit = if ind.method == Method::Sobol { "base points" } else { "trajectories" };
        let colours: Vec<Color32> = (0..ind.factors.len()).map(crate::palette::class_colour).collect();
        theme::plot(Plot::new(("sensitivity convergence", ind.method.name())))
            .height(130.0)
            .legend(Legend::default())
            .allow_scroll(false)
            .x_axis_label(unit)
            .y_axis_label(c.by.as_str())
            .show(ui, |p| {
                for (i, f) in ind.factors.iter().enumerate() {
                    let pts: Vec<[f64; 2]> = c.n.iter().zip(&c.values[i]).map(|(n, v)| [*n as f64, *v]).collect();
                    p.line(Line::new(f.as_str(), pts.clone()).color(colours[i]));
                    p.points(Points::new(f.as_str(), pts).color(colours[i]).radius(3.0));
                }
            });
    }
}

fn bars_ui(ui: &mut Ui, b: &Bars) {
    let n = b.rows.len();
    let bands = b.rows.first().map_or(1, |r| r.bands.len()).max(1);
    let height = (n as f32 * 18.0 * bands as f32 + 70.0).clamp(150.0, 460.0);
    let names: Vec<String> = b.rows.iter().rev().map(|r| r.name.clone()).collect();
    ui.label(egui::RichText::new(&b.title).weak());
    let ink = ui.visuals().text_color();
    theme::plot(Plot::new(("sensitivity chart", b.legend.len())))
        .height(height)
        .legend(Legend::default())
        .allow_scroll(false)
        .x_axis_label(b.x_label.as_str())
        .y_axis_formatter(move |m, _| {
            let k = m.value.round();
            if (m.value - k).abs() < 1e-6 && k >= 0.0 {
                names.get(k as usize).cloned().unwrap_or_default()
            } else {
                String::new()
            }
        })
        .show(ui, |p| {
            let (lo, hi) = range_of(b);
            p.set_plot_bounds_x(lo..=hi);
            p.set_plot_bounds_y(-0.6..=(n as f64 - 0.4));
            let mut series: Vec<Vec<Bar>> = vec![Vec::new(); b.legend.len()];
            for (r, row) in b.rows.iter().enumerate() {
                let y0 = (n - 1 - r) as f64;
                let k = row.bands.len() as f64;
                for (j, band) in row.bands.iter().enumerate() {
                    let y = y0 + (k - 1.0) / 2.0 * 0.4 - j as f64 * 0.4;
                    for s in band {
                        series[s.series].push(
                            Bar::new(y, (s.b - s.a).abs())
                                .base_offset(s.a.min(s.b))
                                .width(0.8 / k * 0.9)
                                .name(format!("{}: {:.4}", row.name, s.b))
                                .horizontal(),
                        );
                        if let Some((a, c)) = s.whisker {
                            p.line(Line::new("95 % interval", vec![[a, y], [c, y]]).color(ink).width(1.2));
                            for e in [a, c] {
                                p.line(Line::new("95 % interval", vec![[e, y - 0.06], [e, y + 0.06]]).color(ink));
                            }
                        }
                    }
                }
            }
            for (bars, (colour, name)) in series.into_iter().zip(&b.legend) {
                p.bar_chart(BarChart::new(name.as_str(), bars).color(*colour).horizontal());
            }
            if let Some(base) = b.base {
                p.vline(VLine::new(format!("output at the base: {base:.4}"), base).color(ink));
            }
        });
}

/// The axes' ends of Morris's chart: a margin past the largest μ* and σ, and σ room to show the line σ/μ* = 0.1.
fn extents(dots: &[(String, f64, f64)]) -> (f64, f64) {
    let top = |f: fn(&(String, f64, f64)) -> f64| dots.iter().map(f).filter(|v| v.is_finite()).fold(0.0, f64::max);
    let x = if top(|d| d.1) > 0.0 { top(|d| d.1) * 1.15 } else { 1.0 };
    (x, (top(|d| d.2) * 1.15).max(0.25 * x))
}

fn dots_ui(ui: &mut Ui, dots: &[(String, f64, f64)]) {
    let (xmax, ymax) = extents(dots);
    ui.label(egui::RichText::new("Morris: mean absolute effect (μ*) against its spread (σ)").weak());
    let ink = ui.visuals().text_color();
    let r = theme::plot(Plot::new("sensitivity morris"))
        .height(280.0)
        .allow_scroll(false)
        .include_x(0.0)
        .include_y(0.0)
        .include_x(xmax)
        .include_y(ymax)
        .x_axis_label("μ*")
        .y_axis_label("σ")
        .show(ui, |p| {
            let faint = ink.gamma_multiply(0.35);
            for slope in [1.0, 0.1] {
                let x = xmax.min(ymax / slope);
                let name = format!("σ/μ* = {slope}");
                p.line(Line::new(name.as_str(), vec![[0.0, 0.0], [x, x * slope]]).color(faint).allow_hover(false));
                p.text(
                    Text::new(name.as_str(), PlotPoint::new(x, x * slope), name.as_str())
                        .color(faint)
                        .anchor(Align2::RIGHT_BOTTOM),
                );
            }
            for (i, (name, mu, sigma)) in dots.iter().enumerate() {
                if !mu.is_finite() || !sigma.is_finite() {
                    continue;
                }
                let c = crate::palette::class_colour(i);
                p.points(Points::new(name.as_str(), vec![[*mu, *sigma]]).color(c).radius(5.0));
                p.text(
                    Text::new(name.as_str(), PlotPoint::new(*mu, *sigma), format!(" {name}"))
                        .color(ink)
                        .anchor(Align2::LEFT_BOTTOM),
                );
            }
        });
    r.response.on_hover_text(MORRIS_HINT);
}

// -- As files -------------------------------------------------------------------------------------------------------

const W: f32 = 560.0;
const M: f32 = 16.0;
const INK: Color32 = Color32::from_rgb(34, 34, 34);
const GRID: Color32 = Color32::from_rgb(225, 225, 225);
const WEAK: Color32 = Color32::from_rgb(110, 110, 110);
const WARN: Color32 = Color32::from_rgb(190, 90, 0);

/// Round ticks over `lo..hi` and their decimals.
fn ticks(lo: f64, hi: f64) -> (Vec<f64>, usize) {
    let step = nice_step(hi - lo, 5.0);
    let decimals = if step >= 1.0 { 0 } else { (-step.log10()).ceil() as usize };
    let first = (lo / step).ceil() as i64;
    let last = (hi / step + 1e-9).floor() as i64;
    ((first..=last).take(60).map(|k| k as f64 * step).collect(), decimals)
}

/// A text cut at its start (with an ellipsis) to fit `max` points.
fn fit(text: &str, size: f32, max: f32) -> String {
    if text_width(size, text) <= max {
        return text.into();
    }
    let chars: Vec<char> = text.chars().collect();
    for cut in 1..chars.len() {
        let t: String = std::iter::once('…').chain(chars[cut..].iter().copied()).collect();
        if text_width(size, &t) <= max {
            return t;
        }
    }
    "…".into()
}

/// The legend in a row at `y`: a swatch and a text each.
fn legend_row(d: &mut Drawing, y: f32, items: &[(Color32, String)]) {
    let mut x = M;
    for (c, t) in items {
        d.rect((x, y - 8.0, 10.0, 10.0), Some(*c));
        d.label((x + 14.0, y), 9.5, t, false, INK, false);
        x += 14.0 + text_width(9.5, t) + 14.0;
    }
}

fn verdict_line(d: &mut Drawing, y: f32, ind: &Indices) {
    if let Some((text, warn)) = verdict(ind) {
        d.label((M, y), 9.5, &text, false, if warn { WARN } else { WEAK }, false);
    }
}

/// The chart as a drawing: none when the analysis has nothing to draw.
pub fn drawing(ind: &Indices) -> Option<Drawing> {
    if ind.factors.is_empty() {
        return None;
    }
    if let Some(b) = bars_of(ind) {
        return bars_drawing(ind, &b);
    }
    dots_drawing(ind, &dots_of(ind)?)
}

fn bars_drawing(ind: &Indices, b: &Bars) -> Option<Drawing> {
    let n = b.rows.len();
    let bands = b.rows.first()?.bands.len().max(1);
    let label_w = b.rows.iter().map(|r| text_width(10.0, &r.name)).fold(0.0, f32::max).min(220.0);
    let (left, right) = (M + label_w + 10.0, W - M);
    let row_h = 13.0 * bands as f32 + 9.0;
    let top = 58.0;
    let plot_h = n as f32 * row_h;
    let h = top + plot_h + 40.0 + if verdict(ind).is_some() { 18.0 } else { 0.0 } + M / 2.0;
    let mut d = Drawing::blank(W, h);
    d.label((M, 18.0), 12.0, &b.title, false, INK, false);
    let mut legend = b.legend.clone();
    if b.interval {
        legend.push((INK, "95 % interval".into()));
    }
    legend_row(&mut d, 38.0, &legend);
    let (lo, hi) = range_of(b);
    let px = |v: f64| left + ((v - lo) / (hi - lo)) as f32 * (right - left);
    let (ts, decimals) = ticks(lo, hi);
    for t in ts {
        let x = px(t);
        d.polyline(vec![(x, top), (x, top + plot_h)], GRID, 0.6);
        let s = format!("{t:.decimals$}");
        d.label((x, top + plot_h + 13.0), 9.0, &s, true, WEAK, false);
    }
    d.label(((left + right) / 2.0, top + plot_h + 28.0), 10.0, &b.x_label, true, INK, false);
    for (r, row) in b.rows.iter().enumerate() {
        let yc = top + (r as f32 + 0.5) * row_h;
        let name = fit(&row.name, 10.0, label_w);
        d.label((left - 8.0 - text_width(10.0, &name), yc + 3.5), 10.0, &name, false, INK, false);
        for (j, band) in row.bands.iter().enumerate() {
            let y = yc + (j as f32 - (bands as f32 - 1.0) / 2.0) * 14.0;
            for s in band {
                let (a, c) = (px(s.a.min(s.b)), px(s.a.max(s.b)));
                d.rect((a, y - 6.0, (c - a).max(0.8), 12.0), Some(b.legend[s.series].0));
                if let Some((wa, wb)) = s.whisker {
                    let (wa, wb) = (px(wa), px(wb));
                    d.polyline(vec![(wa, y), (wb, y)], INK, 0.9);
                    for e in [wa, wb] {
                        d.polyline(vec![(e, y - 3.0), (e, y + 3.0)], INK, 0.9);
                    }
                }
            }
        }
    }
    if let Some(base) = b.base {
        let x = px(base);
        d.polyline(vec![(x, top - 4.0), (x, top + plot_h)], INK, 1.2);
        d.label((x, top - 7.0), 9.0, &format!("base {base:.4}"), true, INK, false);
    }
    d.polyline(vec![(left, top), (left, top + plot_h)], INK, 0.8);
    verdict_line(&mut d, top + plot_h + 46.0, ind);
    Some(d)
}

fn dots_drawing(ind: &Indices, dots: &[(String, f64, f64)]) -> Option<Drawing> {
    let (left, right, top) = (M + 40.0, W - M, 50.0);
    let plot_h = 290.0;
    let h = top + plot_h + 40.0 + if verdict(ind).is_some() { 34.0 } else { 16.0 };
    let mut d = Drawing::blank(W, h);
    d.label((M, 18.0), 12.0, "Morris: mean absolute effect (μ*) against its spread (σ)", false, INK, false);
    let (xmax, ymax) = extents(dots);
    let px = |v: f64| left + (v / xmax) as f32 * (right - left);
    let py = |v: f64| top + plot_h - (v / ymax) as f32 * plot_h;
    let (xs, xd) = ticks(0.0, xmax);
    for t in xs {
        d.polyline(vec![(px(t), top), (px(t), top + plot_h)], GRID, 0.6);
        d.label((px(t), top + plot_h + 13.0), 9.0, &format!("{t:.xd$}"), true, WEAK, false);
    }
    let (ys, yd) = ticks(0.0, ymax);
    for t in ys {
        d.polyline(vec![(left, py(t)), (right, py(t))], GRID, 0.6);
        let s = format!("{t:.yd$}");
        d.label((left - 5.0 - text_width(9.0, &s), py(t) + 3.0), 9.0, &s, false, WEAK, false);
    }
    d.label(((left + right) / 2.0, top + plot_h + 28.0), 10.0, "μ*", true, INK, false);
    d.label((M + 8.0, top + plot_h / 2.0), 10.0, "σ", true, INK, true);
    for slope in [1.0, 0.1] {
        let x = xmax.min(ymax / slope);
        d.polyline(vec![(px(0.0), py(0.0)), (px(x), py(x * slope))], Color32::from_rgb(190, 190, 190), 0.9);
        let name = format!("σ/μ* = {slope}");
        d.label((px(x) - 4.0 - text_width(8.5, &name), py(x * slope) - 3.0), 8.5, &name, false, WEAK, false);
    }
    d.polyline(vec![(left, top), (left, top + plot_h), (right, top + plot_h)], INK, 0.8);
    for (i, (name, mu, sigma)) in dots.iter().enumerate() {
        if !mu.is_finite() || !sigma.is_finite() {
            continue;
        }
        let (x, y) = (px(*mu), py(*sigma));
        d.dot(x, y, 4.0, crate::palette::class_colour(i));
        let w = text_width(9.0, name);
        let at = if x + 7.0 + w > right { x - 7.0 - w } else { x + 7.0 };
        d.label((at, y - 5.0), 9.0, name, false, INK, false);
    }
    let y = top + plot_h + 46.0;
    d.label(
        (M, y),
        8.5,
        "Below σ/μ* = 0.1 a factor acts linearly, above 1 non-linearly or through interactions.",
        false,
        WEAK,
        false,
    );
    verdict_line(&mut d, y + 15.0, ind);
    Some(d)
}

/// Write `sensitivity.svg` and `sensitivity.png` in `dir`; nothing for an analysis without a chart.
pub fn write(dir: &Path, ind: &Indices) -> Result<(), String> {
    let Some(d) = drawing(ind) else { return Ok(()) };
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    save_drawing(&d, &dir.join("sensitivity.svg"))?;
    save_drawing(&d, &dir.join("sensitivity.png"))
}

/// The chart in a file of the kind its extension says: SVG, else PNG.
pub fn save(path: &Path, ind: &Indices) -> Result<(), String> {
    save_drawing(&drawing(ind).ok_or("nothing to draw")?, path)
}

fn save_drawing(d: &Drawing, path: &Path) -> Result<(), String> {
    let svg = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("svg"));
    if svg {
        std::fs::write(path, d.svg(d.w * 25.4 / 72.0)).map_err(|e| e.to_string())
    } else {
        let png = d.pixels(200)?.encode_png().map_err(|e| e.to_string())?;
        std::fs::write(path, png).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use terraflux_dist::sensitivity::{Design, analyze, rows};

    use super::*;

    fn keys() -> Vec<String> {
        vec!["a.x".into(), "b.y".into(), "c.z".into()]
    }

    fn indices(method: Method, n: usize) -> Indices {
        let bounds = [(0.0, 1.0); 3];
        let d = Design { n, ..Design::new(method) };
        let r = rows(method, &bounds, &d).unwrap();
        let y: Vec<f64> = r.iter().map(|r| 4.0 * r[0] + 2.0 * r[1] * r[2] + 0.1 * r[2]).collect();
        analyze(method, &keys(), &r, &y, &d, 20).unwrap()
    }

    fn count(s: &str, what: &str) -> usize {
        s.matches(what).count()
    }

    fn dir(name: &str) -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("terraflux-sens-chart-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        d
    }

    fn inked(path: &Path) -> bool {
        let pm = tiny_skia::Pixmap::load_png(path).unwrap();
        pm.pixels().iter().any(|p| (p.red(), p.green(), p.blue(), p.alpha()) != (255, 255, 255, 255))
    }

    #[test]
    fn the_chart_of_each_method_has_a_shape_per_factor() {
        // OAT: two segments a factor (and the background); Sobol: two bars and two whiskers; Morris: a dot
        let oat = dir("oat");
        write(&oat, &indices(Method::Oat, 1)).unwrap();
        let svg = std::fs::read_to_string(oat.join("sensitivity.svg")).unwrap();
        assert!(count(&svg, "<rect") >= 1 + 3 * 2 + 2, "{svg}");
        assert!(svg.contains(">a.x<") && svg.contains(">c.z<") && svg.contains("base "));
        assert!(inked(&oat.join("sensitivity.png")));

        let sobol = dir("sobol");
        write(&sobol, &indices(Method::Sobol, 16)).unwrap();
        let svg = std::fs::read_to_string(sobol.join("sensitivity.svg")).unwrap();
        assert!(count(&svg, "<rect") >= 1 + 3 * 2 + 3, "{svg}");
        assert!(svg.contains("S1, alone") && svg.contains("95 % interval"));
        assert!(inked(&sobol.join("sensitivity.png")));

        let morris = dir("morris");
        write(&morris, &indices(Method::Morris, 8)).unwrap();
        let svg = std::fs::read_to_string(morris.join("sensitivity.svg")).unwrap();
        assert_eq!(count(&svg, "<path"), 3, "a dot a factor: {svg}");
        assert!(svg.contains(">b.y<") && svg.contains("σ/μ* = 0.1") && svg.contains("table"), "{svg}");
        assert!(inked(&morris.join("sensitivity.png")));
        for d in [oat, sobol, morris] {
            let _ = std::fs::remove_dir_all(d);
        }
    }

    #[test]
    fn the_verdict_follows_the_convergence() {
        let mut ok = indices(Method::Morris, 8);
        ok.convergence.as_mut().unwrap().stable = true;
        let (text, warn) = verdict(&ok).unwrap();
        assert!(!warn && text.starts_with("Stable") && text.contains("5 %"), "{text}");
        let mut bad = ok.clone();
        bad.convergence.as_mut().unwrap().stable = false;
        let (text, warn) = verdict(&bad).unwrap();
        assert!(warn && text.contains("Not stable yet") && text.contains("trajectories"), "{text}");
        let few = indices(Method::Morris, 3);
        assert!(few.convergence.is_none() && verdict(&few).unwrap().0.contains("not checked"));
        assert!(verdict(&indices(Method::Oat, 1)).is_none());
    }

    #[test]
    fn a_long_name_is_cut_at_its_start() {
        let long = "a_node_with_a_very_long_name.and_its_setting_too";
        let t = fit(long, 10.0, 80.0);
        assert!(t.starts_with('…') && t.ends_with("too") && text_width(10.0, &t) <= 80.0, "{t}");
        assert_eq!(fit("a.x", 10.0, 80.0), "a.x");
    }
}
