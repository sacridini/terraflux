//! The classic land change models as graphs: every node with a note saying what it does and why its
//! settings, so that a model made from a preset can be read and changed node by node.
//!
//! Each preset adds its nodes and outputs to a model whose inputs it names (the caller declares them).

use serde_json::json;
use terraflux_graph::{Body, Model, Node, Output};

/// Dinamica EGO's classic model: Markov rates, weights of evidence, Expander and Patcher.
#[derive(Debug, Clone)]
pub struct Dinamica {
    /// Inputs: the maps at the start and the end of the calibration period, the drivers, the seed, and the map
    /// the simulation starts from.
    pub t0: String,
    pub t1: String,
    pub drivers: String,
    pub seed: String,
    pub start: String,
    /// Time steps in the calibration period (the rates are per step) and steps to simulate.
    pub steps: usize,
    pub simulate: usize,
    /// The weights' buffer increment (`None`: a hundredth of each variable's range).
    pub increment: Option<f64>,
    /// The Expander's share for every transition (`None`: measured on the observed change).
    pub expansion: Option<f64>,
    pub isometry: f64,
    pub prune_factor: f64,
}

impl Default for Dinamica {
    fn default() -> Self {
        Dinamica {
            t0: "t0".into(),
            t1: "t1".into(),
            drivers: "drivers".into(),
            seed: "seed".into(),
            start: "t0".into(),
            steps: 1,
            simulate: 1,
            increment: None,
            expansion: None,
            isometry: 1.5,
            prune_factor: 10.0,
        }
    }
}

pub fn dinamica(m: &mut Model, p: &Dinamica) {
    let (t0, t1, dr, seed, start) = (
        format!("${}", p.t0),
        format!("${}", p.t1),
        format!("${}", p.drivers),
        format!("${}", p.seed),
        format!("${}", p.start),
    );
    m.nodes.push(
        Node::new("markov", "markov.fit").input("t0", &t0).input("t1", &t1).literal("steps", p.steps as i64).note(
            &format!(
                "Transition rates per step: the {}-th root of the matrix of the calibration period, as Dinamica's \
                 DetermineTransitionMatrix.",
                p.steps
            ),
        ),
    );
    let mut woe = Node::new("woe", "woe.fit")
        .input("t0", &t0)
        .input("t1", &t1)
        .input("drivers", &dr)
        .input("markov", "markov.markov");
    woe = match p.increment {
        Some(i) => woe.param("increment", i).note("Weights of evidence of every transition with a rate, the ranges cut automatically from buffers of the given increment."),
        None => woe.note("Weights of evidence of every transition with a rate, the ranges cut automatically (buffers of a hundredth of each variable's range)."),
    };
    m.nodes.push(woe);
    let mut patches = Node::new("patches", "patches.observe")
        .input("t0", &t0)
        .input("t1", &t1)
        .input("markov", "markov.markov")
        .param("isometry", p.isometry);
    patches = match p.expansion {
        Some(e) => patches.param("expansion", e).note("Patch sizes measured on the observed change; the Expander's share given; the isometry is not measurable (Dinamica's usual 1.5)."),
        None => patches.note("The Expander's share and the patch sizes measured on the observed change (expansions touch the target class); the isometry is not measurable (Dinamica's usual 1.5)."),
    };
    m.nodes.push(patches);
    let body = Body {
        nodes: vec![
            Node::new("probability", "woe.probability")
                .input("model", "woe.model")
                .input("landscape", "@landscape")
                .input("drivers", &dr)
                .note("The transition probabilities of the current map."),
            Node::new("allocate", "dinamica.step")
                .input("landscape", "@landscape")
                .input("probability", "probability.probability")
                .input("rates", "markov.markov")
                .input("patches", "patches.patches")
                .input("step", "@step")
                .input("seed", &seed)
                .param("prune_factor", p.prune_factor)
                .param("window", json!([3, 3]))
                .note("Expander and Patcher: each transition changes its rate times the cells of its origin class."),
        ],
        carry: [("landscape".to_string(), "allocate.landscape".to_string())].into(),
        collect: [
            ("maps".to_string(), "allocate.landscape".to_string()),
            ("info".to_string(), "allocate.info".to_string()),
        ]
        .into(),
        ..Default::default()
    };
    m.nodes.push(
        Node::new("simulation", "control.repeat")
            .literal("times", p.simulate as i64)
            .input("landscape", &start)
            .note("One iteration per time step, the map carried from step to step.")
            .body(body),
    );
    m.outputs.insert("maps".into(), Output { from: "simulation.maps".into(), path: None, dtype: None });
    m.outputs.insert("steps".into(), Output { from: "simulation.info".into(), path: None, dtype: None });
    m.outputs.insert("weights".into(), Output { from: "woe.table".into(), path: None, dtype: None });
}

/// A CLUE-S or ordered model (lulcc's): logistic regressions, a linear demand from observed maps and
/// lulcc's allocation with its decision rules.
#[derive(Debug, Clone)]
pub struct Lulcc {
    /// Inputs: the observed maps (a stack, one band per date) for the demand, the drivers, the seed, the map
    /// the simulation starts from (and the regressions are fitted on), optional restrictions and history.
    pub observed: String,
    pub drivers: String,
    pub seed: String,
    pub start: String,
    pub restrictions: Option<String>,
    pub history: Option<String>,
    /// The date of each observed map.
    pub times: Vec<f64>,
    /// { class: drivers } of the regressions (None: every class on every driver).
    pub formulas: Option<serde_json::Value>,
    /// `{ method = "clue", ... }` or `{ method = "ordered", order = [...] }`.
    pub allocation: serde_json::Value,
    pub elasticity: Option<serde_json::Value>,
    pub allowed: Option<serde_json::Value>,
    pub neighbourhood: Option<serde_json::Value>,
}

pub fn lulcc(m: &mut Model, p: &Lulcc) {
    let (obs, dr, seed, start) =
        (format!("${}", p.observed), format!("${}", p.drivers), format!("${}", p.seed), format!("${}", p.start));
    let mut fit = Node::new("regression", "glm.fit").input("landscape", &start).input("drivers", &dr);
    if let Some(f) = &p.formulas {
        fit = fit.param("formulas", f.clone());
    }
    m.nodes.push(fit.note("Logistic regression of the presence of each class on its drivers (lulcc's glmModels)."));
    m.nodes.push(
        Node::new("suitability", "glm.predict")
            .input("model", "regression.model")
            .input("drivers", &dr)
            .input("landscape", &start)
            .note("The suitability of each class at every cell; the drivers are static, so it is computed once."),
    );
    m.nodes.push(
        Node::new("demand", "demand.from_maps")
            .input("maps", &obs)
            .param("times", json!(p.times))
            .note("The cells of each class at each step interpolated and extrapolated linearly from the observed maps (lulcc's approxExtrapDemand)."),
    );
    m.nodes.push(
        Node::new("rng", "rng.r").input("seed", &seed).note("R's generator after set.seed(seed), as lulcc draws."),
    );
    let mut hist = Node::new("history", "lulcc.history").input("landscape", &start);
    if let Some(h) = &p.history {
        hist = hist.input("history", &format!("${h}"));
    }
    m.nodes.push(hist.note("How long each cell has had its class (1 where not given)."));
    let mut step = Node::new("allocate", "lulcc.step")
        .input("landscape", "@landscape")
        .input("history", "@history")
        .input("suitability", "suitability.suitability")
        .input("demand", "demand.demand")
        .input("step", "@step")
        .input("rng", "rng.rng")
        .param("allocation", p.allocation.clone());
    if let Some(r) = &p.restrictions {
        step = step.input("restrictions", &format!("${r}"));
    }
    if let Some(e) = &p.elasticity {
        step = step.param("elasticity", e.clone());
    }
    if let Some(a) = &p.allowed {
        step = step.param("allowed", a.clone());
    }
    if let Some(nb) = &p.neighbourhood {
        step = step.param("neighbourhood", nb.clone());
    }
    let method = p.allocation.get("method").and_then(|v| v.as_str()).unwrap_or("clue");
    step = step.note(if method == "ordered" {
        "The ordered allocation (Fuchs et al. 2013): classes served in order, cells drawn by suitability."
    } else {
        "CLUE-S: every cell takes the class of highest suitability plus an iteration variable, moved until the demand is met."
    });
    m.nodes.push(
        Node::new("simulation", "control.repeat")
            .input("times", "demand.steps")
            .input("landscape", &start)
            .input("history", "history.history")
            .note("One iteration per step of the demand, the map and the history carried.")
            .body(Body {
                nodes: vec![step],
                carry: [
                    ("landscape".to_string(), "allocate.landscape".to_string()),
                    ("history".to_string(), "allocate.history".to_string()),
                ]
                .into(),
                collect: [
                    ("maps".to_string(), "allocate.landscape".to_string()),
                    ("info".to_string(), "allocate.info".to_string()),
                ]
                .into(),
                ..Default::default()
            }),
    );
    m.outputs.insert("maps".into(), Output { from: "simulation.maps".into(), path: None, dtype: None });
    m.outputs.insert("steps".into(), Output { from: "simulation.info".into(), path: None, dtype: None });
}
