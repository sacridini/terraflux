//! The histogram of a map's values, for the Viewer: bars between the colouring's range, each in the colour of its
//! value, with the extremes and the mean; worked out in a thread from a sample when the map is large.

use std::sync::mpsc::{Receiver, Sender, channel};

use egui::{Pos2, Rect, Sense, Ui, Vec2};

use crate::map_view::{Source, SourceKey, cell_text};
use crate::palette::Coloring;

/// The bars of a histogram.
pub const BINS: usize = 60;
/// The most cells a histogram is worked out from; of a larger map, a sample of about this many.
pub const SAMPLE: usize = 4_000_000;

/// The values of a map in bars.
#[derive(Debug, Clone, PartialEq)]
pub struct Histogram {
    /// The range the bars span (values beyond it are in the first and last bar).
    pub lo: f64,
    pub hi: f64,
    pub bins: Vec<usize>,
    /// The smallest and the largest value, and their mean (NaN when there is no value).
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    /// The cells with a value and the cells without.
    pub valid: usize,
    pub empty: usize,
    /// Whether the cells are a sample of the map (every few rows and columns), not all of it.
    pub sampled: bool,
}

impl Histogram {
    /// The histogram of some values between `lo` and `hi` (a range with no width is the values' own; a
    /// constant map is all in the first bar). NaN and infinite values are cells with no value.
    pub fn of(values: impl Iterator<Item = f64>, lo: f64, hi: f64, sampled: bool) -> Histogram {
        let values: Vec<f64> = values.collect();
        let (mut min, mut max, mut sum) = (f64::INFINITY, f64::NEG_INFINITY, 0.0);
        let mut valid = 0usize;
        for &v in values.iter().filter(|v| v.is_finite()) {
            min = min.min(v);
            max = max.max(v);
            sum += v;
            valid += 1;
        }
        let (lo, hi) = if lo.is_finite() && hi.is_finite() && hi > lo { (lo, hi) } else { (min, max) };
        let width = hi - lo;
        let mut bins = vec![0usize; BINS];
        for &v in values.iter().filter(|v| v.is_finite()) {
            let at =
                if width > 0.0 { ((v - lo) / width * BINS as f64).floor().clamp(0.0, (BINS - 1) as f64) } else { 0.0 };
            bins[at as usize] += 1;
        }
        let mean = if valid > 0 { sum / valid as f64 } else { f64::NAN };
        let (min, max) = if valid > 0 { (min, max) } else { (f64::NAN, f64::NAN) };
        Histogram { lo, hi, bins, min, max, mean, valid, empty: values.len() - valid, sampled }
    }

    /// The histogram of a source between `lo` and `hi`: of all its cells, or of a sample of about `SAMPLE` of
    /// them when it has more (every few rows and columns; a raster on disk read at a coarser step).
    pub fn of_source(source: &Source, lo: f64, hi: f64) -> Histogram {
        let (w, h) = source.size();
        let step = source_step(source, w, h);
        let sampled = step > 1;
        match source {
            Source::Map(m) if !sampled => Histogram::of(m.data.iter().copied(), lo, hi, false),
            Source::Map(m) => {
                let rows = (0..h).step_by(step).flat_map(|y| (0..w).step_by(step).map(move |x| y * w + x));
                Histogram::of(rows.map(|i| m.data[i]), lo, hi, true)
            }
            Source::Rasters(r, l) => {
                let win = terraflux_raster::tiff::Window { x0: 0, y0: 0, width: w, height: h };
                let values = r.sample(*l, win, step).unwrap_or_default();
                Histogram::of(values.into_iter().map(f64::from), lo, hi, sampled)
            }
            Source::Blank(_) => Histogram::of(std::iter::empty(), lo, hi, false),
            Source::Rgb(r) => Histogram::of_source(&r.bands[0], lo, hi),
            Source::Shade(s) => Histogram::of_source(&s.base, lo, hi),
        }
    }

    /// The range of a bar.
    pub fn range(&self, i: usize) -> (f64, f64) {
        let w = (self.hi - self.lo) / BINS as f64;
        (self.lo + w * i as f64, self.lo + w * (i + 1) as f64)
    }
}

/// How many rows and columns apart the cells of a sample are, for a map `w` x `h`: 1 up to `SAMPLE` cells.
pub fn sample_step(w: usize, h: usize) -> usize {
    step_for(w, h, SAMPLE)
}

/// The most cells the statistics of a raster on the web are worked out from: a sample read from its overview, a
/// fraction of the bytes of a file of [`SAMPLE`] cells.
pub const REMOTE_SAMPLE: usize = 1_000_000;

/// [`sample_step`] for a source: a raster on the web is sampled more coarsely ([`REMOTE_SAMPLE`]), so that its
/// numbers come from an overview, not from every cell of a file far away.
pub fn source_step(source: &Source, w: usize, h: usize) -> usize {
    step_for(w, h, if source.is_remote() { REMOTE_SAMPLE } else { SAMPLE })
}

/// How many rows and columns apart the cells of a sample of at most `limit` cells are (about `limit` of them).
fn step_for(w: usize, h: usize, limit: usize) -> usize {
    let n = w.saturating_mul(h);
    if n <= limit {
        return 1;
    }
    let mut step = ((n as f64 / limit as f64).sqrt().ceil() as usize).max(2);
    while (w.div_ceil(step)) * (h.div_ceil(step)) > limit {
        step += 1;
    }
    step
}

type Worked = ((SourceKey, u64, u64), Histogram);
type Kept = ((SourceKey, u64, u64), Source, Option<Histogram>, u64);

/// The histograms of the maps shown, each worked out once in a thread.
#[derive(Default)]
pub struct Histograms {
    kept: Vec<Kept>,
    channel: Option<(Sender<Worked>, Receiver<Worked>)>,
    clock: u64,
}

impl Histograms {
    /// The histogram of a source between `lo` and `hi`, or none while it is worked out.
    pub fn get(&mut self, ctx: &egui::Context, source: &Source, lo: f64, hi: f64) -> Option<Histogram> {
        let (tx, rx) = self.channel.get_or_insert_with(channel);
        while let Ok((key, h)) = rx.try_recv() {
            if let Some(e) = self.kept.iter_mut().find(|e| e.0 == key) {
                e.2 = Some(h);
            }
        }
        self.clock += 1;
        let key = (source.key(), lo.to_bits(), hi.to_bits());
        if let Some(e) = self.kept.iter_mut().find(|e| e.0 == key) {
            e.3 = self.clock;
            return e.2.clone();
        }
        let (tx, src, c) = (tx.clone(), source.clone(), ctx.clone());
        std::thread::spawn(move || {
            let _ = tx.send((key, Histogram::of_source(&src, lo, hi)));
            c.request_repaint();
        });
        self.kept.push((key, source.clone(), None, self.clock));
        // the sources are held while their histograms are: keep a few
        while self.kept.len() > 24 {
            let Some(i) = (0..self.kept.len()).filter(|&i| self.kept[i].2.is_some()).min_by_key(|&i| self.kept[i].3)
            else {
                break;
            };
            self.kept.swap_remove(i);
        }
        None
    }
}

/// The histogram drawn: a bar per range in the colour of its value, the numbers under it; a bar hovered tells
/// its range and cells.
pub fn show(ui: &mut Ui, h: &Histogram, coloring: &Coloring) {
    bars(ui, h, coloring);
    numbers_of(ui, h.lo, h.hi);
    numbers(ui, h);
}

/// What was asked of the range of the colours under a histogram.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Asked {
    /// The colours from the least to the most of these two.
    Range(f64, f64),
    /// The colours as at first (over the 2nd to the 98th percentiles).
    Again,
}

/// [`show`] with the range of a ramp's colours to set: its least and most drawn as two lines over the bars,
/// dragged along them, typed under them, or set to the values' extremes or back to the percentiles; what was
/// asked, and whether a line is being dragged (the bars' span then stays).
pub fn show_ranged(ui: &mut Ui, h: &Histogram, coloring: &Coloring, id: egui::Id) -> (Option<Asked>, bool) {
    let Coloring::Continuous { min, max, log, .. } = *coloring else {
        show(ui, h, coloring);
        return (None, false);
    };
    let r = bars(ui, h, coloring);
    let span = (h.hi - h.lo).max(f64::MIN_POSITIVE);
    let x_of = |v: f64| r.left() + r.width() * ((v - h.lo) / span).clamp(0.0, 1.0) as f32;
    let v_of = |x: f32| h.lo + span * f64::from(((x - r.left()) / r.width()).clamp(0.0, 1.0));
    // (the values out of the range shaded: they take the ramp's ends)
    let shade = ui.visuals().extreme_bg_color.gamma_multiply(0.6);
    let (a, b) = (x_of(min), x_of(max));
    ui.painter().rect_filled(Rect::from_min_max(r.min, Pos2::new(a, r.bottom())), 0.0, shade);
    ui.painter().rect_filled(Rect::from_min_max(Pos2::new(b, r.top()), r.max), 0.0, shade);
    let (mut lo, mut hi) = (min, max);
    let mut asked = None;
    let mut dragging = false;
    let accent = ui.visuals().selection.stroke.color;
    for (k, x) in [(0, a), (1, b)] {
        let handle = ui
            .interact(
                Rect::from_center_size(Pos2::new(x, r.center().y), Vec2::new(10.0, r.height())),
                id.with(k),
                Sense::drag(),
            )
            .on_hover_cursor(egui::CursorIcon::ResizeHorizontal)
            .on_hover_text(if k == 0 {
                "the least of the colours: drag it"
            } else {
                "the most of the colours: drag it"
            });
        let strong = handle.hovered() || handle.dragged();
        let stroke = egui::Stroke::new(if strong { 2.5 } else { 1.5 }, accent);
        ui.painter().line_segment([Pos2::new(x, r.top()), Pos2::new(x, r.bottom())], stroke);
        // (a tab at the top to take it by)
        let tab = Rect::from_center_size(Pos2::new(x, r.top() + 4.0), Vec2::new(8.0, 8.0));
        ui.painter().rect_filled(tab, 2.0, accent);
        if handle.dragged() {
            dragging = true;
            if let Some(p) = handle.interact_pointer_pos() {
                let v = v_of(p.x);
                if k == 0 {
                    lo = v.min(hi - span * 1e-6);
                } else {
                    hi = v.max(lo + span * 1e-6);
                }
                asked = Some(Asked::Range(lo, hi));
            }
        }
    }
    numbers_of(ui, h.lo, h.hi);
    // the range typed
    let speed = ((hi - lo).abs() / 200.0).max(1e-9);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("colours from").size(11.0));
        let a = ui
            .add(egui::DragValue::new(&mut lo).speed(speed).max_decimals(4))
            .on_hover_text("the least of the colours");
        ui.label(egui::RichText::new("to").size(11.0));
        let b =
            ui.add(egui::DragValue::new(&mut hi).speed(speed).max_decimals(4)).on_hover_text("the most of the colours");
        if (a.changed() || b.changed()) && hi > lo {
            asked = Some(Asked::Range(lo, hi));
        }
    });
    ui.horizontal(|ui| {
        if ui
            .small_button("2% to 98%")
            .on_hover_text("the colours over the 2nd to the 98th percentiles of the values, as at first")
            .clicked()
        {
            asked = Some(Asked::Again);
        }
        if h.valid > 0
            && h.max > h.min
            && ui.small_button("least to most").on_hover_text("the colours over all the values").clicked()
        {
            // (on a scale of logarithms, from the least above 0)
            let least = if log && h.min <= 0.0 { h.max * 1e-6 } else { h.min };
            asked = Some(Asked::Range(least, h.max));
        }
    });
    if log
        && let Some(Asked::Range(a, b)) = asked
        && a <= 0.0
    {
        asked = Some(Asked::Range(b.abs() * 1e-6, b));
    }
    numbers(ui, h);
    (asked, dragging)
}

/// The bars of a histogram drawn, a bar hovered telling its range and cells; where they are.
fn bars(ui: &mut Ui, h: &Histogram, coloring: &Coloring) -> Rect {
    let (r, resp) = ui.allocate_exact_size(Vec2::new(ui.available_width().clamp(120.0, 260.0), 90.0), Sense::hover());
    ui.painter().rect_filled(r, 2.0, ui.visuals().extreme_bg_color);
    let top = h.bins.iter().copied().max().unwrap_or(0).max(1);
    let w = r.width() / BINS as f32;
    let at = resp.hover_pos().map(|p| (((p.x - r.left()) / w) as usize).min(BINS - 1));
    for (i, &n) in h.bins.iter().enumerate() {
        if n == 0 {
            continue;
        }
        let (a, b) = h.range(i);
        let height = (r.height() - 2.0) * n as f32 / top as f32;
        let x = r.left() + w * i as f32;
        let bar = Rect::from_min_max(Pos2::new(x, r.bottom() - height.max(1.0)), Pos2::new(x + w, r.bottom()));
        ui.painter().rect_filled(bar, 0.0, coloring.colour((a + b) / 2.0));
        if at == Some(i) {
            ui.painter().rect_stroke(bar, 0.0, ui.visuals().selection.stroke, egui::StrokeKind::Outside);
        }
    }
    if let Some(i) = at {
        let (a, b) = h.range(i);
        let (a, b) = (cell_text(a), cell_text(b));
        resp.on_hover_text(format!("{a} to {b}: {} cells", h.bins[i]));
    }
    r
}

/// The values at the ends of the bars, under them.
fn numbers_of(ui: &mut Ui, lo: f64, hi: f64) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(cell_text(lo)).weak().size(11.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(cell_text(hi)).weak().size(11.0));
        });
    });
}

/// The values' extremes and mean, and the cells.
fn numbers(ui: &mut Ui, h: &Histogram) {
    if h.valid == 0 {
        ui.label(egui::RichText::new("no cell has a value").weak());
        return;
    }
    ui.label(format!("min {}, max {}, mean {}", cell_text(h.min), cell_text(h.max), cell_text(h.mean)));
    ui.label(egui::RichText::new(format!("{} cells with a value, {} with none", h.valid, h.empty)).weak());
    if h.sampled {
        ui.label(egui::RichText::new(format!("from a sample of {} cells", h.valid + h.empty)).weak());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bins_and_counts() {
        let v: Vec<f64> = (0..600).map(|i| i as f64 / 10.0).collect();
        let h = Histogram::of(v.iter().copied(), 0.0, 60.0, false);
        assert_eq!(h.bins.len(), BINS);
        assert!(h.bins.iter().all(|&n| n == 10));
        assert_eq!((h.min, h.max, h.valid, h.empty), (0.0, 59.9, 600, 0));
        assert!((h.mean - 29.95).abs() < 1e-9);
        assert_eq!(h.range(1), (1.0, 2.0));
        // the values beyond the range in the first and last bars
        let h = Histogram::of([-5.0, 0.5, 99.0, 60.0].into_iter(), 0.0, 60.0, false);
        assert_eq!((h.bins[0], h.bins[BINS - 1]), (2, 2));
    }

    #[test]
    fn cells_with_no_value() {
        let h = Histogram::of([1.0, f64::NAN, 3.0, f64::INFINITY].into_iter(), 1.0, 3.0, false);
        assert_eq!((h.valid, h.empty), (2, 2));
        assert_eq!((h.min, h.max, h.mean), (1.0, 3.0, 2.0));
        assert_eq!(h.bins.iter().sum::<usize>(), 2);
        let h = Histogram::of([f64::NAN, f64::NAN].into_iter(), 0.0, 1.0, false);
        assert_eq!((h.valid, h.empty), (0, 2));
        assert!(h.mean.is_nan() && h.min.is_nan());
    }

    #[test]
    fn a_constant_map() {
        let h = Histogram::of(std::iter::repeat_n(4.0, 25), 4.0, 4.0, false);
        assert_eq!((h.min, h.max, h.mean), (4.0, 4.0, 4.0));
        assert_eq!(h.bins[0], 25);
        assert_eq!(h.bins.iter().sum::<usize>(), 25);
    }

    #[test]
    fn a_large_map_is_sampled() {
        assert_eq!(sample_step(1000, 1000), 1);
        let step = sample_step(10_000, 10_000);
        assert!(step > 1);
        assert!(10_000usize.div_ceil(step).pow(2) <= SAMPLE);
        // a map of 3000 x 3000 (9 million cells): a sample, its cells every `step`
        let (w, h) = (3000usize, 3000usize);
        let grid = terraflux_raster::Grid::new(w, h);
        let data: Vec<f64> = (0..w * h).map(|i| (i % w) as f64).collect();
        let m = terraflux_ops::Map { grid: std::sync::Arc::new(grid), data: std::sync::Arc::new(data) };
        let hist = Histogram::of_source(&Source::Map(m), 0.0, 3000.0);
        assert!(hist.sampled);
        assert!(hist.valid + hist.empty <= SAMPLE);
        assert!(hist.valid > SAMPLE / 2);
        assert!((hist.mean - 1500.0).abs() < 50.0, "{}", hist.mean);
    }
}
