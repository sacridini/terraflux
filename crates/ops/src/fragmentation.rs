//! Fragmentation from landschaft's port of GuidosToolbox: the foreground's density in a moving window (FAD,
//! `fragmentation.fad`, and at several windows, `fragmentation.fad_multiscale`) and the fragmentation at a fixed
//! observation scale (`fragmentation.fos`), the same bits as GTB's; contagion and clustering in a moving window,
//! the spatial entropy, the landscape mosaic and GraySpatCon's metrics of a % cover.
//!
//! The maps give a % (0-100) on the foreground and GTB's codes elsewhere: 101 background, 102 missing, 105 and
//! 106 special backgrounds (the second not fragmenting).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::landscape::{foreground_param, gtb_coded, map_of};
use crate::{Args, Ctx, Error, Key, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Fad));
    r.add(Arc::new(FadMultiscale));
    r.add(Arc::new(Fos));
    r.add(Arc::new(Contagion));
    r.add(Arc::new(Clustering));
    r.add(Arc::new(Entropy));
    r.add(Arc::new(EntropyMap));
    r.add(Arc::new(LandscapeMosaic));
    r.add(Arc::new(GrayConvolution));
}

/// The columns of the shares of FAD's classes, most connected first, as landschaft's tables.
const SHARES: [&str; 6] = ["intact", "interior", "dominant", "transitional", "patchy", "rare"];

/// GTB's observation scales of FAD-MS.
const SCALES: [i32; 5] = [7, 13, 27, 81, 243];

fn window(args: &Args, default: i32) -> Result<i32> {
    let w: i32 = args.param("window", default)?;
    if w < 3 || w % 2 == 0 {
        return Err(Error::msg(format!(
            "node {:?}: the window is an odd number of cells, 3 or more (got {w})",
            args.node
        )));
    }
    Ok(w)
}

struct Fad;

impl Operator for Fad {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "fragmentation.fad",
                "Forest Area Density, as GuidosToolbox computes it: the share (%) of foreground in a square window \
                 around each foreground cell, and its classes: rare (<10%), patchy, transitional, dominant, interior \
                 (90-100%) and intact (100%). The foreground is the classes given, or the map is coded 0 missing, 1 \
                 background, 2 foreground, 3 and 4 special backgrounds (the second not fragmenting).",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the density (%), GTB's codes off the foreground"))
            .output(PortSpec::new("classes", Type::Map, "the classes 1 (rare) to 6 (intact), GTB's codes elsewhere"))
            .output(PortSpec::new(
                "stats",
                Type::Table,
                "the window, the % of the foreground in each class and the average density",
            )),
        )
        .param(PortSpec::optional("window", Type::Integer, "the window's side in cells, odd (default 7)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let w = window(args, 7)?;
        let input = gtb_coded(args, map, true)?;
        let (ny, nx) = map.shape();
        let out = ffi::fad(&input, ny as i64, nx as i64, w)?;
        let fg = out.foreground as f64;
        let share = |n: i64| if fg > 0.0 { n as f64 / fg * 100.0 } else { 0.0 };
        let mut row = vec![f64::from(w)];
        // the counts go from rare (class 1) to intact (6)
        row.extend(out.class_counts.iter().rev().map(|&n| share(n)));
        row.push(if fg > 0.0 { out.density_sum as f64 / fg } else { 0.0 });
        let mut stats = Table::new(columns(&[]));
        stats.rows.push(row);
        Ok(outputs([
            ("map", map_of(map, out.density)),
            ("classes", map_of(map, out.classes)),
            ("stats", Value::Table(stats)),
        ]))
    }
}

/// The columns of FAD's tables: the window, the shares of the classes, the average density and `more`.
fn columns(more: &[&str]) -> Vec<String> {
    let mut c = vec!["window".to_string()];
    c.extend(SHARES.iter().map(|s| format!("{s}_percent")));
    c.push("average".into());
    c.extend(more.iter().map(|s| s.to_string()));
    c
}

struct FadMultiscale;

impl Operator for FadMultiscale {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "fragmentation.fad_multiscale",
                "Forest Area Density at several observation scales, as GuidosToolbox's FAD-MS: the density (%) in \
                 windows of 7, 13, 27, 81 and 243 cells (or those given) and their average, with its classes. The \
                 foreground is the classes given, or the map is coded 0 missing, 1 background, 2 foreground, 3 and \
                 4 special backgrounds.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the average density over the windows (%), GTB's codes elsewhere"))
            .output(PortSpec::new("classes", Type::Map, "the average's classes 1 (rare) to 6 (intact)"))
            .output(PortSpec::new("scales", Type::Stack, "the density at each window, by its side"))
            .output(PortSpec::new(
                "stats",
                Type::Table,
                "per window (0 for the average): the % of the foreground in each class, the average density and \
                 AVCON (the average over the area with data)",
            )),
        )
        .param(PortSpec::optional(
            "windows",
            Type::Any,
            "the windows' sides in cells, odd (default 7 13 27 81 243)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let windows: Vec<i32> = args.param("windows", SCALES.to_vec())?;
        let input = gtb_coded(args, map, true)?;
        let (ny, nx) = map.shape();
        let out = ffi::fad_multiscale(&input, ny as i64, nx as i64, &windows)?;
        // GTB's AVCON: the average scaled to the area with data
        let fg = input.iter().filter(|&&v| v == 2).count() as f64;
        let data = input.iter().filter(|&&v| v != 0).count() as f64;
        let factor = if data > 0.0 { fg / data } else { 0.0 };
        let mut stats = Table::new(columns(&["avcon"]));
        for r in out.stats.chunks(8) {
            let mut row = r.to_vec();
            row.push(r[7] * factor);
            stats.rows.push(row);
        }
        let cells = ny * nx;
        let scales = Stack {
            grid: map.grid.clone(),
            keys: windows.iter().map(|w| Key::Name(w.to_string())).collect(),
            layers: out
                .maps
                .chunks(cells.max(1))
                .map(|m| Arc::new(m.iter().map(|&v| f64::from(v)).collect()))
                .collect(),
        };
        Ok(outputs([
            ("map", map_of(map, out.multiscale)),
            ("classes", map_of(map, out.classes)),
            ("scales", Value::Stack(scales)),
            ("stats", Value::Table(stats)),
        ]))
    }
}

/// What a cell of FOS's map is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Cell {
    Missing,
    Background,
    Foreground,
    /// A special background (GTB's 3, or 103 in a % cover).
    Special,
    /// A special background that does not fragment (4, or 104): missing for the density.
    NonFragmenting,
}

struct Fos;

impl Operator for Fos {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "fragmentation.fos",
                "Fragmentation at a Fixed Observation Scale, as GuidosToolbox's FOS: for each foreground cell the \
                 connectivity (%) of the foreground in a square window, by FAD (the foreground's share), FED (its \
                 edges: GraySpatCon's clustering) or FAC (foreground-foreground pairs, binary maps only), reported \
                 in 5 or 6 classes. The map is binary (the foreground's classes given, or coded 0 missing, 1 \
                 background, 2 foreground, 3 and 4 special backgrounds) or a % cover (0-100, 255 missing, 103 and 104 \
                 special backgrounds) whose foreground is the cover from a threshold up.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the connectivity (%), GTB's codes off the foreground"))
            .output(PortSpec::new("stats", Type::Table, "per class (1 rare .. 6 intact): the % of the foreground"))
            .output(PortSpec::new(
                "summary",
                Type::Table,
                "GTB's report: the average connectivity, AVCON, the foreground and its patches, ECA and coherence",
            )),
        )
        .param(PortSpec::optional("window", Type::Integer, "the window's side in cells, odd (default 27)"))
        .param(PortSpec::optional("method", Type::Text, "FAD (default), FED or FAC"))
        .param(PortSpec::optional("classes", Type::Integer, "5 or 6 (default: intact apart from interior)"))
        .param(PortSpec::optional("connectivity", Type::Integer, "of the patches: 8 (default) or 4"))
        .param(PortSpec::optional("grayscale", Type::Bool, "a % cover (default: when its largest value passes 4)"))
        .param(PortSpec::optional(
            "gray_threshold",
            Type::Integer,
            "a % cover's foreground: from this up (default 30)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let map = args.map("map")?;
        let w = window(args, 27)?;
        let method: String = args.param::<String>("method", "FAD".into())?.to_uppercase();
        let classes: i32 = args.param("classes", 6)?;
        let connectivity: i32 = args.param("connectivity", 8)?;
        let threshold: i64 = args.param("gray_threshold", 30)?;
        if !matches!(method.as_str(), "FAD" | "FED" | "FAC") {
            return Err(Error::msg(format!("node {node:?}: the method is FAD, FED or FAC, not {method:?}")));
        }
        if classes != 5 && classes != 6 {
            return Err(Error::msg(format!("node {node:?}: the classes are 5 or 6, not {classes}")));
        }
        if connectivity != 4 && connectivity != 8 {
            return Err(Error::msg(format!("node {node:?}: the connectivity is 4 or 8, not {connectivity}")));
        }
        let (ny, nx) = map.shape();
        if w as usize >= ny.min(nx) {
            return Err(Error::msg(format!(
                "node {node:?}: the window ({w}) must be smaller than the map ({ny} x {nx})"
            )));
        }
        let given = !matches!(args.params.get("foreground"), None | Some(serde_json::Value::Null));
        let top = map.data.iter().filter(|v| !v.is_nan()).fold(f64::NEG_INFINITY, |a, &b| a.max(b));
        let grayscale = !given && args.opt_param::<bool>("grayscale")?.unwrap_or(top > 4.0);

        // the map in GTB's codes, and what each cell is
        let (arr, cells): (Vec<u8>, Vec<Cell>) = if grayscale {
            if method == "FAC" {
                return Err(Error::msg(format!("node {node:?}: FAC is for binary maps only (as in GTB)")));
            }
            if !(1..=100).contains(&threshold) {
                return Err(Error::msg(format!(
                    "node {node:?}: the gray threshold is a cover of 1-100, not {threshold}"
                )));
            }
            let arr: Vec<u8> = map
                .data
                .iter()
                .map(|&v| match v {
                    v if v.is_nan() => Ok(255),
                    v if v.fract() == 0.0 && ((0.0..=100.0).contains(&v) || v == 103.0 || v == 104.0 || v == 255.0) => {
                        Ok(v as u8)
                    }
                    v => Err(Error::msg(format!(
                        "node {node:?}: a % cover holds 0-100, 103 and 104 (special backgrounds) and 255 (missing), \
                         not {v}"
                    ))),
                })
                .collect::<Result<_>>()?;
            let cells: Vec<Cell> = arr
                .iter()
                .map(|&v| match v {
                    255 => Cell::Missing,
                    103 => Cell::Special,
                    104 => Cell::NonFragmenting,
                    v if i64::from(v) < threshold => Cell::Background,
                    _ => Cell::Foreground,
                })
                .collect();
            if !cells.contains(&Cell::Background) {
                return Err(Error::msg(format!("node {node:?}: the map has no background (cover below {threshold})")));
            }
            (arr, cells)
        } else {
            let arr = gtb_coded(args, map, true)?;
            let cells = arr
                .iter()
                .map(|&v| match v {
                    0 => Cell::Missing,
                    1 => Cell::Background,
                    2 => Cell::Foreground,
                    3 => Cell::Special,
                    _ => Cell::NonFragmenting,
                })
                .collect();
            (arr, cells)
        };
        let reporting = cells.iter().filter(|&&c| c != Cell::Missing).count();
        let fg_area = cells.iter().filter(|&&c| c == Cell::Foreground).count();
        if fg_area == 0 {
            return Err(Error::msg(format!("node {node:?}: the map has no foreground")));
        }

        // the patches of the foreground (Accounting labels them)
        let binary: Vec<u8> = cells
            .iter()
            .map(|c| match c {
                Cell::Foreground => 2,
                Cell::Missing => 0,
                _ => 1,
            })
            .collect();
        let patches = ffi::accounting(&binary, ny as i64, nx as i64, connectivity, &[1], false)?.object_area;
        let n_patches = patches.len();
        let eca = patches.iter().map(|&a| (a as f64) * (a as f64)).sum::<f64>().sqrt();
        let eca_max = patches.iter().sum::<i64>() as f64;
        // GTB: 100.0/n_ruarea*farea in single precision
        let pct = 100f32 / reporting as f32;
        let (fg_pct, aps) = if grayscale {
            (f64::from(pct * fg_area as f32), f64::from(fg_area as f32 / n_patches as f32))
        } else {
            (f64::from(pct) * fg_area as f64, fg_area as f64 / n_patches as f64)
        };

        // the connectivity
        let mut im = if grayscale {
            let work: Vec<u8> = arr
                .iter()
                .map(|&v| match v {
                    103 => 0,
                    104 => 255,
                    v => v,
                })
                .collect();
            ffi::grayspatcon_fos(&work, ny as i64, nx as i64, if method == "FAD" { 1 } else { 52 }, w)?
        } else if method == "FED" {
            // GraySpatCon's clustering of 0 background, 100 foreground, 255 missing
            let gsc: Vec<u8> = cells
                .iter()
                .map(|c| match c {
                    Cell::Foreground => 100,
                    Cell::Missing | Cell::NonFragmenting => 255,
                    _ => 0,
                })
                .collect();
            ffi::grayspatcon_fos(&gsc, ny as i64, nx as i64, 52, w)?
        } else {
            // the non-fragmenting background is missing for the density
            let work: Vec<u8> = arr.iter().map(|&v| if v == 4 { 0 } else { v }).collect();
            if method == "FAD" {
                ffi::fad_density(&work, ny as i64, nx as i64, w, false)?
            } else {
                ffi::clustering_fac(&work, ny as i64, nx as i64, w, false)?
            }
        };
        let sum: u64 = im.iter().zip(&cells).filter(|(_, c)| **c == Cell::Foreground).map(|(&v, _)| u64::from(v)).sum();
        let average = sum as f64 / fg_area as f64;
        let avcon = average * fg_area as f64 / reporting as f64;
        for (v, c) in im.iter_mut().zip(&cells) {
            *v = match c {
                Cell::Foreground => *v,
                Cell::Background => 101,
                Cell::Missing => 102,
                Cell::Special => 105,
                Cell::NonFragmenting => 106,
            };
        }

        let share = |f: &dyn Fn(u8) -> bool| im.iter().filter(|&&v| f(v)).count() as f64 / fg_area as f64 * 100.0;
        let mut shares = vec![
            share(&|v| v < 10),
            share(&|v| (10..40).contains(&v)),
            share(&|v| (40..60).contains(&v)),
            share(&|v| (60..90).contains(&v)),
        ];
        if classes == 6 {
            shares.extend([share(&|v| (90..100).contains(&v)), share(&|v| v == 100)]);
        } else {
            shares.push(share(&|v| (90..=100).contains(&v)));
        }
        let mut stats = Table::new(vec!["class".into(), "percent".into()]);
        stats.rows = shares.into_iter().enumerate().map(|(i, p)| vec![(i + 1) as f64, p]).collect();
        let mut summary = Table::new(
            [
                "average",
                "avcon",
                "foreground",
                "reporting_area",
                "foreground_percent",
                "patches",
                "average_patch_size",
                "eca",
                "coherence",
                "coherence_ru",
            ]
            .map(String::from)
            .to_vec(),
        );
        summary.rows.push(vec![
            average,
            avcon,
            fg_area as f64,
            reporting as f64,
            fg_pct,
            n_patches as f64,
            aps,
            eca,
            if eca_max > 0.0 { eca / eca_max * 100.0 } else { f64::NAN },
            eca / reporting as f64 * 100.0,
        ]);
        Ok(outputs([("map", map_of(map, im)), ("stats", Value::Table(stats)), ("summary", Value::Table(summary))]))
    }
}

/// A kernel of GTB over a window, from a map in its codes to a map of bytes.
type Windowed = fn(&[u8], i64, i64, i32) -> std::result::Result<Vec<u8>, terraflux_ls_sys::Error>;

/// The map of a kernel over a window, the map coded as GTB's.
fn windowed(args: &Args, default: i32, f: Windowed) -> Result<Outputs> {
    let map = args.map("map")?;
    let w = window(args, default)?;
    let input = gtb_coded(args, map, false)?;
    let (ny, nx) = map.shape();
    Ok(outputs([("map", map_of(map, f(&input, ny as i64, nx as i64, w)?))]))
}

struct Contagion;

impl Operator for Contagion {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "fragmentation.contagion",
                "Contagion (Pff) in a moving window, as GuidosToolbox (SpatCon's rule 77): the share (%) of the \
                 foreground's edges in the window that join foreground to foreground; 101 background, 102 missing. \
                 The foreground is the classes given, or the map is coded 0 missing, 1 background, 2 foreground.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the contagion (%), GTB's codes off the foreground")),
        )
        .param(PortSpec::optional("window", Type::Integer, "the window's side in cells, odd (default 7)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        windowed(args, 7, ffi::contagion_pff)
    }
}

struct Clustering;

impl Operator for Clustering {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "fragmentation.clustering",
                "Foreground area clustering (FAC) in a moving window, as GuidosToolbox (SpatCon's rule 76): the \
                 share (%) of the edges in the window with data that join foreground to foreground; 101 background, \
                 102 missing. The foreground is the classes given, or the map is coded 0 missing, 1 background, 2 \
                 foreground.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the clustering (%), GTB's codes off the foreground")),
        )
        .param(PortSpec::optional("window", Type::Integer, "the window's side in cells, odd (default 7)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        windowed(args, 7, |m, ny, nx, w| ffi::clustering_fac(m, ny, nx, w, true))
    }
}

struct Entropy;

impl Operator for Entropy {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "fragmentation.entropy",
                "Spatial joint entropy of the whole map, as GuidosToolbox: how mixed the pairs of neighbouring \
                 cells are, in % of the most they could be (0 one class, 100 as mixed as can be). The foreground is \
                 the classes given, or the map is coded 0 missing, 1 background, 2 foreground.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("index", Type::Number, "the entropy (%)"))
            .output(PortSpec::new(
                "pairs",
                Type::Table,
                "the code of each kind of pair of neighbours and its count",
            )),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let input = gtb_coded(args, map, false)?;
        let (ny, nx) = map.shape();
        let out = ffi::spatial_entropy(&input, ny as i64, nx as i64)?;
        let mut pairs = Table::new(vec!["code".into(), "count".into()]);
        pairs.rows =
            out.histogram.iter().enumerate().filter(|(_, n)| **n > 0).map(|(k, &n)| vec![k as f64, n as f64]).collect();
        Ok(outputs([("index", Value::Number(out.index)), ("pairs", Value::Table(pairs))]))
    }
}

struct EntropyMap;

impl Operator for EntropyMap {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "fragmentation.entropy_map",
                "Spatial joint entropy in a moving window, as GuidosToolbox's batch: for each cell the entropy (%) \
                 of the pairs of neighbours in the window around it. The foreground is the classes given, or the map \
                 is coded 0 missing, 1 background, 2 foreground.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map"))
            .output(PortSpec::new("map", Type::Map, "the entropy (%) around each cell")),
        )
        .param(PortSpec::optional(
            "window",
            Type::Integer,
            "the window's side in cells, 3 or more (default 50)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let w: i32 = args.param("window", 50)?;
        let input = gtb_coded(args, map, false)?;
        let (ny, nx) = map.shape();
        let out = ffi::spatial_entropy_map(&input, ny as i64, nx as i64, w)?;
        Ok(outputs([("map", map_of(map, out))]))
    }
}

/// The settings of the classes of a land cover that are each of the landscape mosaic's three, and its codes.
const MOSAIC_CLASSES: [(&str, u8); 3] = [("agriculture", 1), ("natural", 2), ("developed", 3)];

struct LandscapeMosaic;

impl Operator for LandscapeMosaic {
    fn signature(&self) -> OpSignature {
        let mut sig = OpSignature::new(
            "fragmentation.landscape_mosaic",
            "The landscape mosaic, as GuidosToolbox (SpatCon's rule 7): for each cell the mix of agriculture, \
             natural land and developed land in a moving window, in 19 classes (dominant, pairs, mosaics, pure) or \
             103. The map is coded 0 missing, 1 agriculture, 2 natural, 3 developed, or the classes of each are \
             given (the others missing).",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the land cover"))
        .output(PortSpec::new("map", Type::Map, "the mosaic's classes (0 missing)"))
        .output(PortSpec::new(
            "stats",
            Type::Table,
            "per class of 103: its cells, % of the cells with data and its class of 19",
        ));
        for (name, _) in MOSAIC_CLASSES {
            sig = sig.param(PortSpec::optional(name, Type::Any, "the land cover's classes of it (a class or a list)"));
        }
        sig.param(PortSpec::optional("window", Type::Integer, "the window's side in cells, odd (default 27)"))
            .param(PortSpec::optional("classes", Type::Integer, "19 (default) or 103"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let w = window(args, 27)?;
        let classes: i32 = args.param("classes", 19)?;
        let mut given: Vec<(Vec<f64>, u8)> = Vec::new();
        for (name, code) in MOSAIC_CLASSES {
            if let Some(c) = classes_param(args, name)? {
                given.push((c, code));
            }
        }
        let input: Vec<u8> = if given.is_empty() {
            map.data
                .iter()
                .map(|&v| match v {
                    v if v.is_nan() => Ok(0),
                    v if v.fract() == 0.0 && (0.0..=3.0).contains(&v) => Ok(v as u8),
                    v => Err(Error::msg(format!(
                        "node {:?}: the map has a {v}: give the classes of agriculture, natural and developed land, \
                         or code it 0 missing, 1 agriculture, 2 natural, 3 developed",
                        args.node
                    ))),
                })
                .collect::<Result<_>>()?
        } else {
            map.data.iter().map(|v| given.iter().find(|(c, _)| c.contains(v)).map_or(0, |(_, k)| *k)).collect()
        };
        let (ny, nx) = map.shape();
        let out = ffi::landscape_mosaic(&input, ny as i64, nx as i64, w, classes)?;
        let lm103 =
            if classes == 103 { out.clone() } else { ffi::landscape_mosaic(&input, ny as i64, nx as i64, w, 103)? };
        let s = ffi::landscape_mosaic_stats(&lm103, ny as i64, nx as i64)?;
        let mut stats = Table::new(["class", "cells", "percent", "class_19"].map(String::from).to_vec());
        for i in 0..s.codes.len() {
            let code = s.codes[i];
            let to19 = crate::gtb_palettes::RECODE_103_TO_19[code.clamp(0, 255) as usize];
            stats.rows.push(vec![f64::from(code), s.counts[i] as f64, s.percentages[i], f64::from(to19)]);
        }
        Ok(outputs([("map", map_of(map, out)), ("stats", Value::Table(stats))]))
    }
}

/// A setting of classes: a class or a list of them.
pub(crate) fn classes_param(args: &Args, name: &str) -> Result<Option<Vec<f64>>> {
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => Ok(Some(vec![n.as_f64().unwrap()])),
        Some(v) => serde_json::from_value(v.clone())
            .map(Some)
            .map_err(|e| Error::msg(format!("node {:?}: {name}: {e}", args.node))),
    }
}

/// GraySpatCon's metrics by the names it takes.
const GRAY_METRICS: [&str; 10] = [
    "mean",
    "median",
    "majority",
    "min",
    "max",
    "range",
    "stddev",
    "shannon_diversity",
    "shannon_evenness",
    "diagonal_contagion",
];

/// GraySpatCon's value of the cells it gives none for.
pub const GRAY_MISSING: f32 = -0.01;

struct GrayConvolution;

impl Operator for GrayConvolution {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "fragmentation.gray_convolution",
            "A moving window over a % cover (0-100; above 100 or without a value missing), as GuidosToolbox's \
             GraySpatCon: the mean, median, majority, min, max, range, standard deviation (stddev), Shannon's \
             diversity or evenness, or diagonal contagion of the cover around each cell.",
            Access::Global,
        )
        .input(PortSpec::new("map", Type::Map, "the cover (%)"))
        .output(PortSpec::new("map", Type::Map, "the metric around each cell"))
        .param(PortSpec::optional("metric", Type::Text, &format!("{} (default mean)", GRAY_METRICS.join(", "))))
        .param(PortSpec::optional("window", Type::Integer, "the window's side in cells, odd (default 7)"))
        .param(PortSpec::optional(
            "mask_missing",
            Type::Bool,
            "no value where the cover is missing (default true)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let w = window(args, 7)?;
        let metric: String = args.param::<String>("metric", "mean".into())?.to_lowercase();
        let metric = match metric.as_str() {
            "average" => "mean",
            "mode" => "majority",
            "minimum" => "min",
            "maximum" => "max",
            "std" | "standard_deviation" => "stddev",
            m if GRAY_METRICS.contains(&m) => m,
            m => {
                return Err(Error::msg(format!(
                    "node {:?}: the metric is one of {}, not {m:?}",
                    args.node,
                    GRAY_METRICS.join(", ")
                )));
            }
        };
        let mask: bool = args.param("mask_missing", true)?;
        let input: Vec<u8> = map
            .data
            .iter()
            .map(|&v| match v {
                v if v.is_nan() => Ok(255),
                v if v.fract() == 0.0 && (0.0..=255.0).contains(&v) => Ok(v as u8),
                v => Err(Error::msg(format!("node {:?}: a cover holds whole values 0-100, not {v}", args.node))),
            })
            .collect::<Result<_>>()?;
        let (ny, nx) = map.shape();
        let out = ffi::gray_convolution(&input, ny as i64, nx as i64, metric, w, mask)?;
        // GraySpatCon's missing: no value
        let data = out.into_iter().map(|v| if v == GRAY_MISSING { f64::NAN } else { f64::from(v) }).collect();
        Ok(outputs([("map", Value::Map(crate::Map::new(map.grid.clone(), data)))]))
    }
}
