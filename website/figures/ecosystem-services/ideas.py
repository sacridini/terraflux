"""Figures of 'Ecosystem service models: the ideas' (ecosystem-services/ideas).

Ribeirão Claro's maps (land use of 1985, 2020 and 2040, elevation), the four models run on the land use of 2020
and of 2040 with their headline numbers, and Terraflux against InVEST 3.20.2 on InVEST's own runs (landschaft's
reference cases): the maps cell by cell.
"""

import json

import numpy as np

import es_common as es
from es_common import figs, plt

SLUG = "ideas"
OUT = es.img(SLUG)
DAT = es.data(SLUG)
REF = figs.ROOT / "tests" / "data" / "reference"
INVEST = figs.ROOT / "extern" / "landschaft" / "tests" / "data" / "reference" / "invest"

THREATS = ",\n".join(
    f'  {{ threat = "{t["threat"]}", max_dist = {t["max_dist"]}, weight = {t["weight"]}, decay = "{t["decay"]}" }}'
    for t in es.THREATS)

FOUR = '''format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = {{ path = "{lulc}" }}

[[nodes]]
id = "pools"
op = "table.read"
params = {{ path = "carbon_pools.csv" }}

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = {{ lulc = "lulc.raster", pools = "pools.table" }}

[[nodes]]
id = "crops"
op = "raster.read"
params = {{ path = "crops_{y}.tif" }}

[[nodes]]
id = "roads"
op = "raster.read"
params = {{ path = "roads_{y}.tif" }}

[[nodes]]
id = "urban"
op = "raster.read"
params = {{ path = "urban_{y}.tif" }}

[[nodes]]
id = "threat_maps"
op = "stack.of"
inputs = {{ crops = "crops.raster", roads = "roads.raster", urban = "urban.raster" }}

[[nodes]]
id = "sensitivity"
op = "table.read"
params = {{ path = "sensitivity.csv" }}

[[nodes]]
id = "habitat"
op = "services.habitat_quality"
inputs = {{ lulc = "lulc.raster", sensitivity = "sensitivity.table", threat_maps = "threat_maps.stack" }}
params = {{ half_saturation = 0.35, threats = [
{threats}
] }}

[[nodes]]
id = "watersheds"
op = "vector.read"
params = {{ path = "watersheds.gpkg" }}

[[nodes]]
id = "bio_wy"
op = "table.read"
params = {{ path = "biophysical_wy.csv" }}

[[nodes]]
id = "precipitation"
op = "raster.read"
params = {{ path = "precipitation.tif" }}

[[nodes]]
id = "eto"
op = "raster.read"
params = {{ path = "eto.tif" }}

[[nodes]]
id = "depth"
op = "raster.read"
params = {{ path = "depth.tif" }}

[[nodes]]
id = "pawc"
op = "raster.read"
params = {{ path = "pawc.tif" }}

[[nodes]]
id = "water"
op = "services.water_yield"
inputs = {{ lulc = "lulc.raster", biophysical = "bio_wy.table", precipitation = "precipitation.raster", eto = "eto.raster", depth = "depth.raster", pawc = "pawc.raster", watersheds = "watersheds.vector" }}
params = {{ seasonality = 10 }}

[[nodes]]
id = "dem"
op = "raster.read"
params = {{ path = "dem.tif" }}

[[nodes]]
id = "erosivity"
op = "raster.read"
params = {{ path = "erosivity.tif" }}

[[nodes]]
id = "erodibility"
op = "raster.read"
params = {{ path = "erodibility.tif" }}

[[nodes]]
id = "bio_sdr"
op = "table.read"
params = {{ path = "biophysical_sdr.csv" }}

[[nodes]]
id = "sediment"
op = "services.sdr"
inputs = {{ dem = "dem.raster", erosivity = "erosivity.raster", erodibility = "erodibility.raster", lulc = "lulc.raster", biophysical = "bio_sdr.table", watersheds = "watersheds.vector" }}
params = {{ threshold = 1000 }}
'''


def landscape():
    d = es.landscape()
    es.zip_landscape(DAT / "ribeirao-claro.zip")
    for y in (1985, 2020, 2040):
        es.land_map(es.grid(d / f"lulc_{y}.tif"), OUT / f"lulc_{y}.png", counts=True, width=820)
        if y != 1985:
            es.land_map(es.grid(d / f"lulc_{y}.tif"), OUT / f"lulc_{y}_plain.png", legend=False, width=760)
    # elevation over a hillshade, with the rivers and the watersheds
    import geopandas as gpd

    dem = es.grid(d / "dem.tif")
    ls = mcol_hillshade(dem)
    river = np.load(d / "river.npy")
    ws = gpd.read_file(d / "watersheds.gpkg")
    fig = plt.figure(figsize=(8.4, 4.9), dpi=150)
    ax = fig.add_axes([0, 0, 0.8, 1])
    ext = (es.ORIGIN[0], es.ORIGIN[0] + es.W * es.PIXEL, es.ORIGIN[1] - es.H * es.PIXEL, es.ORIGIN[1])
    ax.imshow(ls, cmap="gray", extent=ext, vmin=0, vmax=1)
    im = ax.imshow(dem, cmap="terrain", alpha=0.6, extent=ext, vmin=450, vmax=1150)
    ax.imshow(np.where(river, 1, np.nan), cmap=plt.matplotlib.colors.ListedColormap([figs.WATER]), extent=ext,
              interpolation="nearest")
    ws.boundary.plot(ax=ax, color=figs.INK, lw=1.3)
    for _, r in ws.iterrows():
        c = r.geometry.representative_point()
        ax.text(c.x, c.y, f"watershed {r.ws_id}", ha="center", fontsize=10, color=figs.INK, fontweight="bold",
                bbox=dict(boxstyle="round,pad=0.2", fc="white", ec="none", alpha=0.75))
    ax.set_axis_off()
    cax = fig.add_axes([0.83, 0.12, 0.03, 0.76])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.ax.grid(False)
    cb.set_label("Elevation (m)", color=figs.INK2)
    fig.savefig(OUT / "terrain.png", dpi=150, bbox_inches="tight", pad_inches=0.05)
    plt.close(fig)
    figs._shrink(OUT / "terrain.png")


def mcol_hillshade(z, az=315, alt=45):
    dy, dx = np.gradient(z, es.PIXEL)
    slope = np.pi / 2 - np.arctan(np.hypot(dx, dy))
    aspect = np.arctan2(-dx, dy)
    a, b = np.radians(az), np.radians(alt)
    hs = np.sin(b) * np.sin(slope) + np.cos(b) * np.cos(slope) * np.cos(a - aspect)
    return (hs - hs.min()) / (hs.max() - hs.min())


def four_services():
    """The four models on 2020 and 2040: maps of 2020 and the headline numbers."""
    rows = []
    for y in (2020, 2040):
        w = es.work(SLUG, f"four_{y}")
        es.copy_inputs(w)
        model = FOUR.format(lulc=f"lulc_{y}.tif", y=y, threats=THREATS)
        es.run(model, w, {"carbon.storage": "carbon.tif", "habitat.quality": "quality.tif",
                          "water.wyield": "wyield.tif", "sediment.sed_export": "export.tif",
                          "carbon.totals": "carbon.csv", "water.watershed_table": "water.csv",
                          "sediment.watershed_table": "sediment.csv"})
        lu = es.grid(w / f"lulc_{y}.tif")
        q = es.grid(w / "quality.tif")
        carbon = float(figs.read_csv(w / "output" / "carbon.csv")[0]["baseline"])
        water = sum(float(r["wyield_vol"]) for r in figs.read_csv(w / "output" / "water.csv"))
        sed = sum(float(r["sed_export"]) for r in figs.read_csv(w / "output" / "sediment.csv"))
        rows.append({"year": y, "carbon": carbon, "quality": float(np.nanmean(q[lu == 1])),
                     "habitat_quality_all": float(np.nanmean(q[np.isin(lu, [1, 2, 4])])),
                     "water": water / 1e6, "sediment": sed})
        if y == 2020:
            figs.raster(w / "carbon.tif", OUT / "carbon.png", cmap="YlGn", unit="t C / ha", width=560)
            figs.raster(w / "quality.tif", OUT / "quality.png", cmap="viridis", unit="quality (0 to 1)",
                        width=560, vmin=0, vmax=1)
            figs.raster(w / "wyield.tif", OUT / "wyield.png", cmap="Blues", unit="mm / year", width=560)
            figs.raster(w / "export.tif", OUT / "export.png", cmap="magma_r", unit="t / ha / year", width=560,
                        log=True, vmin=0.001)
    a, b = rows
    table = [
        {"service": "Carbon stored (t C)", "2020": round(a["carbon"]), "2040": round(b["carbon"]),
         "change": f'{(b["carbon"] / a["carbon"] - 1) * 100:+.1f}%'},
        {"service": "Habitat quality, mean of the forest", "2020": round(a["quality"], 3),
         "2040": round(b["quality"], 3), "change": f'{(b["quality"] / a["quality"] - 1) * 100:+.1f}%'},
        {"service": "Water yield of the two watersheds (million m³ / year)", "2020": round(a["water"], 2),
         "2040": round(b["water"], 2), "change": f'{(b["water"] / a["water"] - 1) * 100:+.1f}%'},
        {"service": "Sediment exported to the rivers (t / year)", "2020": round(a["sediment"]),
         "2040": round(b["sediment"]), "change": f'{(b["sediment"] / a["sediment"] - 1) * 100:+.1f}%'},
    ]
    figs.write_csv(DAT / "four-services.csv", table)
    print(json.dumps(table, indent=1))


# ------------------------------------------------------------------------------------------------ against InVEST


def _write_float(path, a, origin=es.ORIGIN):
    import rasterio
    from rasterio.transform import from_origin

    prof = dict(driver="GTiff", height=a.shape[0], width=a.shape[1], count=1, dtype="float64", crs=es.CRS,
                transform=from_origin(origin[0], origin[1], 30, 30), nodata=np.nan)
    with rasterio.open(path, "w", **prof) as ds:
        ds.write(a.astype("float64"), 1)


def _rings_gpkg(path, rings, field):
    import geopandas as gpd
    from shapely.geometry import Polygon

    gpd.GeoDataFrame({field: list(range(1, len(rings) + 1))}, geometry=[Polygon(r) for r in rings],
                     crs=es.CRS).to_file(path, driver="GPKG")


def _table_csv(path, t):
    es.write_table(path, t)


def _compare(ours: np.ndarray, theirs: np.ndarray):
    """Cells with a value in both; identical float32 values; the largest difference."""
    ok = ~np.isnan(ours)
    o = ours[ok].astype("float32")
    t = theirs[ok].astype("float32")
    same = int(np.sum((o == t) | (np.isnan(o) & np.isnan(t))))
    diff = float(np.nanmax(np.abs(o.astype("float64") - t.astype("float64")))) if o.size else 0.0
    return int(ok.sum()), same, diff


def against_invest():
    ref = np.load(REF / "services.npz")
    meta = json.loads((REF / "services.json").read_text(encoding="utf-8"))["cases"]
    rows = []

    # carbon with valuation
    case = "carbon_valuation"
    w = es.work(SLUG, "invest_" + case)
    inv = np.load(INVEST / f"{case}.npz")
    _write_float(w / "lulc.tif", ref[f"{case}/in/lulc"])
    _write_float(w / "future.tif", ref[f"{case}/in/future"])
    _table_csv(w / "pools.csv", meta[case]["pools"])
    p = meta[case]["params"]
    model = f'''format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = {{ path = "lulc.tif" }}

[[nodes]]
id = "future"
op = "raster.read"
params = {{ path = "future.tif" }}

[[nodes]]
id = "pools"
op = "table.read"
params = {{ path = "pools.csv" }}

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = {{ lulc = "lulc.raster", pools = "pools.table", future = "future.raster" }}
params = {{ price = {p["price"]}, discount_rate = {p["discount_rate"]}, rate_change = {p["rate_change"]}, year = {p["year"]}, future_year = {p["future_year"]} }}
'''
    es.run(model, w, {"carbon.storage": "storage.tif", "carbon.change": "change.tif", "carbon.npv": "npv.tif"})
    for ours, theirs, label in (("storage", "c_storage_bas", "carbon storage"), ("change", "c_change_bas_alt",
                                "sequestration"), ("npv", "npv_alt", "net present value")):
        n, same, diff = _compare(es.grid(w / f"{ours}.tif"), inv[f"out/{theirs}"])
        rows.append({"model": "services.carbon", "map": label, "cells": n, "identical": same, "max_diff": diff})

    # habitat quality with a future and a base
    case = "hq_scenarios"
    w = es.work(SLUG, "invest_" + case)
    inv = np.load(INVEST / f"{case}.npz")
    for k in ("lulc", "future", "base"):
        _write_float(w / f"{k}.tif", ref[f"{case}/in/{k}"])
    for t in ("crops", "roads", "urban"):
        _write_float(w / f"{t}.tif", ref[f"{case}/in/threat_maps/{t}"])
        _write_float(w / f"{t}_f.tif", ref[f"{case}/in/future_threat_maps/{t}"])
    _table_csv(w / "sensitivity.csv", meta[case]["sensitivity"])
    th = ",\n".join(f'  {{ threat = "{t["threat"]}", max_dist = {t["max_dist"]}, weight = {t["weight"]}, '
                    f'decay = "{t["decay"]}" }}' for t in meta[case]["threats"])
    nodes = "".join(f'''
[[nodes]]
id = "{k}"
op = "raster.read"
params = {{ path = "{k}.tif" }}
''' for k in ("lulc", "future", "base", "crops", "roads", "urban", "crops_f", "roads_f", "urban_f"))
    model = f'''format = "terraflux/1"
{nodes}
[[nodes]]
id = "sens"
op = "table.read"
params = {{ path = "sensitivity.csv" }}

[[nodes]]
id = "now"
op = "stack.of"
inputs = {{ crops = "crops.raster", roads = "roads.raster", urban = "urban.raster" }}

[[nodes]]
id = "later"
op = "stack.of"
inputs = {{ crops = "crops_f.raster", roads = "roads_f.raster", urban = "urban_f.raster" }}

[[nodes]]
id = "hq"
op = "services.habitat_quality"
inputs = {{ lulc = "lulc.raster", sensitivity = "sens.table", threat_maps = "now.stack", future = "future.raster", future_threat_maps = "later.stack", base = "base.raster" }}
params = {{ half_saturation = {meta[case]["k"]}, threats = [
{th}
] }}
'''
    es.run(model, w, {"hq.quality": "q.tif", "hq.degradation": "d.tif", "hq.quality_future": "qf.tif",
                      "hq.rarity": "r.tif"})
    for ours, theirs, label in (("d", "deg_sum_c", "degradation"), ("q", "quality_c", "quality"),
                                ("qf", "quality_f", "quality, future"), ("r", "rarity_c", "rarity")):
        n, same, diff = _compare(es.grid(w / f"{ours}.tif"), inv[f"out/{theirs}"])
        rows.append({"model": "services.habitat_quality", "map": label, "cells": n, "identical": same,
                     "max_diff": diff})

    # annual water yield
    case = "wy_full"
    w = es.work(SLUG, "invest_" + case)
    inv = np.load(INVEST / f"{case}.npz")
    for k in ("lulc", "precipitation", "eto", "depth", "pawc"):
        _write_float(w / f"{k}.tif", ref[f"{case}/in/{k}"])
    _table_csv(w / "bio.csv", meta[case]["biophysical"])
    model = f'''format = "terraflux/1"
''' + "".join(f'''
[[nodes]]
id = "{k}"
op = "raster.read"
params = {{ path = "{k}.tif" }}
''' for k in ("lulc", "precipitation", "eto", "depth", "pawc")) + f'''
[[nodes]]
id = "bio"
op = "table.read"
params = {{ path = "bio.csv" }}

[[nodes]]
id = "wy"
op = "services.water_yield"
inputs = {{ lulc = "lulc.raster", biophysical = "bio.table", precipitation = "precipitation.raster", eto = "eto.raster", depth = "depth.raster", pawc = "pawc.raster" }}
params = {{ seasonality = {meta[case]["z"]}, precision = "{meta[case]["precision"]}" }}
'''
    es.run(model, w, {"wy.wyield": "wyield.tif", "wy.aet": "aet.tif", "wy.fractp": "fractp.tif"})
    for ours, label in (("fractp", "AET / P"), ("aet", "actual evapotranspiration"), ("wyield", "water yield")):
        n, same, diff = _compare(es.grid(w / f"{ours}.tif"), inv[f"out/{ours}"])
        rows.append({"model": "services.water_yield", "map": label, "cells": n, "identical": same,
                     "max_diff": diff})

    # sediment delivery ratio, multiple flow directions
    case = "sdr_mfd"
    w = es.work(SLUG, "invest_" + case)
    inv = np.load(INVEST / f"{case}.npz")
    for k in ("dem", "erosivity", "erodibility", "lulc"):
        _write_float(w / f"{k}.tif", ref[f"{case}/in/{k}"])
    _table_csv(w / "bio.csv", meta[case]["biophysical"])
    _rings_gpkg(w / "ws.gpkg", meta[case]["watersheds"], "ws_id")
    p = meta[case]["params"]
    model = f'''format = "terraflux/1"
''' + "".join(f'''
[[nodes]]
id = "{k}"
op = "raster.read"
params = {{ path = "{k}.tif" }}
''' for k in ("dem", "erosivity", "erodibility", "lulc")) + f'''
[[nodes]]
id = "bio"
op = "table.read"
params = {{ path = "bio.csv" }}

[[nodes]]
id = "ws"
op = "vector.read"
params = {{ path = "ws.gpkg" }}

[[nodes]]
id = "sdr"
op = "services.sdr"
inputs = {{ dem = "dem.raster", erosivity = "erosivity.raster", erodibility = "erodibility.raster", lulc = "lulc.raster", biophysical = "bio.table", watersheds = "ws.vector" }}
params = {{ threshold = {p["threshold"]}, k = {p["k"]}, ic_0 = {p["ic_0"]}, sdr_max = {p["sdr_max"]}, l_max = {p["l_max"]}, method = "{p["method"]}" }}
'''
    es.run(model, w, {f"sdr.{k}": f"{k}.tif" for k in ("usle", "ic", "sdr", "sed_export", "sed_deposition")})
    for ours, theirs, label in (("usle", "usle", "soil loss (USLE)"), ("ic", "ic", "index of connectivity"),
                                ("sdr", "sdr_factor", "sediment delivery ratio"),
                                ("sed_export", "sed_export", "sediment export"),
                                ("sed_deposition", "sed_deposition", "sediment deposition")):
        n, same, diff = _compare(es.grid(w / f"{ours}.tif"), inv[f"out/{theirs}"])
        rows.append({"model": "services.sdr", "map": label, "cells": n, "identical": same, "max_diff": diff})
    for r in rows:
        print(r)
    figs.write_csv(DAT / "against-invest.csv", rows)


if __name__ == "__main__":
    landscape()
    four_services()
    against_invest()
