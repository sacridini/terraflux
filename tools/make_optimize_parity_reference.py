# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's HiGHS gives prioritize.optimize's problems, for Terraflux's speed and result parity test: every
objective on every kind of planning unit (raster cells, tables, polygons, H3 hexagons; management zones on raster
and table units), solved by landschaft (HiGHS, its default routing: n_threads=4) on the data Terraflux's tests use
(tests/data/reference/optimize_inputs.npz and landschaft's optimize cases), written to
tests/data/reference/optimize_parity.json: per case the settings (as the node's), the model's size and sums, the
solver's status, objective, bound and gap, the units selected, whether that optimum is unique, and the seconds (median
of three solves). A second, larger set of rasters (40 x 50 cells; tests/data/reference/optimize_parity_inputs.npz and
vectors/parity_polygons.gpkg) makes the times mean something. Run, one job at a time (4 threads):

    python tools/make_optimize_parity_reference.py

Run with the landschaft the submodule points at installed (and h3). Terraflux's test is
crates/ops/tests/parity.rs (`cargo test -p terraflux-ops --release --test ops -- --ignored parity::`).
What landschaft solves that Terraflux does not: the `anneal` solver and corridors.
"""

from __future__ import annotations

import dataclasses
import json
import math
import os
import sys
import time
from pathlib import Path

os.environ.setdefault("OMP_NUM_THREADS", "4")
os.environ.setdefault("RAYON_NUM_THREADS", "4")

import numpy as np  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "extern" / "landschaft" / "tests"))

import landschaft as ls  # noqa: E402
import optimize_cases as oc  # noqa: E402
import scipy.sparse as sp  # noqa: E402

REF = ROOT / "tests" / "data" / "reference"
THREADS = 4
TRY_SECONDS = 8.0  # one solve's time limit: no case takes long
GAPS = (0.0, 0.001, 0.01, 0.05, 0.1)  # the first that HiGHS proves within the time limit
FEATURES = oc.FEATURES
TREE = oc.TREE_SHUFFLED
SHARE = 0.3  # a budget's share of the units' total cost


# ----------------------------------------------------------------------------- the data

NY2, NX2 = 40, 50


def medium_arrays() -> tuple[np.ndarray, np.ndarray]:
    """The larger rasters: a cost (10 to 100, holes) and the five features (the recipe of oc._features)."""
    cost = 10.0 + 90.0 * oc.smooth_field(21, (NY2, NX2))
    cost[:6, :8] = np.nan
    cost[12, 14] = np.nan
    cost[30, 5] = np.nan
    out = []
    for k, share in enumerate((0.5, 0.3, 0.15, 1.0, 0.4)):
        f = oc.smooth_field(200 + k, (NY2, NX2))
        cut = np.quantile(f, 1 - share) if share < 1 else 0.0
        out.append(np.where(f > cut, f - cut, 0.0) * (k + 1))
    f = np.stack(out)
    f[1, 25, 30] = np.nan
    return cost, f


def medium_polygons():
    """Squares of 150 m over the larger raster (5000 x 4000 m), the last of a row or a column cut short."""
    xb = np.append(np.arange(0.0, NX2 * oc.RES, 150.0), NX2 * oc.RES)
    yb = np.append(np.arange(0.0, NY2 * oc.RES, 150.0), NY2 * oc.RES)
    return oc._polygons(xb, yb)


def boundary_table(cost: np.ndarray):
    """A table's boundary matrix: the rook neighbours of the cells with a cost (the shared edge, 100 m), and each
    cell's exposed edges on the diagonal (i, j, x; the units' numbers from 1, in row-major order)."""
    import pandas as pd

    ny, nx = cost.shape
    cells = np.flatnonzero(np.isfinite(cost).ravel())
    pos = {int(c): k for k, c in enumerate(cells)}
    i, j, x = [], [], []
    for k, c in enumerate(cells):
        r, q = divmod(int(c), nx)
        edges = 4
        for rr, qq in ((r - 1, q), (r + 1, q), (r, q - 1), (r, q + 1)):
            m = pos.get(rr * nx + qq) if 0 <= rr < ny and 0 <= qq < nx else None
            if m is not None:
                i.append(k + 1)
                j.append(m + 1)
                x.append(oc.RES)
                edges -= 1
        i.append(k + 1)
        j.append(k + 1)
        x.append(edges * oc.RES)
    return pd.DataFrame({"i": np.array(i, dtype=np.float64), "j": np.array(j, dtype=np.float64),
                         "x": np.array(x, dtype=np.float64)})


class Data:
    """One set of inputs: the cost and features rasters, the table of cells and the polygons."""

    def __init__(self, name: str):
        self.name = name
        if name == "small":
            self.cost, self.features = oc.pu_cost(), oc.features()
            self.table = oc.pu_table()
            self.polygons = oc.pu_polygons_partial()
            self.bnd = boundary_table(oc._cost())
        else:
            c, f = medium_arrays()
            self.cost, self.features = oc.grid(c), oc.grid(f)
            cells = np.flatnonzero(np.isfinite(c).ravel())
            import pandas as pd

            data = {"id": np.arange(1, cells.size + 1, dtype=np.float64), "cost": c.ravel()[cells]}
            ff = np.nan_to_num(f.reshape(5, -1)[:, cells])
            data.update({f"f{k + 1}": ff[k] for k in range(5)})
            self.table = pd.DataFrame(data)
            self.polygons = medium_polygons()
            self.bnd = boundary_table(c)


# ----------------------------------------------------------------------------- the cases

# the objective's settings (as the node's), the budget a share of the total cost
OBJECTIVES = {
    "min_set": {"relative_targets": 0.3},
    "min_shortfall": {"objective": "min_shortfall", "budget": SHARE, "relative_targets": 0.4},
    "min_largest_shortfall": {"objective": "min_largest_shortfall", "budget": 0.05,
                              "relative_targets": [0.3, 0.4, 0.5, 0.2, 0.35]},
    "max_utility": {"objective": "max_utility", "budget": SHARE},
    "max_features": {"objective": "max_features", "budget": SHARE, "relative_targets": [0.3, 0.4, 0.5, 0.2, 0.35],
                     "feature_weights": [1, 2, 3, 1, 2]},
    "max_cover": {"objective": "max_cover", "budget": 0.1},
    "min_penalties": {"objective": "min_penalties", "budget": SHARE, "relative_targets": 0.2,
                      "boundary_penalty": 0.5},
    "max_phylo_div": {"objective": "max_phylo_div", "budget": SHARE, "tree": TREE, "relative_targets": 0.3},
    "max_phylo_end": {"objective": "max_phylo_end", "budget": SHARE, "tree": TREE, "relative_targets": 0.3},
    "min_extinction_risk": {"objective": "min_extinction_risk", "budget": SHARE},
}


def cases() -> list[dict]:
    out = []

    def add(data, units, name, params, **kw):
        out.append({"name": f"{data}_{units}_{name}", "data": data, "units": units, "params": dict(params), **kw})

    for data in ("small", "medium"):
        # H3 at both resolutions on both sets (the small one has 25 and 130 hexagons)
        for units in ("raster", "table", "polygons", "h3_9", "h3_10"):
            for name, params in OBJECTIVES.items():
                add(data, units, name, params)
    # the raster's cases with locked units and with contiguity, tables of Marxan's rij
    add("small", "raster", "min_set_locked", {"relative_targets": 0.3}, locks=True)
    add("small", "table", "min_set_locked", {"relative_targets": 0.3}, locks=True)
    add("small", "raster", "min_set_contiguity", {"relative_targets": 0.2, "contiguity": True})
    add("small", "rij", "min_set", {"absolute_targets": oc.ABSOLUTE})
    # management zones: the objectives that take them, on rasters and on tables
    zb = {"budget": [1200, 800, 600]}
    zm = "ZONE_MATRIX"
    add("small", "zones_raster", "min_set", {"relative_targets": zm})
    add("small", "zones_raster", "min_shortfall", {"objective": "min_shortfall", **zb, "relative_targets": zm})
    add("small", "zones_raster", "min_largest_shortfall",
        {"objective": "min_largest_shortfall", "budget": 2500, "relative_targets": zm})
    add("small", "zones_raster", "max_utility", {"objective": "max_utility", "budget": 2500})
    add("small", "zones_raster", "max_features", {"objective": "max_features", "budget": [900, 600, 500],
                                                  "relative_targets": zm})
    add("small", "zones_raster", "max_cover", {"objective": "max_cover", "budget": 1500})
    add("small", "zones_raster", "max_phylo_div", {"objective": "max_phylo_div", "budget": 2000, "tree": TREE,
                                                   "relative_targets": zm})
    add("small", "zones_raster", "max_phylo_end", {"objective": "max_phylo_end", "budget": [900, 600, 500],
                                                   "tree": TREE, "relative_targets": zm})
    add("small", "zones_table", "min_set", {"relative_targets": zm}, locks=True)
    add("small", "zones_table", "max_utility", {"objective": "max_utility", "budget": 2500})
    return out


# ----------------------------------------------------------------------------- the problems in landschaft

ZONE_MATRIX = [0.2, 0.1, 0.15, 0.3, 0.1, 0.1, 0.1, 0.05, 0.2, 0.1, 0.05, 0, 0.1, 0.05, 0.05]
ZONES = oc.ZONES


def zone_matrix() -> np.ndarray:
    """R's matrix(c(...), ncol = 3): features x zones."""
    return np.array(ZONE_MATRIX).reshape(3, 5).T


def units_problem(case: dict, data: Data):
    kind = case["units"]
    if kind == "raster":
        return ls.prioritize.problem(data.cost, data.features)
    if kind == "table":
        return ls.prioritize.problem(data.table, FEATURES, cost_column="cost")
    if kind == "rij":
        return ls.prioritize.problem(data.table[["id", "cost"]], oc.spec_table(), cost_column="cost",
                                     rij=oc.rij_table())
    if kind == "polygons":
        return ls.prioritize.problem(data.polygons, data.features, cost_column="cost")
    if kind.startswith("h3_"):
        hexes = ls.prioritize.h3_units(data.cost, int(kind[3:]), n_threads=THREADS)
        # the hexagons go to Terraflux as they are: the cases lie at x = 0 of UTM zone 23S (latitude -85), where
        # PROJ's transverse Mercator differs between versions by 1e-8 m (9.5.1 here, 9.7.1 in Terraflux's GDAL),
        # enough to move the amounts of the cells a hexagon only touches
        path = REF / "vectors" / f"parity_h3_{data.name}_{kind[3:]}.gpkg"
        if path.exists():
            path.unlink()
        hexes.to_file(path, layer=path.stem, driver="GPKG")
        return ls.prioritize.problem(hexes, data.features, cost_column="cost")
    if kind == "zones_raster":
        return oc._zone_general()
    if kind == "zones_table":
        return oc._zone_table_problem()
    raise ValueError(kind)


def resolved(case: dict, p) -> dict:
    """The case's settings with the budget's share turned into a number (of the units' total cost)."""
    params = dict(case["params"])
    b = params.get("budget")
    if isinstance(b, float) and b < 1:
        total = float(np.nansum(p.units.cost))
        params["budget"] = float(round(b * total))
    if params.get("relative_targets") == "ZONE_MATRIX":
        params["relative_targets"] = zone_matrix().tolist()
    return params


def build(case: dict, data: Data, params: dict, p=None):
    """The problem of the case (its units, objective, targets, weights, penalties, locks, decisions)."""
    p = units_problem(case, data) if p is None else p
    kind = case["units"]
    obj = params.get("objective", "min_set")
    budget = params.get("budget")
    if isinstance(budget, list):
        budget = dict(zip(ZONES, budget)) if len(budget) == 3 else budget
    zoned = kind.startswith("zones")
    if obj == "min_set":
        p = p.add_min_set_objective()
    elif obj == "min_extinction_risk":
        p = p.add_min_extinction_risk_objective(budget)
    elif obj in ("max_phylo_div", "max_phylo_end"):
        p = getattr(p, f"add_{obj}_objective")(budget, params["tree"])
    else:
        p = getattr(p, f"add_{obj}_objective")(budget)
    if "relative_targets" in params:
        t = params["relative_targets"]
        p = p.add_relative_targets(np.asarray(t) if isinstance(t, list) and zoned else t)
    if "absolute_targets" in params:
        p = p.add_absolute_targets(params["absolute_targets"])
    if "feature_weights" in params:
        p = p.add_feature_weights(params["feature_weights"])
    if "boundary_penalty" in params:
        if kind == "table":  # a table's boundary is a matrix of its rows
            p = p.add_boundary_penalties(params["boundary_penalty"], data=oc._matrix(data.bnd, len(data.table)))
        else:
            p = p.add_boundary_penalties(params["boundary_penalty"])
    if params.get("contiguity"):
        p = p.add_contiguity_constraints()
    if case.get("locks"):
        if kind == "raster":
            p = p.add_locked_in_constraints(oc.locked_in()).add_locked_out_constraints(oc.locked_out())
        elif kind == "table":
            p = p.add_locked_in_constraints("locked_in").add_locked_out_constraints("locked_out")
        elif kind == "zones_table":
            p = p.add_locked_in_constraints([f"locked_in_{z}" for z in ZONES]).add_locked_out_constraints(
                [f"locked_out_{z}" for z in ZONES])
    return p.add_binary_decisions()


# ----------------------------------------------------------------------------- solving

def solved(p, gap: float, limit: float):
    t0 = time.perf_counter()
    s = p.add_highs_solver(gap=gap, time_limit=limit, n_threads=THREADS).solve()
    return s, time.perf_counter() - t0


def model_of(p) -> dict:
    from landschaft.prioritize._milp_compile import compile_problem

    m, _ = compile_problem(p)
    nz = m.A.copy()
    nz.eliminate_zeros()
    return {"nrow": m.nrow, "ncol": m.ncol, "nnz": int(nz.nnz), "rhs_sum": float(np.sum(m.rhs)),
            "obj_sum": float(np.sum(m.obj)), "a_sum": float(nz.sum()), "rhs_head": [float(v) for v in m.rhs[:16]]}


def is_unique(p, s) -> bool | None:
    """Whether the optimum's selection is the only one: the model with a cut that excludes it (the decisions' columns
    at 1 where selected, at 0 where not, not all as the solution) has a worse optimum, or none."""
    from landschaft.prioritize._milp_compile import compile_problem
    from landschaft.prioritize._milp_solve import solve_model

    m, _ = compile_problem(p)
    cols = np.flatnonzero((np.asarray(m.col_ids) == "pu") & (np.asarray(m.vtype) == "B"))
    if cols.size != m.col_ids.count("pu"):
        return None  # proportion or semicontinuous decisions
    x = np.asarray(s.values, dtype=np.float64)
    x = (x.T.ravel() if x.ndim == 2 else x.ravel())[: cols.size]
    on = x > 0.5
    row = np.zeros(m.ncol)
    row[cols] = np.where(on, 1.0, -1.0)
    m2 = dataclasses.replace(
        m, A=sp.vstack([m.A, sp.csr_matrix(row)], format="csr"), rhs=np.append(m.rhs, on.sum() - 1.0),
        sense=(*m.sense, "<="), row_ids=(*m.row_ids, "no_good"))
    r = solve_model(m2, "highs", {"gap": 0.0, "n_threads": THREADS, "time_limit": 20.0}, engines=False)
    if r["status"] == "infeasible":
        return True
    if r["status"] != "optimal":
        return None
    return abs(r["objective"] - s.objective) > 1e-9 * max(1.0, abs(s.objective))


def run_case(case: dict, datas: dict) -> dict:
    data = datas[case["data"]]
    base = units_problem(case, data)
    params = resolved(case, base)
    p = build(case, data, params, base)
    out = {"name": case["name"], "data": case["data"], "units": case["units"], "params": params}
    if case.get("locks"):
        out["locks"] = True
    out["model"] = model_of(p)
    # the first gap HiGHS proves within the time limit
    s = None
    for gap in GAPS:
        s, secs = solved(p, gap, TRY_SECONDS)
        if s.status == "optimal":
            break
    out["gap_asked"], out["time_limit"] = gap, TRY_SECONDS
    times = [secs]
    if s.status == "optimal":
        times += [solved(p, gap, TRY_SECONDS)[1] for _ in range(2)]
    out.update({"status": s.status, "objective": float(s.objective), "bound": float(s.bound),
                "gap": float(s.gap), "seconds": float(np.median(times)), "runs": len(times),
                "solver_seconds": float(s.seconds)})
    x = np.asarray(s.values, dtype=np.float64)
    x = x.T.ravel() if x.ndim == 2 else x.ravel()
    out["n"] = int(x.size)
    out["selected"] = [int(i) for i in np.flatnonzero(x > 0.5)]
    out["unique"] = is_unique(p, s) if gap == 0.0 and s.status == "optimal" else None
    print(f"{out['name']:42s} {out['status']:10s} obj {out['objective']:14.6f} gap {gap:<6} {out['seconds']:7.3f} s"
          f"  unique {out['unique']}  {out['model']['nrow']}x{out['model']['ncol']}", flush=True)
    return out


def main() -> None:
    # the larger set's data, for Terraflux's test
    cost, f = medium_arrays()
    np.savez_compressed(REF / "optimize_parity_inputs.npz", pu_cost=np.ascontiguousarray(cost),
                        features=np.ascontiguousarray(f))
    path = REF / "vectors" / "parity_polygons.gpkg"
    if path.exists():
        path.unlink()
    medium_polygons().to_file(path, layer="parity_polygons", driver="GPKG")
    datas = {"small": Data("small"), "medium": Data("medium")}
    only = sys.argv[1:]  # names (or parts of them) to solve again, the others kept from the file
    out = REF / "optimize_parity.json"
    rows = [run_case(c, datas) for c in cases() if not only or any(o in c["name"] for o in only)]
    if only and out.exists():
        old = {r["name"]: r for r in json.loads(out.read_text(encoding="utf-8"))["cases"]}
        old.update({r["name"]: r for r in rows})
        rows = [old[c["name"]] for c in cases() if c["name"] in old]
    doc = {"landschaft": ls.__version__, "threads": THREADS, "cases": rows}
    out.write_text(json.dumps(doc, indent=0, separators=(",", ":")) + "\n", encoding="utf-8")
    print(out, len(rows), "cases", math.fsum(r["seconds"] for r in rows), "s")


if __name__ == "__main__":
    main()
