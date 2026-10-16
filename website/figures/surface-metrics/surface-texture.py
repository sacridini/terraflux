"""Figures of the tutorial "Surface metrics: texture without patches" (surface-metrics/surface-texture).

Summer NDVI of a forested landscape in south-western Oregon (Landsat, 30 m, the example data of landschaft and of
the R package geodiv) and the elevation of the same place: thresholds turned into patches, the trend removed,
ISO 25178's surface metrics, the bearing area curve, texture in moving windows, and Gaussian fields of known grain.

    python website/figures/surface-metrics/surface-texture.py
"""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402

SLUG = "surface-metrics/surface-texture"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
src = figs.ROOT / "extern/landschaft/src/landschaft/surface/data"
shutil.copy(src / "orforest.tif", work / "ndvi.tif")
shutil.copy(src / "orelevation.tif", work / "elevation.tif")
numbers = {}

# ------------------------------------------------------------------ 1. thresholds: the same NDVI as patches
THRESHOLDS = [0.55, 0.60, 0.65, 0.70, 0.75, 0.80]
m = '''format = "terraflux/1"

[[nodes]]
id = "ndvi"
op = "raster.read"
params = { path = "ndvi.tif" }
'''
w = {}
for t in THRESHOLDS:
    k = int(round(t * 100))
    m += f'''
[[nodes]]
id = "habitat_{k}"
op = "map.calc"
inputs = {{ n = "ndvi.raster" }}
params = {{ expression = "if n >= {t} then 2 else 1" }}

[[nodes]]
id = "metrics_{k}"
op = "metrics.class"
inputs = {{ map = "habitat_{k}.map" }}
params = {{ metrics = ["pland", "np"], show = "pland" }}
'''
    w[f"habitat_{k}.map"] = f"habitat_{k}.tif"
    w[f"metrics_{k}.table"] = f"metrics_{k}.csv"
figs.run(figs.add_writes(m, w), work, name="thresholds.toml")

rows = []
frames = []
for t in THRESHOLDS:
    k = int(round(t * 100))
    tab = figs.read_csv(work / "output" / f"metrics_{k}.csv")
    hab = next(r for r in tab if float(r["class"]) == 2)
    pland, np_ = float(hab["pland"]), int(float(hab["np"]))
    rows.append({"threshold": t, "patches": np_, "habitat": round(pland, 1)})
    figs.raster(work / f"habitat_{k}.tif", out / f"habitat_{k}.png", colors={1: "#e9e4d8", 2: figs.FOREST},
                legend=False, width=520)
figs.write_csv(data / "thresholds.csv", [{"threshold": r["threshold"], "Forest patches": r["patches"]}
                                         for r in rows])
numbers["thresholds"] = rows
print(rows)

# ------------------------------------------------------------------ 2. the trend and the residuals, the metrics
MAIN = '''format = "terraflux/1"

[[nodes]]
id = "ndvi"
op = "raster.read"
params = { path = "ndvi.tif" }

[[nodes]]
id = "trend"
op = "surface.fit_plane"
inputs = { map = "ndvi.raster" }
params = { order = "best" }

[[nodes]]
id = "metrics"
op = "surface.metrics"
inputs = { map = "trend.residuals" }

[[nodes]]
id = "bearing"
op = "surface.bearing"
inputs = { map = "trend.residuals" }

[[nodes]]
id = "peaks"
op = "surface.peaks"
inputs = { map = "trend.residuals" }

[[nodes]]
id = "aacf"
op = "surface.aacf"
inputs = { map = "trend.residuals" }

[[nodes]]
id = "raw_metrics"
op = "surface.metrics"
inputs = { map = "ndvi.raster" }

[[nodes]]
id = "elevation"
op = "raster.read"
params = { path = "elevation.tif" }

[[nodes]]
id = "relief"
op = "surface.fit_plane"
inputs = { map = "elevation.raster" }

[[nodes]]
id = "relief_metrics"
op = "surface.metrics"
inputs = { map = "relief.residuals" }

[[nodes]]
id = "relief_aacf"
op = "surface.aacf"
inputs = { map = "relief.residuals" }
'''
WINDOWS = [5, 9, 15, 25]
for s in WINDOWS:
    MAIN += f'''
[[nodes]]
id = "texture_{s}"
op = "surface.window"
inputs = {{ map = "trend.residuals" }}
params = {{ metrics = ["sa", "sku"], window = {s} }}
'''
w = {"trend.plane": "plane.tif", "trend.residuals": "residuals.tif", "metrics.table": "metrics.csv",
     "bearing.table": "bearing.csv", "peaks.table": "peaks.csv", "aacf.map": "aacf.tif",
     "raw_metrics.table": "raw_metrics.csv", "relief.residuals": "relief_residuals.tif",
     "relief_metrics.table": "relief_metrics.csv", "relief_aacf.map": "relief_aacf.tif"}
for s in WINDOWS:
    w[f"texture_{s}.maps"] = f"texture_{s}.tif"
r = figs.run(figs.add_writes(MAIN, w), work, name="texture.toml")
print(r.stdout[-800:])

o = work / "output"
met = figs.read_csv(o / "metrics.csv")[0]
raw = figs.read_csv(o / "raw_metrics.csv")[0]
rel = figs.read_csv(o / "relief_metrics.csv")[0]
numbers["metrics"] = {k: float(v) for k, v in met.items()}
numbers["raw_metrics"] = {k: float(v) for k, v in raw.items()}
numbers["relief_metrics"] = {k: float(v) for k, v in rel.items()}
numbers["peaks"] = len(figs.read_csv(o / "peaks.csv"))
for k in met:
    print(f"{k:6s} ndvi-res {float(met[k]):12.5g}  ndvi-raw {float(raw[k]):12.5g}  relief-res {float(rel[k]):12.5g}")

ndvi = figs.read(work / "ndvi.tif")[0]
plane = figs.read(work / "plane.tif")[0]
res = figs.read(work / "residuals.tif")[0]
numbers["ndvi"] = dict(min=float(ndvi.min()), max=float(ndvi.max()), mean=float(ndvi.mean()))
numbers["res"] = dict(min=float(res.min()), max=float(res.max()), mean=float(res.mean()))
lo, hi = float(ndvi.min()), float(ndvi.max())
figs.raster(work / "ndvi.tif", out / "ndvi.png", cmap="YlGn", title="Summer NDVI", width=480, vmin=lo, vmax=hi)
figs.raster(work / "plane.tif", out / "plane.png", cmap="YlGn", title="Its trend (a polynomial)", width=480,
            vmin=lo, vmax=hi)
mres = float(np.nanpercentile(np.abs(res.compressed()), 99.5))
figs.raster(work / "residuals.tif", out / "residuals.png", diverging=True, title="Residuals: NDVI less the trend",
            width=480, vmin=-mres, vmax=mres, cmap="BrBG")

# the height distribution and the bearing area curve
b = figs.read_csv(o / "bearing.csv")
bear = np.array([float(x["bearing"]) for x in b])
hgt = np.array([float(x["height"]) for x in b])
order = np.argsort(bear)
bear, hgt = bear[order], hgt[order]
pick = np.unique(np.r_[np.linspace(0, len(bear) - 1, 160).astype(int), np.arange(0, len(bear), max(1, len(bear) // 2000))[:40]])
pick = np.unique(np.r_[pick, np.where(bear < 0.02)[0][:: max(1, len(np.where(bear < 0.02)[0]) // 20)],
                       np.where(bear > 0.98)[0][:: max(1, len(np.where(bear > 0.98)[0]) // 20)]])
figs.write_csv(data / "bearing.csv", [{"bearing": round(float(bear[i]) * 100, 3), "height": round(float(hgt[i]), 4)}
                                      for i in pick])
numbers["bearing_rows"] = len(b)


def hist(ax):
    v = res.compressed()
    ax.hist(v, bins=120, color=figs.FOREST, alpha=0.85)
    ax.axvline(0, color=figs.INK2, lw=0.8)
    ax.set_xlabel("Residual NDVI (height above or below the trend)")
    ax.set_ylabel("Cells")
    ax.annotate("a long tail of cells\nfar below the trend", xy=(-0.35, 150), xytext=(-0.55, 2500),
                fontsize=9, color=figs.INK2, arrowprops=dict(arrowstyle="->", color=figs.INK2))


figs.chart(out / "histogram.png", hist, width=900, height=380)

# texture in moving windows
for s in WINDOWS:
    sa = figs.read(work / f"texture_{s}.tif", 1)[0]
    figs.raster(None, out / f"sa_{s}.png", array=sa, cmap="magma", vmin=0, vmax=0.12, unit="Sa (NDVI)",
                width=560)
sku = figs.read(work / "texture_9.tif", 2)[0]
figs.raster(None, out / "sku_9.png", array=sku, cmap="PuOr_r", diverging=True, vmin=-6, vmax=6,
            title="Sku, 9 x 9 cells", width=480)
figs.raster(None, out / "sa_9_title.png", array=figs.read(work / "texture_9.tif", 1)[0], cmap="magma", vmin=0,
            vmax=0.12, title="Sa, 9 x 9 cells", width=480)
numbers["sa_window_mean"] = {s: float(figs.read(work / f"texture_{s}.tif", 1)[0].mean()) for s in WINDOWS}
figs.write_csv(data / "window.csv", [{"window": s, "Mean Sa": round(numbers["sa_window_mean"][s], 4)}
                                    for s in WINDOWS])
numbers["sa_window_cells"] = {s: int(figs.read(work / f"texture_{s}.tif", 1)[0].count()) for s in WINDOWS}

# the autocorrelation of the two surfaces
for name, f, t in (("aacf", "aacf.tif", "NDVI residuals"), ("relief_aacf", "relief_aacf.tif", "Elevation residuals")):
    a = figs.read(work / f)[0]
    n0, n1 = a.shape
    c0, c1 = n0 // 2, n1 // 2
    crop = a[c0 - 60:c0 + 61, c1 - 60:c1 + 61]
    figs.raster(None, out / f"{name}.png", array=crop, cmap="RdBu_r", diverging=True, vmin=-1, vmax=1,
                title=f"Autocorrelation, {t}", width=480)

# ------------------------------------------------------------------ 3. Gaussian fields of known grain
RANGES = [3, 6, 12, 24, 48]
g = 'format = "terraflux/1"\n'
w = {}
for rg in RANGES:
    g += f'''
[[nodes]]
id = "field_{rg}"
op = "nlm.gaussian_field"
params = {{ rows = 200, columns = 200, resolution = 30, range = {rg}, seed = 4 }}

[[nodes]]
id = "flat_{rg}"
op = "surface.fit_plane"
inputs = {{ map = "field_{rg}.map" }}

[[nodes]]
id = "metrics_{rg}"
op = "surface.metrics"
inputs = {{ map = "flat_{rg}.residuals" }}
params = {{ metrics = ["sq", "sdq", "sal", "str", "sfd"] }}
'''
    g += f'''
[[nodes]]
id = "raw_{rg}"
op = "surface.metrics"
inputs = {{ map = "field_{rg}.map" }}
params = {{ metrics = ["sal"] }}
'''
    w[f"raw_{rg}.table"] = f"raw_{rg}.csv"
    w[f"metrics_{rg}.table"] = f"field_{rg}.csv"
    w[f"field_{rg}.map"] = f"field_{rg}.tif"
figs.run(figs.add_writes(g, w), work, name="fields.toml")
grain = []
for rg in RANGES:
    t = figs.read_csv(o / f"field_{rg}.csv")[0]
    raw_sal = float(figs.read_csv(o / f"raw_{rg}.csv")[0]["sal"])
    grain.append({"range_m": rg * 30, "Sal (m)": round(float(t["sal"]), 1), "raw_sal": raw_sal, "sq": float(t["sq"]),
                  "sdq": float(t["sdq"]), "str": float(t["str"]), "sfd": float(t["sfd"])})
figs.write_csv(data / "grain.csv", [{"range_m": x["range_m"], "Autocorrelation length Sal (m)": x["Sal (m)"]}
                                    for x in grain])
numbers["grain"] = grain
print(grain)
for rg in (3, 12, 48):
    figs.raster(work / f"field_{rg}.tif", out / f"field_{rg}.png", cmap="viridis", legend=False, width=360,
                title=f"range = {rg} cells")

(work / "numbers.json").write_text(json.dumps(numbers, indent=1))
