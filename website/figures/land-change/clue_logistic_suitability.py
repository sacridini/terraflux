"""Figures of "Suitability from logistic regressions" (land-change/logistic-suitability): the drivers of Plum
Island, the regressions of lulcc's vignette (glm.fit), the suitability maps (glm.predict), response curves from
the fitted coefficients, ROC curves of the suitability against the observed maps, and the regression of built
land with one, two and three drivers (real runs)."""

import json

import matplotlib.pyplot as plt
import numpy as np

from clue_common import CLASSES, NAMES, SUITABILITY, figs, pie_work

SLUG = "land-change/logistic-suitability"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
work = pie_work(SLUG)
DRIVERS = ["elevation", "slope", "dist_built_1985"]


def cpanels(arrays, titles, png, cmap="viridis", vmin=None, vmax=None, units=None, width=1100, shared=False):
    """Continuous maps side by side, each with its colour bar (or one shared)."""
    n = len(arrays)
    h, w = arrays[0].shape
    fig_w = width / 150
    fig_h = fig_w / n * h / w + 0.75
    fig, axes = plt.subplots(1, n, figsize=(fig_w, fig_h), dpi=150)
    ims = []
    for i, (ax, a, t) in enumerate(zip(axes, arrays, titles)):
        a = np.ma.masked_invalid(np.ma.asarray(a, dtype=float))
        cm = plt.get_cmap(cmap).copy()
        cm.set_bad((1, 1, 1, 0))
        lo = vmin if vmin is not None else float(a.min())
        hi = vmax if vmax is not None else float(a.max())
        im = ax.imshow(a, cmap=cm, vmin=lo, vmax=hi, interpolation="nearest")
        ims.append(im)
        ax.set_axis_off()
        ax.set_title(t, fontsize=11, loc="left")
        if not shared:
            cb = fig.colorbar(im, ax=ax, orientation="horizontal", fraction=0.05, pad=0.02, aspect=30)
            cb.outline.set_visible(False)
            cb.ax.tick_params(labelsize=8.5)
            if units:
                cb.set_label(units[i], fontsize=9)
    if shared:
        cb = fig.colorbar(ims[0], ax=list(axes), orientation="horizontal", fraction=0.05, pad=0.03, aspect=50)
        cb.outline.set_visible(False)
        cb.ax.tick_params(labelsize=8.5)
        if units:
            cb.set_label(units[0], fontsize=9)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.08)
    plt.close(fig)
    figs._shrink(png)


land = figs.read(work / "pie_landuse.tif", 1)[0]
land99 = figs.read(work / "pie_landuse.tif", 3)[0]
drv = [figs.read(work / "pie_drivers.tif", b)[0] for b in (1, 2, 3)]
cpanels(drv, ["Elevation", "Slope", "Distance to built land (1985)"], out / "drivers.png",
        units=["", "", "metres"])
valid = ~land.mask
print("driver correlations", np.round(np.corrcoef(np.stack([d[valid] for d in drv])), 3))

# --- the regressions of the vignette and their suitability
model = figs.add_writes(SUITABILITY, {"suitability.suitability": "suitability.tif", "regression.model": "model.json"})
figs.run(model, work, name="glm.toml")
fits = json.loads((work / "output" / "model.json").read_text())["fits"]
rows = []
for f in fits:
    k = int(f["class"])
    r2 = 1 - f["deviance"] / f["null_deviance"]
    rows.append({"class": NAMES[k], "intercept": f["coefficients"][0],
                 **{d: (f["coefficients"][1 + f["drivers"].index(d)] if d in f["drivers"] else None) for d in DRIVERS},
                 "deviance": f["deviance"], "null_deviance": f["null_deviance"], "pseudo_r2": r2,
                 "iterations": f["iterations"], "cells": f["cells"]})
    print(NAMES[k], [round(c, 5) for c in f["coefficients"]], "R2", round(r2, 3), f["iterations"], f["converged"])
suit = [figs.read(work / "suitability.tif", b)[0] for b in (1, 2, 3)]
cpanels(suit, [f"{NAMES[k]}" for k in CLASSES], out / "suitability.png", vmin=0, vmax=1, shared=True,
        units=["Suitability (probability of the class)"])
total = suit[0] + suit[1] + suit[2]
print("sum of suitabilities, percentiles 1 50 99:", np.round(np.percentile(total.compressed(), [1, 50, 99]), 3))

# --- response curves: the probability of built land along each driver, the others at their medians
med = {d: float(np.median(x.compressed())) for d, x in zip(DRIVERS, drv)}
f2 = next(f for f in fits if int(f["class"]) == 2)


def built_p(values: dict) -> np.ndarray:
    z = f2["coefficients"][0] + sum(c * values[d] for d, c in zip(f2["drivers"], f2["coefficients"][1:]))
    return 1 / (1 + np.exp(-z))


curves = {}
for d, x in zip(DRIVERS, drv):
    lo, hi = np.percentile(x.compressed(), [0.5, 99.5])
    xs = np.linspace(lo, hi, 60)
    vals = {k: np.full_like(xs, med[k]) for k in DRIVERS}
    vals[d] = xs
    curves[d] = (xs, built_p(vals))


def draw_curves(ax):
    for d, col, lab in zip(DRIVERS, ["#3b8a5a", "#2f6f9f", "#b4508a"],
                           ["Elevation", "Slope", "Distance to built land (m)"]):
        pass


fig, axes = plt.subplots(1, 3, figsize=(1100 / 150, 330 / 150), dpi=150, sharey=True)
for ax, d, lab in zip(axes, DRIVERS, ["Elevation", "Slope", "Distance to built land in 1985 (m)"]):
    xs, ys = curves[d]
    ax.plot(xs, ys, color="#b4508a", lw=2)
    ax.set_xlabel(lab)
    ax.set_ylim(0, 1)
axes[0].set_ylabel("Probability of built land")
fig.suptitle("Built land along each driver, the other two at their medians", x=0.01, ha="left", fontsize=11.5,
             fontweight="bold")
fig.tight_layout()
fig.savefig(out / "response_built.png", dpi=150)
plt.close(fig)
figs._shrink(out / "response_built.png")


# --- ROC: how well the suitability ranks the cells of each class (1985), and the new built land (1999)
def roc(score, truth, n=60):
    s = score.astype(float)
    t = truth.astype(bool)
    order = np.argsort(-s, kind="stable")
    t = t[order]
    tp = np.cumsum(t) / t.sum()
    fp = np.cumsum(~t) / (~t).sum()
    auc = float(np.trapezoid(np.r_[0, tp], np.r_[0, fp]))
    idx = np.linspace(0, len(tp) - 1, n).astype(int)
    return np.r_[0, fp[idx]], np.r_[0, tp[idx]], auc


rocs = {}
for k in CLASSES:
    s = suit[k - 1][valid].filled(np.nan)
    ok = ~np.isnan(s)
    rocs[NAMES[k]] = roc(s[ok], (land[valid] == k)[ok])
# where built land appeared: among the cells not built in 1985
cand = valid & (land.filled(0) != 2)
s = suit[1][cand].filled(np.nan)
ok = ~np.isnan(s)
rocs["New built land (1985-1999)"] = roc(s[ok], (land99[cand] == 2)[ok])
for k, (_, _, a) in rocs.items():
    print("AUC", k, round(a, 3))
grid = np.linspace(0, 1, 41)
rows_roc = []
for x in grid:
    r = {"fpr": round(float(x), 3)}
    for k, (fp, tp, _) in rocs.items():
        r[k] = round(float(np.interp(x, fp, tp)), 4)
    r["Chance"] = round(float(x), 3)
    rows_roc.append(r)
figs.write_csv(data / "roc.csv", rows_roc)

# --- built land with one, two and three drivers (real runs of glm.fit / glm.predict)
variants = {1: ["elevation"], 2: ["elevation", "slope"], 3: ["elevation", "slope", "dist_built_1985"]}
stats = {}
for n, ds in variants.items():
    m = SUITABILITY.replace(
        '2 = ["elevation", "slope", "dist_built_1985"]', "2 = [" + ", ".join(f'"{d}"' for d in ds) + "]")
    m = figs.add_writes(m, {"suitability.suitability": f"suit_{n}.tif", "regression.model": f"model_{n}.json"})
    figs.run(m, work, name=f"glm_{n}.toml")
    b = figs.read(work / f"suit_{n}.tif", 2)[0]
    figs.raster(None, out / f"built_{n}.png", array=b, vmin=0, vmax=1, unit="Suitability of built land", width=760)
    fit = next(f for f in json.loads((work / "output" / f"model_{n}.json").read_text())["fits"] if f["class"] == 2)
    sb = b[valid].filled(np.nan)
    ok = ~np.isnan(sb)
    auc_now = roc(sb[ok], (land[valid] == 2)[ok])[2]
    sc = b[cand].filled(np.nan)
    ok2 = ~np.isnan(sc)
    auc_new = roc(sc[ok2], (land99[cand] == 2)[ok2])[2]
    stats[n] = {"r2": 1 - fit["deviance"] / fit["null_deviance"], "auc": auc_now, "auc_new": auc_new}
    print(n, ds, {k: round(v, 3) for k, v in stats[n].items()})

# the slider's frames (pasted into the page)
for n, ds in variants.items():
    st = stats[n]
    print(f'  - {{ value: {n}, src: /img/tutorials/{SLUG}/built_{n}.png, caption: "{" + ".join(ds)}", '
          f'stats: {{ "Pseudo R²": "{st["r2"]:.3f}", "AUC, built land of 1985": "{st["auc"]:.3f}", '
          f'"AUC, new built land": "{st["auc_new"]:.3f}" }} }}')
