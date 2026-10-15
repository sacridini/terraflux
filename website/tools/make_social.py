"""The picture shown when a page of the site is shared (static/img/social.png, 1200 x 630): the logo's cells
turning from forest to farming, the name and the tagline."""

import pathlib

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
from matplotlib.patches import FancyBboxPatch  # noqa: E402

WEB = pathlib.Path(__file__).resolve().parents[1]
OUT = WEB / "static" / "img" / "social.png"

rng = np.random.default_rng(7)
fig = plt.figure(figsize=(12, 6.3), dpi=100)
fig.patch.set_facecolor("#f4f6f3")
ax = fig.add_axes([0, 0, 1, 1])
ax.set_xlim(0, 1200)
ax.set_ylim(630, 0)
ax.set_axis_off()

# a field of cells at the right: forest, farming spreading from the lower right, a river
cols, rows, s = 19, 21, 27
x0, y0 = 660, 26
for r in range(rows):
    for c in range(cols):
        d = np.hypot(c - cols, r - rows) / np.hypot(cols, rows)
        river = abs(r - (6 + 3 * np.sin(c / 3.2))) < 1.0
        if river:
            col = "#3f7fae"
        elif d < 0.45 + rng.normal(0, 0.07):
            col = "#e8901f" if rng.random() < 0.55 else "#c9a35b"
        else:
            col = "#4c9f6a"
        alpha = 1.0 if col != "#4c9f6a" else 0.7 + 0.3 * rng.random()
        ax.add_patch(FancyBboxPatch((x0 + c * s + 2, y0 + r * s + 2), s - 4, s - 4,
                                    boxstyle="round,pad=0,rounding_size=4", fc=col, ec="none", alpha=alpha))

ax.add_patch(FancyBboxPatch((0, 0), 640, 630, boxstyle="square,pad=0", fc="#f4f6f3", ec="none"))
ax.text(64, 210, "Terraflux", fontsize=64, fontweight="bold", color="#1c2420", family="serif")
ax.text(66, 290, "Landscape ecology, conservation", fontsize=21, color="#48534d")
ax.text(66, 326, "planning and land change, open source.", fontsize=21, color="#48534d")
ax.text(66, 420, "Habitat patterns  ·  connectivity  ·  prioritization", fontsize=15, color="#2a6b44")
ax.text(66, 450, "species distributions  ·  land change", fontsize=15, color="#2a6b44")
fig.savefig(OUT, dpi=100, facecolor=fig.get_facecolor())
print(OUT)
