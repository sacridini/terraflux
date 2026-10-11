//! Fragmentation from landschaft's port of GuidosToolbox: the foreground's density in a moving window (FAD,
//! `fragmentation.fad`, and at several windows, `fragmentation.fad_multiscale`) and the fragmentation at a fixed
//! observation scale (`fragmentation.fos`), the same bits as GTB's.
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
