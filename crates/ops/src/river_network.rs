//! The connectivity of rivers, as hydrographr's (get_distance, get_buffer_along_the_network, get_centrality) and
//! riverconn's dendritic connectivity index: distances along the network between points, the stretches up and down
//! the river from each point, the centrality of the links and the connectivity left by barriers (dams, weirs,
//! culverts) and gained by removing each. In Rust, over the D8 directions of `hydro`.

use std::collections::HashMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Geometry, Vector};

use crate::hydro::cells;
use crate::rivers::{Cells, cells_of_points, dem_and_flats, drainage, on_dem, points_of, snap_distance, snap_inputs};
use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(NetworkDistance));
    r.add(Arc::new(AlongNetwork));
    r.add(Arc::new(Centrality));
    r.add(Arc::new(Dci));
}

/// The length of the step from a cell to the one it drains to: a side or the diagonal of the cell (the smaller side
/// out of the grid, as hydro.stream_links' lengths).
pub(crate) fn step(k: usize, to: i64, nx: usize, dx: f64, dy: f64) -> f64 {
    if to < 0 {
        return dx.min(dy);
    }
    let to = to as usize;
    match (k / nx != to / nx, k % nx != to % nx) {
        (true, true) => dx.hypot(dy),
        (true, false) => dy,
        _ => dx,
    }
}

/// The path from a cell down to the edge of the grid: each cell and its distance from the first.
fn path_down(k: usize, down: &[i64], nx: usize, dx: f64, dy: f64) -> Vec<(usize, f64)> {
    let mut out = vec![(k, 0.0)];
    let mut at = k;
    while down[at] >= 0 && out.len() <= down.len() {
        let d = out.last().unwrap().1 + step(at, down[at], nx, dx, dy);
        at = down[at] as usize;
        out.push((at, d));
    }
    out
}

/// The cell of each point of a node: moved onto its streams (when given).
fn placed(ctx: &mut Ctx, args: &Args, dem: &Map) -> Result<Vec<Option<usize>>> {
    let (_, pts) = points_of(args, "points", &dem.grid)?;
    let streams = args
        .opt_map("streams")?
        .map(|s| on_dem(dem, s, "streams").map(|_| crate::rivers::stream_cells(s)))
        .transpose()?;
    let distance = snap_distance(args, "snap", &dem.grid)?;
    Ok(cells_of_points(ctx, &pts, &dem.grid, streams.as_deref(), distance))
}

struct NetworkDistance;

impl Operator for NetworkDistance {
    fn signature(&self) -> OpSignature {
        snap_inputs(dem_and_flats(OpSignature::new(
            "hydro.network_distance",
            "The distance along the river network between every two points, as hydrographr's get_distance: down \
             the D8 directions from each to where their waters meet (fish swimming between two gauges, two \
             populations), and the straight distance; whether one drains into the other. Points in different \
             basins are not connected.",
            Access::Global,
        )))
        .input(PortSpec::new("points", Type::Vector, "the points (moved onto the streams when they are given)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per two points: from, to (1..), network (map units; m for longitude / latitude; no value when not \
             connected), euclidean, connected (1 or 0), flow (1 when one drains into the other)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let r = |ctx: &mut Ctx| -> Result<Outputs> {
            let dem = args.map("dem")?;
            let down = drainage(args, dem)?;
            let at = placed(ctx, args, dem)?;
            let (dx, dy) = cells(&dem.grid);
            let nx = dem.grid.width;
            let c = Cells::of(&dem.grid);
            let paths: Vec<Option<Vec<(usize, f64)>>> =
                at.iter().map(|k| k.map(|k| path_down(k, &down, nx, dx, dy))).collect();
            let (ex, ey) = (dx / c.rx, dy / c.ry);
            let mut t =
                Table::new(["from", "to", "network", "euclidean", "connected", "flow"].map(String::from).to_vec());
            for i in 0..at.len() {
                let index: Option<HashMap<usize, f64>> = paths[i].as_ref().map(|p| p.iter().copied().collect());
                for j in i + 1..at.len() {
                    let mut row = vec![(i + 1) as f64, (j + 1) as f64, f64::NAN, f64::NAN, 0.0, 0.0];
                    if let (Some(index), Some(pj), Some(a), Some(b)) = (&index, &paths[j], at[i], at[j]) {
                        let (pa, pb) = (c.centre(a), c.centre(b));
                        row[3] = ((pa[0] - pb[0]) * ex).hypot((pa[1] - pb[1]) * ey);
                        if let Some(&(m, db)) = pj.iter().find(|(k, _)| index.contains_key(k)) {
                            row[2] = index[&m] + db;
                            row[4] = 1.0;
                            row[5] = f64::from(u8::from(m == a || m == b));
                        }
                    }
                    t.rows.push(row);
                }
            }
            Ok(outputs([("table", Value::Table(t))]))
        };
        r(ctx).map_err(fail)
    }
}

struct AlongNetwork;

impl Operator for AlongNetwork {
    fn signature(&self) -> OpSignature {
        dem_and_flats(OpSignature::new(
            "hydro.along_network",
            "The stretches of river up and down from every point within a distance along the network, as \
             hydrographr's get_buffer_along_the_network: downstream along the D8 directions, upstream along every \
             stream that flows into it (a buffer that follows the river, not a circle: the reach of a fish, of a \
             spill below a mine). Each cell given the point nearest to it along the network.",
            Access::Global,
        ))
        .input(PortSpec::new("points", Type::Vector, "the points, moved onto the streams"))
        .input(PortSpec::new("streams", Type::Map, "the streams: non-zero on their cells"))
        .output(PortSpec::new("map", Type::Map, "the point (1..) of each cell within reach, 0 elsewhere"))
        .output(PortSpec::new(
            "distance",
            Type::Map,
            "the distance along the network to that point (map units; m for longitude / latitude)",
        ))
        .param(PortSpec::optional("upstream", Type::Number, "how far upstream (0)"))
        .param(PortSpec::optional("downstream", Type::Number, "how far downstream (0)"))
        .param(PortSpec::optional(
            "snap",
            Type::Number,
            "the largest move of a point onto the streams, map units (300 m; 3 cells without a projection)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        let r = |ctx: &mut Ctx| -> Result<Outputs> {
            let dem = args.map("dem")?;
            let (up, dn): (f64, f64) = (args.param("upstream", 0.0)?, args.param("downstream", 0.0)?);
            if up.is_nan() || dn.is_nan() || up < 0.0 || dn < 0.0 {
                return Err(Error::msg("upstream and downstream are distances of 0 or more"));
            }
            let streams = args.map("streams")?;
            on_dem(dem, streams, "streams")?;
            let on = crate::rivers::stream_cells(streams);
            let down = drainage(args, dem)?;
            let at = placed(ctx, args, dem)?;
            let (dx, dy) = cells(&dem.grid);
            let n = dem.data.len();
            let nx = dem.grid.width;
            // the stream cells draining into each, in a row (compressed)
            let mut first = vec![0usize; n + 1];
            for k in 0..n {
                if on[k] && down[k] >= 0 {
                    first[down[k] as usize + 1] += 1;
                }
            }
            for k in 0..n {
                first[k + 1] += first[k];
            }
            let mut ups = vec![0usize; first[n]];
            let mut fill = first.clone();
            for k in 0..n {
                if on[k] && down[k] >= 0 {
                    let j = down[k] as usize;
                    ups[fill[j]] = k;
                    fill[j] += 1;
                }
            }
            let (mut label, mut dist) = (vec![0.0; n], vec![f64::INFINITY; n]);
            let reach = |k: usize, d: f64, id: f64, label: &mut Vec<f64>, dist: &mut Vec<f64>| {
                if d < dist[k] {
                    dist[k] = d;
                    label[k] = id;
                }
            };
            for (i, k) in at.iter().enumerate() {
                let Some(k) = *k else { continue };
                let id = (i + 1) as f64;
                for (c, d) in path_down(k, &down, nx, dx, dy) {
                    if d > dn * (1.0 + 1e-12) {
                        break;
                    }
                    reach(c, d, id, &mut label, &mut dist);
                }
                let mut stack = vec![(k, 0.0)];
                while let Some((c, d)) = stack.pop() {
                    for &u in &ups[first[c]..first[c + 1]] {
                        let du = d + step(u, c as i64, nx, dx, dy);
                        if du <= up * (1.0 + 1e-12) {
                            reach(u, du, id, &mut label, &mut dist);
                            stack.push((u, du));
                        }
                    }
                }
            }
            for k in 0..n {
                if dem.data[k].is_nan() {
                    label[k] = f64::NAN;
                }
                if !dist[k].is_finite() || dem.data[k].is_nan() {
                    dist[k] = f64::NAN;
                }
            }
            let g = &dem.grid;
            Ok(outputs([
                ("map", Value::Map(Map::new(g.clone(), label))),
                ("distance", Value::Map(Map::new(g.clone(), dist))),
            ]))
        };
        r(ctx).map_err(fail)
    }
}

struct Centrality;

impl Operator for Centrality {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "hydro.centrality",
            "The place of every link in its river network, as hydrographr's get_centrality on the graph of the \
             links: the links flowing into it, its degree, its Shreve magnitude (the sources above it), the length \
             of river above it, and its betweenness (the pairs of links whose path goes through it) and closeness \
             (one over the sum of the steps to every other link of its network), igraph's on the undirected graph.",
            Access::None,
        )
        .input(PortSpec::new("links", Type::Table, "the links' table of hydro.stream_links (link, downstream, length)"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "link, downstream, upstream, degree, magnitude, upstream_length, betweenness, closeness",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        centrality(args).map_err(fail)
    }
}

fn centrality(args: &Args) -> Result<Outputs> {
    let Some(Value::Table(t)) = args.get("links") else { return Err(Error::msg("links is the links' table")) };
    let col = |name: &str| {
        t.column(name)
            .ok_or_else(|| Error::msg(format!("the links' table has no column {name:?} (hydro.stream_links')")))
    };
    let (ids, below, length) = (col("link")?, col("downstream")?, col("length")?);
    let n = ids.len();
    let at: HashMap<u64, usize> = ids.iter().enumerate().map(|(i, l)| (l.to_bits(), i)).collect();
    let parent: Vec<Option<usize>> = below.iter().map(|b| at.get(&b.to_bits()).copied()).collect();
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); n];
    for (i, p) in parent.iter().enumerate() {
        if let Some(p) = *p {
            children[p].push(i);
        }
    }
    // from the sources down: every link after all those above it
    let mut order: Vec<usize> = (0..n).filter(|&i| children[i].is_empty()).collect();
    let mut left: Vec<usize> = children.iter().map(Vec::len).collect();
    let mut i = 0;
    while i < order.len() {
        if let Some(p) = parent[order[i]] {
            left[p] -= 1;
            if left[p] == 0 {
                order.push(p);
            }
        }
        i += 1;
    }
    if order.len() != n {
        return Err(Error::msg("the links go round in a circle"));
    }
    // sizes, sources, length and the sum of the steps to every link above, from the sources down
    let (mut size, mut magnitude, mut above, mut steps) = (vec![1.0; n], vec![0.0; n], length.clone(), vec![0.0; n]);
    for &v in &order {
        if children[v].is_empty() {
            magnitude[v] = 1.0;
        }
        if let Some(p) = parent[v] {
            size[p] += size[v];
            magnitude[p] += magnitude[v];
            above[p] += above[v];
            steps[p] += steps[v] + size[v];
        }
    }
    // each network's size, from its outlet; the sum of the steps to every link, from the outlets up
    let mut total = vec![0.0; n];
    let mut sum = vec![0.0; n];
    for &v in order.iter().rev() {
        match parent[v] {
            None => {
                total[v] = size[v];
                sum[v] = steps[v];
            }
            Some(p) => {
                total[v] = total[p];
                sum[v] = sum[p] - size[v] + (total[v] - size[v]);
            }
        }
    }
    let mut out = Table::new(
        ["link", "downstream", "upstream", "degree", "magnitude", "upstream_length", "betweenness", "closeness"]
            .map(String::from)
            .to_vec(),
    );
    for v in 0..n {
        // the branches around the link: those above it, and the rest of its network below
        let mut branches: Vec<f64> = children[v].iter().map(|&c| size[c]).collect();
        if parent[v].is_some() {
            branches.push(total[v] - size[v]);
        }
        let s: f64 = branches.iter().sum();
        let betweenness = (s * s - branches.iter().map(|b| b * b).sum::<f64>()) / 2.0;
        let degree = branches.len() as f64;
        out.rows.push(vec![
            ids[v],
            if parent[v].is_some() { below[v] } else { 0.0 },
            children[v].len() as f64,
            degree,
            magnitude[v],
            above[v],
            betweenness,
            if sum[v] > 0.0 { 1.0 / sum[v] } else { f64::NAN },
        ]);
    }
    Ok(outputs([("table", Value::Table(out))]))
}

/// A river network as a forest of segments (the stretches between barriers): each segment's downstream one (none at
/// a mouth), its weight (its length, or its habitat), and the passability of the barrier between it and the one
/// below, for a fish going up through it and going down.
pub struct Segments {
    pub below: Vec<Option<usize>>,
    pub weight: Vec<f64>,
    pub up: Vec<f64>,
    pub down: Vec<f64>,
}

impl Segments {
    /// The segments in an order where each comes before the one below it.
    fn order(&self) -> Vec<usize> {
        let n = self.below.len();
        let mut left = vec![0usize; n];
        for b in self.below.iter().flatten() {
            left[*b] += 1;
        }
        let mut order: Vec<usize> = (0..n).filter(|&i| left[i] == 0).collect();
        let mut i = 0;
        while i < order.len() {
            if let Some(b) = self.below[order[i]] {
                left[b] -= 1;
                if left[b] == 0 {
                    order.push(b);
                }
            }
            i += 1;
        }
        order
    }

    /// The passability of the barrier below each segment going down and going up: both directions' product for each
    /// when symmetric (a fish passes it and comes back, Cote et al. 2009), else its own.
    fn factors(&self, symmetric: bool) -> (Vec<f64>, Vec<f64>) {
        if symmetric {
            let both: Vec<f64> = self.up.iter().zip(&self.down).map(|(u, d)| u * d).collect();
            (both.clone(), both)
        } else {
            (self.down.clone(), self.up.clone())
        }
    }

    /// riverconn's reach index of every segment, unscaled: the weights of all the segments of its network, each
    /// times the passability of the barriers on the way from it to them (`from`: from them to it).
    pub fn reach(&self, symmetric: bool, from: bool) -> Vec<f64> {
        let (mut going_down, mut going_up) = self.factors(symmetric);
        if from {
            std::mem::swap(&mut going_down, &mut going_up);
        }
        let order = self.order();
        // what each reaches upstream in its own subtree, from the sources down
        let mut above = self.weight.clone();
        for &v in &order {
            if let Some(b) = self.below[v] {
                above[b] += going_up[v] * above[v];
            }
        }
        // and the rest of its network, through the one below, from the mouths up
        let mut all = vec![0.0; self.below.len()];
        for &v in order.iter().rev() {
            all[v] = match self.below[v] {
                None => above[v],
                Some(b) => above[v] + going_down[v] * (all[b] - going_up[v] * above[v]),
            };
        }
        all
    }

    /// The network (its mouth segment) of each segment.
    pub fn mouths(&self) -> Vec<usize> {
        let mut mouth: Vec<usize> = (0..self.below.len()).collect();
        for &v in self.order().iter().rev() {
            if let Some(b) = self.below[v] {
                mouth[v] = mouth[b];
            }
        }
        mouth
    }

    /// The dendritic connectivity index of each network (by its mouth segment), 0 to 100: riverconn's full index,
    /// the potamodromous DCI (between every two segments), and its reach index of the mouth, the diadromous DCI
    /// (from the mouth to every segment).
    pub fn dci(&self, symmetric: bool) -> HashMap<usize, (f64, f64)> {
        let reach = self.reach(symmetric, false);
        let mouth = self.mouths();
        let mut sums: HashMap<usize, (f64, f64)> = HashMap::new();
        for (i, &m) in mouth.iter().enumerate() {
            let e = sums.entry(m).or_insert((0.0, 0.0));
            e.0 += self.weight[i] * reach[i];
            e.1 += self.weight[i];
        }
        sums.into_iter()
            .map(|(m, (num, total))| {
                let (p, d) = if total > 0.0 { (num / (total * total), reach[m] / total) } else { (f64::NAN, f64::NAN) };
                (m, (100.0 * p, 100.0 * d))
            })
            .collect()
    }
}

struct Dci;

impl Operator for Dci {
    fn signature(&self) -> OpSignature {
        dem_and_flats(OpSignature::new(
            "hydro.dci",
            "The dendritic connectivity index of a river network cut by barriers (Cote et al. 2009), as R's \
             riverconn: the network split into segments at each dam, weir or culvert, each segment weighted by its \
             length (or its habitat); the potamodromous DCI (the chance that a fish can swim between two points of \
             the river, 0 to 100) and the diadromous one (between the mouth and a point), from the passability of \
             each barrier up and down. For every barrier, the DCI if it were removed and its gain (riverconn's \
             d_index, percent): which dams to remove first.",
            Access::Global,
        ))
        .input(PortSpec::new(
            "streams",
            Type::Map,
            "the river network: non-zero on its cells (hydro.streams, the links)",
        ))
        .input(PortSpec::new(
            "barriers",
            Type::Vector,
            "the barriers as points, moved onto the streams; their passability from the attributes pass_u and pass_d, \
             or passability (0 to 1)",
        ))
        .input(PortSpec::optional(
            "weights",
            Type::Map,
            "the habitat of each cell: a segment weighs the length of its cells times this (default 1)",
        ))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per network with barriers: network (its mouth segment), segments, barriers, weight, dci_p, dci_d",
        ))
        .output(PortSpec::new(
            "barriers",
            Type::Vector,
            "the barriers moved, their attributes and segment, network, pass_u, pass_d, dci_p_removed, d_index (the \
             percent the DCI of its network grows when it is removed), rank (1 the largest)",
        ))
        .output(PortSpec::new("segments", Type::Map, "the segment of each stream cell (1..), 0 elsewhere"))
        .output(PortSpec::new(
            "reach",
            Type::Map,
            "on each stream cell, riverconn's reach index of its segment, 0 to 100: the share of its network a fish \
             there can reach",
        ))
        .param(PortSpec::optional(
            "passability",
            Type::Number,
            "of the barriers without pass_u, pass_d or passability attributes (0: impassable)",
        ))
        .param(
            PortSpec::optional(
                "direction",
                Type::Text,
                "symmetric (default: the product of the passabilities up and down at every barrier, Cote's) or \
                 asymmetric (up through a barrier going upstream, down going downstream)",
            )
            .choices(["symmetric", "asymmetric"]),
        )
        .param(PortSpec::optional(
            "snap",
            Type::Number,
            "the largest move of a barrier onto the streams, map units (300 m; 3 cells without a projection)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let fail = |e: Error| Error::msg(format!("node {:?}: {e}", args.node));
        dci(ctx, args).map_err(fail)
    }
}

/// The passabilities of each barrier, up and down, from its attributes (or the default).
fn passabilities(v: &Vector, default: f64) -> Result<(Vec<f64>, Vec<f64>)> {
    let attr = |name: &str| v.attributes.iter().find(|(n, _)| n == name).map(|(_, c)| c.numbers());
    let both = attr("passability");
    let pick = |own: Option<Vec<f64>>| -> Vec<f64> {
        (0..v.len())
            .map(|i| {
                let x = own.as_ref().map(|o| o[i]).filter(|x| !x.is_nan());
                x.or_else(|| both.as_ref().map(|b| b[i]).filter(|x| !x.is_nan())).unwrap_or(default)
            })
            .collect()
    };
    let (up, down) = (pick(attr("pass_u")), pick(attr("pass_d")));
    if let Some(i) = up.iter().chain(&down).position(|p| !(0.0..=1.0).contains(p)) {
        return Err(Error::msg(format!("barrier {}: a passability is between 0 and 1", i % v.len() + 1)));
    }
    Ok((up, down))
}

fn dci(ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
    let dem = args.map("dem")?;
    let streams = args.map("streams")?;
    on_dem(dem, streams, "streams")?;
    let symmetric = match args.param("direction", "symmetric".to_string())?.as_str() {
        "symmetric" => true,
        "asymmetric" => false,
        d => return Err(Error::msg(format!("direction must be symmetric or asymmetric, got {d:?}"))),
    };
    let weights = args.opt_map("weights")?.map(|w| on_dem(dem, w, "weights")).transpose()?;
    let n = dem.data.len();
    let nx = dem.grid.width;
    let on: Vec<bool> =
        crate::rivers::stream_cells(streams).iter().zip(dem.data.iter()).map(|(s, z)| *s && !z.is_nan()).collect();
    let down = drainage(args, dem)?;
    let (v, pts) = points_of(args, "barriers", &dem.grid)?;
    let (pass_u, pass_d) = passabilities(&v, args.param("passability", 0.0)?)?;
    let distance = snap_distance(args, "snap", &dem.grid)?;
    let at = cells_of_points(ctx, &pts, &dem.grid, Some(&on), distance);
    // the segments: one above each barrier's cell (barriers on one cell are one), one at each mouth of the network
    let mut seed = vec![0u32; n];
    let mut barrier_cell: Vec<Option<usize>> = Vec::new();
    for k in at.iter().flatten() {
        if seed[*k] == 0 {
            barrier_cell.push(Some(*k));
            seed[*k] = barrier_cell.len() as u32;
        }
    }
    for k in 0..n {
        if on[k] && seed[k] == 0 && (down[k] < 0 || !on[down[k] as usize]) {
            barrier_cell.push(None);
            seed[k] = barrier_cell.len() as u32;
        }
    }
    let label = crate::rivers::first_below(&down, &seed)?;
    let m = barrier_cell.len();
    let (dx, dy) = cells(&dem.grid);
    let mut net = Segments { below: vec![None; m], weight: vec![0.0; m], up: vec![1.0; m], down: vec![1.0; m] };
    for k in 0..n {
        if on[k] && label[k] > 0 {
            let w = weights.map_or(1.0, |w| if w[k].is_nan() { 0.0 } else { w[k] });
            net.weight[label[k] as usize - 1] += step(k, down[k], nx, dx, dy) * w;
        }
    }
    let mut lost = Vec::new();
    for (s, cell) in barrier_cell.iter().enumerate() {
        let Some(k) = *cell else { continue };
        let below = down[k];
        if below >= 0 && on[below as usize] && label[below as usize] > 0 {
            net.below[s] = Some(label[below as usize] as usize - 1);
            // (barriers on one cell: their passabilities multiplied)
            for (j, kj) in at.iter().enumerate() {
                if *kj == Some(k) {
                    net.up[s] *= pass_u[j];
                    net.down[s] *= pass_d[j];
                }
            }
        } else {
            lost.extend(at.iter().enumerate().filter(|(_, kj)| **kj == Some(k)).map(|(j, _)| (j + 1).to_string()));
        }
    }
    if !lost.is_empty() {
        ctx.warn(format!("barrier(s) {} at the mouth of the network: nothing below them", lost.join(", ")));
    }
    let mouth = net.mouths();
    let base = net.dci(symmetric);
    let reach = net.reach(symmetric, false);
    let mut total: HashMap<usize, f64> = HashMap::new();
    for (s, &mo) in mouth.iter().enumerate() {
        *total.entry(mo).or_insert(0.0) += net.weight[s];
    }
    // each barrier removed in turn: the DCI of its network then
    let mut removed = vec![f64::NAN; m];
    for s in 0..m {
        if net.below[s].is_none() {
            continue;
        }
        let kept = (net.up[s], net.down[s]);
        (net.up[s], net.down[s]) = (1.0, 1.0);
        removed[s] = net.dci(symmetric)[&mouth[s]].0;
        (net.up[s], net.down[s]) = kept;
    }
    let mut table =
        Table::new(["network", "segments", "barriers", "weight", "dci_p", "dci_d"].map(String::from).to_vec());
    let mut networks: Vec<usize> = (0..m).filter(|&s| net.below[s].is_some()).map(|s| mouth[s]).collect();
    networks.sort_unstable();
    networks.dedup();
    for &mo in &networks {
        let members = mouth.iter().filter(|&&x| x == mo).count();
        let barriers = (0..m).filter(|&s| mouth[s] == mo && net.below[s].is_some()).count();
        let (p, d) = base[&mo];
        table.rows.push(vec![(mo + 1) as f64, members as f64, barriers as f64, total[&mo], p, d]);
    }
    // the barriers, ranked by what removing each gives
    let names = ["segment", "network", "pass_u", "pass_d", "dci_p_removed", "d_index", "rank"];
    let mut cols: Vec<Vec<f64>> = vec![vec![f64::NAN; v.len()]; names.len()];
    let mut gains = Vec::new();
    for (i, k) in at.iter().enumerate() {
        let Some(k) = *k else { continue };
        let s = label[k] as usize - 1;
        let mo = mouth[s];
        let row = [(s + 1) as f64, (mo + 1) as f64, pass_u[i], pass_d[i], removed[s]];
        for (c, x) in cols.iter_mut().zip(row) {
            c[i] = x;
        }
        if !removed[s].is_nan() {
            let gain = (removed[s] - base[&mo].0) / base[&mo].0 * 100.0;
            cols[5][i] = gain;
            gains.push((gain, i));
        }
    }
    gains.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
    for (r, (_, i)) in gains.iter().enumerate() {
        cols[6][*i] = (r + 1) as f64;
    }
    let mut moved = (*v).clone();
    let c = crate::rivers::Cells::of(&dem.grid);
    for (i, k) in at.iter().enumerate() {
        if let Some(k) = k {
            moved.geometries[i] = Geometry::Point(c.centre(*k));
        }
    }
    let (moved, _) = crate::vector::with_columns(&moved, names.iter().map(|n| n.to_string()).zip(cols).collect());
    let segments: Vec<f64> = (0..n)
        .map(|k| {
            if dem.data[k].is_nan() {
                f64::NAN
            } else if on[k] {
                f64::from(label[k])
            } else {
                0.0
            }
        })
        .collect();
    let reached: Vec<f64> = (0..n)
        .map(|k| {
            if !on[k] || label[k] == 0 {
                return f64::NAN;
            }
            let s = label[k] as usize - 1;
            100.0 * reach[s] / total[&mouth[s]]
        })
        .collect();
    let g = &dem.grid;
    Ok(outputs([
        ("table", Value::Table(table)),
        ("barriers", Value::Vector(Arc::new(moved))),
        ("segments", Value::Map(Map::new(g.clone(), segments))),
        ("reach", Value::Map(Map::new(g.clone(), reached))),
    ]))
}
