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
const MEASURES: [&str; 8] =
    ["metrics.", "fragmentation.", "landscape.", "distance.", "cost.", "conn.", "circuit.", "restoration."];

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
        if p.starts_with('$') || p.starts_with('@') {
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

    out.sort_by_key(|a| std::cmp::Reverse(a.level));
    out
}

/// The operators that solve a prioritization (their solution worth evaluating).
const PRIORITIZATIONS: [&str; 4] =
    ["prioritize.optimize", "prioritize.marxan", "prioritize.actions", "prioritize.actions_raster"];

/// The operators that clean or thin the records of a species.
const THINNING: [&str; 4] = ["sdm.thin", "sdm.one_per_cell", "sdm.prepare", "sdm.clean_occurrences"];

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
fn year_in(s: &str) -> Option<i64> {
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
