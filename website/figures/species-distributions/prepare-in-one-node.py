"""Figures of "The whole preparation in one node" (tutorials/species-distributions/prepare-in-one-node).

    python website/figures/species-distributions/prepare-in-one-node.py

sdm.prepare on the sloth's records with growing sets of settings: its report and settings outputs, the records each
step removes drawn on a map, and the scores of the same MaxNet model (regmult 2, classes lqh) after each preparation.
"""

from __future__ import annotations

import json

import numpy as np

import sdmfig as S
from sdmfig import figs

SLUG = "prepare-in-one-node"
IMG = figs.img_dir(f"{S.CAT}/{SLUG}")
DAT = figs.data_dir(f"{S.CAT}/{SLUG}")
FIT = {"model": "maxnet", "regmult": 2, "classes": "lqh"}
SCORES = ("auc_test", "tss", "boyce", "or_10p")

RECIPES = {
    "Defaults": {"background": 5000},
    "Thinned at 50 km": {"thin": 50000, "background": 5000},
    "Area of 500 km": {"thin": 50000, "area": 500000, "background": 2000},
    "Blocks of 500 km": {"thin": 50000, "area": 500000, "background": 2000, "folds": {"size": 500000, "k": 5}},
    "Random folds": {"thin": 50000, "area": 500000, "background": 2000, "folds": {"method": "random", "k": 5}},
}


def recipes():
    w = S.work(SLUG + "_recipes")
    m = S.HEAD + S.READ
    outs = {}
    for i, (name, p) in enumerate(RECIPES.items()):
        p = dict(p, seed=1)
        m += S.node(f"data{i}", "sdm.prepare", {"occurrences": "records.vector", "env": "climate.stack"}, p)
        m += S.node(f"fit{i}", "sdm.fit", {"records": f"data{i}.records"}, FIT)
        outs[f"report{i}"] = f"data{i}.report"
        outs[f"settings{i}"] = f"data{i}.settings"
        outs[f"summary{i}"] = f"fit{i}.summary"
    m += S.outputs(outs)
    S.run(m, w)
    rows = []
    for i, name in enumerate(RECIPES):
        rep = S.out(w, f"report{i}.csv") if (w / "output" / f"report{i}.csv").exists() else S.out(w, f"report{i}.json")
        print(name, "report", rep)
        if i == len(RECIPES) - 2:
            print("settings", json.dumps(S.out(w, f"settings{i}.json"), indent=1)[:3000])
        s = S.out(w, f"summary{i}.csv")[0]
        r = {"preparation": name}
        r.update({k: round(float(s[f"{k}_mean"]), 3) for k in SCORES})
        rows.append(r)
        print(r)
    figs.write_csv(DAT / "recipes.csv", rows)
    return rows


def steps_map():
    """The 116 records and the step that removed each, the area and the background of the careful recipe."""
    w = S.work(SLUG + "_steps")
    base = {"area": False, "predictors": False, "background": 100, "folds": False, "seed": 1}
    m = S.HEAD + S.READ
    variants = {"clean": dict(base, one_per_cell=False), "cell": dict(base),
                "thin": dict(base, thin=50000),
                "full": {"thin": 50000, "area": 500000, "background": 2000, "folds": {"size": 500000, "k": 5}, "seed": 1}}
    outs = {}
    for k, p in variants.items():
        m += S.node(f"d_{k}", "sdm.prepare", {"occurrences": "records.vector", "env": "climate.stack"}, p)
        outs[f"p_{k}"] = f"d_{k}.presences"
    m += S.node("w_area", "vector.write", {"vector": "d_full.area"}, {"path": "area.gpkg"})
    m += S.node("w_bg", "vector.write", {"vector": "d_full.background_points"}, {"path": "background.gpkg"})
    m += S.node("w_pres", "vector.write", {"vector": "d_full.presences"}, {"path": "presences.gpkg"})
    for k in ("clean", "cell", "thin"):
        m += S.node(f"w_{k}", "vector.write", {"vector": f"d_{k}.presences"}, {"path": f"{k}.gpkg"})
    m += S.outputs({"report": "d_full.report"})
    S.run(m, w)
    import geopandas as gpd
    raw = figs.read_csv(w / "bradypus.csv")
    xy_raw = np.array([[float(r["lon"]), float(r["lat"])] for r in raw])
    sets = {k: gpd.read_file(w / f"{k}.gpkg") for k in ("clean", "cell", "thin")}
    pts = {k: {(round(float(a), 4), round(float(b), 4)) for a, b in zip(v["lon"], v["lat"])} for k, v in sets.items()}
    key = [(round(x, 4), round(y, 4)) for x, y in xy_raw]
    status = []
    for k in key:
        if k not in pts["clean"]:
            status.append("cleaning")
        elif k not in pts["cell"]:
            status.append("one per cell")
        elif k not in pts["thin"]:
            status.append("thinning")
        else:
            status.append("kept")
    from collections import Counter
    print("status", Counter(status))
    area = gpd.read_file(w / "area.gpkg")
    bg = gpd.read_file(w / "background.gpkg")
    fig, ax = S.map_axes(760, legend_frac=0.24)
    S.base(ax)
    area.plot(ax=ax, facecolor="#2f6f9f14", edgecolor=figs.WATER, linewidth=0.9)
    S.points(ax, bg.geometry.x, bg.geometry.y, color=S.BACK, size=2, edge="none", alpha=0.35, z=2)
    st = np.array(status)
    cols = {"kept": S.PRES, "thinning": figs.EMBER, "one per cell": "#785ef0", "cleaning": "#1c2420"}
    marks = {"kept": "o", "thinning": "o", "one per cell": "s", "cleaning": "X"}
    for k, c in cols.items():
        q = st == k
        S.points(ax, xy_raw[q, 0], xy_raw[q, 1], color=c, size=22 if k != "kept" else 16, marker=marks[k], lw=0.5, z=4)
    S.base_limits(ax)
    ax.set_xlim(-100, -30)
    ax.set_ylim(-35, 25)
    n = Counter(status)
    S.legend(fig, [(f"Kept ({n['kept']})", S.PRES, "o"), (f"Thinned out ({n['thinning']})", figs.EMBER, "o"),
                   (f"Same cell ({n['one per cell']})", "#785ef0", "s"), (f"Failed a test ({n['cleaning']})", "#1c2420", "X"),
                   ("Background (2 000)", S.BACK, "o"), ("Accessible area", "#2f6f9f55", "s")], loc=(0.77, 0.5))
    S.save(fig, IMG / "steps.png")
    print("report", S.out(w, "report.csv") if (w / "output" / "report.csv").exists() else S.out(w, "report.json"))


if __name__ == "__main__":
    import sys
    for t in sys.argv[1:] or ["steps_map", "recipes"]:
        print(t, globals()[t]())
