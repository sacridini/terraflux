//! Change between two maps from landschaft's port of GuidosToolbox: the simple change of the foreground
//! (`change.simple`), its morphological change, which tells the change of the core from the spurious one at the
//! edges (`change.morphological`), and the change of the fragmentation measured by FOS (`change.fos`).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::landscape::{foreground_param, gtb_coded, map_of};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Simple));
    r.add(Arc::new(Morphological));
    r.add(Arc::new(Fos));
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
