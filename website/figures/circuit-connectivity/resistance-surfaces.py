"""Figures of "Resistance surfaces" (circuit connectivity, part 2): a resistance from land cover classes, one from a
habitat suitability (Keeley et al.'s transformation), and how the current between two forests changes with them."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-networks"))
import _clc  # noqa: E402
from _clc import figs, np  # noqa: E402

SLUG = "circuit-connectivity/resistance-surfaces"
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
id = "forest"
op = "map.calc"
inputs = {{ l = "land.map" }}
params = {{ expression = "l >= 23 and l <= 25" }}

[[nodes]]
id = "suitability"
op = "map.calc"
inputs = {{ f = "forest.map" }}
params = {{ expression = "nbmean(f, 5)" }}

[[nodes]]
id = "from_suitability"
op = "map.calc"
inputs = {{ s = "suitability.map" }}
params = {{ expression = "100 - 99 * (1 - exp(-8 * s)) / (1 - exp(-8))" }}

[[nodes]]
id = "weaker"
op = "map.calc"
inputs = {{ r = "resistance.map" }}
params = {{ expression = "sqrt(r)" }}

[[nodes]]
id = "stronger"
op = "map.calc"
inputs = {{ r = "resistance.map" }}
params = {{ expression = "r ^ 2" }}

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = {{ map = "land.map" }}
params = {{ habitat = [23, 24, 25] }}

[[nodes]]
id = "patches"
op = "conn.nodes"
inputs = {{ map = "land.map" }}
params = {{ habitat = [23, 24, 25] }}

[[nodes]]
id = "ids"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "patches.table" }}
params = {{ column = "id" }}

[[nodes]]
id = "two_forests"
op = "map.calc"
inputs = {{ i = "ids.map" }}
params = {{ expression = "coalesce(if i == 55 then 1 else if i == 96 then 2 else 0, 0)" }}
"""
writes = {"resistance.map": "resistance.tif", "suitability.map": "suitability.tif",
          "from_suitability.map": "from_suitability.tif", "two_forests.map": "two.tif", "ids.map": "ids.tif"}
for name in ("resistance", "weaker", "stronger", "from_suitability"):
    MODEL += f"""
[[nodes]]
id = "current_{name}"
op = "circuit.pairwise"
inputs = {{ habitat = "{name}.map", points = "two_forests.map" }}
params = {{ cum_map = true, max_map = true }}
"""
    writes[f"current_{name}.cum"] = f"current_{name}.tif"
    writes[f"current_{name}.resistances"] = f"reff_{name}.csv"
figs.run(figs.add_writes(MODEL, writes), work)

res = figs.read(work / "resistance.tif")[0]
figs.raster(None, out / "resistance.png", array=res, cmap="magma_r", log=True, vmin=1, vmax=100, unit="Resistance",
            width=900)
suit = figs.read(work / "suitability.tif")[0]
figs.raster(None, out / "suitability.png", array=suit, cmap="YlGn", vmin=0, vmax=1, unit="Forest within 500 m (share)",
            width=640)
rs = figs.read(work / "from_suitability.tif")[0]
figs.raster(None, out / "from-suitability.png", array=rs, cmap="magma_r", log=True, vmin=1, vmax=100,
            unit="Resistance", width=640)

two = figs.read(work / "two.tif")[0]
for name in ("resistance", "weaker", "stronger", "from_suitability"):
    cur = figs.read(work / f"current_{name}.tif")[0]
    cur = np.ma.masked_less_equal(cur, 0)
    cur = np.ma.masked_where(np.ma.filled(two, 0) > 0, cur)
    hi = float(np.percentile(cur.compressed(), 99.9))
    a = np.ma.filled(cur.astype(float), np.nan)
    a[np.ma.filled(two, 0) > 0] = np.nan
    figs.raster(None, out / f"current-{name}.png", array=a, cmap="magma", log=True, vmin=hi / 150, vmax=hi,
                legend=False, width=520)
    r = figs.read_csv(work / "output" / f"reff_{name}.csv")
    print(name, "effective resistance", r)
    # how concentrated: the share of the current in the cells of the top 5 % of current
    v = np.sort(cur.compressed())[::-1]
    print("   share of the current in the top 5% of cells:", round(float(v[: len(v) // 20].sum() / v.sum()), 3))

# Keeley et al.'s transformation of a suitability into a resistance
rows = []
for h in np.linspace(0, 1, 51):
    r = {"suitability": round(float(h), 2)}
    for c in (0.25, 2, 4, 8, 16):
        r[f"c{c:g}"] = round(float(100 - 99 * (1 - np.exp(-c * h)) / (1 - np.exp(-c))), 2)
    r["linear"] = round(float(100 - 99 * h), 2)
    rows.append(r)
figs.write_csv(data / "keeley.csv", rows)
v, c = np.unique(res.compressed(), return_counts=True)
print({float(a): round(100 * b / res.count(), 1) for a, b in zip(v, c)})
