//! A window of an image segmented on its own: what the Viewer's live segmentation runs while a slider moves (the part
//! of the image in view, not the whole), and the ESP curve over the same window.
//!
//! The window is read every `step`-th cell when it has too many for the answer to come in a second; the objects then
//! live on a coarser grid placed where the window is (its transform says so), so they draw over the image as they are.

use std::sync::Arc;

use terraflux_raster::Grid;

use super::Objects;
use super::esp::{Settings, esp, tables};
use super::multiresolution::{Cost, merge_level};
use crate::raster_stats::Layers;
use crate::{Error, Result, Table};

/// A window of an image: the cells from column `x0` and row `y0`, `width` by `height`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Window {
    pub x0: usize,
    pub y0: usize,
    pub width: usize,
    pub height: usize,
}

impl Window {
    pub fn cells(&self) -> usize {
        self.width * self.height
    }

    /// Every how many cells the window is read so that it has no more than `most` of them (1: all).
    pub fn step_for(&self, most: usize) -> usize {
        let mut s = 1;
        while self.width.div_ceil(s) * self.height.div_ceil(s) > most.max(1) {
            s += 1;
        }
        s
    }

    /// The grid of the window read every `step`-th cell, placed where the window is on `grid`.
    pub fn grid(&self, grid: &Grid, step: usize) -> Grid {
        let t = grid.transform;
        let s = step as f64;
        Grid {
            width: self.width.div_ceil(step),
            height: self.height.div_ceil(step),
            transform: [
                t[0] + self.x0 as f64 * t[1] + self.y0 as f64 * t[2],
                t[1] * s,
                t[2] * s,
                t[3] + self.x0 as f64 * t[4] + self.y0 as f64 * t[5],
                t[4] * s,
                t[5] * s,
            ],
            geo: grid.geo.clone(),
        }
    }
}

/// The settings of a segmentation (as `segment.multiresolution`'s).
#[derive(Debug, Clone)]
pub struct WindowSettings {
    pub scale: f64,
    pub shape: f64,
    pub compactness: f64,
    /// The weight of each band (empty: 1 for all).
    pub weights: Vec<f64>,
    pub mutual: bool,
    pub seed: u64,
}

impl Default for WindowSettings {
    fn default() -> Self {
        WindowSettings { scale: 1.0, shape: 0.1, compactness: 0.5, weights: Vec::new(), mutual: true, seed: 0 }
    }
}

impl WindowSettings {
    fn cost(&self, nb: usize) -> Result<Cost> {
        if !self.scale.is_finite() || self.scale <= 0.0 {
            return Err(Error::msg(format!("scale {}: above 0", self.scale)));
        }
        if !(0.0..=0.9).contains(&self.shape) {
            return Err(Error::msg(format!("shape {}: 0 to 0.9", self.shape)));
        }
        if !(0.0..=1.0).contains(&self.compactness) {
            return Err(Error::msg(format!("compactness {}: 0 to 1", self.compactness)));
        }
        let weights = if self.weights.is_empty() { vec![1.0; nb] } else { self.weights.clone() };
        if weights.len() != nb {
            return Err(Error::msg(format!("weights: {nb} numbers, one for each band")));
        }
        Ok(Cost { colour: 1.0 - self.shape, compact: self.compactness, weights })
    }
}

/// The grid of a window and its bands.
type Bands = (Arc<Grid>, Vec<Arc<Vec<f64>>>);

/// The bands of the window, read every `step`-th cell, and the grid they are on.
fn read(grid: &Grid, data: &[Arc<Vec<f64>>], win: Window, step: usize) -> Result<Bands> {
    let (w, h) = (grid.width, grid.height);
    if step == 0 || win.width == 0 || win.height == 0 || win.x0 + win.width > w || win.y0 + win.height > h {
        return Err(Error::msg(format!(
            "window {}x{} at ({}, {}) is not inside the {w}x{h} image (step {step})",
            win.width, win.height, win.x0, win.y0
        )));
    }
    let g = win.grid(grid, step);
    let bands = data
        .iter()
        .map(|b| {
            let mut v = Vec::with_capacity(g.width * g.height);
            for r in 0..g.height {
                let row = (win.y0 + r * step) * w + win.x0;
                v.extend((0..g.width).map(|c| b[row + c * step]));
            }
            Arc::new(v)
        })
        .collect();
    Ok((Arc::new(g), bands))
}

/// The multiresolution segmentation of a window of an image (`data`, a band each, on `grid`), read every `step`-th
/// cell: the objects on the window's grid, no table. With `step` 1 the cost is the whole image's on those cells, so
/// where the window is aligned with whole objects the objects are the same as in the whole image's segmentation.
pub fn segment_window(
    grid: &Grid,
    data: &[Arc<Vec<f64>>],
    win: Window,
    step: usize,
    p: &WindowSettings,
) -> Result<Objects> {
    let cost = p.cost(data.len())?;
    let (g, bands) = read(grid, data, win, step)?;
    Ok(merge_level(&g, &bands, &cost, p.scale, p.mutual, p.seed, 100, None).objects)
}

/// The table of ESP (as `segment.scale`'s `table`: scale, objects, lv_, roc_, roc_mean, suggested) over a window read
/// every `step`-th cell, over `levels` scales from the image's own start.
#[allow(clippy::too_many_arguments)]
pub fn esp_window(
    grid: &Grid,
    data: &[Arc<Vec<f64>>],
    names: &[String],
    win: Window,
    step: usize,
    levels: usize,
    p: &WindowSettings,
) -> Result<Table> {
    let cost = p.cost(data.len()).or_else(|_| WindowSettings { scale: 1.0, ..p.clone() }.cost(data.len()))?;
    let (g, bands) = read(grid, data, win, step)?;
    let names: Vec<String> = (0..bands.len()).map(|k| names.get(k).cloned().unwrap_or_default()).collect();
    let l = Layers { grid: g, names: names.clone(), data: bands };
    let set = Settings { start: None, step: None, levels: levels.max(3), window: 0, seed: p.seed };
    let run = esp(&l, &cost, &set)?;
    Ok(tables(&run, &names).0)
}
