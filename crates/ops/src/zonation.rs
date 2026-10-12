//! Zonation 5's transforms of the features before the ranking and its analyses after it, as landschaft does them
//! around its C++ (prioritize/_connectivity.py, _zonation.py): conditions, retention, the connectivity transforms
//! (a dispersal kernel, interactions between features, connectivity matrices; FFT convolution in blocks as
//! Zonation's convolver), the pre-processing maps and the features as ranked.

use std::collections::BTreeMap;

use rayon::prelude::*;
use serde::Deserialize;
use terraflux_ls_sys::ffi::{self, PrioData};

/// Zonation's Z5_FILTER_MIN_SIZE: the least side of a kernel's image.
const MIN_FILTER: usize = 128;

/// A dispersal kernel of Zonation's connectivity transforms: uniform (1 within `distance` cells), triangular
/// (1 - d / distance within it), negexp (exp(-2 d / distance), 0 beyond distance * cut) or gaussian (the normal
/// density of deviation `distance`, 0 beyond distance * cut).
#[derive(Debug, Clone, Deserialize)]
pub struct Kernel {
    pub shape: String,
    pub distance: f64,
    #[serde(default = "one")]
    pub cut: f64,
}

fn one() -> f64 {
    1.0
}

fn yes() -> bool {
    true
}

impl Kernel {
    fn check(&self) -> Result<(), String> {
        if !["uniform", "triangular", "negexp", "gaussian"].contains(&self.shape.as_str()) {
            return Err(format!("unknown kernel {:?}: uniform, triangular, negexp or gaussian", self.shape));
        }
        if self.distance.is_nan() || self.distance <= 0.0 {
            return Err("the kernel distance must be above 0".into());
        }
        Ok(())
    }

    /// The side of the kernel's image (getKernel's getFilterSize).
    pub fn size(&self, max_distance: usize) -> usize {
        let mut d = f64::from(self.distance as f32);
        if self.shape == "negexp" || self.shape == "gaussian" {
            d = f64::from((self.distance as f32) * (self.cut as f32));
        }
        let d = (d as i64).min(max_distance as i64).max(0) as f64;
        MIN_FILTER.max(2 * 2usize.pow((d + 1.0).log2().ceil() as u32))
    }

    /// The kernel's image (not normalized), row-major.
    pub fn image(&self, max_distance: usize) -> (usize, Vec<f64>) {
        let n = self.size(max_distance);
        let d = f64::from(self.distance as f32);
        let c = f64::from((self.distance as f32) * (self.cut as f32));
        let s2 = d * d;
        let mut out = vec![0.0; n * n];
        for y in 0..n {
            let oy = y as f64 - n as f64 * 0.5;
            for x in 0..n {
                let ox = x as f64 - n as f64 * 0.5;
                let dist = (oy * oy + ox * ox).sqrt();
                out[y * n + x] = match self.shape.as_str() {
                    "uniform" => f64::from(u8::from(dist <= d)),
                    "triangular" if dist <= d => 1.0 - dist / d,
                    "negexp" if dist <= c => (-2.0 * dist / d).exp(),
                    "gaussian" if dist <= c => (-(dist * dist / (2.0 * s2))).exp() / (2.0 * std::f64::consts::PI * s2),
                    _ => 0.0,
                };
            }
        }
        (n, out)
    }
}

/// In-place radix-2 FFT of a power-of-2 number of complex values (unnormalised; e^{+i} when `inverse`).
fn fft(re: &mut [f64], im: &mut [f64], inverse: bool) {
    let n = re.len();
    let mut j = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j |= bit;
        if i < j {
            re.swap(i, j);
            im.swap(i, j);
        }
    }
    let sign = if inverse { 1.0 } else { -1.0 };
    let mut len = 2;
    while len <= n {
        let ang = sign * 2.0 * std::f64::consts::PI / len as f64;
        let half = len / 2;
        let tw: Vec<(f64, f64)> = (0..half).map(|k| ((ang * k as f64).cos(), (ang * k as f64).sin())).collect();
        for start in (0..n).step_by(len) {
            for (k, &(wr, wi)) in tw.iter().enumerate() {
                let (a, b) = (start + k, start + k + half);
                let (xr, xi) = (re[b] * wr - im[b] * wi, re[b] * wi + im[b] * wr);
                re[b] = re[a] - xr;
                im[b] = im[a] - xi;
                re[a] += xr;
                im[a] += xi;
            }
        }
        len <<= 1;
    }
}

/// The 2-D FFT of an s x s complex image (rows, then columns).
fn fft2(re: &mut [f64], im: &mut [f64], s: usize, inverse: bool) {
    re.par_chunks_mut(s).zip(im.par_chunks_mut(s)).for_each(|(r, i)| fft(r, i, inverse));
    let cols: Vec<(Vec<f64>, Vec<f64>)> = (0..s)
        .into_par_iter()
        .map(|x| {
            let mut cr: Vec<f64> = (0..s).map(|y| re[y * s + x]).collect();
            let mut ci: Vec<f64> = (0..s).map(|y| im[y * s + x]).collect();
            fft(&mut cr, &mut ci, inverse);
            (cr, ci)
        })
        .collect();
    for (x, (cr, ci)) in cols.into_iter().enumerate() {
        for y in 0..s {
            re[y * s + x] = cr[y];
            im[y * s + x] = ci[y];
        }
    }
}

/// A raster (h x w, row-major; 0 outside the data) convolved with a kernel normalized to sum 1, as Zonation does:
/// FFT convolution in blocks of the kernel's size, the blocks' results added as float32 in Zonation's order.
pub fn convolve(image: &[f64], h: usize, w: usize, kernel: &Kernel) -> Vec<f32> {
    let (n, img) = kernel.image(h.max(w));
    let total = ffi::prio_eigen_sum(&img);
    let s = 2 * n;
    let (mut fr, mut fi) = (vec![0.0; s * s], vec![0.0; s * s]);
    for y in 0..n {
        for x in 0..n {
            fr[y * s + x] = img[y * n + x] / total;
        }
    }
    fft2(&mut fr, &mut fi, s, false);
    let mut result = vec![0f32; h * w];
    let blocks: Vec<(usize, usize)> =
        (0..h).step_by(n).flat_map(|by| (0..w).step_by(n).map(move |bx| (by, bx))).collect();
    let batch = ((64usize << 20) / (s * s * 16)).max(1);
    for part in blocks.chunks(batch) {
        let outs: Vec<Option<Vec<f64>>> = part
            .par_iter()
            .map(|&(by, bx)| {
                let (y1, x1) = ((by + n).min(h), (bx + n).min(w));
                if !(by..y1).any(|y| image[y * w + bx..y * w + x1].iter().any(|v| *v != 0.0)) {
                    return None;
                }
                let (mut re, mut im) = (vec![0.0; s * s], vec![0.0; s * s]);
                for y in by..y1 {
                    re[(y - by) * s..(y - by) * s + (x1 - bx)].copy_from_slice(&image[y * w + bx..y * w + x1]);
                }
                fft2(&mut re, &mut im, s, false);
                for k in 0..s * s {
                    let (a, b) = (re[k] * fr[k] - im[k] * fi[k], re[k] * fi[k] + im[k] * fr[k]);
                    re[k] = a;
                    im[k] = b;
                }
                fft2(&mut re, &mut im, s, true);
                let norm = 1.0 / (s * s) as f64;
                Some(re.into_iter().map(|v| v * norm).collect())
            })
            .collect();
        for (&(by, bx), out) in part.iter().zip(outs) {
            let Some(out) = out else { continue };
            let (y1, x1) = (by as i64 - (n / 2) as i64, bx as i64 - (n / 2) as i64);
            let (oy, ox) = ((-y1).max(0) as usize, (-x1).max(0) as usize);
            let (ya, xa) = (y1.max(0) as usize, x1.max(0) as usize);
            let (yb, xb) = ((y1 + s as i64).min(h as i64), (x1 + s as i64).min(w as i64));
            if yb <= ya as i64 || xb <= xa as i64 {
                continue;
            }
            for y in ya..yb as usize {
                for x in xa..xb as usize {
                    result[y * w + x] += out[(oy + y - ya) * s + ox + x - xa] as f32;
                }
            }
        }
    }
    result
}

/// An interaction (Zonation's -i, -I): a feature weighted by the connectivity of a resource (`positive`), or
/// `beta` times its share of the product removed (keeping its level with `normalize`).
#[derive(Debug, Clone, Deserialize)]
pub struct Interaction {
    pub kernel: Kernel,
    pub resource: String,
    #[serde(default = "yes")]
    pub positive: bool,
    #[serde(default = "one")]
    pub beta: f64,
    #[serde(default = "yes")]
    pub normalize: bool,
}

/// A feature's matrix connectivity (-m): its kernel and the matrix it shares with the features of the same key.
#[derive(Debug, Clone, Deserialize)]
pub struct MatrixLink {
    pub kernel: Kernel,
    pub matrix: String,
}

/// The connectivity transforms of the features (per input feature; none where null).
#[derive(Debug, Clone, Default)]
pub struct Connectivity {
    pub kernel: Vec<Option<Kernel>>,
    pub self_multiply: Vec<bool>,
    pub interaction: Vec<Option<Interaction>>,
    pub interaction2: Vec<Option<Interaction>>,
    pub matrix: Vec<Option<MatrixLink>>,
    pub matrices: BTreeMap<String, Vec<Vec<f64>>>,
}

impl Connectivity {
    pub fn any(&self) -> bool {
        self.kernel.iter().any(Option::is_some)
            || self.interaction.iter().any(Option::is_some)
            || self.interaction2.iter().any(Option::is_some)
            || self.matrix.iter().any(Option::is_some)
    }

    pub fn check(&self) -> Result<(), String> {
        let kernels = self.kernel.iter().flatten();
        let ia = self.interaction.iter().chain(&self.interaction2).flatten().map(|i| &i.kernel);
        let m = self.matrix.iter().flatten().map(|m| &m.kernel);
        kernels.chain(ia).chain(m).try_for_each(Kernel::check)
    }
}

/// The values of a map at the data cells (NoData as 0).
pub fn gather(map: &[f64], cells: &[i64]) -> Vec<f64> {
    cells.iter().map(|&c| map[c as usize]).map(|v| if v.is_nan() { 0.0 } else { v }).collect()
}

fn dense(values: &[f64], cells: &[i64], n: usize) -> Vec<f64> {
    let mut out = vec![0.0; n];
    for (&c, &v) in cells.iter().zip(values) {
        out[c as usize] = v;
    }
    out
}

/// A layer's quantized values per data cell (-1 without data).
fn quantized(data: &PrioData, k: usize) -> Result<Vec<f64>, String> {
    ffi::prio_data_layer(data, k as i32, false, -1.0).map_err(|e| e.to_string())
}

/// Zonation's connectivity transforms of the quantized layers, in its order: single feature, interaction 1,
/// matrix, interaction 2 (landschaft's _connectivity.apply). `kept`: the input features ranked; `resource`: a
/// resource map by name.
pub fn connect(
    data: &mut cxx::UniquePtr<PrioData>,
    kept: &[usize],
    cells: &[i64],
    (h, w): (usize, usize),
    c: &Connectivity,
    resource: &dyn Fn(&str) -> Result<Vec<f64>, String>,
) -> Result<(), String> {
    let storage = if bits(data) == 8 { 255.0 } else { 65535.0 };
    let storage_r = (1.0 / storage) as f32;
    let replace = |data: &mut cxx::UniquePtr<PrioData>, k: usize, v: &[f32], level: f64| -> Result<(), String> {
        ffi::prio_data_replace_layer(data.pin_mut(), k as i32, v, level).map(|_| ()).map_err(|e| e.to_string())
    };
    let n = h * w;
    // single-feature connectivity
    for (k, &i) in kept.iter().enumerate() {
        let Some(kern) = &c.kernel[i] else { continue };
        let q = quantized(data, k)?;
        let present: Vec<f64> = q.iter().map(|&v| if v >= 0.0 { v } else { 0.0 }).collect();
        let full = convolve(&dense(&present, cells, n), h, w, kern);
        let mut res: Vec<f32> = cells.iter().map(|&c| full[c as usize]).collect();
        if c.self_multiply[i] {
            for (r, p) in res.iter_mut().zip(&present) {
                *r *= (*p as f32) * storage_r;
            }
        }
        for (r, v) in res.iter_mut().zip(&q) {
            if *v < 0.0 {
                *r = 0.0;
            }
        }
        let level = info(data).initial_levels[k];
        replace(data, k, &res, level)?;
    }
    interact(data, kept, cells, (h, w), &c.interaction, resource)?;
    // matrix connectivity: the layers sharing a matrix, in the order of their keys' first features
    let mut keys: Vec<&str> = Vec::new();
    for m in c.matrix.iter().flatten() {
        if !keys.contains(&m.matrix.as_str()) {
            keys.push(&m.matrix);
        }
    }
    for key in keys {
        let members: Vec<(usize, &Kernel)> = kept
            .iter()
            .enumerate()
            .filter_map(|(k, &i)| c.matrix[i].as_ref().filter(|m| m.matrix == key).map(|m| (k, &m.kernel)))
            .collect();
        let mat = c.matrices.get(key).ok_or_else(|| format!("matrix_connectivity: no matrix {key:?} in matrices"))?;
        let m = members.len();
        if mat.len() != m || mat.iter().any(|r| r.len() != m) {
            return Err(format!("matrix {key:?}: {m} x {m} for its {m} features"));
        }
        let scales = info(data).layer_scale;
        let mut conv = Vec::new();
        for &(k, kern) in &members {
            let q = quantized(data, k)?;
            let v: Vec<f64> = q
                .iter()
                .map(|&q| if q >= 0.0 { f64::from((q as f32 as f64 * scales[k]) as f32) } else { 0.0 })
                .collect();
            let full = convolve(&dense(&v, cells, n), h, w, kern);
            conv.push(cells.iter().map(|&c| full[c as usize]).collect::<Vec<f32>>());
        }
        for (t, &(k, _)) in members.iter().enumerate() {
            let mut res = vec![0f32; cells.len()];
            for (s, cv) in conv.iter().enumerate() {
                let f = mat[s][t] as f32;
                for (r, v) in res.iter_mut().zip(cv) {
                    *r += f * v;
                }
            }
            let q = quantized(data, k)?;
            for (r, &q) in res.iter_mut().zip(&q) {
                *r = if q >= 0.0 { (f64::from(*r) * (f64::from(q as f32) * (1.0 / storage))) as f32 } else { 0.0 };
            }
            let level = info(data).initial_levels[k];
            replace(data, k, &res, level)?;
        }
    }
    interact(data, kept, cells, (h, w), &c.interaction2, resource)
}

fn bits(data: &PrioData) -> i32 {
    info(data).bits
}

fn info(data: &PrioData) -> ffi::PrioDataInfo {
    ffi::prio_data_info(data)
}

fn interact(
    data: &mut cxx::UniquePtr<PrioData>,
    kept: &[usize],
    cells: &[i64],
    (h, w): (usize, usize),
    specs: &[Option<Interaction>],
    resource: &dyn Fn(&str) -> Result<Vec<f64>, String>,
) -> Result<(), String> {
    for (k, &i) in kept.iter().enumerate() {
        let Some(ia) = &specs[i] else { continue };
        let r: Vec<f64> = gather(&resource(&ia.resource)?, cells).iter().map(|v| f64::from(*v as f32)).collect();
        let full = convolve(&dense(&r, cells, h * w), h, w, &ia.kernel);
        let q = quantized(data, k)?;
        let dsb: Vec<f32> =
            cells.iter().zip(&q).map(|(&c, &q)| if q >= 0.0 { full[c as usize] } else { 0.0 }).collect();
        let inf = info(data);
        let scale = inf.layer_scale[k];
        let mut a: Vec<f32> =
            q.iter().map(|&q| if q >= 0.0 { (f64::from(q as f32) * scale) as f32 } else { 0.0 }).collect();
        let level = inf.initial_levels[k];
        let to = if ia.positive {
            for (a, d) in a.iter_mut().zip(&dsb) {
                *a *= d;
            }
            level
        } else {
            let mut p: Vec<f32> = a.iter().zip(&dsb).map(|(a, d)| a * d).collect();
            let total = ffi::prio_eigen_sum(&p.iter().map(|v| f64::from(*v)).collect::<Vec<_>>());
            if total > 0.0 {
                let r = (1.0 / total) as f32;
                p.iter_mut().for_each(|v| *v *= r);
            }
            let beta = ia.beta as f32;
            for (a, p) in a.iter_mut().zip(&p) {
                *a = (*a - beta * p).max(0.0);
            }
            if ia.normalize { level } else { ffi::prio_eigen_sum(&a.iter().map(|v| f64::from(*v)).collect::<Vec<_>>()) }
        };
        ffi::prio_data_replace_layer(data.pin_mut(), k as i32, &a, to).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// A feature's retention (Zonation's -r): a fraction of it retained everywhere, a map of fractions per cell, or a
/// map of the amounts retained per cell.
#[derive(Debug, Clone)]
pub enum Retention {
    Everywhere(f64),
    Fractions(String),
    Absolute(String),
}

impl<'de> Deserialize<'de> for Retention {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::Error;
        match serde_json::Value::deserialize(d)? {
            serde_json::Value::Number(n) => Ok(Retention::Everywhere(n.as_f64().unwrap())),
            serde_json::Value::String(s) => Ok(Retention::Fractions(s)),
            serde_json::Value::Object(m) if m.len() == 1 => {
                let (k, v) = m.into_iter().next().unwrap();
                let name =
                    v.as_str().ok_or_else(|| D::Error::custom("retention: {absolute: map} or {fraction: map}"))?;
                match k.as_str() {
                    "absolute" => Ok(Retention::Absolute(name.into())),
                    "fraction" => Ok(Retention::Fractions(name.into())),
                    _ => Err(D::Error::custom("retention: {absolute: map} or {fraction: map}")),
                }
            }
            _ => Err(D::Error::custom("retention: a fraction, a map's name, or {absolute: map}")),
        }
    }
}

/// Zonation's retention transforms (after the others); the cells clipped per feature.
pub fn retain(
    data: &mut cxx::UniquePtr<PrioData>,
    kept: &[usize],
    cells: &[i64],
    retention: &[Option<Retention>],
    map: &dyn Fn(&str) -> Result<Vec<f64>, String>,
) -> Result<Vec<(usize, i64)>, String> {
    let mut clipped = Vec::new();
    for (k, &i) in kept.iter().enumerate() {
        let (kind, values, fraction) = match &retention[i] {
            None => continue,
            Some(Retention::Everywhere(f)) => (2, Vec::new(), *f as f32),
            Some(Retention::Fractions(m)) => (1, gather(&map(m)?, cells), 0.0),
            Some(Retention::Absolute(m)) => (0, gather(&map(m)?, cells), 0.0),
        };
        let v: Vec<f32> = values.iter().map(|v| *v as f32).collect();
        if v.iter().any(|v| *v < 0.0) {
            return Err("retention maps must not have negative values".into());
        }
        if kind == 1 && v.iter().any(|v| *v > 1.0) {
            return Err("fractional retention maps must have values in [0, 1]".into());
        }
        let c = ffi::prio_data_retain(data.pin_mut(), k as i32, kind, &v, fraction).map_err(|e| e.to_string())?;
        if c > 0 {
            clipped.push((i, c));
        }
    }
    Ok(clipped)
}

/// The features with positive, negative and zero weights (all positive without weights).
pub fn weight_classes(w: &[f32], weights_on: bool) -> [(String, Vec<usize>); 3] {
    let of = |f: fn(f32) -> bool| -> Vec<usize> {
        (0..w.len()).filter(|&i| if weights_on { f(w[i]) } else { f(1.0) }).collect()
    };
    [("pos".into(), of(|x| x > 0.0)), ("neg".into(), of(|x| x < 0.0)), ("zero".into(), of(|x| x == 0.0))]
}

/// Zonation's pre-processing maps (main.cpp outputPreprocessedMaps), per data cell: richness (the sum of the
/// features' values) and range-size rarity (of their normalized values) of the features of positive, negative and
/// zero weight and of every output group; with weights, weighted too.
pub fn preprocessing(
    data: &PrioData,
    w: &[f32],
    weights_on: bool,
    groups: Option<&[i64]>,
) -> Result<Vec<(String, Vec<f32>)>, String> {
    let l = w.len();
    let mut sets: Vec<(String, Vec<usize>)> = weight_classes(w, weights_on).into_iter().collect();
    if let Some(g) = groups {
        let mut ids: Vec<i64> = g.iter().copied().filter(|x| *x >= 0).collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            sets.push((format!("group{id}"), (0..l).filter(|&i| g[i] == id).collect()));
        }
    }
    let one = |idx: &[usize], weights: &[f32], unscaled: bool| -> Result<Vec<f32>, String> {
        let mut mult = vec![0f32; l];
        idx.iter().for_each(|&i| mult[i] = 1.0);
        ffi::prio_data_preprocessing(data, &mult, weights, unscaled).map_err(|e| e.to_string())
    };
    let mut out = Vec::new();
    for (name, idx) in sets.iter().filter(|(_, i)| !i.is_empty()) {
        out.push((format!("richness.{name}"), one(idx, &[], true)?));
        out.push((format!("rsr.{name}"), one(idx, &[], false)?));
    }
    if weights_on {
        for (name, idx) in sets.iter().filter(|(n, i)| !i.is_empty() && n != "zero") {
            out.push((format!("richness.{name}.weighted"), one(idx, w, true)?));
            out.push((format!("rsr.{name}.weighted"), one(idx, w, false)?));
        }
    }
    Ok(out)
}

/// A feature as ranked (Zonation's -f): its quantized values times its scale (sum-normalized), NaN without data.
pub fn transformed(data: &PrioData, k: usize) -> Result<Vec<f32>, String> {
    let scale = info(data).layer_scale[k];
    Ok(quantized(data, k)?
        .into_iter()
        .map(|q| if q >= 0.0 { (f64::from(q as f32) * scale) as f32 } else { f32::NAN })
        .collect())
}

/// The features' values per data cell as Zonation's getFeatures<float> (value times the layer's scale, 0 without
/// data): a column per layer.
pub fn features_float(data: &PrioData, layers: usize) -> Result<Vec<Vec<f32>>, String> {
    let scale = info(data).layer_scale;
    (0..layers)
        .map(|k| {
            Ok(quantized(data, k)?
                .into_iter()
                .map(|q| if q >= 0.0 { (f64::from(q as f32) * scale[k]) as f32 } else { 0.0 })
                .collect())
        })
        .collect()
}

/// The sets of features the analyses report on: the weight classes (pos, neg, zero) and the output groups, those
/// with features.
pub fn feature_sets(w: &[f32], weights_on: bool, groups: Option<&[i64]>) -> Vec<(String, Vec<usize>)> {
    let mut sets: Vec<(String, Vec<usize>)> = weight_classes(w, weights_on).into_iter().collect();
    if let Some(g) = groups {
        let mut ids: Vec<i64> = g.iter().copied().filter(|x| *x >= 0).collect();
        ids.sort_unstable();
        ids.dedup();
        for id in ids {
            sets.push((id.to_string(), (0..w.len()).filter(|&i| g[i] == id).collect()));
        }
    }
    sets.retain(|(_, i)| !i.is_empty());
    sets
}

/// Zonation's statistics of an area: its size (cells), mean rank, the sum of the features' values (ds) and
/// weighted (wds), relative to what as many cells hold on average (rfd, wfd), its cost; and the share of every
/// feature of the set in it.
#[derive(Debug, Clone)]
pub struct AreaRow {
    pub set: String,
    pub area: i64,
    pub size: i64,
    pub avgrank: f64,
    pub ds: f64,
    pub wds: f64,
    pub rfd: f64,
    pub wfd: f64,
    pub cost: f64,
    pub coverage: Vec<(usize, f64)>,
}

/// Zonation's areaStatistics for every set of features: `mask` an area per data cell (0: none).
pub fn area_statistics(
    feats: &[Vec<f32>],
    w: &[f32],
    cost: Option<&[f64]>,
    sets: &[(String, Vec<usize>)],
    rank: &[f32],
    mask: &[i64],
) -> Vec<AreaRow> {
    let n = rank.len();
    let mut areas: Vec<i64> = mask.iter().copied().filter(|a| *a != 0).collect();
    areas.sort_unstable();
    areas.dedup();
    let na = areas.len();
    let row = |a: i64| areas.binary_search(&a).unwrap();
    let mut out = Vec::new();
    for (set, idx) in sets {
        let (mut fsum, mut fwsum) = (vec![0.0f64; n], vec![0.0f64; n]);
        for &i in idx {
            for c in 0..n {
                let v = f64::from(feats[i][c]);
                fsum[c] += v;
                fwsum[c] += v * f64::from(w[i]);
            }
        }
        let full: f64 = fsum.iter().sum();
        let wfull: f64 = fwsum.iter().sum();
        let (mut size, mut avg) = (vec![0i64; na], vec![0.0; na]);
        let (mut ds, mut wds, mut ac) = (vec![0.0; na], vec![0.0; na], vec![0.0; na]);
        let mut cov = vec![vec![0.0f64; idx.len()]; na];
        for c in 0..n {
            if mask[c] == 0 {
                continue;
            }
            let r = row(mask[c]);
            size[r] += 1;
            avg[r] += f64::from(rank[c]);
            ds[r] += fsum[c];
            wds[r] += fwsum[c];
            if let Some(cost) = cost {
                ac[r] += f64::from(cost[c] as f32);
            }
            for (j, &i) in idx.iter().enumerate() {
                cov[r][j] += f64::from(feats[i][c]);
            }
        }
        for a in 0..na {
            let avgrank = if size[a] > 0 { avg[a] / size[a] as f64 } else { avg[a] };
            let es = size[a] as f64 * full / n as f64;
            let wes = size[a] as f64 * wfull / n as f64;
            out.push(AreaRow {
                set: set.clone(),
                area: areas[a],
                size: size[a],
                avgrank,
                ds: ds[a],
                wds: wds[a],
                rfd: ds[a] / es,
                wfd: wds[a] / wes,
                cost: ac[a],
                coverage: idx.iter().copied().zip(cov[a].iter().copied()).collect(),
            });
        }
    }
    out
}

/// The cells ranked in [min_rank, max_rank] (1, else 0).
pub fn rank_range_mask(rank: &[f32], min_rank: f64, max_rank: f64) -> Result<Vec<i64>, String> {
    check_range(min_rank, max_rank)?;
    Ok(rank.iter().map(|&r| i64::from(f64::from(r) >= min_rank && f64::from(r) <= max_rank)).collect())
}

fn check_range(lo: f64, hi: f64) -> Result<(), String> {
    if lo > hi {
        return Err("min_rank should be less than max_rank".into());
    }
    if !(0.0..=1.0).contains(&lo) || !(0.0..=1.0).contains(&hi) {
        return Err("min_rank and max_rank should be between 0 and 1".into());
    }
    Ok(())
}

/// Zonation's separate areas: the 8-connected areas of the cells ranked in [min_rank, max_rank] (the rank map,
/// h x w), numbered by their first cell in row order, those under `min_size` cells 0.
pub fn separate_areas(
    rank: &[f64],
    (h, w): (usize, usize),
    min_rank: f64,
    max_rank: f64,
    min_size: i64,
) -> Result<Vec<i64>, String> {
    check_range(min_rank, max_rank)?;
    if min_size <= 0 {
        return Err("min_area_size should be greater than 0 grid cells".into());
    }
    let inside: Vec<bool> = rank.iter().map(|&r| r.is_finite() && r >= min_rank && r <= max_rank).collect();
    let mut labels = vec![0i64; h * w];
    let mut sizes = vec![0i64];
    for start in 0..h * w {
        if !inside[start] || labels[start] != 0 {
            continue;
        }
        let id = sizes.len() as i64;
        let mut stack = vec![start];
        labels[start] = id;
        let mut count = 0;
        while let Some(c) = stack.pop() {
            count += 1;
            let (y, x) = ((c / w) as i64, (c % w) as i64);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (yy, xx) = (y + dy, x + dx);
                    if yy < 0 || xx < 0 || yy >= h as i64 || xx >= w as i64 {
                        continue;
                    }
                    let k = yy as usize * w + xx as usize;
                    if inside[k] && labels[k] == 0 {
                        labels[k] = id;
                        stack.push(k);
                    }
                }
            }
        }
        sizes.push(count);
    }
    Ok(labels.into_iter().map(|l| if l > 0 && sizes[l as usize] >= min_size { l } else { 0 }).collect())
}

/// Zonation's solution comparison: the cells ranked in [min_rank, max_rank] against those of another solution
/// with values in [min_value, max_value] (per data cell; NoData 0): per cell 1 both, 2 only this, 3 only the
/// other, 4 neither; and the counts of both, only this, only the other, neither.
pub fn compare(rank: &[f32], other: &[f64], range: [f64; 4]) -> (Vec<u8>, [i64; 4]) {
    let [lo, hi, vlo, vhi] = range.map(|v| v as f32);
    let mut counts = [0i64; 4];
    let codes = rank
        .iter()
        .zip(other)
        .map(|(&r, &o)| {
            let o = o as f32;
            let code = match (r >= lo && r <= hi, o >= vlo && o <= vhi) {
                (true, true) => 1u8,
                (true, false) => 2,
                (false, true) => 3,
                (false, false) => 4,
            };
            counts[usize::from(code) - 1] += 1;
            code
        })
        .collect();
    (codes, counts)
}

/// Zonation's getInterpolatedOccurrences: the features' remaining levels at a rank, from the curves.
fn interpolated(crank: &[f32], levels: &[f32], l: usize, scalar: f64, rank: f32) -> Vec<f32> {
    let n = crank.len();
    let i1 = ((f64::from(rank) * scalar) as usize).min(n - 1);
    let i2 = (i1 + 1).min(n - 1);
    let r = crank[i2] - crank[i1];
    let t = if r > 0.0 { (rank - crank[i1]) / r } else { 0.0 };
    (0..l).map(|k| (levels[i2 * l + k] - levels[i1 * l + k]) * t + levels[i1 * l + k]).collect()
}

/// Zonation's histogram of the features' coverage of the cells ranked in [min_rank, max_rank]: per set of
/// features, the count of features per bin of coverage. `scalar`: the data cells over the curves' interval.
pub fn histogram(
    crank: &[f32],
    levels: &[f32],
    l: usize,
    scalar: f64,
    sets: &[(String, Vec<usize>)],
    (min_rank, max_rank, bins): (f64, f64, usize),
) -> Result<Vec<(String, Vec<i64>)>, String> {
    if bins < 1 {
        return Err("Number of bars should be greater than 0 in histogram".into());
    }
    if !(0.0..=1.0).contains(&min_rank) || !(0.0..=1.0).contains(&max_rank) {
        return Err("Priority rank should be between 0 and 1 in histogram".into());
    }
    if max_rank < min_rank {
        return Err("Priority rank maximum should not be less than priority rank minimum in histogram".into());
    }
    let at = |r: f64| interpolated(crank, levels, l, scalar, r as f32);
    let (a, b, c) = (at(min_rank), at(max_rank), at(1.0));
    let v: Vec<f32> = (0..l).map(|k| a[k] - b[k] + c[k]).collect();
    Ok(sets
        .iter()
        .map(|(name, idx)| {
            let mut counts = vec![0i64; bins];
            for &i in idx {
                let fo = v[i].clamp(0.0, 1.0);
                counts[((fo * bins as f32) as usize).min(bins - 1)] += 1;
            }
            (name.clone(), counts)
        })
        .collect())
}

// ---------------------------------------------------------------------------------------------------- impacts

/// An impact source of an impact group (a line of Zonation's impact buffers file): the value of its cells in the
/// sources' map, the loss (%) in them, and rings around them (radius in cells, loss %).
#[derive(Debug, Clone, Deserialize)]
pub struct ImpactSource {
    pub group: i64,
    pub source: i64,
    pub footprint: f64,
    #[serde(default)]
    pub buffers: Vec<(i64, f64)>,
}

/// An impact assessment scenario (Zonation's -s): land-use change at the sources (a map of their values, 0 none),
/// reported per subarea (a map, 0 none).
#[derive(Debug, Clone, Deserialize)]
pub struct ImpactScenario {
    pub sources: String,
    pub impacts: Vec<ImpactSource>,
    #[serde(default)]
    pub subareas: Option<String>,
    #[serde(default = "yes")]
    pub rasters: bool,
}

/// A pressure map of a cumulative scenario.
#[derive(Debug, Clone, Deserialize)]
pub struct Pressure {
    pub raster: String,
    #[serde(default)]
    pub normalize: bool,
    #[serde(default)]
    pub offset: f64,
    #[serde(default = "one")]
    pub divisor: f64,
}

/// A response function of an impact group (to one pressure, 1-based, or its default): (pressure, loss) points.
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseFunction {
    pub group: i64,
    #[serde(default)]
    pub pressure: Option<i64>,
    pub points: Vec<(f64, f64)>,
}

fn add() -> String {
    "add".into()
}

/// A cumulative impact assessment scenario (Zonation's -e): direct habitat loss (sum or product of loss maps,
/// weighted per group) and cumulative pressures (variant1: their weighted sum through the group's response;
/// variant2: each through its response, aggregated by add, multiply or max).
#[derive(Debug, Clone, Deserialize)]
pub struct CumulativeScenario {
    #[serde(default)]
    pub subareas: Option<String>,
    #[serde(default = "yes")]
    pub rasters: bool,
    #[serde(default)]
    pub direct: Option<String>,
    #[serde(default)]
    pub direct_rasters: Vec<String>,
    #[serde(default)]
    pub direct_matrix: Option<Vec<Vec<f64>>>,
    #[serde(default)]
    pub cumulative: Option<String>,
    #[serde(default)]
    pub pressures: Vec<Pressure>,
    #[serde(default)]
    pub cumulative_matrix: Option<Vec<Vec<f64>>>,
    #[serde(default = "add")]
    pub aggregation: String,
    #[serde(default)]
    pub functions: Vec<ResponseFunction>,
}

/// The squared Euclidean distance (float32) of every cell of an h x w mask to its nearest true cell (FLT_MAX
/// without any): exact (Felzenszwalb and Huttenlocher's lower envelopes, in integers).
fn sqr_distance(mask: &[bool], h: usize, w: usize) -> Vec<f32> {
    if !mask.iter().any(|m| *m) {
        return vec![f32::MAX; h * w];
    }
    const INF: i64 = i64::MAX / 4;
    // along the columns
    let mut g = vec![INF; h * w];
    for x in 0..w {
        let mut last: Option<usize> = None;
        for y in 0..h {
            if mask[y * w + x] {
                last = Some(y);
            }
            if let Some(l) = last {
                g[y * w + x] = ((y - l) * (y - l)) as i64;
            }
        }
        last = None;
        for y in (0..h).rev() {
            if mask[y * w + x] {
                last = Some(y);
            }
            if let Some(l) = last {
                g[y * w + x] = g[y * w + x].min(((l - y) * (l - y)) as i64);
            }
        }
    }
    // along the rows: the lower envelope of the parabolas (x - q)^2 + g(q)
    let mut out = vec![0f32; h * w];
    let (mut v, mut z) = (vec![0usize; w], vec![0f64; w + 1]);
    for y in 0..h {
        let f = &g[y * w..(y + 1) * w];
        let finite: Vec<usize> = (0..w).filter(|&q| f[q] < INF).collect();
        if finite.is_empty() {
            for x in 0..w {
                out[y * w + x] = f32::MAX;
            }
            continue;
        }
        let mut k = 0;
        v[0] = finite[0];
        z[0] = f64::NEG_INFINITY;
        z[1] = f64::INFINITY;
        for &q in &finite[1..] {
            // z[0] is -inf: the loop stops at the first parabola at the latest
            let mut s;
            loop {
                let p = v[k];
                s = ((f[q] + (q * q) as i64) - (f[p] + (p * p) as i64)) as f64 / (2.0 * (q as f64 - p as f64));
                if s > z[k] {
                    break;
                }
                k -= 1;
            }
            k += 1;
            v[k] = q;
            z[k] = s;
            z[k + 1] = f64::INFINITY;
        }
        let mut j = 0;
        for x in 0..w {
            while z[j + 1] < x as f64 {
                j += 1;
            }
            let p = v[j];
            let d = (x as i64 - p as i64).pow(2) + f[p];
            out[y * w + x] = d as f32;
        }
    }
    out
}

/// A zone of an impact source: its group, source, number (0 the footprint, then the rings), strength (the loss as a
/// fraction) and cells (a mask on the grid).
struct Zone {
    source: i64,
    zone: i64,
    strength: f32,
    mask: Vec<bool>,
}

/// Zonation's ZoneMasks: per group its zones, source by source (ascending), the footprint then the rings.
fn make_zones(sources: &[i64], (h, w): (usize, usize), impacts: &[ImpactSource]) -> BTreeMap<i64, Vec<Zone>> {
    let mut ids: Vec<i64> = impacts.iter().map(|s| s.source).collect();
    ids.sort_unstable();
    ids.dedup();
    let mut groups: Vec<i64> = impacts.iter().map(|s| s.group).collect();
    groups.sort_unstable();
    groups.dedup();
    let mut zones: BTreeMap<i64, Vec<Zone>> = BTreeMap::new();
    for sid in ids {
        let cells: Vec<usize> = (0..h * w).filter(|&c| sources[c] == sid).collect();
        if cells.is_empty() {
            continue;
        }
        let (ys, xs): (Vec<usize>, Vec<usize>) = cells.iter().map(|&c| (c / w, c % w)).unzip();
        let rmax = impacts
            .iter()
            .filter(|s| s.source == sid)
            .filter_map(|s| s.buffers.last().map(|b| b.0))
            .max()
            .unwrap_or(0)
            .max(0) as usize;
        let y0 = ys.iter().min().unwrap().saturating_sub(rmax);
        let y1 = (ys.iter().max().unwrap() + rmax).min(h - 1);
        let x0 = xs.iter().min().unwrap().saturating_sub(rmax);
        let x1 = (xs.iter().max().unwrap() + rmax).min(w - 1);
        let (sh, sw) = (y1 - y0 + 1, x1 - x0 + 1);
        let sub: Vec<bool> = (0..sh * sw).map(|i| sources[(y0 + i / sw) * w + x0 + i % sw] == sid).collect();
        let sqrd = sqr_distance(&sub, sh, sw);
        for &g in &groups {
            for s in impacts.iter().filter(|s| s.group == g && s.source == sid) {
                let mut rings = vec![(-1i64, 0i64, s.footprint)];
                let mut prev = 0;
                for &(r, imp) in &s.buffers {
                    rings.push((prev, r, imp));
                    prev = r;
                }
                for (zn, (rmin, rmx, strength)) in rings.into_iter().enumerate() {
                    let mind = if rmin < 0 { -1.0f32 } else { (rmin * rmin) as f32 };
                    let maxd = (rmx * rmx) as f32;
                    let mut mask = vec![false; h * w];
                    let mut any = false;
                    for (i, &d) in sqrd.iter().enumerate() {
                        if d > mind && d <= maxd {
                            mask[(y0 + i / sw) * w + x0 + i % sw] = true;
                            any = true;
                        }
                    }
                    if any {
                        zones.entry(g).or_default().push(Zone {
                            source: sid,
                            zone: zn as i64,
                            strength: (strength as f32) / 100.0,
                            mask,
                        });
                    }
                }
            }
        }
    }
    zones
}

/// getInterpolatedValue of a response function (lower bound on x, float32).
fn pressure_value(points: &[(f64, f64)], x: &[f32]) -> Vec<f32> {
    let xs: Vec<f32> = points.iter().map(|p| p.0 as f32).collect();
    let ys: Vec<f32> = points.iter().map(|p| p.1 as f32).collect();
    x.iter()
        .map(|&x| {
            let i = xs.partition_point(|v| *v < x);
            if i == 0 {
                ys[0]
            } else if i >= xs.len() {
                ys[ys.len() - 1]
            } else {
                let t = (x - xs[i]) / (xs[i] - xs[i - 1]);
                ys[i] + (ys[i] - ys[i - 1]) * t
            }
        })
        .collect()
}

/// A map read for the impacts: float32, NoData 0, negative values 0.
fn pressure_map(map: &dyn Fn(&str) -> Result<Vec<f64>, String>, name: &str) -> Result<Vec<f32>, String> {
    Ok(map(name)?.into_iter().map(|v| if v.is_nan() { 0.0 } else { (v as f32).max(0.0) }).collect())
}

fn direct_raster(
    sc: &CumulativeScenario,
    row: usize,
    map: &dyn Fn(&str) -> Result<Vec<f64>, String>,
) -> Result<Vec<f32>, String> {
    let weights: Vec<f32> = sc
        .direct_matrix
        .as_ref()
        .and_then(|m| m.get(row))
        .ok_or("the direct habitat loss needs a row of direct_matrix per impact group")?
        .iter()
        .map(|v| *v as f32)
        .collect();
    let multiply = sc.direct.as_deref() == Some("product");
    let mut raster: Option<Vec<f32>> = None;
    for (k, r) in sc.direct_rasters.iter().enumerate() {
        if weights.get(k).copied().unwrap_or(0.0) <= 0.0 {
            continue;
        }
        let data = pressure_map(map, r)?;
        let acc = raster.get_or_insert_with(|| vec![if multiply { 1.0 } else { 0.0 }; data.len()]);
        for (a, d) in acc.iter_mut().zip(&data) {
            *a = if multiply { *a * (1.0 - d) } else { *a + d * weights[k] };
        }
    }
    let mut raster = raster.ok_or_else(|| format!("empty direct habitat loss raster for impact group row {row}"))?;
    if !multiply {
        raster.iter_mut().for_each(|v| *v = 1.0 - v.min(1.0));
    }
    Ok(raster)
}

fn cumulative_raster(
    sc: &CumulativeScenario,
    group: i64,
    row: usize,
    map: &dyn Fn(&str) -> Result<Vec<f64>, String>,
) -> Result<Vec<f32>, String> {
    let variant1 = sc.cumulative.as_deref() == Some("variant1");
    let agg = if variant1 { "add" } else { sc.aggregation.as_str() };
    let n = sc.pressures.len();
    let weights: Vec<f32> = if variant1 {
        sc.cumulative_matrix
            .as_ref()
            .and_then(|m| m.get(row))
            .ok_or("variant1 needs a row of cumulative_matrix per impact group")?
            .iter()
            .map(|v| *v as f32)
            .collect()
    } else {
        vec![1.0; n]
    };
    let fun = |p: Option<i64>| sc.functions.iter().find(|f| f.group == group && f.pressure == p).map(|f| &f.points);
    let mut raster: Option<Vec<f32>> = None;
    for (k, p) in sc.pressures.iter().enumerate() {
        if weights.get(k).copied().unwrap_or(0.0) <= 0.0 {
            continue;
        }
        let mut data = pressure_map(map, &p.raster)?;
        if p.normalize {
            let m = data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));
            if m > 0.0 {
                data.iter_mut().for_each(|v| *v /= m);
            }
        }
        if p.offset != 0.0 {
            let o = p.offset as f32;
            data.iter_mut().for_each(|v| *v = (*v - o).max(0.0));
        }
        if p.divisor != 1.0 {
            if p.divisor <= 0.0 {
                return Err("zero divisor for a pressure raster".into());
            }
            let d = p.divisor as f32;
            data.iter_mut().for_each(|v| *v /= d);
        }
        if !variant1 {
            let f = fun(Some(k as i64 + 1))
                .or_else(|| fun(None))
                .ok_or_else(|| format!("no response function for impact group {group}, pressure {}", k + 1))?;
            data = pressure_value(f, &data);
        }
        let acc = raster.get_or_insert_with(|| vec![if agg == "multiply" { 1.0 } else { 0.0 }; data.len()]);
        for (a, d) in acc.iter_mut().zip(&data) {
            *a = match agg {
                "add" => *a + d,
                "multiply" => *a * (1.0 - d),
                _ => a.max(*d),
            };
        }
    }
    let mut raster = raster.ok_or_else(|| format!("empty cumulative pressure raster for impact group {group}"))?;
    if variant1 {
        let f = fun(None).ok_or_else(|| format!("pressure function not found for impact group {group}"))?;
        raster = pressure_value(f, &raster);
    }
    if agg != "multiply" {
        raster.iter_mut().for_each(|v| *v = 1.0 - v.min(1.0));
    }
    Ok(raster)
}

/// getLayerBins: the value intervals (float32, of the sum-normalized values) of a feature: `spec` 0 for ten
/// automatic bins, or the limits in the feature's units.
fn layer_bins(spec: &serde_json::Value, max_norm_sum: f64, orig_abs_sum: f64) -> Option<Vec<(f32, f32)>> {
    let limits: Option<Vec<f64>> = match spec {
        serde_json::Value::Number(n) if n.as_f64().unwrap() < 0.0 => return None,
        serde_json::Value::Number(_) => None,
        serde_json::Value::Array(a) => Some(a.iter().filter_map(serde_json::Value::as_f64).collect()),
        _ => return None,
    };
    let mut bins: Vec<f32> = if max_norm_sum > 0.0 {
        let mut b: Vec<f32> = match limits {
            None => {
                let m = 1.0 / max_norm_sum;
                (0..=10).map(|i| (i as f64 * m / 10.0) as f32).collect()
            }
            Some(l) => l.iter().map(|&b| (f64::from(b as f32) / orig_abs_sum) as f32).collect(),
        };
        if b.len() == 1 {
            b.push(f32::MAX);
        } else {
            let last = b.len() - 1;
            // the next float32 up (np.nextafter towards FLT_MAX)
            let x = b[last];
            if x < f32::MAX {
                b[last] = if x == 0.0 {
                    f32::from_bits(1)
                } else if x > 0.0 {
                    f32::from_bits(x.to_bits() + 1)
                } else {
                    f32::from_bits(x.to_bits() - 1)
                };
            }
        }
        b
    } else {
        vec![0.0, f32::MAX]
    };
    if bins.len() < 2 {
        bins.push(f32::MAX);
    }
    Some(bins.windows(2).map(|p| (p[0], p[1])).collect())
}

/// Zonation's Impact{impact, loss, area} sums per key, added in its order: by cell, then by the order within the
/// cell (zone, phase).
#[derive(Default)]
struct Accumulator {
    keys: Vec<Vec<i64>>,
    at: std::collections::HashMap<Vec<i64>, usize>,
    /// per key its impacts and losses: (cell, order within the cell, value)
    parts: Vec<(Vec<Entry>, Vec<Entry>)>,
}

type Entry = (usize, usize, f64);

impl Accumulator {
    fn entry(&mut self, key: Vec<i64>) -> usize {
        if let Some(&i) = self.at.get(&key) {
            return i;
        }
        self.keys.push(key.clone());
        self.parts.push((Vec::new(), Vec::new()));
        self.at.insert(key, self.parts.len() - 1);
        self.parts.len() - 1
    }

    fn impact(&mut self, key: Vec<i64>, cells: &[usize], sub: usize, v: &[f32]) {
        if cells.is_empty() {
            return;
        }
        let i = self.entry(key);
        self.parts[i].0.extend(cells.iter().map(|&c| (c, sub, f64::from(v[c]))));
    }

    fn loss(&mut self, key: Vec<i64>, cells: &[usize], sub: usize, v: &[f32]) {
        if cells.is_empty() {
            return;
        }
        let i = self.entry(key);
        self.parts[i].1.extend(cells.iter().map(|&c| (c, sub, f64::from(v[c]))));
    }

    /// Per key in the order first met: the impact, the loss and the cells of the impact.
    fn finish(mut self) -> Vec<(Vec<i64>, [f64; 3])> {
        let sum = |p: &mut Vec<Entry>| -> f64 {
            p.sort_by_key(|e| (e.0, e.1));
            p.iter().fold(0.0, |a, e| a + e.2)
        };
        let keys = std::mem::take(&mut self.keys);
        keys.into_iter()
            .zip(self.parts.iter_mut())
            .map(|(k, (imp, loss))| {
                let n = imp.len() as f64;
                (k, [sum(imp), sum(loss), n])
            })
            .collect()
    }
}

fn round_half_away(x: f32) -> f32 {
    let t = x.trunc();
    let frac = x - t;
    t + if frac.abs() >= 0.5 { x.signum() } else { 0.0 }
}

/// The impacts of the scenarios: per area (0 all), group, source, zone, feature (its layer) and bin (-1 all), the
/// features' distribution in the zone, its loss and cells; per area, group, phase (1 direct, 2 cumulative) and
/// feature the same; and the impact maps of the groups.
pub struct Impacts {
    pub zones: Vec<[f64; 9]>,
    pub cumulative: Vec<[f64; 7]>,
    pub maps: Vec<(String, Vec<f32>)>,
}

/// Zonation's runImpactScenario: the features of the impact groups lose what the scenarios take (the quantized
/// data reduced, the levels scaled by what remains), and the impacts accumulated. `groups` and `bins` per input
/// feature; `map` a map by name (on the grid).
#[allow(clippy::too_many_arguments)]
pub fn impacts(
    data: &mut cxx::UniquePtr<PrioData>,
    kept: &[usize],
    cells: &[i64],
    (h, w): (usize, usize),
    groups: &[i64],
    bins_spec: &[serde_json::Value],
    scenario: Option<&ImpactScenario>,
    cumulative: Option<&CumulativeScenario>,
    map: &dyn Fn(&str) -> Result<Vec<f64>, String>,
) -> Result<Impacts, String> {
    let n = cells.len();
    let mut group_layers: BTreeMap<i64, Vec<usize>> = BTreeMap::new();
    for (k, &i) in kept.iter().enumerate() {
        if groups[i] >= 0 {
            group_layers.entry(groups[i]).or_default().push(k);
        }
    }
    let group_numbers: Vec<i64> = group_layers.keys().copied().collect();
    let inf = info(data);
    let mut bins: BTreeMap<usize, Vec<(f32, f32)>> = BTreeMap::new();
    for (k, &i) in kept.iter().enumerate() {
        if let Some(b) = bins_spec.get(i).and_then(|s| layer_bins(s, inf.max_norm_sums[k], inf.orig_abs_sums[k])) {
            bins.insert(k, b);
        }
    }
    // a subareas map: per data cell the position of its value among those above 0 (1..), 0 none
    let areas_of = |name: Option<&String>| -> Result<(Vec<i64>, Vec<i64>), String> {
        let Some(name) = name else { return Ok((vec![0; n], Vec::new())) };
        let a: Vec<i64> =
            gather(&map(name)?, cells).iter().map(|v| v.round_ties_even().clamp(0.0, 4_294_967_295.0) as i64).collect();
        let mut ids: Vec<i64> = a.iter().copied().filter(|v| *v != 0).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return Err("the subareas raster has no values above 0".into());
        }
        Ok((a.iter().map(|v| ids.binary_search(v).map_or(0, |p| p as i64 + 1)).collect(), ids))
    };
    let (mut src, mut zones) = (None, BTreeMap::new());
    if let Some(sc) = scenario {
        let ids: Vec<i64> = map(&sc.sources)?
            .iter()
            .map(|v| if v.is_nan() { 0 } else { v.round_ties_even().clamp(0.0, 4_294_967_295.0) as i64 })
            .collect();
        zones = make_zones(&ids, (h, w), &sc.impacts);
        let (area, area_ids) = areas_of(sc.subareas.as_ref())?;
        src = Some((area, area_ids, Accumulator::default()));
    }
    let mut cum = None;
    if let Some(cs) = cumulative {
        let (area, area_ids) = areas_of(cs.subareas.as_ref())?;
        cum = Some((area, area_ids, Accumulator::default()));
    }
    let mut maps = Vec::new();
    let full_at = |v: &[f32]| -> Vec<f32> { cells.iter().map(|&c| v[c as usize]).collect() };
    for (grow, &g) in group_numbers.iter().enumerate() {
        let (mut direct, mut cumr) = (None, None);
        if let Some(cs) = cumulative {
            if cs.direct.is_some() {
                direct = Some(direct_raster(cs, grow, map)?);
            }
            if cs.cumulative.is_some() {
                cumr = Some(cumulative_raster(cs, g, grow, map)?);
            }
        }
        let zl = if src.is_some() { zones.get(&g) } else { None };
        let mut impact_raster = None;
        let mut dense: Vec<Vec<bool>> = Vec::new();
        if let Some(zl) = zl {
            let mut r = vec![1f32; h * w];
            for z in zl {
                for (v, m) in r.iter_mut().zip(&z.mask) {
                    if *m {
                        *v *= 1.0 - z.strength;
                    }
                }
            }
            dense = zl.iter().map(|z| cells.iter().map(|&c| z.mask[c as usize]).collect()).collect();
            impact_raster = Some(r);
        }
        for &k in &group_layers[&g] {
            let inf = info(data);
            let scale = inf.layer_scale[k];
            let q = quantized(data, k)?;
            let mut v: Vec<f32> =
                q.iter().map(|&q| if q >= 0.0 { (f64::from(q as f32) * scale) as f32 } else { 0.0 }).collect();
            let inv = if scale > 0.0 { 1.0 / scale } else { 0.0 };
            let old = ffi::prio_eigen_sum(&v.iter().map(|x| f64::from(*x)).collect::<Vec<_>>()) * inv;
            let pos: Vec<bool> = v.iter().map(|x| *x > 0.0).collect();
            if let Some((area, _, acc)) = cum.as_mut() {
                let mut first = pos.clone();
                for (sub, (phase, rast)) in [(1i64, &direct), (2, &cumr)].into_iter().enumerate() {
                    let Some(rast) = rast else { continue };
                    let imp = full_at(rast);
                    let sel: Vec<bool> = (0..n).map(|c| pos[c] && imp[c] < 1.0).collect();
                    let loss: Vec<f32> = (0..n).map(|c| (1.0 - imp[c]) * v[c]).collect();
                    for all in [true, false] {
                        let key_of = |c: usize| if all { 0 } else { area[c] };
                        let s: Vec<usize> = (0..n).filter(|&c| sel[c] && (all || area[c] > 0)).collect();
                        let mut ids: Vec<i64> = s.iter().map(|&c| key_of(c)).collect();
                        ids.sort_unstable();
                        ids.dedup();
                        for a in ids {
                            let m: Vec<usize> = s.iter().copied().filter(|&c| key_of(c) == a).collect();
                            let f: Vec<usize> = m.iter().copied().filter(|&c| first[c]).collect();
                            acc.impact(vec![0, a, k as i64], &f, sub, &v);
                            acc.loss(vec![0, a, k as i64], &m, sub, &loss);
                            acc.impact(vec![phase, a, g, k as i64], &m, sub, &v);
                            acc.loss(vec![phase, a, g, k as i64], &m, sub, &loss);
                        }
                    }
                    for c in 0..n {
                        if sel[c] {
                            v[c] *= imp[c];
                            first[c] = false;
                        }
                    }
                }
            }
            if let (Some(zl), Some(ir), Some((area, _, acc))) = (zl, impact_raster.as_ref(), src.as_mut()) {
                let imp = full_at(ir);
                let sel: Vec<bool> = (0..n).map(|c| pos[c] && imp[c] < 1.0).collect();
                let total_loss: Vec<f32> = (0..n).map(|c| (1.0 - imp[c]) * v[c]).collect();
                let mut tstr = vec![0f32; n];
                for (z, m) in zl.iter().zip(&dense) {
                    for c in 0..n {
                        if m[c] {
                            tstr[c] += z.strength;
                        }
                    }
                }
                let tr: Vec<f32> = tstr.iter().map(|t| if *t > 0.0 { 1.0 / t } else { 0.0 }).collect();
                let bk: Vec<i64> = match bins.get(&k) {
                    Some(b) => v
                        .iter()
                        .map(|x| (0..b.len()).find(|&i| *x >= b[i].0 && *x < b[i].1).map_or(-1, |i| i as i64))
                        .collect(),
                    None => vec![-1; n],
                };
                let mut first = sel.clone();
                for (sub, (z, m)) in zl.iter().zip(&dense).enumerate() {
                    let mz: Vec<usize> = (0..n).filter(|&c| sel[c] && m[c]).collect();
                    let loss: Vec<f32> = (0..n).map(|c| (total_loss[c] * z.strength) * tr[c]).collect();
                    for all in [true, false] {
                        let key_of = |c: usize| if all { 0 } else { area[c] };
                        let ms: Vec<usize> = mz.iter().copied().filter(|&c| all || area[c] > 0).collect();
                        let mut ids: Vec<i64> = ms.iter().map(|&c| key_of(c)).collect();
                        ids.sort_unstable();
                        ids.dedup();
                        for a in ids {
                            let m2: Vec<usize> = ms.iter().copied().filter(|&c| key_of(c) == a).collect();
                            let fm: Vec<usize> = m2.iter().copied().filter(|&c| first[c]).collect();
                            let (kk, zone) = (k as i64, vec![1, a, g, z.source, k as i64, -1, z.zone]);
                            acc.impact(zone.clone(), &m2, sub, &v);
                            acc.loss(zone, &m2, sub, &loss);
                            acc.loss(vec![0, a, g, kk, -1], &m2, sub, &loss);
                            acc.impact(vec![0, a, g, kk, -1], &fm, sub, &v);
                            let mut bs: Vec<i64> = m2.iter().map(|&c| bk[c]).filter(|b| *b >= 0).collect();
                            bs.sort_unstable();
                            bs.dedup();
                            for b in bs {
                                let mb: Vec<usize> = m2.iter().copied().filter(|&c| bk[c] == b).collect();
                                let fb: Vec<usize> = mb.iter().copied().filter(|&c| first[c]).collect();
                                let zone = vec![1, a, g, z.source, kk, b, z.zone];
                                acc.impact(zone.clone(), &mb, sub, &v);
                                acc.loss(zone, &mb, sub, &loss);
                                acc.loss(vec![0, a, g, kk, b], &mb, sub, &loss);
                                acc.impact(vec![0, a, g, kk, b], &fb, sub, &v);
                            }
                        }
                    }
                    for &c in &mz {
                        first[c] = false;
                    }
                }
                for c in 0..n {
                    if sel[c] {
                        v[c] *= imp[c];
                    }
                }
            }
            // back to the storage units; the level scaled by what remains
            let invf = inv as f32;
            let newq: Vec<f32> = v.iter().map(|x| round_half_away(x * invf)).collect();
            if old > 0.0 {
                let new = ffi::prio_eigen_sum(&newq.iter().map(|x| f64::from(*x)).collect::<Vec<_>>());
                ffi::prio_data_scale_level(data.pin_mut(), k as i32, new / old).map_err(|e| e.to_string())?;
            }
            ffi::prio_data_set_layer_values(data.pin_mut(), k as i32, &newq).map_err(|e| e.to_string())?;
        }
        if let (Some(ir), Some(sc)) = (&impact_raster, scenario)
            && sc.rasters
        {
            maps.push((format!("ig{g}_zones"), ir.iter().map(|v| 1.0 - v).collect()));
        }
        if cumulative.is_some_and(|c| c.rasters) {
            if let Some(d) = &direct {
                maps.push((format!("ig{g}_directhabitatloss"), d.iter().map(|v| 1.0 - v).collect()));
            }
            if let Some(c) = &cumr {
                maps.push((format!("ig{g}_accumulativeloss"), c.iter().map(|v| 1.0 - v).collect()));
            }
        }
    }
    let area_value = |ids: &[i64], a: i64| if a == 0 { 0.0 } else { ids[a as usize - 1] as f64 };
    let mut out = Impacts { zones: Vec::new(), cumulative: Vec::new(), maps };
    if let Some((_, ids, acc)) = src {
        for (key, [imp, loss, cells]) in acc.finish() {
            if key[0] == 1 {
                let [_, a, g, s, k, b, z] = key[..] else { unreachable!() };
                out.zones.push([
                    area_value(&ids, a),
                    g as f64,
                    s as f64,
                    z as f64,
                    (kept[k as usize] + 1) as f64,
                    b as f64,
                    imp,
                    loss,
                    cells,
                ]);
            }
        }
    }
    if let Some((_, ids, acc)) = cum {
        for (key, [imp, loss, cells]) in acc.finish() {
            if key[0] != 0 {
                let [phase, a, g, k] = key[..] else { unreachable!() };
                out.cumulative.push([
                    area_value(&ids, a),
                    g as f64,
                    phase as f64,
                    (kept[k as usize] + 1) as f64,
                    imp,
                    loss,
                    cells,
                ]);
            }
        }
    }
    Ok(out)
}
