//! A layer shown as a colour composite of three of its maps: the red, green and blue channels each a band of a
//! stack (a series' step, a band of a raster on disk), as natural colour or false colour. Each channel runs from
//! its range's least value (black) to its most (the full colour), through a gamma; a cell without a value in any of
//! the three is left transparent. The tiles are made by the Viewer's threads like any map's ([`RgbSource`]: its
//! values are the three channels packed in one number, see [`pack`]); the ranges by the 2nd to 98th percentiles of
//! each band are worked out by a thread from a sample ([`Ranges`]), never by the window.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use crate::map_view::{Source, SourceKey};

/// How a layer is drawn as colour: the map of each channel, the range of its values chosen by hand (none: the 2nd
/// to 98th percentiles of the band), and the gamma of the three (1: none; above 1 brighter).
#[derive(Debug, Clone, PartialEq)]
pub struct Rgb {
    pub bands: [usize; 3],
    pub ranges: [Option<(f64, f64)>; 3],
    pub gamma: f32,
}

/// The names of the channels.
pub const CHANNELS: [&str; 3] = ["R", "G", "B"];

/// The gammas the slider gives.
pub const GAMMAS: std::ops::RangeInclusive<f32> = 0.2..=3.0;

impl Rgb {
    pub fn new(bands: [usize; 3]) -> Rgb {
        Rgb { bands, ranges: [None; 3], gamma: 1.0 }
    }

    /// The composite a layer gets when it is first switched on: the natural colour of the bands' names when they
    /// say it, else the first three maps.
    pub fn first(names: &[String]) -> Rgb {
        let found = presets(names).into_iter().find(|(label, _)| label.starts_with("natural"));
        Rgb::new(found.map_or([0, 1, 2], |(_, b)| b))
    }

    /// As a Plot keeps it.
    pub fn to_plot(&self) -> terraflux_graph::PlotRgb {
        let r = |i: usize| self.ranges[i].map(|(a, b)| [a, b]);
        terraflux_graph::PlotRgb {
            bands: self.bands,
            red: r(0),
            green: r(1),
            blue: r(2),
            gamma: (self.gamma != 1.0).then_some(f64::from(self.gamma)),
        }
    }

    /// From what a Plot keeps.
    pub fn of_plot(p: &terraflux_graph::PlotRgb) -> Rgb {
        let r = |v: Option<[f64; 2]>| v.map(|[a, b]| (a, b));
        Rgb {
            bands: p.bands,
            ranges: [r(p.red), r(p.green), r(p.blue)],
            gamma: p.gamma.map_or(1.0, |g| (g as f32).clamp(*GAMMAS.start(), *GAMMAS.end())),
        }
    }

    /// Whether `maps` can be drawn as colour: three or more, of one size.
    pub fn applicable(maps: &[Source]) -> bool {
        maps.len() >= 3 && maps.iter().all(|m| m.size() == maps[0].size())
    }

    /// The map of each channel among `maps` (the last when there are fewer).
    pub fn maps<'a>(&self, maps: &'a [Source]) -> Option<[&'a Source; 3]> {
        let last = maps.len().checked_sub(1)?;
        Some([&maps[self.bands[0].min(last)], &maps[self.bands[1].min(last)], &maps[self.bands[2].min(last)]])
    }

    /// [`Rgb::source`] with the ranges worked out here, not by a thread: for a file written without a window.
    pub fn source_now(&self, maps: &[Source], names: &[String]) -> Option<Source> {
        if !Rgb::applicable(maps) {
            return None;
        }
        let m = self.maps(maps)?;
        let range = |i: usize| self.ranges[i].unwrap_or_else(|| percentiles(m[i]));
        Some(Source::Rgb(Arc::new(RgbSource {
            bands: [m[0].clone(), m[1].clone(), m[2].clone()],
            names: self.names(names, maps.len()),
            ranges: [range(0), range(1), range(2)],
            gamma: self.gamma,
        })))
    }

    /// The names of the three bands among `names` of `count` maps ("band 4" when the maps have none).
    pub fn names(&self, names: &[String], count: usize) -> [String; 3] {
        let name = |i: usize| {
            let b = self.bands[i].min(count.saturating_sub(1));
            names.get(b).cloned().unwrap_or_else(|| format!("band {}", b + 1))
        };
        [name(0), name(1), name(2)]
    }

    /// The range each channel is drawn over: the one chosen, else the band's percentiles; none while those are
    /// worked out.
    pub fn used(&self, ctx: &egui::Context, cache: &Ranges, maps: &[Source]) -> Option<[(f64, f64); 3]> {
        let m = self.maps(maps)?;
        let mut out = [(0.0, 1.0); 3];
        let mut ready = true;
        for i in 0..3 {
            match self.ranges[i] {
                Some(r) => out[i] = r,
                None => match cache.get(ctx, m[i]) {
                    Some(r) => out[i] = r,
                    None => ready = false,
                },
            }
        }
        ready.then_some(out)
    }

    /// The source it is drawn from: the three maps through the ranges; none while the ranges are worked out, or
    /// when the maps cannot make one.
    pub fn source(&self, ctx: &egui::Context, cache: &Ranges, maps: &[Source], names: &[String]) -> Option<Source> {
        if !Rgb::applicable(maps) {
            return None;
        }
        let ranges = self.used(ctx, cache, maps)?;
        let m = self.maps(maps)?;
        Some(Source::Rgb(Arc::new(RgbSource {
            bands: [m[0].clone(), m[1].clone(), m[2].clone()],
            names: self.names(names, maps.len()),
            ranges,
            gamma: self.gamma,
        })))
    }
}

/// A channel's value as 0 to 255: its place in `lo` to `hi` (out of them clamped; the middle when they are one
/// value), raised to `1 / gamma`; none for a cell without a value.
pub fn channel(v: f64, lo: f64, hi: f64, gamma: f32) -> Option<u8> {
    if !v.is_finite() {
        return None;
    }
    let t = if hi > lo { ((v - lo) / (hi - lo)).clamp(0.0, 1.0) } else { 0.5 };
    let g = if gamma.is_finite() && gamma > 0.0 { gamma } else { 1.0 };
    Some((t.powf(1.0 / f64::from(g)) * 255.0).round().clamp(0.0, 255.0) as u8)
}

/// The three channels in one number (every integer up to 2^24 is an f32): what a tile of an [`RgbSource`] holds.
pub fn pack(r: u8, g: u8, b: u8) -> f32 {
    ((u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)) as f32
}

/// The channels of a packed colour; none for NaN (a cell without a value).
pub fn unpack(v: f32) -> Option<[u8; 3]> {
    if v.is_nan() {
        return None;
    }
    let u = v as u32;
    Some([(u >> 16) as u8, (u >> 8) as u8, u as u8])
}

/// Three maps drawn as colour: their ranges and gamma (see [`Rgb`]) and the names of their bands.
#[derive(Clone)]
pub struct RgbSource {
    pub bands: [Source; 3],
    pub names: [String; 3],
    pub ranges: [(f64, f64); 3],
    pub gamma: f32,
}

impl RgbSource {
    /// What tells it from the others: the maps, ranges and gamma it is of.
    pub fn key(&self) -> SourceKey {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        for b in &self.bands {
            b.key().hash(&mut h);
        }
        for (a, b) in self.ranges {
            a.to_bits().hash(&mut h);
            b.to_bits().hash(&mut h);
        }
        self.gamma.to_bits().hash(&mut h);
        (h.finish() as usize, usize::MAX - 1)
    }

    /// The packed colours of three tiles (or rows) of the bands' values.
    pub fn pack_values(&self, v: [&[f32]; 3]) -> Vec<f32> {
        let ch = |i: usize, k: usize| channel(f64::from(v[i][k]), self.ranges[i].0, self.ranges[i].1, self.gamma);
        (0..v[0].len().min(v[1].len()).min(v[2].len()))
            .map(|k| match (ch(0, k), ch(1, k), ch(2, k)) {
                (Some(r), Some(g), Some(b)) => pack(r, g, b),
                _ => f32::NAN,
            })
            .collect()
    }

    /// A cell's values in the three bands (NaN where it has none).
    pub fn cell(&self, x: usize, y: usize) -> [f64; 3] {
        let v = crate::builder::series_at(&self.bands, y, x);
        [v[0], v[1], v[2]]
    }

    /// What a cell says: each channel's band and value ("R B4 0.12   G B3 0.1   B B2 0.05").
    pub fn text(&self, v: [f64; 3]) -> String {
        let said = |v: f64| if v.is_nan() { "no value".to_string() } else { terraflux_ops::value::class_label(v) };
        (0..3).map(|i| format!("{} {} {}", CHANNELS[i], self.names[i], said(v[i]))).collect::<Vec<_>>().join("   ")
    }

    /// The legend's lines: a channel, its band and the range it is drawn over ("R B4 0.01–0.18"), with the
    /// colour of the full channel.
    pub fn legend(&self) -> [(egui::Color32, String); 3] {
        legend(&self.names, &self.ranges)
    }
}

/// The lines of an RGB legend for the bands `names` over `ranges`.
pub fn legend(names: &[String; 3], ranges: &[(f64, f64); 3]) -> [(egui::Color32, String); 3] {
    let text = |i: usize| {
        let (a, b) = ranges[i];
        format!("{} {} {}–{}", CHANNELS[i], names[i], crate::map_view::cell_text(a), crate::map_view::cell_text(b))
    };
    [
        (egui::Color32::from_rgb(230, 40, 40), text(0)),
        (egui::Color32::from_rgb(40, 190, 60), text(1)),
        (egui::Color32::from_rgb(60, 100, 240), text(2)),
    ]
}

/// A band's number as its name says it: "B4", "SR_B4", "B04", "band 4", "S2_B8A_10m" are 4, 4, 4, 4 and 8A; none
/// when no word of the name is one.
fn band_id(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .find_map(|w| {
            let rest = w.strip_prefix("band").or_else(|| w.strip_prefix('b'))?;
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let tail = &rest[digits.len()..];
            if digits.is_empty() || !(tail.is_empty() || tail == "a") {
                return None;
            }
            Some(format!("{}{tail}", digits.trim_start_matches('0')))
        })
        .or_else(|| {
            // ("band 4": the word "band" and the number apart)
            let words: Vec<&str> =
                lower.split(|c: char| !c.is_ascii_alphanumeric()).filter(|w| !w.is_empty()).collect();
            words
                .windows(2)
                .find(|w| w[0] == "band" && w[1].chars().all(|c| c.is_ascii_digit()))
                .map(|w| w[1].trim_start_matches('0').to_string())
        })
}

/// The composites a stack's band names offer: natural colour, colour infrared and short-wave infrared of Sentinel-2
/// and Landsat 8/9 when the bands they need are there (by their names, whatever the prefix or suffix: `B4`,
/// `SR_B4`, `B04`), then the first three maps. Each: its label and the maps of the red, green and blue channels.
pub fn presets(names: &[String]) -> Vec<(String, [usize; 3])> {
    const SETS: [(&str, [&str; 3]); 7] = [
        ("natural colour", ["4", "3", "2"]),
        ("colour infrared, Sentinel-2", ["8", "4", "3"]),
        ("colour infrared, Landsat 8/9", ["5", "4", "3"]),
        ("short-wave infrared, Sentinel-2", ["11", "8", "4"]),
        ("short-wave infrared, Sentinel-2", ["12", "8a", "4"]),
        ("short-wave infrared, Landsat 8/9", ["6", "5", "4"]),
        ("short-wave infrared, Landsat 8/9", ["7", "5", "4"]),
    ];
    let ids: Vec<Option<String>> = names.iter().map(|n| band_id(n)).collect();
    let find = |id: &str| ids.iter().position(|x| x.as_deref() == Some(id));
    // A full Sentinel-2 stack has a B5 and a B6 too (red edge), and Landsat a B8 and a B11 (panchromatic, thermal):
    // Sentinel-2's B8A and B12, or Landsat's B10 and `SR_`/`ST_` names, tell one from the other.
    let landsat = find("10").is_some() || names.iter().any(|n| n.to_lowercase().contains("sr_b"));
    let sentinel = !landsat && (find("8a").is_some() || find("12").is_some());
    let mut out: Vec<(String, [usize; 3])> = Vec::new();
    for (label, want) in SETS {
        if (sentinel && label.contains("Landsat")) || (landsat && label.contains("Sentinel")) {
            continue;
        }
        if let (Some(r), Some(g), Some(b)) = (find(want[0]), find(want[1]), find(want[2])) {
            let bands: Vec<String> = want.iter().map(|w| format!("B{}", w.to_uppercase())).collect();
            out.push((format!("{label} ({})", bands.join(" ")), [r, g, b]));
        }
    }
    if names.len() >= 3 {
        out.push(("first three".to_string(), [0, 1, 2]));
    }
    out
}

/// What the 2nd to 98th percentiles of the bands are, worked out by threads from a sample of each (as the looks of
/// the maps are) and kept by the band: asked for again and again by the frames, it costs nothing once it is there.
#[derive(Default)]
pub struct Ranges(Arc<Mutex<Worked>>);

#[derive(Default)]
struct Worked {
    /// The bands done (held, so that a key stays its own): their ranges.
    done: HashMap<SourceKey, (Source, (f64, f64))>,
    asked: HashSet<SourceKey>,
}

/// The cells a range is worked out from along the longest side (a sample of the whole, as the looks').
const SAMPLE: usize = 1024;

/// The 2nd to 98th percentiles of a map's values (of a sample when it is large): the whole span when they are one
/// value; 0 to 1 when it has no value.
pub fn percentiles(src: &Source) -> (f64, f64) {
    let values: Vec<f64> = match src {
        Source::Map(m) => m.data.iter().step_by(m.data.len().div_ceil(SAMPLE * SAMPLE).max(1)).copied().collect(),
        Source::Rasters(r, l) => {
            let (w, h) = (r.grid_of(*l).width, r.grid_of(*l).height);
            let step = w.max(h).div_ceil(SAMPLE).max(1);
            let win = terraflux_raster::tiff::Window { x0: 0, y0: 0, width: w, height: h };
            r.sample(*l, win, step).map(|v| v.into_iter().map(f64::from).collect()).unwrap_or_default()
        }
        Source::Rgb(r) => return percentiles(&r.bands[0]),
        Source::Blank(_) => Vec::new(),
    };
    let (lo, hi) = values
        .iter()
        .filter(|v| v.is_finite())
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &v| (a.min(v), b.max(v)));
    if !lo.is_finite() || !hi.is_finite() {
        return (0.0, 1.0);
    }
    match crate::palette::percentiles_2_98(&values) {
        Some((a, b)) if b > a => (a, b),
        _ => (lo, hi),
    }
}

impl Ranges {
    /// A band's range, or none while a thread works it out (the view drawn again when it is there).
    pub fn get(&self, ctx: &egui::Context, src: &Source) -> Option<(f64, f64)> {
        let key = src.key();
        let mut w = self.0.lock().unwrap();
        if let Some((_, r)) = w.done.get(&key) {
            return Some(*r);
        }
        if w.asked.insert(key) {
            let (shared, src, ctx) = (self.0.clone(), src.clone(), ctx.clone());
            let spawned = std::thread::Builder::new().name("terraflux-rgb".into()).spawn(move || {
                let r = percentiles(&src);
                let mut w = shared.lock().unwrap();
                // (a few kept: those of the bands shown now are asked for again at once)
                if w.done.len() > 96 {
                    w.done.clear();
                }
                w.asked.remove(&key);
                w.done.insert(key, (src, r));
                ctx.request_repaint();
            });
            if spawned.is_err() {
                w.asked.remove(&key);
            }
        }
        None
    }

    /// A band's range if it is worked out already.
    pub fn peek(&self, src: &Source) -> Option<(f64, f64)> {
        self.0.lock().unwrap().done.get(&src.key()).map(|(_, r)| *r)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Map;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn labels(v: &[String]) -> Vec<(String, [usize; 3])> {
        presets(v)
    }

    #[test]
    fn presets_read_the_bands_of_sentinel_2() {
        let n = names(&["B2", "B3", "B4", "B8", "B8A", "B11", "B12"]);
        let p = labels(&n);
        let by = |l: &str| p.iter().find(|(x, _)| x.starts_with(l)).map(|(_, b)| *b);
        // (the red channel is B4, the third band)
        assert_eq!(by("natural colour"), Some([2, 1, 0]));
        assert_eq!(by("colour infrared, Sentinel-2"), Some([3, 2, 1]));
        assert_eq!(p.iter().filter(|(x, _)| x.starts_with("short-wave infrared, Sentinel-2")).count(), 2);
        assert!(p.contains(&("short-wave infrared, Sentinel-2 (B11 B8 B4)".to_string(), [5, 3, 2])));
        assert!(p.contains(&("short-wave infrared, Sentinel-2 (B12 B8A B4)".to_string(), [6, 4, 2])));
        // (no Landsat's B5 and B6 here)
        assert!(!p.iter().any(|(x, _)| x.contains("Landsat")));
        assert_eq!(p.last().unwrap(), &("first three".to_string(), [0, 1, 2]));
    }

    #[test]
    fn presets_read_the_bands_of_landsat_with_prefixes_and_suffixes() {
        let n = names(&["LC08_SR_B1.tif", "sr_b2", "SR_B3", "SR_B4", "SR_B5", "sr_b6", "SR_B7"]);
        let p = labels(&n);
        assert_eq!(p[0], ("natural colour (B4 B3 B2)".to_string(), [3, 2, 1]));
        assert!(p.contains(&("colour infrared, Landsat 8/9 (B5 B4 B3)".to_string(), [4, 3, 2])));
        assert!(p.contains(&("short-wave infrared, Landsat 8/9 (B6 B5 B4)".to_string(), [5, 4, 3])));
        assert!(p.contains(&("short-wave infrared, Landsat 8/9 (B7 B5 B4)".to_string(), [6, 4, 3])));
        // zeros in front, any case, "band 4"
        let z = labels(&names(&["S2_b02_10m", "S2_B03_10m", "S2_B04_10m"]));
        assert_eq!(z[0].1, [2, 1, 0]);
        let w = labels(&names(&["band 2", "Band 3", "band4"]));
        assert_eq!(w[0].1, [2, 1, 0]);
    }

    #[test]
    fn a_whole_stack_of_one_sensor_offers_only_its_composites() {
        let s2: Vec<String> = ["B1", "B2", "B3", "B4", "B5", "B6", "B7", "B8", "B8A", "B9", "B11", "B12"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let p = labels(&s2);
        assert!(!p.iter().any(|(x, _)| x.contains("Landsat")), "{p:?}");
        assert!(p.iter().any(|(x, _)| x.contains("colour infrared, Sentinel-2")));
        let l8: Vec<String> = (1..=11).map(|i| format!("B{i}")).collect();
        let p = labels(&l8);
        assert!(!p.iter().any(|(x, _)| x.contains("Sentinel")), "{p:?}");
        assert!(p.iter().any(|(x, _)| x.contains("colour infrared, Landsat 8/9")));
    }

    #[test]
    fn presets_without_band_names_give_the_first_three() {
        assert_eq!(labels(&names(&["1997", "2011", "2020", "2021"])), [("first three".to_string(), [0, 1, 2])]);
        assert!(labels(&names(&["a", "b"])).is_empty());
        // the first switch-on: the natural colour when named, else the first three
        assert_eq!(Rgb::first(&names(&["B2", "B3", "B4"])).bands, [2, 1, 0]);
        assert_eq!(Rgb::first(&names(&["x", "y", "z", "w"])).bands, [0, 1, 2]);
        assert_eq!(Rgb::first(&names(&["B3", "B4", "B8"])).bands, [0, 1, 2]);
    }

    #[test]
    fn a_channel_is_the_value_in_its_range_through_the_gamma() {
        assert_eq!(channel(0.0, 0.0, 1.0, 1.0), Some(0));
        assert_eq!(channel(1.0, 0.0, 1.0, 1.0), Some(255));
        assert_eq!(channel(0.5, 0.0, 1.0, 1.0), Some(128));
        // out of the range: clamped
        assert_eq!(channel(-3.0, 0.0, 1.0, 1.0), Some(0));
        assert_eq!(channel(9.0, 0.0, 1.0, 1.0), Some(255));
        // gamma 2: the square root, brighter; gamma 0.5: the square, darker
        assert_eq!(channel(0.25, 0.0, 1.0, 2.0), Some(128));
        assert_eq!(channel(0.5, 0.0, 1.0, 0.5), Some(64));
        // a range of 100 to 200
        assert_eq!(channel(150.0, 100.0, 200.0, 1.0), Some(128));
        // no value, and a range of one value
        assert_eq!(channel(f64::NAN, 0.0, 1.0, 1.0), None);
        assert_eq!(channel(5.0, 3.0, 3.0, 1.0), Some(128));
    }

    #[test]
    fn colours_are_packed_in_a_number_that_keeps_them() {
        for c in [[0, 0, 0], [255, 255, 255], [1, 2, 3], [255, 0, 128], [17, 200, 99]] {
            assert_eq!(unpack(pack(c[0], c[1], c[2])), Some(c));
        }
        assert_eq!(unpack(f32::NAN), None);
        let src = |v: f64| Source::Map(Map::new(Arc::new(terraflux_raster::Grid::new(2, 1)), vec![v, f64::NAN]));
        let rgb = RgbSource {
            bands: [src(0.0), src(0.5), src(1.0)],
            names: Default::default(),
            ranges: [(0.0, 1.0); 3],
            gamma: 1.0,
        };
        let v = rgb.pack_values([&[0.0, 0.0], &[0.5, 0.5], &[1.0, f32::NAN]]);
        assert_eq!(unpack(v[0]), Some([0, 128, 255]));
        // (a cell without a value in any of the three: transparent)
        assert!(v[1].is_nan());
    }

    #[test]
    fn the_legend_tells_the_bands_and_the_ranges() {
        let l = legend(&names(&["B4", "B3", "B2"]).try_into().unwrap(), &[(0.01, 0.18), (0.0, 1.0), (-5.0, 5.0)]);
        assert_eq!(l[0].1, "R B4 0.01–0.18");
        assert_eq!(l[1].1, "G B3 0–1");
        assert_eq!(l[2].1, "B B2 -5–5");
    }

    #[test]
    fn a_plot_keeps_the_composite_and_an_old_file_has_none() {
        let rgb = Rgb { bands: [2, 1, 0], ranges: [Some((0.0, 0.3)), None, Some((1.0, 9.0))], gamma: 1.5 };
        assert_eq!(Rgb::of_plot(&rgb.to_plot()), rgb);
        // (the defaults are not written)
        let plain = Rgb::new([0, 1, 2]).to_plot();
        assert_eq!((plain.red, plain.gamma), (None, None));
        let mut m = terraflux_graph::Model::new("colour");
        m.plots.push(terraflux_graph::Plot {
            id: "colour".into(),
            layers: vec![terraflux_graph::PlotLayer {
                rgb: Some(rgb.to_plot()),
                opacity: Some(0.5),
                hidden: true,
                ..terraflux_graph::PlotLayer::new("stack.map")
            }],
            ..Default::default()
        });
        let text = m.to_toml().unwrap();
        let back = terraflux_graph::Model::from_toml(&text).unwrap();
        let kept = back.plots[0].layers[0].rgb.clone().unwrap();
        assert_eq!(Rgb::of_plot(&kept), rgb, "{text}");
        assert_eq!(back.plots[0].layers[0].opacity, Some(0.5));
        // an old file: a layer without it
        let mut old = terraflux_graph::Model::new("old");
        old.plots = vec![terraflux_graph::Plot {
            id: "p".into(),
            layers: vec![terraflux_graph::PlotLayer::new("stack.map")],
            ..Default::default()
        }];
        let text = old.to_toml().unwrap();
        assert!(!text.contains("rgb"), "{text}");
        let back = terraflux_graph::Model::from_toml(&text).unwrap();
        assert!(back.plots[0].layers[0].rgb.is_none());
    }
}
