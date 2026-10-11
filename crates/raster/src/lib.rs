//! Grids, rasters and their GeoTIFF files.
//!
//! A [`Raster`] holds one or more bands of the same [`Grid`] (size,
//! georeference and coordinate reference system). Cell values are kept as
//! `f64` whatever the type in the file (exact for every integer type up to
//! 32 bits and for floats); [`Band::masked`] turns no-data cells into NaN,
//! the convention the simulation uses for missing cells.
//!
//! GeoTIFF files are read and written by [`tiff`], in pure Rust: strips or
//! tiles, pixel or band interleaving, no compression, Deflate, LZW or
//! PackBits, horizontal and floating-point predictors, classic TIFF and
//! BigTIFF. The georeference tags (GeoKeys, tie points, pixel scale or
//! transformation) are kept as they are, so a written file has the
//! coordinate reference system of the file it came from; GDAL's no-data value
//! and band descriptions are read and written too.

// bytes to numbers by fixed-size chunks, as readable as as_chunks
#![allow(clippy::chunks_exact_to_as_chunks)]

pub mod image;
pub mod npy;
pub mod resample;
pub mod tiff;

use serde::{Deserialize, Serialize};

/// Errors of reading and writing rasters.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("not a TIFF file")]
    NotTiff,
    #[error("unsupported TIFF: {0}")]
    Unsupported(String),
    #[error("invalid TIFF: {0}")]
    Invalid(String),
    #[error("{0}")]
    Raster(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// The georeference tags of a GeoTIFF, kept as they are in the file.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GeoTags {
    /// ModelPixelScaleTag (33550).
    pub pixel_scale: Option<Vec<f64>>,
    /// ModelTiepointTag (33922).
    pub tiepoints: Option<Vec<f64>>,
    /// ModelTransformationTag (34264).
    pub transformation: Option<Vec<f64>>,
    /// GeoKeyDirectoryTag (34735).
    pub geokeys: Option<Vec<u16>>,
    /// GeoDoubleParamsTag (34736).
    pub double_params: Option<Vec<f64>>,
    /// GeoAsciiParamsTag (34737).
    pub ascii_params: Option<String>,
    /// The coordinate reference system as WKT, when it comes from GDAL (a file of another format) rather than
    /// from GeoTIFF keys.
    /// (Always written: the workers' binary messages carry every field.)
    #[serde(default)]
    pub wkt: Option<String>,
}

impl GeoTags {
    /// The EPSG code of the coordinate reference system, when the GeoKeys give one.
    pub fn epsg(&self) -> Option<u16> {
        let keys = self.geokeys.as_ref()?;
        // header: version, revision, minor, count; then (key, location, count, value) per key
        let n = *keys.get(3)? as usize;
        let mut found = None;
        for k in 0..n {
            let e = keys.get(4 + 4 * k..8 + 4 * k)?;
            // ProjectedCSTypeGeoKey (3072) wins over GeographicTypeGeoKey (2048)
            if e[1] == 0 && (e[0] == 3072 || (e[0] == 2048 && found.is_none())) && e[3] != 32767 {
                found = Some(e[3]);
            }
        }
        found
    }
}

/// The size and georeference of a grid of cells.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grid {
    pub width: usize,
    pub height: usize,
    /// GDAL's geotransform: x of the upper-left corner, cell width, row rotation,
    /// y of the upper-left corner, column rotation, cell height (negative for north-up).
    pub transform: [f64; 6],
    /// The coordinate reference system, as the GeoTIFF tags give it.
    pub geo: GeoTags,
}

impl Grid {
    /// A grid of cells of side 1 without a georeference.
    pub fn new(width: usize, height: usize) -> Self {
        Grid { width, height, transform: [0.0, 1.0, 0.0, 0.0, 0.0, -1.0], geo: GeoTags::default() }
    }

    pub fn cells(&self) -> usize {
        self.width * self.height
    }

    /// Cell width and height (absolute values).
    pub fn resolution(&self) -> (f64, f64) {
        (self.transform[1].abs(), self.transform[5].abs())
    }

    /// Whether another grid has the same size and the same georeference.
    pub fn same_as(&self, other: &Grid) -> bool {
        self.width == other.width && self.height == other.height && self.transform == other.transform
    }
}

/// The type of the cell values in a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DataType {
    U8,
    I8,
    U16,
    I16,
    U32,
    I32,
    U64,
    I64,
    F32,
    F64,
}

impl DataType {
    pub fn bytes(self) -> usize {
        match self {
            DataType::U8 | DataType::I8 => 1,
            DataType::U16 | DataType::I16 => 2,
            DataType::U32 | DataType::I32 | DataType::F32 => 4,
            DataType::U64 | DataType::I64 | DataType::F64 => 8,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, DataType::F32 | DataType::F64)
    }

    /// TIFF's SampleFormat: 1 unsigned, 2 signed, 3 float.
    pub fn sample_format(self) -> u16 {
        match self {
            DataType::U8 | DataType::U16 | DataType::U32 | DataType::U64 => 1,
            DataType::I8 | DataType::I16 | DataType::I32 | DataType::I64 => 2,
            DataType::F32 | DataType::F64 => 3,
        }
    }

    pub fn from_tiff(bits: u16, format: u16) -> Option<DataType> {
        Some(match (format, bits) {
            (1, 8) => DataType::U8,
            (2, 8) => DataType::I8,
            (1, 16) => DataType::U16,
            (2, 16) => DataType::I16,
            (1, 32) => DataType::U32,
            (2, 32) => DataType::I32,
            (1, 64) => DataType::U64,
            (2, 64) => DataType::I64,
            (3, 32) => DataType::F32,
            (3, 64) => DataType::F64,
            _ => return None,
        })
    }

    /// A type by its name (`u8`, `i16`, `f32`...).
    pub fn parse(name: &str) -> Option<DataType> {
        serde_json::from_value(serde_json::Value::String(name.trim().to_ascii_lowercase())).ok()
    }

    /// The type to write maps in: `wanted`, else the smallest that holds their values (u8 for classes 0-254,
    /// i32 for other whole numbers, f32 otherwise); and a no-data value outside the values (NaN for floats).
    pub fn fitting(values: &[&[f64]], wanted: Option<DataType>) -> (DataType, Option<f64>) {
        let finite = || values.iter().flat_map(|v| v.iter()).filter(|x| !x.is_nan());
        let whole = finite().all(|x| x.fract() == 0.0);
        let (lo, hi) = finite().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), &x| (a.min(x), b.max(x)));
        let t = match wanted {
            Some(t) => t,
            None if whole && lo >= 0.0 && hi <= 254.0 => DataType::U8,
            None if whole && lo > -2147483648.0 && hi <= 2147483647.0 => DataType::I32,
            None => DataType::F32,
        };
        let nodata = if t.is_float() {
            Some(f64::NAN)
        } else {
            let (min, max) = t.range();
            if hi < max {
                Some(max)
            } else if lo > min {
                Some(min)
            } else {
                None
            }
        };
        (t, nodata)
    }

    /// The smallest and largest values the type holds.
    pub fn range(self) -> (f64, f64) {
        match self {
            DataType::U8 => (0.0, 255.0),
            DataType::I8 => (-128.0, 127.0),
            DataType::U16 => (0.0, 65535.0),
            DataType::I16 => (-32768.0, 32767.0),
            DataType::U32 => (0.0, 4294967295.0),
            DataType::I32 => (-2147483648.0, 2147483647.0),
            DataType::U64 => (0.0, 18446744073709551615.0),
            DataType::I64 => (-9223372036854775808.0, 9223372036854775807.0),
            DataType::F32 => (f32::MIN as f64, f32::MAX as f64),
            DataType::F64 => (f64::MIN, f64::MAX),
        }
    }
}

/// One band of a raster.
#[derive(Debug, Clone, PartialEq)]
pub struct Band {
    /// The band's description (GDAL's), e.g. a year or a variable name.
    pub name: Option<String>,
    /// The cell values row by row.
    pub data: Vec<f64>,
}

impl Band {
    /// The values with the missing cells (NaN or `nodata`) as NaN.
    pub fn masked(&self, nodata: Option<f64>) -> Vec<f64> {
        match nodata {
            Some(nd) if !nd.is_nan() => self.data.iter().map(|&v| if v == nd { f64::NAN } else { v }).collect(),
            _ => self.data.clone(),
        }
    }
}

/// Bands of the same grid, with the type and no-data value of their file.
#[derive(Debug, Clone, PartialEq)]
pub struct Raster {
    pub grid: Grid,
    pub bands: Vec<Band>,
    pub dtype: DataType,
    pub nodata: Option<f64>,
}

impl Raster {
    /// A raster of one band.
    pub fn single(grid: Grid, data: Vec<f64>, dtype: DataType, nodata: Option<f64>) -> Self {
        Raster { grid, bands: vec![Band { name: None, data }], dtype, nodata }
    }

    /// The band of a 0-based index or of a name.
    pub fn band(&self, key: &str) -> Option<&Band> {
        if let Some(b) = self.bands.iter().find(|b| b.name.as_deref() == Some(key)) {
            return Some(b);
        }
        key.parse::<usize>().ok().and_then(|i| self.bands.get(i))
    }

    /// Band names, the 0-based index for bands without one.
    pub fn band_names(&self) -> Vec<String> {
        self.bands.iter().enumerate().map(|(i, b)| b.name.clone().unwrap_or_else(|| i.to_string())).collect()
    }

    pub fn read(path: impl AsRef<std::path::Path>) -> Result<Raster> {
        tiff::read(path)
    }

    pub fn write(&self, path: impl AsRef<std::path::Path>) -> Result<()> {
        tiff::write(self, path, &tiff::WriteOptions::default())
    }
}
