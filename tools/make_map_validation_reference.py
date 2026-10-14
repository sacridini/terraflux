# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's comparisons of simulated and observed maps give, for Terraflux's tests, written to
tests/data/reference/map_validation.npz and .json:

    python tools/make_map_validation_reference.py

Three maps of four classes (40 x 52 cells of 30 m, a few missing): lulcc's three-map comparison at blocks of 1, 2,
4 and 7 cells (its tables, budget and figures of merit), MOLUSCE's kappas and error budget (scales 2 and 3); and
three maps of developed land (0/1) and of three classes: FUTURES' validation with and without the original map,
by tiles and by regions.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np
import xarray as xr
from affine import Affine

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
S = ls.simulate


def grid(values: np.ndarray) -> xr.DataArray:
    ny, nx = values.shape
    da = xr.DataArray(values, dims=("y", "x"), coords={"y": 5000 + (np.arange(ny) + 0.5) * -30.0,
                                                     "x": 1000 + (np.arange(nx) + 0.5) * 30.0})
    return da.rio.write_transform(Affine(30.0, 0, 1000.0, 0, -30.0, 5000.0))


def main() -> None:
    ny, nx = 40, 52
    rng = np.random.default_rng(11)
    t0 = ls.nlm.random_cluster(nx, ny, p=0.5, ai=[0.4, 0.3, 0.2, 0.1], seed=12, rescale=False).values.astype(float)
    t1 = t0.copy()
    ch = rng.random((ny, nx)) < 0.2
    t1[ch] = rng.integers(1, 5, int(ch.sum()))
    sim = t0.copy()
    ch2 = rng.random((ny, nx)) < 0.2
    sim[ch2] = rng.integers(1, 5, int(ch2.sum()))
    t0[2:4, 3:6] = np.nan
    sim[30, 40] = np.nan
    out: dict[str, np.ndarray] = {"in/t0": t0, "in/t1": t1, "in/sim": sim}
    T0, T1, SIM = grid(t0), grid(t1), grid(sim)
    factors = [1, 2, 4, 7]
    tmc = S.three_map_comparison(T0, T1, SIM, factors=factors)
    for f in factors:
        out[f"tmc/{f}"] = tmc.tables[f]
    b = tmc.agreement_budget()
    out["tmc/budget"] = b[["factor", "misses", "hits", "wrong_hits", "false_alarms", "correct_rejections"]].to_numpy(float)
    fom = tmc.figure_of_merit()
    out["tmc/fom"] = np.column_stack([fom["overall"].to_numpy(), fom["category"].to_numpy()])
    out["tmc/transition"] = np.concatenate([fom["transition"][f].to_numpy() for f in factors])
    k = S.compare_maps(T1, SIM)
    out["kappa"] = np.array([k[n] for n in ("kappa", "kappa_location", "kappa_histogram", "correctness", "cramer",
                                            "jiu", "correlation")])
    for scale in (2, 3):
        out[f"budget/{scale}"] = S.error_budget(T1, SIM, iterations=4, scale=scale).to_numpy(float)
    # FUTURES: developed land
    orig = (ls.nlm.gaussian_field(nx, ny, autocorr_range=10, seed=20).values > 0.7).astype(float)
    ref = orig.copy()
    ref[(ls.nlm.gaussian_field(nx, ny, autocorr_range=6, seed=21).values > 0.6)] = 1.0
    fsim = orig.copy()
    fsim[(ls.nlm.gaussian_field(nx, ny, autocorr_range=6, seed=22).values > 0.62)] = 1.0
    ref[0, :4] = np.nan
    regions = np.floor(ls.nlm.gaussian_field(nx, ny, autocorr_range=12, seed=23).values * 4) + 1
    regions[5, 5] = np.nan
    three = np.where(np.isnan(t1), np.nan, t1 % 3)
    out.update({"in/orig": orig, "in/ref": ref, "in/fsim": fsim, "in/regions": regions, "in/three": three})
    O, R, F, G, TH = grid(orig), grid(ref), grid(fsim), grid(regions), grid(three)
    meta = {"futures": {}, "grid": {}}

    def scal(r):
        names = ["total_quantity", "total_allocation", "kappa", "kappasimulation", "misses", "hits", "false_alarms",
                 "null_successes", "figure_of_merit", "producer", "user", "initially_developed"]
        return [k for k in names if k in r], [np.nan if r[k] is None else float(r[k]) for k in names if k in r]

    for name, (a, b2, o) in {"binary": (R, F, O), "no_original": (R, F, None), "classes": (TH, grid(sim % 3), grid(t0 % 3))}.items():
        r = S.futures_validation(a, b2, o)
        names, vals = scal(r)
        out[f"futures/{name}"] = np.array(vals)
        out[f"futures/{name}/classes"] = np.column_stack([r["classes"], r["quantity"], r["allocation"]])
        meta["futures"][name] = names
    out["in/sim3"] = sim % 3
    out["in/t03"] = t0 % 3
    for name, kw in {"tiles": {"size": 15}, "regions": {"regions": G}, "tiles_no_original": {"size": 20}}.items():
        o = None if name == "tiles_no_original" else O
        t = S.futures_validation_grid(R, F, o, **kw)
        out[f"grid/{name}"] = t.to_numpy(dtype=float)
        meta["grid"][name] = list(t.columns)
    np.savez_compressed(REF / "map_validation.npz", **out)
    (REF / "map_validation.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "map_validation.npz", len(out))


if __name__ == "__main__":
    main()
