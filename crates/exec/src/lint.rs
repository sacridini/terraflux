//! Checking a model's data before it runs: the problems a modeller should hear about in plain words, each
//! with what to do about it.
//!
//! - every map on the same grid (size, cells, coordinate reference system);
//! - land use maps of whole classes, not too many of them, and some change between the two dates;
//! - drivers with values, not constant, not missing where the land use has a class, not collinear.

use std::collections::{BTreeMap, BTreeSet};

use terraflux_graph::{Model, Node, Source};
use terraflux_ops::{Map, Value};
use terraflux_raster::Grid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Info,
    Warning,
    Error,
}

/// What the application can do about a finding.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Resample the input to the grid of the reference input.
    Resample { input: String, reference: String },
}

/// A problem in the data.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub level: Level,
    /// The input (`$name`, `$name/band`) it is about.
    pub at: String,
    pub message: String,
    /// What to do about it.
    pub fix: Option<String>,
    pub action: Option<Action>,
}

impl std::fmt::Display for Finding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let l = match self.level {
            Level::Info => "note",
            Level::Warning => "warning",
            Level::Error => "error",
        };
        write!(f, "{l}: {}: {}", self.at, self.message)?;
        if let Some(fix) = &self.fix {
            write!(f, " ({fix})")?;
        }
        Ok(())
    }
}

/// Ports that take land use maps (categorical), and those that take drivers.
const CATEGORICAL: [&str; 7] = ["t0", "t1", "landscape", "start", "maps", "observed", "simulated"];
const DRIVERS: [&str; 1] = ["drivers"];
/// What hands a map or a stack on with its classes read as they were: a grid put right, classes translated or
/// named, the maps of dates stacked. An input read by one of these feeds the ports its result feeds.
const PASS: [&str; 4] = ["raster.match", "raster.reclassify", "raster.legend", "stack.of"];

fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
    for n in nodes {
        out.push(n);
        if let Some(b) = &n.body {
            walk(&b.nodes, out);
        }
    }
}

/// The ports node `id` feeds: those that read it, and (through the nodes of [`PASS`] that read it) theirs.
fn fed(all: &[&Node], id: &str, seen: &mut BTreeSet<String>, out: &mut BTreeSet<String>) {
    if !seen.insert(id.to_string()) {
        return;
    }
    let prefix = format!("{id}.");
    for n in all {
        for (port, src) in &n.inputs {
            if let Source::Ref(r) = src
                && r.starts_with(&prefix)
            {
                out.insert(port.clone());
                if PASS.contains(&n.op.as_str()) {
                    fed(all, &n.id, seen, out);
                }
            }
        }
    }
}

/// The ports each input feeds, by input name: where a node reads it, and where the nodes of [`PASS`] that read it
/// give what they make of it (an input read by a `raster.reclassify` feeds what the reclassified map feeds).
fn uses(model: &Model) -> BTreeMap<String, BTreeSet<String>> {
    let mut all = Vec::new();
    walk(&model.nodes, &mut all);
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in &all {
        for (port, src) in &n.inputs {
            if let Source::Ref(r) = src
                && let Some(name) = r.strip_prefix('$')
            {
                let ports = out.entry(name.to_string()).or_default();
                ports.insert(port.clone());
                if PASS.contains(&n.op.as_str()) {
                    fed(&all, &n.id, &mut BTreeSet::new(), ports);
                }
            }
        }
    }
    out
}

/// The input that plays a part in the model (`t0`, `t1`, `start`): the one of that name, else the first, by name,
/// that feeds a port of that name; only among the inputs `fit` takes. The parts are tried in order.
fn role<'a>(
    used: &BTreeMap<String, BTreeSet<String>>,
    inputs: &'a BTreeMap<String, Value>,
    parts: &[&str],
    fit: impl Fn(&Value) -> bool,
) -> Option<&'a String> {
    parts.iter().find_map(|part| {
        inputs
            .iter()
            .filter(|(_, v)| fit(v))
            .find(|(k, _)| k.as_str() == *part || used.get(*k).is_some_and(|p| p.contains(*part)))
            .map(|(k, _)| k)
    })
}

fn grids(v: &Value) -> Option<&Grid> {
    match v {
        Value::Map(m) => Some(&m.grid),
        Value::Stack(s) => Some(&s.grid),
        _ => None,
    }
}

fn pearson(a: &[f64], b: &[f64]) -> Option<f64> {
    let (mut n, mut sa, mut sb) = (0.0, 0.0, 0.0);
    for (&x, &y) in a.iter().zip(b) {
        if x.is_finite() && y.is_finite() {
            n += 1.0;
            sa += x;
            sb += y;
        }
    }
    if n < 3.0 {
        return None;
    }
    let (ma, mb) = (sa / n, sb / n);
    let (mut cov, mut va, mut vb) = (0.0, 0.0, 0.0);
    for (&x, &y) in a.iter().zip(b) {
        if x.is_finite() && y.is_finite() {
            cov += (x - ma) * (y - mb);
            va += (x - ma) * (x - ma);
            vb += (y - mb) * (y - mb);
        }
    }
    if va == 0.0 || vb == 0.0 { None } else { Some(cov / (va * vb).sqrt()) }
}

fn share(n: usize, of: usize) -> String {
    format!("{:.1}%", 100.0 * n as f64 / of.max(1) as f64)
}

/// The problems of a model's data.
pub fn lint(model: &Model, inputs: &BTreeMap<String, Value>) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut push = |level, at: &str, message: String, fix: Option<&str>| {
        out.push(Finding { level, at: at.to_string(), message, fix: fix.map(String::from), action: None });
    };
    let used = uses(model);
    // inputs a raster.match puts on another grid (their own grid does not matter), and those it puts them on
    let mut all = Vec::new();
    walk(&model.nodes, &mut all);
    let (mut matched, mut likes) = (BTreeSet::new(), Vec::new());
    for n in all.iter().filter(|n| n.op == "raster.match") {
        if let Some(Source::Ref(r)) = n.inputs.get("raster")
            && let Some(name) = r.strip_prefix('$')
        {
            matched.insert(name.to_string());
        }
        if let Some(Source::Ref(r)) = n.inputs.get("like")
            && let Some(name) = r.strip_prefix('$')
        {
            likes.push(name.to_string());
        }
    }
    // the reference grid: the start's, else the one the others are matched to, else the first map's
    let reference = role(&used, inputs, &["start", "t0"], |v| grids(v).is_some())
        .filter(|k| !matched.contains(*k))
        .cloned()
        .or_else(|| likes.into_iter().find(|k| inputs.get(k).is_some_and(|v| grids(v).is_some())))
        .or_else(|| inputs.iter().find(|(k, v)| grids(v).is_some() && !matched.contains(*k)).map(|(k, _)| k.clone()))
        .or_else(|| inputs.iter().find(|(_, v)| grids(v).is_some()).map(|(k, _)| k.clone()));
    if let Some(r) = &reference {
        let g0 = grids(&inputs[r]).unwrap();
        for (name, v) in inputs {
            if matched.contains(name) && name != r {
                continue;
            }
            let Some(g) = grids(v) else { continue };
            let at = format!("${name}");
            if g.width != g0.width || g.height != g0.height {
                push(
                    Level::Error,
                    &at,
                    format!(
                        "{} x {} cells, the reference grid (${r}) has {} x {}",
                        g.width, g.height, g0.width, g0.height
                    ),
                    Some("resample it to the reference grid"),
                );
            } else if g.transform != g0.transform {
                push(
                    Level::Error,
                    &at,
                    format!("not aligned with the reference grid (${r}): its cells are elsewhere or of another size"),
                    Some("resample it to the reference grid"),
                );
            }
            if g.geo.geokeys.is_some() && g0.geo.geokeys.is_some() && g.geo.epsg() != g0.geo.epsg() {
                push(
                    Level::Error,
                    &at,
                    format!(
                        "another coordinate reference system ({:?}) than the reference grid's ({:?})",
                        g.geo.epsg(),
                        g0.geo.epsg()
                    ),
                    Some("reproject it to the reference grid"),
                );
            }
        }
    }
    // land use maps
    let mut categorical: Vec<(&String, &Map)> = Vec::new();
    for (name, v) in inputs {
        let ports = used.get(name).cloned().unwrap_or_default();
        let at = format!("${name}");
        if !ports.iter().any(|p| CATEGORICAL.contains(&p.as_str())) {
            continue;
        }
        let maps: Vec<Map> = match v {
            Value::Map(m) => vec![m.clone()],
            Value::Stack(s) => s.layers.iter().map(|l| Map { grid: s.grid.clone(), data: l.clone() }).collect(),
            _ => continue,
        };
        for m in &maps {
            let valid = m.data.iter().filter(|v| !v.is_nan()).count();
            if valid == 0 {
                push(Level::Error, &at, "no cell has a class".into(), Some("check its no-data value"));
                continue;
            }
            if m.data.iter().any(|v| !v.is_nan() && v.fract() != 0.0) {
                push(
                    Level::Error,
                    &at,
                    "holds fractional values: a land use map holds whole class codes".into(),
                    Some("reclassify it, or give it as a driver if it is a continuous variable"),
                );
                continue;
            }
            let k = m.classes().len();
            if k > 100 {
                push(
                    Level::Warning,
                    &at,
                    format!("{k} classes: is it a continuous variable?"),
                    Some("merge classes, or give it as a driver"),
                );
            }
            let missing = m.data.len() - valid;
            if missing * 2 > m.data.len() {
                push(Level::Info, &at, format!("{} of the cells have no class", share(missing, m.data.len())), None);
            }
        }
        if let Value::Map(m) = v {
            categorical.push((name, m));
        }
    }
    // two dates: some change between them, and the same cells with a class
    let is_map = |v: &Value| matches!(v, Value::Map(_));
    let (first, second) = (role(&used, inputs, &["t0"], is_map), role(&used, inputs, &["t1"], is_map));
    if let (Some(a_name), Some(b_name)) = (first, second)
        && a_name != b_name
        && let (Value::Map(a), Value::Map(b)) = (&inputs[a_name], &inputs[b_name])
        && a.data.len() == b.data.len()
    {
        let changed = a.data.iter().zip(b.data.iter()).filter(|(x, y)| !x.is_nan() && !y.is_nan() && x != y).count();
        if changed == 0 {
            push(
                Level::Error,
                &format!("${b_name}"),
                format!("no cell changed class since ${a_name}: there is no change to calibrate on"),
                Some("check the dates of the two maps"),
            );
        }
        let only = a.data.iter().zip(b.data.iter()).filter(|(x, y)| x.is_nan() != y.is_nan()).count();
        if only > 0 {
            push(
                Level::Warning,
                &format!("${b_name}"),
                format!("{only} cells have a class in only one of the two maps: they are left out"),
                Some("give the maps the same extent and no-data cells"),
            );
        }
    }
    // drivers
    let start = role(&used, inputs, &["start", "t0", "landscape"], is_map)
        .and_then(|n| categorical.iter().find(|(k, _)| *k == n))
        .map(|(_, m)| *m);
    for (name, v) in inputs {
        let ports = used.get(name).cloned().unwrap_or_default();
        if !ports.iter().any(|p| DRIVERS.contains(&p.as_str())) {
            continue;
        }
        let Value::Stack(s) = v else { continue };
        let names: Vec<String> = s.keys.iter().map(|k| k.to_string()).collect();
        for (k, l) in s.layers.iter().enumerate() {
            let at = format!("${name}/{}", names[k]);
            let finite: Vec<f64> = l.iter().copied().filter(|v| v.is_finite()).collect();
            if finite.is_empty() {
                push(Level::Error, &at, "has no value".into(), Some("check its no-data value, or drop it"));
                continue;
            }
            if finite.iter().all(|&v| v == finite[0]) {
                push(Level::Warning, &at, "is constant: it explains nothing".into(), Some("drop it"));
            }
            if let Some(st) = start
                && st.data.len() == l.len()
            {
                let lost = st.data.iter().zip(l.iter()).filter(|(c, v)| !c.is_nan() && v.is_nan()).count();
                let with = st.data.iter().filter(|c| !c.is_nan()).count();
                if lost * 20 > with {
                    push(
                        Level::Warning,
                        &at,
                        format!(
                            "missing at {} of the cells with a class: they take no part in the fits",
                            share(lost, with)
                        ),
                        Some("fill the gaps, or reduce the study area"),
                    );
                }
            }
        }
        for i in 0..s.layers.len() {
            for j in i + 1..s.layers.len() {
                if let Some(r) = pearson(&s.layers[i], &s.layers[j])
                    && r.abs() > 0.9
                {
                    push(
                        Level::Warning,
                        &format!("${name}/{}", names[i]),
                        format!("collinear with {} (r = {r:.2}): the two say almost the same", names[j]),
                        Some("keep one of them"),
                    );
                }
            }
        }
    }
    // the grid mismatches can be resampled (same coordinate reference system)
    if let Some(r) = &reference {
        for f in out.iter_mut() {
            if f.level == Level::Error && f.fix.as_deref() == Some("resample it to the reference grid") {
                f.action =
                    Some(Action::Resample { input: f.at.trim_start_matches('$').to_string(), reference: r.clone() });
            }
        }
    }
    out.sort_by_key(|f| std::cmp::Reverse(f.level));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use terraflux_ops::{Key, Stack};

    fn model() -> Model {
        let mut m = Model::new("t");
        m.nodes.push(Node::new("a", "markov.fit").input("t0", "$t0").input("t1", "$t1").literal("steps", 1));
        m.nodes.push(Node::new("b", "woe.fit").input("t0", "$t0").input("t1", "$t1").input("drivers", "$drivers"));
        m
    }

    fn map(g: &Arc<Grid>, v: Vec<f64>) -> Value {
        Value::Map(Map::new(g.clone(), v))
    }

    #[test]
    fn finds_what_is_wrong() {
        let g = Arc::new(Grid::new(4, 1));
        let mut other = Grid::new(4, 1);
        other.transform[0] = 10.0;
        let mut inputs = BTreeMap::new();
        inputs.insert("t0".to_string(), map(&g, vec![1.0, 1.0, 2.0, f64::NAN]));
        inputs.insert("t1".to_string(), map(&Arc::new(other), vec![1.0, 1.5, 2.0, 2.0]));
        let x = vec![1.0, 2.0, 3.0, 4.0];
        inputs.insert(
            "drivers".to_string(),
            Value::Stack(Stack {
                grid: g.clone(),
                keys: vec![Key::Name("a".into()), Key::Name("b".into()), Key::Name("c".into())],
                layers: vec![
                    Arc::new(x.clone()),
                    Arc::new(x.iter().map(|v| v * 2.0 + 1.0).collect()),
                    Arc::new(vec![5.0; 4]),
                ],
            }),
        );
        let f = lint(&model(), &inputs);
        let text: Vec<String> = f.iter().map(|x| x.to_string()).collect();
        let has = |s: &str| text.iter().any(|t| t.contains(s));
        assert!(has("not aligned"), "{text:?}");
        assert!(has("fractional"), "{text:?}");
        assert!(has("collinear with b"), "{text:?}");
        assert!(has("constant"), "{text:?}");
        assert!(has("only one of the two maps"), "{text:?}");
        assert_eq!(f[0].level, Level::Error);
    }

    #[test]
    fn good_data_is_quiet() {
        let g = Arc::new(Grid::new(4, 1));
        let mut inputs = BTreeMap::new();
        inputs.insert("t0".to_string(), map(&g, vec![1.0, 1.0, 2.0, 2.0]));
        inputs.insert("t1".to_string(), map(&g, vec![1.0, 2.0, 2.0, 2.0]));
        inputs.insert(
            "drivers".to_string(),
            Value::Stack(Stack {
                grid: g.clone(),
                keys: vec![Key::Name("a".into()), Key::Name("b".into())],
                layers: vec![Arc::new(vec![1.0, 2.0, 3.0, 4.0]), Arc::new(vec![4.0, 1.0, 3.0, 2.0])],
            }),
        );
        assert!(lint(&model(), &inputs).is_empty());
    }

    #[test]
    fn an_input_put_on_the_grid_by_raster_match_is_not_flagged() {
        let g = Arc::new(Grid::new(4, 1));
        let mut other = Grid::new(2, 1);
        other.transform[0] = 10.0;
        // the matched input sorts before the one it is matched to: it must not become the reference
        let mut m = Model::new("t");
        m.nodes.push(Node::new("a_match", "raster.match").input("raster", "$a_height").input("like", "$z_image"));
        m.nodes
            .push(Node::new("seg", "segment.multiresolution").input("image", "$z_image").input("h", "a_match.raster"));
        let mut inputs = BTreeMap::new();
        inputs.insert("a_height".to_string(), map(&Arc::new(other), vec![1.0, 2.0]));
        inputs.insert("z_image".to_string(), map(&g, vec![1.0, 2.0, 3.0, 4.0]));
        assert!(lint(&m, &inputs).iter().all(|f| f.level != Level::Error), "{:?}", lint(&m, &inputs));
        // without the match the mismatch is an error
        let mut m2 = Model::new("t");
        m2.nodes.push(Node::new("seg", "segment.multiresolution").input("image", "$z_image").input("h", "$a_height"));
        assert!(lint(&m2, &inputs).iter().any(|f| f.level == Level::Error));
    }

    /// The model of the presets' names (`t0`, `t1`...), and the same one with other names and a chain between the inputs and what
    /// reads them (a grid put right, classes translated, the maps stacked), as "Use my data" writes it.
    fn chained() -> Model {
        let mut m = Model::new("t");
        m.nodes.push(Node::new("a_match", "raster.match").input("raster", "$before").input("like", "$before"));
        m.nodes.push(Node::new("a_classes", "raster.reclassify").input("map", "a_match.raster"));
        m.nodes.push(Node::new("b_classes", "raster.reclassify").input("map", "$after"));
        m.nodes.push(Node::new("a", "markov.fit").input("t0", "a_classes.map").input("t1", "b_classes.map"));
        m.nodes.push(Node::new("obs", "stack.of").input("x", "a_classes.map").input("y", "b_classes.map"));
        m.nodes.push(Node::new("d", "demand.from_maps").input("maps", "obs.stack"));
        m.nodes.push(Node::new("w", "woe.fit").input("drivers", "$layers"));
        m
    }

    #[test]
    fn the_roles_follow_the_ports_an_input_feeds_not_its_name() {
        let used = uses(&chained());
        let ports = |n: &str| used[n].iter().map(String::as_str).collect::<Vec<_>>();
        assert_eq!(ports("before"), ["like", "map", "maps", "raster", "t0", "x"]);
        assert_eq!(ports("after"), ["map", "maps", "t1", "y"]);
        assert_eq!(ports("layers"), ["drivers"]);
        let g = Arc::new(Grid::new(2, 1));
        let both = |a: Vec<f64>, b: Vec<f64>| {
            let named =
                BTreeMap::from([("t0".to_string(), map(&g, a.clone())), ("t1".to_string(), map(&g, b.clone()))]);
            let other = BTreeMap::from([("before".to_string(), map(&g, a)), ("after".to_string(), map(&g, b))]);
            let text = |f: Vec<Finding>| {
                f.into_iter().map(|f| (f.level, f.message.replace("$before", "$t0"))).collect::<Vec<_>>()
            };
            (text(lint(&model(), &named)), text(lint(&chained(), &other)))
        };
        // no change: the same finding, whatever the names
        let (plain, mine) = both(vec![1.0, 2.0], vec![1.0, 2.0]);
        assert!(plain.iter().any(|f| f.1.contains("no cell changed")), "{plain:?}");
        assert_eq!(plain, mine);
        // cells with a class in one of the maps only
        let (plain, mine) = both(vec![1.0, f64::NAN], vec![2.0, 2.0]);
        assert!(plain.iter().any(|f| f.1.contains("only one of the two maps")), "{plain:?}");
        assert_eq!(plain, mine);
        let f = lint(
            &chained(),
            &BTreeMap::from([
                ("before".to_string(), map(&g, vec![1.0, 1.0])),
                ("after".to_string(), map(&g, vec![1.0, 1.0])),
            ]),
        );
        assert!(
            f.iter().any(|f| f.at == "$after"
                && f.message == "no cell changed class since $before: there is no change to calibrate on"),
            "{f:?}"
        );
        // fractional values are found in a map that only reaches the model through the chain
        let f = lint(
            &chained(),
            &BTreeMap::from([
                ("before".to_string(), map(&g, vec![1.5, 1.0])),
                ("after".to_string(), map(&g, vec![1.0, 2.0])),
            ]),
        );
        assert!(f.iter().any(|f| f.at == "$before" && f.message.contains("fractional")), "{f:?}");
    }

    #[test]
    fn no_change_is_an_error() {
        let g = Arc::new(Grid::new(2, 1));
        let mut inputs = BTreeMap::new();
        inputs.insert("t0".to_string(), map(&g, vec![1.0, 2.0]));
        inputs.insert("t1".to_string(), map(&g, vec![1.0, 2.0]));
        assert!(
            lint(&model(), &inputs).iter().any(|f| f.level == Level::Error && f.message.contains("no cell changed"))
        );
    }
}
