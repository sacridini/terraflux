"""Figures of "Evaluation and thresholds" (tutorials/species-distributions/evaluation-and-thresholds).

    python website/figures/species-distributions/evaluation-and-thresholds.py

The sloth's model of part 4 (MaxNet, regmult 2, classes lqh) on the records prepared by sdm.prepare; its predictions
at the presences and at the background points (sdm.dataset of each, sdm.maxnet_predict on the tables); sdm.evaluate's
scores, confusion table and Boyce classes; the thresholds of sdm.threshold by every method and the maps they give.
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


if __name__ == "__main__":
    main()
