//! The guides an assistant reads when it needs them (the tool `guide`, and the resources `terraflux://guide/...`
//! of `terraflux mcp`): how a model is built (its nodes, the calculators' scripts, the containers, the record of
//! its choices, its results) and the methods of each field (land change, connectivity, landscape patterns,
//! conservation planning, species distributions, the data put in shape), step by step with the operators and the
//! templates of each step. The instructions told when an assistant connects stay short (clients cut long ones);
//! the rest is here, read on demand.

/// A guide: its name (the topic asked for), its title and its text.
pub struct Guide {
    pub name: &'static str,
    pub title: &'static str,
    pub text: &'static str,
}

/// The guides, those of building a model first, then those of the methods.
pub const GUIDES: &[Guide] = &[
    Guide {
        name: "nodes",
        title: "Nodes, their inputs and settings, files",
        text: "\
- A node runs one operator (`op`) and gives outputs (its ports). An input of a node reads `node.port` (another \
node's output), `$name` (an input of the model), `@name` (inside a container: a value carried along a loop, \
`@step`, a group's input) or is a literal value.
- Settings are the operator's options: a value, or a value read when the model runs (`\"$name\"`, `\"@step\"`, \
or `{\"from\": \"node.port\"}` for a node's number or text): a threshold that changes with the loop's step, a \
submodel's setting given as its input.
- Look an operator up (describe_operator) before using it: its exact ports, settings, choices and defaults, a node \
of a template using it, what it usually reads and what reads it. Never guess a port's name.
- Every edit is checked: a connection of the wrong type, a port the operator does not have, a cycle, is refused \
with the reason and the file is left as it was. The reply lists the problems the model still has (a model being \
built has some: inputs not connected yet).
- Files: a raster.read node reads a raster (setting `path`) and gives `raster`; give absolute paths (they are \
stored relative to the model's folder). The model's outputs (set_output) are what run_model writes; raster.write \
and table.write nodes write files too. inspect_file tells what a file holds before it is used.",
    },
    Guide {
        name: "calc",
        title: "Calculators: raster.calc, vector.calc, vector.filter and every expression",
        text: "\
- raster.calc (and every expression: value.calc, table.filter, agents.update's rules...) takes Python's syntax, \
the only one: one statement a line (`#` a comment), `name = value`, `name[where] = value` (the cells where it \
holds), `a if c else b`, `**`, `and or not`, `None` (no value), chained comparisons, a stack's layers as \
`s.layer`, and `out name = value` for each output (the node's ports). Other forms (`if c then a else b`, `^`, \
`&&`, `null`, `ln`) are refused.
- Write a script of a few named lines with a comment. A reclassification: `classes = 1`, \
`classes[elev >= 700] = 2`, `classes[elev >= 1000] = 3`, `out classes` (a line each).
- write_calc writes a calculator's script: of maps (raster.calc), of features' attributes (vector.calc: \
`out name = value` for each attribute, texts compared as uso == \"forest\", the geometry as geometry.area) or of \
the features kept (vector.filter: the script ends with the condition); checked, and tried on a sample of its \
inputs' cells or features, before anything changes: read what each output gives in its reply.
- explain_calc takes a script apart: what each input is and what each name holds on a sample. Use write_calc and \
explain_calc, not add_node or edit_node, for a calculator's script.
- describe_operator raster.calc lists its functions.",
    },
    Guide {
        name: "containers",
        title: "Containers, stages, parameters and scenarios",
        text: "\
- Containers hold a body of nodes: control.repeat (a loop of steps: a simulation step after step, `@step` the \
step), control.each (the body once per element of a collection: `@item`, `@index` and `@name` inside, what it \
collects gathered: the maps of a folder, the years, the seeds of a stochastic model), control.switch (a group per \
branch in its body, the one named as its case runs: a choice of method), control.group (a stage of the model).
- Give `scope` (the container's id; `outer/inner` for one in another's body) to add or edit nodes inside it, and \
edit_container for what the body carries (a value from step to step), collects or gives.
- A model is read in stages: once it is built, put its top level in a group per stage (group_by_stage: data, \
preparation, model, validation, results, each with a note; or group_nodes for groups of your own). The \
application then shows a block per stage, and the model's document a section per group.
- The values a user is to change (a rate, a threshold, a demand) go in the model's form of parameters \
(set_parameter: a label, a unit, a range and a help each), and named sets of them in scenarios (set_scenario: \
business as usual, a protection policy...). run_model takes `scenario`.",
    },
    Guide {
        name: "record",
        title: "The model as the record of its choices",
        text: "\
- The model is the record of what was done and why: give each node you add a `note` (what it is for) and a `why` \
(why this operator and these settings), and `refs` (the papers, methods, documentation the choice rests on), \
`assumptions` (what it takes for granted: the classes' meaning, a stationary rate) and `alternatives` (what was \
considered and set aside, and why) when there are.
- Give the model the user's `request` in the user's words, a `summary` of the conversation and its `author` \
(new_model, set_model_info); update the summary when the model changes.
- check_model names the nodes without a note or a why. methods drafts a paper's methods section from this \
record (the methods, the papers to cite), in English or Portuguese.",
    },
    Guide {
        name: "results",
        title: "Results: runs, failures, why a cell has its value, methods",
        text: "\
- check_model checks the model and its data (grids, classes, drivers) and gives advice on the method (a \
simulation not validated, maps in degrees measured in metres...); run_model runs it and writes its outputs; \
inspect_file summarises a raster (size, coordinate system, classes, statistics) or shows a table.
- When a run fails, its answer says which node failed, its message and what it was given side by side (each \
input's grid: size, cells, coordinate system) with what likely does not fit: read them before changing anything, \
look at the inputs it names (inspect_file, describe_operator), and put right what does not fit (raster.match for \
maps of different grids, raster.reproject for other coordinate systems), then run again.
- Why a cell of a result has its value: trace_cell gives the chain of values that made it (expressions with the \
values in place, the cells around, what a model weighs, a loop step by step). Explain a result from it, not from \
what the model should do.
- methods drafts the methods section of a paper from the model (the methods it uses, the papers to cite, the \
model step by step, the references; or BibTeX, RIS), in English or Portuguese (`language`: pt).",
    },
    Guide {
        name: "data-preparation",
        title: "Data put in shape: one grid, classes, drivers",
        text: "\
1. Look at every file first (inspect_file): its size, cells, coordinate system, classes or statistics, no-data.
2. One reference grid: the land use (or habitat) map of the start, in a projected coordinate system (metres: a \
UTM zone, or an equal-area projection for areas). Maps in longitudes and latitudes go through raster.reproject; \
every other map through raster.match onto the reference (its cells the same cells). Resampling by the data: \
classes by the nearest cell or the mode, values bilinear or by the mean (`auto` chooses).
3. Classes: whole numbers, a few of them; reclassify with raster.reclassify (rules `1 2 3 -> 1`) or write_calc; \
clean speckles with raster.sieve or raster.majority. Say what each class is in the node's note.
4. Drivers (explanatory variables) as a stack (stack.of, a layer each, named): distances to roads, rivers, towns \
or a class (vector.distance, distance.proximity), slope and elevation (raster.terrain, surface.terrain), soils as \
categories. Not constant, with values wherever the land has a class, not collinear: check_model checks them.
5. Vectors: vector.read, vector.reproject to the reference's system, vector.rasterize onto its grid.
Templates: \"Drivers of land change\", \"Cleaning a map of classes: filter, sieve, label and grow\", \"Terrain of a \
surface\".",
    },
    Guide {
        name: "land-change",
        title: "Land use and land cover change: analyse, calibrate, simulate, validate",
        text: "\
1. Data: two dates of land use to calibrate (t0, t1) and, to validate, a third (t2) not used to calibrate; all on \
one grid (guide data-preparation); the drivers as a stack.
2. Analyse the change observed: change.crosstab and change.transitions (the matrix of transitions), \
change.intensity (intensity analysis, Aldwaik & Pontius 2012: which classes and transitions are active), \
change.components (quantity, exchange and shift, Pontius & Santacruz 2014). Template \"Land change analysis\".
3. Calibrate on the first period only (t0 to t1):
   - the quantity: markov.fit (`steps`: the time steps in the period, the rates are per step), or a demand \
(demand.table, demand.from_maps, plus.demand);
   - where: weights of evidence (woe.fit, then woe.probability: Dinamica EGO), logistic regressions (glm.fit, \
glm.predict: CLUE-S), random forests (ml.potential with `later`: PLUS's LEAS). Drivers correlated give double \
weight: check_model warns on collinear drivers.
4. Simulate step by step in a control.repeat: dinamica.step (Expander and Patcher), lulcc.step (CLUE-S), cars.step \
(PLUS), clumondo.step, molusce.step. Templates \"Dinamica EGO\", \"CLUE-S\", \"PLUS (LEAS and CARS)\".
5. Validate: simulate from t1 to the date of t2 and compare with t2: validate.three_map (the budget of agreement \
and the figure of merit at several resolutions, Pontius et al. 2011), validate.similarity (Dinamica's fuzzy \
similarity), validate.agreement, validate.kappa. Compare with the null model (no change: t1 as the simulation, \
Pontius et al. 2004): a model that does not beat it has learnt nothing. Never validate on the map it was \
calibrated on. Template \"Validating a simulation\".
6. Uncertainty: the stochastic steps (a seed) give one possible future a run; run several seeds (control.each \
over a list of seeds) and report the spread; futures under other assumptions as scenarios (set_scenario).
7. Record the classes, the period, the rates' assumption (stationary) and the validation in the nodes' notes; \
methods drafts the paper's section.",
    },
    Guide {
        name: "connectivity",
        title: "Connectivity: structural, graphs, least costs, circuits",
        text: "\
1. Data: the habitat (a map of classes, or 0 missing, 1 background, 2 habitat), in a projected coordinate system \
(distances in metres); a resistance (or conductance) surface where movement is studied: by reclassifying the \
land use (raster.reclassify, from the literature or expert values, recorded in the node's refs) or growing with \
the distance to the habitat (restoration.resistance).
2. Structural connectivity: landscape.mspa (cores, bridges, loops, branches; `edge_width` in cells), then \
networks.conefor or networks.gtn_components and networks.gtn_importance (the network of cores and bridges). \
Template \"Network of a forest (MSPA)\".
3. Graphs of patches: conn.graph_from_raster or conn.graph_from_vector (Euclidean, least-cost or commute-time \
distances), conn.overall (IIC, PC and their equivalent connectivity), conn.node_importance (dIIC, dPC: which \
patches matter), conn.link_importance, conn.protconn (ProtConn of protected areas). The dispersal distance comes \
from the species studied (record it in refs). Template \"Habitat network (Conefor)\".
4. Least costs and random walks: cost.transition of a conductance (1 / resistance; cost.geo_correction for the \
cells' distances), then cost.acc_cost, cost.shortest_path, cost.passage (random walks, randomised shortest \
paths); restoration.corridor and restoration.least_cost_path. Template \"Least costs and random walks\", \
\"Restoring a corridor\".
5. Circuits: circuit.pairwise (effective resistances and currents between focal points), circuit.one_to_all, \
circuit.omniscape (omnidirectional, moving windows). The resistance must be above 0. Template \"Circuit \
connectivity\".
6. Rivers: hydro.dci (the dendritic connectivity index of dams). Template \"Rivers: dams and their connectivity \
(DCI)\".",
    },
    Guide {
        name: "landscape-metrics",
        title: "Landscape patterns: metrics, MSPA, fragmentation",
        text: "\
1. Data: a map of classes (whole numbers) in a projected coordinate system: areas and edges are measured in the \
cells' units. A continuous map is classified first (raster.reclassify, write_calc).
2. Metrics as landscapemetrics and FRAGSTATS: metrics.patch, metrics.class, metrics.landscape (`metrics` a name \
or a list; `type` a group of them), metrics.window (a moving window: a map of a metric). Choose a few metrics \
that answer the question (and are not redundant: metrics.correlation), not all of them. The patches' \
connectivity (`directions`: 8 or 4) changes the counts: say which.
3. Many landscapes or dates: metrics.sample (points, buffers, polygons), vector.grid_zones (zones in a grid), \
metrics.series (dates); signatures and groups: metrics.signatures, metrics.pca, metrics.cluster.
4. Morphology and fragmentation (GuidosToolbox): landscape.mspa (the foreground's classes in `foreground`, the \
edge's width in cells: the scale of the result), fragmentation.fad and fragmentation.fad_multiscale (forest area \
density), fragmentation.fos, landscape.spa, landscape.accounting.
5. Change of patterns: change.simple, change.morphological, change.fad. Neutral landscapes (nlm.*) give the \
expected values of a metric without structure, to compare with.
Templates: \"Landscape metrics\", \"MSPA of your map\", \"Fragmentation of a forest\", \"Patches and their \
shapes\", \"Forest gain and loss\".",
    },
    Guide {
        name: "conservation-planning",
        title: "Conservation and restoration planning",
        text: "\
1. Data: the features (species' distributions, habitats, ecosystem services) as a stack, a layer each (stack.of), \
on one grid; a cost per planning unit (land price, opportunity cost; cells without a cost are not units), or \
planning units as polygons; areas already protected (locked in) or excluded (locked out).
2. Targets: how much of each feature to hold (relative, absolute, loglinear), from policy or the literature \
(record them in refs).
3. Solve: prioritize.optimize (integer linear programming, optimal within the gap: min_set, min_shortfall, \
max_utility... with boundary or connectivity penalties), prioritize.zonation (a ranking of every cell, Zonation \
5), prioritize.marxan (simulated annealing, Marxan 4), prioritize.actions and prioritize.actions_raster (actions \
against threats), restoration.plan (restoration's benefits and costs).
4. Evaluate: prioritize.evaluate (the cost, how much of each feature the solution holds, targets met, \
irreplaceability), prioritize.proxirr; compare solutions under other costs or targets as scenarios \
(set_scenario).
Templates: \"Conservation planning (linear programming)\", \"Zonation\", \"Marxan and irreplaceability\", \
\"Irreplaceability of a plan\", \"Restoration plan\", \"Multi-action planning\".",
    },
    Guide {
        name: "species-distribution",
        title: "Species distribution models",
        text: "\
1. Records: cleaned (sdm.clean_occurrences), one per cell (sdm.one_per_cell), thinned (sdm.thin) against the \
sampling bias; or all of it at once with sdm.prepare.
2. Predictors: a stack (stack.of) on one grid; without collinearity (sdm.vif, sdm.select_predictors); the \
accessible area (sdm.accessible_area); background points (sdm.background, a bias surface with \
sdm.bias_surface).
3. Folds for the evaluation: spatial (sdm.spatial_blocks, sdm.buffer_folds, sdm.checkerboard_folds) rather than \
random (sdm.random_folds) when records cluster, which inflates the accuracy (Roberts et al. 2017); \
sdm.autocorrelation_range gives the blocks' size.
4. Fit and evaluate: sdm.fit (MaxNet, a GLM or a random forest, by fold: AUC of training and test, TSS, Boyce), \
sdm.evaluate; auc_diff (training minus test) large: the model overfits.
5. Predict: sdm.maxnet_predict on the predictors' stack; a threshold (sdm.threshold) for a map of presence; \
extrapolation where the environment is new (sdm.mess, sdm.exdet, sdm.mop); in another climate, the same \
predictors' names on the future stack. Template \"Species distribution model\", \"A species in a warmer climate\".",
    },
];

/// A guide by its name (or its beginning, when only one guide has it).
pub fn find(name: &str) -> Option<&'static Guide> {
    let n = name.trim().to_lowercase().replace([' ', '_'], "-");
    if let Some(g) = GUIDES.iter().find(|g| g.name == n) {
        return Some(g);
    }
    let mut like = GUIDES.iter().filter(|g| g.name.starts_with(&n) || g.name.contains(&n));
    match (like.next(), like.next()) {
        (Some(g), None) if !n.is_empty() => Some(g),
        _ => None,
    }
}

/// The guides listed, a line each.
pub fn listed() -> String {
    let mut s = String::from("Guides (guide with `topic`):\n");
    for g in GUIDES {
        s.push_str(&format!("- {}: {}\n", g.name, g.title));
    }
    s
}

/// A guide's text with its title, or the list of them when no topic (or one not found) is given.
pub fn guide(topic: Option<&str>) -> Result<String, String> {
    match topic.map(str::trim).filter(|t| !t.is_empty()) {
        None => Ok(listed()),
        Some(t) => match find(t) {
            Some(g) => Ok(format!("{}\n\n{}\n", g.title, g.text)),
            None => Err(format!("no guide {t:?}. {}", listed())),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The operators a text names (`family.name` of a family the registry has).
    fn named_ops(text: &str, families: &std::collections::BTreeSet<String>) -> Vec<String> {
        let mut out = Vec::new();
        for w in text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '.' || c == '_')) {
            let w = w.trim_end_matches('.');
            if let Some((f, rest)) = w.split_once('.')
                && families.contains(f)
                && !rest.is_empty()
                && !rest.contains('.')
                && rest != "*"
            {
                out.push(w.to_string());
            }
        }
        out
    }

    #[test]
    fn every_operator_named_is_there() {
        let r = terraflux_ops::Registry::standard();
        let families: std::collections::BTreeSet<String> =
            r.signatures().iter().filter_map(|s| s.name.split_once('.').map(|(f, _)| f.to_string())).collect();
        // (ports named like operators: `node.port` examples)
        let not_ops = ["s.layer", "geometry.area"];
        for g in GUIDES {
            for op in named_ops(g.text, &families) {
                if not_ops.contains(&op.as_str()) {
                    continue;
                }
                assert!(r.sig(&op).is_some(), "the guide {} names {op}, not an operator", g.name);
            }
        }
    }

    #[test]
    fn every_template_named_is_there() {
        for g in GUIDES {
            for (k, part) in g.text.split('"').enumerate() {
                // the templates are quoted, after "Template"/"Templates" or in a list of them
                if k % 2 == 1 && part.chars().next().is_some_and(char::is_uppercase) && !part.contains('=') {
                    assert!(
                        crate::templates::TEMPLATES.iter().any(|t| t.name == part),
                        "the guide {} names the template {part:?}",
                        g.name
                    );
                }
            }
        }
    }

    #[test]
    fn found_by_name_or_its_beginning() {
        assert_eq!(find("land-change").map(|g| g.name), Some("land-change"));
        assert_eq!(find("Land change").map(|g| g.name), Some("land-change"));
        assert_eq!(find("species").map(|g| g.name), Some("species-distribution"));
        assert!(find("").is_none());
        assert!(guide(Some("nothing")).unwrap_err().contains("land-change"));
        assert!(guide(None).unwrap().contains("connectivity"));
    }
}
