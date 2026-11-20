"""The labelled places of the Sentinel-2 clip, for parts 4 and 5 of the object-based image analysis series.

A figure run cannot click, so the training samples and the reference samples are drawn here, from places of the clip
that were looked at in true and false colour (the part-2 clip of obia_data.py): a box (rows and columns of the clip) for
each place, and a number of points drawn in it with a fixed seed. They stand for what a reader does with the
*Samples* tool of the Viewer (a click on an object) and are not field data: the labels are those of an analyst who
looked at the image.

The training places and the reference places never overlap (and no object is in both), so the accuracy of part 5 is
measured on objects the classifier did not see. `trap_places()` are the places of the autocorrelation experiment.

    import obia_samples as sm
    pts = sm.draw(sm.TRAIN, ids)        # [(class, row, col)] one point per object, from the map of object ids
    toml = sm.table_nodes("train", pts) # a table.of and a vector.from_table node (UTM 23S), as the application writes
"""

from __future__ import annotations

import numpy as np

X0, Y1, CELL = 190520.0, 8247080.0, 10.0  # the clip's top-left corner and cell (EPSG:31983)
CRS = "EPSG:31983"
CLASSES = ["bare soil", "cerrado", "forest", "urban", "water"]  # alphabetical, the order objects.samples numbers them
COLORS = {"water": "#1f6fd0", "forest": "#1a8a3c", "cerrado": "#d9b02e", "urban": "#c2338a", "bare soil": "#f0601a"}

# (class, (row0, row1, col0, col1), points drawn in it)
TRAIN = [
    ("water", (15, 40, 280, 340), 8), ("water", (95, 135, 248, 285), 6),
    ("forest", (330, 420, 218, 232), 10), ("forest", (385, 420, 385, 410), 6), ("forest", (225, 250, 188, 212), 6),
    ("cerrado", (530, 570, 200, 300), 8), ("cerrado", (450, 480, 290, 340), 6), ("cerrado", (235, 285, 395, 470), 8),
    ("urban", (60, 110, 60, 130), 10), ("urban", (270, 330, 260, 330), 8), ("urban", (110, 150, 420, 470), 8),
    ("bare soil", (297, 312, 385, 402), 3), ("bare soil", (356, 368, 252, 268), 3), ("bare soil", (362, 380, 180, 195), 3),
]
REFERENCE = [
    ("water", (10, 30, 400, 470), 8), ("water", (60, 85, 265, 310), 8),
    ("forest", (420, 480, 445, 475), 10), ("forest", (500, 540, 440, 470), 6),
    ("cerrado", (520, 570, 330, 410), 10), ("cerrado", (480, 520, 340, 400), 8),
    ("urban", (150, 200, 30, 100), 10), ("urban", (380, 430, 200, 260), 8), ("urban", (30, 70, 430, 480), 6),
    ("bare soil", (186, 200, 372, 386), 3), ("bare soil", (320, 345, 285, 300), 3),
]


def draw(places, ids: np.ndarray, seed: int = 7, taken: set | None = None):
    """[(class, row, col)]: points drawn in the boxes, one per object (`ids`: the map of object ids, 0 or -1 for none),
    none in an object of `taken` (a set that is filled with the objects used)."""
    rng = np.random.default_rng(seed)
    taken = set() if taken is None else taken
    out = []
    for cls, (r0, r1, c0, c1), n in places:
        got, tries = 0, 0
        while got < n and tries < 400:
            tries += 1
            r, c = int(rng.integers(r0, r1)), int(rng.integers(c0, c1))
            k = int(ids[r, c])
            if k <= 0 or k in taken:
                continue
            taken.add(k)
            out.append((cls, r, c))
            got += 1
    return out


def xy(row: int, col: int):
    """The map coordinates of the centre of a cell."""
    return X0 + (col + 0.5) * CELL, Y1 - (row + 0.5) * CELL


def table_nodes(prefix: str, pts) -> str:
    """A `table.of` (columns x, y, class) and the `vector.from_table` after it, as the application writes samples."""
    rows = ", ".join(f'[{xy(r, c)[0]:.1f}, {xy(r, c)[1]:.1f}, "{cls}"]' for cls, r, c in pts)
    return (
        f'\n[[nodes]]\nid = "{prefix}_table"\nop = "table.of"\n'
        f'params = {{ columns = ["x", "y", "class"], rows = [{rows}] }}\n'
        f'\n[[nodes]]\nid = "{prefix}"\nop = "vector.from_table"\ninputs = {{ table = "{prefix}_table.table" }}\n'
        f'params = {{ crs = "{CRS}" }}\n'
    )
