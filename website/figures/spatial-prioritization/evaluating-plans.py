"""Figures of 'Evaluating, comparing and reporting plans' (spatial-prioritization/evaluating-plans).

prioritize.evaluate on Serra Verde's 30 % minimum set (coverage, the importance of the selected cells: replacement
cost, Ferrier's irreplaceability, rarity-weighted richness, rank importance; extinction risk), a gap analysis of the
protected areas, prioritize.calibrate (blended and hierarchical calibration of the boundary penalty, a Pareto front of
budget and shortfall), and Zonation's best cells against the optimized plan at the same cost.

    python website/figures/spatial-prioritization/evaluating-plans.py [importance gap_analysis blended hierarchical pareto zonation]
"""

import time

import numpy as np
from scipy.stats import spearmanr

import sp_common as sp
from sp_common import figs, plt

SLUG = "evaluating-plans"
OUT = sp.img(SLUG)
DAT = sp.data(SLUG)
COST = sp.grid(sp.DATA / "cost.tif")
PLAN = '''
[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = { features = "features.stack", cost = "cost.map" }
params = { objective = "min_set", relative_targets = 0.3, gap = 0.001 }
'''


def tbl(w, name):
    return sp.table(w, name)


# ------------------------------------------------------------------------------------------------ importance
def importance():
    t = time.time()
    w = sp.run(SLUG, "importance", PLAN + '''
[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = { features = "features.stack", cost = "cost.map", solution = "plan.solution" }
params = { relative_targets = 0.3, replacement = true, rank = 10, extinction_z = 0.25, gap = 0.001 }

[[nodes]]
id = "ferrier_total"
op = "map.calc"
inputs = { f = "evaluation.ferrier" }
params = { expression = "f.total" }

[outputs.summary]
from = "evaluation.summary"
[outputs.coverage]
from = "evaluation.coverage"
[outputs.risk]
from = "evaluation.extinction_risk"
[outputs.ranks]
from = "evaluation.rank_budgets"
''', {"plan.solution": "solution.tif", "evaluation.replacement": "replacement.tif", "ferrier_total.map": "ferrier.tif",
      "evaluation.rarity": "rarity.tif", "evaluation.rank": "rank.tif"})
    print("importance run", round(time.time() - t, 1), "s")
    print("summary", tbl(w, "summary"))
    print("risk", tbl(w, "risk"))
    print("ranks", tbl(w, "ranks"))
    sol = sp.grid(w / "solution.tif") > 0.5
    maps = {k: sp.grid(w / f"{k}.tif") for k in ("replacement", "ferrier", "rarity", "rank")}
    titles = {"replacement": "Replacement cost", "ferrier": "Ferrier's irreplaceability", "rarity": "Rarity-weighted richness",
              "rank": "Rank importance"}
    arrays = []
    for k, a in maps.items():
        v = np.where(sol, a, np.nan)
        lo, hi = np.nanmin(v), np.nanmax(v)
        arrays.append((v - lo) / (hi - lo) if hi > lo else v)
        print(k, "min", lo, "max", hi)
    sp.panels(arrays, [titles[k] for k in maps], OUT / "importance.png", cols=2, cmap="magma_r", vmin=0, vmax=1,
              width=1000, colorbar="Importance, rescaled 0 (least) to 1 (most) among the selected cells")
    vals = {k: maps[k][sol] for k in maps}
    rows = []
    for a in maps:
        r = {"measure": titles[a]}
        for b in maps:
            r[titles[b]] = round(float(spearmanr(vals[a], vals[b], nan_policy="omit").statistic), 2)
        rows.append(r)
        print(r)
    sp.write_csv(DAT / "importance-correlations.csv", rows)
    # the ten most important cells by replacement cost: which features they hold
    f = sp.features()
    idx = np.argsort(-np.nan_to_num(np.where(sol, maps["replacement"], -1)), axis=None)[:15]
    for i in idx:
        r, c = np.unravel_index(i, sol.shape)
        top = sorted(((f[k][r, c], k) for k in sp.FEATURES), reverse=True)[:2]
        print("top cell", r, c, round(maps["replacement"][r, c], 3), [(k, round(v, 2)) for v, k in top])


# ------------------------------------------------------------------------------------------------ gap analysis
def gap_analysis():
    w = sp.run(SLUG, "gap", sp.read_node("protected", "protected.tif") + '''
[[nodes]]
id = "evaluation"
op = "prioritize.evaluate"
inputs = { features = "features.stack", cost = "cost.map", solution = "protected.map" }
params = { relative_targets = 0.3 }

[outputs.coverage]
from = "evaluation.coverage"
[outputs.summary]
from = "evaluation.summary"
''')
    cov = tbl(w, "coverage")
    print("summary", tbl(w, "summary"))
    rows = [{"feature": sp.SHORT[sp.FEATURES[int(r["feature"]) - 1]], "Held by the protected areas": round(100 * r["relative_held"], 1),
             "Target": 30} for r in cov]
    print(rows)
    sp.write_csv(DAT / "gap-analysis.csv", rows)


# ------------------------------------------------------------------------------------------------ calibration
def calibrate(name, params, extra_out=""):
    t = time.time()
    w = sp.run(SLUG, name, f'''
[[nodes]]
id = "calibration"
op = "prioritize.calibrate"
inputs = {{ features = "features.stack", cost = "cost.map" }}
params = {{ {params} }}

[outputs.table]
from = "calibration.table"
''', {"calibration.best": "best.tif", "calibration.solutions": "solutions.tif"})
    print(name, round(time.time() - t, 1), "s")
    return w, tbl(w, "table")


def blended():
    w, t = calibrate("blended", 'approach = "blended", penalty = "boundary", objective = "min_set", relative_targets = 0.3, '
                                'values = [0.001, 0.002, 0.005, 0.01, 0.02, 0.05], gap = 0.02, time_limit = 90')
    for r in t:
        print({k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items()})
    sp.write_csv(DAT / "blended.csv", [{"Boundary (km)": round(r["boundary"] / 1000), "Cost (million USD)": round(r["cost"] / 1000, 2)}
                                       for r in t])
    knee = next(i for i, r in enumerate(t) if r.get("knee") == 1)
    top = next(i for i, r in enumerate(t) if r.get("topsis") == 1)
    for lab, i in (("knee", knee), ("topsis", top)):
        sel = sp.grid(w / "solutions.tif", i + 1)
        sp.plan_map(sel, OUT / f"blended_{lab}.png", legend=[], width=760)
        print(lab, t[i])


def hierarchical():
    w, t = calibrate("hierarchical", 'approach = "hierarchical", penalty = "boundary", objective = "min_set", relative_targets = 0.3, '
                                     'inflation = [0.005, 0.01, 0.02, 0.04, 0.07, 0.1], gap = 0.05, time_limit = 120')
    for r in t:
        print({k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items()})
    sp.write_csv(DAT / "hierarchical.csv", [{"Boundary (km)": round(r["boundary"] / 1000), "Cost (million USD)": round(r["cost"] / 1000, 2)}
                                            for r in t])


def pareto():
    w, t = calibrate("pareto", 'approach = "pareto", objective = "min_set", relative_targets = 0.3, values = [5000, 10000, 15000, 20000, 25000, 30000, 35000, 40000], gap = 0.01, time_limit = 60')
    for r in t:
        print({k: (round(v, 4) if isinstance(v, float) else v) for k, v in r.items()})
    sp.write_csv(DAT / "pareto.csv", [{"budget": round(r["value"] / 1000, 1), "Shortfall": round(r["shortfall"], 3),
                                       "Targets met": int(r["met"])} for r in t])


# ------------------------------------------------------------------------------------------------ Zonation against the plan
def zonation():
    w = sp.run(SLUG, "zonation", PLAN + '''
[[nodes]]
id = "zonation"
op = "prioritize.zonation"
inputs = { features = "features.stack", cost = "cost.map" }
params = { rule = "CAZ2" }

[outputs.summary]
from = "plan.summary"
''', {"plan.solution": "plan.tif", "zonation.rank": "rank.tif"})
    plan = sp.grid(w / "plan.tif") > 0.5
    rank = sp.grid(w / "rank.tif")
    budget = float(np.nansum(COST * plan))
    order = np.argsort(-np.nan_to_num(rank, nan=-1), axis=None)
    c = np.nan_to_num(COST.ravel()[order])
    keep = order[np.cumsum(c) <= budget]
    zsel = np.zeros(plan.shape, bool)
    zsel.ravel()[keep] = True
    zsel &= sp.land()
    print("budget", budget, "zonation cells", int(zsel.sum()), "cost", float(np.nansum(COST * zsel)), "plan cells", int(plan.sum()))
    import rasterio
    with rasterio.open(sp.DATA / "cost.tif") as ds:
        prof = ds.profile
    prof.update(dtype="float32", count=1, nodata=None)
    with rasterio.open(w / "zsel.tif", "w", **prof) as ds:
        ds.write(np.where(sp.land(), zsel, np.nan).astype("float32"), 1)
    w2 = sp.run(SLUG, "zonation_eval", sp.read_node("zsel", "../zonation/zsel.tif") + sp.read_node("plan", "../zonation/plan.tif") + '''
[[nodes]]
id = "eval_z"
op = "prioritize.evaluate"
inputs = { features = "features.stack", cost = "cost.map", solution = "zsel.map" }
params = { relative_targets = 0.3, extinction_z = 0.25 }

[[nodes]]
id = "eval_p"
op = "prioritize.evaluate"
inputs = { features = "features.stack", cost = "cost.map", solution = "plan.map" }
params = { relative_targets = 0.3, extinction_z = 0.25 }

[outputs.cz]
from = "eval_z.coverage"
[outputs.cp]
from = "eval_p.coverage"
[outputs.sz]
from = "eval_z.summary"
[outputs.sp]
from = "eval_p.summary"
[outputs.rz]
from = "eval_z.extinction_risk"
[outputs.rp]
from = "eval_p.extinction_risk"
''')
    cz, cp = tbl(w2, "cz"), tbl(w2, "cp")
    rows = [{"feature": sp.SHORT[sp.FEATURES[i]], "Zonation's best cells": round(100 * a["relative_held"], 1),
             "Optimized plan": round(100 * b["relative_held"], 1)} for i, (a, b) in enumerate(zip(cz, cp))]
    sp.write_csv(DAT / "zonation-vs-plan.csv", rows)
    print(rows)
    print("summaries", tbl(w2, "sz"), tbl(w2, "sp"))
    rz, rp = tbl(w2, "rz"), tbl(w2, "rp")
    print("risk sums", rz[-1], rp[-1])
    print("met", sum(int(r["met"]) for r in cz), sum(int(r["met"]) for r in cp),
          "mean", np.mean([r["relative_held"] for r in cz]), np.mean([r["relative_held"] for r in cp]))
    both = plan & zsel
    sp.plan_map(both.astype(float), OUT / "zonation-vs-plan.png",
                legend=[("Both", sp.SELECTED), ("Only Zonation's best cells", figs.EMBER), ("Only the plan", figs.WATER),
                        ("Neither", sp.UNSELECTED)],
                extra=[(zsel & ~plan, figs.EMBER), (plan & ~zsel, figs.WATER)])
    print("overlap: both", int(both.sum()), "only z", int((zsel & ~plan).sum()), "only plan", int((plan & ~zsel).sum()))


if __name__ == "__main__":
    import sys
    for t in sys.argv[1:] or ["importance", "gap_analysis", "zonation", "blended", "pareto", "hierarchical"]:
        print(t, globals()[t]())
