//! Preparing data, as landschaft's "Data & configuration": rasters reprojected, resampled, put on the grid of
//! another, clipped, converted, joined into a mosaic, measured as a surface (slope, aspect...), their gaps
//! filled, their distances to some values measured (GDAL, see `terraflux_gdal`), reclassified (by Terraflux).
//!
//! The rasters made are files (GeoTIFF): at the path given, else in a cache folder of the session (named by
//! what made them); a series (a folder of files) is done file by file into a folder.

use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::{DataType, Grid, Raster};

use crate::files::Rasters;
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Reproject));
    r.add(Arc::new(Match));
    r.add(Arc::new(Clip));
    r.add(Arc::new(Reclassify));
    r.add(Arc::new(Resample));
    r.add(Arc::new(Convert));
    r.add(Arc::new(Mosaic));
    r.add(Arc::new(Terrain));
    r.add(Arc::new(Fill));
    r.add(Arc::new(Proximity));
}

/// The rasters a value is: files on disk (a map or a stack in memory written to a temporary GeoTIFF first).
fn rasters_of(v: &Value, node: &str) -> Result<Arc<Rasters>> {
    match v {
        Value::Raster(r) => Ok(r.clone()),
        Value::Map(_) | Value::Stack(_) => {
            let path = cache_dir().join(format!("{}-{node}-in.tif", std::process::id()));
            let raster = match v {
                Value::Map(m) => Raster {
                    grid: (*m.grid).clone(),
                    bands: vec![terraflux_raster::Band { name: None, data: m.data.to_vec() }],
                    dtype: DataType::F64,
                    nodata: Some(f64::NAN),
                },
                Value::Stack(s) => Raster {
                    grid: (*s.grid).clone(),
                    bands: s
                        .keys
                        .iter()
                        .zip(&s.layers)
                        .map(|(k, l)| terraflux_raster::Band { name: Some(k.to_string()), data: l.to_vec() })
                        .collect(),
                    dtype: DataType::F64,
                    nodata: Some(f64::NAN),
                },
                _ => unreachable!(),
            };
            let refs: Vec<&[f64]> = raster.bands.iter().map(|b| b.data.as_slice()).collect();
            let (dtype, nodata) = DataType::fitting(&refs, None);
            let raster = Raster { dtype, nodata, ..raster };
            write_with_crs(&raster, &path)?;
            Ok(Arc::new(Rasters::open(&path.to_string_lossy(), None, None)?))
        }
        v => Err(Error::msg(format!("node {node:?}: a {:?} is not a raster", v.ty()))),
    }
}

/// Write a raster; its CRS given by GDAL afterwards when it has a WKT and no GeoTIFF keys.
pub fn write_with_crs(r: &Raster, path: &Path) -> Result<()> {
    r.write(path).map_err(|e| Error::msg(format!("{}: {e}", path.display())))?;
    if r.grid.geo.geokeys.is_none()
        && let Some(wkt) = &r.grid.geo.wkt
    {
        terraflux_gdal::set_crs(path, wkt).map_err(Error::msg)?;
    }
    Ok(())
}

/// The folder of the rasters made without a path.
pub fn cache_dir() -> PathBuf {
    let d = std::env::temp_dir().join("terraflux-cache");
    let _ = std::fs::create_dir_all(&d);
    d
}

/// Where a node's rasters go: the path given, else the cache (named by the inputs' files and the settings).
fn destination(args: &Args, sources: &Rasters, what: &str) -> Result<PathBuf> {
    if let Some(p) = args.opt_param::<String>("path")?.filter(|p| !p.is_empty()) {
        return Ok(PathBuf::from(p));
    }
    let mut h = std::collections::hash_map::DefaultHasher::new();
    what.hash(&mut h);
    for f in &sources.images {
        f.path().hash(&mut h);
        if let Ok(m) = std::fs::metadata(f.path()) {
            (m.len(), m.modified().ok()).hash(&mut h);
        }
    }
    serde_json::to_string(args.params).unwrap_or_default().hash(&mut h);
    Ok(cache_dir().join(format!("{what}-{:016x}", h.finish())))
}

/// Run a GDAL utility on every file of rasters (a file: `dest` is the file; a series: `dest` a folder of files
/// of the same names); the rasters made, opened.
fn each_file(
    sources: &Rasters,
    dest: &Path,
    run: &dyn Fn(&Path, &Path) -> std::result::Result<(), String>,
) -> Result<Arc<Rasters>> {
    let files: Vec<PathBuf> = sources.images.iter().map(|f| f.path()).collect();
    if files.len() == 1 {
        let out = if dest.extension().is_none() { dest.with_extension("tif") } else { dest.to_path_buf() };
        if let Some(d) = out.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d).map_err(|e| Error::msg(format!("{}: {e}", d.display())))?;
        }
        let _ = std::fs::remove_file(&out);
        run(&files[0], &out).map_err(Error::msg)?;
        return Ok(Arc::new(Rasters::open(&out.to_string_lossy(), None, None)?.choose(sources.chosen)));
    }
    std::fs::create_dir_all(dest).map_err(|e| Error::msg(format!("{}: {e}", dest.display())))?;
    for f in &files {
        let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let out = dest.join(format!("{name}.tif"));
        let _ = std::fs::remove_file(&out);
        run(f, &out).map_err(|e| Error::msg(format!("{}: {e}", f.display())))?;
    }
    Ok(Arc::new(Rasters::open(&dest.to_string_lossy(), None, None)?.choose(sources.chosen)))
}

/// A CRS as GDAL takes it: the WKT GDAL read, else the EPSG code of the GeoTIFF keys.
fn crs_of(g: &Grid) -> Option<String> {
    g.geo.wkt.clone().or_else(|| g.geo.epsg().map(|c| format!("EPSG:{c}")))
}

/// GDAL's resampling: the one given, else by the data: classes (whole numbers) the nearest cell, or the most
/// common when the cells grow; values the bilinear, or the mean when the cells grow.
fn resampling(args: &Args, r: &Rasters, coarser: bool) -> Result<String> {
    let given: String = args.param("resampling", "auto".to_string())?;
    if given != "auto" {
        return Ok(given);
    }
    let classes = !r.dtype().is_float();
    Ok(match (classes, coarser) {
        (true, false) => "near",
        (true, true) => "mode",
        (false, false) => "bilinear",
        (false, true) => "average",
    }
    .to_string())
}

/// Extra arguments of a GDAL utility, as on its command line.
fn extra(args: &Args) -> Result<Vec<String>> {
    Ok(args
        .opt_param::<String>("options")?
        .map(|o| o.split_whitespace().map(String::from).collect())
        .unwrap_or_default())
}

fn strings(v: &[&str]) -> Vec<String> {
    v.iter().map(|s| s.to_string()).collect()
}

/// A resolution setting: a side, or [x, y].
fn resolution(args: &Args) -> Result<Option<(f64, f64)>> {
    match args.params.get("resolution") {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(n.as_f64().map(|x| (x, x))),
        Some(v) => {
            let xy: Vec<f64> = serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: resolution: {e}", args.node)))?;
            match xy.as_slice() {
                [x, y] => Ok(Some((*x, *y))),
                _ => Err(Error::msg(format!("node {:?}: resolution is a side or [x, y]", args.node))),
            }
        }
    }
}

fn need_gdal(node: &str) -> Result<()> {
    terraflux_gdal::gdal().map(|_| ()).map_err(|e| Error::msg(format!("node {node:?} needs GDAL: {e}")))
}

struct Reproject;

impl Operator for Reproject {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.reproject",
            "Rasters in another coordinate reference system and/or resolution (GDAL's gdalwarp): the method by \
             the data unless one is given (classes: the nearest cell, or the most common when the cells grow; \
             values: bilinear, or the mean when the cells grow). A series is done file by file.",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Raster, "the rasters"))
        .output(PortSpec::new("raster", Type::Raster, "the rasters reprojected"))
        .param(PortSpec::optional(
            "crs",
            Type::Text,
            "the new CRS: EPSG:4326, a WKT, a PROJ string (default: the same)",
        ))
        .param(PortSpec::optional("resolution", Type::Any, "the new cells' side, or [x, y], in the new CRS's units"))
        .param(PortSpec::optional(
            "resampling",
            Type::Text,
            "auto (default), near, bilinear, cubic, average, mode, min, max...",
        ))
        .param(PortSpec::optional("source_crs", Type::Text, "the rasters' CRS, when their files do not say it"))
        .param(PortSpec::optional("extent", Type::Any, "[xmin, ymin, xmax, ymax] in the new CRS"))
        .param(PortSpec::optional("options", Type::Text, "more of gdalwarp's options, as on its command line"))
        .param(PortSpec::optional("path", Type::Text, "the file (or folder, for a series) made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the rasters"))?, args.node)?;
        let res = resolution(args)?;
        let crs = args.opt_param::<String>("crs")?.filter(|c| !c.trim().is_empty());
        if crs.is_none() && res.is_none() {
            return Err(Error::msg(format!("node {:?}: give a CRS or a resolution", args.node)));
        }
        let (sx, _) = src.grid.resolution();
        let coarser = res.is_some_and(|(x, _)| crs.is_none() && x > sx * 1.5);
        let mut a = strings(&["-r", &resampling(args, &src, coarser)?, "-multi", "-wo", "NUM_THREADS=ALL_CPUS"]);
        a.extend(strings(&["-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-co", "BIGTIFF=IF_SAFER"]));
        if let Some(c) = &crs {
            a.extend(strings(&["-t_srs", c]));
        }
        if let Some(c) = args.opt_param::<String>("source_crs")?.filter(|c| !c.trim().is_empty()) {
            a.extend(strings(&["-s_srs", &c]));
        }
        if let Some((x, y)) = res {
            a.extend(strings(&["-tr", &x.to_string(), &y.to_string()]));
        }
        if let Some(e) = args.opt_param::<Vec<f64>>("extent")? {
            if e.len() != 4 {
                return Err(Error::msg(format!("node {:?}: extent is [xmin, ymin, xmax, ymax]", args.node)));
            }
            a.push("-te".into());
            a.extend(e.iter().map(|v| v.to_string()));
        }
        a.extend(extra(args)?);
        let dest = destination(args, &src, "reproject")?;
        let out = each_file(&src, &dest, &|from, to| terraflux_gdal::warp(&[from], to, &a))?;
        Ok(outputs([("raster", Value::Raster(out))]))
    }
}

struct Match;

impl Operator for Match {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.match",
            "Rasters on the grid of another (its CRS, extent and cells: landschaft's match_grid, by gdalwarp), so \
             that their cells are the same cells; the method by the data unless one is given.",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Raster, "the rasters"))
        .input(PortSpec::new("like", Type::Raster, "the raster whose grid to take"))
        .output(PortSpec::new("raster", Type::Raster, "the rasters on that grid"))
        .param(PortSpec::optional("resampling", Type::Text, "auto (default), near, bilinear, average, mode..."))
        .param(PortSpec::optional("source_crs", Type::Text, "the rasters' CRS, when their files do not say it"))
        .param(PortSpec::optional("path", Type::Text, "the file (or folder, for a series) made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the rasters"))?, args.node)?;
        let like = rasters_of(args.get("like").ok_or_else(|| Error::msg("connect the grid's raster"))?, args.node)?;
        let g = &like.grid;
        let t = g.transform;
        let (xmin, ymax) = (t[0], t[3]);
        let (xmax, ymin) = (t[0] + t[1] * g.width as f64, t[3] + t[5] * g.height as f64);
        let coarser = g.resolution().0 > src.grid.resolution().0 * 1.5;
        let mut a = strings(&["-r", &resampling(args, &src, coarser)?, "-multi", "-wo", "NUM_THREADS=ALL_CPUS"]);
        a.extend(strings(&["-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-co", "BIGTIFF=IF_SAFER"]));
        a.extend(strings(&[
            "-te",
            &xmin.to_string(),
            &ymin.min(ymax).to_string(),
            &xmax.to_string(),
            &ymax.max(ymin).to_string(),
        ]));
        a.extend(strings(&["-ts", &g.width.to_string(), &g.height.to_string()]));
        if let Some(c) = crs_of(g) {
            a.extend(strings(&["-t_srs", &c]));
        }
        if let Some(c) = args.opt_param::<String>("source_crs")?.filter(|c| !c.trim().is_empty()) {
            a.extend(strings(&["-s_srs", &c]));
        }
        let dest = destination(args, &src, "match")?;
        let out = each_file(&src, &dest, &|from, to| terraflux_gdal::warp(&[from], to, &a))?;
        Ok(outputs([("raster", Value::Raster(out))]))
    }
}

struct Clip;

impl Operator for Clip {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.clip",
            "Rasters cut to an extent in their own coordinates (GDAL's gdal_translate), their cells as they are.",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Raster, "the rasters"))
        .output(PortSpec::new("raster", Type::Raster, "the rasters cut"))
        .param(PortSpec::new("extent", Type::Any, "[xmin, ymin, xmax, ymax]"))
        .param(PortSpec::optional("path", Type::Text, "the file (or folder, for a series) made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the rasters"))?, args.node)?;
        let e: Vec<f64> = args.param("extent", Vec::new())?;
        let [xmin, ymin, xmax, ymax] = e.as_slice() else {
            return Err(Error::msg(format!("node {:?}: extent is [xmin, ymin, xmax, ymax]", args.node)));
        };
        let a = strings(&[
            "-projwin",
            &xmin.to_string(),
            &ymax.to_string(),
            &xmax.to_string(),
            &ymin.to_string(),
            "-co",
            "COMPRESS=DEFLATE",
            "-co",
            "TILED=YES",
            "-co",
            "BIGTIFF=IF_SAFER",
        ]);
        let dest = destination(args, &src, "clip")?;
        let out = each_file(&src, &dest, &|from, to| terraflux_gdal::translate(from, to, &a))?;
        Ok(outputs([("raster", Value::Raster(out))]))
    }
}

/// The options of a GDAL-made GeoTIFF: deflated, tiled, BigTIFF when large.
fn tiff_options() -> Vec<String> {
    strings(&["-co", "COMPRESS=DEFLATE", "-co", "TILED=YES", "-co", "BIGTIFF=IF_SAFER"])
}

struct Resample;

impl Operator for Resample {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.resample",
            "Rasters at another resolution in their own CRS (gdalwarp): the method by the data unless one is given \
             (classes: the nearest cell, or the most common when the cells grow; values: bilinear, or the mean \
             when the cells grow).",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Raster, "the rasters"))
        .output(PortSpec::new("raster", Type::Raster, "the rasters resampled"))
        .param(PortSpec::new("resolution", Type::Any, "the new cells' side, or [x, y], in the rasters' units"))
        .param(PortSpec::optional("resampling", Type::Text, "auto (default), near, bilinear, cubic, average, mode..."))
        .param(PortSpec::optional("path", Type::Text, "the file (or folder, for a series) made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the rasters"))?, args.node)?;
        let (x, y) =
            resolution(args)?.ok_or_else(|| Error::msg(format!("node {:?}: give the resolution", args.node)))?;
        let coarser = x > src.grid.resolution().0 * 1.01;
        let mut a = strings(&["-r", &resampling(args, &src, coarser)?, "-tr", &x.to_string(), &y.to_string()]);
        a.extend(strings(&["-multi", "-wo", "NUM_THREADS=ALL_CPUS"]));
        a.extend(tiff_options());
        let dest = destination(args, &src, "resample")?;
        let out = each_file(&src, &dest, &|from, to| terraflux_gdal::warp(&[from], to, &a))?;
        Ok(outputs([("raster", Value::Raster(out))]))
    }
}

/// The usual extension of a GDAL format's files.
fn extension(format: &str) -> &'static str {
    match format.to_ascii_lowercase().as_str() {
        "hfa" => "img",
        "netcdf" => "nc",
        "vrt" => "vrt",
        "aaigrid" => "asc",
        "jp2openjpeg" => "jp2",
        "kea" => "kea",
        "envi" => "bin",
        "saga" => "sdat",
        _ => "tif",
    }
}

struct Convert;

impl Operator for Convert {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.convert",
            "Rasters copied into another format or cell type (gdal_translate): GTiff, COG (cloud-optimised, with \
             overviews), HFA (Erdas .img), netCDF, KEA...; the type Byte, Int16, UInt16, Int32, Float32...",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Raster, "the rasters"))
        .output(PortSpec::new("raster", Type::Raster, "the rasters converted"))
        .param(PortSpec::optional("format", Type::Text, "GDAL's format (default GTiff; COG, HFA, netCDF...)"))
        .param(PortSpec::optional("type", Type::Text, "the cell type: Byte, Int16, UInt16, Int32, Float32, Float64"))
        .param(PortSpec::optional("options", Type::Text, "more of gdal_translate's options, as on its command line"))
        .param(PortSpec::optional("path", Type::Text, "the file (or folder, for a series) made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the rasters"))?, args.node)?;
        let format: String = args.param("format", "GTiff".to_string())?;
        let mut a = strings(&["-of", &format]);
        if let Some(t) = args.opt_param::<String>("type")?.filter(|t| !t.is_empty()) {
            a.extend(strings(&["-ot", &t]));
        }
        if format.eq_ignore_ascii_case("GTiff") {
            a.extend(tiff_options());
        }
        a.extend(extra(args)?);
        let ext = extension(&format);
        let files: Vec<PathBuf> = src.images.iter().map(|f| f.path()).collect();
        let mut dest = destination(args, &src, "convert")?;
        if files.len() == 1 {
            if dest.extension().is_none() {
                dest = dest.with_extension(ext);
            }
            if let Some(d) = dest.parent().filter(|d| !d.as_os_str().is_empty()) {
                std::fs::create_dir_all(d).map_err(|e| Error::msg(format!("{}: {e}", d.display())))?;
            }
            let _ = std::fs::remove_file(&dest);
            terraflux_gdal::translate(&files[0], &dest, &a).map_err(Error::msg)?;
            let r = Rasters::open(&dest.to_string_lossy(), None, None)?.choose(src.chosen);
            return Ok(outputs([("raster", Value::Raster(Arc::new(r)))]));
        }
        std::fs::create_dir_all(&dest).map_err(|e| Error::msg(format!("{}: {e}", dest.display())))?;
        for f in &files {
            let name = f.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let out = dest.join(format!("{name}.{ext}"));
            let _ = std::fs::remove_file(&out);
            terraflux_gdal::translate(f, &out, &a).map_err(|e| Error::msg(format!("{}: {e}", f.display())))?;
        }
        let r = Rasters::open(&dest.to_string_lossy(), None, None)?.choose(src.chosen);
        Ok(outputs([("raster", Value::Raster(Arc::new(r)))]))
    }
}

struct Mosaic;

impl Operator for Mosaic {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.mosaic",
            "The files of a series (a folder, a pattern: tiles of a region) joined into one raster (gdalwarp), in \
             the CRS of the first unless another is given; where they overlap, the cells of the last.",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Raster, "the rasters: a series of files"))
        .output(PortSpec::new("raster", Type::Raster, "the mosaic"))
        .param(PortSpec::optional("crs", Type::Text, "the CRS of the mosaic (default: the first file's)"))
        .param(PortSpec::optional("resolution", Type::Any, "the side of the mosaic's cells, or [x, y]"))
        .param(PortSpec::optional("resampling", Type::Text, "auto (default), near, bilinear, average, mode..."))
        .param(PortSpec::optional("path", Type::Text, "the file made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the rasters"))?, args.node)?;
        let mut a = strings(&["-r", &resampling(args, &src, false)?, "-multi", "-wo", "NUM_THREADS=ALL_CPUS"]);
        a.extend(tiff_options());
        if let Some(c) = args.opt_param::<String>("crs")?.filter(|c| !c.trim().is_empty()) {
            a.extend(strings(&["-t_srs", &c]));
        }
        if let Some((x, y)) = resolution(args)? {
            a.extend(strings(&["-tr", &x.to_string(), &y.to_string()]));
        }
        let dest = destination(args, &src, "mosaic")?.with_extension("tif");
        if let Some(d) = dest.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(d).map_err(|e| Error::msg(format!("{}: {e}", d.display())))?;
        }
        let _ = std::fs::remove_file(&dest);
        let files: Vec<PathBuf> = src.images.iter().map(|f| f.path()).collect();
        let refs: Vec<&Path> = files.iter().map(PathBuf::as_path).collect();
        terraflux_gdal::warp(&refs, &dest, &a).map_err(Error::msg)?;
        let r = Rasters::open(&dest.to_string_lossy(), None, None)?;
        Ok(outputs([("raster", Value::Raster(Arc::new(r)))]))
    }
}

struct Terrain;

impl Operator for Terrain {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.terrain",
            "A measure of a surface such as an elevation model (gdaldem): slope (degrees, or percent with the \
             option -p), aspect, hillshade, TRI (terrain ruggedness), TPI (topographic position) or roughness. For \
             a surface in degrees of latitude and longitude, the option -s 111120 gives the slope in metres.",
            Access::Halo(1),
        )
        .input(PortSpec::new("raster", Type::Raster, "the surface"))
        .output(PortSpec::new("raster", Type::Raster, "the measure"))
        .param(PortSpec::optional("measure", Type::Text, "slope (default), aspect, hillshade, TRI, TPI, roughness"))
        .param(PortSpec::optional("options", Type::Text, "more of gdaldem's options, as on its command line"))
        .param(PortSpec::optional("path", Type::Text, "the file (or folder, for a series) made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the surface"))?, args.node)?;
        let measure: String = args.param("measure", "slope".to_string())?;
        let mut a = strings(&["-b", &(src.layers[src.chosen].band + 1).to_string(), "-compute_edges"]);
        a.extend(tiff_options());
        a.extend(extra(args)?);
        let dest = destination(args, &src, &format!("terrain-{measure}"))?;
        let out = each_file(&src, &dest, &|from, to| terraflux_gdal::dem(from, to, &measure, &a))?;
        Ok(outputs([("raster", Value::Raster(Arc::new(out.choose(0))))]))
    }
}

struct Fill;

impl Operator for Fill {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.fill",
            "The cells without a value filled from the cells around them (gdal_fillnodata: inverse distance \
             weighting, up to a distance), then smoothed if asked.",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Raster, "the rasters"))
        .output(PortSpec::new("raster", Type::Raster, "the rasters filled"))
        .param(PortSpec::optional("max_distance", Type::Number, "the farthest a value is taken from, in cells (100)"))
        .param(PortSpec::optional("smoothing", Type::Integer, "smoothing passes over the cells filled (0)"))
        .param(PortSpec::optional("path", Type::Text, "the file (or folder, for a series) made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the rasters"))?, args.node)?;
        let (d, k): (f64, usize) = (args.param("max_distance", 100.0)?, args.param("smoothing", 0)?);
        let dest = destination(args, &src, "fill")?;
        let out = each_file(&src, &dest, &|from, to| terraflux_gdal::fill_nodata(from, to, d, k))?;
        Ok(outputs([("raster", Value::Raster(out))]))
    }
}

struct Proximity;

impl Operator for Proximity {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "distance.proximity",
            "The distance from every cell to the nearest cell of some values (gdal_proximity), in the units of the \
             raster or in cells: a driver such as the distance to roads, rivers or forest edges; done on the file, \
             whatever its size.",
            Access::Global,
        )
        .input(PortSpec::new("raster", Type::Raster, "the raster (its band chosen)"))
        .output(PortSpec::new("raster", Type::Raster, "the distances"))
        .param(PortSpec::new("values", Type::Any, "the values to measure from: a value or a list"))
        .param(PortSpec::optional("units", Type::Text, "geo (default: the units of the raster) or cells"))
        .param(PortSpec::optional("max_distance", Type::Number, "beyond it, no value"))
        .param(PortSpec::optional("path", Type::Text, "the file made; else a cache"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        need_gdal(args.node)?;
        let src = rasters_of(args.get("raster").ok_or_else(|| Error::msg("connect the raster"))?, args.node)?;
        let values: Vec<f64> = match args.params.get("values") {
            Some(serde_json::Value::Number(n)) => vec![n.as_f64().unwrap_or(0.0)],
            Some(v) => serde_json::from_value(v.clone())
                .map_err(|e| Error::msg(format!("node {:?}: values: {e}", args.node)))?,
            None => return Err(Error::msg(format!("node {:?}: give the values", args.node))),
        };
        let units: String = args.param("units", "geo".to_string())?;
        let mut options = vec![
            format!("VALUES={}", values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(",")),
            format!("DISTUNITS={}", if units.eq_ignore_ascii_case("cells") { "PIXEL" } else { "GEO" }),
            "NODATA=-1".to_string(),
        ];
        if let Some(m) = args.opt_param::<f64>("max_distance")? {
            options.push(format!("MAXDIST={m}"));
        }
        let layer = &src.layers[src.chosen];
        let file = src.images[layer.image].path();
        let dest = destination(args, &src, "proximity")?.with_extension("tif");
        let _ = std::fs::remove_file(&dest);
        terraflux_gdal::proximity(&file, layer.band + 1, &dest, &options).map_err(Error::msg)?;
        let r = Rasters::open(&dest.to_string_lossy(), None, None)?;
        Ok(outputs([("raster", Value::Raster(Arc::new(r)))]))
    }
}

/// A rule of a reclassification: the values it takes and the value they get.
#[derive(Debug, Clone, PartialEq)]
enum Take {
    One(f64),
    Range(f64, f64),
    Rest,
}

/// Rules `from -> to`, one per line or separated by `;`: `from` values (`1 2 3`), ranges (`0..10`, both ends
/// in) or `*` (every other value); `to` a number or `nan`.
fn rules(text: &str) -> std::result::Result<Vec<(Vec<Take>, f64)>, String> {
    let mut out = Vec::new();
    for line in text.split(['\n', ';']).map(str::trim).filter(|l| !l.is_empty()) {
        let (from, to) = line.split_once("->").ok_or_else(|| format!("{line:?}: a rule is `from -> to`"))?;
        let to = match to.trim().to_lowercase().as_str() {
            "nan" | "none" | "nodata" => f64::NAN,
            t => t.parse::<f64>().map_err(|_| format!("{line:?}: {t:?} is not a number"))?,
        };
        let mut takes = Vec::new();
        for f in from.split([' ', ',']).map(str::trim).filter(|f| !f.is_empty()) {
            takes.push(if f == "*" {
                Take::Rest
            } else if let Some((a, b)) = f.split_once("..") {
                let p = |s: &str| s.trim().parse::<f64>().map_err(|_| format!("{line:?}: {s:?} is not a number"));
                Take::Range(p(a)?, p(b)?)
            } else {
                Take::One(f.parse::<f64>().map_err(|_| format!("{line:?}: {f:?} is not a number"))?)
            });
        }
        out.push((takes, to));
    }
    Ok(out)
}

struct Reclassify;

impl Operator for Reclassify {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "raster.reclassify",
            "A map's values changed by rules, one per line: `1 2 3 -> 1` (values), `0..10 -> 2` (a range, both \
             ends in), `* -> nan` (every other value); values no rule takes stay as they are. The first rule that \
             takes a value gives it.",
            Access::Local,
        )
        .input(PortSpec::new("map", Type::Map, "the map"))
        .output(PortSpec::new("map", Type::Map, "the map reclassified"))
        .param(PortSpec::new(
            "rules",
            Type::Text,
            "the values -> their new value, a rule per line: `3 -> 1`, `1 2 -> 1`, `0..10 -> 2`, `* -> 0`, `5 -> nan`",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let m = args.map("map")?;
        let text: String = args.param("rules", String::new())?;
        let rules = rules(&text).map_err(|e| Error::msg(format!("node {:?}: {e}", args.node)))?;
        let rest = rules.iter().find(|(t, _)| t.contains(&Take::Rest)).map(|(_, v)| *v);
        let data = m
            .data
            .iter()
            .map(|&v| {
                if v.is_nan() {
                    return v;
                }
                for (takes, to) in &rules {
                    let hit = takes.iter().any(|t| match t {
                        Take::One(x) => v == *x,
                        Take::Range(a, b) => v >= *a && v <= *b,
                        Take::Rest => false,
                    });
                    if hit {
                        return *to;
                    }
                }
                rest.unwrap_or(v)
            })
            .collect();
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), data)))]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_read() {
        let r = rules("1 2 -> 10\n3..5 -> 20; * -> nan").unwrap();
        assert_eq!(r[0], (vec![Take::One(1.0), Take::One(2.0)], 10.0));
        assert_eq!(r[1], (vec![Take::Range(3.0, 5.0)], 20.0));
        assert!(r[2].1.is_nan() && r[2].0 == [Take::Rest]);
        assert!(rules("1 => 2").is_err());
    }
}
