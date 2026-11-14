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

/// The web drivers (WMTS, WFS, OAPIF) are told, not required: a GDAL built without curl has none of them.
#[test]
fn the_web_drivers_are_told() {
    if gdal().is_err() {
        return;
    }
    for d in ["WMTS", "WMS", "WFS", "OAPIF"] {
        eprintln!("{d}: {}", terraflux_gdal::has_driver(d));
    }
    assert!(terraflux_gdal::has_driver("GeoJSON"));
    assert!(!terraflux_gdal::has_driver("No Such Driver"));
}

/// A web server on 127.0.0.1 (a port the system chose) answering `body` for `/features.geojson` with HTTP ranges, as
/// GDAL's /vsicurl/ asks. Its port.
fn serve(body: &'static str) -> u16 {
    use std::io::{BufRead, BufReader, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || {
                let mut out = stream.try_clone().unwrap();
                let mut input = BufReader::new(stream);
                loop {
                    let mut first = String::new();
                    if input.read_line(&mut first).unwrap_or(0) == 0 {
                        return;
                    }
                    let mut range = None;
                    loop {
                        let mut l = String::new();
                        if input.read_line(&mut l).unwrap_or(0) == 0 || l == "\r\n" {
                            break;
                        }
                        if let Some((k, v)) = l.split_once(':')
                            && k.eq_ignore_ascii_case("range")
                        {
                            range = Some(v.trim().to_string());
                        }
                    }
                    let mut parts = first.split_whitespace();
                    let (method, path) = (parts.next().unwrap_or(""), parts.next().unwrap_or("/"));
                    let all = body.as_bytes();
                    let answer: Vec<u8> = if path.split('?').next() != Some("/features.geojson") {
                        b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_vec()
                    } else if method == "HEAD" {
                        format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nAccept-Ranges: bytes\r\n\r\n", all.len())
                            .into_bytes()
                    } else {
                        let span = range.as_deref().and_then(|r| r.strip_prefix("bytes=")).and_then(|r| {
                            let (a, b) = r.split_once('-')?;
                            Some((a.parse::<usize>().ok()?, b.parse().unwrap_or(all.len() - 1).min(all.len() - 1)))
                        });
                        match span {
                            Some((a, b)) if a <= b => {
                                let mut v = format!(
                                    "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {a}-{b}/{}\r\n\r\n",
                                    b - a + 1,
                                    all.len()
                                )
                                .into_bytes();
                                v.extend_from_slice(&all[a..=b]);
                                v
                            }
                            _ => {
                                let mut v =
                                    format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n", all.len()).into_bytes();
                                v.extend_from_slice(all);
                                v
                            }
                        }
                    };
                    if out.write_all(&answer).is_err() {
                        return;
                    }
                    let _ = out.flush();
                }
            });
        }
    });
    port
}

/// Five points of a GeoJSON (WGS 84): a, b, c near 10 to 12 degrees, d and e far away.
const FIVE: &str = r#"{"type":"FeatureCollection","features":[
 {"type":"Feature","properties":{"name":"a","n":1},"geometry":{"type":"Point","coordinates":[10.0,10.0]}},
 {"type":"Feature","properties":{"name":"b","n":2},"geometry":{"type":"Point","coordinates":[11.0,11.0]}},
 {"type":"Feature","properties":{"name":"c","n":3},"geometry":{"type":"Point","coordinates":[12.0,12.0]}},
 {"type":"Feature","properties":{"name":"d","n":4},"geometry":{"type":"Point","coordinates":[20.0,20.0]}},
 {"type":"Feature","properties":{"name":"e","n":5},"geometry":{"type":"Point","coordinates":[30.0,30.0]}}]}"#;

fn names(v: &terraflux_raster::vector::Vector) -> Vec<String> {
    match v.column("name") {
        Some(terraflux_raster::vector::Column::Texts(t)) => t.clone(),
        _ => panic!("no names"),
    }
}

/// The features of a box only, of a layer on the web and of a file: in the layer's coordinates and in another system's
/// (the box carried to the layer's).
#[test]
fn features_are_read_in_a_box() {
    use terraflux_gdal::{Transform, read_vector, read_vector_in};
    if gdal().is_err() {
        return;
    }
    let port = serve(FIVE);
    let url = format!("/vsicurl/http://127.0.0.1:{port}/features.geojson");
    assert_eq!(read_vector(&url, None).unwrap().len(), 5, "whole");
    // the box keeps b and c
    let v = read_vector_in(&url, None, [10.5, 10.5, 12.5, 12.5], None).unwrap();
    assert_eq!(names(&v), ["b", "c"]);
    assert_eq!(v.attributes.len(), 2, "its attributes");
    assert!(v.crs.as_deref().is_some_and(|c| c.contains("WGS 84")), "its CRS");
    // the box of the layer's own system, said
    let v = read_vector_in(&url, None, [10.5, 10.5, 12.5, 12.5], Some("EPSG:4326")).unwrap();
    assert_eq!(names(&v), ["b", "c"]);
    // the box in web Mercator, carried to the layer's degrees
    let (mut xs, mut ys) = ([10.5, 12.5], [10.5, 12.5]);
    Transform::new("EPSG:4326", "EPSG:3857").unwrap().points(&mut xs, &mut ys);
    let v = read_vector_in(&url, None, [xs[0], ys[0], xs[1], ys[1]], Some("EPSG:3857")).unwrap();
    assert_eq!(names(&v), ["b", "c"], "a box in another system");
    // a box with nothing in it: no features, no error; a file the same
    assert_eq!(read_vector_in(&url, None, [50.0, 50.0, 60.0, 60.0], None).unwrap().len(), 0);
    let file = tmp("five.geojson");
    std::fs::write(&file, FIVE).unwrap();
    assert_eq!(names(&read_vector_in(&file, None, [9.0, 9.0, 10.5, 10.5], None).unwrap()), ["a"]);
    // errors say where
    let e = read_vector_in(format!("/vsicurl/http://127.0.0.1:{port}/none.geojson"), None, [0.0; 4], None).unwrap_err();
    assert!(e.contains("none.geojson"), "{e}");
}

#[test]
fn the_geographic_system_of_a_projected_one_is_its_own_datum() {
    if gdal().is_err() {
        return;
    }
    use terraflux_gdal::{Transform, geographic_of, is_geographic, same_crs};
    // SAD69 / UTM 23S has SAD69's longitudes and latitudes, WGS 84 has itself
    let sad = geographic_of("EPSG:29193").unwrap();
    assert!(is_geographic(&sad).unwrap());
    assert!(same_crs(&sad, "EPSG:4618").unwrap(), "SAD69");
    assert!(!same_crs(&sad, "EPSG:4326").unwrap());
    assert!(same_crs(&geographic_of("EPSG:32651").unwrap(), "EPSG:4326").unwrap(), "WGS 84");
    assert!(same_crs(&geographic_of("EPSG:4326").unwrap(), "EPSG:4326").unwrap());
    // longitude first, as the other transformations: the central meridian of zone 23 at the equator
    let (mut x, mut y) = ([-45.0], [0.0]);
    Transform::new(&sad, "EPSG:29193").unwrap().points(&mut x, &mut y);
    assert!((x[0] - 500_000.0).abs() < 1.0 && (y[0] - 10_000_000.0).abs() < 1.0, "{x:?} {y:?}");
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
