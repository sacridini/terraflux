"""Figures of "Patch and class metrics": three toy patches of the same area and their shape metrics, the patch
metrics of the Augusta land cover as maps, the core area as the edge deepens, aggregation at a constant amount,
and the class metrics of Augusta."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _lm as L  # noqa: E402
from _lm import figs, np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402

SLUG = "landscape-metrics/patch-and-class-metrics"
w = L.work("patch-and-class-metrics")
img = figs.img_dir(SLUG)
dat = figs.data_dir(SLUG)

# ---------------------------------------------------------------- three patches of 24 cells: compact, long, contorted
toy = np.ones((12, 30), dtype="uint8")
toy[3:9, 2:6] = 2                    # a compact block of 6 x 4
toy[5:7, 9:21] = 3                   # a strip of 2 x 12
# a spiral: 6 + 6 + 5 + 3 + 3 + 1 cells
contorted = ([(2, x) for x in range(23, 29)] + [(y, 28) for y in range(3, 9)] + [(8, x) for x in range(23, 28)]
             + [(y, 23) for y in range(5, 8)] + [(5, x) for x in range(24, 27)] + [(6, 26)])
for y, x in contorted:
    toy[y, x] = 4
L.write_tif(w / "shapes.tif", toy, res=30.0)
SHAPE_M = ["area", "perim", "para", "shape", "frac", "circle", "gyrate", "core", "cai"]
model = L.HEAD + L.node("toy", "raster.read", params={"path": "shapes.tif"})
model += L.node("patches", "metrics.patch", {"map": "toy.map"}, {"metrics": SHAPE_M, "show": "shape"})
L.run(model, w, {"patches.table": "shapes.csv", "patches.patches": "shapes_ids.tif"})
shapes = {int(r["class"]): r for r in L.table(w, "shapes") if int(r["class"]) > 1}
assert len([r for r in L.table(w, "shapes") if int(r["class"]) > 1]) == 3
for k, r in shapes.items():
    print("shape", k, {m: round(r[m], 3) for m in SHAPE_M})
fig, ax = plt.subplots(figsize=(10, 4.6), dpi=150)
cols = {1: "#f4f1e8", 2: figs.FOREST, 3: figs.FOREST, 4: figs.FOREST}
rgb = np.array([mcolors.to_rgb(cols[v]) for v in toy.ravel()]).reshape(toy.shape + (3,))
ax.imshow(rgb, interpolation="nearest")
ax.set_xticks(np.arange(-0.5, toy.shape[1], 1))
ax.set_yticks(np.arange(-0.5, toy.shape[0], 1))
ax.grid(True, color="white", linestyle="-", linewidth=0.6)
ax.tick_params(length=0, labelbottom=False, labelleft=False)
for s in ax.spines.values():
    s.set_visible(False)
for k, x in ((2, 3.5), (3, 14.5), (4, 25.5)):
    r = shapes[k]
    txt = (f"area {r['area']:.2f} ha\nperimeter {r['perim']:.0f} m\nshape index {r['shape']:.2f}\n"
           f"fractal dim. {r['frac']:.3f}\ncore area {r['core']:.2f} ha\ncore area index {r['cai']:.0f} %")
    ax.text(x, 11.9, txt, ha="center", va="top", fontsize=8.6, color=figs.INK, linespacing=1.3)
ax.set_ylim(17.0, -0.5)
fig.tight_layout()
fig.savefig(img / "shapes.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
plt.close(fig)
figs._shrink(img / "shapes.png")

# ---------------------------------------------------------------- the patch metrics of Augusta as maps
model = L.HEAD + L.node("cover", "raster.read", params={"path": "augusta.tif"})
SHOWS = {"area": "area", "shape": "shape", "frac": "frac", "cai": "cai", "enn": "enn", "gyrate": "gyrate"}
for m in SHOWS:
    model += L.node(f"p_{m}", "metrics.patch", {"map": "cover.map"},
                    {"metrics": ["area", "perim", "shape", "frac", "core", "cai", "enn", "gyrate"], "show": m})
writes = {f"p_{m}.map": f"aug_{m}.tif" for m in SHOWS}
writes["p_area.table"] = "aug_patches.csv"
L.run(model, w, writes)
figs.raster(w / "aug_shape.tif", img / "aug_shape.png", cmap="magma", unit="shape index", width=640, vmin=1, vmax=4)
figs.raster(w / "aug_frac.tif", img / "aug_frac.png", cmap="magma", unit="fractal dimension", width=640, vmin=1, vmax=1.3)
figs.raster(w / "aug_cai.tif", img / "aug_cai.png", cmap="viridis", unit="core area index (%)", width=640)
figs.raster(w / "aug_enn.tif", img / "aug_enn.png", cmap="plasma", log=True, unit="nearest neighbour (m)", width=640)
figs.raster(w / "aug_gyrate.tif", img / "aug_gyrate.png", cmap="viridis", unit="radius of gyration (m)", width=640)
pt = L.table(w, "aug_patches")
print("patches", len(pt))
areas = np.array([r["area"] for r in pt])
print("area: median", np.median(areas), "mean", areas.mean(), "share of cells in 10 largest", np.sort(areas)[-10:].sum() / areas.sum())
ev = [r for r in pt if int(r["class"]) == 42]
print("evergreen patches", len(ev), "enn median", np.nanmedian([r["enn"] for r in ev]))
# patch table excerpt: the 8 largest evergreen patches
ev.sort(key=lambda r: -r["area"])
figs.write_csv(dat / "evergreen_patches.csv", [{k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()} for r in ev])
for r in ev[:6]:
    print({k: round(v, 3) for k, v in r.items()})
# area against shape, every patch of the forests (a chart)
rows = []
for r in pt:
    c = int(r["class"])
    if c in (41, 42, 43) and r["area"] >= 0.18:
        rows.append({"log10_area": round(float(np.log10(r["area"])), 3),
                     L.NLCD[c][0]: round(r["shape"], 3)})
rows.sort(key=lambda r: r["log10_area"])
keys = [L.NLCD[c][0] for c in (41, 42, 43)]
figs.write_csv(dat / "area_shape.csv", [{"log10_area": r["log10_area"], **{k: r.get(k, "") for k in keys}} for r in rows])

# ---------------------------------------------------------------- the core as the edge deepens
frames = []
crow = []
for d in range(1, 7):
    model = L.HEAD + L.node("cover", "raster.read", params={"path": "augusta.tif"})
    model += L.node("patches", "metrics.patch", {"map": "cover.map"}, {"metrics": ["core", "cai"], "edge_depth": d, "show": "cai"})
    model += L.node("classes", "metrics.class", {"map": "cover.map"},
                    {"metrics": ["tca", "cpland", "ndca", "pland"], "edge_depth": d, "show": "cpland"})
    L.run(model, w, {"patches.map": f"cai_{d}.tif", "classes.table": f"core_{d}.csv"}, name=f"core_{d}.toml")
    figs.raster(w / f"cai_{d}.tif", img / f"cai_{d}.png", cmap="viridis", vmin=0, vmax=100, unit="core area index (%)", width=640)
    c = {int(r["class"]): r for r in L.table(w, f"core_{d}")}
    row = {"depth_m": d * 30}
    for k, name in ((42, "evergreen"), (41, "deciduous"), (43, "mixed")):
        row[name] = round(c[k]["cpland"], 3)
    row["evergreen_ndca"] = int(c[42]["ndca"])
    crow.append(row)
    print(row)
figs.write_csv(dat / "core_depth.csv", crow)

# ---------------------------------------------------------------- aggregation at a constant amount of habitat
HAB = {1: ("Matrix", "#ece6d6"), 2: ("Habitat", figs.FOREST)}
agg = []
for dim in (0.4, 0.9, 1.4, 1.9):
    tag = str(dim).replace(".", "_")
    model = L.HEAD + L.node("surface", "nlm.fbm", params={"rows": 160, "columns": 160, "resolution": 30, "dimension": dim, "seed": 21})
    model += L.node("habitat", "nlm.binarize", {"map": "surface.map"}, {"breaks": 0.35})
    model += L.node("m", "metrics.class", {"map": "habitat.map"},
                    {"metrics": ["pland", "np", "ed", "lpi", "ai", "clumpy", "cohesion", "pladj", "enn_mn"], "show": "ai"})
    L.run(model, w, {"habitat.map": f"agg_{tag}.tif", "m.table": f"agg_{tag}.csv"}, name=f"agg_{tag}.toml")
    r = [x for x in L.table(w, f"agg_{tag}") if x["class"] == 2][0]
    agg.append({"dimension": dim, **{k: round(v, 2) for k, v in r.items() if k != "class"}})
    figs.raster(w / f"agg_{tag}.tif", img / f"agg_{tag}.png", palette=HAB, legend=False, width=420)
for a in agg:
    print(a)
figs.write_csv(dat / "aggregation.csv", agg)

# ---------------------------------------------------------------- the class metrics of Augusta
CM = ["ca", "pland", "np", "lpi", "area_mn", "area_am", "shape_mn", "shape_am", "ed", "ai", "cohesion", "enn_mn", "cai_mn"]
model = L.HEAD + L.node("cover", "raster.read", params={"path": "augusta.tif"})
model += L.node("classes", "metrics.class", {"map": "cover.map"}, {"metrics": CM, "show": "ai"})
L.run(model, w, {"classes.table": "aug_classes.csv", "classes.map": "aug_ai.tif"})
cls = L.table(w, "aug_classes")
out = []
for r in cls:
    c = int(r["class"])
    out.append({"class": c, "name": L.NLCD[c][0], **{k: round(r[k], 3) for k in CM}})
    print(out[-1])
figs.write_csv(dat / "augusta_classes.csv", out)
figs.raster(w / "aug_ai.tif", img / "aug_ai.png", cmap="viridis", unit="aggregation index (%)", width=640)
# AI against PLAND, a series per group of classes (a scatter)
groups = {"Forest": (41, 42, 43), "Developed": (21, 22, 23, 24), "Open and farmed": (31, 52, 71, 81, 82),
          "Water and wetlands": (11, 90, 95)}
sc = []
for r in sorted(out, key=lambda r: r["pland"]):
    row = {"pland": r["pland"]}
    for g, cs in groups.items():
        row[g] = r["ai"] if r["class"] in cs and not np.isnan(r["ai"]) else ""
    row["class"] = r["name"]
    sc.append(row)
figs.write_csv(dat / "ai_pland.csv", sc)
