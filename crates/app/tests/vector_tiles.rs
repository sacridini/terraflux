//! Vector layers drawn as tiles: the features under a point (polygons with holes, lines, points), the tiles of
//! them, a layer of a quarter of a million polygons prepared and drawn in a few hundred milliseconds (release), a
//! tile deep in a polygon of 400 000 vertices drawn in a few milliseconds (its fill and hole the same), and
//! the viewer drawn off screen (pictures terraflux-vectors-*.png in the temporary folder): vectors alone, and the
//! clusters of zones over their map.

use std::path::Path;
use std::sync::Arc;
use std::time::Instant;

use egui_kittest::Harness;
use terraflux_app::map_view::{MapView, Source};
use terraflux_app::palette::Coloring;
use terraflux_app::vector_tiles::{Overlay, Style, pick, prepare, rasterize};
use terraflux_app::vector_view::grid_over;
use terraflux_raster::vector::{Column, Geometry, Vector};

fn square(x: f64, y: f64, s: f64) -> Vec<[f64; 2]> {
    vec![[x, y], [x + s, y], [x + s, y + s], [x, y + s], [x, y]]
}

/// A grid of n x n squares of side 0.8 a unit apart, an attribute of the row.
fn squares(n: usize) -> Vector {
    let mut geometries = Vec::with_capacity(n * n);
    let mut row = Vec::with_capacity(n * n);
    for i in 0..n {
        for j in 0..n {
            geometries.push(Geometry::Polygon(vec![square(j as f64, i as f64, 0.8)]));
            row.push((i % 7) as f64);
        }
    }
    Vector { geometries, attributes: vec![("row".into(), Column::Numbers(row))], crs: None }
}

#[test]
fn features_are_picked() {
    // a square with a hole, a line, a point
    let v = Arc::new(Vector {
        geometries: vec![
            Geometry::Polygon(vec![square(0.0, 0.0, 10.0), square(4.0, 4.0, 2.0)]),
            Geometry::LineString(vec![[20.0, 0.0], [30.0, 10.0]]),
            Geometry::Point([40.0, 5.0]),
        ],
        attributes: Vec::new(),
        crs: None,
    });
    let grid = grid_over(&v);
    let layer = prepare(&v, &grid).unwrap();
    let t = grid.transform;
    let cell = |x: f64, y: f64| [(x - t[0]) / t[1], (y - t[3]) / t[5]];
    let tol = 0.05 / t[1];
    assert_eq!(pick(&layer, cell(1.0, 1.0), tol), Some(0));
    assert_eq!(pick(&layer, cell(5.0, 5.0), tol), None, "the hole");
    assert_eq!(pick(&layer, cell(25.0, 5.02), tol), Some(1));
    assert_eq!(pick(&layer, cell(40.02, 5.0), tol), Some(2));
    assert_eq!(pick(&layer, cell(35.0, 5.0), tol), None);
    // a tile over the features has them drawn; one away from them is none
    let style = Style::default();
    let colours = vec![style.colour; 3];
    let img = rasterize(&layer, &colours, &style, 8, 0, 0).expect("the layer in one tile");
    assert!(img.pixels.iter().any(|p| p.a() > 0));
    assert!(rasterize(&layer, &colours, &style, 0, 1000, 1000).is_none());
}

#[test]
fn a_large_layer_is_quick() {
    let n = 500; // a quarter of a million polygons
    let v = Arc::new(squares(n));
    let grid = grid_over(&v);
    let t0 = Instant::now();
    let layer = prepare(&v, &grid).unwrap();
    let prepared = t0.elapsed();
    let style = Style { by: Some("row".into()), ..Style::default() };
    let colours = terraflux_app::vector_tiles::colours(&v, &style);
    // the whole layer in one tile (every feature a dot), then a tile of the finest level in view
    let t1 = Instant::now();
    let top = rasterize(&layer, &colours, &style, 8, 0, 0).unwrap();
    let whole = t1.elapsed();
    let t2 = Instant::now();
    let fine = rasterize(&layer, &colours, &style, 3, 10, 10).unwrap();
    let detail = t2.elapsed();
    eprintln!(
        "{} polygons: prepared in {prepared:?}, the whole in a tile in {whole:?}, a tile of detail in {detail:?}",
        n * n
    );
    assert!(top.pixels.iter().filter(|p| p.a() > 0).count() > 30_000);
    assert!(fine.pixels.iter().any(|p| p.a() > 0));
    if !cfg!(debug_assertions) {
        assert!(prepared.as_secs_f64() < 2.0 && whole.as_secs_f64() < 2.0 && detail.as_secs_f64() < 0.5);
    }
}

#[test]
fn a_tile_of_a_large_polygon_draws_what_is_over_it() {
    // a disc of 200 000 vertices, a hole of as many in its middle
    let ring = |r: f64| -> Vec<[f64; 2]> {
        (0..=200_000)
            .map(|k| {
                let a = k as f64 / 200_000.0 * std::f64::consts::TAU;
                [r * a.cos(), r * a.sin()]
            })
            .collect()
    };
    let v = Arc::new(Vector {
        geometries: vec![Geometry::Polygon(vec![ring(1000.0), ring(500.0)])],
        attributes: Vec::new(),
        crs: None,
    });
    let grid = grid_over(&v);
    let layer = prepare(&v, &grid).unwrap();
    let style = Style { fill: 1.0, width: 0.0, ..Style::default() };
    let colours = vec![style.colour];
    let t = grid.transform;
    // a tile of the level -4 (16 cells), deep in: its runs off it drawn as lines, its fill the same
    let tile = |x: f64, y: f64| {
        let c = [(x - t[0]) / t[1], (y - t[3]) / t[5]];
        let t0 = Instant::now();
        let img = rasterize(&layer, &colours, &style, -4, (c[0] / 16.0).floor() as i64, (c[1] / 16.0).floor() as i64);
        (img.expect("the polygon over the tile"), t0.elapsed())
    };
    let (inside, took) = tile(750.0, 0.0);
    assert!(inside.pixels.iter().all(|p| p.a() > 0), "the tile between the hole and the border filled");
    let (hole, _) = tile(0.0, 250.0);
    assert!(hole.pixels.iter().all(|p| p.a() == 0), "the hole empty");
    let (border, _) = tile(1000.0, 0.0);
    let filled = border.pixels.iter().filter(|p| p.a() > 0).count();
    assert!(filled > 0 && filled < 256 * 256, "the border across the tile");
    // outlined, on the border
    let outlined = Style::default();
    let c = [(1000.0 - t[0]) / t[1], (0.0 - t[3]) / t[5]];
    let t0 = Instant::now();
    let img = rasterize(&layer, &colours, &outlined, -4, (c[0] / 16.0).floor() as i64, (c[1] / 16.0).floor() as i64);
    let outline = t0.elapsed();
    assert!(img.unwrap().pixels.iter().any(|p| p.a() > 0));
    eprintln!("a tile of 400 000 vertices: filled in {took:?}, outlined in {outline:?}");
    if !cfg!(debug_assertions) {
        assert!(took.as_secs_f64() < 0.005 && outline.as_secs_f64() < 0.005, "a tile of 400 000 vertices");
    }
}

fn render(h: &mut Harness<'_, ()>, name: &str) {
    if let Ok(img) = h.render() {
        img.save(std::env::temp_dir().join(format!("terraflux-vectors-{name}.png"))).unwrap();
    }
}

/// Step until the view's tiles are made, a minute at most.
fn settle(h: &mut Harness<'_, ()>, view: &std::sync::Mutex<MapView>) {
    let t = Instant::now();
    h.run_steps(3);
    while view.lock().unwrap().busy() {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(t.elapsed().as_secs() < 60, "the tiles take too long");
    }
    h.run_steps(3);
}

fn harness(f: impl FnMut(&mut egui::Ui) + 'static) -> Harness<'static, ()> {
    let rs = egui_kittest::wgpu::create_render_state(
        egui_kittest::wgpu::default_wgpu_setup(),
        eframe::egui_wgpu::RendererOptions::PREDICTABLE,
    );
    let mut f = f;
    Harness::builder()
        .with_size((1000.0, 700.0))
        .renderer(egui_kittest::wgpu::WgpuTestRenderer::from_render_state(rs))
        .build_ui(move |ui| {
            terraflux_app::theme::apply(ui.ctx());
            f(ui)
        })
}

#[test]
fn the_viewer_draws_vectors() {
    // a layer of 40,000 squares alone, coloured by an attribute
    let v = Arc::new(squares(200));
    let view = Arc::new(std::sync::Mutex::new(MapView::default()));
    let (vv, vw) = (v.clone(), view.clone());
    let mut h = harness(move |ui| {
        terraflux_app::vector_view::show(ui, &vv, &mut vw.lock().unwrap(), egui::Id::new("squares"));
    });
    h.ctx.data_mut(|d| {
        d.insert_temp(egui::Id::new("squares").with("style"), Style { by: Some("row".into()), ..Style::default() })
    });
    settle(&mut h, &view);
    render(&mut h, "alone");

    // the clusters of the zones of augusta.tif over its map
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extern/landschaft/tests/data/lsm/augusta.tif");
    let raster = terraflux_raster::Raster::read(&path).unwrap();
    let map = terraflux_ops::Map::new(Arc::new(raster.grid.clone()), raster.bands[0].masked(raster.nodata));
    let run = |op: &str, ins: Vec<(&str, terraflux_ops::Value)>, p: serde_json::Value| {
        let params = p.as_object().cloned().unwrap();
        let args = terraflux_ops::Args {
            node: "n",
            inputs: ins.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            params: &params,
        };
        terraflux_ops::Registry::standard().get(op).unwrap().run(&mut terraflux_ops::Ctx::default(), &args).unwrap()
    };
    let zones = run(
        "vector.grid_zones",
        vec![("map", terraflux_ops::Value::Map(map.clone()))],
        serde_json::json!({"size": 600}),
    );
    let sig = run(
        "metrics.signatures",
        vec![("maps", terraflux_ops::Value::Map(map.clone()))],
        serde_json::json!({"grid": 600, "landscape_metrics": ["shdi", "ed"], "class_metrics": ["pland"]}),
    );
    let clusters = run("metrics.cluster", vec![("table", sig["table"].clone())], serde_json::json!({"k": 4}));
    let joined = run(
        "vector.join",
        vec![("vector", zones["zones"].clone()), ("table", clusters["table"].clone())],
        serde_json::json!({"on": "zone"}),
    );
    let terraflux_ops::Value::Vector(zv) = &joined["vector"] else { panic!() };
    let overlays = vec![Overlay {
        vector: zv.clone(),
        style: Style { by: Some("cluster".into()), fill: 0.45, ..Style::default() },
    }];
    let view = Arc::new(std::sync::Mutex::new(MapView::default()));
    let vw = view.clone();
    let coloring = Coloring::for_values(&map.data);
    let source = Source::Map(map);
    let mut h = harness(move |ui| {
        vw.lock().unwrap().show_layers(ui, &source, &coloring, &overlays);
    });
    settle(&mut h, &view);
    render(&mut h, "over-map");
}
