//! A GeoTIFF file read where and as finely as needed: opening reads its directories only (the image's and its
//! overviews'); [`Image::sample`] gives a window's cells at every `step`-th row and column, from the overview
//! that is finest without being finer than needed, decoding only the chunks (strips or tiles) that hold the
//! cells taken, in parallel; [`Image::read_band`] a whole band, its chunks decoded in parallel.
//!
//! The file is read at positions (no shared cursor), so one `Image` serves many threads. Chunks read densely
//! (a step of a few cells) are kept in a cache, so that windows side by side do not decode the same chunk
//! again.

use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rayon::prelude::*;

use crate::tiff::{Layout, Reader, Window, chunk_values, decompress, directories};
use crate::{Error, Raster, Result};

/// The bytes of decoded chunks kept.
const CACHE_BYTES: usize = 768 << 20;

/// Read `len` bytes at `offset` without moving a cursor (threads share the file).
fn pread(f: &File, offset: u64, len: usize) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; len];
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut done = 0;
        while done < len {
            let n = f.seek_read(&mut buf[done..], offset + done as u64)?;
            if n == 0 {
                return Err(Error::Invalid("the file ends inside a chunk".into()));
            }
            done += n;
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        f.read_exact_at(&mut buf, offset)?;
    }
    Ok(buf)
}

/// A chunk: the level (0 the image, then its overviews), the chunk's index and the band.
type Key = (usize, usize, usize);

#[derive(Default)]
struct Chunks {
    kept: HashMap<Key, (Arc<Vec<f32>>, u64)>,
    bytes: usize,
    clock: u64,
}

impl Chunks {
    fn get(&mut self, k: &Key) -> Option<Arc<Vec<f32>>> {
        self.clock += 1;
        let clock = self.clock;
        self.kept.get_mut(k).map(|(v, used)| {
            *used = clock;
            v.clone()
        })
    }

    fn put(&mut self, k: Key, v: Arc<Vec<f32>>) {
        self.clock += 1;
        self.bytes += v.len() * 4;
        if let Some((old, _)) = self.kept.insert(k, (v, self.clock)) {
            self.bytes -= old.len() * 4;
        }
        while self.bytes > CACHE_BYTES && self.kept.len() > 1 {
            let oldest = *self.kept.iter().min_by_key(|(_, (_, used))| *used).map(|(k, _)| k).unwrap();
            if let Some((v, _)) = self.kept.remove(&oldest) {
                self.bytes -= v.len() * 4;
            }
        }
    }
}

/// An image file, read where needed.
pub struct Image {
    file: File,
    pub path: PathBuf,
    /// The image, then its overviews: each with the image's cells a cell of it covers.
    levels: Vec<(usize, Layout)>,
    /// The raster without cells: grid, bands' names, type, no-data value.
    pub meta: Raster,
    chunks: Mutex<Chunks>,
}

impl Image {
    pub fn open(path: impl AsRef<Path>) -> Result<Image> {
        let path = path.as_ref();
        let meta = Reader::open(path)?.meta;
        let (_, dirs) = directories(path)?;
        let full = dirs[0].1.width.max(1);
        let mut levels: Vec<(usize, Layout)> = dirs
            .into_iter()
            .map(|(_, l)| ((full as f64 / l.width.max(1) as f64).round().max(1.0) as usize, l))
            .filter(|(_, l)| l.spp == meta.bands.len())
            .collect();
        levels.sort_by_key(|(f, _)| *f);
        levels.dedup_by_key(|(f, _)| *f);
        Ok(Image {
            file: File::open(path)?,
            path: path.to_path_buf(),
            levels,
            meta,
            chunks: Mutex::new(Chunks::default()),
        })
    }

    pub fn width(&self) -> usize {
        self.meta.grid.width
    }

    pub fn height(&self) -> usize {
        self.meta.grid.height
    }

    /// The reductions of the overviews in the file (2, 4, 8...), none when it has none.
    pub fn overviews(&self) -> Vec<usize> {
        self.levels.iter().skip(1).map(|(f, _)| *f).collect()
    }

    /// A chunk of a level decoded, one band, exactly (cells without a value as they are).
    fn decode(&self, level: usize, index: usize, band: usize) -> Result<Vec<f64>> {
        let l = &self.levels[level].1;
        let (cw, ch) = l.chunk();
        let samples = if l.planar == 2 { 1 } else { l.spp };
        // strips hold only the image's rows; tiles are always whole
        let rows = match l.tiles {
            Some(_) => ch,
            None => {
                let strip = index % l.height.div_ceil(ch).max(1);
                ch.min(l.height.saturating_sub(strip * ch))
            }
        };
        if l.counts[index] == 0 {
            return Ok(vec![0.0; rows * cw]); // a sparse chunk (GDAL): zeros
        }
        let expected = rows * cw * samples * l.dtype.bytes();
        let raw = pread(&self.file, l.offsets[index], l.counts[index] as usize)?;
        let all = chunk_values(l, decompress(l.compression, &raw, expected)?, cw, samples)?;
        Ok(if samples == 1 { all } else { all.into_iter().skip(band).step_by(samples).collect() })
    }

    /// A chunk of a level decoded, one band, as f32 (to show); kept when `keep`.
    fn chunk(&self, level: usize, index: usize, band: usize, keep: bool) -> Result<Arc<Vec<f32>>> {
        let key = (level, index, band);
        if let Some(v) = self.chunks.lock().unwrap().get(&key) {
            return Ok(v);
        }
        let v = Arc::new(self.decode(level, index, band)?.into_iter().map(|v| v as f32).collect::<Vec<f32>>());
        if keep {
            self.chunks.lock().unwrap().put(key, v.clone());
        }
        Ok(v)
    }

    /// The index of the chunk holding a level's cell (`x`, `y`) of a band.
    fn chunk_of(l: &Layout, x: usize, y: usize, band: usize) -> usize {
        let (cw, ch) = l.chunk();
        let across = l.width.div_ceil(cw);
        let per_plane = across * l.height.div_ceil(ch);
        let plane = if l.planar == 2 { band } else { 0 };
        plane * per_plane + (y / ch) * across + x / cw
    }

    /// The cells of a window (the image's cells) at every `step`-th row and column, one band: the cell at the
    /// top-left of each step x step block, from the coarsest overview that is not coarser than `step`;
    /// `ceil(width / step)` x `ceil(height / step)` values, row by row, NaN outside the image (no-data values
    /// as they are).
    pub fn sample(&self, band: usize, win: Window, step: usize) -> Result<Vec<f32>> {
        if band >= self.meta.bands.len() {
            return Err(Error::Invalid(format!("no band {band}")));
        }
        let step = step.max(1);
        let (ow, oh) = (win.width.div_ceil(step), win.height.div_ceil(step));
        let mut out = vec![f32::NAN; ow * oh];
        let level = self.levels.iter().rposition(|(f, _)| *f <= step).unwrap_or(0);
        let (f, l) = (&self.levels[level].0, &self.levels[level].1);
        // the level's rows and columns taken (none outside the image)
        let at = |start: i64, k: usize, size: usize| -> Option<usize> {
            let full = start + (k * step) as i64;
            (full >= 0).then(|| full as usize / f).filter(|&c| c < size)
        };
        let cols: Vec<Option<usize>> = (0..ow).map(|j| at(win.x0, j, l.width)).collect();
        let rows: Vec<Option<usize>> = (0..oh).map(|i| at(win.y0, i, l.height)).collect();
        // the chunks holding them, decoded in parallel
        let (cw, ch) = l.chunk();
        let mut needed: Vec<usize> = Vec::new();
        {
            let mut seen = std::collections::HashSet::new();
            let col_chunks: Vec<usize> = {
                let mut v: Vec<usize> = cols.iter().flatten().map(|&x| x / cw).collect();
                v.dedup();
                v
            };
            for &y in rows.iter().flatten() {
                for &cx in &col_chunks {
                    let k = Self::chunk_of(l, cx * cw, y, band);
                    if seen.insert(k) {
                        needed.push(k);
                    }
                }
            }
        }
        // a dense read (a few cells per step) is kept for the windows around it
        let keep = step / f <= 4;
        let decoded: HashMap<usize, Arc<Vec<f32>>> =
            needed.par_iter().map(|&k| self.chunk(level, k, band, keep).map(|v| (k, v))).collect::<Result<_>>()?;
        for (i, y) in rows.iter().enumerate() {
            let Some(y) = *y else { continue };
            let row = &mut out[i * ow..(i + 1) * ow];
            for (j, x) in cols.iter().enumerate() {
                let Some(x) = *x else { continue };
                let k = Self::chunk_of(l, x, y, band);
                let chunk = &decoded[&k];
                let (cx, cy) = (x % cw, y % ch);
                row[j] = chunk.get(cy * cw + cx).copied().unwrap_or(f32::NAN);
            }
        }
        Ok(out)
    }

    /// A window's cells at full resolution, one band, exactly: NaN outside the image (no-data values as they
    /// are); the chunks it touches decoded in parallel.
    pub fn read_window(&self, band: usize, win: Window) -> Result<Vec<f64>> {
        if band >= self.meta.bands.len() {
            return Err(Error::Invalid(format!("no band {band}")));
        }
        let l = &self.levels[0].1;
        let (cw, ch) = l.chunk();
        let mut out = vec![f64::NAN; win.width * win.height];
        let (x0, y0) = (win.x0.max(0) as usize, win.y0.max(0) as usize);
        let x1 = (win.x0 + win.width as i64).clamp(0, l.width as i64) as usize;
        let y1 = (win.y0 + win.height as i64).clamp(0, l.height as i64) as usize;
        if x0 >= x1 || y0 >= y1 {
            return Ok(out);
        }
        let keys: Vec<(usize, usize)> =
            (y0 / ch..y1.div_ceil(ch)).flat_map(|ty| (x0 / cw..x1.div_ceil(cw)).map(move |tx| (tx, ty))).collect();
        let decoded: Vec<((usize, usize), Vec<f64>)> = keys
            .par_iter()
            .map(|&(tx, ty)| self.decode(0, Self::chunk_of(l, tx * cw, ty * ch, band), band).map(|v| ((tx, ty), v)))
            .collect::<Result<_>>()?;
        for ((tx, ty), chunk) in decoded {
            for y in (ty * ch).max(y0)..((ty + 1) * ch).min(y1) {
                let (cx0, cx1) = ((tx * cw).max(x0), ((tx + 1) * cw).min(x1));
                let from = (y - ty * ch) * cw + (cx0 - tx * cw);
                let to = (y as i64 - win.y0) as usize * win.width + (cx0 as i64 - win.x0) as usize;
                out[to..to + (cx1 - cx0)].copy_from_slice(&chunk[from..from + (cx1 - cx0)]);
            }
        }
        Ok(out)
    }

    /// A whole band at full resolution, its chunks decoded in parallel (no-data values as they are).
    pub fn read_band(&self, band: usize) -> Result<Vec<f64>> {
        if band >= self.meta.bands.len() {
            return Err(Error::Invalid(format!("no band {band}")));
        }
        let l = &self.levels[0].1;
        let (w, h) = (l.width, l.height);
        let (cw, ch) = l.chunk();
        let across = w.div_ceil(cw);
        let mut out = vec![f64::NAN; w * h];
        // a band of chunks (a strip, or a row of tiles) per task, written into its own rows
        out.par_chunks_mut(w * ch).enumerate().try_for_each(|(ty, rows)| -> Result<()> {
            for tx in 0..across {
                let k = Self::chunk_of(l, tx * cw, ty * ch, band);
                let chunk = self.decode(0, k, band)?;
                let (x0, n) = (tx * cw, rows.len() / w);
                let wid = cw.min(w - x0);
                for r in 0..n {
                    rows[r * w + x0..r * w + x0 + wid].copy_from_slice(&chunk[r * cw..r * cw + wid]);
                }
            }
            Ok(())
        })?;
        Ok(out)
    }
}
