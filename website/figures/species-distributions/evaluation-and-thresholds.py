"""Figures of "Evaluation and thresholds" (tutorials/species-distributions/evaluation-and-thresholds).

    python website/figures/species-distributions/evaluation-and-thresholds.py

The sloth's model of part 4 (MaxNet, regmult 2, classes lqh) on the records prepared by sdm.prepare; its predictions
at the presences and at the background points (sdm.dataset of each, sdm.maxnet_predict on the tables); sdm.evaluate's
scores, confusion table and Boyce classes; the thresholds of sdm.threshold by every method and the maps they give;
and the fit's out-of-fold predictions (sdm.fit's predictions): their scores, thresholds, Boyce curve and map.

    python website/figures/species-distributions/evaluation-and-thresholds.py oof   # only the out-of-fold part
"""

from __future__ import annotations

import numpy as np
import matplotlib.pyplot as plt

import sdmfig as S
from sdmfig import figs

SLUG = "evaluation-and-thresholds"
IMG = figs.img_dir(f"{S.CAT}/{SLUG}")
DAT = figs.data_dir(f"{S.CAT}/{SLUG}")
METHODS = ["no_omission", "mtp", "p10", "sensitivity", "max_spec_sens", "equal_sens_spec", "max_kappa",
           "equal_prevalence"]
FIT = {"model": "maxnet", "regmult": 2, "classes": "lqh"}


def model() -> str:
    m = S.prepared()
    m += S.node("fit", "sdm.fit", {"records": "data.records"}, FIT)
    m += S.node("predict", "sdm.maxnet_predict", {"model": "fit.model", "env": "data.env"})
    m += S.node("at_presences", "sdm.dataset", {"presences": "data.presences", "env": "data.env"})
    m += S.node("at_background", "sdm.dataset", {"presences": "data.background_points", "env": "data.env"})
    m += S.node("p_pres", "sdm.maxnet_predict", {"model": "fit.model", "env": "at_presences.records"})
    m += S.node("p_back", "sdm.maxnet_predict", {"model": "fit.model", "env": "at_background.records"})
    m += S.node("evaluation", "sdm.evaluate", {"presences": "p_pres.prediction", "background": "p_back.prediction"})
    outs = {"stats": "evaluation.stats", "confusion": "evaluation.table", "thresholds": "evaluation.thresholds",
            "boyce": "evaluation.boyce", "pres": "p_pres.prediction", "back": "p_back.prediction",
            "summary": "fit.summary", "fit_threshold": "fit.threshold", "records": "data.records"}
    for meth in METHODS:
        p = {"method": "sensitivity", "sensitivity": 0.95} if meth == "sensitivity" else {"method": meth}
        m += S.node(f"t_{meth}", "sdm.threshold", {"presences": "p_pres.prediction", "background": "p_back.prediction"}, p)
        m += S.node(f"bin_{meth}", "raster.calc", {"s": "predict.prediction", "t": f"t_{meth}.threshold"},
                    {"expression": "1 if s >= t else 0"})
        m += S.node(f"w_{meth}", "raster.write", {"value": f"bin_{meth}.map"}, {"path": f"bin_{meth}.tif"})
        outs[f"t_{meth}"] = f"t_{meth}.threshold"
    m += S.node("w_pred", "raster.write", {"value": "predict.prediction"}, {"path": "prediction.tif"})
    m += S.node("w_area", "vector.write", {"vector": "data.area"}, {"path": "area.gpkg"})
    m += S.outputs(outs)
    return m


def main():
    w = S.work(SLUG)
    S.run(model(), w)
    stats = S.out(w, "stats.csv")[0]
    print("stats", stats)
    print("summary", S.out(w, "summary.csv")[0])
    print("fit threshold", S.out(w, "fit_threshold.json"))
    print("thresholds table", S.out(w, "thresholds.csv"))
    pres = S.table(w, "pres.csv")
    back = S.table(w, "back.csv")
    kp = list(pres)[-1]
    p, b = pres[kp], back[list(back)[-1]]
    print("columns", list(pres), "n", len(p), len(b))

    # the confusion table: the ROC curve and sensitivity, specificity, TSS by threshold
    conf = S.table(w, "confusion.csv")
    print("confusion columns", list(conf))
    th = conf["threshold"] if "threshold" in conf else conf[list(conf)[0]]
    sens, spec = conf["tpr"], conf["tnr"]
    order = np.argsort(1 - spec)
    roc = [{"fpr": round(float(1 - spec[i]), 4), "Model": round(float(sens[i]), 4),
            "Chance": round(float(1 - spec[i]), 4)} for i in order]
    roc = [{"fpr": 0.0, "Model": 0.0, "Chance": 0.0}] + roc + [{"fpr": 1.0, "Model": 1.0, "Chance": 1.0}]
    # thin the curve to at most ~120 points
    step = max(1, len(roc) // 120)
    figs.write_csv(DAT / "roc.csv", roc[::step] + ([roc[-1]] if (len(roc) - 1) % step else []))
    bythr = []
    for i in np.argsort(th):
        bythr.append({"threshold": round(float(th[i]), 3), "Sensitivity": round(float(sens[i]), 3),
                      "Specificity": round(float(spec[i]), 3), "TSS": round(float(sens[i] + spec[i] - 1), 3)})
    step = max(1, len(bythr) // 100)
    figs.write_csv(DAT / "by-threshold.csv", bythr[::step])

    # Boyce's classes
    by = S.table(w, "boyce.csv")
    figs.write_csv(DAT / "boyce.csv", [{"suitability": round(float(h), 3), "P/E": round(float(f), 3)}
                                       for h, f in zip(by["hs"], by["f_ratio"]) if np.isfinite(f)])

    # the thresholds and their maps
    a, info = figs.read(w / "prediction.tif")
    import geopandas as gpd
    area = gpd.read_file(w / "area.gpkg")
    from rasterio import features as rf
    inside = rf.geometry_mask(area.geometry, out_shape=a.shape, transform=info["transform"], invert=True)
    land = ~np.ma.getmaskarray(a)
    rows = []
    for meth in METHODS:
        t = float(S.out(w, f"t_{meth}.json"))
        bmap = figs.read(w / f"bin_{meth}.tif")[0]
        se = float(np.mean(p >= t))
        sp_ = float(np.mean(b < t))
        share = float(np.sum((bmap.filled(0) > 0.5) & inside & land) / np.sum(inside & land))
        rows.append({"method": meth, "threshold": round(t, 3), "sensitivity": round(se, 3),
                     "specificity": round(sp_, 3), "tss": round(se + sp_ - 1, 3), "area": round(100 * share, 1)})
        print(rows[-1])
        if meth != "mtp":  # the same threshold as no_omission here
            draw_binary(bmap, info, area, w, IMG / f"binary_{meth}.png")
    figs.write_csv(DAT / "thresholds.csv", rows)
    histogram(p, b, rows, IMG / "histogram.png")


def draw_binary(bmap, info, area, w, png):
    fig, ax = S.map_axes(600)
    S.base(ax)
    rgba = np.zeros(bmap.shape + (4,))
    from matplotlib import colors as mc
    v = bmap.filled(np.nan)
    rgba[v > 0.5] = mc.to_rgba(figs.FOREST)
    rgba[v <= 0.5] = mc.to_rgba("#d8dccf")
    ax.imshow(rgba, extent=S.extent(info), interpolation="nearest")
    area.plot(ax=ax, facecolor="none", edgecolor=figs.INK, linewidth=0.7)
    S.base_limits(ax)
    rec = S.table(w, "records.csv")
    q = rec["presence"] == 1
    S.points(ax, rec["x"][q], rec["y"][q], size=7, lw=0.3)
    S.save(fig, png)


def draw_continuous(a, info, area, w, png):
    fig, ax = S.map_axes(640, legend_frac=0.16)
    S.base(ax)
    im = S.surface(ax, a, info, vmin=0, vmax=1)
    area.plot(ax=ax, facecolor="none", edgecolor="white", linewidth=0.8)
    S.base_limits(ax)
    rec = S.table(w, "records.csv")
    q = rec["presence"] == 1
    S.points(ax, rec["x"][q], rec["y"][q], size=7, lw=0.3)
    S.colorbar(fig, im, "Suitability (cloglog)", rect=(0.86, 0.3, 0.03, 0.45))
    S.save(fig, png)


def histogram(p, b, rows, png):
    fig, ax = plt.subplots(figsize=(7.6, 3.4), dpi=150)
    bins = np.linspace(0, 1, 26)
    ax.hist(b, bins=bins, density=True, color=S.BACK, alpha=0.45, label=f"Background points ({len(b)})")
    ax.hist(p, bins=bins, density=True, color=S.PRES, alpha=0.6, label=f"Presences ({len(p)})")
    show = {"mtp": "mtp", "p10": "p10", "max_spec_sens": "max TSS"}
    for r in rows:
        if r["method"] in show:
            ax.axvline(r["threshold"], color=figs.INK, lw=1, ls=(0, (3, 2)))
            ax.text(r["threshold"] + 0.008, ax.get_ylim()[1] * 0.92, show[r["method"]], fontsize=8.5, color=figs.INK,
                    rotation=90, va="top")
    ax.set_xlabel("Predicted suitability")
    ax.set_ylabel("Density")
    ax.legend(fontsize=8.5, loc="upper right")
    fig.tight_layout()
    S.save(fig, png)


def oof_model() -> str:
    """The fit's out-of-fold predictions split into presences and background, evaluated and thresholded, as the
    page's graph; with the training predictions of the same records for comparison."""
    m = S.prepared()
    m += S.node("fit", "sdm.fit", {"records": "data.records"}, FIT)
    m += S.node("oof_pres", "table.filter", {"table": "fit.predictions"}, {"expression": "presence == 1"})
    m += S.node("oof_back", "table.filter", {"table": "fit.predictions"}, {"expression": "presence == 0"})
    m += S.node("oof_evaluation", "sdm.evaluate", {"presences": "oof_pres.table", "background": "oof_back.table"},
                {"column": "prediction"})
    m += S.node("oof_threshold", "sdm.threshold", {"presences": "oof_pres.table", "background": "oof_back.table"},
                {"column": "prediction", "method": "p10"})
    return m


def out_of_fold():
    w = S.work(SLUG + "_oof")
    m = oof_model()
    m += S.node("t_mss", "sdm.threshold", {"presences": "oof_pres.table", "background": "oof_back.table"},
                {"column": "prediction", "method": "max_spec_sens"})
    m += S.node("at_presences", "sdm.dataset", {"presences": "data.presences", "env": "data.env"})
    m += S.node("at_background", "sdm.dataset", {"presences": "data.background_points", "env": "data.env"})
    m += S.node("p_pres", "sdm.maxnet_predict", {"model": "fit.model", "env": "at_presences.records"})
    m += S.node("p_back", "sdm.maxnet_predict", {"model": "fit.model", "env": "at_background.records"})
    m += S.node("evaluation", "sdm.evaluate", {"presences": "p_pres.prediction", "background": "p_back.prediction"})
    m += S.node("t_p10_train", "sdm.threshold", {"presences": "p_pres.prediction", "background": "p_back.prediction"},
                {"method": "p10"})
    m += S.node("predict", "sdm.maxnet_predict", {"model": "fit.model", "env": "data.env"})
    m += S.node("w_pred", "raster.write", {"value": "predict.prediction"}, {"path": "prediction.tif"})
    m += S.outputs({"predictions": "fit.predictions", "oof_stats": "oof_evaluation.stats",
                    "oof_boyce": "oof_evaluation.boyce", "oof_p10": "oof_threshold.threshold",
                    "oof_mss": "t_mss.threshold", "stats": "evaluation.stats", "boyce": "evaluation.boyce",
                    "train_p10": "t_p10_train.threshold", "fit_threshold": "fit.threshold",
                    "summary": "fit.summary", "train_pres": "p_pres.prediction"})
    S.run(m, w)
    pred = S.table(w, "predictions.csv")
    print("columns", list(pred), "rows", len(pred["prediction"]), "nan", int(np.isnan(pred["prediction"]).sum()))
    print("folds", {int(k): int((pred["fold"] == k).sum()) for k in np.unique(pred["fold"])})
    res = {k: S.out(w, f"{k}.csv")[0] for k in ("oof_stats", "stats", "summary")}
    res.update({k: S.out(w, f"{k}.json") for k in ("oof_p10", "oof_mss", "train_p10", "fit_threshold")})
    for k, v in res.items():
        print(k, v)
    pres = pred["presence"] == 1
    # an excerpt of the table, for the page
    cols = [c for c in pred if c not in ("weight", "cell")]
    head = []
    for i in list(np.flatnonzero(pres)[:3]) + list(np.flatnonzero(~pres)[:2]):
        head.append({c: (round(float(pred[c][i]), 3) if c not in ("presence", "fold") else int(pred[c][i]))
                     for c in cols})
    figs.write_csv(DAT / "oof_excerpt.csv", head)
    print("excerpt", head)
    # the training predictions of the same presences, in the same order
    train = S.table(w, "train_pres.csv")["prediction"]
    p = pred["prediction"][pres]
    print("presences", int(pres.sum()), "median train", float(np.median(train)), "median oof", float(np.median(p)))
    # per fold: the presences' median out-of-fold suitability
    for k in np.unique(pred["fold"]):
        s = pres & (pred["fold"] == k)
        print("fold", int(k), "presences", int(s.sum()), "median oof", round(float(np.median(pred["prediction"][s])), 3),
              "below oof p10", int(np.sum(pred["prediction"][s] < float(res["oof_p10"]))))
    # Boyce's P/E curves, training and out-of-fold
    tb, ob = S.table(w, "boyce.csv"), S.table(w, "oof_boyce.csv")
    fig, ax = plt.subplots(figsize=(7.2, 3.4), dpi=150)
    for t, col, lab in ((tb, "#6b7a72", "Training predictions (the model on every record)"),
                        (ob, "#e8901f", "Out-of-fold predictions (each fold from a model without it)")):
        ok = np.isfinite(t["f_ratio"])
        ax.plot(t["hs"][ok], t["f_ratio"][ok], color=col, lw=2, label=lab)
    ax.axhline(1, color=figs.INK2, lw=0.8, ls=(0, (3, 2)))
    ax.set_xlabel("Predicted suitability (the window's middle)")
    ax.set_ylabel("Predicted / expected")
    ax.legend(fontsize=8.5, loc="upper left", frameon=False)
    fig.tight_layout()
    S.save(fig, IMG / "oof_boyce.png")
    # the map: every presence by its out-of-fold suitability, over the training map in grey
    a, info = figs.read(w / "prediction.tif")
    fig, ax = S.map_axes(760, legend_frac=0.2)
    S.base(ax)
    S.surface(ax, a, info, cmap="Greys", vmin=-0.4, vmax=1.6)
    from matplotlib import colors as mc
    sc = ax.scatter(pred["x"][pres], pred["y"][pres], c=p, cmap="plasma", norm=mc.Normalize(0, 1), s=30,
                    edgecolors="white", linewidths=0.5, zorder=4)
    low = pres & (pred["prediction"] < float(res["oof_p10"]))
    ax.scatter(pred["x"][low], pred["y"][low], s=95, facecolors="none", edgecolors=figs.INK, linewidths=1.0, zorder=5)
    S.base_limits(ax)
    ax.set_xlim(-95, -33)
    ax.set_ylim(-30, 20)
    S.colorbar(fig, sc, "Out-of-fold suitability", rect=(0.84, 0.25, 0.03, 0.5))
    S.save(fig, IMG / "oof_map.png")
    return res


if __name__ == "__main__":
    import sys
    if sys.argv[1:] == ["oof"]:
        print(out_of_fold())
    else:
        main()
        print(out_of_fold())
