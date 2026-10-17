"""Figures of the tutorial "Wetness, stream power and sediment connectivity" (terrain-hydrology/wetness-and-connectivity).

The SRTM elevation of south-western Oregon (UTM 10N, 30 m) filled; its topographic wetness, stream power and
sediment transport indices by the five accumulation methods; and the index of connectivity of InVEST's sediment
model with a cover weight from the summer NDVI, today and with the riparian forest cleared.

    python website/figures/terrain-hydrology/wetness-and-connectivity.py
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _hydro as H  # noqa: E402
from _hydro import figs, np, plt, mcolors  # noqa: E402

SLUG = "wetness-and-connectivity"
work, out, data = H.setup(SLUG)
numbers = {}

METHODS = ["d8", "dinf", "fd8", "quinn", "mfd"]
NAMES = {"d8": "D8", "dinf": "D-infinity", "fd8": "FD8", "quinn": "Quinn", "mfd": "MFD (InVEST)"}
FOREST_C, OPEN_C = 0.003, 0.05

H.make_filled(work)
MODEL = H.FILLED + H.NDVI + f'''
[[nodes]]
id = "spi"
op = "hydro.spi"
inputs = {{ dem = "filled.raster" }}

[[nodes]]
id = "sti"
op = "hydro.sti"
inputs = {{ dem = "filled.raster" }}

[[nodes]]
id = "twi_min"
op = "hydro.twi"
inputs = {{ dem = "filled.raster" }}
params = {{ min_slope = 0.001 }}

[[nodes]]
id = "sca_p1"
op = "hydro.flow_accumulation"
inputs = {{ dem = "filled.raster" }}
params = {{ method = "fd8", exponent = 1.0, units = "sca" }}

[[nodes]]
id = "twi_p1"
op = "hydro.twi"
inputs = {{ dem = "filled.raster", sca = "sca_p1.map" }}

[[nodes]]
id = "hand"
op = "hydro.hand"
inputs = {{ dem = "filled.raster" }}
params = {{ threshold = 500 }}

[[nodes]]
id = "cover"
op = "map.calc"
inputs = {{ n = "ndvi.raster" }}
params = {{ expression = "if n > 0.55 then {FOREST_C} else {OPEN_C}" }}

[[nodes]]
id = "ic"
op = "hydro.connectivity_index"
inputs = {{ dem = "filled.raster", weight = "cover.map" }}
params = {{ threshold = 500 }}

[[nodes]]
id = "cleared"
op = "map.calc"
inputs = {{ c = "cover.map", h = "hand.map" }}
params = {{ expression = "if h < 20 then {OPEN_C} else c" }}

[[nodes]]
id = "ic_cleared"
op = "hydro.connectivity_index"
inputs = {{ dem = "filled.raster", weight = "cleared.map" }}
params = {{ threshold = 500 }}

[[nodes]]
id = "ic_d8"
op = "hydro.connectivity_index"
inputs = {{ dem = "filled.raster", weight = "cover.map" }}
params = {{ threshold = 500, method = "d8" }}
'''
for m in METHODS:
    MODEL += f'''
[[nodes]]
id = "twi_{m}"
op = "hydro.twi"
inputs = {{ dem = "filled.raster" }}
params = {{ method = "{m}" }}
'''
for t in (100, 2500):
    MODEL += f'''
[[nodes]]
id = "ic_{t}"
op = "hydro.connectivity_index"
inputs = {{ dem = "filled.raster", weight = "cover.map" }}
params = {{ threshold = {t} }}
'''
writes = {"spi.map": "spi.tif", "sti.map": "sti.tif", "twi_min.map": "twi_min.tif", "twi_p1.map": "twi_p1.tif",
          "hand.map": "hand.tif", "cover.map": "cover.tif", "ic.map": "ic.tif", "cleared.map": "cleared.tif",
          "ic_cleared.map": "ic_cleared.tif", "ic_d8.map": "ic_d8.tif", "ic_100.map": "ic_100.tif",
          "ic_2500.map": "ic_2500.tif"}
for m in METHODS:
    writes[f"twi_{m}.map"] = f"twi_{m}.tif"
figs.run(figs.add_writes(MODEL, writes), work)


def rd(n):
    return H.rd(work, n)


z = rd("elevation_utm.tif")
shade = rd("shade.tif").filled(np.nan)
hand = rd("hand.tif")

# ------------------------------------------------------------------ the wetness index, by method
twi = {m: rd(f"twi_{m}.tif") for m in METHODS}
lo = float(np.percentile(np.concatenate([t.compressed() for t in twi.values()]), 1))
hi = float(np.percentile(np.concatenate([t.compressed() for t in twi.values()]), 99.5))
numbers["twi_range"] = (lo, hi)
frames = []
hf = hand.filled(np.nan)
low = hf < 5
for m in METHODS:
    t = twi[m]
    H.fig_map(out / f"twi_{m}.png", t, "YlGnBu", None, vmin=lo, vmax=hi, legend=False, width=520)
    tf = t.filled(np.nan)
    frames.append(dict(method=NAMES[m], mean=round(float(np.nanmean(tf)), 2), sd=round(float(np.nanstd(tf)), 2),
                       p10=round(float(np.nanpercentile(tf, 10)), 2), p90=round(float(np.nanpercentile(tf, 90)), 2),
                       nan=int(np.isnan(tf).sum() - z.mask.sum()),
                       low=round(float(np.nanmean(tf[low])), 2), rest=round(float(np.nanmean(tf[~low & np.isfinite(hf)])), 2)))
numbers["twi_frames"] = frames
# correlation between the methods' TWI and with the FD8 of exponent 1.0
f8 = twi["fd8"].filled(np.nan)
corr = {}
for m in METHODS + ["p1"]:
    other = (twi[m] if m != "p1" else rd("twi_p1.tif")).filled(np.nan)
    ok = np.isfinite(f8) & np.isfinite(other)
    corr[m] = float(np.corrcoef(f8[ok], other[ok])[0, 1])
numbers["twi_corr_fd8"] = corr
p1 = rd("twi_p1.tif").filled(np.nan)
numbers["twi_p1_vs_fd8_meandiff"] = float(np.nanmean(p1 - f8))
mn = rd("twi_min.tif").filled(np.nan)
numbers["twi_min_extra_cells"] = int(np.isfinite(mn).sum() - np.isfinite(f8).sum())
# TWI against HAND: a sample
rng = np.random.default_rng(2)
ok = np.argwhere(np.isfinite(f8) & np.isfinite(hf))
pick = ok[rng.choice(len(ok), 600, replace=False)]
figs.write_csv(data / "twi-hand.csv", sorted(({"HAND": round(float(hf[i, j]), 1), "TWI": round(float(f8[i, j]), 2)}
                                              for i, j in pick), key=lambda r: r["HAND"]))
both = np.isfinite(f8) & np.isfinite(hf)
numbers["corr_twi_hand"] = float(np.corrcoef(f8[both], hf[both])[0, 1])
# mean TWI by HAND class, for a small bar chart
bins = [(0, 5), (5, 20), (20, 50), (50, 100), (100, 200), (200, 500)]
rows = []
for a_, b_ in bins:
    sel = (hf >= a_) & (hf < b_) & np.isfinite(f8)
    rows.append({"HAND": f"{a_}–{b_} m", "Mean TWI (FD8)": round(float(np.mean(f8[sel])), 2),
                 "Mean TWI (D8)": round(float(np.nanmean(twi['d8'].filled(np.nan)[sel])), 2)})
figs.write_csv(data / "twi-by-hand.csv", rows)
numbers["twi_by_hand"] = rows

# ------------------------------------------------------------------ stream power and sediment transport
spi = rd("spi.tif")
sti = rd("sti.tif")
numbers["spi"] = H.stats(spi)
numbers["sti"] = H.stats(sti)
spos = np.ma.masked_less_equal(spi, 0)
H.fig_map(out / "spi.png", spos, "inferno", "Stream power index", unit="a · tan β, m (log scale)",
          norm=mcolors.LogNorm(vmin=float(np.percentile(spos.compressed(), 2)),
                               vmax=float(np.percentile(spos.compressed(), 99.5))), width=480)
H.fig_map(out / "sti.png", sti, "OrRd", "Sediment transport index (LS)", unit="LS factor", vmin=0,
          vmax=float(np.percentile(sti.compressed(), 98)), width=480)
H.fig_map(out / "twi-small.png", twi["fd8"], "YlGnBu", "Topographic wetness index", unit="ln(m)", vmin=lo, vmax=hi,
          width=480)

# ------------------------------------------------------------------ the index of connectivity
cover = rd("cover.tif")
numbers["forest_share"] = float((cover < 0.01).sum() / cover.count() * 100)
ic = rd("ic.tif")
icc = rd("ic_cleared.tif")
numbers["ic"] = H.stats(ic)
numbers["ic_cleared"] = H.stats(icc)
cl = rd("cleared.tif")
numbers["cleared_share"] = float((cl > 0.01).sum() / cl.count() * 100)
lo_ic = float(np.percentile(np.concatenate([ic.compressed(), icc.compressed()]), 1))
hi_ic = float(np.percentile(np.concatenate([ic.compressed(), icc.compressed()]), 99))
numbers["ic_range"] = (lo_ic, hi_ic)
H.fig_map(out / "ic.png", ic, "RdYlBu_r", "Index of connectivity", unit="IC (log10)", vmin=lo_ic, vmax=hi_ic,
          width=640)
for name, a in (("ic_now", ic), ("ic_cleared", icc)):
    H.fig_map(out / f"{name}.png", a, "RdYlBu_r", None, vmin=lo_ic, vmax=hi_ic, legend=False, width=560)
dif = (icc - ic)
numbers["ic_diff"] = H.stats(dif)
both = ~(ic.mask | icc.mask)
rip = both & (hf < 20)
numbers["ic_riparian"] = dict(now=float(ic[rip].mean()), cleared=float(icc[rip].mean()),
                              above_now=float(ic[both & (hf >= 20)].mean()),
                              above_cleared=float(icc[both & (hf >= 20)].mean()))
H.fig_map(out / "cover.png", np.ma.masked_where(cover.mask, np.where(cover.filled(1) < 0.01, 1, 2)), None,
          "Cover weight W (USLE C)", classes={1: (f"Forest, C = {FOREST_C}", "#3b8a5a"), 2: (f"Open, C = {OPEN_C}", "#d9a441")},
          width=480)
H.fig_map(out / "cleared.png", np.ma.masked_where(cl.mask, np.where(cl.filled(1) < 0.01, 1, 2)), None,
          "Riparian forest cleared", classes={1: (f"Forest, C = {FOREST_C}", "#3b8a5a"), 2: (f"Open, C = {OPEN_C}", "#d9a441")},
          width=480)
# the threshold of the streams and the routing
ics = []
for name, f in (("100 cells (9 ha)", "ic_100.tif"), ("500 cells (45 ha)", "ic.tif"), ("2,500 cells (225 ha)", "ic_2500.tif")):
    a = rd(f)
    ics.append(dict(name=name, mean=round(float(a.mean()), 2), p10=round(float(np.percentile(a.compressed(), 10)), 2),
                    p90=round(float(np.percentile(a.compressed(), 90)), 2), nan=int(a.mask.sum() - z.mask.sum())))
numbers["ic_thresholds"] = ics
d8 = rd("ic_d8.tif")
okk = ~(d8.mask | ic.mask)
numbers["ic_d8"] = dict(mean=float(d8.mean()), corr=float(np.corrcoef(d8[okk], ic[okk])[0, 1]))

(work / "numbers.json").write_text(json.dumps(numbers, indent=1, default=float))
print(json.dumps(numbers, indent=1, default=float))
