"""Figures of "What landscape metrics are": a toy map and its patches by 4 and 8 neighbours, composition versus
configuration, the Augusta land cover at its three levels, and the same map at coarser grains."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _lm as L  # noqa: E402
from _lm import figs, np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402

SLUG = "landscape-metrics/what-are-landscape-metrics"
w = L.work("what-are-landscape-metrics")
img = figs.img_dir(SLUG)
dat = figs.data_dir(SLUG)

# ---------------------------------------------------------------- a toy map: its patches by 8 and by 4 neighbours
toy = np.array([
    [1, 1, 1, 2, 2, 2, 3, 3],
    [1, 1, 2, 2, 2, 3, 3, 3],
    [1, 1, 1, 2, 1, 3, 3, 1],
    [3, 1, 1, 1, 2, 1, 3, 1],
    [3, 3, 1, 2, 2, 2, 1, 1],
    [3, 3, 2, 2, 3, 2, 1, 1],
    [2, 2, 2, 3, 3, 1, 1, 1],
    [2, 2, 1, 3, 3, 1, 1, 1],
], dtype="uint8")
L.write_tif(w / "toy.tif", toy, res=10.0)
model = L.HEAD + L.node("toy", "raster.read", params={"path": "toy.tif"})
for d in (8, 4):
    model += L.node(f"p{d}", "metrics.patch", {"map": "toy.map"}, {"metrics": ["area", "perim"], "directions": d, "show": "area"})
model += L.node("c8", "metrics.class", {"map": "toy.map"}, {"metrics": ["ca", "pland", "np"], "show": "pland"})
model += L.node("c4", "metrics.class", {"map": "toy.map"}, {"metrics": ["ca", "pland", "np"], "directions": 4, "show": "pland"})
L.run(model, w, {"p8.patches": "p8.tif", "p4.patches": "p4.tif", "p8.table": "toy_p8.csv", "p4.table": "toy_p4.csv",
                 "c8.table": "toy_c8.csv", "c4.table": "toy_c4.csv"})
TOY = {1: ("Forest", figs.FOREST), 2: ("Farming", "#d9a441"), 3: ("Water", "#3f7fae")}
p8 = figs.read(w / "p8.tif")[0].filled(0).astype(int)
p4 = figs.read(w / "p4.tif")[0].filled(0).astype(int)
fig, axes = plt.subplots(1, 3, figsize=(10.2, 3.75), dpi=150)
for ax, (title, ids) in zip(axes, [("The map: 3 classes", None), ("Patches, 8 neighbours", p8),
                                   ("Patches, 4 neighbours", p4)]):
    rgb = np.array([mcolors.to_rgb(TOY[v][1]) for v in toy.ravel()]).reshape(8, 8, 3)
    ax.imshow(rgb, interpolation="nearest")
    ax.set_xticks(np.arange(-0.5, 8, 1), minor=False)
    ax.set_yticks(np.arange(-0.5, 8, 1), minor=False)
    ax.grid(True, color="white", linestyle="-", linewidth=0.8)
    ax.tick_params(length=0, labelbottom=False, labelleft=False)
    for s in ax.spines.values():
        s.set_visible(False)
    ax.set_title(title, loc="left", fontsize=11)
    if ids is not None:
        for y in range(8):
            for x in range(8):
                ax.text(x, y, str(ids[y, x]), ha="center", va="center", fontsize=9, color="white", fontweight="bold")
        # outline every patch
        for y in range(8):
            for x in range(8):
                if x < 7 and ids[y, x] != ids[y, x + 1]:
                    ax.plot([x + 0.5, x + 0.5], [y - 0.5, y + 0.5], color=figs.INK, lw=1.6)
                if y < 7 and ids[y, x] != ids[y + 1, x]:
                    ax.plot([x - 0.5, x + 0.5], [y + 0.5, y + 0.5], color=figs.INK, lw=1.6)
from matplotlib.patches import Patch  # noqa: E402
fig.legend(handles=[Patch(facecolor=c, label=n) for n, c in TOY.values()], loc="lower center", ncol=3,
           bbox_to_anchor=(0.5, -0.01), fontsize=10)
fig.tight_layout(rect=(0, 0.07, 1, 0.97))
fig.savefig(img / "toy_patches.png", dpi=150)
plt.close(fig)
figs._shrink(img / "toy_patches.png")
print("toy patches 8:", int(p8.max()), "4:", int(p4.max()))
for d in (8, 4):
    print(d, L.table(w, f"toy_c{d}"))

# ---------------------------------------------------------------- composition and configuration: the same 40 % forest
model = L.HEAD
model += L.node("clumped_s", "nlm.fbm", params={"rows": 150, "columns": 150, "resolution": 30, "dimension": 1.7, "seed": 3})
model += L.node("random_s", "nlm.random", params={"rows": 150, "columns": 150, "resolution": 30, "seed": 3})
model += L.node("clumped", "nlm.binarize", {"map": "clumped_s.map"}, {"breaks": 0.4})
model += L.node("random", "nlm.binarize", {"map": "random_s.map"}, {"breaks": 0.4})
for k in ("clumped", "random"):
    model += L.node(f"m_{k}", "metrics.class", {"map": f"{k}.map"}, {"metrics": ["pland", "np", "ed", "area_mn", "ai"], "show": "pland"})
L.run(model, w, {"clumped.map": "clumped.tif", "random.map": "random.tif", "m_clumped.table": "cc_clumped.csv",
                 "m_random.table": "cc_random.csv"})
HAB = {1: ("Matrix", "#ece6d6"), 2: ("Forest", figs.FOREST)}
for k in ("clumped", "random"):
    t = [r for r in L.table(w, f"cc_{k}") if r["class"] == 2][0]
    print(k, t)
    figs.raster(w / f"{k}.tif", img / f"cc_{k}.png", palette=HAB, legend=False, width=520)

# ---------------------------------------------------------------- Augusta: the map and its three levels
model = L.HEAD + L.node("cover", "raster.read", params={"path": "augusta.tif"})
model += L.node("patches", "metrics.patch", {"map": "cover.map"}, {"metrics": ["area"], "show": "area"})
model += L.node("classes", "metrics.class", {"map": "cover.map"}, {"metrics": ["ca", "pland", "np"], "show": "pland"})
model += L.node("landscape", "metrics.landscape", {"map": "cover.map"}, {"metrics": ["ta", "np", "pr", "shdi"]})
L.run(model, w, {"patches.patches": "aug_patches.tif", "patches.map": "aug_area.tif", "classes.map": "aug_pland.tif",
                 "patches.table": "aug_patch.csv", "classes.table": "aug_class.csv",
                 "landscape.table": "aug_land.csv"})
figs.raster(L.AUGUSTA, img / "augusta.png", palette=L.NLCD, counts=True, width=1000)
pa = figs.read(w / "aug_patches.tif")[0]
rng = np.random.default_rng(1)
n = int(pa.max())
# figs.raster colours at most 255 classes: every patch gets one of 20 colours, picked at random per patch
perm = rng.integers(1, 21, size=n + 1)
ids = pa.filled(0).astype(int)
col = np.where(ids > 0, perm[ids], 0).astype("float32")
pal = {i: (str(i), mcolors.to_hex(plt.get_cmap("tab20")(i - 1))) for i in range(1, 21)}
figs.raster(w / "aug_patches.tif", img / "aug_patches.png", palette=pal, legend=False, width=640,
            array=np.ma.masked_equal(col, 0))
figs.raster(w / "aug_area.tif", img / "aug_area.png", cmap="viridis", log=True, unit="patch area (ha)", width=640)
figs.raster(w / "aug_pland.tif", img / "aug_pland.png", cmap="cividis", unit="share of the map (%)", width=640)
print("augusta landscape", L.table(w, "aug_land"))
cls = L.table(w, "aug_class")
for r in cls:
    print(int(r["class"]), L.NLCD[int(r["class"])][0], r)
pt = L.table(w, "aug_patch")
print("patches", len(pt), "largest", max(r["area"] for r in pt), "single cells", sum(1 for r in pt if r["area"] < 0.1))

# ---------------------------------------------------------------- grain: the same map at coarser cells
rows = []
frames = []
for res in (30, 60, 90, 150, 240):
    model = L.HEAD + L.node("cover", "raster.read", params={"path": "augusta.tif"})
    src = "cover.raster"
    if res != 30:
        model += L.node("coarse", "raster.resample", {"raster": "cover.raster"},
                        {"resolution": res, "resampling": "mode"})
        src = "coarse.raster"
    model += L.node("land", "metrics.landscape", {"map": src}, {"metrics": ["np", "ed", "pr", "shdi", "lpi"]})
    model += L.node("forest", "metrics.class", {"map": src}, {"metrics": ["pland", "np", "area_mn"], "show": "pland"})
    writes = {"land.table": f"grain_land_{res}.csv", "forest.table": f"grain_class_{res}.csv"}
    if res != 30:
        writes["coarse.raster"] = f"grain_{res}.tif"
    L.run(model, w, writes, name=f"grain_{res}.toml")
    t = L.table(w, f"grain_land_{res}")[0]
    c = {int(r["class"]): r for r in L.table(w, f"grain_class_{res}")}
    rows.append({"cell": res, "np": int(t["np"]), "ed": round(t["ed"], 1), "pr": int(t["pr"]),
                 "shdi": round(t["shdi"], 3), "evergreen_mn": round(c[42]["area_mn"], 2)})
    tif = L.AUGUSTA if res == 30 else w / f"grain_{res}.tif"
    figs.raster(tif, img / f"grain_{res}.png", palette=L.NLCD, legend=False, width=640)
    frames.append(t)
figs.write_csv(dat / "grain.csv", rows)
for r in rows:
    print(r)

# the map itself, for the readers to download
import shutil  # noqa: E402

shutil.copy(L.AUGUSTA, dat / "augusta.tif")
