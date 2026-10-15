"""Figures of "Euclidean and least-cost links" (habitat networks, part 4): a resistance made from the land cover,
the forests' graph with straight (edge-to-edge) and least-cost distances, and how the importance of the patches
changes between them."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _clc  # noqa: E402
from _clc import figs, np, plt  # noqa: E402

SLUG = "habitat-networks/least-cost-links"
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
id = "straight"
op = "conn.graph_from_raster"
inputs = {{ map = "land.map" }}
params = {{ habitat = [23, 24, 25], area_unit = "ha", distance = "edge", landscape_area = 250000 }}

[[nodes]]
id = "least_cost"
op = "conn.graph_from_raster"
inputs = {{ map = "land.map", resistance = "resistance.map" }}
params = {{ habitat = [23, 24, 25], area_unit = "ha", distance = "least-cost", landscape_area = 250000 }}

[[nodes]]
id = "imp_straight"
op = "conn.node_importance"
inputs = {{ graph = "straight.graph" }}
params = {{ distance = 1000, probability = 0.5 }}

[[nodes]]
id = "imp_cost"
op = "conn.node_importance"
inputs = {{ graph = "least_cost.graph" }}
params = {{ distance = 20000, probability = 0.5 }}

[[nodes]]
id = "over_straight"
op = "conn.overall"
inputs = {{ graph = "straight.graph" }}
params = {{ distance = [500, 1000, 2000, 5000], probability = 0.5 }}

[[nodes]]
id = "over_cost"
op = "conn.overall"
inputs = {{ graph = "least_cost.graph" }}
params = {{ distance = [10000, 20000, 40000, 100000], probability = 0.5 }}

[[nodes]]
id = "dpc_straight"
op = "conn.to_map"
inputs = {{ graph = "straight.graph", table = "imp_straight.table" }}
params = {{ column = "dPC" }}

[[nodes]]
id = "dpc_cost"
op = "conn.to_map"
inputs = {{ graph = "least_cost.graph", table = "imp_cost.table" }}
params = {{ column = "dPC" }}

[[nodes]]
id = "ids"
op = "conn.to_map"
inputs = {{ graph = "straight.graph", table = "imp_straight.table" }}
params = {{ column = "id" }}

[[nodes]]
id = "d_straight"
op = "conn.distances"
inputs = {{ map = "land.map" }}
params = {{ habitat = [23, 24, 25], distance = "edge" }}

[[nodes]]
id = "d_cost"
op = "conn.distances"
inputs = {{ map = "land.map", resistance = "resistance.map" }}
params = {{ habitat = [23, 24, 25], distance = "least-cost" }}
"""
figs.run(figs.add_writes(MODEL, {
    "resistance.map": "resistance.tif", "dpc_straight.map": "dpc_straight.tif", "dpc_cost.map": "dpc_cost.tif",
    "ids.map": "ids.tif", "imp_straight.table": "imp_straight.csv", "imp_cost.table": "imp_cost.csv",
    "over_straight.table": "over_straight.csv", "over_cost.table": "over_cost.csv",
    "d_straight.table": "d_straight.csv", "d_cost.table": "d_cost.csv"}), work)

o = work / "output"
ids = figs.read(work / "ids.tif")[0]
res = figs.read(work / "resistance.tif")[0]

# ---------------------------------------------------------------- the resistance
figs.raster(None, out / "resistance.png", array=res, cmap="magma_r", log=True, unit="Resistance", width=900)

# ---------------------------------------------------------------- dPC both ways, for the before-and-after
for name in ("straight", "cost"):
    v = figs.read(work / f"dpc_{name}.tif")[0]
    _clc.network_map(out / f"dpc_{name}.png", ids, values=np.ma.filled(v.astype(float), np.nan), cmap="viridis",
                     log=True, vmin=0.01, vmax=30, legend=False, width=900)
# the colour bar alone, for the caption
fig = plt.figure(figsize=(6, 0.62), dpi=150)
cax = fig.add_axes([0.05, 0.55, 0.9, 0.25])
from matplotlib import colors as mc, cm  # noqa: E402
cb = fig.colorbar(cm.ScalarMappable(norm=mc.LogNorm(0.01, 30), cmap="viridis"), cax=cax, orientation="horizontal")
cb.outline.set_visible(False)
cb.ax.tick_params(labelsize=8.5)
cb.set_label("dPC (% of PC lost without the patch)", fontsize=9, color=figs.INK2)
fig.savefig(out / "dpc-scale.png", dpi=150, bbox_inches="tight", pad_inches=0.04)
plt.close(fig)

s = {int(r["id"]): float(r["dPC"]) for r in figs.read_csv(o / "imp_straight.csv")}
c = {int(r["id"]): float(r["dPC"]) for r in figs.read_csv(o / "imp_cost.csv")}
sc = {int(r["id"]): float(r["dPCconnector"]) for r in figs.read_csv(o / "imp_straight.csv")}
cc = {int(r["id"]): float(r["dPCconnector"]) for r in figs.read_csv(o / "imp_cost.csv")}
ida = sorted(s)
x = np.array([s[i] for i in ida])
y = np.array([c[i] for i in ida])
ratio = np.log10(np.maximum(y, 1e-4) / np.maximum(x, 1e-4))
movers = [ida[k] for k in np.argsort(-np.abs(ratio) * (np.maximum(x, y) > 0.3))[:6]]


def scatter(ax):
    ax.scatter(np.maximum(x, 1e-4), np.maximum(y, 1e-4), s=20, color=figs.FOREST, edgecolors="white", linewidths=0.4,
               zorder=3)
    lim = (1e-4, 60)
    ax.plot(lim, lim, color=figs.INK2, lw=0.8, ls="--", zorder=1)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlim(*lim)
    ax.set_ylim(*lim)
    ax.set_xlabel("dPC with straight links, 1 km (%)")
    ax.set_ylabel("dPC with least-cost links (%)")
    chain = [i for i in movers if s[i] > 0.9]
    ax.scatter([s[i] for i in chain], [c[i] for i in chain], s=34, color=figs.EMBER, edgecolors="white",
               linewidths=0.5, zorder=4)
    ax.annotate("small stepping stones\n" + ", ".join(str(i) for i in sorted(chain)) + ":\ntheir links cross farmland",
                (s[chain[0]], c[chain[0]]), xytext=(0.3, 6e-4), textcoords="data", fontsize=8.5, color=figs.INK,
                arrowprops=dict(arrowstyle="-", color=figs.INK2, lw=0.7))


figs.chart(out / "straight-vs-cost.png", scatter, width=700, height=560)


def ranks(v):
    return np.argsort(np.argsort(v))


print("Spearman dPC straight vs least-cost:", round(np.corrcoef(ranks(x), ranks(y))[0, 1], 2))
print("top 10 straight:", sorted(ida, key=lambda i: -s[i])[:10])
print("top 10 cost:", sorted(ida, key=lambda i: -c[i])[:10])
for i in movers:
    print("mover", i, round(s[i], 3), round(c[i], 3), "connector", round(sc[i], 3), round(cc[i], 3))
for f in ("over_straight", "over_cost"):
    for r in figs.read_csv(o / f"{f}.csv"):
        print(f, r["distance"], "NC", r["NC"], "EC(PC)", round(float(r["EC(PC)"])), "intra", round(float(r["PCintra(%)"]), 1),
              "direct", round(float(r["PCdirect(%)"]), 1), "step", round(float(r["PCstep(%)"]), 2))
ds = {(int(r["from"]), int(r["to"])): float(r["distance"]) for r in figs.read_csv(o / "d_straight.csv")}
dc = {(int(r["from"]), int(r["to"])): float(r["distance"]) for r in figs.read_csv(o / "d_cost.csv")}
rat = np.array([dc[k] / ds[k] for k in ds if k in dc and ds[k] > 300])
print("cost / straight distance: median", round(float(np.median(rat)), 1), "10-90%", np.percentile(rat, [10, 90]).round(1))
rows = [{"straight": round(s[i], 4), "least_cost": round(c[i], 4)} for i in ida]
figs.write_csv(data / "dpc-both.csv", rows)
