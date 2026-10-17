"""Figures of 'Habitat quality and rarity' (ecosystem-services/habitat-quality).

services.habitat_quality on Ribeirão Claro: the threats and how their impact decays, the degradation by threat
and in all, the quality and its half-saturation constant, the access of a park, the scenario of 2040 and the
rarity of the classes against 1985.
"""

import numpy as np

import es_common as es
from es_common import figs, plt

SLUG = "habitat-quality"
OUT = es.img(SLUG)
DAT = es.data(SLUG)

THREATS = ",\n".join(
    f'  {{ threat = "{t["threat"]}", max_dist = {t["max_dist"]}, weight = {t["weight"]}, decay = "{t["decay"]}" }}'
    for t in es.THREATS)


def model(k=0.35, access=False, future=False, base=False, threats=None):
    nodes = ['''format = "terraflux/1"

[[nodes]]
id = "lulc"
op = "raster.read"
params = { path = "lulc_2020.tif" }

[[nodes]]
id = "sensitivity"
op = "table.read"
params = { path = "sensitivity.csv" }
''']
    for y in ("2020", "2040") if future else ("2020",):
        for t in ("crops", "roads", "urban"):
            nodes.append(f'''
[[nodes]]
id = "{t}_{y}"
op = "raster.read"
params = {{ path = "{t}_{y}.tif" }}
''')
        nodes.append(f'''
[[nodes]]
id = "threats_{y}"
op = "stack.of"
inputs = {{ crops = "crops_{y}.raster", roads = "roads_{y}.raster", urban = "urban_{y}.raster" }}
''')
    inputs = {"lulc": "lulc.raster", "sensitivity": "sensitivity.table", "threat_maps": "threats_2020.stack"}
    if future:
        nodes.append('''
[[nodes]]
id = "lulc_2040"
op = "raster.read"
params = { path = "lulc_2040.tif" }
''')
        inputs.update(future="lulc_2040.raster", future_threat_maps="threats_2040.stack")
    if base:
        nodes.append('''
[[nodes]]
id = "lulc_1985"
op = "raster.read"
params = { path = "lulc_1985.tif" }
''')
        inputs["base"] = "lulc_1985.raster"
    if access:
        nodes.append('''
[[nodes]]
id = "access"
op = "vector.read"
params = { path = "access.gpkg" }
''')
        inputs["access"] = "access.vector"
    ins = ", ".join(f'{k} = "{v}"' for k, v in inputs.items())
    th = f'"{threats}"' if threats else f"[\n{THREATS}\n]"
    nodes.append(f'''
[[nodes]]
id = "habitat"
op = "services.habitat_quality"
inputs = {{ {ins} }}
params = {{ half_saturation = {k}, threats = {th} }}
''')
    return "".join(nodes)


def decay_chart():
    d = np.linspace(0, 3000, 601)

    def draw(ax):
        cols = {"crops": figs.EMBER, "roads": figs.INK2, "urban": "#b4508a"}
        for t in es.THREATS:
            m = t["max_dist"]
            if t["decay"] == "linear":
                i = np.clip(1 - d / m, 0, None)
            else:
                i = np.where(d > m, 0, np.exp(-2.99 * d / m))
            ax.plot(d, i, color=cols[t["threat"]], lw=2.2,
                    label=f'{t["threat"]}: {t["decay"]}, max_dist {t["max_dist"]} m')
        ax.set_xlabel("Distance to the nearest cell of the threat (m)")
        ax.set_ylabel("Impact $i_{rxy}$")
        ax.set_ylim(0, 1.05)
        ax.set_xlim(0, 3000)
        ax.legend(loc="upper right")
    figs.chart(OUT / "decay.png", draw, width=900, height=420)


def threat_maps():
    d = es.landscape()
    lu = es.grid(d / "lulc_2020.tif")
    fig, axs = plt.subplots(1, 3, figsize=(9.6, 2.7), dpi=150)
    for ax, t, col in zip(axs, ("crops", "roads", "urban"), (figs.EMBER, figs.INK, "#b4508a")):
        a = es.grid(d / f"{t}_2020.tif")
        bg = np.where(np.isnan(lu), np.nan, 1)
        ax.imshow(bg, cmap=plt.matplotlib.colors.ListedColormap(["#eef0ea"]), interpolation="nearest")
        ax.imshow(np.where(a > 0, 1, np.nan), cmap=plt.matplotlib.colors.ListedColormap([col]),
                  interpolation="nearest")
        ax.set_title(f"{t}_2020.tif", fontsize=10.5)
        ax.set_axis_off()
    fig.tight_layout()
    fig.savefig(OUT / "threat-maps.png", dpi=150)
    plt.close(fig)
    figs._shrink(OUT / "threat-maps.png")


def overlay(src, png, polys, vmin=0, vmax=1, cmap="viridis", unit="", legend=True, width=820):
    """A continuous map with the access polygons drawn over it."""
    import geopandas as gpd

    a = es.grid(src)
    g = gpd.read_file(polys)
    ext = (es.ORIGIN[0], es.ORIGIN[0] + es.W * es.PIXEL, es.ORIGIN[1] - es.H * es.PIXEL, es.ORIGIN[1])
    h = width / 150
    fig = plt.figure(figsize=(h, h * (0.8 if legend else 1.0) * es.H / es.W), dpi=150)
    ax = fig.add_axes([0, 0, 0.8 if legend else 1, 1])
    cm = plt.get_cmap(cmap).copy()
    cm.set_bad((1, 1, 1, 0))
    im = ax.imshow(np.ma.masked_invalid(a), cmap=cm, vmin=vmin, vmax=vmax, extent=ext, interpolation="nearest")
    for _, r in g.iterrows():
        ax.plot(*r.geometry.exterior.xy, color="white", lw=2.6)
        ax.plot(*r.geometry.exterior.xy, color=figs.INK, lw=1.2, ls="--" if r.access > 0 else "-")
    ax.set_xlim(ext[0], ext[1])
    ax.set_ylim(ext[2], ext[3])
    ax.set_axis_off()
    if legend:
        cax = fig.add_axes([0.84, 0.1, 0.03, 0.8])
        cb = fig.colorbar(im, cax=cax)
        cb.outline.set_visible(False)
        cb.ax.grid(False)
        cb.set_label(unit, color=figs.INK2)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


def main():
    decay_chart()
    threat_maps()
    inputs = ["lulc_1985.tif", "lulc_2020.tif", "lulc_2040.tif", "crops_2020.tif", "roads_2020.tif",
              "urban_2020.tif", "crops_2040.tif", "roads_2040.tif", "urban_2040.tif", "sensitivity.csv",
              "threats.csv", "access.gpkg"]

    # first run: the current map, k = 0.05 (InVEST's default)
    w = es.work(SLUG, "k005")
    es.copy_inputs(w, inputs)
    es.run(model(k=0.05), w, {"habitat.degradation": "degradation.tif", "habitat.quality": "quality.tif",
                              "habitat.threats": "threats.tif"})
    deg = es.grid(w / "degradation.tif")
    dmax = float(np.nanmax(deg))
    print("highest degradation", dmax)
    for b, t in enumerate(("crops", "roads", "urban"), start=1):
        figs.raster(w / "threats.tif", OUT / f"deg_{t}.png", band=b, cmap="magma_r", vmin=0, vmax=0.4,
                    unit="degradation", width=520)
    figs.raster(w / "degradation.tif", OUT / "degradation.png", cmap="magma_r", vmin=0, unit="total degradation D")
    # the same run reading InVEST's threats table: the same maps
    w2 = es.work(SLUG, "csv")
    es.copy_inputs(w2, inputs)
    es.run(model(k=0.05, threats="threats.csv"), w2, {"habitat.quality": "quality.tif"})
    same = np.array_equal(es.grid(w / "quality.tif"), es.grid(w2 / "quality.tif"), equal_nan=True)
    print("threats.csv gives the same quality:", same)

    lu = es.grid(w / "lulc_2020.tif")
    forest = lu == 1

    # the half-saturation constant
    ks = [0.05, 0.1, 0.2, 0.35, 0.5]
    rows = []
    for k in ks:
        wk = es.work(SLUG, f"k{k}")
        es.copy_inputs(wk, inputs)
        es.run(model(k=k), wk, {"habitat.quality": "quality.tif"})
        q = es.grid(wk / "quality.tif")
        figs.raster(wk / "quality.tif", OUT / f"quality_k{k}.png", cmap="viridis", vmin=0, vmax=1,
                    unit="habitat quality", width=820)
        rows.append({"k": k, "forest_mean": round(float(np.nanmean(q[forest])), 3),
                     "forest_below_half": round(float(np.mean(q[forest] < 0.5)) * 100, 1)})
    print(rows)

    # quality and degradation along a transect? the relation D -> Q for each k
    dd = np.linspace(0, 0.8, 161)

    def draw(ax):
        for k, col in zip(ks, figs.CLASSES):
            ax.plot(dd, 1 - dd ** 2.5 / (dd ** 2.5 + k ** 2.5), color=col, lw=2, label=f"k = {k}")
        ax.axvline(dmax, color=figs.INK2, lw=1, ls=":")
        ax.text(dmax - 0.01, 0.97, "highest D\nin the landscape", ha="right", va="top", fontsize=9, color=figs.INK2)
        ax.set_xlabel("Total degradation D")
        ax.set_ylabel("Quality of a class with H = 1")
        ax.set_xlim(0, 0.8)
        ax.set_ylim(0, 1.03)
        ax.legend(loc="center left", bbox_to_anchor=(1.01, 0.5))
    figs.chart(OUT / "saturation.png", draw, width=900, height=400)

    # access: a park (0) and its buffer zone (0.5)
    wa = es.work(SLUG, "access")
    es.copy_inputs(wa, inputs)
    es.run(model(k=0.35, access=True), wa, {"habitat.quality": "quality.tif"})
    w35 = figs.WEB / ".cache" / "work" / es.CATEGORY / SLUG / "k0.35"
    overlay(w35 / "quality.tif", OUT / "access_without.png", wa / "access.gpkg", legend=False)
    overlay(wa / "quality.tif", OUT / "access_with.png", wa / "access.gpkg", legend=False)
    qa, q0 = es.grid(wa / "quality.tif"), es.grid(w35 / "quality.tif")
    import geopandas as gpd
    from rasterio import features

    g = gpd.read_file(wa / "access.gpkg")
    zone = features.rasterize([(geom, i + 1) for i, geom in enumerate(g.geometry)], out_shape=(es.H, es.W),
                              transform=es.transform())
    acc_rows = []
    for i, r in g.iterrows():
        m = (zone == i + 1) & forest
        acc_rows.append({"area": r["name"], "access": r.access, "forest_quality_without": round(float(np.nanmean(q0[m])), 3),
                         "forest_quality_with": round(float(np.nanmean(qa[m])), 3)})
    figs.write_csv(DAT / "access.csv", acc_rows)
    print(acc_rows)

    # the future and the rarity
    wf = es.work(SLUG, "future")
    es.copy_inputs(wf, inputs)
    es.run(model(k=0.35, future=True, base=True), wf,
           {"habitat.quality": "quality.tif", "habitat.quality_future": "quality_future.tif",
            "habitat.degradation_future": "degradation_future.tif", "habitat.rarity": "rarity.tif",
            "habitat.rarity_future": "rarity_future.tif", "habitat.rarity_table": "rarity.csv"})
    q20, q40 = es.grid(wf / "quality.tif"), es.grid(wf / "quality_future.tif")
    figs.raster(None, OUT / "quality_change.png", array=q40 - q20, diverging=True, cmap="RdBu",
                unit="quality, 2040 - 2020", vmin=-1, vmax=1)
    figs.raster(wf / "quality_future.tif", OUT / "quality_2040.png", cmap="viridis", vmin=0, vmax=1,
                unit="habitat quality")
    lu40 = es.grid(wf / "lulc_2040.tif")
    hab = {int(r[0]): r[1] for r in es.SENSITIVITY["rows"]}
    summ = []
    for y, q, l in (("2020", q20, lu), ("2040", q40, lu40)):
        hcells = np.isin(l, [k for k, v in hab.items() if v > 0])
        summ.append({"year": {"2020": "current (2020)", "2040": "scenario (2040)"}[y], "forest_ha": round(float((l == 1).sum()) * 0.09),
                     "mean_quality_forest": round(float(np.nanmean(q[l == 1])), 3),
                     "habitat_quality_ha": round(float(np.nansum(q[hcells])) * 0.09)})
    figs.write_csv(DAT / "scenarios.csv", summ)
    print(summ)
    rt = figs.read_csv(wf / "output" / "rarity.csv")
    rows = []
    for r in rt:
        rows.append({"scenario": "current (2020)" if float(r["scenario"]) == 0 else "future (2040)",
                     "class": es.NAMES[int(float(r["lucode"]))], "rarity": round(float(r["rarity"]), 3)})
    figs.write_csv(DAT / "rarity.csv", rows)
    print(rows)
    figs.raster(wf / "rarity.tif", OUT / "rarity.png", cmap="cividis", vmin=0, vmax=1, unit="rarity, 2020 against 1985")


if __name__ == "__main__":
    main()
