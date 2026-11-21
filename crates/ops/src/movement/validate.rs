//! `move.validate`, the library's `tf.move.validate`: the use of the animals (a utilisation distribution, points, or
//! corridor segments) against a connectivity surface, after Zeller et al. (2018). Not in R: the definitions are the
//! library's and are the same here, number for number (sums run in order, as the library's do).

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::Grid;
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::TRACK_NAMES;
use super::corridor::{gfmt, quantile7, uds_of};
use super::fail;
use crate::{Args, Ctx, Operator, Outputs, Registry, Result, Table, Value};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Validate));
}

/// Whether a number is above 0 (not for NaN).
fn positive(x: f64) -> bool {
    x > 0.0
}

/// The sum in order (the first value is the start, as `cumsum` does).
fn sum(a: &[f64]) -> f64 {
    let mut it = a.iter();
    match it.next() {
        None => 0.0,
        Some(&f) => it.fold(f, |s, &x| s + x),
    }
}

/// Average ranks, 1-based: ties share the mean of their ranks.
fn ranks(x: &[f64]) -> Vec<f64> {
    let n = x.len();
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| x[a].partial_cmp(&x[b]).unwrap());
    let mut out = vec![0.0; n];
    let mut start = 0;
    while start < n {
        let mut end = start + 1;
        while end < n && x[order[end]] == x[order[start]] {
            end += 1;
        }
        let avg = (start + end + 1) as f64 / 2.0;
        for &i in &order[start..end] {
            out[i] = avg;
        }
        start = end;
    }
    out
}

/// Spearman's rank correlation: Pearson's of the average ranks; NaN for fewer than three pairs or a constant variable.
fn spearman(x: &[f64], y: &[f64]) -> f64 {
    let n = x.len();
    if n < 3 {
        return f64::NAN;
    }
    let (rx, ry) = (ranks(x), ranks(y));
    let (mx, my) = (sum(&rx) / n as f64, sum(&ry) / n as f64);
    let dx: Vec<f64> = rx.iter().map(|v| v - mx).collect();
    let dy: Vec<f64> = ry.iter().map(|v| v - my).collect();
    let sxx = sum(&dx.iter().map(|v| v * v).collect::<Vec<_>>());
    let syy = sum(&dy.iter().map(|v| v * v).collect::<Vec<_>>());
    if sxx == 0.0 || syy == 0.0 {
        return f64::NAN;
    }
    let sxy = sum(&dx.iter().zip(&dy).map(|(a, b)| a * b).collect::<Vec<_>>());
    (sxy / (sxx * syy).sqrt()).clamp(-1.0, 1.0)
}

/// A north-up raster's cells: the corner and the cell size.
struct Cells {
    nx: usize,
    ny: usize,
    xmin: f64,
    ymax: f64,
    xres: f64,
    yres: f64,
}

impl Cells {
    fn of(g: &Grid, what: &str, node: &str) -> Result<Cells> {
        let t = g.transform;
        if t[2] != 0.0 || t[4] != 0.0 || !positive(-t[5]) || !positive(t[1]) {
            return Err(fail(node, format!("{what}: a north-up raster without rotation")));
        }
        if g.width < 2 || g.height < 2 {
            return Err(fail(node, format!("{what}: at least two rows and two columns")));
        }
        Ok(Cells { nx: g.width, ny: g.height, xmin: t[0], ymax: t[3], xres: t[1], yres: -t[5] })
    }

    fn same_as(&self, o: &Cells) -> bool {
        let tol = 1e-9;
        self.nx == o.nx
            && self.ny == o.ny
            && (self.xmin - o.xmin).abs() <= tol * self.xres
            && (self.ymax - o.ymax).abs() <= tol * self.yres
            && (self.xres - o.xres).abs() <= tol * self.xres
            && (self.yres - o.yres).abs() <= tol * self.yres
    }

    /// The (row, column) of the cell holding a point (an edge belongs to the cell after it), if inside.
    fn cell(&self, x: f64, y: f64) -> Option<(usize, usize)> {
        let c = ((x - self.xmin) / self.xres).floor();
        let r = ((self.ymax - y) / self.yres).floor();
        (c.is_finite() && r.is_finite() && c >= 0.0 && c < self.nx as f64 && r >= 0.0 && r < self.ny as f64)
            .then_some((r as usize, c as usize))
    }
}

/// Coordinates from one CRS to another (unchanged where either is missing or they agree).
fn project(x: &mut [f64], y: &mut [f64], src: Option<&str>, dst: Option<&str>, node: &str) -> Result<()> {
    if let (Some(s), Some(d)) = (src, dst)
        && s != d
    {
        terraflux_gdal::Transform::new(s, d).map_err(|e| fail(node, e))?.points(x, y);
    }
    Ok(())
}

/// The measures of one use: `u` the use of each analysis cell, `s` the surface there, `corr` the corridor flags,
/// `cells` the positions the correlation runs over, in order.
fn measures(u: &[f64], s: &[f64], corr: &[bool], cells: &[usize], coverage: f64, sign: f64) -> [f64; 8] {
    let total = sum(u);
    if !positive(total) {
        return [0.0, f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN, f64::NAN, coverage];
    }
    let n = s.len() as f64;
    let kept: Vec<f64> = u.iter().zip(corr).map(|(&v, &c)| if c { v } else { 0.0 }).collect();
    let use_frac = sum(&kept) / total;
    let area_frac = corr.iter().filter(|&&c| c).count() as f64 / n;
    let ratio = if area_frac > 0.0 { use_frac / area_frac } else { f64::NAN };
    let (xu, xs): (Vec<f64>, Vec<f64>) = cells.iter().map(|&i| (u[i], sign * s[i])).unzip();
    let us: Vec<f64> = u.iter().zip(s).map(|(a, b)| a * b).collect();
    [cells.len() as f64, spearman(&xu, &xs), use_frac, area_frac, ratio, sum(&us) / total, sum(s) / n, coverage]
}

/// The positions of the cells inside the `support` volume contour of the use, the most used first.
fn support_cells(u: &[f64], support: f64) -> Vec<usize> {
    let mut order: Vec<usize> = (0..u.len()).filter(|&i| u[i] > 0.0).collect();
    if order.is_empty() {
        return order;
    }
    order.sort_by(|&a, &b| u[b].partial_cmp(&u[a]).unwrap());
    if support >= 1.0 {
        return order;
    }
    let mut cum = Vec::with_capacity(order.len());
    let mut acc = 0.0;
    for (j, &i) in order.iter().enumerate() {
        acc = if j == 0 { u[i] } else { acc + u[i] };
        cum.push(acc);
    }
    let target = support * cum[cum.len() - 1];
    let k = cum.partition_point(|&c| c < target) + 1;
    order.truncate(k.min(cum.len()));
    order
}

fn pct(f: f64) -> String {
    format!("{}%", gfmt(100.0 * f, 3))
}

/// The pooled row (or the only one) in words.
fn note(label: &str, r: &[f64; 8]) -> String {
    let (use_f, area, ratio, rho, coverage) = (r[2], r[3], r[4], r[1], r[7]);
    if !use_f.is_finite() {
        return format!("{label}: no use fell on cells with data, so nothing can be compared.");
    }
    let head = if !ratio.is_finite() {
        format!("{label}: the corridors cover none of the compared area, so the use ratio is undefined.")
    } else {
        let verdict = if ratio >= 1.5 {
            "so the animals favour the corridors."
        } else if ratio <= 1.0 / 1.5 {
            "so the animals avoid the corridors."
        } else {
            "about what chance gives."
        };
        format!(
            "{label}: {} of the use lies in the corridors, {} of the area: {} times its share, {verdict}",
            pct(use_f),
            pct(area),
            gfmt(ratio, 3)
        )
    };
    let mut tail = if rho.is_nan() {
        " The rank correlation cannot be computed (too few cells or a constant variable).".to_string()
    } else {
        let a = rho.abs();
        let word = if a < 0.1 {
            "no"
        } else if a < 0.3 {
            "a weak"
        } else if a < 0.5 {
            "a moderate"
        } else {
            "a strong"
        };
        if word == "no" {
            format!(" Spearman's rho is {}: no rank relation between the use and the surface.", gfmt(rho, 3))
        } else {
            let sense = if rho > 0.0 {
                "the use is higher where the surface is better"
            } else {
                "the use is higher where the surface is worse"
            };
            format!(" Spearman's rho is {}: {word} rank relation ({sense}).", gfmt(rho, 3))
        }
    };
    if coverage < 0.9 {
        tail.push_str(&format!(" Only {} of the use falls on cells with data.", pct(coverage)));
    }
    head + &tail
}

/// The text of a track column's value.
fn label_of(c: &Column, i: usize) -> String {
    match c {
        Column::Texts(t) => t[i].clone(),
        Column::Numbers(x) => crate::value::class_label(x[i]),
    }
}

/// `move.validate`.
struct Validate;

impl Operator for Validate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "move.validate",
            "Does the use of the animals follow a connectivity surface? Zeller et al.'s (2018) validation with \
             movement data, in numbers (not in R: the definitions are the library's tf.move.validate; of the paper \
             only the abstract was available). The surface gives each cell a score, higher where movement is better: \
             the surface itself for kind current (a current, a cost-distance kernel) and binary (a least-cost \
             corridor: the cells that are not 0), minus the surface for kind resistance. The corridors are the cells \
             with a score at or above a cut: threshold (a surface value: from it up for a current, up to it for a \
             resistance) or else the quantile of the scores over the whole surface (R's type 7; 0.9 makes the best \
             10 % of the cells the corridors); a binary surface takes neither. The use is a UD (a raster, or a \
             raster with a band per track), read on the surface's grid (the same grid cell by cell, otherwise the \
             surface at each UD cell centre, nearest cell, the centres reprojected when the CRSs differ; each band \
             cut to the cells where the surface has a value and renormalised to 1, coverage the part of its mass that \
             was on such cells; the compared area is those cells), or points (each counts 1 in the cell that holds \
             it; the compared area is the cells with a value inside the bounding box of the kept points' cells, track \
             by track), or lines (move.corridor's lines: each segment is cut into ceil(length / spacing) equal \
             pieces, spacing half the surface's smaller cell side, and each piece counts its length at its midpoint: \
             the use is the length of line in each cell). The table has a row for each track and then pooled (the \
             mean of the tracks' normalised UDs, or all the points or lines together): rho, Spearman's rank \
             correlation between the use and the score (average ranks for ties; positive always means the animals use \
             better cells more, also for a resistance; for a UD over the cells inside its support volume contour, the \
             UD's probability being the use; for points and lines over all compared cells, the use being the number \
             of points or the length of line, most cells 0: a tie; no p-value, cells next to each other are not \
             independent); use_fraction, the share of the use inside the corridors; area_fraction, the share of the \
             compared cells that are corridors; use_ratio, their quotient (used against available: above 1 the \
             corridors catch more of the use than their area, 1 is chance); surface_used and surface_available, the \
             mean surface value weighted by the use and over the compared cells; n_cells, the cells the correlation \
             ran over; and coverage. The note reads the pooled row in words, threshold is the surface value the \
             corridors start at, and corridor is the 1 / 0 raster of the corridors.",
            Access::None,
        )
        .input(PortSpec::new(
            "use",
            Type::Any,
            "the use: a UD raster (a band for each track), the tracks' points, or lines such as move.corridor's",
        ))
        .input(PortSpec::new(
            "surface",
            Type::Raster,
            "the connectivity surface: a current, a resistance or a corridor",
        ))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row for each track and pooled: track, n_cells, rho, use_fraction, area_fraction, use_ratio, \
             surface_used, surface_available, coverage",
        ))
        .output(PortSpec::new("note", Type::Text, "the pooled row in words"))
        .output(PortSpec::new("threshold", Type::Number, "the surface value the corridors start at (none for binary)"))
        .output(PortSpec::new(
            "corridor",
            Type::Raster,
            "the corridor cells as 1, the others 0, no data where the surface has none",
        ))
        .param(PortSpec::optional("kind", Type::Text, "current (default), resistance or binary").choices([
            "current",
            "resistance",
            "binary",
        ]))
        .param(PortSpec::optional(
            "threshold",
            Type::Number,
            "the surface value that makes a corridor: from it up (current), up to it (resistance)",
        ))
        .param(PortSpec::optional(
            "quantile",
            Type::Number,
            "in (0, 1), without a threshold: the corridors are the cells at or above this quantile of the scores (0.9)",
        ))
        .param(PortSpec::optional(
            "support",
            Type::Number,
            "in (0, 1]: a UD's correlation runs over the cells inside this volume contour (0.99); 1 takes all with \
             probability above 0",
        ))
        .param(PortSpec::optional("track", Type::Text, "the column of the track of points and lines"))
        .param(PortSpec::optional(
            "spacing",
            Type::Number,
            "the longest piece a line is cut into, in the surface's units (half its smaller cell side)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let kind: String = args.param("kind", "current".to_string())?;
        if !["current", "resistance", "binary"].contains(&kind.as_str()) {
            return Err(fail(node, "kind: \"current\", \"resistance\" or \"binary\""));
        }
        let threshold: Option<f64> = args.opt_param("threshold")?;
        if kind == "binary" && threshold.is_some() {
            return Err(fail(node, "kind \"binary\": the surface is its own corridor, a threshold has no meaning"));
        }
        if threshold.is_some_and(|t| !t.is_finite()) {
            return Err(fail(node, "threshold: a finite number or none"));
        }
        let quantile: f64 = args.param("quantile", 0.9)?;
        if kind != "binary" && threshold.is_none() && !(quantile > 0.0 && quantile < 1.0) {
            return Err(fail(node, "quantile: a number in (0, 1)"));
        }
        let support: f64 = args.param("support", 0.99)?;
        if !(support > 0.0 && support <= 1.0) {
            return Err(fail(node, "support: a number in (0, 1]"));
        }
        let spacing: Option<f64> = args.opt_param("spacing")?;
        if spacing.is_some_and(|s| !(s.is_finite() && s > 0.0)) {
            return Err(fail(node, "spacing: a positive number or none"));
        }

        let surface = crate::files::as_port(
            args.get("surface").ok_or_else(|| fail(node, "no input \"surface\""))?.clone(),
            Type::Any,
        )?;
        let Value::Map(smap) = &surface else {
            return Err(fail(node, format!("surface: a raster, not a {}", surface.ty().name())));
        };
        let grid = smap.grid.clone();
        let sc = Cells::of(&grid, "surface", node)?;
        let scrs = crate::prepare::crs_of(&grid);
        let s_all: Vec<f64> = smap.data.iter().map(|&v| if v.is_finite() { v } else { f64::NAN }).collect();
        let finite: Vec<bool> = s_all.iter().map(|v| v.is_finite()).collect();
        if !finite.iter().any(|&f| f) {
            return Err(fail(node, "surface: no cell has a value"));
        }
        let sign = if kind == "resistance" { -1.0 } else { 1.0 };
        let score: Vec<f64> = s_all.iter().map(|&v| sign * v).collect();
        let (corr_all, cut): (Vec<bool>, f64) = if kind == "binary" {
            let c: Vec<bool> = s_all.iter().zip(&finite).map(|(&v, &f)| f && v != 0.0).collect();
            if !c.iter().any(|&f| f) {
                return Err(fail(node, "surface: a binary surface with no corridor cell"));
            }
            (c, f64::NAN)
        } else {
            let cut_score = match threshold {
                Some(t) => sign * t,
                None => {
                    let valid: Vec<f64> = score.iter().zip(&finite).filter(|(_, f)| **f).map(|(&v, _)| v).collect();
                    quantile7(&valid, quantile)
                }
            };
            let c: Vec<bool> = score.iter().zip(&finite).map(|(&v, &f)| f && v >= cut_score).collect();
            (c, sign * cut_score)
        };

        let use_value = args.get("use").ok_or_else(|| fail(node, "no input \"use\""))?;
        let mut rows: Vec<(String, [f64; 8])> = Vec::new();
        if let Value::Vector(v) = use_value {
            let (names, samples) = samples(v, args.opt_param("track")?, spacing, &sc, scrs.as_deref(), node)?;
            let mut stacks = Vec::with_capacity(names.len());
            for (x, y, w) in &samples {
                let (mut r, mut c, mut kw) = (Vec::new(), Vec::new(), Vec::new());
                for i in 0..x.len() {
                    if let Some((ri, ci)) = sc.cell(x[i], y[i])
                        && finite[ri * sc.nx + ci]
                    {
                        r.push(ri);
                        c.push(ci);
                        kw.push(w[i]);
                    }
                }
                let kept = sum(&kw);
                stacks.push((r, c, kw, kept, sum(w)));
            }
            let on_cells = |r: &[usize], c: &[usize], w: &[f64], kept: f64, whole: f64| -> [f64; 8] {
                let coverage = if whole > 0.0 { kept / whole } else { f64::NAN };
                if r.is_empty() || !positive(kept) {
                    return measures(&[0.0], &[0.0], &[false], &[], coverage, sign);
                }
                let (r0, r1) = (*r.iter().min().unwrap(), *r.iter().max().unwrap());
                let (c0, c1) = (*c.iter().min().unwrap(), *c.iter().max().unwrap());
                let (h, wd) = (r1 - r0 + 1, c1 - c0 + 1);
                let mut use_ = vec![0.0; h * wd];
                for i in 0..r.len() {
                    use_[(r[i] - r0) * wd + (c[i] - c0)] += w[i];
                }
                let (mut u, mut s, mut cr) = (Vec::new(), Vec::new(), Vec::new());
                for i in 0..h {
                    for j in 0..wd {
                        let k = (r0 + i) * sc.nx + c0 + j;
                        if finite[k] {
                            u.push(use_[i * wd + j]);
                            s.push(s_all[k]);
                            cr.push(corr_all[k]);
                        }
                    }
                }
                let cells: Vec<usize> = (0..u.len()).collect();
                measures(&u, &s, &cr, &cells, coverage, sign)
            };
            for (nm, (r, c, w, kept, whole)) in names.iter().zip(&stacks) {
                rows.push((nm.clone(), on_cells(r, c, w, *kept, *whole)));
            }
            if names.len() > 1 {
                let r: Vec<usize> = stacks.iter().flat_map(|a| a.0.iter().copied()).collect();
                let c: Vec<usize> = stacks.iter().flat_map(|a| a.1.iter().copied()).collect();
                let w: Vec<f64> = stacks.iter().flat_map(|a| a.2.iter().copied()).collect();
                let kept = sum(&stacks.iter().map(|a| a.3).collect::<Vec<_>>());
                let whole = sum(&stacks.iter().map(|a| a.4).collect::<Vec<_>>());
                rows.push(("pooled".to_string(), on_cells(&r, &c, &w, kept, whole)));
            }
        } else {
            let (ugrid, layers) = uds_of(node, "use", use_value, 0)
                .map_err(|_| fail(node, format!("use: a UD (a raster) or vectors, not a {}", use_value.ty().name())))?;
            let uc = Cells::of(&ugrid, "use", node)?;
            let ucrs = crate::prepare::crs_of(&ugrid);
            let crs_differ = matches!((&ucrs, &scrs), (Some(a), Some(b)) if a != b);
            let same = uc.same_as(&sc) && !crs_differ;
            // the surface at each UD cell: the cell it is in, and whether it is inside
            let (s, corr, ok): (Vec<f64>, Vec<bool>, Vec<bool>) = if same {
                (s_all.clone(), corr_all.clone(), finite.clone())
            } else {
                let (mut px, mut py) = (Vec::with_capacity(uc.nx * uc.ny), Vec::with_capacity(uc.nx * uc.ny));
                for i in 0..uc.ny {
                    for j in 0..uc.nx {
                        px.push(uc.xmin + (j as f64 + 0.5) * uc.xres);
                        py.push(uc.ymax - (i as f64 + 0.5) * uc.yres);
                    }
                }
                project(&mut px, &mut py, ucrs.as_deref(), scrs.as_deref(), node)?;
                let n = px.len();
                let (mut s, mut corr, mut ok) = (vec![f64::NAN; n], vec![false; n], vec![false; n]);
                for k in 0..n {
                    if let Some((r, c)) = sc.cell(px[k], py[k]) {
                        let at = r * sc.nx + c;
                        s[k] = s_all[at];
                        corr[k] = corr_all[at];
                        ok[k] = finite[at];
                    }
                }
                (s, corr, ok)
            };
            let pos: Vec<usize> = (0..ok.len()).filter(|&k| ok[k]).collect();
            let s_a: Vec<f64> = pos.iter().map(|&k| s[k]).collect();
            let corr_a: Vec<bool> = pos.iter().map(|&k| corr[k]).collect();
            let mut norm: Vec<(String, Vec<f64>, f64)> = Vec::new();
            for (name, values) in &layers {
                if values.iter().any(|&v| v.is_finite() && v < 0.0) {
                    return Err(fail(node, format!("{name}: a UD has no negative values")));
                }
                let u_all: Vec<f64> = values.iter().map(|&v| if v.is_finite() { v } else { 0.0 }).collect();
                let total_all = sum(&u_all);
                let u: Vec<f64> = pos.iter().map(|&k| u_all[k]).collect();
                let tot = sum(&u);
                if !positive(tot) {
                    return Err(fail(
                        node,
                        format!("{name}: the UD has no mass on cells where the surface has a value"),
                    ));
                }
                let cov = if total_all > 0.0 { tot / total_all } else { f64::NAN };
                norm.push((name.clone(), u.iter().map(|v| v / tot).collect(), cov));
            }
            for (name, u, cov) in &norm {
                rows.push((name.clone(), measures(u, &s_a, &corr_a, &support_cells(u, support), *cov, sign)));
            }
            if norm.len() > 1 {
                let mut pooled = vec![0.0; pos.len()];
                for (_, u, _) in &norm {
                    for (p, v) in pooled.iter_mut().zip(u) {
                        *p += v;
                    }
                }
                let nb = norm.len() as f64;
                let pooled: Vec<f64> = pooled.iter().map(|v| v / nb).collect();
                let cov = sum(&norm.iter().map(|a| a.2).collect::<Vec<_>>()) / nb;
                rows.push((
                    "pooled".to_string(),
                    measures(&pooled, &s_a, &corr_a, &support_cells(&pooled, support), cov, sign),
                ));
            }
        }

        let mut table = Table::new(
            [
                "track",
                "n_cells",
                "rho",
                "use_fraction",
                "area_fraction",
                "use_ratio",
                "surface_used",
                "surface_available",
                "coverage",
            ]
            .map(String::from)
            .to_vec(),
        );
        table.labels.insert("track".into(), rows.iter().map(|r| r.0.clone()).collect());
        for (i, (nm, m)) in rows.iter().enumerate() {
            let mut row = vec![(i + 1) as f64];
            row.extend_from_slice(m);
            table.rows.push(row);
            if !positive(m[7]) && nm != "pooled" {
                ctx.warn(format!("node {node:?}: {nm}: none of the use falls on cells with data"));
            }
        }
        let last = rows.last().unwrap();
        let text = note(&last.0, &last.1);
        let flag: Vec<f64> =
            corr_all.iter().zip(&finite).map(|(&c, &f)| if f { f64::from(u8::from(c)) } else { f64::NAN }).collect();
        let mut o = crate::outputs([
            ("table", Value::Table(table)),
            ("note", Value::Text(text)),
            ("threshold", Value::Number(cut)),
        ]);
        o.insert("corridor".into(), Value::Map(crate::Map::new(grid, flag)));
        Ok(o)
    }
}

/// The points the use stands for, by track (in order of appearance): their coordinates in the surface's CRS and weights.
/// A point weighs 1; a line is cut into pieces no longer than `spacing` and each piece's midpoint weighs its length.
#[allow(clippy::type_complexity)]
fn samples(
    v: &Vector,
    track: Option<String>,
    spacing: Option<f64>,
    sc: &Cells,
    crs: Option<&str>,
    node: &str,
) -> Result<(Vec<String>, Vec<(Vec<f64>, Vec<f64>, Vec<f64>)>)> {
    if v.geometries.is_empty() {
        return Err(fail(node, "use: no locations"));
    }
    if v.geometries.iter().any(|g| matches!(g, Geometry::Empty)) {
        return Err(fail(node, "use: some geometries are empty: drop them first"));
    }
    let n_points = v.geometries.iter().filter(|g| matches!(g, Geometry::Point(_))).count();
    let n_lines = v.geometries.iter().filter(|g| matches!(g, Geometry::LineString(_))).count();
    if n_points + n_lines != v.geometries.len() {
        return Err(fail(node, "use: points (the tracks), or lines (move.corridor's lines)"));
    }
    if n_points > 0 && n_lines > 0 {
        return Err(fail(node, "use: points or lines, not both"));
    }
    let col = match track.filter(|t| !t.is_empty()) {
        Some(t) if v.column(&t).is_some() => Some(t),
        Some(t) => return Err(fail(node, format!("track column {t:?} not found"))),
        None => TRACK_NAMES.iter().find(|n| v.column(n).is_some()).map(|n| n.to_string()),
    };
    let labels: Vec<String> = match &col {
        Some(c) => {
            let c = v.column(c).unwrap();
            (0..v.len()).map(|i| label_of(c, i)).collect()
        }
        None => vec!["use".to_string(); v.len()],
    };
    let mut names: Vec<String> = Vec::new();
    for l in &labels {
        if !names.contains(l) {
            names.push(l.clone());
        }
    }
    let slot = |l: &str| names.iter().position(|n| n == l).unwrap();
    let mut out: Vec<(Vec<f64>, Vec<f64>, Vec<f64>)> = vec![Default::default(); names.len()];
    let src = v.crs.as_deref();
    if n_lines == 0 {
        let (mut px, mut py): (Vec<f64>, Vec<f64>) = v
            .geometries
            .iter()
            .map(|g| match g {
                Geometry::Point(p) => (p[0], p[1]),
                _ => unreachable!(),
            })
            .unzip();
        project(&mut px, &mut py, src, crs, node)?;
        for (i, l) in labels.iter().enumerate() {
            let o = &mut out[slot(l)];
            o.0.push(px[i]);
            o.1.push(py[i]);
            o.2.push(1.0);
        }
        return Ok((names, out));
    }
    let step = spacing.unwrap_or_else(|| sc.xres.min(sc.yres) / 2.0);
    for (l, g) in labels.iter().zip(&v.geometries) {
        let Geometry::LineString(pts) = g else { unreachable!() };
        let (mut x, mut y): (Vec<f64>, Vec<f64>) = pts.iter().map(|p| (p[0], p[1])).unzip();
        project(&mut x, &mut y, src, crs, node)?;
        let o = &mut out[slot(l)];
        for i in 0..x.len().saturating_sub(1) {
            let (dx, dy) = (x[i + 1] - x[i], y[i + 1] - y[i]);
            let length = (dx * dx + dy * dy).sqrt();
            if !positive(length) {
                continue;
            }
            let n = ((length / step).ceil() as usize).max(1);
            for k in 0..n {
                let t = (k as f64 + 0.5) / n as f64;
                o.0.push(x[i] + t * dx);
                o.1.push(y[i] + t * dy);
                o.2.push(length / n as f64);
            }
        }
    }
    Ok((names, out))
}
