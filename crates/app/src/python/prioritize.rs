//! The Python counterparts of the operators of prioritization and restoration (prioritize, restoration).
//!
//! prioritize.optimize, prioritize.zonation and the three by-name operators (decay_connectivity, protected_cells,
//! refine) are in python.rs. The matrices of units (boundary, adjacency, connectivity, proximity) are scipy sparse
//! matrices in the library and tables `i, j, x` (1-based) in Terraflux: the table is made beside the matrix
//! (`<node>_table`). The problem the evaluation or the calibration needs is built as prioritize.optimize builds it.

#![allow(unused_imports)]

use super::*;

/// The operators of this family called with their arguments mapped.
pub(super) const MAPPED: &[Py] = &[
    Py {
        op: "prioritize.coarsen",
        call: "tf.prioritize.coarsen",
        args: &[("cost", ""), ("features", "features"), ("factor", "factor")],
        defaults: &[],
        outs: &[("cost", "[0]"), ("features", "[1]")],
        kind: Kind::Plain,
    },
    Py {
        op: "prioritize.zonation_project",
        call: "tf.prioritize.run_project",
        args: &[("path", ""), ("flags", "flags"), ("mode", "mode"), ("output", "output"), ("bits", "bits")],
        defaults: &[],
        outs: &[
            ("rank", ".rank"),
            ("curves", ".summary_curves"),
            ("features", ".features"),
            ("auc", ".auc"),
            ("iterations", ".iterations"),
        ],
        kind: Kind::Plain,
    },
    Py {
        op: "restoration.intactness",
        call: "tf.intactness",
        args: &[
            ("footprint", ""),
            ("radius", "radius"),
            ("gamma", "gamma"),
            ("beta", "beta"),
            ("z", "z"),
            ("cell_size", "cell_size"),
            ("reduction", "reduction"),
            ("restored", "restored"),
        ],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "restoration.intactness_gradient",
        call: "tf.intactness_gradient",
        args: &[
            ("footprint", ""),
            ("reduction", "reduction"),
            ("radius", "radius"),
            ("gamma", "gamma"),
            ("beta", "beta"),
            ("z", "z"),
            ("cell_size", "cell_size"),
            ("restored", "restored"),
        ],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Plain,
    },
    Py {
        op: "restoration.resistance",
        call: "tf.distance_to_resistance",
        args: &[("map", "")],
        defaults: &[],
        outs: &[("map", "")],
        kind: Kind::Habitat,
    },
];

/// The operators of this family translated by code of their own (as `special` in python.rs); `None`: not one of them.
pub(super) fn special(n: &Node, var: &str, sc: &Scope) -> Result<Option<Code>, String> {
    let code = match n.op.as_str() {
        "prioritize.boundary_matrix"
        | "prioritize.adjacency_matrix"
        | "prioritize.connectivity_matrix"
        | "prioritize.proximity_matrix" => matrix(n, var, sc)?,
        "prioritize.unit_points" => unit_points(n, var, sc)?,
        "prioritize.h3_units" => h3_units(n, var, sc)?,
        "prioritize.binary_stack" => binary_stack(n, var, sc)?,
        "prioritize.marxan" => marxan(n, var, sc)?,
        "prioritize.proxirr" => proxirr(n, var, sc)?,
        "prioritize.evaluate" => evaluate(n, var, sc)?,
        "prioritize.calibrate" => calibrate(n, var, sc)?,
        "prioritize.actions" => actions(n, var, sc, false)?,
        "prioritize.actions_raster" => actions(n, var, sc, true)?,
        "restoration.status" => status(n, var, sc)?,
        "restoration.cost_distance" => cost_distance(n, var, sc)?,
        "restoration.least_cost_path" => least_cost_path(n, var, sc)?,
        "restoration.corridor" => corridor(n, var, sc)?,
        "restoration.isochrone" => isochrone(n, var, sc)?,
        "restoration.original_vegetation" => original_vegetation(n, var, sc)?,
        "restoration.ecosystem_areas" => ecosystem_areas(n, var, sc)?,
        "restoration.ecosystem_targets" => ecosystem_targets(n, var, sc)?,
        "restoration.plan" => plan(n, var, sc)?,
        _ => return Ok(None),
    };
    Ok(Some(code))
}

// --- helpers

/// An input or setting that has to be there.
fn need(n: &Node, sc: &Scope, k: &str, what: &str) -> Result<String, String> {
    value_of(n, k, sc)?.ok_or_else(|| format!("{what} is not connected"))
}

/// Whether the node gives the input or the setting (a null setting is none).
fn has(n: &Node, k: &str) -> bool {
    n.inputs.contains_key(k) || n.params.get(k).is_some_and(|v| !v.is_null())
}

/// The inputs and settings given among `names` (ours, theirs), as `theirs=value`.
fn kw(n: &Node, sc: &Scope, names: &[(&str, &str)]) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for (ours, theirs) in names {
        if has(n, ours)
            && let Some(v) = value_of(n, ours, sc)?
        {
            out.push(format!("{theirs}={v}"));
        }
    }
    Ok(out)
}

/// The node's outputs: the port and its expression.
fn outs(pairs: &[(&str, String)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.clone())).collect()
}

/// The classes of a setting (a name or a list) as a Python list of texts.
fn class_list(n: &Node, sc: &Scope, key: &str) -> Result<Option<String>, String> {
    let Some(v) = n.params.get(key).filter(|v| !v.is_null()) else { return Ok(None) };
    if terraflux_graph::setting_ref(v).is_some() {
        return setting(n, key, sc);
    }
    Ok(Some(match v {
        Value::String(s) => format!("[{}]", text(s)),
        Value::Array(a) => format!(
            "[{}]",
            a.iter()
                .map(|x| match x {
                    Value::String(s) => text(s),
                    x => text(&x.to_string()),
                })
                .collect::<Vec<_>>()
                .join(", ")
        ),
        v => return Err(format!("{key} = {}: a class or a list of them", literal(v))),
    }))
}

/// A habitat map in GuidosToolbox's coding (0 missing, 1 background, 2 habitat), as the habitat operators do it.
fn habitat(n: &Node, sc: &Scope, port: &str) -> Result<String, String> {
    let m = need(n, sc, port, &format!("its {port}"))?;
    Ok(match value_of(n, "foreground", sc)? {
        Some(fg) => format!("tf.reclassify({m}, foreground={fg})"),
        None => format!("{m}.fillna(0).astype(\"uint8\")"),
    })
}

/// A map of bytes (a resistance, a habitat): no value is 0.
fn bytes(n: &Node, sc: &Scope, port: &str) -> Result<String, String> {
    Ok(format!("{}.fillna(0).astype(\"uint8\")", need(n, sc, port, &format!("its {port}"))?))
}

// --- matrices of units

fn matrix(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    let units = need(n, sc, "units", "its units")?;
    let name = n.op.trim_start_matches("prioritize.");
    let mut args = vec![units];
    match n.op.as_str() {
        "prioritize.boundary_matrix" => refuse_others(n, &["units"])?,
        "prioritize.adjacency_matrix" => {
            refuse_others(n, &["units", "directions"])?;
            args.extend(kw(n, sc, &[("directions", "directions")])?);
        }
        "prioritize.connectivity_matrix" => {
            refuse_others(n, &["units", "y", "column"])?;
            // polygons: an attribute of them, or a map averaged over each; cells: a map
            let y = match (n.inputs.contains_key("y"), has(n, "column")) {
                (true, true) => return Err("y and column: one of them".into()),
                (true, false) => need(n, sc, "y", "its y")?,
                (false, true) => need(n, sc, "column", "its column")?,
                (false, false) => return Err("its y is not connected".into()),
            };
            args.push(format!("y={y}"));
        }
        _ => {
            refuse_others(n, &["units", "distance"])?;
            let d = need(n, sc, "distance", "its distance")?;
            args.push(format!("distance={d}"));
        }
    }
    let code = format!(
        "{var} = tf.prioritize.{name}({})\nimport pandas as _pd\n{var}_table = _pd.DataFrame({{\"i\": {var}.tocoo().row + 1, \"j\": {var}.tocoo().col + 1, \"x\": {var}.tocoo().data}})\n",
        args.join(", ")
    );
    Ok((code, outs(&[("table", format!("{var}_table"))]), Need::Nothing))
}

fn unit_points(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["units"])?;
    let u = need(n, sc, "units", "its units")?;
    let code = format!(
        "{var} = tf.prioritize.unit_points({u})\nimport geopandas as _gpd\n{var}_points = _gpd.GeoDataFrame(\n    {{\"unit\": range(1, len({var}) + 1)}},\n    geometry=_gpd.points_from_xy({var}[:, 0], {var}[:, 1]),\n    crs=({u}.rio.crs if hasattr({u}, \"rio\") else {u}.crs),\n)\n"
    );
    Ok((code, outs(&[("points", format!("{var}_points"))]), Need::Nothing))
}

fn h3_units(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["cost", "resolution"])?;
    let cost = need(n, sc, "cost", "its cost")?;
    let r = need(n, sc, "resolution", "its resolution")?;
    let code = format!(
        "{var} = tf.prioritize.h3_units({cost}, {r})\nimport pandas as _pd\n{var}_table = _pd.DataFrame({{\"unit\": range(1, len({var}) + 1), \"cost\": {var}[\"cost\"].to_numpy()}})\n"
    );
    Ok((code, outs(&[("units", var.to_string()), ("table", format!("{var}_table"))]), Need::Nothing))
}

fn binary_stack(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["zones", "names"])?;
    let zones = need(n, sc, "zones", "its zones")?;
    let names = kw(n, sc, &[("names", "zones")])?;
    let sep = if names.is_empty() { "" } else { ", " };
    // the library's mapping zone -> raster as a stack with a layer per zone
    let code = format!(
        "import xarray as _xr\n{var}_zones = tf.prioritize.binary_stack({zones}{sep}{})\n{var} = _xr.concat(list({var}_zones.values()), dim=\"band\").assign_coords(band=list({var}_zones))\n",
        names.join(", ")
    );
    Ok((code, outs(&[("stack", var.to_string())]), Need::Nothing))
}

// --- Marxan and ProxIRR (from Marxan's tables)

fn marxan(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    if ["features", "cost", "status"].iter().any(|k| n.inputs.contains_key(*k)) {
        return Err("from maps: not in the script yet (the library's marxan takes Marxan's tables)".into());
    }
    refuse_others(
        n,
        &[
            "pu",
            "spec",
            "puvspr",
            "bound",
            "blm",
            "prop",
            "num_reps",
            "num_iters",
            "start_temp",
            "cool_fac",
            "num_temp",
            "run_mode",
            "heur_type",
            "itimp_type",
            "misslevel",
            "cost_thresh",
            "thresh_pen1",
            "thresh_pen2",
            "connectivity_in",
            "seed",
        ],
    )?;
    if n.params.get("prop").is_some_and(|p| !p.is_number() && !p.is_null()) {
        return Err("prop as a list is for maps".into());
    }
    let mut args =
        vec![need(n, sc, "pu", "its pu")?, need(n, sc, "spec", "its spec")?, need(n, sc, "puvspr", "its puvspr")?];
    args.extend(kw(
        n,
        sc,
        &[
            ("bound", "bound"),
            ("blm", "blm"),
            ("num_reps", "num_reps"),
            ("num_iters", "num_iters"),
            ("start_temp", "start_temp"),
            ("cool_fac", "cool_fac"),
            ("num_temp", "num_temp"),
            ("run_mode", "run_mode"),
            ("heur_type", "heur_type"),
            ("itimp_type", "itimp_type"),
            ("prop", "prop"),
            ("misslevel", "misslevel"),
            ("cost_thresh", "cost_thresh"),
            ("thresh_pen1", "thresh_pen1"),
            ("thresh_pen2", "thresh_pen2"),
            ("connectivity_in", "connectivity_in"),
            ("seed", "seed"),
        ],
    )?);
    let code = format!("{var} = tf.prioritize.marxan({})\n", args.join(", "));
    // the tables as Terraflux names their columns; its summary's cost and penalty are the final ones, the library
    // only gives the run's incremental ones (not the same numbers): those two columns are left out
    let o = outs(&[
        (
            "summary",
            format!(
                "{var}.summary[[\"Run_Number\", \"Score\", \"Planning_Units\", \"Connectivity\", \"Shortfall\", \"Missing_Values\", \"MPM\", \"Final_Score\"]].rename(columns={{\"Run_Number\": \"run\", \"Score\": \"score\", \"Planning_Units\": \"planning_units\", \"Connectivity\": \"connectivity\", \"Shortfall\": \"shortfall\", \"Missing_Values\": \"missing\", \"MPM\": \"mpm\", \"Final_Score\": \"final_score\"}})"
            ),
        ),
        ("best", format!("{var}.best.rename(columns={{\"planning_unit\": \"pu\"}})")),
        (
            "solutions",
            format!(
                "{var}.solutions.rename(columns=lambda c: \"pu\" if c == \"planning_unit\" else c.replace(\"solution_\", \"run_\"))"
            ),
        ),
        ("frequency", format!("{var}.frequency.rename(columns={{\"planning_unit\": \"pu\"}})")),
        ("penalties", format!("{var}.penalties.rename(columns={{\"spid\": \"feature\"}})")),
    ]);
    Ok((code, o, Need::Nothing))
}

fn proxirr(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    if n.inputs.contains_key("features") {
        return Err("from maps: not in the script yet (the library's eval_proxirr_marxan takes Marxan's tables)".into());
    }
    refuse_others(n, &["pu", "spec", "puvspr", "triage"])?;
    let mut args =
        vec![need(n, sc, "pu", "its pu")?, need(n, sc, "spec", "its spec")?, need(n, sc, "puvspr", "its puvspr")?];
    args.extend(kw(n, sc, &[("triage", "triage")])?);
    let code = format!(
        "{var} = tf.prioritize.eval_proxirr_marxan({})\nimport pandas as _pd\n{var}_units = _pd.DataFrame({{\"pu\": {var}.beta[\"id\"], \"beta\": {var}.beta[\"beta\"], \"gamma\": {var}.gamma[\"gamma\"]}})\n",
        args.join(", ")
    );
    let o = outs(&[
        ("units", format!("{var}_units")),
        ("alpha", format!("{var}.alpha")),
        ("targets", format!("{var}.targets[[\"id\", \"global\", \"target\"]].rename(columns={{\"id\": \"feature\"}})")),
    ]);
    Ok((code, o, Need::Nothing))
}

// --- the problem of rasters, for the evaluation and the calibration

/// The lines of the chain that builds the problem of the node's cost and features: its objective, targets,
/// decisions and solver, as prioritize.optimize's.
fn problem_chain(n: &Node, sc: &Scope, weights: bool) -> Result<Vec<String>, String> {
    let cost = need(n, sc, "cost", "its cost")?;
    let features = need(n, sc, "features", "its features")?;
    let mut chain = vec![format!("tf.prioritize.problem({cost}, {features})")];
    let objective = choice(n, "objective").unwrap_or("min_set");
    match OBJECTIVES.iter().find(|(o, _)| *o == objective) {
        Some((o, false)) => chain.push(format!(".add_{o}_objective()")),
        Some((o, true)) => {
            let budget = setting(n, "budget", sc)?.ok_or_else(|| format!("{o} needs a budget"))?;
            chain.push(format!(".add_{o}_objective({budget})"));
        }
        None => return Err(format!("the objective {objective:?}: not in the script yet")),
    }
    if has(n, "loglinear_targets") {
        return Err("loglinear_targets: not in the script yet".into());
    }
    let mut targets = vec![("relative_targets", "add_relative_targets"), ("absolute_targets", "add_absolute_targets")];
    if weights {
        targets.push(("feature_weights", "add_feature_weights"));
    }
    for (k, method) in targets {
        if has(n, k)
            && let Some(x) = setting(n, k, sc)?
        {
            chain.push(format!(".{method}({x})"));
        }
    }
    for (k, method) in [("locked_in", "add_locked_in_constraints"), ("locked_out", "add_locked_out_constraints")] {
        if n.inputs.contains_key(k)
            && let Some(x) = value_of(n, k, sc)?
        {
            chain.push(format!(".{method}({x})"));
        }
    }
    match choice(n, "decisions").unwrap_or("binary") {
        "binary" => chain.push(".add_binary_decisions()".into()),
        "proportion" => chain.push(".add_proportion_decisions()".into()),
        d => return Err(format!("the decisions {d:?}: not in the script yet")),
    }
    let solver = choice(n, "solver").unwrap_or("highs");
    let mut s = given(n, &[("gap", "gap"), ("time_limit", "time_limit")], sc)?;
    match solver {
        "highs" => {}
        "gurobi" => s.extend(given(n, &[("threads", "n_threads")], sc)?),
        o => return Err(format!("the solver {o:?}: not in the script yet")),
    }
    if !s.is_empty() || solver != "highs" {
        chain.push(format!(".add_{solver}_solver({})", s.join(", ")));
    }
    Ok(chain)
}

/// The matrix of units of a table of pairs the node is given (as prioritize.optimize's `_unit_pairs`), or none.
fn matrix_data(n: &Node, sc: &Scope, port: &str, cost: &str) -> Result<Option<String>, String> {
    if !n.inputs.contains_key(port) {
        return Ok(None);
    }
    Ok(value_of(n, port, sc)?.map(|t| format!("_unit_pairs({t}, {cost}.size)")))
}

fn evaluate(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(
        n,
        &[
            "features",
            "cost",
            "solution",
            "selection",
            "boundary_data",
            "connectivity_data",
            "asym_connectivity_data",
            "relative_targets",
            "absolute_targets",
            "edge_factor",
            "rescale",
            "replacement",
            "rank",
            "budgets",
            "objective",
            "budget",
            "gap",
            "solver",
            "threads",
            "extinction_z",
        ],
    )?;
    let cost = need(n, sc, "cost", "its cost")?;
    let solution = match (value_of(n, "solution", sc)?, value_of(n, "selection", sc)?) {
        (Some(s), None) | (None, Some(s)) => s,
        (Some(_), Some(_)) => return Err("solution and selection: one of them".into()),
        (None, None) => return Err("its solution is not connected".into()),
    };
    let chain = problem_chain(n, sc, false)?;
    let (p, s) = (format!("{var}_problem"), format!("{var}_solution"));
    let mut code = format!("{p} = (\n    {}\n)\n{s} = {solution}\n", chain.join("\n    "));
    // the summary: the units, their cost and boundary, and the connectivity with its matrices
    let mut boundary = given(n, &[("edge_factor", "edge_factor")], sc)?;
    if let Some(d) = matrix_data(n, sc, "boundary_data", &cost)? {
        boundary.push(format!("data={d}"));
    }
    let tail = |a: &[String]| if a.is_empty() { String::new() } else { format!(", {}", a.join(", ")) };
    let mut parts = vec![
        format!("tf.prioritize.eval_n_summary({p}, {s})"),
        format!("tf.prioritize.eval_cost_summary({p}, {s})"),
        format!("tf.prioritize.eval_boundary_summary({p}, {s}{})", tail(&boundary)),
    ];
    for (port, f) in [
        ("connectivity_data", "eval_connectivity_summary"),
        ("asym_connectivity_data", "eval_asym_connectivity_summary"),
    ] {
        if let Some(d) = matrix_data(n, sc, port, &cost)? {
            parts.push(format!("tf.prioritize.{f}({p}, {s}, {d})"));
        }
    }
    let parts: Vec<String> = parts.iter().map(|x| format!("{x}.drop(columns=\"summary\")")).collect();
    code.push_str(&format!(
        "{var}_summary = _pd.concat(\n    [\n        {}\n    ],\n    axis=1,\n)\n",
        parts.join(",\n        ")
    ));
    code.push_str(&format!(
        "{var}_representation = tf.prioritize.eval_feature_representation_summary({p}, {s}).drop(columns=\"summary\").assign(feature=range(1, len({p}.units.feature_names) + 1))\n"
    ));
    // the features by their position (1..), as Terraflux numbers them; the total row's name has none (NaN)
    let index = format!(
        ".assign(feature=lambda d: d[\"feature\"].map({{n: i + 1 for i, n in enumerate({p}.units.feature_names)}}))"
    );
    let rescale = given(n, &[("rescale", "rescale")], sc)?;
    let mut o = vec![
        ("summary", format!("{var}_summary")),
        ("representation", format!("{var}_representation")),
        ("rarity", format!("tf.prioritize.eval_rare_richness_importance({p}, {s}{})", tail(&rescale))),
    ];
    // with targets: their coverage and Ferrier's irreplaceability
    if has(n, "relative_targets") || has(n, "absolute_targets") {
        o.push(("coverage", format!("tf.prioritize.eval_target_coverage_summary({p}, {s}){index}")));
        o.push(("ferrier", format!("tf.prioritize.eval_ferrier_importance({p}, {s})")));
    }
    if n.params.get("replacement").and_then(Value::as_bool) == Some(true) {
        o.push(("replacement", format!("tf.prioritize.eval_replacement_importance({p}, {s}{})", tail(&rescale))));
    }
    if has(n, "rank") || has(n, "budgets") {
        let mut a = given(n, &[("rank", "n"), ("budgets", "budgets"), ("rescale", "rescale")], sc)?;
        a.insert(0, format!("{p}, {s}"));
        code.push_str(&format!("{var}_rank = tf.prioritize.eval_rank_importance({})\n", a.join(", ")));
        o.push(("rank", format!("{var}_rank[0]")));
        o.push(("rank_budgets", format!("{var}_rank[1][[\"budget\", \"objective\"]]")));
    }
    if has(n, "extinction_z") {
        let z = setting(n, "extinction_z", sc)?.unwrap_or_default();
        o.push(("extinction_risk", format!("tf.prioritize.eval_extinction_risk({p}, {s}, z={z}){index}")));
    }
    Ok((code, outs(&o), Need::Flow))
}

fn calibrate(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(
        n,
        &[
            "features",
            "cost",
            "locked_in",
            "locked_out",
            "boundary_data",
            "connectivity_data",
            "asym_connectivity_data",
            "approach",
            "penalty",
            "values",
            "n",
            "inflation",
            "objective",
            "budget",
            "relative_targets",
            "absolute_targets",
            "feature_weights",
            "decisions",
            "edge_factor",
            "gap",
            "time_limit",
            "solver",
            "threads",
        ],
    )?;
    let cost = need(n, sc, "cost", "its cost")?;
    let approach = choice(n, "approach").unwrap_or("blended");
    let penalty = choice(n, "penalty").unwrap_or("boundary");
    let objective = choice(n, "objective").unwrap_or("min_set");
    if approach != "pareto" && objective != "min_set" {
        return Err(format!("the objective {objective:?}: the library calibrates a min set problem"));
    }
    let chain = problem_chain(n, sc, true)?;
    let data = match penalty {
        "boundary" => matrix_data(n, sc, "boundary_data", &cost)?,
        "connectivity" => Some(
            matrix_data(n, sc, "connectivity_data", &cost)?
                .ok_or("the connectivity penalty needs connectivity_data")?,
        ),
        "asym_connectivity" => Some(
            matrix_data(n, sc, "asym_connectivity_data", &cost)?
                .ok_or("the asymmetric connectivity penalty needs asym_connectivity_data")?,
        ),
        p => return Err(format!("the penalty {p:?}: not in the script yet")),
    };
    let mut a = vec![format!("{var}_problem")];
    let call = match approach {
        "blended" | "hierarchical" => {
            a.push(format!("penalty={}", text(penalty)));
            if approach == "blended" {
                a.extend(given(n, &[("values", "values"), ("n", "n")], sc)?);
            } else {
                a.extend(given(n, &[("values", "thresholds"), ("inflation", "inflation")], sc)?);
            }
            if let Some(d) = data {
                a.push(format!("data={d}"));
            }
            a.extend(given(n, &[("edge_factor", "edge_factor")], sc)?);
            if approach == "blended" { "calibrate_penalty" } else { "calibrate_budget" }
        }
        "pareto" => {
            a.extend(given(n, &[("values", "budgets"), ("n", "n")], sc)?);
            "pareto_front"
        }
        o => return Err(format!("the approach {o:?}: not in the script yet")),
    };
    let code =
        format!("{var}_problem = (\n    {}\n)\n{var} = tf.prioritize.{call}({})\n", chain.join("\n    "), a.join(", "));
    // the candidates' table as the library has it (its `status` where Terraflux has `optimal`), and the candidate
    // TOPSIS chooses
    let o = outs(&[
        (
            "table",
            format!(
                "{var}.table.assign(optimal=lambda t: (t[\"status\"] == \"optimal\").astype(int)).drop(columns=\"status\")"
            ),
        ),
        ("best", format!("{var}.best().selection")),
        (
            "solutions",
            format!(
                "_xr.concat([s.selection for s in {var}.solutions], dim=\"band\").assign_coords(band=[str(k + 1) for k in range(len({var}.solutions))])"
            ),
        ),
    ]);
    Ok((code, o, Need::Flow))
}

// --- multi-action planning

fn actions(n: &Node, var: &str, sc: &Scope, raster: bool) -> Result<Code, String> {
    let mut known: Vec<&str> =
        vec!["model", "budget", "blm", "curve", "segments", "gap", "time_limit", "solver", "threads"];
    let d = format!("{var}_data");
    let mut code = String::new();
    if raster {
        known.extend([
            "monitoring_cost",
            "features",
            "threats",
            "maps",
            "locked_in",
            "locked_out",
            "sensitivity",
            "action_cost",
            "target_recovery",
            "target_conservation",
            "relative",
            "blm_actions",
            "boundary",
        ]);
        refuse_others(n, &known)?;
        let mut a = vec![
            need(n, sc, "monitoring_cost", "its monitoring cost")?,
            need(n, sc, "features", "its features")?,
            need(n, sc, "threats", "its threats")?,
        ];
        // the cost of acting: numbers, or the names of the layers of `maps`
        if let Some(v) = n.params.get("action_cost").filter(|v| !v.is_null()) {
            let maps = if n.inputs.contains_key("maps") { value_of(n, "maps", sc)? } else { None };
            let c = if terraflux_graph::setting_ref(v).is_some() {
                setting(n, "action_cost", sc)?.unwrap_or_default()
            } else {
                action_cost(v, maps.as_deref())?
            };
            a.push(format!("action_cost={c}"));
        }
        a.extend(kw(
            n,
            sc,
            &[
                ("target_recovery", "target_recovery"),
                ("target_conservation", "target_conservation"),
                ("relative", "relative"),
                ("sensitivity", "sensitivity"),
                ("blm_actions", "blm_actions"),
                ("locked_in", "locked_in"),
                ("locked_out", "locked_out"),
                ("boundary", "boundary"),
            ],
        )?);
        code.push_str(&format!("{d} = tf.prioritize.actions_raster_data({})\n", a.join(", ")));
    } else {
        known.extend(["pu", "features", "dist_features", "threats", "dist_threats", "sensitivity", "boundary"]);
        refuse_others(n, &known)?;
        let mut a = vec![
            need(n, sc, "pu", "its pu")?,
            need(n, sc, "features", "its features")?,
            need(n, sc, "dist_features", "its dist_features")?,
            need(n, sc, "threats", "its threats")?,
            need(n, sc, "dist_threats", "its dist_threats")?,
        ];
        a.extend(kw(n, sc, &[("sensitivity", "sensitivity"), ("boundary", "boundary")])?);
        code.push_str(&format!("{d} = tf.prioritize.actions_data({})\n", a.join(", ")));
    }
    let model = given(
        n,
        &[("model", "model"), ("budget", "budget"), ("blm", "blm"), ("curve", "curve"), ("segments", "segments")],
        sc,
    )?;
    let solver = choice(n, "solver").unwrap_or("highs");
    let mut s = given(n, &[("gap", "gap"), ("time_limit", "time_limit")], sc)?;
    match solver {
        "highs" => {}
        "gurobi" => s.extend(given(n, &[("threads", "n_threads")], sc)?),
        o => return Err(format!("the solver {o:?}: not in the script yet")),
    }
    code.push_str(&format!("{var} = (\n    {d}.problem({})\n", model.join(", ")));
    if !s.is_empty() || solver != "highs" {
        code.push_str(&format!("    .add_{solver}_solver({})\n", s.join(", ")));
    }
    code.push_str("    .solve()\n)\n");
    code.push_str(&format!(
        "import pandas as _pd\n{var}_result = _pd.DataFrame({{\"objective\": [{var}.objective], \"gap\": [{var}.gap], \"optimal\": [int({var}.status == \"optimal\")]}})\n"
    ));
    let mut o = vec![
        ("monitoring", format!("{var}.monitoring()")),
        ("actions", format!("{var}.action_table()")),
        ("summary", format!("{var}.action_summary()")),
        ("result", format!("{var}_result")),
        ("cost", format!("{var}.cost()")),
        ("benefit", format!("{var}.benefit()")),
        ("local", format!("{var}.local_benefit")),
        ("potential", format!("{d}.potential_benefit()")),
        ("connectivity", format!("{var}.connectivity_penalty()")),
    ];
    if raster {
        o.push(("maps", format!("{var}.maps()")));
        o.push(("pu", format!("{d}.pu")));
        o.push(("dist_threats", format!("{d}.dist_threats.iloc[:, :4]")));
    }
    Ok((code, outs(&o), Need::Nothing))
}

/// The cost of the actions as Python: numbers as they are, a name the layer of `maps` of that name.
fn action_cost(v: &Value, maps: Option<&str>) -> Result<String, String> {
    Ok(match v {
        Value::String(s) => {
            let m = maps.ok_or_else(|| format!("action_cost {s:?}: a map's name, but no maps"))?;
            format!("{m}.sel(band={})", text(s))
        }
        Value::Array(a) => {
            format!("[{}]", a.iter().map(|x| action_cost(x, maps)).collect::<Result<Vec<_>, String>>()?.join(", "))
        }
        Value::Object(o) => format!(
            "{{{}}}",
            o.iter()
                .map(|(k, x)| Ok(format!("{}: {}", text(k), action_cost(x, maps)?)))
                .collect::<Result<Vec<_>, String>>()?
                .join(", ")
        ),
        v => literal(v),
    })
}

// --- restoration

fn status(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["map", "foreground"])?;
    let map = habitat(n, sc, "map")?;
    let code = format!(
        "{var} = tf.restoration_status({map})\nimport pandas as _pd\n{var}_stats = _pd.DataFrame([{{\"data_area\": {var}.data_area, \"objects\": {var}.nr_comp, \"habitat\": {var}.eca_max, \"largest\": {var}.larg_obj, \"aps\": {var}.aps, \"cnoa\": {var}.cnoa, \"eca\": {var}.eca, \"coherence\": {var}.doc, \"restoration_potential\": {var}.rest_pot, \"rac\": {var}.rac}}])\n"
    );
    Ok((code, outs(&[("stats", format!("{var}_stats"))]), Need::Nothing))
}

fn cost_distance(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["resistance", "sources"])?;
    let r = bytes(n, sc, "resistance")?;
    let s = need(n, sc, "sources", "its sources")?;
    // the cost where the sources cannot be reached: no value
    let code = format!("{var} = tf.cost_distance({r}, {s})\n{var} = {var}.where(np.isfinite({var}))\n");
    Ok((code, outs(&[("map", var.to_string())]), Need::Nothing))
}

/// The arguments of the least-cost path and the corridor: the resistance, the areas and the distance.
fn path_args(n: &Node, sc: &Scope) -> Result<Vec<String>, String> {
    let mut a = vec![bytes(n, sc, "resistance")?, need(n, sc, "a", "its area a")?, need(n, sc, "b", "its area b")?];
    a.extend(kw(n, sc, &[("distance", "dist_map")])?);
    Ok(a)
}

fn least_cost_path(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["resistance", "a", "b", "distance"])?;
    let a = path_args(n, sc)?;
    let code = format!(
        "{var} = tf.least_cost_path({})\nimport pandas as _pd\n{var}_stats = _pd.DataFrame({{\"length\": [{var}.path_length], \"restored\": [{var}.rest_pixels], \"expense\": [{var}.expense], \"mean_distance\": [{var}.avg_distance]}})\n",
        a.join(", ")
    );
    let o = outs(&[
        ("path", format!("{var}.lcp_mask")),
        ("cost", format!("{var}.combined_cost.where(np.isfinite({var}.combined_cost))")),
        ("stats", format!("{var}_stats")),
    ]);
    Ok((code, o, Need::Nothing))
}

fn corridor(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["resistance", "a", "b", "distance", "habitat"])?;
    let habitat = bytes(n, sc, "habitat")?;
    let mut a = vec![format!("{var}_habitat")];
    a.extend(path_args(n, sc)?);
    let code = format!(
        "{var}_habitat = {habitat}\n{var} = tf.restoration_corridor({})\nimport pandas as _pd\n{var}_stats = _pd.DataFrame([{{\"size_a\": {var}.size_a, \"size_b\": {var}.size_b, \"restored\": {var}.rest_pixels, \"mean_distance\": {var}.lcp_result.avg_distance, \"expense\": {var}.expense, \"efficiency\": {var}.efficiency, \"eca\": {var}.eca_orig, \"eca_new\": {var}.eca_new, \"delta_eca\": {var}.delta_eca, \"coherence\": {var}.doc_orig, \"coherence_new\": {var}.doc_new, \"delta_coherence\": {var}.delta_doc}}])\n{var}_restored = {var}_habitat.where(~(({var}_habitat == 1) & ({var}.lcp_result.lcp_mask > 0)), 2)\n",
        a.join(", ")
    );
    let o = outs(&[
        ("path", format!("{var}.lcp_result.lcp_mask")),
        ("restored", format!("{var}_restored")),
        ("stats", format!("{var}_stats")),
    ]);
    Ok((code, o, Need::Nothing))
}

fn isochrone(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["resistance", "marker"])?;
    let r = bytes(n, sc, "resistance")?;
    let m = bytes(n, sc, "marker")?;
    // between A and B when the marker has an object B (2), else from A
    let code = format!(
        "{var}_resistance = {r}\n{var}_marker = {m}\n{var} = (\n    tf.isochrone_map_ab({var}_resistance, {var}_marker)\n    if ({var}_marker == 2).any()\n    else tf.isochrone_map({var}_resistance, {var}_marker)\n)\n{var}_cost = {var}.cost.where({var}.cost != -2147483648)\nimport pandas as _pd\n{var}_stats = _pd.DataFrame({{\"min_cost\": [{var}.min_cost], \"max_cost\": [{var}.max_cost]}})\n"
    );
    let o = outs(&[
        ("map", format!("{var}.map")),
        ("cost", format!("{var}_cost")),
        ("path", format!("{var}.lcp")),
        ("stats", format!("{var}_stats")),
    ]);
    Ok((code, o, Need::Nothing))
}

fn original_vegetation(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(
        n,
        &["land_cover", "past", "ecoregions", "natural", "anthropic", "failsafe", "threshold", "restore_to"],
    )?;
    let mut a = vec![need(n, sc, "land_cover", "its land cover")?];
    let natural = class_list(n, sc, "natural")?.ok_or("its natural classes are not set")?;
    a.push(format!("natural={natural}"));
    a.push(format!("anthropic={}", class_list(n, sc, "anthropic")?.ok_or("its anthropic classes are not set")?));
    a.extend(kw(n, sc, &[("past", "past"), ("ecoregions", "ecoregions")])?);
    if let Some(f) = class_list(n, sc, "failsafe")? {
        a.push(format!("failsafe={f}"));
    }
    a.extend(kw(n, sc, &[("threshold", "threshold"), ("restore_to", "restore_to")])?);
    // the failsafe's shares as a table: the class's position among the natural ones, and its share
    let code = format!(
        "{var} = tf.original_vegetation({})\nimport pandas as _pd\n{var}_failsafe = _pd.DataFrame({{\"class\": [{natural}.index(c) + 1 for c in {var}.failsafe], \"share\": list({var}.failsafe.values())}})\n",
        a.join(", ")
    );
    let o = outs(&[
        ("original", format!("{var}.original")),
        ("restore", format!("{var}.restore")),
        ("source", format!("{var}.source")),
        ("anthropic", format!("{var}.anthropic")),
        ("failsafe", format!("{var}_failsafe")),
    ]);
    Ok((code, o, Need::Nothing))
}

fn ecosystem_areas(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["land_cover", "ecoregions", "natural", "anthropic", "cell_area"])?;
    let mut a = vec![
        need(n, sc, "land_cover", "its land cover")?,
        class_list(n, sc, "natural")?.ok_or("its natural classes are not set")?,
        class_list(n, sc, "anthropic")?.ok_or("its anthropic classes are not set")?,
        need(n, sc, "ecoregions", "its ecoregions")?,
    ];
    a.extend(kw(n, sc, &[("cell_area", "cell_area")])?);
    let code = format!("{var} = tf.prioritize.ecosystem_areas({})\n", a.join(", "));
    // the table (`species`: the ecoregion's code) and the gain: a sparse matrix of ecoregions x cells
    Ok((code, outs(&[("table", format!("{var}.table")), ("gain", format!("{var}.gain"))]), Need::Nothing))
}

/// The library's `HabitatAreas` from a table of the areas of habitat or of ecosystems (its species named `species`
/// or `code`) and, when given, their gain and loss (a stack, a layer each, or the library's sparse matrix).
fn habitat_areas(name: &str, table: &str, gain: Option<&str>, loss: Option<&str>) -> String {
    let sparse = |x: Option<&str>| match x {
        Some(g) => {
            format!("(_sp.csr_matrix({g}.values.reshape({g}.shape[0], -1)) if hasattr({g}, \"values\") else {g})")
        }
        None => "None".to_string(),
    };
    format!(
        "import scipy.sparse as _sp\n{name} = tf.sdm.HabitatAreas(\n    table=({table} if \"species\" in {table} else {table}.rename(columns={{\"code\": \"species\"}})),\n    gain={},\n    loss={},\n)\n",
        sparse(gain),
        sparse(loss)
    )
}

fn ecosystem_targets(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(n, &["table", "target", "minimum", "z", "half_life", "method", "steps"])?;
    let table = need(n, sc, "table", "its table")?;
    let target = need(n, sc, "target", "its target")?;
    let mut a = vec![format!("{var}_areas"), target];
    a.extend(kw(
        n,
        sc,
        &[("minimum", "minimum"), ("z", "z"), ("half_life", "half_life"), ("method", "method"), ("steps", "steps")],
    )?);
    let mut code = habitat_areas(&format!("{var}_areas"), &table, None, None);
    code.push_str(&format!("{var} = tf.prioritize.ecosystem_targets({})\n", a.join(", ")));
    Ok((code, outs(&[("targets", format!("{var}[[\"region\", \"target\"]]"))]), Need::Nothing))
}

/// The targets by region as a Python mapping: the codes numbers where they are.
fn regional(o: &serde_json::Map<String, Value>) -> String {
    let key = |k: &String| match k.parse::<f64>() {
        Ok(x) if x.fract() == 0.0 => format!("{}", x as i64),
        Ok(_) => k.clone(),
        Err(_) => text(k),
    };
    format!("{{{}}}", o.iter().map(|(k, v)| format!("{}: {}", key(k), literal(v))).collect::<Vec<_>>().join(", "))
}

fn plan(n: &Node, var: &str, sc: &Scope) -> Result<Code, String> {
    refuse_others(
        n,
        &[
            "restorable",
            "conservable",
            "convertible",
            "threat",
            "loss",
            "table",
            "gain",
            "linear",
            "cost",
            "regions",
            "limit",
            "method",
            "steps",
            "target",
            "sense",
            "relative",
            "budget",
            "z",
            "weight",
            "linear_weights",
            "restoration_discount",
            "cell_area",
            "min_benefit",
            "tolerance",
            "time_limit",
            "solver",
            "threads",
        ],
    )?;
    let (p, s) = (format!("{var}_problem"), format!("{var}_solution"));
    let mut code = String::new();
    let land = [("restorable", "restored"), ("conservable", "conserved"), ("convertible", "converted")];
    let acts: Vec<&str> = land.iter().filter(|(k, _)| n.inputs.contains_key(*k)).map(|(_, port)| *port).collect();
    if acts.is_empty() {
        return Err("give the land that can be restored (restorable), conserved or converted".into());
    }
    let start = kw(
        n,
        sc,
        &[
            ("restorable", "restorable"),
            ("conservable", "conservable"),
            ("convertible", "convertible"),
            ("limit", "limit"),
            ("threat", "threat"),
            ("cell_area", "cell_area"),
        ],
    )?;
    let mut chain = vec![format!("tf.prioritize.restoration_problem({})", start.join(", "))];
    // the extinction risk: the area of habitat's table with its gain (and loss)
    if n.inputs.contains_key("table") {
        let table = need(n, sc, "table", "its table")?;
        let gain = if n.inputs.contains_key("gain") { value_of(n, "gain", sc)? } else { None };
        let loss = if n.inputs.contains_key("loss") { value_of(n, "loss", sc)? } else { None };
        code.push_str(&habitat_areas(&format!("{var}_areas"), &table, gain.as_deref(), loss.as_deref()));
        let mut r = vec![format!("{var}_areas")];
        r.extend(kw(n, sc, &[("z", "z"), ("weight", "weight"), ("restoration_discount", "restoration_discount")])?);
        chain.push(format!(".add_extinction_risk_benefit({})", r.join(", ")));
    } else if ["gain", "loss"].iter().any(|k| n.inputs.contains_key(*k))
        || ["z", "weight", "restoration_discount"].iter().any(|k| has(n, k))
    {
        return Err("gain, loss, z, weight and restoration_discount need the table of the area of habitat".into());
    }
    let linear = if n.inputs.contains_key("linear") { value_of(n, "linear", sc)? } else { None };
    // the area target, the budget and the least benefit
    let mut after: Vec<String> = Vec::new();
    if let Some(t) = n.params.get("target").filter(|t| !t.is_null()) {
        let mut a = Vec::new();
        match t {
            Value::Object(o) if terraflux_graph::setting_ref(t).is_none() => {
                let regions = need(n, sc, "regions", "its regions (the targets are by region)")?;
                a.push(regional(o));
                a.push(format!("regions={regions}"));
            }
            _ => {
                if n.inputs.contains_key("regions") {
                    return Err("regions with one target for the landscape: not in the script yet".into());
                }
                a.push(setting(n, "target", sc)?.unwrap_or_default());
            }
        }
        a.extend(given(n, &[("sense", "sense"), ("relative", "relative")], sc)?);
        after.push(format!(".add_area_target({})", a.join(", ")));
    }
    let cost = if n.inputs.contains_key("cost") { value_of(n, "cost", sc)? } else { None };
    if has(n, "budget") {
        let b = setting(n, "budget", sc)?.unwrap_or_default();
        let c = cost.clone().ok_or("the budget needs the cost")?;
        after.push(format!(".add_budget({b}, {c})"));
    }
    if has(n, "min_benefit") {
        let m = setting(n, "min_benefit", sc)?.unwrap_or_default();
        match (&cost, has(n, "budget")) {
            (_, true) => after.push(format!(".add_min_cost_objective({m})")),
            (Some(c), false) => after.push(format!(".add_min_cost_objective({m}, cost={c})")),
            (None, false) => return Err("min_benefit needs the cost".into()),
        }
    }
    let solver = choice(n, "solver").unwrap_or("highs");
    let mut sa = given(n, &[("tolerance", "tolerance"), ("time_limit", "time_limit")], sc)?;
    match solver {
        "highs" => {}
        "gurobi" => sa.extend(given(n, &[("threads", "n_threads")], sc)?),
        o => return Err(format!("the solver {o:?}: not in the script yet")),
    }
    if !sa.is_empty() || solver != "highs" {
        after.push(format!(".add_{solver}_solver({})", sa.join(", ")));
    }
    match linear {
        None => {
            chain.extend(after);
            code.push_str(&format!("{p} = (\n    {}\n)\n", chain.join("\n    ")));
        }
        Some(l) => {
            code.push_str(&format!("{p} = (\n    {}\n)\n", chain.join("\n    ")));
            // a benefit per layer of the stack, each its weight
            let weights = match setting(n, "linear_weights", sc)?.filter(|_| has(n, "linear_weights")) {
                Some(w) => w,
                None => format!("[1.0] * {l}.sizes[\"band\"]"),
            };
            code.push_str(&format!(
                "for _name, _weight in zip({l}[\"band\"].values, {weights}):\n    {p} = {p}.add_linear_benefit({l}.sel(band=_name), weight=_weight, name=str(_name))\n"
            ));
            if !after.is_empty() {
                code.push_str(&format!("{p} = (\n    {p}\n    {}\n)\n", after.join("\n    ")));
            }
        }
    }
    let solve = match choice(n, "method").unwrap_or("exact") {
        "exact" => ".solve()".to_string(),
        "steps" => {
            let steps: String = given(n, &[("steps", "steps")], sc)?.iter().map(|x| format!(", {x}")).collect();
            format!(".solve(method=\"steps\"{steps})")
        }
        "uniform" => ".uniform()".to_string(),
        m => return Err(format!("the method {m:?}: not in the script yet")),
    };
    code.push_str(&format!("{s} = {p}{solve}\nimport pandas as _pd\n"));
    code.push_str(&format!(
        "{var}_summary = _pd.DataFrame({{\"benefit\": [{s}.benefit], \"area\": [{s}.area], \"cost\": [{s}.cost], \"bound\": [{s}.bound], \"optimal\": [int({s}.status == \"optimal\")], \"rounds\": [{s}.rounds]}})\n"
    ));
    let mut o: Vec<(&str, String)> = acts.iter().map(|port| (*port, format!("{s}.{port}"))).collect();
    o.push(("summary", format!("{var}_summary")));
    o.push(("benefits", format!("{s}.benefits[[\"value\", \"weight\", \"weighted\"]]")));
    Ok((code, outs(&o), Need::Nothing))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The script parses as Python (when there is a Python to ask).
    fn parses(t: &str) {
        if let Ok(o) = std::process::Command::new("python")
            .args(["-I", "-c", "import ast, sys; ast.parse(sys.stdin.read())"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut c| {
                use std::io::Write;
                c.stdin.take().unwrap().write_all(t.as_bytes())?;
                c.wait_with_output()
            })
        {
            assert!(o.status.success(), "{}\n{t}", String::from_utf8_lossy(&o.stderr));
        }
    }

    #[test]
    fn a_plan_of_units_evaluated() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "cost"
op = "nlm.random"

[[nodes]]
id = "a"
op = "nlm.random"

[[nodes]]
id = "b"
op = "nlm.random"

[[nodes]]
id = "features"
op = "stack.of"
inputs = { sp1 = "a.map", sp2 = "b.map" }

[[nodes]]
id = "bm"
op = "prioritize.boundary_matrix"
inputs = { units = "cost.map" }

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { cost = "cost.map", features = "features.stack", connectivity_data = "bm.table" }
params = { relative_targets = 0.2, connectivity_penalty = 0.1 }

[[nodes]]
id = "check"
op = "prioritize.evaluate"
inputs = { cost = "cost.map", features = "features.stack", solution = "plan.solution", boundary_data = "bm.table" }
params = { relative_targets = 0.2, edge_factor = 0.5, extinction_z = 0.25 }

[[nodes]]
id = "covered"
op = "table.write"
inputs = { table = "check.coverage" }
params = { path = "coverage.csv" }

[[nodes]]
id = "points"
op = "prioritize.unit_points"
inputs = { units = "cost.map" }

[[nodes]]
id = "coarse"
op = "prioritize.coarsen"
inputs = { cost = "cost.map", features = "features.stack" }
params = { factor = 4 }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        assert!(s.left.is_empty(), "{:?}\n{}", s.left, s.text);
        let t = &s.text;
        assert!(t.contains("bm = tf.prioritize.boundary_matrix(cost)\n"), "{t}");
        assert!(t.contains("bm_table = pd.DataFrame({\"i\": bm.tocoo().row + 1"), "{t}");
        assert!(
            t.contains("check_problem = (\n    tf.prioritize.problem(cost, features)\n    .add_min_set_objective()\n    .add_relative_targets(0.2)\n    .add_binary_decisions()\n)\ncheck_solution = plan.selection\n"),
            "{t}"
        );
        assert!(
            t.contains("tf.prioritize.eval_boundary_summary(check_problem, check_solution, edge_factor=0.5, data=_unit_pairs(bm_table, cost.size))"),
            "{t}"
        );
        assert!(t.contains("tf.prioritize.eval_target_coverage_summary(check_problem, check_solution)"), "{t}");
        assert!(t.contains("coarse = tf.prioritize.coarsen(cost, features=features, factor=4)\n"), "{t}");
        parses(t);
    }

    #[test]
    fn restoration_from_the_original_vegetation_to_the_plan() {
        let r = Registry::standard();
        let m = Model::from_toml(
            r#"
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "nlm.random"

[[nodes]]
id = "crops"
op = "nlm.random"

[[nodes]]
id = "carbon"
op = "nlm.random"

[[nodes]]
id = "cover"
op = "stack.of"
inputs = { forest = "forest.map", crops = "crops.map" }

[[nodes]]
id = "original"
op = "restoration.original_vegetation"
inputs = { land_cover = "cover.stack" }
params = { natural = ["forest"], anthropic = ["crops"] }

[[nodes]]
id = "habitat"
op = "restoration.status"
inputs = { map = "forest.map" }
params = { foreground = [1] }

[[nodes]]
id = "layers"
op = "stack.of"
inputs = { carbon = "carbon.map" }

[[nodes]]
id = "restore"
op = "restoration.plan"
inputs = { restorable = "original.anthropic", linear = "layers.stack", cost = "carbon.map" }
params = { target = 50, budget = 1000, cell_area = 1.0 }
"#,
        )
        .unwrap();
        let s = super::super::script(&m, &r, None);
        assert!(s.left.is_empty(), "{:?}\n{}", s.left, s.text);
        let t = &s.text;
        assert!(
            t.contains("original = tf.original_vegetation(cover, natural=[\"forest\"], anthropic=[\"crops\"])\n"),
            "{t}"
        );
        assert!(t.contains("habitat = tf.restoration_status(tf.reclassify(forest, foreground=[1]))\n"), "{t}");
        assert!(t.contains("tf.prioritize.restoration_problem(restorable=original.anthropic, cell_area=1.0)"), "{t}");
        assert!(t.contains(".add_area_target(50)"), "{t}");
        assert!(t.contains(".add_budget(1000, carbon)"), "{t}");
        assert!(t.contains("restore_problem = restore_problem.add_linear_benefit(layers.sel(band=_name)"), "{t}");
        assert!(t.contains("restore_solution = restore_problem.solve()\n"), "{t}");
        parses(t);
    }
}
