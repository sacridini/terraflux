"""Helpers of the two case-study pages (parts 14 and 15) of the object-based image analysis series.

They do not run Terraflux: they draw the image chips a person reads to label a reference point, and read the labels
back. A figure script cannot click, so the reference of each case study is made like this:

1. the script draws the points (objects.sample, a seed, so the same points every run) and writes them to a CSV;
2. `chip_sheets` draws, for each point, a window of the image around it (and only the image: no map, no class), a
   cross at the point, and a number;
3. a person reads the sheets and writes one label per number into a labels file (`labels/<name>.csv`, columns
   `n,label`), kept in the repository beside the script;
4. the script reads the labels and gives them to objects.accuracy.

The labels are therefore the reading of an analyst who looked at the image only, blind to the map's class. They are
not field data and not independent of the image the map came from; the pages say so.
"""

from __future__ import annotations

import csv
import pathlib

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402


def read_labels(path: pathlib.Path) -> dict[int, str]:
    """{n: label} from a labels CSV (columns n and label)."""
    with open(path, newline="", encoding="utf-8") as f:
        return {int(r["n"]): r["label"].strip() for r in csv.DictReader(f)}


def chip_sheets(rgb: np.ndarray, pts: list[tuple[int, int]], out_dir: pathlib.Path, prefix: str, half: int = 32,
                per_sheet: int = 25, cols: int = 5, zoom: int = 4, first: int = 1, rgb2: np.ndarray | None = None) -> list[pathlib.Path]:
    """Contact sheets of image chips. `rgb` is (rows, cols, 3) in 0..1, `pts` are (row, col) of the cell of each point;
    the chip is the window of +-`half` cells around it with a small cross at the point. With `rgb2` (a second rendering
    of the same grid, say false colour) each chip is two panels side by side. The points are numbered from `first`.
    Returns the PNGs."""
    out_dir.mkdir(parents=True, exist_ok=True)
    rows_img, cols_img = rgb.shape[:2]
    panels = [rgb] if rgb2 is None else [rgb, rgb2]
    paths = []
    for s in range(0, len(pts), per_sheet):
        batch = pts[s:s + per_sheet]
        nrow = -(-len(batch) // cols)
        np_ = len(panels)
        fig, axes = plt.subplots(nrow, cols * np_, figsize=(cols * 2.1 * np_, nrow * 2.25), dpi=100)
        axes = np.atleast_2d(axes)
        for ax in axes.flat:
            ax.axis("off")
        for k, (r, c) in enumerate(batch):
            r0, r1 = max(0, r - half), min(rows_img, r + half)
            c0, c1 = max(0, c - half), min(cols_img, c + half)
            for j, img in enumerate(panels):
                ax = axes[k // cols, (k % cols) * np_ + j]
                ax.imshow(img[r0:r1, c0:c1], extent=(c0, c1, r1, r0), interpolation="bicubic" if zoom else "nearest")
                ax.plot([c], [r], marker="+", color="#ffff00", ms=9, mew=1.2)
                if j == 0:
                    ax.set_title(str(first + s + k), fontsize=9, pad=2, color="black", loc="left")
        fig.subplots_adjust(left=0.01, right=0.99, top=0.96, bottom=0.01, wspace=0.03, hspace=0.18)
        p = out_dir / f"{prefix}_{first + s:03d}.png"
        fig.savefig(p, dpi=100)
        plt.close(fig)
        paths.append(p)
    return paths
