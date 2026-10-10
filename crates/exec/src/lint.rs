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

fn walk<'a>(nodes: &'a [Node], out: &mut Vec<&'a Node>) {
    for n in nodes {
        out.push(n);
        if let Some(b) = &n.body {
            walk(&b.nodes, out);
        }
    }
}

/// The ports each input feeds, by input name.
fn uses(model: &Model) -> BTreeMap<String, BTreeSet<String>> {
    let mut all = Vec::new();
    walk(&model.nodes, &mut all);
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for n in all {
        for (port, src) in &n.inputs {
            if let Source::Ref(r) = src
                && let Some(name) = r.strip_prefix('$')
            {
                out.entry(name.to_string()).or_default().insert(port.clone());
            }
        }
    }
    out
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
    // the reference grid: the start's, else the first map's
    let reference = ["start", "t0"]
        .iter()
        .find(|n| inputs.get(**n).and_then(grids).is_some())
        .map(|n| n.to_string())
        .or_else(|| inputs.iter().find(|(_, v)| grids(v).is_some()).map(|(k, _)| k.clone()));
    if let Some(r) = &reference {
        let g0 = grids(&inputs[r]).unwrap();
        for (name, v) in inputs {
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
    if let (Some(a), Some(b)) = (inputs.get("t0"), inputs.get("t1"))
        && let (Value::Map(a), Value::Map(b)) = (a, b)
        && a.data.len() == b.data.len()
    {
        let changed = a.data.iter().zip(b.data.iter()).filter(|(x, y)| !x.is_nan() && !y.is_nan() && x != y).count();
        if changed == 0 {
            push(
                Level::Error,
                "$t1",
                "no cell changed class since $t0: there is no change to calibrate on".into(),
                Some("check the dates of the two maps"),
            );
        }
        let only = a.data.iter().zip(b.data.iter()).filter(|(x, y)| x.is_nan() != y.is_nan()).count();
        if only > 0 {
            push(
                Level::Warning,
                "$t1",
                format!("{only} cells have a class in only one of the two maps: they are left out"),
                Some("give the maps the same extent and no-data cells"),
            );
        }
    }
    // drivers
    let start = categorical.iter().find(|(n, _)| n.as_str() == "start" || n.as_str() == "t0").map(|(_, m)| *m);
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
