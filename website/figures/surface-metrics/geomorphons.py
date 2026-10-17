"""Figures of the tutorial "Landforms with geomorphons" (surface-metrics/geomorphons).

The SRTM elevation of south-western Oregon shipped with landschaft (the DEM of the tutorial "Terrain from an
elevation model"), reprojected to UTM zone 10N in cells of 30 m, and its geomorphons computed by Terraflux at
several search radii, flatness thresholds and options; the patterns, measures and shares they give; and a check
of the forms against GRASS GIS's r.geomorphon on landschaft's reference DEMs.

    python website/figures/surface-metrics/geomorphons.py
"""

import json
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch, Polygon, Rectangle  # noqa: E402

SLUG = "surface-metrics/geomorphons"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
LS = figs.ROOT / "extern/landschaft"
shutil.copy(LS / "src/landschaft/surface/data/orelevation.tif", work / "elevation.tif")

# the landforms with r.geomorphon's colours (as the application draws them: crates/ops/src/landscape.rs)
FORMS = {1: ("Flat", "#dcdcdc"), 2: ("Peak", "#380000"), 3: ("Ridge", "#c80000"), 4: ("Shoulder", "#ff5014"),
         5: ("Spur", "#fad23c"), 6: ("Slope", "#ffff3c"), 7: ("Hollow", "#b4e614"), 8: ("Footslope", "#3cfa96"),
         9: ("Valley", "#0000ff"), 10: ("Pit", "#000038")}

HEAD = '''format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = { path = "elevation.tif" }

[[nodes]]
id = "elevation"
op = "raster.reproject"
inputs = { raster = "dem.raster" }
params = { crs = "EPSG:32610", resolution = 30, extent = [500010, 4760850, 508110, 4771920], resampling = "bilinear" }

[[nodes]]
id = "shade"
op = "raster.terrain"
inputs = { raster = "elevation.raster" }
params = { measure = "hillshade" }
'''


def node(id_, params):
    return f'''
[[nodes]]
id = "{id_}"
op = "surface.geomorphons"
inputs = {{ dem = "elevation.raster" }}
params = {{ {params} }}
'''


# ------------------------------------------------------------------ the tutorial's model, every output written
SEARCHES = [3, 5, 10, 20, 40, 80]
FLATS = [1, 3, 6]
model = HEAD + node("forms", "search = 20, measures = true")
writes = {"elevation.raster": "elevation_utm.tif", "shade.raster": "shade.tif"}
for k in ["forms", "ternary", "positive", "negative", "intensity", "exposition", "range", "variance",
          "elongation", "azimuth", "extend", "width"]:
    writes[f"forms.{k}"] = f"g20_{k}.tif"
writes["forms.share"] = "share.csv"
for s in SEARCHES:
    model += node(f"s{s}", f"search = {s}")
    writes[f"s{s}.forms"] = f"forms_{s}.tif"
for f in FLATS:
    model += node(f"f{f}", f"search = 20, flat = {f}")
    writes[f"f{f}.forms"] = f"flat_{f}.tif"
# options compared with the default at a search of 20 (or 40) cells
model += node("skip3", "search = 20, skip = 3")
model += node("v2", 'search = 20, comparison = "anglev2"')
model += node("meters", 'search = 600, units = "map"')
model += node("ext40", "search = 40, extended = true")
model += node("dist", "search = 20, dist = 10")
for k in ["skip3", "v2", "meters", "ext40", "dist"]:
    writes[f"{k}.forms"] = f"opt_{k}.tif"
figs.run(figs.add_writes(model, writes), work)


def rd(name):
    return figs.read(work / name)[0]


shade = rd("shade.tif")
forms = rd("g20_forms.tif")
print("grid", forms.shape, "cells with a form", forms.count())

# ------------------------------------------------------------------ drawing


def draw_forms(png, arr, title=None, width=560, legend=True, alpha=0.82, counts=True):
    h, w = arr.shape
    dpi = 150
    win = width / dpi
    mapw = win * (0.70 if legend else 1.0)
    fig = plt.figure(figsize=(win, mapw * h / w + (0.35 if title else 0.08)), dpi=dpi)
    top = 1 - (0.32 / (mapw * h / w + 0.35)) if title else 1.0
    ax = fig.add_axes([0, 0, 0.70 if legend else 1.0, top])
    ax.set_axis_off()
    ax.imshow(shade, cmap="gray", vmin=0, vmax=255, interpolation="nearest")
    f = arr.filled(-1)
    rgba = np.zeros(arr.shape + (4,))
    for v, (_, c) in FORMS.items():
        rgba[f == v] = mcolors.to_rgba(c)
    rgba[..., 3] *= alpha
    ax.imshow(rgba, interpolation="nearest")
    if legend:
        total = arr.count()
        hs = [Patch(facecolor=c, edgecolor="#00000033",
                    label=f"{n}  {(f == v).sum() / total * 100:.1f}%" if counts else n)
              for v, (n, c) in FORMS.items()]
        fig.legend(handles=hs, loc="center left", bbox_to_anchor=(0.71, 0.5), fontsize=9.5, handlelength=1.1,
                   handleheight=1.1, labelspacing=0.5)
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.05)
    plt.close(fig)
    figs._shrink(png)


def draw_cont(png, arr, cmap, unit, vmin=None, vmax=None, diverging=False, width=430):
    h, w = arr.shape
    dpi = 150
    win = width / dpi
    mapw = win * 0.78
    fig = plt.figure(figsize=(win, mapw * h / w + 0.08), dpi=dpi)
    ax = fig.add_axes([0, 0, 0.78, 1])
    ax.set_axis_off()
    ax.imshow(shade, cmap="gray", vmin=0, vmax=255, interpolation="nearest")
    if diverging:
        m = max(abs(np.nanpercentile(arr.compressed(), 1)), abs(np.nanpercentile(arr.compressed(), 99)))
        norm = mcolors.TwoSlopeNorm(vmin=-m, vcenter=0, vmax=m)
    else:
        norm = mcolors.Normalize(vmin=vmin if vmin is not None else np.nanpercentile(arr.compressed(), 1),
                                 vmax=vmax if vmax is not None else np.nanpercentile(arr.compressed(), 99))
    cm = plt.get_cmap(cmap).copy()
    cm.set_bad((1, 1, 1, 0))
    im = ax.imshow(arr, cmap=cm, norm=norm, alpha=0.8, interpolation="nearest")
    cax = fig.add_axes([0.81, 0.12, 0.04, 0.74])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.ax.grid(False)
    cb.ax.tick_params(labelsize=8.5, color=figs.RULE)
    cb.set_label(unit, fontsize=9, color=figs.INK2)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.05)
    plt.close(fig)
    figs._shrink(png)


def shares(arr):
    f = arr.filled(-1)
    n = arr.count()
    return {v: (f == v).sum() / n * 100 for v in FORMS}


# ------------------------------------------------------------------ the main map and the share table
draw_forms(out / "forms.png", forms, width=760)
rows = figs.read_csv(work / "output" / "share.csv")
table = [{"form": int(float(r["form"])), "name": FORMS[int(float(r["form"]))][0], "cells": int(float(r["cells"])),
          "share (%)": round(float(r["share"]) * 100, 2)} for r in rows]
figs.write_csv(data / "share.csv", table)
print("share", [(t["name"], t["share (%)"]) for t in table])

# ------------------------------------------------------------------ the lookup table of the forms
# r.geomorphon's table (landschaft's src/cpp/surf/geomorphons.cpp): rows the number of lower directions (-),
# columns the number of higher ones (+); 0 an impossible count (more than 8 in all)
FL, PK, RI, SH, SP, SL, HL, FS, VL, PT, ER = 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 0
TABLE = [
    [FL, FL, FL, FS, FS, VL, VL, VL, PT],
    [FL, FL, FS, FS, FS, VL, VL, VL, ER],
    [FL, SH, SL, SL, HL, HL, VL, ER, ER],
    [SH, SH, SL, SL, SL, HL, ER, ER, ER],
    [SH, SH, SP, SL, SL, ER, ER, ER, ER],
    [RI, RI, SP, SP, ER, ER, ER, ER, ER],
    [RI, RI, RI, ER, ER, ER, ER, ER, ER],
    [RI, RI, ER, ER, ER, ER, ER, ER, ER],
    [PK, ER, ER, ER, ER, ER, ER, ER, ER],
]
fig, ax = plt.subplots(figsize=(4.6, 4.1), dpi=150)
ax.set_xlim(-0.5, 8.5)
ax.set_ylim(8.5, -0.5)
ax.grid(False)
for m in range(9):
    for p in range(9):
        v = TABLE[m][p]
        if v == ER:
            continue
        name, col = FORMS[v]
        ax.add_patch(Rectangle((p - 0.5, m - 0.5), 1, 1, facecolor=col, edgecolor="white", lw=1.2))
        dark = v in (2, 3, 9, 10)
        ax.text(p, m, name[:2] if name != "Footslope" else "Fs", ha="center", va="center", fontsize=7.5,
                color="white" if dark else figs.INK)
ax.set_xticks(range(9))
ax.set_yticks(range(9))
ax.set_xlabel("Directions where the terrain is higher (+)")
ax.set_ylabel("Directions where the terrain is lower (−)")
ax.xaxis.set_label_position("top")
ax.xaxis.tick_top()
for s in ax.spines.values():
    s.set_visible(False)
hs = [Patch(facecolor=c, edgecolor="#00000033", label=f"{n[:2] if n != 'Footslope' else 'Fs'}  {n}")
      for _, (n, c) in FORMS.items()]
ax.legend(handles=hs, loc="lower right", fontsize=7, handlelength=1, ncol=2, bbox_to_anchor=(1.02, -0.02))
fig.tight_layout()
fig.savefig(out / "lookup.png", dpi=150)
plt.close(fig)
figs._shrink(out / "lookup.png")

# ------------------------------------------------------------------ lines of sight along one direction
z = rd("elevation_utm.tif").filled(np.nan)
L = 20
d = np.arange(L + 1) * 30.0


def sight(r, c):
    p = z[r, c:c + L + 1]
    return p, np.degrees(np.arctan2(p[1:] - p[0], d[1:]))


# a cell of the slope class whose line of sight towards the east first falls into a valley, then rises above it
# (both angles clearly away from the horizon): the first such cell from row 150
row = col = None
for r in range(150, 300):
    for c in range(30, 200):
        if forms[r, c] is np.ma.masked or forms[r, c] != 6:
            continue
        p, a = sight(r, c)
        if a.max() > 12 and a.min() < -8 and abs(a.max()) > abs(a.min()) + 3 and np.argmin(a) < np.argmax(a):
            row, col = r, c
            break
    if row is not None:
        break
prof, ang = sight(row, col)
print("line of sight from row", row, "column", col)
iz, inad = int(np.argmax(ang)) + 1, int(np.argmin(ang)) + 1
fig, ax = plt.subplots(figsize=(7.2, 3.3), dpi=150)
ax.fill_between(d, prof, prof.min() - 20, color="#e9e4d8")
ax.plot(d, prof, color=figs.INK, lw=1.4)
ax.plot([0, d[iz]], [prof[0], prof[iz]], color=figs.EMBER, lw=1.6)
ax.plot([0, d[inad]], [prof[0], prof[inad]], color=figs.WATER, lw=1.6)
ax.axhline(prof[0], color=figs.INK2, lw=0.8, ls=(0, (3, 3)))
ax.plot([0], [prof[0]], "o", color=figs.INK, ms=6)
ax.annotate(f"steepest line of sight up: {ang[iz - 1]:+.1f}°", (d[iz] * 0.6, prof[0] + (prof[iz] - prof[0]) * 0.6),
            xytext=(d[iz] * 0.6 - 10, prof.max() + 8), color=figs.EMBER, fontsize=9,
            arrowprops=dict(arrowstyle="-", color=figs.EMBER, lw=0.7))
ax.annotate(f"steepest line of sight down: {ang[inad - 1]:+.1f}°", (d[inad] * 0.6, prof[0] + (prof[inad] - prof[0]) * 0.6),
            xytext=(d[inad] + 60, prof.min() - 14), color=figs.WATER, fontsize=9,
            arrowprops=dict(arrowstyle="-", color=figs.WATER, lw=0.7))
ax.axvline(L * 30, color=figs.INK2, lw=0.8)
ax.text(L * 30 - 8, prof.min() - 12, "search radius\n(20 cells, 600 m)", ha="right", va="bottom", fontsize=8.5,
        color=figs.INK2)
ax.text(4, prof[0] + 4, "the cell", fontsize=9, color=figs.INK)
ax.set_xlabel("Distance from the cell towards the east (m)")
ax.set_ylabel("Elevation (m)")
ax.set_ylim(prof.min() - 22, prof.max() + 22)
fig.tight_layout()
fig.savefig(out / "line-of-sight.png", dpi=150)
plt.close(fig)
figs._shrink(out / "line-of-sight.png")
print("line of sight: up", ang[iz - 1], "down", ang[inad - 1], "form there", forms[row, col])

# ------------------------------------------------------------------ the ternary patterns: commonest per form


def signs(code):
    """The 8 signs of a ternary code (digits 0 lower, 1 level, 2 higher), direction 0 first."""
    out_ = []
    for _ in range(8):
        out_.append(int(code % 3) - 1)
        code //= 3
    return out_


tern = rd("g20_ternary.tif")
fv, tv = forms.filled(0).astype(int).ravel(), tern.filled(-1).astype(int).ravel()
ok = (fv > 0) & (tv >= 0)
distinct = len(np.unique(tv[ok]))
print("distinct ternary codes", distinct)
patterns = []
for v in range(1, 11):
    sel = tv[ok & (fv == v)]
    if sel.size == 0:
        continue
    codes, counts = np.unique(sel, return_counts=True)
    i = int(np.argmax(counts))
    patterns.append((v, int(codes[i]), int(counts[i]), int(sel.size)))
# r.geomorphon's directions: 0 NE, 1 N, 2 NW, 3 W, 4 SW, 5 S, 6 SE, 7 E
ANG = {0: 45, 1: 90, 2: 135, 3: 180, 4: 225, 5: 270, 6: 315, 7: 0}
n = len(patterns)
ncol = 5
fig, axes = plt.subplots(2, ncol, figsize=(5.6, 2.5), dpi=150)
axes = axes.ravel()
for ax in axes[n:]:
    ax.set_axis_off()
for ax, (v, code, cnt, tot) in zip(axes, patterns):
    ax.set_xlim(-1.35, 1.35)
    ax.set_ylim(-1.55, 1.35)
    ax.set_aspect("equal")
    ax.set_axis_off()
    ax.add_patch(Rectangle((-0.22, -0.22), 0.44, 0.44, facecolor=FORMS[v][1], edgecolor=figs.INK, lw=0.6))
    for i, s in enumerate(signs(code)):
        a = np.radians(ANG[i])
        x, y = 0.95 * np.cos(a), 0.95 * np.sin(a)
        col = {1: figs.EMBER, -1: figs.WATER, 0: "#9aa39d"}[s]
        ax.plot([0.25 * np.cos(a), 0.7 * np.cos(a)], [0.25 * np.sin(a), 0.7 * np.sin(a)], color=col, lw=1.2)
        ax.text(x, y, {1: "+", -1: "−", 0: "0"}[s], ha="center", va="center", fontsize=10, color=col,
                fontweight="bold")
    ax.text(0, -1.42, FORMS[v][0], ha="center", va="center", fontsize=8.5, color=figs.INK)
fig.subplots_adjust(left=0.01, right=0.99, top=0.99, bottom=0.01, wspace=0.05)
fig.savefig(out / "patterns.png", dpi=150)
plt.close(fig)
figs._shrink(out / "patterns.png")
print("patterns", patterns)

# the commonest patterns of the map, as a table
codes, counts = np.unique(tv[ok], return_counts=True)
order = np.argsort(-counts)[:10]
prow = []
for i in order:
    c = int(codes[i])
    fsel = fv[ok & (tv == c)]
    sg = signs(c)
    prow.append({"code": c, "signs": " ".join({1: "+", -1: "−", 0: "0"}[s] for s in sg),
                 "higher": sg.count(1), "lower": sg.count(-1), "form": FORMS[int(np.bincount(fsel).argmax())][0],
                 "share (%)": round(counts[i] / ok.sum() * 100, 2)})
figs.write_csv(data / "patterns.csv", prow)
print("top patterns", prow[:3])

# ------------------------------------------------------------------ scale: the search radius
frames, chart = [], []
for s in SEARCHES:
    a = rd(f"forms_{s}.tif")
    draw_forms(out / f"forms_{s}.png", a, width=470, legend=False)
    sh = shares(a)
    frames.append((s, sh, a.count() / a.size * 100))
    chart.append({"search_m": s * 30, **{FORMS[v][0]: round(sh[v], 2) for v in (2, 3, 5, 6, 7, 9, 10)},
                  "Flat, shoulder, footslope": round(sh[1] + sh[4] + sh[8], 2)})
figs.write_csv(data / "search.csv", chart)
for s, sh, cov in frames:
    print(f"search {s}: " + ", ".join(f"{FORMS[v][0]} {sh[v]:.1f}" for v in FORMS) + f"; with a form {cov:.1f}%")

# the legend, alone, for the slider
fig = plt.figure(figsize=(7.2, 0.45), dpi=150)
hs = [Patch(facecolor=c, edgecolor="#00000033", label=n) for _, (n, c) in FORMS.items()]
fig.legend(handles=hs, loc="center", ncol=10, fontsize=8.5, handlelength=1.0, handleheight=1.0, columnspacing=0.9,
           handletextpad=0.4)
fig.savefig(out / "legend.png", dpi=150, bbox_inches="tight", pad_inches=0.03)
plt.close(fig)
figs._shrink(out / "legend.png")

# ------------------------------------------------------------------ flatness threshold
for f in FLATS:
    a = rd(f"flat_{f}.tif")
    draw_forms(out / f"flat_{f}.png", a, width=360, legend=False)
    sh = shares(a)
    print(f"flat {f}: flat {sh[1]:.1f}, slope {sh[6]:.1f}, ridge {sh[3]:.1f}, valley {sh[9]:.1f}, "
          f"shoulder {sh[4]:.1f}, footslope {sh[8]:.1f}")

# ------------------------------------------------------------------ the options, compared with the default
base20 = forms.filled(0)
for k in ["skip3", "v2", "meters", "dist"]:
    a = rd(f"opt_{k}.tif")
    both = (~np.ma.getmaskarray(a)) & (~np.ma.getmaskarray(forms))
    diff = (a.filled(0) != base20) & both
    print(f"option {k}: {diff.sum()} of {both.sum()} cells differ ({diff.sum() / both.sum() * 100:.2f}%); "
          + ", ".join(f"{FORMS[v][0]} {shares(a)[v]:.1f}" for v in FORMS))
f40 = rd("forms_40.tif")
e40 = rd("opt_ext40.tif")
both = (~np.ma.getmaskarray(f40)) & (~np.ma.getmaskarray(e40))
diff = (f40.filled(0) != e40.filled(0)) & both
print(f"extended at 40: {diff.sum()} of {both.sum()} differ ({diff.sum() / both.sum() * 100:.2f}%)")
for v in FORMS:
    print("   ", FORMS[v][0], f"{shares(f40)[v]:.2f} -> {shares(e40)[v]:.2f}")

# ------------------------------------------------------------------ the measures
units = {"intensity": ("RdBu_r", "metres", True), "range": ("magma", "metres", False),
         "extend": ("viridis", "share of the search octagon", False), "elongation": ("cividis", "ratio", False)}
for k, (cm, u, dv) in units.items():
    a = rd(f"g20_{k}.tif")
    draw_cont(out / f"{k}.png", a, cm, u, diverging=dv)
    print(k, "range", float(a.min()), float(a.max()), "mean", float(a.mean()))
inten = rd("g20_intensity.tif")
for v in (2, 3, 6, 9, 10):
    m = forms.filled(0) == v
    print("intensity of", FORMS[v][0], float(inten.filled(np.nan)[m].mean()))

# ------------------------------------------------------------------ the check against GRASS GIS's r.geomorphon
import rasterio  # noqa: E402
from rasterio.transform import from_origin  # noqa: E402

meta = json.loads((LS / "tests/data/reference/surface/geomorphons.json").read_text())
ref = np.load(LS / "tests/data/reference/surface/geomorphons.npz")
check = HEAD.split("[[nodes]]")[0]
cw = {}
CASES = ["default", "search10_skip2_dist", "extended_anglev2", "ties_anglev2_distance", "holes", "map_units"]
for name in CASES:
    c = meta["cases"][name]
    zc = ref[f"{name}/dem"]
    ew, ns = c["cell"]
    x0, y0 = c["origin"]
    with rasterio.open(work / f"ref_{name}.tif", "w", driver="GTiff", height=zc.shape[0], width=zc.shape[1],
                       count=1, dtype="float32", transform=from_origin(x0, y0, ew, ns), nodata=np.nan,
                       crs="EPSG:32723") as dst:
        dst.write(zc, 1)
    p = [f"search = {c['search']}", f"skip = {c['skip']}", f"flat = {c['flat']}", f"dist = {c['dist']}",
         f'comparison = "{c["comparison"]}"']
    if c.get("meters"):
        p.append('units = "map"')
    if c.get("extended"):
        p.append("extended = true")
    check += f'''
[[nodes]]
id = "r_{name}"
op = "raster.read"
params = {{ path = "ref_{name}.tif" }}

[[nodes]]
id = "g_{name}"
op = "surface.geomorphons"
inputs = {{ dem = "r_{name}.raster" }}
params = {{ {", ".join(p)} }}
'''
    cw[f"g_{name}.forms"] = f"chk_{name}_forms.tif"
    cw[f"g_{name}.ternary"] = f"chk_{name}_ternary.tif"
figs.run(figs.add_writes(check, cw), work, name="check.toml")
tot_same = tot = 0
for name in CASES:
    for k in ("forms", "ternary"):
        mine = rd(f"chk_{name}_{k}.tif").filled(np.nan)
        theirs = ref[f"{name}/{k}"].astype(np.float64)
        # GRASS was given one more row of no value at the bottom (see landschaft's
        # tools/make_reference_geomorphons.py), so its border without forms is one row lower: compare where both
        # have a value
        valid = ~np.isnan(theirs) & ~np.isnan(mine)
        same = (mine == theirs) & valid
        print(f"GRASS {name} {k}: {same.sum()} of {valid.sum()} identical; "
              f"value in one only: {int((np.isnan(theirs) != np.isnan(mine)).sum())}")
        if k == "forms":
            tot_same += int(same.sum())
            tot += int(valid.sum())
print(f"GRASS check, forms: {tot_same} of {tot} cells identical over {len(CASES)} cases")
