"""Figures of "The right scale" (tutorials/object-based-image-analysis/choosing-the-scale.md).

One model of Terraflux, run on the Sentinel-2 clip of the series (obia_data.py): segment.scale (ESP) and
segment.optimize (USPO) on the whole clip and on three 100 x 100 windows (water, town, cerrado), a multiresolution
segmentation at each scale of the slider, and the Otsu water mask of the NDWI as a reference for how many cells an
object puts on the wrong side of the shore. The figures draw the outlines of the objects over the RGB.
Run with: python website/figures/object-based-image-analysis/choosing-the-scale.py
"""

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
SLUG = "choosing-the-scale"
OUT = figs.img_dir(f"{od.CATEGORY}/{SLUG}")
DATA = figs.data_dir(f"{od.CATEGORY}/{SLUG}")

# the part of the clip the slider shows (same as part 2) and the detail of the first figure
R0, R1, C0, C1 = 90, 360, 200, 470
D0, D1, E0, E1 = 190, 290, 230, 350
# the scales of the slider, which are also the values segment.optimize scores
SL = [1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24]
ESP_SCALE = 2.48  # what segment.scale suggests first on this clip (checked below)
CELL_HA = 0.01
# three windows (row0, row1, col0, col1) of 100 x 100 cells: the lake, the town, the dry cerrado
WIN = {"water": (20, 120, 240, 340), "town": (90, 190, 20, 120), "cerrado": (440, 540, 250, 350)}
WIN_SCALES = [1, 4, 12]
WIN_VALUES = [0.5, 0.75, 1, 1.5, 2, 3, 4, 6, 8, 12]


def sid(s):
    return f"mr_{str(s).replace('.', '_')}"


def node(i, op, inputs, params):
    return f'\n[[nodes]]\nid = "{i}"\nop = "{op}"\ninputs = {inputs}\nparams = {params}\n'


def extent(r0, r1, c0, c1):
    """The UTM extent of a window of the clip (rows, columns)."""
    x0, y1 = od.S2_UTM_EXTENT[0], od.S2_UTM_EXTENT[3]
    return [x0 + c0 * 10, y1 - r1 * 10, x0 + c1 * 10, y1 - r0 * 10]


def build():
    m = od.sentinel2_model()
    m += node("ndwi", "raster.calc", '{ s = "refl.image" }', '{ expression = "(s.green - s.nir) / (s.green + s.nir)" }')
    m += node("th", "segment.threshold", '{ image = "ndwi.map" }', "{ classes = 2 }")
    writes = {"ndwi.map": "ndwi.tif", "th.thresholds": "thresholds.csv"}
    # ESP and USPO on the whole clip
    m += node("esp", "segment.scale", '{ image = "refl.image" }', "{ }")
    values = ", ".join(str(v) for v in SL)
    m += node("opt", "segment.optimize", '{ image = "refl.image" }', f"{{ values = [{values}] }}")
    m += node("opt_narrow", "segment.optimize", '{ image = "refl.image" }', "{ from = 1, to = 8, step = 0.5 }")
    m += node("opt_f", "segment.optimize", '{ image = "refl.image" }', f'{{ values = [{values}], objective = "f" }}')
    writes |= {"esp.table": "esp.csv", "esp.scales": "esp_scales.csv", "opt.scores": "scores.csv",
               "opt_narrow.scores": "scores_narrow.csv", "opt_f.scores": "scores_f.csv"}
    # the scales of the slider, and ESP's
    for s in SL + [ESP_SCALE]:
        i = sid(s)
        m += node(i, "segment.multiresolution", '{ image = "refl.image" }', f"{{ scale = {s} }}")
        m += node(f"{i}_map", "objects.to_map", f'{{ objects = "{i}.objects" }}', '{ what = "id" }')
        writes[f"{i}_map.map"] = f"{i}.tif"
    # a window of each class
    for k, e in WIN.items():
        m += node(f"win_{k}", "raster.clip", '{ raster = "refl.image" }', f"{{ extent = {extent(*e)} }}")
        m += node(f"esp_{k}", "segment.scale", f'{{ image = "win_{k}.raster" }}', "{ }")
        v = ", ".join(str(x) for x in WIN_VALUES)
        m += node(f"opt_{k}", "segment.optimize", f'{{ image = "win_{k}.raster" }}', f"{{ values = [{v}] }}")
        writes |= {f"esp_{k}.table": f"esp_{k}.csv", f"opt_{k}.scores": f"scores_{k}.csv"}
    return figs.add_writes(m, writes)


def labels(work, name):
    with rasterio.open(work / f"{name}.tif") as ds:
        a = ds.read(1, masked=True).astype("float64")
    return np.ma.filled(a, -1).astype("int64")


def table(path):
    import csv

    with open(path, newline="", encoding="utf-8") as f:
        return [{k: float(v) if v not in ("", "NaN", "nan") else float("nan") for k, v in r.items()} for r in csv.DictReader(f)]


def scaled(x):
    """segment.optimize's scaling over the runs: 1 for the lowest, 0 for the highest."""
    x = np.asarray(x, float)
    lo, hi = np.nanmin(x), np.nanmax(x)
    return (hi - x) / (hi - lo) if hi > lo else np.zeros_like(x)


def wrong_share(lab, water):
    """The share of the cells (with a value in both) that are on the other side of the shore (water or land) than
    their object's majority: what an object that mixes water and land costs."""
    ok = (lab >= 0) & (water >= 0)
    ids = lab[ok]
    w = (water[ok] == 1).astype(float)
    n = np.bincount(ids)
    sw = np.bincount(ids, weights=w)
    major = (sw[ids] / n[ids]) > 0.5
    return float(((w > 0.5) != major).mean() * 100)


def main():
    od.sentinel2()
    work = figs.workdir(f"{od.CATEGORY}/{SLUG}")
    r = od.run(build(), work, "--profile")
    print(r.stdout[:400])
    out = work / "output"
    rgb = od.rgb("sentinel2")
    lab = {s: labels(work, sid(s)) for s in SL + [ESP_SCALE]}
    # the Otsu water mask of the NDWI: 1 above the cut, 0 below, -1 without data
    cut = float((out / "thresholds.csv").read_text().split()[-1])
    with rasterio.open(work / "ndwi.tif") as ds:
        nd = ds.read(1, masked=True).astype("float64")
    water = np.where(np.ma.getmaskarray(nd), -1, (np.ma.filled(nd, 0) >= cut).astype("int64"))
    n = {s: len(np.unique(lab[s][lab[s] >= 0])) for s in lab}
    print("objects", n)

    # --- ESP of the whole clip
    esp = table(out / "esp.csv")
    sug = [row for row in esp if row["suggested"] == 1]
    print("ESP peaks", [(round(x["scale"], 3), int(x["objects"])) for x in sug])
    assert abs(sug[0]["scale"] - ESP_SCALE) < 0.005, sug[0]["scale"]
    bands = [c for c in esp[0] if c.startswith("lv_")]
    rows = [
        {"level": k + 1, "scale": round(x["scale"], 3), "objects": int(x["objects"]),
         "lv": round(float(np.mean([x[b] for b in bands])), 5), "roc_mean": round(x["roc_mean"], 2),
         "suggested": int(x["suggested"])}
        for k, x in enumerate(esp) if k >= 5
    ]
    figs.write_csv(DATA / "esp.csv", rows)
    print("ESP levels", len(esp), "ROC min", min(x["roc_mean"] for x in rows))

    # --- the scores on the whole clip (the slider's chart)
    sc = table(out / "scores.csv")
    scf = {x["value"]: x["objective"] for x in table(out / "scores_f.csv")}
    v, i = scaled([x["variance"] for x in sc]), scaled([x["moran"] for x in sc])
    best = max(sc, key=lambda x: x["objective"])
    print("USPO best", best["value"], best["objects"], "objective", round(best["objective"], 4))
    rows = []
    for k, x in enumerate(sc):
        s = x["value"]
        wr = wrong_share(lab[s], water)
        rows.append({"scale": s, "objects": int(x["objects"]), "variance": round(x["variance"], 6),
                     "moran": round(x["moran"], 4), "objective": round(x["objective"], 4),
                     "variance_scaled": round(v[k], 4), "moran_scaled": round(i[k], 4),
                     "mean_ha": round((lab[s] >= 0).sum() / x["objects"] * CELL_HA, 2), "wrong_pct": round(wr, 2),
                     "objective_f": round(scf[s], 4)})
    figs.write_csv(DATA / "scores.csv", rows)
    for row in rows:
        print(row)
    print("ESP scale wrong share", round(wrong_share(lab[ESP_SCALE], water), 2))
    narrow = table(out / "scores_narrow.csv")
    bn = max(narrow, key=lambda x: x["objective"])
    figs.write_csv(DATA / "narrow.csv", [{"scale": x["value"], "objects": int(x["objects"]), "objective": round(x["objective"], 4)}
                                         for x in narrow])
    print("narrow best", bn["value"], bn["objects"], [round(x["objective"], 3) for x in narrow])
    bf = max(table(out / "scores_f.csv"), key=lambda x: x["objective"])
    print("f best", bf["value"], bf["objects"])
    print("thresholds", (out / "thresholds.csv").read_text().split())

    # --- the three windows: ESP's peaks and USPO's best
    wrows = []
    for k, e in WIN.items():
        es = table(out / f"esp_{k}.csv")
        pk = [x for x in es if x["suggested"] == 1]
        sw = table(out / f"scores_{k}.csv")
        bw = max((x for x in sw if np.isfinite(x["objective"])), key=lambda x: x["objective"])
        wrows.append({"window": k, "peak1": round(pk[0]["scale"], 2), "objects1": int(pk[0]["objects"]),
                      "ha1": round(10000 * CELL_HA / pk[0]["objects"], 2),
                      "peak2": round(pk[1]["scale"], 2) if len(pk) > 1 else "",
                      "peak3": round(pk[2]["scale"], 2) if len(pk) > 2 else "",
                      "uspo": bw["value"], "uspo_objects": int(bw["objects"]),
                      "uspo_objective": round(bw["objective"], 3)})
        print(wrows[-1])
        print("  scores", [(x["value"], int(x["objects"]), round(x["objective"], 3)) for x in sw])
    figs.write_csv(DATA / "windows.csv", wrows)

    # --- figures
    # the three windows over the clip
    fig = plt.figure(figsize=(5.4, 5.4), dpi=150)
    ax = fig.add_axes([0, 0, 1, 1])
    ax.imshow(rgb)
    col = {"water": "#4aa8ff", "town": "#ffd91a", "cerrado": "#ff8a3c"}
    for k, (a, b, c, d) in WIN.items():
        ax.add_patch(plt.Rectangle((c, a), d - c, b - a, fill=False, ec=col[k], lw=1.8))
        ax.text(c + 5, a + 14, k, color=col[k], fontsize=9, fontweight="bold")
    ax.set_axis_off()
    fig.savefig(OUT / "windows.png", dpi=150)
    plt.close(fig)
    od.save_png(np.asarray(plt.imread(OUT / "windows.png"))[..., :3], OUT / "windows.png", width=640, colors=128)

    # too fine, about right, too coarse: a detail of the clip
    det = lambda a: a[D0:D1, E0:E1]  # noqa: E731
    for tag, s in (("fine", 1), ("right", ESP_SCALE), ("coarse", 16)):
        img = od.outlines(det(rgb), det(lab[s]), scale=5, thick=2)
        od.save_png(img, OUT / f"detail_{tag}.png", width=520, colors=128)

    # the slider
    crop = lambda a: a[R0:R1, C0:C1]  # noqa: E731
    for s in SL:
        img = od.outlines(crop(rgb), crop(lab[s]), scale=2, thick=2)
        od.save_png(img, OUT / f"scale_{str(s).replace('.', '_')}.png", width=560, colors=96)

    # each window at three scales
    for k, (a, b, c, d) in WIN.items():
        for s in WIN_SCALES:
            lw = lab[s][a:b, c:d]
            img = od.outlines(rgb[a:b, c:d], lw, scale=4, thick=2)
            od.save_png(img, OUT / f"win_{k}_{str(s).replace('.', '_')}.png", width=400, colors=96)
            print(k, s, "objects touching the window", len(np.unique(lw[lw >= 0])))

    (work / "numbers.json").write_text(json.dumps({"objects": {str(s): n[s] for s in n}}, indent=1), encoding="utf-8")


if __name__ == "__main__":
    main()
