//! Deep learning for objects, optional (ONNX Runtime, loaded at run time: [`super::onnx`]): `segment.sam` (Segment
//! Anything, automatic masks, simplified) and `objects.embed` (the embedding of a foundation model per object).
//! The pieces that are plain Rust (tiling, stretching, resolving the masks, averaging the patches) take the model
//! as an [`Infer`], so they run with a fake in the tests.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::onnx::{Infer, Ort, Tensor};
use super::{NO_REGION, Objects, band_names, objects_in, stats_table};
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Sam));
    r.add(Arc::new(Embed));
}

/// The side of SAM's input.
pub const SAM_SIZE: usize = 1024;

/// SAM's input normalisation (of values 0-255): mean and standard deviation per band.
const SAM_MEAN: [f32; 3] = [123.675, 116.28, 103.53];
const SAM_STD: [f32; 3] = [58.395, 57.12, 57.375];

/// The region of the leftover cells (no mask) when they are kept as objects.
const LEFT: u64 = u64::MAX - 1;

// ---- tiles and stretching

/// A window of the image: first row and column, and its size (clipped at the image's edges).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tile {
    pub r0: usize,
    pub c0: usize,
    pub h: usize,
    pub w: usize,
}

/// The windows of `size` cells that cover an image, row by row, from its top-left corner; the ones at the right
/// and bottom edges are clipped.
pub fn tiles(w: usize, h: usize, size: usize) -> Vec<Tile> {
    let mut v = Vec::new();
    for r0 in (0..h).step_by(size.max(1)) {
        for c0 in (0..w).step_by(size.max(1)) {
            v.push(Tile { r0, c0, h: size.min(h - r0), w: size.min(w - c0) });
        }
    }
    v
}

/// The 2nd and 98th percentile of a band's values (of a sample of at most a million, evenly taken: the same every
/// time); the same value twice is opened to one unit.
pub fn stretch_range(band: &[f64]) -> (f64, f64) {
    let valid = band.iter().filter(|v| !v.is_nan()).count();
    if valid == 0 {
        return (0.0, 1.0);
    }
    let step = valid.div_ceil(1_000_000);
    let mut s: Vec<f64> = band.iter().copied().filter(|v| !v.is_nan()).step_by(step).collect();
    s.sort_by(f64::total_cmp);
    let at = |q: f64| s[((s.len() - 1) as f64 * q).round() as usize];
    let (lo, hi) = (at(0.02), at(0.98));
    if hi > lo { (lo, hi) } else { (lo, lo + 1.0) }
}

/// A value stretched to 0-255 between the range's ends (no value: 0).
pub fn to_byte(v: f64, (lo, hi): (f64, f64)) -> f32 {
    if v.is_nan() { 0.0 } else { (((v - lo) / (hi - lo)).clamp(0.0, 1.0) * 255.0) as f32 }
}

/// A tile's three bands stretched to 0-255 and resized (bilinear) as large as fits in a `frame` (height, width), the
/// aspect kept: each band's `nw` x `nh` values, `nw`, `nh` and the scale of the resize.
fn resized(
    rgb: [&[f64]; 3],
    width: usize,
    t: &Tile,
    ranges: [(f64, f64); 3],
    frame: (usize, usize),
) -> ([Vec<f32>; 3], usize, usize, f64) {
    let scale = (frame.1 as f64 / t.w as f64).min(frame.0 as f64 / t.h as f64);
    let nw = ((t.w as f64 * scale).round() as usize).clamp(1, frame.1);
    let nh = ((t.h as f64 * scale).round() as usize).clamp(1, frame.0);
    // (the source position of an output pixel's centre, clamped at the tile's edges)
    let axis = |n: usize, len: usize| -> Vec<(usize, usize, f32)> {
        (0..n)
            .map(|i| {
                let x = ((i as f64 + 0.5) * len as f64 / n as f64 - 0.5).clamp(0.0, (len - 1) as f64);
                let i0 = x.floor() as usize;
                (i0, (i0 + 1).min(len - 1), (x - i0 as f64) as f32)
            })
            .collect()
    };
    let (ys, xs) = (axis(nh, t.h), axis(nw, t.w));
    let band = |b: usize| -> Vec<f32> {
        let src: Vec<f32> = (0..t.h)
            .flat_map(|r| (0..t.w).map(move |c| (r, c)))
            .map(|(r, c)| to_byte(rgb[b][(t.r0 + r) * width + t.c0 + c], ranges[b]))
            .collect();
        let mut out = vec![0.0f32; nw * nh];
        for (y, &(y0, y1, fy)) in ys.iter().enumerate() {
            for (x, &(x0, x1, fx)) in xs.iter().enumerate() {
                let a = src[y0 * t.w + x0] * (1.0 - fx) + src[y0 * t.w + x1] * fx;
                let c = src[y1 * t.w + x0] * (1.0 - fx) + src[y1 * t.w + x1] * fx;
                out[y * nw + x] = a * (1.0 - fy) + c * fy;
            }
        }
        out
    };
    ([band(0), band(1), band(2)], nw, nh, scale)
}

/// SAM's input for a tile: the three bands stretched to 0-255, the tile resized (bilinear) so its longer side is
/// 1024, normalised as SAM does and padded with zeros at the right and bottom: a 1 x 3 x 1024 x 1024 tensor, and the
/// scale of the resize.
pub fn sam_image(rgb: [&[f64]; 3], width: usize, t: &Tile, ranges: [(f64, f64); 3]) -> (Tensor, f64) {
    let (bands, nw, nh, scale) = resized(rgb, width, t, ranges, (SAM_SIZE, SAM_SIZE));
    let mut data = vec![0.0f32; 3 * SAM_SIZE * SAM_SIZE];
    for (b, band) in bands.iter().enumerate() {
        let out = &mut data[b * SAM_SIZE * SAM_SIZE..(b + 1) * SAM_SIZE * SAM_SIZE];
        for y in 0..nh {
            for x in 0..nw {
                out[y * SAM_SIZE + x] = (band[y * nw + x] - SAM_MEAN[b]) / SAM_STD[b];
            }
        }
    }
    (Tensor::new(vec![1, 3, SAM_SIZE as i64, SAM_SIZE as i64], data), scale)
}

/// The frame (height, width) of the exports that normalise and pad inside the graph (`input_image`): the ones made
/// by `samexporter` were traced with an image of 1500 x 2250, which fixes the decoder's crop of the mask to the top
/// 683 x 1024 of the 1024 x 1024 frame whatever `orig_im_size` is (a tile of another aspect gets masks stretched
/// and shifted in height, as measured). So a tile is resized to fit that frame and the decoder told the size of the
/// frame in the tile's cells.
pub const HWC_FRAME: (usize, usize) = (683, 1024);

/// SAM's input for the exports that normalise and pad inside the graph (`input_image`): the tile resized to fit
/// [`HWC_FRAME`], as height x width x 3 values of 0-255 (neither normalised nor padded), the scale and the size
/// (height, width) to give the decoder as `orig_im_size`: the tile's cells, plus the margin that makes its aspect the
/// frame's (the masks come as large as that, and are cut to the tile).
pub fn sam_image_hwc(
    rgb: [&[f64]; 3],
    width: usize,
    t: &Tile,
    ranges: [(f64, f64); 3],
) -> (Tensor, f64, (usize, usize)) {
    let (bands, nw, nh, scale) = resized(rgb, width, t, ranges, HWC_FRAME);
    let data = (0..nw * nh).flat_map(|i| [bands[0][i], bands[1][i], bands[2][i]]).collect();
    let size = ((HWC_FRAME.0 as f64 / scale).round() as usize, (HWC_FRAME.1 as f64 / scale).round() as usize);
    (Tensor::new(vec![nh as i64, nw as i64, 3], data), scale, size)
}

// ---- masks

/// A mask of a tile's cells (a bit each) and the model's predicted IoU of it.
#[derive(Debug, Clone, PartialEq)]
pub struct Mask {
    pub iou: f32,
    n: usize,
    bits: Vec<u64>,
}

impl Mask {
    pub fn from_bools(iou: f32, cells: &[bool]) -> Mask {
        let mut bits = vec![0u64; cells.len().div_ceil(64)];
        for (i, &c) in cells.iter().enumerate() {
            if c {
                bits[i / 64] |= 1 << (i % 64);
            }
        }
        Mask { iou, n: cells.len(), bits }
    }

    pub fn get(&self, i: usize) -> bool {
        i < self.n && self.bits[i / 64] >> (i % 64) & 1 == 1
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn cells(&self) -> usize {
        self.bits.iter().map(|w| w.count_ones() as usize).sum()
    }
}

/// The overlaps of masks resolved cell by cell: the masks by higher IoU first (equal ones, the earlier first), the
/// first to hold a cell keeps it. Each cell's mask (its index in `masks` plus 1), 0 for none.
pub fn resolve(masks: &[Mask], n: usize) -> Vec<u32> {
    let mut order: Vec<usize> = (0..masks.len()).collect();
    order.sort_by(|&a, &b| masks[b].iou.total_cmp(&masks[a].iou));
    let mut out = vec![0u32; n];
    for k in order {
        for (wi, &word) in masks[k].bits.iter().enumerate() {
            let mut w = word;
            while w != 0 {
                let i = wi * 64 + w.trailing_zeros() as usize;
                w &= w - 1;
                if i < n && out[i] == 0 {
                    out[i] = k as u32 + 1;
                }
            }
        }
    }
    out
}

/// The name among a model's `names` that is `want`, else the one at `at`.
fn pick(names: &[String], want: &str, at: usize) -> Result<String> {
    names
        .iter()
        .find(|n| n.as_str() == want)
        .or_else(|| names.get(at))
        .cloned()
        .ok_or_else(|| Error::msg(format!("the model has no {want} (it has {})", names.join(", "))))
}

/// What a SAM run takes.
#[derive(Debug, Clone, Copy)]
pub struct SamParams {
    /// The prompt points per side of the grid.
    pub points: usize,
    /// The least predicted IoU kept.
    pub iou: f32,
}

/// The masks of a tile: the encoder run once, then the decoder on each point of a grid (a cell without a value is
/// not prompted); of the masks the decoder gives for a point the one of the best predicted IoU is kept when that is
/// at least `iou`. A mask holds only cells with a value. Decoder inputs: `image_embeddings`, `point_coords`
/// (1 x N x 2, in the resized 1024 frame, the point and a padding point 0,0), `point_labels` (1 x N: 1 and -1),
/// `mask_input` (zeros 1 x 1 x 256 x 256), `has_mask_input` (0) and `orig_im_size` (height, width of the tile, or of the frame in its cells for the HWC export);
/// outputs `masks` (1 x K x height x width, logits: above 0 is in) and `iou_predictions` (1 x K).
#[allow(clippy::too_many_arguments)]
pub fn sam_tile(
    enc: &dyn Infer,
    dec: &dyn Infer,
    rgb: [&[f64]; 3],
    width: usize,
    valid: &[bool],
    t: &Tile,
    ranges: [(f64, f64); 3],
    p: &SamParams,
) -> Result<Vec<Mask>> {
    let (e_in, e_out) =
        (pick(&enc.input_names(), "input_image", 0)?, pick(&enc.output_names(), "image_embeddings", 0)?);
    // (an input of three dimensions, or named input_image when the model does not say, is the export that takes
    // the raw HWC image)
    let hwc = enc.input_rank(&e_in).map_or(e_in == "input_image", |r| r == 3);
    let (image, scale, osize) = if hwc {
        sam_image_hwc(rgb, width, t, ranges)
    } else {
        let (image, scale) = sam_image(rgb, width, t, ranges);
        (image, scale, (t.h, t.w))
    };
    let emb = enc.run(&[(&e_in, &image)], &[&e_out])?.pop().ok_or_else(|| Error::msg("the encoder gave nothing"))?;
    drop(image);
    let d_in = dec.input_names();
    let d_out = [pick(&dec.output_names(), "masks", 0)?, pick(&dec.output_names(), "iou_predictions", 1)?];
    let mask_input = Tensor::new(vec![1, 1, 256, 256], vec![0.0; 256 * 256]);
    let has_mask = Tensor::new(vec![1], vec![0.0]);
    let size = Tensor::new(vec![2], vec![osize.0 as f32, osize.1 as f32]);
    let labels = Tensor::new(vec![1, 2], vec![1.0, -1.0]);
    let n = p.points.max(1);
    let mut masks = Vec::new();
    for j in 0..n {
        for i in 0..n {
            // the point in the tile's cells (the cell it falls in must have a value)
            let (x, y) = ((i as f64 + 0.5) / n as f64 * t.w as f64, (j as f64 + 0.5) / n as f64 * t.h as f64);
            let (c, r) = ((x as usize).min(t.w - 1), (y as usize).min(t.h - 1));
            if !valid[(t.r0 + r) * width + t.c0 + c] {
                continue;
            }
            let coords = Tensor::new(vec![1, 2, 2], vec![(x * scale) as f32, (y * scale) as f32, 0.0, 0.0]);
            let all: [(&str, &Tensor); 6] = [
                ("image_embeddings", &emb),
                ("point_coords", &coords),
                ("point_labels", &labels),
                ("mask_input", &mask_input),
                ("has_mask_input", &has_mask),
                ("orig_im_size", &size),
            ];
            // (only what the export takes)
            let given: Vec<(&str, &Tensor)> =
                all.iter().copied().filter(|(k, _)| d_in.iter().any(|n| n == k)).collect();
            let out = dec.run(&given, &[&d_out[0], &d_out[1]])?;
            let [m, q] = <[Tensor; 2]>::try_from(out).map_err(|_| Error::msg("the decoder gave fewer outputs"))?;
            // (the masks are as large as the size given, at least the tile)
            let k = q.data.len();
            let (mh, mw) = match m.shape[..] {
                [_, _, h, w] if h as usize >= t.h && w as usize >= t.w => (h as usize, w as usize),
                _ => (0, 0),
            };
            if k == 0 || mh == 0 || m.data.len() != k * mh * mw {
                return Err(Error::msg(format!(
                    "the decoder's masks hold {} values for {k} masks of {} x {} cells (shape {:?})",
                    m.data.len(),
                    osize.0,
                    osize.1,
                    m.shape
                )));
            }
            let best = (0..k).max_by(|&a, &b| q.data[a].total_cmp(&q.data[b])).unwrap();
            if q.data[best] < p.iou {
                continue;
            }
            let logits = &m.data[best * mh * mw..(best + 1) * mh * mw];
            let inside: Vec<bool> = (0..t.h * t.w)
                .map(|q| {
                    let (r, c) = (q / t.w, q % t.w);
                    logits[r * mw + c] > 0.0 && valid[(t.r0 + r) * width + t.c0 + c]
                })
                .collect();
            masks.push(Mask::from_bools(q.data[best], &inside));
        }
    }
    Ok(masks)
}

/// The regions of a whole image by SAM: tiles of 1024, each tile's masks resolved by IoU, one region per mask (the
/// ones of different tiles are different); the cells no mask holds are the region [`LEFT`] when `fill`, else none.
/// Masks of fewer than `min_size` cells are left out.
pub fn sam_regions(
    enc: &dyn Infer,
    dec: &dyn Infer,
    rgb: [&[f64]; 3],
    (width, height): (usize, usize),
    p: &SamParams,
    min_size: usize,
    fill: bool,
) -> Result<Vec<u64>> {
    let valid: Vec<bool> = (0..width * height).map(|i| rgb.iter().any(|b| !b[i].is_nan())).collect();
    let ranges = [stretch_range(rgb[0]), stretch_range(rgb[1]), stretch_range(rgb[2])];
    let mut region = vec![NO_REGION; width * height];
    let mut next = 0u64;
    for t in tiles(width, height, SAM_SIZE) {
        let masks = sam_tile(enc, dec, rgb, width, &valid, &t, ranges, p)?;
        let owner = resolve(&masks, t.h * t.w);
        // each mask's cells left after the overlaps, kept when they are enough
        let mut left = vec![0usize; masks.len()];
        owner.iter().filter(|&&o| o != 0).for_each(|&o| left[o as usize - 1] += 1);
        for (q, &o) in owner.iter().enumerate() {
            let i = (t.r0 + q / t.w) * width + t.c0 + q % t.w;
            if !valid[i] {
                continue;
            }
            let o = o as usize;
            region[i] = if o != 0 && left[o - 1] >= min_size.max(1) {
                next + o as u64 - 1
            } else if fill {
                LEFT
            } else {
                NO_REGION
            };
        }
        next += masks.len() as u64;
    }
    Ok(region)
}

// ---- the layers of the image

/// The names and values of the layers taken.
type Chosen = (Vec<String>, Vec<Arc<Vec<f64>>>);

/// The layers an image holds, or the ones named in `bands` (the names, case apart).
fn chosen(
    node: &str,
    l: &crate::raster_stats::Layers,
    bands: Option<Vec<String>>,
    default: Option<usize>,
) -> Result<Chosen> {
    let names = band_names(&l.names);
    let at: Vec<usize> = match bands {
        Some(b) => b
            .iter()
            .map(|n| {
                names
                    .iter()
                    .position(|x| x == n)
                    .or_else(|| names.iter().position(|x| x.eq_ignore_ascii_case(n)))
                    .ok_or_else(|| {
                        Error::msg(format!("node {node:?}: bands {n:?} is none of the layers: {}", names.join(", ")))
                    })
            })
            .collect::<Result<_>>()?,
        None => (0..default.unwrap_or(names.len()).min(names.len())).collect(),
    };
    Ok((at.iter().map(|&k| names[k].clone()).collect(), at.iter().map(|&k| l.data[k].clone()).collect()))
}

// ---- segment.sam

/// `segment.sam`.
struct Sam;

impl Operator for Sam {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "segment.sam",
            "Objects by Segment Anything (SAM), automatic masks, simplified, run by ONNX Runtime (optional: it is \
             loaded when this operator runs; if it is not found the error says how to install it and to set \
             ORT_DYLIB_PATH, and every other operator works without it). With no `encoder` and `decoder` the weights              download on first use: SAM ViT-B as exported to ONNX by vietanhdev (Apache-2.0,              huggingface.co/vietanhdev/segment-anything-onnx-models), the quantised one of 75 MB by default or, with              `weights` full, the full-precision one of 348 MB, to Terraflux's cache folder (the SHA-256 checked;              later runs reuse the files; TERRAFLUX_CACHE moves the folder). Or give the ONNX files of SAM's image              encoder and mask decoder as the common SAM ONNX export makes them. The encoder takes a 1 x 3 x 1024 x              1024 float image (after SAM's normalisation) or, for the exports that do that inside (input_image), the              tile as height x width x 3 values of 0-255, and gives image_embeddings 1 x 256 x 64 x 64; the decoder takes image_embeddings, \
             point_coords, point_labels, mask_input, has_mask_input and orig_im_size and gives masks and \
             iou_predictions. The image is cut into tiles of 1024 x 1024 cells (clipped at the edges, resized so \
             the longer side is 1024 and padded), the three bands stretched to 0-255 between their 2nd and 98th \
             percentiles; a grid of `points` x `points` prompt points is run on each tile (a cell without a value is \
             not prompted), the best mask of a point is kept when its predicted IoU is at least `iou`, and the \
             overlaps are resolved by the higher IoU first, cell by cell (the first kept wins). With `fill` the \
             cells that no mask holds become objects too (each connected area). The masks of a tile are cut at its \
             edge, so an object spanning tiles is one object for each. Each mask's cells that are not connected \
             are different objects.",
            Access::Global,
        )
        .input(PortSpec::new("image", Type::Any, "a stack of bands holding the three colours (red, green, blue)"))
        .output(PortSpec::new("objects", Type::Objects, "the objects"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per object: id, cells, area (map units squared), mean_<band> and sd_<band> of the three bands",
        ))
        .param(PortSpec::optional(
            "encoder",
            Type::Text,
            "the path of SAM's image encoder (an ONNX file); with no encoder and decoder the weights are downloaded",
        ))
        .param(PortSpec::optional("decoder", Type::Text, "the path of SAM's mask decoder (an ONNX file)"))
        .param(PortSpec::optional(
            "weights",
            Type::Text,
            "the weights downloaded when no encoder and decoder are given: quant (75 MB, default) or full (348 MB)",
        ))
        .param(PortSpec::optional(
            "bands",
            Type::Any,
            "three layer names for red, green and blue (default: the first three)",
        ))
        .param(PortSpec::optional("points", Type::Integer, "prompt points per side of the grid, 1 to 64 (default 16)"))
        .param(PortSpec::optional("iou", Type::Number, "the least predicted IoU of a mask kept, 0 to 1 (default 0.8)"))
        .param(PortSpec::optional(
            "min_size",
            Type::Integer,
            "the fewest cells of a mask (after the overlaps) kept (default 1)",
        ))
        .param(PortSpec::optional(
            "fill",
            Type::Bool,
            "make objects of the cells no mask holds (default true); else they are no object",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let text =
            |name: &str| -> Result<Option<String>> { Ok(args.opt_param::<String>(name)?.filter(|s| !s.is_empty())) };
        let (encoder, decoder) = (text("encoder")?, text("decoder")?);
        if encoder.is_some() != decoder.is_some() {
            return Err(bad("give both encoder and decoder, or neither (the weights are then downloaded)".into()));
        }
        let weights = text("weights")?.unwrap_or_else(|| "quant".into());
        let src = super::weights::source(&weights).map_err(|e| bad(e.to_string()))?;
        let points: i64 = args.param("points", 16)?;
        if !(1..=64).contains(&points) {
            return Err(bad(format!("points {points}: from 1 to 64")));
        }
        let iou: f64 = args.param("iou", 0.8)?;
        if !(0.0..=1.0).contains(&iou) {
            return Err(bad(format!("iou {iou}: from 0 to 1")));
        }
        let min_size: i64 = args.param("min_size", 1)?;
        if min_size < 1 {
            return Err(bad(format!("min_size {min_size}: 1 cell or more")));
        }
        let fill: bool = args.param("fill", true)?;
        let l = crate::raster_stats::layers(args, "image")?;
        let (names, data) = chosen(node, &l, args.opt_param::<Vec<String>>("bands")?, Some(3))?;
        if data.len() != 3 {
            return Err(bad(format!("an image of three bands (red, green, blue) is needed: {} given", data.len())));
        }
        let ort = Ort::load().map_err(|e| bad(e.to_string()))?;
        let (encoder, decoder) = match (encoder, decoder) {
            (Some(e), Some(d)) => (e, d),
            _ => {
                // (the weights come after the runtime: no download when it cannot run)
                let name = src.name;
                let mut report = |got: u64, total: Option<u64>| {
                    ctx.progress(
                        total.map(|t| got as f64 / t.max(1) as f64),
                        format!(
                            "downloading the SAM weights ({name}): {} of {} MB",
                            got >> 20,
                            total.unwrap_or(0) >> 20
                        ),
                    )
                };
                let (e, d) = super::weights::ensure(
                    &crate::sdm::refdata::cache_dir(),
                    src,
                    &mut super::weights::download,
                    &mut report,
                )
                .map_err(|e| bad(e.to_string()))?;
                (e.to_string_lossy().into_owned(), d.to_string_lossy().into_owned())
            }
        };
        let enc = super::onnx::open(&ort, node, "encoder", &encoder)?;
        let dec = super::onnx::open(&ort, node, "decoder", &decoder)?;
        let (w, h) = (l.grid.width, l.grid.height);
        let p = SamParams { points: points as usize, iou: iou as f32 };
        let rgb = [data[0].as_slice(), data[1].as_slice(), data[2].as_slice()];
        let region =
            sam_regions(&enc, &dec, rgb, (w, h), &p, min_size as usize, fill).map_err(|e| bad(e.to_string()))?;
        let o = Objects::from_regions(l.grid.clone(), &region, 4);
        let t = stats_table(&o, &names, &data);
        let o = o.with_table(t.clone())?;
        Ok(outputs([("objects", Value::Objects(Arc::new(o))), ("table", Value::Table(t))]))
    }
}

// ---- objects.embed

/// A band standardised (mean 0, standard deviation 1 over its cells with a value; a constant band is centred), a
/// cell without a value 0; as float32.
pub fn standardize(band: &[f64]) -> Vec<f32> {
    let (s, n) = band.iter().filter(|v| !v.is_nan()).fold((0.0, 0usize), |(s, n), v| (s + v, n + 1));
    let mean = if n > 0 { s / n as f64 } else { 0.0 };
    let var = band.iter().filter(|v| !v.is_nan()).map(|v| (v - mean) * (v - mean)).sum::<f64>() / n.max(1) as f64;
    let sd = if var > 0.0 { var.sqrt() } else { 1.0 };
    band.iter().map(|&v| if v.is_nan() { 0.0 } else { ((v - mean) / sd) as f32 }).collect()
}

/// A band as float32, a cell without a value 0 (no standardising).
pub fn plain(band: &[f64]) -> Vec<f32> {
    band.iter().map(|&v| if v.is_nan() { 0.0 } else { v as f32 }).collect()
}

/// The patch at a window: a 1 x C x P x P tensor, the cells beyond the image's edge 0.
pub fn patch_tensor(bands: &[Vec<f32>], width: usize, t: &Tile, p: usize) -> Tensor {
    let mut data = vec![0.0f32; bands.len() * p * p];
    for (b, band) in bands.iter().enumerate() {
        for r in 0..t.h {
            let src = (t.r0 + r) * width + t.c0;
            data[b * p * p + r * p..b * p * p + r * p + t.w].copy_from_slice(&band[src..src + t.w]);
        }
    }
    Tensor::new(vec![1, bands.len() as i64, p as i64, p as i64], data)
}

/// A model's output as one vector: 1 x D as it is, 1 x T x D (or any shape ending in D) averaged over the others.
pub fn pool(t: &Tensor) -> Result<Vec<f64>> {
    let d = *t.shape.last().ok_or_else(|| Error::msg("the model's output has no shape"))? as usize;
    if d == 0 || t.data.is_empty() || !t.data.len().is_multiple_of(d) {
        return Err(Error::msg(format!("the model's output of shape {:?} cannot be one vector", t.shape)));
    }
    let rows = t.data.len() / d;
    let mut out = vec![0.0f64; d];
    for row in t.data.chunks(d) {
        out.iter_mut().zip(row).for_each(|(o, &v)| *o += v as f64);
    }
    out.iter_mut().for_each(|o| *o /= rows as f64);
    Ok(out)
}

/// The embedding of each object: the image cut into patches of `patch` x `patch` cells (clipped at the edges and
/// padded with 0), each patch with an object's cell run once; an object's embedding is the mean of the patches its
/// cells fall in, weighted by the number of its cells in each. A row per object (id order), D values.
pub fn embed_objects(
    model: &dyn Infer,
    labels: &[u32],
    count: usize,
    bands: &[Vec<f32>],
    (width, height): (usize, usize),
    patch: usize,
) -> Result<Vec<Vec<f64>>> {
    let input = model.input_names().into_iter().next().ok_or_else(|| Error::msg("the model has no input"))?;
    let output = model.output_names().into_iter().next().ok_or_else(|| Error::msg("the model has no output"))?;
    let mut acc: Vec<Vec<f64>> = vec![Vec::new(); count];
    let mut weight = vec![0.0f64; count];
    let mut dim = 0usize;
    for t in tiles(width, height, patch) {
        // the cells of each object in the patch
        let mut here: Vec<(u32, u32)> = Vec::new();
        for r in 0..t.h {
            for &l in &labels[(t.r0 + r) * width + t.c0..(t.r0 + r) * width + t.c0 + t.w] {
                if l != 0 {
                    here.push((l, 1));
                }
            }
        }
        if here.is_empty() {
            continue;
        }
        here.sort_unstable();
        here.dedup_by(|b, a| {
            let same = a.0 == b.0;
            if same {
                a.1 += b.1;
            }
            same
        });
        let x = patch_tensor(bands, width, &t, patch);
        let out = model.run(&[(&input, &x)], &[&output])?.pop().ok_or_else(|| Error::msg("the model gave nothing"))?;
        let e = pool(&out)?;
        if dim == 0 {
            dim = e.len();
        } else if dim != e.len() {
            return Err(Error::msg(format!("the model gave vectors of {} and of {dim} values", e.len())));
        }
        for (l, n) in here {
            let k = l as usize - 1;
            if acc[k].is_empty() {
                acc[k] = vec![0.0; dim];
            }
            acc[k].iter_mut().zip(&e).for_each(|(a, v)| *a += v * n as f64);
            weight[k] += n as f64;
        }
    }
    Ok((0..count)
        .map(|k| if weight[k] > 0.0 { acc[k].iter().map(|a| a / weight[k]).collect() } else { vec![f64::NAN; dim] })
        .collect())
}

/// `objects.embed`.
struct Embed;

impl Operator for Embed {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.embed",
            "The embedding of each object by a foundation model of Earth observation (Clay, Prithvi or alike), run \
             by ONNX Runtime (optional: it is loaded when this operator runs; if it is not found the error says how \
             to install it and to set ORT_DYLIB_PATH, and every other operator works without it). `model` is the \
             ONNX file of the model's encoder: it takes a 1 x C x P x P float image (the `bands`, standardised per \
             band over the image unless `normalize` is false, a cell without a value 0) and gives 1 x D, or 1 x T x \
             D tokens (any shape ending in D) that are averaged over the tokens. The image is cut into patches of \
             P x P cells (clipped at the edges and padded with 0), each patch holding an object's cells run once; an \
             object's embedding is the mean of the patches its cells fall in, weighted by its number of cells in \
             each. The embeddings join the objects' table as emb_1 to emb_D (a column of an old name is replaced), \
             to classify or cluster the objects by what the model sees. No weights come with Terraflux: export the \
             model's encoder to ONNX with the model's own tools.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects"))
        .input(PortSpec::new("image", Type::Any, "a map, or a stack of bands, on the objects' grid"))
        .output(PortSpec::new("objects", Type::Objects, "the same objects, with emb_1.. in their table"))
        .output(PortSpec::new("table", Type::Table, "the objects' table"))
        .param(PortSpec::new("model", Type::Text, "the path of the model's encoder (an ONNX file)"))
        .param(PortSpec::optional("patch", Type::Integer, "the side P of a patch in cells, 1 or more (default 224)"))
        .param(PortSpec::optional("bands", Type::Any, "the layers the model reads, a list of names (default: all)"))
        .param(PortSpec::optional(
            "normalize",
            Type::Bool,
            "standardise each band over the image (default true); false gives the values as they are",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let model = args
            .opt_param::<String>("model")?
            .filter(|s| !s.is_empty())
            .ok_or_else(|| bad("the setting model is needed".into()))?;
        let patch: i64 = args.param("patch", 224)?;
        if patch < 1 {
            return Err(bad(format!("patch {patch}: 1 cell or more")));
        }
        let normalize: bool = args.param("normalize", true)?;
        let l = crate::raster_stats::layers(args, "image")?;
        if !l.grid.same_as(&o.grid) {
            return Err(bad(format!(
                "the image is on a {}x{} grid, the objects' is {}x{}: put it on the objects' grid with raster.match",
                l.grid.height, l.grid.width, o.grid.height, o.grid.width
            )));
        }
        let (_, data) = chosen(node, &l, args.opt_param::<Vec<String>>("bands")?, None)?;
        if data.is_empty() {
            return Err(bad("bands: no layer given".into()));
        }
        let ort = Ort::load().map_err(|e| bad(e.to_string()))?;
        let session = super::onnx::open(&ort, node, "model", &model)?;
        let bands: Vec<Vec<f32>> = data.iter().map(|b| if normalize { standardize(b) } else { plain(b) }).collect();
        let size = (o.grid.width, o.grid.height);
        let emb = embed_objects(&session, &o.labels, o.count, &bands, size, patch as usize)
            .map_err(|e| bad(e.to_string()))?;
        let out = with_embeddings(o, &emb)?;
        let t = out.table.clone();
        Ok(outputs([("objects", Value::Objects(Arc::new(out))), ("table", Value::Table(t))]))
    }
}

/// The objects with the embeddings as columns emb_1..emb_D of their table (an old column of the name replaced).
pub fn with_embeddings(o: &Objects, emb: &[Vec<f64>]) -> Result<Objects> {
    let d = emb.first().map_or(0, Vec::len);
    let mut t = o.table.clone();
    for j in 0..d {
        let name = format!("emb_{}", j + 1);
        match t.columns.iter().position(|c| *c == name) {
            Some(c) => {
                t.labels.remove(&name);
                t.rows.iter_mut().zip(emb).for_each(|(r, e)| r[c] = e[j]);
            }
            None => {
                t.columns.push(name);
                t.rows.iter_mut().zip(emb).for_each(|(r, e)| r.push(e[j]));
            }
        }
    }
    Objects::clone(o).with_table(t)
}
