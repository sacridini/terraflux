"""Figures of 'Sediment delivery ratio' (ecosystem-services/sediment).

services.sdr on Ribeirão Claro: the soil loss of the RUSLE, the index of connectivity and the delivery ratio, the
export, deposition and retention, the table by watershed, Borselli's k, the stream threshold, multiple flow
directions against D8, roads that drain to the streams, and the scenario of 2040.
"""

import numpy as np

import es_common as es
from es_common import figs, plt

SLUG = "sediment"
OUT = es.img(SLUG)
DAT = es.data(SLUG)
INPUTS = ["dem.tif", "erosivity.tif", "erodibility.tif", "lulc_2020.tif", "lulc_2040.tif", "biophysical_sdr.csv",
          "watersheds.gpkg", "roads_2020.tif"]
HA = 0.09


def model(lulc="lulc_2020.tif", drainage=False, **params):
    p = {"threshold": 1000, **params}
    text = f'''format = "terraflux/1"

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
id = "lulc"
op = "raster.read"
params = {{ path = "{lulc}" }}

[[nodes]]
id = "biophysical"
op = "table.read"
params = {{ path = "biophysical_sdr.csv" }}

[[nodes]]
id = "watersheds"
op = "vector.read"
params = {{ path = "watersheds.gpkg" }}
'''
    ins = 'dem = "dem.raster", erosivity = "erosivity.raster", erodibility = "erodibility.raster", ' \
          'lulc = "lulc.raster", biophysical = "biophysical.table", watersheds = "watersheds.vector"'
    if drainage:
        text += '''
[[nodes]]
id = "roads"
op = "raster.read"
params = { path = "roads_2020.tif" }
'''
        ins += ', drainage = "roads.raster"'
    ps = ", ".join(f"{k} = " + (f'"{v}"' if isinstance(v, str) else str(v)) for k, v in p.items())
    text += f'''
[[nodes]]
id = "sediment"
op = "services.sdr"
inputs = {{ {ins} }}
params = {{ {ps} }}
'''
    return text


def total(w, col="sed_export"):
    return sum(float(r[col]) for r in figs.read_csv(w / "output" / "ws.csv"))


def curves():
    ic = np.linspace(-6, 4, 300)

    def draw(ax):
        for k, col in zip([0.5, 1, 2, 3], figs.CLASSES):
            ax.plot(ic, 0.8 / (1 + np.exp((0.5 - ic) / k)), color=col, lw=2.2, label=f"k = {k}")
        ax.axvline(0.5, color=figs.INK2, lw=1, ls=":")
        ax.text(0.6, 0.05, "IC$_0$ = 0.5", color=figs.INK2, fontsize=9.5)
        ax.axhline(0.8, color=figs.INK2, lw=1, ls="--")
        ax.text(-5.9, 0.82, "SDR$_{max}$ = 0.8", color=figs.INK2, fontsize=9.5)
        ax.set_xlabel("Index of connectivity IC")
        ax.set_ylabel("Sediment delivery ratio")
        ax.set_ylim(0, 0.9)
        ax.legend(loc="center left", bbox_to_anchor=(1.01, 0.5))
    figs.chart(OUT / "sdr-curve.png", draw, width=900, height=400)


def main():
    curves()
    w = es.work(SLUG, "main")
    es.copy_inputs(w, INPUTS)
    outs = ["usle", "sed_export", "sed_deposition", "avoided_export", "avoided_erosion", "rkls", "sdr", "ic",
            "stream", "drains_to_stream"]
    writes = {f"sediment.{k}": f"{k}.tif" for k in outs}
    writes["sediment.watershed_table"] = "ws.csv"
    es.run(model(), w, writes)
    figs.raster(w / "erosivity.tif", OUT / "erosivity.png", cmap="PuBu", unit="R, MJ mm / (ha h yr)", width=520)
    figs.raster(w / "erodibility.tif", OUT / "erodibility.png", cmap="YlOrBr", unit="K, t ha h / (ha MJ mm)",
                width=520)
    figs.raster(w / "rkls.tif", OUT / "rkls.png", cmap="magma_r", unit="t / ha / yr (bare soil)", width=520, log=True,
                vmin=1)
    figs.raster(w / "usle.tif", OUT / "usle.png", cmap="magma_r", unit="t / ha / yr", width=520, log=True, vmin=0.01)
    figs.raster(w / "ic.tif", OUT / "ic.png", cmap="viridis", unit="IC", width=520)
    figs.raster(w / "sdr.tif", OUT / "sdr.png", cmap="viridis", unit="SDR", width=520, vmin=0, vmax=0.8)
    figs.raster(w / "sed_export.tif", OUT / "sed_export.png", cmap="magma_r", unit="t / ha / yr", width=520, log=True,
                vmin=0.001)
    figs.raster(w / "sed_deposition.tif", OUT / "sed_deposition.png", cmap="cividis_r", unit="t / ha / yr", width=520,
                log=True, vmin=0.01)
    figs.raster(w / "avoided_erosion.tif", OUT / "avoided_erosion.png", cmap="Greens", unit="t / ha / yr", width=520,
                log=True, vmin=1)
    figs.raster(w / "avoided_export.tif", OUT / "avoided_export.png", cmap="Greens", unit="t / ha / yr", width=520,
                log=True, vmin=0.1)
    rows = [{k: round(float(v), 1) if k != "ws_id" else int(float(v)) for k, v in r.items()}
            for r in figs.read_csv(w / "output" / "ws.csv")]
    figs.write_csv(DAT / "watersheds.csv", rows)
    print(rows)

    # by class: soil loss and export per hectare
    lu = es.grid(w / "lulc_2020.tif")
    usle, exp_, sdr = es.grid(w / "usle.tif"), es.grid(w / "sed_export.tif"), es.grid(w / "sdr.tif")
    byc = []
    for k, (name, _) in es.CLASSES.items():
        m = (lu == k) & ~np.isnan(usle)
        if m.sum() == 0:
            continue
        byc.append({"class": name, "usle": round(float(np.nanmean(usle[m])), 2),
                    "sdr": round(float(np.nanmean(sdr[m])), 3), "export": round(float(np.nanmean(exp_[m])), 3),
                    "export_total": round(float(np.nansum(exp_[m])) * HA, 1)})
    figs.write_csv(DAT / "by-class.csv", byc)
    print(byc)

    # Borselli's k
    kr = []
    for k in [0.5, 1, 1.5, 2, 2.5, 3, 4]:
        wk = es.work(SLUG, f"k{k}")
        es.copy_inputs(wk, INPUTS)
        es.run(model(k=k), wk, {"sediment.watershed_table": "ws.csv"})
        kr.append({"k": k, "export": round(total(wk)), "deposition": round(total(wk, "sed_dep"))})
    figs.write_csv(DAT / "k.csv", kr)
    print(kr)

    # the stream threshold
    tr = []
    for t in (250, 500, 1000, 2000, 4000):
        wt = es.work(SLUG, f"t{t}")
        es.copy_inputs(wt, INPUTS)
        es.run(model(threshold=t), wt, {"sediment.sed_export": "sed_export.tif", "sediment.stream": "stream.tif",
                                        "sediment.watershed_table": "ws.csv"})
        ex = es.grid(wt / "sed_export.tif")
        st = es.grid(wt / "stream.tif")
        export_with_streams(ex, st, OUT / f"threshold_{t}.png")
        tr.append({"threshold": t, "stream_ha": round(float((st == 1).sum()) * HA, 1), "export": round(total(wt)),
                   "usle": round(total(wt, "usle_tot"))})
    print(tr)

    # D8 against multiple flow directions
    wd = es.work(SLUG, "d8")
    es.copy_inputs(wd, INPUTS)
    es.run(model(method="d8"), wd, {"sediment.sdr": "sdr.tif", "sediment.watershed_table": "ws.csv"})
    for src, png in ((w / "sdr.tif", "sdr_mfd.png"), (wd / "sdr.tif", "sdr_d8.png")):
        figs.raster(src, OUT / png, cmap="viridis", vmin=0, vmax=0.8, legend=False, width=760)
    methods = [{"method": "mfd", "export": round(total(w)), "deposition": round(total(w, "sed_dep"))},
               {"method": "d8", "export": round(total(wd)), "deposition": round(total(wd, "sed_dep"))}]
    print(methods)

    # roads that drain to the streams
    wr = es.work(SLUG, "drainage")
    es.copy_inputs(wr, INPUTS)
    es.run(model(drainage=True), wr, {"sediment.sed_export": "sed_export.tif", "sediment.watershed_table": "ws.csv"})
    figs.raster(None, OUT / "drainage_change.png", array=es.grid(wr / "sed_export.tif") - exp_, diverging=True,
                cmap="PuOr_r", unit="export with the roads - without, t / ha / yr")
    methods.append({"method": "mfd, roads drain to the streams", "export": round(total(wr)),
                    "deposition": round(total(wr, "sed_dep"))})
    figs.write_csv(DAT / "routing.csv", methods)

    # 2040
    wf = es.work(SLUG, "future")
    es.copy_inputs(wf, INPUTS)
    es.run(model(lulc="lulc_2040.tif"), wf, {"sediment.sed_export": "sed_export.tif",
                                             "sediment.watershed_table": "ws.csv"})
    figs.raster(None, OUT / "export_change.png", array=es.grid(wf / "sed_export.tif") - exp_, diverging=True,
                cmap="PuOr_r", unit="export, 2040 - 2020, t / ha / yr", vmin=-1, vmax=1)
    sc = []
    for col, label in (("usle_tot", "Soil loss"), ("sed_export", "Sediment export"), ("sed_dep", "Deposition"),
                       ("avoid_exp", "Avoided export"), ("avoid_eros", "Avoided erosion")):
        sc.append({"quantity": label + " (t / yr)", "2020": round(total(w, col)), "2040": round(total(wf, col)),
                   "change": f"{(total(wf, col) / total(w, col) - 1) * 100:+.1f}%"})
    figs.write_csv(DAT / "scenario.csv", sc)
    print(sc)


def export_with_streams(ex, st, png):
    fig = plt.figure(figsize=(820 / 150, 820 / 150 * 0.8 * es.H / es.W), dpi=150)
    ax = fig.add_axes([0, 0, 0.8, 1])
    cm = plt.get_cmap("magma_r").copy()
    cm.set_bad((1, 1, 1, 0))
    from matplotlib import colors as mc

    im = ax.imshow(np.ma.masked_invalid(np.where(ex > 0, ex, np.nan)), cmap=cm, norm=mc.LogNorm(0.001, 10),
                   interpolation="nearest")
    ax.imshow(np.where(st == 1, 1, np.nan), cmap=mc.ListedColormap([figs.WATER]), interpolation="nearest")
    ax.set_axis_off()
    cax = fig.add_axes([0.84, 0.1, 0.03, 0.8])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.ax.grid(False)
    cb.set_label("export, t / ha / yr (streams in blue)", color=figs.INK2)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


if __name__ == "__main__":
    main()
