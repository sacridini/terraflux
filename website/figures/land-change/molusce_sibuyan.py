"""Figures of the tutorial "MOLUSCE: weights of evidence and a Monte Carlo cellular automaton"
(land-change/molusce).

MOLUSCE on Sibuyan Island (1997 and 2011; 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other):

1. MOLUSCE's weights of evidence fitted with landschaft (MolusceWoE: the elevation, the slope and the distance to
   roads cut into categories), written as molusce.total's settings;
2. the cross table of 1997-2011 with change.crosstab (the cells of each transition, the "demand" of every
   iteration);
3. Terraflux runs the simulator from 2011 for two iterations (2025, 2039), with the weights and with a logistic
   regression's potential instead;
4. a check on the calibration period: one iteration from 1997 compared with 2011 (validate.three_map).

    python website/figures/land-change/molusce_sibuyan.py
"""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "land-change/molusce"
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
PAL = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Rice", "#4f9fc4"),
       5: ("Other", "#b4508a")}
BINS = {"elevation": [50, 150, 300, 600], "slope": [4, 10, 16, 22], "dist_roads": [250, 500, 1000, 2000, 4000]}


def inline(v) -> str:
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{k} = {inline(x)}" for k, x in v.items()) + " }"
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(inline(x) for x in v) + "]"
    if isinstance(v, str):
        return json.dumps(v)
    if isinstance(v, float):
        return repr(round(v, 6))
    return str(v)


def fit_woe():
    import landschaft as ls
    from landschaft.simulate._graph import _molusce_json

    S = ls.simulate
    lu = ls.load_raster(str(INPUTS / "sibuyan_landuse.tif"))
    ef = ls.load_raster(str(INPUTS / "sibuyan_drivers.tif"))
    t0, t1 = lu[0].where(lu[0] != 255), lu[1].where(lu[1] != 255)
    d = ef.sel(band=["elevation", "slope", "dist_roads"])
    woe = S.MolusceWoE(BINS).fit(t0, t1, d)
    js = _molusce_json(woe)
    for e in js["weights"]:
        e["categories"] = [float(x) for x in e["categories"]]
        e["weights"] = [round(float(x), 6) for x in e["weights"]]
    return js


CROSSTAB = """format = "terraflux/1"

[[nodes]]
id = "lu1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "lu2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "cross"
op = "change.crosstab"
inputs = { from = "lu1997.raster", to = "lu2011.raster" }

[outputs.cross]
from = "cross.table"
"""


def model(woe: dict, counts: list, start: str, iterations: int, potential: str = "woe", end: str | None = None):
    pot = ""
    if potential == "woe":
        pot = f"""[[nodes]]
id = "weights"
op = "molusce.total"
inputs = {{ drivers = "drivers.raster" }}
params = {{ classes = {inline(woe["classes"])}, drivers = {inline(woe["drivers"])}, bins = {inline(woe["bins"])}, weights = {inline(woe["weights"])} }}
"""
        forecast = 'total = "weights.total"'
    else:
        pot = """[[nodes]]
id = "regression"
op = "glm.fit"
inputs = { landscape = "start.raster", drivers = "drivers.raster" }
params = { formulas = { 1 = ["elevation", "slope", "dist_roads"], 2 = ["elevation", "slope", "dist_roads"], 3 = ["elevation", "slope", "dist_roads"], 4 = ["elevation", "slope", "dist_roads"], 5 = ["elevation", "slope", "dist_roads"] } }

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "drivers.raster", landscape = "start.raster" }
"""
        forecast = 'potential = "suitability.suitability"'
    text = f"""format = "terraflux/1"
name = "MOLUSCE on Sibuyan"

[[nodes]]
id = "start"
op = "raster.read"
params = {{ path = "sibuyan_landuse.tif", band = "{start}" }}

[[nodes]]
id = "drivers"
op = "raster.read"
params = {{ path = "sibuyan_drivers.tif" }}

{pot}
[[nodes]]
id = "rng"
op = "rng.numpy"
inputs = {{ seed = 1 }}

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = {{ times = {iterations}, landscape = "start.raster" }}

[nodes.body]
carry = {{ landscape = "allocate.landscape" }}
collect = {{ maps = "allocate.landscape", info = "allocate.info" }}

[[nodes.body.nodes]]
id = "allocate"
op = "molusce.step"
inputs = {{ landscape = "@landscape", {forecast}, rng = "rng.rng", step = "@step" }}
params = {{ classes = [1, 2, 3, 4, 5], counts = {inline(counts)} }}

[outputs.maps]
from = "simulation.maps"

[outputs.info]
from = "simulation.info"
"""
    if end:
        text += f"""
[[nodes]]
id = "observed"
op = "raster.read"
params = {{ path = "sibuyan_landuse.tif", band = "{end}" }}

[[nodes]]
id = "validation"
op = "validate.three_map"
inputs = {{ t0 = "start.raster", t1 = "observed.raster", simulated = "simulation.landscape" }}
params = {{ factors = [1, 2, 4, 8] }}

[outputs.fom]
from = "validation.figure_of_merit"
"""
    return text


def read(path, band=1):
    import rasterio

    with rasterio.open(path) as d:
        a = d.read(band).astype(float)
        if d.nodata is not None and not np.isnan(d.nodata):
            a[a == d.nodata] = np.nan
    return a


def main():
    img = figs.img_dir(SLUG)
    data = figs.data_dir(SLUG)
    work = figs.workdir(f"{SLUG}/fit")
    for f in ("sibuyan_landuse.tif", "sibuyan_drivers.tif"):
        shutil.copy(INPUTS / f, work / f)
    woe = fit_woe()
    (work / "woe.json").write_text(json.dumps(woe), encoding="utf-8")
    # the weights of forest -> grassland and forest -> forest along the elevation, the slope, the roads
    rows = []
    for e in woe["weights"]:
        if e["from"] == 1 and e["to"] in (1, 3):
            for c, w in zip(e["categories"], e["weights"]):
                rows.append({"transition": f'{PAL[e["from"]][0]} to {PAL[e["to"]][0]}', "driver": e["driver"],
                             "category": int(c), "weight": w})
    # a chart per driver: categories in rows, the two transitions as columns
    for drv, bounds in BINS.items():
        labels = [f"< {bounds[0]}"] + [f"{a}-{b}" for a, b in zip(bounds, bounds[1:])] + [f">= {bounds[-1]}"]
        out = []
        for k, lab in enumerate(labels, start=1):
            r = {"range": lab}
            for t in ("Forest to Forest", "Forest to Grassland"):
                v = [x["weight"] for x in rows if x["driver"] == drv and x["transition"] == t and x["category"] == k]
                r[t.replace(" ", "_")] = v[0] if v else ""
            out.append(r)
        figs.write_csv(data / f"weights_{drv}.csv", out)
        # for a chart: the ranges without a weight of about -33 (a transition that never happened there)
        figs.write_csv(data / f"weights_{drv}_chart.csv",
                       [r for r in out if all(v == "" or v > -30 for k, v in r.items() if k != "range")])
    # the cross table
    figs.run(CROSSTAB, work, name="cross.toml")
    cross = figs.read_csv(work / "output" / "cross.csv")
    print(cross)
    shutil.copy(work / "output" / "cross.csv", data / "cross.csv")
    counts = []
    for r in cross:
        f = int(float(r["from"]))
        for k, v in r.items():
            if k == "from":
                continue
            counts.append({"from": f, "to": int(float(k)), "cells": int(float(v))})
    # 1. calibration check: one iteration from 1997, against 2011
    res = {}
    for pot in ("woe", "glm"):
        w = figs.workdir(f"{SLUG}/check_{pot}")
        for f in ("sibuyan_landuse.tif", "sibuyan_drivers.tif"):
            shutil.copy(INPUTS / f, w / f)
        figs.run(model(woe, counts, "1997", 1, pot, end="2011"), w)
        fom = figs.read_csv(w / "output" / "fom.csv")
        res[pot] = {int(float(r["factor"])): round(float(r["overall"]), 4) for r in fom}
        sim = read(w / "output" / "maps.tif", 1)
        figs.raster(None, img / f"check_{pot}.png", array=np.ma.masked_invalid(sim), palette=PAL, legend=False,
                    width=700)
    print("figure of merit", res)
    figs.write_csv(data / "fom.csv", [{"factor": k, "woe": res["woe"][k], "glm": res["glm"][k]} for k in res["woe"]])
    obs = read(INPUTS / "sibuyan_landuse.tif", 2)
    figs.raster(None, img / "observed_2011.png", array=np.ma.masked_invalid(obs), palette=PAL, legend=False,
                width=700)
    figs.raster(None, img / "observed_2011_legend.png", array=np.ma.masked_invalid(obs), palette=PAL,
                title="Observed 2011", width=760, counts=True)
    figs.raster(None, img / "observed_1997.png", array=np.ma.masked_invalid(read(INPUTS / "sibuyan_landuse.tif", 1)),
                palette=PAL, title="Observed 1997", width=760, counts=True)
    # 2. projection from 2011: two iterations (2025, 2039)
    for pot in ("woe", "glm"):
        w = figs.workdir(f"{SLUG}/project_{pot}")
        for f in ("sibuyan_landuse.tif", "sibuyan_drivers.tif"):
            shutil.copy(INPUTS / f, w / f)
        text = model(woe, counts, "2011", 2, pot)
        if pot == "woe":
            (data / "model.toml").write_text(text, encoding="utf-8")
        figs.run(text, w)
        m = w / "output" / "maps.tif"
        for b, y in ((1, 2025), (2, 2039)):
            figs.raster(m, img / f"{pot}_{y}.png", band=b, palette=PAL, title=f"{y} ({'weights of evidence' if pot == 'woe' else 'logistic regression'})",
                        width=760, counts=True)
        info = figs.read_csv(w / "output" / "info.csv")
        print(pot, info[:3])
    # the total weights of forest -> forest and forest -> grassland
    w = figs.workdir(f"{SLUG}/totals")
    for f in ("sibuyan_landuse.tif", "sibuyan_drivers.tif"):
        shutil.copy(INPUTS / f, w / f)
    tot = f"""format = "terraflux/1"

[[nodes]]
id = "drivers"
op = "raster.read"
params = {{ path = "sibuyan_drivers.tif" }}

[[nodes]]
id = "weights"
op = "molusce.total"
inputs = {{ drivers = "drivers.raster" }}
params = {{ classes = {inline(woe["classes"])}, drivers = {inline(woe["drivers"])}, bins = {inline(woe["bins"])}, weights = {inline(woe["weights"])} }}

[[nodes]]
id = "write"
op = "raster.write"
inputs = {{ value = "weights.total" }}
params = {{ path = "total.tif" }}
"""
    figs.run(tot, w)
    import rasterio

    with rasterio.open(w / "total.tif") as d:
        names = list(d.descriptions)
        print(names)
        for want, title in (("1_to_1", "forest stays forest"), ("1_to_3", "forest to grassland")):
            b = names.index(want) + 1
            figs.raster(w / "total.tif", img / f"total_{want}.png", band=b, diverging=True, vmin=-5, vmax=5,
                        unit="total weight (clipped at -5 and 5)", title=f"Total weight: {title}", width=700)


if __name__ == "__main__":
    main()
