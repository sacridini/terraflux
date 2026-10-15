---
title: Running models
description: Run, check, create and convert models from the command line, list the operators and resample a raster.
order: 10
---

These are the commands you use every day: `run` a model, `check` it first, create a land change model from
your maps with `new`, and a few helpers. The examples use the classic Dinamica EGO example of Sibuyan Island
(two land use maps, 1997 and 2011, and thirteen explanatory variables), the outputs shown are real.

## terraflux run

Runs a model and writes its outputs. A model's outputs are its `[outputs]` (written into the output folder) and
any file its nodes write (`raster.write`, `vector.write`).

```tfcli
run
```

```text
> terraflux run model.toml
  simulation: step 13/14 (0.2 s)
  simulation: step 14/14 (0.2 s)
wrote output\maps.tif
wrote output\steps.csv
wrote output\weights.csv
done in 0.2 s
```

The outputs go to a folder named `output` next to the model, unless `--out` names another. Run again, they are
overwritten.

### Changing an input without editing the model

`--set name=value` replaces one of the model's `[inputs]` for this run only, and `--set node.setting=value` one
of a node's settings (`container/node.setting` for a node inside a loop), as a scenario does. A file input takes a
path; a value takes a value written as JSON (a number, `true`, a list `[1, 2]`, a text in quotes):

```bash
terraflux run model.toml --set seed=7 --out seed7          # another seed
terraflux run model.toml --set t0=landuse_2005.tif         # another starting map
terraflux run model.toml --set patches.isometry=2.0        # a node's setting
terraflux run model.toml --set threshold=0.6 --set seed=2  # several at once
```

This is the simplest way to compare scenarios by hand. For many runs at once, use
[`submit`](/cli/batches/).

### Maps larger than memory

`--tiled` runs Dinamica EGO's model tile by tile, so a map of billions of cells needs only the memory of a few
tiles. With the same tile size and seed, the maps are the same whatever the number of threads or machines; they
are statistically equivalent to the exact engine's (the same quantities of change, a similar spatial pattern).
The tile size itself changes the maps a little, because the sizes of the patches a fit measures are measured
within tiles: keep `--tile` fixed when you compare runs.

```bash
terraflux run model.toml --tiled                            # tiles of 1024 cells, layers in memory
terraflux run model.toml --tiled --tile 2048 --disk layers  # the steps' layers kept on disk
terraflux run model.toml --tiled --disk layers --coordinator http://server:7700   # tiles on many machines
```

`--radius` bounds how far a patch grows from its seed (in cells): a patch is grown within one tile and the
halo around it. The guide's page on [maps larger than memory](/parallel/) explains the method.

## terraflux check

Checks a model without running it: the file's format, every operator and setting, every connection (an output
must fit the input it feeds), and that every input file exists and can be read.

```tfcli
check
```

```text
> terraflux check model.toml
model.toml: no errors
```

Use it before a long run, after editing a model by hand, and in scripts before a batch.

## terraflux new

Writes a complete land change model from your maps, fitted with the classic methods. The model is written out
node by node, each with a note saying why it is there, so you can open it in the application, read it and change
any part.

```tfcli
new
```

### Dinamica EGO's model

Markov transition rates, weights of evidence of the drivers, and the Expander and Patcher allocation, fitted on
two maps of the calibration period:

```bash
terraflux new dinamica --t0 landuse.tif:1997 --t1 landuse.tif:2011 \
    --drivers drivers.tif --steps 14 -o model.toml
```

A map is a file, or a file and a band after a colon: the band's name (`landuse.tif:1997`) or its 0-based index
(`landuse.tif:0`). `--steps` is the number of time steps between the two maps (14 years here, one step a year);
`--simulate` how many steps to run (by default the same), from `--start` (by default the first map).

```tfcli
new dinamica
```

### CLUE-S and the ordered allocation (lulcc)

Logistic regressions of each class on the drivers, a demand interpolated from the observed maps, and CLUE's
iterative allocation, or the ordered allocation:

```bash
terraflux new clue --observed observed.tif --times 0,6,14 --drivers drivers.tif \
    --elasticity 0.2,0.2,0.2 -o clue.toml
terraflux new ordered --observed observed.tif --times 0,6,14 --drivers drivers.tif \
    --order 2,1,3 -o ordered.toml
```

```tfcli
new clue
```

The settings given as JSON (`--formulas`, `--allowed`, `--neighbourhood`, `--settings`) are easiest to write
inside single quotes on macOS and Linux (`--allowed '[[1,1,0],[0,1,0],[1,1,1]]'`); in PowerShell, write the
inner double quotes twice or keep the JSON in a variable. The tutorials on
[land change simulation](/tutorials/#land-change) explain every setting.

## terraflux convert

Writes a model in the other format, chosen by the extension: TOML (easy to read and edit) or JSON (easy for
programs, and the format landschaft writes from Python).

```bash
terraflux convert model.toml model.json
terraflux convert dinamica.json dinamica.toml
```

## terraflux ops

Lists every operator with what it does; `--json` gives their full signatures (inputs, outputs, settings, types),
the data this website's [operator reference](/operators/) is made from.

```bash
terraflux ops                      # read on screen
terraflux ops > operators.txt      # into a file
terraflux ops --json > ops.json
```

## terraflux resample

Puts a raster on the grid of another one (same coordinate reference system): the same cell size, the same cell
edges, the same extent. It is what you need before combining maps cell by cell.

```tfcli
resample
```

```bash
terraflux resample drivers.tif --like landuse.tif -o drivers_aligned.tif
terraflux resample slope.tif --like landuse.tif --method average -o slope_100m.tif
```

With `--method auto` (the default) Terraflux chooses by the values: classes keep their codes (nearest cell, or
the most common one when cells get larger), continuous values are interpolated (bilinear, or averaged when cells
get larger). The tutorial [One grid for all your maps](/tutorials/getting-started/one-grid/) explains why the
choice matters. To also change the coordinate system, use the operator [[raster.reproject]] in a model.

## terraflux app

Opens the application, with a model if one is given; the same as double-clicking the program or a model file.

```bash
terraflux app
terraflux app model.toml
```
