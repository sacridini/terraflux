---
title: "Linkage Mapper: corridors, pinch points and barriers"
short: Linkage Mapper
description: Link the core areas of a habitat by least-cost corridors, find where the movement between them pinches into a narrow passage, and find where restoring a small circle of habitat would shorten a link most, with the three tools of Linkage Mapper.
order: 70
level: intermediate
time: 45 min
template: "Linkage Mapper: corridors, pinch points and barriers"
ops: [raster.sieve, raster.label, linkage.pathways, linkage.pinchpoints, linkage.barriers, raster.calc, view.plot]
---

Conservation planners who have a set of core areas (reserves, or the large patches of a habitat) usually ask three
questions in a row. Which cores should be linked, and along which routes? Within those routes, where does movement
squeeze into a narrow passage that must not be lost? And where is the landscape holding a link back, so that
restoring a small area would shorten it most? **Linkage Mapper** (McRae and Kavanagh, 2011) answers them with three
tools, and Terraflux has all three as operators. In this tutorial you build the model of the template **Linkage
Mapper: corridors, pinch points and barriers**: you pick the cores from a habitat drawn by chance, make a resistance
surface from its quality, and run [[linkage.pathways]], [[linkage.pinchpoints]] and [[linkage.barriers]] in turn.
At the end you will have a map of corridors, a map of current in them and a map of where restoration pays most, and
you will know how to read each and which settings change them.

## The idea

### Cores, resistance and cost-weighted distance

A **core area** is a place a species can live and that is worth connecting: a protected area, a population, or a
patch of habitat large enough to hold one. Between cores lies the **matrix**, land that is harder to cross. A
**resistance surface** says how much harder: a raster whose every cell holds the cost, for the species, of moving
through it (1 in good habitat, 10 in pasture, 100 in a town). Resistances are relative; what matters is how much
dearer one cell is than another.

On a resistance, the **cost-weighted distance** (CWD) from a core to a cell is the least total cost of any route
from the core to that cell. Each step to one of the 8 neighbours costs the mean resistance of the two cells times
the step's length (the length is the cell's size, or √2 times it on a diagonal). This is the same idea as the
least-cost links of [part 4](../least-cost-links/) of this series; Linkage Mapper uses it for more than a distance.

### The three questions

!!! science "Key idea: three steps, three answers"
    1. **Linkage Pathways** (McRae and Kavanagh, 2011) decides which pairs of cores to link and draws their
       **corridors**. For a link between cores A and B, the **least-cost distance** (LCD) is the cheapest route
       from one to the other. For every cell, the **normalized** cost distance is \(\text{CWD}_A + \text{CWD}_B -
       \text{LCD}\): how much dearer a route through that cell is than the best route. It is 0 along the least-cost
       path and grows away from it, so a corridor is the set of cells under a cutoff (`width`).
    2. **Pinchpoint Mapper** (McRae, 2012) lays Circuitscape (McRae et al., 2008) over each corridor. Current
       flows through every route in proportion to how easy it is, and concentrates where the corridor narrows: a
       **pinch point**, a place where losing a little habitat cuts a lot of movement.
    3. **Barrier Mapper** (McRae et al., 2012) asks the reverse: if a circle of habitat of radius \(R\) were
       restored around a cell, how much shorter would the link become? The gain is the LCD less the cost of the
       route that goes through the restored circle, per unit of distance restored. High values mark **barriers**:
       places where a small restoration shortens a link a lot.

Pinch points and barriers are different things. A pinch point is *good* land that the movement depends on, to be
protected. A barrier is *bad* land in the way, to be restored. The first map is where you defend, the second is
where you invest.

### Which cores are linked

Linking every pair of cores gives a tangle of routes that mostly run through other cores. Linkage Mapper links the
cores that are **neighbours**: the pairs whose cost allocation zones touch (the zone of a core being the cells
nearer to it, in cost, than to any other core). This is the setting `adjacency = "cost"`, the default; `"euclidean"`
uses straight-line zones and `"all"` every pair. A link can also be dropped when its path crosses a third core
(`drop_crossing`), when it is longer than a limit (`max_euclidean`, `max_cwd`) or when it is not among each
core's `nearest` k. The table of links lists every candidate pair with a `kept` column and, for the others, the
reason.

## When to use it

- You know the cores (reserves, or the large patches of a habitat) and need the routes and **where to act**:
  which corridors to protect, which pinch points to secure, where to restore.
- You have, or can make, a resistance surface for one species or one guild (see the
  [resistance surfaces](/tutorials/circuit-connectivity/resistance-surfaces/) part of the circuit series).
- You want output that planners already read: Linkage Mapper is one of the most used tools in corridor planning.

It is not the tool when you have no cores (continuous habitat: use [Omniscape](/tutorials/circuit-connectivity/omniscape/)),
or when you want one number for a network (use the [connectivity indices](../connectivity-indices/) of a habitat
graph), or the current between all pairs at once
([Circuitscape](/tutorials/circuit-connectivity/circuitscape/)).

!!! warning "How it differs from Linkage Mapper"
    Terraflux's operators follow Linkage Mapper's algorithms but are not its code, and three things differ:

    - The cores are a raster of ids, not polygons. The `euclidean` distance in the links table is between the
      centres of the two cores' nearest cells, not between polygon edges.
    - Pinchpoint Mapper's "all pairs" scenario is not made. `mode = "all_to_one"` runs one circuit over the union
      of the corridors with every core a focal region, which is a different analysis.
    - Barrier Mapper's optional trim of the circles by the resistance is not made.

    The cost-weighted distances follow ArcGIS's convention, as Linkage Mapper's. The operators are tested against
    scikit-image's least-cost routes, the library's Circuitscape and numpy.

## What you need

- **Cores**: a raster of whole ids (1, 2, 3...), 0 or no value elsewhere. If you have reserves as polygons, burn
  them into a raster of ids on the resistance's grid.
- **A resistance** on the same grid, above 0 everywhere it can be crossed (0 or no value is a barrier the species
  cannot cross). A projected coordinate system, so that the cell size is in metres.

The template draws both from a habitat quality map by chance, so that you can run it and change it at once. In the
application choose **Templates > Connectivity > Linkage Mapper: corridors, pinch points and barriers**.

## Build the model

### 1. The cores and the resistance

```tfgraph
#! caption: A habitat's quality drawn by chance, its large patches as numbered cores, and a resistance made from the quality.
#! numbered: true
#! highlight: large, cores, resistance
format = "terraflux/1"

[[nodes]]
id = "quality"
op = "nlm.gaussian_field"
params = { rows = 80, columns = 80, range = 12, seed = 31 }
note = "Quality from 0 to 1 (in its place, a raster.read of a suitability map)."

[[nodes]]
id = "habitat"
op = "raster.calc"
inputs = { q = "quality.map" }
params = { expression = "q > 0.7" }
note = "The habitat: quality above 0.7."

[[nodes]]
id = "large"
op = "raster.sieve"
inputs = { map = "habitat.map" }
params = { threshold = 25, neighbours = 8 }
note = "Patches of fewer than 25 cells merged into what is around them."

[[nodes]]
id = "cores"
op = "raster.label"
inputs = { map = "large.map" }
params = { neighbours = 8, classes = [1] }
note = "Every core numbered, no value elsewhere."

[[nodes]]
id = "resistance"
op = "raster.calc"
inputs = { q = "quality.map" }
params = { expression = "1 + 99 * (1 - q) ** 3" }
note = "1 in the best habitat, up to 100 where there is none."
```

Node 1 is [[nlm.gaussian_field]], a neutral landscape model: a smooth random surface that stands for a habitat's
quality. In your study it is a [[raster.read]] of a suitability map, or of your reserves and land cover.

Node 2 makes the habitat, the cells of a quality above 0.7. Node 3, [[raster.sieve]], is the size filter: it
merges every patch of fewer than 25 cells (`threshold`, with `neighbours = 8` so that diagonal cells join a patch)
into the largest patch around it, so that only the large patches remain as habitat. How large a core must be
depends on the species: record where your threshold comes from. Node 4, [[raster.label]], gives each patch a whole
number; `classes = [1]` labels only the habitat and leaves the rest without a value, which is what the cores' input
asks for. With the seed of the template there are four cores.

Node 5 makes the resistance from the same quality: \(1 + 99(1 - q)^3\) is 1 in the best habitat and rises steeply
as quality falls, so a cell of medium quality is moderately dear (a quality of 0.5 costs 13) and a cell with none costs almost 100. This is a
choice, not a fact: take your resistance from the literature, from expert opinion or from movement data, and say
which.

### 2. Linkage Pathways: the links and the corridors

Add [[linkage.pathways]] and connect the cores and the resistance to its `cores` and `resistance` inputs. The
settings that matter:

- `adjacency`: which cores are neighbours. The default, `"cost"`, links those whose cost allocation zones touch.
- `width`: the cutoff of the corridors, in **cost units**, not metres. The template uses 60: a cell is in the
  corridor if a route through it costs at most 60 more than the least-cost path. A narrow width keeps a thin band
  around each path, a wide one lets the corridors fatten and merge. Choose it as a fraction of the links' LCDs.
- `max_cwd`, `max_euclidean`, `nearest`, `drop_crossing`: drop links that are too long or redundant.

It gives four outputs: `links` (a table), `corridors` (a map), `paths` (the least-cost lines, through the
cells' centres) and `cwd` (the cost-weighted distance to the nearest core).

### 3. Pinchpoint Mapper: where the movement squeezes

```tfgraph
#! caption: The pinch points of the corridors, from the same cores, the same resistance and the links of Linkage Pathways.
#! numbered: true
#! highlight: pinchpoints
format = "terraflux/1"

[[nodes]]
id = "cores"
op = "raster.read"
params = { path = "cores.tif" }
note = "The cores (ids)."

[[nodes]]
id = "resistance"
op = "raster.read"
params = { path = "resistance.tif" }
note = "The resistance."

[[nodes]]
id = "pathways"
op = "linkage.pathways"
inputs = { cores = "cores.raster", resistance = "resistance.raster" }
params = { adjacency = "cost", width = 60 }
note = "The links and corridors."

[[nodes]]
id = "pinchpoints"
op = "linkage.pinchpoints"
inputs = { cores = "cores.raster", resistance = "resistance.raster", links = "pathways.links" }
params = { width = 60, mode = "pairwise" }
note = "Circuitscape inside each corridor."
```

Add [[linkage.pinchpoints]] with the same `cores` and `resistance`, and the `links` table of Linkage Pathways. For
each kept link it runs Circuitscape between the link's two cores, **inside the link's corridor only**: the cells
beyond `width` are a barrier. Its `current` is the largest of the links' currents in each cell (the cells of the
cores have no value, as in Linkage Mapper). `width` here is the same cutoff as before, and it is the setting that
matters most: with a wide band the current spreads over many routes and no pinch point stands out, with a narrow
one it is forced through the best cells and shows the passages the link depends on. Use the same value as in
Linkage Pathways unless you want to ask that question.

Its `links` table adds, per link, the effective resistance (Circuitscape's, in cost units) and
`cwd_to_resistance`, the link's LCD over it. A link with a low ratio has a corridor much worse than its best path:
a bottleneck.

### 4. Barrier Mapper: where restoring pays most

Add [[linkage.barriers]], again with the cores, the resistance and the links. The settings:

- `min_radius`, `max_radius` and `step`: the radii of the restored circles, **in map units** (metres in a projected
  coordinate system). The template's cells are 1 unit across, so the radii 2, 4 and 6 are in cells. The ring
  around each cell is 1 map unit thick, so use a multiple of the cell size.
- `restored`: the cost of a cell once restored, per map unit (1: Linkage Mapper's value).
- `combine`: how the links' scores are merged, by their greatest (`max`, the default) or their sum.
- `cwd_threshold`: keep each core's distance only below it, to speed up large maps.

The outputs are `improvement` (the score at the centre of each circle, the greatest over the radii), `circles` (the
same grown out to the whole circle, the map to look at), `percent` (the improvement as a percentage of the LCD) and
`radius` (the radius that gave the improvement).

### 5. The whole model

```tfgraph
#! caption: The template: the data, then Linkage Pathways, Pinchpoint Mapper and Barrier Mapper.
#! numbered: true
#! highlight: pathways, pinchpoints, barriers
format = "terraflux/1"

[[nodes]]
id = "quality"
op = "nlm.gaussian_field"
params = { rows = 80, columns = 80, range = 12, seed = 31 }

[[nodes]]
id = "habitat"
op = "raster.calc"
inputs = { q = "quality.map" }
params = { expression = "q > 0.7" }

[[nodes]]
id = "large"
op = "raster.sieve"
inputs = { map = "habitat.map" }
params = { threshold = 25, neighbours = 8 }

[[nodes]]
id = "cores"
op = "raster.label"
inputs = { map = "large.map" }
params = { neighbours = 8, classes = [1] }

[[nodes]]
id = "resistance"
op = "raster.calc"
inputs = { q = "quality.map" }
params = { expression = "1 + 99 * (1 - q) ** 3" }

[[nodes]]
id = "pathways"
op = "linkage.pathways"
inputs = { cores = "cores.map", resistance = "resistance.map" }
params = { adjacency = "cost", width = 60 }

[[nodes]]
id = "pinchpoints"
op = "linkage.pinchpoints"
inputs = { cores = "cores.map", resistance = "resistance.map", links = "pathways.links" }
params = { width = 60, mode = "pairwise" }

[[nodes]]
id = "barriers"
op = "linkage.barriers"
inputs = { cores = "cores.map", resistance = "resistance.map", links = "pathways.links" }
params = { min_radius = 2, max_radius = 6, step = 2 }
```

The template adds four Plots, for the cores, the corridors, the current and the improvement. In the application
press ++ctrl+enter++ on the last node to run the model to it; on this small landscape the whole model takes well
under a second.

## Read the results

**Links.** Open the `links` table of Linkage Pathways. Each row is a pair of cores with its `euclidean` distance,
`lcd` (least-cost distance), `path_length`, `cwd_to_path` (`lcd` over `path_length`: the mean resistance met along
the path) and whether it is `kept`. A link of a high `cwd_to_path` crosses hard country even if it is short; a link
that is long but of a low value runs mostly through habitat. For a link that was dropped, `reason` says why (for
instance that its path crosses a third core). Here the four cores give five candidate pairs, of which one, cores 1 and 2 (49 cells apart, an LCD of
361), is dropped because its path crosses core 3; the other four are kept, from cores 3 and 4 (2 cells apart, an
LCD of 7.8) to cores 1 and 3 (an LCD of 280).

**Corridors.** The corridors map holds, in each cell inside a corridor, the normalized cost distance of the least
costly link through it. Cells of a low value are on or near a least-cost path; the band fades out at `width`. A
corridor that is broad, with many cells near 0, has many alternative routes; a thin one has few.

**Current.** In the Pinchpoint Mapper's `current` map, bright cells carry the most movement. Read where it
**concentrates**, not its absolute values: the current is relative to the 1 A injected for each link. A bright
narrow stripe is a pinch point. The links table's `cwd_to_resistance` ranks the links: the lowest are the
bottlenecks.

**Improvement.** In `circles`, high values mark where restoring a circle of the `radius` map's size shortens a
link most per unit of restored distance. Cells with no value are where the improvement is negative (restoring would
not pay) or where the circle's ring touches a core. A bright patch in the matrix between two cores, beside a
corridor, is a barrier: a place of high resistance the least-cost path has to skirt.

!!! tip "Reporting"
    Report the cores and how you chose them, the resistance and its source, the adjacency rule, the corridors'
    `width` (in cost units) and the radii. Keep the links table: it is the result planners use to rank links.

## Settings that change the answer

- **The resistance.** Everything is relative to it. A different scaling (a cube of the quality instead of the
  quality itself) changes which cells are cheap, so the paths, the corridors and the pinch points. Test two or three
  plausible surfaces.
- **The cores' size threshold.** Raising `threshold` in node 3 removes small patches and changes which cores are
  linked, and with them everything downstream.
- **`width`.** In Linkage Pathways it sets how wide the corridors are; in Pinchpoint Mapper, how many routes the
  current can use, and so how sharp the pinch points are.
- **The radii.** Small radii find narrow barriers (a road), large ones find wide ones (a field), at a higher
  cost; `radius` tells which scale produced a value.

## Common pitfalls

- **Width in map units.** `width` is in cost units: 60 is the cost of 60 cells of resistance 1, not 60 metres.
- **Cores and resistance on different grids.** They must be on the same grid. A core cell on an impassable
  resistance is not a core cell.
- **Large rasters in Pinchpoint Mapper.** Circuitscape solves every cell of each corridor: keep corridors to a few
  million cells (a narrower `width`, or `nearest` links in Linkage Pathways).
- **Geographic coordinates.** Radii and distances are in the map's units: in degrees they mean nothing. Reproject first.
- **Reading the current in amps.** It is a relative measure of where movement concentrates.

## Going further

- The [circuit connectivity](/tutorials/circuit-connectivity/circuit-theory/) series explains the circuit theory
  behind the pinch points, and [Circuitscape](/tutorials/circuit-connectivity/circuitscape/) its modes.
- [Least-cost links](../least-cost-links/) is the cost distance on its own, and
  [costs, paths and corridors](/tutorials/distance-restoration/costs-and-corridors/) the GuidosToolbox way of asking what restoring a corridor gains.
- The operators: [[linkage.pathways]], [[linkage.pinchpoints]] and [[linkage.barriers]].

## References

- Adriaensen, F., Chardon, J. P., De Blust, G., Swinnen, E., Villalba, S., Gulinck, H., & Matthysen, E. (2003). The application of 'least-cost' modelling as a functional landscape model. *Landscape and Urban Planning*, 64(4), 233–247. https://doi.org/10.1016/S0169-2046(02)00242-6
- McRae, B. H. (2012). *Pinchpoint Mapper Connectivity Analysis Software*. The Nature Conservancy, Seattle, WA. https://linkagemapper.org
- McRae, B. H., Dickson, B. G., Keitt, T. H., & Shah, V. B. (2008). Using circuit theory to model connectivity in ecology, evolution, and conservation. *Ecology*, 89(10), 2712–2724. https://doi.org/10.1890/07-1861.1
- McRae, B. H., Hall, S. A., Beier, P., & Theobald, D. M. (2012). Where to restore ecological connectivity? Detecting barriers and quantifying restoration benefits. *PLoS ONE*, 7(12), e52604. https://doi.org/10.1371/journal.pone.0052604
- McRae, B. H., & Kavanagh, D. M. (2011). *Linkage Mapper Connectivity Analysis Software*. The Nature Conservancy, Seattle, WA. https://linkagemapper.org
