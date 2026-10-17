---
title: Landforms with geomorphons
short: Geomorphons
description: Classify every cell of an elevation model into one of ten landforms (peak, ridge, spur, slope, hollow, valley, pit...) by looking along eight lines of sight, and learn how the search radius sets the scale of the landforms you map.
order: 30
level: intermediate
time: 40 min
ops: [raster.read, raster.reproject, surface.geomorphons, raster.terrain, view.plot, map.calc, table.write]
series: Continuous surfaces
part: 3
---

In this tutorial you classify the mountains of the [first part of the series](../terrain/) into landforms: every
cell of the elevation model becomes a peak, a ridge, a shoulder, a spur, a slope, a hollow, a footslope, a valley,
a pit or a flat. You will see how the method decides, by looking from each cell along eight lines of sight; build
the model in Terraflux; read the map, the patterns and the measures it gives; and move the search radius to watch
the landforms change from gullies and knolls to whole ridges and valleys. At the end you will have a categorical
map of landforms ready to use as a covariate in a species distribution or land change model, and you will know
which settings to report.

## The idea

The [terrain indices](../terrain/) of the first part (slope, curvature, the topographic position index) describe
the ground around a cell through a window: 3 × 3 cells for slope and curvature, a square or a circle for TPI. Two
things make windows awkward for classifying **landforms**. A small window sees only a small part of a landform
(a 3 × 3 window on a 30 m elevation model sees 90 m of a ridge that may be 2 km long), and a large window
averages the heights inside it, so that a narrow ridge on a broad slope disappears in the mean. To name a
landform you would rather know, from where you stand, *where the ground goes up and where it goes down*, as a
geomorphologist does in the field.

Jasiewicz & Stepinski (2013), building on a short paper of 2011 (Stepinski & Jasiewicz, 2011), proposed exactly
that. From each cell they look along the eight compass directions, up to a distance \( L \), and ask in each
direction whether the terrain is higher than the cell, lower, or about level. Eight answers, each one of three
symbols, make a **geomorphon** (a "form element"), and a geomorphon names a landform: if the terrain falls in all
eight directions, the cell is a **peak**; if it rises in all eight, a **pit**; if it falls in six and rises in
two (on either side of a crest line), a **ridge**. The angles they use build on the *openness* of Yokoyama et al.
(2002), a measure of how much of the sky or of the ground a point "sees" along lines of sight.

![The line of sight from one cell towards the east, with its steepest angle up and its steepest angle down](/img/tutorials/surface-metrics/geomorphons/line-of-sight.png "The elevation along the 20 cells (600 m) east of one cell of the map. From the cell, the steepest line of sight up the terrain (orange, +29.5°) is steeper than the steepest line down (blue, −9.8°): in this direction the terrain is higher, a +.")

!!! science "Key idea: one direction of a geomorphon"
    Along direction \( i \) (north, north-east, east...), consider every cell from the first one (or, with a
    *skip*, from a little further) up to the search distance \( L \). For each, the angle of the line of sight from
    the cell to it, above (positive) or below (negative) the horizontal: \( \beta = \arctan(\Delta z / d) \), with
    \( \Delta z \) the difference of elevation and \( d \) the horizontal distance. Keep the largest,
    \( \beta^{+}_i \), and the smallest, \( \beta^{-}_i \). (Jasiewicz & Stepinski express them as the **zenith
    angle** \( 90° - \beta^{+}_i \), between the vertical above the cell and its highest line of sight, and the
    **nadir angle** \( 90° + \beta^{-}_i \), between the vertical below the cell and its lowest one.)

    With a flatness threshold \( t \) (1° by default), the direction is:

    - **level (0)** if neither \( |\beta^{+}_i| \) nor \( |\beta^{-}_i| \) exceeds \( t \): the terrain looks flat;
    - **higher (+)** if \( |\beta^{+}_i| > |\beta^{-}_i| \): the terrain rises more steeply than it falls;
    - **lower (−)** if \( |\beta^{-}_i| > |\beta^{+}_i| \).

    This is the comparison of GRASS GIS's `r.geomorphon` (its default, `anglev1`), which Terraflux reproduces.

What makes the method fast and robust is what comes next. The eight symbols of a cell are counted: how many
directions are higher (\( n^{+} \)) and how many lower (\( n^{-} \)). The pair \( (n^{+}, n^{-}) \), whatever the
directions they point to, names one of **ten landforms** through a lookup table:

![The table that turns the counts of higher and lower directions into a landform](/img/tutorials/surface-metrics/geomorphons/lookup.png "The landform of a cell from the number of directions in which the terrain is higher (columns) and lower (rows), as r.geomorphon and Terraflux use it. The cells below the diagonal cannot occur (there are only eight directions).")

Read the table from its corners. No direction lower and none higher (top left): **flat**. All eight lower: **peak**;
all eight higher: **pit**. Six or seven lower: **ridge**; six or seven higher: **valley**. In the middle, where the
terrain rises on one side and falls on the other, is the **slope**; a slope with more directions falling than
rising bends outwards like the nose of a hill, a **spur**; one with more rising than falling bends inwards, a
**hollow**. Where many directions are level and a few fall, the cell is at the edge of a flat, about to drop: a
**shoulder**; where many are level and a few rise, it is at the foot of a rise: a **footslope**.

The real patterns behind those ten names are more than ten. Up to rotation and reflection (a ridge running north
and one running east are the same landform) there are 498 different geomorphons. Here is the commonest one of each
landform on the map of this tutorial:

![The commonest pattern of each landform on the map](/img/tutorials/surface-metrics/geomorphons/patterns.png "The commonest geomorphon of each landform in the Oregon mountains, at a search radius of 600 m: + where the terrain is higher in that direction, − where it is lower, 0 where it is level. Each is shown in one of its rotations. No cell of this map is flat.")

The slope (four + in a row, four − in a row) is a plane tilted one way; the spur has one − more, the hollow one +
more; the ridge has two + on opposite sides of six −.

Two things set geomorphons apart from window-based indices:

- **The scale is the search distance \( L \), and it can be large without blurring.** The method keeps only the
  steepest lines of sight, not a mean, so a narrow ridge is still a ridge with \( L \) = 2 km, and one map can
  hold landforms of very different sizes, a gully and a whole valley (Jasiewicz & Stepinski, 2013).
- **It is a pattern, not a number.** There is no threshold to choose on the output: the classes come from the
  counts, and they mean the same thing in a lowland and a mountain range (only the flatness threshold depends on
  the relief).

## When to use it

Use geomorphons when you want **named landforms** rather than continuous indices:

- **As covariates of species distribution models.** Many plants and animals sort themselves along slope position:
  ridge-top shrubs, valley-bottom riparian species, cliff nesters on spurs and peaks. A map of ten landforms (or a
  few indicators made from it, such as "ridge or peak") is easy to interpret in a fitted model. Amatulli et al.
  (2018) computed geomorphons with other topographic variables for the whole world, at 1 km and coarser, for
  environmental and biodiversity modelling, and Amatulli et al. (2020) at 90 m (Geomorpho90m), so that they can be
  used as predictors anywhere (see [Species distributions](../../species-distributions/)).
- **As drivers of land change.** Deforestation and farming follow the terrain: flat valley floors and gentle slopes
  are cleared first, ridges and steep spurs last. A landform map is a natural categorical driver for the models of
  [Land change simulation](../../land-change/) (as a static variable in Dinamica's weights of evidence, or a factor
  in a regression).
- **For soils and geomorphology.** Libohova et al. (2016) used geomorphons to predict landforms and soil properties
  on a glacial moraine in Indiana; Robaina et al. (2017) used them to classify the landforms of the state of
  Tocantins, Brazil.
- **To stratify field work.** Sampling plots by landform spreads them over ridges, slopes and valleys.

What geomorphons are not: they are not a hydrological analysis (a "valley" is a cell lower than its surroundings in
six or seven directions, not a stream; for flow, see [Terrain hydrology](../../terrain-hydrology/)), and they are not
a slope map: a gentle valley and a gorge are both valleys. Combine them with slope when steepness matters. For
continuous position, the TPI and the slope positions of the [first part](../terrain/) remain useful; for the
protected hilltops of Brazil's Forest Code, see the [next part](../hilltops/).

## What you need

- **An elevation model in projected coordinates.** Terraflux measures the lines of sight in map units, so the cells
  must be in metres: [[surface.geomorphons]] refuses a map in longitude and latitude ("geomorphons need a projected
  DEM (distances in map units): reproject it first"). Here: `orelevation.tif`, the SRTM elevation of south-western
  Oregon (about 30 m) of the [first part](../terrain/), saved as `elevation.tif` and reprojected to UTM zone 10N
  with [[raster.reproject]], exactly as there. For your own data use the UTM zone of the area (for most of Brazil,
  SIRGAS 2000 / UTM, EPSG:31978 to EPSG:31985) or an equal-area projection.
- **A resolution matched to your landforms.** The search radius is counted in cells by default: 20 cells is 600 m
  on a 30 m model and 20 m on a 1 m lidar model. Free global models (SRTM, Copernicus GLO-30, ALOS AW3D30) have
  cells of about 30 m.
- **A margin.** Lines of sight stop at the edge of the map (see the pitfalls): read the model with a margin of the
  search radius around your study area.

## Build the model

### Read, reproject and classify

<div class="steps" markdown>

1. In the library, open **IO** and click [[raster.read]]; set **path** to `elevation.tif` and rename the node `dem`
   (++ctrl+r++). With it selected, add [[raster.reproject]] from **Data preparation**, named `elevation`, with **crs** =
   `EPSG:32610`, **resolution** = `30`, **resampling** = `bilinear` and **extent** =
   `[500010, 4760850, 508110, 4771920]` (the reason for each is in the [first part](../terrain/#read-and-reproject-the-elevation)).
2. With `elevation` selected, open the subject **Surface metrics** and click [[surface.geomorphons]]: it comes
   connected through its `dem` input. Rename it `landforms` and set **search** to `20`.
3. Press ++ctrl+enter++. The panel under the canvas shows the `forms` output: the ten landforms with their names in
   the legend, in the colours of GRASS GIS (the application recognises this output). Its header lists the node's
   other outputs (`ternary`, `share`...) to choose from.

</div>

```tfgraph
#! caption: The elevation model reprojected to 30 m and classified into landforms with a search radius of 20 cells (600 m).
#! highlight: landforms
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "elevation.tif" }
note = "SRTM elevation, 1 arc-second, longitude and latitude."

[[nodes]]
id = "elevation"
op = "raster.reproject"
inputs = { raster = "dem.raster" }
params = { crs = "EPSG:32610", resolution = 30, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }
note = "Square cells of 30 m: geomorphons need metres."

[[nodes]]
id = "landforms"
op = "surface.geomorphons"
inputs = { dem = "elevation.raster" }
params = { search = 20 }
note = "Eight lines of sight of 600 m from every cell."
```

Node 3 has five outputs you will use, and eight more with **measures** (below):

| Output | What it holds |
|---|---|
| `forms` | the landform of each cell, 1 flat, 2 peak, 3 ridge, 4 shoulder, 5 spur, 6 slope, 7 hollow, 8 footslope, 9 valley, 10 pit |
| `ternary` | the geomorphon itself: its eight symbols as a number in base 3, the smallest of its rotations and reflections (498 possible codes between 0 and 6560) |
| `positive`, `negative` | the directions in which the terrain is higher (or lower) than the flatness threshold, as eight bits, the smallest of their rotations (0 to 255) |
| `share` | a table with a row per landform: `form` (its code), `cells`, `share` (a fraction of the cells with a form) |

The settings of [[surface.geomorphons]], and what they mean on the ground:

- **search**: the outer search radius \( L \), 3 by default, in cells (or in map units with **units** = `map`).
  This is the scale of the landforms: a valley is a valley within this distance. With **units** = `map`,
  **search** = `600` gives exactly the same map as 20 cells of 30 m here (every cell identical; it is GRASS's `-m`).
- **skip**: an inner radius not looked at, 0 by default. With **skip** = 3 the lines of sight start 4 cells away, so
  that bumps and noise within 90 m of a cell do not decide its form: GRASS's manual describes it as removing forms
  smaller than the skip.
- **flat**: the flatness threshold \( t \) in degrees, 1 by default. Lines of sight within \( t \) of the horizon
  count as level. Raise it on rough or noisy models to find flats; lower it on very coarse ones (GRASS's manual
  suggests a smaller threshold for cells larger than about 1 km).
- **dist**: the flatness distance, 0 (none) by default. Beyond it the threshold is no longer an angle but a height,
  \( \tan(t) \times \text{dist} \): far points count as higher or lower when they differ by more than that height,
  however small their angle. It keeps distant low hills from being "level" on large search radii. It must lie
  between **skip** and **search**, or it is ignored.
- **comparison**: how the two angles decide a direction: `anglev1` (the original, the default), `anglev2` and
  `anglev2_distance`, which GRASS describes as better handling of the threshold and of angles up and down that are
  exactly equal (the second breaking such ties by the distance of the two points). On this map `anglev2` changes
  1.2 % of the cells: all 185 shoulders become slopes, nearly all 284 footslopes become valleys, and some slopes
  become valleys or hollows. Expect larger differences on models with whole-metre heights and on terraced or
  synthetic surfaces, where equal angles are common.
- **extended**: GRASS's "extended form correction" (`-e`). When the search radius is larger than 10 cells, a cell
  classified as a ridge, peak, shoulder or footslope is looked at again with half the radius (at least 4 cells): a
  ridge or a peak takes the form of the shorter look, and a shoulder or footslope becomes flat if the shorter look
  finds it flat. At 40 cells it changes 2.9 % of this map: 29 % of the peaks become ridges and 16 % of the ridges
  become spurs, crests that looked sharper from far away than from close by.
- **measures**: also compute the eight measures of each geomorphon (below); off by default because they take time.

### Show it over the relief, save it, and make a covariate

<div class="steps" markdown>

1. Select `elevation` and add [[raster.terrain]] with **measure** = `hillshade`, named `shade`. Add a [[view.plot]]
   after it (**title** = `Relief`), then a second [[view.plot]] named `map` with `landforms`'s `forms` connected to
   its `value` and `relief`'s `shown` to its `under`, **opacity** = `0.75`: the landforms drawn over the relief.
2. Add a [[table.write]] after `landforms`, its `table` input from the `share` output, **path** = `landforms.csv`.
3. Add a [[map.calc]] named `crests` (subject **Map algebra**), connect `landforms`'s `forms` to an input you name
   `f`, and write `if f == 2 or f == 3 then 1 else 0`: 1 on peaks and ridges, 0 elsewhere.

</div>

```tfgraph
#! caption: The landforms over the relief, their shares written to a file, and an indicator of crests (peaks and ridges) for a model of species or land change.
#! highlight: shade, relief, map, save, crests
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "elevation.tif" }

[[nodes]]
id = "elevation"
op = "raster.reproject"
inputs = { raster = "dem.raster" }
params = { crs = "EPSG:32610", resolution = 30, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }

[[nodes]]
id = "landforms"
op = "surface.geomorphons"
inputs = { dem = "elevation.raster" }
params = { search = 20 }

[[nodes]]
id = "shade"
op = "raster.terrain"
inputs = { raster = "elevation.raster" }
params = { measure = "hillshade" }

[[nodes]]
id = "relief"
op = "view.plot"
inputs = { value = "shade.raster" }
params = { title = "Relief" }

[[nodes]]
id = "map"
op = "view.plot"
inputs = { value = "landforms.forms", under = "relief.shown" }
params = { title = "Landforms", opacity = 0.75 }

[[nodes]]
id = "save"
op = "table.write"
inputs = { table = "landforms.share" }
params = { path = "landforms.csv" }

[[nodes]]
id = "crests"
op = "map.calc"
inputs = { f = "landforms.forms" }
params = { expression = "if f == 2 or f == 3 then 1 else 0" }
note = "1 on peaks and ridges."
```

=== "Application"

    Press ++f5++ to run the whole model. The tab *Landforms* shows the classes over the relief; the legend names
    them. To keep the map, use *Save output* in its tab or add a [[raster.write]] node after `landforms`'s `forms`.

=== "Command line"

    Save the model as `landforms.toml` beside `elevation.tif`, add a [[raster.write]] node for the maps you want on
    disk, and run:

    ```bash
    terraflux check landforms.toml
    terraflux run landforms.toml --out output
    ```

The indicator in node 8 is how a categorical map usually enters a regression: one 0/1 map per landform (or group
of landforms) you want a coefficient for. Most species distribution methods also accept the `forms` map directly as
a categorical predictor; then the coefficients are relative to a reference class (choose a common one, here the
slope).

## Read the results

### The landform map

![The landforms of the Oregon mountains at a search radius of 600 m](/img/tutorials/surface-metrics/geomorphons/forms.png "The ten landforms at a search radius of 20 cells (600 m), over the hillshade, in GRASS GIS's colours, with the share of each in the legend. Ridges (red) and valleys (blue) draw the skeleton of the relief; slopes (yellow) fill the space between, spurs (orange-yellow) bulge from the ridges and hollows (green) cut into them.")

The map reads like a sketch of the relief. Red **ridges** run along every crest and its side spurs, ending in dark
**peaks** on the summits; blue **valleys** follow the streams from the main valley in the south-east up into each
side valley, ending in black **pits** where a valley floor is lower than everything around it within 600 m (small
closed depressions, or the elevation model's own errors). Between them, **slopes** (36 %) are flanks that rise on
one side and fall on the other; **spurs** (22 %) and **hollows** (18 %) are flanks that bend outwards and inwards.
Shoulders and footslopes are rare and **flat** cells absent: in these mountains almost no line of sight stays within
1° of the horizon.

```tftable
data: /data/tutorials/surface-metrics/geomorphons/share.csv
labels: { form: "Code", name: "Landform", cells: "Cells", "share (%)": "Share (%)" }
digits: 2
caption: "The share output of node 3 (search = 20 cells), as written by table.write, with the names added. 98,356 cells have a form: all but the outermost ring of the map."
```

A good sign: ridges and valleys form continuous lines, as on a topographic map, and their share is in proportion to
the relief (here, about 11 % ridges and 10 % valleys). A bad sign: a speckle of peaks and pits all over the map, or
ridges broken into dots, means the search radius is too small for the elevation model's noise; raise **search**, or
use **skip**.

### Patterns

The `ternary` output keeps the geomorphon of each cell. Of the 498 possible patterns, only 63 occur on this map, and
ten account for 97 % of the cells:

```tftable
data: /data/tutorials/surface-metrics/geomorphons/patterns.csv
labels: { code: "Ternary code", signs: "The eight directions (one rotation)", higher: "Higher", lower: "Lower", form: "Landform", "share (%)": "Share (%)" }
digits: 2
caption: "The ten commonest geomorphons of the map at search = 20 cells. The code is the pattern as a number in base 3 (a digit per direction: 0 lower, 1 level, 2 higher), the smallest of its rotations and reflections."
```

Code 80 (four directions up, then four down) is a plane, the commonest form of a mountain flank; code 0 (all eight
down) is a peak and code 6560 (all eight up) a pit. Patterns add detail to the forms: two ridges with the same counts
can differ in where their higher directions are, side by side (a ridge ending in a spur) or opposite (a crest
between two valleys). Use the `ternary` map when ten classes are too coarse, for instance to tell one-sided from
two-sided ridges, or as a texture of the terrain. The `positive` and `negative` maps keep only the directions that
rise (or fall), each as eight bits up to rotation: 0 for none, 255 for all.

### The measures

With **measures** = true, [[surface.geomorphons]] also describes each geomorphon by the points that decide its
directions (the farthest point of steepest ascent in a higher direction, of steepest descent in a lower one):

- `intensity`: the mean height of the cell above those points, in metres: positive on peaks and ridges (here
  +83 m on average on peaks, +54 m on ridges), near 0 on slopes, negative in valleys (−51 m) and pits (−78 m);
- `exposition`: minus the height difference to the most different of those points, in metres;
- `range` and `variance`: the range and the variance of their heights;
- the shape of the octagon those points draw around the cell (a level direction counts at the search radius):
  `elongation` (the ratio of its long and short sides), `azimuth` (the direction of its long axis, in degrees),
  `extend` (its area, as a share of the full octagon of the search radius) and `width` (its short side, in metres).

```tfgallery
columns: 2
caption: Four measures of the geomorphons at search = 20 cells, over the hillshade.
images:
  - { src: /img/tutorials/surface-metrics/geomorphons/intensity.png, caption: "Intensity: how far the cell stands above (red) or below (blue) the terrain that shapes it" }
  - { src: /img/tutorials/surface-metrics/geomorphons/range.png, caption: "Range: the relief within the cell's view, highest on the big flanks" }
  - { src: /img/tutorials/surface-metrics/geomorphons/extend.png, caption: "Extend: small where the view is closed in by nearby relief, large on open crests" }
  - { src: /img/tutorials/surface-metrics/geomorphons/elongation.png, caption: "Elongation: high along narrow ridges and valleys" }
```

GRASS's manual marks the measures as experimental. Intensity is the easiest to read: a continuous companion of the
forms, close in meaning to the TPI but measured over the lines of sight instead of a window.

### How Terraflux compares with GRASS

[[surface.geomorphons]] runs landschaft's port of `r.geomorphon` from GRASS GIS 8.4. On the six reference elevation
models kept with landschaft's tests (real SRTM heights, heights rounded to whole metres, holes in the data, cells of
20 × 30 m, search in map units, the extended correction, all three comparisons), its forms and patterns are those of
GRASS in all 73,500 cells where both give a value. It corrects four errors of `r.geomorphon` 8.4: the lines of sight
of the cells in GRASS's bottom rows stop one row short of the map's last row; GRASS's `positive` and `negative`
patterns add the numbers of the directions instead of setting their bits; its extended correction keeps the skip in
its second, shorter look although it means to drop it; and the shape of a geomorphon whose first direction (north-east)
has no point comes out as zeros. Outside these cases the values are GRASS's.

### How to report

Give the elevation model (source, resolution, projection), the method (geomorphons, Jasiewicz & Stepinski, 2013,
as implemented in GRASS GIS's `r.geomorphon`, here through Terraflux), the **search radius in cells and in metres**,
the skip, the flatness threshold and distance, the comparison, and whether the extended correction was used. Show
the map over a hillshade with the ten names in the legend, and give the share of each landform.

## Settings that change the answer

The search radius is the setting that matters. Move the slider from 3 cells (90 m) to 80 cells (2.4 km):

![Legend of the landforms](/img/tutorials/surface-metrics/geomorphons/legend.png "The ten landforms in GRASS GIS's colours.")

```tfslider
label: Search radius (m)
caption: The landforms at six search radii over the same relief. At 90 m the map is mostly slope, with short ridges and valleys on every bump and gully; from 600 m on, ridges and valleys become continuous lines and the map changes little. The chart shows the share of each landform against the radius.
frames:
  - { value: 90, src: /img/tutorials/surface-metrics/geomorphons/forms_3.png, caption: "search = 3 cells (90 m), the default", stats: { Slope: "52.1%", Ridge: "4.6%", Valley: "5.1%", Peak: "0.3%" } }
  - { value: 150, src: /img/tutorials/surface-metrics/geomorphons/forms_5.png, caption: "search = 5 cells (150 m)", stats: { Slope: "44.3%", Ridge: "6.5%", Valley: "7.2%", Peak: "0.6%" } }
  - { value: 300, src: /img/tutorials/surface-metrics/geomorphons/forms_10.png, caption: "search = 10 cells (300 m)", stats: { Slope: "38.0%", Ridge: "9.5%", Valley: "9.0%", Peak: "0.9%" } }
  - { value: 600, src: /img/tutorials/surface-metrics/geomorphons/forms_20.png, caption: "search = 20 cells (600 m)", stats: { Slope: "36.2%", Ridge: "11.0%", Valley: "9.9%", Peak: "1.4%" } }
  - { value: 1200, src: /img/tutorials/surface-metrics/geomorphons/forms_40.png, caption: "search = 40 cells (1.2 km)", stats: { Slope: "34.5%", Ridge: "12.1%", Valley: "10.2%", Peak: "1.9%" } }
  - { value: 2400, src: /img/tutorials/surface-metrics/geomorphons/forms_80.png, caption: "search = 80 cells (2.4 km)", stats: { Slope: "34.0%", Ridge: "12.9%", Valley: "10.1%", Peak: "2.3%" } }
start: 3
chart:
  type: line
  data: /data/tutorials/surface-metrics/geomorphons/search.csv
  x: search_m
  y: [Slope, Spur, Hollow, Ridge, Valley, Peak, Pit]
  colors: { Slope: "#b8b000", Spur: "#d9a441", Hollow: "#7fa80f", Ridge: "#c80000", Valley: "#0000ff", Peak: "#380000", Pit: "#6b6b9a" }
  xlabel: Search radius (m)
  ylabel: "% of the cells"
  height: 280
```

What to notice:

1. **Small radii see small landforms.** At the default of 3 cells, a ridge must be a ridge within 90 m: only the
   sharpest crests qualify, and every small gully is a valley. Half the map is "slope".
2. **The map settles.** From about 600 m on, the shares change by a point or two: the landforms of these mountains
   are a few hundred metres across, and longer lines of sight add few new ones. Look for that plateau on your own
   data: it marks the scale of the main landforms. GRASS's manual puts it this way: a radius \( L \) identifies
   correctly the landforms smaller than \( L \), and a large \( L \) identifies landforms of many sizes at once.
3. **Unlike a window, a larger radius costs no cells.** Every frame has forms on 98.7 % of the map: lines of sight
   simply stop at the edge. The price is hidden: near the edges, forms are decided by shorter lines than elsewhere.

The flatness threshold matters on gentle relief. In these steep mountains no cell becomes flat even at 6°, but
the slopes grow at the expense of ridges and valleys, and a few shoulders and footslopes appear where gentle crests
and valley floors meet steeper flanks:

```tfgallery
columns: 3
caption: "The landforms at search = 20 cells with flatness thresholds of 1°, 3° and 6° (colours as above)."
images:
  - { src: /img/tutorials/surface-metrics/geomorphons/flat_1.png, caption: "flat = 1° (default): slope 36.2 %" }
  - { src: /img/tutorials/surface-metrics/geomorphons/flat_3.png, caption: "flat = 3°: slope 37.1 %" }
  - { src: /img/tutorials/surface-metrics/geomorphons/flat_6.png, caption: "flat = 6°: slope 43.1 %, shoulders and footslopes 1.8 %" }
```

On a floodplain, a coastal plain or a plateau the threshold decides how much of the map is flat; there, try 1° to 3°
and check the flats against what you know of the ground.

**skip** changes more than one would think: with **skip** = 3 at 20 cells, 37 % of the cells change form, and ridges
(16.4 %) and valleys (15.5 %) widen at the expense of slopes (26.3 %): the form of a cell is now decided by the
terrain from 120 m away, not by its nearest neighbours. Use it to remove noise or features smaller than the skip, and
report it.

## Common pitfalls

- **Longitude and latitude.** The operator refuses them; reproject first, with `bilinear` resampling (not `near`,
  which makes terraces whose edges become false shoulders and footslopes).
- **The default radius.** 3 cells is a very local look (90 m on a 30 m model). Choose the radius in metres from the
  landforms that matter to your question, and convert it to cells for your resolution.
- **Comparing maps of different resolutions.** 20 cells at 30 m (600 m) and 20 cells at 90 m (1.8 km) are different
  scales; use **units** = `map` to keep the same distance.
- **Surface models.** Radar elevation models (SRTM in places, other "DSMs") include the canopy and buildings: a forest
  edge becomes a step, a clearing a pit. Use a terrain model (bare ground) where one exists, for instance ANADEM for
  South America (Laipelt et al., 2024).
- **Edges.** Lines of sight stop at the map's edge, so cells near it are classified from fewer and shorter lines.
  Read a margin of the search radius around your study area and cut the map afterwards.
- **Pits.** Small pits in valleys are often errors of the elevation model, not real depressions. Do not interpret
  them without checking.
- **Treating codes as numbers.** The forms are classes: a mean landform of 5.9 means nothing. Use shares,
  indicators, or the map as a categorical variable.

## Going further

- [Terrain from an elevation model](../terrain/): slope, aspect, TPI and Weiss's slope positions, the window-based
  counterpart of geomorphons.
- [Hilltops protected by Brazil's Forest Code](../hilltops/): summits, saddles and the hilltop APP, the next part.
- [Species distributions](../../species-distributions/) and [Land change simulation](../../land-change/): use the
  landforms as covariates.
- The operators' reference: [[surface.geomorphons]], [[raster.reproject]], [[raster.terrain]], [[map.calc]].

## References

- Amatulli, G., Domisch, S., Tuanmu, M.-N., Parmentier, B., Ranipeta, A., Malczyk, J., & Jetz, W. (2018). A suite of global, cross-scale topographic variables for environmental and biodiversity modeling. *Scientific Data*, 5, 180040. https://doi.org/10.1038/sdata.2018.40
- Amatulli, G., McInerney, D., Sethi, T., Strobl, P., & Domisch, S. (2020). Geomorpho90m, empirical evaluation and accuracy assessment of global high-resolution geomorphometric layers. *Scientific Data*, 7, 162. https://doi.org/10.1038/s41597-020-0479-6
- Jasiewicz, J., & Stepinski, T. F. (2013). Geomorphons — a pattern recognition approach to classification and mapping of landforms. *Geomorphology*, 182, 147–156. https://doi.org/10.1016/j.geomorph.2012.11.005
- Laipelt, L., Comini de Andrade, B., Collischonn, W., de Amorim Teixeira, A., Paiva, R. C. D., & Ruhoff, A. (2024). ANADEM: A digital terrain model for South America. *Remote Sensing*, 16(13), 2321. https://doi.org/10.3390/rs16132321
- Libohova, Z., Winzeler, H. E., Lee, B., Schoeneberger, P. J., Datta, J., & Owens, P. R. (2016). Geomorphons: Landform and property predictions in a glacial moraine in Indiana landscapes. *Catena*, 142, 66–76. https://doi.org/10.1016/j.catena.2016.01.002
- Robaina, L. E. de S., Trentin, R., Cristo, S. S. V. de, & Sccoti, A. A. V. (2017). Application of the concept of geomorphons to the landform classification in Tocantins State, Brazil. *Raega – O Espaço Geográfico em Análise*, 41, 37. https://doi.org/10.5380/raega.v41i0.48724
- Stepinski, T. F., & Jasiewicz, J. (2011). Geomorphons — a new approach to classification of landforms. In *Proceedings of Geomorphometry 2011* (pp. 109–112). Redlands, CA.
- Yokoyama, R., Shirasawa, M., & Pike, R. J. (2002). Visualizing topography by openness: A new application of image processing to digital elevation models. *Photogrammetric Engineering and Remote Sensing*, 68(3), 257–265.
