//! Change between two maps from landschaft's port of GuidosToolbox: the simple change of the foreground
//! (`change.simple`), its morphological change, which tells the change of the core from the spurious one at the
//! edges (`change.morphological`), the change of the fragmentation measured by FOS (`change.fos`) and by FAD at
//! several scales (`change.fad`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::landscape::{foreground_param, g6, gtb_coded, map_of};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Simple));
    r.add(Arc::new(Morphological));
    r.add(Arc::new(Fos));
    r.add(Arc::new(Fad));
}

/// The two maps of a change, of the same grid.
fn both<'a>(args: &'a Args) -> Result<(&'a Map, &'a Map)> {
    let (a, b) = (args.map("before")?, args.map("after")?);
    if a.shape() != b.shape() {
        return Err(Error::msg(format!(
            "node {:?}: the maps differ in size ({:?} and {:?})",
            args.node,
            a.shape(),
            b.shape()
        )));
    }
    Ok((a, b))
}

/// The signature of a change of the foreground between two maps.
fn signature(name: &str, doc: &str, stats: &str) -> OpSignature {
    foreground_param(
        OpSignature::new(name, doc, Access::Global)
            .input(PortSpec::new("before", Type::Map, "the map at the first date"))
            .input(PortSpec::new("after", Type::Map, "the map at the second date"))
            .output(PortSpec::new("map", Type::Map, "the change in GTB's codes"))
            .output(PortSpec::new("stats", Type::Table, stats)),
    )
}

/// A table of one row.
fn row(columns: &[&str], values: Vec<f64>) -> Value {
    Value::Table(Table { columns: columns.iter().map(|c| c.to_string()).collect(), rows: vec![values] })
}

struct Simple;

impl Operator for Simple {
    fn signature(&self) -> OpSignature {
        signature(
            "change.simple",
            "The change of the foreground between two dates, as GuidosToolbox: 11 stable background, 12 gain, 21 \
             loss, 22 stable foreground, 255 missing at either date. The foreground is the classes given, or the \
             maps are coded 0 missing, 1 background, 2 foreground.",
            "the cells of stable background, gain, loss, stable foreground and missing, and those with data",
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (a, b) = both(args)?;
        let (ia, ib) = (gtb_coded(args, a, false)?, gtb_coded(args, b, false)?);
        let (ny, nx) = a.shape();
        let out = ffi::simple_change(&ia, &ib, ny as i64, nx as i64)?;
        let stats = row(
            &["stable_background", "gain", "loss", "stable_foreground", "missing", "data"],
            out.counts.iter().map(|&n| n as f64).collect(),
        );
        Ok(outputs([("map", map_of(a, out.map)), ("stats", stats)]))
    }
}

struct Morphological;

impl Operator for Morphological {
    fn signature(&self) -> OpSignature {
        signature(
            "change.morphological",
            "Morphological change detection, as GuidosToolbox: the change of the foreground between two dates \
             told apart from the spurious change at its edges (a shift of a cell, a misregistration): 11 stable \
             background, 12 gain, 21 loss, 22 stable foreground, 176 spurious, 255 missing. The foreground is the \
             classes given, or the maps are coded 0 missing, 1 background, 2 foreground.",
            "the foreground at each date, the net change, the cells of each kind of change, and the elasticity \
             (the rate of the core's change over the foreground's)",
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (a, b) = both(args)?;
        let (ia, ib) = (gtb_coded(args, a, false)?, gtb_coded(args, b, false)?);
        let (ny, nx) = a.shape();
        let out = ffi::morphological_change(&ia, &ib, ny as i64, nx as i64)?;
        let mut values: Vec<f64> = out.counts.iter().map(|&n| n as f64).collect();
        values.push(out.values[0]);
        let stats = row(
            &[
                "foreground_before",
                "foreground_after",
                "net_change",
                "stable_background",
                "stable_foreground",
                "gain",
                "loss",
                "spurious",
                "missing",
                "elasticity",
            ],
            values,
        );
        Ok(outputs([("map", map_of(a, out.map)), ("stats", stats)]))
    }
}

/// FOS's classes, rare first.
pub const FOS_CLASSES: [&str; 6] = ["rare", "patchy", "transitional", "dominant", "interior", "intact"];

struct Fos;

impl Operator for Fos {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "change.fos",
            "The change of fragmentation between two FOS maps (fragmentation.fos, or FAD) of the same windows, as \
             GuidosToolbox: on the foreground of both dates the change of its connectivity + 100 (100 none), 250 \
             foreground gained, 251 lost, 252 stable background, 254 missing; and how the foreground moved \
             between the classes rare to intact.",
            Access::Global,
        )
        .input(PortSpec::new("before", Type::Map, "the FOS map at the first date"))
        .input(PortSpec::new("after", Type::Map, "the FOS map at the second date"))
        .output(PortSpec::new("map", Type::Map, "the change in GTB's codes"))
        .output(PortSpec::new(
            "stats",
            Type::Table,
            "the foreground gained and lost, stable background, the common foreground, in the same class and in \
             another, and the % of it whose fragmentation decreased and increased",
        ))
        .output(PortSpec::new(
            "classes",
            Type::Table,
            "the common foreground from each class (rows, rare to intact) to each (columns)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (a, b) = both(args)?;
        let bytes = |m: &Map| -> Result<Vec<u8>> {
            m.data
                .iter()
                .map(|&v| match v {
                    v if v.is_nan() => Ok(102),
                    v if v.fract() == 0.0 && (0.0..=255.0).contains(&v) => Ok(v as u8),
                    v => Err(Error::msg(format!("node {:?}: a FOS map holds GTB's bytes, not {v}", args.node))),
                })
                .collect()
        };
        let (ia, ib) = (bytes(a)?, bytes(b)?);
        let (ny, nx) = a.shape();
        let out = ffi::fos_change(&ia, &ib, ny as i64, nx as i64)?;
        let mut values: Vec<f64> = out.counts.iter().map(|&n| n as f64).collect();
        values.extend(&out.values);
        let stats = row(
            &[
                "gain",
                "loss",
                "stable_background",
                "common",
                "same_class",
                "other_class",
                "decreased_percent",
                "increased_percent",
            ],
            values,
        );
        let mut classes =
            Table::new(std::iter::once("from".to_string()).chain(FOS_CLASSES.map(String::from)).collect());
        classes.rows = out
            .matrix
            .chunks(6)
            .enumerate()
            .map(|(i, r)| std::iter::once((i + 1) as f64).chain(r.iter().map(|&n| n as f64)).collect())
            .collect();
        Ok(outputs([("map", map_of(a, out.map)), ("stats", stats), ("classes", Value::Table(classes))]))
    }
}

/// GTB's classes of FAD's change, background first.
pub const FAD_CHANGE_CLASSES: [&str; 7] =
    ["Background", "Rare", "Patchy", "Transitional", "Dominant", "Interior", "Intact"];

/// The change of FAD at one scale (landschaft's FADChangeScale).
struct FadScale {
    matrix: Vec<i64>,
    percent: Vec<f64>,
    fg_a: i64,
    fg_b: i64,
    forcom: i64,
    same: i64,
    changed: i64,
    decrease: f64,
    increase: f64,
}

struct Fad;

impl Operator for Fad {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "change.fad",
                "The change of fragmentation between two dates measured by FAD at several scales, as GuidosToolbox's \
                 Change -> FAD Multiscale (landschaft's fad_multiscale_change): FAD-MS of both maps, and at every \
                 window and for the average the cells moved between the classes background, rare, patchy, \
                 transitional, dominant, interior and intact; the change of the average FAD (FAD_AV, AVCON).",
                Access::Global,
            )
            .input(PortSpec::new("before", Type::Map, "the map at the first date (FAD's input)"))
            .input(PortSpec::new("after", Type::Map, "the map at the second date"))
            .output(PortSpec::new(
                "matrix",
                Type::Table,
                "per scale (its window, 0 the average) and class at A (0 background .. 6 intact): the cells in each \
                 class at B",
            ))
            .output(PortSpec::new(
                "stats",
                Type::Table,
                "per scale: the foreground at A, at B, in both, in the same class and in another, the % whose \
                 fragmentation decreased and increased, FAD_AV and AVCON at A and B and their changes",
            ))
            .output(PortSpec::new(
                "report",
                Type::Text,
                "the average's report as GTB's FAD_change_mscale.txt",
            )),
        )
        .param(PortSpec::optional(
            "windows",
            Type::Any,
            "the windows' sides in cells, odd (default 7 13 27 81 243)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (a, b) = both(args)?;
        let windows: Vec<i32> = args.param("windows", crate::fragmentation::SCALES.to_vec())?;
        let (ny, nx) = a.shape();
        let n = ny * nx;
        let fa = ffi::fad_multiscale(&gtb_coded(args, a, true)?, ny as i64, nx as i64, &windows)?;
        let fb = ffi::fad_multiscale(&gtb_coded(args, b, true)?, ny as i64, nx as i64, &windows)?;
        if fa.multiscale.contains(&106) != fb.multiscale.contains(&106) {
            return Err(Error::msg(format!(
                "node {:?}: one of the maps has non-fragmenting background (input 4 / FAD 106): a FAD change is \
                 meaningful when both or neither have it",
                args.node
            )));
        }
        let maps = |o: &ffi::FadScalesOut, k: usize| -> Vec<u8> {
            if k < windows.len() { o.maps[k * n..(k + 1) * n].to_vec() } else { o.multiscale.clone() }
        };
        let mut scales = Vec::new();
        let (mut av_a, mut av_b, mut factor) = (Vec::new(), Vec::new(), (0.0, 0.0));
        for k in 0..=windows.len() {
            let c = ffi::fad_change_counts(&maps(&fa, k), &maps(&fb, k), ny as i64, nx as i64)?;
            let [fg_a, fg_b, forcom, sum_a, sum_b, ru_a, ru_b] = c.counts[..] else { unreachable!() };
            av_a.push(sum_a as f64 / fg_a as f64);
            av_b.push(sum_b as f64 / fg_b as f64);
            if k == windows.len() {
                factor = (fg_a as f64 / ru_a as f64, fg_b as f64 / ru_b as f64);
            }
            // the foreground part, its diagonal 0
            let mut flat = vec![0.0f64; 36];
            let mut same = 0;
            for (k, x) in flat.iter_mut().enumerate() {
                let (i, j) = (k / 6, k % 6);
                let v = c.matrix[(i + 1) * 7 + j + 1];
                if i == j {
                    same += v;
                } else {
                    *x = v as f64;
                }
            }
            let changed = numpy_sum(&flat);
            let percent: Vec<f64> = flat.iter().map(|v| v / changed * 100.0).collect();
            // np.triu: the lower triangle 0
            let upper: Vec<f64> = (0..36).map(|k| if k % 6 >= k / 6 { percent[k] } else { 0.0 }).collect();
            let above = numpy_sum(&upper);
            let total = numpy_sum(&percent);
            scales.push(FadScale {
                matrix: c.matrix.to_vec(),
                percent,
                fg_a,
                fg_b,
                forcom,
                same,
                changed: changed as i64,
                decrease: above,
                increase: total - above,
            });
        }
        let con_a: Vec<f64> = av_a.iter().map(|v| v * factor.0).collect();
        let con_b: Vec<f64> = av_b.iter().map(|v| v * factor.1).collect();
        let scale_id = |k: usize| if k < windows.len() { f64::from(windows[k]) } else { 0.0 };
        let mut matrix = Table::new(
            ["scale", "from"]
                .iter()
                .map(|s| s.to_string())
                .chain(FAD_CHANGE_CLASSES.iter().map(|c| c.to_lowercase()))
                .collect(),
        );
        let mut stats = Table::new(
            [
                "scale",
                "foreground_a",
                "foreground_b",
                "common",
                "same_class",
                "other_class",
                "decreased_percent",
                "increased_percent",
                "fad_av_a",
                "fad_av_b",
                "delta_fad_av",
                "avcon_a",
                "avcon_b",
                "delta_avcon",
            ]
            .map(String::from)
            .to_vec(),
        );
        for (k, s) in scales.iter().enumerate() {
            for i in 0..7 {
                let mut row = vec![scale_id(k), i as f64];
                row.extend(s.matrix[i * 7..(i + 1) * 7].iter().map(|&v| v as f64));
                matrix.rows.push(row);
            }
            stats.rows.push(vec![
                scale_id(k),
                s.fg_a as f64,
                s.fg_b as f64,
                s.forcom as f64,
                s.same as f64,
                s.changed as f64,
                s.decrease,
                s.increase,
                av_a[k],
                av_b[k],
                av_b[k] - av_a[k],
                con_a[k],
                con_b[k],
                con_b[k] - con_a[k],
            ]);
        }
        // the report of the average, in the layout of GTB's FAD_change_mscale.txt
        let sc = scales.last().unwrap();
        let label =
            format!("average over {}", windows.iter().map(|w| format!("{w}x{w}")).collect::<Vec<_>>().join("-"));
        let names = FAD_CHANGE_CLASSES;
        let mut lines = vec![
            format!("FAD: Fragmentation class change from A -> B at observation scale: {label} pixels"),
            "=".repeat(142),
            "# pixels A->B  :".to_string()
                + &names.iter().enumerate().map(|(i, c)| format!("{:>18}", format!("B{i}-{c}"))).collect::<String>(),
        ];
        for (i, c) in names.iter().enumerate() {
            lines.push(
                format!("{:<15}:", format!("A{i}-{c}"))
                    + &sc.matrix[i * 7..(i + 1) * 7].iter().map(|v| format!("{v:>18}")).collect::<String>(),
            );
        }
        let ids: Vec<String> = (0..=windows.len())
            .map(|k| if k < windows.len() { windows[k].to_string() } else { "mscale".into() })
            .collect();
        lines.extend([
            "  ".to_string(),
            format!("Foreground cover at time A [pixels]: {}", sc.fg_a),
            format!("Foreground cover at time B [pixels]: {}", sc.fg_b),
            format!("Net foreground cover change (A->B) [pixels]: {}", sc.fg_b - sc.fg_a),
            "=".repeat(89),
            "Observation scale:  ".to_string() + &ids.iter().map(|w| format!("{w:>12}")).collect::<String>(),
            "Diff: FAD_AV(A->B): ".to_string()
                + &av_a.iter().zip(&av_b).map(|(a, b)| format!("{:>12.4}", b - a)).collect::<String>(),
            "Diff: AVCON(A->B):  ".to_string()
                + &con_a.iter().zip(&con_b).map(|(a, b)| format!("{:>12.4}", b - a)).collect::<String>(),
            "  ".to_string(),
            format!("Change matrix constrained to the {} pixels in different fragmentation classes [%]: ", sc.changed),
            format!(
                "Fragmentation decrease (=connectivity increase) - above the matrix diagonal [%]: {}",
                g6(sc.decrease)
            ),
            format!(
                "Fragmentation increase (=connectivity decrease) - below the matrix diagonal [%]: {}",
                g6(sc.increase)
            ),
            "A->B [%]     :  ".to_string()
                + &names
                    .iter()
                    .enumerate()
                    .skip(1)
                    .map(|(i, c)| format!("{:>16}", format!("B{i}-{c}")))
                    .collect::<String>(),
        ]);
        for (i, c) in names.iter().enumerate().skip(1) {
            lines.push(
                format!("{:<15}:", format!("A{i}-{c}"))
                    + &sc.percent[(i - 1) * 6..i * 6].iter().map(|v| format!("{:>16}", g6(*v))).collect::<String>(),
            );
        }
        Ok(outputs([
            ("matrix", Value::Table(matrix)),
            ("stats", Value::Table(stats)),
            ("report", Value::Text(lines.join("\n"))),
        ]))
    }
}

/// numpy's sum of an array: pairwise (8 accumulators in blocks of up to 128, halves above).
pub(crate) fn numpy_sum(v: &[f64]) -> f64 {
    fn pairwise(a: &[f64]) -> f64 {
        let n = a.len();
        if n < 8 {
            a.iter().fold(0.0, |s, x| s + x)
        } else if n <= 128 {
            let mut r = [0.0; 8];
            r.copy_from_slice(&a[..8]);
            let full = n - n % 8;
            for i in (8..full).step_by(8) {
                for j in 0..8 {
                    r[j] += a[i + j];
                }
            }
            let mut res = ((r[0] + r[1]) + (r[2] + r[3])) + ((r[4] + r[5]) + (r[6] + r[7]));
            for x in &a[full..] {
                res += x;
            }
            res
        } else {
            let mut n2 = n / 2;
            n2 -= n2 % 8;
            pairwise(&a[..n2]) + pairwise(&a[n2..])
        }
    }
    pairwise(v)
}
