//! Deep learning of objects (ONNX Runtime loaded at run time): the loader's message when the runtime is missing, the
//! plain Rust parts (tiling, stretching, resolving masks, averaging patches) with a fake model, and, when the model
//! files and the runtime are given (`TF_TEST_SAM_ENCODER`, `TF_TEST_SAM_DECODER`, `TF_TEST_EMBED_MODEL`,
//! `ORT_DYLIB_PATH`), both operators for real (`cargo test -- --ignored`).

use std::cell::Cell;
use std::ffi::OsString;
use std::sync::Arc;

use terraflux_ops::objects::deep::{
    Mask, SAM_SIZE, SamParams, Tile, embed_objects, patch_tensor, pool, resolve, sam_image, sam_regions, standardize,
    stretch_range, tiles, to_byte, with_embeddings,
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
    let e = run("segment.sam", vec![("image", img)], serde_json::json!({})).err().unwrap().to_string();
    assert!(e.contains("encoder"), "{e}");
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
        let mut masks = vec![1.0f32; h * w];
        masks.extend((0..h * w).map(|i| if (i % w < w / 2) == left { 1.0 } else { -1.0 }));
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

    fn run(&self, inputs: &[(&str, &Tensor)], outputs: &[&str]) -> Result<Vec<Tensor>> {
        assert_eq!(inputs[0].0, "input_image");
        assert_eq!(inputs[0].1.shape, vec![1, 3, 1024, 1024]);
        assert_eq!(outputs, ["image_embeddings"]);
        Ok(vec![Tensor::new(vec![1, 1, 1, 1], vec![0.0])])
    }
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
    let (o, _) = sam(&dec, SamParams { points: 2, iou: 0.8 }, 1, false);
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
    let (o, region) = sam(&dec, SamParams { points: 2, iou: 0.8 }, 1, false);
    assert_eq!(o.count, 0);
    assert!(region.iter().all(|&r| r == NO_REGION));
    // with fill the whole tile is leftover: one object
    let (o, _) = sam(&dec, SamParams { points: 2, iou: 0.8 }, 1, true);
    assert_eq!(o.count, 1);
    assert_eq!(o.cells, vec![1200]);
}

#[test]
fn sam_min_size_sends_small_masks_to_the_leftover() {
    let dec = halves(0.3, 0.9);
    // the halves are 600 cells: a minimum above leaves them out, and fill keeps the cells as one object
    let (o, _) = sam(&dec, SamParams { points: 2, iou: 0.8 }, 601, false);
    assert_eq!(o.count, 0);
    let (o, _) = sam(&dec, SamParams { points: 2, iou: 0.8 }, 601, true);
    assert_eq!(o.count, 1);
}

#[test]
fn sam_does_not_prompt_cells_without_a_value() {
    let dec = halves(0.3, 0.9);
    let (w, h) = (40, 30);
    // the left half has no value: the points there are not run, and no mask holds it
    let band: Vec<f64> = (0..w * h).map(|i| if i % w < 20 { f64::NAN } else { 1.0 }).collect();
    let p = SamParams { points: 2, iou: 0.8 };
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
    // an image of a bright square on a dark ground
    let (w, h) = (256usize, 256usize);
    let band = |v: f64| -> Vec<f64> {
        (0..w * h)
            .map(|i| if (64..192).contains(&(i / w)) && (64..192).contains(&(i % w)) { v } else { v / 4.0 })
            .collect()
    };
    let img = stack(w, h, vec![band(200.0), band(180.0), band(160.0)]);
    let out = run(
        "segment.sam",
        vec![("image", img.clone())],
        serde_json::json!({ "encoder": enc, "decoder": dec, "points": 4 }),
    )
    .unwrap();
    let Value::Objects(o) = &out["objects"] else { panic!("objects") };
    assert!(o.count >= 1);
    let out = run(
        "objects.embed",
        vec![("objects", Value::Objects(o.clone())), ("image", img)],
        serde_json::json!({ "model": emb, "patch": 224 }),
    )
    .unwrap();
    let Value::Objects(e) = &out["objects"] else { panic!("objects") };
    assert!(e.table.columns.iter().any(|c| c == "emb_1"));
    assert_eq!(e.table.rows.len(), o.count);
}
