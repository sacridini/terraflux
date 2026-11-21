---
title: Animal movement
description: "The measures of animal tracks (move2, move2utils and amt): steps and turns, track summaries, corridors, home ranges and their bandwidths, the overlap of home ranges, Brownian bridge utilisation distributions and the validation of a connectivity surface with the animals' use."
order: 120
---

::: metrics animal-movement

## The example track {#example}

The values on the cards were computed on the fisher M4 of the fishers' study that ships with the library (GPS fixes of
eight fishers near Albany, New York, from Movebank), as the [tutorial on home ranges](../../tutorials/animal-movement/home-ranges/)
prepares it: the records Movebank shows, one per time and with a location, projected to the track's own azimuthal
equidistant projection so that lengths are in metres, and thinned to one location per about 10 minutes. The two halves of
the track, before and after 6 March 2010, give the overlap and the validation cards two periods of the same animal to
compare; their kernel estimates share one template raster and their Brownian bridges one grid, as those indices need:

```python
import numpy as np
import pandas as pd
import terraflux as tf

ID = "individual-local-identifier"
tracks = tf.load_tracks(tf.move.example_data("fishers"))
tracks = tf.move.filter_unique(tf.move.filter_movebank_visible(tracks))
tracks = tracks[~tracks.geometry.is_empty]

# the fisher M4 in metres from its centre, one location per about 10 minutes
m4 = tracks[tracks[ID] == "M4"].reset_index(drop=True)
m4 = m4.to_crs(tf.move.aeqd_crs(m4))
m4 = m4[tf.move.thin_time(m4, "10 mins")].reset_index(drop=True)

# its first and second half (before and after 6 March 2010): kernel estimates on one template, bridges on one grid
half = m4["timestamp"] < pd.Timestamp("2010-03-06", tz="UTC")
first, second = m4[half], m4[~half]
trast = tf.move.make_trast(m4, factor=1.25, res=25)
kde_a = tf.move.hr_kde(first, trast=trast, levels=[0.5, 0.95])
kde_b = tf.move.hr_kde(second, trast=trast, levels=[0.5, 0.95])
halves = m4.assign(half=np.where(half, "first", "second"))
uds = tf.move.dbbmm_ud(halves, location_error=20, window_size=25, margin=7, res=50, track="half")
ud_a, ud_b = uds.isel(band=0), uds.isel(band=1)

print(len(m4), "locations:", len(first), "in the first half,", len(second), "in the second")
area = tf.move.hr_area(tf.move.hr_kde(m4, trast=trast, levels=[0.5, 0.95])).set_index("level")["area"] / 1e6
print(f"kernel home range {area[0.95]:.2f} km2, core {area[0.5]:.2f} km2")
print(tf.move.hr_overlap(kde_a, kde_b, type="vi").round(3).to_string(index=False))
```

```text
2228 locations: 951 in the first half, 1277 in the second
kernel home range 9.99 km2, core 2.38 km2
track  level  overlap
   M4    1.0    0.427
```

The kernel estimates of the two halves share 43 % of their use (VI), and the
[earth mover's distance](#ud_distance) between their Brownian bridges says how far the use moved. The bandwidth matters
more than any other choice: the [reference bandwidth](#hr_kde_ref) of M4 is about 260 m and the
[plug-in one](#hr_kde_pi) about 35 m, and the home ranges they give differ by a factor of three (see the tutorial).
Step measures depend on the sampling interval, so compare animals only at a common one.
