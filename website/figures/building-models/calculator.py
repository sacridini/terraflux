"""Figures of the tutorial "The raster calculator" (building-models/calculator).

Sibuyan Island (Philippines), land use of 1997 and 2011 and its drivers at 250 m (lulcc's and landschaft's example
data): one map.calc script gives the years each cell was forest, the forest lost, a habitat suitability index of
the forest of 1997, its best quarter and its mean, in a real run of Terraflux. The application's own pictures of
the calculator are drawn by `cargo test --release -p terraflux-app --test readme_images -- --ignored`.
"""

import json
import shutil
import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

NAME = "building-models/calculator"
work = figs.workdir(NAME)
out = figs.img_dir(NAME)
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
shutil.copy(INPUTS / "sibuyan_landuse.tif", work / "landuse.tif")
shutil.copy(INPUTS / "sibuyan_drivers.tif", work / "drivers.tif")
CELL_KM2 = 0.25 * 0.25

# the script of the tutorial, as the page shows it
SCRIPT = """# lu: the land use of 1997 and 2011 (a stack); d: the drivers (a stack)
forest = lu == 1                        # a stack: the forest of each year
out years = sum(forest)                 # years as forest: 0, 1 or 2
out lost = forest._1997 and not forest._2011

# a score from 0 to 1 for elevation and for roads, and the index
si_elev = clamp((1200 - d.elevation) / 900, 0, 1)
si_road = clamp(d.dist_roads / 3000, 0, 1)
out hsi = sqrt(si_elev * si_road)
hsi[not forest._1997] = null            # the forest of 1997 only

out best = hsi >= hsi.percentile(75)    # its best quarter
out mean_hsi = hsi.mean()
"""

MODEL = f'''format = "terraflux/1"

[inputs.lu]
kind = "stack"
path = "landuse.tif"

[inputs.d]
kind = "stack"
path = "drivers.tif"

[[nodes]]
id = "calc"
op = "map.calc"
inputs = {{ lu = "$lu", d = "$d" }}
params = {{ expression = """
{SCRIPT}""" }}
'''

writes = {f"calc.{k}": f"{k}.tif" for k in ("years", "lost", "hsi", "best")}
writes["calc.mean_hsi"] = "mean_hsi.json"
model = figs.add_writes(MODEL, writes)
figs.run(model, work)

figs.raster(work / "years.tif", out / "years.png", width=760,
            palette={0: ("Not forest", "#e6d36a"), 1: ("Forest in one year", "#a7c957"),
                     2: ("Forest in both", "#3b8a5a")}, counts=True)
figs.raster(work / "lost.tif", out / "lost.png", width=760,
            palette={0: ("Kept, or never forest", "#dddddd"), 1: ("Forest lost", "#c0392b")}, counts=True)
figs.raster(work / "hsi.tif", out / "hsi.png", width=760, vmin=0, vmax=1, unit="index")
figs.raster(work / "best.tif", out / "best.png", width=760,
            palette={0: ("The other forest", "#a7c957"), 1: ("The best quarter", "#1f5f3a")}, counts=True)

# the numbers the page quotes
years, _ = figs.read(work / "years.tif")
lost, _ = figs.read(work / "lost.tif")
hsi, _ = figs.read(work / "hsi.tif")
best, _ = figs.read(work / "best.tif")
mean_file = next((work / "output").glob("mean_hsi*"))
numbers = {
    "forest both years km2": float((years == 2).sum() * CELL_KM2),
    "forest lost km2": float((lost == 1).sum() * CELL_KM2),
    "forest 1997 km2": float(hsi.count() * CELL_KM2),
    "best quarter km2": float((best == 1).sum() * CELL_KM2),
    "hsi mean (numpy)": float(hsi.mean()),
    "mean_hsi (terraflux)": mean_file.read_text(encoding="utf-8").strip(),
    "hsi p75 (numpy, linear)": float(np.percentile(hsi.compressed(), 75)),
}
print(json.dumps(numbers, indent=2))
