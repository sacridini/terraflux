"""Figures of 'MSPA step by step on a real forest map' (habitat-patterns/mspa).

Two real maps: CORINE Land Cover near Hannover (reclassified to a forest map, then MSPA) and GuidosToolbox's
sample forest map (MSPA with its settings: edge widths 1 to 6, 8 and 4 neighbours, internal features on and off).

    python website/figures/habitat-patterns/mspa.py
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import gtbfig as g  # noqa: E402
from gtbfig import figs, np  # noqa: E402

NAME = "habitat-patterns/mspa"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)
g.copy_data(g.CLC, work)
g.copy_data(g.GTB_INPUT, work)
# the reader's copies of the two maps
g.copy_data(g.CLC, data)
g.copy_data(g.GTB_INPUT, data)

# ------------------------------------------------------------------------------------------------ the runs
nodes = [
    g.node("landcover", "raster.read", path="clc.tif"),
    g.node("clc_forest", "raster.reclassify", {"map": "landcover.map"}, rules="23 24 25 -> 2\n* -> 1"),
    g.node("clc_mspa", "landscape.mspa", {"map": "clc_forest.map"}, edge_width=4),
    g.node("w1", "raster.write", {"value": "clc_forest.map"}, path="clc_forest.tif"),
    g.node("w2", "raster.write", {"value": "clc_mspa.map"}, path="clc_mspa.tif"),
    g.node("forest", "raster.read", path="input.tif"),
]
runs = {}
for ew in range(1, 7):
    runs[f"e{ew}"] = dict(edge_width=ew)
runs["c4"] = dict(edge_width=1, connectivity=4)
runs["noint"] = dict(edge_width=1, internal=False)
for k, p in runs.items():
    nodes.append(g.node(k, "landscape.mspa", {"map": "forest.map"}, **p))
    nodes.append(g.node("w_" + k, "raster.write", {"value": f"{k}.map"}, path=f"{k}.tif"))
figs.run(g.toml_nodes(*nodes), work)
M = {k: g.read(work / f"{k}.tif") for k in runs}

# ------------------------------------------------------------------------------------------------ CORINE
lc = g.read(work / "clc.tif")
groups = {  # CORINE's level-1 and level-2 groups, by the raster's codes (1-44)
    1: ("Artificial surfaces", "#c8507d", range(1, 12)),
    2: ("Arable land", "#f3e27a", range(12, 18)),
    3: ("Pastures", "#d6e68a", range(18, 19)),
    4: ("Mixed farming", "#e8c27a", range(19, 23)),
    5: ("Forest", "#2f7d47", range(23, 26)),
    6: ("Open natural", "#a7c957", range(26, 35)),
    7: ("Wetlands", "#9aa8d8", range(35, 40)),
    8: ("Water", "#3f7fae", range(40, 45)),
}
lcg = np.zeros(lc.shape, np.int32)
for k, (_, _, rng) in groups.items():
    lcg[np.isin(lc, list(rng))] = k
g.classmap(lcg, out / "clc-landcover.png", {k: (n, c) for k, (n, c, _) in groups.items()}, width=900, counts=True,
           legend_width=0.34)
cf = g.read(work / "clc_forest.tif")
FOREST = {2: ("Forest (2)", figs.FOREST), 1: ("Background (1)", "#e9e4d8"), 0: ("Missing (0)", "#ffffff")}
g.classmap(cf, out / "clc-forest.png", FOREST, order=[2, 1], width=900, counts=True)
cm = g.mspa_fold(g.read(work / "clc_mspa.tif"))
g.classmap(cm, out / "clc-mspa.png", g.MSPA_FOLDED, order=g.MSPA_ORDER[:-1], width=900)
clc_sh = g.mspa_shares(g.read(work / "clc_mspa.tif"))

# ------------------------------------------------------------------------------------------------ GTB's sample
fo = g.read(work / "input.tif")
g.classmap(fo, out / "forest.png", FOREST, order=[2, 1, 0], width=900, counts=True)
m1 = M["e1"]
g.classmap(g.mspa_fold(m1), out / "mspa.png", g.MSPA_FOLDED, order=g.MSPA_ORDER, width=1000)

# a zoom: forest and its MSPA, the same frame
R = slice(650, 950)
C = slice(250, 550)
g.classmap(fo[R, C], out / "zoom-forest.png", FOREST, legend=False, width=760)
g.classmap(g.mspa_fold(m1)[R, C], out / "zoom-mspa.png", g.MSPA_FOLDED, legend=False, width=760)
g.classmap(g.mspa_fold(m1)[R, C], out / "zoom-mspa-legend.png", g.MSPA_FOLDED, order=g.MSPA_ORDER[:-1], width=900)

# the classes' shares (the chart of the reading section)
CLS = [17, 9, 5, 3, 65, 33, 1]
f1 = g.mspa_fold(m1)
nfg = np.isin(f1, CLS).sum()
rows = []
for k in CLS:
    n = int((f1 == k).sum())
    rows.append({"class": g.MSPA_FOLDED[k][0], "cells": n, "forest": round(n / nfg * 100, 2),
                 "map": round(n / f1.size * 100, 2)})
figs.write_csv(data / "classes.csv", rows)
# the raw codes, for the table of the page
codes = {int(v): int(c) for v, c in zip(*np.unique(m1, return_counts=True))}

# ------------------------------------------------------------------------------------------------ edge width
COL = {"Core": "#00c800", "Edge": "#000000", "Perforation": "#0000ff", "Islet": "#a03c00", "Bridge": "#ff0000",
       "Loop": "#c9a800", "Branch": "#ff8c00"}
srows = []
frames = []
for ew in range(1, 7):
    s = g.mspa_shares(M[f"e{ew}"])
    srows.append({"edge_width": ew, **s})
    g.classmap(g.mspa_fold(M[f"e{ew}"])[R, C], out / f"edge-{ew}.png", g.MSPA_FOLDED, legend=False, width=760)
    frames.append({"value": ew, "src": f"/img/tutorials/{NAME}/edge-{ew}.png",
                   "caption": f"edge_width = {ew}: an edge {ew} cell{'s' if ew > 1 else ''} deep",
                   "stats": {k: f"{s[k]:.1f}%" for k in ("Core", "Edge", "Bridge", "Loop", "Branch", "Islet")}})
figs.write_csv(data / "edge-width.csv", srows)

# ------------------------------------------------------------------------------------------------ 8 and 4 neighbours
f4 = g.mspa_fold(M["c4"])
diff = f1 != f4
best = max(((int(diff[r:r + 200, c:c + 200].sum()), r, c) for r in range(0, 801, 50) for c in range(0, 801, 50)))
_, r0, c0 = best
R2, C2 = slice(r0, r0 + 200), slice(c0, c0 + 200)
g.classmap(f1[R2, C2], out / "conn-8.png", g.MSPA_FOLDED, legend=False, width=760)
g.classmap(f4[R2, C2], out / "conn-4.png", g.MSPA_FOLDED, legend=False, width=760)
s4 = g.mspa_shares(M["c4"])

# ------------------------------------------------------------------------------------------------ internal features
fn = g.mspa_fold(M["noint"])
g.classmap(f1[R, C], out / "internal-on.png", g.MSPA_FOLDED, legend=False, width=600)
g.classmap(fn[R, C], out / "internal-off.png", g.MSPA_FOLDED, legend=False, width=600)

summary = {
    "clc_shares": clc_sh,
    "clc_forest_pct": round(float((cf == 2).mean() * 100), 2),
    "forest_pct": round(float((fo == 2).sum() / (fo > 0).sum() * 100), 2),
    "codes_e1": codes,
    "shares_e1": g.mspa_shares(m1),
    "shares_c4": s4,
    "conn_window": [r0, c0],
    "conn_diff_cells": int(diff.sum()),
    "openings": {"core_opening": codes.get(100, 0), "border_opening": codes.get(220, 0)},
}
(work / "summary.json").write_text(json.dumps(summary, indent=1))
import yaml  # noqa: E402

(work / "slider.yaml").write_text(yaml.safe_dump({"frames": frames}, sort_keys=False, allow_unicode=True))
print(json.dumps(summary, indent=1))
print(srows)
