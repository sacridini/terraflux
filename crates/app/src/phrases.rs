//! What a step does with its settings, in a sentence written by hand for every operator ("A Gaussian random
//! field of 100 × 100 cells: autocorrelation range 10 cells…"), shown in the document and the
//! report with the settings listed.
//!
//! A phrase is a template: `{name}` is a setting's value in words (the one given, else the operator's default),
//! `<port>` where an input comes from (a node's name, a model input's), and `[…]` a part said only when every
//! setting and input it names is given.

use terraflux_graph::{Node, OpSignature};

use crate::document::From;
use crate::methods::Lang;

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
         <predictors>][, learning <target>][; seed {seed}].",
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
        "ml.potential",
        "The potential of each class of <landscape>[ ({target})] from the drivers <drivers>[ and the later map \
         <later>][, a {method} model][; seed {seed}].",
    ),
    ("ml.predict", "The prediction of the model <model>[ on the maps <predictors>][ on the rows of <table>]."),
    (
        "molusce.step",
        "An iteration of MOLUSCE's simulator on <landscape>[, by the weights <total>][, by the potentials \
         <potential>].",
    ),
    ("molusce.total", "MOLUSCE's total weight of each transition from the drivers <drivers>."),
    ("move.aeqd_crs", "<tracks> in an azimuthal equidistant projection centred on their {center}, in {units}."),
    ("move.as_tracks", "Tracks from the locations[ of <vector>][ of <table>][ (columns {x} and {y})][ in {crs}]."),
    ("move.azimuth", "The direction of each step of <tracks>, clockwise from north, in {units}."),
    ("move.distance", "The distance from each location of <tracks> to the next[, in {units}]."),
    ("move.distribution", "A {name} distribution of step lengths or turn angles."),
    ("move.extract_covariates", "The values of the rasters connected at the locations of <tracks> (steps: {where})."),
    ("move.filter_min_n_burst", "The bursts of <tracks> of at least {min_n} locations."),
    ("move.filter_movebank_visible", "The tracks <tracks> without the locations Movebank hides."),
    ("move.filter_per_interval", "One location of <tracks> per {unit} (criterion {criterion})."),
    ("move.fit_distribution", "A {name} distribution fitted to <values>."),
    (
        "move.interpolate",
        "Locations of <tracks> interpolated[ at the times {times}][, gaps of more than {max_time_lag} s left empty].",
    ),
    ("move.nsd", "The net squared displacement of each location of <tracks> from the first of its track."),
    ("move.random_numbers", "{n} draws of the distribution <distribution>; seed {seed}."),
    (
        "move.random_steps",
        "{n_control} random steps for each observed step of <steps>[, lengths from <sl_distr>][, turn angles from \
         <ta_distr>]; seed {seed}.",
    ),
    ("move.resample", "The locations of <tracks> resampled every {rate}, tolerance {tolerance}, in bursts."),
    ("move.segments", "The steps of <tracks> as lines."),
    ("move.simulate_brownian", "{tracks} tracks of Brownian motion, diffusion {sigma}; seed {seed}."),
    ("move.speed", "The speed of each step of <tracks>[, in {units}]."),
    ("move.steps", "The steps between consecutive locations of <tracks>: their length, direction and turn angle."),
    ("move.time_lags", "The time from each location of <tracks> to the next, in {units}."),
    ("move.time_of_day", "Day or night at each location of <tracks>."),
    ("move.track_data", "A row per track of <tracks> with the columns that do not change within it."),
    ("move.track_lines", "Each track of <tracks> as a line through its locations."),
    ("move.track_summary", "The distance, straightness and sinuosity of each track of <tracks>."),
    ("move.turn_angle", "The change of direction at each location of <tracks>, in {units}."),
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
        "raster.local_correlation",
        "The correlation between <x> and <y> in a {shape} window[ of radius {radius}] around each cell.",
    ),
    ("raster.majority", "<map> cleaned by the majority filter, {neighbours} neighbours, {iterations} times."),
    ("raster.match", "<raster> on the grid of <like>, by {resampling}."),
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
    ("swat.from_tables", "The SWAT+ project of <watershed> and the weather of <stations>[ from {start}][ to {end}], as the SWAT+ Editor writes it."),
    ("swat.load", "Loads the SWAT+ project {path}."),
    ("swat.run", "Simulates the SWAT+ project[ {path}][ <project>], the tables by {step}."),
    ("swat.set", "<project> with {parameter} changed ({change} {value})[ in the soil layers from {first_layer}][ to {last_layer}][ in the HRUs {objects}]."),
    ("swat.watershed", "The watershed of <dem> divided as QSWAT+ does[ with streams from {stream_threshold} km²][ and the outlets <outlets>], its HRUs by {hru_method} of <landuse> and <soils>."),
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

/// The phrases in Portuguese, the same placeholders as the English.
const PHRASES_PT: &[(&str, &str)] = &[
    ("raster.read", "Lê os rasters de {path}[, banda {band}][, com {nodata} como sem dado]."),
    ("raster.write", "Grava <value> em {path}[ como {type}]."),
    ("vector.read", "Lê as feições de {path}[, camada {layer}]."),
    ("vector.write", "Grava as feições de <vector> em {path}[, camada {layer}]."),
    ("table.read", "Lê a tabela {path}."),
    ("move.read_tracks", "Lê as trilhas dos animais de {path}."),
    ("view.plot", "Mostra[ <value>][ <features>][ <table>][ sobre <under>][ como \"{title}\"]."),
    ("nlm.random", "Um mapa aleatório de {rows} × {columns} células[ de lado {resolution}], semente {seed}."),
    (
        "nlm.gaussian_field",
        "Um campo aleatório gaussiano de {rows} × {columns} células[ de lado {resolution}]: alcance da \
         autocorrelação de {range} células, variância {variance}, efeito pepita {nugget}; semente {seed}.",
    ),
    (
        "nlm.random_cluster",
        "Agrupamentos aleatórios em {rows} × {columns} células[ de lado {resolution}], uma fração {p} das células \
         em agrupamentos[ ({neighbourhood} vizinhos)][, as classes com pesos {weights}]; semente {seed}.",
    ),
    (
        "nlm.fbm",
        "Movimento browniano fracionário em {rows} × {columns} células[ de lado {resolution}], dimensão fractal \
         {dimension}; semente {seed}.",
    ),
    (
        "nlm.planar_gradient",
        "Um gradiente planar em {rows} × {columns} células[ de lado {resolution}][, voltado para {direction}°]; \
         semente {seed}.",
    ),
    (
        "nlm.distance_gradient",
        "Um gradiente da distância a um retângulo[ ({origin})] em {rows} × {columns} células; semente {seed}.",
    ),
    ("nlm.binarize", "<map> dividido em habitat e matriz, uma fração {breaks} dele habitat."),
    ("raster.reclassify", "<map> com os valores reclassificados: {rules}."),
    ("raster.reproject", "<raster> reprojetado[ para {crs}][, células de lado {resolution}][, por {resampling}]."),
    ("raster.resample", "<raster> reamostrado para células de lado {resolution}[, por {resampling}]."),
    ("markov.fit", "A cadeia de Markov das mudanças de <t0> para <t1>[, em <steps> passos]."),
    ("glm.fit", "Uma regressão logística de cada classe de <landscape> em <drivers>[ (fórmulas {formulas})]."),
    ("glm.predict", "A probabilidade de cada classe pela regressão <model>, em <drivers>[ e <landscape>]."),
    (
        "woe.fit",
        "Os pesos de evidência de cada mudança de <t0> para <t1> por <drivers>[, faixas em incrementos de \
         {increment}][, {categorical} tomadas como categorias].",
    ),
    ("woe.probability", "A probabilidade de cada mudança pelos pesos <model>, em <landscape> e <drivers>."),
    ("demand.from_maps", "A área de cada classe ao longo do tempo, de <maps>[ datados {times}][, para {out_times}]."),
    ("dinamica.step", "Um passo da alocação das mudanças <rates> do Dinamica em <landscape>, pela <probability>."),
    ("lulcc.step", "Um passo da alocação de <demand> em <landscape>, pela <suitability>."),
    (
        "patches.observe",
        "As manchas das mudanças de <t0> para <t1>: quanto elas se expandem e a sua isometria[ ({isometry})].",
    ),
    ("metrics.class", "As métricas de cada classe de <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    ("metrics.landscape", "As métricas da paisagem inteira <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    ("metrics.patch", "As métricas de cada mancha de <map>[: {metrics}][, manchas de {directions} vizinhos]."),
    (
        "landscape.mspa",
        "Os padrões do MSPA de <map>[, o primeiro plano {foreground}]: conectados por {connectivity} vizinhos, as \
         bordas de {edge_width} células de largura.",
    ),
    ("validate.agreement", "Quanto <simulated> concorda com <observed>, os dois a partir de <start>."),
    ("validate.kappa", "O kappa de <simulated> contra <reference>."),
    (
        "sdm.fit",
        "Um modelo de distribuição de espécies ajustado a <records>[ em {predictors}][, as feições {classes}]\
         [, regularização {regmult}], saída {type}.",
    ),
    (
        "sdm.area_of_habitat",
        "A área de habitat de cada espécie dentro de <ranges> em <land_cover>[, as classes naturais {natural}].",
    ),
    (
        "services.carbon",
        "O carbono estocado em <lulc> pelos estoques <pools>[, e a sua mudança para <future>][ a {price} a tonelada].",
    ),
    (
        "prioritize.optimize",
        "As unidades de planejamento escolhidas[ para <features>][ ao custo <cost>], o objetivo {objective}[ num \
         orçamento de {budget}].",
    ),
    ("raster.calc", "Mapas calculados célula a célula a partir das entradas por um script."),
    ("stack.of", "As entradas como as camadas de uma pilha."),
    ("table.of", "Uma tabela escrita no modelo, as colunas {columns}."),
    ("table.write", "Grava <table> em {path}."),
    ("table.filter", "As linhas de <table> onde {expression}."),
    ("check.assert", "Uma verificação: {expression}[ ({message})]."),
    (
        "check.range",
        "Uma verificação de que cada valor de <value> está na faixa[, no mínimo {min}][, no máximo {max}].",
    ),
    (
        "check.no_nodata",
        "Uma verificação de que <map> tem valor em cada célula[ de <within>][, até uma fração {max_share}].",
    ),
    (
        "check.area_kept",
        "Uma verificação de que a área[ da classe {class}] se mantém de <before> para <after>[, dentro de <within>]\r
         [, até {tolerance} células].",
    ),
    ("check.accuracy", "Uma verificação de que <map> concorda com <reference>[ pela {measure}], no mínimo {min}."),
    ("check.demand", "Uma verificação de que <landscape> atende <demand> no passo <step>[, até {tolerance}]."),
    ("conn.to_map", "Os valores de <table> pintados nas manchas de <graph>[, a coluna {column}]."),
    (
        "conn.graph_from_raster",
        "Um grafo de habitat das manchas[ de <map>][ de <vector>][, as classes de habitat {habitat}], as \
         distâncias por {distance}[ sobre <resistance>][ até {max_distance}].",
    ),
    ("conn.nodes", "Os nós das manchas[ de <map>][ de <vector>][, as classes de habitat {habitat}]."),
    (
        "conn.node_importance",
        "A importância de cada nó em <graph>[ à distância {distance}][, probabilidade {probability}].",
    ),
    ("conn.overall", "A conectividade de <graph>[ à distância {distance}][, probabilidade {probability}]."),
    (
        "dynamic.distance_to",
        "A distância de cada célula de <landscape> à célula mais próxima[ das classes {classes}][, pelos custos \
         <cost>].",
    ),
    ("sdm.maxnet_predict", "A adequabilidade pelo modelo <model> em <env>[, saída {type}]."),
    ("stack.set", "<stack> com <map> como a camada {name}."),
    (
        "hydro.flow_accumulation",
        "O fluxo acumulado de <dem> pelo método {method}[, ponderado por <weights>], em {units}.",
    ),
    ("hydro.fill", "<dem> com as depressões preenchidas."),
    (
        "hydro.streams",
        "Os rios de <accumulation>: as células de acumulação de {threshold} ou mais[, traçados em <dem>].",
    ),
    ("rng.r", "O gerador de números aleatórios do R, semente <seed>."),
    ("vector.filter", "As feições de <vector> onde {expression}."),
    ("vector.calc", "Os atributos de <vector>[ ({column})] calculados a partir dos outros por um script."),
    ("vector.extract", "Os valores de <map> nos pontos de <vector>, pelo método {method}[, num raio de {buffer}]."),
    ("vector.zonal", "As estatísticas de <map> em cada polígono de <vector>[: {stats}]."),
    (
        "vector.points",
        "Pontos ({method}) dentro dos polígonos <vector>[, {n} em cada][, a {spacing} de distância]; semente {seed}.",
    ),
    ("vector.polygonize", "As manchas de <map> como polígonos[, {neighbours} vizinhos]."),
    ("agents.update", "Os agentes <agents> atualizados pelas regras: {rules}."),
    ("agents.parcels", "Um agente por parcela de <parcels>, com a média dos outros mapas em cada uma."),
    ("agents.map", "O atributo {column} de <agents> pintado nas suas parcelas <parcels>."),
    ("agents.summary", "O número de agentes em <agents> e a soma e a média de cada atributo."),
    ("lulcc.history", "Há quantos passos cada célula de <landscape> está na sua classe[, de <history>]."),
    (
        "fragmentation.fos",
        "A fragmentação de <map>[, o primeiro plano {foreground}] numa escala fixa: {method} numa janela de \
         {window} células.",
    ),
    (
        "sdm.prepare",
        "Os registros <occurrences> preparados para um modelo de distribuição de espécies em <env>[, a espécie \
         {species}][, rarefeitos a {thin} m]; semente {seed}.",
    ),
    ("sdm.background", "{n} pontos de fundo em <env>[ dentro de <area>][, ponderados por <bias>]; semente {seed}."),
    ("sdm.clean_occurrences", "Os registros <occurrences> limpos de coordenadas suspeitas (ação: {action})."),
    (
        "sdm.thin",
        "Os registros <occurrences> rarefeitos a {distance} m uns dos outros, {reps} repetições; semente {seed}.",
    ),
    ("sdm.mess", "A similaridade ambiental (MESS) de <env> com <reference>."),
    ("surface.terrain", "A medida {v} de <map>[ em {unit}][, {neighbors} vizinhos]."),
    ("raster.terrain", "A medida {measure} de <raster>[, as opções do gdaldem {options}]."),
    ("surface.curvature", "A curvatura {type} de <map>."),
    ("surface.tpi", "O índice de posição topográfica de <map>[, uma janela de {scale}][, {win}]."),
    (
        "nlm.mosaic_tess",
        "Um mosaico de Voronoi de valores aleatórios em {rows} × {columns} células[ de lado {resolution}][, \
         {germs} germes]; semente {seed}.",
    ),
    (
        "nlm.percolation",
        "Um mapa binário aleatório de {rows} × {columns} células[ de lado {resolution}], cada célula 1 com \
         probabilidade {prob}; semente {seed}.",
    ),
    ("nlm.classify", "<map> em classes[: {n} por quebras naturais][, as frações {weights}][ como em <real>]."),
    ("change.transitions", "As transições entre os mapas de <maps>[ datados {times}]."),
    ("change.crosstab", "A matriz de transição de <from> para <to>."),
    (
        "change.components",
        "Os componentes da diferença[ de <from>][ para <to>][ ao longo de <maps>], a análise de {analysis}.",
    ),
    ("change.simple", "A mudança do primeiro plano[ {foreground}] de <before> para <after>."),
    ("change.morphological", "A mudança morfológica do primeiro plano[ {foreground}] de <before> para <after>."),
    (
        "change.fad",
        "A mudança da fragmentação (FAD) de <before> para <after>[, o primeiro plano {foreground}], em janelas de \
         {windows} células.",
    ),
    (
        "restoration.plan",
        "Um plano de restauração pelo método {method}[ num orçamento de {budget}][, a meta {target}].",
    ),
    ("metrics.signatures", "As assinaturas espaciais de <maps>[ nas zonas <plots>]."),
    ("metrics.window", "As métricas da paisagem[ {metrics}] de <map> numa janela móvel."),
    ("networks.gtn_components", "Os componentes da rede de habitat do mapa do MSPA <map>."),
    ("circuit.pairwise", "A resistência efetiva do Circuitscape entre cada par dos pontos <points> em <habitat>."),
    (
        "circuit.omniscape",
        "A conectividade do Omniscape em <resistance>[ das fontes <source>], em janelas de raio {radius} células[, \
         um alvo a cada {block_size} células].",
    ),
    (
        "cost.passage",
        "As passagens de um passeio aleatório de <origins> para <goals> por <transition>[, theta {theta}].",
    ),
    ("prioritize.evaluate", "A avaliação da solução[ <solution>][ <selection>][ para <features>][ ao custo <cost>]."),
    ("prioritize.zonation", "A ordem de prioridade do Zonation de <features> pela regra {rule}[ ao custo <cost>]."),
    (
        "prioritize.marxan",
        "As reservas do Marxan[ para <features>][ ao custo <cost>][, modificador do comprimento de fronteira \
         {blm}], {num_reps} rodadas.",
    ),
    (
        "services.pollination",
        "A polinização das culturas por abelhas silvestres em <lulc>, pela tabela <biophysical>[ e as fazendas \
         <farms>].",
    ),
    (
        "services.habitat_quality",
        "A qualidade do habitat de <lulc> sob as ameaças <threat_maps>, pelas sensibilidades <sensitivity>[, e no \
         futuro <future>].",
    ),
    (
        "plus.demand",
        "As células de cada classe a cada passo a partir de <start>[ pela cadeia <markov>][ pela demanda \
         <demand>][ por região <regional>][, {steps} passos].",
    ),
    (
        "landscape.accounting",
        "Os objetos do primeiro plano de <map>[ ({foreground})] em classes de tamanho, conectados por \
         {connectivity} vizinhos.",
    ),
    (
        "raster.sieve",
        "<map> com as manchas de menos de {threshold} células juntadas à maior vizinha[, {neighbours} vizinhos].",
    ),
    (
        "validate.similarity",
        "A similaridade fuzzy da mudança em <simulated> e <observed>, os dois a partir de <start>, em janelas de \
         {windows} células.",
    ),
    (
        "accuracy.sample",
        "Uma amostra aleatória estratificada das células de <map>[, {n} por estrato][ por <sizes>]; semente {seed}.",
    ),
    ("data.download", "Dados baixados[ de {source}][ ({variable})][ na grade de <like>][, recortados por <area>]."),
    ("raster.legend", "Os nomes e as cores das classes de <map>."),
    ("markov.matrix", "Uma cadeia de Markov dada no modelo[, as classes {classes}], em {steps} passos."),
    ("raster.stats", "Um resumo de <map>[: {stats}]."),
    ("raster.clip", "<raster> recortado na extensão {extent}."),
    ("distance.euclidean", "A distância euclidiana à borda do primeiro plano de <map>[ ({foreground})]."),
    ("move.filter_unique", "As trilhas <tracks> sem tempos duplicados (critério {criterion})."),
    (
        "uncertainty.monte_carlo",
        "{n} sorteios de {expression}, os parâmetros sorteados de distribuições normais; semente {seed}.",
    ),
    (
        "accuracy.assess",
        "A exatidão e a área das classes de <map>, estimadas de uma amostra estratificada[ <points>][ <samples>][ nos \
         estratos <strata>]; confiança {confidence}.",
    ),
    (
        "accuracy.sample_size",
        "O tamanho de uma amostra estratificada para avaliar <map>, para um erro padrão de {standard_error}[ ({n} \
         pontos)], alocação {allocation}.",
    ),
    ("agents.table", "Agentes de uma tabela escrita no modelo[, ou de <table>]."),
    (
        "batch.run",
        "Roda {operator} em cada raster de {inputs}, em paralelo[, {workers} de cada vez], e grava os resultados em \
         {output_dir}.",
    ),
    (
        "bio.cluster",
        "A análise de agrupamento do Biodiverse de <basedata>[ por {index}][, ligação {linkage}][, cortada em {k} \
         grupos]; semente {seed}.",
    ),
    ("bio.h3", "Uma basedata de <records> em células H3 de resolução {resolution}, os rótulos de {label}."),
    ("bio.hex", "Uma basedata de <records> em hexágonos {orientation} de tamanho {size}, os rótulos de {label}."),
    ("bio.matrix", "Lê uma matriz de rótulos do Biodiverse de {path}."),
    (
        "bio.occurrences",
        "Uma basedata dos registros de ocorrência[ de {path}][ <occurrences>] em células de {cell_sizes}[, \
         reprojetados para {to_crs}].",
    ),
    ("bio.points", "Uma basedata dos pontos <points> em células de {cell_sizes}, os rótulos de {label}."),
    ("bio.properties", "As propriedades ({of}) de <basedata>, lidas de {path}."),
    (
        "bio.randomise",
        "{iterations} aleatorizações ({function}) da análise do Biodiverse de <basedata>: {calculations}; semente \
         {rand_seed}.",
    ),
    (
        "bio.raster",
        "Uma basedata dos rasters <raster>[, os valores de {threshold} ou mais contando 1][ em células de \
         {cell_sizes}].",
    ),
    (
        "bio.region_grower",
        "O crescimento de regiões do Biodiverse em <basedata>[, maximizando {index}][, cortado em {k} regiões]; \
         semente {seed}.",
    ),
    ("bio.spatial", "A análise espacial do Biodiverse de <basedata> na vizinhança de cada grupo: {calculations}."),
    ("bio.table", "Uma basedata da tabela {path}, os grupos de {group_columns} e os rótulos de {label_columns}."),
    ("bio.tree", "Lê a árvore filogenética de {path}."),
    (
        "bio.vector",
        "Uma basedata das feições <features> em células de {cell_sizes}, os rótulos de {label}[, medidas por \
         {measure}].",
    ),
    (
        "carbon.emissions",
        "O carbono emitido e removido pela mudança do uso do solo ao longo de <maps>, pelo estoque de cada classe[ \
         (datas {times})].",
    ),
    (
        "cars.step",
        "Um passo da alocação CARS do PLUS em <landscape>, pela <probability>[ rumo a <demand>][ por região \
         <regional>]; semente <seed>.",
    ),
    (
        "change.composite",
        "A tabulação cruzada de <comp> contra <ref> pelo método composto, em blocos de {factor} células de lado.",
    ),
    ("change.count", "Quantas vezes cada célula mudou ao longo de <maps>[ (datas {times})]."),
    ("change.fos", "A mudança da fragmentação do mapa FOS <before> para <after>."),
    (
        "change.intensity",
        "A análise de intensidade das mudanças ao longo de <maps>[ (datas {times})], os ganhos da classe {gain} e as \
         perdas da classe {loss}.",
    ),
    ("change.mad", "O desvio absoluto médio entre <grid1> e <grid2> e os seus componentes[, pelos estratos <strata>]."),
    ("change.memberships", "A fração de cada classe de <map> em blocos de {factor} células de lado."),
    ("change.mosaic", "A mudança do mosaico da paisagem de <before> para <after>, em janelas de {window} células."),
    (
        "change.resolutions",
        "As diferenças de quantidade, troca e deslocamento entre <comp> e <ref> em resoluções mais grossas, os blocos \
         crescendo por {factor}[, escalonadas para <population>].",
    ),
    (
        "change.sample_to_population",
        "A tabulação cruzada <matrix> escalonada para a população <population> de cada classe.",
    ),
    ("change.suggest", "As transições que vale modelar ao longo de <maps>: as de pelo menos {least} % da mudança."),
    (
        "circuit.advanced",
        "As correntes e as tensões do Circuitscape em <conductance>, a corrente injetada em <source> e drenada em \
         <ground>.",
    ),
    ("circuit.circuitscape_ini", "Roda o arquivo INI do Circuitscape {path}."),
    (
        "circuit.network_advanced",
        "O modo avançado do Circuitscape numa rede[ <graph>][ {graph_file}], a corrente injetada nas fontes e drenada \
         nos aterramentos.",
    ),
    (
        "circuit.network_pairwise",
        "A resistência efetiva do Circuitscape entre cada par dos nós focais {focal_nodes} de uma rede[ <graph>][ \
         {graph_file}].",
    ),
    ("circuit.omniscape_ini", "Roda o arquivo INI do Omniscape {path}."),
    (
        "circuit.one_to_all",
        "O modo um-para-todos do Circuitscape em <habitat>, de cada um dos pontos <points> por vez[, as regiões \
         <polygons>][, mascarado por <mask>].",
    ),
    (
        "clumondo.ages",
        "Os anos que cada célula de <landscape> tem o seu sistema de uso[, de <history>][, aleatórios até {max_age}][; \
         semente <seed>].",
    ),
    (
        "clumondo.step",
        "Um ano do CLUMondo em <landscape>: a adequabilidade de cada sistema de uso[ pelas regressões <model>][ dada \
         por <suitability>], depois a alocação que atende a demanda.",
    ),
    (
        "conn.centrality",
        "As centralidades de cada nó de <graph>[ à distância {distance}][, probabilidade {probability}].",
    ),
    ("conn.components", "O componente de cada nó de <graph>[ à distância {distance}]."),
    (
        "conn.connectivity_grid",
        "A conectividade das áreas protegidas <protected> em cada célula de uma grade sobre <region>[ (<grid>)], à \
         distância {distance}.",
    ),
    (
        "conn.distances",
        "As distâncias entre as manchas[ de <map>][ de <vector>][, as classes de habitat {habitat}], por {distance}[ \
         sobre <resistance>][ até {max_distance}].",
    ),
    (
        "conn.eca_change",
        "A mudança da área conectada equivalente ({metric}) ao longo dos grafos <graphs>[ à distância {distance}], \
         numa paisagem de {landscape_area}.",
    ),
    (
        "conn.eca_change_grid",
        "A mudança da área conectada equivalente ({metric}) de <times> em cada célula de uma grade sobre <region>[ \
         (<grid>)], à distância {distance}.",
    ),
    (
        "conn.focal_nodes",
        "A importância de cada nó de <vector> na sua vizinhança de {search_buffer}, por {metric} à distância \
         {distance}.",
    ),
    ("conn.graph", "Um grafo de habitat dos nós <nodes> e dos pares <pairs>[ ({kind})]."),
    (
        "conn.graph_from_samc",
        "Um grafo de habitat das probabilidades de estabelecimento entre as manchas <vector> sobre <resistance>[, a \
         absorção <absorption>].",
    ),
    (
        "conn.graph_from_vector",
        "Um grafo de habitat das feições[ <vector>], as distâncias por {distance}[ sobre <resistance>][ até \
         {max_distance}].",
    ),
    (
        "conn.link_change",
        "A mudança do {metric} em <graph> quando os seus pares mudam como em <changes>[, à distância {distance}].",
    ),
    (
        "conn.link_importance",
        "A importância de cada ligação de <graph>: a fração do {metric} perdida sem ela[, à distância {distance}].",
    ),
    ("conn.make_grid", "Uma grade de células[ de área {cell_area}] sobre <region>."),
    (
        "conn.metapopulation",
        "Uma metapopulação nas manchas <patches> ao longo de {steps} passos, pelo modelo de função de incidência de \
         Hanski (α {alpha}, x {x}, y {y}, e {e}); semente {seed}.",
    ),
    (
        "conn.protconn",
        "O ProtConn: a fração de <region> protegida e conectada por <protected>, a uma distância de dispersão de \
         {distance}[ sobre <resistance>].",
    ),
    (
        "conn.protconn_raster",
        "O ProtConn das áreas protegidas <protected>[ em <region>][ em <region_map>], a uma distância de dispersão de \
         {distance}.",
    ),
    (
        "conn.protconn_regions",
        "O ProtConn das áreas protegidas <protected> em cada região de <regions>, a uma distância de dispersão de \
         {distance}[ sobre <resistance>].",
    ),
    ("conn.read_conefor", "Lê um grafo de habitat dos arquivos do Conefor {nodes} e {connections}."),
    (
        "conn.settlement_probabilities",
        "As probabilidades de estabelecimento entre os nós <vector> por um passeio aleatório sobre <resistance>[, a \
         absorção <absorption>].",
    ),
    ("conn.to_vector", "A tabela <table> unida às feições de <graph>."),
    ("conn.write_conefor", "Grava <graph> nos arquivos do Conefor {nodes} e {connections}."),
    ("control.each", "Roda os seus passos uma vez para cada elemento de <over>."),
    ("control.group", "Roda os seus passos uma vez, como um grupo."),
    ("control.repeat", "Repete os seus passos <times> vezes."),
    ("control.submodel", "O modelo {path} rodado como um passo."),
    ("control.switch", "Roda o ramo escolhido por <case>."),
    (
        "cost.acc_cost",
        "O custo acumulado a partir da origem mais próxima de <origins>, sobre a transição <transition>.",
    ),
    ("cost.areas", "As transições de área das categorias de <map>, {directions} vizinhos."),
    ("cost.barriers", "As transições de barreira de <map>[ nas quebras {breaks}], {directions} vizinhos."),
    ("cost.combine", "As transições <a> e <b> combinadas por {operation}."),
    (
        "cost.commute_distance",
        "As distâncias de ida e volta de um passeio aleatório entre os pontos <points> sobre <transition>.",
    ),
    ("cost.distance", "Os menores custos sobre <transition> de cada ponto de <from>[ para cada um de <to>]."),
    ("cost.geo_correction", "<transition> corrigida pelas distâncias entre as células, tipo {type}."),
    ("cost.mahal", "A transição de Mahalanobis dos mapas <maps>, {directions} vizinhos."),
    ("cost.math", "A função {function} dos valores de <transition>[, com o valor {value}]."),
    ("cost.normalize", "<transition> normalizada por {method}."),
    (
        "cost.path_inc",
        "A sobreposição e a não sobreposição das rotas sobre <transition> de <origin> até cada local de <from>[ e \
         <to>][, ponderadas por <weights>][, theta {theta}].",
    ),
    (
        "cost.rsp_distance",
        "As distâncias de caminhos mais curtos aleatorizados sobre <transition> de cada ponto de <from>[ para cada um \
         de <to>], theta {theta}.",
    ),
    ("cost.shortest_path", "Os caminhos de menor custo sobre <transition> de <origin> até cada um de <goals>."),
    ("cost.sum_reciprocal", "As transições <a> e <b> como resistências em série."),
    ("cost.to_map", "Um mapa das conexões de cada célula de <transition>, por {reduce}."),
    (
        "cost.transition",
        "A transição entre células vizinhas de <map>, pela função {function} dos seus valores, {directions} vizinhos.",
    ),
    (
        "crafty.allocate",
        "Um passo da competição pela terra do CRAFTY em <land>, pelos capitais <capitals> e a demanda <demand>; \
         semente <seed>.",
    ),
    (
        "data.environment90m",
        "A estatística {stats} das variáveis {variables} do Environment90m em cada sub-bacia de <sub_catchments>.",
    ),
    ("demand.regional", "Uma demanda por região de <regions>, as regiões {ids}."),
    ("demand.table", "Uma demanda dada como tabela no modelo."),
    (
        "dinamica.continuous_patcher",
        "O Patcher contínuo do Dinamica: novas manchas de tamanho médio {mean}, isometria {isometry}, tomam o estoque \
         de <stock> pela <probability> até atingir <quota>[; semente <seed>].",
    ),
    (
        "dinamica.cost_map",
        "O custo de chegar a cada célula a partir da fonte mais próxima de <sources>, sobre o atrito <friction>.",
    ),
    (
        "dinamica.destinations",
        "Destinos sorteados pela atração <attraction>, {per_quadrant} por quadrante[ (divisões {quadrants})][, a \
         {min_distance} de distância][; semente <seed>].",
    ),
    (
        "dinamica.pathways",
        "Caminhos de <sources> até a rede <network>[ pelos custos <cost>][ pelas direções <directions>].",
    ),
    (
        "dinamica.roads",
        "Um passo da construção de estradas: novas estradas de <network> até pontas sorteadas pela <attraction>, sobre \
         o atrito <friction>[, dentro de <mask>][; semente <seed>].",
    ),
    (
        "distance.influence_zones",
        "As zonas de influência dos objetos do primeiro plano de <map>[ ({foreground})][, objetos de {min_area} \
         células ou mais].",
    ),
    (
        "distance.objects_proximity",
        "A proximidade entre objetos vizinhos do primeiro plano de <map>[ ({foreground})][, objetos de {min_area} \
         células ou mais].",
    ),
    (
        "distance.proximity",
        "A distância de cada célula de <raster> à célula mais próxima dos valores {values}[, em {units}][, até \
         {max_distance}].",
    ),
    (
        "dynamic.share_of",
        "A fração das células em volta de cada célula de <landscape> nas classes {classes}[, uma janela de {window}].",
    ),
    (
        "forest_code.app",
        "As áreas de preservação permanente (APP) do Código Florestal[ ao longo dos rios <rivers>][ em volta dos lagos \
         <lakes>][ em volta das nascentes <springs>][ nas encostas <slope>][ nos topos de morro <hilltops>].",
    ),
    (
        "forest_code.reserve",
        "O saldo da Reserva Legal de cada imóvel rural de <properties>, pela sua vegetação nativa <vegetation>[ e as \
         APPs <app>].",
    ),
    (
        "fragmentation.clustering",
        "O agrupamento da área do primeiro plano de <map>[ ({foreground})] numa janela de {window} células.",
    ),
    (
        "fragmentation.contagion",
        "O contágio do primeiro plano de <map>[ ({foreground})] numa janela de {window} células.",
    ),
    ("fragmentation.entropy", "A entropia espacial conjunta do mapa inteiro <map>[, o primeiro plano {foreground}]."),
    (
        "fragmentation.entropy_map",
        "A entropia espacial conjunta de <map>[, o primeiro plano {foreground}] numa janela de {window} células.",
    ),
    (
        "fragmentation.fad",
        "A densidade da área de floresta (FAD) de <map>[, o primeiro plano {foreground}] numa janela de {window} \
         células.",
    ),
    (
        "fragmentation.fad_multiscale",
        "A densidade da área de floresta (FAD) de <map>[, o primeiro plano {foreground}] em janelas de {windows} \
         células.",
    ),
    ("fragmentation.gray_convolution", "A medida {metric} da cobertura <map> numa janela de {window} células."),
    (
        "fragmentation.landscape_mosaic",
        "O mosaico da paisagem de <map>: a mistura de agricultura, terra natural e terra urbanizada numa janela de \
         {window} células, em {classes} classes.",
    ),
    (
        "futures.calibrate",
        "A calibração das manchas do FUTURES no desenvolvimento de <landscape> para <end>, {repeat} simulações por \
         combinação; semente {seed}.",
    ),
    ("futures.devpressure", "A pressão de desenvolvimento de <developed> num raio de {size} células, por {method}."),
    (
        "futures.patch_library",
        "Os tamanhos das manchas desenvolvidas de <start> para <end>[ em cada sub-região de <subregions>].",
    ),
    (
        "futures.simulation",
        "O crescimento de manchas do FUTURES em <landscape>, a partir da pressão de desenvolvimento <pressure>[ nas \
         sub-regiões <subregions>]; semente <seed>.",
    ),
    ("glm.coefficients", "Regressões logísticas dadas no modelo."),
    (
        "hydro.along_network",
        "Os trechos de rio a partir de cada ponto de <points> ao longo de <streams>, {upstream} rio acima e \
         {downstream} rio abaixo, pelas direções de <dem>.",
    ),
    ("hydro.breach", "<dem> com as depressões rompidas por canais de até {max_length} células."),
    (
        "hydro.centrality",
        "O lugar de cada trecho de <links> na sua rede de rios: o grau, a magnitude de Shreve, a intermediação e a \
         proximidade.",
    ),
    (
        "hydro.connectivity_index",
        "O índice de conectividade de <dem>, ponderado por <weight>, roteado por {method}[, os rios <streams>].",
    ),
    (
        "hydro.dci",
        "O índice de conectividade dendrítica da rede de rios <streams> cortada pelas barreiras <barriers>, sobre \
         <dem>[, ponderado por <weights>].",
    ),
    (
        "hydro.distance_to_channel",
        "A distância média que a água percorre de cada célula de <dem> até os rios <streams>[, ponderada por \
         <weights>].",
    ),
    ("hydro.flow_direction", "A direção para onde drena cada célula de <dem>, por {method}."),
    (
        "hydro.hand",
        "A altura de cada célula de <dem> acima da drenagem mais próxima[ <streams>][ (uma acumulação de {threshold} \
         células ou mais)].",
    ),
    (
        "hydro.network_distance",
        "A distância ao longo da rede de rios de <dem> entre cada dois pontos de <points>[, movidos para <streams>].",
    ),
    (
        "hydro.snap",
        "Os pontos <points> movidos para os rios <streams>[, a no máximo {distance}][, de acumulação de \
         {min_accumulation} ou mais].",
    ),
    (
        "hydro.spi",
        "O índice de potência do escoamento de <dem>, a área de contribuição por {method}[ ou <sca>][, a declividade \
         <slope>].",
    ),
    (
        "hydro.sti",
        "O índice de transporte de sedimentos de <dem>, a área de contribuição por {method}[ ou <sca>][, a declividade \
         <slope>], expoentes {sca_exponent} e {slope_exponent}.",
    ),
    ("hydro.stream_links", "Os trechos da rede de rios <streams> pelas direções de <dem>[, terminando em <outlets>]."),
    (
        "hydro.twi",
        "O índice topográfico de umidade de <dem>, a área de contribuição por {method}[ ou <sca>][, a declividade \
         <slope>].",
    ),
    (
        "hydro.upstream",
        "A terra a montante de cada trecho ou ponto em <dem>[ (<links>)][ (<points>)][, as estatísticas de <values>][, \
         a fração de cada classe de <classes>].",
    ),
    ("hydro.upstream_catchment", "A bacia de cada ponto de <points> em <dem>[, movido para <streams>]."),
    ("hydro.watersheds", "A área que drena para cada exutório de <seeds> pelas direções de <dem>."),
    (
        "landscape.contortion",
        "A contorção da borda de cada mancha do primeiro plano de <map>[ ({foreground})], manchas de {min_area} \
         células ou mais.",
    ),
    (
        "landscape.mspa_tiled",
        "Os padrões do MSPA de <map>[, o primeiro plano {foreground}] em blocos de {tile_size} células sobrepostos em \
         {overlap}: conectados por {connectivity} vizinhos, as bordas de {edge_width} células de largura.",
    ),
    (
        "landscape.parcellation",
        "As parcelas de cada classe de <map> e o seu índice de parcelamento, {connectivity} vizinhos.",
    ),
    (
        "landscape.spa",
        "A análise de padrões simplificada (SPA) de <map>[, o primeiro plano {foreground}], em {variant} classes.",
    ),
    ("metrics.cluster", "{k} grupos das linhas de <table> com métricas parecidas[, por {method}]; semente {seed}."),
    (
        "metrics.clustergram",
        "O clustergrama de Schonlau de <table>, de {k_min} a {k_max} grupos[, por {method}]; semente {seed}.",
    ),
    ("metrics.correlation", "A correlação entre as métricas de <table>, por {method}."),
    ("metrics.nmf", "{factors} fatores não negativos das métricas de <table>."),
    ("metrics.pca", "Os componentes principais das métricas de <table>[, {components} mantidos]."),
    (
        "metrics.sample",
        "As métricas da paisagem[ {metrics}] de <map> nas parcelas <plots>[ de tamanho {size}], no nível {level}.",
    ),
    (
        "metrics.series",
        "As métricas da paisagem[ {metrics}] ao longo dos mapas <maps>[ nas zonas <plots>], no nível {level}.",
    ),
    (
        "ml.fit",
        "Uma floresta aleatória ou um gradient boosting[ ({method})] ajustado aos registros[ de <table>][ nos mapas \
         <predictors>][, aprendendo <target>][; semente {seed}].",
    ),
    (
        "ml.importance",
        "A importância por permutação de cada preditor de <model> em <table>, {n_repeats} embaralhamentos; semente \
         {random_state}.",
    ),
    (
        "ml.partial_dependence",
        "Como a predição de <model> em <table> acompanha o preditor {predictor}, uma grade de {grid_resolution} \
         valores.",
    ),
    (
        "ml.potential",
        "O potencial de cada classe de <landscape>[ ({target})] a partir dos fatores <drivers>[ e do mapa posterior \
         <later>][, um modelo {method}][; semente {seed}].",
    ),
    ("ml.predict", "A predição do modelo <model>[ nos mapas <predictors>][ nas linhas de <table>]."),
    (
        "molusce.step",
        "Uma iteração do simulador do MOLUSCE em <landscape>[, pelos pesos <total>][, pelos potenciais <potential>].",
    ),
    ("molusce.total", "O peso total de cada transição do MOLUSCE a partir dos fatores <drivers>."),
    ("move.aeqd_crs", "<tracks> numa projeção azimutal equidistante centrada em {center}, em {units}."),
    ("move.as_tracks", "Trilhas das localizações[ de <vector>][ de <table>][ (colunas {x} e {y})][ em {crs}]."),
    ("move.azimuth", "A direção de cada passo de <tracks>, em sentido horário a partir do norte, em {units}."),
    ("move.distance", "A distância de cada localização de <tracks> até a seguinte[, em {units}]."),
    ("move.distribution", "Uma distribuição {name} de comprimentos de passo ou ângulos de giro."),
    ("move.extract_covariates", "Os valores dos rasters conectados nas localizações de <tracks> (passos: {where})."),
    ("move.filter_min_n_burst", "As rajadas de <tracks> de pelo menos {min_n} localizações."),
    ("move.filter_movebank_visible", "As trilhas <tracks> sem as localizações que o Movebank esconde."),
    ("move.filter_per_interval", "Uma localização de <tracks> por {unit} (critério {criterion})."),
    ("move.fit_distribution", "Uma distribuição {name} ajustada a <values>."),
    (
        "move.interpolate",
        "Localizações de <tracks> interpoladas[ nos tempos {times}][, lacunas de mais de {max_time_lag} s deixadas \
         vazias].",
    ),
    (
        "move.nsd",
        "O deslocamento quadrático líquido de cada localização de <tracks> em relação à primeira da sua trilha.",
    ),
    ("move.random_numbers", "{n} sorteios da distribuição <distribution>; semente {seed}."),
    (
        "move.random_steps",
        "{n_control} passos aleatórios para cada passo observado de <steps>[, comprimentos de <sl_distr>][, ângulos de \
         giro de <ta_distr>]; semente {seed}.",
    ),
    ("move.resample", "As localizações de <tracks> reamostradas a cada {rate}, tolerância {tolerance}, em rajadas."),
    ("move.segments", "Os passos de <tracks> como linhas."),
    ("move.simulate_brownian", "{tracks} trilhas de movimento browniano, difusão {sigma}; semente {seed}."),
    ("move.speed", "A velocidade de cada passo de <tracks>[, em {units}]."),
    (
        "move.steps",
        "Os passos entre localizações consecutivas de <tracks>: o comprimento, a direção e o ângulo de giro.",
    ),
    ("move.time_lags", "O tempo de cada localização de <tracks> até a seguinte, em {units}."),
    ("move.time_of_day", "Dia ou noite em cada localização de <tracks>."),
    ("move.track_data", "Uma linha por trilha de <tracks> com as colunas que não mudam dentro dela."),
    ("move.track_lines", "Cada trilha de <tracks> como uma linha pelas suas localizações."),
    ("move.track_summary", "A distância, a retidão e a sinuosidade de cada trilha de <tracks>."),
    ("move.turn_angle", "A mudança de direção em cada localização de <tracks>, em {units}."),
    ("networks.conefor", "A rede de habitat do mapa do MSPA <map> como entradas do Conefor."),
    (
        "networks.gtn_importance",
        "A importância de cada ligação e trampolim ecológico da rede de habitat do mapa do MSPA <map>.",
    ),
    (
        "nlm.curds",
        "Um mapa binário hierárquico por coalhamento, as frações de matriz {curds} nas divisões {steps}[, com soro \
         {wheyes}]; semente {seed}.",
    ),
    (
        "nlm.edge_gradient",
        "Um gradiente com pico no meio de {rows} × {columns} células[ de lado {resolution}][, voltado para \
         {direction}°]; semente {seed}.",
    ),
    ("nlm.merge", "<primary> mesclado com <secondary> escalado por {scaling}."),
    (
        "nlm.mosaic_field",
        "Um campo aleatório de mosaico de {n} mosaicos em {rows} × {columns} células[ de lado {resolution}]; semente \
         {seed}.",
    ),
    (
        "nlm.mosaic_gibbs",
        "Um mosaico de Voronoi de {patch_classes} classes em {rows} × {columns} células[ de lado {resolution}], \
         {germs} germes a pelo menos {r} células uns dos outros; semente {seed}.",
    ),
    (
        "nlm.mpd",
        "Uma superfície fractal por deslocamento do ponto médio em {rows} × {columns} células[ de lado {resolution}], \
         rugosidade {roughness}; semente {seed}.",
    ),
    (
        "nlm.neigh",
        "Um mapa de {categories} categorias crescidas por vizinhança em {rows} × {columns} células[ de lado \
         {resolution}] (probabilidades {p_neigh} e {p_empty}); semente {seed}.",
    ),
    (
        "nlm.random_rectangular_cluster",
        "Retângulos sobrepostos de valores aleatórios em {rows} × {columns} células[ de lado {resolution}], lados de \
         {minl} a {maxl} células; semente {seed}.",
    ),
    ("nlm.rescale", "<map> reescalado para 0-1."),
    ("patches.settings", "Os parâmetros do Expander e do Patcher dados no modelo."),
    ("plugin.wasm", "Um mapa calculado célula a célula pelo módulo WebAssembly {path}[, a função {function}]."),
    (
        "prioritize.actions",
        "Quais ações contra as ameaças <threats> tomar nas unidades de planejamento <pu> para que <features> atinjam \
         as suas metas, objetivo {model}[ num orçamento de {budget}].",
    ),
    (
        "prioritize.actions_raster",
        "Quais ações contra as ameaças <threats> tomar nas células de <monitoring_cost> para que <features> atinjam as \
         suas metas, objetivo {model}[ num orçamento de {budget}].",
    ),
    ("prioritize.adjacency_matrix", "Quais unidades de planejamento de <units> são adjacentes, {directions} vizinhos."),
    ("prioritize.binary_stack", "Uma camada 0/1 por zona de <zones>[, com os nomes {names}]."),
    (
        "prioritize.boundary_matrix",
        "O comprimento da fronteira que cada duas unidades de planejamento de <units> compartilham.",
    ),
    (
        "prioritize.calibrate",
        "Soluções candidatas que trocam o custo pela penalidade {penalty}, pela abordagem {approach}[ para \
         <features>][ ao custo <cost>][, {n} candidatas].",
    ),
    ("prioritize.coarsen", "O custo <cost> e os alvos <features> somados em blocos de {factor} células de lado."),
    ("prioritize.connectivity_matrix", "A conectividade das unidades de planejamento adjacentes de <units>[ por <y>]."),
    (
        "prioritize.decay_connectivity",
        "A conectividade das unidades de planejamento pelas suas distâncias <distances>, a probabilidade de dispersão \
         0,5 na distância mediana {median}.",
    ),
    ("prioritize.h3_units", "Hexágonos H3 de resolução {resolution} sobre <cost>, como unidades de planejamento."),
    ("prioritize.protected_cells", "As células selecionadas pela solução <solution>."),
    (
        "prioritize.proximity_matrix",
        "Quais unidades de planejamento de <units> estão a até {distance} umas das outras.",
    ),
    (
        "prioritize.proxirr",
        "A insubstituibilidade (ProxIRR) de cada unidade de planejamento[ de <pu>][ dos mapas <features>].",
    ),
    ("prioritize.refine", "A seleção grossa <selection> de volta na grade de <like>."),
    ("prioritize.unit_points", "Um ponto por unidade de planejamento de <units>."),
    ("prioritize.zonation_project", "Roda o projeto do Zonation 5 {path}[ com as opções {flags}][, regra {mode}]."),
    ("raster.cell_area", "A área de cada célula da grade de <map>, em {units}."),
    ("raster.classify", "Os valores de <map> em {n} classes por {method}[, as quebras {breaks}]."),
    ("raster.contour", "As curvas de nível de <map>[ a cada {interval}][ nos níveis {levels}]."),
    ("raster.convert", "<raster> convertido para {format}[ como {type}][, em {path}]."),
    ("raster.correlation", "A correlação entre as camadas de <x>[ e as de <y>], por {method}."),
    (
        "raster.expand",
        "As classes {classes} de <map> expandidas {cells} células sobre as outras, {neighbours} vizinhos.",
    ),
    (
        "raster.fill",
        "<raster> com as células sem valor preenchidas a partir de até {max_distance} células de distância[, \
         suavizadas {smoothing} vezes].",
    ),
    (
        "raster.focal",
        "A estatística {stat} das células de <map> numa janela {shape}[ de raio {radius}] em volta de cada célula.",
    ),
    ("raster.label", "As manchas de <map> numeradas[, as classes {classes}], {neighbours} vizinhos."),
    (
        "raster.local_correlation",
        "A correlação entre <x> e <y> numa janela {shape}[ de raio {radius}] em volta de cada célula.",
    ),
    ("raster.majority", "<map> limpo pelo filtro de maioria, {neighbours} vizinhos, {iterations} vezes."),
    ("raster.match", "<raster> na grade de <like>, por {resampling}."),
    ("raster.mosaic", "Os rasters de <raster> juntados num mosaico[ em {crs}][, células de lado {resolution}]."),
    ("raster.shrink", "As classes {classes} de <map> encolhidas em {cells} células, {neighbours} vizinhos."),
    ("raster.standardize", "<map> posto numa escala comum por {method}."),
    ("raster.tabulate", "A área de cada classe de <classes> em cada zona de <zones>, em {units}."),
    ("raster.viewshed", "O que se vê sobre <map>[ dos observadores <points>][ de ({x}, {y})][, até {radius}]."),
    ("raster.zonal", "As estatísticas de <map> em cada zona de <zones>[: {stats}]."),
    (
        "redd.allocate",
        "A alocação do desmatamento do VT0007 nas classes de vulnerabilidade <classes> pelas frequências <table>[, os \
         <expected> ha esperados][, como observado em <deforestation>].",
    ),
    (
        "redd.assess",
        "A avaliação do VT0007 do desmatamento previsto <density> contra <deforestation>, em células de {grid_area} \
         ha.",
    ),
    (
        "redd.fit",
        "O ajuste do VT0007 das classes de vulnerabilidade <classes> ao desmatamento <deforestation>[ nas subdivisões \
         <subdivisions>].",
    ),
    (
        "redd.nrt",
        "O limiar de risco desprezível do VT0007: a distância <distance> à borda da floresta dentro da qual ocorreu \
         99,5 % de <deforestation>.",
    ),
    (
        "redd.vulnerability",
        "O mapa de vulnerabilidade do VT0007 em {classes} classes[ da distância <distance>][ de <vulnerability>].",
    ),
    (
        "restoration.corridor",
        "O ganho de restaurar o corredor de menor custo entre <a> e <b> através de <resistance>, no habitat <habitat>.",
    ),
    ("restoration.cost_distance", "O custo acumulado de ir de <sources> através de <resistance>."),
    (
        "restoration.ecosystem_areas",
        "A área natural de cada ecorregião de <ecoregions> hoje e antes da conversão, a partir de <land_cover>.",
    ),
    (
        "restoration.ecosystem_targets",
        "Uma meta de restauração de {target} km² repartida entre os ecossistemas de <table> pelo menor risco de \
         colapso, por {method}.",
    ),
    (
        "restoration.intactness",
        "A integridade ecológica de <footprint> num raio de {radius} células[, depois de restaurar <restored>].",
    ),
    (
        "restoration.intactness_gradient",
        "Quanto a integridade ecológica de <footprint> cresce por unidade de cada célula restaurada, a pegada caindo \
         em <reduction>.",
    ),
    (
        "restoration.isochrone",
        "As isócronas do custo de chegar a cada célula a partir dos objetos de <marker> através de <resistance>.",
    ),
    ("restoration.least_cost_path", "O caminho de menor custo entre <a> e <b> através de <resistance>."),
    (
        "restoration.original_vegetation",
        "A vegetação original da terra convertida de <land_cover>[, da cobertura passada <past>][, das ecorregiões \
         <ecoregions>].",
    ),
    ("restoration.resistance", "Uma resistência que cresce com a distância ao habitat de <map>[ ({foreground})]."),
    (
        "restoration.status",
        "Quão conectado está o habitat de <map>[ ({foreground})] e quanto reconectá-lo poderia acrescentar.",
    ),
    ("rng.numpy", "O gerador de números aleatórios do numpy, semente <seed>."),
    (
        "sdm.accessible_area",
        "A área que a espécie de <occurrences> poderia ter alcançado, por {method}[ num raio de {distance} m][ nas \
         regiões <regions>][, recortada por <within>].",
    ),
    (
        "sdm.autocorrelation_range",
        "O alcance da autocorrelação espacial de <env>, {n} células amostradas por camada; semente {seed}.",
    ),
    (
        "sdm.bias_surface",
        "O esforço amostral dos registros <occurrences> como uma densidade de kernel na grade de <env>[, largura de \
         banda {bandwidth}].",
    ),
    ("sdm.boyce", "O índice de Boyce contínuo das predições <presences> contra <available>, por {method}."),
    ("sdm.buffer_folds", "Partições deixa-um-de-fora de <records>, cada registro com um buffer de {size}."),
    (
        "sdm.checkerboard_folds",
        "Partições em tabuleiro de xadrez de <records> nas células de <env> agregadas por {aggregation}.",
    ),
    (
        "sdm.dataset",
        "Os registros de um modelo de distribuição de espécies: as presenças <presences>[ e os pontos de fundo \
         <background>] com os valores de <env>.",
    ),
    (
        "sdm.download_gbif",
        "Os registros de ocorrência de {species} do GBIF[ em {country}][, anos {year}][ dentro de <area>].",
    ),
    (
        "sdm.evaluate",
        "Quão bem as predições separam as presenças <presences> do fundo <background>: AUC, TSS, o índice de Boyce e \
         os limiares.",
    ),
    ("sdm.exdet", "A extrapolação (ExDet) de <env> além das condições <reference>."),
    (
        "sdm.importance",
        "A importância por permutação dos preditores de <model> em <records>, {n_repeats} embaralhamentos; semente \
         {seed}.",
    ),
    ("sdm.jackknife_folds", "Partições jackknife de <records>: cada presença a sua própria partição."),
    (
        "sdm.maxnet",
        "Um modelo de distribuição de espécies MaxNet[ de <data>][ das presenças <presences>][ e do fundo \
         <background>][, as feições {classes}], regularização {regmult}, saída {type}.",
    ),
    (
        "sdm.migrate",
        "A dispersão da espécie a partir de <initial> pela adequabilidade <suitability>, {steps} passos por período, o \
         kernel {kernel}[, as barreiras <barriers>]; semente {seed}.",
    ),
    ("sdm.mop", "A paridade orientada à mobilidade de <env> contra <reference> ({type}, distância {distance})."),
    ("sdm.omission_rate", "A taxa de omissão das presenças de teste <test> no limiar {threshold} de <train>."),
    ("sdm.one_per_cell", "Um registro de <occurrences> por célula de <raster>."),
    ("sdm.quadrant_folds", "Os quatro quadrantes das presenças de <records>, orientação {orientation}."),
    ("sdm.random_folds", "{k} partições aleatórias das presenças de <records>; semente {seed}."),
    ("sdm.reference_data", "Os dados de referência {name} para a limpeza de registros."),
    (
        "sdm.response_curves",
        "As curvas de resposta de <model> sobre os registros <records>, os outros preditores fixados em {fixed}.",
    ),
    (
        "sdm.select_predictors",
        "Os preditores de <env> sem colinearidade, um limiar de VIF de {vif}[, um limiar de correlação de {cor}].",
    ),
    (
        "sdm.spatial_blocks",
        "Blocos espaciais de <records>[ de tamanho {size}] em {k} partições ({selection}); semente {seed}.",
    ),
    ("sdm.spatial_clusters", "{k} agrupamentos espaciais de <records>[, ambientais em <env>]; semente {seed}."),
    ("sdm.threshold", "Um limiar de adequabilidade a partir de <presences> e <background>, por {method}."),
    ("sdm.vif", "O fator de inflação da variância de cada preditor de <env>[ dentro de <area>]."),
    (
        "sensitivity.analyze",
        "Os índices de sensibilidade ({method}) das rodadas <design> a partir das saídas <results>.",
    ),
    ("sensitivity.design", "As rodadas de uma análise de sensibilidade global ({method})[, n = {n}]; semente {seed}."),
    (
        "services.ndr",
        "A entrega de nutrientes do InVEST em <lulc> pela tabela <biophysical>, sobre <dem> e o escoamento \
         <runoff_proxy> em <watersheds>, rios a partir de {threshold} células.",
    ),
    (
        "services.sdr",
        "A entrega de sedimentos do InVEST em <lulc> pela tabela <biophysical>, sobre <dem>, a erosividade <erosivity> \
         e a erodibilidade <erodibility> em <watersheds>, rios a partir de {threshold} células.",
    ),
    (
        "services.seasonal_water_yield",
        "A produção sazonal de água do InVEST sobre <dem> em <watersheds>[ em <lulc>][ pela tabela <biophysical>][ a \
         partir da recarga <recharge>], rios a partir de {threshold} células.",
    ),
    (
        "services.water_yield",
        "A produção anual de água do InVEST de <lulc> pela tabela <biophysical>, a chuva <precipitation> e a \
         evapotranspiração <eto>, sazonalidade {seasonality}[, em <watersheds>].",
    ),
    ("stack.classes", "<stack> com as camadas tomadas como classes ou transições."),
    ("stack.extract_layer", "A camada {layer} de <stack>."),
    ("stack.reduce", "A estatística {stat} das camadas de <maps> em cada célula."),
    ("stats.getis_ord", "Os pontos quentes e frios Gi* de Getis-Ord de <map>[, vizinhos {contiguity}]."),
    (
        "stats.idw",
        "Os valores dos pontos <points> interpolados em <like> pelo inverso da distância, potência {power}[, os {nmax} \
         mais próximos].",
    ),
    (
        "stats.kriging",
        "Os valores dos pontos <points> interpolados em <like> por krigagem ordinária[, o variograma <model>][, os \
         {nmax} mais próximos].",
    ),
    (
        "stats.lisa",
        "O I de Moran local de cada célula de <map>, {permutations} permutações, agrupamentos a {alpha}[, vizinhos \
         {contiguity}]; semente {seed}.",
    ),
    (
        "stats.moran",
        "O I de Moran global de <map>[, vizinhos {contiguity}], {permutations} permutações; semente {seed}.",
    ),
    (
        "stats.trend",
        "A tendência de Mann-Kendall e a inclinação de Sen de cada célula ao longo de <maps>, significância {alpha}.",
    ),
    ("stats.variogram", "O variograma amostral dos pontos <points>[ ({field})] e um modelo {model} ajustado a ele."),
    ("surface.aacf", "A função de autocorrelação areal de <map>."),
    ("surface.bearing", "A curva de área de suporte (Abbott-Firestone) de <map>."),
    ("surface.dissection", "A dissecação de Martonne de <map> numa janela de {s} células."),
    (
        "surface.fit_plane",
        "A tendência de <map>, um polinômio de mínimos quadrados[ de ordem {order}], e a superfície sem ela.",
    ),
    (
        "surface.geomorphons",
        "As formas de relevo de <dem> por geomorphons, um raio de busca de {search}, um limiar de planura de {flat}°.",
    ),
    (
        "surface.gravity",
        "O potencial gravitacional das fontes <sources>[, o decaimento com a distância {exponent}][, mascarado por \
         <mask>].",
    ),
    (
        "surface.hilltop",
        "As APPs de topo de morro de <dem>: os morros de pelo menos {min_height} m de altura e mais íngremes que \
         {min_slope}°[, as bases em <plain>].",
    ),
    (
        "surface.hli",
        "O índice de carga térmica a partir da orientação <aspect> e da declividade <slope>[, na latitude {latitude}].",
    ),
    ("surface.hsp", "A posição hierárquica na encosta de <map>, escalas de {min_scale} a {max_scale}."),
    ("surface.metrics", "As métricas de superfície de <map>[: {metrics}]."),
    ("surface.peaks", "Os picos ou vales de <map>."),
    ("surface.spherical_sd", "O desvio padrão esférico das normais de <map> numa janela de {d} células."),
    ("surface.srr", "A razão de relevo da superfície de <map> numa janela de {s} células."),
    ("surface.trasp", "A orientação transformada de <map>."),
    ("surface.tri", "O índice de rugosidade do terreno de <map> numa janela de {s} células."),
    ("surface.vrm", "A medida vetorial de rugosidade de <map> numa janela de {s} células."),
    ("surface.window", "As métricas de superfície[ {metrics}] de <map> numa janela {shape} de {window} células."),
    ("swat.from_tables", "O projeto SWAT+ de <watershed> e do clima de <stations>[ de {start}][ a {end}], como o SWAT+ Editor o escreve."),
    ("swat.load", "Carrega o projeto SWAT+ {path}."),
    ("swat.run", "Simula o projeto SWAT+[ {path}][ <project>], as tabelas por {step}."),
    ("swat.set", "<project> com {parameter} alterado ({change} {value})[ nas camadas do solo a partir de {first_layer}][ até {last_layer}][ nas HRUs {objects}]."),
    ("swat.watershed", "A bacia de <dem> dividida como o QSWAT+ divide[ com rios a partir de {stream_threshold} km²][ e os exutórios <outlets>], suas HRUs por {hru_method} de <landuse> e <soils>."),
    ("swat.wgn", "As estatísticas do gerador de clima das estações de <pcp>, <tmax> e <tmin>."),
    ("table.aggregate", "As linhas de <table> agrupadas[ por {by}] e resumidas por {stats}[: {columns}]."),
    ("table.calc", "A coluna {column} de <table> calculada como {expression}."),
    ("table.join", "<left> unida a <right> por {on} (junção {how})."),
    ("table.pivot", "<table> pivotada ({direction})[, uma coluna por valor de {names}]."),
    ("table.select", "As colunas de <table>[: {columns}][, sem {drop}]."),
    ("table.sort", "As linhas de <table> ordenadas por {by}[, as primeiras {top} mantidas]."),
    ("uncertainty.delta", "A incerteza de {expression} pelo método delta ({method})."),
    ("uncertainty.interval", "O intervalo de confiança de <value> a partir do seu desvio padrão <sd>, nível {level}."),
    (
        "uncertainty.product_sd",
        "O desvio padrão do produto de <x> e <v>, a partir dos seus desvios padrão <sd_x> e <sd_v>.",
    ),
    (
        "uncertainty.sum_sd",
        "O desvio padrão do total a partir dos desvios padrão <sd>[, ponderados por <weights>], {method}[, correlação \
         {correlation}].",
    ),
    (
        "validate.error_budget",
        "O orçamento de erros do MOLUSCE de <simulated> contra <reference>, em {iterations} resoluções juntando \
         {scale} células de lado.",
    ),
    ("validate.futures", "A validação do FUTURES de <simulated> contra <reference>[, a partir de <original>]."),
    (
        "validate.futures_grid",
        "A validação do FUTURES de <simulated> contra <reference>[ em blocos de {size} células][ por região de \
         <regions>][, a partir de <original>].",
    ),
    (
        "validate.three_map",
        "A comparação de três mapas de <simulated> com a referência de <t0> para <t1>[, blocos de {factors} células].",
    ),
    ("value.calc", "Um valor calculado a partir das entradas[: {expression}]."),
    ("vector.buffer", "A área a até {distance} de cada feição de <vector>."),
    ("vector.clip", "As feições de <vector> recortadas por <mask>."),
    (
        "vector.density",
        "A densidade dos pontos <vector> na grade de <map>, um kernel {kernel}[ de raio {radius}][ ponderado por \
         {field}].",
    ),
    ("vector.dissolve", "As feições de <vector> fundidas[ por {by}]."),
    ("vector.distance", "A distância de cada célula da grade de <like> à feição mais próxima de <vector>."),
    ("vector.exact_sum", "A soma de <map> em cada polígono de <vector>, ponderada pela fração coberta de cada célula."),
    ("vector.from_table", "Pontos das colunas {x} e {y} de <table>[ em {crs}]."),
    ("vector.geometry", "A área, o perímetro, o comprimento e o centroide de cada feição de <vector>."),
    ("vector.grid_zones", "Quadrados de lado {size} cobrindo <map>."),
    ("vector.join", "As colunas de <table> unidas às feições de <vector>[ pelo atributo {on}][ e a coluna {key}]."),
    ("vector.overlay", "A sobreposição ({how}) dos polígonos <vector> e <other>."),
    (
        "vector.rasterize",
        "As feições de <vector> rasterizadas na grade de <like>[, o atributo {field}][, o valor {value}].",
    ),
    ("vector.reproject", "As feições de <vector> reprojetadas[ para {crs}][ para o sistema de <like>]."),
    ("vector.spatial_join", "As feições de <other> unidas às de <vector> onde se encontram ({mode})."),
    ("woe.weights", "Pesos de evidência dados no modelo."),
];

/// The phrase of an operator, if it has one.
pub fn phrase(op: &str) -> Option<&'static str> {
    phrase_in(op, Lang::En)
}

/// The phrase of an operator in a language, if it has one.
pub fn phrase_in(op: &str, lang: Lang) -> Option<&'static str> {
    let table = if lang == Lang::Pt { PHRASES_PT } else { PHRASES };
    table.iter().find(|(o, _)| *o == op).map(|(_, p)| *p)
}

/// The operators with a phrase.
pub fn operators() -> impl Iterator<Item = &'static str> {
    PHRASES.iter().map(|(o, _)| *o)
}

/// A setting's value in words: given, else the operator's default (none: neither).
fn value(n: &Node, sig: Option<&OpSignature>, name: &str, given_only: bool, lang: Lang) -> Option<String> {
    // (an empty text is none given)
    if let Some(v) = n.params.get(name).filter(|v| v.as_str() != Some("")) {
        return Some(crate::document::words_in(v, lang).replace('\n', "; "));
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
fn fill(t: &str, n: &Node, sig: Option<&OpSignature>, given_only: bool, lang: Lang) -> Option<String> {
    let mut out = String::new();
    let mut rest = t;
    while let Some(at) = rest.find(['{', '<']) {
        out.push_str(&rest[..at]);
        let close = if rest[at..].starts_with('{') { '}' } else { '>' };
        let end = at + rest[at..].find(close)?;
        let name = &rest[at + 1..end];
        let v = if close == '}' { value(n, sig, name, given_only, lang)? } else { source(n, name)? };
        out.push_str(&v);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    Some(out)
}

/// The sentence of a step: its operator's phrase with its settings and inputs (none: no phrase, or a setting it
/// needs has no value).
pub fn sentence(n: &Node, sig: Option<&OpSignature>) -> Option<String> {
    sentence_in(n, sig, Lang::En)
}

/// The sentence of a step in a language.
pub fn sentence_in(n: &Node, sig: Option<&OpSignature>, lang: Lang) -> Option<String> {
    let t = phrase_in(&n.op, lang)?;
    let mut out = String::new();
    let mut rest = t;
    while let Some(at) = rest.find('[') {
        out.push_str(&fill(&rest[..at], n, sig, false, lang)?);
        let end = at + rest[at..].find(']')?;
        out.push_str(&fill(&rest[at + 1..end], n, sig, true, lang).unwrap_or_default());
        rest = &rest[end + 1..];
    }
    out.push_str(&fill(rest, n, sig, false, lang)?);
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
                for lang in [Lang::En, Lang::Pt] {
                    if let Some(t) = sentence_in(n, sig.as_ref(), lang) {
                        out.push(format!("{} {}: {t}", n.op, lang.code()));
                    }
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
        // in Portuguese, the same operators, settings and inputs
        let marks = |t: &str| {
            let mut m: Vec<String> = t
                .split(['{', '<'])
                .skip(1)
                .filter_map(|p| p.split_once(['}', '>']).map(|(a, _)| a.to_string()))
                .collect();
            m.sort();
            m
        };
        for op in operators() {
            let pt = phrase_in(op, Lang::Pt).unwrap_or_else(|| panic!("{op}: no phrase in Portuguese"));
            assert_eq!(marks(pt), marks(phrase(op).unwrap()), "{op}");
            assert_eq!(pt.matches('[').count(), phrase(op).unwrap().matches('[').count(), "{op}");
        }
        assert_eq!(PHRASES_PT.len(), PHRASES.len());
        let n = Node::new("habitat", "nlm.binarize").input("map", "aptitude.map").param("breaks", 0.3);
        assert_eq!(
            sentence_in(&n, sig("nlm.binarize").as_ref(), Lang::Pt).as_deref(),
            Some("Aptitude dividido em habitat e matriz, uma fração 0.3 dele habitat.")
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
