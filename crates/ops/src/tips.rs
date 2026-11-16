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
        "data.stac",
        "Keep the grid small and the cells as coarse as the question allows: every scene of the search is read and \
         warped (a Sentinel-2 summer over a few tens of kilometres at 10 m is many scenes). Ask for few scenes with \
         `datetime`, `limit` and `query` (`{\"eo:cloud_cover\": {\"lt\": 20}}`); the median of the scenes drops \
         clouds and shadows that move, the `first` composite takes a scene as it is (classes, a DEM's tiles: set \
         `classes` for the most common cell). The Planetary Computer's files need `sign = planetary_computer`. The \
         values are the files' integers: scale and offset (reflectance / 10000) are the catalogue's, not applied.",
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
        "conn.metapopulation",
        "`alpha` is 1 / the species' mean dispersal distance, in the inverse units of the patches' coordinates \
         (metres for a projected map): 0.002 is 500 m. `x`, `y` and `e` are the incidence function's extinction \
         and colonisation parameters: take them from a fitted model or the literature, and say where. A run is one \
         possible future, as the seed draws it: use many `steps`, several seeds (control.each) and report the \
         spread of the incidence.",
    ),
    (
        "conn.overall",
        "A row per distance: give several to see how the indices change with the dispersal distance. IIC counts \
         the links and PC their probabilities, both from the graph's distances, so the graph's distance type (edge \
         or least-cost) and its units set the result. Say the species' distance and `probability` used.",
    ),
    (
        "conn.link_importance",
        "Each link is removed in turn and the share of IIC or PC lost is the importance: it takes one distance, \
         and the cost grows with the number of links (a run per link): limit the graph with `max_distance`. Rank the \
         links at the species' dispersal distance; with least-cost distances the detours a removal forces are not \
         measured again.",
    ),
    (
        "conn.distances",
        "`distance` edge is the gap between the patches' closest cells (Euclidean, ignores the matrix); centroid \
         joins their centres; least-cost and commute-time need a `resistance` and join the centroids, in the \
         resistance's units, not in metres. `max_distance` keeps the pairs up to it (the dispersal distance of \
         the species), which makes the table much smaller on many patches.",
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
    // data preparation
    (
        "raster.reclassify",
        "The first rule that takes a value gives it, so the specific rules go before the general ones; a range \
         `0..10` has both ends in. Values no rule takes stay as they are: end with `* -> nan` to drop the rest.",
    ),
    (
        "raster.resample",
        "Another resolution in the raster's own coordinate system: it neither reprojects (raster.reproject) nor \
         lines the map up with another (raster.match). Classes by the nearest cell (the most common when the cells \
         grow), values by bilinear (the mean when the cells grow): `resampling` auto (the default) chooses by the data.",
    ),
    (
        "raster.zonal",
        "The zones are a map of ids (municipalities or watersheds rasterized, classes): cells without a zone or \
         without a value are left out of the statistics. Besides the row of each zone it gives a map of every cell \
         with its zone's statistic; for polygons as they are use vector.zonal.",
    ),
    (
        "vector.zonal",
        "Each polygon takes the cells GDAL burns for it: a cell under two overlapping polygons counts for both, a \
         cell without a value is left out. `all_touched` takes every cell a polygon touches, not only those \
         whose centre it holds.",
    ),
    (
        "vector.buffer",
        "`distance` is in the features' units: on a layer in degrees it is degrees, so reproject to a projected \
         coordinate system (vector.reproject) before buffering in metres. A negative distance shrinks polygons; \
         `dissolve` merges the buffers into one feature.",
    ),
    (
        "vector.rasterize",
        "A cell is burnt where a feature's centre falls (`all_touched`: every cell it touches), so thin polygons \
         may lose cells on a coarse grid. Later features go over the earlier ones, or add up to count \
         overlaps; the features are reprojected to the map's coordinate system first.",
    ),
    (
        "vector.overlay",
        "The intersection keeps the attributes of both layers; the union keeps, for the parts of one layer the \
         other does not cover, only that layer's own; the difference is the first layer less the second. The \
         second layer is reprojected to the first's coordinate system.",
    ),
    (
        "vector.spatial_join",
        "Each feature takes the attributes of the first feature of the other layer it meets: a point on borders \
         or overlaps gets one of them only. To count what falls in each polygon (the points in each) use \
         `summary`, which gives the count and the sum and mean of `fields`.",
    ),
    // land change analysis
    (
        "change.suggest",
        "The transitions worth modelling from a series of maps: those that make at least `least` % of all the \
         change (default 1); the others are too small to fit a model. Read its table (of_change, of_class) before \
         choosing what a change model fits.",
    ),
    (
        "change.crosstab",
        "The rows are the classes of the first map, the columns those of the second: the diagonal is what did not \
         change. Classes found in neither map are left out. The cells count: multiply by the cell area for \
         hectares, in a projected coordinate system.",
    ),
    (
        "change.transitions",
        "The area (km2) of a transition is the cells times a cell's area: give `cell_area` (m2) when the grid is \
         not in metres. The years of the maps are `times`, by default the layers' names when numbers, else 0, 1, \
         2: name the layers by year or set `times`.",
    ),
    (
        "change.components",
        "Report the difference split into quantity, exchange and shift, not only the overall: a model can get \
         the quantity of each class right and still place it wrong, which shows in exchange and shift. A class's \
         gain is its omission, its loss its commission.",
    ),
    (
        "change.intensity",
        "Rates are in % a year: set `times` (the years of the maps) or name the layers by year, else the periods \
         are 0, 1, 2. `gain` and `loss` choose the class whose gains and whose losses the transition level \
         examines: choose classes that are active at the category level or large in change.suggest. The \
         stationarity needs several periods.",
    ),
    // validation of simulations
    (
        "validate.kappa",
        "Compares the simulated map with the reference of the same date, cell by cell, without the start map: a \
         high agreement may be mostly persistence. For a simulation of change also give validate.agreement or \
         validate.three_map, which start from the map of the beginning; the reference not used to calibrate.",
    ),
    (
        "validate.agreement",
        "Needs the map both start from (`start`), the observed one and the simulated one: the figure of merit is \
         hits over hits, misses, false alarms and wrong hits, so a model that changes nothing scores 0 on the \
         change that happened. The observed map must not be one the model was calibrated on.",
    ),
    (
        "validate.error_budget",
        "The simulated map must not have a class the reference lacks. `iterations` resolutions are compared, each \
         merging `scale` x `scale` cells (the edges left over are dropped): the table gives the agreement of \
         each resolution under no, medium and perfect information of quantity and location.",
    ),
    (
        "markov.matrix",
        "A chain given by hand: the probabilities of the whole period over `steps` steps (a row per class at the \
         start); those per step are its principal root unless `multi` gives them. Say where the numbers come from \
         (markov.fit estimates them from two maps) and calibrate on a period, validate on another.",
    ),
    // species distribution
    (
        "sdm.maxnet",
        "The background is the area the species could reach (sdm.accessible_area), drawn with the bias of the \
         records (sdm.bias_surface, sdm.background): the scores depend on it. `classes` (l q p h t) and `regmult` \
         set the complexity: fewer feature classes and a larger regmult overfit less. Name the categorical \
         predictors in `categorical`; `type` (cloglog by default) is what the predictions mean.",
    ),
    (
        "sdm.background",
        "The points are drawn where every predictor has a value, within `area` (sdm.accessible_area) or a band of \
         `distance` from the records; `bias` (sdm.bias_surface) draws them with the sampling effort of the \
         records, the target-group background. The records' cells are left out by default. Record how it was \
         drawn and the `seed`: the evaluation scores depend on the background.",
    ),
    (
        "sdm.threshold",
        "Different methods give very different suitable areas: `method` max_spec_sens by default, or max_kappa, \
         no_omission, equal_prevalence, equal_sens_spec, mtp, p10, a sensitivity. Record the choice and why. Keep \
         the continuous suitability where the analysis takes it (prioritization), and threshold only for a map of \
         presence and absence.",
    ),
    (
        "sdm.evaluate",
        "The background is not a set of absences: a background point predicted suitable may be a place the \
         species lives. AUC grows with the extent of the background, so compare it only between models with the \
         same one. Give the predictions of a model at records it did not see.",
    ),
    (
        "sdm.mess",
        "Negative where the model extrapolates (an environment outside the reference's range): check it before \
         projecting a model to another region or period and say where it is negative; `full` gives the MESS of \
         each predictor to see which one leaves the range.",
    ),
    // conservation planning
    (
        "prioritize.zonation",
        "The rank is an order (0 removed first, 1 kept last), not a selection: choose how much is kept (the \
         curves tell how much of each feature remains). The rule (CAZ1, CAZ2, CAZMAX, ABF...), the `weights`, \
         the cost and the hierarchic mask are the choices: record them.",
    ),
    (
        "prioritize.marxan",
        "`num_reps` independent runs from the `seed`: the best run (`best`) and how often each unit was \
         selected (`frequency`) are outputs.`blm` (the weight of the boundary) and the features' `spf` (the penalty of a shortfall) \
         are choices to calibrate and record; the cost of a unit is a choice too.",
    ),
    (
        "prioritize.evaluate",
        "Give the problem as to prioritize.optimize (the same features, cost and targets) so that the coverage \
         is of the same targets; the solution as a map or as optimize's selection. Compare solutions from \
         several methods or targets with it.",
    ),
    // landscape metrics and patterns
    (
        "metrics.window",
        "`window` is the side in cells, odd (default 5), `shape` square or circle: the window sets the scale of \
         every map, record it. Cells outside the map are NA.A map per metric, in a stack: ask \
         for the few metrics needed.",
    ),
    (
        "metrics.series",
        "A layer per date, the dates in `dates` (else the layers' keys, or 1, 2...). At class level every class \
         appears at every date, with 0 for the area, edge and count metrics of an absent class, not no value. \
         Zones, buffers around points or a grid give a row per zone and date.",
    ),
    (
        "landscape.parcellation",
        "The classes are whole values 1-255; 0 and cells without a value are missing (recode with \
         raster.reclassify first). It gives the patches' number, the average patch size, the area-weighted one \
         (the effective mesh size), the division and the parcellation index, per class and for the whole map.",
    ),
    (
        "landscape.spa",
        "A coarser, faster MSPA of the 8-connected foreground, edges of one cell: for the finer classes use \
         landscape.mspa. SPA2, SPA3, SPA5 and SPA6 give different classes: say which was used. The foreground is \
         the classes given, else the map coded 0 missing, 1 background, 2 foreground.",
    ),
    // cost distances and circuits
    (
        "cost.geo_correction",
        "Correct a transition for the distances between cell centres: type c (the default) for least-cost \
         distances (cost.acc_cost, shortest paths), type r for random walks (cost.passage). Longitude/latitude \
         grids use great-circle distances.",
    ),
    (
        "cost.acc_cost",
        "Of a transition of conductances (cost.transition, corrected with cost.geo_correction), from the nearest \
         origin: infinity where no origin can be reached. Its units are those of the cost the transition holds, \
         not metres.",
    ),
    (
        "cost.passage",
        "Without `theta` it is the current of a unit current from the origins to the goals (a symmetric \
         transition); with theta (0 to 20) randomised shortest paths that prefer cheap routes more as theta \
         grows. Record theta: it is the choice of how directed the movement is.",
    ),
    // hydrology
    (
        "hydro.snap",
        "A point moves to the nearest stream cell within `distance`; with `accumulation` and `min_accumulation` to \
         the nearest one that has at least that accumulation (on the river, not on a tributary beside it). A \
         point farther than the distance stays where it was, snapped 0: look at that column.",
    ),
    (
        "hydro.upstream",
        "Statistics of the maps over all the cells that drain through a link or point (D8 directions): cells \
         without a value in a map are left out of its statistics, so the share of a class is of the cells with a \
         value.",
    ),
    // accuracy of a map and the area of its classes
    (
        "accuracy.sample_size",
        "The size follows the standard error wanted of the overall accuracy and the user's accuracy expected of \
         each class (`standard_error`, default 0.01; lower expected accuracies for classes hard to map, such as \
         change: 0.6 to 0.7). The `minimum` allocation keeps a sample (default 50) in every stratum when some \
         are rare.",
    ),
    (
        "accuracy.sample",
        "The points are drawn by stratum; the `reference` column is empty: fill it with the class seen in \
         imagery or in the field before accuracy.assess. The same seed gives \
         the same sample.",
    ),
    (
        "accuracy.assess",
        "The strata are the map's classes (Olofsson et al.) or strata of their own (Stehman 2014: give the strata). \
         The area of each class is estimated from the sample, with its confidence interval, not counted on the \
         map: report these areas, not the map's.",
    ),
    // sensitivity analysis
    (
        "sensitivity.design",
        "Morris (`n` trajectories of factors + 1 runs) screens many factors cheaply: use it first; Sobol (`n` base \
         points, a power of 2, of 2 x factors + 2 runs, or factors + 2 without `second_order`) gives variance \
         shares. Run the model at every row, then give the outputs to sensitivity.analyze with the same method \
         and levels.",
    ),
    (
        "sensitivity.analyze",
        "The `method`, `levels` and `second_order` of the design; `column` is the result analysed (default the \
         last). Morris's mu* is the importance and sigma the interactions and non-linearity; Sobol's S1 is the \
         share of the factor alone and ST with its interactions. Read the indices with their bootstrap intervals.",
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
