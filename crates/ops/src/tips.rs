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
         values are the files' integers: scale and offset (reflectance / 10000) are the catalogue's, not applied. A \
         median of scenes left as they are keeps the clouds where they persist (the tropics): `mask = auto` drops \
         them pixel by pixel before the scenes are combined (Sentinel-2's SCL, Landsat's QA_PIXEL, HLS's Fmask; \
         `mask_classes`, `mask_buffer` to grow it), and `first` then takes the first clear value. An asset of several bands (NAIP's `image`) \
         gives a layer per band, named by the item's eo:bands (`bands` chooses); `crs` gives `extent` and `cellsize` in \
         a projected system (EPSG:32631, metres) and warps the scenes straight onto it, one resampling and no \
         raster.reproject after.",
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
        "linkage.pathways",
        "The cores are the habitat patches to connect, whole ids on the resistance's grid; the resistance is a cost \
         per cell (1 or more, from the literature or from movement data: record where it comes from), no value or \
         0 a barrier. The links by `adjacency` cost (the default) are those Linkage Mapper draws; `width` is in \
         cost units, not map units. A link past `max_cwd`, or through a third core, is listed with its reason, \
         not drawn.",
    ),
    (
        "linkage.pinchpoints",
        "`links` is linkage.pathways' table, on the same cores and resistance; `width` is the corridor's cutoff in \
         cost units (a wider band lets the current spread, a narrow one forces it through the pinch points). The \
         current is relative: read where it concentrates, not its amps. A low cwd_to_resistance marks a link whose \
         corridor is a bottleneck. Circuitscape solves every cell of the band: keep it to a few million cells.",
    ),
    (
        "linkage.barriers",
        "`links` is linkage.pathways' table, on the same cores and resistance. The radii are in map units (metres in \
         a projected system; the ring is 1 map unit thick, as Linkage Mapper's, so a radius that is a multiple of the \
         cell size catches the cells at that distance). A high improvement marks where restoring a circle of that \
         radius shortens a link most per unit restored; `restored` 1 means a restored cell costs 1 per map unit, as in \
         Linkage Mapper. Read `circles` for the map, `improvement` for the centres, and compare radii with `radius`.",
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
    // objects (OBIA)
    (
        "segment.from_map",
        "The way in for any segmenter run elsewhere (SAM, scikit-image, OTB, eCognition, a k-means of the bands): \
         give the map of its segments (a number per region) or of its classes. Cells of one value that do not \
         touch are different objects, so a class map becomes its patches; `neighbours` 8 joins the diagonal ones. \
         A cell without a value is no object.",
    ),
    (
        "segment.chessboard",
        "The simplest segmentation, with no regard to the image: use it for a first level above finer objects, to \
         test a rule set, or to give the scale of a sampling grid. `size` is in cells: 1 gives a cell per object. \
         For objects that follow what the image holds, use segment.quadtree or segment.from_map. `bounds` (a map, \
         vectors such as parcels, or objects) cuts the squares where they cross a zone.",
    ),
    (
        "segment.quadtree",
        "Squares as small as the image needs: big where it is even, small at its edges. `threshold` is in the \
         image's units (a range inside a square, any band): start from a tenth of the image's range and lower it \
         until the objects follow the edges you want; `min_size` stops the splitting at a few cells. A stack uses \
         every band, so give it the bands that tell the classes apart. `bounds` cuts the squares where they cross \
         a zone.",
    ),
    (
        "segment.multiresolution",
        "The scale is in the image's units (the same scale gives other objects on reflectance 0-1 than on 0-10000), \
         so find it with segment.scale, which suggests scales from the image, then look at the objects in the \
         Viewer and raise or lower it. shape 0.1 and compactness 0.5 are eCognition's usual start: more shape \
         gives smoother, more compact objects at the cost of following the colour. For a hierarchy, connect a \
         level to `from` of the next with a higher scale; a superpixel level (segment.slic, segment.snic) as the \
         first one makes the run faster than starting from single cells. Weights tell the bands apart (0 \
         leaves one out of the colour). `bounds` (a map, vectors such as parcels, or objects) keeps every \
         object inside one zone; with `from`, the objects of the level below are cut by the zones first.",
    ),
    (
        "segment.slic",
        "Superpixels are many small, regular objects: use them as the first level of segment.multiresolution \
         (`from`), or as the cells of a classification, not as the final objects. `size` sets their side in cells \
         and `compactness` their regularity (higher: more regular; lower: closer to the edges). `bounds` (a map, \
         vectors such as parcels, or objects) cuts the superpixels where they cross a zone.",
    ),
    (
        "segment.snic",
        "As segment.slic, and no iterations: it grows the superpixels from their seeds in one pass, so it is faster \
         and needs no tuning of `iterations`. Use the superpixels as the first level of segment.multiresolution \
         (`from`), or as the cells of a classification. `bounds` cuts the superpixels where they cross a zone.",
    ),
    (
        "segment.felzenszwalb",
        "`k` sets how large the segments grow, in the units of the bands (standard deviations by default): raise it \
         for larger segments; `min_size` joins the smallest ones. It makes segments of very different sizes, from \
         a graph of the cells' differences, which suits irregular scenes and less a fixed scale: for a scale you \
         control, use segment.multiresolution. `bounds` (a map, vectors such as parcels, or objects) leaves out \
         the graph's edges between zones, so no segment crosses one.",
    ),
    (
        "segment.threshold",
        "The multi-threshold segmentation of eCognition: it cuts one band (`band`, the first by default) into \
         classes of values and makes an object of each connected piece of a class, so it suits what one band \
         separates (water against land in an index, burnt against unburnt). Give the `thresholds`, or `classes` \
         and multi-level Otsu finds them in the gap between the modes of the histogram (the thresholds are an \
         output and are noted in the run's warnings). It cuts at the values alone: noise in the band makes many \
         small objects, so smooth the band first, or merge the small ones afterwards (objects.remove_small). `bounds` cuts the pieces where they cross a \
         zone.",
    ),
    (
        "segment.spectral_difference",
        "It merges the neighbours of objects you have (a chessboard, superpixels, a multiresolution level) whose \
         mean values differ by less than `max_difference`, in the image's units, and repeats with the means of the \
         merged objects, so a gradual change can chain into one object: keep the limit small against the contrast \
         you want kept. It joins by the tone alone, with no shape, which is the reason to run it after a \
         segmenter that keeps the edges, not on single cells. Use `weights` to make a band count more or less. `bounds` (a map, vectors such as parcels, or objects) \
         lets only neighbours of one zone merge, and cuts the objects you give where they cross a zone first.",
    ),
    (
        "segment.region_growing",
        "The method of GRASS GIS's i.segment. The bands are scaled to 0-1 over the whole image and `threshold` is a \
         distance in that scale (0 to 1): 0.01 to 0.05 is the usual start, so the same threshold means the same on \
         other data. Neighbouring regions merge when they are each other's most similar and closer than it, pass \
         after pass until none does. Lower thresholds give finer objects: use the result as `seeds` of a run with a \
         higher one for a hierarchy. `minsize` joins the specks to their most similar neighbour at the end; \
         `bounds` (a map of zones, vectors such as parcels, or objects) keeps the objects inside the zones.",
    ),
    (
        "segment.quickshift",
        "Quick shift finds the modes of a density over position and colour, so it needs no count of objects: \
         `kernel_size` (cells) and `max_dist` set how far apart the objects are, `ratio` how much colour counts \
         against position (the bands are standardised, so one standard deviation counts as `ratio` cells: 5 to 30 \
         is the usual range). The cost is about (6 x kernel_size) squared exponentials per cell, so keep `kernel_size` \
         small on a large image. The objects are always connected: a mode's area that its links leave in pieces is \
         split. `bounds` cuts the segments where they cross a zone.",
    ),
    (
        "segment.watershed",
        "Without markers the basins are the minima of the gradient, so many on a noisy image: raise `sigma` (the \
         smoothing) or `h` until they are about the objects you want. With markers (a map of seeds, the centres of \
         the trees or the buildings, a classification's cores) every object has one marker, which is the way to \
         control what it finds. `bounds` cuts the basins where they cross a zone.",
    ),
    (
        "segment.meanshift",
        "It is the slowest of the segmenters: its cost grows with the square of `spatial_radius` and with the \
         cells, so try it on a part of the image first. `range_radius` (in standard deviations when the bands are \
         standardised) sets how different cells may be and still be one object; `min_size` removes the specks. \
         `bounds` cuts the regions where they cross a zone.",
    ),
    (
        "segment.scale",
        "ESP runs the segmentation at 30 scales on the central `window` of the image (512 x 512 cells by default) \
         so it stays fast on large images: the scales hold for the whole image, but choose a window that holds \
         the objects you want, or 0 for all of it. The peaks of the ROC are guides: open the chart of the table, \
         try the suggested scales in segment.multiresolution and look at the objects. Give `start` and `step` \
         to look closer at a range of scales.",
    ),
    (
        "segment.optimize",
        "USPO: it runs a segmenter at each value you give (`values`, or `from`, `to` and `step`) and scores each \
         result by the variance inside its objects (low: homogeneous objects) and Moran's I of the objects' means \
         over the objects that touch (low: neighbours that differ), both scaled over the runs; the highest sum \
         wins (`objective` f takes the F-measure of Johnson and Xie, `alpha` weighing the variance). Too fine \
         a segmentation is alike its neighbours and too coarse a one is not homogeneous, so the best lies between. \
         The segmenter is `segmenter` (multiresolution by default) and the setting varied is `parameter` (scale, \
         threshold, size... by segmenter); its other settings go in `params`, an object. Look at the table of \
         `scores` and at the objects, as for segment.scale: the score is a guide. The runs go in parallel, so \
         try a range of 5 to 10 values on a part of a large image first.",
    ),
    (
        "objects.features",
        "`basic` (the default) is the usual start: the mean and standard deviation of each band, NDVI when the \
         red and near infrared bands are found, the area and the compactness. Ask for more by name, or `all` for \
         every group: it computes many columns, so on a large image with many objects name what you will use \
         (texture, when it comes, is the expensive group). Give `red`, `nir`, `green` and `swir` when the bands' \
         names do not say which they are. Put a DEM, a height model or another date in `more` (on the same grid: \
         raster.match first). Give a map or vectors (lots, a land-use layer) as `thematic` and each object gets \
         th_mode, the most common value under it, and th_share, its share of the cells that have a value; with \
         vectors, `field` names the attribute (text too: its codes are named in the table). Rules read th_mode \
         as any column. The `skeleton` group (not in `basic`) is for long thin things, roads, rivers, fields, \
         crowns: skel_main_length and skel_width, a high skel_length_width with a low skel_width picks the \
         linear ones, skel_branches and skel_junctions tell a network from a line, skel_curvature a winding \
         river from a straight road. It thins the raster of each object, so the values differ a little from \
         eCognition's polygon skeleton; `spur` (default 3 cells) cuts the small branches thinning leaves at \
         ragged edges.",
    ),
    (
        "segment.sam",
        "Optional: it needs ONNX Runtime (pip install onnxruntime, then set ORT_DYLIB_PATH to its shared library). \
         With no `encoder` and `decoder` the weights of SAM ViT-B (Apache-2.0, exported to ONNX by vietanhdev, from \
         huggingface.co) download on first use to the cache folder, 75 MB, or 348 MB with `weights` full; or give \
         the ONNX files of SAM's encoder and decoder. It is slow on a CPU: \
         one encoder run per 1024 x 1024 tile and one decoder run per prompt point, so start with `points` 8 on a \
         small area. Give three bands as red, green and blue; raise `iou` for fewer, surer masks.",
    ),
    (
        "objects.embed",
        "Optional: it needs ONNX Runtime (pip install onnxruntime, then set ORT_DYLIB_PATH) and the ONNX encoder of \
         a foundation model, which do not come with Terraflux. `patch` and `bands` must be what the model takes \
         (its input is 1 x bands x patch x patch). The emb_ columns are for a classifier or a clustering of the \
         objects, alone or with the objects.features columns.",
    ),
    (
        "objects.to_vector",
        "The polygons follow the cell edges exactly (stairs); `smooth` cuts their corners that many times (1 to 3 \
         is enough to look natural) and shrinks them a little at each corner, so keep 0 when the polygons are \
         counted, measured or put back on the map's grid.",
    ),
    (
        "objects.samples",
        "Give at least 10 objects per class (a few points or small polygons on each class, over its different \
         looks): a point picks its object, a polygon every object it covers by half. Objects with samples of \
         two classes are left out, so draw the samples inside one object. Fit the table with `column` = class: every \
         other column is a predictor (the table has no id).",
    ),
    (
        "objects.classify",
        "The objects must carry the same attributes as those of the training: run objects.features with the same \
         settings on the same image before. Pass the samples' table to name the classes. The margin (first minus \
         second probability) is low where the model hesitates: look there for more samples. `min` leaves \
         unclassified the objects whose best probability is under it.",
    ),
    (
        "objects.rules",
        "A leaf class (one that is no one's parent) is what an object becomes; the parents only narrow what their \
         daughters may take, as they inherit the mother's rule. An object whose best membership is under `min` \
         (0.1, as eCognition's minimum membership value) stays unclassified: lower it, or widen the rules' ramps. \
         A rule that reads classes (border_to, nb.count(cls == \"water\")) reads those the objects have before this \
         node: chain a second objects.rules for a second pass. The margin shows where two classes are close.",
    ),
    (
        "objects.edit",
        "Put it right after the segmenter, before objects.features and the classification, which run again on the \
         edited objects (it gives objects with no classes). The edits are places, not object numbers: after a change \
         of the segmentation above they find the objects now at those places, and an edit that finds none (or \
         changes nothing) is warned about and has a row in `report`. A cut line must cross the object and split it; \
         connect the image for a delete into the most similar neighbour and for the table of band means.",
    ),
    (
        "objects.split",
        "For crowns, buildings, cells and mounds that were segmented as one object. by distance cuts at the necks:          a centre must stand `h` (default one cell) above the saddle that joins it to a higher one, so raise `h` if          round objects split and lower it if a pinched one does not. by surface floods a map instead (a canopy height          model, the brightness: smooth it with `sigma` first when it is noisy) and `invert` is for dark centres.          `max_neck` keeps the objects whose parts are joined by a wide neck (a cut longer than max_neck times the          smaller piece's width is undone: 0.5 to 1 is usual), `min_size` sends the crumbs back to a neighbour piece,          and `classes` limits it to the objects of some classes. The pieces keep the class; the level above and the          other attributes are dropped, so run objects.features after it.",
    ),
    (
        "objects.resize",
        "mode grow takes the free cells within `distance` (map units; or `steps` cells) of the objects, each to the          nearest, and shrink gives them back; open (shrink, then grow back over what the object had) cuts thin necks          and drops specks, close (grow, then shrink the cells added) fills slits and small gaps, fill closes the holes          one object encloses (`max_hole` limits their size), smooth rounds spikes and notches. A `mask` (raster.calc,          say ndvi > 0.3) or `into` (class names whose cells grow may take, eCognition's candidates) makes the growth          geodesic, ring by ring, so it follows the condition and cannot jump over a barrier. refine, the border          optimization, needs the image: each border cell goes to the adjacent object of the closest band means,          `steps` times. `classes` limits the objects that change. Pieces cut off by shrink or open become objects of          the same class; the level above and the other attributes are dropped, so run objects.features after it.",
    ),
    (
        "objects.fuse",
        "For objects segmented too finely (a building in pieces, a field in strips): a seed joins a neighbour only when \
         the object they make is good by the `fitting`, an expression over the joined object (compactness, which is 1 \
         for a rectangle and grows as the shape gets ragged, so goal min; shape_index; mean_<band> and sd_<band> with \
         the image; length, width, area). `improve` (the default) asks the joined object to be as good as the seed's \
         own value, `threshold` that it reach a value (a compactness of at most 1.3, a mean_nir of at least 0.3), and \
         both can be used: improve false with a threshold joins whatever reaches it. `seeds` and `candidates` are \
         class names (`unclassified` counts), the result has the seed's class. mode best gives each seed its best \
         candidate, best pairs first; all lets a seed whose best was taken fall back to the next in the same pass. \
         It repeats until nothing joins (`max_passes`), so a control.repeat is not needed. Texture, skeleton and \
         context features cannot be derived for the joined object and are refused; the box stands for length and \
         width. The level above and the other attributes are dropped, so run objects.features after it.",
    ),
    (
        "objects.extrema",
        "For the tops of the crowns, the brightest roofs, the wettest spots: kind max (or min) marks the objects whose \
         attribute is larger (smaller) than that of every other object near them. The attribute is a column of the \
         table or an expression as objects.calc's (nb.mean(ndvi) first smooths a noisy one over the neighbours). Near \
         is the adjacent objects (`steps` 1, the default; more steps reach further across the neighbours) or all \
         within `radius` map units between centroids: use the radius when the objects are small and many (after \
         objects.split) and the steps when they differ in size. `among` limits the classes compared, so a maximum is \
         searched among the trees only. strict (the default) leaves a tie out, so a flat top gives none; switch it off \
         to keep one of a plateau (all of them, since each is as high as the highest). The extrema take the class \
         `class`, the other objects keep theirs; the table has the column extremum. To find the objects enclosed by a \
         class (eCognition's find enclosed by class) no operator is needed: the context column `rel_border_<class>` of \
         objects.features (or `border_to('<class>')` in objects.calc) equal to 1 means the whole border is shared with \
         that class; for several classes add the borders (`border_to('a') + border_to('b') > 0.999`).",
    ),
    (
        "objects.nearest",
        "A few attributes, chosen with the feature space (objects.features), work better than many: every \
         attribute adds to the distance, and size and shape attributes can swamp the spectral ones, so name the \
         `columns` (the mean of each band). Give a few samples of every class, of different looks. `slope` is the \
         membership at one standard deviation of distance; `min` leaves the far objects unclassified.",
    ),
    (
        "objects.resolve",
        "Give each class its own hypothesis (a membership map from a threshold, a model or a rule) as a layer of the \
         stack, named for the class: where hypotheses overlap, the larger membership wins the cell and the loser \
         keeps the rest. `min` leaves cells without a class when no hypothesis reaches it. `merge` connected makes \
         an object of each connected area of a class, global one object per class and none one per cell: use none \
         to inspect the arbitration. The conflicts table tells which classes took cells from which.",
    ),
    (
        "objects.temporal",
        "The matrix gives the possibility of each transition (1 possible, 0 impossible, between for rare ones): a \
         transition it rules out cannot be what the objects of the second date are: each membership is cut to \
         what the first date and the matrix allow (a max-min composition). Both \
         dates must be on the same grid, classified with memberships (objects.classify, objects.rules). `both` \
         adjusts the first date from the second too.",
    ),
    (
        "objects.transition_matrix",
        "Give samples labelled with the class at both dates (the table's columns before and after), or the \
         transitions table of change.transitions: the possibility of each pair is its count over the largest one of \
         its origin, so a pair never seen is 0, or `floor` when you would not rule it out. A matrix by hand (table.of \
         with values 0 to 1) does the same for transitions you know are impossible, as forest to water in a year.",
    ),
    (
        "objects.accuracy",
        "Judge the classification on objects that were not used to train it: draw the reference apart from the \
         training samples (objects.sample draws them stratified, with the same chance in every object of a \
         class). The area-adjusted estimates are the ones to report (Olofsson et al. 2014): the counts of the \
         confusion matrix by objects ignore how large each class is. Aim at 50 objects or more per class; a class \
         with none sampled is left out. The errors table lists the misclassified objects to look at.",
    ),
    (
        "objects.sample",
        "The sample is stratified by class, each object of a class with the same chance, as the area-adjusted \
         estimates of objects.accuracy ask. Olofsson et al. advise 50 or more per class: use `per_class` 50 where \
         the classes have that many objects, `n` to share a total equally. Fill the column `reference` of the \
         points (their attribute table) with the true class, from imagery or the field, then give them to \
         objects.accuracy as samples with `class` = reference.",
    ),
    (
        "objects.calc",
        "Write a rule as a script: nb.mean(x) and border_to(\"water\") read the neighbours, parent.x and \
         children.count(cond) the levels (children need the input `below`). above(x, a, b), below, between and \
         about give a membership from 0 to 1, and in this calculator `and` is the smaller, `or` the larger and \
         `not` is one minus. A class name in `cls == \"water\"` compares as text; an unclassified object has no cls, \
         so the comparison is missing for it, not false.",
    ),
    (
        "objects.merge",
        "Merge by class to make a level above: neighbours of the same class become one object, and with `level` \
         the objects of this level stay below it (`below`), each with one parent, so rules can read parent.x and \
         children.count(cond). By condition, write it over the pair: abs(a.mean_red - b.mean_red) < 0.1 and \
         border > 3. A merged object's attributes are means weighted by cells: run objects.features again when \
         you need exact ones. To merge until nothing changes, put it in a control.repeat with `until` the `done` \
         output of this node (or compare `count`).",
    ),
    (
        "objects.grow",
        "Grow a class into its neighbours for what is long and thin, as roads and rivers that a segmentation cut \
         in pieces: the condition is read on the candidate neighbour (mean_nir > 0.2 and cls == \"unclassified\"), \
         and it repeats, over the grown objects, up to `max_steps`. Keep the condition narrow, or the class eats \
         the whole image.",
    ),
    (
        "objects.remove_small",
        "Run it after a segmentation to take the specks out: an object under `min_cells` (or `min_area`) goes into \
         the neighbour most like it (`similar`, by the bands' means) or the one with the longest border (`border`, \
         as raster.sieve), until none is left. `classes` limits it to the small objects of some classes.",
    ),
    (
        "objects.reclassify",
        "Rename classes and join them (`classes`: old name to new name) without classifying again; `merge` then \
         joins the neighbours that now share a class, which makes the objects of a coarser class map.",
    ),
    (
        "ml.pca",
        "Keep `standardise` on (the correlation) when the variables are in different units or ranges (a band in reflectance beside an elevation in metres), or the one with the largest numbers is the first component alone; turn it off for bands in the same unit, where their spread is the information. The default keeps the components that explain 95% of the variance: if the first few explain little (the `explained` table, the cumulative column), the variables are nearly independent and a PCA will not reduce them. The scores of other maps or a later date come from `ml.predict` with the model, which uses the means of this fit, not of the new data, so the same change means the same numbers. For a very large stack the fit uses a sample of a million cells (`sample` and `seed` to choose).",
    ),
    (
        "ml.kmeans",
        "Keep `standardise` on when the variables are in different units or ranges (a band in reflectance beside an \
         elevation in metres), or the one with the largest numbers decides the clusters alone; turn it off for bands \
         in the same unit. Without `k` the best silhouette from 2 to 10 is taken: look at the `choice` table, for a \
         silhouette that is low for every k (below 0.25) says the records have no clear groups, and the clusters \
         are then a way of cutting a continuum, not of finding classes. A `k` large for the records (a few \
         hundred per cluster at least) makes clusters of a handful of records that change with the seed: give \
         fewer, or more records (`sample`). The clusters are numbered by the first principal component of their \
         centres, so a number means the same from one run to the next, and `ml.predict` with the model gives \
         another image or date the nearest centre of this fit, not of the new data. A fit on very many cells uses \
         a sample of a million (`sample` and `seed` to choose); every cell then has its cluster.",
    ),
    (
        "ml.som",
        "Keep `standardise` on when the variables are in different units or ranges, or the one with the largest \
         numbers decides the map alone. The grid is about 5 sqrt(n) nodes (Vesanto's rule, said in the note): with \
         fewer than a few records per node (the `count` of the `codebook`, the `empty_nodes` of `quality`) the \
         nodes are filled by the neighbours and not by data, so give `rows` and `cols` for a smaller grid, or more \
         records. A `topographic_error` high (above 0.3) says the map is folded: more `iterations`, or a larger \
         `sigma`; the `quantisation_error` falls as the grid grows, so it only compares maps of the same size. \
         The `umatrix` column of the `codebook` is large across the gap between two types of records, and small \
         inside one: `clusters` k groups the nodes (Ward's method) and the groups are numbered by the first \
         principal component of their centres, so the colours do not change between runs. Each node weighs the \
         same in the grouping, however many records it holds: on a large grid of thinly filled nodes a small, \
         distinct type (a lake among objects) can share a group with another, and a smaller grid keeps it apart. \
         `training` online \
         takes a record at a time and is slow for very many cells: batch (the default) uses all the threads. \
         `ml.predict` with the model gives another image or date the nearest node of this fit, not of the new \
         data.",
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
