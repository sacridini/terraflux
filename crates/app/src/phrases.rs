//! What a step does with its settings, in a sentence written by hand for every operator ("A Gaussian random
//! field of 100 × 100 cells: autocorrelation range 10 cells…"), shown in the document and the
//! report with the settings listed.
//!
//! A phrase is a template: `{name}` is a setting's value in words (the one given, else the operator's default),
//! `<port>` where an input comes from (a node's name, a model input's), and `[…]` a part said only when every
//! setting and input it names is given.

use terraflux_graph::{Node, OpSignature};

use crate::document::From;

/// The phrases, by operator.
const PHRASES: &[(&str, &str)] = &[
    ("raster.read", "Reads the rasters of {path}[, band {band}][, with {nodata} as no data]."),
    ("raster.write", "Writes <value> to {path}[ as {type}]."),
    ("vector.read", "Reads the features of {path}[, layer {layer}]."),
    ("vector.write", "Writes the features of <vector> to {path}[, layer {layer}]."),
    ("table.read", "Reads the table {path}."),
    ("move.read_tracks", "Reads the animal tracks of {path}."),
    ("view.plot", "Shows[ <value>][ <features>][ <table>][ over <under>][ as \"{title}\"]."),
    ("nlm.random", "A random map of {rows} × {columns} cells[ of side {resolution}], seed {seed}."),
    (
        "nlm.gaussian_field",
        "A Gaussian random field of {rows} × {columns} cells[ of side {resolution}]: autocorrelation range \
         {range} cells, variance {variance}, nugget {nugget}; seed {seed}.",
    ),
    (
        "nlm.random_cluster",
        "Random clusters on {rows} × {columns} cells[ of side {resolution}], a share {p} of the cells in clusters\
         [ ({neighbourhood} neighbours)][, the classes weighted {weights}]; seed {seed}.",
    ),
    (
        "nlm.fbm",
        "Fractional Brownian motion on {rows} × {columns} cells[ of side {resolution}], fractal dimension \
         {dimension}; seed {seed}.",
    ),
    (
        "nlm.planar_gradient",
        "A planar gradient on {rows} × {columns} cells[ of side {resolution}][, facing {direction}°]; seed {seed}.",
    ),
    (
        "nlm.distance_gradient",
        "A gradient of the distance to a rectangle[ ({origin})] on {rows} × {columns} cells; seed {seed}.",
    ),
    ("nlm.binarize", "<map> split into habitat and matrix, a share {breaks} of it habitat."),
    ("raster.reclassify", "<map> with its values reclassified: {rules}."),
    ("raster.reproject", "<raster> reprojected[ to {crs}][, cells of side {resolution}][, by {resampling}]."),
    ("raster.resample", "<raster> resampled to cells of side {resolution}[, by {resampling}]."),
    ("markov.fit", "The Markov chain of the changes from <t0> to <t1>[, over <steps> steps]."),
    ("glm.fit", "A logistic regression of each class of <landscape> on <drivers>[ (formulas {formulas})]."),
    ("glm.predict", "The probability of each class by the regression <model>, on <drivers>[ and <landscape>]."),
    (
        "woe.fit",
        "The weights of evidence of each change from <t0> to <t1> by <drivers>[, ranges by increments of \
         {increment}][, {categorical} taken as categories].",
    ),
    ("woe.probability", "The probability of each change by the weights <model>, on <landscape> and <drivers>."),
    ("demand.from_maps", "The area of each class over time, from <maps>[ dated {times}][, for {out_times}]."),
    ("dinamica.step", "A step of Dinamica's allocation of the changes <rates> on <landscape>, by <probability>."),
    ("lulcc.step", "A step of the allocation of <demand> on <landscape>, by <suitability>."),
    (
        "patches.observe",
        "The patches of the changes from <t0> to <t1>: how much they expand and their isometry[ ({isometry})].",
    ),
    ("metrics.class", "The metrics of each class of <map>[: {metrics}][, patches of {directions} neighbours]."),
    (
        "metrics.landscape",
        "The metrics of the whole landscape <map>[: {metrics}][, patches of {directions} neighbours].",
    ),
    ("metrics.patch", "The metrics of each patch of <map>[: {metrics}][, patches of {directions} neighbours]."),
    (
        "landscape.mspa",
        "MSPA's patterns of <map>[, its foreground {foreground}]: connected by {connectivity} neighbours, the \
         edges {edge_width} cell wide.",
    ),
    ("validate.agreement", "How well <simulated> agrees with <observed>, both from <start>."),
    ("validate.kappa", "The kappa of <simulated> against <reference>."),
    (
        "sdm.fit",
        "A species distribution model fitted to <records>[ on {predictors}][, its features {classes}]\
         [, regularisation {regmult}], output {type}.",
    ),
    (
        "sdm.area_of_habitat",
        "Each species' area of habitat within <ranges> on <land_cover>[, natural classes {natural}].",
    ),
    (
        "services.carbon",
        "The carbon stored in <lulc> by the pools <pools>[, and its change to <future>][ valued at {price} a ton].",
    ),
    (
        "prioritize.optimize",
        "The planning units chosen[ for <features>][ at the cost <cost>], the objective {objective}[ within a budget \
         of {budget}].",
    ),
    ("raster.calc", "Maps computed cell by cell from its inputs by a script."),
    ("stack.of", "Its inputs as the layers of a stack."),
    ("table.of", "A table written in the model, its columns {columns}."),
    ("table.write", "Writes <table> to {path}."),
    ("table.filter", "The rows of <table> where {expression}."),
    ("check.assert", "A check: {expression}[ ({message})]."),
    ("check.range", "A check that every value of <value> is in range[, at least {min}][, at most {max}]."),
    ("check.no_nodata", "A check that <map> has a value in every cell[ of <within>][, up to a share {max_share}]."),
    (
        "check.area_kept",
        "A check that the area[ of class {class}] is kept from <before> to <after>[, within <within>]\r
         [, up to {tolerance} cells].",
    ),
    ("check.accuracy", "A check that <map> agrees with <reference>[ by {measure}], at least {min}."),
    ("check.demand", "A check that <landscape> meets <demand> at step <step>[, up to {tolerance}]."),
    ("conn.to_map", "The values of <table> painted on the patches of <graph>[, the column {column}]."),
    (
        "conn.graph_from_raster",
        "A habitat graph of the patches[ of <map>][ of <vector>][, the habitat classes {habitat}], the distances \
         by {distance}[ over <resistance>][ up to {max_distance}].",
    ),
    ("conn.nodes", "The nodes of the patches[ of <map>][ of <vector>][, the habitat classes {habitat}]."),
    (
        "conn.node_importance",
        "Each node's importance in <graph>[ at a distance {distance}][, probability {probability}].",
    ),
    ("conn.overall", "The connectivity of <graph>[ at a distance {distance}][, probability {probability}]."),
    (
        "dynamic.distance_to",
        "The distance from each cell of <landscape> to the nearest cell[ of the classes {classes}][, over the \
         costs <cost>].",
    ),
    ("sdm.maxnet_predict", "The suitability by the model <model> on <env>[, output {type}]."),
    ("stack.set", "<stack> with <map> as its layer {name}."),
    ("hydro.flow_accumulation", "The flow accumulation of <dem> by {method}[, weighted by <weights>], in {units}."),
    ("hydro.fill", "<dem> with its depressions filled."),
    (
        "hydro.streams",
        "The streams of <accumulation>: the cells of an accumulation of {threshold} or more[, traced on <dem>].",
    ),
    ("rng.r", "R's random number generator, seed <seed>."),
    ("vector.filter", "The features of <vector> where {expression}."),
    ("vector.calc", "The attributes of <vector>[ ({column})] computed from the others by a script."),
    ("vector.extract", "The values of <map> at the points of <vector>, by {method}[, in a radius of {buffer}]."),
    ("vector.zonal", "The statistics of <map> in each polygon of <vector>[: {stats}]."),
    ("vector.points", "Points ({method}) inside the polygons <vector>[, {n} in each][, {spacing} apart]; seed {seed}."),
    ("vector.polygonize", "The patches of <map> as polygons[, {neighbours} neighbours]."),
    ("agents.update", "The agents <agents> updated by the rules: {rules}."),
    ("agents.parcels", "An agent per parcel of <parcels>, with the mean of the other maps in each."),
    ("agents.map", "The attribute {column} of <agents> painted on their parcels <parcels>."),
    ("agents.summary", "The number of agents in <agents> and the sum and mean of each attribute."),
    ("lulcc.history", "The steps each cell of <landscape> has had its class[, from <history>]."),
    (
        "fragmentation.fos",
        "The fragmentation of <map>[, its foreground {foreground}] at a fixed scale: {method} in a window of \
         {window} cells.",
    ),
    (
        "sdm.prepare",
        "The records <occurrences> prepared for a species distribution model on <env>[, the species {species}][, \
         thinned at {thin} m]; seed {seed}.",
    ),
    ("sdm.background", "{n} background points on <env>[ within <area>][, weighted by <bias>]; seed {seed}."),
    ("sdm.clean_occurrences", "The records <occurrences> cleaned of suspicious coordinates (action: {action})."),
    ("sdm.thin", "The records <occurrences> thinned to {distance} m apart, {reps} repetitions; seed {seed}."),
    ("sdm.mess", "The environmental similarity (MESS) of <env> to <reference>."),
    ("surface.terrain", "The {v} of <map>[ in {unit}][, {neighbors} neighbours]."),
    ("raster.terrain", "The {measure} of <raster>[, gdaldem's options {options}]."),
    ("surface.curvature", "The {type} curvature of <map>."),
    ("surface.tpi", "The topographic position index of <map>[, a window of {scale}][, {win}]."),
    (
        "nlm.mosaic_tess",
        "A Voronoi mosaic of random values on {rows} × {columns} cells[ of side {resolution}][, {germs} germs]; \
         seed {seed}.",
    ),
    (
        "nlm.percolation",
        "A random binary map of {rows} × {columns} cells[ of side {resolution}], each cell 1 with probability \
         {prob}; seed {seed}.",
    ),
    ("nlm.classify", "<map> in classes[: {n} by natural breaks][, the shares {weights}][ as in <real>]."),
    ("change.transitions", "The transitions between the maps of <maps>[ dated {times}]."),
    ("change.crosstab", "The transition matrix from <from> to <to>."),
    (
        "change.components",
        "The components of the difference[ from <from>][ to <to>][ along <maps>], the analysis of {analysis}.",
    ),
    ("change.simple", "The change of the foreground[ {foreground}] from <before> to <after>."),
    ("change.morphological", "The morphological change of the foreground[ {foreground}] from <before> to <after>."),
    (
        "change.fad",
        "The change of fragmentation (FAD) from <before> to <after>[, the foreground {foreground}], in windows of \
         {windows} cells.",
    ),
    ("restoration.plan", "A restoration plan by {method}[ within a budget of {budget}][, the target {target}]."),
    ("metrics.signatures", "The spatial signatures of <maps>[ in the zones <plots>]."),
    ("metrics.window", "The landscape metrics[ {metrics}] of <map> in a moving window."),
    ("networks.gtn_components", "The components of the habitat network of the MSPA map <map>."),
    ("circuit.pairwise", "Circuitscape's effective resistance between every pair of the points <points> on <habitat>."),
    (
        "circuit.omniscape",
        "Omniscape's connectivity on <resistance>[ from the sources <source>], in windows of radius {radius} \
         cells[, a target every {block_size} cells].",
    ),
    (
        "linkage.pathways",
        "Linkage Mapper's least-cost links and corridors between the cores <cores> over <resistance>[, the cores \
         linked by {adjacency} adjacency][, corridors cut at {width}].",
    ),
    (
        "linkage.pinchpoints",
        "Linkage Mapper's pinch points: Circuitscape's current in the corridors (width {width}) of the links <links> \
         between the cores <cores> over <resistance>[, in {mode} mode].",
    ),
    (
        "linkage.barriers",
        "Linkage Mapper's barriers: the gain of restoring circles of radius {min_radius}[ to {max_radius}] in the \
         links <links> between the cores <cores> over <resistance>.",
    ),
    ("cost.passage", "The passages of a random walk from <origins> to <goals> through <transition>[, theta {theta}]."),
    (
        "prioritize.evaluate",
        "The evaluation of the solution[ <solution>][ <selection>][ for <features>][ at the cost <cost>].",
    ),
    ("prioritize.zonation", "Zonation's priority ranking of <features> by the rule {rule}[ at the cost <cost>]."),
    (
        "prioritize.marxan",
        "Marxan's reserves[ for <features>][ at the cost <cost>][, boundary length modifier {blm}], {num_reps} runs.",
    ),
    (
        "services.pollination",
        "The pollination of crops by wild bees on <lulc>, by the table <biophysical>[ and the farms <farms>].",
    ),
    (
        "services.habitat_quality",
        "The habitat quality of <lulc> under the threats <threat_maps>, by the sensitivities <sensitivity>[, and \
         in the future <future>].",
    ),
    (
        "plus.demand",
        "The cells of each class at each step from <start>[ by the chain <markov>][ by the demand <demand>][ per \
         region <regional>][, {steps} steps].",
    ),
    (
        "landscape.accounting",
        "The objects of the foreground of <map>[ ({foreground})] in size classes, connected by {connectivity} \
         neighbours.",
    ),
    (
        "raster.sieve",
        "<map> with its patches of fewer than {threshold} cells merged into their largest neighbour[, {neighbours} \
         neighbours].",
    ),
    (
        "validate.similarity",
        "The fuzzy similarity of the change in <simulated> and <observed>, both from <start>, in windows of \
         {windows} cells.",
    ),
    (
        "accuracy.sample",
        "A stratified random sample of the cells of <map>[, {n} per stratum][ by <sizes>]; seed {seed}.",
    ),
    ("data.download", "Data downloaded[ from {source}][ ({variable})][ onto the grid of <like>][, cut to <area>]."),
    (
        "data.stac",
        "The scenes of {collection} from a STAC catalogue[ ({assets})][ of {datetime}][ onto the grid of <like>].",
    ),
    ("raster.legend", "The names and colours of the classes of <map>."),
    ("markov.matrix", "A Markov chain given in the model[, the classes {classes}], over {steps} steps."),
    ("raster.stats", "A summary of <map>[: {stats}]."),
    ("raster.clip", "<raster> cut to the extent {extent}."),
    ("distance.euclidean", "The Euclidean distance to the edge of the foreground of <map>[ ({foreground})]."),
    ("move.filter_unique", "The tracks <tracks> without duplicated times (criterion {criterion})."),
    (
        "uncertainty.monte_carlo",
        "{n} draws of {expression}, its parameters drawn from normal distributions; seed {seed}.",
    ),
    (
        "accuracy.assess",
        "The accuracy and the area of the classes of <map>, estimated from a stratified sample[ <points>][ <samples>][ \
         in the strata <strata>]; confidence {confidence}.",
    ),
    (
        "accuracy.sample_size",
        "The size of a stratified sample to assess <map>, for a standard error of {standard_error}[ ({n} points)], \
         allocation {allocation}.",
    ),
    ("agents.table", "Agents from a table written in the model[, or from <table>]."),
    (
        "batch.run",
        "Runs {operator} on each raster of {inputs}, in parallel[, {workers} at a time], and writes the results to \
         {output_dir}.",
    ),
    (
        "bio.cluster",
        "Biodiverse's cluster analysis of <basedata>[ by {index}][, linkage {linkage}][, cut into {k} clusters]; seed \
         {seed}.",
    ),
    ("bio.h3", "A basedata of <records> in H3 cells of resolution {resolution}, the labels from {label}."),
    ("bio.hex", "A basedata of <records> in {orientation} hexagons of size {size}, the labels from {label}."),
    ("bio.matrix", "Reads a Biodiverse label matrix from {path}."),
    (
        "bio.occurrences",
        "A basedata of the occurrence records[ of {path}][ <occurrences>] in cells of {cell_sizes}[, reprojected to \
         {to_crs}].",
    ),
    ("bio.points", "A basedata of the points <points> in cells of {cell_sizes}, the labels from {label}."),
    ("bio.properties", "The properties of the {of} of <basedata>, read from {path}."),
    (
        "bio.randomise",
        "{iterations} randomisations ({function}) of Biodiverse's analysis of <basedata>: {calculations}; seed \
         {rand_seed}.",
    ),
    (
        "bio.raster",
        "A basedata of the rasters <raster>[, the values of {threshold} or more counting 1][ in cells of \
         {cell_sizes}].",
    ),
    (
        "bio.region_grower",
        "Biodiverse's region grower on <basedata>[, maximising {index}][, cut into {k} regions]; seed {seed}.",
    ),
    (
        "bio.spatial",
        "Biodiverse's spatial analysis of <basedata> over the neighbourhood of each group: {calculations}.",
    ),
    (
        "bio.table",
        "A basedata of the table {path}, the groups from {group_columns} and the labels from {label_columns}.",
    ),
    ("bio.tree", "Reads the phylogenetic tree of {path}."),
    (
        "bio.vector",
        "A basedata of the features <features> in cells of {cell_sizes}, the labels from {label}[, measured by \
         {measure}].",
    ),
    (
        "carbon.emissions",
        "The carbon emitted and removed by land change along <maps>, by the stock of each class[ (dates {times})].",
    ),
    (
        "cars.step",
        "A step of PLUS's CARS allocation on <landscape>, by <probability>[ toward <demand>][ per region <regional>]; \
         seed <seed>.",
    ),
    (
        "change.composite",
        "The cross-tabulation of <comp> against <ref> by the composite method, in blocks of {factor} cells a side.",
    ),
    ("change.count", "How many times each cell changed along <maps>[ (dates {times})]."),
    ("change.fos", "The change of fragmentation from the FOS map <before> to <after>."),
    (
        "change.intensity",
        "The intensity analysis of the changes along <maps>[ (dates {times})], the gains of class {gain} and the \
         losses of class {loss}.",
    ),
    (
        "change.mad",
        "The mean absolute deviation between <grid1> and <grid2> and its components[, by the strata <strata>].",
    ),
    ("change.memberships", "The share of each class of <map> in blocks of {factor} cells a side."),
    ("change.mosaic", "The change of the landscape mosaic from <before> to <after>, in windows of {window} cells."),
    (
        "change.resolutions",
        "The quantity, exchange and shift differences between <comp> and <ref> at coarser resolutions, the blocks \
         growing by {factor}[, scaled to <population>].",
    ),
    (
        "change.sample_to_population",
        "The cross-tabulation <matrix> scaled to the population <population> of each class.",
    ),
    ("change.suggest", "The transitions worth modelling along <maps>: those of at least {least} % of the change."),
    (
        "circuit.advanced",
        "Circuitscape's currents and voltages on <conductance>, the current injected at <source> and drawn at \
         <ground>.",
    ),
    ("circuit.circuitscape_ini", "Runs the Circuitscape INI file {path}."),
    (
        "circuit.network_advanced",
        "Circuitscape's advanced mode on a network[ <graph>][ {graph_file}], the current injected at the sources and \
         drawn at the grounds.",
    ),
    (
        "circuit.network_pairwise",
        "Circuitscape's effective resistance between every pair of the focal nodes {focal_nodes} of a network[ \
         <graph>][ {graph_file}].",
    ),
    ("circuit.omniscape_ini", "Runs the Omniscape INI file {path}."),
    (
        "circuit.one_to_all",
        "Circuitscape's one-to-all mode on <habitat>, from each of the points <points> in turn[, the regions \
         <polygons>][, masked by <mask>].",
    ),
    (
        "clumondo.ages",
        "The years each cell of <landscape> has had its land system[, from <history>][, random up to {max_age}][; seed \
         <seed>].",
    ),
    (
        "clumondo.step",
        "A year of CLUMondo on <landscape>: the suitability of each land system[ by the regressions <model>][ given by \
         <suitability>], then the allocation that meets the demand.",
    ),
    (
        "conn.centrality",
        "The centralities of each node of <graph>[ at a distance {distance}][, probability {probability}].",
    ),
    ("conn.components", "The component of each node of <graph>[ at a distance {distance}]."),
    (
        "conn.connectivity_grid",
        "The connectivity of the protected areas <protected> in each cell of a grid over <region>[ (<grid>)], at a \
         distance {distance}.",
    ),
    (
        "conn.distances",
        "The distances between the patches[ of <map>][ of <vector>][, the habitat classes {habitat}], by {distance}[ \
         over <resistance>][ up to {max_distance}].",
    ),
    (
        "conn.eca_change",
        "The change of the equivalent connected area ({metric}) along the graphs <graphs>[ at a distance {distance}], \
         in a landscape of {landscape_area}.",
    ),
    (
        "conn.eca_change_grid",
        "The change of the equivalent connected area ({metric}) of <times> in each cell of a grid over <region>[ \
         (<grid>)], at a distance {distance}.",
    ),
    (
        "conn.focal_nodes",
        "The importance of each node of <vector> in its neighbourhood of {search_buffer}, by {metric} at a distance \
         {distance}.",
    ),
    ("conn.graph", "A habitat graph of the nodes <nodes> and the pairs <pairs>[ ({kind})]."),
    (
        "conn.graph_from_samc",
        "A habitat graph of the probabilities of settling between the patches <vector> over <resistance>[, the \
         absorption <absorption>].",
    ),
    (
        "conn.graph_from_vector",
        "A habitat graph of the features[ <vector>], the distances by {distance}[ over <resistance>][ up to \
         {max_distance}].",
    ),
    (
        "conn.link_change",
        "The change of {metric} in <graph> when its pairs change as in <changes>[, at a distance {distance}].",
    ),
    (
        "conn.link_importance",
        "The importance of each link of <graph>: the share of {metric} lost without it[, at a distance {distance}].",
    ),
    ("conn.make_grid", "A grid of cells[ of area {cell_area}] over <region>."),
    (
        "conn.metapopulation",
        "A metapopulation on the patches <patches> over {steps} steps, by Hanski's incidence function model (α \
         {alpha}, x {x}, y {y}, e {e}); seed {seed}.",
    ),
    (
        "conn.protconn",
        "ProtConn: the share of <region> protected and connected by <protected>, at a dispersal distance of \
         {distance}[ over <resistance>].",
    ),
    (
        "conn.protconn_raster",
        "ProtConn of the protected areas <protected>[ in <region>][ in <region_map>], at a dispersal distance of \
         {distance}.",
    ),
    (
        "conn.protconn_regions",
        "ProtConn of the protected areas <protected> in each region of <regions>, at a dispersal distance of \
         {distance}[ over <resistance>].",
    ),
    ("conn.read_conefor", "Reads a habitat graph from Conefor's files {nodes} and {connections}."),
    (
        "conn.settlement_probabilities",
        "The probabilities of settling between the nodes <vector> by a random walk over <resistance>[, the absorption \
         <absorption>].",
    ),
    ("conn.to_vector", "The table <table> joined onto the features of <graph>."),
    ("conn.write_conefor", "Writes <graph> to Conefor's files {nodes} and {connections}."),
    ("control.each", "Runs its steps once for each element of <over>."),
    ("control.group", "Runs its steps once, as a group."),
    ("control.repeat", "Repeats its steps <times> times."),
    ("control.submodel", "The model {path} run as one step."),
    ("control.switch", "Runs the branch chosen by <case>."),
    ("cost.acc_cost", "The accumulated cost from the nearest origin of <origins>, over the transition <transition>."),
    ("cost.areas", "The area transitions of the categories of <map>, {directions} neighbours."),
    ("cost.barriers", "The barrier transitions of <map>[ at the breaks {breaks}], {directions} neighbours."),
    ("cost.combine", "The transitions <a> and <b> combined by {operation}."),
    ("cost.commute_distance", "The commute distances of a random walk between the points <points> over <transition>."),
    ("cost.distance", "The least costs over <transition> from each point of <from>[ to each of <to>]."),
    (
        "cost.energy_landscape",
        "The energy a {mass} kg animal spends walking over <dem>[ ({model} model)], {neigh} neighbours, in {unit}.",
    ),
    ("cost.geo_correction", "<transition> corrected for the distances between the cells, type {type}."),
    ("cost.mahal", "The Mahalanobis transition of the maps <maps>, {directions} neighbours."),
    ("cost.math", "The function {function} of the values of <transition>[, with the value {value}]."),
    ("cost.normalize", "<transition> normalised by {method}."),
    (
        "cost.path_inc",
        "The overlap and non-overlap of the routes over <transition> from <origin> to each location of <from>[ and \
         <to>][, weighted by <weights>][, theta {theta}].",
    ),
    (
        "cost.rsp_distance",
        "The randomised shortest path distances over <transition> from each point of <from>[ to each of <to>], theta \
         {theta}.",
    ),
    ("cost.shortest_path", "The least-cost paths over <transition> from <origin> to each of <goals>."),
    ("cost.sum_reciprocal", "The transitions <a> and <b> as resistances in series."),
    ("cost.to_map", "A map of the connections of each cell of <transition>, by {reduce}."),
    (
        "cost.transition",
        "The transition between neighbouring cells of <map>, by the function {function} of their values, {directions} \
         neighbours.",
    ),
    (
        "crafty.allocate",
        "A step of CRAFTY's competition for land on <land>, by the capitals <capitals> and the demand <demand>; seed \
         <seed>.",
    ),
    (
        "data.environment90m",
        "The {stats} of the variables {variables} of Environment90m in each sub-catchment of <sub_catchments>.",
    ),
    ("demand.regional", "A demand per region of <regions>, the regions {ids}."),
    ("demand.table", "A demand given as a table in the model."),
    (
        "dinamica.continuous_patcher",
        "Dinamica's continuous Patcher: new patches of mean size {mean}, isometry {isometry}, take the stock of \
         <stock> by <probability> until <quota> is met[; seed <seed>].",
    ),
    (
        "dinamica.cost_map",
        "The cost of reaching each cell from the nearest source of <sources>, over the friction <friction>.",
    ),
    (
        "dinamica.destinations",
        "Destinations drawn by the attraction <attraction>, {per_quadrant} per quadrant[ (divisions {quadrants})][, \
         {min_distance} apart][; seed <seed>].",
    ),
    (
        "dinamica.pathways",
        "Paths from <sources> down to the network <network>[ along the costs <cost>][ along the directions \
         <directions>].",
    ),
    (
        "dinamica.roads",
        "A step of road building: new roads from <network> to ends drawn by <attraction>, over the friction \
         <friction>[, within <mask>][; seed <seed>].",
    ),
    (
        "distance.influence_zones",
        "The zones of influence of the foreground objects of <map>[ ({foreground})][, objects of {min_area} cells or \
         more].",
    ),
    (
        "distance.objects_proximity",
        "The proximity between neighbouring foreground objects of <map>[ ({foreground})][, objects of {min_area} cells \
         or more].",
    ),
    (
        "distance.proximity",
        "The distance from each cell of <raster> to the nearest cell of the values {values}[, in {units}][, up to \
         {max_distance}].",
    ),
    (
        "dynamic.share_of",
        "The share of the cells around each cell of <landscape> in the classes {classes}[, a window of {window}].",
    ),
    (
        "forest_code.app",
        "The permanent preservation areas (APP) of the Forest Code[ along the rivers <rivers>][ around the lakes \
         <lakes>][ around the springs <springs>][ on the slopes <slope>][ on the hilltops <hilltops>].",
    ),
    (
        "forest_code.reserve",
        "The balance of the Reserva Legal of each rural property of <properties>, by its native vegetation \
         <vegetation>[ and the APPs <app>].",
    ),
    (
        "fragmentation.clustering",
        "The foreground area clustering of <map>[ ({foreground})] in a window of {window} cells.",
    ),
    (
        "fragmentation.contagion",
        "The contagion of the foreground of <map>[ ({foreground})] in a window of {window} cells.",
    ),
    ("fragmentation.entropy", "The spatial joint entropy of the whole map <map>[, its foreground {foreground}]."),
    (
        "fragmentation.entropy_map",
        "The spatial joint entropy of <map>[, its foreground {foreground}] in a window of {window} cells.",
    ),
    (
        "fragmentation.fad",
        "The forest area density (FAD) of <map>[, its foreground {foreground}] in a window of {window} cells.",
    ),
    (
        "fragmentation.fad_multiscale",
        "The forest area density (FAD) of <map>[, its foreground {foreground}] in windows of {windows} cells.",
    ),
    ("fragmentation.gray_convolution", "The {metric} of the cover <map> in a window of {window} cells."),
    (
        "fragmentation.landscape_mosaic",
        "The landscape mosaic of <map>: the mix of agriculture, natural and developed land in a window of {window} \
         cells, in {classes} classes.",
    ),
    (
        "futures.calibrate",
        "The calibration of FUTURES' patches on the development from <landscape> to <end>, {repeat} simulations per \
         combination; seed {seed}.",
    ),
    ("futures.devpressure", "The development pressure of <developed> in a radius of {size} cells, by {method}."),
    (
        "futures.patch_library",
        "The sizes of the patches developed from <start> to <end>[ in each subregion of <subregions>].",
    ),
    (
        "futures.simulation",
        "FUTURES' patch growing on <landscape>, from the development pressure <pressure>[ in the subregions \
         <subregions>]; seed <seed>.",
    ),
    ("glm.coefficients", "Logistic regressions given in the model."),
    (
        "hydro.along_network",
        "The stretches of river from each point of <points> along <streams>, {upstream} upstream and {downstream} \
         downstream, by the directions of <dem>.",
    ),
    ("hydro.breach", "<dem> with its depressions breached by channels of up to {max_length} cells."),
    (
        "hydro.centrality",
        "The place of each link of <links> in its river network: its degree, Shreve magnitude, betweenness and \
         closeness.",
    ),
    (
        "hydro.connectivity_index",
        "The index of connectivity of <dem>, weighted by <weight>, routed by {method}[, the streams <streams>].",
    ),
    (
        "hydro.dci",
        "The dendritic connectivity index of the river network <streams> cut by the barriers <barriers>, over <dem>[, \
         weighted by <weights>].",
    ),
    (
        "hydro.distance_to_channel",
        "The mean distance water travels from each cell of <dem> to the streams <streams>[, weighted by <weights>].",
    ),
    ("hydro.flow_direction", "The direction each cell of <dem> drains to, by {method}."),
    (
        "hydro.hand",
        "The height of each cell of <dem> above the nearest drainage[ <streams>][ (an accumulation of {threshold} \
         cells or more)].",
    ),
    (
        "hydro.network_distance",
        "The distance along the river network of <dem> between every two points of <points>[, moved onto <streams>].",
    ),
    (
        "hydro.snap",
        "The points <points> moved onto the streams <streams>[, at most {distance} away][, of an accumulation of \
         {min_accumulation} or more].",
    ),
    (
        "hydro.spi",
        "The stream power index of <dem>, the contributing area by {method}[ or <sca>][, the slope <slope>].",
    ),
    (
        "hydro.sti",
        "The sediment transport index of <dem>, the contributing area by {method}[ or <sca>][, the slope <slope>], \
         exponents {sca_exponent} and {slope_exponent}.",
    ),
    (
        "hydro.stream_links",
        "The links of the stream network <streams> along the directions of <dem>[, ending at <outlets>].",
    ),
    (
        "hydro.twi",
        "The topographic wetness index of <dem>, the contributing area by {method}[ or <sca>][, the slope <slope>].",
    ),
    (
        "hydro.upstream",
        "The land upstream of each link or point on <dem>[ (<links>)][ (<points>)][, the statistics of <values>][, the \
         share of each class of <classes>].",
    ),
    ("hydro.upstream_catchment", "The catchment of each point of <points> on <dem>[, moved onto <streams>]."),
    ("hydro.watersheds", "The area draining to each seed of <seeds> along the directions of <dem>."),
    (
        "landscape.contortion",
        "The contortion of the boundary of each foreground patch of <map>[ ({foreground})], patches of {min_area} \
         cells or more.",
    ),
    (
        "landscape.mspa_tiled",
        "MSPA's patterns of <map>[, its foreground {foreground}] in tiles of {tile_size} cells overlapping by \
         {overlap}: connected by {connectivity} neighbours, the edges {edge_width} cell wide.",
    ),
    (
        "landscape.parcellation",
        "The parcels of each class of <map> and their parcellation index, {connectivity} neighbours.",
    ),
    (
        "landscape.spa",
        "The simplified pattern analysis (SPA) of <map>[, its foreground {foreground}], in {variant} classes.",
    ),
    ("metrics.cluster", "{k} groups of the rows of <table> with similar metrics[, by {method}]; seed {seed}."),
    (
        "metrics.clustergram",
        "Schonlau's clustergram of <table>, from {k_min} to {k_max} clusters[, by {method}]; seed {seed}.",
    ),
    ("metrics.correlation", "The correlation between the metrics of <table>, by {method}."),
    ("metrics.nmf", "{factors} non-negative factors of the metrics of <table>."),
    ("metrics.pca", "The principal components of the metrics of <table>[, {components} kept]."),
    (
        "metrics.sample",
        "The landscape metrics[ {metrics}] of <map> in the plots <plots>[ of size {size}], at the {level} level.",
    ),
    (
        "metrics.series",
        "The landscape metrics[ {metrics}] along the maps <maps>[ in the zones <plots>], at the {level} level.",
    ),
    (
        "ml.fit",
        "A random forest or gradient boosting[ ({method})] fitted to the records[ of <table>][ on the maps \
         <predictors>][, learning <target>][, cross-validated by the folds in {fold}][, cross-validated in {cv} random \
         folds][; seed {seed}].",
    ),
    (
        "ml.importance",
        "The permutation importance of each predictor of <model> on <table>, {n_repeats} shuffles; seed \
         {random_state}.",
    ),
    (
        "ml.partial_dependence",
        "How the prediction of <model> on <table> follows the predictor {predictor}, a grid of {grid_resolution} \
         values.",
    ),
    (
        "ml.pca",
        "The principal components of the variables[ of <table>][ of the maps <predictors>][ and the later maps <later>][, {change}][, by the {method}][, {components} kept][, by the covariance][; seed {seed}].",
    ),
    (
        "ml.kmeans",
        "The k-means clusters of the variables[ of <table>][ of the maps <predictors>][, {k} clusters][, not \
         standardised][; seed {seed}].",
    ),
    (
        "ml.som",
        "A self-organizing map of the variables[ of <table>][ of the maps <predictors>][, {rows} rows of {cols} nodes][, \
         {topology}][, {training} training][, the nodes in {clusters} groups][; seed {seed}].",
    ),
    (
        "ml.potential",
        "The potential of each class of <landscape>[ ({target})] from the drivers <drivers>[ and the later map \
         <later>][, a {method} model][; seed {seed}].",
    ),
    (
        "ml.predict",
        "The prediction of the model <model>[ on the maps <predictors>][ on the rows of <table>][, novelty by {novelty}].",
    ),
    ("ml.read", "The fitted model read from the file {path}."),
    ("ml.write", "The model <model> written to the file {path}."),
    (
        "molusce.step",
        "An iteration of MOLUSCE's simulator on <landscape>[, by the weights <total>][, by the potentials \
         <potential>].",
    ),
    ("molusce.total", "MOLUSCE's total weight of each transition from the drivers <drivers>."),
    ("move.aeqd_crs", "<tracks> in an azimuthal equidistant projection centred on their {center}, in {units}."),
    ("move.as_tracks", "Tracks from the locations[ of <vector>][ of <table>][ (columns {x} and {y})][ in {crs}]."),
    ("move.azimuth", "The direction of each step of <tracks>, clockwise from north, in {units}."),
    (
        "move.corridor",
        "The segments of <tracks> that run along a corridor: faster than [{speed_threshold} m/s][the upper quartile \
         of the speeds], with the directions of their neighbours in agreement[ (circular variance up to \
         {circvar_threshold})].",
    ),
    (
        "move.dbbmm_ud",
        "The utilisation distribution of the dynamic Brownian bridge of <tracks>, a band for each track[ on the grid of \
         <like>][ (location error {location_error})].",
    ),
    (
        "move.dbbmm_variance",
        "The motion variance of the dynamic Brownian bridge along each track of <tracks>, in windows of {window_size} \
         locations[ (location error {location_error})].",
    ),
    (
        "move.dbgb_ud",
        "The utilisation distribution of the directional Brownian bridge of <tracks>, a band for each track[ on the \
         grid of <like>][ (location error {location_error})].",
    ),
    (
        "move.dbgb_variance",
        "The variances along and across the movement of the directional Brownian bridge of <tracks>, in windows of \
         {window_size} locations[ (location error {location_error})].",
    ),
    ("move.distance", "The distance from each location of <tracks> to the next[, in {units}]."),
    ("move.distribution", "A {name} distribution of step lengths or turn angles."),
    (
        "move.extract_covariates",
        "The values of the rasters connected at the locations of <tracks> (steps: {where})[, summarised along the \
         steps: {stats}].",
    ),
    ("move.filter_gps_quality", "<tracks> without the locations of an unreliable GPS geometry."),
    ("move.filter_min_n_burst", "The bursts of <tracks> of at least {min_n} locations."),
    ("move.filter_movebank_visible", "The tracks <tracks> without the locations Movebank hides."),
    ("move.filter_per_interval", "One location of <tracks> per {unit} (criterion {criterion})."),
    ("move.fit_distribution", "A {name} distribution fitted to <values>."),
    (
        "move.fit_issf",
        "A step selection function fitted to <steps>[ with the formula {formula}][ and the strata {strata}], by \
         conditional logistic regression.",
    ),
    (
        "move.home_range",
        "The {method} home range of each track of <tracks>, levels {levels}[, kernel bandwidth {bandwidth}][ ({type} \
         neighbourhoods of {n})][ on the cells of <like>].",
    ),
    (
        "move.home_range_overlap",
        "The {type} overlap between the home ranges[ of <polygons>][ of <ud>], {which} pairs[, conditional: \
         {conditional}].",
    ),
    (
        "move.intensity_use",
        "The intensity of use of each track of <tracks>: its length over the square root of the area of its minimum \
         convex polygon.",
    ),
    (
        "move.interpolate",
        "Locations of <tracks> interpolated[ at the times {times}][, gaps of more than {max_time_lag} s left empty].",
    ),
    (
        "move.flag_speed",
        "The locations of <tracks> next to a step faster than [{v_max} m/s][the maximum speed of a {mass} kg animal] \
         flagged.",
    ),
    (
        "move.issf_model",
        "A step selection model from the coefficients {coefficients}, to draw candidate steps from <sl_distr> and          <ta_distr>.",
    ),
    (
        "move.log_rss",
        "The log relative selection strength of the covariates x1 against x2 under <model>[, with a {ci} interval \
         at {level}].",
    ),
    (
        "move.mask_segments",
        "The bridges of <tracks> left out of the distribution[ ({gap} hours of gap or more)][ (those in {column})].",
    ),
    ("move.max_speed", "The maximum speed of a {mass} kg animal ({mode}), from Hirt et al.'s scaling law."),
    ("move.nsd", "The net squared displacement of each location of <tracks> from the first of its track."),
    ("move.path_density", "The share of the positions of <paths> in each cell of <like>[, one band for each {by}]."),
    ("move.random_numbers", "{n} draws of the distribution <distribution>; seed {seed}."),
    (
        "move.random_steps",
        "{n_control} random steps for each observed step of <steps>[, lengths from <sl_distr>][, turn angles from \
         <ta_distr>]; seed {seed}.",
    ),
    ("move.resample", "The locations of <tracks> resampled every {rate}, tolerance {tolerance}, in bursts."),
    (
        "move.resistance",
        "A resistance surface (transform {transform}) from the habitat terms of <model> over the covariates.",
    ),
    ("move.segments", "The steps of <tracks> as lines."),
    ("move.simulate_brownian", "{tracks} tracks of Brownian motion, diffusion {sigma}; seed {seed}."),
    (
        "move.simulate_path",
        "{n_paths} path(s) of {n_steps} steps simulated from <model> over the covariates, {n_control} candidate steps          each; seed {seed}.",
    ),
    ("move.speed", "The speed of each step of <tracks>[, in {units}]."),
    ("move.steps", "The steps between consecutive locations of <tracks>: their length, direction and turn angle."),
    (
        "move.suggest_window",
        "A window size and margin for the motion variance of <tracks>, covering about {target_hours} hours.",
    ),
    ("move.surface_distance", "The length of each step of <steps> measured on the relief <dem>."),
    (
        "move.thin_distance",
        "<tracks> thinned to locations about {distance} m of travel apart (method {method}[, tolerance {tolerance} m]).",
    ),
    (
        "move.thin_time",
        "<tracks> thinned to locations about {interval} apart[, within {tolerance}] (criterion {criterion}).",
    ),
    ("move.time_lags", "The time from each location of <tracks> to the next, in {units}."),
    ("move.time_of_day", "Day or night at each location of <tracks>."),
    ("move.track_data", "A row per track of <tracks> with the columns that do not change within it."),
    ("move.track_lines", "Each track of <tracks> as a line through its locations."),
    ("move.track_summary", "The distance, straightness and sinuosity of each track of <tracks>."),
    ("move.turn_angle", "The change of direction at each location of <tracks>, in {units}."),
    (
        "move.ud_outer_probability",
        "The probability of the cells at least as likely as the one of each location of <points>, under the \
         distribution <ud>.",
    ),
    (
        "move.ud_volume",
        "The volume of the distribution <ud>: each cell's cumulative probability from the largest down[, with the \
         contours at {levels}].",
    ),
    (
        "move.ud_distance",
        "The earth mover's distance between the utilisation distributions of <ud>[ and <ud_b>] (method {method}).",
    ),
    (
        "move.validate",
        "How well the connectivity surface <surface> ({kind}) captures the use of the animals <use>: the rank \
         correlation, the share of the use inside the corridors and the use against the area.",
    ),
    ("networks.conefor", "The habitat network of the MSPA map <map> as Conefor's inputs."),
    (
        "networks.gtn_importance",
        "The importance of each link and stepping stone of the habitat network of the MSPA map <map>.",
    ),
    (
        "nlm.curds",
        "A hierarchical binary map by curdling, the shares of matrix {curds} at the splits {steps}[, wheyed by \
         {wheyes}]; seed {seed}.",
    ),
    (
        "nlm.edge_gradient",
        "A gradient peaking in the middle of {rows} × {columns} cells[ of side {resolution}][, facing {direction}°]; \
         seed {seed}.",
    ),
    ("nlm.merge", "<primary> merged with <secondary> scaled by {scaling}."),
    (
        "nlm.mosaic_field",
        "A mosaic random field of {n} mosaics on {rows} × {columns} cells[ of side {resolution}]; seed {seed}.",
    ),
    (
        "nlm.mosaic_gibbs",
        "A Voronoi mosaic of {patch_classes} classes on {rows} × {columns} cells[ of side {resolution}], {germs} germs \
         at least {r} cells apart; seed {seed}.",
    ),
    (
        "nlm.mpd",
        "A fractal surface by midpoint displacement on {rows} × {columns} cells[ of side {resolution}], roughness \
         {roughness}; seed {seed}.",
    ),
    (
        "nlm.neigh",
        "A map of {categories} categories grown by neighbourhood on {rows} × {columns} cells[ of side {resolution}] \
         (probabilities {p_neigh} and {p_empty}); seed {seed}.",
    ),
    (
        "nlm.random_rectangular_cluster",
        "Overlapping rectangles of random values on {rows} × {columns} cells[ of side {resolution}], sides of {minl} \
         to {maxl} cells; seed {seed}.",
    ),
    ("nlm.rescale", "<map> rescaled to 0-1."),
    ("patches.settings", "The Expander and Patcher settings given in the model."),
    ("plugin.wasm", "A map computed cell by cell by the WebAssembly module {path}[, its function {function}]."),
    (
        "prioritize.actions",
        "Which actions against the threats <threats> to take in the planning units <pu> so that <features> reach their \
         targets, objective {model}[ within a budget of {budget}].",
    ),
    (
        "prioritize.actions_raster",
        "Which actions against the threats <threats> to take in the cells of <monitoring_cost> so that <features> \
         reach their targets, objective {model}[ within a budget of {budget}].",
    ),
    ("prioritize.adjacency_matrix", "Which planning units of <units> are adjacent, {directions} neighbours."),
    ("prioritize.binary_stack", "A 0/1 layer per zone of <zones>[, named {names}]."),
    ("prioritize.boundary_matrix", "The length of the boundary every two planning units of <units> share."),
    (
        "prioritize.calibrate",
        "Candidate solutions trading the cost off against the {penalty} penalty, by the {approach} approach[ for \
         <features>][ at the cost <cost>][, {n} candidates].",
    ),
    ("prioritize.coarsen", "The cost <cost> and the features <features> summed in blocks of {factor} cells a side."),
    ("prioritize.connectivity_matrix", "The connectivity of the adjacent planning units of <units>[ by <y>]."),
    (
        "prioritize.decay_connectivity",
        "The connectivity of the planning units from their distances <distances>, the dispersal probability 0.5 at the \
         median distance {median}.",
    ),
    ("prioritize.h3_units", "H3 hexagons of resolution {resolution} over <cost>, as planning units."),
    ("prioritize.protected_cells", "The cells selected by the solution <solution>."),
    ("prioritize.proximity_matrix", "Which planning units of <units> are within {distance} of each other."),
    ("prioritize.proxirr", "The irreplaceability (ProxIRR) of each planning unit[ of <pu>][ of the maps <features>]."),
    ("prioritize.refine", "The coarse selection <selection> back on the grid of <like>."),
    ("prioritize.unit_points", "A point per planning unit of <units>."),
    ("prioritize.zonation_project", "Runs the Zonation 5 project {path}[ with the switches {flags}][, rule {mode}]."),
    ("raster.cell_area", "The area of each cell of the grid of <map>, in {units}."),
    ("raster.classify", "The values of <map> in {n} classes by {method}[, the breaks {breaks}]."),
    ("raster.contour", "The contour lines of <map>[ every {interval}][ at the levels {levels}]."),
    ("raster.convert", "<raster> converted to {format}[ as {type}][, in {path}]."),
    ("raster.correlation", "The correlation between the layers of <x>[ and those of <y>], by {method}."),
    ("raster.expand", "The classes {classes} of <map> grown {cells} cells into the others, {neighbours} neighbours."),
    (
        "raster.fill",
        "<raster> with the cells without a value filled from up to {max_distance} cells away[, smoothed {smoothing} \
         times].",
    ),
    ("raster.focal", "The {stat} of the cells of <map> in a {shape} window[ of radius {radius}] around each cell."),
    ("raster.label", "The patches of <map> numbered[, the classes {classes}], {neighbours} neighbours."),
    (
        "segment.from_map",
        "The regions of <map> as objects, each group of connected cells of one value[ ({neighbours} neighbours)].",
    ),
    ("segment.chessboard", "<image> cut into squares[ of {size} cells] as objects[, within the zones of <bounds>]."),
    (
        "segment.quadtree",
        "<image> split into squares as objects while their values differ by more than {threshold}[, down to {min_size} cells][, within the zones of <bounds>].",
    ),
    ("segment.from_vector", "The polygons of <vector> as objects on the grid of <like>[ ({neighbours} neighbours)]."),
    (
        "segment.multiresolution",
        "<image> merged into objects by multiresolution segmentation[ at scale {scale}][, shape {shape}][, compactness {compactness}][, on top of the objects of <from>][, within the zones of <bounds>].",
    ),
    ("segment.slic", "<image> divided into SLIC superpixels[ of about {size} cells][, compactness {compactness}][, within the zones of <bounds>]."),
    ("segment.snic", "<image> divided into SNIC superpixels[ of about {size} cells][, compactness {compactness}][, within the zones of <bounds>]."),
    (
        "segment.felzenszwalb",
        "<image> segmented by Felzenszwalb and Huttenlocher's graph method[ with k {k}][, segments of at least {min_size} cells][, within the zones of <bounds>].",
    ),
    (
        "segment.threshold",
        "<image> cut into classes by thresholds[ in band {band}][ at {thresholds}][, or into {classes} classes by multi-level Otsu][, within the zones of <bounds>]; each connected piece of a class is an object.",
    ),
    (
        "segment.spectral_difference",
        "The neighbouring objects of <objects> whose means in <image> differ by less than {max_difference} merged[, the bands weighted {weights}][, within the zones of <bounds>].",
    ),
    (
        "segment.region_growing",
        "<image> segmented by region growing and merging (as GRASS's i.segment) at a threshold of {threshold}[, by the {similarity} distance][, from the seeds of <seeds>][, within the zones of <bounds>][, segments of at least {minsize} cells].",
    ),
    (
        "segment.quickshift",
        "<image> segmented by quick shift[ with a kernel size of {kernel_size}][ and links up to {max_dist}][, colour weighed {ratio}][, within the zones of <bounds>].",
    ),
    (
        "segment.watershed",
        "<image> cut into the watershed basins of its gradient[, from the markers of <markers>][, smoothed by {sigma} cells][, within the zones of <bounds>].",
    ),
    (
        "segment.meanshift",
        "<image> segmented by mean shift[ with a range radius of {range_radius}][ and a spatial radius of {spatial_radius} cells][, within the zones of <bounds>].",
    ),
    (
        "segment.scale",
        "The scales worth trying in a multiresolution segmentation of <image>, by ESP[ from {start}][ in steps of {step}].",
    ),
    (
        "segment.optimize",
        "<image> segmented by {segmenter} at the values of {parameter} tried, the best by the intra-object variance and Moran's I[ ({objective} objective)][, measured against the reference <reference>].",
    ),
    (
        "segment.evaluate",
        "The segmentation <objects> measured against the reference <reference>[ ({metrics})].",
    ),
    (
        "objects.features",
        "The attributes {features} of the objects of <objects>, from <image>[ and the layers of <more>][, with the thematic layer <thematic> (field {field})][, skeleton spurs under {spur} cells cut].",
    ),
    ("objects.to_map", "The objects of <objects> as a map[ of their {what}]."),
    ("objects.samples", "The objects under the samples of <samples> (the class in {class}), as a training table."),
    (
        "objects.classify",
        "The objects of <objects> classed by <model>[, a class only above a probability of {min}][, novelty by {novelty}].",
    ),
    (
        "objects.calc",
        "The attributes of the objects of <objects>[ ({column})] computed from their attributes, their neighbours and their levels by a script.",
    ),
    (
        "objects.merge",
        "The neighbouring objects of <objects> merged[ by {by}][ where {condition}][ into a level above].",
    ),
    (
        "objects.grow",
        "The objects of class {class} in <objects> grown into the neighbours where {condition}[, at most {max_steps} steps].",
    ),
    (
        "objects.remove_small",
        "The small objects of <objects>[ (under {min_cells} cells)][ (under {min_area} of area)] merged into a neighbour[, the {into} one].",
    ),
    ("objects.reclassify", "The classes of <objects> renamed and joined as {classes}[, neighbours of a class merged]."),
    (
        "objects.rules",
        "The objects of <objects> classed by a hierarchy of fuzzy rules[, a class only above a membership of {min}].",
    ),
    (
        "objects.edit",
        "The objects of <objects> corrected by hand: {edits}[, with <image> for the similarity].",
    ),
    (
        "objects.split",
        "The objects of <objects>[ of the classes {classes}] divided into their pieces by the {by} (a peak must stand {h} over the saddle)[, pieces under {min_size} cells given back][, cuts kept shorter than {max_neck} times the width][, with <surface> as the surface].",
    ),
    (
        "objects.resize",
        "The objects of <objects>[ of the classes {classes}] resized by {mode}[ over {distance} map units][ in {steps} steps][, taking the cells of {into}][, limited by <mask>][, holes up to {max_hole} cells][, with <image> for the border optimization].",
    ),
    (
        "objects.fuse",
        "The objects of <objects>[ of the classes {seeds}] joined to a neighbour[ of the classes {candidates}] when the joined object has the best {fitting}[ ({goal} is better)][, only if not worse than the seed's own][, reaching {threshold}][, in at most {max_passes} passes][, with <image> for the bands' means].",
    ),
    (
        "objects.extrema",
        "The local extrema of {attribute} among the objects of <objects>[ of the classes {among}][ ({kind})][, within {radius} map units][, within {steps} steps of neighbours][, given the class {class}].",
    ),
    (
        "objects.similar",
        "The objects of <objects> that look like the examples[ in <samples>][, the {top} most similar][, those above {threshold}][, by {metric}].",
    ),
    (
        "objects.join",
        "The attributes of <objects> joined by id to the columns of <table>[ ({columns})][, replacing those they have].",
    ),
    (
        "objects.nearest",
        "The objects of <objects> classed by their nearest samples in <samples>[, a class only above a membership of {min}].",
    ),
    ("objects.to_vector", "The objects of <objects> as polygons[, their corners cut {smooth} times]."),
    (
        "objects.resolve",
        "The hypotheses of <memberships> arbitrated cell by cell into objects[, a class only above a membership of {min}][, merged {merge}].",
    ),
    (
        "objects.temporal",
        "The classes of <after> adjusted by <matrix>, the transitions possible from <before>[, both dates adjusted {both}][, a class only above a membership of {min}].",
    ),
    (
        "objects.transition_matrix",
        "The matrix of possible transitions between two dates, from <samples> or <transitions>[, a transition never seen at {floor}].",
    ),
    (
        "segment.sam",
        "<image> cut into the masks of the Segment Anything Model[ ({encoder}, {decoder})][, {points} prompt points per side][, one object for each group of the prompts <prompts>] as objects.",
    ),
    (
        "objects.embed",
        "The objects of <objects> given the embeddings of {model} over patches of <image>[ of {patch} cells].",
    ),
    (
        "objects.accuracy",
        "The accuracy of the classes of <objects> against <samples> or <reference>[ (the class in {class})], by objects and by area[, counting the reference objects of the training <training>].",
    ),
    (
        "objects.series",
        "The time series of the objects of <objects> from the layers of <series>[ ({variable})][, the groups {groups}][, smoothed by {smooth}][; the season from {season_threshold} of the amplitude].",
    ),
    (
        "raster.phenology",
        "The seasons of the time series of each cell of <series>[, the groups {groups}][, smoothed by {smooth}][; the season from {season_threshold} of the amplitude].",
    ),
    (
        "objects.folds",
        "The training objects of <samples> (of <objects>) dealt into {k} folds[ by {method}][, objects within {steps} steps together][; seed {seed}].",
    ),
    (
        "objects.sample",
        "A stratified sample of the objects of <objects>[, {per_class} of each class][, {n} in all], to label.",
    ),
    (
        "nlm.image",
        "A synthetic multispectral image of the classes of <classes>, with their truth known[; seed {seed}].",
    ),
    (
        "raster.local_correlation",
        "The correlation between <x> and <y> in a {shape} window[ of radius {radius}] around each cell.",
    ),
    ("raster.majority", "<map> cleaned by the majority filter, {neighbours} neighbours, {iterations} times."),
    ("raster.match", "<raster> on the grid of <like>, by {resampling}."),
    ("raster.match_histogram", "<raster> brought to the histogram of <reference>[ over the cells of <mask>]."),
    ("raster.mosaic", "The rasters of <raster> joined into one mosaic[ in {crs}][, cells of side {resolution}]."),
    ("raster.shrink", "The classes {classes} of <map> shrunk by {cells} cells, {neighbours} neighbours."),
    ("raster.standardize", "<map> put on a common scale by {method}."),
    ("raster.tabulate", "The area of each class of <classes> in each zone of <zones>, in {units}."),
    (
        "raster.viewshed",
        "What can be seen over <map>[ from the observers <points>][ from ({x}, {y})][, up to {radius}].",
    ),
    ("raster.zonal", "The statistics of <map> in each zone of <zones>[: {stats}]."),
    (
        "redd.allocate",
        "VT0007's allocation of the deforestation on the vulnerability classes <classes> by the frequencies <table>[, \
         the expected <expected> ha][, as observed in <deforestation>].",
    ),
    (
        "redd.assess",
        "VT0007's assessment of the predicted deforestation <density> against <deforestation>, in cells of {grid_area} \
         ha.",
    ),
    (
        "redd.fit",
        "VT0007's fit of the vulnerability classes <classes> to the deforestation <deforestation>[ in the subdivisions \
         <subdivisions>].",
    ),
    (
        "redd.nrt",
        "VT0007's negligible risk threshold: the distance <distance> to the forest's edge within which 99.5 % of \
         <deforestation> happened.",
    ),
    (
        "redd.vulnerability",
        "VT0007's vulnerability map in {classes} classes[ from the distance <distance>][ from <vulnerability>].",
    ),
    (
        "restoration.corridor",
        "The gain of restoring the least-cost corridor between <a> and <b> across <resistance>, on the habitat \
         <habitat>.",
    ),
    ("restoration.cost_distance", "The accumulated cost of travelling from <sources> across <resistance>."),
    (
        "restoration.ecosystem_areas",
        "The natural area of each ecoregion of <ecoregions> now and before conversion, from <land_cover>.",
    ),
    (
        "restoration.ecosystem_targets",
        "A restoration target of {target} km² shared among the ecosystems of <table> for the least risk of collapse, \
         by {method}.",
    ),
    (
        "restoration.intactness",
        "The ecological intactness of <footprint> in a radius of {radius} cells[, after restoring <restored>].",
    ),
    (
        "restoration.intactness_gradient",
        "How much the ecological intactness of <footprint> grows per unit of each cell restored, the footprint falling \
         by <reduction>.",
    ),
    (
        "restoration.isochrone",
        "The isochrones of the cost of reaching each cell from the objects of <marker> across <resistance>.",
    ),
    ("restoration.least_cost_path", "The least-cost path between <a> and <b> across <resistance>."),
    (
        "restoration.original_vegetation",
        "The original vegetation of the converted land of <land_cover>[, from the past cover <past>][, from the \
         ecoregions <ecoregions>].",
    ),
    ("restoration.resistance", "A resistance growing with the distance to the habitat of <map>[ ({foreground})]."),
    (
        "restoration.status",
        "How connected the habitat of <map>[ ({foreground})] is and how much reconnecting it could add.",
    ),
    ("rng.numpy", "numpy's random number generator, seed <seed>."),
    (
        "sdm.accessible_area",
        "The area the species of <occurrences> could have reached, by {method}[ within {distance} m][ in the regions \
         <regions>][, cut to <within>].",
    ),
    (
        "sdm.autocorrelation_range",
        "The range of spatial autocorrelation of <env>, {n} cells sampled per layer; seed {seed}.",
    ),
    (
        "sdm.bias_surface",
        "The sampling effort of the records <occurrences> as a kernel density on the grid of <env>[, bandwidth \
         {bandwidth}].",
    ),
    ("sdm.boyce", "The continuous Boyce index of the predictions <presences> against <available>, by {method}."),
    ("sdm.buffer_folds", "Leave-one-out folds of <records>, each record buffered by {size}."),
    ("sdm.checkerboard_folds", "Checkerboard folds of <records> on the cells of <env> aggregated by {aggregation}."),
    (
        "sdm.dataset",
        "The records of a species distribution model: the presences <presences>[ and the background points \
         <background>] with the values of <env>.",
    ),
    (
        "sdm.download_gbif",
        "The occurrence records of {species} from GBIF[ in {country}][, years {year}][ within <area>].",
    ),
    (
        "sdm.evaluate",
        "How well the predictions separate the presences <presences> from the background <background>: AUC, TSS, the \
         Boyce index and the thresholds.",
    ),
    ("sdm.exdet", "The extrapolation (ExDet) of <env> beyond the conditions <reference>."),
    (
        "sdm.importance",
        "The permutation importance of the predictors of <model> on <records>, {n_repeats} shuffles; seed {seed}.",
    ),
    ("sdm.jackknife_folds", "Jackknife folds of <records>: each presence its own fold."),
    (
        "sdm.maxnet",
        "A MaxNet species distribution model[ of <data>][ of the presences <presences>][ and the background \
         <background>][, its features {classes}], regularisation {regmult}, output {type}.",
    ),
    (
        "sdm.migrate",
        "The dispersal of the species from <initial> through the suitability <suitability>, {steps} steps per period, \
         the kernel {kernel}[, the barriers <barriers>]; seed {seed}.",
    ),
    ("sdm.mop", "The mobility-oriented parity of <env> against <reference> ({type}, {distance} distance)."),
    ("sdm.omission_rate", "The omission rate of the test presences <test> at the {threshold} threshold of <train>."),
    ("sdm.one_per_cell", "A record of <occurrences> per cell of <raster>."),
    ("sdm.quadrant_folds", "The four quadrants of the presences of <records>, orientation {orientation}."),
    ("sdm.random_folds", "{k} random folds of the presences of <records>; seed {seed}."),
    ("sdm.reference_data", "The reference data {name} for cleaning records."),
    (
        "sdm.response_curves",
        "The response curves of <model> over the records <records>, the other predictors at their {fixed}.",
    ),
    (
        "sdm.select_predictors",
        "The predictors of <env> without collinearity, a VIF threshold of {vif}[, a correlation threshold of {cor}].",
    ),
    ("sdm.spatial_blocks", "Spatial blocks of <records>[ of size {size}] in {k} folds ({selection}); seed {seed}."),
    ("sdm.spatial_clusters", "{k} spatial clusters of <records>[, environmental on <env>]; seed {seed}."),
    ("sdm.threshold", "A threshold of suitability from <presences> and <background>, by {method}."),
    ("sdm.vif", "The variance inflation factor of each predictor of <env>[ within <area>]."),
    ("sensitivity.analyze", "The sensitivity indices ({method}) of the runs <design> from their outputs <results>."),
    ("sensitivity.design", "The runs of a global sensitivity analysis ({method})[, n = {n}]; seed {seed}."),
    (
        "services.ndr",
        "InVEST's nutrient delivery on <lulc> by the table <biophysical>, over <dem> and the runoff <runoff_proxy> in \
         <watersheds>, streams at {threshold} cells.",
    ),
    (
        "services.sdr",
        "InVEST's sediment delivery on <lulc> by the table <biophysical>, over <dem>, the erosivity <erosivity> and \
         the erodibility <erodibility> in <watersheds>, streams at {threshold} cells.",
    ),
    (
        "services.seasonal_water_yield",
        "InVEST's seasonal water yield over <dem> in <watersheds>[ on <lulc>][ by the table <biophysical>][ from the \
         recharge <recharge>], streams at {threshold} cells.",
    ),
    (
        "services.water_yield",
        "InVEST's annual water yield of <lulc> by the table <biophysical>, the rain <precipitation> and the \
         evapotranspiration <eto>, seasonality {seasonality}[, in <watersheds>].",
    ),
    ("stack.classes", "<stack> with its layers taken as classes or transitions."),
    ("stack.extract_layer", "The layer {layer} of <stack>."),
    ("stack.rename", "<stack> with its layers named {names}[, multiplied by {scale}][ plus {offset}]."),
    ("stack.reduce", "The {stat} of the layers of <maps> at each cell."),
    ("stats.getis_ord", "The Getis-Ord Gi* hot and cold spots of <map>[, {contiguity} neighbours]."),
    (
        "stats.idw",
        "The values of the points <points> interpolated onto <like> by inverse distance weighting, power {power}[, the \
         nearest {nmax}].",
    ),
    (
        "stats.kriging",
        "The values of the points <points> interpolated onto <like> by ordinary kriging[, the variogram <model>][, the \
         nearest {nmax}].",
    ),
    (
        "stats.lisa",
        "The local Moran's I of each cell of <map>, {permutations} permutations, clusters at {alpha}[, {contiguity} \
         neighbours]; seed {seed}.",
    ),
    (
        "stats.moran",
        "The global Moran's I of <map>[, {contiguity} neighbours], {permutations} permutations; seed {seed}.",
    ),
    ("stats.trend", "The Mann-Kendall trend and Sen's slope of each cell along <maps>, significance {alpha}."),
    ("stats.variogram", "The sample variogram of the points <points>[ ({field})] and a {model} model fitted to it."),
    ("surface.aacf", "The areal autocorrelation function of <map>."),
    ("surface.bearing", "The bearing area curve (Abbott-Firestone) of <map>."),
    ("surface.dissection", "Martonne's dissection of <map> in a window of {s} cells."),
    (
        "surface.fit_plane",
        "The trend of <map>, a least-squares polynomial[ of order {order}], and the surface without it.",
    ),
    (
        "surface.geomorphons",
        "The landforms of <dem> by geomorphons, a search radius of {search}, a flatness threshold of {flat}°.",
    ),
    (
        "surface.gravity",
        "The gravitational potential of the sources <sources>[, the distance decay {exponent}][, masked by <mask>].",
    ),
    (
        "surface.hilltop",
        "The hilltop APPs of <dem>: the hills at least {min_height} m high and steeper than {min_slope}°[, their bases \
         at <plain>].",
    ),
    ("surface.hli", "The heat load index from the aspect <aspect> and the slope <slope>[, at latitude {latitude}]."),
    ("surface.hsp", "The hierarchical slope position of <map>, scales {min_scale} to {max_scale}."),
    ("surface.metrics", "The surface metrics of <map>[: {metrics}]."),
    ("surface.peaks", "The peaks or valleys of <map>."),
    ("surface.spherical_sd", "The spherical standard deviation of the normals of <map> in a window of {d} cells."),
    ("surface.srr", "The surface relief ratio of <map> in a window of {s} cells."),
    ("surface.trasp", "The transformed aspect of <map>."),
    ("surface.tri", "The terrain ruggedness index of <map> in a window of {s} cells."),
    ("surface.vrm", "The vector ruggedness measure of <map> in a window of {s} cells."),
    ("surface.window", "The surface metrics[ {metrics}] of <map> in a {shape} window of {window} cells."),
    (
        "swat.channel_map",
        "The results of the channels of <project> on the map[ ({column})][, by {how}][, read from <channel_sd>].",
    ),
    (
        "swat.from_tables",
        "The SWAT+ project of <watershed> and the weather of <stations>[ from {start}][ to {end}], as the SWAT+ Editor writes it.",
    ),
    (
        "swat.hru_map",
        "The results of the HRUs of <project> on the map[ ({variable})][, by {how}][, read from <hru_wb>][ <hru_nb>][ <hru_ls>][ <hru_pw>].",
    ),
    ("swat.load", "Loads the SWAT+ project {path}."),
    ("swat.reach_discharge", "The daily discharge of each channel of the SWAT+ project[ <project>][ in <channel_sd>]."),
    ("swat.run", "Simulates the SWAT+ project[ {path}][ <project>], the tables by {step}."),
    (
        "swat.set",
        "<project> with {parameter} changed ({change} {value})[ in the soil layers from {first_layer}][ to {last_layer}][ in the HRUs {objects}].",
    ),
    (
        "swat.watershed",
        "The watershed of <dem> divided as QSWAT+ does[ with streams from {stream_threshold} km²][ and the outlets <outlets>], its HRUs by {hru_method} of <landuse> and <soils>.",
    ),
    ("swat.wgn", "The weather generator statistics of the stations of <pcp>, <tmax> and <tmin>."),
    ("table.aggregate", "The rows of <table> grouped[ by {by}] and summarised by {stats}[: {columns}]."),
    ("table.calc", "The column {column} of <table> computed as {expression}."),
    ("table.join", "<left> joined with <right> on {on} ({how} join)."),
    ("table.pivot", "<table> pivoted ({direction})[, a column per value of {names}]."),
    ("table.select", "The columns of <table>[: {columns}][, without {drop}]."),
    ("table.sort", "The rows of <table> sorted by {by}[, the first {top} kept]."),
    ("uncertainty.delta", "The uncertainty of {expression} by the delta method ({method})."),
    ("uncertainty.interval", "The confidence interval of <value> from its standard deviation <sd>, level {level}."),
    (
        "uncertainty.product_sd",
        "The standard deviation of the product of <x> and <v>, from their standard deviations <sd_x> and <sd_v>.",
    ),
    (
        "uncertainty.sum_sd",
        "The standard deviation of the total from the standard deviations <sd>[, weighted by <weights>], {method}[, \
         correlation {correlation}].",
    ),
    (
        "validate.error_budget",
        "MOLUSCE's error budget of <simulated> against <reference>, at {iterations} resolutions merging {scale} cells \
         a side.",
    ),
    ("validate.futures", "FUTURES' validation of <simulated> against <reference>[, from <original>]."),
    (
        "validate.futures_grid",
        "FUTURES' validation of <simulated> against <reference>[ in tiles of {size} cells][ per region of <regions>][, \
         from <original>].",
    ),
    (
        "validate.three_map",
        "The three-map comparison of <simulated> with the reference from <t0> to <t1>[, blocks of {factors} cells].",
    ),
    ("value.calc", "A value computed from its inputs[: {expression}]."),
    ("vector.buffer", "The area within {distance} of each feature of <vector>."),
    ("vector.clip", "The features of <vector> cut to <mask>."),
    (
        "vector.density",
        "The density of the points <vector> on the grid of <map>, a {kernel} kernel[ of radius {radius}][ weighted by \
         {field}].",
    ),
    ("vector.dissolve", "The features of <vector> merged[ by {by}]."),
    ("vector.distance", "The distance from each cell of the grid of <like> to the nearest feature of <vector>."),
    ("vector.exact_sum", "The sum of <map> in each polygon of <vector>, weighted by the share of each cell covered."),
    ("vector.from_table", "Points from the columns {x} and {y} of <table>[ in {crs}]."),
    ("vector.geometry", "The area, perimeter, length and centroid of each feature of <vector>."),
    ("vector.grid_zones", "Squares of side {size} tiling <map>."),
    (
        "vector.join",
        "The columns of <table> joined to the features of <vector>[ by their attribute {on}][ and its column {key}].",
    ),
    ("vector.overlay", "The {how} of the polygons <vector> and <other>."),
    (
        "vector.rasterize",
        "The features of <vector> burnt into the grid of <like>[, the attribute {field}][, the value {value}].",
    ),
    ("vector.reproject", "The features of <vector> reprojected[ to {crs}][ to the system of <like>]."),
    ("vector.spatial_join", "The features of <other> joined to those of <vector> where they meet ({mode})."),
    ("woe.weights", "Weights of evidence given in the model."),
];

/// The phrase of an operator, if it has one.
pub fn phrase(op: &str) -> Option<&'static str> {
    PHRASES.iter().find(|(o, _)| *o == op).map(|(_, p)| *p)
}

/// The operators with a phrase.
pub fn operators() -> impl Iterator<Item = &'static str> {
    PHRASES.iter().map(|(o, _)| *o)
}

/// A setting's value in words: given, else the operator's default (none: neither).
fn value(n: &Node, sig: Option<&OpSignature>, name: &str, given_only: bool) -> Option<String> {
    // (an empty text is none given)
    if let Some(v) = n.params.get(name).filter(|v| v.as_str() != Some("")) {
        return Some(crate::document::words(v).replace('\n', "; "));
    }
    if given_only {
        return None;
    }
    let doc = sig?.params.iter().find(|p| p.name == name)?.doc.clone();
    let d = crate::node_view::default_of(&doc);
    let d = d.strip_prefix("default ").unwrap_or(&d).trim().to_string();
    if !d.is_empty() && d != "default" {
        return Some(d);
    }
    // "8 (default) or 4": the word before it
    let before = doc.split_once(" (default)")?.0;
    let word = before.rsplit([' ', ':', ',', '(']).next()?.trim();
    (!word.is_empty()).then(|| word.to_string())
}

/// Where an input comes from, in a word: the node (not its port), the model's input, the value.
fn source(n: &Node, port: &str) -> Option<String> {
    Some(match From::of(n.inputs.get(port)?) {
        From::Node { node, .. } | From::Within { node, .. } => node,
        From::Input(name) | From::Carried(name) => name,
        From::Value(v) => v,
    })
}

/// Fill a template's placeholders (`given_only`: inside brackets, nothing taken from the defaults); none when one
/// has no value.
fn fill(t: &str, n: &Node, sig: Option<&OpSignature>, given_only: bool) -> Option<String> {
    let mut out = String::new();
    let mut rest = t;
    while let Some(at) = rest.find(['{', '<']) {
        out.push_str(&rest[..at]);
        let close = if rest[at..].starts_with('{') { '}' } else { '>' };
        let end = at + rest[at..].find(close)?;
        let name = &rest[at + 1..end];
        let v = if close == '}' { value(n, sig, name, given_only)? } else { source(n, name)? };
        out.push_str(&v);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// The sentence of a step: its operator's phrase with its settings and inputs (none: no phrase, or a setting it
/// needs has no value).
pub fn sentence(n: &Node, sig: Option<&OpSignature>) -> Option<String> {
    let t = phrase(&n.op)?;
    let mut out = String::new();
    let mut rest = t;
    while let Some(at) = rest.find('[') {
        out.push_str(&fill(&rest[..at], n, sig, false)?);
        let end = at + rest[at..].find(']')?;
        out.push_str(&fill(&rest[at + 1..end], n, sig, true).unwrap_or_default());
        rest = &rest[end + 1..];
    }
    out.push_str(&fill(rest, n, sig, false)?);
    // (a sentence starts with a capital, an input's name first too)
    let mut c = out.chars();
    Some(c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use terraflux_ops::Registry;

    /// The settings a step's sentence says (left out of the list under it, unless they differ from the default).
    fn said(op: &str) -> std::collections::BTreeMap<&'static str, ()> {
        let mut out = std::collections::BTreeMap::new();
        if let Some(t) = phrase(op) {
            let mut rest = t;
            while let Some(at) = rest.find('{') {
                let Some(end) = rest[at..].find('}') else { break };
                out.insert(&rest[at + 1..at + end], ());
                rest = &rest[at + end + 1..];
            }
        }
        out
    }

    #[test]
    fn the_templates_steps_read_as_sentences() {
        let r = Registry::standard();
        fn walk(nodes: &[Node], r: &Registry, out: &mut Vec<String>) {
            for n in nodes {
                let sig = r.sig(&n.op).map(|s| s.as_ref().clone());
                if let Some(t) = sentence(n, sig.as_ref()) {
                    out.push(format!("{}: {t}", n.op));
                }
                if let Some(b) = &n.body {
                    walk(&b.nodes, r, out);
                }
            }
        }
        let mut said = Vec::new();
        for t in crate::templates::TEMPLATES {
            walk(&t.model().nodes, &r, &mut said);
        }
        if std::env::var("TERRAFLUX_PRINT").is_ok() {
            said.sort();
            said.dedup();
            for s in &said {
                eprintln!("{s}");
            }
        }
        for s in &said {
            assert!(!s.contains(['{', '}']) || s.contains("rules") || s.contains("expression"), "{s}");
            assert!(!s.contains("[ ") && !s.contains(" ]"), "{s}");
        }
        assert!(said.len() > 200, "{}", said.len());
    }

    #[test]
    fn a_step_reads_as_a_sentence() {
        let r = Registry::standard();
        let sig = |op: &str| r.sig(op).map(|s| s.as_ref().clone());
        let n = Node::new("aptitude", "nlm.gaussian_field").param("rows", 100).param("columns", 120).param("seed", 7);
        assert_eq!(
            sentence(&n, sig("nlm.gaussian_field").as_ref()).as_deref(),
            Some(
                "A Gaussian random field of 100 × 120 cells: autocorrelation range 10 cells, variance 5, nugget 0.2; seed 7."
            )
        );
        let n = Node::new("habitat", "nlm.binarize").input("map", "aptitude.map").param("breaks", 0.3);
        assert_eq!(
            sentence(&n, sig("nlm.binarize").as_ref()).as_deref(),
            Some("Aptitude split into habitat and matrix, a share 0.3 of it habitat.")
        );
        let n = Node::new("chain", "markov.fit").input("t0", "$landuse_2000").input("t1", "$landuse_2010");
        assert_eq!(
            sentence(&n, sig("markov.fit").as_ref()).as_deref(),
            Some("The Markov chain of the changes from landuse_2000 to landuse_2010.")
        );
        // objects: the optional settings only when given
        let n = Node::new("blocks", "segment.chessboard").input("image", "scene.image").param("size", 8);
        assert_eq!(
            sentence(&n, sig("segment.chessboard").as_ref()).as_deref(),
            Some("Scene cut into squares of 8 cells as objects.")
        );
        let n = Node::new("regions", "segment.from_map").input("map", "sam.map");
        assert_eq!(
            sentence(&n, sig("segment.from_map").as_ref()).as_deref(),
            Some("The regions of sam as objects, each group of connected cells of one value.")
        );
        // an input it needs not given: no sentence
        assert_eq!(sentence(&Node::new("k", "validate.kappa"), sig("validate.kappa").as_ref()), None);
        // every phrase names settings and inputs its operator has
        for op in operators() {
            let s = sig(op).unwrap_or_else(|| panic!("{op}: no such operator"));
            for name in said(op).keys() {
                assert!(s.params.iter().any(|p| p.name == *name), "{op}: no setting {name}");
            }
            let mut rest = phrase(op).unwrap();
            while let Some(at) = rest.find('<') {
                let end = at + rest[at..].find('>').unwrap();
                let port = &rest[at + 1..end];
                assert!(s.inputs.iter().any(|p| p.name == port), "{op}: no input {port}");
                rest = &rest[end + 1..];
            }
        }
    }
}
