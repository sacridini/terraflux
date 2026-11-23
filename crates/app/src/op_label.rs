//! The name of an operator for people: `raster.reclassify` is "Reclassify", `fragmentation.fos` "Fragmentation (FOS)".
//! A node of the editor shows it in place of the operator's dotted name. A curated table comes first; any operator
//! not in it gets its name read plainly (the part after the dot, underscores as spaces, the known acronyms in
//! capitals). And whether a node's id is the one the app gave it (so the label can stand in for it on the node).

use terraflux_graph::valid_id;

/// The longest label.
const MAX: usize = 30;

/// Words kept in capitals when a label is derived from a name.
const ACRONYMS: &[&str] = &[
    "ahp", "api", "app", "crs", "csv", "dem", "eca", "fac", "fad", "fos", "gbif", "gdal", "glm", "gps", "h3", "hli",
    "hr", "hru", "hsp", "idw", "iic", "ini", "lisa", "lulc", "lulcc", "mess", "mnf", "mop", "mspa", "ndr", "ndvi",
    "nmf", "nsd", "pca", "rng", "rsp", "sam", "sd", "sdm", "sdr", "som", "spa", "spi", "srr", "stac", "sti", "tpi",
    "tri", "twi", "ud", "url", "vif", "vrm", "wgn", "wkt",
];

/// Operators whose derived label is poor, cryptic or the same as another's: sorted by operator name.
const OVERRIDES: &[(&str, &str)] = &[
    ("accuracy.assess", "Accuracy assessment"),
    ("accuracy.sample", "Accuracy sample"),
    ("accuracy.sample_size", "Accuracy sample size"),
    ("agents.map", "Agents to raster"),
    ("agents.parcels", "Agents from parcels"),
    ("agents.summary", "Agents summary"),
    ("agents.table", "Agents from table"),
    ("agents.update", "Update agents"),
    ("batch.run", "Run on many rasters"),
    ("bio.cluster", "Biodiverse cluster"),
    ("bio.h3", "Basedata (H3 cells)"),
    ("bio.hex", "Basedata (hexagons)"),
    ("bio.matrix", "Label matrix"),
    ("bio.occurrences", "Basedata (occurrences)"),
    ("bio.points", "Basedata (points)"),
    ("bio.properties", "Biodiverse properties"),
    ("bio.randomise", "Randomise analysis"),
    ("bio.raster", "Basedata (rasters)"),
    ("bio.region_grower", "Region grower"),
    ("bio.spatial", "Biodiverse spatial"),
    ("bio.table", "Basedata (table)"),
    ("bio.tree", "Phylogenetic tree"),
    ("bio.vector", "Basedata (vector)"),
    ("carbon.emissions", "Carbon emissions"),
    ("cars.step", "CARS step"),
    ("change.components", "Change components"),
    ("change.composite", "Composite resolution"),
    ("change.count", "Count changes"),
    ("change.crosstab", "Cross-tabulation"),
    ("change.fad", "Change in FAD"),
    ("change.fos", "Change in FOS"),
    ("change.intensity", "Intensity analysis"),
    ("change.mad", "Mean absolute deviation"),
    ("change.memberships", "Class memberships"),
    ("change.morphological", "Morphological change"),
    ("change.mosaic", "Mosaic change"),
    ("change.resolutions", "Differences by scale"),
    ("change.sample_to_population", "Sample to population"),
    ("change.simple", "Simple change"),
    ("change.suggest", "Suggest transitions"),
    ("change.transitions", "Transitions"),
    ("check.accuracy", "Check accuracy"),
    ("check.area_kept", "Check area kept"),
    ("check.assert", "Check expression"),
    ("check.demand", "Check demand met"),
    ("check.no_nodata", "Check no nodata"),
    ("check.range", "Check value range"),
    ("circuit.advanced", "Circuitscape advanced"),
    ("circuit.circuitscape_ini", "Circuitscape INI file"),
    ("circuit.network_advanced", "Network advanced"),
    ("circuit.network_pairwise", "Network pairwise"),
    ("circuit.omniscape", "Omniscape"),
    ("circuit.omniscape_ini", "Omniscape INI file"),
    ("circuit.one_to_all", "Circuitscape one-to-all"),
    ("circuit.pairwise", "Circuitscape pairwise"),
    ("clumondo.ages", "CLUMondo ages"),
    ("clumondo.step", "CLUMondo step"),
    ("conn.centrality", "Node centrality"),
    ("conn.components", "Graph components"),
    ("conn.connectivity_grid", "Connectivity grid"),
    ("conn.distances", "Patch distances"),
    ("conn.eca_change", "ECA change"),
    ("conn.eca_change_grid", "ECA change grid"),
    ("conn.focal_nodes", "Focal nodes"),
    ("conn.graph", "Habitat graph"),
    ("conn.graph_from_raster", "Graph from raster"),
    ("conn.graph_from_samc", "Graph from SAMC"),
    ("conn.graph_from_vector", "Graph from vector"),
    ("conn.link_change", "Link change"),
    ("conn.link_importance", "Link importance"),
    ("conn.make_grid", "Make grid"),
    ("conn.metapopulation", "Metapopulation"),
    ("conn.node_importance", "Node importance"),
    ("conn.nodes", "Graph nodes"),
    ("conn.overall", "Overall connectivity"),
    ("conn.protconn", "ProtConn"),
    ("conn.protconn_raster", "ProtConn (raster)"),
    ("conn.protconn_regions", "ProtConn by region"),
    ("conn.read_conefor", "Read Conefor files"),
    ("conn.settlement_probabilities", "Settlement probability"),
    ("conn.to_map", "Node values to raster"),
    ("conn.to_vector", "Nodes to vector"),
    ("conn.write_conefor", "Write Conefor files"),
    ("control.each", "For each"),
    ("control.group", "Group"),
    ("control.repeat", "Repeat"),
    ("control.submodel", "Submodel"),
    ("control.switch", "Switch"),
    ("cost.acc_cost", "Accumulated cost"),
    ("cost.areas", "Area transition"),
    ("cost.barriers", "Barrier transition"),
    ("cost.combine", "Combine transitions"),
    ("cost.commute_distance", "Commute distance"),
    ("cost.distance", "Least-cost distance"),
    ("cost.energy_landscape", "Energy landscape"),
    ("cost.geo_correction", "Geographic correction"),
    ("cost.mahal", "Mahalanobis transition"),
    ("cost.math", "Transition math"),
    ("cost.normalize", "Normalise transition"),
    ("cost.passage", "Random walk passage"),
    ("cost.path_inc", "Path overlap"),
    ("cost.rsp_distance", "RSP distance"),
    ("cost.shortest_path", "Least-cost path"),
    ("cost.sum_reciprocal", "Resistances in series"),
    ("cost.to_map", "Transition to raster"),
    ("cost.transition", "Cost transition"),
    ("crafty.allocate", "CRAFTY allocation"),
    ("data.download", "Download data"),
    ("data.environment90m", "Catchment environment"),
    ("data.stac", "Satellite scenes (STAC)"),
    ("demand.from_maps", "Demand from rasters"),
    ("demand.regional", "Regional demand"),
    ("demand.table", "Demand from table"),
    ("dinamica.continuous_patcher", "Continuous patcher"),
    ("dinamica.cost_map", "Dinamica cost map"),
    ("dinamica.destinations", "Dinamica destinations"),
    ("dinamica.pathways", "Dinamica pathways"),
    ("dinamica.roads", "Dinamica roads"),
    ("dinamica.step", "Dinamica step"),
    ("distance.euclidean", "Euclidean distance"),
    ("distance.influence_zones", "Influence zones"),
    ("distance.objects_proximity", "Object proximity"),
    ("distance.proximity", "Proximity"),
    ("dynamic.distance_to", "Distance to classes"),
    ("dynamic.share_of", "Neighbourhood share"),
    ("forest_code.app", "Forest Code: APP"),
    ("forest_code.reserve", "Forest Code: reserve"),
    ("fragmentation.clustering", "Area clustering (FAC)"),
    ("fragmentation.contagion", "Contagion"),
    ("fragmentation.entropy", "Spatial entropy"),
    ("fragmentation.entropy_map", "Entropy in a window"),
    ("fragmentation.fad", "Fragmentation (FAD)"),
    ("fragmentation.fad_multiscale", "FAD multiscale"),
    ("fragmentation.fos", "Fragmentation (FOS)"),
    ("fragmentation.gray_convolution", "Cover convolution"),
    ("fragmentation.landscape_mosaic", "Landscape mosaic"),
    ("futures.calibrate", "FUTURES calibration"),
    ("futures.devpressure", "Development pressure"),
    ("futures.patch_library", "Patch size library"),
    ("futures.simulation", "FUTURES simulation"),
    ("glm.coefficients", "Regression coefficients"),
    ("glm.fit", "Fit logistic regression"),
    ("glm.predict", "Predict probabilities"),
    ("hydro.along_network", "Along the river network"),
    ("hydro.breach", "Breach depressions"),
    ("hydro.centrality", "River link centrality"),
    ("hydro.connectivity_index", "Connectivity index (IC)"),
    ("hydro.dci", "Dendritic connectivity"),
    ("hydro.distance_to_channel", "Distance to channel"),
    ("hydro.fill", "Fill depressions"),
    ("hydro.flow_accumulation", "Flow accumulation"),
    ("hydro.flow_direction", "Flow direction"),
    ("hydro.hand", "Height above drainage"),
    ("hydro.network_distance", "River network distance"),
    ("hydro.snap", "Snap to streams"),
    ("hydro.spi", "Stream power index"),
    ("hydro.sti", "Sediment transport index"),
    ("hydro.stream_links", "Stream links"),
    ("hydro.streams", "Streams"),
    ("hydro.twi", "Wetness index (TWI)"),
    ("hydro.upstream", "Upstream land"),
    ("hydro.upstream_catchment", "Upstream catchment"),
    ("hydro.watersheds", "Watersheds"),
    ("landscape.accounting", "Object size classes"),
    ("landscape.contortion", "Contortion"),
    ("landscape.mspa", "MSPA"),
    ("landscape.mspa_tiled", "MSPA (tiled)"),
    ("landscape.parcellation", "Parcellation"),
    ("landscape.spa", "Simplified pattern (SPA)"),
    ("linkage.barriers", "Linkage barriers"),
    ("linkage.pathways", "Linkage pathways"),
    ("linkage.pinchpoints", "Pinch points"),
    ("lulcc.history", "Class history"),
    ("lulcc.step", "CLUE-S step"),
    ("markov.fit", "Fit Markov chain"),
    ("markov.matrix", "Markov matrix"),
    ("metrics.class", "Class metrics"),
    ("metrics.cluster", "Cluster landscapes"),
    ("metrics.clustergram", "Clustergram"),
    ("metrics.correlation", "Metric correlation"),
    ("metrics.landscape", "Landscape metrics"),
    ("metrics.nmf", "Signature factors (NMF)"),
    ("metrics.patch", "Patch metrics"),
    ("metrics.pca", "Signature PCA"),
    ("metrics.sample", "Plot metrics"),
    ("metrics.series", "Metrics over time"),
    ("metrics.signatures", "Spatial signatures"),
    ("metrics.window", "Metrics in a window"),
    ("ml.fit", "Fit model"),
    ("ml.importance", "Predictor importance"),
    ("ml.kmeans", "K-means clusters"),
    ("ml.partial_dependence", "Partial dependence"),
    ("ml.pca", "Principal components"),
    ("ml.potential", "Class potential"),
    ("ml.predict", "Predict with model"),
    ("ml.som", "Self-organizing map"),
    ("molusce.step", "MOLUSCE step"),
    ("molusce.total", "MOLUSCE weights"),
    ("move.aeqd_crs", "Azimuthal projection"),
    ("move.as_tracks", "Make tracks"),
    ("move.azimuth", "Step azimuth"),
    ("move.corridor", "Corridor segments"),
    ("move.dbbmm_ud", "dBBMM distribution"),
    ("move.dbbmm_variance", "dBBMM variance"),
    ("move.dbgb_ud", "dBGB distribution"),
    ("move.dbgb_variance", "dBGB variance"),
    ("move.distance", "Step distance"),
    ("move.distribution", "Step distribution"),
    ("move.extract_covariates", "Extract covariates"),
    ("move.filter_gps_quality", "Filter GPS quality"),
    ("move.filter_min_n_burst", "Filter small bursts"),
    ("move.filter_movebank_visible", "Filter hidden locations"),
    ("move.filter_per_interval", "One per interval"),
    ("move.filter_unique", "Drop duplicate times"),
    ("move.fit_distribution", "Fit step distribution"),
    ("move.fit_issf", "Step selection function"),
    ("move.flag_speed", "Flag fast steps"),
    ("move.home_range", "Home range"),
    ("move.home_range_overlap", "Home range overlap"),
    ("move.intensity_use", "Intensity of use"),
    ("move.interpolate", "Interpolate locations"),
    ("move.issf_model", "Step selection model from coefficients"),
    ("move.log_rss", "Log relative selection strength"),
    ("move.mask_segments", "Mask segments"),
    ("move.max_speed", "Maximum speed"),
    ("move.nsd", "Net squared displacement"),
    ("move.path_density", "Density of simulated paths"),
    ("move.random_numbers", "Draw random steps"),
    ("move.random_steps", "Random control steps"),
    ("move.read_tracks", "Read tracks"),
    ("move.resample", "Resample tracks"),
    ("move.resistance", "Resistance from a step selection model"),
    ("move.segments", "Step segments"),
    ("move.simulate_brownian", "Simulate Brownian motion"),
    ("move.simulate_path", "Simulate paths from a step selection model"),
    ("move.speed", "Step speed"),
    ("move.steps", "Steps"),
    ("move.suggest_window", "Suggest window size"),
    ("move.surface_distance", "Step length on relief"),
    ("move.thin_distance", "Thin by distance"),
    ("move.thin_time", "Thin by time"),
    ("move.time_lags", "Time lags"),
    ("move.time_of_day", "Day or night"),
    ("move.track_data", "Track data"),
    ("move.track_lines", "Track lines"),
    ("move.track_summary", "Track summary"),
    ("move.turn_angle", "Turn angle"),
    ("move.ud_distance", "UD distance"),
    ("move.ud_outer_probability", "UD outer probability"),
    ("move.ud_volume", "UD volume"),
    ("move.validate", "Validate with tracks"),
    ("networks.conefor", "Conefor inputs"),
    ("networks.gtn_components", "Network components"),
    ("networks.gtn_importance", "Network importance"),
    ("nlm.binarize", "Binarize"),
    ("nlm.classify", "Make classes"),
    ("nlm.curds", "Curdling"),
    ("nlm.distance_gradient", "Distance gradient"),
    ("nlm.edge_gradient", "Edge gradient"),
    ("nlm.fbm", "Fractional Brownian"),
    ("nlm.gaussian_field", "Gaussian field"),
    ("nlm.image", "Synthetic image"),
    ("nlm.merge", "Merge landscapes"),
    ("nlm.mosaic_field", "Mosaic field"),
    ("nlm.mosaic_gibbs", "Gibbs mosaic"),
    ("nlm.mosaic_tess", "Voronoi mosaic"),
    ("nlm.mpd", "Midpoint displacement"),
    ("nlm.neigh", "Neighbourhood classes"),
    ("nlm.percolation", "Percolation"),
    ("nlm.planar_gradient", "Planar gradient"),
    ("nlm.random", "Random values"),
    ("nlm.random_cluster", "Random cluster"),
    ("nlm.random_rectangular_cluster", "Random rectangles"),
    ("nlm.rescale", "Rescale 0 to 1"),
    ("objects.accuracy", "Object accuracy"),
    ("objects.calc", "Object calculator"),
    ("objects.classify", "Classify objects"),
    ("objects.edit", "Edit objects"),
    ("objects.embed", "Object embeddings"),
    ("objects.extrema", "Object extrema"),
    ("objects.features", "Object features"),
    ("objects.folds", "Object folds"),
    ("objects.fuse", "Fuse objects"),
    ("objects.grow", "Grow objects"),
    ("objects.merge", "Merge objects"),
    ("objects.nearest", "Classify by nearest"),
    ("objects.reclassify", "Reclassify objects"),
    ("objects.remove_small", "Remove small objects"),
    ("objects.resize", "Resize objects"),
    ("objects.resolve", "Resolve hypotheses"),
    ("objects.rules", "Classify by rules"),
    ("objects.sample", "Sample objects"),
    ("objects.samples", "Training objects"),
    ("objects.split", "Split objects"),
    ("objects.temporal", "Objects over time"),
    ("objects.to_map", "Objects to raster"),
    ("objects.to_vector", "Objects to vector"),
    ("objects.transition_matrix", "Fuzzy transition matrix"),
    ("patches.observe", "Observe patches"),
    ("patches.settings", "Patch settings"),
    ("plugin.wasm", "WebAssembly plugin"),
    ("plus.demand", "PLUS demand"),
    ("prioritize.actions", "Multi-action planning"),
    ("prioritize.actions_raster", "Multi-action (raster)"),
    ("prioritize.adjacency_matrix", "Adjacency matrix"),
    ("prioritize.binary_stack", "Zones to layers"),
    ("prioritize.boundary_matrix", "Boundary matrix"),
    ("prioritize.calibrate", "Calibrate trade-offs"),
    ("prioritize.coarsen", "Coarsen problem"),
    ("prioritize.connectivity_matrix", "Connectivity matrix"),
    ("prioritize.decay_connectivity", "Dispersal decay matrix"),
    ("prioritize.evaluate", "Evaluate solution"),
    ("prioritize.h3_units", "H3 planning units"),
    ("prioritize.marxan", "Marxan"),
    ("prioritize.optimize", "Prioritise areas"),
    ("prioritize.protected_cells", "Selected cells"),
    ("prioritize.proximity_matrix", "Proximity matrix"),
    ("prioritize.proxirr", "ProxIRR"),
    ("prioritize.refine", "Refine selection"),
    ("prioritize.unit_points", "Unit points"),
    ("prioritize.zonation", "Zonation"),
    ("prioritize.zonation_project", "Zonation project"),
    ("raster.calc", "Raster calculator"),
    ("raster.cell_area", "Cell area"),
    ("raster.classify", "Classify raster"),
    ("raster.clip", "Clip raster"),
    ("raster.contour", "Contour lines"),
    ("raster.convert", "Convert raster"),
    ("raster.correlation", "Raster correlation"),
    ("raster.expand", "Expand classes"),
    ("raster.fill", "Fill gaps"),
    ("raster.focal", "Moving window"),
    ("raster.label", "Label patches"),
    ("raster.legend", "Legend"),
    ("raster.local_correlation", "Local correlation"),
    ("raster.majority", "Majority filter"),
    ("raster.match", "Match grid"),
    ("raster.mosaic", "Mosaic rasters"),
    ("raster.read", "Read raster"),
    ("raster.reclassify", "Reclassify"),
    ("raster.reproject", "Reproject raster"),
    ("raster.resample", "Resample raster"),
    ("raster.shrink", "Shrink classes"),
    ("raster.sieve", "Sieve patches"),
    ("raster.standardize", "Standardise raster"),
    ("raster.stats", "Raster statistics"),
    ("raster.tabulate", "Tabulate area"),
    ("raster.terrain", "Terrain (GDAL)"),
    ("raster.viewshed", "Viewshed"),
    ("raster.write", "Write raster"),
    ("raster.zonal", "Zonal statistics"),
    ("redd.allocate", "REDD allocation"),
    ("redd.assess", "REDD assessment"),
    ("redd.fit", "REDD fit"),
    ("redd.nrt", "REDD risk threshold"),
    ("redd.vulnerability", "REDD vulnerability"),
    ("restoration.corridor", "Restoration corridor"),
    ("restoration.cost_distance", "Restoration cost distance"),
    ("restoration.ecosystem_areas", "Ecosystem areas"),
    ("restoration.ecosystem_targets", "Ecosystem targets"),
    ("restoration.intactness", "Ecological intactness"),
    ("restoration.intactness_gradient", "Intactness gradient"),
    ("restoration.isochrone", "Isochrone map"),
    ("restoration.least_cost_path", "Restoration path"),
    ("restoration.original_vegetation", "Original vegetation"),
    ("restoration.plan", "Restoration plan"),
    ("restoration.resistance", "Resistance from habitat"),
    ("restoration.status", "Restoration status"),
    ("rng.numpy", "NumPy random"),
    ("rng.r", "R random"),
    ("sdm.accessible_area", "Accessible area"),
    ("sdm.area_of_habitat", "Area of habitat"),
    ("sdm.autocorrelation_range", "Autocorrelation range"),
    ("sdm.background", "Background points"),
    ("sdm.bias_surface", "Sampling bias surface"),
    ("sdm.boyce", "Boyce index"),
    ("sdm.buffer_folds", "Buffered folds"),
    ("sdm.checkerboard_folds", "Checkerboard folds"),
    ("sdm.clean_occurrences", "Clean occurrences"),
    ("sdm.dataset", "SDM dataset"),
    ("sdm.download_gbif", "Download GBIF records"),
    ("sdm.evaluate", "Evaluate SDM"),
    ("sdm.exdet", "Extrapolation (ExDet)"),
    ("sdm.fit", "Fit SDM"),
    ("sdm.importance", "SDM predictor importance"),
    ("sdm.jackknife_folds", "Jackknife folds"),
    ("sdm.maxnet", "MaxNet model"),
    ("sdm.maxnet_predict", "Predict suitability"),
    ("sdm.mess", "Novel conditions (MESS)"),
    ("sdm.migrate", "Species migration"),
    ("sdm.mop", "Novel conditions (MOP)"),
    ("sdm.omission_rate", "Omission rate"),
    ("sdm.one_per_cell", "One record per cell"),
    ("sdm.prepare", "Prepare SDM"),
    ("sdm.quadrant_folds", "Quadrant folds"),
    ("sdm.random_folds", "Random folds"),
    ("sdm.reference_data", "Cleaning reference data"),
    ("sdm.response_curves", "Response curves"),
    ("sdm.select_predictors", "Select predictors"),
    ("sdm.spatial_blocks", "Spatial blocks"),
    ("sdm.spatial_clusters", "Spatial clusters"),
    ("sdm.thin", "Thin records"),
    ("sdm.threshold", "Suitability threshold"),
    ("sdm.vif", "Variance inflation (VIF)"),
    ("segment.chessboard", "Chessboard segmentation"),
    ("segment.felzenszwalb", "Felzenszwalb"),
    ("segment.from_map", "Objects from raster"),
    ("segment.from_vector", "Objects from vector"),
    ("segment.meanshift", "Mean shift"),
    ("segment.multiresolution", "Multiresolution"),
    ("segment.optimize", "Optimise segmenter"),
    ("segment.quadtree", "Quadtree"),
    ("segment.quickshift", "Quick shift"),
    ("segment.region_growing", "Region growing"),
    ("segment.sam", "Segment Anything (SAM)"),
    ("segment.scale", "Suggest scales (ESP)"),
    ("segment.slic", "SLIC"),
    ("segment.snic", "SNIC"),
    ("segment.spectral_difference", "Spectral difference"),
    ("segment.threshold", "Multi-threshold"),
    ("segment.watershed", "Watershed segments"),
    ("sensitivity.analyze", "Sensitivity analysis"),
    ("sensitivity.design", "Sensitivity design"),
    ("services.carbon", "Carbon storage"),
    ("services.habitat_quality", "Habitat quality"),
    ("services.ndr", "Nutrient retention"),
    ("services.pollination", "Pollination"),
    ("services.sdr", "Sediment retention"),
    ("services.seasonal_water_yield", "Seasonal water yield"),
    ("services.water_yield", "Water yield"),
    ("stack.classes", "Stack by class"),
    ("stack.extract_layer", "Extract layer"),
    ("stack.of", "Stack of maps"),
    ("stack.reduce", "Reduce stack"),
    ("stack.rename", "Rename layers"),
    ("stack.set", "Set stack layer"),
    ("stats.getis_ord", "Hot spots (Gi*)"),
    ("stats.idw", "Inverse distance (IDW)"),
    ("stats.kriging", "Kriging"),
    ("stats.lisa", "Local Moran (LISA)"),
    ("stats.moran", "Global Moran's I"),
    ("stats.trend", "Trend (Mann-Kendall)"),
    ("stats.variogram", "Variogram"),
    ("surface.aacf", "Areal autocorrelation"),
    ("surface.bearing", "Bearing area curve"),
    ("surface.curvature", "Curvature"),
    ("surface.dissection", "Dissection"),
    ("surface.fit_plane", "Fit surface trend"),
    ("surface.geomorphons", "Geomorphons"),
    ("surface.gravity", "Gravity potential"),
    ("surface.hilltop", "Hilltops"),
    ("surface.hli", "Heat load index"),
    ("surface.hsp", "Slope position (HSP)"),
    ("surface.metrics", "Surface metrics"),
    ("surface.peaks", "Peaks and valleys"),
    ("surface.spherical_sd", "Spherical deviation"),
    ("surface.srr", "Relief ratio"),
    ("surface.terrain", "Slope or aspect"),
    ("surface.tpi", "Position index (TPI)"),
    ("surface.trasp", "Transformed aspect"),
    ("surface.tri", "Ruggedness index (TRI)"),
    ("surface.vrm", "Vector ruggedness"),
    ("surface.window", "Windowed surface metrics"),
    ("swat.channel_map", "SWAT+ channel map"),
    ("swat.from_tables", "SWAT+ from tables"),
    ("swat.hru_map", "SWAT+ HRU map"),
    ("swat.load", "Load SWAT+ project"),
    ("swat.reach_discharge", "Reach discharge"),
    ("swat.run", "Run SWAT+"),
    ("swat.set", "Set SWAT+ parameter"),
    ("swat.watershed", "SWAT+ watershed"),
    ("swat.wgn", "Weather generator"),
    ("table.aggregate", "Group and summarise"),
    ("table.calc", "Table calculator"),
    ("table.filter", "Filter rows"),
    ("table.join", "Join tables"),
    ("table.of", "Table of values"),
    ("table.pivot", "Pivot table"),
    ("table.read", "Read table"),
    ("table.select", "Select columns"),
    ("table.sort", "Sort rows"),
    ("table.write", "Write table"),
    ("uncertainty.delta", "Delta-method uncertainty"),
    ("uncertainty.interval", "Confidence interval"),
    ("uncertainty.monte_carlo", "Monte Carlo"),
    ("uncertainty.product_sd", "SD of a product"),
    ("uncertainty.sum_sd", "SD of a sum"),
    ("validate.agreement", "Map agreement"),
    ("validate.error_budget", "Error budget"),
    ("validate.futures", "FUTURES validation"),
    ("validate.futures_grid", "FUTURES validation grid"),
    ("validate.kappa", "Kappa comparison"),
    ("validate.similarity", "Fuzzy similarity"),
    ("validate.three_map", "Three-map comparison"),
    ("value.calc", "Value calculator"),
    ("vector.buffer", "Buffer"),
    ("vector.calc", "Vector calculator"),
    ("vector.clip", "Clip vector"),
    ("vector.density", "Point density"),
    ("vector.dissolve", "Dissolve"),
    ("vector.distance", "Distance to features"),
    ("vector.exact_sum", "Coverage-weighted sum"),
    ("vector.extract", "Raster values at points"),
    ("vector.filter", "Filter features"),
    ("vector.from_table", "Points from table"),
    ("vector.geometry", "Geometry measures"),
    ("vector.grid_zones", "Grid zones"),
    ("vector.join", "Join table to vector"),
    ("vector.overlay", "Overlay"),
    ("vector.points", "Points in polygons"),
    ("vector.polygonize", "Raster to polygons"),
    ("vector.rasterize", "Vector to raster"),
    ("vector.read", "Read vector"),
    ("vector.reproject", "Reproject vector"),
    ("vector.spatial_join", "Spatial join"),
    ("vector.write", "Write vector"),
    ("vector.zonal", "Statistics by polygon"),
    ("view.plot", "Plot (retired)"),
    ("woe.fit", "Fit weights of evidence"),
    ("woe.probability", "WoE probability"),
    ("woe.weights", "Weights of evidence"),
];

/// A word with its first letter in capitals.
fn capital(w: &str) -> String {
    let mut c = w.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// A label read from the name: the part after the dot, in plain words. `read` and `write` alone say nothing, so
/// they take the group as their subject (`vector.read` is "Read vector").
fn derived(op: &str) -> String {
    let (group, part) = op.split_once('.').unwrap_or(("", op));
    if matches!(part, "read" | "write" | "load") && !group.is_empty() {
        return format!("{} {group}", capital(part));
    }
    let words: Vec<String> = part
        .split('_')
        .filter(|w| !w.is_empty())
        .enumerate()
        .map(|(i, w)| {
            if ACRONYMS.contains(&w) {
                w.to_uppercase()
            } else if i == 0 {
                capital(w)
            } else {
                w.to_string()
            }
        })
        .collect();
    if words.is_empty() { op.to_string() } else { words.join(" ") }
}

/// The label of an operator, short and in plain words: `raster.read` is "Read raster", `fragmentation.fos`
/// "Fragmentation (FOS)". The curated table first, else read from the name; never longer than 30 characters.
pub fn label(op: &str) -> String {
    let text = match OVERRIDES.binary_search_by(|(name, _)| (*name).cmp(op)) {
        Ok(i) => OVERRIDES[i].1.to_string(),
        Err(_) => derived(op),
    };
    if text.chars().count() <= MAX {
        return text;
    }
    let cut: String = text.chars().take(MAX - 1).collect();
    format!("{}…", cut.trim_end())
}

/// Whether a node's id is the one the app gives by itself (`fresh_id`): the operator's whole name with every
/// character that is not a letter or digit an underscore (`raster_calc`, `n_` first when that is no valid id),
/// then `_2`, `_3`... for the next of the same; older models and templates named a node by the part after the dot
/// (`fos`, `mspa`, `reclassify`), the same ways, so those count too.
pub fn is_auto_id(id: &str, op: &str) -> bool {
    let base: String = op.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
    let base = if valid_id(&base) { base } else { format!("n_{base}") };
    let short = op.rsplit('.').next().unwrap_or(op);
    let names = |s: &str| s == base || (!short.is_empty() && s == short);
    if names(id) {
        return true;
    }
    match id.rsplit_once('_') {
        Some((stem, n)) => !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && names(stem),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use terraflux_ops::Registry;

    #[test]
    fn table_is_sorted() {
        for w in OVERRIDES.windows(2) {
            assert!(w[0].0 < w[1].0, "{} before {}", w[0].0, w[1].0);
        }
    }

    #[test]
    fn every_operator_has_a_short_unique_label() {
        let mut seen: HashMap<String, String> = HashMap::new();
        for s in Registry::standard().signatures() {
            let l = label(&s.name);
            assert!(!l.is_empty(), "{} has no label", s.name);
            assert!(l.chars().count() <= MAX, "{} -> {l} is too long", s.name);
            if let Some(other) = seen.insert(l.clone(), s.name.clone()) {
                panic!("{} and {} are both \"{l}\"", other, s.name);
            }
        }
    }

    #[test]
    fn overrides_name_registered_operators() {
        let names: Vec<String> = Registry::standard().signatures().into_iter().map(|s| s.name).collect();
        let stale: Vec<&str> = OVERRIDES.iter().map(|o| o.0).filter(|o| !names.iter().any(|n| n == o)).collect();
        assert!(stale.is_empty(), "labels of operators that do not exist: {stale:?}");
    }

    #[test]
    fn labels_of_known_operators() {
        let cases = [
            ("raster.read", "Read raster"),
            ("raster.reclassify", "Reclassify"),
            ("raster.calc", "Raster calculator"),
            ("vector.read", "Read vector"),
            ("table.write", "Write table"),
            ("stack.of", "Stack of maps"),
            ("fragmentation.fos", "Fragmentation (FOS)"),
            ("landscape.mspa", "MSPA"),
            ("prioritize.zonation", "Zonation"),
            ("nlm.random_cluster", "Random cluster"),
            ("control.repeat", "Repeat"),
            ("control.each", "For each"),
            ("hydro.hand", "Height above drainage"),
        ];
        for (op, want) in cases {
            assert_eq!(label(op), want, "{op}");
        }
    }

    #[test]
    fn labels_of_unknown_operators_are_derived() {
        assert_eq!(label("raster.some_new_thing"), "Some new thing");
        assert_eq!(label("x.read"), "Read x");
        assert_eq!(label("x.pca_scores"), "PCA scores");
        assert_eq!(label("lonely"), "Lonely");
        assert!(label("x.a_very_long_name_that_goes_on_and_on_and_on").chars().count() <= MAX);
    }

    #[test]
    fn automatic_ids() {
        assert!(is_auto_id("raster_calc", "raster.calc"));
        assert!(is_auto_id("raster_calc_3", "raster.calc"));
        assert!(is_auto_id("fos", "fragmentation.fos"));
        assert!(is_auto_id("fos_2", "fragmentation.fos"));
        assert!(is_auto_id("fragmentation_fos", "fragmentation.fos"));
        assert!(is_auto_id("mspa", "landscape.mspa"));
        assert!(is_auto_id("reclassify_12", "raster.reclassify"));
        assert!(!is_auto_id("forest", "raster.read"));
        assert!(!is_auto_id("landscape", "landscape.mspa"));
        assert!(!is_auto_id("my_calc", "raster.calc"));
        assert!(!is_auto_id("calc_x", "raster.calc"));
        assert!(!is_auto_id("raster_calc_", "raster.calc"));
        assert!(!is_auto_id("raster_calc_2b", "raster.calc"));
        assert!(!is_auto_id("", "raster.calc"));
    }
}
