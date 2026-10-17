"""Figures of 'Carbon storage and sequestration' (ecosystem-services/carbon).

services.carbon on Ribeirão Claro: the pools of 2020, the storage of 2020 and 2040, the change and its value, the
totals, where the change comes from, and how the discount rate and the soil carbon of pasture change the answer.
"""

import numpy as np

import es_common as es
from es_common import figs, plt

SLUG = "carbon"
OUT = es.img(SLUG)
DAT = es.data(SLUG)
HA = es.PIXEL * es.PIXEL / 10000

MODEL = '''format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = {{ path = "lulc_2020.tif" }}

[[nodes]]
id = "future"
op = "raster.read"
params = {{ path = "lulc_2040.tif" }}

[[nodes]]
id = "pools"
op = "table.read"
params = {{ path = "{pools}" }}

[[nodes]]
id = "carbon"
op = "services.carbon"
inputs = {{ lulc = "lulc.raster", pools = "pools.table", future = "future.raster" }}
params = {{ price = {price}, discount_rate = {discount}, rate_change = {change}, year = 2020, future_year = 2040 }}
'''


def run(w, pools="carbon_pools.csv", price=50, discount=5, change=0, writes=None):
    es.copy_inputs(w, ["lulc_2020.tif", "lulc_2040.tif", "carbon_pools.csv"])
    model = MODEL.format(pools=pools, price=price, discount=discount, change=change)
    es.run(model, w, writes or {"carbon.totals": "totals.csv"})
    return figs.read_csv(w / "output" / "totals.csv")[0]


def main():
    # the pools table, for a chart
    rows = [{"class": es.NAMES[int(r[0])], "c_above": r[1], "c_below": r[2], "c_soil": r[3], "c_dead": r[4],
             "total": sum(r[1:])} for r in es.POOLS["rows"]]
    figs.write_csv(DAT / "pools.csv", rows)

    w = es.work(SLUG, "main")
    t = run(w, writes={"carbon.storage": "storage.tif", "carbon.storage_future": "storage_future.tif",
                       "carbon.pools": "pools.tif", "carbon.change": "change.tif", "carbon.npv": "npv.tif",
                       "carbon.totals": "totals.csv"})
    print("totals", t)
    names = {"baseline": "Baseline (2020)", "alternate": "Alternate (2040)", "change": "Change (2040 - 2020)",
             "npv": "Net present value of the change"}
    units = {"baseline": "t C", "alternate": "t C", "change": "t C", "npv": "currency"}
    figs.write_csv(DAT / "totals.csv", [{"column": k, "meaning": names[k], "value": round(float(v), 1),
                                         "unit": units[k]} for k, v in t.items()])

    for b, p in enumerate(["c_above", "c_below", "c_soil", "c_dead"], start=1):
        figs.raster(w / "pools.tif", OUT / f"{p}.png", band=b, cmap="YlGn", vmin=0, vmax=140, unit="t C / ha",
                    width=520)
    hi = 282
    figs.raster(w / "storage.tif", OUT / "storage_2020.png", cmap="YlGn", vmin=0, vmax=hi, unit="t C / ha")
    figs.raster(w / "storage.tif", OUT / "storage_2020_plain.png", cmap="YlGn", vmin=0, vmax=hi, legend=False,
                width=760)
    figs.raster(w / "storage_future.tif", OUT / "storage_2040_plain.png", cmap="YlGn", vmin=0, vmax=hi,
                legend=False, width=760)
    figs.raster(w / "change.tif", OUT / "change.png", diverging=True, cmap="RdBu", unit="t C / ha, 2040 - 2020")

    # where the change comes from: transitions between classes, their area and carbon
    a = es.grid(w / "lulc_2020.tif")
    b = es.grid(w / "lulc_2040.tif")
    ch = es.grid(w / "change.tif")
    trans = []
    for i in es.CLASSES:
        for j in es.CLASSES:
            if i == j:
                continue
            m = (a == i) & (b == j)
            if m.sum() == 0:
                continue
            trans.append({"from": es.NAMES[i], "to": es.NAMES[j], "hectares": round(m.sum() * HA, 1),
                          "t_C": round(float(np.nansum(ch[m])) * HA, 0)})
    trans.sort(key=lambda r: r["t_C"])
    figs.write_csv(DAT / "transitions.csv", trans)
    print(trans)

    # storage by class in 2020 and 2040
    s20 = es.grid(w / "storage.tif")
    s40 = es.grid(w / "storage_future.tif")
    byclass = []
    for k, (name, _) in es.CLASSES.items():
        byclass.append({"class": name, "2020": round(float(np.nansum(s20[a == k])) * HA),
                        "2040": round(float(np.nansum(s40[b == k])) * HA)})
    figs.write_csv(DAT / "by-class.csv", byclass)

    # the discount rate and the price's change: the net present value of the change
    rows = []
    for d in [0, 1, 2, 3, 4, 5, 6, 7, 8, 10, 12]:
        r = {"discount": d}
        for c in (0, 2, 5):
            t = run(es.work(SLUG, f"d{d}_c{c}"), discount=d, change=c)
            r[f"change_{c}"] = round(float(t["npv"]) / 1e6, 3)
        rows.append(r)
    figs.write_csv(DAT / "discount.csv", rows)
    print(rows)

    # the soil carbon of pasture: the uncertain number of the forest -> pasture conversion
    frames = []
    base = [list(r) for r in es.POOLS["rows"]]
    for soil in (40, 50, 60, 70, 80, 95):
        wk = es.work(SLUG, f"soil{soil}")
        rows_t = [list(r) for r in base]
        rows_t[1][3] = soil
        es.copy_inputs(wk, ["lulc_2020.tif", "lulc_2040.tif"])
        es.write_table(wk / "pools_variant.csv", {"columns": es.POOLS["columns"], "rows": rows_t})
        model = MODEL.format(pools="pools_variant.csv", price=50, discount=5, change=0)
        es.run(model, wk, {"carbon.change": "change.tif", "carbon.totals": "totals.csv"})
        t = figs.read_csv(wk / "output" / "totals.csv")[0]
        figs.raster(wk / "change.tif", OUT / f"soil_{soil}.png", diverging=True, cmap="RdBu", vmin=-200,
                    vmax=200, unit="t C / ha, 2040 - 2020", width=820)
        frames.append({"soil": soil, "change_t": round(float(t["change"])), "baseline_t": round(float(t["baseline"]))})
    print(frames)


if __name__ == "__main__":
    main()
