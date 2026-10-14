//! The whole preparation of a species distribution model in one node (landschaft's prepare): the records
//! cleaned, one per cell, thinned; the accessible area; the predictors without collinearity in it; background
//! points in it; the predictors' values at the records; the folds. Every step is the node of its own
//! (`sdm.clean_occurrences`, `sdm.one_per_cell`, `sdm.thin`, `sdm.accessible_area`, `sdm.select_predictors`,
//! `sdm.background`, `sdm.dataset`, the folds), run with the settings given, one seed for the random ones.

use std::sync::Arc;

use serde_json::{Map as JsonMap, Value as Json, json};
use terraflux_graph::{Access, OpSignature, PortSpec, Type};
use terraflux_raster::vector::{Column, Geometry, Vector};

use super::{bad, coordinate_settings, key_name, layers_of, occurrences};
use crate::{Args, Ctx, Operator, Outputs, Registry, Result, Value, outputs};

const FOLD_METHODS: [&str; 6] = ["blocks", "quadrants", "checkerboard", "random", "jackknife", "clusters"];

/// A step's settings: None (skip it) for false or null, {} for true, {main: v} for a number, or the object.
fn step(args: &Args, name: &str, default: Json, main: Option<&str>) -> Result<Option<JsonMap<String, Json>>> {
    let v = args.params.get(name).cloned().unwrap_or(default);
    match v {
        Json::Null | Json::Bool(false) => Ok(None),
        Json::Bool(true) => Ok(Some(JsonMap::new())),
        Json::Object(o) => Ok(Some(o)),
        Json::Number(_) if main.is_some() => Ok(Some(JsonMap::from_iter([(main.unwrap().to_string(), v)]))),
        v => Err(bad(args, format!("{name}: false, true, a number or an object of settings, not {v}"))),
    }
}

/// One step: an operator of the registry run on its inputs.
struct Steps<'a> {
    registry: Registry,
    args: &'a Args<'a>,
}

impl Steps<'_> {
    fn run(
        &self,
        ctx: &mut Ctx,
        op: &str,
        inputs: Vec<(&str, Value)>,
        params: JsonMap<String, Json>,
    ) -> Result<Outputs> {
        let node = format!("{} ({op})", self.args.node);
        let a = Args {
            node: &node,
            inputs: inputs.into_iter().map(|(k, v)| (k.to_string(), v)).collect(),
            params: &params,
        };
        self.registry.get(op).expect("an operator of the registry").run(ctx, &a)
    }
}

fn records_len(args: &Args, v: &Value) -> Result<usize> {
    let a = Args { node: args.node, inputs: [("r".to_string(), v.clone())].into(), params: args.params };
    Ok(occurrences(&a, "r")?.len())
}

/// The extent of a grid as a polygon in its coordinate system.
fn extent(args: &Args, env: &Value) -> Result<Vector> {
    let (grid, _) =
        layers_of(&Args { node: args.node, inputs: [("e".into(), env.clone())].into(), params: args.params }, "e")?;
    let (x0, x1, y0, y1, ..) = super::points::grid_of(&grid);
    Ok(Vector {
        geometries: vec![Geometry::Polygon(vec![vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1], [x0, y0]]])],
        attributes: vec![("method".into(), Column::Texts(vec!["extent".into()]))],
        crs: super::points::grid_crs(&grid),
    })
}

fn names_of(v: &Value) -> Vec<String> {
    match v {
        Value::Stack(s) => s.keys.iter().map(key_name).collect(),
        _ => vec!["layer".into()],
    }
}

pub(super) struct Prepare;

impl Operator for Prepare {
    fn signature(&self) -> OpSignature {
        coordinate_settings(
            OpSignature::new(
                "sdm.prepare",
                "The preparation of a species distribution model in one node (landschaft's prepare): the records \
                 cleaned (CoordinateCleaner's tests), one per cell, thinned; the accessible area; the predictors \
                 without collinearity in it; background points in it; the predictors' values at the records; the \
                 folds. Each step takes false (skip it), true, a number (its main setting) or an object of the \
                 settings of its node; one seed for every random step. The report counts what each step removed.",
                Access::Global,
            )
            .input(PortSpec::new("occurrences", Type::Any, "the records: points, or a table with coordinate columns"))
            .input(PortSpec::new("env", Type::Any, "the predictors: a stack (by name) or a map"))
            .input(PortSpec::optional("area", Type::Vector, "the accessible area, in place of the area step"))
            .input(PortSpec::optional("background", Type::Any, "background points, in place of the background step"))
            .input(PortSpec::optional("regions", Type::Vector, "regions for the area step (method regions)"))
            .input(PortSpec::optional("within", Type::Vector, "polygons the area is cut to"))
            .output(PortSpec::new("records", Type::Table, "x, y, presence, the predictors and the fold"))
            .output(PortSpec::new("env", Type::Any, "the predictors kept"))
            .output(PortSpec::new("area", Type::Vector, "the accessible area (else the predictors' extent)"))
            .output(PortSpec::new("presences", Type::Any, "the records left"))
            .output(PortSpec::new("background_points", Type::Any, "the background points"))
            .output(PortSpec::new("report", Type::Any, "per step: the presences, those removed and why"))
            .output(PortSpec::new("settings", Type::Any, "every setting used")),
        )
        .param(PortSpec::optional("species", Type::Text, "keep only this species (the species column)"))
        .param(PortSpec::optional("clean", Type::Any, "sdm.clean_occurrences' settings (default true)"))
        .param(PortSpec::optional("one_per_cell", Type::Bool, "a record per cell of the predictors (default true)"))
        .param(PortSpec::optional("thin", Type::Any, "sdm.thin: the distance in metres or its settings (off)"))
        .param(PortSpec::optional(
            "area",
            Type::Any,
            "sdm.accessible_area: the buffer's distance in metres or its settings (off: the whole extent)",
        ))
        .param(PortSpec::optional("predictors", Type::Any, "sdm.select_predictors: the VIF threshold (10) or settings"))
        .param(PortSpec::optional("background", Type::Any, "sdm.background: the points (10000) or settings"))
        .param(PortSpec::optional(
            "folds",
            Type::Any,
            "k spatial blocks (5), or {method: blocks, quadrants, checkerboard, random, jackknife or clusters, and \
             its settings}; blocks take size \"auto\" (the predictors' range of autocorrelation)",
        ))
        .param(PortSpec::optional("seed", Type::Integer, "the seed of every random step (default 1)"))
    }

    fn run(&self, ctx: &mut Ctx, args: &Args) -> Result<Outputs> {
        let s = Steps { registry: Registry::standard(), args };
        let seed: i64 = args.param("seed", 1)?;
        let env = args.get("env").cloned().ok_or_else(|| bad(args, "connect the predictors (env)"))?;
        if !matches!(env, Value::Stack(_) | Value::Map(_)) {
            return Err(bad(args, "connect the predictors (env): a stack or a map"));
        }
        let mut records = args.get("occurrences").cloned().ok_or_else(|| bad(args, "connect the occurrences"))?;
        let coords: JsonMap<String, Json> =
            ["x", "y"].iter().filter_map(|k| args.params.get(*k).map(|v| (k.to_string(), v.clone()))).collect();
        let with_coords = |mut p: JsonMap<String, Json>| {
            for (k, v) in &coords {
                p.entry(k.clone()).or_insert_with(|| v.clone());
            }
            p
        };
        let mut report: Vec<Json> = Vec::new();
        let mut settings = JsonMap::from_iter([("seed".to_string(), json!(seed))]);
        let mut note = |step: &str, before: usize, after: usize, detail: String| {
            report.push(json!({"step": step, "presences": after, "removed": before as i64 - after as i64,
                               "detail": detail}));
        };

        if let Some(sp) = args.opt_param::<String>("species")? {
            let occ = occurrences(args, "occurrences")?;
            let cols = occ.columns();
            let col = ["species", "scientificName", "label"]
                .iter()
                .find(|c| cols.iter().any(|n| n == *c))
                .ok_or_else(|| bad(args, "species needs a species column (species, scientificName or label)"))?;
            let keep: Vec<bool> = match occ.column(col).unwrap() {
                Column::Texts(t) => t.iter().map(|v| *v == sp).collect(),
                Column::Numbers(x) => x.iter().map(|v| sp.parse::<f64>().is_ok_and(|s| s == *v)).collect(),
            };
            if !keep.iter().any(|&k| k) {
                return Err(bad(args, format!("no record of {sp:?}")));
            }
            records = super::occurrences_out(occ.subset(&keep));
        }
        let n = records_len(args, &records)?;
        note("input", n, n, String::new());

        if let Some(mut p) = step(args, "clean", Json::Bool(true), None)? {
            settings.insert("clean".into(), Json::Object(p.clone()));
            p.insert("action".into(), "drop".into());
            let before = records_len(args, &records)?;
            let out = s.run(ctx, "sdm.clean_occurrences", vec![("occurrences", records)], with_coords(p))?;
            records = out["occurrences"].clone();
            let detail = match &out["counts"] {
                Value::Table(t) => t
                    .columns
                    .iter()
                    .zip(&t.rows[0])
                    .filter(|(k, v)| **v > 0.0 && *k != "total")
                    .map(|(k, v)| format!("{k} {v}"))
                    .collect::<Vec<_>>()
                    .join(", "),
                _ => String::new(),
            };
            note("clean", before, records_len(args, &records)?, detail);
        }

        if args.param("one_per_cell", true)? {
            let before = records_len(args, &records)?;
            let out = s.run(
                ctx,
                "sdm.one_per_cell",
                vec![("occurrences", records), ("raster", env.clone())],
                with_coords(JsonMap::new()),
            )?;
            records = out["occurrences"].clone();
            note("one per cell", before, records_len(args, &records)?, String::new());
        }

        if let Some(mut p) = step(args, "thin", Json::Null, Some("distance"))? {
            p.entry("seed").or_insert(json!(seed));
            settings.insert("thin".into(), Json::Object(p.clone()));
            let before = records_len(args, &records)?;
            let detail = p.get("distance").map(|d| format!("{d} m")).unwrap_or_default();
            let out = s.run(ctx, "sdm.thin", vec![("occurrences", records)], with_coords(p))?;
            records = out["occurrences"].clone();
            note("thin", before, records_len(args, &records)?, detail);
        }

        let area: Option<Value> = match args.get("area") {
            Some(a) => Some(a.clone()),
            None => match step(args, "area", Json::Null, Some("distance"))? {
                Some(p) => {
                    settings.insert("area".into(), Json::Object(p.clone()));
                    let mut inputs = vec![("occurrences", records.clone())];
                    for port in ["regions", "within"] {
                        if let Some(v) = args.get(port) {
                            inputs.push((port, v.clone()));
                        }
                    }
                    Some(s.run(ctx, "sdm.accessible_area", inputs, with_coords(p))?["area"].clone())
                }
                None => None,
            },
        };
        let n = records_len(args, &records)?;
        note("area", n, n, if area.is_none() { "whole extent of the predictors".into() } else { String::new() });

        let env_used = match step(args, "predictors", json!(10.0), Some("vif"))? {
            Some(mut p) => {
                p.entry("seed").or_insert(json!(seed));
                settings.insert("predictors".into(), Json::Object(p.clone()));
                let mut inputs = vec![("env", env.clone())];
                if let Some(a) = &area {
                    inputs.push(("area", a.clone()));
                }
                let kept = s.run(ctx, "sdm.select_predictors", inputs, p)?["env"].clone();
                let names = names_of(&kept);
                let dropped: Vec<String> = names_of(&env).into_iter().filter(|b| !names.contains(b)).collect();
                note(
                    "predictors",
                    n,
                    n,
                    if dropped.is_empty() { String::new() } else { format!("dropped {}", dropped.join(", ")) },
                );
                kept
            }
            None => env.clone(),
        };

        let background: Option<Value> = match args.get("background") {
            Some(b) => Some(b.clone()),
            None => match step(args, "background", json!(10_000), Some("n"))? {
                Some(mut p) => {
                    p.entry("seed").or_insert(json!(seed));
                    settings.insert("background".into(), Json::Object(p.clone()));
                    let mut inputs = vec![("env", env_used.clone()), ("occurrences", records.clone())];
                    if let Some(a) = &area {
                        inputs.push(("area", a.clone()));
                    }
                    Some(s.run(ctx, "sdm.background", inputs, with_coords(p))?["points"].clone())
                }
                None => None,
            },
        };
        let nb = match &background {
            Some(b) => records_len(args, b)?,
            None => 0,
        };
        note("background", n, n, format!("{nb} points"));

        let mut inputs = vec![("presences", records.clone()), ("env", env_used.clone())];
        if let Some(b) = &background {
            inputs.push(("background", b.clone()));
        }
        let mut data = s.run(ctx, "sdm.dataset", inputs, with_coords(JsonMap::new()))?["records"].clone();
        let Value::Table(t) = &data else { unreachable!() };
        let presences = t.rows.iter().filter(|r| r[2] == 1.0).count();
        let background_rows = t.rows.iter().filter(|r| r[2] == 0.0).count();
        let removed = n + nb - t.rows.len();
        note(
            "predictor values",
            n,
            presences,
            if removed > 0 { "no value in some predictor".into() } else { String::new() },
        );

        if let Some(mut p) = step(args, "folds", json!(5), Some("k"))? {
            let method = match p.remove("method") {
                None => "blocks".to_string(),
                Some(Json::String(m)) => m,
                Some(m) => return Err(bad(args, format!("folds method: a text, not {m}"))),
            };
            if !FOLD_METHODS.contains(&method.as_str()) {
                return Err(bad(args, format!("folds method must be one of {}", FOLD_METHODS.join(", "))));
            }
            let random = matches!(method.as_str(), "blocks" | "random" | "clusters");
            if random {
                p.entry("seed").or_insert(json!(seed));
            }
            let op = match method.as_str() {
                "blocks" => "sdm.spatial_blocks",
                "quadrants" => "sdm.quadrant_folds",
                "checkerboard" => "sdm.checkerboard_folds",
                "random" => "sdm.random_folds",
                "jackknife" => "sdm.jackknife_folds",
                _ => "sdm.spatial_clusters",
            };
            let mut inputs = vec![("records", data.clone())];
            if method == "checkerboard" {
                inputs.push(("env", env_used.clone()));
            }
            let out = if method == "blocks" && p.get("size").and_then(Json::as_str) == Some("auto") {
                let r = s.run(ctx, "sdm.autocorrelation_range", vec![("env", env_used.clone())], {
                    JsonMap::from_iter([("seed".to_string(), json!(seed))])
                })?;
                let Value::Number(range) = r["range"] else { unreachable!() };
                p.insert("size".into(), json!(range));
                match s.run(ctx, op, inputs.clone(), p.clone()) {
                    Ok(o) => o,
                    Err(_) => {
                        ctx.warn(format!(
                            "node {:?}: blocks of the autocorrelation range ({:.0} km) are too large for the folds: \
                             using the default grid of blocks",
                            args.node,
                            range / 1000.0
                        ));
                        p.remove("size");
                        s.run(ctx, op, inputs, p.clone())?
                    }
                }
            } else {
                s.run(ctx, op, inputs, p.clone())?
            };
            data = out["records"].clone();
            p.insert("method".into(), json!(method));
            settings.insert("folds".into(), Json::Object(p));
            let Value::Table(t) = &data else { unreachable!() };
            let f = t.columns.iter().position(|c| c == "fold").unwrap();
            let mut ks: Vec<i64> = t.rows.iter().map(|r| r[f] as i64).filter(|&k| k > 0).collect();
            ks.sort_unstable();
            ks.dedup();
            note("folds", presences, presences, format!("{} folds", ks.len()));
        }

        settings.insert("n_presences".into(), json!(presences));
        settings.insert("n_background".into(), json!(background_rows));
        settings.insert("predictors_used".into(), json!(names_of(&env_used)));
        let area = match area {
            Some(a) => a,
            None => Value::Vector(Arc::new(extent(args, &env_used)?)),
        };
        let mut out = outputs([
            ("records", data),
            ("env", env_used),
            ("area", area),
            ("presences", records),
            ("report", Value::Json(Json::Array(report))),
            ("settings", Value::Json(Json::Object(settings))),
        ]);
        if let Some(b) = background {
            out.insert("background_points".into(), b);
        }
        Ok(out)
    }
}
