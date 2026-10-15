"""Figures of "Circuitscape: currents between forests" (circuit connectivity, part 3): the pairwise, one-to-all,
all-to-one and advanced modes between the eight largest forests of the CORINE landscape, and a least-cost corridor
beside the current between two of them."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-networks"))
import _clc  # noqa: E402
from _clc import figs, np, plt  # noqa: E402

SLUG = "circuit-connectivity/circuitscape"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)

RULES = _clc.RESISTANCE_RULES.replace("\n", "\\n")
MODEL = f"""format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = {{ path = "clc_100m.tif" }}

[[nodes]]
id = "resistance"
op = "raster.reclassify"
inputs = {{ map = "land.map" }}
params = {{ rules = "{RULES}" }}

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = {{ map = "land.map" }}
params = {{ habitat = [23, 24, 25], area_unit = "ha" }}

[[nodes]]
id = "patches"
op = "conn.nodes"
inputs = {{ map = "land.map" }}
params = {{ habitat = [23, 24, 25], area_unit = "ha" }}

[[nodes]]
id = "area"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "patches.table" }}
params = {{ column = "attribute" }}

[[nodes]]
id = "patch"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "patches.table" }}
params = {{ column = "id" }}

[[nodes]]
id = "cores"
op = "map.calc"
inputs = {{ a = "area.map", p = "patch.map" }}
params = {{ expression = "coalesce(if a >= 1500 then p else 0, 0)" }}

[[nodes]]
id = "pairwise"
op = "circuit.pairwise"
inputs = {{ habitat = "resistance.map", points = "cores.map" }}
params = {{ cum_map = true, max_map = true }}

[[nodes]]
id = "one_to_all"
op = "circuit.one_to_all"
inputs = {{ habitat = "resistance.map", points = "cores.map" }}
params = {{ cum_map = true, max_map = true }}

[[nodes]]
id = "all_to_one"
op = "circuit.one_to_all"
inputs = {{ habitat = "resistance.map", points = "cores.map" }}
params = {{ cum_map = true, max_map = true, all_to_one = true }}

[[nodes]]
id = "conductance"
op = "map.calc"
inputs = {{ r = "resistance.map" }}
params = {{ expression = "1 / r" }}

[[nodes]]
id = "source"
op = "map.calc"
inputs = {{ c = "cores.map" }}
params = {{ expression = "if c == 55 then 1 else 0" }}

[[nodes]]
id = "ground"
op = "map.calc"
inputs = {{ c = "cores.map" }}
params = {{ expression = "if c > 0 and c != 55 then 1 / 0 else 0" }}

[[nodes]]
id = "advanced"
op = "circuit.advanced"
inputs = {{ conductance = "conductance.map", source = "source.map", ground = "ground.map" }}

[[nodes]]
id = "two"
op = "map.calc"
inputs = {{ p = "patch.map" }}
params = {{ expression = "coalesce(if p == 55 then 1 else if p == 96 then 2 else 0, 0)" }}

[[nodes]]
id = "between_two"
op = "circuit.pairwise"
inputs = {{ habitat = "resistance.map", points = "two.map" }}
params = {{ cum_map = true, max_map = true }}

[[nodes]]
id = "a"
op = "map.calc"
inputs = {{ t = "two.map" }}
params = {{ expression = "t == 1" }}

[[nodes]]
id = "b"
op = "map.calc"
inputs = {{ t = "two.map" }}
params = {{ expression = "t == 2" }}

[[nodes]]
id = "transition"
op = "cost.transition"
inputs = {{ map = "resistance.map" }}
params = {{ function = "mean", directions = 8, reciprocal = true }}

[[nodes]]
id = "corrected"
op = "cost.geo_correction"
inputs = {{ transition = "transition.transition" }}

[[nodes]]
id = "cost_a"
op = "cost.acc_cost"
inputs = {{ transition = "corrected.transition", origins = "a.map" }}

[[nodes]]
id = "cost_b"
op = "cost.acc_cost"
inputs = {{ transition = "corrected.transition", origins = "b.map" }}

[[nodes]]
id = "total"
op = "map.calc"
inputs = {{ a = "cost_a.map", b = "cost_b.map" }}
params = {{ expression = "a + b" }}
"""
figs.run(figs.add_writes(MODEL, {
    "cores.map": "cores.tif", "pairwise.cum": "pair_cum.tif", "pairwise.max": "pair_max.tif",
    "one_to_all.cum": "ota_cum.tif", "all_to_one.cum": "ato_cum.tif", "advanced.current": "adv_current.tif",
    "advanced.voltage": "adv_voltage.tif", "between_two.cum": "two_cum.tif", "total.map": "total.tif",
    "two.map": "two.tif", "pairwise.resistances": "pair_r.csv", "one_to_all.resistances": "ota_r.csv",
    "all_to_one.resistances": "ato_r.csv", "between_two.resistances": "two_r.csv"}), work)
o = work / "output"

cores = figs.read(work / "cores.tif")[0]
cf = np.ma.filled(cores, 0).astype(int)
CORES = sorted(int(v) for v in np.unique(cf) if v)
print("cores:", CORES)
cen = _clc.centroids(cf)


def current_png(name, png, legend=True, width=900, label=True, extra_mask=None, title=None, span=150):
    cur = figs.read(work / name)[0]
    a = np.ma.masked_less_equal(cur, 0)
    a = np.ma.masked_where((cf > 0) if extra_mask is None else extra_mask, a)
    hi = float(np.percentile(a.compressed(), 99.9))
    arr = np.ma.filled(a.astype(float), np.nan)
    mask = cf > 0 if extra_mask is None else extra_mask
    arr[mask] = np.nan
    figs.raster(None, png, array=arr, cmap="magma", log=True, vmin=hi / span, vmax=hi, unit="Current (A)",
                legend=legend, width=width, title=title)


current_png("pair_cum.tif", out / "pairwise-cum.png")
current_png("ota_cum.tif", out / "one-to-all.png", width=640)
current_png("ato_cum.tif", out / "all-to-one.png", width=640)
current_png("adv_current.tif", out / "advanced.png", width=900)
two = np.ma.filled(figs.read(work / "two.tif")[0], 0) > 0
_c2 = np.ma.masked_less_equal(figs.read(work / "two_cum.tif")[0], 0)
_c2 = np.ma.masked_where(two, _c2)
_hi = float(np.percentile(_c2.compressed(), 99))
_a2 = np.ma.filled(np.ma.clip(_c2 / _hi, 1e-3, 1).astype(float), np.nan)
_a2[two] = np.nan
figs.raster(None, out / "compare-current.png", array=_a2, cmap="magma", log=True, vmin=4e-2, vmax=1, legend=False)

total = figs.read(work / "total.tif")[0]
mn = float(total.min())
excess = 100 * (total / mn - 1)
arr = np.ma.filled((1 - np.ma.clip(excess, 0, 25) / 25).astype(float), np.nan)
arr[two] = np.nan
figs.raster(None, out / "compare-corridor.png", array=arr, cmap="magma", vmin=0, vmax=1, legend=False)
print("least cost between 55 and 96:", mn, "cells within 5%:", int(((excess <= 5) & ~two).sum()),
      "within 10%:", int(((excess <= 10) & ~two).sum()))
print("effective resistance 55-96:", figs.read_csv(o / "two_r.csv"))

# the cores labelled
lab = np.zeros(cf.shape)
lab[cf > 0] = 1
_clc.network_map(out / "cores.png", np.ma.masked_equal(cf, 0).astype(float), node_color=figs.FOREST,
                 highlight={c: str(c) for c in CORES}, base=_clc.group_array(np.ma.filled(figs.read(work / "clc_100m.tif")[0], 0)),
                 width=900)

# the matrix of effective resistances
pr = figs.read_csv(o / "pair_r.csv")
ids = [int(r["id"]) for r in pr]
M = np.array([[float(r[str(j)]) for j in ids] for r in pr])


def heat(ax):
    im = ax.imshow(M, cmap="YlOrBr")
    ax.set_xticks(range(len(ids)), [str(i) for i in ids])
    ax.set_yticks(range(len(ids)), [str(i) for i in ids])
    ax.grid(False)
    for i in range(len(ids)):
        for j in range(len(ids)):
            if i != j:
                ax.text(j, i, f"{M[i, j]:.1f}", ha="center", va="center", fontsize=8,
                        color="white" if M[i, j] > 5.5 else figs.INK)
    ax.set_xlabel("Forest")
    ax.set_ylabel("Forest")
    cb = plt.colorbar(im, ax=ax, pad=0.02)
    cb.outline.set_visible(False)
    cb.set_label("Effective resistance (Ω)", color=figs.INK2)


figs.chart(out / "resistances.png", heat, width=700, height=560)
print("pairwise matrix", ids)
print(M.round(2))
print("one-to-all:", figs.read_csv(o / "ota_r.csv"))
print("all-to-one:", figs.read_csv(o / "ato_r.csv"))
# straight distances between the cores' centres (km), for isolation by distance vs resistance
for i in ids:
    print(i, [round(float(np.hypot(cen[i][0] - cen[j][0], cen[i][1] - cen[j][1])) / 10, 1) for j in ids])
rows = []
for a in range(len(ids)):
    for b in range(a + 1, len(ids)):
        i, j = ids[a], ids[b]
        rows.append({"km": round(float(np.hypot(cen[i][0] - cen[j][0], cen[i][1] - cen[j][1])) / 10, 1),
                     "resistance": round(M[a, b], 3), "pair": f"{i}-{j}"})
rows.sort(key=lambda r: r["km"])
figs.write_csv(data / "resistance-vs-distance.csv", [{"km": r["km"], "resistance": r["resistance"]} for r in rows])
d = np.array([r["km"] for r in rows])
rr = np.array([r["resistance"] for r in rows])
print("Pearson distance vs effective resistance:", round(float(np.corrcoef(d, rr)[0, 1]), 2))
