//! `objects.resolve`: the arbitration of the hypotheses of sibling classes, cell by cell (the idea of GeoAIDA and
//! InterIMAGE). Kept in a file of its own, apart from the rest of the objects, so it can be left out of a release.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;

use super::{NO_REGION, ObjectClass, Objects};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Result, Table, Value, outputs};

pub fn register(r: &mut crate::Registry) {
    r.add(Arc::new(Resolve));
}

/// How the winning cells become objects.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Merge {
    Connected,
    Global,
    None,
}

/// What the arbitration decided, cell by cell.
struct Decision {
    /// The winning class (0-based) of each cell, `None` where none won (or outside the region).
    win: Vec<Option<usize>>,
    /// The winner's membership (NaN none) and the second highest membership (NaN outside the region).
    best: Vec<f64>,
    second: Vec<f64>,
    /// `lost[c][w]`: the cells of class c's hypotheses (membership >= min) that went to class w.
    lost: Vec<Vec<u64>>,
}

/// A membership as a number: NaN is no membership.
fn mem(v: f64) -> f64 {
    if v.is_nan() { 0.0 } else { v }
}

/// The arbitration: the class of greatest membership wins a cell if it reaches `min` (a tie goes to the class first
/// in `rank`, the classes in priority order); outside `region` (when there is one) nothing is classified.
fn decide(layers: &[Arc<Vec<f64>>], priority: &[usize], min: f64, region: Option<&[f64]>) -> Decision {
    let n = layers.first().map_or(0, |l| l.len());
    let k = layers.len();
    let mut d =
        Decision { win: vec![None; n], best: vec![f64::NAN; n], second: vec![f64::NAN; n], lost: vec![vec![0; k]; k] };
    for i in 0..n {
        if let Some(r) = region
            && (r[i].is_nan() || r[i] == 0.0)
        {
            continue;
        }
        let (mut first, mut second) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        let mut at = None;
        for &c in priority {
            let v = mem(layers[c][i]);
            if v > first {
                second = first;
                first = v;
                at = Some(c);
            } else if v > second {
                second = v;
            }
        }
        d.second[i] = if second.is_finite() { second } else { 0.0 };
        if let Some(w) = at
            && first >= min
            && first > 0.0
        {
            d.win[i] = Some(w);
            d.best[i] = first;
            for (c, layer) in layers.iter().enumerate() {
                let v = mem(layer[i]);
                if c != w && v >= min && v > 0.0 {
                    d.lost[c][w] += 1;
                }
            }
        }
    }
    d
}

/// What the arbitration decides in one cell (the Viewer and the trace tell it): the memberships `values` of the
/// classes, in the order `order` (indices, priority first; empty: the order of `values`), and `min`.
#[derive(Debug, Clone, PartialEq)]
pub struct Verdict {
    /// The class (index in `values`) that wins, if its membership reaches `min`.
    pub winner: Option<usize>,
    pub best: f64,
    /// The class of the second highest membership and that membership (the doubt, 0 when there is none).
    pub second: Option<usize>,
    pub doubt: f64,
    /// Whether the doubt reaches `min` (another class had its claim, lost to the winner).
    pub conflict: bool,
}

/// The arbitration of one cell: the same decision [`decide`] makes.
pub fn arbitrate(values: &[f64], order: &[usize], min: f64) -> Verdict {
    let all: Vec<usize> = (0..values.len()).collect();
    let priority: &[usize] = if order.is_empty() { &all } else { order };
    let (mut first, mut second) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
    let (mut at, mut at2) = (None, None);
    for &c in priority {
        let v = mem(values[c]);
        if v > first {
            (second, at2) = (first, at);
            (first, at) = (v, Some(c));
        } else if v > second {
            (second, at2) = (v, Some(c));
        }
    }
    let doubt = if second.is_finite() { second } else { 0.0 };
    let winner = at.filter(|_| first >= min && first > 0.0);
    Verdict {
        winner,
        best: if winner.is_some() { first } else { f64::NAN },
        second: at2.filter(|_| second.is_finite()),
        doubt,
        conflict: winner.is_some() && doubt >= min && doubt > 0.0,
    }
}

/// A cell's arbitration in a sentence: `Trees 0.82 won over Grass 0.64 (conflict: the second reaches the minimum
/// 0.1)`; the classes by `names`, `order` as the node's setting names them (none: the order of the layers).
pub fn explain_cell(names: &[String], values: &[f64], order: Option<&[String]>, min: f64) -> String {
    let order: Vec<usize> =
        order.map(|o| o.iter().filter_map(|n| names.iter().position(|x| x == n)).collect()).unwrap_or_default();
    // (the classes `order` leaves out, in the order of the layers)
    let order: Vec<usize> = if order.is_empty() {
        order
    } else {
        order.iter().copied().chain((0..names.len()).filter(|c| !order.contains(c))).collect()
    };
    let v = arbitrate(values, &order, min);
    let num = |x: f64| format!("{x:.2}");
    let name = |c: usize| names.get(c).map_or("?", |n| n.as_str());
    let said: Vec<String> = names.iter().zip(values).map(|(n, x)| format!("{n} {}", num(mem(*x)))).collect();
    let tail = match (v.winner, v.second) {
        (Some(w), Some(s)) if v.conflict => format!(
            "{} {} won over {} {} (conflict: the second reaches the minimum {min})",
            name(w),
            num(v.best),
            name(s),
            num(v.doubt)
        ),
        (Some(w), Some(s)) => {
            format!("{} {} won; doubt {} {}", name(w), num(v.best), name(s), num(v.doubt))
        }
        (Some(w), None) => format!("{} {} won; no doubt", name(w), num(v.best)),
        (None, _) => format!("no class: the greatest membership does not reach the minimum {min}"),
    };
    format!("memberships {}; {tail}", said.join(", "))
}

/// The region of a parent class: the union of the winning cells of its children, a map of 1 and NaN. A child is a
/// `class` map of [`Resolve`] (NaN where it won nothing); every map is on the grid.
#[allow(dead_code)] // for the class tree
pub(crate) fn union_region(grid: &Arc<Grid>, children: &[&[f64]]) -> Map {
    let n = grid.width * grid.height;
    let data =
        (0..n)
            .map(|i| {
                if children.iter().any(|c| c.get(i).is_some_and(|v| !v.is_nan() && *v != 0.0)) { 1.0 } else { f64::NAN }
            })
            .collect();
    Map::new(grid.clone(), data)
}

struct Resolve;

impl Operator for Resolve {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.resolve",
            "Decides between the hypotheses of sibling classes, cell by cell, as GeoAIDA (Bückner et al. 2001) and \
             InterIMAGE (Costa et al. 2010) do: each class is found by its own method and gives a map of membership \
             (0 outside its hypotheses), the maps overlap, and in each cell the class of the greatest membership \
             wins, if it reaches `min` (below it the cell has no class); a tie goes by `order`. A hypothesis \
             partly overlapped loses only the cells where another won. The winning cells become objects: one per \
             connected area of a class (`connected`), one per class (`global`) or one per cell (`none`, to look at \
             the arbitration). Gives the objects with their classes and the mean membership of each class over \
             each object, the maps of the class (its number, 1 to the number of classes), of the winning membership \
             and of the doubt (the second highest membership, where to look next), a table of the conflicts (the \
             cells of each class's hypotheses that went to each other class) and the objects' table.",
            Access::Global,
        )
        .input(PortSpec::new(
            "memberships",
            Type::Any,
            "a stack with a layer per sibling class, named for it: the membership of each cell, 0 to 1",
        ))
        .input(PortSpec::optional(
            "region",
            Type::Map,
            "the region of the parent class: only the cells with a value other than 0 are classified",
        ))
        .output(PortSpec::new("objects", Type::Objects, "the winning cells as objects, with classes and memberships"))
        .output(PortSpec::new("class", Type::Map, "the class number of each cell, 1 to the number of classes"))
        .output(PortSpec::new("membership", Type::Map, "the membership of the winning class"))
        .output(PortSpec::new("doubt", Type::Map, "the second highest membership of each cell"))
        .output(PortSpec::new("conflicts", Type::Table, "class, lost_to, cells and area of the lost hypotheses"))
        .output(PortSpec::new("table", Type::Table, "id, cells, area, class and membership of each object"))
        .param(PortSpec::optional("min", Type::Number, "the least membership that wins a cell (default 0.1)"))
        .param(PortSpec::optional(
            "order",
            Type::Any,
            "the class names in the order a tie goes by (default: the order of the layers)",
        ))
        .param(PortSpec::optional(
            "merge",
            Type::Text,
            "connected (default: an object per connected area of a class, 4 neighbours), global (an object per class, \
             in parts if need be) or none (an object per cell, to inspect the arbitration)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let stack = crate::raster_stats::layers(args, "memberships")?;
        let names = stack.names.clone();
        let k = names.len();
        for (j, n) in names.iter().enumerate() {
            if names[..j].contains(n) {
                return Err(bad(format!("memberships: two layers are named {n:?}")));
            }
        }
        let grid = stack.grid.clone();
        let cells = grid.width * grid.height;
        let min: f64 = args.param("min", 0.1)?;
        if min.is_nan() || !(0.0..=1.0).contains(&min) {
            return Err(bad(format!("min {min}: 0 to 1")));
        }
        let merge = match args.param("merge", "connected".to_string())?.as_str() {
            "connected" => Merge::Connected,
            "global" => Merge::Global,
            "none" => Merge::None,
            other => return Err(bad(format!("merge {other:?}: connected, global or none"))),
        };
        // the classes in the order a tie goes by: those of `order`, then the rest as the layers come
        let mut priority: Vec<usize> = Vec::new();
        if let Some(order) = args.opt_param::<Vec<String>>("order")? {
            for o in &order {
                let c = names
                    .iter()
                    .position(|n| n == o)
                    .ok_or_else(|| bad(format!("order: {o:?} is none of the classes: {}", names.join(", "))))?;
                if priority.contains(&c) {
                    return Err(bad(format!("order: {o:?} twice")));
                }
                priority.push(c);
            }
        }
        let rest: Vec<usize> = (0..k).filter(|c| !priority.contains(c)).collect();
        priority.extend(rest);
        let region = match args.get("region") {
            None => None,
            Some(Value::Map(m)) => {
                if !m.grid.same_as(&grid) {
                    return Err(bad(
                        "region is not on the grid of the memberships: put it there with raster.match".into()
                    ));
                }
                Some(m.data.clone())
            }
            Some(v) => return Err(bad(format!("region is a {}, not a map", v.ty().name()))),
        };
        let d = decide(&stack.data, &priority, min, region.as_deref().map(|r| r.as_slice()));

        // the objects
        let (w, h) = (grid.width, grid.height);
        let objects = match merge {
            Merge::Connected => {
                let reg: Vec<u64> = d.win.iter().map(|x| x.map_or(NO_REGION, |c| c as u64)).collect();
                Objects::from_regions(grid.clone(), &reg, 4)
            }
            Merge::Global => {
                let mut id_of = vec![0u32; k];
                let mut count = 0u32;
                let labels: Vec<u32> = d
                    .win
                    .iter()
                    .map(|x| match x {
                        None => 0,
                        Some(c) => {
                            if id_of[*c] == 0 {
                                count += 1;
                                id_of[*c] = count;
                            }
                            id_of[*c]
                        }
                    })
                    .collect();
                Objects::from_labels(grid.clone(), labels, count as usize)?
            }
            Merge::None => {
                let mut count = 0u32;
                let labels: Vec<u32> = d
                    .win
                    .iter()
                    .map(|x| {
                        if x.is_some() {
                            count += 1;
                            count
                        } else {
                            0
                        }
                    })
                    .collect();
                Objects::from_labels(grid.clone(), labels, count as usize)?
            }
        };
        let count = objects.count;
        let mut class_of = vec![0u32; count];
        let mut sums = vec![vec![0.0f64; count]; k];
        for i in 0..w * h {
            let l = objects.labels[i] as usize;
            if l == 0 {
                continue;
            }
            class_of[l - 1] = d.win[i].map_or(0, |c| c as u32 + 1);
            for (c, s) in sums.iter_mut().enumerate() {
                s[l - 1] += mem(stack.data[c][i]);
            }
        }
        for s in sums.iter_mut() {
            for (o, v) in s.iter_mut().enumerate() {
                *v /= objects.cells[o] as f64;
            }
        }
        let area = objects.cell_area();
        let mut table = Table::new(["id", "cells", "area", "class", "membership"].map(String::from).to_vec());
        table.labels.insert("class".into(), names.clone());
        table.rows = (0..count)
            .map(|o| {
                let c = class_of[o] as usize;
                vec![(o + 1) as f64, objects.cells[o] as f64, objects.cells[o] as f64 * area, c as f64, sums[c - 1][o]]
            })
            .collect();
        let classes = names.iter().map(|n| ObjectClass { name: n.clone(), colour: None }).collect();
        let objects = objects.with_classes(classes, class_of, sums)?.with_table(table.clone())?;

        let mut conflicts = Table::new(["class", "lost_to", "cells", "area"].map(String::from).to_vec());
        conflicts.labels.insert("class".into(), names.clone());
        conflicts.labels.insert("lost_to".into(), names.clone());
        let cell_area = objects.cell_area();
        for c in 0..k {
            for x in 0..k {
                let n = d.lost[c][x];
                if n > 0 {
                    conflicts.rows.push(vec![(c + 1) as f64, (x + 1) as f64, n as f64, n as f64 * cell_area]);
                }
            }
        }
        let class_map: Vec<f64> = d.win.iter().map(|x| x.map_or(f64::NAN, |c| (c + 1) as f64)).collect();
        debug_assert_eq!(class_map.len(), cells);
        Ok(outputs([
            ("objects", Value::Objects(Arc::new(objects))),
            ("class", Value::Map(Map::new(grid.clone(), class_map))),
            ("membership", Value::Map(Map::new(grid.clone(), d.best))),
            ("doubt", Value::Map(Map::new(grid, d.second))),
            ("conflicts", Value::Table(conflicts)),
            ("table", Value::Table(table)),
        ]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn union_of_children() {
        let grid = Arc::new(Grid::new(3, 1));
        let a = [1.0, f64::NAN, f64::NAN];
        let b = [f64::NAN, f64::NAN, 2.0];
        let m = union_region(&grid, &[&a, &b]);
        assert_eq!(m.data[0], 1.0);
        assert!(m.data[1].is_nan());
        assert_eq!(m.data[2], 1.0);
    }
}
