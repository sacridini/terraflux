# SPDX-License-Identifier: GPL-3.0-or-later
"""landschaft's minimum extinction risk objective (add_min_extinction_risk_objective, its extension of prioritizr)
and its evaluation, for Terraflux's tests of prioritize.optimize and prioritize.evaluate: the models compiled (on
the rasters of landschaft's optimize cases, tests/data/reference/optimize_inputs.npz and optimize_zones.npz), their
optima, and the extinction risk of the solutions; written to tests/data/reference/optimize_extinction.json:

    python tools/make_extinction_reference.py
"""

from __future__ import annotations

import json
import sys
import warnings
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import landschaft as ls  # noqa: E402
import optimize_cases as oc  # noqa: E402

P = ls.prioritize


def model(m) -> dict:
    a = m.A.tocoo()
    order = np.lexsort((a.col, a.row))
    return {"A_i": a.row[order].tolist(), "A_j": a.col[order].tolist(), "A_x": a.data[order].tolist(),
            "obj": list(map(float, m.obj)), "lb": list(map(float, m.lb)), "ub": list(map(float, m.ub)),
            "rhs": list(map(float, m.rhs)), "sense": list(m.sense), "vtype": list(m.vtype),
            "row_ids": list(m.row_ids), "col_ids": list(m.col_ids), "modelsense": m.modelsense}


def main() -> None:
    cases = {
        "raster_default": P.problem(oc.pu_cost(), oc.features()).add_min_extinction_risk_objective(2500)
        .add_binary_decisions(),
        "raster_z_weights": P.problem(oc.pu_cost(), oc.features())
        .add_min_extinction_risk_objective(2500, z=[0.25, 0.3, 0.2, 0.15, 0.35], segments=8)
        .add_feature_weights([1.0, 2.0, 1.0, 0.5, 1.0]).add_binary_decisions(),
        "raster_proportion": P.problem(oc.pu_cost(), oc.features()).add_min_extinction_risk_objective(1800, z=0.4)
        .add_proportion_decisions(),
        "zones": oc._zone_general().add_min_extinction_risk_objective(2500).add_binary_decisions(),
    }
    out = {}
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        for name, p in cases.items():
            p = p.add_highs_solver(gap=0)
            s = p.solve()
            ev = P.eval_extinction_risk(p, s, p.objective[2] if np.ndim(p.objective[2]) else float(p.objective[2]))
            out[name] = {"model": model(p.compile()), "objective": float(s.objective), "cost": float(s.cost),
                         "risk": [float(v) for v in ev["extinction_risk"]],
                         "relative_held": [None if not np.isfinite(v) else float(v) for v in ev["relative_held"]]}
    path = ROOT / "tests" / "data" / "reference" / "optimize_extinction.json"
    path.write_text(json.dumps(out) + "\n", encoding="utf-8")
    print(path, {k: (len(v["model"]["obj"]), v["objective"]) for k, v in out.items()})


if __name__ == "__main__":
    main()
