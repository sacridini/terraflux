"""Figures of 'Crop pollination' (ecosystem-services/pollination).

services.pollination on Ribeirão Claro, the cropland taken as coffee: three guilds of wild bees (ground-nesting
bees, stingless bees and carpenter bees, nesting in the ground or in cavities, active in a wet and a dry season),
the nesting and floral resources of the land uses, sixteen coffee farms. The supply and the abundance of each
guild, the abundance on the farms against the distance to the forest, the farms' yields, the foraging distance
and the half saturation, and the scenario of 2040. The tables and the farms are made here (illustrative values)
and zipped for the readers to website/static/data/tutorials/ecosystem-services/pollination/pollination-inputs.zip.

Every run uses one thread (RAYON_NUM_THREADS, OMP_NUM_THREADS and --threads 1): the machine's rule.
"""

import os

os.environ["RAYON_NUM_THREADS"] = "1"
os.environ["OMP_NUM_THREADS"] = "1"

import zipfile  # noqa: E402

import numpy as np  # noqa: E402
from scipy import ndimage as ndi  # noqa: E402

import es_common as es  # noqa: E402
from es_common import figs, plt, mcolors  # noqa: E402

SLUG = "pollination"
OUT = es.img(SLUG)
DAT = es.data(SLUG)

# -- the guilds (illustrative): ground-nesting solitary bees, stingless bees nesting in tree cavities, large
# carpenter bees boring into dead wood; alpha in metres
GUILDS = [
    {"species": "ground", "nesting_suitability_ground_index": 1.0, "nesting_suitability_cavity_index": 0.0,
     "foraging_activity_dry_index": 0.5, "foraging_activity_wet_index": 1.0, "alpha": 400,
     "relative_abundance": 0.4},
    {"species": "stingless", "nesting_suitability_ground_index": 0.0, "nesting_suitability_cavity_index": 1.0,
     "foraging_activity_dry_index": 1.0, "foraging_activity_wet_index": 1.0, "alpha": 700,
     "relative_abundance": 0.4},
    {"species": "carpenter", "nesting_suitability_ground_index": 0.0, "nesting_suitability_cavity_index": 1.0,
     "foraging_activity_dry_index": 0.6, "foraging_activity_wet_index": 1.0, "alpha": 1000,
     "relative_abundance": 0.2},
]
GUILD_NAMES = {"ground": "Ground-nesting bees", "stingless": "Stingless bees", "carpenter": "Carpenter bees"}
GUILD_COLORS = {"ground": "#b5651d", "stingless": "#2f6f9f", "carpenter": "#7b4fa0"}

# -- the land uses (illustrative): nesting substrates and flowers by season
BIO = {
    "columns": ["lucode", "nesting_ground_availability_index", "nesting_cavity_availability_index",
                "floral_resources_dry_index", "floral_resources_wet_index"],
    "rows": [[1, 0.5, 1.0, 0.5, 0.7],    # forest
             [2, 0.8, 0.1, 0.2, 0.4],    # pasture
             [3, 0.3, 0.05, 0.1, 0.6],   # cropland: coffee, flowering after the first rains
             [4, 0.4, 0.3, 0.5, 0.2],    # eucalyptus, flowering in the dry season
             [5, 0.1, 0.2, 0.3, 0.3],    # urban: gardens
             [6, 0.0, 0.0, 0.0, 0.0]],   # water
}
# -- the coffee farms' values (the same resources as the cropland's row)
FARM = {"crop_type": "coffee", "half_sat": 0.05, "season": "wet", "p_dep": 0.25, "fr_dry": 0.1, "fr_wet": 0.6,
        "n_cavity": 0.05, "n_ground": 0.3}
MANAGED = {3: 0.3, 9: 0.3}  # two farms rent hives of honey bees: p_managed 0.3, the others 0
MIN_FARM = 150  # cells (13.5 ha)
SPAN = 110  # the transect: its first 110 cells (3.3 km)

INPUTS = ["lulc_2020.tif", "lulc_2040.tif"]


def guild_toml(guilds) -> str:
    rows = []
    for g in guilds:
        vals = ", ".join(f'{k} = "{v}"' if isinstance(v, str) else f"{k} = {v:g}" for k, v in g.items())
        rows.append(f"  {{ {vals} }}")
    return "[\n" + ",\n".join(rows) + ",\n]"


def model(lulc="lulc_2020.tif", farms=False, guilds=None, farms_file="farms.gpkg") -> str:
    g = f'"{guilds}"' if isinstance(guilds, str) else guild_toml(guilds or GUILDS)
    m = f'''format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = {{ path = "{lulc}" }}

[[nodes]]
id = "biophysical"
op = "table.read"
params = {{ path = "biophysical.csv" }}
'''
    ins = 'lulc = "lulc.raster", biophysical = "biophysical.table"'
    if farms:
        m += f'''
[[nodes]]
id = "farms"
op = "vector.read"
params = {{ path = "{farms_file}" }}
'''
        ins += ', farms = "farms.vector"'
    m += f'''
[[nodes]]
id = "pollination"
op = "services.pollination"
inputs = {{ {ins} }}
params = {{ guilds = {g} }}
'''
    return m


def run(text, w, writes):
    r = figs.run(figs.add_writes(text, writes), w, "-q", "--threads", "1", check=False)
    if r.returncode != 0:
        raise SystemExit(f"terraflux run failed in {w}:\n{r.stdout}\n{r.stderr}")


def layers(path):
    """The bands of a stack written by raster.write, by their names."""
    import rasterio

    with rasterio.open(path) as ds:
        return {name: ds.read(b + 1, masked=True).astype("float64").filled(np.nan)
                for b, name in enumerate(ds.descriptions)}


def write_inputs(d):
    """The guild table, the biophysical table and the farms, in folder d."""
    import geopandas as gpd

    with open(d / "guilds.csv", "w", newline="", encoding="utf-8") as f:
        cols = list(GUILDS[0])
        f.write(",".join(cols) + "\n")
        for g in GUILDS:
            f.write(",".join(str(g[c]) for c in cols) + "\n")
    es.write_table(d / "biophysical.csv", BIO)
    src = es.landscape()
    a, b = es.grid(src / "lulc_2020.tif"), es.grid(src / "lulc_2040.tif")
    lab, n = ndi.label((a == 3) & (b == 3), structure=np.ones((3, 3)))
    size = np.bincount(lab.ravel())
    keep = [i for i in range(1, n + 1) if size[i] >= MIN_FARM]
    # number the farms from north-west to south-east (by their centres, in rows of 30 cells)
    cents = {i: ndi.center_of_mass(lab == i) for i in keep}
    keep.sort(key=lambda i: (int(cents[i][0] // 45), cents[i][1]))
    ids = np.zeros_like(lab)
    for k, i in enumerate(keep, start=1):
        ids[lab == i] = k
    g = es._polygonize(ids, "farm")
    g = g.sort_values("farm").reset_index(drop=True)
    for k, v in FARM.items():
        g[k] = v
    g["p_managed"] = [MANAGED.get(int(i), 0.0) for i in g["farm"]]
    g = g[["farm", "crop_type", "half_sat", "season", "p_dep", "p_managed", "fr_dry", "fr_wet", "n_cavity",
           "n_ground", "geometry"]]
    g.to_file(d / "farms.gpkg", driver="GPKG")
    return ids


# ------------------------------------------------------------------------------------------------ drawings


LIGHT = {1: "#cfe0d3", 2: "#e7eed8", 3: "#f3e3c3", 4: "#cfe6e2", 5: "#e8d3df", 6: "#c9dbe9"}


def on_land(a, png, lulc, vmin=0, vmax=None, cmap="viridis", unit="", outline=None, numbers=False, width=900,
            legend=True):
    """Values over a pale land use map (cells where the values are NaN show the land use)."""
    fig = plt.figure(figsize=(width / 150, width / 150 * (0.74 if legend else 1.0) * es.H / es.W), dpi=150)
    ax = fig.add_axes([0, 0, 0.8 if legend else 1, 1])
    base = np.zeros((es.H, es.W, 4))
    for k, c in LIGHT.items():
        base[lulc == k] = mcolors.to_rgba(c)
    ax.imshow(base, interpolation="nearest")
    cm = plt.get_cmap(cmap).copy()
    cm.set_bad((1, 1, 1, 0))
    im = ax.imshow(np.ma.masked_invalid(a), cmap=cm, vmin=vmin, vmax=vmax, interpolation="nearest")
    if outline is not None:
        ax.contour(outline > 0, levels=[0.5], colors=[figs.INK], linewidths=0.7)
        if numbers:
            for k in range(1, int(outline.max()) + 1):
                r, c = ndi.center_of_mass(outline == k)
                ax.text(c, r, str(k), ha="center", va="center", fontsize=7.5, fontweight="bold", color=figs.INK,
                        bbox=dict(boxstyle="round,pad=0.12", fc="white", ec="none", alpha=0.85))
    ax.set_axis_off()
    if legend:
        cax = fig.add_axes([0.84, 0.1, 0.03, 0.8])
        cb = fig.colorbar(im, cax=cax)
        cb.outline.set_visible(False)
        cb.ax.grid(False)
        cb.ax.tick_params(labelsize=9)
        cb.set_label(unit, color=figs.INK2, fontsize=9.5)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


def farms_map(lulc, ids):
    """The land use of 2020 with the farms outlined and numbered."""
    fig = plt.figure(figsize=(6.4, 6.4 * 0.74 * es.H / es.W + 0.1), dpi=150)
    ax = fig.add_axes([0, 0, 0.74, 1])
    rgba = np.zeros((es.H, es.W, 4))
    for k, (name, c) in es.CLASSES.items():
        rgba[lulc == k] = mcolors.to_rgba(c)
    ax.imshow(rgba, interpolation="nearest")
    ax.contour(ids > 0, levels=[0.5], colors=["white"], linewidths=1.8)
    ax.contour(ids > 0, levels=[0.5], colors=[figs.INK], linewidths=0.8)
    for k in range(1, int(ids.max()) + 1):
        r, c = ndi.center_of_mass(ids == k)
        ax.text(c, r, str(k), ha="center", va="center", fontsize=7.5, fontweight="bold", color=figs.INK,
                bbox=dict(boxstyle="round,pad=0.12", fc="white", ec="none", alpha=0.9))
    ax.set_axis_off()
    names = dict(es.NAMES)
    names[3] = "Cropland (coffee)"
    handles = [es.Patch(facecolor=c, edgecolor="#00000022", label=names[k]) for k, (_, c) in es.CLASSES.items()]
    handles.append(es.Patch(facecolor="none", edgecolor=figs.INK, label="Farm (numbered)"))
    fig.legend(handles=handles, loc="center left", bbox_to_anchor=(0.75, 0.5), fontsize=8.5, handlelength=1.1,
               handleheight=1.1, borderaxespad=0, labelspacing=0.55)
    fig.savefig(OUT / "farms.png", dpi=150)
    plt.close(fig)
    figs._shrink(OUT / "farms.png")


def kernel_chart():
    d = np.linspace(0, 5200, 1041)

    def draw(ax):
        for g in GUILDS:
            a = g["alpha"]
            w = np.where(d <= 5 * a, np.exp(-d / a), np.nan)
            ax.plot(d, w, color=GUILD_COLORS[g["species"]], lw=2.2,
                    label=f'{GUILD_NAMES[g["species"]]}: alpha = {a} m')
            ax.plot([5 * a], [np.exp(-5)], "o", color=GUILD_COLORS[g["species"]], ms=4)
        ax.axhline(np.exp(-1), color=figs.INK2, lw=0.8, ls=":")
        ax.text(5150, np.exp(-1) + 0.02, "e$^{-1}$ = 0.37: at d = alpha", ha="right", va="bottom", fontsize=9,
                color=figs.INK2)
        ax.set_xlabel("Distance from the nest (m)")
        ax.set_ylabel("Weight exp(-d / alpha)")
        ax.set_xlim(0, 5200)
        ax.set_ylim(0, 1.03)
        ax.legend(loc="upper right")
    figs.chart(OUT / "kernel.png", draw, width=900, height=400)


def fp_chart(pat_farms):
    p = np.linspace(0, 0.4, 401)

    def draw(ax):
        lo, hi = np.nanpercentile(pat_farms, [2, 98])
        ax.axvspan(lo, hi, color=figs.RULE, alpha=0.7, lw=0)
        ax.text((lo + hi) / 2, 0.06, "abundance on\nthe farms", ha="center", fontsize=9, color=figs.INK2)
        for h, col in zip((0.05, 0.1, 0.2, 0.3), figs.CLASSES):
            ax.plot(p, p * (1 - h) / (h * (1 - 2 * p) + p), color=col, lw=2, label=f"h = {h}")
            ax.plot([h], [0.5], "o", color=col, ms=4)
        ax.set_xlabel("Pollinator abundance on the farm, PAT")
        ax.set_ylabel("FP")
        ax.set_xlim(0, 0.4)
        ax.set_ylim(0, 1.02)
        ax.legend(loc="center left", bbox_to_anchor=(1.01, 0.5))
    figs.chart(OUT / "half-sat.png", draw, width=900, height=400)


def transect(lulc, ab, sup, row):
    """Land use, supply and abundance (wet season) along one row of the map."""
    x = np.arange(SPAN) * es.PIXEL / 1000
    fig, axs = plt.subplots(3, 1, figsize=(6.4, 4.6), dpi=150, sharex=True,
                            gridspec_kw={"height_ratios": [0.35, 1, 1]})
    rgba = np.zeros((1, SPAN, 4))
    for k, (_, c) in es.CLASSES.items():
        rgba[0, lulc[row, :SPAN] == k] = mcolors.to_rgba(c)
    axs[0].imshow(rgba, aspect="auto", extent=(0, x[-1] + 0.03, 0, 1), interpolation="nearest")
    axs[0].set_yticks([])
    axs[0].grid(False)
    axs[0].set_title(f"Land use along row {row} (forest green, pasture light green, coffee ochre)", loc="left",
                     fontsize=10)
    for g in GUILDS:
        s = g["species"]
        axs[1].plot(x, sup[s][row, :SPAN], color=GUILD_COLORS[s], lw=1.8, label=GUILD_NAMES[s])
        axs[2].plot(x, ab[f"{s}_wet"][row, :SPAN], color=GUILD_COLORS[s], lw=1.8)
    axs[1].set_ylabel("Supply PS")
    axs[2].set_ylabel("Abundance PA, wet")
    axs[2].set_xlabel("Distance from the western edge (km)")
    axs[1].legend(loc="upper left", bbox_to_anchor=(1.01, 1.0), fontsize=8.5)
    fig.tight_layout()
    fig.savefig(OUT / "transect.png", dpi=150)
    plt.close(fig)
    figs._shrink(OUT / "transect.png")


# ------------------------------------------------------------------------------------------------ main


def main():
    for old in OUT.glob("*.png"):  # no figures left from earlier settings
        old.unlink()
    kernel_chart()
    src = es.landscape()
    lu20, lu40 = es.grid(src / "lulc_2020.tif"), es.grid(src / "lulc_2040.tif")

    # the inputs readers download
    dz = es.work(SLUG, "inputs")
    ids = write_inputs(dz)
    with zipfile.ZipFile(DAT / "pollination-inputs.zip", "w", zipfile.ZIP_DEFLATED) as z:
        for n in ("guilds.csv", "biophysical.csv", "farms.gpkg"):
            z.write(dz / n, f"pollination/{n}")
    farms_map(lu20, ids)
    import geopandas as gpd

    fg = gpd.read_file(dz / "farms.gpkg")
    print(len(fg), "farms")

    def prepare(name):
        w = es.work(SLUG, name)
        es.copy_inputs(w, INPUTS)
        for n in ("guilds.csv", "biophysical.csv", "farms.gpkg"):
            (w / n).write_bytes((dz / n).read_bytes())
        return w

    allout = {"pollination.supply": "supply.tif", "pollination.abundance": "abundance.tif",
              "pollination.total_abundance": "total.tif"}
    farmout = {"pollination.farm_pollinators": "farm_pollinators.tif", "pollination.total_yield": "total_yield.tif",
               "pollination.wild_yield": "wild_yield.tif", "pollination.farm_table": "farm_table.csv"}

    # 1. the land use alone, the guilds as a list and as InVEST's table: the same maps
    w1 = prepare("list")
    run(model(), w1, allout)
    w2 = prepare("file")
    run(model(guilds="guilds.csv"), w2, allout)
    same = all(np.array_equal(es.grid(w1 / f, b), es.grid(w2 / f, b), equal_nan=True)
               for f, nb in (("supply.tif", 3), ("abundance.tif", 6), ("total.tif", 2)) for b in range(1, nb + 1))
    print("guilds.csv gives the same maps as the list:", same)

    # 2. 2020 with the farms
    w = prepare("farms2020")
    run(model(farms=True), w, {**allout, **farmout})
    sup, ab, tot = layers(w / "supply.tif"), layers(w / "abundance.tif"), layers(w / "total.tif")
    print("supply", {k: (float(np.nanmin(v)), float(np.nanmax(v))) for k, v in sup.items()})
    print("total", {k: (float(np.nanmin(v)), float(np.nanmean(v)), float(np.nanmax(v))) for k, v in tot.items()})
    # without farms the same supply? (the farms carry the cropland's values)
    sup1 = layers(w1 / "supply.tif")
    print("farms change the supply:", not all(np.array_equal(sup[k], sup1[k], equal_nan=True) for k in sup))

    smax = max(float(np.nanmax(v)) for v in sup.values())
    for s, a in sup.items():
        figs.raster(None, OUT / f"supply_{s}.png", array=a, cmap="YlGnBu", vmin=0, vmax=smax,
                    unit="pollinator supply", width=520)
    tmax = max(float(np.nanmax(v)) for v in tot.values())
    for se, a in tot.items():
        figs.raster(None, OUT / f"total_{se}.png", array=a, cmap="viridis", vmin=0, vmax=tmax,
                    unit="pollinator abundance", width=620)
    amax = max(float(np.nanmax(v)) for v in ab.values())
    for k, a in ab.items():
        if k.endswith("_wet"):
            figs.raster(None, OUT / f"abundance_{k}.png", array=a, cmap="viridis", vmin=0, vmax=amax,
                        unit="abundance, wet season", width=520)

    # a transect across forest and coffee: the row whose cells change most often between forest and cropland
    best, row = -1, 0
    for r in range(20, es.H - 20):
        lr = lu20[r, :SPAN]
        n = np.sum(((lr[:-1] == 1) & (lr[1:] == 3)) | ((lr[:-1] == 3) & (lr[1:] == 1)) |
                   ((lr[:-1] == 1) & (lr[1:] == 2)) | ((lr[:-1] == 2) & (lr[1:] == 3)))
        if n > best and (lr == 3).sum() > 20 and (lr == 1).sum() > 20 and (lr == 2).sum() > 10:
            best, row = n, r
    print("transect row", row)
    transect(lu20, ab, sup, row)

    # abundance on the coffee against the distance to the forest
    dist = ndi.distance_transform_edt(lu20 != 1) * es.PIXEL
    coffee = lu20 == 3
    edges = [0, 60, 120, 180, 240, 300, 360, 450, 600]
    rows = []
    for lo, hi in zip(edges[:-1], edges[1:]):
        m = coffee & (dist > lo) & (dist <= hi)
        r = {"distance": (lo + hi) / 2, "cells": int(m.sum())}
        for g in GUILDS:
            r[g["species"]] = round(float(np.nanmean(ab[f'{g["species"]}_wet'][m])), 4)
        r["total"] = round(float(np.nanmean(tot["wet"][m])), 4)
        rows.append(r)
    figs.write_csv(DAT / "distance.csv", rows)
    print(rows)

    # the farms
    fp = es.grid(w / "farm_pollinators.tif")
    on_land(fp, OUT / "farm_pollinators.png", lu20, vmin=0.7, vmax=0.85, unit="FP, wet season",
            outline=ids, numbers=True)
    ft = figs.read_csv(w / "output" / "farm_table.csv")
    farm_rows = []
    for r in ft:
        k = int(float(r["farm"]))
        m = ids == k
        farm_rows.append({"farm": k, "ha": round(float(m.sum()) * 0.09, 1),
                          "dist": round(float(np.mean(dist[m]))), "p_managed": MANAGED.get(k, 0.0),
                          "p_abund": round(float(r["p_abund"]), 3), "pdep_y_w": round(float(r["pdep_y_w"]), 3),
                          "y_tot": round(float(r["y_tot"]), 3), "y_wild": round(float(r["y_wild"]), 3)})
    figs.write_csv(DAT / "farms.csv", farm_rows)
    for r in farm_rows:
        print(r)
    pat_farms = tot["wet"][ids > 0]
    fp_chart(pat_farms)

    # the half saturation: the farms' mean wild-pollinator yield for four values of h
    hrows = []
    for h in (0.05, 0.1, 0.2, 0.3):
        wh = prepare(f"h{h}")
        g = gpd.read_file(wh / "farms.gpkg")
        g["half_sat"] = h
        g.to_file(wh / "farms_h.gpkg", driver="GPKG")
        run(model(farms=True, farms_file="farms_h.gpkg"), wh, {"pollination.farm_table": "farm_table.csv"})
        t = figs.read_csv(wh / "output" / "farm_table.csv")
        pw = np.array([float(r["pdep_y_w"]) for r in t])
        yw = np.array([float(r["y_wild"]) for r in t])
        hrows.append({"h": h, "pdep_mean": round(float(pw.mean()), 3), "pdep_min": round(float(pw.min()), 3),
                      "pdep_max": round(float(pw.max()), 3), "y_wild_mean": round(float(yw.mean()), 3)})
    figs.write_csv(DAT / "half-sat.csv", hrows)
    print(hrows)

    # the foraging distance of the stingless bees
    near, far = coffee & (dist <= 60), coffee & (dist > 240)
    arows, frames = [], []
    alphas = [100, 200, 350, 700, 1400]
    maps = {}
    for a in alphas:
        gs = [dict(g, alpha=a) if g["species"] == "stingless" else g for g in GUILDS]
        wa = prepare(f"alpha{a}")
        run(model(guilds=gs, farms=True), wa, {"pollination.abundance": "abundance.tif",
                                                "pollination.farm_table": "farm_table.csv"})
        maps[a] = layers(wa / "abundance.tif")["stingless_wet"]
        t = figs.read_csv(wa / "output" / "farm_table.csv")
        gw = maps[a]
        arows.append({"alpha": a, "near": round(float(np.nanmean(gw[near])), 4),
                      "far": round(float(np.nanmean(gw[far])), 4),
                      "farms": round(float(np.nanmean(gw[ids > 0])), 4),
                      "y_wild": round(float(np.mean([float(r["y_wild"]) for r in t])), 4)})
    vmax = max(float(np.nanmax(v)) for v in maps.values())
    for a in alphas:
        figs.raster(None, OUT / f"alpha_{a}.png", array=maps[a], cmap="viridis", vmin=0, vmax=vmax,
                    unit="stingless bees, wet season", width=820)
    figs.write_csv(DAT / "alpha.csv", arows)
    print(arows)

    # 2040: the same farms in the landscape of 2040
    wf = prepare("farms2040")
    run(model(lulc="lulc_2040.tif", farms=True), wf, {**allout, **farmout})
    tot40 = layers(wf / "total.tif")
    figs.raster(None, OUT / "total_wet_2040.png", array=tot40["wet"], cmap="viridis", vmin=0, vmax=tmax,
                unit="pollinator abundance", width=620)
    for name, a in (("2020", tot["wet"]), ("2040", tot40["wet"])):
        figs.raster(None, OUT / f"wet_{name}_plain.png", array=a, cmap="viridis", vmin=0, vmax=tmax, legend=False,
                    width=820)
    ch = tot40["wet"] - tot["wet"]
    m = float(np.nanmax(np.abs(ch)))
    figs.raster(None, OUT / "change.png", array=ch, diverging=True, cmap="RdBu", vmin=-m, vmax=m,
                unit="abundance, wet, 2040 - 2020", width=820)
    ft40 = figs.read_csv(wf / "output" / "farm_table.csv")
    dist40 = ndi.distance_transform_edt(lu40 != 1) * es.PIXEL
    scen = []
    for y, lu, t, tw, dd in (("current (2020)", lu20, ft, tot["wet"], dist),
                             ("scenario (2040)", lu40, ft40, tot40["wet"], dist40)):
        scen.append({"year": y, "forest_ha": round(float((lu == 1).sum()) * 0.09),
                     "coffee_ha": round(float((lu == 3).sum()) * 0.09),
                     "coffee_dist": round(float(np.mean(dd[lu == 3]))),
                     "coffee_pat": round(float(np.nanmean(tw[lu == 3])), 3),
                     "farms_pat": round(float(np.mean([float(r["p_abund"]) for r in t])), 3),
                     "farms_y_wild": round(float(np.mean([float(r["y_wild"]) for r in t])), 4)})
    figs.write_csv(DAT / "scenarios.csv", scen)
    print(scen)
    per = []
    for r20, r40 in zip(ft, ft40):
        k = int(float(r20["farm"]))
        per.append({"farm": k, "p_abund_2020": round(float(r20["p_abund"]), 3),
                    "p_abund_2040": round(float(r40["p_abund"]), 3),
                    "y_wild_2020": round(float(r20["y_wild"]), 4), "y_wild_2040": round(float(r40["y_wild"]), 4)})
    figs.write_csv(DAT / "farms-2040.csv", per)
    print(per)


if __name__ == "__main__":
    main()
