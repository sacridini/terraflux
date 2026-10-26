"""Figures of "Projection and extrapolation" (tutorials/species-distributions/projection-and-extrapolation).

    python website/figures/species-distributions/projection-and-extrapolation.py

The sloth's model of parts 4 and 5 (MaxNet, regmult 2, classes lqh) projected to a simple scenario of a warmer and
drier climate (the maximum temperature of the warmest month 2 degrees higher, the rain of the driest quarter 20 %
lower); suitability today and in the scenario, gains and losses at the threshold of maximum TSS; and where the
projection extrapolates: MESS (sdm.mess), MOP (sdm.mop), ExDet (sdm.exdet), and the effect of clamping.
"""

from __future__ import annotations

import numpy as np
from matplotlib import colors as mcolors

import sdmfig as S
from sdmfig import figs, plt

SLUG = "projection-and-extrapolation"
IMG = figs.img_dir(f"{S.CAT}/{SLUG}")
DAT = figs.data_dir(f"{S.CAT}/{SLUG}")
FIT = {"model": "maxnet", "regmult": 2, "classes": "lqh"}
WARM = 20       # bio5 is in tenths of a degree: +2 degrees
DRY = 0.8       # bio17 times 0.8


def model() -> str:
    m = S.prepared()
    m += S.node("fit", "sdm.fit", {"records": "data.records"}, FIT)
    m += S.node("today", "sdm.maxnet_predict", {"model": "fit.model", "env": "data.env"})
    m += S.node("hotter", "raster.calc", {"e": "data.env"}, {"expression": f"e.bio5 + {WARM}"})
    m += S.node("drier", "raster.calc", {"e": "data.env"}, {"expression": f"e.bio17 * {DRY}"})
    m += S.node("future_t", "stack.set", {"stack": "data.env", "map": "hotter.map"}, {"name": "bio5"})
    m += S.node("future", "stack.set", {"stack": "future_t.stack", "map": "drier.map"}, {"name": "bio17"})
    m += S.node("tomorrow", "sdm.maxnet_predict", {"model": "fit.model", "env": "future.stack"})
    m += S.node("unclamped", "sdm.maxnet_predict", {"model": "fit.model", "env": "future.stack"}, {"clamp": False})
    m += S.node("shift", "raster.calc", {"a": "today.prediction", "b": "tomorrow.prediction"}, {"expression": "b - a"})
    m += S.node("mess", "sdm.mess", {"reference": "data.records", "env": "future.stack"}, {"full": True})
    m += S.node("mess_now", "sdm.mess", {"reference": "data.records", "env": "data.env"})
    m += S.node("mop", "sdm.mop", {"reference": "data.records", "env": "future.stack"}, {"type": "detailed"})
    m += S.node("exdet", "sdm.exdet", {"reference": "data.records", "env": "future.stack"})
    writes = {"today.prediction": "today.tif", "tomorrow.prediction": "tomorrow.tif", "unclamped.prediction":
              "unclamped.tif", "shift.map": "shift.tif", "mess.result": "mess.tif", "mess_now.result": "mess_now.tif",
              "mop.result": "mop.tif", "exdet.result": "exdet.tif"}
    for i, (ref, path) in enumerate(writes.items()):
        m += S.node(f"w{i}", "raster.write", {"value": ref}, {"path": path})
    m += S.node("w_area", "vector.write", {"vector": "data.area"}, {"path": "area.gpkg"})
    m += S.outputs({"records": "data.records", "threshold": "fit.threshold", "summary": "fit.summary"})
    return m


def layer_names(path):
    import rasterio
    with rasterio.open(path) as ds:
        return list(ds.descriptions)


def surface_map(a, info, png, area=None, cmap="viridis", vmin=0, vmax=1, label="Suitability", norm=None,
                legend=True, records=None):
    fig, ax = S.map_axes(640, legend_frac=0.16 if legend else 0.0)
    S.base(ax)
    im = S.surface(ax, a, info, cmap=cmap, vmin=None if norm else vmin, vmax=None if norm else vmax, norm=norm)
    if area is not None:
        area.plot(ax=ax, facecolor="none", edgecolor=figs.INK, linewidth=0.6)
    S.base_limits(ax)
    if records is not None:
        q = records["presence"] == 1
        S.points(ax, records["x"][q], records["y"][q], size=6, lw=0.3)
    if legend:
        S.colorbar(fig, im, label, rect=(0.86, 0.3, 0.03, 0.45))
    S.save(fig, png)


def main():
    import geopandas as gpd
    w = S.work(SLUG)
    S.run(model(), w)
    area = gpd.read_file(w / "area.gpkg")
    rec = S.table(w, "records.csv")
    thr = float(S.out(w, "threshold.json"))
    print("threshold", thr, "summary", S.out(w, "summary.csv")[0])
    today, info = figs.read(w / "today.tif")
    tomorrow, _ = figs.read(w / "tomorrow.tif")
    unclamped, _ = figs.read(w / "unclamped.tif")
    shift, _ = figs.read(w / "shift.tif")
    # today and tomorrow with the same framing and no legend, for the comparison
    surface_map(today, info, IMG / "today.png", area, legend=False, records=rec)
    surface_map(tomorrow, info, IMG / "tomorrow.png", area, legend=False, records=rec)
    m = float(np.nanmax(np.abs(shift)))
    surface_map(shift, info, IMG / "shift.png", area, cmap="RdBu", label="Change of suitability",
                norm=mcolors.TwoSlopeNorm(vmin=-m, vcenter=0, vmax=m))
    # gains and losses at the threshold of maximum TSS
    land = ~np.ma.getmaskarray(today)
    t0 = today.filled(np.nan) >= thr
    t1 = tomorrow.filled(np.nan) >= thr
    cls = np.full(today.shape, np.nan)
    cls[land] = 0
    cls[land & t0 & t1] = 1
    cls[land & t0 & ~t1] = 2
    cls[land & ~t0 & t1] = 3
    from rasterio import features as rf
    inside = rf.geometry_mask(area.geometry, out_shape=today.shape, transform=info["transform"], invert=True)
    counts = {k: int(np.sum((cls == k) & inside)) for k in (1, 2, 3)}
    counts_all = {k: int(np.sum(cls == k)) for k in (1, 2, 3)}
    print("cells inside the area: kept, lost, gained", counts, "everywhere", counts_all)
    pal = {0: ("Unsuitable in both", "#e3e6dc"), 1: ("Suitable in both", figs.FOREST),
           2: ("Lost", "#c0392b"), 3: ("Gained", figs.WATER)}
    figs.raster(None, IMG / "gains-losses.png", array=np.ma.masked_invalid(cls), palette=pal, width=700)
    # extrapolation
    names = layer_names(w / "mess.tif")
    print("mess layers", names)
    mess = {n: figs.read(w / "mess.tif", i + 1)[0] for i, n in enumerate(names)}
    mmess = mess.get("mess", mess[names[-1]])
    now = figs.read(w / "mess_now.tif")[0]
    neg_future = int(np.sum(mmess.filled(1) < 0))
    neg_now = int(np.sum(now.filled(1) < 0))
    neg_in = int(np.sum((mmess.filled(1) < 0) & inside))
    print("MESS < 0 cells: now", neg_now, "future", neg_future, "future inside area", neg_in, "land", int(land.sum()))
    for n in names:
        if n != "mess":
            print("  per predictor", n, int(np.sum(mess[n].filled(1) < 0)))
    # a scale that makes every negative (novel) cell visible: red below 0, blue above
    norm = mcolors.TwoSlopeNorm(vmin=-20, vcenter=0, vmax=100)
    surface_map(np.clip(mmess, -20, 100), info, IMG / "mess.png", area, cmap="RdBu", norm=norm,
                label="MESS (below 0: novel)")
    surface_map(np.clip(now, -20, 100), info, IMG / "mess-now.png", area, cmap="RdBu", norm=norm,
                label="MESS (below 0: novel)")
    mop_names = layer_names(w / "mop.tif")
    print("mop layers", mop_names)
    mop = {n: figs.read(w / "mop.tif", i + 1)[0] for i, n in enumerate(mop_names)}
    for n, a in mop.items():
        v = a.compressed()
        print("  mop", n, "min", np.nanmin(v) if v.size else None, "max", np.nanmax(v) if v.size else None,
              "nonzero", int(np.sum(v != 0)))
    ex_names = layer_names(w / "exdet.tif")
    print("exdet layers", ex_names)
    ex = {n: figs.read(w / "exdet.tif", i + 1)[0] for i, n in enumerate(ex_names)}
    for n, a in ex.items():
        v = a.compressed()
        print("  exdet", n, "min", float(np.nanmin(v)), "max", float(np.nanmax(v)), "n<0", int(np.sum(v < 0)),
              "n>1", int(np.sum(v > 1)))
    # maps of MOP's distance and ExDet's NT1 and NT2
    first = mop_names[0]
    surface_map(mop[first], info, IMG / "mop.png", area, cmap="magma_r", vmin=float(np.nanmin(mop[first])),
                vmax=float(np.nanmax(mop[first])), label="MOP distance")
    e = ex["exdet"]
    surface_map(np.clip(e, -0.5, 1.5), info, IMG / "exdet.png", area, cmap="RdBu",
                norm=mcolors.TwoSlopeNorm(vmin=-0.5, vcenter=0, vmax=1.5),
                label="ExDet: NT1 below 0, NT2 from 0")
    mic = ex["mic"]
    preds = [n.replace("_mess", "") for n in names if n != "mess"]
    novel = e.filled(1) < 0
    for k, n in enumerate(preds):
        print("  mic", k + 1, n, "novel cells", int(np.sum(novel & (mic.filled(-1) == k + 1))))
    # clamping
    d = unclamped - tomorrow
    k = float(np.nanmax(np.abs(d)))
    print("clamping: largest difference", k, "cells differing > 0.05", int(np.sum(np.abs(d.filled(0)) > 0.05)))
    surface_map(d, info, IMG / "clamp-diff.png", area, cmap="RdBu", label="Unclamped minus clamped",
                norm=mcolors.TwoSlopeNorm(vmin=-max(k, 1e-6), vcenter=0, vmax=max(k, 1e-6)))
    # the predictors today and in the scenario at the records, for a chart
    figs.write_csv(DAT / "summary.csv", [{"what": "Suitable in both", "cells": counts[1]},
                                         {"what": "Lost", "cells": counts[2]}, {"what": "Gained", "cells": counts[3]}])


if __name__ == "__main__":
    main()
