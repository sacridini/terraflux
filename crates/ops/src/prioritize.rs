//! Spatial prioritization from landschaft's C++: Zonation 5's ranking of the cells (`prioritize.zonation`), the
//! same rank map as the z5 binary.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Map, Operator, Outputs, Registry, Result, Table, Value, outputs};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Zonation));
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
