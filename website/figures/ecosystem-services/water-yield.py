"""Figures of 'Annual water yield' (ecosystem-services/water-yield).

services.water_yield on Ribeirão Claro: Budyko's curve in Fu and Zhang's form, the inputs, the maps of the water
balance, the tables by watershed with the demand and the hydropower, the seasonality Z, the scenario of 2040, and
float32 against float64.
"""

import numpy as np

import es_common as es
from es_common import figs, plt

SLUG = "water-yield"
OUT = es.img(SLUG)
DAT = es.data(SLUG)
INPUTS = ["lulc_2020.tif", "lulc_2040.tif", "biophysical_wy.csv", "precipitation.tif", "eto.tif", "depth.tif",
          "pawc.tif", "watersheds.gpkg", "subwatersheds.gpkg", "demand.csv", "valuation.csv"]


def model(lulc="lulc_2020.tif", z=10, demand=True, sub=True, precision=None):
    head = f'''format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = {{ path = "{lulc}" }}

[[nodes]]
id = "biophysical"
op = "table.read"
params = {{ path = "biophysical_wy.csv" }}
''' + "".join(f'''
[[nodes]]
id = "{k}"
op = "raster.read"
params = {{ path = "{k}.tif" }}
''' for k in ("precipitation", "eto", "depth", "pawc")) + '''
[[nodes]]
id = "watersheds"
op = "vector.read"
params = { path = "watersheds.gpkg" }
'''
    ins = {"lulc": "lulc.raster", "biophysical": "biophysical.table", "precipitation": "precipitation.raster",
           "eto": "eto.raster", "depth": "depth.raster", "pawc": "pawc.raster", "watersheds": "watersheds.vector"}
    if sub:
        head += '''
[[nodes]]
id = "subwatersheds"
op = "vector.read"
params = { path = "subwatersheds.gpkg" }
'''
        ins["subwatersheds"] = "subwatersheds.vector"
    if demand:
        head += '''
[[nodes]]
id = "demand"
op = "table.read"
params = { path = "demand.csv" }

[[nodes]]
id = "valuation"
op = "table.read"
params = { path = "valuation.csv" }
'''
        ins.update(demand="demand.table", valuation="valuation.table")
    p = f"seasonality = {z}" + (f', precision = "{precision}"' if precision else "")
    head += f'''
[[nodes]]
id = "water"
op = "services.water_yield"
inputs = {{ {", ".join(f'{k} = "{v}"' for k, v in ins.items())} }}
params = {{ {p} }}
'''
    return head


def budyko():
    x = np.linspace(0.01, 4, 400)

    def draw(ax):
        ax.plot(x, np.minimum(x, 1), color=figs.INK2, lw=1.2, ls="--", label="limits: AET = PET, AET = P")
        for w, col in zip([1.25, 1.5, 2, 3, 5], figs.CLASSES):
            ax.plot(x, 1 + x - (1 + x ** w) ** (1 / w), color=col, lw=2.2, label=f"ω = {w}")
        ax.set_xlabel("Dryness: PET / P")
        ax.set_ylabel("Share of rain evaporated: AET / P")
        ax.set_xlim(0, 4)
        ax.set_ylim(0, 1.05)
        ax.axvline(1, color=figs.INK2, lw=1, ls=":"); ax.text(0.7, 0.03, "energy\nlimited", fontsize=9, color=figs.INK2, ha="center")
        ax.text(2.5, 0.04, "water limited", fontsize=9.5, color=figs.INK2, ha="center")
        ax.legend(loc="center left", bbox_to_anchor=(1.01, 0.5))
    figs.chart(OUT / "budyko.png", draw, width=900, height=440)


def main():
    budyko()
    w = es.work(SLUG, "main")
    es.copy_inputs(w, INPUTS)
    es.run(model(), w, {"water.wyield": "wyield.tif", "water.aet": "aet.tif", "water.pet": "pet.tif",
                        "water.fractp": "fractp.tif", "water.demand_map": "demand.tif",
                        "water.watershed_table": "watersheds.csv", "water.subwatershed_table": "sub.csv"})
    for k, cmap, unit in (("precipitation", "Blues", "mm / year"), ("eto", "YlOrRd", "mm / year"),
                          ("depth", "copper_r", "mm"), ("pawc", "YlGnBu", "fraction")):
        figs.raster(w / f"{k}.tif", OUT / f"{k}.png", cmap=cmap, unit=unit, width=520)
    figs.raster(w / "pet.tif", OUT / "pet.png", cmap="YlOrRd", unit="mm / year", width=520)
    figs.raster(w / "aet.tif", OUT / "aet.png", cmap="YlGn", unit="mm / year", width=520)
    figs.raster(w / "fractp.tif", OUT / "fractp.png", cmap="viridis", unit="AET / P", width=520, vmin=0, vmax=1)
    figs.raster(w / "wyield.tif", OUT / "wyield.png", cmap="Blues", unit="mm / year", width=520)
    figs.raster(w / "demand.tif", OUT / "demand.png", cmap="Oranges", unit="m³ / year per cell", log=False)

    def rows(path, digits=1):
        out = []
        for r in figs.read_csv(path):
            out.append({k: (round(float(v), digits) if v not in ("", None) else "") for k, v in r.items()})
        return out
    def shown(path):
        """The table as the page shows it: volumes in million m³, energy in MWh, value in thousands."""
        out = []
        for r in figs.read_csv(path):
            d = {}
            for k, v in r.items():
                v = float(v)
                if k in ("ws_id", "subws_id"):
                    d[k] = int(v)
                elif k in ("wyield_vol", "consum_vol", "rsupply_vl"):
                    d[k] = round(v / 1e6, 3)
                elif k == "hp_energy":
                    d[k] = round(v / 1e3, 1)
                elif k == "hp_val":
                    d[k] = round(v / 1e3, 1)
                else:
                    d[k] = round(v, 1)
            out.append(d)
        return out
    figs.write_csv(DAT / "watersheds.csv", shown(w / "output" / "watersheds.csv"))
    figs.write_csv(DAT / "subwatersheds.csv", shown(w / "output" / "sub.csv"))
    print(figs.read_csv(w / "output" / "watersheds.csv"))

    # yield by class
    lu = es.grid(w / "lulc_2020.tif")
    wy, aet, p = es.grid(w / "wyield.tif"), es.grid(w / "aet.tif"), es.grid(w / "precipitation.tif")
    byc = []
    for k, (name, _) in es.CLASSES.items():
        m = lu == k
        byc.append({"class": name, "precipitation": round(float(np.nanmean(p[m]))),
                    "aet": round(float(np.nanmean(aet[m]))), "wyield": round(float(np.nanmean(wy[m]))),
                    "aet_share": round(float(np.nanmean(aet[m] / p[m])), 2)})
    figs.write_csv(DAT / "by-class.csv", byc)
    print(byc)

    # the seasonality Z
    zs = [1, 3, 5, 10, 15, 20, 30]
    zr = []
    for z in zs:
        wz = es.work(SLUG, f"z{z}")
        es.copy_inputs(wz, INPUTS)
        es.run(model(z=z, demand=False, sub=False), wz, {"water.wyield": "wyield.tif",
                                                         "water.watershed_table": "ws.csv"})
        figs.raster(wz / "wyield.tif", OUT / f"z_{z}.png", cmap="Blues", vmin=200, vmax=1300,
                    unit="water yield, mm / year", width=820)
        t = figs.read_csv(wz / "output" / "ws.csv")
        l2 = es.grid(wz / "lulc_2020.tif")
        y = es.grid(wz / "wyield.tif")
        zr.append({"z": z, "mean_yield": round(float(np.nanmean(y)), 1),
                   "forest": round(float(np.nanmean(y[l2 == 1])), 1),
                   "pasture": round(float(np.nanmean(y[l2 == 2])), 1),
                   "cropland": round(float(np.nanmean(y[l2 == 3])), 1),
                   "volume": round(sum(float(r["wyield_vol"]) for r in t) / 1e6, 3)})
    figs.write_csv(DAT / "seasonality.csv", zr)
    print(zr)

    # 2040
    wf = es.work(SLUG, "future")
    es.copy_inputs(wf, INPUTS)
    es.run(model(lulc="lulc_2040.tif"), wf, {"water.wyield": "wyield.tif", "water.watershed_table": "ws.csv"})
    figs.raster(None, OUT / "wyield_change.png", array=es.grid(wf / "wyield.tif") - wy, diverging=True, cmap="BrBG",
                unit="mm / year, 2040 - 2020")
    a = figs.read_csv(w / "output" / "watersheds.csv")
    b = figs.read_csv(wf / "output" / "ws.csv")
    comp = []
    for ra, rb in zip(a, b):
        for col, label, scale in (("wyield_vol", "Water yield (million m³)", 1e6),
                                  ("consum_vol", "Consumption (million m³)", 1e6),
                                  ("rsupply_vl", "Realized supply (million m³)", 1e6),
                                  ("hp_energy", "Hydropower (MWh / year)", 1e3),
                                  ("hp_val", "Hydropower, net present value (millions)", 1e6)):
            comp.append({"watershed": int(float(ra["ws_id"])), "quantity": label,
                         "2020": round(float(ra[col]) / scale, 3), "2040": round(float(rb[col]) / scale, 3)})
    figs.write_csv(DAT / "scenario.csv", comp)
    print(comp)

    # float32 against float64
    w64 = es.work(SLUG, "f64")
    es.copy_inputs(w64, INPUTS)
    es.run(model(precision="float64", demand=False, sub=False), w64, {"water.wyield": "wyield.tif"})
    d = np.nanmax(np.abs(es.grid(w64 / "wyield.tif") - wy))
    print("float64 - float32: largest difference", d, "mm")


if __name__ == "__main__":
    main()
