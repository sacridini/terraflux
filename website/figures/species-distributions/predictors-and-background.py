"""Figures of "Predictors and the accessible area" (tutorials/species-distributions/predictors-and-background).

    python website/figures/species-distributions/predictors-and-background.py

The nine bioclimatic variables; their variance inflation factors over the continent and inside the accessible
area (sdm.vif), the selection of predictors (sdm.select_predictors) and the range of autocorrelation
(sdm.autocorrelation_range); the accessible area (sdm.accessible_area) and background points (sdm.background);
and a model's scores for accessible areas of several sizes (sdm.prepare, sdm.fit).
"""

from __future__ import annotations

import numpy as np
import matplotlib.pyplot as plt

import sdmfig as S
from sdmfig import figs

SLUG = "predictors-and-background"
IMG = figs.img_dir(f"{S.CAT}/{SLUG}")
DAT = figs.data_dir(f"{S.CAT}/{SLUG}")
ORDER = ["bio1", "bio5", "bio6", "bio7", "bio8", "bio9", "bio12", "bio16", "bio17"]


def predictors_panel():
    fig, axes = plt.subplots(3, 3, figsize=(7.4, 7.9), dpi=150)
    for b, (ax, name) in enumerate(zip(axes.flat, ORDER), start=1):
        a, info = figs.read(S.DATA / "bioclim.tif", b)
        temp = name not in ("bio12", "bio16", "bio17")
        v = a / 10 if temp else a
        cm = plt.get_cmap("RdYlBu_r" if temp else "YlGnBu").copy()
        cm.set_bad((1, 1, 1, 0))
        im = ax.imshow(np.ma.masked_invalid(v), cmap=cm, extent=S.extent(info), interpolation="nearest",
                       vmax=None if temp else np.nanpercentile(v.compressed(), 98))
        ax.set_axis_off()
        ax.set_title(name, fontsize=10.5, loc="left")
        cb = fig.colorbar(im, ax=ax, fraction=0.05, pad=0.01)
        cb.outline.set_visible(False)
        cb.ax.grid(False)
        cb.ax.tick_params(labelsize=7.5)
        cb.set_label("°C" if temp else "mm", fontsize=8)
    fig.tight_layout(h_pad=0.6, w_pad=0.4)
    S.save(fig, IMG / "predictors.png")


def steps():
    """Step by step: cleaned, one per cell, thinned, the area, the VIFs, the selection, the background."""
    w = S.work(SLUG)
    m = S.HEAD + S.READ
    m += S.node("clean", "sdm.clean_occurrences", {"occurrences": "records.vector"})
    m += S.node("cells", "sdm.one_per_cell", {"occurrences": "clean.occurrences", "raster": "climate.stack"})
    m += S.node("thin", "sdm.thin", {"occurrences": "cells.occurrences"}, {"distance": 50000})
    m += S.node("area", "sdm.accessible_area", {"occurrences": "thin.occurrences"}, {"distance": 500000})
    m += S.node("hull", "sdm.accessible_area", {"occurrences": "thin.occurrences"}, {"method": "convex_hull"})
    m += S.node("vif_all", "sdm.vif", {"env": "climate.stack"}, {"seed": 1})
    m += S.node("vif_area", "sdm.vif", {"env": "climate.stack", "area": "area.area"}, {"seed": 1})
    m += S.node("select", "sdm.select_predictors", {"env": "climate.stack", "area": "area.area"}, {"seed": 1})
    m += S.node("select5", "sdm.select_predictors", {"env": "climate.stack", "area": "area.area"},
                {"vif": 5, "seed": 1})
    m += S.node("vif_kept", "sdm.vif", {"env": "select.env", "area": "area.area"}, {"seed": 1})
    m += S.node("auto", "sdm.autocorrelation_range", {"env": "select.env"})
    m += S.node("background", "sdm.background",
                {"env": "select.env", "occurrences": "thin.occurrences", "area": "area.area"}, {"n": 2000, "seed": 1})
    m += S.node("bg_all", "sdm.background", {"env": "select.env", "occurrences": "thin.occurrences"},
                {"n": 2000, "seed": 1})
    m += S.node("w_area", "vector.write", {"vector": "area.area"}, {"path": "area.gpkg"})
    m += S.node("w_hull", "vector.write", {"vector": "hull.area"}, {"path": "hull.gpkg"})
    m += S.node("w_bg", "vector.write", {"vector": "background.points"}, {"path": "background.gpkg"})
    m += S.node("w_bga", "vector.write", {"vector": "bg_all.points"}, {"path": "bg_all.gpkg"})
    m += S.node("w_thin", "vector.write", {"vector": "thin.occurrences"}, {"path": "thin.gpkg"})
    m += S.outputs({"vif_all": "vif_all.vif", "vif_area": "vif_area.vif", "selection": "select.selection",
                    "selection5": "select5.selection", "vif_kept": "vif_kept.vif", "range": "auto.range",
                    "block": "auto.block_size", "layers": "auto.layers", "models": "auto.models"})
    S.run(m, w)
    import geopandas as gpd
    area = gpd.read_file(w / "area.gpkg")
    hull = gpd.read_file(w / "hull.gpkg")
    bg = gpd.read_file(w / "background.gpkg")
    bga = gpd.read_file(w / "bg_all.gpkg")
    th = gpd.read_file(w / "thin.gpkg")
    lon = th["lon"].astype(float)
    lat = th["lat"].astype(float)
    km2 = float(area.to_crs("EPSG:6933").area.sum()) / 1e6
    hull_km2 = float(hull.to_crs("EPSG:6933").area.sum()) / 1e6

    for name, pts, poly in (("background_area", bg, area), ("background_all", bga, None)):
        fig, ax = S.map_axes(760, legend_frac=0.28)
        S.base(ax)
        if poly is not None:
            poly.plot(ax=ax, facecolor="#e8901f22", edgecolor=S.FLAG, linewidth=1.0)
            S.base_limits(ax)
        S.points(ax, pts.geometry.x, pts.geometry.y, color=S.BACK, size=2.5, edge="none", alpha=0.7)
        S.points(ax, lon, lat, size=12)
        items = [("Records (83)", S.PRES, "o"), ("Background (2 000)", S.BACK, "o")]
        if poly is not None:
            items.append(("Accessible area", "#f6dcb8", "s"))
        S.legend(fig, items, loc=(0.72, 0.6))
        S.save(fig, IMG / f"{name}.png")

    vif_all = S.out(w, "vif_all.csv")[0]
    vif_area = S.out(w, "vif_area.csv")[0]
    vif_kept = S.out(w, "vif_kept.csv")[0]
    sel = S.out(w, "selection.json")
    sel5 = S.out(w, "selection5.json")

    def draw(ax):
        x = np.arange(len(ORDER))
        va = [min(float(vif_all[k]), 1e6) for k in ORDER]
        vb = [min(float(vif_area[k]), 1e6) for k in ORDER]
        vk = [float(vif_kept[k]) if k in vif_kept else 0 for k in ORDER]
        ax.bar(x - 0.27, va, 0.27, color="#b9c2bc", label="Whole map")
        ax.bar(x, vb, 0.27, color=figs.WATER, label="Accessible area (500 km)")
        ax.bar(x + 0.27, vk, 0.27, color=figs.FOREST, label="Kept, after the selection")
        ax.set_yscale("log")
        ax.axhline(10, color=figs.EMBER, lw=1.2, ls="--")
        ax.text(len(ORDER) - 0.5, 11.5, "threshold 10", ha="right", fontsize=8.5, color=figs.EMBER)
        ax.set_xticks(x, ORDER)
        ax.set_ylabel("Variance inflation factor (log)")
        ax.set_ylim(0.5, 2e6)
        ax.legend(loc="upper right", fontsize=8.5)
    figs.chart(IMG / "vif.png", draw, width=900, height=440)
    layers = S.out(w, "layers.csv")
    models = S.out(w, "models.json")
    return dict(km2=km2, hull_km2=hull_km2, n_bg=len(bg), vif_all=vif_all, vif_area=vif_area, vif_kept=vif_kept,
                sel=sel, sel5=sel5, range=S.out(w, "range.json"), block=S.out(w, "block.json"), layers=layers,
                models=models)


def areas():
    """A model fitted with the background drawn from accessible areas of several sizes."""
    dists = [250, 500, 1000, 2000, 0]
    rows = []
    w = S.work(SLUG + "_areas")
    m = S.HEAD + S.READ
    outs = {}
    for d in dists:
        p = dict(S.CAREFUL)
        p["area"] = d * 1000 if d else False
        m += S.node(f"data_{d}", "sdm.prepare", {"occurrences": "records.vector", "env": "climate.stack"}, p)
        m += S.node(f"fit_{d}", "sdm.fit", {"records": f"data_{d}.records"})
        m += S.node(f"area_w_{d}", "vector.write", {"vector": f"data_{d}.area"}, {"path": f"area_{d}.gpkg"})
        m += S.node(f"bg_w_{d}", "vector.write", {"vector": f"data_{d}.background_points"}, {"path": f"bg_{d}.gpkg"})
        outs[f"s_{d}"] = f"fit_{d}.summary"
        outs[f"set_{d}"] = f"data_{d}.settings"
    m += S.outputs(outs)
    S.run(m, w)
    import geopandas as gpd
    for d in dists:
        s = S.out(w, f"s_{d}.csv")[0]
        st = S.out(w, f"set_{d}.json")
        a = gpd.read_file(w / f"area_{d}.gpkg")
        km2 = float(a.to_crs("EPSG:6933").area.sum()) / 1e6 if d else float("nan")
        bg = gpd.read_file(w / f"bg_{d}.gpkg")
        fig, ax = S.map_axes(640)
        S.base(ax)
        if d:
            a.plot(ax=ax, facecolor="#e8901f22", edgecolor=S.FLAG, linewidth=1.0)
            S.base_limits(ax)
        S.points(ax, bg.geometry.x, bg.geometry.y, color=S.BACK, size=2, edge="none", alpha=0.7)
        ax.text(-123, -50, (f"{d} km" if d else "Whole map") + f" | AUC {float(s['auc_test_mean']):.2f}",
                fontsize=12, fontweight="bold", color=figs.INK, va="bottom")
        S.save(fig, IMG / f"area_{d}.png")
        rows.append({"area": f"{d} km" if d else "whole map", "auc_test": round(float(s["auc_test_mean"]), 3),
                     "tss": round(float(s["tss_mean"]), 3), "boyce": round(float(s["boyce_mean"]), 3),
                     "predictors": " ".join(st["predictors_used"]), "km2": round(km2 / 1e6, 1) if d else ""})
    figs.write_csv(DAT / "areas.csv", [{k: r[k] for k in ("area", "auc_test", "tss", "boyce")} for r in rows])
    return rows


if __name__ == "__main__":
    predictors_panel()
    r = steps()
    for k, v in r.items():
        print(k, v)
    for row in areas():
        print(row)
