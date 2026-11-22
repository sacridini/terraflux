"""Figures of "Seasons of objects" (tutorials/object-based-image-analysis/seasons-of-objects.md).

A year of Sentinel-2 over farmland north-west of Sorriso, Mato Grosso (annual crops, bare fields, pasture, forest,
the Brazilian cerrado-amazon edge): 23 windows of 16 days from 1 September 2022, each a median composite of the clear
cells (data.stac with mask = auto: the scene classification drops cloud, shadow and cirrus before the scenes are
combined) turned into NDVI, a layer a window named by the window's middle date. The series is written once
(website/.cache/obia/seasons/series.tif, a download of a few minutes, kept by Terraflux's cache); the analysis (the
parcels, objects.series, k-means on their seasons) runs on it.

Run with: TERRAFLUX_BIN=<release terraflux> python website/figures/object-based-image-analysis/seasons-of-objects.py
"""

import datetime
import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import matplotlib  # noqa: E402

matplotlib.use("Agg")
import matplotlib.pyplot as plt  # noqa: E402
import numpy as np  # noqa: E402
import obia_data as od  # noqa: E402
import rasterio  # noqa: E402

figs = od.figs
SLUG = "seasons-of-objects"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")
CACHE = od.CACHE / "seasons"

URL = "https://earth-search.aws.element84.com/v1"
EXTENT = [623032, 8632475, 627032, 8636475]  # 4 x 4 km in SIRGAS 2000 / UTM 21S (EPSG:31981), farmland north-west of Sorriso
CRS = "EPSG:31981"
START = datetime.date(2022, 9, 1)
WINDOWS = 23
STEP = 16


# Earth Search says for each scene whether the offset of 1000 of the scenes processed since 2022 is already taken out
# of its values (earthsearch:boa_offset_applied): it is for 72 of the 74 scenes of the year over this area, and the two
# others (baseline 04.00) are left out by this query. A cell too dark to have a ratio (the two bands add up to less than
# 300, a reflectance of 0.03) has no NDVI.
QUERY = '{"earthsearch:boa_offset_applied": {"eq": true}}'
NDVI = '{ expression = "None if (s.nir + s.red) < 300 else clamp((s.nir - s.red) / (s.nir + s.red), -1, 1)" }'


def window(k):
    a = START + datetime.timedelta(days=STEP * k)
    b = a + datetime.timedelta(days=STEP - 1)
    return a, b, a + datetime.timedelta(days=STEP // 2 - 1)


def layer_name(k):
    return "ndvi_" + window(k)[2].strftime("%Y%m%d")


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def fetch_model():
    """The series: a data.stac a window and its NDVI, stacked."""
    m = 'format = "terraflux/1"\n'
    inputs = []
    for k in range(WINDOWS):
        a, b, _ = window(k)
        m += node(f"w{k}", "data.stac", "{ }",
                  f'{{ url = "{URL}", collection = "sentinel-2-l2a", assets = ["red", "nir"], datetime = "{a}/{b}", '
                  f'extent = {EXTENT}, cellsize = 10, crs = "{CRS}", mask = "auto", composite = "median", '
                  f"query = '{QUERY}' }}")
        m += node(f"n{k}", "raster.calc", f'{{ s = "w{k}.raster" }}', NDVI)
        inputs.append(f'{layer_name(k)} = "n{k}.map"')
    m += node("series", "stack.of", "{ " + ", ".join(inputs) + " }", "{ }")
    # a true-colour view of the dry season, for the figures
    m += node("rgb", "data.stac", "{ }",
              f'{{ url = "{URL}", collection = "sentinel-2-l2a", assets = ["red", "green", "blue"], '
              f'datetime = "2023-07-01/2023-07-31", extent = {EXTENT}, cellsize = 10, crs = "{CRS}", mask = "auto", '
              'composite = "median" }')
    return figs.add_writes(m, {"series.stack": "series.tif", "rgb.raster": "rgb.tif"})


SCALE = 10
KMEANS = ["ndvi_amplitude", "ndvi_hmean", "ndvi_amp1", "ndvi_amp2"]


def analysis_model(series, rgb, scale=SCALE):
    """The parcels, their seasons and their crop calendars, on the series written above."""
    m = 'format = "terraflux/1"\n'
    m += node("s", "raster.read", "{ }", f'{{ path = "{series.as_posix()}" }}')
    m += node("feat", "raster.calc", '{ s = "s.raster" }',
              '{ expression = """\nout hi = max(s)\nout lo = min(s)\nout mid = mean(s)\n""" }')
    m += node("featstack", "stack.of", '{ hi = "feat.hi", lo = "feat.lo", mid = "feat.mid" }', "{ }")
    m += node("parcels", "segment.multiresolution", '{ image = "featstack.stack" }', f"{{ scale = {scale} }}")
    m += node("seasons", "objects.series", '{ objects = "parcels.objects", series = "s.raster" }',
              '{ variable = "ndvi", groups = ["phenology", "harmonics"], n = 2, smooth = "savgol", window = 5, order = 2 }')
    cols = ", ".join(f'"{c}"' for c in KMEANS)
    m += node("km", "ml.kmeans", '{ table = "seasons.table" }', f"{{ columns = [{cols}], k = 4, seed = 1 }}")
    m += node("calendar", "objects.classify", '{ objects = "seasons.objects", model = "km.model" }', "{ }")
    m += node("idmap", "objects.to_map", '{ objects = "seasons.objects" }', '{ what = "id" }')
    m += node("peakmap", "objects.to_map", '{ objects = "seasons.objects" }', '{ what = "ndvi_peak_doy" }')
    m += node("classmap", "objects.to_map", '{ objects = "calendar.objects" }', '{ what = "class" }')
    writes = {"seasons.series": "long.csv", "seasons.table": "objects.csv", "calendar.table": "classes.csv",
              "idmap.map": "ids.tif", "peakmap.map": "peak.tif", "classmap.map": "classes.tif"}
    return figs.add_writes(m, writes)


def table(path):
    import csv

    with open(path, newline="", encoding="utf-8") as f:
        return [{k: (float(v) if v not in ("", "NaN", "nan") else float("nan")) if k != "class" else v
                 for k, v in r.items()} for r in csv.DictReader(f)]


OKABE = ["#0072B2", "#E69F00", "#009E73", "#CC79A7", "#56B4E9", "#D55E00"]
MONTHS = "JFMAMJJASOND"


def read_tif(path):
    with rasterio.open(path) as ds:
        return np.ma.filled(ds.read(masked=True).astype("float64"), np.nan)


def day_of(days):
    return datetime.date(1970, 1, 1) + datetime.timedelta(days=int(round(days)))


def month_axis(ax, dates):
    """Ticks on the first of the months, named by their initial."""
    marks = []
    d = min(dates).replace(day=1)
    while d <= max(dates):
        marks.append(d)
        d = (d.replace(day=1) + datetime.timedelta(days=32)).replace(day=1)
    ax.set_xticks(marks)
    ax.set_xticklabels([MONTHS[m.month - 1] for m in marks], fontsize=8)
    ax.set_xlim(min(dates) - datetime.timedelta(days=8), max(dates) + datetime.timedelta(days=8))


def png(fig, path, width, colors):
    fig.savefig(path)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(path))[..., :3], path, width=width, colors=colors)


def analysis():
    series, rgb_tif = CACHE / "series.tif", CACHE / "rgb.tif"
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    r = od.run(analysis_model(series, rgb_tif), work, "--profile")
    print(r.stdout[:300])
    out = work / "output"
    objs = table(out / "objects.csv")
    cls = table(out / "classes.csv")
    long = table(out / "long.csv")
    ids = read_tif(work / "ids.tif")[0]
    peak = read_tif(work / "peak.tif")[0]
    classes = read_tif(work / "classes.tif")[0]
    a = read_tif(series)
    names = list(rasterio.open(series).descriptions)
    wdates = [datetime.datetime.strptime(x[-8:], "%Y%m%d").date() for x in names]
    rgb = np.nan_to_num(np.moveaxis(read_tif(rgb_tif)[:3], 0, -1))
    lo, hi = np.percentile(rgb, 2), np.percentile(rgb, 98)
    rgb = np.clip((rgb - lo) / (hi - lo), 0, 1) ** 0.8
    ids_i = np.nan_to_num(ids, nan=-1).astype("int64")
    print("values at the clamp (-1 or 1):", int(np.nansum(np.abs(a) >= 1.0)), "of", int((~np.isnan(a)).sum()), "; min", float(np.nanmin(a)), "max", float(np.nanmax(a)))
    print("parcels", len(objs), "windows", len(wdates), "missing share", round(float(np.isnan(a).mean()), 3))
    print({k: round(float(np.nanmedian([o[k] for o in objs])), 3) for k in objs[0] if k.startswith("ndvi_") and "date" not in k})
    # the parcels over the dry-season image
    od.save_png(od.outlines(rgb, ids_i, scale=2, thick=1), OUT / "parcels.png", width=640, colors=128)
    # six windows of NDVI: the clouds are gaps
    cmap = plt.get_cmap("YlGn").copy()
    cmap.set_bad("#d9d9d9")
    for k in (1, 4, 7, 10, 14, 19):
        fig = plt.figure(figsize=(3.2, 3.2), dpi=110)
        ax = fig.add_axes([0, 0, 1, 1])
        ax.imshow(np.ma.masked_invalid(a[k]), cmap=cmap, vmin=0, vmax=0.9, interpolation="nearest")
        ax.text(6, 22, f"{wdates[k]:%d %b %Y}", fontsize=9, fontweight="bold", color="#222222",
                bbox={"fc": "white", "alpha": 0.7, "ec": "none", "pad": 2})
        ax.set_axis_off()
        png(fig, OUT / f"ndvi_{k}.png", 320, 64)
        print("window", k, wdates[k], "missing", round(float(np.isnan(a[k]).mean()), 3))
    # the long table by object
    by = {}
    for row in long:
        by.setdefault(int(row["id"]), []).append(row)
    dates = [day_of(r["date"]) for r in by[1]]
    label = {int(c["id"]): c["class"] for c in cls}
    order = sorted({v for v in label.values() if v})
    code = {v: k for k, v in enumerate(order)}
    # four parcels, one of each calendar: the biggest ones near the median peak of their calendar
    chosen = []
    for c in order:
        members = [o for o in objs if label.get(int(o["id"])) == c and o["cells"] >= 400 and np.isfinite(o["ndvi_start_date"])]
        if members:
            med = np.median([o["ndvi_peak_doy"] for o in members])
            chosen.append(min(members, key=lambda o: (abs(o["ndvi_peak_doy"] - med), -o["cells"])))
    fig, axes = plt.subplots(2, 2, figsize=(7.4, 4.6), dpi=130, sharey=True)
    for ax, o in zip(axes.flat, chosen):
        rows = by[int(o["id"])]
        v = np.array([r_["value"] for r_ in rows])
        sm = np.array([r_["smoothed"] for r_ in rows])
        color = OKABE[code[label[int(o["id"])]]]
        ax.plot(dates, sm, color=color, lw=2)
        ax.plot([d for d, x in zip(dates, v) if np.isfinite(x)], v[np.isfinite(v)], "o", color=color, ms=4)
        for d, x in zip(dates, v):
            if not np.isfinite(x):
                ax.axvline(d, color="#bbbbbb", lw=0.8, zorder=0)
        for key in ("ndvi_start_date", "ndvi_end_date"):
            ax.axvline(day_of(o[key]), color="#444444", lw=1, ls="--")
        ax.plot([day_of(o["ndvi_peak_date"])], [o["ndvi_max"]], marker="^", color="black", ms=7)
        ax.set_title(f"parcel {int(o['id'])}: {label[int(o['id'])]}, {int(o['cells'])} cells", fontsize=9)
        month_axis(ax, dates)
        ax.set_ylim(0, 1)
        ax.tick_params(labelsize=8)
    for ax in axes[:, 0]:
        ax.set_ylabel("NDVI", fontsize=9)
    fig.tight_layout()
    png(fig, OUT / "profiles.png", 720, 128)
    print("profiles of", [(int(o["id"]), label[int(o["id"])], round(o["ndvi_peak_doy"]), round(o["ndvi_amplitude"], 2)) for o in chosen])
    # the day of the peak
    fig = plt.figure(figsize=(4.8, 4.8), dpi=130)
    ax = fig.add_axes([0, 0, 1, 1])
    im = ax.imshow(np.ma.masked_invalid(peak), cmap="twilight", vmin=1, vmax=366, interpolation="nearest")
    ax.set_axis_off()
    cax = fig.add_axes([0.08, 0.04, 0.5, 0.025])
    cb = fig.colorbar(im, cax=cax, orientation="horizontal")
    cb.set_ticks([1, 91, 182, 274, 365])
    cb.set_ticklabels(["1 Jan", "1 Apr", "1 Jul", "1 Oct", "31 Dec"], fontsize=7)
    png(fig, OUT / "peak.png", 480, 64)
    # the calendars (the clusters of the seasons)
    img = np.ones(classes.shape + (3,))
    for c, k in code.items():
        img[classes == k + 1] = np.array(matplotlib.colors.to_rgb(OKABE[k]))
    img = np.where(np.isnan(classes)[..., None], 0.85, img)
    od.save_png(od.outlines(img, ids_i, scale=2, thick=1, color=(1, 1, 1)), OUT / "calendars.png", width=480, colors=64)
    # the curve of each calendar: the median and the quartiles of the smoothed series of its parcels
    fig, ax = plt.subplots(figsize=(6.4, 3.4), dpi=130)
    summary = []
    for c in order:
        sel = [o for o in objs if label.get(int(o["id"])) == c]
        m = np.array([[r_["smoothed"] for r_ in by[int(o["id"])]] for o in sel], float)
        q1, med, q3 = np.nanpercentile(m, [25, 50, 75], axis=0)
        color = OKABE[code[c]]
        ax.fill_between(dates, q1, q3, color=color, alpha=0.2)
        ax.plot(dates, med, color=color, lw=2, label=f"{c} ({len(sel)} parcels)")
        share = sum(o["cells"] for o in sel) / sum(o["cells"] for o in objs)
        med_of = lambda k, sel=sel: float(np.nanmedian([o[k] for o in sel]))  # noqa: E731
        summary.append({"calendar": c, "parcels": len(sel), "area_share": round(100 * share, 1),
                        "peak_doy": round(med_of("ndvi_peak_doy")), "amplitude": round(med_of("ndvi_amplitude"), 2),
                        "max": round(med_of("ndvi_max"), 2), "amp1": round(med_of("ndvi_amp1"), 3),
                        "amp2": round(med_of("ndvi_amp2"), 3), "length": round(med_of("ndvi_length"))})
    month_axis(ax, dates)
    ax.set_ylim(0, 1)
    ax.set_ylabel("NDVI", fontsize=9)
    ax.legend(fontsize=8, frameon=False, loc="upper right")
    fig.tight_layout()
    png(fig, OUT / "calendar_curves.png", 640, 128)
    figs.write_csv(DATA / "calendars.csv", summary)
    for row in summary:
        print(row)
    rows = [{"parcel": int(o["id"]), "calendar": label[int(o["id"])], "cells": int(o["cells"]),
             "max": round(o["ndvi_max"], 3), "amplitude": round(o["ndvi_amplitude"], 3),
             "peak_date": str(day_of(o["ndvi_peak_date"])), "start": str(day_of(o["ndvi_start_date"])),
             "end": str(day_of(o["ndvi_end_date"])), "length": round(o["ndvi_length"]),
             "integral": round(o["ndvi_integral"], 1), "hmean": round(o["ndvi_hmean"], 3),
             "amp1": round(o["ndvi_amp1"], 3), "amp2": round(o["ndvi_amp2"], 3), "rmse": round(o["ndvi_rmse"], 3)}
            for o in chosen]
    figs.write_csv(DATA / "parcels.csv", rows)
    return work


def main():
    CACHE.mkdir(parents=True, exist_ok=True)
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}-fetch")
    series = CACHE / "series.tif"
    rgb = CACHE / "rgb.tif"
    if not series.exists():
        r = od.run(fetch_model(), work, "--profile")
        print(r.stdout[:300])
        import shutil

        shutil.copy(work / "series.tif", series)
        shutil.copy(work / "rgb.tif", rgb)
    with rasterio.open(series) as ds:
        a = ds.read(masked=True)
        print("series", a.shape, "missing", float(a.mask.mean()), "names", list(ds.descriptions)[:3])
    analysis()


if __name__ == "__main__":
    main()
