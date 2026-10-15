"""Shared drawing helpers for the GuidosToolbox tutorials (habitat-patterns and fragmentation).

Maps are drawn at a fixed size with a fixed legend (every class of the palette, present or not), so that the frames
of a slider and the two sides of a comparison line up exactly. Colours are GuidosToolbox's (crates/ops/src
gtb_palettes.rs and landscape.rs), read through website/tools/figs.py.
"""

from __future__ import annotations

import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import figs  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

DATA = figs.ROOT / "extern" / "landschaft" / "src" / "landschaft" / "data"
CLC = DATA / "clc.tif"            # CORINE Land Cover, 2000 x 2000 cells of 25 m near Hannover (EPSG:3035)
GTB_INPUT = DATA / "input.tif"    # GuidosToolbox's sample forest map, 1000 x 1000 cells, 0 missing 1 background 2 forest
GSC_INPUT = DATA / "gscinput.tif" # GraySpatCon's sample: % tree cover, cells of 2430 m, 255 missing

# ------------------------------------------------------------------------------------------------ palettes

_MSPA = figs.gtb_colors("MSPA_ON")

# MSPA's classes folded for a legend: the transitions (bridge in edge...) with their class, internal features with
# theirs; GuidosToolbox draws them in the same colours anyway.
MSPA_FOLDED = {
    17: ("Core", _MSPA[17]),
    9: ("Islet", _MSPA[9]),
    5: ("Perforation", _MSPA[5]),
    3: ("Edge", _MSPA[3]),
    65: ("Loop", _MSPA[65]),
    33: ("Bridge", _MSPA[33]),
    1: ("Branch", _MSPA[1]),
    0: ("Background", _MSPA[0]),
    100: ("Core-opening", _MSPA[100]),
    220: ("Border-opening", _MSPA[220]),
    129: ("Missing", "#ffffff"),
}
MSPA_ORDER = [17, 9, 5, 3, 65, 33, 1, 0, 100, 220, 129]


def mspa_fold(a: np.ndarray) -> np.ndarray:
    """An MSPA map with its transitions and internal features folded into the seven classes."""
    a = np.asarray(a).astype(np.int32).copy()
    internal = (a > 100) & (a < 200) & (a != 129)
    a[internal] -= 100
    a[np.isin(a, (35, 37))] = 33
    a[np.isin(a, (67, 69))] = 65
    return a


FAD_CLASS = {
    6: ("Intact (100%)", "#007800"),
    5: ("Interior (90-99%)", "#00af00"),
    4: ("Dominant (60-89%)", "#8cc864"),
    3: ("Transitional (40-59%)", "#ffc800"),
    2: ("Patchy (10-39%)", "#fa8c5a"),
    1: ("Rare (0-9%)", "#d73228"),
    101: ("Background", "#afafaf"),
    102: ("Missing", "#ffffff"),
}
FAD_ORDER = [6, 5, 4, 3, 2, 1, 101, 102]


def density_classes(a: np.ndarray, five=False) -> np.ndarray:
    """A density map (0-100 on the foreground, GTB's codes elsewhere) as its classes 1 (rare) to 6 (intact)."""
    a = np.asarray(a)
    out = np.full(a.shape, 102, dtype=np.int32)
    out[(a >= 0) & (a < 10)] = 1
    out[(a >= 10) & (a < 40)] = 2
    out[(a >= 40) & (a < 60)] = 3
    out[(a >= 60) & (a < 90)] = 4
    out[(a >= 90) & (a < 100)] = 5
    out[a == 100] = 5 if five else 6
    out[a == 101] = 101
    return out


# ------------------------------------------------------------------------------------------------ reading/writing


def read(path, band=1) -> np.ndarray:
    import rasterio

    with rasterio.open(path) as ds:
        return ds.read(band)


def read_all(path) -> np.ndarray:
    import rasterio

    with rasterio.open(path) as ds:
        return ds.read()


def write_grid(a: np.ndarray, path, cell=1.0):
    """A small array as a GeoTIFF (no CRS), for toy examples run through Terraflux."""
    import rasterio
    from rasterio.transform import from_origin

    a = np.asarray(a)
    with rasterio.open(path, "w", driver="GTiff", width=a.shape[1], height=a.shape[0], count=1,
                       dtype=a.dtype.name, transform=from_origin(0, a.shape[0] * cell, cell, cell)) as d:
        d.write(a, 1)


def copy_data(src, dst_dir) -> pathlib.Path:
    import shutil

    dst = pathlib.Path(dst_dir) / pathlib.Path(src).name
    shutil.copy(src, dst)
    return dst


# ------------------------------------------------------------------------------------------------ drawing


def _rgba(a: np.ndarray, palette: dict) -> np.ndarray:
    lut_keys = sorted(palette)
    out = np.ones(a.shape + (4,))
    for k in lut_keys:
        out[a == k] = mcolors.to_rgba(palette[k][1])
    return out


def classmap(a: np.ndarray, png, palette: dict, order=None, width=900, legend=True, title=None,
             counts=False, grid=False, legend_cols=1, legend_width=0.30, labels=None, fontsize=9.5):
    """A map of classes at a fixed size: the map on the left, a legend of every class in `order` on the right
    (or none). `grid` draws the cells' outlines (toy examples). `labels` = [(row, col, text), ...] annotations."""
    a = np.asarray(a)
    order = order or sorted(palette)
    h, w = a.shape
    dpi = 150
    W = width / dpi
    map_frac = 1 - legend_width if legend else 1.0
    H = W * map_frac * h / w + (0.32 if title else 0.04)
    fig = plt.figure(figsize=(W, H), dpi=dpi)
    top = 1 - (0.30 / H if title else 0.02 / H)
    ax = fig.add_axes([0.005, 0.02 / H, map_frac - 0.01, top - 0.02 / H])
    ax.imshow(_rgba(a, palette), interpolation="nearest")
    ax.set_xticks([])
    ax.set_yticks([])
    for s in ax.spines.values():
        s.set_visible(grid)
        s.set_color("#00000055")
    ax.grid(False)
    if grid:
        ax.set_xticks(np.arange(-0.5, w, 1), minor=True)
        ax.set_yticks(np.arange(-0.5, h, 1), minor=True)
        ax.grid(which="minor", color="#00000030", linewidth=0.5, linestyle="-")
        ax.tick_params(which="both", length=0)
    if labels:
        for r, c, text in labels:
            ax.annotate(text, (c, r), ha="center", va="center", fontsize=fontsize - 1, color=figs.INK,
                        bbox=dict(boxstyle="round,pad=0.25", fc="#ffffffdd", ec="#00000033", lw=0.5))
    if legend:
        total = a.size
        handles = []
        for k in order:
            if k not in palette:
                continue
            name, col = palette[k]
            if counts:
                name = f"{name}  {(a == k).sum() / total * 100:.1f}%"
            handles.append(Patch(facecolor=col, edgecolor="#00000033", label=name))
        fig.legend(handles=handles, loc="center left", bbox_to_anchor=(map_frac + 0.01, 0.5), fontsize=fontsize,
                   handlelength=1.1, handleheight=1.1, borderaxespad=0, labelspacing=0.5, ncol=legend_cols)
    if title:
        fig.text(0.005, 1 - 0.06 / H, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=dpi)
    plt.close(fig)
    figs._shrink(png)
    return png


def ramp(a: np.ndarray, png, cmap="viridis", vmin=None, vmax=None, width=900, unit="", title=None, mask=None,
         diverging=False, legend=True):
    """A continuous map at a fixed size with a colour bar."""
    a = np.ma.masked_invalid(np.asarray(a, dtype="float64"))
    if mask is not None:
        a = np.ma.masked_where(mask, a)
    h, w = a.shape
    dpi = 150
    W = width / dpi
    map_frac = 0.80 if legend else 1.0
    H = W * map_frac * h / w + (0.32 if title else 0.04)
    fig = plt.figure(figsize=(W, H), dpi=dpi)
    top = 1 - (0.30 / H if title else 0.02 / H)
    ax = fig.add_axes([0.005, 0.02 / H, map_frac - 0.01, top - 0.02 / H])
    ax.set_axis_off()
    lo = float(a.min()) if vmin is None else vmin
    hi = float(a.max()) if vmax is None else vmax
    if diverging:
        m = max(abs(lo), abs(hi)) or 1
        norm = mcolors.TwoSlopeNorm(vmin=-m, vcenter=0, vmax=m)
    else:
        norm = mcolors.Normalize(vmin=lo, vmax=hi)
    cm = plt.get_cmap(cmap).copy()
    cm.set_bad("#ffffff")
    im = ax.imshow(a, cmap=cm, norm=norm, interpolation="nearest")
    if legend:
        cax = fig.add_axes([map_frac + 0.03, 0.12, 0.025, top - 0.2])
        cb = fig.colorbar(im, cax=cax)
        cb.outline.set_visible(False)
        cb.ax.grid(False)
        cb.ax.tick_params(labelsize=9)
        if unit:
            cb.set_label(unit, fontsize=9.5, color=figs.INK2)
    if title:
        fig.text(0.005, 1 - 0.06 / H, title, ha="left", va="top", fontsize=11.5, fontweight="bold", color=figs.INK)
    png = pathlib.Path(png)
    fig.savefig(png, dpi=dpi)
    plt.close(fig)
    figs._shrink(png)
    return png


def share(a: np.ndarray, codes, of=None) -> float:
    """The % of the cells in `codes` among the cells in `of` (all cells by default)."""
    a = np.asarray(a)
    den = a.size if of is None else np.isin(a, of).sum()
    return float(np.isin(a, codes).sum()) / max(den, 1) * 100


def mspa_shares(m: np.ndarray) -> dict:
    """The share (% of the foreground) of each of MSPA's seven classes."""
    f = mspa_fold(m)
    fg = np.isin(f, [17, 9, 5, 3, 65, 33, 1])
    n = fg.sum()
    return {MSPA_FOLDED[k][0]: round(float((f == k).sum()) / float(n) * 100, 2) for k in [17, 9, 5, 3, 65, 33, 1]}


def toml_nodes(*nodes: str) -> str:
    return 'format = "terraflux/1"\n\n' + "\n".join(nodes)


def node(id, op, inputs=None, **params) -> str:
    def val(v):
        if isinstance(v, bool):
            return "true" if v else "false"
        if isinstance(v, str):
            return '"' + v.replace("\\", "\\\\").replace("\n", "\\n").replace('"', '\\"') + '"'
        if isinstance(v, (list, tuple)):
            return "[" + ", ".join(val(x) for x in v) + "]"
        return str(v)

    s = f'[[nodes]]\nid = "{id}"\nop = "{op}"\n'
    if inputs:
        s += "inputs = { " + ", ".join(f'{k} = "{v}"' for k, v in inputs.items()) + " }\n"
    if params:
        s += "params = { " + ", ".join(f"{k} = {val(v)}" for k, v in params.items()) + " }\n"
    return s
