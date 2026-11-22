# Changelog

All notable changes to Terraflux are documented here. The project follows
[Semantic Versioning](https://semver.org/). Since 1.0, a model saved by an earlier version opens in every later
version; a change that breaks models, the command line or the Python library's API would come only with a new major
version.

## [Unreleased]

### Added
- **The SAM tool in the Viewer** (`crates/app/src/sam_tool.rs`, `crates/app/src/map_view.rs`, `crates/app/src/builder.rs`): with objects shown, *SAM tool* in the side panel runs SAM's encoder once on the part of the image in view (in a thread; the status bar says so), then each click runs only the decoder and draws the mask live: left click inside, right click outside, drag a box. Enter keeps the object, writing its prompts into the model in one step of undo (a GeoPackage beside the model read by a `vector.read` that feeds a `segment.sam` node's `prompts`; later objects join the same file and node), Esc drops it; a class chosen goes with the object. The objects are the SAM node's own.
- **`segment.sam` takes `prompts`: the objects you point at** (`crates/ops/src/objects/deep.rs`): a vector of points (`label` 1 inside the object, 0 outside) and polygons (a box), with `group` for which object each is for (without it each feature is one) and `class` for its class. Each group is one object, the mask of the best predicted IoU kept to the connected area that holds the first point; the encoder runs only on the 1024-cell tiles the groups are anchored in and no grid of points is run; the overlaps go to the smaller object; a group that gives no mask is a warning; the table has a `group` column and the groups' classes become the objects' classes. The masks are the same cells as onnxruntime's in Python with the same weights (`tools/make_sam_prompts_reference.py`).
- **The profile of an object in the inspector, and the template "Crops by their seasons"** (`crates/app/src/series_view.rs`, `crates/app/src/objects_inspect.rs`, `crates/app/src/templates.rs`): when an `objects.series` made the objects, the inspector draws the selected object's series, the dates as points (a cloud leaves a gap), the smoothed line, the band of the samples of its class (median and interquartile range by date) and, with ctrl+click, a second object to compare. The template makes parcels of four crops with a year of NDVI each (clouds and noise drawn), measures their seasons and harmonics with `objects.series`, and classes them with a random forest. New tutorial, part 21 of the object-based series: Seasons of objects.
- **`objects.series` and `raster.phenology`: the seasons of a time series** (`crates/ops/src/objects/series.rs`, `crates/ops/src/phenology.rs`): from a stack with a layer a date (dates in the names, YYYY-MM-DD or YYYYMMDD, or `dates`), `objects.series` gives each object the mean of its cells on each date (a cloud is a missing value, a date with fewer than `min_share` of the cells is missing), and the columns of three groups: `values`, `phenology` (max, min, amplitude, peak_doy, peak_date, integral, the start and end of the season by TIMESAT's amplitude method, length, rise and fall rates; the series as it is, or smoothed by Savitzky-Golay, equal to scipy's, or by harmonics) and `harmonics` (a least squares fit of n harmonics of a one-year period, equal to numpy's: mean, trend, amplitude and phase of each, rmse), and the long table of the profiles. `raster.phenology` gives the same numbers for every cell.
- **`objects.similar` and *Find similar* in the objects inspector** (`crates/ops/src/objects/similar.rs`, `crates/app/src/objects_inspect.rs`): every object takes its similarity (cosine, or 1 / (1 + the Euclidean distance)) to the nearest of some examples (ids, a table of ids, or the objects already sampled), on the embeddings of `objects.embed` when there are some, else on the standardised attributes; the `top` or those above a `threshold` are the selection. In the Viewer, *Find similar* lights the objects like the selected ones as its threshold moves, and *Label all as <class>* writes them as samples in one step of undo.
- **Reference objects in the Viewer, and the template "Choose the segmentation by reference objects"** (`crates/app/src/reference_link.rs`, `crates/app/src/builder.rs`, `crates/app/src/templates.rs`): with objects shown, polygons drawn under **Draw on the map** can be added with **Add as reference objects**: a `vector.read` of them and a `segment.evaluate` of the objects are made in one step of undo. The panel **Reference objects** shows ED2, D, F, OS and US, **Show where it errs** colours each object by os less us (red: the reference was cut, blue: the object holds what is outside it) with a legend, and **Use as the reference of ...** gives the same reference to a `segment.optimize` that has none. The template draws an image of known regions, takes a polygon for every fourth of them as the reference and lets `segment.optimize` choose the scale by ED2. New tutorial, part 20 of the object-based series: Measuring a segmentation against reference objects.
- **`segment.evaluate`, and a `reference` for `segment.optimize`** (`crates/ops/src/objects/evaluate.rs`, `crates/ops/src/objects/optimize.rs`): a segmentation measured against reference objects (polygons drawn, or objects) as the R package segmetric measures it: os (over-segmentation), us (under-), d, qr, afi, precision, recall and the overlap F (Clinton et al. 2010; Lucieer and Stein 2002; Weidner 2008; Zhang et al. 2015), the same numbers as segmetric 0.3.0, and pse, nsr and ed2 after Liu et al. 2012 (which segmetric has not). Its `table` has a row per pair of a reference and an object, `summary` the averages, and `objects` the os and us of each object to paint on a map. `segment.optimize` with a `reference` also measures each run against it, adds os, us, d, ed2 and f_overlap to its scores, and takes the objective d, ed2 or f_overlap to choose the value that matches what was drawn; without a reference nothing changes.
- **A model carried to another scene says what it has not seen** (`crates/ops/src/ml/novelty.rs`, `crates/ops/src/ml/file.rs`, `crates/ops/src/raster_match.rs`, `crates/ops/src/objects/classify.rs`): the model of `ml.fit` keeps what it was trained on (each predictor's range, quantiles, mean and deviation, and the training rows, a seeded sample of 10 000 above that; models saved before read as they were and give no novelty). `ml.write` and `ml.read` carry a model in a JSON file between models. `objects.classify` and `ml.predict` take `novelty` (`mess`, the default, `mop` or `none`): the columns `mess` (or `mop`) and `novel`, a warning with the share of the objects and of the area outside and the attributes that weigh most (ExDet), and another when the scene's band means are all higher or all lower than the training's; the Viewer draws the novel objects hatched. `raster.match_histogram` brings a scene to another's histogram band by band (scikit-image's `match_histograms`, with a `mask` for the common area). The template "Classify a second scene with the model of the first" shows the whole path (the accuracy on the common ground goes from 0.46 to 0.93 after the match).
- **The confusion matrix as a chart, linked to the objects in the Viewer** (`crates/app/src/charts.rs`, `crates/app/src/confusion_link.rs`, `crates/ops/src/objects/accuracy.rs`): the `matrix` of `objects.accuracy` and the `cv_matrix` of `ml.fit` open as a confusion matrix: the counts coloured by the share of each row (a colour-blind safe ramp, the diagonal outlined), the user's and producer's accuracy in the margins and the overall accuracy above. A click on a cell of the accuracy's matrix lights the reference objects of that pair (mapped class and reference class) on the classified objects in the Viewer, the rest veiled, and the list under the chart goes to each object. `objects.accuracy` has a new output `sampled` (id, mapped and reference class of every reference object), which the diagonal cells need.
- **Spatial cross-validation of objects: `objects.folds`, `ml.fit`'s `fold` and `cv`, and `objects.accuracy`'s `training`** (`crates/ops/src/objects/folds.rs`, `crates/ops/src/ml/cv.rs`, `crates/ops/src/objects/accuracy.rs`): neighbouring objects are nearly alike, so folds drawn at random give an accuracy that is too high (Ploton et al. 2020; Meyer and Pebesma 2022). `objects.folds` deals the training objects into folds: `neighbours` (objects within `steps` steps of each other are never split, the classes kept even), `blocks` (Roberts et al. 2017) or `clusters` of the centroids, or `random`. `ml.fit` with `fold` (a column) or `cv` (k random stratified folds) fits the model with each fold out and gives `cv` (per fold and in all: accuracy, kappa, macro F1 and the F1 of each class; RMSE, MAE and R2 for values, the same numbers as scikit-learn's with the same folds) and `cv_matrix`, the model itself still fitted on every row. `objects.accuracy` takes the `training` objects and counts the reference objects that are training objects or touch one, with a warning. The template "Classify an image by clicking samples" shows the accuracy by place beside the one by random folds.
- **`ml.pca` with `method = "mnf"`: the minimum noise fraction** (`crates/ops/src/ml/pca.rs`): the components of a stack ordered by signal-to-noise ratio instead of variance (Green, Berman, Switzer and Craig 1988; Lee, Woodyatt and Berman 1990). The noise is read from the differences between neighbouring cells (`noise` shift, the right-hand neighbour, or shift_both, with the one below too), the bands are whitened by it and the components of the whitened bands are the scores `mnf1`, `mnf2`... (the noise of each has a variance of 1; the explained table has the eigenvalue and the `snr`). The model works with `ml.predict`, and the new output `denoised` is the image rebuilt from the components kept. The same numbers as Spectral Python's `mnf`, `reduce` and `denoise`.
- **Segmenters compared from any image in the Viewer** (`crates/app/src/segment_compare.rs`, `crates/app/src/layers_panel.rs`): a raster layer's menu has *Compare segmenters...*, which opens the comparison window on that image with no segmenter shown; each thumbnail then has *Add*, which adds that segmenter to the model reading the image, with the thumbnail's settings and beside the image's node, in one step of undo, and shows its objects. With a segmenter shown, *Use* works as before.
- **`ml.pca` analyses two dates for change** (`crates/ops/src/ml/pca.rs`): give the later date's maps as `later` (the same layers by name, on the same grid; with `predictors`, not a table) and choose `change`. `stacked` (the default; Byrne, Crapper and Mayo 1980; Fung and LeDrew 1987) is the PCA of the layers of both dates (`<layer>_1`, `<layer>_2`), whose explained table gains a `change` column (1 a pure difference of the dates, 0 a pure sum) and a note names the component most like a change; `selective` (Chavez and Kwarteng 1989) is a PCA of each layer's two dates, with the scores `<layer>_pc1` (what stayed) and `<layer>_pc2` (what changed) and no model. A cell with a layer missing at either date stays out. The template "Change between two dates by principal components" uses it.
- **A click on the chart of a `segment.optimize`'s scores chooses another value than the best** (`crates/app/src/charts.rs`, `crates/app/src/optimize_view.rs`): the candidate value nearest the pointer is marked by a second line, the panel says *Chosen: v* (with the best and *Back to the best*), and *Apply* writes the chosen value in one step of undo. The choice is dropped when another run changes the scores.
- **The SOM view has a button to the node's map, and a cell clicked again picks its node again** (`crates/app/src/builder.rs`, `crates/app/src/som_link.rs`, `crates/app/src/map_view.rs`): *Show the map of nodes* (*of groups* when the view colours by groups) shows the `nodes` (or `groups`) output of the `ml.som` node in the Viewer in one action; every click on a map's cell counts, so clicking the same cell after picking another node in the view picks that cell's node again.
- **`objects.join`, and the template "Classify objects by principal components of their attributes"** (`crates/ops/src/objects/join.rs`, `crates/app/src/templates.rs`): `objects.join` adds the columns of a table (ml.pca's components, ml.predict's scores, values of your own) to the objects' attributes, matched by `id` in any row order; an id that is no object's, an id twice or an object with no row is refused with the ids named, and a column the objects already have is refused unless `replace` is true. Everything that reads the attributes (objects.samples, objects.classify, the inspector) then sees them. The new template cuts the scene into objects, measures 29 attributes, reduces them to four components with ml.pca, joins them, and a random forest trained on clicked samples classes the objects by the components alone.

### Changed
- **`segment.region_growing`'s merge fetches the means of the neighbours whose nearest it will work out again, with the same objects** (`crates/ops/src/objects/growing.rs`): about 10-20% less time on one thread at 1000².
- **`segment.multiresolution`'s merging fetches the regions it will read ahead, with the same objects** (`crates/ops/src/objects/multiresolution.rs`): the neighbours whose cost is still to be worked out, and the lists of the neighbours of a merge, are prefetched together; about 10-20% less time on one thread at 1000².
- **`segment.multiresolution` has a `seam_band` setting (an experiment, default 0 = unchanged)** (`crates/ops/src/objects/tiles.rs`, `multiresolution.rs`): with tiles, above 0 only the cells of the pieces at a tile border that lie within `seam_band` cells of it are taken apart before the stitching, and the rest of each piece stays one region; the stitching is up to twice as fast.
- **The sequential parts around the multiresolution's tiles and the Gaussian smoothing are faster, with the same objects** (`crates/ops/src/objects/mod.rs`, `multiresolution.rs`, `tiles.rs`, `superpixels.rs`, `split.rs`): the neighbour lists and the id table of `Objects::indexed` are built in parallel, the costs of every pair, the sorting of the neighbour lists and the cached terms of the regions are worked out in parallel, the stitching visits only the cells beside the tile borders, `smooth` (SLIC, SNIC, Felzenszwalb, the watershed) no longer copies the image twice and takes a short way where its window is whole, and `objects.split` sorts the gradient once. The labels are bit for bit the same.
- **`segment.watershed` floods with a bucket queue when `compactness` is 0** (`crates/ops/src/objects/watershed.rs`): one bucket per distinct gradient value, first in first out, instead of a heap, and the gradient is sorted once for the markers and the flood. The labels are the same as before; the watershed is about 25% faster on one thread and 35% on eight.
- **The Sentinel-2 clip of the object-based image analysis series is read straight onto its UTM grid** (`website/figures/object-based-image-analysis/obia_data.py`, `obia_dates.py`, `large-images.py`, the pages of the series): one `data.stac` with `crs = "EPSG:31983"`, `extent` in metres and `cellsize = 10` replaces the `data.stac` in degrees followed by `raster.reproject` (one resampling less); the figures, tables, samples and the numbers quoted in 14 pages were made again, and the downloadable `sentinel2_brasilia.tif` too. The NAIP clip stays the published file, because its scene left the Planetary Computer.

## [1.61.0] - 2026-11-21

### Added
- **Space over the Viewer's Layers hides the layers selected, or shows them again** (`crates/app/src/layers_panel.rs`): with the pointer over the list and no text being written, the layers selected (several with ctrl and shift; the map shown's layer when none is) are hidden, and shown again when every one of them was hidden; a box that has the focus does not turn as well. Space over the map still hides the map. It is the shortcut `viewer_hide`, so a key changed in Settings (Keyboard) changes both.

### Changed
- **Terraflux opens with only the Files drawer at the window's left edge** (`crates/app/src/app.rs`, `crates/app/src/settings.rs`): the Operators drawer is off unless **the operators in a drawer at the left edge** is turned on in the settings (`operators_drawer`); the operators stay in the Operators menu, behind "/" and the commands (ctrl+K), and their pane in the panes' menu. The Prepare drawer still comes with the first data. The site's pages on the window, the canvas, the Viewer, the shortcuts, the tour and the first raster say so.

## [1.60.0] - 2026-11-21

### Added
- **Cleaning and thinning of tracks, from move2utils: `move.max_speed`, `move.flag_speed`, `move.filter_gps_quality`, `move.thin_time` and `move.thin_distance`** (`crates/ops/src/movement/clean.rs`; the C++ is the library's `movecore`, bound in `crates/ls-sys`: `move_v_phys`, `move_speed_cap`, `move_thin_time`, `move_thin_distance`), the same numbers as the library's `tf.move` and as R's move2utils 0.4.6 (Kranstauber, Safi and Scharf 2026, bioRxiv doi:10.64898/2026.07.07.736908; Hirt et al. 2017, Nature Ecology & Evolution 1:1116-1122, doi:10.1038/s41559-017-0241-4). `move.max_speed` (`mass` in kg, `mode` flying, running or swimming, `ci_level`) gives the allometric maximum speed, the interval of the parameter uncertainty and the speed in km/h as the numbers `v_max`, `lower`, `upper` and `kmh` (m/s) and a one-row `table`, with a warning outside the data of Hirt et al. (3e-8 to 1.084e5 kg). `move.flag_speed` is the hard cap of `mt_flag_speed_cap`: the limit is `v_max` (m/s) or `mass` and `mode` (both needed), a location is flagged when the step leaving it or the step arriving at it is faster (haversine on a sphere of 6 371 008.8 m in longitude / latitude, the CRS's units otherwise), locations without a time are left out of the track and steps with no positive lag have no speed; `jitter` also flags the steps shorter than it, `remove` gives only the locations not flagged, and the columns `is_outlier`, `step_speed`, `is_speed_above_cap`, `is_step_below_jitter`, `flagged_by_speed` and `loglr_speed` (1, 0 or NaN for a missing time) and the output `v_max` are those of the library. `move.filter_gps_quality` drops the locations with fewer than `sat_min` satellites (5), a dilution of precision above `dop_max` (10), an accuracy estimate above `hacc_max` (100) or no position (`drop_empty`), the columns found by Movebank's names (`gps:satellite-count`, `gps:dop`, `eobs:horizontal-accuracy-estimate`, also with underscores or dashes; move2utils itself finds only the other two forms, so it filters nothing from a Movebank file); a limit of `none` skips its criterion, as a column that is not there, and a missing value or text that is not a number does not drop a location. `move.thin_time` (`interval`, `tolerance` a tenth of it, `criterion` closest or first; seconds or text such as `45 mins`) and `move.thin_distance` (`distance` in metres, `method` step or interval, `tolerance`, `criterion`; the steps are `move.distance`'s) keep the longest chain of locations whose successive lags (or travel) are within the interval +/- the tolerance, by a linear dynamic programme; it is not `move.filter_per_interval`. `output` is `selected` (the locations kept, the default) or `flag` (all of them, with the column `thin_selected`). The tracks run in parallel under the central cap and the result is the same on 1 and 4 threads. In the app: the sentences (`phrases.rs`), the advice (`tips.rs`), the methods paragraph with Hirt et al. and move2utils cited (`methods.rs`), `essentials.rs` for `move.flag_speed`, and the Python script's calls (`tf.move.v_phys_estimate`, `flag_speed`, `filter_gps_quality`, `filter_thin_time`, `thin_time` and the same for distance). Tests in `crates/ops/tests/movement.rs` against `tests/data/reference/move.npz` and `move.json` (written by `tools/make_move_reference.py` from the library: fishers, move2utils' synthetic tracks `tests/data/move/m2u_synthetic.csv.gz` and the library's cleaning sets; the older keys are unchanged), bit for bit, the errors, and 1 and 4 threads on 56,000 locations.
- **The values along each step: `move.extract_covariates` with `where = along`** (`crates/ops/src/movement/ssf.rs`; the C++ is the library's `movecore` (`move/along.cpp`), bound in `crates/ls-sys` as `move_along_cells` and `move_along_stats`), the same numbers as the library's `tf.move.along_summary` and `extract_covariates_along`, and as amt's `extract_covariates_along` (Signer, Fieberg and Avgar 2019, Ecology and Evolution 9(2):880-890) as R's terra 1.9-50 gives it (`terra::extract` of the line from the start to the end of the step with its defaults, every cell the line touches, GDAL's ALL_TOUCHED line walk on the grid terra crops to the line; the coordinates go through 15 significant digits first, as amt's `paste0` does; a step that touches no cell takes the cells of its two ends and a step outside the raster gives one missing value). The new setting `stats` (a list or a comma text of `mean`, `min`, `max`, `ascent`, `descent`, `delta`; default all six) adds, for each raster (or layer), the columns `<layer>_along_<stat>` to the steps: the mean (as R's `mean(x, na.rm = TRUE)`), minimum and maximum of the cells with data, the ascent and the descent (with the cells in the order along the step, the sum of the rises and the sum of the falls between successive values, as a positive number; cells with no data are skipped) and the delta (`|last - first|`), which tell a step that climbs straight up (`ascent = delta`) from one that follows a contour line (both near 0), what a step selection function with terrain needs (a negative coefficient on the ascent: the animal follows the contour lines). The new optional output `along` is the table of the values themselves as amt gives them: `step` (the step's number from 1, as amt's ID) and a column per raster, a row per cell in terra's order (by cell number, not along the step), a step outside the raster with one row of missing values. The rasters must share one grid (north up, not rotated), and the grid passed to the kernel is the raster's own geotransform, not one recomputed from the cell centres. The steps run in parallel under the central cap and the result is the same on 1 and 4 threads. In the app: the sentence (`phrases.rs`), the advice (`tips.rs`: what the ascent and descent mean, and which cells are read) and the Python script (`tf.move.along_summary` with the stats, `tf.move.extract_covariates_along` for the `along` output). Tests in `crates/ops/tests/movement.rs` against `tests/data/reference/move.npz` and `move.json` (the keys `along/...` and `in/along/...`, written by `tools/make_move_reference.py` from the library's `along_track_sets`: edges and corners of the cells, lines within 0.01 cell of an axis, zero-length and outside steps, 15-digit coordinates, UTM-sized coordinates, an origin that is not a multiple of the cell, random steps and the fishers' 32,896 steps; the older keys are unchanged), bit for bit, the errors and 1 and 4 threads.
- **Home ranges of tracks, from amt: `move.home_range`, `move.home_range_overlap` and `move.intensity_use`** (`crates/ops/src/movement/homerange.rs`; the numbers are the library's `movecore` (`move/homerange.cpp`), bound in `crates/ls-sys`: `move_hr_chull`, `move_hr_quantile7`, `move_hr_bkde2d`, `move_hr_dpik`, `move_hr_lscv`, `move_hr_contour_lines`, `move_hr_locoh_neighbours`, `move_hr_cud`, `move_hr_vol_base` and the small R helpers; the polygons' hulls, buffers, unions and intersections are GEOS's, through `crates/gdal/src/geos.rs`, which now also builds polygons, takes the convex hull and the buffer with `quad_segs` of a set of points, the union of polygons and the intersection of two shapes), the same numbers as the library's `tf.move.hr_mcp`, `hr_kde`, `hr_kde_ref`, `hr_kde_pi`, `hr_kde_lscv`, `hr_kde_ref_scaled`, `hr_locoh`, `hr_isopleths`, `hr_area`, `hr_ud`, `hr_overlap` and `intensity_use` and as R's amt 0.3.1 (Signer, Fieberg and Avgar 2019, Ecology and Evolution 9(2):880-890, doi:10.1002/ece3.4823; Worton 1989, Ecology 70(1):164-168; Getz et al. 2007, PLoS ONE 2(2):e207; Fieberg and Kochanny 2005, Journal of Wildlife Management 69(4):1346-1359; Sheather and Jones 1991, Journal of the Royal Statistical Society B 53(3):683-690, whose plug-in bandwidth KernSmooth's `dpik` is). `move.home_range` estimates each track alone (a track column by its setting or its usual names, or one track; projected coordinates, longitude / latitude is refused with the way to project them) by `method` `mcp` (the hull of the locations whose squared distance to the mean is at most the quantile of each level, R's `chull`), `kde` (the KernSmooth `bkde2D` estimate with R's own fft on the cells of `like` or of one template made from all the locations, amt's `make_trast` with `factor` and `res`; `bandwidth` `ref`, `ref_scaled` (`num_parts`), `pi`, `lscv` (`which_min`), a number or a list `[x, y]`, `rescale` `none`, `unitvar` or `xvar`; the isopleths the contour lines of the cumulative distribution, closed and united, an error where a contour reaches the edge of the template) or `locoh` (`type` `k`, `a` or `r`, `n`, `rand_buffer`; the hull of each location and its neighbours, buffered, taken from the smallest and united until they hold each level). Outputs: `polygons` (a feature for each track and level, from the highest level down as amt lists them: the track column, `level`, `what` and `area`, GDAL's OGR ring sums), `table` (the same without the shapes) and, for `kde`, `ud` (a band for each track named by it, the cells summing to 1) and `bandwidths` (`hx` and `hy`). `move.home_range_overlap` takes the `polygons` (type `hr`: the share of the first track's polygon the second covers, at each level) or the `ud` (`phr`, `vi`, `ba`, `udoi`, `hd`, with `conditional` on each level) of `move.home_range` and gives the table of the pairs (`which` `consecutive`, `all` or `one_to_all`): `from`, `to`, `level` and `overlap`; the UD is rescaled to sum to 1 again there, so the distribution indices agree with the library's to 1e-12 relative, not to the bit. `move.intensity_use` is the length of each track over the square root of the area of its 100 % polygon. The polygons, areas, bandwidths and UDs of MCP, KDE and LoCoH and the overlaps of polygons are bit for bit the library's (the tests ran with GDAL's GEOS 3.14.1 against the library's shapely, GEOS 3.13.1; another GEOS may differ in the last digits of the unions and buffers). The tracks' work runs under the central cap and the results are the same on 1 and 4 threads. In the app: the sentences (`phrases.rs`), the advice (`tips.rs`: which bandwidth when, the outliers of the MCP, LoCoH's k, the template that must hold the isopleths), the methods paragraph and the citing page with the six papers (`methods.rs`; each checked on Crossref), `essentials.rs`, and the Python script (`tf.move.hr_mcp`, `hr_kde` with `hr_kde_pi`, `hr_kde_lscv`, `hr_kde_ref_scaled` or `hr_kde_ref` for the bandwidth and `make_trast` for the template, `hr_locoh`, `hr_isopleths`, `hr_area`, `hr_ud`, `hr_overlap`, `intensity_use`). Tests in `crates/ops/tests/movement.rs` against `tests/data/reference/move.npz` and `move.json` (the keys `hr/...` and `in/hr/...`, written by `tools/make_move_reference.py` from the library's `hr_track_sets`: UTM, no CRS, three small tracks, the fishers' first 150 locations each and degenerate tracks; the older keys are byte for byte unchanged), the errors and 1 and 4 threads; the reference data grew by 0.9 MB.
- **The dynamic Brownian bridge movement model and the directional bridge, from move2utils: `move.dbbmm_variance`, `move.dbbmm_ud`, `move.dbgb_variance`, `move.dbgb_ud`, `move.suggest_window`, `move.mask_segments`, `move.ud_volume` and `move.ud_outer_probability`** (`crates/ops/src/movement/ud.rs`; the C++ is the library's `movecore` (`move/bridge.cpp`, `move/ud.cpp`, `move/locerr.cpp`), bound in `crates/ls-sys` as `move_dbbmm_variance`, `move_dbgb_variance`, `move_impute_loc_err`, `move_ud_probe`, `move_ud_cells` and `move_ud_volume`), the same numbers as the library's `tf.move.dbbmm_variance`, `dbbmm_ud`, `dbgb_variance`, `dbgb_ud`, `suggest_dbbmm_window`, `mask_segments`, `ud_volume` and `ud_outer_probability` and as R's move2utils 0.4.6 (Kranstauber, Safi and Scharf 2026, bioRxiv, doi:10.64898/2026.07.07.736908) on the C kernels of the `move` package (Kranstauber et al. 2012, Journal of Animal Ecology 81(4):738-746, doi:10.1111/j.1365-2656.2012.01955.x; the bridges of Horne et al. 2007, Ecology 88(9):2354-2363; the directional bridge of Kranstauber, Safi and Bartumeus 2014, Movement Ecology 2:5, doi:10.1186/2051-3933-2-5). `move.dbbmm_variance` and `move.dbgb_variance` put the motion variance in columns of the tracks (`motion_variance`, `in_windows`, `interest` and the `location_error` used; `para_sd`, `orth_sd`, `n_estim`, `seg_interest` and `dbgb_location_error`): a window of `window_size` locations slides along each track and a BIC test keeps one break point of the variance (of each axis, for the dBGB) per window; `location_error` is a number, a column (Movebank's names with colons too) or `auto` (the e-obs estimate, else Argos' classes), its missing values filled by `location_error_na` (`median`, `mean`, `zero`, `approx`); a track shorter than the window is skipped with a warning (an error when it is the only one); the bursts of `move.resample` are tracks of their own (`burst`); longitude / latitude is projected to the corrected `move.aeqd_crs` (the centroid's) for the computation. `move.dbbmm_ud` and `move.dbgb_ud` give `ud`, a stack with a band for each track (or burst, `track__burst`), each summing to 1, from the tracks with or without those columns (a `location_error` given then replaces the errors they hold) on one grid: the raster `like` (never enlarged: the error says how large it has to be), or the box of the locations widened by `ext_factor` in cells of `res` or `dim_size` cells along its longest side, enlarged when the bridges' windows do not fit (told with `verbose` only, as move2utils does; `max_cells`); `time_step` (minutes); `crs` `projected` (in the CRS of the tracks if projected in metres, else in the local projection centred on the middle of the ranges of longitude and latitude) or `input` (warped back with GDAL's bilinear, the negative cells made 0 and every band summed to 1 again: tested by tolerance). `move.suggest_window` gives the window and margin from the sampling (a table); `move.mask_segments` leaves bridges out of the distribution (`segments`: a list, or an object by track; `gap` in hours; `column`); `move.ud_volume` the cumulative probability of the cells of a stack from the largest down, with `levels` also the contours as polygons through the home ranges' isopleths; `move.ud_outer_probability` the volume at the points (a column). The variance, the grid, the distributions and the masks are bit for bit the library's (a test for each of 20 variance cases and 26 distributions, with the enlarged grids, the templates, the time step, the bursts, Argos and e-obs errors and the dBGB's cases; the projected coordinates of longitude / latitude tracks are the library's to the last bit here, the tests allow a tolerance for another PROJ), the tracks or bursts run side by side, one thread each, when there are as many as threads and the results are the same on 1 and 4 threads. In the app: the sentences (`phrases.rs`), the advice (`tips.rs`: the window and the margin, the location error, the grid and its cost, the dBGB's larger templates, the contours 0.5 and 0.95), the methods paragraph and the citing page with Horne et al. (2007) and Kranstauber et al. (2012, 2014) (`methods.rs`; each checked on Crossref), `essentials.rs`, and the Python script (`tf.move.dbbmm_variance`, `dbbmm_ud`, `dbgb_variance`, `dbgb_ud`, `suggest_dbbmm_window`, `mask_segments`, `ud_volume`, `ud_outer_probability`). Tests in `crates/ops/tests/movement.rs` against `tests/data/reference/move.npz` and `move.json` (the keys `ud/...` and `in/ud/...`, written by `tools/make_move_reference.py` from the library's `ud_track_sets` and the fishers' M1 on coarse grids; the older keys are byte for byte unchanged), the errors and 1 and 4 threads; the reference data grew by 0.4 MB.
- **Corridors in tracks and the distance between utilisation distributions, from move2utils: `move.corridor` and `move.ud_distance`** (`crates/ops/src/movement/corridor.rs`; the C++ is the library's `movecore` (`move/corridor.cpp`, `move/emd.cpp` and Rubner's solver of R's emdist, MIT, in `move/vendor/emdist`), bound in `crates/ls-sys`: `move_corridor_neighbourhoods`, `move_corridor_flags`, `move_ud_prepare`, `move_emd_sinkhorn`, `move_emd_exact`), the same numbers as the library's `tf.move.corridor`, `corridor_lines` and `ud_distance` and as R's move2utils 0.4.6 (Kranstauber, Safi and Scharf 2026, bioRxiv doi:10.64898/2026.07.07.736908; LaPoint et al. 2013, Landscape Ecology 28(8):1615-1630; Rubner et al. 2000, International Journal of Computer Vision 40(2):99-121; Cuturi 2013, Advances in Neural Information Processing Systems 26:2292-2300). `move.corridor` marks the segments (each location to the next of its track) that are fast (`speed_threshold`, m/s) and whose neighbours go the same way: the neighbours are the segments, of all tracks together, whose midpoints lie in GEOS' 120-gon buffer of its own midpoint of half the segment's length, the circular variance of their pseudo-azimuths is at most `circvar_threshold`, and it has at least `min_segments` neighbours (2, itself included) and strictly more qualifying than not. A threshold left out is the 0.75 quantile of the speeds or the 0.25 quantile of the variances (R's type 7), with a warning; longitude / latitude is projected to the local azimuthal equidistant projection of `move.aeqd_crs`; empty locations are left out; a warning for a very irregular sampling interval. The columns `corridor` (`corridor` or `not corridor`), `corridor_speed`, `corridor_azimuth`, `corridor_circvar` and `corridor_n_neighbours` are added to the tracks, the outputs `lines` (the corridor segments as two-point lines), `speed_threshold`, `circvar_threshold` (the ones used) and the one-row table `thresholds` come with them. `move.ud_distance` is the earth mover's distance between UDs on one grid, the bands of a raster (the UDs of several tracks) or `ud` and `ud_b`: each UD is cut to the cells inside its `mask_quantile` volume contour (0.999), then `method` `sinkhorn` (default; `reg` 0.05, `max_iter` 500, `tol` 1e-9) or `exact` (Rubner's solver in single precision, Euclidean, `exact_max_iter` 500, memory that grows with the square of the cells), with `gc` for great-circle distances in metres on longitude / latitude grids (a warning without it) and `threshold` to clip the cost (Sinkhorn only). The output `table` has a row per pair of bands (`a`, `b` named by the bands, `distance`), and `distance` is the number when there are two UDs. Both are bit for bit the library's (`tests/data/reference/move.npz`, `tools/make_move_reference.py`: 24 corridor cases from the lattices, the fishers' M1, a longitude / latitude set and a projection in feet, and 18 distance cases on the fishers' UDs and on blobs on a longitude / latitude grid), with 1 and 4 threads giving the same bits. In the Python script they are `tf.move.corridor` (and `corridor_lines`) and `tf.move.ud_distance`; `website/data/python_api.json` has them.
- **Does the use of the animals follow a connectivity surface? `move.validate`** (`crates/ops/src/movement/validate.rs`; the library's `tf.move.validate`, not in R: the definitions are the library's and the numbers are the same bit for bit, 22 cases of UDs, points and corridor lines, a current, a resistance and a binary surface, in `tests/data/reference/move.npz`), after Zeller et al. (2018). The use is a UD (a band per track, read on the surface's grid or by nearest cell), points or lines (`move.corridor`'s `lines`); the surface is a current, a resistance or a least-cost corridor (`kind`); the corridors are the cells past a `threshold` or the top `quantile`. The table has a row per track and pooled: Spearman's rank correlation between the use and the surface (average ranks, `support` volume contour for a UD), the share of the use inside the corridors, the share of the compared area they are, their ratio, the mean surface used and available and the coverage; the outputs are also a `note` in words, the `threshold` and the `corridor` raster. The home ranges' polygons, the UD contours of `move.ud_volume` and the UD bands of `move.home_range` are tested to go on as they come out into `metrics.sample` and `vector.zonal` (zones), `vector.rasterize` and `prioritize.optimize` (features); `move.corridor`'s lines need `vector.buffer` before `metrics.sample`.
- **Movement: three templates, the guide `movement` and the method's advice** (`crates/app/src/templates.rs`, `guide.rs`, `advice.rs`). New group *Movement*: "Home range of a track" (the MCP, the kernel density and the dynamic Brownian bridge with its 50 % and 95 % contours, the areas as bars), "Observed corridors against current" (the dBBMM and `move.corridor` against Omniscape's current with `move.validate`; the resistance is marked as data for Use my data) and "Cleaning a track" (`move.filter_gps_quality`, `move.flag_speed` with the limit of `move.max_speed`, `move.thin_time`); each runs as it is on a simulated track. The guide `movement` (for the MCP's `guide` tool, listed in its instructions) takes the method from a Movebank file to the validation of a connectivity surface, with the operators, the templates and the pitfalls. check_model advises on the dBBMM and the dBGB without `location_error`, a window that is even, smaller than 2 margins + 1, without room or short, a UD grid of millions of cells, a sampling interval nothing in the model looks at, and `move.flag_speed` without a limit (or with two) or before the GPS quality is filtered.

### Changed
- **The website's reference for the movement block**: the Python API page "Animal movement" documents the cleaning, along, home range, bridge, corridor, distance and validation functions; the accuracy table has seven new comparison groups (54 in all; against amt 0.3.1, move2utils 0.4.6, move 4.2.7 and emdist) with `website/tools/make_accuracy_view.py`, which makes the three views of the table; the speed page has an "Animal movement" section with the times of the dynamic Brownian bridges against R (measured in the library, not in the application).

### Fixed
- **The instructions of `terraflux mcp` name the guide `multivariate`** (`crates/app/src/tools.rs`): it was missing from the list of guides there, and the test of the instructions failed without it; the new `movement` guide is listed too.
- **`vector.rasterize` refuses a degenerate grid instead of crashing the process** (`crates/ops/src/vector.rs` `check_grid`, `crates/gdal/src/lib.rs`; test `crates/ops/tests/rasterize_grid.rs`): a map with no columns or rows made GDAL's rasterizer (which burns the lines) divide by zero (`STATUS_INTEGER_DIVIDE_BY_ZERO`); a size beyond 32 bits overflowed, and a geotransform that is not finite or with a cell of size 0 burnt nothing without a word. Now an error names the problem (no cells, too large, not finite, a cell of size 0), also for the other operators that burn features (objects, services, protected-area connectivity, species distribution).
- **The gamma draws of `move.random_steps` and `move.random_numbers` are thread-safe**: R's `rgamma()` in the library's `movecore` kept its cached constants in statics and drew from the caller's generator through a global pointer, so two draws at once (two models running, or tests in parallel) used each other's generator or a null one and crashed (STATUS_ACCESS_VIOLATION, about one parallel run of the movement tests in twenty). Both are per thread now (terraflux-py 51acd76); the draws are the same. Test: `gamma_draws_on_several_threads_are_the_serial_ones`.

## [1.59.0] - 2026-11-20

### Added
- **Three templates for the new object tools** (`crates/app/src/templates.rs`, group *Objects (OBIA)*): "Classes by parcel" (segmentation held to parcels with `bounds`, `thematic` in `objects.features`), "Separate tree crowns" (`objects.split` by the surface of a height model, `objects.fuse`, `objects.extrema` for the tops) and "Roads and rivers by the skeleton" (the `skeleton` group and rules on `skel_length_width` and `skel_width`); each runs as it is on a synthetic scene.
- **The shape by the skeleton: the `skeleton` group of `objects.features`**, as eCognition's skeleton features (`crates/ops/src/objects/skeleton.rs`), asked for by name (`features`: `skeleton`, or a column such as `skel_main_length`; not in `basic`, and `all` has it). Per object, in its box with a pad of one cell (`each.rs`), the cells are thinned by Zhang and Suen's algorithm (1984, CACM 27(3):236-239) over a list of border cells, so the work follows the outline and not the area; the cells that can go without changing the topology (staircase steps, two-cell wide diagonals) are taken out, and the branches that end in a junction after fewer than `spur` cells (the new setting, default 3, 0 keeps them) are pruned. The graph of the skeleton's cells (an end has one run of neighbours, a junction three or more) gives `skel_main_length`, the main line as the graph's diameter by two Dijkstra searches in map units (a diagonal step is the square root of 2 cells, a cell that is not square is respected), `skel_width` (area / main length), `skel_length_width`, `skel_branches` (the segments between ends and junctions; a ring counts one), `skel_ends`, `skel_junctions` (touching junction cells are one), `skel_branch_length` (mean) and `skel_curvature` (the turning of the main line in radians per map unit of its length: the line is simplified at one cell by Douglas and Peucker's algorithm, so the raster staircase of a slanted straight line is not turning, and the angles at the vertices are summed; a straight bar at 0, 25, 45 and 63 degrees gives 0 and an arc of radius r gives 1/r within 3 %) with `skel_curvature_sd`. The skeleton is of the raster, not of the polygon eCognition triangulates, so the values differ a little. Objects run in parallel under the central cap, the biggest first, each thread reusing its buffers; the same numbers on 1 and 8 threads. Reference: Zhang and Suen 1984 (cited in the operator's description; no entry in `methods.rs`, which matches methods by operator and would cite it for every `objects.features`). Tests: `crates/ops/tests/objects_skeleton.rs`; bench `objects_skeleton_bench` (touching blobs: 1000 x 1000 in 0.01 s, 2000 x 2000 in 0.05 s on one thread and 0.01 s on 8).
- **`data.stac` reads every band of a multi-band asset, and gives the grid in a chosen CRS** (`crates/ops/src/stac.rs`). An asset of several bands (NAIP's `image`: red, green, blue, nir; orthophotos; Planet) is a layer per band, named by the item's `eo:bands` (else `raster:bands`) as the common name, else the name, else `<asset>_<number>` (`image_1`...); the new setting `bands` (names, any case, or numbers from 1, a list or a comma-separated text) chooses which, in the order asked. An asset of one band is as it always was (one layer named by the asset, the same values, the same cache), and `bands` does not touch it. The new setting `crs` (EPSG code, WKT or PROJ string) gives `extent` and `cellsize` in that CRS (metres for UTM) and the scenes are warped straight onto the grid, one resampling instead of `data.stac` and `raster.reproject`; the search box is still made in longitudes and latitudes. Without `crs` (and with `like`) nothing changes. Tests read local files, no network.
- **Joining objects by the result: `objects.fuse`**, as eCognition's image object fusion (`crates/ops/src/objects/fuse.rs`; `regroup` of `levels.rs` is `pub(super)` now). A seed joins a neighbour only when the object they make is good by `fitting`, an expression over the joined object written as `objects.calc`'s: `compactness`, `shape_index`, `border_index`, `length`, `width`, `length_width`, `area`, `cells`, `perimeter`, and with the `image` `mean_<band>` and `sd_<band>`. These are derived, not measured again from the cells: each group keeps its cells, its perimeter (the two parts' less twice the border they share), its box and per band the count, sum and sum of squares, so a pair is evaluated in constant time (the box's sides stand for the length and the width, a close approximation of the rotated rectangle of `objects.features`). Anything else (texture, skeleton, context, the neighbours) is refused with the list of what works. `goal` is `max` (the default) or `min`; `improve` (the default) asks the joined object to be as good as the seed's own value (a tie joins, so pieces of a rectangle make the rectangle); `threshold` is the value it must reach; `seeds` and `candidates` are class names (`unclassified` counts), the result has the seed's class. A pass evaluates the fitting in batch over a table of pairs (in parallel), each seed proposes its best candidate (`mode` `best`, the default) or all that qualify (`all`), the proposals are taken best first and a pair only when neither object has joined in the pass (disjoint pairs), and it repeats until nothing joins or `max_passes` (default 100). The level above is dropped with a warning and the table is made again from the `image`; outputs `objects`, `table`, `report` (the joined objects and their parts), `count` and `done`. The same objects on 1 and 8 threads. The passes only look again at what the last one changed (a pair's value, a seed's own value and its proposals are kept; a join dirties the group it makes and its neighbours), the joins of a pass run in id order and the proposals in parallel, with the same objects as a full re-evaluation: 2000x2000 "alike" 0.83 s to 0.61 s on one thread and 0.88 s to 0.49 s on 8. Tests: `crates/ops/tests/objects_fuse.rs`; bench `objects_fuse_bench`.
- **The local extrema of an attribute: `objects.extrema`**, as eCognition's find local extrema (`crates/ops/src/objects/extrema.rs`). `attribute` is a column of the objects' table or an `objects.calc` expression (`mean_nir`, `nb.mean(ndvi)`), `kind` is `max` (the default) or `min`, and an object is an extremum when no other candidate near it has a larger (smaller) value. Near is `steps` objects across the neighbours (default 1: the adjacent objects, a breadth-first search over the adjacency) or `radius` map units between centroids (a grid of buckets as wide as the radius). `among` limits the classes compared and marked, `strict` (the default; eCognition's exclude equal) makes a tie no extremum, so a flat top gives none, and objects with no value for the attribute are not compared. The extrema take the class `class` (default `maximum` or `minimum`; made when the objects have none of the name), the other objects keep their classes and levels, and the table has the column `extremum` (1, 0, or missing for an object not compared). The objects run in parallel under the central cap and the result is the same on 1 and 8 threads. eCognition's find enclosed by class has no operator: `rel_border_<class> == 1` (`objects.features`, context) or `border_to("<class>") == 1` (`objects.calc`), and for several classes the sum of their borders; the guide, the tip and the from-eCognition tutorial say so. Tests: `crates/ops/tests/objects_extrema.rs`; bench `objects_extrema_bench`.

### Fixed
- **`objects.accuracy` weights the sampled objects by their area** (new setting `design`; `olofsson_objects` in `crates/ops/src/accuracy.rs`, the operator in `crates/ops/src/objects/accuracy.rs`). The area-adjusted estimates (adjusted areas, user's, producer's and overall accuracy by area) gave every sampled object the weight of one of Olofsson et al.'s pixels, which are equal-area units; with objects of very different sizes drawn with the same chance per object (as `objects.sample` does) the areas and the producer's accuracies were biased (a NAIP case: 3.3 ha of water against about 0.7 ha of ponds). `design = objects` is the ratio estimator for units of unequal size (Radoux and Bogaert 2017, Remote Sensing 9(7):646; Stehman 2014, IJRS 35(13):4923-4939): in a stratum (a mapped class) the share of reference class j is R_hj = sum(a_u 1[ref_u = j]) / sum(a_u) over its sampled objects, the area of j is sum_h W_h R_hj with W_h the stratum's share of the area, and the variance of R_hj is the linearised ratio variance of a simple random sample (Cochran 1977, ch. 6), (1 - n_h/N_h) / (n_h abar_h^2) * sum(e_u^2) / (n_h - 1) with e_u = a_u (1[ref_u = j] - R_hj) and abar_h the stratum's mean object area (known); the strata being independent, V(area_j) = sum_h W_h^2 V(R_hj), V(overall) = sum_h W_h^2 V(R_hh), V(user_j) = V(R_jj), and the producer's accuracy by the delta method (Olofsson et al.'s formula 9 with these variances). `design = area` is what the operator did (each sampled object counts once; right when the reference comes from points at random over the map, where an object's chance is its area) and gives the same numbers bit for bit. Without `design`, a table or polygons are `objects`, and points are `area` when an object holds several of them and `objects` otherwise, with a warning that says it is a guess. The confusion matrix, the accuracies by objects and kappa are by objects, as before. Tests in `crates/ops/tests/objects_accuracy.rs`: over 300 seeds, with objects of 1 and 9 cells (true water 660 of 4200 cells), a sample with the same chance per object is unbiased with `objects` (the 95% interval covers the truth 89 to 99.5% of the time) and gives about 0.34 with `area`; a sample proportional to the area, the reverse.
- **`objects.resize` without an `image` gives a table**: the resized objects had an empty attribute table, so `objects.to_vector` after it wrote polygons with only the class; now the table has id, cells and area (the class is written by `objects.to_vector` itself), and with an `image` it is the band statistics as before. Test: `resize_without_an_image_has_the_basic_table`.
- **`terraflux methods` cites the classifier of `ml.fit`**: random forest learning (Breiman 2001, Machine Learning 45(1):5-32) by default, gradient boosting of trees (Friedman 2001, Annals of Statistics 29(5):1189-1232) when its `method` is `gradient_boosting`, and no paper when `method` is a model input; `objects.accuracy` also cites Radoux and Bogaert (2017) and Stehman (2014). `crates/app/src/methods.rs` (`method_for` reads the node's `method`). Test: `ml_fit_cites_the_algorithm_it_fits`.
- **`objects.fuse` measures length, width and compactness by the rotated minimum-area rectangle, as `objects.features` does** (`crates/ops/src/objects/fuse.rs`). It used the axis-aligned box, so the compactness of crown pieces was higher than in `objects.features` (median 1.72 against 1.59 over 1,993 pieces) and `improve` with compactness almost never joined watershed pieces. Each group now keeps the convex hull of its cells' corners; the joined object's hull is the hull of the two, and the rectangle comes from the same rotating calipers (the numbers equal `objects.features` on the result, within 1e-9). `improve` off with no `threshold` now warns that nothing limits the joins. The "Separate tree crowns" template's note names the rotated rectangle.
- **`skel_curvature` no longer counts the raster staircase of a slanted line as turning** (`skeleton.rs`): see the skeleton entry under Added. The help of `objects.features` and its tips say that `skel_width` is inflated on a branching network and that `skel_width` and `skel_length_width` are missing (read as 0 by a rule) for one-cell objects and main length 0.
- **Segmentation by tiles leaves no seam** (`segment.multiresolution` and `segment.region_growing` with `tile`, and the cluster's `TileJob` path, which shares the code). Each tile's window segmented the area around a tile border on its own, so the pieces on the two sides were unrelated partitions cut by a straight line, and the second merging joined only pieces whose merge cost was under the scale (the cost of two big pieces grows with their size, however alike): the border stayed an object edge. Measured as the share of pairs of neighbouring cells in different objects, 71 % across a tile border against 10 % along other lines on a 580 x 580 Sentinel-2 clip (22 % for region growing; a bigger `margin` did not help), and 0.438 against 0.268 on 2000 x 2000 x 4 of the synthetic image; IDEAS.md's "0.98 of the borders equal" was the share of cells with the same border state, which is near 1 for any large objects and measured nothing. Now the pieces that touch a tile border are taken apart into single cells before the second merging (`dissolve_at_borders` in `crates/ops/src/objects/tiles.rs`; about 5 % of the cells), which grows them across the border as the merging does anywhere: 0.273 on the borders against 0.267 elsewhere, the object count and the boundaries' F against the untiled run within what another seed gives. Cost: the single-thread stitching has about 5 % more regions, 3.7 s to 4.4 s at 4 threads (2000 x 2000 x 4, tiles of 512) and the one-thread peak 353 to 480 MB; the untiled result is unchanged. Tests: `tiled_multiresolution_has_no_seam`, `tiled_region_growing_has_no_seam` (they fail without the change: 0.47 and 0.38 on the borders against 0.25 elsewhere), and the ignored `seam_report` and `seam_bench` in `crates/ops/tests/objects_tiles.rs`.
- **`objects.grow` keeps the class that grows**: a neighbour merged in took the merged object to the class of its biggest part (`regroup`), so an unclassified neighbour larger than the grown object ate the class; now each merged object keeps the class and the memberships of the grown object, as the operator says. Test: `grow_keeps_the_class_when_the_neighbour_is_bigger`.
- **`segment.sam` and `objects.embed` use the run's threads**: the ONNX Runtime session took `threads::most()` (15 on 20 cores) intra-op threads whatever `--threads`, `TERRAFLUX_THREADS` or the app's setting said; now `session_threads` in `crates/ops/src/objects/onnx.rs` gives the size of the rayon pool the run is in (the process's, set by the central cap, outside any pool), never more than the cap. Test: `session_threads_follow_the_pool`.
- **`ml.kmeans`, `ml.som` and `ml.pca` accept a constant column when standardising**, as scikit-learn's StandardScaler: it is centred, not divided (scale 1), weighs nothing (a zero row and column in the correlation, a zero loading) and a warning names the columns; before, the run was refused (the embeddings of `objects.embed` often have all-zero columns). Inputs without a constant column give the same numbers bit for bit; the results equal those without the column. `metrics.pca` is another code path and is untouched. Tests: `*_standardises_a_constant_column_as_scale_one_and_says_it` in `crates/ops/tests/ml.rs`.
- **`segment.felzenszwalb` enforces `min_size`** (`crates/ops/src/objects/superpixels.rs`): with 8 neighbours (the default) a component joined only by corners passed the size check whole and was then split into 4-connected pieces by the objects' construction, leaving pieces of 1 cell; on a 580 x 580 Sentinel-2 clip 2,408 of 4,063 objects were under 20 cells. Now the components are cut into their 4-connected pieces before the second pass, which joins those under `min_size` along 4-neighbour edges, so no object has fewer cells than `min_size` unless the whole image, a zone of `bounds` or an area cut off by cells without a value is smaller. 4 neighbours give the same objects as before. On the 1000 x 1000 x 4 benchmark image: 4,621 objects in 0.39 s (1 thread) and 0.24 s (8) before, 833 in 0.37 s and 0.22 s after (2000 x 2000: 19,940 to 3,601 objects, 2.24 s to 2.07 s on 1 thread). Test: `felzenszwalb_enforces_min_size_except_in_cut_off_areas` in `crates/ops/tests/objects.rs` (4 and 8 neighbours, 1 and 8 threads, bounds).
- **`th_share` of `objects.features` counts all the object's cells** (`crates/ops/src/objects/features.rs`): it was the share of the cells that have a value in the thematic layer, so an object half in a parcel and half outside every feature had `th_share` 1. Now cells with no value count as none: `th_share` is the share of all the object's cells in `th_mode`, and when none is more common than every value (a tie goes to the value) `th_mode` and `th_share` are missing (NaN). Objects wholly inside a zone, or segmented with the same layer as `bounds` over a layer without holes, are unchanged. The docs and the template "Classes by parcel" say it. Test: `cells_outside_the_layer_count_as_none_in_th_share` in `crates/ops/tests/objects_thematic.rs`.
- **Missing values in `objects.calc` and `objects.rules` are documented**: a comparison or arithmetic with a missing value gives missing, a rule reads missing as 0, `isnull(x)` tests it and `coalesce(x, y)` fills it (the operators' descriptions, their tips and the guide topic `calc`).
- **`objects.resize` cites Soille 2004** (Morphological Image Analysis, 2nd ed.), as the references of `methods.rs`, not 2003.

## [1.58.0] - 2026-11-20

### Added
- **Thematic layers in the multiresolution segmentation**: `segment.multiresolution` takes the optional input `bounds` (`crates/ops/src/objects/zones.rs` reads it), as eCognition's thematic layer: a map (a zone for each value, the cells without a value one zone together), vectors (each feature a zone, burnt with its index, the cells in none one zone) or objects (each object a zone, id 0 one zone), and no object crosses from one zone to another (a parcel, a block, a field). With `from` the objects of the level below are cut by the zones first; the neighbour lists skip the pairs of cells of different zones while the perimeter still counts those edges, so the shape cost stays right; without `bounds`, or with one zone, the objects are the same as before, bit for bit. By tiles each tile gets the zones of its window, the stitching the whole layer, and the cluster's `TileJob` carries the zones, so the same objects come on any number of threads and on the workers. `segment.region_growing` reads `bounds` through the same helper, so it takes vectors and objects too (it took a map only). The live segmentation in the Viewer cuts the node's `bounds` to the window for the multiresolution (`segment_window_bounds`), and `segment_window_op` runs `segment.multiresolution` as the other segmenters. The other segmenters and the layer's attributes: the next entry.
- **Thematic layers in every segmenter, and their attributes**: `segment.felzenszwalb` and `segment.spectral_difference` take `bounds` natively (the graph's edges between zones are left out; only neighbours of one zone merge, and the objects given are cut by the zones first), and `segment.slic`, `snic`, `watershed`, `quickshift`, `meanshift`, `threshold`, `chessboard` and `quadtree` take it as a step after the segmentation (`split_by_zones` in `zones.rs`: the connected components of (object, zone), in parallel strips, the same on any number of threads; the tables are made from the final objects). Without `bounds`, or with one zone, the objects are the same as before, bit for bit. The live segmentation passes `bounds` to every segmenter it runs, and the comparison window passes the shown node's `bounds` to each segmenter it compares. `objects.features` takes the optional input `thematic` (a map or vectors) and the setting `field` (the vector's attribute, a text one too, as codes named in the table's labels; else the feature's number from 1): the columns `th_mode`, the most common value under each object (the smallest on a tie), and `th_share`, the share of the cells with a value that have it (cells with no value, or in no feature, count for none), which the rules and the classifiers read like any column.
- **Dividing objects that touch: `objects.split`**, as eCognition's watershed transformation and CellProfiler's declumping (`crates/ops/src/objects/split.rs`, with `each.rs`, the walk over objects in their boxes). `by` = `distance` (the default) floods, per object and in its box with a pad of one cell, the exact Euclidean distance to the object's border (`linkage::edt`, in map units), so the cut is at the neck; `by` = `surface` floods a map whose peaks are the centres (`invert` for dark ones, `sigma` to smooth it). The markers are the peaks that stand `h` above the saddle (`segment.watershed`'s h-minima over the landscape upside down; default one cell for the distance, 0.05 of the surface's range for a surface) and the flooding is that operator's queue (`minima` and `flood` are `pub(super)` now; the watershed's results are the same, bit for bit). `min_size` gives a piece of fewer cells to the neighbour piece with the longest shared border; `max_neck` (eCognition's length factor) undoes a cut longer than `max_neck` times the width of the smaller piece (twice its largest distance to the border); `classes` limits which objects split. The pieces keep the class and the memberships of their object and replace it in the same level: the level above is dropped, with a warning, and so are the other attributes (the table is made again with the `image`); with nothing to split the objects come back as they are. Objects run in parallel under the central cap, the biggest boxes first, and the result is assembled in the order of the ids: the same objects on 1 and on 8 threads. Outputs `objects`, `table` and `report` (the object and its pieces). On a synthetic image of touching blobs (`objects_split_bench`): 1000 x 1000 in 0.05 s on one thread and 0.01 s on 8, 2000 x 2000 in 0.20 s and 0.05 s. The method's references (Vincent and Soille 1991; Carpenter et al. 2006) are in the methods list, and `docs/patents-objects-resolve.md` has the paragraph on the patent reading.
- **Growing, shrinking and smoothing objects cell by cell: `objects.resize`**, as eCognition's pixel-based object resizing, morphology, hole filling and border optimization (`crates/ops/src/objects/resize.rs`; `each.rs` gains `each_with` for boxes of changed labels). `mode` is `grow`, `shrink`, `open`, `close`, `fill`, `smooth` or `refine`, by `distance` (map units) or `steps` (cells); `classes` picks the objects that change. Without a condition on the cells, `grow` and `shrink` are exact Euclidean in one pass (Felzenszwalb and Huttenlocher's transform of `linkage::edt`: each free cell within the distance goes to the nearest chosen object; `shrink` removes the cells within the distance of a cell that is not their object's, per object in its box and in parallel, the image's edge not a border). A `mask` (a map, cells that are not 0 may be taken, say `ndvi > 0.3` from `raster.calc`) or `into` (class names whose cells may be taken, eCognition's candidates) makes the growth geodesic, ring by ring in rows in parallel with two buffers: a cell goes to the object with most cells around it, a tie to the lower id. `open` shrinks and grows back over the cells the object had (it cuts a one-cell bridge into two objects of the same class), `close` grows and shrinks only the cells added (the image's edge counts as outside), `fill` gives the holes of free cells that one object encloses, not touching the image's edge, up to `max_hole` cells, `smooth` gives each cell the most common label in a disc, and `refine` (with the `image`) moves each border cell to the adjacent object of the closest band means in z-scores, `steps` passes that read the means of the pass before. Outputs `objects` (the classes kept, the level above dropped with a warning), `table` (with an image) and `report` (a row per chosen object: its cells before and after and the objects it ended in); the same objects on 1 and 8 threads. References: Serra 1982, Soille 2004, Felzenszwalb and Huttenlocher 2012 (`methods.rs`). Tests: `crates/ops/tests/objects_resize.rs`; bench `objects_resize_bench` (1000² in 0.03-0.06 s, 2000² in 0.11-0.23 s for grow, open and refine, on 1 and 8 threads).

## [1.57.0] - 2026-11-19

### Added
- **The time of a segmenter before it runs**: a segmenter's node in the Node pane now says, next to the time of its last run, how long it will take: "about 5 min on 8 threads; by tiles of 512, about 1 min", from the rates of the benchmark battery (`crates/ops/src/objects/rates.json`, written by `BENCH_RATES=1` on `objects_bench`: the seconds of 1000² and 2000² cells on 1 thread and on 8, and of the tiled runs) and the size of the image (the value of its input if it ran, else the grid the memory estimate reads from the file headers, which `memory::Estimate` now carries). The time grows as cells^alpha (alpha from the two sizes: multiresolution 1.17, region growing 1.07), the threads by Amdahl's law from the 1 and 8 runs, and the tiled run as the work of its tiles (with margins) over the threads plus the stitch. `terraflux_ops::objects::estimate(op, params, cells, bands, threads)` covers the twelve image segmenters and the tiled `segment.multiresolution` and `segment.region_growing`; it reads only `tile` and `margin` (no setting scales the rate: `threshold` of region growing goes opposite ways at 1000² and 2000²), and not the image's content. From 30 s on, *Use tiles* writes `tile` = 512 as one step of undo. The rates are one machine's, so each run that finishes corrects them: the last ratio of seconds seen to seconds told is kept for each operator in the settings (`time_ratios`). The rates of this release come from a run on a busy machine; a quiet run will refine them, and the correction by the times seen meanwhile makes up the difference.
- **Region growing by tiles**: `segment.region_growing` takes `tile` (the side in cells, 8 or more; 0, the default, is the whole image at once as before) and `margin` (default 64 cells, at most half the tile), and runs as `segment.multiresolution` does by tiles: each tile with its margin is grown on its own, all the tiles in parallel on the rayon pool (a continuous queue, the memory for tiles limiting how many at once), with the scaling of the whole image's ranges (or the given `ranges`) and `minsize` 1; the cores of the tiles are the pieces, and one merging over the whole image starts from them (the path of `seeds`), joins the pieces of one seed, and applies `threshold` and `minsize`, so only the pieces along the tile borders merge. `bounds` and `seeds` are cut per tile; a seed that crosses a tile border is one object again, and no object crosses a zone. The labels are the same on any number of threads, and a tile over the image is the run without tiles bit for bit; on an image of blocks of one value tiled equals untiled. On the synthetic 2000 x 2000 x 4 image of the benchmark (`threshold` 0.05) the run takes 16.2 s on one thread without tiles, 14.1 s with `tile` 512 on one thread and 4.2 s on 8 (2277 objects against 2590: the greedy merge is order dependent, so a tiled run is not the same segmentation, with `minsize` 20 it is 225 against 228). Memory images only; multiresolution keeps its labels (the benchmark's hashes are unchanged) and its cluster path. The tiling of `tiles.rs` is now `merge_tiled_with`, for any segmenter (a function per tile and one to stitch).
- **Two templates of the new object operators**, in the objects group, running as they are on drawn data and marking the image with `data`: "Water by Otsu on the NDWI" (`raster.calc` of the NDWI, `segment.threshold` with two classes whose threshold Otsu finds, `objects.rules` keeping the high class as water, `objects.to_vector`) and "The scale chosen by itself" (`segment.optimize` over the scales 2 to 20: the `scores` open as the scores chart, `value` is the best scale, and *Apply* writes it into a segmenter). With "Unsupervised classes of objects" they are in the guide's list of object templates.
- **Compare segmenters side by side**: *Compare...* in the live segmentation's panel opens a window (`segment_compare.rs`) that segments the same window of the image (the whole image when small, else the part in view) with several segmenters, one after another in a worker thread that never blocks the Viewer: multiresolution, SLIC, watershed and Felzenszwalb by default, and threshold, quickshift, region growing and mean shift by checkbox. Each shows its outlines over the image (grey, or a 2-98% stretch of three bands), the number of objects and the seconds, and runs with the settings of a node of that operator on the same image, else the sliders' defaults. *Use* puts the segmenter in place of the one shown: what read the shown node's objects reads the new one's (a node of that operator on the image if the model has one, else a new node connected to it), the old node stays with no readers, one step of undo.
- **Manual editing of objects**: `objects.edit` (`crates/ops/src/objects/edit.rs`) merges, cuts and deletes objects as eCognition's manual editing does, from a list in `edits` applied in order: `{"merge": [[x, y], ...]}` (the objects at the points become the first one's), `{"cut": [[x, y], ...]}` (a line of two points or more; each object it crosses is split into the connected pieces left without the line's cells, and each cell of the line goes to the piece next to it with the smallest label, so the result is the same every time) and `{"delete": [x, y], "into": "none" | "neighbour"}` (the object is left with no object, or taken by the most similar neighbour: the nearest band means with the optional `image`, else the longest shared border). The edits are places on the objects' grid, not object numbers, so the same list finds its objects again when the segmentation above changes; an edit that finds no object, merges objects already one, merges objects that do not touch or cuts nothing is warned about (`edit 3 (cut): ...`) and has a row in the `report` table. The objects are numbered again, with no classes or levels (put the node right after the segmenter), and with an `image` the `table` has the segmenters' band means. In the Viewer's objects inspector a new *Edit* block: *Merge* (two or more selected objects) and *Delete* (with *into a neighbour*) act on the selection, *Cut* draws a line on the map (a click a vertex, Enter ends it); each is written as one step of undo into the `objects.edit` node that reads the segmenter's objects, or into a new one made right after the segmenter, with every node that read the segmenter moved onto it.
- **Live segmentation for every segmenter**: the Viewer's sliders, until now for `segment.multiresolution` only, now serve `segment.slic` and `snic` (`size`, `compactness`), `felzenszwalb` (`k`, `sigma`), `watershed` (`h`, `compactness`), `threshold` (`classes`), `spectral_difference` (`max_difference`), `region_growing` (`threshold`), `quickshift` (`kernel_size`, `max_dist`) and `meanshift` (`spatial_radius`, `range_radius`), each over a table of ranges (fixed, around the node's value or from the band's range, logarithmic or whole). The preview is the node's own result: an image of up to a million cells (250 000 for the slow ones: region growing, quickshift, mean shift) is segmented whole, a bigger one by the part in view, read every few cells. *Apply* writes the values in one step of undo (whole numbers for the integer settings, dropping `count` or `thresholds` when `size` or `classes` replaces them). `segment_window_op` (`crates/ops/src/objects/window.rs`) runs any of them through the registry on a window: the window is standardised by the whole image's mean and standard deviation (with `standardise` off), the settings in cells are divided by the reading step, the thresholds are the whole image's, and `objects`, `markers`, `seeds` and `bounds` are cut to it. `segment.region_growing` has a new optional `ranges` (the range of each band for the 0-1 scaling), which a window or another date passes with the whole image's.
- **The chart and *Apply* of `segment.optimize`**: its `scores` table opens as a *scores* chart (`charts.rs`): the objective and the variance and Moran's I scaled 0-1 as the operator scales them, against the value, the best marked by a vertical line, and the objects each value made in a smaller panel. Under it (and in the node's pane) `optimize_view.rs` says "best scale = 4" and offers *Apply to*: a segmenter of the same operator on the same image, or a new node connected to that image; *Apply* writes the value and the fixed `params` of the optimize (what was scored), one step of undo. `scaled` and `default_parameter` of `terraflux_ops::objects` are public.
- **Unsupervised classification of objects**: `objects.classify` takes a model without labels, an `ml.kmeans` or an `ml.som` fitted on the table of `objects.features` (`columns` naming the attributes, not the id, the cells or the area), and needs no samples: each object takes its cluster (classes "cluster 1" and on), or, for a map with `clusters`, the group of its node ("group 1"...), or its node ("node 1"...); the memberships are 1 for its class and 0 for the others, and the objects, the table and the map of the classes come out as with a forest. The attributes the model reads are looked up by name, with a message that says to name them in `columns`. Segment, `objects.features`, `ml.kmeans`, `objects.classify` works end to end (`crates/ops/src/objects/classify.rs`).
- **Five templates of the multivariate operators**, each running as it is on drawn data and marking the user's with `data`: "Principal components of an image" (`ml.pca`, the scores shown as RGB, the loadings and the elbow), "Unsupervised classification of an image (k-means)" (`ml.kmeans` with k auto, the map of clusters, the choice of k and the centres), "Types of landscape by a self-organizing map" (four environmental variables to `ml.som` with five groups, the map of the types and the codebook that opens as the SOM chart), "Change between two dates by principal components" (the red and near infrared bands of two dates as one set of variables: the third component carries the change) and, in the objects group, "Unsupervised classes of objects" (k-means on the objects' attributes, `objects.classify` with the model).
- **The `multivariate` guide** of the MCP server (`guide multivariate`): when to use PCA, k-means or a self-organizing map, standardising, the choices made without being asked (95% of the variance, k by the silhouette, the grid by Vesanto's rule, a sample above a million records), reading the results, change between dates by PCA and unsupervised classes of objects.
- **`ml.som`**, a self-organizing map (Kohonen 1982) of a stack or a table (`crates/ops/src/multivar/som.rs`: the engine of Zeit, an operation-by-operation port of MiniSom 2.3, batch or online, the same bits as MiniSom; `codebook.rs`: the map): from the cells of a stack (a layer per variable, `cells` the cells to use) or the columns of a table (`columns`), a grid of `rows` by `cols` nodes (without them about 5 sqrt(n) nodes, Vesanto & Alhoniemi 2000, the sides in the ratio of the square roots of the first two eigenvalues of the variables' covariance, at most 2500 nodes, said in a note), `topology` rectangular or hexagonal (MiniSom's coordinates), `neighbourhood` gaussian, bubble or triangle, `learning_rate`, `sigma`, `decay` (asymptotic, inverse or linear), `training` batch (the default, on all the threads with the sums in fixed blocks of nodes, so the same weights on any number of threads) or online (a record at a time, in the order of a seeded shuffle), `iterations`, `init` (the first two principal components, or random records), `standardise`, `sample` and `seed` as `ml.pca`. `clusters` k above 0 groups the nodes in k groups by Ward's method (or `grouping` = `kmeans`; the two levels of Vesanto & Alhoniemi), numbered by the first principal component of their centres. Outputs: `nodes` (a map, the node of each cell, row by row from 1) and `groups` (a map), or a table with the columns `node` and `group`; `codebook` (a row per node: row, column, count, group, `umatrix` -- its mean distance to the nodes next to it --, `topology` and its weights in the variables' own units); `quality` (quantisation and topographic error, empty nodes) and `model`, which `ml.predict` applies to other maps or tables with the means and scales of the fit (a `map` and `groups`, or `node` and `group` columns). Checked against MiniSom (`tools/make_som_reference.py`, 13 cases of Zeit's parity fixtures and of the hexagonal topology with an even and an odd number of rows): the trained weights to the bit, and its winners, quantization and topographic errors and distance map. `ml.importance` and `ml.partial_dependence` do not take it.
- **The SOM view and the PCA composite**, the charts and the Viewer for `ml.som` and `ml.pca`: the `codebook` table opens as a *SOM* chart (`charts.rs`), the grid of nodes drawn as squares or, for the hexagonal topology (a new `topology` column of the codebook, named by the table's labels), as hexagons with the rows shifted as MiniSom's, in four views: the U-matrix, the component planes (a grid per variable), the counts (the hits of each node) and the groups, on the viridis ramp (the groups in the classes' colours), the weights and counts told under the pointer. A click on a node (in the groups view, on its group) picks it and says how many records it holds; clicked again, it is let go. The pick is linked to the maps of the node (`som_link.rs`): showing the `nodes` or `groups` output lights the cells of the node or group picked in the accent and veils the others, and a cell clicked on either map picks its node or group in the SOM view. The scores of `ml.pca` (a stack pc1, pc2...) have a composite of pc1, pc2 and pc3 as one click on the box *RGB* of the Layers, each band from its 2nd to 98th percentile, also among the *composites* as "principal components (PC1 PC2 PC3)".
- **`ml.kmeans`**, the clusters of a stack or a table by k-means (`crates/ops/src/multivar/clusters.rs`; the Janus k-means of `multivar/kmeans.rs`: the sums in fixed blocks on the rayon pool, so the same clusters on any number of threads): from the cells of a stack (a layer per variable, `cells` the cells to use) or the columns of a table (`columns`), the greedy k-means++ start of scikit-learn, Lloyd's iterations (`max_iter` 300, `tol` 1e-4 of the variables' mean variance) and the best of `n_init` runs (10) by inertia, on the variables standardised (`standardise`, the default). `k` is a number, a list or `"auto"` (the default): with several, each is tried (on at most 100 000 records; the one chosen is then fitted on all), the `choice` table gives per k the inertia, the silhouette (Rousseeuw 1987, on a sample of at most 10 000 records, in parallel and deterministic) and the Calinski-Harabasz index (drawn as a chart of inertia and silhouette against k), and the best silhouette wins, said in a note. The clusters are numbered 1 to k by the first principal component of their centres (the largest loading positive), so the numbers and the colours do not change between runs or with the order of the records. Outputs: `clusters` (a map, NaN where a variable has none or the cell is not in `cells`) or `table` (with a `cluster` column), `centres` (a row per cluster: its size, its inertia and its centre in the variables' own units) and `model`, which `ml.predict` applies to other maps or tables with the means and scales of the fit (a `map`, or a `cluster` column). As `ml.pca`, a fit above a million records uses a sample (`sample`, `seed`), said in a note, and every cell is then assigned, in parallel by pieces. Checked against scikit-learn's `KMeans` (`tools/make_kmeans_reference.py`): Lloyd's iterations from the same centres give its labels, centres and inertia, and the inertia, silhouette and Calinski-Harabasz index of each k are its own. `ml.importance` and `ml.partial_dependence` do not take it.
- **`ml.pca`**, the principal components of a stack or a table (the first of the multivariate operators; `crates/ops/src/multivar/pca.rs`): from the cells of a stack (a layer per variable, `cells` the cells to use) or the columns of a table (`columns`, default all the numeric ones), the covariance or, with `standardise` (the default, right for variables in different units), the correlation, in one pass over the records in fixed blocks and its eigenvectors by Jacobi's rotations, so the same numbers on any number of threads. `components` is a number or the share of the variance to keep (default 0.95); each component's largest loading is positive, as `metrics.pca`'s and scikit-learn's. Outputs: `scores` (a stack pc1, pc2... with NaN where a variable has none, or `table` with the columns added), `loadings` (as `metrics.pca`'s: `component`, `explained` and a column per variable, so the circle of correlations draws it as it is), `explained` (the variance, share and cumulative share of every component, drawn as an *elbow* chart: bars and a line) and `model`. The fit uses all the complete records up to a million and a sample above (`sample`, `seed`, numpy's draw), with a note saying so. `ml.predict` scores other maps or tables with the model, from the means and scales of the fit, not of the new data. Checked against scikit-learn's `PCA` up to the sign (`tools/make_pca_reference.py`). `ml.importance` and `ml.partial_dependence` do not take it.
- **`segment.optimize`**, the unsupervised optimisation of a segmenter's setting (USPO, Espindola et al. 2006, as GRASS's `i.segment.uspo`): any `segment.*` that takes an image runs at each value of one setting (`values`, or `from`, `to` and `step`; `parameter` defaults to the main one of each segmenter, `params` holds the others), the runs in parallel and as many at once as the memory allows. Each result is scored by its intra-object variance (weighted by the cells, the mean over the bands) and Moran's I of the objects' means over the objects that touch; both are scaled over the runs and joined by their sum or, with `objective = "f"`, by the F-measure of Johnson & Xie (2011) with `alpha`. The best run's objects and table are the outputs, with a `scores` table of every value and the best `value`; only runs not beaten on both scores keep their objects, so the memory is that of a few runs, and the result is the same on any number of threads.
- **`segment.region_growing` and `segment.quickshift`**, two more segmenters: `segment.region_growing` is the region growing and merging of GRASS GIS's `i.segment` (Momsen & Metz): regions start as cells (or as the segments of `seeds`, objects or a map) and merge in passes, each one with its most similar neighbour when they are each other's best (`mutual`) and their distance, on the bands scaled to 0-1 over the image (Euclidean or Manhattan, `similarity`), is at or under `threshold` (0 to 1), until a pass merges nothing; `minsize` joins the small ones to their most similar neighbour at the end, `bounds` (a map of zones) is never crossed, 4 or 8 `neighbours`; it runs on a region adjacency graph with the distances kept and the visits prefetched, the set up in parallel, and gives the same objects on any number of threads. `segment.quickshift` is Vedaldi & Soatto's quick shift (as scikit-image's): a Parzen density of Gaussian `kernel_size` over position and `ratio` times the (standardised) bands, each cell linked to the nearest cell of higher density within `max_dist`, the trees the segments (split into connected pieces), by rows in parallel with the exponentials in single precision on contiguous planes. Both are in the benchmark of the segmenters.
- **`segment.threshold` and `segment.spectral_difference`**, two more of eCognition's segmenters: `segment.threshold` is the multi-threshold segmentation (one band, by name or number, cut at the thresholds given or, with `classes` 2 to 8, at those of multi-level Otsu found on a histogram of the band by a dynamic programme in O(classes x bins squared); each connected piece of a class is an object, with its class in the table and the thresholds as an output and a note); `segment.spectral_difference` merges the neighbours of any objects whose mean values in an image differ by less than `max_difference` (the weighted mean over the bands of the absolute differences of the means, `weights` optional), the closest pair first and the means of the merged objects counting for the next, so the result is the same on any number of threads. Both are in the benchmark of the segmenters.
- **The application's runs use the computers that work together**: when other computers have joined this one's pool (Computers), a run's `segment.multiresolution` tiles and `objects.features` groups go to them, with no setting to turn on; with none, nothing changes. A computer that leaves or fails never fails the run: the tiles (as the features already did) run here, with a warning, and the node says while its work is on other computers.
- **`objects.features` on the cluster**: with `--coordinator` (and no `--tiled`), the spectral and shape groups and each layer of the texture are worked out on the workers from the rasters on disk and the objects' labels (a file the workers fetch, or read on shared storage), the context on the coordinator from the joined columns: the same table as a run here. Without a cluster, or when an image is in memory, the features run here as before; if the cluster fails they run here with a warning.
- **A whole Sentinel-2 scene by objects on a common computer**: `segment.multiresolution` by tiles writes each tile's objects straight into one map of labels (the tiles never all in memory, as many in flight as 2 GB allow) and `objects.features` keeps each object's cells only when a feature asks for them: a 10 980 × 10 980 × 10 band scene from disk segmented and described with a peak of 2.1 GB (5.3 GB before). Labels larger than a sixteenth of the memory are kept in a memory-mapped temporary file, deleted with them.

### Changed
- **SAM asks before it downloads its weights**: a run (asked for, or to a node) of a `segment.sam` with no `encoder` and `decoder` whose weights are not in the cache is held back by a window that says the size (75 MB quantised, 348 MB full), the licence (Apache-2.0), the source address and the folder they are kept in (`<cache>/sam/<name>`), with the choice between quantised and full, which writes the node's `weights` (one step of undo). *Download and run* is remembered on the computer for each of the two (`sam_agreed` of the settings); a run the application starts by itself after an edit only says that a run asks first. `weights` is now a choice (`quant`, `full`), `weights::cached` tells whether the files are in the cache, and the progress of the download counts in MB, as the sizes do (it counted MiB). The progress and *Stop* were there already (`crates/app/src/sam_view.rs`).
- **`segment.sam` makes its objects as SAM's automatic mask generator does**: each mask's stability score (the IoU of the mask cut at logit +1 and at -1, `stability_offset`) must reach `stability_thresh` (0.95), its predicted IoU `iou` (now 0.88, was 0.8), and a mask covering more than `max_area` of the tile (0.9, the background) is dropped; the rest are thinned by box non-maximum suppression at 0.7, ranked by stability x IoU, and painted larger first so that the smaller ones on top survive (before, the highest predicted IoU won every overlap, which with the ONNX export's IoUs of about 1 for any mask let the background take the shapes). On a test scene of four shapes each is now one object with IoU 0.99 or more, with the quantised and the full weights.
- **`segment.sam` and `objects.embed` run with a real ONNX Runtime, and SAM's weights download once**: both were checked against ONNX Runtime 1.32 and real models (`objects.embed` against numpy, with a small Conv + ReLU + pooling model kept as a fixture, `tools/make_embed_reference.py`). `segment.sam` now takes the common `input_image` export (the tile resized, as height x width x 3 of 0-255, normalised inside) as well as the NCHW one, told apart by the encoder's input; that export's decoder crops the mask to a fixed 683 x 1024 frame, so the tile is fitted to that frame (without it the masks came stretched and shifted in height). With no `encoder` and `decoder`, SAM ViT-B (the ONNX export of vietanhdev, Apache-2.0) downloads on first use to the cache folder, the quantised one of 75 MB or, with `weights` `full`, the one of 348 MB, its SHA-256 checked and the files reused afterwards.
- **`segment.multiresolution` about 1.7 times faster on one core, the same objects**: each region keeps the terms of the cost that depend on it alone (each band's n·sd and the shape terms), so a cost takes one square root per band instead of three; the cost of each pair of neighbours is kept in the lists until either region changes (worked out once, in a row at the start); the lists live in the regions (no heap allocation per cell); the merge reuses its buffer; and the regions of the visits to come are prefetched (the visits are a shuffle, so each one waited for memory). On 4-band synthetic images: 1000 × 1000 at scale 20 from 2.3 to 1.4 s and at scale 50 from 4.5 to 2.5 s, 2000 × 2000 from 19.5 to 11.1 s; by tiles of 512 on 8 threads, 2000 × 2000 from 4.7 to 3.3 s. The costs are the same numbers bit for bit, so the objects are the same. `crates/ops/tests/objects_bench.rs` (ignored) times every segmenter at 1 and 8 threads.
- **The other segmenters faster, on one core and on many, the same objects**: SLIC assigns the cells by rows in parallel (each cell takes the nearest centre of the windows that hold it, the lowest on a tie, as before) and moves each centre with the sums of its own cells in the same order; Felzenszwalb builds and sorts its edges in parallel (the sort key is the weight, then the order the edges were made in, so equal weights keep it), keeps the union-find in one array with the larger set as root, fetches the sets of the edges to come ahead and leaves out of the second pass the cells already in a set of min_size; the watershed's flood queues a cell only when its priority is lower than every one queued for it (a later one would come out after the first, when the cell is labelled already: about an eighth of the pushes), SNIC does the same with its distance; both use a 4-ary queue of 16 bytes a key and one array for a cell's label and lowest priority; the Gaussian smoothing, the gradient, the sort of the cells of the minima, the preparation of the bands, the regions, the connected components of `Objects` (strips of rows, joined at their borders), the cell edges between objects and the table of means and deviations (the bands in parallel) all run on the threads. `segment.multiresolution` by tiles no longer waits for the slowest tile of each batch: a tile starts as soon as another is done (as many in flight as the memory for tiles allows), and the cores are put in the order of the tiles, so the labels are the same. On 4-band synthetic images, 1 thread / 8 threads, seconds: 1000 x 1000 SLIC 0.55 / 0.51 to 0.25 / 0.06, SNIC 0.78 / 0.80 to 0.31 / 0.29, Felzenszwalb 0.79 / 0.79 to 0.34 / 0.20, watershed 1.74 / 1.72 to 0.65 / 0.39; 2000 x 2000 SLIC 2.2 / 2.1 to 1.0 / 0.4, SNIC 4.9 / 5.1 to 3.0 / 2.8, Felzenszwalb 3.7 / 3.8 to 3.0 / 1.5, watershed 9.3 / 9.0 to 2.9 / 1.8; multiresolution by tiles of 512 on 8 threads, 2000 x 2000, from 3.3 to 2.9. The labels are the same bit for bit (the bench prints a hash of them).

## [1.56.0] - 2026-11-19

### Added
- **Objects: an image divided into objects, the base of object-based image analysis** (the first stage of OBIA, in place of eCognition; `crates/ops/src/objects/`): a new kind of value, **objects** (each cell's object, the objects' cells, boxes and neighbours with the length of the border they share, a table of attributes with a row per object, classes with their memberships, and the level above), kept on disk by frozen nodes and written as a GeoTIFF of the ids with the table beside it. **`segment.from_map`** makes objects of any map of classes or labels (every connected group of cells of one value, 4 or 8 neighbours: the way in for a segmenter run elsewhere, SAM included), **`segment.from_vector`** of polygons (their attributes in the table), **`segment.chessboard`** squares of a size and **`segment.quadtree`** squares split in four while a band's range in them passes a threshold; **`objects.to_map`** gives the id, the class, the parent or an attribute as a map, **`objects.to_vector`** a polygon per object with its class and attributes (Chaikin smoothing optional). The Viewer draws objects as a layer: outlines over the image at a width on the screen that does not change with the zoom (colour and width in the layer's row), filled by class (or a colour per object, or none); the object under the cursor highlighted, with its id, class and area in the status bar; a node's objects show their attribute table under the map. A new subject in the library, *Objects (OBIA)*. **`nlm.image`**: a synthetic multispectral image (blue to SWIR 2, Sentinel-2-like reflectances of water, forest, grassland, cropland and urban, or your signatures) from a map of classes, with noise, texture and blurred edges, its truth known: the data of the object templates and tests.
- **The segmenters of object-based image analysis** (the second stage of OBIA): **`segment.multiresolution`**, eCognition's multiresolution segmentation (Baatz and Schäpe 2000: neighbours merged while the growth of colour and shape heterogeneity stays under the scale², by local mutual best fit, in a seeded order; `shape`, `compactness`, band `weights`; from the cells or, with `from`, from the objects of a level, which come out as `below` with their parents); **`segment.slic`** and **`segment.snic`** superpixels (Achanta et al. 2012 and 2017), **`segment.felzenszwalb`** (Felzenszwalb and Huttenlocher 2004), **`segment.watershed`** (on the gradient of every band, from your markers or the gradient's deep minima, compact when asked: Neubert and Protzel 2014) and **`segment.meanshift`** (Comaniciu and Meer 2002, as OTB's large-scale mean shift); and **`segment.scale`**, the estimation of scale parameter (ESP2, Drăguţ et al. 2010 and 2014): the local variance of the objects and its rate of change level by level, on a central window, with the three suggested scales and an *ESP* chart. Without a `scale`, `segment.multiresolution` takes the first one, and says which. Every segmenter gives a table with each object's cells, area, and each band's mean and standard deviation; the same objects with the same settings, whatever the number of threads.
- **`objects.features`, what describes each object** (the third stage of OBIA): from the objects, an image and any more rasters (a DEM, a height model, a map of classes, another date), a column per feature in the objects' table, by group: **spectral** (each band's mean, standard deviation, minimum, maximum and median, brightness, maximum difference, ratios, and NDVI, NDWI and NDMI when the red, near infrared, green and SWIR bands are found by their names, Sentinel-2's and Landsat's included, or named), **shape** (area, perimeter, compactness, shape index, length and width of the smallest rectangle, asymmetry, main direction, elliptic and rectangular fit, border index, density, centroid), **texture** (Haralick's grey-level co-occurrence per object in four directions: homogeneity, contrast, entropy, correlation, angular second moment, of the brightness and of each band) and **context** (the mean difference to the neighbours weighted by the border, the neighbours, the border on the image's edge; with classes, the relative border to, the neighbours of and the distance to each class; the parent's attributes; with the level `below`, the children of each class and their share of the area). `basic`, the default, is the means, the standard deviations, NDVI, the area, the compactness and the homogeneity; `all` is every one.
- **Objects classified three ways** (the fourth stage of OBIA): **by samples**, `objects.samples` turns points or polygons with a class (drawn, read, or clicked later) into the table of the objects under them, an object of two classes left out with a warning, for `ml.fit`, and **`objects.classify`** gives each object the class of a fitted model with the probability of every class as its membership; **by rules**, **`objects.calc`**, the objects' calculator (Python's syntax as `vector.calc`, with `cls`, the neighbours `nb.mean(x)`, `nb.count(cls == "water")`…, `border_to`, `distance_to`, `parent.x`, `children.count(...)`/`area`/`mean`, and the fuzzy memberships `above`, `below`, `between`, `about` where `and`, `or`, `not` are the minimum, the maximum and 1 − x), written with the MCP's `write_calc` too, and **`objects.rules`**, a hierarchy of classes each with a rule (a daughter inherits her mother's), the leaf of the greatest membership and a least membership as eCognition; and **by the nearest neighbour**, **`objects.nearest`**, eCognition's Standard Nearest Neighbour in the space of the objects' colours.
- **The accuracy of objects**: **`objects.accuracy`** checks a classification of objects against samples kept apart (points, polygons, or a table of ids and classes): the confusion matrix, the user's and producer's accuracy and kappa by objects, the estimates adjusted by area with their 95% intervals (Olofsson et al. 2014), and the misclassified objects; **`objects.sample`** draws a stratified random sample of objects to label, a point inside each.
- **Levels and object operations**: **`objects.merge`** joins neighbours of the same class, or the pairs a condition holds for (`abs(a.mean_red - b.mean_red) < 0.1`, `border`), into objects, or into a level above with the objects below as its children; **`objects.grow`** grows the objects of a class into the neighbours a condition holds for; **`objects.remove_small`** puts objects under a size into the most similar neighbour or the one of the longest border; **`objects.reclassify`** renames and joins classes. Each gives `done`, for the `until` of a `control.repeat`.
- **Hypotheses arbitrated and two dates bound** (InterIMAGE): **`objects.resolve`** takes a membership map per class, each found by its own method, and gives each cell the class of the greatest (above a least), with the second as the doubt, the conflicts counted, and the winners joined into objects; **`objects.temporal`** corrects the classes of one date with those of another through a fuzzy transition matrix (Feitosa et al. 2009), estimated from samples of the two dates by **`objects.transition_matrix`**, with the table of changes.
- **Big images**: `segment.multiresolution` segments by tiles in parallel above 4 million cells (`tile`, `margin`), the objects cut by a tile's border stitched by a second merging over the whole image: 2.5 times faster with 3 threads, the same borders in 98% of the cells.
- **Deep learning, optional**: **`segment.sam`** (Segment Anything) and **`objects.embed`** (an embedding of each object by a model of the user's choice) run ONNX models through the ONNX Runtime, loaded at run time when installed (`pip install onnxruntime`, or `ORT_DYLIB_PATH`): Terraflux works without it.
- **The class tree** (InterIMAGE): what is in the scene said first, Urban (Buildings, Roads), Vegetation, Water, each class found its own way inside its mother's region. The Model pane's **Classes** button shows a card per class (its colour, method, rule, samples and area after a run), and **Add a class...** writes its branch: a `control.group` with the class, its mother and colour, reading `@region`, from a recipe (water by NDWI, vegetation by NDVI, buildings by height, by samples), its siblings arbitrated by an `objects.resolve`, in one undo step.
- **Objects in the Viewer**: a click selects an object (N its neighbours, P its parent, switching to the level above, C its children, Esc none), the selection one with the attribute table's row; a level selector for objects with levels above; the **inspector** of the selected object, its attributes, its membership to each class in bars, its margin and what decided (each rule's value, the most likely classes, the distance to each class's nearest sample); and **samples by clicking**: the *Samples* mode's palette of classes (added there, counted, flagged under 10), a click or a drag gives the objects a class, a right click takes it away, and *Apply* (or *Live*) writes them into the model as points with the chain objects.samples → ml.fit → objects.classify, in one undo step. `trace_cell` gives the memberships and the margin too.
- **Templates and a guide for objects**: six templates in a new group *Objects (OBIA)*, "Classify an image by clicking samples", "Classify an image by rules", "Segment an image into polygons", "Land cover change by objects", "Trees and buildings from a height model" and "Each class its own way" (the class tree, a recipe per class), each running as it is on a synthetic image; and the MCP's guide `objects`, the method step by step.
- **Use my data for images**: a data node of kind `image`; the **Bands** page gives each band of the user's image its role (blue, green, red, nir, swir1, swir2) from its name (Sentinel-2, Landsat 8/9) or by hand, with the reflectance's scale detected (0 to 10 000, Landsat Collection 2) and normalised by the new **`stack.rename`** (renames, drops and scales a stack's layers); the **Classes** page edits the classes of the template's rules (names, colours, added, removed).
- **Advice for objects**: warnings at the run when an image has no coordinate system, its bands are in very different scales or look like reflectance in 0 to 10 000, a class has no or fewer than 10 sample objects, the classes are very unbalanced, the sample objects are much larger or smaller than the sample polygons, or two attributes are almost equal; and in the check when a model classifies other objects than it was fitted on, validates with its training samples, or has a rule reading an attribute no objects.features makes.
- **Big images from disk**: `segment.multiresolution` reads each tile's window from rasters on disk, and `objects.features` reads them by strips of rows, the image never whole in memory (a 4000 × 4000 × 4 image segmented and described with a peak of 1.2 GB); reading whole an image larger than half the memory is refused with what to do. With `--coordinator` (and no `--tiled`), the tiles are segmented on the cluster's workers, the same objects as here.
- **Live segmentation and comparisons in the Viewer**: for a `segment.multiresolution`, sliders of the scale, the shape and the compactness re-segment only the visible window in a thread (under a second for a screen), beside the ESP curve with its suggested scales, and *Apply* writes them into the node; the curtain, side by side and difference compare two objects (two levels, two classifications, two dates); and for an `objects.resolve`, each class's membership as a layer and, under the cursor, every class's membership, the winner, the doubt and the conflict (`trace_cell` too).
- **The feature space of the objects**: the *Feature space...* window draws two attributes with the samples coloured by class and a histogram per class, ranks the attributes by how well they separate the classes (Jeffries-Matusita), turns a range dragged on the histogram into a fuzzy rule of `objects.rules` (*Make a rule*), and lists the most uncertain objects and the validation's errors, each leading to its object.
- **More of the class tree and Use my data**: a class *made of its daughters* (Vegetation = Forest ∪ Grassland, the fuzzy union) and buildings by height and shape; an image from a folder of single-band files (Sentinel-2, Landsat), or, with *No image of your own?*, a cloud-free Sentinel-2 scene of an area and dates read by `data.stac` when the model runs. The *Samples* mode reads a template's sample points as its own.
- **Tutorials of object-based image analysis**, on the site and in the Help menu: "Your first map by objects in 10 minutes", "Coming from eCognition" and "Coming from InterIMAGE".

## [1.55.0] - 2026-11-18

### Added
- **A Help menu, and the application's way to its documentation**: **Help** after View in the menu bar opens the website's Quickstart, Tutorials, The application and Operator reference in the browser; **Keyboard shortcuts** (F1) moved there from View; **Report a problem...** opens a new GitHub issue in the browser with the Terraflux version, the system and the last failure (the last run's, else the first error of the model) filled in and the headings *What I did*, *What I expected* and *What happened* to write (the address is cut so that it stays within what GitHub takes; the person reads it and sends it, Terraflux sends nothing); **About Terraflux** is a window with the version, the licence, how to cite (built from `CITATION.cff` when Terraflux is built, with a **Copy** button) and the links to the website, the source code, the releases and the changelog. All of them are also in the command palette. The new module `docs` has `op_url(id)` (`/operators/{subject}/#op-{id}`, by the same rule as the site's `website/build.py`, from the subjects of the operator library, which the site reads from `website/data/subjects.json`: a test fails when the two differ), `page`, `tutorial_url`, `issue_url` and `open`. On the website, `subject_of` now gives the operator named in full over a prefix, so `raster.stats` and the other full names of Raster analysis are no longer listed in Data preparation; the file operators (`raster.read`...) are the page IO (`refresh_data.py` read an empty list of them), and `clumondo.*`, `molusce.*` and `futures.*` (in Land change simulation) and `check.*` and `value.calc` (in Model flow) have a subject, as they were in no page of the reference. Tests: `cargo test -p terraflux-app --lib docs`, `--test ui the_help_menu`.
- **Every operator leads to its page, and a template to its tutorial**: the Node pane has a **Documentation** link beside *more...* (not for a node of your own library, a submodel or an input), and the right-click menu of an operator, in the **Operators** menu and in the library's drawer, and of a node on the canvas, has **Open its documentation**; all open the operator's page in the operator reference in the browser. A new shortcut, `op_docs` (shift+F1, remappable in Settings > Keyboard), opens the page of the node selected; **Help > Page of the selected operator** (off with no node selected) and the command palette do the same. A template that has a tutorial on the website (the `template:` of 45 pages) shows a small **tutorial** link at the right of its row in **Templates**. The table is `crates/app/src/tutorials.json`, made from `website/content` and `website/site.yaml` by `website/tools/tutorial_index.py` (the categories in the site's order, each tutorial with its title, its template and the models published with it) and read once by the new module `tutorials` (`index`, `for_template`, `url`); a test fails when the file is out of date, and another when a tutorial names a template that does not exist. Tests: `cargo test -p terraflux-app --lib tutorials`, `--test ui an_operator_and_a_template`.
- **A start page on the canvas of an empty model**: with no node in the model (and not inside a group or loop), the Model pane shows over its canvas a column in the theme's colours: **Start from your data** (**Open a raster or vectors...**, **Wizard: a land change model from your maps**, **Open a model...**, and the hint to drop files on the window), **Recent models** (up to six that exist on disk, the name and its folder), **Learn** (Quickstart, Tutorials, The application) and **Start from a task** (the template groups as a grid of one to three columns by the width, each template a button with its description on hover and its **tutorial** link, and **Yours**, the templates of your own). A template starts as it does from the Templates menu and the other buttons do what the File menu and the Wizard do. The page goes with the first node and is back when the model is empty again; the canvas under it keeps working (dropped files, `/`, the library). It is built only while the model is empty (the recent models and your templates are looked at again when the list of recent models changes or once a second, not on every frame) and it scrolls in a narrow window. **Show this page when the model is empty**, at its foot and in Settings > Model (`start_page`, on by default; an old settings file loads as on), turns it off. New module `start_page`. Tests: `cargo test -p terraflux-app --test ui the_start_page`.
- **Help > Tutorials with a model...**: a window (also in the command palette, and as a **Tutorials with a model** link in the start page's Learn row) that lists the website's tutorials by category (the site's order, the first open, a filter box over the title, the category and the template, in any case), by default those that come with a model, and **all tutorials** for the rest. Each row has **Read** (the page) and **Open the model**: a template opens as the Templates menu does (asking about unsaved changes); the models published with a tutorial (`/data/tutorials/...`) are downloaded with every file of their folder (the csv and rasters they read) into `tutorial models` in the application's data folder, in a thread with a spinner in the row, and opened like File > Open (one button per `.toml` when a tutorial has several); a failure is told in the row, in red, not as a dialog. On the website, a tutorial with a template or a published model says in its facts how to open it in the application (`In Terraflux: Templates > group > name, or Help > Tutorials with a model`; the group is read from `templates.rs` by `build.py`). `tutorials.json` now lists every file of each tutorial's data folder (`files`). New module `tutorial_models`. Tests: `cargo test -p terraflux-app --lib tutorial_models` and `--test ui the_tutorial_models`.
- **`linkage.pathways`, Linkage Mapper's Linkage Pathways in Terraflux** (McRae and Kavanagh 2011; pure Rust, `crates/ops/src/linkage.rs`): from a map of core ids and a resistance, the cost-weighted distance from each core (ArcGIS's convention: 8 neighbours, a step the two cells' mean resistance times its length, sqrt(2) diagonally; no value or resistance 0 or less impassable), the cores to link by `adjacency` (`cost` allocation zones that touch, the default; `euclidean`; `all`), the links dropped past `max_euclidean` or `max_cwd`, through a third core (`drop_crossing`) or outside each core's `nearest` k, and the corridors, the least of the kept links' normalized distances (CWD A + CWD B - LCD) cut at `width`. Outputs `corridors`, `links` (from, to, euclidean, lcd, path_length, cwd_to_path, kept, reason), `paths` (lines through the cells' centres) and `cwd`. The search (`linkage::cwd`) takes any sources with their labels, for the pinch points and barriers to come. Tests against scikit-image's MCP_Geometric (`tools/make_linkage_reference.py`): `cargo test -p terraflux-ops --test ops linkage::`.
- **`linkage.pinchpoints`, Linkage Mapper's Pinchpoint Mapper in Terraflux** (McRae 2012; Circuitscape, McRae et al. 2008; `crates/ops/src/linkage.rs`): from `cores`, `resistance` and the kept rows of `linkage.pathways`' `links`, in `pairwise` mode (the default) a circuit per link between its two cores, inside its corridor only (the cells whose normalized least-cost distance is within `width`; the cores' cells there the focal regions), run in parallel through the library's Circuitscape, and the `current` the largest of the links' (no value outside the corridors and in the link's own cores, as Linkage Mapper's); in `all_to_one` mode one all-to-one circuit over the union of the corridors with every core of the map focal, and the cumulative current. `square` squares the resistances as Linkage Mapper's option. Output `links`: from, to, lcd, resistance (Circuitscape's times the cell's height, in cost units), cwd_to_resistance (lcd over it) and nodes. Tests against the library's Circuitscape on the corridors of the Linkage Pathways reference (`tools/make_linkage_reference.py`): `cargo test -p terraflux-ops --test ops linkage::`.
- **`linkage.barriers`, Linkage Mapper's Barrier Mapper in Terraflux** (McRae et al. 2012; `crates/ops/src/linkage.rs`): from `cores`, `resistance` and the kept rows of `linkage.pathways`' `links`, for each radius from `min_radius` to `max_radius` by `step` (map units) and each link, the improvement of restoring a circle around each cell: (LCD - the least distance of each core over the ring of cells at a distance above R - 1 and up to R map units - 2R times `restored`) / 2R (`restored` 1, Linkage Mapper's cost, by default), no value where a ring touches a core; the links' mosaic by `max` or `sum`, the negatives without value, the greatest over radii. Outputs `improvement` (the circles' centres), `circles` (each centre grown out to its circle), `percent` (the same as a percentage of the LCD) and `radius` (the one that gave the improvement). `cwd_threshold` keeps each core's distance only below it; Linkage Mapper's optional trim by the resistance is not made. Tests against numpy/scipy on the reference of `tools/make_linkage_reference.py` (key `barriers`): `cargo test -p terraflux-ops --test ops linkage::`.
- **The template "Linkage Mapper: corridors, pinch points and barriers", its tutorial and the MCP guide**: in Templates > Connectivity, a habitat drawn by `nlm.gaussian_field`, its large patches as cores (`raster.sieve`, `raster.label`) and a resistance from its quality, then `linkage.pathways`, `linkage.pinchpoints` and `linkage.barriers` with four Plots (cores, corridors, current, improvement); it runs in well under a second. The tutorial "Linkage Mapper: corridors, pinch points and barriers" (Habitat networks) explains the three steps for ecologists and how this differs from Linkage Mapper (centres of cells, no polygon cores, no all-pairs pinch scenario, no trim by the resistance). The `connectivity` guide gains the workflow, the methods section a paragraph for the `linkage.*` operators (McRae et al. 2012) and the Document pane a sentence for each, and the README's Connectivity row lists Linkage Mapper.
- **`data.stac` masks clouds pixel by pixel**: new settings `mask` (`none`, the default and what it was; `auto`, chosen by the collection: `sentinel-2` gives `scl`, `landsat` `qa_pixel`, `hls` `fmask`, any other an error that says to set `none` or a kind; or `scl`, `qa_pixel`, `fmask`), `mask_classes` (a list of `cloud`, `shadow`, `cirrus`, `snow`; default cloud, shadow and cirrus) and `mask_buffer` (cells to grow the mask by, a square of side 2b + 1 on the grid; default 0). The mask asset of each item (found in any case: Earth Search's `scl`, the Planetary Computer's `SCL`) is read once, nearest, signed like the other assets and never a layer (kept in the cache folder as `mask-<item>.tif`), and the cells it marks are NaN in every asset of the item before the composite, which already skips NaN (so `first` gives the first clear value). No data and fill are always marked (SCL 0 and 1; QA_PIXEL bit 0; Fmask 255); SCL: 3 shadow, 8 and 9 cloud, 10 cirrus, 11 snow; QA_PIXEL: bits 1 and 3 cloud, 2 cirrus, 4 shadow, 5 snow; Fmask: bits 1 and 2 cloud, 0 cirrus (reserved in HLS v2.0), 3 shadow, 4 snow. An item without the mask asset is left out of every asset and the run says how many. The mask, its classes and the buffer are part of the cache's key. Tests: `cargo test -p terraflux-ops --lib stac`; live (`--ignored`): `earth_search_masks_sentinel_2`, `planetary_computer_masks_landsat`. The Data catalog's Sentinel-2 and Landsat presets offer it as **Mask clouds per pixel** (on by default, with *and snow* and *grow by* cells, 1 by default) and write `mask = "auto"`; the Python script of a model comments the mask, as the library does not mask yet.
- **`data` on a node: what stands for the user's data** (the first stage of Use my data): a node of any model may say that it takes the place of a data of the user's, `data = { what = "a land use map, the first date", kind = "classes", classes = { 1 = "forest", 2 = "agriculture" }, expect = "1 or more" }` (`crates/graph`: `Data`, `DataKind` with `classes`, `values`, `mask` and `stack`; the classes' codes are the keys, written as bare keys in TOML and as text in JSON, read back as whole numbers; `expect` is for values, a range or a unit). It is part of the record (it does not change what the node gives, and `unrecorded` leaves it out), is written only when given, and goes with the node through groups and the calculator made a group; the model's check warns of a `data` with no `what`, with classes on values or a stack, or on a node that gives no map or stack. The Node pane says *Stands for your data: ...* with its kind and classes (`1 forest · 2 agriculture`), edits it under *Your data* (what, kind, the classes a line each `1 = forest`, what the values are expected to be), takes the mark off, and offers *Mark as data of yours...* on a node that has none; the Changes pane tells a change of it; the Python script comments it. The MCP's `edit_node`, `add_node` and `write_calc` take `data` (an object, or null to clear), `show_model` shows it, the `adapt_template` prompt says to read it first and asks for the files by it, and the `record` guide explains it. The 56 templates mark their nodes of data (116 nodes in 49 of them, plus the Dinamica EGO, CLUE-S and PLUS ones built in Rust: the maps of classes with their classes and names, the surfaces with what they are expected to hold, the masks and the drivers' stacks; the neutral landscapes that only help make another map are left unmarked), and a test asks every template with an `nlm.*` node for at least one (but the two neutral landscapes' demonstrations). Tests: `cargo test -p terraflux-graph`, `cargo test -p terraflux-app --lib templates`, `--test ui a_node_standing`.
- **The engine of Use my data** (the second stage; no window yet, `crates/app/src/my_data.rs`): from a model, the data it draws (the nodes that stand for yours, the `nlm.*`, a `raster.read` or an input without a file) and, for each one chosen, a copy of the model with your file in its place: a model's input (so the data are checked as the data of any model are), a `raster.match` to the reference grid only where the file's grid or CRS differs (`near` for classes and masks, `bilinear` for values), a `raster.reclassify` of your table of classes (your class to the model's; a class not in the table has no data) and a `raster.legend` with the model's names, each with its note and why; the drawn node goes and what read it reads the end of the chain, what only it read goes too, and the `nlm.*` that stay are drawn on the reference grid. The data of "any classes" of Landscape metrics, Kinds of landscapes and the zones of Raster statistics now name no classes (your classes are kept). Tests: `cargo test -p terraflux-app --lib my_data`.
- **Use my data, the window** (the third stage; `crates/app/src/my_data_view.rs`): **File > Use my data...**, the command palette, the start page's **Use my data in a template** and a strip over the Model pane's canvas ("This model runs on drawn data", with **Use my data...** and **Hide** for this model) open it. With no drawn data in the model the window offers the templates to start one first. *Your data* has a row for each thing the model draws (what it is, its node, its kind): a file to choose, its band, **Keep drawn**, and the **grid of reference**; each choice is checked as soon as it is made, in a thread (the plan and the data's findings, shown under its row; a choice that cannot be made says why). *Classes* has, for each chosen map of classes or mask, a table of your classes (code, name from the file's legend, cells) against the model's, prefilled by name (else by code), and says how many cells go to no data; data of "any classes" keeps your classes as they are. *Review* lists what will be done, what was refused and the findings (an error stops **Apply**), with the counts of data replaced, nodes added and taken away. **Apply** is one step of undo named *Use my data*, seen in the Changes pane, and runs the model once its data is read (**Run it now**, on by default). What a model draws is worked out once for each revision of it, not on every frame. The Node pane says *any classes* for data of classes that names none. Tests: `cargo test -p terraflux-app --lib my_data_view`, `--test ui use_my_data`.
- **Use my data on the land change templates** (the fourth stage): on Dinamica EGO's classic model, CLUE-S and PLUS it does what the Wizard did for Dinamica, CLUE-S and ordered allocation. The two dates are two data: after the file of one, the other offers **The same file, band ...** (the next band; two files work too), its table of classes starting as the first's; a new page, **Parameters**, after *Classes*, has the model's exposed parameters (the form of the Parameters pane, `params_view::control`) and the settings the Wizard asked: steps between the dates and to simulate, buffer increment (automatic or a value), the Expander's share (as observed or a value) and isometry for Dinamica; the allocation (CLUE-S or **ordered** with the order the classes are served in, no elasticity then, as the Wizard's), the elasticity, the date of each map and the dates simulated for CLUE-S; the seed; and *the simulation starts from* the second date (what the templates do: the future) or the first (the observed period again, what the Wizard's models do). Where the bands are named by dates the steps, the dates and the order of the classes start from them (`my_data::derive`), the user's values stay, and the plan is made and checked again at each change (`my_data::settings`, `apply_settings`; the settings made are told in the review). The checks of `lint` follow the ports an input feeds (through `raster.match`, `raster.reclassify`, `raster.legend` and `stack.of`), not its name: the two dates, the reference grid and the drivers' coverage are found whatever the inputs are called (`$land_2000`), with the finding at the input's own name. An input of a map or a stack may carry `data` (what the file stands for; written only when given, in TOML and JSON): Use my data writes it where the chain has no node, so a second run and the strip know that the file is yours (the strip no longer shows on a model of your files only), the Node pane shows it, and *Make it a node* and *Make it an input* carry it. A test runs Dinamica EGO's, CLUE-S's and the ordered allocation's models from Use my data and from the Wizard on the same files and compares the simulated maps cell by cell (equal when the simulation starts from the same date and the template's allocation steps are given to the Wizard's model). Tests: `cargo test -p terraflux-graph`, `-p terraflux-exec --lib lint`, `-p terraflux-app --lib my_data`, `--test ui use_my_data`.

### Changed
- **The Wizard is now Use my data**: the toolbar's **Wizard** button is **Use my data**, in the same place, and it, the start page's *Use my data: a land change model from your maps* and the command palette open the window of *Use my data* at a first page, *Which model* (Dinamica EGO's classic model, CLUE-S, Ordered allocation, which is CLUE-S's template with its allocation set to ordered, and PLUS), with the Wizard's texts for each. The model chosen opens as its template and the window goes on to your data, which is checked as it is chosen, as any template's is: a file on another grid is put on the land use map's by a `raster.match` (the Wizard's *Resample* button), the classes are checked, and the settings of the Wizard's third step are the *Parameters* page. CLUE-S takes a file of three bands or more as its dates (the second date is offered *All N bands as the dates*: the demand reads the stack of the file, the dates come from the bands' names; no table of classes with it), where the Wizard took any number. `wizard.rs` and the Wizard's window are gone (the models it wrote are the same: Dinamica, CLUE-S and ordered give the same maps cell by cell, with the same data, checked in `my_data`'s parity tests). `terraflux new dinamica` does not change. Tests: `cargo test -p terraflux-app --lib my_data`, `--test ui use_my_data`, `--test ui the_land_change_entry_points`.

### Fixed
- The links open in the browser on Windows: the pages of the Help menu, the start page, the operators' and the tutorials'
  pages, Report a problem (`explorer.exe <url>` opened nothing on some systems: the shell's `ShellExecuteW` now, `rundll32`'s
  URL handler if it fails), and the links of the widgets (the Data catalog's home pages, Methods' *What to cite*), which
  nothing opened. The start page has a *Close* button (and Esc): the bare canvas until Terraflux starts again;
  *Help > Start page* shows it again.

## [1.54.0] - 2026-11-16

### Added
- **The memory a run needs, told before it runs**: a model's memory is estimated from the headers of the rasters it reads (no data is read): a map is a number of 8 bytes a cell, each node's maps take the cells of its main input's grid (`like`, `map`, `raster`, `stack`) or its file's or its `nlm.*`'s `rows` x `columns`, a stack its layers, a series a map a step, a table or a vector its file's size (a vector twice); the peak adds the outputs the cache keeps of every top-level node, the maps the inputs hold and those a `raster.read` gives to a node that reads it whole (none when the nodes that read it work by `block_rows`, or `landscape.mspa_tiled`, or the metrics of a map of more than 250 million cells), what the bodies of loops keep (the iterations, up to the 1 GiB the cache keeps), and the largest node's output again; what the nodes' outputs held at the end of the last run of the same model (the journal's new `memory`) replaces the estimate of them, and the text says "from the run before" or "estimated from the model". The memory of the computer, total and available, is read on Windows (`GlobalMemoryStatusEx`), Linux (`/proc/meminfo`) and macOS (`sysctl`). Above four fifths of the available memory the Problems pane and `check_model` warn (`memory::advice`, yellow) with what to do: the nodes that weigh most, `block_rows` on the nodes that read a map whole and take it, the tiled engine (`terraflux run --tiled`) for the model it runs, a check run at 1/4 resolution; above all of it, the whole run asks first (*Not much memory*: **Run anyway** or **Cancel**, nothing is refused). A batch counts each run's memory times the runs at the same time (the workers) on this computer: its cost line says it, and **Run** asks and tells how many workers fit. Tests: `cargo test -p terraflux-app --lib memory::`, `--test ui a_run_that_may_not_fit_the_memory_asks_first`.
- **A Fix button on the common problems of the Problems pane**: what has a safe correction is a click away, as one step of undo (the checks run again after it), and the button's hint says exactly what changes. The maps a node was given on different grids (the run's failure explained): a `raster.match` is added before each one that differs from the first, on the first's grid, `near` when its cells are whole numbers (classes), `bilinear` when they are continuous, and two buttons (**Fix as classes**, **Fix as continuous**) when the cells say nothing. A map read without its no-data value, when the data's finding says to check it (and a `raster.read` the failure names): a field for the value, then **Fix** sets it (the input's or the node's `nodata`). A node not filled in: **Use $input** for each model input of the type the port takes, and **Fix** opens the file dialog and adds a `raster.read`, `vector.read` or `table.read` (by the port's type) connected to the port. A problem without a safe correction has no button. `fixes.rs` has `fix_for` and `apply` (on a copy, refused when it does not apply or brings an error). Tests: `cargo test -p terraflux-app --lib fixes`, `--test ui a_fix_puts_maps_on_one_grid`.
- **The model's graph as a figure for a paper (SVG, PDF and PNG)**: File > *Export the model as a figure...* (and the command palette, and *Export as a figure...* in the menu of a selection or of a group) draws the graph as the canvas does, in the light theme on white: the boxes with the id and operator, the ports, the wires in the colour of their type (darkened to 3:1 on white), the groups with their inside framed and titled under the graph, the notes and frames, and the rows of settings. The text is 4.5:1 or more on every fill it sits on (tested), written as text in SVG and PDF, in the figure's own typeface with sans-serif as the fallback. The width is 85 mm (a column), 180 mm (a page) or the natural size, cropped to what is drawn plus a margin; a figure whose smallest text would be under 5 pt is told so. `terraflux figure model.toml -o graph.svg --width 85mm` does the same from the command line (`.pdf`, `.png`; `--nodes`, `--group`, `--no-notes`, `--no-settings`, `--no-groups`). It shares the maps' figure drawing (`figure::Drawing`), so the three formats come from one layout. Tests: `cargo test -p terraflux-app --lib graph_figure`.
- **A model packed with its data in one file**: File > Pack for sharing... and `terraflux pack MODEL [-o FILE] [--outputs]` write a `.tfpack` (a zip file, so it opens in any system) with the model, its file paths all made relative into `data/`, the files its nodes read (a shapefile with its `.shx`, `.dbf`, `.prj` and `.cpg`, the rasters of a folder or a pattern, tables, plugins, the `control.submodel` files with their own data, recursively), optionally the outputs of the last run, and a `manifest.json` (version of Terraflux, date, each file's SHA-256, size and original path). Two files of the same name from different folders do not collide (`data/2/`); a file the model names that is not there is listed and recorded in the manifest, not skipped in silence; the window and the command tell the total before writing and warn above 2 GB. File > Open package... (also a `.tfpack` dropped on the window) and `terraflux unpack PACKAGE [-o FOLDER]` unpack it beside the package, check every file against the manifest (a changed, damaged or missing file is listed and the application does not open the model) and open it: the unpacked model gives the same outputs as the original. The zip format is written by Terraflux itself (deflate, ZIP64 past 4 GB) with `flate2`, already in the build. Tests: `cargo test -p terraflux-exec --lib pack`, `-p terraflux-exec --test exec pack::`, `-p terraflux-cli --test pack`, `-p terraflux-app --test ui pack_ui`.
- **Find and replace in the whole model (ctrl+H)**: **Edit > Find and replace...**, the palette and the shortcut (`find_replace`, remappable in Settings) open a window that looks for a text (optionally matching case, as a whole word, or as a regular expression with `$1` groups) in the settings of every node (inside groups, loops and switches), the files they name, the scripts of the calculators and the rules of a reclassification, the notes, and the names and files of the model's inputs and outputs, filtered by where to look and by operator. The preview lists every occurrence (the node with its scope's path, the setting, the text with the match struck out and what it becomes), each with a box (all checked), and a click on a row goes to the node. **Replace selected** applies everything chosen in one step of undo, all or nothing and checked as an edit is (a number made a text, an input name that is not valid or taken, an error the model did not have: refused with the reason and the place); a number is compared as its text and written back as a number (`-9999` to `0` in a no-data value), and after a change of folder (`D:/dados/` to `E:/projeto/dados/`) the files not found are listed. The core is `replace::find` and `replace::replace`, tested apart. Tests: `cargo test -p terraflux-app --lib replace`, `--test ui find_and_replace`.
- **Plugins beyond the cell: windows, whole maps, several outputs and tables**: `plugin.wasm` has an `interface` setting, `cell` (the default: nothing changes for the models that exist), `window` and `map`. `window` (with `radius`, 1 to 255) writes the (2r+1)² cells around each cell of every input into the module's memory (row by row, NaN outside the map) and calls `window(ptr, side, n_inputs, row, col) -> f64`, in parallel pieces as `cell`. `map` calls `map(rows, cols, n_in, inputs, n_out, outputs) -> i32` once, with arrays of `f64` for every input and for the outputs the host fills with NaN (0 is ok, another number an error with its code), and the names in `outputs = ["sum", "diff"]` become the node's map output ports (the registry's `outputs_listed`, as `raster.calc` has its `out` lines). Tables go in as columns (descriptors with the data, the rows and the name) and come out as `table_outputs = {name = [columns]}`, the module allocating each column and writing its pointer and rows in the descriptor. Both need the module's `memory` and `alloc(bytes) -> i32`, still no imports; `fuel` sets the instructions per cell (up to 100 million), and a `map` over more than 3.5 GB (a 32-bit memory) is refused with a way out. The check refuses an unknown interface, a `radius` below 1, repeated outputs and the like before the run. The tutorial has the ABI and two `.wat` examples. Tests: `cargo test -p terraflux-ops --lib plugin`, `-p terraflux-exec --test main plugin::`.
- **Almost every operator in the Python script of a model**: File > Python script (`terraflux python`, the MCP's `python_script`) now writes the machine learning operators as scikit-learn's models and inspection (`ml.fit`, `ml.predict`, `ml.importance`, `ml.partial_dependence`: Terraflux's trees are scikit-learn 1.9's, the same defaults and seed), `sdm.maxnet` as the library's `MaxNet`, Biodiverse's randomisations and properties, the metrics of sample plots, and `data.stac` as `tf.download.stac`. What the library has no function for is written into the script as small numpy and scipy functions that compute what Terraflux does: the statistics of maps and stacks, moving windows, majority, sieve, label, expand and shrink, the classes by breaks, gdaldem's terrain (slope, aspect, hillshade, TRI, TPI, roughness), a map's legend, the layers of a stack, the checks (an `AssertionError`, or a warning with `warn`), `value.calc`, numpy's generator, the agents and `batch.run`; a submodel becomes its body in place. Checked against the operators on 103 cases (no difference) and the terrain against gdaldem 3.12. Still comments, saying why: GDAL's own tools (fill, mosaic, contour, proximity), Jenks's breaks, R's generator, CRAFTY, and the steps the library runs only within a whole model.
- **The releases are signed, and Terraflux checks the signature before it installs an update**: the release's `SHA256SUMS.txt` is signed (ed25519, the private key in the repository's `UPDATE_SIGNING_KEY` secret; the release fails without it) and published as `SHA256SUMS.txt.sig`; the application checks it against the public key built into it before it trusts the checksums, and a release with no signature or a wrong one is refused with a message, nothing installed (`update::verify_sums`, tested with good, altered and foreign signatures).
- **An apt repository on the website**: `https://sacridini.github.io/terraflux/apt` (`stable main`, amd64) holds the latest release's `.deb` with its indexes, signed with a GPG key (`InRelease`, `Release.gpg`; fingerprint `E549997CCBAB309D887B06FF07FE80E05948E926`; the secret `APT_GPG_KEY`), built by `website.yml` (without the secret the site is published without it). The install page tells how to add it, and `sudo apt update && sudo apt upgrade` updates Terraflux.
- **STAC catalogues: `data.stac`** reads the scenes of a collection of a public STAC catalogue (Earth Search, Microsoft's Planetary Computer, any API) over a grid: the catalogue's search (`POST /search`, the pages followed up to `limit`) over the grid's box, the `datetime` and a `query` on the items' properties (`{"eo:cloud_cover": {"lt": 20}}`), then each `assets` name of each item read by GDAL (only the part the grid covers), warped onto the grid of `like` (or `extent` and `cellsize`) and the items combined cell by cell with `composite` (`median`, `mean`, `min`, `max`, or `first`, the first item with a value). A layer per asset, named by it, kept in the cache and made once. `sign = planetary_computer` signs the files with the Planetary Computer's token (asked without a key, once per storage container). Errors say what failed: the catalogue unreachable or refusing (its HTTP status and answer), no item over the grid, an asset no item has. The result is cell for cell the one of the Python library's `tf.download.stac` for the same request (the same items in the same order, no-data handling, resampling by `classes` and grid; checked on a window of Sentinel-2 of Earth Search: identical values). In the data catalog, a **STAC catalogue** card (theme *imagery*) adds the node with presets for Earth Search (Sentinel-2 L2A, Landsat Collection 2, the Copernicus DEM) and the Planetary Computer (Sentinel-2, Landsat, 10 m land cover by year), the assets, dates and cloud cover to choose, and the collection's reference; the cell sizes of the box now include 1 arc-sec (~30 m) and 0.3 arc-sec (~10 m). Tests: the search's body, pages, signing and composites run offline; `cargo test -p terraflux-ops --lib stac:: -- --ignored` searches Earth Search over a few hundred metres of Barcelona.
- **A submodel node tells what its file says about itself**: in the Node pane of a `control.submodel` node, the model's `summary` (or `description`) as its words, each of its inputs with its type and value (marked *given* when the node gives it) and each of its outputs with the node that gives it (`terraflux_graph::submodel::about`). A setting inside a submodel can read one of its inputs (`"$width"`, `{ from = "$width" }`), given by the node or kept at the submodel's own value: tested, and the tutorial no longer says it cannot.
- **The MCP asks for your files when it starts a model from a template**: a client that declares `elicitation` is shown a form (`elicitation/create`) after `new_model` with a `template`, a field for each node the template draws with a neutral landscape (`nlm.*`), for the path of your file; each node whose file exists becomes a `raster.read` of it (what read its `map` reads its `raster`), the paths that do not exist are told and their nodes stay as drawn, and declining or cancelling keeps the template as it is. A client without `elicitation` is not asked (the same as before). Tests with a client that answers, one that declines and one without `elicitation` (`mcp::tests::the_user_gives_the_files_of_a_template`).
- **Advice on 39 more operators in `describe_operator`**, taken from their documentation and the tutorials (88 operators with advice now): raster.reclassify, raster.resample, raster.zonal, vector.zonal, vector.buffer, vector.rasterize, vector.overlay, vector.spatial_join, change.suggest, change.crosstab, change.transitions, change.components, change.intensity, validate.kappa, validate.agreement, validate.error_budget, markov.matrix, sdm.maxnet, sdm.background, sdm.threshold, sdm.evaluate, sdm.mess, prioritize.zonation, prioritize.marxan, prioritize.evaluate, metrics.window, metrics.series, landscape.parcellation, landscape.spa, cost.geo_correction, cost.acc_cost, cost.passage, hydro.snap, hydro.upstream, accuracy.sample_size, accuracy.sample, accuracy.assess, sensitivity.design and sensitivity.analyze.
- **The assistant's evaluation with and without Terraflux's help, and its history**: the `plain` mode does the same tasks without the method's advice of `check_model` and `run_model`, the operators' advice of `describe_operator` and the long instructions (`tools::set_plain`), to measure what that help is worth. Every evaluation is kept (the model, the mode, Terraflux's version, the date, and for each task passed or failed, the seconds and the tools), not only the last of each model (the file of the older versions is read as one each), and `eval::compare` gives two as a text table. The HU Berlin's server is asked at most 12 requests a minute (one every 5 s). The ignored test `eval::tests::hu_berlin_does_the_tasks` runs the tasks in both modes (in a temporary folder, not the user's) and prints the comparison. First result with the HU Berlin's Qwen (llm3), one run each: 6 of 9 tasks without the help, 7 of 9 with it (only "a model fixed" differs, within the noise of one run).

### Changed
- **A Terraflux installed from the apt repository says to update with `sudo apt update && sudo apt upgrade`** in the update window (the update by the package manager and `pkexec` still works).

## [1.53.0] - 2026-11-15

### Added
- **Most of the operators in the Python script of a model**: File > Python script (`terraflux python`, the MCP's `python_script`) now writes about 85 % of the operators as calls of the library's function of the same method (`import terraflux as tf`), up from about a third; the others stay a comment saying why. New: connectivity (habitat graphs and their analyses as `tf.conn.HabitatGraph`'s methods, ProtConn, ECA change, metapopulations, Conefor files; Circuitscape and Omniscape; GuidosToolbox's networks), costs and distances (gdistance's transitions, accumulated and shortest paths, commute and RSP distances, passage; euclidean distance, influence zones and proximity), movement ecology (amt's tracks, filters, steps, distributions, random steps, covariates, summaries), species distributions (occurrences cleaned and thinned, folds, datasets, predictors, bias, migration, area of habitat, evaluation, Boyce, thresholds; `ml.potential` as the library's `Estimator`), prioritization and restoration (boundary, adjacency, connectivity and proximity matrices, units, evaluation, calibration, actions, Marxan, ProxIRR, Zonation projects; restoration's status, costs, corridors, isochrones, ecosystems and plan), terrain, water and ecosystem services (surface metrics, geomorphons, hilltops, peaks; snapping, stream links, upstream, DCI; InVEST's carbon, habitat quality, NDR, SDR, water yields, pollination; emissions; viewshed), land change (crosstabs, intensity, components, MAD, memberships, the change maps, Markov, regional demands, dynamic variables, Dinamica's patches, FUTURES' pressure and library; agreement, kappa, similarity, error budget) and the rest (accuracy, biodiversity, REDD+, statistics, uncertainty, sensitivity, fragmentation, neutral landscapes, window metrics and signatures, the Forest Code, downloads; rasters clipped, matched, resampled, reprojected, zonal, cell areas). Tables keep Terraflux's column names, so the nodes after them read the same columns; settings with no counterpart are written as comments. Left as comments: agents, checks, stacks, random generators, the general raster tools (focal, sieve, majority...), and the steps the library runs only within a whole model (MOLUSCE, CLUMondo, FUTURES, CARS' demand). The counterparts live in `crates/app/src/python/` (one file per family), and a test checks every call and argument against the library's API (`website/data/python_api.json`).

### Fixed
- **The Python script of a prioritization solved by Gurobi** passes `n_threads=` (it passed `threads=`, which the library refuses); pandas is imported once in the script's header.

### Added
- **Colour vision in the charts, and a warning for classes that look alike**: View > Colour vision now draws the charts too (the table viewer's charts, the area of the classes along the steps, series, calibration, profiles, weights of evidence, sensitivity charts, the gain and loss bars), not only maps and legends. A legend of classes warns when two colours are closer than 10 (CIEDE2000) to normal vision or to the vision chosen ("forest and pasture look alike for deuteranopia"), with a button for colour-blind safe colours (Okabe and Ito's, also in Colours as "colour-blind safe classes"); worked out only when the colours or the vision change. Tests: `cargo test -p terraflux-app --test vision` (a chart drawn in both visions, pictures in `target/tmp`).
- **Export to QGIS into one GeoPackage**: a choice in the window, *Layers: one GeoPackage* (the default stays the folder of files): every vector layer a table and every map a raster table (whole numbers as 16-bit tiles, other values as Float32 tiles, the missing cells and the coordinate system kept) of a single `<project>.gpkg`, the `.qgz` (or `.qgs`) pointing into it (`GPKG:./file.gpkg:table`, `./file.gpkg|layername=table`) with the same colours and legends. Off, with the reason, when GDAL has no GeoPackage driver.
- **The figure's font, and its legend and scale bar placed by hand**: Figure... has a **Font** (Source Sans 3, Inter or the application's Ubuntu Light) and a **Text size** (7.5 pt as before; the title, captions, coordinates, north arrow and scale bar follow it), in every format: the SVG names the font, the PDF embeds it (text that can be searched and copied; it was Helvetica) and the PNG draws its letters. The legend and the scale bar stand where they fell, in a corner or at a place of your own (x and y in percent), or are dragged in the preview (they snap to a corner); a legend by hand goes over the figure (over each map when each has its own), and nothing leaves it. The choices are kept in the settings for the next figure; older settings read the same.
- **Google Cloud Storage's public buckets can be read without keys**: the cloud settings have the box "public buckets, requests not signed" for `gs://` as they had for `s3://` (`GS_NO_SIGN_REQUEST`). Ignored tests read a public COG of S3 and a public GeoTIFF of GCS by `raster.read` (`cargo test -p terraflux-app --test app cloud_public:: -- --ignored`).
- **A group's outputs can be renamed and ordered** in its panel: each output's name is a field (Enter renames it, and what reads `group.old` follows: nodes, settings, the model's outputs, the Plots; refused when the name is not a port name or is taken) and ↑ ↓ move it in the order the group's box shows its ports. One step of undo each. The order is an optional `order` list next to `outputs` in the body; older files read the same and older versions ignore it.
- **The application's pictures redrawn** in the theme Slate: the README's, the calculator's and the tour's (the window, the Templates menu, a node's settings panel, now drawn off screen by `tour_images` instead of cut from desktop captures); their texts describe the drawers, the boxes' settings and the panel beside a box.
- **The Settings window opens in the middle of the screen** (it stays movable).
- **A setting's wire has a tip, points and can be drawn from the socket.** The pointer resting on the wire of a
  setting that reads a value tells what it feeds ("→ node.setting"), the type and the last value, as a connection's
  tip does. A wire dragged from the socket of a free setting and let go on an output that fits it makes the setting
  read that output (the same check as a wire dropped on the socket; one that does not fit is not made). A double
  click on the wire adds a reroute point, which is dragged, removed by a double click or right click, and saved with
  the node's `wires` under `setting:<name>` (no port has such a name: a model without them is read and written as
  before); the points go with the wire when the setting gets a value of its own or another wire.

### Changed
- **The settings panel scrolls to the setting you clicked.** Opened from a row the box does not edit (or from the
  node's menu on a row), the panel beside the box scrolls to that setting, unfolds Advanced if it is there, and marks
  it with an outline that fades; opened from the gear it stays at the top.
- **A click on the row of an open list closes it.** On the box, a click on the row of a setting with a list of
  values opens the list, and a click on the same row again closes it (it reopened it before).
- **Dragging a node in a big model is smooth.** In a model of 10,000 nodes a frame of a node being dragged took 31 ms
  and now takes about 5 ms (1,000 nodes: 2.8 to 0.5 ms): the wires are not gone around the box that is dragged, so
  once it is left out of them only the wires at its ends are made again, and the wires near it are looked for by
  halving their curves. The wires look the same, and when the node is let go they are as a new look at the whole
  model finds them.

### Fixed
- **The map no longer drops a row after an edit.** The Viewer's header put its buttons on a second row when the status beside them grew (to "nothing changed: from the cache" once the run an edit asks for had ended), moving the map down by 24 px; the choice of one row or two no longer depends on the status.

## [1.51.0] - 2026-11-15

### Changed
- **Thinner top bars.** The tab bars of the panes are lower (21 px in Classic, 25 in the modern themes, from 24 and 30) and so are the title bars of the windows (about a quarter lower: a smaller title text and less padding above and below it, the sides as before).
- **Settings, in sections.** The Settings window is a fixed size (it fits small screens) with a column of sections at the left (Appearance, Model builder, Runs, Performance, Cloud storage, Node libraries, Keyboard, About & updates) and the chosen one at the right, each with a heading and small named groups; on/off settings are switches in the modern themes. "Back to the defaults" stays at the foot of the column, the GDAL line moved to About & updates, and the update button is orange (a dot beside About & updates tells a new version is out).
- **Prepare in a drawer.** The Prepare pane no longer comes open by itself with the first data: a PREPARE tab waits at the window's left edge, under OPERATORS and FILES, while the model has data and the pane is closed (the pointer over it brings the drawer out, a click opens the pane docked). It still opens from the View menu.
- **Themes.** The theme named *Default* is now *Classic* (it was not the default any more: Slate is); a settings file that still holds "Default" reads as Slate. Three new modern themes: *Graphite* (the old greys and orange, modern), *Ash* (light greys and orange) and *Midnight* (deep navy, violet accent). The Settings list shows the modern themes first, then the classic, under two small headings, and Slate as "Slate (default)".

### Removed
- **Portuguese output.** Documents, reports, methods, guides and the search of operators and templates by Portuguese words, and the `--language` option of `report` and `methods`; Terraflux is in English only.

## [1.50.0] - 2026-11-15

### Added
- **A node's main settings on its box.** Under its ports a box shows a row for each setting the operator needs,
  each one given and its essential ones (four at most): the name at the left, the value at the right (cut to the
  box; a default the operator's doc tells in grey; a setting that reads another value as `← $name`), a setting the
  check has a problem with in red or amber. The others are counted in a row, "+ 5 settings · 2 given" (a setting
  you changed is never hidden unseen), that a click unfolds and folds again. The boxes are taller by the rows, and
  Arrange and the automatic layout make room for them. *Settings* has an option, "a node's main settings on its
  box", to have the boxes as before. A click on a row (a drag on it still moves the node) selects the node and
  edits the setting there: a yes or no turns over at once, a setting with a few values opens their list under the
  row ("default" first when it is optional, the current one marked), and a number or a text opens a field over the
  value (Enter or a click elsewhere sets it, Tab goes to the next row, Esc gives up, an empty field puts the
  default back). Each edit is one step of undo, set as the Node pane sets it, and the canvas's keys wait while you
  type. What reads another value, a file, a script, the metrics, a table of rules or a list is not edited on the
  box: a click on its row opens the node's settings panel.
- **A node's settings in a panel beside its box.** The Node pane's content in a floating panel at the right of the
  selected node's box (at its left when there is no room), inside the canvas, following the box as the view or the
  box moves; it shows the node selected (another node clicked: it moves there; several selected: the pane of them
  all, beside the first) and has a scroll of its own. A small gear at the right end of each box's operator line
  opens it, as does a click on a setting row the box does not edit (a file, a script, a list...), shift+Enter with a
  node selected (*Open the settings of the node selected beside its box* in Settings, Keyboard; Enter still shows
  the node's output) and *Settings...* in the node's right-click menu. It closes with its x, Esc, a click on the empty
  canvas and when you go into or out of a group. Closed it costs nothing; open it costs what the Node tab does.
- **A setting as a pin.** A setting that takes a number, a whole number, a yes or no or a text has a small hollow
  socket on its row at the box's left edge (filled, in the colour of its type, when it reads a value). A wire
  dragged from an output, from a model's input box or from a container's value and let go on the socket, or anywhere
  on the row when its type fits, makes the setting read it (`{ from = "node.port" }`, `"$name"` or `"@name"`),
  checked as a connection is (a value that does not fit is refused with the reason, the model left as it was), one
  step of undo; the row is lit while a wire is over it. The reading is drawn as a wire from the output to the
  socket, in the colour of the type and bent around the boxes, following the boxes as they are dragged. A wire
  dragged off the socket, or *Use a value of its own* in the row's right-click menu, takes it off (the setting goes
  back to its default). A setting that reads a value is always shown on the box, never folded or hidden.
- **Show or hide a setting on the box.** *Hide from the node* in a row's right-click menu leaves the setting off the
  box (it is folded in the row that counts the others, given or not, and shown when they are all unfolded);
  *Show on the node* in the node's right-click menu (a list of the settings the box folds) or in the menu of a
  hidden row puts it on the box, always, even beyond the four. The choice is kept in the node's `shown` and `hidden`
  (absent when empty: a model without them is read and written as before), one step of undo each.

### Changed
- **The Node pane no longer opens by itself.** The Node tab did not need to come beside the model with its first
  node now that the settings are on the box and beside it; it stays in the panes' menu, a layout kept with it keeps it,
  and *Settings* has an option, "open the Node pane by itself", to have it come as before (off by default).

### Changed
- **The Python library is now terraflux** (`pip install terraflux-py`, `import terraflux as tf`; it was landschaft, `import
  landschaft as ls`). File > Python script (the menu entry lost the "(landschaft)"), `terraflux python` and the MCP's
  `python_script` write `import terraflux as tf` and call `tf.`; the generated script's name `tf` is taken, as `ls` was. A
  GeoTIFF carries the product of a map as `terraflux_product` (the files that only say `landschaft_product` are still
  read, the new tag winning). The library's submodule is `extern/terraflux-py` (the repository is renamed
  terraflux-py), and the build reads `TERRAFLUX_PY_DIR` and `TERRAFLUX_EIGEN_DIR` (`LANDSCHAFT_DIR` and
  `LANDSCHAFT_EIGEN_DIR` still read); the C++ is built with the CMake options `TERRAFLUX_PYTHON` and `TERRAFLUX_MILP`.

## [1.49.0] - 2026-11-15

### Changed
- **An edit takes the status only of what it reaches.** Moving a box, a note, a frame or a wire's reroute point no
  longer makes every connection say "changed since the last run": a node keeps how it did in the last run unless its
  own settings or connections changed, it reads (all the way down, inside groups too) a node that did, or it was
  taken away; the nodes of another chain, and the other nodes of a group, stay as they ran.

### Added
- **Two wiring gestures on the canvas.** Inside an open group, a wire dragged from a node's output to the edge of the
  scope (a zone, *new output of the group*, drawn at the right while the wire is dragged) makes that port an output of the
  group, named as the port (`raster_2`... when taken; the same port again changes nothing), one step of undo; the
  group's outputs are shown as boxes at the right of its nodes. And a wire dragged from a free input, backwards, to an
  output that fits connects it; let go on the empty canvas it opens the search of the operators that give what the input
  takes, and the one chosen is added there with its output wired in (Esc cancels, a click elsewhere too).
- **Keys for Align, and groups that read in the light themes.** The selection's *Align* entries have shortcuts, shown in
  the menu and in the sheet of shortcuts (F1) and changed in Settings (Keyboard) like the others: alt+shift with the
  arrows for the left, right, top and bottom edges, C and M for the horizontal and vertical centres, H and V to
  distribute horizontally and vertically (three nodes or more), K and R to put them in a column and in a row; one step of undo each, as the menu. In the light
  themes a group's border and header now read at 3:1 on the canvas (its title at 4.5:1 on the header, its accent
  icon and count at 4.5:1 on the box), and the notes' and frames' colours are darkened to read on it; the dark themes
  are as they were.
- **Find a node in the command palette, and the camera glides to it.** *Find a node* (ctrl+K, with its ctrl+F) opens
  the canvas's search from anywhere; going to a node from the search, and from a node of the Problems pane, moves the
  camera with the same eased glide as F (none when animations are off), instead of jumping.
- **Edit the features of a vector layer already read.** *Edit the features* in the menu of a vector layer's row in
  the Layers (a layer a `vector.read` node reads from one file, all of it, in the map's coordinate system): its
  vertices are dragged, the dot amid an edge adds one, a right click takes one away, *Add a feature* clicks a new
  one, × in the table of attributes deletes one, the attributes are typed (a number stays a number, an integer field
  an integer), Undo, Redo and Ctrl+Z take edits back. Rings with holes and multipart features are kept as read; what
  is not touched is written as it was. *Save the edited features* writes a new GeoPackage (`<name>_edited.gpkg`
  beside the file, or where *Browse...* says) in the layer's coordinate system, read by a new `vector.read` node;
  the file read is never written. `write_vector_as` and `integer_fields` in the GDAL crate keep integer fields.
- **WMTS services as raster sources.** An address in GDAL's own syntax, `WMTS:https://host/WMTSCapabilities.xml,
  layer=name` (and `tilematrixset=`), is a raster for `raster.read`, *A URL...* in the Layers and the Viewer's header
  (it is checked in a thread: size, bands, coordinate system, or why not) and anything else that reads a raster from
  the web; it is kept as it is (never made `/vsicurl/`, never relativized, never checked as a file), read by windows at
  the level that fits the screen. `WFS:https://...` and `OAPIF:https://...` are web addresses of vector features the
  same way. The basemap stays XYZ.
- **`vector.read` with an `extent`, and remote features read by the view.** `vector.read` takes an optional `extent`
  (`[xmin, ymin, xmax, ymax]`, in the layer's coordinate system or in `extent_crs`) and reads only the features that
  meet it (OGR's spatial filter, `terraflux_gdal::read_vector_in`; a folder or a pattern, each file), useful from the
  command line too. In the Viewer, a vector layer on the web (`https://`, `/vsicurl/`, `WFS:`, `OAPIF:`, a bucket)
  added over a map is no longer run whole: the features that meet the view are read, in a thread, per box of the view
  rounded outward to a coarse grid, the last six boxes kept for each layer (a box seen again asks for nothing, the
  last one read drawn while the new one is). A node that was run (the whole layer) is drawn from its value as before,
  and local files stay read whole.
- **Credentials of S3, Google Cloud and Azure storage in the settings.** *Settings > General > Cloud storage*: AWS
  access key id, secret key (masked), session token (masked), region, endpoint (any S3-compatible storage; `http://`
  or `https://` accepted), public buckets without signing; Google's interoperability keys or a service account's
  `.json`; Azure's account and key. The secrets (secret keys, session token, Azure's key) are kept in the system's store of
  credentials, as the assistant's keys are, and the rest in the settings file (written for its owner alone on Unix);
  none is ever in a model, a script or the logs. They are given to GDAL as options for the whole program at the start and whenever they
  change, so the Viewer's tile threads and the runs' threads read `s3://`, `gs://` and `/vsiaz/` data with them. The
  command line and the workers use the environment's variables (`AWS_ACCESS_KEY_ID`...), as GDAL does by itself; a
  field left empty leaves the variable in charge. Vector files are opened by the same path as rasters.
- **The land-change Sankey over every step of a series.** In the window of a series' transitions (the "first to
  last..." button under the area of the classes along the steps), *Every step* draws a column of classes per step
  (the same classes in the same order at each, a class absent at a step keeping its row at zero height), the bands
  between consecutive columns in the colour of the class they came from, and the steps named under the columns;
  *First and last* is today's view. Three to eight steps open on every step, more on the first and the last (with a
  note), and the choice is kept in the window; above twelve steps only the first and the last. The matrix of each
  pair of steps is worked out in a thread of its own ("computing the flows of every step..." meanwhile). *Copy* and
  *Save CSV...* give one block per pair of steps, each row starting with `from_step` and `to_step`.
- **Maps of different coordinate systems are compared without `raster.match`.** The curtain, the views side by side
  (and the views more) and the difference carry a map of another system (UTM against geographic, say) to the map
  shown on the fly, as they did for layers. The camera and the part of the grid a calculation follows (`calc_view`)
  follow a map of another system too. A map with no coordinate system is told so ("The other map has no coordinate
  system..." or "This map has no...", in the views, the curtain and the difference, also in Portuguese) with what to
  do: give it one with `raster.reproject` (`source_crs` and `crs` the one it is in), and `raster.match` only to have
  the maps cell by cell.
- **The map's coordinate grid on a projected map is of longitudes and latitudes.** Lines of constant longitude and
  latitude (64 points each carried through PROJ, so they bend as the projection does; a rotated grid too), cut to the
  view and labelled (°E at the top or bottom, °N at the left or right) where they cross its edges, at round
  degrees chosen from the view. They are the map's own datum's (SAD69 / UTM has SAD69's lines, `geographic_of` in
  `terraflux-gdal`). A geographic map keeps its lines in degrees.
- **Go to takes WGS 84 on a map of another datum.** The degrees typed are WGS 84 on a geographic map too (SAD69,
  NAD83...): they are carried to its datum (the shift included), as on a projected one; the field's hover says so.
  The Figure's location inset is in WGS 84 whatever the map's datum (its hover says so). Measuring keeps the WGS 84
  ellipsoid for every geographic map (told in `measure.rs`).
- **A Plot's layer can be a node inside a container.** `g.inner.map` and `g/inner.map` (a group, a loop's body, a
  switch's branch, at any depth) are accepted by the check, which says what is missing when one does not resolve
  (`"g" has no node "gone"`), opened by the Viewer, drawn by `terraflux plot`, the report and the export, and taken by
  the MCP `set_plot`. A loop's node is its last step's from the command line, the step chosen in the Viewer.
- **The Figure and the export of a saved view no longer freeze the window on a big raster.** The part of a raster on
  disk is read by a thread (a "Reading the map" window shows meanwhile), and the Document reads the part its figure
  shows once in the background ("reading the map..."), not at every frame.
- **The Viewer's layers can be renamed, grouped and selected several at a time.** *Rename...* in a layer's menu (or a
  double click on its name) edits its name in the row: Enter keeps it, Esc gives it up, empty goes back to the
  output's name; the node keeps its own, and the new name is the one in the list, the legend, the figure and export
  of the Plot, and the views side by side (`name` of a layer in a `[[plots]]`). *Group* makes a group of the layers
  selected, as in QGIS (one level): its row has a fold arrow, a box that shows or hides its layers, an opacity that
  multiplies theirs and a rename; drag a layer onto the group's row to put it in, out of its rows to take it out,
  *Ungroup* keeps the layers (`groups` of a Plot and `group` of a layer). A Ctrl click adds a layer to the selection
  or takes it away, a Shift click selects the range from the last one clicked; the box of one turns all, a drag
  moves them as a block, *Group* and remove act on them all (the properties show the first). The list's edits live
  in the session, as visibility and opacity do; saving the Plot makes them model edits, with their undo.
- **An input's box can have copies in its scope, as Blender's several Group Input nodes.** Duplicate (the box's
  menu, Ctrl+D) on the box of a model's input (`$name`), a container's value (`@step`, `@item`) or an outer node
  makes another box of the same reference below it, kept in the model's places as `scope|reference#2`, `#3`... (the
  first keeps its key, so old models load the same; `$`/`@` in the model are unchanged). Each node takes its wire
  from the nearest box, so wires need not cross the canvas; a copy no node is nearest to stays, with no wires.
  Deleting a copy takes only that box; deleting the last still takes the input and disconnects what read it.
  Undo covers both; a copy shows the input in the pane.
- **The portable Windows copy updates itself.** The `windows-x64.zip` now holds an empty `portable` file; a copy
  with it in a folder you may write downloads the release's zip, checks it against `SHA256SUMS.txt`, and after
  Terraflux closes replaces the files of its folder (yours stay; a file in use is renamed `.old` and removed at the
  next start) and opens again with the model.
- **Sensitivity analysis: classes by name.** The class of a share, area, cells or metric of a map is chosen from a
  menu of "value – name" entries when the output has a legend (a `raster.legend` node or a GuidosToolbox product's
  classes), with "Other value..." to type a number; the methods text tells it as "forest (1)".
- **Forest Code art. 61-B:** `forest_code.app`, with `properties` and `consolidated`, caps the recomposition of the
  consolidated APPs (rivers, lakes, springs, veredas) at 10% of a property of up to 2 fiscal modules and 20% up to 4,
  keeping the cells nearest the water; new outputs `recompose` (map) and `properties` (table: area, modules, cap,
  APP before the cap, recomposition, capped), and a warning for each capped property. `app` is unchanged.
- **Zonation 5's output files:** `prioritize.zonation` and `prioritize.zonation_project` take `output` (a folder) and
  write `rankmap.tif` (float32, NoData -inf, LZW with the floating-point predictor, tiled by 512 from 512 x 512
  cells), `feature_curves.csv`, `summary_curves.csv`, `feature_density_curves.csv`, `group_curves.csv` (with
  output groups) and `features_info.csv` as landschaft writes them: the same lines as Zonation's own files in 64
  cases (the means are Zonation's float32 sums, Eigen's order). `prioritize.zonation`'s `output_names` chooses the
  curves' header: the features' names (default) or their numbers, as z5 without names. With a project,
  `prioritize.zonation_project`'s `output` also writes the files of the post-processing calls
  (`post_processing/`: area_stats, coverage_matrix, highest_occurrences, solution_comparison, histogram, the masks
  and flexibility maps) and the impact assessment folder (`impacts/<scenario>/`: the source analysis per area,
  group, source and bin, the cumulative tables and the group maps), the same lines as Zonation's own files in the
  4 post-processing and 5 impact projects of landschaft's reference (the rasters bit for bit).
- **Advice and tips:** a metrics operator on a map of whole values but more than 100 classes (a map of ids, or a
  continuous quantity rounded) is advised to group them first; tips for `conn.metapopulation`, `conn.overall`,
  `conn.link_importance` and `conn.distances`.
- **`prioritize.optimize` with `solver = anneal`:** landschaft's anneal solver, Marxan's simulated annealing on the
  compiled minimum-set model (every pair column of a boundary or connectivity penalty a Marxan boundary), each run
  repaired to meet every target and trimmed, the best run's objective, the bound of the linear relaxation without
  the boundaries' cut terms, and the status `heuristic` (`optimal` when the gap closes). Settings with landschaft's
  names and defaults: `num_reps` (10), `num_iters` (1000 per unit, at least a million), `start_temp` (-1),
  `cool_fac`, `num_temp` (10000), `spf` ([1, 2]), `itimp_type` (0 or 3), `seed` (1) and `time_limit`. With a seed
  the same selection and objective as landschaft's (6 cases); zones, portfolios and other models than minimum sets
  with binary decisions are refused with landschaft's messages. The Python script writes `add_anneal_solver`.
- **Corridors in `prioritize.optimize`:** landschaft's `add_corridor_constraints`: the terminals of each network
  (`corridor_terminals`, a layer per network; or pu's columns `corridor_columns`) are selected and joined by selected
  units through a flow on the edges between units, with an optional resistance (`corridor_resistance`,
  `corridor_penalty`) in the objective. The model is landschaft's bit for bit (5 compiled models), solved as
  landschaft does (node cuts at the root and in HiGHS's tree: the same selection as landschaft's in the same time).
  The Python script writes `add_corridor_constraints`.

- **The table operators read text.** `table.filter` and `table.calc` take `cover == "forest"` (and `or` for a list)
  over a column of names, as `vector.filter` does over attributes (the same evaluator); a calc giving texts makes a
  column of names, one giving numbers in place of a column of names drops its names. `table.sort` sorts a column of
  names by the names, and `select`, `filter`, `aggregate` (the group keys), `pivot` (a column per name) and `join`
  keep the names; `join` matches names by their text, so tables with different dictionaries join, and the result
  merges them. `Table::concat` merges the dictionaries instead of keeping the first, `vector.from_table` gives text
  attributes, the sensitivity's `factor` column holds the factors' names and the rows of a `control.each` over a
  table keep the labels (a text first column names the row). The table viewer's condition takes `cover = forest`.

### Changed
- **`table.read` keeps the text columns**, as columns of names (codes 1, 2... by first appearance, with the names
  beside them) for CSV, TSV and JSON, instead of leaving them out; a column with any value that is not a number is
  all text, numbers among them as written. The reader is quote-aware (line breaks in quotes). `table.aggregate`'s
  default columns, the dist measure and the sensitivity's results take the columns of numbers only.
- **The nodes of a loop's body are kept by iteration.** A `control.repeat` or `control.each` that runs again (one
  more step, a changed collect, one layer of its stack changed) runs only the body nodes whose inputs changed, by what
  they read: the carried values, `@step`, `@item`, `@index`, `@name` and the outputs of nodes outside the body count
  by their content, not by where they are in memory. Nodes that are never kept (`raster.write`, random generators)
  still run at every step. The kept steps live in memory, at most 100000 nodes and 1 GiB of outputs, the least
  recently used let go first. A loop taken from the cache now tells its steps exactly as it told them when it ran.

- **Loops inside groups and inside loops in the viewer.** A body node of a loop in a group, or of a loop in a loop,
  shows the step chooser of each loop on the way (named when there are several): choose the outer step and then the
  inner one, and the viewer shows that pair's value, titled `outer/inner/node (step 2, step 3)`. A node inside a
  container shows how it did in the last run (running, done, cached, failed) by its full place on the canvas of its
  container.

### Fixed
- **Frozen tables are written and read back faithfully**: the CSV keeps the codes and the names of a labelled column
  go in `frozen.json` (old folders still read); the reader is quote-aware (quotes, doubled quotes, line breaks in
  quotes, CRLF) and names the row whose field count differs from the header; a table's CSV quotes a header with a
  comma, a quote or a line break as it does the cells.
- **A `control.each` taken from the cache reports its steps again** (a step and what it collected for each element, as
  when it ran), at the top level or inside a group.
- **A node frozen inside a switch's branch is held:** frozen places and the Freeze toggle go through `control.switch`
  branches (`switch/branch/node`), and a switch's branch keeps its nodes between runs as a group's. The branch itself cannot be frozen, only the nodes inside it.

## [1.48.0] - 2026-11-14

### Added
- **SWAT+ results on the map: `swat.hru_map`, `swat.channel_map`, `swat.reach_discharge`:** landschaft's `hru_map`,
  `channel_map` and `reach_discharge`, bit for bit (28 cases: each `how` (annual, mean, max, sum), variables of the
  four HRU tables and of the channel table, `c_factor` plant, values given by HRU or channel, crop rotations, the
  example landscape). An HRU's or a channel's value is painted on its cells (the project must come from
  `swat.from_tables`, which keeps the watershed); `reach_discharge` gives the daily discharge, a row per date and a
  column per channel. They read the tables of a `swat.run` of the same project, connected (`hru_wb`, `hru_nb`,
  `hru_ls`, `hru_pw`, `channel_sd`: the average annual one, step `aa`, for `annual`; the daily one for the others;
  a table of another step is refused with the `swat.run` setting to change), so several maps of one run simulate it
  once; with no table connected the node runs the project itself, as landschaft does without a result (`nyskip` and
  `c_factor` as `swat.run`'s).
- **`swat.watershed` divides a watershed as QSWAT+ does:** landschaft's `watershed`, bit for bit (18 cases: each HRU
  method, thresholds, outlets and snap, slope bands, areas in ha and in percent, exempt land uses, a usersoil table,
  land use on another grid; only the degrees differ, by 7e-15 at most, between the two PROJ versions). The DEM is
  filled and drained along D8 directions, streams come from a drainage area, each stream link is a channel with its
  subbasin and landscape unit, HRUs are selected as QSWAT+ does. New type `swat_watershed` (written as JSON, kept in
  frozen outputs) holding QSWAT+'s project tables and the grids of subbasins, channels and HRUs; the node's other
  outputs are those three maps and the table of the HRUs; the kernel `hydro_flow_distance` joins landschaft's
  hydrology bindings.
- **`swat.from_tables` builds a SWAT+ project from a watershed and the weather, `swat.wgn` the weather generator's
  statistics:** landschaft's `from_tables` (the SWAT+ Editor 4.0.2's GIS and weather import and file writers) and
  `wgn_from_series`, every file byte for byte with landschaft's (6 cases: its synthetic valley with dates, `codes`
  and a name, with crop rotations, with three stations of different periods and gaps and humidity, wind and solar
  radiation, with the generator given, and the example landscape; only the date on each file's first line differs)
  and the statistics bit for bit (numpy's pairwise sums, pandas' standard deviation). The weather is tables: the
  stations (`name`, `lat`, `lon`, `elev`) and one series per variable (a `date` column, as 2001-01-31 or 20010131,
  or `year` and `day`; a column per station; empty for a missing day); `monthly` gives the columns not derived from
  daily data (`pcp_hhr` at least). The project keeps the watershed it came from (serialized with it), so the chain
  `swat.watershed` → `swat.from_tables` → `swat.run` is landschaft's `from_rasters` and run, bit for bit.
- **A SWAT+ project is a value of the graph:** the new type `swat_project` (written as JSON, kept in frozen
  outputs); `swat.load` reads a TxtInOut folder or a zip (checked as `swat.run` does); `swat.set` changes a
  parameter of a project as SWAT+'s hard calibration does (a row in calibration.cal, and in cal_parms.cal when it
  lacks the parameter): `parameter` (the 69 the port calibrates), `change` (pctchg, abschg, absval), `value` (a
  model input `"$x"` for batches and calibration), the soil layers and the HRUs, the input project untouched, chained
  nodes making their changes in order, equal to landschaft's with the same calibration.cal; `swat.run` takes a
  `project` input instead of `path` (`path` still works; one of the two).
- **The template "SWAT+ from rasters" (group Surfaces and uncertainty) and its Python script:** a valley of 3 km by
  3 km drawn by `nlm` (terrain, land use, soils), the lookup and soil tables and two years of daily weather written
  in the model, `swat.watershed` → `swat.wgn` → `swat.from_tables` → two `swat.set` (the curve number and the water
  the soil holds, changed by a percentage that two model inputs, `cn2_change` and `awc_change`, give, ready for Many
  runs and calibration) → two `swat.run` (the HRUs' average year, the channels' days) → `swat.hru_map` (soil lost,
  runoff), `swat.channel_map` and `swat.reach_discharge`; its maps and discharge are those of the same project in
  landschaft 1.58.1. `terraflux python` and the `python_script` tool write the nine `swat.*` operators as
  landschaft's `ls.hydro.swat` (`load_txtinout`, `watershed`, `climate` with `from_tables`, `wgn_from_series`,
  `run`, `hru_map`, `channel_map`, `reach_discharge`; `swat.set` as a helper in the script that adds the
  calibration.cal row as Terraflux does), none left as a comment. To make it run as it is: `table.of` takes names
  (text columns) as well as numbers, and the drawn maps of `nlm.*` take a `crs`. A new guide `hydrology` for
  assistants (terrain hydrology and SWAT+, in English and Portuguese).
- **The tutorial "A SWAT+ project from rasters, and its results on the map"** (Terrain hydrology): the division
  into subbasins, channels and HRUs with the stream threshold and the HRU method, the weather and the generator's
  statistics, the project, a parameter changed by a model input, two runs and the maps; the tutorial "Watershed
  simulation with SWAT+" points to it and tells of `swat.load` and `swat.set`.
- **`prioritize.calibrate` solves on Gurobi:** new settings `solver` (highs or gurobi) and `threads`, as
  `prioritize.optimize`'s: the candidates solve in parallel with one Gurobi thread each, as landschaft's
  `_solve_all`, and the ideal solves (Cohon's two, the least cost) with the node's threads; HiGHS and one warning
  where Gurobi is not installed or licensed.
- **`restoration.plan` solves on Gurobi:** new settings `solver` (highs or gurobi) and `threads`, as
  `prioritize.optimize`'s: every linear program (the exact plan's rounds, the steps, the minimum cost, the
  trade-off curves and the sets of weights) solves with the node's threads, as landschaft's `_solve_lp`; HiGHS
  and one warning where Gurobi is not installed or licensed.
- **`prioritize.actions` and `prioritize.actions_raster` solve on Gurobi:** new settings `solver` (highs or gurobi)
  and `threads`, as `prioritize.optimize`'s: with Gurobi, curve 1 goes as the equivalent model (the benefit columns
  substituted, one activation row per action) and curves 2 and 3 as prioriactions's model with the activation rows
  per action and the response curves as pieces, as landschaft's `ActionsProblem.solve`; the same optimum as
  HiGHS's at gap 0. HiGHS (the closure engine first, unchanged) and one warning where Gurobi is not installed or
  licensed.

### Fixed
- **The folder of a SWAT+ project travels with the model and is watched by the cache:** the `path` of `swat.run`
  and `swat.load` was neither sent to the workers of a distributed batch nor considered when a run's cache decided
  whether a node was up to date. A TxtInOut folder is now sent whole (every file `swat.load` reads, not SWAT+'s
  outputs or hidden files), a zip as a file, and a change in one of those files runs the node again.
- **A changed table or track file runs its reader again:** the run cache looked at the files of `raster.read` and
  `vector.read` only, so after editing the file of a `table.read` or `move.read_tracks` a run gave back the old
  table from the cache.

## [1.47.0] - 2026-11-14

### Added
- **Minimum sets with zones and pair terms solve as landschaft solves them:** `prioritize.optimize` with zones,
  boundary penalties or connectivity rewards (in Marxan's form) now goes to landschaft's zones cut engine
  (`opt/zones_cut.cpp`: a Lagrangian bound by minimum cuts, repaired selections) before HiGHS, and on to HiGHS's MIP
  from its plan when the gap is not reached and the model has at most 50,000 columns, as landschaft's
  `solve_zone_cut` does: the same plan, bound and status as landschaft 1.58.1, and Stop and the progress bar work
  in the engine too. Before, those models went straight to HiGHS's MIP, which takes minutes where the engine takes
  a hundredth of a second.
- **Maximum features and maximum phylogenetic diversity and endemism solve as landschaft solves them:** the wide
  engine for 40 % of the time limit (its bound and a first plan), then landschaft's subset search
  (`_milp_subset.py`) for the rest: a local search over the set of targets met, by additions and swaps that raise the
  reward, each set priced by a minimum set on the covering engine and taken when it fits the budget. The search
  stops with the run. The same moves and plans as landschaft 1.58.1 from the same selection.
- **`prioritize.evaluate` re-solves on Gurobi:** new settings `solver` (highs or gurobi) and `threads`, as
  `prioritize.optimize`'s, for the replacement cost and the rank importance: the base and rank solves with the
  node's threads, the replacement's parallel solves with one Gurobi thread each, as landschaft's
  `eval_replacement_importance`; HiGHS and a warning where Gurobi is not installed or licensed.
- **Stop ends more long nodes at once, and they tell how far they are:** `sdm.fit` stops within a fit (each Newton
  step of the GLM, each batch of trees of the random forest, each boosting iteration), not only between folds;
  `sdm.importance` stops and tells its shuffles (k of n), `sdm.folds` its random tries of spatial blocks, `ml.fit`
  and the per-class fits stop within the fit. The results are the same to the bit. A single call into landschaft's
  C++ (the MSPA, the circuits, maxnet) still runs to its end in the background after Stop: they need a check in
  landschaft itself.

### Fixed
- **landschaft's C++ is optimised on Windows:** the build compiled every C++ source under Terraflux (landschaft's
  kernels and engines, HiGHS) without optimisation and with the asserts on, because the cmake crate replaces
  CMake's Release flags with cc's, less their `/O`, under MSVC. They are now CMake's own (`/O2 /Ob2 /DNDEBUG`), as
  landschaft's wheels are built: a prioritization with zones that went on to HiGHS took 22.7 s, now 7.0 s, as in
  landschaft; everything that runs in landschaft's C++ gains alike. Linux and macOS were not affected.
- The Computers pane's pool code no longer changes when two pools start at once (it is read and made under the
  settings' lock), and the Settings window and the lending's fields no longer write back settings that another thread
  changed meanwhile (the window wrote them all at every frame while open); the test
  `a_computer_lends_itself_to_another_by_its_code`, which failed now and then in the whole suite, with them.

## [1.46.0] - 2026-11-14

### Added
- **The model as a Python script of landschaft, from the command line, the MCP and on each save**, not only from the
  application's window *Python script (landschaft)...*:
  - `terraflux python MODEL...` prints the script, writes it to a file (`-o`) or, with `--beside`, writes
    `<model>.py` beside each model (many models at once, for batches and CI). The script beside the model finds its
    data from its own folder (`MODEL_DIR = Path(__file__).parent`), so it can be committed and run elsewhere; one
    written to another folder points at the model's folder by its absolute path. The summary (the nodes translated,
    those left as comments and why) goes to the error output; `--strict` fails when any node was left as a comment.
  - The MCP's tool `python_script` gives the script with its summary, or writes it (`out`, or `beside`).
  - Settings, Runs: "write the model's Python script (landschaft) beside it, <model>.py, on each save and run" (off
    by default) rewrites `<model>.py` at each save and each run asked for (not the automatic runs made while the
    model is built).
  (`terraflux_app::python::{summary, beside, write_beside, write_to}`, `terraflux_app::tools::python_script`.)

## [1.45.1] - 2026-11-07

### Fixed
- **The same results on every core of a hybrid processor** (Intel's performance and efficient cores, 12th generation
  on): Eigen, under landschaft's kernels, blocks its dense products by the caches it reads once, on the core its first
  product runs on, and the efficient cores have other L1 and L2 caches, so the last bits of some results (the
  circuits' resistances, among others) depended on where the process started. Its cache sizes are now fixed
  (48 KB, 2 MB, 24 MB) before any product. The parity tests of Circuitscape's modes against landschaft failed now and
  then because of it (always, with the process held to an efficient core).
- The VRM and the spherical standard deviation square with `x * x`, as numpy's `** 2`: a debug build's `powi(2)`
  goes to the C library's `pow`, which Windows' UCRT of October 2026 does not always round to the square (release
  builds were already right; the terrain's test against landschaft failed).

## [1.45.0] - 2026-11-06

### Added
- Application, `terraflux run` and the MCP: **a copy of each run's small outputs, kept in the journal**, so a run's
  maps can still be shown and compared after a later run writes over the same files. Each run's output files (not
  folders, not a batch's), with their sidecars (`.aux.xml`, `.ovr`, a shapefile's `.shx`, `.dbf`, `.prj`...), are
  copied into `.terraflux/<model file>/outputs/<run>/` while they fit in 50 MB a run; all the copies of a model take
  1 GB at most, the oldest runs' going first. Both are in Settings, Runs ("keep a copy of each run's outputs up to
  N MB", 0 for none; the command line and the MCP keep the defaults). The Runs pane opens a copy where the file was
  written over or deleted ("overwritten since, a copy kept"), and *Compare the maps* and *Their difference* work
  for two runs that wrote the same file (the older's copy against the newer's file). *Clean outputs...* can delete
  the copies too (a box, off by default). (`terraflux_exec::journal::{Keep, Journal::keeping, kept, kept_bytes,
  clear_kept}`, `Written::kept`.)
- **Several windows, `terraflux run` and the MCP writing one model's journal at once**: every write to the journal
  (a run recorded, a version saved or deleted, the copies pruned or cleared) holds a lock on `<journal>/lock`, so
  runs ended at the same moment in different windows are all kept, each with its own id, and the files replaced
  (the models' snapshots, `versions.jsonl`) are written aside and renamed, never seen half written.
- The journals' folder, `.terraflux/`, has a `.gitignore` of its own (`*`), written with the folder: a model kept
  in a git repository does not get its journal committed. Terraflux's repository ignores `.terraflux/` too.
- Application: **configurable keyboard shortcuts**, the canvas', Viewer's and panes' own keys too. Frame the
  selection (F), home, run up to the node, delete, duplicate, find, rename, arrange, group, ungroup, bypass and select
  all on the canvas; satellite, hide the map, measure, go to, the view before and after and the simulation's steps in
  the Viewer; run and completions in the calculator; back, forward, up and the path in Files; select all in the
  attribute table: all are in the *Keyboard* tab under their place (the model's canvas, maps, the calculator, files,
  the assistant, attribute tables, lists and boxes) and can be changed. The keys that are part of editing a text or
  of a widget (Enter, Esc, the arrows in lists, Tab and Enter of the completions, copy and paste, the digits of the
  places) are listed there, read-only. F1 is built from the table (a section per place), and the status bar's hints
  name the keys chosen. An everywhere shortcut given to a key a place already has (Run on ctrl+R, which renames on
  the canvas) is warned of, and over that place the place's action wins.
- Application: **configurable keyboard shortcuts** (first part: the keys of the whole application). Settings has a
  *Keyboard* tab listing each action (run, save, save as, save a version, new, open a model, open data, the
  commands, the shortcuts sheet, Tab to the model, close the pane, next and previous model, close the model) with
  its shortcuts as buttons: click one and press the new combination (Esc cancels, Backspace takes it away, "+" adds a
  second), *Reset* per action and *Reset all*, a filter box, and a shortcut two actions share shown at once (in the
  error colour for the same place, a warning for an everywhere one against one of a place). They are kept in
  `settings.json` (`shortcuts`), and the menus, the palette, the Run button's hint and F1 show the ones chosen.
  The defaults are the keys as they were. The table is `terraflux_app::keymap` (`ACTIONS`, `Shortcut`, `take`,
  `clashes`).
- Application and MCP: **saved places** in the Viewer and on the canvas. The *Places* menu in the Viewer's header
  lists the places of the map, keeps the part of the map in view under a name (*Save this place...*, with an option
  to keep the layers shown too, which going there then shows and hides the others), and renames, gives a slot to or
  deletes each; the same menu is in the right-click menu of the empty canvas, keeping where the canvas looks (its
  container, centre and zoom). With the pointer over the Viewer or the canvas and no field being typed in,
  ctrl+1 to ctrl+9 keep the view in a slot (replacing the slot's place, which keeps its name; a new one is "Place
  N") and 1 to 9 go to it; the Viewer and the canvas have their own slots, and the palette has "Go to place N:
  name". A place of the map is a box in the map's coordinate system (kept as an EPSG code or WKT), carried to the
  map shown when it is another system, or refused with the reason; the view comes back as it was, even beyond the
  map's edge. Places are kept in the model (`[[bookmarks]]`, skipped when none), so they travel with the file; each
  save, rename, slot and delete is a step of undo with its name. The MCP's `set_plot` takes `place` (a place of the
  map's name) for the plot's saved view, and `show_model` lists the places. (`terraflux_graph::{Place, PlaceView}`,
  `Model::bookmarks` and its `save_bookmark`, `save_bookmark_slot`, `rename_bookmark`, `set_bookmark_slot`,
  `remove_bookmark`; `Model::places` stays where the canvas draws the boxes of what a scope reads.)
- Application, `terraflux sensitivity` and the MCP: **maps of sensitivity, cell by cell**. For an output that is a
  map, series or stack, *Maps of sensitivity, cell by cell* in Many runs > Sensitivity (and `--maps`, or `maps:
  true` in the MCP tool `sensitivity`) makes each run keep the last map of that output (and nothing else of the
  model's outputs), and when the analysis ends writes one GeoTIFF, `sensitivity_maps.tif`, in the results' folder,
  on the grid of the runs' maps (float32, no-data NaN): the bands `mean` and `sd` (the mean and standard deviation
  of the cell across the design's points, replicates averaged first) and an index per factor, the swing for one at
  a time (`swing_<factor>`), mu* for Morris (`mu_star_<factor>`), S1 and ST for Sobol (`s1_<factor>`,
  `st_<factor>`); NaN where a point has no value, 0 for a cell that does not change at all. It is read in blocks of
  rows (the runs' cells of a block within 256 MB) and computed in a thread of its own (the share done is shown),
  with no bootstrap per cell (the intervals stay for the number of the analysis). The runs' maps are deleted once
  the maps are made, unless *Keep each run's map* (`--keep-run-maps`) is on; the tab tells the disk they take
  (runs x one map) and the cost line and the disk check count it. The results show the file, its bands and *Show
  in the Viewer*, and the methods paragraph adds "Cell by cell, the standard deviation of the output across the
  design and the swing (mu*, first-order and total-order indices) of each parameter were mapped." An analysis taken
  up again after the application closed still makes its maps. (`terraflux_dist::sens_maps::{make, Spec, Made}` and
  `sensitivity::{map_bands, cell_bands, average_replicates}`, shared by the application and the command line.) An
  output's `layer` may now be `last` (the last step of a series, the last band of a stack), also for
  `stack.extract_layer`.
- Application and `terraflux run`: **check runs**, the model on a small part of the map or at a coarser resolution,
  to see in seconds whether it is right before a run of hours. The menu next to the Run button (and the palette)
  has *Check run on the view* (the part of the map the Viewer shows, in its coordinate system), *Check run at 1/2,
  1/4 and 1/8 resolution* and *on the view at 1/4 resolution*; `terraflux run` takes `--coarsen N` and `--window
  xmin,ymin,xmax,ymax` (in the first raster's coordinate system, or `--window-crs`). Every raster file the model
  reads (`raster.read`, the inputs of maps) is read through the window (moved out to the file's cells, so rasters on
  one grid stay on one grid, and read with windows: a large file is not read whole) and one cell in every N (the
  nearest, so classes stay classes), its grid updated; the neutral landscape models (`nlm.*`) are drawn with N
  times fewer rows and columns and wider cells; frozen results are not used (their files are of the whole map); the
  files the model writes (`raster.write` and the others that write a file) go to the check's folder, `<model> runs/check` beside the
  model (replaced each time; with `-o`, where it says), never over the real outputs, with the run's manifest. The
  application keeps the results apart from the normal runs' (a cache of their own; the normal one is back with the next
  normal run) and marks them ("check run, 1/4 resolution") in the Viewer, the status bar and the window's title, with a
  hover that says landscape metrics, distances and neighbourhoods change with the resolution and the border, so it
  checks the model and does not replace it. The journal records it as its own kind (the Runs pane: "Check run
  (1/4)"), Clean outputs... lists its files as a check run's, and the disk check does not stop it. A window that
  cannot be applied (a raster with no coordinate system and a window in one, no cells in common) stops it at once,
  naming the raster. (`terraflux_ops::check::Check`, `terraflux_raster::part::Part`, `Engine::checked`,
  `load_inputs_checked`, `journal::Kind::Check`.) Left out: the operators that open a raster file named in their own settings (CIRCUIT's INI files,
  SWAT's folder) and `nlm.curds` (sized by its steps) run as in a normal run.
- Application: **the journal of a model's runs, and the Runs pane**. Every run you ask for (the whole model, or up to
  a node; not the runs the application starts by itself) and every batch of Many runs (one entry, with its runs
  counted and, for a sensitivity analysis, its factors ranked) is recorded beside the model's file, in
  `.terraflux/<model file>/`: `runs.jsonl` (an entry a line: when, how long, done, stopped or failed with its
  message, the values of the parameters and of the inputs of values, the outputs written with their size, time and
  SHA-256 from the run's manifest, the warnings, the version and where it was asked for) and `models/<sha256>.json`
  (the model as it ran, once for each hash). The **Runs** pane (View menu, commands) lists them, newest first: the
  date and time, the duration, the status (the failure's message in the hover), what it was, what the values changed
  from the run before ("patches.isometry 1.5 -> 2"), the outputs (marked "overwritten since" when a later run or
  anything else wrote over their files), a "model changed" mark and a button to open the folder. One or two runs
  can be selected (click, ctrl+click). The small outputs are copied with the run (see the copies kept, above). `terraflux run` records its run
  too (`--no-journal` leaves it out), as does `terraflux sensitivity` and the MCP's `run_model` and `sensitivity`.
  (`terraflux_exec::journal`: `Journal`, `Entry`, `Written`, `record_run`.)
- Application: **the Runs pane's actions**. A run selected is told in full under the table (every value as it ran,
  the outputs with their path, size and whether they are still as written, the warnings, the failure's message, the
  manifest, the version and where it was asked for; a batch's folder and, for a sensitivity analysis, its factors
  ranked), with *Show its outputs* (those still as written, opened as layers of the Viewer; the ones written over
  since, or gone, are left out and counted) and *Restore this model* (the model as it ran in place of the open one,
  one step of undo named "Restore the model of the run of ..."; off when the snapshot is gone or is the model
  already). Two runs selected are compared: their values side by side (only those that differ, a box for all),
  *Review the model's differences* (the Changes pane, read-only, from the older run's model to the newer's),
  *Newer against the model open* (a review with accept and reject), and *Compare the maps* or *Their difference*
  (a map both runs wrote in files of their own, in the Viewer's side by side or difference). A scenario applied in
  the Parameters pane is recorded with its name by the runs that follow, until a value it set changes. The MCP tool
  `list_runs` (and the application's assistant) reads the journal: newest first, with `limit`, each run's values
  changed from the run before and its outputs' state, and with `id` one run in full.
- Application: **named versions of a model, without git**. *Save a version...* (File menu, the commands, ctrl+alt+S;
  listed in F1) keeps the model as it is on screen under a label and an optional note (the window says so when it has
  edits not saved), beside the runs' journal (`versions.jsonl`, the model in the same `models/` folder). The Runs
  pane has two tabs, **Runs** and **Versions**: the versions newest first, each with its date, its note and what
  changed from the version before (nodes added, removed and changed). A version selected can be reviewed against the
  model open (*Review against the model open*, in the Changes pane, to accept or reject each change), restored (one
  step of undo named "Restore the version ...") or deleted after asking; two selected have their differences shown,
  to read. A run's detail says which version its model is. The MCP tools `save_version` and `list_versions` (and the
  application's assistant) save and list them, so an agent can mark a point before a large change.
  (`terraflux_exec::journal`: `Version`, `Journal::save_version`, `versions`, `delete_version`.)
- Application: **the disk checked before a run, and Clean outputs...**. Before a run you ask for (the Run button, F5,
  the commands) and before a batch of Many runs that keeps its files, the application estimates what will be written
  (the outputs of the latest whole run in the journal, the one of the same model first; else the model's: the cells
  of the largest raster it reads, 4 bytes a cell or the output's type, the layers of each map, series and stack it
  writes, halved for the compression) and measures the free space of the output's disk (Windows, Linux and macOS).
  When it may not fit (more than the free space less 5 % of the disk, a gigabyte at least) a window says "The outputs
  may take about 3.2 GB; the disk (D:) has 1.1 GB free" with *Run anyway*, *Clean outputs...* and *Cancel*; when it
  fits but leaves under a tenth of the disk free, a line in the status says so. The Many runs line under the button
  adds ", about 12 GB on disk (40 GB free)" for batches that keep files, in the warning colour when it does not fit.
  `check_model` (the MCP, the assistant and the Problems pane) warns of the same. **File, Clean outputs...** (also
  the commands, the Runs pane and the disk window) lists the files and folders the model's runs wrote, from the
  journal, each with its size, date and whether it exists: the batch folders and the outputs of the runs before the
  newest are chosen, the newest run's are not, and *Delete N items (X GB)...* asks first (the deletion is for good,
  not to the recycle bin). Only what the journal lists is touched, never the model's folder, a drive's root or the
  home folder; the journal keeps its runs, which then show the outputs missing. (`terraflux_app::disk`,
  `clean_view`; `terraflux_exec::journal::hash_of`.)
- Application: **a Sensitivity tab in Many runs**. Which settings matter to an output, without the command line: the
  factors are picked from the model's numeric settings (a range of 20 % either side of the value, whole numbers
  widened to whole numbers) and the method is one at a time (each factor at its ends, the others in the middle of
  their ranges: the swing of the output), Morris (the screening: mu, mu*, sigma) or Sobol (S1 and ST with bootstrap
  intervals, the second order on request). The output is chosen among the model's (a number, a table's column, or
  the mean, sum, min or max of a map, series or stack), only numbers come back from the workers, and the indices
  are a table sorted by what matters most, with `design.csv`, `indices.csv` (and `s2.csv`) in the results folder.
  A batch taken up again after the application closed still gives its indices.
- What a sensitivity analysis measures of a map, without building nodes: besides the mean, sum, min and max, the
  cells of a class (`cells:<class>`), their share of the cells with a value (`share:<class>`, 0 to 1), their area
  (`area:<class>`, in hectares on a grid projected in metres, else in the map's units squared), the fraction of
  cells that changed (`changed`: first against last map of a series or stack, a single map against the start map
  chosen in the pane) and a landscape metric of a class (`metric:<np|pland|ed|area_mn|lpi>:<class>`). In the
  Sensitivity tab and in `terraflux sensitivity --statistic` (`terraflux_dist::evaluate::measure_from`).
- **Replicates at each point of a sensitivity analysis**, for models that draw at random. Each point of the design runs
  with R seeds (the model seed, +1, +2...), the output is their mean, and the share of the output's variance that
  comes from the seed, not the factors, is told ("Chance: 12 %", with a warning above 50 %). `design.csv` gets the
  `sd` among the replicates and `n`, those that ran (a replicate that failed is left out of its point's mean). In the
  Sensitivity tab ("Replicates at each point", off when the model has no seed; the run count and the cost line
  include them) and in `terraflux sensitivity --replicates R`
  (`terraflux_dist::sensitivity::{seeds, total_runs, replicated}`).
- Application: **the cost of a batch before it runs**. Under the run button of Many runs (replicates, scenarios,
  calibration and sensitivity), the runs and about how long they take, from the times of the nodes in the runs
  before divided by the runs at once here (or the workers of the cluster chosen). A sensitivity analysis names its
  design ("Sobol of 6 factors, N=256: 3584 runs, about 5 h here"). Over 2 hours the line turns to the warning
  colour and suggests fewer runs, or fewer base points and Morris first.
- Application: **the factors of a sensitivity analysis marked in the model**. Right-click a numeric setting in the Node
  pane and choose *Vary in a sensitivity analysis* (a setting that reads an input offers that input): the factor is
  added to the Sensitivity tab, 20 % either side, and Many runs comes to the front. The Parameters pane has a *Vary*
  button on each number (its range, when it has one) and *Sensitivity of these parameters...* for all of them. The tab
  lists *Suggested* factors, with a hover saying why and *Add all*: the parameters with a `min` and a `max`, and the
  settings (or `$inputs`) that the nodes' `assumptions` speak of, as they are the ones assumed, not measured.
- Application: **the charts of a sensitivity analysis, and whether it has settled**. Above the indices table, a
  tornado for one at a time (each factor from its minimum to its maximum around the base output, the largest swing on
  top), μ* against σ for Morris (with the σ/μ* = 0.1 and 1 guides of Garcia Sanchez et al. 2014 and what they mean on
  hover) and S1 and ST bars with their intervals for Sobol. For Morris and Sobol the indices are computed again with
  a quarter and half of the trajectories or base points: "Stable" when no factor's value moved by more than 5 % of the
  largest from half the runs to all of them and the factors that matter (above 10 % of the largest) kept their order,
  else a warning to use more trajectories or base points, with a line chart of the values at each size. The chart is
  written as `sensitivity.svg` and `sensitivity.png` next to `indices.csv`, with `convergence.csv` (also from
  `terraflux sensitivity`), and *Save the chart...* writes it elsewhere
  (`terraflux_dist::sensitivity::{Convergence, convergence_csv}`).
- **The methods paragraph of a sensitivity analysis, and the MCP tool `sensitivity`**. When an analysis ends (in the
  application and in `terraflux sensitivity`) `methods.md` is written next to its tables: a paragraph in the words of
  a methods section (the method and its papers, the design, the factors and their ranges, the output and statistic,
  the runs, the replicates and the share of variance that is chance, whether the indices settled, the Terraflux
  version and the SALib it reproduces) and the references of the method; the results show a *Copy the methods text*
  button. The papers (Morris 1991; Campolongo et al. 2007; Sobol' 2001; Saltelli 2002; Saltelli et al. 2010; Herman &
  Usher 2017; Iwanaga et al. 2022) are on the "Cite Terraflux" page and tied to the `sensitivity.*` operators in the
  methods of a model. `terraflux mcp` has a `sensitivity` tool: factors (`{key, from, to}`, or 20 % either side), the
  method, `n`, `levels`, `second_order`, `replicates`, the output and its statistic; it runs the design on up to 4
  workers here and answers with the ranked indices, the chance share, the convergence verdict, the paragraph and the
  files (a design of more than 500 runs is refused until `confirm`). In the application's assistant the same tool
  puts the analysis in Many runs, Sensitivity, for the user to run. `terraflux sensitivity` also writes the chart.
- `terraflux sensitivity --method oat`, and the new `terraflux_ops::sensitivity::oat_sample` and `oat_analyze`.
  The design, the candidates, the indices and their CSV files are now `terraflux_dist::sensitivity`, shared by the
  command and the application (the files and columns of Morris and Sobol are the same as before).

## [1.44.1] - 2026-11-06

### Changed
- Application: **the tabs of the models open stand out**, each model in a colour of its own (its tab washed in it,
  a band of it at its left, the one in front stronger and outlined), "2 models open" before them, and the model in
  front marked in its colour by its name in the toolbar.

### Fixed
- Application: **no pane draws over the others any more**. A narrow Viewer (the Trace opened beside it, say) put
  its buttons and its Layers panel over the panes at its left; now its buttons go to a row of their own when one
  row is too narrow, the panels beside the map narrow with the Viewer, the Layers' title goes over its buttons, and
  every pane of the dock is cut at its sides. The boxes floating over a pane (the slowest nodes, Go to, Measure, the
  input to choose) stay inside it.
- Application: **dragging the Viewer's own window no longer stutters**. Its place was written to the settings every
  0.6 s while it moved (a file written in the middle of the drag); now once, when it stops.

## [1.44.0] - 2026-11-06

### Added
- Application: **the files not found are looked for around the model at once**. When a saved model opens with
  files it reads not where it says (a project copied whole to another computer), they are looked for in its folder
  and the one above, in a thread; the strip says "2 found beside the model." with *Apply (2)*, one step of undo.
  *Look in a folder...* also looks in a thread now, with a spinner and *Cancel*, the window never frozen.
- MCP: **`locate_files`** finds again the files a model reads that are not where it says (in the model's folder and
  the one above, or the folders given) and makes the model read them from there; it says what was found and what
  is still not found, with the nodes that read it.
- Command line: **`terraflux relink model.toml [--in FOLDER]... [--dry-run]`** does the same and writes the model
  again with the paths found (relative to its folder); it exits with 1 while a file is still not found, for scripts.
- Canvas: **notes and frames go with the nodes copied**. Copying, duplicating (ctrl+D, the menu) or ctrl-dragging
  nodes takes the frames around them all along, the copy around the copies; a frame selected is copied with its
  nodes, a note selected by itself. Pasted in another scope, model or window, they come with new ids.
- Canvas: **how long each node took before, from the first moment**. A model opened again shows under each box,
  fainter and with a `~`, how long it took when it last ran (in the sessions before, as the time left of a run
  already uses), until it runs in this one; the tooltip says so.
- Application: **an agent's edits named in the History by its tools**. When an agent changes the model open through
  `terraflux mcp`, the step that reads the file again is named by the tools it used ("the MCP: add_node, connect"),
  no longer guessed from the difference. The MCP tells them in the user's application data (`agent-edits.json`)
  before writing the file; the application takes them when it reads it.
- Application: **when a run will end, by the clock**. The tooltip of the run's progress bar says when the time left
  runs out: "ends about 14:32", "ends about 09:10 tomorrow", "ends about 09:10 in 3 days" (the computer's own time
  zone and summer time, on Windows, Linux and macOS).
- Application: **a click on the notification of a run that ended brings Terraflux to the front**. On Windows the
  toast opens `terraflux-front:`, a protocol registered in the user's registry (`HKCU\Software\Classes`) that runs
  `terraflux front`, which asks the Terraflux that told it to come forward; on Linux `notify-send` waits for the
  click (an older `notify-send` shows the notification as before). On macOS a click still does nothing
  (`osascript`'s notifications cannot tell it).

## [1.43.0] - 2026-11-06

### Added
- Application: **edits not saved are never lost without asking**. Closing the window, *New model*, *Open...*,
  *Open recent*, a template, a model made by the wizard or a model file dropped on the window first ask "Save the
  changes to …? Save / Don't save / Cancel" when the model open has edits not saved (a template only started, with
  nothing done to it, is not asked about). Before, they were thrown away silently.
- Application: **the edits not saved are copied and recovered after a crash**. While a model has edits not saved,
  a copy of it is written every half a minute (when it changed) in the user's application data
  (`Terraflux/recovery/`), deleted once the model is saved or closed without saving. When Terraflux opens after one
  that ended without saving (a crash, the power gone), a strip offers "Terraflux closed with edits not saved in …
  (copied 3 minutes ago): Recover / Discard / Later"; Recover opens the model from its file with those edits, not
  saved, a step of undo back to the file as it is on disk. The copies of another Terraflux still open are never
  offered (each gives a pulse while it runs). Windows, Linux and macOS alike.
- Application: **the computer is kept awake while a model runs**, a batch of Many runs goes on or this computer
  works for another's batch, so a run left for the night does not stop when the system would sleep (the screen may
  still go dark). On Windows by the system's call, on macOS by `caffeinate`, on Linux by `systemd-inhibit` (idle and
  sleep, else idle only, as where blocking sleep is refused), ended with the work or with Terraflux itself. Settings
  > Runs turns it off; the progress's tooltip says it.
- Application: **the end of a long run is told** when Terraflux is not in front: a run you asked for (not the
  application's own runs after an edit) or a batch of Many runs that took 10 s or more (Settings > Runs: on or off,
  and the seconds) lights the window's button in the taskbar (the Dock's icon bounces on macOS) and shows a
  notification of the system: "land: done · The run ended in 14 min.", or why it failed. On Windows a toast under
  the name Terraflux (registered for the user, as applications not from the Store do), on macOS the Notification
  Center, on Linux the desktop's notifications (`notify-send`, else `gdbus`). While a model runs, the window's title
  starts with how far it is ("[42%] land - Terraflux"), seen on its button in the taskbar.
- Application: **the time a run has left**, beside its progress: "~14 min left" from how long each node took in the
  runs before (kept per model between sessions, in the user's application data, the last 200 models), the node
  running counted by how far it says it is (a loop by its steps); a model never run gets "about 3 min left", guessed
  from how far the run is once it has gone a few seconds.
- Canvas: **how long each node took**, under its box once the model has run ("2.4 s", "3 min", "cache" for one taken
  from the cache; a container its body summed), with what it took and the memory of what it gave in the tooltip.
  **Times** in the canvas bar washes each box in a warm colour by its share of the run's time and lists the five
  slowest in the corner, a click taking the view to one.
- Canvas: **the History of the edits** (the canvas bar's History, View > History of the edits, ctrl+K): every step
  named by what it changed ("added suit (raster.calc)", "reclass: setting threshold: 3 → 4", "3 nodes moved",
  "renamed 4 by a pattern", "the assistant: edit_node"), the model as it was opened first, the step it is at
  marked and those undone faint. A click goes back or forward to any step at once; "compare" opens the Changes with
  the model as it was then.
- Application: **the files a model reads that are not found are located again**, as for a model from another
  computer or a folder moved: a strip over the canvas and in Problems says "2 files of the model are not found",
  and *Locate them...* lists each with the nodes and inputs that read it. *Look in a folder...* finds them by the
  longest end of their old path that is there, else by their name (four levels down); once one is found, the same
  change of folder is tried on all the others; *Choose...* points to one by hand. *Apply* changes them all at once,
  one step of undo, relative to the model's folder, and reads the data again. The old paths are cut at `/` and at
  `\` alike, so a model written on Windows relinks on Linux and macOS, and the other way round. The files written
  are never asked for; a folder or a pattern (`*.tif`) is found when its folder is.
- Canvas: **notes and frames**. A right click on the empty canvas, *Add a note here*: a card with a text, for
  whoever reads the model. The selection's menu, *Frame them*: a rectangle around the nodes with a title, which
  follows them (*Arrange*, drags); dragging its title drags its nodes, a node let go in it joins it, a node's menu
  takes it out. A double click edits the text (a frame's title is its first line), Delete takes the one selected
  away, the menu gives the colour; each a step of undo. They are kept in the model's file (`[[notes]]`), outside the
  graph: never checked nor run. The Document puts the steps of a frame under its title, and the MCP sets and
  removes them (`set_note`, `remove_note`; `show_model` lists them).
- Application: **several models open, in tabs**. *Open*, *Open recent*, *New model*, a template, the wizard, a
  model recovered and the MCP's `open_in_app` open a model in a tab of its own (an empty model nothing was done to
  is replaced; a model open already goes to its tab), instead of putting the one open aside. With two models or
  more a strip of tabs under the toolbar: each model's name, a dot when it has edits not saved, a spinner while it
  runs, its × (or the middle button) closing it. ctrl+Tab and ctrl+shift+Tab go to the next and the one before,
  ctrl+shift+W closes the model in front (ctrl+W still closes the pane in front); both are in the commands too. The
  models behind go on running, and the end of a long run says which model it was; the Trace, the Parameters, the
  Calculator and the Assistant's conversation follow the model in front. Closing a tab, or Terraflux, asks about
  each model with edits not saved. Nodes copied in one tab are pasted in another as before.
- Application: **the last session comes back**. When Terraflux starts without a model given, the models that were
  open when it closed open again in their tabs, the one in front in front (Settings > Start: *reopen the models open
  last*, on unless turned off). And each model keeps its view, for the last 100 opened: where the canvas looked (the
  group open, the camera), the node selected, what the Viewer showed (its layers, the map shown, the comparison,
  the frame of the map) and the pane in front; a model opened, by *Open recent* too, gets it back, what the Viewer
  showed run the quiet way (as a double click does), never the whole model. Kept in the user's application data
  (`last-session.json`, `views.json`), every minute and when Terraflux or a tab closes; never in the model.

## [1.42.0] - 2026-11-06

### Added
- Canvas: **a connection let go on empty canvas asks for an operator that takes it**: the search opens at the
  pointer with only the operators that have an input for that type ("Add an operator that takes a raster", those
  of that exact type first), and the one chosen is born there, connected and selected, in one step of undo. The
  connections of nodes selected dragged together go into it (each a new input of a `stack.of`, else the free inputs
  that fit); inside a group it is added to its body. Esc or a click outside adds nothing.
- Canvas: **a node let go over a connection goes in its middle**: a node with a free input for the connection's type
  and an output its end takes, dragged over it, lights the connection up, and on release A → B becomes A → node →
  B (never a cycle), in one step of undo. **Alt+drag takes a node out**: what it read is connected again to what
  it fed where the types fit, the rest left unconnected and said.
- Canvas: **ctrl+F finds a node of the model**, at any depth (inside groups, loops, switches), by its id,
  operator, note or a file of its settings: the list tells each one's place (`group > inner`) and the file that
  matched; choosing one opens the containers on its way, selects it and centres the view on it. The command
  palette's *Go to* reaches nested nodes too (`Go to group/inner`).
- Canvas: **F frames the nodes selected** (none selected: the whole model, as Home), within the group open, zoomed
  in at most 1.75 for a single small node. The view glides there (about a quarter of a second, easing out), for
  Home and ctrl+F too.
- Node pane: **a setting changed in all the nodes selected at once**. With nodes of one operator selected it shows
  "5 raster.read selected" and their settings once: the value where they agree, "several values: 0, 255, default"
  where they differ; a change goes to every one in one step of undo, and a value refused for any is set on none.
  Nodes of different operators are listed, each a click away.
- Canvas: **the selection's menu**. A right click on a node of a selection (or on the empty canvas) with several
  selected, headed "3 nodes selected": *Delete*, *Duplicate* (the links among them copied), *Disconnect all their
  inputs* (the values typed kept), *Group*, *Run to them* (one run after the other up to those no other needs,
  the shared part from the cache), *Freeze* or *Unfreeze*, *Bypass* or *Turn them on*, and *Connect the others
  selected* to the node clicked; each one step of undo, those that do not apply greyed with the reason.
- Canvas: **ctrl+drag duplicates** a node, or the selection when dragged by one of its nodes: outlines follow the
  pointer while the originals stay, and on release the copies are made there, selected, in one step of undo, reading
  what the originals read and linked among themselves as they were (inside groups too). Esc cancels; ctrl+click
  still adds to or takes from the selection. A node's own menu gains *Duplicate* (ctrl+D), saying so.
- Canvas: **the file of a read node changed in place**. *Change file...* in the menu of a `raster.read`,
  `vector.read` or `table.read`, or a file of that kind dropped from the file manager onto the node (it lights up:
  "Drop: the file of a (its connections kept)"), sets its `path` keeping its id, connections and other settings,
  in one step of undo, inside groups too. A file of another kind, or several, is added as before.
- Canvas: **align and distribute the nodes selected**. The selection's menu has *Align*: left edges, centres,
  right edges, top edges, middles, bottom edges; *Distribute* across or down (equal gaps between the boxes, the two
  outermost staying, three nodes or more); *In a column* or *In a row* (stacked from the first). Each one step of
  undo, inside groups too, by the boxes' real sizes.
- Canvas: **disconnect all the inputs of a node** from its menu (the values typed kept; greyed when it reads no
  node), and **two connections swapped**: one taken off its input and let go on another input already connected
  shows "swap with b" and exchanges them when each fits where the other was (else as before), with its taking off
  one step of undo, inside groups too.
- Canvas: **a minimap of a large model**. When the model (a group's body, inside a group) does not fit in the view,
  the corner shows all its nodes, the selected ones lit, and the view's rectangle: a click centres the view there, a
  drag pans it live, and the canvas behind gets neither clicks nor the wheel. × hides it (Settings: *a minimap of a
  large model on the canvas*).
- Canvas: **the nodes selected renamed with a pattern**. *Rename...* in the selection's menu: `{n}` (1, 2, 3 in
  their order on the canvas, top to bottom), `{id}`, `{op}` and `{file}` (the stem of the file it reads), each new
  name previewed and the wrong ones (taken, repeated, invalid, no file) said, OK only when all are right; one step of
  undo, the connections following, inside groups too. And **a read node's id follows its file**: when its file is
  changed and its id is still the old file's name, it takes the new one's (made unique), in the same step.
- Canvas: **reroute points on a connection**. A double click on a connection adds a point, dragged to where it should
  pass; a double click on it (or *Remove point* in its menu) removes it; each one step of undo, inside groups too.
  They are saved on the node the connection goes into (`wires = { port = [[x, y]] }`, absent when there are none, not
  part of what the cache sees), copied with the nodes, kept in a group made, dropped with the connection and by
  *Arrange*. And **connections go around the boxes**: one without points whose curve would cross another node bends
  above or below it (Settings: *wires go around the boxes*).
- Canvas: **a connection tells its type and its value**. The pointer resting on a connection shows its type and a
  summary of what went through it in the last run: a map's cells, bands, cell size, coordinate system, range and
  cells without a value (the range up to 20 million cells), the first map of a stack, a table's rows × columns,
  vector features' count and geometry, a number itself; "not run yet" or "changed since the last run" otherwise.
  Inside groups too; never while dragging.
- Viewer: **from a layer to its node**. *Show its node* in a layer's menu (Layers) brings the model's canvas forward
  (the main window too, with the Viewer in a window of its own), opens the group, loop or switch it is in, selects
  the node and frames it with the glide of F; greyed, with the reason, for a difference of two maps.
- Viewer: *A URL...* also in the Viewer's header (the same dialog as the Layers panel's, in the Viewer's own window
  too).
- Viewer: **Go to... with a zoom**: "keep the zoom" (as before) or a scale beside the coordinate (1:5 000 to
  1:1 000 000), or typed after an `@` (`-15.8, -47.9 @ 1:50000`, `@ 2 km` for the width of the view), one step of
  back and forward; and **its mark in the views side by side**, at the same place in each (across coordinate
  systems), esc on any of them removing it from all.
- Viewer: **the features selected kept with the attribute table closed**: each layer's selection outlives its
  table (reopened, the table shows it again), and *Update the plot* keeps the selection of every layer of the Plot,
  not only of the table open; a Plot opened selects in all its layers. The MCP's `set_plot` takes `selected` per
  layer (the features' numbers from 1).
- Viewer: a layer in another coordinate system (drawn reprojected, as a mesh) is **blended** in the live view too
  (multiply, screen), as the export already did.
- Viewer: **the same colours for all similar layers**. From a layer's menu, its palette and range go to every other
  layer of values (a map of classes: its colours to the maps of the same classes), the side-by-side views and the
  Plot saved with them; the hover tells how many layers change, greyed when there is none.

## [1.41.0] - 2026-11-06

### Added
- Viewer: **three bands as one colour image**. A layer of three maps or more (a stack, a series, a raster of
  bands) checked **RGB** in its settings is drawn as colour, a band for each of red, green and blue: natural colour,
  colour infrared and short-wave infrared of Sentinel-2 and Landsat 8/9 offered when the bands' names tell them
  (`B4`, `SR_B4`, `B04`, `band 4`), else the first three. Each channel from its band's 2nd to 98th percentile
  (worked out by a thread from a sample) or a range set by hand, with a gamma; a cell without a value in any band
  transparent. The legend under its row tells the bands and ranges, the pointer the three values. Kept in the Plot
  (`rgb = { bands, red, green, blue, gamma }`, also through the MCP's `set_plot`), drawn by the figure, *Export the
  view*, the animation and `terraflux plot` / `export_plot`.
- Viewer: **features selected by area, and the selection in the model**. Shift+drag on the map selects the features
  of a vector layer that meet a rectangle, shift+alt+drag a lasso (exactly, not by their boxes), ctrl too adds them;
  the table of attributes follows (opened for the topmost vector layer when none is). A Plot keeps the selection
  (`selected = [..]`, the features' numbers). *Filter the selection* adds a `vector.filter` that keeps exactly the
  features selected (`feature == 1 or 3 <= feature <= 9`, compact for thousands), with its note, drawn over the map.
  Ctrl+A selects every row with the keyboard in the table, not only the pointer.
- Viewer: **the symbology of vector features**. Coloured by an attribute along a ramp chosen among the Viewer's,
  continuously or in 2 to 12 classes by equal intervals, quantiles or natural breaks (Fisher-Jenks); numbers as
  categories when asked. Points as a circle, square, triangle, diamond or cross, sized by an attribute (the area by
  the value). The legend under the layer's row and in the figure (PNG and SVG) tells categories, classes, ramp and
  sizes. Kept in the Plot (`ramp`, `breaks`, `classes`, `categories`, `size_by`, `size_min`, `size_max`, `symbol`,
  also through the MCP's `set_plot`), drawn by the export and `terraflux plot`, and written to QGIS as a
  categorized or graduated renderer with the symbol and a data-defined size.
- Viewer: **relief under the colours, and blend modes**. *Shade with this elevation* on a map layer of values draws
  its colours over its own hillshade (Horn's, as `gdaldem`; azimuth, sun altitude, exaggeration, strength; metres
  per degree on geographic grids), made tile by tile by the Viewer's threads with a halo, without seams. A layer
  blends with those under it as *normal*, *multiply* or *screen* (drawn by the GPU with its own blend state). Kept in
  the Plot (`blend`, `shade = { azimuth, altitude, z, strength }`, also through the MCP's `set_plot`), drawn by the
  figure, the export, the animation and `terraflux plot`.
- Viewer: **finding one's way on the live map**. A scale bar (m or km; geodesic metres at the view's latitude on
  grids in degrees; cells without a coordinate system) and, when asked, a coordinate grid in the map's own
  coordinates, in every view side by side (*On the map* in the panel, kept in the settings). Back and forward along
  the views stopped at (‹ ›, alt+← alt+→, the mouse's back and forward buttons). *Go to...* (ctrl+G): a coordinate in
  decimal degrees ("lat, lon" with a comma, "lon lat" without), degrees-minutes-seconds with hemispheres, or x y in
  the map's units, centred and marked until esc.
- Viewer: **in a window of its own**, for a second screen: *Pop out* in its header or *View > Viewer in a window of
  its own*, back by *Back into the window* or by closing it. Files dropped on it, its shortcuts, Figure and Export
  work there, and a double click on a node brings that window forward. Its place and size are kept (back on a
  visible screen when the remembered one is gone), and it opens out again at the next start (the layout's version
  12; layouts of 11 still load).
- Viewer: **maps on the web as layers**. *A URL...* in the Layers' add menu checks an address in a thread (size,
  bands, coordinate system, or the error) and adds a `raster.read` of it: a Cloud Optimized GeoTIFF or any raster
  GDAL reads through `/vsicurl/` (`https://` given as it is, `s3://` and `gs://` as `/vsis3/` and `/vsigs/`), read
  by ranges of bytes as the map asks, from its overviews when zoomed out (the whole view of a 22 MB COG read 6 MB),
  an error of the server a layer's error; GeoJSON and FlatGeobuf as a `vector.read`. A model keeps such an address
  as it is (not made relative, not checked as a file). *Custom...* basemaps: any XYZ tile service by its template
  (`{z}/{x}/{y}`), with its attribution and deepest zoom, the last five kept, cached on disk as the others.

### Changed
- Viewer: the **histogram** of a map coloured on a logarithmic scale has its bars equal in the logarithm.
- Viewer: ***Export the view*** puts the views compared side by side in one PNG (*with the views side by side*; a
  GeoTIFF keeps the map alone and says so) and draws the basemap shown under the map (*with the basemap*, from the
  tiles on disk or fetched).
- `terraflux plot` / `export_plot`: difference layers are made and drawn as in the Viewer, a Plot that compares
  side by side is written with its views side by side (PNG), and a node inside a container is found by its name.
- Viewer: **measuring** with views side by side, each view measures its own line (drawn in the others too), and the
  box of numbers is in the view last measured in only.
- Viewer: where **labels** collide the larger feature keeps its own (polygons by size, then lines by length, then
  points), in the view and the figure; the project for QGIS labels a layer by its style's attribute and size.

## [1.40.0] - 2026-11-04

### Added
- Canvas: **a group looks like one, and going in or out is a zoom**. A group's box has two cards behind it (a stack
  of nodes), an accent border and header, a stack icon and how many nodes are inside ("group · 5 nodes"). Entering
  it, the view zooms into its box: the scene inside grows out of the box as the outer scene fades; leaving, it
  shrinks back into the box (about 0.3 s, easing out; none when egui's animations are off; clicks pass through).
- Canvas: **the nodes selected are connected together**. A connection drawn from the output of a node selected
  takes the others selected with it (their wires drawn along, "3 connections" at the pointer): let go on a node that
  takes any (`stack.of`), each is a new input, in their order on the canvas; let go on another node, they fill its
  free inputs that fit, in order. The menu of a node does the same: *Connect the 3 nodes selected here* (every
  output of a group selected). One step of undo.
- Canvas: **a group gives the results of its last nodes as outputs**: rasters grouped that nothing read yet (or the
  end of a chain) are outputs of the group, to connect to a `raster.reclassify` or a `stack.of` outside. A group
  made before gets them from its menu, *Give it the outputs of its last nodes*.

### Changed
- **Terraflux never takes every core**: by default and at most it uses the cores but a quarter (20 cores: 15; 8: 6;
  4: 3; 2: 1), for every tool, run, solver and view: the settings, `--threads` and `TERRAFLUX_THREADS` may ask for
  fewer, never more. It was every core but one. A number of threads given to Gurobi, `batch.run`'s workers and the
  runs of a model over many files is held to it too.
- Viewer: **the legend of the map shown is in Layers, under its row**, as in a GIS, for every kind of data: the
  classes with their colours (with *Colours* and *Edit legend*), or the ramp of a continuous map. The panel at the
  right keeps the *Area of the classes* or the histogram, the statistics and the steps.

### Fixed
- Viewer: **the window stays fluid while many rasters load**. The rasters on disk are read for the view by a few
  threads of their own (a quarter of the cores, 4 at most) instead of every core, and a frame takes at most 32
  tiles new to the GPU, the others at the next frames.

## [1.39.0] - 2026-11-04

### Changed
- Viewer: **the legend of a map of classes is one table**. The legend and the *Area of the classes* were two
  lists of the same classes, the second with a bar of the cells in each (a model of MSPA classes showed them
  twice). Now one table: a row per class with its colour, its name, its area (of the whole map, the part in view,
  the polygon measured or the polygons selected, in hectares, km² or cells), a bar of its share and the share; the
  palette and *Edit legend* in its header.
- Viewer: **the Layers panel redone as a GIS has it**: a bar of icons (add, remove, up, down, zoom to the layer, all
  shown or hidden, every legend folded), the rows in a list that scrolls when long (the top layer first: an arrow
  folding its legend under it, its box, its symbol, its name, what it is, and a ⋯ menu), then the properties of
  the layer selected (opacity, the map of a stack, the palette and the histogram, the style of features). A click
  selects a layer, a double click zooms to it, a drag puts it over or under the others, a right click or the ⋯
  gives what can be done with it.

### Fixed
- Viewer: **many rasters at once draw quickly**. The 41 Landsat scenes of a folder dropped as 41 layers never
  finished loading: more tiles and colourings were needed than were kept, so those let go were asked for again
  without end. The tiles and looks of the last frame are now always kept; a map's tiles hidden under an opaque
  layer are neither read nor drawn; the map shown and the top layers come first. A large GeoTIFF without
  overviews of its own gets one in memory, made once (each strip read once), from which the whole map in view,
  its colours and its histogram are taken, the same cells as from the file. The folder above: the map in view in
  under a second and every layer in about 4 seconds (before: still loading after 20); as one stack, under a
  second.

## [1.38.0] - 2026-11-04

### Changed
- Viewer: **the transition matrix** of two maps of classes compared by *Difference*: *Transition matrix...* opens
  every class before (rows) to every class after (columns) in hectares, km², a share of the landscape or of the
  class it came from, or cells (areas geodesic on a grid in degrees, none without a coordinate system); each
  class's gains, losses, net change and swap as bars; and the flows from before to after as a Sankey diagram.
  A cell of the matrix (or a flow) clicked shows only that transition on the map, the other changes grey;
  *Copy* (tab-separated) and *Save CSV*. The difference opened before the map was ever drawn is no longer blank.
- Viewer: **the area of each class, and the statistics of a region**. Under the legend, *Area of the classes* (of a
  map of classes: each class in hectares, km² or cells and its share) or *Statistics* (of a map of values: the
  cells with a value, their area, mean, minimum, maximum, sum and standard deviation), of the whole map, of the
  part in view (once the view rests), of the polygon drawn with *Measure* (three points or more) or of the
  polygons selected in the table of attributes; a cell is in when its centre is. Worked out in a thread, from a
  sample when the part has more than about 4 million cells; *Copy* gives the numbers.
- Viewer: **the profile along a line**: the line of *Measure* (two points or more) samples the map about every half
  cell (up to 2 000 points): a map of values as a chart of its values by the distance (with the minimum, maximum and
  mean), a map of classes as a band of the classes crossed with the length in each; the place under the pointer on
  the chart is marked on the map; *Copy* gives every point (distance, column, row, value).
- Viewer: **the classes along the steps** of a series or a stack of classes (a `control.repeat`, CLUE-S, cellular
  automata, the years of a file): a chart of the area of each class at every step beside *Play* (a click shows that
  step, *Copy* the table), and *first to last...* opens the transitions from the first step to the last: the flows
  as a Sankey diagram, the matrix, the gains and losses.
- Viewer: **saved views**. A Plot keeps the frame of its map (`view = { extent = [west, south, east, north] }` in the
  `[[plots]]` of the model file, in the map's coordinates, so it does not depend on the window): *Keep this view in the
  plot* in the Plots menu keeps what is in view (*Update the saved view*, *Go to the saved view* and *Forget the saved
  view* follow), and opening the Plot brings the map to that frame. The Figure, *Export the view*, the export of a Plot
  by its id (`terraflux plot`, `export_plot`) and the Document and report pictures show only that part of the map,
  at the same proportions, the vectors and the other maps cut the same way. `set_plot` takes `view` too. Plots without
  a view, and old files, are unchanged.
- Viewer: **the Layers as a GIS has them**. A bar over the list: *➕* adds a map or features of the run, or files
  from disk (files dragged from the Files pane onto the list too), on top; *🗑*, *⏷*, *⏶* and *🔍* act on the layer
  selected, *👁* shows or hides them all. Each row: its box (shown or hidden, the map shown too, now), an arrow that
  folds its legend and settings, its symbol (its ramp, its first classes, its features' colour as points, a line or a
  polygon), its name. A click selects it (a map: its legend at the right), a double click zooms to it, a right click
  or its *⋯* gives zoom, the attribute table, colours, show only this layer, show every layer, to the top, up, down,
  to the bottom and remove. Rows are **dragged** to their place, a line showing where they go (it did not work
  before). *Add a layer* no longer puts the new layer under the map shown, where it could not be seen: the map shown
  is kept as a layer and the new one goes over it. The map shown takes its layer's opacity.
- Viewer: **the palette changed by a click on it**: the legend's ramp (or the classes' colours) at the right and in
  each layer opens the ramps, drawn, with *by what they are* and *logarithmic*; the *Colours* list is gone.
- Viewer: **the range of the colours set on the histogram**: two lines dragged along it, the least and the most typed
  under it, *2% to 98%* (as at first) and *least to most*. Kept for the output and in a Plot (`range = [least,
  most]` in a layer of `[[plots]]`, `set_plot` takes it), and used by the export of a Plot, the Document and QGIS.
  The layers are drawn in the palette and legend they have when shown.
- Viewer: **several files at once**. Files dragged together (from the system's file manager or the Files pane) are
  all layers of the Viewer, wherever they are let go, not only the last one, which was the one shown; the map shown
  stays under them. The Viewer takes files dragged from the Files pane (only the canvas did). A **folder** let go
  (or dragged from the Files pane) is a `raster.read` of it: its rasters as one stack, a time series played with
  *Play* (folders were left out).
- Viewer: **rasters dropped at once joined as one stack**, when asked: the Layers offer *As one stack (a time
  series)* (or *Keep them apart*). In one folder, one `raster.read` of a pattern of their names that names exactly
  them (`landuse_*.tif`) takes the place of their nodes (those another node reads stay); in several folders, a
  `stack.of` they feed. The stack is the layer in their place, shown and played with *Play*; Ctrl+Z undoes it.
- Problems: the *Ask the assistant* of an advice is under its text, no longer inside the box of a text of several
  lines.
- Builder: **a node that reads a file is named by it**: `landuse_1997` for `landuse_1997.tif` (dropped, opened with
  Ctrl+O or from the Files pane), a folder by its name, a pattern by its words (`landuse` for `landuse_*.tif`), no
  longer `raster_read_2`, `raster_read_3`... (accents dropped, another character a `_`, `map_` before a digit).
- Viewer: **the maps of a stack by their years**: when each file's name has a year, a different one, and the rest of
  the names is the same (`landuse_1997`, `landuse_2011`), the steps are named `1997`, `2011` (under *Play*, in the
  layer's choice of map, the charts along the steps, the figures and animations, and in the simulation's map tab).
- `vector.read` reads **a folder or a pattern** (`roads/*.gpkg`): the features of every file as one layer, in the
  coordinate system of the first (the others' carried to it), with every attribute of any of them (none where a
  file has none) and `file`, the file each feature comes from; a folder's `.json` files are left out. A run reads a
  folder (of rasters too) again when one of its files changes, is added or taken away.
- Viewer: **a folder of vector features** dropped (or dragged from the Files pane) is one `vector.read` of it, a
  layer over its rasters (a stack, as before; named by the folder and `_features` beside it); a folder with neither
  is refused.

## [1.37.0] - 2026-11-04

### Changed
- Viewer: **rotated grids and maps across the antimeridian are drawn reprojected**. A map layer, the curtain's or a
  side-by-side view's map whose grid is rotated (of the map's coordinate system or another) is no longer skipped
  with a note to run `raster.match`: its cells are carried by their full affine transform, and a layer in degrees
  across ±180° is drawn without the quads that stretched over the whole world; the difference, the Figure and
  *Export the view* do the same.
- Viewer: the line of the **Measure** tool is drawn in the views side by side too, carried to each view's own map
  (also when it is of another coordinate system).

## [1.36.0] - 2026-11-04

### Changed
- Viewer: **up to four views side by side**. Comparing side by side, *Another view* adds a third and a fourth map
  (another output of the run, or another layer of the map shown): three in a row, four in two rows, all zooming
  and panning together, the pointer marked in the others, in one colouring. A Plot keeps them
  (`compare.also`, also in `set_plot`), the figure has "the views side by side", and a cell of any view is traced
  from its menu. Comparing with a stack or series of another output, a *band* choice picks which of its maps each
  view shows (`compare.band` and `compare.also_bands` in the Plot, also in `set_plot`).
- Viewer: **the difference as a layer**. Comparing by *Difference*, *Keep as a layer* makes it a layer of its own
  (values: the other map minus this one; classes: the transitions, named): drawn with the other layers, at an
  opacity, its legend and summary in the Layers, kept in a Plot (`minus` of a layer, also in `set_plot`), and
  following its nodes renamed or removed. It is also the difference of two steps of one stack or series (a
  `minus_band` beside `minus` in the Plot's layer, "x.map step 5 minus step 1"), and of the band chosen of
  another output's stack.
- Viewer: **measure on the map**. *Measure* (or M over the map) turns on a line clicked point by point: each
  segment and the total length, and from three points the area and perimeter, in m, km, m², ha and km² (geodesic
  on the WGS84 ellipsoid for grids in degrees; the grid's units and cells without a coordinate system), with
  *Copy*. Right click or backspace takes the last point away, a double click ends the line, esc clears it.
- Viewer: **a file dropped onto it becomes a layer**. Files dragged from the file manager over the Viewer add the
  nodes that read them, kept as layers of the Viewer (the features over the maps), the first shown when the Viewer
  showed nothing, and run so that they appear without another click; the drop's overlay says so over the Viewer.
  Dropped elsewhere, a node each as before.
- Viewer: **the attribute table of a vector layer**, under the map, from the *table* button of the layer's row
  (and *Table* for features shown alone): sorted and filtered as the other tables, copied tab-separated or saved
  as CSV. A row clicked selects its feature, outlined on the map and zoomed to; a feature clicked on the map
  selects its row.
- Viewer: **the histogram of a map** under its legend: its values in 60 bars coloured by the ramp, with the
  minimum, maximum, mean and the cells with and without a value, worked out in a thread (from a sample of about
  4 million cells when the map is larger). A layer's settings have a *histogram* fold, and the legend of a map of
  classes has a bar under each class.
- Viewer: **labels on vector layers**, by an attribute (*labels* in the layer's style, with a size): polygons
  labelled at an inner point, lines at their middle, points beside them, on a thin halo; only the features in
  view and large enough for their label, none covering another, at most 2 000 at a time. Kept in the Plot's
  style and drawn in the figure too.
- Viewer: **Export the view...**, beside *Figure...*, writes the map as shown (the layers under and over it at
  their opacity, the vectors and their labels) as a PNG, or as an RGBA GeoTIFF on the map's grid that opens in
  QGIS over other data: a pixel to a cell, or ×2 and ×4 (up to 8 000 px on the long side), in the background.
- Viewer: **the views play together**: playing a series or a stack, the maps compared that are other steps of it
  (by a curtain, side by side, the views more) go on with it, each keeping its distance from the map shown.
- Viewer: **a layer of another coordinate system is drawn reprojected** on the fly instead of skipped: a map layer
  (or the map compared by a curtain) in another system than the map shown is laid on it tile by tile, as meshes
  the GPU stretches over the shown grid, and its value under the pointer is read through the inverse
  transformation. Side by side, a view whose map is of another system follows the others (the camera carried
  through the transformation of its centre, the scale from a cell's displacement; the pointer too), the Figure and
  *Export the view* (and the animation's frames) draw such layers reprojected pixel by pixel, and a layer's *zoom*
  carries its edges to the shown grid. The *Difference* of two maps of different systems carries the other map
  to the first one's grid cell by cell (nearest neighbour, classes and values alike). A rotated grid still has to be put on the map's grid first (`raster.match`).
- Viewer: **several features selected in the attribute table**: ctrl+click toggles a row, shift+click selects the
  range from the last row clicked, a plain click still selects just one and zooms to it; on the map a click selects
  a feature and ctrl+click toggles it. All of them are outlined (those in view) in every view, side by side too;
  the table's header tells how many are selected, with *Zoom to selection*, *Clear selection* (or esc), and *Copy
  selected* and *Save selected* for their rows only. *Select all shown* (or ctrl+A with the pointer on the table)
  selects the rows the filter leaves; a feature clicked in a view side by side that draws the Viewer's layers
  selects its row too; a ctrl+click that leaves one row selected no longer zooms to it.
- CLI and MCP: **a Plot exported by its id**. `terraflux plot model.toml cover -o cover.png` (or `.tif`, `--scale
  2`, `--set`, `--scenario`) and the tool `export_plot` run the model and write the Plot as the Viewer's *Export
  the view* does, without the application: the first map, the maps over it at their opacity, the features with
  their style and labels, each layer in the palette the Plot names, else the one the Viewer would choose, and the
  classes in the model's legend colours. Hidden layers and differences are left out and said so; a Plot that
  compares maps gives its map only.
- Viewer: **each view side by side with layers of its own**. A view still draws the Viewer's layers, but under
  *Compare* each view (the compared one and the views more) has a fold, "same as the map shown" or "its own": a
  compact list started as a copy of the Viewer's, with show/hide, opacity, remove and *Add a layer* (differences
  too). A Plot keeps them (`compare.layers`, a list of layers per view, also in `set_plot` and checked like the
  Plot's layers); the features selected are outlined only in the views that draw the Viewer's layers.

## [1.35.0] - 2026-11-04

### Changed
- Viewer: *Save as a new plot* and *Update the plot* keep the chart chosen for a table in the Viewer (its kind and
  columns, by name), not only the one a plot already had.
- Viewer: **a layer's band and palette**. A layer of a stack, a series or rasters on disk shows the map chosen
  for it in the Layers (no longer always the first). A Plot keeps each layer's band and the palette chosen by
  hand, and gives them back when it opens; the Document colours a map by the Plot that shows it. `set_plot`
  takes a layer's `palette` by its name in the Viewer (or its first word: `cividis`, `terrain, logarithmic`).
- Viewer: **Zoom to layer**: the "zoom" of a layer in the Layers puts the view on all of it (a map's extent, the
  box of vector features); the map shown fits again.
- Viewer: **the Layers in a panel of their own** at the map's left, hidden and shown by the *Layers* button above
  the map (no longer in the panel at its right); a layer's name is cut short rather than run under its buttons.
- Viewer: a layer is **dragged by its name** onto another's row to take its place in the Layers.
- Viewer: the pointer tells **the feature of every vector layer** under it (each by its layer's name, with its
  attributes), not only the topmost one's.
- Viewer: **the figure and the GIF take the layers**: the maps drawn with the one shown (under it or over it,
  at their opacity) are in each map of a figure (PNG, SVG, PDF) and in each frame of an animation, as the vector
  layers already were.
- Document: **a Plots section** lists the model's Plots (title, layers, comparison, the figure each is), and
  each figure has "open in the Viewer": the Plot opened there with its layers and settings.
- Website: the tour's pictures of the window made again from 1.34 (the Model in the whole window as it opens; the
  template run with its parts numbered; the Model pane), drawn by the test `readme_images::tour_images`.

## [1.34.0] - 2026-11-03

### Added
- A **frozen node inside a group** no longer has the nodes only it reads run (as at the top level).
- **Tables inside tables** are edited in place in the Document and the Node pane. A cell that is a list, rows or
  records in turn (the zones of manual targets, a demand per region, a demand's table) shows as a button, and opens
  for editing below its table.
- The QGIS project (`.qgz`) with labels, markers and rules was **checked in QGIS 3.34 itself**. `tools/check_qgz.py`
  reads it in PyQGIS: the layers valid, the classes paletted, the labels, the marker and the rules as written.
- **Layers in the Viewer.** The side panel's *Layers* list the outputs kept on the map whatever node is clicked,
  from the top: shown or hidden, up and down, taken away, a map's opacity, vector features' style; a click on a
  layer's name shows that map, with its legend and settings. The map shown is kept with 📌 (a double click on a
  node keeps it too), and *Add a layer* adds the run's other maps and features. The layers under the map shown are
  drawn under it, those over it over it; the pointer tells each map's value by its name. Layers follow a node
  renamed and go with one removed, and the QGIS project takes the visible ones. (In place of *Maps over it* and
  *Vectors over the map*.)
- **Plots: views of the results kept with the model** (`[[plots]]` in its file), in place of `view.plot` nodes.
  The Viewer's **Plots** menu opens one (its layers, the map shown and its band, its comparison, a table's chart),
  keeps what the Viewer shows as a new one, updates the one open, renames its title or removes it. The Document
  and the report show the Plots as figures, the comparison of scenarios offers their maps, Many runs keeps what
  they show, and the Python script names them. A Plot follows a node renamed or grouped, and loses a layer whose
  node is removed; `terraflux check` checks what they read.
- **`set_plot` and `remove_plot`** for agents (`terraflux mcp`) and the assistant: a Plot's layers (the lowest
  first, each as opaque as asked, a band), a map compared and how, a table's chart, a title and the record. The
  *results* guide tells of them.

### Changed
- **The `view.plot` nodes of a model are Plots** when it is read (or when an agent adds one): their `under`
  chains, maps, opacity, features, tables and charts, comparisons, titles and notes kept; what read a plot's
  `shown` reads what it showed. Plots inside groups stay nodes. A template begins with its last Plot open.
- **`view.plot` is retired** (until 2.0, for the plots inside groups): out of the operators' library, the search
  and `list_operators`; `add_node` with it says to use `set_plot`, and `describe_operator` says why.
- **One place to see a result: the Viewer.** A double click on a node shows its output in the Viewer (brought to
  the front), run first when it has not run yet (quietly, as the preview after an edit), instead of opening a map
  tab of its own; a `view.plot` that runs no longer opens a tab either (it shows in the Viewer when clicked, with
  the maps under it), and a template begins with its last plot in the Viewer. Map tabs are gone. The Viewer
  compares two maps itself (*Compare*: curtain, side by side or difference), and its list now begins with the
  layers kept.

## [1.33.0] - 2026-11-03

### Added
- The assistant **makes the calls a model writes as text**: when an answer has no tool calls, the calls of the tools
  offered written in it are found and made, as Python (`edit_node(node="a", ...)`), as JSON (a block with `name` and
  `arguments`, Hermes' `<tool_call>`) or as tags (`<function=...>`, `<invoke name="...">`, `<list_operators>
  <query>...`), the values given the types the tools ask for; they enter the conversation as calls and leave the text
  shown. An answer that announces a call without making it is asked again, once. Smaller models served by vLLM
  (Qwen) wrote their calls this way, and the tasks ended without a tool. Measured on the evaluation's 10 tasks with
  the HU Berlin Qwen: 5 and 2 before, 4, 7 and 6 after; the evaluation keeps the conversation of a failed task.
- The assistant **paces its requests** to a server that limits them (the HU Berlin one, 15 a minute), reads Qwen's
  `<function_calls>` and the tags inside `<tool_call>` by the tool's name and its schema, and asks again an answer
  that ends promising to act ("Let me build this model now.").
- **More advice on the method** (check_model, after a run, the assistant): a species distribution model of records
  not cleaned or thinned (`sdm.one_per_cell`, `sdm.thin`); a prioritization never evaluated
  (`prioritize.evaluate`); a resistance of 0 or less read by Circuitscape, Omniscape or a cost (a warning, from the
  data loaded); a Markov chain whose `steps` does not divide the years between its maps (the years read in the
  files' and nodes' names), or whose maps are in the wrong order; weights of evidence without any distance among the
  drivers (when the drivers can be told).
- The advice on the method in the **Problems pane** (a section of its own, each piece with its nodes and *Ask the
  assistant*) and in the **report** (HTML, PDF and `terraflux report`, in English or Portuguese, beside the checks).
- **Advice on the operators** kept with them (a field of the operator's description, filled by the operator or by
  a table of the operators' crate): describe_operator and the documentation tell it; new advice for the graphs of
  connectivity (`conn.*`), the hydrology (`hydro.*`), InVEST (`services.*`), the agents, REDD+ and the emissions.
- `terraflux mcp` answers **as data** too (MCP 2025-06-18's `outputSchema` and `structuredContent`): check_model
  (whether the model runs, its errors, warnings and advice) and run_model (the files written, the warnings, the
  advice, or why it did not run).
- `terraflux mcp` **asks the user** (MCP's elicitation, when the client can): set_input of a map of classes without
  a note asks what each class is, and keeps the names as the input's note.
- The guides of the methods **in Portuguese** (`guide` with `language: pt`), for the assistant that answers in it.
- The **Python script** writes a conservation plan with **management zones** (`prioritize.optimize` with
  `zone_costs`, the features' `contribution` and `mandatory_allocation`) as landschaft's: the zone of each cell, and
  each zone's share as a layer.
- **Stop and progress** in more long operators: Monte Carlo's draws (`uncertainty.monte_carlo`), a metapopulation's
  and MigClim's steps (`conn.metapopulation`, `sdm.migrate`) and the importance of the nodes at several distances
  (`conn.node_importance`) say how far they are and stop between two.
- The Viewer shows a node of a **`control.each`'s body** (what it gave for the last element), and **a step chosen**
  of a loop: *step* (or *element*) in the Viewer's header, *last* or a number; the run keeps that step (*Run to see
  it* when it was not kept), and going step by step shows the step it is paused after.
- **Grouping nodes, easier to find**: with several nodes selected, a bar at the canvas's bottom tells how many and
  groups them (*Group them*, ctrl+G); a node's right-click menu groups the nodes selected, and a group's opens it,
  ungroups it and saves it as my node (a calculator's too). A group made asks for its name at once, with a tick to
  save it as my node then.

### Changed
- The application **opens on the Model tab**: the Document pane is no longer there from the start, and comes with
  the first node, as the Node pane does (in a tab behind the model's); a layout kept opens with the model in front,
  whatever tab was there when it closed.
- **A node just added tells nothing until a run is asked for**: the preview run after an edit no longer starts while
  the node shown, or one it needs, is not filled in (a `raster.read` whose file is not chosen yet), and such a run
  that fails no longer shows the error and *Why it failed* (the node's pane says, quietly, that a run tells why); it
  no longer tells the nodes not filled in either. A run asked for (F5, ctrl+enter) tells them as before.

### Fixed
- A stray `</think>` (a thinking model's closing tag without its opening one) no longer shows in the assistant's
  answer, even when it arrives split in two; nor an empty code block left where a written call was taken out.

## [1.32.0] - 2026-11-02

### Added
- **Checks as operators of their own**, beside `check.assert`: `check.range` (a value between a minimum and a
  maximum), `check.no_nodata` (a map without missing cells, or at most a share of them, within a mask if given),
  `check.area_kept` (a class's area kept from one map to another, within a tolerance), `check.accuracy` (a map
  against a reference: the overall agreement or kappa at least a value) and `check.demand` (a step's areas as the
  demand asked, within a tolerance). Each gives `ok`, stops the run when it fails (or only warns, with `warn`), and
  says its message (one of its own when none is given); the report's seals, the Document pane and `terraflux test` count them.
- A **sentence for every operator**, in English and in Portuguese: the 322 that had none (and the new checks), in
  the report, the Document pane and the methods.
- The Calculator's preview **follows the Viewer**: the part of the map in view there (its zoom and place), worked
  out again when it moves; *Follow the viewer* turns it off, and a part drawn by hand on the preview too.
- **Node libraries of a group** reached by name: a shared library's nodes are listed with the user's own (in the
  operators' pane, the quick search, the commands and the assistant's list), as `my:lab/ndvi` when the name is in
  two libraries; the assistant saves a group or a calculator in a library with **save_my_node** (its name, what it
  does, the library), the node in the model becoming that version.
- **MODIS** in the catalog and in `data.download`: besides NDVI and EVI, the **land surface temperature** of the day
  and of the night (MOD11A2, 8 days at 1 km), the **burned area** (MCD64A1, monthly at 500 m, the day of the year a
  cell burned) and the yearly **gross and net primary production** (MOD17A3HGF, 500 m), each from its product on the
  Planetary Computer and cited by its own reference.
- **Export to QGIS**: a vector layer's **labels** by an attribute (around a point, along a line, over a polygon,
  dark on a light halo), its points' **marker** (circle, square, triangle, diamond, star, cross, pentagon, hexagon)
  and its style **as rules** (QGIS's rule-based renderer: a rule each class or range, and one for the features of
  none), chosen for each vector layer in the export window.

- The **Python script** of a model translates the **tables** (filter, calc, aggregate, select, sort, join, pivot, in
  pandas, their expressions as the calculator's), the **vector layers** (read, rasterize, zonal, extract, filter,
  calc, buffer, clip, dissolve, reproject, joins, overlay, random points, polygonize, distance, grid zones,
  measures; geopandas and landschaft), the **logistic regressions** of the land change models, Dinamica's roads,
  cost maps and continuous patcher, the demands, and a prioritization's **connectivity penalties**. Still said, not
  translated: a step of Dinamica's, PLUS's or CLUE-S's allocation, and a prioritization by zones.

### Changed
- The **Node**, **Prepare** and **Calculator** tabs come when they make sense, not when the program opens: Node with
  the first node or template, Prepare (behind it) and the Viewer with the data, the Calculator when it is asked for.
  A layout kept with them does not bring them back, and a tab closed stays closed until another model opens.
- **Save as MP4...** needs **nothing installed**: the video (H.264) is encoded by OpenH264, built into the
  application, and written as an MP4 that plays before it is all read; ffmpeg is no longer used.

### Fixed
- A Viewer that showed a map and then a table or a value (a `raster.write`'s path) no longer stays busy for ever,
  waiting for the map's tiles.

## [1.31.0] - 2026-11-01

### Changed
- A new installation opens in **Slate**, the modern dark theme (a theme's name unknown in the settings too); the
  theme chosen before stays.
- **Process a folder...** only builds the nodes (the folder read, the operator, what writes): its operator selected
  for its settings, nothing runs on the folder's files until Run.
- The steps of a **series** (and the layers of a stack) go from one to the next **smoothly**: the map shown is kept
  under the next one while its tiles come, the next fading in over it, and its colours and legend stay until the
  next one's are worked out (no longer a blank map or a spinner between two dates).

## [1.30.0] - 2026-11-01

### Added
- The **report in Portuguese**: File, Export report (and the Document pane's) asks for the language, In English or
  Em português, and `terraflux report model.toml -l pt` writes it so. The report's own words (the sections, the
  tables' headings, what a step takes and how a loop repeats, the figures, the checks and their badges, a
  comparison's words, the footer), the stages, the kinds of the inputs and the parameters' ranges are in
  Portuguese, and so are the methods and citations at its end. The operators' descriptions of their settings stay
  in English.
- The sentences of the steps in Portuguese, for the 36 operators that have one (`Reads the rasters of…` is `Lê os
  rasters de…`), in the report and in the methods: a step of the methods in Portuguese is its sentence and its
  note, no longer only its note or its settings.
- Sentences for 73 more operators, 109 in all, in English and in Portuguese: those the templates and tutorials
  use most (map algebra, stacks and tables, vectors, terrain and water, the habitat graphs and circuits, the changes
  between maps, agents, species distribution models, Zonation, Marxan and the services among them).
- A setting that is a **table of several columns** is edited **cell by cell**, in the Document pane and in the Node
  pane: a list of rows (a legend's `[[value, "name", "#colour"], ...]`), a list of records (`[{"class": 1,
  "carbon": 50}, ...]`) or records by name (`{"forest": {"carbon": 50}, ...}`), under the columns' names, each
  column as wide as its values; **+ row** (a copy of the last), **+ column** and **x** to take a row away.
- The **Changes** pane matches the rows of a table before and now by a **key** (*rows matched by class*): a column
  of both of whole numbers or names, none twice, some in both (`id`, `key`, `class`, `value`... first), shown first.
  A row only the table before has is in red, a row only the table now has in green, and a row matched is compared
  cell by cell wherever it is; without a key the rows are matched by their place, as before.
- **Guides** for the assistants (`terraflux mcp` and the application's Assistant): the tool `guide` gives the
  method of a field step by step, with its operators and templates (land-change, connectivity, landscape-metrics,
  conservation-planning, species-distribution, data-preparation), and how a model is built (nodes, calc,
  containers, record, results). `terraflux mcp`'s instructions are short now (2,200 characters, from 5,500: Claude
  Code cut them at about 4,000) and send the assistant to the guides.
- **Advice on the method** in `check_model` and after `run_model`, as a paper's reviewer would ask, each with what
  to do and its guide: a land change simulated and never compared with an observed map, or compared with the map
  it was calibrated on (a warning); a stochastic simulation run once; distances, areas or metrics on maps in
  degrees (a warning); landscape metrics of a continuous map (a warning); a species distribution model evaluated
  only on random folds. `inspect_file` notes a raster in degrees or without a coordinate system.
- `describe_operator` adds advice on the operator's use (19 operators: the calibrations, simulations and
  validations of land change, MSPA, metrics, SDM folds, planning, circuits), the method it is part of and the
  papers to cite, and the operators it reads and gives to in the templates, with the templates that use it.
- `list_operators` and `list_templates` (now with a `query`) find by Portuguese words too (*densidade de
  bordas*, *pesos de evidência*, *desmatamento*), without accents or small words, by plurals, and give the
  nearest ones when none has every word.
- `terraflux mcp`'s prompts `build_model`, `adapt_template`, `review_model` and `explain_model` (with
  `explain_cell`), and its resources: the guides (`terraflux://guide/...`), the templates' files
  (`terraflux://template/...`) and the operators' documentation (`terraflux://operator/{name}`). A template copied
  says which of its nodes draw data, to be replaced by the user's files.
- Two tasks more in the Assistant's evaluation: landscape metrics asked in Portuguese, and a land change model's
  method reviewed (its missing validation) without changing it.
- The viewer shows the map of a node inside a **loop's body** (the last step's, *years/grow (the last step)*) and
  inside a **switch's branch** (the branch that ran), as inside a group: `control.repeat` gives what its body's
  nodes gave at the last step as `node.port`, and `control.switch` what the branch that ran gave as
  `branch.node.port`; the Document pane shows the steps of the branch that ran with their values.
- A node inside a group (groups inside groups too) can be **frozen**: Freeze in the Node pane, its result kept in
  `<model>.frozen/<group>.<node>/` and used whatever it reads, in the application and by `terraflux run`.
- Scenarios compared on **several outputs at once**: the outputs checked in the list beside Compare, a block
  per output, each scenario run once up to all of them (what they share run once), in the application and through
  Many runs.
- The **whole transition matrix** of two scenarios' maps of classes, under the most common changes: a row per class
  of the first scenario's map, a column per class of the other's, the cells of each pair, with the totals.
- **Trace**: the card of a patch's operator draws the patch the cell is in on the Viewer's map (its border, around
  the cell; picking another cell lets it go). An allocation's card says **why** the cell changed: for `lulcc.step`
  its suitability for each class there, how the class it took ranks among the cells and the elasticity of the class
  it had; for `dinamica.step` its probability of that transition there and how it ranks.
- **Breakpoints**: Breakpoint in the Node pane (a red dot on the canvas) pauses the run before that node, at every
  step for a node of a loop's body, with what ran before it shown; the status bar says *paused before …* with
  **Next node** (run it, and pause before the next node) and **Continue** (on to the next breakpoint).
- **Calculator**: the outputs of a script (`out`) have the type of their value in the graph's checker, from the
  types of the node's inputs (a map, a stack, a number; rasters read a map or a stack): an output connected where it
  does not fit is refused when the model is checked, not when it runs (a number of all the cells where a map is
  taken, a map where a number is), and the assistants' replies say what each output is.
- **Remote libraries of nodes**: a git repository (cloned, then pulled when the application starts; `git`
  installed) or a folder on the web (read by its `index.txt`, which Terraflux now writes in every library it saves
  in), added by its address under Settings, Node libraries; its nodes offered beside yours, only read, with
  **Update** to fetch it again. A library's `library.toml` gives it a `name` and its `writers`: only they save in a
  shared library and take nodes out of it, the others' dialog showing it greyed and saving a node of it edited in
  their own library.

### Fixed
- `prioritize.actions_raster` with a feature whose layer has no amounts (all 0) says that it dropped it ("The
  following features are not represented…"), as landschaft does; the feature was dropped, but without a word.
  Checked against landschaft 1.58.1: the rasters' references regenerated the same, a new case of a feature without
  amounts (the same tables, relative targets and optimum), and `prioritize.actions` with units' ids unsorted, with
  gaps and as integers is the same problem as with ids 1..n.
- `terraflux.exe` builds in a worktree beside another sharing `target`: the build script of its icon read the
  folder it was built in, not the one it runs for.

## [1.29.0] - 2026-11-01

### Added
- **Difference**, the third way of **Compare** in the Viewer and the map tabs, beside Curtain and Side by side:
  one map of how the two differ, drawn where the map is. Maps of values: the other map minus this one, in a ramp
  centred on 0, with the mean difference, its range and the share of cells higher and lower. Maps of classes: the
  transitions, a class each named by the legends of both maps ("forest -> pasture"), the most common first (24 at
  most, the rest together), the cells that kept their class in grey, with how many cells changed. Made in the
  background, on the map's grid (the other map read where its cells are, by their coordinates; every few cells
  when the map has more than 16 million); a cell clicked on it is the map's cell, and **Figure...** offers it as a
  figure.
- The **report** (HTML and PDF, from the app and `terraflux report`) tells how the model's checks fared: the
  health at the top (so many passed, warned, failed, not run), a line per check with its badge, and the badge on
  each check's step and on the containers holding checks (their worst). Its last section, **Methods and
  citations**, is the methods paragraph and the references to cite in the APA style (Terraflux's among them), and
  those the nodes give.
- In the report, the maps of a loop's steps are an animation in the HTML (an animated GIF, each step written on
  its map, the legend under them; 40 steps at most, the first and the last among them) and four steps in a row in
  the PDF, in one colouring, instead of the last step alone.
- `view.plot` compares: another map connected to its new **compared** input, and **compare** (`curtain`,
  `side by side`, the default, or `difference`). Its tab opens with that comparison, and in the report its figure
  is the two maps side by side in the colours they share, and their difference when asked, with what it says and
  its key.
- **Play** in the Viewer for a map of several layers (a loop's steps, a stack, a raster's bands), and a **speed**
  in frames a second, the same for playing (here and in the Simulation pane) and for the animations saved.
- **Save as MP4...** beside Save as GIF...: the same frames as an H.264 video, made by ffmpeg when it is installed
  (on the PATH, or named by `TERRAFLUX_FFMPEG`), with 4 threads at most.
- `data.download` takes an **area**: polygons (an area drawn, a region) to cut the data to, no data outside them
  (gdalwarp's cutline; the data's own no-data value, else NaN for values and 0 for classes). Without `like` and
  `extent` the grid spans the area, in longitudes and latitudes widened to whole cells (`cellsize`, by default
  the area's longer side in 500 cells). The data catalog offers it (**Cut to the area**, and the grid **the
  area's bounds**), and **Add, and data for it...** under Draw on the map adds the polygons drawn and opens the
  catalog so.
- The features drawn on the map are **edited** before they are added: a vertex dragged, a new one dragged from
  the middle of an edge, one taken away by a right click (a rectangle stays one). And their **attributes typed**:
  columns named as wished, a value per feature, written as numbers when all are, else as texts.
- `data.download` reads three more sources: **ESA WorldCover** (`esa_worldcover`: land cover at 10 m, 2020 and
  2021, its 3-degree tiles), Hansen's **Global Forest Change** (`global_forest_change`, v1.13: the tree cover of
  2000, the year of loss up to 2025, gain, the mask) and **MODIS** (`modis`: NDVI or EVI of the MOD13Q1 16-day
  composite begun on a `date` or last before it, one layer a date, found in Microsoft's Planetary Computer and
  read with its anonymous tokens). Tiles over the sea are left out when warped, as before.
- The **data catalog** has their cards and one for **GBIF** (a species, countries and years, within an area's
  bounds), which adds an `sdm.download_gbif` node. It says what of the choices is **in the cache already**
  (nothing to download, or so many layers to download), and **Preview** fetches the first layer over the grid,
  coarse (200 cells a side), in the background and shows it, or GBIF's count of the records.
- The **figure** (Figure... in the Viewer) is saved as **PDF** too: a page of the figure's size, its lines, shapes
  and text as they are (Helvetica) and the maps as pictures. The **vector layers** drawn over the map in the Viewer
  are drawn over it in the figure, in their colours (polygons filled and outlined, lines, points), cut at the map's
  edge. A **location inset**, on by default: the land of the world around the map (Natural Earth's, public domain,
  in the application) and the map's place in red, in its bottom right corner.
- **Export to QGIS** writes the project as **`.qgz`**, QGIS's own (zipped), or as `.qgs` (text), a choice in its
  window. The **vector layers** go in the colours they have over the map in the Viewer: their colour, fill and line
  width, or by an attribute, its classes a colour each (a categorized renderer) or its range of numbers in seven
  ranges along the Viewer's ramp (a graduated renderer). The vector layers drawn over the map are chosen at first,
  with the map shown.
- The **Python script (landschaft)** translates the **containers** into Python's own: a loop (`control.repeat`) a
  `for` over its steps, its carried values assigned at the end of each step, what it collects appended to lists,
  its end (`until`) a `break`; `control.each` a `for` over the collection's elements (`index`, `name`, `item`),
  what it collects gathered (maps a stack of the elements' names, tables one table); `control.switch` an
  `if`/`elif`/`else` per branch; a group its body in place, its inputs a variable each. A node of a body without a
  counterpart stays a comment without losing the container. New counterparts: `stack.of` and `table.of`; the
  **species distribution models** (`sdm.prepare`, `sdm.background`, `sdm.fit` with MaxNet, the GLM or the random
  forest, its predictions, importance and response curves); the **landscape metrics** (`metrics.class`,
  `metrics.landscape`, `metrics.patch`, by `ls.landscape_metrics`); **prioritization** (`prioritize.optimize` as
  prioritizr's `problem()` chain landschaft builds: objective, targets, weights, boundary penalties, locked units,
  neighbours, contiguity, decisions, portfolios, the solver's gap and time limit; `prioritize.zonation`); and the
  pieces of the **land change models** (`markov.fit`, `woe.fit`, `woe.probability`, `change.transitions`,
  `validate.three_map`), the allocation (Dinamica's, CLUE-S's, PLUS's) said to be landschaft's
  `ls.simulate.run(..., preset=...)`, which runs only a whole model; and `surface.terrain`, `tpi`, `vrm`,
  `curvature`. Checked by running both: Zonation, the prioritization, the SDM, a loop, a switch and a loop over a
  stack give the same maps. In the templates, 390 nodes are translated (319 before) and 170 left (229 before,
  counting none inside containers then).

- **Methods and citations** in **Portuguese** too (a choice in its window): the paragraph, the methods' names and the
  headings, Terraflux's reference ("Versão", "Software"); each step told by its node's note, else its settings.
  **`terraflux methods model.toml`** on the command line (`--language en|pt`, `--format text|bibtex|ris`, `-o`
  a file) and the **`methods`** tool of `terraflux mcp` and of the application's assistant, with the same choices.

### Fixed
- The Python script's calculator: the neighbourhood sums (`nbsum`, `nbmean`, `nbcount`) are exact sums, not a mean
  times the cells (`3 / 9 * 9` was 2.9999999999999996, so `nbsum(...) >= 3` failed where Terraflux grew a cell).

## [1.28.0] - 2026-10-31

### Added
- **Compare** in the Viewer and the map tabs: another output of the run, or another layer of the one shown (a
  step of a loop, a band), beside the map. As a **Curtain**, the other map right of a line dragged across the map
  by its handle, the names of both at the top; **Side by side**, two views that zoom and pan together, a cross in
  one where the pointer is in the other. The two in the same colours (the classes of both, or the range of both),
  the values of both under the pointer; a map of another grid placed by its coordinates.
- **Methods and citations** (File menu, the Document pane, ctrl+K): the methods section of a paper drafted from
  the model. A paragraph with the version of Terraflux and each method the model uses, with its papers and the
  software reproduced ("We computed morphological spatial pattern analysis (MSPA; Soille & Vogt, 2009) with
  Terraflux, which reproduces GuidosToolbox (Vogt & Riitters, 2017)."); the model step by step; the references
  in the APA style (those of the website's Cite Terraflux, and the nodes' own), copied, or saved as BibTeX or RIS.

- **This cell in every map**: a cell clicked in the Viewer or a map tab lists its value in every map of the run at
  the same place (by their coordinates; a stack's first layer, rasters' band chosen), a name showing that map,
  the list copied as text.
- **Draw on the map** (the Viewer): polygons, rectangles, points or lines clicked on the map shown, a study area
  or plots; **Add to the model** writes them as a GeoPackage beside the model, in the map's coordinate system, read
  by a new `vector.read` node and drawn over the map.
- **Python script (landschaft)** (File menu, ctrl+K): the model as a Python script of landschaft, for a paper's
  code or a reviewer: a call a node in the order the model runs them, its note as a comment, its settings as the
  function's arguments (by landschaft's names), its outputs saved by `ls.save`; a calculator's script kept and run
  in numpy by a helper in the file, reclassification rules as `ls.reclassify`'s table, the foreground of the
  patterns (MSPA, SPA, FAD, FOS, Accounting) through `ls.reclassify(foreground=...)`, the neutral landscapes with
  Terraflux's seeds and defaults (the same maps). Nodes without a counterpart known stay as comments with their
  settings, listed at the top and in the window; copied, or saved as a `.py`.
- **Data catalog** (File menu, ctrl+K): the datasets `data.download` fetches as cards (WorldClim 2.1 and its
  CMIP6 futures, CHELSA 2.1 and its futures, the Copernicus DEM, SoilGrids 2.0, MapBiomas Brazil, Hydrography90m),
  each with its resolution, years, coverage, licence and the paper to cite; searched, filtered by theme. Its choices
  (the bioclimatic variables by name, months, model, scenario and period, soil property and depths, years) and a
  grid (a map of the model, or a box of longitudes and latitudes); **Add to the model** adds the `data.download`
  node so set, the reference in its references (Methods and citations cites it).
- **Figure...** in the Viewer: the map shown as a figure for a paper, alone, beside the one compared, or every layer
  of a stack or a series as panels (a), (b)... in columns: a title, the width of a journal's column or page (mm),
  150 to 1200 DPI, the legend (one for maps of the same colours, else each map's), a scale bar in metres or
  kilometres (also on longitudes and latitudes), a north arrow and the coordinates along the edges, previewed as it
  is set. Saved as PNG (its resolution recorded) or as SVG, its text kept as text to edit in Inkscape or Illustrator.
- **Export to QGIS...** (File menu, ctrl+K, and the viewer's Save output): the maps and vector layers of the run
  chosen, written to a folder (GeoTIFF, GeoPackage; a raster read from a file referred to where it is) with a QGIS
  project that opens them in Terraflux's colours: the classes as a paletted map with their names, values on the
  same ramp and range, the vector layers on top, the coordinate system of the maps; a `.qml` style beside each map
  written. **Open in QGIS** opens it.
- **Save as GIF...**: the steps of a simulation (the Simulation pane), the maps of an input's stack, or the
  layers of a series, a stack or rasters (the Viewer) as an animated GIF, for a talk or a class. Each map in the
  colours it is shown in (a series' steps in one colouring), its step or name in its corner and its legend under
  it, a small map enlarged and a large one read every few cells; made in the background, the last frame held.
- Colour-blind readers: the ramps **cividis** and **orange to purple, centred on 0** (ColorBrewer's PuOr) in the
  Viewer's colours, and **View > Colour vision**, every map, legend and picture drawn as a reader with
  protanopia, deuteranopia or tritanopia sees them (Machado et al. 2009), said in the bar at the top until set back.

### Changed
- The rules of **raster.reclassify** are a table of three columns in the Node pane, **from**, **to** and
  **becomes**: a range is its start and its end in two fields (`0`, `10`, `2`), no `0..10` to type; `to` left
  empty takes only the values in from (`1 2 3`, `*`), `inf` as the end takes every value above the start. The
  rules' text takes a line of three fields too, `0 10 2` (or a table's rows pasted, with commas or tabs), the
  range `0..10 -> 2`; models written with `..` read as before.

## [1.27.0] - 2026-10-31

### Added
- Modern themes: **Slate** (dark), **Paper** (light, in the colours and the font of the website, Source Sans 3)
  and **Canopy**, beside the classic ones, kept as they were. Rounded tabs and panes, the Node pane in cards
  (inputs, settings, body, the run), its references as coloured chips, switches for Bypass and Freeze, lighter
  plot grids and a quieter toolbar. Slate and Canopy use Inter.

### Fixed
- Larger text (Settings, **Text size**) no longer spills out of its boxes, in the classic and the modern looks:
  the toolbar puts its tools in a **Tools** menu when there is no room, the status bar, the Node pane's name and
  **Rename**, the Operators pane (long subjects and names cut short), the model's bar, the Files pane, Many
  runs, Parameters and the Assistant fit their panes at every size. A test draws every pane, menu and window at
  each size and theme and checks that nothing overflows.
- Windows no longer asks for the firewall when a pool shared on this computer only (the tests) announces itself:
  its beacons stay on this computer. The firewall is asked only when the computer is shared or lent on the network.

## [1.26.0] - 2026-10-30

### Added
- The table gathered from the files of a folder names each row's file in its column `layer` (`clc_r0c0`), where it
  held the file's number. Tables can name the numbers of a column (`labels`, as a factor in R): the CSV, the JSON
  and the text of `table.write`, the outputs of a run, the table viewer and the charts show the names. When the
  files' names end in different numbers (`forest_2010`), the column still holds those numbers.
- A chain of nodes run on a folder keeps in each file's record what is read after it, when small (a table, a
  number, a text). A model with MSPA and its metrics no longer runs every file again: the files done before are
  taken from their records, the table of all of them the same.
- The files that failed in a chain of nodes are listed in the Node pane of its first node, each with why, with
  **Run again** (the files done before not run again) and **Copy the list**. The engine tells them to its observer
  (`Observer::failures`).
- `distance.proximity` and `landscape.mspa_tiled` given a folder run on each of its files, as `raster.terrain`
  and `raster.fill` do. `mspa_tiled` writes a file per file, by its name (`{name}` in `path`, else the name before
  the file's).

### Fixed
- `distance.proximity` and `landscape.mspa_tiled` given a folder measured only its first file, without saying so.

## [1.25.0] - 2026-10-30

### Added
- A folder of rasters goes straight into any node that takes a map, and runs through the chain file by file.
  A `raster.read` of a folder (or a pattern) feeds a node where it takes one map. That node, and the nodes after
  it that take its maps, run as one chain, file by file: a file is read, passed along the chain and written,
  then let go. Before, each node ran on all the files before the next one, with every result in memory.
  - A file that fails does not stop the others. A warning says which files failed and why; the run fails only
    when every file does.
  - A warning that some files give is told once, with the files that gave it.
  - At the top level, the maps of the chain that are only written to files are not kept in memory when they
    would take more than 2 GB.
- A chain of nodes on the files of a folder runs several files at the same time, as many as the threads. The
  `raster.read` setting `workers` sets fewer for large files, each of which is in memory while it runs. The
  files run in a pool of that many threads, which the operators share. The results, failures and warnings are
  the same, in the files' order, whatever the number of workers.
- A chain that writes files records each file next to its first file written (`mspa/clc_r0c0_mspa.json`): the
  file read, the files written, the nodes and their settings, the time taken and Terraflux's version. When
  nothing but the chain's `raster.write`s reads what it gives, a file whose record says it was done with the
  same nodes, settings and file, with its files still there, is not run again. A run stopped halfway goes on
  where it stopped, and a run again after failures runs only the files that failed.
- The boxes of what a scope reads on the canvas (the model's inputs `$name`, a container's values `@name`, the
  outer nodes a body reads) are moved, selected and deleted like the nodes. Their places are kept in the model
  (`places`); those never moved stay in the column at the left.
  - An input of the model, clicked, is edited in the Node pane: renamed (what reads it, its parameters and
    scenarios follow), its file chosen or its value typed, its note written, or deleted.
  - **Make it a node** turns an input of a raster file into a `raster.read` of its file, read by what read it.
    **Make it an input of the model**, in a top-level `raster.read`'s right-click menu, does the reverse.
  - Deleting an input of the model (Delete, its menu, or the MCP tool `remove_input`) disconnects what read it.
    Before, the MCP tool left those references dangling.
- `raster.read`'s `first`: only the first files of a folder, to try the nodes after it on a few before all.
- "Run on a folder..." in the node pane and in a node's new right-click menu on the canvas. It reads a folder
  where the node takes its first map: the `raster.read` that feeds that input reads the folder, else one is
  added at the node's left.
- A node given two folders (or more) where it takes maps runs on their files paired by name: `uso/bacia_07.tif`
  with `estradas/bacia_07.tif`. Names that differ are paired once each folder's common start and end are cut at a
  `_`, `-`, `.` or space (`uso_bacia_07` with `estradas_bacia_07`). The files with no pair are left out, with a
  warning. The records and the resuming cover the files paired.
- **Process a folder...** in the File menu, the command palette and the empty canvas. It asks for the folder, then
  for the operator (the operators that take a map), and builds the nodes: a `raster.read` of the folder, the
  operator, a `raster.write` of its map to `<folder>_<op>/{name}_<op>.tif` and a `table.write` of its table to
  `<folder>_<op>.csv`. The operator is selected, to choose its settings.
- **Make it a chain of nodes** in the Node pane of a `batch.run` turns it into the same nodes: a `raster.read` of
  its folder, its operator with its settings, and writers to its output folder. It is refused, saying why, when
  something reads its table or report, or when it reads a list of files or the sub-folders.
- The tutorial "Batch processing" is rewritten for folders given to the nodes, and so is the batch section of
  "Large maps".

### Fixed
- The files of a folder of different grids (tiles, basins, municipalities) keep their names through the nodes
  run once per file. `raster.write` with `{name}` wrote `10_mspa.tif` instead of `clc_r0c0_mspa.tif`.
- `raster.write` of maps of different grids without `{name}` in its path writes a file each, named by the map's
  name followed by the file's name (`mspa/mspa.tif` gives `mspa/clc_r0c0_mspa.tif`). Before, it wrote one
  file on the first map's grid.
- The landscape metrics of rasters on disk (`metrics.patch`, `metrics.class`, `metrics.landscape`,
  `metrics.window`, `metrics.sample`) run once per file of a folder and per band of a stack, as the operators of
  a map do. Before, they measured only the first file, without saying so.

## [1.24.2] - 2026-10-29

### Fixed
- `move.extract_covariates` reads rasters whose coordinate system is only an EPSG code (as `data.download`'s)
  under tracks in another one: every value was empty, the tracks were not reprojected. The species
  distribution operators read such rasters the same way now.

## [1.24.1] - 2026-10-29

1.24.0 was not published: its Linux build failed. This release has its additions and these fixes.

### Fixed
- The Linux build: landschaft's x87 `log1p` (R's, which the movement operators use) did not compile with GCC.
- Vectors are written to GeoPackage (and SQLite) in one transaction: 47 000 points in half a second, not 29
  seconds (a commit per feature before).
- `vector.filter` and `vector.calc` read attributes whose names have other characters than letters, digits and
  `_`, as Movebank's: `individual-local-identifier` is `individual_local_identifier` in a script,
  `eobs:type-of-fix` is `eobs_type_of_fix` (the error of a name not found lists them so).

## [1.24.0] - 2026-10-29

### Added
- Animal movement, `move.*` (landschaft's `ls.move`: the methods of move2 and amt), each bit for bit with
  landschaft on its reference tracks (`tools/make_move_reference.py`):
  - tracks: `move.read_tracks` reads Movebank's files (csv, csv.gz or zip, the columns typed as pandas does),
    `move.as_tracks` makes tracks of points or of a table's coordinates, `move.simulate_brownian` draws Brownian
    tracks; a track is a point vector with a time column (ISO 8601 texts, read in UTC, or numbers) and a track
    column, both found by their usual names or given by the `time` and `track` settings;
  - measures, each a column added: `move.distance` (on the ellipsoid in longitude and latitude),
    `move.time_lags`, `move.speed`, `move.azimuth`, `move.turn_angle`, in the units chosen;
  - filters: `move.filter_unique` (the duplicated times, by subsets or the first, last or a sample),
    `move.filter_per_interval`, `move.filter_movebank_visible`;
  - lines: `move.track_lines`, `move.segments`, `move.track_data`; `move.interpolate` (at given times or every
    unit, the great circles of s2 in longitude and latitude) and `move.aeqd_crs` (the azimuthal equidistant CRS
    centred on the tracks, and the tracks in it);
  - amt: `move.resample` (rate and tolerance, the bursts), `move.filter_min_n_burst`, `move.steps`,
    `move.time_of_day` (day, night, dawn and dusk from the sun's times), `move.nsd`, `move.track_summary`
    (distances, straightness, mean squared displacement, sinuosity, temporal autocorrelation of the directions);
  - step selection: `move.distribution` and `move.fit_distribution` (gamma, exponential, log-normal, half-normal,
    von Mises, uniform; the fits with their covariances), `move.random_numbers`, `move.random_steps` (the control
    steps drawn as amt draws them) and `move.extract_covariates` (the maps' values at the steps' ends or starts).
  A step distribution is a new type of the graph's values, written as JSON. The library has a subject for them,
  Animal movement.
- `vector.calc` and `vector.filter` read texts: the features' text attributes compared with texts
  (`uso == "floresta"`, `"a" <= nome < "n"`), chosen (`"alto" if pop > 100 else "baixo"`, `coalesce`,
  `uso[pop > 100] = "outro"`), tested with `isnull` (an empty text has no value) and counted (`uso.count()`); a
  text input of the node compared too, and `float(cod)` reads a text attribute that holds numbers. A text where a
  number goes (a sum, `uso > 3`, `uso.mean()`) is refused before running, saying what to write instead.
- The features' measures in their scripts: `geometry.area`, `geometry.perimeter`, `geometry.length`,
  `geometry.x`, `geometry.y` (those of `vector.geometry`), and `feature`, the feature's number.
- `vector.calc` runs scripts of many lines, as `raster.calc` does: each `out name = ...` gives an attribute, new
  or replacing one, and the column is left empty; without `out` the last line goes to the column as before.
  `vector.filter` runs scripts too, ending with the condition.
- The assistant and the MCP server write and explain these calculators (write_calc with `op` vector.calc or
  vector.filter, explain_calc): the script checked and tried on a sample of the features (up to 5000) before the
  model changes, the reply telling each attribute given (its classes or texts and their shares) or how many
  features the filter keeps.
- The website has a Calculators section: the raster calculator and the vector calculator, each with its language
  in full, recipes and pitfalls, and a playground where scripts run as you type over Sibuyan's maps and land-use
  patches, computed in the browser by Terraflux's own map algebra built to WebAssembly (the crate
  `terraflux-calc-web`). Every example on the pages gives what `terraflux run` gives.
- The website's series on ecosystem services has three parts more, each from real runs on Ribeirão Claro:
  nutrient delivery (`services.ndr`), seasonal water yield (`services.seasonal_water_yield`) and crop pollination
  by wild bees (`services.pollination`). New pages and sections for the application: the model's document and its
  reports, Trace, the Changes pane, the application's own assistant, Step through a loop, the Parameters pane and
  Compare in Many runs, shared node libraries (`TERRAFLUX_NODES`) and updates on macOS and Linux; checks, tests
  and reproducible runs (`check.assert`, `terraflux test`, `terraflux reproduce`, the manifest, frozen nodes).

### Changed
- A run's manifest holds the files the nodes read (`reads`: a `raster.read`'s, a `table.read`'s, the threats of
  habitat quality...), the files they wrote (`writes`, compared again by `terraflux reproduce`, but a GeoPackage)
  and the frozen results used (`frozen`), each with its SHA-256, and every path also from the manifest's folder.
- `terraflux test` and `terraflux reproduce` use the results of the frozen nodes, as `terraflux run` does (a
  frozen result changed or gone since the run noted); `--thaw` runs every node instead.

### Fixed
- `terraflux reproduce` of a study whose folder moved (copied to another computer, renamed) stopped with "The
  system cannot find the path specified": the model, the inputs and the files read are found from the manifest's
  folder first, and a file found at neither place is named.
- `terraflux run model.toml` (a model named without its folder) stopped with "cannot make an empty path
  absolute": a model's folder is now the current one when its path names none, in every command.
- A setting's default given as a list (`(default [0.05, 0.95])`) was cut at its first comma in the Node pane's
  hint and the document ("default [0.05").

## [1.23.0] - 2026-10-28

### Added
- The document says what a step does in a sentence written for its operator, with its settings and where its
  inputs come from ("A Gaussian random field of 100 × 120 cells: autocorrelation range 10 cells, variance 5,
  nugget 0.2; seed 7."; "Habitat split into habitat and matrix, a share 0.4 of it habitat."), for the operators
  models use most (reading and writing, the neutral landscapes, Markov, regressions, weights of evidence,
  allocation, metrics, MSPA, validation, SDM, carbon, prioritization); in the HTML and PDF reports too.
- The document edits a setting's list or table in place, item by item (add, change, take away), as the Node pane
  does.
- The document's pictures of maps in the viewer's palettes: the one chosen by hand for the output, else by what
  made it (MSPA's classes in GuidosToolbox's colours, elevations, ranks...), the classes in the model's legend's
  colours.
- The Changes pane compares the model with any commit of its file ("Compare with a commit (git)": the last 50,
  with their dates and messages), not only the last one.
- A node's tables before and now side by side in the Changes pane, the cells that changed (by row and column
  name) highlighted and the columns only one of them has in the colour of what was added or removed.
- Inside a group, the node selected shows its map in the viewer, named by its place in the group
  (`preparation/temperature`), with its own outputs to choose from and Trace from its cells.
- Compare in Many runs (Parameters): the scenarios checked run at once as a batch of Many runs (on this computer
  and those helping it, or a cluster), their results shown side by side in the pane when it ends.
- Scenarios compared on a map of classes differ by their changes: the class each cell changed to (nothing where
  it stayed), how many cells changed class and the most common changes ("2 → 3: 8517"), instead of the values
  subtracted.
- Trace: a card's name selects its node and shows its map in the viewer with the traced cell outlined; a patch
  operator's cell (labels, sieves, MSPA, patch metrics) is explained by its whole patch (its class, the cells joined
  to it by 4 or 8 neighbours as the operator joins them, the rows and columns it spans); an allocation (CLUE-S,
  ordered, Dinamica's Expander and Patcher, alone or in a loop traced inside) tells the steps the cell changed
  class at and each step's demand (the cells of each class and how much they changed) or the rate of its
  transition.
- The nodes the assistant reads or changes glow on the canvas while it works on them (a halo that fades).
- The assistant's model evaluated on a fixed set of tasks (Evaluate this model, in the server's settings): 8
  tasks of drawing, changing, fixing, explaining and running models, each in a model of its own and checked on what
  it made (a threshold cell by cell, a loop's last number...); the last score of each model kept and shown with
  its server, so models are compared on the same tasks.
- Calculator: a rectangle dragged on a map of the preview computes that part alone, every cell of it when they
  are few enough (Whole map, or a double-click, goes back to all of it).
- Calculator: a stack the script makes from another, its cells changed one by one (`s2 = s * 2`), has its layers
  completed after a dot (`s2.`).
- The script of a calculator in the Node pane completes names as it is typed, as the Calculator does, its inputs
  known by their values in the last run.
- Shared libraries of nodes (a group's folder on a shared drive, a repository cloned), added in the settings: their
  nodes in the operators' pane beside yours, each library in a section of its own, and the library to save a node
  in chosen in the dialog.
- A node of your libraries in the quick search (`/`) and in the assistant's and the MCP server's tools:
  `list_operators` lists them, `describe_operator` tells one (`my:name`), `add_node` adds it with its settings.

### Changed
- The values a node of your library takes (numbers, whole numbers, yes or no, texts), unless another node feeds
  them, are its settings: in the Node pane's Settings, its own value shown and given back when the field is
  emptied; on the canvas folded away as an operator's optional inputs.
- A group runs again only the nodes of its body that changed and what depends on them (and the same for the groups
  inside it), not its whole body: the cache keeps the outputs of the nodes of the groups by place.

## [1.22.0] - 2026-10-27

### Added
- The checks' badges: each `check.assert` shows how it fared in the last run (passed in green, failed and only
  warned in amber, failed and stopped the run in red, not run or turned off in grey) on its node, where the tooltip
  says what failed, and on its step of the document; a container shows the worst of the checks it holds, inside
  loops and groups too. The model's health is a line in the status bar and at the top of the document ("3 checks:
  2 passed, 1 warned"), each check listed on hover.
- A whole run in the application of a model saved writes the model's outputs and the manifest of the run
  (`output/manifest.json` next to the model), as `terraflux run` does: `terraflux reproduce` runs it again. The
  status bar says so after the time of the run ("manifest", which opens the folder).
- Stop stops the long operators between their parts, and they tell how far they are: MSPA by tiles
  (`landscape.mspa_tiled`, between two tiles), the circuits from one to all and all to one (`circuit.one_to_all`
  and the INI files' runs, between two batches of points), the folds of `sdm.fit`, the replacement and rank
  importance of `prioritize.evaluate` (between two lots of solves, between two budgets) and the metrics by blocks
  (`metrics.*` with `block_rows` or rasters on disk, between two blocks; `metrics.window`, between two strips).
- A node's result frozen (Freeze in the Node pane, `frozen = true` in the model file): its outputs written next to
  the model (`<model>.frozen/<node>/`, a file each: maps in GeoTIFF, those not of whole numbers in float64, tables
  in CSV, vectors in GeoPackage, numbers, texts and fitted models in JSON; the same values read back) and used
  instead of running it, in later sessions and by `terraflux run` too, whatever it reads; what only it reads does
  not run either. A ❄ marks it on the canvas. Unchecked, the files go and it runs again. A node frozen before it
  ran is frozen when it runs; a random generator, agents or a series not of maps cannot be.
- A loop step by step, as at a breakpoint: with **Step** on in the status bar, the run pauses after each step of a
  loop (its maps shown as they come), the status bar saying where ("paused at years: step 2 of 10") with **Next**
  (the next step) and **Continue** (on to the end); Stop stops it there.

## [1.21.0] - 2026-10-27

### Added
- Three more of InVEST's models (natcap.invest 3.20) as landschaft 1.54 reimplements them, the same float32 and
  float64 bits (landschaft's references, `tests/data/reference/services_invest.npz`):
  `services.ndr` (the nutrient delivery ratio: nitrogen and phosphorus loads, surface and subsurface, their
  effective retention down to the streams, the delivery ratio, the exports, the index of connectivity and the
  totals by watershed; MFD or D8; the load types for every class or by class code),
  `services.pollination` (crop pollination by wild bees: each species' supply and abundance by season, the total
  abundance, and with farm polygons the pollinators on the farms, the total and wild yields and the means by farm;
  the guilds a list or InVEST's guild table, a file) and `services.seasonal_water_yield` (the monthly and annual
  quickflow of the curve numbers and rain events, constant or by climate zone, the local recharge, the available
  recharge, the evapotranspiration, the baseflow and its sum from upslope, the VRI, the tables by watershed and
  by watershed and month; or the routing of a recharge map alone). scipy's `exp1` is ported for the quickflow.
  The subsurface distance of the NDR is in metres (InVEST measures it in cells against a length in metres).
- The assistant writes map algebra. The tool `write_calc` (in the application and in `terraflux mcp`) writes a
  `raster.calc`'s script, a new node or the one named: checked before anything changes (its language, every name
  an input of the node or given a value before, the layers a stack has) and tried on a sample of its inputs' cells
  (the last run's values, or the rasters' files, sampled; `terraflux mcp` runs the model up to what it reads): a
  script that fails is refused with the reason and the model left as it was, and the reply says what each output
  gives (a map's classes and their shares, or its least, mean and largest values). `explain_calc` takes a script
  apart: each input and what it is (a map, a stack and the layers it is read as, a number), the layers read, and
  what each name holds at the end on the sample. Both are among the few tools of a small model. In the Calculator
  pane, a field asks the assistant to write the script (**Write**) and **Explain** asks it to explain it; the change
  comes to the Changes pane like the assistant's others.
- Terraflux updates itself on macOS and Linux too: the application (`Terraflux.app`, in a folder the user may
  write) is replaced by the release's zip, unpacked beside it; the folder of the `.tar.gz` by the release's; the
  `.deb`, `.rpm` and Arch packages are installed by the system's manager, the administrator's password asked
  (`pkexec`). Each closes, waits, installs and opens again with the model, as on Windows. Elsewhere (Windows' zip, a
  build, a folder the user may not write) the release's page is opened, as before.
- An update that did not install is told when Terraflux opens again ("Terraflux X was not installed"), with its log
  (`terraflux-update.log` in the temporary folder: the installer's on Windows, the script's elsewhere).

### Fixed
- The update on Windows closed Terraflux and installed nothing: PowerShell, started detached from any console to
  install once the application had closed, ends before it runs anything. It now has a console of its own, not
  shown (and leaves the application's job when the job lets it). A Terraflux 1.20.0 or older still has the old
  updater: install 1.21.0 by hand once.
- The settings are kept on macOS (`~/Library/Application Support/Terraflux`) and on Linux without `XDG_CONFIG_HOME`
  (`~/.config/Terraflux`); they were not kept there at all.

## [1.20.0] - 2026-10-27

### Added
- `sdm.bias_surface`: the sampling effort of a target group (the records of every species sampled the same way) as
  a Gaussian kernel density on the grid of the predictors, the bias of `sdm.background` (a target-group background,
  Maxent's bias file). It is landschaft's `bias_surface`, `MASS::kde2d`'s density at the cells' centres, with R's
  `bw.nrd` in each direction by default (`kde2d(h = H)` is `bandwidth = H / 4`), a number or `[x, y]`, `isotropic`,
  bandwidths raised or lowered to `limits`, the records' `weights` from a column, a linear `rescale` (e.g.
  `[0.05, 0.95]`, so that every cell keeps a chance) and the cells without predictors masked. The bandwidths are
  landschaft's to the bit, the density up to rounding.
- `sdm.fit` gives the out-of-fold predictions (`predictions`): the records with a column `prediction`, each one's
  suitability from the model fitted without its fold (none in fold 0), the same as landschaft's
  `SDMResult.predictions`. They score the model on what it did not see and feed models stacked on it (an ensemble,
  a calibration, the distance to an expert range).

## [1.19.0] - 2026-10-26

### Added
- Terraflux updates itself. It asks GitHub for the latest release when it starts (at most once a day; off in the
  settings) and shows **Update to X.Y.Z** in orange in the toolbar. On Windows, the Terraflux of the installer
  saves the model, downloads the new installer, checks it against the release's `SHA256SUMS.txt`, installs it once
  the application has closed (its progress only, no question, no administrator) and opens again with the model;
  the next start says "updated to X.Y.Z". Elsewhere (the zip, macOS, Linux) the release's page opens.
- Terraflux's version is shown: beside its name in the toolbar, in the window's title and in the settings, with
  the updates (**Check now**, the check at the start turned on or off).
- The Windows installer has pages: its progress, then the last page, which says Terraflux is installed and opens
  it (ticked).

### Changed
- Inside a container (a group, a loop, a switch, a node of your own), the canvas is framed in orange and its
  corner says which one it shows and its kind, and the Model pane's bar has **‹ Back** (also Esc on the canvas,
  nothing selected) and the trail as buttons (*Whole model › ...*), each back to its level. The Node pane's
  **Edit the body** is **Open inside ›**.
- The completions of the calculator show the one chosen by the keyboard (up and down, enter or tab puts it in):
  it was not marked, so the keys seemed not to work. Their keys are told under the list.

### Fixed
- Installing or removing Terraflux on Windows stalled the whole computer for about a minute: Windows made a restore
  point first (a snapshot of the disk), even for the installer of 1.18, which does not need the administrator.
  The installer no longer asks for one.

## [1.18.0] - 2026-10-26

### Changed
- The Windows installer installs Terraflux for the user, without the administrator's permission: Windows no longer
  asks to allow it when installing or uninstalling (nor makes a restore point first, which made the installer seem
  stuck). It goes to `%LOCALAPPDATA%\Programs\Terraflux`, with a shortcut in the user's Start menu and the folder
  on the user's PATH. A Terraflux installed for all the users by an earlier installer is to be removed first (the
  installer says so). The installer no longer adds a rule to Windows' firewall: the firewall asks the first time
  Terraflux is opened to the computers of the local network.

## [1.17.0] - 2026-10-26

### Changed
- Map algebra has one syntax, Python's, in every expression (`raster.calc`, `value.calc`, `check.assert`,
  `table.filter`, `table.calc`, `vector.filter`, `vector.calc`, `uncertainty.delta`, `uncertainty.monte_carlo`,
  `agents.update`'s rules): `a if c else b`, `**`, `and or not`, `None`, `log`. The forms of other languages
  (`if c then a else b`, `c ? a : b`, `^`, `&& || !`, `null`, `ln`) are refused with the one to write ("write **
  for a power (not ^)"), told at the token in the calculator and in the Node pane. A model written before is
  rewritten when it is read, only the statements that had an old form (their comments and brackets kept), with a
  note of each (`terraflux_graph::old_algebra`); saving it keeps the new form. The nodes of your library too.
- `map.calc` is `raster.calc`: a model with `map.calc` reads it as `raster.calc` (as `stack.layer` became
  `stack.extract_layer`). Its node's default id is `raster_calc`.
- The Node pane of a `raster.calc` edits its script in an editor of code under the setting's name, the pane's
  width (the terminal's font, its words in colour, its first error underlined and told under it), with an orange
  Expand button beside the name: the Calculator, the full editor (the names completed, the outputs shown as they
  are typed, the lines numbered, the console), in a pane of its own. It replaces Open in the calculator.
- The console of the calculator writes a missing value as `None`, and a map's missing cells as "missing".
- `terraflux mcp`'s `open_in_app` opens the model in the Terraflux already open, in place of its model, and
  brings its window to the front, rather than starting a new window each time: each application listens on a port
  of this computer only, told with a secret in a file of the application's data (`instances`). A new window opens
  only when none is open, or when the model open has edits not saved or a wizard is open (the answer says which).

### Fixed
- Runs of many tries (seeds, scenarios, a batch) failed with "the bundle ... could not be made" once Windows'
  Storage Sense had cleaned the temporary folder: it removes the empty mark of a bundle not used for a while,
  and the bundle's folder, left without it, was never made again. A folder without its mark is now made again.

## [1.16.0] - 2026-10-26

### Added
- Map algebra writes scripts, in a language close to Python's (an expression of before is a script of one line):
  one statement a line (or separated by `;`, `#` a comment), `name = value`, `name[where] = value` (the cells
  where the condition holds, the others keeping theirs), `s.layer = value` (a stack's layer replaced or added),
  and `out name = value` for each output of the node, a map, a stack or a number (the ports are the script's
  `out`s; without any, the last value is `map`). An operation on a stack is made on each of its layers;
  `mean sd median sum count min max (s)` and `percentile(s, p)` are the value of each cell over a stack's
  layers, and `x.mean()`, `x.sd()`, `x.median()`, `x.sum()`, `x.count()`, `x.min()`, `x.max()` and
  `x.percentile(p)` one number from all the cells (missing values left out in both). Also `a if c else b`,
  chained comparisons (`5 <= x < 8`), `**`, `True`, `False`, `None` and `log`. A stack's layers are read by
  name with every name made one (`1997` is `s._1997`, a band without a name `s.b1`), and a band name begun with
  a digit is told how to write it.
- The Calculator pane (View, or "Open in the calculator" in a map algebra node's pane) writes the script of the
  `map.calc` node selected, under the model: its words in colour, its first error underlined where it is and told
  with its line (a name that is no input, with the one meant; a layer a stack does not have), the names completed
  as they are typed or with ctrl+space (the inputs, the names given in the script, the functions and the methods
  with their help, and after `s.` a stack's layers with their names in the file, known from the last run or from
  the raster's header before anything runs), and each output shown beside it as the script is typed, computed on
  a sample of the cells. A console under it computes a line in the script's scope on every cell (`img.mean()`
  gives the number at once; a map, its numbers and its image) without changing the node. ctrl+enter gives the
  script to the node and runs the model up to it.
- Nodes of your own: a group or a calculator saved as a node of your library ("Save as my node..." in its pane,
  or the commands), used in any model as one operator from the Operators pane (My nodes), the commands or ctrl+K.
  The dialog shows what it takes and gives, found from what it is given and what its scripts read: each input's
  type, the layers a stack must have (`img.nir` read: `nir`), its value when none is given; each output's type;
  all of them, the name and what it does changed there before it is saved. Each save is a new version, a file of
  its own (`ndvi@3.toml`, in `nodes` of the application's data or `TERRAFLUX_NODES`). In a model the node is a
  group with `uses` (the name, the version, its ports) and a copy of that version's body: the model runs the
  version it was made with on any computer, with or without the library, and a newer version changes it only when
  it is updated to it (its pane offers it, keeping what it is given). Its pane also tells when it was changed
  since its version (Save as a new version) and makes it a plain group again. The check takes its ports' types
  and needs (an input not given takes its value of its own, in the run too), and a stack given it without a layer
  it reads is told when connected and stops the run with the layers missing and those it has.
- The tutorial The raster calculator (Building models): one script over Sibuyan's land use of 1997 and 2011 and
  its drivers gives the forest's years, its loss, a habitat index of the forest of 1997, its best quarter and its
  mean, written in the Calculator pane with its completions, previews and console, and saved as a node of your own
  with its versions; the language in full and its pitfalls. Its maps and numbers come from a run of Terraflux and
  its pictures of the application from `cargo test --release -p terraflux-app --test readme_images calculator --
  --ignored`.

### Changed
- Map algebra is faster, with the same results to the bit: an expression is compiled (the numbers folded, a value
  written twice computed once, the maps read in place) and run over blocks of cells in parallel, and `nbmin` and
  `nbmax` take the same time whatever the radius (van Herk and Gil-Werman, along the rows and then the columns).
  On 9 million cells with four threads, local expressions run 5 to 20 times faster, `nbmax(a, 5)` 18 times and
  `nbmin(b, 25)` 400 times (45 s to 0.11 s). `map.calc`, `value.calc`, `check.assert`, `uncertainty.delta`, the
  agents' rules and the tables' expressions share the evaluator.

## [1.15.0] - 2026-10-21

### Added
- Reproducing a run: every `terraflux run` writes `manifest.json` with its outputs (the version of Terraflux, the
  model and every input file with its SHA-256, the values, scenario and seeds, every output's SHA-256), and
  `terraflux reproduce manifest.json` runs it again and tells whether every output is the same, byte for byte.
- Testing a model: `[[tests]]` in the model name a value and the file of what it should be, within a tolerance;
  `terraflux test` runs models (or folders of them) and checks their tests and `check.assert` nodes, and
  `--update` writes the expected files from the run.

## [1.14.0] - 2026-10-21

### Added
- Debugging a model: a node turned off without being taken away (`bypass`; B on the canvas): it does not run, each
  of its outputs the first of its inputs of the same type, its box faint; how long every node took and about how
  much memory what it gave holds (a loop's body summed over its steps), in the node's pane and in
  `terraflux run --profile`; and `check.assert`, a check that stops the run when its expression is false (in every
  cell with a value, for maps), with its message and what failed, or only warns.

## [1.13.0] - 2026-10-21

### Added
- `control.switch`: one branch of a model run, chosen by its case, a group per branch in its body (the one named as
  the case, `case_<n>` for a number, `otherwise` for the rest), its outputs the branch's, every branch giving the
  same ones; with the case an input, a parameter chooses the method and each choice is a scenario. `value.calc`: a
  number from an expression in map algebra's syntax over the node's inputs (a table's value as `table.column`), and a
  text with them in it (`out/{species}_{step}.tif`), for the settings that read values. The template Choose the
  method: a switch and its scenarios.

## [1.12.0] - 2026-10-21

### Added
- `control.each`, a loop over a collection: its body runs once for each layer of a stack (a folder of rasters, the
  bands of a file), item of a series, row of a table, feature of vectors or value of a list, with `@item`, `@index`
  and `@name` inside (its other inputs as a group's); what it collects is gathered, maps as a stack of the elements'
  names, tables as one table with a column `item` first, the rest as a series. The template For each year: one
  analysis per map (control.each).

### Changed
- An input of a value that a run is not given takes the model's own value in the engine too (as the command line
  and the application already did).

## [1.11.0] - 2026-10-21

### Added
- Settings that read values: a setting holds a value of its own or reads one when the model runs, `"$input"` (an
  input of a value), `"@step"` or `"@carried"` (a loop's step or carried value, a group's input) or
  `{ from = "node.port" }` (another node's number, true or false, or text). A threshold changes with the loop's
  step, one input feeds several settings, and a submodel shows its settings as inputs (a setting reading
  `"$threshold"` is given by the node that uses it). The node read runs first and counts in the cache; a map or a
  table read is refused when the model is checked; renaming, copying, grouping and removing nodes follow the
  references. In the application, a right click on a setting's name reads a value of the model, and the row shows
  what it reads.

## [1.10.0] - 2026-10-21

### Added
- Hydrography90m and Environment90m, the river network of the world at 90 m and its environment per
  sub-catchment: `data.download` with `source = "hydrography90m"` reads only the window of the grid from the tiles of
  any of its variables (sub-catchments, segments, basins, outlets, flow directions and accumulation, stream orders,
  slopes, distances, flow indices); `data.environment90m` gives the climate (CHELSA, observed and projected), land
  cover (ESA CCI by year), soil (SoilGrids), elevation (MERIT DEM), aridity (CGIAR) and hydrography of every
  sub-catchment of a map (their min, max, range, mean or sd), the tables of the tiles downloaded once into the cache,
  and a column on the map.

## [1.9.0] - 2026-10-21

### Added
- The connectivity of rivers, as R's riverconn and hydrographr: `hydro.dci` splits the network into segments at the
  barriers (dams, weirs, culverts: points with their passability up and down) and gives the dendritic connectivity
  index of each network (Cote et al. 2009; potamodromous and diadromous, symmetric or asymmetric), the share of its
  network a fish can reach from every stretch, and for every barrier the DCI without it and its gain (riverconn's
  d_index), ranked: which dams to remove first; the same values as riverconn 0.3. `hydro.network_distance` gives the
  distance along the river between every two points, `hydro.along_network` the stretches up and down from each point
  within a distance, and `hydro.centrality` the degree, Shreve magnitude, river above, betweenness and closeness of
  every link. The template Rivers: dams and their connectivity (DCI).

## [1.8.0] - 2026-10-21

### Added
- Rivers as a network, as hydrographr does on the Hydrography90m: `hydro.snap` moves points (gauges, dams,
  records of a species) onto the streams, to the nearest stream cell or the nearest of a river with a flow
  accumulation of at least a minimum; `hydro.upstream` summarises the land upstream of every link of
  `hydro.stream_links`, or of every point, its cells and area, the count, sum, mean, min, max and standard deviation
  of maps and the share of each class of a map (the forest upstream of each stretch), with a column painted on the
  links; `hydro.upstream_catchment` gives the catchment of every point as a polygon with the point's attributes, and
  the map of the catchments nested between one point and the next. In Rust, over the D8 directions of `hydro`. The
  template Rivers: the land upstream and the gauges' catchments.
- Packages for Fedora (`.rpm`, also openSUSE) and Arch Linux (`.pkg.tar.zst`), with the `.deb`: the same folder in
  `/opt/terraflux`, the command in `/usr/bin`, an entry in the menu (`tools/make-linux-package.py`, with
  [nfpm](https://nfpm.goreleaser.com)). At every release each Linux package is installed in a container of Ubuntu
  24.04, Debian 13, Fedora or Arch and runs a model with its GDAL before it is published; the website's downloads
  list them. The release workflow run without a tag builds and tries the packages without publishing them.

### Changed
- The Assistant pane looks like a terminal: the conversation in a fixed font on one ground, each line marked in its
  gutter (❯ the question, ● the answer and each tool called, coloured by how it went, └ the first line of its
  result, a click opening it whole, ∴ the thinking); a prompt where enter sends, up and down bring back what was
  typed and esc stops the answer; commands after a slash: /new, /stop, /review, /accept, /reject, /explain, /setup
  and /help.

## [1.7.2] - 2026-10-20

### Added
- The packages of every release are built by GitHub Actions at its tag `vX.Y.Z` (`.github/workflows/release.yml`):
  the MSI and the zip for Windows, the disk image and the zip for macOS (Apple silicon), the `.deb` and the
  archive for Linux, published as the tag's release with its changes and their checksums (`SHA256SUMS.txt`).
  The website is published on GitHub Pages after each release, its downloads those of the latest one.

### Fixed
- Terraflux builds on macOS again: landschaft's reading of SWAT+ files used `std::from_chars` of reals, which
  Apple's libc++ does not have (`strtof`/`strtod` there, the same values).

## [1.7.1] - 2026-10-19

### Fixed
- Vector layers draw as quickly zoomed in as zoomed out. A tile no longer goes through every vertex of the
  features over it: the runs of their vertices off the tile, or smaller than a pixel, are drawn as one line each.
  A tile deep in a municipality of 100 000 vertices is drawn about 30 times quicker.

### Changed
- On macOS the release build's line tables go into a `.dSYM` (`.cargo/config.toml`), not into object files left
  in `target/` (about 2 GB at each build). `tools/make-macos-app.py` removes the workspace's crates of earlier
  builds before building, which cargo never does.

## [1.7.0] - 2026-10-19

### Added
- A node that takes one map, given many layers (a stack, the bands of a file, a folder of maps, a loop's series),
  runs once per layer: its maps come out as a stack of the same layer names (a series for a loop's), its tables
  as one table with a column `layer` (the year of names such as `forest_2010`), its other values as a series.
  Stacks given together are paired layer by layer. The canvas marks such a node ×N.
- `raster.write` with `{name}` in its path writes a file per layer, named after it
  (`mspa/{name}_mspa.tif`).

### Changed
- A file of several bands connected to a port of one map no longer gives that port its first band only: the
  node runs on every band. `raster.read`'s `band` chooses one. Where a port still takes one map alone
  (`stack.of`, `agents.parcels`, the inputs of groups), the bands left out are told in a warning.
- The rasters the preparation operators make without a `path` are kept in Terraflux's cache (`prepare`), made
  once and used again while their sources and settings are the same, instead of a new copy at every run in the
  temporary folder; the old copies there are removed. The caches of rasters made, of data downloaded and of the
  workers are held under 10 GB each (`TERRAFLUX_CACHE_GB`, **Settings > Cache**, which also empties them).
- The tiled engine's layers on disk (`--disk`) are removed as soon as the run no longer needs them, instead of
  every step's staying after the run.
- Builds take four jobs at a time (`.cargo/config.toml`), and landschaft's C++ is built once per profile
  (`target/<profile>/landschaft`), not again for every change of the crate's hash.

## [1.6.0] - 2026-10-18

### Added
- Statistics of rasters: `raster.stats` (a map, or every layer of a stack or of a series, summarised as a table:
  count, nodata, sum, mean, std, min, percentiles, max..., and a histogram), `stack.reduce` (a statistic of the
  layers at every cell: mean, std, min, max, median, sum, count, majority, a percentile, the layer of the largest
  or smallest), `raster.zonal` (the statistics of a map in the zones of a map of zone ids, as a table and as a map
  of every cell given its zone's), `raster.tabulate` (the cells, area, hectares or share of every class in every
  zone), `raster.correlation` (Pearson's or Spearman's correlation and the covariance between maps, the matrix of a
  stack, the line of each pair), `raster.local_correlation` (the correlation, slope, intercept and r² of two maps
  in a moving window) and `raster.standardize` (z-score, min-max or a percentile stretch, with each layer's centre
  and scale); the library's subject Raster analysis, and the template Raster statistics: summaries, zones and
  correlation.
- Neighbourhoods and the cleaning of maps of classes: `raster.focal` (a statistic in a moving window around every
  cell: mean, median, majority, std, variety, range, a percentile...; a square, a circle, an annulus or a Gaussian
  kernel), `raster.majority` (the majority filter, as ArcGIS's), `raster.sieve` (the patches under a size merged
  into their largest neighbour, as gdal_sieve), `raster.label` (the patches numbered, as r.clump, with a table of
  their cells and area), `raster.expand` and `raster.shrink` (classes grown or shrunk a number of cells); the
  template Cleaning a map of classes: filter, sieve, label and grow.
- Vector tools: `vector.polygonize` (a map of classes as polygons, a patch each with its holes, as
  gdal_polygonize), `vector.filter` and `vector.calc` (features kept or given an attribute by an expression over
  their attributes), `vector.geometry` (area, perimeter, length and centroid; geodesic areas), `vector.overlay`
  (intersection, union and difference of two layers of polygons, as geopandas' overlay), `vector.spatial_join` (the
  attributes of the feature met, or the features met counted and their attributes summed and averaged: the points
  in every polygon), `vector.reproject`, `vector.from_table` (points from a table's x and y) and `vector.points`
  (random or regular points inside polygons); the library's subject Vector analysis, and the template Vector tools:
  polygons, overlay and points.
- Classic spatial analysis: `vector.density` (the kernel density of points, quartic as ArcGIS's or triweight,
  Epanechnikov, triangular, uniform, per square unit, hectare or km², Silverman's radius by default; or the points
  in each cell counted), `raster.contour` (contour lines at an interval or at levels, as gdal_contour),
  `raster.viewshed` (how many observers see each cell, their height and the target's, a reach, the earth's
  curvature), `raster.cell_area` (the true area of every cell, on the WGS84 ellipsoid for longitudes and latitudes)
  and `raster.classify` (quantiles, equal intervals, Jenks's natural breaks, standard deviations or breaks given,
  with a table of the classes); the template Spatial analysis: density, contours, viewshed and classes.
- A cell explained by an assistant through `terraflux mcp` (Claude Code, Claude Desktop...): Explain with an MCP
  assistant in a cell's menu (a right click) keeps its trace for it and copies a request to paste; the server's
  prompt `explain_cell` (a slash command in Claude Code) brings it the trace and the question.

### Fixed
- The menu of a right click on a map's cell went as soon as the pointer moved to it: it stays now until an item is
  chosen (or a click elsewhere).

## [1.5.0] - 2026-10-18

### Added
- Tables worked on, as dplyr and tidyr do (the tables of metrics, transitions, zonal statistics, evaluations):
  `table.filter` (the rows where an expression is true: class == 3 and area > 10), `table.calc` (a column computed
  from the others, the expressions of map algebra), `table.aggregate` (grouped by columns and summarised: sum, mean,
  count, min, max, sd, median), `table.join` (two tables by key columns: left, inner, full), `table.sort` (sorted,
  the first rows kept), `table.select` (columns chosen, dropped, renamed) and `table.pivot` (long to wide, as the
  transitions into a matrix, and back); the library's subject Tables, and the template Tables: filter, summarise,
  join and pivot.

## [1.4.0] - 2026-10-18

### Added
- Why a cell has its value (the Trace pane): a cell of a map clicked, then Trace this cell, and the chain of values
  that made it, back through the graph: a card per node with its value there and how it was made (map algebra's
  expression with the values in place of its names, the window of cells a neighbourhood's operator reads, the
  layers a model weighs, a loop's value step by step, through groups), its inputs a click away, a sentence of it at
  the top. Trace inside the loops runs the model again keeping what their bodies give at the cell, step by step.
- The trace with the assistant: Ask the assistant why sends it to the chat, to be explained in words; the tool
  `trace_cell` (in the application, and in `terraflux mcp` with `inside_loops`) gives an assistant the same chain,
  and the assistants are told to explain results from it.
- A right click on a cell of a map (the viewer, a map's tab): Trace this cell, or Ask the assistant why. The last
  trace made is kept for an assistant through `terraflux mcp` too (its tool `last_trace`).
- Why a node failed (the Problems pane): the last run's failure explained (the node and its message, what it was
  given side by side: where each input comes from, what it is, its grid's size, cells and coordinate system, what
  differs from the first marked; what likely does not fit and what puts it right, raster.match or
  raster.reproject), then the graph's checks, the data's findings and the run's warnings, each a click away from
  its node. Opened from the status bar ("the run failed: why?", the problems counted) and from the node's pane (Why
  it failed); Ask the assistant sends a failure, explained, to the assistant. The assistant is told the same when a
  run fails, and `run_model` (in the application and in `terraflux mcp`) answers a failure with it.

### Fixed
- A pane brought to the front by another while the panes were drawn did not come (the assistant's Changes, among
  others).
- The assistant's text ran out of its pane's side when something in it was wider (a tool's call): the panes that
  scroll by themselves (the assistant, the document, a trace, the parameters, the changes, the problems) wrap at
  their sides; a tool's call is cut to the pane's width, and "fix problems" is beside Send.

## [1.3.0] - 2026-10-18

### Added
- The assistant is told of the problems that come up, for it to fix them: in a conversation going on, a run that
  fails (whoever started it) and the model's errors when an answer of its ends are sent to it on their own, each
  once and a few times one after another at most until the user writes ("fix problems" in its bar turns it off).
- The collections of favourites are named (a right click on one), not only by their colours.

### Changed
- The panes as they first are: the node's pane (and Prepare) at the left of the model, nothing at the right; the
  files in a drawer of their own at the window's left edge, as the operators (a tab each on the strip); the
  assistant at the right when it opens, not with the node's pane. A layout kept of another version is let go.
- The operators' pane begins with the favourites: the buttons to open rasters, vectors and folders and to save a
  raster are gone (File, Open a raster or vectors, ctrl+O, and the Files drawer open data), and GDAL's version is
  in the Settings.
- Tab goes between the model and the last map seen (a map's tab or the viewer), and from any other pane to the
  model.
- `stack.layer` is `stack.extract_layer` (a model that names the old one reads as before).
- The run's progress: the whole run (its nodes done, and the share of the one running: a loop's steps, what its
  operator tells), begun anew at every run and gone once it ends; its words beside the bar, readable whatever the
  bar shows under them (the bars of a node's progress and of Many runs too).

### Fixed
- A GeoTIFF of a projection of its own (no EPSG code: a country's Albers, ArcGIS's projections) had no coordinate
  system for the basemap and the warping: its WKT now comes from GDAL. And the basemap's "no CRS" stayed after a map
  of one was shown again.
- The run's progress bar showed the last loop of an earlier run, and did not move in a model without loops.

## [1.2.0] - 2026-10-18

### Added
- A model read in stages. ctrl+G puts the nodes selected into a group (what read them from outside reads the
  group's outputs, named after them; refused when it would make a cycle) and ctrl+shift+G takes a group apart;
  **Group the model by stage** (ctrl+K) puts the top level in a group per stage (data, preparation, model,
  validation, results), each with a note, the plots, files written and random generators left outside, and lays it
  out again. The canvas shows the groups as blocks linked by what their bodies read (a thinner line), their notes
  under them; zoomed out, a model without groups shows its stages framed and named. The Document pane and the
  reports read a section per group (its note under its title), what a group gives named by the node of its body
  that gives it, and the plots of a group's output as the figures of that node.
- The tools of `terraflux mcp` and of the assistant: `group_nodes`, `ungroup` and `group_by_stage`; the assistants
  are told to put a model built in a group per stage.
- Parameters as a form (the Parameters pane): the values a user is to change, kept in the model (`parameters`,
  optional in `terraflux/1`: an input of a value or a node's setting or literal input, with its label, unit,
  range, step, choices and help; a setting cannot read an input, so a parameter names it directly). A slider for a
  number in a range, a list for a few choices, a box for yes or no, a field for the others; each change an edit
  (ctrl+Z) and the model run again. A setting of the model added from a list, its label and help taken from its
  operator's description.
- Scenarios (`scenarios` in the model: named sets of the parameters' values, with a note): the values now saved
  under a name, a scenario applied in a click, sent to Many runs as its table of scenarios, the parameters of a
  range sent to its calibration, and scenarios compared side by side: each run to an output chosen, its maps (or
  tables, or numbers) in a column per scenario with their differences from the first. `terraflux run --scenario
  name`; `set_parameter`, `remove_parameter`, `set_scenario`, `remove_scenario` and `run_model`'s `scenario` for
  the assistants. The Document pane and the reports list the parameters and the scenarios; `check` warns of a
  parameter or a scenario that names nothing the model has.

### Changed
- A group keeps what the nodes of its body gave with its outputs (as `node.port`, which no reference names): the
  Document pane and the reports show its steps' maps after a run.
- The document's order and stages are found in time proportional to the nodes and connections (the canvas uses
  them).

## [1.1.0] - 2026-10-18

### Added
- The model read as a document (the Document pane, a tab beside the model's): the same graph as the methods
  section of a paper, for whoever reads, checks and decides rather than builds. The top-level nodes in five stages
  (data, preparation, model, validation, results; the analysis when there is no model), each stage in the order of
  the dependencies with the ties broken by where the nodes are on the canvas; a step each, with its note, why,
  inputs, settings in words (the operator's own description, and the default when the value differs), map
  algebra's expression with its variables linked to their nodes, references, assumptions and alternatives; the
  data of one kind in a row, the plumbing (stacks, generators) in a line; a loop's or group's body within it, with
  what it carries, collects and gives; the plots as numbered figures of the steps they show; the maps, series and
  tables of the last run beside the steps. A step's name selects its node; a setting of a single value is edited in
  place (ctrl+Z undoes it) and the model runs again. The design, drawn first by hand on three models, is in
  `docs/DOCUMENT_VIEW.md`.
- Reports: the document written as one HTML page (its images in it, read without Terraflux) or as a PDF, from the
  pane, File, Export report, or `terraflux report model.toml [-o report.pdf] [--run]` (`--run`: the maps and tables
  of the steps in it).

## [1.0.0] - 2026-10-18

### Added
- The model as the record of its choices: optional fields of a node beside its `note` (`why`, `refs`,
  `assumptions`, `alternatives`) and of the model (`request`, `summary`, `author`), in the format `terraflux/1`
  (older models stay valid; none changes what a node gives, nor runs it again). The tools of `terraflux mcp` and
  of the assistant take them (`add_node`, `edit_node`, `new_model`, the new `set_model_info`); `add_node` asks for
  a note and a why when they are missing, and `check_model` advises of the nodes made without them (those copied
  from a template aside). The assistant signs what it changes (its model and server among the authors, the first
  question of the conversation as the request). The Node pane shows and edits the record, and the model's own when
  no node is selected; the canvas shows a node's why with its note.
- The review of a model's changes (the Changes pane), as a pull request's: what the assistant changes, or another
  program writes to the model's file (an assistant through `terraflux mcp`), listed as nodes added, removed and
  changed, with each connection, setting and record before and after, and the model's inputs, outputs and record;
  the maps a changed node gives before and now side by side; the changed nodes outlined on the canvas. Each change
  accepted or rejected (an edit that ctrl+Z undoes), or all at once; the assistant's pane says how many are to
  review. File, Compare lists the differences with the saved file, another file or the last commit (git).

## [0.99.0] - 2026-10-18

### Added
- The assistant (View, Assistant): a chat beside the graph with a model that explains, changes and runs the
  model open, with the tools of `terraflux mcp` called in the application. Each change is an edit of its own
  (ctrl+Z undoes it) with the node it touched selected; a run goes in the application, its maps shown there, and
  the assistant is told what the outputs gave. "Explain" asks about the node selected. The model is told the
  model open, its operators' documentation and the node selected.
- The assistant talks to any server of OpenAI's chat completions API with tool calling, streamed: a university's
  (the HU Berlin's KI-Service, from its network or VPN; the GWDG's Chat AI), one on this computer (Ollama,
  LM Studio) or a commercial provider (OpenAI, Anthropic). A server's address, model, key (kept in the system's
  store of credentials, never in the settings or a model), thinking (Qwen 3 on vLLM) and a small set of tools for
  small models; "Test the connection" checks that the server answers, has the model and lets it call tools.

### Changed
- The tools of `terraflux mcp` live in the application's library (`terraflux_app::tools`), shared by the server
  and the assistant; the server's tools and answers are as they were.
- The viewer opens in a tab behind the model's graph when the first data comes (a template, a file read): the
  graph stays in front. The assistant is a tab of the inspector at the right (the layout kept is reset once).

## [0.98.0] - 2026-10-18

### Added
- `conn.metapopulation`, a metapopulation in a network of habitat patches over time, Hanski's incidence function
  model as R's MetaLandSim 2.0 simulates it (spom), the same run for a seed (R's runif after set.seed): every step
  each patch's connectivity to the occupied ones (an exponential or power kernel of the distance times their
  areas, or times the patch's own), an empty patch colonised (logistic, exponential or power in the connectivity)
  and an occupied one lost (by its area, or with the rescue effect). The patches occupied at every step and each
  patch's incidence. The patches come as a table (x, y, area, occupied) or as points with those attributes.
- The template "Metapopulation (incidence function model)": a century in 60 drawn patches.

## [0.97.0] - 2026-10-18

### Added
- `sdm.migrate`, a species' dispersal through the habitat suitability of successive periods as MigClim 1.6
  simulates it, the same run for a seed (its random numbers are the C runtime's rand(), ported): colonisation from
  occupied cells within a dispersal kernel's reach, weighted by the source's propagule production while it matures
  and by the sink's suitability; weak or strong barriers; long-distance dispersal; cells that turn unsuitable kept
  until the period ends. The final state with MigClim's codes, the cells occupied, the state after every step and
  the statistics of every step.
- The template "Species migration (MigClim)": three periods of a moving climate, a river as a barrier.

## [0.96.0] - 2026-10-18

### Added
- Interpolation of points onto a map as R's gstat 2.1 does it: `stats.variogram`, the sample variogram of points
  (gstat's classes of distance up to a cutoff) and a nugget with a spherical, exponential or Gaussian structure
  fitted to it (fit.variogram's iterated weighted least squares, its Gauss-Newton steps, weights and stopping rule:
  the converged fits to the bit); `stats.kriging`, ordinary kriging with every point or the nearest, the prediction
  and its variance, under the model fitted or one given; `stats.idw`, inverse distance weighting. The points are
  features with their values in an attribute; the result takes the cells of a map.
- The template "Interpolation: IDW and kriging": points sampled from a drawn soil property, its variogram, kriging,
  inverse distance weighting and their errors.

## [0.95.0] - 2026-10-18

### Added
- Spatial statistics, a new subject of the library: `stats.trend`, the monotonic trend of every cell along a series
  of maps (the Mann-Kendall test with its variance corrected for ties, and Sen's slope with its intercept, as
  pymannkendall's original_test: the same numbers); `stats.moran`, the global Moran's I of a map with its inference
  under normality and randomisation and a pseudo p of permutations; `stats.lisa`, the local Moran's I of every cell,
  its quadrant, z and the clusters of a pseudo p of conditional permutations; `stats.getis_ord`, Getis and Ord's Gi*
  with its z, p and the hot and cold spots at 90, 95 and 99% (PySAL's esda, to rounding; rook or queen contiguity of
  the cells with a value, row-standardised). scipy's normal distribution (Cephes' ndtr, erf and erfc) to the bit.
- The template "Trend and hot spots": five years of a drawn NDVI, its trend, LISA's clusters and Gi*'s hot spots.

## [0.94.0] - 2026-10-18

### Added
- Global sensitivity analysis as SALib 1.6 does it, the same numbers: `sensitivity.design` (Morris's trajectories
  on a grid of levels, or Saltelli's runs of a scrambled Sobol' sequence, scipy.stats.qmc's: Joe and Kuo's direction
  numbers, the linear matrix scrambling and the digital shift) and `sensitivity.analyze` (Morris's μ, μ*, σ and μ*'s
  bootstrap interval; Sobol's first, total and second order indices with their intervals; scipy's normal quantile,
  Cephes' ndtri). `terraflux sensitivity model --param node.setting=from:to --method morris|sobol --output
  node.port` runs the design's runs here or on a coordinator's workers (clusters and clouds as `submit`), each
  measuring the output on the worker (a number, a table's column, a map's mean, sum, min or max), and writes the
  design with the outputs and the indices.

### Fixed
- `ml.partial_dependence` of a classifier of three classes or more is now scikit-learn's to the bit (numpy's mean
  down the columns of the probabilities adds in order), not to rounding.

## [0.93.0] - 2026-10-17

### Added
- Brazil's Forest Code (Lei 12.651/2012 with Lei 12.727/2012): `forest_code.app` (the Áreas de Preservação
  Permanente of art. 4: the margins of rivers from the channel's edge by their width, of natural lakes by their area
  (urban zones' 30 m), springs, veredas, slopes above 45°, hilltops, land above 1,800 m and protected classes such
  as mangroves; in the rural areas consolidated by 22 July 2008, art. 61-A's narrower margins by the property's
  fiscal modules) and `forest_code.reserve` (every property's Reserva Legal: art. 12's share, the native vegetation
  counted outside and, as art. 15 allows, inside the APPs, the deficit or surplus; art. 67's properties of up to 4
  fiscal modules keep the vegetation of 2008). Exact Euclidean distances (Felzenszwalb and Huttenlocher) with the
  nearest feature of every cell. The library's subject Forest Code (Brazil) and the template Forest Code: APP and
  Reserva Legal.

## [0.92.0] - 2026-10-17

### Added
- `data.download`: data downloaded on demand and warped onto a map's grid (or an extent of longitudes and
  latitudes), GDAL reading straight from the servers only the part the grid covers: WorldClim 2.1 (the 1970-2000
  climate, its monthly variables and elevation; the CMIP6 futures of a model, scenario and period), CHELSA 2.1
  (1981-2010 and CMIP6 futures), the Copernicus DEM (GLO-30 or GLO-90, its tiles found and mosaicked), SoilGrids (a
  property at depths) and MapBiomas Brazil (collections 9, 10 and 11, by year), or any raster a URL or path names;
  a file per layer in the cache, downloaded once.

## [0.91.0] - 2026-10-17

### Added
- Verra's VT0007, the unplanned deforestation allocation of a jurisdiction (REDD+), as Clark Labs' UDef-ARP:
  `redd.nrt` (the negligible risk threshold of the distance to the forest's edge), `redd.vulnerability` (the
  benchmark classes of geometric widths from that distance, or the alternative from a map of vulnerability),
  `redd.fit` (the relative frequency of deforestation by class and subdivision, and its density), `redd.allocate`
  (the density of another period adjusted to the expected deforestation, or to a period's observed one, never above
  a cell; a region the fit lacks takes its class's average) and `redd.assess` (the predicted against the actual
  deforestation in the cells of an assessment grid: the mean and median absolute errors, the intersection over
  union, R², the OLS and Theil-Sen lines); the same maps and tables as UDef-ARP's own functions.
- `carbon.emissions`: the carbon emitted and removed by land change over a series of maps (the IPCC's
  stock-difference method), a stock per class or a curve of regrowth (Chapman-Richards or linear) from the date a
  class appears in a cell; by period, by transition, and every cell's net emission.
- The library's subject Carbon and REDD+, and the template REDD+ baseline (VT0007).

## [0.90.0] - 2026-10-17

### Added
- What a fitted model learnt, as scikit-learn's inspection tells it, the same numbers: `ml.importance` (the
  permutation importance of each predictor: the drop of the accuracy or of R² when its values are shuffled among the
  records, numpy's RandomState shuffles; the mean, the standard deviation and every shuffle) and
  `ml.partial_dependence` (the mean prediction along a grid of a predictor's values: its distinct values, or from
  one percentile to another as scipy's mquantiles; the value, or the probability of each class).

## [0.89.0] - 2026-10-17

### Added
- Histogram gradient boosting (`terraflux_ops::ml::boosting`), scikit-learn's HistGradientBoostingClassifier (two
  classes or more) and HistGradientBoostingRegressor ported to the bit, without early stopping: the predictors
  binned by numpy's quantiles (from a subsample of 200,000 records when there are more), the gradients and hessians
  of the half squared error, binomial or multinomial loss as float32, the trees grown best leaf first from
  histograms (Python's heap breaking the ties of gain), the leaves shrunk by the learning rate; the same
  predictions as scikit-learn 1.9. `ml.fit` and `ml.potential` take it with `method = "gradient_boosting"`
  (`learning_rate`, `max_iter`, `max_leaf_nodes`, `l2_regularization`, `max_bins`, a share of the predictors per
  split); its importances are each predictor's share of the gain of its splits.

## [0.88.0] - 2026-10-17

### Added
- Machine learning (`ml.*`), scikit-learn's random forests ported to the bit (now `terraflux_ops::ml::forest`,
  shared with `sdm.fit`): `ml.fit` (RandomForestClassifier of any number of classes, or RandomForestRegressor of
  values, on the rows of a table or the cells of maps, a sample of them drawn as numpy draws it; the predictors'
  importances, the mean decrease of impurity), `ml.predict` (the class or the value and each class's probability, on
  maps or a table) and `ml.potential` (landschaft's Estimator: a forest per class of where it is, or where it
  appeared between two maps: PLUS's land expansion analysis, LEAS, the growth probability `cars.step` takes; with
  each driver's contribution), each the same numbers as scikit-learn 1.9 and landschaft. The trees are grown and the
  cells predicted in parallel (the same bits). A fitted model is a value of its own (`ml_model`), written as JSON.
  The library's subject Machine learning, and the template PLUS (LEAS and CARS).

## [0.87.0] - 2026-10-17

### Fixed
- `services.habitat_quality`'s `threats`, when it names InVEST's threats table, is taken from the model's folder
  (and kept relative to it when the model is saved), as the `path` of the operators that read files; it was read
  from the folder the program ran in (`terraflux_graph::FILE_SETTINGS`).

### Added
- The accuracy and the area of a map from a stratified random sample, the good practices of Olofsson et al. (2014):
  `accuracy.sample_size` (the size that gives the overall accuracy a standard error, from the user's accuracy
  expected of each class, and its share of each stratum: a minimum in every one, the rest in proportion; in
  proportion, equally or by Neyman's allocation), `accuracy.sample` (the points drawn in each stratum without
  replacement as numpy's `default_rng(seed).choice`, to be labelled with their reference class) and
  `accuracy.assess` (the error matrix in shares of the area, the user's, producer's and overall accuracies and each
  class's area estimated from the sample, with standard errors and confidence intervals; R's mapaccuracy, its
  `olofsson` and, for strata of their own, `stehman2014`, to 1e-12). The library's subject Map accuracy and area,
  and the template Accuracy and area of a map.
- The website's tutorials of the 0.86 operators: Terrain hydrology (depressions and flow; streams, watersheds and
  HAND; wetness and connectivity; SWAT+), Ecosystem services with InVEST (the ideas, carbon, habitat quality, water
  yield, sediment), geomorphons and the hilltops of Brazil's Forest Code, and the Dinamica series' Patcher of a
  stock, roads that grow and gravity potentials.
- The website to read offline: `python website/build.py --offline` makes a copy that opens from the disk, without a
  server or a network (links to each page's index.html, the search as a script, KaTeX and the fonts within, the
  images in WebP only), and its zip (43 MB), to share.

## [0.86.0] - 2026-10-17

### Added
- Terrain hydrology (`hydro.*`, landschaft 1.50's `ls.hydro`, bit for bit): depression filling (Priority-Flood,
  with or without the epsilon that drains the filled areas) and least-cost breaching (WhiteboxTools'
  BreachDepressionsLeastCost); flow directions (D8 as ESRI's codes, D-infinity azimuths, pygeoprocessing's
  multiple flow directions packed as the InVEST models route); flow accumulation (D8, D-infinity, FD8, Quinn's,
  pygeoprocessing's MFD; in cells, area or specific contributing area; with weights); streams by a threshold or
  traced as InVEST traces them; the distance to them along the MFD; HAND; stream links with their table
  (downstream link, length, elevations, Strahler order) and outlets snapped to them; watersheds; the wetness,
  stream power and sediment transport indices; the index of connectivity of Borselli et al. (2008).
- Ecosystem services, InVEST's models (natcap.invest 3.20) as landschaft reimplements them, the same float32 bits:
  `services.carbon` (storage, its pools, the sequestration and its net present value, the totals),
  `services.habitat_quality` (degradation by threat and in all, quality, rarity; access from a raster or
  polygons), `services.water_yield` (the Budyko curve, actual and potential evapotranspiration, the tables of
  watersheds and subwatersheds with consumption, realized supply and hydropower) and `services.sdr` (USLE, the
  sediment delivery ratio, export, deposition, avoided erosion and export, by watershed). numpy's own float32
  exp, sin and cos (its AVX2 kernels, not the C library's) are ported for them (`terraflux_ops::npmath`).
- Landforms and potentials of a surface (landschaft 1.50's, bit for bit): `surface.geomorphons` (GRASS's
  r.geomorphon 8.4 with four of its errors corrected: the ten landforms, named and in GRASS's colours in the viewer,
  the ternary and binary patterns, the measures, the share of each form), `surface.hilltop` (the hilltops of
  Brazil's Forest Code, APP de topo de morro of Lei 12.651/2012: every summit with its base, height and slope, the
  protected upper part of the hills that qualify) and `surface.gravity` (Dinamica EGO's
  CalcInteractionPotentialMap, by direct sums or an FFT).
- Dinamica EGO's functors of landschaft 1.50, bit for bit: `dinamica.continuous_patcher` (PatcherContinuousState:
  patches that take a stock, biomass or timber, up to a quota, step by step) and the road constructor of the Amazon
  models: `dinamica.cost_map` (CalcCostMap), `dinamica.destinations` (CalcDestinationMap), `dinamica.pathways`
  (CalcPathwayMap) and `dinamica.roads` (a step of road building, with the distance to the roads as a dynamic
  variable).
- SWAT+ (landschaft 1.50's port of SWAT+ 62.0.1, bit for bit): `swat.run` simulates a project (a TxtInOut folder,
  or a zip of one) and gives the tables SWAT+ prints at a step (day, month, year, average annual): the water,
  nutrient, losses and plant balances of the HRUs and of the basin, the SWAT-DEG channels, the aquifers, and the
  daily weather of each station; a spin-up, MUSLE's C of SWAT+ 62 or of the plants. Its folder is a file setting
  (chosen in the Node pane, kept relative to the model).
- The library's subjects Terrain hydrology and Ecosystem services.

### Changed
- landschaft is at its 1.50.0 (the submodule; its `hydrocore` and `servcore` linked).
- Debug and release builds keep line tables only (backtraces with their lines), the dependencies of debug builds
  no debug information; and the integration tests of `terraflux-ops`, `terraflux-exec` and `terraflux-app` are
  one program per crate (`tests/main.rs`, a module per file; `circuit_ini`, `ui` and `readme_images` programs of
  their own), not one per file each linking landschaft's C++ whole: `target` had grown to 400 GB. A test is now
  run as `cargo test -p terraflux-ops --test ops hydro::`.

## [0.85.0] - 2026-10-17

### Changed
- The operators' pane closed, a strip at the window's left edge says they are there ("OPERATORS ›" along it): the
  pointer over it brings their drawer out, a click opens their pane.
- The Node pane of an operator of many settings shows its essential ones and folds the others as advanced
  ("Advanced (39)"; with the count of those the model gives, "39 · 1 given", open then): the inputs and settings
  most models set, listed for the 34 operators of ten settings or more in `terraflux_ops::essentials` (a test keeps
  every name one of its operator's); the boxes of the canvas show the essential inputs too. A setting is a row: its
  name at the left (what it does on hover), its value at the right, the empty field hinting its default ("default
  0.1"); a dot marks the settings the model gives, and a right-click on the name takes one back to its default.
  The inputs are rows too, what feeds them cut to the pane.

## [0.84.0] - 2026-10-17

### Added
- The palette of commands (ctrl+K, ctrl+shift+P; View > Commands): everything the application does, found by
  typing: its actions (run, save, arrange, undo...), the panes, the models opened last, the templates, the nodes of
  the scope shown (to go to one) and the operators (to add one after the node selected). The arrows choose, enter
  does it.
- The keyboard's shortcuts in a window of their own (F1; View > Keyboard shortcuts), by where they work.
- File > Open recent: the ten models opened or saved last. New shortcuts: ctrl+N (a new model), ctrl+shift+O (open
  a model), ctrl+shift+S (save it as).

### Changed
- The bar at the top in three parts: the menus at the left (File: new, open, recent, save, save as, open data;
  Templates; Operators; View), running the model in the middle (Run, Stop, the model's name with a dot while it
  has edits not saved, how the run goes), the tools at the right (Wizard, Many runs, Computers) and the settings
  in a gear. The window's title names the model, with the dot too.
- The status bar says what the keys and the mouse do in the pane in focus (the canvas's, the maps', else the
  application's) instead of every shortcut at once; "not saved" left it for the title.
- The accent is kept for the main action (Run) and for the node selected on the canvas: what is selected in a
  list or a tab is the accent softened into the panel, "Open a raster..." and "Run to here" are plain buttons,
  and the place in the model is a trail of links (Top level › growth) rather than buttons.
- The Node pane puts first what is edited: the problems found in the node under its name, then the first sentence
  of what its operator does (the rest a click away), its inputs (those it needs and those given; the optional
  others folded, "Optional inputs (9)"), its settings named in words ("Gray threshold"; the name and what it does
  on hover), a container's body, and last the note of why it is there. A setting that is a list of numbers or texts
  (`weights = [2.0, 1.0, 1.0, 1.5]`) or a table of them by name is edited item by item, with a row to add one and
  a button to take one away; `{ }` edits it as text.
- The canvas: a box shows the inputs its node needs and those given, its optional others folded in a row ("+ 10
  optional", the names on hover) that a click unfolds and folds again; a connection let go on a node rather than on
  an input goes to the free input it fits, or to the one chosen in a list when several do. A node with problems (an
  input it needs not given, the data's, the run's error) has a mark, red or amber, that tells them on hover; a
  port's hover tells its type and whether it is needed. Zoomed out, the nodes' names stay readable (never under
  11 points, in the middle of their boxes, cut to them) where they used to vanish under 35%. Undo and Redo are
  named (their arrows were not in the font).
- The panes as they first are: the model in most of the window and the node selected in an inspector at its right
  (the preparation and the files in tabs beside it); the operators in their drawer at the window's left edge, the
  Operators menu, "/" and the commands, their pane, shown from View, taking the left edge (the layout kept of
  earlier versions is let go once). The collections of favourites are named, with their counts ("■ Red 1"), only
  those with operators shown, and a word on how to fill one when none has any.
- Every theme's text, weak text included, reads at 4.5:1 at least on its panels, canvas and bars (WCAG's AA, kept
  by a test): the weak text of Default, Immaterial and Riparian stronger, Immaterial's accent darker (white on it),
  the light themes' warnings, errors and links darker; the operator, the file and the folded inputs written on a
  node's box between the weak and the plain text. A second light theme, **Field Notes**: warm paper and a forest's
  green, for daylight and projectors.

## [0.83.0] - 2026-10-16

### Added
- `stack.layer`: one layer of a stack as a map, by its name or its number from 0: a band of rasters, a layer an
  operator made (`total` of `prioritize.evaluate`'s `ferrier`), a step of a loop's series; names that are numbers
  too, which an expression cannot reach.
- The node editor's boxes are as wide as their texts need: a long name, and on every row an input's and an
  output's names side by side (`prioritize.optimize`'s no longer run into each other); measured in the fonts
  they are written in. Drag a box's right edge to make it wider (kept in the model as `width`, never narrower
  than its texts), double-click the edge to bring it back; Arrange's columns and the place of a new node follow
  the widths, and the column of what a scope reads is as wide as its longest name; a file's name at the bottom
  of a box is cut to the box's width.
- `prioritize.optimize` solves with Gurobi where it is installed and licensed (`solver = "gurobi"`, `threads`), as
  prioritizr's `add_gurobi_solver`: its library loaded when a node asks for it (found by `GUROBI_HOME`, the
  installers' folders or `TERRAFLUX_GUROBI`), nothing of Gurobi needed to build or run Terraflux; without it, HiGHS
  solves and a warning says why.
- How far a node is, as its operator tells it, in a bar at the top of the window: the prioritizations' gap proved
  falling to the gap asked for, with the best plan and the bound. **Stop** interrupts a node in the middle: the
  solvers stop (HiGHS, landschaft's engines and Gurobi), and a node that cannot stop in the middle is left to end
  in the background a second later, its results not shown. Operators are given the means (`Ctx::progress`,
  `Ctx::stopped`).
- `terraflux mcp`: run_model runs in a thread of its own, so the other tools (open_in_app...) answer while a model
  runs; it tells the client how it goes (progress notifications: the nodes, the solvers' gap) and stops when the
  client cancels it.
- An output of a model writes one layer of a stack alone with `layer` (`[outputs.importance]`, `from =
  "evaluation.ferrier"`, `layer = "total"`), and `terraflux mcp`'s `set_output` takes it.

### Fixed
- A model run in the application or by `terraflux run` (each in a thread of its own) gave its OpenMP kernels and
  solvers one thread, OpenMP's for a new thread: they have the threads in use again (every core but one by
  default), as landschaft's.

### Changed
- `prioritize.optimize` solves as landschaft does: a minimum set with boundary penalties or symmetric
  connectivity rewards on landschaft's cut engine (a Lagrangian bound by minimum cuts, repaired selections), a
  budgeted minimisation with pair penalties on its closure engine, then HiGHS's MIP from their plan when the gap is
  not reached and the model has at most 50,000 columns. A minimum set of 49 species with a boundary penalty and
  locked-in areas on 85,732 cells: from 45 minutes (HiGHS's MIP alone) to 4 seconds, the same plan as landschaft.

## [0.82.0] - 2026-10-16

### Added
- `terraflux mcp`: Terraflux as a server of the Model Context Protocol, for AI assistants (Claude Desktop, Claude
  Code, any MCP client) to build models in plain words: the templates and the operators listed and described (a
  node of a template shown for each), a model made empty or from a template, its inputs, nodes, connections,
  settings, containers' bodies and outputs edited (each edit checked as the node editor checks it: a wrong
  connection, a setting the operator does not have or a cycle refused with the reason, the file left as it was;
  the problems left listed after each edit), the nodes arranged, the model checked, run and its outputs looked at
  (a raster's size, georeference, classes or statistics; a table's first lines), and the model opened in the
  application. New nodes are placed beside what they read; the file is written whole, so a program following it
  never reads half a model.
- The application follows the file of the model open: written by another program (an assistant through
  `terraflux mcp`, a text editor), it is read again, as an edit that ctrl+Z undoes (the data read again when the
  inputs changed). With only nodes moved here and not saved, the file's model is taken with the nodes where they
  were moved; with other edits not saved, a bar asks whether to read it or keep the model as it is.

## [0.81.0] - 2026-10-16

### Added
- A Files pane, in a tab beside the operators', a file manager for data:
  - back, forward and up (alt+arrows, the mouse's side buttons), the path's folders as buttons (a click on the
    bar, or ctrl+L, to type a path, the folders that match offered, tab completing), places (folders pinned with
    the star, the model's, home, documents, desktop, downloads, the drives, the folders seen last);
  - a folder's rasters, vectors and models (every file if asked; hidden files and the files that go with others
    left out), with their icons, size and age, sorted by any column (names as people sort them: `map2` before
    `map10`), filtered by a part of the name or a pattern (`*.tif`); a folder opens in place by its arrow;
  - a click chooses, ctrl+click one more, shift+click a range; the arrows, right and left (a folder opened or
    closed in place) and the first letters of a name from the keyboard;
  - the raster or vector features chosen described under the list (a picture of them, their size, coordinate
    system, cells, bands, type, no-data; features, kinds, attributes, bounds), read in a thread;
  - what is chosen dragged to the model's canvas is a node each that reads it, where it is let go (several at
    once); enter or a double click adds it after the node selected; a right click reads a folder of rasters as
    one stack, opens a model, copies the path, shows the file in the system's file manager.

  The folder shown, the folders pinned and seen last are kept between sessions.
- Rasters and vectors dragged from the system's file manager onto the main window or a window of its own: a node
  each that reads it (`raster.read` or `vector.read` by the extension); a model dropped is opened.
- A node that takes any inputs (`stack.of`'s maps, a group's values) shows a place for one more on the canvas: a
  connection let go on it, or anywhere on the node, adds an input named after the node it comes from
  (`elevation.map`: `elevation`, then `elevation_2`...). A raster joins a stack by a connection.
- At the viewer's closest zoom each cell's value is written in it (only the cells in view, read from the map in
  memory or the tiles kept).
- ctrl+D duplicates the nodes selected (beside them, reading what they read); ctrl+C copies them and ctrl+V
  pastes them, through the system's clipboard (to another container, model or window): new nodes each time, of
  new ids (`calc`: `calc_2`), connected to each other as the originals were, where the pointer is on the canvas.
- Space over the viewer's map hides it (what is under and over it seen alone), and shows it again.
- `table.read`: a table from a file, comma-separated (`.csv`), tab-separated (`.tsv`, `.txt`), with `;` and a
  decimal comma as Excel writes them where a comma marks the decimals (both found by themselves), or JSON; the
  columns of text left out (a table is of numbers), named in a warning. The Files pane lists `.csv` and `.tsv`
  files as tables (a node that reads them by a double click or a drag; their columns and first rows described).
- `table.write`: a table to a file, comma-separated (`.csv`, the default), tab-separated (`.tsv`, `.txt`) or
  JSON (`.json`); the separator and the decimal mark as a spreadsheet of the place expects (`;` and `,`). In a
  batch, each run writes it in its own folder, as the other writers.
- `view.plot` shows tables too (a `table` input): drawn as a chart in its tab, bars, lines, points or boxes
  (`chart`), of the columns `y` along `x`, a group by `by`; a chart suggested when none is asked (lines along a
  date, year, step or layer, else bars), and changed in the tab as any table's.
- Undo and redo of the model's edits: ctrl+Z, ctrl+shift+Z or ctrl+Y, and two buttons over the canvas. Each edit
  is a step (a node added, deleted, pasted, a connection, a setting); what changes while a button of the mouse is
  held (nodes dragged) is one.

### Changed
- `raster.read` has one output, `raster`: the file on disk, one band (`band`, else the first) where a node takes a
  raster and every band where it takes a stack, as its `map` and `stack` outputs were. A model that read them
  reads `raster` (brought up to date when it is read; the website's tutorials and the templates changed). The
  viewer shows one entry per raster read among the maps over a map (three before), and the buttons of a node's
  outputs only for a node of several. Types are named for people where the application shows them: a map in
  memory and rasters on disk are both a raster.
- A node added is named after its operator's whole name (`stack_of`, `raster_read`, `vector_read`,
  `landscape_mspa`), not after its last part (`of`, `read`).
- The operators' lists (the library, the search with "/", the menu) write the whole names (`stack.of`, the
  subject a shade weaker), not the part after the dot.
- On the model's canvas a node that reads or writes a file (`raster.read`, `vector.read`, `raster.write`...) and
  a model's input of a raster name the file at their bottom (its whole path when the pointer is over it). The
  nodes of data (the model's inputs, what reads, writes, stacks or plots) may have a header in the colour of the
  type they give (a map blue, vectors pink...), an option of the settings (Model), off at first.
- The viewer is a tab beside the model's, not under it, and opens with the first data (a raster or vectors read,
  a template; behind the model's tab when a model is opened). A run's plots open beside it without taking the
  front from the model.
- The operators' and the node's panes open a quarter narrower; the window opens maximized.
- The node editor's connections and ports in the colour of their type, the website's graphs' own: maps blue,
  stacks violet, tables gold, numbers green, vectors pink, agents ochre (a connection being drawn too).
- The operators pinned again at the window's left edge, the node's pane beside them, the model and the viewer in
  the rest. The operators' pane closed, they are in the drawer at the left edge and in the toolbar as before.

### Fixed
- The model's boxes, ports and connections drawn when another model is opened (a template after the blank model
  the application starts with): every editor's first shapes had the same generation, so the GPU kept the shapes
  it had and only the nodes' texts showed until a node was moved.
- The window opens maximized on Windows (made maximized, it was not once shown), and the model's pane is not blank
  when the layout kept had the viewer in front of it.
- A tab dragged out of the dock no longer trips egui's check of a widget drawn in two windows in one frame (its
  window of its own shown from the next frame).
- A pane opened from the View menu has the focus (it was put in the dock behind the tab in front).

## [0.80.1] - 2026-10-15

### Fixed
- `raster.read`'s `map` is the band chosen alone: since 0.80.0 a multi-band file's map reached map algebra, groups
  and submodels (inputs of any value) as a stack of every band. A band is still read once, whichever port asks.
- `terraflux calibrate` types a range by the model's value of the setting for every node (0.80.0 found only the
  model's first node).
- `terraflux run --set node.setting=value` refuses a setting the node's operator does not take (a misspelt name ran
  silently).
- The template "A city that grows (a loop)" grows: with its neighbours counted by `nbsum` (0.80.0), its bar was
  above every cell next to the old town; three city neighbours and a bar from 0.6 now.
- `terraflux serve` without a code on 127.0.0.1 prints the worker command with its address, and `terraflux jobs`
  says when there are none.

## [0.80.0] - 2026-10-15

### Added
- Templates for many more of the tools, each running as soon as it opens on data drawn by neutral landscape
  models: Forest gain and loss (simple, morphological and FAD change), Patches and their shapes (patch metrics,
  parcellation, contortion), Kinds of landscapes (spatial signatures, PCA, k-means), Network of a forest (MSPA's
  GTN components and importance), Habitat network (Conefor's IIC and PC, every patch's dPC), Circuit
  connectivity (Circuitscape's pairwise currents, Omniscape), Least costs and random walks (gdistance),
  Restoring a corridor (GTB's Restoration Planner), Irreplaceability of a plan (prioritizr's evaluation and a
  portfolio), A species in a warmer climate (response curves, importance, the projection and MESS), Drivers of
  land change (Markov, weights of evidence, logistic regressions), A city that grows (a cellular automaton in a
  loop), Farmers on their parcels (agents), CRAFTY, Land change analysis (transitions, Pontius's components,
  intensity analysis), Validating a simulation (figure of merit, fuzzy similarity, three-map comparison,
  kappas), Terrain of a surface, Carbon and its uncertainty (Monte Carlo) and More neutral landscapes.

### Changed
- The Templates menu in groups: patterns, landscape metrics, connectivity, conservation and restoration,
  species, land change, surfaces and uncertainty.
- `terraflux run --set node.setting=value` changes a node's setting, as a scenario does (`--set name=value` an
  input, as before).
- `terraflux calibrate` (and the Many runs pane) makes a range's values whole numbers when the model's value of
  the setting is one and decimals when it is not (`patches.expansion=0:1` of a share tries 0, 0.25, 0.5...), and
  rounds them to 12 significant digits (0.7, not 0.7000000000000001).
- `terraflux check` reports a setting written as a reference (`edge_width = "$x"`): settings take values.
- Map algebra: the radius of `nbsum nbmean nbmin nbmax nbcount` may be a number input (`nbmean(x, r)`).
- `stack.of` takes inputs named by numbers: `1_to_2` makes the layer a transition's (a probability for
  `dinamica.step`, so map algebra can make them), `1` a class's.
- `batch.run`'s table has a column `skipped` (inputs done before, not run again), and its report says so.
- A multi-band raster connected to an input of any type arrives as a stack of every band, not its first.
- A run writes its text outputs as plain text (`name.txt`), not as a JSON string.
- The node's note is edited in the Node pane ("Why").
- `terraflux serve --code` says how to start workers for its address (no beacons on 127.0.0.1), `terraflux
  workers` says when there are none and marks those not heard of for 30 s as silent, and `terraflux submit` prints
  its job once.
- `terraflux cloud-init --worker`: the worker service runs as an unprivileged user (systemd's `DynamicUser`).
- `terraflux new clue` and `new ordered` write the start band by its name when the observed file has band names.

### Fixed
- A model no longer stops when an output given only with some settings or inputs is missing and nothing reads
  it; a node that reads it is told why. This was the cause of `gave no output` with `metrics.class` and
  `metrics.patch` without `show` or by blocks, `bio.cluster` without calculations, Marxan's and ProxIRR's tables
  mode, Omniscape without its flow potential, and circuit networks without their maps.
- The templates counted neighbours in a state with `nbcount` (cells with a value) where `nbsum` was meant: the
  city of "A city that grows" spread to every suitable cell, and the land change templates' second date grew
  farming almost everywhere. The CLUE-S template takes a smaller iteration step (lulcc's default never met the
  demand on its 25,600 cells).
- The names of the landscape mosaic's classes 6 to 12 (two of them shared one), as landschaft 55d03a8.
- `sdm.maxnet` with a records table on `data` no longer takes the coordinates, the folds and the weights as
  predictors.
- `prioritize.calibrate`'s pareto approach takes any objective (it always solves min shortfall within budgets).
- A raster on disk written by `raster.write` no longer has its band's number as its description.
- The descriptions of `change.fos` (the sign of its change), `networks.gtn_importance` (within 1, 5, 10 % of the
  most important), `conn.*` (least-cost distances between centroids; importance without measuring distances
  again), `circuit.one_to_all` (all-to-one gives no resistances), `nlm.random_cluster`, `sdm.exdet` (mic is
  1-based), `sdm.fit`'s feature classes and `prioritize.calibrate`'s time limit; the README and `run --tiled` on
  what the tile size changes.

## [0.79.0] - 2026-10-14

### Added
- Machines rented in the cloud: `terraflux cloud-init` writes the setup script (cloud-init user data) of a server
  (Terraflux installed from the Linux package of this version, then saved with `terraflux cluster add`) or, with
  `--worker`, of a machine that keeps a worker running as a service for a server's coordinator (`--on <server>
  --coordinator <its private address>`: its port and code from the profile). In the application, the Computers
  pane copies both scripts.

## [0.78.0] - 2026-10-14

### Changed
- The tiled engine on many machines needs no storage in common: the model's maps go to the workers once (by
  hash), and the steps' layers by their tiles (a pass lists each layer's tiles and their hashes; a worker fetches
  those of its tiles and of their neighbours, and sends back by hash the tiles it wrote, fetched into the layer's
  folder on the machine that runs the model). The same maps as on one machine. `terraflux run --tiled
  --coordinator ... --shared-disk` keeps the workers reading and writing the maps on shared storage, as before.

## [0.77.0] - 2026-10-14

### Added
- Clusters and servers reached by SSH, saved once (`terraflux cluster add/list/test/stop/remove`; in the
  application, the Computers pane): `submit --on`, `calibrate --on` and `results --on` check Terraflux there (the
  same version), start its coordinator (with a pairing code of the profile's) and its workers (on the host, or in a
  Slurm allocation of `sbatch`), reach it through an SSH tunnel, and stop them once the batch is done; a batch
  whose client went away goes on there and is followed again by its job. "Many runs" in the application runs on a
  cluster chosen in **Run on**.
- `terraflux worker --threads` (the threads of each task) and `terraflux serve --no-beacons`.

## [0.76.0] - 2026-10-14

### Added
- Other computers of the local network help with the runs, from a code and nothing else: the **Computers** pane
  (the toolbar's "Computers...") shows this computer's code when "Let other computers help" is on, and lends this
  computer to another's by typing the other's code ("Lend this computer": the threads it uses, only when nobody
  uses it for some minutes, paused at will). The computers find each other on the network by themselves; the
  sharing computer lists those helping, the lent one says whom it works for and how many runs it did. "Many runs"
  goes to the application's own coordinator (its jobs in the user's application data), which other computers
  join when it is shared.
- Pairing codes for coordinators (`terraflux serve --code`, `worker --code`, `submit --code`, `calibrate --code`,
  `jobs --code`, `results --code`, `run --tiled --code`): TLS with a certificate the coordinator makes for
  itself, the coordinator's proof that it knows the code (an HMAC of its certificate) before anything is sent,
  the certificate then pinned, and every call with a token made from the code (PBKDF2-HMAC-SHA256); without
  `--coordinator`, the coordinator of the code is found on the local network by its beacons (UDP port 7701).
- The Windows installer lets Terraflux through the firewall for the local network.

## [0.75.0] - 2026-10-14

### Added
- Batches taken up again: the coordinator keeps its data, the results' files and its jobs on disk (`terraflux
  serve --data`, by default in the user's application data) and goes on with them when started again (the tasks
  that were running run again). `terraflux submit --detach` only submits a batch, `terraflux jobs` lists a
  coordinator's jobs and `terraflux results <job>` follows one to its end and writes its files (those written
  already kept). In the application, a batch the application was closed on is offered again ("Go on").
- Every file a model reads travels with it: the files of `raster.read` and `vector.read`, with the files beside
  them of the same name (a shapefile's parts, a raster's sidecars), the rasters of a folder or a pattern, and the
  files a scenario names; they were read from the paths of the machine that submitted the batch.
- The computer kept awake while a worker runs a task (Windows, macOS, Linux); `terraflux worker --idle <minutes>`
  takes tasks only when the computer's user is away (Windows and macOS).

### Changed
- Workers and clients of another version than the coordinator's are turned away, with the version to install
  (`WorkerHello.version` was not checked).
- Files travel in pieces of a megabyte (no more messages of up to a gigabyte held in memory); a task's files go
  to the coordinator as blobs, and the client fetches each run's files as soon as it is done.

## [0.74.0] - 2026-10-14

### Added
- Many runs from the application: the "Many runs" pane (the toolbar's "Many runs...") runs the model again and
  again on workers of this computer, as many at once as chosen. Replicates (the model's seeds found by
  themselves, or chosen), scenarios in a table (a row per scenario, a column per setting it changes, any input
  or node setting picked by name) and calibration (settings tried in ranges, every combination or random draws,
  each with some seeds, scored against an observed map by the fuzzy similarity, the figure of merit or the
  share of cells of the same class). The runs' progress and the time left; the results as a table, in a folder
  per run; the runs of a map as one series in the model in a click; a calibration's ranking, its scores against
  each setting and its best settings applied to the model in a click.
- `raster.read` takes patterns in the folders' names too (`runs/*/landuse.tif`: a file of each run, the layers
  named by their folders).
- `terraflux_dist::run`: a batch from start to end (placed here or on a coordinator, its data sent, followed,
  its results written), the command line's and the application's.

### Changed
- In a batch, the files the nodes write (`raster.write`, `vector.write`, `landscape.mspa_tiled`) go to each run's
  folder and come back with its outputs, instead of every run writing the same file.
- A batch's seed may be a node's (`--seed-input simulation.seed`) or several (`seed,other.seed`); a calibration
  may score any output of maps and start from a node's map (a `raster.read`), not only the model's inputs.

## [0.73.0] - 2026-10-14

### Added
- scikit-learn's models in `sdm.fit` (landschaft's fit with them): `model = "glm"` (StandardScaler and
  LogisticRegression, the L2 penalty's `C`; the optimum scikit-learn converges to, within its tolerance) and
  `model = "random_forest"` (RandomForestClassifier: the trees grown as its Cython grows them, from numpy's legacy
  MT19937 seeds and bootstraps, its rand_r, introsort and Gini; `n_estimators`, `max_depth`, `min_samples_split`,
  `min_samples_leaf`, `max_features`, `bootstrap`, `random_state`), the same trees and probabilities to the bit. The
  records weighted as landschaft's dataset weighs them (`weights`: balanced by default, equal, or a weight column),
  the forest's bootstraps drawn with the weights as scikit-learn draws them. The fitted model (`SdmModel`) is MaxNet,
  a GLM or a forest; `sdm.maxnet_predict`, `sdm.response_curves` and `sdm.importance` take any of them.

## [0.72.0] - 2026-10-14

### Changed
- `nlm.mosaic_tess` and `nlm.mosaic_gibbs` follow NLMR by default (`nlmr_compat`, as landschaft's): the k-th value
  goes to the k-th Voronoi cell in the order GEOS lists them (sf's `st_voronoi` of the germs' union), the values
  as floats as `terra::rasterize` writes them; `nlmr_compat = false` keeps the earlier maps (each germ its own
  value). The same maps as landschaft's, to the bit.

### Added
- GEOS's own C API for what GDAL does not pass on (`terraflux_gdal::geos`): the geos_c library next to GDAL's (in
  the installers' GDAL too), else the system's; its Voronoi diagram.

## [0.71.0] - 2026-10-14

### Added
- The calibration of FUTURES' patches (landschaft's calibrate_patches, r.futures.calib): `futures.calibrate`
  takes futures.simulation's inputs and settings and the development observed at the end; the patches developed
  in between give the patch library and the histograms of patch area and compactness, and every combination of
  the compactness means, ranges and discount factors is simulated `repeat` times from the start and compared with
  them (the histograms' chi-square distance, numpy's histograms and sums). The table of the combinations from the
  best and the library, the same as landschaft's to the bit.

## [0.70.0] - 2026-10-14

### Added
- Linux packages (`tools/make-linux-package.py`, in `target/linux`): a folder to keep anywhere, as a tar.gz (its
  `install.sh` links the command into `~/.local/bin` and adds a menu entry for the user) and a Debian package
  (`/opt/terraflux`, the command in `/usr/bin`, the menu entry). They bring the system's GDAL (its library and every
  library it needs but the C and C++ runtimes, in `gdal/`, each loading the others from there; GDAL's and PROJ's
  data and the web's certificates) and the OpenMP runtime: nothing else is needed but a glibc as new as the one
  they were built with. Terraflux builds and runs on Linux (Ubuntu 24.04: the application on Wayland and X11).

## [0.69.0] - 2026-10-14

### Added
- Multi-action planning on rasters (landschaft's actions_raster_data, after prioripy): `prioritize.actions_raster`
  takes the cells with a monitoring cost as the planning units, a layer per feature (its amounts) and per threat
  (its intensity), the actions' costs as numbers or maps (one for all, per threat in order or by name), the units
  locked in or out by maps, a sensitivity table and the cells' shared edges as boundary data; builds
  prioriactions's tables as landschaft builds them (relative targets shares of what each feature has where a
  threat harms it and where none does) and solves them as `prioritize.actions`; the monitored cells and every
  threat's actions as maps. The same tables and optima as landschaft's on its test rasters and on its example.

## [0.68.0] - 2026-10-14

### Added
- Zonation 5's projects (landschaft's read_project and run_project): `prioritize.zonation_project` reads a
  settings file (`.z5`), its feature list (weights, groups, thresholds, ABF's z, zero modes, value extraction as
  `file.tif/3`) and link files (conditions, retention, weight groups, connectivity kernels, interactions and
  matrices, impact scenarios and their binning) with the switches of the z5 command line (`-wgtcrxXhabfdiImsep`,
  `--area=N`, `--mode=`), reads the rasters they name and runs the ranking of `prioritize.zonation`, adding up the
  features by the blocks of rows of their files as z5 does; with `-p` the calls of the post-processing file run on
  the ranking one after another (the tables numbered by call, the maps named as Zonation's files). The projects of
  landschaft's 65 cases give the same rank maps as the z5 binary, to the bit.
- The height of a raster file's blocks (strips or tiles), read by Terraflux's own TIFF reader and by GDAL.

## [0.67.0] - 2026-10-14

### Added
- The matrices and rasters around prioritizr's problems (landschaft's prioritize): `prioritize.boundary_matrix`,
  `prioritize.adjacency_matrix` (4 or 8 directions), `prioritize.connectivity_matrix` (the shared boundary times
  the mean of a map or an attribute) and `prioritize.proximity_matrix` of cells or polygons, as the i, j, x tables
  the problems take; `prioritize.decay_connectivity` (distances between units as Conefor's dispersal
  probability), `prioritize.unit_points`, `prioritize.coarsen` and `prioritize.refine` (a problem on coarser blocks
  and its selection back on the fine grid), `prioritize.binary_stack` (zone numbers as a 0/1 layer per zone) and
  `prioritize.protected_cells`. The same as landschaft's, to the bit.

## [0.66.0] - 2026-10-14

### Added
- FUTURES' inputs (landschaft's simulate): `futures.devpressure` (r.futures.devpressure: the development pressure
  by occurrence, gravity or kernel, its r.mfilter by landschaft's C++, padded or not) and `futures.patch_library`
  (r.futures.calib's library of the patches developed between two maps, whole or per subregion, with a threshold;
  in the form of futures.simulation's patch_sizes). The same as landschaft's, to the bit (the kernel to the last
  bit of a float).

## [0.65.0] - 2026-10-14

### Added
- The comparisons of simulated and observed land change (landschaft's simulate): `validate.three_map` (lulcc's
  three-map comparison of Pontius et al. 2011 at blocks of several sizes, its table by landschaft's C++, the budget
  of agreement and disagreement and the figure of merit, overall, per class and per transition),
  `validate.kappa` (MOLUSCE's kappa, kappa location and histogram, correctness, Cramer's V, joint information
  uncertainty, correlation), `validate.error_budget` (MOLUSCE's error budget of Pontius 2002 at coarser
  resolutions), `validate.futures` and `validate.futures_grid` (r.futures.validation and gridvalidation:
  disagreement, kappa, kappa simulation and the change metrics, whole, by tiles or by regions). The same as
  landschaft's, to the bit (MOLUSCE's correlation and joint information uncertainty, whose numpy uses BLAS and its
  own logarithm, to 1e-12).

## [0.64.0] - 2026-10-14

### Added
- diffeR's comparisons at coarser resolutions (landschaft's simulate): `change.memberships` (the share of each
  class in blocks of cells), `change.composite` (the composite cross-tabulation of Pontius & Cheuk),
  `change.resolutions` (differenceMR: quantity, exchange and shift at resolutions growing by a factor, scaled to a
  population or not), `change.mad` (the mean absolute deviation of two continuous maps by quantity, strata and
  element, at every resolution) and `change.sample_to_population` (sample2pop). R's and terra's sums; the same as
  landschaft's, to the bit.

## [0.63.0] - 2026-10-14

### Added
- `vector.exact_sum` and `vector.distance` out of core (`block_rows`, and rasters on disk of more than 250 million
  cells): the exact sums with only the values under each polygon read from disk (exactextract's coverage of the
  polygon on the whole grid, in landschaft's pieces), the distances by strips into a GeoTIFF (the features burnt
  strip by strip, the column passes through a file of the nearest rows below, the rows' envelopes); the same bits
  as in memory.

## [0.62.0] - 2026-10-14

### Added
- Biodiverse's spatial conditions on shapefiles in `bio.spatial`, `bio.randomise` and the other nodes of
  conditions: `sp_point_in_poly_shape` (the groups inside or on the features of a vector file, `file.gpkg:layer`
  or a shapefile, or of those with an attribute's value, or the feature at a position; the neighbour's or the
  processing group's coordinates, other axes) and `sp_points_in_same_poly_shape` (the groups in the same features,
  identical geometries one); the same masks and neighbourhoods as landschaft's.

## [0.61.0] - 2026-10-14

### Added
- Biodiverse's other imports (landschaft's BaseData.from_raster, from_vector, from_hex, from_h3 and
  from_occurrences): `bio.raster` (a label per layer and its values as counts, a threshold for presences, larger
  group cells; or the values as labels), `bio.vector` (points, lines and polygons over a fishnet of cells: the
  cells they touch, or the area or length of their parts, GEOS through GDAL; counts from attributes, binarised),
  `bio.hex` (hexagons, pointy or flat), `bio.h3` (H3 cells, their ids kept) and `bio.occurrences` (a GBIF download,
  the zip or its table, a Darwin Core Archive with meta.xml, a delimited file or records with Darwin Core columns
  such as `sdm.download_gbif`'s; cleaned as read_occurrences cleans them, read a line at a time through GDAL's
  virtual file systems; in cells of degrees or of a projection). The same groups, labels, counts and coordinates
  as landschaft's.

## [0.60.0] - 2026-10-14

### Added
- Occurrence records from GBIF (landschaft's download_gbif): `sdm.download_gbif`, GBIF's occurrence search API
  through GDAL's HTTP (no account): a scientific name matched to GBIF's backbone (a synonym gives the accepted
  name's records) or a taxon key, by country, the box of an area, years, basis of record; up to 100 000 records,
  pages asked a few at a time; the records with coordinates as points with their Darwin Core terms and GBIF's
  issues, those less precise than a distance left out; the query, the taxon matched and the count.

## [0.59.0] - 2026-10-14

### Added
- The preparation of a species distribution model in one node (landschaft's prepare): `sdm.prepare` cleans the
  records, keeps one per cell, thins them, draws the accessible area, selects the predictors in it, draws
  background points in it, takes the predictors' values and makes the folds (spatial blocks, of the
  autocorrelation range too, quadrants, a checkerboard, random, jackknife or clusters), each step the node of its
  own with its settings and one seed; an area or background points of one's own in place of those steps; the
  report of what each step removed and every setting. The same records, predictors, folds and report as
  landschaft's, to the bit.
- The builder's template "Species distribution model": predictors and presences drawn by chance, `sdm.prepare`,
  MaxNet fitted over spatial folds and its map of suitability.

## [0.58.0] - 2026-10-14

### Added
- The area accessible to a species (landschaft's accessible_area): `sdm.accessible_area`, the land within a
  distance of the records (geodesic circles, GeographicLib's, merged), their convex hull or the regions that hold
  them (widened by a distance), cut at the antimeridian and folded back, within a mask; the polygon of landschaft
  (its areas, planar and geodesic, to 1e-9; GEOS through GDAL, so not its vertices).

## [0.57.0] - 2026-10-14

### Added
- The cleaning of occurrence records (landschaft's clean_occurrences, CoordinateCleaner's tests):
  `sdm.clean_occurrences` flags or drops records with invalid coordinates, equal longitude and latitude, at 0/0,
  at capitals, country and province centroids, GBIF's headquarters, biodiversity institutions or artificial
  hotspots (the circles of CoordinateCleaner from landschaft's C++, geosphere's or terra's), in the sea, in towns,
  outside the country they claim, far from the other records of their species (quantile, MAD or distance; the
  raster variant of large species) or duplicated; each test off, on, with its main option or with an object of
  options, with the `verify` of the capitals, centroids, GBIF and institutions. `sdm.reference_data` gives the
  reference data as features. CoordinateCleaner's tables travel inside Terraflux (tools/make_sdm_refdata.py);
  Natural Earth's land, urban areas and countries are downloaded on first use into the cache (pinned by their
  SHA-256), or connected as polygons of one's own. The same flags as landschaft's, record by record.
- Occurrence records as features without geometries (a CSV read by GDAL): their coordinates from the columns.

## [0.56.0] - 2026-10-13

### Added
- The choice of predictors and their range of autocorrelation (landschaft's vif, select_predictors and
  autocorrelation_range, after usdm and blockCV): `sdm.vif` (R's lm over the complete cells, usdm's sample of
  them on large grids, by latitude on lon/lat grids, or a table's rows), `sdm.select_predictors` (vifcor and
  vifstep, predictors kept, within an area) and `sdm.autocorrelation_range` (gstat's variograms, spherical,
  exponential, Gaussian and Matern, fitted to a sample of every layer; their median range and the block size).
  The same as landschaft's, to the bit.

## [0.55.0] - 2026-10-13

### Added
- The species distribution modelling workflow with MaxNet (landschaft's dataset, fit, response_curves and
  importance): `sdm.dataset` (presences, or a table of 1 and 0, and background points with the predictors'
  values at their cells), `sdm.fit` (one fit per fold of the records with AUC of training and test, their
  difference, the largest TSS, Boyce's index and the omission rates; their means and standard deviations as
  pandas computes them; the model on every record and its threshold), `sdm.response_curves` (each predictor
  over its range, the others at their mean or median) and `sdm.importance` (the drop of AUC when a predictor
  is shuffled, numpy's generator). The same as landschaft's, to the bit.

## [0.54.0] - 2026-10-13

### Added
- Partitions of records for cross-validation (landschaft's folds, after ENMeval and blockCV), as a fold column
  added to the records: `sdm.random_folds`, `sdm.jackknife_folds`, `sdm.quadrant_folds` (the four
  orientations), `sdm.checkerboard_folds` (one or two aggregations of the predictors' cells),
  `sdm.spatial_blocks` (hexagons pointy or flat, squares of a size or of rows and columns; random, systematic
  or checkerboard assignment), `sdm.spatial_clusters` (R's kmeans of the coordinates or of the predictors) and
  `sdm.buffer_folds` (buffered leave-one-out). R's generator and long double arithmetic; the same as
  landschaft's, to the bit (with a seed: landschaft draws one when none is given, Terraflux defaults to 1).

## [0.53.0] - 2026-10-13

### Added
- Occurrence records and background points for species distribution models (landschaft's thin, one_per_cell
  and background): `sdm.thin` (spThin's thinning, the repetition that keeps the most), `sdm.one_per_cell` (the
  first record per cell, those on missing cells left out or not) and `sdm.background` (dismo's randomPoints with
  R's sampling: the cells' widths on lon/lat grids, a bias, an area, a band of distances from the records,
  outside their environmental envelope). Records as points or tables of coordinates (lon/lat columns
  recognised; PROJ through GDAL between coordinate systems). The same as landschaft's, to the bit.

## [0.52.0] - 2026-10-13

### Added
- MaxNet species distribution models (landschaft's MaxNet, the maxnet package; a new `SdmModel` type):
  `sdm.maxnet` (from a table with a presence column, or tables of presences and background points; the
  default classes or any of l, q, p, h, t; categorical predictors; the regularisation multiplier; the presences
  added to the background or not) with its coefficients by feature name, and `sdm.maxnet_predict` (on a stack
  of the predictors or a table; cloglog, logistic, exponential or the link; clamped or not). The same as
  landschaft's, to the bit.

## [0.51.0] - 2026-10-13

### Added
- The evaluation of species distribution models (landschaft's evaluate, boyce, threshold and omission_rate):
  `sdm.evaluate` (AUC, the point-biserial correlation and its p-value, prevalence, the largest TSS and Boyce's
  index; dismo's confusion table at every threshold and the optimal thresholds), `sdm.boyce` (ecospat's
  continuous Boyce index in a moving window, classes or breaks), `sdm.threshold` and `sdm.omission_rate`
  (ENMeval's mtp and p10). The same as landschaft's, to the bit, but for the p-value of the correlation
  (Student's t) to rounding.

## [0.50.0] - 2026-10-13

### Added
- Extrapolation of species distribution models (landschaft's mess, mop and exdet): `sdm.mess` (Elith's MESS,
  per predictor and the least), `sdm.mop` (the mop package's MOP: basic, simple or detailed ranges, Euclidean
  distances scaled and centered or not, Mahalanobis, rescaled, over the in-range, out-of-range or all
  conditions) and `sdm.exdet` (Mesgaran's NT1, NT2 and most influential covariate), on stacks of predictors or
  tables. MESS and the Euclidean MOP are landschaft's to the bit; the Mahalanobis MOP and ExDet (numpy's
  covariance, LAPACK's inverse) to rounding.

## [0.49.0] - 2026-10-13

### Added
- Settlement probabilities from a spatial absorbing Markov chain (the samc package's model, landschaft's
  settlement_probabilities and HabitatGraph.from_samc): `conn.settlement_probabilities` (a random walk over a
  resistance, absorbed by an absorption raster or from the dispersal distance; the probability of settling
  between nodes over their boundary vertices, summed or averaged) and `conn.graph_from_samc`. landschaft's C++
  for the chain, Eigen's sparse LU for its inverse (scipy's SuperLU in landschaft: the same to rounding).
- `conn.protconn_raster`: ProtConn of a raster of protected areas (its patches at their centroids; the
  region's features rasterized or a raster of the region; transboundary patches, the region's parts). The same
  as landschaft's, to the bit. The `conn` module is all in Terraflux.

## [0.48.0] - 2026-10-13

### Added
- Saura's ProtConn (Makurhini's MK_ProtConn, landschaft's protconn): `conn.protconn` (the protected areas
  clipped to a region and dissolved, transboundary ones around the nodes or the region, the region's parts
  with `bound`, edge or least-cost distances, a single protected area; ECA, PC, Prot, ProtConn, ProtUnconn,
  RelConn and ProtConn's fractions; every protected area's dProt, dProtConn and varProtConn with `delta`) and
  `conn.protconn_regions`. Grids over a region: `conn.make_grid` (sf's st_make_grid hexagons or squares,
  clipped or wholly inside), `conn.connectivity_grid` (ProtConn or PC per cell, Makurhini's MK_Connect_grid)
  and `conn.eca_change_grid` (the change of ECA per cell between times, MK_dECA_grid). The same as
  landschaft's, to the bit.

## [0.47.0] - 2026-10-13

### Added
- Habitat graphs of vector features (landschaft's node_table, distance_table and HabitatGraph.from_vector):
  `conn.nodes` and `conn.distances` take features too (GDAL's areas of the polygons, an attribute column or
  values, weighted or not; the distances between centroids, the largest part's or GEOS's point on the surface
  when it falls outside, between edges with GEOS's distance, or least-cost and commute times over a
  resistance), `conn.graph_from_vector`, `conn.to_vector` (a node table joined onto the features) and
  `conn.focal_nodes` (Makurhini's MK_Focal_nodes: each node's importance in the graph of the nodes within a
  search buffer of it). GEOS through GDAL (`terraflux_gdal::Geos`, `difference`, `make_valid`). The same as
  landschaft's, to the bit.

### Fixed
- A lint of 0.46.0 (`conn::graph::Readings`).

## [0.46.0] - 2026-10-13

### Added
- `conn.eca_change`: the change of the equivalent connected area from habitat graph to habitat graph (a series
  of them, e.g. years; Makurhini's MK_dECA, landschaft's eca_change): the habitat's area and ECA of PC or IIC
  per distance, their changes, their ratio and how they compare. The same as landschaft's, to the bit.

## [0.45.0] - 2026-10-13

### Added
- Habitat graphs of raster patches (landschaft's node_table, distance_table and HabitatGraph.from_raster, as
  Makurhini's nodesfile and distancefile): `conn.nodes` (each patch's area in a unit, or an attribute, weighted
  or not; the patches of habitat classes with 8 or 4 neighbours, or a raster of patch ids), `conn.distances`
  (between centroids, edges, least-cost over a resistance as gdistance with Makurhini's or the mean
  conductance, or commute times; the straight distance for the pairs a resistance does not join),
  `conn.graph_from_raster` and `conn.to_map` (a node table's column back on the patches). The same as
  landschaft's, to the bit.

## [0.44.0] - 2026-10-13

### Added
- Habitat graphs (landschaft's HabitatGraph, a new `HabitatGraph` type): `conn.graph` from a table of nodes
  and a table of pairs or an n x n matrix (distances, probabilities or links; Makurhini's conventions or
  Conefor's; nodes to restore), `conn.read_conefor` and `conn.write_conefor` (Conefor Sensinode's files),
  `conn.overall` (NC, NL, H, CCP, LCP, IIC, F, AWF, PC, their equivalent connectivity and fractions),
  `conn.node_importance` (dI and its intra, flux and connector fractions, BC, BC(IIC), BC(PC); the gain of each
  node to restore, Makurhini's dIICres/dPCres or Conefor's d*add), `conn.link_importance`, `conn.link_change`,
  `conn.components` and `conn.centrality` (Makurhini's MK_RMCentrality, without igraph's walktrap
  communities). The same as landschaft's, to the bit, but for what it computes with numpy's BLAS, SIMD
  exponentials or LAPACK (the eigenvector centrality, the strengths on probabilities, the gains of nodes
  to restore): those to rounding.

## [0.43.0] - 2026-10-13

### Added
- The rest of landschaft's neutral landscape models (its port of NLMR): `nlm.random`, `nlm.percolation`,
  `nlm.planar_gradient` and `nlm.edge_gradient` (a direction drawn with the seed when none is given),
  `nlm.distance_gradient` (terra's haversine or GDAL's proximity as NLMR, or exact distances), `nlm.curds`
  (wheyed too), `nlm.mpd` (NLMR's grid or the size asked for, on a torus too), `nlm.neigh`,
  `nlm.random_rectangular_cluster`, `nlm.mosaic_field` (every step's sum and the infinite field too),
  `nlm.mosaic_tess` and `nlm.mosaic_gibbs` (each germ with its own value: NLMR's order of GEOS's Voronoi cells is
  not reproduced), and the utilities `nlm.classify` (by weights, a real landscape's shares or Jenks's natural
  breaks), `nlm.binarize`, `nlm.merge` and `nlm.rescale`. The same as landschaft's, to the bit.

### Changed
- `nlm.gaussian_field` and `nlm.fbm` rescale with NLMR's util_rescale (all 0 when the minimum and maximum are
  all.equal), as landschaft.

## [0.42.1] - 2026-10-13

### Fixed
- GDAL no longer writes its `.properties` index next to the gzipped rasters it reads (`/vsigzip/`): reading
  Circuitscape's gzipped test grids left the landschaft submodule modified.

## [0.42.0] - 2026-10-13

### Added
- Circuitscape's and Omniscape's INI files (landschaft's circuitscape_ini and omniscape_ini):
  `circuit.circuitscape_ini` runs a Circuitscape configuration on rasters (advanced, pairwise, one-to-all and
  all-to-one, with its polygons, mask, reclassification, included pairs and variable source strengths; ASCII
  grids read in double, gzipped ones, GeoTIFFs and text lists; paths from the working directory or next to the
  INI) and writes its outputs as Circuitscape names them (ASCII grids with C++'s shortest round trip, or
  GeoTIFF; the resistances and their three columns; the configuration), the pairs' and points' voltage and
  current maps too; `circuit.omniscape_ini` runs an Omniscape configuration (every option of Omniscape 0.7,
  conditions, reclass tables with missing values, single precision) and writes its project folder. The same as
  landschaft's, to the bit, on Circuitscape's fifty-one raster test cases and landschaft's Omniscape
  configurations, and the files written byte for byte.

## [0.41.0] - 2026-10-13

### Added
- Circuitscape's network mode and included pairs (landschaft's circuit module): `circuit.network_advanced`
  (currents and grounds on a graph of edges: node voltages, node and branch currents) and
  `circuit.network_pairwise` (the effective resistances between focal nodes, with each pair's voltages and
  currents and their sums), the graphs, focal nodes and values read as Circuitscape reads its text files; and
  Circuitscape's include / exclude pairs files (lists and min/max matrices) for `circuit.pairwise`,
  `circuit.one_to_all` and the networks, focal regions with included pairs rebuilding the graph per point. The
  same as landschaft's, to the bit, on Circuitscape's network test cases and the pairs cases made here.

## [0.40.0] - 2026-10-13

### Added
- Batch processing (landschaft's batch, GuidosToolbox's Batch Process): `batch.run` runs any operator on the
  rasters of a folder, a glob pattern or a list, in parallel, one raster per worker, and writes what it gives
  named after the input and the operator (maps as GeoTIFF, tables as CSV, texts) with a JSON per input (the
  parameters, the files, the numbers, the time); inputs already done are skipped unless overwrite, errors are
  reported per file, and MSPA keeps GuidosToolbox's naming.

## [0.39.0] - 2026-10-13

### Added
- Propagation of uncertainty (landschaft's uncertainty module, the tools plangea uses), a subject of its own
  in the builder: `uncertainty.sum_sd` (the standard deviation of a total of uncertain values: quadratic,
  linear, or with a correlation), `uncertainty.product_sd` (Goodman's variance of a product, cell by cell or of
  the total), `uncertainty.interval` (the normal confidence interval, cephes' ndtri as scipy's norm.ppf),
  `uncertainty.delta` (the delta method over an expression of maps, numbers and uncertain parameters, map.calc's
  expressions; central or forward differences) and `uncertainty.monte_carlo` (draws of the parameters with
  numpy's generator: its ziggurat normal, PCG64 and seed; the mean, standard deviation and quantiles as numpy
  computes them). The same as landschaft's, to the bit (`tools/make_uncertainty_reference.py`).

## [0.38.0] - 2026-10-13

### Added
- Properties and label matrices of Biodiverse's analyses (landschaft's bio module): `bio.properties` (label or
  group properties of a basedata from a Biodiverse properties file; they feed the lbprop and gpprop
  calculations, and RANGE and ABUNDANCE the labels' ranges and abundances) and `bio.matrix` (a label matrix,
  full, a triangle or sparse pairs; a new type of value) for the matrix calculations of `bio.spatial`,
  `bio.randomise`, `bio.cluster` and `bio.region_grower`. `bio.randomise` moves group properties by set or by
  item (randomise_group_props_by), within subsets too. The reading, the property tables and the shuffles are
  ported from landschaft's Python. The same as landschaft's, to the bit.

## [0.37.0] - 2026-10-13

### Added
- Biodiverse's cluster analysis and region grower (landschaft's bio module): `bio.cluster` (groups joined by
  the dissimilarity of their labels: Sorenson, Jaccard and S2 directly, Bray-Curtis, the phylogenetic ones and
  the rest pair by pair; every linkage, link_recalculate through a callback from landschaft's C++ to Rust;
  several conditions, a definition query, tie breakers, singletons) and `bio.region_grower` (regions grown by
  an index of the merged groups). The dendrogram as a tree, its nodes with Biodiverse's values, the
  calculations of each node in Biodiverse's hierarchical mode and, with k, the dendrogram cut into k clusters as
  a table and a map. The tree's finishing (lengths, zero-length branches raised, roots joined, ladderised,
  terminals numbered) and the cut are ported from landschaft's Python. The same as landschaft's, to the bit, in
  thirteen cases.

## [0.36.0] - 2026-10-13

### Added
- Randomisations of Biodiverse's analyses (landschaft's bio module): `bio.randomise` runs an analysis, then
  randomises the basedata again and again with Biodiverse's Mersenne Twister (rand_structured and its spatial
  forms with their allocation orders, backtracking and seed groups; the independent swaps, modified, and
  curveball; rand_csr_by_group; rand_nochange), within subsets of groups or a definition query, with labels kept
  in place and tree terminal names shuffled, and compares each index: the counters (C, P, Q, SUMX, SUMXX, T),
  p-ranks, z-scores and CANAPE, as tables and maps. The subsets, the labels kept in place, the merging, the tree
  shuffle and the counters are ported from landschaft's Python; the algorithms are its C++. The same as
  landschaft's, to the bit, in fourteen cases, scalar and list indices.

## [0.35.0] - 2026-10-13

### Added
- Phylogenetic trees and indices (landschaft's bio module): a new type of value, the tree, and `bio.tree` (a
  Newick or Nexus file read as Biodiverse reads it: unnamed nodes `0___`..., `__dup` names, the Translate table,
  all-zero lengths made 1, a forest given a root; the reader ported from landschaft's Python); `bio.spatial`
  takes a tree for the phylogenetic indices (PD, PE, phylo ABC, Sorenson and Jaccard, RPD and RPE, AED, NRI and
  NTI, VPD, the clade contributions and losses...: Biodiverse's trimming to the labels and the totals ported
  too, PhyloCom's random tips from a seed) and gives the list indices as JSON. The same as landschaft's, to the
  bit, on its test trees (trimmed, polytomies, names that do not match).

## [0.34.0] - 2026-10-13

### Added
- Biodiversity analyses (landschaft's bio module, its port of Biodiverse), a subject of their own in the builder
  and a new type of value, the basedata (Biodiverse's groups x labels sample counts with the groups'
  coordinates): `bio.table` (a delimited text file read like Biodiverse's import: group columns snapped to cells,
  kept as written or as text axes, labels joined with ':', counts summed; separator and quote guessed) and
  `bio.points` (point records read like its feature import); `bio.spatial` (Biodiverse's spatial analysis:
  every index of landschaft's C++ engine without trees, matrices or properties, over the neighbourhoods of
  spatial conditions in Biodiverse's Perl, one or two sets, with a definition query; a table of the indices
  of each group and their maps when the groups are cells). The Perl of the conditions (its tokenizer and Pratt
  parser), their compilation, Biodiverse's result types and the recycling of neighbour lists are ported from
  landschaft's Python to Rust. The same as landschaft's, to the bit, scalar and list indices, on its example
  sites under twenty conditions (`tools/make_bio_reference.py`).

### Fixed
- The test of RSP's method 2 allows the 1e-11 that landschaft's own runs differ by on asymmetric transitions
  (Eigen's SparseLU), where 1e-12 failed now and then.

## [0.33.0] - 2026-10-13

### Added
- The rest of landschaft's grid distances: `cost.barriers` (the connections across a boundary: per break of
  continuous values, or per pair of categories in gdistance's order, one or both ways) and `cost.areas` (per
  category), sets of transitions in one stack (the layout's `count`); `cost.mahal` (gdistance's Mahalanobis
  transition of a stack of maps: R's cov, solve, %*% and mean, LAPACK's recursive LU over R's BLAS rewritten
  in Rust); and `cost.path_inc` (the overlap and non-overlap of the routes from a common origin, random walks or
  randomised shortest paths, weighted by the costs, a transition or a set). The same as landschaft's, to the
  bit.

## [0.32.0] - 2026-10-13

### Added
- Random walks over grid distances (landschaft's cost module, gdistance's through its C++):
  `cost.commute_distance` (the expected commute distances, each separate part of the graph with its own volume),
  `cost.passage` (the net passages of a random walk, gdistance's grounded cell or the exact currents; with theta,
  randomised shortest paths, net or total), `cost.rsp_distance` (methods 1 and 2, net or total). And the
  matrix's own algebra: `cost.normalize` (row, col, symm), `cost.sum_reciprocal`, `cost.combine` (sum,
  difference, product in the common layout), `cost.math` (R's sqrt, exp, log and powers, factors and
  reciprocals on the connections) and `cost.to_map` (gdistance's raster(): sums and means of rows and columns).
  The same as landschaft's, to the bit, but for RSP's method 2, whose Eigen SparseLU changes its last bits from
  run to run in landschaft itself (within 1e-12).

## [0.31.0] - 2026-10-13

### Added
- Grid distances (landschaft's cost module, the R package gdistance through its C++), a subject of their own in
  the builder: `cost.transition` (the connections of every cell with its 4, 8 or 16 neighbours, a built-in
  function of the two cells' values: mean, min, max, sum, prod, from, to, diff, harmonic, geometric; symmetric or
  not; its reciprocal for a conductance of a resistance), kept as a stack of a layer per direction whose grid
  carries the layout; `cost.geo_correction` (types c and r, scaled or not; great-circle distances on
  longitude/latitude grids), `cost.acc_cost` (from points or a mask), `cost.distance` (among points or from
  points to others; a table) and `cost.shortest_path` (lines through the cells' centres and a map of the share of
  the paths through each cell). Global longitude/latitude grids wrap around as gdistance's. The same as
  landschaft's, to the bit, on its example rasters and a global grid (`tools/make_cost_reference.py`).

## [0.30.0] - 2026-10-13

### Added
- The terrain of surfaces (landschaft's surface module: spatialEco's, terra's and geodiv's numbers through its
  C++): `surface.terrain` (slope and aspect, 8 or 4 neighbours), `surface.tpi` (a rectangle of cells or a circle of
  a radius, normalised or not), `surface.tri`, `surface.vrm`, `surface.srr`, `surface.dissection`,
  `surface.curvature` (planform, profile, total, McNab's, Bolstad's), `surface.spherical_sd`, `surface.hsp`,
  `surface.trasp`, `surface.hli` (McCune & Keon's three equations, the latitude of the map's centre), and geodiv's
  `surface.fit_plane` (a polynomial trend of order 0-6 or the best, and the surface without it), `surface.peaks`
  (peaks or valleys), `surface.bearing` (the Abbott-Firestone curve) and `surface.aacf` (the areal
  autocorrelation). The same as landschaft's, to the bit, on its example surfaces.

## [0.29.0] - 2026-10-13

### Added
- Surface metrics (landschaft's surface module, ISO 25178 as the R package geodiv computes them, geodiv's bits),
  a subject of their own in the builder: `surface.metrics` (the 27 metrics of a surface: amplitude, hybrid,
  local features, the bearing area curve, spatial and fractal; a table) and `surface.window` (the metrics in a
  square or circular moving window around every cell; a map each), geographic grids measured as such
  (`terraflux_gdal::is_geographic`). The same as landschaft's, to the bit, on its example surfaces
  (`tools/make_surface_reference.py`).

## [0.28.0] - 2026-10-13

### Added
- Circuitscape's raster modes with focal points: `circuit.pairwise` (the effective resistance between every pair
  of focal points, focal regions of an id's cells, the cumulative and largest current maps) and
  `circuit.one_to_all` (one-to-all and all-to-one: the voltage over the strength at each point, strengths by
  position as Circuitscape, the cumulative and largest currents, the focal nodes' currents at 0), with
  short-circuit polygons, a mask, averaged resistances and a reclassification; the same as landschaft's, to the
  bit (`tools/make_circuit_reference.py`).

## [0.27.0] - 2026-10-13

### Added
- Connectivity by circuits (landschaft's C++ of Circuitscape 6.1 and Omniscape 0.7), a subject of its own in the
  builder: `circuit.omniscape` (Omniscape's moving-window circuits: the cumulative current, the flow potential and
  the normalised current; sources from the resistance or given, their threshold and cutoff, a reclassification,
  conditional connectivity with future conditions, single precision, the no-data mask) and `circuit.advanced`
  (Circuitscape's advanced mode on a raster: currents and voltages from sources and grounds). The same maps as
  landschaft's, to the bit, on the cases of its references (themselves checked against Omniscape.jl and
  Circuitscape.jl) (`tools/make_circuit_reference.py`).

## [0.26.0] - 2026-10-13

### Added
- The weights of evidence in the viewer, editable as in Dinamica EGO: a transition and a variable chosen, the W+
  of its ranges in bars (the significant in the accent) and the ranges in a table (cells, changed, W+, contrast):
  two ranges merged or one split, a weight set by hand (or back to the fitted one), the fit run again. `woe.fit`
  takes `ranges` per transition (`"1->2": {variable: limits}`, as landschaft's (from, to) keys) and `weights`
  set by hand (`{"1->2": {variable: [W+ or null per range]}}`), in the probabilities too; the tiled engine takes
  the weights and refuses ranges of one transition.
- Legends: `raster.legend` gives the names and colours of a map's classes, carried with the map in its grid's
  metadata (`terraflux_legend`) to the maps made from it with its classes and to the GeoTIFF it is written to;
  `raster.reclassify` leaves them behind. The viewer colours and names the classes so, and edits them ("Edit the
  legend": a colour and a name per class, applied as a `raster.legend` node after the node shown, or that node
  changed).

### Fixed
- A node selected by what the viewer shows (the legend's node, the WoE's...) is shown at once (the viewer no
  longer put back the node it had shown).

## [0.25.0] - 2026-10-13

### Added
- The analysis of land change, landschaft's `changes` (OpenLand's and diffeR's numbers, to the bit):
  `change.crosstab` (the transition matrix of two maps), `change.transitions` (between consecutive dates and from
  the first to the last, the area of each class at each date, the matrix), `change.intensity` (Aldwaik & Pontius's
  intensity analysis at the interval, category and transition levels, with its stationarity; km2 or cells),
  `change.components` (gain, persistence and loss, and the quantity, exchange and shift components of each class
  and overall, diffeR's diffTablej and overallDiff), `change.count` (how many times each cell changed) and
  `change.suggest` (the transitions worth modelling: the largest first, those of 1 % of the change at least).
  Tested against landschaft on Pie (three dates) and Sibuyan (`tools/make_change_reference.py`).
- The assistant of the analysis of changes: in the Prepare pane, a series of maps of classes shows the
  transitions worth modelling with their shares, and one click adds the analysis (a node each, with a note).

## [0.24.0] - 2026-10-13

### Added
- The viewer's Metadata (beside Save output): what rasters, maps or vector features are, in a window: the files
  and their reader, the layers, the data type and no-data value, the coordinate system (and its WKT), the cells
  (their number and size), the extent and origin; of features their number, kinds, attributes and bounds; copied
  as text.

### Changed
- The Operators menu of the toolbar is a classic menu (as Photoshop's): the favourites by colour, then a submenu
  per subject, its groups a level below; the drawer at the left edge keeps the search and the collections.
- ctrl+R selects the node's whole name: what is typed replaces it.

## [0.23.0] - 2026-10-13

### Added
- Maps stacked in the viewer: other maps of the run drawn over the one shown ("Maps over it", each as opaque as
  asked, placed by its coordinates whatever its cells), and in the model: `view.plot` takes `features` (vectors)
  and `under` (another plot's shown, drawn beneath it), plots stacking as deep as wanted (`builder::plot_stack`).
- A cell clicked on a stack, a series or rasters of many layers: its values along the layers in a chart beside
  the map (by the layers' names when they are years), a click on the chart shows that layer.
- Close in, the cell under the pointer outlined in white; the cell clicked in the accent.
- The zoom stops out at about the world when the map has a coordinate system (at 32 points across without one).
- B over a map puts the satellite's imagery under it, or takes it away.
- A map opened goes in a tab beside the viewer, wherever the viewer is (a window of its own too).
- Favourites in the operators' library, as Ableton Live's collections: seven colours, an operator put in them by
  a right click, the squares of its colours beside its name; a colour chosen shows its operators.
- Settings (in the toolbar): the theme (Default, Immaterial, Riparian, Angst Robot, Twenty Four Carat), the
  text's size, the interface's zoom, the threads the functions use (from the next start); kept in the user's
  application data (`settings.json`, or `TERRAFLUX_SETTINGS`).

### Changed
- A node added after the one selected takes its value in its first input that fits it when it must have none
  (a plot's map).

### Fixed
- Texts of some operators had runs of spaces in them (the doc of `view.plot`, `metrics.window`,
  `metrics.signatures`, `demand.regional`, `table.of`, `submodel`...).

## [0.22.0] - 2026-10-13

### Added
- Basemaps fetched and drawn anew: the web tiles of the service fetched by a pool of twelve threads shared by
  every view (each tile once, kept on disk and decoded in memory), and drawn by the GPU as textures on meshes whose
  vertices are carried to the map's coordinate system once per tile: nothing is warped on the processor, a tile
  shows as it comes, two levels coarser first, the coarser ones kept under those still to come. GDAL's HTTP
  (`terraflux_gdal::fetch`), image decoding (`decode_image`, palettes too) and coordinate transformations
  (`Transform`).
- The operators in a drawer at the window's left edge (out while the pointer is at the edge or over it) and in
  the toolbar (Operators); the node's and the Prepare panes at the left.
- Tabs dragged out of the dock become windows of their own (on another screen too); ctrl+W closes the tab in
  front of the window it is pressed in, and each window takes its own keys (F5, ctrl+S, ctrl+O).
- ctrl+O opens a raster or vector features (a node that reads the file, by its extension); vector.read and
  vector.write choose their file in a dialog, and their paths are kept relative to the model as the rasters'.
- Backspace deletes the node selected (as delete; the key macOS deletes with).
- The vector layers' line width (the points' size) beside their fill.
- Charts zoom with the wheel around the point under the pointer, as maps.

### Changed
- The attributes' table of vectors is only the node's table output (no longer repeated beside the features).

## [0.21.0] - 2026-10-13

### Added
- The Prepare pane (beside the node's): the model's reference grid (the land use map's suggested, or the node
  selected), the data of the node selected next to it (coordinate system, cells, extent and what differs; classes,
  values or counts) and the steps a click away, each a node of an operator already there with a note saying why:
  onto the grid (`raster.match`, resampled by what the values are), reprojected (`raster.reproject`), the slope
  (`raster.terrain`), the distance to a class (`dynamic.distance_to`); of vectors the distance to them
  (`vector.distance`), burnt into the grid (`vector.rasterize`, by an attribute or not), the statistics of the
  grid's map in each polygon (`vector.zonal`); every raster read off the grid put on it at once. The reference
  grid of a model opened is the one its steps put maps on (`terraflux_app::prepare`).

## [0.20.0] - 2026-10-13

### Added
- Out of core, the rest of the landscape metrics: `metrics.window` by strips of rows with the window's half side
  above and below (every cell's window whole, so the same maps to the bit), of rasters on disk into a GeoTIFF of a
  band per metric (`path`); `metrics.sample` reads each plot's window alone from the rasters on disk (and the map's
  classes for `all_classes` by blocks); `metrics.series` and `metrics.signatures` take rasters on disk, a layer
  per date, whole maps by blocks of rows and zones window by window. `block_rows` on all of them, and rasters on
  disk of more than 250 million cells always; the same tables as in memory, to the bit. `vector.grid_zones` reads
  only the grid of its map.
- Vectors over rasters larger than memory: `vector.zonal` by strips of rows (the zones that fit a strip by
  landschaft's zonal statistics on it, a zone too large read run after run and summed as landschaft sums it, its
  order statistics from its values or, for whole numbers, their counts), `vector.extract` from the strips of rows
  that hold points (landschaft's extract of a block, a row around for bilinear), `vector.rasterize` into a GeoTIFF
  strip by strip (`path`; polygons by their runs, lines by GDAL in the grid's pixels, points by their cell). The
  same values as in memory, to the bit, but GDAL's lines through cells' corners with `all_touched`.
- `RowsWriter` writes several bands side by side (`create_bands`, `push_bands`).

### Changed
- `metrics.window`'s maps, `vector.rasterize`'s map and the maps of `metrics.series`, `metrics.signatures`,
  `metrics.sample`, `vector.zonal` and `vector.extract` are of the type `Raster` (a map, a stack or rasters on
  disk): any map connects as before.

## [0.19.0] - 2026-10-13

### Added
- Vector layers over the maps in the viewer ("Vectors over the map": any vector of the run, reprojected to the
  map, in a colour or coloured by an attribute, polygons filled as asked), and vectors alone drawn the same way
  over a grid that spans them (with a basemap under them). The layers are drawn as tiles threads rasterize
  (tiny-skia, antialiased) from an R-tree of the features made once per layer on a thread of its own, vertices
  closer than a pixel skipped and features smaller than one drawn as dots: the screen only draws textures, so
  layers of millions of features move as smoothly as a map (a quarter of a million polygons indexed in 60 ms and
  the whole drawn in 90 ms, off the screen's thread). The feature under the pointer is found by the tree and
  named with its attributes.

- `vector.extract`: the values of a map or a stack at points, as landschaft's extract (the cell or bilinear, to
  the bit), or the statistics of the cells within a radius of them.
- `vector.buffer`, `vector.clip` and `vector.dissolve`: buffers of features (or one of them all), features cut to a
  mask's polygons, features merged all or by an attribute, by GEOS through GDAL; the same features and areas as
  geopandas (`tools/make_vector_ops_reference.py`).

- The colours of a map chosen by hand in the viewer (Colours: viridis, centred on 0, terrain, grey, red to green,
  rank, each on a scale of logarithms or not), remembered for that output.
- Circuit currents (current maps, flow potential) are shown on a scale of logarithms, as landschaft's plot.
- What a map is travels with it: raster.write writes the product of GuidosToolbox a map is (MSPA, FAD, SPA...) as
  `landschaft_product` in the GeoTIFF's metadata, as landschaft does, and rasters on disk that say so are shown
  as that product whatever their name (`product` sets it by hand); MSPA by tiles writes it too. The dataset's
  metadata items are read and written with the GeoTIFF (`GeoTags::metadata`).
- Models saved by landschaft with a potential Terraflux has no form for (a scikit-learn estimator, a
  multi-criteria evaluation, MOLUSCE's weights) run as `ls.simulate` runs them: landschaft writes their
  predictions on the starting map (`stack.classes`). A test case of the bridge (`tests/data/bridge/estimator`).
- Regional demands (`demand.regional`, a new type `Regional`): the map of region ids and each region's demand
  (cells per class and step, or a Markov chain). `lulcc.step` allocates region by region in the order of their
  ids with the one generator, `plus.demand` makes CARS's table of each region from its own cells and `cars.step`
  meets them all, `dinamica.step` runs each region with its own rates, its cells only and a seed of its own;
  cells of no region keep their class. As landschaft's `Regional`, bit for bit (three test cases of the bridge).
- `dynamic.distance_to` takes a transition of conductances (`cost`, a layer per direction, and its `layout`):
  the accumulated cost from the nearest cell of the classes (gdistance's accCost, through landschaft's
  `costcore`, now linked), as landschaft's `distance_to(..., cost=...)`.
- MOLUSCE's simulator: `rng.numpy` (numpy's legacy generator, as MOLUSCE draws), `molusce.total` (the total
  weight of each transition from MOLUSCE's weights of evidence and the drivers' categories) and `molusce.step`
  (the transition of largest weight, or the class of largest potential, with its confidence; the cells of each
  observed transition with the highest confidence change), through landschaft's C++ (`sim/molusce.cpp`). Models
  saved by landschaft with MOLUSCE run bit for bit (three test cases of the bridge).
- CLUMondo: `clumondo.ages` (the ages at the start: a map, or CLUMondo's random ages from the seed) and
  `clumondo.step` (one year: the suitability from logistic regressions in floats or given maps, neighbourhood
  functions, location preferences, then the allocation on the demand for goods and services, through
  landschaft's C++ of CLUMondo, with the 2017 executable's extensions and the land system roles' caps and pair
  resistances). Its `stop` ends the loop at a year without solution, as CLUMondo.
- FUTURES: `futures.simulation` runs r.futures.simulation as landschaft's `PatchGrowing` (subregions, potential
  subregions, patch libraries, development pressure, weights, zoning and its effects, incentives, restrictions)
  with its flood submodel (depths or flood probabilities and HAND, damage curves, response, adaptation,
  redistribution), every step in one run of landschaft's C++. A test case of the bridge.
- `control.repeat` takes `until` in its body: a reference to a yes-or-no value; when it is true at the end of a
  step the loop ends there, that step neither collected nor carried.

- The builder without the mouse: with the model's canvas in focus and the pointer over it the arrows go to the node
  beside the one selected (the first, top left, when none is; the view follows), enter shows its output in the
  viewer; with "/" to add operators, a model is built from the keyboard.
- Templates of Dinamica EGO's classic model and of CLUE-S (lulcc's), on two dates of land use drawn by chance,
  calibrated and run three steps ahead as soon as they open.
- Templates of one's own: a model saved as a template (Templates, "Save this model as a template"; its files'
  paths made absolute) and listed under Yours, opened as a new model, removed with its x.

### Changed
- A continuous map's ramp spans its values' 2nd to 98th percentiles (98th of the absolute values centred on 0),
  as landschaft's plot, no longer their least and most: a few outliers no longer wash it out.
- `vector.zonal`'s statistics are worked out by a function the extraction in buffers shares.
- The vector panel works out a layer's table, counts and grid once, not at every frame.
- `dinamica::step` takes `Rates::One(&markov)` or `Rates::Regional(&regional)`; `lulcc::step` takes
  `Demands::One` or `Demands::Regional`, `plus::step` `CarsDemand::One` or `CarsDemand::Regional`. The `demand`
  inputs of `lulcc.step` and `cars.step` and `rates` of `dinamica.step` are optional (or `regional`).

## [0.18.0] - 2026-10-13

### Added
- `vector.join`: a table's columns as attributes of features, by a key (the feature's number or an attribute):
  the clusters of zones (metrics.cluster) on their squares (vector.grid_zones), burnt into a map of the groups by
  vector.rasterize.
- Charts of tables: box plots of the columns by group (landschaft's plot_cluster_metrics), and metrics.pca's
  loadings as the circle of correlations (every metric an arrow to its correlations with two components).
- `metrics.patch` of rasters on disk by blocks writes the patches to a GeoTIFF block by block (`patches_path`),
  never whole in memory; the same patches as the map in memory.

### Changed
- MSPA by tiles writes its GeoTIFF through the same writer of rows in order (`files::RowsWriter`).

## [0.17.0] - 2026-10-13

### Added
- Spatial signatures, as pylandstats' SpatialSignatureAnalysis in landschaft (`tools/make_signature_reference.py`):
  - `metrics.signatures`: the table of landscapes described by their metrics, a row per zone of a map (polygons,
    buffers around points, a grid of squares) or per layer of a stack, a column per landscape metric and per
    class metric and class (`pland_11`...), to the bit;
  - `metrics.pca` (the principal components of the z-scores: scores, loadings and the share of the variance),
    `metrics.nmf` (non-negative factors: NNDSVDa start, HALS sweeps), `metrics.cluster` (k-means from numpy's
    generator, PCG64 and its SeedSequence, so the same starts; or scipy's linkages: ward, average, complete,
    single, centroid, median, weighted, cut as fcluster's maxclust) and `metrics.clustergram` (the clusters for a
    range of k and their means, on the first principal axis or not). The same clusters as landschaft, label by
    label; the components, factors and means within 1e-11 of numpy's LAPACK.
- The landscape and class metrics of a signatures node are chosen in the metrics' list, as those of the other
  metric nodes.
- Landscape metrics of maps larger than memory: `metrics.patch`, `metrics.class` and `metrics.landscape` read a
  map by blocks of rows (`block_rows`, a multiple of 64) through landschaft's out-of-core landscape (its C++
  `TiledLandscape`: the patches joined across the seams), the same tables to the bit as the map in memory; rasters
  on disk of more than 250 million cells always are. prox, simi, econ, connect (and dcore by FRAGSTATS' convention)
  need the whole map, as in landschaft; chosen by default or by type they are left out.
- `landscape.mspa_tiled`: GuidosToolbox's MSPA Tiling, as landschaft's mspa_tiled, the same maps
  (`tools/make_mspa_tiled_reference.py`): MSPA by overlapping tiles with GTB's shortcuts, of a map in memory, or of
  rasters on disk read by windows and written tile by tile to a GeoTIFF (GTB's name next to the input by default).

- The Windows installer and a portable zip (`tools/make-installer.ps1`, in `target/installer`): Terraflux in
  Program Files with a Start menu shortcut and the PATH, GDAL in the folder `gdal`, the Visual C++ and OpenMP
  runtimes next to the program (a clean Windows has neither). The MSI is built by WiX 5, a .NET tool of the
  repository (`.config/dotnet-tools.json`).

- Tables drawn as charts of several metrics at once in the viewer: bars, lines or points of some columns (a panel
  each) along another, the rows split by a column (a class, a zone, a cluster) in colours, as landschaft's
  plot_metric and plot_cluster_metrics; the clustergram of `metrics.clustergram`'s means (landschaft's
  plot_clustergram); a correlation matrix (`metrics.correlation`) as a heatmap.

- Planning units of polygons with management zones in `prioritize.optimize` (landschaft's polygon_zone_units): a
  cost column per zone, every zone's own features or the base ones with contributions, summed over the polygons
  exactly; prioritizr's compile() of its case to the bit, and its optimum.
- `prioritize.h3_units`: H3 hexagons as planning units, as landschaft's h3_units (h3o, a port of Uber's H3): the
  hexagons of a resolution over a cost map, in its coordinate system, with the exact sums of the costs under them;
  the same hexagons and costs to the bit (`tools/make_h3_reference.py`).

### Changed
- A table's chart (formerly the bars of one column) is chosen in the viewer by its kind, its columns, the column
  along x and the one that splits the rows; a series suggests the date along x and the class as groups.
- The map input of `metrics.patch`, `metrics.class` and `metrics.landscape` takes rasters on disk as they are
  (read by blocks when large or asked to); the maps of patches and metrics are given for maps in memory only.

## [0.16.0] - 2026-10-12

### Added
- Settings of a few values are chosen in a list (the operators' settings say their values, `PortSpec::choices`;
  57 of them), yes-or-no settings too, with their default; the metrics of a landscape metrics node in a list of
  their own: all (the default), by type, or one by one.
- Tab goes to the model's tab and Tab again back to the tab it came from; ctrl+W closes the tab in focus; ctrl+R
  renames the node selected (a box over it).
- A connection is taken off by dragging it away from its input (onto another input: moved) or by a right click
  on the input.
- The viewer writes the coordinates under the pointer in its corner (and the cell's row, column and value), for
  maps and vector features.

### Changed
- The library opens with its subjects closed but IO (reading and writing rasters and vectors, a subject of its
  own) and Plot (formerly Visualization); "Data & configuration" is "Data preparation".
- The model as text is a button of the model's tab (Text), no longer a tab of its own.

## [0.15.0] - 2026-10-12

### Added
- `metrics.sample`: landscape metrics of sample plots, as landscapemetrics' sample_lsm and scale_sample: square or
  circular buffers (R's 100 vertices, its x87 sines) of one or several sizes around points, or polygons; each plot
  cut as terra's crop(mask = TRUE) (GDAL's all-touched cells), the share of it inside the map (geodesic areas
  when the plots' coordinate system is known, by GDAL 3.9's GeographicLib), every class of the map with
  `all_classes`; the plots as features. The 9 cases of landscapemetrics that landschaft keeps, the same rows and
  bits.
- `vector.grid_zones`: square zones tiling a map (landschaft's grid_zones).
- `metrics.series`: landscape and class metrics along a series of maps (a layer per date) as pylandstats'
  SpatioTemporalAnalysis in landschaft, of the whole maps, of zones, of buffers or of a grid; absent classes 0 for
  the counts and areas (`fillna`). The same values as landschaft (`tools/make_lsm_reference.py`).
- `metrics.correlation`: R's cor (Pearson, Spearman, Kendall) between the metrics of a table, as
  landscapemetrics' calculate_correlation, to the bit.

### Fixed
- The EPSG code of a GeoTIFF whose projection is its own (user-defined) was that of its datum (e.g. 4326 for an
  Albers on WGS 84); it has none now.

## [0.14.0] - 2026-10-12

### Added
- The rest of GuidosToolbox, by landschaft's C++ of GTB, the same maps and reports as landschaft
  (`tools/make_gtb_reference.py`):
  - the habitat network of an MSPA map (`networks.gtn_components`, `networks.gtn_importance`: GTN's components,
    ECA and coherence, the dPC importance of every link and stepping-stone node; `networks.conefor`: Conefor
    Sensinode's node and link tables), with GTB's colours and reports;
  - the Restoration Planner: the habitat's status (`restoration.status`: ECA, coherence, the restoration potential
    and RAC), a resistance from the distance to the habitat (`restoration.resistance`), the cost distance
    (`restoration.cost_distance`), the least-cost path and the corridor between two objects with its gain in ECA
    and coherence (`restoration.least_cost_path`, `restoration.corridor`), the isochrones from one object or
    between two (`restoration.isochrone`);
  - the zones of influence of the objects and their proximity (`distance.influence_zones`,
    `distance.objects_proximity`, experimental as in landschaft);
  - the change of FAD at several scales between two dates (`change.fad`: the cells moved between the classes at
    every window, the change of FAD_AV and AVCON);
  - the landscape mosaic's heatmap and triangle (`fragmentation.landscape_mosaic`'s `heatmap`, `triangle` and
    `heatmap_csv`) and its change between two dates (`change.mosaic`);
  - GTB's text reports of FOS (fos.txt), SPA (spaN.txt) and Accounting (acc.txt) as a `report` output, with the
    cell size the report converts to hectares (`pixel_resolution`); Accounting's three largest objects come from
    the C++.

## [0.13.0] - 2026-10-12

### Added
- Vector features in the model: a value type (`Vector`: geometries in 2-D, attributes as numbers or texts, the
  coordinate system), read and written by OGR (`vector.read`, `vector.write`; GeoPackage, Shapefile, GeoJSON,
  FlatGeobuf, KML...; a model's vector outputs written as GeoPackage), reprojected by PROJ where they meet a map
  of another coordinate system.
- `vector.rasterize`: features burnt into a map's grid as landschaft's rasterize, the same cells: polygons by
  landschaft's C++ port of GDAL's rasterizer (centre rule or all touched), lines by GDAL, points by their cell;
  an attribute or a value, replaced or added up, a fill.
- `vector.zonal`: a map's (or every layer of a stack's) statistics in every polygon as landschaft's zonal_stats
  (count, sum, mean, min, max, range, std, var, median, majority, minority, unique, percentiles).
- `vector.exact_sum`: the coverage-weighted sums over polygons of exactextractr's exact_extract, bit for bit
  (landschaft's port of exactextract).
- `vector.distance`: the exact Euclidean distance from every cell to the features.
- Planning units of polygons in `prioritize.optimize`, `evaluate` and `calibrate` (`polygons`, `cost_column`):
  the features' amounts exactextractr's sums, the boundary penalties on the polygons' shared paths and the
  contiguity on the polygons that touch; prioritizr's models bit for bit, its boundary and adjacency matrices of
  polygons exactly (`tools/make_vector_inputs.py` writes the cases' polygons).
- The viewer draws vector features (polygons, lines, points; zoom and pan, the feature under the pointer named)
  beside their attributes, and saves them as GeoPackage, GeoJSON or Shapefile; "Open vectors..." in the builder.
- The build links landschaft's io library (zones of polygons, exact sums).

## [0.12.0] - 2026-10-12

### Added
- The minimum extinction risk objective in `prioritize.optimize` (`min_extinction_risk`, landschaft's extension of
  prioritizr): within the budget, the least sum of the features' species-area risk 1 - (held / total)^z (`z`
  one or per feature), piecewise linear through `segments` + 1 points of the curve; feature weights, every
  decision type, management zones (every feature over all its zones); the risks of the solution
  (`extinction_risk`). `prioritize.evaluate`'s `extinction_z`: the features' exact risk and their sum. landschaft's
  models bit for bit, its optima and risks (`tools/make_extinction_reference.py`); the optimum against
  enumeration.

## [0.11.0] - 2026-10-12

### Added
- Zonation's transforms in `prioritize.zonation`, through landschaft's C++ quantized data (now kept between its
  steps: `prio_data_*` in `terraflux-ls-sys`): conditions (`condition`, `renormalize`, `condition_maps`; -c),
  retention (a fraction everywhere, fractions or amounts per cell; -r), the pre-processing maps (`preprocessing`:
  richness and range-size rarity of the weight classes and groups, weighted; -b) and the features as ranked
  (`transformed`; -f).
- Zonation's connectivity transforms: single-feature smoothing with a dispersal kernel (uniform, triangular,
  negexp, gaussian; times itself; -d), interactions with resources, positive or negative, before and after the
  matrices (-i, -I), connectivity matrices (-m); Zonation's FFT convolution in blocks, in Rust.
- Zonation's impact assessment: impact sources with footprints and rings (-s) and cumulative impacts (direct
  habitat loss and pressures through response functions, -e), per subarea, source, zone and value bin; the
  features lose what the scenarios take before the ranking (`impact_groups`, `impact_bins`, `impact`,
  `cumulative_impact`, `impact_maps`; the tables `impact_zones` and `impact_cumulative`, the maps of the groups).
- Zonation's post-processing of the ranking: statistics of areas (external, a range of ranks, its separate
  8-connected areas) with the coverage of every feature, the comparison with another solution, the histogram of
  the features' coverage and the flexibility map.
- All of landschaft's Zonation cases: the 24 of the transform, connectivity, post-processing and impact groups
  now with the 41 of the core, the ranks bit for bit with the z5 binary, the transformed and pre-processing maps,
  the impact maps and the post-processing tables (word for word) too (`tools/make_zonation_inputs.py`).

## [0.10.0] - 2026-10-12

### Added
- prioritizr's systematic conservation planning (`prioritize.optimize`): the planning units of a cost raster and
  the features' amounts in them, the objectives min set, min shortfall, min largest shortfall, max utility, max
  features, max cover and min penalties, relative, absolute, loglinear and manual targets, feature weights,
  binary, proportion and semicontinuous decisions, locked units; boundary penalties (prioritizr's boundary
  matrix of the raster, an edge factor), linear penalties and constraints (a map), neighbour constraints; the model
  compiled in Rust (`terraflux_ops::milp`,
  a port of landschaft's), solved by HiGHS through landschaft's C++ (its covering and wide engines); the solution
  on the grid, a summary and the targets' coverage. The models are prioritizr 8.0.6's compile(), bit for bit, on
  the 25 raster cases landschaft keeps, and HiGHS reaches their optima (Gurobi's at gap 0)
  (`tools/make_optimize_inputs.py`).
- Planning units in tables for `prioritize.optimize`: a table of units (a cost column, the features' columns,
  locked units' columns; units without cost dropped) or Marxan's (pu, spec, rij); boundary, connectivity and
  asymmetric connectivity matrices and adjacency matrices as tables (i, j, x); linear penalties and constraints of
  a column; penalties and constraints in the order given (`spatial`); the selection per unit as a table. The
  models are prioritizr's compile(), bit for bit, on landschaft's 12 table cases.
- Portfolios of solutions for `prioritize.optimize` (`portfolio`): cuts (the best distinct solutions, prioritizr's
  no-good cut fixed), top (each to optimality), gap (within `pool_gap` of the optimum) and shuffle (the columns in
  random orders); a column per solution and the frequency of selection, a row per solution in the summary, and
  with maps a layer per solution and the frequency map. The objectives of prioritizr's (Gurobi's solution pool)
  on the cases landschaft keeps.
- `prioritize.evaluate` of table units and of `prioritize.optimize`'s selection (a table): the boundary of a
  boundary matrix, the connectivity and asymmetric connectivity of connectivity matrices (prioritizr's warnings
  when a matrix does not suit its summary), the per-unit results as a table (`units`: the solution, the rarity,
  Ferrier's, the replacement and rank importance). prioritizr's numbers bit for bit on landschaft's table case,
  and its replacement importance of a max utility problem with boundary penalties.
- Contiguity constraints for `prioritize.optimize` (`contiguity`, `feature_contiguity`: the selected units, or every
  feature's units, form one patch; the raster's 4 neighbours or an adjacency matrix): landschaft's exact
  formulation (a spanning arborescence with flows), not prioritizr's, which misses patches (it can find a worse
  one, or none); solved in three steps: a first patch from prioritizr's restriction, directed cuts separated by
  maximum flows on the linear relaxation, then the MIP with the cuts. With prioritizr's restriction it reaches
  prioritizr's optima (Gurobi's at gap 0) on landschaft's four cases; the exact model finds the best patch of a
  grid by enumeration.
- Manual bounds for `prioritize.optimize` (prioritizr's add_manual_bounded_constraints and
  add_manual_locked_constraints): tables of units and their bounds (`bounds`: pu, lower, upper) or statuses
  (`status`: pu, status), after the locked units; prioritizr's compile() bit for bit and its optima on the 7 cases
  landschaft keeps.
- The phylogenetic objectives of `prioritize.optimize`: max_phylo_div and max_phylo_end (a phylogeny in Newick,
  its tips the features; the branches met where a target below them is). prioritizr's compile() bit for bit (with
  its tip-order bug fixed when the tips come in another order) and its optima on landschaft's 4 cases.
- prioritizr's calibration of trade-offs (`prioritize.calibrate`): the blended approach (a range of penalty
  weights, by default around Cohon et al.'s penalty), the hierarchical one (the least penalty within cost
  thresholds above the least cost), the Pareto front of cost and shortfall, and Cohon's penalty; boundary,
  connectivity or linear penalties; every candidate's cost and criterion, and the candidates TOPSIS and the knee of
  the curve choose; the solves in parallel. prioritizr's optima, thresholds and least cost on landschaft's 3 cases.
- Management zones for `prioritize.optimize` (prioritizr's zones): planning units that go to at most one zone (or
  exactly one, `mandatory_allocation`), a cost per zone (`zone_costs`, a layer per zone; or a cost column per
  zone), every zone's own features (`zone_features`, prioritizr's zones(); a mapping of columns) or the base
  features with every zone's contribution (a number, one per feature, a raster or a column), Marxan's rij with a
  zone column; targets summed over the zones, per zone (a features x zones matrix) or by hand with lists of zones
  (`manual_targets`); every objective, with one budget or one per zone; weights per target or per feature and
  zone; locked units and manual bounds per zone; the solution as the zone of every unit and a layer per zone. The
  models are prioritizr's compile() bit for bit on landschaft's 23 zone cases, solved on landschaft's zones
  engines to prioritizr's optima.
- Spatial penalties and constraints with zones: boundary penalties (edge factors per zone, a zones matrix of how much
  a boundary between two zones counts), connectivity and asymmetric connectivity penalties with zones matrices,
  linear penalties and constraints with data per zone (`zone_penalty_data`, `zone_constraint_data`), neighbour
  constraints with k per zone, and contiguity per cluster of zones (landschaft's exact formulation). prioritizr's
  compile() bit for bit on landschaft's 10 cases.
- `prioritize.evaluate` with zones (`zone_solution`, a layer per zone, or the selection): the units, cost,
  boundary, connectivity and asymmetric connectivity overall and per zone, the features' representation and the
  targets' coverage (with lists of zones). prioritizr's numbers bit for bit on landschaft's 4 cases.
- Multi-action planning as prioriactions 0.5.0 (`prioritize.actions`), through landschaft's C++: which units to
  monitor and which actions against which threats to take in them, at the least cost for the features' recovery
  and conservation targets (minimizeCosts) or for the most benefit within a budget (maximizeBenefits); the
  boundary of the units and of every threat's actions, linear, quadratic or cubic responses; prioriactions's
  inputData() checks, defaults and warnings; the monitoring, the actions, getActions(), the cost, the benefit
  (total and per unit), the potential benefit and the connectivity. prioriactions's model bit for bit, its optima
  and its figures on landschaft's 6 cases (`tools/make_optimize_inputs.py` writes their tables).
- Restoration planning as plangea (`restoration.plan`): how much of every converted cell to restore for the most
  benefit within area targets (in the landscape or per region, absolute or shares) and a budget: the reduction of
  the species' extinction risk (the species-area relationship on `sdm.area_of_habitat`'s table and gain; z and a
  restoration discount per species) and linear benefits (carbon per km²). exact: the optimum within a tolerance
  by simplicial decomposition (a proven Frank-Wolfe bound; species at their potential lifted); steps: plangea's
  procedure (the target in steps, the benefits recomputed between them); uniform. plangea's units, bounds,
  biodiversity layers and plans of its steps on the cases landschaft keeps.
- The reduction of the extinction risk a plan brings and its uncertainty from z (plangea's multi_exrisk_aggregate:
  correlated, in quadrature or linear), in `restoration.plan`'s summary and species table; plangea's numbers.
- The ecosystems' risk of collapse: `restoration.ecosystem_areas` (every ecoregion's natural area now and before
  conversion, and what restoring a cell gives it; plangea's ec over all the ecoregion's land, the corrected form)
  and `restoration.ecosystem_targets` (a restoration target shared among the ecosystems for the least risk:
  plangea's steps, or the exact optimum). plangea's layers and targets on the cases landschaft keeps.
- Conservation and conversion plans (`restoration.plan`'s `conservable`, `convertible` and `threat`; plangea's C,
  D and R + D): conserving keeps the habitat the baseline loses, converting takes it away (the habitat lost per
  unit of natural land: `sdm.area_of_habitat`'s new `loss`); the area targets of the net change with
  restoration and conversion; the maps conserved and converted. plangea's units, layers and plans of its steps
  on the 4 cases landschaft keeps, and their exact plans.
- Trade-off curves in `restoration.plan` (`levels`: the landscape target growing; plangea's gradient steps, or
  exact plans nested or on the frontier) with the gradient (the levels acting on every cell), and projected
  changes of natural land per region (`projected_change`; plangea's econometric targets, compensated as
  harmonize_sr_sparable). plangea's plans and spared land on the cases landschaft keeps.
- The intactness benefit in `restoration.plan` (`hfi`, `reduction`, `increase`; landschaft's
  add_intactness_benefit): the total ecological intactness a plan gains, for restoration, conservation and
  conversion. Linearised at the plan of nothing (restoration.intactness_gradient, to the bit; recomputed between
  plangea's steps) or exact (`intactness_form`: the total intactness itself, not concave: successive concave
  minorants to a local optimum, bounded by the secants of the pairs through landschaft's C++
  `intactness_secant`).
- The cheapest plan for a benefit (`restoration.plan`'s `min_benefit`, landschaft's min-cost objective): regula
  falsi on the budget, each step the most benefit for it, the cost proven by the budgets that fall short.
- Sets of weights compared (`restoration.plan`'s `weights`, plangea's post_choose_weights): a plan per set, the
  benefits and cost rescaled over the sets and ranked by mean over deviation; the table and the best set's plan.
  landschaft's plans, costs and tables on the bd_absent problem (`tools/make_plangea_plan_reference.py`).
- `table.of`: a table written in the settings (the small tables of a planning problem).
- Templates: Restoration plan (plangea) and Multi-action planning (prioriactions).
- prioritizr's evaluation of a solution (`prioritize.evaluate`): the units selected, their cost and boundary, the
  features' representation, the targets' coverage, Ferrier et al.'s irreplaceability and the rarity-weighted
  richness; prioritizr's numbers bit for bit on the cases landschaft keeps; and on request the replacement
  importance (a solve without each unit selected, in parallel) and the rank importance (incremental solutions
  over budgets), prioritizr's to 1e-9.
- Template: Conservation planning (prioritizr).
- Marxan 4 (`prioritize.marxan`) through landschaft's C++: from Marxan's tables (pu, spec, puvspr, bound) or from
  maps (every cell a planning unit, its shared edges its boundaries), every option of input.dat; every run's
  reserve, the best, the selection frequency, the summary and the penalties. The same runs as Marxan 4.0.6 on the
  8 cases landschaft keeps (`tools/make_marxan_inputs.py`).
- ProxIRR irreplaceability (`prioritize.proxirr`): alpha, beta and gamma from Marxan's tables or maps, R's
  proxirr 0.5's bits (`tools/make_proxirr_inputs.py`).
- Template: Marxan and irreplaceability.
- Zonation 5's priority ranking (`prioritize.zonation`) through landschaft's C++: every rule (CAZ1, CAZ2, CAZMAX,
  ABF, CAZP, CAZPW, RAND, LOAD), weights and weight groups, thresholds, value extraction, the zero modes, 8 or 16
  bits, costs, hierarchic masks, analysis areas, the iterations' settings; the rank map, the performance curves,
  the features' table, the AUC. The same rank maps, bit for bit, and iterations as the z5 binary on the 41 cases
  landschaft keeps (`tools/make_zonation_inputs.py` writes the rasters they were made from).
- The rank in Zonation's ramp (dark to red), for the maps named rank or priority.
- `stack.of`: maps as one stack, a layer per input (the features of a prioritization).
- Template: Zonation.
- `.npy` files in Fortran order, and of 16-bit and unsigned 32-bit integers.
- Restoration planning's inputs after plangea, through landschaft's C++, plangea's bits: ecological intactness
  (Beyer et al. 2020) of a human footprint, before and after restoring cells (`restoration.intactness`), its
  gradient (`restoration.intactness_gradient`), the original vegetation of converted land
  (`restoration.original_vegetation`: past cover, ecoregions, failsafe shares) and the area of habitat of species
  (`sdm.area_of_habitat`: suitable classes, elevation limits, the habitat gained per cell restored). Tested against
  the plangea outputs landschaft keeps (`tools/make_plangea_inputs.py` writes the maps they were made from).
- Template: Ecological intactness.
- `GeoTags::is_geographic`, `linear_unit` and `geokey`.
- The build links landschaft's prioritization (Zonation), restoration planning and MILP (HiGHS) libraries
  (landschaft's new `LANDSCHAFT_MILP` option).

### Fixed
- Numbers in models and settings are read to the bit (serde_json's float_roundtrip): a decimal could become the
  double next to its own (the comparisons with landschaft's JSON references are now exact too).
- Outputs given on request or for some inputs only (`prioritize.evaluate`'s replacement, rank, coverage and
  Ferrier's; `prioritize.optimize`'s maps with tables) no longer fail the run: an operator's optional outputs may
  be missing.
- Closing a pane, or the maps of a past session, no longer leaves an empty floating window that panics when
  drawn (egui_dock's retain_tabs): such windows are removed.

## [0.9.0] - 2026-10-11

### Added
- Landscape metrics, landscapemetrics' and FRAGSTATS', through landschaft's C++: `metrics.patch` (a row per
  patch, the map of the patches and of a metric), `metrics.class` (a row per class, the map of a metric),
  `metrics.landscape` and `metrics.window` (the landscape's metrics in a moving window, square or circular, a map
  each). The metrics by name (`ca`, `lsm_c_ca`), by type, or every one of the level, FRAGSTATS' extras when asked;
  every option of landscapemetrics (directions, boundaries, edge depth, complexity) and FRAGSTATS' (convention,
  search radius, contrast and similarity weights). The catalogue of the 256 metrics comes from landschaft
  (`tools/make_lsm_catalog.py`).
- Tested against the R package landscapemetrics: every value landschaft keeps of its calculate_lsm (14 rasters, 5
  sets of options, more than 100,000 values) and of window_lsm.
- Tables in the viewer: as many rows as they have (those in view laid out), sorted by a click on a column, filtered
  by words or a condition (`area > 1`, `np != 1`, `ed = NA`), and a column drawn as bars by class or patch.
- Template: Landscape metrics (per class with the share as a map, the landscape's diversity, a moving window).
- More of GuidosToolbox through landschaft's C++: `fragmentation.contagion` (Pff) and `fragmentation.clustering`
  (FAC) in a moving window, `fragmentation.entropy` (the map's spatial joint entropy) and
  `fragmentation.entropy_map`, `fragmentation.landscape_mosaic` (19 or 103 classes, from a map coded so or the
  classes of agriculture, natural and developed land given), `fragmentation.gray_convolution` (GraySpatCon's ten
  metrics of a % cover), `landscape.parcellation` (APS, AWAPS, division, PARC per class and of the map),
  `landscape.contortion` (and GTB's display of it), `distance.euclidean` (signed, with GTB's byte map),
  `change.simple`, `change.morphological` and `change.fos`.
- Their tests: the landscape mosaic of 103 classes against GTB's own outputs (the digests landschaft keeps), the
  others against landschaft's (`tools/make_gtb_reference.py` writes `tests/data/reference/gtb_landschaft.json`):
  every map bit for bit, the reports' numbers to their last bits.
- GTB's colour tables and names for them (`tools/make_gtb_palettes.py` writes `terraflux_ops::gtb_palettes` from
  landschaft): the mosaic's 19 classes, change, FOS change, entropy, distance and contortion; ramps in named bands
  in the legend (10-19%..., distances in cells, the contortion in fifths).
- Template: Landscape mosaic (the mosaic of a land cover, the contagion and entropy of its natural land).

### Changed
- The library shows the operator's name in one line and its prefix beside it (long names broke in two).
- A legend puts the values of the same name in one line whatever their colours (the bands of a ramp).

## [0.8.0] - 2026-10-11

### Added
- The rest of GuidosToolbox's main tools, through landschaft's port (its C++, as MSPA), the same bits as GTB's
  outputs that landschaft keeps (`tools/make_gtb_inputs.py` writes the maps they were made from):
  `landscape.spa` (SPA2, 3, 5 and 6, the objects counted and the porosity), `landscape.accounting` (the
  objects in size classes: GTB's batch classes, those given or "auto"; the objects' ids and areas as maps),
  `fragmentation.fad` and `fragmentation.fad_multiscale` (the density at each window, their average, its
  classes, AVCON), `fragmentation.fos` (FAD, FED or FAC, 5 or 6 classes, binary maps or % covers, GTB's report:
  ECA, coherence). Every one takes the foreground's classes or a map in GTB's codes.
- Their maps in GTB's colours with the names of their values (`terraflux_ops::landscape::Product`), by the
  operator and port that made them or the names GTB gives its files; the legend of a density (FAD, FOS) in its
  classes, rare to intact, a line each.
- Templates: Fragmentation of a forest (FAD-MS and FOS), Patterns and objects (SPA6 and Accounting).
- `/` searches the operators from the keyboard: the arrows choose, enter adds the one chosen connected to the node
  selected, esc closes the search. The library's search ranks names before descriptions.
- Templates (the Templates menu): models that run as soon as they open, to see a flow and check it works: MSPA
  of a forest, MSPA of your map, a forest from a surface, MSPA's edge width, neutral landscapes.
- Neutral landscape models from landschaft's port of NLMR: `nlm.random_cluster`, `nlm.gaussian_field`,
  `nlm.fbm` (a size, or another map's grid; a seed).
- `raster.reclassify`'s rules as a table in the node: the classes of the map read with their share (A rule per
  class), or its range (4 equal ranges), and how to write the rules.
- Open a folder as a stack (the library, and A folder... on a `raster.read`).
- Colours by what the values are: a ramp centred on 0 for values on both sides of it; terrain, grey and
  red-to-green ramps for elevations, hillshades and NDVI (by the names of nodes, files and bands); MSPA's
  colours and names through `view.plot` and for files named so.
- GDAL, loaded when the program runs (`terraflux-gdal`: a few functions of its C API, no GDAL to build): every
  format it reads, read in windows at any resolution with a handle per thread; `raster.reproject` (gdalwarp: a
  CRS, a resolution, an extent, the method by the data), `raster.match` (landschaft's match_grid),
  `raster.clip` (gdal_translate); the CRS of rasters it read written to the files made. Found in the folder
  `gdal` next to the program (`tools/bundle-gdal.ps1`), in `TERRAFLUX_GDAL`, or installed.
- `raster.reclassify`: rules `1 2 3 -> 1`, `0..10 -> 2`, `* -> nan`.
- More of GDAL: `raster.resample`, `raster.convert` (formats, cell types), `raster.mosaic` (a series of tiles
  into one raster), `raster.terrain` (gdaldem), `raster.fill` (gdal_fillnodata), `distance.proximity`
  (gdal_proximity). A series may hold files of different grids (each layer on its file's grid; no stack of
  them).
- Several nodes at once: a box of selection, shift or ctrl and a click, ctrl+A, Esc; the nodes selected moved
  together and deleted together. Arrange (ctrl+L): columns by depth, each ordered by the rows of the nodes it
  reads.
- A double click on a node opens its output in a map tab of its own (run first if it has not run), beside the
  viewer; as many map tabs as one likes, each movable to another window.
- The View menu turns each pane on or off.
- `view.plot` (Visualization): its map tab opens by itself when it runs and is refreshed at every run; nothing
  is written. Save output, in the viewer and every map tab: the output to a file (GeoTIFF, CSV or JSON, in the
  background, the status bar telling when done), or to a file and a `raster.write` in the model.
- Nodes move anywhere on the canvas (no more limit at the top and the left).
- Basemaps in the viewer and the map tabs: Esri's imagery and topographic map, OpenStreetMap, OpenTopoMap and
  Carto's light map, by GDAL's WMS driver (tiles kept in the user's cache), warped onto the data's grid tile by
  tile in threads of their own (finer than the data's cells when zoomed in, around the data too); the data shown
  or hidden over it, at an opacity; the services' attribution written on the map
  (`terraflux_gdal::tile_service`, `warp_image`). GDAL's web requests use its certificates and say they are
  Terraflux's.
- Rasters of any size: `raster.read` reads headers only and gives rasters on disk (`Value::Raster`, type
  `raster`; a folder or a pattern gives a series), read whole only where a port takes a map or a stack (up to
  two billion cells); `raster.write` copies them in blocks. `terraflux_raster::image::Image` reads windows at
  any step, from overviews when the file has them, decoding only the chunks needed, in parallel, with a cache.
- The viewer draws maps from a pyramid of tiles made by threads (the tiles in view, at the screen's
  resolution; the coarser one shown until a tile comes): a raster of 7.3 billion cells shows in 0.4 s and a
  view 67 times closer completes in 0.3 s, frames under 10 ms.
- Panes in a dock (`egui_dock`): dragged, split, closed and opened again (Panels menu), or taken to a window of
  their own on another screen; the layout kept in the user's application data.
- The library follows landschaft's subjects (the sections of its reference), land change simulation grouped by
  model; the subjects still empty shown dimmed.
- The application opens in the model builder: a library of the operators by subject (with a search) whose
  operators come connected to the node selected, rasters opened and saved from it, and under the canvas what
  the node selected gives (a map with its legend, the maps of a stack or of a loop's steps, a table, a value).
  A run stops at the node shown (ctrl+enter, or by itself after every edit) and takes the rest from the cache;
  each node shows how it did (running, run, from the cache, failed); F5 runs the whole model, del deletes a
  node, ctrl+S saves. New models start empty (New), the wizard is a button (Wizard...).
- Runs that keep what the nodes gave (`Engine::run_with` and `Cache`): a node runs again only when it, a node it
  depends on, an input or a file it reads changed (its place on the canvas and its note do not count); a run
  may stop at a node (its ancestors only, and only their errors stop it); loops give back what they collected
  from the cache. Nodes that give a random generator, those after them, and `raster.write`, always run.
- `raster.read` (a band as a map, every band as a stack) and `raster.write` (a map, a stack or a loop's maps,
  in the smallest type that holds them): rasters read and written by nodes, their paths relative to the
  model's folder in the file and absolute once it is expanded, as plugins'.
- `landscape.mspa`: GuidosToolbox's Morphological Spatial Pattern Analysis through landschaft's port (the
  `gtbcore` library), its foreground the classes given or GTB's codes; shown in GTB's colours with the names
  of its classes. Tested bit for bit against the outputs of GTB's `mspa` executable that landschaft keeps.
- `terraflux_raster::npy::read_npz` and `DataType::fitting` (the cell type a raster is written in).
- The builder's canvas for large diagrams: drawn by the GPU (wgpu pipelines in egui's render pass: the boxes,
  ports and connections as instances, the curves and the smoothed edges in the shaders), uploaded only when the
  graph changes; a camera (drag the background or with the middle button, the wheel zooms around the pointer,
  Home fits the graph) instead of a scrolled area; the texts of the nodes in view only, and only when readable;
  a grid finds what is under the pointer; a node dragged moves alone in the scene. With 10,000 nodes a frame
  takes about 0.1 ms (it took 4.7 s with 2,000), dragging a node 0.5 ms, an edit about 30 ms.
- How frames are presented: one frame queued by default (the window follows the pointer more closely);
  `TERRAFLUX_PRESENT`, `TERRAFLUX_LATENCY` and wgpu's `WGPU_BACKEND` to try what is smoothest on a machine.
- Tests of the canvas with the pointer (select, move, connect, show an output, zoom) and with 5,000 nodes;
  timings with 100, 1,000 and 10,000 nodes (`cargo test --release -p terraflux-app --test canvas -- --ignored
  --nocapture`).
- The map tab shows the maps of the inputs too: a stack's one after the other (a slider, the left and right
  keys, play) with small images of all of them; maps of the land use's classes in its colours.

### Changed
- A double click on a node shows its map in its one tab: brought to the front when it is open already, a new one
  only once it is closed (it opened a tab at every double click).
- A folder or a pattern read by `raster.read` gives every band of every file (named `file band` when a file
  has more than one), as one stack when they share a grid; it gave the first band of each file.
- No more tabs at the top: the panes are arranged in the dock; the application opens with the model, the
  operators, the node and the viewer (the simulation's maps, the areas and the validation from the View menu).
- The library's groups (land change simulation's models) are closed until opened.
- The canvas: dragging the background selects; the view moves with the right or the middle button.
- The application's look: mid greys in flat panels, an orange accent for what is selected or on (after Ableton
  Live), a status bar with the cell under the pointer.
- The wizard has three steps: the data is checked as soon as it is chosen, in a thread, and what is wrong is
  shown below it (no step only to say that the data is right).
- A faster application: a model's data is read in a thread (the window answers at once); the area of each
  class is counted once per map, in the run's thread, instead of at every frame (the charts and the map tab
  drew slower the more steps there were); maps are coloured from tables, in a thread (a frame never waits
  for a map: the one shown before stays meanwhile), and their textures kept to be shown again at once; the
  legends of the maps shown are worked out in threads; a run uses the data read when the model was opened.
  With a map of 2048 x 1728 cells shown and MSPA running on it, frames take under 4 ms.

### Fixed
- The name of an MSPA value from 156 up overflowed (a panic in debug builds).
- Checking a model and ordering its nodes took time growing with the square of its nodes (a node looked for in
  a list for every reference): 141 ms for 10,000 nodes, now 10 ms; the engine's runs and fingerprints too. The
  order of the nodes no longer recurses (chains of any length).
- The automatic layout of the canvas took time growing with the cube of the nodes, at every frame.
- The operators' signatures are made once (`Registry::sig`), not at every look-up.
- The text tab lays out only the lines in view, and the model's text is made only when the tab is shown.
- The node editor no longer shows the references inside a loop's body as sources of the top level.
- The wizard's check of an ordered allocation no longer fails before the order of the classes is chosen.

## [0.7.1] - 2026-10-10

### Changed
- `terraflux` without a command (a double click on the executable) opens the application, and so does a model
  file given alone (dropped on the executable, "open with"); on Windows the console opened for it goes away (a
  terminal's stays).

## [0.7.0] - 2026-10-10

### Added
- Agents (type `Agents`: a population, a column per attribute): `agents.table` (macro agents written in the
  model), `agents.parcels` (an agent per parcel, its size and the means of maps over it), `agents.update`
  (behaviour: rules `attribute = expression` in order, map algebra's expressions), `agents.map` (an attribute
  painted on the parcels) and `agents.summary` (count, sums and means, to collect). Agents are carried along
  loops as the map is, and written as CSV.
- CRAFTY's competition for land (`crafty.allocate`): agent functional types produce services from the cells'
  capitals; unmet demand sets the services' value; managers give up below a threshold, and the most competitive
  type takes offered cells over the incumbent by its giving-in threshold; the demand's columns mapped to
  services (`services`); the same maps with the same seed.
- `docs/agents.md`.
- Tests: the agents' operators; CRAFTY's supply meets the demand and each type settles where its capitals are;
  a coupled model (a growing population giving the demand, parcel managers subsidising fertility, CRAFTY
  allocating) run by the engine.

## [0.6.0] - 2026-10-10

### Added
- The node editor (the application's Graph tab): operators added from a list, ports connected by dragging
  (each connection checked against the operators' signatures: types, cycles, ports), nodes moved (their place
  saved in the model, `pos`), renamed (their references follow) and deleted, values and settings typed, the
  bodies of loops and groups edited in their own scope (what they read from outside on the left), the model
  saved as written. `terraflux_app::editor` holds the editing operations.
- Map algebra (`map.calc`, as Dinamica EGO's Calculate Map): an expression over inputs of any names (maps,
  numbers, the layers of stacks as `stack.layer`): arithmetic, comparisons, `and or not`, `if then else`,
  functions, and neighbourhood functions (`nbsum nbmean nbmin nbmax nbcount`); missing cells propagate.
- Groups (`control.group`: a body run once, its inputs `@name` inside, its outputs `body.outputs`) and
  submodels (`control.submodel`: another model file as one node, expanded into a group before a run; its
  file inputs become inputs of the outer model).
- WebAssembly plugins (`plugin.wasm`, wasmi): a function of any language compiled to WebAssembly called for
  every cell, in a sandbox (no imports, a budget of instructions); batches send the module to the workers by
  hash.
- Operators may take inputs of any name (`OpSignature::extra`).
- `docs/modelling.md`.
- Tests: the editor's operations (types, cycles, renames, removals, bodies) and the editor in the application;
  map algebra; groups and submodels in the engine; plugins (and their sandbox); a submodel of map algebra and a
  plugin give on workers the bytes of a local run.

### Changed
- The application's Graph tab is the node editor (the read-only diagram is gone).

## [0.5.0] - 2026-10-10

### Added
- The tiled engine (crate `terraflux-engine`, `terraflux run --tiled`): Dinamica EGO's model on maps larger than
  memory, cut in tiles read, computed and written one at a time, in parallel, with the steps' layers in memory
  or on disk (`--disk`). With the same fitted pieces, the same seed gives the same maps whatever the tiling
  (`--tile`); the patches a fit measures depend on the tiles (a patch across a tile's edge counts as two).
  - Fits by reduction over the tiles: the Markov chain, the weights of evidence and the patches' statistics
    are the exact engine's to the bit.
  - Random numbers by counter: every draw is a function of the seed, the step, the transition and the cell.
  - Allocation by thresholds: the Expander's and the Patcher's seeds are the cells whose random keys pass
    thresholds found from histograms summed over the tiles; patches grow within a radius (`--radius`) of their
    seed, every tile with a halo of twice that radius; cells claimed by several patches go to the highest key;
    the thresholds are adjusted in rounds until the changes meet the demand. Statistically equivalent to the
    exact engine (the same areas, a similar fuzzy similarity), not the same draws.
  - Few passes over the map per phase: the thresholds of every transition are found in the same passes; the
    largest keys are collected once, so each round's seeds are a selection; the changes past the demand are
    cut by their exact scores.
  - Each window of a file of drivers is decoded once for all its bands; readers are kept open per thread.
  - Layers on disk keep the tiles read lately decoded in memory (`TERRAFLUX_CACHE_MB`, 2048 by default).
  - `TERRAFLUX_TRACE` prints the time of each pass of a step.
  - 56.6 million cells, 10 transitions, 3 steps on 4 threads: 318 s (43 s of fits, about 92 s a step).
- The tiled engine on many machines (`terraflux run --tiled --disk <shared> --coordinator <url>`): every pass
  over the tiles is a job of the coordinator, its tasks runs of tiles, the workers reading the maps and writing
  the layers on shared storage; the maps are those of the same run on one machine (`terraflux_engine::pass`:
  passes as data, `terraflux_dist::tiles::Remote`). The coordinator waits for jobs (`Wait`) and forgets
  them (`Forget`).
- GeoTIFF: windowed reading (`Reader`) and tiled writing in any order (`TiledWriter`), for maps larger than
  memory.
- `woe.probability` computes the buckets of a variable once for the transitions from a class (they share
  their layout): about twice as fast, the same values.
- `--threads` on every command. Parallel work never takes every core: by default all the cores but one (or
  `TERRAFLUX_THREADS`), for Rust's tasks, landschaft's OpenMP kernels and the workers of a local cluster.

## [0.4.0] - 2026-10-10

### Added
- Batches on many machines (crate `terraflux-dist`, gRPC): `terraflux serve` (a coordinator), `terraflux worker`
  (a headless worker: it only connects to the coordinator), `terraflux workers`, `terraflux submit` (replicates or
  scenarios: a folder of outputs per task) and `terraflux calibrate` (a grid or a Latin hypercube of settings,
  each run with the same seeds, scored on the workers against an observed map: figure of merit, overall
  agreement or the fuzzy similarity of the changes; the best settings written as a calibrated model). Without
  `--coordinator`, a coordinator and workers run in the command's own process. The data travels once, by
  SHA-256; workers keep a cache; a task whose worker stops sending heartbeats runs on another (three attempts).
- Model overrides (`Model::apply`): `$input` and `node.setting` (`node/inner.setting` inside a loop).
- `docs/cluster.md` and a Slurm template (`tools/slurm/terraflux-workers.sbatch`).
- Tests: replicates on workers give the bytes of local runs; a calibration finds the setting that made the
  observed map; the task of a vanished worker runs on another; workers sharing a cache fetch the data together.
## [0.3.0] - 2026-10-10

### Added
- The application (`terraflux app`, crate `terraflux-app`, egui on wgpu):
  - a wizard of new models (Dinamica EGO's, CLUE-S, ordered): the land use maps and drivers picked, the dates
    and steps taken from the band names, the data checked, misaligned drivers resampled to the land use map's
    grid on request, the model written with a note on every choice;
  - the run in the background, its maps drawn step by step as they come (zoom, pan, the cell under the
    pointer), stopped at any time;
  - the area of each class along the steps; the model's graph (what each node does and why); the model and the
    problems of its data;
  - validation against an observed map: figure of merit, hits, wrong hits, misses, false alarms, and the fuzzy
    similarity of the changes per window.
- The checks of a model's data (`terraflux_exec::lint`, also in `terraflux check`): grids, coordinate reference
  systems, whole classes, change between the two dates, drivers without values, constant, missing where the land
  use has a class or collinear; each with what to do.
- `validate.agreement` and `validate.similarity` (landschaft's `agreement` and `similarity_of_differences`).
- Resampling to another grid of the same coordinate reference system (`terraflux_raster::resample`,
  `terraflux resample`): nearest or mode for classes, bilinear or average for continuous values.
- The engine reports the values collected at each step and can be stopped.
- Tests of the application rendered off screen (egui_kittest): a model opened and run, every tab, the wizard
  stopping on misaligned data.
## [0.2.0] - 2026-10-10

### Added
- Models written by landschaft (1.44.0, `Model.save`) run here bit for bit as `ls.simulate` runs them: their fitted
  pieces are given in the graph by `woe.weights`, `glm.coefficients`, `markov.matrix` and `patches.settings` (NaN
  coefficients as `null` in JSON).
- Dynamic variables: `dynamic.distance_to` (exact Euclidean distance, map units or cells), `dynamic.share_of`
  (focal share of classes) and `stack.set`, recomputed from the simulated map at every step.
- PLUS's CARS: `plus.demand` (a Markov chain projected from the starting map, or an area demand) and `cars.step`.
- `stack.classes`: layers named by class or transition (`1_to_3`) as a potential computed elsewhere.
- Tests: models saved by landschaft (Dinamica, CLUE-S with a dynamic variable and neighbourhood rules, CARS) give
  the maps landschaft gives.

### Changed
- The submodule `extern/landschaft` at 1.44.0.
## [0.1.0] - 2026-10-10

### Added
- The model graph (`terraflux-graph`): models as TOML or JSON files (`format = "terraflux/1"`) with named
  inputs (maps, stacks, values), nodes running operators on inputs from `$inputs`, `node.port` outputs,
  `@carried` values and literals, a note per node, containers with a body (`control.repeat`: carried and
  collected values), outputs; checks of every operator, reference, port, setting and type, and of cycles.
- The operators (`terraflux-ops`), each the same code as landschaft's `ls.simulate` through its C++ kernels:
  `markov.fit`, `demand.from_maps`, `demand.table`, `glm.fit`, `glm.predict`, `woe.fit`, `woe.probability`,
  `patches.observe`, `dinamica.step`, `lulcc.history`, `lulcc.step` (CLUE-S and the ordered allocation with
  conversion rules, elasticities, neighbourhoods, restrictions and history), `rng.r`, `control.repeat`; the
  presets `dinamica` and `lulcc` (CLUE-S and ordered) write their graphs with a note on every choice.
- The exact engine (`terraflux-exec`): inputs read from GeoTIFF, the graph run in the order of its
  dependencies, outputs written as GeoTIFF (series of maps as a band per step), CSV and JSON.
- GeoTIFF in pure Rust (`terraflux-raster`): strips and tiles, pixel and band interleaving, no compression,
  Deflate, LZW and PackBits, horizontal and floating-point predictors, little- and big-endian, classic TIFF and
  BigTIFF; the georeference tags, GDAL's no-data value and band descriptions kept.
- The bridge to landschaft's kernels (`terraflux-ls-sys`, cxx): landschaft 1.43.0 built with CMake
  (`LANDSCHAFT_PYTHON=OFF`) from the submodule `extern/landschaft`.
- The `terraflux` command: `run`, `check`, `ops`, `new dinamica|clue|ordered`, `convert`.
- Tests: parity with landschaft bit for bit (Dinamica EGO's model on Sibuyan, CLUE-S and the ordered allocation
  on Plum Island with every rule), the GeoTIFF layouts GDAL writes, the graph's checks, the engine's loops.
