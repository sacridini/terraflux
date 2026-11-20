//! The `skeleton` group of `objects.features`: the shape of each object by its skeleton (eCognition's skeleton
//! features: main line length and width, curvature, branches).
//!
//! Per object, in its box with a pad of one cell ([`super::each`]): Zhang and Suen's thinning (1984, CACM 27(3):
//! 236-239) over a list of border cells (so the work follows the outline, not the area), the diagonal staircase steps
//! cleaned, the spurs shorter than `spur` cells pruned; then the graph of the skeleton's cells: an end has one run of
//! neighbours around it (crossing number 1), a junction three or more; the main line is the diameter of the graph (two
//! Dijkstra searches, a step of the cell's width, height or diagonal in map units). A cycle does not break the count:
//! a ring is one branch, and its main line is its longest shortest path. The skeleton is of the raster, not of the
//! polygon that eCognition triangulates, so the values differ a little from its own.
//!
//! The objects run in parallel (the biggest first) and the result is in the order of the ids; each thread reuses one
//! set of buffers, so there is no allocation per cell and the result is the same on any number of threads.

use std::cell::RefCell;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::each::{Frame, each};
use super::features::{Columns, Input, Want};
use crate::Result;

pub(super) const SKELETON: &[&str] = &[
    "skel_main_length",
    "skel_width",
    "skel_length_width",
    "skel_branches",
    "skel_ends",
    "skel_junctions",
    "skel_branch_length",
    "skel_curvature",
    "skel_curvature_sd",
];

/// The default `spur`: branches that end in a junction after fewer cells than this are cut.
pub(super) const SPUR: usize = 3;

/// The half width, in cells, of the window that smooths the main line before its turning is measured.
const SMOOTH: usize = 2;

const N: usize = SKELETON.len();

pub(super) fn skeleton(inp: &Input, want: Want) -> Result<Columns> {
    let mut on = [false; N];
    for (name, _) in want {
        if let Some(f) = SKELETON.iter().position(|s| s == name) {
            on[f] = true;
        }
    }
    let o = inp.o;
    let ids: Vec<u32> = (1..=o.count as u32).collect();
    let rows = each(o, &ids, |id, fr| SCRATCH.with(|s| of(&mut s.borrow_mut(), inp, id, fr)));
    Ok(SKELETON
        .iter()
        .enumerate()
        .filter(|(f, _)| on[*f])
        .map(|(f, name)| (name.to_string(), rows.iter().map(|r| r[f]).collect()))
        .collect())
}

/// The buffers one thread reuses from object to object.
#[derive(Default)]
struct Scratch {
    /// Per cell of the frame: bit 0 the cell is in the (skeleton of the) object, 1 queued for thinning, 2 junction,
    /// 3 visited.
    img: Vec<u8>,
    members: Vec<u32>,
    cur: Vec<u32>,
    next: Vec<u32>,
    del: Vec<u32>,
    cells: Vec<u32>,
    stack: Vec<u32>,
    path: Vec<usize>,
    lab: Vec<u32>,
    dist: Vec<f64>,
    pred: Vec<u32>,
    heap: BinaryHeap<Reverse<(u64, u32)>>,
}

thread_local! {
    static SCRATCH: RefCell<Scratch> = RefCell::new(Scratch::default());
}

/// The neighbours clockwise from the north (the P2..P9 of the paper): 1 where the cell is in the skeleton.
#[inline]
fn ring(img: &[u8], i: usize, w: usize) -> [u8; 8] {
    [
        img[i - w] & 1,
        img[i - w + 1] & 1,
        img[i + 1] & 1,
        img[i + w + 1] & 1,
        img[i + w] & 1,
        img[i + w - 1] & 1,
        img[i - 1] & 1,
        img[i - w - 1] & 1,
    ]
}

/// The runs of neighbours around a cell (the 0 to 1 steps of the ring): 1 an end, 2 a line, 3 or more a junction.
#[inline]
fn crossing(p: &[u8; 8]) -> u8 {
    (0..8).filter(|&k| p[k] == 0 && p[(k + 1) % 8] == 1).count() as u8
}

/// Whether a cell with these neighbours can be taken out of the skeleton without changing its topology and without
/// shortening an end: its neighbours (8 neighbours, the north and east ones touch at the corner too) form one
/// piece, and the background that touches it by an edge is one piece as well.
fn simple(p: &[u8; 8]) -> bool {
    if p.iter().sum::<u8>() < 2 {
        return false;
    }
    // (the foreground pieces: a union over the ring and the four corner pairs)
    let mut root = [0usize, 1, 2, 3, 4, 5, 6, 7];
    fn find(root: &mut [usize; 8], mut a: usize) -> usize {
        while root[a] != a {
            root[a] = root[root[a]];
            a = root[a];
        }
        a
    }
    for (a, b) in [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7), (7, 0), (0, 2), (2, 4), (4, 6), (6, 0)] {
        if p[a] == 1 && p[b] == 1 {
            let (ra, rb) = (find(&mut root, a), find(&mut root, b));
            root[ra.max(rb)] = ra.min(rb);
        }
    }
    let pieces = (0..8).filter(|&a| p[a] == 1 && find(&mut root, a) == a).count();
    if pieces != 1 {
        return false;
    }
    // (the background runs around the ring that hold a cell touching by an edge: the even places)
    let runs = (0..8).filter(|&k| p[k] == 0 && p[(k + 7) % 8] == 1).filter(|&k| {
        let mut j = k;
        while p[j % 8] == 0 {
            if j % 2 == 0 {
                return true;
            }
            j += 1;
        }
        false
    });
    runs.count() == 1
}

/// Zhang and Suen's test for the cell `i`, in the first or the second sub-iteration.
#[inline]
fn removable(img: &[u8], i: usize, w: usize, second: bool) -> bool {
    let p = ring(img, i, w);
    let b: u8 = p.iter().sum();
    if !(2..=6).contains(&b) || crossing(&p) != 1 {
        return false;
    }
    let (n, e, s, we) = (p[0], p[2], p[4], p[6]);
    if second { n * e * we == 0 && n * s * we == 0 } else { n * e * s == 0 && e * s * we == 0 }
}

/// The offsets of the 8 neighbours.
fn offsets(w: usize) -> [isize; 8] {
    let w = w as isize;
    [-w, -w + 1, 1, w + 1, w, w - 1, -1, -w - 1]
}

/// The neighbours of `i` joined to it by a step (with its length): a diagonal step is left out when one of the two
/// cells that touch both is in the skeleton (the path goes round by them).
fn adj(img: &[u8], i: usize, w: usize, c: &[f64; 3], out: &mut [(usize, f64); 8]) -> usize {
    let mut k = 0;
    let mut put = |j: usize, d: f64| {
        out[k] = (j, d);
        k += 1;
    };
    let on = |j: usize| img[j] & 1 == 1;
    if on(i - w) {
        put(i - w, c[1]);
    }
    if on(i + 1) {
        put(i + 1, c[0]);
    }
    if on(i + w) {
        put(i + w, c[1]);
    }
    if on(i - 1) {
        put(i - 1, c[0]);
    }
    if on(i - w + 1) && !on(i - w) && !on(i + 1) {
        put(i - w + 1, c[2]);
    }
    if on(i + w + 1) && !on(i + w) && !on(i + 1) {
        put(i + w + 1, c[2]);
    }
    if on(i + w - 1) && !on(i + w) && !on(i - 1) {
        put(i + w - 1, c[2]);
    }
    if on(i - w - 1) && !on(i - w) && !on(i - 1) {
        put(i - w - 1, c[2]);
    }
    k
}

/// Dijkstra from `start` over the skeleton `cells`; the farthest cell (the first on a tie) and its distance. Leaves
/// the paths in `pred`.
fn dijkstra(s: &mut Scratch, w: usize, start: usize, c: &[f64; 3]) -> (usize, f64) {
    let Scratch { img, cells, dist, pred, heap, .. } = s;
    for &i in cells.iter() {
        dist[i as usize] = f64::INFINITY;
    }
    dist[start] = 0.0;
    pred[start] = start as u32;
    heap.clear();
    heap.push(Reverse((0f64.to_bits(), start as u32)));
    let mut out = [(0usize, 0.0f64); 8];
    while let Some(Reverse((bits, i))) = heap.pop() {
        let i = i as usize;
        let d = f64::from_bits(bits);
        if d > dist[i] {
            continue;
        }
        let n = adj(img, i, w, c, &mut out);
        for &(j, step) in &out[..n] {
            let nd = d + step;
            if nd < dist[j] {
                dist[j] = nd;
                pred[j] = i as u32;
                heap.push(Reverse((nd.to_bits(), j as u32)));
            }
        }
    }
    let (mut far, mut best) = (start, 0.0);
    for &i in cells.iter() {
        let d = dist[i as usize];
        if d.is_finite() && d > best {
            (far, best) = (i as usize, d);
        }
    }
    (far, best)
}

/// The object's skeleton numbers, in the order of [`SKELETON`].
fn of(s: &mut Scratch, inp: &Input, id: u32, fr: &Frame) -> [f64; N] {
    let o = inp.o;
    let (gw, gh) = (o.grid.width as i64, o.grid.height as i64);
    let (rw, rh) = o.grid.resolution();
    let (rw, rh) = (rw.abs(), rh.abs());
    let costs = [rw, rh, rw.hypot(rh)];
    let (w, h) = (fr.w, fr.h);
    let size = w * h;
    if s.lab.len() < size {
        s.lab.resize(size, 0);
        s.dist.resize(size, 0.0);
        s.pred.resize(size, 0);
    }
    s.img.clear();
    s.img.resize(size, 0);
    s.members.clear();
    for r in 0..h {
        let gr = fr.top + r as i64;
        if gr < 0 || gr >= gh {
            continue;
        }
        for c in 0..w {
            let gc = fr.left + c as i64;
            if gc >= 0 && gc < gw && o.labels[(gr * gw + gc) as usize] == id {
                s.img[r * w + c] = 1;
                s.members.push((r * w + c) as u32);
            }
        }
    }
    let area = s.members.len() as f64 * rw * rh;
    let offs = offsets(w);
    thin(s, w, &offs);
    prune(s, w, &offs, inp.spur);
    let mut out = [f64::NAN; N];
    // the graph: ends, junction cells and clusters, the branches and their lengths
    let Scratch { img, cells, stack, lab, .. } = s;
    let mut ends = 0usize;
    for &i in cells.iter() {
        let p = ring(img, i as usize, w);
        match crossing(&p) {
            1 => ends += 1,
            3.. => img[i as usize] |= 4,
            _ => {}
        }
    }
    let mut junctions = 0usize;
    for &i in cells.iter() {
        let i = i as usize;
        if img[i] & 4 != 0 && img[i] & 8 == 0 {
            junctions += 1;
            img[i] |= 8;
            stack.push(i as u32);
            while let Some(a) = stack.pop() {
                for d in offs {
                    let j = (a as isize + d) as usize;
                    if img[j] & 4 != 0 && img[j] & 8 == 0 {
                        img[j] |= 8;
                        stack.push(j as u32);
                    }
                }
            }
        }
    }
    let mut lens: Vec<f64> = Vec::new();
    let mut near = [(0usize, 0.0f64); 8];
    for &i in cells.iter() {
        let i = i as usize;
        if img[i] & 4 == 0 && lab[i] == 0 {
            lens.push(0.0);
            let id = lens.len() as u32;
            lab[i] = id;
            stack.push(i as u32);
            // (along the steps of the graph: the arms of a cross touch at their corners, they are not one branch)
            while let Some(a) = stack.pop() {
                let n = adj(img, a as usize, w, &costs, &mut near);
                for &(j, _) in &near[..n] {
                    if img[j] & 4 == 0 && lab[j] == 0 {
                        lab[j] = id;
                        stack.push(j as u32);
                    }
                }
            }
        }
    }
    for &i in cells.iter() {
        let i = i as usize;
        for (j, cost, shared) in [
            (i + 1, costs[0], None),
            (i + w, costs[1], None),
            (i + w + 1, costs[2], Some((i + 1, i + w))),
            (i + w - 1, costs[2], Some((i - 1, i + w))),
        ] {
            if img[j] & 1 == 0 || shared.is_some_and(|(a, b)| img[a] & 1 == 1 || img[b] & 1 == 1) {
                continue;
            }
            let b = if lab[i] != 0 { lab[i] } else { lab[j] };
            if b != 0 {
                lens[b as usize - 1] += cost;
            }
        }
    }
    let branches = lens.len();
    out[3] = branches as f64;
    out[4] = ends as f64;
    out[5] = junctions as f64;
    out[6] = lens.iter().sum::<f64>() / branches as f64;
    for &i in cells.iter() {
        lab[i as usize] = 0;
    }
    // the main line: the diameter by two searches
    let start = cells[0] as usize;
    let (a, _) = dijkstra(s, w, start, &costs);
    let (b, length) = dijkstra(s, w, a, &costs);
    out[0] = length;
    if length > 0.0 {
        out[1] = area / length;
        out[2] = length / out[1];
        let mut pts: Vec<(f64, f64)> = Vec::new();
        let mut at = b;
        loop {
            pts.push(((at % w) as f64 * rw, (at / w) as f64 * rh));
            if at == a {
                break;
            }
            at = s.pred[at] as usize;
        }
        let (sum, sd) = curve(&pts);
        out[7] = sum / length;
        out[8] = sd;
    }
    out
}

/// Zhang and Suen's thinning of the cells in `s.img` (the object's), with the staircase cleaned; the cells left are in
/// `s.cells`, ascending (at least one: a shape the thinning erases, a 2 by 2 square, keeps its cell nearest the middle).
fn thin(s: &mut Scratch, w: usize, offs: &[isize; 8]) {
    let Scratch { img, members, cur, next, del, cells, .. } = s;
    cur.clear();
    for &i in members.iter() {
        let i = i as usize;
        if ring(img, i, w).contains(&0) {
            img[i] |= 2;
            cur.push(i as u32);
        }
    }
    loop {
        let mut any = false;
        for second in [false, true] {
            del.clear();
            del.extend(cur.iter().copied().filter(|&i| removable(img, i as usize, w, second)));
            if del.is_empty() {
                continue;
            }
            any = true;
            for &i in del.iter() {
                img[i as usize] = 0;
            }
            next.clear();
            next.extend(cur.iter().copied().filter(|&i| img[i as usize] & 1 == 1));
            for &i in del.iter() {
                for d in offs {
                    let j = (i as isize + d) as usize;
                    if img[j] == 1 {
                        img[j] = 3;
                        next.push(j as u32);
                    }
                }
            }
            std::mem::swap(cur, next);
        }
        if !any {
            break;
        }
    }
    cells.clear();
    for &i in members.iter() {
        let i = i as usize;
        img[i] &= 1;
        if img[i] == 1 {
            cells.push(i as u32);
        }
    }
    if cells.is_empty() {
        let n = members.len() as f64;
        let (mr, mc) = (
            members.iter().map(|&i| (i as usize / w) as f64).sum::<f64>() / n,
            members.iter().map(|&i| (i as usize % w) as f64).sum::<f64>() / n,
        );
        let near = |i: u32| ((i as usize / w) as f64 - mr).powi(2) + ((i as usize % w) as f64 - mc).powi(2);
        let best = members.iter().copied().fold(members[0], |b, i| if near(i) < near(b) { i } else { b });
        img[best as usize] = 1;
        cells.push(best);
        return;
    }
    // the staircase steps and the two-cell wide diagonals the thinning leaves: the cells that can go without changing
    // the topology (ends stay), in the order of the cells
    for &i in cells.iter() {
        let i = i as usize;
        if simple(&ring(img, i, w)) {
            img[i] = 0;
        }
    }
    cells.retain(|&i| img[i as usize] & 1 == 1);
}

/// The spurs cut: from each end, the cells up to a junction when they are fewer than `spur`.
fn prune(s: &mut Scratch, w: usize, offs: &[isize; 8], spur: usize) {
    let Scratch { img, cells, path, .. } = s;
    if spur == 0 {
        return;
    }
    let ends: Vec<usize> = cells.iter().map(|&i| i as usize).filter(|&i| crossing(&ring(img, i, w)) == 1).collect();
    // (the 4 neighbours first)
    let order = [offs[0], offs[2], offs[4], offs[6], offs[1], offs[3], offs[5], offs[7]];
    for e in ends {
        if img[e] & 1 == 0 || crossing(&ring(img, e, w)) != 1 {
            continue;
        }
        path.clear();
        path.push(e);
        let mut at = e;
        loop {
            let next =
                order.iter().map(|d| (at as isize + d) as usize).find(|&j| img[j] & 1 == 1 && !path.contains(&j));
            let Some(j) = next else { break };
            if crossing(&ring(img, j, w)) >= 3 {
                if path.len() < spur {
                    for &i in path.iter() {
                        img[i] = 0;
                    }
                }
                break;
            }
            path.push(j);
            at = j;
            if path.len() >= spur {
                break;
            }
        }
    }
    cells.retain(|&i| img[i as usize] & 1 == 1);
}

/// The total turning (radians) of the line after smoothing, and the standard deviation of the turning per unit length
/// at its vertices.
fn curve(pts: &[(f64, f64)]) -> (f64, f64) {
    let n = pts.len();
    if n < 3 {
        return (0.0, 0.0);
    }
    let sm: Vec<(f64, f64)> = (0..n)
        .map(|i| {
            let m = SMOOTH.min(i).min(n - 1 - i);
            let k = (2 * m + 1) as f64;
            let (x, y) = pts[i - m..=i + m].iter().fold((0.0, 0.0), |a, p| (a.0 + p.0, a.1 + p.1));
            (x / k, y / k)
        })
        .collect();
    let (mut total, mut local) = (0.0f64, Vec::with_capacity(n));
    for k in 1..n - 1 {
        let (v1, v2) = ((sm[k].0 - sm[k - 1].0, sm[k].1 - sm[k - 1].1), (sm[k + 1].0 - sm[k].0, sm[k + 1].1 - sm[k].1));
        let (l1, l2) = (v1.0.hypot(v1.1), v2.0.hypot(v2.1));
        if l1 == 0.0 || l2 == 0.0 {
            local.push(0.0);
            continue;
        }
        let turn = (v1.0 * v2.1 - v1.1 * v2.0).atan2(v1.0 * v2.0 + v1.1 * v2.1).abs();
        total += turn;
        local.push(turn / ((l1 + l2) / 2.0));
    }
    let mean = local.iter().sum::<f64>() / local.len() as f64;
    let var = local.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / local.len() as f64;
    (total, var.sqrt())
}
