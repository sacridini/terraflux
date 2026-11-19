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
        title: "Calculators: raster.calc, vector.calc, vector.filter, objects.calc and every expression",
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
the features kept (vector.filter: the script ends with the condition) or of objects' attributes (objects.calc: \
as vector.calc, over the objects' attributes and `cls`, the class name; `nb.mean(x)`, `nb.count(cond)` and \
`border_to(\"water\")` read the neighbours, `parent.x` the level above, `children.count(cond)` the level \
beneath (the input `below`); `above`, `below`, `between` and `about` give a fuzzy membership from 0 to 1, \
and there `and` is the smaller, `or` the larger and `not` one minus); checked, and tried on a sample of its \
inputs' cells, features or objects, before anything changes: read what each output gives in its reply.
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
- A node that stands for a data of the user's (a map the template draws, to be replaced by the user's file) has \
`data`: `what` (\"a land use map, the first date\"), `kind` (classes, values, mask, stack, image), `classes` (code -> name, \
the classes the method expects) and `expect` (for values: \"1 or more\", a unit). Read it in show_model before \
replacing the node: it says which of the user's files goes there and what its classes must be (reclassify the file \
to them); set it with edit_node (`data`; null clears it) on the nodes that take the place of the user's data in a \
model you build from a template.
- Give the model the user's `request` in the user's words, a `summary` of the conversation and its `author` \
(new_model, set_model_info); update the summary when the model changes.
- check_model names the nodes without a note or a why. methods drafts a paper's methods section from this \
record (the methods, the papers to cite).",
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
- The runs already made: list_runs gives the model's journal, newest first (when, how long, how it ended, the \
scenario, the values changed from the run before, the outputs and whether they are still as written; `id`: one \
run in full). Read it before running again, to see what was tried and which run wrote a file.
- Before a large change, save a version (save_version with a label and a note): a named point the user can go \
back to or review against, with no git (list_versions tells them, with what changed from the one before).
- A figure of the results is a Plot, kept with the model outside the graph (set_plot: its layers, the lowest \
first, a map compared, a table's chart, a title, a saved view; remove_plot): the application's Viewer opens it, \
the Document and the report show it. The saved view (`view: { extent: [west, south, east, north] }`, in the map's \
coordinates) is the frame it is drawn at: the figure, the export and the Document show only that part of the map. \
There is no view.plot node to add.
- Files the model reads that are not where it says (a model from another computer, a folder moved): \
locate_files finds them (in the model's folder and the one above, or the folders given) and makes the model read \
them from there.
- A note on the canvas, for whoever reads the model, is a note of the model outside the graph (set_note: a text put \
somewhere, or a frame around nodes with a title, which follows them; remove_note); show_model lists them under \
[[notes]].
- methods drafts the methods section of a paper from the model (the methods it uses, the papers to cite, the \
model step by step, the references; or BibTeX, RIS).
- Which settings matter to a result: sensitivity runs the analysis (`factors`: the numbers to vary, each \
{key, from, to}; `output`: what is measured, with a `statistic` for a map). Start with Morris (`method`: morris, \
r x (k + 1) runs) to screen many factors, then Sobol (S1 and ST) on the few that matter, with `n` a power of 2 and \
`second_order` only for a pair that interacts; oat (one at a time) for a tornado chart. A model that draws at \
random needs `replicates` (3 to 5: the output is the mean, and the reply says the share of its variance that is \
chance). Read the ranking, the chance share and the convergence verdict (a larger design when the indices have not \
settled) before saying what matters. A design of more than 500 runs is refused until `confirm` is true: the reply \
says its cost, so tell the user first. It writes methods.md, a paragraph for the methods section with its \
references. Record the analysis in the model's notes (set_note, or the node's `note`/`why`): the method, the \
ranges and why, the factors that mattered.",
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
6. Sentinel-2 or Landsat scenes (data.stac): set `mask = \"auto\"` (and `mask_buffer = 1`) to take each scene's \
clouds, shadows and cirrus out per pixel before the composite; a median without it keeps the clouds where they \
persist (the humid tropics), whatever the cloud cover asked for.
7. Many correlated bands or drivers: ml.pca gives their principal components, from a stack (a layer per variable) \
or the columns of a table: scores (a stack pc1, pc2... or the table with the columns added), loadings (as \
metrics.pca's, for the circle of correlations), explained (the variance of each component, for an elbow) and a \
model. `components` is a number or the share of the variance to keep (0.95); `standardise` (the default) makes it \
the correlation, which is right when the variables are in different units. The model scores another image or date \
with ml.predict, using the means of this fit, not of the new data.
8. Groups with no classes given (an unsupervised classification of an image, types of cells or records): ml.kmeans \
clusters a stack or the columns of a table by k-means. `k` is a number, a list, or \"auto\" (the default: the best \
silhouette from 2 to 10); outputs: clusters (a map, 1 to k, NaN where a variable has none), centres (a row per \
cluster in the variables' own units, with its size), choice (inertia, silhouette and Calinski-Harabasz per k) and a \
model for ml.predict. Variables are standardised by default; the clusters are numbered by the first principal \
component of their centres, so the colours do not change between runs.
9. Types of records or cells that follow a gradient, with their neighbourhoods kept (a map of the landscape's \
types, an unsupervised classification whose groups are ordered): ml.som trains a self-organizing map (Kohonen) of a \
stack or the columns of a table. Without `rows` and `cols` the grid has about 5 sqrt(n) nodes (Vesanto's rule); \
`clusters` k groups the nodes in k groups (Ward's method, or `grouping` = \"kmeans\"). Outputs: nodes (a map of each \
cell's node, row by row from 1) and groups (a map, 1 to k), codebook (a row per node: row, column, count, group, \
umatrix, topology, and its weights in the variables' own units), quality (quantisation and topographic error) and a model for \
ml.predict. `topology` is rectangular or hexagonal; `training` batch (the default) or online; variables are \
standardised by default. Guide multivariate: which one, standardising, the automatic choices, change between \
dates, objects.
Templates: \"Drivers of land change\", \"Cleaning a map of classes: filter, sieve, label and grow\", \"Terrain of a \
surface\".",
    },
    Guide {
        name: "multivariate",
        title: "Principal components, k-means and self-organizing maps: images, stacks, tables and objects",
        text: "\
1. Which one. ml.pca when the variables repeat each other (the bands of an image, drivers that follow the relief): \
it turns them into a few uncorrelated layers, the first holding the most variance, to look at (the scores as an \
RGB composite), to feed another model with fewer inputs or to find change between two dates. ml.kmeans when you \
want groups and no classes are given (an unsupervised classification of an image, types of records or of \
objects): every cell or row takes the nearest of k centres. ml.som when the groups should keep their \
neighbourhoods, ordered along a gradient (types of landscape from environmental variables): a grid of nodes taught \
by the cells, the nodes then grouped in a few groups. K-means is the plain choice; the map shows more (U-matrix, a \
plane of each variable, the cells of each node) and is slower to read.
2. The input is a stack (`predictors`, a layer per variable: stack.of; `cells` is a map of the cells to use) or a \
table (`table`, with `columns` naming the variables, by default every numeric one: name them when the table also \
has an id, an area or a class). A cell or row with a variable none stays out of the fit and has no result. \
Maps give a map, a table gives the table with the new columns (pc1..., cluster, node and group).
3. Standardise (`standardise`, the default) when the variables are in different units or scales (rain in mm with a \
slope in degrees, an NDVI with a band of reflectance); a variable of large values would otherwise decide \
everything. Leave it off only for variables of one unit whose differences in scale are meant (bands of one \
sensor, when the bright bands should weigh more). The fit's means and scales stay in the model: ml.predict gives \
another image or date its scores, cluster or node by those, not by the new data's.
4. The choices left out are made, and said in the node's note. ml.pca keeps the components that add up to 95% of \
the variance (`components` is a number, or a share). ml.kmeans takes k = \"auto\": the best silhouette from 2 to \
10 (the choice output and its chart show every k; a list of numbers chooses among those). ml.som takes a grid of \
about 5 sqrt(n) nodes (Vesanto's rule) and, with `clusters` k, groups them by Ward's method. Above a million \
records the fit uses a sample (`sample` sets it, and the whole image is then given its results, by blocks): a \
sample is enough for centres and components, and keeps a large image quick. The same `seed` gives the same \
results, on any number of threads.
5. Read the results before using them. The loadings say what each component is made of (a component whose \
loadings are all one sign is a brightness); `explained` and the elbow chart say how many are worth keeping, and \
a few that add up to little variance are noise. Clusters are groups of alike cells, not classes: look at the \
centres (in the variables' own units) and at the map, and name them; the silhouette's k is a statistical \
answer, a k that suits the question may be better. A map with many nodes reads as the shape of the data, with \
few as a classification: look at its quantisation and topographic errors (quality) and at its empty nodes.
6. Change between two dates: the same bands of both dates as the variables of one ml.pca (stack.of with a layer \
per band and date, on one grid, registered): what the dates share is in the first components and what changed is \
in a minor one, found by its loadings (the dates' opposite signs). Its absolute value is a map of the size of the \
change. It tells where, not what became what: classify the dates for that.
7. Objects: the table of objects.features is a table. ml.kmeans or ml.som on it (`columns` naming the \
attributes, not id, cells or area) and objects.classify with the model (no samples) give every object its cluster \
(\"cluster 1\"...), or the group of its node, or its node: unsupervised classes of objects, as polygons and as a \
map. ml.pca reduces the attributes first when they are many and alike; the scores go to ml.fit as the \
predictors.
Templates: \"Principal components of an image\", \"Unsupervised classification of an image (k-means)\", \"Types of \
landscape by a self-organizing map\", \"Change between two dates by principal components\", \"Unsupervised \
classes of objects\".",
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
weight: check_model warns on collinear drivers. Distances (to roads, rivers, towns and the change already there) are \
the strongest drivers of most models.
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
6. Linkage Mapper's corridors for conservation planning (McRae and Kavanagh 2011): when the cores are known \
(reserves, large patches; a map of ids) and the question is which links to protect, where the current pinches and \
where restoring would pay most, rather than a graph's indices or circuits between all points. In order: \
linkage.pathways (cores, resistance: the links between neighbouring cores by least cost, with their length, the \
paths and the corridors cut at `width`, in cost units), linkage.pinchpoints (the same cores and resistance, and \
pathways' `links` table: Circuitscape inside each corridor, the current largest where the corridor narrows; \
`width` the corridor's cutoff, as pathways'), linkage.barriers (the same inputs, radii in map units: the gain of \
restoring a circle around each cell, `circles` the map). Euclidean distances are between the cells' centres, \
and there is no pinch scenario of all pairs nor trim of the circles by the resistance. Template \"Linkage \
Mapper: corridors, pinch points and barriers\".
7. Rivers: hydro.dci (the dendritic connectivity index of dams). Template \"Rivers: dams and their connectivity \
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
    Guide {
        name: "objects",
        title: "Object-based image analysis (OBIA): segment, describe, classify, check, polygons",
        text: "\
1. Data: an image with a layer per band, named for what it is (blue, green, red, nir, swir1, swir2: the NDVI and \
NDWI attributes find the red, near infrared and green bands by their names; otherwise give `red`, `nir` and \
`green` to objects.features), and, for the context, any raster on the same grid (a height model, another date: \
raster.match first). nlm.image draws a synthetic image of a map of classes, with the truth known, to try a flow \
on.
2. Segment: segment.multiresolution (Baatz and Schape: eCognition's) is the usual choice. Without `scale` it takes \
the first scale segment.scale suggests (ESP, Dragut et al.); look at the objects in the Viewer and raise the scale \
for larger objects, lower it for smaller ones. The scale is in the image's units: reflectance 0 to 1 and 0 to \
10000 need other scales. shape 0.1 and compactness 0.5 are the usual start. The others: segment.slic and \
segment.snic (superpixels: regular, small, many), segment.felzenszwalb, segment.watershed (markers optional), \
segment.meanshift, segment.chessboard and segment.quadtree (the simplest), segment.threshold (a band cut at \
thresholds, or by multi-level Otsu); segment.spectral_difference merges the neighbours of an over-segmentation \
whose means are close; segment.region_growing (the region growing of GRASS i.segment, with seeds and \
bounds optional) and segment.quickshift (modes of the density in colour and position); segment.from_map and \
segment.from_vector bring objects made elsewhere (a map of classes or labels, polygons). segment.optimize \
runs a segmenter at a range of values of one setting and keeps the best by the variance inside the objects and \
Moran's I (USPO), the table of scores beside it.
   Touching objects (crowns, buildings, cells) are divided by objects.split: by the distance to the object's border (the cut is at the neck) or by a surface whose peaks are the centres (a height model); `h`, `min_size` and `max_neck` decide how many pieces it makes, `classes` which objects it works on. The pieces keep the class.
   Objects grown, shrunk and smoothed cell by cell (eCognition's pixel-based resizing, morphology and border optimization) are objects.resize: `mode` grow, shrink, open, close, fill, smooth or refine, by `distance` (map units) or `steps` (cells); a `mask` (from raster.calc) or `into` (classes whose cells may be taken) makes growth geodesic, `max_hole` limits fill, refine needs the `image`. An object cut in two by shrink or open becomes two objects of its class.
3. Describe: objects.features with the image (and `more`: a height model, another date, a class map). `basic` (the \
default) is the mean and the standard deviation of each band, NDVI, the area, the compactness and the texture's \
homogeneity; ask for groups (spectral, shape, texture, context) or `all` when the rules need them. A layer given \
in `more` is named mean_more (a map has its port's name).
4. Classify, by one of these paths, which combine:
   - by samples: points or polygons with a class column (vector.from_table of x, y and class, a drawing, or \
vector.read) go to objects.samples (the object under a point; a polygon picks every object it covers by half), \
which gives the training table; ml.fit with column = class (a random forest works without tuning); then \
objects.classify gives the classes, the probability of each and the margin. Give at least 10 objects per class, \
over their different looks; classes are numbered by their names in alphabetical order. objects.nearest is \
eCognition's nearest neighbour on the same table, with a few spectral attributes chosen.
   - by rules: objects.rules, a hierarchy of classes each with a fuzzy rule (objects.calc's script: above(ndwi, 0, \
0.2), below, between, about; `and` is the smaller and `or` the larger), a class inheriting its parent's rule; an \
object becomes the leaf of the largest membership. The thresholds come from the objects' attributes: read them in \
the table. Chain a second objects.rules for rules that read the neighbours' classes (border_to, nb.count).
   - by a class tree, when each class wants a method of its own inside its mother's region (urban, then buildings \
and roads in it): a control.group per class with the settings class, parent_class and colour, its body reading \
`@region` and giving a `membership` map; the siblings are decided by one objects.resolve. The Model pane's Classes \
button shows and edits the tree.
   - by two dates: objects.temporal binds the classes of two dates by a transition matrix (table.of, a column \
`class` of text, then a column per destination; objects.transition_matrix estimates it) and gives the table of \
what changed to what.
5. Check: objects.sample draws a stratified random sample of the classified objects (at least 50 per class, \
Olofsson et al.) with an empty column `reference`; fill it with the true class and take it to objects.accuracy: the \
confusion matrix, the user's and producer's accuracies, kappa and the area of each class with its confidence \
interval. Never check with the samples the model learned from: keep others apart. Look at the margin too (low \
where the model hesitates: put samples there). A map of the objects' classes is objects.to_map.
6. Polygons: objects.to_vector (`smooth` 1 to 3 cuts the corners; 0 keeps the cell edges when the polygons are \
counted or measured), then vector.write.
Templates: \"Classify an image by clicking samples\" (the main one), \"Classify an image by rules\", \
\"Segment an image into polygons\", \"Land cover change by objects\", \
\"Trees and buildings from a height model\", \"Unsupervised classes of objects\", \"Water by Otsu on the NDWI\", \
\"The scale chosen by itself\". Each draws its image (nlm.image): put the user's in its place with \
Use my data (the node marked as data).
The prompt \"classify this image into water, forest and city\": new_model from the samples template; replace the \
image with the user's (a stack of its bands, named); set the class names in the samples' table to water, forest \
and city (the table's rows are points in the image's coordinates: in the application the user clicks them); \
open_in_app and ask the user to click a few objects of each class in the Viewer; run_model; check the margin and \
ask for more samples where the model hesitates; then objects.sample and objects.accuracy on samples kept apart; \
the polygons are written beside the model.",
    },
    Guide {
        name: "hydrology",
        title: "Terrain hydrology and SWAT+: flow, streams, watersheds, a watershed model",
        text: "\
1. Data: a DEM in a projected coordinate system (metres; a model that needs latitudes, like swat.watershed, asks \
for a coordinate system). Fill its depressions first (hydro.fill, or hydro.breach to cut through them): every \
other step reads the filled DEM.
2. Flow and streams: hydro.flow_direction, hydro.flow_accumulation (the cells draining to each cell), \
hydro.streams (the cells draining a threshold or more: the threshold sets how dense the network is), \
hydro.stream_links, hydro.watersheds. The terrain's indices: hydro.hand, hydro.twi, hydro.spi, hydro.sti, \
hydro.distance_to_channel, hydro.connectivity_index. Gauges and dams on the network: hydro.snap, \
hydro.upstream_catchment, hydro.upstream. Templates \"Terrain of a surface\", \"Rivers: the land upstream and the \
gauges' catchments\", \"Rivers: dams and their connectivity (DCI)\".
3. How much water, when, and how much soil: SWAT+ (the Python library's port of SWAT+ 62, bit for bit for what it covers). \
A project already made (a TxtInOut folder or a zip): swat.load (or the `path` of swat.run), swat.set to change a \
parameter as SWAT+'s hard calibration does (cn2, awc, esco, perco...; pctchg, abschg or absval; a setting may read \
a model input `$x`, which is how batches and calibrations vary it), swat.run for the tables SWAT+ prints. A \
project from maps, as QSWAT+ and the SWAT+ Editor make it: swat.watershed (the DEM, the land use and soil maps \
with their lookup tables of code and name, the soil table; `stream_threshold` in km2, `hru_method` and its \
thresholds), then swat.from_tables (the weather stations and a daily table each of pcp, tmax and tmin: a date or \
year and day, a column per station; swat.wgn computes the weather generator's statistics, which need `monthly` \
values daily series cannot give), then swat.run. Template \"SWAT+ from rasters\".
4. Results on the map: swat.hru_map (a variable of an HRU table: sedyld, surq_gen, wateryld, et...), \
swat.channel_map (a channel's column, the mean discharge by default) and swat.reach_discharge (each channel's \
daily flow). Connect them to the tables of one swat.run (the average annual step `aa` for the HRUs, `day` for the \
channels) so the project is not simulated again for each map; they need the project of swat.from_tables.
5. Trust: look at the water balance first (is evapotranspiration plausible for the climate and the plants? check \
lai and bioms in the plants' table). A model with defaults is not calibrated: compare the daily discharge with \
measured flow (Nash-Sutcliffe, Moriasi et al. 2007) on a period not used to calibrate. Skip the first year \
(`nyskip`) so the soil water and aquifers settle. Say in the notes which `c_factor` was used: it changes the \
sediment by orders of magnitude on a project whose plants do not grow.",
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
        assert_eq!(find("multivariate").map(|g| g.name), Some("multivariate"));
        // (PCA, k-means and SOM: the guide says when to use each and the standardising)
        let text = find("multivariate").unwrap().text;
        for word in ["ml.pca", "ml.kmeans", "ml.som", "standardise", "silhouette", "Vesanto", "objects.classify"] {
            assert!(text.contains(word), "{word}");
        }
        assert!(find("").is_none());
        assert!(guide(Some("nothing")).unwrap_err().contains("land-change"));
        assert!(guide(None).unwrap().contains("connectivity"));
    }
}
