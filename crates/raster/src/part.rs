//! A part of a grid at a coarser resolution: the cells a check run reads of a raster (a window of the file, one
//! cell in every `step`), and the grid they make.

use crate::Grid;

/// A rectangle of the cells of a grid (columns `col..col + width`, rows `row..row + height`), taken one cell in
/// every `step` (the nearest cell, so classes stay classes: the cell at the middle of each block of `step`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    pub col: usize,
    pub row: usize,
    pub width: usize,
    pub height: usize,
    pub step: usize,
}

/// Slack, in cells, when a window's edge falls on a cell's edge (a rounding of the coordinates is not a cell).
const SLACK: f64 = 1e-6;

impl Part {
    /// The whole grid, one cell in every `step`.
    pub fn whole(grid: &Grid, step: usize) -> Part {
        Part { col: 0, row: 0, width: grid.width, height: grid.height, step: step.max(1) }
    }

    /// The cells of the grid a window `[xmin, ymin, xmax, ymax]` (in the grid's own coordinates) touches: its edges
    /// moved outward to the cell edges, so that rasters on one grid stay on one grid. An error when the window is
    /// empty, does not meet the grid, or the grid is rotated.
    pub fn of(grid: &Grid, window: [f64; 4], step: usize) -> Result<Part, String> {
        let t = grid.transform;
        if t[2] != 0.0 || t[4] != 0.0 || t[1] == 0.0 || t[5] == 0.0 {
            return Err("the grid is rotated: a window cannot be cut from it".into());
        }
        let [xmin, ymin, xmax, ymax] = window;
        if ![xmin, ymin, xmax, ymax].iter().all(|v| v.is_finite()) || xmin >= xmax || ymin >= ymax {
            return Err("the window is empty".into());
        }
        let span = |a: f64, b: f64, origin: f64, size: f64, n: usize| -> Option<(usize, usize)> {
            let (p, q) = ((a - origin) / size, (b - origin) / size);
            let (lo, hi) = if p < q { (p, q) } else { (q, p) };
            let first = (lo + SLACK).floor().max(0.0);
            let last = (hi - SLACK).ceil().min(n as f64);
            (first < last).then_some((first as usize, last as usize))
        };
        let cols = span(xmin, xmax, t[0], t[1], grid.width);
        let rows = span(ymin, ymax, t[3], t[5], grid.height);
        match (cols, rows) {
            (Some((c0, c1)), Some((r0, r1))) => {
                Ok(Part { col: c0, row: r0, width: c1 - c0, height: r1 - r0, step: step.max(1) })
            }
            _ => Err("the window does not meet the raster".into()),
        }
    }

    /// Columns of the part's grid.
    pub fn columns(&self) -> usize {
        self.width.div_ceil(self.step)
    }

    /// Rows of the part's grid.
    pub fn rows(&self) -> usize {
        self.height.div_ceil(self.step)
    }

    /// The column of the grid (not of the part) the part's column `k` takes its value from: the middle of its
    /// block of `step` cells, the last cell of the window for a block cut short by its edge.
    pub fn source_col(&self, k: usize) -> usize {
        self.col + (k * self.step + self.step / 2).min(self.width.saturating_sub(1))
    }

    /// The row of the grid the part's row `k` takes its value from.
    pub fn source_row(&self, k: usize) -> usize {
        self.row + (k * self.step + self.step / 2).min(self.height.saturating_sub(1))
    }

    /// Whether it is the grid itself (nothing cut, nothing coarsened).
    pub fn is_whole(&self, grid: &Grid) -> bool {
        self.step == 1 && self.col == 0 && self.row == 0 && self.width == grid.width && self.height == grid.height
    }

    /// The grid the part makes: its cells `step` times as wide and high, its corner the window's.
    pub fn grid(&self, grid: &Grid) -> Grid {
        let t = grid.transform;
        let (c, r, s) = (self.col as f64, self.row as f64, self.step as f64);
        let transform =
            [t[0] + c * t[1] + r * t[2], t[1] * s, t[2] * s, t[3] + c * t[4] + r * t[5], t[4] * s, t[5] * s];
        let mut g = Grid { width: self.columns(), height: self.rows(), transform, geo: grid.geo.clone() };
        if g.geo.pixel_scale.is_some() {
            g.geo.pixel_scale = Some(vec![transform[1], -transform[5], 0.0]);
        }
        if g.geo.tiepoints.is_some() {
            g.geo.tiepoints = Some(vec![0.0, 0.0, 0.0, transform[0], transform[3], 0.0]);
        }
        g
    }

    /// The part's cells of a band of the whole grid (row by row, `grid.width` cells a row).
    pub fn take(&self, grid: &Grid, data: &[f64]) -> Vec<f64> {
        let (w, h) = (self.columns(), self.rows());
        let mut out = Vec::with_capacity(w * h);
        for k in 0..h {
            let row = self.source_row(k);
            for j in 0..w {
                out.push(data[row * grid.width + self.source_col(j)]);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grid() -> Grid {
        // 10 x 8 cells of 10 m, the corner at (1000, 2000)
        let mut g = Grid::new(10, 8);
        g.transform = [1000.0, 10.0, 0.0, 2000.0, 0.0, -10.0];
        g
    }

    #[test]
    fn a_window_is_moved_outward_to_the_cells() {
        // x 1013..1047 is the columns 1 to 4, y 1943..1977 the rows 2 to 5
        let p = Part::of(&grid(), [1013.0, 1943.0, 1047.0, 1977.0], 1).unwrap();
        assert_eq!((p.col, p.row, p.width, p.height), (1, 2, 4, 4));
        let g = p.grid(&grid());
        assert_eq!((g.width, g.height), (4, 4));
        assert_eq!(g.transform, [1010.0, 10.0, 0.0, 1980.0, 0.0, -10.0]);
        // edges on the cells' edges stay there
        let q = Part::of(&grid(), [1010.0, 1950.0, 1040.0, 1980.0], 1).unwrap();
        assert_eq!((q.col, q.row, q.width, q.height), (1, 2, 3, 3));
        // a window passing the grid is cut to it; one outside it is refused
        let r = Part::of(&grid(), [900.0, 1000.0, 1030.0, 3000.0], 1).unwrap();
        assert_eq!((r.col, r.row, r.width, r.height), (0, 0, 3, 8));
        assert!(Part::of(&grid(), [5000.0, 0.0, 6000.0, 100.0], 1).is_err());
        assert!(Part::of(&grid(), [1010.0, 1950.0, 1010.0, 1980.0], 1).is_err());
    }

    #[test]
    fn coarsening_takes_the_nearest_cell_and_updates_the_transform() {
        let g = grid();
        // classes by column: 1 2 3 4 5 6 7 8 9 10 on every row
        let data: Vec<f64> = (0..g.cells()).map(|i| (i % 10 + 1) as f64).collect();
        let p = Part::whole(&g, 4);
        assert_eq!((p.columns(), p.rows()), (3, 2));
        let out = p.take(&g, &data);
        // the cell at the middle of each block of four (columns 2, 6, and the last one for the short block)
        assert_eq!(&out[..3], &[3.0, 7.0, 10.0]);
        assert!(out.iter().all(|v| data.contains(v)), "classes stay classes");
        let c = p.grid(&g);
        assert_eq!((c.width, c.height), (3, 2));
        assert_eq!(c.transform, [1000.0, 40.0, 0.0, 2000.0, 0.0, -40.0]);
    }

    #[test]
    fn rasters_on_one_grid_stay_on_one_grid() {
        let a = grid();
        let mut b = grid();
        b.geo.metadata.push(("k".into(), "v".into()));
        let w = [1013.0, 1943.0, 1047.0, 1977.0];
        let (pa, pb) = (Part::of(&a, w, 2).unwrap(), Part::of(&b, w, 2).unwrap());
        assert!(pa.grid(&a).same_as(&pb.grid(&b)));
    }
}
