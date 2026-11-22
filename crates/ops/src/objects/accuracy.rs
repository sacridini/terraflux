//! `objects.accuracy` and `objects.sample`: the accuracy of a classification of objects, by objects and by area
//! (Olofsson et al. 2014), and the stratified sample of objects to label for it.

use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::classify::sample_classes;
use super::{Objects, objects_in};
use crate::accuracy::{apportion, olofsson, olofsson_objects, qnorm};
use crate::signatures::NumpyRng;
use crate::{Args, Ctx, Error, Operator, Outputs, Registry, Result, Table, Value, outputs};

/// Olofsson et al.'s advice: at least this many sample units in each class.
const ADVISED: usize = 50;

/// The name of the class of objects that no class took.
const NONE: &str = "unclassified";

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Accuracy));
    r.add(Arc::new(Sample));
}

/// The name of an object's mapped class.
fn mapped_name(o: &Objects, i: usize) -> &str {
    match o.class_of[i] {
        0 => NONE,
        k => o.classes[k as usize - 1].name.as_str(),
    }
}

/// The reference class of each object of a table (columns `id` and `attr`): as [`sample_classes`], the names and a
/// code per object (0 none, left out when the rows disagree).
fn table_classes(ctx: &mut Ctx, node: &str, o: &Objects, t: &Table, attr: &str) -> Result<(Vec<String>, Vec<u32>)> {
    let (Some(ids), Some(j)) = (t.column("id"), t.columns.iter().position(|c| c == attr)) else {
        return Err(Error::msg(format!(
            "node {node:?}: the reference table needs the columns id and {attr:?} (it has {})",
            t.columns.join(", ")
        )));
    };
    let mut said: Vec<Vec<String>> = vec![Vec::new(); o.count];
    let (mut unknown, mut classless) = (0usize, 0usize);
    for (row, &id) in t.rows.iter().zip(&ids) {
        let class = t.cell(j, row[j]);
        if class.is_empty() {
            classless += 1;
        } else if id.fract() != 0.0 || id < 1.0 || id > o.count as f64 {
            unknown += 1;
        } else {
            said[id as usize - 1].push(class);
        }
    }
    if unknown > 0 {
        ctx.warn(format!("objects.accuracy: {unknown} reference row(s) name an id that is no object: ignored"));
    }
    if classless > 0 {
        ctx.warn(format!("objects.accuracy: {classless} reference row(s) without a class ({attr}): ignored"));
    }
    let mut names: Vec<String> = said.iter().flatten().cloned().collect();
    names.sort();
    names.dedup();
    let mut class_of = vec![0u32; o.count];
    let mut conflict = 0usize;
    for (k, s) in said.iter_mut().enumerate() {
        s.sort();
        s.dedup();
        match s.len() {
            0 => {}
            1 => class_of[k] = names.binary_search(&s[0]).unwrap() as u32 + 1,
            _ => conflict += 1,
        }
    }
    if conflict > 0 {
        ctx.warn(format!("objects.accuracy: {conflict} object(s) have reference rows of different classes: left out"));
    }
    Ok((names, class_of))
}

/// `objects.accuracy`.
struct Accuracy;

impl Operator for Accuracy {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.accuracy",
            "The accuracy of a classification of objects against objects of known class (apart from those the \
             classifier was trained on). The reference is `samples` (points or polygons with the class, resolved to \
             objects as objects.samples does) or `reference` (a table with the columns id, the object, and class: \
             e.g. the points of objects.sample, filled in). The confusion matrix, the user's and producer's \
             accuracies and the overall accuracy and kappa are by objects (a sampled object counts one). The \
             area-adjusted estimates (Olofsson et al. 2014: the user's, producer's and overall accuracy, and each \
             class's area with its confidence interval) take the classes of the map as strata, with the cells of \
             the objects mapped to each as its area. `design` says how the sample was drawn: `objects` (each object \
             of a class had the same chance, as objects.sample draws them, or a table or polygons of such objects: \
             each sampled object is weighted by its area, the ratio estimator of Radoux and Bogaert 2017 and \
             Stehman 2014) or `area` (the chance of an object is proportional to its area, as points thrown at \
             random over the map: each sampled object counts once). Without `design`, a table or polygons are \
             `objects`, and points are `area` when an object holds several of them and `objects` otherwise (with a \
             warning: set it). An object left unclassified is a stratum only when sampled; a class with no \
             sampled object is left out, with a warning.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the classified objects"))
        .input(PortSpec::optional(
            "samples",
            Type::Vector,
            "points or polygons with the reference class, in any coordinate system",
        ))
        .input(PortSpec::optional(
            "reference",
            Type::Table,
            "the reference as a table instead: the columns id (the object) and class",
        ))
        .input(PortSpec::optional(
            "training",
            Type::Any,
            "the training objects (objects.samples' `objects`, or a table with their `id`): the reference objects that \
             are also training objects, or touch one, are counted in `overall` and warned of: the accuracy of objects \
             beside the ones the model learned from is higher than it is elsewhere",
        ))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "a row per class: class, map_cells, objects (sampled and mapped to it), reference_objects, \
             user_objects, producer_objects, user_adjusted, user_se, user_ci, producer_adjusted, producer_se, \
             producer_ci, area_share, area_cells, area_cells_ci, area_ha, area_ha_ci (the _ci the half-widths of \
             the confidence intervals)",
        ))
        .output(PortSpec::new(
            "overall",
            Type::Table,
            "overall_objects, overall_adjusted, overall_se, overall_ci, kappa (by objects), objects (sampled), \
             cells, confidence (and, with training, training_objects and touching_training: the sampled objects that \
             are training objects, and those that touch one)",
        ))
        .output(PortSpec::new(
            "matrix",
            Type::Table,
            "the confusion matrix by objects: a row per class of the map, a column per reference class",
        ))
        .output(PortSpec::new(
            "errors",
            Type::Table,
            "the misclassified sample objects: id, mapped class, reference class",
        ))
        .param(PortSpec::optional(
            "class",
            Type::Text,
            "the attribute (samples) or column (reference) holding the class (default class)",
        ))
        .param(PortSpec::optional("confidence", Type::Number, "the level of the confidence intervals (default 0.95)"))
        .param(PortSpec::optional(
            "design",
            Type::Text,
            "how the sample was drawn: objects (the same chance for each object of a class: the area-adjusted \
             estimates weight each sampled object by its area) or area (the chance proportional to the area, as \
             points at random over the map: each counts once). Default: from the reference (see above)",
        ))
        .param(PortSpec::optional(
            "cell_area",
            Type::Number,
            "a cell's area in m2 for the hectares (default: the cells' sides)",
        ))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let o = objects_in(args, "objects")?;
        let attr: String = args.param("class", "class".to_string())?;
        let confidence: f64 = args.param("confidence", 0.95)?;
        if !(confidence > 0.0 && confidence < 1.0) {
            return Err(Error::msg(format!("node {node:?}: confidence must be between 0 and 1")));
        }
        let z = qnorm(0.5 + confidence / 2.0);
        let cell_area: f64 = args.param("cell_area", o.cell_area())?;
        if o.class_of.iter().all(|&k| k == 0) {
            return Err(Error::msg(format!("node {node:?}: the objects have no class (classify them first)")));
        }
        let design: Option<String> = match args.params.get("design") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) if s == "objects" || s == "area" => Some(s.clone()),
            Some(v) => {
                return Err(Error::msg(format!("node {node:?}: design: objects or area, not {v}")));
            }
        };
        // by objects unless said or seen otherwise
        let mut by_objects = design.as_deref() != Some("area");
        let (ref_names, ref_of) = match (args.get("samples"), args.get("reference")) {
            (Some(_), Some(_)) => {
                return Err(Error::msg(format!("node {node:?}: the samples or the reference table, not both")));
            }
            (Some(_), None) => {
                let v = crate::vector::on_grid(crate::vector::vector_of(args, "samples")?, &o.grid)?;
                if design.is_none() && v.geometries.iter().any(|g| matches!(g, Geometry::Point(_))) {
                    by_objects = !points_share_objects(o, &v);
                    ctx.warn(format!(
                        "objects.accuracy: design not set: the points are taken as {}; set design to objects (one \
                         chance per object, as objects.sample) or area (points at random over the map)",
                        if by_objects {
                            "a sample with the same chance per object (objects)"
                        } else {
                            "random over the area (area)"
                        }
                    ));
                }
                sample_classes(ctx, node, o, &v, &attr)?
            }
            (None, Some(Value::Table(t))) => table_classes(ctx, node, o, t, &attr)?,
            (None, Some(v)) => {
                return Err(Error::msg(format!("node {node:?}: reference: a table, not a {:?}", v.ty())));
            }
            (None, None) => {
                return Err(Error::msg(format!(
                    "node {node:?}: connect the reference: samples (points or polygons) or a table"
                )));
            }
        };
        let sampled: Vec<usize> = (0..o.count).filter(|&i| ref_of[i] > 0).collect();
        if sampled.is_empty() {
            return Err(Error::msg(format!("node {node:?}: no object has a reference class")));
        }
        // the classes, by name: the codes of the estimates are positions in this list
        let mut names: Vec<String> = ref_names.clone();
        names.extend(o.classes.iter().map(|c| c.name.clone()));
        names.push(NONE.to_string());
        names.sort();
        names.dedup();
        let code = |s: &str| names.binary_search_by(|n| n.as_str().cmp(s)).unwrap() as f64 + 1.0;
        let map_code: Vec<f64> = (0..o.count).map(|i| code(mapped_name(o, i))).collect();
        let ref_code: Vec<f64> = (0..o.count)
            .map(|i| if ref_of[i] > 0 { code(&ref_names[ref_of[i] as usize - 1]) } else { f64::NAN })
            .collect();
        // the strata: the classes with cells and a sampled object
        let mut cells = vec![0.0; names.len()];
        let mut got = vec![0usize; names.len()];
        for i in 0..o.count {
            cells[map_code[i] as usize - 1] += o.cells[i] as f64;
        }
        for &i in &sampled {
            got[map_code[i] as usize - 1] += 1;
        }
        let mut strata = Vec::new();
        let mut stratum_cells = Vec::new();
        for (c, name) in names.iter().enumerate() {
            if cells[c] > 0.0 && got[c] > 0 {
                strata.push(c as f64 + 1.0);
                stratum_cells.push(cells[c]);
            } else if cells[c] > 0.0 && name != NONE {
                ctx.warn(format!(
                    "objects.accuracy: no sampled object is mapped to {name:?}: left out of the estimates"
                ));
            }
        }
        let map_s: Vec<f64> = sampled.iter().map(|&i| map_code[i]).collect();
        let ref_s: Vec<f64> = sampled.iter().map(|&i| ref_code[i]).collect();
        let e = if by_objects {
            let size: Vec<f64> = sampled.iter().map(|&i| o.cells[i] as f64).collect();
            let mut objs = vec![0.0; names.len()];
            for i in 0..o.count {
                objs[map_code[i] as usize - 1] += 1.0;
            }
            let objs: Vec<f64> = strata.iter().map(|&c| objs[c as usize - 1]).collect();
            olofsson_objects(&map_s, &ref_s, &size, &strata, &stratum_cells, &objs)
        } else {
            olofsson(&map_s, &ref_s, &strata, &stratum_cells)
        }
        .map_err(|m| Error::msg(format!("node {node:?}: {m}")))?;
        let mut idx: Vec<usize> = (0..e.classes.len()).collect();
        idx.sort_by(|&a, &b| e.classes[a].total_cmp(&e.classes[b]));
        // the confusion matrix by objects (rows: map, columns: reference), in the order of idx
        let q = idx.len();
        let at = |c: f64| idx.iter().position(|&j| e.classes[j] == c).unwrap();
        let mut count = vec![vec![0.0f64; q]; q];
        for (&m, &r) in map_s.iter().zip(&ref_s) {
            count[at(m)][at(r)] += 1.0;
        }
        let n = sampled.len() as f64;
        let row: Vec<f64> = count.iter().map(|r| r.iter().sum()).collect();
        let col: Vec<f64> = (0..q).map(|j| count.iter().map(|r| r[j]).sum()).collect();
        let diag: f64 = (0..q).map(|j| count[j][j]).sum();
        let (po, pe) = (diag / n, (0..q).map(|j| row[j] * col[j]).sum::<f64>() / (n * n));
        let kappa = if pe < 1.0 { (po - pe) / (1.0 - pe) } else { f64::NAN };
        let label = |t: &mut Table, column: &str| {
            t.labels.insert(column.to_string(), names.clone());
        };
        let mut t = Table::new(
            [
                "class",
                "map_cells",
                "objects",
                "reference_objects",
                "user_objects",
                "producer_objects",
                "user_adjusted",
                "user_se",
                "user_ci",
                "producer_adjusted",
                "producer_se",
                "producer_ci",
                "area_share",
                "area_cells",
                "area_cells_ci",
                "area_ha",
                "area_ha_ci",
            ]
            .map(String::from)
            .to_vec(),
        );
        for (k, &j) in idx.iter().enumerate() {
            let c = e.classes[j];
            let (a, a_ci) = (e.area[j] * e.total, z * e.area_se[j] * e.total);
            t.rows.push(vec![
                c,
                cells[c as usize - 1],
                row[k],
                col[k],
                count[k][k] / row[k],
                count[k][k] / col[k],
                e.user[j],
                e.user_se[j],
                z * e.user_se[j],
                e.producer[j],
                e.producer_se[j],
                z * e.producer_se[j],
                e.area[j],
                a,
                a_ci,
                a * cell_area / 10_000.0,
                a_ci * cell_area / 10_000.0,
            ]);
        }
        label(&mut t, "class");
        let mut overall = Table::new(
            [
                "overall_objects",
                "overall_adjusted",
                "overall_se",
                "overall_ci",
                "kappa",
                "objects",
                "cells",
                "confidence",
            ]
            .map(String::from)
            .to_vec(),
        );
        overall.rows.push(vec![po, e.overall, e.overall_se, z * e.overall_se, kappa, n, e.total, confidence]);
        if let Some(train) = training_of(args, o)? {
            let own = sampled.iter().filter(|&&i| train[i]).count();
            let near = sampled
                .iter()
                .filter(|&&i| !train[i] && o.neighbours_of(i as u32 + 1).iter().any(|&(nb, _)| train[nb as usize - 1]))
                .count();
            overall.columns.extend(["training_objects", "touching_training"].map(String::from));
            overall.rows[0].extend([own as f64, near as f64]);
            if own + near > 0 {
                ctx.warn(format!(
                    "objects.accuracy: {own} of the {} reference objects are training objects and {near} more touch \
                     one: their accuracy is higher than the model's on other places; use reference objects far from \
                     the samples (objects.folds makes the folds of a cross-validation by place)",
                    sampled.len()
                ));
            }
        }
        let header: Vec<String> = std::iter::once("map".to_string())
            .chain(idx.iter().map(|&j| names[e.classes[j] as usize - 1].clone()))
            .collect();
        let mut matrix = Table::new(header);
        for (k, &j) in idx.iter().enumerate() {
            matrix.rows.push(std::iter::once(e.classes[j]).chain(count[k].iter().copied()).collect());
        }
        label(&mut matrix, "map");
        let mut errors = Table::new(["id", "mapped", "reference"].map(String::from).to_vec());
        for &i in &sampled {
            if map_code[i] != ref_code[i] {
                errors.rows.push(vec![(i + 1) as f64, map_code[i], ref_code[i]]);
            }
        }
        label(&mut errors, "mapped");
        label(&mut errors, "reference");
        Ok(outputs([
            ("table", Value::Table(t)),
            ("overall", Value::Table(overall)),
            ("matrix", Value::Table(matrix)),
            ("errors", Value::Table(errors)),
        ]))
    }
}

/// The objects the classifier was trained on, when `training` is connected: the marked objects of objects.samples, or
/// the ids of a table.
fn training_of(args: &Args, o: &Objects) -> Result<Option<Vec<bool>>> {
    let node = args.node;
    Ok(match args.get("training") {
        None => None,
        Some(Value::Objects(t)) => {
            if t.count != o.count {
                return Err(Error::msg(format!("node {node:?}: training: objects of another segmentation")));
            }
            Some(t.class_of.iter().map(|&k| k != 0).collect())
        }
        Some(Value::Table(t)) => {
            let ids = t.column("id").ok_or_else(|| {
                Error::msg(format!("node {node:?}: training: a table needs the column id (or connect objects.samples' objects)"))
            })?;
            let mut flag = vec![false; o.count];
            for id in ids {
                if id.fract() != 0.0 || id < 1.0 || id > o.count as f64 {
                    return Err(Error::msg(format!("node {node:?}: training: the id {id} is no object")));
                }
                flag[id as usize - 1] = true;
            }
            Some(flag)
        }
        Some(v) => return Err(Error::msg(format!("node {node:?}: training: objects or a table, not a {:?}", v.ty()))),
    })
}

/// Whether some object holds two or more of the points (a sample of one object each never does).
fn points_share_objects(o: &Objects, v: &Vector) -> bool {
    let (g, t) = (&o.grid, o.grid.transform);
    let mut hits = vec![0u32; o.count];
    for geom in &v.geometries {
        let Geometry::Point(xy) = geom else { continue };
        let (c, r) = (((xy[0] - t[0]) / t[1]).floor(), ((xy[1] - t[3]) / t[5]).floor());
        if c >= 0.0 && r >= 0.0 && (c as usize) < g.width && (r as usize) < g.height {
            let l = o.labels[r as usize * g.width + c as usize];
            if l > 0 {
                hits[l as usize - 1] += 1;
                if hits[l as usize - 1] > 1 {
                    return true;
                }
            }
        }
    }
    false
}

/// `objects.sample`.
struct Sample;

impl Operator for Sample {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "objects.sample",
            "A stratified random sample of classified objects to label for objects.accuracy: the strata are the \
             classes, each object of a class with the same chance. `per_class` objects of each class, or `n` in \
             all shared equally among the classes (a class with fewer objects gives them all, the rest go to the \
             others). Olofsson et al. (2014) advise at least 50 per class: a class with fewer sampled objects \
             (though it has more) is warned of. Each sampled object gets a point at the cell nearest its centroid \
             that is in it, and a column `reference`, empty: fill it with the true class in the attribute table \
             (or a table) and take it to objects.accuracy. Unclassified objects are not sampled.",
            Access::Global,
        )
        .input(PortSpec::new("objects", Type::Objects, "the classified objects"))
        .output(PortSpec::new(
            "points",
            Type::Vector,
            "a point per sampled object: id (the object), mapped (its class, text) and reference (empty text, to \
             fill)",
        ))
        .output(PortSpec::new("table", Type::Table, "class, objects and sample of each class"))
        .param(PortSpec::optional(
            "n",
            Type::Integer,
            "the sample in all, shared equally among the classes (default 50)",
        ))
        .param(PortSpec::optional("per_class", Type::Integer, "the sample of each class, instead of n"))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of numpy's generator (default 1)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let o = objects_in(args, "objects")?;
        let seed: u64 = args.param("seed", 1)?;
        let each = match args.params.get("per_class") {
            None | Some(serde_json::Value::Null) => None,
            Some(v) => Some(v.as_u64().ok_or_else(|| Error::msg(format!("node {node:?}: per_class: a whole number")))?),
        };
        let n: usize = args.param("n", 50)?;
        let g = &o.grid;
        if g.transform[2] != 0.0 || g.transform[4] != 0.0 {
            return Err(Error::msg(format!("node {node:?}: the objects' grid is rotated")));
        }
        // the classes with objects, and their objects (ids by index, increasing)
        let mut members: Vec<Vec<usize>> = vec![Vec::new(); o.classes.len()];
        for (i, &k) in o.class_of.iter().enumerate() {
            if k > 0 {
                members[k as usize - 1].push(i);
            }
        }
        let used: Vec<usize> = (0..members.len()).filter(|&k| !members[k].is_empty()).collect();
        if used.is_empty() {
            return Err(Error::msg(format!("node {node:?}: the objects have no class (classify them first)")));
        }
        let caps: Vec<usize> = used.iter().map(|&k| members[k].len()).collect();
        let sizes: Vec<usize> = match each {
            Some(p) => caps.iter().map(|&c| c.min(p as usize)).collect(),
            None => apportion(n.min(caps.iter().sum()), &vec![1.0; used.len()], &caps),
        };
        let mut rng = NumpyRng::new(seed);
        let mut ids = Vec::new();
        let mut mapped = Vec::new();
        let mut geometries = Vec::new();
        let mut t = Table::new(["class", "objects", "sample"].map(String::from).to_vec());
        for (u, &k) in used.iter().enumerate() {
            let (have, take) = (caps[u], sizes[u]);
            let name = &o.classes[k].name;
            if take < ADVISED && take < have {
                ctx.warn(format!(
                    "objects.sample: {take} object(s) of {name:?}, which has {have}: Olofsson et al. advise at least \
                     {ADVISED} per class"
                ));
            }
            t.rows.push(vec![(u + 1) as f64, have as f64, take as f64]);
            let mut pick: Vec<usize> = if take == 0 {
                Vec::new()
            } else {
                rng.choice_distinct(have, take).into_iter().map(|j| members[k][j]).collect()
            };
            pick.sort_unstable();
            for i in pick {
                geometries.push(Geometry::Point(inside_point(o, i)));
                ids.push((i + 1) as f64);
                mapped.push(name.clone());
            }
        }
        t.labels.insert("class".into(), used.iter().map(|&k| o.classes[k].name.clone()).collect());
        let reference = vec![String::new(); ids.len()];
        let v = Vector {
            geometries,
            attributes: vec![
                ("id".to_string(), Column::Numbers(ids)),
                ("mapped".to_string(), Column::Texts(mapped)),
                ("reference".to_string(), Column::Texts(reference)),
            ],
            crs: crate::prepare::crs_of(g),
        };
        Ok(outputs([("points", Value::Vector(Arc::new(v))), ("table", Value::Table(t))]))
    }
}

/// The centre of the cell of an object (index) nearest its centroid.
fn inside_point(o: &Objects, i: usize) -> [f64; 2] {
    let g = &o.grid;
    let id = i as u32 + 1;
    let [r0, c0, r1, c1] = o.boxes[i];
    let (mut sr, mut sc, mut n) = (0.0, 0.0, 0.0);
    for r in r0..=r1 {
        for c in c0..=c1 {
            if o.labels[r as usize * g.width + c as usize] == id {
                sr += r as f64;
                sc += c as f64;
                n += 1.0;
            }
        }
    }
    let (mr, mc) = (sr / n, sc / n);
    let mut best = (f64::INFINITY, r0, c0);
    for r in r0..=r1 {
        for c in c0..=c1 {
            if o.labels[r as usize * g.width + c as usize] == id {
                let d = (r as f64 - mr).powi(2) + (c as f64 - mc).powi(2);
                if d < best.0 {
                    best = (d, r, c);
                }
            }
        }
    }
    let (x, y) = (best.2 as f64 + 0.5, best.1 as f64 + 0.5);
    let t = &g.transform;
    [t[0] + x * t[1] + y * t[2], t[3] + x * t[4] + y * t[5]]
}
