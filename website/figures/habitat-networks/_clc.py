"""The landscape of the connectivity tutorials (habitat networks, circuit connectivity, grid distances).

A 50 x 50 km excerpt of CORINE Land Cover (landschaft's example `clc`, 25 m cells, EPSG:3035) around Lake
Steinhude and Hannover, Lower Saxony, Germany, made 100 m cells by terraflux (raster.resample, the most common
class). Its forests (CLC classes 23, 24, 25) are the habitat; a resistance for a forest species is made from its
classes with raster.reclassify. The scripts of the three categories import this module:

    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-networks"))
    import _clc
"""

from __future__ import annotations

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.collections import LineCollection  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

CLC_SRC = figs.ROOT / "extern" / "landschaft" / "src" / "landschaft" / "data" / "clc.tif"
# the data a reader downloads (shared by the three categories)
DATA = figs.data_dir("habitat-networks/connectivity-as-a-graph")
DATA_URL = "/data/tutorials/habitat-networks/connectivity-as-a-graph"

# the resistance of each class for a forest species (relative values, 1 to 100)
RESISTANCE_RULES = (
    "23 24 25 -> 1\n"           # forests
    "29 -> 2\n"                 # transitional woodland-shrub
    "21 26 27 32 35 36 -> 5\n"  # semi-natural land: heath, bogs, natural grassland, farmland with natural vegetation
    "18 20 -> 10\n"             # pastures, complex cultivation
    "12 -> 20\n"                # arable land
    "10 11 -> 30\n"             # green urban areas, sport and leisure
    "7 8 9 -> 50\n"             # mines, dumps, construction sites
    "1 2 3 4 5 6 40 41 -> 100\n"  # towns, industry, roads, airports, rivers and lakes
    "* -> 20"
)

# the classes drawn in groups
GROUPS = [
    ("Towns and industry", "#b4508a", list(range(1, 10))),
    ("Green urban areas", "#d9a3c6", [10, 11]),
    ("Arable land", "#e9cf8f", list(range(12, 18))),
    ("Pastures and mixed farming", "#bcd38b", list(range(18, 23))),
    ("Forest", figs.FOREST, [23, 24, 25]),
    ("Heath, bogs and shrubs", "#8fae7e", list(range(26, 40))),
    ("Water", "#3f7fae", list(range(40, 45))),
]

FOREST_LIGHT = "#cfe3d4"
MATRIX = "#f1efe8"


def land_cover() -> pathlib.Path:
    """website/static/data/.../clc_100m.tif (and clc.tif), made by terraflux the first time."""
    out = DATA / "clc_100m.tif"
    if not (DATA / "clc.tif").exists():
        shutil.copy2(CLC_SRC, DATA / "clc.tif")
    if not out.exists():
        work = figs.workdir("habitat-networks/_landscape")
        shutil.copy2(CLC_SRC, work / "clc.tif")
        figs.run(MAKE_100M, work)
        shutil.copy2(work / "clc_100m.tif", out)
    return out


MAKE_100M = """format = "terraflux/1"

[[nodes]]
id = "clc"
op = "raster.read"
params = { path = "clc.tif" }
note = "CORINE Land Cover, 25 m cells (EPSG:3035)."

[[nodes]]
id = "coarse"
op = "raster.resample"
inputs = { raster = "clc.raster" }
params = { resolution = 100, resampling = "mode" }
note = "Cells of 100 m, each the most common class of its 16 cells of 25 m."

[[nodes]]
id = "save"
op = "raster.write"
inputs = { value = "coarse.raster" }
params = { path = "clc_100m.tif" }
"""


def setup(work: pathlib.Path) -> pathlib.Path:
    """The land cover copied into a work folder."""
    src = land_cover()
    shutil.copy2(src, work / "clc_100m.tif")
    return work / "clc_100m.tif"


# ------------------------------------------------------------------------------------------------ drawing


def group_array(a: np.ndarray) -> np.ndarray:
    out = np.zeros(a.shape, dtype=np.int16)
    for i, (_, _, classes) in enumerate(GROUPS, start=1):
        out[np.isin(a, classes)] = i
    return out


def group_palette():
    return {i: (name, col) for i, (name, col, _) in enumerate(GROUPS, start=1)}


def centroids(ids: np.ndarray) -> dict[int, tuple[float, float]]:
    """The centre (column, row) of every patch of a map of patch ids."""
    a = np.ma.filled(ids, 0) if np.ma.isMaskedArray(ids) else np.nan_to_num(ids)
    a = a.astype(np.int64)
    rows, cols = np.nonzero(a > 0)
    v = a[rows, cols]
    n = v.max() + 1
    cnt = np.bincount(v, minlength=n)
    sr = np.bincount(v, weights=rows, minlength=n)
    sc = np.bincount(v, weights=cols, minlength=n)
    return {int(i): (sc[i] / cnt[i], sr[i] / cnt[i]) for i in np.nonzero(cnt)[0]}


def network_map(png, ids, values=None, links=None, link_weights=None, title=None, cmap="viridis", vmin=None,
                vmax=None, unit="", base=None, width=900, legend=True, node_color=figs.FOREST, link_color="#1c2420",
                link_width=(0.4, 2.2), log=False, extent_px=None, crop=None, colorbar=True, dots=True,
                highlight=None):
    """Patches (a map of ids) painted with a value each (or one colour), links drawn between their centres.
    `links`: [(i, j)], `link_weights` 0-1 per link (width and opacity). `base`: a map drawn under (classes 1..7
    of the land cover groups, light)."""
    ids = np.ma.masked_invalid(np.ma.filled(ids.astype(float), np.nan)) if np.ma.isMaskedArray(ids) else np.ma.masked_invalid(ids)
    h, w = ids.shape
    if crop:
        r0, r1, c0, c1 = crop
    else:
        r0, r1, c0, c1 = 0, h, 0, w
    dpi = 150
    fw = width / dpi
    show_bar = values is not None and colorbar and legend
    map_w = fw * (0.84 if show_bar else 1.0)
    fh = map_w * (r1 - r0) / (c1 - c0) + (0.32 if title else 0.05)
    fig = plt.figure(figsize=(fw, fh), dpi=dpi)
    top = 1 - (0.32 / fh if title else 0.0)
    ax = fig.add_axes([0, 0, 0.84 if show_bar else 1.0, top])
    ax.set_axis_off()
    bg = np.ones((h, w, 4))
    bg[:] = mcolors.to_rgba(MATRIX)
    if base is not None:
        b = np.asarray(base)
        for i, (_, col, _) in enumerate(GROUPS, start=1):
            rgba = np.array(mcolors.to_rgba(col))
            light = 0.25 * rgba + 0.75 * np.array([1, 1, 1, 1])
            bg[b == i] = light
    ax.imshow(bg, interpolation="nearest")
    filled = ~ids.mask
    im = None
    if values is None:
        rgba = np.zeros((h, w, 4))
        rgba[filled] = mcolors.to_rgba(node_color)
        ax.imshow(rgba, interpolation="nearest")
    else:
        vals = np.ma.masked_invalid(values)
        lo = float(vals.min()) if vmin is None else vmin
        hi = float(vals.max()) if vmax is None else vmax
        norm = mcolors.LogNorm(vmin=max(lo, 1e-6), vmax=hi) if log else mcolors.Normalize(vmin=lo, vmax=hi)
        cm = plt.get_cmap(cmap).copy()
        cm.set_bad((1, 1, 1, 0))
        im = ax.imshow(np.ma.masked_where(ids.mask, vals), cmap=cm, norm=norm, interpolation="nearest")
    if links:
        cen = centroids(ids)
        segs, lw, al = [], [], []
        for k, (i, j) in enumerate(links):
            if i in cen and j in cen:
                segs.append([cen[i], cen[j]])
                wgt = 1.0 if link_weights is None else float(link_weights[k])
                lw.append(link_width[0] + (link_width[1] - link_width[0]) * wgt)
                al.append(0.25 + 0.65 * wgt)
        if segs:
            cols = [mcolors.to_rgba(link_color, a) for a in al]
            ax.add_collection(LineCollection(segs, linewidths=lw, colors=cols, capstyle="round", zorder=3))
        if dots:
            used = sorted({i for l in links for i in l if i in cen})
            xy = np.array([cen[i] for i in used]) if used else np.zeros((0, 2))
            if len(xy):
                ax.scatter(xy[:, 0], xy[:, 1], s=5, c=link_color, zorder=4, linewidths=0)
    if highlight:
        cen = centroids(ids)
        for i, lab in highlight.items():
            if i in cen:
                x, y = cen[i]
                ax.annotate(lab, (x, y), fontsize=8.5, color=figs.INK, ha="center", va="center", zorder=5,
                            bbox=dict(boxstyle="round,pad=0.2", fc="white", ec="none", alpha=0.85))
    ax.set_xlim(c0 - 0.5, c1 - 0.5)
    ax.set_ylim(r1 - 0.5, r0 - 0.5)
    if show_bar and im is not None:
        cax = fig.add_axes([0.87, 0.12 * top, 0.03, 0.74 * top])
        cb = fig.colorbar(im, cax=cax)
        cb.outline.set_visible(False)
        cb.ax.grid(False)
        cb.ax.tick_params(labelsize=8.5, color=figs.RULE)
        if unit:
            cb.set_label(unit, fontsize=9, color=figs.INK2)
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=11, fontweight="bold", color=figs.INK)
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.06)
    plt.close(fig)
    figs._shrink(png)
    return png


def scalebar(ax, cells_per_km=10, km=5, x0=None, y0=None, color="#1c2420"):
    """A scale bar of `km` kilometres (100 m cells: 10 per km)."""
    x0 = 12 if x0 is None else x0
    y0 = ax.get_ylim()[0] - 14 if y0 is None else y0
    ax.plot([x0, x0 + km * cells_per_km], [y0, y0], color=color, lw=2.2, solid_capstyle="butt")
    ax.text(x0 + km * cells_per_km / 2, y0 - 5, f"{km} km", ha="center", va="bottom", fontsize=8.5, color=color)


def legend_patches(items):
    return [Patch(facecolor=c, edgecolor="#00000022", label=n) for n, c in items]
