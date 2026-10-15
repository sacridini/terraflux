"""Figures of the tutorial "PLUS: land expansion and random patch seeds" (land-change/plus-cars).

PLUS on Plum Island Ecosystems (lulcc's example: 1 forest, 2 built, 3 other; 1985, 1991, 1999):

1. LEAS, a random forest per class on where it expanded between 1985 and 1991 (landschaft's Estimator, as
   presets.plus does: 50 trees, 5 % of the cells drawn from the expansions and the others separately), its
   growth probabilities written to potential.tif (a band per class) and the importance of each driver;
2. CARS in Terraflux: a Markov chain of 1985-1991 (6 yearly steps) projected from 1991 for 8 years
   (plus.demand), cars.step in a loop, compared with the observed map of 1999;
3. the same with other patch generation thresholds (seeds).

    python website/figures/land-change/plus_pie.py
"""

import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[2] / "tools"))
import figs  # noqa: E402
import numpy as np  # noqa: E402

SLUG = "land-change/plus-cars"
INPUTS = figs.ROOT / "tests" / "data" / "inputs"
PAL = {1: ("Forest", "#3b8a5a"), 2: ("Built", "#b4508a"), 3: ("Other", "#d9a441")}
# a window of the map to show the patterns (rows, columns)
WIN = (slice(170, 370), slice(150, 400))


def leas(work: pathlib.Path):
    """LEAS with landschaft: the growth probability of each class, from where it expanded in 1985-1991."""
    import landschaft as ls
    import rasterio
    from sklearn.ensemble import RandomForestClassifier

    S = ls.simulate
    lu = ls.load_raster(str(INPUTS / "pie_landuse.tif"))
    ef = ls.load_raster(str(INPUTS / "pie_drivers.tif"))
    t0 = lu[0].where(lu[0] != 255)
    t1 = lu[1].where(lu[1] != 255)
    est = S.Estimator(RandomForestClassifier(n_estimators=50, min_samples_leaf=50, random_state=1, n_jobs=1), target="expansion",
                      classes=[1, 2, 3], sample=0.05, stratify=True, seed=1)
    est.fit(t0, ef, t1=t1)
    p = est.predict(ef, landscape=t1)
    imp = est.importance()
    with rasterio.open(INPUTS / "pie_landuse.tif") as src:
        prof = src.profile
    prof.update(count=3, dtype="float32", nodata=float("nan"))
    with rasterio.open(work / "potential.tif", "w", **prof) as dst:
        for i, c in enumerate(p[p.dims[0]].values):
            dst.write(np.asarray(p.values[i], dtype="float32"), i + 1)
            dst.set_band_description(i + 1, str(int(c)))
    return imp


def model(seeds: float = 0.1, window: int = 3) -> str:
    return f"""format = "terraflux/1"
name = "PLUS on Plum Island"

[[nodes]]
id = "lu1985"
op = "raster.read"
params = {{ path = "pie_landuse.tif", band = "1985" }}

[[nodes]]
id = "lu1991"
op = "raster.read"
params = {{ path = "pie_landuse.tif", band = "1991" }}

[[nodes]]
id = "leas"
op = "raster.read"
params = {{ path = "potential.tif" }}

[[nodes]]
id = "growth"
op = "stack.classes"
inputs = {{ stack = "leas.stack" }}

[[nodes]]
id = "markov"
op = "markov.fit"
inputs = {{ t0 = "lu1985.map", t1 = "lu1991.map", steps = 6 }}

[[nodes]]
id = "demand"
op = "plus.demand"
inputs = {{ start = "lu1991.map", markov = "markov.markov" }}
params = {{ steps = 8 }}

[[nodes]]
id = "simulation"
op = "control.repeat"
inputs = {{ times = "demand.steps", landscape = "lu1991.map" }}

[nodes.body]
carry = {{ landscape = "cars.landscape" }}
collect = {{ maps = "cars.landscape", info = "cars.info" }}

[[nodes.body.nodes]]
id = "cars"
op = "cars.step"
inputs = {{ landscape = "@landscape", probability = "growth.stack", demand = "demand.demand", step = "@step", seed = 1 }}
params = {{ seeds = {seeds}, weights = {{ 1 = 0.5, 2 = 1.0, 3 = 0.3 }}, decay = 0.9, step_size = 50, window = {window}, max_iter = 5000 }}

[[nodes]]
id = "lu1999"
op = "raster.read"
params = {{ path = "pie_landuse.tif", band = "1999" }}

[[nodes]]
id = "validation"
op = "validate.three_map"
inputs = {{ t0 = "lu1991.map", t1 = "lu1999.map", simulated = "simulation.landscape" }}
params = {{ factors = [1, 2, 4, 8, 16] }}

[outputs.maps]
from = "simulation.maps"

[outputs.steps]
from = "simulation.info"

[outputs.fom]
from = "validation.figure_of_merit"
"""


def read(path, band=1):
    import rasterio

    with rasterio.open(path) as d:
        a = d.read(band).astype(float)
        if d.nodata is not None and not np.isnan(d.nodata):
            a[a == d.nodata] = np.nan
    return a


def patches(a, cls):
    from scipy import ndimage

    lab, n = ndimage.label(a == cls, structure=np.ones((3, 3)))
    return n


def main():
    img = figs.img_dir(SLUG)
    data = figs.data_dir(SLUG)
    work = figs.workdir(f"{SLUG}/base")
    shutil.copy(INPUTS / "pie_landuse.tif", work / "pie_landuse.tif")
    shutil.copy(INPUTS / "pie_drivers.tif", work / "pie_drivers.tif")
    imp = leas(work)
    print(imp)
    rows = []
    for drv in imp.index:
        rows.append({"driver": drv, **{PAL[int(c)][0]: round(float(imp.loc[drv, c]), 4) for c in imp.columns}})
    figs.write_csv(data / "importance.csv", rows)
    # the growth probabilities
    for b, c in enumerate((1, 2, 3), start=1):
        g = read(work / "potential.tif", b)
        figs.raster(work / "potential.tif", img / f"growth_{c}.png", band=b, cmap="viridis", vmin=0,
                    vmax=round(float(np.nanpercentile(g, 99.5)), 2),
                    title=f"Growth probability of {PAL[c][0].lower()} (LEAS)", width=700)
    # observed
    obs = {y: read(INPUTS / "pie_landuse.tif", b) for b, y in ((1, 1985), (2, 1991), (3, 1999))}
    for y, a in obs.items():
        figs.raster(None, img / f"observed_{y}.png", array=np.ma.masked_invalid(a), palette=PAL,
                    title=f"Observed {y}", width=760, counts=True)
    # the base run
    figs.run(model(0.1), work)
    maps = work / "output" / "maps.tif"
    steps = figs.read_csv(work / "output" / "steps.csv")
    print(steps[-1])
    figs.write_csv(data / "steps.csv", steps)
    n = figs.bands(maps)
    sim = read(maps, n)
    figs.raster(None, img / "simulated_1999.png", array=np.ma.masked_invalid(sim), palette=PAL,
                title="Simulated 1999", width=760, counts=True)
    # compare (same framing, no legend)
    figs.raster(None, img / "cmp_observed.png", array=np.ma.masked_invalid(obs[1999][WIN]), palette=PAL,
                legend=False, width=760)
    figs.raster(None, img / "cmp_simulated.png", array=np.ma.masked_invalid(sim[WIN]), palette=PAL, legend=False,
                width=760)
    # the cells of each class at each step (they meet the Markov demand)
    areas = [{"year": 1991, **{PAL[c][0]: int(np.nansum(obs[1991] == c)) for c in PAL}}]
    for r in steps:
        areas.append({"year": 1991 + int(float(r["step"])), **{PAL[c][0]: int(float(r[f"cells_{c}"])) for c in PAL}})
    figs.write_csv(data / "areas.csv", areas)
    # frames of the simulation, the window
    figs.raster(None, img / "step_0.png", array=np.ma.masked_invalid(obs[1991][WIN]), palette=PAL,
                legend=False, width=700)
    for b in range(1, n + 1):
        a = read(maps, b)
        figs.raster(None, img / f"step_{b}.png", array=np.ma.masked_invalid(a[WIN]), palette=PAL,
                    legend=False, width=700)
    # hits and misses of the change 1991-1999
    o91, o99 = obs[1991], obs[1999]
    ch = np.full(o91.shape, np.nan)
    ok = ~np.isnan(o91) & ~np.isnan(o99) & ~np.isnan(sim)
    ch[ok] = 0
    obs_ch = ok & (o91 != o99)
    sim_ch = ok & (o91 != sim)
    ch[obs_ch & ~sim_ch] = 1  # misses
    ch[obs_ch & sim_ch & (o99 == sim)] = 2  # hits
    ch[obs_ch & sim_ch & (o99 != sim)] = 3  # wrong hits
    ch[~obs_ch & sim_ch] = 4  # false alarms
    print(open(work / "output" / "fom.csv").read())
    shutil.copy(work / "output" / "fom.csv", data / "fom.csv")
    figs.raster(None, img / "hits.png", array=np.ma.masked_invalid(ch[WIN]),
                colors={0: "#ebe7dc", 1: "#e69f00", 2: "#009e73", 3: "#785ef0", 4: "#d55e00"},
                labels={0: "Persistence", 1: "Miss", 2: "Hit", 3: "Wrong hit", 4: "False alarm"},
                title="Change 1991-1999", width=760)
    print("observed built patches", {y: patches(a, 2) for y, a in obs.items()})

    def variant(name, **kw):
        w = figs.workdir(f"{SLUG}/{name}")
        for f in ("pie_landuse.tif", "potential.tif"):
            shutil.copy(work / f, w / f)
        figs.run(model(**kw), w)
        m = w / "output" / "maps.tif"
        a = read(m, figs.bands(m))
        f1 = float(figs.read_csv(w / "output" / "fom.csv")[0]["overall"])
        return a, f1

    # the neighbourhood's side
    sens = []
    for win in (3, 5, 7, 9):
        a, f1 = variant(f"window_{win}", window=win)
        figs.raster(None, img / f"window_{win}.png", array=np.ma.masked_invalid(a[WIN]), palette=PAL,
                    width=700, legend=False)
        sens.append({"window": win, "built_patches": patches(a, 2), "fom": round(f1, 4)})
    print(sens)
    figs.write_csv(data / "window.csv", sens)
    # the patch generation threshold
    sens = []
    for sd in (0.0, 0.1, 0.5, 1.0):
        a, f1 = variant(f"seeds_{sd}", seeds=sd)
        sens.append({"seeds": sd, "built_patches": patches(a, 2), "fom": round(f1, 4)})
    print(sens)
    figs.write_csv(data / "seeds.csv", sens)


if __name__ == "__main__":
    main()
