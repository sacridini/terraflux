# SPDX-License-Identifier: GPL-3.0-or-later
"""The reference outputs Terraflux is compared with: landschaft's ls.simulate on the same models.

Run with the landschaft the submodule points at installed:

    python tools/make_reference.py

It copies the example rasters into tests/data/inputs, writes the maps that lulcc's reference cases need
(Plum Island's history and restrictions), and for each case the maps of every step and the fitted pieces
(.npy) into tests/data/reference/<case>/.
"""

from __future__ import annotations

import json
import shutil
import sys
import warnings
from pathlib import Path

import numpy as np
import rasterio

import landschaft as ls

S = ls.simulate
ROOT = Path(__file__).resolve().parents[1] / "tests" / "data"
INPUTS = ROOT / "inputs"
REF = ROOT / "reference"

PIE_FORMS = {2: ["elevation", "slope", "dist_built_1985"], 1: ["elevation", "slope"], 3: ["elevation", "slope"]}
RULES = [[1, 1008, 105], [0, 1, 0], [-1, -2, 1]]
NB = {1: 0.3, 2: 0.2, 3: 0.3}
VIGNETTE = {"jitter": 0.0002, "scale": 0.000001, "max_iter": 1000, "max_diff": 50, "mean_diff": 50}


def copy_inputs():
    INPUTS.mkdir(parents=True, exist_ok=True)
    for name in ("pie_landuse", "pie_drivers", "sibuyan_landuse", "sibuyan_drivers"):
        shutil.copyfile(S.example_data(name), INPUTS / f"{name}.tif")
    # lulcc's reference case: a history (0-12 steps, seed 5) and a restricted block, as landschaft's tests
    lu = ls.load_raster(INPUTS / "pie_landuse.tif")[0]
    nan = np.isnan(lu.values)
    hist = np.random.default_rng(5).integers(0, 13, lu.shape).astype(np.float64)
    hist[nan] = np.nan
    restricted = np.zeros(lu.shape)
    restricted[100:200, 150:300] = 1
    with rasterio.open(INPUTS / "pie_landuse.tif") as src:
        profile = src.profile | {"count": 1, "dtype": "float32", "nodata": np.nan}
    for name, data in (("pie_history", hist), ("pie_restrictions", restricted)):
        with rasterio.open(INPUTS / f"{name}.tif", "w", **profile) as dst:
            dst.write(data.astype(np.float32), 1)


def one_band(path):
    da = ls.load_raster(path)
    return da[0] if da.ndim == 3 else da


def save(case: str, arrays: dict, meta: dict | None = None):
    """Small arrays as .npy; the maps of the steps as a compressed GeoTIFF of bytes (255: no class)."""
    d = REF / case
    shutil.rmtree(d, ignore_errors=True)
    d.mkdir(parents=True)
    for k, v in arrays.items():
        v = np.asarray(v, dtype=np.float64)
        if k == "maps":
            profile = {"driver": "GTiff", "count": v.shape[0], "height": v.shape[1], "width": v.shape[2],
                       "dtype": "uint8", "nodata": 255, "compress": "deflate"}
            with rasterio.open(d / "maps.tif", "w", **profile) as dst:
                dst.write(np.where(np.isnan(v), 255, v).astype(np.uint8))
        else:
            np.save(d / f"{k}.npy", np.ascontiguousarray(v))
    if meta:
        (d / "meta.json").write_text(json.dumps(meta, indent=2))


def dinamica():
    lu = ls.load_raster(INPUTS / "sibuyan_landuse.tif")
    dr = ls.load_raster(INPUTS / "sibuyan_drivers.tif")
    model = S.presets.dinamica(lu[0], lu[1], dr, steps=14, woe=250)
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        sim = model.run(lu[0], dr, seed=1)
    obs = S.presets.observed_patches(lu[0], lu[1], model.potential.transitions)
    t = model.potential.table
    names = sorted(t.Variable.unique())
    table = t.assign(Variable=[names.index(v) for v in t.Variable]).to_numpy(dtype=np.float64)
    save("dinamica_sibuyan", {"maps": sim.maps.values[1:], "single": model.demand.single, "multi": model.demand.multi,
                              "weights": table, "observed": obs.to_numpy(dtype=np.float64)},
         {"classes": [float(c) for c in model.demand.classes], "variables": names})


def lulcc(case, allocation, seed, *, elasticity=None, rules=False, nb=False):
    pie = ls.load_raster(INPUTS / "pie_landuse.tif")
    drivers = ls.load_raster(INPUTS / "pie_drivers.tif")
    r = S.Rules(elasticity=elasticity, neighbourhood=S.Neighbourhood(dict(NB)) if nb else None)
    if rules:
        r.allowed = np.array(RULES)
        r.history = one_band(INPUTS / "pie_history.tif")
        r.restrictions = one_band(INPUTS / "pie_restrictions.tif")
    lr = S.LogisticRegression(PIE_FORMS).fit(pie[0], drivers)
    demand = S.AreaDemand.from_maps(pie, times=[0, 6, 14])
    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        sim = S.Model(lr, demand, allocation, r).run(pie[0], drivers, seed=seed)
    save(case, {"maps": sim.maps.values[1:], "demand": demand.table.to_numpy(),
                "coefficients": lr.coefficients.loc[[1, 2, 3]].to_numpy()})


def bridge():
    """Models saved by landschaft (Model.save) that Terraflux must run as ls.simulate does."""
    lu = ls.load_raster(INPUTS / "sibuyan_landuse.tif")
    dr = ls.load_raster(INPUTS / "sibuyan_drivers.tif")
    names = [str(v) for v in dr.band.values]
    drivers = {n: dr[k] for k, n in enumerate(names)}
    land_path, drv_path = "../../inputs/sibuyan_landuse.tif", "../../inputs/sibuyan_drivers.tif"

    def write(case, model, drv_for_save, run_drivers, seed):
        d = ROOT / "bridge" / case
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        model.save(d / "model.json", land_path, drv_for_save, seed=seed)
        with warnings.catch_warnings():
            warnings.simplefilter("ignore")
            maps = model.run(lu[0], run_drivers, seed=seed).maps.values[1:]
        profile = {"driver": "GTiff", "count": maps.shape[0], "height": maps.shape[1], "width": maps.shape[2],
                   "dtype": "uint8", "nodata": 255, "compress": "deflate"}
        with rasterio.open(d / "expected.tif", "w", **profile) as dst:
            dst.write(np.where(np.isnan(maps), 255, maps).astype(np.uint8))

    write("dinamica", S.presets.dinamica(lu[0], lu[1], dr, steps=14, woe=250), drv_path, dr, 3)
    # CLUE-S with a dynamic variable (the share of forest around) and neighbourhood rules
    share = S.share_of(1, 3)
    lr = S.LogisticRegression().fit(lu[0], {**drivers, "forest_share": share(lu[0])})
    model = S.Model(lr, S.AreaDemand.from_maps(lu, times=[0, 14]), S.CLUE(max_diff=20, mean_diff=10),
                    S.Rules(elasticity=0.3, neighbourhood=S.Neighbourhood({2: 0.1})), dynamic={"forest_share": share})
    write("clue_dynamic", model, drv_path, dr, 5)
    model = S.Model(S.LogisticRegression().fit(lu[0], dr), S.Markov.from_maps(lu[0], lu[1], steps=14),
                    S.CARS(steps=3, step_size=20, max_iter=500))
    write("cars", model, drv_path, dr, 7)
    # a scikit-learn estimator, written as its predictions on the start
    from sklearn.linear_model import LogisticRegression as SkLogit

    est = S.Estimator(SkLogit(max_iter=500), sample=4000, seed=1).fit(lu[0], dr)
    model = S.Model(est, S.AreaDemand.from_maps(lu, times=[0, 14]), S.CLUE(max_diff=20, mean_diff=10),
                    S.Rules(elasticity=0.3))
    write("estimator", model, drv_path, dr, 9)
    # regional demands: three regions in vertical bands, a corner of the map in none
    shape = lu[0].shape
    r = np.ones(shape)
    r[:, shape[1] // 3:] = 2
    r[:, 2 * shape[1] // 3:] = 3
    r[: shape[0] // 4, 2 * shape[1] // 3:] = np.nan
    regions = lu[0].copy(data=r)
    lr = S.LogisticRegression().fit(lu[0], dr)
    model = S.Model(lr, S.Regional.from_maps(regions, lu, times=[0, 14]), S.CLUE(max_diff=20, mean_diff=10),
                    S.Rules(elasticity=0.3))
    write("clue_regional", model, drv_path, dr, 11)
    model = S.Model(lr, S.Regional.markov(regions, lu[0], lu[1], steps=14), S.CARS(steps=3, step_size=20,
                                                                                  max_iter=500))
    write("cars_regional", model, drv_path, dr, 12)
    dyn = S.presets.dinamica(lu[0], lu[1], dr, steps=14, woe=250)
    rates = {1: {(1, 2): 0.01, (3, 2): 0.02}, 2: {(1, 2): 0.03, (1, 3): 0.01}, 3: {(3, 4): 0.02, (4, 3): 0.01}}
    alloc = S.ExpanderPatcher(expansion=0.5, expander=(4, 2, 1.5), patcher=(6, 4, 1.5), steps=5)
    write("dinamica_regional", S.Model(dyn.potential, S.Regional(regions, rates), alloc), drv_path, dr, 13)
    # the accumulated cost from built land over the slope, a dynamic variable
    tr = ls.cost.geo_correction(1 / ls.cost.transition(dr.sel(band="slope").fillna(0) + 1, "mean", 8))
    cost = S.distance_to(2, cost=tr)
    lr = S.LogisticRegression().fit(lu[0], {**drivers, "cost_built": cost(lu[0])})
    model = S.Model(lr, S.AreaDemand.from_maps(lu, times=[0, 14]), S.CLUE(max_diff=20, mean_diff=10),
                    S.Rules(elasticity=0.3), dynamic={"cost_built": cost})
    write("clue_cost", model, drv_path, dr, 14)


def main():
    if sys.argv[1:] == ["bridge"]:  # only the models saved by landschaft
        bridge()
        return
    copy_inputs()
    dinamica()
    lulcc("clues_pie", S.CLUE(**VIGNETTE), 1, elasticity=[0.2, 0.2, 0.2])
    lulcc("clues_pie_rules", S.CLUE(**VIGNETTE), 11, elasticity=[0.2, 0.6, 0.1], rules=True)
    lulcc("clues_pie_neighb", S.CLUE(**VIGNETTE), 13, elasticity=[0.2, 0.2, 0.2], nb=True)
    lulcc("ordered_pie", S.Ordered([2, 1, 3], stochastic=True), 3)
    lulcc("ordered_pie_det", S.Ordered([2, 1, 3], stochastic=False), 3)
    lulcc("ordered_pie_rules", S.Ordered([2, 1, 3], stochastic=True), 5, rules=True, nb=True)
    bridge()
    print(f"landschaft {ls.__version__}: references written to {REF}")


if __name__ == "__main__":
    main()
