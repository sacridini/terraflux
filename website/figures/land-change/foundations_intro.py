"""Figures of "What land change models are" (land-change/what-land-change-models-are): Sibuyan's land use in 1997
and 2011, what changed, its transition matrix and gains and losses (run by Terraflux), and two drawings: the
anatomy of a land change model and its calibration and validation periods."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np, plt  # noqa: E402
from matplotlib.patches import FancyArrowPatch, FancyBboxPatch  # noqa: E402

SLUG = "land-change/what-land-change-models-are"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = dc.sibuyan("intro")

MODEL = """format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "sibuyan_landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "sibuyan_landuse.tif"
band = "2011"

[[nodes]]
id = "series"
op = "stack.of"
inputs = { y1997 = "$t0", y2011 = "$t1" }

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "$t0", to = "$t1" }

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "$t0", to = "$t1" }

[[nodes]]
id = "suggest"
op = "change.suggest"
inputs = { maps = "series.stack" }

[[nodes]]
id = "changed"
op = "map.calc"
inputs = { a = "$t0", b = "$t1" }
params = { expression = "if a != b then b else 0" }

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "changed.map" }
params = { path = "changed.tif" }

[outputs.crosstab]
from = "crosstab.table"

[outputs.components]
from = "components.table"

[outputs.suggest]
from = "suggest.table"
"""
res = dc.run(MODEL, work)
dc.downloads(SLUG)

t0, t1 = dc.landuse()
dc.draw_map(t0, out / "sibuyan_1997.png", title="Sibuyan, 1997", counts=True, width=640)
dc.draw_map(t1, out / "sibuyan_2011.png", title="Sibuyan, 2011", counts=True, width=640)
dc.draw_change(t0, t1, out / "change_1997_2011.png", title="What changed, 1997 to 2011", width=760)

# the transition matrix, in cells (and its rows in shares)
ct = dc.table(res / "crosstab.csv")
print("crosstab (cells):")
for r in ct:
    print("  ", r)
comp = dc.table(res / "components.csv")
print("components:")
for r in comp:
    print("  ", r)
sug = dc.table(res / "suggest.csv")
print("suggest:")
for r in sug:
    print("  ", r)

# gains and losses of each class, % of the island (a chart)
cells = np.count_nonzero(~np.isnan(t0))
rows = []
for c, (name, _) in dc.SIB.items():
    a, b = np.sum(t0 == c), np.sum(t1 == c)
    gain = np.sum((t1 == c) & (t0 != c))
    loss = np.sum((t0 == c) & (t1 != c))
    rows.append({"class": name, "gain": round(gain / cells * 100, 2), "loss": round(-loss / cells * 100, 2),
                 "net": round((b - a) / cells * 100, 2)})
figs.write_csv(data / "gains_losses.csv", rows)
print(rows)

# ---- drawing: the anatomy of a model
fig, ax = plt.subplots(figsize=(9.6, 4.3), dpi=150)
ax.set_xlim(0, 100)
ax.set_ylim(-7, 46)
ax.set_axis_off()


def box(x, y, w, h, title, text, color):
    ax.add_patch(FancyBboxPatch((x, y), w, h, boxstyle="round,pad=0.4,rounding_size=1.6", facecolor=color + "22",
                                edgecolor=color, linewidth=1.4))
    ax.text(x + w / 2, y + h - 2.6, title, ha="center", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    ax.text(x + w / 2, y + h - 8.2, text, ha="center", va="top", fontsize=9.2, color=figs.INK2, linespacing=1.35)


def arrow(x0, y0, x1, y1, rad=0.0, color=figs.INK2):
    ax.add_patch(FancyArrowPatch((x0, y0), (x1, y1), arrowstyle="-|>", mutation_scale=14, linewidth=1.3,
                                 color=color, connectionstyle=f"arc3,rad={rad}"))


box(1, 27, 22, 16, "Map at time t", "the land use of\nevery cell", figs.INK2)
box(29, 27, 26, 16, "How much", "the quantity of each\ntransition per step\n(Markov chain, a demand)", figs.WATER)
box(29, 4, 26, 18, "Where", "the probability of each\ntransition in each cell\n(weights of evidence,\nregressions...) from drivers", figs.FOREST)
box(61, 15, 21, 16, "Allocation", "the changes placed\non the likely cells,\nin patches", figs.EMBER)
box(86, 15, 13, 16, "Map at\nt + 1", "", figs.INK2)
arrow(23.5, 35, 28.5, 35)
arrow(15, 26.5, 28.5, 13)
arrow(55.5, 35, 60.5, 27)
arrow(55.5, 13, 60.5, 19)
arrow(82.5, 23, 85.5, 23)
ax.plot([92.5, 92.5, 6, 6], [14.2, -2.5, -2.5, 20], color=figs.EMBER, linewidth=1.3)
arrow(6, 19, 6, 26.4, color=figs.EMBER)
ax.text(49, -5.6, "the new map becomes the start of the next step (a loop)", ha="center", fontsize=9, color=figs.EMBER,
        style="italic")
fig.savefig(out / "anatomy.png", dpi=150, bbox_inches="tight", pad_inches=0.1)
plt.close(fig)
figs._shrink(out / "anatomy.png")

# ---- drawing: calibration and validation periods
fig, ax = plt.subplots(figsize=(7.4, 2.5), dpi=150)
ax.set_xlim(1982, 2033)
ax.set_ylim(0.2, 7.6)
ax.set_axis_off()
ax.plot([1984, 2031], [5, 5], color=figs.INK2, linewidth=1.2)
for yr, lab in [(1985, "map 1"), (2000, "map 2"), (2015, "map 3")]:
    ax.plot([yr], [5], "o", color=figs.INK, markersize=7)
    ax.text(yr, 6.0, f"{yr}\n{lab}", ha="center", va="bottom", fontsize=9.5, color=figs.INK)
ax.annotate("", xy=(2000, 3.2), xytext=(1985, 3.2), arrowprops=dict(arrowstyle="<->", color=figs.WATER, lw=1.6))
ax.text(1992.5, 2.2, "calibration: fit the rates,\nthe weights, the patches", ha="center", va="top", fontsize=9.2,
        color=figs.WATER)
ax.annotate("", xy=(2015, 3.2), xytext=(2000, 3.2), arrowprops=dict(arrowstyle="<->", color=figs.FOREST, lw=1.6))
ax.text(2007.5, 2.2, "validation: simulate from map 2,\ncompare with map 3", ha="center", va="top", fontsize=9.2,
        color=figs.FOREST)
ax.annotate("", xy=(2030, 3.2), xytext=(2015, 3.2), arrowprops=dict(arrowstyle="->", color=figs.EMBER, lw=1.6,
                                                                   linestyle="--"))
ax.text(2022.5, 2.2, "projection: scenarios\nfrom map 3", ha="center", va="top", fontsize=9.2, color=figs.EMBER)
fig.savefig(out / "periods.png", dpi=150, bbox_inches="tight", pad_inches=0.08)
plt.close(fig)
figs._shrink(out / "periods.png")
print("done")
