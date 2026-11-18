# SPDX-License-Identifier: GPL-3.0-or-later
"""What Linkage Mapper's Linkage Pathways gives on a small deterministic grid, for Terraflux's tests of
`linkage.pathways` (crates/ops/tests/linkage.rs), written to tests/data/reference/linkage.json:

    python tools/make_linkage_reference.py

The cost-weighted distances are scikit-image's MCP_Geometric (a step the mean of the two cells' costs times its
length, sqrt(2)-diagonal, `sampling` the cells' sizes), independent of the Rust search; the allocation zones, the
adjacency, the least-cost distances, the normalized corridors and their mosaic, the links' filters and the paths'
lengths are written here in numpy. The grid is 40 x 50 cells of 30 x 20 map units (not square, so the sampling
counts), four cores, a low-resistance diagonal that runs through the middle core (a link that crosses a core) and
a barrier of cells without a value.

The pinch points (`linkage.pinchpoints`) are the same landscape's: the band of each kept link (the two cores' CWDs
added, less the LCD, within `width`) is made in numpy from the MCP_Geometric distances, and the circuit is the
library's Circuitscape (terraflux-py's `circuitscape_pairwise` between the two cores' cells in the band, and
`circuitscape_one_to_all` in all-to-one, as Linkage Mapper's s8_pinchpoints.py calls it), so it needs the library
importable. Its compiled module is looked for as `terraflux._core`; where the package is not installed with it,
TERRAFLUX_CORE names the file of the module (a `_core*.pyd` or `.so`) and the package is taken from
extern/terraflux-py/src.

The barriers (`linkage.barriers`) are Linkage Mapper's Barrier Mapper on the kept links of `adjacency = all`, from
the same MCP_Geometric distances, in numpy and scipy.ndimage: the minimum of each core's distance over the annulus
(the cells whose centre is at a distance above R - 1 and up to R map units from the cell's centre; ArcGIS's
rule is assumed, its source is not available), the score (LCD - fA - fB - 2R restored) / 2R where neither minimum
is 0, the links' mosaic (max, or sum with negatives and no data 0), the negatives dropped, the circles (the maximum
over the disk of centre distance up to R) and the maximum over the radii, in the key `barriers`.
"""

from __future__ import annotations

import importlib.util
import json
import os
import sys
import types
from pathlib import Path

os.environ.setdefault("OMP_NUM_THREADS", "4")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "4")

import numpy as np  # noqa: E402
from scipy.spatial.distance import cdist  # noqa: E402
from skimage.graph import MCP_Geometric  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"

SRC = ROOT / "extern" / "terraflux-py" / "src" / "terraflux"

NY, NX = 40, 50
DX, DY = 30.0, 20.0


def landscape():
    rng = np.random.default_rng(7)
    res = np.exp(rng.normal(1.0, 0.7, (NY, NX)))
    # a low-resistance diagonal from the first core to the last, through the middle one
    for k in range(60):
        r, c = 6 + k * 28 // 60, 4 + k * 40 // 60
        res[r, c] = 0.2
    res[10:30, 12] = np.nan  # a barrier with a gap
    res[18:22, 12] = 3.0
    cores = np.zeros((NY, NX))
    cores[5:9, 2:7] = 3
    cores[18:23, 20:25] = 8
    cores[30:35, 42:48] = 11
    cores[2:6, 38:45] = 5
    return res, cores


def surface(res, cells):
    cost = np.where(np.isfinite(res) & (res > 0), res, np.inf)
    mcp = MCP_Geometric(cost, fully_connected=True, sampling=(DY, DX))
    d, _ = mcp.find_costs([tuple(c) for c in cells])
    return d, mcp


def touching(label):
    pairs = set()
    for dr, dc in [(0, 1), (1, 0), (1, 1), (1, -1)]:
        a = label[0 : NY - dr, max(0, -dc) : NX - max(0, dc)]
        b = label[dr:NY, max(0, dc) : NX - max(0, -dc)]
        m = (a > 0) & (b > 0) & (a != b)
        for x, y in zip(a[m], b[m]):
            pairs.add((int(min(x, y)), int(max(x, y))))
    return pairs


def seg(a, b):
    dr, dc = abs(a[0] - b[0]), abs(a[1] - b[1])
    if dr and dc:
        return float(np.hypot(DY, DX))
    return DY if dr else DX


def link_table(res, cores, cfg):
    ids = sorted({int(v) for v in cores[cores > 0]})
    cells = {i: np.argwhere(cores == i) for i in ids}
    surf, mcp = {}, {}
    for i in ids:
        surf[i], mcp[i] = surface(res, cells[i])
    stack = np.stack([surf[i] for i in ids])
    adjacency = cfg.get("adjacency", "cost")
    if adjacency == "cost":
        zone = np.where(np.isfinite(stack.min(axis=0)), np.argmin(stack, axis=0) + 1, 0)
        label = np.array(ids)[zone - 1] * (zone > 0)
        adj = touching(label)
    elif adjacency == "euclidean":
        pts = np.argwhere(cores > 0)
        centres = np.c_[pts[:, 0] * DY, pts[:, 1] * DX]
        allc = np.argwhere(np.ones((NY, NX)))
        d = cdist(np.c_[allc[:, 0] * DY, allc[:, 1] * DX], centres)
        label = cores[pts[np.argmin(d, axis=1), 0], pts[np.argmin(d, axis=1), 1]].reshape(NY, NX).astype(int)
        adj = touching(label)
    else:
        adj = {(a, b) for a in ids for b in ids if a < b}
    rows = []
    for a in ids:
        for b in ids:
            if a >= b:
                continue
            row = dict(frm=a, to=b, euclid=None, lcd=None, plen=None, kept=False, reason="")
            if (a, b) in adj:
                ca = np.c_[cells[a][:, 0] * DY, cells[a][:, 1] * DX]
                cb = np.c_[cells[b][:, 0] * DY, cells[b][:, 1] * DX]
                row["euclid"] = float(cdist(ca, cb).min())
                if cfg.get("max_euclidean") is not None and row["euclid"] > cfg["max_euclidean"]:
                    row["reason"] = "max_euclidean"
                rows.append(row)
    for row in rows:
        if row["reason"] != "":
            continue
        a, b = row["frm"], row["to"]
        s = surf[a] + surf[b]
        k = int(np.argmin(s))
        cell = (k // NX, k % NX)
        row["lcd"] = float(s.flat[k])
        row["cell"] = cell
        pa = mcp[a].traceback(cell)
        pb = mcp[b].traceback(cell)[::-1]
        path = pa + pb[1:]
        row["plen"] = sum(seg(p, q) for p, q in zip(path, path[1:]))
        row["cross"] = next((int(cores[p]) for p in path if cores[p] not in (0, a, b)), None)
        if cfg.get("max_cwd") is not None and row["lcd"] > cfg["max_cwd"]:
            row["reason"] = "max_cwd"
        elif row["cross"] is not None and cfg.get("drop_crossing", True):
            row["reason"] = f"crosses core {row['cross']}"
    if cfg.get("nearest"):
        keep = set()
        for i in ids:
            mine = sorted(
                (r["lcd"], n) for n, r in enumerate(rows) if r["reason"] == "" and i in (r["frm"], r["to"])
            )
            keep.update(n for _, n in mine[: cfg["nearest"]])
        for n, r in enumerate(rows):
            if r["reason"] == "" and n not in keep:
                r["reason"] = "not nearest"
    mosaic = np.full((NY, NX), np.inf)
    for r in rows:
        if r["reason"] == "":
            r["kept"] = True
            mosaic = np.minimum(mosaic, surf[r["frm"]] + surf[r["to"]] - r["lcd"])
    if cfg.get("width") is not None:
        mosaic[mosaic > cfg["width"]] = np.inf
    mosaic[~np.isfinite(mosaic)] = np.nan
    return rows, mosaic, surf, stack


def circuit_module():
    """terraflux.circuit: the installed library's, else the submodule's source with the module named by TERRAFLUX_CORE."""
    try:
        from terraflux import circuit

        return circuit
    except ImportError:
        pass
    for k in [k for k in sys.modules if k == "terraflux" or k.startswith("terraflux.")]:
        del sys.modules[k]
    pkg = types.ModuleType("terraflux")
    pkg.__path__ = [str(SRC)]
    sys.modules["terraflux"] = pkg
    core = os.environ.get("TERRAFLUX_CORE")
    if core:
        spec = importlib.util.spec_from_file_location("terraflux._core", core)
        mod = importlib.util.module_from_spec(spec)
        sys.modules["terraflux._core"] = mod
        spec.loader.exec_module(mod)
    from terraflux import circuit

    return circuit


def pinch(circuit, res, cores, cfg, width, mode, square):
    """Linkage Mapper's pinch points of the kept links of `cfg`: the current map and a row per link."""
    rows, _, surf, _ = link_table(res, cores, cfg)
    kept = [r for r in rows if r["kept"]]
    cost = np.where(np.isfinite(res) & (res > 0), res, np.nan)
    r_circuit = cost * cost if square else cost
    out_rows = []
    if mode == "pairwise":
        current = np.full((NY, NX), np.nan)
        for r in kept:
            a, b = r["frm"], r["to"]
            band = (surf[a] + surf[b] - r["lcd"]) <= width
            hab = np.where(band, r_circuit, np.nan)
            pts = np.where(band & (cores == a), 1, np.where(band & (cores == b), 2, 0))
            res_ = circuit.circuitscape_pairwise(hab, pts, cum_map=True, set_focal_node_currents_to_zero=True)
            cur = np.where(np.isfinite(hab) & (pts == 0), res_.cum_current, np.nan)
            current = np.where(np.isnan(current) | (cur > current), np.where(np.isnan(cur), current, cur), current)
            eff = float(res_.resistances[1, 2]) * DY
            out_rows.append(
                {"from": a, "to": b, "lcd": r["lcd"], "resistance": eff,
                 "ratio": None if square else r["lcd"] / eff, "nodes": int(np.isfinite(hab).sum())}
            )
    else:
        band = np.zeros((NY, NX), dtype=bool)
        for r in kept:
            band |= (surf[r["frm"]] + surf[r["to"]] - r["lcd"]) <= width
        band |= (cores > 0) & np.isfinite(cost)
        hab = np.where(band, r_circuit, np.nan)
        pts = np.where((cores > 0) & np.isfinite(cost), cores, 0).astype(int)
        res_ = circuit.circuitscape_one_to_all(hab, pts, all_to_one=True, cum_map=True, set_focal_node_currents_to_zero=True)
        current = np.where(band & (pts == 0), res_.cum_current, np.nan)
        out_rows = [{"from": r["frm"], "to": r["to"], "lcd": r["lcd"]} for r in kept]
    return current, out_rows


def footprint(radius, inner=None):
    """The offsets' mask of the cells whose centre is at a distance above `inner` (none: the centre too) and up to
    `radius` map units from a cell's centre: the annulus (inner = R - 1) and the circle of Linkage Mapper's
    FocalStatistics (an assumption on ArcGIS: a cell is in when its centre is)."""
    kr, kc = int(np.floor(radius / DY)), int(np.floor(radius / DX))
    dr, dc = np.mgrid[-kr : kr + 1, -kc : kc + 1]
    d = np.sqrt((dc * DX) ** 2 + (dr * DY) ** 2)
    return (d <= radius) if inner is None else ((d <= radius) & (d > inner))


def focal(a, mask, least):
    """The least (or greatest) value of the data (finite) cells of `a` under `mask` around each cell; NaN if none."""
    from scipy import ndimage

    fill = np.inf if least else -np.inf
    x = np.where(np.isfinite(a), a, fill)
    f = ndimage.minimum_filter if least else ndimage.maximum_filter
    out = f(x, footprint=mask, mode="constant", cval=fill)
    return np.where(np.isfinite(out), out, np.nan)


def barriers(res, cores, cfg, min_radius, max_radius=None, step=None, restored=1.0, combine="max", cwd_threshold=None):
    """Linkage Mapper's Barrier Mapper on the kept links of `cfg`: improvement (centres), circles, percent, radius."""
    rows, _, surf, _ = link_table(res, cores, cfg)
    kept = [r for r in rows if r["kept"]]
    if cwd_threshold is not None:
        surf = {i: np.where(s < cwd_threshold, s, np.nan) for i, s in surf.items()}
    max_radius = min_radius if max_radius is None else max_radius
    count = 1 if max_radius == min_radius else int(np.floor((max_radius - min_radius) / step + 1e-9)) + 1
    nan = np.full((NY, NX), np.nan)
    imp, circles, pct, radius = nan.copy(), nan.copy(), nan.copy(), nan.copy()

    def better(best, v):
        return np.where(np.isnan(v), False, np.isnan(best) | (v > best))

    for k in range(count):
        r = min_radius + k * (step or 0.0)
        dia = 2 * r
        ring, disk = footprint(r, r - 1), footprint(r)
        near = {i: focal(s, ring, True) for i, s in surf.items()}
        ms = np.zeros((NY, NX)) if combine == "sum" else nan.copy()
        mp = ms.copy()
        for l in kept:
            fa, fb = near[l["frm"]], near[l["to"]]
            ok = (fa > 0) & (fb > 0)
            v = np.where(ok, (l["lcd"] - fa - fb - dia * restored) / dia, np.nan)
            q = 100 * v / l["lcd"]
            if combine == "sum":
                ms = ms + np.where(np.isnan(v) | (v < 0), 0.0, v)
                mp = mp + np.where(np.isnan(q) | (q < 0), 0.0, q)
            else:
                ms = np.where(better(ms, v), v, ms)
                mp = np.where(better(mp, q), q, mp)
        ms = np.where(ms < 0, np.nan, ms)
        mp = np.where(mp < 0, np.nan, mp)
        grown = focal(ms, disk, False)
        radius = np.where(better(imp, ms), r, radius)
        imp = np.where(better(imp, ms), ms, imp)
        pct = np.where(better(pct, mp), mp, pct)
        circles = np.where(better(circles, grown), grown, circles)
    return imp, circles, pct, radius


def barrier_reference(res, cores):
    all_links = {"adjacency": "all", "drop_crossing": False}
    barrier_cases = {
        "one_radius": {"min_radius": 60.0},
        "three_radii": {"min_radius": 60.0, "max_radius": 100.0, "step": 20.0},
        "sum": {"min_radius": 60.0, "max_radius": 100.0, "step": 20.0, "combine": "sum"},
        "threshold": {"min_radius": 60.0, "max_radius": 100.0, "step": 20.0, "cwd_threshold": 150.0},
        "restored": {"min_radius": 60.0, "max_radius": 100.0, "step": 20.0, "restored": 0.5},
    }
    out = {}
    for name, params in barrier_cases.items():
        imp, circles, pct, radius = barriers(res, cores, all_links, **params)
        out[name] = {
            "links": all_links,
            "params": params,
            "improvement": clean(imp),
            "circles": clean(circles),
            "percent": clean(pct),
            "radius": clean(radius),
        }
    return out


def clean(x):
    a = np.asarray(x, dtype=float)
    return [None if not np.isfinite(v) else float(v) for v in a.ravel()]


def main():
    res, cores = landscape()
    # an impassable cell inside the grid is NaN in the file, and the map's nodata
    cfgs = {
        "default": {},
        "all_keep_crossing": {"adjacency": "all", "drop_crossing": False},
        "euclidean": {"adjacency": "euclidean", "max_euclidean": 520.0},
        "all_drop_crossing": {"adjacency": "all"},
        "max_cwd": {"adjacency": "all", "drop_crossing": False, "max_cwd": 710.0},
        "nearest": {"adjacency": "all", "drop_crossing": False, "nearest": 1},
        "width": {"width": 400.0},
    }
    out = {
        "ny": NY,
        "nx": NX,
        "dx": DX,
        "dy": DY,
        "resistance": clean(res),
        "cores": [int(v) for v in cores.ravel()],
        "cases": {},
    }
    for name, cfg in cfgs.items():
        rows, mosaic, surf, stack = link_table(res, cores, cfg)
        case = {"params": cfg, "corridors": clean(mosaic), "links": []}
        for r in rows:
            case["links"].append(
                {
                    "from": r["frm"],
                    "to": r["to"],
                    "euclidean": r["euclid"],
                    "lcd": r["lcd"],
                    "path_length": r["plen"],
                    "kept": r["kept"],
                    "reason": r["reason"],
                }
            )
        out["cases"][name] = case
    ids = sorted({int(v) for v in cores[cores > 0]})
    _, _, surf, stack = link_table(res, cores, {})
    out["surfaces"] = {str(i): clean(surf[i]) for i in ids}
    out["cwd"] = clean(stack.min(axis=0))
    circuit = circuit_module()
    pinch_cases = {
        "pairwise": ({}, 350.0, "pairwise", False),
        "pairwise_all": ({"adjacency": "all", "drop_crossing": False}, 300.0, "pairwise", False),
        "square": ({}, 350.0, "pairwise", True),
        "all_to_one": ({}, 350.0, "all_to_one", False),
    }
    out["pinch"] = {}
    for name, (cfg, width, mode, square) in pinch_cases.items():
        current, prows = pinch(circuit, res, cores, cfg, width, mode, square)
        out["pinch"][name] = {
            "links": cfg,
            "width": width,
            "mode": mode,
            "square": square,
            "current": clean(current),
            "rows": prows,
        }
    out["barriers"] = barrier_reference(res, cores)
    path = REF / "linkage.json"
    path.write_text(json.dumps(out) + "\n", encoding="utf-8")
    print(path, {k: [(l["from"], l["to"], l["kept"], l["reason"]) for l in v["links"]] for k, v in out["cases"].items()})


if __name__ == "__main__":
    main()
