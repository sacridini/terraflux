"""Figures of "Cost distance, least-cost paths and corridors" (grid distances, part 1): the accumulated cost from a
forest of the CORINE landscape, its isochrones, the least-cost paths to four other forests, the least costs
between them and a least-cost corridor."""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-networks"))
import _clc  # noqa: E402
from _clc import figs, np, plt  # noqa: E402

SLUG = "grid-distances/cost-distance"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)

# a point inside each of five forests (the cell of the forest nearest its centre), for the tools that take points
FORESTS = {55: "South", 96: "North-east", 8: "South-west", 30: "North-west", 110: "Far north-east"}
ids_src = figs.WEB / ".cache" / "work" / "habitat-networks" / "importance-of-each-patch" / "ids.tif"
if not ids_src.exists():
    raise SystemExit("run website/figures/habitat-networks/importance-of-each-patch.py first")
import rasterio  # noqa: E402

with rasterio.open(ids_src) as ds:
    ida = ds.read(1)
    T = ds.transform
feats = []
for k, (pid, name) in enumerate(FORESTS.items(), start=1):
    r, c = np.nonzero(ida == pid)
    j = int(np.argmin((r - r.mean()) ** 2 + (c - c.mean()) ** 2))
    x, y = T * (c[j] + 0.5, r[j] + 0.5)
    feats.append({"type": "Feature", "properties": {"id": k, "forest": pid, "name": name},
                  "geometry": {"type": "Point", "coordinates": [float(x), float(y)]}})
gj = {"type": "FeatureCollection", "crs": {"type": "name", "properties": {"name": "urn:ogc:def:crs:EPSG::3035"}},
      "features": feats}
(work / "forests.geojson").write_text(json.dumps(gj, indent=1), encoding="utf-8")
shutil.copy2(work / "forests.geojson", data / "forests.geojson")

RULES = _clc.RESISTANCE_RULES.replace("\n", "\\n")
MODEL = f"""format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = {{ path = "clc_100m.tif" }}

[[nodes]]
id = "resistance"
op = "raster.reclassify"
inputs = {{ map = "land.raster" }}
params = {{ rules = "{RULES}" }}

[[nodes]]
id = "transition"
op = "cost.transition"
inputs = {{ map = "resistance.map" }}
params = {{ function = "mean", directions = 8, reciprocal = true }}

[[nodes]]
id = "corrected"
op = "cost.geo_correction"
inputs = {{ transition = "transition.transition" }}
params = {{ type = "c" }}

[[nodes]]
id = "forests"
op = "vector.read"
params = {{ path = "forests.geojson" }}

[[nodes]]
id = "graph"
op = "conn.graph_from_raster"
inputs = {{ map = "land.raster" }}
params = {{ habitat = [23, 24, 25] }}

[[nodes]]
id = "patches"
op = "conn.nodes"
inputs = {{ map = "land.raster" }}
params = {{ habitat = [23, 24, 25] }}

[[nodes]]
id = "patch"
op = "conn.to_map"
inputs = {{ graph = "graph.graph", table = "patches.table" }}
params = {{ column = "id" }}

[[nodes]]
id = "south"
op = "map.calc"
inputs = {{ p = "patch.map" }}
params = {{ expression = "coalesce(p == 55, 0)" }}

[[nodes]]
id = "north_east"
op = "map.calc"
inputs = {{ p = "patch.map" }}
params = {{ expression = "coalesce(p == 96, 0)" }}

[[nodes]]
id = "cost_south"
op = "cost.acc_cost"
inputs = {{ transition = "corrected.transition", origins = "south.map" }}

[[nodes]]
id = "cost_north_east"
op = "cost.acc_cost"
inputs = {{ transition = "corrected.transition", origins = "north_east.map" }}

[[nodes]]
id = "both"
op = "map.calc"
inputs = {{ a = "cost_south.map", b = "cost_north_east.map" }}
params = {{ expression = "a + b" }}

[[nodes]]
id = "paths"
op = "cost.shortest_path"
inputs = {{ transition = "corrected.transition", origin = "forests.vector", goals = "forests.vector" }}

[[nodes]]
id = "save_paths"
op = "vector.write"
inputs = {{ vector = "paths.lines" }}
params = {{ path = "paths.geojson" }}

[[nodes]]
id = "least_costs"
op = "cost.distance"
inputs = {{ transition = "corrected.transition", from = "forests.vector" }}
"""
figs.run(figs.add_writes(MODEL, {"resistance.map": "resistance.tif", "cost_south.map": "cost_south.tif",
                                 "both.map": "both.tif", "patch.map": "patch.tif", "paths.map": "paths_share.tif",
                                 "least_costs.table": "least_costs.csv"}), work)

H = W = 500
x0, y0 = T.c, T.f


def to_px(x, y):
    return (x - x0) / 100 - 0.5, (y0 - y) / 100 - 0.5


patch = np.ma.filled(figs.read(work / "patch.tif")[0], 0).astype(int)
cost = figs.read(work / "cost_south.tif")[0] / 1000  # km-equivalents
cost = np.ma.masked_invalid(np.ma.filled(cost.astype(float), np.nan))
lines = json.loads((work / "paths.geojson").read_text(encoding="utf-8"))
from matplotlib import colors as mc  # noqa: E402

fig = plt.figure(figsize=(6.4, 5.6), dpi=150)
ax = fig.add_axes([0, 0, 0.84, 1])
ax.set_axis_off()
im = ax.imshow(cost, cmap="viridis_r", norm=mc.Normalize(0, float(np.nanpercentile(cost.filled(np.nan), 99))),
               interpolation="nearest")
cs = ax.contour(cost.filled(np.nan), levels=[25, 50, 100, 150, 200, 300, 400], colors="white", linewidths=0.6,
                alpha=0.9)
ax.clabel(cs, fmt="%d", fontsize=7)
for f in lines["features"]:
    if f["geometry"]:
        xy = np.array([to_px(*p) for p in f["geometry"]["coordinates"]])
        ax.plot(xy[:, 0], xy[:, 1], color=figs.EMBER, lw=1.6)
for f in feats:
    px, py = to_px(*f["geometry"]["coordinates"])
    ax.plot(px, py, "o", ms=5, mfc="white", mec=figs.INK, zorder=5)
    ax.annotate(f["properties"]["name"], (px, py), xytext=(5, 5), textcoords="offset points", fontsize=8,
                color=figs.INK, bbox=dict(boxstyle="round,pad=0.15", fc="white", ec="none", alpha=0.85))
ax.set_xlim(-0.5, W - 0.5)
ax.set_ylim(H - 0.5, -0.5)
cax = fig.add_axes([0.87, 0.12, 0.03, 0.76])
cb = fig.colorbar(im, cax=cax)
cb.outline.set_visible(False)
cb.set_label("Cost from the southern forest (km of forest)", fontsize=9, color=figs.INK2)
fig.savefig(out / "acc-cost.png", dpi=150)
plt.close(fig)
figs._shrink(out / "acc-cost.png")

res = figs.read(work / "resistance.tif")[0]
figs.raster(None, out / "resistance.png", array=res, cmap="magma_r", log=True, vmin=1, vmax=100, unit="Resistance",
            width=760)

# the corridor: cells whose cheapest route between the two forests is little dearer than the least cost
both = figs.read(work / "both.tif")[0]
mn = float(both.min())
ex = 100 * (both / mn - 1)
two = (patch == 55) | (patch == 96)
cls = np.zeros(both.shape)
for k, lim in enumerate((10, 5, 2), start=1):
    cls[np.ma.filled(ex, 1e9) <= lim] = k
cls[two] = 4
groups = _clc.group_array(np.ma.filled(figs.read(work / "clc_100m.tif")[0], 0))
rgba = np.ones(both.shape + (4,))
for i, (_, col, _) in enumerate(_clc.GROUPS, start=1):
    c = 0.3 * np.array(mc.to_rgba(col)) + 0.7
    rgba[groups == i] = c
for k, col in ((1, "#fde0a6"), (2, "#f6a54a"), (3, "#c4361d"), (4, figs.FOREST)):
    rgba[cls == k] = mc.to_rgba(col)
fig = plt.figure(figsize=(6.0, 6.0), dpi=150)
ax = fig.add_axes([0, 0, 1, 1])
ax.set_axis_off()
ax.imshow(rgba, interpolation="nearest")
ax.legend(handles=_clc.legend_patches([("Within 2% of the least cost", "#c4361d"), ("Within 5%", "#f6a54a"),
                                       ("Within 10%", "#fde0a6"), ("The two forests", figs.FOREST)]),
          loc="lower right", fontsize=8, frameon=True, framealpha=0.9)
fig.savefig(out / "corridor.png", dpi=150)
plt.close(fig)
figs._shrink(out / "corridor.png")
for lim in (2, 5, 10):
    print(f"cells within {lim}%:", int(((np.ma.filled(ex, 1e9) <= lim) & ~two).sum()))
print("least cost south - north-east:", mn)

lc = figs.read_csv(work / "output" / "least_costs.csv")
pts = [f["geometry"]["coordinates"] for f in feats]
rows = []
for i, r in enumerate(lc):
    for j in range(i + 1, len(lc)):
        e = float(np.hypot(pts[i][0] - pts[j][0], pts[i][1] - pts[j][1])) / 1000
        c = float(r[str(j + 1)]) / 1000
        rows.append({"pair": f"{feats[i]['properties']['name']} – {feats[j]['properties']['name']}",
                     "straight_km": round(e, 1), "least_cost": round(c, 1), "ratio": round(c / e, 1)})
for r in rows:
    print(r)
figs.write_csv(data / "least-costs.csv", rows)
for f in lines["features"]:
    if f["geometry"]:
        xy = np.array(f["geometry"]["coordinates"])
        print("path", f["properties"], "length km", round(float(np.sum(np.hypot(*np.diff(xy, axis=0).T))) / 1000, 1))
