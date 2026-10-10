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
    let t = if t.is_nan() { 0.0 } else { t.clamp(0.0, 1.0) } * (STOPS.len() - 1) as f32;
    let i = (t.floor() as usize).min(STOPS.len() - 2);
    let f = t - i as f32;
    let c: Vec<u8> = (0..3).map(|k| (STOPS[i][k] + f * (STOPS[i + 1][k] - STOPS[i][k])).round() as u8).collect();
    Color32::from_rgb(c[0], c[1], c[2])
}

/// How a map is coloured.
#[derive(Debug, Clone, PartialEq)]
pub enum Coloring {
    /// A colour per class (classes not listed are grey).
    Classes(Vec<(f64, Color32)>),
    /// From `min` to `max` along the sequential palette.
    Continuous { min: f64, max: f64 },
}

impl Coloring {
    /// Classes coloured in order, for a map of whole values with few of them; else continuous.
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
            Coloring::Classes(classes.into_iter().enumerate().map(|(i, c)| (c, class_colour(i))).collect())
        } else {
            Coloring::Continuous {
                min: if lo.is_finite() { lo } else { 0.0 },
                max: if hi.is_finite() { hi } else { 1.0 },
            }
        }
    }

    pub fn colour(&self, v: f64) -> Color32 {
        if v.is_nan() {
            return Color32::TRANSPARENT;
        }
        match self {
            Coloring::Classes(c) => c.iter().find(|(k, _)| *k == v).map_or(Color32::GRAY, |(_, col)| *col),
            Coloring::Continuous { min, max } => {
                let t = if max > min { ((v - min) / (max - min)) as f32 } else { 0.5 };
                sequential(t)
            }
        }
    }
}

/// The image of a map of `width` x `height` cells, one pixel per `step` x `step` cells (the cell at the
/// block's corner) so that neither side passes `max_side`; returns the image and `step`.
pub fn image(values: &[f64], width: usize, height: usize, max_side: usize, coloring: &Coloring) -> (ColorImage, usize) {
    let step = width.max(height).div_ceil(max_side.max(1)).max(1);
    let (w, h) = (width.div_ceil(step), height.div_ceil(step));
    let mut pixels = Vec::with_capacity(w * h);
    for r in 0..h {
        for c in 0..w {
            pixels.push(coloring.colour(values[(r * step) * width + c * step]));
        }
    }
    (ColorImage::new([w, h], pixels), step)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_or_continuous() {
        assert!(matches!(Coloring::for_values(&[1.0, 2.0, f64::NAN, 2.0]), Coloring::Classes(c) if c.len() == 2));
        assert!(
            matches!(Coloring::for_values(&[0.5, 2.0]), Coloring::Continuous { min, max } if min == 0.5 && max == 2.0)
        );
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
    }

    #[test]
    fn sequential_ends() {
        assert_eq!(sequential(0.0), Color32::from_rgb(68, 1, 84));
        assert_eq!(sequential(1.0), Color32::from_rgb(253, 231, 37));
    }
}
