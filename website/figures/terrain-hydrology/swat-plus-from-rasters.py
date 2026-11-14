"""Figures of the tutorial "A SWAT+ project from rasters, and its results on the map" (terrain-hydrology/swat-plus-from-rasters).

The application's template "SWAT+ from rasters" (website/models/templates/swat-plus-from-rasters.toml: a valley of
30 x 30 cells of 100 m drawn by chance, two soils, one station and two years of weather made up) run by Terraflux:
the division of the valley (subbasins, channels, HRUs), the soil lost, the runoff and the mean discharge on the map,
the daily discharge of the channels, and the same model with other stream thresholds, HRU methods and parameters.

    python website/figures/terrain-hydrology/swat-plus-from-rasters.py
"""

import csv
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _hydro as H  # noqa: E402
from _hydro import figs, np, plt  # noqa: E402

SLUG = "swat-plus-from-rasters"
img = figs.img_dir(f"terrain-hydrology/{SLUG}")
data = figs.data_dir(f"terrain-hydrology/{SLUG}")
BASE = figs.template(SLUG)
WRITES = {
    "watershed.subbasins": "subbasins.tif",
    "watershed.channels": "channels.tif",
    "watershed.hrus": "hrus.tif",
    "sediment.map": "sediment.tif",
    "runoff.map": "runoff.tif",
    "flow.map": "flow.tif",
    "discharge.table": "discharge.csv",
    "watershed.table": "hrus.csv",
}
WATERSHED = "params = { stream_threshold = 0.3 }"
assert WATERSHED in BASE


def variant(tag, model=BASE, *sets):
    """The model run in a folder of its own: its maps and tables (`--drawn`: those already in the folders, to try
    the drawing)."""
    drawn = "--drawn" in sys.argv
    work = figs.workdir(f"terrain-hydrology/{SLUG}/{tag}", clean=not drawn)
    if not drawn:
        figs.run(figs.add_writes(model, WRITES), work, *sets)
    return work


def mean(work, name):
    a = figs.read(work / name)[0]
    return float(a.mean())


def table(work, name):
    return figs.read_csv(work / "output" / name)


base = variant("base")
sub = figs.read(base / "subbasins.tif")[0]
chan = figs.read(base / "channels.tif")[0]
hrus = figs.read(base / "hrus.tif")[0]
sed = figs.read(base / "sediment.tif")[0]
run = figs.read(base / "runoff.tif")[0]
flow = figs.read(base / "flow.tif")[0]
q = table(base, "discharge.csv")
hru_rows = table(base, "hrus.csv")

# ------------------------------------------------------------------ the division: subbasins, channels, HRUs
fig, axes = plt.subplots(1, 3, figsize=(9.6, 3.5), dpi=150)
cmap = plt.get_cmap("tab20")
shown = np.ma.masked_where(~(sub > 0), sub)
axes[0].imshow(shown, cmap=cmap, interpolation="nearest")
axes[0].set_title("Subbasins")
axes[1].imshow(np.ma.masked_where(~(sub > 0), np.zeros_like(sub)), cmap="Greys", vmin=0, vmax=3, interpolation="nearest")
axes[1].imshow(np.ma.masked_where(~(chan > 0), chan), cmap="Blues_r", vmin=-5, interpolation="nearest")
axes[1].set_title("Channels")
# an HRU by its land use (forest or pasture) and its soil (the upland loam darker)
tone = {("FRSD", "UPLAND_LOAM"): "#2c6a45", ("FRSD", "VALLEY_CLAY_LOAM"): "#7fbf96",
        ("PAST", "UPLAND_LOAM"): "#c98f1f", ("PAST", "VALLEY_CLAY_LOAM"): "#ecd08a"}
colour = {int(float(r["id"])): tone[(r["landuse"], r["soil"])] for r in hru_rows}
filled = hrus.filled(0).astype(int)
rgba = np.zeros(hrus.shape + (4,))
for k, c in colour.items():
    rgba[filled == k] = plt.matplotlib.colors.to_rgba(c)
axes[2].imshow(rgba, interpolation="nearest")
axes[2].set_title("HRUs")
for ax in axes:
    ax.set_axis_off()
fig.tight_layout()
fig.savefig(img / "division.png", dpi=150)
plt.close(fig)
figs._shrink(img / "division.png")

# ------------------------------------------------------------------ the results on the map
figs.raster(None, img / "sediment.png", array=sed, cmap="YlOrBr", title="Soil lost (t/ha a year)", width=640)
figs.raster(None, img / "runoff.png", array=run, cmap="Blues", title="Surface runoff (mm a year)", width=640)
figs.raster(None, img / "flow.png", array=flow, cmap="viridis", title="Mean discharge (m3/s)", width=640)

# ------------------------------------------------------------------ the daily discharge
cols = [c for c in q[0] if c.startswith("channel_")]
series = {c: np.array([float(r[c]) for r in q]) for c in cols}
order = sorted(cols, key=lambda c: -series[c].mean())
day = np.array([float(r["day"]) for r in q])


def draw(ax):
    for k, c in enumerate(order):
        ax.plot(day, series[c], lw=1.3 if k == 0 else 0.7, color=figs.WATER if k == 0 else "#9ecae1",
                label="the outlet" if k == 0 else ("the headwater channels" if k == 1 else None))
    ax.set_xlabel("Day of the year")
    ax.set_ylabel("Discharge (m³/s)")
    ax.legend(fontsize=9, loc="upper right")


figs.chart(img / "hydrograph.png", draw, width=1000, height=380)

# ------------------------------------------------------------------ the stream threshold and the HRU method
rows = []
for thr in (0.15, 0.3, 0.6, 1.2):
    work = variant(f"threshold_{thr}", BASE.replace(WATERSHED, f"params = {{ stream_threshold = {thr} }}"))
    a = figs.read(work / "subbasins.tif")[0]
    rows.append({"threshold": thr, "subbasins": len({int(v) for v in a.compressed() if v > 0}),
                 "hrus": len(table(work, "hrus.csv"))})
figs.write_csv(data / "threshold.csv", [{"Stream threshold (km2)": r["threshold"], "Subbasins": r["subbasins"],
                                         "HRUs": r["hrus"]} for r in rows])
rows = []
for name, extra in (("dominant_hru (QSWAT+'s default)", ""), ("dominant", ', hru_method = "dominant"'),
                    ("filter, 10 % of each", ', hru_method = "filter", thresholds = [10, 10, 10]'),
                    ("filter, with slope bands",
                     ', hru_method = "filter", thresholds = [10, 10, 10], slope_bands = [15, 35]')):
    work = variant(f"method_{len(rows)}", BASE.replace(WATERSHED, f"params = {{ stream_threshold = 0.3{extra} }}"))
    rows.append({"HRU method": name, "HRUs": len(table(work, "hrus.csv"))})
figs.write_csv(data / "methods.csv", rows)

# ------------------------------------------------------------------ the parameters
rows = []
for name, key, values in (("cn2", "cn2_change", (-10, 0, 10)), ("awc", "awc_change", (-20, 0, 20))):
    for v in values:
        work = variant(f"{key}_{v}", BASE, "--set", f"{key}={v}")
        qq = table(work, "discharge.csv")
        peak = max(float(r[c]) for r in qq for c in qq[0] if c.startswith("channel_"))
        rows.append({"Parameter": name, "Change (%)": v, "Soil lost (t/ha)": mean(work, "sediment.tif"),
                     "Runoff (mm)": mean(work, "runoff.tif"), "Mean discharge (m3/s)": mean(work, "flow.tif"),
                     "Largest daily discharge (m3/s)": peak})
figs.write_csv(data / "parameters.csv", rows)
print("done")
