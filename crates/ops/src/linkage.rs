//! Linkage Mapper's Linkage Pathways (McRae and Kavanagh 2011): the least-cost links between core areas over a
//! resistance surface, and the corridors around them. Pure Rust, no kernel of the library.
//!
//! The cost-weighted distance (CWD) is ArcGIS's and Linkage Mapper's: 8 neighbours, a step costing the mean of the
//! two cells' resistances times its length (the cell's size, times sqrt(2) diagonally), a source cell 0. A cell
//! without a value, or of a resistance of 0 or less, is impassable. [`cwd`] is the search (from any set of
//! sources, with the nearest source's label: a cost allocation), [`edt`] the Euclidean one.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};
use std::sync::Arc;

use rayon::prelude::*;
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use crate::circuit::{Circuit, FocalPoints, OneToAllOptions, one_to_all};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Pathways));
    r.add(Arc::new(Pinchpoints));
    r.add(Arc::new(Barriers));
}

/// The most memory the cost-weighted distances of all the cores in links may take together (bytes).
const MEMORY: f64 = 4e9;

/// The neighbours: (row, column) offsets.
const DIRS: [(i64, i64); 8] = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)];

/// A grid's shape and the size of its cells.
#[derive(Debug, Clone, Copy)]
pub struct Lattice {
    pub w: usize,
    pub h: usize,
    pub dx: f64,
    pub dy: f64,
}

impl Lattice {
    pub fn of(g: &Grid) -> Lattice {
        Lattice { w: g.width, h: g.height, dx: g.transform[1].abs(), dy: g.transform[5].abs() }
    }

    pub fn cells(&self) -> usize {
        self.w * self.h
    }

    /// The length of a step of (dr, dc).
    fn length(&self, dr: i64, dc: i64) -> f64 {
        match (dr != 0, dc != 0) {
            (true, true) => (self.dx * self.dx + self.dy * self.dy).sqrt(),
            (true, false) => self.dy,
            _ => self.dx,
        }
    }

    /// The neighbours of a cell on the grid, with the length of the step to each.
    fn around(&self, c: usize) -> impl Iterator<Item = (usize, f64)> + '_ {
        let (r, k) = ((c / self.w) as i64, (c % self.w) as i64);
        DIRS.iter().filter_map(move |&(dr, dc)| {
            let (rr, kk) = (r + dr, k + dc);
            (rr >= 0 && kk >= 0 && rr < self.h as i64 && kk < self.w as i64)
                .then(|| (rr as usize * self.w + kk as usize, self.length(dr, dc)))
        })
    }
}

fn passable(v: f64) -> bool {
    v.is_finite() && v > 0.0
}

/// The cost of a step between two cells of these resistances over this length.
fn step(a: f64, b: f64, len: f64) -> f64 {
    (a + b) / 2.0 * len
}

/// The cost-weighted distance from the sources (cell, label): to every cell the least cost to the nearest
/// source (infinity: not reachable), and the label of that source (0: none). Sources on impassable cells are
/// ignored. Dijkstra's, on f64; a tie goes to the first found, by cost then cell.
pub fn cwd(res: &[f64], lat: &Lattice, sources: &[(usize, u32)]) -> (Vec<f64>, Vec<u32>) {
    let n = lat.cells();
    let mut dist = vec![f64::INFINITY; n];
    let mut label = vec![0u32; n];
    let mut heap = BinaryHeap::new();
    for &(c, l) in sources {
        if passable(res[c]) && dist[c] != 0.0 {
            dist[c] = 0.0;
            label[c] = l;
            heap.push(Reverse((0.0f64.to_bits(), c)));
        }
    }
    while let Some(Reverse((bits, c))) = heap.pop() {
        let d = f64::from_bits(bits);
        if d > dist[c] {
            continue;
        }
        for (nb, len) in lat.around(c) {
            if !passable(res[nb]) {
                continue;
            }
            let nd = d + step(res[c], res[nb], len);
            if nd < dist[nb] {
                dist[nb] = nd;
                label[nb] = label[c];
                heap.push(Reverse((nd.to_bits(), nb)));
            }
        }
    }
    (dist, label)
}

/// The Euclidean distance (map units, between cells' centres) from every cell to the nearest site (a cell of a
/// label above 0), and that site's label; infinity and 0 where there is none. Felzenszwalb and Huttenlocher's
/// exact transform, a pass down the columns and one along the rows.
pub fn edt(sites: &[u32], lat: &Lattice) -> (Vec<f64>, Vec<u32>) {
    let (w, h) = (lat.w, lat.h);
    // the nearest site's row in each cell's column (-1 none)
    let mut near = vec![-1i64; w * h];
    for x in 0..w {
        let mut last = -1i64;
        for y in 0..h {
            if sites[y * w + x] > 0 {
                last = y as i64;
            }
            near[y * w + x] = last;
        }
        let mut next = -1i64;
        for y in (0..h).rev() {
            if sites[y * w + x] > 0 {
                next = y as i64;
            }
            let a = near[y * w + x];
            if next >= 0 && (a < 0 || (next - y as i64) < (y as i64 - a)) {
                near[y * w + x] = next;
            }
        }
    }
    let mut dist = vec![f64::INFINITY; w * h];
    let mut label = vec![0u32; w * h];
    let mut v = vec![0usize; w];
    let mut z = vec![0f64; w + 1];
    let dx2 = lat.dx * lat.dx;
    for y in 0..h {
        let g = |q: usize| {
            let s = near[y * w + q];
            (s >= 0).then(|| ((s - y as i64).abs() as f64 * lat.dy).powi(2))
        };
        // the parabolas' lower envelope
        let mut k: isize = -1;
        for q in 0..w {
            let Some(gq) = g(q) else { continue };
            if k < 0 {
                k = 0;
                v[0] = q;
                z[0] = f64::NEG_INFINITY;
                z[1] = f64::INFINITY;
                continue;
            }
            loop {
                let p = v[k as usize];
                let gp = g(p).unwrap();
                let s = ((gq + dx2 * (q * q) as f64) - (gp + dx2 * (p * p) as f64)) / (2.0 * dx2 * (q - p) as f64);
                if s <= z[k as usize] {
                    k -= 1;
                    if k < 0 {
                        k = 0;
                        v[0] = q;
                        z[0] = f64::NEG_INFINITY;
                        z[1] = f64::INFINITY;
                        break;
                    }
                    continue;
                }
                k += 1;
                v[k as usize] = q;
                z[k as usize] = s;
                z[k as usize + 1] = f64::INFINITY;
                break;
            }
        }
        if k < 0 {
            continue;
        }
        let mut j = 0usize;
        for x in 0..w {
            while z[j + 1] < x as f64 {
                j += 1;
            }
            let q = v[j];
            let sy = near[y * w + q] as usize;
            let (ex, ey) = ((x as f64 - q as f64) * lat.dx, (y as f64 - sy as f64) * lat.dy);
            dist[y * w + x] = (ex * ex + ey * ey).sqrt();
            label[y * w + x] = sites[sy * w + q];
        }
    }
    (dist, label)
}

/// The cells of the least-cost path from a cell down a core's distance surface to the core (the cell first, a
/// cell of the core last): each step to the neighbour the distance came through.
fn trace(dist: &[f64], res: &[f64], lat: &Lattice, ids: &[u32], from: usize, core: u32) -> Result<Vec<usize>> {
    let mut path = vec![from];
    let mut c = from;
    while ids[c] != core {
        let mut best: Option<(f64, usize)> = None;
        for (nb, len) in lat.around(c) {
            if !passable(res[nb]) || dist[nb] >= dist[c] {
                continue;
            }
            let v = dist[nb] + step(res[c], res[nb], len);
            if best.is_none_or(|(b, _)| v < b) {
                best = Some((v, nb));
            }
        }
        let Some((_, nb)) = best else { return Err(Error::msg("a least-cost path could not be traced back")) };
        c = nb;
        path.push(c);
        if path.len() > lat.cells() {
            return Err(Error::msg("a least-cost path could not be traced back"));
        }
    }
    Ok(path)
}

/// The length of a path of cells, in map units.
fn path_length(path: &[usize], lat: &Lattice) -> f64 {
    path.windows(2)
        .map(|p| {
            let (a, b) = (p[0], p[1]);
            let (dr, dc) = ((b / lat.w) as i64 - (a / lat.w) as i64, (b % lat.w) as i64 - (a % lat.w) as i64);
            lat.length(dr, dc)
        })
        .sum()
}

/// The cores of a map of ids over a resistance: their sorted ids, and every cell's core (index + 1, 0 none; a
/// core cell of an impassable resistance is none).
fn core_ids(cores: &[f64], res: &[f64]) -> Result<(Vec<i64>, Vec<u32>)> {
    let is_core = |c: usize| cores[c].is_finite() && cores[c] != 0.0 && passable(res[c]);
    let mut found: BTreeSet<i64> = BTreeSet::new();
    for (c, &v) in cores.iter().enumerate() {
        if is_core(c) {
            if v.fract() != 0.0 {
                return Err(Error::msg(format!("cores holds whole ids, not {v}")));
            }
            found.insert(v as i64);
        }
    }
    let names: Vec<i64> = found.into_iter().collect();
    let index: BTreeMap<i64, u32> = names.iter().enumerate().map(|(k, &i)| (i, k as u32 + 1)).collect();
    let ids = (0..cores.len()).map(|c| if is_core(c) { index[&(cores[c] as i64)] } else { 0 }).collect();
    Ok((names, ids))
}

/// A link between two cores (by index into the sorted ids).
struct Link {
    a: usize,
    b: usize,
    euclid: f64,
    lcd: f64,
    plen: f64,
    reason: String,
    path: Vec<usize>,
}

impl Link {
    fn new(a: usize, b: usize, reason: &str) -> Link {
        Link { a, b, euclid: f64::NAN, lcd: f64::NAN, plen: f64::NAN, reason: reason.into(), path: Vec::new() }
    }

    fn kept(&self) -> bool {
        self.reason.is_empty()
    }
}

/// The pairs of cores whose zones (a label per cell, 0 none) touch (8 neighbours).
fn touching(label: &[u32], lat: &Lattice) -> BTreeSet<(usize, usize)> {
    let mut s = BTreeSet::new();
    for c in 0..lat.cells() {
        let l = label[c];
        if l == 0 {
            continue;
        }
        for (nb, _) in lat.around(c) {
            let m = label[nb];
            if m > 0 && m != l {
                s.insert((l.min(m) as usize - 1, l.max(m) as usize - 1));
            }
        }
    }
    s
}

/// A link's search: its index, LCD, path, the path's length and the id of a core it crosses.
type Found = (usize, f64, Vec<usize>, f64, Option<i64>);

struct Pathways;

impl Operator for Pathways {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "linkage.pathways",
            "The least-cost links between core areas and the corridors around them, as Linkage Mapper's Linkage \
             Pathways (McRae and Kavanagh 2011): from every core the cost-weighted distance over a resistance (8 \
             neighbours, a step the two cells' mean resistance times its length, sqrt(2) diagonally; a cell without \
             a value or of a resistance of 0 or less is impassable); the cores to link, by `adjacency`: those whose \
             allocation zones touch; their least-cost distance (LCD, the least of the two cores' distances added) \
             and path, a link dropped past `max_euclidean` or `max_cwd`, when its path crosses a third core \
             (`drop_crossing`) or when it is not among a core's `nearest`; and the corridors, the normalized \
             least-cost corridor of each link (the two distances added, less the LCD) merged by their least, \
             cut at `width`. The cores are a map of whole ids on the resistance's grid (0 or no value: not a \
             core); a core cell of an impassable resistance is no core cell. The cost-weighted distances of the \
             cores in links are kept in memory together. The `euclidean` distance of `links` is between the centres \
             of the nearest cells of the two cores, not between polygon edges as Linkage Mapper's.",
            Access::Global,
        )
        .input(PortSpec::new("cores", Type::Map, "the core areas: whole ids, 0 or no value elsewhere"))
        .input(PortSpec::new("resistance", Type::Map, "the resistance, above 0 (0 or no value: impassable)"))
        .param(
            PortSpec::optional(
                "adjacency",
                Type::Text,
                "the cores to link: cost (their cost allocation zones touch, the default), euclidean (their \
                 Euclidean allocation zones touch) or all (every pair)",
            )
            .choices(["cost", "euclidean", "all"]),
        )
        .param(PortSpec::optional("max_cwd", Type::Number, "drop the links of a least-cost distance above it"))
        .param(PortSpec::optional(
            "max_euclidean",
            Type::Number,
            "drop the links of cores further apart than this, in map units (between the nearest cells' centres)",
        ))
        .param(PortSpec::optional(
            "drop_crossing",
            Type::Bool,
            "drop a link whose least-cost path crosses a third core (true)",
        ))
        .param(PortSpec::optional(
            "nearest",
            Type::Number,
            "keep a link only if it is among the k of least LCD of one of its cores (all kept links by default)",
        ))
        .param(PortSpec::optional(
            "width",
            Type::Number,
            "the corridors' cutoff: no cell of a normalized least-cost distance above it, in cost units (none: the \
             whole mosaic)",
        ))
        .output(PortSpec::new(
            "corridors",
            Type::Map,
            "the least of the kept links' normalized corridors (no value beyond `width`, or without a link)",
        ))
        .output(PortSpec::new(
            "links",
            Type::Table,
            "a row per pair of cores: from, to, euclidean, lcd, path_length, cwd_to_path (lcd / path_length), kept \
             and the reason it is not ('' kept; max_euclidean, unreachable, max_cwd, crosses core \
             <id>, not nearest)",
        ))
        .output(PortSpec::new("paths", Type::Vector, "the kept links' least-cost paths, through the cells' centres"))
        .output(PortSpec::new("cwd", Type::Map, "the cost-weighted distance to the nearest core"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let cores = args.map("cores")?;
        let resm = args.map("resistance")?;
        let (g, rg) = (&cores.grid, &resm.grid);
        if g.width != rg.width || g.height != rg.height || g.transform != rg.transform {
            return Err(fail(format!(
                "cores ({}x{} cells) and resistance ({}x{}) are not on one grid: raster.match puts them on one",
                g.width, g.height, rg.width, rg.height
            )));
        }
        let adjacency: String = args.param("adjacency", "cost".to_string())?;
        if !["cost", "euclidean", "all"].contains(&adjacency.as_str()) {
            return Err(fail(format!("adjacency is cost, euclidean or all, not {adjacency:?}")));
        }
        let max_cwd: Option<f64> = args.opt_param("max_cwd")?;
        let max_euclid: Option<f64> = args.opt_param("max_euclidean")?;
        let width: Option<f64> = args.opt_param("width")?;
        let nearest: Option<usize> = args.opt_param("nearest")?;
        let drop_crossing: bool = args.param("drop_crossing", true)?;
        if nearest == Some(0) {
            return Err(fail("nearest is at least 1".into()));
        }
        let lat = Lattice::of(g);
        let n = lat.cells();
        if n >= u32::MAX as usize {
            return Err(fail("the grid has too many cells".into()));
        }
        let res: &[f64] = &resm.data;

        // the cores: sorted ids, and every cell's core (index + 1, 0 none)
        let (names, ids) = core_ids(&cores.data, res).map_err(|e| fail(e.to_string()))?;
        if names.len() < 2 {
            return Err(fail(format!("two cores are needed to link, found {}", names.len())));
        }
        let m = names.len();
        let sources: Vec<(usize, u32)> =
            ids.iter().enumerate().filter(|&(_, &l)| l > 0).map(|(c, &l)| (c, l)).collect();
        crate::threads::ensure();

        // the cost-weighted distance to the nearest core, and the cost allocation
        let (nearest_cwd, zone) = cwd(res, &lat, &sources);

        // the pairs to link
        let adjacent: BTreeSet<(usize, usize)> = match adjacency.as_str() {
            "cost" => touching(&zone, &lat),
            "euclidean" => touching(&edt(&ids, &lat).1, &lat),
            _ => (0..m).flat_map(|a| (a + 1..m).map(move |b| (a, b))).collect(),
        };
        let mut links: Vec<Link> = adjacent.iter().map(|&(a, b)| Link::new(a, b, "")).collect();

        // the cores' cells, and the Euclidean distance between the cores of the pairs
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); m];
        for (c, &l) in ids.iter().enumerate() {
            if l > 0 {
                cells[l as usize - 1].push(c);
            }
        }
        let mut partners: Vec<Vec<usize>> = vec![Vec::new(); m];
        for (k, l) in links.iter().enumerate() {
            if l.kept() {
                partners[l.a].push(k);
            }
        }
        let euclid: Vec<(usize, f64)> = (0..m)
            .into_par_iter()
            .filter(|a| !partners[*a].is_empty())
            .flat_map_iter(|a| {
                let own: Vec<u32> = ids.iter().map(|&l| u32::from(l as usize == a + 1)).collect();
                let (d, _) = edt(&own, &lat);
                partners[a]
                    .iter()
                    .map(|&k| (k, cells[links[k].b].iter().map(|&c| d[c]).fold(f64::INFINITY, f64::min)))
                    .collect::<Vec<_>>()
            })
            .collect();
        for (k, e) in euclid {
            links[k].euclid = e;
            if max_euclid.is_some_and(|x| e > x) {
                links[k].reason = "max_euclidean".into();
            }
        }

        // the cost-weighted distance from the cores in the links left
        let mut needed: Vec<usize> = links.iter().filter(|l| l.kept()).flat_map(|l| [l.a, l.b]).collect();
        needed.sort_unstable();
        needed.dedup();
        if needed.len() as f64 * n as f64 * 8.0 > MEMORY {
            return Err(fail(format!(
                "{} cores in links over {n} cells need more than {} GB for their distances: link fewer cores \
                 (adjacency, max_euclidean) or a smaller grid",
                needed.len(),
                MEMORY / 1e9
            )));
        }
        let surfaces: BTreeMap<usize, Vec<f64>> = needed
            .par_iter()
            .map(|&a| (a, cwd(res, &lat, &cells[a].iter().map(|&c| (c, a as u32 + 1)).collect::<Vec<_>>()).0))
            .collect::<Vec<_>>()
            .into_iter()
            .collect();

        // each link's LCD and path
        let live: Vec<usize> = (0..links.len()).filter(|&k| links[k].kept()).collect();
        let found: Vec<Result<Found>> = live
            .par_iter()
            .map(|&k| {
                let (a, b) = (links[k].a, links[k].b);
                let (da, db) = (&surfaces[&a], &surfaces[&b]);
                let mut best = (f64::INFINITY, usize::MAX);
                for c in 0..n {
                    let s = da[c] + db[c];
                    if s < best.0 {
                        best = (s, c);
                    }
                }
                if best.1 == usize::MAX {
                    return Ok((k, f64::INFINITY, Vec::new(), f64::NAN, None));
                }
                let mut path = trace(da, res, &lat, &ids, best.1, a as u32 + 1)?;
                path.reverse();
                path.extend(trace(db, res, &lat, &ids, best.1, b as u32 + 1)?.into_iter().skip(1));
                let crossing = path
                    .iter()
                    .map(|&c| ids[c])
                    .find(|&l| l != 0 && l as usize != a + 1 && l as usize != b + 1)
                    .map(|l| names[l as usize - 1]);
                let plen = path_length(&path, &lat);
                Ok((k, best.0, path, plen, crossing))
            })
            .collect();
        let mut crossing: BTreeMap<usize, i64> = BTreeMap::new();
        for r in found {
            let (k, lcd, path, plen, cross) = r.map_err(|e| fail(e.to_string()))?;
            let l = &mut links[k];
            l.lcd = if lcd.is_finite() { lcd } else { f64::NAN };
            l.plen = plen;
            l.path = path;
            if !lcd.is_finite() {
                l.reason = "unreachable".into();
            } else if max_cwd.is_some_and(|x| lcd > x) {
                l.reason = "max_cwd".into();
            } else if let Some(c) = cross {
                crossing.insert(k, c);
            }
        }
        if drop_crossing {
            for (k, c) in crossing {
                if links[k].kept() {
                    links[k].reason = format!("crosses core {c}");
                }
            }
        }

        // only the k nearest of a core, by LCD (a link is kept if it is among those of either core)
        if let Some(k) = nearest {
            let mut by_core: Vec<Vec<(f64, usize)>> = vec![Vec::new(); m];
            for (i, l) in links.iter().enumerate().filter(|(_, l)| l.kept()) {
                by_core[l.a].push((l.lcd, i));
                by_core[l.b].push((l.lcd, i));
            }
            let mut keep = vec![false; links.len()];
            for list in &mut by_core {
                list.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
                for &(_, i) in list.iter().take(k) {
                    keep[i] = true;
                }
            }
            for (i, l) in links.iter_mut().enumerate() {
                if l.kept() && !keep[i] {
                    l.reason = "not nearest".into();
                }
            }
        }

        // the corridors: the least normalized corridor of the kept links
        let kept: Vec<&Link> = links.iter().filter(|l| l.kept()).collect();
        let mut corridors = vec![f64::NAN; n];
        corridors.par_chunks_mut(4096).enumerate().for_each(|(b, out)| {
            for (j, o) in out.iter_mut().enumerate() {
                let c = b * 4096 + j;
                let mut best = f64::INFINITY;
                for l in &kept {
                    let v = surfaces[&l.a][c] + surfaces[&l.b][c] - l.lcd;
                    if v < best {
                        best = v;
                    }
                }
                if best.is_finite() && width.is_none_or(|w| best <= w) {
                    *o = best;
                }
            }
        });

        // the outputs
        let mut t = Table::new(
            ["from", "to", "euclidean", "lcd", "path_length", "cwd_to_path", "kept", "reason"]
                .map(String::from)
                .to_vec(),
        );
        let mut reasons: Vec<String> = Vec::new();
        for l in &links {
            let code = reasons.iter().position(|r| *r == l.reason).unwrap_or_else(|| {
                reasons.push(l.reason.clone());
                reasons.len() - 1
            });
            t.rows.push(vec![
                names[l.a] as f64,
                names[l.b] as f64,
                l.euclid,
                l.lcd,
                l.plen,
                l.lcd / l.plen,
                f64::from(u8::from(l.kept())),
                (code + 1) as f64,
            ]);
        }
        t.labels.insert("reason".into(), reasons);
        let tf = g.transform;
        let centre =
            |c: usize| [tf[0] + ((c % lat.w) as f64 + 0.5) * tf[1], tf[3] + ((c / lat.w) as f64 + 0.5) * tf[5]];
        let paths = Vector {
            geometries: kept
                .iter()
                .map(|l| Geometry::LineString(l.path.iter().map(|&c| centre(c)).collect()))
                .collect(),
            attributes: vec![
                ("from".into(), Column::Numbers(kept.iter().map(|l| names[l.a] as f64).collect())),
                ("to".into(), Column::Numbers(kept.iter().map(|l| names[l.b] as f64).collect())),
                ("lcd".into(), Column::Numbers(kept.iter().map(|l| l.lcd).collect())),
            ],
            crs: crate::prepare::crs_of(g),
        };
        let nearest_cwd = nearest_cwd.into_iter().map(|v| if v.is_finite() { v } else { f64::NAN }).collect();
        Ok(outputs([
            ("corridors", Value::Map(Map::new(g.clone(), corridors))),
            ("links", Value::Table(t)),
            ("paths", Value::Vector(Arc::new(paths))),
            ("cwd", Value::Map(Map::new(g.clone(), nearest_cwd))),
        ]))
    }
}

/// The kept links of a `links` table (linkage.pathways'): (core, core, lcd of the table), the cores by index into
/// the sorted ids. All the rows if there is no `kept` column.
fn read_links(lt: &Table, index: &BTreeMap<i64, usize>) -> std::result::Result<Vec<(usize, usize, f64)>, String> {
    let col = |c: &str| lt.column(c).ok_or_else(|| format!("links has no column {c:?}"));
    let (from, to, lcd) = (col("from")?, col("to")?, col("lcd")?);
    let kept = lt.column("kept");
    let mut links: Vec<(usize, usize, f64)> = Vec::new();
    for i in 0..lt.rows.len() {
        if kept.as_ref().is_some_and(|k| k[i] != 1.0) {
            continue;
        }
        let find =
            |v: f64| index.get(&(v as i64)).copied().ok_or_else(|| format!("the core {v} of links is not in cores"));
        links.push((find(from[i])?, find(to[i])?, lcd[i]));
    }
    if links.is_empty() {
        return Err("links has no kept link to map".into());
    }
    Ok(links)
}

/// Each link's LCD from the cores' distances, checked against the table's.
fn check_lcds(
    links: &[(usize, usize, f64)],
    surfaces: &BTreeMap<usize, Vec<f64>>,
    names: &[i64],
    n: usize,
) -> std::result::Result<Vec<f64>, String> {
    links
        .par_iter()
        .map(|&(a, b, tl)| {
            let (da, db) = (&surfaces[&a], &surfaces[&b]);
            let l = (0..n).map(|c| da[c] + db[c]).fold(f64::INFINITY, f64::min);
            if !l.is_finite() {
                return Err(format!("the cores {} and {} are not connected", names[a], names[b]));
            }
            if (l - tl).abs() > 1e-6 * l.max(1.0) {
                return Err(format!(
                    "the lcd of the link {} to {} in links is {tl}, this resistance gives {l}: links is not \
                     linkage.pathways' on these cores and this resistance",
                    names[a], names[b]
                ));
            }
            Ok(l)
        })
        .collect()
}

/// Circuitscape's no-data value: a cell below it has no current.
const NODATA: f64 = -9999.0;

struct Pinchpoints;

impl Operator for Pinchpoints {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "linkage.pinchpoints",
            "The pinch points of the links between cores, as Linkage Mapper's Pinchpoint Mapper (McRae 2012) with \
             Circuitscape (McRae et al. 2008): in `pairwise` mode, for each kept link of `links` (the output of \
             linkage.pathways) the circuit between its two cores, inside the link's corridor only (the cells whose \
             normalized least-cost distance, the two cores' distances added less the link's LCD, is within \
             `width`; the resistance outside it a barrier; the cores' cells in the corridor the focal regions, 1 \
             A across), and the `current` the largest of the links' (the core cells have no value, as Linkage \
             Mapper's); in `all_to_one` mode one circuit over the union of the corridors, every core grounded in \
             turn with the others injecting, and the cumulative current. The effective resistance of a link is \
             Circuitscape's times the cell's height, in the units of the cost-weighted distance, with `lcd` over \
             it as `cwd_to_resistance` (low: the link is a pinch point or a bottleneck; not given with `square`). \
             The resistance is the cost surface of linkage.pathways (a cell without a value or of 0 or less is a \
             barrier). The links' circuits run in parallel; the cost-weighted distances of the cores in links are \
             kept in memory together.",
            Access::Global,
        )
        .input(PortSpec::new("cores", Type::Map, "the core areas: whole ids, 0 or no value elsewhere"))
        .input(PortSpec::new("resistance", Type::Map, "the resistance, above 0 (0 or no value: impassable)"))
        .input(PortSpec::new(
            "links",
            Type::Table,
            "the links to map: from, to, lcd and kept (the rows kept; all if no `kept` column), as linkage.pathways' \
             `links`",
        ))
        .param(PortSpec::new(
            "width",
            Type::Number,
            "the corridors' cutoff: the cells of a normalized least-cost distance up to it are in the circuit, in cost \
             units",
        ))
        .param(
            PortSpec::optional(
                "mode",
                Type::Text,
                "pairwise (a circuit per link, between its two cores, the default) or all_to_one (one circuit over \
                 the corridors of all the links, every core a focal region)",
            )
            .choices(["pairwise", "all_to_one"]),
        )
        .param(PortSpec::optional(
            "square",
            Type::Bool,
            "square the resistances for the circuit, as Linkage Mapper's option, to weight the pinch points more \
             (false)",
        ))
        .output(PortSpec::new(
            "current",
            Type::Map,
            "the current: the largest of the links' (pairwise) or the cumulative one (all_to_one); no value outside the \
             corridors and in the cores",
        ))
        .output(PortSpec::new(
            "links",
            Type::Table,
            "a row per link mapped: from, to, lcd, resistance (the effective resistance, in cost units; pairwise), \
             cwd_to_resistance (lcd / resistance, not with `square`) and nodes (the cells of its circuit)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let cores = args.map("cores")?;
        let resm = args.map("resistance")?;
        let (g, rg) = (&cores.grid, &resm.grid);
        if g.width != rg.width || g.height != rg.height || g.transform != rg.transform {
            return Err(fail(format!(
                "cores ({}x{} cells) and resistance ({}x{}) are not on one grid: raster.match puts them on one",
                g.width, g.height, rg.width, rg.height
            )));
        }
        let lt = match args.get("links").ok_or_else(|| args.missing("links"))? {
            Value::Table(t) => t,
            v => return Err(args.wrong("links", "table", v)),
        };
        let width: f64 = args.opt_param("width")?.ok_or_else(|| fail("width is required".into()))?;
        if !width.is_finite() || width < 0.0 {
            return Err(fail(format!("width is a distance of 0 or more, not {width}")));
        }
        let mode: String = args.param("mode", "pairwise".to_string())?;
        if !["pairwise", "all_to_one"].contains(&mode.as_str()) {
            return Err(fail(format!("mode is pairwise or all_to_one, not {mode:?}")));
        }
        let square: bool = args.param("square", false)?;
        let lat = Lattice::of(g);
        let n = lat.cells();
        if n >= u32::MAX as usize {
            return Err(fail("the grid has too many cells".into()));
        }
        let res: &[f64] = &resm.data;
        let threads = crate::threads::ensure();

        // the cores, and the links to map: (core, core, lcd of the table)
        let (names, ids) = core_ids(&cores.data, res).map_err(|e| fail(e.to_string()))?;
        let index: BTreeMap<i64, usize> = names.iter().enumerate().map(|(k, &i)| (i, k)).collect();
        let links = read_links(lt, &index).map_err(fail)?;

        // the cost-weighted distance from the cores in the links
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); names.len()];
        for (c, &l) in ids.iter().enumerate() {
            if l > 0 {
                cells[l as usize - 1].push(c);
            }
        }
        let mut needed: Vec<usize> = links.iter().flat_map(|l| [l.0, l.1]).collect();
        needed.sort_unstable();
        needed.dedup();
        if needed.len() as f64 * n as f64 * 8.0 > MEMORY {
            return Err(fail(format!(
                "{} cores in links over {n} cells need more than {} GB for their distances: map fewer links or a \
                 smaller grid",
                needed.len(),
                MEMORY / 1e9
            )));
        }
        let surfaces: BTreeMap<usize, Vec<f64>> = needed
            .par_iter()
            .map(|&a| (a, cwd(res, &lat, &cells[a].iter().map(|&c| (c, a as u32 + 1)).collect::<Vec<_>>()).0))
            .collect::<Vec<_>>()
            .into_iter()
            .collect();

        // each link's LCD, to be the table's
        let lcds = check_lcds(&links, &surfaces, &names, n).map_err(fail)?;

        // the conductance of the circuit where a cell is in it: 1 / resistance (squared or not)
        let conductance = |inside: &dyn Fn(usize) -> bool| -> Vec<f64> {
            (0..n).map(|c| if inside(c) { 1.0 / if square { res[c] * res[c] } else { res[c] } } else { 0.0 }).collect()
        };
        let mut rows: Vec<(usize, f64, f64)> = Vec::new();
        let current: Vec<f64> = if mode == "pairwise" {
            let (cur, found) = (0..links.len())
                .into_par_iter()
                .try_fold(
                    || (vec![f64::NAN; n], Vec::<(usize, f64, f64)>::new()),
                    |(mut acc, mut found), k| -> Result<_> {
                        let (a, b, _) = links[k];
                        let (da, db) = (&surfaces[&a], &surfaces[&b]);
                        let inside = |c: usize| da[c] + db[c] - lcds[k] <= width;
                        let gm = conductance(&inside);
                        // the focal regions: the cores' cells in the corridor, 1 and 2
                        let mut pm = vec![0.0; n];
                        for (id, core) in [(1.0, a), (2.0, b)] {
                            for &c in cells[core].iter().filter(|&&c| inside(c)) {
                                pm[c] = id;
                            }
                        }
                        let pts = FocalPoints::of(&pm, lat.w, lat.h);
                        let at: Vec<i64> =
                            pts.rows.iter().zip(&pts.cols).map(|(r, c)| (r * lat.w + c) as i64).collect();
                        let regions = pts.has_regions();
                        let out = ffi::circuit_pairwise(
                            &gm,
                            &[],
                            lat.w as i64,
                            lat.h as i64,
                            &at,
                            if regions { &pts.ids } else { &[] },
                            &[0u8; 4],
                            false,
                            false,
                            &[1, 0, 1, 0, 0, 0, 0, 0],
                        )?;
                        let r = out.resistances[1];
                        if r < 0.0 {
                            return Err(fail(format!(
                                "the cores {} and {} are not connected inside their corridor",
                                names[a], names[b]
                            )));
                        }
                        for (c, a) in acc.iter_mut().enumerate() {
                            let v = out.cum[c];
                            if pm[c] > 0.0 || gm[c] == 0.0 || v <= NODATA || v.is_nan() {
                                continue;
                            }
                            if a.is_nan() || v > *a {
                                *a = v;
                            }
                        }
                        found.push((k, r * lat.dy, gm.iter().filter(|x| **x > 0.0).count() as f64));
                        Ok((acc, found))
                    },
                )
                .try_reduce(
                    || (vec![f64::NAN; n], Vec::new()),
                    |(mut a, mut fa), (b, fb)| {
                        for (x, y) in a.iter_mut().zip(&b) {
                            if !y.is_nan() && (x.is_nan() || y > x) {
                                *x = *y;
                            }
                        }
                        fa.extend(fb);
                        Ok((a, fa))
                    },
                )?;
            rows = found;
            rows.sort_by_key(|r| r.0);
            cur
        } else {
            // the union of the corridors, and every core's cells (all the cores of the map)
            let mut inside = vec![false; n];
            inside.par_chunks_mut(4096).enumerate().for_each(|(b, out)| {
                for (j, o) in out.iter_mut().enumerate() {
                    let c = b * 4096 + j;
                    *o = ids[c] > 0
                        || links
                            .iter()
                            .enumerate()
                            .any(|(k, l)| surfaces[&l.0][c] + surfaces[&l.1][c] - lcds[k] <= width);
                }
            });
            let gm = conductance(&|c| inside[c]);
            let pm: Vec<f64> = ids.iter().map(|&l| l as f64).collect();
            let mut circuit =
                Circuit { g: gm, poly: Vec::new(), points: FocalPoints::of(&pm, lat.w, lat.h), nx: lat.w, ny: lat.h };
            if circuit.points.unique().len() < 2 {
                return Err(fail("two cores are needed for a circuit".into()));
            }
            let o = OneToAllOptions {
                one_to_all: false,
                strengths: None,
                pairs: None,
                cum: true,
                max: false,
                focal_zero: true,
                volt_maps: false,
                cur_maps: false,
                avg_resistances: false,
                four_neighbors: false,
                batch: 4 * threads,
            };
            let r = one_to_all(ctx, &mut circuit, &o)?;
            r.cum
                .iter()
                .enumerate()
                .map(|(c, &v)| if !inside[c] || ids[c] > 0 || v <= NODATA || v.is_nan() { f64::NAN } else { v })
                .collect()
        };

        // the outputs
        let mut t =
            Table::new(["from", "to", "lcd", "resistance", "cwd_to_resistance", "nodes"].map(String::from).to_vec());
        for (k, &(a, b, tl)) in links.iter().enumerate() {
            let (r, nodes) = rows.get(k).map_or((f64::NAN, f64::NAN), |r| (r.1, r.2));
            let ratio = if square { f64::NAN } else { tl / r };
            t.rows.push(vec![names[a] as f64, names[b] as f64, tl, r, ratio, nodes]);
        }
        Ok(outputs([("current", Value::Map(Map::new(g.clone(), current))), ("links", Value::Table(t))]))
    }
}

/// The offsets (row, column) of the cells whose centre is at a distance (map units) above `inner` and up to
/// `outer` from a cell's: an annulus, or a circle (no `inner`: the centre too).
fn footprint(lat: &Lattice, outer: f64, inner: Option<f64>) -> Vec<(i64, i64)> {
    let (kr, kc) = ((outer / lat.dy).floor() as i64, (outer / lat.dx).floor() as i64);
    let mut v = Vec::new();
    for dr in -kr..=kr {
        for dc in -kc..=kc {
            let (x, y) = (dc as f64 * lat.dx, dr as f64 * lat.dy);
            let d = (x * x + y * y).sqrt();
            if d <= outer && inner.is_none_or(|i| d > i) {
                v.push((dr, dc));
            }
        }
    }
    v
}

/// The least (or greatest) finite value of a surface over the footprint around each cell, NaN where there is
/// none (ArcGIS's focal statistic over data cells, the grid's edge ignored).
fn focal(data: &[f64], lat: &Lattice, offs: &[(i64, i64)], least: bool) -> Vec<f64> {
    let mut out = vec![f64::NAN; lat.cells()];
    out.par_chunks_mut(lat.w).enumerate().for_each(|(r, row)| {
        for (k, o) in row.iter_mut().enumerate() {
            let mut best = f64::NAN;
            for &(dr, dc) in offs {
                let (rr, kk) = (r as i64 + dr, k as i64 + dc);
                if rr < 0 || kk < 0 || rr >= lat.h as i64 || kk >= lat.w as i64 {
                    continue;
                }
                let v = data[rr as usize * lat.w + kk as usize];
                if v.is_finite() && (best.is_nan() || if least { v < best } else { v > best }) {
                    best = v;
                }
            }
            *o = best;
        }
    });
    out
}

/// `acc` takes `v` if it is greater (NaN: nothing yet).
fn raise(acc: &mut f64, v: f64) {
    if !v.is_nan() && (acc.is_nan() || v > *acc) {
        *acc = v;
    }
}

struct Barriers;

impl Operator for Barriers {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "linkage.barriers",
            "The barriers in the links between cores, as Linkage Mapper's Barrier Mapper (McRae et al. 2012): for \
             each kept link of `links` (the output of linkage.pathways) and each radius R from `min_radius` to \
             `max_radius` by `step` (map units), the improvement of restoring a circle of radius R around a cell: \
             the link's LCD less the least cost-weighted distance of each core over the ring of cells around the \
             cell (those whose centre is at a distance above R - 1 and up to R map units: Linkage Mapper's \
             annulus), less the cost of crossing the restored circle (its diameter 2R times `restored`), per unit \
             of restored distance (over 2R); `percent` is that improvement as a percentage of the LCD. A cell whose \
             ring touches a core has no value. The links are mosaicked by their greatest (`combine` max) or their \
             sum (sum: a negative or missing value counts 0), the negative cells left without a value: the barrier \
             `improvement` (the centres of the circles), over the radii the greatest, with the `radius` that gave \
             it (the least on a tie). `circles` grows each centre out to its circle (the greatest value of a \
             centre within R). `cwd_threshold` keeps each core's distance only below it. The trim of the \
             circles by the resistance (Linkage Mapper's optional one) is not made. The cost-weighted distances of \
             the cores in links are kept in memory together.",
            Access::Global,
        )
        .input(PortSpec::new("cores", Type::Map, "the core areas: whole ids, 0 or no value elsewhere"))
        .input(PortSpec::new("resistance", Type::Map, "the resistance, above 0 (0 or no value: impassable)"))
        .input(PortSpec::new(
            "links",
            Type::Table,
            "the links to map: from, to, lcd and kept (the rows kept; all if no `kept` column), as linkage.pathways' \
             `links`",
        ))
        .param(PortSpec::new(
            "min_radius",
            Type::Number,
            "the least radius of the restored circles, in map units (at least the cell's size)",
        ))
        .param(PortSpec::optional("max_radius", Type::Number, "the greatest radius, in map units (min_radius)"))
        .param(PortSpec::optional(
            "step",
            Type::Number,
            "the step between radii, in map units (required, above 0, when max_radius is above min_radius)",
        ))
        .param(PortSpec::optional(
            "restored",
            Type::Number,
            "the cost of a restored cell per map unit (1, as Linkage Mapper's)",
        ))
        .param(
            PortSpec::optional(
                "combine",
                Type::Text,
                "how the links' scores are mosaicked: max (the default) or sum (a negative or missing score counts 0)",
            )
            .choices(["max", "sum"]),
        )
        .param(PortSpec::optional(
            "cwd_threshold",
            Type::Number,
            "keep each core's cost-weighted distance only below it, in cost units (all of it by default)",
        ))
        .output(PortSpec::new(
            "improvement",
            Type::Map,
            "the barrier score at the centre of the circle: the greatest over radii; no value where it is negative \
             or the ring touches a core",
        ))
        .output(PortSpec::new(
            "circles",
            Type::Map,
            "the improvement grown out to the circles: the greatest value of a centre within the radius of the cell",
        ))
        .output(PortSpec::new("percent", Type::Map, "the improvement as a percentage of the link's LCD, the same way"))
        .output(PortSpec::new("radius", Type::Map, "the radius that gave the improvement, in map units"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |m: String| Error::msg(format!("node {:?}: {m}", args.node));
        let cores = args.map("cores")?;
        let resm = args.map("resistance")?;
        let (g, rg) = (&cores.grid, &resm.grid);
        if g.width != rg.width || g.height != rg.height || g.transform != rg.transform {
            return Err(fail(format!(
                "cores ({}x{} cells) and resistance ({}x{}) are not on one grid: raster.match puts them on one",
                g.width, g.height, rg.width, rg.height
            )));
        }
        let lt = match args.get("links").ok_or_else(|| args.missing("links"))? {
            Value::Table(t) => t,
            v => return Err(args.wrong("links", "table", v)),
        };
        let lat = Lattice::of(g);
        let n = lat.cells();
        if n >= u32::MAX as usize {
            return Err(fail("the grid has too many cells".into()));
        }
        let cell = lat.dx.max(lat.dy);
        let min_r: f64 = args.opt_param("min_radius")?.ok_or_else(|| fail("min_radius is required".into()))?;
        if !min_r.is_finite() || min_r < cell {
            return Err(fail(format!("min_radius is at least the cell's size ({cell}) in map units, not {min_r}")));
        }
        let max_r: f64 = args.param("max_radius", min_r)?;
        if !max_r.is_finite() || max_r < min_r {
            return Err(fail(format!("max_radius is at least min_radius ({min_r}), not {max_r}")));
        }
        let step: Option<f64> = args.opt_param("step")?;
        let count = if max_r == min_r {
            1
        } else {
            let s = step.ok_or_else(|| fail("step is required when max_radius is above min_radius".into()))?;
            if !s.is_finite() || s <= 0.0 {
                return Err(fail(format!("step is above 0, not {s}")));
            }
            let k = ((max_r - min_r) / s + 1e-9).floor();
            if k > 1000.0 {
                return Err(fail(format!("{} radii: raise step", k + 1.0)));
            }
            k as usize + 1
        };
        let step = step.unwrap_or(0.0);
        let restored: f64 = args.param("restored", 1.0)?;
        if !restored.is_finite() || restored < 0.0 {
            return Err(fail(format!("restored is a cost of 0 or more, not {restored}")));
        }
        let combine: String = args.param("combine", "max".to_string())?;
        if !["max", "sum"].contains(&combine.as_str()) {
            return Err(fail(format!("combine is max or sum, not {combine:?}")));
        }
        let sum = combine == "sum";
        let thresh: Option<f64> = args.opt_param("cwd_threshold")?;
        let res: &[f64] = &resm.data;
        crate::threads::ensure();

        // the cores, and the links to map
        let (names, ids) = core_ids(&cores.data, res).map_err(|e| fail(e.to_string()))?;
        let index: BTreeMap<i64, usize> = names.iter().enumerate().map(|(k, &i)| (i, k)).collect();
        let links = read_links(lt, &index).map_err(fail)?;

        // the cost-weighted distance from the cores in the links (a radius' focal surfaces are as many again)
        let mut cells: Vec<Vec<usize>> = vec![Vec::new(); names.len()];
        for (c, &l) in ids.iter().enumerate() {
            if l > 0 {
                cells[l as usize - 1].push(c);
            }
        }
        let mut needed: Vec<usize> = links.iter().flat_map(|l| [l.0, l.1]).collect();
        needed.sort_unstable();
        needed.dedup();
        if needed.len() as f64 * n as f64 * 16.0 > MEMORY {
            return Err(fail(format!(
                "{} cores in links over {n} cells need more than {} GB for their distances: map fewer links or a \
                 smaller grid",
                needed.len(),
                MEMORY / 1e9
            )));
        }
        let mut surfaces: BTreeMap<usize, Vec<f64>> = needed
            .par_iter()
            .map(|&a| (a, cwd(res, &lat, &cells[a].iter().map(|&c| (c, a as u32 + 1)).collect::<Vec<_>>()).0))
            .collect::<Vec<_>>()
            .into_iter()
            .collect();
        let lcds = check_lcds(&links, &surfaces, &names, n).map_err(fail)?;
        // no data (NaN) where a distance is out of reach or past the threshold
        for s in surfaces.values_mut() {
            for v in s.iter_mut() {
                if !v.is_finite() || thresh.is_some_and(|t| *v >= t) {
                    *v = f64::NAN;
                }
            }
        }

        let nan = || vec![f64::NAN; n];
        let (mut imp, mut circles, mut pct, mut radius) = (nan(), nan(), nan(), nan());
        for k in 0..count {
            let r = min_r + k as f64 * step;
            let dia = 2.0 * r;
            let ring = footprint(&lat, r, Some(r - 1.0));
            let disk = footprint(&lat, r, None);
            let near: BTreeMap<usize, Vec<f64>> = needed
                .par_iter()
                .map(|&a| (a, focal(&surfaces[&a], &lat, &ring, true)))
                .collect::<Vec<_>>()
                .into_iter()
                .collect();
            // the links' mosaics of the scores and of the percents
            let mut ms = vec![if sum { 0.0 } else { f64::NAN }; n];
            let mut mp = ms.clone();
            ms.par_iter_mut().zip(mp.par_iter_mut()).enumerate().for_each(|(c, (s, p))| {
                for (&(a, b, _), &lcd) in links.iter().zip(&lcds) {
                    let (fa, fb) = (near[&a][c], near[&b][c]);
                    // a ring that touches a core (0), or has no distance, gives no value
                    let (v, q) = if fa > 0.0 && fb > 0.0 {
                        let v = (lcd - fa - fb - dia * restored) / dia;
                        (v, if lcd > 0.0 { 100.0 * v / lcd } else { f64::NAN })
                    } else {
                        (f64::NAN, f64::NAN)
                    };
                    if sum {
                        *s += if v.is_nan() || v < 0.0 { 0.0 } else { v };
                        *p += if q.is_nan() || q < 0.0 { 0.0 } else { q };
                    } else {
                        raise(s, v);
                        raise(p, q);
                    }
                }
                // the centres: no negative
                if *s < 0.0 {
                    *s = f64::NAN;
                }
                if *p < 0.0 {
                    *p = f64::NAN;
                }
            });
            let grown = focal(&ms, &lat, &disk, false);
            for c in 0..n {
                if !ms[c].is_nan() && (imp[c].is_nan() || ms[c] > imp[c]) {
                    radius[c] = r;
                }
                raise(&mut imp[c], ms[c]);
                raise(&mut pct[c], mp[c]);
                raise(&mut circles[c], grown[c]);
            }
        }
        Ok(outputs([
            ("improvement", Value::Map(Map::new(g.clone(), imp))),
            ("circles", Value::Map(Map::new(g.clone(), circles))),
            ("percent", Value::Map(Map::new(g.clone(), pct))),
            ("radius", Value::Map(Map::new(g.clone(), radius))),
        ]))
    }
}
