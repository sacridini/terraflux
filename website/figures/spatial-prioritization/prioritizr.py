"""Figures of 'Optimal plans with prioritizr' (spatial-prioritization/prioritizr).

prioritize.optimize on Serra Verde: the minimum set with the protected areas locked in and the cities locked out,
the objectives that spend a budget (max utility, min shortfall, min largest shortfall, max features, min
extinction risk), a slider over the budget, the optimality gap, proportion decisions (the linear relaxation's
bound) and a gap portfolio's selection frequency.

    python website/figures/spatial-prioritization/prioritizr.py [locked objectives budget gaps portfolio]
"""

import time

import numpy as np

import sp_common as sp
from sp_common import figs, plt

SLUG = "prioritizr"
OUT = sp.img(SLUG)
DAT = sp.data(SLUG)
OPTIMUM = 37141.1          # the 30 % minimum set, proven (gap 0), thousand USD
BUDGET = round(OPTIMUM / 2)  # half of it
COST = sp.grid(sp.DATA / "cost.tif")
FEAT = sp.features()
TOTAL = {f: float(np.nansum(a)) for f, a in FEAT.items()}
PROTECTED = sp.grid(sp.DATA / "protected.tif") > 0.5
URBAN = sp.grid(sp.DATA / "urban.tif") > 0.5
LOCKS = sp.read_node("protected", "protected.tif") + sp.read_node("urban", "urban.tif")


def solve(name, params, inputs="", extra="", writes=None):
    body = extra + f'''
[[nodes]]
id = "plan"
op = "prioritize.optimize"
inputs = {{ features = "features.stack", cost = "cost.map"{inputs} }}
params = {{ {params} }}

[outputs.summary]
from = "plan.summary"
'''
    t = time.time()
    w = sp.run(SLUG, name, body, writes or {"plan.solution": "solution.tif"})
    secs = time.time() - t
    s = sp.table(w, "summary")
    return w, s, secs


def held(sol):
    """The share (%) of every feature in a solution (shares of cells allowed)."""
    x = np.nan_to_num(sol)
    return {f: 100 * float(np.nansum(FEAT[f] * x)) / TOTAL[f] for f in sp.FEATURES}


def cost_of(sol):
    return float(np.nansum(COST * np.nan_to_num(sol)))


# ------------------------------------------------------------------------------------------------ locked areas
def locked():
    w, s, _ = solve("locked", 'objective = "min_set", relative_targets = 0.3, gap = 0.001',
                    ', locked_in = "protected.map", locked_out = "urban.map"', LOCKS)
    sol = sp.grid(w / "solution.tif")
    sp.plan_map(sol, OUT / "locked.png",
                legend=[("Selected", sp.SELECTED), ("Protected (locked in)", sp.PROTECTED),
                        ("Cities (locked out)", sp.URBAN), ("Not selected", sp.UNSELECTED)],
                extra=[(PROTECTED, sp.PROTECTED), (URBAN, sp.URBAN)])
    h = held(sol)
    print("locked:", s[0], "cells", int(np.nansum(sol > 0.5)), "protected cells", int(PROTECTED.sum()),
          "urban", int(URBAN.sum()), {sp.SHORT[k]: round(v, 1) for k, v in h.items()})
    # what the protected areas hold on their own (a gap analysis in one line)
    hp = held(PROTECTED.astype(float))
    print("protected alone", {sp.SHORT[k]: round(v, 1) for k, v in hp.items()}, "cost", cost_of(PROTECTED))
    w0, s0, _ = solve("plain", 'objective = "min_set", relative_targets = 0.3, gap = 0.001')
    print("plain:", s0[0])
    return s[0]


# ------------------------------------------------------------------------------------------------ objectives
OBJECTIVES = {
    "Max utility": 'objective = "max_utility", budget = B',
    "Max utility, weighted": 'objective = "max_utility", budget = B, feature_weights = W',
    "Min shortfall": 'objective = "min_shortfall", budget = B, relative_targets = 0.3',
    "Min largest shortfall": 'objective = "min_largest_shortfall", budget = B, relative_targets = 0.3',
    "Max features": 'objective = "max_features", budget = B, relative_targets = 0.3',
    "Min extinction risk": 'objective = "min_extinction_risk", budget = B, z = 0.25',
}


def objectives():
    weights = "[" + ", ".join(f"{1000 / TOTAL[f]:.6g}" for f in sp.FEATURES) + "]"
    rows, maps = [], []
    for k, p in OBJECTIVES.items():
        p = p.replace("B", str(BUDGET)).replace("W", weights) + ", gap = 0.005"
        w, s, secs = solve("obj_" + k.replace(" ", "_").replace(",", ""), p)
        sol = sp.grid(w / "solution.tif")
        h = held(sol)
        met = sum(v >= 30 - 1e-6 for v in h.values())
        risk = sum(1 - (v / 100) ** 0.25 for v in h.values())
        r = {"objective": k}
        r.update({sp.SHORT[f]: round(v, 1) for f, v in h.items()})
        rows.append(r)
        maps.append((k, sol))
        print(f"{k}: cost {cost_of(sol) / 1000:.2f} M, targets met {met}, mean {np.mean(list(h.values())):.1f}, "
              f"min {min(h.values()):.1f}, risk {risk:.2f}, {secs:.1f} s, summary {s[0]}")
    sp.write_csv(DAT / "objectives.csv", rows)
    # the chart wants a row per feature, a column per objective
    t = [{"feature": sp.SHORT[f], **{r["objective"]: r[sp.SHORT[f]] for r in rows}} for f in sp.FEATURES]
    sp.write_csv(DAT / "objectives-by-feature.csv", t)
    sp.panels([m for _, m in maps], [k for k, _ in maps], OUT / "objectives.png", cols=3, kinds=["plan"] * len(maps),
              width=1050)


# ------------------------------------------------------------------------------------------------ the budget
BUDGETS = [5, 10, 15, 20, 25, 30, 35, 40]


def budget():
    rows = []
    for b in BUDGETS:
        w, s, secs = solve(f"budget_{b}", f'objective = "min_shortfall", budget = {b * 1000}, relative_targets = 0.3, '
                                          f'gap = 0.005')
        sol = sp.grid(w / "solution.tif")
        sp.plan_map(sol, OUT / f"budget_{b}.png", legend=[], width=760)
        h = held(sol)
        met = sum(v >= 30 - 1e-3 for v in h.values())
        worst = min(h, key=h.get)
        r = {"budget": b}
        r.update({sp.SHORT[f]: round(min(100.0, 100 * v / 30), 1) for f, v in h.items()})
        rows.append(r)
        print(f"budget {b}: met {met}, worst {sp.SHORT[worst]} {h[worst]:.1f} % ({100 * h[worst] / 30:.0f} % of its "
              f"target), cost {cost_of(sol) / 1000:.2f}, cells {int(np.nansum(sol > 0.5))}, {secs:.1f} s")
    sp.write_csv(DAT / "budget.csv", rows)


# ------------------------------------------------------------------------------------------------ gaps
def gaps():
    rows = []
    for g in (0.1, 0.01, 0.001, 0):
        w, s, secs = solve(f"gap_{g}", f'objective = "min_set", relative_targets = 0.3, gap = {g}')
        r = s[0]
        rows.append({"gap asked": g, "cost": round(r["cost"] / 1000, 3), "gap reported": r["gap"],
                     "cells": int(r["selected"]), "seconds": round(secs, 1)})
        print(rows[-1])
    # proportion decisions: the linear relaxation, a bound below every plan
    w, s, secs = solve("proportion", 'objective = "min_set", relative_targets = 0.3, decisions = "proportion"')
    sol = sp.grid(w / "solution.tif")
    frac = int(np.nansum((sol > 1e-6) & (sol < 1 - 1e-6)))
    print("proportion:", s[0], "fractional cells", frac, "full", int(np.nansum(sol >= 1 - 1e-6)), f"{secs:.1f} s")
    rows.append({"gap asked": "proportion", "cost": round(s[0]["cost"] / 1000, 3), "gap reported": s[0]["gap"],
                 "cells": int(np.nansum(sol > 1e-6)), "seconds": round(secs, 1)})
    figs.raster(None, OUT / "proportion.png", array=np.where(sp.land(), np.where(sol > 1e-6, sol, np.nan), np.nan),
                cmap="viridis", vmin=0, vmax=1, unit="Share of the cell selected")
    sp.write_csv(DAT / "gaps.csv", rows)
    bound()


def bound():
    """The bound and the plans on a line, from gaps.csv."""
    rows = figs.read_csv(DAT / "gaps.csv")
    loose = float(next(r for r in rows if r["gap asked"] == "0.1")["cost"])
    lp = float(next(r for r in rows if r["gap asked"] == "proportion")["cost"])
    opt = float(next(r for r in rows if r["gap asked"] == "0")["cost"])

    def draw(ax):
        ax.set_xlim(lp - 0.025, max(loose, opt) + 0.025)
        ax.set_ylim(-1, 1.4)
        ax.axhline(0, color=figs.INK2, lw=1)
        ax.axvspan(lp, opt, ymin=0.3, ymax=0.5, color="#2e7d4f22")
        for x, lab, col, dy in ((lp, f"Bound: fractions of\ncells allowed, {lp:.3f}", figs.WATER, 0.62),
                                (opt, f"Proven optimum\n{opt:.3f}", figs.FOREST, -0.62),
                                (loose, f"Plan found with\ngap = 0.1: {loose:.3f}", figs.EMBER, 0.62)):
            ax.plot([x], [0], "o", color=col, ms=9, zorder=3)
            ax.text(x, dy, lab, ha="center", va="center", fontsize=9, color=figs.INK)
        ax.set_yticks([])
        ax.grid(False)
        ax.spines["left"].set_visible(False)
        ax.set_xlabel("Cost of the plan (million USD)")

    figs.chart(OUT / "bound.png", draw, width=900, height=300)


# ------------------------------------------------------------------------------------------------ a portfolio
def portfolio():
    w, s, secs = solve("portfolio", 'objective = "min_set", relative_targets = 0.3, gap = 0.01, portfolio = "gap", '
                                    'number_solutions = 20, pool_gap = 0.02',
                       writes={"plan.frequency": "freq.tif", "plan.solutions": "solutions.tif"})
    freq = sp.grid(w / "freq.tif")
    m = sp.land()
    costs = [r["cost"] for r in s]
    print("portfolio:", len(s), "plans, cost", min(costs), max(costs), f"{secs:.1f} s")
    print("always", int(np.nansum(freq[m] >= 0.999)), "some", int(np.nansum((freq[m] > 0) & (freq[m] < 0.999))))
    always = m & (np.nan_to_num(freq) >= 0.999)
    some = m & (np.nan_to_num(freq) > 0) & ~always
    sp.plan_map(always.astype(float), OUT / "portfolio.png",
                legend=[("In all 20 plans", sp.SELECTED), ("In some of them", figs.EMBER), ("In none", sp.UNSELECTED)],
                extra=[(some, figs.EMBER)])
    # how much two plans differ
    n = figs.bands(w / "solutions.tif")
    sols = [sp.grid(w / "solutions.tif", b) > 0.5 for b in range(1, n + 1)]
    diff = [int(np.sum(sols[0] ^ x)) for x in sols[1:]]
    print("cells differing from the first plan", diff)
    sp.write_csv(DAT / "portfolio.csv", [{"plan": i + 1, "cost": round(c / 1000, 3)} for i, c in enumerate(costs)])


if __name__ == "__main__":
    import sys
    for t in sys.argv[1:] or ["locked", "objectives", "budget", "gaps", "portfolio"]:
        print(t, globals()[t]())
