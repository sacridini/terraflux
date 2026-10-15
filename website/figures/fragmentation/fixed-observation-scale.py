"""Figures of "Connectivity, contagion and mosaics at a fixed scale" (fragmentation/fixed-observation-scale).

GuidosToolbox's sample forest: FOS by FAD, FED and FAC at 27 cells, FOS-FAC over a range of windows (a slider with
the class shares), contagion with Riitters et al.'s (2000) model of density and contagion, the spatial entropy; the
GraySpatCon sample (% tree cover): FOS of a % cover and GraySpatCon's metrics; CORINE around Hannover: the landscape
mosaic and its heatmap.

    python website/figures/fragmentation/fixed-observation-scale.py
"""

import json


def text_output(o, name):
    """A text output of a run: name.txt (Terraflux 0.80), or the JSON string of older versions."""
    t = o / f"{name}.txt"
    if t.exists():
        return t.read_text(encoding="utf-8")
    return json.loads((o / f"{name}.json").read_text(encoding="utf-8"))
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-patterns"))
import gtbfig as g  # noqa: E402
from gtbfig import figs, np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

NAME = "fragmentation/fixed-observation-scale"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)
for src in (g.GTB_INPUT, g.GSC_INPUT, g.CLC):
    g.copy_data(src, work)
o = work / "output"
summary = {}
R = (slice(650, 950), slice(250, 550))
CLASSES6 = ("intact", "interior", "dominant", "transitional", "patchy", "rare")


def run(nodes, writes, outputs, name):
    model = figs.add_writes(g.toml_nodes(*nodes), writes)
    for ref, nm in outputs.items():
        model += f'\n[outputs.{nm}]\nfrom = "{ref}"\n'
    figs.run(model, work, name=name)


def shares(rows):
    """FOS's stats (a row per class) as {class name: % of the foreground}."""
    out_ = {}
    for r in rows:
        name = [v for k, v in r.items() if k.lower() in ("class", "name", "fos_class")]
        out_[str(name[0]) if name else str(list(r.values())[0])] = r
    return out_


# ------------------------------------------------------------------------------------------------ FOS: FAD, FED, FAC
nodes = [g.node("forest", "raster.read", path="input.tif")]
writes, outputs = {}, {}
for m in ("FAD", "FED", "FAC"):
    nodes.append(g.node(f"fos_{m}", "fragmentation.fos", {"map": "forest.map"}, window=27, method=m, classes=6))
    writes[f"fos_{m}.map"] = f"fos_{m}.tif"
    outputs[f"fos_{m}.stats"] = f"fos_{m}_stats"
    outputs[f"fos_{m}.summary"] = f"fos_{m}_summary"
nodes.append(g.node("fos5", "fragmentation.fos", {"map": "forest.map"}, window=27, method="FAC", classes=5))
outputs["fos5.stats"] = "fos5_stats"
outputs["fos_FAC.report"] = "fos_FAC_report"
nodes.append(g.node("pff", "fragmentation.contagion", {"map": "forest.map"}, window=7))
nodes.append(g.node("pf", "fragmentation.fad", {"map": "forest.map"}, window=7))
nodes.append(g.node("ent", "fragmentation.entropy", {"map": "forest.map"}))
nodes.append(g.node("entmap", "fragmentation.entropy_map", {"map": "forest.map"}, window=27))
writes.update({"pff.map": "pff7.tif", "pf.map": "pf7.tif", "entmap.map": "entropy27.tif"})
outputs.update({"ent.index": "entropy_index", "ent.pairs": "entropy_pairs"})
run(nodes, writes, outputs, "fos.toml")
for m in ("FAD", "FED", "FAC"):
    a = g.read(work / f"fos_{m}.tif").astype(float)
    g.classmap(g.density_classes(a)[R], out / f"fos-{m.lower()}-zoom.png", g.FAD_CLASS, legend=False, width=640)
    st = figs.read_csv(o / f"fos_{m}_stats.csv")
    sm = figs.read_csv(o / f"fos_{m}_summary.csv")
    summary[f"fos_{m}"] = {"stats": st, "summary": sm}
    print(m, st, sm)
summary["fos5"] = figs.read_csv(o / "fos5_stats.csv")
g.classmap(g.density_classes(g.read(work / "fos_FAC.tif").astype(float)), out / "fos-fac.png", g.FAD_CLASS,
           order=g.FAD_ORDER[:-1], width=900, legend_width=0.34)
# text outputs are written as JSON strings: GuidosToolbox's report as a plain text file
(data / "fos-report.txt").write_text(text_output(o, "fos_FAC_report"),
                                     encoding="utf-8")
summary["entropy"] = json.loads((o / "entropy_index.json").read_text())
print("entropy", summary["entropy"])

# Riitters et al. (2000): density (Pf) and contagion (Pff) of every forest cell in windows of 7
pf = g.read(work / "pf7.tif").astype(float)
pff = g.read(work / "pff7.tif").astype(float)
fg = (pf <= 100) & (pff <= 100)
x, y = pf[fg] / 100, pff[fg] / 100
cat = np.full(pf.shape, 0, dtype=np.int32)          # 0 not forest
P, Q = pf / 100, pff / 100
cat[fg & (P < 0.4)] = 5                              # patch
cat[fg & (P >= 0.4) & (P < 0.6)] = 4                 # transitional
cat[fg & (P >= 0.6) & (P - Q > 0)] = 3               # edge
cat[fg & (P >= 0.6) & (P - Q < 0)] = 2               # perforated
cat[fg & (P >= 0.6) & (P == Q)] = 6                  # undetermined
cat[fg & (P == 1)] = 1                               # interior
RIIT = {1: ("Interior", "#006400"), 2: ("Perforated", "#4fb34f"), 3: ("Edge", "#1f3d99"), 4: ("Transitional", "#f2b705"),
        5: ("Patch", "#c0392b"), 6: ("Undetermined", "#9a8c98"), 0: ("Not forest", "#e9e4d8")}
g.classmap(cat[R], out / "riitters-zoom.png", RIIT, order=[1, 2, 3, 4, 5, 6, 0], width=760, legend_width=0.30)
n = fg.sum()
summary["riitters"] = {RIIT[k][0]: round(float((cat == k).sum()) / n * 100, 2) for k in (1, 2, 3, 4, 5, 6)}
print("riitters", summary["riitters"])
fig, ax = plt.subplots(figsize=(6.4, 5.6), dpi=150)
hb = ax.hexbin(x, y, gridsize=40, bins="log", cmap="Greens", mincnt=1, extent=(0, 1, 0, 1))
ax.plot([0, 1], [0, 1], color=figs.INK, lw=1)
for v in (0.4, 0.6):
    ax.axvline(v, color=figs.INK, lw=1, ls="--")
ax.annotate("interior (Pf = 1)", xy=(1.0, 1.0), xytext=(0.62, 0.1), fontsize=9.5, color=figs.INK,
            arrowprops=dict(arrowstyle="->", color=figs.INK, lw=0.9))
for (tx, ty, t) in ((0.2, 0.92, "patch"), (0.5, 0.92, "transitional"), (0.78, 0.95, "perforated"),
                    (0.86, 0.55, "edge")):
    ax.text(tx, ty, t, ha="center", va="center", fontsize=9.5, color=figs.INK,
            bbox=dict(boxstyle="round,pad=0.2", fc="white", ec="none", alpha=0.85))
ax.set_xlim(0, 1.02)
ax.set_ylim(0, 1.02)
ax.set_xlabel("Pf: forest density in the window (FAD / 100)")
ax.set_ylabel("Pff: forest contagion in the window")
cb = fig.colorbar(hb, ax=ax, fraction=0.046, pad=0.03)
cb.outline.set_visible(False)
cb.set_label("forest cells (log)", color=figs.INK2)
fig.tight_layout()
fig.savefig(out / "pf-pff.png", dpi=150)
plt.close(fig)
figs._shrink(out / "pf-pff.png")
g.ramp(np.where(fg, pff, np.nan)[R], out / "contagion-zoom.png", cmap="YlGn", vmin=0, vmax=100, width=700,
       unit="contagion Pff (%), 7 x 7 cells")
ent = g.read(work / "entropy27.tif").astype(float)
print("entropy map range", np.nanmin(ent), np.nanmax(ent))
g.ramp(np.where(ent <= 100, ent, np.nan), out / "entropy27.png", cmap="magma", vmin=0, vmax=100, width=760,
       unit="spatial entropy (%), 27 x 27 cells")

# ------------------------------------------------------------------------------------------------ FOS over windows
WINDOWS = [5, 9, 15, 27, 45, 81, 135]
nodes = [g.node("forest", "raster.read", path="input.tif")]
outputs = {}
writes = {}
for wdw in WINDOWS:
    nodes.append(g.node(f"w{wdw}", "fragmentation.fos", {"map": "forest.map"}, window=wdw, method="FAC", classes=6))
    writes[f"w{wdw}.map"] = f"fac_{wdw}.tif"
    outputs[f"w{wdw}.stats"] = f"fac_{wdw}_stats"
    outputs[f"w{wdw}.summary"] = f"fac_{wdw}_summary"
run(nodes, writes, outputs, "windows.toml")
rows = []
frames = []
for wdw in WINDOWS:
    a = g.read(work / f"fac_{wdw}.tif").astype(float)
    g.classmap(g.density_classes(a)[R], out / f"fac-{wdw}.png", g.FAD_CLASS, legend=False, width=640)
    st = figs.read_csv(o / f"fac_{wdw}_stats.csv")
    sm = figs.read_csv(o / f"fac_{wdw}_summary.csv")
    frames.append({"window": wdw, "stats": st, "summary": sm})
    fgm = a <= 100
    row = {"window": wdw}
    cl = g.density_classes(a)
    for k, name in zip((6, 5, 4, 3, 2, 1), CLASSES6):
        row[name] = round(float((cl == k).sum()) / float(fgm.sum()) * 100, 2)
    row["average"] = round(float(a[fgm].mean()), 2)
    rows.append(row)
    print(row)
figs.write_csv(data / "fac-windows.csv", rows)
summary["fac_windows"] = rows
summary["fac_frames"] = frames

# ------------------------------------------------------------------------------------------------ a % tree cover
nodes = [g.node("cover", "raster.read", path="gscinput.tif"),
         g.node("fos", "fragmentation.fos", {"map": "cover.map"}, window=7, method="FAD", gray_threshold=30),
         g.node("mean", "fragmentation.gray_convolution", {"map": "cover.map"}, metric="mean", window=7),
         g.node("sd", "fragmentation.gray_convolution", {"map": "cover.map"}, metric="stddev", window=7)]
run(nodes, {"fos.map": "gray_fos.tif", "mean.map": "gray_mean.tif", "sd.map": "gray_sd.tif"},
    {"fos.stats": "gray_fos_stats", "fos.summary": "gray_fos_summary"}, "gray.toml")
gc = g.read(work / "gscinput.tif").astype(float)
g.ramp(np.where(gc <= 100, gc, np.nan), out / "treecover.png", cmap="YlGn", vmin=0, vmax=100, width=820,
       unit="tree cover (%)")
gf = g.read(work / "gray_fos.tif").astype(float)
print("gray fos codes", np.unique(gf)[-6:])
g.classmap(g.density_classes(gf), out / "treecover-fos.png", g.FAD_CLASS, order=g.FAD_ORDER[:-1], width=820,
           legend_width=0.30)
gm = g.read(work / "gray_mean.tif").astype(float)
gs = g.read(work / "gray_sd.tif").astype(float)
miss = gc > 100
g.ramp(np.where(miss, np.nan, gs), out / "treecover-sd.png", cmap="magma", width=820,
       unit="SD of tree cover, 7 x 7 cells (%)")
summary["gray"] = {"stats": figs.read_csv(o / "gray_fos_stats.csv"), "summary": figs.read_csv(o / "gray_fos_summary.csv"),
                   "mean_range": [float(np.nanmin(np.where(miss, np.nan, gm))), float(np.nanmax(np.where(miss, np.nan, gm)))],
                   "sd_range": [float(np.nanmin(np.where(miss, np.nan, gs))), float(np.nanmax(np.where(miss, np.nan, gs)))]}

# ------------------------------------------------------------------------------------------------ the landscape mosaic
nodes = [g.node("corine", "raster.read", path="clc.tif"),
         g.node("mosaic", "fragmentation.landscape_mosaic", {"map": "corine.map"}, window=27,
                agriculture=list(range(12, 23)), natural=list(range(23, 40)), developed=list(range(1, 12)))]
run(nodes, {"mosaic.map": "lm19.tif", "mosaic.triangle": "lm_triangle.tif"},
    {"mosaic.stats": "lm_stats", "mosaic.heatmap": "lm_heatmap"}, "mosaic.toml")
LMC = figs.gtb_colors("LM")
NAMES19 = {0: "Missing", 1: "Agriculture (>60%)", 2: "Developed (>60%)", 3: "Natural (>60%)",
           4: "Agriculture-Developed", 5: "Agriculture-Natural", 6: "Developed-Natural",
           7: "Developed-Agriculture", 8: "Natural-Agriculture", 9: "Natural-Developed",
           10: "Agriculture-Natural-Developed", 11: "Developed-Agriculture-Natural", 12: "Natural-Agriculture-Developed",
           13: "Mosaic Agriculture-Developed", 14: "Mosaic Agriculture-Natural", 15: "Mosaic Developed-Natural",
           16: "Mosaic of the three", 17: "Pure natural", 18: "Pure agriculture", 19: "Pure developed"}
lm = g.read(work / "lm19.tif")
vals, cnt = np.unique(lm, return_counts=True)
present = [int(v) for v, c in sorted(zip(vals, cnt), key=lambda t: -t[1]) if v != 0]
print("lm19", dict(zip(vals.tolist(), cnt.tolist())))
pal = {k: (NAMES19[k], LMC[k]) for k in range(0, 20)}
pal[0] = ("Missing (water)", "#ffffff")
g.classmap(lm, out / "mosaic.png", pal, order=present[:12], width=1000, legend_width=0.36, fontsize=8.5)
tot = cnt[vals != 0].sum()
figs.write_csv(data / "mosaic-shares.csv",
               [{"class": NAMES19[k], "share": round(float(cnt[vals == k][0]) / tot * 100, 2)} for k in present])
# the three-class land cover the mosaic reads
c = g.read(work / "clc.tif")
three = np.zeros(c.shape, dtype=np.int32)
three[(c >= 12) & (c <= 22)] = 1
three[(c >= 23) & (c <= 39)] = 2
three[(c >= 1) & (c <= 11)] = 3
g.classmap(three, out / "mosaic-input.png", {1: ("Agriculture", "#f1e08a"), 2: ("Natural", figs.FOREST),
                                             3: ("Developed", "#c8384a"), 0: ("Water (missing)", "#ffffff")},
           order=[1, 2, 3, 0], width=1000, legend_width=0.36, counts=True)
tri = g.read(work / "lm_triangle.tif").astype(float)
print("triangle", tri.shape, np.nanmin(tri), np.nanmax(tri))
fig, ax = plt.subplots(figsize=(7.2, 4.6), dpi=150)
t = np.ma.masked_invalid(np.where(tri < 0, np.nan, tri))
im = ax.imshow(t, cmap="viridis", norm=mcolors.LogNorm(vmin=0.01, vmax=max(float(t.max()), 0.02)),
               interpolation="nearest")
ax.set_xticks([])
ax.set_yticks([])
ax.grid(False)
for s in ax.spines.values():
    s.set_visible(False)
for (cx, cy, txt, ha) in ((10, -1.0, "pure natural", "center"), (0, 11.9, "pure developed", "left"),
                          (20, 11.9, "pure agriculture", "right")):
    ax.text(cx, cy, txt, ha=ha, va="center", fontsize=9.5, color=figs.INK)
ax.set_ylim(12.5, -1.7)
cb = fig.colorbar(im, ax=ax, fraction=0.04, pad=0.02)
cb.outline.set_visible(False)
cb.set_label("% of the cells (log)", color=figs.INK2)
fig.tight_layout()
fig.savefig(out / "mosaic-triangle.png", dpi=150)
plt.close(fig)
figs._shrink(out / "mosaic-triangle.png")
summary["mosaic_stats_head"] = figs.read_csv(o / "lm_stats.csv")[:5]
(work / "summary.json").write_text(json.dumps(summary, indent=1, default=str))
print(json.dumps({k: v for k, v in summary.items() if k != "fac_frames"}, indent=1, default=str)[:6000])
