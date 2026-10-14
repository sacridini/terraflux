//! Models to start from: small graphs that run as they are (their data drawn by neutral landscape models) or
//! once a file is chosen, to see how a flow is built and to check that it works.

use terraflux_graph::{Model, Node, Source};
use terraflux_ops::presets;

/// A model to start from: its name, what it shows, and the model (written, or built from a preset).
pub struct Template {
    pub name: &'static str,
    pub about: &'static str,
    toml: &'static str,
    build: Option<fn() -> Model>,
}

impl Template {
    pub fn model(&self) -> Model {
        match self.build {
            Some(f) => f(),
            None => Model::from_toml(self.toml).expect("a template is a model"),
        }
    }
}

/// Two dates of land use drawn for the land change templates: classes 1 (forest), 2 (farming) and 3 (water) by a
/// random cluster, and ten years later farming grown into the forest beside it where a suitability is high; the
/// suitability and a roughness, the drivers of the change. Nodes `land_2000`, `land_2010`, `drivers`.
fn land_change_data(m: &mut Model) {
    m.nodes.push(
        Node::new("land_2000", "nlm.random_cluster")
            .param("rows", 160)
            .param("columns", 160)
            .param("p", 0.5)
            .param("weights", serde_json::json!([0.55, 0.35, 0.10]))
            .param("seed", 11)
            .note("Land use drawn by chance: 1 forest, 2 farming, 3 water. In its place, a raster.read of a map of yours."),
    );
    m.nodes.push(
        Node::new("aptitude", "nlm.gaussian_field")
            .param("rows", 160)
            .param("columns", 160)
            .param("range", 25)
            .param("seed", 12)
            .note("A driver: how suited each cell is to farming (0 to 1)."),
    );
    m.nodes.push(
        Node::new("roughness", "nlm.fbm")
            .param("rows", 160)
            .param("columns", 160)
            .param("dimension", 1.4)
            .param("seed", 13)
            .note("A driver of no effect on the change, as real models have some."),
    );
    m.nodes.push(
        Node::new("land_2010", "map.calc")
            .input("t0", "land_2000.map")
            .input("suit", "aptitude.map")
            .param("expression", "if t0 == 1 and nbcount(t0 == 2, 1) >= 2 and suit > 0.45 then 2 else t0")
            .note("Ten years later: farming grown into the forest beside it, where it is suited."),
    );
    m.nodes.push(
        Node::new("drivers", "stack.of")
            .input("aptitude", "aptitude.map")
            .input("roughness", "roughness.map")
            .note("The drivers of the change, a layer each."),
    );
}

/// The references to inputs (`$name`) of nodes (and of their bodies) replaced by references to nodes, the seed by
/// a number.
fn rewire(nodes: &mut [Node], to: &[(&str, &str)]) {
    for n in nodes {
        for src in n.inputs.values_mut() {
            if let Source::Ref(r) = src {
                if r == "$seed" {
                    *src = Source::Literal(7.into());
                } else if let Some((_, by)) = to.iter().find(|(from, _)| *from == r.as_str()) {
                    *r = by.to_string();
                }
            }
        }
        if let Some(b) = &mut n.body {
            rewire(&mut b.nodes, to);
        }
    }
}

/// Dinamica EGO's classic model on the data drawn: Markov rates, weights of evidence, Expander and Patcher.
fn dinamica() -> Model {
    let mut m = Model::new("Dinamica EGO");
    m.description = Some(
        "Dinamica EGO's classic model, calibrated on two dates of land use drawn by chance (2000 and 2010) and \
         run 3 steps of 10 years from 2010."
            .into(),
    );
    land_change_data(&mut m);
    presets::dinamica(&mut m, &presets::Dinamica { start: "t1".into(), steps: 1, simulate: 3, ..Default::default() });
    rewire(&mut m.nodes, &[("$t0", "land_2000.map"), ("$t1", "land_2010.map"), ("$drivers", "drivers.stack")]);
    m.nodes.push(Node::new("plot", "view.plot").input("value", "simulation.maps").param("title", "Simulated land use"));
    m
}

/// CLUE-S (lulcc's CluesModel) on the data drawn: logistic regressions, a linear demand, CLUE's allocation.
fn clue_s() -> Model {
    let mut m = Model::new("CLUE-S");
    m.description = Some(
        "CLUE-S as lulcc's CluesModel: the regressions of every class on the drivers, the demand extended from \
         two dates of land use drawn by chance (2000 and 2010), the allocation of three steps of 10 years."
            .into(),
    );
    land_change_data(&mut m);
    m.nodes.push(
        Node::new("observed", "stack.of")
            .input("t2000", "land_2000.map")
            .input("t2010", "land_2010.map")
            .note("The observed maps, a layer per date, for the demand."),
    );
    presets::lulcc(
        &mut m,
        &presets::Lulcc {
            observed: "observed".into(),
            drivers: "drivers".into(),
            seed: "seed".into(),
            start: "start".into(),
            restrictions: None,
            history: None,
            times: vec![2000.0, 2010.0],
            formulas: None,
            allocation: serde_json::json!({"method": "clue"}),
            elasticity: Some(serde_json::json!([0.5, 0.2, 1.0])),
            allowed: None,
            neighbourhood: None,
        },
    );
    rewire(
        &mut m.nodes,
        &[("$observed", "observed.stack"), ("$start", "land_2010.map"), ("$drivers", "drivers.stack")],
    );
    // three steps of ten years from 2010
    if let Some(d) = m.nodes.iter_mut().find(|n| n.id == "demand") {
        d.params.insert("out_times".into(), serde_json::json!([2010, 2020, 2030, 2040]));
    }
    m.nodes.push(Node::new("plot", "view.plot").input("value", "simulation.maps").param("title", "Simulated land use"));
    m
}

pub const TEMPLATES: [Template; 18] = [
    Template {
        name: "MSPA of a forest",
        build: None,
        about: "a map of three classes drawn by chance, its forest made MSPA's foreground by a reclassification, \
                MSPA, and its map in a tab",
        toml: r#"
format = "terraflux/1"
name = "MSPA of a forest"
description = "GuidosToolbox's MSPA on a forest map: read (here drawn), reclassify, MSPA, plot."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.55, weights = [0.3, 0.15, 0.55], seed = 7 }
note = "A map of three classes drawn by chance (1 farming, 2 water, 3 forest): in its place, a raster.read of a map of yours."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "landscape.map" }
params = { rules = "3 -> 2\n* -> 1" }
note = "MSPA reads 2 as the foreground and 1 as the background: the forest (3) becomes 2, every other class 1."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 1, connectivity = 8 }
note = "Core, islet, perforation, edge, loop, bridge and branch, as GuidosToolbox computes them."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }
note = "Its map in a tab of its own, in GuidosToolbox's colours, refreshed at every run."
"#,
    },
    Template {
        name: "MSPA of your map",
        build: None,
        about: "the same flow on a raster of yours: choose its file in the node read, and in the reclassification \
                the classes of its forest",
        toml: r#"
format = "terraflux/1"
name = "MSPA of your map"
description = "GuidosToolbox's MSPA on a map of yours: read, reclassify, MSPA, plot."

[[nodes]]
id = "read"
op = "raster.read"
params = { path = "" }
note = "Choose your map's file (Node > path > Choose...)."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "read.map" }
params = { rules = "1 -> 2\n* -> 1" }
note = "Your forest's classes become 2 (the foreground), the rest 1 (the background): put your forest's classes in place of 1 (A rule per class lists the map's classes)."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 1, connectivity = 8 }
note = "Core, islet, perforation, edge, loop, bridge and branch, as GuidosToolbox computes them."

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }
note = "Its map in a tab of its own, refreshed at every run."
"#,
    },
    Template {
        name: "Forest from a surface",
        build: None,
        about: "a smooth surface drawn by chance cut at a threshold into forest and the rest (a reclassification \
                by ranges), then MSPA",
        toml: r#"
format = "terraflux/1"
name = "Forest from a surface"
description = "A continuous surface reclassified by ranges into a forest map, and its MSPA."

[[nodes]]
id = "surface"
op = "nlm.gaussian_field"
params = { rows = 300, columns = 300, range = 15, seed = 3 }
note = "A smooth surface from 0 to 1 drawn by chance: a suitability, say."

[[nodes]]
id = "forest"
op = "raster.reclassify"
inputs = { map = "surface.map" }
params = { rules = "0..0.45 -> 1\n0.45..1 -> 2" }
note = "Ranges: up to 0.45 the background (1), above it the forest (2). Move the threshold and see MSPA change."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.map" }
params = { edge_width = 2 }
note = "An edge two cells wide."

[[nodes]]
id = "plot_surface"
op = "view.plot"
inputs = { value = "surface.map" }
params = { title = "Surface" }
note = "A continuous map: a ramp of colours."

[[nodes]]
id = "plot_mspa"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }
note = "MSPA's classes in GuidosToolbox's colours."
"#,
    },
    Template {
        name: "Fragmentation of a forest",
        build: None,
        about: "one forest map, its density at five scales (FAD-MS) and its connectivity at one (FOS), in GuidosToolbox's colours",
        toml: r#"
format = "terraflux/1"
name = "Fragmentation of a forest"
description = "GuidosToolbox's fragmentation of a forest map: its density at five scales (FAD-MS) and FOS."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.55, weights = [0.3, 0.15, 0.55], seed = 7 }
note = "A map of three classes drawn by chance (1 farming, 2 water, 3 forest): in its place, a raster.read of a map of yours."

[[nodes]]
id = "fad"
op = "fragmentation.fad_multiscale"
inputs = { map = "landscape.map" }
params = { foreground = 3 }
note = "The forest's share (%) in windows of 7, 13, 27, 81 and 243 cells and their average; scales has each, stats the shares of the classes (rare to intact). The foreground is the forest's class: no reclassification needed."

[[nodes]]
id = "fos"
op = "fragmentation.fos"
inputs = { map = "landscape.map" }
params = { foreground = 3, window = 27, method = "FAC", classes = 5 }
note = "The connectivity of the forest at one scale (27 cells), by FAC; summary has GTB's report (ECA, coherence)."

[[nodes]]
id = "plot_fad"
op = "view.plot"
inputs = { value = "fad.map" }
params = { title = "FAD, the average of five scales" }

[[nodes]]
id = "plot_fos"
op = "view.plot"
inputs = { value = "fos.map" }
params = { title = "FOS (FAC, 27 cells)" }
"#,
    },
    Template {
        name: "Patterns and objects",
        build: None,
        about: "one forest map, its simplified pattern (SPA6) and its objects by size (Accounting)",
        toml: r#"
format = "terraflux/1"
name = "Patterns and objects"
description = "GuidosToolbox's SPA and Accounting on a forest map: a coarser MSPA, and the forest's objects by size."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.5, weights = [0.5, 0.5], seed = 5 }
note = "Two classes drawn by chance: 1 the background, 2 the forest (GuidosToolbox's coding already)."

[[nodes]]
id = "spa"
op = "landscape.spa"
inputs = { map = "landscape.map" }
params = { variant = 6 }
note = "Core, islet, edge, perforation and margin: MSPA's main classes at less cost; stats has their areas, porosity the share of core-openings."

[[nodes]]
id = "objects"
op = "landscape.accounting"
inputs = { map = "landscape.map" }
params = { thresholds = [10, 100, 1000, 5000], highlight_largest = true }
note = "The forest's objects in five size classes (up to 10 cells, 100, 1000, 5000, larger), the three largest painted apart; stats counts each class."

[[nodes]]
id = "plot_spa"
op = "view.plot"
inputs = { value = "spa.map" }
params = { title = "SPA6" }

[[nodes]]
id = "plot_objects"
op = "view.plot"
inputs = { value = "objects.map" }
params = { title = "Objects by size" }
"#,
    },
    Template {
        name: "Landscape mosaic",
        build: None,
        about: "a land cover of agriculture, natural and developed land, its landscape mosaic, and the natural \
                land's contagion and entropy",
        toml: r#"
format = "terraflux/1"
name = "Landscape mosaic"
description = "GuidosToolbox's landscape mosaic of a land cover, and the contagion and entropy of its natural land."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.5, weights = [0.35, 0.5, 0.15], seed = 9 }
note = "A land cover drawn by chance: 1 agriculture, 2 natural, 3 developed (the mosaic's coding already). With a map of yours, give the classes of each in the mosaic's settings."

[[nodes]]
id = "mosaic"
op = "fragmentation.landscape_mosaic"
inputs = { map = "cover.map" }
params = { window = 27 }
note = "The mix of the three around each cell (27 cells): which dominates, pairs, mosaics; stats has the share of each class."

[[nodes]]
id = "contagion"
op = "fragmentation.contagion"
inputs = { map = "cover.map" }
params = { foreground = 2, window = 7 }
note = "How much of the natural land's edges join natural land, around each cell."

[[nodes]]
id = "entropy"
op = "fragmentation.entropy"
inputs = { map = "cover.map" }
params = { foreground = 2 }
note = "How mixed the natural land and the rest are over the whole map (%)."

[[nodes]]
id = "plot_mosaic"
op = "view.plot"
inputs = { value = "mosaic.map" }
params = { title = "Landscape mosaic" }

[[nodes]]
id = "plot_contagion"
op = "view.plot"
inputs = { value = "contagion.map" }
params = { title = "Contagion of the natural land" }
"#,
    },
    Template {
        name: "Landscape metrics",
        build: None,
        about: "a land cover's metrics per class (with the share of each as a map) and of the whole landscape, \
                and its diversity in a moving window",
        toml: r#"
format = "terraflux/1"
name = "Landscape metrics"
description = "landscapemetrics' metrics of a land cover: per class, of the landscape, and in a moving window."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, p = 0.5, weights = [0.4, 0.35, 0.25], seed = 12 }
note = "A land cover of three classes drawn by chance: in its place, a raster.read of a map of yours (its cells' size gives the areas: hectares for metres)."

[[nodes]]
id = "classes"
op = "metrics.class"
inputs = { map = "cover.map" }
params = { metrics = ["ca", "pland", "np", "ed", "area_mn", "ai", "cohesion"], show = "pland" }
note = "A row per class: area, share, patches, edge density, mean patch area, aggregation and cohesion. Click a column to sort, choose one to see as bars; map paints the share of each class."

[[nodes]]
id = "landscape"
op = "metrics.landscape"
inputs = { map = "cover.map" }
params = { type = "diversity" }
note = "The diversity of the whole landscape: Shannon's, Simpson's, richness..."

[[nodes]]
id = "window"
op = "metrics.window"
inputs = { map = "cover.map" }
params = { metrics = ["shdi", "ed"], window = 9 }
note = "Shannon's diversity and the edge density of the 9 x 9 cells around each cell, a map each."

[[nodes]]
id = "plot_share"
op = "view.plot"
inputs = { value = "classes.map" }
params = { title = "Share of each class (%)" }

[[nodes]]
id = "plot_window"
op = "view.plot"
inputs = { value = "window.maps" }
params = { title = "Diversity around each cell" }
"#,
    },
    Template {
        name: "Ecological intactness",
        build: None,
        about: "a human footprint drawn by chance, the ecological intactness it leaves (Beyer et al., as plangea), \
                and where restoring would raise it most",
        toml: r#"
format = "terraflux/1"
name = "Ecological intactness"
description = "Beyer et al.'s ecological intactness of a human footprint, and its gradient as cells are restored."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 12, seed = 8 }
note = "A smooth surface from 0 to 1 drawn by chance."

[[nodes]]
id = "footprint"
op = "map.calc"
inputs = { f = "field.map" }
params = { expression = "f * f * 30" }
note = "A human footprint of 0 to 30 (in its place, a raster.read of the Human Footprint Index)."

[[nodes]]
id = "intactness"
op = "restoration.intactness"
inputs = { footprint = "footprint.map" }
params = { radius = 3, cell_size = 1.0 }
note = "The habitat's quality exp(-gamma footprint) of each cell and of those within 3 cells: 1 intact, 0 lost. cell_size: a cell's side in km (a projected map gives it)."

[[nodes]]
id = "reduction"
op = "map.calc"
inputs = { f = "footprint.map" }
params = { expression = "f / 2" }
note = "How much a cell's footprint falls when it is restored: half of it, here."

[[nodes]]
id = "gradient"
op = "restoration.intactness_gradient"
inputs = { footprint = "footprint.map", reduction = "reduction.map" }
params = { radius = 3, cell_size = 1.0 }
note = "How much the total intactness rises per unit of each cell restored: where restoring pays most."

[[nodes]]
id = "plot_intactness"
op = "view.plot"
inputs = { value = "intactness.map" }
params = { title = "Ecological intactness" }

[[nodes]]
id = "plot_gradient"
op = "view.plot"
inputs = { value = "gradient.map" }
params = { title = "Where restoring raises it most" }
"#,
    },
    Template {
        name: "Zonation",
        build: None,
        about: "four species' distributions drawn by chance and a cost, Zonation's priority rank of the cells and \
                its performance curves",
        toml: r#"
format = "terraflux/1"
name = "Zonation"
description = "Zonation 5's priority ranking of four features with a cost: the rank map and the performance curves."

[[nodes]]
id = "bird"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 120, range = 15, seed = 1 }
note = "A species' suitability drawn by chance (in its place, a raster.read of a distribution map)."

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 120, range = 6, seed = 2 }

[[nodes]]
id = "orchid"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 120, range = 25, seed = 3 }

[[nodes]]
id = "wetland"
op = "nlm.fbm"
params = { rows = 100, columns = 120, dimension = 1.4, seed = 4 }

[[nodes]]
id = "features"
op = "stack.of"
inputs = { bird = "bird.map", frog = "frog.map", orchid = "orchid.map", wetland = "wetland.map" }
note = "The features, a layer each."

[[nodes]]
id = "cost"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 120, range = 30, seed = 9 }
note = "What each cell costs (land prices, say): Zonation keeps the cheap cells where they serve as well."

[[nodes]]
id = "zonation"
op = "prioritize.zonation"
inputs = { features = "features.stack", cost = "cost.map" }
params = { rule = "CAZ2", weights = [2.0, 1.0, 1.0, 1.5] }
note = "CAZ2: the cells removed first are those whose loss the features feel least (weights in the order of the features' names). curves: how much of each feature remains as cells are removed."

[[nodes]]
id = "plot_rank"
op = "view.plot"
inputs = { value = "zonation.rank" }
params = { title = "Priority rank" }
"#,
    },
    Template {
        name: "Marxan and irreplaceability",
        build: None,
        about: "four species and a cost: Marxan's reserves for a third of each, how often each cell was chosen, \
                and ProxIRR's irreplaceability of the cells",
        toml: r#"
format = "terraflux/1"
name = "Marxan and irreplaceability"
description = "Marxan 4's reserves from maps (every cell a planning unit), and ProxIRR's irreplaceability."

[[nodes]]
id = "bird"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 8, seed = 1 }
note = "Species' amounts drawn by chance (in their place, raster.read of distribution maps)."

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 4, seed = 2 }

[[nodes]]
id = "orchid"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 12, seed = 3 }

[[nodes]]
id = "wetland"
op = "nlm.fbm"
params = { rows = 40, columns = 50, dimension = 1.4, seed = 4 }

[[nodes]]
id = "features"
op = "stack.of"
inputs = { bird = "bird.map", frog = "frog.map", orchid = "orchid.map", wetland = "wetland.map" }

[[nodes]]
id = "cost"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 15, seed = 9 }
note = "What each cell costs."

[[nodes]]
id = "marxan"
op = "prioritize.marxan"
inputs = { features = "features.stack", cost = "cost.map" }
params = { prop = [0.3, 0.3, 0.3, 0.3], blm = 0.2, num_reps = 10, num_iters = 200000, num_temp = 1000, seed = 7 }
note = "Reserves holding 30% of every species at the least cost and boundary (blm: how much compactness weighs), 10 runs. map: the best run; frequency_map: how often each cell was chosen."

[[nodes]]
id = "irreplaceability"
op = "prioritize.proxirr"
inputs = { features = "features.stack" }
params = { prop = 0.3 }
note = "ProxIRR: beta, the chance each cell is needed to reach the targets; gamma, how many targets need it."

[[nodes]]
id = "plot_best"
op = "view.plot"
inputs = { value = "marxan.map" }
params = { title = "Best reserve" }

[[nodes]]
id = "plot_frequency"
op = "view.plot"
inputs = { value = "marxan.frequency_map" }
params = { title = "Selection frequency" }

[[nodes]]
id = "plot_beta"
op = "view.plot"
inputs = { value = "irreplaceability.beta" }
params = { title = "Irreplaceability (beta)" }
"#,
    },
    Template {
        name: "Conservation planning (prioritizr)",
        build: None,
        about: "four species and a cost: the cheapest set of cells that holds 30% of each, as prioritizr \
                solves it (an integer program, HiGHS), and how much of each it holds",
        toml: r#"
format = "terraflux/1"
name = "Conservation planning (prioritizr)"
description = "prioritizr's minimum set problem: the cheapest cells that meet every species' target, solved exactly."

[[nodes]]
id = "bird"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 8, seed = 1 }
note = "Species' amounts drawn by chance (in their place, raster.read of distribution maps)."

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 4, seed = 2 }

[[nodes]]
id = "orchid"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 12, seed = 3 }

[[nodes]]
id = "wetland"
op = "nlm.fbm"
params = { rows = 30, columns = 40, dimension = 1.4, seed = 4 }

[[nodes]]
id = "features"
op = "stack.of"
inputs = { bird = "bird.map", frog = "frog.map", orchid = "orchid.map", wetland = "wetland.map" }

[[nodes]]
id = "cost"
op = "nlm.gaussian_field"
params = { rows = 30, columns = 40, range = 15, seed = 9 }
note = "What each cell costs (no value: not a planning unit)."

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.map" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.01 }
note = "The cheapest cells holding 30% of each species: prioritizr's model, solved by HiGHS within 1% of the optimum. Try objective = max_features with a budget, or decisions = proportion. coverage: what each species gets."

[[nodes]]
id = "plot_solution"
op = "view.plot"
inputs = { value = "plan.solution" }
params = { title = "Selected cells" }
"#,
    },
    Template {
        name: "Restoration plan (plangea)",
        build: None,
        about: "a forest and its cropland drawn by chance, three species' ranges: where to restore 100 km² for \
                the most reduction of their extinction risk, as plangea (an exact plan, a proven bound)",
        toml: r#"
format = "terraflux/1"
name = "Restoration plan (plangea)"
description = "plangea's restoration problem: the cells to restore for the most reduction of the species' extinction risk."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 10, seed = 5 }
note = "A smooth surface from 0 to 1 drawn by chance."

[[nodes]]
id = "forest"
op = "map.calc"
inputs = { f = "field.map" }
params = { expression = "f * 0.9" }
note = "The share of forest in each cell (in its place, a raster.read of land cover shares)."

[[nodes]]
id = "crops"
op = "map.calc"
inputs = { f = "field.map" }
params = { expression = "1 - f * 0.9" }
note = "The rest is cropland: the land restoration can return to forest."

[[nodes]]
id = "land_cover"
op = "stack.of"
inputs = { forest = "forest.map", crops = "crops.map" }

[[nodes]]
id = "vegetation"
op = "restoration.original_vegetation"
inputs = { land_cover = "land_cover.stack" }
params = { natural = ["forest"], anthropic = ["crops"] }
note = "What the converted land was before (plangea): forest here, and the share of anthropic land."

[[nodes]]
id = "toucan"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 15, seed = 11 }

[[nodes]]
id = "jaguar"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 20, seed = 12 }

[[nodes]]
id = "frog"
op = "nlm.gaussian_field"
params = { rows = 40, columns = 50, range = 6, seed = 13 }

[[nodes]]
id = "ranges"
op = "map.calc"
inputs = { t = "toucan.map" }
params = { expression = "t > 0.4" }
note = "A species' range: the cells above 0 (in its place, raster.read of range maps)."

[[nodes]]
id = "ranges2"
op = "map.calc"
inputs = { j = "jaguar.map" }
params = { expression = "j > 0.5" }

[[nodes]]
id = "ranges3"
op = "map.calc"
inputs = { f = "frog.map" }
params = { expression = "f > 0.3" }

[[nodes]]
id = "species"
op = "stack.of"
inputs = { toucan = "ranges.map", jaguar = "ranges2.map", frog = "ranges3.map" }

[[nodes]]
id = "habitat"
op = "sdm.area_of_habitat"
inputs = { ranges = "species.stack", land_cover = "land_cover.stack", original = "vegetation.original", restore = "vegetation.restore", anthropic = "vegetation.anthropic" }
params = { natural = ["forest"], cell_area = 1.0 }
note = "Every species' habitat now and before conversion (km²), and what restoring a cell gives it."

[[nodes]]
id = "plan"
op = "restoration.plan"
inputs = { restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain" }
params = { target = 100, cell_area = 1.0, z_sd = 0.1 }
note = "The share of every cell to restore, 100 km² in all, for the most reduction of the species' extinction risk (exact within 1e-7; method = steps for plangea's own). Try levels = [0.25, 0.5, 1] for a trade-off curve. summary: the risk reduction and its deviation from z's."

[[nodes]]
id = "plot_plan"
op = "view.plot"
inputs = { value = "plan.restored" }
params = { title = "Share of each cell restored" }
"#,
    },
    Template {
        name: "Species distribution model",
        build: None,
        about: "three predictors drawn by chance, presences drawn where a species finds its conditions, their \
                preparation in one node (a record per cell, the predictors without collinearity, background \
                points, spatial blocks), MaxNet fitted with cross-validation and its map of suitability",
        toml: r#"
format = "terraflux/1"
name = "Species distribution model"
description = "A species distribution model with MaxNet: the records and predictors prepared in one node, the model fitted over spatial folds, the suitability mapped."

[[nodes]]
id = "temperature"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 14, seed = 21 }
note = "A predictor drawn by chance (in its place, raster.read of a climate layer)."

[[nodes]]
id = "rainfall"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 22, seed = 22 }

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 6, seed = 23 }

[[nodes]]
id = "predictors"
op = "stack.of"
inputs = { temperature = "temperature.map", rainfall = "rainfall.map", relief = "relief.map" }

[[nodes]]
id = "suitability"
op = "map.calc"
inputs = { t = "temperature.map", r = "rainfall.map" }
params = { expression = "exp(-((t - 0.6) ^ 2) / 0.02 - ((r - 0.5) ^ 2) / 0.05)" }
note = "Where the species finds its conditions: warm cells of middling rainfall."

[[nodes]]
id = "presences"
op = "sdm.background"
inputs = { env = "predictors.stack", bias = "suitability.map" }
params = { n = 120, seed = 3 }
note = "Presences drawn by the suitability (in their place, vector.read of the records, e.g. a CSV of GBIF)."

[[nodes]]
id = "data"
op = "sdm.prepare"
inputs = { occurrences = "presences.points", env = "predictors.stack" }
params = { clean = false, background = 800, folds = 4, seed = 1 }
note = "The preparation (landschaft's prepare): a record per cell, the predictors without collinearity (VIF 10), 800 background points, 4 folds of spatial blocks. With lon/lat records clean = true runs CoordinateCleaner's tests; area = a distance in metres draws the background near the records. report: what each step removed."

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
note = "MaxNet fitted on every fold and tested on the others (AUC, TSS, Boyce...), then on every record."

[[nodes]]
id = "predict"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }

[[nodes]]
id = "plot"
op = "view.plot"
inputs = { value = "predict.prediction" }
params = { title = "Suitability (MaxNet, cloglog)" }
"#,
    },
    Template {
        name: "Multi-action planning (prioriactions)",
        build: None,
        about: "six planning units, two species and a threat: which units to monitor and where to act against \
                the threat so that both species recover, at the least cost (prioriactions)",
        toml: r#"
format = "terraflux/1"
name = "Multi-action planning (prioriactions)"
description = "prioriactions's minimizeCosts: the cheapest monitoring and actions that meet the species' recovery targets."

[[nodes]]
id = "pu"
op = "table.of"
params = { columns = ["id", "monitoring_cost"], rows = [[1, 2], [2, 1], [3, 3], [4, 1], [5, 2], [6, 2]] }
note = "The planning units and what monitoring them costs (in its place, a table read from a file)."

[[nodes]]
id = "features"
op = "table.of"
params = { columns = ["id", "target_recovery"], rows = [[1, 1], [2, 1]] }
note = "Two species and the recovery each must reach."

[[nodes]]
id = "dist_features"
op = "table.of"
params = { columns = ["pu", "feature", "amount"], rows = [[1, 1, 1], [2, 1, 1], [3, 1, 1], [3, 2, 1], [4, 2, 1], [5, 2, 1], [6, 1, 1]] }
note = "Where each species lives."

[[nodes]]
id = "threats"
op = "table.of"
params = { columns = ["id"], rows = [[1]] }

[[nodes]]
id = "dist_threats"
op = "table.of"
params = { columns = ["pu", "threat", "amount", "action_cost"], rows = [[1, 1, 1, 1], [2, 1, 1, 4], [3, 1, 1, 2], [4, 1, 1, 1], [5, 1, 1, 2]] }
note = "Where the threat is, and what acting against it costs there."

[[nodes]]
id = "plan"
op = "prioritize.actions"
inputs = { pu = "pu.table", features = "features.table", dist_features = "dist_features.table", threats = "threats.table", dist_threats = "dist_threats.table" }
params = { model = "minimizeCosts" }
note = "prioriactions's model, solved exactly. Try model = maximizeBenefits with a budget. summary: every unit's actions; benefit: what each species gets."
"#,
    },
    Template {
        name: "MSPA's edge width",
        build: None,
        about: "one forest map, MSPA with edges of 1 and of 4 cells side by side",
        toml: r#"
format = "terraflux/1"
name = "MSPA's edge width"
description = "How the edge's width changes MSPA's classes: the same forest, edges of 1 and 4 cells."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 300, columns = 300, p = 0.6, weights = [0.4, 0.6], seed = 11 }
note = "Two classes drawn by chance: 1 the background, 2 the forest (MSPA's coding already)."

[[nodes]]
id = "edge_1"
op = "landscape.mspa"
inputs = { map = "landscape.map" }
params = { edge_width = 1 }

[[nodes]]
id = "edge_4"
op = "landscape.mspa"
inputs = { map = "landscape.map" }
params = { edge_width = 4 }

[[nodes]]
id = "plot_1"
op = "view.plot"
inputs = { value = "edge_1.map" }
params = { title = "Edge of 1 cell" }

[[nodes]]
id = "plot_4"
op = "view.plot"
inputs = { value = "edge_4.map" }
params = { title = "Edge of 4 cells" }
"#,
    },
    Template {
        name: "Neutral landscapes",
        build: None,
        about: "the three neutral landscape models side by side: patches of classes, a smooth surface, a fractal \
                one (the viewer colours each as what it is)",
        toml: r#"
format = "terraflux/1"
name = "Neutral landscapes"
description = "NLMR's models: a random cluster (classes), a Gaussian field and a fractional Brownian motion (0 to 1)."

[[nodes]]
id = "clusters"
op = "nlm.random_cluster"
params = { rows = 256, columns = 256, p = 0.4, weights = [0.25, 0.25, 0.5], seed = 4 }
note = "Classes: a colour each."

[[nodes]]
id = "field"
op = "nlm.gaussian_field"
params = { rows = 256, columns = 256, range = 20, seed = 4 }
note = "Values from 0 to 1: a ramp."

[[nodes]]
id = "fractal"
op = "nlm.fbm"
params = { rows = 256, columns = 256, dimension = 1.5, seed = 4 }

[[nodes]]
id = "plot_clusters"
op = "view.plot"
inputs = { value = "clusters.map" }
params = { title = "Random cluster" }

[[nodes]]
id = "plot_field"
op = "view.plot"
inputs = { value = "field.map" }
params = { title = "Gaussian field" }

[[nodes]]
id = "plot_fractal"
op = "view.plot"
inputs = { value = "fractal.map" }
params = { title = "Fractal" }
"#,
    },
    Template {
        name: "Dinamica EGO",
        about: "Dinamica EGO's classic model (Markov rates, weights of evidence, Expander and Patcher) on two dates \
                of land use drawn by chance, run three steps ahead",
        toml: "",
        build: Some(dinamica),
    },
    Template {
        name: "CLUE-S",
        about: "CLUE-S as lulcc's (logistic regressions, a linear demand, CLUE's allocation) on two dates of land use \
                drawn by chance, run three steps ahead",
        toml: "",
        build: Some(clue_s),
    },
];

/// Where the user's own templates are kept (the user's application data): a model file each, named as shown.
pub fn user_dir() -> Option<std::path::PathBuf> {
    crate::app::layout_file().and_then(|f| f.parent().map(|d| d.join("templates")))
}

/// The user's own templates: (name, file), by name.
pub fn user_templates() -> Vec<(String, std::path::PathBuf)> {
    let Some(dir) = user_dir() else { return Vec::new() };
    let Ok(entries) = std::fs::read_dir(&dir) else { return Vec::new() };
    let mut out: Vec<(String, std::path::PathBuf)> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")))
        .map(|p| (p.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default(), p))
        .collect();
    out.sort_by_key(|a| a.0.to_lowercase());
    out
}

/// The paths of a model's files (of the nodes that read and write them, and of its inputs) made absolute from the
/// folder `base` they were relative to, so that the model opens the same files from anywhere.
pub fn absolute_paths(m: &mut Model, base: &std::path::Path) {
    fn nodes(ns: &mut [Node], base: &std::path::Path) {
        for n in ns {
            if terraflux_graph::FILE_OPS.contains(&n.op.as_str())
                && let Some(serde_json::Value::String(p)) = n.params.get_mut("path")
                && !terraflux_graph::is_sent(p)
                && !p.is_empty()
                && std::path::Path::new(p.as_str()).is_relative()
            {
                *p = base.join(p.as_str()).to_string_lossy().into_owned();
            }
            if let Some(b) = &mut n.body {
                nodes(&mut b.nodes, base);
            }
        }
    }
    nodes(&mut m.nodes, base);
    for input in m.inputs.values_mut() {
        if let terraflux_graph::Input::Map { path, .. } | terraflux_graph::Input::Stack { path, .. } = input
            && std::path::Path::new(path.as_str()).is_relative()
        {
            *path = base.join(path.as_str()).to_string_lossy().into_owned();
        }
    }
}

/// A model saved as a template of the user's, named `name` (characters a file name cannot hold replaced); its
/// files' paths made absolute from `base`. The file written.
pub fn save_user_template(
    model: &Model,
    base: Option<&std::path::Path>,
    name: &str,
) -> Result<std::path::PathBuf, String> {
    let dir = user_dir().ok_or("no folder for the application's data")?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let clean: String = name.trim().chars().map(|c| if "\\/:*?\"<>|".contains(c) { '_' } else { c }).collect();
    if clean.is_empty() {
        return Err("a template needs a name".into());
    }
    let mut m = model.clone();
    m.name = Some(clean.clone());
    if let Some(b) = base {
        absolute_paths(&mut m, b);
    }
    let path = dir.join(format!("{clean}.toml"));
    m.save(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_are_sound() {
        let r = terraflux_ops::Registry::standard();
        for t in &TEMPLATES {
            let m = t.model();
            let issues = terraflux_graph::check(&m, &r);
            let errors: Vec<_> = issues.iter().filter(|i| i.severity == terraflux_graph::Severity::Error).collect();
            // (the one for a file to choose excepted)
            assert!(errors.iter().all(|i| i.message.contains("path")), "{}: {errors:?}", t.name);
        }
    }

    #[test]
    fn paths_made_absolute() {
        let mut m = Model::new("t");
        m.nodes.push(Node::new("read", "raster.read").param("path", "data/land.tif"));
        m.nodes.push(Node::new("other", "nlm.fbm").param("path", "keep"));
        m.inputs.insert(
            "x".into(),
            terraflux_graph::Input::Map { path: "x.tif".into(), band: None, nodata: None, note: None },
        );
        let base = std::env::temp_dir();
        absolute_paths(&mut m, &base);
        assert_eq!(m.nodes[0].params["path"], base.join("data/land.tif").to_string_lossy().as_ref());
        assert_eq!(m.nodes[1].params["path"], "keep");
        assert!(
            matches!(&m.inputs["x"], terraflux_graph::Input::Map { path, .. } if path == &base.join("x.tif").to_string_lossy())
        );
    }

    /// The land change templates run: three steps simulated, farming grown.
    #[test]
    fn land_change_templates_run() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        for name in ["Dinamica EGO", "CLUE-S"] {
            let m = TEMPLATES.iter().find(|t| t.name == name).unwrap().model();
            let out = terraflux_exec::Engine::default()
                .run(&m, &Default::default(), &mut Quiet)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let Some(terraflux_ops::Value::Series(maps)) = out.get("simulation.maps") else {
                panic!("{name}: no maps")
            };
            assert_eq!(maps.len(), 3, "{name}");
            let farming = |v: &terraflux_ops::Value| match v {
                terraflux_ops::Value::Map(m) => m.data.iter().filter(|&&x| x == 2.0).count(),
                _ => 0,
            };
            assert!(farming(&maps[2]) > farming(&maps[0]), "{name}: farming grows");
        }
    }

    /// The species distribution model template runs: a map of suitability from 0 to 1.
    #[test]
    fn sdm_template_runs() {
        struct Quiet;
        impl terraflux_exec::Observer for Quiet {}
        let m = TEMPLATES.iter().find(|t| t.name == "Species distribution model").unwrap().model();
        let out = terraflux_exec::Engine::default().run(&m, &Default::default(), &mut Quiet).unwrap();
        let Some(terraflux_ops::Value::Map(p)) = out.get("predict.prediction") else { panic!("no prediction") };
        assert!(p.data.iter().all(|v| (0.0..=1.0).contains(v)));
        let Some(terraflux_ops::Value::Table(t)) = out.get("data.records") else { panic!("no records") };
        assert!(t.columns.contains(&"fold".to_string()) && t.rows.len() > 800);
    }
}
