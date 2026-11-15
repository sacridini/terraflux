//! Public data in the clouds, really read (ignored: they need the internet, `cargo test -p terraflux-app --test app
//! cloud_public:: -- --ignored --nocapture`): a COG of Amazon S3 and a GeoTIFF of Google Cloud Storage, by their
//! `s3://` and `gs://` addresses, with the settings' "public buckets, requests not signed" (no key of anyone's), read
//! by `raster.read` as a model runs it: the header, and a window of the cells.

use std::sync::Arc;
use std::time::Instant;

use terraflux_app::settings::{self, Cloud};
use terraflux_ops::{Args, Ctx, Registry, Value};
use terraflux_raster::tiff::Window;

/// The tests change the one cloud setting of the program: one at a time.
static CLOUD: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// `raster.read` of `path`, as a node of a model runs it.
fn read(path: &str) -> Arc<terraflux_ops::files::Rasters> {
    let r = Registry::standard();
    let params = serde_json::json!({ "path": path }).as_object().cloned().unwrap();
    let args = Args { node: "n", inputs: Default::default(), params: &params };
    let out = r.get("raster.read").unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{path}: {e}"));
    let Value::Raster(r) = &out["raster"] else { panic!("not a raster") };
    r.clone()
}

/// The header and a window of the middle of the raster, printed; its values.
fn look(path: &str, side: usize) -> (usize, usize, String, Vec<f64>) {
    let t = Instant::now();
    let r = read(path);
    let g = r.grid_of(0).clone();
    println!(
        "{path}: {} x {} cells, crs {}, header in {:?}",
        g.width,
        g.height,
        g.geo.wkt.as_deref().unwrap_or("none").chars().take(60).collect::<String>(),
        t.elapsed()
    );
    let win = Window {
        x0: (g.width.saturating_sub(side) / 2) as i64,
        y0: (g.height.saturating_sub(side) / 2) as i64,
        width: side,
        height: side,
    };
    let t = Instant::now();
    let v = r.window(0, win).unwrap_or_else(|e| panic!("{path}: {e}"));
    let bytes = v.len() * 8;
    println!("  window of {side} x {side}: {} values ({bytes} bytes as numbers) in {:?}", v.len(), t.elapsed());
    (g.width, g.height, g.geo.wkt.clone().unwrap_or_default(), v)
}

fn unsigned(aws: bool, region: &str) {
    crate::point_settings();
    settings::update(|s| {
        s.cloud = Cloud {
            aws_no_sign_request: aws,
            gs_no_sign_request: !aws,
            aws_region: region.to_string(),
            ..Default::default()
        }
    });
}

#[test]
#[ignore = "reads Amazon S3 (the Copernicus DEM, public)"]
fn a_public_s3_cog_is_read_without_keys() {
    if terraflux_gdal::gdal().is_err() {
        println!("no GDAL: nothing to read the cloud with");
        return;
    }
    let _one = CLOUD.lock().unwrap_or_else(|e| e.into_inner());
    unsigned(true, "eu-central-1");
    let key =
        "copernicus-dem-30m/Copernicus_DSM_COG_10_N00_00_E006_00_DEM/Copernicus_DSM_COG_10_N00_00_E006_00_DEM.tif";
    let (w, h, wkt, v) = look(&format!("s3://{key}"), 64);
    assert_eq!((w, h), (3600, 3600));
    assert!(wkt.contains("4326") && wkt.contains("WGS 84"), "{wkt}");
    assert_eq!(v.len(), 64 * 64);
    assert!(v.iter().filter(|x| x.is_finite()).count() > 64 * 32, "cells with values");
    assert!(v.iter().filter(|x| x.is_finite()).all(|x| (-500.0..4000.0).contains(x)), "elevations");
    // the same object by the web, /vsicurl/: the same cells
    let (w2, h2, _, again) =
        look(&format!("https://copernicus-dem-30m.s3.eu-central-1.amazonaws.com/{}", &key[19..]), 64);
    assert_eq!((w2, h2), (w, h));
    assert_eq!(again, v, "S3 and https give the same cells");
    settings::update(|s| s.cloud = Default::default());
}

#[test]
#[ignore = "reads Google Cloud Storage (Landsat, public)"]
fn a_public_gs_geotiff_is_read_without_keys() {
    if terraflux_gdal::gdal().is_err() {
        println!("no GDAL: nothing to read the cloud with");
        return;
    }
    let _one = CLOUD.lock().unwrap_or_else(|e| e.into_inner());
    unsigned(false, "");
    let key = "gcp-public-data-landsat/LC08/01/044/034/LC08_L1GT_044034_20130330_20170310_01_T2/LC08_L1GT_044034_20130330_20170310_01_T2_B1.TIF";
    let (w, h, wkt, v) = look(&format!("gs://{key}"), 64);
    assert!(w > 7000 && h > 7000, "{w} x {h}");
    assert!(wkt.contains("UTM zone 10N"), "{wkt}");
    assert_eq!(v.len(), 64 * 64);
    assert!(v.iter().any(|x| x.is_finite() && *x > 0.0), "some cells have values");
    let (w2, h2, _, again) = look(&format!("https://storage.googleapis.com/{key}"), 64);
    assert_eq!((w2, h2), (w, h));
    assert_eq!(again, v, "gs:// and https give the same cells");
    settings::update(|s| s.cloud = Default::default());
}
