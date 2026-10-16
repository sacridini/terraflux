"""Figures of "IIC, PC and the equivalent connected area" (habitat networks, part 2): a network of four patches
computed by terraflux, then the forests of the CORINE landscape at many dispersal distances (the dashboard)."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _clc  # noqa: E402
from _clc import figs, np, plt  # noqa: E402

SLUG = "habitat-networks/connectivity-indices"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)

# ---------------------------------------------------------------- four patches, as Conefor's small examples
TOY = """format = "terraflux/1"

[[nodes]]
id = "patches"
op = "table.of"
params = { columns = ["id", "attribute"], rows = [[1, 100], [2, 50], [3, 20], [4, 80]] }
note = "Four patches and their areas (ha)."

[[nodes]]
id = "gaps"
op = "table.of"
params = { columns = ["from", "to", "value"], rows = [[1, 2, 800], [1, 3, 1500], [1, 4, 5000], [2, 3, 600], [2, 4, 4000], [3, 4, 3000]] }
note = "The distance between every two of them (m)."

[[nodes]]
id = "graph"
op = "conn.graph"
inputs = { nodes = "patches.table", pairs = "gaps.table" }
params = { landscape_area = 10000 }
note = "A landscape of 10,000 ha."

[[nodes]]
id = "overall"
op = "conn.overall"
inputs = { graph = "graph.graph" }
params = { distance = 1000, probability = 0.5 }
note = "Half of the dispersers cross 1 km."

[[nodes]]
id = "importance"
op = "conn.node_importance"
inputs = { graph = "graph.graph" }
params = { distance = 1000, probability = 0.5 }
"""
toy_work = figs.workdir(SLUG + "/toy")
figs.run(figs.add_writes(TOY, {"overall.table": "overall.csv", "importance.table": "importance.csv"}), toy_work)
tov = figs.read_csv(toy_work / "output" / "overall.csv")[0]
timp = figs.read_csv(toy_work / "output" / "importance.csv")
print("toy:", {k: tov[k] for k in ("NL", "IIC", "EC(IIC)", "PC", "PCnum", "EC(PC)", "PCintra(%)", "PCdirect(%)", "PCstep(%)")})
for r in timp:
    print("toy patch", r["id"], {k: round(float(r[k]), 2) for k in ("dPC", "dPCintra", "dPCflux", "dPCconnector", "dIIC")})
figs.write_csv(data / "toy-importance.csv", [{"patch": int(r["id"]), "intra": round(float(r["dPCintra"]), 2),
                                             "flux": round(float(r["dPCflux"]), 2),
                                             "connector": round(float(r["dPCconnector"]), 2)} for r in timp])

# the drawing of the four patches
P = {1: (1.0, 1.55), 2: (2.2, 2.75), 3: (3.05, 1.85), 4: (6.0, 1.0)}
A = {1: 100, 2: 50, 3: 20, 4: 80}
D = {(1, 2): 800, (1, 3): 1500, (1, 4): 5000, (2, 3): 600, (2, 4): 4000, (3, 4): 3000}
fig, ax = plt.subplots(figsize=(8.4, 3.2), dpi=150)
ax.set_aspect("equal")
ax.set_axis_off()
for (i, j), d in D.items():
    p = 0.5 ** (d / 1000)
    (x1, y1), (x2, y2) = P[i], P[j]
    ax.plot([x1, x2], [y1, y2], color=figs.INK, lw=0.6 + 3.2 * p, alpha=0.35 + 0.6 * p, zorder=1)
    mx, my = (x1 + x2) / 2, (y1 + y2) / 2
    off = {(1, 3): (-0.15, 0.0), (1, 4): (0.6, -0.08), (2, 4): (0.35, 0.0), (3, 4): (0.35, -0.1), (1, 2): (-0.32, 0.12),
           (2, 3): (0.3, 0.14)}[(i, j)]
    ax.text(mx + off[0], my + off[1], f"{d / 1000:g} km\np = {p:.2f}", ha="center", va="center", fontsize=8,
            color=figs.INK2, bbox=dict(boxstyle="round,pad=0.15", fc="white", ec="none", alpha=0.9), zorder=3)
for i, (x, y) in P.items():
    r = np.sqrt(A[i]) / 28
    ax.add_patch(plt.Circle((x, y), r, color=figs.FOREST, zorder=2))
    ax.text(x, y, f"{i}", ha="center", va="center", color="white", fontsize=10, fontweight="bold", zorder=4)
    ax.text(x, y - r - 0.12, f"{A[i]} ha", ha="center", va="top", fontsize=8.5, color=figs.INK, zorder=4)
ax.set_xlim(0.4, 6.6)
ax.set_ylim(0.55, 3.2)
fig.tight_layout()
fig.savefig(out / "toy-network.png", dpi=150)
plt.close(fig)
figs._shrink(out / "toy-network.png")

# ---------------------------------------------------------------- the forests at many distances
FRAMES = [250, 500, 1000, 2000, 3000, 5000, 10000]
CURVE = [100, 250, 500, 750, 1000, 1500, 2000, 3000, 5000, 7500, 10000, 15000, 20000]

MODEL = """format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = { path = "clc_100m.tif" }

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = { map = "land.raster" }
params = { habitat = [23, 24, 25], area_unit = "ha", distance = "edge", landscape_area = 250000 }

[[nodes]]
id = "overall"
op = "conn.overall"
inputs = { graph = "graph.graph" }
params = { distance = %s, probability = 0.5 }

[[nodes]]
id = "pairs"
op = "conn.distances"
inputs = { map = "land.raster" }
params = { habitat = [23, 24, 25], distance = "edge" }
""" % (sorted(set(CURVE) | set(FRAMES)),)
writes = {"overall.table": "overall.csv", "pairs.table": "pairs.csv"}
for d in FRAMES:
    MODEL += f"""
[[nodes]]
id = "imp_{d}"
op = "conn.node_importance"
inputs = {{ graph = "graph.graph" }}
params = {{ distance = {d}, probability = 0.5 }}

[[nodes]]
id = "dpc_{d}"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "imp_{d}.table" }}
params = {{ column = "dPC" }}
"""
    writes[f"dpc_{d}.map"] = f"dpc_{d}.tif"
MODEL += """
[[nodes]]
id = "ids"
op = "conn.to_map"
inputs = { graph = "graph.graph", table = "imp_1000.table" }
params = { column = "id" }
"""
writes["ids.map"] = "ids.tif"
figs.run(figs.add_writes(MODEL, writes), work)

ids = figs.read(work / "ids.tif")[0]
pairs = [(int(r["from"]), int(r["to"]), float(r["distance"])) for r in figs.read_csv(work / "output" / "pairs.csv")]
overall = {int(float(r["distance"])): r for r in figs.read_csv(work / "output" / "overall.csv")}
forest = float(overall[1000]["A"])


def km(d):
    return f"{d / 1000:g} km" if d >= 1000 else f"{d} m"


frames, chart = [], []
for d in FRAMES:
    v = figs.read(work / f"dpc_{d}.tif")[0]
    links = [(i, j) for i, j, g in pairs if g <= d]
    wts = [0.5 ** (g / d) for i, j, g in pairs if g <= d]  # 0.5 to 1
    wts = [(w - 0.5) * 2 for w in wts]
    _clc.network_map(out / f"dpc_{d}.png", ids, values=v, links=links, link_weights=wts, cmap="viridis", log=True,
                     vmin=0.01, vmax=30, unit="dPC (% of PC lost without the patch)", width=900,
                     link_width=(0.25, 1.3), dots=False)
    r = overall[d]
    ec = float(r["EC(PC)"])
    frames.append({"value": km(d), "src": f"/img/tutorials/{SLUG}/dpc_{d}.png",
                   "stats": {"Components": int(float(r["NC"])), "Links (IIC)": int(float(r["NL"])),
                             "EC(PC)": f"{ec:,.0f} ha", "EC(PC) / forest": f"{100 * ec / forest:.0f}%",
                             "Stepping stones (PCstep)": f"{float(r['PCstep(%)']):.0f}%"}})
    chart.append({"distance": km(d), "ECPC": round(100 * ec / forest, 2),
                  "ECIIC": round(100 * float(r["EC(IIC)"]) / forest, 2)})
figs.write_csv(data / "eca-frames.csv", chart)

curve = []
for d in CURVE:
    r = overall[d]
    curve.append({"distance": km(d), "intra": round(float(r["PCintra(%)"]), 2),
                  "direct": round(float(r["PCdirect(%)"]), 2), "step": round(float(r["PCstep(%)"]), 2),
                  "NC": int(float(r["NC"])), "ECPC": round(float(r["EC(PC)"])),
                  "ECIIC": round(float(r["EC(IIC)"])),
                  "PC": float(r["PC"]), "IIC": float(r["IIC"])})
figs.write_csv(data / "fractions.csv", [{k: c[k] for k in ("distance", "intra", "direct", "step")} for c in curve])
figs.write_csv(data / "overall.csv", curve)

import json  # noqa: E402

(work / "frames.json").write_text(json.dumps(frames, indent=1), encoding="utf-8")
for c in curve:
    print(c)
print("forest", forest)
