//! A window of an image segmented on its own (the Viewer's live segmentation): on an image of homogeneous blocks, a
//! window aligned with the blocks gives the objects of the whole image's segmentation restricted to it, also read
//! every second cell; and ESP over the window gives a curve with suggested scales.

use std::sync::Arc;

use terraflux_ops::objects::{Objects, Whole, Window, WindowSettings, esp_window, segment_window, segment_window_op};
use terraflux_ops::{Args, Ctx, Key, Registry, Stack, Value};
use terraflux_raster::Grid;

const SIDE: usize = 8;
const BLOCKS: usize = 6;

/// Two bands, a block of SIDE x SIDE cells each with its own value in each band.
fn bands() -> (Arc<Grid>, Vec<Arc<Vec<f64>>>) {
    let n = SIDE * BLOCKS;
    let value = |r: usize, c: usize, band: usize| {
        let k = (r / SIDE) * BLOCKS + c / SIDE;
        // (every block differs from its neighbours in both bands by a lot)
        100.0 * (1 + k * (3 + 2 * band) % 17) as f64
    };
    let data = (0..2).map(|b| Arc::new((0..n * n).map(|i| value(i / n, i % n, b)).collect::<Vec<f64>>())).collect();
    (Arc::new(Grid::new(n, n)), data)
}

fn settings() -> WindowSettings {
    WindowSettings { scale: 10.0, shape: 0.0, compactness: 0.5, ..Default::default() }
}

fn whole() -> Arc<Objects> {
    let (grid, data) = bands();
    let stack = Value::Stack(Stack { grid, keys: vec![Key::Name("a".into()), Key::Name("b".into())], layers: data });
    let params = serde_json::json!({ "scale": 10.0, "shape": 0.0 });
    let params = params.as_object().cloned().unwrap();
    let args = Args { node: "n", inputs: [("image".to_string(), stack)].into(), params: &params };
    let out = Registry::standard().get("segment.multiresolution").unwrap().run(&mut Ctx::default(), &args).unwrap();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

#[test]
fn a_window_aligned_with_blocks_is_the_whole_segmentations_restriction() {
    let (grid, data) = bands();
    let full = whole();
    assert_eq!(full.count, BLOCKS * BLOCKS, "the whole image is one object per block");
    let win = Window { x0: SIDE, y0: 2 * SIDE, width: 3 * SIDE, height: 2 * SIDE };
    let o = segment_window(&grid, &data, win, 1, &settings()).unwrap();
    assert_eq!((o.grid.width, o.grid.height), (3 * SIDE, 2 * SIDE));
    assert_eq!(o.count, 6, "three blocks by two");
    // (the same partition: two cells share an object in the window exactly when they do in the whole)
    let n = grid.width;
    let cells: Vec<(usize, usize)> = (0..win.height).flat_map(|r| (0..win.width).map(move |c| (r, c))).collect();
    for pair in cells.windows(7) {
        let (a, b) = (pair[0], pair[6]);
        let same_in = o.labels[a.0 * win.width + a.1] == o.labels[b.0 * win.width + b.1];
        let at = |(r, c): (usize, usize)| full.labels[(win.y0 + r) * n + win.x0 + c];
        assert_eq!(same_in, at(a) == at(b), "{a:?} and {b:?}");
    }
    // (and the window's objects are the cells the whole's objects have inside it)
    for r in 0..win.height {
        for c in 0..win.width {
            let (a, b) = (o.labels[r * win.width + c], o.labels[0]);
            let (fa, fb) = (full.labels[(win.y0 + r) * n + win.x0 + c], full.labels[win.y0 * n + win.x0]);
            assert_eq!(a == b, fa == fb);
        }
    }
}

#[test]
fn a_window_read_every_second_cell_is_placed_where_it_is() {
    let mut grid = Grid::new(SIDE * BLOCKS, SIDE * BLOCKS);
    grid.transform = [100.0, 2.0, 0.0, 500.0, 0.0, -2.0];
    let (_, data) = bands();
    let win = Window { x0: 2 * SIDE, y0: SIDE, width: 2 * SIDE, height: 2 * SIDE };
    assert_eq!(win.step_for(1_000_000), 1);
    assert_eq!(win.step_for(100), 2, "256 cells in at most 100: every second (8 x 8 = 64)");
    let o = segment_window(&grid, &data, win, 2, &settings()).unwrap();
    assert_eq!((o.grid.width, o.grid.height), (SIDE, SIDE));
    assert_eq!(o.count, 4, "two blocks by two, whatever the step");
    // (the window's corner, and a cell twice as wide)
    assert_eq!(
        o.grid.transform,
        [100.0 + 2.0 * (2 * SIDE) as f64 * 1.0, 4.0, 0.0, 500.0 - 2.0 * SIDE as f64, 0.0, -4.0]
    );
}

#[test]
fn a_window_outside_the_image_is_refused() {
    let (grid, data) = bands();
    let win = Window { x0: 40, y0: 0, width: 16, height: 8 };
    let Err(e) = segment_window(&grid, &data, win, 1, &settings()) else { panic!("outside the image") };
    assert!(e.to_string().contains("not inside"), "{e}");
    let bad = WindowSettings { scale: 0.0, ..settings() };
    let ok = Window { x0: 0, y0: 0, width: 8, height: 8 };
    assert!(segment_window(&grid, &data, ok, 1, &bad).is_err());
}

#[test]
fn esp_over_a_window_has_the_curve_of_segment_scale() {
    // (noisy blocks, so that the local variance means something)
    let (grid, data) = bands();
    let mut seed = 7u64;
    let noisy: Vec<Arc<Vec<f64>>> = data
        .iter()
        .map(|b| {
            Arc::new(
                b.iter()
                    .map(|v| {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        v + ((seed >> 40) as f64 / (1u64 << 24) as f64 - 0.5) * 40.0
                    })
                    .collect(),
            )
        })
        .collect();
    let win = Window { x0: 0, y0: 0, width: SIDE * BLOCKS, height: SIDE * BLOCKS };
    let names = ["a".to_string(), "b".to_string()];
    let t = esp_window(&grid, &noisy, &names, win, 1, 12, &WindowSettings::default()).unwrap();
    for c in ["scale", "objects", "lv_a", "lv_b", "roc_mean", "suggested"] {
        assert!(t.columns.iter().any(|x| x == c), "column {c}: {:?}", t.columns);
    }
    assert!(t.rows.len() >= 3);
    let scale = t.columns.iter().position(|c| c == "scale").unwrap();
    assert!(t.rows.windows(2).all(|w| w[1][scale] > w[0][scale]), "the scales grow");
}

/// Two noisy bands over smooth fields (a step across the middle), `w` by `h`; band 1 on another scale (values in the
/// thousands) so that the standardising matters.
fn fields(w: usize, h: usize) -> (Arc<Grid>, Vec<Arc<Vec<f64>>>) {
    let mut seed = 5u64;
    let mut noise = move || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 40) as f64 / (1u64 << 24) as f64 - 0.5
    };
    let band = |k: usize, noise: &mut dyn FnMut() -> f64| {
        let v: Vec<f64> = (0..w * h)
            .map(|i| {
                let (r, c) = ((i / w) as f64, (i % w) as f64);
                let step = if c < w as f64 / 2.0 { 0.0 } else { 3.0 };
                (step + ((r / 9.0).sin() + (c / 11.0).cos()) * 0.4 + noise() * 0.3) * if k == 1 { 1000.0 } else { 1.0 }
            })
            .collect();
        Arc::new(v)
    };
    let a = band(0, &mut noise);
    let b = band(1, &mut noise);
    (Arc::new(Grid::new(w, h)), vec![a, b])
}

fn stack_of(grid: &Arc<Grid>, data: &[Arc<Vec<f64>>]) -> Value {
    let keys = ["a", "b"].iter().take(data.len()).map(|n| Key::Name(n.to_string())).collect();
    Value::Stack(Stack { grid: grid.clone(), keys, layers: data.to_vec() })
}

fn own_run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Arc<Objects> {
    let params = params.as_object().cloned().unwrap();
    let inputs = inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect();
    let args = Args { node: "n", inputs, params: &params };
    let out = Registry::standard().get(op).unwrap().run(&mut Ctx::default(), &args).unwrap();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    o.clone()
}

fn labels_of(o: &Objects) -> Vec<u32> {
    o.labels.iter().copied().collect()
}

fn window_run(
    op: &str,
    params: serde_json::Value,
    image: &(Arc<Grid>, Vec<Arc<Vec<f64>>>),
    win: Window,
    step: usize,
    extra: &[(String, Value)],
) -> Arc<Objects> {
    let names = ["a".to_string(), "b".to_string()];
    let whole = Whole::of(&image.1);
    let params = params.as_object().cloned().unwrap();
    segment_window_op(&Registry::standard(), op, &params, &image.0, &image.1, &names, &whole, win, step, extra).unwrap()
}

#[test]
fn every_segmenter_on_the_whole_window_is_its_own_run() {
    let image = fields(48, 36);
    let stack = stack_of(&image.0, &image.1);
    let all = Window { x0: 0, y0: 0, width: 48, height: 36 };
    let board = own_run("segment.chessboard", vec![("image", stack.clone())], serde_json::json!({ "size": 4 }));
    let cases = [
        ("segment.slic", serde_json::json!({ "size": 6, "compactness": 0.5 })),
        ("segment.snic", serde_json::json!({ "size": 6 })),
        ("segment.felzenszwalb", serde_json::json!({ "k": 0.5, "sigma": 1.0, "min_size": 10 })),
        ("segment.watershed", serde_json::json!({ "sigma": 1.0, "compactness": 0.02 })),
        ("segment.quickshift", serde_json::json!({ "kernel_size": 2.0, "max_dist": 6.0 })),
        ("segment.meanshift", serde_json::json!({ "spatial_radius": 3.0, "range_radius": 0.5 })),
        ("segment.threshold", serde_json::json!({ "classes": 3 })),
        ("segment.spectral_difference", serde_json::json!({ "max_difference": 200.0 })),
        ("segment.region_growing", serde_json::json!({ "threshold": 0.05 })),
    ];
    for (op, params) in cases {
        let mut inputs = vec![("image", stack.clone())];
        let mut extra = Vec::new();
        if op == "segment.spectral_difference" {
            inputs.push(("objects", Value::Objects(board.clone())));
            extra.push(("objects".to_string(), Value::Objects(board.clone())));
        }
        let want = own_run(op, inputs, params.clone());
        let got = window_run(op, params, &image, all, 1, &extra);
        assert!(want.count > 1, "{op}: {} objects", want.count);
        assert_eq!(got.count, want.count, "{op}");
        assert_eq!(labels_of(&got), labels_of(&want), "{op}: the window is the image's own run");
    }
}

#[test]
fn a_standardised_window_is_the_whole_images_objects_where_they_are_whole() {
    // (a window of a bigger image, standardised by the whole's mean and sd: the same z-scores the whole run has)
    let image = fields(48, 36);
    let stack = stack_of(&image.0, &image.1);
    let want = own_run("segment.slic", vec![("image", stack)], serde_json::json!({ "size": 6 }));
    let win = Window { x0: 0, y0: 0, width: 24, height: 36 };
    let got = window_run("segment.slic", serde_json::json!({ "size": 6 }), &image, win, 1, &[]);
    // (the grid is the window's, its cells the whole's: the objects are of the window alone, with its own seeds, so
    // only the count is near)
    assert_eq!((got.grid.width, got.grid.height), (24, 36));
    let (a, b) = (got.count as f64, want.count as f64 / 2.0);
    assert!((a - b).abs() / b < 0.35, "{a} objects in half the image, {b} expected");
}

#[test]
fn a_window_read_every_second_cell_scales_the_settings_in_cells() {
    let image = fields(96, 72);
    let all = Window { x0: 0, y0: 0, width: 96, height: 72 };
    let one = window_run("segment.slic", serde_json::json!({ "size": 8 }), &image, all, 1, &[]);
    let two = window_run("segment.slic", serde_json::json!({ "size": 8 }), &image, all, 2, &[]);
    assert_eq!((two.grid.width, two.grid.height), (48, 36));
    let (a, b) = (one.count as f64, two.count as f64);
    assert!((a - b).abs() / a < 0.2, "{a} superpixels read whole, {b} every second cell: the size is in cells");
    for op in ["segment.felzenszwalb", "segment.watershed", "segment.quickshift", "segment.meanshift"] {
        let o = window_run(op, serde_json::json!({}), &image, all, 2, &[]);
        assert!(o.count > 1, "{op}");
    }
}

#[test]
fn the_thresholds_of_a_window_are_the_whole_images() {
    // (the step across the middle splits the image in two classes; a window on the left only still has the one)
    let image = fields(48, 36);
    let win = Window { x0: 0, y0: 0, width: 16, height: 36 };
    let o = window_run("segment.threshold", serde_json::json!({ "classes": 2 }), &image, win, 1, &[]);
    let classes: std::collections::BTreeSet<u64> =
        o.table.rows.iter().map(|r| r[o.table.columns.iter().position(|c| c == "class").unwrap()] as u64).collect();
    assert_eq!(classes.len(), 1, "the window's own Otsu would cut its noise in two: {classes:?}");
}

#[test]
fn inputs_are_cut_to_the_window() {
    let image = fields(48, 36);
    let stack = stack_of(&image.0, &image.1);
    let board = own_run("segment.chessboard", vec![("image", stack)], serde_json::json!({ "size": 8 }));
    let win = Window { x0: 8, y0: 8, width: 16, height: 16 };
    let extra = [("objects".to_string(), Value::Objects(board))];
    let o =
        window_run("segment.spectral_difference", serde_json::json!({ "max_difference": 1.0 }), &image, win, 1, &extra);
    assert_eq!((o.grid.width, o.grid.height), (16, 16));
    // (a difference this small merges no square: four of them in the window)
    assert_eq!(o.count, 4);
    let Err(e) = segment_window_op(
        &Registry::standard(),
        "segment.sam",
        &serde_json::Map::new(),
        &image.0,
        &image.1,
        &[],
        &Whole::of(&image.1),
        win,
        1,
        &[],
    ) else {
        panic!("sam is not run on a window")
    };
    assert!(e.to_string().contains("window"), "{e}");
}

#[test]
fn region_growing_with_the_images_own_ranges_is_the_run_without() {
    let image = fields(40, 30);
    let stack = stack_of(&image.0, &image.1);
    let params = serde_json::json!({ "threshold": 0.05 });
    let want = own_run("segment.region_growing", vec![("image", stack.clone())], params.clone());
    let ranges: Vec<[f64; 2]> = image
        .1
        .iter()
        .map(|b| [b.iter().copied().fold(f64::INFINITY, f64::min), b.iter().copied().fold(f64::NEG_INFINITY, f64::max)])
        .collect();
    let with = own_run(
        "segment.region_growing",
        vec![("image", stack.clone())],
        serde_json::json!({ "threshold": 0.05, "ranges": ranges }),
    );
    assert_eq!(labels_of(&with), labels_of(&want));
    // (wider ranges make the same gaps smaller: fewer objects at the same threshold)
    let wide: Vec<[f64; 2]> = ranges.iter().map(|[lo, hi]| [lo - (hi - lo), hi + (hi - lo)]).collect();
    let wider = own_run(
        "segment.region_growing",
        vec![("image", stack.clone())],
        serde_json::json!({ "threshold": 0.05, "ranges": wide }),
    );
    assert!(wider.count < want.count, "{} then {}", wider.count, want.count);
    // (one pair per band)
    let params = serde_json::json!({ "threshold": 0.05, "ranges": [[0.0, 1.0]] });
    let params = params.as_object().cloned().unwrap();
    let args = Args { node: "n", inputs: [("image".to_string(), stack)].into(), params: &params };
    let e = Registry::standard().get("segment.region_growing").unwrap().run(&mut Ctx::default(), &args).err().unwrap();
    assert!(e.to_string().contains("ranges"), "{e}");
}

/// How long a screen window takes (run with `--ignored --nocapture`, in release for the figure that counts): 800 x 600
/// cells of three noisy bands over smooth fields.
#[test]
#[ignore = "a timing, printed"]
fn a_screen_window_is_segmented_in_under_a_second() {
    let (w, h) = (1200usize, 900usize);
    let mut seed = 11u64;
    let data: Vec<Arc<Vec<f64>>> = (0..3)
        .map(|b| {
            Arc::new(
                (0..w * h)
                    .map(|i| {
                        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                        let (r, c) = ((i / w) as f64, (i % w) as f64);
                        let field = ((r / 37.0 + b as f64).sin() + (c / 53.0).cos()) * 0.2 + 0.5;
                        field + ((seed >> 40) as f64 / (1u64 << 24) as f64 - 0.5) * 0.02
                    })
                    .collect(),
            )
        })
        .collect();
    let grid = Grid::new(w, h);
    let win = Window { x0: 100, y0: 100, width: 800, height: 600 };
    for scale in [0.05, 0.1, 0.3] {
        let set = WindowSettings { scale, ..Default::default() };
        let t = std::time::Instant::now();
        let o = segment_window(&grid, &data, win, 1, &set).unwrap();
        eprintln!("800x600x3, scale {scale}: {} objects in {:.3} s", o.count, t.elapsed().as_secs_f64());
    }
}
