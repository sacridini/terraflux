"""Figures of the tutorial "FUTURES: urban growth by patches" (land-change/futures).

FUTURES on Plum Island Ecosystems (lulcc's example: class 2 is built land; 1985, 1991, 1999), two subregions
(the west and the east of the map, in the place of counties):

1. a Terraflux model fits the pieces on 1985-1991: the development pressure of 1985 (futures.devpressure), the
   potential (a logistic regression of the cells newly developed in each subregion on the pressure, the slope
   and the elevation: glm.fit, in the place of r.futures.potential's mixed model) and the patch library
   (futures.patch_library);
2. futures.calibrate tries compactness means and ranges on 1985-1991;
3. futures.simulation from 1991 for 8 years with the demand of the trend, compared with 1999;
4. the same with other incentive powers (infill against sprawl).

    python website/figures/land-change/futures_pie.py
"""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "land-change/futures"
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
DEV = {0: ("Not developed", "#e9e4d8"), 1: ("Developed", "#b4508a")}
DEV3 = {0: ("Not developed", "#e9e4d8"), 1: ("Developed in 1991", "#8f8f8f"), 2: ("Developed after 1991", "#b4508a")}


def new3(a, dev91):
    return np.where(np.isnan(a), np.nan, np.where(dev91 == 1, 1.0, np.where(a == 1, 2.0, 0.0)))

WIN = (slice(170, 370), slice(150, 400))
PRESSURE = dict(neighborhood=10, approach="gravity", gamma=1.5, scaling_factor=1.0)


def inline(v) -> str:
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{k} = {inline(x)}" for k, x in v.items()) + " }"
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(inline(x) for x in v) + "]"
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return json.dumps(v)
    if v is None:
        return "[]"
    return repr(round(v, 6)) if isinstance(v, float) else str(v)


def read(path, band=1):
    import rasterio

    with rasterio.open(path) as d:
        a = d.read(band).astype(float)
        if d.nodata is not None and not np.isnan(d.nodata):
            a[a == d.nodata] = np.nan
    return a


def write_like(path, a, like):
    import rasterio

    with rasterio.open(like) as src:
        prof = src.profile
    prof.update(count=1, dtype="float32", nodata=float("nan"))
    with rasterio.open(path, "w", **prof) as dst:
        dst.write(a.astype("float32"), 1)


FIT = """format = "terraflux/1"
name = "FUTURES: the pieces, fitted on 1985-1991"

[[nodes]]
id = "lu1985"
op = "raster.read"
params = { path = "pie_landuse.tif", band = "1985" }

[[nodes]]
id = "lu1991"
op = "raster.read"
params = { path = "pie_landuse.tif", band = "1991" }

[[nodes]]
id = "drivers"
op = "raster.read"
params = { path = "pie_drivers.tif" }

[[nodes]]
id = "counties"
op = "raster.read"
params = { path = "subregions.tif" }

[[nodes]]
id = "dev1985"
op = "raster.calc"
inputs = { lu = "lu1985.raster" }
params = { expression = "lu == 2" }

[[nodes]]
id = "dev1991"
op = "raster.calc"
inputs = { lu = "lu1991.raster" }
params = { expression = "lu == 2" }

[[nodes]]
id = "pressure"
op = "futures.devpressure"
inputs = { developed = "dev1985.map" }
params = { size = 10, method = "gravity", gamma = 1.5, scaling_factor = 1.0 }

[[nodes]]
id = "predictors"
op = "stack.set"
inputs = { stack = "drivers.raster", map = "pressure.map" }
params = { name = "devpressure" }

[[nodes]]
id = "new_west"
op = "raster.calc"
inputs = { a = "dev1985.map", b = "dev1991.map", c = "counties.raster" }
params = { expression = "None if a == 1 or c != 1 else b" }

[[nodes]]
id = "new_east"
op = "raster.calc"
inputs = { a = "dev1985.map", b = "dev1991.map", c = "counties.raster" }
params = { expression = "None if a == 1 or c != 2 else b" }

[[nodes]]
id = "potential_west"
op = "glm.fit"
inputs = { landscape = "new_west.map", drivers = "predictors.stack" }
params = { formulas = { 1 = ["devpressure", "slope", "elevation"] } }

[[nodes]]
id = "potential_east"
op = "glm.fit"
inputs = { landscape = "new_east.map", drivers = "predictors.stack" }
params = { formulas = { 1 = ["devpressure", "slope", "elevation"] } }

[[nodes]]
id = "library"
op = "futures.patch_library"
inputs = { start = "dev1985.map", end = "dev1991.map", subregions = "counties.raster" }
params = { per_subregion = true }

[outputs.west]
from = "potential_west.model"

[outputs.east]
from = "potential_east.model"

[outputs.library]
from = "library.library"

[outputs.patches]
from = "library.table"

[outputs.pressure]
from = "pressure.map"
"""


def simulation(potential, demand, library, compactness_mean=0.4, compactness_range=0.05, incentive=1.0,
               start="lu1991", seed=1, calibrate=None) -> str:
    """The simulation (or, with `calibrate`, the calibration) model."""
    common = dict(potential=potential, demand=demand, neighborhood=10, approach="gravity", gamma=1.5,
                  scaling_factor=1.0, num_neighbors=4, seed_search="probability", incentive_power=incentive)
    if calibrate is None:
        op = "futures.simulation"
        params = dict(common, patch_sizes=library, discount_factor=1.0, compactness_mean=compactness_mean,
                      compactness_range=compactness_range)
        extra = 'seed = 1'
        outs = """[outputs.maps]
from = "futures.maps"

[outputs.development]
from = "futures.development"

[outputs.info]
from = "futures.info"
"""
    else:
        op = "futures.calibrate"
        params = dict(common, **calibrate)
        extra = 'end = "dev_end.map"'
        outs = """[outputs.calibration]
from = "futures.table"
"""
    band_end = {"lu1985": "1991", "lu1991": "1999"}[start]
    text = f"""format = "terraflux/1"
name = "FUTURES on Plum Island"

[[nodes]]
id = "land"
op = "raster.read"
params = {{ path = "pie_landuse.tif", band = "{start[2:]}" }}

[[nodes]]
id = "end"
op = "raster.read"
params = {{ path = "pie_landuse.tif", band = "{band_end}" }}

[[nodes]]
id = "drivers"
op = "raster.read"
params = {{ path = "pie_drivers.tif" }}

[[nodes]]
id = "counties"
op = "raster.read"
params = {{ path = "subregions.tif" }}

[[nodes]]
id = "developed"
op = "raster.calc"
inputs = {{ lu = "land.raster" }}
params = {{ expression = "lu == 2" }}

[[nodes]]
id = "dev_end"
op = "raster.calc"
inputs = {{ lu = "end.raster" }}
params = {{ expression = "lu == 2" }}

[[nodes]]
id = "pressure"
op = "futures.devpressure"
inputs = {{ developed = "developed.map" }}
params = {{ size = 10, method = "gravity", gamma = 1.5, scaling_factor = 1.0 }}

[[nodes]]
id = "futures"
op = "{op}"
inputs = {{ landscape = "developed.map", pressure = "pressure.map", drivers = "drivers.raster", subregions = "counties.raster", {extra} }}
params = {inline(params)}
"""
    return text + "\n" + outs


def main():
    import rasterio

    img = figs.img_dir(SLUG)
    data = figs.data_dir(SLUG)
    work = figs.workdir(f"{SLUG}/fit")
    for f in ("pie_landuse.tif", "pie_drivers.tif"):
        shutil.copy(INPUTS / f, work / f)
    lu = {y: read(INPUTS / "pie_landuse.tif", b) for b, y in ((1, 1985), (2, 1991), (3, 1999))}
    # two subregions: the west and the east halves of the map (in the place of counties)
    sub = np.where(np.isnan(lu[1985]), np.nan, 1.0)
    sub[:, sub.shape[1] // 2:] = np.where(np.isnan(lu[1985][:, sub.shape[1] // 2:]), np.nan, 2.0)
    write_like(work / "subregions.tif", sub, INPUTS / "pie_drivers.tif")
    (work / "fit.toml").write_text(FIT, encoding="utf-8")
    figs.run(FIT, work, name="fit.toml")
    out = work / "output"
    west = json.loads((out / "west.json").read_text())["fits"][0]
    east = json.loads((out / "east.json").read_text())["fits"][0]
    library = json.loads((out / "library.json").read_text())
    print("west", west["coefficients"], "east", east["coefficients"])
    print("library", {k: (len(v) if isinstance(v, list) else v) for k, v in library.items()})
    fits = []
    for name, f in (("West", west), ("East", east)):
        fits.append({"subregion": name, **{d: round(c, 6) for d, c in zip(["intercept"] + f["drivers"],
                                                                           f["coefficients"])}})
    figs.write_csv(data / "potential.csv", fits)
    potential = {"ids": [1, 2], "intercept": [west["coefficients"][0], east["coefficients"][0]],
                 "devpressure": [west["coefficients"][1], east["coefficients"][1]],
                 "predictors": ["slope", "elevation"],
                 "coefficients": [west["coefficients"][2:], east["coefficients"][2:]]}
    # patch sizes
    sizes = [float(r["size"]) for r in figs.read_csv(out / "patches.csv")]
    hist = np.bincount(np.minimum(np.array(sizes, dtype=int), 30))
    figs.write_csv(data / "patch_sizes.csv", [{"size": (str(i) if i < 30 else "30+"), "patches": int(hist[i])}
                                              for i in range(1, len(hist))])
    print("patches", len(sizes), "mean", np.mean(sizes), "max", max(sizes))
    # the observed demand per subregion and year
    def dev_cells(a, s):
        return int(((a == 2) & (sub == s)).sum())

    rate = {s: (dev_cells(lu[1991], s) - dev_cells(lu[1985], s)) / 6 for s in (1, 2)}
    obs99 = {s: dev_cells(lu[1999], s) - dev_cells(lu[1991], s) for s in (1, 2)}
    print("yearly rate 1985-1991", rate, "observed 1991-1999", obs99)
    calib_demand = {"subregions": [1, 2], "rows": [[round(rate[1]), round(rate[2])]] * 6}
    demand = {"subregions": [1, 2], "rows": [[round(rate[1]), round(rate[2])]] * 8,
              "years": list(range(1992, 2000))}
    # maps of the pieces
    figs.raster(out / "pressure.tif", img / "pressure.png", cmap="magma", title="Development pressure, 1985",
                width=760)
    figs.raster(work / "subregions.tif", img / "subregions.png", colors={1: "#56b4e9", 2: "#e69f00"},
                labels={1: "West (1)", 2: "East (2)"}, title="Subregions", width=600)
    # calibration on 1985-1991
    cw = figs.workdir(f"{SLUG}/calibrate")
    for f in ("pie_landuse.tif", "pie_drivers.tif"):
        shutil.copy(INPUTS / f, cw / f)
    shutil.copy(work / "subregions.tif", cw / "subregions.tif")
    cal = simulation(potential, calib_demand, library, start="lu1985",
                     calibrate=dict(compactness_means=[0.1, 0.3, 0.5, 0.7, 0.9], compactness_ranges=[0.05, 0.3],
                                    discount_factors=[1.0], repeat=4, seed=1, per_subregion=True))
    figs.run(cal, cw)
    table = figs.read_csv(cw / "output" / "calibration.csv")
    print(table[:3])
    figs.write_csv(data / "calibration.csv", [{k: round(float(v), 4) for k, v in r.items()} for r in table])
    best = table[0]
    cm, cr = float(best["compactness_mean"]), float(best["compactness_range"])
    # simulation 1991-1999
    sw = figs.workdir(f"{SLUG}/simulate")
    for f in ("pie_landuse.tif", "pie_drivers.tif"):
        shutil.copy(INPUTS / f, sw / f)
    shutil.copy(work / "subregions.tif", sw / "subregions.tif")
    model = simulation(potential, demand, library, cm, cr)
    (data / "simulation.toml").write_text(model, encoding="utf-8")
    figs.run(model, sw)
    info = figs.read_csv(sw / "output" / "info.csv")
    print(info[:4])
    maps = sw / "output" / "maps.tif"
    n = figs.bands(maps)
    dev91 = np.where(np.isnan(lu[1991]), np.nan, (lu[1991] == 2).astype(float))
    figs.raster(None, img / "year_0.png", array=np.ma.masked_invalid(dev91[WIN]), palette=DEV, legend=False,
                width=700)
    yearly = [{"year": 1991, "developed": int(np.nansum(dev91))}]
    for b in range(1, n + 1):
        a = read(maps, b)
        figs.raster(None, img / f"year_{b}.png", array=np.ma.masked_invalid(new3(a, dev91)[WIN]), palette=DEV3,
                    legend=False, width=700)
        yearly.append({"year": 1991 + b, "developed": int(np.nansum(a == 1))})
    figs.write_csv(data / "yearly.csv", yearly)
    # the year each cell was developed
    with rasterio.open(sw / "output" / "development.tif") as d:
        devstep = d.read(d.count).astype(float)
    devstep[devstep < 0] = np.nan
    devstep[np.isnan(lu[1991])] = np.nan
    when = np.where(np.isnan(devstep), np.nan, 0.0)
    when[np.isnan(lu[1991])] = np.nan
    when[(devstep == 0)] = 1
    when[(devstep >= 1) & (devstep <= 4)] = 2
    when[(devstep >= 5)] = 3
    when[np.isnan(devstep) & ~np.isnan(lu[1991])] = 0
    figs.raster(None, img / "development.png", array=np.ma.masked_invalid(when[WIN]),
                colors={0: "#ece8de", 1: "#9a9a9a", 2: "#e69f00", 3: "#785ef0"},
                labels={0: "Not developed", 1: "Before 1992", 2: "1992-1995", 3: "1996-1999"},
                title="When each cell was developed", width=760)
    sim99 = read(maps, n)
    val = f"""format = "terraflux/1"
[[nodes]]
id = "lu"
op = "raster.read"
params = {{ path = "pie_landuse.tif", band = "1999" }}

[[nodes]]
id = "start"
op = "raster.read"
params = {{ path = "pie_landuse.tif", band = "1991" }}

[[nodes]]
id = "sim"
op = "raster.read"
params = {{ path = "output/maps.tif", band = "{n - 1}" }}

[[nodes]]
id = "observed"
op = "raster.calc"
inputs = {{ lu = "lu.raster" }}
params = {{ expression = "lu == 2" }}

[[nodes]]
id = "original"
op = "raster.calc"
inputs = {{ lu = "start.raster" }}
params = {{ expression = "lu == 2" }}

[[nodes]]
id = "validation"
op = "validate.futures"
inputs = {{ reference = "observed.map", simulated = "sim.raster", original = "original.map" }}

[outputs.metrics]
from = "validation.metrics"
"""
    figs.run(val, sw, name="validate.toml")
    print("validation", open(sw / "output" / "metrics.csv").read())
    shutil.copy(sw / "output" / "metrics.csv", data / "validation.csv")
    obs99m = np.where(np.isnan(lu[1999]), np.nan, (lu[1999] == 2).astype(float))
    figs.raster(None, img / "cmp_observed.png", array=np.ma.masked_invalid(new3(obs99m, dev91)[WIN]), palette=DEV3,
                legend=False, width=760)
    figs.raster(None, img / "cmp_simulated.png", array=np.ma.masked_invalid(new3(sim99, dev91)[WIN]), palette=DEV3,
                legend=False, width=760)
    figs.raster(None, img / "legend3.png", array=np.ma.masked_invalid(np.array([[0.0, 1.0, 2.0]])), palette=DEV3,
                width=320)
    # incentive power
    sens = []
    from scipy import ndimage

    for ip in (0.25, 0.5, 1.0, 2.0, 4.0):
        w = figs.workdir(f"{SLUG}/incentive_{ip}")
        for f in ("pie_landuse.tif", "pie_drivers.tif"):
            shutil.copy(INPUTS / f, w / f)
        shutil.copy(work / "subregions.tif", w / "subregions.tif")
        figs.run(simulation(potential, demand, library, cm, cr, incentive=ip), w)
        m = w / "output" / "maps.tif"
        a = read(m, figs.bands(m))
        new = (a == 1) & (dev91 == 0)
        figs.raster(None, img / f"incentive_{ip}.png", array=np.ma.masked_invalid(new3(a, dev91)[WIN]),
                    palette=DEV3, width=700, legend=False)
        lab, k = ndimage.label(a == 1, structure=np.ones((3, 3)))
        # mean pressure of the newly developed cells: how near to the old built land they are
        pr = read(work / "output" / "pressure.tif")
        sens.append({"incentive_power": ip, "developed_patches": int(k),
                     "new_cells": int(new.sum()),
                     "new_next_to_old": round(float((ndimage.binary_dilation(dev91 == 1, np.ones((3, 3))) & new).sum()
                                                    / max(1, new.sum()) * 100), 1)})
    print(sens)
    figs.write_csv(data / "incentive.csv", sens)


if __name__ == "__main__":
    main()
