---
title: "Maps larger than memory: Dinamica EGO's model on tiles"
short: Maps larger than memory
description: Run Dinamica EGO's model on a map too large for the computer's memory with the tiled engine, see why its maps are the same whatever the tiling or the number of machines, and how they compare with the exact engine's.
order: 320
level: advanced
time: 30 min
ops: [markov.fit, woe.fit, patches.settings, dinamica.step, woe.probability]
series: Validating and calibrating
part: 3
---

A model of Sibuyan Island holds a few thousand cells; a model of the Brazilian Cerrado at 30 m holds two billion.
At that size the maps of a single step (the land use, the drivers, a probability per transition) no longer fit in
the memory of a computer, and the [exact engine](/tutorials/land-change/dinamica-on-sibuyan/), which holds whole
maps, cannot run. In this short part you will run Dinamica EGO's model with Terraflux's **tiled engine**, which
cuts the maps into tiles and runs them one at a time and in parallel, keeping the steps' layers on disk if needed.
You will check on real runs that its maps are the same with tiles of any size and on any number of machines, and
how they compare with the exact engine's. The [guide to parallel computing](/parallel/maps-larger-than-memory/)
covers the machines in more detail.

## The idea

### Why memory runs out

A map is held in memory as a grid of numbers; in Terraflux's exact engine every cell is a number of 8 bytes. A map
of 30 m cells over 2 million km² has about 2.2 billion cells: 18 GB for the land use alone, as much for each driver,
and as much for each transition's probability, recomputed every step. Thirteen drivers and ten transitions ask for
more than 400 GB at once. A desktop computer has 16 to 64 GB.

### Tiles, halos and sums

The tiled engine cuts every map into square **tiles** (1,024 cells a side by default) and processes them one by
one, many at a time on the cores of the computer, or on the workers of a coordinator. Three ideas make this give
the right model:

!!! science "Key idea: what the tiled engine does differently"
    - **Fits by reduction.** What a fit needs from the whole map is a sum: the Markov chain counts cells of each
      transition, the weights of evidence count cells in the ranges of each driver. Counted tile by tile and
      added, the sums are those of the whole map, so the chain and the weights are **the exact engine's to the
      bit**.
    - **Random numbers by counter.** Every random draw is a function of what it is for (the seed, the step, the
      cell), not of the draws before it, so it is the same whichever tile, thread or machine computes it.
    - **Allocation by thresholds.** The Expander and Patcher cannot run cell after cell across a map they never
      hold whole. The tiled engine gives every cell a random key, finds the thresholds of the keys that make the
      right number of seeds from **histograms summed over the tiles**, and grows each patch within `--radius`
      cells (32) of its seed. A tile is processed with a **halo** of twice that radius around it, so every patch that
      may reach it grows the same way in any tile. The thresholds are adjusted in rounds until the changes meet
      the rates.

The result: with the same fitted pieces and the same seed, **the same maps whatever the tiling and the number of
machines**, and maps **statistically equivalent** to the exact engine's (the same amounts of change, a similar
figure of merit and fuzzy similarity), though not the same draws.

![A tile and its halo on a simulated map](/img/guide/parallel/tiles.png "A map simulated by the tiled engine (Sibuyan's maps enlarged eight times, 1,024 x 864 cells) with its tiles of 256 cells; a tile (orange) is processed with a halo of 64 cells (dashed), twice the radius within which a patch grows from its seed.")

## When to use it

Use the tiled engine when the exact engine runs out of memory, or when one large run should use many machines.
Keep the exact engine otherwise: it is Dinamica EGO's sequential allocation (and landschaft's, bit for bit), and on
maps that fit in memory it is as fast. The tiled engine runs **Dinamica EGO's classic model** only: a Markov chain
([[markov.fit]] or [[markov.matrix]]), weights of evidence ([[woe.fit]] or [[woe.weights]]), the patches' settings
([[patches.observe]] or [[patches.settings]]) and a loop of [[woe.probability]] and [[dinamica.step]] with its
3 × 3 window; no dynamic drivers in the loop yet. The models of `terraflux new dinamica` qualify. Other analyses
of large maps (landscape metrics by blocks of rows, MSPA by tiles) are in [the guide](/parallel/maps-larger-than-memory/).

## What you need

- A model as above, with its maps as **files** (GeoTIFF, read by windows).
- To get the same maps whatever the tiling: the patches' settings **given** with [[patches.settings]]. [[patches.observe]]
  works too, but the tiled engine measures the observed patches within tiles, so its settings (and the maps) can
  shift a little with the tile size.
- Disk space for `--disk`, if the steps' layers must leave memory.

## Run it

```bash
terraflux run model.toml --tiled                            # tiles of 1024 cells, layers in memory
terraflux run model.toml --tiled --tile 2048 --disk layers  # the steps' layers kept on disk, a file per tile
terraflux run model.toml --tiled --threads 8                # threads (default: all the cores but one)
terraflux run model.toml --tiled --disk layers --coordinator http://coordinator-host:7700   # on many machines
```

```tfcli
run
```

The tiled engine writes each step's map as a tiled GeoTIFF in a folder named after the output
(`out/maps/step_001.tif` to `step_014.tif`, rather than one `maps.tif` of fourteen bands), and `steps.csv` and
`weights.csv` as the exact engine does. It prints what it does:

```text
  fitting the Markov chain (0.0 s)
  fitting the weights of evidence (0.0 s)
  measuring the patches (0.2 s)
  simulating 14 steps (0.2 s)
  step 1 of 14 (2.2 s)
  ...
  step 14 of 14 (29.4 s)
wrote tiled_dist\maps\step_001.tif
...
done in 29.4 s
```

`--radius` sets how far a patch may grow from its seed (32 cells by default); patches larger than a disc of that
radius are cut. With patches of a few hundred cells, the default is ample.

## Read the results

The test: Sibuyan's maps with every cell cut into 4 × 4 cells (432 × 512 cells of 62.5 m), the model with its
patches' settings given (every transition: half by the Expander, patches of 4 cells, isometry 1.5), run by the
exact engine, by the tiled engine with tiles of 128, 256 and 1,024 cells, with the layers on disk, and on a
coordinator with two workers of this computer.

| Run | Cells that differ from the tiled run (256) | Forest cells in 2011 | Figure of merit | Fuzzy similarity 1 × 1 / 5 × 5 / 9 × 9 |
|---|---:|---:|---:|---|
| exact engine | 4,874 | 66,560 | 0.515 | 0.644 / 0.719 / 0.743 |
| tiled, tiles of 128 | 0 | 66,571 | 0.513 | 0.645 / 0.716 / 0.739 |
| tiled, tiles of 256 | 0 | 66,571 | 0.513 | 0.645 / 0.716 / 0.739 |
| tiled, tiles of 1,024 (one tile) | 0 | 66,571 | 0.513 | 0.645 / 0.716 / 0.739 |
| tiled, layers on disk | 0 | 66,571 | 0.513 | 0.645 / 0.716 / 0.739 |
| tiled, on a coordinator with 2 workers | 0 | 66,571 | 0.513 | 0.645 / 0.716 / 0.739 |

Every tiled run gives the same map to the cell, whatever the tile size, with or without disk, on one machine or
on two workers. Against the exact engine, 4,874 of the island's 115,616 cells differ (4%, all of them in the zones
of change), with the same amounts within 11 cells of forest and the same scores within 0.004: two equally valid
draws of the same model.

```tfgallery
columns: 3
caption: The change of 1997-2011 in the north-west of the enlarged Sibuyan (ochre to coconut, light green to grassland, blue to rice). The exact engine's draw, the tiled engine's (tiles of 256), and the tiled engine's on two workers, identical to it.
images:
  - { src: /img/tutorials/land-change/maps-larger-than-memory/exact.png, caption: "Exact engine" }
  - { src: /img/tutorials/land-change/maps-larger-than-memory/tiled.png, caption: "Tiled engine" }
  - { src: /img/tutorials/land-change/maps-larger-than-memory/tiled_workers.png, caption: "Tiled engine, 2 workers" }
```

![Legend of the change](/img/tutorials/land-change/maps-larger-than-memory/change_legend.png)

### Time

On this small map the tiled engine took about 4 seconds on one computer, as long as the exact engine, and 25 to 30
seconds on two workers through a coordinator: every pass over the tiles (each round of the thresholds, each step)
is a job sent through the network, so for a map that fits in memory the machines spend their time talking. The
tiled engine pays off when the map does not fit, or when each tile's work is large. On a map eight times larger
(1,024 × 864 cells) one computer ran it in 35 seconds with 1 thread and 11 to 12 seconds with 8 or more
([timings](/parallel/many-runs/#how-much-faster)).

### What to report

Say that the tiled engine was used, its tile size and `--radius`, and that its allocation is statistically
equivalent to Dinamica EGO's rather than identical; validate its maps as any other
([Validating a simulation](/tutorials/land-change/validating-a-simulation/)).

## Settings that change the answer

- **`--radius`**: the largest distance (in cells) of a patch's cell from its seed. Too small for the patches'
  sizes, and large patches are cut; larger, and the halos (twice the radius) grow, and so does the work per tile.
- **`--tile`**: changes the time and the memory, not the maps. Larger tiles: fewer halos to compute again, more
  memory per thread. 1,024 to 4,096 suit most computers.
- **[[patches.observe]] against [[patches.settings]]**: measured within tiles, the observed patches can differ
  slightly from the whole map's; give them with [[patches.settings]] (copied from a measurement) when the maps must
  not depend on the tiling.
- **The seed**: as always, one run is one draw; replicates of a large model are costly, so plan how many.

## Common pitfalls

- **A model the tiled engine does not run.** It stops with a message naming what it lacks or does not take ("no
  dinamica.step in the loop", "only woe.probability and dinamica.step in the loop (no dynamic variables yet)", "the
  tiled Expander and Patcher take a 3 x 3 window").
- **Maps that are not files.** The tiled engine reads its maps by windows from files; maps drawn by a node (a
  neutral landscape model) must be written first.
- **Expecting the exact engine's maps.** The tiled engine's draws differ; compare scores and amounts, not cells.
- **Too little disk.** `--disk` writes every layer of every step, tile by tile; budget several times the size of
  the land use map per step.
- **Many machines for a small map.** The coordination costs more than it saves (above).

## Going further

- [Maps larger than memory](/parallel/maps-larger-than-memory/) in the parallel guide: many machines, shared disks,
  metrics and MSPA of large maps.
- [Dinamica EGO on Sibuyan](/tutorials/land-change/dinamica-on-sibuyan/): the model the tiled engine runs.
- [Calibration and many runs](/tutorials/land-change/calibration-and-many-runs/): many small runs, rather than one
  large one.
- The operators: [[dinamica.step]], [[patches.settings]], [[woe.fit]].
