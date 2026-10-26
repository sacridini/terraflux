"""Figures of the tutorial "Dynamic drivers" (land-change/dynamic-drivers).

CLUE-S on lulcc's Plum Island Ecosystems data (1985, 1991, 1999), twice: with the distance to built land of 1985
as a static driver, and with that distance computed again from the simulated map at every step
(dynamic.distance_to put back into the drivers under the same name with stack.set). Also the share of built land
around each cell (dynamic.share_of) and the distance over a cost (a transition of conductances).

    python website/figures/land-change/dynamic_drivers.py
"""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "land-change/dynamic-drivers"
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
PIE = {1: ("Forest", "#3b8a5a"), 2: ("Built", "#b4508a"), 3: ("Other", "#d9a441")}
CELL = 99.92  # the side of Pie's cells, in metres (they are not exactly square)

HEAD = """format = "terraflux/1"

[inputs.observed]
kind = "stack"
path = "pie_landuse.tif"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.mid]
kind = "map"
path = "pie_landuse.tif"
band = "1991"

[inputs.end]
kind = "map"
path = "pie_landuse.tif"
band = "1999"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

[inputs.seed]
kind = "value"
value = SEED

[[nodes]]
id = "near_built_0"
op = "dynamic.distance_to"
inputs = { landscape = "$start" }
params = { classes = 2, unit = "cells" }

[[nodes]]
id = "in_metres_0"
op = "raster.calc"
inputs = { d = "near_built_0.distance" }
params = { expression = "d * 99.92" }

[[nodes]]
id = "drivers_0"
op = "stack.set"
inputs = { stack = "$drivers", map = "in_metres_0.map" }
params = { name = "dist_built_1985" }

[[nodes]]
id = "expansion"
op = "raster.calc"
inputs = { t0 = "$start", t1 = "$mid" }
params = { expression = "None if t0 == 2 else t1" }

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = { landscape = "expansion.map", drivers = "drivers_0.stack" }
params = { formulas = { 1 = ["elevation", "slope"], 2 = ["elevation", "slope", "dist_built_1985"], 3 = ["elevation", "slope"] } }

[[nodes]]
id = "demand"
op = "demand.from_maps"
inputs = { maps = "$observed" }
params = { times = [0, 6, 14] }

[[nodes]]
id = "rng"
op = "rng.r"
inputs = { seed = "$seed" }

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = { landscape = "$start" }
"""

ALLOCATE = """
[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = { landscape = "@landscape", history = "@history", suitability = "SUIT", demand = "demand.demand", step = "@step", rng = "rng.rng" }
params = { allocation = { method = "clue", jitter = 0.0002, scale = 0.000001, max_iter = 1000, max_diff = 50, mean_diff = 50 }, elasticity = [0.2, 0.2, 0.2] }
"""

TAIL = """
[[nodes]]
id = "check"
op = "validate.three_map"
inputs = { t0 = "$start", t1 = "$end", simulated = "simulation.landscape" }
params = { factors = [1, 2, 4, 8, 16] }

[outputs.steps]
from = "simulation.info"

[outputs.glm]
from = "regression.model"

[outputs.fom]
from = "check.figure_of_merit"
"""

STATIC = HEAD + """
[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "drivers_0.stack", landscape = "$start" }

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = "demand.steps", landscape = "$start", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape", info = "allocate.info" }
""" + ALLOCATE.replace("SUIT", "suitability.suitability") + TAIL

DYNAMIC = HEAD + """
[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = { times = "demand.steps", landscape = "$start", history = "history.history" }

[nodes.body]
carry = { landscape = "allocate.landscape", history = "allocate.history" }
collect = { maps = "allocate.landscape", info = "allocate.info", distance = "in_metres.map" }

[[nodes.body.nodes]]
id = "near_built"
op = "dynamic.distance_to"
inputs = { landscape = "@landscape" }
params = { classes = 2, unit = "cells" }

[[nodes.body.nodes]]
id = "in_metres"
op = "raster.calc"
inputs = { d = "near_built.distance" }
params = { expression = "d * 99.92" }

[[nodes.body.nodes]]
id = "drivers_now"
op = "stack.set"
inputs = { stack = "$drivers", map = "in_metres.map" }
params = { name = "dist_built_1985" }

[[nodes.body.nodes]]
id = "suitability"
op = "glm.predict"
inputs = { model = "regression.model", drivers = "drivers_now.stack", landscape = "@landscape" }
""" + ALLOCATE.replace("SUIT", "suitability.suitability") + TAIL

# the share of built land around each cell, and the distance over a cost, on the map of 1985
EXTRAS = """format = "terraflux/1"

[inputs.start]
kind = "map"
path = "pie_landuse.tif"
band = "1985"

[inputs.drivers]
kind = "stack"
path = "pie_drivers.tif"

[[nodes]]
id = "near"
op = "dynamic.distance_to"
inputs = { landscape = "$start" }
params = { classes = 2, unit = "cells" }

[[nodes]]
id = "share3"
op = "dynamic.share_of"
inputs = { landscape = "$start" }
params = { classes = 2, window = 3 }

[[nodes]]
id = "share9"
op = "dynamic.share_of"
inputs = { landscape = "$start" }
params = { classes = 2, window = 9 }

[[nodes]]
id = "ease"
op = "raster.calc"
inputs = { d = "$drivers" }
params = { expression = "1 / (1 + d.slope)" }

[[nodes]]
id = "conductance"
op = "cost.transition"
inputs = { map = "ease.map" }
params = { directions = 8, symmetric = true }

[[nodes]]
id = "cost_near"
op = "dynamic.distance_to"
inputs = { landscape = "$start", cost = "conductance.transition" }
params = { classes = 2, layout = { directions = 8, symmetric = true, extent = [213729.92125984, 263390.78740157, 911169.90970655, 954550.31602709] } }
"""


def prepare(name):
    work = figs.workdir(f"{SLUG}/{name}")
    for f in ("pie_landuse.tif", "pie_drivers.tif"):
        shutil.copy(INPUTS / f, work / f)
    return work


def main():
    out = figs.img_dir(SLUG)
    data = figs.data_dir(SLUG)
    runs = {}
    for name, model in (("static", STATIC), ("dynamic", DYNAMIC)):
        work = prepare(name)
        writes = {"simulation.maps": "maps.tif"}
        if name == "dynamic":
            writes["simulation.distance"] = "distance.tif"
        figs.run(figs.add_writes(model.replace("SEED", "1"), writes), work)
        runs[name] = work
        print(name, "done")

    start = figs.read(INPUTS / "pie_landuse.tif", 1)[0]
    end = figs.read(INPUTS / "pie_landuse.tif", 3)[0]
    start = np.ma.masked_where(start == 255, start)
    end = np.ma.masked_where(end == 255, end)
    built0 = start == 2

    # where the new built land went: 1985 built (grey), new built in the run (magenta)
    new_pal = {0: ("Not built", "#eef1ec"), 1: ("Built in 1985", "#8a8f8b"), 2: ("Built since 1985", "#d0217a")}

    def new_map(final):
        a = np.where(built0, 1, np.where(final == 2, 2, 0)).astype(float)
        return np.ma.masked_where(np.ma.getmaskarray(start), a)

    finals = {n: figs.read(w / "maps.tif", figs.bands(w / "maps.tif"))[0] for n, w in runs.items()}
    figs.raster(None, out / "new_static.png", array=new_map(finals["static"]), palette=new_pal, legend=False,
                width=800)
    figs.raster(None, out / "new_dynamic.png", array=new_map(finals["dynamic"]), palette=new_pal, legend=False,
                width=800)
    figs.raster(None, out / "new_legend.png", array=new_map(end), palette=new_pal, width=800)

    # the distance driver at the first and the last step of the dynamic run, and how much it moved
    dist = runs["dynamic"] / "distance.tif"
    n = figs.bands(dist)
    d1 = figs.read(dist, 1)[0]
    dn = figs.read(dist, n)[0]
    figs.raster(None, out / "distance_step1.png", array=d1, cmap="magma_r", vmin=0, vmax=2000, unit="metres",
                title="Distance to built land, step 1 (1985)", width=800)
    figs.raster(None, out / "distance_step14.png", array=dn, cmap="magma_r", vmin=0, vmax=2000, unit="metres",
                title="Distance to built land, step 14", width=800)
    figs.raster(None, out / "distance_change.png", array=dn - d1, cmap="Blues_r", vmax=0, unit="metres",
                title="How much nearer built land came", width=800)

    # the static driver vs the dynamic one at step 1: they should be the same map (a check)
    lulcc_d = figs.read(INPUTS / "pie_drivers.tif", 3)[0]
    gap = np.ma.masked_invalid(np.abs(lulcc_d - d1)).compressed()
    print("gap between lulcc's dist_built_1985 and the distance to built land of the 1985 map (m), "
          "50/90/99 percentiles and largest:", np.percentile(gap, [50, 90, 99]).round(1), float(gap.max()))
    static_d = np.ma.filled(d1, np.nan)  # the distance to the built land of 1985, as both models were fitted on

    # a slider over the steps of the dynamic run, the new built land of each step in red
    rows = []
    frames_static = runs["static"] / "maps.tif"
    for b in range(1, n + 1):
        cur = figs.read(runs["dynamic"] / "maps.tif", b)[0]
        prev = start if b == 1 else figs.read(runs["dynamic"] / "maps.tif", b - 1)[0]
        a = np.where(built0, 1, np.where(cur == 2, np.where(prev == 2, 2, 3), 0)).astype(float)
        a = np.ma.masked_where(np.ma.getmaskarray(start), a)
        pal = {0: ("Not built", "#eef1ec"), 1: ("Built in 1985", "#8a8f8b"), 2: ("Built in earlier steps", "#e58fbd"),
               3: ("Built at this step", "#a3004f")}
        figs.raster(None, out / f"step_{b}.png", array=a, palette=pal, legend=False, width=760)
        db = figs.read(dist, b)[0]
        cs = figs.read(frames_static, b)[0]
        new_dyn = (cur == 2) & ~built0
        new_sta = (cs == 2) & ~built0
        rows.append({
            "step": b,
            "year": 1985 + b,
            "dynamic": round(float(np.ma.mean(db)), 1),
            "static": round(float(np.ma.mean(np.ma.masked_invalid(static_d))), 1),
            "far_dynamic": int(((static_d > 500) & new_dyn).sum()),
            "far_static": int(((static_d > 500) & new_sta).sum()),
        })
    figs.write_csv(data / "steps.csv", rows)

    # where new built land went, by its distance to the built land of 1985
    bins = [0, 100, 200, 300, 400, 500, 750, 1000, 1500, 2100]
    hist_rows = []
    groups = {"observed": end, "static": finals["static"], "dynamic": finals["dynamic"]}
    counts = {}
    for g, m in groups.items():
        new = np.ma.filled((m == 2) & ~built0, False)
        dvals = static_d[new]
        counts[g] = np.histogram(dvals, bins=bins)[0]
    for i in range(len(bins) - 1):
        r = {"distance": f"{bins[i]}-{bins[i + 1]}"}
        for g in groups:
            r[g] = round(float(counts[g][i]) / max(counts[g].sum(), 1) * 100, 2)
        hist_rows.append(r)
    figs.write_csv(data / "by_distance.csv", hist_rows)

    # figures of merit
    fom_rows = []
    for name, work in runs.items():
        t = figs.read_csv(work / "output" / "fom.csv")
        for r in t:
            fom_rows.append({"model": name, **r})
    figs.write_csv(data / "fom.csv", fom_rows)
    for r in fom_rows:
        print(r)

    # the share of built land and the distance over a cost, on the map of 1985
    work = prepare("extras")
    figs.run(figs.add_writes(EXTRAS, {"near.distance": "near.tif", "share3.share": "share3.tif",
                                      "share9.share": "share9.tif", "cost_near.distance": "cost.tif"}), work)
    figs.raster(work / "share3.tif", out / "share3.png", cmap="viridis", vmin=0, vmax=1, unit="share",
                title="Share of built land, 3 x 3 cells", width=700)
    figs.raster(work / "share9.tif", out / "share9.png", cmap="viridis", vmin=0, vmax=1, unit="share",
                title="Share of built land, 9 x 9 cells", width=700)
    near = figs.read(work / "near.tif")[0] * CELL
    figs.raster(None, out / "near_euclid.png", array=near, cmap="magma_r", vmin=0, vmax=2000, unit="metres",
                title="Straight-line distance", width=700)
    cost = np.ma.masked_invalid(figs.read(work / "cost.tif")[0])
    cost = np.ma.masked_where(~np.isfinite(cost.filled(np.nan)), cost)
    figs.raster(None, out / "near_cost.png", array=cost, cmap="magma_r", vmin=0,
                vmax=float(np.percentile(cost.compressed(), 98)), unit="accumulated cost",
                title="Distance over the slope (cost)", width=700)


if __name__ == "__main__":
    main()
