//! GDAL loaded when the program runs: the same cells as Terraflux's own reader, other formats, warping and
//! translating. Without GDAL on the machine the tests say so and pass.

use std::path::PathBuf;

use terraflux_gdal::{Dataset, gdal, translate, warp};
use terraflux_raster::Raster;

fn data(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/inputs").join(name)
}

fn tmp(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("gdal");
    std::fs::create_dir_all(&d).unwrap();
    d.join(name)
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y || (x.is_nan() && y.is_nan()))
}

#[test]
fn reads_formats_warps_and_translates() {
    let g = match gdal() {
        Ok(g) => g,
        Err(e) => {
            eprintln!("{e}: not tested");
            return;
        }
    };
    eprintln!("GDAL {} from {} ({} drivers)", g.version, g.library.display(), g.drivers);
    assert!(g.drivers > 50);
    // the same header and cells as the own reader
    let own = Raster::read(data("sibuyan_landuse.tif")).unwrap();
    let ds = Dataset::open(data("sibuyan_landuse.tif")).unwrap();
    assert_eq!(ds.driver, "GTiff");
    let (w, h) = (own.grid.width, own.grid.height);
    assert_eq!((ds.meta.grid.width, ds.meta.grid.height), (w, h));
    assert_eq!(ds.meta.grid.transform, own.grid.transform);
    assert_eq!(ds.meta.band_names(), own.band_names());
    assert_eq!(ds.meta.nodata, own.nodata);
    assert_eq!(ds.meta.grid.geo.wkt, None, "the file has no CRS");
    for b in 0..own.bands.len() {
        assert!(same(&ds.read(b, 0, 0, w, h, w, h).unwrap(), &own.bands[b].data), "band {b}");
    }
    // reduced: every second cell
    let half = ds.read(0, 0, 0, w, h, w / 2, h / 2).unwrap();
    assert_eq!(half.len(), (w / 2) * (h / 2));
    // another format: Erdas Imagine, the same cells
    let img = tmp("lu.img");
    translate(&data("sibuyan_landuse.tif"), &img, &["-of".into(), "HFA".into()]).unwrap();
    let other = Dataset::open(&img).unwrap();
    assert_eq!(other.driver, "HFA");
    assert!(same(&other.read(1, 0, 0, w, h, w, h).unwrap(), &own.bands[1].data));
    // warped to geographic coordinates, by mode (classes)
    let geo = tmp("lu_4326.tif");
    let _ = std::fs::remove_file(&geo);
    let args: Vec<String> =
        ["-s_srs", "EPSG:32651", "-t_srs", "EPSG:4326", "-r", "mode"].iter().map(|s| s.to_string()).collect();
    warp(&[&data("sibuyan_landuse.tif")], &geo, &args).unwrap();
    let warped = Raster::read(&geo).unwrap();
    assert!(warped.grid.transform[1] < 0.01, "degrees: {:?}", warped.grid.transform);
    assert_eq!(warped.grid.geo.epsg(), Some(4326));
    let wkt = Dataset::open(&geo).unwrap().meta.grid.geo.wkt.clone().unwrap();
    assert!(wkt.contains("WGS 84"), "{wkt}");
    // a file it cannot open says why
    assert!(Dataset::open(tmp("none.tif")).err().unwrap().contains("none.tif"));
}

/// A web map (Esri's imagery) warped onto Sibuyan's extent in UTM 51N: pixels of the island. Without the web,
/// the test says so and passes.
#[test]
fn web_maps_are_warped_onto_a_grid() {
    if gdal().is_err() {
        return;
    }
    let cache = tmp("tiles");
    let esri = "https://server.arcgisonline.com/ArcGIS/rest/services/World_Imagery/MapServer/tile/${z}/${y}/${x}";
    let service = terraflux_gdal::tile_service(esri, 19, &cache);
    let own = Raster::read(data("sibuyan_landuse.tif")).unwrap();
    let t = own.grid.transform;
    let (w, h) = (own.grid.width as f64, own.grid.height as f64);
    let extent = [t[0], t[3] + h * t[5], t[0] + w * t[1], t[3]];
    match terraflux_gdal::warp_image(&service, "EPSG:32651", extent, 256, 216) {
        Err(e) => eprintln!("no web map ({e}): not tested"),
        Ok(px) => {
            assert_eq!(px.len(), 256 * 216);
            let opaque = px.iter().filter(|p| p[3] == 255).count();
            assert!(opaque > px.len() / 2, "{opaque} opaque pixels");
            let (lo, hi) = px.iter().fold((255u8, 0u8), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
            assert!(hi - lo > 40, "an image, not a flat colour: {lo}..{hi}");
            assert!(std::fs::read_dir(&cache).is_ok_and(|mut d| d.next().is_some()), "the tiles kept");
        }
    }
}
