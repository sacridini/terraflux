//! The area and the statistics of a part of a map, for the Viewer: the whole map, the part in view, or polygons (one
//! drawn on the map, the features selected in a table of attributes). Of a map of classes, the cells and the area
//! of each class in it; of a map of values, their count, mean, extremes, sum and spread. Worked out in a thread, of
//! every cell, or of a sample of about [`crate::histogram::SAMPLE`] cells when the part is larger (each sampled cell
//! standing for the cells around it).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};

use crate::map_view::{Source, SourceKey};

/// Which part of a map.
#[derive(Debug, Clone, PartialEq)]
pub enum Part {
    Map,
    /// A box of cells (`[x0, y0, x1, y1]`, columns and rows of the map's grid): the part in view.
    Window([f64; 4]),
    /// Polygons, as rings of cells (column, row) of the map's grid; a cell is in when its centre is (even-odd).
    Polygons(Vec<Vec<[f64; 2]>>),
}

impl Part {
    fn key(&self) -> u64 {
        let mut h = std::hash::DefaultHasher::new();
        match self {
            Part::Map => 0u8.hash(&mut h),
            Part::Window(b) => {
                1u8.hash(&mut h);
                b.iter().for_each(|v| v.to_bits().hash(&mut h));
            }
            Part::Polygons(rings) => {
                2u8.hash(&mut h);
                for r in rings {
                    r.len().hash(&mut h);
                    r.iter().flatten().for_each(|v| v.to_bits().hash(&mut h));
                }
            }
        }
        h.finish()
    }

    /// The box of cells it spans, within a map of `w` by `h` cells: `[x0, y0, x1, y1]` (none when outside it).
    fn bounds(&self, w: usize, h: usize) -> Option<[usize; 4]> {
        let b = match self {
            Part::Map => [0.0, 0.0, w as f64, h as f64],
            Part::Window(b) => *b,
            Part::Polygons(rings) => {
                let mut b = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];
                for p in rings.iter().flatten() {
                    b = [b[0].min(p[0]), b[1].min(p[1]), b[2].max(p[0]), b[3].max(p[1])];
                }
                b
            }
        };
        if !b.iter().all(|v| v.is_finite()) {
            return None;
        }
        let x0 = b[0].floor().clamp(0.0, w as f64) as usize;
        let y0 = b[1].floor().clamp(0.0, h as f64) as usize;
        let x1 = b[2].ceil().clamp(0.0, w as f64) as usize;
        let y1 = b[3].ceil().clamp(0.0, h as f64) as usize;
        (x1 > x0 && y1 > y0).then_some([x0, y0, x1, y1])
    }
}

/// A class in a part of a map: its value, its cells and their area in square metres (none with no coordinate
/// system).
#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub value: f64,
    pub cells: f64,
    pub area: Option<f64>,
}

/// What a part of a map holds.
#[derive(Debug, Clone, PartialEq)]
pub struct Stats {
    /// The cells of the part with a value and without (as many as the cells sampled stand for).
    pub cells: f64,
    pub empty: f64,
    /// The area of the cells with a value, and of the whole part, in square metres (none with no coordinate system).
    pub area: Option<f64>,
    pub part_area: Option<f64>,
    /// Of a map of classes: each class, by value.
    pub classes: Vec<Class>,
    /// Of a map of values (NaN when no cell has one); the sum of every cell's value.
    pub min: f64,
    pub max: f64,
    pub mean: f64,
    pub sum: f64,
    pub sd: f64,
    /// Every how many rows and columns the cells were read (1: all of them).
    pub step: usize,
}

impl Stats {
    fn none(step: usize) -> Stats {
        Stats {
            cells: 0.0,
            empty: 0.0,
            area: None,
            part_area: None,
            classes: Vec::new(),
            min: f64::NAN,
            max: f64::NAN,
            mean: f64::NAN,
            sum: f64::NAN,
            sd: f64::NAN,
            step,
        }
    }

    /// The share of the cells with a value (of the area, when known) a class has, in percent.
    pub fn share(&self, c: &Class) -> f64 {
        match (c.area, self.area) {
            (Some(a), Some(all)) if all > 0.0 => 100.0 * a / all,
            _ if self.cells > 0.0 => 100.0 * c.cells / self.cells,
            _ => 0.0,
        }
    }
}

/// Where a row of cells (its centre at `y`) is inside rings: the spans `[from, to)` of x, by the even-odd rule.
pub fn spans(rings: &[Vec<[f64; 2]>], y: f64) -> Vec<[f64; 2]> {
    let mut xs = Vec::new();
    for r in rings {
        for i in 0..r.len() {
            let (p, q) = (r[i], r[(i + 1) % r.len()]);
            if (p[1] <= y) != (q[1] <= y) {
                xs.push(p[0] + (y - p[1]) * (q[0] - p[0]) / (q[1] - p[1]));
            }
        }
    }
    xs.sort_by(f64::total_cmp);
    xs.as_chunks::<2>().0.to_vec()
}

/// The statistics of a part of a map: of its classes when `classes`, else of its values.
pub fn of(source: &Source, part: &Part, classes: bool) -> Stats {
    let (w, h) = source.size();
    let Some([x0, y0, x1, y1]) = part.bounds(w, h) else { return Stats::none(1) };
    let (bw, bh) = (x1 - x0, y1 - y0);
    let step = crate::histogram::sample_step(bw, bh);
    let sw = bw.div_ceil(step);
    let values: Vec<f64> = match source {
        Source::Map(m) => (y0..y1)
            .step_by(step)
            .flat_map(|y| (x0..x1).step_by(step).map(move |x| (y, x)))
            .map(|(y, x)| m.data[y * w + x])
            .collect(),
        Source::Rasters(r, l) => {
            let win = terraflux_raster::tiff::Window { x0: x0 as i64, y0: y0 as i64, width: bw, height: bh };
            r.sample(*l, win, step).map(|v| v.into_iter().map(f64::from).collect()).unwrap_or_default()
        }
        Source::Blank(_) => vec![f64::NAN; sw * bh.div_ceil(step)],
        // (a colour composite: the statistics of its first band)
        Source::Rgb(r) => return of(&r.bands[0], part, classes),
        Source::Shade(s) => return of(&s.base, part, classes),
    };
    let grid = source.grid();
    let weight = (step * step) as f64;
    let mut st = Stats::none(step);
    let (mut sum, mut sq, mut n) = (0.0, 0.0, 0.0);
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    let mut area = 0.0;
    let mut part_area = 0.0;
    let mut known = true;
    let mut by: HashMap<u64, (f64, f64)> = HashMap::new();
    for (i, y) in (y0..y1).step_by(step).enumerate() {
        let inside = match part {
            Part::Polygons(rings) => Some(spans(rings, y as f64 + 0.5)),
            _ => None,
        };
        let cell = crate::measure::cell_area(grid, y);
        known &= cell.is_some();
        let cell = cell.unwrap_or(0.0) * weight;
        for (j, x) in (x0..x1).step_by(step).enumerate() {
            if let Some(s) = &inside {
                let cx = x as f64 + 0.5;
                if !s.iter().any(|s| cx >= s[0] && cx < s[1]) {
                    continue;
                }
            }
            part_area += cell;
            let v = values.get(i * sw + j).copied().unwrap_or(f64::NAN);
            if !v.is_finite() {
                st.empty += weight;
                continue;
            }
            st.cells += weight;
            area += cell;
            if classes {
                let e = by.entry(v.to_bits()).or_default();
                e.0 += weight;
                e.1 += cell;
            } else {
                n += 1.0;
                sum += v;
                sq += v * v;
                min = min.min(v);
                max = max.max(v);
            }
        }
    }
    if known {
        (st.area, st.part_area) = (Some(area), Some(part_area));
    }
    if classes {
        let mut c: Vec<Class> = by
            .into_iter()
            .map(|(v, (cells, a))| Class { value: f64::from_bits(v), cells, area: known.then_some(a) })
            .collect();
        c.sort_by(|a, b| a.value.total_cmp(&b.value));
        st.classes = c;
    } else if n > 0.0 {
        st.mean = sum / n;
        st.sd = (sq / n - st.mean * st.mean).max(0.0).sqrt();
        (st.min, st.max, st.sum) = (min, max, sum * weight);
    }
    st
}

/// Things worked out in threads from maps (the statistics of parts of them, the profiles along lines), each once,
/// by a key: the map's and what is asked of it. A few kept, the maps held while they are.
pub struct Jobs<T> {
    kept: Vec<Job<T>>,
    channel: Option<Channel<T>>,
    clock: u64,
}

/// A job kept: its key, the map it is of (held), what it gave (none while worked out) and when it was last asked for.
type Job<T> = ((SourceKey, u64), Source, Option<Arc<T>>, u64);
type Channel<T> = (Sender<((SourceKey, u64), T)>, Receiver<((SourceKey, u64), T)>);

impl<T> Default for Jobs<T> {
    fn default() -> Self {
        Jobs { kept: Vec::new(), channel: None, clock: 0 }
    }
}

impl<T: Send + Sync + 'static> Jobs<T> {
    /// Whether a job is being worked out (what came meanwhile taken in).
    pub fn busy(&mut self) -> bool {
        if let Some((_, rx)) = &self.channel {
            while let Ok((k, v)) = rx.try_recv() {
                if let Some(e) = self.kept.iter_mut().find(|e| e.0 == k) {
                    e.2 = Some(Arc::new(v));
                }
            }
        }
        self.kept.iter().any(|e| e.2.is_none())
    }

    /// What `work` gives of `source` for `key`, or none while it is worked out (in a thread).
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        source: &Source,
        key: u64,
        work: impl FnOnce(&Source) -> T + Send + 'static,
    ) -> Option<Arc<T>> {
        let (tx, rx) = self.channel.get_or_insert_with(channel);
        while let Ok((k, v)) = rx.try_recv() {
            if let Some(e) = self.kept.iter_mut().find(|e| e.0 == k) {
                e.2 = Some(Arc::new(v));
            }
        }
        self.clock += 1;
        let key = (source.key(), key);
        if let Some(e) = self.kept.iter_mut().find(|e| e.0 == key) {
            e.3 = self.clock;
            return e.2.clone();
        }
        let (tx, src, c) = (tx.clone(), source.clone(), ctx.clone());
        let _ = std::thread::Builder::new().name("terraflux-viewer-job".into()).spawn(move || {
            let _ = tx.send((key, work(&src)));
            c.request_repaint();
        });
        self.kept.push((key, source.clone(), None, self.clock));
        while self.kept.len() > 16 {
            let Some(i) = (0..self.kept.len()).filter(|&i| self.kept[i].2.is_some()).min_by_key(|&i| self.kept[i].3)
            else {
                break;
            };
            self.kept.swap_remove(i);
        }
        None
    }
}

/// The statistics of parts of maps.
#[derive(Default)]
pub struct Regions(Jobs<Stats>);

impl Regions {
    pub fn busy(&mut self) -> bool {
        self.0.busy()
    }

    /// The statistics of a part of a map, or none while they are worked out.
    pub fn get(&mut self, ctx: &egui::Context, source: &Source, part: &Part, classes: bool) -> Option<Arc<Stats>> {
        let key = part.key() ^ u64::from(classes);
        let part = part.clone();
        self.0.get(ctx, source, key, move |src| of(src, &part, classes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Map;
    use terraflux_raster::Grid;

    /// 4 x 4 cells of 100 m (1 ha) in UTM: the rows 0..4 of classes 1, 1, 2, NaN.
    fn classes() -> Source {
        let mut g = Grid::new(4, 4);
        g.transform = [500_000.0, 100.0, 0.0, 9_000_000.0, 0.0, -100.0];
        g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 32723]);
        let mut v = vec![1.0; 8];
        v.extend([2.0; 4]);
        v.extend([f64::NAN; 4]);
        Source::Map(Map::new(Arc::new(g), v))
    }

    #[test]
    fn the_area_of_each_class() {
        let st = of(&classes(), &Part::Map, true);
        assert_eq!((st.cells, st.empty, st.step), (12.0, 4.0, 1));
        assert_eq!(st.area, Some(120_000.0));
        assert_eq!(st.part_area, Some(160_000.0));
        assert_eq!(st.classes.len(), 2);
        assert_eq!((st.classes[0].value, st.classes[0].cells, st.classes[0].area), (1.0, 8.0, Some(80_000.0)));
        assert!((st.share(&st.classes[1]) - 100.0 / 3.0).abs() < 1e-9);
        // the part in view: the left half of the rows 1 and 2
        let st = of(&classes(), &Part::Window([0.0, 1.0, 2.0, 3.0]), true);
        assert_eq!(st.classes.iter().map(|c| c.cells).collect::<Vec<_>>(), vec![2.0, 2.0]);
    }

    #[test]
    fn a_polygon_takes_the_cells_whose_centres_are_in() {
        // a triangle over the top-left: the cells (0,0), (1,0), (0,1) have their centres in it
        let tri = Part::Polygons(vec![vec![[0.0, 0.0], [2.6, 0.0], [0.0, 2.6]]]);
        let st = of(&classes(), &tri, true);
        assert_eq!(st.cells, 3.0);
        // a ring with a hole: the 4 x 4 square less the middle 2 x 2
        let holed = Part::Polygons(vec![
            vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]],
            vec![[1.0, 1.0], [3.0, 1.0], [3.0, 3.0], [1.0, 3.0]],
        ]);
        let st = of(&classes(), &holed, true);
        assert_eq!((st.cells, st.empty), (8.0, 4.0));
        assert_eq!(spans(&[vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0], [0.0, 4.0]]], 1.5), vec![[0.0, 4.0]]);
    }

    #[test]
    fn values_mean_extremes_and_sum() {
        let g = Grid::new(3, 2);
        let src = Source::Map(Map::new(Arc::new(g), vec![1.0, 2.0, 3.0, 4.0, f64::NAN, 6.0]));
        let st = of(&src, &Part::Map, false);
        assert_eq!((st.cells, st.empty, st.min, st.max, st.sum), (5.0, 1.0, 1.0, 6.0, 16.0));
        assert!((st.mean - 3.2).abs() < 1e-12);
        assert!((st.sd - (13.2f64 - 3.2 * 3.2).sqrt()).abs() < 1e-9, "{}", st.sd);
        // no coordinate system: no area
        assert_eq!(st.area, None);
        // a part outside the map: nothing
        let st = of(&src, &Part::Window([10.0, 10.0, 12.0, 12.0]), false);
        assert_eq!(st.cells, 0.0);
        assert!(st.mean.is_nan());
    }
}
