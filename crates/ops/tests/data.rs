//! Data downloaded on demand: the paths of every source (offline), a raster a path names warped onto a grid, and
//! (ignored, they need the internet) a small window of each source: `cargo test -p terraflux-ops --test ops data::
//! -- --ignored`.

use std::sync::Arc;

use terraflux_ops::data::{chelsa, copernicus_tiles, mapbiomas, soilgrids, worldclim, worldclim_future};
use terraflux_ops::{Args, Ctx, Map, Outputs, Registry, Value};
use terraflux_raster::{DataType, Grid, Raster};

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Outputs {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args).unwrap_or_else(|e| panic!("{op}: {e}"))
}

/// Every layer of a raster output, read.
fn layers(v: &Value) -> Vec<(String, Vec<f64>)> {
    let Value::Raster(r) = v else { panic!("{v:?}") };
    let s = r.stack().unwrap();
    s.keys.iter().map(|k| k.to_string()).zip(s.layers.iter().map(|l| l.to_vec())).collect()
}

#[test]
fn the_sources_paths() {
    let w = worldclim("bio", "10m", &[1, 12]);
    assert_eq!(
        w[1].path,
        "/vsizip//vsicurl/https://geodata.ucdavis.edu/climate/worldclim/2_1/base/wc2.1_10m_bio.zip/wc2.1_10m_bio_12.tif"
    );
    assert_eq!(w[1].name, "bio12");
    assert!(worldclim("tavg", "2.5m", &[3])[0].path.ends_with("wc2.1_2.5m_tavg.zip/wc2.1_2.5m_tavg_03.tif"));
    let f = worldclim_future("bioc", "10m", "ACCESS-CM2", "ssp245", "2041-2060", &[5]);
    assert_eq!(
        f[0].path,
        "vrt:///vsicurl/https://geodata.ucdavis.edu/cmip6/10m/ACCESS-CM2/ssp245/wc2.1_10m_bioc_ACCESS-CM2_ssp245_2041-2060.tif?bands=5"
    );
    assert!(chelsa(&[1], None)[0].path.ends_with("/1981-2010/bio/CHELSA_bio1_1981-2010_V.2.1.tif"));
    assert!(
        chelsa(&[1], Some(("GFDL-ESM4", "ssp126", "2041-2070")))[0]
            .path
            .ends_with("/2041-2070/GFDL-ESM4/ssp126/bio/CHELSA_bio1_2041-2070_gfdl-esm4_ssp126_V.2.1.tif")
    );
    // a box across the equator and the meridian of Greenwich: four tiles
    let t = copernicus_tiles(90, [-0.5, 0.5], [-0.5, 0.5]);
    assert_eq!(t.len(), 4);
    assert!(t[0].ends_with("Copernicus_DSM_COG_30_S01_00_W001_00_DEM/Copernicus_DSM_COG_30_S01_00_W001_00_DEM.tif"));
    assert!(t[3].contains("copernicus-dem-90m") && t[3].contains("_N00_00_E000_00_"));
    assert!(soilgrids("soc", &["0-5cm".into()], "mean")[0].path.ends_with("/data/soc/soc_0-5cm_mean.vrt"));
    let m = mapbiomas(11, &[2025]).unwrap();
    assert!(
        m[0].classes
            && m[0].path.ends_with("collection11/lulc/coverage/brazil_coverage/brazil_coverage-col11_2025.tif")
    );
    assert!(mapbiomas(8, &[2000]).is_err());
}

#[test]
fn a_raster_warped_onto_a_grid() {
    // a raster of 20 x 10 cells of 1 degree, its value the column, read onto a grid of 2 degrees
    let dir = std::env::temp_dir().join(format!("terraflux-data-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("columns.tif");
    let mut g = Grid::new(20, 10);
    g.transform = [-10.0, 1.0, 0.0, 5.0, 0.0, -1.0];
    g.geo.wkt = Some("EPSG:4326".into());
    let data: Vec<f64> = (0..200).map(|i| (i % 20) as f64).collect();
    Raster::single(g, data, DataType::F32, None).write(&path).unwrap();
    terraflux_gdal::set_crs(&path, "EPSG:4326").unwrap();
    let out = run(
        "data.download",
        vec![],
        serde_json::json!({"url": path.to_string_lossy(), "extent": [-10, -5, 10, 5], "cellsize": 2, "resampling": "average"}),
    );
    let l = layers(&out["raster"]);
    assert_eq!(l.len(), 1);
    // each cell of 2 degrees the mean of two columns
    assert_eq!(l[0].1[..10], [0.5, 2.5, 4.5, 6.5, 8.5, 10.5, 12.5, 14.5, 16.5, 18.5]);
    // and onto a map's grid
    let mut like = Grid::new(4, 2);
    like.transform = [-10.0, 5.0, 0.0, 5.0, 0.0, -5.0];
    like.geo.wkt = Some("EPSG:4326".into());
    let out = run(
        "data.download",
        vec![("like", Value::Map(Map::new(Arc::new(like), vec![0.0; 8])))],
        serde_json::json!({"url": path.to_string_lossy(), "resampling": "near"}),
    );
    let l = layers(&out["raster"]);
    assert_eq!(l[0].1.len(), 8);
    let _ = std::fs::remove_dir_all(&dir);
}

fn window(source: serde_json::Value) -> Vec<(String, Vec<f64>)> {
    let mut p = source;
    p["extent"] = serde_json::json!([-47.2, -23.0, -46.8, -22.7]);
    p["cellsize"] = serde_json::json!(0.01);
    layers(&run("data.download", vec![], p)["raster"])
}

#[test]
#[ignore = "downloads from CHELSA"]
fn downloads_chelsa() {
    let l = window(serde_json::json!({"source": "chelsa", "layers": [1, 12]}));
    assert_eq!(l.len(), 2);
    assert!(l.iter().all(|(_, v)| v.iter().all(|x| x.is_finite())), "{:?}", &l[0].1[..4]);
}

#[test]
#[ignore = "downloads from the Copernicus DEM"]
fn downloads_copernicus_dem() {
    let l = window(serde_json::json!({"source": "copernicus_dem", "dem_resolution": 90}));
    // Campinas, Brazil: 500 to 1100 m
    assert!(l[0].1.iter().all(|x| (400.0..1300.0).contains(x)), "{:?}", &l[0].1[..4]);
}

#[test]
#[ignore = "downloads from SoilGrids"]
fn downloads_soilgrids() {
    let l = window(serde_json::json!({"source": "soilgrids", "property": "clay", "depth": ["0-5cm", "5-15cm"]}));
    assert_eq!(l.len(), 2);
    assert!(l[0].1.iter().any(|x| *x > 0.0));
}

#[test]
#[ignore = "downloads from MapBiomas"]
fn downloads_mapbiomas() {
    let l = window(serde_json::json!({"source": "mapbiomas", "collection": 10, "years": [2000, 2024]}));
    assert_eq!(l.len(), 2);
    assert!(l[1].1.iter().all(|x| x.is_nan() || (0.0..=75.0).contains(x) && x.fract() == 0.0));
}

#[test]
#[ignore = "downloads from WorldClim"]
fn downloads_worldclim() {
    let l = window(serde_json::json!({"source": "worldclim", "layers": [1]}));
    assert!(l[0].1.iter().all(|x| (10.0..30.0).contains(x)), "{:?}", &l[0].1[..4]);
}
