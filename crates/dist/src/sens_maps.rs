//! The maps of sensitivity of an analysis, cell by cell: the map each run kept (`<runs>/<label>/<file>`) read by blocks
//! of rows, so that memory stays bounded however many runs and cells, and the index of each factor in every cell
//! written as one GeoTIFF of several bands (`sensitivity_maps.tif`). Shared by the application and `terraflux
//! sensitivity --maps`.

use std::path::{Path, PathBuf};

use terraflux_raster::image::Image;
use terraflux_raster::tiff::Window;
use terraflux_raster::{Band, DataType, Raster};

use crate::sensitivity::{Design, Method, average_replicates, cell_bands, map_bands};
use crate::{Error, Result};

/// The file the maps are written to, in the results' folder.
pub const FILE: &str = "sensitivity_maps.tif";

/// The file a run keeps of the output measured (`node.port`): `node_port.tif`, as the outputs of a model are named.
pub fn run_file(output: &str) -> String {
    format!("{}.tif", output.replace(['.', '/'], "_"))
}

/// The model set so that a run keeps the last map of `output` and nothing else of the model's outputs, in
/// [`run_file`].
pub fn keep_last_map(model: &mut terraflux_graph::Model, output: &str) {
    model.outputs.clear();
    model.outputs.insert(
        output.replace(['.', '/'], "_"),
        terraflux_graph::Output { from: output.to_string(), path: None, dtype: None, layer: Some("last".into()) },
    );
}

/// The most bytes of the runs' cells read at once (all the runs' cells of a block of rows, as f64).
pub const BLOCK_BYTES: usize = 256 << 20;

/// What the maps are made from.
pub struct Spec<'a> {
    pub method: Method,
    /// The factors' keys, and the design's points (a value per factor).
    pub keys: &'a [String],
    pub rows: &'a [Vec<f64>],
    pub design: &'a Design,
    /// The runs of each point.
    pub replicates: usize,
    /// The folders of the runs, in their order (`labels.len() == rows.len() * replicates`), under `runs`.
    pub labels: &'a [String],
    pub runs: &'a Path,
    /// The name of the map each run kept.
    pub file: &'a str,
    /// Where the maps are written (`sensitivity_maps.tif`).
    pub out: &'a Path,
    /// Leave each run's map in place (else they are deleted once the maps are made).
    pub keep_runs: bool,
}

/// The maps made.
#[derive(Debug, Clone, PartialEq)]
pub struct Made {
    pub path: PathBuf,
    /// The bands' names.
    pub bands: Vec<String>,
    /// The runs whose map was read, and those that had none.
    pub read: usize,
    pub missing: usize,
}

/// The rows of a block: as many as keep the runs' cells within `budget` bytes (one at least).
pub fn block_rows(runs: usize, width: usize, budget: usize) -> usize {
    (budget / (runs.max(1) * width.max(1) * 8)).max(1)
}

/// The path of a run's map.
fn path_of(s: &Spec, label: &str) -> PathBuf {
    s.runs.join(label).join(s.file)
}

/// Make the maps; `progress` is told the share done (0 to 1) after each block of rows.
pub fn make(s: &Spec, progress: &mut dyn FnMut(f64)) -> Result<Made> {
    make_within(s, BLOCK_BYTES, progress)
}

/// [`make`] reading blocks of rows that hold at most `budget` bytes of the runs' cells.
pub fn make_within(s: &Spec, budget: usize, progress: &mut dyn FnMut(f64)) -> Result<Made> {
    let r = s.replicates.max(1);
    if s.labels.len() != s.rows.len() * r {
        return Err(Error::Other(format!("{} runs for a design of {} points x {r}", s.labels.len(), s.rows.len())));
    }
    // the runs' maps (a run that failed has none); each opened only while it is read, so that thousands of runs
    // do not hold thousands of files open at once
    let open = |p: &Path| Image::open(p).map_err(|e| Error::Other(format!("{}: {e}", p.display())));
    let images: Vec<Option<PathBuf>> =
        s.labels.iter().map(|l| path_of(s, l)).map(|p| p.exists().then_some(p)).collect();
    let first =
        images.iter().flatten().next().ok_or_else(|| Error::Other(format!("no run kept its map ({})", s.file)))?;
    let grid = open(first)?.meta.grid.clone();
    let (width, height) = (grid.width, grid.height);
    for (l, p) in s.labels.iter().zip(&images) {
        if let Some(p) = p {
            let i = open(p)?;
            if (i.width(), i.height()) != (width, height) {
                return Err(Error::Other(format!(
                    "the map of run {l} is {}x{} cells, not {width}x{height}",
                    i.width(),
                    i.height()
                )));
            }
        }
    }
    // (a point without a single map has nothing to analyse)
    if let Some(p) = images.chunks(r).position(|point| point.iter().all(Option::is_none)) {
        return Err(Error::Other(format!("no run of point {} kept its map ({})", p + 1, s.labels[p * r])));
    }
    let names = map_bands(s.method, s.keys);
    let mut out: Vec<Vec<f64>> = vec![vec![f64::NAN; width * height]; names.len()];
    let step = block_rows(images.iter().flatten().count(), width, budget).min(height.max(1));
    let mut y0 = 0;
    while y0 < height {
        let rows = step.min(height - y0);
        let cells = rows * width;
        let win = Window { x0: 0, y0: y0 as i64, width, height: rows };
        let mut runs: Vec<Option<Vec<f64>>> = Vec::with_capacity(images.len());
        for p in &images {
            runs.push(match p {
                None => None,
                Some(p) => {
                    let i = open(p)?;
                    let band = i.meta.bands.len().saturating_sub(1);
                    let mut v =
                        i.read_window(band, win).map_err(|e| Error::Other(format!("{}: {e}", i.path.display())))?;
                    // (the cells without a value, as the file says them, are NaN)
                    if let Some(nd) = i.meta.nodata.filter(|nd| !nd.is_nan()) {
                        v.iter_mut().filter(|x| **x == nd).for_each(|x| *x = f64::NAN);
                    }
                    Some(v)
                }
            });
        }
        let points = average_replicates(&runs, r, cells);
        drop(runs);
        let bands = cell_bands(s.method, s.rows, &points, s.design, cells)?;
        for (o, b) in out.iter_mut().zip(&bands) {
            o[y0 * width..y0 * width + cells].copy_from_slice(b);
        }
        y0 += rows;
        progress(y0 as f64 / height as f64);
    }
    let missing = images.iter().filter(|i| i.is_none()).count();
    let read = images.len() - missing;
    let raster = Raster {
        grid,
        bands: names.iter().zip(out).map(|(n, data)| Band { name: Some(n.clone()), data }).collect(),
        dtype: DataType::F32,
        nodata: Some(f64::NAN),
    };
    std::fs::create_dir_all(s.out)?;
    let path = s.out.join(FILE);
    terraflux_ops::prepare::write_with_crs(&raster, &path).map_err(|e| Error::Other(e.to_string()))?;
    if !s.keep_runs {
        for l in s.labels {
            let p = path_of(s, l);
            let _ = std::fs::remove_file(&p);
            // (the run's folder too, when nothing else is in it)
            if let Some(d) = p.parent() {
                let _ = std::fs::remove_dir(d);
            }
        }
    }
    Ok(Made { path, bands: names, read, missing })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Vec<String> {
        vec!["a.x".into(), "b.y".into()]
    }

    /// A folder with a map for each run of an OAT design (one replicate): the left cells follow the first factor,
    /// the right ones the second; `height` rows of `width` cells.
    fn runs_dir(name: &str, width: usize, height: usize, skip: Option<usize>) -> (PathBuf, Vec<String>, Vec<Vec<f64>>) {
        let dir = std::env::temp_dir().join(format!("terraflux-sensmaps-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let rows = crate::sensitivity::rows(Method::Oat, &[(0.0, 1.0), (0.0, 1.0)], &Design::new(Method::Oat)).unwrap();
        let labels: Vec<String> = (1..=rows.len()).map(|k| format!("run{k}")).collect();
        for (k, (l, r)) in labels.iter().zip(&rows).enumerate() {
            if skip == Some(k) {
                continue;
            }
            let data: Vec<f64> = (0..width * height)
                .map(|c| {
                    let (col, row) = (c % width, c / width);
                    // a cell of the first column is no data (a value the file says is none)
                    if col == 0 && row == 0 {
                        f64::NAN
                    } else if col < width / 2 {
                        r[0] * (1.0 + row as f64)
                    } else {
                        r[1]
                    }
                })
                .collect();
            let m = Raster {
                grid: terraflux_raster::Grid::new(width, height),
                bands: vec![Band { name: None, data }],
                dtype: DataType::F32,
                nodata: Some(f64::NAN),
            };
            std::fs::create_dir_all(dir.join(l)).unwrap();
            m.write(dir.join(l).join("m.tif")).unwrap();
        }
        (dir, labels, rows)
    }

    fn spec<'a>(
        dir: &'a Path,
        labels: &'a [String],
        rows: &'a [Vec<f64>],
        keys: &'a [String],
        d: &'a Design,
    ) -> Spec<'a> {
        Spec {
            method: Method::Oat,
            keys,
            rows,
            design: d,
            replicates: 1,
            labels,
            runs: dir,
            file: "m.tif",
            out: dir,
            keep_runs: false,
        }
    }

    #[test]
    fn the_maps_are_made_block_by_block_and_the_runs_maps_deleted() {
        let (dir, labels, rows) = runs_dir("a", 6, 7, None);
        let (k, d) = (keys(), Design::new(Method::Oat));
        let s = spec(&dir, &labels, &rows, &k, &d);
        let mut told = Vec::new();
        let made = make(&s, &mut |p| told.push(p)).unwrap();
        assert_eq!(made.bands, ["mean", "sd", "swing_a.x", "swing_b.y"]);
        assert_eq!((made.read, made.missing), (5, 0));
        assert!(told.last() == Some(&1.0) && told.windows(2).all(|w| w[0] <= w[1]));
        let img = Raster::read(&made.path).unwrap();
        assert_eq!((img.bands.len(), img.grid.width, img.grid.height), (4, 6, 7));
        assert_eq!(img.band_names(), made.bands);
        let band = |n: &str| img.band(n).unwrap().data.clone();
        let (sx, sy) = (band("swing_a.x"), band("swing_b.y"));
        // row 3 (value x * 4 on the left): swing 4 for the first factor; the right is the second factor's
        assert!((sx[3 * 6 + 1] - 4.0).abs() < 1e-6 && sy[3 * 6 + 1] == 0.0);
        assert!((sy[3 * 6 + 5] - 1.0).abs() < 1e-6 && sx[3 * 6 + 5] == 0.0);
        // a cell without a value in the runs has none in the maps
        assert!(sx[0].is_nan() && band("mean")[0].is_nan());
        // the runs' maps and their folders are gone, the maps stay
        assert!(labels.iter().all(|l| !dir.join(l).exists()));
        assert!(made.path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_block_of_rows_holds_the_runs_within_the_budget() {
        // the block is at least a row, and never more cells than the budget holds
        assert_eq!(block_rows(1, usize::MAX / 16, BLOCK_BYTES), 1);
        assert_eq!(block_rows(1000, 10_000, BLOCK_BYTES), (BLOCK_BYTES / (1000 * 10_000 * 8)).max(1));
        assert_eq!(block_rows(100, 1000, BLOCK_BYTES), BLOCK_BYTES / (100 * 1000 * 8));
        assert!(block_rows(0, 0, BLOCK_BYTES) >= 1);
    }

    #[test]
    fn a_map_whose_rows_are_not_a_multiple_of_the_block_gives_the_same_maps() {
        // 5 runs of 6 cells a row: a budget of 3 rows reads 3 + 3 + 1 rows of the 7
        let (dir, labels, rows) = runs_dir("d", 6, 7, None);
        let (k, d) = (keys(), Design::new(Method::Oat));
        let mut s = spec(&dir, &labels, &rows, &k, &d);
        s.keep_runs = true;
        let whole = make(&s, &mut |_| {}).unwrap();
        let a = Raster::read(&whole.path).unwrap();
        let mut told = Vec::new();
        let blocks = make_within(&s, 5 * 6 * 8 * 3, &mut |p| told.push(p)).unwrap();
        assert_eq!(told.len(), 3, "{told:?}");
        let b = Raster::read(&blocks.path).unwrap();
        for (x, y) in a.bands.iter().zip(&b.bands) {
            assert!(
                x.data.iter().zip(&y.data).all(|(p, q)| p == q || (p.is_nan() && q.is_nan())),
                "{}",
                x.name.clone().unwrap()
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_failed_run_is_left_out_and_the_maps_can_keep_the_runs() {
        // the second replicate of nothing: one replicate per point, one run failed: its point has no map at all
        let (dir, labels, rows) = runs_dir("b", 4, 3, Some(2));
        let (k, d) = (keys(), Design::new(Method::Oat));
        let err = make(&spec(&dir, &labels, &rows, &k, &d), &mut |_| {}).unwrap_err().to_string();
        assert!(err.contains("point 3") && err.contains("run3"), "{err}");
        assert!(dir.join("run1").join("m.tif").exists(), "nothing deleted when the maps failed");
        let _ = std::fs::remove_dir_all(&dir);
        // two replicates of each point, one of them failed: the other one stands for the point
        let (dir, labels, rows) = runs_dir("c", 4, 3, None);
        let doubled: Vec<String> = labels.iter().flat_map(|l| [l.clone(), format!("{l}b")]).collect();
        let mut s = spec(&dir, &doubled, &rows, &k, &d);
        s.replicates = 2;
        s.keep_runs = true;
        let made = make(&s, &mut |_| {}).unwrap();
        assert_eq!((made.read, made.missing), (5, 5));
        assert!(dir.join("run1").join("m.tif").exists(), "the runs' maps kept on request");
        // the wrong number of runs for the design is refused
        s.replicates = 1;
        assert!(make(&s, &mut |_| {}).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
