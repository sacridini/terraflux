//! Models to start from: small graphs that run as they are (their data drawn by neutral landscape models) or
//! once a file is chosen, to see how a flow is built and to check that it works.

use terraflux_graph::{Model, Node, Source};
use terraflux_ops::presets;

/// A model to start from: its name, its group in the menu, what it shows, and the model (written, or built from a
/// preset).
pub struct Template {
    pub name: &'static str,
    pub group: &'static str,
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

const PATTERNS: &str = "Patterns (GuidosToolbox)";
const METRICS: &str = "Landscape metrics";
const CONNECTIVITY: &str = "Connectivity";
const CONSERVATION: &str = "Conservation and restoration";
const SPECIES: &str = "Species";
const LAND_CHANGE: &str = "Land change";
const SURFACES: &str = "Surfaces and uncertainty";

/// The templates' groups, in the menu's order.
pub const GROUPS: [&str; 7] = [PATTERNS, METRICS, CONNECTIVITY, CONSERVATION, SPECIES, LAND_CHANGE, SURFACES];

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
            .param("expression", "if t0 == 1 and nbsum(t0 == 2, 1) >= 2 and suit > 0.45 then 2 else t0")
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
            // the iteration moves by the difference in cells times the scale: lulcc's 0.0005 overshoots on a
            // map of 25,600 cells and never meets the demand; a smaller step does in a few hundred iterations
            allocation: serde_json::json!({"method": "clue", "scale": 0.000005, "max_iter": 5000}),
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

pub const TEMPLATES: [Template; 37] = [
    Template {
        name: "MSPA of a forest",
        group: PATTERNS,
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
        group: PATTERNS,
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
        group: PATTERNS,
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
        group: PATTERNS,
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
        group: PATTERNS,
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
        group: PATTERNS,
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
        group: METRICS,
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
        group: CONSERVATION,
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
        group: CONSERVATION,
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
        group: CONSERVATION,
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
        group: CONSERVATION,
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
        group: CONSERVATION,
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
        group: SPECIES,
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
        group: CONSERVATION,
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
        group: PATTERNS,
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
        group: SURFACES,
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
        group: LAND_CHANGE,
        about: "Dinamica EGO's classic model (Markov rates, weights of evidence, Expander and Patcher) on two dates \
                of land use drawn by chance, run three steps ahead",
        toml: "",
        build: Some(dinamica),
    },
    Template {
        name: "CLUE-S",
        group: LAND_CHANGE,
        about: "CLUE-S as lulcc's (logistic regressions, a linear demand, CLUE's allocation) on two dates of land use \
                drawn by chance, run three steps ahead",
        toml: "",
        build: Some(clue_s),
    },
    Template {
        name: "Forest gain and loss",
        group: PATTERNS,
        build: None,
        about: "a forest at two dates: its simple and morphological change (the real one told from the \
                shifts of its edges) and the change of its fragmentation (FAD at four scales)",
        toml: r#"
format = "terraflux/1"
name = "Forest gain and loss"
description = "GuidosToolbox's change analysis of a forest at two dates: simple change, morphological change (the real one told from the shifts of its edges) and the change of its fragmentation (FAD)."

[[nodes]]
id = "before"
op = "nlm.random_cluster"
params = { rows = 250, columns = 250, p = 0.55, weights = [0.4, 0.6], seed = 27 }
note = "The forest at the first date, drawn by chance: 1 the background, 2 the forest (in its place, a raster.read of a map of yours)."

[[nodes]]
id = "pressure"
op = "nlm.gaussian_field"
params = { rows = 250, columns = 250, range = 30, seed = 28 }
note = "Where clearing presses, from 0 to 1."

[[nodes]]
id = "after"
op = "map.calc"
inputs = { f = "before.map", p = "pressure.map" }
params = { expression = "if f == 2 and p > 0.6 and nbsum(f == 1, 1) >= 1 then 1 else if f == 1 and p < 0.2 and nbsum(f == 2, 1) >= 5 then 2 else f" }
note = "The second date: the forest cleared from its edges where the pressure is high, regrown in the gaps where it is low."

[[nodes]]
id = "simple"
op = "change.simple"
inputs = { before = "before.map", after = "after.map" }
note = "Every cell's change: stable background, gain, loss, stable forest; stats counts them."

[[nodes]]
id = "morphological"
op = "change.morphological"
inputs = { before = "before.map", after = "after.map" }
note = "The same change with the spurious one at the forest's edges (a cell's shift) told apart; stats has the net change and the elasticity."

[[nodes]]
id = "fad"
op = "change.fad"
inputs = { before = "before.map", after = "after.map" }
params = { foreground = 2, windows = [7, 13, 27, 81] }
note = "FAD at four scales at both dates: how many forest cells moved to a more fragmented class or a less fragmented one (stats), and the matrix of the classes."

[[nodes]]
id = "plot_simple"
op = "view.plot"
inputs = { value = "simple.map" }
params = { title = "Simple change" }

[[nodes]]
id = "plot_morphological"
op = "view.plot"
inputs = { value = "morphological.map" }
params = { title = "Morphological change" }
"#,
    },
    Template {
        name: "Patches and their shapes",
        group: METRICS,
        build: None,
        about: "every patch of a land cover measured (area, shape, core, nearest neighbour), GuidosToolbox's \
                parcellation and the contortion of the forest's outlines",
        toml: r#"
format = "terraflux/1"
name = "Patches and their shapes"
description = "Every patch of a land cover measured (landscapemetrics' patch level): its area, shape and core; GuidosToolbox's parcellation (the effective mesh size) and the contortion of the forest's outlines."

[[nodes]]
id = "cover"
op = "nlm.random_cluster"
params = { rows = 200, columns = 200, resolution = 30, p = 0.5, weights = [0.5, 0.3, 0.2], seed = 71 }
note = "A land cover of three classes drawn by chance on cells of 30 m: 1 forest, 2 grassland, 3 farming (in its place, a raster.read of a map of yours)."

[[nodes]]
id = "patches"
op = "metrics.patch"
inputs = { map = "cover.map" }
params = { metrics = ["area", "perim", "shape", "core", "enn"], show = "area" }
note = "A row per patch: its area (ha), perimeter, shape index, core area and the distance to its nearest neighbour of the same class. Sort a column by a click; map paints every patch with its area."

[[nodes]]
id = "parcellation"
op = "landscape.parcellation"
inputs = { map = "cover.map" }
note = "GuidosToolbox's parcellation: per class its parcels, their mean size (APS), the effective mesh size (AWAPS) and the division."

[[nodes]]
id = "contortion"
op = "landscape.contortion"
inputs = { map = "cover.map" }
params = { foreground = 1, min_area = 20 }
note = "How contorted the outline of every forest patch of 20 cells or more is."

[[nodes]]
id = "plot_area"
op = "view.plot"
inputs = { value = "patches.map" }
params = { title = "Area of each patch (ha)" }

[[nodes]]
id = "plot_contortion"
op = "view.plot"
inputs = { value = "contortion.map" }
params = { title = "Contortion of the forest" }
"#,
    },
    Template {
        name: "Kinds of landscapes",
        group: METRICS,
        build: None,
        about: "a land cover cut into squares described by their metrics (spatial signatures), their \
                principal components and the squares grouped into kinds of landscapes (k-means)",
        toml: r#"
format = "terraflux/1"
name = "Kinds of landscapes"
description = "A land cover cut into squares, every square described by its metrics (pylandstats' spatial signatures), their principal components, and the squares grouped by k-means into kinds of landscapes."

[[nodes]]
id = "gradient"
op = "nlm.planar_gradient"
params = { rows = 240, columns = 240, direction = 120, seed = 81 }
note = "A gradient across the map (a climate, an altitude...)."

[[nodes]]
id = "patchiness"
op = "nlm.gaussian_field"
params = { rows = 240, columns = 240, range = 6, seed = 82 }

[[nodes]]
id = "surface"
op = "nlm.merge"
inputs = { primary = "gradient.map", secondary = "patchiness.map" }
params = { scaling = 1.2 }
note = "The gradient with patches on it."

[[nodes]]
id = "cover"
op = "nlm.classify"
inputs = { map = "surface.map" }
params = { weights = [0.3, 0.3, 0.2, 0.2] }
note = "Four classes of 30, 30, 20 and 20 % of the cells, which change along the gradient (in its place, a raster.read of a land cover of yours)."

[[nodes]]
id = "signatures"
op = "metrics.signatures"
inputs = { maps = "cover.classes" }
params = { grid = 30, landscape_metrics = ["shdi", "ed", "lpi", "contag"], class_metrics = ["pland"] }
note = "The map tiled by squares of 30 cells: a row per square, its diversity, edge density, largest patch, contagion and the share of every class."

[[nodes]]
id = "pca"
op = "metrics.pca"
inputs = { table = "signatures.table" }
params = { components = 3 }
note = "The principal components of the metrics: loadings tells what each means, scores places every square on them."

[[nodes]]
id = "kinds"
op = "metrics.cluster"
inputs = { table = "signatures.table" }
params = { k = 4, seed = 0 }
note = "The squares in 4 groups of like metrics (k-means of their z-scores): kinds of landscape."

[[nodes]]
id = "plot_cover"
op = "view.plot"
inputs = { value = "cover.classes" }
params = { title = "Land cover" }
"#,
    },
    Template {
        name: "Network of a forest (MSPA)",
        group: CONNECTIVITY,
        build: None,
        about: "MSPA's cores and bridges as a network: its components, the equivalent connected area and the \
                importance of every link and stepping stone (GuidosToolbox)",
        toml: r#"
format = "terraflux/1"
name = "Network of a forest (MSPA)"
description = "GuidosToolbox's network analysis: MSPA's cores (the nodes) and bridges (the links), their components and the importance of every link and stepping stone."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 250, columns = 250, resolution = 30, p = 0.55, weights = [0.45, 0.55], seed = 19 }
note = "Two classes drawn by chance on cells of 30 m: 1 the background, 2 the forest (MSPA's coding already)."

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "landscape.map" }
params = { edge_width = 2 }
note = "The cores and the bridges between them."

[[nodes]]
id = "components"
op = "networks.gtn_components"
inputs = { map = "mspa.map" }
note = "The networks of cores joined by bridges: summary has their number, the equivalent connected area (ECA) and the coherence (ECA over its largest, %)."

[[nodes]]
id = "importance"
op = "networks.gtn_importance"
inputs = { map = "mspa.map" }
note = "The loss of connectivity (dPC) without each bridge and each stepping-stone core; the top 1, 5 and 10 % in GTB's colours. links and nodes rank them."

[[nodes]]
id = "plot_mspa"
op = "view.plot"
inputs = { value = "mspa.map" }
params = { title = "MSPA" }

[[nodes]]
id = "plot_components"
op = "view.plot"
inputs = { value = "components.map" }
params = { title = "Networks" }

[[nodes]]
id = "plot_importance"
op = "view.plot"
inputs = { value = "importance.map" }
params = { title = "The most important links and nodes" }
"#,
    },
    Template {
        name: "Habitat network (Conefor)",
        group: CONNECTIVITY,
        build: None,
        about: "the forest's patches as a graph: the connectivity of the landscape at three distances (IIC, \
                PC, the equivalent connected area) and the importance of every patch (dPC), as Conefor and \
                Makurhini",
        toml: r#"
format = "terraflux/1"
name = "Habitat network (Conefor)"
description = "A graph of habitat patches: the connectivity of the landscape (IIC, PC and their equivalent connected areas) and the importance of every patch to it, as Conefor and Makurhini."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, resolution = 100, p = 0.4, weights = [0.75, 0.25], seed = 23 }
note = "Two classes drawn by chance on cells of 100 m: 1 the matrix, 2 the forest (in its place, a raster.read of a map of yours)."

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "landscape.map" }
params = { habitat = 2, area_unit = "ha", distance = "edge" }
note = "Every patch of forest a node, its area its attribute; between every two, the gap between their edges (m)."

[[nodes]]
id = "overall"
op = "conn.overall"
inputs = { graph = "graph.graph" }
params = { distance = [500, 1000, 2000], probability = 0.5 }
note = "The landscape's connectivity for species that cross 500 m, 1 and 2 km of matrix with a probability of one half: a row per distance (IIC, PC, the equivalent connected area...)."

[[nodes]]
id = "importance"
op = "conn.node_importance"
inputs = { graph = "graph.graph" }
params = { distance = 1000, probability = 0.5 }
note = "How much of PC (and IIC) is lost without each patch: dPC, and its fractions intra (its own area), flux and connector (a stepping stone)."

[[nodes]]
id = "dpc"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPC" }
note = "Every patch painted with its dPC."

[[nodes]]
id = "connector"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPCconnector" }
note = "And with the part of it as a stepping stone between the others."

[[nodes]]
id = "plot_dpc"
op = "view.plot"
inputs = { value = "dpc.map" }
params = { title = "Importance of each patch (dPC, %)" }

[[nodes]]
id = "plot_connector"
op = "view.plot"
inputs = { value = "connector.map" }
params = { title = "As a stepping stone (dPCconnector)" }
"#,
    },
    Template {
        name: "Circuit connectivity",
        group: CONNECTIVITY,
        build: None,
        about: "a resistance and its best habitat: Circuitscape's currents between the patches (pairwise) \
                and Omniscape's omnidirectional connectivity",
        toml: r#"
format = "terraflux/1"
name = "Circuit connectivity"
description = "Circuitscape's currents between habitat patches, and Omniscape's omnidirectional connectivity, over one resistance."

[[nodes]]
id = "quality"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 20, seed = 31 }
note = "A habitat's quality from 0 to 1 drawn by chance (in its place, a raster.read of a suitability map)."

[[nodes]]
id = "resistance"
op = "map.calc"
inputs = { q = "quality.map" }
params = { expression = "1 + 99 * (1 - q) ^ 3" }
note = "The resistance to movement: 1 in the best habitat, up to 100 where there is none."

[[nodes]]
id = "cores"
op = "map.calc"
inputs = { q = "quality.map" }
params = { expression = "q > 0.8" }
note = "The cores: the best habitat (1), the rest 0."

[[nodes]]
id = "parcels"
op = "landscape.parcellation"
inputs = { map = "cores.map" }
note = "Every patch numbered, of the cores and of the rest."

[[nodes]]
id = "patches"
op = "map.calc"
inputs = { c = "cores.map", p = "parcels.parcels" }
params = { expression = "if c == 1 then p else 0" }
note = "The cores' patches by number: Circuitscape's focal regions (0 elsewhere)."

[[nodes]]
id = "pairwise"
op = "circuit.pairwise"
inputs = { habitat = "resistance.map", points = "patches.map" }
params = { cum_map = true, max_map = true }
note = "Circuitscape's pairwise mode: the effective resistance between every pair of patches (resistances), the current of all pairs summed (cum): where animals moving between them would pass."

[[nodes]]
id = "omniscape"
op = "circuit.omniscape"
inputs = { resistance = "resistance.map" }
params = { radius = 12, block_size = 5, flow_potential = true, normalized_current = true }
note = "Omniscape: currents from every cell to those within 12 cells (a target every 5 cells), the sources from the resistance. normalized: the current over what it would be without resistances (above 1, channelled; below, impeded)."

[[nodes]]
id = "plot_resistance"
op = "view.plot"
inputs = { value = "resistance.map" }
params = { title = "Resistance" }

[[nodes]]
id = "plot_current"
op = "view.plot"
inputs = { value = "pairwise.cum" }
params = { title = "Current between the cores (Circuitscape)" }

[[nodes]]
id = "plot_omniscape"
op = "view.plot"
inputs = { value = "omniscape.normalized" }
params = { title = "Normalised current (Omniscape)" }
"#,
    },
    Template {
        name: "Least costs and random walks",
        group: CONNECTIVITY,
        build: None,
        about: "a resistance and two places: the accumulated cost of the cheapest routes, randomised \
                shortest paths and a random walk between them (gdistance)",
        toml: r#"
format = "terraflux/1"
name = "Least costs and random walks"
description = "gdistance's ways across a landscape between two places: the accumulated cost of the cheapest routes, and the passages of walkers that know the way less and less well (randomised shortest paths, then a random walk)."

[[nodes]]
id = "roughness"
op = "nlm.fbm"
params = { rows = 100, columns = 100, dimension = 1.5, seed = 91 }
note = "How hard each cell is to cross, 0 to 1 (in its place, a raster.read of a resistance)."

[[nodes]]
id = "conductance"
op = "map.calc"
inputs = { r = "roughness.map" }
params = { expression = "1 / (1 + 20 * r ^ 2)" }
note = "How easily it is crossed: 1 over the resistance."

[[nodes]]
id = "transition"
op = "cost.transition"
inputs = { map = "conductance.map" }
params = { function = "mean", directions = 8 }
note = "The connection of every cell with its 8 neighbours: the mean of their conductances."

[[nodes]]
id = "corrected"
op = "cost.geo_correction"
inputs = { transition = "transition.transition" }
params = { type = "c" }
note = "Corrected for the distances between the cells' centres (the diagonals longer)."

[[nodes]]
id = "near_a"
op = "nlm.distance_gradient"
params = { rows = 100, columns = 100, origin = [3, 8, 45, 55], nlmr_compat = false, rescale = false }

[[nodes]]
id = "origin"
op = "map.calc"
inputs = { d = "near_a.map" }
params = { expression = "d == 0" }
note = "Where the walkers start: a strip in the west (in its place, a raster of a reserve, or vector.read of points)."

[[nodes]]
id = "near_b"
op = "nlm.distance_gradient"
params = { rows = 100, columns = 100, origin = [93, 98, 45, 55], nlmr_compat = false, rescale = false }

[[nodes]]
id = "goal"
op = "map.calc"
inputs = { d = "near_b.map" }
params = { expression = "d == 0" }
note = "Where they go: a strip in the east."

[[nodes]]
id = "cost"
op = "cost.acc_cost"
inputs = { transition = "corrected.transition", origins = "origin.map" }
note = "The cost of the cheapest route from the origin to every cell."

[[nodes]]
id = "rsp"
op = "cost.passage"
inputs = { transition = "corrected.transition", origins = "origin.map", goals = "goal.map" }
params = { theta = 0.05 }
note = "Randomised shortest paths (theta 0.05): walkers that mostly take the cheap ways, spread over several. Try theta = 1 (near the least-cost path) or 0.001 (near a random walk)."

[[nodes]]
id = "random_walk"
op = "cost.passage"
inputs = { transition = "transition.transition", origins = "origin.map", goals = "goal.map" }
note = "Without theta, a random walk: the current from the origin to the goal (Circuitscape's), over the transition before the correction."

[[nodes]]
id = "plot_cost"
op = "view.plot"
inputs = { value = "cost.map" }
params = { title = "Accumulated cost from the origin" }

[[nodes]]
id = "plot_rsp"
op = "view.plot"
inputs = { value = "rsp.map" }
params = { title = "Randomised shortest paths" }

[[nodes]]
id = "plot_walk"
op = "view.plot"
inputs = { value = "random_walk.map" }
params = { title = "Random walk" }
"#,
    },
    Template {
        name: "Restoring a corridor",
        group: CONNECTIVITY,
        build: None,
        about: "two areas of habitat: the least-cost path between them across a resistance and what \
                restoring it gains in connectivity (GuidosToolbox's Restoration Planner)",
        toml: r#"
format = "terraflux/1"
name = "Restoring a corridor"
description = "GuidosToolbox's Restoration Planner: a resistance from the distance to the habitat, the least-cost path between two areas, and what restoring it gains."

[[nodes]]
id = "landscape"
op = "nlm.random_cluster"
params = { rows = 150, columns = 150, p = 0.4, weights = [0.8, 0.2], seed = 17 }
note = "Two classes drawn by chance: 1 the background, 2 the habitat (GuidosToolbox's coding already)."

[[nodes]]
id = "near_a"
op = "nlm.distance_gradient"
params = { rows = 150, columns = 150, origin = [5, 20, 5, 20], nlmr_compat = false, rescale = false }
note = "The distance to a square in the north-west: 0 inside it."

[[nodes]]
id = "near_b"
op = "nlm.distance_gradient"
params = { rows = 150, columns = 150, origin = [130, 145, 130, 145], nlmr_compat = false, rescale = false }
note = "And to one in the south-east."

[[nodes]]
id = "a"
op = "map.calc"
inputs = { d = "near_a.map" }
params = { expression = "d == 0" }
note = "Area A (in its place, a raster of a reserve)."

[[nodes]]
id = "b"
op = "map.calc"
inputs = { d = "near_b.map" }
params = { expression = "d == 0" }
note = "Area B."

[[nodes]]
id = "habitat"
op = "map.calc"
inputs = { h = "landscape.map", a = "a.map", b = "b.map" }
params = { expression = "if a or b then 2 else h" }
note = "The two areas are habitat too."

[[nodes]]
id = "resistance"
op = "restoration.resistance"
inputs = { map = "habitat.map" }
note = "GTB's resistance: 2 in the habitat, growing with the distance from it up to 100."

[[nodes]]
id = "path"
op = "restoration.least_cost_path"
inputs = { resistance = "resistance.map", a = "a.map", b = "b.map" }
note = "The cheapest way from A to B: cost is the cost from A plus that from B (its least along the path); stats has its length and expense."

[[nodes]]
id = "corridor"
op = "restoration.corridor"
inputs = { resistance = "resistance.map", a = "a.map", b = "b.map", habitat = "habitat.map" }
note = "The path's background restored: stats has the cells restored and the ECA and coherence gained by it."

[[nodes]]
id = "plot_cost"
op = "view.plot"
inputs = { value = "path.cost" }
params = { title = "Cost from A plus from B" }

[[nodes]]
id = "shown"
op = "map.calc"
inputs = { h = "habitat.map", p = "corridor.path" }
params = { expression = "if p == 1 then 3 else h" }
note = "The corridor (3) over the background (1) and the habitat (2); corridor.restored is the habitat once its background is restored."

[[nodes]]
id = "plot_corridor"
op = "view.plot"
inputs = { value = "shown.map" }
params = { title = "The corridor (3)" }
"#,
    },
    Template {
        name: "Irreplaceability of a plan",
        group: CONSERVATION,
        build: None,
        about: "prioritizr's plan for four species evaluated (what it holds, Ferrier's irreplaceability, the \
                rarity of its cells) and a portfolio of ten near-optimal plans",
        toml: r#"
format = "terraflux/1"
name = "Irreplaceability of a plan"
description = "prioritizr's plan for four species and its evaluation: how much of each it holds, Ferrier's irreplaceability and the rarity of the cells chosen, and a portfolio of near-optimal plans."

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
note = "What each cell costs."

[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.map" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.01 }
note = "The cheapest cells that hold 30 % of every species (within 1 % of the optimum)."

[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = { features = "features.stack", cost = "cost.map", solution = "plan.solution" }
params = { relative_targets = 0.3 }
note = "prioritizr's eval_*: summary (the cost, the cells, the boundary), representation and coverage of every species, and per cell chosen its rarity-weighted richness and Ferrier's irreplaceability (ferrier: a layer per species and the total)."

[[nodes]]
id = "portfolio"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.map" }
params = { objective = "min_set", relative_targets = 0.3, portfolio = "gap", pool_gap = 0.1, number_solutions = 10 }
note = "Ten plans within 10 % of the best: frequency, how often each cell is chosen; the cells always chosen are hard to replace."

[[nodes]]
id = "plot_solution"
op = "view.plot"
inputs = { value = "plan.solution" }
params = { title = "The plan" }

[[nodes]]
id = "plot_ferrier"
op = "view.plot"
inputs = { value = "evaluation.ferrier" }
params = { title = "Ferrier's irreplaceability" }

[[nodes]]
id = "plot_frequency"
op = "view.plot"
inputs = { value = "portfolio.frequency" }
params = { title = "Chosen in how many of ten plans" }
"#,
    },
    Template {
        name: "A species in a warmer climate",
        group: SPECIES,
        build: None,
        about: "a MaxNet model explained (response curves, the importance of the predictors) and projected \
                to a climate 2 °C warmer: the gains, the losses and where it extrapolates (MESS)",
        toml: r#"
format = "terraflux/1"
name = "A species in a warmer climate"
description = "A species distribution model explained and projected: its response curves and the importance of its predictors, its suitability today and in a climate 2 degrees warmer, and where that projection extrapolates (MESS)."

[[nodes]]
id = "field_t"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 14, seed = 21 }

[[nodes]]
id = "temperature"
op = "map.calc"
inputs = { f = "field_t.map" }
params = { expression = "14 + f * 12" }
note = "The mean temperature, from 14 to 26 °C (in its place, raster.read of a climate layer)."

[[nodes]]
id = "field_r"
op = "nlm.gaussian_field"
params = { rows = 60, columns = 80, range = 22, seed = 22 }

[[nodes]]
id = "rainfall"
op = "map.calc"
inputs = { f = "field_r.map" }
params = { expression = "800 + f * 1600" }
note = "The rainfall, from 800 to 2400 mm."

[[nodes]]
id = "predictors"
op = "stack.of"
inputs = { temperature = "temperature.map", rainfall = "rainfall.map" }

[[nodes]]
id = "suitability"
op = "map.calc"
inputs = { t = "temperature.map", r = "rainfall.map" }
params = { expression = "exp(-((t - 19) ^ 2) / 3 - ((r - 1600) ^ 2) / 150000)" }
note = "Where the species finds its conditions: about 19 °C and 1600 mm."

[[nodes]]
id = "presences"
op = "sdm.background"
inputs = { env = "predictors.stack", bias = "suitability.map" }
params = { n = 150, seed = 3 }
note = "Presences drawn by the suitability (in their place, vector.read of the records)."

[[nodes]]
id = "data"
op = "sdm.prepare"
inputs = { occurrences = "presences.points", env = "predictors.stack" }
params = { clean = false, background = 1000, folds = 4, seed = 1 }
note = "A record per cell, 1000 background points, 4 folds of spatial blocks."

[[nodes]]
id = "fit"
op = "sdm.fit"
inputs = { records = "data.records" }
note = "MaxNet fitted over the folds (stats: AUC, TSS, Boyce...)."

[[nodes]]
id = "curves"
op = "sdm.response_curves"
inputs = { model = "fit.model", records = "data.records" }
note = "How the suitability answers each predictor, the other at its mean: a table of a column of values and one of suitability per predictor."

[[nodes]]
id = "importance"
op = "sdm.importance"
inputs = { model = "fit.model", records = "data.records" }
note = "How much the AUC falls when each predictor is shuffled: the most important first."

[[nodes]]
id = "today"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "data.env" }
note = "The suitability today."

[[nodes]]
id = "warmer"
op = "map.calc"
inputs = { t = "temperature.map" }
params = { expression = "t + 2" }
note = "The temperature in a climate 2 °C warmer (in its place, raster.read of a scenario's layer)."

[[nodes]]
id = "future"
op = "stack.set"
inputs = { stack = "data.env", map = "warmer.map" }
params = { name = "temperature" }
note = "The predictors of the future: the warmer temperature in place of today's."

[[nodes]]
id = "tomorrow"
op = "sdm.maxnet_predict"
inputs = { model = "fit.model", env = "future.stack" }
note = "The suitability in the warmer climate."

[[nodes]]
id = "shift"
op = "map.calc"
inputs = { a = "today.prediction", b = "tomorrow.prediction" }
params = { expression = "b - a" }
note = "What changes: gains (positive) and losses (negative)."

[[nodes]]
id = "mess"
op = "sdm.mess"
inputs = { reference = "data.env", env = "future.stack" }
note = "Elith's MESS: how like the conditions of today the future's are; negative where the model extrapolates, and is not to be trusted."

[[nodes]]
id = "plot_today"
op = "view.plot"
inputs = { value = "today.prediction" }
params = { title = "Suitability today" }

[[nodes]]
id = "plot_tomorrow"
op = "view.plot"
inputs = { value = "tomorrow.prediction" }
params = { title = "Suitability 2 °C warmer" }

[[nodes]]
id = "plot_shift"
op = "view.plot"
inputs = { value = "shift.map" }
params = { title = "Gains and losses" }

[[nodes]]
id = "plot_mess"
op = "view.plot"
inputs = { value = "mess.result" }
params = { title = "MESS (negative: extrapolation)" }
"#,
    },
    Template {
        name: "Drivers of land change",
        group: LAND_CHANGE,
        build: None,
        about: "two dates of land use and their drivers: the Markov chain, Dinamica EGO's weights of \
                evidence and lulcc's logistic regressions, each with its map of probability",
        toml: r#"
format = "terraflux/1"
name = "Drivers of land change"
description = "What explains a change: the Markov chain of two dates, the weights of evidence of the change on its drivers (Dinamica EGO) and the logistic regressions of every class (lulcc), each as maps of probability."

[[nodes]]
id = "land_2000"
op = "nlm.random_cluster"
params = { rows = 160, columns = 160, p = 0.5, weights = [0.55, 0.35, 0.10], seed = 11 }
note = "Land use drawn by chance: 1 forest, 2 farming, 3 water. In its place, a raster.read of a map of yours."

[[nodes]]
id = "aptitude"
op = "nlm.gaussian_field"
params = { rows = 160, columns = 160, range = 25, seed = 12 }
note = "A driver: how suited each cell is to farming (0 to 1)."

[[nodes]]
id = "roughness"
op = "nlm.fbm"
params = { rows = 160, columns = 160, dimension = 1.4, seed = 13 }
note = "A driver of no effect on the change, as real models have some."

[[nodes]]
id = "land_2010"
op = "map.calc"
inputs = { t0 = "land_2000.map", suit = "aptitude.map" }
params = { expression = "if t0 == 1 and nbsum(t0 == 2, 1) >= 3 and suit > 0.6 then 2 else t0" }
note = "Ten years later: farming grown into the forest beside it, where it is suited."

[[nodes]]
id = "to_farming"
op = "dynamic.distance_to"
inputs = { landscape = "land_2000.map" }
params = { classes = 2, unit = "cells" }
note = "A driver of the map itself: the distance to farming."

[[nodes]]
id = "drivers"
op = "stack.of"
inputs = { aptitude = "aptitude.map", roughness = "roughness.map", to_farming = "to_farming.distance" }
note = "The drivers, a layer each."

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = { t0 = "land_2000.map", t1 = "land_2010.map", steps = 1 }
note = "The transition probabilities of the period: which share of each class went to each other."

[[nodes]]
id = "weights"
op = "woe.fit"
inputs = { t0 = "land_2000.map", t1 = "land_2010.map", drivers = "drivers.stack", markov = "markov.markov" }
params = { ranges = { aptitude = 0.05, roughness = 0.05, to_farming = 1 }, fix_abnormal = true }
note = "Dinamica EGO's weights of evidence of every transition (here forest to farming) on the ranges of every driver: table is Dinamica's report, a positive weight favouring the change, a negative one keeping it away."

[[nodes]]
id = "woe_probability"
op = "woe.probability"
inputs = { model = "weights.model", landscape = "land_2010.map", drivers = "drivers.stack" }
note = "The probability of the change in every cell of forest, from the weights."

[[nodes]]
id = "regressions"
op = "glm.fit"
inputs = { landscape = "land_2010.map", drivers = "drivers.stack" }
note = "A logistic regression of the presence of every class on the drivers (R's glm, as lulcc)."

[[nodes]]
id = "glm_probability"
op = "glm.predict"
inputs = { model = "regressions.model", drivers = "drivers.stack", landscape = "land_2010.map" }
note = "The probability of every class in every cell, a layer each."

[[nodes]]
id = "plot_woe"
op = "view.plot"
inputs = { value = "woe_probability.probability" }
params = { title = "Forest to farming (weights of evidence)" }

[[nodes]]
id = "plot_glm"
op = "view.plot"
inputs = { value = "glm_probability.suitability" }
params = { title = "Every class (logistic regression)" }
"#,
    },
    Template {
        name: "A city that grows (a loop)",
        group: LAND_CHANGE,
        build: None,
        about: "a cellular automaton written in map algebra: a city grown over 25 steps of a loop into its \
                best suited neighbours, never into water or the mountains",
        toml: r#"
format = "terraflux/1"
name = "A city that grows (a loop)"
description = "A cellular automaton written by hand: a loop of 25 steps where a city grows into the cells next to it, the best suited first, never into water or the mountains."

[[nodes]]
id = "relief"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 30, nugget = 0, seed = 41 }
note = "A relief from 0 to 1 (in its place, a raster.read of an elevation model)."

[[nodes]]
id = "attraction"
op = "nlm.gaussian_field"
params = { rows = 150, columns = 150, range = 20, seed = 42 }
note = "How attractive each place is (the jobs, the roads...): 0 to 1."

[[nodes]]
id = "centre"
op = "nlm.distance_gradient"
params = { rows = 150, columns = 150, origin = [72, 78, 72, 78], nlmr_compat = false, rescale = false }
note = "The distance to the old town in the middle."

[[nodes]]
id = "suitability"
op = "map.calc"
inputs = { a = "attraction.map", d = "centre.map", r = "relief.map" }
params = { expression = "if r < 0.3 or r > 0.75 then null else 0.6 * a + 0.4 * (1 - d / 106)" }
note = "Where a city would rather grow: attractive and near the centre; no value (never) in the water (the lowest land) and on the mountains (the highest)."

[[nodes]]
id = "town"
op = "map.calc"
inputs = { d = "centre.map", suit = "suitability.map" }
params = { expression = "if isnull(suit) then null else d == 0" }
note = "The city at the start: the old town (1), the rest 0."

[[nodes]]
id = "growth"
op = "control.repeat"
inputs = { times = 25, city = "town.map" }
note = "The loop: every step its body runs with @city, the city of the step before, and @step; collect keeps every step's map (growth.maps), carry passes the city on (growth.city is the last)."

[nodes.body]
carry = { city = "grow.map" }
collect = { maps = "grow.map" }

[[nodes.body.nodes]]
id = "grow"
op = "map.calc"
inputs = { c = "@city", suit = "suitability.map", step = "@step" }
params = { expression = "if c == 0 and nbsum(c == 1, 1) >= 2 and suit > 0.8 - step * 0.008 then 1 else c" }
note = "A cell becomes city when two of its neighbours are, if it is suited enough; the bar falls a little every step, so the best land goes first."

[[nodes]]
id = "plot_growth"
op = "view.plot"
inputs = { value = "growth.maps" }
params = { title = "The city, step by step" }

[[nodes]]
id = "plot_city"
op = "view.plot"
inputs = { value = "growth.city" }
params = { title = "The city after 25 steps" }

[[nodes]]
id = "plot_suitability"
op = "view.plot"
inputs = { value = "suitability.map" }
params = { title = "Suitability" }
"#,
    },
    Template {
        name: "Farmers on their parcels (agents)",
        group: LAND_CHANGE,
        build: None,
        about: "a farmer per parcel in a loop of 20 years: rules for the wealth, from the fertility and a \
                falling price; the parcels left as farmers go broke",
        toml: r#"
format = "terraflux/1"
name = "Farmers on their parcels (agents)"
description = "Agents in a loop: a farmer per parcel, whose wealth follows the fertility of the parcel and a price that falls, who gives up farming when broke."

[[nodes]]
id = "mosaic"
op = "nlm.mosaic_tess"
params = { rows = 120, columns = 120, germs = 60, seed = 51 }
note = "A Voronoi mosaic: 60 cells of a random value each."

[[nodes]]
id = "parcels"
op = "map.calc"
inputs = { m = "mosaic.map" }
params = { expression = "1 + floor(m * 100000)" }
note = "Each Voronoi cell a parcel with an id (in its place, a raster.read of a cadastre's parcel ids)."

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 120, columns = 120, range = 25, seed = 52 }
note = "The soil's fertility, 0 to 1."

[[nodes]]
id = "farms"
op = "agents.parcels"
inputs = { parcels = "parcels.map", fertility = "fertility.map" }
note = "An agent per parcel: its id, its cells and the mean fertility of its land."

[[nodes]]
id = "start"
op = "agents.update"
inputs = { agents = "farms.agents" }
params = { rules = "wealth = 20\nfarming = 1" }
note = "Every farmer starts with a wealth of 20, farming."

[[nodes]]
id = "years"
op = "control.repeat"
inputs = { times = 20, farmers = "start.agents" }
note = "20 years: @farmers are the farmers of the year before; summary collects the totals of every year, maps the parcels still farmed."

[nodes.body]
carry = { farmers = "year.agents" }
collect = { summary = "totals.table", maps = "farmed.map" }

[[nodes.body.nodes]]
id = "year"
op = "agents.update"
inputs = { agents = "@farmers", step = "@step" }
params = { rules = "price = 10 - step * 0.25\nincome = if farming == 1 then fertility * price * 2 else 1\nwealth = wealth + income - 7\nfarming = if wealth > 0 then farming else 0" }
note = "The rules of a year, in order: the price falls, the farmers earn by the fertility of their parcel, pay 7, and those broke stop farming (and earn 1 elsewhere)."

[[nodes.body.nodes]]
id = "totals"
op = "agents.summary"
inputs = { agents = "year.agents" }
note = "How many farmers and the sum and mean of every attribute."

[[nodes.body.nodes]]
id = "farmed"
op = "agents.map"
inputs = { agents = "year.agents", parcels = "parcels.map" }
params = { column = "farming" }
note = "Whether each parcel is still farmed, painted on it."

[[nodes]]
id = "wealth"
op = "agents.map"
inputs = { agents = "years.farmers", parcels = "parcels.map" }
params = { column = "wealth" }
note = "The farmers after 20 years (years.farmers): their wealth painted on their parcels."

[[nodes]]
id = "plot_wealth"
op = "view.plot"
inputs = { value = "wealth.map" }
params = { title = "Wealth after 20 years" }

[[nodes]]
id = "plot_fertility"
op = "view.plot"
inputs = { value = "fertility.map" }
params = { title = "Fertility" }

[[nodes]]
id = "plot_farming"
op = "view.plot"
inputs = { value = "years.maps" }
params = { title = "Parcels still farmed, year by year" }
"#,
    },
    Template {
        name: "CRAFTY: farmers and foresters",
        group: LAND_CHANGE,
        build: None,
        about: "CRAFTY's competition for land: farmers and foresters taking the cells where their food or \
                timber is wanted, as the demand for food grows over 20 steps",
        toml: r#"
format = "terraflux/1"
name = "CRAFTY: farmers and foresters"
description = "CRAFTY's competition for land: farmers and foresters produce food and timber from the capitals of the cells, and over 20 steps take the land where their services are wanted most, as the demand for food grows."

[[nodes]]
id = "fertility"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 15, seed = 101 }
note = "A capital, 0 to 1: the soil's fertility (in its place, raster.read of a capital's map)."

[[nodes]]
id = "wood"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 25, seed = 102 }
note = "Another: the forest's growing stock."

[[nodes]]
id = "capitals"
op = "stack.of"
inputs = { fertility = "fertility.map", wood = "wood.map" }
note = "The capitals, a layer each by name."

[[nodes]]
id = "start"
op = "nlm.random_cluster"
params = { rows = 100, columns = 100, p = 0.5, weights = [0.5, 0.5], seed = 103 }
note = "Who manages each cell at the start: 1 farmers, 2 foresters, drawn by chance."

[[nodes]]
id = "demand"
op = "table.of"
params = { columns = ["food", "timber"], rows = [[1500, 1500], [1600, 1500], [1700, 1500], [1800, 1500], [1900, 1500], [2000, 1500], [2100, 1500], [2200, 1500], [2300, 1500], [2400, 1500], [2500, 1500], [2600, 1500], [2700, 1500], [2800, 1500], [2900, 1500], [3000, 1500], [3100, 1500], [3200, 1500], [3300, 1500], [3400, 1500]] }
note = "The demand for food and timber at every step (a row each): food grows, timber stays."

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = 20, land = "start.map" }
note = "20 steps: @land, who manages each cell, passed on; cover (the land cover of every step) and info (the supply and demand) collected."

[nodes.body]
carry = { land = "compete.land" }
collect = { cover = "compete.cover", info = "compete.info" }

[[nodes.body.nodes]]
id = "compete"
op = "crafty.allocate"
inputs = { land = "@land", capitals = "capitals.stack", demand = "demand.table", step = "@step", seed = 7 }
params = { search = 0.2, giving_up_rate = 0.2, afts = [
  { name = "farmer", class = 1, production = { food = 1.0 }, sensitivity = { fertility = 2.0 }, giving_up = 0.05, giving_in = 0.1 },
  { name = "forester", class = 2, production = { timber = 1.0 }, sensitivity = { wood = 1.0 }, giving_up = 0.05, giving_in = 0.1 },
] }
note = "A step of CRAFTY: what each AFT would produce from the capitals (production x capital^sensitivity), the value of the demand left unmet, managers giving up below their threshold, and a share of the cells (search) offered to the most competitive."

[[nodes]]
id = "plot_cover"
op = "view.plot"
inputs = { value = "simulation.cover" }
params = { title = "Farmers (1), foresters (2), unmanaged (0), step by step" }
"#,
    },
    Template {
        name: "Land change analysis",
        group: LAND_CHANGE,
        build: None,
        about: "three dates of land use compared: the transitions, the components of the change (Pontius) \
                and the intensity analysis (as OpenLand)",
        toml: r#"
format = "terraflux/1"
name = "Land change analysis"
description = "Three dates of land use compared: the transitions, the components of the change (Pontius) and the intensity analysis (Aldwaik & Pontius, as OpenLand)."

[[nodes]]
id = "land_2000"
op = "nlm.random_cluster"
params = { rows = 160, columns = 160, resolution = 100, p = 0.5, weights = [0.55, 0.35, 0.10], seed = 11 }
note = "Land use drawn by chance on cells of 100 m: 1 forest, 2 farming, 3 water. In its place, a raster.read of a map of yours."

[[nodes]]
id = "aptitude"
op = "nlm.gaussian_field"
params = { rows = 160, columns = 160, resolution = 100, range = 25, seed = 12 }
note = "How suited each cell is to farming."

[[nodes]]
id = "land_2010"
op = "map.calc"
inputs = { t0 = "land_2000.map", suit = "aptitude.map" }
params = { expression = "if t0 == 1 and nbsum(t0 == 2, 1) >= 3 and suit > 0.65 then 2 else t0" }
note = "Ten years later: farming grown into the forest beside it, where it is suited."

[[nodes]]
id = "land_2020"
op = "map.calc"
inputs = { t1 = "land_2010.map", suit = "aptitude.map" }
params = { expression = "if t1 == 1 and nbsum(t1 == 2, 1) >= 3 and suit > 0.5 then 2 else if t1 == 2 and suit < 0.2 then 1 else t1" }
note = "Ten more: farming grows faster, and the least suited farms go back to forest."

[[nodes]]
id = "series"
op = "stack.of"
inputs = { y2000 = "land_2000.map", y2010 = "land_2010.map", y2020 = "land_2020.map" }
note = "The maps, a layer per date."

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "land_2000.map", to = "land_2020.map" }
note = "The transition matrix of 2000 to 2020: the cells of each class (rows) in each class twenty years later (columns)."

[[nodes]]
id = "transitions"
op = "change.transitions"
inputs = { maps = "series.stack" }
params = { times = [2000, 2010, 2020] }
note = "Every transition of every period in km² (areas: each class at each date), as OpenLand's contingency table."

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "land_2000.map", to = "land_2020.map" }
note = "Pontius's components: of each class its gain, loss and persistence; overall the change made of quantity, exchange and shift."

[[nodes]]
id = "intensity"
op = "change.intensity"
inputs = { maps = "series.stack" }
params = { times = [2000, 2010, 2020], gain = 2, loss = 1 }
note = "The intensity analysis: whether each period changed faster than the uniform rate (interval), which classes gained and lost actively (category), and which classes farming took (transition_gain) and forest went to (transition_loss)."

[[nodes]]
id = "plot_series"
op = "view.plot"
inputs = { value = "series.stack" }
params = { title = "Land use in 2000, 2010 and 2020" }
"#,
    },
    Template {
        name: "Validating a simulation",
        group: LAND_CHANGE,
        build: None,
        about: "a simulated map against the observed one: agreement and figure of merit, Dinamica EGO's \
                fuzzy similarity, the three-map comparison at coarser resolutions and MOLUSCE's kappas",
        toml: r#"
format = "terraflux/1"
name = "Validating a simulation"
description = "A simulated map compared with the observed one, both from the same start: agreement and figure of merit, Dinamica EGO's fuzzy similarity, the three-map comparison at coarser resolutions (Pontius) and MOLUSCE's kappas."

[[nodes]]
id = "start"
op = "nlm.random_cluster"
params = { rows = 160, columns = 160, p = 0.5, weights = [0.55, 0.35, 0.10], seed = 11 }
note = "The map both start from: 1 forest, 2 farming, 3 water (in its place, a raster.read of the map of the first date)."

[[nodes]]
id = "aptitude"
op = "nlm.gaussian_field"
params = { rows = 160, columns = 160, range = 25, seed = 12 }

[[nodes]]
id = "observed"
op = "map.calc"
inputs = { t0 = "start.map", suit = "aptitude.map" }
params = { expression = "if t0 == 1 and nbsum(t0 == 2, 1) >= 3 and suit > 0.6 then 2 else t0" }
note = "What happened: farming grew into the forest beside it, where it was suited (in its place, the map of the second date)."

[[nodes]]
id = "simulated"
op = "map.calc"
inputs = { t0 = "start.map", suit = "aptitude.map" }
params = { expression = "if t0 == 1 and nbsum(t0 == 2, 1) >= 3 and nbmean(suit, 4) > 0.6 then 2 else t0" }
note = "What a model said: much the same, but from a blurred suitability (its mean over 9 x 9 cells) (in its place, a model's simulated map)."

[[nodes]]
id = "agreement"
op = "validate.agreement"
inputs = { start = "start.map", observed = "observed.map", simulated = "simulated.map" }
note = "The share of cells right, and the figure of merit of the change: hits over hits, misses, false alarms and wrong hits."

[[nodes]]
id = "similarity"
op = "validate.similarity"
inputs = { start = "start.map", observed = "observed.map", simulated = "simulated.map" }
params = { windows = [1, 3, 5, 9, 15] }
note = "Dinamica EGO's fuzzy similarity of the changes in windows of growing size: a model right near the place scores better in the wider windows."

[[nodes]]
id = "three_map"
op = "validate.three_map"
inputs = { t0 = "start.map", t1 = "observed.map", simulated = "simulated.map" }
params = { factors = [1, 2, 4, 8, 16] }
note = "The three-map comparison (lulcc): misses, hits, wrong hits and false alarms (budget) and the figure of merit at blocks of 1 to 16 cells."

[[nodes]]
id = "kappa"
op = "validate.kappa"
inputs = { reference = "observed.map", simulated = "simulated.map" }
note = "MOLUSCE's kappa, kappa location and kappa histogram of the two maps."

[[nodes]]
id = "errors"
op = "map.calc"
inputs = { t0 = "start.map", o = "observed.map", s = "simulated.map" }
params = { expression = "if o != t0 and s == o then 1 else if o != t0 then 2 else if s != t0 then 3 else 0" }
note = "Where the model was right: 1 a hit, 2 a miss (changed, not simulated), 3 a false alarm (simulated, not changed), 0 no change."

[[nodes]]
id = "plot_errors"
op = "view.plot"
inputs = { value = "errors.map" }
params = { title = "Hits (1), misses (2), false alarms (3)" }
"#,
    },
    Template {
        name: "Terrain of a surface",
        group: SURFACES,
        build: None,
        about: "a relief drawn by midpoint displacement and its slope, aspect, heat load, topographic \
                position, ruggedness, curvature and surface metrics (ISO 25178)",
        toml: r#"
format = "terraflux/1"
name = "Terrain of a surface"
description = "The measures of a relief: slope, aspect, heat load, topographic position, ruggedness, curvature and ISO 25178's surface metrics."

[[nodes]]
id = "relief"
op = "nlm.mpd"
params = { rows = 200, columns = 200, resolution = 30, roughness = 0.55, nlmr_compat = false, seed = 6 }
note = "A fractal surface by midpoint displacement, from 0 to 1, of cells of 30 m (in its place, a raster.read of an elevation model)."

[[nodes]]
id = "elevation"
op = "map.calc"
inputs = { r = "relief.map" }
params = { expression = "200 + r * 800" }
note = "Metres: from 200 to 1000."

[[nodes]]
id = "slope"
op = "surface.terrain"
inputs = { map = "elevation.map" }
params = { v = "slope" }
note = "Degrees, by Horn's 8 neighbours (terra's terrain)."

[[nodes]]
id = "aspect"
op = "surface.terrain"
inputs = { map = "elevation.map" }
params = { v = "aspect" }

[[nodes]]
id = "heat_load"
op = "surface.hli"
inputs = { aspect = "aspect.map", slope = "slope.map" }
params = { latitude = -20, hemisphere = "southern", scaled = true }
note = "McCune & Keon's heat load: the slopes facing the sun warmer (a latitude of 20° south, here)."

[[nodes]]
id = "position"
op = "surface.tpi"
inputs = { map = "elevation.map" }
params = { scale = 9 }
note = "Each cell above (ridges) or below (valleys) the mean of the 9 x 9 cells around it."

[[nodes]]
id = "ruggedness"
op = "surface.vrm"
inputs = { map = "elevation.map" }
params = { s = 5 }
note = "The vector ruggedness measure: 0 flat or an even slope, up to 1 broken."

[[nodes]]
id = "curvature"
op = "surface.curvature"
inputs = { map = "elevation.map" }
params = { type = "profile" }

[[nodes]]
id = "surface"
op = "surface.metrics"
inputs = { map = "elevation.map" }
params = { metrics = ["sa", "sq", "ssk", "sku", "sdr"] }
note = "The surface's roughness (sa, sq), skewness, kurtosis and developed area ratio, as geodiv computes them."

[[nodes]]
id = "plot_elevation"
op = "view.plot"
inputs = { value = "elevation.map" }
params = { title = "Elevation" }

[[nodes]]
id = "plot_heat"
op = "view.plot"
inputs = { value = "heat_load.map" }
params = { title = "Heat load" }

[[nodes]]
id = "plot_position"
op = "view.plot"
inputs = { value = "position.map" }
params = { title = "Topographic position" }
"#,
    },
    Template {
        name: "Carbon and its uncertainty",
        group: SURFACES,
        build: None,
        about: "the carbon of a forest from its cover, its parameters drawn 500 times (Monte Carlo): the \
                mean, the deviation, the quantiles and an interval of every cell",
        toml: r#"
format = "terraflux/1"
name = "Carbon and its uncertainty"
description = "The carbon of a forest and how sure one can be of it: its parameters drawn many times (Monte Carlo), the mean, the deviation and an interval of 95 % of every cell."

[[nodes]]
id = "canopy"
op = "nlm.gaussian_field"
params = { rows = 100, columns = 100, range = 15, seed = 61 }
note = "The canopy's cover, 0 to 1 (in its place, a raster.read of a tree cover map)."

[[nodes]]
id = "carbon"
op = "uncertainty.monte_carlo"
inputs = { canopy = "canopy.map" }
params = { expression = "a * canopy ^ b * fraction", parameters = [["a", 250, 25], ["b", 1.5, 0.15], ["fraction", 0.47, 0.02]], n = 500, seed = 1 }
note = "The carbon, t/ha: the biomass from the cover by an allometry a cover^b (a 250 ± 25, b 1.5 ± 0.15), times its carbon fraction (0.47 ± 0.02); the three drawn 500 times from normal distributions. mean, sd and quantiles (2.5 % and 97.5 %) of every cell."

[[nodes]]
id = "interval"
op = "uncertainty.interval"
inputs = { value = "carbon.mean", sd = "carbon.sd" }
params = { level = 0.9 }
note = "A normal interval of 90 % from the mean and the deviation."

[[nodes]]
id = "relative"
op = "map.calc"
inputs = { m = "carbon.mean", s = "carbon.sd" }
params = { expression = "if m > 0 then 100 * s / m else null" }
note = "The deviation as a share of the mean (%)."

[[nodes]]
id = "plot_mean"
op = "view.plot"
inputs = { value = "carbon.mean" }
params = { title = "Carbon (t/ha), the mean" }

[[nodes]]
id = "plot_sd"
op = "view.plot"
inputs = { value = "carbon.sd" }
params = { title = "Its standard deviation" }

[[nodes]]
id = "plot_quantiles"
op = "view.plot"
inputs = { value = "carbon.quantiles" }
params = { title = "2.5 % and 97.5 %" }
"#,
    },
    Template {
        name: "More neutral landscapes",
        group: SURFACES,
        build: None,
        about: "NLMR's other models side by side (midpoint displacement, a gradient with noise, curds, \
                growth by neighbourhood, a Voronoi mosaic, rectangles, percolation), a surface cut into \
                habitat or classes",
        toml: r#"
format = "terraflux/1"
name = "More neutral landscapes"
description = "NLMR's other models: midpoint displacement, a gradient with noise, curds, growth by neighbourhood, a Voronoi mosaic, rectangles, percolation, and a surface cut into habitat or classes."

[[nodes]]
id = "mpd"
op = "nlm.mpd"
params = { rows = 129, columns = 129, roughness = 0.6, seed = 5 }
note = "A fractal surface by midpoint displacement: roughness from 0 (smooth) to 1."

[[nodes]]
id = "gradient"
op = "nlm.planar_gradient"
params = { rows = 129, columns = 129, direction = 45, seed = 5 }
note = "A gradient across the map, from the north-east."

[[nodes]]
id = "noise"
op = "nlm.gaussian_field"
params = { rows = 129, columns = 129, range = 5, seed = 5 }

[[nodes]]
id = "gradient_noise"
op = "nlm.merge"
inputs = { primary = "gradient.map", secondary = "noise.map" }
params = { scaling = 0.5 }
note = "The gradient with noise on it (util_merge): a climate, say."

[[nodes]]
id = "curds"
op = "nlm.curds"
params = { curds = [0.8, 0.7, 0.6], steps = [8, 4, 4], seed = 5 }
note = "Hierarchical curdling: at each level, every block split and a share of it kept as habitat (1)."

[[nodes]]
id = "neigh"
op = "nlm.neigh"
params = { rows = 129, columns = 129, p_neigh = 0.8, p_empty = 0.2, categories = 3, rescale = false, seed = 5 }
note = "Categories grown from their neighbours."

[[nodes]]
id = "mosaic"
op = "nlm.mosaic_tess"
params = { rows = 129, columns = 129, germs = 40, seed = 5 }
note = "A Voronoi mosaic of 40 germs."

[[nodes]]
id = "rectangles"
op = "nlm.random_rectangular_cluster"
params = { rows = 129, columns = 129, minl = 5, maxl = 30, seed = 5 }
note = "Overlapping rectangles of random values."

[[nodes]]
id = "percolation"
op = "nlm.percolation"
params = { rows = 129, columns = 129, prob = 0.59, seed = 5 }
note = "Every cell 1 with a probability of 0.59, near the threshold of percolation."

[[nodes]]
id = "habitat"
op = "nlm.binarize"
inputs = { map = "mpd.map" }
params = { breaks = 0.3 }
note = "The surface cut into 30 % habitat (2), the highest cells, and the matrix (1)."

[[nodes]]
id = "classes"
op = "nlm.classify"
inputs = { map = "gradient_noise.map" }
params = { weights = [0.5, 0.3, 0.2] }
note = "Or into classes of 50, 30 and 20 % of the cells."

[[nodes]]
id = "plot_mpd"
op = "view.plot"
inputs = { value = "mpd.map" }
params = { title = "Midpoint displacement" }

[[nodes]]
id = "plot_gradient"
op = "view.plot"
inputs = { value = "gradient_noise.map" }
params = { title = "A gradient with noise" }

[[nodes]]
id = "plot_curds"
op = "view.plot"
inputs = { value = "curds.map" }
params = { title = "Curds" }

[[nodes]]
id = "plot_neigh"
op = "view.plot"
inputs = { value = "neigh.map" }
params = { title = "Grown by neighbourhood" }

[[nodes]]
id = "plot_mosaic"
op = "view.plot"
inputs = { value = "mosaic.map" }
params = { title = "Voronoi mosaic" }

[[nodes]]
id = "plot_rectangles"
op = "view.plot"
inputs = { value = "rectangles.map" }
params = { title = "Rectangles" }

[[nodes]]
id = "plot_percolation"
op = "view.plot"
inputs = { value = "percolation.map" }
params = { title = "Percolation" }

[[nodes]]
id = "plot_habitat"
op = "view.plot"
inputs = { value = "habitat.map" }
params = { title = "30 % habitat" }

[[nodes]]
id = "plot_classes"
op = "view.plot"
inputs = { value = "classes.classes" }
params = { title = "Three classes" }
"#,
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

    /// Every template is in a group of the menu, under a name of its own.
    #[test]
    fn templates_are_grouped() {
        for t in &TEMPLATES {
            assert!(GROUPS.contains(&t.group), "{}", t.name);
            assert_eq!(TEMPLATES.iter().filter(|u| u.name == t.name).count(), 1, "{}", t.name);
            if t.build.is_none() {
                assert_eq!(t.model().name.as_deref(), Some(t.name));
            }
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
            let counts: Vec<usize> = maps.iter().map(farming).collect();
            assert!(counts[2] > counts[0], "{name}: farming grows ({counts:?})");
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
