//! Partitions of records for cross-validation (landschaft's folds, after blockCV and ENMeval): random folds of
//! the presences, jackknife, the four quadrants, a checkerboard of aggregated cells, spatial blocks (squares or
//! hexagons, assigned at random to balance the folds, systematically or as a checkerboard) and clusters of the
//! coordinates or of the predictors (R's kmeans). R's generator and arithmetic throughout.

use terraflux_gdal::Geos;
use terraflux_ls_sys::ffi;
use terraflux_raster::Grid;
use terraflux_raster::vector::Geometry;

use super::evaluate::r_seq;

const EPS: f64 = f64::EPSILON;

/// A number with a 64-bit mantissa (an x87 long double): sign, mantissa and exponent.
#[derive(Clone, Copy, Debug)]
struct Ext {
    neg: bool,
    m: u128,
    e: i32,
}

impl Ext {
    fn from_f64(x: f64) -> Ext {
        if x == 0.0 {
            return Ext { neg: false, m: 0, e: 0 };
        }
        let bits = x.to_bits();
        let exp = ((bits >> 52) & 0x7ff) as i32;
        let frac = (bits & ((1u64 << 52) - 1)) as u128;
        let (m, e) = if exp == 0 { (frac, -1074) } else { (frac | (1 << 52), exp - 1075) };
        Ext { neg: x < 0.0, m, e }.round()
    }

    /// Rounded to 64 bits of mantissa, half to even.
    fn round(self) -> Ext {
        if self.m == 0 {
            return Ext { neg: false, m: 0, e: 0 };
        }
        let bits = 128 - self.m.leading_zeros() as i32;
        if bits <= 64 {
            return self;
        }
        let s = bits - 64;
        let (q, r) = (self.m >> s, self.m & ((1u128 << s) - 1));
        let half = 1u128 << (s - 1);
        let q = if r > half || (r == half && q & 1 == 1) { q + 1 } else { q };
        Ext { neg: self.neg, m: q, e: self.e + s }.round()
    }

    fn mul(self, o: Ext) -> Ext {
        Ext { neg: self.neg != o.neg, m: self.m * o.m, e: self.e + o.e }.round()
    }

    fn sub(self, o: Ext) -> Ext {
        let o = Ext { neg: !o.neg, ..o };
        if self.m == 0 {
            return o;
        }
        if o.m == 0 {
            return self;
        }
        let e = self.e.min(o.e);
        let (da, db) = ((self.e - e) as u32, (o.e - e) as u32);
        if da > 60 || db > 60 {
            // too far apart for the exact sum here: the larger, rounded
            return if da > db { self } else { o };
        }
        let (a, b) = (self.m << da, o.m << db);
        let (neg, m) = match (self.neg, o.neg) {
            (x, y) if x == y => (x, a + b),
            (x, _) => {
                if a >= b {
                    (x, a - b)
                } else {
                    (!x, b - a)
                }
            }
        };
        Ext { neg, m, e }.round()
    }

    fn div(self, o: Ext) -> Ext {
        if self.m == 0 {
            return self;
        }
        // both mantissas to 64 bits, the quotient to 65 or 64 bits with the remainder for the rounding
        let (a, ea) = (self.m << (self.m.leading_zeros() - 64), self.e - (self.m.leading_zeros() as i32 - 64));
        let (b, eb) = (o.m << (o.m.leading_zeros() - 64), o.e - (o.m.leading_zeros() as i32 - 64));
        let num = a << 64;
        let (q, r) = (num / b, num % b);
        // fold the remainder into a sticky bit below the quotient
        let q2 = (q << 2)
            | if r == 0 {
                0
            } else if 2 * r > b {
                3
            } else if 2 * r == b {
                2
            } else {
                1
            };
        Ext { neg: self.neg != o.neg, m: q2, e: ea - eb - 64 - 2 }.round()
    }

    /// floor, as a double (whole numbers this large are exact).
    fn floor(self) -> f64 {
        let v = self.to_f64();
        let f = v.floor();
        if f != v {
            return f;
        }
        // v rounded to an integer: check against the exact value
        let back = Ext::from_f64(f);
        let d = self.sub(back);
        if d.m != 0 && d.neg { f - 1.0 } else { f }
    }

    fn to_f64(self) -> f64 {
        if self.m == 0 {
            return 0.0;
        }
        let v = (self.m as u64 as f64) * 2f64.powi(self.e);
        if self.neg { -v } else { v }
    }
}

/// R's fmod (landschaft's _r_fmod): its long double steps with 64-bit mantissas.
fn r_fmod(x1: f64, x2: f64) -> f64 {
    if x2.abs() * EPS > 1.0 && x1.is_finite() && x1.abs() <= x2.abs() {
        if x1.abs() == x2.abs() {
            return 0.0;
        }
        return if (x1 < 0.0 && 0.0 < x2) || (x2 < 0.0 && 0.0 < x1) { x1 + x2 } else { x1 };
    }
    let q = x1 / x2;
    let (a, b) = (Ext::from_f64(x1), Ext::from_f64(x2));
    let tmp = a.sub(Ext::from_f64(q.floor()).mul(b));
    let fl = tmp.div(b).floor();
    tmp.sub(Ext::from_f64(fl).mul(b)).to_f64()
}

/// The records: their coordinates, whether they are lon/lat, and their classes (presence 1, background 0).
pub struct Records<'a> {
    pub x: &'a [f64],
    pub y: &'a [f64],
    pub lonlat: bool,
    pub classes: Option<&'a [f64]>,
}

/// Whether coordinates are lon/lat: their coordinate system is geographic, or (none known) they fit.
pub fn is_lonlat(crs: Option<&str>, x: &[f64], y: &[f64]) -> bool {
    match crs {
        Some(c) => terraflux_gdal::is_geographic(c).unwrap_or(false),
        None => x.iter().zip(y).all(|(a, b)| (-180.0..=180.0).contains(a) && (-180.0..=180.0).contains(b)),
    }
}

/// Random folds of the presences (ENMeval's get.randomkfold): k groups of near-equal size, assigned after R's
/// runif; background 0. Without classes, every record.
pub fn random_folds(n_all: usize, classes: Option<&[f64]>, k: usize, seed: i32) -> Result<Vec<i64>, String> {
    let pres: Vec<usize> = match classes {
        Some(c) => (0..n_all).filter(|&i| c[i] == 1.0).collect(),
        None => (0..n_all).collect(),
    };
    let n = pres.len();
    if (n as f64) / (k as f64) < 1.0 {
        return Err("more folds than presences".into());
    }
    let mut bounds = vec![0i64];
    for j in 1..k {
        bounds.push((n as f64 / k as f64 * j as f64).round_ties_even() as i64);
    }
    bounds.push(n as i64);
    let group: Vec<i64> =
        (0..k).flat_map(|g| std::iter::repeat_n(g as i64 + 1, (bounds[g + 1] - bounds[g]) as usize)).collect();
    let u = ffi::rrng_runif(ffi::rrng_new(seed).pin_mut(), n);
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| u[a].partial_cmp(&u[b]).unwrap());
    let mut folds = vec![0i64; n_all];
    // numpy's group[argsort(u)]: presence i takes the group at the i-th position of the order
    for (i, &o) in order.iter().enumerate() {
        folds[pres[i]] = group[o];
    }
    Ok(folds)
}

/// Jackknife: every presence its own fold; background 0.
pub fn jackknife_folds(classes: &[f64]) -> Vec<i64> {
    let mut k = 0;
    classes
        .iter()
        .map(|&c| {
            if c == 1.0 {
                k += 1;
                k
            } else {
                0
            }
        })
        .collect()
}

/// R's mean of two numbers.
fn r_mean2(a: f64, b: f64) -> f64 {
    ffi::cost_r_mean(&[a, b])
}

/// ENMeval's block partition: the presences in four quadrants of near-equal counts (split first along the
/// orientation's first axis), the background by the same lines.
pub fn quadrant_folds(r: &Records, orientation: &str) -> Result<Vec<i64>, String> {
    let classes = r.classes.ok_or("quadrant folds need presences and background")?;
    if !["lat_lon", "lon_lat", "lon_lon", "lat_lat"].contains(&orientation) {
        return Err("orientation must be lat_lon, lon_lat, lon_lon or lat_lat".into());
    }
    let pres: Vec<usize> = (0..classes.len()).filter(|&i| classes[i] == 1.0).collect();
    let bgi: Vec<usize> = (0..classes.len()).filter(|&i| classes[i] == 0.0).collect();
    let occ = |i: usize, d: usize| if d == 0 { r.x[pres[i]] } else { r.y[pres[i]] };
    let n = pres.len();
    let (n1, n2) = (n.div_ceil(2), n / 2);
    let (n3, n4) = (n1.div_ceil(2), n2.div_ceil(2));
    let d1 = usize::from(orientation == "lat_lon" || orientation == "lat_lat");
    let d2 = usize::from(orientation != "lat_lon" && orientation != "lon_lon");
    let mut o: Vec<usize> = (0..n).collect();
    o.sort_by(|&a, &b| occ(a, d1).partial_cmp(&occ(b, d1)).unwrap());
    let grp_a: Vec<usize> = o[..n1].to_vec();
    let grp_b: Vec<usize> = o.iter().rev().take(n2).copied().collect();
    let sorted = |g: &[usize]| -> Vec<usize> {
        let mut s = g.to_vec();
        s.sort_by(|&a, &b| occ(a, d2).partial_cmp(&occ(b, d2)).unwrap());
        s
    };
    let a_sorted = sorted(&grp_a);
    let g1 = a_sorted[..n3].to_vec();
    let g2: Vec<usize> = grp_a.iter().copied().filter(|i| !g1.contains(i)).collect();
    let b_sorted = sorted(&grp_b);
    let g3 = b_sorted[..n4.min(b_sorted.len())].to_vec();
    let g4: Vec<usize> = grp_b.iter().copied().filter(|i| !g3.contains(i)).collect();
    let mx = |g: &[usize], d: usize| g.iter().map(|&i| occ(i, d)).fold(f64::NEG_INFINITY, f64::max);
    let mn = |g: &[usize], d: usize| g.iter().map(|&i| occ(i, d)).fold(f64::INFINITY, f64::min);
    let mut bgg = vec![0i64; bgi.len()];
    for (k, &i) in bgi.iter().enumerate() {
        let (bx, by) = (r.x[i], r.y[i]);
        let conds: [bool; 4] = match orientation {
            "lat_lon" => {
                let (bvert, tvert) = (r_mean2(mx(&g1, 0), mn(&g2, 0)), r_mean2(mx(&g3, 0), mn(&g4, 0)));
                let horz = r_mean2(mx(&grp_a, 1), mn(&grp_b, 1));
                [by <= horz && bx < bvert, by <= horz && bx >= bvert, by > horz && bx <= tvert, by > horz && bx > tvert]
            }
            "lon_lat" => {
                let (bhorz, thorz) = (r_mean2(mx(&g1, 1), mn(&g2, 1)), r_mean2(mx(&g3, 1), mn(&g4, 1)));
                let vert = r_mean2(mx(&grp_a, 0), mn(&grp_b, 0));
                [bx <= vert && by < bhorz, bx <= vert && by >= bhorz, bx > vert && by <= thorz, bx > vert && by > thorz]
            }
            "lon_lon" => {
                let (bvert, tvert) = (r_mean2(mx(&g1, 0), mn(&g2, 0)), r_mean2(mx(&g3, 0), mn(&g4, 0)));
                let vert = r_mean2(mx(&grp_a, 0), mn(&grp_b, 0));
                [bx <= bvert, bx > bvert && bx <= vert, bx > vert && bx < tvert, bx >= tvert]
            }
            _ => {
                let (bhorz, thorz) = (r_mean2(mx(&g1, 1), mn(&g2, 1)), r_mean2(mx(&g3, 1), mn(&g4, 1)));
                let horz = r_mean2(mx(&grp_a, 1), mn(&grp_b, 1));
                [by <= bhorz, by > bhorz && by <= horz, by > horz && by < thorz, by >= thorz]
            }
        };
        if let Some(g) = conds.iter().position(|c| *c) {
            bgg[k] = g as i64 + 1;
        }
    }
    let mut folds = vec![0i64; classes.len()];
    for (g, idx) in [&g1, &g2, &g3, &g4].iter().enumerate() {
        for &i in idx.iter() {
            folds[pres[i]] = g as i64 + 1;
        }
    }
    for (k, &i) in bgi.iter().enumerate() {
        folds[i] = bgg[k];
    }
    Ok(folds)
}

/// ENMeval's checkerboard: the cells of the predictors aggregated by a factor (or two, hierarchically), the
/// records in their white and black squares.
pub fn checkerboard_folds(r: &Records, grid: &Grid, aggregation: &[usize]) -> Result<Vec<i64>, String> {
    let classes = r.classes.ok_or("checkerboard folds need presences and background")?;
    if aggregation.is_empty() || aggregation.len() > 2 {
        return Err("aggregation: one factor (basic) or two (hierarchical)".into());
    }
    let t = grid.transform;
    let (ny, nx) = (grid.height, grid.width);
    let (xmin, ymax) = (t[0], t[3]);
    let xres = (xmin + nx as f64 * t[1] - xmin) / nx as f64;
    let yres = (ymax - (ymax + ny as f64 * t[5])) / ny as f64;
    let white = |f: usize| -> Vec<(f64, bool)> {
        let (anx, any) = (nx.div_ceil(f), ny.div_ceil(f));
        let (axres, ayres) = (xres * f as f64, yres * f as f64);
        let (axmax, aymin) = (xmin + anx as f64 * axres, ymax - any as f64 * ayres);
        r.x.iter()
            .zip(r.y)
            .map(|(&x, &y)| {
                let col =
                    if x == axmax { (anx - 1) as f64 } else { ((x - xmin) * (anx as f64 / (axmax - xmin))).floor() };
                let row =
                    if y == aymin { (any - 1) as f64 } else { ((ymax - y) * (any as f64 / (ymax - aymin))).floor() };
                let inside = x >= xmin && x <= axmax && y >= aymin && y <= ymax;
                let w = if (row + col) % 2.0 == 0.0 { 1.0 } else { 0.0 };
                (if inside { w } else { f64::NAN }, inside)
            })
            .collect()
    };
    let w1 = white(aggregation[0]);
    let grp: Vec<i64> = if aggregation.len() == 1 {
        w1.iter()
            .map(|&(w, inside)| {
                if !inside {
                    0
                } else if w == 1.0 {
                    1
                } else {
                    2
                }
            })
            .collect()
    } else {
        let w2 = white(aggregation[0] * aggregation[1]);
        w1.iter()
            .zip(&w2)
            .map(|(&(a, ia), &(b, ib))| {
                if !(ia && ib) {
                    0
                } else if a == 1.0 && b == 1.0 {
                    1
                } else if a == 1.0 && b == 0.0 {
                    2
                } else if a == 0.0 && b == 1.0 {
                    3
                } else {
                    4
                }
            })
            .collect()
    };
    let groups = |c: f64| -> std::collections::BTreeSet<i64> {
        (0..grp.len()).filter(|&i| classes[i] == c && grp[i] > 0).map(|i| grp[i]).collect()
    };
    let (occ, bg) = (groups(1.0), groups(0.0));
    if !occ.is_subset(&bg) {
        return Err("some fold has occurrences but no background: use a smaller aggregation".into());
    }
    if occ.len() < 2 {
        return Err("the occurrences fall in a single square: use a smaller aggregation".into());
    }
    Ok(grp)
}

/// The settings of spatial_blocks.
pub struct Blocks<'a> {
    pub size: Option<f64>,
    pub rows_cols: (usize, usize),
    pub k: usize,
    pub hexagon: bool,
    pub flat_top: bool,
    /// random, systematic or checkerboard.
    pub selection: &'a str,
    pub iteration: usize,
    pub offset: (f64, f64),
    pub extend: f64,
    pub deg_to_metre: f64,
    pub seed: i32,
}

/// (record, block) pairs: each record in each block it falls in.
type Pairs = Vec<(usize, usize)>;

fn square_blocks(r: &Records, b: &Blocks, extend: f64, checkerboard: bool) -> (Pairs, usize, Option<Vec<i64>>) {
    let (x, y) = (r.x, r.y);
    let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (xmin0, xmax0, ymin0, ymax0) = (min(x), max(x), min(y), max(y));
    let mut ext = [xmin0, xmax0, ymin0, ymax0];
    let (mut nrows, mut ncols) = b.rows_cols;
    let mut size = b.size;
    let mut off = [0.0, 0.0];
    if let Some(s) = size.as_mut() {
        if r.lonlat {
            *s /= b.deg_to_metre;
        }
        off = [*s * (b.offset.0.abs() % 1.0), *s * (b.offset.1.abs() % 1.0)];
    }
    let (xrange, yrange) = (xmax0 - xmin0, ymax0 - ymin0);
    ext[0] -= xrange * extend;
    ext[1] += xrange * extend;
    ext[2] -= yrange * extend;
    ext[3] += yrange * extend;
    if let Some(s) = size {
        let (mut xpix, mut ypix) = ((xrange / s).ceil() as usize, (yrange / s).ceil() as usize);
        let xdif = (xpix as f64 * s - xrange) / 2.0;
        let ydif = (ypix as f64 * s - yrange) / 2.0;
        ext[0] = ext[0] - xdif + off[0];
        ext[1] = ext[1] + xdif + off[0];
        ext[2] = ext[2] - ydif + off[1];
        ext[3] = ext[3] + ydif + off[1];
        if ext[0] > xmin0 {
            ext[0] -= s;
            xpix += 1;
        }
        if ext[2] > ymin0 {
            ext[2] -= s;
            ypix += 1;
        }
        ncols = xpix;
        nrows = ypix;
    }
    let (nrows, ncols) = (nrows.max(1), ncols.max(1));
    let [gxmin, gxmax, gymin, gymax] = ext;
    let xres = (gxmax - gxmin) / ncols as f64;
    let yres = (gymax - gymin) / nrows as f64;
    let cx: Vec<f64> = (0..ncols).map(|c| gxmin + (c as f64 + 0.5) * xres).collect();
    let cy: Vec<f64> = (0..nrows).map(|r| gymax - (r as f64 + 0.5) * yres).collect();
    let (hx, hy) = (xres / 2.0, yres / 2.0);
    let mut pairs = Vec::new();
    for dr in [-1i64, 0, 1] {
        for dc in [-1i64, 0, 1] {
            for i in 0..x.len() {
                let row0 = ((gymax - y[i]) / yres).floor() as i64;
                let col0 = ((x[i] - gxmin) / xres).floor() as i64;
                let (rr, cc) = (row0 + dr, col0 + dc);
                if rr < 0 || rr >= nrows as i64 || cc < 0 || cc >= ncols as i64 {
                    continue;
                }
                let (rr, cc) = (rr as usize, cc as usize);
                if x[i] >= cx[cc] - hx && x[i] <= cx[cc] + hx && y[i] >= cy[rr] - hy && y[i] <= cy[rr] + hy {
                    pairs.push((i, rr * ncols + cc));
                }
            }
        }
    }
    let ids = checkerboard.then(|| {
        (0..nrows * ncols)
            .map(|k| {
                let (rr, cc) = (k / ncols, k % ncols);
                if (rr + 1) % 2 == 0 {
                    if cc % 2 == 0 { 1 } else { 2 }
                } else if cc % 2 == 0 {
                    2
                } else {
                    1
                }
            })
            .collect()
    });
    (pairs, nrows * ncols, ids)
}

fn hex_blocks(r: &Records, b: &Blocks) -> Result<(Pairs, Vec<Geometry>), String> {
    let (x, y) = (r.x, r.y);
    let min = |v: &[f64]| v.iter().copied().fold(f64::INFINITY, f64::min);
    let max = |v: &[f64]| v.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (bxmin, bxmax, bymin, bymax) = (min(x), max(x), min(y), max(y));
    let (xy_off, hexsize) = match b.size {
        Some(s) => {
            let fix = |o: f64| if o != 0.0 && o != 1.0 { 1.0 - o } else { o };
            let s = if r.lonlat { s / b.deg_to_metre } else { s };
            ([s * (fix(b.offset.0).abs() % 1.0), s * (fix(b.offset.1).abs() % 1.0)], s)
        }
        None => ([0.0, 0.0], (bxmax - bxmin) / b.rows_cols.0 as f64),
    };
    let mut pt = [bxmin - xy_off[0], bymin - xy_off[1]];
    let dx = hexsize / 3f64.sqrt();
    let dy = 3f64.sqrt() * dx / 2.0;
    let (xlim, ylim) = if !b.flat_top {
        pt.reverse();
        ((bymin, bymax), (bxmin, bxmax))
    } else {
        ((bxmin, bxmax), (bymin, bymax))
    };
    let ox = r_fmod(pt[0] - xlim.0, dx);
    let oy = r_fmod(pt[1] - ylim.0, 2.0 * dy);
    let x0: Vec<f64> = r_seq(xlim.0 - dx, xlim.1 + 2.0 * dx, dx).into_iter().map(|v| v + ox).collect();
    let y0: Vec<f64> = r_seq(ylim.0 - 2.0 * dy, ylim.1 + 2.0 * dy, dy).into_iter().map(|v| v + oy).collect();
    let (nx, ny) = (x0.len(), y0.len());
    let len = nx * ny;
    let gy: Vec<f64> = y0.iter().flat_map(|v| std::iter::repeat_n(*v, nx)).collect();
    let both: Vec<f64> = x0.iter().copied().chain(x0.iter().map(|v| v + dx / 2.0)).collect();
    let gx: Vec<f64> = (0..len).map(|k| both[k % both.len()]).collect();
    let pattern: Vec<i64> = (1..2 * nx as i64).step_by(2).chain((2..=2 * nx as i64).step_by(2)).collect();
    let xi: Vec<i64> = (0..len).map(|k| pattern[k % pattern.len()]).collect();
    let yi: Vec<i64> = (1..=ny as i64).flat_map(|v| std::iter::repeat_n(v, nx)).collect();
    let (xmax_i, ymax_i) = (*xi.iter().max().unwrap(), *yi.iter().max().unwrap());
    let offsets = [[-2i64, 0], [-1, -1], [1, -1], [2, 0], [1, 1], [-1, 1], [-2, 0]];
    let mut polys = Vec::new();
    for k in 0..len {
        if !(xi[k] >= 3 && xi[k] <= xmax_i - 2 && xi[k] % 3 == 0 && yi[k] > 1 && yi[k] < ymax_i) {
            continue;
        }
        let ring: Vec<[f64; 2]> = offsets
            .iter()
            .map(|o| {
                let (mx, my) = (xi[k] + o[0], yi[k] + o[1]);
                let i = ((mx - 1).div_euclid(2) + 1 + (my - 1) * nx as i64 - 1) as usize;
                if b.flat_top { [gx[i], gy[i]] } else { [gy[i], gx[i]] }
            })
            .collect();
        polys.push(Geometry::Polygon(vec![ring]));
    }
    let geos = Geos::new(&polys)?;
    let mut pairs = Vec::new();
    for i in 0..x.len() {
        let p = Geometry::Point([x[i], y[i]]);
        for k in 0..polys.len() {
            if geos.intersects_with(k, &p)? {
                pairs.push((i, k));
            }
        }
    }
    Ok((pairs, polys))
}

/// blockCV's cv_spatial: the records in blocks (squares or hexagons), the blocks assigned to k folds (at
/// random, the most balanced of the iterations; systematically; or as a checkerboard).
pub fn spatial_blocks(r: &Records, b: &Blocks) -> Result<Vec<i64>, String> {
    if !["random", "systematic", "checkerboard"].contains(&b.selection) {
        return Err("selection must be random, systematic or checkerboard".into());
    }
    if b.hexagon && b.selection == "checkerboard" {
        return Err("checkerboard selection needs square blocks (hexagon false)".into());
    }
    let n = r.x.len();
    let k = if b.selection == "checkerboard" { 2 } else { b.k };
    let extend = 5f64.min(b.extend.abs()) / 100.0;
    let iteration = b.iteration.max(1);
    let (pairs, nblocks, ids, polys) = if b.hexagon {
        let (p, polys) = hex_blocks(r, b)?;
        let nb = polys.len();
        (p, nb, None, polys)
    } else {
        let (p, nb, ids) = square_blocks(r, b, extend, b.selection == "checkerboard");
        (p, nb, ids, Vec::new())
    };
    let mut used: Vec<usize> = pairs.iter().map(|p| p.1).collect();
    used.sort_unstable();
    used.dedup();
    let mut rank = vec![usize::MAX; nblocks];
    for (j, &u) in used.iter().enumerate() {
        rank[u] = j;
    }
    let mut pairs: Pairs = pairs.iter().map(|&(i, blk)| (i, rank[blk])).collect();
    pairs.sort();
    let nb = used.len();
    if k > nb {
        return Err(format!("k is larger than the number of blocks holding points ({nb})"));
    }
    if k < 2 {
        return Err("k must be 2 or more".into());
    }
    if pairs.len() > n {
        let mut rng = ffi::rrng_new(b.seed);
        let perm = ffi::rrng_sample_int(rng.pin_mut(), pairs.len() as i64, pairs.len() as i64, false, &[])
            .map_err(|e| e.to_string())?;
        let p2: Pairs = perm.iter().map(|&j| pairs[j as usize - 1]).collect();
        let mut seen = std::collections::HashSet::new();
        pairs = p2.into_iter().filter(|p| seen.insert(p.0)).collect();
    }
    let (rec, blk): (Vec<usize>, Vec<usize>) = pairs.iter().copied().unzip();
    let mut rng = ffi::rrng_new(b.seed);
    let classes: Option<Vec<f64>> = r.classes.map(|c| {
        let mut v = c.to_vec();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        v.dedup();
        v
    });
    let table = |fold_of_block: &[i64]| -> Vec<f64> {
        let f: Vec<i64> = blk.iter().map(|&b| fold_of_block[b]).collect();
        let mut cols = Vec::new();
        match (&classes, r.classes) {
            (Some(cl), Some(c)) => {
                for test in [false, true] {
                    for class in cl {
                        for p in 1..=k as i64 {
                            cols.push(
                                (0..f.len()).filter(|&t| (f[t] == p) == test && c[rec[t]] == *class).count() as f64
                            );
                        }
                    }
                }
            }
            _ => {
                for test in [false, true] {
                    for p in 1..=k as i64 {
                        cols.push((0..f.len()).filter(|&t| (f[t] == p) == test).count() as f64);
                    }
                }
            }
        }
        cols
    };
    let best: Vec<i64> = match b.selection {
        "random" => {
            let (mut min_num, mut max_sd) = (0.0, f64::INFINITY);
            let mut best = Vec::new();
            for _ in 0..iteration {
                let mut fb = vec![0i64; nb];
                let num = nb / k;
                let reps: Vec<i64> = (0..num * k).map(|j| (j % k) as i64 + 1).collect();
                let perm = ffi::rrng_sample_int(rng.pin_mut(), (num * k) as i64, (num * k) as i64, false, &[])
                    .map_err(|e| e.to_string())?;
                for (j, &p) in perm.iter().enumerate() {
                    fb[j] = reps[p as usize - 1];
                }
                let rest = nb % k;
                if rest > 0 {
                    let s = ffi::rrng_sample_int(rng.pin_mut(), k as i64, rest as i64, false, &[])
                        .map_err(|e| e.to_string())?;
                    for (j, &v) in s.iter().enumerate() {
                        fb[num * k + j] = v;
                    }
                }
                let t = table(&fb);
                let sd = ffi::sdm_r_sd(&t);
                let tmin = t.iter().copied().fold(f64::INFINITY, f64::min);
                if tmin >= min_num && sd < max_sd {
                    min_num = tmin;
                    max_sd = sd;
                    best = fb;
                }
            }
            best
        }
        "systematic" => {
            if b.hexagon {
                let used_polys: Vec<Geometry> = used.iter().map(|&u| polys[u].clone()).collect();
                let g = Geos::new(&used_polys)?;
                let mut keys = Vec::with_capacity(used_polys.len());
                for j in 0..used_polys.len() {
                    let c = g.centroid(j)?[1];
                    keys.push(format!("{c:.14e}").parse::<f64>().unwrap());
                }
                let mut order: Vec<usize> = (0..keys.len()).collect();
                order.sort_by(|&a, &b| keys[a].partial_cmp(&keys[b]).unwrap());
                let mut z = vec![0i64; keys.len()];
                for (pos, &j) in order.iter().enumerate() {
                    z[j] = (pos % k) as i64 + 1;
                }
                z
            } else {
                (0..nb).map(|j| (j % k) as i64 + 1).collect()
            }
        }
        _ => {
            let ids = ids.expect("checkerboard ids");
            used.iter().map(|&u| ids[u]).collect()
        }
    };
    if best.is_empty() {
        return Err("no assignment of the blocks keeps every fold".into());
    }
    let mut folds = vec![0i64; n];
    for (t, &i) in rec.iter().enumerate() {
        folds[i] = best[blk[t]];
    }
    Ok(folds)
}

/// R's scale of every layer as terra computes it (the mean and root mean square of the centred values,
/// sequential sums).
fn terra_scale(layers: &[&[f64]]) -> Vec<Vec<f64>> {
    layers
        .iter()
        .map(|v| {
            let ok: Vec<f64> = v.iter().copied().filter(|x| !x.is_nan()).collect();
            let mean = ok.iter().fold(0.0, |s, x| s + x) / ok.len() as f64;
            let c: Vec<f64> = v.iter().map(|x| x - mean).collect();
            let cok: Vec<f64> = c.iter().copied().filter(|x| !x.is_nan()).collect();
            let rms = (cok.iter().fold(0.0, |s, x| s + x * x) / (cok.len() as f64 - 1.0)).sqrt();
            c.iter().map(|x| x / rms).collect()
        })
        .collect()
}

/// blockCV's cv_cluster: k clusters (R's kmeans, 500 iterations, 25 starts) of the coordinates, or of the
/// predictors at the records (scaled or not).
pub fn spatial_clusters(
    r: &Records,
    k: usize,
    env: Option<(&Grid, &[&[f64]])>,
    scale: bool,
    algorithm: &str,
    seed: i32,
) -> Result<Vec<i64>, String> {
    let alg = match algorithm {
        "Hartigan-Wong" => 1,
        "Lloyd" | "Forgy" => 2,
        "MacQueen" => 3,
        a => return Err(format!("unknown algorithm {a:?}")),
    };
    let n = r.x.len();
    let (table, p): (Vec<f64>, usize) = match env {
        None => (r.x.iter().zip(r.y).flat_map(|(a, b)| [*a, *b]).collect(), 2),
        Some((grid, layers)) => {
            let values: Vec<Vec<f64>> =
                if scale { terra_scale(layers) } else { layers.iter().map(|l| l.to_vec()).collect() };
            let cells = super::points::grid_cells(r.x, r.y, grid);
            if cells.iter().any(|&c| c < 0) {
                return Err("some records are outside the predictors".into());
            }
            let t: Vec<f64> = cells.iter().flat_map(|&c| values.iter().map(move |v| v[c as usize])).collect();
            if t.iter().any(|x| x.is_nan()) {
                return Err("the predictors have no value at some records".into());
            }
            (t, values.len())
        }
    };
    let mut rng = ffi::rrng_new(seed);
    let cl = ffi::sdm_kmeans(rng.pin_mut(), &table, n as i64, p as i64, k as i32, 500, 25, alg)
        .map_err(|e| e.to_string())?;
    Ok(cl.iter().map(|&c| c as i64).collect())
}

/// The records to train on and to test on.
pub type TrainTest = (Vec<usize>, Vec<usize>);

/// The folds of buffer_folds (blockCV's cv_buffer): for every presence (or record), the records farther than
/// `size` to train on and the record (with the background within `size`) to test on.
pub fn buffer_folds(
    r: &Records,
    size: f64,
    presence_background: bool,
    add_background: bool,
) -> Result<Vec<TrainTest>, String> {
    let classes = r.classes.ok_or("buffer folds need presences and background")?;
    let n = r.x.len();
    let targets: Vec<usize> =
        if presence_background { (0..n).filter(|&i| classes[i] == 1.0).collect() } else { (0..n).collect() };
    let mut out = Vec::new();
    for &i in &targets {
        let d: Vec<f64> = if r.lonlat {
            ffi::cost_geodesic(&vec![r.x[i]; n], &vec![r.y[i]; n], r.x, r.y).map_err(|e| e.to_string())?
        } else {
            (0..n)
                .map(|j| ((r.x[j] - r.x[i]) * (r.x[j] - r.x[i]) + (r.y[j] - r.y[i]) * (r.y[j] - r.y[i])).sqrt())
                .collect()
        };
        let train: Vec<usize> = (0..n).filter(|&j| d[j] > size).collect();
        let test: Vec<usize> = if presence_background && add_background {
            (0..n).filter(|&j| d[j] <= size && classes[j] == 0.0).chain(std::iter::once(i)).collect()
        } else {
            vec![i]
        };
        out.push((train, test));
    }
    Ok(out)
}
