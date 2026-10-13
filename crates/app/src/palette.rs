//! Colours of maps: a categorical palette for classes, a sequential one for continuous values, and the image
//! of a map at the size the screen needs.

use egui::{Color32, ColorImage};

/// Distinct colours for classes (Okabe & Ito's, then Tableau's), in order.
const CLASSES: [[u8; 3]; 20] = [
    [0, 114, 178],
    [230, 159, 0],
    [0, 158, 115],
    [204, 121, 167],
    [86, 180, 233],
    [213, 94, 0],
    [240, 228, 66],
    [120, 94, 240],
    [140, 86, 75],
    [127, 127, 127],
    [188, 189, 34],
    [23, 190, 207],
    [31, 119, 180],
    [255, 127, 14],
    [44, 160, 44],
    [214, 39, 40],
    [148, 103, 189],
    [227, 119, 194],
    [158, 218, 229],
    [219, 219, 141],
];

/// The colour of the `i`-th class.
pub fn class_colour(i: usize) -> Color32 {
    let [r, g, b] = CLASSES[i % CLASSES.len()];
    Color32::from_rgb(r, g, b)
}

/// A sequential colour for `t` in [0, 1] (viridis, by its control points).
pub fn sequential(t: f32) -> Color32 {
    const STOPS: [[f32; 3]; 6] = [
        [68.0, 1.0, 84.0],
        [65.0, 68.0, 135.0],
        [42.0, 120.0, 142.0],
        [34.0, 168.0, 132.0],
        [122.0, 209.0, 81.0],
        [253.0, 231.0, 37.0],
    ];
    along(&STOPS, t)
}

/// A ramp of colours for continuous values, chosen by what the values are (as landschaft's plot does).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Ramp {
    /// Viridis: most values.
    #[default]
    Viridis,
    /// Blue to white to red, centred on 0: values on both sides of 0 (differences, changes, correlations).
    Diverging,
    /// Green lowlands to brown and white peaks: elevations.
    Terrain,
    /// Black to white: a hillshade.
    Grey,
    /// Red to yellow to green: NDVI, EVI.
    Vegetation,
    /// Dark to red: Zonation's priority ranks (the top fraction red), as landschaft's.
    Rank,
}

impl Ramp {
    /// The colour at `t` in [0, 1].
    pub fn colour(self, t: f32) -> Color32 {
        let stops: &[[f32; 3]] = match self {
            Ramp::Viridis => return sequential(t),
            // RdBu reversed (ColorBrewer)
            Ramp::Diverging => &[
                [5.0, 48.0, 97.0],
                [67.0, 147.0, 195.0],
                [209.0, 229.0, 240.0],
                [247.0, 247.0, 247.0],
                [253.0, 219.0, 199.0],
                [214.0, 96.0, 77.0],
                [103.0, 0.0, 31.0],
            ],
            Ramp::Terrain => &[
                [51.0, 102.0, 51.0],
                [120.0, 170.0, 80.0],
                [230.0, 220.0, 140.0],
                [170.0, 120.0, 70.0],
                [140.0, 110.0, 100.0],
                [250.0, 250.0, 250.0],
            ],
            Ramp::Grey => &[[0.0, 0.0, 0.0], [255.0, 255.0, 255.0]],
            Ramp::Rank => &[
                [0.0, 0.0, 0.0],
                [43.0, 26.0, 91.0],
                [47.0, 75.0, 154.0],
                [42.0, 133.0, 180.0],
                [79.0, 185.0, 164.0],
                [166.0, 217.0, 106.0],
                [254.0, 224.0, 139.0],
                [253.0, 174.0, 97.0],
                [244.0, 109.0, 67.0],
                [215.0, 48.0, 39.0],
                [165.0, 0.0, 38.0],
            ],
            // RdYlGn (ColorBrewer)
            Ramp::Vegetation => &[
                [165.0, 0.0, 38.0],
                [244.0, 109.0, 67.0],
                [254.0, 224.0, 139.0],
                [217.0, 239.0, 139.0],
                [102.0, 189.0, 99.0],
                [0.0, 104.0, 55.0],
            ],
        };
        along(stops, t)
    }
}

/// The colour at `t` in [0, 1] along control points spread evenly.
fn along(stops: &[[f32; 3]], t: f32) -> Color32 {
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) } * (stops.len() - 1) as f32;
    let i = (t.floor() as usize).min(stops.len() - 2);
    let f = t - i as f32;
    let c: Vec<u8> = (0..3).map(|k| (stops[i][k] + f * (stops[i + 1][k] - stops[i][k])).round() as u8).collect();
    Color32::from_rgb(c[0], c[1], c[2])
}

/// How a map is coloured.
#[derive(Debug, Clone, PartialEq)]
pub enum Coloring {
    /// A colour per class (classes not listed are grey).
    Classes(Vec<(f64, Color32)>),
    /// From `min` to `max` along a ramp, on a scale of logarithms when `log` (values at or below 0 the first
    /// colour).
    Continuous { min: f64, max: f64, ramp: Ramp, log: bool },
}

/// The values' 2nd and 98th percentiles (numpy's, by linear interpolation) of the finite ones, of a sample of
/// about a million when they are more; none when there is none.
pub fn percentiles_2_98(values: &[f64]) -> Option<(f64, f64)> {
    let step = values.len().div_ceil(1 << 20).max(1);
    let mut v: Vec<f64> = values.iter().step_by(step).copied().filter(|x| x.is_finite()).collect();
    if v.is_empty() {
        return None;
    }
    let mut at = |p: f64| {
        let h = p * (v.len() - 1) as f64;
        let (lo, hi) = (h.floor() as usize, h.ceil() as usize);
        let a = *v.select_nth_unstable_by(lo, f64::total_cmp).1;
        let b = *v.select_nth_unstable_by(hi, f64::total_cmp).1;
        a + (h - lo as f64) * (b - a)
    };
    Some((at(0.02), at(0.98)))
}

impl Coloring {
    /// Classes coloured in order, for a map of whole values with few of them; else continuous: centred on 0
    /// when the values are on both sides of it (much of each), else from the least to the most.
    pub fn for_values(values: &[f64]) -> Coloring {
        let mut classes: Vec<f64> = Vec::new();
        let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
        let mut whole = true;
        for &v in values {
            if v.is_nan() {
                continue;
            }
            lo = lo.min(v);
            hi = hi.max(v);
            if whole {
                if v.fract() != 0.0 {
                    whole = false;
                } else if !classes.contains(&v) {
                    classes.push(v);
                    if classes.len() > 40 {
                        whole = false;
                    }
                }
            }
        }
        if whole && !classes.is_empty() {
            classes.sort_by(|a, b| a.partial_cmp(b).unwrap());
            return Coloring::Classes(classes.into_iter().enumerate().map(|(i, c)| (c, class_colour(i))).collect());
        }
        // a ramp over the 2nd to the 98th percentiles (as landschaft's plot), centred on 0 when the values are on
        // both sides of it, much of each
        let (p2, p98) = percentiles_2_98(values).unwrap_or((lo, hi));
        if lo < 0.0 && hi > 0.0 && p2 < 0.0 && p98 > 0.0 && (-p2).min(p98) / (-p2).max(p98) > 0.2 {
            let abs: Vec<f64> = values.iter().map(|v| v.abs()).collect();
            let m = percentiles_2_98(&abs).map_or((-lo).max(hi), |(_, m)| m);
            return Coloring::Continuous { min: -m, max: m, ramp: Ramp::Diverging, log: false };
        }
        Coloring::continuous(values, Ramp::Viridis, false)
    }

    /// A ramp over the values' 2nd to 98th percentiles (of their logarithms, of the values above 0, with `log`);
    /// the whole range when those are the same.
    pub fn continuous(values: &[f64], ramp: Ramp, log: bool) -> Coloring {
        let positive: Vec<f64>;
        let of = if log {
            positive = values.iter().copied().filter(|v| *v > 0.0).collect();
            &positive[..]
        } else {
            values
        };
        let (lo, hi) = of
            .iter()
            .filter(|v| v.is_finite())
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
        let (mut min, mut max) = percentiles_2_98(of).unwrap_or((lo, hi));
        if max.is_nan() || min.is_nan() || max <= min {
            (min, max) = (lo, hi);
        }
        if !min.is_finite() || !max.is_finite() {
            (min, max) = if log { (1.0, 10.0) } else { (0.0, 1.0) };
        }
        Coloring::Continuous { min, max, ramp, log }
    }

    /// Where a value falls along a continuous colouring, 0 to 1 (out of the range clamped by the ramp).
    pub fn position(min: f64, max: f64, log: bool, v: f64) -> f32 {
        if log {
            if v <= 0.0 {
                return 0.0;
            }
            let (a, b) = (min.log10(), max.log10());
            return if b > a { ((v.log10() - a) / (b - a)) as f32 } else { 0.5 };
        }
        if max > min { ((v - min) / (max - min)) as f32 } else { 0.5 }
    }

    /// This colouring with a ramp of its own, when it is continuous (the range stays).
    pub fn with_ramp(self, ramp: Ramp) -> Coloring {
        match self {
            Coloring::Continuous { min, max, log, .. } => Coloring::Continuous { min, max, ramp, log },
            c => c,
        }
    }

    /// A key of the colouring: equal colourings have equal keys.
    pub fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        match self {
            Coloring::Classes(c) => {
                0u8.hash(&mut h);
                for (v, col) in c {
                    v.to_bits().hash(&mut h);
                    col.to_array().hash(&mut h);
                }
            }
            Coloring::Continuous { min, max, ramp, log } => {
                1u8.hash(&mut h);
                min.to_bits().hash(&mut h);
                max.to_bits().hash(&mut h);
                ramp.hash(&mut h);
                log.hash(&mut h);
            }
        }
        h.finish()
    }

    /// This colouring with the colours of `base` for the classes both have (a class of its own gets the next
    /// colour), so that the maps of the same classes look the same.
    pub fn like(self, base: &Coloring) -> Coloring {
        match (self, base) {
            (Coloring::Classes(own), Coloring::Classes(b)) => {
                let mut next = b.len();
                Coloring::Classes(
                    own.into_iter()
                        .map(|(v, _)| {
                            let c = b.iter().find(|(k, _)| *k == v).map(|(_, c)| *c).unwrap_or_else(|| {
                                next += 1;
                                class_colour(next - 1)
                            });
                            (v, c)
                        })
                        .collect(),
                )
            }
            (c, _) => c,
        }
    }

    pub fn colour(&self, v: f64) -> Color32 {
        if v.is_nan() {
            return Color32::TRANSPARENT;
        }
        match self {
            Coloring::Classes(c) => c.iter().find(|(k, _)| *k == v).map_or(Color32::GRAY, |(_, col)| *col),
            Coloring::Continuous { min, max, ramp, log } => ramp.colour(Coloring::position(*min, *max, *log, v)),
        }
    }
}

/// The colours of a colouring as tables: the classes 0 to 255 (the usual ones) and the continuous range in 256
/// steps, so that a map is coloured without a search per cell.
struct Lut {
    classes: Option<Box<[Color32; 256]>>,
    ramp: Option<(f64, f64, bool, Box<[Color32; 256]>)>,
}

impl Lut {
    fn new(coloring: &Coloring) -> Lut {
        match coloring {
            Coloring::Classes(_) => {
                let mut t = Box::new([Color32::GRAY; 256]);
                for (i, c) in t.iter_mut().enumerate() {
                    *c = coloring.colour(i as f64);
                }
                Lut { classes: Some(t), ramp: None }
            }
            Coloring::Continuous { min, max, ramp, log } => {
                let mut t = Box::new([Color32::BLACK; 256]);
                for (i, c) in t.iter_mut().enumerate() {
                    *c = ramp.colour(i as f32 / 255.0);
                }
                Lut { classes: None, ramp: Some((*min, *max, *log, t)) }
            }
        }
    }

    #[inline]
    fn colour(&self, v: f64, coloring: &Coloring) -> Color32 {
        if v.is_nan() {
            return Color32::TRANSPARENT;
        }
        if let Some(t) = &self.classes {
            if (0.0..256.0).contains(&v) && v.fract() == 0.0 {
                return t[v as usize];
            }
            return coloring.colour(v);
        }
        if let Some((min, max, log, t)) = &self.ramp {
            let f = Coloring::position(*min, *max, *log, v);
            return t[(f * 255.0).round().clamp(0.0, 255.0) as usize];
        }
        coloring.colour(v)
    }
}

/// The image of values (f32, row by row) at one pixel each.
pub fn image_f32(values: &[f32], width: usize, height: usize, coloring: &Coloring) -> ColorImage {
    let lut = Lut::new(coloring);
    let pixels = values[..width * height].iter().map(|&v| lut.colour(v as f64, coloring)).collect();
    ColorImage::new([width, height], pixels)
}

/// The image of a map of `width` x `height` cells, one pixel per `step` x `step` cells (the cell at the
/// block's corner) so that neither side passes `max_side`; returns the image and `step`.
pub fn image(values: &[f64], width: usize, height: usize, max_side: usize, coloring: &Coloring) -> (ColorImage, usize) {
    let step = width.max(height).div_ceil(max_side.max(1)).max(1);
    let (w, h) = (width.div_ceil(step), height.div_ceil(step));
    let lut = Lut::new(coloring);
    let mut pixels = Vec::with_capacity(w * h);
    for r in 0..h {
        let row = &values[(r * step) * width..];
        pixels.extend((0..w).map(|c| lut.colour(row[c * step], coloring)));
    }
    (ColorImage::new([w, h], pixels), step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_or_continuous() {
        assert!(matches!(Coloring::for_values(&[1.0, 2.0, f64::NAN, 2.0]), Coloring::Classes(c) if c.len() == 2));
        // the 2nd and 98th percentiles
        assert!(matches!(
            Coloring::for_values(&[0.5, 2.0]),
            Coloring::Continuous { min, max, ramp: Ramp::Viridis, log: false } if (min - 0.53).abs() < 1e-9 && (max - 1.97).abs() < 1e-9
        ));
        // on both sides of 0: centred on it
        assert!(matches!(
            Coloring::for_values(&[-0.5, 2.0, 0.25]),
            Coloring::Continuous { min, max, ramp: Ramp::Diverging, .. } if (min + 1.94).abs() < 1e-9 && max == -min
        ));
        // an outlier does not take the ramp
        let mut v: Vec<f64> = (0..1000).map(|i| f64::from(i) / 10.0 + 0.05).collect();
        v.push(1e9);
        let Coloring::Continuous { max, .. } = Coloring::for_values(&v) else { panic!() };
        assert!(max < 100.0);
        // logarithms: the percentiles of the values above 0, and their positions
        let Coloring::Continuous { min, max, log: true, .. } =
            Coloring::continuous(&[0.0, 1.0, 10.0, 100.0, 1000.0], Ramp::Viridis, true)
        else {
            panic!()
        };
        assert!((Coloring::position(min, max, true, 100.0) - 0.68).abs() < 0.05, "{min} {max}");
        assert_eq!(Coloring::position(min, max, true, -1.0), 0.0);
        // a little below 0 only: not
        assert!(matches!(Coloring::for_values(&[-0.1, 2.0, 0.5]), Coloring::Continuous { ramp: Ramp::Viridis, .. }));
        let many: Vec<f64> = (0..100).map(f64::from).collect();
        assert!(matches!(Coloring::for_values(&many), Coloring::Continuous { .. }));
    }

    #[test]
    fn images_fit_the_screen() {
        let v: Vec<f64> = (0..100 * 50).map(|i| (i % 3) as f64).collect();
        let c = Coloring::for_values(&v);
        let (img, step) = image(&v, 100, 50, 32, &c);
        assert_eq!(step, 4);
        assert_eq!(img.size, [25, 13]);
        let (full, step) = image(&v, 100, 50, 4096, &c);
        assert_eq!((step, full.size), (1, [100, 50]));
        assert_eq!(full.pixels[1], c.colour(1.0));
        assert_eq!(c.colour(f64::NAN), Color32::TRANSPARENT);
        let r = Coloring::Continuous { min: 0.0, max: 10.0, ramp: Ramp::Viridis, log: false };
        let (img, _) = image(&[0.0, 10.0, 20.0, f64::NAN], 4, 1, 4096, &r);
        assert_eq!(img.pixels, vec![sequential(0.0), sequential(1.0), sequential(1.0), Color32::TRANSPARENT]);
        let odd = Coloring::for_values(&[-3.0, 300.0]);
        let (img, _) = image(&[-3.0, 300.0, 7.0], 3, 1, 4096, &odd);
        assert_eq!(img.pixels, vec![class_colour(0), class_colour(1), Color32::GRAY]);
    }

    #[test]
    fn shared_colours() {
        let base = Coloring::for_values(&[1.0, 2.0, 3.0]);
        let Coloring::Classes(c) = Coloring::for_values(&[2.0, 9.0]).like(&base) else { panic!() };
        assert_eq!(c, vec![(2.0, class_colour(1)), (9.0, class_colour(3))]);
        assert_ne!(base.key(), Coloring::for_values(&[1.0, 2.0]).key());
    }

    #[test]
    fn sequential_ends() {
        assert_eq!(sequential(0.0), Color32::from_rgb(68, 1, 84));
        assert_eq!(sequential(1.0), Color32::from_rgb(253, 231, 37));
    }
}
