//! The advice of a modeller on the operators where a choice is easy to get wrong: what to watch when using one,
//! given to its signature ([`OpSignature::advice`](terraflux_graph::OpSignature)) when it is added to the registry
//! (an operator may give its own instead). describe_operator, the application's documentation of the operators
//! and the site tell it with the operator's ports.

/// The advice of an operator, if it has one.
pub fn of(op: &str) -> Option<&'static str> {
    TIPS.iter().find(|(o, _)| *o == op).map(|(_, t)| *t)
}

/// The operators with advice, and it.
pub const TIPS: &[(&str, &str)] = &[
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
         editor) before simulating: a weight against what is known of the transition is worth a note. Distances \
         (to roads, rivers, towns and the change already there) are the strongest drivers of most models.",
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
         larger `regmult`. Thin the records first (sdm.one_per_cell, sdm.thin): clusters of records teach the \
         sampling, not the species.",
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
        "circuit.pairwise",
        "The resistance must be above 0 where it has a value (no data: a barrier); the focal points are the \
         patches or sites to connect. The effective resistance depends on the cell size: compare runs on one grid.",
    ),
    (
        "cost.transition",
        "Of a conductance (1 / resistance), usually corrected for the cells' distances (cost.geo_correction) before \
         cost.acc_cost or cost.passage.",
    ),
    // connectivity on graphs
    (
        "conn.graph_from_raster",
        "The nodes are the habitat patches: the habitat a map of classes in a projected coordinate system, small \
         patches below the species' minimum area left out. Euclidean distances between edges ignore the matrix; \
         least-cost ones need a resistance from the literature or from movement data (cite it).",
    ),
    (
        "conn.graph_from_vector",
        "The nodes are the patches' polygons, in a projected coordinate system (distances and areas in metres). \
         Euclidean distances between edges ignore the matrix; least-cost ones need a resistance (cite it).",
    ),
    (
        "conn.protconn",
        "`distance` is the species' median dispersal distance (the probability `probability` at it): the result \
         is for that species; give several distances for a range of species, and say where each comes from. \
         `distance_type` least-cost needs a resistance.",
    ),
    (
        "conn.eca_change",
        "Compare graphs built the same way (the same distances, `probability` and dispersal distance): the change \
         of ECA is of the habitat and of its connections together, so show the change of the habitat area beside it.",
    ),
    (
        "conn.node_importance",
        "The importance of a patch (dPC, dIIC: the loss when it goes) depends on the dispersal distance of the \
         graph: rank the patches at the species' distance, and say it.",
    ),
    // hydrology
    (
        "hydro.fill",
        "Fill (or breach, hydro.breach) the DEM's depressions before the flow directions, else the flow stops in \
         every pit. A DEM in a projected coordinate system, its cells square; breaching changes the terrain less \
         than filling where roads and dams cross the valleys.",
    ),
    (
        "hydro.flow_accumulation",
        "Of a DEM with its depressions filled (hydro.fill or hydro.breach). `method` d8 for streams and \
         catchments, mfd (InVEST's) or dinf for indices over the slopes (TWI, SPI); `units` area to compare grids \
         of other cell sizes.",
    ),
    (
        "hydro.streams",
        "`threshold` is in the accumulation's units (cells by default): 150 cells of 30 m are 13.5 ha. Check the \
         streams on a map of the rivers (or the Hydrography90m) and record the threshold chosen and why.",
    ),
    (
        "hydro.hand",
        "The height above the nearest drainage depends on the drainage it is measured from: the streams given \
         (hydro.streams) or `threshold`; record the threshold. A DEM with its depressions filled.",
    ),
    (
        "hydro.twi",
        "Of a DEM with its depressions filled, in a projected coordinate system (the slope and the area in \
         metres); `method` chooses the accumulation (fd8 by default). Flat cells get no value unless `min_slope` \
         gives tan(slope) a floor: say which.",
    ),
    (
        "hydro.watersheds",
        "Snap the outlets to the streams first (hydro.snap): a point a cell off the channel gives a catchment of a \
         few cells.",
    ),
    // ecosystem services (InVEST)
    (
        "services.carbon",
        "The pools' table gives the carbon of each class (above and below ground, soil, dead matter) in Mg per \
         hectare: cite its source (IPCC tables, a local inventory). Two maps of land use give the change; the \
         valuation needs a price and a discount rate.",
    ),
    (
        "services.habitat_quality",
        "The threats' table (each threat's maximum distance, weight and decay) and the sensitivity of each class \
         are the model: take them from the literature or experts and record them; the result ranks places, it is \
         not a measure of biodiversity.",
    ),
    (
        "services.water_yield",
        "The precipitation and the evapotranspiration in mm a year, the depth in mm, the plant available water as \
         a share; calibrate the seasonality (Z) against observed flows at a gauge before comparing scenarios.",
    ),
    (
        "services.sdr",
        "A DEM with its depressions filled, the erosivity (R) and erodibility (K) in the units of the USLE; the \
         flow `threshold` must give the real streams. Calibrate k and ic_0 against measured sediment loads when \
         there are any: without them compare scenarios, not absolute loads.",
    ),
    (
        "services.ndr",
        "The loads and retention of each class come from a table to cite; compare scenarios rather than absolute \
         exports when there are no measured loads to calibrate against.",
    ),
    (
        "services.pollination",
        "The pollinators' guilds (nesting, flight season, foraging distance) and the floral resources of each \
         class are the model: take them from the literature or experts and record them.",
    ),
    // agents
    (
        "agents.update",
        "Rules are applied in order, each line seeing the lines before: put the state each rule reads above it. \
         Record where each rule and number comes from (a survey, the literature) in the node's why; run several \
         seeds when a rule draws at random.",
    ),
    (
        "crafty.allocate",
        "The agents' production functions and the demand of each service are the model: record their sources. \
         Its seed draws the competition: run several seeds, and compare the land with an observed map.",
    ),
    // REDD+ (VT0007)
    (
        "redd.vulnerability",
        "Fit the vulnerability on the calibration period and check it on a later one (redd.assess) before the \
         baseline: VT0007 asks for the model to be confirmed on a period it was not fitted on.",
    ),
    (
        "redd.fit",
        "Fitted on the deforestation of the historical reference period only; the subdivisions (states, \
         municipalities) let the frequency differ between them.",
    ),
    (
        "redd.assess",
        "Assess the fitted density on the deforestation of a period it was not fitted on (the confirmation \
         period): the residuals and the error tell whether the model can be used for the baseline.",
    ),
    (
        "redd.allocate",
        "The expected deforestation (hectares) comes from the historical rate of the jurisdiction, not from the \
         model; the allocation only places it.",
    ),
    (
        "carbon.emissions",
        "The stocks of each class in Mg of carbon per hectare (cite them); CO2 is carbon times 44/12. A loss is an \
         emission at once here: decay of the dead wood and the soil over the years is not modelled.",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_tip_is_of_an_operator() {
        let r = crate::Registry::standard();
        for (op, tip) in TIPS {
            let sig = r.sig(op).unwrap_or_else(|| panic!("a tip of {op}, not an operator"));
            assert_eq!(sig.advice, *tip);
        }
        // the families that lacked advice have some
        for family in ["conn.", "hydro.", "services.", "agents.", "redd."] {
            assert!(TIPS.iter().filter(|(o, _)| o.starts_with(family)).count() >= 1, "{family}");
        }
    }
}
