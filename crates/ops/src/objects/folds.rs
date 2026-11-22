//! `objects.folds`: the folds of the training objects for a cross-validation that is not optimistic. Neighbouring
//! objects are nearly alike, so a random split puts a twin of every test object in the training (Ploton et al. 2020;
//! Meyer and Pebesma 2022): the folds are made of blocks of the map, of clusters of the centroids, or of groups of
//! neighbouring objects that are never split.

use std::collections::VecDeque;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};

use super::extrema::centroids;
use super::{Objects, objects_in};
use crate::sdm::folds::{self, Blocks, Records};
use crate::signatures::NumpyRng;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Folds));
}

/// The groups of marked objects (their positions among the rows of the samples) that touch: two objects within
/// `steps` steps across the neighbours of the objects (through any object) are one group.
fn neighbour_groups(o: &Objects, marked: &[usize], steps: usize) -> Vec<Vec<usize>> {
    let mut at = vec![usize::MAX; o.count];
    for (p, &i) in marked.iter().enumerate() {
        at[i] = p;
    }
    let mut parent: Vec<usize> = (0..marked.len()).collect();
    fn root(parent: &mut [usize], mut a: usize) -> usize {
        while parent[a] != a {
            parent[a] = parent[parent[a]];
            a = parent[a];
        }
        a
    }
    // a breadth-first search to `steps` from each marked object; `seen` is stamped with the object that reached it
    let mut seen = vec![usize::MAX; o.count];
    for (p, &i) in marked.iter().enumerate() {
        let mut queue = VecDeque::from([(i, 0usize)]);
        seen[i] = p;
        while let Some((cur, depth)) = queue.pop_front() {
            if at[cur] != usize::MAX {
                let (a, b) = (root(&mut parent, p), root(&mut parent, at[cur]));
                if a != b {
                    parent[a.max(b)] = a.min(b);
                }
            }
            if depth == steps {
                continue;
            }
            for &(nb, _) in o.neighbours_of(cur as u32 + 1) {
                let n = nb as usize - 1;
                if seen[n] != p {
                    seen[n] = p;
                    queue.push_back((n, depth + 1));
                }
            }
        }
    }
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut index = vec![usize::MAX; marked.len()];
    for p in 0..marked.len() {
        let r = root(&mut parent, p);
        if index[r] == usize::MAX {
            index[r] = groups.len();
            groups.push(Vec::new());
        }
        groups[index[r]].push(p);
    }
    groups
}

/// The groups spread over `k` folds, the largest first (those of a size in an order drawn from `rng`), each to the
/// fold where its classes keep the classes of the folds the most even.
fn spread(groups: &mut [Vec<usize>], class: &[usize], classes: usize, k: usize, rng: &mut NumpyRng) -> Vec<usize> {
    rng.shuffle(groups);
    groups.sort_by_key(|g| std::cmp::Reverse(g.len()));
    let mut count = vec![vec![0usize; classes]; k];
    let mut total = vec![0usize; k];
    let mut fold = vec![0usize; class.len()];
    for g in groups.iter() {
        let mut here = vec![0usize; classes];
        for &p in g {
            here[class[p]] += 1;
        }
        let cost = |f: usize| -> usize { (0..classes).map(|c| (count[f][c] + here[c]).pow(2)).sum() };
        let best = (0..k).min_by_key(|&f| (cost(f), total[f], f)).unwrap();
        for &p in g {
            fold[p] = best;
            count[best][class[p]] += 1;
        }
        total[best] += g.len();
    }
    fold
}

/// `objects.folds`.
struct Folds;

impl Operator for Folds {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.folds",
            "The folds of the training objects for a spatial cross-validation (ml.fit's `fold`). Neighbouring \
             objects are nearly alike, so folds drawn at random put a twin of every test object in the training and \
             the accuracy comes out too high (Ploton et al. 2020; Meyer and Pebesma 2022). The `method` is \
             neighbours (the default: objects within `steps` steps of each other, through any object, are a group \
             that is never split; the groups are spread over the `k` folds, the largest first, each to the fold \
             that keeps the classes most even), blocks (squares of `size` map units over the objects' centroids \
             given to the folds at random, blockCV's cv_spatial and Roberts et al. 2017), clusters (k-means of the \
             centroids, blockCV's cv_cluster) or random (stratified by class: not spatial, to compare). `objects` and \
             `samples` are objects.samples' objects and table: the table's rows are the marked objects, in order of \
             id. The same `seed` gives the same folds on any number of threads.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the objects marked by objects.samples (its `objects`)"))
        .input(PortSpec::new("samples", Type::Table, "the table of objects.samples: class and the attributes"))
        .output(PortSpec::new("table", Type::Table, "the samples' table with the column fold (1 to k) added"))
        .output(PortSpec::new(
            "counts",
            Type::Table,
            "per fold: the objects and the cells, and the objects of each class",
        ))
        .param(PortSpec::optional("method", Type::Text, "neighbours (default), blocks, clusters or random").choices([
            "neighbours",
            "blocks",
            "clusters",
            "random",
        ]))
        .param(PortSpec::optional("k", Type::Integer, "the folds (default 5)"))
        .param(PortSpec::optional(
            "steps",
            Type::Integer,
            "with neighbours: the steps across the objects' neighbours within which two sample objects are a group \
             (default 1: touching)",
        ))
        .param(PortSpec::optional(
            "size",
            Type::Number,
            "with blocks: the side of a block in map units (default a quarter of the longer side of the samples' \
             extent)",
        ))
        .param(PortSpec::optional("class", Type::Text, "the samples' column of the class (default class)"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of the draws (default 1)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let bad = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let o = objects_in(args, "objects")?;
        let t = match args.get("samples") {
            Some(Value::Table(t)) => t,
            Some(v) => return Err(bad(format!("samples: a table, not a {:?}", v.ty()))),
            None => return Err(bad("connect the samples (objects.samples' table)".into())),
        };
        let attr: String = args.param("class", "class".to_string())?;
        let class =
            t.column(&attr).ok_or_else(|| bad(format!("the samples have no column {attr:?} (setting class)")))?;
        let marked: Vec<usize> = (0..o.count).filter(|&i| o.class_of[i] != 0).collect();
        if marked.len() != t.rows.len() {
            return Err(bad(format!(
                "the samples have {} rows and the objects {} marked: connect objects.samples' `objects` and `table`",
                t.rows.len(),
                marked.len()
            )));
        }
        let method: String = args.param("method", "neighbours".to_string())?;
        let k: usize = args.param("k", 5)?;
        let seed: u64 = args.param("seed", 1)?;
        let n = marked.len();
        if k < 2 || k > n {
            return Err(bad(format!("k {k}: 2 to the {n} sample objects")));
        }
        // the classes by code, 0..
        let mut codes: Vec<f64> = class.clone();
        codes.sort_by(f64::total_cmp);
        codes.dedup();
        let cls: Vec<usize> = class.iter().map(|c| codes.partition_point(|x| x < c)).collect();
        let mut rng = NumpyRng::new(seed);
        let fold: Vec<usize> = match method.as_str() {
            "random" => {
                let mut fold = vec![0usize; n];
                let mut counter = 0;
                for c in 0..codes.len() {
                    let mut rows: Vec<usize> = (0..n).filter(|&r| cls[r] == c).collect();
                    rng.shuffle(&mut rows);
                    for r in rows {
                        fold[r] = counter % k;
                        counter += 1;
                    }
                }
                fold
            }
            "neighbours" => {
                let steps: usize = args.param("steps", 1)?;
                if steps == 0 {
                    return Err(bad("steps: 1 or more".into()));
                }
                let mut groups = neighbour_groups(o, &marked, steps);
                if groups.len() < k {
                    return Err(bad(format!(
                        "the {n} sample objects make only {} groups of neighbours: lower k or steps, or use blocks \
                         or random",
                        groups.len()
                    )));
                }
                spread(&mut groups, &cls, codes.len(), k, &mut rng)
            }
            "blocks" | "clusters" => {
                let mid = centroids(o);
                let (x, y): (Vec<f64>, Vec<f64>) = marked.iter().map(|&i| mid[i]).unzip();
                let r = Records { x: &x, y: &y, lonlat: false, classes: None };
                let folds = if method == "blocks" {
                    let extent = |v: &[f64]| {
                        v.iter().copied().fold(f64::NEG_INFINITY, f64::max)
                            - v.iter().copied().fold(f64::INFINITY, f64::min)
                    };
                    let size = args
                        .opt_param::<f64>("size")?
                        .unwrap_or_else(|| (extent(&x).max(extent(&y)) / 4.0).max(f64::MIN_POSITIVE));
                    if size.is_nan() || size <= 0.0 {
                        return Err(bad(format!("size {size}: a positive number of map units")));
                    }
                    let b = Blocks {
                        size: Some(size),
                        rows_cols: (10, 10),
                        k,
                        hexagon: false,
                        flat_top: false,
                        selection: "random",
                        iteration: 100,
                        offset: (0.0, 0.0),
                        extend: 0.0,
                        deg_to_metre: 111_325.0,
                        seed: seed as i32,
                    };
                    folds::spatial_blocks(&r, &b, &mut || !ctx.stopped())
                } else {
                    folds::spatial_clusters(&r, k, None, false, "Hartigan-Wong", seed as i32)
                }
                .map_err(|e| bad(format!("{method}: {e}")))?;
                if folds.iter().any(|&f| f < 1 || f as usize > k) {
                    return Err(bad(format!("{method}: some sample objects fell in no fold: change size or k")));
                }
                folds.iter().map(|&f| f as usize - 1).collect()
            }
            m => return Err(bad(format!("method {m:?}: neighbours, blocks, clusters or random"))),
        };
        // the table, and what each fold holds
        let mut out = t.clone();
        if out.columns.iter().any(|c| c == "fold") {
            return Err(bad("the samples already have a column fold".into()));
        }
        out.columns.push("fold".into());
        for (row, &f) in out.rows.iter_mut().zip(&fold) {
            row.push((f + 1) as f64);
        }
        let name = |c: f64| t.label(&attr, c).map_or_else(|| crate::value::class_label(c), str::to_string);
        let mut head = vec!["fold".to_string(), "objects".into(), "cells".into()];
        head.extend(codes.iter().map(|&c| name(c)));
        let mut counts = Table::new(head);
        for f in 0..k {
            let rows: Vec<usize> = (0..n).filter(|&r| fold[r] == f).collect();
            let mut row =
                vec![(f + 1) as f64, rows.len() as f64, rows.iter().map(|&r| o.cells[marked[r]] as f64).sum()];
            row.extend((0..codes.len()).map(|c| rows.iter().filter(|&&r| cls[r] == c).count() as f64));
            counts.rows.push(row);
        }
        for (c, &code) in codes.iter().enumerate() {
            let holding = counts.rows.iter().filter(|r| r[3 + c] > 0.0).count();
            if holding < k {
                ctx.warn(format!(
                    "node {node:?}: {} of the {k} folds have no object of class {:?}: its test and training folds \
                     will lack it",
                    k - holding,
                    name(code)
                ));
            }
        }
        Ok(outputs([("table", Value::Table(out)), ("counts", Value::Table(counts))]))
    }
}
