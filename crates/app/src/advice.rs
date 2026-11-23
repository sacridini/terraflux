//! Advice on a model's method, beyond its graph's errors and its data's problems (`terraflux_exec::lint`): what a
//! reviewer of the paper would ask. A land change simulated but never compared with an observed map, or compared
//! with the map it was calibrated on; a stochastic simulation run once; distances and areas measured on maps in
//! degrees; landscape metrics of a continuous map; a species distribution model evaluated on random folds. Each
//! piece of advice says what to do and the guide (`guide`) that tells more. Told by
//! check_model and after a run (run_model), to `terraflux mcp` and to the assistant in the application alike.

use std::collections::BTreeMap;
use std::path::Path;

use terraflux_graph::{Model, Node, Source};
use terraflux_ops::{Registry, Value};
use terraflux_raster::Grid;

/// How much a piece of advice matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    /// A good practice the model does not follow yet.
    Advice,
    /// What makes a result wrong or misleading.
    Warning,
}

/// A piece of advice on the method.
#[derive(Debug, Clone, PartialEq)]
pub struct Advice {
    pub level: Level,
    /// The nodes it is about (empty: the whole model).
    pub at: Vec<String>,
    pub message: String,
    /// The guide that tells more.
    pub guide: &'static str,
}

impl std::fmt::Display for Advice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let l = match self.level {
            Level::Advice => "advice",
            Level::Warning => "warning",
        };
        write!(f, "{l}: ")?;
        if !self.at.is_empty() {
            write!(f, "{}: ", self.at.join(", "))?;
        }
        write!(f, "{} (guide {})", self.message, self.guide)
    }
}

/// The operators that simulate land change, a step each.
const SIMULATORS: [&str; 7] = [
    "dinamica.step",
    "lulcc.step",
    "cars.step",
    "clumondo.step",
    "molusce.step",
    "futures.simulation",
    "crafty.allocate",
];

/// The operators calibrated on two dates (their input `t1` the later), the rates and the transitions' potential.
const CALIBRATED: [&str; 2] = ["markov.fit", "woe.fit"];

/// The inputs of the validations that take the observed (reference) map.
const OBSERVED: [&str; 3] = ["observed", "t1", "reference"];

/// What repeats a run (seeds, scenarios, samples): a stochastic model run under it is not run once.
const REPEATS: [&str; 5] =
    ["control.each", "batch.run", "uncertainty.monte_carlo", "sensitivity.design", "sensitivity.analyze"];

/// The families of the operators that measure distances or areas on the cells.
const MEASURES: [&str; 9] = [
    "metrics.",
    "fragmentation.",
    "landscape.",
    "distance.",
    "cost.",
    "conn.",
    "circuit.",
    "restoration.",
    "move.surface_distance",
];

/// The families of the operators that take a map of classes.
const OF_CLASSES: [&str; 3] = ["metrics.", "fragmentation.", "landscape."];

/// The spatial folds of a species distribution model.
const SPATIAL_FOLDS: [&str; 5] =
    ["sdm.spatial_blocks", "sdm.buffer_folds", "sdm.checkerboard_folds", "sdm.quadrant_folds", "sdm.spatial_clusters"];

/// Every node, inside the containers too.
fn all(nodes: &[Node]) -> Vec<&Node> {
    fn add<'a>(ns: &'a [Node], out: &mut Vec<&'a Node>) {
        for n in ns {
            out.push(n);
            if let Some(b) = &n.body {
                add(&b.nodes, out);
            }
        }
    }
    let mut out = Vec::new();
    add(nodes, &mut out);
    out
}

/// What an input of a node reads, when it is a reference.
fn reads<'a>(n: &'a Node, port: &str) -> Option<&'a str> {
    match n.inputs.get(port)? {
        Source::Ref(r) => Some(r.as_str()),
        Source::Literal(_) => None,
    }
}

/// The nodes as `id (op)`, a sorted list.
fn ids(ns: &[&Node]) -> Vec<String> {
    let mut v: Vec<String> = ns.iter().map(|n| format!("{} ({})", n.id, n.op)).collect();
    v.sort();
    v.dedup();
    v
}

/// Whether a grid's coordinates are longitudes and latitudes (by its coordinate system, when known).
fn geographic(g: &Grid) -> bool {
    g.geo.is_geographic().unwrap_or_else(|| {
        terraflux_ops::prepare::crs_of(g).is_some_and(|c| terraflux_gdal::is_geographic(&c).unwrap_or(false))
    })
}

/// The grids of the model's maps: its inputs' (when loaded) and its raster.read nodes' files (their headers), by
/// what they are (`$input`, the node's id).
fn grids(nodes: &[&Node], dir: &Path, inputs: Option<&BTreeMap<String, Value>>) -> Vec<(String, Grid)> {
    let mut out = Vec::new();
    for (name, v) in inputs.into_iter().flatten() {
        match v {
            Value::Map(m) => out.push((format!("${name}"), m.grid.as_ref().clone())),
            Value::Stack(s) => out.push((format!("${name}"), s.grid.as_ref().clone())),
            _ => {}
        }
    }
    for n in nodes.iter().filter(|n| n.op == "raster.read") {
        let Some(p) = n.params.get("path").and_then(|v| v.as_str()) else { continue };
        if p.starts_with('$') || p.starts_with('@') || terraflux_graph::is_sent(p) {
            continue;
        }
        let path = if Path::new(p).is_absolute() { Path::new(p).to_path_buf() } else { dir.join(p) };
        if let Ok(r) = terraflux_ops::files::Rasters::open(&path.to_string_lossy(), None, None) {
            out.push((n.id.clone(), r.grid.as_ref().clone()));
        }
    }
    out
}

/// The advice on a model's method (`m` with its submodels expanded; `dir` its folder; `inputs` its inputs'
/// values, when loaded), the warnings first.
pub fn advise(m: &Model, r: &Registry, dir: &Path, inputs: Option<&BTreeMap<String, Value>>) -> Vec<Advice> {
    let nodes = all(&m.nodes);
    let has = |op: &str| nodes.iter().any(|n| n.op == op);
    let of = |ops: &[&str]| -> Vec<&Node> { nodes.iter().copied().filter(|n| ops.contains(&n.op.as_str())).collect() };
    let mut out = Vec::new();

    // a simulation of land change: validated, on a map it was not calibrated on, run more than once
    let simulators = of(&SIMULATORS);
    let validations: Vec<&Node> = nodes.iter().copied().filter(|n| n.op.starts_with("validate.")).collect();
    if !simulators.is_empty() && validations.is_empty() {
        out.push(Advice {
            level: Level::Advice,
            at: ids(&simulators),
            message: "the model simulates land change but never compares a simulation with an observed map: \
                      simulate a date already observed and not used to calibrate (calibrated on t0 to t1, \
                      simulated from t1 to t2, compared with the map of t2) with validate.three_map (the figure of \
                      merit at several resolutions) or validate.similarity (Dinamica's), and with the null model of \
                      no change"
                .into(),
            guide: "land-change",
        });
    }
    let calibrated_on: Vec<(&str, &Node)> =
        of(&CALIBRATED).into_iter().filter_map(|n| Some((reads(n, "t1")?, n))).collect();
    for v in &validations {
        let Some(observed) = OBSERVED.iter().find_map(|p| reads(v, p)) else { continue };
        let on: Vec<&Node> = calibrated_on.iter().filter(|(src, _)| *src == observed).map(|(_, n)| *n).collect();
        if !on.is_empty() {
            out.push(Advice {
                level: Level::Warning,
                at: vec![v.id.clone()],
                message: format!(
                    "it compares the simulation with {observed}, the map {} {} calibrated on: the agreement is \
                     optimistic (the model has seen it); compare with a later map, not used to calibrate",
                    ids(&on).join(" and "),
                    if on.len() > 1 { "were" } else { "was" }
                ),
                guide: "land-change",
            });
        }
    }
    let stochastic: Vec<&Node> = simulators
        .iter()
        .copied()
        .filter(|n| {
            r.sig(&n.op)
                .is_some_and(|s| s.inputs.iter().chain(s.params.iter()).any(|p| p.name == "seed" || p.name == "rng"))
        })
        .collect();
    if !stochastic.is_empty() && !REPEATS.iter().any(|op| has(op)) && m.scenarios.is_empty() {
        out.push(Advice {
            level: Level::Advice,
            at: ids(&stochastic),
            message: "a stochastic simulation (its seed draws the changes) run once is one possible future: run it \
                      with several seeds (control.each over a list of seeds) and report the spread"
                .into(),
            guide: "land-change",
        });
    }

    // distances and areas on maps in degrees
    let measuring: Vec<&Node> =
        nodes.iter().copied().filter(|n| MEASURES.iter().any(|f| n.op.starts_with(f))).collect();
    if !measuring.is_empty() && !has("raster.reproject") {
        let in_degrees: Vec<String> =
            grids(&nodes, dir, inputs).into_iter().filter(|(_, g)| geographic(g)).map(|(at, _)| at).collect();
        if !in_degrees.is_empty() {
            let mut ops: Vec<&str> = measuring.iter().map(|n| n.op.as_str()).collect();
            ops.sort();
            ops.dedup();
            ops.truncate(4);
            out.push(Advice {
                level: Level::Warning,
                at: in_degrees,
                message: format!(
                    "maps in longitudes and latitudes, and the model measures distances or areas on their cells \
                     ({}): cells of degrees are neither square nor of one size; put the maps in a projected \
                     coordinate system first (raster.reproject: a UTM zone, or an equal-area projection for areas)",
                    ops.join(", ")
                ),
                guide: "data-preparation",
            });
        }
    }

    // the energy of walking is a cost per kilogram of the animal: without its mass there is no landscape
    let unweighed: Vec<&Node> = of(&["cost.energy_landscape"]).into_iter().filter(|n| !given(n, "mass")).collect();
    if !unweighed.is_empty() {
        out.push(Advice {
            level: Level::Warning,
            at: ids(&unweighed),
            message: "no mass: the energy of walking depends on the body mass of the animal (kg), and the operator \
                      stops without it; give the mass of the species (an adult's, from the literature) and say where \
                      it comes from"
                .into(),
            guide: "movement",
        });
    }

    // metrics of classes on a continuous map
    if let Some(inputs) = inputs {
        for n in nodes.iter().filter(|n| OF_CLASSES.iter().any(|f| n.op.starts_with(f))) {
            let Some(name) = reads(n, "map").and_then(|s| s.strip_prefix('$')) else { continue };
            let Some(Value::Map(map)) = inputs.get(name) else { continue };
            if map.data.iter().filter(|v| v.is_finite()).take(100_000).any(|v| v.fract() != 0.0) {
                out.push(Advice {
                    level: Level::Warning,
                    at: vec![n.id.clone()],
                    message: format!(
                        "{} takes a map of classes, and ${name} has values that are not whole (a continuous map): \
                         classify it first (raster.reclassify, or write_calc)",
                        n.op
                    ),
                    guide: "landscape-metrics",
                });
            } else {
                // whole values, but so many classes that the metrics per class mean little
                let mut seen: std::collections::BTreeSet<u64> = std::collections::BTreeSet::new();
                for v in map.data.iter().take(1_000_000).filter(|v| v.is_finite()) {
                    seen.insert(v.to_bits());
                    if seen.len() > 100 {
                        break;
                    }
                }
                if seen.len() > 100 {
                    out.push(Advice {
                        level: Level::Advice,
                        at: vec![n.id.clone()],
                        message: format!(
                            "{} computes metrics per class, and ${name} has more than 100 classes (a map of ids or \
                             of a continuous quantity rounded): group them first (raster.reclassify), or check \
                             that it is the map of classes meant",
                            n.op
                        ),
                        guide: "landscape-metrics",
                    });
                }
            }
        }
    }

    // a species distribution model evaluated on random folds
    if has("sdm.fit") && has("sdm.random_folds") && !SPATIAL_FOLDS.iter().any(|op| has(op)) {
        out.push(Advice {
            level: Level::Advice,
            at: ids(&of(&["sdm.random_folds"])),
            message: "random folds inflate a distribution model's accuracy when the records cluster in space: \
                      evaluate it on spatial folds too (sdm.spatial_blocks, sdm.buffer_folds; \
                      sdm.autocorrelation_range gives the blocks' size)"
                .into(),
            guide: "species-distribution",
        });
    }

    // a species distribution model of records not thinned
    if has("sdm.fit") && !THINNING.iter().any(|op| has(op)) {
        out.push(Advice {
            level: Level::Advice,
            at: ids(&of(&["sdm.fit"])),
            message: "the records are fitted as they come: duplicates and clusters of records (near roads, towns, \
                      the collectors' paths) weigh as many presences and teach the model the sampling, not the \
                      species; keep one record a cell (sdm.one_per_cell) or thin them by a distance (sdm.thin), \
                      after cleaning them (sdm.clean_occurrences), or prepare them with sdm.prepare"
                .into(),
            guide: "species-distribution",
        });
    }

    // a prioritization never evaluated
    let solved = of(&PRIORITIZATIONS);
    if !solved.is_empty() && !has("prioritize.evaluate") {
        out.push(Advice {
            level: Level::Advice,
            at: ids(&solved),
            message: "the solution is not evaluated: how much of each feature's target it meets, its cost, and \
                      how irreplaceable each unit is; prioritize.evaluate gives them (the features' representation, \
                      the replacement cost or the rank of the units) for the paper and the stakeholders"
                .into(),
            guide: "conservation-planning",
        });
    }

    // resistances of zero or less
    if let Some(inputs) = inputs {
        for n in &nodes {
            let Some(sig) = r.sig(&n.op) else { continue };
            // (Circuitscape's modes read it as `habitat`, a resistance unless habitat_is_resistance is false)
            let circuit_resistance = n.op.starts_with("circuit.")
                && n.params.get("habitat_is_resistance").and_then(serde_json::Value::as_bool) != Some(false);
            for p in sig
                .inputs
                .iter()
                .filter(|p| p.name.contains("resistance") || (p.name == "habitat" && circuit_resistance))
            {
                let Some(name) = reads(n, &p.name).and_then(|s| s.strip_prefix('$')) else { continue };
                let Some(Value::Map(map)) = inputs.get(name) else { continue };
                let low = map.data.iter().filter(|v| v.is_finite()).fold(f64::INFINITY, |a, &b| a.min(b));
                if low <= 0.0 {
                    out.push(Advice {
                        level: Level::Warning,
                        at: vec![n.id.clone()],
                        message: format!(
                            "the resistance ${name} has cells of {} ({} {}): a resistance must be above 0 (a cell \
                             of 0 costs nothing to cross and shorts the circuit; one below 0 has no meaning); set a \
                             small positive minimum (write_calc), or make the cells that cannot be crossed no data",
                            if low < 0.0 { "less than 0" } else { "0" },
                            n.op,
                            p.name
                        ),
                        guide: "connectivity",
                    });
                }
            }
        }
    }

    // the Markov chain's steps against the dates of its maps
    for n in of(&["markov.fit"]) {
        let (Some(y0), Some(y1)) = (year_of(&nodes, reads(n, "t0")), year_of(&nodes, reads(n, "t1"))) else {
            continue;
        };
        let years = y1 - y0;
        let steps = match n.inputs.get("steps") {
            Some(Source::Literal(v)) => v.as_f64(),
            None => Some(1.0),
            _ => None,
        };
        let Some(steps) = steps.filter(|s| *s >= 1.0 && s.fract() == 0.0) else { continue };
        let steps = steps as i64;
        if years <= 0 {
            out.push(Advice {
                level: Level::Warning,
                at: vec![n.id.clone()],
                message: format!(
                    "t0 is of {y0} and t1 of {y1}: the period runs backwards or has no length; t0 is the earlier map"
                ),
                guide: "land-change",
            });
        } else if steps > years || years % steps != 0 {
            out.push(Advice {
                level: Level::Advice,
                at: vec![n.id.clone()],
                message: format!(
                    "the maps are of {y0} and {y1} ({years} years) and steps is {steps}: a step of {:.2} years. \
                     The rates are per step, so the loop that simulates must take steps of that length; with \
                     yearly steps, steps = {years} (or {} steps of {} years)",
                    years as f64 / steps as f64,
                    years / divisor(years, steps),
                    divisor(years, steps)
                ),
                guide: "land-change",
            });
        }
    }

    // weights of evidence without any distance among the drivers
    for n in of(&["woe.fit"]) {
        if let Some(false) = has_distance(&nodes, reads(n, "drivers"), inputs) {
            out.push(Advice {
                level: Level::Advice,
                at: vec![n.id.clone()],
                message: "none of the drivers is a distance: land change (deforestation above all) follows roads, \
                          rivers, towns and the edge of what already changed, and the distance to them is the \
                          strongest driver in most models; add distances (distance.to_class of the cleared class, \
                          vector.distance of roads and rivers) to the stack"
                    .into(),
                guide: "land-change",
            });
        }
    }

    // object-based classification: the training and the prediction objects, the validation samples, the rules' attributes
    out.extend(objects_advice(&nodes));

    // movement: location error, windows, sampling, the UD's grid, the speed limit
    out.extend(movement_advice(&nodes, inputs));

    // outputs that will not fit the disk
    out.extend(crate::disk::advice(m, r, dir));

    out.sort_by_key(|a| std::cmp::Reverse(a.level));
    out
}

/// What a reference reads as the node that makes it: its id, or the input's name (`$name`, `@name`).
fn origin(n: &Node, port: &str) -> Option<String> {
    let r = reads(n, port)?;
    if r.starts_with(['$', '@']) { Some(r.to_string()) } else { Some(r.split('.').next()?.to_string()) }
}

/// The nodes a reference reads, and those they read in turn (not the container's inputs).
fn upstream<'a>(nodes: &[&'a Node], r: &str) -> Vec<&'a Node> {
    let mut out: Vec<&Node> = Vec::new();
    let mut todo = vec![r.to_string()];
    while let Some(r) = todo.pop() {
        let Some(n) = source(nodes, &r) else { continue };
        if out.iter().any(|o| o.id == n.id) {
            continue;
        }
        out.push(n);
        for s in n.inputs.values() {
            if let Source::Ref(x) = s {
                todo.push(x.as_str().to_string());
            }
        }
    }
    out
}

/// The advice on object-based classification, from the graph alone: a model fitted on a table that is not of the
/// objects it classifies, validation samples that are the training ones, rules that read attributes no
/// objects.features node makes.
fn objects_advice(nodes: &[&Node]) -> Vec<Advice> {
    let mut out = Vec::new();
    let ops =
        |names: &[&str]| -> Vec<&Node> { nodes.iter().copied().filter(|n| names.contains(&n.op.as_str())).collect() };
    let trained = ops(&["objects.samples"]);
    for n in ops(&["objects.classify"]) {
        let Some(fit) = reads(n, "model").and_then(|r| source(nodes, r)).filter(|f| f.op == "ml.fit") else {
            continue;
        };
        let Some(table) = reads(fit, "table") else { continue };
        let made: Vec<&Node> = upstream(nodes, table).into_iter().filter(|u| u.op == "objects.samples").collect();
        let same = origin(n, "objects");
        if made.is_empty() {
            out.push(Advice {
                level: Level::Advice,
                at: vec![n.id.clone()],
                message: format!(
                    "the model ({}) is fitted on a table that does not come from objects.samples: the model reads \
                     the objects' attributes by name, so the table must hold the same attributes, made with the same \
                     objects.features settings; take the training table from objects.samples, or check that its \
                     columns match",
                    fit.id
                ),
                guide: "objects",
            });
        } else if same.is_some() && made.iter().all(|s| origin(s, "objects").is_some_and(|o| Some(&o) != same.as_ref()))
        {
            out.push(Advice {
                level: Level::Advice,
                at: vec![n.id.clone()],
                message: format!(
                    "the model is fitted on the samples of other objects ({}) than the ones it classifies ({}): \
                     fine when the training scene is another on purpose, but then make the attributes with the \
                     same objects.features settings in both, bring the bands to the same scale, and check that the \
                     classes look the same in the two",
                    made.iter().filter_map(|s| origin(s, "objects")).collect::<Vec<_>>().join(", "),
                    same.unwrap_or_default()
                ),
                guide: "objects",
            });
        }
    }
    // validation samples that are the training ones
    let training: Vec<(String, String)> =
        trained.iter().filter_map(|s| Some((origin(s, "samples")?, s.id.clone()))).collect();
    for n in ops(&["objects.accuracy"]) {
        let from_samples = origin(n, "samples")
            .and_then(|k| training.iter().find(|(o, _)| *o == k))
            .map(|(_, id)| id.clone())
            .or_else(|| {
                let r = reads(n, "reference")?;
                upstream(nodes, r).into_iter().find(|u| u.op == "objects.samples").map(|u| u.id.clone())
            });
        if let Some(id) = from_samples {
            out.push(Advice {
                level: Level::Warning,
                at: vec![n.id.clone()],
                message: format!(
                    "the accuracy is assessed with the samples the model was trained on (objects.samples {id}): it \
                     is optimistic, the model has seen them; validation samples must be kept apart: draw them \
                     separately (objects.sample draws a probability sample), not used to train"
                ),
                guide: "objects",
            });
        }
    }
    // rules that read attributes nothing makes
    for n in ops(&["objects.rules"]) {
        let Some(given) = n.params.get("classes").and_then(|v| terraflux_ops::objects::tree(v).ok()) else { continue };
        let Some(objects) = reads(n, "objects") else { continue };
        let up = upstream(nodes, objects);
        if up.iter().any(|u| u.op == "objects.calc") {
            continue;
        }
        let makers: Vec<&&Node> = up.iter().filter(|u| u.op == "objects.features").collect();
        if makers.is_empty() {
            continue;
        }
        let mut missing: Vec<(String, &str)> = Vec::new();
        for text in given.iter().filter_map(|c| c.rule.as_deref()) {
            let Ok(script) = terraflux_ops::algebra::parse_script_objects(text) else { continue };
            use terraflux_ops::algebra::Stmt;
            let mut names: Vec<String> = Vec::new();
            let mut set: Vec<&str> = Vec::new();
            for (_, s) in &script.statements {
                match s {
                    Stmt::Set { name, layer, mask, value, .. } => {
                        names.extend(value.names());
                        names.extend(mask.iter().flat_map(|m| m.names()));
                        if layer.is_none() {
                            set.push(name);
                        }
                    }
                    Stmt::Out(_) => {}
                    Stmt::Value(e) => names.extend(e.names()),
                }
            }
            for name in names.into_iter().filter(|x| !set.contains(&x.as_str())) {
                let gives: Vec<Option<(&str, bool)>> = makers
                    .iter()
                    .map(|f| terraflux_ops::objects::features_give(f.params.get("features"), &name))
                    .collect();
                if gives.iter().any(Option::is_none) || gives.iter().any(|g| g.is_some_and(|g| g.1)) {
                    continue;
                }
                if let Some((group, _)) = gives[0]
                    && !missing.iter().any(|(m, _)| *m == name)
                {
                    missing.push((name, group));
                }
            }
        }
        if !missing.is_empty() {
            let mut groups: Vec<&str> = missing.iter().map(|m| m.1).collect();
            groups.sort();
            groups.dedup();
            let names: Vec<&str> = missing.iter().map(|m| m.0.as_str()).collect();
            out.push(Advice {
                level: Level::Warning,
                at: vec![n.id.clone()],
                message: format!(
                    "the rules read {}, which no objects.features node before them makes with its settings: the \
                     rule would find no such attribute; add {} to `features` of {} (or `all`)",
                    names.join(", "),
                    groups.join(" and "),
                    makers.iter().map(|f| f.id.as_str()).collect::<Vec<_>>().join(" or ")
                ),
                guide: "objects",
            });
        }
    }
    out
}

/// The operators that solve a prioritization (their solution worth evaluating).
const PRIORITIZATIONS: [&str; 4] =
    ["prioritize.optimize", "prioritize.marxan", "prioritize.actions", "prioritize.actions_raster"];

/// The operators that clean or thin the records of a species.
const THINNING: [&str; 4] = ["sdm.thin", "sdm.one_per_cell", "sdm.prepare", "sdm.clean_occurrences"];

/// The operators that make the sampling interval regular, or tell it (the answer to its irregularity).
const SAMPLING: [&str; 6] = [
    "move.thin_time",
    "move.resample",
    "move.filter_per_interval",
    "move.suggest_window",
    "move.time_lags",
    "move.interpolate",
];

/// A literal setting of a node, as a number (none: not given, or not a plain number).
fn number_of(n: &Node, name: &str) -> Option<f64> {
    n.params.get(name).and_then(|v| v.as_f64())
}

/// Whether a setting is given: not missing, null, empty, `none` or 0 (a setting read from an input counts as given).
fn given(n: &Node, name: &str) -> bool {
    match n.params.get(name) {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::String(s)) => !(s.trim().is_empty() || s.trim().eq_ignore_ascii_case("none")),
        Some(serde_json::Value::Number(x)) => x.as_f64().is_some_and(|x| x != 0.0),
        Some(_) => true,
    }
}

/// The advice on movement analysis, from the graph (and the tracks of the inputs, when loaded): the dynamic Brownian
/// bridges without the location error or with a window the method cannot use well, a utilisation distribution
/// on a grid of too many cells, a regular sampling nothing in the model secures, a speed limit not given or given
/// twice, speeds flagged before the GPS quality is.
fn movement_advice(nodes: &[&Node], inputs: Option<&BTreeMap<String, Value>>) -> Vec<Advice> {
    let mut out = Vec::new();
    let has = |ops: &[&str]| nodes.iter().any(|n| ops.contains(&n.op.as_str()));
    let model_of = |op: &str| if op.starts_with("move.dbgb") { "dBGB" } else { "dBBMM" };
    let is_bridge =
        |op: &str| ["move.dbbmm_variance", "move.dbbmm_ud", "move.dbgb_variance", "move.dbgb_ud"].contains(&op);
    for n in nodes.iter().copied().filter(|n| is_bridge(&n.op)) {
        let is_ud = n.op.ends_with("_ud");
        // (a UD reading the variance of another node uses its columns: the variance node is the one to check)
        let variance_upstream = is_ud
            && reads(n, "tracks")
                .is_some_and(|r| upstream(nodes, r).iter().any(|u| u.op == n.op.replace("_ud", "_variance")));
        let at = vec![n.id.clone()];
        let what = model_of(&n.op);
        if !variance_upstream {
            if !given(n, "location_error") {
                out.push(Advice {
                    level: Level::Warning,
                    at: at.clone(),
                    message: format!(
                        "no location_error: the {what} adds the error of each fix to the bridge's variance, and \
                         without it the error of a GPS is read as movement, so the variance and the distribution \
                         come out too large. Give the standard deviation of the error in metres (a number, a column \
                         of the tracks, or auto for Movebank's accuracy columns)"
                    ),
                    guide: "movement",
                });
            }
            let default_margin = if what == "dBGB" { 15 } else { 11 };
            let (size, margin) = (
                number_of(n, "window_size").map_or(31, |x| x as i64),
                number_of(n, "margin").map_or(default_margin, |x| x as i64),
            );
            let explicit = n.params.contains_key("window_size") || n.params.contains_key("margin");
            if size % 2 == 0 || margin % 2 == 0 || size < 2 * margin + 1 {
                out.push(Advice {
                    level: Level::Warning,
                    at: at.clone(),
                    message: format!(
                        "window_size {size} and margin {margin}: both must be odd and window_size at least \
                         2 * margin + 1 ({}), or the node is refused; move.suggest_window proposes them from the \
                         sampling",
                        2 * margin + 1
                    ),
                    guide: "movement",
                });
            } else if explicit && size - 2 * margin < 3 {
                out.push(Advice {
                    level: Level::Advice,
                    at: at.clone(),
                    message: format!(
                        "window_size {size} with margin {margin} leaves {} location(s) of each window to the \
                         estimate: the variance of the {what} is then close to the bridge's of one location. A \
                         margin of about a quarter of the window is the usual (move.suggest_window)",
                        size - 2 * margin
                    ),
                    guide: "movement",
                });
            } else if explicit && size < 11 {
                out.push(Advice {
                    level: Level::Advice,
                    at: at.clone(),
                    message: format!(
                        "a window of {size} locations is short for a variance (move.suggest_window takes 11 at \
                         least): the estimate follows the noise"
                    ),
                    guide: "movement",
                });
            }
        }
        if is_ud {
            // the grid: cells along the longest side, or the cells' size over the tracks' extent
            let cells = if let Some(d) = number_of(n, "dim_size") {
                Some(d * d)
            } else if let (Some(res), Some(Value::Vector(v))) = (
                number_of(n, "res").filter(|r| *r > 0.0),
                reads(n, "tracks").and_then(|r| inputs?.get(r.strip_prefix('$')?)),
            ) {
                v.bounds().map(|b| {
                    let widen = 1.0 + 2.0 * number_of(n, "ext_factor").unwrap_or(0.5);
                    ((b[2] - b[0]) * widen / res).ceil().max(1.0) * ((b[3] - b[1]) * widen / res).ceil().max(1.0)
                })
            } else {
                None
            };
            if let Some(c) = cells.filter(|c| *c > 4.0e6) {
                out.push(Advice {
                    level: Level::Advice,
                    at: at.clone(),
                    message: format!(
                        "the grid has about {c:.1e} cells: the cost of the distribution grows with the cells times \
                         the steps of the bridges, and it is memory too. Run a coarse grid first (dim_size 100 or a \
                         larger res), and a fine one only for the final figure; a cell much smaller than the \
                         location error says nothing more"
                    ),
                    guide: "movement",
                });
            }
        }
    }
    // the sampling interval: the bridges, the corridors and the steps read the lags as if they were regular
    let needs_regular: Vec<&Node> = nodes
        .iter()
        .copied()
        .filter(|n| is_bridge(&n.op) || n.op == "move.corridor" || n.op == "move.random_steps")
        .collect();
    if !needs_regular.is_empty() && !has(&SAMPLING) {
        out.push(Advice {
            level: Level::Advice,
            at: ids(&needs_regular),
            message: "nothing in the model looks at the sampling interval, and these read the lags between fixes as \
                      if they were regular (a corridor's search radius, a step selection's steps, a bridge's \
                      window): see the lags (move.time_lags, move.suggest_window), then thin to one interval \
                      (move.thin_time) or cut at the gaps (move.resample, move.mask_segments)"
                .into(),
            guide: "movement",
        });
    }
    for n in nodes.iter().copied().filter(|n| n.op == "move.flag_speed") {
        let (v_max, mass, mode) = (given(n, "v_max"), given(n, "mass"), given(n, "mode"));
        if !v_max && !(mass && mode) {
            out.push(Advice {
                level: Level::Warning,
                at: vec![n.id.clone()],
                message: "no speed limit: give v_max (m/s, the species' own when it is known), or the body mass and \
                          the locomotion mode (flying, running or swimming: move.max_speed says what limit they \
                          make); the node is refused without one"
                    .into(),
                guide: "movement",
            });
        } else if v_max && (mass || mode) {
            out.push(Advice {
                level: Level::Warning,
                at: vec![n.id.clone()],
                message: "v_max and mass or mode both given: give the limit one way, v_max or the body mass with its \
                          mode; the node is refused with both"
                    .into(),
                guide: "movement",
            });
        }
        // (tracks read from an input of the model may have been filtered before they came: not told)
        let from_node = reads(n, "tracks").is_some_and(|r| !r.starts_with(['$', '@']));
        let filtered =
            reads(n, "tracks").is_some_and(|r| upstream(nodes, r).iter().any(|u| u.op == "move.filter_gps_quality"));
        if from_node && !filtered {
            out.push(Advice {
                level: Level::Advice,
                at: vec![n.id.clone()],
                message: "the speeds are flagged on tracks whose GPS quality is not filtered first: poor fixes (few \
                          satellites, a high dilution of precision) make the false jumps, and a limit tight enough \
                          for them cuts real movement too. Put move.filter_gps_quality upstream (it also drops the \
                          locations without a position, which move.flag_speed needs gone)"
                    .into(),
                guide: "movement",
            });
        }
    }
    out
}

/// The number of years of a step close to the one asked that divides the period (for the advice).
fn divisor(years: i64, steps: i64) -> i64 {
    let per = (years as f64 / steps as f64).round().max(1.0) as i64;
    (1..=years).filter(|d| years % d == 0).min_by_key(|d| (d - per).abs()).unwrap_or(1)
}

/// The node a reference reads (`node.port`, `node`), among the nodes.
fn source<'a>(nodes: &[&'a Node], r: &str) -> Option<&'a Node> {
    let id = r.split('.').next()?;
    nodes.iter().copied().find(|n| n.id == id)
}

/// A year in a name (four digits from 1900 to 2100, not part of a longer number).
pub(crate) fn year_in(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    (0..b.len().saturating_sub(3)).find_map(|i| {
        let digits = b[i..i + 4].iter().all(u8::is_ascii_digit);
        let alone = (i == 0 || !b[i - 1].is_ascii_digit()) && b.get(i + 4).is_none_or(|c| !c.is_ascii_digit());
        let y: i64 = s.get(i..i + 4)?.parse().ok()?;
        (digits && alone && (1900..=2100).contains(&y)).then_some(y)
    })
}

/// The year of a map a reference reads: in its file's name (raster.read), the node's id or the input's name.
fn year_of(nodes: &[&Node], r: Option<&str>) -> Option<i64> {
    let r = r?;
    if let Some(name) = r.strip_prefix('$') {
        return year_in(name);
    }
    let n = source(nodes, r)?;
    n.params
        .get("path")
        .and_then(|p| p.as_str())
        .and_then(|p| year_in(Path::new(p).file_name()?.to_str()?))
        .or_else(|| year_in(&n.id))
}

/// Whether what a reference reads has a distance among its sources (an operator of distances, or a layer, a node or
/// a file named so): `None` when it cannot be told (a file or an input not loaded, without such a name).
fn has_distance(nodes: &[&Node], r: Option<&str>, inputs: Option<&BTreeMap<String, Value>>) -> Option<bool> {
    let named = |s: &str| {
        let s = s.to_lowercase();
        s.contains("dist") || s.contains("cost")
    };
    let mut todo = vec![r?.to_string()];
    let mut seen = std::collections::HashSet::new();
    let mut unknown = false;
    while let Some(r) = todo.pop() {
        if !seen.insert(r.clone()) {
            continue;
        }
        if let Some(name) = r.strip_prefix('$') {
            if named(name) {
                return Some(true);
            }
            match inputs.and_then(|i| i.get(name)) {
                Some(Value::Stack(s)) => {
                    if s.keys.iter().any(|k| matches!(k, terraflux_ops::Key::Name(n) if named(n))) {
                        return Some(true);
                    }
                }
                Some(Value::Map(_)) => {}
                _ => unknown = true,
            }
            continue;
        }
        if r.starts_with('@') {
            unknown = true;
            continue;
        }
        let Some(n) = source(nodes, &r) else {
            unknown = true;
            continue;
        };
        if n.op.contains("distance") || n.op == "dinamica.cost_map" || n.op.starts_with("cost.") || named(&n.id) {
            return Some(true);
        }
        if n.inputs.keys().any(|k| named(k)) {
            return Some(true);
        }
        if n.op == "raster.read" || n.op == "vector.read" {
            let path = n.params.get("path").and_then(|p| p.as_str()).unwrap_or("");
            if named(path) {
                return Some(true);
            }
            unknown = true;
        }
        for s in n.inputs.values() {
            if let Source::Ref(x) = s {
                todo.push(x.as_str().to_string());
            }
        }
    }
    (!unknown).then_some(false)
}

/// The advice told after a run: the warnings and a few pieces of advice, a line each (none: empty).
pub fn after_run(m: &Model, r: &Registry, dir: &Path, inputs: Option<&BTreeMap<String, Value>>) -> String {
    // (the evaluation's plain mode: no help of the method)
    if crate::tools::plain() {
        return String::new();
    }
    let all = advise(m, r, dir, inputs);
    if all.is_empty() {
        return String::new();
    }
    let mut s = String::from("\nOn the method (check_model tells it all):");
    for a in all.iter().take(3) {
        s.push_str(&format!("\n- {a}"));
    }
    if all.len() > 3 {
        s.push_str(&format!("\n- ... and {} more", all.len() - 3));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terraflux_ops::Map;

    fn registry() -> Registry {
        Registry::standard()
    }

    fn simulation() -> Model {
        let mut m = Model::new("sim");
        m.nodes.push(Node::new("rates", "markov.fit").input("t0", "$t0").input("t1", "$t1"));
        m.nodes.push(Node::new("weights", "woe.fit").input("t0", "$t0").input("t1", "$t1"));
        m.nodes.push(Node::new("step", "dinamica.step").input("landscape", "$t1"));
        m.plots.push(terraflux_graph::Plot {
            id: "plot".into(),
            layers: vec![terraflux_graph::PlotLayer::new("step.landscape")],
            ..Default::default()
        });
        m
    }

    #[test]
    fn a_simulation_not_validated() {
        let a = advise(&simulation(), &registry(), Path::new("."), None);
        let text: Vec<String> = a.iter().map(|a| a.to_string()).collect();
        assert!(text.iter().any(|t| t.contains("never compares") && t.contains("step")), "{text:?}");
        assert!(text.iter().any(|t| t.contains("several seeds")), "{text:?}");
    }

    #[test]
    fn validated_on_the_calibration_map() {
        let mut m = simulation();
        m.nodes.push(
            Node::new("fom", "validate.three_map")
                .input("t0", "$t0")
                .input("t1", "$t1")
                .input("simulated", "step.landscape"),
        );
        let a = advise(&m, &registry(), Path::new("."), None);
        let w: Vec<&Advice> = a.iter().filter(|a| a.level == Level::Warning).collect();
        assert_eq!(w.len(), 1, "{a:?}");
        assert!(
            w[0].message.contains("rates (markov.fit) and weights (woe.fit) were calibrated on"),
            "{}",
            w[0].message
        );
        assert!(!a.iter().any(|a| a.message.contains("never compares")));
        // validated on a later map: no warning
        let mut m = simulation();
        m.nodes.push(
            Node::new("fom", "validate.three_map")
                .input("t0", "$t1")
                .input("t1", "$t2")
                .input("simulated", "step.landscape"),
        );
        assert!(advise(&m, &registry(), Path::new("."), None).iter().all(|a| a.level != Level::Warning));
    }

    #[test]
    fn seeds_run_in_a_loop_are_not_one_run() {
        let mut m = simulation();
        m.nodes.push(Node::new("seeds", "control.each"));
        assert!(!advise(&m, &registry(), Path::new("."), None).iter().any(|a| a.message.contains("several seeds")));
    }

    fn map(geographic: bool, data: Vec<f64>) -> Value {
        let mut g = Grid::new(2, 2);
        g.geo.wkt = Some(if geographic { "GEOGCS[\"WGS 84\"]".into() } else { "PROJCS[\"UTM\"]".into() });
        Value::Map(Map::new(Arc::new(g), data))
    }

    #[test]
    fn metrics_on_degrees_and_on_a_continuous_map() {
        let mut m = Model::new("metrics");
        m.nodes.push(Node::new("lsm", "metrics.landscape").input("map", "$land"));
        let r = registry();
        let inputs = BTreeMap::from([("land".to_string(), map(true, vec![1.0, 2.0, 1.0, 2.0]))]);
        let a = advise(&m, &r, Path::new("."), Some(&inputs));
        assert!(a.iter().any(|a| a.level == Level::Warning && a.message.contains("longitudes")), "{a:?}");
        let inputs = BTreeMap::from([("land".to_string(), map(false, vec![0.1, 0.5, 0.7, 0.9]))]);
        let a = advise(&m, &r, Path::new("."), Some(&inputs));
        assert!(!a.iter().any(|a| a.message.contains("longitudes")), "{a:?}");
        assert!(a.iter().any(|a| a.message.contains("not whole")), "{a:?}");
        // reprojected: no warning on degrees
        m.nodes.push(Node::new("utm", "raster.reproject").input("raster", "$land"));
        let inputs = BTreeMap::from([("land".to_string(), map(true, vec![1.0, 2.0, 1.0, 2.0]))]);
        assert!(!advise(&m, &r, Path::new("."), Some(&inputs)).iter().any(|a| a.message.contains("longitudes")));
    }

    #[test]
    fn metrics_on_a_map_of_too_many_classes() {
        let mut m = Model::new("metrics");
        m.nodes.push(Node::new("lsm", "metrics.landscape").input("map", "$land"));
        let r = registry();
        let many = |n: usize, k: usize| {
            let mut g = Grid::new(n, 1);
            g.geo.wkt = Some("PROJCS[\"UTM\"]".into());
            Value::Map(Map::new(Arc::new(g), (0..n).map(|i| (i % k) as f64).collect()))
        };
        let msg = "more than 100 classes";
        let inputs = BTreeMap::from([("land".to_string(), many(300, 150))]);
        let a = advise(&m, &r, Path::new("."), Some(&inputs));
        assert!(a.iter().any(|a| a.level == Level::Advice && a.message.contains(msg)), "{a:?}");
        let inputs = BTreeMap::from([("land".to_string(), many(300, 5))]);
        let a = advise(&m, &r, Path::new("."), Some(&inputs));
        assert!(!a.iter().any(|a| a.message.contains(msg)), "{a:?}");
        let inputs = BTreeMap::from([("land".to_string(), many(300, 150))]);
        let Some(Value::Map(mp)) = inputs.get("land") else { panic!() };
        let data: Vec<f64> = mp.data.iter().map(|v| v + 0.5).collect();
        let inputs = BTreeMap::from([("land".to_string(), Value::Map(Map::new(mp.grid.clone(), data)))]);
        let a = advise(&m, &r, Path::new("."), Some(&inputs));
        assert!(a.iter().any(|a| a.message.contains("not whole")), "{a:?}");
        assert!(!a.iter().any(|a| a.message.contains(msg)), "{a:?}");
    }

    #[test]
    fn random_folds() {
        let mut m = Model::new("sdm");
        m.nodes.push(Node::new("folds", "sdm.random_folds"));
        m.nodes.push(Node::new("fit", "sdm.fit").input("records", "folds.records"));
        let a = advise(&m, &registry(), Path::new("."), None);
        assert!(a.iter().any(|a| a.message.contains("spatial folds")), "{a:?}");
        m.nodes.push(Node::new("blocks", "sdm.spatial_blocks"));
        let a = advise(&m, &registry(), Path::new("."), None);
        assert!(!a.iter().any(|a| a.message.contains("spatial folds")), "{a:?}");
    }

    #[test]
    fn records_not_thinned_and_a_prioritization_not_evaluated() {
        let r = registry();
        let mut m = Model::new("sdm");
        m.nodes.push(Node::new("fit", "sdm.fit").input("records", "$records"));
        assert!(advise(&m, &r, Path::new("."), None).iter().any(|a| a.message.contains("sdm.one_per_cell")));
        m.nodes.push(Node::new("thin", "sdm.thin").input("records", "$records"));
        assert!(!advise(&m, &r, Path::new("."), None).iter().any(|a| a.message.contains("sdm.one_per_cell")));
        let mut m = Model::new("plan");
        m.nodes.push(Node::new("solve", "prioritize.optimize"));
        let a = advise(&m, &r, Path::new("."), None);
        assert!(a.iter().any(|a| a.message.contains("prioritize.evaluate") && a.at == ["solve (prioritize.optimize)"]));
        m.nodes.push(Node::new("eval", "prioritize.evaluate").input("solution", "solve.solution"));
        assert!(advise(&m, &r, Path::new("."), None).is_empty());
    }

    #[test]
    fn a_resistance_of_zero() {
        let r = registry();
        let mut m = Model::new("circuits");
        m.nodes.push(Node::new("pairs", "circuit.pairwise").input("habitat", "$res").input("points", "$points"));
        let inputs = BTreeMap::from([("res".to_string(), map(false, vec![1.0, 0.0, 5.0, f64::NAN]))]);
        let a = advise(&m, &r, Path::new("."), Some(&inputs));
        assert!(a.iter().any(|a| a.level == Level::Warning && a.message.contains("cells of 0")), "{a:?}");
        let inputs = BTreeMap::from([("res".to_string(), map(false, vec![1.0, 2.0, 5.0, f64::NAN]))]);
        assert!(advise(&m, &r, Path::new("."), Some(&inputs)).is_empty());
    }

    #[test]
    fn the_markov_steps_against_the_dates() {
        let r = registry();
        let markov = |steps: i64| {
            let mut m = Model::new("rates");
            m.nodes.push(Node::new("land_2000", "raster.read").param("path", "maps/land_2000.tif"));
            m.nodes.push(Node::new("land_2010", "raster.read").param("path", "maps/land_2010.tif"));
            m.nodes.push(
                Node::new("rates", "markov.fit")
                    .input("t0", "land_2000.map")
                    .input("t1", "land_2010.map")
                    .literal("steps", serde_json::json!(steps)),
            );
            advise(&m, &r, Path::new("."), None)
        };
        for good in [1, 2, 5, 10] {
            assert!(!markov(good).iter().any(|a| a.message.contains("steps is")), "{good}");
        }
        let a = markov(3);
        assert!(
            a.iter().any(|a| a.message.contains("10 years) and steps is 3") && a.message.contains("steps = 10")),
            "{a:?}"
        );
        assert!(markov(20).iter().any(|a| a.message.contains("steps is 20")));
        // the years of the inputs' names
        assert_eq!(year_of(&[], Some("$land1995")), Some(1995));
        assert_eq!(year_in("cover_20101"), None);
    }

    #[test]
    fn weights_of_evidence_without_distances() {
        let r = registry();
        let woe = |driver_op: &str, id: &str| {
            let mut m = Model::new("woe");
            m.nodes.push(Node::new(id, driver_op));
            m.nodes.push(Node::new("drivers", "stack.of").input("slope", &format!("{id}.map")));
            m.nodes.push(
                Node::new("w", "woe.fit").input("t0", "$t0").input("t1", "$t1").input("drivers", "drivers.stack"),
            );
            advise(&m, &r, Path::new("."), None)
        };
        assert!(woe("nlm.random", "slope").iter().any(|a| a.message.contains("none of the drivers is a distance")));
        assert!(!woe("distance.to_class", "roads").iter().any(|a| a.message.contains("none of the drivers")));
        assert!(!woe("nlm.random", "dist_river").iter().any(|a| a.message.contains("none of the drivers")));
        // a file of unknown layers: not told
        assert!(!woe("raster.read", "drivers_file").iter().any(|a| a.message.contains("none of the drivers")));
    }

    /// Features, samples of `samples_of`, a fit on their table and a classification of `classify_on`'s objects.
    fn classification(samples_objects: &str, classify_objects: &str) -> Model {
        let mut m = Model::new("obia");
        m.nodes.push(Node::new("feats", "objects.features").input("objects", "$objects").input("image", "$image"));
        m.nodes.push(Node::new("other", "objects.features").input("objects", "$scene2").input("image", "$image2"));
        m.nodes
            .push(Node::new("samples", "objects.samples").input("objects", samples_objects).input("samples", "$train"));
        m.nodes.push(Node::new("fit", "ml.fit").input("table", "samples.table"));
        m.nodes.push(
            Node::new("classify", "objects.classify").input("objects", classify_objects).input("model", "fit.model"),
        );
        m
    }

    #[test]
    fn a_model_fitted_on_other_objects_than_it_classifies() {
        let r = registry();
        let msg = "fitted on the samples of other objects";
        let a = advise(&classification("feats.objects", "feats.objects"), &r, Path::new("."), None);
        assert!(a.is_empty(), "{a:?}");
        let a = advise(&classification("other.objects", "feats.objects"), &r, Path::new("."), None);
        assert!(a.iter().any(|a| a.level == Level::Advice && a.message.contains(msg) && a.at == ["classify"]), "{a:?}");
        // a table that is not objects.samples'
        let mut m = classification("feats.objects", "feats.objects");
        m.nodes.retain(|n| n.id != "fit");
        m.nodes.push(Node::new("fit", "ml.fit").input("table", "$table"));
        let a = advise(&m, &r, Path::new("."), None);
        assert!(a.iter().any(|a| a.message.contains("does not come from objects.samples")), "{a:?}");
        assert!(!a.iter().any(|a| a.message.contains(msg)), "{a:?}");
    }

    #[test]
    fn validation_samples_that_are_the_training_ones() {
        let r = registry();
        let with = |samples: &str| {
            let mut m = classification("feats.objects", "feats.objects");
            m.nodes.push(
                Node::new("acc", "objects.accuracy").input("objects", "classify.objects").input("samples", samples),
            );
            advise(&m, &r, Path::new("."), None)
        };
        let a = with("$train");
        assert!(
            a.iter().any(|a| a.level == Level::Warning && a.message.contains("kept apart") && a.at == ["acc"]),
            "{a:?}"
        );
        assert!(!with("$validation").iter().any(|a| a.message.contains("kept apart")));
        // the reference as the training table
        let mut m = classification("feats.objects", "feats.objects");
        m.nodes.push(
            Node::new("acc", "objects.accuracy")
                .input("objects", "classify.objects")
                .input("reference", "samples.table"),
        );
        assert!(advise(&m, &r, Path::new("."), None).iter().any(|a| a.message.contains("kept apart")));
    }

    #[test]
    fn rules_that_read_what_the_features_do_not_make() {
        let r = registry();
        let model = |features: Option<serde_json::Value>, rule: &str| {
            let mut f = Node::new("feats", "objects.features").input("objects", "$objects").input("image", "$image");
            if let Some(v) = features {
                f = f.param("features", v);
            }
            let mut m = Model::new("rules");
            m.nodes.push(f);
            m.nodes.push(
                Node::new("rules", "objects.rules")
                    .input("objects", "feats.objects")
                    .param("classes", serde_json::json!([{ "name": "field", "rule": rule }])),
            );
            advise(&m, &r, Path::new("."), None)
        };
        let rule = "above(rectangular_fit, 0.5, 0.9) and above(ndvi, 0.2, 0.4)";
        let a = model(None, rule);
        let w: Vec<&Advice> = a.iter().filter(|a| a.message.contains("the rules read")).collect();
        assert_eq!(w.len(), 1, "{a:?}");
        assert!(
            w[0].level == Level::Warning
                && w[0].message.contains("rectangular_fit, which")
                && w[0].message.contains("add shape")
        );
        assert!(!w[0].message.contains("ndvi"), "{}", w[0].message);
        for ok in [
            serde_json::json!(["spectral", "shape"]),
            serde_json::json!("all"),
            serde_json::json!(["rectangular_fit", "ndvi"]),
        ] {
            assert!(!model(Some(ok), rule).iter().any(|a| a.message.contains("the rules read")));
        }
        // an attribute of the user's, or of the table itself: not told
        assert!(
            !model(None, "above(my_index, 0, 1) and above(area, 0, 100)")
                .iter()
                .any(|a| a.message.contains("the rules read"))
        );
        // a texture column of a band
        let a = model(None, "above(glcm_contrast_b4, 0.1, 0.5)");
        assert!(a.iter().any(|a| a.message.contains("glcm_contrast_b4") && a.message.contains("add texture")), "{a:?}");
    }

    fn movement(nodes: Vec<Node>, inputs: Option<&BTreeMap<String, Value>>) -> Vec<Advice> {
        let mut m = Model::new("move");
        m.nodes.extend(nodes);
        advise(&m, &registry(), Path::new("."), inputs)
    }

    fn said(a: &[Advice], level: Level, text: &str) -> bool {
        a.iter().any(|a| a.level == level && a.message.contains(text))
    }

    #[test]
    fn bridges_without_the_location_error() {
        let ud = || Node::new("ud", "move.dbbmm_ud").input("tracks", "$tracks").param("dim_size", 60);
        let a = movement(vec![ud()], None);
        assert!(said(&a, Level::Warning, "no location_error"), "{a:?}");
        assert_eq!(a.iter().find(|a| a.message.contains("no location_error")).unwrap().guide, "movement");
        // (none, an empty text and 0 are none as well)
        for none in [serde_json::json!("none"), serde_json::json!(0), serde_json::json!("")] {
            let a = movement(vec![ud().param("location_error", none)], None);
            assert!(said(&a, Level::Warning, "no location_error"), "{a:?}");
        }
        for given in [serde_json::json!(10), serde_json::json!("auto"), serde_json::json!("accuracy")] {
            let a = movement(vec![ud().param("location_error", given)], None);
            assert!(!said(&a, Level::Warning, "no location_error"), "{a:?}");
        }
        // the directional one too, and the variance alone
        let a = movement(vec![Node::new("v", "move.dbgb_variance").input("tracks", "$tracks")], None);
        assert!(said(&a, Level::Warning, "dBGB"), "{a:?}");
        // a UD that reads the variance of another node has its columns: that node is the one told
        let var = Node::new("var", "move.dbbmm_variance").input("tracks", "$tracks");
        let a = movement(vec![var.clone(), ud().input("tracks", "var.tracks")], None);
        assert_eq!(a.iter().filter(|a| a.message.contains("no location_error")).count(), 1, "{a:?}");
        assert_eq!(a.iter().find(|a| a.message.contains("no location_error")).unwrap().at, ["var"]);
        let a = movement(vec![var.param("location_error", 5), ud().input("tracks", "var.tracks")], None);
        assert!(!said(&a, Level::Warning, "no location_error"), "{a:?}");
    }

    #[test]
    fn windows_the_method_cannot_use() {
        let var = |size: i64, margin: i64| {
            Node::new("v", "move.dbbmm_variance")
                .input("tracks", "$tracks")
                .param("location_error", 10)
                .param("window_size", size)
                .param("margin", margin)
        };
        // refused: even, or too small for the margin
        assert!(said(&movement(vec![var(30, 11)], None), Level::Warning, "both must be odd"));
        assert!(said(&movement(vec![var(31, 10)], None), Level::Warning, "both must be odd"));
        let a = movement(vec![var(21, 11)], None);
        assert!(said(&a, Level::Warning, "at least 2 * margin + 1 (23)"), "{a:?}");
        // accepted but with next to no room: a core of one location
        let a = movement(vec![var(23, 11)], None);
        assert!(said(&a, Level::Advice, "leaves 1 location(s)"), "{a:?}");
        // a short window
        let a = movement(vec![var(9, 3)], None);
        assert!(said(&a, Level::Advice, "a window of 9 locations is short"), "{a:?}");
        // the usual: none
        let a = movement(vec![var(31, 11)], None);
        assert!(!a.iter().any(|a| a.message.contains("window_size") || a.message.contains("location(s)")), "{a:?}");
        // the defaults of the dBBMM are fine, the dBGB's say nothing unless they are given
        let a = movement(
            vec![Node::new("v", "move.dbgb_variance").input("tracks", "$tracks").param("location_error", 5)],
            None,
        );
        assert!(!a.iter().any(|a| a.message.contains("window_size") || a.message.contains("location(s)")), "{a:?}");
        // the margin's default of the dBGB (15) against a window given alone
        let a = movement(
            vec![
                Node::new("v", "move.dbgb_variance")
                    .input("tracks", "$tracks")
                    .param("location_error", 5)
                    .param("window_size", 31),
            ],
            None,
        );
        assert!(said(&a, Level::Advice, "leaves 1 location(s)"), "{a:?}");
    }

    #[test]
    fn a_grid_of_too_many_cells() {
        use terraflux_raster::vector::{Geometry, Vector};
        let ud = |k: &str, v: serde_json::Value| {
            Node::new("ud", "move.dbbmm_ud").input("tracks", "$tracks").param("location_error", 10).param(k, v)
        };
        let a = movement(vec![ud("dim_size", 3000.into())], None);
        assert!(said(&a, Level::Advice, "cells"), "{a:?}");
        assert!(!said(&movement(vec![ud("dim_size", 100.into())], None), Level::Advice, "cells"));
        // a size of cell against the extent of the tracks, when they are loaded: 10 km by 10 km in cells of 1 m
        let tracks = Value::Vector(Arc::new(Vector {
            geometries: vec![Geometry::Point([0.0, 0.0]), Geometry::Point([10_000.0, 10_000.0])],
            attributes: Vec::new(),
            crs: None,
        }));
        let inputs = BTreeMap::from([("tracks".to_string(), tracks)]);
        let a = movement(vec![ud("res", 1.into())], Some(&inputs));
        assert!(said(&a, Level::Advice, "cells"), "{a:?}");
        let a = movement(vec![ud("res", 100.into())], Some(&inputs));
        assert!(!said(&a, Level::Advice, "cells"), "{a:?}");
        // (not loaded: not told)
        assert!(!said(&movement(vec![ud("res", 1.into())], None), Level::Advice, "cells"));
    }

    #[test]
    fn a_sampling_nothing_looks_at() {
        let ud = Node::new("ud", "move.dbbmm_ud").input("tracks", "$tracks").param("location_error", 10);
        let a = movement(vec![ud.clone()], None);
        assert!(said(&a, Level::Advice, "sampling interval") && a.iter().all(|a| a.level != Level::Warning), "{a:?}");
        let thin = Node::new("thin", "move.thin_time").input("tracks", "$tracks").param("interval", "1 hours");
        assert!(!said(&movement(vec![ud.clone(), thin], None), Level::Advice, "sampling interval"));
        let lags = Node::new("window", "move.suggest_window").input("tracks", "$tracks");
        assert!(!said(&movement(vec![ud, lags], None), Level::Advice, "sampling interval"));
        let corridor = Node::new("c", "move.corridor").input("tracks", "$tracks");
        assert!(said(&movement(vec![corridor], None), Level::Advice, "sampling interval"));
    }

    #[test]
    fn a_speed_limit_not_given() {
        let flag = || Node::new("speed", "move.flag_speed").input("tracks", "$tracks");
        let a = movement(vec![flag()], None);
        assert!(said(&a, Level::Warning, "no speed limit"), "{a:?}");
        let a = movement(vec![flag().param("mass", 5)], None);
        assert!(said(&a, Level::Warning, "no speed limit"), "{a:?}");
        let a = movement(vec![flag().param("mode", "flying")], None);
        assert!(said(&a, Level::Warning, "no speed limit"), "{a:?}");
        for ok in [flag().param("v_max", 30), flag().param("mass", 5).param("mode", "flying")] {
            let a = movement(vec![ok], None);
            assert!(a.iter().all(|a| a.level != Level::Warning), "{a:?}");
        }
        let a = movement(vec![flag().param("v_max", 30).param("mass", 5)], None);
        assert!(said(&a, Level::Warning, "both given"), "{a:?}");
        // the GPS quality first, when the tracks come from a node of the model
        let read = Node::new("read", "move.read_tracks").param("path", "tracks.csv");
        let speed = || Node::new("speed", "move.flag_speed").input("tracks", "read.tracks").param("v_max", 30);
        let a = movement(vec![read.clone(), speed()], None);
        assert!(said(&a, Level::Advice, "GPS quality is not filtered"), "{a:?}");
        let gps = Node::new("gps", "move.filter_gps_quality").input("tracks", "read.tracks");
        let a = movement(vec![read, gps, speed().input("tracks", "gps.tracks")], None);
        assert!(a.is_empty(), "{a:?}");
    }

    #[test]
    fn an_energy_landscape_needs_the_mass() {
        let en = || Node::new("en", "cost.energy_landscape").input("dem", "$dem");
        let a = movement(vec![en()], None);
        assert!(said(&a, Level::Warning, "no mass"), "{a:?}");
        assert_eq!(a.iter().find(|a| a.message.contains("no mass")).unwrap().guide, "movement");
        for mass in [serde_json::json!(50), serde_json::json!("$mass")] {
            let a = movement(vec![en().param("mass", mass)], None);
            assert!(!said(&a, Level::Warning, "no mass"), "{a:?}");
        }
    }

    /// The movement templates follow the advice they give: nothing is told of them.
    #[test]
    fn the_movement_templates_are_told_nothing() {
        let r = registry();
        for t in crate::templates::TEMPLATES.iter().filter(|t| t.group == "Movement") {
            let a = advise(&t.model(), &r, Path::new("."), None);
            assert!(a.is_empty(), "{}: {a:?}", t.name);
        }
    }

    /// The templates show good practice: none of them gets a warning.
    #[test]
    fn the_templates_are_not_warned() {
        let r = registry();
        for t in crate::templates::TEMPLATES {
            let a = advise(&t.model(), &r, Path::new("."), None);
            assert!(a.iter().all(|a| a.level != Level::Warning), "{}: {a:?}", t.name);
        }
    }
}
