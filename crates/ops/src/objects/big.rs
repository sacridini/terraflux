//! Big images: rasters on disk read by windows instead of whole, and the guard against reading one whole that does
//! not fit.
//!
//! A [`Source`] gives rows or windows of the bands of an image, from memory ([`Layered`] with maps and stacks) or from
//! rasters on disk. The segmentation by tiles reads each tile's window and the stitching reads strips of rows; the
//! features read strips. Nothing there holds more than a strip (or a tile) of an image at a time.

use std::borrow::Cow;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use terraflux_raster::Grid;
use terraflux_raster::tiff::Window;

use crate::files::Rasters;
use crate::{Args, Error, Result, Value};

/// The size in memory (cells x bands x 8 bytes) above which `objects.features` reads rasters on disk by strips
/// instead of whole (256 MiB by default; the tests set it to 0 to try the strips on small images).
pub static STRIP_BYTES: AtomicU64 = AtomicU64::new(256 << 20);

/// The values of a strip (all the bands) a read aims at, about (8 million numbers, 64 MB; the tests make it small).
pub static STRIP_VALUES: AtomicUsize = AtomicUsize::new(1 << 23);

/// The bands of an image by rows and windows.
pub(crate) trait Source: Sync {
    fn nb(&self) -> usize;
    /// Rows `r0..r1`, whole width, of the bands in `which`, in that order.
    fn rows(&self, r0: usize, r1: usize, which: &[usize]) -> Result<Vec<Cow<'_, [f64]>>>;
    /// The window of rows `r0..r1` and columns `c0..c1` of every band.
    fn window(&self, r0: usize, r1: usize, c0: usize, c1: usize) -> Result<Vec<Arc<Vec<f64>>>>;
    /// How many rows a strip reads at a time.
    fn step(&self) -> usize;
}

/// Each strip of `h` rows in turn, with the rows of the bands `which` (the strips in order of the rows).
pub(crate) fn strips(
    src: &dyn Source,
    h: usize,
    which: &[usize],
    mut f: impl FnMut(usize, usize, &[Cow<'_, [f64]>]),
) -> Result<()> {
    let step = src.step().max(1);
    let mut r0 = 0;
    while r0 < h {
        let r1 = (r0 + step).min(h);
        let rows = src.rows(r0, r1, which)?;
        f(r0, r1, &rows);
        r0 = r1;
    }
    Ok(())
}

/// One layer of an image: in memory, or a layer of rasters on disk.
#[derive(Clone)]
pub(crate) enum Part {
    Mem(Arc<Vec<f64>>),
    Disk(Arc<Rasters>, usize),
}

/// An image of layers, each in memory or on disk, on one grid.
pub(crate) struct Layered {
    pub parts: Vec<Part>,
    pub w: usize,
    step: usize,
}

impl Layered {
    pub fn new(parts: Vec<Part>, w: usize, h: usize) -> Layered {
        // the strips: whole blocks of the files, about STRIP_VALUES numbers
        let block = parts
            .iter()
            .find_map(|p| match p {
                Part::Disk(r, l) => Some(r.images[r.layers[*l].image].block_rows()),
                Part::Mem(_) => None,
            })
            .unwrap_or(1)
            .max(1);
        let rows = (STRIP_VALUES.load(Ordering::Relaxed) / (w * parts.len()).max(1)).max(1);
        let step = if parts.iter().all(|p| matches!(p, Part::Mem(_))) {
            h.max(1)
        } else if rows < block {
            block
        } else {
            rows / block * block
        };
        Layered { parts, w, step }
    }

    /// An image all in memory.
    pub fn mem(data: &[Arc<Vec<f64>>], grid: &Grid) -> Layered {
        Layered::new(data.iter().cloned().map(Part::Mem).collect(), grid.width, grid.height)
    }
}

impl Source for Layered {
    fn nb(&self) -> usize {
        self.parts.len()
    }

    fn rows(&self, r0: usize, r1: usize, which: &[usize]) -> Result<Vec<Cow<'_, [f64]>>> {
        which
            .iter()
            .map(|&j| match &self.parts[j] {
                Part::Mem(d) => Ok(Cow::Borrowed(&d[r0 * self.w..r1 * self.w])),
                Part::Disk(r, l) => {
                    let win = Window { x0: 0, y0: r0 as i64, width: self.w, height: r1 - r0 };
                    r.window(*l, win).map(Cow::Owned)
                }
            })
            .collect()
    }

    fn window(&self, r0: usize, r1: usize, c0: usize, c1: usize) -> Result<Vec<Arc<Vec<f64>>>> {
        self.parts
            .iter()
            .map(|p| match p {
                Part::Mem(d) => {
                    let mut v = Vec::with_capacity((c1 - c0) * (r1 - r0));
                    for r in r0..r1 {
                        v.extend_from_slice(&d[r * self.w + c0..r * self.w + c1]);
                    }
                    Ok(Arc::new(v))
                }
                Part::Disk(rs, l) => {
                    let win = Window { x0: c0 as i64, y0: r0 as i64, width: c1 - c0, height: r1 - r0 };
                    rs.window(*l, win).map(Arc::new)
                }
            })
            .collect()
    }

    fn step(&self) -> usize {
        self.step
    }
}

/// An image on disk: its layers, names and grid.
pub(crate) struct Image {
    pub src: Layered,
    pub names: Vec<String>,
    pub grid: Arc<Grid>,
}

/// The layers a value of rasters on disk gives a port: every layer, or the one chosen when it was asked for alone
/// (as [`crate::raster_stats::layers`] takes them). None for the others, and for files of different grids.
fn disk_layers(v: &Value) -> Option<(Arc<Rasters>, Vec<usize>)> {
    let Value::Raster(r) = v else { return None };
    if r.mixed {
        return None;
    }
    let all = r.layers.len() > 1 && !r.picked;
    let layers = if all { (0..r.layers.len()).collect() } else { vec![r.chosen] };
    Some((r.clone(), layers))
}

/// The image of rasters on disk at `port`, none when the port holds anything else.
pub(crate) fn disk_image(args: &Args, port: &str) -> Option<Image> {
    let (r, layers) = disk_layers(args.get(port)?)?;
    let grid = Arc::new(r.grid_of(layers[0]).clone());
    let names = if layers.len() > 1 && !r.picked {
        layers.iter().map(|&l| r.layers[l].name.clone()).collect()
    } else {
        vec![port.to_string()]
    };
    let parts = layers.iter().map(|&l| Part::Disk(r.clone(), l)).collect();
    Some(Image { src: Layered::new(parts, grid.width, grid.height), names, grid })
}

/// The bytes the layers of rasters on disk at `port` take read whole (0 for anything else).
fn disk_bytes(args: &Args, port: &str) -> u64 {
    let Some((r, layers)) = args.get(port).and_then(disk_layers) else { return 0 };
    r.grid_of(layers[0]).cells() as u64 * layers.len() as u64 * 8
}

/// Whether the ports read their rasters on disk by strips: some hold rasters on disk, and read whole they would
/// take more than [`STRIP_BYTES`].
pub(crate) fn strips_wanted(args: &Args, ports: &[&str]) -> bool {
    let total: u64 = ports.iter().map(|p| disk_bytes(args, p)).sum();
    total > 0 && total >= STRIP_BYTES.load(Ordering::Relaxed)
}

/// The layers of a port for strips: names, grid and each layer in memory or on disk.
pub(crate) fn parts_of(args: &Args, port: &str) -> Result<(Vec<String>, Arc<Grid>, Vec<Part>)> {
    if let Some(i) = disk_image(args, port) {
        return Ok((i.names, i.grid, i.src.parts));
    }
    let l = layers(args, port)?;
    Ok((l.names, l.grid, l.data.into_iter().map(Part::Mem).collect()))
}

// ---- the guard

/// The memory of the computer (bytes), when the system says.
fn physical_memory() -> Option<u64> {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Status {
            length: u32,
            load: u32,
            total: u64,
            avail: u64,
            page_total: u64,
            page_avail: u64,
            virt_total: u64,
            virt_avail: u64,
            ext: u64,
        }
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn GlobalMemoryStatusEx(s: *mut Status) -> i32;
        }
        // SAFETY: `s` is zeroed memory of the right layout with its length set, for the call to fill
        let mut s: Status = unsafe { std::mem::zeroed() };
        s.length = std::mem::size_of::<Status>() as u32;
        let ok = unsafe { GlobalMemoryStatusEx(&mut s) };
        (ok != 0 && s.total > 0).then_some(s.total)
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = text.lines().find(|l| l.starts_with("MemTotal:"))?;
        line.split_whitespace().nth(1)?.parse::<u64>().ok().map(|k| k * 1024)
    }
    #[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
    {
        None
    }
}

/// The most an image may take read whole into memory: half the computer's memory, else 8 GB.
pub(crate) fn memory_limit() -> u64 {
    physical_memory().map_or(8_000_000_000, |m| m / 2)
}

/// Refuse reading `bands` bands of `cells` cells whole (8 bytes a cell) when they take more than `limit`.
pub(crate) fn check_size(node: &str, port: &str, cells: usize, bands: usize, limit: u64, what: &str) -> Result<()> {
    let bytes = cells as u64 * bands as u64 * 8;
    if bytes <= limit {
        return Ok(());
    }
    Err(Error::msg(format!(
        "node {node:?}: {port} read whole is {bands} band(s) x {:.1} million cells = {:.1} GB as numbers, over the \
         {:.1} GB this computer can give an image (half its memory): {what}",
        cells as f64 / 1e6,
        bytes as f64 / 1e9,
        limit as f64 / 1e9
    )))
}

/// [`crate::raster_stats::layers`] for the operators of objects, refusing rasters on disk too big to read whole.
pub(crate) fn layers(args: &Args, port: &str) -> Result<crate::raster_stats::Layers> {
    if let Some((r, layers)) = args.get(port).and_then(disk_layers) {
        check_size(
            args.node,
            port,
            r.grid_of(layers[0]).cells(),
            layers.len(),
            memory_limit(),
            "read the image with raster.read as rasters on disk and, in segment.multiresolution, give tile above 0 (a \
             window of it is read at a time); objects.features reads rasters on disk by strips",
        )?;
    }
    crate::raster_stats::layers(args, port)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_image_over_the_limit_is_refused_with_what_to_do() {
        // 10 bands of the 120 million cells of a Sentinel-2 tile: 9.6 GB as numbers
        let cells = 10_980 * 10_980;
        let e = check_size("n", "image", cells, 10, 8_000_000_000, "read it as rasters on disk").unwrap_err();
        let m = e.to_string();
        assert!(m.contains("9.6 GB") && m.contains("8.0 GB") && m.contains("rasters on disk"), "{m}");
        assert!(check_size("n", "image", cells, 10, 16_000_000_000, "").is_ok());
        // the computer's memory is known here, and the limit is half of it
        if let Some(m) = physical_memory() {
            assert_eq!(memory_limit(), m / 2);
        }
    }
}
