#!/usr/bin/env python3
"""Write the overview of the Python API reference, content/python/api/index.md.

Port of the library's own tools/make_api_index.py (which imported the library): the table of every function and
class is read from the `:::` lines of the pages in content/python/api/ and, for the first sentence of each
summary, from data/python_api.json (made by refresh_python_api.py), so nothing is imported. Run it again after
adding or removing an entry on a page, or after the data file changes:

    python website/tools/make_python_api_index.py
"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
API = ROOT / "content" / "python" / "api"
sys.path.insert(0, str(ROOT))
from lib.pyapi import resolve_path  # noqa: E402

# (page, title, what it covers), in the order of the index and of the sidebar
PAGES = [
    ("data", "Data & configuration", "Read, write and reclassify rasters, collections, colour tables, threads."),
    ("download", "Data on demand", "WorldClim, CHELSA, Copernicus DEM, SoilGrids, MapBiomas, Hydrography90m, STAC, onto a map's grid."),
    ("patterns", "Habitat patterns", "MSPA, SPA, objects by size, parcels and contortion."),
    ("fragmentation", "Fragmentation", "Density, fragmentation classes, landscape mosaics, contagion, entropy."),
    ("landscape-metrics", "Landscape metrics", "Patch, class and landscape metrics; windows, plots, zones, time series."),
    ("networks", "Habitat networks", "Habitat graphs, IIC/PC/ECA, importance, ProtConn, grids, metapopulations, MSPA networks."),
    ("circuits", "Circuit connectivity", "Omniscape and Circuitscape: current, resistance distance, networks."),
    ("surface", "Surface metrics", "Texture of continuous rasters (ISO 25178): roughness, shape, trend surfaces."),
    ("hydro", "Terrain hydrology", "Filling, flow directions and accumulation, streams, watersheds, HAND, wetness indices, river networks."),
    ("swat", "Watershed model (SWAT+)", "SWAT+ projects from TxtInOut folders or rasters, daily water, sediment and nutrients, results on maps."),
    ("cost", "Grid distances", "Transitions, cost surfaces, least-cost and random-walk distances and paths."),
    ("distance", "Distance & restoration", "Distances, resistance, least-cost paths, corridors, zones of influence, original vegetation, intactness."),
    ("change", "Change detection", "Habitat, fragmentation and landscape-mosaic change between dates."),
    ("batch", "Large data & batch", "Folders of rasters, tiled MSPA, maps larger than memory."),
    ("nlm", "Neutral landscape models", "Simulated landscapes as null models: random, gradients, clusters, fractals."),
    ("biodiversity", "Biodiversity analysis", "Species counts per cell, phylogenies and dissimilarity matrices (Biodiverse)."),
    ("prioritization", "Spatial prioritization", "Zonation's ranking of the cells for conservation: rules, weights, costs, curves."),
    ("sdm", "Species distributions", "Clean records, choose predictors and background, spatial folds, fit, evaluate, project, disperse."),
    ("move", "Animal movement", "Movebank tracks, steps, speeds and turns, filters, track lines (move2); random steps for step selection (amt)."),
    ("simulate", "Land change simulation", "Land use change: transitions, potentials, CLUE / Dinamica / PLUS allocation, validation."),
    ("accuracy", "Map accuracy & area", "Stratified samples of a map, its accuracy and the area of its classes (Olofsson et al. 2014)."),
    ("redd", "Deforestation allocation (VT0007)", "Verra's VT0007 as UDef-ARP: risk threshold, vulnerability, fit, allocation, assessment."),
    ("forest-code", "Brazil's Forest Code", "APPs of rivers, lakes, springs, slopes, hilltops; the Reserva Legal's balance per property."),
    ("services", "Ecosystem services", "InVEST's models: carbon, habitat quality, water yield and hydropower, sediment delivery."),
    ("uncertainty", "Uncertainty", "Totals, products, the delta method, Monte Carlo and confidence intervals."),
    ("sensitivity", "Sensitivity analysis", "Morris's elementary effects and Sobol's indices of a model's parameters (SALib's)."),
    ("stats", "Statistics of maps", "Trends of every cell, Moran's I, LISA, Gi*; kriging, IDW; cell statistics, local correlation."),
]

INTRO = """---
title: API reference
description: Every function and class of the Python library, with its parameters, what it returns and an example you can run, generated from its docstrings.
order: 90
---

Every public function and class, with its parameters, what it returns and an example you can run. Each page covers
one topic; the [tutorials](/python/tutorials/) show the same tools at work on real maps. The reference is generated
from the library's docstrings (`website/tools/refresh_python_api.py`), so it always matches the version it
describes, {{pyversion}}.

<!-- no-run -->
```python
import terraflux as tf             # most tools: tf.mspa, tf.fad, tf.Landscape...
tf.conn.protconn(...)               # habitat networks and protected areas
from terraflux import circuit      # Circuitscape
tf.cost.transition(r, "mean", 8)    # distances and routes on grids
tf.surface.metrics(z)               # surface texture of continuous rasters
tf.nlm.random(100, 100, seed=1)     # neutral landscape models
tf.bio.BaseData.from_table(...)     # biodiversity analysis
tf.prioritize.zonation(features)    # spatial conservation prioritization
tf.sdm.prepare(occ, env)            # species distribution models
```

!!! example "Every example runs"
    The examples use the maps that ship with terraflux (`tf.example_data`) or
    small arrays built in place, and they are run as tests, outputs included.
    Copy one into a Python session to try a function.

"""


def summary(text: str) -> str:
    """First sentence of a docstring's summary."""
    doc = " ".join(text.split())
    doc = re.sub(r"\[`([^`]*?)(?:\(\))?`\]\[[^\]]*\]", r"`" + chr(92) + "1`", doc)  # an autoref is plain code here
    doc = re.split(r"\. (?=[A-Z])", doc)[0]
    return doc.rstrip(".").replace("|", r"\|")


def prefix(ident: str) -> str:
    """How a user writes an object: tf.x, tf.conn.x, circuit.x (omniscape is tf.omniscape)."""
    rest = ident.split(".", 1)[1]
    if rest.startswith("circuit."):
        return "tf." if rest.endswith(".omniscape") else "circuit."
    return "tf." + rest.rsplit(".", 1)[0] + "." if "." in rest else "tf."


def main() -> None:
    d = json.loads((ROOT / "data" / "python_api.json").read_text(encoding="utf-8"))
    objects, aliases = d["objects"], d["aliases"]
    out = [INTRO, "The pages, one per area:\n"]
    for page, title, text in PAGES:
        out.append(f"- **[{title}](/python/api/{page}/)**: {text}")
    out.append("\n## All functions and classes\n")
    for page, title, _ in PAGES:
        ids = re.findall(r"^::: (\S+)", (API / f"{page}.md").read_text(encoding="utf-8"), flags=re.MULTILINE)
        out.append(f"### {title}\n\n| Name | Summary |\n|---|---|")
        for ident in ids:
            key = resolve_path(ident, objects, aliases)
            if not key:
                raise SystemExit(f"{page}: ::: {ident} is not in data/python_api.json")
            name = ident.split(".")[-1]
            out.append(f"| [`{prefix(ident)}{name}`](/python/api/{page}/#{ident}) | {summary(objects[key]['summary'])} |")
        out.append("")
    (API / "index.md").write_text("\n".join(out), encoding="utf-8", newline="\n")
    print(f"wrote {API / 'index.md'}")


if __name__ == "__main__":
    main()
