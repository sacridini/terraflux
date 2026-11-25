---
title: The window and its panes
short: Window and panes
description: "Every pane of Terraflux's window and what View turns on and off, the menus item by item, the tabs of models, the Changes pane, the History, recovery, files not found, notes and frames, run progress and notifications."
order: 10
---

This page lists the parts of Terraflux's window and what each one is for. For a first walk through them, with a
model to look at, read [A tour of Terraflux](/tutorials/getting-started/tour/#the-window).

![The parts of the window](/img/tutorials/getting-started/tour/window.png "The window with the template 'MSPA of a forest' run, the Viewer dragged below the Model. The tour numbers each part. Screenshot of Terraflux 1.51 on Windows (the theme Slate).")

From top to bottom the window has:

- the **toolbar**: the menus, **Run** and **Stop**, the model's name and how its run goes, and the tools at the right;
- a **strip of tabs**, only when two or more models are open ([Several models at once](#several-models-at-once));
- the **panes**, in the middle: tabs you can drag, split and move to another screen;
- the **drawers** at the window's left edge: **Files** and, once the model has data, **Prepare** (an
  **Operators** drawer too, if **the operators in a drawer at the left edge** is on in the settings);
- the **status bar** at the bottom: a reminder of the mouse and keys of the pane under the pointer, and the model's
  health when it has checks ([Checks shown on nodes](/app/canvas/#checks-shown-on-nodes)).

## The panes

Every pane is a tab. **View** lists them all with a tick box: ticked means open. Terraflux starts with the
**Model** pane only; the others open when you ask for them in **View**, or by themselves when they have something
to show. Close the pane's tab in front with ++ctrl+w++, or remove its tick in **View**.

| Pane | What it holds | Opens |
|---|---|---|
| **Model** | The canvas where the nodes and their wires are drawn, with its bar ([Building on the canvas](/app/canvas/)). | At the start. |
| **Document** | The same model read as the methods section of a paper: stages, steps, settings in words, the maps and tables of the last run. | By itself with the first node. |
| **Operators** | The library of operators, by subject, with a search box and your favourites at the top. | From the **Operators** menu, ++slash++ and the commands (++ctrl+k++); as a pane from the panes' menu; in a drawer at the left edge if **the operators in a drawer at the left edge** is on in the settings. |
| **Files** | The computer's folders, to open a file or drag it onto the canvas or the Viewer. | In a drawer at the left edge; a click on its tab opens it as a pane. |
| **Node** | The node selected: its name, what its operator does (with a **Documentation** link to the operator's page on this website), its **Inputs**, its **Settings** and its record (**Note**, **Why**, references). With no node selected, the model's own record. | Only if **open the Node pane by itself** is on in the settings. Otherwise a node's settings open in a panel beside its box. |
| **Prepare** | The data of the node selected compared with the model's reference grid, and the steps that put it on that grid ([One grid for all your maps](/tutorials/getting-started/one-grid/)). | In a drawer, once the model has data. |
| **Viewer** | What the node selected gives: a map, a table, a number ([The viewer](/app/viewer/)). | By itself with the first data. |
| **Simulation** | The maps of a run step by step, and the maps of an input's stack. | **View**. |
| **Areas** | The area of each class along the steps of a run. | **View**. |
| **Validation** | The last simulated map compared with an observed one, cell by cell (the figure of merit) and in neighbourhoods of growing size ([Land change simulation](/tutorials/land-change/)). | **View**. |
| **Many runs** | The model run many times: replicates, scenarios, calibration, sensitivity analysis. Also the **Many runs** button of the toolbar. | **View**, or the toolbar. |
| **Computers** | Other computers helping with the runs, and this one helping another ([Parallel computing](/parallel/)). | **View**, or the toolbar. |
| **Assistant** | A chat with a language model that explains, changes and runs the model ([AI assistants](/cli/assistants/)). | **View**, or the toolbar. |
| **Changes** | What changed in the model since an earlier version, node by node ([The Changes pane](#the-changes-pane)). | **View**, or **File > Compare**. |
| **Parameters** | The model's parameters as a form, and its scenarios saved, applied and compared. | **View**. |
| **Trace** | Why a cell of a result has its value: the chain of values that made it ([Trace](/app/canvas/#trace-why-a-cell-has-its-value)). | **Trace this cell**. |
| **Problems** | What is wrong with the model: the failure of the last run explained, the checks of the graph, the problems of the data. The common ones have a **Fix** button, one step of undo ([Fix a problem](#fix-a-problem-with-a-click)). | **View**; also when a run is asked with nodes not filled in. |
| **Calculator** | The script of the map algebra node selected, with its names completed and its outputs previewed ([The raster calculator](/tutorials/building-models/calculator/)). | By itself when a script is opened. |
| **Runs** | The journal of the model's runs, kept beside its file: when, how long, how they ended, what changed and what was written. | **View**. |

Panes you drag:

- Drag a tab to put it beside, above or below another, or on another pane to share its tabs.
- Drag a tab out of the window, or right-click it and choose **Open in a window of its own**, to put it on a second
  screen.
- The Viewer also has **Pop out** in its header, and **View > Viewer in a window of its own** (then **Viewer back
  into the window**). Terraflux remembers where the Viewer's window was.
- **Tab** goes to the Model pane, and **Tab** again goes back to where you were.

The layout is kept between sessions, in `layout.json` beside the settings file ([Settings](/app/settings/#where-the-settings-are-kept)).

### Reset the layout

**View > Reset the layout** brings back the window as it first was: the Model pane alone, the Viewer back in the
window, windows of their own closed. Panes that open by themselves (the Viewer, the Document, Prepare, the
Calculator) come again when there is something to show. It is the answer when a pane has disappeared.

## The menu bar

At the left of the toolbar: the logo and the version number, then the menus: **File**, **Templates**, **Operators**, **Edit**, **View** and **Help**. **Operators** and **Edit** appear
only when a model is open. Items that need a model are greyed out without one.

### File

| Item | Does |
|---|---|
| **New model** (++ctrl+n++) | A new empty model. With a model already open, it opens in a tab of its own. |
| **Open...** (++ctrl+shift+o++) | Opens a model file (`.toml` or `.json`). |
| **Open recent** | The last ten models opened or saved, the last first; files that are gone are left out. |
| **Open package...** | Opens a model sent as one file (`.tfpack`): unpacks it into a folder you choose (by default beside the package, named like it), checks every file against the manifest and opens the model ([Packing a model with its data](#packing-a-model-with-its-data)). Dropping a `.tfpack` on the window does the same. |
| **Save** (++ctrl+s++) | Saves the model. Greyed out when there is nothing to save. |
| **Save as...** (++ctrl+shift+s++) | Saves it under another name. |
| **Save a version...** (++ctrl+alt+s++) | Keeps the model as it is now under a name, to review against or go back to (the **Runs** pane lists versions). |
| **Pack for sharing...** | The model and the files it reads in one `.tfpack` file, to send to a supervisor or a reviewer ([Packing a model with its data](#packing-a-model-with-its-data)). |
| **Clean outputs...** | Deletes the files and folders that the runs of the model wrote. |
| **Open a raster or vectors...** (++ctrl+o++) | Opens a map or a vector file and adds a node that reads it. |
| **Data catalog...** | The public datasets Terraflux downloads onto the model's grid ([The data catalog](/app/canvas/#the-data-catalog)). |
| **Process a folder...** | Choose a folder, then an operator: it runs on every raster of the folder. |
| **Compare** | Opens the **Changes** pane with the model compared with: **Compare with the saved file**, **Compare with the last commit (git)**, **Compare with a commit (git)** (a list of the file's commits) or **Compare with a file...**. The first three need a saved model. |
| **Export report** | The model as a document, as a web page or a PDF. |
| **Methods and citations...** | The model's methods as a paper tells them, and the papers to cite (BibTeX, RIS). |
| **Python script...** | The model as a script of the [Python library](/python/). |
| **Export to QGIS...** | The run's maps and vectors as a QGIS project ([Export to QGIS](/app/qgis/)). |

**Open a raster or vectors...**, **Data catalog...**, **Compare**, **Export report**, **Methods and citations...**,
**Python script...** and **Export to QGIS...** appear only when a model is open. **Save a version...**,
**Pack for sharing...** and **Clean outputs...** are greyed out without one.

### Templates

**Templates** opens with the line *Models to start from, run at once* and seven groups: **Patterns
(GuidosToolbox)**, **Landscape metrics**, **Connectivity**, **Conservation and restoration**, **Species**,
**Land change** and **Surfaces and uncertainty**. A click on a template opens it in the Model pane and runs it.
Hover over a name to read what the template does. A template that has a tutorial on this website shows a small
**tutorial** link at the right of its row, which opens the page in the browser. The [tour](/tutorials/getting-started/tour/#templates-models-to-start-from)
lists them all.

Under the groups, **Yours** lists the templates you saved. A small **x** beside one removes it. With a model open,
a box and the button **Save this model as a template** add the model to the list (the name is the model's if the
box is empty). Your templates are `.toml` files in a `templates` folder beside the layout file.

### Operators

The library as a menu, to add an operator after the node selected: **Open a raster or vectors...**, **Favourites**
(by colour, empty until you put an operator in a collection), then one submenu per subject (data preparation,
habitat patterns, landscape metrics...). The same library is in the **Operators** drawer and behind ++slash++. A right click on an operator, in the menu or in
the drawer, offers **Open its documentation**: its page in the operator reference.

### Edit

**Find and replace...** (++ctrl+h++, with a model open) opens the window that changes a path, a no-data value, a name or
a word in every node at once, with a preview and one step of undo ([Find and replace](/app/canvas/#find-and-replace)).

### View

| Item | Does |
|---|---|
| The panes, each with a tick box | Opens or closes a pane ([The panes](#the-panes)). |
| **Viewer in a window of its own** / **Viewer back into the window** | Takes the Viewer out of the window, to another screen, and back. |
| **History of the edits** | Opens the [History](#history). Needs a model. |
| **Reset the layout** | See [Reset the layout](#reset-the-layout). |
| **Colour vision** | Draws every map, legend and chart as a reader with **normal colour vision**, **protanopia (red-blind)**, **deuteranopia (green-blind)** or **tritanopia (blue-blind)** sees them ([Colour vision and accessibility](/app/colour-vision/)). |
| **Commands...** (++ctrl+k++ or ++ctrl+shift+p++) | The palette: anything the application does, by its name: a command, a pane, a template, a node, an operator, a place. |

### Help

| Item | Does |
|---|---|
| **Quickstart**, **Tutorials**, **The application**, **Operator reference** | Open that page of this website in the browser. |
| **Tutorials with a model...** | A window with the tutorials of this website by category that come with a model, each with **Read** (its page) and **Open the model**: a template opens as from the Templates menu, and a model published with the tutorial is downloaded with the files it reads into the application's data folder (`tutorial models`) and opened like File > Open. A filter box narrows them by title, category or template; **all tutorials** also lists those without a model. A failed download is told in its row. |
| **Keyboard shortcuts** (++f1++) | The sheet of shortcuts ([Keyboard and mouse](/app/shortcuts/#the-f1-sheet)). |
| **Page of the selected operator** (++shift+f1++) | Opens the operator reference's page of the node selected (off with no node selected). |
| **Report a problem...** | Opens a new issue on GitHub in the browser with the Terraflux version, the system and the last failure (the run's, else the first error of the model) already written, and the headings *What I did*, *What I expected* and *What happened* to fill in. You read it and send it: Terraflux sends nothing by itself. |
| **About Terraflux** | A small window with the name, the version, the licence (GPL-3.0-or-later), how to cite Terraflux (with a **Copy** button) and the links to the website, the source code, the releases and the changelog. |

Each of these is also in the command palette (**Help: Quickstart**, **Help: Report a problem**, **About Terraflux**...).

### The rest of the toolbar

After the menus come the controls of a run:

| Control | Does |
|---|---|
| **Run** (++f5++) | Runs the whole model. Greyed out while nodes are not filled in; the **Problems** pane says which. |
| **Check run** | Runs the model on the part of the map the Viewer shows, or at 1/2, 1/4 or 1/8 of the resolution, or both: a check in seconds. Its files go to a `runs/check` folder beside the model, not over the real outputs. Landscape metrics, distances and neighbourhoods change with the resolution, so it checks the model; it does not replace a run. |
| **Stop** | Stops the run. |
| **Step** | A run that pauses after each step of a loop, until **Next** or **Continue** ([Loops](/tutorials/building-models/loops/)). A node with a breakpoint pauses the run before it: **Next node** runs it and pauses before the next one. |

Then the model's name (with a dot when it has edits not saved, and a colour bar when several models are open),
the progress, and, once it has run, *done in 0.04 s* and a **manifest** link that opens the folder of the run's
outputs and manifest.

At the right: **Use my data** (a land change model from your maps: the window of [Use my data](#use-my-data) at its first
page, which model), **Many runs**, **Computers**, **Assistant**, and **⚙**, the [settings](/app/settings/). The classic
themes write *Use my data...*, *Many runs...* and so on. When the window is too narrow, these four go into one menu, **Tools**.

When a colour vision is set, a button *seen with deuteranopia · back to normal* (or another) shows next to the
version; a click sets it back.

## The Model pane's bar

Above the canvas, the bar says where you are in the model and holds the editing buttons:

| Control | Does |
|---|---|
| **Whole model**, then a path | Where you are: the whole model, or inside a group or a loop (**‹ Back**, or ++esc++, goes up a level). |
| **Run to here** (++ctrl+enter++) | Runs the node selected and what it needs; the rest comes from the cache. |
| **Arrange** (++ctrl+l++) | Puts the nodes in tidy columns, by what they read. |
| **Undo**, **Redo** | One edit back or forward (++ctrl+z++, ++ctrl+shift+z++). |
| **History** | The edits by name ([History](#history)). |
| **update after every edit** | Runs the model again up to the node shown after each change. |
| **Times** | Colours the boxes by their share of the last run's time and lists the slowest ([How long things take](#progress-and-time-left)). |
| **Text** / **Diagram** | The model as text (TOML), with the problems found in it, and back. |

If another program changes the model's file while you have edits not saved, a line says so, with **Read it** and
**Keep mine**.

### The start page

A model with no node shows, over the empty canvas, a **start page**: **Start from your data** (open a raster or vectors,
Use my data for a land change model, use your data in a template, open a model, or drop files on the window), **Recent models**, **Learn** (the Quickstart, the tutorials and
this guide on the website) and **Start from a task**, the templates by group, each with its tutorial. Click a template
and the model opens and runs, its data drawn by neutral landscape models. The page goes with the first node and comes
back when the model is empty again; the canvas under it works as always (drop files, ++slash++, the operators'
library). Turn it off with **Show this page when the model is empty** at its foot, or in **Settings > Model**.

### Use my data

Every template draws its data by neutral landscape models. **File > Use my data...** (also in the command palette, as
**Use my data in a template** on the start page, and as the strip *This model runs on drawn data* over the canvas of a
model that draws some, with **Hide** for that model) puts your files in their place. With a model that draws nothing,
the window first offers the templates: start one, then choose your files. It has four pages (the third only for a
model that has parameters or is one of the three land change templates).

The toolbar's **Use my data**, the start page's **Use my data: a land change model from your maps** and the same entry of
the command palette open the window at a first page, **Which model**: *Dinamica EGO's classic model*, *CLUE-S*,
*Ordered allocation* (CLUE-S with its allocation set to ordered) and *PLUS*. The one you click opens its template and the
window goes on to **Your data**.

- **Your data**: a row for each thing the model draws (what it is, its node and kind). **Choose a file...**, the band of
  a file with several, and **Keep drawn** for what you leave as it is. With more than one file, the **grid of reference**
  is the one the others are put on (a `raster.match` is added only where a file's grid or CRS differs). Each choice is
  checked as soon as it is made, and what is wrong with the data is told under its row.
- **Classes**: for each map of classes or mask, a table of your classes (the code, the name from the file's legend and
  the number of cells) against the classes the model reads, prefilled by name and else by code. A class sent to *no
  data* has none, and a line says how many cells that is. Data of any classes (Landscape metrics) keeps yours as they
  are.
- **Parameters**: the model's exposed parameters (the form of the **Parameters** pane) and, for the land change
  templates, its own settings: for Dinamica EGO's classic model the steps between the two dates, the steps to
  simulate, the buffer increment of the weights (automatic, or a value), the share of the changes that expand patches
  (as observed, or a value), the isometry of the patches and the seed; for CLUE-S the allocation (**clue** or
  **ordered**, with the order the classes are served in), the elasticity, the date of each map, the dates simulated
  and the seed; for PLUS the steps and the seed. For Dinamica and CLUE-S, *the simulation starts from* the second
  date (the future, what the templates do) or the first (the observed period again, to
  compare it with what happened). Where your files have dates (the names of their bands, as numbers) the steps and
  the dates start from them; a value you set stays. What you set is part of the plan: the data
  is checked again after each change.
- **Review**: what will be done, what was refused and the findings of the check (the settings made are told too); an
  error stops **Apply**.

On a land change template (Dinamica EGO, CLUE-S, PLUS) the two dates are two data. Choose the file of the first; the
second then offers **The same file, band ...** (the next band), so that a file with one band per date is chosen once;
two files work as well. CLUE-S also takes a file with three bands or more: the second date is then offered
**All N bands as the dates** (a box on the page takes it back), the demand reads every band as a date and the dates
come from the bands' names (no table of classes then: your classes are read as they are). The drivers are one file with a band per variable. A file on another grid than the land use
map's is put on its grid by a `raster.match`, its method chosen by the data (the mean of the cells where they are
coarser). The checks are those of every model (the grids, whole class codes, some change between the two dates, the drivers'
values), whatever the inputs are called: they follow the ports the data feeds, through the new nodes.

**What a template can take.** Each row of *Your data* has a kind, and the kind decides what you can bring:

- **An image** (a stack of bands, or the band files of a scene): the layers are named by their roles (`blue`, `green`,
  `red`, `nir`, `swir1`, `swir2`) on the **Bands** page. A band the template reads and your image lacks (the `swir2` of a
  mean of that band, say) is told before **Apply**, which waits until you choose one in **Bands**. Names such as
  `band_1` or *Band 4* carry no role: choose them yourself. The **scale** is read from the file where it can be: an 8-bit
  image (NAIP) is *8-bit, divided by 255*, and a Sentinel-2 file of 2022 on, which carries its offset
  (`BOA_ADD_OFFSET`), is divided by 10 000 less 0.1; where the file does not say, **Bands** reminds you of
  *Sentinel-2 from 2022*.
- **Points**, the example samples of the OBIA templates: a vector file (GeoPackage, GeoJSON, Shapefile) or a table (CSV) of
  `x`, `y` in your image's coordinate system, with a **class in column** box for the column that holds the class. The
  window says how many points fall on the image, a class with none, and a single class (a classifier needs two); the
  points that do not fall are left out.
- **Polygons**, the parcels of *Classes by parcel* and of *Crops by their seasons* (the latter also takes a map of parcel
  ids), and the reference objects of *Choose the segmentation by reference objects*: a vector file, checked for how many
  overlap the image.

A template's drawn data lies on its own image, so replacing the image alone leaves the points and the polygons behind.
The window says so (*the template's example points are on its drawn image...*): bring a file of yours, or click the
points in the Viewer after **Apply** (the Samples box). Maps the template draws only to help draw others (noise, cloud
fields, patches, the ground of a height model) are not offered.

When your file's cells differ from the drawn ones, a note names the settings measured in cells that were chosen for the
drawn cells (the `scale` of *segments*, for example) and says to choose them again; an empty `scale` of a segmenter fits
your image.

**Apply** changes the model in one step named *Use my data*: your file as an input of the model, a reclassification of
your classes into the model's, the drawn node gone, what read it reading the end of the new nodes, each with a note. It
shows in the **Changes** pane, ++ctrl+z++ puts the template back, and the model runs once its data is read (untick
**Run it now** to skip that). A file read as it is stays an input that says what it stands for (*Stands for your
data* in the Node pane), so a second **Use my data...** offers it again, to replace the file; and a model of your
files only no longer shows the strip.

A model not saved yet (a template just opened) writes its outputs under `Documents/Terraflux/<the model's name>`, not in
the folder the program runs in; once the model is saved they go beside its file.

## Several models at once

**New model** or **Open...** with a model already open puts the new one in a tab. A strip appears under the
toolbar (*2 models open*): each model has a colour of its own, shown also in the toolbar beside its name.

- A click on a tab brings its model to the front. ++ctrl+tab++ and ++ctrl+shift+tab++ go to the next and the
  previous one.
- ++ctrl+shift+w++, the tab's **x**, or a middle click on it closes the model in front. It asks first if it has
  edits not saved.
- The models behind keep running. Each tab keeps its own canvas, selection and Viewer. Nodes copied in one tab paste
  in another.
- When Terraflux starts with no model given, the models that were open when it closed open again, each with its view
  as you left it. The setting is **reopen the models open last**.

## The Changes pane

**View > Changes** lists what changed in the model since an earlier version, node by node, each change to keep or
to undo. The earlier version can be:

- what an AI assistant changed;
- the saved file, a commit of the model's file in git, or another model file (**File > Compare**);
- a step of the **History** (**compare**).

For a node changed, it shows its map or table before and now, side by side. Outlines in the colour of the change
mark the changed nodes on the canvas. [AI assistants](/cli/assistants/#the-changes-pane-what-the-assistant-changed)
explains the pane in full.

## History

**History**, in the Model pane's bar (or **View > History of the edits**), lists the edits of the model by name:
*added suit (raster.calc)*, *reclass: setting threshold: 3 → 4*, *3 nodes moved*, *the MCP: add_node, connect* for
an assistant's. The edit you are at is marked and those undone are faint. A click goes back, or forward, to any of
them at once. **compare** opens the Changes pane with the model as it was then. ++ctrl+z++ and ++ctrl+shift+z++ still
go one step at a time.

## Edits are not lost

- **Asking.** Closing the window, **New model**, **Open...** or a template with edits not saved first asks *Save
  the changes to...?* with **Save**, **Don't save** and **Cancel**.
- **Autosave.** While a model has edits not saved, Terraflux keeps a copy of it every 30 seconds in a `recovery`
  folder of the application's data folder ([Settings](/app/settings/#where-the-settings-are-kept)). The copy is
  deleted when the model is saved, or closed with *Don't save*.
- **Recovery.** If Terraflux ends without saving (a crash, the power gone), the next time it opens a strip at the
  top says *Terraflux closed with edits not saved in...* with **Recover** (opens the model with those edits),
  **Discard** (throws them away) and **Later** (asks again next time).

## Packing a model with its data

A model sent alone does not run on another computer: the files it reads are not there. **File > Pack for sharing...**
makes one file, `.tfpack`, to send to a supervisor or a reviewer; whoever receives it opens it and the model runs
without any change.

The window says what goes in before anything is written: how many files and how many bytes (it warns above 2 GB,
when the file may be too large to send), and the files the model names that are not there, one by one: the package
is made without them, and the manifest lists them. **Include the last outputs** adds the files the model's last run
wrote (from its journal, else the `output` folder beside the model) in `outputs/`. The model must be saved: the
file is what is packed.

Inside (a `.tfpack` is a zip file: rename it `.zip` to open it with any program, on Windows, macOS or Linux):

- the model, with **every file path made relative** and pointing into `data/`;
- the files its nodes read: a shapefile with its `.shx`, `.dbf`, `.prj` and `.cpg`, the rasters of a folder or of a
  pattern such as `runs/*/landuse.tif`, tables, plugins (`.wasm`), and the submodels of `control.submodel` nodes with
  their own data, as deep as they go (`data/submodels/`, their paths relative to where they are in the package).
  Two files of the same name from different folders do not collide: the second goes in `data/2/`. Files written by
  the model (`raster.write`) are not packed; one written at an absolute path is written to `output/` of the folder
  the package is opened in;
- `manifest.json`: the version of Terraflux, the date, the model, and each file with its SHA-256, its size and the
  path it came from.

**File > Open package...** (or dropping a `.tfpack` on the window) unpacks it into the folder you choose, by
default beside the package and named like it, checks every file against the manifest and opens the model. A file
that does not check (changed after the package was made, damaged, or missing) is listed and the model is not
opened. On the command line, [`terraflux pack`](/cli/models/#terraflux-pack) and
[`terraflux unpack`](/cli/models/#terraflux-unpack) do the same.

## Files not found

A model written on another computer, or whose data folder moved, reads files that are not where it says. A strip
over the canvas, and the **Problems** pane, say so: *2 files of the model are not found.*

- If the model is saved in a folder, Terraflux first looks for the files around it (its folder and the one above).
  A project copied whole is found at once: *2 found beside the model.* **Apply (2)** makes the model read them from
  there, in one step of undo.
- **Locate them...** opens the window *Files not found*, which lists each file with the nodes that read it.
  **Look in a folder...** finds them by the end of their old path or by their name, and a folder found for one is
  tried on the others. **Choose...** points to one file by hand.
- Windows paths and Unix paths are read alike, so a model made on Windows finds its data on Linux or macOS and the
  other way round.

On the command line, [`terraflux relink`](/cli/models/#terraflux-relink) does the same.

## Fix a problem with a click

The common problems in the **Problems** pane have a **Fix** button. Hover over it: the hint says exactly what it will
change. A fix is one step of undo (++ctrl+z++ takes it back), and the checks run again after it. A problem without a
safe correction has no button.

- **Maps of a node on different grids** (in the last run's failure, under *What it was given*): **Fix** adds a
  `raster.match` before each input that differs from the first, on the grid of the first. The resampling is `near`
  when the cells are whole numbers (classes) and `bilinear` when they are continuous; when the cells do not say,
  two buttons ask: **Fix as classes** and **Fix as continuous**.
- **A map read without its no-data value**, when the data's check says to look at it: type the value in the field
  beside the problem, then **Fix** sets it as the `nodata` of the input (or of the `raster.read` node).
- **A node not filled in** (an input it needs and has not): **Use $name** connects a model's input of the type the
  port takes; **Fix** opens the file dialog and adds a `raster.read`, `vector.read` or `table.read` (by the type of
  the port) on the file chosen, connected to the port.

## Disk and memory before a run

Before a whole run of a saved model starts, Terraflux tells what it will ask of the computer, and does not leave it
to fail at the end.

- **The disk.** The outputs the run will write (about: the latest run of the model in the journal, else the cells of
  its largest map, the layers it writes, halved for the compression) against the free space of the disk the outputs go
  to. When they may not fit, a window asks first: **Run anyway**, **Clean outputs...** or **Cancel**; when they fit but
  leave little, a line says so.
- **The memory.** The memory the run needs (about: in Terraflux's memory a cell is a number of 8 bytes, so a map of
  50 million cells takes 400 MB; the cells and layers come from the headers of the files, no data is read, and what
  each node's outputs held in the last run of the same model replaces the estimate of them) against the memory the
  computer has available. Above four fifths of it, the **Problems** pane warns, in yellow, with what to do about it:
  the nodes that weigh most, `block_rows` on the nodes that take it (the map is read by blocks of rows, with the same
  values), the tiled engine for a model it runs (`terraflux run --tiled`), and a **Check run** at 1/4 resolution, which
  takes a sixteenth of the memory, to try the model first. Above all of it, a window asks before the run starts:
  **Run anyway** or **Cancel**; nothing is ever refused.
- **Many runs** estimate the memory of each run times the runs that go at the same time (the workers): the cost line
  says it, and when the workers do not fit, **Run** asks and tells how many do (fewer workers). Runs on other
  computers are not counted: their memory is theirs.

The same warning is in the assistant's `check_model` (the MCP).

## Notes and frames

A **note** is a card of text on the canvas for whoever reads the model; a **frame** is a rectangle with a title
around some nodes. Both are kept in the model's file, and the frames become the sections of its
[document](/tutorials/building-models/the-model-document/).

- **Add a note here** (right click on the empty canvas) adds a note. A double click edits its text.
- **Frame them** (in the menu of two or more nodes selected) draws a frame around them, titled by its text's first
  line. The frame follows the nodes when they move. Drag a frame by its title to move its nodes.
- A node let go inside a frame joins it. **Take it out of the frame** in the node's menu takes it out.
- The menu of a note or a frame gives its **Colour**. ++delete++ removes the note or frame selected (a frame's nodes
  stay).
- Copying, duplicating or dragging nodes with ++ctrl++ takes the frames around them along.

## Progress and time left

- **Progress.** While a model runs, a bar in the toolbar shows how far it is, the node running and, once an
  earlier run gives the times, the time left (*~14 min left*; *about* when it is guessed from how far the run is).
  Its tooltip says when the run will end by the clock (*ends about 14:32*). On the canvas, the node running
  has a bar along its bottom.
- **Node times.** After a run, each box tells under its corner how long it took (*2.4 s*, or *cache* when it came
  from the cache). A model opened again shows the times of its runs before, fainter (*~2.4 s*), until it runs. **Times**
  colours the boxes by their share of the run.
- **Node state.** The small dot at the top right of a box: green when it ran, darker green when its result came from
  the cache, orange while it runs, red when it failed.
- **The computer stays awake.** While a model runs, a batch of **Many runs** goes on or this computer works for
  another's, the computer is kept from going to sleep (the screen may still go dark). Turn it off with **keep the
  computer awake while a model runs** in the settings.

## Notifications

When a run you asked for, or a batch of **Many runs**, ends after more than 10 seconds while Terraflux is not in
front, the system shows a notification (*land: done*, or *failed* and why) and the window's button in the taskbar
lights up. The 10 seconds can be changed, and the notification turned off, in **Runs** of the
[settings](/app/settings/#runs).

- On Windows it is a toast. A click on it brings Terraflux to the front.
- On Linux it uses the desktop's notifications (`notify-send`, else `gdbus`). A click brings Terraflux to the front
  when `notify-send` is recent enough.
- On macOS it uses the Notification Center. A click does not bring Terraflux to the front.

## See also

- [A tour of Terraflux](/tutorials/getting-started/tour/): the same window, step by step.
- [Building on the canvas](/app/canvas/) and [The viewer](/app/viewer/): the two panes you use most.
- [Keyboard and mouse](/app/shortcuts/): every shortcut.
