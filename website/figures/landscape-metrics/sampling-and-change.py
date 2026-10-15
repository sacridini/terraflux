"""Figures of "Sampling, time series and redundancy": circles of four radii around six sites on the CORINE land
cover (metrics.sample), the metrics of a growing city along its steps (metrics.series), and the correlation of
landscape metrics over a grid of zones (metrics.correlation)."""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _lm as L  # noqa: E402
from _lm import figs, np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

SLUG = "landscape-metrics/sampling-and-change"
w = L.work("sampling-and-change")
img = figs.img_dir(SLUG)
dat = figs.data_dir(SLUG)

# ---------------------------------------------------------------- six sites, four radii
a, info = figs.read(L.CLC)
T = info["transform"]
SITES = {1: (1650, 760), 2: (1180, 700), 3: (640, 420), 4: (980, 1430), 5: (330, 1000), 6: (260, 1780)}
feats = []
for sid, (r, c) in SITES.items():
    x, y = T.c + (c + 0.5) * T.a, T.f + (r + 0.5) * T.e
    feats.append({"type": "Feature", "properties": {"site": sid}, "geometry": {"type": "Point", "coordinates": [x, y]}})
(w / "sites.geojson").write_text(json.dumps({"type": "FeatureCollection", "crs": {"type": "name", "properties": {
    "name": "urn:ogc:def:crs:EPSG::3035"}}, "features": feats}), encoding="utf-8")
RADII = [250, 500, 1000, 2000]
model = (L.HEAD + L.node("corine", "raster.read", params={"path": "clc.tif"})
         + L.node("cover", "raster.reclassify", {"map": "corine.map"}, {"rules": L.CLC7_RULES})
         + L.node("sites", "vector.read", params={"path": "sites.geojson"})
         + L.node("plots", "metrics.sample", {"map": "cover.map", "plots": "sites.vector"},
                  {"level": "class", "metrics": ["pland", "np", "ed"], "size": RADII, "shape": "circle", "id_column": "site"})
         + L.node("land", "metrics.sample", {"map": "cover.map", "plots": "sites.vector"},
                  {"level": "landscape", "metrics": ["shdi", "pr", "ed"], "size": RADII, "shape": "circle", "id_column": "site"})
         + L.node("save", "vector.write", {"vector": "plots.plots"}, {"path": "plots.geojson"}))
L.run(model, w, {"cover.map": "cover7.tif", "plots.table": "sample_class.csv", "land.table": "sample_land.csv"})
cls = L.table(w, "sample_class")
land = L.table(w, "sample_land")
print(cls[:3], land[:3])
forest = []
shdi = []
for rad in RADII:
    fr = {"radius_m": rad}
    sr = {"radius_m": rad}
    for sid in SITES:
        v = [r for r in cls if r["plot_id"] == sid and r["size"] == rad and r["class"] == 4]
        fr[f"site {sid}"] = round(v[0]["pland"], 2) if v else 0.0
        s = [r for r in land if r["plot_id"] == sid and r["size"] == rad][0]
        sr[f"site {sid}"] = round(s["shdi"], 3)
    forest.append(fr)
    shdi.append(sr)
    print(fr, sr)
figs.write_csv(dat / "forest_by_radius.csv", forest)
figs.write_csv(dat / "shdi_by_radius.csv", shdi)
print("percentage_inside", sorted({(int(r["size"]), round(r["percentage_inside"], 1)) for r in land if r["plot_id"] == 1}))

# the map with the circles
c7 = figs.read(w / "cover7.tif")[0].filled(0).astype(int)
lut = np.zeros((8, 3))
for k, (_, col) in L.CLC7.items():
    lut[k] = mcolors.to_rgb(col)
fig, ax = plt.subplots(figsize=(8.6, 6.6), dpi=150)
ext = (T.c, T.c + T.a * c7.shape[1], T.f + T.e * c7.shape[0], T.f)
ax.imshow(lut[c7], extent=ext, interpolation="nearest")
plots = json.loads((w / "plots.geojson").read_text(encoding="utf-8"))
for f in plots["features"]:
    ring = np.array(f["geometry"]["coordinates"][0])
    ax.plot(ring[:, 0], ring[:, 1], color="black", lw=0.9)
for sid, (r, c) in SITES.items():
    x, y = T.c + (c + 0.5) * T.a, T.f + (r + 0.5) * T.e
    ax.plot(x, y, "o", ms=3.5, color="black")
    ax.text(x + 2300, y + 1300, str(sid), fontsize=11, fontweight="bold", color="black",
            bbox=dict(boxstyle="round,pad=0.2", fc="white", ec="none", alpha=0.85))
ax.set_axis_off()
ax.legend(handles=[Patch(facecolor=col, label=n) for n, col in L.CLC7.values()], loc="center left",
          bbox_to_anchor=(1.01, 0.5), fontsize=9.5)
fig.tight_layout()
fig.savefig(img / "sites.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
plt.close(fig)
figs._shrink(img / "sites.png")

# ---------------------------------------------------------------- a forest cleared over six dates: metrics.series
# The forest (a fractal surface cut at 70 %) is cleared from the west, along a front with noise: at every date
# the forest cells whose clearing pressure is in the highest share become farmland. Six maps, written as the six
# bands of one GeoTIFF (a band per date, as maps of real dates are), then read back and measured.
R = {"rows": 200, "columns": 200, "resolution": 30}
DATES = [2000, 2005, 2010, 2015, 2020, 2025]
SHARES = [0.0, 0.12, 0.24, 0.36, 0.48, 0.60]
model = (L.HEAD + L.node("surface", "nlm.fbm", params={**R, "dimension": 1.5, "seed": 31})
         + L.node("forest", "nlm.binarize", {"map": "surface.map"}, {"breaks": 0.7})
         + L.node("front", "nlm.planar_gradient", params={**R, "direction": 270})
         + L.node("noise", "nlm.gaussian_field", params={**R, "range": 6, "seed": 32})
         + L.node("pressure", "nlm.merge", {"primary": "front.map", "secondary": "noise.map"}, {"scaling": 0.6}))
layers = []
for d, sh in zip(DATES, SHARES):
    if sh == 0:
        model += L.node(f"y{d}", "map.calc", {"f": "forest.map"}, {"expression": "if f == 2 then 1 else 2"})
    else:
        model += L.node(f"cut{d}", "nlm.binarize", {"map": "pressure.map"}, {"breaks": sh})
        model += L.node(f"y{d}", "map.calc", {"f": "forest.map", "c": f"cut{d}.map"},
                        {"expression": "if f == 2 and c == 1 then 1 else 2"})
    layers.append(f'y{d} = "y{d}.map"')
model += '\n[[nodes]]\nid = "dates"\nop = "stack.of"\ninputs = { ' + ", ".join(layers) + " }\n"
L.run(model, w, {"dates.stack": "forest_dates.tif"}, name="clearing.toml")
model = (L.HEAD + L.node("dates", "raster.read", params={"path": "forest_dates.tif"})
         + L.node("series", "metrics.series", {"maps": "dates.raster"},
                  {"level": "class", "metrics": ["pland", "np", "ed", "lpi", "area_mn", "ai", "enn_mn"],
                   "dates": DATES}))
L.run(model, w, {"series.table": "series.csv"}, name="series.toml")
ser = [r for r in L.table(w, "series") if int(r["class"]) == 1]
rows = [{"date": int(r["date"]), "pland": round(r["pland"], 2), "np": int(r["np"]), "lpi": round(r["lpi"], 2),
         "area_mn": round(r["area_mn"], 2), "ed": round(r["ed"], 2), "ai": round(r["ai"], 2),
         "enn_mn": round(r["enn_mn"], 1)} for r in ser]
figs.write_csv(dat / "forest_series.csv", rows)
for r in rows:
    print(r)
FOR = {1: ("Forest", figs.FOREST), 2: ("Farmland", "#e9dca8")}
for b, d in enumerate(DATES, start=1):
    figs.raster(w / "forest_dates.tif", img / f"forest_{d}.png", band=b, palette=FOR, legend=False, width=560)

# ---------------------------------------------------------------- redundancy: the metrics of 400 zones
MET = ["np", "pd", "te", "ed", "lsi", "lpi", "area_mn", "ai", "contag", "division", "mesh", "shdi", "sidi", "pr"]
model = (L.HEAD + L.node("corine", "raster.read", params={"path": "clc.tif"})
         + L.node("cover", "raster.reclassify", {"map": "corine.map"}, {"rules": L.CLC7_RULES})
         + L.node("zones", "vector.grid_zones", {"map": "corine.raster"}, {"size": 2500})
         + L.node("zm", "metrics.sample", {"map": "cover.map", "plots": "zones.zones"},
                  {"level": "landscape", "metrics": MET})
         + L.node("corr", "metrics.correlation", {"table": "zm.table"}, {"method": "spearman"}))
L.run(model, w, {"zm.table": "zones.csv", "corr.table": "corr.csv"}, name="zones.toml")
zt = L.table(w, "zones")
print("zones", len(zt), "contag NA", sum(1 for r in zt if np.isnan(r["contag"])), "pr==1", sum(1 for r in zt if r["pr"] == 1))
cm = np.array([[r[k] for k in MET] for r in L.table(w, "corr")])
print(np.round(cm, 2))
pairs = []
for i in range(len(MET)):
    for j in range(i + 1, len(MET)):
        if not np.isnan(cm[i, j]):
            pairs.append(abs(cm[i, j]))
pairs = np.array(pairs)
print("pairs", len(pairs), "|r|>0.7", int((pairs > 0.7).sum()), "|r|>0.9", int((pairs > 0.9).sum()))
fig, ax = plt.subplots(figsize=(7.4, 6.4), dpi=150)
im = ax.imshow(np.ma.masked_invalid(cm), cmap="RdBu_r", vmin=-1, vmax=1)
ax.set_xticks(range(len(MET)), MET, rotation=90)
ax.set_yticks(range(len(MET)), MET)
ax.grid(False)
for i in range(len(MET)):
    for j in range(len(MET)):
        v = cm[i, j]
        ax.text(j, i, "NA" if np.isnan(v) else f"{v:.2f}".replace("0.", ".").replace("-.", "-."), ha="center",
                va="center", fontsize=6.6, color="white" if (not np.isnan(v) and abs(v) > 0.6) else figs.INK)
cb = fig.colorbar(im, ax=ax, fraction=0.046, pad=0.03)
cb.outline.set_visible(False)
cb.set_label("Spearman's correlation", color=figs.INK2)
fig.tight_layout()
fig.savefig(img / "correlation.png", dpi=150)
plt.close(fig)
figs._shrink(img / "correlation.png")

# the sites and the six dates, for the readers to download
import shutil  # noqa: E402

shutil.copy(w / "sites.geojson", dat / "sites.geojson")
shutil.copy(w / "forest_dates.tif", dat / "forest_dates.tif")
