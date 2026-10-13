//! MSPA of maps larger than memory by overlapping tiles: GuidosToolbox's MSPA Tiling, as landschaft's
//! mspa_tiled (`landscape.mspa_tiled`), the same maps. The map is cut into tiles of `tile_size` cells that overlap
//! by `overlap`, MSPA runs on every tile (with GTB's shortcuts for tiles without background or foreground), and of
//! each overlap the half nearest to each tile is kept. Rasters on disk are read by windows and written by TIFF
//! tiles, a row of MSPA tiles at a time.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::DataType;
use terraflux_raster::tiff::Window;

use crate::files::{Rasters, RowsWriter};
use crate::landscape::{foreground_of, foreground_param, gtb_code, map_of};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(MspaTiled));
}

/// A tile along an axis: (read start, read stop, keep start, keep stop).
pub type Span = (usize, usize, usize, usize);

/// GTB's tiles along an axis of `size` cells (landschaft's tile_layout): tiles starting every
/// `tile_size - overlap` cells, the last ending at the border, the first `overlap / 2` cells of each overlap kept
/// from the earlier tile.
pub fn tile_layout(size: usize, tile_size: usize, overlap: usize) -> Result<Vec<Span>> {
    if tile_size <= overlap {
        return Err(Error::msg(format!("tile_size ({tile_size}) must be larger than overlap ({overlap})")));
    }
    let (step, border) = (tile_size - overlap, overlap / 2);
    let mut starts = vec![0];
    while starts[starts.len() - 1] + tile_size < size {
        starts.push(starts[starts.len() - 1] + step);
    }
    let n = starts.len();
    Ok(starts
        .iter()
        .enumerate()
        .map(|(k, &s)| {
            let keep_start = if k == 0 { 0 } else { s + border };
            let keep_stop = if k == n - 1 { size } else { s + border + step };
            (s, (s + tile_size).min(size), keep_start, keep_stop)
        })
        .collect())
}

/// The tiles of a map of ny x nx cells: GTB's along each axis, or the whole map when it is one tile.
fn tiles_of(ny: usize, nx: usize, tile_size: usize, overlap: usize) -> Result<(Vec<Span>, Vec<Span>)> {
    let (rows, cols) = (tile_layout(ny, tile_size, overlap)?, tile_layout(nx, tile_size, overlap)?);
    if rows.len() == 1 && cols.len() == 1 {
        return Ok((vec![(0, ny, 0, ny)], vec![(0, nx, 0, nx)]));
    }
    Ok((rows, cols))
}

/// MSPA's settings.
#[derive(Clone, Copy)]
struct Settings {
    connectivity: i32,
    edge_width: i32,
    internal: bool,
}

/// MSPA of a tile (ny x nx), with GTB's shortcuts for tiles without background or without foreground.
fn tile_mspa(tile: &[u8], ny: usize, nx: usize, s: Settings) -> Result<Vec<u8>> {
    let (lo, hi) = tile.iter().fold((u8::MAX, 0u8), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    if hi > 2 {
        return Err(Error::msg(format!("MSPA's input is 0 (missing), 1 (background) or 2 (foreground), not {hi}")));
    }
    Ok(if hi == 0 {
        vec![129; tile.len()]
    } else if lo == 2 {
        vec![17; tile.len()]
    } else if hi == 1 {
        tile.iter().map(|&v| if v == 0 { 129 } else { 0 }).collect()
    } else if !tile.contains(&1) {
        tile.iter().map(|&v| if v == 2 { 17 } else { 129 }).collect()
    } else {
        ffi::mspa(tile, ny as i64, nx as i64, s.connectivity, s.edge_width, s.internal)?
    })
}

/// The MSPA of a row of tiles: its rows kept (ky1 - ky0 rows of the map's width). `read` gives the input of a
/// window (x0, y0, width, height) in GTB's codes.
fn tile_row(
    (y0, y1, ky0, ky1): Span,
    cols: &[Span],
    nx: usize,
    s: Settings,
    read: &dyn Fn(usize, usize, usize, usize) -> Result<Vec<u8>>,
) -> Result<Vec<u8>> {
    let mut kept = vec![0u8; (ky1 - ky0) * nx];
    for &(x0, x1, kx0, kx1) in cols {
        let w = x1 - x0;
        let m = tile_mspa(&read(x0, y0, w, y1 - y0)?, y1 - y0, w, s)?;
        for y in ky0..ky1 {
            kept[(y - ky0) * nx + kx0..(y - ky0) * nx + kx1]
                .copy_from_slice(&m[(y - y0) * w + kx0 - x0..(y - y0) * w + kx1 - x0]);
        }
    }
    Ok(kept)
}

struct MspaTiled;

impl Operator for MspaTiled {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "landscape.mspa_tiled",
                "MSPA of a map larger than memory by overlapping tiles, GuidosToolbox's MSPA Tiling as landschaft's \
                 mspa_tiled, the same maps: MSPA of every tile (tile_size cells with an overlap), of each overlap \
                 the half nearest to each tile kept. An approximation, as GTB warns: near the seams the classes \
                 can differ from MSPA of the whole map where objects are larger than the overlap. Rasters on disk \
                 are read and written tile by tile (to path, a GeoTIFF); a map in memory gives a map.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Raster, "the map (0 missing, 1 background, 2 foreground) or rasters"))
            .output(PortSpec::new("map", Type::Raster, "the MSPA classes (on disk for rasters on disk)")),
        )
        .param(PortSpec::optional("connectivity", Type::Integer, "of the foreground: 8 (default) or 4").choices([8, 4]))
        .param(PortSpec::optional("edge_width", Type::Integer, "the edge's width in cells, 1-255 (default 1)"))
        .param(PortSpec::optional("internal", Type::Bool, "tell internal features apart (default true)"))
        .param(PortSpec::optional(
            "tile_size",
            Type::Integer,
            "a tile's side with its overlap, in cells (default 6500)",
        ))
        .param(PortSpec::optional("overlap", Type::Integer, "the tiles' overlap, in cells (default 2000)"))
        .param(PortSpec::optional(
            "path",
            Type::Text,
            "rasters on disk: the GeoTIFF written (default GTB's name next to the input: name_8_1_1_1.tif)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let s = Settings {
            connectivity: args.param("connectivity", 8)?,
            edge_width: args.param("edge_width", 1)?,
            internal: args.param("internal", true)?,
        };
        if (s.connectivity != 4 && s.connectivity != 8) || !(1..=255).contains(&s.edge_width) {
            return Err(fail(Error::msg("connectivity is 4 or 8, edge_width 1 to 255")));
        }
        let tile_size: usize = args.param("tile_size", 6500)?;
        let overlap: usize = args.param("overlap", 2000)?;
        let foreground = foreground_of(args)?;
        match args.get("map") {
            Some(Value::Map(map)) => {
                let (ny, nx) = map.shape();
                let input = gtb_code(args, &foreground, &map.data, false)?;
                let (rows, cols) = tiles_of(ny, nx, tile_size, overlap).map_err(fail)?;
                let read = |x0: usize, y0: usize, w: usize, h: usize| -> Result<Vec<u8>> {
                    Ok((y0..y0 + h).flat_map(|y| input[y * nx + x0..y * nx + x0 + w].iter().copied()).collect())
                };
                let mut out = Vec::with_capacity(ny * nx);
                for &row in &rows {
                    out.extend(tile_row(row, &cols, nx, s, &read).map_err(fail)?);
                }
                Ok(outputs([("map", map_of(map, out))]))
            }
            Some(Value::Raster(r)) => {
                let path = match args.opt_param::<String>("path")?.filter(|p| !p.is_empty()) {
                    Some(p) => PathBuf::from(p).with_extension("tif"),
                    None => {
                        let src = r.images[r.layers[r.chosen].image].path();
                        let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                        let suffix = format!("{}_{}_1_{}", s.connectivity, s.edge_width, u8::from(s.internal));
                        src.with_file_name(format!("{stem}_{suffix}.tif"))
                    }
                };
                if r.images.iter().any(|i| i.path() == path) {
                    return Err(fail(Error::msg("the output must differ from the input")));
                }
                if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
                    std::fs::create_dir_all(dir).map_err(|e| fail(Error::msg(format!("{}: {e}", dir.display()))))?;
                }
                let read = |x0: usize, y0: usize, w: usize, h: usize| -> Result<Vec<u8>> {
                    let win = Window { x0: x0 as i64, y0: y0 as i64, width: w, height: h };
                    gtb_code(args, &foreground, &r.window(r.chosen, win)?, false)
                };
                to_file(r, &path, s, tile_size, overlap, &read).map_err(fail)?;
                let written = Rasters::open(&path.to_string_lossy(), None, None)?;
                Ok(outputs([("map", Value::Raster(Arc::new(written)))]))
            }
            Some(v) => Err(fail(Error::msg(format!("map: a map or rasters, not a {:?}", v.ty())))),
            None => Err(fail(Error::msg("connect a map"))),
        }
    }
}

/// MSPA by GTB's tiles of rasters on disk into a GeoTIFF: a row of tiles at a time (read by windows), the rows
/// kept written as they come ([`RowsWriter`]); in memory a row of tiles and 512 rows.
fn to_file(
    r: &Rasters,
    path: &Path,
    s: Settings,
    tile_size: usize,
    overlap: usize,
    read: &dyn Fn(usize, usize, usize, usize) -> Result<Vec<u8>>,
) -> Result<()> {
    let grid = r.grid_of(r.chosen).clone();
    let (ny, nx) = (grid.height, grid.width);
    let (rows, cols) = tiles_of(ny, nx, tile_size, overlap)?;
    let mut out = RowsWriter::create(path, &grid, "mspa", DataType::U8, None)?;
    for &row in &rows {
        out.push(tile_row(row, &cols, nx, s, read)?.into_iter().map(f64::from))?;
    }
    out.finish()
}
