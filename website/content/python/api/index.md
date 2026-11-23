---
title: API reference
description: Every function and class of the Python library, with its parameters, what it returns and an example you can run, generated from its docstrings.
order: 90
---

Every public function and class, with its parameters, what it returns and an example you can run. Each page covers
one topic; the [tutorials](/python/tutorials/) show the same tools at work on real maps. The reference is generated
from the library's docstrings (`website/tools/refresh_python_api.py`), so it always matches the version it
describes, {{pyversion}}.

<!-- no-run -->
```python
import terraflux as tf             # most tools: tf.mspa, tf.fad, tf.Landscape...
tf.conn.protconn(...)               # habitat networks and protected areas
from terraflux import circuit      # Circuitscape
tf.cost.transition(r, "mean", 8)    # distances and routes on grids
tf.surface.metrics(z)               # surface texture of continuous rasters
tf.nlm.random(100, 100, seed=1)     # neutral landscape models
tf.bio.BaseData.from_table(...)     # biodiversity analysis
tf.prioritize.zonation(features)    # spatial conservation prioritization
tf.sdm.prepare(occ, env)            # species distribution models
```

!!! example "Every example runs"
    The examples use the maps that ship with terraflux (`tf.example_data`) or
    small arrays built in place, and they are run as tests, outputs included.
    Copy one into a Python session to try a function.


The pages, one per area:

- **[Data & configuration](/python/api/data/)**: Read, write and reclassify rasters, collections, colour tables, threads.
- **[Data on demand](/python/api/download/)**: WorldClim, CHELSA, Copernicus DEM, SoilGrids, MapBiomas, Hydrography90m, STAC, onto a map's grid.
- **[Vector analysis](/python/api/vector/)**: Predicates and selection by location, joins, nearest feature, overlay, clip, dissolve, buffer, geometry, lines, Voronoi, clusters, coverages (PostGIS-style).
- **[Habitat patterns](/python/api/patterns/)**: MSPA, SPA, objects by size, parcels and contortion.
- **[Fragmentation](/python/api/fragmentation/)**: Density, fragmentation classes, landscape mosaics, contagion, entropy.
- **[Landscape metrics](/python/api/landscape-metrics/)**: Patch, class and landscape metrics; windows, plots, zones, time series.
- **[Habitat networks](/python/api/networks/)**: Habitat graphs, IIC/PC/ECA, importance, ProtConn, grids, metapopulations, MSPA networks.
- **[Circuit connectivity](/python/api/circuits/)**: Omniscape and Circuitscape: current, resistance distance, networks.
- **[Surface metrics](/python/api/surface/)**: Texture of continuous rasters (ISO 25178): roughness, shape, trend surfaces.
- **[Terrain hydrology](/python/api/hydro/)**: Filling, flow directions and accumulation, streams, watersheds, HAND, wetness indices, river networks.
- **[Watershed model (SWAT+)](/python/api/swat/)**: SWAT+ projects from TxtInOut folders or rasters, daily water, sediment and nutrients, results on maps.
- **[Grid distances](/python/api/cost/)**: Transitions, cost surfaces, least-cost and random-walk distances and paths.
- **[Distance & restoration](/python/api/distance/)**: Distances, resistance, least-cost paths, corridors, zones of influence, original vegetation, intactness.
- **[Change detection](/python/api/change/)**: Habitat, fragmentation and landscape-mosaic change between dates.
- **[Large data & batch](/python/api/batch/)**: Folders of rasters, tiled MSPA, maps larger than memory.
- **[Neutral landscape models](/python/api/nlm/)**: Simulated landscapes as null models: random, gradients, clusters, fractals.
- **[Biodiversity analysis](/python/api/biodiversity/)**: Species counts per cell, phylogenies and dissimilarity matrices (Biodiverse).
- **[Spatial prioritization](/python/api/prioritization/)**: Zonation's ranking of the cells for conservation: rules, weights, costs, curves.
- **[Species distributions](/python/api/sdm/)**: Clean records, choose predictors and background, spatial folds, fit, evaluate, project, disperse.
- **[Animal movement](/python/api/move/)**: Movebank tracks, steps, speeds and turns, filters, track lines (move2); random steps for step selection (amt).
- **[Land change simulation](/python/api/simulate/)**: Land use change: transitions, potentials, CLUE / Dinamica / PLUS allocation, validation.
- **[Map accuracy & area](/python/api/accuracy/)**: Stratified samples of a map, its accuracy and the area of its classes (Olofsson et al. 2014).
- **[Deforestation allocation (VT0007)](/python/api/redd/)**: Verra's VT0007 as UDef-ARP: risk threshold, vulnerability, fit, allocation, assessment.
- **[Brazil's Forest Code](/python/api/forest-code/)**: APPs of rivers, lakes, springs, slopes, hilltops; the Reserva Legal's balance per property.
- **[Ecosystem services](/python/api/services/)**: InVEST's models: carbon, habitat quality, water yield and hydropower, sediment delivery.
- **[Uncertainty](/python/api/uncertainty/)**: Totals, products, the delta method, Monte Carlo and confidence intervals.
- **[Sensitivity analysis](/python/api/sensitivity/)**: Morris's elementary effects and Sobol's indices of a model's parameters (SALib's).
- **[Statistics of maps](/python/api/stats/)**: Trends of every cell, Moran's I, LISA, Gi*; kriging, IDW; cell statistics, local correlation.

## All functions and classes

### Data & configuration

| Name | Summary |
|---|---|
| [`tf.load_raster`](/python/api/data/#terraflux.load_raster) | Read one raster, or collect many |
| [`tf.write_raster`](/python/api/data/#terraflux.write_raster) | Write a raster: one band or several, any format GDAL writes |
| [`tf.match_grid`](/python/api/data/#terraflux.match_grid) | Put a raster on the grid of another: same CRS, cells and coordinates |
| [`tf.reproject`](/python/api/data/#terraflux.reproject) | Reproject a raster or vector to another CRS, or onto the grid of another raster |
| [`tf.clip`](/python/api/data/#terraflux.clip) | Cut a raster to a region: the study area, a biome, a municipality |
| [`tf.reclassify`](/python/api/data/#terraflux.reclassify) | Give the values of a raster new values: from a table, or into a habitat map |
| [`tf.load_vector`](/python/api/data/#terraflux.load_vector) | Read a vector layer (polygons, lines, points) as a GeoDataFrame |
| [`tf.write_vector`](/python/api/data/#terraflux.write_vector) | Write a vector layer, the format given by the file extension |
| [`tf.rasterize`](/python/api/data/#terraflux.rasterize) | Burn vector features into a raster on the grid of another raster |
| [`tf.zonal_stats`](/python/api/data/#terraflux.zonal_stats) | Statistics of a raster in every zone: polygons, or the labels of a raster |
| [`tf.zonal_map`](/python/api/data/#terraflux.zonal_map) | Every cell given a statistic of its zone (zonal statistics as a raster) |
| [`tf.tabulate_area`](/python/api/data/#terraflux.tabulate_area) | The area of every class of a map in every zone of another (ArcGIS's Tabulate Area) |
| [`tf.extract`](/python/api/data/#terraflux.extract) | The values of a raster at points |
| [`tf.load_table`](/python/api/data/#terraflux.load_table) | Read a table, the format given by the file extension |
| [`tf.write_table`](/python/api/data/#terraflux.write_table) | Write a table, the format given by the file extension |
| [`tf.save`](/python/api/data/#terraflux.save) | Save data or a result, each part in the format that suits it |
| [`tf.load_result`](/python/api/data/#terraflux.load_result) | Read back a result folder written by `save` (or ``result.save``) |
| [`tf.plot`](/python/api/data/#terraflux.plot) | Plot anything: a raster, a vector layer, a table or the result of a tool, as it calls for |
| [`tf.Plot`](/python/api/data/#terraflux.Plot) | A plot made by `plot`: interactive maps and charts (or a static figure) |
| [`tf.example_data`](/python/api/data/#terraflux.example_data) | Path of an example data set shipped with the package |
| [`tf.RasterCollection`](/python/api/data/#terraflux.RasterCollection) | Many rasters handled as one object; pixels are read only when processed |
| [`tf.RasterInfo`](/python/api/data/#terraflux.RasterInfo) | Header of one raster of a collection (the pixels are not read) |
| [`tf.validate_mspa_input`](/python/api/data/#terraflux.validate_mspa_input) | Check that ``data`` is a habitat map MSPA can process |
| [`tf.validate_fad_input`](/python/api/data/#terraflux.validate_fad_input) | Check that ``data`` is a habitat map the density and object tools accept |
| [`tf.InvalidInputError`](/python/api/data/#terraflux.InvalidInputError) | The raster cannot be processed by the requested tool |
| [`tf.mspa_colormap`](/python/api/data/#terraflux.mspa_colormap) | Colour table of MSPA class maps (`mspa`) |
| [`tf.fad_colormap`](/python/api/data/#terraflux.fad_colormap) | Colour table of density maps of `fad`, `fad_multiscale` and `fos` (6 classes) |
| [`tf.fad5_colormap`](/python/api/data/#terraflux.fad5_colormap) | Colour table of 5-class fragmentation maps (`fos` with ``classes=5``) |
| [`tf.lm_colormap`](/python/api/data/#terraflux.lm_colormap) | Colour table of landscape mosaic maps (`landscape_mosaic`) |
| [`tf.accounting_colormap`](/python/api/data/#terraflux.accounting_colormap) | Colour table of object size-class maps (`accounting`) |
| [`tf.change_colormap`](/python/api/data/#terraflux.change_colormap) | Colour table of change maps (`simple_change`, `morphological_change`) |
| [`tf.foschange_colormap`](/python/api/data/#terraflux.foschange_colormap) | Colour table of fragmentation change maps (`fos_change`) |
| [`tf.dist_colormap`](/python/api/data/#terraflux.dist_colormap) | Colour table of distance maps (`euclidean_distance`) |
| [`tf.entropy_colormap`](/python/api/data/#terraflux.entropy_colormap) | Colour table of spatial entropy maps (`spatial_entropy_map`) |
| [`tf.contortion_colormap`](/python/api/data/#terraflux.contortion_colormap) | Colour table of contortion maps (`contortion`) |
| [`tf.resistance_colormap`](/python/api/data/#terraflux.resistance_colormap) | Colour table of resistance and isochrone maps (`distance_to_resistance`, `isochrone_map`) |
| [`tf.gtn_components_colormap`](/python/api/data/#terraflux.gtn_components_colormap) | Colour table of network component maps (`gtn_components`) |
| [`tf.gtn_importance_colormap`](/python/api/data/#terraflux.gtn_importance_colormap) | Colour table of network importance maps (`gtn_importance`) |
| [`tf.default_threads`](/python/api/data/#terraflux.default_threads) | The number of threads a parallel function uses when not given one |
| [`tf.set_default_threads`](/python/api/data/#terraflux.set_default_threads) | Set the default thread count of every parallel function for this process |
| [`tf.max_threads`](/python/api/data/#terraflux.max_threads) | Number of processors the compiled core can use |
| [`tf.doctor`](/python/api/data/#terraflux.doctor) | Print what terraflux finds on this computer |

### Data on demand

| Name | Summary |
|---|---|
| [`tf.download.worldclim`](/python/api/download/#terraflux.download.worldclim) | WorldClim 2.1's climate on a grid: the historical climate (1970-2000) or a CMIP6 future |
| [`tf.download.chelsa`](/python/api/download/#terraflux.download.chelsa) | CHELSA 2.1's bioclimatic variables on a grid: 1981-2010, or a CMIP6 future |
| [`tf.download.copernicus_dem`](/python/api/download/#terraflux.download.copernicus_dem) | The Copernicus DEM (GLO-30 or GLO-90) on a grid, from its tiles of a degree over the grid |
| [`tf.download.soilgrids`](/python/api/download/#terraflux.download.soilgrids) | SoilGrids 2.0's properties on a grid, at depths |
| [`tf.download.mapbiomas`](/python/api/download/#terraflux.download.mapbiomas) | MapBiomas Brazil's land use and cover of years on a grid (collections 9, 10 and 11) |
| [`tf.download.hydrography90m`](/python/api/download/#terraflux.download.hydrography90m) | Hydrography90m (Amatulli et al. 2022) on a grid: the river network of the world at 90 m |
| [`tf.download.environment90m`](/python/api/download/#terraflux.download.environment90m) | The environment of every sub-catchment of a map of Hydrography90m's sub-catchments, from Environment90m |
| [`tf.download.Environment90m`](/python/api/download/#terraflux.download.Environment90m) | The environment of Hydrography90m's sub-catchments (`environment90m`) |
| [`tf.download.raster`](/python/api/download/#terraflux.download.raster) | Any raster GDAL reads (a URL, a file), or several, warped onto a grid; only the part the grid covers is read |
| [`tf.download.stac`](/python/api/download/#terraflux.download.stac) | The scenes of a STAC catalogue over a grid, each asset warped onto it and the scenes combined |

### Vector analysis

| Name | Summary |
|---|---|
| [`tf.vector.select`](/python/api/vector/#terraflux.vector.select) | The features of ``a`` for which ``predicate`` holds with at least one feature of ``b`` (selection by location) |
| [`tf.vector.disjoint`](/python/api/vector/#terraflux.vector.disjoint) | The features of ``a`` that meet **no** feature of ``b`` (not ``intersects`` with any) |
| [`tf.vector.dwithin`](/python/api/vector/#terraflux.vector.dwithin) | The features of ``a`` within ``distance`` (in the CRS units, planar, inclusive) of at least one feature of ``b`` |
| [`tf.vector.relate`](/python/api/vector/#terraflux.vector.relate) | Which features of ``a`` satisfy ``predicate`` with at least one feature of ``b``: a bool array, one per feature |
| [`tf.vector.count`](/python/api/vector/#terraflux.vector.count) | How many features of ``b`` satisfy ``predicate`` with each feature of ``a``: an int64 array, one per feature |
| [`tf.vector.distance`](/python/api/vector/#terraflux.vector.distance) | The distance from each feature of ``a`` to the nearest feature of ``b``: a float64 array, one per feature |
| [`tf.vector.join`](/python/api/vector/#terraflux.vector.join) | Spatial join: attributes of ``b`` on the features of ``a`` that satisfy ``predicate`` with them |
| [`tf.vector.nearest`](/python/api/vector/#terraflux.vector.nearest) | Each feature of ``a`` with the attributes of the nearest feature of ``b`` and the distance to it |
| [`tf.vector.overlay`](/python/api/vector/#terraflux.vector.overlay) | Two layers overlaid: the pieces where their features meet, and the parts of one that the other does not cover |
| [`tf.vector.intersection`](/python/api/vector/#terraflux.vector.intersection) | The pieces where the features of ``a`` and ``b`` meet, with both attributes: ``overlay(a, b, "intersection")`` |
| [`tf.vector.difference`](/python/api/vector/#terraflux.vector.difference) | Each feature of ``a`` less the ``b`` it meets, attributes of ``a``: ``overlay(a, b, "difference")`` |
| [`tf.vector.symmetric_difference`](/python/api/vector/#terraflux.vector.symmetric_difference) | What is in ``a`` or in ``b`` but not both: ``overlay(a, b, "symmetric_difference")`` |
| [`tf.vector.identity`](/python/api/vector/#terraflux.vector.identity) | All of ``a``, cut where ``b`` meets it, with ``b``'s attributes there: ``overlay(a, b, "identity")`` |
| [`tf.vector.union`](/python/api/vector/#terraflux.vector.union) | With two layers, their overlay union; with one, all its features merged into one (``dissolve(a)``) |
| [`tf.vector.clip`](/python/api/vector/#terraflux.vector.clip) | Each feature of ``a`` cut to the area of ``mask`` (the union of its polygons); those outside it are left out |
| [`tf.vector.dissolve`](/python/api/vector/#terraflux.vector.dissolve) | The features merged: all into one, or one per value of the column ``by``, with its value and ``count`` |
| [`tf.vector.buffer`](/python/api/vector/#terraflux.vector.buffer) | The area within ``distance`` of every feature (``segments`` per quarter circle), its attributes kept |
| [`tf.vector.measures`](/python/api/vector/#terraflux.vector.measures) | The features with the columns ``area``, ``perimeter``, ``length``, ``x`` and ``y``, as the app's ``vector.geometry`` (QGIS's *Add geometry attributes*), in the units of the CRS |
| [`tf.vector.centroid`](/python/api/vector/#terraflux.vector.centroid) | The centroid of each feature, as a point |
| [`tf.vector.hull`](/python/api/vector/#terraflux.vector.hull) | The convex or concave hull of each feature |
| [`tf.vector.simplify`](/python/api/vector/#terraflux.vector.simplify) | Each geometry simplified (Douglas-Peucker) with ``tolerance``, in the units of the CRS |
| [`tf.vector.envelope`](/python/api/vector/#terraflux.vector.envelope) | The bounding box of each feature, as a polygon |
| [`tf.vector.make_valid`](/python/api/vector/#terraflux.vector.make_valid) | Each invalid geometry repaired (GEOS's linework method: a bow-tie polygon becomes a multipolygon); valid ones are kept as they are |
| [`tf.vector.explode`](/python/api/vector/#terraflux.vector.explode) | Multi-part features split into single parts, one feature per part with the attributes repeated, in the order of the features and, in each, of the parts |
| [`tf.vector.boundary`](/python/api/vector/#terraflux.vector.boundary) | The boundary of each feature: the rings of a polygon as lines, the end points of a line as points (a closed line has none: an empty geometry); the boundary of a point is empty |
| [`tf.vector.points_along`](/python/api/vector/#terraflux.vector.points_along) | Points along each line, at a fixed spacing or in a fixed number |
| [`tf.vector.substring`](/python/api/vector/#terraflux.vector.substring) | Each line (each part of a MultiLineString) cut between the measures ``start`` and ``end`` |
| [`tf.vector.split_lines`](/python/api/vector/#terraflux.vector.split_lines) | Each line cut into consecutive pieces of at most ``length`` |
| [`tf.vector.locate`](/python/api/vector/#terraflux.vector.locate) | For each feature of ``a``, the nearest line of ``lines`` and the measure along it of the closest point |
| [`tf.vector.shortest_line`](/python/api/vector/#terraflux.vector.shortest_line) | The line from each feature of ``a`` to its nearest feature of ``b`` |
| [`tf.vector.closest_point`](/python/api/vector/#terraflux.vector.closest_point) | Each point of ``a`` moved to the nearest point of the nearest feature of ``b`` |
| [`tf.vector.voronoi`](/python/api/vector/#terraflux.vector.voronoi) | The Voronoi polygon of each point: one polygon per point, with the attributes of ``a``, in its order |
| [`tf.vector.delaunay`](/python/api/vector/#terraflux.vector.delaunay) | The Delaunay triangulation of all the vertices of ``a``: triangles, or the edges as lines with ``edges=True`` |
| [`tf.vector.cluster`](/python/api/vector/#terraflux.vector.cluster) | The features of ``a`` with an int64 column of cluster ids: DBSCAN on the distance between geometries |
| [`tf.vector.smooth`](/python/api/vector/#terraflux.vector.smooth) | Lines and polygons smoothed by Chaikin's corner cutting, ``iterations`` times (1 to 10) |
| [`tf.vector.coverage_simplify`](/python/api/vector/#terraflux.vector.coverage_simplify) | The polygons of a coverage simplified together (Visvalingam-Whyatt, GEOS), in the units of the CRS |
| [`tf.vector.coverage_validate`](/python/api/vector/#terraflux.vector.coverage_validate) | The edges of a polygon coverage that overlap or leave a gap with their neighbours, per feature |

### Habitat patterns

| Name | Summary |
|---|---|
| [`tf.mspa`](/python/api/patterns/#terraflux.mspa) | Morphological Spatial Pattern Analysis, identical to GuidosToolbox's MSPA |
| [`tf.mspa_stats`](/python/api/patterns/#terraflux.mspa_stats) | Area of every MSPA class |
| [`tf.spa`](/python/api/patterns/#terraflux.spa) | Simplified pattern analysis: the habitat in 2, 3, 5 or 6 classes |
| [`tf.SPAResult`](/python/api/patterns/#terraflux.SPAResult) | Result of `spa` |
| [`tf.accounting`](/python/api/patterns/#terraflux.accounting) | Group the foreground objects into size classes (GTB Accounting) |
| [`tf.AccountingResult`](/python/api/patterns/#terraflux.AccountingResult) | Result of `accounting` |
| [`tf.parcellation`](/python/api/patterns/#terraflux.parcellation) | How finely the landscape is divided into parcels |
| [`tf.ParcellationResult`](/python/api/patterns/#terraflux.ParcellationResult) | Results from landscape parcellation analysis |
| [`tf.contortion`](/python/api/patterns/#terraflux.contortion) | How convoluted the boundary of every habitat object is |

### Fragmentation

| Name | Summary |
|---|---|
| [`tf.fad`](/python/api/fragmentation/#terraflux.fad) | Compute Forest Area Density (FAD) for a single window size |
| [`tf.FADResult`](/python/api/fragmentation/#terraflux.FADResult) | Result of single-scale FAD analysis (`fad`) |
| [`tf.fad_multiscale`](/python/api/fragmentation/#terraflux.fad_multiscale) | Compute Multiscale Forest Area Density (FAD-MS) matching GuidosToolbox |
| [`tf.FADMultiscaleResult`](/python/api/fragmentation/#terraflux.FADMultiscaleResult) | Result of multiscale FAD analysis (`fad_multiscale`) |
| [`tf.fos`](/python/api/fragmentation/#terraflux.fos) | Fragmentation at a Fixed Observation Scale (GTB FOS) |
| [`tf.FOSResult`](/python/api/fragmentation/#terraflux.FOSResult) | Result of `fos` |
| [`tf.landscape_mosaic`](/python/api/fragmentation/#terraflux.landscape_mosaic) | Compute Landscape Mosaic (LM) moving-window metric matching GuidosToolbox batch_lm |
| [`tf.landscape_mosaic_stats`](/python/api/fragmentation/#terraflux.landscape_mosaic_stats) | Compute Landscape Mosaic 103-class frequency distribution matching GTB heatmap.csv |
| [`tf.landscape_mosaic_heatmap`](/python/api/fragmentation/#terraflux.landscape_mosaic_heatmap) | Heatmap of a Landscape Mosaic, as GTB computes it |
| [`tf.LMHeatmap`](/python/api/fragmentation/#terraflux.LMHeatmap) | Landscape Mosaic heatmap of one map (GTB ``*_heatmap.sav`` / ``.csv``) |
| [`tf.contagion_pff`](/python/api/fragmentation/#terraflux.contagion_pff) | Compute moving-window Contagion (Pff) matching GuidosToolbox batch_pff |
| [`tf.clustering_fac`](/python/api/fragmentation/#terraflux.clustering_fac) | Compute moving-window Foreground Area Clustering (FAC) matching GuidosToolbox batch_fac |
| [`tf.spatial_entropy`](/python/api/fragmentation/#terraflux.spatial_entropy) | Compute global normalized spatial joint directional entropy index |
| [`tf.SpatialEntropyResult`](/python/api/fragmentation/#terraflux.SpatialEntropyResult) | Result of global spatial entropy analysis (`spatial_entropy`) |
| [`tf.spatial_entropy_map`](/python/api/fragmentation/#terraflux.spatial_entropy_map) | Compute moving-window spatial entropy map matching GuidosToolbox batch_ent |
| [`tf.gray_spatial_convolution`](/python/api/fragmentation/#terraflux.gray_spatial_convolution) | Compute moving-window spatial convolution metric over a grayscale raster |

### Landscape metrics

| Name | Summary |
|---|---|
| [`tf.Landscape`](/python/api/landscape-metrics/#terraflux.Landscape) | Categorical raster prepared for landscape metrics |
| [`tf.landscape_metrics`](/python/api/landscape-metrics/#terraflux.landscape_metrics) | Landscape metrics of one or several rasters, in long format |
| [`tf.list_metrics`](/python/api/landscape-metrics/#terraflux.list_metrics) | Available landscape metrics |
| [`tf.sample_metrics`](/python/api/landscape-metrics/#terraflux.sample_metrics) | Landscape metrics of sample plots, like landscapemetrics' ``sample_lsm`` |
| [`tf.scale_sample`](/python/api/landscape-metrics/#terraflux.scale_sample) | Landscape metrics of buffers of several sizes around points, like landscapemetrics' ``scale_sample`` |
| [`tf.extract_metrics`](/python/api/landscape-metrics/#terraflux.extract_metrics) | Patch metrics of the patches under points, like landscapemetrics' ``extract_lsm`` |
| [`tf.window_metrics`](/python/api/landscape-metrics/#terraflux.window_metrics) | Landscape-level metrics in a moving window, like landscapemetrics' ``window_lsm`` |
| [`tf.zonal_metrics`](/python/api/landscape-metrics/#terraflux.zonal_metrics) | Landscape and class metrics of every zone, as a GeoDataFrame |
| [`tf.grid_zones`](/python/api/landscape-metrics/#terraflux.grid_zones) | Square zones tiling a raster, for zonal analyses |
| [`tf.SpatioTemporalAnalysis`](/python/api/landscape-metrics/#terraflux.SpatioTemporalAnalysis) | Landscape metrics along a time series of rasters (pylandstats' ``SpatioTemporalAnalysis`` and its zonal, buffer and grid variants) |
| [`tf.SpatialSignatureAnalysis`](/python/api/landscape-metrics/#terraflux.SpatialSignatureAnalysis) | Landscapes (or zones) described by their metrics: the "spatial signature" matrix, its principal components and clusters (pylandstats' ``SpatialSignatureAnalysis``) |
| [`tf.lsm.PCAResult`](/python/api/landscape-metrics/#terraflux.lsm.PCAResult) | Principal components of the spatial signatures |
| [`tf.lsm.NMFResult`](/python/api/landscape-metrics/#terraflux.lsm.NMFResult) | Non-negative factors of the spatial signatures (min-max scaled metrics ~ scores @ components.T) |
| [`tf.lsm.ClustergramResult`](/python/api/landscape-metrics/#terraflux.lsm.ClustergramResult) | Clusters for a range of k (Schonlau's clustergram, as the clustergram package that pylandstats' ``get_cgram`` uses) |
| [`tf.show_patches`](/python/api/landscape-metrics/#terraflux.show_patches) | Map of the patches, each in its own colour (landscapemetrics' ``show_patches``) |
| [`tf.show_cores`](/python/api/landscape-metrics/#terraflux.show_cores) | Core and edge cells of the patches (landscapemetrics' ``show_cores``): core cells dark, edge cells light, other classes blank |
| [`tf.show_lsm`](/python/api/landscape-metrics/#terraflux.show_lsm) | A patch-level metric painted on the patches (landscapemetrics' ``show_lsm``), with a colour bar; ``labels`` writes the values |
| [`tf.show_correlation`](/python/api/landscape-metrics/#terraflux.show_correlation) | Heatmap of the correlations between metrics (landscapemetrics' ``show_correlation``), one panel per level |
| [`tf.check_landscape`](/python/api/landscape-metrics/#terraflux.check_landscape) | Is a raster suitable for landscape metrics? Like landscapemetrics' ``check_landscape`` |
| [`tf.calculate_correlation`](/python/api/landscape-metrics/#terraflux.calculate_correlation) | Correlation between metrics, like landscapemetrics' ``calculate_correlation`` |
| [`tf.raster_boundaries`](/python/api/landscape-metrics/#terraflux.raster_boundaries) | landscapemetrics' ``get_boundaries`` on the values of a raster |
| [`tf.construct_buffer`](/python/api/landscape-metrics/#terraflux.construct_buffer) | Square or circular sample plots around points, like landscapemetrics' ``construct_buffer`` |
| [`tf.pad_raster`](/python/api/landscape-metrics/#terraflux.pad_raster) | Add rows and columns of ``pad_raster_value`` around a raster, like landscapemetrics' ``pad_raster`` |
| [`tf.unpad_raster`](/python/api/landscape-metrics/#terraflux.unpad_raster) | Remove ``unpad_raster_cells`` rows and columns on each side of a raster, like landscapemetrics' ``unpad_raster`` (the inverse of `pad_raster`) |
| [`tf.raster_to_points`](/python/api/landscape-metrics/#terraflux.raster_to_points) | Cell centres and values, like landscapemetrics' ``raster_to_points`` |
| [`tf.get_unique_values`](/python/api/landscape-metrics/#terraflux.get_unique_values) | The distinct values of rasters (NA left out, doubles truncated to integers), sorted, like landscapemetrics' ``get_unique_values`` |

### Habitat networks

| Name | Summary |
|---|---|
| [`tf.HabitatGraph`](/python/api/networks/#terraflux.HabitatGraph) | Habitat patches (nodes with an attribute) and the connections between them |
| [`tf.conn.node_table`](/python/api/networks/#terraflux.conn.node_table) | Node ids and attributes, as Makurhini's ``nodesfile`` |
| [`tf.conn.distance_table`](/python/api/networks/#terraflux.conn.distance_table) | Euclidean distances between nodes, as Makurhini's ``distancefile`` |
| [`tf.conn.settlement_probabilities`](/python/api/networks/#terraflux.conn.settlement_probabilities) | Probabilities of settling in one patch from another under a spatial absorbing Markov chain |
| [`tf.conn.boundary_vertices`](/python/api/networks/#terraflux.conn.boundary_vertices) | Vertices of a polygon picked as Makurhini's ``SR_vertices`` does (systematic) |
| [`tf.conn.protconn`](/python/api/networks/#terraflux.conn.protconn) | Protected Connected land (ProtConn) of a region, as Makurhini's ``MK_ProtConn`` |
| [`tf.conn.protconn_regions`](/python/api/networks/#terraflux.conn.protconn_regions) | ProtConn of several regions, one row per region and distance (Makurhini's ``MK_ProtConnMult``) |
| [`tf.conn.protconn_raster`](/python/api/networks/#terraflux.conn.protconn_raster) | ProtConn of a region from a raster of protected areas, as Makurhini's ``MK_ProtConn_raster`` |
| [`tf.conn.protconn_nodes`](/python/api/networks/#terraflux.conn.protconn_nodes) | Nodes of ProtConn in a region, as Makurhini's ``Protconn_nodes`` builds them |
| [`tf.conn.ProtConnNodes`](/python/api/networks/#terraflux.conn.ProtConnNodes) | The nodes of a ProtConn analysis of one region (`protconn_nodes`) |
| [`tf.conn.protconn_indicators`](/python/api/networks/#terraflux.conn.protconn_indicators) | ProtConn and its fractions from prepared nodes and distances (Makurhini's ``get_protconn_grid``) |
| [`tf.conn.make_grid`](/python/api/networks/#terraflux.conn.make_grid) | Grid of cells of `cell_area` over a region, as Makurhini's ``make_grid`` |
| [`tf.conn.connectivity_grid`](/python/api/networks/#terraflux.conn.connectivity_grid) | ProtConn (or PC and ECA) in every cell of a grid, as Makurhini's ``MK_Connect_grid`` |
| [`tf.conn.eca_change_grid`](/python/api/networks/#terraflux.conn.eca_change_grid) | ECA change between times in every cell of a grid, as Makurhini's ``MK_dECA_grid`` |
| [`tf.conn.eca_change`](/python/api/networks/#terraflux.conn.eca_change) | ECA of habitat graphs over time and its change, as Makurhini's ``MK_dECA`` |
| [`tf.conn.focal_nodes`](/python/api/networks/#terraflux.conn.focal_nodes) | IIC or PC of every node's neighbourhood and the node's importance in it (``MK_Focal_nodes``) |
| [`tf.conn.metapopulation`](/python/api/networks/#terraflux.conn.metapopulation) | A metapopulation in a network of habitat patches over time, Hanski's incidence function model |
| [`tf.conn.Metapopulation`](/python/api/networks/#terraflux.conn.Metapopulation) | A metapopulation's simulation (`metapopulation`) |
| [`tf.gtn_components`](/python/api/networks/#terraflux.gtn_components) | Components of the habitat network formed by MSPA Cores and Bridges |
| [`tf.GTNComponentsResult`](/python/api/networks/#terraflux.GTNComponentsResult) | Result of `gtn_components` |
| [`tf.gtn_importance`](/python/api/networks/#terraflux.gtn_importance) | Importance of every link (Bridge) and stepping-stone node (Core) of the MSPA network |
| [`tf.GTNImportanceResult`](/python/api/networks/#terraflux.GTNImportanceResult) | Result of `gtn_importance` |
| [`tf.conefor_inputs`](/python/api/networks/#terraflux.conefor_inputs) | Nodes and links of the MSPA network as Conefor Sensinode input files |

### Circuit connectivity

| Name | Summary |
|---|---|
| [`tf.omniscape`](/python/api/circuits/#terraflux.circuit.omniscape.omniscape) | Omnidirectional connectivity with Omniscape's moving-window circuits |
| [`circuit.omniscape_ini`](/python/api/circuits/#terraflux.circuit.omniscape_ini) | Run an Omniscape INI file, as ``run_omniscape(path)`` does |
| [`circuit.source_from_resistance`](/python/api/circuits/#terraflux.circuit.source_from_resistance) | Source strengths from the resistance, as Omniscape's ``source_from_resistance`` |
| [`circuit.read_reclass_table`](/python/api/circuits/#terraflux.circuit.read_reclass_table) | Read an Omniscape reclass table |
| [`circuit.reclassify`](/python/api/circuits/#terraflux.circuit.reclassify) | Reclassify a resistance array, as Omniscape's ``reclassify_resistance!`` |
| [`circuit.parse_ini`](/python/api/circuits/#terraflux.circuit.parse_ini) | Read an INI file as Omniscape's ``parse_cfg`` does |
| [`circuit.circuitscape_pairwise`](/python/api/circuits/#terraflux.circuit.circuitscape_pairwise) | Circuitscape's pairwise mode on rasters (``scenario = pairwise``) |
| [`circuit.circuitscape_one_to_all`](/python/api/circuits/#terraflux.circuit.circuitscape_one_to_all) | Circuitscape's one-to-all (or, with ``all_to_one``, all-to-one) mode on rasters |
| [`circuit.circuitscape_ini`](/python/api/circuits/#terraflux.circuit.circuitscape_ini) | Run a Circuitscape INI file, as ``Circuitscape.compute(path)`` does |
| [`circuit.CircuitscapeResult`](/python/api/circuits/#terraflux.circuit.CircuitscapeResult) | Circuitscape's outputs of a raster run |
| [`circuit.FocalPoints`](/python/api/circuits/#terraflux.circuit.FocalPoints) | Focal points as Circuitscape reads them: 1-based rows and columns and ids, sorted by id |
| [`circuit.IncludedPairs`](/python/api/circuits/#terraflux.circuit.IncludedPairs) | Circuitscape's include / exclude pairs |
| [`circuit.circuitscape_advanced`](/python/api/circuits/#terraflux.circuit.circuitscape_advanced) | Circuitscape's advanced mode on rasters (``scenario = advanced``) |
| [`circuit.advanced`](/python/api/circuits/#terraflux.circuit.advanced.advanced) | Currents and voltages of a raster circuit with sources and grounds |
| [`circuit.AdvancedResult`](/python/api/circuits/#terraflux.circuit.AdvancedResult) | Node currents and voltages of `advanced` |
| [`circuit.circuitscape_network_pairwise`](/python/api/circuits/#terraflux.circuit.circuitscape_network_pairwise) | Circuitscape's pairwise mode on a network (``data_type = network``, ``scenario = pairwise``) |
| [`circuit.circuitscape_network_advanced`](/python/api/circuits/#terraflux.circuit.circuitscape_network_advanced) | Circuitscape's advanced mode on a network (``data_type = network``, ``scenario = advanced``) |
| [`circuit.NetworkResult`](/python/api/circuits/#terraflux.circuit.NetworkResult) | Circuitscape's network outputs, with the node numbers it writes (from 1) |
| [`circuit.read_graph`](/python/api/circuits/#terraflux.circuit.read_graph) | Read a Circuitscape edge list, as its ``load_graph`` |

### Surface metrics

| Name | Summary |
|---|---|
| [`tf.surface.metrics`](/python/api/surface/#terraflux.surface.metrics) | Surface metrics of a raster, as a Series indexed by metric |
| [`tf.surface.list_metrics`](/python/api/surface/#terraflux.surface.list_metrics) | The surface metrics `metrics()` computes |
| [`tf.surface.sa`](/python/api/surface/#terraflux.surface.sa) | Average roughness (Sa): the mean absolute deviation from the mean height |
| [`tf.surface.sq`](/python/api/surface/#terraflux.surface.sq) | Root mean square roughness (Sq): the root mean square of the heights about their mean, sqrt(sum((z - mean(z))^2) / N) (ISO 25178; geodiv uses the sample standard deviation, over N - 1) |
| [`tf.surface.ssk`](/python/api/surface/#terraflux.surface.ssk) | Skewness of the height distribution (Ssk) |
| [`tf.surface.sku`](/python/api/surface/#terraflux.surface.sku) | Kurtosis of the height distribution (Sku) |
| [`tf.surface.sv`](/python/api/surface/#terraflux.surface.sv) | Maximum valley depth (Sv): the absolute value of the lowest height |
| [`tf.surface.sph`](/python/api/surface/#terraflux.surface.sph) | Maximum peak height (Sp): the absolute value of the highest height |
| [`tf.surface.smean`](/python/api/surface/#terraflux.surface.smean) | Mean peak height: the mean of the positive heights |
| [`tf.surface.sdq`](/python/api/surface/#terraflux.surface.sdq) | Root mean square slope (Sdq) |
| [`tf.surface.sdq6`](/python/api/surface/#terraflux.surface.sdq6) | Area root mean square slope (Sdq6), from seven-point derivatives |
| [`tf.surface.sdr`](/python/api/surface/#terraflux.surface.sdr) | Developed interfacial area ratio (Sdr), in percent |
| [`tf.surface.surface_area`](/python/api/surface/#terraflux.surface.surface_area) | Area of the surface, triangulated, in geodiv's units |
| [`tf.surface.flat_area`](/python/api/surface/#terraflux.surface.flat_area) | Area of the surface's projection on a plane (geodiv's ``flatsa``) |
| [`tf.surface.find_peaks`](/python/api/surface/#terraflux.surface.find_peaks) | The peaks of a surface: cells higher than their 8 neighbours and than 0 |
| [`tf.surface.find_valleys`](/python/api/surface/#terraflux.surface.find_valleys) | The valleys of a surface: cells lower than their 8 neighbours and than 0 |
| [`tf.surface.sds`](/python/api/surface/#terraflux.surface.sds) | Density of summits (Sds): peaks per cell |
| [`tf.surface.ssc`](/python/api/surface/#terraflux.surface.ssc) | Mean summit curvature (Ssc) |
| [`tf.surface.s10z`](/python/api/surface/#terraflux.surface.s10z) | Ten-point height (S10z) |
| [`tf.surface.bearing_area`](/python/api/surface/#terraflux.surface.bearing_area) | The bearing area (Abbott-Firestone) curve: the share of the surface at or above each height |
| [`tf.surface.height_ba`](/python/api/surface/#terraflux.surface.height_ba) | Height of the bearing area curve at a bearing fraction |
| [`tf.surface.sdc`](/python/api/surface/#terraflux.surface.sdc) | Areal material ratio height difference (Sdc) |
| [`tf.surface.sbi`](/python/api/surface/#terraflux.surface.sbi) | Surface bearing index (Sbi): Sq divided by the height at 5 % bearing |
| [`tf.surface.svi`](/python/api/surface/#terraflux.surface.svi) | Valley fluid retention index (Svi): the void volume of the valley zone |
| [`tf.surface.sci`](/python/api/surface/#terraflux.surface.sci) | Core fluid retention index (Sci): the void volume of the core zone |
| [`tf.surface.sk`](/python/api/surface/#terraflux.surface.sk) | Core roughness depth (Sk): the height range of the core line |
| [`tf.surface.spk`](/python/api/surface/#terraflux.surface.spk) | Reduced peak height (Spk): the peaks above the core |
| [`tf.surface.svk`](/python/api/surface/#terraflux.surface.svk) | Reduced valley depth (Svk): the valleys below the core |
| [`tf.surface.aacf`](/python/api/surface/#terraflux.surface.aacf) | Areal autocorrelation function of a surface |
| [`tf.surface.scl`](/python/api/surface/#terraflux.surface.scl) | Correlation lengths: how far the autocorrelation takes to decay |
| [`tf.surface.stxr`](/python/api/surface/#terraflux.surface.stxr) | Texture aspect ratio (Str): fastest over slowest decay length |
| [`tf.surface.std`](/python/api/surface/#terraflux.surface.std) | Texture direction (Std) and texture direction index (Stdi) |
| [`tf.surface.srw`](/python/api/surface/#terraflux.surface.srw) | Dominant radial wavelength (Srw), its index (Srwi) and the half-power wavelength (Shw) |
| [`tf.surface.sfd`](/python/api/surface/#terraflux.surface.sfd) | Fractal dimension of the surface (Sfd), by triangular prisms |
| [`tf.surface.window_metrics`](/python/api/surface/#terraflux.surface.window_metrics) | Surface metrics in a moving window: one texture image per metric |
| [`tf.surface.terrain`](/python/api/surface/#terraflux.surface.terrain) | Slope or aspect of a DEM, as terra's ``terrain`` |
| [`tf.surface.tpi`](/python/api/surface/#terraflux.surface.tpi) | Topographic position index: the elevation minus the mean of the window |
| [`tf.surface.tri`](/python/api/surface/#terraflux.surface.tri) | Terrain ruggedness index (Riley et al. 1999) |
| [`tf.surface.vrm`](/python/api/surface/#terraflux.surface.vrm) | Vector ruggedness measure (Sappington et al. 2007) |
| [`tf.surface.srr`](/python/api/surface/#terraflux.surface.srr) | Surface relief ratio (Pike & Wilson 1971): (mean - min) / (max - min) of the window |
| [`tf.surface.dissection`](/python/api/surface/#terraflux.surface.dissection) | Martonne's modified dissection: (z - min) / (max - min) of the window |
| [`tf.surface.curvature`](/python/api/surface/#terraflux.surface.curvature) | Surface curvature of a DEM |
| [`tf.surface.spherical_sd`](/python/api/surface/#terraflux.surface.spherical_sd) | Spherical standard deviation of the slope and aspect (terrain heterogeneity) |
| [`tf.surface.hsp`](/python/api/surface/#terraflux.surface.hsp) | Hierarchical slope position (Murphy et al. 2010) |
| [`tf.surface.trasp`](/python/api/surface/#terraflux.surface.trasp) | Transformed aspect (Roberts & Cooper 1989): (1 - cos(aspect - 30 deg)) / 2 |
| [`tf.surface.hli`](/python/api/surface/#terraflux.surface.hli) | Heat load index (McCune & Keon 2002) |
| [`tf.surface.geomorphons`](/python/api/surface/#terraflux.surface.geomorphons) | Landforms of a DEM by geomorphons (Jasiewicz & Stepinski 2013) |
| [`tf.surface.Geomorphons`](/python/api/surface/#terraflux.surface.Geomorphons) | The geomorphons of a DEM (`geomorphons`) |
| [`tf.surface.hilltop`](/python/api/surface/#terraflux.surface.hilltop) | Hilltops as permanent protection areas (APP de topo de morro) |
| [`tf.surface.Hilltop`](/python/api/surface/#terraflux.surface.Hilltop) | The hilltops of a DEM (`hilltop`) |
| [`tf.surface.viewshed`](/python/api/surface/#terraflux.surface.viewshed) | How many observers see every cell of an elevation map (gdal_viewshed, GRASS's r.viewshed, for many observers) |
| [`tf.surface.gravity`](/python/api/surface/#terraflux.surface.gravity) | Gravitational interaction potential of source cells |
| [`tf.surface.fit_plane`](/python/api/surface/#terraflux.surface.fit_plane) | The least-squares polynomial surface of a given order |
| [`tf.surface.best_fit_plane`](/python/api/surface/#terraflux.surface.best_fit_plane) | The polynomial surface of order 0 to 3 that fits a surface best |
| [`tf.surface.remove_plane`](/python/api/surface/#terraflux.surface.remove_plane) | A surface minus its best-fitting polynomial surface (geodiv's ``remove_plane``) |
| [`tf.surface.example_data`](/python/api/surface/#terraflux.surface.example_data) | Path of an example surface |

### Terrain hydrology

| Name | Summary |
|---|---|
| [`tf.hydro.fill`](/python/api/hydro/#terraflux.hydro.fill) | Fill the depressions of a DEM, so that water can flow from every cell to the edge |
| [`tf.hydro.breach`](/python/api/hydro/#terraflux.hydro.breach) | Remove the depressions of a DEM by cutting channels through their rims |
| [`tf.hydro.flow_direction`](/python/api/hydro/#terraflux.hydro.flow_direction) | The direction each cell drains to |
| [`tf.hydro.flow_accumulation`](/python/api/hydro/#terraflux.hydro.flow_accumulation) | The area that drains through each cell (the cell itself included) |
| [`tf.hydro.streams`](/python/api/hydro/#terraflux.hydro.streams) | The stream cells: those whose flow accumulation is at least ``threshold`` |
| [`tf.hydro.distance_to_channel`](/python/api/hydro/#terraflux.hydro.distance_to_channel) | The mean distance water travels from each cell to a stream, along pygeoprocessing's multiple flow directions (as InVEST's sediment delivery model measures its downslope component) |
| [`tf.hydro.stream_links`](/python/api/hydro/#terraflux.hydro.stream_links) | The links of a stream network: the stretches between sources, junctions and outlets |
| [`tf.hydro.watersheds`](/python/api/hydro/#terraflux.hydro.watersheds) | The area draining to each seed: every cell labelled with the first seed below it |
| [`tf.hydro.snap_outlets`](/python/api/hydro/#terraflux.hydro.snap_outlets) | The stream cell each outlet point stands for |
| [`tf.hydro.snap_points`](/python/api/hydro/#terraflux.hydro.snap_points) | Points moved onto the stream network, as hydrographr's ``snap_to_network`` |
| [`tf.hydro.upstream`](/python/api/hydro/#terraflux.hydro.upstream) | The land upstream of every link of a stream network, or of every point on it, summarised |
| [`tf.hydro.Upstream`](/python/api/hydro/#terraflux.hydro.Upstream) | The land upstream of links or points (`upstream`) |
| [`tf.hydro.upstream_catchment`](/python/api/hydro/#terraflux.hydro.upstream_catchment) | The catchment of every point (a gauge, a dam, a site), as hydrographr's ``get_upstream_catchment`` |
| [`tf.hydro.Catchments`](/python/api/hydro/#terraflux.hydro.Catchments) | The catchments of points (`upstream_catchment`) |
| [`tf.hydro.network_distance`](/python/api/hydro/#terraflux.hydro.network_distance) | The distance along the river network between every two points, as hydrographr's ``get_distance`` |
| [`tf.hydro.along_network`](/python/api/hydro/#terraflux.hydro.along_network) | The stretches of river up and down from every point within a distance along the network |
| [`tf.hydro.NetworkReach`](/python/api/hydro/#terraflux.hydro.NetworkReach) | The stretches of river up and down from points (`along_network`) |
| [`tf.hydro.centrality`](/python/api/hydro/#terraflux.hydro.centrality) | The place of every link in its river network, as hydrographr's ``get_centrality`` on the graph of the links |
| [`tf.hydro.dci`](/python/api/hydro/#terraflux.hydro.dci) | The dendritic connectivity index of a river network cut by barriers (Cote et al. 2009), as R's riverconn |
| [`tf.hydro.DCI`](/python/api/hydro/#terraflux.hydro.DCI) | The dendritic connectivity index of a river network cut by barriers (`dci`) |
| [`tf.hydro.hand`](/python/api/hydro/#terraflux.hydro.hand) | Height Above the Nearest Drainage (HAND; Rennó et al. 2008, Nobre et al. 2011) |
| [`tf.hydro.twi`](/python/api/hydro/#terraflux.hydro.twi) | Topographic wetness index: ln(a / tan(slope)) (Beven & Kirkby 1979) |
| [`tf.hydro.spi`](/python/api/hydro/#terraflux.hydro.spi) | Stream power index: a · tan(slope) (Moore, Grayson & Ladson 1991), the erosive power of overland flow |
| [`tf.hydro.sti`](/python/api/hydro/#terraflux.hydro.sti) | Sediment transport index, the LS factor of the USLE from the contributing area (Moore & Burch 1986): (m + 1) (a / 22.13)^m (sin(slope) / 0.0896)^n |
| [`tf.hydro.connectivity_index`](/python/api/hydro/#terraflux.hydro.connectivity_index) | The index of connectivity of Borselli et al. (2008), as InVEST's SDR model computes it |

### Watershed model (SWAT+)

| Name | Summary |
|---|---|
| [`tf.hydro.swat.load_txtinout`](/python/api/swat/#terraflux.hydro.swat.load_txtinout) | Read a SWAT+ TxtInOut folder, or a zip file of one (the outputs of earlier runs in it are left out) |
| [`tf.hydro.swat.SwatProject`](/python/api/swat/#terraflux.hydro.swat.SwatProject) | The input files of a SWAT+ model (the TxtInOut folder), by name |
| [`tf.hydro.swat.run`](/python/api/swat/#terraflux.hydro.swat.run) | Simulate a SWAT+ project, as SWAT+ 62.0.1 does |
| [`tf.hydro.swat.SwatResult`](/python/api/swat/#terraflux.hydro.swat.SwatResult) | What `run` simulated |
| [`tf.hydro.swat.watershed`](/python/api/swat/#terraflux.hydro.swat.watershed) | Divide a watershed into subbasins, channels and HRUs, as QSWAT+ does |
| [`tf.hydro.swat.SwatWatershed`](/python/api/swat/#terraflux.hydro.swat.SwatWatershed) | A watershed divided as QSWAT+ divides it, ready for the SWAT+ Editor's import |
| [`tf.hydro.swat.from_tables`](/python/api/swat/#terraflux.hydro.swat.from_tables) | A SWAT+ project from QSWAT+'s tables and the weather, as the SWAT+ Editor makes it |
| [`tf.hydro.swat.from_rasters`](/python/api/swat/#terraflux.hydro.swat.from_rasters) | A SWAT+ project from a DEM, land use and soil maps, tables and the weather |
| [`tf.hydro.swat.load_qswat_project`](/python/api/swat/#terraflux.hydro.swat.load_qswat_project) | The tables of a QSWAT+ project database, for `from_tables()` |
| [`tf.hydro.swat.example_landscape`](/python/api/swat/#terraflux.hydro.swat.example_landscape) | The inputs of a small watershed model, from the package's example rasters |
| [`tf.hydro.swat.climate`](/python/api/swat/#terraflux.hydro.swat.climate) | The weather of a SWAT+ project from tables |
| [`tf.hydro.swat.SwatClimate`](/python/api/swat/#terraflux.hydro.swat.SwatClimate) | The weather of a SWAT+ project (made by `climate`) |
| [`tf.hydro.swat.wgn_from_series`](/python/api/swat/#terraflux.hydro.swat.wgn_from_series) | SWAT's weather generator statistics (WXGEN) of each station, from daily series |
| [`tf.hydro.swat.hru_map`](/python/api/swat/#terraflux.hydro.swat.hru_map) | An HRU result on the map: each cell takes the value of its HRU |
| [`tf.hydro.swat.channel_map`](/python/api/swat/#terraflux.hydro.swat.channel_map) | A channel result on the stream cells (by default the mean discharge, m3/s) |
| [`tf.hydro.swat.reach_discharge`](/python/api/swat/#terraflux.hydro.swat.reach_discharge) | The daily discharge leaving each channel (m3/s, ``channel_sd``'s ``flo_out``): dates x channel number. ``result``: a run that printed ``channel_sd`` by day |

### Grid distances

| Name | Summary |
|---|---|
| [`tf.cost.transition`](/python/api/cost/#terraflux.cost.transition) | Transition matrix from a raster: gdistance's ``transition()`` |
| [`tf.cost.Transition`](/python/api/cost/#terraflux.cost.Transition) | A transition matrix on a raster grid: gdistance's ``TransitionLayer`` |
| [`tf.cost.geo_correction`](/python/api/cost/#terraflux.cost.geo_correction) | Correct a transition for the distances between cell centres: gdistance's ``geoCorrection()`` |
| [`tf.cost.normalize`](/python/api/cost/#terraflux.cost.normalize) | Normalise a transition matrix: gdistance's ``normalize()`` |
| [`tf.cost.sum_reciprocal`](/python/api/cost/#terraflux.cost.sum_reciprocal) | Combine two transitions as resistances in series: gdistance's ``sumReciprocal()`` |
| [`tf.cost.acc_cost`](/python/api/cost/#terraflux.cost.acc_cost) | Accumulated cost surface from the nearest origin: gdistance's ``accCost()`` |
| [`tf.cost.cost_distance`](/python/api/cost/#terraflux.cost.cost_distance) | Least-cost distances between points: gdistance's ``costDistance()`` |
| [`tf.cost.shortest_path`](/python/api/cost/#terraflux.cost.shortest_path) | Least-cost path from an origin to one or more goals: gdistance's ``shortestPath()`` |
| [`tf.cost.commute_distance`](/python/api/cost/#terraflux.cost.commute_distance) | Expected commute distances of a random walk between points: gdistance's ``commuteDistance()`` |
| [`tf.cost.passage`](/python/api/cost/#terraflux.cost.passage) | Passages of a random walk from origins to goals: gdistance's ``passage()`` |
| [`tf.cost.rsp_distance`](/python/api/cost/#terraflux.cost.rsp_distance) | Randomised-shortest-path distances between points: gdistance's ``rSPDistance()`` |
| [`tf.cost.path_inc`](/python/api/cost/#terraflux.cost.path_inc) | Overlap and non-overlap of the routes from a common origin to each location: gdistance's ``pathInc()`` |
| [`tf.cost.overlap`](/python/api/cost/#terraflux.cost.overlap) | gdistance's ``overlap``: the flow two routes share on each connection, ``min(a, b)`` |
| [`tf.cost.nonoverlap`](/python/api/cost/#terraflux.cost.nonoverlap) | gdistance's ``nonoverlap``: ``max(max(a, b) * (1 - min(a, b)) - min(a, b), 0)`` |
| [`tf.cost.barriers`](/python/api/cost/#terraflux.cost.barriers) | Barrier transitions: gdistance's ``transition(x, "barriers", directions)`` |
| [`tf.cost.areas`](/python/api/cost/#terraflux.cost.areas) | Area transitions: gdistance's ``transition(x, "areas", directions)`` |
| [`tf.cost.example_data`](/python/api/cost/#terraflux.cost.example_data) | Path of an example data set for the grid distances |

### Distance & restoration

| Name | Summary |
|---|---|
| [`tf.euclidean_distance`](/python/api/distance/#terraflux.euclidean_distance) | Euclidean distance to the habitat edge, inside and outside the habitat (GTB's ``batch_eucldist``) |
| [`tf.EuclideanDistanceResult`](/python/api/distance/#terraflux.EuclideanDistanceResult) | Result of `euclidean_distance` |
| [`tf.distance_to_resistance`](/python/api/distance/#terraflux.distance_to_resistance) | Resistance growing with the distance to the foreground (GTB Setup Tools -> Distance -> Resistance) |
| [`tf.restoration_status`](/python/api/distance/#terraflux.restoration_status) | How connected the habitat is and how much reconnecting it could add (GTB's Restoration Status, ``batch_rss``) |
| [`tf.RestorationStatusResult`](/python/api/distance/#terraflux.RestorationStatusResult) | Result of `restoration_status` |
| [`tf.cost_distance`](/python/api/distance/#terraflux.cost_distance) | Accumulated cost of travelling from the source cells across a resistance surface (8 neighbours) |
| [`tf.least_cost_path`](/python/api/distance/#terraflux.least_cost_path) | The least-cost path between two areas A and B |
| [`tf.LeastCostPathResult`](/python/api/distance/#terraflux.LeastCostPathResult) | Result of `least_cost_path` |
| [`tf.restoration_corridor`](/python/api/distance/#terraflux.restoration_corridor) | Connectivity gain of restoring the least-cost corridor between two areas (GTB's Restoration Planner) |
| [`tf.RestorationCorridorResult`](/python/api/distance/#terraflux.RestorationCorridorResult) | Result of `restoration_corridor` |
| [`tf.isochrone_map`](/python/api/distance/#terraflux.isochrone_map) | Accumulated cost from object A (GTB Restoration Planner -> Isochrone Map A) |
| [`tf.isochrone_map_ab`](/python/api/distance/#terraflux.isochrone_map_ab) | Cheapest route between objects A and B (GTB Restoration Planner -> Isochrone Map AB) |
| [`tf.IsochroneResult`](/python/api/distance/#terraflux.IsochroneResult) | Result of `isochrone_map` / `isochrone_map_ab` |
| [`tf.influence_zones`](/python/api/distance/#terraflux.influence_zones) | Zones of influence of the foreground objects (GTB Distance -> Influence Zones) |
| [`tf.InfluenceZonesResult`](/python/api/distance/#terraflux.InfluenceZonesResult) | Result of `influence_zones` |
| [`tf.proximity`](/python/api/distance/#terraflux.proximity) | Proximity between neighbouring objects (GTB Distance -> Proximity) |
| [`tf.ProximityResult`](/python/api/distance/#terraflux.ProximityResult) | Result of `proximity` |
| [`tf.original_vegetation`](/python/api/distance/#terraflux.original_vegetation) | The original vegetation of converted land, and what restoration brings back |
| [`tf.OriginalVegetation`](/python/api/distance/#terraflux.OriginalVegetation) | Result of `original_vegetation` |
| [`tf.intactness`](/python/api/distance/#terraflux.intactness) | Ecological intactness of every cell from the human footprint (Beyer et al. 2020) |
| [`tf.intactness_gradient`](/python/api/distance/#terraflux.intactness_gradient) | What restoring each cell adds to the total ecological intactness |

### Change detection

| Name | Summary |
|---|---|
| [`tf.simple_change`](/python/api/change/#terraflux.simple_change) | Pixel-by-pixel change of a habitat map between two dates |
| [`tf.SimpleChangeResult`](/python/api/change/#terraflux.SimpleChangeResult) | Result of `simple_change` |
| [`tf.morphological_change`](/python/api/change/#terraflux.morphological_change) | Change of a habitat map that separates real structural change from edge noise (GTB's Morphological Change Detection) |
| [`tf.MorphologicalChangeResult`](/python/api/change/#terraflux.MorphologicalChangeResult) | Result of `morphological_change` |
| [`tf.fos_change`](/python/api/change/#terraflux.fos_change) | Change of fragmentation between two dates (GTB's FOS change) |
| [`tf.FOSChangeResult`](/python/api/change/#terraflux.FOSChangeResult) | Result of `fos_change` |
| [`tf.fad_multiscale_change`](/python/api/change/#terraflux.fad_multiscale_change) | Fragmentation class change between two FAD-MS analyses (GTB Change -> FAD Multiscale) |
| [`tf.FADMultiscaleChangeResult`](/python/api/change/#terraflux.FADMultiscaleChangeResult) | Result of `fad_multiscale_change` |
| [`tf.FADChangeScale`](/python/api/change/#terraflux.FADChangeScale) | FAD change A -> B at one observation scale (one GTB ``FAD_change_<scale>`` table) |
| [`tf.landscape_mosaic_change`](/python/api/change/#terraflux.landscape_mosaic_change) | Change of the Landscape Mosaic heatmap from A to B (GTB Change -> Landscape Mosaic Heatmap) |
| [`tf.LMHeatmapChange`](/python/api/change/#terraflux.LMHeatmapChange) | Result of `landscape_mosaic_change`: the heatmap difference B - A (GTB's ``delta_heatmap.csv``) |

### Large data & batch

| Name | Summary |
|---|---|
| [`tf.batch`](/python/api/batch/#terraflux.batch) | Run a terraflux tool on many rasters in parallel and write its outputs |
| [`tf.mspa_batch`](/python/api/batch/#terraflux.mspa_batch) | Run MSPA on many rasters in parallel and write the results |
| [`tf.BatchResult`](/python/api/batch/#terraflux.BatchResult) | Outcome of one input of `batch` / `mspa_batch` |
| [`tf.batch_status`](/python/api/batch/#terraflux.batch_status) | What the runs of `batch` / `mspa_batch` did in an output folder: the inputs done, failed (with their error) and not done, and the job array tasks to submit again |
| [`tf.batch.Stopped`](/python/api/batch/#terraflux.batch.Stopped) | A batch stopped by SIGTERM (a scheduler's warning before the job's time limit, or its cancellation): no input started after it; ``results`` holds those that ended, in input order (``None`` for the inputs not processed) |
| [`tf.parallel.helpers`](/python/api/batch/#terraflux.parallel.helpers) | This computer and the helpers of its group, for ``on=`` (see `Helpers`) |
| [`tf.parallel.Helpers`](/python/api/batch/#terraflux.parallel.Helpers) | This computer and the helpers of its group, for the ``on=`` argument of terraflux's functions that run many independent tasks |
| [`tf.parallel.map`](/python/api/batch/#terraflux.parallel.map) | Run a tool on many inputs, on this computer or on helpers, in parallel |
| [`tf.parallel.tiled`](/python/api/batch/#terraflux.parallel.tiled) | Run a neighbourhood tool on a large raster tile by tile, on this computer or on helpers |
| [`tf.parallel.call`](/python/api/batch/#terraflux.parallel.call) | A registered tool with its arguments, the drawn ones marked by `param()` |
| [`tf.parallel.param`](/python/api/batch/#terraflux.parallel.param) | The ``index``-th drawn parameter, in the arguments of a `call()` |
| [`tf.parallel.run_worker`](/python/api/batch/#terraflux.parallel.run_worker) | Lend this computer to the group: wait for tasks and run them, until Ctrl+C |
| [`tf.parallel.invite_helpers`](/python/api/batch/#terraflux.parallel.invite_helpers) | Show an invitation code and let other computers join this one's group with ``terraflux-py worker --join <code>``, for ``minutes`` |
| [`tf.parallel.list_helpers`](/python/api/batch/#terraflux.parallel.list_helpers) | The helpers of this computer's group waiting for work now (busy ones are not looking): ``[{"name", "address", "cores", "version"}]`` |
| [`tf.parallel.leave_group`](/python/api/batch/#terraflux.parallel.leave_group) | Forget this computer's group (it no longer helps nor uses helpers) |
| [`tf.parallel.open_firewall`](/python/api/batch/#terraflux.parallel.open_firewall) | Let helpers reach this computer: the Windows firewall rules for the local network |
| [`tf.mspa_tiled`](/python/api/batch/#terraflux.mspa_tiled) | MSPA of a large map by overlapping tiles, identical to GTB's MSPA Tiling |
| [`tf.tile_layout`](/python/api/batch/#terraflux.tile_layout) | GTB's tiles along one axis of ``size`` pixels |

### Neutral landscape models

| Name | Summary |
|---|---|
| [`tf.nlm.random`](/python/api/nlm/#terraflux.nlm.random) | Spatially random neutral landscape model (NLMR's ``nlm_random``) |
| [`tf.nlm.percolation`](/python/api/nlm/#terraflux.nlm.percolation) | Percolation neutral landscape model (NLMR's ``nlm_percolation``) |
| [`tf.nlm.planar_gradient`](/python/api/nlm/#terraflux.nlm.planar_gradient) | Planar gradient neutral landscape model (NLMR's ``nlm_planargradient``) |
| [`tf.nlm.edge_gradient`](/python/api/nlm/#terraflux.nlm.edge_gradient) | Edge gradient neutral landscape model (NLMR's ``nlm_edgegradient``) |
| [`tf.nlm.distance_gradient`](/python/api/nlm/#terraflux.nlm.distance_gradient) | Distance gradient neutral landscape model (NLMR's ``nlm_distancegradient``) |
| [`tf.nlm.mpd`](/python/api/nlm/#terraflux.nlm.mpd) | Midpoint displacement neutral landscape model (NLMR's ``nlm_mpd``) |
| [`tf.nlm.fbm`](/python/api/nlm/#terraflux.nlm.fbm) | Fractional Brownian motion neutral landscape model (NLMR's ``nlm_fbm``) |
| [`tf.nlm.gaussian_field`](/python/api/nlm/#terraflux.nlm.gaussian_field) | Gaussian random field neutral landscape model (NLMR's ``nlm_gaussianfield``) |
| [`tf.nlm.mosaic_field`](/python/api/nlm/#terraflux.nlm.mosaic_field) | Mosaic random field neutral landscape model (NLMR's ``nlm_mosaicfield``, Schlather 1999) |
| [`tf.nlm.mosaic_tess`](/python/api/nlm/#terraflux.nlm.mosaic_tess) | Polygonal (Voronoi) mosaic neutral landscape model (NLMR's ``nlm_mosaictess``, Gaucherel 2008) |
| [`tf.nlm.mosaic_gibbs`](/python/api/nlm/#terraflux.nlm.mosaic_gibbs) | Gibbs-process Voronoi mosaic (NLMR's ``nlm_mosaicgibbs``, Gauchel et al. 2008) |
| [`tf.nlm.random_cluster`](/python/api/nlm/#terraflux.nlm.random_cluster) | Random cluster neutral landscape model (NLMR's ``nlm_randomcluster``, Saura & Martínez-Millán 2000) |
| [`tf.nlm.neigh`](/python/api/nlm/#terraflux.nlm.neigh) | Neighbourhood-based clusters (NLMR's ``nlm_neigh``, Scherer et al. 2016) |
| [`tf.nlm.random_rectangular_cluster`](/python/api/nlm/#terraflux.nlm.random_rectangular_cluster) | Random rectangular clusters (NLMR's ``nlm_randomrectangularcluster``, Gaucherel 2008) |
| [`tf.nlm.curds`](/python/api/nlm/#terraflux.nlm.curds) | Hierarchical curdling (NLMR's ``nlm_curds``, Keitt 2000) |
| [`tf.nlm.classify`](/python/api/nlm/#terraflux.nlm.classify) | Classify a continuous landscape into categories (NLMR's ``landscape_classify``) |
| [`tf.nlm.binarize`](/python/api/nlm/#terraflux.nlm.binarize) | Habitat / matrix maps at given habitat shares (landscapetools' ``util_binarize``) |
| [`tf.nlm.rescale`](/python/api/nlm/#terraflux.nlm.rescale) | Linearly rescale values to 0-1 (NLMR's ``util_rescale``) |
| [`tf.nlm.merge`](/python/api/nlm/#terraflux.nlm.merge) | Merge neutral landscapes (landscapetools' ``util_merge``) |

### Biodiversity analysis

| Name | Summary |
|---|---|
| [`tf.bio.BaseData`](/python/api/biodiversity/#terraflux.bio.BaseData) | Groups × labels sample counts with the group coordinates |
| [`tf.bio.snap_coordinates`](/python/api/biodiversity/#terraflux.bio.snap_coordinates) | Cell centres as Biodiverse's text import computes them (before naming) |
| [`tf.bio.example_data`](/python/api/biodiversity/#terraflux.bio.example_data) | Path of Biodiverse's example data (GPL-3, Laffan et al.) |
| [`tf.bio.read_occurrences`](/python/api/biodiversity/#terraflux.bio.read_occurrences) | Occurrence records of a GBIF download or a Darwin Core Archive, cleaned |
| [`tf.bio.neighbours`](/python/api/biodiversity/#terraflux.bio.neighbours) | Neighbour sets of every group, as Biodiverse's spatial analysis finds them |
| [`tf.bio.definition_query`](/python/api/biodiversity/#terraflux.bio.definition_query) | Boolean mask of the groups passing a definition query |
| [`tf.bio.SpatialCondition`](/python/api/biodiversity/#terraflux.bio.SpatialCondition) | A Biodiverse spatial condition (Perl syntax) compiled for a BaseData |
| [`tf.bio.Neighbourhoods`](/python/api/biodiversity/#terraflux.bio.Neighbourhoods) | Neighbour sets of the processing groups |
| [`tf.bio.spatial_analysis`](/python/api/biodiversity/#terraflux.bio.spatial_analysis) | Biodiverse's spatial analysis: indices over the neighbourhood of every group |
| [`tf.bio.list_calculations`](/python/api/biodiversity/#terraflux.bio.list_calculations) | The calculations available to `spatial_analysis()` |
| [`tf.bio.SpatialResult`](/python/api/biodiversity/#terraflux.bio.SpatialResult) | Results of `spatial_analysis` |
| [`tf.bio.randomise`](/python/api/biodiversity/#terraflux.bio.randomise) | Randomise the basedata of a spatial analysis and compare the results |
| [`tf.bio.Randomisation`](/python/api/biodiversity/#terraflux.bio.Randomisation) | Randomisations of a `SpatialResult` (see `randomise`) |
| [`tf.bio.cluster`](/python/api/biodiversity/#terraflux.bio.cluster) | Cluster the groups by the dissimilarity of their labels (Biodiverse's cluster analysis) |
| [`tf.bio.region_grower`](/python/api/biodiversity/#terraflux.bio.region_grower) | Grow regions by joining the pairs with the highest value of an index (Biodiverse's RegionGrower) |
| [`tf.bio.ClusterResult`](/python/api/biodiversity/#terraflux.bio.ClusterResult) | Results of `cluster` |
| [`tf.bio.Tree`](/python/api/biodiversity/#terraflux.bio.Tree) | A rooted phylogenetic tree |
| [`tf.bio.read_tree`](/python/api/biodiversity/#terraflux.bio.read_tree) | Read one tree (the first, or the one with index/name ``index``) |
| [`tf.bio.read_trees`](/python/api/biodiversity/#terraflux.bio.read_trees) | Read every tree in a Newick, Phylip (several Newicks) or Nexus file |
| [`tf.bio.Matrix`](/python/api/biodiversity/#terraflux.bio.Matrix) | A symmetric matrix of values between elements (usually labels) |
| [`tf.bio.read_matrix`](/python/api/biodiversity/#terraflux.bio.read_matrix) | Read a Biodiverse matrix file (full or lower/upper triangle, or sparse pairs) |
| [`tf.bio.hex_centres`](/python/api/biodiversity/#terraflux.bio.hex_centres) | Centres of the hexagons (circumradius ``size``) containing each point |
| [`tf.bio.h3_cells`](/python/api/biodiversity/#terraflux.bio.h3_cells) | H3 cell of each point (``h3`` package) |
| [`tf.bio.save_session`](/python/api/biodiversity/#terraflux.bio.save_session) | Save basedatas, trees, matrices and analysis results in one file |
| [`tf.bio.load_session`](/python/api/biodiversity/#terraflux.bio.load_session) | Load the objects saved with `save_session()` (a dict by name) |
| [`tf.bio.read_properties`](/python/api/biodiversity/#terraflux.bio.read_properties) | Read a Biodiverse element properties file (label or group properties) |

### Spatial prioritization

| Name | Summary |
|---|---|
| [`tf.prioritize.zonation`](/python/api/prioritization/#terraflux.prioritize.zonation) | Rank the cells of a landscape for conservation, as Zonation 5 does |
| [`tf.prioritize.ZonationResult`](/python/api/prioritization/#terraflux.prioritize.ZonationResult) | The result of `zonation` |
| [`tf.prioritize.run_project`](/python/api/prioritization/#terraflux.prioritize.run_project) | Run a Zonation 5 project (see `read_project()`) |
| [`tf.prioritize.read_project`](/python/api/prioritization/#terraflux.prioritize.read_project) | The `zonation()` arguments of a Zonation 5 project |
| [`tf.prioritize.Kernel`](/python/api/prioritization/#terraflux.prioritize.Kernel) | A dispersal kernel of Zonation 5's connectivity transforms |
| [`tf.prioritize.Interaction`](/python/api/prioritization/#terraflux.prioritize.Interaction) | An interaction connectivity transform (Zonation's ``-i``/``-I``): a feature weighted by the connectivity of a resource (``positive``) or reduced by it |
| [`tf.prioritize.convolve`](/python/api/prioritization/#terraflux.prioritize.convolve) | Convolve a raster with a kernel normalized to sum 1, as Zonation 5 does |
| [`tf.prioritize.AreaStatistics`](/python/api/prioritization/#terraflux.prioritize.AreaStatistics) | Statistics of areas of a ranking (`area_statistics`) |
| [`tf.prioritize.ImpactScenario`](/python/api/prioritization/#terraflux.prioritize.ImpactScenario) | An impact assessment scenario (Zonation's ``-s``): land-use change at sources |
| [`tf.prioritize.ImpactSource`](/python/api/prioritization/#terraflux.prioritize.ImpactSource) | An impact source of an impact group (a line of Zonation's impact buffers file) |
| [`tf.prioritize.CumulativeScenario`](/python/api/prioritization/#terraflux.prioritize.CumulativeScenario) | A cumulative impact assessment scenario (Zonation's ``-e``) |
| [`tf.prioritize.Pressure`](/python/api/prioritization/#terraflux.prioritize.Pressure) | A pressure raster of a cumulative impact scenario |
| [`tf.prioritize.problem`](/python/api/prioritization/#terraflux.prioritize.problem) | A conservation planning problem (prioritizr's ``problem()``) |
| [`tf.prioritize.Problem`](/python/api/prioritization/#terraflux.prioritize.Problem) | A conservation planning problem (prioritizr's ``problem()``): see `problem` |
| [`tf.prioritize.Solution`](/python/api/prioritization/#terraflux.prioritize.Solution) | A solution of a `Problem` |
| [`tf.prioritize.Portfolio`](/python/api/prioritization/#terraflux.prioritize.Portfolio) | Several solutions of a problem (`add_cuts_portfolio` and the others) |
| [`tf.prioritize.OptimizationProblem`](/python/api/prioritization/#terraflux.prioritize.OptimizationProblem) | A compiled model: ``modelsense`` of ``obj @ x`` subject to ``A x (sense) rhs``, ``lb <= x <= ub`` and ``x`` of type ``vtype`` (``"B"`` binary, ``"C"`` continuous, ``"I"`` integer); the layout of prioritizr's ``compile()`` |
| [`tf.prioritize.h3_units`](/python/api/prioritization/#terraflux.prioritize.h3_units) | H3 hexagons as planning units: the hexagons of an H3 ``resolution`` that cover a cost raster, with their cost |
| [`tf.prioritize.example_data`](/python/api/prioritization/#terraflux.prioritize.example_data) | Path of an example data set for prioritization: **Serra Verde**, an imaginary coastal region of 60 x 90 cells of 1 km (SIRGAS 2000 / UTM 23S) |
| [`tf.prioritize.category_layer`](/python/api/prioritization/#terraflux.prioritize.category_layer) | The zone of every cell of a stack of zone layers (prioritizr's ``category_layer()``): the number (1, 2, ...) of the layer with the largest value above 0, 0 where none is, NaN where every layer is NaN. ``stack``: a raster with a layer per zone (a solution's ``selection``) or a mapping zone -> raster |
| [`tf.prioritize.binary_stack`](/python/api/prioritization/#terraflux.prioritize.binary_stack) | One 0 / 1 raster per zone from a raster of zone numbers (prioritizr's ``binary_stack()``): a mapping zone -> raster, ready for `add_locked_in_constraints()` (for example an existing zoning plan). ``zones``: the zones' names in the order of their numbers (default: those of a solution's ``zones`` map, else ``"1"``, ``"2"``, ...); cells with 0 or NaN are in no zone |
| [`tf.prioritize.boundary_matrix`](/python/api/prioritization/#terraflux.prioritize.boundary_matrix) | The length of the boundary every two units share, and every unit's perimeter on the diagonal (prioritizr's ``boundary_matrix()``): raster cells with data share ``xres`` with the cells above and below and ``yres`` with those left and right, perimeter ``2 (xres + yres)``; polygons share the length of their common paths, perimeters their own, rounded to 8 decimals |
| [`tf.prioritize.adjacency_matrix`](/python/api/prioritization/#terraflux.prioritize.adjacency_matrix) | 1 for every two adjacent units (prioritizr's ``adjacency_matrix()``): raster cells with data that share an edge (``directions`` 4) or also a corner (8); polygons that intersect, touching at a corner included |
| [`tf.prioritize.connectivity_matrix`](/python/api/prioritization/#terraflux.prioritize.connectivity_matrix) | Connectivity between adjacent units (prioritizr's ``connectivity_matrix()``): the length of their shared boundary times the mean of ``y`` in the two (``y``: a raster on the units' grid; with polygons a raster summed over each, or a column name) |
| [`tf.prioritize.proximity_matrix`](/python/api/prioritization/#terraflux.prioritize.proximity_matrix) | 1 for every two units within ``distance`` of each other (prioritizr's ``proximity_matrix()``): raster cell centres, or polygons by their nearest points |
| [`tf.prioritize.decay_connectivity`](/python/api/prioritization/#terraflux.prioritize.decay_connectivity) | Connectivity from distances between units as a dispersal probability (Conefor's negative exponential): ``exp(-ln(2) d / median)``, 0.5 at the ``median`` dispersal distance, dropped below ``min_probability`` (sparse), 0 on the diagonal and for unreachable pairs (``inf``, NaN) |
| [`tf.prioritize.unit_points`](/python/api/prioritization/#terraflux.prioritize.unit_points) | A point per unit, (n, 2) coordinates: raster cell centres (every cell), or a point inside every polygon (shapely's point on surface), to measure distances between units with `cost_distance` or `commute_distance` |
| [`tf.prioritize.eval_n_summary`](/python/api/prioritization/#terraflux.prioritize.eval_n_summary) | The number of selected planning units (the sum of the solution's values) |
| [`tf.prioritize.eval_cost_summary`](/python/api/prioritization/#terraflux.prioritize.eval_cost_summary) | The solution's total cost |
| [`tf.prioritize.eval_boundary_summary`](/python/api/prioritization/#terraflux.prioritize.eval_boundary_summary) | The solution's boundary length: the edges between selected and other units, plus ``edge_factor`` x the selected units' edges on the outside of the planning units |
| [`tf.prioritize.eval_connectivity_summary`](/python/api/prioritization/#terraflux.prioritize.eval_connectivity_summary) | The connectivity among the selected units: the sum of ``data[i, j]`` x the solution's values of units i and j (a symmetric matrix over the units) |
| [`tf.prioritize.eval_asym_connectivity_summary`](/python/api/prioritization/#terraflux.prioritize.eval_asym_connectivity_summary) | As `eval_connectivity_summary()` for an asymmetric matrix (its diagonal counted once) |
| [`tf.prioritize.eval_feature_representation_summary`](/python/api/prioritization/#terraflux.prioritize.eval_feature_representation_summary) | Every feature's total amount and the amount the solution holds, absolute and relative |
| [`tf.prioritize.eval_target_coverage_summary`](/python/api/prioritization/#terraflux.prioritize.eval_target_coverage_summary) | Every target's feature, whether it is met, the feature's total, and the target, the amount held and the shortfall, absolute and relative (shares of the total; 0 when the total is 0) |
| [`tf.prioritize.eval_replacement_importance`](/python/api/prioritization/#terraflux.prioritize.eval_replacement_importance) | The replacement cost of every selected unit (Cabeza & Moilanen 2006): how much the objective worsens when the unit is left out and the problem solved again with the other selected units kept (infinite when nothing can replace it), rescaled to 0.01-1 |
| [`tf.prioritize.eval_ferrier_importance`](/python/api/prioritization/#terraflux.prioritize.eval_ferrier_importance) | Ferrier et al. (2000)'s irreplaceability of the selected units for every feature and in total (prioritizr's version: the problem needs one ">=" target per feature, amounts >= 0) |
| [`tf.prioritize.eval_rare_richness_importance`](/python/api/prioritization/#terraflux.prioritize.eval_rare_richness_importance) | The rarity-weighted richness of the selected units (Albuquerque & Beier 2015): the sum over the features of the unit's amount / the feature's largest amount / its total, rescaled to 0.01-1 |
| [`tf.prioritize.eval_rank_importance`](/python/api/prioritization/#terraflux.prioritize.eval_rank_importance) | Rank importance (Jung et al. 2021, prioritizr's eval_rank_importance): the problem is solved with increasing budgets (the min set objective becomes min shortfall), only the solution's units allowed and each step keeping the units of the step before; a unit's rank is how early it enters (n = the first budget, ..., 1 = the last), rescaled to 0.01-1. ``n`` budgets evenly spaced between the locked-in units' cost and the solution's, or the ``budgets`` themselves |
| [`tf.prioritize.eval_extinction_risk`](/python/api/prioritization/#terraflux.prioritize.eval_extinction_risk) | Every feature's extinction risk by the species-area relationship, 1 - (held / total)^z (z = 0.25 by default, one value or one per feature), and their sum (the expected number of features lost) |
| [`tf.prioritize.eval_landscape_metrics`](/python/api/prioritization/#terraflux.prioritize.eval_landscape_metrics) | Landscape metrics of a raster solution (`landscape_metrics` of the map of the selected cells: class 1 selected, 0 not, outside the planning units no data): e.g. the number, sizes and shapes of the selected patches |
| [`tf.prioritize.eval_mspa`](/python/api/prioritization/#terraflux.prioritize.eval_mspa) | MSPA (`mspa`) of a raster solution: the selected cells as foreground, the other planning units as background, the cells outside them missing |
| [`tf.prioritize.eval_landscape_identification`](/python/api/prioritization/#terraflux.prioritize.eval_landscape_identification) | Landscape identification: the separate areas of the top fraction of a priority surface, and what each holds (Zonation's area statistics of separate areas, over any surface) |
| [`tf.prioritize.eval_landscape_comparison`](/python/api/prioritization/#terraflux.prioritize.eval_landscape_comparison) | Landscape comparison: how much the top fractions of two priority surfaces overlap (Zonation's solution comparison at several fractions) |
| [`tf.prioritize.position_rank`](/python/api/prioritization/#terraflux.prioritize.position_rank) | Zonation's ranks of a surface by position: the i-th lowest of the N finite cells gets float32(i * (1 / N)); ties go to the earlier cell in row order first; NaN stays NaN |
| [`tf.prioritize.LandscapeAreas`](/python/api/prioritization/#terraflux.prioritize.LandscapeAreas) | The areas of the top fraction of a priority surface (`eval_landscape_identification`) |
| [`tf.prioritize.LandscapeComparison`](/python/api/prioritization/#terraflux.prioritize.LandscapeComparison) | Two priority surfaces compared at several top fractions (`eval_landscape_comparison`) |
| [`tf.prioritize.eval_proxirr`](/python/api/prioritization/#terraflux.prioritize.eval_proxirr) | ProxIRR (Baisero et al. 2022) of every planning unit for the problem's targets, as the CRAN package proxirr computes it |
| [`tf.prioritize.eval_proxirr_marxan`](/python/api/prioritization/#terraflux.prioritize.eval_proxirr_marxan) | ProxIRR of a Marxan problem's planning units (proxirr's ``marxan_alphas``, ``marxan_betas`` and ``marxan_gammas``), from the tables `marxan()` takes |
| [`tf.prioritize.ProxIRR`](/python/api/prioritization/#terraflux.prioritize.ProxIRR) | ProxIRR of every planning unit (`eval_proxirr`, `eval_proxirr_marxan`) |
| [`tf.prioritize.calibrate_penalty`](/python/api/prioritization/#terraflux.prioritize.calibrate_penalty) | The blended approach: the problem solved with a range of penalty weights (``values``; default ``n`` values evenly spaced in log scale over 1/100 to 100 times the Cohon et al. penalty), every candidate's cost and criterion (boundary length, connectivity, ...), and the candidates TOPSIS and the knee of the curve choose. ``p``: the problem without the penalty |
| [`tf.prioritize.calibrate_budget`](/python/api/prioritization/#terraflux.prioritize.calibrate_budget) | The hierarchical approach: the least cost solution first (``p``, a min set problem without the penalty, solved to optimality), then the least penalty within cost thresholds above its cost (``thresholds``, or the cost x (1 + ``inflation``); default inflation 1e-5 to 4 in 9 steps, the thresholds rounded up as in prioritizr's tutorial) |
| [`tf.prioritize.cohon_penalty`](/python/api/prioritization/#terraflux.prioritize.cohon_penalty) | The penalty of Cohon et al. (1979): the trade-off rate between the solution with the least cost (``p`` as given, a min set problem without the penalty) and the one with the least penalty (min penalties within a budget above every unit's cost): \|cost difference / criterion difference\|, rounded to 5 decimals |
| [`tf.prioritize.pareto_front`](/python/api/prioritization/#terraflux.prioritize.pareto_front) | The trade-off between cost and representation: the least total shortfall of the targets (min shortfall objective: the sum of the shortfalls as shares of their targets) within ``n`` budgets evenly spaced up to the least-cost solution's cost (above the cost of the locked-in units, which every budget includes), or the ``budgets`` given. ``p``: a problem with targets (its objective is replaced) |
| [`tf.prioritize.calibrate`](/python/api/prioritization/#terraflux.prioritize.calibrate) | Any trade-off parameter: ``make(value)`` gives the problem for every value (a feature's weight, a target, a penalty of any kind...), ``criterion(problem, solution)`` the quantity traded off against the cost (smaller is better) |
| [`tf.prioritize.TradeOffs`](/python/api/prioritization/#terraflux.prioritize.TradeOffs) | Candidate solutions over a trade-off parameter |
| [`tf.prioritize.topsis`](/python/api/prioritization/#terraflux.prioritize.topsis) | TOPSIS (as R's ``topsis`` package): the alternatives (rows of ``decision``) scored by their closeness to the ideal (``impacts``: "+" larger is better, "-" smaller is better, per column; equal ``weights`` by default) |
| [`tf.prioritize.knee_point`](/python/api/prioritization/#terraflux.prioritize.knee_point) | The knee (elbow) of a trade-off curve: the point farthest from the straight line between its ends, both axes scaled to 0-1 (the points in the order of x) |
| [`tf.prioritize.mean_sd`](/python/api/prioritization/#terraflux.prioritize.mean_sd) | plangea's choice of weights (``post_choose_weights``): every criterion (a column of ``decision``) rescaled to 0-1 over the alternatives (a constant one to 1), the "-" ones (smaller is better) inverted, and every alternative scored by the mean of its criteria divided by their standard deviation (good on all of them at once) |
| [`tf.prioritize.iterative_solve`](/python/api/prioritization/#terraflux.prioritize.iterative_solve) | Solve, change the problem, solve again: ``update(step, solution, problem)`` gives the next problem (for instance from the landscape a ``tf.simulate`` step leaves, with the solution's cells protected by `protected_cells()` in the simulation's rules), or ``None`` to stop |
| [`tf.prioritize.protected_cells`](/python/api/prioritization/#terraflux.prioritize.protected_cells) | The selected cells of a raster solution as a boolean raster (for ``tf.simulate``'s restrictions) |
| [`tf.prioritize.zonation_features`](/python/api/prioritization/#terraflux.prioritize.zonation_features) | The features as Zonation 5 transforms them before ranking (condition, connectivity kernels, interactions, matrix connectivity, retention, impacts: see `zonation()` for each), back in the features' own units, for an optimization problem (``problem(cost, zonation_features(...))``) |
| [`tf.prioritize.compare_zonation`](/python/api/prioritization/#terraflux.prioritize.compare_zonation) | Compare an optimization solution of ``p`` (solved if not given) with a Zonation ranking of the same features and costs (run with ``rule`` and ``zonation_kw`` if not given: a `ZonationResult` or a rank raster) cut at the solution's cost |
| [`tf.prioritize.ZonationComparison`](/python/api/prioritization/#terraflux.prioritize.ZonationComparison) | A Zonation ranking and an optimization solution with the same budget |
| [`tf.prioritize.sdm_features`](/python/api/prioritization/#terraflux.prioritize.sdm_features) | Features from species distribution models (``tf.sdm``): every result's prediction on ``env`` (its ensemble when it has several models, else its model), as a stack for `problem()` |
| [`tf.prioritize.report`](/python/api/prioritization/#terraflux.prioritize.report) | Solve ``p`` (unless a solution is given) and evaluate the solution: the number of units, cost, boundary (raster and polygon units), the features' representation and the targets' coverage; with ``importance`` the Ferrier irreplaceability of the selected units (problems with one target per feature) or else their rarity-weighted richness; with ``extinction_z`` the species-area extinction risk; with ``zonation`` the comparison with a Zonation ranking at the same cost (raster units) |
| [`tf.prioritize.PrioritizationReport`](/python/api/prioritization/#terraflux.prioritize.PrioritizationReport) | A solution and its evaluation in one object: ``tf.plot(report)`` draws it, ``tf.plot(report).save( "report.html")`` writes it as a page that works offline, ``tf.save(report, folder)`` its tables and maps |
| [`tf.prioritize.coarsen`](/python/api/prioritization/#terraflux.prioritize.coarsen) | A problem's rasters on blocks of ``factor`` x ``factor`` cells (for continental problems: solve the coarse problem, then `refine()` the selection): every block's cost and amounts are the sums of its cells' with a cost (the fine planning units; a block without any has none), so the refined selection holds what the coarse one does |
| [`tf.prioritize.refine`](/python/api/prioritization/#terraflux.prioritize.refine) | A selection on coarse blocks (a `Solution` of a `coarsen()`-ed problem, or its selection raster) back on the fine grid of ``like`` (the fine cost raster): every cell takes its block's value, NaN where ``like`` has none |
| [`tf.prioritize.actions_data`](/python/api/prioritization/#terraflux.prioritize.actions_data) | The inputs of a multi-action problem, checked and completed as prioriactions's ``inputData()`` |
| [`tf.prioritize.actions_raster_data`](/python/api/prioritization/#terraflux.prioritize.actions_raster_data) | The inputs of a multi-action problem from rasters (see `actions_data()` for the tables) |
| [`tf.prioritize.ActionsData`](/python/api/prioritization/#terraflux.prioritize.ActionsData) | Checked inputs of a multi-action problem (prioriactions's ``inputData()``): see `actions_data` |
| [`tf.prioritize.ActionsProblem`](/python/api/prioritization/#terraflux.prioritize.ActionsProblem) | A multi-action problem (prioriactions's ``problem()``): see `problem` |
| [`tf.prioritize.ActionsSolution`](/python/api/prioritization/#terraflux.prioritize.ActionsSolution) | A solution of an `ActionsProblem` |
| [`tf.prioritize.ActionsPortfolio`](/python/api/prioritization/#terraflux.prioritize.ActionsPortfolio) | Several solutions of an `ActionsProblem` (`add_cuts_portfolio` and the others) |
| [`tf.prioritize.restoration_problem`](/python/api/prioritization/#terraflux.prioritize.restoration_problem) | A restoration problem: how much of every converted cell to restore (and of natural land to conserve or convert) |
| [`tf.prioritize.RestorationProblem`](/python/api/prioritization/#terraflux.prioritize.RestorationProblem) | A restoration problem (see `restoration_problem`), built up by its ``add_*`` methods; every method returns a new problem |
| [`tf.prioritize.RestorationSolution`](/python/api/prioritization/#terraflux.prioritize.RestorationSolution) | A restoration plan |
| [`tf.prioritize.RestorationTradeOff`](/python/api/prioritization/#terraflux.prioritize.RestorationTradeOff) | Plans over a trade-off (`tradeoff`) or over sets of weights (`compare_weights`) |
| [`tf.prioritize.ecosystem_areas`](/python/api/prioritization/#terraflux.prioritize.ecosystem_areas) | The natural area of every ecosystem (ecoregion) now and before conversion, and what restoring a cell gives it: the input of `add_extinction_risk_benefit()` for the risk of ecosystem collapse (plangea's ``ec``) |
| [`tf.prioritize.ecosystem_targets`](/python/api/prioritization/#terraflux.prioritize.ecosystem_targets) | Share a restoration target among ecosystems for the least risk of collapse (plangea's ecosystemic sub-region targets, ``harmonize_sr_ecosys``) |
| [`tf.prioritize.eval_risk_reduction`](/python/api/prioritization/#terraflux.prioritize.eval_risk_reduction) | The reduction of the extinction risk a plan brings (plangea's ``multi_exrisk_aggregate``), with the uncertainty of the species-area exponent z |
| [`tf.prioritize.RiskReduction`](/python/api/prioritization/#terraflux.prioritize.RiskReduction) | Result of `eval_risk_reduction`: per species, its habitat now (``current``), without the plan (``baseline``: less than now when conservation's baseline loses land) and gained by it, its risk before and after; the total reduction (negative when conversion raises the risk) and its standard deviation from the uncertainty of z |
| [`tf.prioritize.marxan`](/python/api/prioritization/#terraflux.prioritize.marxan) | Run Marxan: ``num_reps`` independent runs of annealing and iterative improvement (or the greedy heuristics) minimising cost + ``blm`` x boundary + penalties for the features' shortfalls |
| [`tf.prioritize.MarxanResult`](/python/api/prioritization/#terraflux.prioritize.MarxanResult) | The runs of `marxan` |

### Species distributions

| Name | Summary |
|---|---|
| [`tf.sdm.prepare`](/python/api/sdm/#terraflux.sdm.prepare) | Prepare occurrence records and predictors for a species distribution model, in one call |
| [`tf.sdm.download_gbif`](/python/api/sdm/#terraflux.sdm.download_gbif) | Occurrence records of a species from GBIF, with coordinates |
| [`tf.sdm.clean_occurrences`](/python/api/sdm/#terraflux.sdm.clean_occurrences) | Flag or drop occurrence records with suspicious coordinates |
| [`tf.sdm.reference_data`](/python/api/sdm/#terraflux.sdm.reference_data) | A geographic reference data set for cleaning occurrence records |
| [`tf.sdm.one_per_cell`](/python/api/sdm/#terraflux.sdm.one_per_cell) | Keep one occurrence record per cell of the predictor grid |
| [`tf.sdm.thin`](/python/api/sdm/#terraflux.sdm.thin) | Thin occurrence records so that no two kept records are closer than ``distance`` |
| [`tf.sdm.accessible_area`](/python/api/sdm/#terraflux.sdm.accessible_area) | The area the species could have reached: where its background is drawn |
| [`tf.sdm.select_predictors`](/python/api/sdm/#terraflux.sdm.select_predictors) | Drop predictors that the others already explain (collinearity) |
| [`tf.sdm.vif`](/python/api/sdm/#terraflux.sdm.vif) | Variance inflation factor of each predictor: how well the others predict it |
| [`tf.sdm.background`](/python/api/sdm/#terraflux.sdm.background) | Background points: a sample of the conditions available to the species |
| [`tf.sdm.bias_surface`](/python/api/sdm/#terraflux.sdm.bias_surface) | Sampling effort as a surface: a Gaussian kernel density of the records of a group |
| [`tf.sdm.dataset`](/python/api/sdm/#terraflux.sdm.dataset) | Put presences, background points and predictor values together |
| [`tf.sdm.SDMData`](/python/api/sdm/#terraflux.sdm.SDMData) | Presences and background (or absences) with their predictor values, ready for a model |
| [`tf.sdm.spatial_blocks`](/python/api/sdm/#terraflux.sdm.spatial_blocks) | Folds made of spatial blocks: no fold is a neighbour of the records it tests |
| [`tf.sdm.autocorrelation_range`](/python/api/sdm/#terraflux.sdm.autocorrelation_range) | The range of spatial autocorrelation of each predictor (and their median), in metres |
| [`tf.sdm.spatial_clusters`](/python/api/sdm/#terraflux.sdm.spatial_clusters) | Folds from k-means clusters of the records' coordinates (or of their environment) |
| [`tf.sdm.buffer_folds`](/python/api/sdm/#terraflux.sdm.buffer_folds) | Leave-one-out folds with a buffer: each presence is a test, nothing within ``size`` trains |
| [`tf.sdm.quadrant_folds`](/python/api/sdm/#terraflux.sdm.quadrant_folds) | Four folds from the occurrences split at their median latitude and longitude (ENMeval "block") |
| [`tf.sdm.checkerboard_folds`](/python/api/sdm/#terraflux.sdm.checkerboard_folds) | Folds from a checkerboard of aggregated cells (ENMeval "checkerboard") |
| [`tf.sdm.random_folds`](/python/api/sdm/#terraflux.sdm.random_folds) | Random k-fold of the presences, background always training (ENMeval "randomkfold") |
| [`tf.sdm.jackknife_folds`](/python/api/sdm/#terraflux.sdm.jackknife_folds) | Leave-one-out: each presence its own fold, background always training (ENMeval "jackknife") |
| [`tf.sdm.MaxNet`](/python/api/sdm/#terraflux.sdm.MaxNet) | Maxent fitted as R's maxnet does it, natively: the same model, faster |
| [`tf.sdm.evaluate`](/python/api/sdm/#terraflux.sdm.evaluate) | Evaluate predictions at presences against background points (or absences) |
| [`tf.sdm.Evaluation`](/python/api/sdm/#terraflux.sdm.Evaluation) | How well predictions separate presences from background (or absences) |
| [`tf.sdm.boyce`](/python/api/sdm/#terraflux.sdm.boyce) | Continuous Boyce index: does predicted suitability rise with the frequency of presences? |
| [`tf.sdm.threshold`](/python/api/sdm/#terraflux.sdm.threshold) | A threshold that turns continuous predictions into presence / absence |
| [`tf.sdm.omission_rate`](/python/api/sdm/#terraflux.sdm.omission_rate) | Share of test presences predicted below a threshold set on the training presences |
| [`tf.sdm.mess`](/python/api/sdm/#terraflux.sdm.mess) | Multivariate environmental similarity surface: how far conditions lie outside the reference |
| [`tf.sdm.mop`](/python/api/sdm/#terraflux.sdm.mop) | Mobility-oriented parity: where new conditions are outside or far from the reference ones |
| [`tf.sdm.exdet`](/python/api/sdm/#terraflux.sdm.exdet) | Extrapolation detection: univariate (NT1) and combinatorial (NT2) novelty |
| [`tf.sdm.fit`](/python/api/sdm/#terraflux.sdm.fit) | Fit models to SDMData, cross-validate them on its folds, and fit them to all the data |
| [`tf.sdm.SDMResult`](/python/api/sdm/#terraflux.sdm.SDMResult) | Fitted models, their cross-validated scores and what is needed to project them |
| [`tf.sdm.predict`](/python/api/sdm/#terraflux.sdm.predict) | Project fitted models onto rasters of predictors (present or future conditions) |
| [`tf.sdm.response_curves`](/python/api/sdm/#terraflux.sdm.response_curves) | How each model's suitability changes along one predictor, the others held at their mean (or median) |
| [`tf.sdm.importance`](/python/api/sdm/#terraflux.sdm.importance) | Permutation importance: how much AUC drops when a predictor's values are shuffled |
| [`tf.sdm.migrate`](/python/api/sdm/#terraflux.sdm.migrate) | A species' dispersal through the habitat suitability of successive periods, as MigClim simulates it |
| [`tf.sdm.Migration`](/python/api/sdm/#terraflux.sdm.Migration) | A dispersal simulation (`migrate`) |
| [`tf.sdm.area_of_habitat`](/python/api/sdm/#terraflux.sdm.area_of_habitat) | Area of habitat of species: their ranges refined by habitats and elevations |
| [`tf.sdm.HabitatAreas`](/python/api/sdm/#terraflux.sdm.HabitatAreas) | Result of `area_of_habitat` |
| [`tf.sdm.cell_area_km2`](/python/api/sdm/#terraflux.sdm.cell_area_km2) | The area of every cell of a raster's grid in km²: the product of the resolutions in a projected coordinate system (in its linear unit), the cell on the ellipsoid in a geographic one |
| [`tf.sdm.example_data`](/python/api/sdm/#terraflux.sdm.example_data) | Path of an example data set for species distribution modelling |

### Animal movement

| Name | Summary |
|---|---|
| [`tf.move.load_tracks`](/python/api/move/#terraflux.move.load_tracks) | Read a Movebank file into tracks (move2's ``mt_read``): a GeoDataFrame of points in EPSG:4326 |
| [`tf.move.example_data`](/python/api/move/#terraflux.move.example_data) | Path of an example data set for movement ecology |
| [`tf.move.as_tracks`](/python/api/move/#terraflux.move.as_tracks) | Tracks from a table of locations (move2's ``mt_as_move2``): a GeoDataFrame ``tf.move`` understands |
| [`tf.move.set_columns`](/python/api/move/#terraflux.move.set_columns) | Record which columns hold the time and the track (``tracks.attrs``), so later calls need not name them |
| [`tf.move.simulate_brownian`](/python/api/move/#terraflux.move.simulate_brownian) | Tracks of Brownian motion (move2's ``mt_sim_brownian_motion``) |
| [`tf.move.distance`](/python/api/move/#terraflux.move.distance) | Distance from each location to the next location of its track |
| [`tf.move.time_lags`](/python/api/move/#terraflux.move.time_lags) | Time from each location to the next of its track (move2's ``mt_time_lags``), in seconds |
| [`tf.move.speed`](/python/api/move/#terraflux.move.speed) | Speed of each step: `distance` over `time_lags()` (move2's ``mt_speed``) |
| [`tf.move.azimuth`](/python/api/move/#terraflux.move.azimuth) | Direction of each step, clockwise from north (move2's ``mt_azimuth``) |
| [`tf.move.turn_angle`](/python/api/move/#terraflux.move.turn_angle) | Change of direction at each location, in [-pi, pi) (move2's ``mt_turnangle``) |
| [`tf.move.unique`](/python/api/move/#terraflux.move.unique) | Which locations to keep so that no two of a track share their time (move2's ``mt_unique``) |
| [`tf.move.filter_unique`](/python/api/move/#terraflux.move.filter_unique) | ``tracks`` without duplicated times within a track (move2's ``mt_filter_unique``; see `unique()`) |
| [`tf.move.per_interval`](/python/api/move/#terraflux.move.per_interval) | Which locations to keep for one per time interval of each track (move2's ``mt_per_interval``) |
| [`tf.move.filter_per_interval`](/python/api/move/#terraflux.move.filter_per_interval) | ``tracks`` with one location per time interval of each track (see `per_interval()`) |
| [`tf.move.movebank_visible`](/python/api/move/#terraflux.move.movebank_visible) | Which locations Movebank shows (move2's ``mt_movebank_visible``): a boolean mask |
| [`tf.move.filter_movebank_visible`](/python/api/move/#terraflux.move.filter_movebank_visible) | ``tracks`` without the locations Movebank hides (see `movebank_visible()`) |
| [`tf.move.interpolate`](/python/api/move/#terraflux.move.interpolate) | Locations at given times between the recorded ones (move2's ``mt_interpolate``) |
| [`tf.move.track_lines`](/python/api/move/#terraflux.move.track_lines) | Each track as a line through its locations in time order (move2's ``mt_track_lines``) |
| [`tf.move.segments`](/python/api/move/#terraflux.move.segments) | The step from each location to the next of its track as a line (move2's ``mt_segments``) |
| [`tf.move.track_data`](/python/api/move/#terraflux.move.track_data) | One row per track with the columns whose value does not change within a track (move2's ``mt_track_data``) |
| [`tf.move.aeqd_crs`](/python/api/move/#terraflux.move.aeqd_crs) | An azimuthal equidistant projection centred on the tracks (move2's ``mt_aeqd_crs``) |
| [`tf.move.resample`](/python/api/move/#terraflux.move.resample) | Locations at a regular rate, in bursts (amt's ``track_resample``, the same locations and bursts) |
| [`tf.move.filter_min_n_burst`](/python/api/move/#terraflux.move.filter_min_n_burst) | The bursts (`resample()`) of at least ``min_n`` locations (amt's ``filter_min_n_burst``) |
| [`tf.move.steps`](/python/api/move/#terraflux.move.steps) | The steps between consecutive locations of each track (amt's ``steps``, ``steps_by_burst``) |
| [`tf.move.time_of_day`](/python/api/move/#terraflux.move.time_of_day) | Day or night at each location, or at the end or start of each step (amt's ``time_of_day``) |
| [`tf.move.nsd`](/python/api/move/#terraflux.move.nsd) | Net squared displacement: the squared distance of each location from the first of its track (amt's ``nsd``) |
| [`tf.move.cum_dist`](/python/api/move/#terraflux.move.cum_dist) | The length of each track: the sum of its steps (amt's ``cum_dist``), metres in longitude / latitude |
| [`tf.move.tot_dist`](/python/api/move/#terraflux.move.tot_dist) | The distance from the first to the last location of each track (amt's ``tot_dist``) |
| [`tf.move.straightness`](/python/api/move/#terraflux.move.straightness) | `tot_dist()` over `cum_dist()`: 1 for a straight track, near 0 for a tortuous one (amt's ``straightness``) |
| [`tf.move.msd`](/python/api/move/#terraflux.move.msd) | Mean squared distance of the locations of each track from their centre (amt's ``msd``) |
| [`tf.move.sinuosity`](/python/api/move/#terraflux.move.sinuosity) | Sinuosity of each track (Benhamou 2004; amt's ``sinuosity``) |
| [`tf.move.tac`](/python/api/move/#terraflux.move.tac) | Turn angle correlation of each track (Dray et al. 2010; amt's ``tac``) |
| [`tf.move.make_distribution`](/python/api/move/#terraflux.move.make_distribution) | A distribution of step lengths or turn angles (amt's ``make_distribution``, ``make_*_distr``) |
| [`tf.move.Distribution`](/python/api/move/#terraflux.move.Distribution) | A distribution of step lengths or turn angles (amt's ``amt_distr``) |
| [`tf.move.fit_distr`](/python/api/move/#terraflux.move.fit_distr) | A distribution fitted to step lengths or turn angles (amt's ``fit_distr``, the same numbers) |
| [`tf.move.random_numbers`](/python/api/move/#terraflux.move.random_numbers) | Draws of a distribution of steps (amt's ``random_numbers``) |
| [`tf.move.random_steps`](/python/api/move/#terraflux.move.random_steps) | Each observed step with ``n_control`` random steps from its start (amt's ``random_steps``) |
| [`tf.move.extract_covariates`](/python/api/move/#terraflux.move.extract_covariates) | The values of rasters at locations, or at the ends or starts of steps (amt's ``extract_covariates``) |

### Land change simulation

| Name | Summary |
|---|---|
| [`tf.simulate.changes`](/python/api/simulate/#terraflux.simulate.changes) | Land change along a series of categorical maps |
| [`tf.simulate.Changes`](/python/api/simulate/#terraflux.simulate.Changes) | Land change along a series of categorical maps (see `changes`) |
| [`tf.simulate.IntensityAnalysis`](/python/api/simulate/#terraflux.simulate.IntensityAnalysis) | Intensity analysis of land change (Aldwaik & Pontius 2012), as OpenLand computes it |
| [`tf.simulate.crosstab`](/python/api/simulate/#terraflux.simulate.crosstab) | Cells of each class of one map that hold each class of another (a transition matrix of counts) |
| [`tf.simulate.difference_components`](/python/api/simulate/#terraflux.simulate.difference_components) | Agreement and the components of difference of each category and overall (diffeR's ``diffTablej``) |
| [`tf.simulate.overall_difference`](/python/api/simulate/#terraflux.simulate.overall_difference) | The overall difference and its components (diffeR's ``overallDiff``, ``overallQtyD``...) |
| [`tf.simulate.difference_resolutions`](/python/api/simulate/#terraflux.simulate.difference_resolutions) | Quantity, exchange and shift differences at increasingly coarse resolutions (diffeR's ``differenceMR``) |
| [`tf.simulate.composite_matrix`](/python/api/simulate/#terraflux.simulate.composite_matrix) | Cross-tabulation at a coarser resolution by the composite method (diffeR's ``composite``) |
| [`tf.simulate.memberships`](/python/api/simulate/#terraflux.simulate.memberships) | The share of each category in each block of factor x factor cells (diffeR's ``memberships``) |
| [`tf.simulate.mad`](/python/api/simulate/#terraflux.simulate.mad) | Mean absolute deviation between two continuous maps and its components (diffeR's ``MAD``) |
| [`tf.simulate.sample_to_population`](/python/api/simulate/#terraflux.simulate.sample_to_population) | A sample's cross-tabulation scaled to the population of each row category (diffeR's ``sample2pop``) |
| [`tf.simulate.Markov`](/python/api/simulate/#terraflux.simulate.Markov) | A Markov chain of land change: the transition probabilities of a period and of each time step |
| [`tf.simulate.WeightsOfEvidence`](/python/api/simulate/#terraflux.simulate.WeightsOfEvidence) | Weights of evidence of land use transitions, calibrated on two maps (Dinamica EGO's method) |
| [`tf.simulate.weights_of_evidence`](/python/api/simulate/#terraflux.simulate.weights_of_evidence) | Calibrate weights of evidence in one call: ``WeightsOfEvidence(ranges, **kwargs).fit(t0, t1, drivers)`` |
| [`tf.simulate.LogisticRegression`](/python/api/simulate/#terraflux.simulate.LogisticRegression) | Logistic regression of the presence of each land use class on the drivers (lulcc's ``glmModels``) |
| [`tf.simulate.Estimator`](/python/api/simulate/#terraflux.simulate.Estimator) | Potential of each class from a scikit-learn classifier, one model per class |
| [`tf.simulate.MultiCriteria`](/python/api/simulate/#terraflux.simulate.MultiCriteria) | Potential from weighted factors, the weights from pairwise comparisons (AHP; MOLUSCE's MCE) |
| [`tf.simulate.RasterPotential`](/python/api/simulate/#terraflux.simulate.RasterPotential) | A potential given as maps: one layer per class (or transition), as computed elsewhere |
| [`tf.simulate.Model`](/python/api/simulate/#terraflux.simulate.Model) | A land change model: potential, demand, allocation and rules |
| [`tf.simulate.Simulation`](/python/api/simulate/#terraflux.simulate.Simulation) | The maps of a simulation and what happened at each step |
| [`tf.simulate.AreaDemand`](/python/api/simulate/#terraflux.simulate.AreaDemand) | Cells each class must cover at each time step (CLUE-S's demand) |
| [`tf.simulate.Rules`](/python/api/simulate/#terraflux.simulate.Rules) | Decision rules of the allocation (lulcc's ``rules``, ``elas``, ``mask``, ``hist``, ``neighb``) |
| [`tf.simulate.Neighbourhood`](/python/api/simulate/#terraflux.simulate.Neighbourhood) | Neighbourhood thresholds (lulcc's ``NeighbRasterStack`` and ``nb.rules``) |
| [`tf.simulate.CLUE`](/python/api/simulate/#terraflux.simulate.CLUE) | CLUE-S allocation (Verburg et al. 2002), as lulcc's ``CluesModel`` |
| [`tf.simulate.Ordered`](/python/api/simulate/#terraflux.simulate.Ordered) | The ordered allocation of Fuchs et al. (2013) with stochastic choice, as lulcc's ``OrderedModel`` |
| [`tf.simulate.distance_to`](/python/api/simulate/#terraflux.simulate.distance_to) | Distance from each cell to the nearest cell of some classes |
| [`tf.simulate.share_of`](/python/api/simulate/#terraflux.simulate.share_of) | Share of the cells around each cell that hold some classes |
| [`tf.simulate.gravity_of`](/python/api/simulate/#terraflux.simulate.gravity_of) | Gravitational interaction potential of the cells of some classes |
| [`tf.simulate.Regional`](/python/api/simulate/#terraflux.simulate.Regional) | A demand per region (municipalities, biomes, basins...) |
| [`tf.simulate.CLUMondo`](/python/api/simulate/#terraflux.simulate.CLUMondo) | CLUMondo's demand-driven allocation of land systems |
| [`tf.simulate.ServiceDemand`](/python/api/simulate/#terraflux.simulate.ServiceDemand) | The demand for goods and services at each time step (CLUMondo's ``demand.in``) |
| [`tf.simulate.Supply`](/python/api/simulate/#terraflux.simulate.Supply) | What one cell of each land system supplies of each good or service (CLUMondo's ``lusmatrix``) |
| [`tf.simulate.NeighbourhoodEffect`](/python/api/simulate/#terraflux.simulate.NeighbourhoodEffect) | CLUMondo's neighbourhood function (``neighmat.txt`` and ``alloc2.reg``) |
| [`tf.simulate.LandSystemStack`](/python/api/simulate/#terraflux.simulate.LandSystemStack) | Binary layers, possibly overlapping, as land systems (bit masks) |
| [`tf.simulate.Roles`](/python/api/simulate/#terraflux.simulate.Roles) | What each transition between land systems does to their layers |
| [`tf.simulate.Cap`](/python/api/simulate/#terraflux.simulate.Cap) | At most so many cells (or so much area) a year may make a role's transitions (or given pairs) |
| [`tf.simulate.Succession`](/python/api/simulate/#terraflux.simulate.Succession) | Autonomous change after some years, whatever the demand (CLUMondo's code 1000 + years) |
| [`tf.simulate.ExpanderPatcher`](/python/api/simulate/#terraflux.simulate.ExpanderPatcher) | Dinamica's Expander and Patcher allocation of transitions (``AllocateTransitions``) |
| [`tf.simulate.ContinuousPatcher`](/python/api/simulate/#terraflux.simulate.ContinuousPatcher) | New patches that take a stock (biomass, carbon, timber) up to a quota |
| [`tf.simulate.StockAllocation`](/python/api/simulate/#terraflux.simulate.StockAllocation) | What `allocate` took |
| [`tf.simulate.StockSimulation`](/python/api/simulate/#terraflux.simulate.StockSimulation) | What `run` did, step by step |
| [`tf.simulate.RoadBuilder`](/python/api/simulate/#terraflux.simulate.RoadBuilder) | New roads each step, from the network towards attractive places |
| [`tf.simulate.RoadDistance`](/python/api/simulate/#terraflux.simulate.RoadDistance) | Distance to a growing road network: a dynamic variable (see `dynamic`) |
| [`tf.simulate.friction_cost`](/python/api/simulate/#terraflux.simulate.friction_cost) | Cost of reaching every cell from the nearest source over a friction surface |
| [`tf.simulate.destinations`](/python/api/simulate/#terraflux.simulate.destinations) | Cells drawn by attraction in quadrants of a map |
| [`tf.simulate.pathways`](/python/api/simulate/#terraflux.simulate.pathways) | Paths from source cells to a network down a cost surface |
| [`tf.simulate.CARS`](/python/api/simulate/#terraflux.simulate.CARS) | PLUS's CA based on multi-type random patch seeds (Liang et al. 2021) |
| [`tf.simulate.MolusceWoE`](/python/api/simulate/#terraflux.simulate.MolusceWoE) | MOLUSCE's weights of evidence (``WoeManager``): one model per transition, persistence included |
| [`tf.simulate.MolusceCA`](/python/api/simulate/#terraflux.simulate.MolusceCA) | MOLUSCE's simulator: each iteration makes the observed transitions where the model is most confident |
| [`tf.simulate.PatchGrowing`](/python/api/simulate/#terraflux.simulate.PatchGrowing) | FUTURES' patch growing allocation (r.futures.simulation) |
| [`tf.simulate.DevelopmentDemand`](/python/api/simulate/#terraflux.simulate.DevelopmentDemand) | Cells each subregion develops at each step (r.futures.simulation's ``demand`` file) |
| [`tf.simulate.DevelopmentPotential`](/python/api/simulate/#terraflux.simulate.DevelopmentPotential) | The development potential's coefficients per subregion (r.futures.potential's table) |
| [`tf.simulate.development_pressure`](/python/api/simulate/#terraflux.simulate.development_pressure) | The development pressure of each cell (r.futures.devpressure) |
| [`tf.simulate.Flooding`](/python/api/simulate/#terraflux.simulate.Flooding) | FUTURES' flood submodel (Sanchez et al. 2023): floods, damage, response, abandonment, migration |
| [`tf.simulate.patch_library`](/python/api/simulate/#terraflux.simulate.patch_library) | The sizes of the patches developed between two maps (r.futures.calib's patch library) |
| [`tf.simulate.calibrate_patches`](/python/api/simulate/#terraflux.simulate.calibrate_patches) | Calibrate FUTURES' patch compactness and discount factor (r.futures.calib) |
| [`tf.simulate.three_map_comparison`](/python/api/simulate/#terraflux.simulate.three_map_comparison) | Compare the reference maps at the start and the end with the simulated end at several resolutions |
| [`tf.simulate.ThreeMapComparison`](/python/api/simulate/#terraflux.simulate.ThreeMapComparison) | Three maps compared at several resolutions (lulcc's ``ThreeMapComparison``) |
| [`tf.simulate.compare_maps`](/python/api/simulate/#terraflux.simulate.compare_maps) | MOLUSCE's comparison of two maps (``DependenceCoef``): kappa, agreement and dependence |
| [`tf.simulate.error_budget`](/python/api/simulate/#terraflux.simulate.error_budget) | MOLUSCE's error budget (Pontius 2002): agreement for levels of information, at coarser resolutions |
| [`tf.simulate.futures_validation`](/python/api/simulate/#terraflux.simulate.futures_validation) | FUTURES' validation (``r.futures.validation``): disagreement, kappa, kappa simulation, change metrics |
| [`tf.simulate.futures_validation_grid`](/python/api/simulate/#terraflux.simulate.futures_validation_grid) | FUTURES' validation per tile or per region (``r.futures.gridvalidation``) |
| [`tf.simulate.fuzzy_similarity`](/python/api/simulate/#terraflux.simulate.fuzzy_similarity) | Fuzzy similarity of two categorical maps (Dinamica EGO's ``CalcReciprocalSimilarityMap``) |
| [`tf.simulate.FuzzySimilarity`](/python/api/simulate/#terraflux.simulate.FuzzySimilarity) | The fuzzy similarity of two maps (`fuzzy_similarity`) |
| [`tf.simulate.differences`](/python/api/simulate/#terraflux.simulate.differences) | The map of differences: each cell that changed between the maps, in its new class; NaN elsewhere |
| [`tf.simulate.similarity_of_differences`](/python/api/simulate/#terraflux.simulate.similarity_of_differences) | Dinamica EGO's validation: fuzzy similarity of the observed and the simulated change, per window |
| [`tf.simulate.validate`](/python/api/simulate/#terraflux.simulate.validate) | Compare a simulated map with the observed one in every way the land change tools do |
| [`tf.simulate.Validation`](/python/api/simulate/#terraflux.simulate.Validation) | Every comparison of a simulated map with the observed one (`validate`) |
| [`tf.simulate.calibrate`](/python/api/simulate/#terraflux.simulate.calibrate) | Search the settings of a model that best reproduce an observed map |
| [`tf.simulate.Calibration`](/python/api/simulate/#terraflux.simulate.Calibration) | The result of `calibrate` |
| [`tf.simulate.run`](/python/api/simulate/#terraflux.simulate.run) | Fit a land change model on two maps, check it on the observed period and project it |
| [`tf.simulate.Projection`](/python/api/simulate/#terraflux.simulate.Projection) | The result of `run` |
| [`tf.simulate.agreement`](/python/api/simulate/#terraflux.simulate.agreement) | How a simulated map matches the observed one, cell by cell and in quantity |
| [`tf.simulate.presets`](/python/api/simulate/#terraflux.simulate.presets) | The classic land change models, assembled from tf.simulate's pieces |
| [`tf.simulate.terraflux`](/python/api/simulate/#terraflux.simulate.terraflux) | Run a Terraflux model with the ``terraflux`` command; the simulated maps ``(step, y, x)`` |
| [`tf.simulate.scenarios`](/python/api/simulate/#terraflux.simulate.scenarios) | Run a model under several scenarios, several times each, in parallel |
| [`tf.simulate.Ensemble`](/python/api/simulate/#terraflux.simulate.Ensemble) | The simulations of `scenarios`: every scenario, replicate and year |
| [`tf.simulate.example_data`](/python/api/simulate/#terraflux.simulate.example_data) | Path of an example data set for land change modelling |

### Map accuracy & area

| Name | Summary |
|---|---|
| [`tf.accuracy.sample_size`](/python/api/accuracy/#terraflux.accuracy.sample_size) | The size of a stratified random sample for the accuracy and the area of a map, and its share of each stratum |
| [`tf.accuracy.SampleSize`](/python/api/accuracy/#terraflux.accuracy.SampleSize) | The size of a stratified sample and its share of each stratum (`sample_size`) |
| [`tf.accuracy.allocate`](/python/api/accuracy/#terraflux.accuracy.allocate) | The sample of each stratum, and whether the minima asked for more than ``n`` |
| [`tf.accuracy.sample`](/python/api/accuracy/#terraflux.accuracy.sample) | A stratified random sample of a map's cells for its accuracy and area (Olofsson et al. 2014) |
| [`tf.accuracy.assess`](/python/api/accuracy/#terraflux.accuracy.assess) | The accuracy and the area of a map's classes from a stratified random sample labelled with the reference |
| [`tf.accuracy.Accuracy`](/python/api/accuracy/#terraflux.accuracy.Accuracy) | The accuracy and the area of a map's classes from a stratified sample (`assess`) |
| [`tf.accuracy.qnorm`](/python/api/accuracy/#terraflux.accuracy.qnorm) | The standard normal's quantile (Wichura's AS 241, as R's ``qnorm``) |

### Deforestation allocation (VT0007)

| Name | Summary |
|---|---|
| [`tf.redd.edge_distance`](/python/api/redd/#terraflux.redd.edge_distance) | The distance of every forest cell to the nearest cell without forest, in map units (VT0007's input) |
| [`tf.redd.nrt`](/python/api/redd/#terraflux.redd.nrt) | VT0007's negligible risk threshold (UDef-ARP's ``nrt_calculation``) |
| [`tf.redd.benchmark`](/python/api/redd/#terraflux.redd.benchmark) | VT0007's benchmark vulnerability map (UDef-ARP's ``geometric_classification``) |
| [`tf.redd.alternative`](/python/api/redd/#terraflux.redd.alternative) | VT0007's alternative vulnerability map (UDef-ARP's ``geometric_classification_alternative``) |
| [`tf.redd.fit`](/python/api/redd/#terraflux.redd.fit) | VT0007's fit of a vulnerability map to a period's deforestation (UDef-ARP's fitting) |
| [`tf.redd.Fit`](/python/api/redd/#terraflux.redd.Fit) | The fit of a vulnerability map to a period's deforestation (`fit`) |
| [`tf.redd.allocate`](/python/api/redd/#terraflux.redd.allocate) | VT0007's allocation of the expected deforestation (UDef-ARP's prediction) |
| [`tf.redd.Allocation`](/python/api/redd/#terraflux.redd.Allocation) | VT0007's allocation of the expected deforestation (`allocate`) |
| [`tf.redd.assess`](/python/api/redd/#terraflux.redd.assess) | VT0007's assessment of an allocation (UDef-ARP's model evaluation) |
| [`tf.redd.Assessment`](/python/api/redd/#terraflux.redd.Assessment) | VT0007's assessment of an allocation (`assess`) |

### Brazil's Forest Code

| Name | Summary |
|---|---|
| [`tf.forest_code.app`](/python/api/forest-code/#terraflux.forest_code.app) | The Áreas de Preservação Permanente of Brazil's Forest Code (Lei 12.651/2012, art. 4) |
| [`tf.forest_code.APP`](/python/api/forest-code/#terraflux.forest_code.APP) | The Áreas de Preservação Permanente of a map (`app`) |
| [`tf.forest_code.river_margin`](/python/api/forest-code/#terraflux.forest_code.river_margin) | Art. 4, I: the margin of a river of a width (metres): 30 m below 10 m wide, 50 to 50, 100 to 200, 200 to 600, 500 |
| [`tf.forest_code.consolidated_river`](/python/api/forest-code/#terraflux.forest_code.consolidated_river) | Art. 61-A, §§ 1-4: a river's margin in a consolidated area by the property's fiscal modules |
| [`tf.forest_code.consolidated_lake`](/python/api/forest-code/#terraflux.forest_code.consolidated_lake) | Art. 61-A, § 6: a lake's margin in a consolidated area: 5, 8, 15 m up to 1, 2, 4 modules; 30 m beyond |
| [`tf.forest_code.reserve`](/python/api/forest-code/#terraflux.forest_code.reserve) | The balance of every rural property's Reserva Legal (Lei 12.651/2012) |
| [`tf.forest_code.ReservaLegal`](/python/api/forest-code/#terraflux.forest_code.ReservaLegal) | The Reserva Legal's balance of every rural property (`reserve`) |

### Ecosystem services

| Name | Summary |
|---|---|
| [`tf.services.carbon`](/python/api/services/#terraflux.services.carbon) | Carbon storage of a landscape and its change between two maps (InVEST's carbon model) |
| [`tf.services.Carbon`](/python/api/services/#terraflux.services.Carbon) | Carbon stored in a landscape, and its change (`carbon`) |
| [`tf.services.valuation_constant`](/python/api/services/#terraflux.services.valuation_constant) | The factor turning a change in carbon into its net present value (InVEST's) |
| [`tf.services.emissions`](/python/api/services/#terraflux.services.emissions) | The carbon emitted and removed by land change over a series of maps (the IPCC's stock-difference method) |
| [`tf.services.Emissions`](/python/api/services/#terraflux.services.Emissions) | The carbon emitted and removed along a series of land use maps (`emissions`) |
| [`tf.services.water_yield`](/python/api/services/#terraflux.services.water_yield) | Annual water yield, consumption and hydropower value (InVEST's annual water yield model) |
| [`tf.services.WaterYield`](/python/api/services/#terraflux.services.WaterYield) | Annual water yield of a landscape (`water_yield`) |
| [`tf.services.seasonal_water_yield`](/python/api/services/#terraflux.services.seasonal_water_yield) | Quickflow, local recharge and baseflow by month and year (InVEST's seasonal water yield model) |
| [`tf.services.SeasonalWaterYield`](/python/api/services/#terraflux.services.SeasonalWaterYield) | Quickflow, recharge and baseflow of a landscape (`seasonal_water_yield`) |
| [`tf.services.sdr`](/python/api/services/#terraflux.services.sdr) | Sediment export, deposition and retention (InVEST's sediment delivery ratio model) |
| [`tf.services.SDR`](/python/api/services/#terraflux.services.SDR) | Sediment export and retention of a landscape (`sdr`) |
| [`tf.services.ndr`](/python/api/services/#terraflux.services.ndr) | Nutrient export and retention (InVEST's nutrient delivery ratio model) |
| [`tf.services.NDR`](/python/api/services/#terraflux.services.NDR) | Nutrient export and retention of a landscape (`ndr`) |
| [`tf.services.habitat_quality`](/python/api/services/#terraflux.services.habitat_quality) | Habitat degradation, quality and rarity (InVEST's habitat quality model) |
| [`tf.services.HabitatQuality`](/python/api/services/#terraflux.services.HabitatQuality) | Habitat degradation, quality and rarity (`habitat_quality`) |
| [`tf.services.pollination`](/python/api/services/#terraflux.services.pollination) | Pollinator abundance and crop pollination (InVEST's crop pollination model) |
| [`tf.services.Pollination`](/python/api/services/#terraflux.services.Pollination) | Pollinator supply and abundance, and the yield of farms (`pollination`) |
| [`tf.services.coastal_vulnerability`](/python/api/services/#terraflux.services.coastal_vulnerability) | Exposure of the coast to erosion and flooding (InVEST's coastal vulnerability model) |
| [`tf.services.CoastalVulnerability`](/python/api/services/#terraflux.services.CoastalVulnerability) | Exposure of the coast to erosion and flooding (`coastal_vulnerability`) |

### Uncertainty

| Name | Summary |
|---|---|
| [`tf.uncertainty.sum_sd`](/python/api/uncertainty/#terraflux.uncertainty.sum_sd) | Standard deviation of a total of uncertain values, sum of w_i v_i |
| [`tf.uncertainty.product_sd`](/python/api/uncertainty/#terraflux.uncertainty.product_sd) | Standard deviation of the product of two uncertain values, x v |
| [`tf.uncertainty.delta_method`](/python/api/uncertainty/#terraflux.uncertainty.delta_method) | Uncertainty of a result from the uncertainty of its parameters (delta method) |
| [`tf.uncertainty.DeltaResult`](/python/api/uncertainty/#terraflux.uncertainty.DeltaResult) | Result of `delta_method` |
| [`tf.uncertainty.monte_carlo`](/python/api/uncertainty/#terraflux.uncertainty.monte_carlo) | Uncertainty of a result by drawing its parameters (Monte Carlo) |
| [`tf.uncertainty.MonteCarloResult`](/python/api/uncertainty/#terraflux.uncertainty.MonteCarloResult) | Result of `monte_carlo` |
| [`tf.uncertainty.interval`](/python/api/uncertainty/#terraflux.uncertainty.interval) | The normal confidence interval of a value: value -/+ z sd, z the normal quantile of the level |

### Sensitivity analysis

| Name | Summary |
|---|---|
| [`tf.sensitivity.design`](/python/api/sensitivity/#terraflux.sensitivity.design) | The runs of a global sensitivity analysis (SALib's samplers, the same values) |
| [`tf.sensitivity.Design`](/python/api/sensitivity/#terraflux.sensitivity.Design) | The runs of a sensitivity analysis (`design`) |
| [`tf.sensitivity.evaluate`](/python/api/sensitivity/#terraflux.sensitivity.evaluate) | The outputs of a model at every run of a design |
| [`tf.sensitivity.analyze`](/python/api/sensitivity/#terraflux.sensitivity.analyze) | The indices of a global sensitivity analysis from its runs and their outputs (SALib's, the same numbers) |
| [`tf.sensitivity.Sensitivity`](/python/api/sensitivity/#terraflux.sensitivity.Sensitivity) | The indices of a sensitivity analysis (`analyze`) |

### Statistics of maps

| Name | Summary |
|---|---|
| [`tf.stats.trend`](/python/api/stats/#terraflux.stats.trend) | The monotonic trend of every cell along a series of maps (pymannkendall's ``original_test``, the same numbers) |
| [`tf.stats.Trend`](/python/api/stats/#terraflux.stats.Trend) | Every cell's trend along a series of maps (`trend`) |
| [`tf.stats.mann_kendall`](/python/api/stats/#terraflux.stats.mann_kendall) | The Mann-Kendall test and Sen's slope of one series (pymannkendall's ``original_test``, the same bits) |
| [`tf.stats.cell_stats`](/python/api/stats/#terraflux.stats.cell_stats) | A statistic of a stack's layers at every cell (ArcGIS's Cell Statistics, terra's ``app``) |
| [`tf.stats.local_correlation`](/python/api/stats/#terraflux.stats.local_correlation) | The correlation and the least squares line between two maps in a moving window (terra's ``focalPairs``, ``focalReg``) |
| [`tf.stats.LocalCorrelation`](/python/api/stats/#terraflux.stats.LocalCorrelation) | The correlation and least squares line of two maps in a moving window (`local_correlation`) |
| [`tf.stats.moran`](/python/api/stats/#terraflux.stats.moran) | The global Moran's I of a map (PySAL's ``esda.Moran``, to rounding) |
| [`tf.stats.Moran`](/python/api/stats/#terraflux.stats.Moran) | The global Moran's I of a map (`moran`) |
| [`tf.stats.lisa`](/python/api/stats/#terraflux.stats.lisa) | The local Moran's I of every cell (Anselin's LISA; PySAL's ``esda.Moran_Local``, to rounding) |
| [`tf.stats.LISA`](/python/api/stats/#terraflux.stats.LISA) | The local Moran's I of every cell (`lisa`) |
| [`tf.stats.getis_ord`](/python/api/stats/#terraflux.stats.getis_ord) | Getis and Ord's Gi* of every cell (PySAL's ``esda.G_Local`` with ``star`` and row-standardised weights) |
| [`tf.stats.GetisOrd`](/python/api/stats/#terraflux.stats.GetisOrd) | Getis and Ord's Gi* of every cell (`getis_ord`) |
| [`tf.stats.variogram`](/python/api/stats/#terraflux.stats.variogram) | The sample variogram of points and a model fitted to it (R's gstat ``variogram`` and ``fit.variogram``) |
| [`tf.stats.Variogram`](/python/api/stats/#terraflux.stats.Variogram) | The sample variogram of points and the model fitted to it (`variogram`) |
| [`tf.stats.VariogramModel`](/python/api/stats/#terraflux.stats.VariogramModel) | A variogram model: a nugget and a structure of a shape, its partial sill and range (gstat's ``vgm``) |
| [`tf.stats.idw`](/python/api/stats/#terraflux.stats.idw) | Inverse distance weighting of points onto a map (R's gstat ``idw``, the same numbers) |
| [`tf.stats.kriging`](/python/api/stats/#terraflux.stats.kriging) | Ordinary kriging of points onto a map (R's gstat ``krige``, to rounding: 1e-10) |
| [`tf.stats.Kriging`](/python/api/stats/#terraflux.stats.Kriging) | Ordinary kriging of points onto a map (`kriging`) |
