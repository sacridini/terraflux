"""An earlier date of the Sentinel-2 clip of the series, for part 11 (change by objects).

The same box, grid and pipeline as obia_data.py (data.stac straight onto EPSG:31983 at 10 m,
the extent 190520, 8241280, 196320, 8247080), for a dry-season scene of an earlier year, so that the two dates lie on
the same cells. The clip of 2024-08-09 is `obia_data.sentinel2()`.

    import obia_dates as dt
    early = dt.sentinel2_early()     # Path of the cached clip of the earlier date (website/.cache/obia/)
    model = dt.early_model()         # the TOML: data.stac `s2_early` (on the UTM grid), raster.calc `refl_early`

obia_data.py is only imported (its constants and helpers), never changed.
"""

from __future__ import annotations

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import obia_data as od  # noqa: E402

EARLY_DATE = "2019-08-13"  # Earth Search item S2A_22LHH_20190813_0_L2A (0.05 % cloud), processing baseline 02.13
EARLY_ITEM = "S2A_22LHH_20190813_0_L2A"
# the item of 2019-08-13 exists twice (baselines 02.13 and 05.00): `query` keeps the first, so the day is one scene
EARLY_QUERY = '{"s2:processing_baseline": {"eq": "02.13"}}'


def early_node(node_id: str = "s2_early") -> str:
    """The `data.stac` node of the earlier date: the box of obia_data, the day EARLY_DATE (output `<id>.raster`)."""
    bands = ", ".join(f'"{b}"' for b in od.S2_BANDS)
    q = EARLY_QUERY.replace('"', '\\"')
    return (
        f'[[nodes]]\nid = "{node_id}"\nop = "data.stac"\n'
        f'params = {{ url = "{od.S2_URL}", collection = "sentinel-2-l2a", assets = [{bands}], datetime = "{EARLY_DATE}", '
        f'query = "{q}", extent = {od.S2_UTM_EXTENT}, cellsize = {od.S2_CELL}, crs = "{od.S2_CRS}" }}\n'
    )


def early_model(with_reflectance: bool = True, suffix: str = "_early") -> str:
    """A model with `s2_early` (data.stac, on the UTM grid) and, unless `with_reflectance` is false,
    `refl_early` (reflectance 0 to 1, output `refl_early.image`)."""
    m = 'format = "terraflux/1"\n\n' + early_node("s2" + suffix)
    if with_reflectance:
        m += "\n" + od.reflectance_node("refl" + suffix, f"s2{suffix}.raster")
    return m


def sentinel2_early() -> pathlib.Path:
    """The earlier clip as a GeoTIFF (cached), layers blue green red nir swir16, reflectance x 10000."""
    tif = od.CACHE / "sentinel2_early.tif"
    if not tif.exists():
        work = od.CACHE / "work-s2-early"
        work.mkdir(parents=True, exist_ok=True)
        model = od.figs.add_writes(early_model(with_reflectance=False), {"s2_early.raster": "sentinel2_early.tif"})
        od.figs.run(model, work, "--threads", "2", "--no-journal")
        import rasterio

        with rasterio.open(work / "sentinel2_early.tif") as src:
            names = list(src.descriptions)
            data = src.read()
            prof = src.profile | {"compress": "deflate"}
        with rasterio.open(tif, "w", **prof) as out:
            out.write(data[[names.index(b) for b in od.S2_BANDS]])
            out.descriptions = tuple(od.S2_BANDS)
    return tif
