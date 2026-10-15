"""Figures of "A cellular automaton by hand" (land-change/a-cellular-automaton-by-hand): the template "A city that
grows (a loop)" run by Terraflux, its city step by step, the share of the land that is city, the suitability, the
same automaton with other neighbourhood rules, and a drawing of the rule on a toy grid."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import dinamica_common as dc  # noqa: E402
from dinamica_common import figs, np, plt  # noqa: E402

SLUG = "land-change/a-cellular-automaton-by-hand"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = figs.workdir("land-change-a/ca")

# the template, with a bar that starts at 0.6 and falls by 0.004 a step (the template's starts at 0.8, above the
# suitability of the land around the old town, so as it comes its city does not grow) and 40 steps (the city grows
# at most one cell per step)
template = figs.template("a-city-that-grows-a-loop")
base = (template.replace("nbsum(c == 1, 1) >= 3 and suit > 0.6 - step * 0.008",
                         "nbsum(c == 1, 1) >= 2 and suit > 0.6 - step * 0.004")
        .replace("times = 25", "times = 40"))
assert base.count("nbsum") == 1 and "times = 40" in base
print(base)
model = figs.add_writes(base, {"growth.maps": "city.tif", "suitability.map": "suit.tif", "relief.map": "relief.tif"})
figs.run(model, work)

relief = figs.read(work / "relief.tif")[0].filled(np.nan)
CITY = {0: ("Open land", "#e9e4d3"), 1: ("City", "#b4508a"), 2: ("Water", "#3f7fae"), 3: ("Mountains", "#8d8a83")}


def composite(city):
    c = np.where(np.isnan(city), np.where(relief < 0.3, 2, 3), city)
    return c


n = figs.bands(work / "city.tif")
land = np.count_nonzero((relief >= 0.3) & (relief <= 0.75))
rows, frames = [], []
for b in range(2, n + 1, 2):
    city = figs.read(work / "city.tif", b)[0].filled(np.nan)
    figs.raster(None, out / f"step_{b}.png", array=composite(city), palette=CITY, legend=False, width=560)
    share = float(np.nansum(city == 1)) / land * 100
    rows.append({"step": b, "city": round(share, 2)})
figs.write_csv(data / "city.csv", rows)
dc.legend_strip(out / "legend.png", [CITY[k] for k in (0, 1, 2, 3)], width=700)
print("city share:", rows[0], rows[-1])

suit = figs.read(work / "suit.tif")[0]
figs.raster(None, out / "suitability.png", array=suit, cmap="viridis", unit="suitability", title="Suitability",
            width=620)

# the rule written with nbcount: every neighbour with a value counts, so only the suitability decides
w = figs.workdir("land-change-a/ca_nbcount")
figs.run(figs.add_writes(base.replace("nbsum(c == 1, 1)", "nbcount(c == 1, 1)"), {"growth.city": "city.tif"}), w)
city = figs.read(w / "city.tif")[0].filled(np.nan)
figs.raster(None, out / "nbcount_city.png", array=composite(city), palette=CITY, legend=False, width=480)
print(f"with nbcount: city {float(np.nansum(city == 1)) / land * 100:.1f}% of the land")

# the template as it comes (bar 0.8 - 0.008 a step, 25 steps)
w = figs.workdir("land-change-a/ca_template")
figs.run(figs.add_writes(template, {"growth.city": "city.tif"}), w)
city = figs.read(w / "city.tif")[0].filled(np.nan)
print(f"the template as it comes: city {int(np.nansum(city == 1))} cells, {float(np.nansum(city == 1)) / land * 100:.1f}% of the land")

# the same automaton with another neighbourhood rule: at least k neighbours of city
for k in (1, 2, 3, 4):
    m = base.replace("nbsum(c == 1, 1) >= 2", f"nbsum(c == 1, 1) >= {k}")
    assert k == 2 or m != base
    w = figs.workdir(f"land-change-a/ca_k{k}")
    figs.run(figs.add_writes(m, {"growth.city": "city.tif"}), w)
    city = figs.read(w / "city.tif")[0].filled(np.nan)
    share = float(np.nansum(city == 1)) / land * 100
    figs.raster(None, out / f"rule_{k}.png", array=composite(city), palette=CITY, legend=False, width=480)
    print(f"k={k}: city {share:.1f}% of the land")

# ---- drawing: the rule on a toy grid (the 8 neighbours of a cell, counted)
g = np.array([
    [0, 0, 0, 0, 0, 0],
    [0, 1, 1, 0, 0, 0],
    [0, 1, 1, 1, 0, 0],
    [0, 0, 1, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
    [0, 0, 0, 0, 0, 0],
])
counts = np.zeros_like(g)
for i in range(6):
    for j in range(6):
        win = g[max(0, i - 1): i + 2, max(0, j - 1): j + 2]
        counts[i, j] = win.sum()        # the cell included, as nbsum does (it is 0 here)
nxt = np.where((g == 0) & (counts >= 2), 1, g)   # nbsum(c == 1, 1) >= 2
fig, axs = plt.subplots(1, 3, figsize=(9.6, 3.5), dpi=150)
for ax, arr, title in zip(axs, (g, counts, nxt), ("Step t: the city", "nbsum(c == 1, 1)", "Step t + 1")):
    ax.set_axis_off()
    for i in range(6):
        for j in range(6):
            if ax is axs[1]:
                col = "#f3d9e8" if g[i, j] == 0 and arr[i, j] >= 2 else ("#b4508a" if g[i, j] == 1 else "#f4f1e8")
            else:
                col = "#b4508a" if arr[i, j] == 1 else "#f4f1e8"
                if ax is axs[2] and g[i, j] == 0 and arr[i, j] == 1:
                    col = "#d98cb8"
            ax.add_patch(plt.Rectangle((j, 5 - i), 1, 1, facecolor=col, edgecolor="white", linewidth=1.5))
            if ax is axs[1]:
                ax.text(j + 0.5, 5.5 - i, str(arr[i, j]), ha="center", va="center", fontsize=10,
                        color="white" if g[i, j] == 1 else figs.INK)
    ax.set_xlim(-0.1, 6.1)
    ax.set_ylim(-0.1, 6.1)
    ax.set_aspect("equal")
    ax.set_title(title, fontsize=11, loc="left", fontfamily="monospace" if "nb" in title else None)
# the window of one cell
axs[1].add_patch(plt.Rectangle((2, 1), 3, 3, fill=False, edgecolor=figs.EMBER, linewidth=2.2))
axs[0].add_patch(plt.Rectangle((2, 1), 3, 3, fill=False, edgecolor=figs.EMBER, linewidth=2.2))
fig.tight_layout()
fig.savefig(out / "rule.png", dpi=150, bbox_inches="tight", pad_inches=0.06)
plt.close(fig)
figs._shrink(out / "rule.png")
print("done")
