//! A check run: the model run on a small part of the map, or at a coarser resolution, to see in seconds whether it
//! is right before a run of hours. The engine hands the check to the operators that make or read rasters
//! (`raster.read`, the model's inputs of maps, the neutral landscape models): a raster file is read through a
//! window of it and one cell in every `coarsen`; a generated raster has its rows and columns divided and its cells
//! made wider. The results are those of the model on a part of the landscape, not the model's: landscape metrics,
//! distances and neighbourhoods change with the resolution and the border.

use serde::{Deserialize, Serialize};
use terraflux_raster::Grid;
use terraflux_raster::part::Part;

use crate::{Error, Result};

/// What a check run changes in the rasters.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Check {
    /// The part of the map: `[xmin, ymin, xmax, ymax]` in `crs`; none: all of it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window: Option<[f64; 4]>,
    /// The window's coordinate system as GDAL takes it (`EPSG:32633`, a WKT); none: the window is in the rasters' own
    /// coordinates (each file's).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crs: Option<String>,
    /// One cell in every this many, on each side (1: the resolution of the files).
    #[serde(default)]
    pub coarsen: u32,
    /// The folder the files the model writes go to in the check run (by their names), not to where they are said to
    /// go; none: where they are said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub out: Option<std::path::PathBuf>,
}

impl Check {
    /// The check of a window and a factor (0 and 1 are the same: no coarsening).
    pub fn new(window: Option<([f64; 4], Option<String>)>, coarsen: u32) -> Check {
        let (window, crs) = match window {
            Some((w, c)) => (Some(w), c),
            None => (None, None),
        };
        Check { window, crs, coarsen: coarsen.max(1), out: None }
    }

    /// The same check writing the files of the model into a folder.
    pub fn writing_to(mut self, dir: impl Into<std::path::PathBuf>) -> Check {
        self.out = Some(dir.into());
        self
    }

    /// The factor, at least 1.
    pub fn step(&self) -> usize {
        self.coarsen.max(1) as usize
    }

    /// Whether it changes nothing.
    pub fn is_none(&self) -> bool {
        self.window.is_none() && self.step() == 1
    }

    /// The check in words: "1/4 resolution", "on the view", "on the view, 1/4 resolution".
    pub fn label(&self) -> String {
        match (self.window.is_some(), self.step()) {
            (false, 1) => "whole".into(),
            (false, s) => format!("1/{s} resolution"),
            (true, 1) => "on the view".into(),
            (true, s) => format!("on the view, 1/{s} resolution"),
        }
    }

    /// The check in a few characters, for a list: "1/4", "view", "view, 1/4".
    pub fn short(&self) -> String {
        match (self.window.is_some(), self.step()) {
            (false, 1) => "whole".into(),
            (false, s) => format!("1/{s}"),
            (true, 1) => "view".into(),
            (true, s) => format!("view, 1/{s}"),
        }
    }

    /// The part of a raster's grid the check reads. An error, saying which raster (`what`), when the window cannot
    /// be put on it: it has no coordinate system and the window is in one, the grid is rotated, or the window does
    /// not meet it.
    pub fn part_of(&self, grid: &Grid, what: &str) -> Result<Part> {
        let step = self.step();
        let Some(w) = self.window else { return Ok(Part::whole(grid, step)) };
        let own = crate::prepare::crs_of(grid);
        let w = match (&self.crs, &own) {
            (None, _) => w,
            (Some(c), None) => {
                return Err(Error::msg(format!(
                    "check run: {what} has no coordinate system, and the window is in {}",
                    short(c)
                )));
            }
            (Some(c), Some(o)) => window_in(w, c, o).map_err(|e| Error::msg(format!("check run: {what}: {e}")))?,
        };
        Part::of(grid, w, step).map_err(|e| Error::msg(format!("check run: {what}: {e}")))
    }

    /// The settings an operator runs with in the check run: a generated raster drawn smaller, a file written (the
    /// outputs of the model, the rasters a tool makes at a path of its own) put in the check's folder, never over the
    /// real file.
    pub fn params(
        &self,
        sig: &terraflux_graph::OpSignature,
        params: &serde_json::Map<String, serde_json::Value>,
        has_like: bool,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut out = crate::nlm::for_check(sig, params, has_like, self);
        let writes = sig.outputs.iter().any(|o| o.name == "path")
            || sig.params.iter().any(|p| p.name == "path" && p.doc.contains(" made"));
        if let (true, Some(dir), Some(serde_json::Value::String(path))) = (writes, &self.out, params.get("path"))
            && !path.is_empty()
        {
            let name =
                std::path::Path::new(path).file_name().map_or_else(|| path.clone(), |n| n.to_string_lossy().into());
            out.insert("path".into(), dir.join(name).to_string_lossy().into_owned().into());
        }
        out
    }

    /// A generated raster's `rows`, `columns` and `resolution` for the check: divided and multiplied by the factor,
    /// so that it covers the same ground; the settings as given when it changes nothing.
    pub fn scaled(&self, rows: usize, columns: usize, resolution: f64) -> (usize, usize, f64) {
        let s = self.step();
        (rows.div_ceil(s).max(1), columns.div_ceil(s).max(1), resolution * s as f64)
    }
}

/// A name for a CRS in a message: its code, else its first words.
fn short(crs: &str) -> String {
    if crs.len() <= 24 { crs.to_string() } else { format!("{}...", crs.chars().take(24).collect::<String>()) }
}

/// A window `[xmin, ymin, xmax, ymax]` of one coordinate system in another: the box of its edges' points moved.
fn window_in(w: [f64; 4], from: &str, to: &str) -> std::result::Result<[f64; 4], String> {
    if terraflux_gdal::same_crs(from, to)? {
        return Ok(w);
    }
    let [x0, y0, x1, y1] = w;
    // the edges, each in some points (a straight edge is a curve in another projection)
    let n = 16;
    let (mut xs, mut ys) = (Vec::new(), Vec::new());
    for k in 0..=n {
        let f = k as f64 / n as f64;
        let (x, y) = (x0 + (x1 - x0) * f, y0 + (y1 - y0) * f);
        xs.extend([x, x, x0, x1]);
        ys.extend([y0, y1, y, y]);
    }
    terraflux_gdal::Transform::new(from, to)?.points(&mut xs, &mut ys);
    let finite: Vec<(f64, f64)> =
        xs.iter().zip(&ys).filter(|(x, y)| x.is_finite() && y.is_finite()).map(|(x, y)| (*x, *y)).collect();
    if finite.is_empty() {
        return Err("the window cannot be put in the raster's coordinate system".into());
    }
    let fold = |f: fn(f64, f64) -> f64, init: f64, pick: fn(&(f64, f64)) -> f64| finite.iter().map(pick).fold(init, f);
    Ok([
        fold(f64::min, f64::INFINITY, |p| p.0),
        fold(f64::min, f64::INFINITY, |p| p.1),
        fold(f64::max, f64::NEG_INFINITY, |p| p.0),
        fold(f64::max, f64::NEG_INFINITY, |p| p.1),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid(crs: Option<&str>) -> Grid {
        let mut g = Grid::new(100, 80);
        g.transform = [500000.0, 30.0, 0.0, 4000000.0, 0.0, -30.0];
        g.geo.wkt = crs.map(String::from);
        g
    }

    #[test]
    fn a_check_names_itself_and_scales_a_generated_raster() {
        let c = Check::new(None, 4);
        assert_eq!(c.label(), "1/4 resolution");
        assert_eq!(c.scaled(256, 100, 30.0), (64, 25, 120.0));
        assert_eq!(c.scaled(3, 3, 1.0).0, 1);
        assert_eq!(Check::new(Some(([0.0, 0.0, 1.0, 1.0], None)), 1).label(), "on the view");
        assert!(Check::new(None, 0).is_none());
    }

    #[test]
    fn a_window_in_the_rasters_coordinates_is_cut_and_coarsened() {
        // (no coordinate system on either: the window is the file's own)
        let c = Check::new(Some(([500300.0, 3999000.0, 501000.0, 3999700.0], None)), 2);
        let p = c.part_of(&grid(None), "a.tif").unwrap();
        assert_eq!((p.col, p.row), (10, 10));
        let g = p.grid(&grid(None));
        assert_eq!(g.transform[1], 60.0);
        assert_eq!(g.transform[0], 500300.0);
    }

    #[test]
    fn rasters_on_disk_are_seen_through_the_part_and_made_files_of_their_own_for_the_tools() {
        use terraflux_raster::{DataType, Raster};
        let dir = std::env::temp_dir().join(format!("terraflux-check-ops-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("view.tif");
        let g = grid(None);
        let data: Vec<f64> = (0..g.cells()).map(|i| (i % 100) as f64).collect();
        Raster::single(g, data, DataType::U8, None).write(&path).unwrap();
        let check = Check::new(Some(([500300.0, 3999000.0, 501000.0, 3999700.0], None)), 2);
        let name = path.to_string_lossy().into_owned();
        let r = crate::files::Rasters::open_files_checked(&name, vec![path.clone()], None, None, Some(&check)).unwrap();
        assert!(r.is_view());
        assert_eq!((r.grid.width, r.grid.height), (12, 12));
        // the cells by sampling and read whole are the same ones
        let whole = r.map().unwrap();
        assert_eq!(whole.data[..3], [11.0, 13.0, 15.0]);
        let sampled = r.sample(0, terraflux_raster::tiff::Window { x0: 0, y0: 0, width: 12, height: 12 }, 1).unwrap();
        assert!(sampled.iter().zip(whole.data.iter()).all(|(s, w)| *s as f64 == *w));
        // a file of its own, with the part's grid, for the operators that read files
        let own = r.materialized().unwrap();
        assert!(!own.is_view());
        assert_eq!((own.grid.width, own.grid.height), (12, 12));
        assert_eq!(own.grid.transform, r.grid.transform);
        assert_eq!(own.map().unwrap().data, whole.data);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_window_that_cannot_be_applied_is_refused_with_the_raster() {
        let c = Check::new(Some(([0.0, 0.0, 10.0, 10.0], Some("EPSG:4326".into()))), 1);
        let e = c.part_of(&grid(None), "a.tif").unwrap_err().to_string();
        assert!(e.contains("a.tif") && e.contains("no coordinate system"), "{e}");
        let c = Check::new(Some(([0.0, 0.0, 10.0, 10.0], None)), 1);
        let e = c.part_of(&grid(None), "b.tif").unwrap_err().to_string();
        assert!(e.contains("b.tif") && e.contains("does not meet"), "{e}");
    }
}
