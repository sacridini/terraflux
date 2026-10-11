//! NumPy's `.npy` files of float64 or integer arrays (C order): the reference
//! outputs of landschaft that Terraflux is compared with.

use std::io::{Read, Write};
use std::path::Path;

use crate::{Error, Result};

/// An n-dimensional array read from a `.npy` file, as f64.
#[derive(Debug, Clone, PartialEq)]
pub struct Array {
    pub shape: Vec<usize>,
    pub data: Vec<f64>,
}

fn header_value<'a>(header: &'a str, key: &str) -> Option<&'a str> {
    let i = header.find(&format!("'{key}'"))? + key.len() + 2;
    let rest = header[i..].trim_start().strip_prefix(':')?.trim_start();
    Some(rest)
}

/// Read a `.npy` file of a little-endian numeric type in C order.
pub fn read(path: impl AsRef<Path>) -> Result<Array> {
    let mut f = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes)?;
    parse(&bytes)
}

/// The arrays of a `.npz` file (NumPy's zip of `.npy` files, stored or deflated), by name without `.npy`.
pub fn read_npz(path: impl AsRef<Path>) -> Result<std::collections::BTreeMap<String, Array>> {
    let bytes = std::fs::read(path)?;
    let bad = |m: &str| Error::Raster(format!("bad .npz: {m}"));
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
    let u32_at = |i: usize| u32::from_le_bytes(bytes[i..i + 4].try_into().unwrap()) as u64;
    let u64_at = |i: usize| u64::from_le_bytes(bytes[i..i + 8].try_into().unwrap());
    // the end of the central directory, then its records
    let end =
        (0..bytes.len().saturating_sub(21)).rev().find(|&i| u32_at(i) == 0x0605_4b50).ok_or_else(|| bad("no end"))?;
    let (count, mut at) = (u16_at(end + 10), u32_at(end + 16) as usize);
    let mut out = std::collections::BTreeMap::new();
    for _ in 0..count {
        if u32_at(at) != 0x0201_4b50 {
            return Err(bad("no directory record"));
        }
        let method = u16_at(at + 10);
        let (mut csize, mut usize_, mut offset) = (u32_at(at + 20), u32_at(at + 24), u32_at(at + 42));
        let (nlen, xlen, clen) = (u16_at(at + 28), u16_at(at + 30), u16_at(at + 32));
        let name = String::from_utf8_lossy(&bytes[at + 46..at + 46 + nlen]).into_owned();
        // ZIP64: the sizes and offset that do not fit 32 bits, in this order
        let mut x = at + 46 + nlen;
        while x + 4 <= at + 46 + nlen + xlen {
            let (id, len) = (u16_at(x), u16_at(x + 2));
            if id == 1 {
                let mut k = x + 4;
                for field in [&mut usize_, &mut csize, &mut offset] {
                    if *field == 0xFFFF_FFFF {
                        *field = u64_at(k);
                        k += 8;
                    }
                }
            }
            x += 4 + len;
        }
        let local = offset as usize;
        let data = local + 30 + u16_at(local + 26) + u16_at(local + 28);
        let raw = &bytes[data..data + csize as usize];
        let body = match method {
            0 => raw.to_vec(),
            8 => {
                let mut v = Vec::with_capacity(usize_ as usize);
                flate2::read::DeflateDecoder::new(raw).read_to_end(&mut v)?;
                v
            }
            m => return Err(bad(&format!("compression {m}"))),
        };
        out.insert(name.trim_end_matches(".npy").to_string(), parse(&body)?);
        at += 46 + nlen + xlen + clen;
    }
    Ok(out)
}

/// A `.npy` file's bytes.
pub fn parse(bytes: &[u8]) -> Result<Array> {
    if bytes.len() < 10 || &bytes[..6] != b"\x93NUMPY" {
        return Err(Error::Raster("not a .npy file".into()));
    }
    let (hlen, start) = if bytes[6] == 1 {
        (u16::from_le_bytes([bytes[8], bytes[9]]) as usize, 10)
    } else {
        (u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize, 12)
    };
    let header =
        std::str::from_utf8(&bytes[start..start + hlen]).map_err(|_| Error::Raster("bad .npy header".into()))?;
    let descr = header_value(header, "descr").ok_or_else(|| Error::Raster("no descr".into()))?;
    let descr = &descr[1..descr[1..].find('\'').unwrap() + 1];
    let fortran = header_value(header, "fortran_order").is_some_and(|v| v.starts_with("True"));
    let shape_txt = header_value(header, "shape").ok_or_else(|| Error::Raster("no shape".into()))?;
    let shape_txt = &shape_txt[1..shape_txt.find(')').unwrap()];
    let shape: Vec<usize> =
        shape_txt.split(',').filter(|s| !s.trim().is_empty()).map(|s| s.trim().parse().unwrap()).collect();
    let body = &bytes[start + hlen..];
    let data: Vec<f64> = match descr {
        "<f8" => body.chunks_exact(8).map(|b| f64::from_le_bytes(b.try_into().unwrap())).collect(),
        "<f4" => body.chunks_exact(4).map(|b| f32::from_le_bytes(b.try_into().unwrap()) as f64).collect(),
        "<i8" => body.chunks_exact(8).map(|b| i64::from_le_bytes(b.try_into().unwrap()) as f64).collect(),
        "<i4" => body.chunks_exact(4).map(|b| i32::from_le_bytes(b.try_into().unwrap()) as f64).collect(),
        "<i2" => body.chunks_exact(2).map(|b| i16::from_le_bytes(b.try_into().unwrap()) as f64).collect(),
        "<u2" => body.chunks_exact(2).map(|b| u16::from_le_bytes(b.try_into().unwrap()) as f64).collect(),
        "<u4" => body.chunks_exact(4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as f64).collect(),
        "|u1" => body.iter().map(|&b| b as f64).collect(),
        "|b1" => body.iter().map(|&b| b as f64).collect(),
        d => return Err(Error::Raster(format!("unsupported .npy type {d}"))),
    };
    let n: usize = shape.iter().product();
    if data.len() < n {
        return Err(Error::Raster("truncated .npy".into()));
    }
    let data = if fortran && shape.len() > 1 {
        // column-major: the first index varies fastest; reordered to row-major
        let mut out = vec![0.0; n];
        let mut idx = vec![0usize; shape.len()];
        for &v in &data[..n] {
            let mut flat = 0;
            for (d, &i) in idx.iter().enumerate() {
                flat = flat * shape[d] + i;
            }
            out[flat] = v;
            for (d, i) in idx.iter_mut().enumerate() {
                *i += 1;
                if *i < shape[d] {
                    break;
                }
                *i = 0;
            }
        }
        out
    } else {
        data[..n].to_vec()
    };
    Ok(Array { shape, data })
}

/// Write a float64 array as a `.npy` file.
pub fn write(path: impl AsRef<Path>, shape: &[usize], data: &[f64]) -> Result<()> {
    let dims: Vec<String> = shape.iter().map(|d| d.to_string()).collect();
    let shape_txt = if dims.len() == 1 { format!("({},)", dims[0]) } else { format!("({})", dims.join(", ")) };
    let mut header = format!("{{'descr': '<f8', 'fortran_order': False, 'shape': {shape_txt}, }}");
    let pad = 64 - (10 + header.len() + 1) % 64;
    header.push_str(&" ".repeat(pad % 64));
    header.push('\n');
    let mut f = std::io::BufWriter::new(std::fs::File::create(path)?);
    f.write_all(b"\x93NUMPY\x01\x00")?;
    f.write_all(&(header.len() as u16).to_le_bytes())?;
    f.write_all(header.as_bytes())?;
    for v in data {
        f.write_all(&v.to_le_bytes())?;
    }
    f.flush()?;
    Ok(())
}
