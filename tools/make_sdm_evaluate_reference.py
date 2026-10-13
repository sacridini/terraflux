# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's evaluation of species distribution models gives, for Terraflux's tests, written to
tests/data/reference/sdm_evaluate.npz and .json:

    python tools/make_sdm_evaluate_reference.py

Predictions at presences and at background points (more than a thousand, and fewer; ties included): evaluate
(the statistics, the confusion table and the thresholds; dismo's thresholds and given ones), Boyce's index (a
moving window, classes, breaks, Pearson, duplicates kept), every threshold method and the omission rates.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.sdm


def main() -> None:
    rng = np.random.default_rng(17)
    pres = np.round(rng.beta(3, 2, 80), 4)
    bg = rng.uniform(0, 1, 1500)
    bg_small = np.round(rng.beta(2, 3, 300), 3)
    out: dict[str, np.ndarray] = {"in/pres": pres, "in/bg": bg, "in/bg_small": bg_small}
    cases: list[dict] = []
    for name, (p, a, tr) in {"big": (pres, bg, None), "small": (pres, bg_small, None),
                             "given": (pres, bg_small, [0.1, 0.35, 0.5, 0.72, 0.9])}.items():
        e = S.evaluate(p, a, thresholds=tr)
        out[f"{name}/stats"] = np.array([e.stats[k] for k in ["auc", "cor", "pcor", "np", "na", "prevalence",
                                                              "max_tss", "boyce"]], dtype=np.float64)
        out[f"{name}/table"] = e.table.to_numpy(dtype=np.float64)
        out[f"{name}/thresholds"] = np.array(list(e.thresholds.values()), dtype=np.float64)
        out[f"{name}/boyce"] = np.column_stack([e.boyce["f_ratio"], e.boyce["hs"]])
        cases.append({"name": name, "op": "sdm.evaluate", "background": "in/bg" if name == "big" else "in/bg_small",
                      "params": {} if tr is None else {"thresholds": tr}})
    for name, kw in {"b_window": {}, "b_classes": {"nclass": 5}, "b_breaks": {"nclass": [0.2, 0.4, 0.6, 0.8]},
                     "b_pearson": {"window": 0.05, "method": "pearson", "rm_duplicate": False},
                     "b_kendall": {"res": 40, "method": "kendall"}}.items():
        b = S.boyce(pres, np.concatenate([bg_small, pres]), **kw)
        out[name] = np.column_stack([b["f_ratio"], b["hs"]])
        out[f"{name}/cor"] = np.array([b["cor"]])
        out[f"{name}/used"] = np.asarray(b["used"], dtype=np.float64)
        cases.append({"name": name, "op": "sdm.boyce", "params": kw})
    methods = ["max_kappa", "max_spec_sens", "no_omission", "equal_prevalence", "equal_sens_spec", "mtp", "p10"]
    out["thresholds"] = np.array([S.threshold(pres, bg_small, m) for m in methods]
                                 + [S.threshold(pres, bg_small, "sensitivity", sensitivity=0.85)])
    test = np.round(rng.beta(2.5, 2, 37), 4)
    out["in/test"] = test
    out["omission"] = np.array([S.omission_rate(pres, test, "mtp"), S.omission_rate(pres, test, "p10"),
                                S.omission_rate(pres[:7], test, "p10")])
    np.savez_compressed(REF / "sdm_evaluate.npz", **out)
    (REF / "sdm_evaluate.json").write_text(json.dumps({"cases": cases, "methods": methods}, indent=1) + "\n",
                                           encoding="utf-8")
    print(REF / "sdm_evaluate.npz", len(out))


if __name__ == "__main__":
    main()
