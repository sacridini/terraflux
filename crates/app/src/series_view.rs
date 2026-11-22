//! The profile of an object in the inspector, when the objects carry a time series (an `objects.series` node made
//! them): the values by date as points (a date with a cloud is a gap), the smoothed line, the band of the objects given
//! as samples of its class (the median and the interquartile range by date) and, with a second object selected
//! (ctrl+click), its profile too, to compare.

use std::sync::Arc;

use egui::{RichText, Ui};
use egui_plot::{Line, Plot, Points, Polygon};
use terraflux_ops::objects::Objects;
use terraflux_ops::{Table, Value};

use crate::app::Session;

/// The series of the objects: the dates and, per object (from 1), its values and smoothed values by date.
pub struct Profiles {
    pub dates: Vec<f64>,
    table: Arc<Table>,
}

/// What the last profile drawn showed, for the tests: the object, the second one, the dates with a value of the
/// object, the objects of the band (0: no band) and the class the band is of.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Drawn {
    pub id: u32,
    pub second: Option<u32>,
    pub values: usize,
    pub dates: usize,
    pub band: usize,
    pub class: Option<String>,
    pub smoothed: bool,
}

/// The last profile drawn in the context.
pub fn drawn(ctx: &egui::Context) -> Option<Drawn> {
    ctx.data(|d| d.get_temp::<Option<Drawn>>(egui::Id::new("series profile drawn"))).flatten()
}

/// Forget the last profile (no object is selected).
pub fn clear(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp::<Option<Drawn>>(egui::Id::new("series profile drawn"), None));
}

/// The long table of the `objects.series` node that made the objects `o` (its objects share their cells), as profiles.
pub fn profiles(s: &Session, o: &Objects) -> Option<Profiles> {
    for n in s.source.nodes.iter().filter(|n| n.op == "objects.series") {
        let Some(Value::Objects(x)) = s.value_of(&n.id, "objects") else { continue };
        if !Arc::ptr_eq(&x.labels, &o.labels) {
            continue;
        }
        let Some(Value::Table(t)) = s.value_of(&n.id, "series") else { continue };
        if o.count == 0 || t.rows.is_empty() || t.rows.len() % o.count != 0 {
            continue;
        }
        let n_dates = t.rows.len() / o.count;
        let dates = t.rows[..n_dates].iter().map(|r| r[1]).collect();
        return Some(Profiles { dates, table: Arc::new(t) });
    }
    None
}

impl Profiles {
    /// The value and the smoothed value of the object `id` (from 1) on each date.
    pub fn of(&self, id: u32) -> Vec<(f64, f64)> {
        let n = self.dates.len();
        let from = (id as usize - 1) * n;
        self.table.rows[from..from + n].iter().map(|r| (r[3], r[4])).collect()
    }
}

/// The value at a share `q` of the sorted values (linear between the two nearest, as numpy's percentile).
fn quantile(sorted: &[f64], q: f64) -> f64 {
    let at = q * (sorted.len() - 1) as f64;
    let (lo, hi) = (at.floor() as usize, at.ceil() as usize);
    sorted[lo] + (sorted[hi] - sorted[lo]) * (at - lo as f64)
}

/// The median and the first and third quartile by date of the series of the objects `ids`; none where a date has
/// fewer than two values.
pub fn band(p: &Profiles, ids: &[u32]) -> Vec<Option<[f64; 3]>> {
    let series: Vec<Vec<(f64, f64)>> = ids.iter().map(|&i| p.of(i)).collect();
    (0..p.dates.len())
        .map(|j| {
            let mut v: Vec<f64> = series.iter().map(|s| s[j].0).filter(|x| x.is_finite()).collect();
            if v.len() < 2 {
                return None;
            }
            v.sort_by(f64::total_cmp);
            Some([quantile(&v, 0.25), quantile(&v, 0.5), quantile(&v, 0.75)])
        })
        .collect()
}

/// The objects given as samples of a class, by name: the marked objects of an `objects.samples` node of these objects.
pub fn samples_of(s: &Session, o: &Objects, class: &str) -> Vec<u32> {
    for n in s.source.nodes.iter().filter(|n| n.op == "objects.samples") {
        let Some(Value::Objects(m)) = s.value_of(&n.id, "objects") else { continue };
        if !Arc::ptr_eq(&m.labels, &o.labels) {
            continue;
        }
        let Some(code) = m.classes.iter().position(|c| c.name == class) else { continue };
        return (0..m.count).filter(|&k| m.class_of[k] as usize == code + 1).map(|k| k as u32 + 1).collect();
    }
    Vec::new()
}

/// The date as `YYYY-MM` for an axis.
fn month(day: f64) -> String {
    let (y, m, _) = terraflux_ops::swat_weather::civil_from_days(day.floor() as i64);
    format!("{y}-{m:02}")
}

/// The profile of the object `id` (and `second`) in the inspector. Nothing when no `objects.series` made the objects.
pub fn profile(ui: &mut Ui, s: &Session, o: &Objects, id: u32, second: Option<u32>) {
    let Some(p) = profiles(s, o) else {
        clear(ui.ctx());
        return;
    };
    ui.add_space(4.0);
    ui.label(RichText::new("Profile").size(11.5));
    let (k, accent) = (id as usize - 1, crate::palette::seen(crate::theme::accent()));
    let other = crate::palette::seen(crate::palette::class_colour(2));
    let mine = p.of(id);
    let class = (o.class_of[k] > 0).then(|| o.classes[o.class_of[k] as usize - 1].name.clone());
    let samples = class.as_deref().map(|c| samples_of(s, o, c)).unwrap_or_default();
    let band_values = if samples.len() >= 2 { band(&p, &samples) } else { Vec::new() };
    let have_band = band_values.iter().any(Option::is_some);
    let points = |v: &[(f64, f64)], smoothed: bool| -> Vec<[f64; 2]> {
        p.dates
            .iter()
            .zip(v)
            .map(|(&d, &(x, sm))| [d, if smoothed { sm } else { x }])
            .filter(|q| q[1].is_finite())
            .collect()
    };
    let (own, own_smoothed) = (points(&mine, false), points(&mine, true));
    let ink = ui.visuals().text_color().gamma_multiply(0.6);
    Plot::new(egui::Id::new(("series profile", id, second)))
        .height(150.0)
        .allow_scroll(false)
        .x_axis_formatter(|m, _| month(m.value))
        .legend(egui_plot::Legend::default())
        .show(ui, |pl| {
            if have_band {
                // the interquartile range as a filled strip, the median a line
                let mut top: Vec<[f64; 2]> = Vec::new();
                let mut bottom: Vec<[f64; 2]> = Vec::new();
                let mut median: Vec<[f64; 2]> = Vec::new();
                for (d, b) in p.dates.iter().zip(&band_values) {
                    if let Some([lo, mid, hi]) = b {
                        top.push([*d, *hi]);
                        bottom.push([*d, *lo]);
                        median.push([*d, *mid]);
                    }
                }
                bottom.reverse();
                top.extend(bottom);
                pl.polygon(
                    Polygon::new("samples of the class (quartiles)", top)
                        .fill_color(ink.gamma_multiply(0.35))
                        .stroke(egui::Stroke::NONE),
                );
                pl.line(Line::new("samples of the class (median)", median).color(ink).width(1.2));
            }
            if let Some(b) = second {
                let theirs = p.of(b);
                pl.line(Line::new(format!("object {b}"), points(&theirs, true)).color(other).width(1.5));
                pl.points(Points::new(format!("object {b}"), points(&theirs, false)).color(other).radius(2.5));
            }
            pl.line(Line::new(format!("object {id}"), own_smoothed.clone()).color(accent).width(1.8));
            pl.points(Points::new(format!("object {id}"), own.clone()).color(accent).radius(3.0));
        });
    let text = match (&class, have_band) {
        (Some(c), true) => format!(
            "{} of {} dates have a value (the others are clouds); the grey band is the {} samples of {c}",
            own.len(),
            p.dates.len(),
            samples.len()
        ),
        _ => format!("{} of {} dates have a value (the others are clouds)", own.len(), p.dates.len()),
    };
    ui.label(RichText::new(text).weak().size(11.0));
    if second.is_none() {
        ui.label(RichText::new("ctrl+click another object to compare").weak().size(11.0));
    }
    let drawn = Drawn {
        id,
        second,
        values: own.len(),
        dates: p.dates.len(),
        band: if have_band { samples.len() } else { 0 },
        class,
        smoothed: own_smoothed.len() != own.len() || own_smoothed != own,
    };
    ui.ctx().data_mut(|d| d.insert_temp::<Option<Drawn>>(egui::Id::new("series profile drawn"), Some(drawn)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_band_is_the_median_and_the_quartiles_by_date() {
        let mut t = Table::new(["id", "date", "doy", "value", "smoothed"].map(String::from).to_vec());
        // four objects, three dates: the values 1 2 3 4 on the first date, one with a cloud on the second, 5 on the third
        for (id, v) in
            [(1.0, [1.0, 1.0, 5.0]), (2.0, [2.0, 2.0, 5.0]), (3.0, [3.0, 3.0, 5.0]), (4.0, [4.0, f64::NAN, 5.0])]
        {
            for (j, x) in v.iter().enumerate() {
                t.rows.push(vec![id, 10.0 * j as f64, 1.0, *x, *x]);
            }
        }
        let p = Profiles { dates: vec![0.0, 10.0, 20.0], table: Arc::new(t) };
        let b = band(&p, &[1, 2, 3, 4]);
        assert_eq!(b[0], Some([1.75, 2.5, 3.25]));
        assert_eq!(b[1], Some([1.5, 2.0, 2.5]));
        assert_eq!(b[2], Some([5.0, 5.0, 5.0]));
        // a date with one value has no band
        assert_eq!(band(&p, &[4, 1])[1], None);
        assert_eq!(p.of(4)[1].0.is_nan(), true);
    }
}
