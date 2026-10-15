"""Figures of "Fitting models" (tutorials/species-distributions/fitting-models).

    python website/figures/species-distributions/fitting-models.py

The sloth's records prepared as in part 3 (sdm.prepare: thinned at 50 km, 500 km accessible area, VIF 10, 2 000
background points); folds of several designs (sdm.spatial_blocks, random_folds, quadrant_folds,
checkerboard_folds, spatial_clusters) and MaxNet's scores over each; MaxNet over a range of regularisation
multipliers (maps and scores) and of feature classes; a GLM and random forests; response curves and the
permutation importance of the predictors.
"""

from __future__ import annotations

import numpy as np
import matplotlib.pyplot as plt

import sdmfig as S
from sdmfig import figs

SLUG = "fitting-models"
IMG = figs.img_dir(f"{S.CAT}/{SLUG}")
DAT = figs.data_dir(f"{S.CAT}/{SLUG}")
FOLD_COLORS = ["#0072b2", "#e69f00", "#009e73", "#cc79a7", "#56b4e9", "#d55e00"]
SCORES = ("auc_train", "auc_test", "auc_diff", "tss", "boyce", "or_mtp", "or_10p")


def summary(w, name):
    s = S.out(w, name)[0]
    return {k: round(float(s[f"{k}_mean"]), 3) for k in SCORES}


def folds():
    w = S.work(SLUG + "_folds")
    m = S.prepared()
    designs = {
        "blocks": ("sdm.spatial_blocks", {"size": 500000, "k": 5, "seed": 1}, False),
        "random": ("sdm.random_folds", {"k": 5, "seed": 1}, False),
        "quadrants": ("sdm.quadrant_folds", {}, False),
        "checkerboard": ("sdm.checkerboard_folds", {"aggregation": 10}, True),
        "clusters": ("sdm.spatial_clusters", {"k": 5, "seed": 1}, False),
    }
    outs = {}
    for k, (op, p, env) in designs.items():
        inputs = {"records": "data.records"}
        if env:
            inputs["env"] = "data.env"
        m += S.node(f"f_{k}", op, inputs, p)
        m += S.node(f"fit_{k}", "sdm.fit", {"records": f"f_{k}.records"})
        outs[f"r_{k}"] = f"f_{k}.records"
        outs[f"s_{k}"] = f"fit_{k}.summary"
    m += S.outputs(outs)
    S.run(m, w)
    rows = []
    for k in designs:
        t = S.table(w, f"r_{k}.csv")
        draw_folds(t, IMG / f"folds_{k}.png")
        s = summary(w, f"s_{k}.csv")
        p = t["presence"] == 1
        nf = len(np.unique(t["fold"][p]))
        rows.append({"design": k, "folds": nf, **{c: s[c] for c in ("auc_test", "tss", "boyce", "or_10p")}})
    figs.write_csv(DAT / "folds.csv", rows)
    return rows


def draw_folds(t, png):
    fig, ax = S.map_axes(600)
    S.base(ax)
    ax.set_xlim(-95, -30)
    ax.set_ylim(-35, 22)
    p = t["presence"] == 1
    fold = t["fold"].astype(int)
    for i, f in enumerate(sorted(np.unique(fold))):
        col = FOLD_COLORS[i % len(FOLD_COLORS)]
        b = (~p) & (fold == f)
        S.points(ax, t["x"][b], t["y"][b], color=col, size=3, edge="none", alpha=0.45, z=2)
        q = p & (fold == f)
        S.points(ax, t["x"][q], t["y"][q], color=col, size=26, edge="#1c2420", lw=0.7, z=4)
    S.save(fig, png)


def regularisation():
    mults = [0.25, 0.5, 1, 2, 4, 8]
    w = S.work(SLUG + "_regmult")
    m = S.prepared()
    outs = {}
    for r in mults:
        k = str(r).replace(".", "_")
        m += S.node(f"fit_{k}", "sdm.fit", {"records": "data.records"}, {"regmult": r})
        m += S.node(f"pred_{k}", "sdm.maxnet_predict", {"model": f"fit_{k}.model", "env": "data.env"})
        m += S.node(f"w_{k}", "raster.write", {"value": f"pred_{k}.prediction"}, {"path": f"pred_{k}.tif"})
        outs[f"s_{k}"] = f"fit_{k}.summary"
        # the model on every record, as JSON: its features with a nonzero coefficient
        outs[f"c_{k}"] = f"fit_{k}.model"
    m += S.node("w_area", "vector.write", {"vector": "data.area"}, {"path": "area.gpkg"})
    m += S.outputs(outs | {"records": "data.records"})
    S.run(m, w)
    import geopandas as gpd
    area = gpd.read_file(w / "area.gpkg")
    rec = S.table(w, "records.csv")
    rows = []
    for r in mults:
        k = str(r).replace(".", "_")
        s = summary(w, f"s_{k}.csv")
        coef = S.out(w, f"c_{k}.json")
        nfeat = len(coef["names"])
        print("regmult", r, "classes", coef["classes"], "features", nfeat)
        a, info = figs.read(w / f"pred_{k}.tif")
        fig, ax = S.map_axes(640, legend_frac=0.16)
        S.base(ax)
        im = S.surface(ax, a, info, vmin=0, vmax=1)
        area.plot(ax=ax, facecolor="none", edgecolor="white", linewidth=0.8)
        S.base_limits(ax)
        p = rec["presence"] == 1
        S.points(ax, rec["x"][p], rec["y"][p], size=7, lw=0.3)
        S.colorbar(fig, im, "Suitability", rect=(0.86, 0.3, 0.03, 0.45))
        ax.text(-123, -52, f"regmult {r}", fontsize=12, fontweight="bold", color=figs.INK)
        S.save(fig, IMG / f"regmult_{k}.png")
        rows.append({"regmult": r, "features": nfeat, **s})
    figs.write_csv(DAT / "regmult.csv", rows)
    return rows


def models():
    w = S.work(SLUG + "_models")
    m = S.prepared()
    variants = {
        "maxnet l": {"classes": "l"}, "maxnet lq": {"classes": "lq"}, "maxnet lqh": {"classes": "lqh"},
        "maxnet lqph": {"classes": "lqph"}, "maxnet lqpht": {"classes": "lqpht"},
        "glm": {"model": "glm"},
        "forest, defaults": {"model": "random_forest"},
        "forest, leaves of 20": {"model": "random_forest", "n_estimators": 500, "min_samples_leaf": 20},
        "forest, depth 3": {"model": "random_forest", "n_estimators": 500, "max_depth": 3},
    }
    outs = {}
    ids = {}
    for i, (k, p) in enumerate(variants.items()):
        ids[k] = f"m{i}"
        m += S.node(f"fit_m{i}", "sdm.fit", {"records": "data.records"}, p)
        outs[f"s_m{i}"] = f"fit_m{i}.summary"
    # curves, importance and maps of the default MaxNet, the GLM and the tuned forest
    show = {"MaxNet": "fit_m2", "GLM": "fit_m5", "Random forest": "fit_m8"}
    for j, (lab, fid) in enumerate(show.items()):
        m += S.node(f"curves_{j}", "sdm.response_curves", {"model": f"{fid}.model", "records": "data.records"})
        m += S.node(f"imp_{j}", "sdm.importance", {"model": f"{fid}.model", "records": "data.records"})
        m += S.node(f"pred_{j}", "sdm.maxnet_predict", {"model": f"{fid}.model", "env": "data.env"})
        m += S.node(f"wp_{j}", "raster.write", {"value": f"pred_{j}.prediction"}, {"path": f"pred_{j}.tif"})
        outs[f"curves_{j}"] = f"curves_{j}.curves"
        outs[f"imp_{j}"] = f"imp_{j}.importance"
        outs[f"impn_{j}"] = f"imp_{j}.predictors"
    m += S.outputs(outs | {"records": "data.records", "metrics": "fit_m2.metrics", "threshold": "fit_m2.threshold"})
    S.run(m, w)
    rows = []
    for k, i in ids.items():
        s = summary(w, f"s_{i}.csv")
        rows.append({"model": k, **s})
    figs.write_csv(DAT / "models.csv", rows)
    # response curves
    preds = ["bio5", "bio7", "bio16", "bio17"]
    units = {"bio5": ("Max temperature of the warmest month (°C)", 10), "bio7": ("Annual range of temperature (°C)", 10),
             "bio16": ("Precipitation of the wettest quarter (mm)", 1),
             "bio17": ("Precipitation of the driest quarter (mm)", 1)}
    cols = {"MaxNet": figs.WATER, "GLM": figs.FOREST, "Random forest": figs.EMBER}
    curves = {lab: S.table(w, f"curves_{j}.csv") for j, lab in enumerate(show)}
    rec = S.table(w, "records.csv")
    fig, axes = plt.subplots(2, 2, figsize=(7.6, 5.6), dpi=150)
    for ax, pname in zip(axes.flat, preds):
        lab, div = units[pname]
        for name, c in curves.items():
            ax.plot(c[pname] / div, c[f"{pname}_suitability"], color=cols[name], lw=1.8, label=name)
        p = rec["presence"] == 1
        ax.plot(rec[pname][p] / div, np.full(p.sum(), -0.04), "|", color=S.PRES, ms=6, alpha=0.7)
        ax.set_xlabel(lab, fontsize=9)
        ax.set_ylim(-0.08, 1.02)
        ax.set_ylabel("Suitability", fontsize=9)
        ax.tick_params(labelsize=8.5)
    axes[0, 0].legend(fontsize=8.5, loc="upper left")
    fig.tight_layout()
    S.save(fig, IMG / "curves.png")
    # importance
    rows_i = {}
    for j, lab in enumerate(show):
        names = S.out(w, f"impn_{j}.json")
        imp = S.out(w, f"imp_{j}.csv")
        for n, r in zip(names, imp):
            rows_i.setdefault(n, {"predictor": n})[lab] = round(float(r["importance"]), 4)
    figs.write_csv(DAT / "importance.csv", [rows_i[p] for p in preds])
    # maps of the three models
    for j, lab in enumerate(show):
        a, info = figs.read(w / f"pred_{j}.tif")
        fig, ax = S.map_axes(600)
        S.base(ax)
        S.surface(ax, a, info, vmin=0, vmax=1)
        ax.text(-123, -52, lab, fontsize=12, fontweight="bold", color=figs.INK)
        S.save(fig, IMG / f"model_{j}.png")
    return rows, rows_i, S.out(w, "metrics.csv"), S.out(w, "threshold.json")


def feature_classes():
    """An explanatory drawing: the feature classes MaxNet builds from one predictor (and a pair)."""
    z = np.linspace(0, 1, 200)
    fig, axs = plt.subplots(1, 5, figsize=(10.2, 2.35), dpi=150)
    panels = [
        ("Linear (l)", [(z, figs.WATER, "z")]),
        ("Quadratic (q)", [(z ** 2, figs.WATER, "z²")]),
        ("Hinge (h)", [(np.clip((z - 0.4) / 0.6, 0, 1), figs.WATER, "forward"),
                       (np.clip((0.6 - z) / 0.6, 0, 1), figs.EMBER, "reverse")]),
        ("Threshold (t)", [((z > 0.55).astype(float), figs.WATER, "z > 0.55")]),
    ]
    for ax, (title, lines) in zip(axs, panels):
        for y, c, lab in lines:
            ax.plot(z, y, color=c, lw=2, label=lab)
        ax.set_title(title, loc="left", fontsize=10.5)
        ax.set_xlabel("predictor z", fontsize=9)
        ax.set_ylim(-0.05, 1.08)
        ax.tick_params(labelsize=8)
        if len(lines) > 1:
            ax.legend(fontsize=8, loc="upper center")
    ax = axs[4]
    g = np.linspace(0, 1, 60)
    ax.imshow(np.outer(g[::-1], g), cmap="viridis", extent=[0, 1, 0, 1])
    ax.set_title("Product (p)", loc="left", fontsize=10.5)
    ax.set_xlabel("z₁", fontsize=9)
    ax.set_ylabel("z₂", fontsize=9)
    ax.grid(False)
    ax.tick_params(labelsize=8)
    fig.tight_layout()
    S.save(fig, IMG / "feature-classes.png")


if __name__ == "__main__":
    import sys
    if len(sys.argv) > 1:
        for t in sys.argv[1:]:
            print(globals()[t]())
        raise SystemExit
    feature_classes()
    for r in folds():
        print(r)
    for r in regularisation():
        print(r)
    rows, imp, met, thr = models()
    for r in rows:
        print(r)
    print(imp)
    for r in met:
        print(r)
    print("threshold", thr)
