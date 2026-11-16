//! Zonation 5's output files of a ranking, as terraflux-py writes them (`ZonationResult.write`): `rankmap.tif`,
//! `feature_curves.csv`, `summary_curves.csv`, `feature_density_curves.csv`, `group_curves.csv` (with output groups)
//! and `features_info.csv`, digit for digit like Zonation's own. The means are Zonation's float32 sums (Eigen's
//! reduction order, through `prio_eigen_sum_f32`) so the tables match its files.

use std::path::Path;

use terraflux_ls_sys::ffi;
use terraflux_raster::{DataType, Grid, Raster};

use crate::{Error, Result};

/// C's `%.{prec}f` of a float32 widened to a double, as Python (and so numpy's savers) print it: `nan`, `inf`.
pub fn fixed(x: f32, prec: usize) -> String {
    let x = f64::from(x);
    if x.is_nan() {
        "nan".into()
    } else if x.is_infinite() {
        if x > 0.0 { "inf".into() } else { "-inf".into() }
    } else {
        format!("{x:.prec$}")
    }
}

/// C's `%g` with 6 significant digits.
pub fn g6(x: f64) -> String {
    if x.is_nan() {
        return "nan".into();
    }
    if x.is_infinite() {
        return if x > 0.0 { "inf".into() } else { "-inf".into() };
    }
    if x == 0.0 {
        return if x.is_sign_negative() { "-0".into() } else { "0".into() };
    }
    let trim = |s: &str| -> String {
        if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_string() } else { s.to_string() }
    };
    let s = format!("{x:.5e}");
    let (mant, exp) = s.split_once('e').unwrap();
    let exp: i32 = exp.parse().unwrap();
    if !(-4..6).contains(&exp) {
        format!("{}e{}{:02}", trim(mant), if exp < 0 { '-' } else { '+' }, exp.abs())
    } else {
        trim(&format!("{:.*}", (5 - exp) as usize, x))
    }
}

/// Zonation's `{}` of a double: like `%g`, with ".0" when it looks like an integer.
pub fn g(x: f64) -> String {
    let s = g6(x);
    if s.contains(['.', 'e', 'n', 'i']) { s } else { s + ".0" }
}

/// Mean, weighted mean, max and min of the layers `idx` at every sample (CurveInfo::read): Zonation copies the
/// values into a fresh array, whose sums start aligned.
struct Info {
    mean: Vec<f32>,
    wmean: Vec<f32>,
    max: Vec<f32>,
    min: Vec<f32>,
}

fn sum(v: &[f32], offset: i64) -> f32 {
    ffi::prio_eigen_sum_f32(v, offset)
}

fn info(levels: &[f32], l: usize, weights: &[f32], idx: &[usize]) -> Info {
    let samples = levels.len().checked_div(l).unwrap_or(0);
    let n = idx.len() as f32;
    let w: Vec<f32> = idx.iter().map(|&i| weights[i]).collect();
    let wsum = sum(&w, 0);
    let mut out = Info {
        mean: Vec::with_capacity(samples),
        wmean: Vec::with_capacity(samples),
        max: Vec::with_capacity(samples),
        min: Vec::with_capacity(samples),
    };
    let mut sub = vec![0f32; idx.len()];
    for s in 0..samples {
        let row = &levels[s * l..(s + 1) * l];
        for (d, &i) in sub.iter_mut().zip(idx) {
            *d = row[i];
        }
        let mean = sum(&sub, 0) / n;
        out.wmean.push(if wsum != 0.0 {
            let prod: Vec<f32> = sub.iter().zip(&w).map(|(a, b)| a * b).collect();
            sum(&prod, 0) / wsum
        } else {
            mean
        });
        out.mean.push(mean);
        out.max.push(sub.iter().copied().fold(f32::NEG_INFINITY, f32::max));
        out.min.push(sub.iter().copied().fold(f32::INFINITY, f32::min));
    }
    out
}

/// The weight classes: the layers of positive, negative and zero weight (all positive without weights).
fn classes(weights: &[f32], weights_on: bool) -> [Vec<usize>; 3] {
    if !weights_on {
        return [(0..weights.len()).collect(), Vec::new(), Vec::new()];
    }
    let pick = |f: &dyn Fn(f32) -> bool| (0..weights.len()).filter(|&i| f(weights[i])).collect();
    [pick(&|w| w > 0.0), pick(&|w| w < 0.0), pick(&|w| w == 0.0)]
}

/// What the tables are made of: the curves of a ranking and the settings of its features.
pub struct Curves<'a> {
    /// The rank at every sample.
    pub rank: &'a [f32],
    /// The remaining level of every layer (samples x layers).
    pub levels: &'a [f32],
    /// The cost remaining at every sample.
    pub cost: &'a [f64],
    /// The weight of every layer.
    pub weights: &'a [f32],
    pub weights_on: bool,
    /// The cells with data.
    pub cells: usize,
    /// The output group of every layer (-1: none).
    pub groups: Option<&'a [i64]>,
    /// The header of every layer in feature_curves.csv: the quoted name or the number.
    pub headers: &'a [String],
}

impl Curves<'_> {
    fn l(&self) -> usize {
        self.weights.len()
    }

    /// feature_curves.csv.
    pub fn feature_curves(&self) -> String {
        let l = self.l();
        let mut out = String::from("\"rank\" ");
        for h in self.headers {
            out += h;
            out.push(' ');
        }
        out.push('\n');
        for (s, r) in self.rank.iter().enumerate() {
            out += &fixed(*r, 5);
            out.push(' ');
            for v in &self.levels[s * l..(s + 1) * l] {
                out += &fixed(*v, 4);
                out.push(' ');
            }
            out.push('\n');
        }
        out
    }

    /// summary_curves.csv.
    pub fn summary_curves(&self) -> String {
        let l = self.l();
        let samples = self.rank.len();
        let all: Vec<usize> = (0..l).collect();
        // whole rows of the curve matrix: the sums start (s * L) % 8 floats after an aligned start
        let mean: Vec<f32> = (0..samples)
            .map(|s| ffi::prio_eigen_sum_f32(&self.levels[s * l..(s + 1) * l], ((s * l) % 8) as i64) / l as f32)
            .collect();
        let whole = info(self.levels, l, self.weights, &all);
        let mut head = String::from("\"rank\" \"remaining_area\" \"remaining_cost\" \"mean\" \"max\" \"min\"");
        let [pos, neg, zero] = classes(self.weights, self.weights_on);
        let infos: Vec<(&str, bool, Info)> =
            [("positive", true, &pos), ("zero", false, &zero), ("negative", true, &neg)]
                .into_iter()
                .filter(|(_, _, idx)| !idx.is_empty())
                .map(|(name, weighted, idx)| (name, weighted, info(self.levels, l, self.weights, idx)))
                .collect();
        for (name, weighted, _) in &infos {
            head += &format!(" \"mean_{name}\"");
            if *weighted {
                head += &format!(" \"weighted_mean_{name}\"");
            }
            head += &format!(" \"max_{name}\" \"min_{name}\"");
        }
        let mut out = head + "\n";
        for (s, &rank) in self.rank.iter().enumerate() {
            let area = (1.0f32 - rank) * self.cells as f32;
            let cost = self.cost.get(s).copied().unwrap_or(f64::NAN);
            out += &format!(
                "{} {} {} {} {} {}",
                fixed(rank, 5),
                (f64::from(area).trunc()) as i64,
                g(cost),
                fixed(mean[s], 4),
                fixed(whole.max[s], 4),
                fixed(whole.min[s], 4)
            );
            for (_, weighted, ci) in &infos {
                out += &format!(" {}", fixed(ci.mean[s], 4));
                if *weighted {
                    out += &format!(" {}", fixed(ci.wmean[s], 4));
                }
                out += &format!(" {} {}", fixed(ci.max[s], 4), fixed(ci.min[s], 4));
            }
            out.push('\n');
        }
        out
    }

    /// feature_density_curves.csv.
    pub fn density_curves(&self) -> String {
        let l = self.l();
        let samples = self.rank.len();
        let [pos, neg, zero] = classes(self.weights, self.weights_on);
        // the change of the rank between samples, and the density of a mean: the fall of the mean per rank
        let dr: Vec<f32> = (0..samples).map(|s| if s == 0 { 0.0 } else { self.rank[s] - self.rank[s - 1] }).collect();
        let density = |m: &[f32]| -> Vec<f32> {
            (0..samples).map(|s| if s == 0 { 0.0 } else { (m[s - 1] - m[s]) / dr[s] }).collect()
        };
        let mut head = String::from("\"rank\" ");
        let mut cols: Vec<Vec<f32>> = Vec::new();
        for (suffix, weighted, idx) in [("pos", true, &pos), ("neg", true, &neg), ("zero", false, &zero)] {
            if idx.is_empty() {
                continue;
            }
            let ci = info(self.levels, l, self.weights, idx);
            head += &format!("\"feature_density_{suffix}\" ");
            cols.push(density(&ci.mean));
            if weighted {
                head += &format!("\"weighted_feature_density_{suffix}\" ");
                cols.push(density(&ci.wmean));
            }
        }
        let mut out = head + "\n";
        for s in 0..samples {
            out += &fixed(self.rank[s], 5);
            for c in &cols {
                out.push(' ');
                out += &fixed(c[s], 4);
            }
            out.push('\n');
        }
        out
    }

    /// group_curves.csv, with output groups.
    pub fn group_curves(&self) -> Option<String> {
        let groups = self.groups?;
        let l = self.l();
        let mut ids: Vec<i64> = groups.iter().copied().filter(|g| *g >= 0).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.is_empty() {
            return None;
        }
        let mut head = String::from("\"rank\" ");
        let mut cols: Vec<Vec<f32>> = Vec::new();
        for g in &ids {
            let idx: Vec<usize> = (0..l).filter(|&i| groups[i] == *g).collect();
            let ci = info(self.levels, l, self.weights, &idx);
            head += &format!("\"mean.{g}\" \"weighted_mean.{g}\" \"min.{g}\" ");
            cols.extend([ci.mean, ci.wmean, ci.min]);
        }
        let mut out = head + "\n";
        for (s, r) in self.rank.iter().enumerate() {
            out += &fixed(*r, 5);
            out.push(' ');
            for c in &cols {
                out += &fixed(c[s], 4);
                out.push(' ');
            }
            out.push('\n');
        }
        Some(out)
    }
}

/// The features of a ranking, as features_info.csv has them.
pub struct Features<'a> {
    /// The layer's number (0-based, among the features given).
    pub numbers: &'a [usize],
    pub names: &'a [String],
    pub dsum: &'a [f64],
    pub count: &'a [i64],
    pub missing: &'a [i64],
    pub quant_error: &'a [f64],
}

impl Features<'_> {
    /// features_info.csv.
    pub fn text(&self) -> String {
        let mut out = String::from("\"number\" \"dsum\" \"count\" \"missing\" \"quant_error\" \"name\"\n");
        for i in 0..self.numbers.len() {
            out += &format!(
                "{} {} {} {} {} \"{}\"\n",
                self.numbers[i],
                g(self.dsum[i]),
                self.count[i],
                self.missing[i],
                g(self.quant_error[i]),
                self.names[i]
            );
        }
        out
    }
}

/// coverage_matrix.<set>[.<id>].csv: per area the share of every feature of the set in it.
pub fn coverage_matrix(headers: &[String], rows: &[(i64, Vec<f64>)]) -> String {
    let mut out = String::from("\"area\" ");
    for h in headers {
        out += &format!("{h} ");
    }
    out.push('\n');
    for (area, v) in rows {
        out += &format!("{area} ");
        for x in v {
            out += &format!("{} ", g6(*x));
        }
        out.push('\n');
    }
    out
}

/// area_stats.<set>[.<id>].csv: per area its size, mean rank, the features' sum (and weighted), relative to the
/// average (and weighted) and the cost.
pub fn area_stats(rows: &[(i64, i64, [f64; 6])]) -> String {
    let mut out = String::from("\"area\" \"size\" \"avgrank\" \"ds\" \"wds\" \"rfd\" \"wfd\" \"cost\"\n");
    for (area, size, v) in rows {
        out += &format!("{area} {size}");
        for x in v {
            out += &format!(" {}", g6(*x));
        }
        out.push('\n');
    }
    out
}

/// highest_occurrences.<set>[.<id>].txt: per area the features holding more than 0.1% of their distribution in it,
/// the largest first: the percentage, number, quoted name and file.
pub fn highest_occurrences(
    rows: &[(i64, Vec<f64>)],
    idx: &[usize],
    numbers: &[usize],
    quoted: &[String],
    files: &[String],
) -> String {
    let mut out = String::new();
    for (area, sums) in rows {
        let mut order: Vec<usize> = (0..sums.len()).collect();
        order.sort_by(|a, b| sums[*b].partial_cmp(&sums[*a]).unwrap_or(std::cmp::Ordering::Equal));
        let mut block = String::new();
        for i in order {
            let pct = sums[i] * 100.0;
            if pct <= 0.1 {
                break;
            }
            let k = idx[i];
            block += &format!("{} {} {} {}\n", g6(pct), numbers[k], quoted[k], files[k]);
        }
        if !block.is_empty() {
            out += &format!("Area {area}\n{block}\n");
        }
    }
    out
}

/// solution_comparison[.<id>].txt.
pub fn solution_comparison(file: &str, range: [f64; 4], counts: [f64; 4], n: f64) -> String {
    let [both, only_this, only_other, neither] = counts;
    let pct = |c: f64| g(100.0 * c / n);
    format!(
        "Comparing present solution to {file}\n\
         Priority rank range of interest in present solution is from {} to {}\n\
         Numeric range of interest in comparison file is from {} to {}\n\
         Number of grid cells chosen in both solutions is {both} ({}% of analysis area)\n\
         Number of grid cells chosen in neither solution is {neither} ({}% of analysis area)\n\
         Number of grid cells chosen only in present solution is {only_this} ({}% of analysis area)\n\
         Number of grid cells chosen only in comparison file is {only_other} ({}% of analysis area)\n",
        g(range[0]),
        g(range[1]),
        g(range[2]),
        g(range[3]),
        pct(both),
        pct(neither),
        pct(only_this),
        pct(only_other)
    )
}

/// histogram.<set>[.<id>].txt: the count of features per bin of coverage.
pub fn histogram(set: &str, lo: f64, hi: f64, rows: &[(i64, f64, f64)]) -> String {
    let title = match set {
        "pos" => "positive weight".to_string(),
        "neg" => "negative weight".to_string(),
        "zero" => "zero weight".to_string(),
        g => format!("group {g}"),
    };
    let mut out = format!(
        "priority rank minimum: {}\npriority rank maximum: {}\n{title} layers\n\n\"bin\" \"coverage_level\" \"count\"\n",
        g6(lo),
        g6(hi)
    );
    for (bin, level, count) in rows {
        out += &format!("{bin} {level:.4} {}\n", g(*count));
    }
    out
}

/// rankmap.tif: float32, NaN as Zonation's no-data -inf, LZW with the floating-point predictor, tiled by 512 when
/// the grid has that many cells, BigTIFF when needed. GDAL writes it; without GDAL the file is Terraflux's own
/// (deflated).
pub fn write_rankmap(path: &Path, grid: &Grid, rank: &[f64]) -> Result<()> {
    let data: Vec<f64> = rank.iter().map(|v| if v.is_nan() { f64::NEG_INFINITY } else { *v }).collect();
    write_tif(path, grid, &data, DataType::F32, Some(f64::NEG_INFINITY))
}

/// A GeoTIFF as the library's write_raster writes it: LZW with the predictor of the type (3 for floats, 2 for
/// integers), tiled by 512 when the grid has that many cells, BigTIFF when needed.
pub fn write_tif(path: &Path, grid: &Grid, data: &[f64], dtype: DataType, nodata: Option<f64>) -> Result<()> {
    let err = |e: String| Error::msg(format!("{}: {e}", path.display()));
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| err(e.to_string()))?;
    }
    let data: Vec<f64> =
        if dtype == DataType::F32 { data.iter().map(|v| f64::from(*v as f32)).collect() } else { data.to_vec() };
    let raster = Raster::single(grid.clone(), data, dtype, nodata);
    let plain = path.with_extension("tmp.tif");
    terraflux_raster::tiff::write(&raster, &plain, &Default::default()).map_err(|e| err(e.to_string()))?;
    let (ot, predictor) = match dtype {
        DataType::U8 => ("Byte", "2"),
        DataType::I32 => ("Int32", "2"),
        _ => ("Float32", "3"),
    };
    let mut args: Vec<String> =
        ["-ot", ot, "-co", "COMPRESS=LZW", "-co", "BIGTIFF=IF_SAFER"].map(String::from).to_vec();
    args.extend(["-co".to_string(), format!("PREDICTOR={predictor}")]);
    if grid.height * grid.width >= 512 * 512 {
        args.extend(["-co", "TILED=YES", "-co", "BLOCKXSIZE=512", "-co", "BLOCKYSIZE=512"].map(String::from));
    }
    match terraflux_gdal::translate(&plain, path, &args) {
        Ok(()) => std::fs::remove_file(&plain).map_err(|e| err(e.to_string())),
        Err(_) => std::fs::rename(&plain, path).map_err(|e| err(e.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn g6_is_c_printf() {
        // printf("%g") of these, from C
        for (x, s) in [
            (0.0, "0"),
            (1.0, "1"),
            (100000.0, "100000"),
            (999999.5, "1e+06"),
            (1234567.0, "1.23457e+06"),
            (0.0001, "0.0001"),
            (0.00001234, "1.234e-05"),
            (0.5, "0.5"),
            (-2.5, "-2.5"),
            (123456.4, "123456"),
            (0.1 + 0.2, "0.3"),
            (1e100, "1e+100"),
            (3.140_625, "3.14062"),
            (2.000_004_9, "2"),
            (540.941, "540.941"),
            (0.0101520, "0.010152"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
            (f64::NAN, "nan"),
        ] {
            assert_eq!(g6(x), s, "{x}");
        }
        assert_eq!(g(1423.0), "1423.0");
        assert_eq!(g(0.0), "0.0");
        assert_eq!(g(2.5), "2.5");
        assert_eq!(g(1e10), "1e+10");
        assert_eq!(g(f64::NAN), "nan");
    }

    /// A short or long value of a tag of the first image of a little-endian TIFF or BigTIFF.
    fn tag(path: &Path, tag: u16) -> Option<u64> {
        let b = std::fs::read(path).unwrap();
        let u = |at: usize, n: usize| (0..n).fold(0u64, |s, k| s | u64::from(b[at + k]) << (8 * k));
        let big = u(2, 2) == 43;
        let (ifd, count_len, entry, val) =
            if big { (u(8, 8) as usize, 8, 20, 12) } else { (u(4, 4) as usize, 2, 12, 8) };
        (0..u(ifd, count_len) as usize).find_map(|i| {
            let e = ifd + count_len + i * entry;
            (u(e, 2) == u64::from(tag)).then(|| u(e + val, if u(e + 2, 2) == 3 { 2 } else { 4 }))
        })
    }

    #[test]
    fn rankmap_is_tiled_when_large() {
        if terraflux_gdal::gdal().is_err() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("tf_rankmap_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (w, h, tiled) in [(600, 520, true), (40, 30, false)] {
            let rank: Vec<f64> =
                (0..w * h).map(|i| if i % 7 == 0 { f64::NAN } else { i as f64 / (w * h) as f64 }).collect();
            let path = dir.join(format!("r{w}.tif"));
            write_rankmap(&path, &Grid::new(w, h), &rank).unwrap();
            assert_eq!((tag(&path, 259), tag(&path, 317)), (Some(5), Some(3)));
            assert_eq!(tag(&path, 322), tiled.then_some(512));
            assert!(!path.with_extension("tmp.tif").exists());
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn fixed_is_python_f() {
        assert_eq!(fixed(0.5, 4), "0.5000");
        assert_eq!(fixed(0.00070, 5), "0.00070");
        assert_eq!(fixed(-1e-9, 4), "-0.0000");
        assert_eq!(fixed(f32::INFINITY, 4), "inf");
        assert_eq!(fixed(f32::NEG_INFINITY, 4), "-inf");
        assert_eq!(fixed(f32::NAN, 4), "nan");
        // the float32 widened: 0.1f32 is 0.100000001490116...
        assert_eq!(fixed(0.1, 10), "0.1000000015");
        // exact halves go to even, as C does
        assert_eq!(fixed(0.125, 2), "0.12");
        assert_eq!(fixed(0.375, 2), "0.38");
    }

    #[test]
    fn tables_of_two_layers() {
        let rank = [0.0f32, 0.5, 1.0];
        let levels = [1.0f32, 1.0, 0.5, 0.25, 0.0, 0.0];
        let cost = [0.0f64, 1.0, 2.0];
        let headers = ["0".to_string(), "1".to_string()];
        let c = Curves {
            rank: &rank,
            levels: &levels,
            cost: &cost,
            weights: &[1.0, 1.0],
            weights_on: false,
            cells: 4,
            groups: Some(&[0, 0]),
            headers: &headers,
        };
        assert_eq!(
            c.feature_curves(),
            "\"rank\" 0 1 \n0.00000 1.0000 1.0000 \n0.50000 0.5000 0.2500 \n1.00000 0.0000 0.0000 \n"
        );
        assert_eq!(
            c.summary_curves(),
            "\"rank\" \"remaining_area\" \"remaining_cost\" \"mean\" \"max\" \"min\" \"mean_positive\" \
             \"weighted_mean_positive\" \"max_positive\" \"min_positive\"\n\
             0.00000 4 0.0 1.0000 1.0000 1.0000 1.0000 1.0000 1.0000 1.0000\n\
             0.50000 2 1.0 0.3750 0.5000 0.2500 0.3750 0.3750 0.5000 0.2500\n\
             1.00000 0 2.0 0.0000 0.0000 0.0000 0.0000 0.0000 0.0000 0.0000\n"
        );
        assert_eq!(
            c.density_curves(),
            "\"rank\" \"feature_density_pos\" \"weighted_feature_density_pos\" \n\
             0.00000 0.0000 0.0000\n0.50000 1.2500 1.2500\n1.00000 0.7500 0.7500\n"
        );
        assert_eq!(
            c.group_curves().unwrap(),
            "\"rank\" \"mean.0\" \"weighted_mean.0\" \"min.0\" \n\
             0.00000 1.0000 1.0000 1.0000 \n0.50000 0.3750 0.3750 0.2500 \n1.00000 0.0000 0.0000 0.0000 \n"
        );
    }
}
