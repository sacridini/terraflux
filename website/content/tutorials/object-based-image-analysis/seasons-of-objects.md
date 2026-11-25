---
title: "Seasons of objects: a year of Sentinel-2 over farmland"
short: Seasons of objects
description: "Build a year of NDVI from Sentinel-2 over farmland in Mato Grosso, cut it into parcels, give each parcel its profile, its season and its harmonics with objects.series, group the parcels by crop calendar, and read the profile of one parcel in the inspector, with the template Crops by their seasons."
order: 210
level: advanced
time: 75 min
template: Crops by their seasons
ops: [data.stac, raster.calc, stack.of, segment.multiresolution, objects.series, ml.kmeans, objects.classify, objects.to_map]
series: Object-based image analysis
part: 21
---

On one day, a field of soybean and a field of pasture can be the same green. Over a year they are not: the soybean is bare, then
grows, then is cut, and often is followed by a second crop, while the pasture hardly changes. The **time** at which a surface is green
tells what it is. This tutorial puts that to work on objects. You will build a year of NDVI from Sentinel-2 over farmland in Mato Grosso,
Brazil, one image every 16 days, with the clouds left as gaps; cut the area into parcels; give each parcel its own series; and measure
its season. You will end with groups of parcels that share a crop calendar, and with the profile of any parcel in the Viewer.

## The idea

An object is a place, and a place has a **profile**: its NDVI on each date. The value of an object on a date is the mean of its cells that
have a value that day, so a cloud over half of a parcel leaves the other half to give the mean, and a cloud over all of it leaves a gap.
A profile of 23 dates is a curve, and a curve can be measured.

![The area: farmland north-west of Sorriso in the dry season, with the parcels](/img/tutorials/object-based-image-analysis/seasons-of-objects/parcels.png "The 4 km by 4 km of the tutorial (a July composite of Sentinel-2, 10 m cells) and the 65 parcels the segmentation made of the year's NDVI. Large fields, a gallery forest along the creeks, and the bare soil of the dry season.")

[[objects.series]] gives three groups of measures of each profile, as columns of the objects' table:

- **`values`**: a column for each date, `ndvi_20230215`: the profile itself.
- **`phenology`**: the maximum, the minimum and the amplitude; the day of the peak (`peak_doy`, and `peak_date` in days since 1970-01-01); the
  integral (the area under the curve, by trapezoids, in NDVI times days); the **start** and the **end** of the season, with their `length`; and the `rise_rate` and the
  `fall_rate`.
- **`harmonics`**: a least squares fit of `n` harmonics of a one-year period: the mean, the amplitude and the phase of each harmonic, and
  the RMSE of the fit.

!!! science "Key idea: the season, by the amplitude method"
    The start and the end follow TIMESAT's amplitude method (Jönsson and Eklundh, 2004). Each side of the peak has its own **base**, its own minimum: \( b_l \) on the left, \( b_r \) on the right. The
    season starts where the curve, walked back from the peak, first falls below \( b_l + t\,(max - b_l) \) and ends where, walked forward, it first falls below
    \( b_r + t\,(max - b_r) \), with \( t \) the `season_threshold` (0.5 by default), each crossing found by a straight line between the two dates. A curve that never falls below the level on a side
    has no start (or no end): the value is missing, not a guess.

    **Harmonics.** With \( t \) the days since 1 January of the first date's year, the model of a profile is
    \[ y(t) = \mu + \sum_{k=1}^{n} \left[ a_k \cos\frac{2\pi k t}{365.25} + b_k \sin\frac{2\pi k t}{365.25} \right], \]
    the amplitude of the \( k \)-th harmonic is \( \sqrt{a_k^2 + b_k^2} \) and its phase \( \operatorname{atan2}(b_k, a_k) \). One harmonic is a single smooth season a year, two can hold a second one: a **double crop**
    is a large second harmonic (Jakubauskas and colleagues, 2001). It needs \( 2n + 2 \) dates with a value at least.

The two groups tell different things. The phenology measures **one** season, the highest. The harmonics measure the shape of the whole year, and so they
see what the phenology leaves out.

## When to use it

- **Crops and pastures**: what grows when. A calendar is a better key than a colour on one day.
- **Any class with a season**: deciduous and evergreen forest, wetlands that flood, irrigated and rainfed fields, burnt and unburnt areas.
- **When one date is not enough**: a classification of one image confuses a fallow field with a pasture; a year does not.
- **As attributes for a classifier**: the columns join the objects' table, and [[objects.samples]], [[ml.fit]] and [[objects.classify]] read them as any attribute.

## What you need

- A series of an index with a layer for each date, **named by its date** (`ndvi_20230215`, or `2023-02-15` anywhere in the name) or the setting `dates`, and the objects.
  [[objects.series]] reads the date from the name, and refuses a layer with no date, a date that does not follow the one before, and a series of fewer than four dates.
- In the template *Crops by their seasons*, **File > Use my data...** takes your own parcels, as a map of parcel ids or as
  polygons, and the series of your image.
- Terraflux: the application for the profile in the Viewer, the command line for the numbers. About 75 minutes, mostly the downloads.

## Build the model

### Step 1: a year of NDVI

[[data.stac]] combines the scenes of a date range into one image, a **composite**, so a series is made of one composite for each window of dates. Here the windows are 16 days from 1 September 2022, 23 of them,
and each gives the median of the cells of the scenes that have a value. `mask = "auto"` drops the cloud, shadow and cirrus cells (Sentinel-2's scene classification) before the scenes are combined, so a window
with only cloudy scenes has **no value** there: a gap, which is what we want.

```tfgraph
#! caption: One window of the series: the red and near infrared bands of the scenes of 16 days, and their NDVI. The model has one such pair of nodes for each of the 23 windows.
#! highlight: w0, n0
#! numbered: true
#! fragment: true
[[nodes]]
id = "w0"
op = "data.stac"
params = { url = "https://earth-search.aws.element84.com/v1", collection = "sentinel-2-l2a", assets = ["red", "nir"], datetime = "2022-09-01/2022-09-16", extent = [623032, 8632475, 627032, 8636475], cellsize = 10, crs = "EPSG:31981", mask = "auto", composite = "median", query = '{"earthsearch:boa_offset_applied": {"eq": true}}' }
note = "The scenes of the first 16 days over 4 km by 4 km, in UTM 21S, 10 m cells: only the cells without cloud, and only the scenes whose offset the catalogue has already taken out."

[[nodes]]
id = "n0"
op = "raster.calc"
inputs = { s = "w0.raster" }
params = { expression = "None if (s.nir + s.red) < 300 else clamp((s.nir - s.red) / (s.nir + s.red), -1, 1)" }
note = "NDVI of the reflectance (the integers are the reflectance times 10 000). A cell too dark to have a ratio (the two bands add up to less than 300) is none."
```

!!! warning "The offset of 1000"
    Scenes of Sentinel-2 L2A processed with baseline 04.00 or later (from January 2022) have `BOA_ADD_OFFSET` = -1000: the integer in the original file is the reflectance times 10 000 **plus 1 000**. Earth Search
    has taken it out of most of its copies and says so on each scene, `earthsearch:boa_offset_applied`; [[data.stac]] gives the file's values as they are. Over this area, 72 of the 74 scenes of the year
    have it applied, and two (baseline 04.00) do not, so the `query` keeps the 72. Check the property on your own catalogue: subtracting the offset from values that no longer have it makes
    the ratio too high (the first attempt at this series did exactly that, and its NDVI sat at 0.99 to 1.00 all year), and not subtracting it from values that have it makes the ratio too low.

Then the 23 NDVI maps are stacked, one layer for each, **named by the middle date of its window**, `ndvi_20220908` to `ndvi_20230826`, with [[stack.of]]. A script writes the 46 nodes of this stage, and the result is kept as `series.tif`, which the rest reads.

```tfgallery
columns: 3
caption: "Six of the 23 windows (NDVI, light green to dark green; grey where no scene has a value). The 11 November and the 15 February windows have no clear cell at all; the 29 December window has 63 % of the cells missing. The rest are the wet season of Mato Grosso: clouds are the rule, not the exception."
images:
  - { src: /img/tutorials/object-based-image-analysis/seasons-of-objects/ndvi_1.png, caption: "24 September: the fields are bare or just sown" }
  - { src: /img/tutorials/object-based-image-analysis/seasons-of-objects/ndvi_4.png, caption: "11 November: all cloud" }
  - { src: /img/tutorials/object-based-image-analysis/seasons-of-objects/ndvi_7.png, caption: "29 December: most of it cloud" }
  - { src: /img/tutorials/object-based-image-analysis/seasons-of-objects/ndvi_10.png, caption: "15 February: all cloud" }
  - { src: /img/tutorials/object-based-image-analysis/seasons-of-objects/ndvi_14.png, caption: "20 April: the second crop is green" }
  - { src: /img/tutorials/object-based-image-analysis/seasons-of-objects/ndvi_19.png, caption: "9 July: harvest and the dry season" }
```

Of the 23 × 160,000 values of the series, 24 % are missing. A parcel will have gaps, and the measures must be made without them.

### Step 2: the parcels

The parcels are objects that stay the same through the year, so they are cut from what the whole series says: the **maximum**, the **minimum** and the **mean** NDVI of each cell over the dates (a stack of three layers made by [[raster.calc]],
which skips the missing dates), segmented by [[segment.multiresolution]] at the scale 10: 65 parcels in the 400 by 400 cells, with a median of 1,486 cells (14.9 ha).

### Step 3: the series, the seasons and the harmonics

```tfgraph
#! caption: The analysis on the series: the parcels, the profile, season and harmonics of each, four groups by k-means on the seasons, and each parcel given its group.
#! highlight: seasons, calendar
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "s"
op = "raster.read"
params = { path = "series.tif" }
note = "The series made in step 1: 23 layers named by their dates."

[[nodes]]
id = "feat"
op = "raster.calc"
inputs = { s = "s.raster" }
params = { expression = """
out hi = max(s)
out lo = min(s)
out mid = mean(s)
""" }
note = "The maximum, the minimum and the mean of each cell over the year (the dates with no value are skipped)."

[[nodes]]
id = "featstack"
op = "stack.of"
inputs = { hi = "feat.hi", lo = "feat.lo", mid = "feat.mid" }

[[nodes]]
id = "parcels"
op = "segment.multiresolution"
inputs = { image = "featstack.stack" }
params = { scale = 10 }
note = "The parcels: areas that are alike over the whole year."

[[nodes]]
id = "seasons"
op = "objects.series"
inputs = { objects = "parcels.objects", series = "s.raster" }
params = { variable = "ndvi", groups = ["phenology", "harmonics"], n = 2, smooth = "savgol", window = 5, order = 2 }
note = "The mean NDVI of each parcel on each date, smoothed over 5 dates by Savitzky-Golay (a polynomial of order 2), then the measures of its season and a fit of two harmonics."

[[nodes]]
id = "km"
op = "ml.kmeans"
inputs = { table = "seasons.table" }
params = { columns = ["ndvi_amplitude", "ndvi_hmean", "ndvi_amp1", "ndvi_amp2"], k = 4, seed = 1 }
note = "Four groups of parcels by the size of their season and of their harmonics: columns that are not angles (the day of the peak and the phases are left out, see the scale of the peak below)."

[[nodes]]
id = "calendar"
op = "objects.classify"
inputs = { objects = "seasons.objects", model = "km.model" }
note = "Each parcel takes the group of its season."
```

`smooth = "savgol"` fits a polynomial to each window of five dates (Savitzky and Golay, 1964; the same numbers as scipy's `savgol_filter`) before the season is measured, over the series with its gaps filled by
straight lines in time. The window counts **dates**, not days, so use it on dates that are evenly spaced, as these are (16 days): the operator warns when they are not. The smoothed line can go a little beyond the values it is fitted to.

```bash
terraflux run model.toml
```

## Read the results

### The profile of a parcel

The **long table** `series` of [[objects.series]] has a row for each parcel and date. In the Viewer, select a parcel and the inspector draws its profile: the dates as points, a cloud as a gap, the smoothed line,
and, once some parcels have been given as samples of a class, the band of the samples of its class (the median and the quartiles by date). **Ctrl+click** another parcel and its profile is drawn beside the first.

![Four parcels, one of each group](/img/tutorials/object-based-image-analysis/seasons-of-objects/profiles.png "The mean NDVI of four parcels over the year (points), the smoothed series (line), the dates with no value (grey bars), the start and the end of the season (dashed lines) and the peak (triangle). Parcel 61 is flat and high; parcel 19 has two low humps; parcels 39 and 56 grow twice, with a deep dip between the crops.")

The four parcels are four calendars. **61** stays between 0.66 and 0.84: trees, with an amplitude of 0.18 and a second harmonic of 0.03. **19** has two low humps, in December and in April, none above 0.65, and is bare (0.13) from July to September.
**39** and **56** have **two seasons**: a first crop that peaks around the new year (0.79 for 39, 0.92 for 56), a dip at 0.3 in February (the harvest and the sowing of the next crop), and a second peak in April to May. The phenology,
which measures one season, reports the highest: for **39** it is the second crop (the peak on 6 May, 100 days from 9 March to 17 June), for **56** the first (the peak on 29 December, 58 days from 24 November to 21 January). The harmonics see both
(their second amplitudes are 0.21 and 0.13, and the RMSE of the fit of two harmonics, 0.11 and 0.17, tells that the two sharp peaks are not a pure sinusoid).

```tftable
sortable: true
align: { calendar: left, peak_date: left, start: left, end: left }
labels: { parcel: Parcel, calendar: Group, cells: Cells, max: Max, amplitude: Amplitude, peak_date: Peak, start: Start, end: End, length: "Days", integral: Integral, hmean: "Mean (fit)", amp1: "Amp 1", amp2: "Amp 2", rmse: RMSE }
columns: [parcel, calendar, cells, max, amplitude, peak_date, start, end, length, integral, amp1, amp2, rmse]
digits: { cells: 0, max: 2, amplitude: 2, length: 0, integral: 0, amp1: 3, amp2: 3, rmse: 3 }
data: /data/tutorials/object-based-image-analysis/seasons-of-objects/parcels.csv
caption: "The measures of the four parcels. The second harmonic is large for the two-crop parcels (amp2 0.21 and 0.13), 0.11 for parcel 19 and tiny for the forest (0.03). The RMSE of the fit of two harmonics is 0.11 and 0.17 for the double crops, whose curve has two sharp peaks."
```

### The day of the peak

The column `ndvi_peak_doy` is the day of the year of the highest value, and a map of it ([[objects.to_map]] with `what = "ndvi_peak_doy"`) is a map of where the harvest of the crop of the season is.

![The day of the peak of each parcel](/img/tutorials/object-based-image-analysis/seasons-of-objects/peak.png "The day of the year of the highest NDVI of each parcel, from 1 January to 31 December on a cyclic scale: January and December are neighbours and are both pale. The pale parcels peak around the new year: their first crop is the highest.")

The scale is **cyclic**: a peak on 29 December and one on 3 January are five days apart, though their day numbers are 360 apart. A day of the year is an angle, and so is the **phase** of a harmonic: a method that computes distances
(k-means, nearest neighbour) reads a peak just before the new year and one just after it as far apart. That is why the k-means below reads only the **sizes** (the amplitude, the mean, the amplitudes of the two harmonics), which are not angles, and leaves the day of
the peak and the phases out. To use an angle in such a method, give it as two columns, the sine and the cosine of 2π times the day over 365.25 (the expression language of [[objects.calc]] has no sine yet: compute them outside, or leave them to a method
that splits on thresholds, such as a random forest, which the template uses).

### Groups of parcels by their calendar

```tftable
align: { calendar: left }
labels: { calendar: Group, parcels: Parcels, area_share: "Area (%)", peak_doy: "Peak (day of year)", amplitude: Amplitude, max: Max, amp1: "Amp 1", amp2: "Amp 2", length: "Days of the season" }
columns: [calendar, parcels, area_share, peak_doy, amplitude, max, amp1, amp2, length]
digits: { parcels: 0, area_share: 1, peak_doy: 0, amplitude: 2, max: 2, amp1: 3, amp2: 3, length: 0 }
data: /data/tutorials/object-based-image-analysis/seasons-of-objects/calendars.csv
caption: "K-means on the amplitude, the mean and the first two harmonics' amplitudes of the 65 parcels: four groups, with the medians of their parcels (the day of the peak and the length are not in the clustering: they are read from it)."
```

![The four groups on the map](/img/tutorials/object-based-image-analysis/seasons-of-objects/calendars.png "The parcels coloured by their group.")

![The profile of each group](/img/tutorials/object-based-image-analysis/seasons-of-objects/calendar_curves.png "The median (line) and the quartiles (band) of the smoothed NDVI of the parcels of each group.")

The groups are **not crops** yet. Blue (13 parcels, 13 % of the area) is the evergreen cover: NDVI between 0.65 and 0.85 all year, a second harmonic of 0.03 and a maximum of 0.84. Orange (8 parcels, 2 % of the area) has two low humps,
below 0.65, in December and in April to May: a weaker crop or a pasture (a second harmonic of 0.11). Green (33 parcels, 69 %) is the double crop of the area: two seasons with a deep dip in February, a second harmonic of 0.18, the highest peak in May (the median peak on 6 May, a season of 86 days). Pink (11 parcels, 16 %)
is the same calendar sown earlier, with the first crop the highest (a first harmonic of 0.31, the median peak on 29 December), and a season, for most, of a long 206 days at the median because the dip between its crops is shallow in some parcels. To call them
soybean, maize, cotton or pasture you need ground knowledge, and a **reference sample** is the way to give it: name some parcels, train [[ml.fit]] on the season and the harmonics, and classify, as in
[Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/) (part 5). The template **Crops by their seasons** does exactly that on a series drawn for four crops, and reaches a purity of 100 %.

## Settings that change the answer

- **`smooth`**: `none` measures the series as it is (the dates with a value joined by lines), which suits a series with few gaps; `savgol` smooths over a window of dates (`window`, odd, and `order` below it); `harmonic` takes the fit of the harmonics as the series.
- **`season_threshold`**: 0.5 is the middle of the way up; 0.25 starts the season earlier and ends it later. Compare seasons made with the same threshold.
- **`min_share`**: the share of an object's cells that must have a value for the date to count (0.5). Lower it for small objects under patchy clouds, raise it for a cleaner profile.
- **`n`**: the harmonics, one for a single season a year, two or three for a double crop. More harmonics fit the noise too: the RMSE falls whatever you do, so watch the amplitudes of the last ones.
- **`groups`**: `values` adds a column for each date, which a classifier can read too, but 23 columns of values are more than 23 numbers that say when.

!!! warning "Dates that are not a year"
    The harmonics assume a period of **one year**. A series of two years fits the same model to both, and a series of three months is a poor fit of any harmonic. The phenology is made of the whole series: with two years it has one peak.

## Common pitfalls

- **The offset of 1 000.** See the warning above: whether the scenes still have it depends on the catalogue and on the scene (`earthsearch:boa_offset_applied`): wrong either way, the NDVI is too low or pinned near 1. Look at a forest and a bare field before trusting a series.
- **Composites that hide a gap.** A median of the scenes of a window is a single image: a window with one clear day gives a value and a window with none gives none. Windows of different lengths give profiles that are not comparable.
- **A peak near the new year.** A day of the year, and the phase of a harmonic, are angles: do not give them to k-means as numbers (use the sizes, or the sine and cosine).
- **Trusting the season of a double crop.** The start and the end belong to the highest peak only. For two seasons, read the harmonics, or measure two windows apart.
- **Segmenting one date.** The parcels of a crop calendar change between dates. Cut them from the maximum, the minimum and the mean of the year, or from a few dates at the peaks.
- **Expecting a class from a group.** K-means groups the alike; it does not name them.

## Going further

- The operators: [[objects.series]] and its sibling for every cell, [[raster.phenology]]; [[data.stac]], [[segment.multiresolution]], [[ml.kmeans]], [[objects.classify]].
- Choose the scale of the parcels: [Choosing the scale](/tutorials/object-based-image-analysis/choosing-the-scale/) (part 3), and by drawn references [Measuring a segmentation against reference objects](/tutorials/object-based-image-analysis/measuring-against-reference-objects/) (part 20).
- Classify the parcels with samples and a random forest, and judge the result: [Classifying by samples](/tutorials/object-based-image-analysis/classifying-by-samples/) (part 5).
- Groups without samples, with k-means and a self-organizing map: [Classes without samples](/tutorials/object-based-image-analysis/classes-without-samples/) (part 9).
- The template of this tutorial, **Crops by their seasons**, runs on a drawn year: open it from the Templates menu and select a parcel to see its profile.

## References

- Jakubauskas, M. E., Legates, D. R., & Kastens, J. H. (2001). Harmonic analysis of time-series AVHRR NDVI data. *Photogrammetric Engineering & Remote Sensing*, 67(4), 461-470.
- Jönsson, P., & Eklundh, L. (2004). TIMESAT: a program for analyzing time-series of satellite sensor data. *Computers & Geosciences*, 30(8), 833-845. https://doi.org/10.1016/j.cageo.2004.05.006
- Savitzky, A., & Golay, M. J. E. (1964). Smoothing and differentiation of data by simplified least squares procedures. *Analytical Chemistry*, 36(8), 1627-1639. https://doi.org/10.1021/ac60214a047
- Zhu, Z., & Woodcock, C. E. (2014). Continuous change detection and classification of land cover using all available Landsat data. *Remote Sensing of Environment*, 144, 152-171.
  https://doi.org/10.1016/j.rse.2014.01.011
