//! Deep learning of objects (ONNX Runtime loaded at run time): the loader's message when the runtime is missing, the
//! plain Rust parts (tiling, stretching, resolving masks, averaging patches) with a fake model, and, when the model
//! files and the runtime are given (`TF_TEST_SAM_ENCODER`, `TF_TEST_SAM_DECODER`, `TF_TEST_EMBED_MODEL`,
//! `TF_TEST_EMBED_PATCH`, `ORT_DYLIB_PATH`), both operators for real (`cargo test -- --ignored`).

use std::cell::Cell;
use std::ffi::OsString;
use std::sync::Arc;

use terraflux_ops::objects::deep::{
    Mask, SAM_SIZE, SamParams, Tile, embed_objects, paint, patch_tensor, pool, resolve, sam_image, sam_image_hwc,
    sam_regions, standardize, stretch_range, suppress, tiles, to_byte, with_embeddings,
};
use terraflux_ops::objects::onnx::{Infer, Ort, Tensor};
use terraflux_ops::objects::{NO_REGION, Objects};
use terraflux_ops::{Args, Ctx, Outputs, Registry, Result, Table, Value};
use terraflux_raster::Grid;

fn run(op: &str, inputs: Vec<(&str, Value)>, params: serde_json::Value) -> Result<Outputs> {
    let r = Registry::standard();
    let params = params.as_object().cloned().unwrap_or_default();
    let args =
        Args { node: "n", inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(), params: &params };
    r.get(op).unwrap().run(&mut Ctx::default(), &args)
}

fn stack(w: usize, h: usize, layers: Vec<Vec<f64>>) -> Value {
    let keys = (0..layers.len()).map(|k| terraflux_ops::Key::Name(format!("b{}", k + 1))).collect();
    let layers = layers.into_iter().map(Arc::new).collect();
    Value::Stack(terraflux_ops::Stack { grid: Arc::new(Grid::new(w, h)), keys, layers })
}

// ---- the loader

#[test]
fn loader_says_how_to_install_when_the_library_is_missing() {
    let missing = OsString::from("C:/nowhere/at/all/onnxruntime.dll");
    let e = match Ort::load_with(Some(&missing)) {
        Ok(_) => panic!("loaded a library that is not there"),
        Err(e) => e.to_string(),
    };
    assert!(e.contains("ORT_DYLIB_PATH"), "{e}");
    assert!(e.contains("pip install onnxruntime"), "{e}");
    assert!(e.contains("works without it"), "{e}");
    assert!(e.contains("nowhere"), "{e}");
}

#[test]
fn loader_refuses_a_file_that_is_no_library() {
    let f = std::env::temp_dir().join("objects_dl_not_a_library.txt");
    std::fs::write(&f, "not a library").unwrap();
    let e = match Ort::load_with(Some(&f.clone().into_os_string())) {
        Ok(_) => panic!("loaded a text file"),
        Err(e) => e.to_string(),
    };
    let _ = std::fs::remove_file(&f);
    assert!(e.contains("pip install onnxruntime"), "{e}");
}

#[test]
fn operators_are_registered_and_ask_for_their_models() {
    let r = Registry::standard();
    assert!(r.get("segment.sam").is_some());
    assert!(r.get("objects.embed").is_some());
    let img = stack(4, 4, vec![vec![1.0; 16], vec![2.0; 16], vec![3.0; 16]]);
    // (one of the two files alone, or a weights setting that is none: refused before anything is loaded)
    let e = run("segment.sam", vec![("image", img.clone())], serde_json::json!({ "encoder": "e.onnx" }))
        .err()
        .unwrap()
        .to_string();
    assert!(e.contains("both encoder and decoder"), "{e}");
    let e =
        run("segment.sam", vec![("image", img)], serde_json::json!({ "weights": "huge" })).err().unwrap().to_string();
    assert!(e.contains("quant") && e.contains("full"), "{e}");
}

// ---- tiling, stretching, the input

#[test]
fn tiles_cover_the_image_and_clip_at_the_edges() {
    let t = tiles(2500, 1100, 1024);
    assert_eq!(t.len(), 6);
    assert_eq!(t[0], Tile { r0: 0, c0: 0, h: 1024, w: 1024 });
    assert_eq!(t[2], Tile { r0: 0, c0: 2048, h: 1024, w: 452 });
    assert_eq!(t[5], Tile { r0: 1024, c0: 2048, h: 76, w: 452 });
    assert_eq!(t.iter().map(|t| t.h * t.w).sum::<usize>(), 2500 * 1100);
    assert!(tiles(0, 0, 10).is_empty());
}

#[test]
fn stretch_takes_the_2nd_and_98th_percentiles() {
    let band: Vec<f64> = (0..=100).map(f64::from).chain([f64::NAN]).collect();
    let (lo, hi) = stretch_range(&band);
    assert_eq!((lo, hi), (2.0, 98.0));
    assert_eq!(to_byte(2.0, (lo, hi)), 0.0);
    assert_eq!(to_byte(98.0, (lo, hi)), 255.0);
    assert_eq!(to_byte(0.0, (lo, hi)), 0.0);
    assert_eq!(to_byte(f64::NAN, (lo, hi)), 0.0);
    // a constant band opens to one unit; no value gives a range too
    assert_eq!(stretch_range(&[5.0, 5.0]), (5.0, 6.0));
    assert_eq!(stretch_range(&[f64::NAN]), (0.0, 1.0));
}

#[test]
fn sam_image_resizes_normalises_and_pads() {
    // a tile of 20 x 10 cells: scaled by 51.2 to 1024 x 512, then 512 rows of zeros
    let (w, h) = (20, 10);
    let rgb: Vec<Vec<f64>> = vec![vec![255.0; w * h], vec![0.0; w * h], vec![255.0; w * h]];
    let t = Tile { r0: 0, c0: 0, h, w };
    let (x, scale) = sam_image([&rgb[0], &rgb[1], &rgb[2]], w, &t, [(0.0, 255.0); 3]);
    assert_eq!(x.shape, vec![1, 3, SAM_SIZE as i64, SAM_SIZE as i64]);
    assert!((scale - 51.2).abs() < 1e-9);
    let at = |b: usize, y: usize, x_: usize| x.data[(b * SAM_SIZE + y) * SAM_SIZE + x_];
    assert!((at(0, 100, 100) - (255.0 - 123.675) / 58.395).abs() < 1e-4);
    assert!((at(1, 100, 100) - (0.0 - 116.28) / 57.12).abs() < 1e-4);
    assert!((at(2, 511, 1023) - (255.0 - 103.53) / 57.375).abs() < 1e-4);
    // the padding (after the normalisation) is 0
    assert_eq!(at(0, 512, 0), 0.0);
    assert_eq!(at(1, 1023, 1023), 0.0);
}

#[test]
fn sam_image_reads_the_window_of_a_wider_image() {
    // the tile starts at column 3, row 2 of an 8 x 6 image whose cell value is its column
    let (w, h) = (8, 6);
    let band: Vec<f64> = (0..w * h).map(|i| (i % w) as f64 * 10.0).collect();
    let t = Tile { r0: 2, c0: 3, h: 2, w: 2 };
    let (x, scale) = sam_image([&band, &band, &band], w, &t, [(0.0, 255.0); 3]);
    assert_eq!(scale, 512.0);
    // first output column is left of the tile's first cell centre: column 3 (value 30)
    let v = x.data[0] * 58.395 + 123.675;
    assert!((v - 30.0).abs() < 1e-3, "{v}");
    // the last one: column 4 (value 40)
    let v = x.data[1023] * 58.395 + 123.675;
    assert!((v - 40.0).abs() < 1e-3, "{v}");
}

// ---- masks

#[test]
fn overlaps_go_to_the_higher_iou_and_the_first_on_a_tie() {
    let m = |iou: f32, cells: &[usize]| {
        let mut b = vec![false; 6];
        cells.iter().for_each(|&c| b[c] = true);
        Mask::from_bools(iou, &b)
    };
    // 1 (0.5) holds 0,1,2; 2 (0.9) holds 2,3; 3 (0.5) holds 1,4
    let masks = [m(0.5, &[0, 1, 2]), m(0.9, &[2, 3]), m(0.5, &[1, 4])];
    assert_eq!(resolve(&masks, 6), vec![1, 1, 2, 2, 3, 0]);
    assert_eq!(masks[0].cells(), 3);
    assert!(masks[1].get(3) && !masks[1].get(4));
    assert!(resolve(&[], 3).iter().all(|&x| x == 0));
}

#[test]
fn smaller_masks_are_painted_over_the_larger() {
    let m = |iou: f32, cells: &[usize]| {
        let mut b = vec![false; 6];
        cells.iter().for_each(|&c| b[c] = true);
        Mask::from_bools(iou, &b)
    };
    // the largest (0..=4) first, then 1..=2, then 2 alone on top, whatever the predicted IoU
    let masks = [m(0.99, &[2]), m(0.5, &[0, 1, 2, 3, 4]), m(0.7, &[1, 2])];
    assert_eq!(paint(&masks, 6), vec![2, 3, 1, 2, 2, 0]);
}

#[test]
fn boxes_that_overlap_are_thinned_by_stability_times_iou() {
    // masks in a tile 10 wide: a box of 4 x 4 cells twice (shifted by one column), and a far one
    let m = |iou: f32, st: f32, r: usize, c: usize| {
        let mut b = vec![false; 100];
        for i in r..r + 4 {
            for j in c..c + 4 {
                b[i * 10 + j] = true;
            }
        }
        Mask::from_bools(iou, &b).with_stability(st)
    };
    let kept = suppress(vec![m(1.0, 0.8, 0, 0), m(0.9, 0.99, 0, 1), m(0.9, 0.9, 6, 6)], 10, 0.7);
    // the boxes' IoU is 12 / 20 = 0.6: under 0.7 both stay ...
    assert_eq!(kept.len(), 3);
    // ... and at a lower bar the worse of the two goes
    let kept = suppress(vec![m(1.0, 0.8, 0, 0), m(0.9, 0.99, 0, 1), m(0.9, 0.9, 6, 6)], 10, 0.5);
    assert_eq!(kept.len(), 2);
    assert!((kept[0].stability - 0.99).abs() < 1e-6);
}

/// A decoder that gives one mask: logits that are +3 in the left third, +0.5 in the next third (inside, but not
/// stable) and -3 elsewhere.
struct Soft;

impl Infer for Soft {
    fn input_names(&self) -> Vec<String> {
        ["image_embeddings", "point_coords", "point_labels", "mask_input", "has_mask_input", "orig_im_size"]
            .map(String::from)
            .to_vec()
    }

    fn output_names(&self) -> Vec<String> {
        ["masks", "iou_predictions"].map(String::from).to_vec()
    }

    fn run(&self, inputs: &[(&str, &Tensor)], _outputs: &[&str]) -> Result<Vec<Tensor>> {
        let size = inputs.iter().find(|(k, _)| *k == "orig_im_size").unwrap().1;
        let (h, w) = (size.data[0] as usize, size.data[1] as usize);
        let m = (0..h * w)
            .map(|i| {
                if i % w < w / 3 {
                    3.0
                } else if i % w < 2 * w / 3 {
                    0.5
                } else {
                    -3.0
                }
            })
            .collect();
        Ok(vec![Tensor::new(vec![1, 1, h as i64, w as i64], m), Tensor::new(vec![1, 1], vec![0.95])])
    }
}

#[test]
fn unstable_and_background_masks_are_dropped() {
    use terraflux_ops::objects::deep::sam_tile;
    let (w, h) = (30, 30);
    let band: Vec<f64> = (0..w * h).map(|i| (i % w) as f64).collect();
    let t = Tile { r0: 0, c0: 0, h, w };
    let run = |p: SamParams| {
        sam_tile(&Encoder, &Soft, [&band, &band, &band], w, &vec![true; w * h], &t, [(0.0, 29.0); 3], &p).unwrap()
    };
    let base = SamParams { points: 1, iou: 0.5, stability: 0.0, max_area: 1.0, ..SamParams::default() };
    // 20 of 30 columns are inside; 10 are surely in: stability 0.5
    let m = run(base);
    assert_eq!(m.len(), 1);
    assert!((m[0].stability - 0.5).abs() < 1e-6, "{}", m[0].stability);
    assert_eq!(m[0].cells(), 600);
    assert!(run(SamParams { stability: 0.6, ..base }).is_empty());
    // a smaller offset (0.4) makes both parts stable: 20 over 20
    let m = run(SamParams { stability: 0.95, offset: 0.4, ..base });
    assert_eq!(m.len(), 1);
    // 600 of 900 cells: dropped when the limit is a half
    assert!(run(SamParams { max_area: 0.5, ..base }).is_empty());
}

/// A decoder that, for a prompt point, gives two masks: the whole tile (IoU `low`) and the half of the tile that
/// holds the point, left or right (IoU `high`).
struct Halves {
    low: f32,
    high: f32,
    calls: Cell<usize>,
    given: Cell<usize>,
}

impl Infer for Halves {
    fn input_names(&self) -> Vec<String> {
        ["image_embeddings", "point_coords", "point_labels", "mask_input", "has_mask_input", "orig_im_size"]
            .map(String::from)
            .to_vec()
    }

    fn output_names(&self) -> Vec<String> {
        ["masks", "iou_predictions", "low_res_masks"].map(String::from).to_vec()
    }

    fn run(&self, inputs: &[(&str, &Tensor)], outputs: &[&str]) -> Result<Vec<Tensor>> {
        assert_eq!(outputs, ["masks", "iou_predictions"]);
        self.calls.set(self.calls.get() + 1);
        self.given.set(inputs.len());
        let get = |n: &str| inputs.iter().find(|(k, _)| *k == n).unwrap().1;
        let size = get("orig_im_size");
        let (h, w) = (size.data[0] as usize, size.data[1] as usize);
        let scale = SAM_SIZE as f32 / w.max(h) as f32;
        let coords = get("point_coords");
        assert_eq!(coords.shape, vec![1, 2, 2]);
        assert_eq!(&coords.data[2..], &[0.0, 0.0]);
        assert_eq!(get("point_labels").data, vec![1.0, -1.0]);
        let x = coords.data[0] / scale;
        let left = x < w as f32 / 2.0;
        let mut masks = vec![10.0f32; h * w];
        masks.extend((0..h * w).map(|i| if (i % w < w / 2) == left { 10.0 } else { -10.0 }));
        Ok(vec![Tensor::new(vec![1, 2, h as i64, w as i64], masks), Tensor::new(vec![1, 2], vec![self.low, self.high])])
    }
}

struct Encoder;

impl Infer for Encoder {
    fn input_names(&self) -> Vec<String> {
        vec!["input_image".into()]
    }

    fn output_names(&self) -> Vec<String> {
        vec!["image_embeddings".into()]
    }

    fn input_rank(&self, _: &str) -> Option<usize> {
        Some(4)
    }

    fn run(&self, inputs: &[(&str, &Tensor)], outputs: &[&str]) -> Result<Vec<Tensor>> {
        assert_eq!(inputs[0].0, "input_image");
        assert_eq!(inputs[0].1.shape, vec![1, 3, 1024, 1024]);
        assert_eq!(outputs, ["image_embeddings"]);
        Ok(vec![Tensor::new(vec![1, 1, 1, 1], vec![0.0])])
    }
}

/// The export that normalises and pads inside (an input of three dimensions): it gets the raw resized tile.
struct EncoderHwc;

impl Infer for EncoderHwc {
    fn input_names(&self) -> Vec<String> {
        vec!["input_image".into()]
    }

    fn output_names(&self) -> Vec<String> {
        vec!["image_embeddings".into()]
    }

    fn input_rank(&self, _: &str) -> Option<usize> {
        Some(3)
    }

    fn run(&self, inputs: &[(&str, &Tensor)], _outputs: &[&str]) -> Result<Vec<Tensor>> {
        // a 40 x 30 tile resized to fit 683 x 1024 (by 22.77), three values of 0-255 a cell
        assert_eq!(inputs[0].1.shape, vec![683, 911, 3]);
        assert!(inputs[0].1.data.iter().all(|v| (-0.001..=255.001).contains(v)));
        Ok(vec![Tensor::new(vec![1, 1, 1, 1], vec![0.0])])
    }
}

#[test]
fn sam_sends_the_resized_hwc_image_to_an_export_that_normalises_inside() {
    let (w, h) = (40, 30);
    let band: Vec<f64> = (0..w * h).map(|i| (i % w) as f64).collect();
    let dec = halves(0.3, 0.9);
    let p = SamParams { points: 2, iou: 0.8, ..SamParams::default() };
    sam_regions(&EncoderHwc, &dec, [&band, &band, &band], (w, h), &p, 1, true).unwrap();
    assert_eq!(dec.calls.get(), 4);
    let (x, scale, size) = sam_image_hwc([&band, &band, &band], w, &Tile { r0: 0, c0: 0, h, w }, [(0.0, 39.0); 3]);
    assert_eq!(x.shape, vec![683, 911, 3]);
    assert!((scale - 683.0 / 30.0).abs() < 1e-9);
    // the frame in the tile's cells: the tile's height, and the width that makes the frame's aspect
    assert_eq!(size, (30, 45));
    // the left edge is the first column's value (0), the right edge the last (255)
    assert_eq!(&x.data[..3], &[0.0; 3]);
    assert!((x.data[910 * 3] - 255.0).abs() < 1e-3);
}

fn sam(dec: &Halves, p: SamParams, min_size: usize, fill: bool) -> (Objects, Vec<u64>) {
    let (w, h) = (40, 30);
    let band: Vec<f64> = (0..w * h).map(|i| (i % w) as f64).collect();
    let region = sam_regions(&Encoder, dec, [&band, &band, &band], (w, h), &p, min_size, fill).unwrap();
    (Objects::from_regions(Arc::new(Grid::new(w, h)), &region, 4), region)
}

fn halves(low: f32, high: f32) -> Halves {
    Halves { low, high, calls: Cell::new(0), given: Cell::new(0) }
}

#[test]
fn sam_keeps_the_best_mask_of_each_point_and_resolves_the_overlaps() {
    let dec = halves(0.3, 0.9);
    let (o, _) = sam(&dec, SamParams { points: 2, iou: 0.8, ..SamParams::default() }, 1, false);
    // 4 prompts, each with the whole tile at 0.3 and a half at 0.9: the best is the half
    assert_eq!(dec.calls.get(), 4);
    assert_eq!(dec.given.get(), 6);
    assert_eq!(o.count, 2);
    assert_eq!(o.cells, vec![20 * 30, 20 * 30]);
    assert_eq!(o.id_at(0, 0), 1);
    assert_eq!(o.id_at(29, 39), 2);
}

#[test]
fn sam_drops_masks_under_the_iou_and_fills_the_rest() {
    let dec = halves(0.3, 0.7);
    let (o, region) = sam(&dec, SamParams { points: 2, iou: 0.8, ..SamParams::default() }, 1, false);
    assert_eq!(o.count, 0);
    assert!(region.iter().all(|&r| r == NO_REGION));
    // with fill the whole tile is leftover: one object
    let (o, _) = sam(&dec, SamParams { points: 2, iou: 0.8, ..SamParams::default() }, 1, true);
    assert_eq!(o.count, 1);
    assert_eq!(o.cells, vec![1200]);
}

#[test]
fn sam_min_size_sends_small_masks_to_the_leftover() {
    let dec = halves(0.3, 0.9);
    // the halves are 600 cells: a minimum above leaves them out, and fill keeps the cells as one object
    let (o, _) = sam(&dec, SamParams { points: 2, iou: 0.8, ..SamParams::default() }, 601, false);
    assert_eq!(o.count, 0);
    let (o, _) = sam(&dec, SamParams { points: 2, iou: 0.8, ..SamParams::default() }, 601, true);
    assert_eq!(o.count, 1);
}

#[test]
fn sam_does_not_prompt_cells_without_a_value() {
    let dec = halves(0.3, 0.9);
    let (w, h) = (40, 30);
    // the left half has no value: the points there are not run, and no mask holds it
    let band: Vec<f64> = (0..w * h).map(|i| if i % w < 20 { f64::NAN } else { 1.0 }).collect();
    let p = SamParams { points: 2, iou: 0.8, ..SamParams::default() };
    let region = sam_regions(&Encoder, &dec, [&band, &band, &band], (w, h), &p, 1, true).unwrap();
    assert_eq!(dec.calls.get(), 2);
    let o = Objects::from_regions(Arc::new(Grid::new(w, h)), &region, 4);
    assert_eq!(o.count, 1);
    assert_eq!(o.id_at(0, 0), 0);
    assert_eq!(o.id_at(0, 39), 1);
    assert_eq!(o.cells, vec![600]);
}

// ---- embeddings

/// A model that gives [mean of the first channel, 1] for a patch, and counts its runs.
struct Mean {
    calls: Cell<usize>,
    tokens: bool,
}

impl Infer for Mean {
    fn input_names(&self) -> Vec<String> {
        vec!["x".into()]
    }

    fn output_names(&self) -> Vec<String> {
        vec!["y".into()]
    }

    fn run(&self, inputs: &[(&str, &Tensor)], _outputs: &[&str]) -> Result<Vec<Tensor>> {
        self.calls.set(self.calls.get() + 1);
        let x = inputs[0].1;
        let p2 = (x.shape[2] * x.shape[3]) as usize;
        let mean = x.data[..p2].iter().sum::<f32>() / p2 as f32;
        Ok(vec![if self.tokens {
            // two tokens whose mean is [mean, 1]
            Tensor::new(vec![1, 2, 2], vec![mean - 1.0, 0.0, mean + 1.0, 2.0])
        } else {
            Tensor::new(vec![1, 2], vec![mean, 1.0])
        }])
    }
}

#[test]
fn patches_are_padded_and_pooled() {
    let band: Vec<f32> = (0..8 * 4).map(|i| i as f32).collect();
    // patch 3 at column 6 of 8: two cells wide, rows 0..3
    let t = patch_tensor(&[band.clone(), band], 8, &Tile { r0: 0, c0: 6, h: 3, w: 2 }, 3);
    assert_eq!(t.shape, vec![1, 2, 3, 3]);
    assert_eq!(&t.data[..9], &[6.0, 7.0, 0.0, 14.0, 15.0, 0.0, 22.0, 23.0, 0.0]);
    assert_eq!(&t.data[9..12], &[6.0, 7.0, 0.0]);
    assert_eq!(pool(&Tensor::new(vec![1, 2], vec![1.0, 2.0])).unwrap(), vec![1.0, 2.0]);
    assert_eq!(pool(&Tensor::new(vec![1, 2, 2], vec![1.0, 2.0, 3.0, 4.0])).unwrap(), vec![2.0, 3.0]);
    assert!(pool(&Tensor::new(vec![1, 0], vec![])).is_err());
}

#[test]
fn standardize_centres_and_zeroes_the_gaps() {
    let s = standardize(&[1.0, 3.0, f64::NAN]);
    assert_eq!(s, vec![-1.0, 1.0, 0.0]);
    assert_eq!(standardize(&[4.0, 4.0]), vec![0.0, 0.0]);
}

#[test]
fn embedding_is_the_mean_of_the_patches_weighted_by_the_cells() {
    // 4 x 2 cells, patches of 2: the left patch is all 1, the right all 3
    let band: Vec<f32> = vec![1.0, 1.0, 3.0, 3.0, 1.0, 1.0, 3.0, 3.0];
    // object 1 holds 3 columns (4 cells in the left patch, 2 in the right), object 2 the last column
    let labels = vec![1, 1, 1, 2, 1, 1, 1, 2];
    for tokens in [false, true] {
        let m = Mean { calls: Cell::new(0), tokens };
        let e = embed_objects(&m, &labels, 2, std::slice::from_ref(&band), (4, 2), 2).unwrap();
        assert_eq!(m.calls.get(), 2);
        assert!((e[0][0] - (4.0 + 6.0) / 6.0).abs() < 1e-6, "{:?}", e[0]);
        assert!((e[0][1] - 1.0).abs() < 1e-6);
        assert!((e[1][0] - 3.0).abs() < 1e-6);
    }
}

#[test]
fn embedding_runs_only_patches_with_objects() {
    let band = vec![1.0f32; 8 * 2];
    // objects in the left patches only (8 x 2 cells, patches of 2)
    let mut labels = vec![0u32; 16];
    labels[0] = 1;
    labels[8] = 1;
    labels[3] = 2;
    let m = Mean { calls: Cell::new(0), tokens: false };
    let e = embed_objects(&m, &labels, 2, &[band], (8, 2), 2).unwrap();
    assert_eq!(m.calls.get(), 2);
    assert_eq!(e.len(), 2);
}

#[test]
fn embeddings_join_the_table_and_replace_old_ones() {
    let grid = Arc::new(Grid::new(2, 1));
    let o = Objects::from_regions(grid, &[0, 1], 4);
    let o = with_embeddings(&o, &[vec![1.0, 2.0], vec![3.0, 4.0]]).unwrap();
    assert_eq!(o.table.columns, vec!["id", "emb_1", "emb_2"]);
    let o = with_embeddings(&o, &[vec![5.0, 6.0], vec![7.0, 8.0]]).unwrap();
    assert_eq!(o.table.columns, vec!["id", "emb_1", "emb_2"]);
    assert_eq!(o.table.rows, vec![vec![1.0, 5.0, 6.0], vec![2.0, 7.0, 8.0]]);
    let _ = Table::new(vec![]);
}

// ---- with a real runtime and real models

/// A scene of textured shapes on a textured ground: the bands and the truth (0 ground, 1.. the shapes).
fn scene(w: usize, h: usize) -> (Vec<Vec<f64>>, Vec<u32>) {
    // (centre row, centre column, radius or half sides, disc?, colour)
    let shapes: [(f64, f64, f64, bool, [f64; 3]); 4] = [
        (80.0, 90.0, 45.0, true, [220.0, 60.0, 50.0]),
        (90.0, 250.0, 40.0, false, [50.0, 80.0, 210.0]),
        (200.0, 100.0, 38.0, false, [240.0, 220.0, 60.0]),
        (190.0, 240.0, 50.0, true, [40.0, 190.0, 80.0]),
    ];
    let mut truth = vec![0u32; w * h];
    let mut bands = vec![vec![0.0; w * h]; 3];
    for r in 0..h {
        for c in 0..w {
            let i = r * w + c;
            let (y, x) = (r as f64, c as f64);
            let k = shapes.iter().position(|&(cy, cx, s, disc, _)| {
                if disc { (y - cy).hypot(x - cx) <= s } else { (y - cy).abs() <= s && (x - cx).abs() <= s }
            });
            // a deterministic texture, a few units of noise and a slow wave
            let tex = ((r * 31 + c * 17) % 13) as f64 - 6.0 + 4.0 * ((x / 9.0).sin() + (y / 7.0).cos());
            let base = match k {
                Some(k) => shapes[k].4,
                None => [90.0, 100.0, 95.0],
            };
            truth[i] = k.map_or(0, |k| k as u32 + 1);
            for b in 0..3 {
                bands[b][i] = (base[b] + tex).clamp(0.0, 255.0);
            }
        }
    }
    (bands, truth)
}

/// Both operators with the real runtime and models: skipped unless the four variables are set.
#[test]
#[ignore = "needs ONNX Runtime and model files: TF_TEST_SAM_ENCODER, TF_TEST_SAM_DECODER, TF_TEST_EMBED_MODEL, ORT_DYLIB_PATH"]
fn real_models_make_objects_and_embeddings() {
    let var = |n: &str| std::env::var(n).ok().filter(|v| !v.is_empty());
    let (Some(enc), Some(dec), Some(emb), Some(_)) =
        (var("TF_TEST_SAM_ENCODER"), var("TF_TEST_SAM_DECODER"), var("TF_TEST_EMBED_MODEL"), var("ORT_DYLIB_PATH"))
    else {
        eprintln!("skipped: the model files and ORT_DYLIB_PATH are not given");
        return;
    };
    // a 4-shape scene, not square (300 x 400 cells: resized by 2.56)
    let (w, h) = (400usize, 300usize);
    let (bands, truth) = scene(w, h);
    let img = stack(w, h, bands.clone());
    // the masks of single points: each shape must have one that matches it (the geometry of the frame, the
    // resize and the prompt points)
    {
        use terraflux_ops::objects::deep::sam_tile;
        let path = |v: &str| std::path::PathBuf::from(v);
        let ort = Ort::load().unwrap();
        let (e, d) = (ort.session(&path(&enc)).unwrap(), ort.session(&path(&dec)).unwrap());
        let ranges = [stretch_range(&bands[0]), stretch_range(&bands[1]), stretch_range(&bands[2])];
        let p = SamParams { points: 12, iou: 0.0, stability: 0.0, max_area: 1.0, nms: 1.0, ..SamParams::default() };
        let masks = sam_tile(
            &e,
            &d,
            [&bands[0], &bands[1], &bands[2]],
            w,
            &vec![true; w * h],
            &Tile { r0: 0, c0: 0, h, w },
            ranges,
            &p,
        )
        .unwrap();
        let mut best = Vec::new();
        for k in 1..=*truth.iter().max().unwrap() {
            let t = truth.iter().filter(|&&v| v == k).count();
            let iou = masks
                .iter()
                .map(|m| {
                    let inter = (0..w * h).filter(|&i| truth[i] == k && m.get(i)).count();
                    inter as f64 / (t + m.cells() - inter) as f64
                })
                .fold(0.0, f64::max);
            best.push(iou);
        }
        eprintln!("segment.sam: {} masks; best mask IoU with the truth per shape: {best:.3?}", masks.len());
        assert!(best.iter().all(|&v| v > 0.9), "best mask IoU per shape: {best:?}");
    }
    let out = run(
        "segment.sam",
        vec![("image", img.clone())],
        serde_json::json!({ "encoder": enc, "decoder": dec, "points": 12 }),
    )
    .unwrap();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    assert!(o.count >= 1);
    // each shape's best IoU with an object
    let shapes = *truth.iter().max().unwrap() as usize;
    let mut ious = Vec::new();
    for k in 1..=shapes as u32 {
        let t = truth.iter().filter(|&&v| v == k).count();
        let mut inter = vec![0usize; o.count + 1];
        for (i, &v) in truth.iter().enumerate() {
            if v == k {
                inter[o.labels[i] as usize] += 1;
            }
        }
        let best = (1..=o.count)
            .map(|id| inter[id] as f64 / (t + o.cells[id - 1] as usize - inter[id]) as f64)
            .fold(0.0, f64::max);
        ious.push(best);
    }
    eprintln!("segment.sam: {} objects; IoU with the truth per shape: {ious:.3?}", o.count);
    // (stability, the background dropped, box NMS and the smaller masks painted last: one object a shape)
    assert!(ious.iter().all(|&v| v >= 0.9), "IoU per shape: {ious:?}");
    // (the side of the patches the model takes: TF_TEST_EMBED_PATCH, 224 by default; the fixture model takes 16)
    let patch: u64 = var("TF_TEST_EMBED_PATCH").and_then(|v| v.parse().ok()).unwrap_or(224);
    let out = run(
        "objects.embed",
        vec![("objects", Value::Objects(o.clone())), ("image", img)],
        serde_json::json!({ "model": emb, "patch": patch }),
    )
    .unwrap();
    let Value::Objects(e) = &out["objects"] else { panic!("objects") };
    assert!(e.table.columns.iter().any(|c| c == "emb_1"));
    assert_eq!(e.table.rows.len(), o.count);
}

/// `objects.embed` with the real runtime and a small Conv + ReLU + pooling model, against numpy's embeddings of the
/// same image and objects (tools/make_embed_reference.py); skipped without ORT_DYLIB_PATH.
#[test]
fn embed_matches_numpy_with_a_real_runtime() {
    if std::env::var_os("ORT_DYLIB_PATH").is_none_or(|v| v.is_empty()) {
        eprintln!("skipped: ORT_DYLIB_PATH is not set");
        return;
    }
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/data/reference");
    let r: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("embed.json")).unwrap()).unwrap();
    let (w, h) = (r["width"].as_u64().unwrap() as usize, r["height"].as_u64().unwrap() as usize);
    let bands: Vec<Vec<f64>> = r["bands"]
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b.as_array().unwrap().iter().map(|v| v.as_f64().unwrap_or(f64::NAN)).collect())
        .collect();
    let labels: Vec<u64> = r["labels"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap()).collect();
    // (the rectangles are numbered by their first cell, as the file's ids are)
    let region: Vec<u64> = labels.iter().map(|&l| if l == 0 { NO_REGION } else { l - 1 }).collect();
    let o = Objects::from_regions(Arc::new(Grid::new(w, h)), &region, 4);
    let model = dir.join("embed_conv.onnx");
    let out = run(
        "objects.embed",
        vec![("objects", Value::Objects(Arc::new(o))), ("image", stack(w, h, bands))],
        serde_json::json!({ "model": model.to_string_lossy(), "patch": r["patch"] }),
    )
    .unwrap();
    let Value::Objects(e) = &out["objects"] else { panic!("objects") };
    let want = r["embeddings"].as_array().unwrap();
    assert_eq!(e.table.rows.len(), want.len());
    for (row, want) in e.table.rows.iter().zip(want) {
        for (j, v) in want.as_array().unwrap().iter().enumerate() {
            let c = e.table.columns.iter().position(|c| *c == format!("emb_{}", j + 1)).unwrap();
            let v = v.as_f64().unwrap();
            assert!((row[c] - v).abs() < 1e-5, "emb_{}: {} against numpy's {v}", j + 1, row[c]);
        }
    }
}

/// The default weights downloaded for real into a temporary cache (75 MB from the network), then reused by a second
/// call without a download; run with `--ignored`.
#[test]
#[ignore = "downloads 75 MB"]
fn quant_weights_download_once_and_reuse() {
    use terraflux_ops::objects::weights::{self, QUANT};
    let cache = std::env::temp_dir().join(format!("tf_sam_cache_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&cache);
    let mut last = (0u64, 0u64);
    let (enc, dec) = weights::ensure(&cache, &QUANT, &mut weights::download, &mut |got, total| {
        last = (got, total.unwrap_or(0));
        true
    })
    .unwrap();
    eprintln!("downloaded {} of {} bytes: {} and {}", last.0, last.1, enc.display(), dec.display());
    assert_eq!(last.0, QUANT.bytes);
    assert!(enc.is_file() && dec.is_file());
    let mut calls = 0;
    let again = weights::ensure(
        &cache,
        &QUANT,
        &mut |_: &weights::Source, _: &std::path::Path, _: weights::Report| {
            calls += 1;
            Ok(())
        },
        &mut |_, _| true,
    )
    .unwrap();
    assert_eq!(again, (enc, dec));
    assert_eq!(calls, 0);
    let _ = std::fs::remove_dir_all(&cache);
}
