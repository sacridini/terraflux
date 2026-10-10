//! Reading GeoTIFF files written by GDAL in every layout, and writing them back.

use std::path::PathBuf;

use terraflux_raster::tiff::{WriteOptions, write};
use terraflux_raster::{DataType, Grid, Raster, npy};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests").join("data").join(name)
}

fn tmp(name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// The cells of every band, band after band (as NumPy's (band, y, x)).
fn cells(r: &Raster) -> Vec<f64> {
    r.bands.iter().flat_map(|b| b.data.iter().copied()).collect()
}

fn same_bits(a: &[f64], b: &[f64]) {
    assert_eq!(a.len(), b.len());
    for (i, (x, y)) in a.iter().zip(b).enumerate() {
        assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "cell {i}: {x} != {y}");
    }
}

#[test]
fn reads_every_layout_of_the_land_use_map() {
    let expected = npy::read(data("lu.npy")).unwrap();
    assert_eq!(expected.shape, vec![2, 108, 128]);
    for (file, dtype) in [
        ("lu_plain.tif", DataType::U8),
        ("lu_u16_lzw_tiled_pred2.tif", DataType::U16),
        ("lu_packbits_band.tif", DataType::U8),
        ("lu_i16_be_pred2.tif", DataType::I16),
    ] {
        let r = Raster::read(data(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!((r.grid.width, r.grid.height, r.bands.len()), (128, 108, 2), "{file}");
        assert_eq!(r.dtype, dtype, "{file}");
        assert_eq!(r.nodata, Some(255.0), "{file}");
        assert_eq!(r.band_names(), vec!["1997", "2011"], "{file}");
        assert_eq!(r.grid.transform, [436000.0, 250.0, 0.0, 1383000.0, 0.0, -250.0], "{file}");
        same_bits(&cells(&r), &expected.data);
    }
}

#[test]
fn reads_every_layout_of_the_drivers() {
    let expected = npy::read(data("dr.npy")).unwrap();
    for file in ["dr.tif", "dr_f32_pred3_tiled.tif", "dr_bigtiff_be.tif", "dr_f64_lzw_pred3.tif", "dr_f64_be_band.tif"]
    {
        let r = Raster::read(data(file)).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(r.band_names(), vec!["dist_sea", "pop_density", "sediments"], "{file}");
        assert!(r.nodata.is_some_and(f64::is_nan), "{file}");
        same_bits(&cells(&r), &expected.data);
    }
}

#[test]
fn keeps_the_coordinate_reference_system() {
    let r = Raster::read(data("pie_small.tif")).unwrap();
    assert!(r.grid.geo.geokeys.is_some());
    assert_eq!(r.band_names(), vec!["1985", "1991", "1999"]);
    let out = tmp("crs.tif");
    r.write(&out).unwrap();
    let back = Raster::read(&out).unwrap();
    assert_eq!(back.grid, r.grid);
    assert_eq!(back.bands, r.bands);
    assert_eq!(back.nodata, r.nodata);
    assert_eq!(back.dtype, r.dtype);
}

#[test]
fn writes_every_type_and_reads_it_back() {
    let grid = Grid::new(37, 23);
    let base: Vec<f64> = (0..grid.cells()).map(|i| (i % 200) as f64).collect();
    for dtype in [
        DataType::U8,
        DataType::I8,
        DataType::U16,
        DataType::I16,
        DataType::U32,
        DataType::I32,
        DataType::F32,
        DataType::F64,
    ] {
        for deflate in [None, Some(6)] {
            for bigtiff in [false, true] {
                let mut values = base.clone();
                if dtype == DataType::I8 {
                    values.iter_mut().for_each(|v| *v = (*v % 100.0) - 50.0);
                }
                if dtype.is_float() {
                    values[5] = f64::NAN;
                    values[6] = 0.1;
                }
                let r = Raster {
                    grid: grid.clone(),
                    bands: vec![
                        terraflux_raster::Band { name: Some("first".into()), data: values.clone() },
                        terraflux_raster::Band {
                            name: Some("a & <b>".into()),
                            data: values.iter().rev().copied().collect(),
                        },
                    ],
                    dtype,
                    nodata: if dtype.is_float() { Some(f64::NAN) } else { Some(7.0) },
                };
                let out = tmp(&format!("w_{dtype:?}_{deflate:?}_{bigtiff}.tif"));
                write(
                    &r,
                    &out,
                    &WriteOptions { deflate, bigtiff: Some(bigtiff), strip_bytes: 300, ..Default::default() },
                )
                .unwrap();
                let back = Raster::read(&out).unwrap();
                assert_eq!(back.dtype, dtype);
                assert_eq!(back.band_names(), vec!["first", "a & <b>"]);
                let expected: Vec<f64> = if dtype == DataType::F32 {
                    values.iter().map(|&v| v as f32 as f64).collect()
                } else {
                    values.clone()
                };
                same_bits(&back.bands[0].data, &expected);
            }
        }
    }
}

#[test]
fn nan_in_integers_becomes_nodata() {
    let r = Raster::single(Grid::new(3, 1), vec![1.0, f64::NAN, 3.0], DataType::U8, Some(255.0));
    let out = tmp("nan_u8.tif");
    r.write(&out).unwrap();
    let back = Raster::read(&out).unwrap();
    assert_eq!(back.bands[0].data, vec![1.0, 255.0, 3.0]);
    let masked = back.bands[0].masked(back.nodata);
    assert!(masked[1].is_nan());
    let bad = Raster::single(Grid::new(2, 1), vec![1.0, f64::NAN], DataType::U8, None);
    assert!(bad.write(tmp("bad.tif")).is_err());
}

#[test]
fn npy_round_trip() {
    let out = tmp("a.npy");
    npy::write(&out, &[2, 3], &[1.0, f64::NAN, 3.5, -4.0, 5.0, 6.0]).unwrap();
    let a = npy::read(&out).unwrap();
    assert_eq!(a.shape, vec![2, 3]);
    assert!(a.data[1].is_nan() && a.data[2] == 3.5);
}
