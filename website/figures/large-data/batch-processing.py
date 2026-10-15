"""Figures of 'Batch processing': the CORINE excerpt cut into 16 tiles, MSPA of every tile by batch.run (the table,
the files, a tile's JSON), the run again (tiles done are skipped), class metrics of every tile, and the tiles' MSPA
put back together."""

import json
import pathlib
import sys

import rasterio
from rasterio.windows import Window

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[0] / "getting-started"))
from _common import figs, np, plt, CLC, save  # noqa: E402

SLUG = "large-data/batch-processing"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

# ------------------------------------------------------------------ 16 tiles of 500 x 500 cells
(work / "tiles").mkdir()
with rasterio.open(CLC) as src:
    prof = src.profile
    for i in range(4):
        for j in range(4):
            w = Window(j * 500, i * 500, 500, 500)
            p = dict(prof, width=500, height=500, transform=src.window_transform(w), compress="lzw")
            with rasterio.open(work / "tiles" / f"clc_r{i}c{j}.tif", "w", **p) as dst:
                dst.write(src.read(1, window=w), 1)

MODEL = '''format = "terraflux/1"
name = "MSPA of every tile"

[[nodes]]
id = "batch"
op = "batch.run"
params = { operator = "landscape.mspa", inputs = "tiles", output_dir = "mspa", params = { foreground = [23, 24, 25], edge_width = 2 } }
note = "MSPA of the forests (CORINE 23, 24, 25) of every GeoTIFF of the folder tiles, written to the folder mspa."

[outputs.table]
from = "batch.table"

[outputs.report]
from = "batch.report"
'''
(work / "batch.toml").write_text(MODEL, encoding="utf-8")
log = []
for k in range(2):
    r = figs.cli("run", "batch.toml", "--out", f"run{k + 1}", cwd=work)
    log.append(f"$ terraflux run batch.toml --out run{k + 1}\n{r.strip()}\n")
print("\n".join(log))
files = sorted(p.name for p in (work / "mspa").iterdir())
log.append("$ ls mspa\n" + "\n".join(files[:6]) + f"\n... ({len(files)} files)\n")
for k in (1, 2):
    for f in sorted((work / f"run{k}").iterdir()):
        log.append(f"--- run{k}/{f.name}\n" + f.read_text(encoding="utf-8")[:900])
js = sorted((work / "mspa").glob("*.json"))[0]
log.append(f"--- mspa/{js.name}\n" + js.read_text(encoding="utf-8"))
(work / "cli.txt").write_text("\n".join(log), encoding="utf-8")
print("\n".join(log[2:]))

# the tiles' MSPA put back together, with the seams drawn
tifs = sorted((work / "mspa").glob("*.tif"))
print([t.name for t in tifs][:3])
mosaic = np.zeros((2000, 2000))
for t in tifs:
    name = t.name
    i, j = int(name[5]), int(name[7])
    a, _ = figs.read(t)
    mosaic[i * 500:(i + 1) * 500, j * 500:(j + 1) * 500] = a.filled(0)
from matplotlib import colors as mcolors  # noqa: E402

pal = figs.mspa_palette()
rgba = np.ones(mosaic.shape + (4,))
for v, (_, c) in pal.items():
    rgba[mosaic == v] = mcolors.to_rgba(c)
fig = plt.figure(figsize=(5.5, 5.5), dpi=150)
ax = fig.add_axes([0, 0, 1, 1])
ax.imshow(rgba, interpolation="nearest")
for k in range(1, 4):
    ax.axhline(k * 500 - 0.5, color="#111111", lw=0.8)
    ax.axvline(k * 500 - 0.5, color="#111111", lw=0.8)
ax.set_axis_off()
save(fig, out / "mosaic.png")

# ------------------------------------------------------------------ class metrics of every tile
M2 = '''format = "terraflux/1"
name = "Class metrics of every tile"

[[nodes]]
id = "batch"
op = "batch.run"
params = { operator = "metrics.class", inputs = "tiles/*.tif", output_dir = "metrics", params = { metrics = ["pland", "np", "ed"], show = "pland" } }

[outputs.table]
from = "batch.table"
'''
(work / "metrics.toml").write_text(M2, encoding="utf-8")
print(figs.cli("run", "metrics.toml", "--out", "run_metrics", cwd=work))
rows = []
for t in sorted((work / "metrics").glob("*.csv")):
    name = t.name.split("_metrics")[0]
    tab = figs.read_csv(t)
    forest = sum(float(r["pland"]) for r in tab if int(float(r["class"])) in (23, 24, 25))
    arable = sum(float(r["pland"]) for r in tab if int(float(r["class"])) == 12)
    urban = sum(float(r["pland"]) for r in tab if int(float(r["class"])) in (1, 2, 3, 4))
    rows.append({"tile": name.replace("clc_", ""), "forest": round(forest, 1), "arable": round(arable, 1), "urban": round(urban, 1)})
figs.write_csv(data / "tiles.csv", rows)
print(rows)
print(sorted(p.name for p in (work / "metrics").iterdir())[:8])
