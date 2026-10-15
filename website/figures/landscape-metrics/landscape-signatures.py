"""Figures of "Landscape signatures and kinds of landscapes": the CORINE map around Hannover cut into squares of
2.5 km, each described by its metrics (metrics.signatures), their principal components (metrics.pca), clusters
for k = 2 to 7 (metrics.cluster, a slider), a clustergram (metrics.clustergram) and non-negative factors
(metrics.nmf)."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _lm as L  # noqa: E402
from _lm import figs, np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

SLUG = "landscape-metrics/landscape-signatures"
w = L.work("landscape-signatures")
img = figs.img_dir(SLUG)
dat = figs.data_dir(SLUG)

LMET = ["shdi", "ed", "lpi", "contag", "ai"]
KS = [2, 3, 4, 5, 6, 7]
model = (L.HEAD + L.node("corine", "raster.read", params={"path": "clc.tif"})
         + L.node("cover", "raster.reclassify", {"map": "corine.map"}, {"rules": L.CLC7_RULES})
         + L.node("signatures", "metrics.signatures", {"maps": "cover.map"},
                  {"grid": 2500, "landscape_metrics": LMET, "class_metrics": ["pland"], "classes": [1, 2, 3, 4]})
         + L.node("pca", "metrics.pca", {"table": "signatures.table"}, {"components": 3})
         + L.node("nmf", "metrics.nmf", {"table": "signatures.table"}, {"factors": 3})
         + L.node("cgram", "metrics.clustergram", {"table": "signatures.table"}, {"k_min": 1, "k_max": 8}))
writes = {"cover.map": "cover7.tif", "signatures.table": "signatures.csv", "pca.scores": "scores.csv",
          "pca.loadings": "loadings.csv", "nmf.scores": "nmf_scores.csv", "nmf.components": "nmf_components.csv",
          "cgram.means": "cgram_means.csv", "cgram.labels": "cgram_labels.csv"}
for k in KS:
    model += L.node(f"k{k}", "metrics.cluster", {"table": "signatures.table"}, {"k": k, "seed": 0})
    writes[f"k{k}.table"] = f"k{k}.csv"
L.run(model, w, writes)

sig = L.table(w, "signatures")
print("signatures", len(sig), list(sig[0].keys()))
print(sig[0])
cols = [c for c in sig[0] if c != "zone"]
load = L.table(w, "loadings")
for r in load:
    print({k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()})
figs.write_csv(dat / "loadings.csv", [{k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()} for r in load])
nmfc = L.table(w, "nmf_components")
for r in nmfc:
    print("nmf", {k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()})
figs.write_csv(dat / "nmf_components.csv", [{k: (round(v, 3) if isinstance(v, float) else v) for k, v in r.items()} for r in nmfc])

# the map of squares: zone z (1..) is row (z-1)//20, column (z-1)%20
N = 20
KCOL = ["#0072b2", "#e69f00", "#009e73", "#cc79a7", "#56b4e9", "#d55e00", "#f0e442"]
c7 = figs.read(w / "cover7.tif")[0].filled(0).astype(int)
lut = np.zeros((8, 3))
for kk, (_, col) in L.CLC7.items():
    lut[kk] = mcolors.to_rgb(col)
base = lut[c7]


def zone_map(values: dict[int, int], png, title=None, legend=None):
    # a fixed frame (no tight bounding box), so that the maps of every k have the same size in the slider
    fig = plt.figure(figsize=(5.6, 6.3), dpi=150)
    ax = fig.add_axes([0.02, 0.13, 0.96, 0.96 * 5.6 / 6.3])
    grid = np.full((N, N, 4), 0.0)
    grid[:, :] = (0.85, 0.85, 0.85, 1.0)
    for z, v in values.items():
        r, c = (z - 1) // N, (z - 1) % N
        grid[r, c] = (*mcolors.to_rgb(KCOL[v]), 1.0)
    ax.imshow(grid, interpolation="nearest", extent=(0, N, N, 0))
    for i in range(N + 1):
        ax.axhline(i, color="white", lw=0.4)
        ax.axvline(i, color="white", lw=0.4)
    ax.set_axis_off()
    if legend:
        hs = [Patch(facecolor=KCOL[i], label=n) for i, n in legend.items()] + [Patch(facecolor="#d9d9d9", label="left out (NA)")]
        fig.legend(handles=hs, loc="lower center", bbox_to_anchor=(0.5, 0.005), ncol=4, fontsize=9)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


ksum = []
for k in KS:
    t = L.table(w, f"k{k}")
    lab = {int(r["zone"]): int(r["cluster"]) for r in t}
    zone_map(lab, img / f"clusters_{k}.png", legend={i: f"group {i}" for i in range(k)})
    sizes = np.bincount(list(lab.values()), minlength=k)
    ksum.append({"k": k, "sizes": " / ".join(str(s) for s in sizes)})
    print(k, sizes)

# k = 4: what each group is (its mean metrics) and the PCA scores
t4 = L.table(w, "k4")
lab4 = {int(r["zone"]): int(r["cluster"]) for r in t4}
byz = {int(r["zone"]): r for r in sig}
means = []
for g in range(4):
    zs = [z for z, v in lab4.items() if v == g]
    row = {"group": g, "squares": len(zs)}
    for c in cols:
        row[c] = round(float(np.nanmean([byz[z][c] for z in zs])), 2)
    means.append(row)
    print(row)
figs.write_csv(dat / "group_means_k4.csv", means)
# the mean z-score of every metric in every group (a heatmap)
X = np.array([[byz[z][c] for c in cols] for z in lab4])
mu, sd = np.nanmean(X, 0), np.nanstd(X, 0)
Z = (X - mu) / sd
labs = np.array([lab4[z] for z in lab4])
H = np.array([Z[labs == g].mean(0) for g in range(4)])
names = {"pland_1": "% urban", "pland_2": "% arable", "pland_3": "% pasture", "pland_4": "% forest"}
xl = [names.get(c, c) for c in cols]
fig, ax = plt.subplots(figsize=(8.4, 3.0), dpi=150)
im = ax.imshow(H, cmap="RdBu_r", vmin=-2, vmax=2, aspect="auto")
ax.set_xticks(range(len(cols)), xl, rotation=30, ha="right")
ax.set_yticks(range(4), [f"group {g} ({(labs == g).sum()})" for g in range(4)])
ax.grid(False)
for i in range(4):
    for j in range(len(cols)):
        ax.text(j, i, f"{H[i, j]:+.1f}", ha="center", va="center", fontsize=8.5,
                color="white" if abs(H[i, j]) > 1.1 else figs.INK)
cb = fig.colorbar(im, ax=ax, fraction=0.03, pad=0.02)
cb.outline.set_visible(False)
cb.set_label("mean z-score", color=figs.INK2)
fig.tight_layout()
fig.savefig(img / "group_profiles.png", dpi=150)
plt.close(fig)
figs._shrink(img / "group_profiles.png")

# the PCA biplot: the squares on PC1 and PC2, coloured by their group, and the loadings as arrows
sc = L.table(w, "scores")
fig, ax = plt.subplots(figsize=(7.2, 6.0), dpi=150)
for r in sc:
    z = int(r["zone"])
    ax.scatter(r["PC1"], r["PC2"], s=14, color=KCOL[lab4[z]], alpha=0.75, lw=0)
l1 = {k: v for k, v in load[0].items() if k in cols}
l2 = {k: v for k, v in load[1].items() if k in cols}
xm = max(abs(r["PC1"]) for r in sc)
ym = max(abs(r["PC2"]) for r in sc)
sx = 0.8 * min(xm / max(abs(v) for v in l1.values()), ym / max(abs(v) for v in l2.values()))
for c in cols:
    if abs(l1[c]) + abs(l2[c]) < 0.05:
        continue
    ax.annotate("", xy=(l1[c] * sx, l2[c] * sx), xytext=(0, 0), arrowprops=dict(arrowstyle="->", color=figs.INK, lw=1.1))
    dy = {"contag": 0.25, "lpi": -0.25}.get(c, 0)
    ax.text(l1[c] * sx * 1.12, l2[c] * sx * 1.12 + dy, names.get(c, c), fontsize=9, color=figs.INK, ha="center",
            va="center", bbox=dict(boxstyle="round,pad=0.12", fc="white", ec="none", alpha=0.8))
tips = [l2[c] * sx * 1.2 for c in cols]
ax.set_ylim(min(min(r["PC2"] for r in sc), min(tips)) - 0.2, max(max(r["PC2"] for r in sc), max(tips)) + 0.2)
ex = [r.get("explained", float("nan")) for r in load]
ax.set_xlabel(f"PC1 ({100 * ex[0]:.0f} % of the variance)")
ax.set_ylabel(f"PC2 ({100 * ex[1]:.0f} %)")
ax.legend(handles=[Patch(facecolor=KCOL[g], label=f"group {g}") for g in range(4)], fontsize=9, loc="best")
fig.tight_layout()
fig.savefig(img / "biplot.png", dpi=150)
plt.close(fig)
figs._shrink(img / "biplot.png")
print("explained", ex)

# the clustergram: per k the means of the clusters (dots sized by their squares), lines joining clusters that share squares
cm = L.table(w, "cgram_means")
kcols = sorted([c for c in cm[0] if c.startswith("k") and c[1:].isdigit()], key=lambda c: int(c[1:]))
fig, ax = plt.subplots(figsize=(8.0, 4.6), dpi=150)
for a, b in zip(kcols[:-1], kcols[1:]):
    pairs = {}
    for r in cm:
        key = (round(r[a], 6), round(r[b], 6))
        pairs[key] = pairs.get(key, 0) + 1
    for (ya, yb), n in pairs.items():
        ax.plot([int(a[1:]), int(b[1:])], [ya, yb], color=figs.FOREST, lw=0.4 + 6 * n / len(cm), alpha=0.55)
for c in kcols:
    vals, counts = np.unique([round(r[c], 6) for r in cm], return_counts=True)
    ax.scatter([int(c[1:])] * len(vals), vals, s=10 + 160 * counts / len(cm), color=figs.INK, zorder=3)
ax.set_xlabel("number of clusters, k")
ax.set_ylabel("cluster mean on the first principal axis")
fig.tight_layout()
fig.savefig(img / "clustergram.png", dpi=150)
plt.close(fig)
figs._shrink(img / "clustergram.png")

# the land cover with the grid
fig, ax = plt.subplots(figsize=(6.2, 6.2), dpi=150)
ax.imshow(base, interpolation="nearest", extent=(0, N, N, 0))
for i in range(N + 1):
    ax.axhline(i, color="white", lw=0.5)
    ax.axvline(i, color="white", lw=0.5)
ax.set_axis_off()
ax.legend(handles=[Patch(facecolor=col, label=n) for n, col in L.CLC7.values()], loc="upper center",
          bbox_to_anchor=(0.5, -0.01), ncol=4, fontsize=8.5)
fig.tight_layout()
fig.savefig(img / "grid.png", dpi=150, bbox_inches="tight", pad_inches=0.04)
plt.close(fig)
figs._shrink(img / "grid.png")
