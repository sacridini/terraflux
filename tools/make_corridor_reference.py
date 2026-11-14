# SPDX-License-Identifier: GPL-3.0-or-later
"""The models landschaft compiles for corridor constraints (add_corridor_constraints), for Terraflux's bit-for-bit
compile test (crates/ops/tests/optimize.rs `corridors_compile_is_landschafts`): the matrix, the objective, the bounds,
the right-hand sides, senses, types and ids, and the corridor networks (source, terminals, edges) of a few problems
on the rasters and the table of landschaft's optimize cases. Written to tests/data/reference/optimize_corridors.npz
(the numbers) and optimize_corridors.json (the text and the networks). Run with the landschaft the submodule points at
installed:

    python tools/make_corridor_reference.py
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

os.environ.setdefault("OMP_NUM_THREADS", "4")

import numpy as np  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import landschaft as ls  # noqa: E402
import optimize_cases as oc  # noqa: E402
from landschaft.prioritize._milp_compile import compile_problem  # noqa: E402

REF = ROOT / "tests" / "data" / "reference"
NX = oc.NX


def terminals(cells):
    t = np.zeros(oc.NY * oc.NX)
    t[cells] = 1.0
    return oc.grid(t.reshape(oc.NY, oc.NX))


def raster(objective, **kw):
    p = ls.prioritize.problem(oc.pu_cost(), oc.features())
    return objective(p).add_binary_decisions() if kw.get("binary", True) else objective(p)


NETS_ONE = [[4 * NX + 2, 11 * NX + 12, 13]]
NETS_TWO = [[4 * NX + 2, 11 * NX + 12], [5 * NX, 10 * NX + 14]]
ROWS = [5, 80, 150]  # the table's terminal rows


def cases() -> dict:
    out = {}
    out["raster_one_net"] = (
        {"objective": "min_set", "relative_targets": 0.2, "nets": NETS_ONE},
        raster(lambda p: p.add_min_set_objective().add_relative_targets(0.2).add_corridor_constraints(
            [terminals(c) for c in NETS_ONE])))
    out["raster_two_nets_resistance"] = (
        {"objective": "min_set", "relative_targets": 0.2, "nets": NETS_TWO, "resistance": True, "penalty": 0.5},
        raster(lambda p: p.add_min_set_objective().add_relative_targets(0.2).add_corridor_constraints(
            [terminals(c) for c in NETS_TWO], resistance=oc.pu_cost(), penalty=0.5)))
    # a maximised objective: the resistance enters with the opposite sign
    out["raster_max_utility_resistance"] = (
        {"objective": "max_utility", "budget": 1200.0, "nets": NETS_ONE, "resistance": True, "penalty": 2.0},
        raster(lambda p: p.add_max_utility_objective(1200.0).add_corridor_constraints(
            [terminals(c) for c in NETS_ONE], resistance=oc.pu_cost(), penalty=2.0)))
    # after a boundary penalty and next to locked units
    out["raster_boundary_locked"] = (
        {"objective": "min_set", "relative_targets": 0.2, "nets": NETS_ONE, "boundary_penalty": 0.5, "locks": True},
        raster(lambda p: p.add_min_set_objective().add_relative_targets(0.2).add_boundary_penalties(0.5)
               .add_corridor_constraints([terminals(c) for c in NETS_ONE])
               .add_locked_in_constraints(oc.locked_in()).add_locked_out_constraints(oc.locked_out())))
    # a table with an adjacency matrix: the terminals a column, the resistance the cost column
    pu = oc.pu_table()
    pu["term"] = 0.0
    pu.loc[ROWS, "term"] = 1.0
    adj = oc._matrix(oc.adj_table(), len(pu))
    out["table_matrix"] = (
        {"objective": "min_set", "relative_targets": 0.2, "rows": ROWS, "resistance": True, "penalty": 1.0},
        ls.prioritize.problem(pu, oc.FEATURES, cost_column="cost").add_min_set_objective()
        .add_relative_targets(0.2).add_corridor_constraints("term", resistance="cost", penalty=1.0, data=adj)
        .add_binary_decisions())
    return out


def main() -> None:
    arrays, meta = {}, {}
    for name, (settings, p) in cases().items():
        m, _ = compile_problem(p)
        a = m.A.tocsr()
        a.sort_indices()
        a.eliminate_zeros()
        coo = a.tocoo()
        arrays[f"{name}/A_i"] = coo.row.astype(np.float64)
        arrays[f"{name}/A_j"] = coo.col.astype(np.float64)
        arrays[f"{name}/A_x"] = coo.data.astype(np.float64)
        for key in ("obj", "lb", "ub", "rhs"):
            arrays[f"{name}/{key}"] = np.asarray(getattr(m, key), dtype=np.float64)
        meta[name] = {
            "settings": settings, "nrow": m.nrow, "ncol": m.ncol, "sense": list(m.sense), "vtype": list(m.vtype),
            "row_ids": list(m.row_ids), "col_ids": list(m.col_ids), "modelsense": m.modelsense,
            "corridors": [{"source": int(c.source), "sinks": [int(v) for v in c.sinks],
                           "tail": [int(v) for v in c.tail], "head": [int(v) for v in c.head]}
                          for c in m.corridors]}
        print(name, m.nrow, m.ncol, len(m.corridors), "networks")
    np.savez_compressed(REF / "optimize_corridors.npz", **arrays)
    (REF / "optimize_corridors.json").write_text(json.dumps({"landschaft": ls.__version__, "cases": meta},
                                                           separators=(",", ":")) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
