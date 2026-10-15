---
title: Quickstart
description: Install Terraflux, open a template, run it, change a setting, use your own map, save the model and run it from the command line, in fifteen minutes.
order: 10
---

This page takes you from nothing to your first analysis in about fifteen minutes. You will install Terraflux, open
a ready-made model of a forest, run it, change one of its settings and watch the result change, run the same
analysis on a real land cover map, save it, and run it again from the command line. You do not need to know GIS
software: every step says where to click. When you are done, the [tutorials](/tutorials/) take you further.

## 1. Install (3 minutes)

Download the file for your system from the [download page](/download/):

| System | File | What to do |
|---|---|---|
| Windows 10 or 11, 64-bit | `terraflux-<version>-x64.msi` | Double-click it and follow the installer. Terraflux goes to the Start menu, and the command `terraflux` to the PATH. |
| macOS, Apple silicon (M1 or later) | `terraflux-<version>-macos-arm64.dmg` | Open it and drag **Terraflux** to **Applications**. The first time, **right-click** the application and choose **Open**. |
| Linux, x86-64 (Ubuntu 24.04 or newer) | `terraflux_<version>_amd64.deb` | In a terminal: `sudo apt install ./terraflux_<version>_amd64.deb`. Terraflux appears in your desktop's menu. |

Everything Terraflux needs comes with it, including GDAL (the library that reads and writes map files): there is
nothing else to install, neither Python nor R. Other formats (a portable zip for Windows, a tar.gz for Linux
without root), the command line on macOS, and what to do when something does not start are in
[Installing Terraflux](install/).

## 2. Open Terraflux (1 minute)

Start Terraflux from the Start menu (Windows), Applications (macOS) or your desktop's menu (Linux). The window
opens empty:

![The empty window](/img/tutorials/getting-started/tour/empty.png "Terraflux as it opens. Left: the Node pane (the node you select) with Prepare beside it. Right: the Model pane, where a model is drawn, and the Viewer, which shows maps. Top: the toolbar.")

A model in Terraflux is a chain of **nodes**, boxes that each do one thing (read a map, reclassify it, analyse it),
joined by **connections** that carry maps from one to the next. You will not build one from scratch today: you
will start from a **template**, a model that is ready to run.

## 3. Open a template and run it (2 minutes)

<div class="steps" markdown>

1. Click **Templates** in the toolbar.
2. Move to the first group, **Patterns (GuidosToolbox)**.
3. Click **MSPA of a forest**.

</div>

![The Templates menu](/img/tutorials/getting-started/tour/templates-menu.png "Templates > Patterns (GuidosToolbox) > MSPA of a forest.")

The template opens and **runs by itself**, in a fraction of a second. You now see this:

![The template after its run](/img/tutorials/getting-started/tour/window.png "The template 'MSPA of a forest' run: four nodes in the Model pane (3, 5), their settings in the Node pane (3), and the result in the tab MSPA (7) with its legend (9). The tour of the application explains every number.")

What happened? The four nodes in the Model pane ran from left to right:

1. `landscape` drew a map of 300 x 300 cells with three classes, farming, water and forest, **by chance** (a
   *neutral landscape model*), so that the template runs without any data;
2. `forest` reclassified it: forest became 2, everything else 1;
3. `mspa` ran GuidosToolbox's **morphological spatial pattern analysis** (MSPA; Soille & Vogt, 2009), which splits
   the forest into **core** (green: forest far enough from any edge), **edge** (black), **perforations** (blue:
   the edges of holes inside a forest), **bridges** (red: strips joining two cores), **loops**, **branches** and
   **islets** (small woods with no core at all);
4. `plot` showed the result in the tab **MSPA**, with the number of cells of each class beside it.

Click any node to see its own result in the Viewer: `landscape` shows the three classes, `forest` the two that
MSPA reads. Checking every step like this is the best habit you can learn in Terraflux.

## 4. Change a setting (2 minutes)

The core of a forest depends on how deep you think edge effects reach into it. MSPA calls this the **edge width**,
in cells.

<div class="steps" markdown>

1. Click the node `mspa` in the Model pane (it is selected already after the template opens).
2. In the **Node** pane on the left, under **Settings**, find `edge_width`. It says `1`.
3. Click in the box, replace `1` with `3`, and press ++tab++.

</div>

The model runs again at once (the box **update after every edit** in the Model pane's bar is ticked), and only
the nodes after `mspa` run: the map and its reclassification come from Terraflux's cache. Drag the handle below
to compare the two results:

```tfcompare
caption: "Drag the handle. Left: edge width 1. Right: edge width 3. With the wider edge, core (green) shrinks from 33,168 to 12,838 cells, many small cores disappear, and much of the forest becomes edge (black), bridges (red) and islets (brown)."
before: { src: /img/quickstart/edge1.png, label: "edge_width = 1" }
after: { src: /img/quickstart/edge3.png, label: "edge_width = 3" }
```

That one number changed the core forest by 60%. Every setting of every operator has an explanation: hover over
its name in the Node pane, or look it up in the [operator reference](/operators/).

## 5. Use your own map (4 minutes)

The template drew its map by chance. Now run the same analysis on a real one. There is a template made for that:

<div class="steps" markdown>

1. **Templates > Patterns (GuidosToolbox) > MSPA of your map**. Its first node, `read`, has no file yet, so it
   does not run.
2. Click the node `read`. In the Node pane, next to `path`, click **Choose...** and pick your land cover map, a
   GeoTIFF (`.tif`) or any raster file. Its map appears in the Viewer, with its classes.
3. Click the node `forest`. Its rules say `1 -> 2` and `* -> 1`: class 1 becomes the forest, every other class the
   background. Replace `1` with the code of the forest in **your** map. If you do not know it, click **A rule per
   class**: the table lists every class of your map with its share (if it is empty, press ++ctrl+enter++ to run
   the node first).
4. Press ++f5++ (or **Run** in the toolbar).

</div>

Here is the result for the excerpt of **CORINE Land Cover** around Hanover, Germany, used in the
[getting-started tutorials](/tutorials/getting-started/first-raster/) (2000 x 2000 cells of 25 m). Its forests are
three classes, 23, 24 and 25, so the rules are `23..25 -> 2` and `* -> 1`:

![MSPA of the forest around Hanover](/img/quickstart/clc_mspa.png "MSPA of the forests of the CORINE excerpt (50 x 50 km) with an edge of one cell (25 m). The large forests in the south and north-east are mostly core; the small woods of the farmland are edge and islets.")

!!! tip "No map at hand?"
    Any land cover map works: CORINE Land Cover (Europe), MapBiomas (Brazil and other countries), ESA WorldCover
    or the Copernicus global land cover (the world), the National Land Cover Database (USA). Cut a small area
    first; a few thousand cells on each side runs in seconds. If your file has no forest class, MSPA works on any
    habitat: grassland, wetland, urban areas.

## 6. Save the model (1 minute)

<div class="steps" markdown>

1. To keep the map, select the node `mspa`; in the tab or the Viewer, click **Save output > As a file, and a write
   node in the model...**, and give a name, such as `clc_mspa.tif`. A [[raster.write]] node is added to the model,
   so the file is written again at every run.
2. Click **Save as...** in the toolbar and save the model as `my-mspa.toml`, in the folder of your map.

</div>

The model is a small text file. Open it in any text editor: every node, setting and note is there, in plain
words. You can send it to a colleague with the map, and they will get the same result.

## 7. Run it from the command line (2 minutes)

Terraflux is also a command, `terraflux`, that runs model files without opening the window: for long runs,
servers, and running the same model on many maps. Open a terminal (on Windows: **PowerShell** from the Start
menu) in the folder of the model and type:

```text
$ terraflux --version
terraflux 0.80.0

$ terraflux check my-mspa.toml
my-mspa.toml: no errors

$ terraflux run my-mspa.toml
MSPA of your map: inputs read (0.0 s)
done in 0.3 s
```

`check` verifies every node, connection and file without running anything; `run` runs the model, and its
[[raster.write]] node writes `clc_mspa.tif` next to the model. On macOS the command needs one more step, and on
Windows a new terminal after the installation: see [Installing Terraflux](install/#the-command-line).

## What next?

You have used the three things every analysis in Terraflux is made of: a model, its settings and its results.

- [A tour of Terraflux](/tutorials/getting-started/tour/): every pane, key and menu of the window.
- [Your first raster](/tutorials/getting-started/first-raster/): what a raster is, and what the viewer tells you
  about it.
- [Your first model](/tutorials/getting-started/first-model/): build a model by hand, and run it on other maps
  from the command line.
- [MSPA](/tutorials/habitat-patterns/mspa/): what each MSPA class means for species, and how to report it.
- All the [tutorials](/tutorials/), by subject: landscape metrics, connectivity, prioritization, species
  distributions, land change.

## References

- Soille, P., & Vogt, P. (2009). Morphological segmentation of binary patterns. *Pattern Recognition Letters*, 30(4), 456–459. https://doi.org/10.1016/j.patrec.2008.10.015
