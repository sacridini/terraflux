//! GuidosToolbox's Restoration Planner from landschaft's port of it: the restoration status of the habitat
//! (`restoration.status`: ECA, coherence, restoration potential), cost distances and least-cost paths across a
//! resistance (`restoration.cost_distance`, `restoration.least_cost_path`), the gain of restoring a corridor
//! (`restoration.corridor`), a resistance from the distance to the habitat (`restoration.resistance`) and the
//! isochrone maps from one object or between two (`restoration.isochrone`, GTB's ggeo). The kernels are landschaft's
//! C++; what its Python does around them (the isochrones' rounding and scaling, IDL's) is here.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::landscape::{foreground_param, gtb_coded, map_of};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Status));
    r.add(Arc::new(CostDistance));
    r.add(Arc::new(LeastCostPath));
    r.add(Arc::new(Corridor));
    r.add(Arc::new(Resistance));
    r.add(Arc::new(Isochrone));
}

/// A map of bytes (a resistance, markers): whole values 0-255, no value 0.
fn bytes<'a>(args: &'a Args, port: &str) -> Result<(Vec<u8>, &'a Map)> {
    let m = args.map(port)?;
    let v = m
        .data
        .iter()
        .map(|&v| match v {
            v if v.is_nan() => Ok(0),
            v if v.fract() == 0.0 && (0.0..=255.0).contains(&v) => Ok(v as u8),
            v => Err(Error::msg(format!("node {:?}: {port} holds bytes 0-255, not {v}", args.node))),
        })
        .collect::<Result<_>>()?;
    Ok((v, m))
}

/// A mask of a map: its cells above 0.
fn mask(args: &Args, port: &str, like: &Map) -> Result<Vec<u8>> {
    let m = args.map(port)?;
    if m.shape() != like.shape() {
        return Err(Error::msg(format!("node {:?}: {port} is not on the grid of the resistance", args.node)));
    }
    Ok(m.data.iter().map(|&v| u8::from(v > 0.0)).collect())
}

const STATUS: [&str; 10] =
    ["data_area", "objects", "habitat", "largest", "aps", "cnoa", "eca", "coherence", "restoration_potential", "rac"];

fn status(input: &[u8], (ny, nx): (usize, usize)) -> Result<Vec<f64>> {
    let mut s = ffi::restoration_status(input, ny as i64, nx as i64)?;
    // GTB's CNOA: a whole number
    s[5] = s[5].trunc();
    Ok(s)
}

struct Status;

impl Operator for Status {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "restoration.status",
                "How connected the habitat is and how much reconnecting it could add, as GuidosToolbox's Restoration \
                 Status (landschaft's restoration_status): the habitat's objects (8-connected), the equivalent \
                 connected area ECA = sqrt(sum of their areas squared), the coherence ECA / habitat (%), the \
                 restoration potential (100 - coherence), the habitat's share of the area with data (RAC), the \
                 average patch size and the critical new object area.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the habitat map (0 missing, 1 background, 2 habitat)"))
            .output(PortSpec::new(
                "stats",
                Type::Table,
                "the area with data, objects, habitat, largest object, APS, CNOA, ECA, coherence (%), restoration \
                 potential (%), RAC (%)",
            )),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let input = gtb_coded(args, map, false)?;
        let mut t = Table::new(STATUS.map(String::from).to_vec());
        t.rows.push(status(&input, map.shape())?);
        Ok(outputs([("stats", Value::Table(t))]))
    }
}

struct CostDistance;

impl Operator for CostDistance {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "restoration.cost_distance",
            "The accumulated cost of travelling from the sources across a resistance, as GuidosToolbox \
             (landschaft's cost_distance): 8 neighbours, a step the two cells' mean resistance times its length \
             (1, or sqrt(2) diagonally).",
            Access::Global,
        )
        .input(PortSpec::new("resistance", Type::Map, "the resistance, 1-100 (0 or no value: impassable)"))
        .input(PortSpec::new("sources", Type::Map, "the sources: cells above 0"))
        .output(PortSpec::new("map", Type::Map, "the cost (no value where the sources cannot be reached)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (res, m) = bytes(args, "resistance")?;
        let src = mask(args, "sources", m)?;
        let (ny, nx) = m.shape();
        let d = ffi::cost_distance(&res, &src, ny as i64, nx as i64)?;
        let v = d.iter().map(|&x| if x.is_finite() { f64::from(x) } else { f64::NAN }).collect();
        Ok(outputs([("map", Value::Map(Map::new(m.grid.clone(), v)))]))
    }
}

/// A least-cost path, the grid and the markers A and B.
type Lcp = (ffi::LcpOut, Arc<terraflux_raster::Grid>, Vec<u8>, Vec<u8>);

/// A least-cost path's outputs: the combined cost, the path, its numbers.
fn lcp(args: &Args) -> Result<Lcp> {
    let (res, m) = bytes(args, "resistance")?;
    let (a, b) = (mask(args, "a", m)?, mask(args, "b", m)?);
    let dist: Vec<f32> = match args.opt_map("distance")? {
        Some(d) if d.shape() == m.shape() => d.data.iter().map(|&v| v as f32).collect(),
        Some(_) => return Err(Error::msg(format!("node {:?}: distance is not on the resistance's grid", args.node))),
        None => Vec::new(),
    };
    let (ny, nx) = m.shape();
    Ok((ffi::least_cost_path(&res, &a, &b, &dist, ny as i64, nx as i64)?, m.grid.clone(), a, b))
}

fn lcp_ports(sig: OpSignature) -> OpSignature {
    sig.input(PortSpec::new("resistance", Type::Map, "the resistance, 1-100 (0 or no value: impassable)"))
        .input(PortSpec::new("a", Type::Map, "area A: cells above 0"))
        .input(PortSpec::new("b", Type::Map, "area B: cells above 0"))
        .input(PortSpec::optional(
            "distance",
            Type::Map,
            "a distance (distance.euclidean), for the restored cells' mean",
        ))
        .output(PortSpec::new("path", Type::Map, "the path: 1, 0 elsewhere"))
}

struct LeastCostPath;

impl Operator for LeastCostPath {
    fn signature(&self) -> OpSignature {
        lcp_ports(OpSignature::new(
            "restoration.least_cost_path",
            "The least-cost path between two areas across a resistance, as GuidosToolbox (landschaft's \
             least_cost_path): the cells of the least cost from A plus from B.",
            Access::Global,
        ))
        .output(PortSpec::new("cost", Type::Map, "the cost from A plus the cost from B"))
        .output(PortSpec::new(
            "stats",
            Type::Table,
            "the path's length, its cells in the background, their resistance (the expense) and mean distance",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (r, grid, _, _) = lcp(args)?;
        let cost = r.cost.iter().map(|&x| if x.is_finite() { f64::from(x) } else { f64::NAN }).collect();
        let mut t = Table::new(["length", "restored", "expense", "mean_distance"].map(String::from).to_vec());
        t.rows.push(vec![r.path_length as f64, r.rest_pixels as f64, r.expense, r.avg_distance]);
        Ok(outputs([
            ("path", Value::Map(Map::new(grid.clone(), r.path.iter().map(|&v| f64::from(v)).collect()))),
            ("cost", Value::Map(Map::new(grid, cost))),
            ("stats", Value::Table(t)),
        ]))
    }
}

struct Corridor;

impl Operator for Corridor {
    fn signature(&self) -> OpSignature {
        lcp_ports(OpSignature::new(
            "restoration.corridor",
            "The gain of restoring the least-cost corridor between two areas, as GuidosToolbox's Restoration \
             Planner (landschaft's restoration_corridor): the path's background cells turned habitat, the ECA and \
             coherence before and after, and the gain per resistance restored.",
            Access::Global,
        ))
        .input(PortSpec::new("habitat", Type::Map, "the habitat (0 missing, 1 background, 2 habitat)"))
        .output(PortSpec::new("restored", Type::Map, "the habitat with the corridor restored"))
        .output(PortSpec::new(
            "stats",
            Type::Table,
            "the sizes of A and B, the cells restored, their mean distance, the expense, the efficiency (ECA \
             gained per expense), ECA and coherence before, after and their gains",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let (r, grid, a, b) = lcp(args)?;
        let (hab, h) = bytes(args, "habitat")?;
        if (h.grid.width, h.grid.height) != (grid.width, grid.height) {
            return Err(Error::msg(format!("node {:?}: habitat is not on the resistance's grid", args.node)));
        }
        let shape = h.shape();
        let before = status(&hab, shape)?;
        let restored: Vec<u8> = hab.iter().zip(&r.path).map(|(&v, &p)| if v == 1 && p > 0 { 2 } else { v }).collect();
        let after = status(&restored, shape)?;
        let (delta_eca, delta_doc) = (after[6] - before[6], after[7] - before[7]);
        let effic = if r.expense > 0.0 { delta_eca / r.expense } else { 0.0 };
        let count = |m: &[u8]| m.iter().filter(|&&v| v > 0).count() as f64;
        let mut t = Table::new(
            [
                "size_a",
                "size_b",
                "restored",
                "mean_distance",
                "expense",
                "efficiency",
                "eca",
                "eca_new",
                "delta_eca",
                "coherence",
                "coherence_new",
                "delta_coherence",
            ]
            .map(String::from)
            .to_vec(),
        );
        t.rows.push(vec![
            count(&a),
            count(&b),
            r.rest_pixels as f64,
            r.avg_distance,
            r.expense,
            effic,
            before[6],
            after[6],
            delta_eca,
            before[7],
            after[7],
            delta_doc,
        ]);
        Ok(outputs([
            ("path", Value::Map(Map::new(grid, r.path.iter().map(|&v| f64::from(v)).collect()))),
            ("restored", map_of(h, restored)),
            ("stats", Value::Table(t)),
        ]))
    }
}

/// GTB's frame around the maps given to ggeo.
const EEW: usize = 2;

/// A map framed by `EEW` cells of `fill`.
fn pad(v: &[u8], (ny, nx): (usize, usize), fill: u8) -> Vec<u8> {
    let w = nx + 2 * EEW;
    let mut out = vec![fill; (ny + 2 * EEW) * w];
    for y in 0..ny {
        out[(y + EEW) * w + EEW..(y + EEW) * w + EEW + nx].copy_from_slice(&v[y * nx..(y + 1) * nx]);
    }
    out
}

/// The inside of a framed map.
fn crop<T: Copy>(v: &[T], (ny, nx): (usize, usize)) -> Vec<T> {
    let w = nx + 2 * EEW;
    (0..ny).flat_map(|y| v[(y + EEW) * w + EEW..(y + EEW) * w + EEW + nx].iter().copied()).collect()
}

struct Resistance;

impl Operator for Resistance {
    fn signature(&self) -> OpSignature {
        foreground_param(
            OpSignature::new(
                "restoration.resistance",
                "A resistance growing with the distance to the habitat, as GuidosToolbox's Setup Tools -> Distance \
                 -> Resistance (landschaft's distance_to_resistance): the background round(distance + 2) up to 100 \
                 (3 next to the habitat), the habitat 2, missing 0; IDL's quasi-Euclidean distance.",
                Access::Global,
            )
            .input(PortSpec::new("map", Type::Map, "the map (0 missing, 1 background, 2 habitat)"))
            .output(PortSpec::new("map", Type::Map, "the resistance")),
        )
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let map = args.map("map")?;
        let arr = gtb_coded(args, map, false)?;
        if !arr.contains(&2) {
            return Err(Error::msg(format!("node {:?}: the map has no habitat (2)", args.node)));
        }
        let (ny, nx) = map.shape();
        // GTB works on the image as shown (rows flipped)
        let flipped: Vec<u8> =
            (0..ny).rev().flat_map(|y| arr[y * nx..(y + 1) * nx].iter().map(|&v| u8::from(v == 2))).collect();
        let ext = pad(&flipped, (ny, nx), 0);
        let d = crop(&ffi::morph_distance(&ext, (ny + 2 * EEW) as i64, (nx + 2 * EEW) as i64, true)?, (ny, nx));
        let out: Vec<u8> = (0..ny * nx)
            .map(|c| {
                let (y, x) = (c / nx, c % nx);
                match arr[c] {
                    2 => 2,
                    0 => 0,
                    _ => {
                        let v = f64::from((d[(ny - 1 - y) * nx + x] + 2.0).min(100.0));
                        (v + 0.5).floor().max(0.0) as u8
                    }
                }
            })
            .collect();
        Ok(outputs([("map", map_of(map, out))]))
    }
}

/// IDL's round(v / 2.0) to a LONG: single precision, half away from zero, -2^31 out of range.
fn idl_round(v: f64) -> i64 {
    let f = f64::from(v as f32 / 2.0);
    let r = (f.abs() + 0.5).floor().copysign(f);
    if (-2147483648.0..2147483648.0).contains(&r) { r as i64 } else { -2147483648 }
}

/// IDL's BYTSCL of whole numbers to 0-100: ((101 (a - lo) - 1) / (hi - lo)), C's division, clipped.
fn bytscl(a: i64, lo: i64, hi: i64) -> u8 {
    if hi <= lo {
        return if a >= hi { 100 } else { 0 };
    }
    let num = 101 * (a.clamp(lo, hi) - lo) - 1;
    (num / (hi - lo)).clamp(0, 100) as u8
}

/// The 8-connected objects of a mask: their count and the label of every cell.
fn label(m: &[bool], (ny, nx): (usize, usize)) -> (usize, Vec<usize>) {
    let mut lab = vec![0usize; ny * nx];
    let mut n = 0;
    for s in 0..ny * nx {
        if !m[s] || lab[s] != 0 {
            continue;
        }
        n += 1;
        lab[s] = n;
        let mut stack = vec![s];
        while let Some(c) = stack.pop() {
            let (y, x) = ((c / nx) as i64, (c % nx) as i64);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (yy, xx) = (y + dy, x + dx);
                    if yy >= 0 && xx >= 0 && yy < ny as i64 && xx < nx as i64 {
                        let k = yy as usize * nx + xx as usize;
                        if m[k] && lab[k] == 0 {
                            lab[k] = n;
                            stack.push(k);
                        }
                    }
                }
            }
        }
    }
    (n, lab)
}

struct Isochrone;

impl Operator for Isochrone {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "restoration.isochrone",
            "The isochrone map of GuidosToolbox's Restoration Planner (landschaft's isochrone_map and \
             isochrone_map_ab, GTB's ggeo): the accumulated cost of reaching every cell from object A across a \
             resistance (a step between neighbours their mean resistance, the habitat 1), or with an object B the \
             cost of the cheapest route from A to B through every cell and its least-cost path; scaled 0-100 in \
             GTB's codes.",
            Access::Global,
        )
        .input(PortSpec::new("resistance", Type::Map, "the resistance: 2 habitat (cost 1), 3-100 background"))
        .input(PortSpec::new(
            "marker",
            Type::Map,
            "0 background, 1 object A, 2 object B (the route A-B), 3 missing (not crossed); one object each",
        ))
        .output(PortSpec::new(
            "map",
            Type::Map,
            "GTB's codes: the cost scaled 0-100, 101 the objects, 102 missing, 103 the least-cost path, 104 \
             unreachable",
        ))
        .output(PortSpec::new("cost", Type::Map, "the cost as GTB rounds it (no value where unreachable)"))
        .output(PortSpec::new("path", Type::Map, "A-B: the least-cost path (1)"))
        .output(PortSpec::new("stats", Type::Table, "the least and the largest cost"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let (resist, rm) = bytes(args, "resistance")?;
        let (mark, mm) = bytes(args, "marker")?;
        if rm.shape() != mm.shape() {
            return Err(err("the marker and the resistance differ in size".into()));
        }
        let shape = rm.shape();
        let (ny, nx) = shape;
        let n = ny * nx;
        let ab = mark.contains(&2);
        let values: &[u8] = if ab { &[1, 2] } else { &[1] };
        for v in std::iter::once(&0).chain(values) {
            if !mark.contains(v) {
                return Err(err(format!("the marker map must contain {v}")));
            }
        }
        for &v in values {
            if label(&mark.iter().map(|&m| m == v).collect::<Vec<_>>(), shape).0 >= 2 {
                return Err(err(format!("the marker map must have only one object with value {v}")));
            }
        }
        // GTB's mask for ggeo: the habitat (2) costs 1; without habitat the top-right cell does (IDL's where -1)
        let mut m = resist.clone();
        if resist.contains(&2) {
            m.iter_mut().filter(|v| **v == 2).for_each(|v| *v = 1);
        } else {
            m[nx - 1] = 1;
        }
        let framed = (ny + 2 * EEW, nx + 2 * EEW);
        let geo = |reference: &[u8]| -> Result<Vec<u32>> {
            let t =
                ffi::geodesic_time(&pad(&m, shape, 0), &pad(reference, shape, 3), framed.0 as i64, framed.1 as i64)?;
            Ok(crop(&t, shape))
        };
        let missing: Vec<bool> = mark.iter().map(|&v| v == 3).collect();
        let any_missing = missing.iter().any(|m| *m);
        let (out, cost, path, mincost, maxcost) = if !ab {
            let t = geo(&mark.iter().map(|&v| if v == 2 { 0 } else { v }).collect::<Vec<_>>())?;
            let cost: Vec<i64> = t.iter().map(|&v| idl_round(f64::from(v))).collect();
            let mut maxcost = *cost.iter().max().unwrap();
            if any_missing {
                maxcost = cost.iter().map(|&c| if c < maxcost { c } else { 0 }).max().unwrap();
            }
            let mut out: Vec<u8> = cost.iter().map(|&c| bytscl(c, 0, maxcost)).collect();
            for c in 0..n {
                if cost[c] < -2 {
                    out[c] = 104;
                }
                if mark[c] == 1 {
                    out[c] = 101;
                }
                if missing[c] {
                    out[c] = 102;
                }
            }
            let mincost = t.iter().filter(|&&v| v > 0).min().map_or(0, |&v| idl_round(f64::from(v)));
            (out, cost, None, mincost, maxcost)
        } else {
            let both: Vec<bool> = mark.iter().map(|&v| v == 1 || v == 2).collect();
            if label(&both, shape).0 == 1 {
                return Err(err("marker objects A and B must not touch each other".into()));
            }
            if any_missing {
                let (_, comp) = label(&mark.iter().map(|&v| v != 3).collect::<Vec<_>>(), shape);
                let first = |v: u8| comp[mark.iter().position(|&m| m == v).unwrap()];
                if first(1) != first(2) {
                    return Err(err("marker objects A and B can not be connected (separated by missing cells)".into()));
                }
            }
            let t_a = geo(&mark.iter().map(|&v| if v == 2 { 0 } else { v }).collect::<Vec<_>>())?;
            let t_b = geo(&mark
                .iter()
                .map(|&v| match v {
                    1 => 0,
                    2 => 1,
                    v => v,
                })
                .collect::<Vec<_>>())?;
            // uint32 arithmetic, as ggeo's
            let total: Vec<u32> = t_a.iter().zip(&t_b).map(|(a, b)| a.wrapping_add(*b)).collect();
            let least = *total.iter().min().unwrap();
            let on_min: Vec<u8> = total.iter().map(|&v| u8::from(v == least)).collect();
            let anchors: Vec<u8> = both.iter().map(|&b| u8::from(b)).collect();
            let thin = ffi::thin_anchored(
                &pad(&on_min, shape, 0),
                0,
                &pad(&anchors, shape, 0),
                framed.0 as i64,
                framed.1 as i64,
            )?;
            let lcp: Vec<bool> = crop(&thin, shape).iter().map(|&v| v > 0).collect();
            let mincost = idl_round(f64::from(least));
            let most = *total.iter().max().unwrap();
            let maxcost = if any_missing {
                idl_round(total.iter().map(|&v| if v < most { i64::from(v) } else { 0 }).max().unwrap() as f64)
            } else {
                idl_round(f64::from(most))
            };
            let cost: Vec<i64> = total.iter().map(|&v| idl_round(f64::from(v))).collect();
            let mut out: Vec<u8> = cost.iter().map(|&c| bytscl(c, mincost, maxcost)).collect();
            for c in 0..n {
                if lcp[c] {
                    out[c] = 103;
                }
                if both[c] {
                    out[c] = 101;
                }
                if missing[c] {
                    out[c] = 102;
                }
                if cost[c] < -2 {
                    out[c] = 104;
                }
            }
            (out, cost, Some(lcp), mincost, maxcost)
        };
        let cost_map: Vec<f64> = cost.iter().map(|&c| if c == -2147483648 { f64::NAN } else { c as f64 }).collect();
        let mut t = Table::new(["min_cost", "max_cost"].map(String::from).to_vec());
        t.rows.push(vec![mincost as f64, maxcost as f64]);
        let mut o = outputs([
            ("map", map_of(rm, out)),
            ("cost", Value::Map(Map::new(rm.grid.clone(), cost_map))),
            ("stats", Value::Table(t)),
        ]);
        if let Some(lcp) = path {
            o.insert(
                "path".into(),
                Value::Map(Map::new(rm.grid.clone(), lcp.iter().map(|&b| f64::from(u8::from(b))).collect())),
            );
        }
        Ok(o)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idl_arithmetic() {
        assert_eq!(idl_round(5.0), 3);
        assert_eq!(idl_round(4294967295.0), -2147483648);
        assert_eq!(bytscl(5, 0, 10), 50);
        assert_eq!(bytscl(0, 0, 10), 0);
        assert_eq!(bytscl(10, 0, 10), 100);
        assert_eq!(bytscl(3, 5, 5), 0);
    }
}
