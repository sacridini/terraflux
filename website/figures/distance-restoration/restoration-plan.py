"""Figures of tutorials/distance-restoration/restoration-plan: the restoration plan for the extinction risk of
Serra Verde's species: area targets, exact vs steps vs uniform, the trade-off curve and z."""

import importlib.util
import pathlib

import numpy as np

import _common as C
from _common import figs

SLUG = "distance-restoration/restoration-plan"
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

# the habitat part of the model is the previous tutorial's
spec = importlib.util.spec_from_file_location("prev", pathlib.Path(__file__).with_name("_habitat_model.py"))
prev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prev)
HABITAT = prev.HABITAT

PLAN = """
[[nodes]]
id = "plan"
op = "restoration.plan"
inputs = { restorable = "vegetation.anthropic", table = "habitat.table", gain = "habitat.gain" }
params = { PARAMS }
"""
W = 700


def run(name, params, writes=None):
    w = figs.workdir(f"{SLUG}/{name}")
    C.serra(w, "land_cover", "past_cover", "features", "ecoregions")
    m = HABITAT + PLAN.replace("PARAMS", params)
    wr = {"plan.summary": "summary.csv", "plan.species": "species.csv", "plan.restored": "restored.tif"}
    wr.update(writes or {})
    figs.run(figs.add_writes(m, wr), w)
    return w, C.table(w, "summary")[0]


def plan_map(w, png, legend=True, vmax=1):
    r = figs.read(w / "restored.tif")[0]
    C.share_map(r, png, vmax=vmax, width=W)
    return r


# ------------------------------------------------------------------ the first plan
w100, s100 = run("t100", "target = 100, z_sd = 0.1", {"vegetation.anthropic": "anthropic.tif"})
print("100 km2", s100)
plan_map(w100, out / "plan_100.png")
anth = figs.read(w100 / "anthropic.tif")[0]
figs.raster(w100 / "anthropic.tif", out / "restorable.png", array=np.ma.masked_where(anth <= 0, anth), cmap="YlOrBr",
            vmin=0, vmax=1, unit="share that can be restored", width=W)
sp = C.table(w100, "species")
rows = []
for r in sp:
    s = C.SPECIES[int(r["species"]) - 1]
    rows.append({"species": C.NAMES[s], "before": round(r["risk_before"], 4), "after": round(r["risk_after"], 4),
                 "gained": round(r["gained"], 1)})
figs.write_csv(data / "species_100.csv", rows)
print(rows)
rr = figs.read(w100 / "restored.tif")[0]
print("cells restored", int((rr > 1e-6).sum()), "in full", int((np.abs(rr - anth) < 1e-6).sum() if True else 0))

# ------------------------------------------------------------------ the target: exact, steps and uniform
targets = [25, 50, 100, 150, 200, 300, 400, 600]
curve, frames = [], []
for t in targets:
    w, s = run(f"exact_{t}", f"target = {t}, z_sd = 0.1")
    _, su = run(f"uniform_{t}", f"target = {t}, method = \"uniform\"")
    _, ss = run(f"steps_{t}", f"target = {t}, method = \"steps\", steps = 10")
    plan_map(w, out / f"target_{t}.png")
    r = figs.read(w / "restored.tif")[0]
    curve.append({"target": t, "exact": round(s["risk_reduction"], 4), "steps": round(ss["risk_reduction"], 4),
                  "uniform": round(su["risk_reduction"], 4)})
    frames.append({"target": t, "reduction": s["risk_reduction"], "sd": s["sd"],
                   "cells": int((r > 1e-6).sum())})
figs.write_csv(data / "target.csv", curve)
print("curve", curve)
print("frames", frames)
figs.write_csv(data / "frames.csv", [{k: round(v, 4) for k, v in f.items()} for f in frames])

# the three methods' maps at 100 km2
for meth, p in (("steps", 'target = 100, method = "steps", steps = 10'), ("uniform", 'target = 100, method = "uniform"')):
    w, s = run(f"{meth}_map", p)
    plan_map(w, out / f"method_{meth}.png", legend=False, vmax=1)
    print(meth, s)
plan_map(w100, out / "method_exact.png", legend=False)
C.share_map(figs.read(figs.WEB / f".cache/work/{SLUG}/uniform_map/restored.tif")[0], out / "method_uniform_scaled.png",
            vmax=0.12, width=W)

# ------------------------------------------------------------------ a priority gradient
wl, sl = run("levels", "target = 200, levels = [0.25, 0.5, 0.75, 1]",
             {"plan.tradeoff": "tradeoff.csv", "plan.gradient": "gradient.tif"})
trade = C.table(wl, "tradeoff")
print("tradeoff", trade)
g = figs.read(wl / "gradient.tif")[0]
gm = np.ma.masked_where(g.filled(0) <= 0, g)
gland = np.ma.masked_where(~C.land_mask(), g.filled(0))
figs.raster(wl / "gradient.tif", out / "gradient.png", array=gland,
            palette={4: ("In the first 50 km²", "#1b5e3a"), 3: ("By 100 km²", "#4f9a62"),
                     2: ("By 150 km²", "#9cc98a"), 1: ("Only at 200 km²", "#e2edb8"), 0: ("Not restored", "#e6e6e2")},
            counts=False, width=W)
u, c = np.unique(gm.compressed(), return_counts=True)
print("gradient cells", dict(zip(u.tolist(), c.tolist())))
figs.write_csv(data / "tradeoff.csv", [{"area": round(r["area"], 1), "benefit": round(r["benefit"], 4)} for r in trade])

# ------------------------------------------------------------------ z
zrows = []
for z in (0.15, 0.25, 0.35):
    w, s = run(f"z{z}", f"target = 100, z = {z}")
    sp = C.table(w, "species")
    tam = next(r for r in sp if int(r["species"]) == 5)
    man = next(r for r in sp if int(r["species"]) == 8)
    zrows.append({"z": z, "reduction": round(s["risk_reduction"], 4), "tamarin": round(tam["gained"], 1),
                  "mangrove": round(man["gained"], 1)})
print("z", zrows)
figs.write_csv(data / "z.csv", zrows)
for how in ("correlated", "independent", "linear"):
    _, s = run(f"unc_{how}", f'target = 100, z_sd = 0.1, uncertainty = "{how}"')
    print("uncertainty", how, s["risk_reduction"], s["sd"])
print("done")
