//! Rasters read where and as finely as needed: the same cells as a whole read, in every layout; windows at
//! reduced resolution; overviews used when the file has them.

use std::path::PathBuf;

use terraflux_raster::image::Image;
use terraflux_raster::tiff::{TiledWriter, Window};
use terraflux_raster::{DataType, Raster};

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data").join(name)
}

const LAYOUTS: [&str; 10] = [
    "dr.tif",
    "dr_bigtiff_be.tif",
    "dr_f32_pred3_tiled.tif",
    "dr_f64_be_band.tif",
    "dr_f64_lzw_pred3.tif",
    "lu_i16_be_pred2.tif",
    "lu_packbits_band.tif",
    "lu_plain.tif",
    "lu_u16_lzw_tiled_pred2.tif",
    "pie_small.tif",
];

#[test]
fn whole_bands_are_the_same_bits() {
    for name in LAYOUTS {
        let whole = Raster::read(data(name)).unwrap();
        let img = Image::open(data(name)).unwrap();
        assert_eq!(img.meta.grid, whole.grid, "{name}");
        for (b, band) in whole.bands.iter().enumerate() {
            // a window passing the corner, exactly
            let (w, h) = (whole.grid.width, whole.grid.height);
            let win = Window { x0: -3, y0: 2, width: w / 2, height: h - 1 };
            let part = img.read_window(b, win).unwrap();
            for (k, x) in part.iter().enumerate() {
                let (i, j) = ((k / win.width) as i64 + win.y0, (k % win.width) as i64 + win.x0);
                if i < 0 || j < 0 || i >= h as i64 || j >= w as i64 {
                    assert!(x.is_nan());
                } else {
                    let y = band.data[i as usize * w + j as usize];
                    assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{name}: window");
                }
            }
            let got = img.read_band(b).unwrap();
            assert_eq!(got.len(), band.data.len(), "{name}");
            for (x, y) in got.iter().zip(&band.data) {
                assert!(x.to_bits() == y.to_bits() || (x.is_nan() && y.is_nan()), "{name} band {b}: {x} != {y}");
            }
        }
    }
}

#[test]
fn windows_at_every_step() {
    for name in LAYOUTS {
        let whole = Raster::read(data(name)).unwrap();
        let img = Image::open(data(name)).unwrap();
        let (w, h) = (whole.grid.width as i64, whole.grid.height as i64);
        for band in 0..whole.bands.len() {
            for step in [1usize, 2, 3, 7, 50] {
                // a window passing the image's top-left corner
                let win = Window { x0: -5, y0: -3, width: (w / 2) as usize + 9, height: (h / 2) as usize + 4 };
                let got = img.sample(band, win, step).unwrap();
                let (ow, oh) = (win.width.div_ceil(step), win.height.div_ceil(step));
                assert_eq!(got.len(), ow * oh);
                for i in 0..oh {
                    for j in 0..ow {
                        let (x, y) = (win.x0 + (j * step) as i64, win.y0 + (i * step) as i64);
                        let v = got[i * ow + j];
                        if x < 0 || y < 0 || x >= w || y >= h {
                            assert!(v.is_nan(), "{name}: outside the image");
                        } else {
                            let want = whole.bands[band].data[(y * w + x) as usize] as f32;
                            assert!(v == want || (v.is_nan() && want.is_nan()), "{name} step {step} at {x},{y}");
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn overviews_serve_coarse_steps() {
    // a file with an overview of half the size: GDAL's layout, a second directory marked as a reduced image
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("with_overview.tif");
    let status = std::process::Command::new("gdal_translate")
        .args(["-q", "-of", "GTiff", "-co", "TILED=YES"])
        .arg(data("lu_plain.tif"))
        .arg(&path)
        .status();
    let added = status.is_ok_and(|s| s.success())
        && std::process::Command::new("gdaladdo")
            .args(["-q", "-r", "nearest"])
            .arg(&path)
            .arg("2")
            .status()
            .is_ok_and(|s| s.success());
    if !added {
        eprintln!("no GDAL on the PATH: the overview is not tested");
        return;
    }
    let img = Image::open(&path).unwrap();
    assert_eq!(img.overviews(), [2]);
    let whole = Raster::read(&path).unwrap();
    let (w, h) = (whole.grid.width, whole.grid.height);
    let win = Window { x0: 0, y0: 0, width: w, height: h };
    // a step of 2 reads the overview: GDAL's nearest, every second cell
    let got = img.sample(0, win, 2).unwrap();
    assert_eq!(got.len(), w.div_ceil(2) * h.div_ceil(2));
    let same = got
        .iter()
        .enumerate()
        .filter(|(k, v)| {
            let (i, j) = (k / w.div_ceil(2), k % w.div_ceil(2));
            let want = whole.bands[0].data[(2 * i).min(h - 1) * w + (2 * j).min(w - 1)] as f32;
            **v == want || (v.is_nan() && want.is_nan())
        })
        .count();
    assert!(same as f64 > 0.9 * got.len() as f64, "{same} of {}", got.len());
}

#[test]
fn tiled_files_written_here_read_by_windows() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("image_tiled.tif");
    let r = Raster::read(data("lu_plain.tif")).unwrap();
    let (w, h) = (r.grid.width, r.grid.height);
    let tile = 32;
    let mut tw = TiledWriter::create(&path, &r.grid, vec![None], DataType::U8, r.nodata, tile, Some(1)).unwrap();
    for row in 0..h.div_ceil(tile) {
        for col in 0..w.div_ceil(tile) {
            let mut v = Vec::new();
            for y in row * tile..(row * tile + tile).min(h) {
                for x in col * tile..(col * tile + tile).min(w) {
                    v.push(r.bands[0].data[y * w + x]);
                }
            }
            tw.write_tile(col, row, &[&v]).unwrap();
        }
    }
    tw.finish().unwrap();
    let img = Image::open(&path).unwrap();
    let got = img.read_band(0).unwrap();
    assert!(got.iter().zip(&r.bands[0].data).all(|(a, b)| a == b || (a.is_nan() && b.is_nan())));
}
