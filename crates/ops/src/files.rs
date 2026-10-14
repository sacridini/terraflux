//! Rasters read and written by nodes of the graph (`raster.read`, `raster.write`): data brought into a model
//! and results kept, as steps of the model like any other.
//!
//! Reading a raster reads its headers only: the node gives [`Rasters`], files on disk read where and as finely
//! as needed (the viewer reads the windows it shows, at the resolution of the screen); an operator that needs
//! a map or a stack in memory gets it read whole when it runs (the engine does it, see [`Rasters::map`]). A
//! folder, or a pattern such as `forest/*.tif`, gives a series: a layer per file, all of the same grid.
//!
//! The paths of these nodes are relative to the model's folder until the model is expanded
//! (`terraflux_graph::submodel::expand` makes them absolute).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::image::Image;
use terraflux_raster::tiff::{TiledWriter, Window};
use terraflux_raster::{Band, DataType, Grid, Raster};

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Read));
    r.add(Arc::new(Write));
}

/// The most cells a map read whole may have (16 GB as numbers); larger rasters are shown and copied, not held.
pub const MOST_CELLS: usize = 2_000_000_000;

/// A raster file: read by Terraflux itself (GeoTIFF: the fastest, exactly), or by GDAL (any other format, and
/// TIFFs Terraflux does not read, such as JPEG-compressed ones).
pub enum File {
    Own(Image),
    Gdal(terraflux_gdal::Dataset),
}

impl File {
    pub fn open(path: &Path) -> Result<File> {
        let tiff = path.extension().is_some_and(|e| e.eq_ignore_ascii_case("tif") || e.eq_ignore_ascii_case("tiff"));
        let own = if tiff { Some(Image::open(path)) } else { None };
        match own {
            Some(Ok(i)) => Ok(File::Own(i)),
            other => match terraflux_gdal::Dataset::open(path) {
                Ok(d) => Ok(File::Gdal(d)),
                Err(e) => Err(Error::msg(match other {
                    Some(Err(own)) => format!("{}: {own}; and GDAL: {e}", path.display()),
                    _ => format!("{}: {e}", path.display()),
                })),
            },
        }
    }

    pub fn meta(&self) -> &Raster {
        match self {
            File::Own(i) => &i.meta,
            File::Gdal(d) => &d.meta,
        }
    }

    pub fn path(&self) -> PathBuf {
        match self {
            File::Own(i) => i.path.clone(),
            File::Gdal(d) => PathBuf::from(d.path()),
        }
    }

    /// The height of its blocks (strips or tiles).
    pub fn block_rows(&self) -> usize {
        match self {
            File::Own(i) => i.block_rows(),
            File::Gdal(d) => d.block_rows,
        }
    }

    /// Who reads it: "Terraflux" or GDAL's driver.
    pub fn reader(&self) -> String {
        match self {
            File::Own(_) => "Terraflux".into(),
            File::Gdal(d) => format!("GDAL {}", d.driver),
        }
    }

    /// A band's cells at every `step`-th row and column of a window, as f32 (no-data values as they are).
    pub fn sample(&self, band: usize, win: Window, step: usize) -> Result<Vec<f32>> {
        match self {
            File::Own(i) => i.sample(band, win, step).map_err(|e| Error::msg(e.to_string())),
            File::Gdal(d) => {
                let step = step.max(1);
                let (w, h) = (d.meta.grid.width as i64, d.meta.grid.height as i64);
                let (ow, oh) = (win.width.div_ceil(step), win.height.div_ceil(step));
                let mut out = vec![f32::NAN; ow * oh];
                // the output's columns and rows over the image, and the cells they span
                let first = |start: i64| -> usize { if start >= 0 { 0 } else { ((-start) as usize).div_ceil(step) } };
                let (j0, i0) = (first(win.x0), first(win.y0));
                let past = |start: i64, size: i64, n: usize| -> usize {
                    (0..n).take_while(|&k| start + ((k * step) as i64) < size).count()
                };
                let (j1, i1) = (past(win.x0, w, ow), past(win.y0, h, oh));
                if j0 >= j1 || i0 >= i1 {
                    return Ok(out);
                }
                let (x, y) = (win.x0 + (j0 * step) as i64, win.y0 + (i0 * step) as i64);
                let cw = ((win.x0 + (j1 * step) as i64).min(w) - x) as usize;
                let ch = ((win.y0 + (i1 * step) as i64).min(h) - y) as usize;
                let (bw, bh) = (j1 - j0, i1 - i0);
                let v = d.read(band, x as usize, y as usize, cw, ch, bw, bh).map_err(Error::msg)?;
                for r in 0..bh {
                    for c in 0..bw {
                        out[(i0 + r) * ow + j0 + c] = v[r * bw + c] as f32;
                    }
                }
                Ok(out)
            }
        }
    }

    /// A window's cells at full resolution, exactly (no-data values as they are; NaN outside the image).
    pub fn read_window(&self, band: usize, win: Window) -> Result<Vec<f64>> {
        match self {
            File::Own(i) => i.read_window(band, win).map_err(|e| Error::msg(e.to_string())),
            File::Gdal(d) => {
                let (w, h) = (d.meta.grid.width as i64, d.meta.grid.height as i64);
                let mut out = vec![f64::NAN; win.width * win.height];
                let (x0, y0) = (win.x0.max(0), win.y0.max(0));
                let (x1, y1) = ((win.x0 + win.width as i64).min(w), (win.y0 + win.height as i64).min(h));
                if x0 >= x1 || y0 >= y1 {
                    return Ok(out);
                }
                let (cw, ch) = ((x1 - x0) as usize, (y1 - y0) as usize);
                let v = d.read(band, x0 as usize, y0 as usize, cw, ch, cw, ch).map_err(Error::msg)?;
                for r in 0..ch {
                    let to = (y0 - win.y0) as usize + r;
                    let at = to * win.width + (x0 - win.x0) as usize;
                    out[at..at + cw].copy_from_slice(&v[r * cw..(r + 1) * cw]);
                }
                Ok(out)
            }
        }
    }

    /// A whole band, exactly.
    pub fn read_band(&self, band: usize) -> Result<Vec<f64>> {
        match self {
            File::Own(i) => i.read_band(band).map_err(|e| Error::msg(e.to_string())),
            File::Gdal(d) => {
                let (w, h) = (d.meta.grid.width, d.meta.grid.height);
                d.read(band, 0, 0, w, h, w, h).map_err(Error::msg)
            }
        }
    }
}

/// A layer of [`Rasters`]: its name, its file and band, its no-data value.
#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    pub image: usize,
    pub band: usize,
    pub nodata: Option<f64>,
}

/// Rasters on disk of one grid, by layer: a file's bands, or a band of each file of a series.
pub struct Rasters {
    pub grid: Arc<Grid>,
    pub layers: Vec<Layer>,
    pub images: Vec<Arc<File>>,
    /// The layer a map of these rasters is (the band chosen).
    pub chosen: usize,
    /// Whether the files are of different grids (tiles of a region, say): each layer is on its file's grid, and
    /// they make no stack.
    pub mixed: bool,
    /// The layers read whole so far.
    whole: Mutex<HashMap<usize, Arc<Vec<f64>>>>,
}

impl std::fmt::Debug for Rasters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let paths: Vec<PathBuf> = self.images.iter().map(|i| i.path()).collect();
        write!(f, "Rasters({} x {}, {} layers, {paths:?})", self.grid.width, self.grid.height, self.layers.len())
    }
}

/// The extensions of the rasters a folder's files are taken as (GeoTIFF, and GDAL's usual formats).
pub const RASTER_EXTENSIONS: [&str; 16] =
    ["tif", "tiff", "img", "vrt", "nc", "jp2", "hdf", "h5", "asc", "grd", "rst", "sdat", "dem", "bil", "ecw", "kea"];

/// Whether a file name matches a pattern of `*` (any text) and `?` (a character), ignoring case.
fn matches(pattern: &str, name: &str) -> bool {
    let (p, n): (Vec<char>, Vec<char>) =
        (pattern.to_lowercase().chars().collect(), name.to_lowercase().chars().collect());
    // the classic two-pointer match with backtracking to the last star
    let (mut i, mut j, mut star, mut mark) = (0, 0, None, 0);
    while j < n.len() {
        if i < p.len() && (p[i] == '?' || p[i] == n[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    p[i..].iter().all(|c| *c == '*')
}

/// The files a path names: itself, the rasters of a folder, or those matching a pattern in its name.
fn files_of(path: &str) -> Result<Vec<PathBuf>> {
    let p = Path::new(path);
    let raster = |q: &Path| {
        q.extension().is_some_and(|e| {
            let e = e.to_string_lossy().to_lowercase();
            RASTER_EXTENSIONS.contains(&e.as_str())
        })
    };
    let list = |dir: &Path, keep: &dyn Fn(&str) -> bool| -> Result<Vec<PathBuf>> {
        let mut v: Vec<PathBuf> = std::fs::read_dir(dir)
            .map_err(|e| Error::msg(format!("{}: {e}", dir.display())))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|q| q.is_file() && raster(q) && q.file_name().is_some_and(|n| keep(&n.to_string_lossy())))
            .collect();
        v.sort();
        Ok(v)
    };
    let name = p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let found = if name.contains(['*', '?']) {
        let dir = p.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
        list(dir, &|n| matches(&name, n))?
    } else if p.is_dir() {
        list(p, &|_| true)?
    } else {
        vec![p.to_path_buf()]
    };
    if found.is_empty() {
        return Err(Error::msg(format!("{path}: no raster")));
    }
    Ok(found)
}

impl Rasters {
    /// The rasters a path names (a file, a folder, a pattern), headers only; `band` chooses the layer of the
    /// map (by name or 0-based number), `nodata` replaces the files' no-data value.
    pub fn open(path: &str, band: Option<&str>, nodata: Option<f64>) -> Result<Rasters> {
        let files = files_of(path)?;
        let images: Vec<Arc<File>> = files.iter().map(|f| File::open(f).map(Arc::new)).collect::<Result<_>>()?;
        let grid = images[0].meta().grid.clone();
        let mut layers = Vec::new();
        if images.len() == 1 {
            let m = images[0].meta();
            for (b, name) in m.band_names().into_iter().enumerate() {
                layers.push(Layer { name, image: 0, band: b, nodata: nodata.or(m.nodata) });
            }
        } else {
            // every band of every file: a file's name, and its band's when it has more than one
            for (i, (img, f)) in images.iter().zip(&files).enumerate() {
                let stem = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                let names = img.meta().band_names();
                let many = names.len() > 1;
                for (b, band) in names.into_iter().enumerate() {
                    let name = if many { format!("{stem} {band}") } else { stem.clone() };
                    layers.push(Layer { name, image: i, band: b, nodata: nodata.or(img.meta().nodata) });
                }
            }
        }
        let chosen = match band {
            None => 0,
            Some(b) => match layers.iter().position(|l| l.name == b) {
                Some(i) => i,
                None => b
                    .parse::<usize>()
                    .ok()
                    .filter(|&i| i < layers.len())
                    .ok_or_else(|| Error::msg(format!("{path} has no band {b:?}")))?,
            },
        };
        let mixed = images.iter().any(|i| !i.meta().grid.same_as(&grid));
        Ok(Rasters { grid: Arc::new(grid), layers, images, chosen, mixed, whole: Mutex::new(HashMap::new()) })
    }

    /// The same rasters, another layer the map.
    pub fn choose(&self, layer: usize) -> Rasters {
        Rasters {
            grid: self.grid.clone(),
            layers: self.layers.clone(),
            images: self.images.clone(),
            chosen: layer.min(self.layers.len().saturating_sub(1)),
            mixed: self.mixed,
            whole: Mutex::new(self.whole.lock().unwrap().clone()),
        }
    }

    pub fn cells(&self) -> usize {
        self.grid.width * self.grid.height
    }

    /// The grid of a layer (its file's).
    pub fn grid_of(&self, layer: usize) -> &Grid {
        &self.images[self.layers[layer].image].meta().grid
    }

    /// Refused when the files are of different grids.
    fn one_grid(&self, what: &str) -> Result<()> {
        if self.mixed {
            return Err(Error::msg(format!(
                "the files of {} are of different grids: {what} needs one (put them on one grid with raster.match, \
                 or join them with raster.mosaic)",
                self.images[0].path().parent().map_or(String::new(), |p| p.display().to_string())
            )));
        }
        Ok(())
    }

    pub fn dtype(&self) -> DataType {
        self.images[0].meta().dtype
    }

    /// A layer's cells at every `step`-th row and column of a window, as f32, NaN where there is no value.
    pub fn sample(&self, layer: usize, win: Window, step: usize) -> Result<Vec<f32>> {
        let l = &self.layers[layer];
        let mut v = self.images[l.image].sample(l.band, win, step)?;
        if let Some(nd) = l.nodata.filter(|n| !n.is_nan()) {
            let nd = nd as f32;
            for x in v.iter_mut() {
                if *x == nd {
                    *x = f32::NAN;
                }
            }
        }
        Ok(v)
    }

    /// A layer's cells in a window at full resolution, exactly, NaN where there is no value.
    pub fn window(&self, layer: usize, win: Window) -> Result<Vec<f64>> {
        let l = &self.layers[layer];
        let v = self.images[l.image].read_window(l.band, win)?;
        Ok(Band { name: None, data: v }.masked(l.nodata))
    }

    /// A layer read whole (once), NaN where there is no value; refused beyond [`MOST_CELLS`].
    pub fn read(&self, layer: usize) -> Result<Arc<Vec<f64>>> {
        if let Some(v) = self.whole.lock().unwrap().get(&layer) {
            return Ok(v.clone());
        }
        let cells = self.grid_of(layer).cells();
        if cells > MOST_CELLS {
            return Err(Error::msg(format!(
                "{} has {:.1} billion cells ({:.0} GB as numbers): too many to hold in memory; it can be shown, \
                 copied and cut, and analysed in tiles",
                self.images[self.layers[layer].image].path().display(),
                cells as f64 / 1e9,
                cells as f64 * 8.0 / 1e9
            )));
        }
        let l = &self.layers[layer];
        let data = self.images[l.image].read_band(l.band)?;
        let data = Arc::new(Band { name: None, data }.masked(l.nodata));
        self.whole.lock().unwrap().insert(layer, data.clone());
        Ok(data)
    }

    /// The chosen layer as a map (read whole).
    pub fn map(&self) -> Result<Map> {
        Ok(Map { grid: Arc::new(self.grid_of(self.chosen).clone()), data: self.read(self.chosen)? })
    }

    /// Every layer as a stack (read whole).
    pub fn stack(&self) -> Result<Stack> {
        self.one_grid("a stack")?;
        let layers = (0..self.layers.len()).map(|i| self.read(i)).collect::<Result<_>>()?;
        Ok(Stack {
            grid: self.grid.clone(),
            keys: self.layers.iter().map(|l| Key::Name(l.name.clone())).collect(),
            layers,
        })
    }
}

/// A value as an operator's port of type `want` takes it: rasters on disk read whole into a map (the chosen
/// layer) or a stack; others as they are.
pub fn as_port(v: Value, want: Type) -> Result<Value> {
    match (v, want) {
        (Value::Raster(r), Type::Stack) => Ok(Value::Stack(r.stack()?)),
        (Value::Raster(r), Type::Raster) => Ok(Value::Raster(r)),
        (Value::Raster(r), _) => Ok(Value::Map(r.map()?)),
        (v, _) => Ok(v),
    }
}

struct Read;

impl Operator for Read {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.read",
            "Rasters: a file, or a folder or a pattern (forest/*.tif) for every raster in it as one stack (a layer \
             per band of each file, by the files' names; files of different grids are shown, and made one grid \
             by raster.match before they are a stack). Only their headers are read: they are shown at once \
             whatever their size, and read whole when a node needs a map (one band, the first by default) or a \
             stack (every band of every file).",
            Access::Global,
        )
        .output(PortSpec::new("raster", Type::Raster, "the rasters on disk"))
        .output(PortSpec::new("map", Type::Map, "the band chosen, as a map"))
        .output(PortSpec::new("stack", Type::Stack, "every band of every file, as a stack"))
        .param(PortSpec::new(
            "path",
            Type::Text,
            "a file, a folder (a stack) or a pattern, relative to the model's folder",
        ))
        .param(PortSpec::optional("band", Type::Text, "the band of the map, by name or 0-based number"))
        .param(PortSpec::optional("nodata", Type::Number, "a no-data value replacing the files'"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let path: String = args.param("path", String::new())?;
        if path.is_empty() {
            return Err(Error::msg(format!("node {:?}: choose the file", args.node)));
        }
        let band = match args.params.get("band") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(b)) => Some(b.clone()),
            Some(v) => Some(v.to_string()),
        };
        let r = Arc::new(Rasters::open(&path, band.as_deref(), args.opt_param::<f64>("nodata")?)?);
        // every output is the rasters on disk: read whole only where a node needs a map or a stack
        Ok(outputs([
            ("raster", Value::Raster(r.clone())),
            ("map", Value::Raster(r.clone())),
            ("stack", Value::Raster(r)),
        ]))
    }
}

struct Write;

impl Operator for Write {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.write",
            "Write a map, a stack (a band per map), the series of maps of a loop (a band per step) or rasters on \
             disk (copied in blocks, whatever their size) to a GeoTIFF file, in the smallest type that holds the \
             values unless one is given.",
            Access::Global,
        )
        .input(PortSpec::new("value", Type::Raster, "a map, a stack, a series of maps or rasters"))
        .output(PortSpec::new("path", Type::Text, "the file written"))
        .param(PortSpec::new("path", Type::Text, "the file, relative to the model's folder (.tif)"))
        .param(
            PortSpec::optional("type", Type::Text, "the cell type: u8, i16, i32, f32, f64...")
                .choices(["u8", "i8", "u16", "i16", "u32", "i32", "u64", "i64", "f32", "f64"]),
        )
        .param(PortSpec::optional(
            "product",
            Type::Text,
            "the product of GuidosToolbox the map is (MSPA, FAD...), written as landschaft_product: by default that \
             of the operator that made it",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let path: String = args.param("path", String::new())?;
        if path.is_empty() {
            return Err(Error::msg(format!("node {:?}: choose the file", args.node)));
        }
        let wanted = match args.opt_param::<String>("type")? {
            Some(t) => Some(DataType::parse(&t).ok_or_else(|| Error::msg(format!("unknown cell type {t:?}")))?),
            None => None,
        };
        let value = args.get("value").ok_or_else(|| Error::msg(format!("node {:?}: connect a value", args.node)))?;
        let path = Path::new(&path).with_extension("tif");
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(|e| Error::msg(format!("{}: {e}", dir.display())))?;
        }
        let product: Option<String> = args.opt_param::<String>("product")?.filter(|p| !p.is_empty());
        if let Value::Raster(r) = value {
            copy_as(r, &path, wanted, product.as_deref())?;
            return Ok(outputs([("path", Value::Text(path.to_string_lossy().into_owned()))]));
        }
        let (grid, layers): (_, Vec<(Option<String>, &[f64])>) = match value {
            Value::Map(m) => (&*m.grid, vec![(None, m.data.as_slice())]),
            Value::Stack(s) => {
                (&*s.grid, s.keys.iter().zip(&s.layers).map(|(k, l)| (Some(k.to_string()), l.as_slice())).collect())
            }
            Value::Series(items) if !items.is_empty() && items.iter().all(|v| matches!(v, Value::Map(_))) => {
                let Value::Map(first) = &items[0] else { unreachable!() };
                let layers = items
                    .iter()
                    .enumerate()
                    .filter_map(|(i, v)| match v {
                        Value::Map(m) => Some((Some(format!("{}", i + 1)), m.data.as_slice())),
                        _ => None,
                    })
                    .collect();
                (&*first.grid, layers)
            }
            v => return Err(Error::msg(format!("node {:?}: a {:?} is not a raster", args.node, v.ty()))),
        };
        let refs: Vec<&[f64]> = layers.iter().map(|(_, d)| *d).collect();
        let (dtype, nodata) = DataType::fitting(&refs, wanted);
        // the product the values are, not that of the maps they were made from
        let mut grid = grid.clone();
        grid.geo.set_item("landschaft_product", product.as_deref());
        let raster = Raster {
            grid,
            bands: layers.into_iter().map(|(name, d)| Band { name, data: d.to_vec() }).collect(),
            dtype,
            nodata,
        };
        crate::prepare::write_with_crs(&raster, &path)?;
        Ok(outputs([("path", Value::Text(path.to_string_lossy().into_owned()))]))
    }
}

/// A GeoTIFF written row by row, in order, never whole in memory: the rows are kept until they make a row of TIFF
/// tiles (512 x 512), which is then written; at most 512 rows and those of a push are held. One band, or several
/// pushed side by side.
pub struct RowsWriter {
    out: TiledWriter,
    grid: Grid,
    path: PathBuf,
    /// Each band's rows not yet written.
    pending: Vec<Vec<f64>>,
    done: usize,
}

impl RowsWriter {
    const TILE: usize = 512;

    pub fn create(path: &Path, grid: &Grid, name: &str, dtype: DataType, nodata: Option<f64>) -> Result<RowsWriter> {
        Self::create_bands(path, grid, &[name], dtype, nodata)
    }

    /// A GeoTIFF of a band per name.
    pub fn create_bands(
        path: &Path,
        grid: &Grid,
        names: &[&str],
        dtype: DataType,
        nodata: Option<f64>,
    ) -> Result<RowsWriter> {
        let bands = names.iter().map(|n| Some(n.to_string())).collect();
        let out = TiledWriter::create(path, grid, bands, dtype, nodata, Self::TILE, Some(6))
            .map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
        let pending = vec![Vec::new(); names.len()];
        Ok(RowsWriter { out, grid: grid.clone(), path: path.to_path_buf(), pending, done: 0 })
    }

    /// The next rows (a multiple of the width).
    pub fn push(&mut self, rows: impl IntoIterator<Item = f64>) -> Result<()> {
        self.pending[0].extend(rows);
        self.flush(false)
    }

    /// The next rows of every band, as many of each.
    pub fn push_bands(&mut self, rows: &[&[f64]]) -> Result<()> {
        if rows.len() != self.pending.len() || rows.iter().any(|r| r.len() != rows[0].len()) {
            return Err(Error::msg(format!("{}: as many rows of every band", self.path.display())));
        }
        for (p, r) in self.pending.iter_mut().zip(rows) {
            p.extend_from_slice(r);
        }
        self.flush(false)
    }

    fn flush(&mut self, last: bool) -> Result<()> {
        let (nx, ny, t) = (self.grid.width, self.grid.height, Self::TILE);
        while self.pending[0].len() >= t * nx || (last && !self.pending[0].is_empty()) {
            let h = t.min(ny - self.done);
            if self.pending[0].len() < h * nx {
                return Err(Error::msg(format!("{}: rows missing", self.path.display())));
            }
            for col in 0..nx.div_ceil(t) {
                let (x0, w) = (col * t, t.min(nx - col * t));
                let tiles: Vec<Vec<f64>> = self
                    .pending
                    .iter()
                    .map(|p| (0..h).flat_map(|y| p[y * nx + x0..y * nx + x0 + w].iter().copied()).collect())
                    .collect();
                let tiles: Vec<&[f64]> = tiles.iter().map(Vec::as_slice).collect();
                self.out
                    .write_tile(col, self.done / t, &tiles)
                    .map_err(|e| Error::msg(format!("{}: {e}", self.path.display())))?;
            }
            for p in &mut self.pending {
                p.drain(..h * nx);
            }
            self.done += h;
        }
        Ok(())
    }

    /// The last rows written and the file closed (with the grid's coordinate system).
    pub fn finish(mut self) -> Result<()> {
        self.flush(true)?;
        if self.done != self.grid.height {
            return Err(Error::msg(format!("{}: {} of {} rows", self.path.display(), self.done, self.grid.height)));
        }
        let err = |e: terraflux_raster::Error| Error::msg(format!("{}: {e}", self.path.display()));
        self.out.finish().map_err(err)?;
        // a CRS GDAL read (no GeoTIFF keys) given to the file by GDAL
        if self.grid.geo.geokeys.is_none()
            && let Some(wkt) = &self.grid.geo.wkt
        {
            terraflux_gdal::set_crs(&self.path, wkt).map_err(Error::msg)?;
        }
        Ok(())
    }
}

/// Copy rasters on disk to a tiled GeoTIFF, a band per layer, a band of blocks at a time (whatever their size);
/// in their type unless one is given.
pub fn copy(r: &Rasters, path: &Path, wanted: Option<DataType>) -> Result<()> {
    copy_as(r, path, wanted, None)
}

/// [`copy`], the product the rasters are written as landschaft_product (none: the files' own).
pub fn copy_as(r: &Rasters, path: &Path, wanted: Option<DataType>, product: Option<&str>) -> Result<()> {
    const TILE: usize = 256;
    r.one_grid("writing them to one file")?;
    let (w, h) = (r.grid.width, r.grid.height);
    let dtype = wanted.unwrap_or_else(|| r.dtype());
    // the no-data value: NaN for floats, else the files' or one outside the type's values
    let nodata = if dtype.is_float() {
        Some(f64::NAN)
    } else {
        r.layers[0].nodata.filter(|n| !n.is_nan()).or(Some(dtype.range().1))
    };
    let names = r.layers.iter().map(|l| Some(l.name.clone())).collect();
    let err = |e: terraflux_raster::Error| Error::msg(format!("{}: {e}", path.display()));
    let mut grid = (*r.grid).clone();
    if product.is_some() {
        grid.geo.set_item("landschaft_product", product);
    }
    let mut out = TiledWriter::create(path, &grid, names, dtype, nodata, TILE, Some(6)).map_err(err)?;
    for row in 0..h.div_ceil(TILE) {
        let rows = TILE.min(h - row * TILE);
        let win = Window { x0: 0, y0: (row * TILE) as i64, width: w, height: rows };
        let blocks: Vec<Vec<f64>> = (0..r.layers.len()).map(|l| r.window(l, win)).collect::<Result<_>>()?;
        for col in 0..w.div_ceil(TILE) {
            let (x0, cols) = (col * TILE, TILE.min(w - col * TILE));
            let tiles: Vec<Vec<f64>> = blocks
                .iter()
                .map(|b| {
                    let mut t = Vec::with_capacity(cols * rows);
                    for y in 0..rows {
                        t.extend_from_slice(&b[y * w + x0..y * w + x0 + cols]);
                    }
                    t
                })
                .collect();
            let refs: Vec<&[f64]> = tiles.iter().map(Vec::as_slice).collect();
            out.write_tile(col, row, &refs).map_err(err)?;
        }
    }
    out.finish().map_err(err)?;
    // a CRS GDAL read (no GeoTIFF keys) given to the file by GDAL
    if r.grid.geo.geokeys.is_none()
        && let Some(wkt) = &r.grid.geo.wkt
    {
        terraflux_gdal::set_crs(path, wkt).map_err(Error::msg)?;
    }
    Ok(())
}
