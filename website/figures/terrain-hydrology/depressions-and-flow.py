"""Figures of the tutorial "Depressions, flow directions and flow accumulation" (terrain-hydrology/depressions-and-flow).

A toy elevation model of 5 x 5 cells (a valley with a pit) filled, given its D8 directions and accumulated by
Terraflux; then the SRTM elevation of south-western Oregon (reprojected to UTM 10N, 30 m) filled and breached, its
directions, and its flow accumulation by the five methods, with units and weights.

    python website/figures/terrain-hydrology/depressions-and-flow.py
"""

import json
import sys
import pathlib

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _hydro as H  # noqa: E402
from _hydro import figs, np, plt, mcolors  # noqa: E402

SLUG = "depressions-and-flow"
work, out, data = H.setup(SLUG)
numbers = {}

# ------------------------------------------------------------------ a toy valley with a pit, cell by cell
toy = np.array([[10 + 3 * abs(c - 2) + (4 - r) for c in range(5)] for r in range(5)], dtype=float)
toy[2, 2] = 9.0                      # a pit in the valley floor
H.write_tif(work / "toy.tif", toy)
TOY = '''format = "terraflux/1"

[[nodes]]
id = "toy"
op = "raster.read"
params = { path = "toy.tif" }

[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "toy.raster" }

[[nodes]]
id = "raw_d8"
op = "hydro.flow_direction"
inputs = { dem = "toy.raster" }

[[nodes]]
id = "d8"
op = "hydro.flow_direction"
inputs = { dem = "filled.map" }

[[nodes]]
id = "acc"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map" }

[[nodes]]
id = "raw_acc"
op = "hydro.flow_accumulation"
inputs = { dem = "toy.raster" }
'''
figs.run(figs.add_writes(TOY, {"filled.map": "toy_filled.tif", "d8.map": "toy_d8.tif", "acc.map": "toy_acc.tif",
                               "raw_d8.map": "toy_raw_d8.tif", "raw_acc.map": "toy_raw_acc.tif"}),
         work, name="toy.toml")
tf = H.rd(work, "toy_filled.tif").filled(np.nan)
td8 = H.rd(work, "toy_d8.tif").filled(np.nan)
tacc = H.rd(work, "toy_acc.tif").filled(np.nan)
traw = H.rd(work, "toy_raw_d8.tif").filled(np.nan)
trawacc = H.rd(work, "toy_raw_acc.tif").filled(np.nan)
numbers["toy"] = dict(dem=toy.tolist(), filled=tf.round(3).tolist(), d8=td8.tolist(), acc=tacc.tolist(),
                      raw_d8=traw.tolist(), raw_acc=trawacc.tolist())

ARROW = {1: (1, 0), 2: (1, 1), 4: (0, 1), 8: (-1, 1), 16: (-1, 0), 32: (-1, -1), 64: (0, -1), 128: (1, -1)}
fig, axes = plt.subplots(2, 3, figsize=(9.6, 6.9), dpi=150, gridspec_kw={"wspace": 0.1, "hspace": 0.22})
panels = [("Elevation model (m)", toy, "z"), ("D8 code (ESRI)", traw, "d8"), ("Accumulation (cells)", trawacc, "acc"),
          ("Filled (m)", tf, "z"), ("D8 code (ESRI)", td8, "d8"), ("Accumulation (cells)", tacc, "acc")]
cmapz = plt.get_cmap("terrain")
for i, (ax, (t, vals, kind)) in enumerate(zip(axes.ravel(), panels)):
    ax.set_xlim(0, 5)
    ax.set_ylim(5, 0)
    ax.set_aspect("equal")
    ax.set_axis_off()
    ax.set_title(("Raw: " if i < 3 else "Filled: ") + t if kind != "z" else t, loc="left", fontsize=10.5)
    for r in range(5):
        for c in range(5):
            v = vals[r, c]
            if kind == "z":
                face = cmapz((v - 9) / (21 - 9) * 0.6 + 0.22)
                txt = f"{v:g}" if v == int(v) else f"{v:.2f}"
            elif kind == "d8":
                face = "#f3f5f3"
                txt = f"{int(v)}"
            else:
                face = plt.get_cmap("Blues")(min(np.log(v) / np.log(25), 1) * 0.75 + 0.05)
                txt = f"{int(v)}"
            pit = (r, c) == (2, 2)
            ax.add_patch(plt.Rectangle((c, r), 1, 1, facecolor=face, edgecolor="white", lw=2))
            if kind == "d8" and int(v) in ARROW:
                dx, dy = ARROW[int(v)]
                ax.annotate("", xy=(c + 0.5 + dx * 0.36, r + 0.5 + dy * 0.36),
                            xytext=(c + 0.5 - dx * 0.2, r + 0.5 - dy * 0.2),
                            arrowprops=dict(arrowstyle="-|>", color=figs.WATER, lw=1.4, mutation_scale=9))
                ax.text(c + 0.2, r + 0.2, txt, ha="center", va="center", fontsize=7.5, color=figs.INK2)
            else:
                ax.text(c + 0.5, r + 0.5, txt, ha="center", va="center", fontsize=10.5,
                        color="white" if (kind == "acc" and v > 8) else figs.INK,
                        fontweight="bold" if pit or (kind == "d8") else "normal")
            if pit:
                ax.add_patch(plt.Rectangle((c, r), 1, 1, fill=False, edgecolor=figs.EMBER, lw=2.4))
fig.savefig(out / "toy.png", dpi=150, bbox_inches="tight", pad_inches=0.08)
plt.close(fig)
figs._shrink(out / "toy.png")

# ------------------------------------------------------------------ the real DEM: fill, breach, directions, accumulation
MODEL = H.HEAD + H.NDVI + '''
[[nodes]]
id = "filled"
op = "hydro.fill"
inputs = { dem = "elevation.raster" }

[[nodes]]
id = "save_filled"
op = "raster.write"
inputs = { value = "filled.map" }
params = { path = "filled.tif", type = "f64" }

[[nodes]]
id = "filled_eps"
op = "hydro.fill"
inputs = { dem = "elevation.raster" }
params = { drain_flats = true }

[[nodes]]
id = "breached"
op = "hydro.breach"
inputs = { dem = "elevation.raster" }

[[nodes]]
id = "raw_d8"
op = "hydro.flow_direction"
inputs = { dem = "elevation.raster" }

[[nodes]]
id = "raw_acc"
op = "hydro.flow_accumulation"
inputs = { dem = "elevation.raster" }

[[nodes]]
id = "directions"
op = "hydro.flow_direction"
inputs = { dem = "filled.map" }

[[nodes]]
id = "dinf_dir"
op = "hydro.flow_direction"
inputs = { dem = "filled.map" }
params = { method = "dinf" }

[[nodes]]
id = "nf_d8"
op = "hydro.flow_direction"
inputs = { dem = "filled.map" }
params = { flats = false }

[[nodes]]
id = "eps_nf"
op = "hydro.flow_direction"
inputs = { dem = "filled_eps.map" }
params = { flats = false }

[[nodes]]
id = "breach_d8"
op = "hydro.flow_direction"
inputs = { dem = "breached.map" }

[[nodes]]
id = "breach_acc"
op = "hydro.flow_accumulation"
inputs = { dem = "breached.map" }

[[nodes]]
id = "area"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map" }
params = { units = "area" }

[[nodes]]
id = "sca"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map" }
params = { units = "sca" }

[[nodes]]
id = "open"
op = "map.calc"
inputs = { n = "ndvi.raster" }
params = { expression = "if n <= 0.55 then 1 else 0" }

[[nodes]]
id = "open_acc"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.map", weights = "open.map" }

[[nodes]]
id = "open_share"
op = "map.calc"
inputs = { o = "open_acc.map", a = "accumulation.map" }
params = { expression = "if a >= 500 then 100 * o / a else null" }
'''
METHODS = ["d8", "dinf", "fd8", "quinn", "mfd"]
for m in METHODS:
    nid = "accumulation" if m == "d8" else f"acc_{m}"
    MODEL += f'''
[[nodes]]
id = "{nid}"
op = "hydro.flow_accumulation"
inputs = {{ dem = "filled.map" }}
params = {{ method = "{m}" }}
'''
writes = {"elevation.raster": "elevation_utm.tif", "shade.raster": "shade.tif",
          "filled_eps.map": "filled_eps.tif", "breached.map": "breached.tif", "raw_d8.map": "raw_d8.tif",
          "raw_acc.map": "raw_acc.tif", "directions.map": "d8.tif", "dinf_dir.map": "dinf.tif",
          "nf_d8.map": "nf_d8.tif", "eps_nf.map": "eps_nf.tif", "breach_acc.map": "breach_acc.tif", "breach_d8.map": "breach_d8.tif", "area.map": "area.tif", "sca.map": "sca.tif",
          "open.map": "open.tif", "open_acc.map": "open_acc.tif", "open_share.map": "open_share.tif"}
for m in METHODS:
    writes[("accumulation" if m == "d8" else f"acc_{m}") + ".map"] = f"acc_{m}.tif"
figs.run(figs.add_writes(MODEL, writes), work)


def rd(n):
    return H.rd(work, n)


z = rd("elevation_utm.tif")
shade = rd("shade.tif").filled(np.nan)
filled = rd("filled.tif")
breached = rd("breached.tif")
eps = rd("filled_eps.tif")
numbers["shape"] = z.shape
numbers["z"] = H.stats(z)

# fill and breach: what changed (float32 on disk: a change under 1 mm is rounding)
TOL = 1e-3
raise_ = (filled - z).filled(0)
chg = (breached - z).filled(0)
numbers["fill"] = dict(cells=int((raise_ > TOL).sum()), max=float(raise_.max()), total=float(raise_[raise_ > TOL].sum()),
                       share=float((raise_ > TOL).sum() / z.count() * 100))
numbers["breach"] = dict(changed=int((np.abs(chg) > TOL).sum()), lowered=int((chg < -TOL).sum()),
                         raised=int((chg > TOL).sum()), total=float(np.abs(chg[np.abs(chg) > TOL]).sum()),
                         deepest=float(chg.min()), highest=float(chg.max()))
numbers["eps_vs_fill"] = float(np.abs((eps - filled).filled(0)).max())

# the depressions: connected groups of raised cells
from scipy import ndimage  # noqa: E402

lab, nlab = ndimage.label(raise_ > TOL, structure=np.ones((3, 3)))
sizes = ndimage.sum(np.ones_like(raise_), lab, index=np.arange(1, nlab + 1))
numbers["depressions"] = dict(n=int(nlab), largest_cells=int(sizes.max()), single=int((sizes == 1).sum()))

# the raw DEM's sinks: cells with no lower neighbour inside the map (code 0 away from the edge)
raw_d8 = rd("raw_d8.tif").filled(-1)
inner = np.zeros_like(raw_d8, dtype=bool)
inner[1:-1, 1:-1] = True
numbers["raw_sinks_inner"] = int(((raw_d8 == 0) & inner).sum())
d8 = rd("d8.tif").filled(-1)
numbers["d8_zero"] = int((d8 == 0).sum())
numbers["d8_zero_inner"] = int(((d8 == 0) & inner).sum())
numbers["d8_counts"] = {int(k): int((d8 == k).sum()) for k in (1, 2, 4, 8, 16, 32, 64, 128)}
nf = rd("nf_d8.tif").filled(-1)
numbers["noflats_zero_inner"] = int(((nf == 0) & inner).sum())
en = rd("eps_nf.tif").filled(-1)
numbers["eps_noflats_zero_inner"] = int(((en == 0) & inner).sum())
raw_acc = rd("raw_acc.tif")
acc = {m: rd(f"acc_{m}.tif") for m in METHODS}
numbers["raw_acc_max"] = float(raw_acc.max())
numbers["acc_max"] = {m: float(acc[m].max()) for m in METHODS}
numbers["breach_acc_max"] = float(rd("breach_acc.tif").max())
area = rd("area.tif")
sca = rd("sca.tif")
imax = np.unravel_index(np.ma.argmax(acc["d8"]), z.shape)
numbers["outlet"] = dict(row=int(imax[0]), col=int(imax[1]), cells=float(acc["d8"][imax]), area=float(area[imax]),
                         sca=float(sca[imax]))

# --- the elevation
H.fig_map(out / "elevation.png", z, "viridis", "Elevation (m)", unit="metres", under=shade, alpha=0.7, width=600)

# --- a depression: profile along the row of the deepest fill, with the filled and breached surfaces
r0, c0 = np.unravel_index(np.argmax(raise_), z.shape)
numbers["deepest"] = dict(row=int(r0), col=int(c0), raise_=float(raise_[r0, c0]), lab_cells=int(sizes[lab[r0, c0] - 1]))
# the main stream through it: up the largest tributary from the depression, then down the D8 directions
OFF = {1: (0, 1), 2: (1, 1), 4: (1, 0), 8: (1, -1), 16: (0, -1), 32: (-1, -1), 64: (-1, 0), 128: (-1, 1)}
d8f = rd("d8.tif").filled(-1).astype(int)
accd8 = rd("acc_d8.tif").filled(0)
cell = (int(r0), int(c0))
for _ in range(110):                       # upstream: the neighbour draining here with the most area
    best = None
    for code, (dr, dc) in OFF.items():
        rr, cc = cell[0] - dr, cell[1] - dc
        if 0 <= rr < z.shape[0] and 0 <= cc < z.shape[1] and d8f[rr, cc] == code:
            if best is None or accd8[rr, cc] > accd8[best]:
                best = (rr, cc)
    if best is None:
        break
    cell = best
path = [cell]
d8b = rd("breach_d8.tif").filled(-1).astype(int)     # then down the path water takes on the breached DEM
while d8b[path[-1]] in OFF:
    dr, dc = OFF[d8b[path[-1]]]
    nxt = (path[-1][0] + dr, path[-1][1] + dc)
    if not (0 <= nxt[0] < z.shape[0] and 0 <= nxt[1] < z.shape[1]):
        break
    path.append(nxt)
steps = [0.0] + [30.0 * (2 ** 0.5 if (a[0] != b[0] and a[1] != b[1]) else 1.0) for a, b in zip(path, path[1:])]
x = np.cumsum(steps) / 1000
zr = np.array([z.data[p] for p in path])
fr = np.array([filled.data[p] for p in path])
br = np.array([breached.data[p] for p in path])
numbers["profile"] = dict(cells=len(path), km=float(x[-1]))
fig, ax = plt.subplots(figsize=(9.6, 3.6), dpi=150)
ax.fill_between(x, zr, zr.min() - 5, color="#e9e4d8", lw=0)
ax.fill_between(x, zr, fr, where=fr > zr + TOL, color=figs.WATER, alpha=0.25, lw=0)
ax.plot(x, zr, color=figs.INK, lw=1.6, label="Elevation model")
ax.plot(x, fr, color=figs.WATER, lw=1.8, ls="--", label="Filled (hydro.fill)")
ax.plot(x, br, color=figs.EMBER, lw=1.6, ls=":", label="Breached (hydro.breach)")
ax.set_xlabel("Distance down the main stream (km)")
ax.set_ylabel("Elevation (m)")
ax.legend(fontsize=9, loc="lower left")
fig.tight_layout()
fig.savefig(out / "depression-profile.png", dpi=150)
plt.close(fig)
figs._shrink(out / "depression-profile.png")

# --- the maps of what filling and breaching changed
fill_m = np.ma.masked_where(raise_ <= TOL, raise_)
CROP = (230, 369, 140, 270)                 # the south-east of the map, where the largest depressions are
numbers["crop_fill"] = CROP
H.fig_map(out / "fill-depth.png", fill_m, "Blues", "Raised by filling (m)", unit="metres", under=shade, vmin=0,
          vmax=10, width=480, crop=CROP)
ch = np.ma.masked_where(np.abs(chg) <= TOL, chg)
H.fig_map(out / "breach-change.png", ch, "RdBu_r", "Changed by breaching (m)", unit="metres", under=shade,
          norm=mcolors.TwoSlopeNorm(vmin=-5, vcenter=0, vmax=5), width=480, crop=CROP)

# --- D8 directions: a zoom with arrows
dmap = rd("d8.tif")
# a zoom with arrows, around a junction of two tributaries in the middle of the map
OFFS = {1: (0, 1), 2: (1, 1), 4: (1, 0), 8: (1, -1), 16: (0, -1), 32: (-1, -1), 64: (-1, 0), 128: (-1, 1)}
dd = dmap.filled(-1).astype(int)
ad = acc["d8"].filled(0)
junction = None
for r in range(150, 260):
    for c in range(80, 190):
        if not 300 <= ad[r, c] <= 3000:
            continue
        ups = [ad[r - dr, c - dc] for k, (dr, dc) in OFFS.items() if dd[r - dr, c - dc] == k]
        if sum(u >= 60 for u in ups) >= 2:
            junction = (r, c)
            break
    if junction:
        break
ZR, ZC = (junction[0] - 13, junction[0] + 9), (junction[1] - 11, junction[1] + 11)
sub = dmap[ZR[0]:ZR[1], ZC[0]:ZC[1]].filled(0).astype(int)
asub = acc["d8"][ZR[0]:ZR[1], ZC[0]:ZC[1]].filled(0)


def arrows(ax):
    for strong in (False, True):
        xs_, ys_, us_, vs_ = [], [], [], []
        for r in range(sub.shape[0]):
            for c in range(sub.shape[1]):
                v = sub[r, c]
                if v in ARROW and (asub[r, c] >= 50) == strong:
                    dx, dy = ARROW[v]
                    n = (dx * dx + dy * dy) ** 0.5
                    xs_.append(c - dx / n * 0.32)
                    ys_.append(r - dy / n * 0.32)
                    us_.append(dx / n * 0.64)
                    vs_.append(dy / n * 0.64)
        ax.quiver(xs_, ys_, us_, vs_, angles="xy", scale_units="xy", scale=1,
                  color=figs.WATER if strong else figs.INK2, width=0.006 if strong else 0.0035,
                  headwidth=3.5, headlength=3.5, headaxislength=3.2)


H.fig_map(out / "d8-zoom.png", np.ma.masked_all(sub.shape), None, None, legend=False,
          under=shade[ZR[0]:ZR[1], ZC[0]:ZC[1]], extra=arrows, width=620)
numbers["zoom"] = dict(rows=ZR, cols=ZC)

# --- accumulation by method, log colours, one range
lo, hi = 1, float(max(a.max() for a in acc.values()))
frames = []
NAMES = {"d8": "D8", "dinf": "D-infinity", "fd8": "FD8", "quinn": "Quinn", "mfd": "MFD (InVEST)"}
for m in METHODS:
    H.fig_map(out / f"acc_{m}.png", acc[m], "viridis", None, norm=mcolors.LogNorm(vmin=lo, vmax=hi),
              unit="cells (log scale)", width=620)
    a = acc[m].compressed()
    frames.append(dict(method=NAMES[m], largest_km2=round(float(a.max()) * 900 / 1e6, 2),
                       under2=round(float((a < 2).sum()) / a.size * 100, 1),
                       median=round(float(np.median(a)), 2),
                       ge100=round(float((a >= 100).sum()) / a.size * 100, 2)))
numbers["frames"] = frames
# the accumulation of the raw DEM and of the filled one, for a before / after comparison (same framing, no legend)
for name, a in (("compare_raw", raw_acc), ("compare_filled", acc["d8"])):
    H.fig_map(out / f"{name}.png", a, "viridis", None, norm=mcolors.LogNorm(vmin=lo, vmax=hi), legend=False,
              width=560)
# zooms of a hillslope for the comparison D8 / FD8 / MFD
ZR2, ZC2 = (60, 140), (150, 250)
for m in ("d8", "fd8", "dinf", "mfd"):
    H.fig_map(out / f"zoom_{m}.png", acc[m][ZR2[0]:ZR2[1], ZC2[0]:ZC2[1]], "viridis", None,
              norm=mcolors.LogNorm(vmin=1, vmax=3000), legend=False, width=760)
numbers["zoom2"] = dict(rows=ZR2, cols=ZC2)

# the distribution of the accumulation, per method: the share of cells with at least x cells upslope
xs = [1, 1.5, 2, 3, 5, 10, 20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000]
rows = []
for xv in xs:
    r = {"cells": xv}
    for m in METHODS:
        a = acc[m].compressed()
        r[NAMES[m]] = round(float((a >= xv).sum()) / a.size * 100, 3)
    rows.append(r)
figs.write_csv(data / "exceedance.csv", rows)

# --- weights: the share of the upslope area that is open land (NDVI <= 0.55), on the streams
ndvi_open = rd("open.tif")
numbers["open_share_map"] = float(ndvi_open.mean() * 100)
share = rd("open_share.tif")
numbers["open_share"] = H.stats(share)
# drawn three cells wide (the largest share around each stream cell), to be seen at this size
wide = ndimage.grey_dilation(share.filled(-1), size=(3, 3))
wide = np.ma.masked_less(wide, 0)
H.fig_map(out / "open-share.png", wide, "plasma_r", "Open land upslope (%)", unit="% of the upslope area",
          under=shade, vmin=0, vmax=20, width=640)
oacc = rd("open_acc.tif")
numbers["open_acc_outlet"] = float(oacc[imax])

(work / "numbers.json").write_text(json.dumps(numbers, indent=1, default=float))
print(json.dumps(numbers, indent=1, default=float))
