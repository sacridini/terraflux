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
random numbers given a seed). C++ with OpenMP; the same with any number of
threads. The `tf.move` module; the [tutorial](/tutorials/animal-movement/animal-movement-in-python/) walks
through it on the tracks of fishers.

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
