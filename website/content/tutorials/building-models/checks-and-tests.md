---
title: "Checks, tests and reproducible runs"
short: Checks and tests
description: Make a model check itself as it runs with check.assert, keep what it should give as tests that terraflux test runs, record each run in a manifest that terraflux reproduce runs again, and freeze the result of a slow node.
order: 55
level: intermediate
time: 35 min
ops: [check.assert, raster.calc]
---

A model that ran without an error has not necessarily given the right answer. The land use map may have a class
you did not expect, two bands may have been read in the wrong order, an edit made last week may have changed a
map you have already put in a paper. In this tutorial you will build a small model of the forest lost on Sibuyan
Island between 1997 and 2011, and give it three kinds of protection: **checks** that every run must pass, **tests**
that compare its results with the ones you trusted, and a **manifest** that records what a run was so it can be run
again and compared. At the end you will also know how to **freeze** a node, so that a slow step is not run again
while you work on the rest of the model.

## The idea: a model that says when it is wrong

Software developers learnt long ago that code is wrong more often than it looks, and that the cheapest way to find
out is to let the computer check it, every time, rather than to look at the results by eye (Wilson et al., 2014).
The same holds for a model: an assumption written down as a test is checked at every run, whereas an assumption
kept in your head is checked only when you remember it. Ecological modellers ask for the same thing in other
words. The TRACE documents keep the record of how a model was tested along with the model (Grimm et al., 2014), and
"evaludation" treats the checking of a model as a task that lasts as long as the model is used, not a single step
at its end (Augusiak et al., 2014). A computational result is reproducible when someone else can run it again from
the same data and code and get the same numbers (Peng, 2011; Sandve et al., 2013).

Terraflux gives you three tools, each answering one question:

| Tool | The question it answers | Where it lives |
|---|---|---|
| [[check.assert]] | Does what this run computes make sense? (the classes are the expected ones, the areas add up, a demand is met) | a node of the model, checked at every run |
| `[[tests]]` and `terraflux test` | Does the model still give what it gave when I trusted it? | the model file, and files of what the results should be |
| `manifest.json` and `terraflux reproduce` | Was this run what I say it was, and does it give the same today? | written with the outputs of every run |

!!! science "Key idea"
    A **check** is a rule that must hold for *every* run, whatever the data: no cell outside the classes, no
    probability above 1. A **test** compares *one* known run with its known answer: on these maps, these settings,
    the forest lost is these 563 cells. A **manifest** records *one* run as it happened: which version of
    Terraflux, which model file, which data, which seeds, and a fingerprint of every output. A check finds data that
    breaks the model; a test finds an edit that breaks it; a manifest shows that a result came from the model and
    data you say it came from.

## When to use it

- **Checks**, in every model that others will use or that you will run on new data: the data's classes and
  ranges, the total area kept by an allocation, a demand met by a land change simulation, the sum of probabilities.
  A check that fails stops the run with its message, before a wrong map is written.
- **Tests**, as soon as a model gives results you have checked by hand and will report. Run `terraflux test`
  after every change to the model and before every new version of a paper's figures. Tests also catch changes
  that come from outside the model: a new version of Terraflux, or a data file replaced by one that looks the same.
- **Manifests**, always: every `terraflux run` writes one, and so does every whole run in the application. Keep the
  manifest with the results you report, and run `terraflux reproduce` when you need to show that they come from
  the model and data you describe.
- **Freeze**, when a step is slow and finished (a fitted model, a map that took an hour), and you are working on
  what comes after it.

None of them tells you whether a model is a *good* model of the landscape: that is what validation against
observed data does ([validating a simulation](../../land-change/validating-a-simulation/)). A check or a test can only say that the
model computes what you meant it to compute.

## What you need

- `sibuyan_landuse.tif`, the land use of **Sibuyan Island** (Philippines) in 1997 and 2011, two bands named `1997`
  and `2011`, 128 × 108 cells of 250 m, classes 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other, and the sea as
  no data. It is the CLUE-S example distributed with R's lulcc package (see
  [Your first raster](../../getting-started/first-raster/)).
- A folder for the model, `model.toml`, next to the map. The tests will add a folder `expected`, and the runs a
  folder `output`:

```text
my-study/
├── sibuyan_landuse.tif
├── model.toml
├── expected/          (written by terraflux test --update)
└── output/            (written by terraflux run, with manifest.json)
```

## Build the model

### Step 1: the data as inputs, and what the model computes

The model reads the two dates as **model inputs** (`[inputs]`), not with [[raster.read]] nodes. Both work, but only
the files of model inputs are recorded in a run's manifest (below): a file a node reads by itself is not fingerprinted,
so a later change to it would go unnoticed. Then two [[raster.calc]] nodes compute the result:

```tfgraph
#! caption: The model's data and results: the two dates as model inputs, the forest lost (loss) and the cells of forest at each date (forest, a script with two outputs).
#! numbered: true
#! highlight: loss, forest
format = "terraflux/1"
name = "Forest lost on Sibuyan"

[inputs.lu1997]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"
note = "The land use of 1997: 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other; the sea has no value."

[inputs.lu2011]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"
note = "The land use of 2011, the same classes."

[[nodes]]
id = "loss"
op = "raster.calc"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = "None if isnull(a) else a == 1 and b != 1" }
note = "1 where the forest of 1997 was gone in 2011, 0 on the rest of the island."

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = """
out before = (a == 1).sum()
out after = (b == 1).sum()
""" }
note = "The cells of forest at each date."

[outputs.loss]
from = "loss.map"

[outputs.forest2011]
from = "forest.after"
```

- **`loss`** is 1 where a cell was forest in 1997 and is something else in 2011, 0 on the rest of the island, and
  without a value in the sea (`None if isnull(a)`).
- **`forest`** is a script of two lines: each `out` is an output of the node, here a number, the count of forest
  cells at each date (`.sum()` of a map of 1 and 0). See [the raster calculator](../calculator/) for scripts.
- The model's `[outputs]` are the map of loss and the forest of 2011. They are what `terraflux run` writes, and
  what the manifest fingerprints.

### Step 2: a check on every cell

The first check makes sure the data is what the model expects: every cell of the island is one of the five
classes, at both dates. A land use map exported from another program may use 0 for the sea instead of "no data",
or code a class 6 that the model does not know; without a check, `loss` would treat those cells as "not forest" and
give a map that looks fine.

```tfgraph
#! caption: A check.assert node (classes) reads both dates: every cell with a value must be one of the five classes.
#! highlight: classes
format = "terraflux/1"
name = "Forest lost on Sibuyan"

[inputs.lu1997]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.lu2011]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"

[[nodes]]
id = "classes"
op = "check.assert"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = "1 <= a <= 5 and 1 <= b <= 5", message = "a cell outside the five classes" }
note = "Every cell of the island is one of the five classes, at both dates."

[[nodes]]
id = "loss"
op = "raster.calc"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = "None if isnull(a) else a == 1 and b != 1" }
```

[[check.assert]] takes any inputs, by any name (here `a` and `b`), and one `expression` in the language of map
algebra, the same as [[raster.calc]]'s. What "passing" means depends on its inputs:

- **with maps among them**, the expression is computed cell by cell, and *every cell with a value* must give true
  (not 0). Cells without a value (the sea) are left out. When it fails, the message says how many cells failed:
  *"N of M cells fail"*;
- **without maps** (numbers, the row of a table), the expression must give true; when it fails, the message gives
  its value: *"it is 0"*.

Its `message` says what a failure means, in your words. If no message is given, it says *the check failed*.

!!! app "In the application"
    The check is in the library's last subject, **Other** (or type `check.assert` in the quick search, `/`). Click
    to add it, then in the Node pane type a name (`a`) and a reference (`$lu1997`, or `node.port`) and click **Add
    input**, once per input, and write the `expression` and the `message` among its settings.

### Step 3: checks on numbers, one that warns

Two more checks read the counts of forest. The first, `shrank`, says the forest did not grow between the dates:
on Sibuyan it shrank, so a gain would mean that the two bands were swapped somewhere. The second, `tenth`, says that
less than a tenth of the forest was lost. That one is not a rule but a warning sign: a loss above 10% in 14 years
is possible, but it is worth a look, so it has `warn = true` and only warns.

```tfgraph
#! caption: The whole model: three checks (classes on every cell; shrank and tenth on the counts of forest, tenth only warning), its outputs and its tests (in the model file below).
#! numbered: true
#! highlight: shrank, tenth
format = "terraflux/1"
name = "Forest lost on Sibuyan"

[inputs.lu1997]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"
note = "The land use of 1997: 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other; the sea has no value."

[inputs.lu2011]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"
note = "The land use of 2011, the same classes."

[[nodes]]
id = "classes"
op = "check.assert"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = "1 <= a <= 5 and 1 <= b <= 5", message = "a cell outside the five classes" }
note = "Every cell of the island is one of the five classes, at both dates."

[[nodes]]
id = "loss"
op = "raster.calc"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = "None if isnull(a) else a == 1 and b != 1" }
note = "1 where the forest of 1997 was gone in 2011, 0 on the rest of the island."

[[nodes]]
id = "forest"
op = "raster.calc"
inputs = { a = "$lu1997", b = "$lu2011" }
params = { expression = """
out before = (a == 1).sum()
out after = (b == 1).sum()
""" }
note = "The cells of forest at each date."

[[nodes]]
id = "shrank"
op = "check.assert"
inputs = { before = "forest.before", after = "forest.after" }
params = { expression = "after <= before", message = "the forest grew" }
note = "Sibuyan's forest shrank between the dates: a gain would mean the bands were swapped."

[[nodes]]
id = "tenth"
op = "check.assert"
inputs = { before = "forest.before", after = "forest.after" }
params = { expression = "(before - after) / before < 0.1", message = "more than a tenth of the forest was lost", warn = true }
note = "Only a warning: a loss above 10 % is possible, but worth a look."

[outputs.loss]
from = "loss.map"

[outputs.forest2011]
from = "forest.after"

[[tests]]
from = "loss.map"
expected = "expected/loss.tif"
note = "The map of the forest lost, cell by cell."

[[tests]]
from = "forest.after"
expected = "expected/forest2011.json"
note = "The cells of forest in 2011."
```

Run it from the folder of the model (write `./model.toml`, with its folder, in Terraflux 1.23.0):

```text
> terraflux run ./model.toml
Forest lost on Sibuyan: inputs read (0.0 s)
  warning: tenth: more than a tenth of the forest was lost ((before - after) / before < 0.1: it is 0)
wrote .\output\forest2011.json
wrote .\output\loss.tif
wrote .\output\manifest.json (terraflux reproduce runs it again)
done in 0.0 s
```

`classes` and `shrank` passed silently. `tenth` failed and warned: the forest went from 4,723 cells to 4,160, a loss
of 563 cells, 11.9% of the forest of 1997, and *it is 0* means that the expression was false. The run went on and
wrote its outputs. Without `warn`, the same check stops the run, and nothing is written:

```text
> terraflux run ./model.toml --set tenth.warn=false --out strict
Forest lost on Sibuyan: inputs read (0.0 s)
Error: tenth: node "tenth": more than a tenth of the forest was lost ((before - after) / before < 0.1: it is 0)
```

The command ends with an error (a non-zero exit code), so a script that runs many models stops there too.

### The checks in the application

Open the model in the application and run it (**Run**, or ++f5++). Each [[check.assert]] node has a badge at the
top right of its box that tells how it fared in the last run:

| Badge | Colour | Meaning |
|---|---|---|
| ✔ | green | passed |
| ! | amber | failed, warning only (`warn = true`) |
| ✖ | red | failed, the run stopped |
| · | grey | not run yet, or turned off (bypass) |

Hover over its title: the tooltip says *the check: passed*, or what failed, such as *the check: failed, warning only:
more than a tenth of the forest was lost ((before - after) / before < 0.1: it is 0)*. A group or a loop that holds
checks shows the **worst** of them on its own box (*the worst of its checks*), so a failure deep inside a loop is
seen from the top of the model.

The model's **health** is a line in the status bar, in the colour of its worst check. For this model it reads
**! 3 checks: 2 passed, 1 warned**, and hovering over it lists every check with its expression and how it fared.
The [model's document](../the-model-document/) shows the same line above the document, and each check's verdict on
its step.

## Read the results

![The forest lost on Sibuyan between 1997 and 2011](/img/tutorials/building-models/checks-and-tests/loss.png "loss.map: the forest of 1997 that was not forest in 2011, 563 cells (35.2 km²), 7.8% of the island's 7,226 cells and 11.9% of its forest of 1997. Most of it is on the northern plains and along the south-western coast.")

The map is what the tests will protect. Its numbers are simple to report: 563 cells of 6.25 ha, 35.2 km² of forest
lost in 14 years, 11.9% of the forest of 1997. Not one cell became forest, since the forest of 2011 (4,160 cells) is
exactly the forest of 1997 minus the cells lost. The `tenth` warning is the model telling you the same thing: a
loss above a tenth, worth saying in the paper.

## Tests: what the model should give

A test says: *this value of the model should be what this file holds*. Tests are written in the model file, after
the nodes:

```toml
[[tests]]
from = "loss.map"
expected = "expected/loss.tif"
note = "The map of the forest lost, cell by cell."

[[tests]]
from = "forest.after"
expected = "expected/forest2011.json"
note = "The cells of forest in 2011."
```

- `from` is an output of a node of the model's top level (`node.port`), not necessarily one of its `[outputs]`.
- `expected` is the file of what it should be, relative to the model's folder: a raster (`.tif`, a map, or a stack
  when it has several bands), a table (`.csv` or `.tsv`), a JSON value (`.json`: a number, a list...) or a text.
- `tolerance` (optional) is the largest difference allowed between two numbers, by default `1e-9`.
- `note` says what the test is about.

### Write the expected files from a run you trust

You rarely write these files by hand. Once the model gives results you have checked (here, the map of loss looks
right beside the two dates, say, and the counts agree with the areas a GIS gives), `terraflux test --update` runs the model and writes them:

```text
> terraflux test ./model.toml --update
  warning: tenth: more than a tenth of the forest was lost ((before - after) / before < 0.1: it is 0)
written ./model.toml: 2 test(s), 3 check(s) (0.0 s)
1 model(s) passed
```

It wrote `expected/loss.tif` and `expected/forest2011.json` (which holds `4160.0`). Maps whose values are not whole
numbers are written in float64, so that no digit is lost. Before the files exist, `terraflux test` fails and says
so (*terraflux test --update writes it*).

### Run the tests

From now on, `terraflux test` runs the model and compares:

```text
> terraflux test ./model.toml
  warning: tenth: more than a tenth of the forest was lost ((before - after) / before < 0.1: it is 0)
ok   ./model.toml: 2 test(s), 3 check(s) (0.0 s)
1 model(s) passed
```

Suppose you edit the model and, by mistake, `loss` now keeps only the forest that became grassland
(`a == 1 and b == 3` instead of `b != 1`). The map still looks plausible, but the test knows:

```text
> terraflux test ./model.toml
  warning: tenth: more than a tenth of the forest was lost ((before - after) / before < 0.1: it is 0)
FAIL ./model.toml: loss.map: 243 values differ, the first at 440: 0 instead of 1
Error: 1 of 1 models failed
```

243 of the 563 cells lost went to something other than grassland, and the test names the first of them (its
place among the map's cells, counted from 0), what it is now and what it should be. When an edit is meant to change the
results, check the new ones, then run `--update` again to make them the reference; when it is not, the test has
just saved you from reporting a wrong map.

A model passes `terraflux test` when it runs to its end (so every check that does not warn passed) and every test
holds. A check that only warns does not make it fail: its warning is printed, as above.

### What "the same" means for a test

- **Maps, stacks and series of maps**: the same number of layers and of cells, and every cell within the
  tolerance; two cells without a value are equal.
- **Tables**: the same columns, the same number of rows, and every number within the tolerance.
- **Numbers**: within the tolerance.
- **Texts**: the same, with the spaces and line ends at their start and end left out; yes or no values and JSON
  values compared as their texts.

The default tolerance, `1e-9`, is as strict as it gets for numbers that come from the same computations: it
allows only the last digits of rounding. Give a larger one when a result may change a little between versions or
computers for a good reason (an iterative fit that stops at a tolerance of its own, say), and say why in the note.

### Many models at once

`terraflux test` takes several models, or folders: a folder's models (its `.toml` and `.json` files, not those of
its subfolders) are tested in turn, and the command fails if any of them does. Keep the models of a study in one
folder, and run `terraflux test models/` before you send anything.

## Reproduce a run

Every `terraflux run` writes `manifest.json` beside its outputs, as the output above shows. The application writes
one too, in the folder `output` next to the model, after a **whole run** (**Run**, ++f5++, not a run up to a node)
of a model that is **saved** with no edits since: the status bar then says **manifest** after the time of the run,
and a click on it opens the folder.

The manifest of this model's run, its paths left out and some fingerprints shortened (`...`):

```json
{
  "terraflux": "1.23.0",
  "when": 1793179498,
  "platform": "windows-x86_64",
  "model": {
    "sha256": "11c0dbfa5f7d1d0ab175cf07892caa02cb23e23ed8832c81bbb7bdd607830186",
    "bytes": 2021
  },
  "files": {
    "lu1997": {
      "sha256": "82f027e51f174803d6ea4909f12ab57f6394ea9486312f32d559e02577a3e7d2",
      "bytes": 3386
    },
    "lu2011": { "sha256": "82f027e5...", "bytes": 3386 }
  },
  "values": {},
  "seeds": {},
  "outputs": {
    "forest2011": { "sha256": "6964e1c4...", "bytes": 6 },
    "loss": { "sha256": "5af3eb02...", "bytes": 866 }
  }
}
```

It records:

| Field | What it holds |
|---|---|
| `terraflux` | the version that ran it |
| `when`, `platform` | when it ran (seconds since 1970, UTC) and on which system |
| `model` | the model file, its path and its **SHA-256** |
| `scenario`, `set` | the scenario run (`--scenario`) and the values given with `--set`, when there were any |
| `files` | each model input of a file (`[inputs]` of kind map or stack), its path and SHA-256 (a folder: its files') |
| `values` | each model input of a value, as the run used it |
| `seeds` | every setting whose name contains `seed`, and every such input given a value in the node, of every node, inside loops and groups too (`node.seed`, `container/node.seed`) |
| `outputs` | each of the model's `[outputs]`, its file and SHA-256 |

A **SHA-256** is a fingerprint of a file: 64 hexadecimal digits computed from every byte of it. Two files with the
same fingerprint are, for any practical purpose, the same file; change one byte and the fingerprint is another.

`terraflux reproduce` reads a manifest, runs the model again with the same scenario, values and files, writes the
outputs into a folder `reproduced` next to the manifest (or `--out`), and compares each output with the one the
manifest recorded:

```text
> terraflux reproduce output/manifest.json
  warning: tenth: more than a tenth of the forest was lost ((before - after) / before < 0.1: it is 0)
same    forest2011 (output\reproduced\forest2011.json)
same    loss (output\reproduced\loss.tif)
reproduced: every output the same (0.0 s)
```

Before it runs, it says what changed since: another version of Terraflux (*Terraflux is ... now, ... then*), the
model file (*the model ... changed*), an input file (*the input lu1997 (...) changed*). It still runs, and tells
which outputs differ. After the same mistaken edit as above:

```text
> terraflux reproduce output/manifest.json
  note: the model C:\...\model.toml changed
  warning: tenth: more than a tenth of the forest was lost ((before - after) / before < 0.1: it is 0)
same    forest2011 (output\reproduced\forest2011.json)
DIFFERS loss (output\reproduced\loss.tif)
Error: 1 of 2 outputs differ from the run of the manifest
```

`--quiet` prints only what differs.

### What "the same" means for a run

`terraflux reproduce` is stricter than a test: an output is the same when its file is the same **byte for byte**
(the same SHA-256). The only exception is vectors in GeoPackage, which record when they were written: they are the
same when they hold the same features. A run of Terraflux is meant to give the same bits for the same data,
settings and seeds, whatever the number of threads (see [Randomness](../randomness/)): an output that differs
when nothing changed is worth looking into.

Two limits are worth knowing:

- **Only the model's `[outputs]`** are fingerprinted and compared. A file written by a [[raster.write]] node is not
  in the manifest: give the results you report as `[outputs]`.
- **The manifest names its files by their full paths** on the computer that ran it (the model, the inputs). It
  reproduces a run where those paths exist: on that computer, or on one where the study's folder is at the same
  place. Moved elsewhere, `terraflux reproduce` stops because it cannot find the model. To check a result on
  another computer, send the folder with its `expected` files and run `terraflux test` there: a test's paths are
  relative to the model.

!!! note "What the next version changes"
    From the version after 1.23.0, the manifest also holds the files the nodes read (`reads`, by `node.setting`)
    and those they wrote (`writes`, compared again by `terraflux reproduce`, but a GeoPackage, which records when it
    was written), and the frozen results the run used (`frozen`). Every path is also given from the manifest's
    folder, so a study's folder moved or copied with its manifest is reproduced where it is now; a file found at
    neither place is named. `terraflux test` and `terraflux reproduce` use the frozen results as `terraflux run`
    does, and `--thaw` runs every node instead.

## Freeze a result

Some steps take minutes or hours (a species distribution model fitted with cross-validation, a circuit map of a
whole region, a calibrated simulation) and are finished: you are working on what comes after them. **Freezing** a
node keeps its result on disk next to the model and uses it instead of running the node, in later sessions too,
whatever the node reads. What only the frozen node reads does not run either: freeze a fitted model, and the
reading and preparation of its data are skipped as well.

!!! app "In the application"
    Select the node and tick **Freeze** in the Node pane, beside **Bypass**. The model must be saved first (until
    then the box is greyed out: *save the model first: a frozen result is kept next to its file*). A node that has
    run is frozen at once with its outputs of the last run; one that has not run yet is frozen when it runs. A ❄
    marks it at the top left of its box on the canvas (*❄ its result frozen: kept on disk next to the model and
    used instead of running it (Freeze in the Node pane thaws it)*). Only nodes of the model's top level can be
    frozen; a group or a loop at the top level is a node like the others.

The node gets `frozen = true` in the model file, and its outputs go to a folder named after the model, a folder per
node:

```text
my-study/
├── model.toml
└── model.frozen/
    └── forest/
        ├── frozen.json      (what each output is, and when it was frozen)
        └── ...              (a file per output that needs one)
```

Each output is kept so that it reads back as the same values: maps in GeoTIFF (whole numbers in the smallest type
that holds them, other values in float64), tables in CSV with every digit, vectors in GeoPackage, fitted models
(Markov chains, demands, regressions, weights of evidence, species distribution and machine learning models) in
JSON, and numbers, texts and yes or no values in `frozen.json` itself. A random generator, agents, Biodiverse's
data, a tree, a demand by region or a series that is not of maps cannot be kept: freezing such a node is refused,
with the output that cannot be kept.

`terraflux run` uses frozen results too, and says so:

```text
forest: its result frozen (.\model.frozen\forest)
```

When the folder is missing (the model was sent without it), the node runs, with a note: *forest: frozen, its
result not read (...): it runs*.

**To unfreeze** a node, untick **Freeze**: its folder is deleted (and `model.frozen` with its last node), and the node
runs again at the next run. In a model file edited by hand, delete the line `frozen = true`; delete the node's folder
in `model.frozen` too, since nothing else will.

!!! warning "A frozen result does not follow its data"
    A frozen node gives the same result even after its inputs or its settings change. That is the point, and also
    the danger: unfreeze it before the final runs of a study. `terraflux test` and `terraflux reproduce` do not
    use frozen results: they run every node. A model whose frozen result no longer matches what its node gives
    passes or fails its tests on what the node gives now, and a run made with the frozen result does not
    reproduce. (From the next version they use the frozen results, as `terraflux run` does; see the note above.)

## Settings that change the answer

- **`warn`** decides whether a failed check stops the run or only warns. Make a check stop the run when a failure
  means the results are wrong (data outside its classes, an allocation that lost cells); make it warn when a
  failure is unusual but possible (a large loss, a demand nearly met).
- **`message`** is what a reader of the error sees. Write it as the problem, not the expression: *a cell outside
  the five classes* says more than *the check failed*.
- **`row`** (for checks on tables) says which row of the tables read the expression uses: 1, the first, by default;
  `-1` the last.
- A test's **`tolerance`** decides how much a number may move before the test fails. Keep the default unless you
  have a reason, and write the reason in the test's note.

## Common pitfalls

- **Data read by nodes rather than inputs.** In 1.23.0 a file read by a [[raster.read]] node is not in the
  manifest, so `terraflux reproduce` cannot say it changed. Make the data the model's `[inputs]` (the next version
  records both).
- **A check that never fails.** Try each check once with data that should fail it (`--set tenth.warn=false`
  above, or another file with `--set lu1997=...`): a check written wrongly can pass on everything.
- **Updating the expected files without looking.** `terraflux test --update` makes whatever the model gives now
  the reference. Run it only after checking the new results, never to make a failing test pass.
- **Results reported from files outside `[outputs]`.** Neither the manifest nor `terraflux reproduce` sees them.
- **A manifest moved to another computer.** In 1.23.0 it names full paths: use tests to check a model elsewhere
  (the next version finds the files from the manifest's folder).
- **A node left frozen.** Its result no longer follows the data. Before the final runs, untick **Freeze**, or at
  least check the ❄ marks on the canvas.
- **A model named without its folder.** In Terraflux 1.23.0, `terraflux run model.toml` stops with *cannot make an
  empty path absolute*; write `./model.toml` (fixed in the next version).

## Going further

- [The model's document](../the-model-document/): the checks' verdicts on their steps and the model's health above
  the document.
- [Randomness and reproducibility](../randomness/): seeds, which the manifest records, and why the same seed gives
  the same bits.
- [Groups and submodels](../groups-and-submodels/): a group shows the worst of its checks, and a slow group can be
  frozen.
- [Running models](/cli/models/): `terraflux test` and `terraflux reproduce` with all their options.
- [Cite Terraflux](/about/citing/): what to put in a paper's supplementary material.

## References

- Augusiak, J., Van den Brink, P. J., & Grimm, V. (2014). Merging validation and evaluation of ecological models to 'evaludation': A review of terminology and a practical approach. *Ecological Modelling*, 280, 117–128. https://doi.org/10.1016/j.ecolmodel.2013.11.009
- Grimm, V., Augusiak, J., Focks, A., Frank, B. M., Gabsi, F., Johnston, A. S. A., Liu, C., Martin, B. T., Meli, M., Radchuk, V., Thorbek, P., & Railsback, S. F. (2014). Towards better modelling and decision support: Documenting model development, testing, and analysis using TRACE. *Ecological Modelling*, 280, 129–139. https://doi.org/10.1016/j.ecolmodel.2014.01.018
- Peng, R. D. (2011). Reproducible research in computational science. *Science*, 334(6060), 1226–1227. https://doi.org/10.1126/science.1213847
- Sandve, G. K., Nekrutenko, A., Taylor, J., & Hovig, E. (2013). Ten simple rules for reproducible computational research. *PLoS Computational Biology*, 9(10), e1003285. https://doi.org/10.1371/journal.pcbi.1003285
- Wilson, G., Aruliah, D. A., Brown, C. T., Chue Hong, N. P., Davis, M., Guy, R. T., Haddock, S. H. D., Huff, K. D., Mitchell, I. M., Plumbley, M. D., Waugh, B., White, E. P., & Wilson, P. (2014). Best practices for scientific computing. *PLoS Biology*, 12(1), e1001745. https://doi.org/10.1371/journal.pbio.1001745
