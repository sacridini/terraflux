"""Figures of 'Nutrient delivery ratio' (ecosystem-services/nutrients).

services.ndr on Ribeirão Claro: the effective retention along a flow path (a toy path drawn cell by cell), the
delivery ratio against the index of connectivity, the maps of retention, delivery and export of nitrogen and
phosphorus, the table by watershed and by class, the stream threshold, Borselli's k, the critical lengths, the
subsurface parameters, D8 against multiple flow directions, and the scenario of 2040.

The landscape (es_common) has no nutrient table: this script writes one, biophysical_ndr.csv, with illustrative
loads and retention efficiencies for the land uses of the Cerrado and Atlantic forest, into its own work folders,
and ships it to the readers as website/static/data/tutorials/ecosystem-services/nutrients/ribeirao-claro-ndr.zip.
Every run of terraflux uses one thread (RAYON_NUM_THREADS=1, OMP_NUM_THREADS=1).
"""

import json
import os
import zipfile

os.environ["RAYON_NUM_THREADS"] = "1"
os.environ["OMP_NUM_THREADS"] = "1"

import numpy as np  # noqa: E402

import es_common as es  # noqa: E402
from es_common import figs, plt  # noqa: E402

SLUG = "nutrients"
OUT = es.img(SLUG)
DAT = es.data(SLUG)
INPUTS = ["dem.tif", "precipitation.tif", "lulc_2020.tif", "lulc_2040.tif", "watersheds.gpkg"]
HA = 0.09

# by class (illustrative): loads in kg / ha / yr as measured runoff (export coefficients), the largest retention
# efficiency of the class (0 to 1) and the distance over which it is reached (m)
BIOPHYSICAL_NDR = {
    "columns": ["lucode", "load_n", "eff_n", "crit_len_n", "proportion_subsurface_n", "load_p", "eff_p",
                "crit_len_p"],
    "rows": [[1, 2.5, 0.8, 150, 0.3, 0.1, 0.8, 150],     # forest
             [2, 8.0, 0.4, 60, 0.3, 0.5, 0.45, 60],      # pasture
             [3, 20.0, 0.15, 30, 0.4, 1.5, 0.2, 30],     # cropland (soy, maize)
             [4, 4.0, 0.6, 100, 0.3, 0.25, 0.6, 100],    # eucalyptus
             [5, 12.0, 0.05, 30, 0.1, 1.8, 0.05, 30],    # urban
             [6, 0.0, 0.0, 30, 0.0, 0.0, 0.0, 30]],      # water
}
SUB = {"subsurface_critical_length": 200, "subsurface_efficiency": 0.8}


def bio_table(factor=1.0):
    """The table, its critical lengths times `factor`."""
    rows = [[r[0], r[1], r[2], r[3] * factor, r[4], r[5], r[6], r[7] * factor] for r in BIOPHYSICAL_NDR["rows"]]
    return {"columns": BIOPHYSICAL_NDR["columns"], "rows": rows}


def model(lulc="lulc_2020.tif", table="biophysical_ndr.csv", **params):
    p = {"threshold": 1000, "k": 2, **SUB, **params}

    def val(v):
        if isinstance(v, dict):
            return "{ " + ", ".join(f'"{a}" = "{b}"' for a, b in v.items()) + " }"
        return f'"{v}"' if isinstance(v, str) else str(v)
    ps = ", ".join(f"{k} = {val(v)}" for k, v in p.items())
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
id = "rain"
op = "raster.read"
params = {{ path = "precipitation.tif" }}

[[nodes]]
id = "biophysical"
op = "table.read"
params = {{ path = "{table}" }}

[[nodes]]
id = "watersheds"
op = "vector.read"
params = {{ path = "watersheds.gpkg" }}

[[nodes]]
id = "nutrients"
op = "services.ndr"
inputs = {{ dem = "dem.raster", lulc = "lulc.raster", runoff_proxy = "rain.raster", biophysical = "biophysical.table", watersheds = "watersheds.vector" }}
params = {{ {ps} }}
'''


def setup(name, factor=1.0):
    w = es.work(SLUG, name)
    es.copy_inputs(w, INPUTS)
    es.write_table(w / "biophysical_ndr.csv", bio_table(factor))
    return w


def run(w, writes=None, **kw):
    wr = {"nutrients.watershed_table": "ws.csv", "nutrients.ic_0": "ic0.json",
          "nutrients.runoff_proxy_mean": "proxy_mean.json"}
    wr.update(writes or {})
    es.run(model(**kw), w, wr)


def total(w, col):
    return sum(float(r[col]) for r in figs.read_csv(w / "output" / "ws.csv"))


def number(w, name):
    out = w / "output"
    for f in out.iterdir():
        if f.stem == name:
            txt = f.read_text(encoding="utf-8").strip()
            try:
                v = json.loads(txt)
            except json.JSONDecodeError:
                return float(txt)
            return float(v if not isinstance(v, dict) else next(iter(v.values())))
    raise SystemExit(f"no output {name} in {out}: {[f.name for f in out.iterdir()]}")


# ------------------------------------------------------------------------------------------------ explanations


def retention_path(eff, crit, cell=30.0):
    """InVEST's effective retention down a single flow path (D8), from the cell next to the stream upward."""
    out = [0.0] * len(eff)
    down = None
    for i in range(len(eff) - 1, -1, -1):
        s = np.exp(-5 * cell / crit[i])
        if down is None:
            r = eff[i] * (1 - s)
        elif eff[i] > down:
            r = down * s + eff[i] * (1 - s)
        else:
            r = down
        out[i] = r
        down = r
    return out


def toy():
    """A hillslope of 10 cells above a stream: cropland only, and cropland above a strip of three forest cells."""
    t = {r[0]: r for r in BIOPHYSICAL_NDR["rows"]}
    crop, forest = es.CLASSES[3][1], es.CLASSES[1][1]
    cases = [("Cropland down to the stream", [3] * 10), ("Cropland above a forest strip of 90 m", [3] * 7 + [1] * 3)]
    rows = []
    fig, axes = plt.subplots(1, 2, figsize=(1000 / 150, 420 / 150), dpi=150, sharey=True)
    for ax, (title, path) in zip(axes, cases):
        eff = [t[c][2] for c in path]
        crit = [t[c][3] for c in path]
        r = retention_path(eff, crit)
        x = np.arange(1, len(path) + 1)
        ax.bar(x, r, color=[crop if c == 3 else forest for c in path], width=0.8, edgecolor="#00000022")
        ax.bar([len(path) + 1], [0.05], color=figs.WATER, width=0.8)
        ax.text(len(path) + 1, 0.08, "stream", rotation=90, ha="center", va="bottom", color=figs.WATER,
                fontsize=9.5)
        for xi, ri in zip(x, r):
            ax.text(xi, ri + 0.02, f"{ri:.2f}", ha="center", va="bottom", fontsize=8, color=figs.INK2)
        ax.set_title(title, loc="left", fontsize=10.5)
        ax.set_xticks(list(x))
        ax.set_xlabel("Cell of the path (30 m each), downslope →")
        ax.set_ylim(0, 1)
        ax.grid(axis="x", visible=False)
        rows.append({"case": title, **{f"cell_{i}": round(v, 3) for i, v in zip(x, r)}})
    axes[0].set_ylabel("Effective retention eff' (nitrogen)")
    fig.tight_layout()
    fig.savefig(OUT / "retention-path.png", dpi=150)
    plt.close(fig)
    figs._shrink(OUT / "retention-path.png")
    print(rows)


def curves(ic0):
    ic = np.linspace(ic0 - 6, ic0 + 6, 300)

    def draw(ax):
        for e, col in zip([0.0, 0.15, 0.4, 0.8], figs.CLASSES):
            ax.plot(ic, (1 - e) / (1 + np.exp((ic0 - ic) / 2)), color=col, lw=2.2, label=f"eff' = {e}")
        ax.axvline(ic0, color=figs.INK2, lw=1, ls=":")
        ax.text(ic0 + 0.1, 0.92, f"IC$_0$ = {ic0:.2f}", color=figs.INK2, fontsize=9.5)
        ax.set_xlabel("Index of connectivity IC")
        ax.set_ylabel("Nutrient delivery ratio NDR")
        ax.set_ylim(0, 1)
        ax.legend(loc="center left", bbox_to_anchor=(1.01, 0.5))
    figs.chart(OUT / "ndr-curve.png", draw, width=900, height=400)


def with_streams(a, st, png, unit, vmin, vmax, cmap="magma_r", width=820):
    from matplotlib import colors as mc

    fig = plt.figure(figsize=(width / 150, width / 150 * 0.8 * es.H / es.W), dpi=150)
    ax = fig.add_axes([0, 0, 0.8, 1])
    cm = plt.get_cmap(cmap).copy()
    cm.set_bad((1, 1, 1, 0))
    im = ax.imshow(np.ma.masked_invalid(np.where(a > 0, a, np.nan)), cmap=cm, norm=mc.LogNorm(vmin, vmax),
                   interpolation="nearest")
    ax.imshow(np.where(st == 1, 1, np.nan), cmap=mc.ListedColormap([figs.WATER]), interpolation="nearest")
    ax.set_axis_off()
    cax = fig.add_axes([0.84, 0.1, 0.03, 0.8])
    cb = fig.colorbar(im, cax=cax)
    cb.outline.set_visible(False)
    cb.ax.grid(False)
    cb.set_label(unit, color=figs.INK2)
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)


# ------------------------------------------------------------------------------------------------ the runs


OUTS = ["n_surface_export", "n_subsurface_export", "n_total_export", "p_surface_export", "ndr_n", "ndr_p",
        "effective_retention_n", "effective_retention_p", "ic", "stream", "drains_to_stream"]
NUT = ["n_surface_load", "n_surface_export", "n_subsurface_load", "n_subsurface_export", "n_total_export",
       "p_surface_load", "p_surface_export"]


def main():
    toy()
    w = setup("main")
    run(w, {f"nutrients.{k}": f"{k}.tif" for k in OUTS})
    ic0, pmean = number(w, "ic0"), number(w, "proxy_mean")
    print("ic_0", ic0, "proxy mean", pmean)
    curves(ic0)
    g = {k: es.grid(w / f"{k}.tif") for k in OUTS}
    st = g["stream"]
    print("stream ha", float((st == 1).sum()) * HA, "drains", float(np.nansum(g["drains_to_stream"] == 1)) * HA)
    for k in OUTS:
        a = g[k]
        print(k, "min", np.nanmin(a), "max", np.nanmax(a), "mean", np.nanmean(a), "nan", int(np.isnan(a).sum()))

    # the inputs
    figs.raster(w / "precipitation.tif", OUT / "runoff_proxy.png", cmap="PuBu", unit="precipitation, mm / yr",
                width=520)
    es.land_map(es.grid(w / "lulc_2020.tif"), OUT / "lulc_2020.png", width=520)

    # the results
    figs.raster(None, OUT / "ic.png", array=g["ic"], cmap="viridis", unit="IC", width=520)
    figs.raster(None, OUT / "effective_retention_n.png", array=g["effective_retention_n"], cmap="Greens",
                unit="eff', nitrogen", width=520, vmin=0, vmax=0.8)
    figs.raster(None, OUT / "ndr_n.png", array=g["ndr_n"], cmap="viridis", unit="NDR, nitrogen", width=520, vmin=0)
    figs.raster(None, OUT / "n_surface_export.png", array=g["n_surface_export"], cmap="magma_r",
                unit="kg / ha / yr", width=520, log=True, vmin=0.01)
    figs.raster(None, OUT / "n_subsurface_export.png", array=g["n_subsurface_export"], cmap="magma_r",
                unit="kg / ha / yr", width=520, log=True, vmin=0.01)
    figs.raster(None, OUT / "n_total_export.png", array=g["n_total_export"], cmap="magma_r", unit="kg / ha / yr",
                width=520, log=True, vmin=0.01)
    figs.raster(None, OUT / "effective_retention_p.png", array=g["effective_retention_p"], cmap="Greens",
                unit="eff', phosphorus", width=520, vmin=0, vmax=0.8)
    figs.raster(None, OUT / "p_surface_export.png", array=g["p_surface_export"], cmap="magma_r",
                unit="kg / ha / yr", width=520, log=True, vmin=0.001)

    rows = [{k: (int(float(v)) if k == "ws_id" else round(float(v), 1)) for k, v in r.items()}
            for r in figs.read_csv(w / "output" / "ws.csv")]
    figs.write_csv(DAT / "watersheds.csv", [{c: r[c] for c in ["ws_id"] + NUT} for r in rows])
    print(rows)
    # the total export has no value on the streams, the subsurface export has: the gap in the table
    gap = float(np.nansum(g["n_subsurface_export"][st == 1])) * HA
    print("subsurface export of the stream cells", gap, "table gap",
          total(w, "n_surface_export") + total(w, "n_subsurface_export") - total(w, "n_total_export"))

    # cropland's nitrogen load as an application rate (times 1 - eff_n), the other classes as measured runoff
    wa = setup("application")
    types = {str(r[0]): ("application-rate" if r[0] == 3 else "measured-runoff") for r in BIOPHYSICAL_NDR["rows"]}
    run(wa, load_type_n=types)
    print("application rate for cropland: N total", total(wa, "n_total_export"), "against", total(w, "n_total_export"))

    # by class: the means per hectare and the totals of export
    lu = es.grid(w / "lulc_2020.tif")
    tab = {r[0]: r for r in BIOPHYSICAL_NDR["rows"]}
    inside = (g["drains_to_stream"] == 0) | (g["drains_to_stream"] == 1)  # 255 outside the watersheds
    tot_n = np.nansum(g["n_total_export"][inside]) * HA
    tot_p = np.nansum(g["p_surface_export"][inside]) * HA
    byc = []
    for k, (name, _) in es.CLASSES.items():
        m = (lu == k) & inside
        if m.sum() == 0:
            continue
        en, ep = np.nansum(g["n_total_export"][m]) * HA, np.nansum(g["p_surface_export"][m]) * HA
        byc.append({"class": name, "area_ha": round(float(m.sum()) * HA), "load_n": tab[k][1],
                    "eff_n": round(float(np.nanmean(g["effective_retention_n"][m])), 3),
                    "ndr_n": round(float(np.nanmean(g["ndr_n"][m])), 3),
                    "export_n": round(float(en), 1), "share_n": round(float(en / tot_n * 100), 1),
                    "export_p": round(float(ep), 2), "share_p": round(float(ep / tot_p * 100), 1)})
    figs.write_csv(DAT / "by-class.csv", byc)
    print(byc)

    # the stream threshold
    tr = []
    for t in (250, 500, 1000, 2000, 4000):
        wt = setup(f"t{t}")
        run(wt, {"nutrients.n_total_export": "n.tif", "nutrients.stream": "stream.tif"}, threshold=t)
        with_streams(es.grid(wt / "n.tif"), es.grid(wt / "stream.tif"), OUT / f"threshold_{t}.png",
                     "N export, kg / ha / yr (streams in blue)", 0.01, 20)
        st_t = es.grid(wt / "stream.tif")
        tr.append({"threshold": t, "stream_ha": round(float((st_t == 1).sum()) * HA, 1),
                   "n_export": round(total(wt, "n_total_export")), "p_export": round(total(wt, "p_surface_export"), 1),
                   "ic_0": round(number(wt, "ic0"), 2)})
    figs.write_csv(DAT / "threshold.csv", tr)
    print(tr)

    # Borselli's k
    base_n, base_p = total(w, "n_surface_export"), total(w, "p_surface_export")
    kr = []
    for k in [0.5, 1, 1.5, 2, 2.5, 3, 4]:
        wk = setup(f"k{k}")
        run(wk, k=k)
        n_, p_ = total(wk, "n_surface_export"), total(wk, "p_surface_export")
        kr.append({"k": k, "n_surface": round(n_), "p_surface": round(p_, 1),
                   "n_rel": round(n_ / base_n * 100, 1), "p_rel": round(p_ / base_p * 100, 1),
                   "n_total": round(total(wk, "n_total_export"))})
    figs.write_csv(DAT / "k.csv", kr)
    print(kr)

    # the critical lengths of the classes, all scaled
    cr = []
    for f in [0.25, 0.5, 1, 2, 4]:
        wc = setup(f"crit{f}", factor=f)
        run(wc)
        n_, p_ = total(wc, "n_surface_export"), total(wc, "p_surface_export")
        cr.append({"factor": f, "n_surface": round(n_), "p_surface": round(p_, 1),
                   "n_rel": round(n_ / base_n * 100, 1), "p_rel": round(p_ / base_p * 100, 1)})
    figs.write_csv(DAT / "critical-length.csv", cr)
    print(cr)

    # the subsurface parameters
    sr = []
    for l_sub, e_sub in [(50, 0.8), (100, 0.8), (200, 0.8), (400, 0.8), (800, 0.8), (200, 0.5), (200, 0.95)]:
        ws_ = setup(f"sub{l_sub}_{e_sub}")
        run(ws_, subsurface_critical_length=l_sub, subsurface_efficiency=e_sub)
        sr.append({"l_sub": l_sub, "eff_sub": e_sub, "n_subsurface_load": round(total(ws_, "n_subsurface_load")),
                   "n_subsurface_export": round(total(ws_, "n_subsurface_export")),
                   "n_total_export": round(total(ws_, "n_total_export"))})
    figs.write_csv(DAT / "subsurface.csv", sr)
    print(sr)

    # D8
    wd = setup("d8")
    run(wd, method="d8")
    routing = [{"method": m, "n_surface": round(total(x, "n_surface_export")),
                "n_total": round(total(x, "n_total_export")), "p_surface": round(total(x, "p_surface_export"), 1),
                "ic_0": round(number(x, "ic0"), 2)} for m, x in (("mfd", w), ("d8", wd))]
    figs.write_csv(DAT / "routing.csv", routing)
    print(routing)

    # 2040
    wf = setup("future")
    run(wf, {"nutrients.n_total_export": "n.tif", "nutrients.p_surface_export": "p.tif",
             "nutrients.effective_retention_n": "eff_n.tif", "nutrients.ic": "ic.tif"}, lulc="lulc_2040.tif")
    fut_ic = es.grid(wf / "ic.tif")
    print("ic identical 2020/2040:", bool(np.array_equal(np.nan_to_num(fut_ic, nan=-999),
                                                         np.nan_to_num(g["ic"], nan=-999))))
    figs.raster(None, OUT / "n_change.png", array=es.grid(wf / "n.tif") - g["n_total_export"], diverging=True,
                cmap="PuOr_r", unit="N export, 2040 - 2020, kg / ha / yr", vmin=-5, vmax=5)
    for src, png in ((w / "effective_retention_n.tif", "eff_n_2020.png"), (wf / "eff_n.tif", "eff_n_2040.png")):
        figs.raster(src, OUT / png, cmap="Greens", vmin=0, vmax=0.8, legend=False, width=760)
    sc = []
    for col, label in (("n_surface_load", "Nitrogen, surface load"), ("n_subsurface_load", "Nitrogen, subsurface load"),
                       ("n_surface_export", "Nitrogen, surface export"),
                       ("n_subsurface_export", "Nitrogen, subsurface export"),
                       ("n_total_export", "Nitrogen, total export"), ("p_surface_load", "Phosphorus, load"),
                       ("p_surface_export", "Phosphorus, export")):
        a, b = total(w, col), total(wf, col)
        sc.append({"quantity": label + " (kg / yr)", "2020": round(a, 1), "2040": round(b, 1),
                   "change": f"{(b / a - 1) * 100:+.1f}%"})
    for x in ("n", "p"):
        def kept(d):
            if x == "n":
                return 1 - total(d, "n_total_export") / (total(d, "n_surface_load") + total(d, "n_subsurface_load"))
            return 1 - total(d, "p_surface_export") / total(d, "p_surface_load")
        sc.append({"quantity": ("Nitrogen" if x == "n" else "Phosphorus") + ", share of the load retained",
                   "2020": f"{kept(w) * 100:.1f}%", "2040": f"{kept(wf) * 100:.1f}%", "change": ""})
    figs.write_csv(DAT / "scenario.csv", sc)
    print(sc)

    # the table for the readers
    with zipfile.ZipFile(DAT / "ribeirao-claro-ndr.zip", "w", zipfile.ZIP_DEFLATED) as z:
        z.write(w / "biophysical_ndr.csv", "ribeirao-claro/biophysical_ndr.csv")


if __name__ == "__main__":
    main()
