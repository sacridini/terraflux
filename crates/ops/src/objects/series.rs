//! `objects.series`: the time series of each object, from a stack with a layer per date (the mean of the object's cells
//! with a value on each date), and its seasons: the values by date, the phenology and the harmonics. The kernels are
//! those of [`crate::phenology`], which `raster.phenology` runs on cells.

use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::{Objects, objects_in};
use crate::phenology::{self, PHENOLOGY, Settings};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Series));
}

/// The mean of each object's cells with a value on each date (a row per date), NaN when fewer than `min_share` of the
/// object's cells have one. Each date is summed in cell order, so the numbers do not depend on the threads.
fn means(o: &Objects, data: &[Arc<Vec<f64>>], min_share: f64) -> Vec<Vec<f64>> {
    data.par_iter()
        .map(|layer| {
            let (mut sum, mut n) = (vec![0.0f64; o.count], vec![0u32; o.count]);
            for (i, &l) in o.labels.iter().enumerate() {
                if l > 0 && layer[i].is_finite() {
                    sum[l as usize - 1] += layer[i];
                    n[l as usize - 1] += 1;
                }
            }
            (0..o.count)
                .map(|k| {
                    let share = f64::from(n[k]) / f64::from(o.cells[k]).max(1.0);
                    if n[k] > 0 && share >= min_share { sum[k] / f64::from(n[k]) } else { f64::NAN }
                })
                .collect()
        })
        .collect()
}

/// `YYYYMMDD` of a day since 1970-01-01.
fn compact(day: f64) -> String {
    let (y, m, d) = crate::swat_weather::civil_from_days(day as i64);
    format!("{y:04}{m:02}{d:02}")
}

/// `objects.series`.
struct Series;

impl Operator for Series {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.series",
            "The time series of each object and its seasons, from a stack with a layer per date (an index or a band: \
             NDVI of every scene of a year). The dates are in the layers' names (YYYY-MM-DD or YYYYMMDD anywhere in \
             the name) or the setting `dates`, and increase. An object's value on a date is the mean of its cells \
             with a value (a cloud leaves out the cells it covers); with fewer than `min_share` of its cells with one \
             the whole date is missing. The groups, columns of the objects' table named `<variable>_...`: `values` \
             (a column a date, `<variable>_YYYYMMDD`), `phenology` (max, min, amplitude, peak_doy, peak_date as days \
             since 1970-01-01, integral in value times days, start_doy, start_date, end_doy, end_date, length, \
             rise_rate and fall_rate: the season by TIMESAT's amplitude method, Jonsson and Eklundh 2004: each side \
             of the peak from its own minimum, the start where the series, walked back from the peak, falls below \
             left minimum + season_threshold (max - left minimum), the end below right minimum + season_threshold \
             (max - right minimum) walking forward, by linear interpolation) and `harmonics` \
             (the least squares fit of n harmonics of a one-year period, Jakubauskas et al. 2001: hmean, trend with \
             trend, amp and phase of each harmonic, rmse; fewer than 2n + 2 values, 2n + 3 with a trend, give \
             NaN). The season is measured on the series as it is (the dates with a value joined by straight lines) \
             or smoothed: savgol (Savitzky and Golay 1964, window and order, on the dates with the holes filled; \
             it reads the dates as evenly spaced) or harmonic. `series` is the long table (id, date, doy, value, \
             smoothed) for a chart.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects"))
        .input(PortSpec::new("series", Type::Any, "a stack: a layer per date, on the objects' grid"))
        .output(PortSpec::new("objects", Type::Objects, "the objects with the columns of the groups"))
        .output(PortSpec::new("table", Type::Table, "the objects' table with the columns"))
        .output(PortSpec::new(
            "series",
            Type::Table,
            "the long table: id, date (days since 1970-01-01), doy, value, smoothed",
        ))
        .param(PortSpec::optional(
            "variable",
            Type::Text,
            "the prefix of the columns, `<variable>_max`... (default value; ndvi...; empty: none)",
        ))
        .param(PortSpec::optional(
            "dates",
            Type::Any,
            "the layers' dates, YYYY-MM-DD, a list in the layers' order (default: read in their names)",
        ))
        .param(PortSpec::optional("groups", Type::Any, "values, phenology (default) and/or harmonics"))
        .param(PortSpec::optional(
            "min_share",
            Type::Number,
            "the least share of an object's cells with a value for a date to count, 0 to 1 (default 0.5)",
        ))
        .param(
            PortSpec::optional(
                "smooth",
                Type::Text,
                "none (default), savgol or harmonic: the series the season is measured on",
            )
            .choices(["none", "savgol", "harmonic"]),
        )
        .param(PortSpec::optional("window", Type::Integer, "savgol: the window, odd, in dates (default 7)"))
        .param(PortSpec::optional(
            "order",
            Type::Integer,
            "savgol: the polynomial's order, below the window (default 2)",
        ))
        .param(PortSpec::optional("n", Type::Integer, "the harmonics, 1 to 6 (default 2)"))
        .param(PortSpec::optional("trend", Type::Bool, "harmonics: a straight trend too (default false)"))
        .param(PortSpec::optional(
            "season_threshold",
            Type::Number,
            "the fraction of the amplitude above the base where the season starts and ends, 0 to 1 (default 0.5)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let l = crate::raster_stats::layers(args, "series")?;
        if !l.grid.same_as(&o.grid) {
            return Err(bad("the series is not on the objects' grid".into()));
        }
        let t = phenology::dates_of(node, args, &l.names)?;
        if t.len() < 4 {
            return Err(bad(format!("{} dates: a series of 4 at least", t.len())));
        }
        let s: Settings = phenology::settings_of(node, args)?;
        let groups = phenology::groups_of(node, args, &["values", "phenology", "harmonics"], &["phenology"])?;
        let has = |g: &str| groups.iter().any(|x| x == g);
        let min_share: f64 = args.param("min_share", 0.5)?;
        if !(0.0..=1.0).contains(&min_share) {
            return Err(bad(format!("min_share {min_share}: 0 to 1")));
        }
        let var: String = args.param("variable", "value".to_string())?;
        // (no variable: the columns are named by the measure alone)
        let prefix = if var.is_empty() { String::new() } else { format!("{var}_") };
        phenology::uneven_warning(ctx, node, &t, &s);
        let by_date = means(o, &l.data, min_share);
        // per object: its series, and the measures of the groups
        let rows: Vec<(Vec<f64>, Vec<f64>)> = (0..o.count)
            .into_par_iter()
            .map(|k| {
                let y: Vec<f64> = by_date.iter().map(|d| d[k]).collect();
                let mut v: Vec<f64> = Vec::new();
                if has("values") {
                    v.extend(&y);
                }
                if has("phenology") {
                    v.extend(phenology::phenology(&t, &y, &s));
                }
                if has("harmonics") {
                    v.extend(phenology::harmonics(&t, &y, s.n, s.trend));
                }
                (v, y)
            })
            .collect();
        let mut names: Vec<String> = Vec::new();
        if has("values") {
            names.extend(t.iter().map(|&d| format!("{prefix}{}", compact(d))));
        }
        if has("phenology") {
            names.extend(PHENOLOGY.iter().map(|n| format!("{prefix}{n}")));
        }
        if has("harmonics") {
            names.extend(phenology::harmonic_names(s.n, s.trend).into_iter().map(|n| format!("{prefix}{n}")));
        }
        let mut table = o.table.clone();
        for (j, name) in names.iter().enumerate() {
            let at = match table.columns.iter().position(|c| c == name) {
                Some(p) => p,
                None => {
                    table.columns.push(name.clone());
                    table.rows.iter_mut().for_each(|r| r.push(f64::NAN));
                    table.columns.len() - 1
                }
            };
            for (r, (v, _)) in table.rows.iter_mut().zip(&rows) {
                r[at] = v[j];
            }
        }
        // the long table
        let mut long = Table::new(["id", "date", "doy", "value", "smoothed"].map(String::from).to_vec());
        for (k, (_, y)) in rows.iter().enumerate() {
            let sm = phenology::smoothed(&t, y, &s)
                .map(|(ts, vs)| {
                    // (on the dates of the smoothed series; the others have none)
                    let mut v = vec![f64::NAN; t.len()];
                    for (d, x) in ts.iter().zip(vs) {
                        if let Some(i) = t.iter().position(|e| e == d) {
                            v[i] = x;
                        }
                    }
                    v
                })
                .unwrap_or_else(|| vec![f64::NAN; t.len()]);
            for (i, &d) in t.iter().enumerate() {
                long.rows.push(vec![(k + 1) as f64, d, phenology::doy(d), y[i], sm[i]]);
            }
        }
        let marked = o.as_ref().clone().with_table(table.clone()).map_err(|e| bad(e.0))?;
        let mut out = outputs([("table", Value::Table(table)), ("series", Value::Table(long))]);
        out.insert("objects".into(), Value::Objects(Arc::new(marked)));
        Ok(out)
    }
}
