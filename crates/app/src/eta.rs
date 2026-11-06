//! The time a run has left: from how long each node took in the runs before (kept per model between sessions,
//! `run-times.json` in the user's application data), else guessed from how far the run is and how long it has gone.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The models whose times are kept, at most.
const MODELS: usize = 200;
/// A guess from the fraction alone waits for this much of the run, and this long.
const LEAST_FRACTION: f32 = 0.03;
const LEAST_SECONDS: f64 = 5.0;

/// What a run has left: seconds, and whether from the times of the runs before (else a guess from the fraction).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Left {
    pub seconds: f64,
    pub by_times: bool,
}

/// How a run goes: its top-level nodes still to run (not done, not running), the one running (how far it is, 0 to 1,
/// when it tells, and the seconds since it began), the whole run's fraction and seconds.
pub struct Going<'a> {
    pub waiting: Vec<&'a str>,
    pub running: Option<(&'a str, Option<f32>, f64)>,
    pub fraction: f32,
    pub elapsed: f64,
}

/// The time left: by the times of the runs before when every node still to run has one; else from the fraction
/// (after a few seconds and a few percent, not to guess wildly); none before.
pub fn left(g: &Going, times: &BTreeMap<String, f64>) -> Option<Left> {
    let known =
        g.waiting.iter().all(|n| times.contains_key(*n)) && g.running.is_none_or(|(n, _, _)| times.contains_key(n));
    if known {
        let mut s: f64 = g.waiting.iter().map(|n| times[*n]).sum();
        if let Some((n, part, since)) = g.running {
            let before = times[n];
            s += match part {
                // (how far it says it is: its pace now, unless it has barely begun)
                Some(f) if f >= 0.05 => since / f as f64 * (1.0 - f as f64),
                _ => (before - since).max(0.0),
            };
        }
        return Some(Left { seconds: s, by_times: true });
    }
    if g.fraction >= LEAST_FRACTION && g.elapsed >= LEAST_SECONDS && g.fraction < 1.0 {
        let f = g.fraction as f64;
        return Some(Left { seconds: g.elapsed / f * (1.0 - f), by_times: false });
    }
    None
}

/// A time left in words: "~40 s left", "~14 min left", "~2 h 10 min left"; "about" when guessed.
pub fn words(l: Left) -> String {
    let s = l.seconds.max(0.0).round() as u64;
    let t = match s {
        0..60 => format!("{} s", s.max(1)),
        60..3600 => format!("{} min", (s + 30) / 60),
        _ => {
            let m = (s + 30) / 60;
            if m.is_multiple_of(60) { format!("{} h", m / 60) } else { format!("{} h {} min", m / 60, m % 60) }
        }
    };
    if l.by_times { format!("~{t} left") } else { format!("about {t} left") }
}

fn file() -> PathBuf {
    crate::settings::data_dir().join("run-times.json")
}

/// The times kept: by model file, the seconds each node took when it last ran (its place: `node`, `group/inner`),
/// with the order the models ran in (the last at the end).
#[derive(Debug, Default, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Kept {
    pub models: Vec<(PathBuf, BTreeMap<String, f64>)>,
}

impl Kept {
    pub fn read_from(f: &Path) -> Kept {
        std::fs::read_to_string(f).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
    }

    /// A model's times.
    pub fn of(&self, model: &Path) -> BTreeMap<String, f64> {
        self.models.iter().find(|(p, _)| p == model).map(|(_, t)| t.clone()).unwrap_or_default()
    }

    /// A model's times put in (the last), the oldest models let go beyond [`MODELS`].
    pub fn put(&mut self, model: &Path, times: BTreeMap<String, f64>) {
        self.models.retain(|(p, _)| p != model);
        self.models.push((model.to_path_buf(), times));
        if self.models.len() > MODELS {
            let extra = self.models.len() - MODELS;
            self.models.drain(..extra);
        }
    }
}

/// The times of a model's nodes kept from its runs before.
pub fn times_of(model: &Path) -> BTreeMap<String, f64> {
    let model = std::path::absolute(model).unwrap_or_else(|_| model.to_path_buf());
    Kept::read_from(&file()).of(&model)
}

/// A model's times kept (in a thread: the file read, changed and written).
pub fn keep(model: &Path, times: BTreeMap<String, f64>) {
    let model = std::path::absolute(model).unwrap_or_else(|_| model.to_path_buf());
    std::thread::spawn(move || {
        let f = file();
        let mut k = Kept::read_from(&f);
        k.put(&model, times);
        if let Ok(t) = serde_json::to_string(&k) {
            let _ = std::fs::create_dir_all(f.parent().unwrap_or(Path::new(".")));
            let _ = std::fs::write(f, t);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn times(t: &[(&str, f64)]) -> BTreeMap<String, f64> {
        t.iter().map(|(n, s)| (n.to_string(), *s)).collect()
    }

    #[test]
    fn the_time_left_by_the_runs_before() {
        let t = times(&[("read", 2.0), ("calc", 10.0), ("loop", 60.0)]);
        // the loop half way (its steps), the calc still to run
        let g = Going { waiting: vec!["calc"], running: Some(("loop", Some(0.5), 30.0)), fraction: 0.4, elapsed: 32.0 };
        assert_eq!(left(&g, &t), Some(Left { seconds: 40.0, by_times: true }));
        // a node that does not tell how far it is: what it took before, less the time it has gone
        let g = Going { waiting: vec![], running: Some(("loop", None, 45.0)), fraction: 0.9, elapsed: 47.0 };
        assert_eq!(left(&g, &t).unwrap().seconds, 15.0);
        // longer than before: none left, not less than none
        let g = Going { waiting: vec![], running: Some(("loop", None, 90.0)), fraction: 0.9, elapsed: 92.0 };
        assert_eq!(left(&g, &t).unwrap().seconds, 0.0);
    }

    #[test]
    fn without_times_a_guess_from_the_fraction_after_a_while() {
        let t = times(&[("read", 2.0)]);
        let g = Going { waiting: vec!["new"], running: None, fraction: 0.25, elapsed: 30.0 };
        assert_eq!(left(&g, &t), Some(Left { seconds: 90.0, by_times: false }));
        // too early to tell
        let g = Going { waiting: vec!["new"], running: None, fraction: 0.25, elapsed: 2.0 };
        assert_eq!(left(&g, &t), None);
        let g = Going { waiting: vec!["new"], running: None, fraction: 0.01, elapsed: 60.0 };
        assert_eq!(left(&g, &t), None);
    }

    #[test]
    fn in_words() {
        assert_eq!(words(Left { seconds: 0.2, by_times: true }), "~1 s left");
        assert_eq!(words(Left { seconds: 40.0, by_times: true }), "~40 s left");
        assert_eq!(words(Left { seconds: 14.0 * 60.0 + 10.0, by_times: false }), "about 14 min left");
        assert_eq!(words(Left { seconds: 7800.0, by_times: true }), "~2 h 10 min left");
        assert_eq!(words(Left { seconds: 7200.0, by_times: true }), "~2 h left");
    }

    #[test]
    fn the_times_kept_per_model() {
        let mut k = Kept::default();
        k.put(Path::new("/a.toml"), times(&[("x", 1.0)]));
        k.put(Path::new("/b.toml"), times(&[("y", 2.0)]));
        k.put(Path::new("/a.toml"), times(&[("x", 3.0)]));
        assert_eq!(k.of(Path::new("/a.toml")), times(&[("x", 3.0)]));
        assert_eq!(k.models.last().unwrap().0, PathBuf::from("/a.toml"), "the last run at the end");
        for n in 0..MODELS + 5 {
            k.put(&PathBuf::from(format!("/m{n}.toml")), BTreeMap::new());
        }
        assert_eq!(k.models.len(), MODELS);
        assert!(k.of(Path::new("/b.toml")).is_empty(), "the oldest let go");
    }
}
