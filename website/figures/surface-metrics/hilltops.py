"""Figures of the tutorial "Hilltops protected by Brazil's Forest Code" (surface-metrics/hilltops).

Two DEMs: a synthetic landscape made here (hills.tif, with its plain as plain.tif; both offered for download), whose
hills were drawn to show each part of the rule (a range of two peaks joined by a saddle, a lone hill, a hill too
gentle, a knoll too low, on a valley floor that falls to the east), and the SRTM elevation of south-western Oregon
shipped with landschaft, reprojected to UTM zone 10N, for real relief: its hills, the rule of CONAMA 303/2002's
thresholds, and how the cell size and the slope threshold change the answer. Every number is Terraflux's.

    python website/figures/surface-metrics/hilltops.py
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

SLUG = "surface-metrics/hilltops"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
LS = figs.ROOT / "extern/landschaft"
shutil.copy(LS / "src/landschaft/surface/data/orelevation.tif", work / "elevation.tif")

APP = "#e8901f"
PLAIN = "#9cc3de"

# ------------------------------------------------------------------ the synthetic landscape
import rasterio  # noqa: E402
from rasterio.transform import from_origin  # noqa: E402

NY, NX, CELL = 140, 200, 30.0
X0, Y0 = 600000.0, 7500000.0          # SIRGAS 2000 / UTM zone 23S (EPSG:31983)
yy, xx = np.mgrid[0:NY, 0:NX].astype(np.float64)
# the valley floor: 700 m in the west, 600 m in the east, gently undulating, with a river along it
floor = 700.0 - 80.0 * xx / (NX - 1) + 1.5 * np.sin(yy / 9.0) * np.sin(xx / 13.0)
river = np.abs(yy - (126 + 5 * np.sin(xx / 20.0))) < 1.5
floor = np.where(river, floor - 2.0, floor)


def cone(r0, c0, h, slope_deg):
    """A hill of height h (m) above the floor, of flanks at slope_deg, with a rounded top."""
    r = np.hypot(yy - r0, xx - c0) * CELL
    t = np.tan(np.radians(slope_deg))
    cap = 0.1 * h                         # the top rounded (a paraboloid) over its upper tenth
    r1 = cap / t
    top = h + cap / 2                     # so that the rounded summit stands h high
    z = np.where(r < r1, top - cap / 2 - t * r ** 2 / (2 * r1), top - r * t)
    return np.clip(z, 0, None)


HILLS = {  # name: (row, column, height above the floor, slope of the flanks)
    "A": (40, 42, 380.0, 31.0),     # the range's main peak
    "B": (40, 70, 300.0, 31.0),     # its second peak, joined to A by a saddle
    "C": (85, 140, 170.0, 33.0),    # a lone hill on the floor
    "D": (95, 40, 220.0, 14.0),     # high but gentle
    "E": (40, 165, 80.0, 35.0),     # steep but low
}
relief = np.zeros_like(floor)
for r0, c0, h, s in HILLS.values():
    relief = np.maximum(relief, cone(r0, c0, h, s))
dem = (floor + relief).astype(np.float32)
plain = ((relief < 0.5) | river).astype(np.uint8)
profile = {"transform": from_origin(X0, Y0, CELL, CELL), "crs": "EPSG:31983", "driver": "GTiff", "height": NY,
           "width": NX, "count": 1, "compress": "deflate"}
for path in (work / "hills.tif", data / "hills.tif"):
    with rasterio.open(path, "w", dtype="float32", nodata=-9999.0, **profile) as dst:
        dst.write(dem, 1)
for path in (work / "plain.tif", data / "plain.tif"):
    with rasterio.open(path, "w", dtype="uint8", nodata=255, **profile) as dst:
        dst.write(plain, 1)
print("synthetic", dem.min(), dem.max())

SYN = '''format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "hills.tif" }

[[nodes]]
id = "plain"
op = "raster.read"
params = { path = "plain.tif" }

[[nodes]]
id = "shade"
op = "raster.terrain"
inputs = { raster = "dem.raster" }
params = { measure = "hillshade" }

[[nodes]]
id = "hilltop"
op = "surface.hilltop"
inputs = { dem = "dem.raster", plain = "plain.raster" }

[[nodes]]
id = "noplain"
op = "surface.hilltop"
inputs = { dem = "dem.raster" }
'''
figs.run(figs.add_writes(SYN, {"shade.raster": "syn_shade.tif", "hilltop.app": "syn_app.tif",
                               "hilltop.hill": "syn_hill.tif", "hilltop.hills": "syn_hills.csv",
                               "noplain.app": "syn_app_noplain.tif", "noplain.hills": "syn_hills_noplain.csv"}),
         work, name="synthetic.toml")


def rd(name):
    return figs.read(work / name)[0]


def hills_of(name):
    return [{k: float(v) if v not in ("", None) else np.nan for k, v in r.items()}
            for r in figs.read_csv(work / "output" / name)]


def nearest(rows, r0, c0):
    """The row of the hills table whose summit is the one of a hill drawn at (r0, c0)."""
    x, y = X0 + (c0 + 0.5) * CELL, Y0 - (r0 + 0.5) * CELL
    return min(rows, key=lambda h: (h["x"] - x) ** 2 + (h["y"] - y) ** 2)


KIND = {0: "saddle", 1: "plain", 2: "lowest"}
syn = hills_of("syn_hills.csv")
syn_np = hills_of("syn_hills_noplain.csv")
print("synthetic summits", len(syn), "qualify", sum(h["qualifies"] for h in syn))
table = []
for name, (r0, c0, h, s) in HILLS.items():
    a, b = nearest(syn, r0, c0), nearest(syn_np, r0, c0)
    table.append({"hill": name, "top": round(a["top"], 1), "base": round(a["base"], 1),
                  "base_kind": KIND[int(a["base_kind"])], "height": round(a["height"], 1),
                  "mean_slope": round(a["mean_slope"], 1), "line_slope": round(a["line_slope"], 1),
                  "qualifies": "yes" if a["qualifies"] else "no",
                  "threshold": "" if np.isnan(a["threshold"]) else round(a["threshold"], 1),
                  "app_ha": round(a["app_cells"] * CELL * CELL / 1e4, 1)})
    print(name, {k: table[-1][k] for k in table[-1]})
    print("   without the plain:", KIND[int(b["base_kind"])], round(b["base"], 1), round(b["height"], 1),
          round(b["mean_slope"], 1), bool(b["qualifies"]), round(b["area_ha"], 1), "ha")
figs.write_csv(data / "synthetic-hills.csv", table)

shade = rd("syn_shade.tif")
app = rd("syn_app.tif")
app_np = rd("syn_app_noplain.tif")
print("APP with plain", int(app.sum()), "cells; without", int(app_np.sum()))


def draw(png, under, layers, labels=None, width=760, legend=None, title=None):
    """The hillshade with layers [(mask, colour, alpha)] and labelled points [(row, col, text)]."""
    h, w = under.shape
    dpi = 150
    win = width / dpi
    mapw = win * (0.78 if legend else 1.0)
    fig = plt.figure(figsize=(win, mapw * h / w + (0.35 if title else 0.05)), dpi=dpi)
    top = 1 - (0.32 / (mapw * h / w + 0.35)) if title else 1.0
    ax = fig.add_axes([0, 0, 0.78 if legend else 1.0, top])
    ax.set_axis_off()
    ax.imshow(under, cmap="gray", vmin=0, vmax=255, interpolation="nearest")
    for m, col, al in layers:
        rgba = np.zeros(m.shape + (4,))
        rgba[m] = mcolors.to_rgba(col, al)
        ax.imshow(rgba, interpolation="nearest")
    for r, c, t in labels or []:
        ax.text(c, r, t, ha="center", va="center", fontsize=10, fontweight="bold", color=figs.INK,
                bbox=dict(boxstyle="circle,pad=0.18", fc="white", ec=figs.INK, lw=0.6))
    if legend:
        hs = [Patch(facecolor=c, edgecolor="#00000033", label=n) for n, c in legend]
        fig.legend(handles=hs, loc="center left", bbox_to_anchor=(0.79, 0.5), fontsize=9.5, handlelength=1.1,
                   handleheight=1.1, labelspacing=0.55)
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.04)
    plt.close(fig)
    figs._shrink(png)


labels = [(r0, c0 - 0, n) for n, (r0, c0, _, _) in HILLS.items()]
lab_off = [(r0 + 14 if n in "AB" else r0 + 12, c0, n) for n, (r0, c0, _, _) in HILLS.items()]
pl = plain.astype(bool)
draw(out / "synthetic.png", shade, [(pl, PLAIN, 0.4), (app.filled(0) == 1, APP, 0.85)], lab_off,
     legend=[("Plain and river (input plain)", PLAIN), ("Hilltop APP", APP)])
draw(out / "synthetic-app.png", shade, [(pl, PLAIN, 0.4), (app.filled(0) == 1, APP, 0.85)], lab_off, width=620)
draw(out / "synthetic-app-noplain.png", shade, [(app_np.filled(0) == 1, APP, 0.85)], lab_off, width=620)

# the elevation of the synthetic landscape
fig = plt.figure(figsize=(760 / 150, 760 / 150 * 0.78 * NY / NX + 0.05), dpi=150)
ax = fig.add_axes([0, 0, 0.78, 1])
ax.set_axis_off()
ax.imshow(shade, cmap="gray", vmin=0, vmax=255, interpolation="nearest")
im = ax.imshow(dem, cmap="terrain", vmin=560, vmax=1150, alpha=0.6, interpolation="nearest")
for r, c, t in lab_off:
    ax.text(c, r, t, ha="center", va="center", fontsize=10, fontweight="bold", color=figs.INK,
            bbox=dict(boxstyle="circle,pad=0.18", fc="white", ec=figs.INK, lw=0.6))
cax = fig.add_axes([0.81, 0.12, 0.03, 0.76])
cb = fig.colorbar(im, cax=cax)
cb.outline.set_visible(False)
cb.ax.grid(False)
cb.ax.tick_params(labelsize=9, color=figs.RULE)
cb.set_label("Elevation (m)", fontsize=9.5, color=figs.INK2)
fig.savefig(out / "synthetic-dem.png", dpi=150, bbox_inches="tight", pad_inches=0.04)
plt.close(fig)
figs._shrink(out / "synthetic-dem.png")

# ------------------------------------------------------------------ the profile through the range (row 55)
row = HILLS["A"][0]
z = dem[row].astype(float)
d = np.arange(NX) * CELL / 1000
fig, ax = plt.subplots(figsize=(7.4, 3.4), dpi=150)
lo, hi = 0, 110
ax.fill_between(d[lo:hi], z[lo:hi], 560, color="#e9e4d8")
ax.plot(d[lo:hi], z[lo:hi], color=figs.INK, lw=1.3)
lab = rd("syn_hill.tif").filled(-1)[row].astype(int)
thr_of = {int(h["hill"]): h["threshold"] for h in syn}
thr = np.array([thr_of.get(int(v), np.nan) if v >= 0 else np.nan for v in lab])
am = lab >= 0
ax.fill_between(d[lo:hi], z[lo:hi], np.where(am, thr, z)[lo:hi], where=am[lo:hi], color=APP, alpha=0.8,
                interpolate=True, lw=0)
A, B = nearest(syn, *HILLS["A"][:2]), nearest(syn, *HILLS["B"][:2])
for hh, col, nm in ((A, figs.INK, "A"), (B, figs.WATER, "B")):
    xs = (hh["x"] - X0) / 1000
    ax.plot([xs - 0.7, xs + 0.7], [hh["base"]] * 2, color=col, lw=0.9, ls=(0, (4, 3)))
    ax.plot([xs - 0.45, xs + 0.45], [hh["threshold"]] * 2, color=APP, lw=1.2)
    ax.annotate("", (xs, hh["base"]), (xs, hh["top"]), arrowprops=dict(arrowstyle="<->", color=col, lw=0.9))
    ax.text(xs + 0.04, (hh["base"] + hh["top"]) / 2, f"{nm}: {hh['height']:.0f} m", color=col, fontsize=9)
bx = (A["base_x"] - X0) / 1000
ax.text((A["x"] - X0) / 1000 - 0.95, A["base"] + 4, f"base of A: the plain, {A['base']:.0f} m", fontsize=8.5,
        color=figs.INK, va="bottom")
ax.text((B["base_x"] - X0) / 1000 + 0.05, B["base"] - 6,
        f"base of B: the saddle, {B['base']:.0f} m", fontsize=8.5, color=figs.WATER, va="top")
ax.plot([(B["base_x"] - X0) / 1000], [B["base"]], "o", color=figs.WATER, ms=4)
ax.text((A["x"] - X0) / 1000 - 0.75, A["threshold"] + 6, "2/3 of the height", fontsize=8.5, color=APP)
ax.set_xlabel("Distance along the row through the two peaks (km)")
ax.set_ylabel("Elevation (m)")
ax.set_ylim(560, 1130)
ax.set_xlim(d[lo], d[hi - 1])
fig.tight_layout()
fig.savefig(out / "profile.png", dpi=150)
plt.close(fig)
figs._shrink(out / "profile.png")
print("profile saddle cell", B["base_x"], B["base_y"], "row", (Y0 - B["base_y"]) / CELL - 0.5)

# ------------------------------------------------------------------ the merge tree, as a falling water level
fig, axes = plt.subplots(1, 3, figsize=(7.6, 2.3), dpi=150, sharey=True)
levels = [B["top"] - 40, B["base"] - 1, A["base"] - 1]
titles = ["Two summits, two islands", "The islands join at the saddle", "A's island reaches the plain"]
for ax, lev, t in zip(axes, levels, titles):
    zz = z[lo:hi]
    ax.fill_between(d[lo:hi], zz, 560, color="#e9e4d8")
    ax.plot(d[lo:hi], zz, color=figs.INK, lw=1.0)
    ax.fill_between(d[lo:hi], np.maximum(zz, lev), 560, where=zz >= lev, color=figs.FOREST, alpha=0.55,
                    step="mid", lw=0)
    ax.axhline(lev, color=figs.WATER, lw=1.0)
    ax.set_title(t, fontsize=9.5, loc="left")
    ax.set_xticks([])
    ax.set_ylim(560, 1130)
axes[0].set_ylabel("Elevation (m)")
fig.tight_layout()
fig.savefig(out / "sweep.png", dpi=150)
plt.close(fig)
figs._shrink(out / "sweep.png")

# ------------------------------------------------------------------ Oregon: the real relief
HEAD = '''format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "elevation.tif" }
'''
RES = [30, 60, 90, 120]
SLOPES = [15, 17.5, 20, 22.5, 25, 27.5, 30]
model = HEAD
writes = {}
for r in RES:
    model += f'''
[[nodes]]
id = "e{r}"
op = "raster.reproject"
inputs = {{ raster = "dem.raster" }}
params = {{ crs = "EPSG:32610", resolution = {r}, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }}

[[nodes]]
id = "sh{r}"
op = "raster.terrain"
inputs = {{ raster = "e{r}.raster" }}
params = {{ measure = "hillshade" }}

[[nodes]]
id = "h{r}"
op = "surface.hilltop"
inputs = {{ dem = "e{r}.raster" }}
'''
    writes.update({f"sh{r}.raster": f"or_shade_{r}.tif", f"h{r}.app": f"or_app_{r}.tif",
                   f"h{r}.hills": f"or_hills_{r}.csv"})
model += '''
[[nodes]]
id = "conama"
op = "surface.hilltop"
inputs = { dem = "e30.raster" }
params = { min_height = 50, min_slope = 16.7, slope = "line" }

[[nodes]]
id = "line"
op = "surface.hilltop"
inputs = { dem = "e30.raster" }
params = { slope = "line" }

[[nodes]]
id = "slope"
op = "surface.terrain"
inputs = { map = "e30.raster" }
params = { v = "slope" }
'''
writes.update({"conama.app": "or_app_conama.tif", "conama.hills": "or_hills_conama.csv",
               "line.hills": "or_hills_line.csv", "slope.map": "or_slope.tif", "e30.raster": "or_dem.tif"})
for s in SLOPES:
    k = str(s).replace(".", "_")
    model += f'''
[[nodes]]
id = "m{k}"
op = "surface.hilltop"
inputs = {{ dem = "e30.raster" }}
params = {{ min_slope = {s} }}
'''
    writes[f"m{k}.hills"] = f"or_hills_m{k}.csv"
figs.run(figs.add_writes(model, writes), work, name="oregon.toml")

or_shade = rd("or_shade_30.tif")
or_app = rd("or_app_30.tif")
hills30 = hills_of("or_hills_30.csv")
q = [h for h in hills30 if h["qualifies"]]
print("Oregon 30 m:", len(hills30), "summits;", len(q), "qualify")
for h in q:
    print("  ", {k: round(v, 2) for k, v in h.items()})
top = hills30[0]
print("  highest:", {k: round(v, 2) for k, v in top.items()})
big = [h for h in hills30 if h["height"] >= 100]
print("  summits at least 100 m high:", len(big))
for h in big:
    print("     ", round(h["top"], 1), KIND[int(h["base_kind"])], round(h["height"], 1), round(h["mean_slope"], 1),
          round(h["line_slope"], 1), bool(h["qualifies"]))
figs.write_csv(data / "oregon-hills.csv", [
    {"hill": int(h["hill"]), "top": round(h["top"], 1), "base": round(h["base"], 1),
     "base_kind": KIND[int(h["base_kind"])], "height": round(h["height"], 1), "mean_slope": round(h["mean_slope"], 1),
     "line_slope": round(h["line_slope"], 1), "area_ha": round(h["area_ha"], 1),
     "qualifies": "yes" if h["qualifies"] else "no"} for h in big])
print("  line slope rule:", sum(h["qualifies"] for h in hills_of("or_hills_line.csv")), "qualify")

# the summit and base points of the hills of at least 100 m
T = figs.read(work / "or_app_30.tif")[1]["transform"]


def rc(x, y, t=T):
    return (y - t.f) / t.e - 0.5, (x - t.c) / t.a - 0.5


def draw_or(png, appm, shade_, pts=None, width=560, legend=None):
    lay = [(appm.filled(0) == 1, APP, 0.9)]
    h, w = shade_.shape
    dpi = 150
    win = width / dpi
    mapw = win * (0.66 if legend else 1.0)
    fig = plt.figure(figsize=(win, mapw * h / w + 0.05), dpi=dpi)
    ax = fig.add_axes([0, 0, 0.66 if legend else 1.0, 1])
    ax.set_axis_off()
    ax.imshow(shade_, cmap="gray", vmin=0, vmax=255, interpolation="nearest")
    for m, col, al in lay:
        rgba = np.zeros(m.shape + (4,))
        rgba[m] = mcolors.to_rgba(col, al)
        ax.imshow(rgba, interpolation="nearest")
    for (r, c, mk, col) in pts or []:
        ax.plot(c, r, mk, color=col, ms=5, mec="white", mew=0.6)
    if legend:
        fig.legend(handles=legend, loc="center left", bbox_to_anchor=(0.67, 0.5), fontsize=9, handlelength=1.1,
                   labelspacing=0.6)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.04)
    plt.close(fig)
    figs._shrink(png)


from matplotlib.lines import Line2D  # noqa: E402

pts = []
for h in big:
    r, c = rc(h["x"], h["y"])
    pts.append((r, c, "^", figs.FOREST if not h["qualifies"] else APP))
    if int(h["base_kind"]) == 0:
        r2, c2 = rc(h["base_x"], h["base_y"])
        pts.append((r2, c2, "o", figs.WATER))
leg = [Line2D([], [], marker="^", ls="", color=APP, mec="white", label="Summit of a hill that qualifies"),
       Line2D([], [], marker="^", ls="", color=figs.FOREST, mec="white", label="Summit ≥ 100 m that does not"),
       Line2D([], [], marker="o", ls="", color=figs.WATER, mec="white", label="Its saddle (base)"),
       Patch(facecolor=APP, label="Hilltop APP")]
draw_or(out / "oregon.png", or_app, or_shade, pts, width=760, legend=leg)

# the law and CONAMA's thresholds, side by side (same framing, no legend)
conama = hills_of("or_hills_conama.csv")
qc = [h for h in conama if h["qualifies"]]
app_c = rd("or_app_conama.tif")
print("CONAMA thresholds:", len(qc), "hills;", int(app_c.sum()), "cells,", int(app_c.sum()) * 0.09, "ha;",
      "law:", int(or_app.sum()), "cells")
draw_or(out / "oregon-law.png", or_app, or_shade, width=520)
draw_or(out / "oregon-conama.png", app_c, or_shade, width=520)

# the cell size
frames = []
for r in RES:
    a = rd(f"or_app_{r}.tif")
    hs = hills_of(f"or_hills_{r}.csv")
    qq = [h for h in hs if h["qualifies"]]
    s100 = [h for h in hs if h["height"] >= 100]
    draw_or(out / f"res_{r}.png", a, rd(f"or_shade_{r}.tif"), width=470)
    area = float(a.sum()) * r * r / 1e4
    frames.append((r, len(hs), len(s100), len(qq), area, [round(h["mean_slope"], 1) for h in s100]))
    print(f"res {r}: {len(hs)} summits, {len(s100)} of 100 m, {len(qq)} qualify, APP {area:.1f} ha;",
          "slopes of the 100 m ones", sorted([round(h['mean_slope'], 1) for h in s100], reverse=True)[:8])
figs.write_csv(data / "resolution.csv", [{"cell_m": r, "Summits": n, "Summits of 100 m or more": n100,
                                          "Hills that qualify": nq, "APP (ha)": round(a, 1),
                                          "Steepest mean slope (degrees)": max(sl_)}
                                         for r, n, n100, nq, a, sl_ in frames])

# the slope threshold
rows = []
for s in SLOPES:
    k = str(s).replace(".", "_")
    hs = hills_of(f"or_hills_m{k}.csv")
    qq = [h for h in hs if h["qualifies"]]
    rows.append({"min_slope": s, "Hills that qualify": len(qq),
                 "APP (ha)": round(sum(h["app_cells"] for h in qq) * 0.09, 1)})
figs.write_csv(data / "min-slope.csv", rows)
print("min_slope", rows)

# how the slope of the DEM changes with the cell size
sl = rd("or_slope.tif")
print("mean slope of the 30 m DEM", float(sl.mean()), "cells over 25 deg", float((sl > 25).mean() * 100))

# ------------------------------------------------------------------ a closer look at the hill that qualifies
zq = rd("or_dem.tif")
h = q[0]
r0, c0 = (int(round(v)) for v in rc(h["x"], h["y"]))
rb, cb_ = (int(round(v)) for v in rc(h["base_x"], h["base_y"]))
R0, R1, C0, C1 = max(r0 - 30, 0), r0 + 30, 0, 60
sub = zq[R0:R1, C0:C1]
fig, ax = plt.subplots(figsize=(5.4, 5.4), dpi=150)
ax.imshow(or_shade[R0:R1, C0:C1], cmap="gray", vmin=0, vmax=255, interpolation="nearest")
rgba = np.zeros(sub.shape + (4,))
rgba[or_app.filled(0)[R0:R1, C0:C1] == 1] = mcolors.to_rgba(APP, 0.85)
ax.imshow(rgba, interpolation="nearest")
cs = ax.contour(sub, levels=np.arange(800, 1100, 20), colors=figs.INK2, linewidths=0.5)
ax.clabel(cs, fontsize=7, fmt="%d")
ax.contour(sub, levels=[h["threshold"]], colors=[APP], linewidths=1.6)
ax.contour(sub, levels=[h["base"]], colors=[figs.WATER], linewidths=1.2, linestyles="dashed")
ax.plot(c0 - C0, r0 - R0, "^", color=APP, ms=8, mec="white")
ax.plot(cb_ - C0, rb - R0, "o", color=figs.WATER, ms=7, mec="white")
ax.set_axis_off()
ax.text(c0 - C0 + 1.5, r0 - R0 - 1.5, f"summit {h['top']:.0f} m", fontsize=8.5, color=figs.INK,
        bbox=dict(fc="white", ec="none", alpha=0.8, pad=1))
ax.text(cb_ - C0 + 1.5, rb - R0 + 2.5, f"saddle {h['base']:.0f} m", fontsize=8.5, color=figs.WATER,
        bbox=dict(fc="white", ec="none", alpha=0.8, pad=1))
fig.tight_layout(pad=0.2)
fig.savefig(out / "oregon-zoom.png", dpi=150)
plt.close(fig)
figs._shrink(out / "oregon-zoom.png")
print("zoom: summit at column", c0, "row", r0, "; saddle at column", cb_, "row", rb)
