"""Figures of the tutorial "Stacks and layers" (building-models/stacks-and-layers).

Sibuyan's drivers as a stack, a stack made with stack.of, a layer replaced with stack.set, a folder read as a
stack, the bands of a time series, and suitabilities read from a file and declared classes with stack.classes
(CLUE-S run twice, the same maps). All from real runs of Terraflux.
"""

import shutil
import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import rasterio  # noqa: E402

NAME = "building-models/stacks-and-layers"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
data = figs.data_dir(NAME)
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
shutil.copy(INPUTS / "sibuyan_landuse.tif", work / "landuse.tif")
shutil.copy(INPUTS / "sibuyan_drivers.tif", work / "drivers.tif")
(work / "landuse").mkdir()
with rasterio.open(work / "landuse.tif") as ds:
    prof = ds.profile
    prof.update(count=1)
    for b, year in ((1, "1997"), (2, "2011")):
        with rasterio.open(work / "landuse" / f"landuse_{year}.tif", "w", **prof) as o:
            o.write(ds.read(b), 1)

SIBUYAN = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#d9a441"), 3: ("Grassland", "#a7c957"), 4: ("Rice", "#e6d36a"),
           5: ("Other", "#b4508a")}


def contact_sheet(path, png, cols, title_of=None, cmap="viridis", width=1000, same_scale=False):
    """Every band of a raster as a small map with its name."""
    with rasterio.open(path) as ds:
        arrs = [np.ma.masked_invalid(ds.read(b + 1, masked=True).astype(float)) for b in range(ds.count)]
        names = [d or str(i + 1) for i, d in enumerate(ds.descriptions)]
    rows = int(np.ceil(len(arrs) / cols))
    h, w = arrs[0].shape
    fig, axes = plt.subplots(rows, cols, figsize=(width / 150, width / 150 / cols * rows * h / w * 1.25 + 0.25), dpi=150)
    lo = min(float(a.min()) for a in arrs) if same_scale else None
    hi = max(float(a.max()) for a in arrs) if same_scale else None
    for k, ax in enumerate(np.atleast_1d(axes).ravel()):
        ax.axis("off")
        if k >= len(arrs):
            continue
        cm = plt.get_cmap(cmap).copy()
        cm.set_bad((1, 1, 1, 0))
        ax.imshow(arrs[k], cmap=cm, interpolation="nearest", vmin=lo, vmax=hi)
        ax.set_title(title_of(names[k]) if title_of else names[k], fontsize=9.5, family="monospace", loc="left")
    fig.tight_layout(pad=0.4)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)
    return names


# ------------------------------------------------------------------------------------------------ the drivers
names = contact_sheet(work / "drivers.tif", out / "drivers_sheet.png", cols=4, title_of=lambda n: f"d.{n}")
print("drivers:", names)

# ------------------------------------------------------------------------------------------------ the model
MODEL = """format = "terraflux/1"

[inputs.t0]
kind = "map"
path = "landuse.tif"
band = "1997"

[inputs.t1]
kind = "map"
path = "landuse.tif"
band = "2011"

[inputs.drivers]
kind = "stack"
path = "drivers.tif"

[[nodes]]
id = "slope"
op = "map.calc"
inputs = { d = "$drivers" }
params = { expression = "d.slope" }

[[nodes]]
id = "lowland"
op = "map.calc"
inputs = { d = "$drivers" }
params = { expression = "d.elevation < 300" }

[[nodes]]
id = "forest"
op = "map.calc"
inputs = { land = "$t0" }
params = { expression = "land == 1" }

[[nodes]]
id = "habitat"
op = "stack.of"
inputs = { slope = "slope.map", lowland = "lowland.map", forest = "forest.map" }

[[nodes]]
id = "farm_1997"
op = "dynamic.distance_to"
inputs = { landscape = "$t0" }
params = { classes = [2, 4] }

[[nodes]]
id = "farm_2011"
op = "dynamic.distance_to"
inputs = { landscape = "$t1" }
params = { classes = [2, 4] }

[[nodes]]
id = "with_farms"
op = "stack.set"
inputs = { stack = "$drivers", map = "farm_1997.distance" }
params = { name = "dist_farm" }

[[nodes]]
id = "updated"
op = "stack.set"
inputs = { stack = "with_farms.stack", map = "farm_2011.distance" }
params = { name = "dist_farm" }

[[nodes]]
id = "gained"
op = "map.calc"
inputs = { before = "with_farms.stack", after = "updated.stack" }
params = { expression = "after.dist_farm - before.dist_farm" }

[[nodes]]
id = "series"
op = "raster.read"
params = { path = "landuse" }

[[nodes]]
id = "change"
op = "map.calc"
inputs = { t0 = "$t0", t1 = "$t1" }
params = { expression = "if t0 == t1 then 0 else t0 * 10 + t1" }
"""
model = figs.add_writes(MODEL, {"habitat.stack": "habitat.tif", "with_farms.stack": "with_farms.tif",
                                "updated.stack": "updated.tif", "gained.map": "gained.tif",
                                "series.raster": "series.tif", "change.map": "change.tif"})
figs.run(model, work)

for f in ("habitat", "with_farms", "updated", "series"):
    with rasterio.open(work / f"{f}.tif") as ds:
        print(f, ds.count, ds.descriptions)

contact_sheet(work / "habitat.tif", out / "habitat_sheet.png", cols=3, width=900)
with rasterio.open(work / "updated.tif") as ds:
    b = list(ds.descriptions).index("dist_farm") + 1
figs.raster(work / "with_farms.tif", out / "dist_farm_1997.png", band=b, cmap="magma_r", vmin=0, vmax=6000, unit="m",
            width=640, title="dist_farm from the map of 1997")
figs.raster(work / "updated.tif", out / "dist_farm_2011.png", band=b, cmap="magma_r", vmin=0, vmax=6000, unit="m",
            width=640, title="dist_farm from the map of 2011")
figs.raster(work / "gained.tif", out / "dist_farm_change.png", diverging=True, unit="m", width=640,
            title="after.dist_farm - before.dist_farm")

# the time series: two files read as one stack, the shares of the classes, and the transitions
for band, year in ((1, "1997"), (2, "2011")):
    figs.raster(work / "series.tif", out / f"landuse_{year}.png", band=band, palette=SIBUYAN, legend=False, width=640)
rows = []
areas = figs.class_areas(work / "series.tif", {k: v[0] for k, v in SIBUYAN.items()})
for r, year in zip(areas, ("1997", "2011")):
    r = dict(r)
    r.pop("step")
    rows.append({"year": year, **r})
figs.write_csv(data / "shares.csv", rows)
print(rows)
ch = figs.read(work / "change.tif")[0]
vals, counts = np.unique(ch.compressed(), return_counts=True)
top = [int(v) for v, c in sorted(zip(vals, counts), key=lambda x: -x[1]) if v != 0][:6]
print("transitions:", {int(v): int(c) for v, c in zip(vals, counts)})
TR = {0: ("No change", "#e9e4d8")}
pal = ["#d55e00", "#e69f00", "#cc79a7", "#0072b2", "#56b4e9", "#009e73"]
for i, v in enumerate(top):
    TR[v] = (f"{SIBUYAN[v // 10][0]} to {SIBUYAN[v % 10][0].lower()} ({v})", pal[i])
arr = ch.copy()
arr[~np.isin(arr.filled(-1), list(TR))] = np.ma.masked
figs.raster(None, out / "transitions.png", array=arr, palette=TR, counts=True, width=860,
            title="if t0 == t1 then 0 else t0 * 10 + t1")

# ------------------------------------------------------------------------------------------------ stack.classes
# CLUE-S made by terraflux new, its suitability (glm.predict) written to a file; then the same model with the
# suitability read from that file and declared classes: the same maps
clue = work / "clue.toml"
figs.cli("new", "clue", "--observed", "landuse.tif", "--times", "0,14", "--drivers", "drivers.tif", "-o", str(clue),
         cwd=work)
text = clue.read_text(encoding="utf-8")
first = figs.add_writes(text, {"suitability.suitability": "suitability.tif"})
figs.run(first, work, name="clue_glm.toml")
shutil.copy(work / "output" / "maps.tif", work / "maps_glm.tif")
parts = text.split("[[nodes]]")
parts = [p for p in parts if 'id = "regression"' not in p and 'id = "suitability"' not in p]
second = "[[nodes]]".join(parts)
second = second.replace("[inputs.drivers]", '[inputs.suitability]\nkind = "stack"\npath = "suitability.tif"\n\n[inputs.drivers]')
second = second.replace('suitability = "suitability.suitability"', 'suitability = "classes.stack"')
second += '\n[[nodes]]\nid = "classes"\nop = "stack.classes"\ninputs = { stack = "$suitability" }\n'
figs.run(second, work, name="clue_file.toml")
a = rasterio.open(work / "maps_glm.tif").read()
b = rasterio.open(work / "output" / "maps.tif").read()
print("CLUE-S from the file, the same maps:", np.array_equal(a, b), a.shape)
without = second.replace('suitability = "classes.stack"', 'suitability = "$suitability"')
r = figs.run(without, work, name="clue_noclasses.toml", check=False)
print("without stack.classes:", (r.stdout + r.stderr).strip().splitlines()[-1])
contact_sheet(work / "suitability.tif", out / "suitability_sheet.png", cols=5, width=1000, same_scale=True,
              title_of=lambda n: f'"{n}" {SIBUYAN[int(n)][0].lower()}')
print("done")
