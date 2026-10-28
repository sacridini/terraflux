"""Figures of 'Seasonal water yield' (ecosystem-services/seasonal-water-yield).

services.seasonal_water_yield on Ribeirão Claro. The landscape of es_common has annual rasters of precipitation and
reference evapotranspiration but no months, soil groups or curve numbers: this script makes them (illustrative,
written into its own work folder and zipped for the readers): twelve months of rain and ET0 with the wet summer and
dry winter of southeastern Brazil (the annual maps times the share of each month), the rain events of each month,
hydrologic soil groups from the soil depth and the rivers, and a biophysical table of curve numbers and monthly
crop coefficients. Then: the quickflow curve, the inputs, the maps of quickflow, recharge and baseflow, the monthly
quickflow, the tables by class, watershed and month, beta, gamma, alpha, the stream threshold and the scenario of
2040.
"""

import os

# at most four cores on the machine, and other scripts run beside this one
os.environ["RAYON_NUM_THREADS"] = "1"
os.environ["OMP_NUM_THREADS"] = "1"

import csv  # noqa: E402
import shutil  # noqa: E402
import zipfile  # noqa: E402

import numpy as np  # noqa: E402

import es_common as es  # noqa: E402
from es_common import figs, plt  # noqa: E402

SLUG = "seasonal-water-yield"
OUT = es.img(SLUG)
DAT = es.data(SLUG)
MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"]
INPUTS = ["dem.tif", "lulc_2020.tif", "lulc_2040.tif", "watersheds.gpkg"]
OWN = ["precipitation_monthly.tif", "et0_monthly.tif", "soil_group.tif", "biophysical_swy.csv"]

# the climate of a town of the interior of São Paulo or southern Minas Gerais (illustrative): mm per month of a
# year of 1,435 mm of rain and 1,252 mm of ET0, and the days with rain
P_MONTH = np.array([260, 200, 160, 70, 60, 45, 35, 30, 70, 125, 160, 220], float)
ET0_MONTH = np.array([130, 115, 115, 95, 75, 65, 72, 95, 110, 125, 125, 130], float)
P_SHARE = P_MONTH / P_MONTH.sum()
ET0_SHARE = ET0_MONTH / ET0_MONTH.sum()
EVENTS = [18, 15, 13, 7, 6, 4, 3, 3, 6, 10, 12, 16]
THRESHOLD = 1000
BETAS = [0, 0.02, 0.05, 0.1, 0.25, 0.5, 1]

# curve numbers by soil group A, B, C, D (in the range of the USDA's tables for woods, pasture, row crops and
# residential land) and crop coefficients by month (illustrative)
CN = {1: [30, 55, 70, 77], 2: [49, 69, 79, 84], 3: [65, 75, 82, 86], 4: [36, 60, 73, 79], 5: [77, 85, 90, 92],
      6: [99, 99, 99, 99]}
KC = {
    1: [1.0, 1.0, 1.0, 1.0, 0.95, 0.9, 0.9, 0.9, 0.95, 1.0, 1.0, 1.0],
    2: [0.85, 0.85, 0.85, 0.8, 0.7, 0.6, 0.55, 0.55, 0.6, 0.75, 0.85, 0.85],
    3: [1.1, 1.15, 0.9, 0.6, 0.5, 0.4, 0.35, 0.35, 0.4, 0.5, 0.7, 0.95],
    4: [1.05, 1.05, 1.05, 1.0, 1.0, 0.95, 0.95, 0.95, 1.0, 1.05, 1.05, 1.05],
    5: [0.3] * 12,
    6: [1.0] * 12,
}
SOILS = {1: ("A", "#e9d8a6"), 2: ("B", "#c8a46e"), 3: ("C", "#94704a"), 4: ("D", "#4f3a2a")}


# ------------------------------------------------------------------------------------------------ the inputs


def make_inputs() -> "pathlib.Path":  # noqa: F821
    """The months, soil groups and table, in this script's own folder (made each run, from the landscape)."""
    import rasterio

    src = es.landscape()
    d = es.work(SLUG, "inputs")
    p = es.grid(src / "precipitation.tif")
    e = es.grid(src / "eto.tif")
    prof = dict(driver="GTiff", height=es.H, width=es.W, count=12, dtype="float32", crs=es.CRS,
                transform=es.transform(), nodata=None, compress="deflate", predictor=3)
    for name, annual, share in (("precipitation_monthly", p, P_SHARE), ("et0_monthly", e, ET0_SHARE)):
        with rasterio.open(d / f"{name}.tif", "w", **prof) as ds:
            for m in range(12):
                ds.write((annual * share[m]).round(0).astype("float32"), m + 1)
                ds.set_band_description(m + 1, MONTHS[m].lower())
    # soil groups: deep soils on gentle ground drain well (A), shallow soils on the steep slopes and the wet soils
    # along the rivers drain poorly (D)
    depth = es.grid(src / "depth.tif")
    river = np.load(src / "river.npy")
    soil = np.full((es.H, es.W), 4, "uint8")
    soil[depth >= 850] = 3
    soil[depth >= 1150] = 2
    soil[depth >= 1700] = 1
    soil[es._near(river, 2)] = 4
    es.write(d / "soil_group.tif", soil, "uint8", nodata=255)
    with open(d / "biophysical_swy.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.writer(f)
        w.writerow(["lucode", "cn_a", "cn_b", "cn_c", "cn_d"] + [f"kc_{m}" for m in range(1, 13)])
        for k in sorted(CN):
            w.writerow([k] + CN[k] + KC[k])
    with zipfile.ZipFile(DAT / "seasonal-inputs.zip", "w", zipfile.ZIP_DEFLATED) as z:
        for n in OWN:
            z.write(d / n, f"ribeirao-claro/{n}")
    return d


def copy(w, inputs_dir):
    es.copy_inputs(w, INPUTS)
    for n in OWN:
        shutil.copy(inputs_dir / n, w / n)


def fmt(v):
    if isinstance(v, str):
        return f'"{v}"'
    if isinstance(v, (list, tuple)):
        return "[" + ", ".join(fmt(x) for x in v) + "]"
    return repr(float(v)) if not float(v).is_integer() else str(int(v))


def model(lulc="lulc_2020.tif", **params):
    p = {"threshold": THRESHOLD, "rain_events": EVENTS, **params}
    ps = ", ".join(f"{k} = {fmt(v)}" for k, v in p.items())
    return f'''format = "terraflux/1"

[[nodes]]
id = "dem"
op = "raster.read"
params = {{ path = "dem.tif" }}

[[nodes]]
id = "lulc"
op = "raster.read"
params = {{ path = "{lulc}" }}

[[nodes]]
id = "soils"
op = "raster.read"
params = {{ path = "soil_group.tif" }}

[[nodes]]
id = "precipitation"
op = "raster.read"
params = {{ path = "precipitation_monthly.tif" }}

[[nodes]]
id = "et0"
op = "raster.read"
params = {{ path = "et0_monthly.tif" }}

[[nodes]]
id = "biophysical"
op = "table.read"
params = {{ path = "biophysical_swy.csv" }}

[[nodes]]
id = "watersheds"
op = "vector.read"
params = {{ path = "watersheds.gpkg" }}

[[nodes]]
id = "swy"
op = "services.seasonal_water_yield"
inputs = {{ dem = "dem.raster", lulc = "lulc.raster", soil_group = "soils.raster", precipitation = "precipitation.raster", et0 = "et0.raster", biophysical = "biophysical.table", watersheds = "watersheds.vector" }}
params = {{ {ps} }}
'''


MAPS = ["quickflow", "local_recharge", "baseflow", "aet", "l_sum_avail", "b_sum", "stream", "vri",
        "annual_precipitation", "l_avail", "l_sum"]


def run(w, maps=MAPS, monthly=False, **kw):
    writes = {f"swy.{k}": f"{k}.tif" for k in maps}
    if monthly:
        writes["swy.quickflow_monthly"] = "quickflow_monthly.tif"
    writes["swy.watershed_table"] = "ws.csv"
    writes["swy.monthly_table"] = "months.csv"
    es.run(model(**kw), w, writes)


def mean(w, k, mask=None):
    a = es.grid(w / f"{k}.tif")
    return float(np.nanmean(a if mask is None else a[mask]))


def months(w):
    """The monthly table summed over the watersheds, in mm over their area."""
    rows = figs.read_csv(w / "output" / "months.csv")
    out = []
    for m in range(1, 13):
        r = [x for x in rows if int(float(x["month"])) == m]
        out.append({k: sum(float(x[k]) for x in r) / AREA * 1000 for k in ("quickflow", "baseflow", "precipitation")})
    return out


# ------------------------------------------------------------------------------------------------ drawings


def qf_formula(p, n, cn):
    """InVEST's eq. 97 (Guswa et al. 2018), for a cell off the streams: the quickflow of a month, mm."""
    from scipy.special import exp1

    s = 1000.0 / cn - 10.0
    a = p / n / 25.4
    r = s / a
    q = n * ((a - s) * np.exp(-0.2 * r) + s * s / a * np.exp(0.8 * r) * exp1(r)) * 25.4
    return np.where(r > 100, 0.0, np.maximum(q, 0.0))


def curves():
    p = np.linspace(5, 350, 300)
    fig, axs = plt.subplots(1, 2, figsize=(1000 / 150, 430 / 150), dpi=150, sharey=True)
    cols = figs.CLASSES
    for cn, col in zip([55, 70, 79, 86, 92], cols):
        axs[0].plot(p, qf_formula(p, 15, cn) / p, color=col, lw=2.2, label=f"CN {cn}")
    axs[0].set_xlabel("Rain in the month, mm\n(15 rain events)")
    axs[0].set_ylabel("Quickflow / rain")
    n = np.arange(1, 31)
    for cn, col in zip([55, 70, 79, 86, 92], cols):
        axs[1].plot(n, qf_formula(np.full(n.shape, 200.0), n, cn) / 200, color=col, lw=2.2, label=f"CN {cn}")
    axs[1].set_xlabel("Rain events in the month\n(200 mm of rain)")
    axs[1].legend(loc="center left", bbox_to_anchor=(1.01, 0.5))
    axs[0].set_ylim(0, 0.8)
    fig.tight_layout()
    fig.savefig(OUT / "quickflow-curve.png", dpi=150)
    plt.close(fig)
    figs._shrink(OUT / "quickflow-curve.png")
    # the worked cell of the page: pasture on a group C soil (CN 79) in January and in July
    for cn in (49, 79, 84):
        for mo, pm, nm in (("Jan", 260.0, 18), ("Jul", 35.0, 3), ("Jan, 9 events", 260.0, 9)):
            s = 1000 / cn - 10
            a = pm / nm / 25.4
            print(f"worked cell CN {cn} {mo}: a = {a:.3f} in ({a * 25.4:.1f} mm), S = {s:.3f} in ({s * 25.4:.1f} mm), "
                  f"S/a = {s / a:.2f}, QF = {float(qf_formula(np.array(pm), nm, cn)):.2f} mm of {pm}")


def with_streams(a, st, png, cmap, unit, vmin, vmax, log=False, width=520, color=figs.WATER):
    """A map of the cells off the streams (values below vmin drawn as vmin), the streams in blue over it."""
    from matplotlib import colors as mc

    h, w_ = a.shape
    fig = plt.figure(figsize=(width / 150, width / 150 * 0.72 * h / w_ * 1.1 + 0.15), dpi=150)
    ax = fig.add_axes([0, 0, 0.72, 1])
    ax.set_axis_off()
    cm = plt.get_cmap(cmap).copy()
    cm.set_bad((1, 1, 1, 0))
    b = np.where(st == 1, np.nan, np.clip(a, vmin, None))
    norm = mc.LogNorm(vmin, vmax) if log else mc.Normalize(vmin, vmax)
    im = ax.imshow(np.ma.masked_invalid(b), cmap=cm, norm=norm, interpolation="nearest")
    ax.imshow(np.where(st == 1, 1, np.nan), cmap=mc.ListedColormap([color]), interpolation="nearest")
    cax = fig.add_axes([0.76, 0.12, 0.035, 0.76])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.ax.grid(False)
    cb.ax.tick_params(labelsize=9)
    cb.set_label(unit, fontsize=9.5, color=figs.INK2)
    fig.savefig(png, dpi=150, bbox_inches="tight", pad_inches=0.08)
    plt.close(fig)
    figs._shrink(png)


def soil_map(path, png):
    a = es.grid(path)
    figs.raster(None, png, array=a, palette={k: (f"group {v[0]}", v[1]) for k, v in SOILS.items()}, width=520,
                counts=True)


AREA = 0.0


def main():
    global AREA
    import geopandas as gpd

    curves()
    inp = make_inputs()
    AREA = float(gpd.read_file(es.landscape() / "watersheds.gpkg").area.sum())
    print("area of the watersheds, ha:", AREA / 1e4)

    # -- the inputs
    for m in (1, 7):
        figs.raster(inp / "precipitation_monthly.tif", OUT / f"precip_{m}.png", band=m, cmap="Blues",
                    unit="mm / month", width=520)
    soil_map(inp / "soil_group.tif", OUT / "soil_group.png")
    s = es.grid(inp / "soil_group.tif")
    print("soil groups %:", {SOILS[k][0]: round(float((s == k).mean() * 100), 1) for k in SOILS})
    clim = es.grid(es.landscape() / "precipitation.tif")
    eto = es.grid(es.landscape() / "eto.tif")
    figs.write_csv(DAT / "climate.csv", [
        {"month": MONTHS[m], "precipitation": round(float(np.nanmean(clim)) * P_SHARE[m], 1),
         "et0": round(float(np.nanmean(eto)) * ET0_SHARE[m], 1), "events": EVENTS[m]} for m in range(12)])

    # -- the run of 2020
    w = es.work(SLUG, "main")
    copy(w, inp)
    run(w, monthly=True)
    st0 = es.grid(w / "stream.tif")
    qf_year = es.grid(w / "quickflow.tif")
    with_streams(qf_year, st0, OUT / "quickflow.png", "PuBu", "mm / year (streams in black)", 1, 1000, log=True,
                 color=figs.INK)
    off = qf_year[st0 == 0]
    print("quickflow off the streams: max", np.nanmax(off), "median", np.nanmedian(off))
    figs.raster(w / "aet.tif", OUT / "aet.png", cmap="YlGn", unit="mm / year", width=520)
    lr = es.grid(w / "local_recharge.tif")
    figs.raster(w / "local_recharge.tif", OUT / "local_recharge.png", diverging=True, cmap="BrBG",
                unit="mm / year", width=520)
    figs.raster(w / "baseflow.tif", OUT / "baseflow.png", cmap="GnBu", unit="mm / year", width=520)
    figs.raster(w / "l_sum_avail.tif", OUT / "l_sum_avail.png", cmap="cividis", unit="mm / year (log)", width=520,
                log=True, vmin=1)
    figs.raster(w / "b_sum.tif", OUT / "b_sum.png", cmap="cividis", unit="mm / year (log)", width=520, log=True,
                vmin=10)
    print("local recharge: min", np.nanmin(lr), "max", np.nanmax(lr), "share negative",
          float(np.nanmean(lr < 0)))
    b = es.grid(w / "baseflow.tif")
    print("largest |B - max(L, 0)|:", float(np.nanmax(np.abs(b - np.maximum(lr, 0)))))
    for k in ("l_sum_avail", "l_sum", "b_sum"):
        a = es.grid(w / f"{k}.tif")
        print(k, "max", np.nanmax(a), "median", np.nanmedian(a))
    print("negative recharge: streams", float(np.nanmean(lr[st0 == 1] < 0)), "off the streams",
          float(np.nanmean(lr[st0 == 0] < 0)), "cells", int(np.nansum(lr < 0)), "of", int(np.sum(~np.isnan(lr))))
    print("quickflow share of the rain", float(np.nanmean(es.grid(w / "quickflow.tif"))) /
          float(np.nanmean(es.grid(w / "annual_precipitation.tif"))))

    # the quickflow of each month, on one scale
    qm = []
    pm = figs.read(inp / "precipitation_monthly.tif", 1)[0]
    for m in range(1, 13):
        a = figs.read(w / "quickflow_monthly.tif", m)[0].filled(np.nan)
        qm.append(a)
    hi = float(np.nanpercentile(np.stack(qm), 99))
    frames = []
    for m in range(1, 13):
        with_streams(qm[m - 1], st0, OUT / f"qf_month_{m}.png", "PuBu", "quickflow, mm / month", 0.1, 100,
                     log=True, width=700, color=figs.INK)
        pmean = float(np.nanmean(figs.read(inp / "precipitation_monthly.tif", m)[0]))
        frames.append({"month": MONTHS[m - 1], "rain": pmean, "qf": float(np.nanmean(qm[m - 1]))})
    print("monthly quickflow scale max", hi, frames)

    # by class
    lu = es.grid(w / "lulc_2020.tif")
    st = es.grid(w / "stream.tif")
    byc = []
    for k, (name, _) in es.CLASSES.items():
        msk = (lu == k) & (st == 0)
        byc.append({"class": name, "cells": int(msk.sum()), "precipitation": round(mean(w, "annual_precipitation", msk)),
                    "quickflow": round(mean(w, "quickflow", msk)), "aet": round(mean(w, "aet", msk)),
                    "recharge": round(mean(w, "local_recharge", msk)), "baseflow": round(mean(w, "baseflow", msk))})
    figs.write_csv(DAT / "by-class.csv", byc)
    print(byc)
    # by soil group, pasture only (the effect of the soil, on one class)
    bys = []
    for g, (name, _) in SOILS.items():
        msk = (lu == 2) & (s == g) & (st == 0)
        bys.append({"group": name, "cn": CN[2][g - 1], "cells": int(msk.sum()),
                    "quickflow": round(mean(w, "quickflow", msk)), "recharge": round(mean(w, "local_recharge", msk))})
    print("pasture by soil group", bys)
    figs.write_csv(DAT / "pasture-by-soil.csv", bys)

    # by watershed
    ws = figs.read_csv(w / "output" / "ws.csv")
    mo = figs.read_csv(w / "output" / "months.csv")
    shed = gpd.read_file(es.landscape() / "watersheds.gpkg")
    rows = []
    for r in ws:
        g = int(float(r["geom_id"]))
        area = float(shed.area.iloc[g])
        rr = [x for x in mo if int(float(x["geom_id"])) == g]
        qf = sum(float(x["quickflow"]) for x in rr)
        bf = sum(float(x["baseflow"]) for x in rr)
        pp = sum(float(x["precipitation"]) for x in rr)
        rows.append({"ws_id": int(float(r["ws_id"])), "area_ha": round(area / 1e4), "qb": round(float(r["qb"]), 1),
                     "vri_sum": round(float(r["vri_sum"]), 3), "precipitation": round(pp / area * 1000),
                     "quickflow": round(qf / area * 1000), "baseflow": round(bf / area * 1000),
                     "quickflow_vol": round(qf / 1e6, 2), "baseflow_vol": round(bf / 1e6, 2)})
    figs.write_csv(DAT / "watersheds.csv", rows)
    print(rows)
    # the months, both watersheds
    mm = months(w)
    figs.write_csv(DAT / "months.csv", [{"month": MONTHS[i], "precipitation": round(x["precipitation"], 1),
                                         "quickflow": round(x["quickflow"], 1), "baseflow": round(x["baseflow"], 1)}
                                        for i, x in enumerate(mm)])
    print("months", mm)
    # the raw table as the node gives it (its first rows, quoted on the page)
    print("monthly_table, first rows:", mo[:4])

    stats = {"qf": mean(w, "quickflow"), "l": mean(w, "local_recharge"), "b": mean(w, "baseflow"),
             "aet": mean(w, "aet"), "p": mean(w, "annual_precipitation")}
    print("landscape means 2020", stats)

    # -- beta: the share of the water from upslope the cell may evaporate
    br = []
    extra = {}
    for beta in BETAS:
        wb = es.work(SLUG, f"beta{beta}")
        copy(wb, inp)
        run(wb, maps=["baseflow", "local_recharge", "aet", "quickflow"], beta=beta)
        extra[beta] = es.grid(wb / "aet.tif")
        br.append({"beta": beta, "aet": round(mean(wb, "aet"), 1), "recharge": round(mean(wb, "local_recharge"), 1),
                   "baseflow": round(mean(wb, "baseflow"), 1),
                   "negative": round(float(np.nanmean(es.grid(wb / "local_recharge.tif") < 0)) * 100, 1)})
    # the evaporation the water from upslope adds (AET with beta minus AET with beta = 0), on one scale
    hi = float(np.nanpercentile((extra[1] - extra[0])[st0 == 0], 99.5))
    for beta in BETAS:
        d = extra[beta] - extra[0]
        with_streams(d, st0, OUT / f"beta_{beta}.png", "YlOrBr", "AET from upslope water, mm / year", 1, 1000,
                     log=True, width=760)
        print("beta", beta, "extra AET off the streams: mean", float(np.nanmean(d[st0 == 0])), "cells > 10 mm",
              float(np.nanmean(d[st0 == 0] > 10)), "on the streams", float(np.nanmean(d[st0 == 1])))
    print("beta frames scale", hi)
    figs.write_csv(DAT / "beta.csv", br)
    print("beta", br)

    # -- gamma
    gr = []
    for gamma in (0, 0.25, 0.5, 0.75, 1):
        wg = es.work(SLUG, f"gamma{gamma}")
        copy(wg, inp)
        run(wg, maps=["baseflow", "local_recharge", "aet"], gamma=gamma)
        gr.append({"gamma": gamma, "aet": round(mean(wg, "aet"), 1), "recharge": round(mean(wg, "local_recharge"), 1),
                   "baseflow": round(mean(wg, "baseflow"), 1)})
    figs.write_csv(DAT / "gamma.csv", gr)
    print("gamma", gr)

    # -- alpha: the antecedent month's share of the rain, as InVEST's guide suggests for seasonal climates
    alpha = [round(float(P_SHARE[(m - 1) % 12]), 4) for m in range(12)]
    print("alpha by month", alpha, sum(alpha))
    wa = es.work(SLUG, "alpha")
    copy(wa, inp)
    run(wa, maps=["baseflow", "local_recharge", "aet"], alpha=alpha)
    ar = [{"alpha": "1/12 every month (default)", "aet": round(stats["aet"], 1), "recharge": round(stats["l"], 1),
           "baseflow": round(stats["b"], 1)},
          {"alpha": "the previous month's share of the rain", "aet": round(mean(wa, "aet"), 1),
           "recharge": round(mean(wa, "local_recharge"), 1), "baseflow": round(mean(wa, "baseflow"), 1)}]
    figs.write_csv(DAT / "alpha.csv", ar)
    print("alpha", ar)

    # -- the stream threshold
    tr = []
    for t in (250, 500, 1000, 2000, 4000):
        wt = es.work(SLUG, f"t{t}")
        copy(wt, inp)
        run(wt, maps=["quickflow", "baseflow", "local_recharge", "stream"], threshold=t)
        st_t = es.grid(wt / "stream.tif")
        tr.append({"threshold": t, "stream_ha": round(float((st_t == 1).sum()) * 0.09, 1),
                   "quickflow": round(mean(wt, "quickflow"), 1), "recharge": round(mean(wt, "local_recharge"), 1),
                   "baseflow": round(mean(wt, "baseflow"), 1)})
    figs.write_csv(DAT / "threshold.csv", tr)
    print("threshold", tr)

    # -- D8
    wd = es.work(SLUG, "d8")
    copy(wd, inp)
    run(wd, maps=["quickflow", "baseflow", "local_recharge", "aet"], method="d8")
    print("d8", {k: round(mean(wd, k), 1) for k in ("quickflow", "baseflow", "local_recharge", "aet")})

    # -- 2040
    wf = es.work(SLUG, "future")
    copy(wf, inp)
    run(wf, lulc="lulc_2040.tif", maps=["quickflow", "baseflow", "local_recharge", "aet", "annual_precipitation"])
    for k, unit in (("quickflow", "mm / year"), ("baseflow", "mm / year")):
        d = es.grid(wf / f"{k}.tif") - es.grid(w / f"{k}.tif")
        lim = float(np.nanpercentile(np.abs(d), 99.5))
        figs.raster(None, OUT / f"{k}_change.png", array=np.clip(d, -lim, lim), diverging=True, cmap="BrBG",
                    unit=unit, width=520)
        print(k, "change: min", np.nanmin(d), "max", np.nanmax(d), "clip", lim)
    mf = months(wf)
    sc = []
    for col, label in (("quickflow", "Quickflow (mm)"), ("baseflow", "Baseflow (mm)")):
        a, b = sum(x[col] for x in mm), sum(x[col] for x in mf)
        sc.append({"quantity": label, "2020": round(a, 1), "2040": round(b, 1), "change": f"{(b / a - 1) * 100:+.1f}%"})
    wet = [0, 1, 2, 9, 10, 11]
    a, b = sum(mm[i]["quickflow"] for i in wet), sum(mf[i]["quickflow"] for i in wet)
    sc.append({"quantity": "Quickflow, October to March (mm)", "2020": round(a, 1), "2040": round(b, 1),
               "change": f"{(b / a - 1) * 100:+.1f}%"})
    for k, label in (("aet", "Evapotranspiration (mm)"), ("local_recharge", "Local recharge (mm)")):
        a, b = mean(w, k), mean(wf, k)
        sc.append({"quantity": label + ", mean of the cells", "2020": round(a, 1), "2040": round(b, 1),
                   "change": f"{(b / a - 1) * 100:+.1f}%"})
    figs.write_csv(DAT / "scenario.csv", sc)
    print("2040", sc)
    wsf = figs.read_csv(wf / "output" / "ws.csv")
    print("2040 watersheds", wsf)
    # the change by transition (cells off the streams of both runs)
    l20, l40 = es.grid(w / "lulc_2020.tif"), es.grid(wf / "lulc_2040.tif")
    off = (st0 == 0)
    dq = es.grid(wf / "quickflow.tif") - es.grid(w / "quickflow.tif")
    db = es.grid(wf / "baseflow.tif") - es.grid(w / "baseflow.tif")
    de = es.grid(wf / "aet.tif") - es.grid(w / "aet.tif")
    trs = []
    for a in es.CLASSES:
        for b in es.CLASSES:
            m = off & (l20 == a) & (l40 == b)
            if a != b and m.sum() >= 40:
                trs.append({"transition": f"{es.NAMES[a]} to {es.NAMES[b]}", "ha": round(float(m.sum()) * 0.09),
                            "quickflow": round(float(np.nanmean(dq[m])), 1), "aet": round(float(np.nanmean(de[m])), 1),
                            "baseflow": round(float(np.nanmean(db[m])), 1)})
    trs.sort(key=lambda r: -r["ha"])
    figs.write_csv(DAT / "transitions.csv", trs)
    print("transitions", trs)
    m = off & (l20 == l40)
    print("unchanged cells: mean change of baseflow", float(np.nanmean(db[m])), "quickflow", float(np.nanmean(dq[m])))


if __name__ == "__main__":
    main()
