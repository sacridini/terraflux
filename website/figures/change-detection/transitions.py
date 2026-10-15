"""Figures of 'What changed: transitions, gains and losses' (change-detection/transitions).

Sibuyan's land use in 1997 and 2011 cross-tabulated by Terraflux (change.crosstab, change.transitions,
change.components, change.suggest), a toy pair of maps run through the same operators, and the components of the
change at four resolutions (raster.resample by the most common class).

    python website/figures/change-detection/transitions.py
"""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _common as c  # noqa: E402
from _common import figs, np, plt  # noqa: E402

SLUG = "transitions"
work, img, data = c.setup(SLUG)

MODEL = """format = "terraflux/1"

[[nodes]]
id = "land_1997"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "1997" }

[[nodes]]
id = "land_2011"
op = "raster.read"
params = { path = "sibuyan_landuse.tif", band = "2011" }

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "sibuyan_landuse.tif" }

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "land_1997.map", to = "land_2011.map" }

[[nodes]]
id = "transitions"
op = "change.transitions"
inputs = { maps = "dates.stack" }

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "land_1997.map", to = "land_2011.map" }

[[nodes]]
id = "suggest"
op = "change.suggest"
inputs = { maps = "dates.stack" }
params = { least = 5 }

[[nodes]]
id = "from_to"
op = "map.calc"
inputs = { a = "land_1997.map", b = "land_2011.map" }
params = { expression = "if a == b then 0 else a * 10 + b" }
"""

model = figs.add_writes(MODEL, {
    "from_to.map": "from_to.tif",
    "crosstab.table": "crosstab.csv", "transitions.table": "transitions.csv", "transitions.areas": "areas.csv",
    "components.table": "components.csv", "components.overall": "overall.csv", "suggest.table": "suggest.csv",
})
(work / "page_model.toml").write_text(MODEL, encoding="utf-8")
figs.run(model, work)
out = work / "output"

# ------------------------------------------------------------------------------------------------ the two dates
# (the bands from the file itself: raster.write of a raster.read node's map, a band chosen, writes every band of
# the file; see the report of this tutorial)
a97 = figs.read(work / "sibuyan_landuse.tif", 1)[0]
a11 = figs.read(work / "sibuyan_landuse.tif", 2)[0]
for name, a in (("land_1997", a97), ("land_2011", a11)):
    figs.raster(None, img / f"{name}.png", array=a, palette=c.SIB, legend=False, width=820)

fig, axes = plt.subplots(1, 2, figsize=(9.2, 4.0), dpi=150)
for ax, a, t in zip(axes, (a97, a11), ("1997", "2011")):
    c.classes_map(a, c.SIB, ax)
    ax.set_title(t, loc="left")
fig.legend(handles=c.legend_handles(c.SIB), loc="lower center", ncol=5, fontsize=9.5, bbox_to_anchor=(0.5, -0.02),
           handlelength=1.1, handleheight=1.1)
fig.subplots_adjust(bottom=0.12, wspace=0.04)
c.save(fig, img / "dates.png")

# ------------------------------------------------------------------------------------------------ the from-to map
ft = figs.read(work / "from_to.tif")[0]
groups = {
    1: ("Persistence", "#ebe7de", [0]),
    2: ("Forest to grassland", c.SIB[3][1], [13]),
    3: ("Forest to coconut", c.SIB[2][1], [12]),
    4: ("Forest to rice", c.SIB[4][1], [14]),
    5: ("Grassland or rice to coconut", "#6b7d2a", [32, 42]),
    6: ("Other transitions", "#c0392b", None),
}
g = np.ma.masked_all(ft.shape)
filled = ft.filled(-1)
listed = set()
for k, (_, _, vals) in groups.items():
    if vals is not None:
        g[np.isin(filled, vals)] = k
        listed |= set(vals)
g[(filled > 0) & ~np.isin(filled, list(listed))] = 6
g[ft.mask] = np.ma.masked
pal = {k: (n, col) for k, (n, col, _) in groups.items()}
figs.raster(None, img / "from_to.png", array=g, palette=pal, counts=True, width=900)

# ------------------------------------------------------------------------------------------------ tables and charts
names = {k: v[0] for k, v in c.SIB.items()}
ct = c.table(out / "crosstab.csv")
cls = [int(float(k)) for k in ct[0] if k != "from"]
rows = []
for r in ct:
    i = int(r["from"])
    tot = sum(r[str(j)] for j in cls)
    rows.append([names[i]] + [f"{int(r[str(j)])}" for j in cls] + [f"{int(tot)}"])
cols = [sum(r[str(j)] for r in ct) for j in cls]
rows.append(["**Total 2011**"] + [f"**{int(v)}**" for v in cols] + [f"**{int(sum(cols))}**"])
print("CROSSTAB (cells)\n" + c.md(rows, ["1997 \\ 2011"] + [names[j] for j in cls] + ["Total 1997"]))

comp = c.table(out / "components.csv")
gl = []
qes = []
for r in comp:
    if r["Category"] is None:
        print("OVERALL components (cells)", r)
        continue
    k = int(r["Category"])
    gain, loss = r["Gain"] * c.SIB_KM2, r["Loss"] * c.SIB_KM2
    gl.append({"class": names[k], "gain": round(gain, 4), "loss": round(-loss, 4), "net": round(gain - loss, 4)})
    qes.append({"class": names[k], "quantity": round(r["Quantity"] * c.SIB_KM2, 4),
                "exchange": round(r["Exchange"] * c.SIB_KM2, 4), "shift": round(r["Shift"] * c.SIB_KM2, 4)})
figs.write_csv(data / "gains_losses.csv", gl)
figs.write_csv(data / "components.csv", qes)
print("COMPONENTS\n" + c.md([[r["class"], r["gain"], -r["loss"], r["net"]] for r in gl], ["class", "gain", "loss", "net"]))
print(c.md([[r["class"], r["quantity"], r["exchange"], r["shift"]] for r in qes], ["class", "q", "e", "s"]))
print("OVERALL", c.table(out / "overall.csv"))
print("AREAS", c.table(out / "areas.csv"))
print("SUGGEST\n" + c.md([[names[int(r["from"])], names[int(r["to"])], int(r["cells"]), r["km2"],
                            round(r["of_change"], 1), round(r["of_class"], 1), int(r["suggested"])]
                           for r in c.table(out / "suggest.csv")],
                          ["from", "to", "cells", "km2", "of_change", "of_class", "suggested"]))

# ------------------------------------------------------------------------------------------------ a toy
import rasterio  # noqa: E402
from rasterio.transform import from_origin  # noqa: E402

T1 = np.array([[1, 1, 1, 1, 2],
               [1, 1, 1, 2, 2],
               [1, 1, 3, 2, 2],
               [1, 3, 3, 2, 2],
               [3, 3, 3, 2, 2]], dtype="uint8")
T2 = np.array([[1, 1, 1, 2, 2],
               [1, 1, 2, 2, 2],
               [1, 3, 3, 2, 2],
               [3, 3, 2, 2, 2],
               [3, 3, 3, 3, 2]], dtype="uint8")
toy_work = figs.workdir(f"{c.CATEGORY}/{SLUG}-toy")
for name, arr in (("toy_t1.tif", T1), ("toy_t2.tif", T2)):
    with rasterio.open(toy_work / name, "w", driver="GTiff", width=5, height=5, count=1, dtype="uint8",
                       transform=from_origin(0, 5, 1, 1)) as ds:
        ds.write(arr, 1)
TOY = """format = "terraflux/1"

[[nodes]]
id = "t1"
op = "raster.read"
params = { path = "toy_t1.tif" }

[[nodes]]
id = "t2"
op = "raster.read"
params = { path = "toy_t2.tif" }

[[nodes]]
id = "crosstab"
op = "change.crosstab"
inputs = { from = "t1.map", to = "t2.map" }

[[nodes]]
id = "components"
op = "change.components"
inputs = { from = "t1.map", to = "t2.map" }

[outputs.crosstab]
from = "crosstab.table"

[outputs.components]
from = "components.table"

[outputs.overall]
from = "components.overall"
"""
figs.run(TOY, toy_work)
print("TOY crosstab", c.table(toy_work / "output" / "crosstab.csv"))
print("TOY components", c.table(toy_work / "output" / "components.csv"))
print("TOY overall", c.table(toy_work / "output" / "overall.csv"))

TOYPAL = {1: ("Forest", c.SIB[1][1]), 2: ("Farming", "#d9a441"), 3: ("Grassland", "#e8d9a0")}
LET = {1: "F", 2: "A", 3: "G"}
fig, axes = plt.subplots(1, 3, figsize=(9.6, 3.5), dpi=150)
for ax, arr, title in ((axes[0], T1, "Time 1"), (axes[1], T2, "Time 2"), (axes[2], None, "What changed")):
    ax.set_xlim(-0.5, 4.5)
    ax.set_ylim(4.5, -0.5)
    ax.set_aspect("equal")
    ax.set_axis_off()
    ax.set_title(title, loc="left")
    for i in range(5):
        for j in range(5):
            if arr is not None:
                v = arr[i, j]
                ax.add_patch(plt.Rectangle((j - 0.5, i - 0.5), 1, 1, facecolor=TOYPAL[v][1], edgecolor="white", lw=2))
                ax.text(j, i, LET[v], ha="center", va="center", fontsize=11, color="white" if v == 1 else "#333")
            else:
                a, b = T1[i, j], T2[i, j]
                if a == b:
                    ax.add_patch(plt.Rectangle((j - 0.5, i - 0.5), 1, 1, facecolor="#f1eee8", edgecolor="white", lw=2))
                else:
                    ax.add_patch(plt.Rectangle((j - 0.5, i - 0.5), 1, 1, facecolor="#fde0c5", edgecolor="#e8901f", lw=1.5,
                                               zorder=3))
                    ax.text(j, i, f"{LET[a]}→{LET[b]}", ha="center", va="center", fontsize=9.5, color="#7a3e00",
                            zorder=4)
fig.legend(handles=c.legend_handles(TOYPAL), loc="lower center", ncol=3, fontsize=9.5, bbox_to_anchor=(0.36, -0.03))
fig.subplots_adjust(bottom=0.1, wspace=0.08)
c.save(fig, img / "toy.png")

# ------------------------------------------------------------------------------------------------ resolutions
RES = """format = "terraflux/1"

[[nodes]]
id = "dates"
op = "raster.read"
params = { path = "sibuyan_landuse.tif" }

[[nodes]]
id = "coarser"
op = "raster.resample"
inputs = { raster = "dates.raster" }
params = { resolution = RESOLUTION, resampling = "mode" }

[[nodes]]
id = "components"
op = "change.components"
inputs = { maps = "coarser.raster" }

[[nodes]]
id = "write"
op = "raster.write"
inputs = { value = "coarser.raster" }
params = { path = "coarser.tif" }

[outputs.overall]
from = "components.overall"
"""
frames = []
curve = []
for res in (250, 500, 750, 1000):
    w = figs.workdir(f"{c.CATEGORY}/{SLUG}-res{res}")
    import shutil  # noqa: E402

    shutil.copy2(c.FILES["sibuyan_landuse.tif"], w / "sibuyan_landuse.tif")
    figs.run(RES.replace("RESOLUTION", str(res)), w)
    o = c.table(w / "output" / "overall.csv")[0]
    b1, _ = figs.read(w / "coarser.tif", 1)
    b2, _ = figs.read(w / "coarser.tif", 2)
    n = int(b1.count())
    k = res // 250 if res % 250 == 0 else 1
    ftc = np.ma.where((b1 == b2), 0, b1 * 10 + b2)
    ftc = np.ma.masked_where(b1.mask | b2.mask, ftc)
    big = np.kron(ftc.filled(-1), np.ones((k, k)))[:108, :128]
    big = np.pad(big, ((0, max(0, 108 - big.shape[0])), (0, max(0, 128 - big.shape[1]))), constant_values=-1)
    gg = np.ma.masked_all(big.shape)
    for kk, (_, _, vals) in groups.items():
        if vals is not None:
            gg[np.isin(big, vals)] = kk
    gg[(big > 0) & ~np.isin(big, list(listed))] = 6
    gg[big < 0] = np.ma.masked
    figs.raster(None, img / f"res_{res}.png", array=gg, palette=pal, width=760, legend=False)
    pct = lambda v: f"{v / n * 100:.1f}%"  # noqa: E731
    frames.append((res, n, o))
    curve.append({"resolution": res, "quantity": round(o["quantity"] / n * 100, 3),
                  "exchange": round(o["exchange"] / n * 100, 3), "shift": round(o["shift"] / n * 100, 3)})
    print(f"RES {res}: cells {n} change {pct(o['overall'])} quantity {pct(o['quantity'])} exchange "
          f"{pct(o['exchange'])} shift {pct(o['shift'])}")
figs.write_csv(data / "resolutions.csv", curve)
print("images:", sum(p.stat().st_size for p in img.glob("*.png")) / 1e6, "MB")
