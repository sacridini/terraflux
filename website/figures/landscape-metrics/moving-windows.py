"""Figures of "Moving windows": the CORINE land cover around Hannover in seven classes at 100 m, its Shannon
diversity and edge density in windows of growing size (a slider with a linked chart), and square against
circular windows."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _lm as L  # noqa: E402
from _lm import figs, np  # noqa: E402

SLUG = "landscape-metrics/moving-windows"
w = L.work("moving-windows")
img = figs.img_dir(SLUG)
dat = figs.data_dir(SLUG)

BASE = (L.HEAD + L.node("corine", "raster.read", params={"path": "clc.tif"})
        + L.node("broad", "raster.reclassify", {"map": "corine.map"}, {"rules": L.CLC7_RULES})
        + L.node("cover", "raster.resample", {"raster": "broad.map"}, {"resolution": 100, "resampling": "mode"}))

L.run(BASE, w, {"cover.raster": "cover100.tif"}, name="cover.toml")
figs.raster(w / "cover100.tif", img / "cover.png", palette=L.CLC7, counts=True, width=1000)

rows = []
frames = []
SIDES = [3, 5, 9, 15, 21, 31, 41]
for side in SIDES:
    model = BASE + L.node("window", "metrics.window", {"map": "cover.raster"},
                          {"metrics": ["shdi", "ed", "pr"], "window": side})
    L.run(model, w, {"window.maps": f"win_{side}.tif"}, name=f"win_{side}.toml")
    shdi = figs.read(w / f"win_{side}.tif", 1)[0]
    ed = figs.read(w / f"win_{side}.tif", 2)[0]
    pr = figs.read(w / f"win_{side}.tif", 3)[0]
    total = shdi.size
    row = {"window_km": round(side * 0.1, 1), "cells": side, "shdi_mean": round(float(shdi.mean()), 3),
           "shdi_sd": round(float(shdi.std()), 3), "ed_mean": round(float(ed.mean()), 1),
           "pr_mean": round(float(pr.mean()), 2), "na_percent": round(100 * (1 - shdi.count() / total), 1),
           "shdi_zero": round(100 * float((shdi == 0).sum()) / shdi.count(), 1)}
    rows.append(row)
    print(row)
    figs.raster(w / f"win_{side}.tif", img / f"shdi_{side}.png", band=1, cmap="viridis", vmin=0, vmax=1.6,
                unit="Shannon diversity (SHDI)", width=760)
    figs.raster(w / f"win_{side}.tif", img / f"ed_{side}.png", band=2, cmap="magma", vmin=0, vmax=60,
                unit="edge density (m/ha)", width=760)
figs.write_csv(dat / "window_size.csv", rows)

# square and circle windows of 21 cells
model = BASE + L.node("window", "metrics.window", {"map": "cover.raster"},
                      {"metrics": ["shdi"], "window": 21, "shape": "circle"})
L.run(model, w, {"window.maps": "win_21_circle.tif"}, name="circle.toml")
figs.raster(w / "win_21_circle.tif", img / "shdi_21_circle.png", band=1, cmap="viridis", vmin=0, vmax=1.6,
            legend=False, width=640)
figs.raster(w / "win_21.tif", img / "shdi_21_square.png", band=1, cmap="viridis", vmin=0, vmax=1.6,
            legend=False, width=640)
c = figs.read(w / "win_21_circle.tif")[0]
s = figs.read(w / "win_21.tif")[0]
print("circle mean", float(c.mean()), "square mean", float(s.mean()), "corr", float(np.ma.corrcoef(c.ravel(), s.ravel())[0, 1]))
