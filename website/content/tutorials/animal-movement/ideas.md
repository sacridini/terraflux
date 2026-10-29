---
title: "Animal movement: the ideas"
short: The ideas
description: What a GPS track is made of (locations, times, steps, turns), where tracking data come from, and a first model that reads the tracks of eight fishers from a Movebank file and draws them.
order: 10
level: beginner
time: 35 min
ops: [move.read_tracks, move.track_lines, move.track_data, vector.filter]
series: Animal movement
part: 1
---

An animal wearing a GPS collar leaves a trail of points: where it was, and when. From those points ecologists
learn how far animals travel, which habitats they use and avoid, where they cross roads, how they respond to
people, and which corridors connect their populations. In this first part of the series you will learn what a
track is made of, where tracking data come from and how they are stored, and you will build a first model in
Terraflux that reads the tracks of eight fishers, small carnivores of the forests of North America, from a file
of Movebank, and draws them over a map of land cover. At the end you will have the tracks on a map and a table
of the animals, and you will know what to look at before any analysis.

The next parts build on this one: [cleaning the tracks and measuring them](../cleaning-and-measures/) (distances,
speeds, directions, turns), [projections, interpolation and simulated tracks](../projections-and-interpolation/),
[regular steps and the summaries of a track](../steps-and-summaries/) and
[the data of step selection functions](../step-selection/), the method that relates movement to the landscape.

## The idea

### A track is a sequence of locations in time

A tracking device records **fixes**: at a given time, it tries to work out where it is. A GPS receiver does so
from the signals of satellites; an Argos tag from the shift of its own signal received by satellites; a VHF
collar is located by people with antennas on the ground. A successful fix gives a **location** (longitude and
latitude, sometimes the height) and a **timestamp**. A failed fix gives only a time: under a dense canopy, in a
burrow or in a steep valley the receiver may not see enough satellites.

The locations of one animal, in time order, form its **track** (also called a *trajectory* or a *path*). Two
consecutive locations make a **step**, and a step has a length, a duration and a direction. Where one step meets
the next, the animal turned by some angle: the **turn angle**.

![A toy track: six locations, a step's length, a turn angle and a time lag](/img/tutorials/animal-movement/ideas/anatomy.png "The parts of a track. Each orange dot is a location at a time t. A step joins two consecutive locations; its length is the distance between them and its time lag the time between them. At each location the animal turns from the direction it was going (dashed) to the direction of the next step: the turn angle.")

!!! science "Key idea: a track"
    A track is the sequence of locations \( (x_i, y_i) \) of one animal at the times \( t_0 < t_1 < \dots < t_n \).
    The step from location \( i \) to \( i + 1 \) has

    - a **length** \( d_i \), the distance between the two locations;
    - a **time lag** \( \Delta t_i = t_{i+1} - t_i \);
    - a **speed** \( v_i = d_i / \Delta t_i \), which is the *average* speed over the step (the animal did not
      move in a straight line at a constant speed between two fixes);
    - a **direction** (or *heading*, *azimuth*): the angle of the step, measured from north;
    - a **turn angle**: the direction of the step minus that of the step before, between -180° and 180°
      (0° straight on, positive to the right).

    Everything about a track depends on the **sampling interval** \( \Delta t \): a fix every two minutes shows
    a wandering path, a fix every hour only its larger strokes.

The last point is the one to keep in mind through the whole series. The track is not the animal's path; it is a
sample of it. The real path runs between the locations, and a longer interval cuts more of its corners. A track
sampled every 10 minutes looks straighter and shorter than the same animal sampled every 2 minutes, so lengths,
speeds and turn angles can only be compared between tracks sampled at the same rate. Part 4 shows how to make the
rates equal.

### Why movement matters

Nathan et al. (2008) proposed a framework that most movement studies now follow, the *movement ecology
paradigm*. An individual's movement results from four components: its **internal state** (why move: hunger, the
search for a mate, fear), its **motion capacity** (how it moves: walking, flying), its **navigation capacity**
(where to go) and the **external factors** (the landscape, the weather, other animals). A track records only
the outcome, a sequence of locations, but by relating that outcome to the landscape and to time, ecologists infer
the components behind it.

GPS collars made this possible for many species. Since the 2000s they have become light and cheap enough for
animals of a few kilograms, and record a location every few minutes for weeks or months (Cagnacci et al., 2010).
Kays et al. (2015) describe tracking as "an eye on life and planet": the movements of thousands of animals,
shared in public archives, show how they respond to roads, cities, fences and a changing climate.

### Where tracking data live: Movebank

[Movebank](https://www.movebank.org) is the largest of these archives, run by the Max Planck Institute of Animal
Behavior: researchers store their studies there, and many publish them in its data repository. Its data model
(Kranstauber et al., 2011) gives every record the same columns, whatever the device, so that a file of one study
reads like a file of another:

| Column | What it holds |
|---|---|
| `timestamp` | the time of the record, in UTC (Coordinated Universal Time) |
| `location-long`, `location-lat` | the location, in degrees of longitude and latitude (WGS 84) |
| `individual-local-identifier` | the animal, by the name the study gave it |
| `tag-local-identifier` | the device it carried |
| `sensor-type` | the kind of sensor: gps, argos-doppler-shift, acceleration... |
| `visible` | false for the records Movebank hides (outliers flagged by people or by filters) |

A file holds many more columns (the battery's voltage, the number of satellites, the estimated error...) that
depend on the device. Movebank exports a study as a CSV file (text, one record per line), and Terraflux reads it
as it is.

### move2, amt and Terraflux

Two R packages are the reference tools of the field. **move2** (Kranstauber et al., 2024) reads Movebank's
files and handles tracks as tables of points: measures, filters, interpolation, lines. **amt**, the *animal
movement tools* (Signer et al., 2019), turns tracks into steps and builds the data of habitat selection
analyses. Terraflux's operators named `move.*` do what their functions do, and give **the same numbers**: they
are tested against both packages on the same data, to the last bit. Each operator's reference page names the
function it reproduces (`mt_read`, `track_resample`, `random_steps`...), so you can follow a paper's methods
written in R and repeat them here, step by step, in a graph.

## The data: eight fishers in New York

The fisher (*Pekania pennanti*, formerly *Martes pennanti*) is a weasel of the northern forests of North America,
about the size of a house cat: males weigh 3.5 to 6 kg, females about half as much. It was trapped out of much of its range in
the 19th and 20th centuries and has since come back, including to the patchy, suburban forests around Albany, New
York. LaPoint et al. (2013) put GPS collars on fishers there to see how they move through such a landscape, and
whether the corridors predicted by models of landscape resistance are the ones the animals actually use. Their
data are on Movebank (the study "Martes pennanti LaPoint New York") and are the example data of move2 and amt,
which makes them a good set to learn with: you can compare every number of this series with the packages'
documentation.

Download the file to follow along:

- [fishers.csv.gz](/data/tutorials/animal-movement/ideas/fishers.csv.gz) (700 kB): Movebank's export of the
  study, compressed with gzip. Terraflux reads `.csv`, `.csv.gz` and Movebank's `.zip` files directly.

The file has 47,347 records of eight animals, three females (F1 to F3) and five males (M1 to M5), between
February 2009 and May 2011:

```tftable
data: /data/tutorials/animal-movement/ideas/animals.csv
digits: 1
caption: The eight fishers. Records are the fix attempts; locations the ones that gave a position. The median interval is the time between two consecutive locations.
```

Two things stand out already. The animals were not followed at the same time, nor at the same rate: five of
them were fixed about every 2 minutes, two every 10 and one every 15. And many attempts failed: for six animals
less than two thirds of the records have a location, and for F3 fewer than half.

## When to use it

Reading and drawing the tracks is the first step of every movement analysis. The tracks then answer questions
such as:

- **How far and how fast do animals move?** Daily distances, speeds and the time spent moving or resting (part 2).
- **How much space do they use?** The extent of their movements and how it grows over time, an indication of a
  home range or of dispersal (part 4).
- **When are they active?** Movement by day and by night (part 4).
- **Which habitats do they select as they move?** Step selection functions compare each step taken with the
  steps the animal could have taken (part 5).
- **Where do they cross a barrier, a road or a river?** The steps that cross it, drawn as lines (part 2).

Terraflux does not yet estimate home ranges (minimum convex polygons, kernel densities, autocorrelated kernel
densities) or behavioural states (hidden Markov models); for those, R's amt, ctmm and moveHMM read the same files.
If your question is about *where a species can live* rather than *how individuals move*, see the
[species distribution models](../../species-distributions/ideas/); if it is about how the landscape resists
movement, see [circuit connectivity](../../circuit-connectivity/) and [grid distances](../../grid-distances/),
whose resistance surfaces movement data can calibrate and test.

## What you need

- **Tracks**: a Movebank file (CSV, CSV compressed with gzip, or a zip), or any table with a column of x (or
  longitude), one of y (or latitude), one of time and one naming the animal; [[move.as_tracks]] turns such a
  table, or a layer of points, into tracks (part 3).
- **Times** as text in the ISO 8601 form (`2010-03-05 17:02:00`, with an optional fraction of a second and a
  time zone such as `Z` or `-05:00`), read as UTC; or numbers (seconds, days...) for simulated tracks.
- **A coordinate system**: Movebank's locations are in longitude and latitude (EPSG:4326). The measures of
  part 2 work on the ellipsoid directly; part 3 shows when to project the tracks.
- **A map of the landscape** for the figures and, in part 5, for habitat selection. Here, ESA's WorldCover
  map of land cover for 2021 (10 m), which Terraflux downloads with [[data.download]].

## Build the model

The model has four nodes: one reads the file, one removes the records without a location, and two summarise the
tracks, as lines and as a table of the animals.

```tfgraph
#! caption: Read the fishers' tracks (node 1), keep the records with a location (node 2), draw each track as a line (node 3) and list the animals (node 4).
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "read"
op = "move.read_tracks"
params = { path = "fishers.csv.gz" }
note = "The fishers of LaPoint et al. (2013), Movebank's export"

[[nodes]]
id = "located"
op = "vector.filter"
inputs = { vector = "read.tracks" }
params = { expression = "not isnull(geometry.x)" }
note = "The records with a location"

[[nodes]]
id = "lines"
op = "move.track_lines"
inputs = { tracks = "located.vector" }

[[nodes]]
id = "animals"
op = "move.track_data"
inputs = { tracks = "read.tracks" }
```

<div class="steps" markdown>

1. **Read the tracks.** Add [[move.read_tracks]] (library subject **Animal movement**) and set its `path` to
   `fishers.csv.gz` in the Node pane (the button beside the setting opens a file dialog that lists Movebank's
   files). The node reads every record as a point in longitude and latitude, with all the file's columns as
   attributes. It reads the columns as R and pandas would: numbers as numbers, the flags `true` and `false` as 1
   and 0, the rest as text. It then **orders the records by animal and by time**, which every other `move`
   node needs; Movebank's files are usually in that order already, but not always (the setting `sort = false`
   keeps the file's order, as move2 does). The animal is the column `individual-local-identifier`; when an
   animal carried more than one tag during a study, the node adds a column `individual-tag-local-identifier`
   that names the animal and the tag, and takes the tracks from it, as move2 does.

2. **Keep the records with a location.** The 14,443 records whose fix failed have a time but no coordinates:
   they come into Terraflux as points with an empty geometry. Most measures are undefined for them, so drop them
   with [[vector.filter]] (subject **Vectors**) and the expression `not isnull(geometry.x)`: `geometry.x` is the
   longitude of each point, missing where there is no point, and `isnull` tests for a missing value. Connect
   `read.tracks` to the filter's input `vector`. The filter keeps the order of the records, so its output is
   still sorted.

3. **Draw each track as a line.** [[move.track_lines]] joins the locations of each animal in time order into one
   line, with the number of locations (`n_locations`) and the columns that do not change within the track (the
   animal's name, its tag, the study). Connect `located.vector` to its input `tracks`.

4. **List the animals.** [[move.track_data]] gives one row per animal with those same constant columns and no
   geometry: a table of the individuals, like move2's `mt_track_data`. It can read the records before or after
   the filter; here it reads them all.

</div>

Run the model with ++ctrl+enter++ on the last node, or from the command line:

```bash
terraflux run fishers.toml --out results
```

The command writes the model's outputs: add them to the file, one section per output, such as

```toml
[outputs.lines]
from = "lines.lines"
```

Vectors are written as GeoPackage files (`results/lines.gpkg`), which QGIS, R's sf and Python's geopandas read;
[[vector.write]] writes one where you choose, in any format GDAL writes.

!!! app "In the application"
    Beside any map, the viewer's **Vectors over the map** lists every vector output of the run: tick the tracks
    or their lines to draw them over the land cover, reprojected to it, and choose their style. Hover over a
    location to read its attributes (its `timestamp`, its animal, the collar's columns): the quickest way to check
    that the time and the animal are the columns you expect.

## Read the results

### The tracks on the map

![The tracks of the eight fishers over the land cover](/img/tutorials/animal-movement/ideas/tracks.png "The eight fishers' tracks (move.track_lines) over ESA WorldCover 2021 near Albany, New York: tree cover in green, cropland in yellow, built-up land in pink, water in blue. The river crossing from the west is the Mohawk, which joins the Hudson (running north to south near 73.7° W). The pink crosses are runways: Albany's airport near M3 and F3, a smaller airfield north of M4.")

The first look at a set of tracks answers simple questions that are easy to forget later. Are the locations
where they should be? (A location in the sea, or a thousand kilometres away, is a fix gone wrong or a collar
being shipped.) How large an area does each animal cover? Here the answer is a few kilometres: the fishers stay in
the forest patches between suburbs, roads and fields, and no track crosses the Hudson. Do the tracks overlap?
F3's track lies under M3's, which hides it at this scale, and M1's and F2's share the same woods two years apart.

### When each animal was followed

![The period each fisher was tracked](/img/tutorials/animal-movement/ideas/timeline.png "When each fisher was tracked, and how many locations it gave. Each animal was followed for two to ten weeks, in late winter or spring, in one of three years.")

The animals were followed for 2 to 10 weeks each, in winter and spring, in 2009, 2010 and 2011. An analysis that
pools them pools different seasons and years: a difference between two animals may be a difference between two
winters. Studies report this: the number of animals, the dates and the number of locations of each.

### A day of a fisher

![One day of M4: its locations, and the attempts that failed](/img/tutorials/animal-movement/ideas/one-day.png "One day of M4, from noon on 5 March 2010 to noon the next day (local time). Left: its 248 locations, coloured by the hours since noon. Right: the times of the fixes (green) and of the attempts that gave no location (grey).")

Zooming in on one day shows the sampling. M4 was located 248 times in 24 hours, but not regularly: the fixes come
in dense runs in the evening and at night and are sparse by day, and 33 attempts gave no location. The fisher
rested at the east end of its range in the afternoon (the tight cluster of dark points), set off in the evening,
looped through the woods all night, and was resting again in the west the next morning (the yellow points). The
steps between its fixes add up to about 12 km, though some of that is the error of the fixes (part 2). This is typical of GPS data: the interval is a setting of the collar, but the fixes actually
obtained depend on the animal (resting in a hollow tree blocks the signal) and on the collar's battery. Part 4
turns such irregular tracks into steps of a regular interval.

### What to report

From this first model a methods section takes: the number of animals and of locations; the period of tracking of
each; the scheduled interval of the fixes and the share of attempts that gave a location (the *fix success
rate*); and the source of the data, with its DOI. For these fishers: 8 animals, 32,904 locations from 47,347
attempts (69.5 %), intervals of 2, 10 or 15 minutes, February 2009 to May 2011 (LaPoint et al., 2013, with
the DOI of the data).

## Settings that change the answer

[[move.read_tracks]] has one setting besides the path: `sort`. With `sort = true` (the default) the records are
put in order of animal (in the order the animals first appear) and time; ties keep the file's order. Every
other `move` node expects this order and stops with an error when the times of a track go backwards. Keep the
default unless you know the file is ordered and want its order exactly.

The other `move` nodes have two settings in common, `time` and `track`: the columns of the time and of the
animal. They are found by their usual names (`timestamp`, `time`, `datetime`... for the time;
`individual-tag-local-identifier`, `individual-local-identifier`, `track`, `id`, `animal`... for the animal),
so with Movebank's files you never set them. With a table of your own, name the columns there, or set the
settings. Changing the track column changes the answer: tracks by tag instead of by animal split an animal whose
collar was replaced into two tracks, and the step between the old collar's last fix and the new one's first
disappears.

## Common pitfalls

- **Time zones.** Movebank stores times in UTC, and Terraflux reads every time without a zone as UTC. The
  fishers' winter night is between about 22:00 and 12:00 UTC. A table of your own in local time must say so (a zone
  such as `-05:00` at the end of each time), or the day and night of part 4 will be shifted.
- **Records without a location.** They are kept by [[move.read_tracks]], because some analyses need them (the
  fix success rate, or the other sensors of a collar). Drop them before measuring; the measures stop with an
  error if some remain, and tell you to.
- **Tracks out of order.** A table joined or appended from several files may not be sorted. Keep `sort = true`,
  or sort it with [[move.as_tracks]].
- **Different sampling rates.** Do not compare distances, speeds or turn angles between animals fixed at
  different intervals; resample them first (part 4).
- **Names of species.** The study, and the file's `individual-taxon-canonical-name`, use the former name
  *Martes pennanti*; the species has been *Pekania pennanti* since a genetic study moved it out of *Martes*.
  Search for both in the literature.

## Going further

- [Clean the tracks and measure them](../cleaning-and-measures/): Movebank's filters, duplicates, distances,
  speeds, directions and turns.
- [Projections, interpolation and simulated tracks](../projections-and-interpolation/).
- [Regular steps and track summaries](../steps-and-summaries/).
- [The data of step selection functions](../step-selection/).
- The operators' reference: [[move.read_tracks]], [[move.track_lines]], [[move.track_data]],
  [[vector.filter]].

## References

- Cagnacci, F., Boitani, L., Powell, R. A., & Boyce, M. S. (2010). Animal ecology meets GPS-based
  radiotelemetry: a perfect storm of opportunities and challenges. *Philosophical Transactions of the Royal
  Society B*, 365(1550), 2157-2162. https://doi.org/10.1098/rstb.2010.0107
- Kays, R., Crofoot, M. C., Jetz, W., & Wikelski, M. (2015). Terrestrial animal tracking as an eye on life and
  planet. *Science*, 348(6240), aaa2478. https://doi.org/10.1126/science.aaa2478
- Kranstauber, B., Cameron, A., Weinzerl, R., Fountain, T., Tilak, S., Wikelski, M., & Kays, R. (2011). The
  Movebank data model for animal tracking. *Environmental Modelling & Software*, 26(6), 834-835.
  https://doi.org/10.1016/j.envsoft.2010.12.005
- Kranstauber, B., Safi, K., & Scharf, A. K. (2024). move2: R package for processing movement data. *Methods in
  Ecology and Evolution*, 15(9), 1561-1567. https://doi.org/10.1111/2041-210X.14383
- LaPoint, S., Gallery, P., Wikelski, M., & Kays, R. (2013). Animal behavior, cost-based corridor models, and
  real corridors. *Landscape Ecology*, 28(8), 1615-1630. https://doi.org/10.1007/s10980-013-9910-0
- LaPoint, S., Gallery, P., Wikelski, M., & Kays, R. (2013). Data from: Animal behavior, cost-based corridor
  models, and real corridors. Movebank Data Repository. https://doi.org/10.5441/001/1.2tp2j43g
- Nathan, R., Getz, W. M., Revilla, E., Holyoak, M., Kadmon, R., Saltz, D., & Smouse, P. E. (2008). A movement
  ecology paradigm for unifying organismal movement research. *Proceedings of the National Academy of
  Sciences*, 105(49), 19052-19059. https://doi.org/10.1073/pnas.0800375105
- Signer, J., Fieberg, J., & Avgar, T. (2019). Animal movement tools (amt): R package for managing tracking data
  and conducting habitat selection analyses. *Ecology and Evolution*, 9(2), 880-890.
  https://doi.org/10.1002/ece3.4823
- Zanaga, D., Van De Kerchove, R., Daems, D., De Keersmaecker, W., Brockmann, C., Kirches, G., ... Arino, O.
  (2022). ESA WorldCover 10 m 2021 v200. Zenodo. https://doi.org/10.5281/zenodo.7254221
