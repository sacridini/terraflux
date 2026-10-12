//! Spatial prioritization from landschaft's C++: Zonation 5's ranking of the cells (`prioritize.zonation`), the
//! same rank map as the z5 binary; Marxan 4's reserves (`prioritize.marxan`), the same runs as the Marxan
//! program; ProxIRR's irreplaceability (`prioritize.proxirr`), the bits of R's proxirr.

use std::collections::BTreeMap;
use std::sync::Arc;

use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_ls_sys::ffi;

use crate::{Args, Ctx, Error, Key, Map, Operator, Outputs, Registry, Result, Stack, Table, Value, outputs};
use crate::{milp, zonation};

pub fn register(r: &mut Registry) {
    r.add(Arc::new(Zonation));
    r.add(Arc::new(Marxan));
    r.add(Arc::new(Proxirr));
    r.add(Arc::new(Optimize));
    r.add(Arc::new(Evaluate));
    r.add(Arc::new(Calibrate));
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
        .input(PortSpec::optional(
            "condition_maps",
            Type::Stack,
            "the condition of the features' habitat (0-1), a layer each, named in condition",
        ))
        .input(PortSpec::optional("retention_maps", Type::Stack, "the maps of retention, named in retention"))
        .input(PortSpec::optional("resources", Type::Stack, "the interactions' resources, named in interaction"))
        .input(PortSpec::optional("areas", Type::Map, "area numbers (0: none), for the statistics of areas"))
        .input(PortSpec::optional("compare", Type::Map, "another solution, for the comparison"))
        .input(PortSpec::optional(
            "impact_maps",
            Type::Stack,
            "the impact scenarios' maps (sources, subareas, losses, pressures), named in them",
        ))
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
        .output(PortSpec::optional(
            "preprocessing",
            Type::Stack,
            "with preprocessing: richness and range-size rarity of the features of positive, negative and zero \
             weight and of every group (weighted too with weights)",
        ))
        .output(PortSpec::optional("transformed", Type::Stack, "with transformed: the features as ranked"))
        .output(PortSpec::optional(
            "area_statistics",
            Type::Table,
            "per set of features (weight class 1, -1, 0, or group) and area: size, mean rank, the features' sum \
             (ds) and weighted (wds), relative to the average (rfd, wfd), cost",
        ))
        .output(PortSpec::optional("coverage", Type::Table, "per set and area: the share of every feature in it"))
        .output(PortSpec::optional("area_mask", Type::Map, "the areas of a range of ranks (1) or its separate areas"))
        .output(PortSpec::optional(
            "comparison",
            Type::Table,
            "the cells chosen in both solutions, only in this, only in the other, in neither",
        ))
        .output(PortSpec::optional(
            "comparison_map",
            Type::Map,
            "1 both, 2 only this, 3 only the other, 4 neither (0 no data)",
        ))
        .output(PortSpec::optional(
            "histogram",
            Type::Table,
            "per set of features and bin of coverage: the count of features",
        ))
        .output(PortSpec::optional(
            "impact_zones",
            Type::Table,
            "per subarea (0 all), impact group, source, zone (0 the footprint), feature and bin (-1 all): the \
             feature's distribution in the zone, its loss and cells",
        ))
        .output(PortSpec::optional(
            "impact_cumulative",
            Type::Table,
            "per subarea, impact group, phase (1 direct habitat loss, 2 cumulative pressures) and feature: its \
             distribution, loss and cells",
        ))
        .output(PortSpec::optional("impact_maps", Type::Stack, "the impact of every group (ig<g>_zones...)"))
        .output(PortSpec::optional(
            "flexibility",
            Type::Stack,
            "how far each cell could move up, down and both in the order (shares of the cells)",
        ))
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
            "condition",
            Type::Any,
            "per feature: the layer of condition_maps its values are multiplied by (null: none; Zonation's -c)",
        ))
        .param(PortSpec::optional(
            "renormalize",
            Type::Any,
            "per feature (or all): a conditioned feature back to level 1",
        ))
        .param(PortSpec::optional(
            "retention",
            Type::Any,
            "per feature (-r): null, the fraction retained everywhere, a layer of retention_maps (fractions per \
             cell) or {absolute: layer} (amounts per cell)",
        ))
        .param(PortSpec::optional(
            "connectivity",
            Type::Any,
            "per feature (-d): null or a kernel {shape: uniform, triangular, negexp or gaussian, distance (cells), \
             cut}: the feature smoothed by it",
        ))
        .param(PortSpec::optional(
            "self_multiply",
            Type::Any,
            "per feature (or all): the smoothed feature times itself",
        ))
        .param(PortSpec::optional(
            "interaction",
            Type::Any,
            "per feature (-i): null or {kernel, resource (a layer of resources), positive, beta, normalize}",
        ))
        .param(PortSpec::optional("interaction2", Type::Any, "per feature (-I): interactions after the matrices"))
        .param(PortSpec::optional(
            "matrix_connectivity",
            Type::Any,
            "per feature (-m): null or {kernel, matrix}: the features of a matrix smoothed and mixed by it",
        ))
        .param(PortSpec::optional(
            "matrices",
            Type::Any,
            "{matrix: rows} (a row per source feature, a column per target, in the features' order)",
        ))
        .param(PortSpec::optional("groups", Type::Any, "an output group per feature (-1 none)"))
        .param(PortSpec::optional("preprocessing", Type::Bool, "Zonation's pre-processing maps (-b)"))
        .param(PortSpec::optional("impact_groups", Type::Any, "an impact group per feature (-1 none)"))
        .param(PortSpec::optional(
            "impact_bins",
            Type::Any,
            "per feature: null, 0 (ten bins) or the limits of the value classes its impacts are reported in",
        ))
        .param(PortSpec::optional(
            "impact",
            Type::Any,
            "an impact scenario (-s): {sources (a layer of impact_maps), impacts: [{group, source, footprint (%), \
             buffers: [[radius, %]...]}], subareas}",
        ))
        .param(PortSpec::optional(
            "cumulative_impact",
            Type::Any,
            "a cumulative scenario (-e): {direct (sum, product), direct_rasters, direct_matrix, cumulative \
             (variant1, variant2), pressures: [{raster, normalize, offset, divisor}], cumulative_matrix, \
             aggregation (add, multiply, max), functions: [{group, pressure, points}], subareas}",
        ))
        .param(PortSpec::optional(
            "area_statistics",
            Type::Any,
            "statistics of areas: \"areas\" (the map areas, the default with it), [min_rank, max_rank] or \
             [min_rank, max_rank, min_size] (its separate 8-connected areas)",
        ))
        .param(PortSpec::optional(
            "comparison",
            Type::Any,
            "[min_rank, max_rank, min_value, max_value]: the cells ranked so against those of compare valued so",
        ))
        .param(PortSpec::optional("histogram", Type::Any, "[min_rank, max_rank, bins]: the features' coverage"))
        .param(PortSpec::optional(
            "flexibility",
            Type::Number,
            "the flexibility map's max error (%): how far each cell could move before its loss differs by it",
        ))
        .param(PortSpec::optional("transformed", Type::Any, "per feature (or all): the feature as ranked (-f)"))
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
        let mut data =
            ffi::prio_data_new(&pick(&nodata), &pick(&extract), &pick(&thresholds), &max_values, &keep_zeros, bits)?;
        // conditions: a plane per distinct map, its cells without a value 0
        let condition: Vec<Option<String>> = per_feature(args, "condition", l0)?;
        let renormalize: Vec<bool> = per_layer(args, "renormalize", l0, false)?;
        let mut cond_names: Vec<&str> = Vec::new();
        let mut cond_index = vec![-1i32; l0];
        for (i, c) in condition.iter().enumerate() {
            if let Some(c) = c {
                let at = cond_names.iter().position(|n| n == c).unwrap_or_else(|| {
                    cond_names.push(c);
                    cond_names.len() - 1
                });
                cond_index[i] = at as i32;
            }
        }
        let mut conditions: Vec<f32> = Vec::new();
        for c in &cond_names {
            for v in named_map(args, "condition_maps", c, (ny, nx)).map_err(err)? {
                let v = if v.is_nan() { 0.0 } else { v as f32 };
                if !(0.0..=1.0).contains(&v) {
                    return Err(err("condition maps must have values in [0, 1]".into()));
                }
                conditions.push(v);
            }
        }
        if !cond_names.is_empty() {
            let flags: Vec<u8> = kept.iter().map(|&i| u8::from(renormalize[i])).collect();
            let index: Vec<i32> = kept.iter().map(|&i| cond_index[i]).collect();
            ffi::prio_data_set_conditions(data.pin_mut(), &index, &flags)?;
        }
        ffi::prio_data_add(data.pin_mut(), &kept_values, ny as i64, nx as i64, &area, &conditions, block_rows)?;
        let cells = ffi::prio_data_cells(&data);
        // the connectivity transforms, then retention
        let conn = zonation::Connectivity {
            kernel: per_feature(args, "connectivity", l0)?,
            self_multiply: per_layer(args, "self_multiply", l0, false)?,
            interaction: per_feature(args, "interaction", l0)?,
            interaction2: per_feature(args, "interaction2", l0)?,
            matrix: per_feature(args, "matrix_connectivity", l0)?,
            matrices: args.param("matrices", BTreeMap::new())?,
        };
        conn.check().map_err(err)?;
        if conn.any() {
            let resource = |name: &str| named_map(args, "resources", name, (ny, nx));
            zonation::connect(&mut data, &kept, &cells, (ny, nx), &conn, &resource).map_err(err)?;
        }
        let retention: Vec<Option<zonation::Retention>> = per_feature(args, "retention", l0)?;
        let retained = |name: &str| named_map(args, "retention_maps", name, (ny, nx));
        zonation::retain(&mut data, &kept, &cells, &retention, &retained).map_err(err)?;
        // the impact scenarios: the features of the impact groups lose what they take
        let impact: Option<zonation::ImpactScenario> = args.opt_param("impact")?;
        let cumulative: Option<zonation::CumulativeScenario> = args.opt_param("cumulative_impact")?;
        let mut impacts = None;
        if impact.is_some() || cumulative.is_some() {
            let groups: Vec<i64> = per_layer(args, "impact_groups", l0, -1i64)?;
            let bins: Vec<serde_json::Value> = per_feature(args, "impact_bins", l0)?
                .into_iter()
                .map(|b: Option<serde_json::Value>| b.unwrap_or(serde_json::Value::Null))
                .collect();
            let maps = |name: &str| named_map(args, "impact_maps", name, (ny, nx));
            impacts = Some(
                zonation::impacts(
                    &mut data,
                    &kept,
                    &cells,
                    (ny, nx),
                    &groups,
                    &bins,
                    impact.as_ref(),
                    cumulative.as_ref(),
                    &maps,
                )
                .map_err(err)?,
            );
        }
        let r = ffi::prio_rank(
            &data,
            &settings,
            ny as i64,
            nx as i64,
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
        let mut out = outputs([
            ("rank", Value::Map(Map::new(stack.grid.clone(), rank))),
            ("curves", Value::Table(curves)),
            ("features", Value::Table(features)),
            ("auc", Value::Number(r.auc)),
            ("iterations", Value::Number(f64::from(r.iterations))),
        ]);
        // the maps per data cell on the grid
        let on_map = |v: &[f32]| -> Arc<Vec<f64>> {
            let mut m = vec![f64::NAN; n];
            for (&c, &x) in cells.iter().zip(v) {
                m[c as usize] = f64::from(x);
            }
            Arc::new(m)
        };
        if let Some(imp) = impacts {
            if impact.is_some() {
                let mut t = Table::new(
                    ["area", "group", "source", "zone", "layer", "bin", "distribution", "loss", "cells"]
                        .map(String::from)
                        .to_vec(),
                );
                t.rows = imp.zones.iter().map(|r| r.to_vec()).collect();
                out.insert("impact_zones".into(), Value::Table(t));
            }
            if cumulative.is_some() {
                let mut t = Table::new(
                    ["area", "group", "phase", "layer", "distribution", "loss", "cells"].map(String::from).to_vec(),
                );
                t.rows = imp.cumulative.iter().map(|r| r.to_vec()).collect();
                out.insert("impact_cumulative".into(), Value::Table(t));
            }
            if !imp.maps.is_empty() {
                let layer = |v: &[f32]| Arc::new(v.iter().map(|x| f64::from(*x)).collect::<Vec<_>>());
                out.insert(
                    "impact_maps".into(),
                    Value::Stack(Stack {
                        grid: stack.grid.clone(),
                        keys: imp.maps.iter().map(|(k, _)| Key::Name(k.clone())).collect(),
                        layers: imp.maps.iter().map(|(_, v)| layer(v)).collect(),
                    }),
                );
            }
        }
        let weights_on = args.params.get("weights").is_some_and(|v| !v.is_null()) || groups_given;
        let groups: Option<Vec<i64>> = match args.params.get("groups") {
            None | Some(serde_json::Value::Null) => None,
            Some(_) => {
                let g: Vec<i64> = per_layer(args, "groups", l0, -1i64)?;
                Some(kept.iter().map(|&i| g[i]).collect())
            }
        };
        if args.param("preprocessing", false)? {
            let maps = zonation::preprocessing(&data, &w, weights_on, groups.as_deref()).map_err(err)?;
            out.insert(
                "preprocessing".into(),
                Value::Stack(Stack {
                    grid: stack.grid.clone(),
                    keys: maps.iter().map(|(k, _)| Key::Name(k.clone())).collect(),
                    layers: maps.iter().map(|(_, v)| on_map(v)).collect(),
                }),
            );
        }
        let flags: Vec<bool> = per_layer(args, "transformed", l0, false)?;
        if flags.iter().any(|f| *f) {
            let (mut keys, mut layers) = (Vec::new(), Vec::new());
            for (j, &i) in kept.iter().enumerate() {
                if flags[i] {
                    keys.push(stack.keys[i].clone());
                    layers.push(on_map(&zonation::transformed(&data, j).map_err(err)?));
                }
            }
            out.insert("transformed".into(), Value::Stack(Stack { grid: stack.grid.clone(), keys, layers }));
        }
        self.post(
            args,
            &PostInput {
                data: &data,
                settings: &settings,
                r: &r,
                cells: &cells,
                w: &w,
                sets: zonation::feature_sets(&w, weights_on, groups.as_deref()),
                names: &names,
                grid: &stack.grid,
                cost: map_data("cost")?,
                hmask: map_data("hierarchic_mask")?,
            },
            &mut out,
        )?;
        Ok(out)
    }
}

/// What the post-processing analyses of a ranking read.
struct PostInput<'a> {
    data: &'a ffi::PrioData,
    settings: &'a ffi::PrioSettings,
    r: &'a ffi::PrioRankOut,
    cells: &'a [i64],
    w: &'a [f32],
    sets: Vec<(String, Vec<usize>)>,
    names: &'a [String],
    grid: &'a Arc<terraflux_raster::Grid>,
    cost: Vec<f64>,
    hmask: Vec<f64>,
}

impl Zonation {
    /// Zonation's post-processing analyses of the ranking: statistics of areas (external, a range of ranks, the
    /// separate areas of a range), the comparison with another solution, the histogram of the features' coverage
    /// and the flexibility map.
    fn post(&self, args: &Args, p: &PostInput, out: &mut Outputs) -> Result<()> {
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let (ny, nx) = (p.grid.height, p.grid.width);
        let n = ny * nx;
        let rank = &p.r.rank;
        let l = p.w.len();
        // a set of features as its weight class (1, -1, 0) or its group
        let set_cols = |set: &str| -> [f64; 2] {
            match set {
                "pos" => [1.0, f64::NAN],
                "neg" => [-1.0, f64::NAN],
                "zero" => [0.0, f64::NAN],
                g => [f64::NAN, g.parse().unwrap_or(f64::NAN)],
            }
        };
        let packed_map = |m: &Map| zonation::gather(&m.data, p.cells);
        let grid_of = |v: &[f64], fill: f64| -> Vec<f64> {
            let mut g = vec![fill; n];
            for (&c, &x) in p.cells.iter().zip(v) {
                g[c as usize] = x;
            }
            g
        };
        // statistics of areas
        let areas_in = args.opt_map("areas")?;
        let spec: Option<Vec<f64>> = match args.params.get("area_statistics") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::String(s)) if s == "areas" => Some(Vec::new()),
            Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| err(format!("area_statistics: {e}")))?),
        };
        let spec = spec.or_else(|| areas_in.map(|_| Vec::new()));
        if let Some(spec) = spec {
            let (mask, map): (Vec<i64>, Option<Vec<f64>>) = match spec.as_slice() {
                [] => {
                    let m = areas_in.ok_or_else(|| err("area_statistics: give the areas (a map)".into()))?;
                    if m.data.len() != n {
                        return Err(err("areas is not on the features' grid".into()));
                    }
                    (packed_map(m).iter().map(|v| v.round_ties_even() as i64).collect(), None)
                }
                [lo, hi] => {
                    let mask = zonation::rank_range_mask(rank, *lo, *hi).map_err(err)?;
                    let m = grid_of(&mask.iter().map(|v| *v as f64).collect::<Vec<_>>(), 0.0);
                    (mask, Some(m))
                }
                [lo, hi, size] => {
                    let full = grid_of(&rank.iter().map(|v| f64::from(*v)).collect::<Vec<_>>(), f64::NAN);
                    let labels = zonation::separate_areas(&full, (ny, nx), *lo, *hi, *size as i64).map_err(err)?;
                    let mask = p.cells.iter().map(|&c| labels[c as usize]).collect();
                    (mask, Some(labels.iter().map(|v| *v as f64).collect()))
                }
                _ => {
                    return Err(err(
                        "area_statistics: \"areas\", [min_rank, max_rank] or [min_rank, max_rank, min_size]".into(),
                    ));
                }
            };
            let feats = zonation::features_float(p.data, l).map_err(err)?;
            let cost: Option<Vec<f64>> =
                (!p.cost.is_empty()).then(|| zonation::gather(&p.cost, p.cells).iter().map(|v| v.max(0.0)).collect());
            let rows = zonation::area_statistics(&feats, p.w, cost.as_deref(), &p.sets, rank, &mask);
            let mut stats = Table::new(
                ["weights", "group", "area", "size", "avgrank", "ds", "wds", "rfd", "wfd", "cost"]
                    .map(String::from)
                    .to_vec(),
            );
            let mut cols: Vec<String> = ["weights", "group", "area"].map(String::from).to_vec();
            cols.extend(p.names.iter().cloned());
            let mut coverage = Table::new(cols);
            for a in &rows {
                let [wc, g] = set_cols(&a.set);
                stats.rows.push(vec![
                    wc,
                    g,
                    a.area as f64,
                    a.size as f64,
                    a.avgrank,
                    a.ds,
                    a.wds,
                    a.rfd,
                    a.wfd,
                    a.cost,
                ]);
                let mut row = vec![wc, g, a.area as f64];
                row.extend(vec![f64::NAN; l]);
                for &(i, v) in &a.coverage {
                    row[3 + i] = v;
                }
                coverage.rows.push(row);
            }
            out.insert("area_statistics".into(), Value::Table(stats));
            out.insert("coverage".into(), Value::Table(coverage));
            if let Some(m) = map {
                out.insert("area_mask".into(), Value::Map(Map::new(p.grid.clone(), m)));
            }
        }
        // the comparison with another solution
        if let Some(v) = args.params.get("comparison").filter(|v| !v.is_null()) {
            let range: [f64; 4] = serde_json::from_value(v.clone())
                .map_err(|e| err(format!("comparison: {e} ([min_rank, max_rank, min_value, max_value])")))?;
            let other = args.map("compare")?;
            if other.data.len() != n {
                return Err(err("compare is not on the features' grid".into()));
            }
            let (codes, counts) = zonation::compare(rank, &packed_map(other), range);
            let mut t = Table::new(["both", "only_this", "only_other", "neither"].map(String::from).to_vec());
            t.rows.push(counts.iter().map(|c| *c as f64).collect());
            out.insert("comparison".into(), Value::Table(t));
            let codes: Vec<f64> = codes.iter().map(|c| f64::from(*c)).collect();
            out.insert("comparison_map".into(), Value::Map(Map::new(p.grid.clone(), grid_of(&codes, 0.0))));
        }
        // the histogram of the features' coverage
        if let Some(v) = args.params.get("histogram").filter(|v| !v.is_null()) {
            let (lo, hi, bins): (f64, f64, usize) = serde_json::from_value(v.clone())
                .map_err(|e| err(format!("histogram: {e} ([min_rank, max_rank, bins])")))?;
            let scalar = p.cells.len() as f64 / p.r.curve_interval as f64;
            let h = zonation::histogram(&p.r.curve_rank, &p.r.curve_levels, l, scalar, &p.sets, (lo, hi, bins))
                .map_err(err)?;
            let mut t = Table::new(["weights", "group", "bin", "coverage_level", "count"].map(String::from).to_vec());
            for (set, counts) in h {
                let [wc, g] = set_cols(&set);
                for (b, c) in counts.iter().enumerate() {
                    t.rows.push(vec![wc, g, b as f64, f64::from(b as f32 / bins as f32), *c as f64]);
                }
            }
            out.insert("histogram".into(), Value::Table(t));
        }
        // the flexibility map
        if let Some(e) = args.opt_param::<f64>("flexibility")? {
            if !(e > 0.0 && e < 100.0) {
                return Err(err("flexibility: the max error should be between 0 and 100 (%)".into()));
            }
            let f = ffi::prio_flexibility(
                p.data,
                p.settings,
                ny as i64,
                nx as i64,
                &p.cost,
                &p.hmask,
                &p.r.order,
                &p.r.curve_rank,
                &p.r.curve_levels,
                p.r.curve_interval,
                e / 100.0,
                true,
                true,
            )?;
            let both: Vec<f32> = f.up.iter().zip(&f.down).map(|(a, b)| a + b).collect();
            let layer = |v: &[f32]| Arc::new(grid_of(&v.iter().map(|x| f64::from(*x)).collect::<Vec<_>>(), f64::NAN));
            out.insert(
                "flexibility".into(),
                Value::Stack(Stack {
                    grid: p.grid.clone(),
                    keys: ["up", "down", "both"].map(|k| Key::Name(k.into())).to_vec(),
                    layers: vec![layer(&f.up), layer(&f.down), layer(&both)],
                }),
            );
        }
        Ok(())
    }
}

/// A setting with a value or null per feature (null: none for all).
fn per_feature<T: serde::de::DeserializeOwned>(args: &Args, name: &str, n: usize) -> Result<Vec<Option<T>>> {
    let bad = |e: String| Error::msg(format!("node {:?}: {name}: {e}", args.node));
    match args.params.get(name) {
        None | Some(serde_json::Value::Null) => Ok((0..n).map(|_| None).collect()),
        Some(v) => {
            let list: Vec<Option<T>> = serde_json::from_value(v.clone()).map_err(|e| bad(e.to_string()))?;
            if list.len() != n {
                return Err(bad(format!("{} values for {n} features (a list, null for none)", list.len())));
            }
            Ok(list)
        }
    }
}

/// A layer of a stack input by its name, on the features' grid.
fn named_map(args: &Args, port: &str, name: &str, (ny, nx): (usize, usize)) -> std::result::Result<Vec<f64>, String> {
    let s =
        args.opt_stack(port).map_err(|e| e.to_string())?.ok_or_else(|| format!("{port}: no maps (for {name:?})"))?;
    if (s.grid.height, s.grid.width) != (ny, nx) {
        return Err(format!("{port} is not on the features' grid"));
    }
    let k = s.keys.iter().position(|k| k.to_string() == name).ok_or_else(|| format!("{port}: no map {name:?}"))?;
    Ok(s.layers[k].to_vec())
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
        if let Some(Value::Stack(costs)) = args.get("zone_costs") {
            return zone_raster_units(args, costs);
        }
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
    if let Some((zones, cost_columns)) = zone_cost_columns(args)? {
        return zone_table_units(args, pu, zones, cost_columns);
    }
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

/// Raster units with zones: `zone_costs` (a layer per zone, named by zone), and every zone's own features
/// (`zone_features`, zone after zone) or the base `features` with every zone's `contribution` (a number or one per
/// feature; a raster of `contribution_maps`, named by zone).
fn zone_raster_units(args: &Args, costs: &Stack) -> Result<milp::Units> {
    let node = args.node;
    let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let zones: Vec<String> = costs.keys.iter().map(|k| k.to_string()).collect();
    let nz = zones.len();
    let cost_layers: Vec<&[f64]> = costs.layers.iter().map(|l| l.as_slice()).collect();
    let shape = (costs.grid.height, costs.grid.width);
    let given_names: Option<Vec<String>> = args.opt_param("feature_names")?;
    if let Some(Value::Stack(zf)) = args.get("zone_features") {
        if zf.layers.len() % nz != 0 {
            return Err(err(format!(
                "zone_features: every zone needs the same features ({} layers, {nz} zones)",
                zf.layers.len()
            )));
        }
        let nf = zf.layers.len() / nz;
        let names = match (given_names, args.get("features")) {
            (Some(n), _) => n,
            (None, Some(Value::Stack(f))) if f.layers.len() == nf => f.keys.iter().map(|k| k.to_string()).collect(),
            _ => zf.keys[..nf].iter().map(|k| k.to_string()).collect(),
        };
        if names.len() != nf {
            return Err(err(format!("feature_names: {} names for {nf} features", names.len())));
        }
        let layers = zf.layers.iter().map(|l| l.as_slice()).collect();
        return milp::Units::raster_zones(
            &cost_layers,
            milp::ZoneFeatures::General(layers),
            zones,
            names,
            shape,
            costs.grid.transform,
        )
        .map_err(err);
    }
    let features = args.stack("features")?;
    let nf = features.layers.len();
    let names = given_names.unwrap_or_else(|| features.keys.iter().map(|k| k.to_string()).collect());
    let given: serde_json::Map<String, serde_json::Value> = args.param("contribution", serde_json::Map::new())?;
    if let Some(z) = given.keys().find(|z| !zones.contains(z)) {
        return Err(err(format!("contribution: unknown zone {z:?} (zones: {zones:?})")));
    }
    let maps = match args.get("contribution_maps") {
        Some(Value::Stack(m)) => Some(m),
        _ => None,
    };
    let mut factors = Vec::new();
    for z in &zones {
        if let Some(m) =
            maps.and_then(|m| m.keys.iter().position(|k| &k.to_string() == z).map(|k| m.layers[k].as_slice()))
        {
            factors.push(milp::Contribution::Map(m));
            continue;
        }
        let f = match given.get(z) {
            None => vec![1.0; nf],
            Some(serde_json::Value::Number(x)) => vec![x.as_f64().unwrap(); nf],
            Some(v) => serde_json::from_value(v.clone()).map_err(|e| err(format!("contribution of {z:?}: {e}")))?,
        };
        if f.len() != nf || f.iter().any(|x| !x.is_finite()) {
            return Err(err(format!("contribution of {z:?}: one number, or one per feature ({nf})")));
        }
        factors.push(milp::Contribution::Factors(f));
    }
    let base = features.layers.iter().map(|l| l.as_slice()).collect();
    milp::Units::raster_zones(
        &cost_layers,
        milp::ZoneFeatures::Contribution { base, factors },
        zones,
        names,
        shape,
        costs.grid.transform,
    )
    .map_err(err)
}

/// Table units with zones: their names and cost columns (`cost_column` a mapping zone -> column, or a list of
/// columns, the zones then named by `zones` or by the columns). None without zones.
fn zone_cost_columns(args: &Args) -> Result<Option<(Vec<String>, Vec<String>)>> {
    let node = args.node;
    let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let names_of = |v: &serde_json::Value| -> Result<Vec<String>> {
        let l: Vec<serde_json::Value> = serde_json::from_value(v.clone()).map_err(|e| err(format!("zones: {e}")))?;
        l.iter()
            .map(|z| match z {
                serde_json::Value::String(s) => Ok(s.clone()),
                serde_json::Value::Object(o) => o
                    .get("name")
                    .and_then(|n| n.as_str())
                    .map(String::from)
                    .ok_or_else(|| err("zones: [{id, name}, ...] or names".into())),
                _ => Err(err("zones: [{id, name}, ...] or names".into())),
            })
            .collect()
    };
    match args.params.get("cost_column") {
        Some(serde_json::Value::Object(m)) => {
            let zones: Vec<String> = m.keys().cloned().collect();
            let cols = m
                .values()
                .map(|v| v.as_str().map(String::from).ok_or_else(|| err("cost_column: zone -> column".into())))
                .collect::<Result<Vec<_>>>()?;
            Ok(Some((zones, cols)))
        }
        Some(v @ serde_json::Value::Array(_)) => {
            let cols: Vec<String> = serde_json::from_value(v.clone()).map_err(|e| err(format!("cost_column: {e}")))?;
            let zones = match args.params.get("zones") {
                Some(z) if !z.is_null() => names_of(z)?,
                _ => cols.clone(),
            };
            if zones.len() != cols.len() {
                return Err(err(format!("one cost column per zone ({}), got {}", zones.len(), cols.len())));
            }
            Ok(Some((zones, cols)))
        }
        _ => Ok(None),
    }
}

/// Table units with zones: every zone's feature columns (`features` a mapping zone -> columns), the base features'
/// columns with every zone's `contribution` (a number, one per feature, or a column), or Marxan's spec and rij with
/// a zone column (`zones`: [{id, name}, ...]).
fn zone_table_units(args: &Args, pu: &Table, zones: Vec<String>, cost_columns: Vec<String>) -> Result<milp::Units> {
    let node = args.node;
    let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let col = |t: &Table, c: &str, what: &str| t.column(c).ok_or_else(|| err(format!("{what} has no column {c:?}")));
    let costs = cost_columns.iter().map(|c| col(pu, c, "pu")).collect::<Result<Vec<_>>>()?;
    let cost_refs: Vec<&[f64]> = costs.iter().map(|c| c.as_slice()).collect();
    let nz = zones.len();
    if let (Some(Value::Table(spec)), Some(Value::Table(rij))) = (args.get("spec"), args.get("rij")) {
        let ids = col(spec, "id", "spec")?;
        let names = ids.iter().map(|v| terraflux_ops_label(*v)).collect();
        let zone_ids: Vec<f64> = match args.params.get("zones") {
            Some(serde_json::Value::Array(l)) if l.iter().all(|z| z.is_object()) => l
                .iter()
                .map(|z| z.get("id").and_then(|v| v.as_f64()).ok_or_else(|| err("zones: [{id, name}, ...]".into())))
                .collect::<Result<_>>()?,
            _ => (1..=nz).map(|z| z as f64).collect(),
        };
        let zone_col = rij.column("zone").unwrap_or_else(|| vec![zone_ids[0]; rij.rows.len()]);
        return milp::Units::rij_zones(
            &col(pu, "id", "pu")?,
            &cost_refs,
            &ids,
            names,
            (&col(rij, "pu", "rij")?, &col(rij, "species", "rij")?, &zone_col, &col(rij, "amount", "rij")?),
            &zone_ids,
            zones,
        )
        .map_err(err);
    }
    let given_names: Option<Vec<String>> = args.opt_param("feature_names")?;
    match args.params.get("features") {
        Some(serde_json::Value::Object(m)) => {
            let mut amounts = Vec::with_capacity(nz);
            let mut first: Option<Vec<String>> = None;
            for z in &zones {
                let cols: Vec<String> = match m.get(z) {
                    Some(serde_json::Value::String(c)) => vec![c.clone()],
                    Some(v) => serde_json::from_value(v.clone()).map_err(|e| err(format!("features of {z:?}: {e}")))?,
                    None => return Err(err(format!("features: no columns for the zone {z:?}"))),
                };
                if first.as_ref().is_some_and(|f| f.len() != cols.len()) {
                    return Err(err("every zone must have the same number of feature columns".into()));
                }
                amounts.push(cols.iter().map(|c| col(pu, c, "pu")).collect::<Result<Vec<_>>>()?);
                first.get_or_insert(cols);
            }
            let names = given_names.unwrap_or_else(|| first.unwrap_or_default());
            milp::Units::table_zones(&cost_refs, &amounts, None, zones, names).map_err(err)
        }
        Some(v) => {
            let names: Vec<String> = match v {
                serde_json::Value::String(s) => vec![s.clone()],
                _ => serde_json::from_value(v.clone()).map_err(|e| err(format!("features: {e}")))?,
            };
            let base = names.iter().map(|c| col(pu, c, "pu")).collect::<Result<Vec<_>>>()?;
            let nf = base.len();
            let given: serde_json::Map<String, serde_json::Value> =
                args.param("contribution", serde_json::Map::new())?;
            let mut amounts = Vec::with_capacity(nz);
            for z in &zones {
                let a: Vec<Vec<f64>> = match given.get(z) {
                    None => base.clone(),
                    Some(serde_json::Value::Number(x)) => {
                        let k = x.as_f64().unwrap();
                        base.iter().map(|f| f.iter().map(|v| v * k).collect()).collect()
                    }
                    Some(serde_json::Value::String(c)) => {
                        let k = col(pu, c, "pu")?;
                        base.iter().map(|f| f.iter().zip(&k).map(|(v, k)| v * k).collect()).collect()
                    }
                    Some(v) => {
                        let k: Vec<f64> = serde_json::from_value(v.clone())
                            .map_err(|e| err(format!("contribution of {z:?}: {e}")))?;
                        if k.len() != nf {
                            return Err(err(format!(
                                "contribution of {z:?}: one number, one per feature ({nf}) or a column"
                            )));
                        }
                        base.iter().zip(&k).map(|(f, k)| f.iter().map(|v| v * k).collect()).collect()
                    }
                };
                amounts.push(a);
            }
            milp::Units::table_zones(&cost_refs, &amounts, Some(&base), zones, given_names.unwrap_or(names))
                .map_err(err)
        }
        None => Err(err("give pu's feature columns (features: a list, or a mapping zone -> columns)".into())),
    }
}

/// A spatial penalty or constraint of a problem with zones: penalties and k one or one per zone, edge factors one or
/// one per zone, a zones matrix (`zones`: rows the first zone), the linear data a layer per zone
/// (`zone_penalty_data`, `zone_constraint_data`), one map for every zone, or tables' columns (one or one per zone).
fn zone_spatial(
    args: &Args,
    units: &milp::Units,
    it: &serde_json::Map<String, serde_json::Value>,
    kind: &str,
    pu: Option<&Table>,
) -> Result<milp::ZoneSpatial> {
    let node = args.node;
    let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let nums = |k: &str| -> Result<Option<Vec<f64>>> {
        match it.get(k) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(serde_json::Value::Number(x)) => Ok(Some(vec![x.as_f64().unwrap()])),
            Some(v) => {
                serde_json::from_value(v.clone()).map(Some).map_err(|e| err(format!("spatial {kind}: {k}: {e}")))
            }
        }
    };
    let need = |k: &str| nums(k)?.ok_or_else(|| err(format!("spatial: {kind:?} needs {k:?}")));
    let one = |k: &str| -> Result<f64> {
        let v = need(k)?;
        if v.len() != 1 {
            return Err(err(format!("spatial {kind}: {k} is one number")));
        }
        Ok(v[0])
    };
    let zones: Option<Vec<Vec<f64>>> = match it.get("zones") {
        None | Some(serde_json::Value::Null) => None,
        Some(v) => Some(
            serde_json::from_value(v.clone())
                .map_err(|e| err(format!("spatial {kind}: zones: {e} (zones x zones)")))?,
        ),
    };
    // values per zone: a layer per zone, one map, or pu's columns
    let data = |stack: &str, map: &str| -> Result<Vec<Vec<f64>>> {
        if let Some(t) = pu {
            let cols: Vec<String> = match it.get("column") {
                Some(serde_json::Value::String(c)) => vec![c.clone()],
                Some(v) => {
                    serde_json::from_value(v.clone()).map_err(|e| err(format!("spatial {kind}: column: {e}")))?
                }
                None => return Err(err(format!("spatial {kind}: tables need the column(s) of the values"))),
            };
            return cols.iter().map(|c| t.column(c).ok_or_else(|| err(format!("pu has no column {c:?}")))).collect();
        }
        if let Some(Value::Stack(s)) = args.get(stack) {
            return Ok(s.layers.iter().map(|l| l.to_vec()).collect());
        }
        Ok(vec![args.map(map)?.data.to_vec()])
    };
    let matrix = |port: &str| matrix_of(args, port, units.size);
    Ok(match kind {
        "boundary" => milp::ZoneSpatial::Boundary {
            penalty: one("penalty")?,
            edge_factor: nums("edge_factor")?.unwrap_or_else(|| vec![0.5]),
            data: matrix("boundary_data")?,
            zones,
        },
        "connectivity" => milp::ZoneSpatial::Connectivity {
            penalty: one("penalty")?,
            data: matrix("connectivity_data")?
                .ok_or_else(|| err("connectivity penalties need connectivity_data".into()))?,
            zones,
        },
        "asym_connectivity" => milp::ZoneSpatial::AsymConnectivity {
            penalty: one("penalty")?,
            data: matrix("asym_connectivity_data")?
                .ok_or_else(|| err("asymmetric connectivity penalties need asym_connectivity_data".into()))?,
            zones,
        },
        "linear_penalty" => milp::ZoneSpatial::LinearPenalty {
            penalty: need("penalty")?,
            data: data("zone_penalty_data", "penalty_data")?,
        },
        "linear_constraint" => {
            let sense = it.get("sense").and_then(|v| v.as_str()).unwrap_or("<=").to_string();
            if !matches!(sense.as_str(), ">=" | "<=" | "=") {
                return Err(err(format!("a linear constraint's sense is >=, <= or =, not {sense:?}")));
            }
            milp::ZoneSpatial::LinearConstraint {
                threshold: one("threshold")?,
                sense,
                data: data("zone_constraint_data", "constraint_data")?,
            }
        }
        "neighbors" => milp::ZoneSpatial::Neighbors {
            k: need("k")?,
            clamp: it.get("clamp").and_then(|v| v.as_bool()).unwrap_or(true),
            data: matrix("adjacency_data")?,
            zones,
        },
        "contiguity" => milp::ZoneSpatial::Contiguity { data: matrix("adjacency_data")?, zones },
        "feature_contiguity" => return Err(err("feature contiguity constraints are not supported with zones".into())),
        k => return Err(err(format!("unknown spatial penalty or constraint {k:?}"))),
    })
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
    let (nz, zoned) = (units.nz(), units.zoned());
    // one budget, or one per zone
    let budgets: Vec<f64> = match args.params.get("budget") {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::Number(b)) => vec![b.as_f64().unwrap()],
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| err(format!("budget: {e}")))?,
    };
    let budget =
        || -> Result<f64> { budgets.first().copied().ok_or_else(|| err("this objective needs a budget".into())) };
    let zone_budgets = if budgets.len() > 1 { budgets.clone() } else { Vec::new() };
    // a matrix (features x zones) given as a list of lists
    let matrix = |name: &str| -> Result<Option<Vec<Vec<f64>>>> {
        match args.params.get(name) {
            Some(serde_json::Value::Array(l)) if l.iter().any(|v| v.is_array()) => {
                let m: Vec<Vec<f64>> = serde_json::from_value(serde_json::Value::Array(l.clone()))
                    .map_err(|e| err(format!("{name}: {e} (features x zones)")))?;
                Ok(Some(m))
            }
            _ => Ok(None),
        }
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
        "max_phylo_div" | "max_phylo_end" => {
            let tree: String = args
                .opt_param("tree")?
                .ok_or_else(|| err(format!("the {objective} objective needs a tree (Newick text)")))?;
            if objective == "max_phylo_div" {
                milp::Objective::MaxPhyloDiv(budget()?, tree)
            } else {
                milp::Objective::MaxPhyloEnd(budget()?, tree)
            }
        }
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
    let manual = |v: &serde_json::Value| -> Result<milp::TargetSpec> {
        // [{feature: name or 1.., zone: name or [names] (default every zone), type: relative|absolute, sense, target}]
        let l: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_value(v.clone()).map_err(|e| err(format!("manual_targets: {e}")))?;
        let zone_of = |z: &serde_json::Value| -> Result<usize> {
            match z {
                serde_json::Value::String(s) => units
                    .zones
                    .iter()
                    .position(|x| x == s)
                    .ok_or_else(|| err(format!("manual_targets: unknown zone {s:?}"))),
                serde_json::Value::Number(k) => Ok((k.as_f64().unwrap() as usize).saturating_sub(1)),
                _ => Err(err("manual_targets: a zone is a name or a number (1..)".into())),
            }
        };
        let mut out = Vec::with_capacity(l.len());
        for m in &l {
            let feature = match m.get("feature") {
                Some(serde_json::Value::String(s)) => units
                    .names
                    .iter()
                    .position(|x| x == s)
                    .ok_or_else(|| err(format!("manual_targets: unknown feature {s:?}")))?,
                Some(serde_json::Value::Number(k)) => (k.as_f64().unwrap() as usize).saturating_sub(1),
                _ => return Err(err("manual_targets: every target needs its feature".into())),
            };
            let zones = match m.get("zone") {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::Array(zs)) => Some(zs.iter().map(&zone_of).collect::<Result<Vec<_>>>()?),
                Some(z) => Some(vec![zone_of(z)?]),
            };
            out.push(milp::ManualTarget {
                feature,
                relative: m.get("type").and_then(|t| t.as_str()).unwrap_or("relative") == "relative",
                sense: m.get("sense").and_then(|s| s.as_str()).unwrap_or(">=").to_string(),
                target: m
                    .get("target")
                    .and_then(|t| t.as_f64())
                    .ok_or_else(|| err("manual_targets: every target needs its target".into()))?,
                zones,
            });
        }
        Ok(milp::TargetSpec::Manual(out))
    };
    let targets = if let Some(m) = matrix("relative_targets")? {
        Some(milp::TargetSpec::RelativeZones(m))
    } else if let Some(m) = matrix("absolute_targets")? {
        Some(milp::TargetSpec::AbsoluteZones(m))
    } else if let Some(v) = args.params.get("manual_targets").filter(|v| !v.is_null()) {
        Some(manual(v)?)
    } else if let Some(r) = per_feature("relative_targets")? {
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
        let zone = t.column("zone");
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
                    zones: zone.as_ref().map(|z| vec![(z[i] as usize).saturating_sub(1)]),
                })
                .collect(),
        ))
    } else {
        None
    };
    // weights per feature (or target), or per feature and zone (features x zones)
    let (weights, zone_weights) = match matrix("feature_weights")? {
        Some(m) if m.iter().all(|r| r.len() == 1) => (Some(m.into_iter().flatten().collect()), None),
        Some(m) => (None, Some(m)),
        None => match args.params.get("feature_weights") {
            Some(serde_json::Value::Array(l)) if zoned => (
                Some(
                    serde_json::from_value(serde_json::Value::Array(l.clone()))
                        .map_err(|e| err(format!("feature_weights: {e}")))?,
                ),
                None,
            ),
            _ => (per_feature("feature_weights")?, None),
        },
    };
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
        if zoned {
            // a layer per zone (zone_locked_in, zone_locked_out), or a column per zone
            let columns: Option<Vec<String>> = args.opt_param(&format!("{port}_column"))?;
            let layers: Vec<Vec<f64>> = match (pu, columns, args.get(&format!("zone_{port}"))) {
                (Some(t), Some(cols), _) => cols
                    .iter()
                    .map(|c| t.column(c).ok_or_else(|| err(format!("pu has no column {c:?}"))))
                    .collect::<Result<_>>()?,
                (None, _, Some(Value::Stack(s))) => {
                    if s.layers.len() != nz || s.layers.iter().any(|l| l.len() != units.size) {
                        return Err(err(format!("zone_{port}: a layer per zone ({nz}) on the costs' grid")));
                    }
                    s.layers.iter().map(|l| l.to_vec()).collect()
                }
                _ => return Ok(vec![false; n]),
            };
            if layers.len() != nz {
                return Err(err(format!("{port}: one per zone ({nz})")));
            }
            return Ok((0..nz)
                .flat_map(|z| units.cells.iter().map(move |&c| (z, c)))
                .map(|(z, c)| !layers[z][c].is_nan() && layers[z][c] != 0.0)
                .collect());
        }
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
                put(
                    "boundary",
                    &[("boundary_penalty", "penalty"), ("edge_factor", "edge_factor"), ("boundary_zones", "zones")],
                );
            }
            if args.params.contains_key("connectivity_penalty") {
                put("connectivity", &[("connectivity_penalty", "penalty"), ("connectivity_zones", "zones")]);
            }
            if args.params.contains_key("asym_connectivity_penalty") {
                put(
                    "asym_connectivity",
                    &[("asym_connectivity_penalty", "penalty"), ("asym_connectivity_zones", "zones")],
                );
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
                put("neighbors", &[("neighbors", "k"), ("clamp", "clamp"), ("neighbors_zones", "zones")]);
            }
            for kind in ["contiguity", "feature_contiguity"] {
                if args.params.get(kind).and_then(|v| v.as_bool()).unwrap_or(false) {
                    put(kind, &[("contiguity_zones", "zones")]);
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
        if zoned {
            spatial.push(milp::Spatial::Zoned(zone_spatial(args, &units, it, kind, pu)?));
            continue;
        }
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
    // manual bounds: the bounds table, then the status table
    let mut bounds = Vec::new();
    for (port, kind, cols) in [("bounds", "bounded", ["lower", "upper"]), ("status", "locked", ["status", "status"])] {
        if let Some(Value::Table(t)) = args.get(port) {
            let col =
                |c: &str| t.column(c).ok_or_else(|| err(format!("{port} needs the columns pu, {}", cols.join(", "))));
            bounds.push(milp::ManualBounds {
                kind: kind.into(),
                pu: col("pu")?,
                lower: col(cols[0])?,
                upper: col(cols[1])?,
                zone: t.column("zone"),
            });
        }
    }
    let mandatory: bool = args.param("mandatory_allocation", false)?;
    if (mandatory || !zone_budgets.is_empty() || zone_weights.is_some()) && !zoned {
        return Err(err("mandatory allocation, budgets per zone and weights per zone need zones".into()));
    }
    Ok(milp::Problem {
        units,
        objective,
        targets,
        weights,
        decisions,
        locked_in,
        locked_out,
        spatial,
        bounds,
        zone_budgets,
        zone_weights,
        mandatory,
    })
}

struct Optimize;

impl Operator for Optimize {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.optimize",
            "Systematic conservation planning as prioritizr 8.0.6 (Hanson et al.): an integer program over the \
             planning units (the cells with a cost) solved to optimality (within the gap) by HiGHS, the model \
             prioritizr compiles. Objectives: min_set (meet every target at the least cost), min_shortfall, \
             min_largest_shortfall, max_utility, max_features, max_cover, min_penalties, max_phylo_div, \
             max_phylo_end (with a budget); \
             targets relative, absolute, loglinear or by hand; feature weights; binary, proportion or \
             semicontinuous decisions; locked units; boundary, connectivity and linear penalties, linear \
             constraints, neighbour and contiguity constraints; planning units in maps or tables; portfolios of solutions.",
            Access::Global,
        )
        .input(PortSpec::optional("features", Type::Stack, "the features' amounts, a layer each"))
        .input(PortSpec::optional("cost", Type::Map, "the cost of each cell (no value: not a planning unit)"))
        .input(PortSpec::optional(
            "zone_costs",
            Type::Stack,
            "management zones: the cost of every cell in every zone, a layer per zone named by it (no value: the \
             cell cannot go to the zone)",
        ))
        .input(PortSpec::optional(
            "zone_features",
            Type::Stack,
            "zones: every zone's own features, zone after zone (prioritizr's zones()); else features and contribution",
        ))
        .input(PortSpec::optional("contribution_maps", Type::Stack, "zones: a contribution raster per zone, named by it"))
        .input(PortSpec::optional("zone_locked_in", Type::Stack, "zones: the units locked into every zone, a layer each"))
        .input(PortSpec::optional("zone_locked_out", Type::Stack, "zones: the units locked out of every zone"))
        .input(PortSpec::optional("zone_penalty_data", Type::Stack, "zones: linear penalties, a layer per zone"))
        .input(PortSpec::optional("zone_constraint_data", Type::Stack, "zones: a linear constraint, a layer per zone"))
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
        .input(PortSpec::optional(
            "bounds",
            Type::Table,
            "manual bounds: pu (the cell or row, 1..), lower and upper (a unit listed again: its last bounds)",
        ))
        .input(PortSpec::optional(
            "status",
            Type::Table,
            "manual locked units: pu and status (1 in, 0 out; a share with proportion decisions), after bounds",
        ))
        .input(PortSpec::optional("penalty_data", Type::Map, "linear penalties: a value per cell"))
        .input(PortSpec::optional("constraint_data", Type::Map, "the linear constraint: a value per cell"))
        .output(PortSpec::optional(
            "solution",
            Type::Map,
            "maps: the share of each unit selected (1 selected); zones: the zone of each unit (1.., 0 none)",
        ))
        .output(PortSpec::optional("allocation", Type::Stack, "maps with zones: the share of each zone, a layer each"))
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
            "min_set (default), min_shortfall, min_largest_shortfall, max_utility, max_features, max_cover, \
             min_penalties, max_phylo_div, max_phylo_end",
        ))
        .param(PortSpec::optional(
            "budget",
            Type::Any,
            "the objectives with a budget: the cost they may spend (with zones: one, or one per zone)",
        ))
        .param(PortSpec::optional(
            "manual_targets",
            Type::Any,
            "[{feature, zone (a name or names; default every zone), type: relative|absolute, sense, target}, ...]",
        ))
        .param(PortSpec::optional(
            "contribution",
            Type::Any,
            "zones: every zone's share of the features ({zone: a number, one per feature, or tables: a column})",
        ))
        .param(PortSpec::optional("feature_names", Type::Any, "zones: the features' names"))
        .param(PortSpec::optional("zones", Type::Any, "tables with zones: the zones' names, or Marxan's [{id, name}, ...]"))
        .param(PortSpec::optional(
            "boundary_zones",
            Type::Any,
            "zones: how much a boundary counts between two zones (zones x zones; default the identity); likewise \
             connectivity_zones, asym_connectivity_zones, neighbors_zones (0/1) and contiguity_zones (0/1), or \
             `zones` in a spatial item",
        ))
        .param(PortSpec::optional("connectivity_zones", Type::Any, "zones: as boundary_zones"))
        .param(PortSpec::optional("asym_connectivity_zones", Type::Any, "zones: as boundary_zones (not symmetric)"))
        .param(PortSpec::optional("neighbors_zones", Type::Any, "zones: the zones whose units count as neighbours"))
        .param(PortSpec::optional("contiguity_zones", Type::Any, "zones: the zones whose units form one patch together"))
        .param(PortSpec::optional(
            "mandatory_allocation",
            Type::Bool,
            "zones: every planning unit goes to a zone (add_mandatory_allocation_constraints)",
        ))
        .param(PortSpec::optional(
            "tree",
            Type::Text,
            "max_phylo_div, max_phylo_end: the phylogeny in Newick, its tips the features (with branch lengths)",
        ))
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
        let u = &p.units;
        let (nu, nz, zoned) = (u.units(), u.nz(), u.zoned());
        let xs: Vec<&[f64]> = rs.iter().map(|r| &r.x[..n]).collect();
        let x = xs[0];
        let cost = &u.cost;
        // a column per solution (and zone)
        let solution_names: Vec<String> = match portfolio {
            Some(_) => (1..=xs.len()).map(|k| format!("solution_{k}")).collect(),
            None => vec!["solution".into()],
        };
        let zone_suffix = |z: usize| if zoned { format!("_{}", u.zones[z]) } else { String::new() };
        let frequency: Vec<f64> = (0..n).map(|i| xs.iter().map(|x| x[i]).sum::<f64>() / xs.len() as f64).collect();
        let mut columns = vec!["unit".to_string()];
        for name in &solution_names {
            columns.extend((0..nz).map(|z| format!("{name}{}", zone_suffix(z))));
        }
        if portfolio.is_some() {
            columns.extend((0..nz).map(|z| format!("frequency{}", zone_suffix(z))));
        }
        let mut selection = Table::new(columns);
        selection.rows = (0..nu)
            .map(|i| {
                let mut row = vec![(u.cells[i] + 1) as f64];
                for x in &xs {
                    row.extend((0..nz).map(|z| x[z * nu + i]));
                }
                if portfolio.is_some() {
                    row.extend((0..nz).map(|z| frequency[z * nu + i]));
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
            let spent: f64 = cost.iter().zip(x.iter()).filter(|(c, _)| c.is_finite()).map(|(c, x)| c * x).sum();
            let selected = (0..nu).filter(|&i| (0..nz).any(|z| x[z * nu + i] > 0.5)).count();
            let mut row = vec![r.objective, spent, selected as f64, r.gap, f64::from(u8::from(r.status == "optimal"))];
            if portfolio.is_some() {
                row.insert(0, (k + 1) as f64);
            }
            summary.rows.push(row);
        }
        let mut coverage = Table::new(["feature", "held", "target", "total", "met"].map(String::from).to_vec());
        for (k, &f) in t.feature.iter().enumerate() {
            let (idx, v) = u.rij.row(f);
            let counts = |i: usize| !zoned || t.zone[k].contains(&(i / nu));
            let held: f64 = idx.iter().zip(v).filter(|(i, _)| counts(**i)).map(|(&i, a)| a * x[i]).sum();
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
        if u.raster {
            let grid = match args.get("zone_costs") {
                Some(Value::Stack(s)) => s.grid.clone(),
                _ => args.map("cost")?.grid.clone(),
            };
            let on_grid = |v: &[f64]| {
                let mut out = vec![f64::NAN; grid.width * grid.height];
                for (i, &c) in u.cells.iter().enumerate() {
                    out[c] = v[i];
                }
                out
            };
            // with zones: the zone of every unit (prioritizr's category_layer: the zone with the most, 0 none)
            let zone_of = |x: &[f64]| -> Vec<f64> {
                (0..nu)
                    .map(|i| {
                        let (best, v) = (0..nz)
                            .map(|z| (z, if cost[z * nu + i].is_finite() { x[z * nu + i] } else { f64::NEG_INFINITY }))
                            .fold((0, f64::NEG_INFINITY), |b, c| if c.1 > b.1 { c } else { b });
                        if v > 0.0 { (best + 1) as f64 } else { 0.0 }
                    })
                    .collect()
            };
            let shown = |x: &[f64]| if zoned { on_grid(&zone_of(x)) } else { on_grid(x) };
            out.insert("solution".into(), Value::Map(Map::new(grid.clone(), shown(x))));
            if zoned {
                let keys = u.zones.iter().map(|z| Key::Name(z.clone())).collect();
                let layers = (0..nz)
                    .map(|z| {
                        let v: Vec<f64> = (0..nu)
                            .map(|i| if cost[z * nu + i].is_finite() { x[z * nu + i] } else { f64::NAN })
                            .collect();
                        Arc::new(on_grid(&v))
                    })
                    .collect();
                out.insert("allocation".into(), Value::Stack(Stack { grid: grid.clone(), keys, layers }));
            }
            if portfolio.is_some() {
                let keys = solution_names.iter().map(|n| Key::Name(n.clone())).collect();
                let layers = xs.iter().map(|x| Arc::new(shown(x))).collect();
                if !zoned {
                    out.insert("frequency".into(), Value::Map(Map::new(grid.clone(), on_grid(&frequency))));
                }
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

/// A solution's value in every unit and zone, zone after zone (no value: 0): a layer per zone (`zone_solution`),
/// or the selection (prioritize.optimize's: unit and solution_<zone>; or a column solution_<zone> per zone with a
/// row per unit of the table or per planning unit).
fn zone_solution_of(args: &Args, u: &milp::Units) -> Result<Vec<f64>> {
    let node = args.node;
    let err = |m: &str| Error::msg(format!("node {node:?}: {m}"));
    let (nu, nz) = (u.units(), u.nz());
    let finite = |v: f64| if v.is_finite() { v } else { 0.0 };
    if let Some(Value::Stack(st)) = args.get("zone_solution") {
        if st.layers.len() != nz || st.layers.iter().any(|l| l.len() != u.size) {
            return Err(err(&format!("zone_solution: a layer per zone ({nz}) on the units' grid")));
        }
        return Ok((0..nz)
            .flat_map(|z| u.cells.iter().map(move |&c| (z, c)))
            .map(|(z, c)| finite(st.layers[z][c]))
            .collect());
    }
    let Some(Value::Table(t)) = args.get("selection") else {
        return Err(err("the solution with zones: a stack (zone_solution) or a table (selection)"));
    };
    let mut out = vec![0.0; nz * nu];
    for (z, name) in u.zones.iter().enumerate() {
        let col = [format!("solution_{name}"), format!("solution_1_{name}")]
            .iter()
            .find_map(|c| t.column(c))
            .ok_or_else(|| err(&format!("the selection needs a column solution_{name}")))?;
        if let Some(unit) = t.column("unit") {
            let mut at = vec![0.0; u.size];
            for (&k, &v) in unit.iter().zip(&col) {
                if !(k >= 1.0 && k <= u.size as f64 && k.fract() == 0.0) {
                    return Err(err(&format!("the selection's unit {k} is no unit (1-{})", u.size)));
                }
                at[k as usize - 1] = finite(v);
            }
            for (i, &c) in u.cells.iter().enumerate() {
                out[z * nu + i] = at[c];
            }
        } else if col.len() == u.size {
            for (i, &c) in u.cells.iter().enumerate() {
                out[z * nu + i] = finite(col[c]);
            }
        } else if col.len() == nu {
            for (i, &v) in col.iter().enumerate() {
                out[z * nu + i] = finite(v);
            }
        } else {
            return Err(err(&format!("the selection needs a row per unit ({} or {nu})", u.size)));
        }
    }
    Ok(out)
}

/// prioritize.evaluate with zones: the summaries overall and per zone (summary 0: overall, 1..: the zones), the
/// features' representation, the targets' coverage.
fn evaluate_zoned(ctx: &mut Ctx, args: &Args, p: &milp::Problem) -> Result<Outputs> {
    let node = args.node;
    let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
    let u = &p.units;
    let (nu, nz, nf) = (u.units(), u.nz(), u.names.len());
    let s = zone_solution_of(args, u)?;
    let zones_param = |name: &str| -> Result<Option<Vec<Vec<f64>>>> {
        match args.params.get(name) {
            None | Some(serde_json::Value::Null) => Ok(None),
            Some(v) => serde_json::from_value(v.clone()).map(Some).map_err(|e| err(format!("{name}: {e}"))),
        }
    };
    let edge_factor: Vec<f64> = match args.params.get("edge_factor") {
        None | Some(serde_json::Value::Null) => vec![0.5],
        Some(serde_json::Value::Number(x)) => vec![x.as_f64().unwrap()],
        Some(v) => serde_json::from_value(v.clone()).map_err(|e| err(format!("edge_factor: {e}")))?,
    };
    let bm = matrix_of(args, "boundary_data", u.size)?;
    let e = milp::evaluate_zones(u, &s, &edge_factor, &zones_param("boundary_zones")?, bm.as_ref()).map_err(err)?;
    let mut columns: Vec<String> = ["summary", "n", "cost", "boundary"].map(String::from).to_vec();
    let mut cols: Vec<Vec<f64>> = vec![
        (0..=nz).map(|k| k as f64).collect(),
        e.n.clone(),
        e.cost.clone(),
        e.boundary.clone().unwrap_or_else(|| vec![f64::NAN; nz + 1]),
    ];
    for (port, asym, name) in
        [("connectivity_data", false, "connectivity"), ("asym_connectivity_data", true, "asym_connectivity")]
    {
        if let Some(m) = matrix_of(args, port, u.size)? {
            let (v, symmetric) =
                milp::connectivity_zones(u, &m, &s, &zones_param(&format!("{name}_zones"))?, asym).map_err(err)?;
            if symmetric == asym {
                let (is, other) =
                    if asym { ("symmetric", "connectivity") } else { ("asymmetric", "asym_connectivity") };
                ctx.warn(format!("{port} holds {is} values: the {other} summary suits them"));
            }
            columns.push(name.into());
            cols.push(v);
        }
    }
    let mut summary = Table::new(columns);
    summary.rows = (0..=nz).map(|k| cols.iter().map(|c| c[k]).collect()).collect();
    let mut rep =
        Table::new(["summary", "feature", "total_amount", "absolute_held", "relative_held"].map(String::from).to_vec());
    rep.rows = (0..(nz + 1) * nf)
        .map(|k| vec![(k / nf) as f64, (k % nf + 1) as f64, e.total[k], e.held[k], e.held[k] / e.total[k]])
        .collect();
    let mut out = outputs([("summary", Value::Table(summary)), ("representation", Value::Table(rep))]);
    if p.targets.is_some() && !p.objective.no_targets() {
        let (_, t) = milp::compile(p).map_err(err)?;
        let held = milp::zone_targets_held(u, &t, &s);
        let mut cov = Table::new(
            [
                "feature",
                "sense",
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
            let sense = match t.sense[k].as_str() {
                ">=" => 1.0,
                "<=" => -1.0,
                _ => 0.0,
            };
            cov.rows.push(vec![
                (t.feature[k] + 1) as f64,
                sense,
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
    }
    let mut columns = vec!["unit".to_string()];
    columns.extend(u.zones.iter().map(|z| format!("solution_{z}")));
    let mut units = Table::new(columns);
    units.rows = (0..nu)
        .map(|i| std::iter::once((u.cells[i] + 1) as f64).chain((0..nz).map(|z| s[z * nu + i])).collect())
        .collect();
    out.insert("units".into(), Value::Table(units));
    Ok(out)
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
        .input(PortSpec::optional("zone_costs", Type::Stack, "zones: as prioritize.optimize's"))
        .input(PortSpec::optional("zone_features", Type::Stack, "zones: as prioritize.optimize's"))
        .input(PortSpec::optional("contribution_maps", Type::Stack, "zones: as prioritize.optimize's"))
        .input(PortSpec::optional(
            "zone_solution",
            Type::Stack,
            "zones: the share of each unit in every zone, a layer per zone (or the selection: solution_<zone>)",
        ))
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
        .param(PortSpec::optional(
            "edge_factor",
            Type::Any,
            "the boundary: the weight of edges to no unit (0.5; zones: one or one per zone)",
        ))
        .param(PortSpec::optional("boundary_zones", Type::Any, "zones: the boundary's zones matrix (default the identity)"))
        .param(PortSpec::optional("connectivity_zones", Type::Any, "zones: the connectivity's zones matrix"))
        .param(PortSpec::optional("asym_connectivity_zones", Type::Any, "zones: the asymmetric connectivity's"))
        .param(PortSpec::optional("manual_targets", Type::Any, "targets by hand with their zones, as prioritize.optimize's"))
        .param(PortSpec::optional("feature_names", Type::Any, "zones: the features' names"))
        .param(PortSpec::optional("contribution", Type::Any, "zones: as prioritize.optimize's"))
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
        if u.zoned() {
            return evaluate_zoned(ctx, args, &p);
        }
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

/// numpy's linspace: start + k step, the last exactly stop.
fn linspace(start: f64, stop: f64, n: usize) -> Vec<f64> {
    if n == 1 {
        return vec![start];
    }
    let step = (stop - start) / (n - 1) as f64;
    let mut v: Vec<f64> = (0..n).map(|k| k as f64 * step + start).collect();
    v[n - 1] = stop;
    v
}

/// TOPSIS (R's topsis package) of alternatives (rows) on criteria where smaller is better, equal weights: every
/// alternative's closeness to the ideal (0-1, larger is better; no value where undefined).
pub fn topsis(d: &[Vec<f64>]) -> Vec<f64> {
    let nc = d.first().map_or(0, |r| r.len());
    let w = 1.0 / nc as f64;
    let norm: Vec<f64> = (0..nc).map(|c| d.iter().map(|r| r[c] * r[c]).sum::<f64>().sqrt()).collect();
    let v: Vec<Vec<f64>> = d.iter().map(|r| (0..nc).map(|c| r[c] / norm[c] * w).collect()).collect();
    let col = |c: usize| v.iter().map(move |r| r[c]);
    let best: Vec<f64> = (0..nc).map(|c| col(c).fold(f64::NAN, f64::min)).collect();
    let worst: Vec<f64> = (0..nc).map(|c| col(c).fold(f64::NAN, f64::max)).collect();
    v.iter()
        .map(|r| {
            let du = (0..nc).map(|c| (r[c] - best[c]).powi(2)).sum::<f64>().sqrt();
            let dl = (0..nc).map(|c| (r[c] - worst[c]).powi(2)).sum::<f64>().sqrt();
            dl / (dl + du)
        })
        .collect()
}

/// The knee (elbow) of a trade-off curve: the point farthest from the straight line between its ends, both axes
/// scaled to 0-1 (the points in the order of x). Its position.
pub fn knee_point(x: &[f64], y: &[f64]) -> usize {
    if x.len() < 3 {
        return 0;
    }
    let mut order: Vec<usize> = (0..x.len()).collect();
    order.sort_by(|&a, &b| x[a].total_cmp(&x[b]));
    let scale = |v: Vec<f64>| {
        let (lo, hi) = v.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(l, h), &x| (l.min(x), h.max(x)));
        let span = hi - lo;
        v.into_iter().map(|x| if span > 0.0 { (x - lo) / span } else { 0.0 }).collect::<Vec<f64>>()
    };
    let xs = scale(order.iter().map(|&k| x[k]).collect());
    let ys = scale(order.iter().map(|&k| y[k]).collect());
    let last = xs.len() - 1;
    let (lx, ly) = (xs[last] - xs[0], ys[last] - ys[0]);
    let length = lx.hypot(ly);
    if length == 0.0 {
        return order[0];
    }
    let dist: Vec<f64> = (0..xs.len()).map(|k| (lx * (ys[k] - ys[0]) - ly * (xs[k] - xs[0])).abs() / length).collect();
    // the first of the farthest (numpy's argmax)
    let far = (0..dist.len()).fold(0, |b, k| if dist[k] > dist[b] { k } else { b });
    order[far]
}

/// A candidate solved: its problem, the solver's result and the units' values.
struct Candidate {
    problem: milp::Problem,
    out: ffi::SolveOut,
}

struct Calibrate;

impl Calibrate {
    /// The node's settings with the penalty traded off at `value` (added to `spatial` when given as a list).
    fn with_penalty(
        params: &serde_json::Map<String, serde_json::Value>,
        penalty: &str,
        value: f64,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        let mut q = params.clone();
        if let Some(serde_json::Value::Array(list)) = q.get_mut("spatial") {
            list.push(serde_json::json!({"kind": if penalty == "linear" { "linear_penalty" } else { penalty }, "penalty": value}));
            return Ok(q);
        }
        let key = match penalty {
            "boundary" => "boundary_penalty",
            "connectivity" => "connectivity_penalty",
            "asym_connectivity" => "asym_connectivity_penalty",
            "linear" => "linear_penalty",
            o => {
                return Err(Error::msg(format!(
                    "unknown penalty {o:?} (boundary, connectivity, asym_connectivity, linear)"
                )));
            }
        };
        q.insert(key.into(), value.into());
        Ok(q)
    }

    fn with_objective(
        params: &serde_json::Map<String, serde_json::Value>,
        objective: &str,
        budget: Option<f64>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut q = params.clone();
        q.insert("objective".into(), objective.into());
        match budget {
            Some(b) => q.insert("budget".into(), b.into()),
            None => q.remove("budget"),
        };
        q
    }
}

impl Operator for Calibrate {
    fn signature(&self) -> OpSignature {
        OpSignature::new(
            "prioritize.calibrate",
            "prioritizr's calibration of trade-offs (its tutorial, Hanson et al.): candidate solutions of a problem \
             (given as to prioritize.optimize, without the penalty traded off) over a trade-off parameter. \
             blended: the problem with a range of penalty weights (default 9 over 1/100 to 100 times Cohon et \
             al.'s penalty); hierarchical: the least penalty within cost thresholds above the least cost \
             solution's; pareto: the least total shortfall of the targets within budgets; cohon: the penalty of \
             Cohon et al. (1979) from the two ideal solutions. Every candidate's cost and criterion (boundary, \
             connectivity, linear penalty or shortfall), and the candidates TOPSIS and the knee of the curve \
             choose. The solves run in parallel.",
            Access::Global,
        )
        .input(PortSpec::optional("features", Type::Stack, "the features' amounts, a layer each"))
        .input(PortSpec::optional("cost", Type::Map, "the cost of each cell (no value: not a planning unit)"))
        .input(PortSpec::optional("pu", Type::Table, "tables instead of maps: as prioritize.optimize's"))
        .input(PortSpec::optional("spec", Type::Table, "Marxan-style: the features (id)"))
        .input(PortSpec::optional("rij", Type::Table, "Marxan-style: the amounts (pu, species, amount)"))
        .input(PortSpec::optional("locked_in", Type::Map, "the units that must be selected (not 0)"))
        .input(PortSpec::optional("locked_out", Type::Map, "the units that may not be selected (not 0)"))
        .input(PortSpec::optional("targets", Type::Table, "targets by hand, as prioritize.optimize's"))
        .input(PortSpec::optional("boundary_data", Type::Table, "a boundary matrix of every unit: i, j, x (1-based)"))
        .input(PortSpec::optional("connectivity_data", Type::Table, "a symmetric connectivity matrix: i, j, x"))
        .input(PortSpec::optional("asym_connectivity_data", Type::Table, "a connectivity matrix: i, j, x"))
        .input(PortSpec::optional("adjacency_data", Type::Table, "an adjacency matrix of every unit: i, j, x"))
        .input(PortSpec::optional("penalty_data", Type::Map, "the linear penalty: a value per cell"))
        .input(PortSpec::optional("constraint_data", Type::Map, "a linear constraint: a value per cell"))
        .input(PortSpec::optional("bounds", Type::Table, "manual bounds, as prioritize.optimize's"))
        .input(PortSpec::optional("status", Type::Table, "manual locked units, as prioritize.optimize's"))
        .output(PortSpec::new(
            "table",
            Type::Table,
            "per candidate: value (the penalty, cost threshold or budget), cost, the criterion (boundary, \
             connectivity, asym_connectivity, linear or shortfall; pareto: met, the targets met), objective, \
             optimal (1), gap, topsis_score, topsis and knee (1: chosen; pareto: efficient). cohon: the two ideal \
             solutions' cost and criterion and the penalty",
        ))
        .output(PortSpec::new("selection", Type::Table, "per unit: its cell or row (1..) and every candidate's share"))
        .output(PortSpec::optional("solutions", Type::Stack, "maps: every candidate, a layer each"))
        .output(PortSpec::optional("best", Type::Map, "maps: the candidate TOPSIS chooses"))
        .param(PortSpec::optional("approach", Type::Text, "blended (default), hierarchical, pareto or cohon"))
        .param(PortSpec::optional(
            "penalty",
            Type::Text,
            "the penalty traded off against the cost: boundary (default), connectivity, asym_connectivity, linear",
        ))
        .param(PortSpec::optional(
            "values",
            Type::Any,
            "the candidates: blended's penalties, hierarchical's cost thresholds, pareto's budgets",
        ))
        .param(PortSpec::optional("n", Type::Integer, "the candidates without values (blended 9, pareto 10)"))
        .param(PortSpec::optional(
            "inflation",
            Type::Any,
            "hierarchical without values: the thresholds the least cost x (1 + these), rounded up (1e-5 to 4)",
        ))
        .param(PortSpec::optional(
            "objective",
            Type::Text,
            "the problem's objective (min_set; pareto: any with targets)",
        ))
        .param(PortSpec::optional("relative_targets", Type::Any, "as prioritize.optimize's"))
        .param(PortSpec::optional("absolute_targets", Type::Any, "as prioritize.optimize's"))
        .param(PortSpec::optional("loglinear_targets", Type::Any, "as prioritize.optimize's"))
        .param(PortSpec::optional("feature_weights", Type::Any, "as prioritize.optimize's"))
        .param(PortSpec::optional("decisions", Type::Text, "as prioritize.optimize's"))
        .param(PortSpec::optional("edge_factor", Type::Number, "the boundary: the weight of edges to no unit (0.5)"))
        .param(PortSpec::optional("cost_column", Type::Text, "tables: as prioritize.optimize's"))
        .param(PortSpec::optional("features", Type::Any, "tables: as prioritize.optimize's"))
        .param(PortSpec::optional("penalty_column", Type::Text, "tables: pu's column of the linear penalty"))
        .param(PortSpec::optional("spatial", Type::Any, "other penalties and constraints, as prioritize.optimize's"))
        .param(PortSpec::optional("gap", Type::Number, "the candidates' optimality gap (default 0.1)"))
        .param(PortSpec::optional("ideal_gap", Type::Number, "the least cost and Cohon's ideal solutions' gap (0)"))
        .param(PortSpec::optional("time_limit", Type::Number, "seconds per solve (default none)"))
    }

    fn run(&self, _ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        use rayon::prelude::*;
        let node = args.node;
        let err = |m: String| Error::msg(format!("node {node:?}: {m}"));
        let approach: String = args.param("approach", "blended".to_string())?;
        let penalty: String = args.param("penalty", "boundary".to_string())?;
        let edge_factor: f64 = args.param("edge_factor", 0.5)?;
        let gap: f64 = args.param("gap", 0.1)?;
        let ideal_gap: f64 = args.param("ideal_gap", 0.0)?;
        let time_limit: f64 = args.opt_param("time_limit")?.unwrap_or(f64::INFINITY);
        let base = args.params.clone();
        let problem = |params: &serde_json::Map<String, serde_json::Value>| {
            problem_of(&Args { node, inputs: args.inputs.clone(), params })
        };
        let solve = |p: milp::Problem, gap: f64| -> Result<Candidate> {
            let (m, _) = milp::compile(&p).map_err(err)?;
            let out = milp::solve(&m, p.units.n(), gap, time_limit).map_err(err)?;
            if !out.has_solution {
                return Err(err(format!("no solution: {}", out.status)));
            }
            Ok(Candidate { problem: p, out })
        };
        let solve_all = |problems: Vec<milp::Problem>| -> Result<Vec<Candidate>> {
            problems
                .into_par_iter()
                .map(|p| {
                    ffi::set_threads(1);
                    solve(p, gap)
                })
                .collect()
        };
        let p0 = problem(&base)?;
        let u = &p0.units;
        if u.zoned() {
            return Err(err("the calibration does not support zones yet".into()));
        }
        let n = u.n();
        let cost_of = |c: &Candidate| u.cost.iter().zip(&c.out.x[..n]).map(|(a, b)| a * b).sum::<f64>();
        // the criterion traded off: of the problem without the penalty
        let bm = matrix_of(args, "boundary_data", u.size)?;
        let criterion = |c: &Candidate| -> Result<f64> {
            let s = &c.out.x[..n];
            match penalty.as_str() {
                "boundary" => milp::evaluate(u, s, edge_factor, bm.as_ref())
                    .map_err(err)?
                    .boundary
                    .ok_or_else(|| err("table units need boundary_data".into())),
                "connectivity" | "asym_connectivity" => {
                    let port = format!("{penalty}_data");
                    let m = matrix_of(args, &port, u.size)?.ok_or_else(|| err(format!("{penalty} needs {port}")))?;
                    Ok(milp::connectivity(u, &m, s, penalty == "asym_connectivity").map_err(err)?.0)
                }
                "linear" => {
                    let q = problem(&Self::with_penalty(&base, "linear", 1.0)?)?;
                    let data = q
                        .spatial
                        .iter()
                        .find_map(|sp| match sp {
                            milp::Spatial::LinearPenalty { data, .. } => Some(data.clone()),
                            _ => None,
                        })
                        .ok_or_else(|| err("the linear penalty needs penalty_data or penalty_column".into()))?;
                    Ok(u.cells.iter().zip(s).map(|(&c, x)| if data[c].is_finite() { data[c] * x } else { 0.0 }).sum())
                }
                o => Err(err(format!("unknown penalty {o:?} (boundary, connectivity, asym_connectivity, linear)"))),
            }
        };
        let min_set = |what: &str| -> Result<()> {
            if p0.objective != milp::Objective::MinSet {
                return Err(err(format!("{what} needs a min set problem (without the penalty)")));
            }
            Ok(())
        };
        let cohon = || -> Result<(f64, [(f64, f64); 2])> {
            min_set("Cohon's penalty")?;
            let least_cost = solve(problem(&base)?, ideal_gap)?;
            let budget = u.cost.iter().sum::<f64>() + 1.0;
            let q = Self::with_objective(&Self::with_penalty(&base, &penalty, 1.0)?, "min_penalties", Some(budget));
            let least_pen = solve(problem(&q)?, ideal_gap)?;
            let m3 = (cost_of(&least_cost), criterion(&least_cost)?);
            let m4 = (cost_of(&least_pen), criterion(&least_pen)?);
            if m3.1 == m4.1 {
                return Err(err(format!("the two ideal solutions have the same {penalty}: no trade-off to calibrate")));
            }
            Ok((milp::r_round(((m3.0 - m4.0) / (m3.1 - m4.1)).abs(), 5), [m3, m4]))
        };
        let values: Option<Vec<f64>> = match args.params.get("values") {
            None | Some(serde_json::Value::Null) => None,
            Some(serde_json::Value::Number(v)) => Some(vec![v.as_f64().unwrap()]),
            Some(v) => Some(serde_json::from_value(v.clone()).map_err(|e| err(format!("values: {e}")))?),
        };
        if values.as_ref().is_some_and(|v| v.is_empty() || v.iter().any(|x| !x.is_finite() || *x < 0.0)) {
            return Err(err("values must be finite and >= 0".into()));
        }
        let (values, candidates, crit_name) = match approach.as_str() {
            "cohon" => {
                let (value, ideals) = cohon()?;
                let mut t = Table::new(["solution", "cost", penalty.as_str(), "penalty"].map(String::from).to_vec());
                t.rows = ideals.iter().enumerate().map(|(k, (c, v))| vec![(k + 1) as f64, *c, *v, value]).collect();
                let mut sel = Table::new(vec!["unit".into()]);
                sel.rows = u.cells.iter().map(|&c| vec![(c + 1) as f64]).collect();
                return Ok(outputs([("table", Value::Table(t)), ("selection", Value::Table(sel))]));
            }
            "blended" => {
                let values = match values {
                    Some(v) => v,
                    None => {
                        let (c, _) = cohon()?;
                        if c <= 0.0 {
                            return Err(err("the Cohon penalty is 0: give the values".into()));
                        }
                        let k: usize = args.param("n", 9)?;
                        linspace(-2.0, 2.0, k).into_iter().map(|e| c * 10f64.powf(e)).collect()
                    }
                };
                let problems = values
                    .iter()
                    .map(|&v| problem(&Self::with_penalty(&base, &penalty, v)?))
                    .collect::<Result<Vec<_>>>()?;
                (values, solve_all(problems)?, penalty.clone())
            }
            "hierarchical" => {
                min_set("the hierarchical approach")?;
                let thresholds = match values {
                    Some(v) => v,
                    None => {
                        let least = solve(problem(&base)?, ideal_gap)?;
                        let c = cost_of(&least);
                        let inflation: Vec<f64> = match args.params.get("inflation") {
                            None | Some(serde_json::Value::Null) => linspace(1e-5, 4.0, 9),
                            Some(v) => serde_json::from_value(v.clone()).map_err(|e| err(format!("inflation: {e}")))?,
                        };
                        inflation.iter().map(|i| (c + c * i).ceil()).collect()
                    }
                };
                if thresholds.iter().any(|&t| t <= 0.0) {
                    return Err(err("the cost thresholds must be > 0".into()));
                }
                let pen = Self::with_penalty(&base, &penalty, 1.0)?;
                let problems = thresholds
                    .iter()
                    .map(|&t| problem(&Self::with_objective(&pen, "min_penalties", Some(t))))
                    .collect::<Result<Vec<_>>>()?;
                (thresholds, solve_all(problems)?, penalty.clone())
            }
            "pareto" => {
                if p0.targets.is_none() {
                    return Err(err("the pareto front needs targets".into()));
                }
                let budgets = match values {
                    Some(v) => v,
                    None => {
                        let k: usize = args.param("n", 10)?;
                        let top = cost_of(&solve(problem(&Self::with_objective(&base, "min_set", None))?, gap)?);
                        // the locked-in units' cost is spent anyway
                        let (m, _) = milp::compile(&p0).map_err(err)?;
                        let fixed: f64 = u.cost.iter().zip(&m.lb[..n]).map(|(c, l)| c * l).sum();
                        linspace(fixed + (top - fixed) / k as f64, top, k)
                    }
                };
                let problems = budgets
                    .iter()
                    .map(|&b| problem(&Self::with_objective(&base, "min_shortfall", Some(b))))
                    .collect::<Result<Vec<_>>>()?;
                (budgets, solve_all(problems)?, "shortfall".to_string())
            }
            o => return Err(err(format!("unknown approach {o:?} (blended, hierarchical, pareto, cohon)"))),
        };
        let pareto = approach == "pareto";
        let mut crit = Vec::with_capacity(candidates.len());
        let mut met = Vec::new();
        for c in &candidates {
            if pareto {
                // min shortfall's: each shortfall as a share of its target
                let spec = c.problem.targets.as_ref().unwrap();
                let t = milp::resolve_targets(spec, &u.totals).map_err(err)?;
                let held = milp::targets_held(u, &t, &c.out.x[..n]);
                let (mut total, mut k_met) = (0.0, 0.0);
                for (k, &h) in held.iter().enumerate() {
                    let target = t.value[k];
                    let short = match t.sense[k].as_str() {
                        ">=" if h >= target => 0.0,
                        "=" if h - target < 1e-10 => 0.0,
                        "<=" if h <= target => 0.0,
                        _ => (target - h).abs(),
                    };
                    total += if target > 0.0 { short / target } else { 0.0 };
                    k_met += f64::from(u8::from(short < 1e-10));
                }
                crit.push(total);
                met.push(k_met);
            } else {
                crit.push(criterion(c)?);
            }
        }
        let costs: Vec<f64> = candidates.iter().map(cost_of).collect();
        let scores = topsis(&costs.iter().zip(&crit).map(|(&a, &b)| vec![a, b]).collect::<Vec<_>>());
        let chosen = (0..scores.len())
            .filter(|&k| !scores[k].is_nan())
            .fold(None, |b: Option<usize>, k| match b {
                Some(j) if scores[j] >= scores[k] => Some(j),
                _ => Some(k),
            })
            .unwrap_or(0);
        let knee = knee_point(&crit, &costs);
        let mut columns: Vec<String> = vec!["value".into(), "cost".into(), crit_name];
        if pareto {
            columns.push("met".into());
        }
        columns.extend(["objective", "optimal", "gap", "topsis_score", "topsis", "knee"].map(String::from));
        if pareto {
            columns.push("efficient".into());
        }
        let mut table = Table::new(columns);
        for (k, c) in candidates.iter().enumerate() {
            let mut row = vec![values[k], costs[k], crit[k]];
            if pareto {
                row.push(met[k]);
            }
            row.extend([
                c.out.objective,
                f64::from(u8::from(c.out.status == "optimal")),
                c.out.gap,
                scores[k],
                f64::from(u8::from(k == chosen)),
                f64::from(u8::from(k == knee)),
            ]);
            if pareto {
                let dominated = (0..candidates.len())
                    .any(|j| costs[j] <= costs[k] && crit[j] <= crit[k] && (costs[j] < costs[k] || crit[j] < crit[k]));
                row.push(f64::from(u8::from(!dominated)));
            }
            table.rows.push(row);
        }
        let mut columns = vec!["unit".to_string()];
        columns.extend((1..=candidates.len()).map(|k| format!("candidate_{k}")));
        let mut selection = Table::new(columns);
        selection.rows = (0..n)
            .map(|i| std::iter::once((u.cells[i] + 1) as f64).chain(candidates.iter().map(|c| c.out.x[i])).collect())
            .collect();
        let mut out = outputs([("table", Value::Table(table)), ("selection", Value::Table(selection))]);
        if u.raster {
            let grid = args.map("cost")?.grid.clone();
            let on_grid = |x: &[f64]| {
                let mut m = vec![f64::NAN; grid.width * grid.height];
                for (i, &c) in u.cells.iter().enumerate() {
                    m[c] = x[i];
                }
                m
            };
            let keys = (1..=candidates.len()).map(|k| Key::Name(format!("candidate_{k}"))).collect();
            let layers = candidates.iter().map(|c| Arc::new(on_grid(&c.out.x[..n]))).collect();
            out.insert("best".into(), Value::Map(Map::new(grid.clone(), on_grid(&candidates[chosen].out.x[..n]))));
            out.insert("solutions".into(), Value::Stack(Stack { grid, keys, layers }));
        }
        Ok(out)
    }
}
