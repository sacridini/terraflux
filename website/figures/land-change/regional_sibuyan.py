"""Figures of the tutorial "A demand per region" (land-change/regional-demand).

Sibuyan Island (1997 and 2011; 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other) cut into three sectors around
its centre (in the place of its three municipalities). CLUE-S (lulcc.step) simulates 1997-2011 in 14 yearly
steps three times:

- with one demand for the island (the linear trend of the island's areas);
- with a demand per sector (demand.regional: the trend of each sector's areas);
- with a scenario: the northern sector keeps its forest.

Then CARS (PLUS) with a Markov chain per sector (demand.regional of chains, plus.demand).

    python website/figures/land-change/regional_sibuyan.py
"""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "land-change/regional-demand"
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
PAL = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Rice", "#4f9fc4"),
       5: ("Other", "#b4508a")}
REGIONS = {1: ("North", "#0072b2"), 2: ("Southeast", "#e69f00"), 3: ("Southwest", "#cc79a7")}
STEPS = 14
DRIVERS = ["dist_sea", "elevation", "slope", "dist_roads", "dist_urban"]


def inline(v) -> str:
    if isinstance(v, dict):
        return "{ " + ", ".join(f"{k} = {inline(x)}" for k, x in v.items()) + " }"
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(inline(x) for x in v) + "]"
    if isinstance(v, str):
        return json.dumps(v)
    if isinstance(v, float):
        return repr(round(v, 8))
    return str(v)


def read(path, band=1):
    import rasterio

    with rasterio.open(path) as d:
        a = d.read(band).astype(float)
        if d.nodata is not None and not np.isnan(d.nodata):
            a[a == d.nodata] = np.nan
    return a


def linear_rows(c0, c1, steps=STEPS):
    """The cells of each class at each step, linear from c0 to c1, whole cells adding up to the total (as
    demand.from_maps rounds them)."""
    c0, c1 = np.array(c0, float), np.array(c1, float)
    total = c0.sum()
    rows = []
    for t in range(steps + 1):
        v = c0 + (c1 - c0) * t / steps
        r = np.floor(v)
        rest = int(total - r.sum())
        order = np.argsort(-(v - r))
        r[order[:rest]] += 1
        rows.append([float(x) for x in r])
    return rows


def model(demand_node: str, demand_ref: str) -> str:
    formulas = {c: DRIVERS for c in range(1, 6)}
    return f"""format = "terraflux/1"
name = "CLUE-S with a demand per region"

[[nodes]]
id = "lu1997"
op = "raster.read"
params = {{ path = "sibuyan_landuse.tif", band = "1997" }}

[[nodes]]
id = "drivers"
op = "raster.read"
params = {{ path = "sibuyan_drivers.tif" }}

[[nodes]]
id = "regions"
op = "raster.read"
params = {{ path = "regions.tif" }}

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = {{ landscape = "lu1997.raster", drivers = "drivers.raster" }}
params = {{ formulas = {inline(formulas)} }}

[[nodes]]
id = "suitability"
op = "glm.predict"
inputs = {{ model = "regression.model", drivers = "drivers.raster", landscape = "lu1997.raster" }}

{demand_node}
[[nodes]]
id = "rng"
op = "rng.r"
inputs = {{ seed = 1 }}

[[nodes]]
id = "history"
op = "lulcc.history"
inputs = {{ landscape = "lu1997.raster" }}

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = {{ times = {STEPS}, landscape = "lu1997.raster", history = "history.history" }}

[nodes.body]
carry = {{ landscape = "allocate.landscape", history = "allocate.history" }}
collect = {{ maps = "allocate.landscape", info = "allocate.info" }}

[[nodes.body.nodes]]
id = "allocate"
op = "lulcc.step"
inputs = {{ landscape = "@landscape", history = "@history", suitability = "suitability.suitability", {demand_ref}, step = "@step", rng = "rng.rng" }}
params = {{ allocation = {{ method = "clue", jitter = 0.0001, scale = 0.0005, max_iter = 1000, max_diff = 20, mean_diff = 10 }}, elasticity = [0.3, 0.3, 0.3, 0.3, 0.3] }}

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
"""


def cars_model(chains: list) -> str:
    formulas = {c: DRIVERS for c in range(1, 6)}
    demands = [{"markov": ch} for ch in chains]
    return f"""format = "terraflux/1"
name = "CARS with a Markov chain per region"

[[nodes]]
id = "lu1997"
op = "raster.read"
params = {{ path = "sibuyan_landuse.tif", band = "1997" }}

[[nodes]]
id = "drivers"
op = "raster.read"
params = {{ path = "sibuyan_drivers.tif" }}

[[nodes]]
id = "regions"
op = "raster.read"
params = {{ path = "regions.tif" }}

[[nodes]]
id = "regression"
op = "glm.fit"
inputs = {{ landscape = "lu1997.raster", drivers = "drivers.raster" }}
params = {{ formulas = {inline(formulas)} }}

[[nodes]]
id = "probability"
op = "glm.predict"
inputs = {{ model = "regression.model", drivers = "drivers.raster", landscape = "lu1997.raster" }}

[[nodes]]
id = "chains"
op = "demand.regional"
inputs = {{ regions = "regions.raster" }}
params = {{ ids = [1, 2, 3], demands = {inline(demands)} }}

[[nodes]]
id = "demand"
op = "plus.demand"
inputs = {{ start = "lu1997.raster", regional = "chains.regional" }}
params = {{ steps = {STEPS} }}

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = {{ times = "demand.steps", landscape = "lu1997.raster" }}

[nodes.body]
carry = {{ landscape = "cars.landscape" }}
collect = {{ maps = "cars.landscape", info = "cars.info" }}

[[nodes.body.nodes]]
id = "cars"
op = "cars.step"
inputs = {{ landscape = "@landscape", probability = "probability.suitability", regional = "demand.regional", step = "@step", seed = 1 }}
params = {{ seeds = 0.1, decay = 0.9, step_size = 20, window = 3, max_iter = 500 }}

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"
"""


def main():
    img = figs.img_dir(SLUG)
    data = figs.data_dir(SLUG)
    a97 = read(INPUTS / "sibuyan_landuse.tif", 1)
    a11 = read(INPUTS / "sibuyan_landuse.tif", 2)
    ok = ~np.isnan(a97)
    yy, xx = np.indices(a97.shape)
    cy, cx = yy[ok].mean(), xx[ok].mean()
    ang = np.degrees(np.arctan2(-(yy - cy), xx - cx)) % 360  # 0 east, 90 north
    reg = np.where(ang < 30, 2, np.where(ang < 150, 1, np.where(ang < 270, 3, 2))).astype(float)
    reg[~ok] = np.nan
    import rasterio

    with rasterio.open(INPUTS / "sibuyan_drivers.tif") as src:
        prof = src.profile
    prof.update(count=1, dtype="float32", nodata=float("nan"))

    def setup(name):
        w = figs.workdir(f"{SLUG}/{name}")
        for f in ("sibuyan_landuse.tif", "sibuyan_drivers.tif"):
            shutil.copy(INPUTS / f, w / f)
        with rasterio.open(w / "regions.tif", "w", **prof) as dst:
            dst.write(reg.astype("float32"), 1)
        return w

    w0 = setup("regions")
    figs.raster(w0 / "regions.tif", img / "regions.png", colors={k: v[1] for k, v in REGIONS.items()},
                labels={k: f"{v[0]} ({k})" for k, v in REGIONS.items()}, title="Three sectors", width=700)
    figs.raster(INPUTS / "sibuyan_landuse.tif", img / "observed_2011.png", band=2, palette=PAL, legend=False,
                width=700)
    counts = {}
    for r in REGIONS:
        counts[r] = ([int(((a97 == c) & (reg == r)).sum()) for c in range(1, 6)],
                     [int(((a11 == c) & (reg == r)).sum()) for c in range(1, 6)])
    island = ([int((a97 == c).sum()) for c in range(1, 6)], [int((a11 == c).sum()) for c in range(1, 6)])
    print(counts, island)
    obs_rows = []
    for r in REGIONS:
        c0, c1 = counts[r]
        obs_rows.append({"region": REGIONS[r][0], **{PAL[c][0]: c1[c - 1] - c0[c - 1] for c in PAL}})
    times = [float(t) for t in range(STEPS + 1)]
    # one demand for the island
    one = {"classes": [1, 2, 3, 4, 5], "rows": linear_rows(*island), "times": times}
    node_one = f"""[[nodes]]
id = "demand"
op = "demand.table"
params = {{ table = {inline(one)} }}
"""
    # a demand per region
    per = [{"table": {"classes": [1, 2, 3, 4, 5], "rows": linear_rows(*counts[r]), "times": times}} for r in REGIONS]
    node_reg = f"""[[nodes]]
id = "demand"
op = "demand.regional"
inputs = {{ regions = "regions.raster" }}
params = {{ ids = [1, 2, 3], demands = {inline(per)} }}
"""
    # the scenario: the north keeps its forest (its forest's loss given to no class: its other classes keep
    # their 1997 cells but rice and coconut grow by what the trend gave them, taken from grassland)
    c0, c1 = counts[1]
    keep = list(c1)
    lost = c0[0] - c1[0]
    keep[0] = c0[0]
    keep[2] = c1[2] - lost  # grassland gives up what forest would have
    if keep[2] < 0:
        keep[1] += keep[2]
        keep[2] = 0
    per_s = [{"table": {"classes": [1, 2, 3, 4, 5], "rows": linear_rows(c0, keep), "times": times}}] + per[1:]
    node_s = node_reg.replace(inline(per), inline(per_s))
    results = {}
    for name, node, ref in (("island", node_one, 'demand = "demand.demand"'),
                            ("regional", node_reg, 'regional = "demand.regional"'),
                            ("scenario", node_s, 'regional = "demand.regional"')):
        w = setup(name)
        text = model(node, ref)
        if name == "regional":
            (data / "model.toml").write_text(text, encoding="utf-8")
        r = figs.run(text, w)
        last = read(w / "output" / "maps.tif", STEPS)
        results[name] = last
        figs.raster(None, img / f"{name}_2011.png", array=np.ma.masked_invalid(last), palette=PAL, legend=False,
                    width=700)
        st = figs.read_csv(w / "output" / "steps.csv")
        print(name, st[-1])
    figs.raster(INPUTS / "sibuyan_landuse.tif", img / "observed_1997.png", band=1, palette=PAL,
                title="Observed 1997", width=760, counts=True)
    # the forest lost and the coconut gained in each sector: observed, one demand, a demand per region
    rows = []
    for r in REGIONS:
        row = {"region": REGIONS[r][0]}
        for name, m in (("observed", a11), ("island", results["island"]), ("regional", results["regional"]),
                        ("scenario", results["scenario"])):
            row[f"forest_{name}"] = int(((m == 1) & (reg == r)).sum()) - counts[r][0][0]
        rows.append(row)
    print(rows)
    figs.write_csv(data / "forest_change.csv", rows)
    # CARS with a Markov chain per region
    chains = []
    for r in REGIONS:
        m = np.zeros((5, 5))
        for i in range(5):
            for j in range(5):
                m[i, j] = ((a97 == i + 1) & (a11 == j + 1) & (reg == r)).sum()
        single = (m / np.maximum(m.sum(axis=1, keepdims=True), 1)).tolist()
        for i in range(5):
            if m[i].sum() == 0:
                single[i] = [1.0 if j == i else 0.0 for j in range(5)]
        chains.append({"classes": [1, 2, 3, 4, 5], "single": [[round(x, 6) for x in row] for row in single],
                       "steps": STEPS})
    w = setup("cars")
    text = cars_model(chains)
    (data / "cars.toml").write_text(text, encoding="utf-8")
    figs.run(text, w)
    last = read(w / "output" / "maps.tif", figs.bands(w / "output" / "maps.tif"))
    figs.raster(None, img / "cars_2011.png", array=np.ma.masked_invalid(last), palette=PAL,
                title="CARS, a Markov chain per sector: 2011", width=760, counts=True)
    print("cars", figs.read_csv(w / "output" / "steps.csv")[-1])
    crow = []
    for r in REGIONS:
        crow.append({"region": REGIONS[r][0], "forest_observed": int(((a11 == 1) & (reg == r)).sum()) - counts[r][0][0],
                     "forest_cars": int(((last == 1) & (reg == r)).sum()) - counts[r][0][0]})
    print(crow)
    figs.write_csv(data / "cars_forest.csv", crow)


if __name__ == "__main__":
    main()
