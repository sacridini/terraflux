"""What the figures of the getting-started tutorials share: the sample data, CORINE's legend, drawing helpers.

The data are read in place from the repository (landschaft's examples and the test inputs); only figures go into
the website.
"""

from __future__ import annotations

import pathlib
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import figs  # noqa: E402
import numpy as np  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

ROOT = figs.ROOT
LS = ROOT / "extern" / "landschaft" / "src" / "landschaft"
CLC = LS / "data" / "clc.tif"                       # CORINE land cover, 2000 x 2000 cells of 25 m, EPSG:3035
FOREST_WGS84 = LS / "data" / "input.tif"            # a forest map in degrees (EPSG:4326): 0 missing, 1, 2 forest
ELEVATION = LS / "surface" / "data" / "orelevation.tif"   # SRTM elevation, SW Oregon, EPSG:4326, about 30 m
NDVI = LS / "surface" / "data" / "orforest.tif"     # summer NDVI, the same grid
SIBUYAN = ROOT / "tests" / "data" / "inputs" / "sibuyan_landuse.tif"
SIBUYAN_DRIVERS = ROOT / "tests" / "data" / "inputs" / "sibuyan_drivers.tif"

# The CORINE Land Cover nomenclature (level 3) by the grid codes 1-44 of its rasters, with its standard colours.
CLC_LEGEND = {
    1: ("Continuous urban fabric", "#e6004d"), 2: ("Discontinuous urban fabric", "#ff0000"),
    3: ("Industrial or commercial units", "#cc4df2"), 4: ("Road and rail networks", "#cc0000"),
    5: ("Port areas", "#e6cccc"), 6: ("Airports", "#e6cce6"), 7: ("Mineral extraction sites", "#a600cc"),
    8: ("Dump sites", "#a64d00"), 9: ("Construction sites", "#ff4dff"), 10: ("Green urban areas", "#ffa6ff"),
    11: ("Sport and leisure facilities", "#ffe6ff"), 12: ("Non-irrigated arable land", "#ffffa8"),
    13: ("Permanently irrigated land", "#ffff00"), 14: ("Rice fields", "#e6e600"), 15: ("Vineyards", "#e68000"),
    16: ("Fruit trees and berries", "#f2a64d"), 17: ("Olive groves", "#e6a600"), 18: ("Pastures", "#e6e64d"),
    19: ("Annual and permanent crops", "#ffe6a6"), 20: ("Complex cultivation patterns", "#ffe64d"),
    21: ("Agriculture with natural vegetation", "#e6cc4d"), 22: ("Agro-forestry areas", "#f2cca6"),
    23: ("Broad-leaved forest", "#80ff00"), 24: ("Coniferous forest", "#00a600"), 25: ("Mixed forest", "#4dff00"),
    26: ("Natural grasslands", "#ccf24d"), 27: ("Moors and heathland", "#a6ff80"),
    28: ("Sclerophyllous vegetation", "#a6e64d"), 29: ("Transitional woodland-shrub", "#a6f200"),
    30: ("Beaches, dunes, sands", "#e6e6e6"), 31: ("Bare rocks", "#cccccc"),
    32: ("Sparsely vegetated areas", "#ccffcc"), 33: ("Burnt areas", "#000000"),
    34: ("Glaciers and perpetual snow", "#a6e6cc"), 35: ("Inland marshes", "#a6a6ff"), 36: ("Peat bogs", "#4d4dff"),
    37: ("Salt marshes", "#ccccff"), 38: ("Salines", "#e6e6ff"), 39: ("Intertidal flats", "#a6a6e6"),
    40: ("Water courses", "#00ccf2"), 41: ("Water bodies", "#80f2e6"), 42: ("Coastal lagoons", "#00ffa6"),
    43: ("Estuaries", "#a6ffe6"), 44: ("Sea and ocean", "#e6f2ff"),
}

# The six broad groups the tutorials reclassify CORINE into.
BROAD = {1: ("Urban", "#b4508a"), 2: ("Farmland", "#d9a441"), 3: ("Forest", "#3b8a5a"),
         4: ("Open natural", "#a7c957"), 5: ("Wetland", "#6f8fbf"), 6: ("Water", "#2f6f9f")}
BROAD_RULES = "1..11 -> 1\n12..22 -> 2\n23..25 -> 3\n26..34 -> 4\n35..39 -> 5\n40..44 -> 6"

# a 15 x 15 km window of the CORINE excerpt (its north-west corner), in EPSG:3035
WINDOW = [4261250, 3267500, 4276250, 3282500]


def posix(p) -> str:
    return pathlib.Path(p).as_posix()


def clc_palette(values) -> dict:
    return {int(v): CLC_LEGEND[int(v)] for v in values if int(v) in CLC_LEGEND}


def classes_png(a: np.ndarray, png, palette: dict, width=900, legend=True, ncol=1, title=None, fontsize=8.5,
                legend_frac=0.36, counts=False, grid=False, extent=None, axes=False, xlabel=None, ylabel=None,
                boxes=(), all_keys=False):
    """A map of classes with its legend at the right (in `ncol` columns); cells with no value white."""
    a = np.ma.masked_invalid(np.ma.asarray(a, dtype="float64"))
    keys = sorted(palette)
    lut = np.ones((len(keys) + 1, 4))
    idx = np.zeros(a.shape, dtype=np.int32)
    filled = a.filled(-9999).astype(int)
    for i, v in enumerate(keys):
        lut[i + 1] = mcolors.to_rgba(palette[v][1])
        idx[filled == v] = i + 1
    lut[0] = (1, 1, 1, 0)
    rgba = lut[idx]
    h, w = a.shape
    dpi = 150
    win = width / dpi
    mapw = 1 - legend_frac if legend else 1.0
    hin = win * mapw * h / w + (0.3 if title else 0.05) + (0.35 if axes else 0)
    fig = plt.figure(figsize=(win, hin), dpi=dpi)
    top = 1 - (0.3 / hin if title else 0.01)
    bottom = 0.35 / hin if axes else 0.0
    left = 0.09 if axes else 0.0
    ax = fig.add_axes([left, bottom, mapw - left, top - bottom])
    ax.imshow(rgba, interpolation="nearest", extent=extent)
    for (br, bc, bh, bw) in boxes:
        ax.add_patch(plt.Rectangle((bc - 0.5, br - 0.5), bw, bh, fill=False, edgecolor="#111111", linewidth=1.6))
    if axes:
        ax.grid(False)
        ax.tick_params(labelsize=7.5)
        if xlabel:
            ax.set_xlabel(xlabel, fontsize=8.5)
        if ylabel:
            ax.set_ylabel(ylabel, fontsize=8.5)
    else:
        ax.set_axis_off()
    if grid:
        ax.set_xticks(np.arange(-0.5, w, 1), minor=True)
        ax.set_yticks(np.arange(-0.5, h, 1), minor=True)
        ax.grid(which="minor", color="#ffffff", linewidth=0.6)
        ax.tick_params(which="both", length=0)
    if legend:
        total = a.count() or 1
        hs = []
        for v in keys:
            if not all_keys and not (filled == v).any():
                continue
            name, col = palette[v]
            lab = name + (f"  {((filled == v).sum() / total * 100):.1f}%" if counts else "")
            hs.append(Patch(facecolor=col, edgecolor="#00000033", label=lab))
        fig.legend(handles=hs, loc="center left", bbox_to_anchor=(mapw + 0.01, 0.5), fontsize=fontsize, ncol=ncol,
                   handlelength=1.0, handleheight=1.0, borderaxespad=0, labelspacing=0.4, columnspacing=1.0)
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=11, fontweight="bold", color=figs.INK)
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.06)
    plt.close(fig)
    figs._shrink(png)
    return png


def read(path, band=1):
    return figs.read(pathlib.Path(path), band)


def save(fig, png):
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.06)
    plt.close(fig)
    figs._shrink(png)
    return png
