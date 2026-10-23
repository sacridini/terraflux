"""Figures of "Fragmentation and habitat loss" (fragmentation/fragmentation-and-habitat-loss).

Four neutral landscapes crossing habitat amount with fragmentation, a toy moving window run through Terraflux,
and GuidosToolbox's forest area density (FAD) of its sample forest at one scale and at five (FAD-MS).

    python website/figures/fragmentation/fragmentation-and-habitat-loss.py
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-patterns"))
import gtbfig as g  # noqa: E402
from gtbfig import figs, np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch, Rectangle  # noqa: E402

NAME = "fragmentation/fragmentation-and-habitat-loss"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)
g.copy_data(g.GTB_INPUT, work)
summary = {}

# ------------------------------------------------------------------------------------------------ amount x fragmentation
nodes = []
CASES = {"60_agg": (0.6, 1.8), "60_frag": (0.6, 0.4), "20_agg": (0.2, 1.8), "20_frag": (0.2, 0.4)}
writes = {}
outputs = {}
for k, (share, dim) in CASES.items():
    nodes.append(g.node(f"s{k}", "nlm.fbm", rows=150, columns=150, resolution=30, dimension=dim, seed=11))
    nodes.append(g.node(f"h{k}", "nlm.binarize", {"map": f"s{k}.map"}, breaks=share))
    nodes.append(g.node(f"f{k}", "fragmentation.fad", {"map": f"h{k}.map"}, window=13))
    nodes.append(g.node(f"c{k}", "metrics.class", {"map": f"h{k}.map"}, metrics=["pland", "np", "lpi", "ed"], show="pland"))
    writes[f"h{k}.map"] = f"case_{k}.tif"
    outputs[f"f{k}.stats"] = f"fad_{k}"
    outputs[f"c{k}.table"] = f"cls_{k}"
model = figs.add_writes(g.toml_nodes(*nodes), writes)
for ref, name in outputs.items():
    model += f'\n[outputs.{name}]\nfrom = "{ref}"\n'
figs.run(model, work, name="cases.toml")
o = work / "output"
fig, axes = plt.subplots(2, 2, figsize=(7.6, 8.2), dpi=150)
cm = mcolors.ListedColormap(["#ece6d6", figs.FOREST])
cases = {}
for ax, k in zip(axes.ravel(), CASES):
    a = g.read(work / f"case_{k}.tif")
    ax.imshow(a, cmap=cm, vmin=1, vmax=2, interpolation="nearest")
    ax.set_xticks([])
    ax.set_yticks([])
    ax.grid(False)
    for s in ax.spines.values():
        s.set_visible(False)
    fs = figs.read_csv(o / f"fad_{k}.csv")[0]
    cl = [r for r in figs.read_csv(o / f"cls_{k}.csv") if float(r["class"]) == 2][0]
    cases[k] = {"fad": fs, "np": int(float(cl["np"])), "lpi": round(float(cl["lpi"]), 1), "ed": round(float(cl["ed"]), 1)}
    share, dim = CASES[k]
    kind = "aggregated" if dim > 1 else "fragmented"
    avg = [v for kk, v in fs.items() if "average" in kk or kk.lower() in ("fad_av", "avg")]
    ax.set_title(f"{share * 100:.0f}% habitat, {kind}", loc="left", fontsize=10.5)
    ax.text(0.01, -0.035, f"{cases[k]['np']} patches, largest {cases[k]['lpi']}% of the map", transform=ax.transAxes,
            va="top", fontsize=9, color=figs.INK2)
fig.text(0.02, 0.985, "", fontsize=1)
fig.tight_layout(h_pad=2.2)
fig.savefig(out / "amount-fragmentation.png", dpi=150)
plt.close(fig)
figs._shrink(out / "amount-fragmentation.png")
summary["cases"] = cases

# ------------------------------------------------------------------------------------------------ a toy moving window
toy = np.ones((15, 15), dtype="uint8")
toy[1:7, 1:9] = 2
toy[8:14, 3:6] = 2
toy[5:12, 9:14] = 2
toy[3, 11] = 2
toy[12, 10:12] = 1
g.write_grid(toy, work / "toy.tif")
model = g.toml_nodes(g.node("toy", "raster.read", path="toy.tif"),
                     g.node("fad", "fragmentation.fad", {"map": "toy.map"}, window=7))
figs.run(figs.add_writes(model, {"fad.map": "toy_fad.tif"}), work, name="toy.toml")
tf = g.read(work / "toy_fad.tif")
cr, cc = 6, 8                       # the window around a forest cell
fig, axes = plt.subplots(1, 2, figsize=(10.0, 4.9), dpi=150)
rgb = np.array([mcolors.to_rgb("#ece6d6" if v == 1 else figs.FOREST) for v in toy.ravel()]).reshape(toy.shape + (3,))
for ax in axes:
    ax.imshow(rgb, interpolation="nearest")
    ax.set_xticks(np.arange(-0.5, 15, 1), minor=True)
    ax.set_yticks(np.arange(-0.5, 15, 1), minor=True)
    ax.grid(which="minor", color="#ffffff", lw=0.6)
    ax.grid(which="major", visible=False)
    ax.tick_params(which="both", length=0, labelbottom=False, labelleft=False)
    for s in ax.spines.values():
        s.set_visible(False)
win = toy[cr - 3:cr + 4, cc - 3:cc + 4]
n_fg = int((win == 2).sum())
axes[0].add_patch(Rectangle((cc - 3.5, cr - 3.5), 7, 7, fill=False, ec=figs.EMBER, lw=2.6))
axes[0].add_patch(Rectangle((cc - 0.5, cr - 0.5), 1, 1, fill=False, ec="#b03a2e", lw=2.2))
axes[0].set_title(f"A window of 7 x 7 cells: {n_fg} of 49 are forest", loc="left", fontsize=10.5)
for r in range(15):
    for c in range(15):
        if toy[r, c] == 2:
            v = int(round(float(tf[r, c])))
            axes[1].text(c, r, str(v), ha="center", va="center", fontsize=6.6, color="white", fontweight="bold")
axes[1].set_title("FAD (%) of every forest cell", loc="left", fontsize=10.5)
axes[1].add_patch(Rectangle((cc - 0.5, cr - 0.5), 1, 1, fill=False, ec="#b03a2e", lw=2.2))
fig.tight_layout()
fig.savefig(out / "toy-window.png", dpi=150)
plt.close(fig)
figs._shrink(out / "toy-window.png")
summary["toy"] = {"forest_in_window": n_fg, "fad_centre": float(tf[cr, cc])}

# ------------------------------------------------------------------------------------------------ FAD and FAD-MS of GTB's sample
nodes = [g.node("forest", "raster.read", path="input.tif"),
         g.node("fad", "fragmentation.fad", {"map": "forest.map"}, window=27),
         g.node("ms", "fragmentation.fad_multiscale", {"map": "forest.map"})]
writes = {"fad.map": "fad27.tif", "fad.classes": "fad27_classes.tif", "ms.map": "ms_avg.tif",
          "ms.classes": "ms_classes.tif", "ms.scales": "ms_scales.tif"}
model = figs.add_writes(g.toml_nodes(*nodes), writes)
model += '\n[outputs.fad_stats]\nfrom = "fad.stats"\n\n[outputs.ms_stats]\nfrom = "ms.stats"\n'
figs.run(model, work, name="fad.toml")
fs = figs.read_csv(o / "fad_stats.csv")
ms = figs.read_csv(o / "ms_stats.csv")
print("fad stats", fs)
print("ms stats", ms)
summary["fad27"] = fs
summary["ms"] = ms
R = (slice(650, 950), slice(250, 550))
forest = g.read(work / "input.tif")
FOREST = {2: ("Forest", figs.FOREST), 1: ("Background", "#e9e4d8"), 0: ("Missing", "#ffffff")}
g.classmap(forest[R], out / "forest-zoom.png", FOREST, order=[2, 1], width=700, legend_width=0.26)
d27 = g.read(work / "fad27.tif").astype(float)
ZOOM_LEGEND = 0.34   # the zooms' legend width: both sides of the slider put the map at the same pixels


def density_zoom(d, png, width=700, legend_width=ZOOM_LEGEND):
    """The density (0-100 on the forest) drawn as g.classmap draws its classes, same figure and map axes, so that it
    lines up with the classes under a slider: a colour bar in the legend's place, the background in the classes'
    grey (YlGn's 0% is near white), missing cells white."""
    h, w = d.shape
    dpi = 150
    W = width / dpi
    map_frac = 1 - legend_width
    H = W * map_frac * h / w + 0.04
    fig = plt.figure(figsize=(W, H), dpi=dpi)
    top = 1 - 0.02 / H
    ax = fig.add_axes([0.005, 0.02 / H, map_frac - 0.01, top - 0.02 / H])
    ax.set_axis_off()
    cmap = plt.get_cmap("YlGn").copy()
    rgba = cmap(mcolors.Normalize(0, 100)(np.clip(d, 0, 100)))
    rgba[d == 101] = mcolors.to_rgba(g.FAD_CLASS[101][1])
    rgba[d > 101] = (1, 1, 1, 1)
    ax.imshow(rgba, interpolation="nearest")
    cax = fig.add_axes([map_frac + 0.03, 0.26, 0.03, 0.66])
    cb = fig.colorbar(plt.cm.ScalarMappable(mcolors.Normalize(0, 100), cmap), cax=cax)
    cb.outline.set_visible(False)
    cb.ax.grid(False)
    cb.ax.tick_params(labelsize=9)
    cb.set_label("forest within 27 x 27 cells (%)", fontsize=9.5, color=figs.INK2)
    fig.legend(handles=[Patch(facecolor=g.FAD_CLASS[101][1], edgecolor="#00000033", label="Background")],
               loc="center left", bbox_to_anchor=(map_frac + 0.01, 0.12), fontsize=9.5, handlelength=1.1,
               handleheight=1.1, borderaxespad=0, frameon=False)
    fig.savefig(png, dpi=dpi)
    plt.close(fig)
    figs._shrink(png)
    return png


density_zoom(d27[R], out / "fad27-density-zoom.png")
c27 = g.read(work / "fad27_classes.tif")
print("fad27 class codes", np.unique(c27))
g.classmap(g.density_classes(d27), out / "fad27-classes.png", g.FAD_CLASS, order=g.FAD_ORDER[:-1], width=900,
           legend_width=0.34)
g.classmap(g.density_classes(d27)[R], out / "fad27-classes-zoom.png", g.FAD_CLASS, order=g.FAD_ORDER[:-1], width=700,
           legend_width=ZOOM_LEGEND)
# every scale of FAD-MS as classes, the same zoom, a frame each (same size)
scales = g.read_all(work / "ms_scales.tif")
WINDOWS = [7, 13, 27, 81, 243]
for i, wdw in enumerate(WINDOWS):
    g.classmap(g.density_classes(scales[i].astype(float)), out / f"ms-{wdw}.png", g.FAD_CLASS, legend=False, width=640)
avg = g.read(work / "ms_avg.tif").astype(float)
g.classmap(g.density_classes(avg, five=True), out / "ms-average.png", g.FAD_CLASS, order=[5, 4, 3, 2, 1, 101],
           width=900, legend_width=0.34)
g.classmap(g.density_classes(avg, five=True), out / "ms-avg.png", g.FAD_CLASS, legend=False, width=640)
# the class shares per scale (a stacked bar chart's data): the stats' columns, whatever their names
rows = []
for r in ms:
    row = {"window": "average" if r["window"] == "0" else f"{r['window']} cells"}
    for k in ("intact", "interior", "dominant", "transitional", "patchy", "rare"):
        row[k] = round(float(r[f"{k}_percent"]), 2)
    rows.append(row)
figs.write_csv(data / "ms-shares.csv", rows)
(work / "summary.json").write_text(json.dumps(summary, indent=1, default=str))
print(json.dumps(summary, indent=1, default=str))
