"""Figures of "Neutral landscapes in a study": a factorial experiment (habitat amount x fragmentation x 10
replicates) measuring how class metrics respond, and a null-model test of the aggregation of the evergreen forest
of Augusta against random and autocorrelated neutral landscapes with the same amount, plus a neutral Augusta drawn
with the real map's class shares."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _nlm as N  # noqa: E402
from _nlm import figs, np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402

SLUG = "neutral-landscapes/neutral-landscapes-in-a-study"
w = N.work("neutral-landscapes-in-a-study")
img = figs.img_dir(SLUG)
dat = figs.data_dir(SLUG)

R = {"rows": 200, "columns": 200, "resolution": 30}
SHARES = [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9]
DIMS = {0.5: "fragmented", 1.0: "intermediate", 1.5: "aggregated"}
SEEDS = list(range(1, 11))
MET = ["pland", "np", "ed", "lpi", "ai", "cohesion", "enn_mn", "area_mn"]

# ---------------------------------------------------------------- the factorial experiment
res = {}   # (dim, seed) -> list of rows (one per share)
for dim in DIMS:
    for s in SEEDS:
        model = (N.HEAD + N.node("surface", "nlm.fbm", params={**R, "dimension": dim, "seed": s})
                 + N.node("habitat", "nlm.binarize", {"map": "surface.map"}, {"breaks": SHARES})
                 + N.node("metrics", "metrics.signatures", {"maps": "habitat.map"},
                          {"class_metrics": MET, "classes": [2]}))
        tag = f"{str(dim).replace('.', '_')}_{s}"
        writes = {"metrics.table": f"f_{tag}.csv"}
        if s == 1:
            writes["habitat.map"] = f"hab_{str(dim).replace('.', '_')}.tif"
        N.run(model, w, writes, name="factorial.toml")
        res[(dim, s)] = N.table(w, f"f_{tag}")
print(list(res[(0.5, 1)][0].keys()))
print(res[(0.5, 1)][0])
summary = {m: [] for m in MET}
spread = []
for i, sh in enumerate(SHARES):
    for m in MET:
        row = {"habitat": int(sh * 100)}
        for dim, name in DIMS.items():
            vals = [res[(dim, s)][i][f"{m}_2"] for s in SEEDS]
            row[name] = round(float(np.nanmean(vals)), 3)
            row[f"{name}_sd"] = round(float(np.nanstd(vals)), 3)
        summary[m].append(row)
for m in MET:
    figs.write_csv(dat / f"factorial_{m}.csv", summary[m])
    print(m, summary[m][2], summary[m][6])
# every replicate of np (a scatter): a row per replicate, its share as x
rows = []
for i, sh in enumerate(SHARES):
    for s in SEEDS:
        row = {"habitat": int(sh * 100) + (s - 5.5) * 0.35}
        for dim, name in DIMS.items():
            row[name] = res[(dim, s)][i]["np_2"]
        rows.append(row)
figs.write_csv(dat / "replicates_np.csv", [{k: (round(v, 2) if isinstance(v, float) else v) for k, v in r.items()} for r in rows])

# the design as a figure: 3 levels of fragmentation x 3 amounts (seed 1)
fig, axes = plt.subplots(3, 3, figsize=(7.8, 8.1), dpi=150)
from matplotlib import colors as mcolors  # noqa: E402
cm = mcolors.ListedColormap(["#ece6d6", figs.FOREST])
for r, (dim, name) in enumerate(DIMS.items()):
    for c, band in enumerate((2, 5, 8)):          # 20, 50 and 80 % habitat
        a = figs.read(w / f"hab_{str(dim).replace('.', '_')}.tif", band)[0]
        ax = axes[r, c]
        ax.imshow(a, cmap=cm, vmin=1, vmax=2, interpolation="nearest")
        ax.set_xticks([])
        ax.set_yticks([])
        ax.grid(False)
        for sp in ax.spines.values():
            sp.set_visible(False)
        if r == 0:
            ax.set_title(f"{SHARES[band - 1] * 100:.0f} % habitat", fontsize=11)
        if c == 0:
            ax.set_ylabel(f"{name}\n(dimension {dim})", fontsize=10.5, color=figs.INK)
fig.tight_layout()
fig.savefig(img / "design.png", dpi=150)
plt.close(fig)
figs._shrink(img / "design.png")

# ---------------------------------------------------------------- a null-model test: Augusta's evergreen forest
model = (N.HEAD + N.node("cover", "raster.read", params={"path": "augusta.tif"})
         + N.node("forest", "raster.reclassify", {"map": "cover.map"}, {"rules": "42 -> 2\n* -> 1"})
         + N.node("m", "metrics.class", {"map": "forest.map"}, {"metrics": ["pland", "np", "ed", "ai", "lpi"], "show": "ai"}))
N.run(model, w, {"m.table": "observed.csv", "forest.map": "evergreen.tif"}, name="observed.toml")
obs = [r for r in N.table(w, "observed") if r["class"] == 2][0]
share = obs["pland"] / 100
print("observed", obs)
NULLS = {"random": ("nlm.random", {}), "fbm10": ("nlm.fbm", {"dimension": 1.0}),
         "fbm15": ("nlm.fbm", {"dimension": 1.5})}
null = {k: [] for k in NULLS}
for k, (op, p) in NULLS.items():
    model = N.HEAD + N.node("cover", "raster.read", params={"path": "augusta.tif"})
    for s in range(1, 20):
        model += N.node(f"n{s}", op, {"like": "cover.map"}, {**p, "seed": s})
        model += N.node(f"h{s}", "nlm.binarize", {"map": f"n{s}.map"}, {"breaks": round(share, 4)})
        model += N.node(f"m{s}", "metrics.class", {"map": f"h{s}.map"}, {"metrics": ["pland", "np", "ed", "ai", "lpi"], "show": "ai"})
    writes = {f"m{s}.table": f"null_{k}_{s}.csv" for s in range(1, 20)}
    writes["h1.map"] = f"null_{k}.tif"
    N.run(model, w, writes, name=f"null_{k}.toml")
    for s in range(1, 20):
        null[k].append([r for r in N.table(w, f"null_{k}_{s}") if r["class"] == 2][0])
    ai = [r["ai"] for r in null[k]]
    print(k, "pland", round(float(np.mean([r["pland"] for r in null[k]])), 2), "ai", round(min(ai), 2), round(max(ai), 2),
          "np", min(r["np"] for r in null[k]), max(r["np"] for r in null[k]))
rows = [{"model": "observed", "ai": round(obs["ai"], 2), "np": int(obs["np"]), "ed": round(obs["ed"], 2), "pland": round(obs["pland"], 2)}]
for k in NULLS:
    for r in null[k]:
        rows.append({"model": k, "ai": round(r["ai"], 2), "np": int(r["np"]), "ed": round(r["ed"], 2), "pland": round(r["pland"], 2)})
figs.write_csv(dat / "null_test.csv", rows)
# the strip chart
labels = {"random": "random\n(nlm.random)", "fbm10": "fractal, rough\n(fbm, 1.0)", "fbm15": "fractal, smooth\n(fbm, 1.5)"}
fig, axes = plt.subplots(1, 2, figsize=(9, 3.9), dpi=150)
for ax, m, lab in ((axes[0], "ai", "aggregation index (%)"), (axes[1], "np", "number of patches")):
    for i, k in enumerate(NULLS):
        v = [r[m] for r in null[k]]
        ax.scatter(np.full(len(v), i) + np.linspace(-0.18, 0.18, len(v)), v, s=16, color=figs.CLASSES[i], alpha=0.85)
    ax.axhline(obs[m], color=figs.EMBER, lw=2)
    ax.text(len(NULLS) - 0.5, obs[m], " observed", color=figs.EMBER, va="bottom", ha="right", fontsize=10)
    ax.set_xticks(range(len(NULLS)), [labels[k] for k in NULLS], fontsize=8.5)
    ax.set_ylabel(lab)
    if m == "np":
        ax.set_yscale("log")
fig.tight_layout()
fig.savefig(img / "null_test.png", dpi=150)
plt.close(fig)
figs._shrink(img / "null_test.png")
figs.raster(w / "evergreen.tif", img / "evergreen.png", palette={1: ("Other", "#ece6d6"), 2: ("Evergreen forest", "#1c5f2c")},
            legend=False, width=560)
for k in NULLS:
    figs.raster(w / f"null_{k}.tif", img / f"null_{k}.png", palette={1: ("Other", "#ece6d6"), 2: ("Habitat", "#1c5f2c")},
                legend=False, width=560)

# ---------------------------------------------------------------- a neutral Augusta: a Gaussian field with the real shares
NLCD_VALUES = [11, 21, 22, 23, 24, 31, 41, 42, 43, 52, 71, 81, 82, 90, 95]
NLCD_COL = {11: "#466b9f", 21: "#dec5c5", 22: "#d99282", 23: "#eb0000", 24: "#ab0000", 31: "#b3ac9f", 41: "#68ab5f",
            42: "#1c5f2c", 43: "#b5c58f", 52: "#ccb879", 71: "#dfdfc2", 81: "#dcd939", 82: "#ab6c28", 90: "#b8d9eb",
            95: "#6c9fb8"}
model = (N.HEAD + N.node("cover", "raster.read", params={"path": "augusta.tif"})
         + N.node("field", "nlm.gaussian_field", {"like": "cover.map"}, {"range": 8, "seed": 7})
         + N.node("neutral", "nlm.classify", {"map": "field.map", "real": "cover.map"})
         + N.node("mr", "metrics.landscape", {"map": "cover.map"}, {"metrics": ["np", "ed", "shdi", "contag"]})
         + N.node("mn", "metrics.landscape", {"map": "neutral.classes"}, {"metrics": ["np", "ed", "shdi", "contag"]}))
N.run(model, w, {"neutral.classes": "neutral_augusta.tif", "mr.table": "real_land.csv", "mn.table": "neutral_land.csv"}, name="neutral.toml")
a = figs.read(w / "neutral_augusta.tif")[0]
v, c = np.unique(a.compressed(), return_counts=True)
print("neutral classes", dict(zip(v.astype(int).tolist(), np.round(c / c.sum() * 100, 2).tolist())))
print("real", N.table(w, "real_land"), "neutral", N.table(w, "neutral_land"))
pal = {i + 1: (str(val), NLCD_COL[val]) for i, val in enumerate(NLCD_VALUES)}
figs.raster(w / "neutral_augusta.tif", img / "neutral_augusta.png", palette=pal, legend=False, width=700)
figs.raster(N.AUGUSTA, img / "real_augusta.png", palette={val: (str(val), NLCD_COL[val]) for val in NLCD_VALUES},
            legend=False, width=700)
