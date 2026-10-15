"""Shared pieces of the change-detection figures: the data, their classes and colours, and small helpers.

The data are landschaft's example maps, copied into each script's work folder so that the models read them with
plain names, as a reader's model would:

- sibuyan_landuse.tif: Sibuyan Island (Philippines), land use in 1997 and 2011 (bands named 1997 and 2011),
  108 x 128 cells of 250 m, no CRS; 1 forest, 2 coconut, 3 grassland, 4 rice, 5 other, 255 no data. The CLUE-S
  demonstration data of Verburg et al. (2002), modified for teaching.
- sibuyan_drivers.tif: 13 drivers of the same grid (for the simulated map of comparing-maps).
- pie_landuse.tif: Plum Island Ecosystems (Massachusetts), land use in 1985, 1991 and 1999, 434 x 497 cells of
  about 100 m (EPSG:26986); 1 forest, 2 built, 3 other, 255 no data.
"""

from __future__ import annotations

import pathlib
import shutil
import sys

HERE = pathlib.Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "tools"))

import figs  # noqa: E402
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib import colors as mcolors  # noqa: E402
from matplotlib.patches import Patch  # noqa: E402

INPUTS = figs.ROOT / "tests" / "data" / "inputs"
FILES = {
    "sibuyan_landuse.tif": INPUTS / "sibuyan_landuse.tif",
    "sibuyan_drivers.tif": INPUTS / "sibuyan_drivers.tif",
    "pie_landuse.tif": INPUTS / "pie_landuse.tif",
}

SIB = {1: ("Forest", "#3b8a5a"), 2: ("Coconut", "#a7c957"), 3: ("Grassland", "#d9a441"), 4: ("Rice", "#56b4e9"),
       5: ("Other", "#7f7f7f")}
PIE = {1: ("Forest", "#3b8a5a"), 2: ("Built", "#b4508a"), 3: ("Other", "#e3cf8f")}
SIB_KM2 = 0.0625   # a cell of 250 m
CATEGORY = "change-detection"


def setup(slug: str, files=("sibuyan_landuse.tif",)):
    """The work folder (inputs copied in), the images' folder and the data's folder of a tutorial."""
    work = figs.workdir(f"{CATEGORY}/{slug}")
    for f in files:
        shutil.copy2(FILES[f], work / f)
    return work, figs.img_dir(f"{CATEGORY}/{slug}"), figs.data_dir(f"{CATEGORY}/{slug}")


def table(path) -> list[dict]:
    """A CSV written by terraflux run, numbers as floats (empty: None)."""
    out = []
    for r in figs.read_csv(path):
        o = {}
        for k, v in r.items():
            try:
                o[k] = float(v)
            except (TypeError, ValueError):
                o[k] = None if v in ("", None) else v
        out.append(o)
    return out


def md(rows: list[list], head: list[str]) -> str:
    """A Markdown table (printed by the scripts, pasted into the pages)."""
    s = "| " + " | ".join(head) + " |\n|" + "|".join("---:" if i else "---" for i in range(len(head))) + "|\n"
    for r in rows:
        s += "| " + " | ".join(str(x) for x in r) + " |\n"
    return s


def classes_map(a: np.ndarray, palette: dict, ax, alpha_mask=None):
    """A map of classes drawn on an axis (no value transparent)."""
    a = np.ma.masked_invalid(np.asarray(a, dtype=float))
    lut = np.zeros(a.shape + (4,))
    filled = a.filled(-9999)
    for v, (_, col) in palette.items():
        lut[filled == v] = mcolors.to_rgba(col)
    ax.imshow(lut, interpolation="nearest")
    ax.set_axis_off()


def legend_handles(palette: dict, order=None):
    order = order or list(palette)
    return [Patch(facecolor=palette[v][1], edgecolor="#00000022", label=palette[v][0]) for v in order]


def save(fig, png):
    png = pathlib.Path(png)
    png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.06)
    plt.close(fig)
    figs._shrink(png)
    return png
