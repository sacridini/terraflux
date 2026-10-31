---
title: "The model's document: a model read as its methods"
short: The model's document
description: Read a model as the methods section of a paper in the Document pane, a sentence for each step with its settings and where its inputs come from, change settings there, follow its checks, group it by stage, and export it as an HTML or PDF report.
order: 80
level: beginner
time: 20 min
template: MSPA of a forest
ops: []
---

A graph of boxes is a good way to build a model, and a poor way to explain it to someone who did not build it: a
co-author, a reviewer, the manager of a reserve who will act on its maps, or yourself in a year. Terraflux reads
every model a second way, as a **document**: the steps in the order a reader follows them, each told in a sentence
with its settings, where its inputs come from, why it is there, and the map or table it gave in the last run. In
this tutorial you will open the document of a template, read it step by step, change a setting from it, see how
its checks and its stages are shown, and write it as a report to send. At the end you will know how to make a model
of yours read well, which is mostly a matter of writing a note and a *why* on every node.

## The idea: the graph and its methods section

The **Document** pane is beside the Model pane (its tab is next to the Model's when the application opens). It
holds no information of its own: it is the same model, the same file, read in another order. What it shows comes
from three places:

- the **graph**: the nodes, their operators, their settings and their connections;
- the **record** you (or an assistant) wrote: each node's note, its *why*, its references, assumptions and
  alternatives, and the model's name, description, request, summary and author;
- the **last run**: the maps and tables the nodes gave.

So a model whose nodes are explained reads almost like the methods section of a paper, and a change on the canvas
is in the document at once. Ecological modellers have long asked for models to be described fully enough to be
rebuilt, with standard protocols such as ODD for agent-based models (Grimm et al., 2020), and for the record of an
analysis to be kept with it rather than in a separate text that drifts away from the code (Sandve et al., 2013).
The document does not replace your methods section; it is the checked list of what the model really does, to write
it from and to compare it with.

!!! science "Key idea"
    The document is written from the model, never the other way round. If a sentence of the document says
    something you did not mean, the model does it: change the model, not the text.

## When to use it

- **Before you trust a model**, and above all one built by someone else or by an [AI assistant](/cli/assistants/):
  read it top to bottom, and check that every step is one you would have chosen.
- **To write the methods** of a paper or a report: the stages, the settings with their units, the files read.
- **To send a model to someone who does not use Terraflux**: the HTML report opens in any web browser.
- **To find the steps nobody explained**: the document counts them.

It is not a substitute for the [operator reference](/operators/), which says what each operator computes and from
which paper; the document says what *this* model does with it.

## What you need

Only the application. The examples use the template **MSPA of a forest** (Templates, Patterns (GuidosToolbox)),
the same as in [the tour](../../getting-started/tour/): a landscape drawn by chance, its forest made MSPA's
foreground, MSPA, and a plot. Open it, let it run, and click the **Document** tab.

## Read a model as a document

At the top are the model's **name** and its **description** in italics; then, when the model has them, its
**Request** (what it was built to answer, in the words of whoever asked), its **Author** (a person, an assistant,
or both) and its **Summary**. You write these in the Node pane when no node is selected (its section *The
model*); the application's assistant adds itself to the authors, and writes the request it answered when the
model has none.

A line above the document counts the steps and says how many have **no note or no why** (hover over it: *what each
step is for and why it was chosen: the Node pane writes them*). The template's nodes have notes and no *why*, so
it says so. On the right, **Export report...** writes the document as a file (below).

### The sections: the stages of a model

The steps are told in up to five stages, in this order, each a numbered section:

| Stage | What goes in it |
|---|---|
| **Data** | what is read or drawn: the nodes that take nothing from other nodes (a [[raster.read]], a neutral landscape) |
| **Preparation** | what is done to the data before the model (reprojections, reclassifications, distances, stacks) |
| **Model** | what is fitted, simulated or optimised (a regression, MSPA, a loop of a simulation), and what lies between two of its nodes |
| **Validation** | the model checked: the `validate.*` operators, accuracy, [[check.assert]], the evaluation of a species distribution model, importance |
| **Results** | what comes of the model: maps made from it, the files written, the plots |

The stage of a node comes from its operator and from its place in the graph: the same [[raster.calc]] is
preparation before the model and a result after it. A model with no model stage (a set of landscape metrics, say)
calls its preparation **Analysis**. When the model has inputs (`$name`), parameters or scenarios, sections for them
come first; when it has outputs, a last section lists them with the files they are written to.

Within a stage, each step comes after what it depends on, and the ties are broken by where the nodes are on the
canvas, left to right, then top to bottom: the order in which you laid them out is the order in which the document
tells them. Zoomed out, the canvas frames and names the same stages around their nodes.

In the template, the document has three sections: **1. Data** (`landscape`), **2. Preparation** (`forest`) and
**3. Model** (`mspa`). The plot is not a step: it is **Figure 1**, under the step whose map it shows.

### A step

Each step is a card. Its number (`2.1`) and name come first, then its operator in small grey letters; the name is a
link that selects the node on the canvas (and the Node pane shows all of it). Then, in order:

- its **note**, in italics (*No note.* when it has none), and its **Why.**;
- **a sentence of what it does**, written for its operator, with its settings and where its inputs come from
  (next section);
- the **file** it reads or writes, for the operators of files;
- the **expression** of a [[raster.calc]], with *where* and the node each name comes from;
- otherwise, what it **takes**: each input and the node or model input it comes from, a link to that node;
- its **settings**, one a line, in the words of the operator's documentation when they are short ("the edge's width
  in cells, 1-255"), with *(default ...)* beside a value that differs from the operator's default;
- for a loop, how many times it is **repeated**, what it **carries** from step to step (from what, then what),
  what it **collects** and **gives**, when it **stops**, and the steps of its body inside it, numbered `3.4.1`,
  `3.4.2`...;
- the node's **References**, **Assumptions** and **Alternatives**, when it has them;
- the map or the table it gave in the last run, and the figures of the plots that show it.

Two kinds of steps take less room. The data read or drawn are rows of a table at the top, with a small picture of
each map, and maps read the same way with the same note share a row (the three dates of a land use map are one
row, not three cards). The plumbing (stacks, random generators, the history of a CLUE-S simulation) without a *why*
is a line.

## A sentence for each step

For the operators models use most, a step is told in a sentence written for its operator, with the values of its
settings and the names of the nodes its inputs come from. The three steps of the template read:

> Random clusters on 300 × 300 cells, a share 0.55 of the cells in clusters, the classes weighted 0.3, 0.15, 0.55;
> seed 7.
>
> Landscape with its values reclassified: `3 -> 2; * -> 1`.
>
> MSPA's patterns of forest: connected by 8 neighbours, the edges 1 cell wide.

A setting that is not given is said with the operator's default, because the operator uses it: a
[[nlm.gaussian_field]] given only its size and seed reads *A Gaussian random field of 100 × 120 cells:
autocorrelation range 10 cells, variance 5, nugget 0.2; seed 7.* A detail that is optional and not given is left
out (the template's MSPA has no `foreground` set, so the sentence does not mention one), and a step whose operator
needs an input that is not connected has no sentence until it is.

The operators with a sentence:

| Subject | Operators |
|---|---|
| Reading and writing | `raster.read`, `raster.write`, `vector.read`, `vector.write`, `table.read`, `view.plot` |
| Neutral landscapes | `nlm.random`, `nlm.gaussian_field`, `nlm.random_cluster`, `nlm.fbm`, `nlm.planar_gradient`, `nlm.distance_gradient`, `nlm.binarize` |
| Preparation | `raster.reclassify`, `raster.reproject`, `raster.resample` |
| Land change | `markov.fit`, `glm.fit`, `glm.predict`, `woe.fit`, `woe.probability`, `demand.from_maps`, `dinamica.step`, `lulcc.step`, `patches.observe` |
| Patterns and metrics | `landscape.mspa`, `metrics.class`, `metrics.landscape`, `metrics.patch` |
| Validation | `validate.agreement`, `validate.kappa` |
| Species, services, planning | `sdm.fit`, `sdm.area_of_habitat`, `services.carbon`, `prioritize.optimize` |

The other operators have no sentence: their step lists their inputs and settings, and their note says the rest.
The sentence never replaces the note: it says *what* the step does, the note *what for*, and the why *why this and
not another*.

## Change a setting from the document

A setting of a single value is a field in the document itself: click it, type, and press ++tab++ or click
elsewhere; a setting with a few possible values (MSPA's `connectivity`, 8 or 4) is chosen in a list. A setting
that is a list or a table of values (the weights of the classes of a neutral landscape, a list of class codes) is
edited **item by item**, as in the Node pane: each item on a row, with its number or its name, its value, and **x** to take it
away; **+ add** adds one more at the end of a list (a copy of the last), and a table has a *name* and a *value*
field and **Add**. Other settings (nested tables) say *edited in the Node pane (select the step)* when you hover
over them.

After a change, the whole model runs again, since the document shows every step, and its sentences, maps and
tables follow. Try it: in **3.1 mspa**, set the edge's width to 3. The sentence now reads *the edges 3 cell wide*,
the setting shows *(default 1)* beside it, and Figure 1 shows the wider edges.

## Its maps and tables

The map of each step is the one the Viewer shows, **in the Viewer's colours**: the palette you chose by hand for
that output (**Colours**), else the one that fits what made it (MSPA's classes in GuidosToolbox's colours,
elevations, ranks...), and classes in the colours of the model's legend when it has one (a [[raster.legend]] node,
or the legend you edited in the Viewer). A stack shows its first layer, and the maps of a loop its last step, with
a line saying so (*step 14 of 14*). A table shows its first rows (*and 40 more rows*). A step that has not run says
*Not run.*

A [[check.assert]] shows on its step how it fared in the last run: **✔ passed**, **! failed, warning only**,
**✖ failed, the run stopped** (with the message), or **· not run yet** and **· turned off**. When the model has
checks, the line above the document gives the model's health (*3 checks: 2 passed, 1 warned*), each check listed
when you hover over it. A reader sees at once which of the model's own tests held.

## A section per stage: groups

A long model reads better on the canvas, too, when its top level is a few blocks. **Group the model by stage
(data, preparation, model...)**, in the commands (++ctrl+k++), puts the top-level nodes of each stage that has at
least two into a [[control.group]] named `data`, `preparation` (or `analysis`), `model`, `validation` or
`results`, each with a note that says what it is for, and arranges the canvas again. The plots, the files written
and the random generators stay outside, at the top level. The document then reads a section per group, with its
note under its title and a link to the group. It is refused when the top level has groups already. See
[Groups and submodels](../groups-and-submodels/) for groups in general.

## Export a report

**Export report...**, at the top right of the Document pane or in the **File** menu, asks where to write it, as *A
web page (HTML)* or as a *PDF*:

- the **HTML** file is a single file, its images inside it: it opens in any web browser, without Terraflux, and can
  be sent by e-mail or put in a paper's supplementary material;
- the **PDF** has the same content, laid out on pages.

Both hold the document as the pane shows it: the model's name, description, request, author and summary, the
numbered sections and steps with their notes, sentences, settings, references, assumptions and alternatives, the
figures, and the outputs. The images come from the **last run**: run the model before you export it, or its steps
say *Not run.* The report's maps are drawn in colours fitted to their values (a colour per class, or a ramp), not
in the palette chosen in the Viewer. A line at the end says which version of Terraflux made it and on which day.

From the command line, `terraflux report ./model.toml` writes the same report next to the model (its name and
`.html`); `--out report.pdf` chooses the file and its kind by the extension, and `--run` runs the model first, so
that the steps have their maps and tables.

!!! tip "Send the model with its report"
    The report is what a colleague reads; the model file is what they run. Send both: the report says what the
    model does, and `terraflux run` on the file gives the same results.

## Methods and citations

**Methods and citations**, at the top right of the Document pane, in the **File** menu or in the commands
(ctrl+K), drafts the methods section of a paper from the model, in a window:

- **a paragraph** that names the version of Terraflux and the methods the model uses, each with its papers and
  the software whose results Terraflux reproduces: *"We computed morphological spatial pattern analysis (MSPA;
  Soille & Vogt, 2009) with Terraflux, which reproduces GuidosToolbox (Vogt & Riitters, 2017)."*;
- **the model, step by step**: each node's sentence (the one of the document), or the first words of its
  operator's description and the settings given, with its note;
- **the references**: those of the methods used and Terraflux's own, in the APA style, and the references the
  nodes give in their notes (*refs*), as they are written.

**Copy all**, **Copy the paragraph** and **Copy BibTeX** put them on the clipboard; **Save BibTeX...** and **Save
RIS...** write the references for LaTeX, Zotero, Mendeley or EndNote. The papers are those of
[Cite Terraflux](/about/citing/): a method without a paper there (a neutral landscape, map algebra) is told
without one. The text is a draft: it says what the model does, not why each step was chosen; read it and complete
it before it goes in a paper.

## Common pitfalls

- **A step without a note or a why.** The document is only as good as the record: a card with *No note.* tells a
  reader only the operator and its settings. Write in the Node pane what each node is for (**Note**) and why it was
  chosen (**Why**), and the references and assumptions behind its settings.
- **A report exported before the run.** Its maps and tables come from the last run; after a change, run the model
  again before you export.
- **Steps in a surprising order.** Within a stage, the ties follow the canvas: move the nodes, left to right, into
  the order you want them told (or press **Arrange**, ++ctrl+l++).
- **A sentence that mentions a default you did not choose.** It is the value the operator uses: if it is not the
  right one for your landscape, set it.
- **Letters outside Western European alphabets in a PDF.** The PDF is written with the standard fonts of PDF
  readers, which have Latin letters only; other characters come out as `?`. Use the HTML report for notes in Greek,
  Cyrillic or Chinese.

## Going further

- [The tour](../../getting-started/tour/) shows the Document and Changes panes among the others, and how to
  trace why a cell has its value.
- [AI assistants](/cli/assistants/): what an assistant built, checked in the document and in the Changes pane.
- [Checks, tests and reproducible runs](../checks-and-tests/): the checks behind the badges and the health line,
  tests, manifests and frozen nodes.
- [Plots and tables](../plots-and-tables/): the plots that become the document's figures.
- [Groups and submodels](../groups-and-submodels/): groups, which the document reads as sections.

## References

- Grimm, V., Railsback, S. F., Vincenot, C. E., Berger, U., Gallagher, C., DeAngelis, D. L., et al. (2020). The ODD protocol for describing agent-based and other simulation models: a second update to improve clarity, replication, and structural realism. *Journal of Artificial Societies and Social Simulation*, 23(2), 7. https://doi.org/10.18564/jasss.4259
- Sandve, G. K., Nekrutenko, A., Taylor, J., & Hovig, E. (2013). Ten simple rules for reproducible computational research. *PLoS Computational Biology*, 9(10), e1003285. https://doi.org/10.1371/journal.pcbi.1003285
