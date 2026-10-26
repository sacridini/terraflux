"""Figures of 'From patterns to networks and change' (habitat-patterns/networks-and-change).

GuidosToolbox's sample forest: MSPA's network (GTN components and importance), a second date made by clearing
part of the forest (simple and morphological change, MSPA and the network at both dates), and MSPA by tiles.

    python website/figures/habitat-patterns/networks-and-change.py
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import gtbfig as g  # noqa: E402
from gtbfig import figs, np  # noqa: E402

NAME = "habitat-patterns/networks-and-change"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)
g.copy_data(g.GTB_INPUT, work)

CLEAR = ("if f == 2 and p > 0.8 then 1 "
         "else if f == 2 and p > 0.55 and nbsum(f == 1, 3) >= 1 then 1 "
         "else if f == 1 and p < 0.15 and nbsum(f == 2, 1) >= 4 then 2 else f")
nodes = [
    g.node("before", "raster.read", path="input.tif"),
    g.node("pressure", "nlm.gaussian_field", rows=1000, columns=1000, range=60, seed=28),
    g.node("after", "raster.calc", {"f": "before.map", "p": "pressure.map"}, expression=CLEAR),
    g.node("mspa_a", "landscape.mspa", {"map": "before.map"}, edge_width=2),
    g.node("mspa_b", "landscape.mspa", {"map": "after.map"}, edge_width=2),
    g.node("comp_a", "networks.gtn_components", {"map": "mspa_a.map"}),
    g.node("comp_b", "networks.gtn_components", {"map": "mspa_b.map"}),
    g.node("imp_a", "networks.gtn_importance", {"map": "mspa_a.map"}),
    g.node("conefor", "networks.conefor", {"map": "mspa_a.map"}),
    g.node("simple", "change.simple", {"before": "before.map", "after": "after.map"}),
    g.node("morph", "change.morphological", {"before": "before.map", "after": "after.map"}),
    g.node("whole", "landscape.mspa", {"map": "before.map"}, edge_width=2),
]
for ov in (20, 60):
    # each its own path: two tiled MSPAs of the same input would otherwise write the same file at once
    nodes.append(g.node(f"tiled{ov}", "landscape.mspa_tiled", {"map": "before.raster"}, edge_width=2, tile_size=300,
                        overlap=ov, path=f"tiled{ov}.tif"))
writes = {"after.map": "after.tif", "pressure.map": "pressure.tif", "mspa_a.map": "mspa_a.tif",
          "mspa_b.map": "mspa_b.tif", "comp_a.map": "comp_a.tif", "comp_a.components": "comp_a_ids.tif",
          "comp_b.map": "comp_b.tif", "imp_a.map": "imp_a.tif", "imp_a.importance": "imp_a_dpc.tif",
          "simple.map": "simple.tif", "morph.map": "morph.tif", "whole.map": "whole.tif"}
outputs = {"comp_a.summary": "comp_a_summary", "comp_b.summary": "comp_b_summary", "comp_a.stats": "comp_a_stats",
           "imp_a.links": "imp_links", "imp_a.nodes": "imp_nodes", "conefor.nodes": "cf_nodes",
           "conefor.links": "cf_links", "simple.stats": "simple_stats", "morph.stats": "morph_stats"}
model = figs.add_writes(g.toml_nodes(*nodes), writes)
for ref, name in outputs.items():
    model += f'\n[outputs.{name}]\nfrom = "{ref}"\n'
figs.run(model, work)
o = work / "output"
R = (slice(650, 950), slice(250, 550))

before = g.read(work / "input.tif")
after = g.read(work / "after.tif")

# ------------------------------------------------------------------------------------------------ the network
ma = g.read(work / "mspa_a.tif")
g.classmap(g.mspa_fold(ma), out / "mspa-a.png", g.MSPA_FOLDED, order=g.MSPA_ORDER[:-1], width=900)
ca = g.read(work / "comp_a.tif")
ids = g.read(work / "comp_a_ids.tif")
stats = figs.read_csv(o / "comp_a_stats.csv")
# networks (components with a bridge) coloured by component, isolated cores apart, the rest of the forest grey
rng = np.random.default_rng(5)
palette_cols = ["#1b9e77", "#d95f02", "#7570b3", "#e7298a", "#66a61e", "#e6ab02", "#a6761d", "#1f78b4",
                "#b2df8a", "#fb9a99", "#cab2d6", "#ff7f00"]
rgb = np.ones(ids.shape + (3,)) * 0.86
fgm = np.isin(before, [2])
rgb[fgm] = (0.72, 0.80, 0.74)
from matplotlib import colors as mcolors  # noqa: E402

big = sorted(((int(r["total"]), int(r["component"]), int(r["has_bridge"])) for r in stats), reverse=True)
colour_of = {}
k = 0
for tot, comp, hb in big:
    if hb:
        colour_of[comp] = mcolors.to_rgb(palette_cols[k % len(palette_cols)])
        k += 1
for comp, col in colour_of.items():
    rgb[ids == comp] = col
iso = (ids > 0) & ~np.isin(ids, list(colour_of))
rgb[iso] = (0.35, 0.35, 0.35)
rgb[before == 0] = (1, 1, 1)
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402


def rgbmap(rgb, png, handles, width=1000, frac=0.70):
    h, w = rgb.shape[:2]
    fig = plt.figure(figsize=(width / 150, width / 150 * frac * h / w + 0.05), dpi=150)
    ax = fig.add_axes([0.005, 0.01, frac - 0.01, 0.98])
    ax.set_axis_off()
    ax.imshow(rgb, interpolation="nearest")
    if handles:
        fig.legend(handles=handles, loc="center left", bbox_to_anchor=(frac + 0.01, 0.5), fontsize=9,
                   handlelength=1.1, labelspacing=0.5)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


rgbmap(rgb, out / "components.png",
       [Patch(facecolor=palette_cols[0], label="Networks: cores joined"),
        Patch(facecolor=palette_cols[1], label="by bridges, a colour each"),
        Patch(facecolor=(0.35, 0.35, 0.35), label="Isolated core"),
        Patch(facecolor=(0.72, 0.80, 0.74), label="Other forest"),
        Patch(facecolor=(0.86, 0.86, 0.86), label="Background")])

ia = g.read(work / "imp_a.tif")
# GTB's classes are relative to the most important: within 1, 5 and 10 % of its dPC
IMP = {192: ("Link: within 1%", "#aa0000"), 194: ("Link: within 5%", "#ff6565"), 195: ("Link: within 10%", "#ffbfbf"),
       208: ("Node: within 1%", "#006500"), 210: ("Node: within 5%", "#00c800"), 211: ("Node: within 10%", "#aaffaa"),
       103: ("Other bridge", "#000000"), 176: ("Other core", "#bfbfff"), 0: ("Background", "#dcdcdc"),
       129: ("Missing", "#ffffff")}
print("imp codes", sorted(int(v) for v in np.unique(ia)))
g.classmap(ia, out / "importance.png", IMP, order=[192, 194, 195, 208, 210, 211, 103, 176, 0], width=1000)
# a zoom on the south, where the most important links and stepping stones are
RZ = (slice(680, 1000), slice(200, 640))
g.classmap(ia[RZ], out / "importance-zoom.png", IMP, order=[192, 194, 195, 208, 210, 211, 103, 176, 0], width=1000)
links = figs.read_csv(o / "imp_links.csv")
nodes_t = figs.read_csv(o / "imp_nodes.csv")
links_sorted = sorted(links, key=lambda r: -float(r["dpc"]))
nodes_sorted = sorted(nodes_t, key=lambda r: -float(r["dpc"]))
print("links cols", list(links[0].keys()), "nodes cols", list(nodes_t[0].keys()))
figs.write_csv(data / "top-links.csv", links_sorted[:15])
figs.write_csv(data / "top-nodes.csv", nodes_sorted[:15])

sa = figs.read_csv(o / "comp_a_summary.csv")[0]
sb = figs.read_csv(o / "comp_b_summary.csv")[0]

# ------------------------------------------------------------------------------------------------ change
FOREST = {2: ("Forest", figs.FOREST), 1: ("Background", "#e9e4d8"), 0: ("Missing", "#ffffff")}
g.classmap(before[R], out / "before.png", FOREST, legend=False, width=700)
g.classmap(after[R], out / "after.png", FOREST, legend=False, width=700)
pr = g.read(work / "pressure.tif")
g.ramp(pr, out / "pressure.png", cmap="magma_r", width=640, unit="clearing pressure (0-1)")
CH = {11: ("Stable background", "#dcdcdc"), 12: ("Gain", "#00d300"), 21: ("Loss", "#ff0000"),
      22: ("Stable forest", "#7d7d7d"), 176: ("Spurious change", "#bfbfff"), 255: ("Missing", "#ffffff")}
sc = g.read(work / "simple.tif")
mc = g.read(work / "morph.tif")
print("morph codes", sorted(int(v) for v in np.unique(mc)))
g.classmap(sc, out / "simple.png", CH, order=[22, 12, 21, 11], width=900, counts=True, legend_width=0.36)
g.classmap(mc[R], out / "morph-zoom.png", CH, order=[22, 12, 21, 176, 11], width=900)
g.classmap(sc[R], out / "simple-zoom.png", CH, order=[22, 12, 21, 11], width=900)
simple_stats = figs.read_csv(o / "simple_stats.csv")
morph_stats = figs.read_csv(o / "morph_stats.csv")

mb = g.read(work / "mspa_b.tif")
g.classmap(g.mspa_fold(ma)[R], out / "mspa-a-zoom.png", g.MSPA_FOLDED, legend=False, width=700)
g.classmap(g.mspa_fold(mb)[R], out / "mspa-b-zoom.png", g.MSPA_FOLDED, legend=False, width=700)
# where the forest of date A went, by its MSPA class (rows) and the class at date B (columns)
fa, fb = g.mspa_fold(ma), g.mspa_fold(mb)
CLS = [17, 9, 5, 3, 65, 33, 1]
names = {k: g.MSPA_FOLDED[k][0] for k in CLS}
trans = []
for a in CLS:
    m = fa == a
    n = int(m.sum())
    row = {"from": names[a], "cells": n}
    for b in CLS:
        row[names[b]] = round(float((m & (fb == b)).sum()) / n * 100, 1)
    row["Lost"] = round(float((m & ~np.isin(fb, CLS)).sum()) / n * 100, 1)
    trans.append(row)
figs.write_csv(data / "mspa-transitions.csv", trans)
sh_a, sh_b = g.mspa_shares(ma), g.mspa_shares(mb)
figs.write_csv(data / "mspa-ab.csv", [{"class": k, "A": sh_a[k], "B": sh_b[k]} for k in sh_a])
fga, fgb = int((before == 2).sum()), int((after == 2).sum())

# ------------------------------------------------------------------------------------------------ tiles
wh = g.read(work / "whole.tif")
fw = g.mspa_fold(wh)
FG = [17, 9, 5, 3, 65, 33, 1]
tiles = {}
for ov in (20, 60):
    ft = g.mspa_fold(g.read(work / f"tiled{ov}.tif"))
    d = ft != fw
    dfg = d & np.isin(fw, FG)
    dbg = d & ~np.isin(fw, FG)
    tiles[ov] = {"foreground": int(dfg.sum()), "background": int(dbg.sum())}
    base = g._rgba(fw, g.MSPA_FOLDED)[..., :3]
    rgb = base * 0.30 + 0.70
    rgb[dbg] = (1.0, 0.72, 0.30)
    # the foreground's differences drawn 3 cells wide, to be seen at this scale
    big = dfg.copy()
    for dr in (-1, 0, 1):
        for dc in (-1, 0, 1):
            big |= np.roll(np.roll(dfg, dr, 0), dc, 1)
    rgb[big] = (0.80, 0.0, 0.0)
    rgbmap(rgb, out / f"tiles-{ov}.png",
           [Patch(facecolor=(0.8, 0, 0), label="Forest: another\nclass"),
            Patch(facecolor=(1.0, 0.72, 0.3), label="Background: openings\nnot found")], width=900, frac=0.60)

summary = {"comp_a": sa, "comp_b": sb, "n_networks_cols": len(colour_of), "top_links": links_sorted[:5],
           "top_nodes": nodes_sorted[:5], "n_links": len(links), "n_nodes": len(nodes_t),
           "simple": simple_stats, "morph": morph_stats, "shares_a": sh_a, "shares_b": sh_b, "fg_a": fga,
           "fg_b": fgb, "trans": trans, "tiles_diff": tiles, "cf_nodes": len(figs.read_csv(o / "cf_nodes.csv")),
           "cf_links": len(figs.read_csv(o / "cf_links.csv"))}
(work / "summary.json").write_text(json.dumps(summary, indent=1))
print(json.dumps(summary, indent=1))
