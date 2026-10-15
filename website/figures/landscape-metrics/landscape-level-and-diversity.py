"""Figures of "Landscape-level metrics and diversity": four landscapes of the same classes and their diversity and
contagion, diversity as one class comes to dominate, the landscape metrics of Augusta, and the same map with a
coarser legend."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _lm as L  # noqa: E402
from _lm import figs  # noqa: E402

SLUG = "landscape-metrics/landscape-level-and-diversity"
w = L.work("landscape-level-and-diversity")
img = figs.img_dir(SLUG)
dat = figs.data_dir(SLUG)

FOUR = {1: ("Forest", figs.FOREST), 2: ("Farming", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Urban", "#b4508a")}
LM = ["pr", "shdi", "shei", "sidi", "siei", "msidi", "ed", "lpi", "contag", "iji", "ai", "ent", "condent", "mutinf",
      "relmutinf"]
R = {"rows": 150, "columns": 150, "resolution": 30}

# ---------------------------------------------------------------- four landscapes
cases = {
    "dominant": L.node("cover", "nlm.random_cluster", params={**R, "p": 0.5, "weights": [0.82, 0.06, 0.06, 0.06], "seed": 5}),
    "salt": L.node("s", "nlm.random", params={**R, "seed": 5})
    + L.node("cover", "nlm.classify", {"map": "s.map"}, {"weights": [0.25, 0.25, 0.25, 0.25]}),
    "clusters": L.node("cover", "nlm.random_cluster", params={**R, "p": 0.5, "weights": [0.25, 0.25, 0.25, 0.25], "seed": 5}),
    "smooth": L.node("s", "nlm.gaussian_field", params={**R, "range": 25, "seed": 5})
    + L.node("cover", "nlm.classify", {"map": "s.map"}, {"weights": [0.25, 0.25, 0.25, 0.25]}),
}
rows = []
for k, nodes in cases.items():
    port = "cover.map" if "random_cluster" in nodes and "classify" not in nodes else "cover.classes"
    model = L.HEAD + nodes + L.node("m", "metrics.landscape", {"map": port}, {"metrics": LM})
    L.run(model, w, {port: f"four_{k}.tif", "m.table": f"four_{k}.csv"}, name=f"four_{k}.toml")
    t = L.table(w, f"four_{k}")[0]
    rows.append({"landscape": k, **{m: round(t[m], 3) for m in LM}})
    figs.raster(w / f"four_{k}.tif", img / f"four_{k}.png", palette=FOUR, legend=False, width=420)
    print(rows[-1])
figs.write_csv(dat / "four_landscapes.csv", rows)

# ---------------------------------------------------------------- diversity as one class comes to dominate
drows = []
for share in (0.25, 0.35, 0.45, 0.55, 0.65, 0.75, 0.85, 0.95):
    rest = round((1 - share) / 3, 4)
    model = L.HEAD + L.node("cover", "nlm.random_cluster", params={**R, "p": 0.5, "weights": [share, rest, rest, rest], "seed": 5})
    model += L.node("m", "metrics.landscape", {"map": "cover.map"}, {"metrics": ["pr", "shdi", "shei", "sidi", "siei"]})
    model += L.node("c", "metrics.class", {"map": "cover.map"}, {"metrics": ["pland"], "show": "pland"})
    L.run(model, w, {"m.table": "dom.csv", "c.table": "domc.csv"}, name="dom.toml")
    t = L.table(w, "dom")[0]
    c = {int(r["class"]): r["pland"] for r in L.table(w, "domc")}
    drows.append({"forest": round(c[1], 1), **{m: round(t[m], 3) for m in ("shdi", "shei", "sidi", "siei")}})
    print(drows[-1])
figs.write_csv(dat / "dominance.csv", drows)

# ---------------------------------------------------------------- Augusta: the landscape level, two legends
ALL = ["ta", "np", "pd", "lpi", "te", "ed", "area_mn", "area_am", "shape_mn", "pr", "shdi", "shei", "sidi", "siei",
       "contag", "iji", "ai", "mesh", "division", "split", "ent", "condent", "mutinf"]
model = L.HEAD + L.node("cover", "raster.read", params={"path": "augusta.tif"})
model += L.node("level1", "raster.reclassify", {"map": "cover.map"}, {"rules": L.NLCD1_RULES})
model += L.node("m15", "metrics.landscape", {"map": "cover.map"}, {"metrics": ALL})
model += L.node("m8", "metrics.landscape", {"map": "level1.map"}, {"metrics": ALL})
L.run(model, w, {"level1.map": "aug_level1.tif", "m15.table": "aug_15.csv", "m8.table": "aug_8.csv"})
t15, t8 = L.table(w, "aug_15")[0], L.table(w, "aug_8")[0]
figs.write_csv(dat / "augusta_landscape.csv",
               [{"metric": m, "nlcd_15": round(t15[m], 3), "level1_8": round(t8[m], 3)} for m in ALL])
for m in ALL:
    print(m, round(t15[m], 3), round(t8[m], 3))
figs.raster(L.AUGUSTA, img / "aug_15.png", palette=L.NLCD, legend=False, width=900)
figs.raster(w / "aug_level1.tif", img / "aug_8.png", palette=L.NLCD1, legend=False, width=900)
figs.raster(w / "aug_level1.tif", img / "aug_8_legend.png", palette=L.NLCD1, counts=True, width=900)
