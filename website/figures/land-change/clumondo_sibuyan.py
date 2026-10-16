"""Figures of the tutorial "CLUMondo: land systems and the services they supply" (land-change/clumondo).

Runs CLUMondo on Sibuyan Island (the land use of 1997, five classes taken as land systems) with Terraflux: the
logistic regressions of every system fitted on the map, a demand for food that grows 3 % a year and a demand
for wood that stays, ten years. Then the same model with other conversion resistances of the forest.

    python website/figures/land-change/clumondo_sibuyan.py
"""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "land-change/clumondo"
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
SYSTEMS = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#a7c957"), 3: ("Grassland", "#d9a441"),
           4: ("Rice", "#56b4e9"), 5: ("Other", "#7f7f7f")}
YEARS = 10
# what one cell of each system supplies: food, wood
SUPPLY = [[0.0, 1.0], [0.2, 0.3], [1.0, 0.0], [2.0, 0.0], [0.0, 0.0]]


def inline(v) -> str:
    """A value as TOML inline (dicts as inline tables)."""
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{k} = {inline(x)}" for k, x in v.items()) + " }"
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(inline(x) for x in v) + "]"
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, str):
        return json.dumps(v)
    return repr(v) if isinstance(v, float) else str(v)


def demand_rows(start_counts, food_growth=0.03, years=YEARS):
    food0 = sum(SUPPLY[i][0] * start_counts[i] for i in range(5))
    wood0 = sum(SUPPLY[i][1] * start_counts[i] for i in range(5))
    return [[round(food0 * (1 + food_growth) ** t, 2), round(wood0, 2)] for t in range(years + 1)]


def settings(rows, resistance):
    return {
        "systems": [1, 2, 3, 4, 5],
        "services": ["food", "wood"],
        "supply": [{"step": 1, "rows": SUPPLY}],
        "demand": rows,
        "order": [[0, 2], [1, 1], [2, 0], [3, 0], [-1, -1]],
        "weights": [1.0, 1.0],
        "resistance": resistance,
        "allowed": [[1, 1, 1, 1, 0], [1, 1, 1, 1, 0], [110, 1, 1, 1, 0], [110, 1, 1, 1, 0], [0, 0, 0, 0, 1]],
        "deviation": "percent",
        "mean_deviation": 0.5,
        "max_deviation": 1.0,
        "speed": 0.05,
        "max_loops": 20000,
    }


def model(rows, resistance) -> str:
    return f"""format = "terraflux/1"
name = "CLUMondo on Sibuyan"

[[nodes]]
id = "land"
op = "raster.read"
params = {{ path = "sibuyan_landuse.tif", band = "1997" }}

[[nodes]]
id = "drivers"
op = "raster.read"
params = {{ path = "sibuyan_drivers.tif" }}

[[nodes]]
id = "suitability"
op = "glm.fit"
inputs = {{ landscape = "land.raster", drivers = "drivers.raster" }}
params = {{ formulas = {{ 1 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 2 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 3 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 4 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"], 5 = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"] }} }}

[[nodes]]
id = "ages"
op = "clumondo.ages"
inputs = {{ landscape = "land.raster", seed = 1 }}
params = {{ max_age = 10 }}

[[nodes]]
id = "years"
op = "control.repeat"
inputs = {{ times = {YEARS}, landscape = "land.raster", age = "ages.age", elasticity = [0.0, 0.0] }}

[nodes.body]
carry = {{ landscape = "allocate.landscape", age = "allocate.age", elasticity = "allocate.elasticity" }}
collect = {{ maps = "allocate.landscape", info = "allocate.info" }}
until = "allocate.stop"

[[nodes.body.nodes]]
id = "allocate"
op = "clumondo.step"
inputs = {{ landscape = "@landscape", age = "@age", elasticity = "@elasticity", step = "@step", model = "suitability.model", drivers = "drivers.raster" }}
params = {{ settings = {inline(settings(rows, resistance))} }}

[outputs.maps]
from = "years.maps"

[outputs.steps]
from = "years.info"

[outputs.model]
from = "suitability.model"
"""


def counts_of(path):
    import rasterio

    with rasterio.open(path) as d:
        a = d.read(1)
    return [int((a == c).sum()) for c in range(1, 6)]


def run(name, rows, resistance):
    work = figs.workdir(f"{SLUG}/{name}")
    for f in ("sibuyan_landuse.tif", "sibuyan_drivers.tif"):
        shutil.copy(INPUTS / f, work / f)
    text = model(rows, resistance)
    figs.run(text, work)
    return work, text


def main():
    img = figs.img_dir(SLUG)
    data = figs.data_dir(SLUG)
    start = counts_of(INPUTS / "sibuyan_landuse.tif")
    rows = demand_rows(start)
    base_res = [0.8, 0.5, 0.3, 0.6, 1.0]
    work, text = run("base", rows, base_res)
    steps = figs.read_csv(work / "output" / "steps.csv")
    print(steps[0].keys())
    pal = {k: v for k, v in SYSTEMS.items()}
    # the start and each year
    figs.raster(work / "sibuyan_landuse.tif", img / "year_0.png", palette=pal, title="1997 (start)", width=760,
                counts=True)
    figs.raster(work / "sibuyan_landuse.tif", img / "frame_0.png", palette=pal, legend=False, width=700)
    maps = work / "output" / "maps.tif"
    n = figs.bands(maps)
    frames = []
    chart = [{"year": 0, "food_demand": 100.0, "food_supply": 100.0, "wood_demand": 100.0, "wood_supply": 100.0}]
    food0, wood0 = rows[0]
    for b in range(1, n + 1):
        figs.raster(maps, img / f"frame_{b}.png", band=b, palette=pal, legend=False, width=700)
        s = steps[b - 1]
        chart.append({"year": b, "food_demand": round(rows[b][0] / food0 * 100, 2),
                      "food_supply": round(float(s["supply_food"]) / food0 * 100, 2),
                      "wood_demand": round(rows[b][1] / wood0 * 100, 2),
                      "wood_supply": round(float(s["supply_wood"]) / wood0 * 100, 2)})
    figs.write_csv(data / "services.csv", chart)
    cells = [{"year": 0, **{SYSTEMS[c][0]: start[c - 1] for c in SYSTEMS}}]
    for b in range(1, n + 1):
        s = steps[b - 1]
        cells.append({"year": b, **{SYSTEMS[c][0]: int(float(s[f"cells_{c}"])) for c in SYSTEMS}})
    figs.write_csv(data / "cells.csv", cells)
    figs.write_csv(data / "steps.csv", [{k: (round(float(v), 3) if k not in ("step",) else v) for k, v in r.items()}
                                        for r in steps])
    # what changed in ten years
    import rasterio

    with rasterio.open(maps) as d:
        last = d.read(n).astype(float)
    with rasterio.open(INPUTS / "sibuyan_landuse.tif") as d:
        first = d.read(1).astype(float)
    first[first == 255] = np.nan
    change = np.full(first.shape, np.nan)
    ok = ~np.isnan(first) & ~np.isnan(last)
    change[ok] = 0
    trans = {}
    code = 1
    labels = {0: "No change"}
    colors = {0: "#ebe7dc"}
    tcol = ["#d55e00", "#e69f00", "#0072b2", "#cc79a7", "#009e73", "#785ef0", "#56b4e9", "#8c564b"]
    for f in range(1, 6):
        for t in range(1, 6):
            m = ok & (first == f) & (last == t) & (f != t)
            if m.sum() >= 5:
                trans[(f, t)] = int(m.sum())
                change[m] = code
                labels[code] = f"{SYSTEMS[f][0]} to {SYSTEMS[t][0]}"
                colors[code] = tcol[(code - 1) % len(tcol)]
                code += 1
    figs.raster(None, img / "change.png", array=np.ma.masked_invalid(change), colors=colors, labels=labels,
                title="Changes in ten years", width=760, counts=True)
    print("transitions", trans)
    # the suitability of rice and grassland from the regressions
    print(steps[-1])
    # sensitivity: the forest's conversion resistance
    sens = []
    for r in (0.0, 0.3, 0.6, 0.9):
        res = list(base_res)
        res[0] = r
        w, _ = run(f"res_{r}", rows, res)
        st = figs.read_csv(w / "output" / "steps.csv")
        m = w / "output" / "maps.tif"
        k = figs.bands(m)
        figs.raster(m, img / f"forest_res_{r}.png", band=k, palette=pal, legend=False, width=700)
        last = st[-1]
        sens.append({"resistance": r, "forest": int(float(last["cells_1"])), "grassland": int(float(last["cells_3"])),
                     "rice": int(float(last["cells_4"])), "coconut": int(float(last["cells_2"])),
                     "loops": int(float(last["loops"]))})
    print(sens)
    figs.write_csv(data / "resistance.csv", sens)


if __name__ == "__main__":
    main()
