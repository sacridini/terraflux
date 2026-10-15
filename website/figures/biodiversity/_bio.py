"""Shared by the biodiversity tutorials' figures: Biodiverse's example data (landschaft's copy: 1,795 records of 31
species in eastern Australia, coordinates in metres, and a tree made by clustering the species), copied where the
pages offer them for download, and the drawing of maps of 100 km cells."""

from __future__ import annotations

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

SRC = figs.ROOT / "extern" / "landschaft" / "src" / "landschaft" / "bio" / "data"
DATA = figs.data_dir("biodiversity/richness-and-endemism")
DATA_URL = "/data/tutorials/biodiversity/richness-and-endemism"


def setup(work: pathlib.Path):
    """sites.csv and tree.nex in the work folder and in the downloads."""
    for src, name in (("example_sites.csv", "sites.csv"), ("example_tree.nex", "tree.nex")):
        shutil.copy2(SRC / src, DATA / name)
        shutil.copy2(SRC / src, work / name)


def read_records(work: pathlib.Path):
    rows = figs.read_csv(work / "sites.csv")
    return np.array([float(r["x"]) for r in rows]), np.array([float(r["y"]) for r in rows]), [r["species"] for r in rows]


def cell_map(a, png, cmap="viridis", vmin=None, vmax=None, unit="", palette=None, width=820, log=False,
             extent=None, legend=True, points=None, title=None):
    """A map of cells (a raster of groups): continuous with a colour bar, or classes with `palette`
    ({value: (name, colour)}). Cells without a value are left white over a light frame."""
    a = np.ma.masked_invalid(np.ma.filled(np.ma.asarray(a).astype(float), np.nan))
    h, w = a.shape
    dpi = 150
    fw = width / dpi
    mw = fw * (0.68 if legend else 1.0)
    fh = mw * h / w + (0.3 if title else 0.05)
    fig = plt.figure(figsize=(fw, fh), dpi=dpi)
    top = 1 - (0.3 / fh if title else 0)
    ax = fig.add_axes([0, 0, 0.68 if legend else 1.0, top])
    ax.set_xticks([])
    ax.set_yticks([])
    for s in ax.spines.values():
        s.set_visible(True)
        s.set_color(figs.RULE)
    ax.set_facecolor("#f4f6f4")
    ext = extent
    if palette is not None:
        keys = sorted(palette)
        cm = mcolors.ListedColormap([palette[k][1] for k in keys])
        norm = mcolors.BoundaryNorm([k - 0.5 for k in keys] + [keys[-1] + 0.5], cm.N)
        ax.imshow(a, cmap=cm, norm=norm, interpolation="nearest", extent=ext)
        if legend:
            present = {int(v) for v in np.unique(a.compressed())}
            handles = [Patch(facecolor=palette[k][1], edgecolor="#00000022", label=palette[k][0]) for k in keys
                       if k in present]
            fig.legend(handles=handles, loc="center left", bbox_to_anchor=(0.70, 0.5), fontsize=8.5,
                       handlelength=1.1, handleheight=1.1, borderaxespad=0)
    else:
        norm = mcolors.LogNorm(vmin=vmin, vmax=vmax) if log else mcolors.Normalize(
            vmin=a.min() if vmin is None else vmin, vmax=a.max() if vmax is None else vmax)
        cmo = plt.get_cmap(cmap).copy()
        cmo.set_bad((1, 1, 1, 0))
        im = ax.imshow(a, cmap=cmo, norm=norm, interpolation="nearest", extent=ext)
        if legend:
            cax = fig.add_axes([0.73, 0.12 * top, 0.04, 0.76 * top])
            cb = fig.colorbar(im, cax=cax)
            cb.outline.set_visible(False)
            cb.ax.grid(False)
            cb.ax.tick_params(labelsize=8.5)
            if unit:
                cb.set_label(unit, fontsize=9, color=figs.INK2)
    if points is not None:
        ax.scatter(points[0], points[1], s=2, c=figs.INK, alpha=0.5, linewidths=0)
    if title:
        fig.text(0.0, 0.995, title, ha="left", va="top", fontsize=10.5, fontweight="bold", color=figs.INK)
    fig.savefig(png, dpi=dpi, bbox_inches="tight", pad_inches=0.05)
    plt.close(fig)
    figs._shrink(png)
    return png
