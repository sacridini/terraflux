//! The tasks of a batch: replicates (one seed each), scenarios (named sets of overrides), and the candidates
//! of a calibration (a grid or random draws of settings, each run with the same seeds).

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::client::TaskOf;
use crate::evaluate::Metrics;
use crate::{Error, Result};

/// `n` runs with the seeds `first`, `first + 1`... (the input `seed`).
pub fn replicates(n: usize, first: i64, seed_input: &str) -> Vec<TaskOf> {
    (0..n)
        .map(|k| {
            let seed = first + k as i64;
            (format!("seed_{seed}"), [(format!("${seed_input}"), Value::from(seed))].into())
        })
        .collect()
}

#[derive(Deserialize)]
struct ScenarioFile {
    scenario: Vec<Scenario>,
}

#[derive(Deserialize)]
struct Scenario {
    name: String,
    #[serde(default)]
    set: BTreeMap<String, Value>,
}

/// Scenarios from TOML: `[[scenario]] name = "..." set = { "$seed" = 2, "patches.isometry" = 1.2 }`.
pub fn scenarios(toml_text: &str) -> Result<Vec<TaskOf>> {
    let f: ScenarioFile = toml::from_str(toml_text).map_err(|e| Error::Other(format!("scenarios: {e}")))?;
    let mut seen = std::collections::BTreeSet::new();
    f.scenario
        .into_iter()
        .map(|s| {
            if !seen.insert(s.name.clone()) {
                return Err(Error::Other(format!("two scenarios are named {:?}", s.name)));
            }
            Ok((s.name, s.set))
        })
        .collect()
}

/// A setting a calibration varies, with its values (a grid) or range (random draws).
#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub key: String,
    pub from: f64,
    pub to: f64,
    /// Values of a grid (from..=to); 1 takes the middle.
    pub points: usize,
    /// Whole numbers only.
    pub integer: bool,
}

impl Param {
    /// `key=from:to` (`key=from:to:points`), or `key=value`.
    pub fn parse(s: &str, points: usize) -> Result<Param> {
        let (key, range) = s.split_once('=').ok_or_else(|| Error::Other(format!("{s:?}: give key=from:to")))?;
        let parts: Vec<&str> = range.split(':').collect();
        let num = |t: &str| t.trim().parse::<f64>().map_err(|_| Error::Other(format!("{t:?} is not a number")));
        let (from, to, n) = match parts.as_slice() {
            [v] => (num(v)?, num(v)?, 1),
            [a, b] => (num(a)?, num(b)?, points),
            [a, b, n] => (num(a)?, num(b)?, n.trim().parse().map_err(|_| Error::Other(format!("{n:?} points")))?),
            _ => return Err(Error::Other(format!("{s:?}: give key=from:to"))),
        };
        let integer = parts.iter().all(|p| !p.contains('.'));
        Ok(Param { key: key.trim().to_string(), from, to, points: n.max(1), integer })
    }

    pub fn values(&self) -> Vec<f64> {
        if self.points == 1 {
            return vec![self.round((self.from + self.to) / 2.0)];
        }
        let mut v: Vec<f64> = (0..self.points)
            .map(|i| self.round(self.from + (self.to - self.from) * i as f64 / (self.points - 1) as f64))
            .collect();
        v.dedup();
        v
    }

    fn round(&self, x: f64) -> f64 {
        if self.integer { x.round() } else { x }
    }

    fn json(&self, x: f64) -> Value {
        if self.integer { Value::from(x as i64) } else { Value::from(x) }
    }
}

/// Every combination of the parameters' values.
pub fn grid(params: &[Param]) -> Vec<BTreeMap<String, Value>> {
    let mut out = vec![BTreeMap::new()];
    for p in params {
        let mut next = Vec::new();
        for c in &out {
            for v in p.values() {
                let mut c = c.clone();
                c.insert(p.key.clone(), p.json(v));
                next.push(c);
            }
        }
        out = next;
    }
    out
}

/// `n` draws in the parameters' ranges, a Latin hypercube (each range cut in `n` strata, each stratum drawn
/// once), reproducible with the seed.
pub fn random(params: &[Param], n: usize, seed: u64) -> Vec<BTreeMap<String, Value>> {
    let mut state = seed.wrapping_add(0x9E3779B97F4A7C15);
    let mut next = || {
        // SplitMix64
        state = state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    };
    let mut out = vec![BTreeMap::new(); n];
    for p in params {
        let mut strata: Vec<usize> = (0..n).collect();
        for i in (1..n).rev() {
            let j = (next() * (i + 1) as f64) as usize;
            strata.swap(i, j.min(i));
        }
        for (k, c) in out.iter_mut().enumerate() {
            let u = (strata[k] as f64 + next()) / n as f64;
            c.insert(p.key.clone(), p.json(p.round(p.from + (p.to - p.from) * u)));
        }
    }
    out
}

/// The tasks of a calibration: each candidate run with each seed.
pub fn calibration_tasks(candidates: &[BTreeMap<String, Value>], seeds: &[i64], seed_input: &str) -> Vec<TaskOf> {
    let mut out = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        for &s in seeds {
            let mut o = c.clone();
            o.insert(format!("${seed_input}"), Value::from(s));
            out.push((format!("candidate_{i}_seed_{s}"), o));
        }
    }
    out
}

/// A candidate's mean metric over its seeds.
#[derive(Debug, Clone, PartialEq)]
pub struct Scored {
    pub candidate: usize,
    pub settings: BTreeMap<String, Value>,
    pub score: f64,
    pub runs: usize,
}

/// The candidates ranked by the mean of a metric over their seeds (the largest first; NaN last).
pub fn rank(
    candidates: &[BTreeMap<String, Value>],
    seeds: usize,
    metrics: &[Option<Metrics>],
    metric: &str,
) -> Result<Vec<Scored>> {
    if metrics.len() != candidates.len() * seeds {
        return Err(Error::Other("one result per candidate and seed".into()));
    }
    let mut out = Vec::new();
    for (i, c) in candidates.iter().enumerate() {
        let vals: Vec<f64> = metrics[i * seeds..(i + 1) * seeds]
            .iter()
            .flatten()
            .map(|m| m.get(metric).ok_or_else(|| Error::Other(format!("unknown metric {metric:?}"))))
            .collect::<Result<_>>()?;
        let score = if vals.is_empty() { f64::NAN } else { vals.iter().sum::<f64>() / vals.len() as f64 };
        out.push(Scored { candidate: i, settings: c.clone(), score, runs: vals.len() });
    }
    out.sort_by(|a, b| match (a.score.is_nan(), b.score.is_nan()) {
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        _ => b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal),
    });
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn params_and_grids() {
        let p = Param::parse("patches.isometry=0.5:2.5", 5).unwrap();
        assert_eq!(p.values(), vec![0.5, 1.0, 1.5, 2.0, 2.5]);
        let q = Param::parse("allocate.window=3:7:3", 9).unwrap();
        assert!(q.integer);
        assert_eq!(q.values(), vec![3.0, 5.0, 7.0]);
        let g = grid(&[p, q]);
        assert_eq!(g.len(), 15);
        assert_eq!(g[0]["allocate.window"], Value::from(3));
        assert!(Param::parse("x", 3).is_err());
    }

    #[test]
    fn random_draws_cover_the_ranges() {
        let p = Param::parse("a=0.0:1.0", 1).unwrap();
        let d = random(&[p], 10, 7);
        let mut v: Vec<f64> = d.iter().map(|c| c["a"].as_f64().unwrap()).collect();
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        for (k, x) in v.iter().enumerate() {
            assert!(*x >= k as f64 / 10.0 && *x < (k + 1) as f64 / 10.0, "one draw per stratum");
        }
        assert_eq!(random(&[Param::parse("a=0.0:1.0", 1).unwrap()], 10, 7), d, "reproducible");
    }

    #[test]
    fn scenarios_and_replicates() {
        let s = scenarios("[[scenario]]\nname = \"low\"\nset = { \"$seed\" = 2 }\n[[scenario]]\nname = \"high\"\n")
            .unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].1["$seed"], Value::from(2));
        assert!(scenarios("[[scenario]]\nname = \"a\"\n[[scenario]]\nname = \"a\"\n").is_err());
        let r = replicates(3, 10, "seed");
        assert_eq!(r[2].0, "seed_12");
    }
}
