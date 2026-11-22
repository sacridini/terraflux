//! The tidy layout of the node editor (*Arrange*): a layered drawing of the graph. The columns are the nodes'
//! depths (the longest chain of nodes each reads). A connection that spans several columns goes through a
//! virtual node in each column it crosses, so that it has a slot of its own there and is not drawn over a box.
//! The order in each column comes from sweeps of barycentres and medians down and up the columns (the best
//! order, by the crossings it leaves, is kept) with a swap of neighbours at the end of each; then the boxes are
//! placed, with their different heights, as near the ports they are connected to as the order and a gap allow,
//! so that connections run as straight as they can. Everything is deterministic (ties go by the place the
//! nodes were at, then by their order), and takes time proportional to the connections' count (times its
//! logarithm) for each sweep.

use std::collections::{HashMap, HashSet};

use terraflux_graph::{Node, Ref, Source};
use terraflux_ops::Registry;

use crate::editor;
use crate::editor_view::{HEAD, ROW, columns_x, depths, input_rows, node_height, node_width, reads};

/// The space between two boxes of a column.
const GAP: f32 = 30.0;
/// The space between a connection passing through a column and the boxes or connections next to it.
const LANE: f32 = 14.0;
/// The most sweeps of the order, and how many in a row may leave it no better before it stops.
const PASSES: usize = 24;
const STALL: usize = 6;
/// The rounds of alignment of the boxes (each goes down the columns and up again).
const ROUNDS: usize = 12;
/// How much more a virtual node weighs than a box in the alignment (a long connection is kept straight).
const LONG: f32 = 8.0;
/// The most pairs of neighbours looked at to decide a swap of two nodes (very busy nodes are left where they are).
const SWAP_MAX: usize = 40_000;

/// A connection: `from` is the node read, `to` the one that reads, and the rows (graph units under the top of
/// each box) the connection leaves and enters by.
#[derive(Clone, Copy, Debug)]
pub struct Edge {
    pub from: usize,
    pub to: usize,
    pub from_dy: f32,
    pub to_dy: f32,
}

/// What is to be laid out: the column of each node, its height, where it was (its y, to keep an order that is
/// already good), and the connections.
pub struct Graph {
    pub layer: Vec<usize>,
    pub height: Vec<f32>,
    pub start: Vec<f32>,
    pub edges: Vec<Edge>,
}

/// The tops of the boxes, and the crossings the order left between columns (those of the virtual nodes counted).
pub struct Placed {
    pub y: Vec<f32>,
    pub crossings: usize,
}

/// A link between two adjacent columns: the node at the other end, this end's row and the other end's.
type Link = (usize, f32, f32);

/// The layered graph: the nodes (the virtual ones after them), each column's order, and the links between
/// adjacent columns.
struct Lay {
    n: usize,
    layers: Vec<Vec<usize>>,
    layer: Vec<usize>,
    height: Vec<f32>,
    up: Vec<Vec<Link>>,
    down: Vec<Vec<Link>>,
    pos: Vec<usize>,
}

/// Where a link ends, to be compared: the node's place in its column, then the row of the port.
fn spot(pos: usize, row: f32) -> u64 {
    ((pos as u64) << 32) | u64::from(row.max(0.0).to_bits())
}

impl Lay {
    fn new(g: &Graph) -> Lay {
        let n = g.layer.len();
        let columns = g.layer.iter().max().map_or(0, |m| m + 1);
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| g.start[a].total_cmp(&g.start[b]).then(a.cmp(&b)));
        let mut lay = Lay {
            n,
            layers: vec![Vec::new(); columns],
            layer: g.layer.clone(),
            height: g.height.clone(),
            up: vec![Vec::new(); n],
            down: vec![Vec::new(); n],
            pos: vec![0; n],
        };
        for i in order {
            lay.layers[g.layer[i]].push(i);
        }
        for e in &g.edges {
            let (a, b) = (g.layer[e.from], g.layer[e.to]);
            if b <= a {
                continue;
            }
            let (mut prev, mut dy) = (e.from, e.from_dy);
            for l in a + 1..b {
                let d = lay.layer.len();
                lay.layer.push(l);
                lay.height.push(0.0);
                lay.up.push(Vec::new());
                lay.down.push(Vec::new());
                lay.pos.push(0);
                lay.layers[l].push(d);
                lay.link(prev, d, dy, 0.0);
                prev = d;
                dy = 0.0;
            }
            lay.link(prev, e.to, dy, e.to_dy);
        }
        lay.refresh();
        lay
    }

    fn link(&mut self, u: usize, v: usize, du: f32, dv: f32) {
        self.down[u].push((v, du, dv));
        self.up[v].push((u, dv, du));
    }

    fn refresh(&mut self) {
        for c in &self.layers {
            for (k, &s) in c.iter().enumerate() {
                self.pos[s] = k;
            }
        }
    }

    /// The crossings between adjacent columns: the pairs of links whose ends are in opposite orders, counted by
    /// the inversions of one end when the links are sorted by the other (a tree of counts: n log n).
    fn count(&self) -> usize {
        let mut total = 0;
        let mut pairs: Vec<(u64, u64)> = Vec::new();
        let mut lows: Vec<u64> = Vec::new();
        let mut tree: Vec<usize> = Vec::new();
        for l in 0..self.layers.len().saturating_sub(1) {
            pairs.clear();
            for &u in &self.layers[l] {
                pairs.extend(self.down[u].iter().map(|&(v, du, dv)| (spot(self.pos[u], du), spot(self.pos[v], dv))));
            }
            pairs.sort_unstable();
            lows.clear();
            lows.extend(pairs.iter().map(|p| p.1));
            lows.sort_unstable();
            lows.dedup();
            let m = lows.len();
            tree.clear();
            tree.resize(m + 1, 0);
            for (seen, &(_, low)) in pairs.iter().enumerate() {
                let p = lows.binary_search(&low).unwrap_or(0);
                let mut at = p + 1;
                let mut upto = 0;
                while at > 0 {
                    upto += tree[at];
                    at &= at - 1;
                }
                total += seen - upto;
                let mut at = p + 1;
                while at <= m {
                    tree[at] += 1;
                    at += at.isolate_lowest_one();
                }
            }
        }
        total
    }

    /// A sweep: each column, one after the other, ordered by the mean (or the median) of the places of the
    /// nodes it is linked to in the column before it (going down) or after it (going up); a node with none stays.
    fn sweep(&mut self, down: bool, median: bool) {
        let columns = self.layers.len();
        let order: Vec<usize> =
            if down { (1..columns).collect() } else { (0..columns.saturating_sub(1)).rev().collect() };
        let mut places: Vec<f32> = Vec::new();
        for l in order {
            let mut keyed: Vec<(f32, usize, usize)> = Vec::with_capacity(self.layers[l].len());
            for &s in &self.layers[l] {
                let links = if down { &self.up[s] } else { &self.down[s] };
                let key = if links.is_empty() {
                    self.pos[s] as f32
                } else {
                    places.clear();
                    places.extend(links.iter().map(|&(t, ..)| self.pos[t] as f32));
                    if median {
                        places.sort_by(f32::total_cmp);
                        let k = places.len();
                        if k % 2 == 1 { places[k / 2] } else { (places[k / 2 - 1] + places[k / 2]) / 2.0 }
                    } else {
                        places.iter().sum::<f32>() / places.len() as f32
                    }
                };
                keyed.push((key, self.pos[s], s));
            }
            keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
            for (k, &(.., s)) in keyed.iter().enumerate() {
                self.layers[l][k] = s;
                self.pos[s] = k;
            }
        }
    }

    /// The crossings the links of `a` and `b` make with each other's when `a` is left of `b` in their column.
    fn cross(&self, a: usize, b: usize) -> usize {
        let mut c = 0;
        for (sa, sb) in [(&self.up[a], &self.up[b]), (&self.down[a], &self.down[b])] {
            for &(x, _, tx) in sa {
                for &(y, _, ty) in sb {
                    if spot(self.pos[x], tx) > spot(self.pos[y], ty) {
                        c += 1;
                    }
                }
            }
        }
        c
    }

    /// Swaps of two neighbours in a column that leave fewer crossings, until there are none.
    fn transpose(&mut self) {
        for l in 0..self.layers.len() {
            for _ in 0..PASSES {
                let mut changed = false;
                for k in 0..self.layers[l].len().saturating_sub(1) {
                    let (a, b) = (self.layers[l][k], self.layers[l][k + 1]);
                    let work = (self.up[a].len() + self.down[a].len()) * (self.up[b].len() + self.down[b].len());
                    if work == 0 || work > SWAP_MAX {
                        continue;
                    }
                    if self.cross(b, a) < self.cross(a, b) {
                        self.layers[l].swap(k, k + 1);
                        self.pos[a] = k + 1;
                        self.pos[b] = k;
                        changed = true;
                    }
                }
                if !changed {
                    break;
                }
            }
        }
    }

    /// The order of every column: sweeps of means and medians, down and up, each followed by the swaps; the best
    /// order met (by crossings) is kept.
    fn order(&mut self) {
        let mut best = self.count();
        let mut kept = self.layers.clone();
        let mut stall = 0;
        for pass in 0..PASSES {
            if best == 0 {
                break;
            }
            self.sweep(pass % 2 == 0, (pass / 2) % 2 == 1);
            self.transpose();
            let c = self.count();
            if c < best {
                best = c;
                kept.clone_from(&self.layers);
                stall = 0;
            } else {
                stall += 1;
                if stall >= STALL {
                    break;
                }
            }
        }
        self.layers = kept;
        self.refresh();
    }

    /// The least space between the tops of the neighbours of a column, from the first's.
    fn offsets(&self, l: usize) -> Vec<f32> {
        let mut off = Vec::with_capacity(self.layers[l].len());
        let mut at = 0.0;
        let mut last: Option<usize> = None;
        for &s in &self.layers[l] {
            if let Some(p) = last {
                at += self.height[p] + if p >= self.n || s >= self.n { LANE } else { GAP };
            }
            off.push(at);
            last = Some(s);
        }
        off
    }

    /// One column moved toward the rows its neighbours' links lead from: each node to the mean of where they
    /// put it, the order and the spaces kept, with the least total squared move (pooled adjacent violators).
    fn align(&self, l: usize, off: &[f32], y: &mut [f32]) {
        let col = &self.layers[l];
        let mut z = Vec::with_capacity(col.len());
        let mut w = Vec::with_capacity(col.len());
        for (k, &s) in col.iter().enumerate() {
            let (mut sum, mut cnt) = (0.0, 0.0);
            for &(t, mine, theirs) in self.up[s].iter().chain(&self.down[s]) {
                // (a link of a long connection pulls harder: it is kept straight)
                let pull = if s >= self.n || t >= self.n { LONG } else { 1.0 };
                sum += pull * (y[t] + theirs - mine);
                cnt += pull;
            }
            let want = if cnt > 0.0 { sum / cnt } else { y[s] };
            z.push(want - off[k]);
            w.push(if s >= self.n { LONG } else { 1.0 });
        }
        for (k, v) in isotonic(&z, &w).into_iter().enumerate() {
            y[col[k]] = v + off[k];
        }
    }

    /// The tops of all the nodes (the virtual ones last): each column stacked and centred, then aligned.
    fn coordinates(&self) -> Vec<f32> {
        let mut y = vec![0.0f32; self.layer.len()];
        let offs: Vec<Vec<f32>> = (0..self.layers.len()).map(|l| self.offsets(l)).collect();
        for (l, col) in self.layers.iter().enumerate() {
            if let (Some(&last), Some(&at)) = (col.last(), offs[l].last()) {
                let half = (at + self.height[last]) / 2.0;
                for (k, &s) in col.iter().enumerate() {
                    y[s] = offs[l][k] - half;
                }
            }
        }
        for _ in 0..ROUNDS {
            for (l, off) in offs.iter().enumerate().skip(1) {
                self.align(l, off, &mut y);
            }
            for (l, off) in offs.iter().enumerate().rev().skip(1) {
                self.align(l, off, &mut y);
            }
        }
        y
    }
}

/// The non-decreasing sequence nearest to `z` by the weighted sum of squares (pool adjacent violators).
fn isotonic(z: &[f32], w: &[f32]) -> Vec<f32> {
    // (sum of weight times value, sum of weight, how many), in double precision: the sums run long
    let mut blocks: Vec<(f64, f64, usize)> = Vec::with_capacity(z.len());
    for (&v, &wt) in z.iter().zip(w) {
        blocks.push((f64::from(v * wt), f64::from(wt), 1));
        while blocks.len() >= 2 {
            let (b, a) = (blocks[blocks.len() - 1], blocks[blocks.len() - 2]);
            if a.0 / a.1 <= b.0 / b.1 {
                break;
            }
            blocks.truncate(blocks.len() - 2);
            blocks.push((a.0 + b.0, a.1 + b.1, a.2 + b.2));
        }
    }
    let mut out = Vec::with_capacity(z.len());
    for (s, wt, c) in blocks {
        out.extend(std::iter::repeat_n((s / wt) as f32, c));
    }
    out
}

/// The order and the places of a layered graph: the tops of the boxes (the least at 0), and the crossings left.
pub fn place(g: &Graph) -> Placed {
    let mut lay = Lay::new(g);
    lay.order();
    let crossings = lay.count();
    let mut y = lay.coordinates();
    y.truncate(lay.n);
    let top = y.iter().copied().fold(f32::INFINITY, f32::min);
    if top.is_finite() {
        for v in &mut y {
            *v -= top;
        }
    }
    Placed { y, crossings }
}

/// A scope's nodes as a layered graph: the depths, heights, places and connections (each leaving the row of
/// the output it reads and entering the row of the input it is given to; those of a setting or of a body into
/// the middle of the box), with the x of each column and the widths.
pub(crate) struct Scope {
    pub graph: Graph,
    pub xs: Vec<f32>,
    pub depth: Vec<usize>,
    pub width: Vec<f32>,
}

pub(crate) fn scope(nodes: &[Node], r: &Registry) -> Scope {
    let ids: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let deps: Vec<Vec<usize>> = nodes.iter().map(|n| reads(n, &ids)).collect();
    let depth = depths(nodes, &deps);
    let xs = columns_x(nodes, &depth, r, 90.0);
    let height: Vec<f32> = nodes.iter().map(|n| node_height(n, r)).collect();
    let width: Vec<f32> = nodes.iter().map(|n| node_width(n, r)).collect();
    let start: Vec<f32> = nodes.iter().map(|n| n.pos.map_or(0.0, |p| p[1])).collect();
    let rows: Vec<Vec<String>> = nodes.iter().map(|n| input_rows(n, r)).collect();
    let mut outs: Vec<Option<Vec<String>>> = vec![None; nodes.len()];
    let mut edges = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        let own: HashSet<&str> =
            n.body.as_ref().map(|b| b.nodes.iter().map(|m| m.id.as_str()).collect()).unwrap_or_default();
        let mut rest = terraflux_graph::references(n);
        let mut find = |text: &str| -> Option<(usize, f32)> {
            let Ok(Ref::Port { node, port }) = Ref::parse(text) else { return None };
            if node == n.id || own.contains(node.as_str()) {
                return None;
            }
            let &j = ids.get(node.as_str())?;
            let names =
                outs[j].get_or_insert_with(|| editor::output_ports(&nodes[j], r).into_iter().map(|(p, _)| p).collect());
            let k = names.iter().position(|p| *p == port).unwrap_or(0);
            Some((j, HEAD + (k as f32 + 0.5) * ROW))
        };
        for (name, s) in &n.inputs {
            let Source::Ref(text) = s else { continue };
            if let Some(k) = rest.iter().position(|t| t == text) {
                rest.swap_remove(k);
            }
            if let Some((j, from_dy)) = find(text) {
                let to_dy =
                    rows[i].iter().position(|p| p == name).map_or(height[i] / 2.0, |k| HEAD + (k as f32 + 0.5) * ROW);
                edges.push(Edge { from: j, to: i, from_dy, to_dy });
            }
        }
        for text in rest {
            if let Some((j, from_dy)) = find(&text) {
                edges.push(Edge { from: j, to: i, from_dy, to_dy: height[i] / 2.0 });
            }
        }
    }
    Scope { graph: Graph { layer: depth.clone(), height, start, edges }, xs, depth, width }
}

/// The most nodes of a scope whose layout is checked against the stacked one (the check takes time in the
/// square of the connections).
const CHECKED: usize = 400;

/// A tidy place for every node of a scope (see the module): the columns of the depths, the order of each and
/// the boxes' tops from [`place`]. In a scope of up to [`CHECKED`] nodes the wires are drawn straight between
/// their ports to count the crossings and the boxes they go through, and the layout this one replaced (the
/// columns ordered by one mean of rows, the boxes stacked) is kept when this one would be worse in either.
pub fn arrange(nodes: &[Node], r: &Registry) -> HashMap<String, [f32; 2]> {
    let s = scope(nodes, r);
    let new = tidy(nodes, &s);
    if nodes.len() > CHECKED {
        return new;
    }
    let old = stacked(nodes, r);
    let (n, o) = (cost(nodes, &s, &new), cost(nodes, &s, &old));
    if n.0 <= o.0 && n.1 <= o.1 { new } else { old }
}

/// The layout of [`place`] for a scope.
pub(crate) fn tidy(nodes: &[Node], s: &Scope) -> HashMap<String, [f32; 2]> {
    let placed = place(&s.graph);
    nodes.iter().enumerate().map(|(i, n)| (n.id.clone(), [s.xs[s.depth[i]], placed.y[i]])).collect()
}

/// The layout the application had: a column per depth ordered by two passes of the mean row of what each reads,
/// the boxes stacked by their heights.
pub(crate) fn stacked(nodes: &[Node], r: &Registry) -> HashMap<String, [f32; 2]> {
    let ids: HashMap<&str, usize> = nodes.iter().enumerate().map(|(i, n)| (n.id.as_str(), i)).collect();
    let deps: Vec<Vec<usize>> = nodes.iter().map(|n| reads(n, &ids)).collect();
    let depth = depths(nodes, &deps);
    let xs = columns_x(nodes, &depth, r, 90.0);
    let mut column: Vec<Vec<usize>> = vec![Vec::new(); xs.len()];
    // a first order: as they are on the canvas
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by(|&a, &b| {
        let (pa, pb) = (nodes[a].pos.unwrap_or([0.0, 0.0]), nodes[b].pos.unwrap_or([0.0, 0.0]));
        pa[1].total_cmp(&pb[1]).then(a.cmp(&b))
    });
    for i in order {
        column[depth[i]].push(i);
    }
    let mut rank = vec![0.0f32; nodes.len()];
    for c in &column {
        for (k, &i) in c.iter().enumerate() {
            rank[i] = k as f32;
        }
    }
    for _ in 0..2 {
        for c in column.iter_mut().skip(1) {
            let key = |i: usize| {
                let up: Vec<f32> = deps[i].iter().map(|&d| rank[d]).collect();
                if up.is_empty() { rank[i] } else { up.iter().sum::<f32>() / up.len() as f32 }
            };
            c.sort_by(|&a, &b| key(a).total_cmp(&key(b)));
            for (k, &i) in c.iter().enumerate() {
                rank[i] = k as f32;
            }
        }
    }
    let mut out = HashMap::new();
    for (d, c) in column.iter().enumerate() {
        let mut y = 0.0;
        for &i in c {
            out.insert(nodes[i].id.clone(), [xs[d], y]);
            y += node_height(&nodes[i], r) + 30.0;
        }
    }
    out
}

/// Whether the segment (a, b) properly crosses the segment (c, d).
fn crosses(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    let o = |p: [f32; 2], q: [f32; 2], s: [f32; 2]| (q[0] - p[0]) * (s[1] - p[1]) - (q[1] - p[1]) * (s[0] - p[0]);
    let (d1, d2, d3, d4) = (o(a, b, c), o(a, b, d), o(c, d, a), o(c, d, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

/// Whether the segment (a, b) goes through the rectangle (left, top, right, bottom).
fn hits(a: [f32; 2], b: [f32; 2], r: [f32; 4]) -> bool {
    let (mut t0, mut t1) = (0.0f32, 1.0f32);
    for (p, q, lo, hi) in [(a[0], b[0], r[0], r[2]), (a[1], b[1], r[1], r[3])] {
        let dp = q - p;
        if dp.abs() < 1e-6 {
            if p <= lo || p >= hi {
                return false;
            }
        } else {
            let (u, v) = ((lo - p) / dp, (hi - p) / dp);
            t0 = t0.max(u.min(v));
            t1 = t1.min(u.max(v));
        }
    }
    t0 < t1
}

/// The crossings of wires and the wires through the boxes of other nodes, each wire straight from the output
/// to the input it joins.
pub(crate) fn cost(nodes: &[Node], s: &Scope, at: &HashMap<String, [f32; 2]>) -> (usize, usize) {
    let pos = |i: usize| at[&nodes[i].id];
    let wires: Vec<([f32; 2], [f32; 2], usize, usize)> = s
        .graph
        .edges
        .iter()
        .map(|e| {
            let (p, q) = (pos(e.from), pos(e.to));
            ([p[0] + s.width[e.from], p[1] + e.from_dy], [q[0], q[1] + e.to_dy], e.from, e.to)
        })
        .collect();
    let mut crossings = 0;
    for (k, a) in wires.iter().enumerate() {
        for b in &wires[k + 1..] {
            if crosses(a.0, a.1, b.0, b.1) {
                crossings += 1;
            }
        }
    }
    let mut through = 0;
    for w in &wires {
        for i in (0..nodes.len()).filter(|&i| i != w.2 && i != w.3) {
            let p = pos(i);
            if hits(w.0, w.1, [p[0], p[1], p[0] + s.width[i], p[1] + s.graph.height[i]]) {
                through += 1;
            }
        }
    }
    (crossings, through)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(from: usize, to: usize) -> Edge {
        Edge { from, to, from_dy: 10.0, to_dy: 10.0 }
    }

    fn graph(layer: &[usize], start: &[f32], edges: &[(usize, usize)]) -> Graph {
        Graph {
            layer: layer.to_vec(),
            height: vec![60.0; layer.len()],
            start: start.to_vec(),
            edges: edges.iter().map(|&(a, b)| edge(a, b)).collect(),
        }
    }

    /// No two boxes of a column overlap.
    fn no_overlap(g: &Graph, y: &[f32]) -> bool {
        let mut by: Vec<usize> = (0..y.len()).collect();
        by.sort_by(|&a, &b| (g.layer[a], y[a]).partial_cmp(&(g.layer[b], y[b])).unwrap());
        by.windows(2).all(|w| g.layer[w[0]] != g.layer[w[1]] || y[w[1]] >= y[w[0]] + g.height[w[0]] + 2.0 * LANE - 0.1)
    }

    #[test]
    fn crossings_are_counted() {
        // two links crossing; then a complete 2x2 (one crossing)
        let g = graph(&[0, 0, 1, 1], &[0.0, 1.0, 0.0, 1.0], &[(0, 3), (1, 2)]);
        assert_eq!(Lay::new(&g).count(), 1);
        let g = graph(&[0, 0, 1, 1], &[0.0, 1.0, 0.0, 1.0], &[(0, 2), (1, 3)]);
        assert_eq!(Lay::new(&g).count(), 0);
        let g = graph(&[0, 0, 1, 1], &[0.0, 1.0, 0.0, 1.0], &[(0, 2), (0, 3), (1, 2), (1, 3)]);
        assert_eq!(Lay::new(&g).count(), 1);
        // three links all crossing one another
        let g = graph(&[0, 0, 0, 1, 1, 1], &[0.0, 1.0, 2.0, 0.0, 1.0, 2.0], &[(0, 5), (1, 4), (2, 3)]);
        assert_eq!(Lay::new(&g).count(), 3);
    }

    #[test]
    fn a_crossing_is_undone() {
        let g = graph(&[0, 0, 1, 1], &[0.0, 1.0, 0.0, 1.0], &[(0, 3), (1, 2)]);
        let p = place(&g);
        assert_eq!(p.crossings, 0);
        assert!(no_overlap(&g, &p.y));
        // the complete 2x2 cannot go under one
        let g = graph(&[0, 0, 1, 1], &[0.0, 1.0, 0.0, 1.0], &[(0, 2), (0, 3), (1, 2), (1, 3)]);
        assert_eq!(place(&g).crossings, 1);
    }

    /// Whether the straight wire of an edge goes through a box of another node (columns 300 apart, 100 wide).
    fn through(g: &Graph, y: &[f32], e: &Edge) -> bool {
        let (x0, y0) = (g.layer[e.from] as f32 * 300.0 + 100.0, y[e.from] + e.from_dy);
        let (x1, y1) = (g.layer[e.to] as f32 * 300.0, y[e.to] + e.to_dy);
        (0..y.len()).filter(|&k| k != e.from && k != e.to).any(|k| {
            let bx = g.layer[k] as f32 * 300.0;
            if bx < x0 || bx + 100.0 > x1 {
                return false;
            }
            let at = |x: f32| y0 + (y1 - y0) * (x - x0) / (x1 - x0);
            let (a, b) = (at(bx), at(bx + 100.0));
            a.max(b) > y[k] && a.min(b) < y[k] + g.height[k]
        })
    }

    #[test]
    fn a_long_edge_keeps_clear_of_the_boxes_between() {
        // a chain a-b-c-d with a wire from a to d over three columns, a box in each column between
        let g = graph(&[0, 1, 2, 3], &[0.0; 4], &[(0, 1), (1, 2), (2, 3), (0, 3)]);
        let p = place(&g);
        assert_eq!(p.crossings, 0);
        assert!(no_overlap(&g, &p.y));
        for e in &g.edges {
            assert!(!through(&g, &p.y, e), "{e:?} goes through a box: {:?}", p.y);
        }
    }

    #[test]
    fn a_fan_in_of_five() {
        // five maps read by one node, and the model reads a map and the node: the maps start shuffled
        let layer = [0, 0, 0, 0, 0, 1, 2];
        let start = [3.0, 0.0, 4.0, 1.0, 2.0, 0.0, 0.0];
        let g = graph(&layer, &start, &[(0, 5), (1, 5), (2, 5), (3, 5), (4, 5), (5, 6), (2, 6)]);
        let p = place(&g);
        assert_eq!(p.crossings, 0);
        assert!(no_overlap(&g, &p.y));
        // the reader is among the maps it reads
        let (lo, hi) = p.y[..5].iter().fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        assert!(p.y[5] >= lo && p.y[5] <= hi);
        for e in &g.edges {
            assert!(!through(&g, &p.y, e));
        }
    }

    #[test]
    fn the_same_input_gives_the_same_places() {
        let layer: Vec<usize> = (0..60).map(|i| i % 6).collect();
        let start: Vec<f32> = (0..60).map(|i| ((i * 7) % 11) as f32).collect();
        let edges: Vec<(usize, usize)> =
            (0..60).filter(|i| i % 6 > 0).flat_map(|i| [(i - 1, i), (((i * 7) % 10) * 6 + i % 6 - 1, i)]).collect();
        let g = graph(&layer, &start, &edges);
        let (a, b) = (place(&g), place(&g));
        assert_eq!(a.y, b.y);
        assert_eq!(a.crossings, b.crossings);
    }

    /// A layered graph of `columns` x `rows` nodes, each reading two of the column before and now and then one
    /// two columns back.
    fn big(columns: usize, rows: usize) -> Graph {
        let n = columns * rows;
        let mut edges = Vec::new();
        for c in 1..columns {
            for k in 0..rows {
                let i = c * rows + k;
                edges.push((((c - 1) * rows) + (k * 7 + c) % rows, i));
                edges.push((((c - 1) * rows) + (k * 3 + 1) % rows, i));
                if c > 1 && k % 5 == 0 {
                    edges.push((((c - 2) * rows) + (k * 11) % rows, i));
                }
            }
        }
        graph(
            &(0..n).map(|i| i / rows).collect::<Vec<_>>(),
            &(0..n).map(|i| ((i * 31) % rows) as f32).collect::<Vec<_>>(),
            &edges,
        )
    }

    #[test]
    fn three_thousand_nodes_arrange_fast() {
        let g = big(30, 100);
        let t = std::time::Instant::now();
        let p = place(&g);
        let took = t.elapsed();
        eprintln!("3000 nodes, {} links: {took:?}, {} crossings", g.edges.len(), p.crossings);
        assert!(no_overlap(&g, &p.y));
        // (a debug build is some tens of times slower than a release one)
        let limit = if cfg!(debug_assertions) { 8.0 } else { 1.0 };
        assert!(took.as_secs_f32() < limit, "{took:?}");
    }

    /// A scope and the scopes in the bodies of its nodes.
    fn scopes(nodes: &[Node], name: &str, out: &mut Vec<(String, Vec<Node>)>) {
        out.push((name.to_string(), nodes.to_vec()));
        for n in nodes {
            if let Some(b) = &n.body {
                scopes(&b.nodes, &format!("{name}/{}", n.id), out);
            }
        }
    }

    /// The new layout is never worse than the one it replaces, in crossings or in wires through boxes, on every
    /// scope of every template; the table is printed (`--nocapture`).
    #[test]
    fn the_templates_never_arrange_worse() {
        let r = Registry::standard();
        let (mut old_total, mut pure_total, mut new_total, mut kept) = ((0, 0), (0, 0), (0, 0), 0);
        println!(
            "{:<52} {:>5} {:>13} {:>13} {:>13}",
            "template (scope)", "nodes", "old c/t", "layered c/t", "arrange c/t"
        );
        for t in crate::templates::TEMPLATES.iter() {
            let mut m = t.model();
            m.upgrade();
            let mut all = Vec::new();
            scopes(&m.nodes, t.name, &mut all);
            for (name, nodes) in all {
                if nodes.len() < 2 {
                    continue;
                }
                let s = scope(&nodes, &r);
                let (old, pure, new) = (stacked(&nodes, &r), tidy(&nodes, &s), arrange(&nodes, &r));
                let (o, p, n) = (cost(&nodes, &s, &old), cost(&nodes, &s, &pure), cost(&nodes, &s, &new));
                println!(
                    "{:<52} {:>5} {:>6}/{:<6} {:>6}/{:<6} {:>6}/{:<6}",
                    name,
                    nodes.len(),
                    o.0,
                    o.1,
                    p.0,
                    p.1,
                    n.0,
                    n.1
                );
                for (total, c) in [(&mut old_total, o), (&mut pure_total, p), (&mut new_total, n)] {
                    *total = (total.0 + c.0, total.1 + c.1);
                }
                kept += usize::from(new == old && pure != old);
                assert!(n.0 <= o.0, "{name}: crossings {} -> {}", o.0, n.0);
                assert!(n.1 <= o.1, "{name}: through boxes {} -> {}", o.1, n.1);
            }
        }
        println!("total (crossings, through): old {old_total:?}, layered {pure_total:?}, arrange {new_total:?}");
        println!("scopes where the layered one was worse and the old one was kept: {kept}");
    }
}
