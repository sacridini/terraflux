"""Shared pieces of the figures of the landscape metrics tutorials: the real maps (the landscapemetrics sample of
Augusta, Georgia, and the CORINE excerpt of landschaft), their colours, and small helpers to run a model and read
the tables it writes."""

from __future__ import annotations

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

ROOT = figs.ROOT
AUGUSTA = ROOT / "extern/landschaft/tests/data/lsm/augusta.tif"
CLC = ROOT / "extern/landschaft/src/landschaft/data/clc.tif"

# the National Land Cover Database's classes and official colours (augusta.tif)
NLCD = {
    11: ("Open water", "#466b9f"),
    21: ("Developed, open space", "#dec5c5"),
    22: ("Developed, low intensity", "#d99282"),
    23: ("Developed, medium intensity", "#eb0000"),
    24: ("Developed, high intensity", "#ab0000"),
    31: ("Barren land", "#b3ac9f"),
    41: ("Deciduous forest", "#68ab5f"),
    42: ("Evergreen forest", "#1c5f2c"),
    43: ("Mixed forest", "#b5c58f"),
    52: ("Shrub/scrub", "#ccb879"),
    71: ("Grassland", "#dfdfc2"),
    81: ("Pasture/hay", "#dcd939"),
    82: ("Cultivated crops", "#ab6c28"),
    90: ("Woody wetlands", "#b8d9eb"),
    95: ("Emergent wetlands", "#6c9fb8"),
}
# NLCD's level 1 (the rules of raster.reclassify, and the colours)
NLCD1_RULES = "11 -> 1\n21 22 23 24 -> 2\n31 -> 3\n41 42 43 -> 4\n52 -> 5\n71 -> 6\n81 82 -> 7\n90 95 -> 8"
NLCD1 = {1: ("Water", "#466b9f"), 2: ("Developed", "#d0574a"), 3: ("Barren", "#b3ac9f"), 4: ("Forest", "#3b8a5a"),
         5: ("Shrub/scrub", "#ccb879"), 6: ("Grassland", "#dfdfc2"), 7: ("Agriculture", "#dcc339"),
         8: ("Wetlands", "#8fbcd6")}

# CORINE Land Cover, the classes of clc.tif (1-44, CLC's level 3 in order) and CLC's colours
CLC_NAMES = {
    1: ("Continuous urban fabric", "#e6004d"), 2: ("Discontinuous urban fabric", "#ff0000"),
    3: ("Industrial or commercial units", "#cc4df2"), 4: ("Road and rail networks", "#cc0000"),
    5: ("Port areas", "#e6cccc"), 6: ("Airports", "#e6cce6"), 7: ("Mineral extraction sites", "#a600cc"),
    8: ("Dump sites", "#a64d00"), 9: ("Construction sites", "#ff4dff"), 10: ("Green urban areas", "#ffa6ff"),
    11: ("Sport and leisure facilities", "#ffe6ff"), 12: ("Non-irrigated arable land", "#ffffa8"),
    18: ("Pastures", "#e6e64d"), 20: ("Complex cultivation patterns", "#ffe64d"),
    21: ("Agriculture with natural vegetation", "#e6cc4d"), 23: ("Broad-leaved forest", "#80ff00"),
    24: ("Coniferous forest", "#00a600"), 25: ("Mixed forest", "#4dff00"), 26: ("Natural grasslands", "#ccf24d"),
    27: ("Moors and heathland", "#a6ff80"), 29: ("Transitional woodland-shrub", "#a6f200"),
    32: ("Sparsely vegetated areas", "#ccffcc"), 35: ("Inland marshes", "#a6a6ff"), 36: ("Peat bogs", "#4d4dff"),
    40: ("Water courses", "#00ccf2"), 41: ("Water bodies", "#80f2e6"),
}
# CORINE in seven broad classes
CLC7_RULES = "1..11 -> 1\n12 -> 2\n13..22 -> 3\n23..25 -> 4\n26..34 -> 5\n35..39 -> 6\n40..44 -> 7"
CLC7 = {1: ("Urban", "#c8384a"), 2: ("Arable land", "#f1e08a"), 3: ("Pasture, mixed farming", "#c9c94d"),
        4: ("Forest", "#3b8a5a"), 5: ("Heath, grass, shrub", "#a6d96a"), 6: ("Wetlands, bogs", "#7b7fd6"),
        7: ("Water", "#3f7fae")}


def work(name: str) -> pathlib.Path:
    """A clean scratch folder for a tutorial, with the real maps copied in."""
    w = figs.workdir(f"landscape-metrics/{name}")
    shutil.copy(AUGUSTA, w / "augusta.tif")
    shutil.copy(CLC, w / "clc.tif")
    return w


def write_tif(path: pathlib.Path, a: np.ndarray, res: float = 30.0, nodata=None, crs=None, x0=0.0, y0=None):
    import rasterio
    from rasterio.transform import from_origin

    a = np.asarray(a)
    y0 = a.shape[0] * res if y0 is None else y0
    prof = dict(driver="GTiff", height=a.shape[0], width=a.shape[1], count=1, dtype=a.dtype.name,
                transform=from_origin(x0, y0, res, res))
    if nodata is not None:
        prof["nodata"] = nodata
    if crs is not None:
        prof["crs"] = crs
    with rasterio.open(path, "w", **prof) as ds:
        ds.write(a, 1)


def run(model: str, w: pathlib.Path, writes: dict[str, str], name="model.toml"):
    """Runs a model with its maps and tables written (figs.add_writes); tables land in w/output."""
    figs.run(figs.add_writes(model, writes), w, name=name)


def table(w: pathlib.Path, name: str) -> list[dict]:
    """A table the run wrote (w/output/name.csv), its numbers as floats (NA as nan)."""
    rows = figs.read_csv(w / "output" / f"{name}.csv")
    out = []
    for r in rows:
        o = {}
        for k, v in r.items():
            try:
                o[k] = float(v)
            except ValueError:
                o[k] = float("nan") if v in ("", "NA", "NaN", "nan") else v
        out.append(o)
    return out


def node(id: str, op: str, inputs: dict | None = None, params: dict | None = None) -> str:
    """A node of a model in TOML."""
    import json

    def val(v):
        if isinstance(v, bool):
            return "true" if v else "false"
        if isinstance(v, str):
            return json.dumps(v)
        if isinstance(v, (list, tuple)):
            return "[" + ", ".join(val(x) for x in v) + "]"
        return repr(v)

    s = f'\n[[nodes]]\nid = "{id}"\nop = "{op}"\n'
    if inputs:
        s += "inputs = { " + ", ".join(f'{k} = "{v}"' if isinstance(v, str) else f"{k} = {val(v)}"
                                       for k, v in inputs.items()) + " }\n"
    if params:
        s += "params = { " + ", ".join(f"{k} = {val(v)}" for k, v in params.items()) + " }\n"
    return s


HEAD = 'format = "terraflux/1"\n'
