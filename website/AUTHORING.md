# Writing for the Terraflux website

This is the guide for everyone writing pages of the site (people and agents). Read all of it before writing.

## Who we write for

The reader is an **ecologist**, not a computer scientist: a graduate student, a researcher or a practitioner in
conservation, who knows their study system but may never have used Terraflux, may have used GIS software a
little, and may not know the method a tutorial is about. They want to understand **what a method does, why it
exists, when to use it, how to run it, and how to read and report what it gives**.

- Assume no experience with Terraflux, and little with GIS or the method. Define every technical term the first
  time it appears (a *raster*, a *CRS*, *resampling*, a *patch*, *resistance*, a *planning unit*...).
- Explain the **ecological meaning** before the mechanics: what question does this answer about a landscape?
- Use concrete examples (a forest in an agricultural landscape, a jaguar moving between reserves, a city growing).
- Plain, active, specific English. Short paragraphs. No marketing words ("powerful", "seamless", "cutting-edge").
- Never invent facts about Terraflux. Check every operator, input, output and setting against
  `website/data/ops.json` (or `terraflux ops --json`) and the source in `crates/ops/src`. Check the
  application's behaviour in `crates/app/src` (the library's subjects are in `builder.rs`, the templates in
  `templates.rs`). If unsure, leave it out.
- Never invent references. Every paper you cite must be real: verify the authors, year, title, journal and
  DOI (Crossref: `https://api.crossref.org/works?query.bibliographic=...&rows=3`, or open
  `https://doi.org/<doi>`). Good starting points: the docs of terraflux-py in `extern/terraflux-py/docs` (they cite
  the papers behind each method) and the doc strings in `crates/ops/src`. If you cannot verify a DOI, cite
  without it.

## Length and shape of a tutorial

Tutorials are **not short**, unless the subject is genuinely simple. A simple tool: 1,200 to 2,500 words. A
method with real depth (MSPA, connectivity, prioritization, species distributions, land change simulation):
split it into a **series** of parts (2 to 8), each 2,000 to 4,500 words. Begin every series with the ideas,
then build up from the simplest run to the advanced options.

Each tutorial (or each part of a series) follows this outline. Headings may be adapted to the subject, but keep
the order and keep **all** of the substance:

1. **Front matter** (see below) and a one-sentence `description` (shown under the title and in search).
2. An opening paragraph: what the reader will do and what they will have at the end. No heading.
3. `## The idea` (or a better title): the concept in plain words, with a figure. Where it comes from: the
   researchers and papers that developed it, with years, and what problem they were solving.
   Use `!!! science "Key idea"` boxes for the central concepts and formulas (`\( ... \)` inline,
   `\[ ... \]` display; KaTeX renders them). Explain every symbol of a formula. A display formula is a block
   of its own, with a blank line before and after it (and one between two formulas): with text on the next
   line, Markdown reads it as a paragraph, drops its backslashes and it shows as raw text.
4. `## When to use it`: the questions it answers, examples from the literature, what it is *not* for, and the
   alternatives (link the other tutorials).
5. `## What you need`: the data (kinds, resolution, classes, coordinate systems), and the template to start from
   if there is one.
6. `## Build the model` (or several sections): step by step, node by node. For **each** step: what the node
   does, why it is there, which output connects to which input and why, and the settings that matter with their
   ecological meaning. Show the graph (```tfgraph) as it grows, with `highlight` on the nodes just added. Say how
   to do it in the application (the library's subject, click to add, drag from an output port to an input port,
   the Node pane's settings, ++ctrl+enter++ to run to a node) and, where useful, from the command line.
7. `## Read the results`: figures of the real outputs, what each class or value means, what is a good or a bad
   sign, how to report it in a paper (which numbers, which figure). Dashboards here (see below).
8. `## Settings that change the answer`: sensitivity to the important settings, shown with a ```tfslider over
   real runs when possible (an edge width, a distance threshold, a target, a number of steps...).
9. `## Common pitfalls`: the mistakes people make (wrong resolution, projected vs geographic coordinates,
   edge effects, class codes, units) and how to notice them.
10. `## Going further`: links to other tutorials and to the operator reference.
11. `## References`: a list, one paper per item, in the form
    `- Author, A. B., & Author, C. (Year). Title. *Journal*, volume(issue), pages. https://doi.org/...`
    Cite them in the text as (Author et al., Year).

## Front matter

```yaml
---
title: Morphological spatial pattern analysis (MSPA)
short: MSPA                       # optional: the name in the side navigation
description: Split a habitat map into cores, edges, bridges and corridors, and learn what each class means for species.
order: 10                         # position within the category (gaps of 10 leave room)
level: beginner                   # beginner | intermediate | advanced
time: 30 min
template: MSPA of a forest        # optional: the application's template this page starts from
ops: [landscape.mspa, raster.reclassify, view.plot]   # the operators the page teaches
series: Habitat patterns with GuidosToolbox           # optional: pages with the same series form a series
part: 1                                                # its part in the series
---
```

Files: `website/content/tutorials/<category>/<slug>.md`, the categories being the `slug`s of
`tutorial_categories` in `website/site.yaml`. The page's address is `/tutorials/<category>/<slug>/`.

## Components

Everything is Markdown (Python-Markdown with pymdown-extensions), plus these blocks.

### Model graphs: ```tfgraph

A model, in Terraflux's TOML, drawn as the application draws it (nodes, ports, connections, settings), with a
"Model file (TOML)" toggle under it. **Every graph is a real model**: run `python website/build.py --check`
and it is checked with `terraflux check`. Options are TOML comments starting with `#!`:

````markdown
```tfgraph
#! caption: Read a forest map, make the forest MSPA's foreground, run MSPA and show it.
#! highlight: mspa
#! numbered: true
format = "terraflux/1"

[[nodes]]
id = "forest"
op = "raster.read"
params = { path = "forest.tif" }

[[nodes]]
id = "mspa"
op = "landscape.mspa"
inputs = { map = "forest.raster" }
params = { edge_width = 1, foreground = [1] }
note = "Cores, edges, bridges..."
```
````

- `#! highlight: a, b` draws nodes as selected (orange), `#! numbered: true` numbers the nodes in file order
  (refer to them in the text as node 1, node 2), `#! fragment: true` for part of a model (not checked),
  `#! toml: false` hides the model file, `#! params: false` hides the settings in the boxes.
- Prefer models that run as they are: start from a template's data (neutral landscape models such as
  `nlm.random_cluster`, `nlm.gaussian_field`) so a reader can paste the model into the application and see
  the same thing. When a real file is needed, use `raster.read` with a plain name and say what the file is.
- Keep a graph readable: 2 to 9 nodes. For a long model show it in stages, and the full model once.
- Loops (`control.repeat`) are drawn with their body inside.

### Figures

A paragraph with only an image becomes a figure; the title is its caption (Markdown allowed). Every image opens
in a zoomable lightbox (wheel, pinch, drag) with a click.

```markdown
![MSPA classes of the forest](/img/tutorials/habitat-patterns/mspa/mspa.png "MSPA of the forest: cores in green, edges in black, bridges in red.")
```

Paths start with `/` (the site's root); they are made relative when the site is built.

### Several images in a row: ```tfgallery

```yaml
columns: 3
caption: The same forest with three edge widths.
images:
  - { src: /img/tutorials/habitat-patterns/mspa/edge1.png, caption: "edge_width = 1" }
  - { src: /img/tutorials/habitat-patterns/mspa/edge3.png, caption: "edge_width = 3" }
  - { src: /img/tutorials/habitat-patterns/mspa/edge5.png, caption: "edge_width = 5" }
```

### Dashboards: ```tfslider

Frames (real outputs for each value of a setting, or each step of a simulation) shown one at a time with a
slider and a play button; optionally numbers per frame (`stats`) and a chart beside it whose marker follows the
slider. Use them wherever a reader learns by moving a setting.

```yaml
label: Edge width (cells)
caption: As the edge widens, cores shrink and small patches become islets.
values: [1, 2, 3, 4, 5, 6]
src: /img/tutorials/habitat-patterns/mspa/edge_{value}.png
```

or, with numbers and a linked chart (the chart's rows in the same order as the frames):

```yaml
label: Step
frames:
  - { value: 1, src: /img/.../step_1.png, caption: "2010", stats: { Forest: "54.1%", Farming: "35.9%" } }
  - { value: 2, src: /img/.../step_2.png, caption: "2020", stats: { Forest: "51.0%", Farming: "39.0%" } }
chart:
  type: line
  data: /data/tutorials/land-change/dinamica/areas.csv
  x: step
  y: [Forest, Farming]
  colors: { Forest: "#3b8a5a", Farming: "#d9a441" }
  ylabel: "% of the landscape"
```

### Before and after: ```tfcompare

```yaml
caption: Drag the handle to compare the observed map of 2010 with the simulated one.
before: { src: /img/.../observed.png, label: Observed 2010 }
after: { src: /img/.../simulated.png, label: Simulated 2010 }
```

The two images must have the same size and framing: draw them with the same settings and `legend=False`
(explain the colours in the caption or in a figure beside).

### Interactive charts: ```tfchart

Hover shows the values, the legend toggles series. Data from a CSV in `website/static/data/...` (or inline as
`csv: |`).

```yaml
type: line            # line | area | bar | scatter | dot
data: /data/tutorials/landscape-metrics/window/curve.csv
x: window
y: [shdi, ed]
labels: { shdi: "Shannon diversity", ed: "Edge density" }
colors: { shdi: "#3b8a5a" }
xlabel: Window side (cells)
ylabel: Value
stacked: false        # stacked areas or bars
percent: false        # values are percentages
height: 300
caption: How the window's size changes the metrics.
```

### Tables from data: ```tftable

A table from a CSV (a file under `website/static/`, or inline as `csv: |`), with numbers aligned and rounded.
`heat: true` shades the numeric cells by value, for matrices (transitions, correlations).

```yaml
data: /data/tutorials/change-detection/transitions/matrix.csv
columns: [from, forest, farming, water]      # optional: which columns, in which order
labels: { from: "From \ to" }               # optional: other headers
digits: 2
heat: true
caption: Cells that went from each class (rows) to each class (columns), 1997 to 2011.
```

`tfchart` also takes `xlog: true` for a logarithmic x axis (numbers of draws, distances, areas).

Every chart (but the one beside a slider) has a **Show the data** toggle under it: the chart's numbers as a table, in
the same format. `data_table: false` leaves it out.

#### Benchmark charts: speed-ups, timings

For the benchmarks section (speed-ups from about 1.2x to 1000x, seconds, accuracy). These options come with their own
palette, five slots in a fixed order and never cycled (light and dark steps, validated: forest, ember, water, red,
violet; `colors: { key: "#hex" }` still overrides one).

| Option | What it does |
|---|---|
| `horizontal: true` | With `type: bar`: the categories (column `x`) down the left, in the CSV order, the values along x. A long label is cut with an ellipsis (the whole text in the tooltip); on a narrow screen it goes above its bar. The height follows the number of rows (`height` is ignored). `xlabel` titles the value axis. |
| `log: true` | A logarithmic value axis (y for vertical charts, x for horizontal and dot). Ticks at 1, 10, 100, 1000 with faint minor lines at 2 to 9. Bars start at the axis minimum, 1 by default (or the decade below the smallest value if that is under 1), or `min:`; `max:` sets the top. `ylog` is an alias; `xlog` is still the x axis of a line or scatter. |
| `format: "{v}×"` | How a value reads, on the ticks, tooltips, labels and the data table (`"{v} s"`, `"{v}%"`). Only `format` exists (no `suffix`). `digits: 1` fixes the decimals. |
| `rule: { value: 1, label: same speed }` | A thin dashed line across the plot at that value, with a small label. |
| several `y` series | Grouped bars, 2 or 3 per category, a 2 px surface gap between them and a legend (click to hide a series). Put Terraflux first: it takes the forest green. |
| `labels_at_end: true` | The value at the end of each bar (for `dot`: of the last series, right of the row's rightmost dot), in text ink and in the `format`. |
| `type: dot` | A dot plot: one 10 px marker per category and series, with a thin segment between the series of a row (reference against Terraflux). Use it with `log: true`. |
| `color_by: domain` | With one series: colours the bars by a column's values, one palette slot per group in the order they first appear (five at most; more gives a warning), with a legend to hide a group. |

```yaml
type: bar
horizontal: true
log: true
format: "{v}×"
rule: { value: 1, label: same speed }
labels_at_end: true
color_by: domain
x: case
y: [speedup]
labels: { speedup: Speed-up }
xlabel: Times faster than the reference
csv: |
  case,speedup,domain
  "MSPA, CORINE 2000²",12,Fragmentation
  Cost distance,420,Distance
  Random walk,85,Connectivity
```

```yaml
type: dot
log: true
format: "{v} s"
x: case
y: [terraflux_s, reference_s]
labels: { terraflux_s: Terraflux, reference_s: Reference tool }
labels_at_end: true
csv: |
  case,terraflux_s,reference_s
  MSPA,0.9,41
  Random walk,12,1200
```

Every row takes the pointer or the keyboard focus (Tab), and shows the tooltip; the other rows dim. The charts redraw
on a theme switch and on a resize.

`tftable` options for the same section:

| Option | What it does |
|---|---|
| `align: { case: left, speedup: right }` | Aligns a column (`left`, `center`, `right`); a single word aligns all columns but the first. Numbers are right-aligned by default. |
| `digits: { speedup: 1, reference_s: 2 }` | Decimals per column, always shown (`17.0`); `default` sets the others. A single number (as before) keeps whole numbers plain. |
| `format: { speedup: "{v}×" }` | The text of a numeric column (`{v}` is the number). |
| `sortable: true` | A click on a header sorts by it (ascending, descending, the CSV order again); the headers are buttons (Tab, Enter or Space) and `aria-sort` follows. |
| `badge: { result: { identical: good, differs: warning } }` | Turns the listed values of a column into small pills: `good`, `warning`, `bad` or `info`. A pill always carries the text (and a mark: check, !, cross), never the colour alone. |
| `caption` | The line under the table (Markdown); the table is labelled by it. |

```yaml
sortable: true
align: { case: left }
digits: { speedup: 1, reference_s: 2 }
format: { speedup: "{v}×" }
badge:
  result: { identical: good, "within 1e-9": good, differs: warning }
caption: Speed and accuracy against the reference.
csv: |
  case,speedup,reference_s,result
  MSPA,12,41,identical
  Cost distance,420,310.5,within 1e-9
```

### Calculator playgrounds: ```tfcalc

A script editor that runs Terraflux's own calculator in the browser (the map algebra of `crates/ops`, compiled to
WebAssembly in `static/js/calc-engine.js`) over Sibuyan's maps (`kind: raster`) or its land-use patches
(`kind: vector`), with the examples as presets. The page loads `calc.js` and `calc.css` only where a block is.

```yaml
kind: vector                  # raster (the default) or vector
start: 0                      # optional: the preset shown first
presets:
  - name: Areas in hectares
    op: filter                # vector only: calc (the default) or filter
    column: area_ha           # vector.calc only: the column of a script with no out
    inputs: { min_ha: 10 }    # optional: the node's inputs, numbers or texts ("forest")
    script: |
      out area_ha = geometry.area / 10000
caption: Edit the script: the results follow as you type.
```

Every preset is checked against `terraflux run` by `python website/figures/calculators/playground.py` (with
Node.js): run it after changing the presets or the engine.

### Command help: ```tfcli

The help of a command, from the binary: a block whose content is `run`, `submit`, `cluster add`... (an empty
block gives the top-level help).

### Operator links

`[[landscape.mspa]]` links to the operator's reference with its description as a tooltip;
`[[landscape.mspa|MSPA]]` with other text. The build warns about unknown operators.

### Other Markdown

- Admonitions: `!!! tip`, `!!! note`, `!!! warning`, `!!! danger`, `!!! science "Key idea"` (concepts,
  formulas), `!!! paper "In the literature"` (how studies used it), `!!! app "In the application"` (what to
  click). Collapsible: `??? note "Title"`.
  The types of mkdocs-material are styled too, for the pages moved from the library's docs: `abstract`, `success`,
  `failure`, `example`, `question` (and `info`, `note`, `tip`, `warning`, `danger`), with `??? type "Title"` for the
  collapsible ones. `--8<--` snippets are not supported: write the text in the page.
- Tabs: `=== "Application"` / `=== "Command line"` / `=== "Model file"`, content indented 4 spaces.
- Steps: an ordered list inside `<div class="steps" markdown>` ... `</div>` (blank lines around the list)
  renders as numbered steps with a line joining them.
- Keys: `++ctrl+enter++`, `++f5++`. Tables, footnotes, definition lists, `==highlight==`.
- Code blocks with a language (`toml`, `bash`, `powershell`, `python`, `text`).

## The Python library's pages (`python` section)

The library (`extern/terraflux-py`, a submodule; `git submodule update --init extern/terraflux-py`) has its docs in
the `python` section: `content/python/` (getting started, tutorials, API reference, algorithms, changelog), plus the
`*-in-python.md` pages beside the GUI tutorials.

**Sidebar groups.** A section whose pages live in subfolders (`python/getting-started/x.md`, `python/api/x.md`,
`python/api/x.md`) shows a heading for each subfolder, with a fold that opens
for the page being read; the section's own pages (`python/index.md`, `python/changelog.md`) stay in the list. A
section with no subfolder is a flat list, as before. Pages are ordered by their front matter `order:` (then by
title). A group's heading comes from `groups:` of its section in `site.yaml` (a text, or `{ title, order }`);
without an entry, from the title of the page at `/<section>/<subfolder>/` (its `index.md`, or `<subfolder>.md`:
that page is listed first in the group as "Overview"), else from the folder's name. The group's place among the
section's pages is the entry's `order`, else the lead page's `order`, else last. Previous and next follow the
sidebar's order.

**API reference, generated.** `python website/tools/refresh_python_api.py` reads the library's sources with
griffe (nothing is imported or built; `pip install griffelib`) and writes `website/data/python_api.json`, which is
committed (the website's workflow makes it again from the submodule and warns if it differs). Run it when the
submodule moves. In a page, a line of its own

````markdown
::: terraflux.mspa
````

(the syntax of the old mkdocstrings pages) becomes a heading, the signature, the summary, the parameter, returns
and raises tables, the examples as highlighted code, notes, a link to the source on GitHub at the version's tag,
and, for a class, its public methods and properties. An unknown path fails the build. The path is the public one
(`terraflux.mspa`, `terraflux.Landscape`, `terraflux.circuit.omniscape`...); it is also the anchor, so the object
is at `/python/api/<page>/#terraflux.mspa`. Options, as the old pages gave them, indented under the line:

````markdown
::: terraflux.Landscape
    options:
      members: [patch_metrics, class_metrics]   # false: none; a list: those, in that order; default: all
      show_root_heading: false                  # no heading (the default is a heading of level 3)
      show_root_full_path: true                 # the heading shows terraflux.Landscape, not Landscape
      heading_level: 2
      show_source: false                        # no link to the source; show_signature, show_docstring too
````

Links to an object, from a page or from a docstring: `[`tf.mspa`][terraflux.mspa]` (the form mkdocs'
autorefs gave; the target is a public path) becomes a link to the page that documents it by a `:::` line, and is
plain text if no page does (`build.py --check` counts those). `{{pyversion}}` in a page is the library's
version.

**The changelog.** A line `{{< include extern/terraflux-py/CHANGELOG.md >}}` of its own is replaced, when the site
is built, by that file (a path from the repository's root; its leading `# Title` line is dropped), so the library's
repository stays the source of `content/python/changelog.md`. Without the submodule the build warns (and fails when
the `CI` variable is set).

**Checking the examples.** `python website/tools/check_python_blocks.py [pages]` runs every ```` ```python ````
block of `content/python/**` and `content/**/*-in-python.md` in page order, the blocks of a page
sharing a namespace, in a temporary folder, a process per page with a time limit (`--timeout`, 600 s), and reports
the page and the block that fails. A block that cannot run (a file the reader has, a long job) is preceded by a
line `<!-- no-run -->`. `--list` shows the pages and their blocks; `--python` names the interpreter with the library
installed. At most 4 threads in all (`--jobs` pages at once, default 2; the rest of the 4 goes to each page's
OpenMP, Rayon and dask). It is not in the CI.

## Figures: made from real runs

Every map, chart and table of results comes from **running Terraflux**, by a script kept in the repository so
the figures can be made again:

- The script: `website/figures/<category>/<slug>.py`, using `website/tools/figs.py` (read its doc string). It
  runs a model with the binary (`target/release/terraflux.exe`), adding `raster.write` nodes or outputs for what
  it draws (`figs.add_writes`), and draws rasters (`figs.raster`, `figs.frames`), charts (`figs.chart`) and
  writes chart data (`figs.write_csv`, `figs.class_areas`).
- Images to `website/static/img/tutorials/<category>/<slug>/`, chart data to
  `website/static/data/tutorials/<category>/<slug>/`. PNG, 600 to 1100 px wide; the build adds WebP copies.
  Keep a tutorial's images under about 3 MB in all.
- Explanatory drawings (how a method works: a toy grid with MSPA's classes, a circuit, a cost surface, a
  removal curve) are welcome too: draw them with matplotlib in the same script, in the same style (figs sets
  it), or as inline SVG in the page when simpler. Prefer a small toy example the reader can follow cell by cell.
- The application's own screenshots are in `/img/app/` (graph.png: the builder; map.png: a simulation's map;
  validation.png). Do not fake screenshots.
- Use the site's colours for land use (`figs.LAND`), GuidosToolbox's for its products (`figs.gtb_colors`,
  `figs.mspa_palette()`), viridis for continuous values, a diverging ramp centred on 0 for differences.

## Checking your pages

```bash
python website/figures/<category>/<slug>.py          # makes the figures
python website/build.py --check --only tutorials/<category> --out website/.cache/site-<category>
```

The build must print **no warnings** for your pages and `--check` **no graph errors**. Look at your pages: serve
the folder (`python -m http.server 8811 --directory website/.cache/site-<category>`) and take screenshots
with headless Chrome:

```bash
"/c/Program Files/Google/Chrome/Application/chrome.exe" --headless=new --disable-gpu --hide-scrollbars \
  --virtual-time-budget=4000 --window-size=1440,4000 --screenshot=C:\path\to\shot.png \
  "http://localhost:8811/tutorials/<category>/<slug>/?theme=light"
```

Then read the screenshot and fix what looks wrong (a figure too large, a graph unreadable, a caption missing).

## Rules for working in parallel

- Write only your own files: your content folder(s), `website/figures/<your category>/`, and your folders under
  `website/static/img/tutorials/` and `website/static/data/tutorials/`.
- Do **not** edit the shared files (`build.py`, `lib/`, `templates/`, `static/css`, `static/js`, `site.yaml`,
  `tools/figs.py`, this file). If you need a component that does not exist, say so in your final report.
- Build into your own `--out` folder, never into `website/dist`.
- Do not commit.
