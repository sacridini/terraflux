"""Figures of the tutorial "Terrain from an elevation model" (surface-metrics/terrain).

The SRTM elevation of south-western Oregon shipped with landschaft (1 arc-second, about 30 m), reprojected to UTM
zone 10N, and its terrain indices computed by Terraflux: slope, aspect, hillshade, TPI at many scales, TRI, VRM,
curvature, heat load and Weiss's slope positions.

    python website/figures/surface-metrics/terrain.py
"""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

SLUG = "surface-metrics/terrain"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
shutil.copy(figs.ROOT / "extern/landschaft/src/landschaft/surface/data/orelevation.tif", work / "elevation.tif")

EXTENT = "[500010, 4760850, 508110, 4771920]"
HEAD = f'''format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = {{ path = "elevation.tif" }}

[[nodes]]
id = "elevation"
op = "raster.reproject"
inputs = {{ raster = "dem.raster" }}
params = {{ crs = "EPSG:32610", resolution = 30, extent = {EXTENT}, resampling = "bilinear" }}
'''

# ------------------------------------------------------------------ the tutorial's model, every index written
MODEL = HEAD + '''
[[nodes]]
id = "slope"
op = "surface.terrain"
inputs = { map = "elevation.raster" }
params = { v = "slope" }

[[nodes]]
id = "aspect"
op = "surface.terrain"
inputs = { map = "elevation.raster" }
params = { v = "aspect" }

[[nodes]]
id = "shade"
op = "raster.terrain"
inputs = { raster = "elevation.raster" }
params = { measure = "hillshade" }

[[nodes]]
id = "tpi"
op = "surface.tpi"
inputs = { map = "elevation.raster" }
params = { scale = 15 }

[[nodes]]
id = "tri"
op = "surface.tri"
inputs = { map = "elevation.raster" }
params = { s = 3 }

[[nodes]]
id = "vrm"
op = "surface.vrm"
inputs = { map = "elevation.raster" }
params = { s = 5 }

[[nodes]]
id = "profile"
op = "surface.curvature"
inputs = { map = "elevation.raster" }
params = { type = "profile" }

[[nodes]]
id = "planform"
op = "surface.curvature"
inputs = { map = "elevation.raster" }
params = { type = "planform" }

[[nodes]]
id = "heat"
op = "surface.hli"
inputs = { aspect = "aspect.map", slope = "slope.map" }
params = { scaled = true }

[[nodes]]
id = "gdal_slope"
op = "raster.terrain"
inputs = { raster = "elevation.raster" }
params = { measure = "slope" }

[[nodes]]
id = "geo_slope"
op = "surface.terrain"
inputs = { map = "dem.raster" }

[[nodes]]
id = "geo_gdal_slope"
op = "raster.terrain"
inputs = { raster = "dem.raster" }
params = { measure = "slope", options = "-s 111120" }
'''
writes = {"elevation.raster": "elevation_utm.tif", "slope.map": "slope.tif", "aspect.map": "aspect.tif",
          "shade.raster": "shade.tif", "tpi.map": "tpi15.tif", "tri.map": "tri.tif", "vrm.map": "vrm.tif",
          "profile.map": "profile.tif", "planform.map": "planform.tif", "heat.map": "heat.tif",
          "gdal_slope.raster": "gdal_slope.tif", "geo_slope.map": "geo_slope.tif",
          "geo_gdal_slope.raster": "geo_gdal_slope.tif"}
figs.run(figs.add_writes(MODEL, writes), work)


def rd(name):
    return figs.read(work / name)[0]


z = rd("elevation_utm.tif")
shade = rd("shade.tif")
slope = rd("slope.tif")
aspect = rd("aspect.tif")
print("elevation", z.min(), z.max(), z.shape)


def fig_map(png, arr, cmap, title=None, unit="", vmin=None, vmax=None, norm=None, under=None, alpha=1.0,
            width=760, legend=True, classes=None):
    """A map in the site's style, optionally over a hillshade (under) and with a colour bar or class legend."""
    h, w = arr.shape
    dpi = 150
    win = width / dpi
    mapw = win * (0.74 if legend else 1.0)
    fig = plt.figure(figsize=(win, mapw * h / w + (0.35 if title else 0.1)), dpi=dpi)
    top = 1 - (0.32 / (mapw * h / w + 0.35)) if title else 1.0
    ax = fig.add_axes([0, 0, 0.74 if legend else 1.0, top])
    ax.set_axis_off()
    if under is not None:
        ax.imshow(under, cmap="gray", vmin=0, vmax=255, interpolation="nearest")
    if classes is not None:
        lut = {v: mcolors.to_rgba(c) for v, (_, c) in classes.items()}
        rgba = np.zeros(arr.shape + (4,))
        f = arr.filled(-999)
        for v, col in lut.items():
            rgba[f == v] = col
        if under is not None:
            rgba[..., 3] *= alpha
        ax.imshow(rgba, interpolation="nearest")
        if legend:
            total = arr.count()
            hs = [Patch(facecolor=c, edgecolor="#00000022", label=f"{n}  {(f == v).sum() / total * 100:.0f}%")
                  for v, (n, c) in classes.items()]
            fig.legend(handles=hs, loc="center left", bbox_to_anchor=(0.75, 0.5), fontsize=9.5, handlelength=1.1,
                       handleheight=1.1, labelspacing=0.55)
    else:
        cm = plt.get_cmap(cmap).copy()
        cm.set_bad((1, 1, 1, 0))
        im = ax.imshow(arr, cmap=cm, vmin=vmin, vmax=vmax, norm=norm, alpha=alpha, interpolation="nearest")
        if legend:
            cax = fig.add_axes([0.78, 0.12 * top, 0.035, 0.74 * top])
            cb = fig.colorbar(im, cax=cax)
            cb.outline.set_visible(False)
            cb.ax.grid(False)
            cb.ax.tick_params(labelsize=9, color=figs.RULE)
            if unit:
                cb.set_label(unit, fontsize=9.5, color=figs.INK2)
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.06)
    plt.close(fig)
    figs._shrink(png)


# ------------------------------------------------------------------ the elevation, the hillshade, slope, aspect
fig_map(out / "elevation.png", z, "viridis", "Elevation (m)", under=shade, alpha=0.7, unit="metres")
fig_map(out / "hillshade.png", shade, "gray", "Hillshade", vmin=0, vmax=255, legend=False, width=480)
fig_map(out / "slope.png", slope, "magma_r", "Slope (degrees)", unit="degrees", vmin=0, vmax=50, width=480)
fig_map(out / "aspect.png", aspect, "twilight_shifted", "Aspect (degrees from north)", unit="degrees",
        vmin=0, vmax=360, width=480)

# ------------------------------------------------------------------ TPI, TRI, VRM, curvature, heat load
tpi15 = rd("tpi15.tif")
m = float(np.nanpercentile(np.abs(tpi15.compressed()), 98))
fig_map(out / "tpi15.png", tpi15, "RdBu_r", "TPI, 15 x 15 cells (m)", unit="metres",
        norm=mcolors.TwoSlopeNorm(vmin=-m, vcenter=0, vmax=m), width=480)
tri = rd("tri.tif")
vrm = rd("vrm.tif")
fig_map(out / "tri.png", tri, "YlOrBr", "TRI, 3 x 3 cells (m)", unit="metres", vmin=0,
        vmax=float(np.nanpercentile(tri.compressed(), 99)), width=480)
fig_map(out / "vrm.png", vrm, "YlOrBr", "VRM, 5 x 5 cells", unit="0 smooth, 1 broken", vmin=0,
        vmax=float(np.nanpercentile(vrm.compressed(), 99)), width=480)
prof = rd("profile.tif")
plan = rd("planform.tif")
for name, a, t in (("profile", prof, "Profile curvature (1/m)"), ("planform", plan, "Planform curvature (1/m)")):
    c = float(np.nanpercentile(np.abs(a.compressed()), 97))
    fig_map(out / f"{name}.png", a, "PuOr_r", t, unit="1/m", norm=mcolors.TwoSlopeNorm(vmin=-c, vcenter=0, vmax=c),
            width=480)
heat = rd("heat.tif")
fig_map(out / "heat.png", heat, "inferno", "Heat load (MJ cm⁻² yr⁻¹)", unit="MJ cm⁻² yr⁻¹", width=480)

# ------------------------------------------------------------------ numbers quoted in the page
def stats(a):
    a = a.compressed()
    return dict(mean=float(a.mean()), min=float(a.min()), max=float(a.max()), sd=float(a.std()), n=a.size)


numbers = {}
for n in ("slope", "tpi15", "tri", "vrm", "profile", "planform", "heat", "gdal_slope", "geo_slope",
          "geo_gdal_slope", "aspect"):
    numbers[n] = stats(rd(n + ".tif"))
# curvature's sign against the TPI: convex cells sit above their neighbours
t3 = tpi15
both = ~(prof.mask | t3.mask)
numbers["corr_profile_tpi15"] = float(np.corrcoef(prof.data[both], t3.data[both])[0, 1])
both = ~(plan.mask | t3.mask)
numbers["corr_planform_tpi15"] = float(np.corrcoef(plan.data[both], t3.data[both])[0, 1])
both = ~(tri.mask | slope.mask)
numbers["corr_tri_slope"] = float(np.corrcoef(tri.data[both], slope.data[both])[0, 1])
both = ~(vrm.mask | slope.mask)
numbers["corr_vrm_slope"] = float(np.corrcoef(vrm.data[both], slope.data[both])[0, 1])
# heat load on the south-west and north-east faces
asp = aspect.filled(np.nan)
sl = slope.filled(np.nan)
hh = heat.filled(np.nan)
sw = (asp > 180) & (asp < 270) & (sl > 15)
ne = (asp > 0) & (asp < 90) & (sl > 15)
numbers["heat_sw"] = float(np.nanmean(hh[sw]))
numbers["heat_ne"] = float(np.nanmean(hh[ne]))

# a cell by hand: Horn's slope from its 3 x 3 window
r0, c0 = 200, 120
win = z.data[r0 - 1:r0 + 2, c0 - 1:c0 + 2]
a_, b_, c_, d_, e_, f_, g_, h_, i_ = win.ravel()
dzdx = ((c_ + 2 * f_ + i_) - (a_ + 2 * d_ + g_)) / (8 * 30)
dzdy = ((g_ + 2 * h_ + i_) - (a_ + 2 * b_ + c_)) / (8 * 30)
by_hand = np.degrees(np.arctan(np.hypot(dzdx, dzdy)))
numbers["hand"] = dict(window=win.round(1).tolist(), dzdx=dzdx, dzdy=dzdy, slope=float(by_hand),
                       tool=float(slope.data[r0, c0]), tpi3=float(z.data[r0, c0] - win.mean()))


# the toy window drawing: the cell's 3 x 3 neighbourhood with Horn's weights
fig, axes = plt.subplots(1, 3, figsize=(9.6, 3.3), dpi=150, gridspec_kw={"wspace": 0.35})
titles = ["Elevations (m)", "Weights for east–west", "Weights for north–south"]
wx = np.array([[-1, 0, 1], [-2, 0, 2], [-1, 0, 1]])
wy = np.array([[-1, -2, -1], [0, 0, 0], [1, 2, 1]])
for ax, t, vals in zip(axes, titles, (win, wx, wy)):
    ax.set_xlim(0, 3)
    ax.set_ylim(3, 0)
    ax.set_aspect("equal")
    ax.set_axis_off()
    ax.set_title(t, loc="left", fontsize=11)
    for rr in range(3):
        for cc in range(3):
            v = vals[rr, cc]
            if t.startswith("Elev"):
                face = plt.get_cmap("terrain")((v - win.min()) / (win.max() - win.min() + 1e-9) * 0.6 + 0.25)
                txt = f"{v:.0f}"
            else:
                face = "#f4c7a1" if v > 0 else ("#a9c8e3" if v < 0 else "#f3f5f3")
                txt = f"{v:+d}" if v else "0"
            ax.add_patch(plt.Rectangle((cc, rr), 1, 1, facecolor=face, edgecolor="white", lw=2))
            ax.text(cc + 0.5, rr + 0.5, txt, ha="center", va="center", fontsize=12, color=figs.INK,
                    fontweight="bold" if (rr, cc) == (1, 1) else "normal")
    if t.startswith("Elev"):
        ax.add_patch(plt.Rectangle((1, 1), 1, 1, fill=False, edgecolor=figs.EMBER, lw=2.5))
fig.savefig(out / "horn-window.png", dpi=150, bbox_inches="tight", pad_inches=0.08)
plt.close(fig)
figs._shrink(out / "horn-window.png")

# ------------------------------------------------------------------ TPI along a profile, two windows
row = 185
prof_z = z.data[row]
x = np.arange(prof_z.size) * 30 / 1000


def running_mean(v, k):
    out_ = np.full(v.size, np.nan)
    h = k // 2
    for i in range(h, v.size - h):
        out_[i] = v[i - h:i + h + 1].mean()
    return out_


fig, axes = plt.subplots(2, 1, figsize=(9.6, 4.8), dpi=150, sharex=True, gridspec_kw={"height_ratios": [2, 1.1]})
ax = axes[0]
ax.plot(x, prof_z, color=figs.INK, lw=1.6, label="Elevation along a row of the map")
for k, col in ((5, figs.EMBER), (41, figs.WATER)):
    ax.plot(x, running_mean(prof_z, k), color=col, lw=1.4, ls="--",
            label=f"Mean of a window of {k} cells ({k * 30} m)")
ax.set_ylabel("Elevation (m)")
ax.legend(fontsize=9, loc="lower left", bbox_to_anchor=(0, 1.0), ncol=3)
ax2 = axes[1]
for k, col in ((5, figs.EMBER), (41, figs.WATER)):
    ax2.plot(x, prof_z - running_mean(prof_z, k), color=col, lw=1.3, label=f"TPI, {k} cells")
ax2.axhline(0, color=figs.INK2, lw=0.8)
ax2.set_ylabel("TPI (m)")
ax2.set_xlabel("Distance along the row (km)")
ax2.legend(fontsize=9, loc="lower left", ncol=2)
fig.tight_layout()
fig.savefig(out / "tpi-profile.png", dpi=150)
plt.close(fig)
figs._shrink(out / "tpi-profile.png")

# ------------------------------------------------------------------ TPI at many scales, and Weiss's slope positions
SCALES = [3, 5, 9, 15, 25, 41, 61]
m2 = HEAD + '''
[[nodes]]
id = "slope"
op = "surface.terrain"
inputs = { map = "elevation.raster" }
'''
w2 = {"slope.map": "slope_s.tif"}
for s in SCALES:
    m2 += f'''
[[nodes]]
id = "tpi_{s}"
op = "surface.tpi"
inputs = {{ map = "elevation.raster" }}
params = {{ scale = {s} }}
'''
    w2[f"tpi_{s}.map"] = f"tpi_{s}.tif"
figs.run(figs.add_writes(m2, w2), work, name="scales.toml")
sds = {s: float(rd(f"tpi_{s}.tif").compressed().std()) for s in SCALES}
means = {s: float(rd(f"tpi_{s}.tif").compressed().mean()) for s in SCALES}
print("TPI sd per scale", sds)

# the slope positions by map.calc, the thresholds +-0.5 and +-1 standard deviation of each scale's TPI
m3 = m2
w3 = {}
for s in SCALES:
    sd = round(sds[s], 2)
    expr = (f"if t > {sd} then 6 else if t > {round(sd / 2, 2)} then 5 else if t >= -{round(sd / 2, 2)} and "
            f"s > 5 then 4 else if t >= -{round(sd / 2, 2)} then 3 else if t >= -{sd} then 2 else 1")
    m3 += f'''
[[nodes]]
id = "position_{s}"
op = "map.calc"
inputs = {{ t = "tpi_{s}.map", s = "slope.map" }}
params = {{ expression = "{expr}" }}
'''
    w3[f"position_{s}.map"] = f"position_{s}.tif"
figs.run(figs.add_writes(m3, w3), work, name="positions.toml")
numbers["thresholds"] = {s: [round(sds[s], 2), round(sds[s] / 2, 2)] for s in SCALES}
numbers["expr15"] = (f"if t > {round(sds[15], 2)} then 6 else if t > {round(sds[15] / 2, 2)} then 5 else if "
                     f"t >= -{round(sds[15] / 2, 2)} and s > 5 then 4 else if t >= -{round(sds[15] / 2, 2)} "
                     f"then 3 else if t >= -{round(sds[15], 2)} then 2 else 1")

POS = {1: ("Valley", "#2f6f9f"), 2: ("Lower slope", "#8fbcd9"), 3: ("Flat", "#e9e4d8"),
       4: ("Middle slope", "#a7c957"), 5: ("Upper slope", "#e3a65b"), 6: ("Ridge", "#b5452c")}
rows = []
frames = []
for s in SCALES:
    p = rd(f"position_{s}.tif")
    fig_map(out / f"position_{s}.png", p, None, None, classes=POS, under=shade, alpha=0.78, width=560,
            legend=False)
    t = rd(f"tpi_{s}.tif")
    if s in (3, 61):
        mm = float(np.nanpercentile(np.abs(t.compressed()), 98))
        fig_map(out / f"tpi_{s}.png", t, "RdBu_r", f"TPI, {s} x {s} cells (m)", unit="metres",
                norm=mcolors.TwoSlopeNorm(vmin=-mm, vcenter=0, vmax=mm), width=480)
    tot = p.count()
    r = {"scale": s * 30}
    for v, (n, _) in POS.items():
        r[n] = round(float((p == v).sum()) / tot * 100, 2)
    r["cells_with_value"] = int(tot)
    r["sd"] = round(sds[s], 2)
    r["with_value"] = round(tot / z.count() * 100, 1)
    rows.append(r)
figs.write_csv(data / "positions.csv", [{k: v for k, v in r.items() if k not in ("cells_with_value", "sd", "with_value")}
                                       for r in rows])
figs.write_csv(data / "tpi-scale.csv", [{"window_m": r["scale"], "TPI standard deviation (m)": r["sd"],
                                         "Cells with a value (%)": r["with_value"]} for r in rows])
numbers["positions"] = rows
numbers["tpi_sd"] = sds
numbers["tpi_mean"] = means

# the legend of the slope positions, drawn once beside the slider
fig = plt.figure(figsize=(6.4, 0.5), dpi=150)
hs = [Patch(facecolor=c, edgecolor="#00000022", label=n) for v, (n, c) in POS.items()]
fig.legend(handles=hs, loc="center", ncol=6, fontsize=9.5, handlelength=1.1, handleheight=1.1, columnspacing=1.2)
fig.savefig(out / "position-legend.png", dpi=150, bbox_inches="tight", pad_inches=0.04)
plt.close(fig)

# ------------------------------------------------------------------ TRI and VRM against slope (a sample of cells)
rng = np.random.default_rng(1)
ok = np.argwhere(~(tri.mask | vrm.mask | slope.mask))
pick = ok[rng.choice(len(ok), 500, replace=False)]
sample = [{"slope": round(float(slope.data[i, j]), 2), "TRI": round(float(tri.data[i, j]), 2),
           "VRM_x1000": round(float(vrm.data[i, j]) * 1000, 2)} for i, j in pick]
sample.sort(key=lambda r: r["slope"])
figs.write_csv(data / "tri-vrm-slope.csv", sample)

# slope on the geographic grid: terra's (metres per row) and gdaldem's with one scale
import json  # noqa: E402

(work / "numbers.json").write_text(json.dumps(numbers, indent=1))
print(json.dumps({k: v for k, v in numbers.items() if k not in ("positions",)}, indent=1))
for r in rows:
    print(r)
