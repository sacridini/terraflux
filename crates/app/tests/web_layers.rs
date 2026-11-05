//! Data on the web as layers (a part of the `ui` tests): a COG or vector features by their address, read by
//! windows; a basemap of XYZ tiles of the user's own. All against a server on this computer (127.0.0.1, a port the
//! system chose): never the internet.

use super::*;

/// The tests that make basemaps of the user's own change the one list of the settings: one at a time.
static CUSTOMS: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// A request the test server answered.
#[derive(Clone, Debug)]
struct Hit {
    method: String,
    path: String,
    range: Option<String>,
    status: u16,
    /// The bytes of the body sent.
    sent: usize,
}

/// A web server on 127.0.0.1 answering what `files` gives for a path (none: a 404) with HTTP ranges (206 and
/// `Content-Range`), and HEAD with `Content-Length` and `Accept-Ranges`, as GDAL's /vsicurl/ needs; what it
/// answered is kept.
struct Web {
    port: u16,
    log: std::sync::Arc<std::sync::Mutex<Vec<Hit>>>,
}

impl Web {
    fn new(files: impl Fn(&str) -> Option<Vec<u8>> + Send + Sync + 'static) -> Web {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let log: std::sync::Arc<std::sync::Mutex<Vec<Hit>>> = Default::default();
        let (files, kept) = (std::sync::Arc::new(files), log.clone());
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let (files, kept) = (files.clone(), kept.clone());
                std::thread::spawn(move || {
                    let mut out = stream.try_clone().unwrap();
                    let mut input = BufReader::new(stream);
                    // (requests one after the other on the connection, until the client closes it)
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
                        let method = parts.next().unwrap_or("").to_string();
                        let path = parts.next().unwrap_or("/").to_string();
                        let (status, head, bytes) =
                            answer(&method, range.as_deref(), files(path.split('?').next().unwrap()));
                        let sent = if method == "HEAD" { 0 } else { bytes.len() };
                        if out.write_all(head.as_bytes()).is_err() || out.write_all(&bytes).is_err() {
                            return;
                        }
                        let _ = out.flush();
                        kept.lock().unwrap().push(Hit { method, path, range, status, sent });
                    }
                });
            }
        });
        Web { port, log }
    }

    /// A server of files by their path (`/name.tif`).
    fn of_files(files: Vec<(&str, Vec<u8>)>) -> Web {
        let files: Vec<(String, Vec<u8>)> = files.into_iter().map(|(p, b)| (p.to_string(), b)).collect();
        Web::new(move |p| files.iter().find(|(f, _)| f == p).map(|(_, b)| b.clone()))
    }

    fn url(&self, path: &str) -> String {
        format!("http://127.0.0.1:{}{path}", self.port)
    }

    fn hits(&self) -> Vec<Hit> {
        self.log.lock().unwrap().clone()
    }

    /// The bytes of bodies sent for a path.
    fn bytes_of(&self, path: &str) -> usize {
        self.hits().iter().filter(|h| h.path == path).map(|h| h.sent).sum()
    }
}

/// The answer to a request: its status, head and body.
fn answer(method: &str, range: Option<&str>, file: Option<Vec<u8>>) -> (u16, String, Vec<u8>) {
    let Some(all) = file else {
        return (404, "HTTP/1.1 404 Not Found\r\nContent-Length: 9\r\n\r\n".into(), b"not found".to_vec());
    };
    let total = all.len();
    // bytes=a-b, bytes=a-, bytes=-n
    let span = range.and_then(|r| r.strip_prefix("bytes=")).and_then(|r| {
        let (a, b) = r.split_once('-')?;
        Some(if a.is_empty() {
            (total.saturating_sub(b.parse::<usize>().ok()?), total - 1)
        } else {
            (a.parse().ok()?, b.parse().unwrap_or(total - 1).min(total - 1))
        })
    });
    match (method, span) {
        ("HEAD", _) => {
            (200, format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nAccept-Ranges: bytes\r\n\r\n"), Vec::new())
        }
        (_, Some((a, b))) if a <= b => (
            206,
            format!(
                "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {a}-{b}/{total}\r\nAccept-Ranges: bytes\r\n\r\n",
                b - a + 1
            ),
            all[a..=b].to_vec(),
        ),
        _ => (200, format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nAccept-Ranges: bytes\r\n\r\n"), all),
    }
}

/// A raster of noise (f32, `side` x `side` cells) as a plain GeoTIFF and as a COG (tiles of 256, overviews, UTM 51N),
/// and its cells: a file of some MB, large enough to tell a window from the whole.
fn noise_cog(name: &str, side: usize) -> (PathBuf, PathBuf, Vec<f32>) {
    let mut x = 0x9E3779B97F4A7C15u64;
    let cells: Vec<f32> = (0..side * side)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 40) as f32 / 1000.0
        })
        .collect();
    let mut grid = terraflux_raster::Grid::new(side, side);
    grid.transform = [500000.0, 30.0, 0.0, 1400000.0, 0.0, -30.0];
    let plain = tmp(&format!("{name}-plain.tif"));
    let cog = tmp(&format!("{name}-cog.tif"));
    let data = cells.iter().map(|&v| v as f64).collect();
    terraflux_raster::Raster::single(grid, data, terraflux_raster::DataType::F32, None).write(&plain).unwrap();
    let args: Vec<String> = ["-of", "COG", "-a_srs", "EPSG:32651", "-co", "BLOCKSIZE=256", "-co", "COMPRESS=NONE"]
        .iter()
        .map(|a| a.to_string())
        .collect();
    terraflux_gdal::translate(&plain, &cog, &args).unwrap();
    (plain, cog, cells)
}

/// Step until the runs for the layers just added are done and their values are there.
fn layers_run(h: &mut Harness<TerrafluxApp>, ids: &[&str]) {
    until(h, "the layers' runs", |a| {
        let s = a.session.as_ref().unwrap();
        s.show_queue.is_empty()
            && !s.running()
            && ids.iter().all(|id| s.value_of(id, "raster").is_some() || s.value_of(id, "vector").is_some())
    });
}

/// A COG on the web added as a layer by its address: a `raster.read` of `/vsicurl/...` noted where it was read from
/// (one step of undo), its values the file's, read by ranges and by windows: at the view of all of it and zoomed in
/// to a few cells, far fewer bytes than the file.
#[test]
fn a_cog_on_the_web_added_as_a_layer_by_its_url() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let side = 2000;
    let (plain, cog, cells) = noise_cog("web-noise", side);
    let bytes = std::fs::read(&cog).unwrap();
    let size = bytes.len();
    assert!(size > 16_000_000, "a file of some MB: {size}");
    let web = Web::of_files(vec![("/maps/noise.tif", bytes)]);
    let url = web.url("/maps/noise.tif");
    // what opening it tells
    let info = terraflux_app::remote::probe(&url).unwrap();
    assert_eq!((info.kind, info.size, info.bands), (terraflux_app::remote::Kind::Raster, (side, side), 1));
    assert!(info.overviews > 0 && info.driver == "GTiff", "{info:?}");
    assert!(info.describe().contains("2000 x 2000") && info.describe().contains("UTM zone 51N"), "{}", info.describe());
    let r = terraflux_ops::Registry::standard();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    h.state_mut().focus(Pane::Viewer);
    h.run_steps(2);
    assert!(!h.state().session.as_ref().unwrap().history.can_undo());
    let id = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = terraflux_app::builder::add_url(s, &r, &url).unwrap();
        let n = s.source.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(n.op, "raster.read");
        assert_eq!(n.params["path"], format!("/vsicurl/{url}"));
        assert_eq!(n.note.as_deref(), Some(format!("read from {url}").as_str()));
        assert_eq!(s.preview_layers.len(), 1, "a layer of the Viewer");
        id
    };
    layers_run(&mut h, &[&id]);
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the map drawn", |a| a.session.as_ref().unwrap().preview_view.visible().is_some());
    drawn(&mut h);
    let path = "/maps/noise.tif";
    // the whole map in view: its overview by ranges of bytes, not the file
    let hits: Vec<Hit> = web.hits().into_iter().filter(|h| h.path == path).collect();
    let gets: Vec<&Hit> = hits.iter().filter(|h| h.method == "GET").collect();
    assert!(gets.iter().all(|h| h.range.is_some() && h.status == 206), "{hits:?}");
    assert!(gets.len() > 1, "by ranges");
    let whole = web.bytes_of(path);
    eprintln!("a COG of {size} bytes: {whole} bytes for the view of all of it, {} requests", hits.len());
    assert!(whole > 0 && whole < size / 2, "{whole} of {size}");
    // zoomed in to a few cells: a few blocks more
    {
        let v = &mut h.state_mut().session.as_mut().unwrap().preview_view;
        v.zoom_to([1000.0, 1000.0, 1060.0, 1060.0]);
    }
    h.run_steps(3);
    drawn(&mut h);
    let zoomed = web.bytes_of(path) - whole;
    eprintln!("zoomed in to 60 x 60 cells: {zoomed} bytes more, {} in all", web.bytes_of(path));
    assert!(h.state().session.as_ref().unwrap().preview_view.zoom() > 5.0, "zoomed in");
    assert!(zoomed > 0 && zoomed < size / 8, "{zoomed} more of {size}");
    assert!(h.state().session.as_ref().unwrap().preview_view.error().is_none());
    // one step of undo: the node gone
    {
        let s = h.state_mut().session.as_mut().unwrap();
        assert!(s.history.can_undo());
        let mut m = s.source.clone();
        assert!(s.history.undo(&mut m));
        assert!(m.nodes.is_empty() && !s.history.can_undo(), "one step");
    }
    // the cells as the file's
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Raster(remote)) = s.value_of(&id, "raster") else { panic!("a raster") };
    let local = terraflux_ops::files::Rasters::open(&cog.to_string_lossy(), None, None).unwrap();
    assert_eq!((remote.grid.width, remote.grid.height), (side, side));
    assert_eq!(remote.grid.transform, local.grid.transform);
    for (cx, cy) in [(0usize, 0usize), (1234, 77), (1999, 1999), (500, 1500)] {
        let win = terraflux_raster::tiff::Window { x0: cx as i64, y0: cy as i64, width: 1, height: 1 };
        let (a, b) = (remote.window(0, win).unwrap(), local.window(0, win).unwrap());
        assert_eq!(a, b, "cell {cx}, {cy}");
        assert_eq!(a[0], cells[cy * side + cx] as f64, "the source's cell {cx}, {cy}");
    }
    assert!(web.bytes_of(path) < size / 2, "never the whole file: {} of {size}", web.bytes_of(path));
    let _ = std::fs::remove_file(plain);
}

/// An address that is not there, or not a raster: the dialog says why (no node); the one that is: what it holds, then
/// a node on *Add as a layer*.
#[test]
fn the_dialog_of_a_url_says_what_is_there_and_why_not() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let (plain, cog, _) = noise_cog("web-dialog", 300);
    let web =
        Web::of_files(vec![("/ok.tif", std::fs::read(&cog).unwrap()), ("/text.tif", b"not a raster at all".to_vec())]);
    // the errors
    let e = terraflux_app::remote::probe(&web.url("/none.tif")).unwrap_err();
    assert!(e.contains("none.tif"), "{e}");
    let e = terraflux_app::remote::probe(&web.url("/text.tif")).unwrap_err();
    assert!(e.contains("text.tif"), "{e}");
    assert!(web.hits().iter().any(|h| h.path == "/none.tif" && h.status == 404));
    let mut h = harness(new_app(None));
    h.run_steps(2);
    h.state_mut().focus(Pane::Viewer);
    let nodes = h.state().session.as_ref().unwrap().source.nodes.len();
    h.state_mut().session.as_mut().unwrap().url_layer =
        Some(terraflux_app::remote::Dialog::with(&web.url("/none.tif")));
    h.run_steps(3);
    h.get_by_label("Check").click();
    until(&mut h, "the check", |a| {
        a.session.as_ref().unwrap().url_layer.as_ref().is_some_and(|d| !d.checking() && d.result.is_some())
    });
    h.run_steps(2);
    {
        let s = h.state().session.as_ref().unwrap();
        let d = s.url_layer.as_ref().unwrap();
        assert!(matches!(d.found(), Some(Err(e)) if e.contains("none.tif")));
        assert_eq!(s.source.nodes.len(), nodes, "no node for an address that is not there");
    }
    // the one that is there
    h.state_mut().session.as_mut().unwrap().url_layer.as_mut().unwrap().url = web.url("/ok.tif");
    h.run_steps(2);
    h.get_by_label("Check").click();
    until(&mut h, "the check", |a| {
        a.session.as_ref().unwrap().url_layer.as_ref().is_some_and(|d| !d.checking() && d.found().is_some())
    });
    h.run_steps(2);
    {
        let d = h.state().session.as_ref().unwrap().url_layer.as_ref().unwrap();
        let Some(Ok(info)) = d.found() else { panic!("{:?}", d.found()) };
        assert_eq!((info.size, info.bands), ((300, 300), 1));
    }
    h.get_by_label("Add as a layer").click();
    h.run_steps(3);
    let s = h.state().session.as_ref().unwrap();
    assert!(s.url_layer.is_none(), "the window closes");
    assert_eq!(s.source.nodes.len(), nodes + 1);
    let n = s.source.nodes.last().unwrap();
    assert_eq!((n.op.as_str(), n.id.as_str()), ("raster.read", "ok"));
    assert_eq!(n.params["path"], format!("/vsicurl/{}", web.url("/ok.tif")));
    assert!(s.preview_layers.iter().any(|l| l.node == "ok"));
    let _ = std::fs::remove_file(plain);
}

/// A server that stops answering the windows (after the file was opened): the layer says so (its error), nothing
/// crashes, and the window goes on.
#[test]
fn a_server_that_fails_is_a_layer_error_not_a_crash() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let (plain, cog, _) = noise_cog("web-fails", 600);
    let bytes = std::fs::read(&cog).unwrap();
    let failing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let flag = failing.clone();
    let web = Web::new(move |p| {
        (p == "/fails.tif" && !flag.load(std::sync::atomic::Ordering::Relaxed)).then(|| bytes.clone())
    });
    let r = terraflux_ops::Registry::standard();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let id = {
        let s = h.state_mut().session.as_mut().unwrap();
        terraflux_app::builder::add_url(s, &r, &web.url("/fails.tif")).unwrap()
    };
    layers_run(&mut h, &[&id]);
    // (the header was read: the file goes away, its windows are asked for now)
    failing.store(true, std::sync::atomic::Ordering::Relaxed);
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the layer's error", |a| a.session.as_ref().unwrap().preview_view.error().is_some());
    h.run_steps(3);
    let e = h.state().session.as_ref().unwrap().preview_view.error().unwrap().to_string();
    assert!(e.contains("fails.tif"), "{e}");
    assert!(web.hits().iter().any(|h| h.status == 404 && h.path == "/fails.tif"));
    let _ = std::fs::remove_file(plain);
}

/// Vector features on the web (GeoJSON) are a `vector.read` of `/vsicurl/...`, read by GDAL.
#[test]
fn vector_features_on_the_web_added_as_a_layer() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let geojson = r#"{"type":"FeatureCollection","features":[
        {"type":"Feature","properties":{"name":"a"},"geometry":{"type":"Point","coordinates":[122.5,12.4]}},
        {"type":"Feature","properties":{"name":"b"},"geometry":{"type":"Point","coordinates":[122.6,12.5]}}]}"#;
    let web = Web::of_files(vec![("/points.geojson", geojson.as_bytes().to_vec())]);
    let url = web.url("/points.geojson");
    let info = terraflux_app::remote::probe(&url).unwrap();
    assert_eq!(info.kind, terraflux_app::remote::Kind::Vector);
    assert!(!info.layers.is_empty() && info.describe().starts_with("vector features"), "{}", info.describe());
    let r = terraflux_ops::Registry::standard();
    let mut h = harness(new_app(None));
    h.run_steps(2);
    h.state_mut().focus(Pane::Viewer);
    let id = {
        let s = h.state_mut().session.as_mut().unwrap();
        let id = terraflux_app::builder::add_url(s, &r, &url).unwrap();
        let n = s.source.nodes.iter().find(|n| n.id == id).unwrap();
        assert_eq!(n.op, "vector.read");
        assert_eq!(n.params["path"], format!("/vsicurl/{url}"));
        id
    };
    layers_run(&mut h, &[&id]);
    let s = h.state().session.as_ref().unwrap();
    let Some(terraflux_ops::Value::Vector(v)) = s.value_of(&id, "vector") else { panic!("features") };
    assert_eq!(v.len(), 2);
    // an address that is not one
    let s = h.state_mut().session.as_mut().unwrap();
    assert!(terraflux_app::builder::add_url(s, &r, "data/points.geojson").unwrap_err().contains("not an address"));
}

/// A tile of one colour as a PNG.
fn tile_png(rgb: [u8; 3]) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(256, 256, image::Rgb(rgb));
    let mut out = std::io::Cursor::new(Vec::new());
    img.write_to(&mut out, image::ImageFormat::Png).unwrap();
    out.into_inner()
}

/// A basemap of the user's own, by an address with {z}/{x}/{y}: its tiles fetched from the server with the right
/// place in the address, kept on disk in a folder of their own, drawn under the map.
#[test]
fn a_custom_basemap_of_xyz_tiles_is_fetched_and_drawn() {
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let png = tile_png([200, 40, 40]);
    let web = Web::new(move |p| p.starts_with("/tiles/").then(|| png.clone()));
    let mut h = harness(new_app(None));
    h.run_steps(2);
    let r = terraflux_ops::Registry::standard();
    {
        let s = h.state_mut().session.as_mut().unwrap();
        let path = data("sibuyan_landuse.tif").to_string_lossy().into_owned();
        terraflux_app::builder::add(s, &r, "raster.read", &[("path", path.into())]).unwrap();
        terraflux_app::builder::add(
            s,
            &r,
            "raster.reproject",
            &[("crs", "EPSG:32651".into()), ("source_crs", "EPSG:32651".into())],
        )
        .unwrap();
    }
    h.run_steps(2);
    h.state_mut().focus(Pane::Viewer);
    until(&mut h, "the map with a CRS", |a| {
        let s = a.session.as_ref().unwrap();
        !s.running() && s.preview_value().is_some()
    });
    // an address that is not one is refused, the one that is made the basemap (written with ${z} as well)
    assert!(terraflux_app::basemap::register("https://t.org/{z}/{x}.png", "", 5).is_err());
    let template = format!("http://127.0.0.1:{}/tiles/${{z}}/${{x}}/${{y}}.png", web.port);
    let basemap = {
        let _one = CUSTOMS.lock().unwrap_or_else(|e| e.into_inner());
        terraflux_app::basemap::register(&template, "(c) the test", 8).unwrap()
    };
    assert_eq!(basemap.name(), "Custom: 127.0.0.1");
    assert_eq!(basemap.attribution(), "(c) the test");
    let cache = terraflux_app::basemap::cache_dir(basemap);
    let _ = std::fs::remove_dir_all(&cache);
    {
        let s = h.state_mut().session.as_mut().unwrap();
        s.preview_view.basemap = basemap;
        s.preview_view.hide_data = true;
    }
    h.run_steps(2);
    let t = Instant::now();
    while h.state().session.as_ref().unwrap().preview_view.base_busy() && t.elapsed() < Duration::from_secs(60) {
        h.step();
        std::thread::sleep(Duration::from_millis(20));
    }
    h.run_steps(3);
    let (tiles, error) = h.state().session.as_ref().unwrap().preview_view.base_tiles();
    assert!(tiles > 0 && error.is_none(), "{tiles} tiles, {error:?}");
    // what was asked: /tiles/z/x/y.png, in the grid of its level, and the tile of Sibuyan (122.5 E, 12.4 N) among them
    let asked: Vec<(u32, u32, u32)> = web
        .hits()
        .iter()
        .map(|h| {
            let p: Vec<&str> = h.path.trim_start_matches("/tiles/").trim_end_matches(".png").split('/').collect();
            assert_eq!((h.method.as_str(), p.len(), h.status), ("GET", 3, 200), "{h:?}");
            (p[0].parse().unwrap(), p[1].parse().unwrap(), p[2].parse().unwrap())
        })
        .collect();
    assert!(!asked.is_empty());
    assert!(asked.iter().all(|&(z, x, y)| z <= 8 && x < 1 << z && y < 1 << z), "{asked:?}");
    let place = |z: u32| {
        let n = (1u32 << z) as f64;
        let lat = 12.4f64.to_radians();
        let y = (1.0 - (lat.tan() + 1.0 / lat.cos()).ln() / std::f64::consts::PI) / 2.0 * n;
        (((122.5 + 180.0) / 360.0 * n) as u32, y as u32)
    };
    let deepest = asked.iter().map(|a| a.0).max().unwrap();
    let (x, y) = place(deepest);
    assert!(asked.contains(&(deepest, x, y)), "tile {deepest}/{x}/{y} among {asked:?}");
    // kept on disk, by place, in a folder of its own
    assert!(cache.join(deepest.to_string()).join(x.to_string()).join(y.to_string()).is_file());
    // drawn: the tiles' colour in the view (the data hidden)
    drawn(&mut h);
    match h.render() {
        Ok(img) => {
            let near = |p: &image::Rgba<u8>| {
                (p[0] as i32 - 200).abs() < 14 && (p[1] as i32 - 40).abs() < 14 && (p[2] as i32 - 40).abs() < 14
            };
            let n = img.pixels().filter(|p| near(p)).count();
            assert!(n > 5000, "{n} pixels of the tiles' colour");
        }
        Err(e) => eprintln!("no renderer ({e}): the tiles not looked at"),
    }
    // each tile once: drawn again, nothing more fetched
    let fetched = web.hits().len();
    h.run_steps(5);
    assert_eq!(web.hits().len(), fetched);
    let _ = std::fs::remove_dir_all(&cache);
}

/// The view exported over a basemap of the user's own: its tiles fetched from the server on this computer (once,
/// kept on disk) and the second export read from the disk.
#[test]
fn the_view_exported_over_a_custom_basemap_fetches_its_tiles() {
    setup();
    if terraflux_gdal::gdal().is_err() {
        return;
    }
    let png = tile_png([40, 120, 200]);
    let web = Web::new(move |p| p.starts_with("/export/").then(|| png.clone()));
    let template = format!("http://127.0.0.1:{}/export/${{z}}/${{x}}/${{y}}.png", web.port);
    let basemap = {
        let _one = CUSTOMS.lock().unwrap_or_else(|e| e.into_inner());
        terraflux_app::basemap::register(&template, "(c) export", 1).unwrap()
    };
    let cache = terraflux_app::basemap::cache_dir(basemap);
    let _ = std::fs::remove_dir_all(&cache);
    // a map of one cell in the world's middle, in web Mercator
    let e = 20037508.342789244;
    let mut g = terraflux_raster::Grid::new(4, 4);
    g.transform = [-e, e / 2.0, 0.0, e, 0.0, -e / 2.0];
    g.geo.geokeys = Some(vec![1, 1, 0, 2, 1024, 0, 1, 1, 3072, 0, 1, 3857]);
    let data = (0..16).map(|i| if i == 5 { 1.0 } else { f64::NAN }).collect();
    let map = terraflux_ops::Map::new(std::sync::Arc::new(g), data);
    let panel = terraflux_app::figure::Panel {
        source: terraflux_app::map_view::Source::Map(map),
        coloring: terraflux_app::palette::Coloring::Classes(vec![(1.0, egui::Color32::from_rgb(250, 250, 250))]),
        names: Vec::new(),
        caption: String::new(),
        vectors: Vec::new(),
        rasters: Vec::new(),
    };
    let mut o = terraflux_app::export_view::Options::scaled(1);
    o.basemap = basemap;
    let file = std::env::temp_dir().join(format!("terraflux-export-tiles-{}.png", std::process::id()));
    let said =
        terraflux_app::export_view::save_with(&panel, &o, terraflux_app::export_view::Format::Png, &file).unwrap();
    assert!(!said.contains("basemap"), "{said}");
    let back = image::open(&file).unwrap().to_rgba8();
    assert_eq!(back.dimensions(), (4, 4));
    assert_eq!(back.get_pixel(1, 1).0, [250, 250, 250, 255], "the map's cell");
    assert_eq!(back.get_pixel(0, 0).0, [40, 120, 200, 255], "the basemap's tile");
    assert_eq!(back.get_pixel(3, 3).0, [40, 120, 200, 255]);
    let asked = web.hits();
    assert!(!asked.is_empty() && asked.iter().all(|h| h.method == "GET" && h.status == 200), "{asked:?}");
    // again: the tiles come from the disk, nothing more is fetched
    terraflux_app::export_view::save_with(&panel, &o, terraflux_app::export_view::Format::Png, &file).unwrap();
    assert_eq!(web.hits().len(), asked.len());
    std::fs::remove_file(&file).ok();
    let _ = std::fs::remove_dir_all(&cache);
}

/// The basemaps of the user's own are kept in the settings file: the last used first, five at most, and what is
/// read back is what was kept.
#[test]
fn the_custom_basemaps_are_kept_in_the_settings() {
    setup();
    let _one = CUSTOMS.lock().unwrap_or_else(|e| e.into_inner());
    let make = |k: usize| format!("https://tiles{k}.example.org/{{z}}/{{x}}/{{y}}.png");
    for k in 0..7 {
        terraflux_app::basemap::register(&make(k), &format!("(c) {k}"), 10 + k as u32).unwrap();
    }
    let kept = terraflux_app::settings::get().custom_basemaps;
    assert_eq!(kept.len(), terraflux_app::basemap::MOST_CUSTOM);
    assert_eq!(kept[0].template, make(6));
    assert_eq!(kept[4].template, make(2), "the oldest two gone");
    // one used again: first, its attribution and zoom renewed, no copy
    terraflux_app::basemap::register(&make(3), "(c) again", 12).unwrap();
    let kept = terraflux_app::settings::get().custom_basemaps;
    assert_eq!(kept.len(), 5);
    assert_eq!(
        (kept[0].template.as_str(), kept[0].attribution.as_str(), kept[0].max_zoom),
        (make(3).as_str(), "(c) again", 12)
    );
    assert_eq!(kept.iter().filter(|k| k.template == make(3)).count(), 1);
    // the file on disk reads back the same, and the chooser's list is made of it, the first the last used
    let file = terraflux_app::settings::file().unwrap();
    let back: terraflux_app::settings::Settings =
        serde_json::from_str(&std::fs::read_to_string(file).unwrap()).unwrap();
    assert_eq!(back.custom_basemaps, kept);
    let own = terraflux_app::basemap::customs();
    assert_eq!(own.len(), 5);
    assert_eq!(own[0].name(), "Custom: tiles3.example.org");
    assert_eq!(own[0].attribution(), "(c) again");
    // (the test's own settings: none left for the other tests' choosers)
    terraflux_app::settings::update(|s| s.custom_basemaps.clear());
}
