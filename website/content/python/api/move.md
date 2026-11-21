---
title: Animal movement
description: "Movebank tracks, steps, speeds and turns, filters, track lines (move2); random steps for step selection (amt)."
order: 190
---

Animal tracks and their steps: the tracks of tagged animals as GeoDataFrames of
points, one row per location, ordered by track and time. Reads Movebank's
files; the distance, duration, speed, direction and turn of every step;
duplicates, one location per interval and Movebank's outliers; locations
interpolated in time; tracks as lines and segments; a local projection centred
on the tracks; Brownian tracks. The same numbers as R's move2 0.5.0
(Kranstauber et al. 2024): on the sphere of S2 and on the ellipsoid of lwgeom
(GeographicLib) for longitudes and latitudes, the plane of the CRS otherwise.
Then amt 0.3.1's (Signer et al. 2019) steps for step selection functions:
tracks resampled at a regular rate in bursts, the steps with their lengths and
turn angles, summaries of tracks (straightness, sinuosity, net squared
displacement...), day and night, distributions of step lengths and turn angles,
random steps and the covariates at their ends, with amt's numbers (and R's
random numbers given a seed). From move2utils 0.4.6 (Kranstauber, Safi & Scharf
2026): the simple part of cleaning a track (Hirt et al.'s allometric speed limit,
GPS quality, thinning in time and in distance), the dynamic Brownian bridges
(dBBMM and its directional dBGB: the motion variance by moving windows and the
utilisation distribution), the corridors the animals use (LaPoint et al. 2013)
and the earth mover's distance between distributions. From amt, the values of a
raster along each step and the home ranges (MCP, kernel density with four
bandwidths, LoCoH, isopleths, overlaps). Last, the use of the animals against a
connectivity surface (`validate`). C++ with OpenMP; the same with any number of
threads. The `tf.move` module; the [tutorials](/tutorials/animal-movement/) walk
through it on the tracks of fishers, with the Python in each part.

<!-- no-run -->
```python
import terraflux as tf

tracks = tf.load_tracks("study.csv")                    # Movebank's export (.csv, .csv.gz, .zip)
tracks = tf.move.filter_movebank_visible(tracks)        # without the outliers Movebank hides
tracks = tf.move.filter_unique(tracks)                  # one record per time and track
tracks = tracks[~tracks.geometry.is_empty]

tracks["step"] = tf.move.distance(tracks)               # metres to the next location
tracks["speed"] = tf.move.speed(tracks, "km/h")
tracks["turn"] = tf.move.turn_angle(tracks, "deg")

hourly = tf.move.filter_per_interval(tracks, "hour")    # the first location of each hour
regular = tf.move.interpolate(tracks, "30 mins", omit=True)
local = tracks.to_crs(tf.move.aeqd_crs(tracks))         # metres around the tracks
tf.write_vector(tf.move.track_lines(tracks), "tracks.gpkg")
```

The data of a step selection function, as amt makes them:

<!-- no-run -->
```python
utm = tracks.to_crs(tf.move.aeqd_crs(tracks))           # random steps need a projected CRS
bursts = tf.move.resample(utm, "2h", "15min")           # a location every 2 hours, in bursts
steps = tf.move.steps(bursts)                           # sl_, ta_ per burst
ssf = tf.move.random_steps(steps, n_control=10, seed=1) # observed + 10 random steps per stratum
ssf = tf.move.extract_covariates(ssf, {"forest": forest, "elev": dem})
ssf["tod"] = tf.move.time_of_day(ssf, where="start")
ssf["log_sl"] = np.log(ssf["sl_"])
ssf["cos_ta"] = np.cos(ssf["ta_"])
```

Every function finds the time and the track in the usual columns (`timestamp`,
`individual-local-identifier`, `time`, `track`...); others are named once with
`tf.move.set_columns` (or `tf.move.as_tracks`), or in each call with `time=` and
`track=`.

## Tracks

::: terraflux.move.load_tracks

::: terraflux.move.example_data

::: terraflux.move.as_tracks

::: terraflux.move.set_columns

::: terraflux.move.simulate_brownian

## Steps

::: terraflux.move.distance

::: terraflux.move.time_lags

::: terraflux.move.speed

::: terraflux.move.azimuth

::: terraflux.move.turn_angle

## Filters

::: terraflux.move.unique

::: terraflux.move.filter_unique

::: terraflux.move.per_interval

::: terraflux.move.filter_per_interval

::: terraflux.move.movebank_visible

::: terraflux.move.filter_movebank_visible

## Interpolation

::: terraflux.move.interpolate

## Lines and projections

::: terraflux.move.track_lines

::: terraflux.move.segments

::: terraflux.move.track_data

::: terraflux.move.aeqd_crs

## Steps of amt

::: terraflux.move.resample

::: terraflux.move.filter_min_n_burst

::: terraflux.move.steps

::: terraflux.move.time_of_day

## Summaries of tracks

::: terraflux.move.nsd

::: terraflux.move.cum_dist

::: terraflux.move.tot_dist

::: terraflux.move.straightness

::: terraflux.move.msd

::: terraflux.move.sinuosity

::: terraflux.move.tac

## Step selection

::: terraflux.move.make_distribution

::: terraflux.move.Distribution

::: terraflux.move.fit_distr

::: terraflux.move.random_numbers

::: terraflux.move.random_steps

::: terraflux.move.extract_covariates

## Cleaning and thinning

The simple part of move2utils' cleaning: a speed limit from the animal's mass and
mode of travel, the locations beside steps above it, GPS quality, and thinning
a track to a given interval or step.

<!-- no-run -->
```python
tracks = tf.move.filter_gps_quality(tracks)                 # satellites, DOP, horizontal accuracy
vmax = tf.move.v_phys_estimate(mass=5, mode="flying")       # m/s, with its interval
flagged = tf.move.flag_speed(tracks, mass=5, mode="flying") # the column speed_flag
thin = tf.move.filter_thin_time(tracks, "45 min")           # about 45 minutes between locations
```

::: terraflux.move.v_phys_estimate

::: terraflux.move.VPhys

::: terraflux.move.flag_speed

::: terraflux.move.gps_quality

::: terraflux.move.filter_gps_quality

::: terraflux.move.thin_time

::: terraflux.move.filter_thin_time

::: terraflux.move.thin_distance

::: terraflux.move.filter_thin_distance

## Values along the steps

The cells each step crosses and their values (amt's `extract_covariates_along`),
and per-step summaries that tell a step up a hill from one along a contour line.

::: terraflux.move.extract_covariates_along

::: terraflux.move.along_summary

## Home ranges

amt's home ranges, with its numbers: the minimum convex polygon, the kernel
density estimate with the reference, scaled reference, plug-in and
cross-validation bandwidths, and LoCoH. `home_range` runs one of them in a call.

<!-- no-run -->
```python
hr = tf.move.home_range(utm, method="kde", levels=(0.5, 0.95))
hr.polygons                                             # the isopleths, one row per track and level
tf.move.hr_area(hr)                                     # their areas
mcp = tf.move.hr_mcp(utm, levels=0.95)
ud = tf.move.hr_ud(tf.move.hr_kde(utm))                 # the kernel density estimate, summing to 1
```

::: terraflux.move.make_trast

::: terraflux.move.hr_mcp

::: terraflux.move.hr_kde

::: terraflux.move.hr_kde_ref

::: terraflux.move.hr_kde_ref_scaled

::: terraflux.move.hr_kde_pi

::: terraflux.move.hr_kde_lscv

::: terraflux.move.Lscv

::: terraflux.move.LscvFit

::: terraflux.move.hr_locoh

::: terraflux.move.hr_isopleths

::: terraflux.move.hr_area

::: terraflux.move.hr_ud

::: terraflux.move.hr_cud

::: terraflux.move.hr_overlap

::: terraflux.move.intensity_use

::: terraflux.move.home_range

::: terraflux.move.HomeRange

## Dynamic Brownian bridges

move2utils' dBBMM (Kranstauber et al. 2012) and dBGB (Kranstauber et al. 2014):
the motion variance of each location by moving windows, then the utilisation
distribution as a raster with one band per track, summing to 1. The same
numbers with any number of threads.

<!-- no-run -->
```python
win = tf.move.suggest_dbbmm_window(utm, target_hours=48)
var = tf.move.dbbmm_variance(utm, location_error=10, window_size=win.window_size, margin=win.margin)
ud = tf.move.dbbmm_ud(var, res=100)
vol = tf.move.ud_volume(ud)                             # the cumulative volume, for the isopleths
```

::: terraflux.move.dbbmm_variance

::: terraflux.move.dbbmm_ud

::: terraflux.move.dbgb_variance

::: terraflux.move.dbgb_ud

::: terraflux.move.motion_variance

::: terraflux.move.suggest_dbbmm_window

::: terraflux.move.WindowSuggestion

::: terraflux.move.mask_segments

::: terraflux.move.ud_volume

::: terraflux.move.ud_outer_probability

## Corridors and distances between distributions

::: terraflux.move.corridor

::: terraflux.move.corridor_lines

::: terraflux.move.ud_distance

## Validation

::: terraflux.move.validate

::: terraflux.move.Validation
