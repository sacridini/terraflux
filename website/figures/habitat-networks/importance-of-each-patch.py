"""Figures of "The importance of each patch" (habitat networks, part 3): dPC and its fractions, the importance of
the links and the patches' centralities, for the forests of the CORINE landscape at 2 km."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _clc  # noqa: E402
from _clc import figs, np, plt  # noqa: E402

SLUG = "habitat-networks/importance-of-each-patch"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)

MODEL = """format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "land.map" }
params = { habitat = [23, 24, 25], area_unit = "ha", distance = "edge", landscape_area = 250000 }

[[nodes]]
id = "importance"
op = "conn.node_importance"
inputs = { graph = "graph.graph" }
params = { distance = 2000, probability = 0.5 }

[[nodes]]
id = "connector"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPCconnector" }

[[nodes]]
id = "links"
op = "conn.link_importance"
inputs = { graph = "graph.graph" }
params = { distance = 2000, probability = 0.5 }

[[nodes]]
id = "centrality"
op = "conn.centrality"
inputs = { graph = "graph.graph" }
params = { distance = 2000 }

[[nodes]]
id = "areas"
op = "conn.nodes"
inputs = { map = "land.map" }
params = { habitat = [23, 24, 25], area_unit = "ha" }

[[nodes]]
id = "pairs"
op = "conn.distances"
inputs = { map = "land.map" }
params = { habitat = [23, 24, 25], distance = "edge", max_distance = 2000 }

[[nodes]]
id = "ids"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "id" }

[[nodes]]
id = "intra"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPCintra" }

[[nodes]]
id = "flux"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPCflux" }

[[nodes]]
id = "dpc"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "importance.table" }
params = { column = "dPC" }

[[nodes]]
id = "between"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "centrality.table" }
params = { column = "BWC" }
"""
figs.run(figs.add_writes(MODEL, {
    "importance.table": "importance.csv", "links.table": "links.csv", "centrality.table": "centrality.csv",
    "areas.table": "areas.csv", "pairs.table": "pairs.csv", "ids.map": "ids.tif", "intra.map": "intra.tif",
    "flux.map": "flux.tif", "connector.map": "connector.tif", "dpc.map": "dpc.tif", "between.map": "between.tif"}), work)

o = work / "output"
imp = {int(r["id"]): {k: float(v) for k, v in r.items()} for r in figs.read_csv(o / "importance.csv")}
areas = {int(r["id"]): float(r["attribute"]) for r in figs.read_csv(o / "areas.csv")}
cent = {int(r["id"]): {k: float(v or "nan") for k, v in r.items()} for r in figs.read_csv(o / "centrality.csv")}
links = figs.read_csv(o / "links.csv")
pairs = {(int(r["from"]), int(r["to"])): float(r["distance"]) for r in figs.read_csv(o / "pairs.csv")}
ids = figs.read(work / "ids.tif")[0]

# ---------------------------------------------------------------- the three fractions, one scale
for name, label in (("dpc", "dPC"), ("intra", "dPCintra"), ("flux", "dPCflux"), ("connector", "dPCconnector")):
    v = figs.read(work / f"{name}.tif")[0]
    v = np.ma.masked_less_equal(v, 0)
    lab = {i: str(i) for i in (55, 110, 8, 47, 30, 96, 65, 81, 100, 58)} if name in ("dpc", "connector") else None
    _clc.network_map(out / f"{name}.png", ids, values=np.ma.filled(v.astype(float), np.nan), cmap="viridis",
                     log=True, vmin=0.001, vmax=30, unit=f"{label} (%)", width=640 if name != "dpc" else 900,
                     highlight=lab)

# ---------------------------------------------------------------- dPC against area
top_conn = sorted(imp, key=lambda i: -imp[i]["dPCconnector"])[:6]
top_dpc = sorted(imp, key=lambda i: -imp[i]["dPC"])[:12]


def scatter(ax):
    x = np.array([areas[i] for i in imp])
    y = np.array([imp[i]["dPC"] for i in imp])
    c = np.array([imp[i]["dPCconnector"] / max(imp[i]["dPC"], 1e-12) for i in imp])
    sc = ax.scatter(x, y, c=c, cmap="plasma", vmin=0, vmax=1, s=22, edgecolors="white", linewidths=0.4, zorder=3)
    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("Area of the patch (ha)")
    ax.set_ylabel("dPC (% of PC lost without it)")
    cb = plt.colorbar(sc, ax=ax, pad=0.02)
    cb.set_label("Share of dPC as a connector", color=figs.INK2)
    cb.outline.set_visible(False)
    offs = [(5, 3), (5, 3), (-17, 3), (5, -11), (5, -11), (-18, -9)]
    for k, i in enumerate(top_conn):
        ax.annotate(f"{i}", (areas[i], imp[i]["dPC"]), xytext=offs[k], textcoords="offset points", fontsize=8.5,
                    color=figs.INK)


figs.chart(out / "dpc-vs-area.png", scatter, width=900, height=520)

rows = []
for i in top_dpc:
    rows.append({"patch": f"{i} ({areas[i]:,.0f} ha)", "intra": round(imp[i]["dPCintra"], 2),
                 "flux": round(imp[i]["dPCflux"], 2), "connector": round(imp[i]["dPCconnector"], 2)})
figs.write_csv(data / "top-patches.csv", rows)

# ---------------------------------------------------------------- the links
lk = sorted(links, key=lambda r: -float(r["dPC_removal"]))
used = [(int(r["from"]), int(r["to"]), float(r["dPC_removal"])) for r in lk if float(r["dPC_removal"]) > 0]
mx = max(v for _, _, v in used)
top = used[:40]
_clc.network_map(out / "links.png", ids, links=[(i, j) for i, j, _ in top],
                 link_weights=[(np.log10(v) - np.log10(top[-1][2])) / (np.log10(mx) - np.log10(top[-1][2])) for *_, v in top],
                 node_color="#9cc5a6", link_color="#d0471b", link_width=(0.8, 4.0), width=900)
print("links with dPC > 0:", len(used), "of", len(links))
for i, j, v in used[:8]:
    print("link", i, j, round(v, 2), "gap (m)", round(pairs.get((i, j), pairs.get((j, i), float("nan")))),
          "areas", areas[i], areas[j])

# ---------------------------------------------------------------- numbers for the text
def ranks(v):
    order = np.argsort(np.argsort(v))
    return order


ida = list(imp)
a = np.array([areas[i] for i in ida])
for k in ("dPC", "dPCintra", "dPCflux", "dPCconnector"):
    v = np.array([imp[i][k] for i in ida])
    rho = np.corrcoef(ranks(a), ranks(v))[0, 1]
    print(f"Spearman area vs {k}: {rho:.2f}")
bw = np.array([cent[i]["BWC"] for i in ida])
vc = np.array([imp[i]["dPCconnector"] for i in ida])
print("Spearman BWC vs dPCconnector:", round(np.corrcoef(ranks(bw), ranks(vc))[0, 1], 2))
print("top dPC:")
for i in top_dpc[:8]:
    print(i, areas[i], {k: round(imp[i][k], 2) for k in ("dPC", "dPCintra", "dPCflux", "dPCconnector")},
          "BWC", cent[i]["BWC"], "degree", cent[i]["degree"])
print("top connector:")
for i in top_conn:
    print(i, areas[i], {k: round(imp[i][k], 2) for k in ("dPC", "dPCintra", "dPCflux", "dPCconnector")},
          "rank by area", int(np.sum(a > areas[i])) + 1, "BWC", cent[i]["BWC"])
print("median area", np.median(a), "n", len(a))
