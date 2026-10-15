"""Figures of the tutorial "Loops" (building-models/loops): the template "A city that grows (a loop)", run as it
is (every step drawn, the city's area per step) and with its rule changed (the neighbours needed, how fast the
bar of suitability falls, nbcount in place of nbsum)."""

import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib.patches import Rectangle  # noqa: E402

NAME = "building-models/loops"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)

base = figs.template("a-city-that-grows-a-loop")
# the template's rule: three city neighbours and a bar from 0.6 (a bar from 0.8 would be above the suitability of all
# the land around the old town, 0.5 to 0.6, and the city would not grow)
RULE = "if c == 0 and nbsum(c == 1, 1) >= 3 and suit > 0.6 - step * 0.008 then 1 else c"
assert RULE in base
template = base
# the tutorial's rule written with nbcount, which counts the cells *with a value* (0 or 1 alike), not the city's
NBCOUNT = base.replace(RULE, RULE.replace("nbsum", "nbcount"))

writes = {"growth.maps": "maps.tif", "relief.map": "relief.tif", "suitability.map": "suitability.tif",
          "growth.city": "city.tif", "town.map": "town.tif"}
figs.run(figs.add_writes(base, writes), work)

relief = figs.read(work / "relief.tif")[0].filled(np.nan)
suit = figs.read(work / "suitability.tif")[0]
WATER, MOUNTAIN, LAND, CITY, NEW = "#a9c8de", "#b9ad9c", "#eef0e6", "#b4508a", "#e8901f"


def city_rgb(city, previous=None):
    """The city over the ground: water, mountains, land; the cells new at this step in orange."""
    img = np.zeros(city.shape + (3,))
    from matplotlib.colors import to_rgb

    img[:] = to_rgb(LAND)
    img[relief < 0.3] = to_rgb(WATER)
    img[relief > 0.75] = to_rgb(MOUNTAIN)
    c = np.asarray(city.filled(0)) == 1
    img[c] = to_rgb(CITY)
    if previous is not None:
        img[c & ~previous] = to_rgb(NEW)
    return img


CROP = (slice(35, 115), slice(35, 115))  # the old town and the land around it (80 x 80 of the 150 x 150 cells)


def draw(img, png, title=None, legend=True, width=640, crop=True):
    if crop:
        img = img[CROP]
    fig = plt.figure(figsize=(width / 150, width / 150 * (0.92 if legend else 1.0)), dpi=150)
    ax = fig.add_axes([0, 0.08 if legend else 0, 1, 0.84 if title else (0.92 if legend else 1)])
    ax.imshow(img, interpolation="nearest")
    ax.axis("off")
    if title:
        fig.text(0.01, 0.985, title, ha="left", va="top", fontsize=11, fontweight="bold", color=figs.INK)
    if legend:
        from matplotlib.patches import Patch

        hs = [Patch(facecolor=c, label=l) for c, l in ((WATER, "Water"), (MOUNTAIN, "Mountains"), (LAND, "Land"),
                                                      (CITY, "City"), (NEW, "New this step"))]
        fig.legend(handles=hs, loc="lower center", ncol=5, fontsize=8.5, handlelength=1, columnspacing=1.0,
                   borderaxespad=0.2)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


n = figs.bands(work / "maps.tif")
rows = []
prev = np.asarray(figs.read(work / "town.tif")[0].filled(0)) == 1
print("the old town:", int(prev.sum()), "cells")
draw(city_rgb(figs.read(work / "town.tif")[0]), out / "step_0.png", legend=True)
rows.append({"step": 0, "city": int(prev.sum()), "new": 0})
for s in range(1, n + 1):
    city = figs.read(work / "maps.tif", s)[0]
    c = np.asarray(city.filled(0)) == 1
    draw(city_rgb(city, prev), out / f"step_{s}.png", legend=True)
    rows.append({"step": s, "city": int(c.sum()), "new": int(c.sum() - prev.sum())})
    prev = c
figs.write_csv(data / "city.csv", rows)
print(rows[:3], rows[-1])
start = figs.read(work / "maps.tif", 1)[0]

figs.raster(work / "suitability.tif", out / "suitability.png", cmap="viridis", unit="suitability", width=640,
            title="Suitability (no value: water and mountains)")
figs.raster(work / "relief.tif", out / "relief.png", cmap="terrain", width=640, title="Relief, 0 to 1")

# ------------------------------------------------------------------------------------------------ the rule changed
variants = {}
for k in (1, 2, 3, 4):
    m = base.replace(RULE, RULE.replace(">= 3", f">= {k}"))
    assert m != base or k == 3
    w = figs.workdir(f"{NAME}/neighbours_{k}")
    figs.run(figs.add_writes(m, {"growth.city": "city.tif"}), w)
    city = figs.read(w / "city.tif")[0]
    draw(city_rgb(city), out / f"neighbours_{k}.png", legend=False, width=520)
    variants[f"neighbours {k}"] = int((city == 1).sum())
w = figs.workdir(f"{NAME}/nbcount")
figs.run(figs.add_writes(NBCOUNT, {"growth.city": "city.tif"}), w)
city = figs.read(w / "city.tif")[0]
draw(city_rgb(city), out / "nbcount.png", legend=False, width=520)
variants["nbcount"] = int((city == 1).sum())
draw(city_rgb(figs.read(work / "city.tif")[0]), out / "nbsum.png", legend=False, width=520)
w = figs.workdir(f"{NAME}/template")
figs.run(figs.add_writes(template, {"growth.city": "city.tif", "suitability.map": "suitability.tif",
                                    "town.map": "town.tif"}), w)
city = figs.read(w / "city.tif")[0]
draw(city_rgb(city), out / "template.png", legend=False, width=520)
variants["template as it comes"] = int((city == 1).sum())
# the suitability next to the old town (the cells within 2 of it)
from scipy import ndimage  # noqa: E402
town = np.asarray(figs.read(w / "town.tif")[0].filled(0)) == 1
ring = ndimage.binary_dilation(town, iterations=2) & ~town
sv = np.asarray(suit.filled(np.nan))[ring]
sv = sv[~np.isnan(sv)]
print("suitability within 2 cells of the town: min %.3f median %.3f max %.3f; land max %.3f" % (
    sv.min(), np.median(sv), sv.max(), np.nanmax(np.asarray(suit.filled(np.nan)))))
rates = []
for rate in ("0.004", "0.006", "0.008", "0.010", "0.012"):
    m = base.replace(RULE, RULE.replace("step * 0.008", f"step * {rate}"))
    w = figs.workdir(f"{NAME}/rate_{rate}")
    figs.run(figs.add_writes(m, {"growth.city": "city.tif"}), w)
    city = figs.read(w / "city.tif")[0]
    draw(city_rgb(city), out / f"rate_{rate}.png", legend=False, width=520)
    variants[f"rate {rate}"] = int((city == 1).sum())
    rates.append({"rate": rate, "city": int((city == 1).sum())})
figs.write_csv(data / "rates.csv", rates)
print(variants)

# ------------------------------------------------------------------------------------------------ the loop drawn
# how carry and collect pass values from step to step
fig, ax = plt.subplots(figsize=(9.5, 2.9), dpi=150)
ax.set_xlim(0, 10.6)
ax.set_ylim(0, 3.2)
ax.axis("off")
ax.grid(False)
xs = [0.3, 3.2, 6.1]
for i, x in enumerate(xs):
    ax.add_patch(Rectangle((x, 1.0), 2.3, 1.5, facecolor="#f4f6f3", edgecolor=figs.RULE, lw=1.2))
    ax.text(x + 1.15, 2.25, f"step {i + 1}", ha="center", fontsize=10.5, fontweight="bold", color=figs.INK)
    ax.text(x + 1.15, 1.75, "@step = %d" % (i + 1), ha="center", fontsize=9, family="monospace", color=figs.INK2)
    ax.text(x + 1.15, 1.35, "grow: @city -> grow.map", ha="center", fontsize=8.3, family="monospace",
            color=figs.INK2)
    ax.annotate("", xy=(x + 1.15, 0.45), xytext=(x + 1.15, 1.0),
                arrowprops=dict(arrowstyle="->", color=figs.FOREST, lw=1.4))
    ax.text(x + 1.15, 0.2, f"maps[{i + 1}]", ha="center", fontsize=9, family="monospace", color=figs.FOREST)
    if i < 2:
        ax.annotate("", xy=(xs[i + 1], 1.75), xytext=(x + 2.3, 1.75),
                    arrowprops=dict(arrowstyle="->", color=figs.EMBER, lw=1.6))
        ax.text(x + 2.45, 1.95, "carry", fontsize=8.5, color=figs.EMBER)
ax.annotate("", xy=(0.3, 1.75), xytext=(-0.4, 1.75), arrowprops=dict(arrowstyle="->", color=figs.EMBER, lw=1.6))
ax.text(-0.45, 2.0, "town.map", fontsize=8.5, family="monospace", color=figs.EMBER)
ax.text(8.7, 1.75, "...  step 25", fontsize=10, color=figs.INK2, va="center")
ax.text(8.7, 1.3, "growth.city: the last", fontsize=8.5, family="monospace", color=figs.EMBER, va="center")
ax.text(8.7, 0.2, "growth.maps: all 25", fontsize=8.5, family="monospace", color=figs.FOREST, va="center")
ax.set_xlim(-0.6, 10.9)
fig.tight_layout(pad=0.2)
fig.savefig(out / "loop_diagram.png", dpi=150)
plt.close(fig)
figs._shrink(out / "loop_diagram.png")
print("done")
