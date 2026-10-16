"""Figures of "Movement as electric current" (circuit connectivity, part 1): small networks of resistors solved by
terraflux (Circuitscape's network mode), and a wall with two gaps: the least-cost path, Circuitscape's current
and Omniscape's normalised current."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-networks"))
import _clc  # noqa: E402,F401
from _clc import figs, np, plt  # noqa: E402

SLUG = "circuit-connectivity/circuit-theory"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)

# ---------------------------------------------------------------- three small circuits
NETWORKS = {
    "one": [[1, 2, 1], [2, 3, 1]],
    "two": [[1, 2, 1], [2, 3, 1], [1, 4, 1], [4, 3, 1]],
    "long": [[1, 2, 1], [2, 3, 1], [1, 4, 2], [4, 3, 2]],
}
MODEL = 'format = "terraflux/1"\n'
for name, edges in NETWORKS.items():
    MODEL += f"""
[[nodes]]
id = "edges_{name}"
op = "table.of"
params = {{ columns = ["from", "to", "resistance"], rows = {edges} }}

[[nodes]]
id = "circuit_{name}"
op = "circuit.network_pairwise"
inputs = {{ graph = "edges_{name}.table" }}
params = {{ focal_nodes = [1, 3], current_maps = true }}
"""
figs.run(figs.add_writes(MODEL, {f"circuit_{n}.resistances": f"r_{n}.csv" for n in NETWORKS} |
                         {f"circuit_{n}.cum_branch_currents": f"b_{n}.csv" for n in NETWORKS}), work)
reff = {}
branch = {}
for n in NETWORKS:
    rows = figs.read_csv(work / "output" / f"r_{n}.csv")
    reff[n] = float(rows[0][list(rows[0].keys())[2]])
    branch[n] = figs.read_csv(work / "output" / f"b_{n}.csv")
    print(n, "effective resistance", reff[n], "branch currents", branch[n])

POS = {1: (0, 0), 2: (1, 0.55), 3: (2, 0), 4: (1, -0.55)}
fig, axs = plt.subplots(1, 3, figsize=(10, 3.0), dpi=150)
titles = {"one": "One route", "two": "Two equal routes", "long": "A second, longer route"}
for ax, (n, edges) in zip(axs, NETWORKS.items()):
    ax.set_aspect("equal")
    ax.set_axis_off()
    for a, b, r in edges:
        (x1, y1), (x2, y2) = POS[a], POS[b]
        ax.plot([x1, x2], [y1, y2], color=figs.INK2, lw=2.2 if r == 1 else 1.2, zorder=1)
        ax.text((x1 + x2) / 2 + (0.0), (y1 + y2) / 2 + (0.13 if y1 + y2 > 0 else -0.17), f"{r:g} Ω", ha="center",
                va="center", fontsize=8.5, color=figs.INK2)
    used = sorted({v for e in edges for v in e[:2]})
    for v in used:
        x, y = POS[v]
        col = figs.EMBER if v in (1, 3) else "white"
        ax.add_patch(plt.Circle((x, y), 0.11, facecolor=col, edgecolor=figs.INK, lw=1.2, zorder=2))
        ax.text(x, y, str(v), ha="center", va="center", fontsize=8.5, zorder=3,
                color="white" if v in (1, 3) else figs.INK)
    ax.set_xlim(-0.3, 2.3)
    ax.set_ylim(-0.9, 0.9)
    ax.set_title(titles[n], loc="left", fontsize=10.5)
    ax.text(1, -0.86, f"effective resistance {reff[n]:.2f} Ω", ha="center", va="bottom", fontsize=9.5,
            color=figs.INK, fontweight="bold")
fig.tight_layout()
fig.savefig(out / "resistors.png", dpi=150)
plt.close(fig)
figs._shrink(out / "resistors.png")

# ---------------------------------------------------------------- a wall with two gaps
import rasterio  # noqa: E402
from rasterio.transform import from_origin  # noqa: E402

H, W = 60, 100
res = np.ones((H, W), dtype="float32")
res[:, 48:52] = np.nan
res[8:11, 48:52] = 1
res[34:52, 48:52] = 1
pts = np.zeros((H, W), dtype="uint8")
pts[25:35, 5] = 1
pts[25:35, 94] = 2
prof = dict(driver="GTiff", height=H, width=W, count=1, transform=from_origin(0, H * 100, 100, 100), crs="EPSG:3035")
with rasterio.open(work / "wall.tif", "w", dtype="float32", nodata=np.nan, **prof) as f:
    f.write(res, 1)
with rasterio.open(work / "focal.tif", "w", dtype="uint8", nodata=None, **prof) as f:
    f.write(pts, 1)
with rasterio.open(work / "wall_0.tif", "w", dtype="uint8", nodata=None, **prof) as f:
    f.write(np.nan_to_num(res, nan=0).astype("uint8"), 1)

WALL = """format = "terraflux/1"

[[nodes]]
id = "resistance"
op = "raster.read"
params = { path = "wall.tif" }

[[nodes]]
id = "focal"
op = "raster.read"
params = { path = "focal.tif" }

[[nodes]]
id = "current"
op = "circuit.pairwise"
inputs = { habitat = "resistance.raster", points = "focal.raster" }
params = { cum_map = true, max_map = true }

[[nodes]]
id = "omniscape"
op = "circuit.omniscape"
inputs = { resistance = "resistance.raster" }
params = { radius = 15, block_size = 1, flow_potential = true, normalized_current = true }

[[nodes]]
id = "open"
op = "raster.read"
params = { path = "wall_0.tif" }

[[nodes]]
id = "left"
op = "map.calc"
inputs = { f = "focal.raster" }
params = { expression = "f == 1" }

[[nodes]]
id = "right"
op = "map.calc"
inputs = { f = "focal.raster" }
params = { expression = "f == 2" }

[[nodes]]
id = "path"
op = "restoration.least_cost_path"
inputs = { resistance = "open.raster", a = "left.map", b = "right.map" }
"""
figs.run(figs.add_writes(WALL, {"current.cum": "current.tif", "omniscape.normalized": "normalized.tif",
                                "path.path": "path.tif", "current.resistances": "wall_r.csv"}), work, name="wall.toml")
cur = figs.read(work / "current.tif")[0]
nrm = figs.read(work / "normalized.tif")[0]
path = figs.read(work / "path.tif")[0]
narrow = float(np.ma.sum(cur[8:11, 50]))
wide = float(np.ma.sum(cur[34:52, 50]))
print("share through the narrow gap:", round(narrow / (narrow + wide), 3), "current per cell narrow",
      round(float(cur[8:11, 50].mean()), 4), "wide", round(float(cur[34:52, 50].mean()), 4))
print("normalised: narrow", round(float(nrm[8:11, 48:52].mean()), 2), "wide", round(float(nrm[34:52, 48:52].mean()), 2),
      "open land (column 20)", round(float(nrm[:, 20].mean()), 2))
print("least-cost path rows at the wall:", np.nonzero(np.ma.filled(path[:, 50], 0) > 0)[0])


def wall_map(a, png, cmap=None, title=None, vmin=None, vmax=None, unit="", path_mask=None, log=False, diverging=False):
    from matplotlib import colors as mc
    fig = plt.figure(figsize=(4.6, 3.1), dpi=150)
    ax = fig.add_axes([0, 0.1 if unit else 0.0, 1, 0.82 if unit else 0.9])
    ax.set_axis_off()
    base = np.ones((H, W, 4))
    base[np.isnan(res)] = mc.to_rgba("#3c3c3c")
    ax.imshow(base, interpolation="nearest")
    if a is not None:
        if diverging:
            norm = mc.TwoSlopeNorm(vmin=vmin, vcenter=1, vmax=vmax)
        elif log:
            norm = mc.LogNorm(vmin=vmin, vmax=vmax)
        else:
            norm = mc.Normalize(vmin=vmin, vmax=vmax)
        im = ax.imshow(np.ma.masked_invalid(np.ma.filled(a.astype(float), np.nan)), cmap=cmap, norm=norm,
                       interpolation="nearest")
        if unit:
            cax = fig.add_axes([0.2, 0.06, 0.6, 0.04])
            cb = fig.colorbar(im, cax=cax, orientation="horizontal")
            cb.outline.set_visible(False)
            cb.ax.grid(False)
            cb.ax.tick_params(labelsize=7.5)
            cb.set_label(unit, fontsize=8, color=figs.INK2)
    if path_mask is not None:
        m = np.ma.filled(path_mask, 0) > 0
        rgba = np.zeros((H, W, 4))
        rgba[m] = mc.to_rgba(figs.EMBER)
        ax.imshow(rgba, interpolation="nearest")
    f = pts > 0
    rgba = np.zeros((H, W, 4))
    rgba[f] = mc.to_rgba("#2f6f9f")
    ax.imshow(rgba, interpolation="nearest")
    fig.text(0.01, 0.99, title, ha="left", va="top", fontsize=9.5, fontweight="bold", color=figs.INK)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


wall_map(None, out / "wall-path.png", title="Least-cost path", path_mask=path)
wall_map(cur, out / "wall-current.png", cmap="magma", title="Current (Circuitscape)", vmin=1e-3,
         vmax=float(cur.max()), log=True, unit="Current (A)")
wall_map(nrm, out / "wall-normalized.png", cmap="RdBu_r", title="Normalised current (Omniscape)", vmin=0, vmax=4,
         diverging=True, unit="Current / flow potential")

# the wall's rasters, for readers to download
import shutil  # noqa: E402

for f in ("wall.tif", "focal.tif", "wall_0.tif"):
    shutil.copy2(work / f, data / f)
