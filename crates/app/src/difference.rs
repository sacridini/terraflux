//! The difference of two maps, the third view of the viewer's comparison: maps of values the other one minus this
//! one, cell by cell, in a ramp centred on 0; maps of classes the transitions, a class each ("forest -> pasture"),
//! the cells that did not change apart. Made on the grid of the map shown (every few cells when it is very
//! large), the other's cells taken where they are by the coordinates.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use egui::Color32;
use terraflux_ops::Map;
use terraflux_raster::tiff::Window;

use crate::map_view::Source;
use crate::methods::Lang;
use crate::palette::{Coloring, Ramp, class_colour};

/// The most cells a difference is made of: a map with more is compared every few cells.
pub const MOST: usize = 1 << 24;

/// The most transitions named one by one; the others together, as "other changes".
pub const TRANSITIONS: usize = 24;

/// The value of the cells whose class did not change, in a map of transitions.
pub const UNCHANGED: f64 = 0.0;

/// The names of a map's classes, by their values (a value not here by its number).
pub type Names = BTreeMap<i64, String>;

/// A difference made: the map, its colours, the cells of each class (transitions), their names, every how many
/// cells of the map shown it was made, and what it says in a sentence or two.
pub struct Made {
    pub source: Source,
    pub coloring: Coloring,
    pub counts: Vec<(f64, usize)>,
    pub names: Vec<(f64, String)>,
    pub step: usize,
    pub summary: String,
}

impl Made {
    /// The name of a class of the map (a transition), else its value.
    pub fn name(&self, v: f64) -> String {
        self.names
            .iter()
            .find(|(k, _)| *k == v)
            .map_or_else(|| terraflux_ops::value::class_label(v), |(_, n)| n.clone())
    }
}

/// The step at which a map of `w` by `h` cells is read so that it has no more than [`MOST`] cells.
fn step_for(w: usize, h: usize) -> usize {
    let mut s = 1;
    while w.div_ceil(s) * h.div_ceil(s) > MOST {
        s += 1;
    }
    s
}

/// A map's cells at every `step`-th row and column (NaN where there is no value), and how many columns.
fn read(src: &Source, step: usize) -> Result<(Arc<Vec<f64>>, usize), String> {
    let (w, h) = src.size();
    let (sw, sh) = (w.div_ceil(step), h.div_ceil(step));
    match src {
        Source::Map(m) if step == 1 => Ok((m.data.clone(), w)),
        Source::Map(m) => {
            let mut v = Vec::with_capacity(sw * sh);
            for y in (0..h).step_by(step) {
                v.extend(m.data[y * w..(y + 1) * w].iter().step_by(step));
            }
            Ok((Arc::new(v), sw))
        }
        Source::Rasters(r, l) => {
            let win = Window { x0: 0, y0: 0, width: w, height: h };
            let v = r.sample(*l, win, step).map_err(|e| e.to_string())?;
            Ok((Arc::new(v.into_iter().map(f64::from).collect()), sw))
        }
        Source::Blank(_) => Ok((Arc::new(vec![f64::NAN; sw * sh]), sw)),
    }
}

/// The difference of `b` from `a` on `a`'s grid; of classes when `classes` gives the names of both maps' classes.
pub fn make(a: &Source, b: &Source, classes: Option<(&Names, &Names)>) -> Result<Made, String> {
    make_in(a, b, classes, Lang::En)
}

/// The difference in a language: its summary and the transitions' names in it.
pub fn make_in(a: &Source, b: &Source, classes: Option<(&Names, &Names)>, lang: Lang) -> Result<Made, String> {
    if crate::map_view::place_on(b.grid(), a.grid()).is_none() {
        return Err(lang
            .t("The map compared is of another coordinate system, or rotated: put it on this map's grid \
                (raster.match) to see the difference.")
            .into());
    }
    let (ga, gb) = (a.grid(), b.grid());
    let (w, h) = (ga.width, ga.height);
    let step = step_for(w, h);
    let (va, aw) = read(a, step)?;
    let ah = h.div_ceil(step);
    // the other map read as finely (its own step for its own size), each of this one's cells looked up in it by
    // its centre's coordinates; the same grid cell for cell
    let same = gb.width == w && gb.height == h && gb.transform == ga.transform;
    let bstep = if same { step } else { step_for(gb.width, gb.height) };
    let (vb, bw) = read(b, bstep)?;
    let bh = gb.height.div_ceil(bstep);
    let (ta, tb) = (ga.transform, gb.transform);
    let cols: Vec<Option<usize>> = (0..aw)
        .map(|j| {
            if same {
                return Some(j);
            }
            let x = ta[0] + ((j * step) as f64 + 0.5) * ta[1];
            let c = ((x - tb[0]) / tb[1]).floor();
            (c >= 0.0 && (c as usize) < gb.width).then(|| (c as usize / bstep).min(bw - 1))
        })
        .collect();
    let rows: Vec<Option<usize>> = (0..ah)
        .map(|i| {
            if same {
                return Some(i);
            }
            let y = ta[3] + ((i * step) as f64 + 0.5) * ta[5];
            let r = ((y - tb[3]) / tb[5]).floor();
            (r >= 0.0 && (r as usize) < gb.height).then(|| (r as usize / bstep).min(bh - 1))
        })
        .collect();
    let other = |i: usize, j: usize| match (rows[i], cols[j]) {
        (Some(r), Some(c)) => vb[r * bw + c],
        _ => f64::NAN,
    };
    let grid = if step == 1 {
        match a {
            Source::Map(m) => m.grid.clone(),
            _ => Arc::new(ga.clone()),
        }
    } else {
        let mut g = ga.clone();
        g.width = aw;
        g.height = ah;
        g.transform[1] *= step as f64;
        g.transform[2] *= step as f64;
        g.transform[4] *= step as f64;
        g.transform[5] *= step as f64;
        Arc::new(g)
    };
    let every = if step > 1 { lang.f(" (every {} cells)", &[&step]) } else { String::new() };
    let mut made = match classes {
        Some((na, nb)) => transitions(&va, aw, ah, &other, na, nb, step, lang),
        None => values(&va, aw, ah, &other, lang),
    };
    made.0.push_str(&every);
    let (summary, data, coloring, counts, names) = made;
    Ok(Made { source: Source::Map(Map::new(grid, data)), coloring, counts, names, step, summary })
}

type Parts = (String, Vec<f64>, Coloring, Vec<(f64, usize)>, Vec<(f64, String)>);

/// The other one minus this one, cell by cell, in a ramp centred on 0 (to the 98th percentile of the sizes).
fn values(va: &[f64], aw: usize, ah: usize, other: &dyn Fn(usize, usize) -> f64, lang: Lang) -> Parts {
    let mut data = Vec::with_capacity(aw * ah);
    let (mut n, mut sum, mut up, mut down) = (0usize, 0.0, 0usize, 0usize);
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for i in 0..ah {
        for j in 0..aw {
            let d = other(i, j) - va[i * aw + j];
            if d.is_finite() {
                n += 1;
                sum += d;
                lo = lo.min(d);
                hi = hi.max(d);
                if d > 0.0 {
                    up += 1;
                } else if d < 0.0 {
                    down += 1;
                }
            }
            data.push(if d.is_finite() { d } else { f64::NAN });
        }
    }
    let abs: Vec<f64> = data.iter().map(|d| d.abs()).collect();
    let m = crate::palette::percentiles_2_98(&abs).map_or(0.0, |(_, m)| m);
    let m = if m > 0.0 {
        m
    } else if hi.is_finite() && lo.abs().max(hi) > 0.0 {
        lo.abs().max(hi)
    } else {
        1.0
    };
    let coloring = Coloring::Continuous { min: -m, max: m, ramp: Ramp::Diverging, log: false };
    let summary = if n == 0 {
        lang.t("no cell has a value in both maps").to_string()
    } else {
        let share = |k: usize| format!("{:.1}", 100.0 * k as f64 / n as f64);
        lang.f(
            "the map compared minus this one: on average {}, from {} to {}; higher in {}% of the cells, lower in {}%",
            &[&short(sum / n as f64), &short(lo), &short(hi), &share(up), &share(down)],
        )
    };
    (summary, data, coloring, Vec::new(), Vec::new())
}

/// A number written short: a few significant digits.
fn short(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        return terraflux_ops::value::class_label(v);
    }
    let digits = (3 - v.abs().log10().floor() as i32 - 1).clamp(0, 6) as usize;
    let s = format!("{v:.digits$}");
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s }
}

/// The transitions: a class each, the most common first (value 1 on), the cells that kept their class
/// [`UNCHANGED`], those without a value in either map none; beyond [`TRANSITIONS`] the others together.
#[allow(clippy::too_many_arguments)]
fn transitions(
    va: &[f64],
    aw: usize,
    ah: usize,
    other: &dyn Fn(usize, usize) -> f64,
    na: &Names,
    nb: &Names,
    step: usize,
    lang: Lang,
) -> Parts {
    let mut pairs = Vec::with_capacity(aw * ah);
    let mut count: HashMap<(i64, i64), usize> = HashMap::new();
    for i in 0..ah {
        for j in 0..aw {
            let (p, q) = (va[i * aw + j], other(i, j));
            let pair = (p.is_finite() && q.is_finite()).then_some((p as i64, q as i64));
            if let Some(k) = pair {
                *count.entry(k).or_default() += 1;
            }
            pairs.push(pair);
        }
    }
    let both: usize = count.values().sum();
    let kept: usize = count.iter().filter(|((p, q), _)| p == q).map(|(_, n)| n).sum();
    let mut changes: Vec<((i64, i64), usize)> = count.into_iter().filter(|((p, q), _)| p != q).collect();
    changes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    // (a transition's class: its place in that order, the rest together after the named ones)
    let rest = changes.len() > TRANSITIONS;
    let named = if rest { TRANSITIONS - 1 } else { changes.len() };
    let class: HashMap<(i64, i64), f64> =
        changes.iter().enumerate().map(|(i, (k, _))| (*k, (i.min(named) + 1) as f64)).collect();
    let data = pairs
        .iter()
        .map(|p| match p {
            Some((x, y)) if x == y => UNCHANGED,
            Some(k) => class[k],
            None => f64::NAN,
        })
        .collect();
    let name = |names: &Names, v: i64| names.get(&v).cloned().unwrap_or_else(|| v.to_string());
    let mut names = vec![(UNCHANGED, lang.t("unchanged").to_string())];
    let mut colours = vec![(UNCHANGED, Color32::from_gray(205))];
    let cells = step * step;
    let mut counts = vec![(UNCHANGED, kept * cells)];
    for (i, ((p, q), n)) in changes.iter().take(named).enumerate() {
        let v = (i + 1) as f64;
        names.push((v, format!("{} -> {}", name(na, *p), name(nb, *q))));
        colours.push((v, class_colour(i)));
        counts.push((v, n * cells));
    }
    if rest {
        let v = (named + 1) as f64;
        let n: usize = changes[named..].iter().map(|(_, n)| n).sum();
        names.push((v, lang.f("other changes ({})", &[&(changes.len() - named)])));
        colours.push((v, Color32::from_gray(90)));
        counts.push((v, n * cells));
    }
    let changed = both - kept;
    let summary = if both == 0 {
        lang.t("no cell has a class in both maps").to_string()
    } else {
        let said = if changes.len() == 1 {
            "{} of {} cells changed class ({}%), in {} transition"
        } else {
            "{} of {} cells changed class ({}%), in {} transitions"
        };
        let share = format!("{:.1}", 100.0 * changed as f64 / both as f64);
        lang.f(said, &[&(changed * cells), &(both * cells), &share, &changes.len()])
    };
    (summary, data, Coloring::Classes(colours), counts, names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_raster::Grid;

    fn map(w: usize, h: usize, transform: [f64; 6], data: Vec<f64>) -> Source {
        let grid = Grid { width: w, height: h, transform, geo: Default::default() };
        Source::Map(Map::new(Arc::new(grid), data))
    }

    fn data(s: &Source) -> Vec<f64> {
        match s {
            Source::Map(m) => m.data.to_vec(),
            _ => unreachable!(),
        }
    }

    const T: [f64; 6] = [0.0, 1.0, 0.0, 3.0, 0.0, -1.0];

    #[test]
    fn values_are_the_other_minus_this_one() {
        let a = map(3, 1, T, vec![1.0, 2.0, f64::NAN]);
        let b = map(3, 1, T, vec![4.0, 1.0, 5.0]);
        let d = make(&a, &b, None).unwrap();
        assert_eq!(data(&d.source)[..2], [3.0, -1.0]);
        assert!(data(&d.source)[2].is_nan());
        assert!(matches!(d.coloring, Coloring::Continuous { ramp: Ramp::Diverging, min, max, .. } if min == -max));
        assert!(d.summary.contains("on average 1,") && d.summary.contains("higher in 50.0%"), "{}", d.summary);
    }

    #[test]
    fn classes_give_the_transitions_most_common_first() {
        let a = map(5, 1, T, vec![1.0, 1.0, 1.0, 2.0, 2.0]);
        let b = map(5, 1, T, vec![1.0, 2.0, 2.0, 1.0, f64::NAN]);
        let na: Names = [(1, "forest".to_string()), (2, "pasture".to_string())].into();
        let d = make(&a, &b, Some((&na, &na))).unwrap();
        let v = data(&d.source);
        assert_eq!(v[..4], [UNCHANGED, 1.0, 1.0, 2.0]);
        assert!(v[4].is_nan());
        assert_eq!(d.name(1.0), "forest -> pasture");
        assert_eq!(d.name(2.0), "pasture -> forest");
        assert_eq!(d.name(UNCHANGED), "unchanged");
        assert_eq!(d.counts, vec![(0.0, 1), (1.0, 2), (2.0, 1)]);
        assert!(d.summary.starts_with("3 of 4 cells changed class (75.0%), in 2 transitions"), "{}", d.summary);
    }

    #[test]
    fn beyond_the_named_transitions_the_rest_together() {
        let n = TRANSITIONS + 3;
        let a = map(n, 1, T, (0..n).map(|i| i as f64).collect());
        let b = map(n, 1, T, (0..n).map(|i| (i + 100) as f64).collect());
        let d = make(&a, &b, Some((&Names::new(), &Names::new()))).unwrap();
        assert_eq!(d.names.len(), TRANSITIONS + 1);
        assert_eq!(d.names.last().unwrap().1, "other changes (4)");
        assert_eq!(d.counts.last().unwrap(), &(TRANSITIONS as f64, 4));
        assert_eq!(d.name(1.0), "0 -> 100");
    }

    #[test]
    fn another_grid_is_read_where_its_cells_are() {
        // the other map's cells twice as large, starting a cell right of this one's: this one's first cell is
        // outside it, the next two in its first cell, the last in its second
        let a = map(4, 1, T, vec![0.0; 4]);
        let b = map(2, 1, [1.0, 2.0, 0.0, 3.0, 0.0, -1.0], vec![5.0, 7.0]);
        let v = data(&make(&a, &b, None).unwrap().source);
        assert!(v[0].is_nan());
        assert_eq!(v[1..], [5.0, 5.0, 7.0]);
    }

    #[test]
    fn a_very_large_map_is_compared_every_few_cells() {
        assert_eq!(step_for(4096, 4096), 1);
        assert_eq!(step_for(4097, 4096), 2);
        let a = map(4, 2, T, (0..8).map(f64::from).collect());
        let (v, w) = read(&a, 2).unwrap();
        assert_eq!((v.as_slice(), w), ([0.0, 2.0].as_slice(), 2));
    }
}
