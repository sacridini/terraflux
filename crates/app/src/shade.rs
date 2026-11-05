//! A map of values (an elevation) shaded with its own relief: a hillshade made of the map, tile by tile by the
//! Viewer's threads, drawn over the layer's colours as a shade that darkens them,
//! `colour * (1 - strength + strength * shade)`, so that the heights show as light and shadow under the colours of
//! the values. The cells without a value stay transparent.
//!
//! The shade is Horn's, as `gdaldem hillshade` makes it: the slope of a cell from its eight neighbours, the sun's
//! azimuth (degrees clockwise from north) and altitude over the horizon, the heights exaggerated by `z`; the cell is
//! as bright as the surface faces the sun (a flat cell is `sin(altitude)`). A grid in degrees is taken at the
//! metres its cells span at its middle latitude, so that `z` of 1 is the true relief.
//!
//! A tile of the shade needs the cells around it: it is made from a window one cell larger on every side
//! ([`crate::map_view::Source::halo`]), so that the tiles join without a seam. A coarser tile is the shade of the
//! cells it samples (its cells are `2^level` times as wide).

use std::sync::Arc;

use crate::map_view::{Source, SourceKey, TILE};

/// The sun and the exaggeration of a shade, and how strong it is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shade {
    /// Degrees clockwise from north.
    pub azimuth: f32,
    /// Degrees over the horizon.
    pub altitude: f32,
    /// The vertical exaggeration.
    pub z: f32,
    /// How much the shade darkens the colours, 0 (not at all) to 1 (the shade alone).
    pub strength: f32,
}

impl Default for Shade {
    fn default() -> Shade {
        Shade { azimuth: 315.0, altitude: 45.0, z: 1.0, strength: 0.6 }
    }
}

/// The exaggerations the control gives.
pub const EXAGGERATIONS: std::ops::RangeInclusive<f32> = 0.1..=50.0;

impl Shade {
    /// As a Plot keeps it (what is the default is left out).
    pub fn to_plot(&self) -> terraflux_graph::PlotShade {
        let d = Shade::default();
        let keep = |v: f32, d: f32| (v != d).then_some(f64::from(v));
        terraflux_graph::PlotShade {
            azimuth: keep(self.azimuth, d.azimuth),
            altitude: keep(self.altitude, d.altitude),
            z: keep(self.z, d.z),
            strength: keep(self.strength, d.strength),
        }
    }

    /// From what a Plot keeps (the values brought into their ranges).
    pub fn of_plot(p: &terraflux_graph::PlotShade) -> Shade {
        let d = Shade::default();
        let get = |v: Option<f64>, d: f32| v.filter(|v| v.is_finite()).map_or(d, |v| v as f32);
        Shade {
            azimuth: get(p.azimuth, d.azimuth).rem_euclid(360.0),
            altitude: get(p.altitude, d.altitude).clamp(0.0, 90.0),
            z: get(p.z, d.z).clamp(*EXAGGERATIONS.start(), *EXAGGERATIONS.end()),
            strength: get(p.strength, d.strength).clamp(0.0, 1.0),
        }
    }

    /// The line of a legend: "shaded: 315°, 45°, ×1".
    pub fn label(&self) -> String {
        format!("shaded: {}°, {}°, ×{}", crate::map_view::cell_text(f64::from(self.azimuth)), self.altitude, self.z)
    }

    /// The sun, as the shade is worked out from it.
    pub fn sun(&self) -> Sun {
        let (az, alt) = (f64::from(self.azimuth).to_radians(), f64::from(self.altitude).to_radians());
        Sun { east: az.sin() * alt.cos(), north: az.cos() * alt.cos(), up: alt.sin(), z: f64::from(self.z) }
    }

    /// The colouring of the shade as a layer over the map: black, as opaque as the shade is dark (0 to 1 of the
    /// strength).
    pub fn coloring(&self) -> crate::palette::Coloring {
        crate::palette::Coloring::Shade { strength: self.strength }
    }
}

/// The direction of the sun (a unit vector: east, north, up) and the vertical exaggeration.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sun {
    pub east: f64,
    pub north: f64,
    pub up: f64,
    pub z: f64,
}

/// How bright a surface is, 0 to 1, from its slopes (rise per run, east and north): its normal against the sun.
#[inline]
pub fn bright(sun: &Sun, dx: f64, dy: f64) -> f32 {
    let (p, q) = (dx * sun.z, dy * sun.z);
    ((sun.up - p * sun.east - q * sun.north) / (1.0 + p * p + q * q).sqrt()).clamp(0.0, 1.0) as f32
}

/// The metres a grid's cells span, across and down: a geographic grid at its middle latitude (111 320 m a degree of
/// latitude, the longitude's shortened by the cosine), a projected one by its unit, a grid with none by its own units.
pub fn cell_metres(g: &terraflux_raster::Grid) -> (f64, f64) {
    let (dx, dy) = (g.transform[1].abs(), g.transform[5].abs());
    if !(dx > 0.0 && dy > 0.0) {
        return (1.0, 1.0);
    }
    let degrees = g.geo.is_geographic() == Some(true) || g.geo.epsg().is_some_and(|c| matches!(c, 4326 | 4269 | 4258));
    if degrees {
        let lat = g.transform[3] + g.transform[5] * (g.height as f64 / 2.0);
        let across = lat.to_radians().cos().abs().max(0.01);
        return (dx * 111_320.0 * across, dy * 111_320.0);
    }
    let unit = g.geo.linear_unit();
    (dx * unit, dy * unit)
}

/// The shade of the inside of a window of `w` x `h` cells (`(w - 2) * (h - 2)` cells, by rows): each cell from its
/// eight neighbours, `cell` metres wide and high. A cell without a value has none (NaN); a neighbour without one
/// counts as the cell's own height, as gdaldem does at a border.
pub fn shade_window(v: &[f32], w: usize, h: usize, sun: &Sun, cell: (f64, f64)) -> Vec<f32> {
    if w < 3 || h < 3 || v.len() < w * h {
        return Vec::new();
    }
    let mut out = Vec::with_capacity((w - 2) * (h - 2));
    let (ew, ns) = (8.0 * cell.0, 8.0 * cell.1);
    for r in 1..h - 1 {
        for c in 1..w - 1 {
            let e = v[r * w + c];
            if e.is_nan() {
                out.push(f32::NAN);
                continue;
            }
            let at = |dr: usize, dc: usize| {
                let x = v[(r + dr - 1) * w + c + dc - 1];
                f64::from(if x.is_nan() { e } else { x })
            };
            let (a, b, cc) = (at(0, 0), at(0, 1), at(0, 2));
            let (d, f) = (at(1, 0), at(1, 2));
            let (g, hh, i) = (at(2, 0), at(2, 1), at(2, 2));
            let dx = ((cc + 2.0 * f + i) - (a + 2.0 * d + g)) / ew;
            let dy = ((a + 2.0 * b + cc) - (g + 2.0 * hh + i)) / ns;
            out.push(bright(sun, dx, dy));
        }
    }
    out
}

/// The shade of a whole map of `w` x `h` cells (its borders as gdaldem's `-compute_edges`).
pub fn shade_all(v: &[f32], w: usize, h: usize, sun: &Sun, cell: (f64, f64)) -> Vec<f32> {
    let mut padded = vec![f32::NAN; (w + 2) * (h + 2)];
    for r in 0..h {
        padded[(r + 1) * (w + 2) + 1..(r + 1) * (w + 2) + 1 + w].copy_from_slice(&v[r * w..(r + 1) * w]);
    }
    shade_window(&padded, w + 2, h + 2, sun, cell)
}

/// The shade of a map, as a source of the Viewer: its tiles hold the brightness, 0 to 1 (NaN where the map has none).
#[derive(Clone)]
pub struct ShadeSource {
    pub base: Source,
    pub sun: Shade,
}

impl ShadeSource {
    /// A map's shade, when the map is one of values (not a colour composite or the blank grid).
    pub fn of(base: &Source, shade: &Shade) -> Option<Source> {
        match base {
            Source::Map(_) | Source::Rasters(..) => {
                Some(Source::Shade(Arc::new(ShadeSource { base: base.clone(), sun: *shade })))
            }
            _ => None,
        }
    }

    /// What tells it from the others: the map and the sun (not the strength, which is the colouring's).
    pub fn key(&self) -> SourceKey {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        self.base.key().hash(&mut h);
        [self.sun.azimuth, self.sun.altitude, self.sun.z].iter().for_each(|v| v.to_bits().hash(&mut h));
        (h.finish() as usize, usize::MAX - 2)
    }

    /// The metres of a cell sampled every `step` cells.
    fn cell(&self, step: usize) -> (f64, f64) {
        let (x, y) = cell_metres(self.base.grid());
        (x * step as f64, y * step as f64)
    }

    /// A tile's brightness: that of the cells the map's tile samples, from a window with a cell around it.
    pub fn tile(&self, level: u32, tx: usize, ty: usize) -> Result<Vec<f32>, String> {
        let win = self.base.halo(level, tx, ty, 1)?;
        Ok(shade_window(&win, TILE + 2, TILE + 2, &self.sun.sun(), self.cell(1usize << level)))
    }

    /// The brightness of `w` x `h` cells of the map read every `step` cells (see [`crate::animation::values`]).
    pub fn of_values(&self, v: &[f32], w: usize, h: usize, step: usize) -> Vec<f32> {
        shade_all(v, w, h, &self.sun.sun(), self.cell(step))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Map;

    fn map(w: usize, h: usize, f: impl Fn(usize, usize) -> f64) -> Source {
        let data = (0..w * h).map(|i| f(i % w, i / w)).collect();
        Source::Map(Map::new(Arc::new(terraflux_raster::Grid::new(w, h)), data))
    }

    fn grid_of(src: &Source) -> Vec<f32> {
        let (w, h) = src.size();
        let Source::Map(m) = src else { panic!("a map") };
        assert_eq!(m.data.len(), w * h);
        m.data.iter().map(|v| *v as f32).collect()
    }

    #[test]
    fn a_flat_cell_is_as_bright_as_the_sine_of_the_sun() {
        for alt in [10.0f32, 45.0, 80.0] {
            let sun = Shade { altitude: alt, ..Shade::default() }.sun();
            let flat = shade_all(&[7.0; 25], 5, 5, &sun, (1.0, 1.0));
            assert!(flat.iter().all(|v| (v - alt.to_radians().sin()).abs() < 1e-6), "{alt}: {flat:?}");
        }
    }

    #[test]
    fn a_plane_facing_the_sun_is_brighter_than_one_facing_away() {
        // (the sun in the north-west, 315 degrees: a plane that falls to the north-west faces it)
        let sun = Shade::default().sun();
        let plane = |sx: f64, sy: f64| {
            let v: Vec<f32> = (0..49).map(|i| (sx * (i % 7) as f64 + sy * (i / 7) as f64) as f32).collect();
            shade_all(&v, 7, 7, &sun, (1.0, 1.0))[24]
        };
        // columns grow east and rows grow south: a height that falls to the north-west grows to the south-east
        let toward = plane(1.0, 1.0);
        let away = plane(-1.0, -1.0);
        let flat = plane(0.0, 0.0);
        assert!(toward > flat && flat > away, "{toward} {flat} {away}");
        // (facing away enough: in shadow)
        assert_eq!(plane(-5.0, -5.0), 0.0);
        // a slope across the rays (it falls to the south-west) takes the sun's height over the length of its normal
        let across = plane(-1.0, 1.0);
        assert!((across - (sun.up / 3f64.sqrt()) as f32).abs() < 1e-5, "{across}");
    }

    #[test]
    fn the_exaggeration_makes_the_relief_stronger() {
        let v: Vec<f32> = (0..49).map(|i| ((i % 7) + (i / 7)) as f32).collect();
        let at = |z: f32| shade_all(&v, 7, 7, &Shade { z, ..Shade::default() }.sun(), (1.0, 1.0))[24];
        // (a slope facing the sun is brightest as it is; steeper or flatter, it is less bright)
        assert!(at(1.0) > at(3.0) && at(1.0) > at(0.1), "{} {} {}", at(0.1), at(1.0), at(3.0));
    }

    #[test]
    fn a_cell_without_a_value_has_no_shade_and_does_not_spoil_the_others() {
        let sun = Shade::default().sun();
        let mut v = vec![5.0f32; 25];
        v[12] = f32::NAN;
        let s = shade_all(&v, 5, 5, &sun, (1.0, 1.0));
        assert!(s[12].is_nan());
        // (a neighbour without a value counts as the cell's own height: the others stay flat)
        let flat = Shade::default().altitude.to_radians().sin();
        assert!(s.iter().enumerate().all(|(i, v)| i == 12 || (v - flat).abs() < 1e-6), "{s:?}");
    }

    #[test]
    fn a_grid_in_degrees_is_taken_in_metres() {
        let mut g = terraflux_raster::Grid::new(10, 10);
        g.transform = [10.0, 0.001, 0.0, 60.0, 0.0, -0.001];
        g.geo.wkt = Some("GEOGCS[\"WGS 84\"]".into());
        let (x, y) = cell_metres(&g);
        // 0.001 degrees of latitude is 111 m; of longitude at 60 degrees north, half as much
        assert!((y - 111.32).abs() < 0.01, "{y}");
        assert!((x / y - 0.5).abs() < 0.01, "{x} {y}");
        // a projected grid: by its own unit
        let mut p = terraflux_raster::Grid::new(10, 10);
        p.transform = [0.0, 30.0, 0.0, 0.0, 0.0, -30.0];
        assert_eq!(cell_metres(&p), (30.0, 30.0));
    }

    #[test]
    fn a_tile_has_no_seam_with_the_next() {
        // a rough surface over three tiles across and two down: the shade of each tile is the whole map's
        let (w, h) = (TILE * 2 + 40, TILE + 70);
        let rough = |x: usize, y: usize| {
            let (x, y) = (x as f64, y as f64);
            40.0 * (x / 17.0).sin() * (y / 23.0).cos() + 0.1 * x + 7.0 * ((x * y) / 311.0).sin()
        };
        let src = map(w, h, rough);
        let sh = Shade::default();
        let shade = ShadeSource::of(&src, &sh).unwrap();
        let Source::Shade(s) = &shade else { panic!("a shade") };
        let whole = shade_all(&grid_of(&src), w, h, &sh.sun(), s.cell(1));
        for ty in 0..2 {
            for tx in 0..3 {
                let t = shade.tile(0, tx, ty).unwrap();
                assert_eq!(t.len(), TILE * TILE);
                for i in 0..TILE {
                    for j in 0..TILE {
                        let (x, y) = (tx * TILE + j, ty * TILE + i);
                        let v = t[i * TILE + j];
                        if x >= w || y >= h {
                            assert!(v.is_nan(), "outside the map at {x},{y}");
                        } else {
                            assert_eq!(v, whole[y * w + x], "tile {tx},{ty} cell {x},{y}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn a_coarse_tile_is_the_shade_of_the_cells_it_samples() {
        let (w, h) = (TILE * 2, TILE * 2);
        let src = map(w, h, |x, y| ((x as f64) / 9.0).sin() * 30.0 + (y as f64) * 0.3);
        let shade = ShadeSource::of(&src, &Shade::default()).unwrap();
        let Source::Shade(s) = &shade else { panic!("a shade") };
        // level 1: every second cell, cells twice as wide
        let sub: Vec<f32> = (0..TILE)
            .flat_map(|i| (0..TILE).map(move |j| (i, j)))
            .map(|(i, j)| src_value(&src, j * 2, i * 2))
            .collect();
        let want = shade_all(&sub, TILE, TILE, &s.sun.sun(), s.cell(2));
        let got = shade.tile(1, 0, 0).unwrap();
        // (inside: the same; the tile's last row and column see the cells past it)
        for i in 0..TILE - 1 {
            for j in 0..TILE - 1 {
                assert!((got[i * TILE + j] - want[i * TILE + j]).abs() < 1e-6, "{i},{j}");
            }
        }
    }

    /// The shade of the Viewer is `gdaldem hillshade`'s (the operator `raster.terrain`): a surface of hills in a grid
    /// of 30 m cells, shaded by both, agrees inside the map to within the byte gdaldem writes (1, shadow, to 255).
    #[test]
    fn the_shade_is_the_one_of_gdaldem() {
        let (w, h) = (60, 50);
        let data: Vec<f64> = (0..w * h)
            .map(|i| {
                let (x, y) = ((i % w) as f64, (i / w) as f64);
                220.0 + 60.0 * (x / 9.0).sin() * (y / 7.0).cos() + 2.0 * x
            })
            .collect();
        let mut grid = terraflux_raster::Grid::new(w, h);
        grid.transform = [500_000.0, 30.0, 0.0, 4_000_000.0, 0.0, -30.0];
        let dir = std::env::temp_dir().join(format!("terraflux-shade-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (from, to) = (dir.join("dem.tif"), dir.join("shade.tif"));
        terraflux_raster::Raster::single(grid.clone(), data.clone(), terraflux_raster::DataType::F32, None)
            .write(&from)
            .unwrap();
        let args: Vec<String> = ["-compute_edges"].iter().map(|a| a.to_string()).collect();
        if let Err(e) = terraflux_gdal::dem(&from, &to, "hillshade", &args) {
            // (no GDAL library on this machine: nothing to compare with)
            eprintln!("no gdaldem here, the shade is not compared: {e}");
            let _ = std::fs::remove_dir_all(&dir);
            return;
        }
        let theirs = terraflux_raster::tiff::read(&to).unwrap().bands[0].data.clone();
        let src = Source::Map(Map::new(Arc::new(grid), data));
        let shade = ShadeSource::of(&src, &Shade::default()).unwrap();
        let mine = shade.tile(0, 0, 0).unwrap();
        let mut worst = 0.0f64;
        for y in 0..h {
            for x in 0..w {
                let (m, g) = (f64::from(mine[y * TILE + x]), theirs[y * w + x]);
                // (the map's border cells: gdaldem takes the cells past it as the edge's, this as the cell's own)
                if x == 0 || y == 0 || x == w - 1 || y == h - 1 {
                    continue;
                }
                // (gdaldem: 1 + 254 * the brightness, a byte)
                worst = worst.max((1.0 + 254.0 * m - g).abs());
            }
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(worst <= 1.0, "the largest difference from gdaldem's byte: {worst}");
    }

    fn src_value(src: &Source, x: usize, y: usize) -> f32 {
        let Source::Map(m) = src else { panic!("a map") };
        m.data[y * m.grid.width + x] as f32
    }

    #[test]
    fn a_plot_keeps_the_shade_and_an_old_file_has_none() {
        let sh = Shade { azimuth: 270.0, altitude: 30.0, z: 2.5, strength: 0.8 };
        assert_eq!(Shade::of_plot(&sh.to_plot()), sh);
        // (the defaults are not written)
        let plain = Shade::default().to_plot();
        assert_eq!(plain, terraflux_graph::PlotShade::default());
        // (values out of range are brought in)
        let bad = terraflux_graph::PlotShade {
            azimuth: Some(-45.0),
            altitude: Some(120.0),
            z: Some(0.0),
            strength: Some(3.0),
        };
        let b = Shade::of_plot(&bad);
        assert_eq!((b.azimuth, b.altitude, b.strength), (315.0, 90.0, 1.0));
        assert_eq!(b.z, *EXAGGERATIONS.start());
        let mut m = terraflux_graph::Model::new("relief");
        m.plots.push(terraflux_graph::Plot {
            id: "relief".into(),
            layers: vec![terraflux_graph::PlotLayer {
                shade: Some(sh.to_plot()),
                blend: Some("multiply".into()),
                ..terraflux_graph::PlotLayer::new("dem.map")
            }],
            ..Default::default()
        });
        let text = m.to_toml().unwrap();
        let back = terraflux_graph::Model::from_toml(&text).unwrap();
        let l = &back.plots[0].layers[0];
        assert_eq!(Shade::of_plot(l.shade.as_ref().unwrap()), sh, "{text}");
        assert_eq!(l.blend.as_deref(), Some("multiply"));
        // an old file: a layer without either
        let mut old = terraflux_graph::Model::new("old");
        old.plots = vec![terraflux_graph::Plot {
            id: "p".into(),
            layers: vec![terraflux_graph::PlotLayer::new("dem.map")],
            ..Default::default()
        }];
        let text = old.to_toml().unwrap();
        assert!(!text.contains("shade") && !text.contains("blend"), "{text}");
        let back = terraflux_graph::Model::from_toml(&text).unwrap();
        assert!(back.plots[0].layers[0].shade.is_none() && back.plots[0].layers[0].blend.is_none());
    }

    #[test]
    fn the_legend_line_tells_the_sun_and_the_exaggeration() {
        assert_eq!(Shade::default().label(), "shaded: 315°, 45°, ×1");
        assert_eq!(Shade { azimuth: 90.0, altitude: 30.5, z: 2.0, strength: 1.0 }.label(), "shaded: 90°, 30.5°, ×2");
    }
}
