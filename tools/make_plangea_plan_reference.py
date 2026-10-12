# SPDX-License-Identifier: GPL-3.0-or-later
"""landschaft's restoration plans with intactness, the minimum cost and sets of weights, for Terraflux's tests of
restoration.plan: the bd_absent problem of landschaft's tests (tests/plangea_cases.py, the extinction risk of the
species of its area of habitat) with a human footprint, carbon and a cost drawn here, solved by landschaft's
RestorationProblem; written to tests/data/reference/plangea_plan.json:

    python tools/make_plangea_plan_reference.py
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import landschaft as ls  # noqa: E402
import plangea_cases as pc  # noqa: E402

P = ls.prioritize


def flat(a) -> list:
    return [None if not np.isfinite(v) else float(v) for v in np.asarray(a, dtype=np.float64).ravel()]


def main() -> None:
    spec = pc.BD_CASES["bd_absent"]
    oa = pc.OA_CASES[spec["oa"]]
    cover = pc.grid(pc.land_cover(), names=pc.CLASSES)
    past = pc.grid(pc.past(), names=pc.NATURAL) if oa["past"] else None
    er = pc.grid(pc.ecoregions(), names=pc.NATURAL) if oa["er"] else None
    veg = ls.original_vegetation(cover, pc.NATURAL, pc.ANTHROPIC, past=past, ecoregions=er, failsafe=oa["failsafe"],
                                 restore_to="current" if "D" in oa["problem"] else "original")
    names, r = pc.ranges()
    aoh = ls.sdm.area_of_habitat(pc.grid(r, names=names), cover, pc.NATURAL, suitability=pc.habitat_table(),
                                 original=veg, include_unlisted=spec["include_absent"])
    p = P.restoration_problem(veg, cell_area=1.0).add_extinction_risk_benefit(aoh)

    rng = np.random.default_rng(2026)
    hfi = 2.0 + rng.random(pc.SHAPE) * 12.0
    hfi[0, 0] = np.nan
    red = 1.0 + rng.random(pc.SHAPE) * 4.0
    low = rng.random(pc.SHAPE) * 3.0  # a footprint restoration can take to 0
    carbon = rng.random(pc.SHAPE) * 100.0
    cost = 1.0 + rng.random(pc.SHAPE) * 4.0
    it = {"radius": 2, "cell_size": 1.0}
    out: dict = {"hfi": flat(hfi), "reduction": flat(red), "low": flat(low), "carbon": flat(carbon),
                 "cost": flat(cost), "index": p.index.tolist()}

    # the linear intactness: its coefficients, plangea's steps and the exact plan
    q = p.add_intactness_benefit(pc.grid(hfi), pc.grid(red), weight=0.01, **it).add_area_target(6.0)
    out["it_coef"] = flat(q.benefits[1].coef)
    s3 = q.solve(method="steps", steps=3)
    ex = q.solve()
    out["it_linear"] = {"steps3_x": flat(s3.values), "steps3_benefit": s3.benefit, "exact_benefit": ex.benefit,
                        "exact_status": ex.status}

    # the exact intactness, with the risk (weight 0.01) and alone on a footprint that can reach 0
    qe = p.add_intactness_benefit(pc.grid(hfi), pc.grid(red), weight=0.01, form="exact", **it).add_area_target(6.0)
    se = qe.solve()
    out["it_exact"] = {"benefit": se.benefit, "bound": se.bound, "status": se.status, "values": flat(se.benefits["value"]),
                       "x": flat(se.values), "linear_x_value": qe.evaluate(ex.values).benefit}
    alone = P.restoration_problem(veg, cell_area=1.0).add_intactness_benefit(pc.grid(low), pc.grid(red), form="exact",
                                                                             **it)
    alone = alone.add_area_target(0.4 * float(alone.upper @ alone.area))
    sa = alone.solve()
    out["it_alone"] = {"target": float(0.4 * float(alone.upper @ alone.area)), "benefit": sa.benefit,
                       "bound": sa.bound, "status": sa.status}

    # the minimum cost: the cheapest plan for the most benefit a budget buys
    budget = 0.25 * float(np.sum(cost.ravel()[p.index] * p.area * p.upper))
    best = p.add_budget(budget, pc.grid(cost)).solve()
    cheap = p.add_min_cost_objective(best.benefit * (1 - 1e-9), pc.grid(cost)).solve()
    out["min_cost"] = {"budget": budget, "best_benefit": best.benefit, "best_cost": best.cost,
                       "benefit": best.benefit * (1 - 1e-9), "cost": cheap.cost, "cheap_benefit": cheap.benefit,
                       "bound": cheap.bound, "status": cheap.status}

    # sets of weights
    qw = p.add_linear_benefit(pc.grid(carbon), name="carbon").add_area_target(8.0).add_budget(1e9, pc.grid(cost))
    sets = [{"extinction_risk": 1.0, "carbon": 0.0}, {"extinction_risk": 1.0, "carbon": 1e-3},
            {"extinction_risk": 0.0, "carbon": 1.0}, {"extinction_risk": 1.0, "carbon": 1e-4}]
    cmp_ = qw.compare_weights(sets)
    out["weights"] = {"sets": sets, "columns": list(cmp_.table.columns),
                      "rows": [flat(r) for r in cmp_.table.to_numpy(np.float64)]}

    path = ROOT / "tests" / "data" / "reference" / "plangea_plan.json"
    path.write_text(json.dumps(out, indent=1) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
