//! What describe_operator tells beyond an operator's ports: how it is used, learnt from the templates (the
//! operators whose outputs it usually reads, those that read its outputs, the templates it is in), the method it
//! is part of and the papers to cite ([`crate::methods`]), and the advice of a modeller for the operators where
//! a choice is easy to get wrong.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use terraflux_graph::Node;

/// Advice on an operator: what to watch when using it.
const TIPS: &[(&str, &str)] = &[
    (
        "raster.calc",
        "Write its script with write_calc (checked and tried on a sample of the cells before it changes \
         anything), take one apart with explain_calc; a script of a few named lines, a comment each.",
    ),
    (
        "raster.match",
        "Put every map of a model on one reference grid (the land use of the start, in a projected coordinate \
         system) before combining them. Resampling: near or mode for classes, bilinear or average for values (auto \
         chooses by the data).",
    ),
    (
        "raster.reproject",
        "Distances and areas need a projected coordinate system: a UTM zone, or an equal-area projection when \
         areas are compared. Classes are taken by the nearest cell (or the mode when the cells grow).",
    ),
    (
        "markov.fit",
        "Calibrate it on the earlier period (t0, t1) and keep a later map to validate the simulation. `steps` is \
         the number of time steps in the period: the rates are per step (10 years simulated a year at a time: 10).",
    ),
    (
        "woe.fit",
        "Correlated drivers count twice: check_model warns on collinear drivers (drop one of a pair). Name the \
         categorical drivers (soils, zoning) in `categorical`; look at the weights' table (and the viewer's WoE \
         editor) before simulating: a weight against what is known of the transition is worth a note.",
    ),
    (
        "dinamica.step",
        "One step of the simulation: run it in a control.repeat. Its seed draws where the patches go; one run is \
         one possible future, so run several seeds (control.each) and validate against a map not used to \
         calibrate (validate.three_map, validate.similarity).",
    ),
    (
        "lulcc.step",
        "One step of CLUE-S: run it in a control.repeat with the demand of each step; the elasticity of each class \
         (how hard it is to convert) is a choice to record in the node's why.",
    ),
    (
        "cars.step",
        "One step of PLUS's CARS: its growth probabilities come from ml.potential with `later` (LEAS); run it in a \
         control.repeat and validate the simulation against a map not used to calibrate.",
    ),
    (
        "validate.three_map",
        "t0 and t1 are the reference maps of the start and the end: the end must be a map the model was not \
         calibrated on. `factors` [1, 3, 9, 27] give the figure of merit at coarser resolutions (Pontius et al. \
         2011); compare it with the null model of no change.",
    ),
    (
        "validate.similarity",
        "Dinamica's validation: the minimum of the two directions is the result; compare the observed with the \
         simulated change from the same start, the observed map not used to calibrate.",
    ),
    (
        "landscape.mspa",
        "The foreground is the classes in `foreground` (else the map coded 0 missing, 1 background, 2 foreground). \
         `edge_width` is in cells: the result's scale is edge_width times the cell size, say it in the note.",
    ),
    (
        "metrics.landscape",
        "A map of classes (whole numbers) in a projected coordinate system. Ask for the few metrics that answer \
         the question (`metrics`), not all of them; `directions` (8 or 4) changes the patches.",
    ),
    (
        "metrics.class",
        "A map of classes (whole numbers) in a projected coordinate system. Ask for the few metrics that answer \
         the question (`metrics`), not all of them; `directions` (8 or 4) changes the patches.",
    ),
    (
        "metrics.patch",
        "A map of classes (whole numbers) in a projected coordinate system; `directions` (8 or 4) changes the \
         patches.",
    ),
    (
        "sdm.fit",
        "Folds: spatial (sdm.spatial_blocks, sdm.buffer_folds) rather than random when the records cluster. A \
         large auc_diff (training minus test) means the model overfits: fewer feature classes (`classes`), a \
         larger `regmult`.",
    ),
    (
        "sdm.random_folds",
        "Random folds inflate the accuracy when the records cluster in space (Roberts et al. 2017): evaluate on \
         spatial folds too (sdm.spatial_blocks, sdm.buffer_folds).",
    ),
    (
        "prioritize.optimize",
        "The targets, the cost and the penalties are the choices: give their sources in the node's refs; compare \
         solutions under other targets or costs as scenarios, and evaluate one with prioritize.evaluate.",
    ),
    (
        "circuit.omniscape",
        "The resistance must be above 0 everywhere it has a value; `radius` (in cells) is the species' movement \
         range: record where it comes from.",
    ),
    (
        "cost.transition",
        "Of a conductance (1 / resistance), usually corrected for the cells' distances (cost.geo_correction) before \
         cost.acc_cost or cost.passage.",
    ),
];

/// How the templates use an operator.
#[derive(Default)]
struct Use {
    /// The operators whose outputs it reads, and how often.
    reads: BTreeMap<String, usize>,
    /// The operators that read its outputs.
    read_by: BTreeMap<String, usize>,
    templates: Vec<&'static str>,
}

/// The operators' uses in the templates, gathered once.
fn uses() -> &'static BTreeMap<String, Use> {
    static USES: OnceLock<BTreeMap<String, Use>> = OnceLock::new();
    USES.get_or_init(|| {
        let mut out: BTreeMap<String, Use> = BTreeMap::new();
        for t in crate::templates::TEMPLATES {
            let m = t.model();
            let mut nodes = Vec::new();
            every(&m.nodes, &mut nodes);
            let op_of: BTreeMap<&str, &str> = nodes.iter().map(|n| (n.id.as_str(), n.op.as_str())).collect();
            for n in &nodes {
                let u = out.entry(n.op.clone()).or_default();
                if !u.templates.contains(&t.name) {
                    u.templates.push(t.name);
                }
                // its own inputs and settings (not its body's: those are the body's nodes')
                let own = Node { body: None, ..(*n).clone() };
                for r in terraflux_graph::references(&own) {
                    let Some((id, _)) = r.split_once('.') else { continue };
                    let Some(from) = op_of.get(id) else { continue };
                    if *from == n.op {
                        continue;
                    }
                    *out.entry(n.op.clone()).or_default().reads.entry(from.to_string()).or_default() += 1;
                    *out.entry(from.to_string()).or_default().read_by.entry(n.op.clone()).or_default() += 1;
                }
            }
        }
        out
    })
}

fn every<'a>(ns: &'a [Node], out: &mut Vec<&'a Node>) {
    for n in ns {
        out.push(n);
        if let Some(b) = &n.body {
            every(&b.nodes, out);
        }
    }
}

/// The most frequent operators, the plots and neutral landscapes left out (every template draws and plots).
fn most(counts: &BTreeMap<String, usize>) -> Vec<&str> {
    let mut v: Vec<(&str, usize)> = counts
        .iter()
        .filter(|(op, _)| *op != "view.plot" && !op.starts_with("nlm."))
        .map(|(op, n)| (op.as_str(), *n))
        .collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    v.into_iter().take(6).map(|(op, _)| op).collect()
}

/// What describe_operator adds of an operator (empty: nothing to add).
pub fn notes(op: &str) -> String {
    let mut s = String::new();
    if let Some((_, tip)) = TIPS.iter().find(|(o, _)| *o == op) {
        s.push_str(&format!("\nAdvice: {tip}\n"));
    }
    if let Some(m) = crate::methods::method_of(op) {
        let cites: Vec<String> = m
            .refs
            .iter()
            .chain(m.software_refs.iter())
            .filter_map(|k| crate::methods::REFERENCES.iter().find(|r| r.key == *k))
            .map(|r| r.cite())
            .collect();
        s.push_str(&format!("\nMethod: {}", m.name));
        if !m.software.is_empty() {
            s.push_str(&format!(" (as {})", m.software));
        }
        if !cites.is_empty() {
            s.push_str(&format!("; cite {}", cites.join("; ")));
        }
        s.push_str(" (methods drafts the paper's section).\n");
    }
    if let Some(u) = uses().get(op) {
        let (reads, read_by) = (most(&u.reads), most(&u.read_by));
        if !reads.is_empty() {
            s.push_str(&format!("\nIn the templates it reads the outputs of: {}.\n", reads.join(", ")));
        }
        if !read_by.is_empty() {
            s.push_str(&format!("Its outputs are read by: {}.\n", read_by.join(", ")));
        }
        let shown: Vec<String> = u.templates.iter().take(5).map(|t| format!("{t:?}")).collect();
        s.push_str(&format!(
            "Templates with it: {}{}.\n",
            shown.join(", "),
            if u.templates.len() > 5 { format!(" and {} more", u.templates.len() - 5) } else { String::new() }
        ));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tips_are_of_operators() {
        let r = terraflux_ops::Registry::standard();
        for (op, _) in TIPS {
            assert!(r.sig(op).is_some(), "a tip of {op}, not an operator");
        }
    }

    #[test]
    fn learnt_from_the_templates() {
        let n = notes("woe.fit");
        assert!(n.contains("Advice: Correlated drivers"), "{n}");
        assert!(n.contains("Its outputs are read by:") && n.contains("woe.probability"), "{n}");
        assert!(n.contains("\"Dinamica EGO\""), "{n}");
        let n = notes("landscape.mspa");
        assert!(n.contains("Method: morphological spatial pattern analysis"), "{n}");
        assert!(n.contains("cite Soille"), "{n}");
        assert!(notes("no.such").is_empty());
    }
}
