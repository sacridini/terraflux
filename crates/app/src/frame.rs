//! The saved view of a Plot ([`terraflux_graph::PlotView`]): a box in the coordinates of the Plot's map, turned into
//! the cells of a map's grid and cut out of it. The figure, *Export the view*, the export of a Plot by its id and the
//! Document cut the map this way, so all of them show the same part; the Viewer asks for the box of what it shows
//! (to keep it) and for the cells of a box (to open on it).

use std::sync::Arc;

use terraflux_ops::{Map, Stack, Value};
use terraflux_raster::Grid;
use terraflux_raster::tiff::Window;

use crate::figure::Panel;
use crate::map_view::Source;

/// A box of a grid's cells: the first column and row, and how many of each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cells {
    pub x0: usize,
    pub y0: usize,
    pub w: usize,
    pub h: usize,
}

/// The cells of `g` that the box `extent` (west, south, east, north) touches, whole cells; none when it misses the
/// grid.
pub fn cells_of(g: &Grid, extent: [f64; 4]) -> Option<Cells> {
    let t = g.transform;
    let det = t[1] * t[5] - t[2] * t[4];
    if g.width == 0 || g.height == 0 || det == 0.0 || !det.is_finite() {
        return None;
    }
    let [west, south, east, north] = extent;
    let (mut c0, mut c1, mut r0, mut r1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for (x, y) in [(west, south), (east, south), (west, north), (east, north)] {
        let (dx, dy) = (x - t[0], y - t[3]);
        let col = (t[5] * dx - t[2] * dy) / det;
        let row = (t[1] * dy - t[4] * dx) / det;
        (c0, c1, r0, r1) = (c0.min(col), c1.max(col), r0.min(row), r1.max(row));
    }
    if ![c0, c1, r0, r1].iter().all(|v| v.is_finite()) {
        return None;
    }
    // (a little slack, so that a box made from whole cells gives them back)
    let eps = 1e-6;
    let lo = |v: f64, size: usize| (v + eps).floor().clamp(0.0, size as f64) as usize;
    let hi = |v: f64, size: usize| (v - eps).ceil().clamp(0.0, size as f64) as usize;
    let (x0, x1, y0, y1) = (lo(c0, g.width), hi(c1, g.width), lo(r0, g.height), hi(r1, g.height));
    (x1 > x0 && y1 > y0).then_some(Cells { x0, y0, w: x1 - x0, h: y1 - y0 })
}

/// The box (west, south, east, north) of a part of `g` given in cells (`[x0, y0, x1, y1]`, as the Viewer's
/// [`crate::map_view::MapView::visible`] has it).
pub fn extent_of(g: &Grid, cells: [f32; 4]) -> [f64; 4] {
    let t = g.transform;
    let (mut west, mut east, mut south, mut north) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
    for (c, r) in [(cells[0], cells[1]), (cells[2], cells[1]), (cells[0], cells[3]), (cells[2], cells[3])] {
        let (c, r) = (f64::from(c), f64::from(r));
        let (x, y) = (t[0] + c * t[1] + r * t[2], t[3] + c * t[4] + r * t[5]);
        (west, east, south, north) = (west.min(x), east.max(x), south.min(y), north.max(y));
    }
    [west, south, east, north]
}

/// The cells `[x0, y0, x1, y1]` of `g` that make a box, for the Viewer to zoom to.
pub fn view_cells(g: &Grid, extent: [f64; 4]) -> Option<[f32; 4]> {
    let c = cells_of(g, extent)?;
    Some([c.x0 as f32, c.y0 as f32, (c.x0 + c.w) as f32, (c.y0 + c.h) as f32])
}

/// The grid of a part of `g`: the same cells, the origin moved.
fn grid_of(g: &Grid, c: Cells) -> Grid {
    let t = g.transform;
    let (x, y) = (c.x0 as f64, c.y0 as f64);
    Grid {
        width: c.w,
        height: c.h,
        transform: [t[0] + x * t[1] + y * t[2], t[1], t[2], t[3] + x * t[4] + y * t[5], t[4], t[5]],
        geo: g.geo.clone(),
    }
}

/// The part of a layer of cells that `c` is.
fn part(data: &[f64], width: usize, c: Cells) -> Vec<f64> {
    let mut out = Vec::with_capacity(c.w * c.h);
    for r in c.y0..c.y0 + c.h {
        out.extend_from_slice(&data[r * width + c.x0..r * width + c.x0 + c.w]);
    }
    out
}

/// Whether `c` is the whole grid.
fn whole(g: &Grid, c: Cells) -> bool {
    c == Cells { x0: 0, y0: 0, w: g.width, h: g.height }
}

/// A map cut to a box; the map itself when the box holds it all or misses it.
pub fn crop_map(m: &Map, extent: [f64; 4]) -> Map {
    match cells_of(&m.grid, extent) {
        Some(c) if !whole(&m.grid, c) => Map::new(Arc::new(grid_of(&m.grid, c)), part(&m.data, m.grid.width, c)),
        _ => m.clone(),
    }
}

/// A source cut to a box (rasters on disk: the part read); the source itself when the box holds it all or misses it.
pub fn crop_source(s: &Source, extent: [f64; 4]) -> Result<Source, String> {
    let g = s.grid();
    let Some(c) = cells_of(g, extent).filter(|c| !whole(g, *c)) else { return Ok(s.clone()) };
    Ok(match s {
        Source::Map(m) => Source::Map(crop_map(m, extent)),
        Source::Rasters(r, l) => {
            let win = Window { x0: c.x0 as i64, y0: c.y0 as i64, width: c.w, height: c.h };
            let data = r.window(*l, win).map_err(|e| e.to_string())?;
            Source::Map(Map::new(Arc::new(grid_of(g, c)), data))
        }
        Source::Blank(_) => Source::Blank(Arc::new(grid_of(g, c))),
        // (a shade: its map cut, the sun kept)
        Source::Shade(sh) => {
            Source::Shade(Arc::new(crate::shade::ShadeSource { base: crop_source(&sh.base, extent)?, sun: sh.sun }))
        }
        // (a colour composite: each band cut, the ranges kept)
        Source::Rgb(r) => {
            let cut = |b: &Source| crop_source(b, extent);
            Source::Rgb(Arc::new(crate::rgb::RgbSource {
                bands: [cut(&r.bands[0])?, cut(&r.bands[1])?, cut(&r.bands[2])?],
                ..(**r).clone()
            }))
        }
    })
}

/// The panel cut to a box: its map is the part, so the vectors (placed by coordinates), the overlaid maps (placed by
/// their grids), the coordinates, the scale and the inset all follow it.
pub fn crop_panel(p: &Panel, extent: [f64; 4]) -> Result<Panel, String> {
    Ok(Panel { source: crop_source(&p.source, extent)?, ..p.clone() })
}

/// A value cut to a box: a map, the layers of a stack, the steps of a series, rasters on disk (their chosen layer);
/// anything else as it is.
pub fn crop_value(v: &Value, extent: [f64; 4]) -> Value {
    match v {
        Value::Map(m) => Value::Map(crop_map(m, extent)),
        Value::Stack(s) => match cells_of(&s.grid, extent).filter(|c| !whole(&s.grid, *c)) {
            Some(c) => Value::Stack(Stack {
                grid: Arc::new(grid_of(&s.grid, c)),
                keys: s.keys.clone(),
                layers: s.layers.iter().map(|l| Arc::new(part(l, s.grid.width, c))).collect(),
            }),
            None => v.clone(),
        },
        Value::Series(items) => Value::Series(items.iter().map(|i| crop_value(i, extent)).collect()),
        Value::Raster(r) => match crop_source(&Source::Rasters(r.clone(), r.chosen), extent) {
            Ok(Source::Map(m)) => Value::Map(m),
            _ => v.clone(),
        },
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 6 x 4 map of cells 10 wide from (100, 500): the value of a cell is its column + 10 * its row.
    fn map() -> Map {
        let mut g = Grid::new(6, 4);
        g.transform = [100.0, 10.0, 0.0, 500.0, 0.0, -10.0];
        Map::new(Arc::new(g), (0..24).map(|i| (i % 6 + 10 * (i / 6)) as f64).collect())
    }

    #[test]
    fn a_box_is_cells_and_back() {
        let m = map();
        // x 120..150 is columns 2 to 4, y 470..490 is rows 1 to 2
        let c = cells_of(&m.grid, [120.0, 470.0, 150.0, 490.0]).unwrap();
        assert_eq!(c, Cells { x0: 2, y0: 1, w: 3, h: 2 });
        // a box half into a cell takes the cell
        let c = cells_of(&m.grid, [125.0, 475.0, 145.0, 485.0]).unwrap();
        assert_eq!(c, Cells { x0: 2, y0: 1, w: 3, h: 2 });
        // beyond the map: only what is in it; away from it: none
        let c = cells_of(&m.grid, [0.0, 0.0, 130.0, 1000.0]).unwrap();
        assert_eq!(c, Cells { x0: 0, y0: 0, w: 3, h: 4 });
        assert_eq!(cells_of(&m.grid, [0.0, 0.0, 50.0, 50.0]), None);
        // the Viewer's cells give the box and the box gives them back
        let e = extent_of(&m.grid, [2.0, 1.0, 5.0, 3.0]);
        assert_eq!(e, [120.0, 470.0, 150.0, 490.0]);
        assert_eq!(view_cells(&m.grid, e), Some([2.0, 1.0, 5.0, 3.0]));
    }

    #[test]
    fn a_map_is_cut_to_the_box() {
        let m = map();
        let c = crop_map(&m, [120.0, 470.0, 150.0, 490.0]);
        assert_eq!((c.grid.width, c.grid.height), (3, 2));
        assert_eq!(c.grid.transform, [120.0, 10.0, 0.0, 490.0, 0.0, -10.0]);
        assert_eq!(*c.data, vec![12.0, 13.0, 14.0, 22.0, 23.0, 24.0]);
        // the whole map, or a box away from it: the map as it is
        assert_eq!(crop_map(&m, [0.0, 0.0, 1000.0, 1000.0]).data, m.data);
        assert_eq!(crop_map(&m, [0.0, 0.0, 50.0, 50.0]).grid.width, 6);
        // a stack and a series follow
        let stack = Stack {
            grid: m.grid.clone(),
            keys: vec![terraflux_ops::Key::Name("a".into())],
            layers: vec![m.data.clone()],
        };
        let Value::Stack(s) = crop_value(&Value::Stack(stack), [120.0, 470.0, 150.0, 490.0]) else { panic!() };
        assert_eq!((s.grid.width, s.layers[0].len()), (3, 6));
        let Value::Series(items) = crop_value(&Value::Series(vec![Value::Map(m)]), [120.0, 470.0, 150.0, 490.0]) else {
            panic!()
        };
        let Value::Map(c) = &items[0] else { panic!() };
        assert_eq!(c.data.len(), 6);
    }

    #[test]
    fn a_rotated_grid_is_cut_by_the_cells_its_box_touches() {
        // columns go east and a little north, rows south and a little east
        let mut g = Grid::new(10, 10);
        g.transform = [0.0, 1.0, 0.5, 100.0, 0.5, -1.0];
        let c = cells_of(&g, [2.0, 96.0, 4.0, 99.0]).unwrap();
        assert!(c.w >= 1 && c.h >= 1 && c.x0 + c.w <= 10 && c.y0 + c.h <= 10, "{c:?}");
        let cut = grid_of(&g, c);
        assert_eq!(cut.transform[1..3], g.transform[1..3]);
        assert_eq!(cut.transform[0], c.x0 as f64 + 0.5 * c.y0 as f64);
    }
}
