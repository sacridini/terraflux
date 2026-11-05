//! The difference of two maps, the third view of the viewer's comparison: maps of values the other one minus this
//! one, cell by cell, in a ramp centred on 0; maps of classes the transitions, a class each ("forest -> pasture"),
//! the cells that did not change apart. Made on the grid of the map shown (every few cells when it is very
//! large), the other's cells taken where they are by the coordinates; a map of another coordinate system has its
//! cells carried through the transformation, the nearest cell taken (nothing interpolated, classes and values alike).

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
    /// Of two maps of classes: every transition, the cells and the area from each class to each.
    pub matrix: Option<Matrix>,
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
        // (a colour composite: its first band)
        Source::Rgb(r) => read(&r.bands[0], step),
    }
}

/// The difference of `b` from `a` on `a`'s grid; of classes when `classes` gives the names of both maps' classes.
pub fn make(a: &Source, b: &Source, classes: Option<(&Names, &Names)>) -> Result<Made, String> {
    make_in(a, b, classes, Lang::En)
}

/// The difference in a language: its summary and the transitions' names in it.
pub fn make_in(a: &Source, b: &Source, classes: Option<(&Names, &Names)>, lang: Lang) -> Result<Made, String> {
    make_focused(a, b, classes, None, lang)
}

/// The difference of two maps of classes with one transition (from, to) singled out: its cells in its colour, the
/// other changes grey, the cells that kept their class (unless it is theirs) a light grey.
pub fn focused(a: &Source, b: &Source, classes: (&Names, &Names), focus: (i64, i64)) -> Result<Made, String> {
    make_focused(a, b, Some(classes), Some(focus), Lang::En)
}

fn make_focused(
    a: &Source,
    b: &Source,
    classes: Option<(&Names, &Names)>,
    focus: Option<(i64, i64)>,
    lang: Lang,
) -> Result<Made, String> {
    let (ga, gb) = (a.grid(), b.grid());
    // (a map of another coordinate system is carried to this one's cells; its transformation is made here, in the
    // thread that makes the difference, as the transformations are not shared between threads)
    let warp = if crate::map_view::place_on(gb, ga).is_some() {
        None
    } else {
        match crate::reproject::Reproj::new(gb, ga) {
            Ok(re) => Some(re),
            Err(_) => {
                return Err(lang
                    .t("The map compared has a coordinate system that is unknown or cannot be carried to this \
                        map's: put it on this map's grid (raster.match) to see the difference.")
                    .into());
            }
        }
    };
    let (w, h) = (ga.width, ga.height);
    let step = step_for(w, h);
    let (va, aw) = read(a, step)?;
    let ah = h.div_ceil(step);
    // the other map read as finely (its own step for its own size), each of this one's cells looked up in it by
    // its centre's coordinates; the same grid cell for cell
    let same = warp.is_none() && gb.width == w && gb.height == h && gb.transform == ga.transform;
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
    let warped = warp.as_ref().map(|re| carried(re, &vb, bw, bh, bstep, gb, (aw, ah, step)));
    let other = |i: usize, j: usize| match &warped {
        Some(v) => v[i * aw + j],
        None => match (rows[i], cols[j]) {
            (Some(r), Some(c)) => vb[r * bw + c],
            _ => f64::NAN,
        },
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
    let (mut made, matrix) = match classes {
        Some((na, nb)) => {
            let (parts, matrix) = transitions(&va, (aw, ah, step), &grid, &other, (na, nb), focus, lang);
            (parts, Some(matrix))
        }
        None => (values(&va, aw, ah, &other, lang), None),
    };
    made.0.push_str(&every);
    let (summary, data, coloring, counts, names) = made;
    Ok(Made { source: Source::Map(Map::new(grid, data)), coloring, counts, names, step, summary, matrix })
}

/// The other map (`vb`, `bw` by `bh` cells read every `bstep`) on this map's grid (`aw` by `ah` cells, every `step`),
/// row by row: each cell's centre carried to the other map's cells, the nearest one taken (for classes and values
/// alike, nothing is interpolated); NaN where the other map has no cell.
fn carried(
    re: &crate::reproject::Reproj,
    vb: &[f64],
    bw: usize,
    bh: usize,
    bstep: usize,
    gb: &terraflux_raster::Grid,
    (aw, ah, step): (usize, usize, usize),
) -> Vec<f64> {
    let mut out = Vec::with_capacity(aw * ah);
    for i in 0..ah {
        let y = (i * step) as f64 + 0.5;
        let row: Vec<[f64; 2]> = (0..aw).map(|j| [(j * step) as f64 + 0.5, y]).collect();
        out.extend(re.to_source_many(&row).iter().map(|p| {
            let (c, r) = (p[0].floor(), p[1].floor());
            if c >= 0.0 && r >= 0.0 && c < gb.width as f64 && r < gb.height as f64 {
                let (c, r) = ((c as usize / bstep).min(bw - 1), (r as usize / bstep).min(bh - 1));
                vb[r * bw + c]
            } else {
                f64::NAN
            }
        }));
    }
    out
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

/// Every transition of two maps of classes: the classes of this map (the rows, by value) and of the other (the
/// columns), the cells that went from each to each (of the map shown, as many as a cell compared stands for) and
/// their area in square metres (none on a grid with no coordinate system).
#[derive(Debug, Clone, PartialEq)]
pub struct Matrix {
    pub from: Vec<(i64, String)>,
    pub to: Vec<(i64, String)>,
    pub cells: Vec<Vec<f64>>,
    pub area: Option<Vec<Vec<f64>>>,
}

/// What a class gained and lost between the two maps (in cells, or square metres): its amount in each, the gross
/// gain and loss; the net change (gain minus loss) and the swap (twice the smaller of gain and loss: the change
/// that moved the class without changing its amount).
#[derive(Debug, Clone, PartialEq)]
pub struct Budget {
    pub class: i64,
    pub name: String,
    pub before: f64,
    pub after: f64,
    pub gain: f64,
    pub loss: f64,
}

impl Budget {
    pub fn net(&self) -> f64 {
        self.gain - self.loss
    }

    pub fn swap(&self) -> f64 {
        2.0 * self.gain.min(self.loss)
    }
}

impl Matrix {
    /// The amounts: the areas when asked for and known, else the cells.
    pub fn amounts(&self, area: bool) -> &Vec<Vec<f64>> {
        match (&self.area, area) {
            (Some(a), true) => a,
            _ => &self.cells,
        }
    }

    /// Everything compared: the cells (or area) with a class in both maps.
    pub fn total(&self, area: bool) -> f64 {
        self.amounts(area).iter().flatten().sum()
    }

    /// The amount that kept its class (a class of the same value in both maps).
    pub fn kept(&self, area: bool) -> f64 {
        let m = self.amounts(area);
        let mut k = 0.0;
        for (i, (p, _)) in self.from.iter().enumerate() {
            if let Some(j) = self.to.iter().position(|(q, _)| q == p) {
                k += m[i][j];
            }
        }
        k
    }

    /// The gains and losses of every class of either map, by value.
    pub fn budgets(&self, area: bool) -> Vec<Budget> {
        let m = self.amounts(area);
        let mut classes: Vec<(i64, String)> = self.from.clone();
        for c in &self.to {
            if !classes.iter().any(|(v, _)| *v == c.0) {
                classes.push(c.clone());
            }
        }
        classes.sort_by_key(|(v, _)| *v);
        classes
            .into_iter()
            .map(|(class, name)| {
                let i = self.from.iter().position(|(v, _)| *v == class);
                let j = self.to.iter().position(|(v, _)| *v == class);
                let before = i.map_or(0.0, |i| m[i].iter().sum());
                let after = j.map_or(0.0, |j| m.iter().map(|r| r[j]).sum());
                let stay = match (i, j) {
                    (Some(i), Some(j)) => m[i][j],
                    _ => 0.0,
                };
                Budget { class, name, before, after, gain: after - stay, loss: before - stay }
            })
            .collect()
    }
}

/// The transitions: a class each, the most common first (value 1 on), the cells that kept their class
/// [`UNCHANGED`], those without a value in either map none; beyond [`TRANSITIONS`] the others together. With a
/// transition singled out (`focus`), its cells (1), the other changes (2) and the cells kept [`UNCHANGED`]. And the
/// matrix of every transition, the area of each cell by its row (`grid`: the grid the difference is made on).
fn transitions(
    va: &[f64],
    (aw, ah, step): (usize, usize, usize),
    grid: &terraflux_raster::Grid,
    other: &dyn Fn(usize, usize) -> f64,
    (na, nb): (&Names, &Names),
    focus: Option<(i64, i64)>,
    lang: Lang,
) -> (Parts, Matrix) {
    let mut pairs = Vec::with_capacity(aw * ah);
    let mut count: HashMap<(i64, i64), (usize, f64)> = HashMap::new();
    let areas: Option<Vec<f64>> = (0..ah).map(|i| crate::measure::cell_area(grid, i)).collect();
    for i in 0..ah {
        let area = areas.as_ref().map_or(0.0, |a| a[i]);
        for j in 0..aw {
            let (p, q) = (va[i * aw + j], other(i, j));
            let pair = (p.is_finite() && q.is_finite()).then_some((p as i64, q as i64));
            if let Some(k) = pair {
                let e = count.entry(k).or_default();
                e.0 += 1;
                e.1 += area;
            }
            pairs.push(pair);
        }
    }
    let name = |names: &Names, v: i64| names.get(&v).cloned().unwrap_or_else(|| v.to_string());
    let cells = step * step;
    // the matrix: the classes of each map in the order of their values
    let mut from: Vec<i64> = count.keys().map(|k| k.0).collect();
    let mut to: Vec<i64> = count.keys().map(|k| k.1).collect();
    for v in [&mut from, &mut to] {
        v.sort_unstable();
        v.dedup();
    }
    let mut mc = vec![vec![0.0; to.len()]; from.len()];
    let mut ma = mc.clone();
    for ((p, q), (n, a)) in &count {
        let (i, j) = (from.binary_search(p).unwrap_or(0), to.binary_search(q).unwrap_or(0));
        mc[i][j] = (n * cells) as f64;
        ma[i][j] = *a;
    }
    let matrix = Matrix {
        from: from.iter().map(|v| (*v, name(na, *v))).collect(),
        to: to.iter().map(|v| (*v, name(nb, *v))).collect(),
        cells: mc,
        area: areas.is_some().then_some(ma),
    };
    let both: usize = count.values().map(|c| c.0).sum();
    let kept: usize = count.iter().filter(|((p, q), _)| p == q).map(|(_, c)| c.0).sum();
    let mut changes: Vec<((i64, i64), usize)> =
        count.iter().filter(|((p, q), _)| p != q).map(|(k, c)| (*k, c.0)).collect();
    changes.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    let rest = changes.len() > TRANSITIONS;
    let named = if rest { TRANSITIONS - 1 } else { changes.len() };
    if let Some(f) = focus {
        // (in the colour the transition has among those named, when it is one)
        let colour =
            changes.iter().take(named).position(|(k, _)| *k == f).map_or(Color32::from_rgb(230, 85, 13), class_colour);
        let data = pairs
            .iter()
            .map(|p| match p {
                Some(k) if *k == f => 1.0,
                Some((x, y)) if x != y => 2.0,
                Some(_) => UNCHANGED,
                None => f64::NAN,
            })
            .collect();
        let n = count.get(&f).map_or(0, |c| c.0);
        let others = both - kept - if f.0 != f.1 { n } else { 0 };
        let label = if f.0 == f.1 {
            lang.f("{} (kept)", &[&name(na, f.0)])
        } else {
            format!("{} -> {}", name(na, f.0), name(nb, f.1))
        };
        let names = vec![
            (UNCHANGED, lang.t("unchanged").to_string()),
            (1.0, label.clone()),
            (2.0, lang.t("other changes").to_string()),
        ];
        let colours = vec![(UNCHANGED, Color32::from_gray(225)), (1.0, colour), (2.0, Color32::from_gray(140))];
        let unchanged = kept - if f.0 == f.1 { n } else { 0 };
        let counts = vec![(UNCHANGED, unchanged * cells), (1.0, n * cells), (2.0, others * cells)];
        let share = if both > 0 { format!("{:.1}", 100.0 * n as f64 / both as f64) } else { "0".into() };
        let summary = lang.f("{}: {} cells ({}%)", &[&label, &(n * cells), &share]);
        return ((summary, data, Coloring::Classes(colours), counts, names), matrix);
    }
    // (a transition's class: its place in that order, the rest together after the named ones)
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
    let mut names = vec![(UNCHANGED, lang.t("unchanged").to_string())];
    let mut colours = vec![(UNCHANGED, Color32::from_gray(205))];
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
    ((summary, data, Coloring::Classes(colours), counts, names), matrix)
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
    fn the_matrix_of_every_transition_with_gains_and_losses() {
        // cells of 10 x 10 m (1 are) in a projected grid: forest 1 -> 1, 1 -> 2 twice; pasture 2 -> 1; 3 -> 2
        let mut g = Grid::new(5, 1);
        g.transform = [0.0, 10.0, 0.0, 10.0, 0.0, -10.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32633]);
        let a = Source::Map(Map::new(Arc::new(g.clone()), vec![1.0, 1.0, 1.0, 2.0, 3.0]));
        let b = Source::Map(Map::new(Arc::new(g), vec![1.0, 2.0, 2.0, 1.0, 2.0]));
        let na: Names = [(1, "forest".to_string()), (2, "pasture".to_string())].into();
        let m = make(&a, &b, Some((&na, &na))).unwrap().matrix.unwrap();
        assert_eq!(m.from, vec![(1, "forest".into()), (2, "pasture".into()), (3, "3".into())]);
        assert_eq!(m.to, vec![(1, "forest".into()), (2, "pasture".into())]);
        assert_eq!(m.cells, vec![vec![1.0, 2.0], vec![1.0, 0.0], vec![0.0, 1.0]]);
        assert_eq!(m.area.as_ref().unwrap()[0][1], 200.0);
        assert_eq!((m.total(false), m.kept(false), m.total(true)), (5.0, 1.0, 500.0));
        let b = m.budgets(false);
        let forest = &b[0];
        assert_eq!((forest.before, forest.after, forest.gain, forest.loss), (3.0, 2.0, 1.0, 2.0));
        assert_eq!((forest.net(), forest.swap()), (-1.0, 2.0));
        let pasture = &b[1];
        assert_eq!((pasture.gain, pasture.loss, pasture.net(), pasture.swap()), (3.0, 1.0, 2.0, 2.0));
        assert_eq!((b[2].class, b[2].before, b[2].after, b[2].loss), (3, 1.0, 0.0, 1.0));
        // values: no matrix
        assert!(make(&a, &a, None).unwrap().matrix.is_none());
    }

    #[test]
    fn a_transition_singled_out() {
        let a = map(5, 1, T, vec![1.0, 1.0, 1.0, 2.0, 2.0]);
        let b = map(5, 1, T, vec![1.0, 2.0, 2.0, 1.0, f64::NAN]);
        let na: Names = [(1, "forest".to_string()), (2, "pasture".to_string())].into();
        let d = focused(&a, &b, (&na, &na), (2, 1)).unwrap();
        let v = data(&d.source);
        assert!(v[4].is_nan());
        assert_eq!(v[..4], [UNCHANGED, 2.0, 2.0, 1.0]);
        assert_eq!(d.name(1.0), "pasture -> forest");
        assert_eq!(d.counts, vec![(UNCHANGED, 1), (1.0, 1), (2.0, 2)]);
        assert!(d.summary.starts_with("pasture -> forest: 1 cells (25.0%)"), "{}", d.summary);
        // a class kept: its cells
        let d = focused(&a, &b, (&na, &na), (1, 1)).unwrap();
        assert_eq!(data(&d.source)[..4], [1.0, 2.0, 2.0, 2.0]);
        assert_eq!(d.name(1.0), "forest (kept)");
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
    fn a_map_of_another_coordinate_system_is_carried_to_this_ones_grid() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        // 400 x 300 cells of 10 m in UTM 51N, about 122.54 E 11.76 N, all 10
        let mut g = Grid::new(400, 300);
        g.transform = [450_000.0, 10.0, 0.0, 1_300_000.0, 0.0, -10.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32651]);
        let a = Source::Map(Map::new(Arc::new(g), vec![10.0; 400 * 300]));
        // 1000 x 800 cells of 0.0001 degrees from 122.5 E 11.8 N: 1 west of 122.55 E, 2 east of it
        let mut geo = Grid::new(1000, 800);
        geo.transform = [122.5, 0.0001, 0.0, 11.8, 0.0, -0.0001];
        geo.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 2, 2048, 0, 1, 4326]);
        let vb = (0..1000 * 800).map(|i| if i % 1000 < 500 { 1.0 } else { 2.0 }).collect();
        let b = Source::Map(Map::new(Arc::new(geo), vb));
        let d = make(&a, &b, None).unwrap();
        let v = data(&d.source);
        assert_eq!(v.len(), 400 * 300);
        // (122.55 E is about 95 cells from the west edge)
        let at = |r: usize, c: usize| v[r * 400 + c];
        assert_eq!((at(10, 20), at(150, 30), at(280, 40)), (-9.0, -9.0, -9.0));
        assert_eq!((at(10, 300), at(150, 200), at(280, 390)), (-8.0, -8.0, -8.0));
        // classes too
        let na: Names = [(1, "forest".to_string()), (2, "pasture".to_string()), (10, "ten".to_string())].into();
        let d = make(&a, &b, Some((&na, &na))).unwrap();
        // (the east is the larger part: its transition first)
        assert_eq!(d.name(1.0), "ten -> pasture");
        assert_eq!(d.name(2.0), "ten -> forest");
        let v = data(&d.source);
        assert_eq!((v[150 * 400 + 30], v[150 * 400 + 200]), (2.0, 1.0));
    }

    #[test]
    fn a_rotated_grid_is_carried_to_this_ones_cells() {
        // (no coordinate system told for either: the cells' places are the same ones, no GDAL)
        // this map 3 x 3 cells of 1 from (0, 3); the other turned a quarter: its cell (c, r) is at x = r, y = 3 - c,
        // so this map's cell (row i, column j) is the other's (c = i, r = j): its values transposed
        let a = map(3, 3, T, vec![0.0; 9]);
        let b = map(3, 3, [0.0, 0.0, 1.0, 3.0, -1.0, 0.0], (0..9).map(f64::from).collect());
        let v = data(&make(&a, &b, None).unwrap().source);
        let expect: Vec<f64> = (0..9).map(|k| f64::from((k % 3) * 3 + k / 3)).collect();
        assert_eq!(v, expect);
        // and the other way round, this map the rotated one: the flat map's zeros minus its own values, cell for cell
        let v = data(&make(&b, &a, None).unwrap().source);
        assert_eq!(v, (0..9).map(|k| -f64::from(k)).collect::<Vec<_>>());
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
