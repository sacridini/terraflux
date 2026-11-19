//! A window of an image segmented on its own (the Viewer's live segmentation): on an image of homogeneous blocks, a
//! window aligned with the blocks gives the objects of the whole image's segmentation restricted to it, also read
//! every second cell; and ESP over the window gives a curve with suggested scales.

use std::sync::Arc;

use terraflux_ops::objects::{Objects, Window, WindowSettings, esp_window, segment_window};
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
