"""Figures of "Richness and endemism" (biodiversity, part 1): Biodiverse's example data in cells of 100 km, species
richness and weighted endemism in the cells alone and in neighbourhoods of 100 to 300 km, and the same at three cell
sizes, from real runs of terraflux (bio.table, bio.spatial)."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _bio  # noqa: E402
from _bio import figs, np, plt  # noqa: E402

SLUG = "biodiversity/richness-and-endemism"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_bio.setup(work)

CALCS = '["richness", "endemism_whole"]'
NEIGHBOURHOODS = {"self": "sp_self_only()", "100": "sp_circle (radius => 100000)",
                  "200": "sp_circle (radius => 200000)", "300": "sp_circle (radius => 300000)"}
SIZES = [50000, 100000, 200000]

MODEL = 'format = "terraflux/1"\n'
writes = {}
for size in SIZES:
    MODEL += f"""
[[nodes]]
id = "sites_{size // 1000}"
op = "bio.table"
params = {{ path = "sites.csv", group_columns = ["x", "y"], label_columns = ["genus", "species"], cell_sizes = [{size}, {size}] }}

[[nodes]]
id = "self_{size // 1000}"
op = "bio.spatial"
inputs = {{ basedata = "sites_{size // 1000}.basedata" }}
params = {{ conditions = ["sp_self_only()"], calculations = {CALCS} }}
"""
    writes[f"self_{size // 1000}.maps"] = f"self_{size // 1000}.tif"
    writes[f"self_{size // 1000}.table"] = f"self_{size // 1000}.csv"
for name, cond in NEIGHBOURHOODS.items():
    if name == "self":
        continue
    MODEL += f"""
[[nodes]]
id = "circle_{name}"
op = "bio.spatial"
inputs = {{ basedata = "sites_100.basedata" }}
params = {{ conditions = ["{cond}"], calculations = {CALCS} }}
"""
    writes[f"circle_{name}.maps"] = f"circle_{name}.tif"
    writes[f"circle_{name}.table"] = f"circle_{name}.csv"
figs.run(figs.add_writes(MODEL, writes), work)


def band(tif, name):
    import rasterio

    with rasterio.open(tif) as ds:
        i = list(ds.descriptions).index(name) + 1
        a = ds.read(i, masked=True).astype(float)
        b = ds.bounds
    return a, (b.left, b.right, b.bottom, b.top)


x, y, species = _bio.read_records(work)
print("records", len(x), "species", len(set(species)))

# ---------------------------------------------------------------- the records and the cells
rich, ext = band(work / "self_100.tif", "RICHNESS_ALL")
_bio.cell_map(rich, out / "records.png", cmap="Greens", vmin=0, vmax=float(rich.max()) * 1.6, legend=False,
              extent=ext, points=(x, y), width=480)

# ---------------------------------------------------------------- range sizes (cells of 100 km)
cells = {}
for xi, yi, s in zip(x, y, species):
    cells.setdefault(s, set()).add((int(xi // 100000), int(yi // 100000)))
ranges = sorted(len(v) for v in cells.values())
bins = [(1, 2), (3, 5), (6, 10), (11, 20), (21, 40), (41, 80)]
rows = [{"cells": f"{a}–{b}" if a != b else f"{a}", "species": sum(a <= r <= b for r in ranges)} for a, b in bins]
figs.write_csv(data / "range-sizes.csv", rows)
print("range sizes:", ranges)

# ---------------------------------------------------------------- the neighbourhoods
tifs = {"self": work / "self_100.tif", "100": work / "circle_100.tif", "200": work / "circle_200.tif",
        "300": work / "circle_300.tif"}
vmax_r = max(float(band(t, "RICHNESS_ALL")[0].max()) for t in tifs.values())
vmax_e = max(float(band(t, "ENDW_WE")[0].max()) for t in tifs.values())
stats = {}
for name, t in tifs.items():
    r, e = band(t, "RICHNESS_ALL")[0], band(t, "ENDW_WE")[0]
    c = band(t, "ENDW_CWE")[0]
    _bio.cell_map(r, out / f"richness_{name}.png", cmap="viridis", vmin=0, vmax=vmax_r, unit="Species", extent=ext)
    _bio.cell_map(e, out / f"we_{name}.png", cmap="magma_r", vmin=0, vmax=vmax_e, unit="Weighted endemism",
                  extent=ext)
    _bio.cell_map(c, out / f"cwe_{name}.png", cmap="magma_r", vmin=0, vmax=float(c.max()),
                  unit="Corrected weighted endemism", extent=ext)
    stats[name] = {"rich_max": int(r.max()), "rich_mean": round(float(r.mean()), 1), "we_max": round(float(e.max()), 2),
                   "cwe_max": round(float(c.max()), 3), "groups": int(r.count())}
    print(name, stats[name])
# where the top endemism cells are
for name in ("self", "200"):
    rows_ = figs.read_csv(work / "output" / (("self_100" if name == "self" else f"circle_{name}") + ".csv"))
    top = sorted(rows_, key=lambda r: -float(r["ENDW_WE"]))[:5]
    print(name, "top WE", [(r["x"], r["y"], round(float(r["ENDW_WE"]), 2), r["RICHNESS_ALL"]) for r in top])
    topc = sorted(rows_, key=lambda r: -float(r["ENDW_CWE"]))[:5]
    print(name, "top CWE", [(r["x"], r["y"], round(float(r["ENDW_CWE"]), 3), r["RICHNESS_ALL"]) for r in topc])
    rr = np.array([float(r["RICHNESS_ALL"]) for r in rows_])
    ww = np.array([float(r["ENDW_WE"]) for r in rows_])
    cc = np.array([float(r["ENDW_CWE"]) for r in rows_])
    rk = lambda v: np.argsort(np.argsort(v))  # noqa: E731
    print(name, "Spearman richness-WE", round(float(np.corrcoef(rk(rr), rk(ww))[0, 1]), 2), "richness-CWE",
          round(float(np.corrcoef(rk(rr), rk(cc))[0, 1]), 2))
figs.write_csv(data / "scatter-200.csv", [{"richness": float(r["RICHNESS_ALL"]), "we": round(float(r["ENDW_WE"]), 3)}
                                           for r in figs.read_csv(work / "output" / "circle_200.csv")])

# ---------------------------------------------------------------- the cell sizes
for size in SIZES:
    t = work / f"self_{size // 1000}.tif"
    r, ex = band(t, "RICHNESS_ALL")
    _bio.cell_map(r, out / f"cells_{size // 1000}.png", cmap="viridis", vmin=0, vmax=None, unit="Species", extent=ext)
    e = band(t, "ENDW_WE")[0]
    print("cell size", size, "groups", int(r.count()), "max richness", int(r.max()), "mean", round(float(r.mean()), 2),
          "max WE", round(float(e.max()), 2))
