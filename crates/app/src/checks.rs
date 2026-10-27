//! The model's checks (`check.assert`) and how they fared in the last run: a badge on each one's node (green when
//! it passed, amber when it only warned, red when it failed and stopped the run), on the containers that hold them
//! (their worst), on its step of the document, and the model's health in a line (so many passed, warned, failed,
//! not run, turned off).

use std::collections::HashMap;

use terraflux_graph::Node;
use terraflux_ops::Value;

use crate::app::Session;
use crate::editor_view::NodeStatus;

/// How a check fared in the last run.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Passed,
    /// It failed, only warning (`warn`): what it said.
    Warned(String),
    /// It failed and stopped the run: what it said.
    Failed(String),
    /// Not run yet (or the run stopped before it).
    NotRun,
    /// Turned off (bypass).
    Off,
}

impl Verdict {
    /// The worse of two: failed, warned, not run, passed.
    fn rank(&self) -> u8 {
        match self {
            Verdict::Passed => 0,
            Verdict::NotRun | Verdict::Off => 1,
            Verdict::Warned(_) => 2,
            Verdict::Failed(_) => 3,
        }
    }

    pub fn symbol(&self) -> &'static str {
        match self {
            Verdict::Passed => "✔",
            Verdict::Warned(_) => "!",
            Verdict::Failed(_) => "✖",
            Verdict::NotRun | Verdict::Off => "·",
        }
    }

    /// What it says, in a line.
    pub fn words(&self) -> String {
        match self {
            Verdict::Passed => "passed".into(),
            Verdict::Warned(m) => format!("failed, warning only: {m}"),
            Verdict::Failed(m) => format!("failed, the run stopped: {m}"),
            Verdict::NotRun => "not run yet".into(),
            Verdict::Off => "turned off".into(),
        }
    }

    pub fn colour(&self, v: &egui::Visuals) -> egui::Color32 {
        match self {
            Verdict::Passed => egui::Color32::from_rgb(90, 190, 90),
            Verdict::Warned(_) => v.warn_fg_color,
            Verdict::Failed(_) => v.error_fg_color,
            Verdict::NotRun | Verdict::Off => v.weak_text_color(),
        }
    }
}

/// A check of the model: where it is, what must hold, how it fared.
#[derive(Debug, Clone)]
pub struct Check {
    pub path: Vec<String>,
    pub id: String,
    pub expression: String,
    pub verdict: Verdict,
}

/// What the last run said: the node that failed (by its path) and its message, the warnings.
struct Run<'a> {
    error: Option<(String, String)>,
    warnings: &'a [String],
}

/// What a check said, its node's name before it left out (`node "x": the demand is met (...)`).
fn said(message: &str, id: &str) -> String {
    message.trim_start_matches(&format!("node {id:?}: ")).to_string()
}

fn verdict(run: &Run, path: &[String], n: &Node, ok: Option<bool>, ran: bool) -> Verdict {
    let at = path.iter().chain(std::iter::once(&n.id)).cloned().collect::<Vec<_>>().join("/");
    if let Some((failed, message)) = &run.error
        && *failed == at
    {
        return Verdict::Failed(said(message, &n.id));
    }
    if n.bypass {
        return Verdict::Off;
    }
    // (a warning is `node: what it said`; inside a container, the container's)
    let warned = run.warnings.iter().find_map(|w| {
        let (from, text) = w.split_once(": ")?;
        (from == at || from == n.id || (!path.is_empty() && from == path[0]))
            .then(|| said(text, &n.id))
            .filter(|t| t.contains(n.params.get("expression").and_then(|e| e.as_str()).unwrap_or("\u{0}")))
    });
    match (ok, warned) {
        (Some(false), w) | (None, w @ Some(_)) if ran => Verdict::Warned(w.unwrap_or_else(|| "it failed".into())),
        (Some(true), _) => Verdict::Passed,
        (None, None) if ran => Verdict::Passed,
        _ => Verdict::NotRun,
    }
}

/// The model's checks, everywhere in it (the containers' bodies too), as the last run left them.
pub fn of(s: &Session) -> Vec<Check> {
    let run = Run { error: s.error.as_deref().and_then(crate::diagnose::failed_at), warnings: &s.warnings };
    let mut out = Vec::new();
    for n in &s.source.nodes {
        let ran = matches!(s.status.get(&n.id), Some(NodeStatus::Done { .. }));
        walk(&run, s, &[], n, ran, &mut out);
    }
    out
}

fn walk(run: &Run, s: &Session, path: &[String], n: &Node, ran: bool, out: &mut Vec<Check>) {
    if n.op == "check.assert" {
        // (the outputs of a container's body are not kept: its check passed when the container ran to its end)
        let ok = if path.is_empty() {
            match s.value_of(&n.id, "ok") {
                Some(Value::Bool(b)) => Some(b),
                _ => None,
            }
        } else {
            None
        };
        out.push(Check {
            path: path.to_vec(),
            id: n.id.clone(),
            expression: n.params.get("expression").and_then(|e| e.as_str()).unwrap_or_default().to_string(),
            verdict: verdict(run, path, n, ok, ran),
        });
    }
    if let Some(body) = &n.body {
        let inner: Vec<String> = path.iter().cloned().chain(std::iter::once(n.id.clone())).collect();
        for b in &body.nodes {
            walk(run, s, &inner, b, ran, out);
        }
    }
}

/// The badges of the nodes of a scope (`path`): a check's verdict, a container's worst of the checks it holds.
pub fn badges(checks: &[Check], path: &[String]) -> HashMap<String, Verdict> {
    let mut out: HashMap<String, Verdict> = HashMap::new();
    for c in checks {
        let key = if c.path == path {
            c.id.clone()
        } else if c.path.len() > path.len() && c.path.starts_with(path) {
            c.path[path.len()].clone()
        } else {
            continue;
        };
        let worse = out.get(&key).is_none_or(|v| c.verdict.rank() > v.rank());
        if worse {
            out.insert(key, c.verdict.clone());
        }
    }
    out
}

/// The model's health in a few words: how many checks passed, warned, failed, did not run (none: no checks).
pub fn health(checks: &[Check]) -> Option<String> {
    if checks.is_empty() {
        return None;
    }
    let count = |f: fn(&Verdict) -> bool| checks.iter().filter(|c| f(&c.verdict)).count();
    let parts: Vec<String> = [
        (count(|v| matches!(v, Verdict::Passed)), "passed"),
        (count(|v| matches!(v, Verdict::Warned(_))), "warned"),
        (count(|v| matches!(v, Verdict::Failed(_))), "failed"),
        (count(|v| matches!(v, Verdict::NotRun)), "not run"),
        (count(|v| matches!(v, Verdict::Off)), "off"),
    ]
    .into_iter()
    .filter(|(k, _)| *k > 0)
    .map(|(k, w)| format!("{k} {w}"))
    .collect();
    let word = if checks.len() == 1 { "check" } else { "checks" };
    Some(format!("{} {word}: {}", checks.len(), parts.join(", ")))
}

/// The worst verdict of the checks (none: no checks).
pub fn worst(checks: &[Check]) -> Option<Verdict> {
    checks.iter().map(|c| c.verdict.clone()).max_by_key(Verdict::rank)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(path: &[&str], id: &str, verdict: Verdict) -> Check {
        Check { path: path.iter().map(|p| p.to_string()).collect(), id: id.into(), expression: "x".into(), verdict }
    }

    #[test]
    fn a_container_shows_its_worst_check() {
        let checks = [
            check(&[], "sum", Verdict::Passed),
            check(&["years"], "demand", Verdict::Warned("short".into())),
            check(&["years", "inner"], "area", Verdict::Passed),
        ];
        let top = badges(&checks, &[]);
        assert_eq!(top["sum"], Verdict::Passed);
        assert_eq!(top["years"], Verdict::Warned("short".into()));
        let inside = badges(&checks, &["years".to_string()]);
        assert_eq!(inside.len(), 2);
        assert_eq!(inside["inner"], Verdict::Passed);
        assert_eq!(health(&checks).as_deref(), Some("3 checks: 2 passed, 1 warned"));
        assert_eq!(worst(&checks), Some(Verdict::Warned("short".into())));
        assert_eq!(health(&[]), None);
    }

    #[test]
    fn a_check_fares_by_the_run() {
        let n = Node::new("demand", "check.assert").param("expression", "abs(a - d) < 1");
        let warnings = vec!["demand: the demand is met (abs(a - d) < 1: it is 0)".to_string()];
        let run = Run { error: None, warnings: &warnings };
        assert_eq!(
            verdict(&run, &[], &n, Some(false), true),
            Verdict::Warned("the demand is met (abs(a - d) < 1: it is 0)".into())
        );
        assert_eq!(verdict(&run, &[], &n, Some(true), true), Verdict::Passed);
        let none = Run { error: None, warnings: &[] };
        assert_eq!(verdict(&none, &[], &n, None, false), Verdict::NotRun);
        // inside a loop: its error names it by its path
        let failed = Run {
            error: Some(("years/demand".into(), "node \"demand\": short (abs(a - d) < 1: it is 3)".into())),
            warnings: &[],
        };
        assert_eq!(
            verdict(&failed, &["years".to_string()], &n, None, false),
            Verdict::Failed("short (abs(a - d) < 1: it is 3)".into())
        );
        assert_eq!(verdict(&none, &["years".to_string()], &n, None, true), Verdict::Passed);
    }
}
