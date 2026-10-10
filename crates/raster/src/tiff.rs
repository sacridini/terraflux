//! TIFF and GeoTIFF files: reading (strips or tiles, either interleaving, the
//! usual compressions and predictors, classic TIFF and BigTIFF) and writing
//! (pixel-interleaved strips, Deflate or none, classic TIFF or BigTIFF).

use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::{Band, DataType, Error, GeoTags, Grid, Raster, Result};

// tags
const IMAGE_WIDTH: u16 = 256;
const IMAGE_LENGTH: u16 = 257;
const BITS_PER_SAMPLE: u16 = 258;
const COMPRESSION: u16 = 259;
const PHOTOMETRIC: u16 = 262;
const STRIP_OFFSETS: u16 = 273;
const SAMPLES_PER_PIXEL: u16 = 277;
const ROWS_PER_STRIP: u16 = 278;
const STRIP_BYTE_COUNTS: u16 = 279;
const PLANAR_CONFIG: u16 = 284;
const PREDICTOR: u16 = 317;
const TILE_WIDTH: u16 = 322;
const TILE_LENGTH: u16 = 323;
const TILE_OFFSETS: u16 = 324;
const TILE_BYTE_COUNTS: u16 = 325;
const EXTRA_SAMPLES: u16 = 338;
const SAMPLE_FORMAT: u16 = 339;
const MODEL_PIXEL_SCALE: u16 = 33550;
const MODEL_TIEPOINT: u16 = 33922;
const MODEL_TRANSFORMATION: u16 = 34264;
const GEO_KEY_DIRECTORY: u16 = 34735;
const GEO_DOUBLE_PARAMS: u16 = 34736;
const GEO_ASCII_PARAMS: u16 = 34737;
const GDAL_METADATA: u16 = 42112;
const GDAL_NODATA: u16 = 42113;

// field types
const T_BYTE: u16 = 1;
const T_ASCII: u16 = 2;
const T_SHORT: u16 = 3;
const T_LONG: u16 = 4;
const T_RATIONAL: u16 = 5;
const T_SBYTE: u16 = 6;
const T_UNDEFINED: u16 = 7;
const T_SSHORT: u16 = 8;
const T_SLONG: u16 = 9;
const T_SRATIONAL: u16 = 10;
const T_FLOAT: u16 = 11;
const T_DOUBLE: u16 = 12;
const T_LONG8: u16 = 16;
const T_SLONG8: u16 = 17;
const T_IFD8: u16 = 18;

fn type_size(t: u16) -> Option<usize> {
    Some(match t {
        T_BYTE | T_ASCII | T_SBYTE | T_UNDEFINED => 1,
        T_SHORT | T_SSHORT => 2,
        T_LONG | T_SLONG | T_FLOAT => 4,
        T_RATIONAL | T_SRATIONAL | T_DOUBLE | T_LONG8 | T_SLONG8 | T_IFD8 => 8,
        _ => return None,
    })
}

// ----------------------------------------------------------------------------- reading

#[derive(Clone, Copy)]
struct Endian {
    little: bool,
}

impl Endian {
    fn u16(self, b: &[u8]) -> u16 {
        let a = [b[0], b[1]];
        if self.little { u16::from_le_bytes(a) } else { u16::from_be_bytes(a) }
    }
    fn u32(self, b: &[u8]) -> u32 {
        let a = [b[0], b[1], b[2], b[3]];
        if self.little { u32::from_le_bytes(a) } else { u32::from_be_bytes(a) }
    }
    fn u64(self, b: &[u8]) -> u64 {
        let a: [u8; 8] = b[..8].try_into().unwrap();
        if self.little { u64::from_le_bytes(a) } else { u64::from_be_bytes(a) }
    }
}

/// One IFD entry: its type, count and value bytes (in the file's byte order).
struct Entry {
    typ: u16,
    count: u64,
    data: Vec<u8>,
}

impl Entry {
    fn uints(&self, e: Endian) -> Result<Vec<u64>> {
        let n = self.count as usize;
        Ok(match self.typ {
            T_BYTE | T_UNDEFINED => self.data[..n].iter().map(|&v| v as u64).collect(),
            T_SHORT => (0..n).map(|i| e.u16(&self.data[2 * i..]) as u64).collect(),
            T_LONG => (0..n).map(|i| e.u32(&self.data[4 * i..]) as u64).collect(),
            T_LONG8 | T_IFD8 => (0..n).map(|i| e.u64(&self.data[8 * i..])).collect(),
            t => return Err(Error::Invalid(format!("an integer tag of type {t}"))),
        })
    }

    fn floats(&self, e: Endian) -> Result<Vec<f64>> {
        let n = self.count as usize;
        Ok(match self.typ {
            T_DOUBLE => (0..n).map(|i| f64::from_bits(e.u64(&self.data[8 * i..]))).collect(),
            T_FLOAT => (0..n).map(|i| f32::from_bits(e.u32(&self.data[4 * i..])) as f64).collect(),
            T_RATIONAL => {
                (0..n).map(|i| e.u32(&self.data[8 * i..]) as f64 / e.u32(&self.data[8 * i + 4..]) as f64).collect()
            }
            _ => self.uints(e)?.into_iter().map(|v| v as f64).collect(),
        })
    }

    fn text(&self) -> String {
        let end = self.data.iter().position(|&c| c == 0).unwrap_or(self.data.len());
        String::from_utf8_lossy(&self.data[..end]).into_owned()
    }
}

struct Ifd {
    entries: std::collections::BTreeMap<u16, Entry>,
}

impl Ifd {
    fn get(&self, tag: u16) -> Option<&Entry> {
        self.entries.get(&tag)
    }
    fn uint(&self, tag: u16, e: Endian) -> Result<Option<u64>> {
        match self.get(tag) {
            None => Ok(None),
            Some(x) => Ok(x.uints(e)?.first().copied()),
        }
    }
    fn uints(&self, tag: u16, e: Endian) -> Result<Option<Vec<u64>>> {
        self.get(tag).map(|x| x.uints(e)).transpose()
    }
    fn floats(&self, tag: u16, e: Endian) -> Result<Option<Vec<f64>>> {
        self.get(tag).map(|x| x.floats(e)).transpose()
    }
}

fn read_at(f: &mut File, offset: u64, len: usize) -> Result<Vec<u8>> {
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = vec![0u8; len];
    f.read_exact(&mut buf)?;
    Ok(buf)
}

fn read_ifd(f: &mut File, e: Endian, big: bool, offset: u64) -> Result<Ifd> {
    let (count_len, entry_len, inline) = if big { (8, 20, 8) } else { (2, 12, 4) };
    let head = read_at(f, offset, count_len)?;
    let n = if big { e.u64(&head) } else { e.u16(&head) as u64 } as usize;
    let raw = read_at(f, offset + count_len as u64, n * entry_len)?;
    let mut entries = std::collections::BTreeMap::new();
    for k in 0..n {
        let b = &raw[k * entry_len..(k + 1) * entry_len];
        let tag = e.u16(b);
        let typ = e.u16(&b[2..]);
        let (count, value) = if big { (e.u64(&b[4..]), &b[12..20]) } else { (e.u32(&b[4..]) as u64, &b[8..12]) };
        let Some(size) = type_size(typ) else { continue }; // unknown types are skipped, as libtiff does
        let len = size * count as usize;
        let data = if len <= inline {
            value[..len].to_vec()
        } else {
            let off = if big { e.u64(value) } else { e.u32(value) as u64 };
            read_at(f, off, len)?
        };
        entries.insert(tag, Entry { typ, count, data });
    }
    Ok(Ifd { entries })
}

/// How the cells of a TIFF image are stored.
struct Layout {
    width: usize,
    height: usize,
    spp: usize,
    dtype: DataType,
    planar: u16,
    compression: u16,
    predictor: u16,
    /// rows per strip, or the tile's (width, height)
    tiles: Option<(usize, usize)>,
    rows_per_strip: usize,
    offsets: Vec<u64>,
    counts: Vec<u64>,
    endian: Endian,
}

fn layout(ifd: &Ifd, e: Endian) -> Result<Layout> {
    let need =
        |tag: u16, what: &str| -> Result<u64> { ifd.uint(tag, e)?.ok_or_else(|| Error::Invalid(format!("no {what}"))) };
    let width = need(IMAGE_WIDTH, "image width")? as usize;
    let height = need(IMAGE_LENGTH, "image length")? as usize;
    let spp = ifd.uint(SAMPLES_PER_PIXEL, e)?.unwrap_or(1) as usize;
    let bits = ifd.uints(BITS_PER_SAMPLE, e)?.unwrap_or_else(|| vec![1]);
    let formats = ifd.uints(SAMPLE_FORMAT, e)?.unwrap_or_else(|| vec![1]);
    if bits.iter().any(|&b| b != bits[0]) || formats.iter().any(|&f| f != formats[0]) {
        return Err(Error::Unsupported("bands of different types".into()));
    }
    let dtype = DataType::from_tiff(bits[0] as u16, formats[0] as u16)
        .ok_or_else(|| Error::Unsupported(format!("{} bits of sample format {}", bits[0], formats[0])))?;
    let planar = ifd.uint(PLANAR_CONFIG, e)?.unwrap_or(1) as u16;
    let compression = ifd.uint(COMPRESSION, e)?.unwrap_or(1) as u16;
    let predictor = ifd.uint(PREDICTOR, e)?.unwrap_or(1) as u16;
    let (tiles, rows_per_strip, offsets, counts) = if let Some(tw) = ifd.uint(TILE_WIDTH, e)? {
        let th = need(TILE_LENGTH, "tile length")? as usize;
        let off = ifd.uints(TILE_OFFSETS, e)?.ok_or_else(|| Error::Invalid("no tile offsets".into()))?;
        let cnt = ifd.uints(TILE_BYTE_COUNTS, e)?.ok_or_else(|| Error::Invalid("no tile byte counts".into()))?;
        (Some((tw as usize, th)), 0, off, cnt)
    } else {
        let rps = ifd.uint(ROWS_PER_STRIP, e)?.unwrap_or(height as u64).min(height.max(1) as u64) as usize;
        let off = ifd.uints(STRIP_OFFSETS, e)?.ok_or_else(|| Error::Invalid("no strip offsets".into()))?;
        let cnt = ifd.uints(STRIP_BYTE_COUNTS, e)?.ok_or_else(|| Error::Invalid("no strip byte counts".into()))?;
        (None, rps.max(1), off, cnt)
    };
    if offsets.len() != counts.len() {
        return Err(Error::Invalid("offsets and byte counts differ in number".into()));
    }
    Ok(Layout {
        width,
        height,
        spp,
        dtype,
        planar,
        compression,
        predictor,
        tiles,
        rows_per_strip,
        offsets,
        counts,
        endian: e,
    })
}

fn decompress(compression: u16, raw: &[u8], expected: usize) -> Result<Vec<u8>> {
    let mut out = match compression {
        1 => raw.to_vec(),
        8 | 32946 => {
            let mut d = flate2::read::ZlibDecoder::new(raw);
            let mut v = Vec::with_capacity(expected);
            d.read_to_end(&mut v).map_err(|err| Error::Invalid(format!("Deflate: {err}")))?;
            v
        }
        5 => {
            let mut d = weezl::decode::Decoder::with_tiff_size_switch(weezl::BitOrder::Msb, 8);
            let mut v = Vec::with_capacity(expected);
            let r = d.into_vec(&mut v).decode(raw);
            // a missing end code is tolerated (libtiff does too)
            if let Err(err) = r.status
                && v.len() < expected
            {
                return Err(Error::Invalid(format!("LZW: {err}")));
            }
            v
        }
        32773 => packbits(raw, expected),
        c => return Err(Error::Unsupported(format!("compression {c}"))),
    };
    if out.len() < expected {
        return Err(Error::Invalid(format!("a chunk decodes to {} bytes, {} expected", out.len(), expected)));
    }
    out.truncate(expected);
    Ok(out)
}

fn packbits(raw: &[u8], expected: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(expected);
    let mut i = 0;
    while i < raw.len() && out.len() < expected {
        let n = raw[i] as i8;
        i += 1;
        if n >= 0 {
            let len = n as usize + 1;
            let end = (i + len).min(raw.len());
            out.extend_from_slice(&raw[i..end]);
            i = end;
        } else if n != -128 {
            if i < raw.len() {
                let len = (-(n as i32)) as usize + 1;
                out.extend(std::iter::repeat_n(raw[i], len));
            }
            i += 1;
        }
    }
    out
}

/// The values of a decoded chunk (rows x cols x samples), as f64, after the predictor.
fn chunk_values(l: &Layout, mut bytes: Vec<u8>, cols: usize, samples: usize) -> Result<Vec<f64>> {
    let bps = l.dtype.bytes();
    let row_bytes = cols * samples * bps;
    if l.predictor == 3 {
        if !l.dtype.is_float() {
            return Err(Error::Invalid("the floating-point predictor on integers".into()));
        }
        // libtiff's fpAcc: bytes differenced per row (stride = samples), then regrouped by significance
        let wc = cols * samples;
        let mut tmp = vec![0u8; row_bytes];
        let mut out = Vec::with_capacity(bytes.len() / bps);
        for row in bytes.chunks_exact_mut(row_bytes) {
            for i in samples..row_bytes {
                row[i] = row[i].wrapping_add(row[i - samples]);
            }
            tmp.copy_from_slice(row);
            for c in 0..wc {
                if bps == 4 {
                    let b = [tmp[c], tmp[wc + c], tmp[2 * wc + c], tmp[3 * wc + c]];
                    out.push(f32::from_be_bytes(b) as f64);
                } else {
                    let mut b = [0u8; 8];
                    for (k, v) in b.iter_mut().enumerate() {
                        *v = tmp[k * wc + c];
                    }
                    out.push(f64::from_be_bytes(b));
                }
            }
        }
        return Ok(out);
    }
    // to native byte order
    if !l.endian.little && bps > 1 {
        for v in bytes.chunks_exact_mut(bps) {
            v.reverse();
        }
    }
    if l.predictor == 2 {
        macro_rules! hor {
            ($t:ty) => {{
                for row in bytes.chunks_exact_mut(row_bytes) {
                    let n = row.len() / bps;
                    for i in samples..n {
                        let a =
                            <$t>::from_ne_bytes(row[(i - samples) * bps..(i - samples + 1) * bps].try_into().unwrap());
                        let b = <$t>::from_ne_bytes(row[i * bps..(i + 1) * bps].try_into().unwrap());
                        row[i * bps..(i + 1) * bps].copy_from_slice(&b.wrapping_add(a).to_ne_bytes());
                    }
                }
            }};
        }
        match bps {
            1 => hor!(u8),
            2 => hor!(u16),
            4 if !l.dtype.is_float() => hor!(u32),
            8 if !l.dtype.is_float() => hor!(u64),
            _ => return Err(Error::Invalid("the horizontal predictor on floats".into())),
        }
    } else if l.predictor != 1 {
        return Err(Error::Unsupported(format!("predictor {}", l.predictor)));
    }
    let v: Vec<f64> = match l.dtype {
        DataType::U8 => bytes.iter().map(|&b| b as f64).collect(),
        DataType::I8 => bytes.iter().map(|&b| b as i8 as f64).collect(),
        DataType::U16 => bytes.chunks_exact(2).map(|b| u16::from_ne_bytes([b[0], b[1]]) as f64).collect(),
        DataType::I16 => bytes.chunks_exact(2).map(|b| i16::from_ne_bytes([b[0], b[1]]) as f64).collect(),
        DataType::U32 => bytes.chunks_exact(4).map(|b| u32::from_ne_bytes(b.try_into().unwrap()) as f64).collect(),
        DataType::I32 => bytes.chunks_exact(4).map(|b| i32::from_ne_bytes(b.try_into().unwrap()) as f64).collect(),
        DataType::U64 => bytes.chunks_exact(8).map(|b| u64::from_ne_bytes(b.try_into().unwrap()) as f64).collect(),
        DataType::I64 => bytes.chunks_exact(8).map(|b| i64::from_ne_bytes(b.try_into().unwrap()) as f64).collect(),
        DataType::F32 => bytes.chunks_exact(4).map(|b| f32::from_ne_bytes(b.try_into().unwrap()) as f64).collect(),
        DataType::F64 => bytes.chunks_exact(8).map(|b| f64::from_ne_bytes(b.try_into().unwrap())).collect(),
    };
    Ok(v)
}

/// The cells of the image, band by band.
fn read_bands(f: &mut File, l: &Layout) -> Result<Vec<Vec<f64>>> {
    let (w, h, spp) = (l.width, l.height, l.spp);
    let mut bands = vec![vec![0.0f64; w * h]; spp];
    let bps = l.dtype.bytes();
    let chunk_samples = if l.planar == 2 { 1 } else { spp };
    let (cw, ch) = l.tiles.unwrap_or((w, l.rows_per_strip));
    let across = w.div_ceil(cw);
    let down = h.div_ceil(ch);
    let per_plane = across * down;
    let planes = if l.planar == 2 { spp } else { 1 };
    if l.offsets.len() < per_plane * planes {
        return Err(Error::Invalid(format!("{} chunks for {} expected", l.offsets.len(), per_plane * planes)));
    }
    for plane in 0..planes {
        for ty in 0..down {
            for tx in 0..across {
                let k = plane * per_plane + ty * across + tx;
                let y0 = ty * ch;
                // strips hold only the image's rows; tiles are always whole
                let rows = if l.tiles.is_some() { ch } else { ch.min(h - y0) };
                let expected = rows * cw * chunk_samples * bps;
                let vals = if l.counts[k] == 0 {
                    vec![0.0; rows * cw * chunk_samples] // a sparse chunk (GDAL): zeros
                } else {
                    let raw = read_at(f, l.offsets[k], l.counts[k] as usize)?;
                    chunk_values(l, decompress(l.compression, &raw, expected)?, cw, chunk_samples)?
                };
                let x0 = tx * cw;
                for r in 0..rows.min(h.saturating_sub(y0)) {
                    let y = y0 + r;
                    for c in 0..cw.min(w - x0) {
                        let base = (r * cw + c) * chunk_samples;
                        if l.planar == 2 {
                            bands[plane][y * w + x0 + c] = vals[base];
                        } else {
                            for (s, band) in bands.iter_mut().enumerate() {
                                band[y * w + x0 + c] = vals[base + s];
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(bands)
}

fn geotransform(geo: &GeoTags) -> [f64; 6] {
    if let Some(m) = &geo.transformation
        && m.len() >= 8
    {
        return [m[3], m[0], m[1], m[7], m[4], m[5]];
    }
    if let (Some(s), Some(t)) = (&geo.pixel_scale, &geo.tiepoints)
        && s.len() >= 2
        && t.len() >= 6
    {
        let (sx, sy) = (s[0], s[1]);
        let mut x0 = t[3] - t[0] * sx;
        let mut y0 = t[4] + t[1] * sy;
        if raster_is_point(geo) {
            // PixelIsPoint: the tie point is the centre of the cell (GDAL's default reading)
            x0 -= 0.5 * sx;
            y0 += 0.5 * sy;
        }
        return [x0, sx, 0.0, y0, 0.0, -sy];
    }
    [0.0, 1.0, 0.0, 0.0, 0.0, -1.0]
}

fn raster_is_point(geo: &GeoTags) -> bool {
    let Some(keys) = &geo.geokeys else { return false };
    let n = keys.get(3).copied().unwrap_or(0) as usize;
    (0..n).any(|k| keys.get(4 + 4 * k) == Some(&1025) && keys.get(4 + 4 * k + 3) == Some(&2))
}

fn parse_nodata(s: &str) -> Option<f64> {
    let t = s.trim().trim_end_matches('\0').trim();
    if t.is_empty() {
        return None;
    }
    match t.to_ascii_lowercase().as_str() {
        "nan" | "-nan" => Some(f64::NAN),
        "inf" | "+inf" => Some(f64::INFINITY),
        "-inf" => Some(f64::NEG_INFINITY),
        _ => t.parse::<f64>().ok(),
    }
}

fn xml_unescape(s: &str) -> String {
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&amp;", "&")
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

/// Band descriptions in GDAL's metadata XML (items DESCRIPTION, role "description").
fn band_names(xml: &str, spp: usize) -> Vec<Option<String>> {
    let mut names = vec![None; spp];
    for item in xml.split("<Item").skip(1) {
        let Some(close) = item.find('>') else { continue };
        let attrs = &item[..close];
        let Some(end) = item[close + 1..].find("</Item>") else { continue };
        let value = &item[close + 1..close + 1 + end];
        let attr = |name: &str| -> Option<String> {
            let key = format!("{name}=\"");
            let i = attrs.find(&key)? + key.len();
            let j = attrs[i..].find('"')?;
            Some(attrs[i..i + j].to_string())
        };
        if attr("role").as_deref() == Some("description")
            && let Some(Ok(s)) = attr("sample").map(|v| v.parse::<usize>())
            && s < spp
        {
            names[s] = Some(xml_unescape(value));
        }
    }
    names
}

/// Read the first image of a TIFF file.
pub fn read(path: impl AsRef<Path>) -> Result<Raster> {
    let mut f = File::open(path.as_ref())?;
    let head = read_at(&mut f, 0, 16).or_else(|_| read_at(&mut f, 0, 8))?;
    let e = match &head[..2] {
        b"II" => Endian { little: true },
        b"MM" => Endian { little: false },
        _ => return Err(Error::NotTiff),
    };
    let (big, first) = match e.u16(&head[2..]) {
        42 => (false, e.u32(&head[4..]) as u64),
        43 => {
            if head.len() < 16 {
                return Err(Error::NotTiff);
            }
            (true, e.u64(&head[8..]))
        }
        _ => return Err(Error::NotTiff),
    };
    let ifd = read_ifd(&mut f, e, big, first)?;
    let l = layout(&ifd, e)?;
    let geo = GeoTags {
        pixel_scale: ifd.floats(MODEL_PIXEL_SCALE, e)?,
        tiepoints: ifd.floats(MODEL_TIEPOINT, e)?,
        transformation: ifd.floats(MODEL_TRANSFORMATION, e)?,
        geokeys: ifd.uints(GEO_KEY_DIRECTORY, e)?.map(|v| v.into_iter().map(|x| x as u16).collect()),
        double_params: ifd.floats(GEO_DOUBLE_PARAMS, e)?,
        ascii_params: ifd.get(GEO_ASCII_PARAMS).map(|x| String::from_utf8_lossy(&x.data).into_owned()),
    };
    let transform = geotransform(&geo);
    let nodata = ifd.get(GDAL_NODATA).and_then(|x| parse_nodata(&x.text()));
    let names = ifd.get(GDAL_METADATA).map(|x| band_names(&x.text(), l.spp)).unwrap_or_else(|| vec![None; l.spp]);
    let data = read_bands(&mut f, &l)?;
    let bands = data.into_iter().zip(names).map(|(data, name)| Band { name, data }).collect();
    Ok(Raster { grid: Grid { width: l.width, height: l.height, transform, geo }, bands, dtype: l.dtype, nodata })
}

// ----------------------------------------------------------------------------- writing

/// How a raster is written.
#[derive(Debug, Clone)]
pub struct WriteOptions {
    /// Deflate level (0-9), or `None` for no compression.
    pub deflate: Option<u32>,
    /// The type of the cells (default: the raster's).
    pub dtype: Option<DataType>,
    /// BigTIFF: `Some(true)`, `Some(false)` or `None` (when the file could pass 4 GB).
    pub bigtiff: Option<bool>,
    /// Bytes of uncompressed cells per strip (about).
    pub strip_bytes: usize,
}

impl Default for WriteOptions {
    fn default() -> Self {
        WriteOptions { deflate: Some(6), dtype: None, bigtiff: None, strip_bytes: 1 << 16 }
    }
}

enum Value {
    Short(Vec<u16>),
    Long(Vec<u32>),
    Long8(Vec<u64>),
    Double(Vec<f64>),
    Ascii(String),
}

impl Value {
    fn typ(&self) -> u16 {
        match self {
            Value::Short(_) => T_SHORT,
            Value::Long(_) => T_LONG,
            Value::Long8(_) => T_LONG8,
            Value::Double(_) => T_DOUBLE,
            Value::Ascii(_) => T_ASCII,
        }
    }
    fn count(&self) -> u64 {
        match self {
            Value::Short(v) => v.len() as u64,
            Value::Long(v) => v.len() as u64,
            Value::Long8(v) => v.len() as u64,
            Value::Double(v) => v.len() as u64,
            Value::Ascii(s) => s.len() as u64 + 1,
        }
    }
    fn bytes(&self) -> Vec<u8> {
        match self {
            Value::Short(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            Value::Long(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            Value::Long8(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            Value::Double(v) => v.iter().flat_map(|x| x.to_le_bytes()).collect(),
            Value::Ascii(s) => {
                let mut b = s.as_bytes().to_vec();
                b.push(0);
                b
            }
        }
    }
}

fn format_nodata(v: f64) -> String {
    if v.is_nan() {
        "nan".into()
    } else if v.is_finite() && v.fract() == 0.0 && v.abs() < 1e15 {
        format!("{}", v as i64)
    } else {
        format!("{v:?}")
    }
}

/// The cells of one band as bytes of the type (little-endian).
fn encode_value(v: f64, dtype: DataType, out: &mut Vec<u8>) {
    match dtype {
        DataType::U8 => out.push(v as u8),
        DataType::I8 => out.push(v as i8 as u8),
        DataType::U16 => out.extend((v as u16).to_le_bytes()),
        DataType::I16 => out.extend((v as i16).to_le_bytes()),
        DataType::U32 => out.extend((v as u32).to_le_bytes()),
        DataType::I32 => out.extend((v as i32).to_le_bytes()),
        DataType::U64 => out.extend((v as u64).to_le_bytes()),
        DataType::I64 => out.extend((v as i64).to_le_bytes()),
        DataType::F32 => out.extend((v as f32).to_le_bytes()),
        DataType::F64 => out.extend(v.to_le_bytes()),
    }
}

/// Write a raster as a GeoTIFF: pixel-interleaved strips, little-endian.
///
/// With an integer type, NaN cells are written as the no-data value (an
/// error without one).
pub fn write(r: &Raster, path: impl AsRef<Path>, opt: &WriteOptions) -> Result<()> {
    let dtype = opt.dtype.unwrap_or(r.dtype);
    let (w, h, spp) = (r.grid.width, r.grid.height, r.bands.len());
    if spp == 0 {
        return Err(Error::Raster("a raster needs a band".into()));
    }
    if r.bands.iter().any(|b| b.data.len() != w * h) {
        return Err(Error::Raster("a band differs in size from the grid".into()));
    }
    let fill = if dtype.is_float() { None } else { r.nodata.filter(|v| !v.is_nan()) };
    if !dtype.is_float() && fill.is_none() && r.bands.iter().any(|b| b.data.iter().any(|v| v.is_nan())) {
        return Err(Error::Raster("NaN cells in an integer raster without a no-data value".into()));
    }
    let bps = dtype.bytes();
    let row_bytes = w * spp * bps;
    let rps = (opt.strip_bytes / row_bytes.max(1)).clamp(1, h.max(1));
    let nstrips = h.div_ceil(rps);
    let raw_total = (row_bytes * h) as u64;
    let big = opt.bigtiff.unwrap_or(raw_total > 3_900_000_000);

    let file = File::create(path.as_ref())?;
    let mut out = BufWriter::new(file);
    let header_len: u64 = if big { 16 } else { 8 };
    out.write_all(&vec![0u8; header_len as usize])?; // filled in at the end
    let mut pos = header_len;
    let mut offsets = Vec::with_capacity(nstrips);
    let mut counts = Vec::with_capacity(nstrips);
    let mut buf = Vec::with_capacity(rps * row_bytes);
    for s in 0..nstrips {
        buf.clear();
        for y in s * rps..((s + 1) * rps).min(h) {
            for x in 0..w {
                for band in &r.bands {
                    let v = band.data[y * w + x];
                    encode_value(if v.is_nan() { fill.unwrap_or(v) } else { v }, dtype, &mut buf);
                }
            }
        }
        let data = match opt.deflate {
            Some(level) => {
                let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::new(level));
                z.write_all(&buf)?;
                z.finish()?
            }
            None => buf.clone(),
        };
        out.write_all(&data)?;
        offsets.push(pos);
        counts.push(data.len() as u64);
        pos += data.len() as u64;
    }
    if !big && pos > u32::MAX as u64 {
        return Err(Error::Raster("the file passes 4 GB: write it as BigTIFF".into()));
    }

    // the IFD
    let mut tags: Vec<(u16, Value)> = vec![
        (IMAGE_WIDTH, Value::Long(vec![w as u32])),
        (IMAGE_LENGTH, Value::Long(vec![h as u32])),
        (BITS_PER_SAMPLE, Value::Short(vec![(bps * 8) as u16; spp])),
        (COMPRESSION, Value::Short(vec![if opt.deflate.is_some() { 8 } else { 1 }])),
        (PHOTOMETRIC, Value::Short(vec![1])),
        (SAMPLES_PER_PIXEL, Value::Short(vec![spp as u16])),
        (ROWS_PER_STRIP, Value::Long(vec![rps as u32])),
        (PLANAR_CONFIG, Value::Short(vec![1])),
        (SAMPLE_FORMAT, Value::Short(vec![dtype.sample_format(); spp])),
    ];
    if big {
        tags.push((STRIP_OFFSETS, Value::Long8(offsets)));
        tags.push((STRIP_BYTE_COUNTS, Value::Long8(counts)));
    } else {
        tags.push((STRIP_OFFSETS, Value::Long(offsets.iter().map(|&v| v as u32).collect())));
        tags.push((STRIP_BYTE_COUNTS, Value::Long(counts.iter().map(|&v| v as u32).collect())));
    }
    if spp > 1 {
        tags.push((EXTRA_SAMPLES, Value::Short(vec![0; spp - 1])));
    }
    let g = &r.grid.geo;
    let has_geo = g.geokeys.is_some() || g.pixel_scale.is_some() || g.transformation.is_some();
    let t = r.grid.transform;
    if t[2] == 0.0 && t[4] == 0.0 {
        if has_geo || t != [0.0, 1.0, 0.0, 0.0, 0.0, -1.0] {
            tags.push((MODEL_PIXEL_SCALE, Value::Double(vec![t[1], -t[5], 0.0])));
            tags.push((MODEL_TIEPOINT, Value::Double(vec![0.0, 0.0, 0.0, t[0], t[3], 0.0])));
        }
    } else {
        tags.push((
            MODEL_TRANSFORMATION,
            Value::Double(vec![t[1], t[2], 0.0, t[0], t[4], t[5], 0.0, t[3], 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
        ));
    }
    if let Some(k) = &g.geokeys {
        // the raster type key as PixelIsArea: the transform written is the corner's
        let mut keys = k.clone();
        let n = keys.get(3).copied().unwrap_or(0) as usize;
        for i in 0..n {
            if keys.get(4 + 4 * i) == Some(&1025) && keys.len() > 7 + 4 * i {
                keys[7 + 4 * i] = 1;
            }
        }
        tags.push((GEO_KEY_DIRECTORY, Value::Short(keys)));
    }
    if let Some(d) = &g.double_params {
        tags.push((GEO_DOUBLE_PARAMS, Value::Double(d.clone())));
    }
    if let Some(a) = &g.ascii_params {
        tags.push((GEO_ASCII_PARAMS, Value::Ascii(a.trim_end_matches('\0').to_string())));
    }
    if r.bands.iter().any(|b| b.name.is_some()) {
        let mut xml = String::from("<GDALMetadata>\n");
        for (i, b) in r.bands.iter().enumerate() {
            if let Some(n) = &b.name {
                xml.push_str(&format!(
                    "  <Item name=\"DESCRIPTION\" sample=\"{i}\" role=\"description\">{}</Item>\n",
                    xml_escape(n)
                ));
            }
        }
        xml.push_str("</GDALMetadata>");
        tags.push((GDAL_METADATA, Value::Ascii(xml)));
    }
    if let Some(nd) = r.nodata {
        tags.push((GDAL_NODATA, Value::Ascii(format_nodata(nd))));
    }
    tags.sort_by_key(|(tag, _)| *tag);

    // the IFD after the strips (word-aligned), the values that do not fit in their entry after it
    if pos % 2 == 1 {
        out.write_all(&[0])?;
        pos += 1;
    }
    let ifd_offset = pos;
    let (count_len, entry_len, inline, next_len) = if big { (8u64, 20u64, 8usize, 8u64) } else { (2, 12, 4, 4) };
    let mut extra_pos = ifd_offset + count_len + entry_len * tags.len() as u64 + next_len;
    let mut ifd = Vec::new();
    let mut extra = Vec::new();
    if big {
        ifd.extend((tags.len() as u64).to_le_bytes());
    } else {
        ifd.extend((tags.len() as u16).to_le_bytes());
    }
    for (tag, value) in &tags {
        let bytes = value.bytes();
        ifd.extend(tag.to_le_bytes());
        ifd.extend(value.typ().to_le_bytes());
        if big {
            ifd.extend(value.count().to_le_bytes());
        } else {
            ifd.extend((value.count() as u32).to_le_bytes());
        }
        if bytes.len() <= inline {
            let mut field = bytes.clone();
            field.resize(inline, 0);
            ifd.extend(field);
        } else {
            if big {
                ifd.extend(extra_pos.to_le_bytes());
            } else {
                ifd.extend((extra_pos as u32).to_le_bytes());
            }
            extra.extend(&bytes);
            extra_pos += bytes.len() as u64;
            if bytes.len() % 2 == 1 {
                extra.push(0);
                extra_pos += 1;
            }
        }
    }
    ifd.extend(vec![0u8; next_len as usize]); // no next IFD
    if !big && extra_pos > u32::MAX as u64 {
        return Err(Error::Raster("the file passes 4 GB: write it as BigTIFF".into()));
    }
    out.write_all(&ifd)?;
    out.write_all(&extra)?;
    // the header
    let mut header = Vec::new();
    header.extend(b"II");
    if big {
        header.extend(43u16.to_le_bytes());
        header.extend(8u16.to_le_bytes());
        header.extend(0u16.to_le_bytes());
        header.extend(ifd_offset.to_le_bytes());
    } else {
        header.extend(42u16.to_le_bytes());
        header.extend((ifd_offset as u32).to_le_bytes());
    }
    out.seek(SeekFrom::Start(0))?;
    out.write_all(&header)?;
    out.flush()?;
    Ok(())
}
