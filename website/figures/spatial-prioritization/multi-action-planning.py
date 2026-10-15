"""Figures of 'Multi-action planning' (spatial-prioritization/multi-action-planning).

prioritize.actions_raster on Serra Verde: its two threats (deforestation and fire), the cheapest monitoring and
actions for recovery and conservation targets (minimizeCosts), the most benefit within budgets (maximizeBenefits, a
slider), a sensitivity table, the response curve, and the small problem of the template as prioriactions' tables
(prioritize.actions).

    python website/figures/spatial-prioritization/multi-action-planning.py [threats base budgets sensitivity curves tables]
"""

import numpy as np
from matplotlib import colors as mcolors

import sp_common as sp
from sp_common import figs, plt

SLUG = "multi-action-planning"
OUT = sp.img(SLUG)
DAT = sp.data(SLUG)
THREATS = sp.read_node("threats", "threats.tif")
ACT = {0: ("Not managed", sp.UNSELECTED), 1: ("Monitored only", "#a7c957"), 2: ("Against deforestation", "#c0392b"),
       3: ("Against fire", figs.EMBER), 4: ("Against both", "#5e3c99")}


def actions(name, params, extra_nodes="", extra_inputs="", outs=("result", "cost", "benefit", "potential")):
    body = THREATS + extra_nodes + f'''
[[nodes]]
id = "plan"
op = "prioritize.actions_raster"
inputs = {{ monitoring_cost = "cost.map", features = "features.stack", threats = "threats.stack"{extra_inputs} }}
params = {{ action_cost = [30, 10], gap = 0.01{params} }}
'''
    for o in outs:
        body += f'\n[outputs.{o}]\nfrom = "plan.{o}"\n'
    w = sp.run(SLUG, name, body, {"plan.maps": "maps.tif"})
    return w


def read_maps(w):
    import rasterio
    with rasterio.open(w / "maps.tif") as ds:
        names = list(ds.descriptions)
        layers = {n: ds.read(i + 1, masked=True).astype(float).filled(np.nan) for i, n in enumerate(names)}
    land = sp.land()
    mon = np.nan_to_num(layers["monitoring"]) > 0.5
    d = np.nan_to_num(layers["deforestation"]) > 0.5
    f = np.nan_to_num(layers["fire"]) > 0.5
    cls = np.where(land, 0, np.nan)
    cls[land & mon] = 1
    cls[land & d] = 2
    cls[land & f] = 3
    cls[land & d & f] = 4
    return cls


def class_map(cls, png, legend=True):
    figs.raster(None, png, array=np.ma.masked_invalid(cls), palette=ACT, width=900 if legend else 760, legend=legend)


def counts(cls):
    return {ACT[k][0]: int(np.nansum(cls == k)) for k in ACT}


def benefit_share(w):
    b = sp.table(w, "benefit")
    p = sp.table(w, "potential")
    print("potential columns", list(p[0]))
    return b, p


def threats():
    t = [sp.grid(sp.DATA / "threats.tif", 1), sp.grid(sp.DATA / "threats.tif", 2)]
    sp.panels(t, ["Deforestation", "Fire"], OUT / "threats.png", cols=2, cmap="OrRd", vmin=0, vmax=1, width=1000,
              colorbar="Intensity of the threat (0-1)")
    for i, a in enumerate(t):
        print("threat", i + 1, "cells > 0", int(np.nansum(a > 0)))


def base():
    w = actions("base", ", target_recovery = 0.3, target_conservation = 0.1")
    cls = read_maps(w)
    class_map(cls, OUT / "plan.png")
    print("base", sp.table(w, "result"), sp.table(w, "cost"), counts(cls))
    b, p = benefit_share(w)
    for r, q in zip(b, p):
        print({k: round(v, 2) for k, v in r.items()}, {k: round(v, 2) for k, v in q.items()})


BUDGETS = [10, 20, 30, 40, 60]


def budgets():
    rows = []
    for bud in BUDGETS:
        w = actions(f"budget_{bud}", f', model = "maximizeBenefits", budget = {bud * 1000}')
        cls = read_maps(w)
        class_map(cls, OUT / f"budget_{bud}.png", legend=False)
        b, p = benefit_share(w)
        res, cost = sp.table(w, "result")[0], sp.table(w, "cost")[0]
        r = {"budget": bud}
        for i, (x, q) in enumerate(zip(b, p)):
            mx = q.get("maximum.benefit", q.get("maximum", None))
            r[sp.SHORT[sp.FEATURES[i]]] = round(100 * x["benefit.total"] / mx, 1) if mx else x["benefit.total"]
        rows.append(r)
        print(bud, res, cost, counts(cls), r)
    sp.write_csv(DAT / "budgets.csv", rows)


def sensitivity():
    tbl = '''
[[nodes]]
id = "sensitivity"
op = "table.of"
params = { columns = ["feature", "threat", "delta1", "delta2", "delta3", "delta4"], rows = [[5, 1, 0, 1, 0, 1], [8, 1, 0, 1, 0, 1]] }
'''
    w = actions("sensitivity", ", target_recovery = 0.3, target_conservation = 0.1", tbl, ', sensitivity = "sensitivity.table"')
    cls = read_maps(w)
    class_map(cls, OUT / "sensitivity.png")
    print("sensitivity", sp.table(w, "result"), sp.table(w, "cost"), counts(cls))


def curves():
    for c in (1, 3):
        w = actions(f"curve_{c}", f", target_recovery = 0.3, target_conservation = 0.1, curve = {c}")
        cls = read_maps(w)
        class_map(cls, OUT / f"curve_{c}.png", legend=False)
        print("curve", c, sp.table(w, "result"), sp.table(w, "cost"), counts(cls))


def tables():
    w = sp.work(SLUG, "tables")
    model = figs.template("multi-action-planning-prioriactions")
    model += '''
[outputs.summary]
from = "plan.summary"
[outputs.result]
from = "plan.result"
[outputs.cost]
from = "plan.cost"
[outputs.benefit]
from = "plan.benefit"
'''
    figs.run(model, w, "-q")
    for t in ("summary", "result", "cost", "benefit"):
        print(t, sp.table(w, t))


if __name__ == "__main__":
    import sys
    for t in sys.argv[1:] or ["threats", "base", "budgets", "sensitivity", "tables"]:
        print(t, globals()[t]())
