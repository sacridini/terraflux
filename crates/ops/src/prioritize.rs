//! Spatial prioritization from landschaft's C++: Zonation 5's ranking of the cells (`prioritize.zonation`), the
//! same rank map as the z5 binary; Marxan 4's reserves (`prioritize.marxan`), the same runs as the Marxan
//! program; ProxIRR's irreplaceability (`prioritize.proxirr`), the bits of R's proxirr.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::milp;
use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Zonation));
    r.add(Arc::new(Marxan));
    r.add(Arc::new(Proxirr));
    r.add(Arc::new(Optimize));
    r.add(Arc::new(Evaluate));
}

/// Zonation's marginal loss rules.
pub const RULES: [&str; 8] = ["CAZ1", "CAZ2", "CAZMAX", "CAZP", "CAZPW", "ABF", "RAND", "LOAD"];

/// A rule and its parameter ("ABF 0.25", or the rule and `rule_param`), the parameter as Zonation's float.
pub fn parse_rule(rule: &str, rule_param: Option<f64>) -> std::result::Result<(String, f64), String> {
    let mut parts = rule.split_whitespace();
    let name = parts.next().unwrap_or("CAZ2").to_uppercase();
    if !RULES.contains(&name.as_str()) {
        return Err(format!("unknown marginal loss rule {rule:?}; one of {}", RULES.join(", ")));
    }
    let given = match parts.next() {
        Some(p) => {
            if rule_param.is_some() {
                return Err("the rule's parameter is given twice".into());
            }
            Some(p.parse::<f64>().map_err(|_| format!("the rule's parameter {p:?} is no number"))?)
        }
        None => rule_param,
    };
    let param = given.unwrap_or(match name.as_str() {
        "ABF" => 0.25,
        "CAZP" | "CAZPW" => 1.0,
        _ => 0.0,
    });
    if (name == "CAZP" || name == "CAZPW") && param < 0.0 {
        return Err("the CAZP/CAZPW exponent is 0 or more".into());
    }
    Ok((name, f64::from(param as f32)))
}

/// A setting with a value per layer (a list, nulls for the default; or one value for all).
fn per_layer<T: Copy + serde::de::DeserializeOwned>(args: &Args, name: &str, n: usize, default: T) -> Result<Vec<T>> {
    let bad = |e: String| Error::msg(format!("node {:?}: {name}: {e}", args.node));
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok(vec![default; n]),
        Some(v @ serde_json::Value::Array(_)) => {
            let list: Vec<Option<T>> = serde_json::from_value(v.clone()).map_err(|e| bad(e.to_string()))?;
            if list.len() != n {
                return Err(bad(format!("{} values for {n} features", list.len())));
            }
            Ok(list.into_iter().map(|x| x.unwrap_or(default)).collect())
        }
        Some(v) => Ok(vec![serde_json::from_value(v.clone()).map_err(|e| bad(e.to_string()))?; n]),
    }
}

struct Zonation;

impl Operator for Zonation {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.zonation",
            "Zonation 5's priority ranking (Moilanen et al.), the same rank map as the z5 binary: cells removed \
             one after another by the least marginal loss of the features (rules CAZ1, CAZ2, CAZMAX, ABF, CAZP, \
             CAZPW; RAND, or LOAD a ranking given), with weights, costs, a hierarchic mask and an analysis area. \
             The rank goes from 0 (removed first) to 1 (kept last); the curves tell how much of each feature \
             remains as cells are removed.",
            Access::Global,
        )
        .input(PortSpec::new("features", Type::Stack, "the features' distributions, a layer each"))
        .input(PortSpec::optional("cost", Type::Map, "the cost of each cell (no value: 0)"))
        .input(PortSpec::optional("hierarchic_mask", Type::Map, "levels 0-255: higher levels are removed last"))
        .input(PortSpec::optional("analysis_area", Type::Map, "the cells ranked (not 0, or the area given)"))
        .input(PortSpec::optional("solution", Type::Map, "the ranking LOAD evaluates"))
        .output(PortSpec::new("rank", Type::Map, "the priority rank of each cell, 0-1"))
        .output(PortSpec::new(
            "curves",
            Type::Table,
            "the performance curves: the rank, the cost removed and the share of each feature remaining",
        ))
        .output(PortSpec::new(
            "features",
            Type::Table,
            "per feature ranked: its layer (1..), weight, sum, cells with values, cells without data and \
             quantization error (%)",
        ))
        .output(PortSpec::new("auc", Type::Number, "the area under the curve of the mean remaining share"))
        .output(PortSpec::new("iterations", Type::Number, "the iterations of the ranking"))
        .param(PortSpec::optional("rule", Type::Text, "CAZ2 (default), CAZ1, CAZMAX, ABF, CAZP, CAZPW, RAND, LOAD"))
        .param(PortSpec::optional("rule_param", Type::Number, "ABF's z (0.25), CAZP's and CAZPW's p (1)"))
        .param(PortSpec::optional("weights", Type::Any, "a weight per feature (default 1)"))
        .param(PortSpec::optional("abf_z", Type::Any, "ABF's z per feature (null: rule_param)"))
        .param(PortSpec::optional("weight_groups", Type::Any, "a group per feature (-1 none)"))
        .param(PortSpec::optional("group_weights", Type::Any, "{group: weight} shared by a group's features"))
        .param(PortSpec::optional("thresholds", Type::Any, "per feature: values below become 0"))
        .param(PortSpec::optional("extract", Type::Any, "per feature: the value its layer is 1 at (null: none)"))
        .param(PortSpec::optional(
            "nodata",
            Type::Any,
            "per feature: its file's NoData value (null: none), which value extraction tells from no value",
        ))
        .param(PortSpec::optional("zero_mode", Type::Text, "all (default), none or dynamic: the zeros are data"))
        .param(PortSpec::optional("keep_zeros", Type::Any, "dynamic: per feature, whether its zeros are data"))
        .param(PortSpec::optional("cost_in_ranking", Type::Bool, "the cost weighs the removal (default true)"))
        .param(PortSpec::optional("area", Type::Integer, "the analysis area's value ranked (default 0: every one)"))
        .param(PortSpec::optional("bits", Type::Integer, "8 (default) or 16: the precision of the data"))
        .param(PortSpec::optional("max_iterations", Type::Integer, "default 500"))
        .param(PortSpec::optional("ranking_error", Type::Number, "the error to stop at, % (default 0.01)"))
        .param(PortSpec::optional("stop_on_plateau", Type::Bool, "stop when the error no longer falls"))
        .param(PortSpec::optional("smoothing_factor", Type::Number, "default 0"))
        .param(PortSpec::optional("curves_interval", Type::Number, "the curves' step, a share of the cells"))
        .param(PortSpec::optional("seed", Type::Integer, "RAND's seed (default 0)"))
        .param(PortSpec::optional(
            "block_rows",
            Type::Integer,
            "Zonation adds the features up by blocks of rows: those of the files read (default: one block)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let stack = args.stack("features")?;
        let (ny, nx) = (stack.grid.height, stack.grid.width);
        let n = ny * nx;
        let l0 = stack.layers.len();
        if l0 == 0 {
            return Err(err("no features".into()));
        }
        let rule: String = args.param("rule", "CAZ2".to_string())?;
        let (rule, param) = parse_rule(&rule, args.opt_param("rule_param")?).map_err(err)?;
        let bits: i32 = args.param("bits", 8)?;
        if bits != 8 && bits != 16 {
            return Err(err("bits is 8 or 16".into()));
        }
        let groups_given = args.params.get("weight_groups").is_some_and(|v| !v.is_null());
        let weights: Vec<f32> = per_layer(args, "weights", l0, 1.0f32)?;
        let extract: Vec<f32> = per_layer(args, "extract", l0, f32::NAN)?;
        let thresholds: Vec<f32> = per_layer(args, "thresholds", l0, 0.0f32)?;
        let abf_z: Vec<f32> = per_layer(args, "abf_z", l0, f32::NAN)?;
        let wgroups: Vec<i64> = per_layer(args, "weight_groups", l0, -1i64)?;
        let zero_mode: String = args.param("zero_mode", "all".to_string())?;
        let keep: Vec<u8> = match zero_mode.as_str() {
            "all" => vec![1; l0],
            "none" => vec![0; l0],
            "dynamic" => per_layer::<u8>(args, "keep_zeros", l0, 0)?,
            m => return Err(err(format!("zero_mode is all, none or dynamic, not {m:?}"))),
        };
        // the maps the features are on
        let on_grid = |port: &str| -> Result<Option<&Map>> {
            let m = args.opt_map(port)?;
            if let Some(m) = m
                && m.shape() != (ny, nx)
            {
                return Err(err(format!("{port} is not on the features' grid")));
            }
            Ok(m)
        };
        let area_map = on_grid("analysis_area")?;
        let which: i64 = args.param("area", 0)?;
        // the analysis area: cells not 0 (or the value given), no value 0
        let area: Vec<u8> = area_map.map_or_else(Vec::new, |m| {
            m.data
                .iter()
                .map(|&v| {
                    let v = if v.is_nan() { 0.0 } else { v.round_ties_even().clamp(0.0, 4_294_967_295.0) };
                    u8::from(if which == 0 { v != 0.0 } else { v == which as f64 })
                })
                .collect()
        });
        // a feature's NoData value: the cells without a value hold it, as its file does (Zonation's value
        // extraction tells them from NaN)
        let nodata: Vec<f32> = per_layer(args, "nodata", l0, f32::NAN)?;
        let values: Vec<f32> = stack
            .layers
            .iter()
            .zip(&nodata)
            .flat_map(|(l, &nd)| l.iter().map(move |&v| if v.is_nan() { nd } else { v as f32 }))
            .collect();
        let stats = ffi::prio_stats(&values, l0 as i32, ny as i64, nx as i64, &nodata, &extract, &thresholds, &area)?;
        let kept: Vec<usize> = (0..l0).filter(|&i| stats.max_values[i] > 0.0).collect();
        if kept.is_empty() {
            return Err(err("nothing remains after leaving out the empty features".into()));
        }
        // the weights of the features kept, those of a group sharing its weight (Zonation's floats)
        let mut w: Vec<f32> = kept.iter().map(|&i| weights[i]).collect();
        if groups_given {
            let gw: BTreeMap<i64, f32> = match args.params.get("group_weights") {
                None | Some(serde_json::Value::Null) => BTreeMap::new(),
                Some(v) => {
                    let m: BTreeMap<String, f64> = serde_json::from_value(v.clone())
                        .map_err(|e| err(format!("group_weights: {e} ({{group: weight}})")))?;
                    m.into_iter()
                        .map(|(k, v)| k.parse::<i64>().map(|k| (k, v as f32)))
                        .collect::<std::result::Result<_, _>>()
                        .map_err(|_| err("group_weights: groups are numbers".into()))?
                }
            };
            if !gw.is_empty() {
                let wg: Vec<i64> = kept.iter().map(|&i| wgroups[i]).collect();
                let mut sums: BTreeMap<i64, f32> = BTreeMap::new();
                for (g, x) in wg.iter().zip(&w) {
                    if *g >= 0 {
                        *sums.entry(*g).or_insert(0.0) += x.abs();
                    }
                }
                for (i, g) in wg.iter().enumerate() {
                    if let (Some(gwv), Some(s)) = (gw.get(g), sums.get(g))
                        && *s > 0.0
                    {
                        w[i] *= gwv / s;
                    }
                }
            }
        }
        let k = kept.len();
        let pick = |v: &[f32]| -> Vec<f32> { kept.iter().map(|&i| v[i]).collect() };
        let kept_values: Vec<f32> = kept.iter().flat_map(|&i| values[i * n..(i + 1) * n].iter().copied()).collect();
        let max_values: Vec<f32> = kept.iter().map(|&i| stats.max_values[i]).collect();
        let keep_zeros: Vec<u8> = kept.iter().map(|&i| keep[i]).collect();
        let map_data =
            |port: &str| -> Result<Vec<f64>> { Ok(on_grid(port)?.map_or_else(Vec::new, |m| m.data.to_vec())) };
        let settings = ffi::PrioSettings {
            rule: rule.clone(),
            param,
            weights: w.clone(),
            abf_z: pick(&abf_z),
            cost_in_ranking: args.param("cost_in_ranking", true)?,
            max_iterations: args.param("max_iterations", 500)?,
            ranking_error: args.param("ranking_error", 0.01f32)?,
            stop_on_plateau: args.param("stop_on_plateau", false)?,
            smoothing: args.param("smoothing_factor", 0.0f32)?,
            curves_interval: args.param("curves_interval", 0.0001f32)?,
            seed: args.param("seed", 0u64)?,
        };
        let block_rows: i64 = args.param("block_rows", 0)?;
        let r = ffi::prio_rank(
            &kept_values,
            k as i32,
            ny as i64,
            nx as i64,
            &pick(&nodata),
            &pick(&extract),
            &pick(&thresholds),
            &max_values,
            &keep_zeros,
            bits,
            &area,
            block_rows,
            &settings,
            &map_data("cost")?,
            &map_data("hierarchic_mask")?,
            &map_data("solution")?,
        )?;
        // the rank on the grid
        let mut rank = vec![f64::NAN; n];
        for (c, v) in r.cell_index.iter().zip(&r.rank) {
            rank[*c as usize] = f64::from(*v);
        }
        let names: Vec<String> = kept.iter().map(|&i| stack.keys[i].to_string()).collect();
        let mut columns = vec!["rank".to_string(), "cost".to_string()];
        columns.extend(names.iter().cloned());
        let mut curves = Table::new(columns);
        for (s, cr) in r.curve_rank.iter().enumerate() {
            let mut row = vec![f64::from(*cr), r.curve_cost.get(s).copied().unwrap_or(f64::NAN)];
            row.extend(r.curve_levels[s * k..(s + 1) * k].iter().map(|&v| f64::from(v)));
            curves.rows.push(row);
        }
        let mut features =
            Table::new(["layer", "weight", "sum", "count", "missing", "quantization_error"].map(String::from).to_vec());
        for (j, &i) in kept.iter().enumerate() {
            let count = stats.pixels - stats.nodata[i] - stats.zeros[i];
            features.rows.push(vec![
                (i + 1) as f64,
                f64::from(w[j]),
                r.orig_abs_sums[j],
                count as f64,
                stats.nodata[i] as f64,
                r.quantization_errors[j],
            ]);
        }
        Ok(outputs([
            ("rank", Value::Map(Map::new(stack.grid.clone(), rank))),
            ("curves", Value::Table(curves)),
            ("features", Value::Table(features)),
            ("auc", Value::Number(r.auc)),
            ("iterations", Value::Number(f64::from(r.iterations))),
        ]))
    }
}

/// A table's column by name (its default when the table has none).
fn column(t: &Table, name: &str) -> Option<Vec<f64>> {
    t.column(name)
}

/// Marxan's problem from its tables: pu (id, cost, status), spec (id, target, prop, spf, targetocc), puvspr
/// (species, pu, amount) and bound (id1, id2, boundary), as landschaft's ls.prioritize.marxan reads them.
pub fn marxan_data(
    pu: &Table,
    spec: &Table,
    puvspr: &Table,
    bound: Option<&Table>,
) -> std::result::Result<ffi::MarxanData, String> {
    let need = |t: &Table, cols: &[&str], what: &str| -> std::result::Result<(), String> {
        let missing: Vec<&&str> = cols.iter().filter(|c| !t.columns.iter().any(|x| x == **c)).collect();
        if missing.is_empty() { Ok(()) } else { Err(format!("{what} needs the columns {missing:?}")) }
    };
    need(pu, &["id", "cost"], "pu")?;
    need(spec, &["id"], "spec")?;
    need(puvspr, &["species", "pu", "amount"], "puvspr")?;
    let ids = column(pu, "id").unwrap();
    let key = |v: f64| v.to_bits();
    let mut pu_index = std::collections::HashMap::new();
    for (i, &id) in ids.iter().enumerate() {
        if pu_index.insert(key(id), i).is_some() {
            return Err("planning-unit ids must be unique".into());
        }
    }
    let sp_ids = column(spec, "id").unwrap();
    let mut sp_index = std::collections::HashMap::new();
    for (i, &id) in sp_ids.iter().enumerate() {
        if sp_index.insert(key(id), i).is_some() {
            return Err("feature ids must be unique".into());
        }
    }
    let npu = ids.len();
    let (species, units, amount) =
        (column(puvspr, "species").unwrap(), column(puvspr, "pu").unwrap(), column(puvspr, "amount").unwrap());
    let mut rows: Vec<(usize, usize, f64)> = Vec::with_capacity(species.len());
    let mut seen = std::collections::HashSet::new();
    for i in 0..species.len() {
        let (Some(&r), Some(&k)) = (pu_index.get(&key(units[i])), sp_index.get(&key(species[i]))) else {
            return Err("puvspr refers to planning units or features that are not in pu or spec".into());
        };
        if !seen.insert((r, k)) {
            return Err("puvspr has a feature twice in a planning unit".into());
        }
        rows.push((r, k, amount[i]));
    }
    // by unit, then feature
    rows.sort_by_key(|&(r, k, _)| (r, k));
    let mut offset = vec![0i64; npu + 1];
    for &(r, ..) in &rows {
        offset[r + 1] += 1;
    }
    for i in 0..npu {
        offset[i + 1] += offset[i];
    }
    let (fixed, nb_start, nb, nb_cost) = match bound {
        Some(b) if !b.rows.is_empty() => {
            need(b, &["id1", "id2", "boundary"], "bound")?;
            let (i1, i2, v) = (column(b, "id1").unwrap(), column(b, "id2").unwrap(), column(b, "boundary").unwrap());
            let mut edges = Vec::with_capacity(i1.len());
            for j in 0..i1.len() {
                let (Some(&a), Some(&c)) = (pu_index.get(&key(i1[j])), pu_index.get(&key(i2[j]))) else {
                    return Err("bound refers to planning units that are not in pu".into());
                };
                edges.push((a, c, v[j]));
            }
            neighbours(npu, &edges)
        }
        _ => (vec![0.0; npu], vec![0i64; npu + 1], Vec::new(), Vec::new()),
    };
    let col_or = |t: &Table, name: &str, default: f64| column(t, name).unwrap_or_else(|| vec![default; t.rows.len()]);
    Ok(ffi::MarxanData {
        cost: column(pu, "cost").unwrap(),
        status: col_or(pu, "status", 0.0).into_iter().map(|v| v as i32).collect(),
        offset,
        spindex: rows.iter().map(|&(_, k, _)| k as i32).collect(),
        amount: rows.iter().map(|&(.., a)| a).collect(),
        target: col_or(spec, "target", 0.0),
        prop: col_or(spec, "prop", 0.0),
        spf: col_or(spec, "spf", -1.0),
        targetocc: col_or(spec, "targetocc", 0.0).into_iter().map(|v| v as i32).collect(),
        fixedcost: fixed,
        nb_start,
        nb,
        nb_cost,
    })
}

/// Marxan's boundaries from edges (unit a, unit b, length) in their order: a unit's edge with itself adds to its
/// fixed boundary; every other pair is a neighbour of both once (the first length given).
fn neighbours(npu: usize, edges: &[(usize, usize, f64)]) -> (Vec<f64>, Vec<i64>, Vec<i32>, Vec<f64>) {
    let mut fixed = vec![0.0; npu];
    let mut nbrs: Vec<Vec<(usize, f64)>> = vec![Vec::new(); npu];
    for &(a, c, v) in edges {
        if a == c {
            fixed[a] += v;
            continue;
        }
        if nbrs[a].iter().all(|(n, _)| *n != c) {
            nbrs[a].push((c, v));
        }
        if nbrs[c].iter().all(|(n, _)| *n != a) {
            nbrs[c].push((a, v));
        }
    }
    let mut nb_start = vec![0i64; npu + 1];
    for i in 0..npu {
        nb_start[i + 1] = nb_start[i] + nbrs[i].len() as i64;
    }
    let nb = nbrs.iter().flat_map(|x| x.iter().map(|(n, _)| *n as i32)).collect();
    let nb_cost = nbrs.iter().flat_map(|x| x.iter().map(|(_, v)| *v)).collect();
    (fixed, nb_start, nb, nb_cost)
}

/// Marxan's problem from maps: every cell with a cost a planning unit (numbered from 1 in the map's order), the
/// features' layers its amounts, the edges it shares with the units beside it its boundaries (their lengths in
/// the map's units), its edges to no unit `edge_factor` times their length.
fn raster_data(
    features: &Stack,
    cost: &Map,
    status: Option<&Map>,
    edge_factor: f64,
) -> std::result::Result<(ffi::MarxanData, Vec<usize>), String> {
    let (ny, nx) = cost.shape();
    if (features.grid.height, features.grid.width) != (ny, nx) {
        return Err("the features are not on the cost's grid".into());
    }
    let cells: Vec<usize> = (0..ny * nx).filter(|&c| !cost.data[c].is_nan()).collect();
    let mut unit = vec![usize::MAX; ny * nx];
    for (i, &c) in cells.iter().enumerate() {
        unit[c] = i;
    }
    let npu = cells.len();
    let (w, h) = (cost.grid.transform[1].abs(), cost.grid.transform[5].abs());
    let mut offset = vec![0i64; npu + 1];
    let (mut spindex, mut amount) = (Vec::new(), Vec::new());
    for (i, &c) in cells.iter().enumerate() {
        for (k, layer) in features.layers.iter().enumerate() {
            let a = layer[c];
            if !a.is_nan() && a > 0.0 {
                spindex.push(k as i32);
                amount.push(a);
            }
        }
        offset[i + 1] = spindex.len() as i64;
    }
    // the edges in the map's order: a unit's right and lower neighbour, its edges to no unit
    let mut edges = Vec::new();
    for (i, &c) in cells.iter().enumerate() {
        let (y, x) = (c / nx, c % nx);
        let mut exposed = 0.0;
        for (dy, dx, len) in [(0i64, 1i64, h), (1, 0, w), (0, -1, h), (-1, 0, w)] {
            let (yy, xx) = (y as i64 + dy, x as i64 + dx);
            let other = (yy >= 0 && yy < ny as i64 && xx >= 0 && xx < nx as i64)
                .then(|| unit[yy as usize * nx + xx as usize])
                .filter(|&u| u != usize::MAX);
            match other {
                Some(o) if o > i => edges.push((i, o, len)),
                Some(_) => {}
                None => exposed += len,
            }
        }
        if edge_factor != 0.0 && exposed > 0.0 {
            edges.push((i, i, exposed * edge_factor));
        }
    }
    let (fixed, nb_start, nb, nb_cost) = neighbours(npu, &edges);
    let st: Vec<i32> = match status {
        Some(s) => cells.iter().map(|&c| if s.data[c].is_nan() { 0 } else { s.data[c] as i32 }).collect(),
        None => vec![0; npu],
    };
    let nsp = features.layers.len();
    Ok((
        ffi::MarxanData {
            cost: cells.iter().map(|&c| cost.data[c]).collect(),
            status: st,
            offset,
            spindex,
            amount,
            target: vec![0.0; nsp],
            prop: vec![0.0; nsp],
            spf: vec![-1.0; nsp],
            targetocc: vec![0; nsp],
            fixedcost: fixed,
            nb_start,
            nb,
            nb_cost,
        },
        cells,
    ))
}

struct Marxan;

impl Operator for Marxan {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.marxan",
            "Marxan 4: reserves that meet the features' targets at the least cost and boundary, by simulated \
             annealing and iterative improvement, a number of runs; the same runs as the Marxan program. From \
             Marxan's tables (pu: id, cost, status; spec: id, target, prop, spf, targetocc; puvspr: species, pu, \
             amount; bound: id1, id2, boundary), or from maps: every cell with a cost a planning unit, the \
             features' layers its amounts, its shared edges its boundaries.",
            Access::Global,
        )
        .input(PortSpec::optional("pu", Type::Table, "the planning units: id, cost, status (0-3)"))
        .input(PortSpec::optional("spec", Type::Table, "the features: id, target, prop, spf, targetocc"))
        .input(PortSpec::optional("puvspr", Type::Table, "the amounts: species, pu, amount"))
        .input(PortSpec::optional("bound", Type::Table, "the boundaries: id1, id2, boundary"))
        .input(PortSpec::optional("features", Type::Stack, "maps instead of tables: the features' amounts"))
        .input(PortSpec::optional("cost", Type::Map, "maps: the cost of each cell (no value: no unit)"))
        .input(PortSpec::optional("status", Type::Map, "maps: 0 available, 1 in at first, 2 locked in, 3 out"))
        .output(PortSpec::new("best", Type::Table, "the best run's reserve: pu, solution (1 in)"))
        .output(PortSpec::new("solutions", Type::Table, "every run's reserve: pu, then a column per run (1 in)"))
        .output(PortSpec::new("frequency", Type::Table, "how often each unit was selected: pu, frequency (0-1)"))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "per run: its score, cost, units, boundary, penalty, shortfall, features missed, least proportion met \
             and final score",
        ))
        .output(PortSpec::new("penalties", Type::Table, "per feature: its id, target and penalty"))
        .output(PortSpec::new("map", Type::Map, "maps: the best run's reserve (1 in, 0 out)"))
        .output(PortSpec::new("frequency_map", Type::Map, "maps: how often each cell was selected"))
        .param(PortSpec::optional("blm", Type::Number, "the boundary length modifier (default 0)"))
        .param(PortSpec::optional(
            "prop",
            Type::Any,
            "Marxan's PROP: every feature's target as a share of its total; maps: a list, a share per feature",
        ))
        .param(PortSpec::optional("targets", Type::Any, "maps: the features' targets (amounts)"))
        .param(PortSpec::optional("spf", Type::Any, "maps: the features' penalty factors (default Marxan's)"))
        .param(PortSpec::optional("edge_factor", Type::Number, "maps: the weight of the edges to no unit (default 1)"))
        .param(PortSpec::optional("num_reps", Type::Integer, "runs (default 10)"))
        .param(PortSpec::optional("num_iters", Type::Integer, "annealing's iterations (default 1000000)"))
        .param(PortSpec::optional("start_temp", Type::Number, "the starting temperature (-1: adaptive, default)"))
        .param(PortSpec::optional("cool_fac", Type::Number, "the cooling factor (default 0)"))
        .param(PortSpec::optional("num_temp", Type::Integer, "temperature decreases (default 10000)"))
        .param(PortSpec::optional("run_mode", Type::Integer, "Marxan's run mode (default 1)"))
        .param(PortSpec::optional("heur_type", Type::Integer, "the heuristic (default 1)"))
        .param(PortSpec::optional("itimp_type", Type::Integer, "the iterative improvement (default 0)"))
        .param(PortSpec::optional("misslevel", Type::Number, "the share of a target that counts as met (1)"))
        .param(PortSpec::optional("cost_thresh", Type::Number, "the cost threshold (default 0: none)"))
        .param(PortSpec::optional("thresh_pen1", Type::Number, "the cost threshold's penalty a"))
        .param(PortSpec::optional("thresh_pen2", Type::Number, "the cost threshold's penalty b"))
        .param(PortSpec::optional("connectivity_in", Type::Bool, "boundaries within the reserve count"))
        .param(PortSpec::optional("seed", Type::Integer, "the random seed (default 1)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let table = |port: &str| -> Result<Option<&Table>> {
            match args.get(port) {
                None => Ok(None),
                Some(Value::Table(t)) => Ok(Some(t)),
                Some(v) => Err(err(format!("{port} is a {:?}, not a table", v.ty()))),
            }
        };
        let (mut data, ids, cells, grid) = match (table("pu")?, table("spec")?, table("puvspr")?) {
            (Some(pu), Some(spec), Some(puvspr)) => {
                let d = marxan_data(pu, spec, puvspr, table("bound")?).map_err(err)?;
                (d, pu.column("id").unwrap(), None, None)
            }
            (None, None, None) => {
                let features = args
                    .stack("features")
                    .map_err(|_| err("give the tables pu, spec and puvspr, or the maps features and cost".into()))?;
                let cost = args.map("cost")?;
                let edge_factor: f64 = args.param("edge_factor", 1.0)?;
                let (mut d, cells) = raster_data(features, cost, args.opt_map("status")?, edge_factor).map_err(err)?;
                let nsp = features.layers.len();
                let per = |name: &str, default: f64| -> Result<Vec<f64>> {
                    match args.params.get(name) {
                        None | Some(serde_json::Value::Null) => Ok(vec![default; nsp]),
                        Some(serde_json::Value::Number(n)) => Ok(vec![n.as_f64().unwrap(); nsp]),
                        Some(v) => {
                            let l: Vec<f64> =
                                serde_json::from_value(v.clone()).map_err(|e| err(format!("{name}: {e}")))?;
                            if l.len() != nsp {
                                return Err(err(format!("{name}: {} values for {nsp} features", l.len())));
                            }
                            Ok(l)
                        }
                    }
                };
                d.target = per("targets", 0.0)?;
                // a list of shares per feature (a number is Marxan's PROP, for all)
                d.prop = match args.params.get("prop") {
                    Some(serde_json::Value::Array(_)) => per("prop", 0.0)?,
                    _ => vec![0.0; nsp],
                };
                d.spf = per("spf", -1.0)?;
                let ids = (1..=cells.len()).map(|i| i as f64).collect();
                (d, ids, Some(cells), Some(cost.grid.clone()))
            }
            _ => return Err(err("give the three tables pu, spec and puvspr".into())),
        };
        let run_mode: i32 = args.param("run_mode", 1)?;
        if !(-1..=6).contains(&run_mode) {
            return Err(err(format!("Marxan's run modes are -1 to 6, not {run_mode}")));
        }
        let options = ffi::MarxanOptions {
            blm: args.param("blm", 0.0)?,
            prop: match args.params.get("prop") {
                Some(serde_json::Value::Number(n)) => n.as_f64().unwrap(),
                _ => 0.0,
            },
            iterations: args.param("num_iters", 1_000_000)?,
            tinit: args.param("start_temp", -1.0)?,
            tcool: args.param("cool_fac", 0.0)?,
            titns: args.param("num_temp", 10_000)?,
            repeats: args.param("num_reps", 10)?,
            costthresh: args.param("cost_thresh", 0.0)?,
            tpf1: args.param("thresh_pen1", 0.0)?,
            tpf2: args.param("thresh_pen2", 0.0)?,
            misslevel: args.param("misslevel", 1.0)?,
            runmode: run_mode,
            heurtype: args.param("heur_type", 1)?,
            itimptype: args.param("itimp_type", 0)?,
            connectivity_in: args.param("connectivity_in", false)?,
            seed: (args.param::<i64>("seed", 1)? & 0xFFFF_FFFF) as u32,
        };
        // the spec's proportion targets apply as Marxan's PROP only per feature
        if data.prop.iter().any(|p| p.is_nan()) {
            data.prop.iter_mut().for_each(|p| *p = if p.is_nan() { 0.0 } else { *p });
        }
        let npu = data.cost.len();
        let nsp = data.target.len();
        let out = ffi::opt_marxan(&data, &options)?;
        let runs = out.score.len();
        let selected = |r: usize, i: usize| matches!(out.solutions[r * npu + i], 1 | 2);
        let best = out.best as usize;
        let mut best_t = Table::new(vec!["pu".into(), "solution".into()]);
        let mut freq_t = Table::new(vec!["pu".into(), "frequency".into()]);
        let mut freq = vec![0.0; npu];
        for i in 0..npu {
            let n = (0..runs).filter(|&r| selected(r, i)).count();
            freq[i] = if runs > 0 { n as f64 / runs as f64 } else { 0.0 };
            best_t.rows.push(vec![ids[i], f64::from(u8::from(runs > 0 && selected(best, i)))]);
            freq_t.rows.push(vec![ids[i], freq[i]]);
        }
        let mut columns = vec!["pu".to_string()];
        columns.extend((1..=runs).map(|r| format!("run_{r}")));
        let mut solutions = Table::new(columns);
        solutions.rows = (0..npu)
            .map(|i| std::iter::once(ids[i]).chain((0..runs).map(|r| f64::from(u8::from(selected(r, i))))).collect())
            .collect();
        let mut summary = Table::new(
            [
                "run",
                "score",
                "cost",
                "planning_units",
                "connectivity",
                "penalty",
                "shortfall",
                "missing",
                "mpm",
                "final_score",
            ]
            .map(String::from)
            .to_vec(),
        );
        for r in 0..runs {
            summary.rows.push(vec![
                (r + 1) as f64,
                out.incremental_score[r],
                out.cost[r],
                out.pus[r] as f64,
                out.connectivity[r],
                out.penalty_total[r],
                out.shortfall[r],
                out.missing[r] as f64,
                out.mpm[r],
                out.score[r],
            ]);
        }
        let mut penalties = Table::new(vec!["feature".into(), "target".into(), "penalty".into()]);
        let sp_ids =
            table("spec")?.and_then(|s| s.column("id")).unwrap_or_else(|| (1..=nsp).map(|i| i as f64).collect());
        penalties.rows =
            sp_ids.iter().zip(&out.target).zip(&out.penalty).map(|((id, t), p)| vec![*id, *t, *p]).collect();
        let mut o = outputs([
            ("best", Value::Table(best_t)),
            ("solutions", Value::Table(solutions)),
            ("frequency", Value::Table(freq_t)),
            ("summary", Value::Table(summary)),
            ("penalties", Value::Table(penalties)),
        ]);
        if let (Some(cells), Some(grid)) = (cells, grid) {
            let n = grid.width * grid.height;
            let (mut m, mut f) = (vec![f64::NAN; n], vec![f64::NAN; n]);
            for (i, &c) in cells.iter().enumerate() {
                m[c] = f64::from(u8::from(runs > 0 && selected(best, i)));
                f[c] = freq[i];
            }
            o.insert("map".into(), Value::Map(Map::new(grid.clone(), m)));
            o.insert("frequency_map".into(), Value::Map(Map::new(grid, f)));
        }
        Ok(o)
    }
}

/// ProxIRR from Marxan's tables (landschaft's eval_proxirr_marxan): per puvspr row its feature's global amount
/// (R's sum of its rows) and target (spec's prop times it, or its target), its alpha; per unit of pu its beta and
/// gamma from its rows' alphas in their order.
pub struct ProxirrTables {
    pub global: Vec<f64>,
    pub target: Vec<f64>,
    pub alpha: Vec<f64>,
    pub beta: Vec<f64>,
    pub gamma: Vec<f64>,
    /// per spec row: its global amount and target
    pub spec_global: Vec<f64>,
    pub spec_target: Vec<f64>,
}

pub fn proxirr_tables(
    pu: &Table,
    spec: &Table,
    puvspr: &Table,
    triage: bool,
) -> std::result::Result<ProxirrTables, String> {
    let need = |t: &Table, cols: &[&str], what: &str| -> std::result::Result<(), String> {
        let missing: Vec<&&str> = cols.iter().filter(|c| !t.columns.iter().any(|x| x == **c)).collect();
        if missing.is_empty() { Ok(()) } else { Err(format!("{what} needs the columns {missing:?}")) }
    };
    need(pu, &["id"], "pu")?;
    need(spec, &["id"], "spec")?;
    need(puvspr, &["species", "pu", "amount"], "puvspr")?;
    let (prop, target) = (spec.column("prop"), spec.column("target"));
    if prop.is_none() && target.is_none() {
        return Err("spec needs a 'prop' or a 'target' column".into());
    }
    let sp_ids = spec.column("id").unwrap();
    let mut sp_pos = std::collections::HashMap::new();
    for (i, id) in sp_ids.iter().enumerate() {
        if sp_pos.insert(id.to_bits(), i).is_some() {
            return Err("feature ids must be unique".into());
        }
    }
    let (species, units, amount) =
        (puvspr.column("species").unwrap(), puvspr.column("pu").unwrap(), puvspr.column("amount").unwrap());
    // a feature's global amount: R's sum of its rows, in their order
    let glob: Vec<f64> = sp_ids
        .iter()
        .map(|sid| {
            let rows: Vec<f64> = species.iter().zip(&amount).filter(|(s, _)| *s == sid).map(|(_, a)| *a).collect();
            ffi::r_sum(&rows)
        })
        .collect();
    let spec_target: Vec<f64> = match &prop {
        Some(p) => glob.iter().zip(p).map(|(g, p)| g * p).collect(),
        None => target.clone().unwrap(),
    };
    let (mut g, mut tt) = (vec![f64::NAN; species.len()], vec![f64::NAN; species.len()]);
    for (i, s) in species.iter().enumerate() {
        if let Some(&k) = sp_pos.get(&s.to_bits()) {
            g[i] = glob[k];
            tt[i] = spec_target[k];
        }
    }
    let alpha = ffi::proxirr_alpha(&amount, &g, &tt, triage).map_err(|e| e.what().to_string())?;
    let pu_ids = pu.column("id").unwrap();
    let mut pu_pos = std::collections::HashMap::new();
    for (i, id) in pu_ids.iter().enumerate() {
        if pu_pos.insert(id.to_bits(), i).is_some() {
            return Err("planning-unit ids must be unique".into());
        }
    }
    // the rows of every unit, in their order (the rows of no unit left out)
    let mut by_unit: Vec<Vec<f64>> = vec![Vec::new(); pu_ids.len()];
    for (i, u) in units.iter().enumerate() {
        if let Some(&p) = pu_pos.get(&u.to_bits()) {
            by_unit[p].push(alpha[i]);
        }
    }
    let mut start = vec![0i64];
    let mut alphas = Vec::new();
    for a in &by_unit {
        alphas.extend(a);
        start.push(alphas.len() as i64);
    }
    let bg = ffi::proxirr_beta_gamma(&start, &alphas).map_err(|e| e.what().to_string())?;
    Ok(ProxirrTables { global: g, target: tt, alpha, beta: bg.beta, gamma: bg.gamma, spec_global: glob, spec_target })
}

struct Proxirr;

impl Operator for Proxirr {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.proxirr",
            "ProxIRR irreplaceability (Baisero et al. 2022), the bits of R's proxirr: for every feature in a unit \
             alpha = min(1, local / (global - target)), and per unit beta = 1 - prod(1 - alpha) (the chance it is \
             needed for some target) and gamma (the targets it is needed for). From Marxan's tables (pu: id; \
             spec: id, prop or target; puvspr: species, pu, amount), or from maps: every cell a unit, the \
             features' layers its amounts.",
            Access::Global,
        )
        .input(PortSpec::optional("pu", Type::Table, "the planning units: id"))
        .input(PortSpec::optional("spec", Type::Table, "the features: id, prop (a share of the total) or target"))
        .input(PortSpec::optional("puvspr", Type::Table, "the amounts: species, pu, amount"))
        .input(PortSpec::optional("features", Type::Stack, "maps instead of tables: the features' amounts"))
        .output(PortSpec::new("units", Type::Table, "per unit: pu, beta, gamma"))
        .output(PortSpec::new(
            "alpha",
            Type::Table,
            "per feature in a unit: species, pu, amount, the feature's global amount and target, alpha",
        ))
        .output(PortSpec::new("targets", Type::Table, "per feature: its id, global amount and target"))
        .output(PortSpec::new("beta", Type::Map, "maps: each cell's beta"))
        .output(PortSpec::new("gamma", Type::Map, "maps: each cell's gamma"))
        .param(PortSpec::optional("prop", Type::Any, "maps: the targets as a share of each feature's total"))
        .param(PortSpec::optional("targets", Type::Any, "maps: the features' targets (amounts)"))
        .param(PortSpec::optional(
            "triage",
            Type::Bool,
            "targets above the global amount count 0 (default false)",
        ))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let triage: bool = args.param("triage", false)?;
        let table = |port: &str| -> Option<&Table> {
            match args.get(port) {
                Some(Value::Table(t)) => Some(t),
                _ => None,
            }
        };
        // maps: every cell a unit, its features with an amount above 0 the entries (Marxan's tables made of them)
        let (pu, spec, puvspr, cells) = match (table("pu"), table("spec"), table("puvspr")) {
            (Some(pu), Some(spec), Some(puvspr)) => (pu.clone(), spec.clone(), puvspr.clone(), None),
            _ => {
                let features = args
                    .stack("features")
                    .map_err(|_| err("give the tables pu, spec and puvspr, or the map features".into()))?;
                let nsp = features.layers.len();
                let n = features.grid.width * features.grid.height;
                let cells: Vec<usize> = (0..n).filter(|&c| features.layers.iter().any(|l| !l[c].is_nan())).collect();
                let mut pu = Table::new(vec!["id".into()]);
                pu.rows = (1..=cells.len()).map(|i| vec![i as f64]).collect();
                let mut puvspr = Table::new(vec!["species".into(), "pu".into(), "amount".into()]);
                for (i, &c) in cells.iter().enumerate() {
                    for (k, l) in features.layers.iter().enumerate() {
                        if l[c] > 0.0 {
                            puvspr.rows.push(vec![(k + 1) as f64, (i + 1) as f64, l[c]]);
                        }
                    }
                }
                let per = |name: &str| -> Result<Option<Vec<f64>>> {
                    match args.params.get(name) {
                        None | Some(serde_json::Value::Null) => Ok(None),
                        Some(serde_json::Value::Number(x)) => Ok(Some(vec![x.as_f64().unwrap(); nsp])),
                        Some(v) => {
                            let l: Vec<f64> =
                                serde_json::from_value(v.clone()).map_err(|e| err(format!("{name}: {e}")))?;
                            if l.len() != nsp {
                                return Err(err(format!("{name}: {} values for {nsp} features", l.len())));
                            }
                            Ok(Some(l))
                        }
                    }
                };
                let mut spec = Table::new(vec!["id".into()]);
                spec.rows = (1..=nsp).map(|k| vec![k as f64]).collect();
                match (per("prop")?, per("targets")?) {
                    (Some(p), _) => {
                        spec.columns.push("prop".into());
                        spec.rows.iter_mut().zip(p).for_each(|(r, v)| r.push(v));
                    }
                    (None, Some(t)) => {
                        spec.columns.push("target".into());
                        spec.rows.iter_mut().zip(t).for_each(|(r, v)| r.push(v));
                    }
                    (None, None) => return Err(err("give the targets: prop or targets".into())),
                }
                (pu, spec, puvspr, Some((cells, features.grid.clone())))
            }
        };
        let r = proxirr_tables(&pu, &spec, &puvspr, triage).map_err(err)?;
        let ids = pu.column("id").unwrap();
        let mut units = Table::new(vec!["pu".into(), "beta".into(), "gamma".into()]);
        units.rows = (0..ids.len()).map(|i| vec![ids[i], r.beta[i], r.gamma[i]]).collect();
        let mut alpha = Table::new(["species", "pu", "amount", "global", "target", "alpha"].map(String::from).to_vec());
        alpha.rows = puvspr
            .rows
            .iter()
            .enumerate()
            .map(|(i, row)| {
                let c = |name: &str| row[puvspr.columns.iter().position(|x| x == name).unwrap()];
                vec![c("species"), c("pu"), c("amount"), r.global[i], r.target[i], r.alpha[i]]
            })
            .collect();
        let mut targets = Table::new(vec!["feature".into(), "global".into(), "target".into()]);
        let sp_ids = spec.column("id").unwrap();
        targets.rows = (0..sp_ids.len()).map(|k| vec![sp_ids[k], r.spec_global[k], r.spec_target[k]]).collect();
        let mut out = outputs([
            ("units", Value::Table(units)),
            ("alpha", Value::Table(alpha)),
            ("targets", Value::Table(targets)),
        ]);
        if let Some((cells, grid)) = cells {
            let n = grid.width * grid.height;
            let (mut b, mut g) = (vec![f64::NAN; n], vec![f64::NAN; n]);
            for (i, &c) in cells.iter().enumerate() {
                b[c] = r.beta[i];
                g[c] = r.gamma[i];
            }
            out.insert("beta".into(), Value::Map(Map::new(grid.clone(), b)));
            out.insert("gamma".into(), Value::Map(Map::new(grid, g)));
        }
        Ok(out)
    }
}

/// The planning units of a node's inputs: the cells of a cost map with the features' layers, or the rows of a table
/// (`pu`) with a cost column and feature columns, or Marxan-style tables (`pu` with id and cost, `spec` with id,
/// `rij` with pu, species, amount).
fn units_of(args: &Args) -> Result<milp::Units> {
    let node = args.node;
    let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let table = |port: &str| -> Option<&Table> {
        match args.get(port) {
            Some(Value::Table(t)) => Some(t),
            _ => None,
        }
    };
    let Some(pu) = table("pu") else {
        let features = args.stack("features")?;
        let cost = args.map("cost")?;
        if (features.grid.width, features.grid.height) != (cost.grid.width, cost.grid.height) {
            return Err(err("the features are not on the cost's grid".into()));
        }
        let layers: Vec<&[f64]> = features.layers.iter().map(|l| l.as_slice()).collect();
        let names: Vec<String> = features.keys.iter().map(|k| k.to_string()).collect();
        return milp::Units::raster(
            &cost.data,
            &layers,
            names,
            (cost.grid.height, cost.grid.width),
            cost.grid.transform,
        )
        .map_err(err);
    };
    let cost_column: String = args.param("cost_column", "cost".to_string())?;
    let col = |t: &Table, c: &str, what: &str| t.column(c).ok_or_else(|| err(format!("{what} has no column {c:?}")));
    let cost = col(pu, &cost_column, "pu")?;
    match (table("spec"), table("rij")) {
        (Some(spec), Some(rij)) => {
            let ids = col(spec, "id", "spec")?;
            let names = ids.iter().map(|v| terraflux_ops_label(*v)).collect();
            milp::Units::rij(
                &col(pu, "id", "pu")?,
                &cost,
                &ids,
                names,
                (&col(rij, "pu", "rij")?, &col(rij, "species", "rij")?, &col(rij, "amount", "rij")?),
            )
            .map_err(err)
        }
        _ => {
            let names: Vec<String> = match args.params.get("features") {
                Some(serde_json::Value::String(s)) => vec![s.clone()],
                Some(v) => serde_json::from_value(v.clone())
                    .map_err(|e| err(format!("features: {e} (the names of pu's feature columns)")))?,
                None => return Err(err("give the names of pu's feature columns (features)".into())),
            };
            let cols = names.iter().map(|n| col(pu, n, "pu")).collect::<Result<Vec<_>>>()?;
            let refs: Vec<&[f64]> = cols.iter().map(|c| c.as_slice()).collect();
            milp::Units::table(&cost, &refs, names).map_err(err)
        }
    }
}

/// A number as a name (a whole number without decimals).
fn terraflux_ops_label(v: f64) -> String {
    crate::value::class_label(v)
}

/// A square matrix of every unit from a table of triplets (i, j, x; 1-based, R's sparseMatrix).
fn matrix_of(args: &Args, port: &str, n: usize) -> Result<Option<milp::Matrix>> {
    let Some(Value::Table(t)) = args.get(port) else { return Ok(None) };
    let col = |c: &str| {
        t.column(c).ok_or_else(|| Error::msg(format!("node {:?}: {port} needs the columns i, j, x", args.node)))
    };
    let (i, j, x) = (col("i")?, col("j")?, col("x")?);
    let index = |v: &[f64]| -> Result<Vec<usize>> {
        v.iter()
            .map(|&k| {
                if k >= 1.0 && k <= n as f64 && k.fract() == 0.0 {
                    Ok(k as usize - 1)
                } else {
                    Err(Error::msg(format!("node {:?}: {port}: {k} is no unit (1-{n})", args.node)))
                }
            })
            .collect()
    };
    Ok(Some(milp::Matrix { n, i: index(&i)?, j: index(&j)?, x }))
}

/// A problem of prioritizr from a node's inputs and settings.
pub fn problem_of(args: &Args) -> Result<milp::Problem> {
    let node = args.node;
    let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let units = units_of(args)?;
    let nf = units.names.len();
    let n = units.n();
    let budget = || -> Result<f64> {
        args.opt_param::<f64>("budget")?.ok_or_else(|| err("this objective needs a budget".into()))
    };
    let objective: String = args.param("objective", "min_set".to_string())?;
    let objective = match objective.as_str() {
        "min_set" => milp::Objective::MinSet,
        "min_shortfall" => milp::Objective::MinShortfall(budget()?),
        "min_largest_shortfall" => milp::Objective::MinLargestShortfall(budget()?),
        "max_utility" => milp::Objective::MaxUtility(budget()?),
        "max_features" => milp::Objective::MaxFeatures(budget()?),
        "max_cover" => milp::Objective::MaxCover(budget()?),
        "min_penalties" => milp::Objective::MinPenalties(budget()?),
        o => return Err(err(format!("unknown objective {o:?}"))),
    };
    let per_feature = |name: &str| -> Result<Option<Vec<f64>>> {
        match args.params.get(name) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::Number(x)) => Ok(Some(vec![x.as_f64().unwrap(); nf])),
            Some(v) => {
                let l: Vec<f64> = serde_json::from_value(v.clone()).map_err(|e| err(format!("{name}: {e}")))?;
                if l.len() != nf {
                    return Err(err(format!("{name}: {} values for {nf} features", l.len())));
                }
                Ok(Some(l))
            }
        }
    };
    let targets = if let Some(r) = per_feature("relative_targets")? {
        Some(milp::TargetSpec::Relative(r))
    } else if let Some(a) = per_feature("absolute_targets")? {
        Some(milp::TargetSpec::Absolute(a))
    } else if let Some(v) = args.params.get("loglinear_targets").filter(|v| !v.is_null()) {
        let l: Vec<Option<f64>> = serde_json::from_value(v.clone()).map_err(|e| {
            err(format!("loglinear_targets: {e} ([lower amount, lower target, upper amount, upper target, cap amount, cap target])"))
        })?;
        if l.len() != 4 && l.len() != 6 {
            return Err(err("loglinear_targets: 4 numbers, or 6 with the cap".into()));
        }
        let g = |i: usize| l[i].ok_or_else(|| err("loglinear_targets: the bounds are numbers".into()));
        Some(milp::TargetSpec::Loglinear {
            lo_amount: g(0)?,
            lo_target: g(1)?,
            hi_amount: g(2)?,
            hi_target: g(3)?,
            cap: if l.len() == 6 { l[4].zip(l[5]) } else { None },
        })
    } else if let Some(Value::Table(t)) = args.get("targets") {
        // a table of manual targets: feature (1..), relative (1) or absolute (0), sense (1 >=, -1 <=, 0 =), target
        let col = |c: &str| t.column(c).ok_or_else(|| err(format!("targets needs a column {c:?}")));
        let (f, rel, tg) = (col("feature")?, col("relative")?, col("target")?);
        let sense = t.column("sense").unwrap_or_else(|| vec![1.0; f.len()]);
        Some(milp::TargetSpec::Manual(
            (0..f.len())
                .map(|i| milp::ManualTarget {
                    feature: (f[i] as usize).saturating_sub(1),
                    relative: rel[i] != 0.0,
                    sense: match sense[i] {
                        s if s > 0.0 => ">=".into(),
                        s if s < 0.0 => "<=".into(),
                        _ => "=".into(),
                    },
                    target: tg[i],
                })
                .collect(),
        ))
    } else {
        None
    };
    let weights = per_feature("feature_weights")?;
    let decisions: String = args.param("decisions", "binary".to_string())?;
    let decisions = match decisions.as_str() {
        "binary" => milp::Decisions::Binary,
        "proportion" => milp::Decisions::Proportion,
        "semicontinuous" => milp::Decisions::Semicontinuous(args.param("upper_limit", 1.0)?),
        d => return Err(err(format!("decisions are binary, proportion or semicontinuous, not {d:?}"))),
    };
    // a value per cell (a map on the cost's grid) or per row (a column of pu)
    let pu = match args.get("pu") {
        Some(Value::Table(t)) => Some(t),
        _ => None,
    };
    let values = |port: &str, column: Option<&str>| -> Result<Vec<f64>> {
        if let (Some(t), Some(c)) = (pu, column) {
            return t.column(c).ok_or_else(|| err(format!("pu has no column {c:?}")));
        }
        let m = args.map(port)?;
        if m.data.len() != units.size {
            return Err(err(format!("{port} is not on the cost's grid")));
        }
        Ok(m.data.to_vec())
    };
    let locked = |port: &str| -> Result<Vec<bool>> {
        let column: Option<String> = args.opt_param(&format!("{port}_column"))?;
        if pu.is_some() && column.is_none() {
            return Ok(vec![false; n]);
        }
        if pu.is_none() && args.opt_map(port)?.is_none() {
            return Ok(vec![false; n]);
        }
        let v = values(port, column.as_deref())?;
        Ok(units.cells.iter().map(|&c| !v[c].is_nan() && v[c] != 0.0).collect())
    };
    let (locked_in, locked_out) = (locked("locked_in")?, locked("locked_out")?);
    // the spatial penalties and constraints: in the order of the list `spatial`, else of the settings
    let items: Vec<serde_json::Map<String, serde_json::Value>> = match args.params.get("spatial") {
        Some(v) if !v.is_null() => serde_json::from_value(v.clone())
            .map_err(|e| err(format!("spatial: {e} (a list of {{kind: ..., ...}})")))?,
        _ => {
            let mut l = Vec::new();
            let mut put = |kind: &str, keys: &[(&str, &str)]| {
                let mut m = serde_json::Map::new();
                m.insert("kind".into(), kind.into());
                for (from, to) in keys {
                    if let Some(v) = args.params.get(*from) {
                        m.insert(to.to_string(), v.clone());
                    }
                }
                l.push(m);
            };
            if args.params.contains_key("boundary_penalty") {
                put("boundary", &[("boundary_penalty", "penalty"), ("edge_factor", "edge_factor")]);
            }
            if args.params.contains_key("connectivity_penalty") {
                put("connectivity", &[("connectivity_penalty", "penalty")]);
            }
            if args.params.contains_key("asym_connectivity_penalty") {
                put("asym_connectivity", &[("asym_connectivity_penalty", "penalty")]);
            }
            if args.params.contains_key("linear_penalty") {
                put("linear_penalty", &[("linear_penalty", "penalty"), ("penalty_column", "column")]);
            }
            if args.params.contains_key("linear_threshold") {
                put(
                    "linear_constraint",
                    &[("linear_threshold", "threshold"), ("linear_sense", "sense"), ("constraint_column", "column")],
                );
            }
            if args.params.contains_key("neighbors") {
                put("neighbors", &[("neighbors", "k"), ("clamp", "clamp")]);
            }
            for kind in ["contiguity", "feature_contiguity"] {
                if args.params.get(kind).and_then(|v| v.as_bool()).unwrap_or(false) {
                    put(kind, &[]);
                }
            }
            l
        }
    };
    let mut spatial = Vec::new();
    for it in &items {
        let num = |k: &str| it.get(k).and_then(|v| v.as_f64());
        let need = |k: &str| num(k).ok_or_else(|| err(format!("spatial: {:?} needs {k:?}", it.get("kind"))));
        let column = it.get("column").and_then(|v| v.as_str());
        let kind = it.get("kind").and_then(|v| v.as_str()).unwrap_or("");
        spatial.push(match kind {
            "boundary" => milp::Spatial::Boundary {
                penalty: need("penalty")?,
                edge_factor: num("edge_factor").unwrap_or(0.5),
                data: matrix_of(args, "boundary_data", units.size)?,
            },
            "connectivity" => milp::Spatial::Connectivity {
                penalty: need("penalty")?,
                data: matrix_of(args, "connectivity_data", units.size)?
                    .ok_or_else(|| err("connectivity penalties need connectivity_data".into()))?,
            },
            "asym_connectivity" => milp::Spatial::AsymConnectivity {
                penalty: need("penalty")?,
                data: matrix_of(args, "asym_connectivity_data", units.size)?
                    .ok_or_else(|| err("asymmetric connectivity penalties need asym_connectivity_data".into()))?,
            },
            "linear_penalty" => {
                milp::Spatial::LinearPenalty { penalty: need("penalty")?, data: values("penalty_data", column)? }
            }
            "linear_constraint" => {
                let sense = it.get("sense").and_then(|v| v.as_str()).unwrap_or("<=").to_string();
                if !matches!(sense.as_str(), ">=" | "<=" | "=") {
                    return Err(err(format!("a linear constraint's sense is >=, <= or =, not {sense:?}")));
                }
                milp::Spatial::LinearConstraint {
                    threshold: need("threshold")?,
                    sense,
                    data: values("constraint_data", column)?,
                }
            }
            "neighbors" => milp::Spatial::Neighbors {
                k: need("k")?,
                clamp: it.get("clamp").and_then(|v| v.as_bool()).unwrap_or(true),
                data: matrix_of(args, "adjacency_data", units.size)?,
            },
            "contiguity" => milp::Spatial::Contiguity { data: matrix_of(args, "adjacency_data", units.size)? },
            "feature_contiguity" => {
                milp::Spatial::FeatureContiguity { data: matrix_of(args, "adjacency_data", units.size)? }
            }
            k => return Err(err(format!("unknown spatial penalty or constraint {k:?}"))),
        });
    }
    Ok(milp::Problem { units, objective, targets, weights, decisions, locked_in, locked_out, spatial })
}

struct Optimize;

impl Operator for Optimize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.optimize",
            "Systematic conservation planning as prioritizr 8.0.6 (Hanson et al.): an integer program over the \
             planning units (the cells with a cost) solved to optimality (within the gap) by HiGHS, the model \
             prioritizr compiles. Objectives: min_set (meet every target at the least cost), min_shortfall, \
             min_largest_shortfall, max_utility, max_features, max_cover, min_penalties (with a budget); \
             targets relative, absolute, loglinear or by hand; feature weights; binary, proportion or \
             semicontinuous decisions; locked units; boundary, connectivity and linear penalties, linear \
             constraints, neighbour and contiguity constraints; planning units in maps or tables; portfolios of solutions.",
            Access::Global,
        )
        .input(PortSpec::optional("features", Type::Stack, "the features' amounts, a layer each"))
        .input(PortSpec::optional("cost", Type::Map, "the cost of each cell (no value: not a planning unit)"))
        .input(PortSpec::optional(
            "pu",
            Type::Table,
            "tables instead of maps: the planning units, a row each (a cost column, the features' columns)",
        ))
        .input(PortSpec::optional("spec", Type::Table, "Marxan-style: the features (id)"))
        .input(PortSpec::optional("rij", Type::Table, "Marxan-style: the amounts (pu, species, amount)"))
        .input(PortSpec::optional("boundary_data", Type::Table, "a boundary matrix of every unit: i, j, x (1-based)"))
        .input(PortSpec::optional("connectivity_data", Type::Table, "a symmetric connectivity matrix: i, j, x"))
        .input(PortSpec::optional("asym_connectivity_data", Type::Table, "a connectivity matrix: i, j, x"))
        .input(PortSpec::optional("adjacency_data", Type::Table, "an adjacency matrix of every unit: i, j, x"))
        .input(PortSpec::optional("locked_in", Type::Map, "the units that must be selected (not 0)"))
        .input(PortSpec::optional("locked_out", Type::Map, "the units that may not be selected (not 0)"))
        .input(PortSpec::optional(
            "targets",
            Type::Table,
            "targets by hand: feature (1..), relative (1) or absolute (0), sense (1 >=, -1 <=, 0 =), target",
        ))
        .input(PortSpec::optional("penalty_data", Type::Map, "linear penalties: a value per cell"))
        .input(PortSpec::optional("constraint_data", Type::Map, "the linear constraint: a value per cell"))
        .output(PortSpec::optional("solution", Type::Map, "maps: the share of each unit selected (1 selected)"))
        .output(PortSpec::new(
            "selection",
            Type::Table,
            "per unit: its cell or row (1..) and the share selected (a portfolio: solution_1.., frequency)",
        ))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "the solution's objective, cost, units selected, the solver's gap and status (1 optimal); a row per \
             solution of a portfolio",
        ))
        .output(PortSpec::optional("solutions", Type::Stack, "maps, a portfolio: every solution, a layer each"))
        .output(PortSpec::optional("frequency", Type::Map, "maps, a portfolio: the share of the solutions selecting a unit"))
        .output(PortSpec::new(
            "coverage",
            Type::Table,
            "per target: its feature, the amount held, the target, the feature's total, and whether it is met \
             (of the first solution)",
        ))
        .param(PortSpec::optional(
            "objective",
            Type::Text,
            "min_set (default), min_shortfall, min_largest_shortfall, max_utility, max_features, max_cover, min_penalties",
        ))
        .param(PortSpec::optional("budget", Type::Number, "the objectives with a budget: the cost they may spend"))
        .param(PortSpec::optional("relative_targets", Type::Any, "a share of each feature's total (or one for all)"))
        .param(PortSpec::optional("absolute_targets", Type::Any, "an amount per feature"))
        .param(PortSpec::optional(
            "loglinear_targets",
            Type::Any,
            "[lower amount, lower target, upper amount, upper target(, cap amount, cap target)]",
        ))
        .param(PortSpec::optional("feature_weights", Type::Any, "a weight per feature"))
        .param(PortSpec::optional("decisions", Type::Text, "binary (default), proportion or semicontinuous"))
        .param(PortSpec::optional("upper_limit", Type::Number, "semicontinuous: the most of a unit selected"))
        .param(PortSpec::optional(
            "boundary_penalty",
            Type::Number,
            "boundary penalties: the weight of the selection's boundary (its exposed edges times edge_factor)",
        ))
        .param(PortSpec::optional("edge_factor", Type::Number, "boundary penalties: the weight of edges to no unit (0.5)"))
        .param(PortSpec::optional("linear_penalty", Type::Number, "linear penalties: the weight of penalty_data"))
        .param(PortSpec::optional("linear_threshold", Type::Number, "a linear constraint on constraint_data"))
        .param(PortSpec::optional("linear_sense", Type::Text, "the linear constraint's sense: <= (default), >=, ="))
        .param(PortSpec::optional("neighbors", Type::Number, "every selected unit has this many selected neighbours"))
        .param(PortSpec::optional("clamp", Type::Bool, "neighbors: fewer for the units with fewer (default true)"))
        .param(PortSpec::optional(
            "contiguity",
            Type::Bool,
            "the selected units form one patch (adjacent: the raster's 4 neighbours, or adjacency_data)",
        ))
        .param(PortSpec::optional("feature_contiguity", Type::Bool, "every feature's units form one patch"))
        .param(PortSpec::optional("cost_column", Type::Text, "tables: pu's cost column (default cost)"))
        .param(PortSpec::optional("features", Type::Any, "tables: pu's feature columns"))
        .param(PortSpec::optional("locked_in_column", Type::Text, "tables: pu's column of the units locked in"))
        .param(PortSpec::optional("locked_out_column", Type::Text, "tables: pu's column of the units locked out"))
        .param(PortSpec::optional("connectivity_penalty", Type::Number, "connectivity penalties' weight"))
        .param(PortSpec::optional("asym_connectivity_penalty", Type::Number, "asymmetric connectivity's weight"))
        .param(PortSpec::optional("penalty_column", Type::Text, "tables: pu's column of the linear penalties"))
        .param(PortSpec::optional("constraint_column", Type::Text, "tables: pu's column of the linear constraint"))
        .param(PortSpec::optional(
            "spatial",
            Type::Any,
            "penalties and constraints in order: [{kind: boundary|connectivity|asym_connectivity|linear_penalty|\
             linear_constraint|neighbors|contiguity|feature_contiguity, penalty, edge_factor, threshold, sense, \
             column, k, clamp}, ...]",
        ))
        .param(PortSpec::optional(
            "portfolio",
            Type::Text,
            "several solutions: cuts (the best distinct ones), top (the best, each to optimality), gap (within \
             pool_gap of the optimum), shuffle (the columns in random orders)",
        ))
        .param(PortSpec::optional("number_solutions", Type::Number, "a portfolio's solutions (default 10)"))
        .param(PortSpec::optional("pool_gap", Type::Number, "the gap portfolio: how far from the optimum (0.1)"))
        .param(PortSpec::optional("seed", Type::Number, "the shuffle portfolio's random orders (default 0)"))
        .param(PortSpec::optional("remove_duplicates", Type::Bool, "the shuffle portfolio: distinct solutions (true)"))
        .param(PortSpec::optional("gap", Type::Number, "the relative optimality gap (default 0.1, as prioritizr)"))
        .param(PortSpec::optional("time_limit", Type::Number, "seconds (default none)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let p = problem_of(args)?;
        let (model, t) = milp::compile(&p).map_err(err)?;
        let gap: f64 = args.param("gap", 0.1)?;
        let time_limit: f64 = args.opt_param("time_limit")?.unwrap_or(f64::INFINITY);
        let n = p.units.n();
        let portfolio = portfolio_of(args)?;
        let rs = match &portfolio {
            Some(pf) => milp::solve_portfolio(&model, n, pf, gap, time_limit).map_err(err)?,
            None => {
                let r = milp::solve(&model, n, gap, time_limit).map_err(err)?;
                if !r.has_solution {
                    return Err(err(format!("no solution: {}", r.status)));
                }
                vec![r]
            }
        };
        let xs: Vec<&[f64]> = rs.iter().map(|r| &r.x[..n]).collect();
        let x = xs[0];
        let cost = &p.units.cost;
        let names: Vec<String> = match portfolio {
            Some(_) => (1..=xs.len()).map(|k| format!("solution_{k}")).collect(),
            None => vec!["solution".into()],
        };
        let frequency: Vec<f64> = (0..n).map(|i| xs.iter().map(|x| x[i]).sum::<f64>() / xs.len() as f64).collect();
        let mut columns = vec!["unit".to_string()];
        columns.extend(names.iter().cloned());
        if portfolio.is_some() {
            columns.push("frequency".into());
        }
        let mut selection = Table::new(columns);
        selection.rows = (0..n)
            .map(|i| {
                let mut row = vec![(p.units.cells[i] + 1) as f64];
                row.extend(xs.iter().map(|x| x[i]));
                if portfolio.is_some() {
                    row.push(frequency[i]);
                }
                row
            })
            .collect();
        let mut columns: Vec<String> = ["objective", "cost", "selected", "gap", "optimal"].map(String::from).to_vec();
        if portfolio.is_some() {
            columns.insert(0, "solution".into());
        }
        let mut summary = Table::new(columns);
        for (k, (r, x)) in rs.iter().zip(&xs).enumerate() {
            let spent: f64 = cost.iter().zip(x.iter()).map(|(c, x)| c * x).sum();
            let mut row = vec![
                r.objective,
                spent,
                x.iter().filter(|&&v| v > 0.5).count() as f64,
                r.gap,
                f64::from(u8::from(r.status == "optimal")),
            ];
            if portfolio.is_some() {
                row.insert(0, (k + 1) as f64);
            }
            summary.rows.push(row);
        }
        let mut coverage = Table::new(["feature", "held", "target", "total", "met"].map(String::from).to_vec());
        for (k, &f) in t.feature.iter().enumerate() {
            let (idx, v) = p.units.rij.row(f);
            let held: f64 = idx.iter().zip(v).map(|(&i, a)| a * x[i]).sum();
            let tol = 1e-6 * t.value[k].abs().max(1.0);
            let met = match t.sense[k].as_str() {
                ">=" => held >= t.value[k] - tol,
                "<=" => held <= t.value[k] + tol,
                "=" => (held - t.value[k]).abs() <= tol,
                _ => true,
            };
            coverage.rows.push(vec![(f + 1) as f64, held, t.value[k], t.total[k], f64::from(u8::from(met))]);
        }
        let mut out = outputs([
            ("selection", Value::Table(selection)),
            ("summary", Value::Table(summary)),
            ("coverage", Value::Table(coverage)),
        ]);
        if p.units.raster {
            let grid = args.map("cost")?.grid.clone();
            let on_grid = |v: &[f64]| {
                let mut out = vec![f64::NAN; grid.width * grid.height];
                for (i, &c) in p.units.cells.iter().enumerate() {
                    out[c] = v[i];
                }
                out
            };
            out.insert("solution".into(), Value::Map(Map::new(grid.clone(), on_grid(x))));
            if portfolio.is_some() {
                let keys = names.iter().map(|n| Key::Name(n.clone())).collect();
                let layers = xs.iter().map(|x| Arc::new(on_grid(x))).collect();
                out.insert("frequency".into(), Value::Map(Map::new(grid.clone(), on_grid(&frequency))));
                out.insert("solutions".into(), Value::Stack(Stack { grid, keys, layers }));
            }
        }
        Ok(out)
    }
}

/// The portfolio asked for: portfolio, number_solutions, pool_gap, seed, remove_duplicates.
fn portfolio_of(args: &Args) -> Result<Option<milp::Portfolio>> {
    let node = args.node;
    let Some(kind) = args.opt_param::<String>("portfolio")? else { return Ok(None) };
    let k: f64 = args.param("number_solutions", 10.0)?;
    if k < 1.0 || k.fract() != 0.0 {
        return Err(Error::msg(format!("node {node:?}: number_solutions must be a whole number >= 1")));
    }
    let k = k as usize;
    Ok(Some(match kind.as_str() {
        "cuts" => milp::Portfolio::Cuts(k),
        "top" => milp::Portfolio::Top(k),
        "gap" => {
            let g: f64 = args.param("pool_gap", 0.1)?;
            if !(g >= 0.0 && g.is_finite()) {
                return Err(Error::msg(format!("node {node:?}: pool_gap must be >= 0")));
            }
            milp::Portfolio::Gap(k, g)
        }
        "shuffle" => milp::Portfolio::Shuffle {
            k,
            seed: args.param::<f64>("seed", 0.0)? as u64,
            remove_duplicates: args.param("remove_duplicates", true)?,
        },
        o => return Err(Error::msg(format!("node {node:?}: unknown portfolio {o:?} (cuts, top, gap, shuffle)"))),
    }))
}

/// A solution's value per planning unit (no value: 0): the solution map on the cost's grid, or the selection
/// table (prioritize.optimize's: unit and solution; or a column solution, solution_1 or the only one, with a row
/// per unit of the table or per planning unit).
fn solution_of(args: &Args, u: &milp::Units) -> Result<Vec<f64>> {
    let node = args.node;
    let err = |m: &str| Error::msg(format!("node {node:?}: {m}"));
    let finite = |v: f64| if v.is_finite() { v } else { 0.0 };
    if let Some(Value::Table(t)) = args.get("selection") {
        let name = ["solution", "solution_1"]
            .into_iter()
            .find(|c| t.columns.iter().any(|x| x == c))
            .map(String::from)
            .or_else(|| {
                let rest: Vec<&String> = t.columns.iter().filter(|c| *c != "unit").collect();
                (rest.len() == 1).then(|| rest[0].clone())
            })
            .ok_or_else(|| err("the selection needs a column solution"))?;
        let col = t.column(&name).unwrap();
        if let Some(unit) = t.column("unit") {
            let mut at = vec![0.0; u.size];
            for (&k, &v) in unit.iter().zip(&col) {
                if !(k >= 1.0 && k <= u.size as f64 && k.fract() == 0.0) {
                    return Err(err(&format!("the selection's unit {k} is no unit (1-{})", u.size)));
                }
                at[k as usize - 1] = finite(v);
            }
            return Ok(u.cells.iter().map(|&c| at[c]).collect());
        }
        return if col.len() == u.size {
            Ok(u.cells.iter().map(|&c| finite(col[c])).collect())
        } else if col.len() == u.n() {
            Ok(col.iter().map(|&v| finite(v)).collect())
        } else {
            Err(err(&format!("the selection needs a row per unit ({} or {}), not {}", u.size, u.n(), col.len())))
        };
    }
    let Some(Value::Map(sol)) = args.get("solution") else {
        return Err(err("the solution: a map (solution) or a table (selection)"));
    };
    if !u.raster || sol.data.len() != u.ny * u.nx {
        return Err(err("the solution map is not on the cost's grid"));
    }
    Ok(u.cells.iter().map(|&c| finite(sol.data[c])).collect())
}

struct Evaluate;

impl Operator for Evaluate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.evaluate",
            "prioritizr's evaluation of a solution (eval_*), its numbers bit for bit: the units selected, their \
             cost and boundary; how much of each feature it holds; with targets, their coverage and Ferrier et \
             al.'s irreplaceability of the units selected; the rarity-weighted richness of the units selected. The \
             problem is given as to prioritize.optimize (maps or tables), the solution as a map or as \
             prioritize.optimize's selection; the connectivity of the solution with connectivity matrices.",
            Access::Global,
        )
        .input(PortSpec::optional("features", Type::Stack, "the features' amounts, a layer each"))
        .input(PortSpec::optional("cost", Type::Map, "the cost of each cell (no value: not a planning unit)"))
        .input(PortSpec::optional("pu", Type::Table, "tables instead of maps: as prioritize.optimize's"))
        .input(PortSpec::optional("spec", Type::Table, "Marxan-style: the features (id)"))
        .input(PortSpec::optional("rij", Type::Table, "Marxan-style: the amounts (pu, species, amount)"))
        .input(PortSpec::optional("solution", Type::Map, "the share of each unit selected"))
        .input(PortSpec::optional(
            "selection",
            Type::Table,
            "the solution as a table: prioritize.optimize's selection (unit, solution), or a column (solution, \
             solution_1 or the only one) with a row per unit",
        ))
        .input(PortSpec::optional("targets", Type::Table, "targets by hand, as prioritize.optimize's"))
        .input(PortSpec::optional("boundary_data", Type::Table, "a boundary matrix of every unit: i, j, x (1-based)"))
        .input(PortSpec::optional("connectivity_data", Type::Table, "a symmetric connectivity matrix: i, j, x"))
        .input(PortSpec::optional("asym_connectivity_data", Type::Table, "a connectivity matrix: i, j, x"))
        .output(PortSpec::new(
            "summary",
            Type::Table,
            "the units selected (n), their cost and boundary (no value: tables without a boundary matrix), and \
             with the matrices their connectivity and asym_connectivity",
        ))
        .output(PortSpec::new("representation", Type::Table, "per feature: its total, the amount held and its share"))
        .output(PortSpec::optional(
            "coverage",
            Type::Table,
            "per target: its feature, met (1), the total, the target, the amount held and the shortfall, and \
             these relative to the total",
        ))
        .output(PortSpec::new(
            "units",
            Type::Table,
            "per unit: its cell or row (1..), the solution, the rarity-weighted richness, Ferrier's (ferrier_<feature>, \
             ferrier_total), the replacement cost and the rank importance when asked",
        ))
        .output(PortSpec::optional("ferrier", Type::Stack, "maps: Ferrier's irreplaceability per feature and in total"))
        .output(PortSpec::optional("rarity", Type::Map, "maps: the rarity-weighted richness of the units selected"))
        .output(PortSpec::optional("replacement", Type::Map, "maps: each selected unit's replacement cost"))
        .output(PortSpec::optional("rank", Type::Map, "maps: each selected unit's rank importance"))
        .output(PortSpec::optional("rank_budgets", Type::Table, "rank: the budgets and the objective at each"))
        .param(PortSpec::optional("relative_targets", Type::Any, "a share of each feature's total (or one for all)"))
        .param(PortSpec::optional("absolute_targets", Type::Any, "an amount per feature"))
        .param(PortSpec::optional("loglinear_targets", Type::Any, "as prioritize.optimize's"))
        .param(PortSpec::optional("edge_factor", Type::Number, "the boundary: the weight of edges to no unit (0.5)"))
        .param(PortSpec::optional("rescale", Type::Bool, "the importances rescaled to 0.01-1 (default true)"))
        .param(PortSpec::optional(
            "replacement",
            Type::Bool,
            "the replacement cost of every unit selected (a solve without each)",
        ))
        .param(PortSpec::optional("rank", Type::Integer, "the rank importance with this many budgets"))
        .param(PortSpec::optional("budgets", Type::Any, "the rank importance at these budgets"))
        .param(PortSpec::optional("objective", Type::Text, "the problem's objective (as prioritize.optimize's)"))
        .param(PortSpec::optional("budget", Type::Number, "the objective's budget"))
        .param(PortSpec::optional("gap", Type::Number, "the solves' optimality gap (default 0.1)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let p = problem_of(args)?;
        let u = &p.units;
        let s = solution_of(args, u)?;
        let bm = matrix_of(args, "boundary_data", u.size)?;
        let e = milp::evaluate(u, &s, args.param("edge_factor", 0.5)?, bm.as_ref()).map_err(err)?;
        let mut columns = vec!["n".to_string(), "cost".into(), "boundary".into()];
        let mut row = vec![e.n, e.cost, e.boundary.unwrap_or(f64::NAN)];
        for (port, asym, name) in
            [("connectivity_data", false, "connectivity"), ("asym_connectivity_data", true, "asym_connectivity")]
        {
            if let Some(m) = matrix_of(args, port, u.size)? {
                let (v, symmetric) = milp::connectivity(u, &m, &s, asym).map_err(err)?;
                if symmetric == asym {
                    let (is, other) =
                        if asym { ("symmetric", "connectivity") } else { ("asymmetric", "asym_connectivity") };
                    ctx.warn(format!("{port} holds {is} values: the {other} summary suits them"));
                }
                columns.push(name.into());
                row.push(v);
            }
        }
        let mut summary = Table::new(columns);
        summary.rows.push(row);
        let mut rep =
            Table::new(["feature", "total_amount", "absolute_held", "relative_held"].map(String::from).to_vec());
        rep.rows =
            (0..u.names.len()).map(|f| vec![(f + 1) as f64, u.totals[f], e.held[f], e.held[f] / u.totals[f]]).collect();
        let rescale: bool = args.param("rescale", true)?;
        // per unit: a column each, and with maps a map each
        let mut per_unit: Vec<(String, Vec<f64>)> =
            vec![("solution".into(), s.clone()), ("rarity".into(), milp::rare_richness(u, &s, rescale).map_err(err)?)];
        let mut out = outputs([("summary", Value::Table(summary)), ("representation", Value::Table(rep))]);
        let gap: f64 = args.param("gap", 0.1)?;
        if args.param("replacement", false)? {
            per_unit.push(("replacement".into(), milp::replacement_importance(&p, &s, gap, rescale).map_err(err)?));
        }
        let n_rank: Option<usize> = args.opt_param("rank")?;
        let budgets: Option<Vec<f64>> = args.opt_param("budgets")?;
        if n_rank.is_some() || budgets.is_some() {
            let (rs, info) = milp::rank_importance(&p, &s, n_rank, budgets, gap, rescale).map_err(err)?;
            per_unit.push(("rank".into(), rs));
            let mut t = Table::new(vec!["budget".into(), "objective".into()]);
            t.rows = info.into_iter().map(|(b, o)| vec![b, o]).collect();
            out.insert("rank_budgets".into(), Value::Table(t));
        }
        let mut ferrier = None;
        if let Some(spec) = &p.targets {
            let t = milp::resolve_targets(spec, &u.totals).map_err(err)?;
            let held = milp::targets_held(u, &t, &s);
            let mut cov = Table::new(
                [
                    "feature",
                    "met",
                    "total_amount",
                    "absolute_target",
                    "absolute_held",
                    "absolute_shortfall",
                    "relative_target",
                    "relative_held",
                    "relative_shortfall",
                ]
                .map(String::from)
                .to_vec(),
            );
            for (k, &h) in held.iter().enumerate() {
                let (target, total) = (t.value[k], t.total[k]);
                let mut short = (target - h).abs();
                match t.sense[k].as_str() {
                    ">=" if h >= target => short = 0.0,
                    "=" if h - target < 1e-10 => short = 0.0,
                    "<=" if h <= target => short = 0.0,
                    _ => {}
                }
                let rel = |v: f64| if (v / total).is_finite() { v / total } else { 0.0 };
                cov.rows.push(vec![
                    (t.feature[k] + 1) as f64,
                    f64::from(u8::from(short < 1e-10)),
                    total,
                    target,
                    h,
                    short,
                    rel(target),
                    rel(h),
                    rel(short),
                ]);
            }
            out.insert("coverage".into(), Value::Table(cov));
            if let Ok((scores, totals)) = milp::ferrier(u, &t, &s) {
                for (name, v) in u.names.iter().zip(&scores) {
                    per_unit.push((format!("ferrier_{name}"), v.clone()));
                }
                per_unit.push(("ferrier_total".into(), totals.clone()));
                ferrier = Some((scores, totals));
            }
        }
        let mut units =
            Table::new(std::iter::once("unit".to_string()).chain(per_unit.iter().map(|c| c.0.clone())).collect());
        units.rows = (0..u.n())
            .map(|i| std::iter::once((u.cells[i] + 1) as f64).chain(per_unit.iter().map(|c| c.1[i])).collect())
            .collect();
        out.insert("units".into(), Value::Table(units));
        if u.raster {
            let grid = args.map("cost")?.grid.clone();
            let on_grid = |v: &[f64]| {
                let mut m = vec![f64::NAN; u.ny * u.nx];
                for (i, &c) in u.cells.iter().enumerate() {
                    m[c] = v[i];
                }
                m
            };
            for (name, v) in &per_unit {
                if ["rarity", "replacement", "rank"].contains(&name.as_str()) {
                    out.insert(name.clone(), Value::Map(Map::new(grid.clone(), on_grid(v))));
                }
            }
            if let Some((scores, totals)) = ferrier {
                let mut keys: Vec<Key> = u.names.iter().map(|n| Key::Name(n.clone())).collect();
                keys.push(Key::Name("total".into()));
                let mut layers: Vec<Arc<Vec<f64>>> = scores.iter().map(|v| Arc::new(on_grid(v))).collect();
                layers.push(Arc::new(on_grid(&totals)));
                out.insert("ferrier".into(), Value::Stack(Stack { grid, keys, layers }));
            }
        }
        Ok(out)
    }
}
