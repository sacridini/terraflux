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
    if header_value(header, "fortran_order").is_some_and(|v| v.starts_with("True")) {
        return Err(Error::Raster("Fortran-ordered .npy".into()));
    }
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
        "|u1" => body.iter().map(|&b| b as f64).collect(),
        "|b1" => body.iter().map(|&b| b as f64).collect(),
        d => return Err(Error::Raster(format!("unsupported .npy type {d}"))),
    };
    let n: usize = shape.iter().product();
    if data.len() < n {
        return Err(Error::Raster("truncated .npy".into()));
    }
    Ok(Array { shape, data: data[..n].to_vec() })
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
