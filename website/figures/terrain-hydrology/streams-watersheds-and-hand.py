"""Figures of the tutorial "Streams, watersheds and HAND" (terrain-hydrology/streams-watersheds-and-hand).

The SRTM elevation of south-western Oregon (UTM 10N, 30 m) filled, its D8 accumulation turned into streams at
several thresholds, the streams split into links with their table (Strahler order, lengths), an outlet point
snapped to the streams and its watershed, the subbasins of every link, the height above the nearest drainage
(HAND) and the distance to the streams.

    python website/figures/terrain-hydrology/streams-watersheds-and-hand.py
"""

import json
import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import _hydro as H  # noqa: E402
from _hydro import figs, np, plt, mcolors  # noqa: E402

SLUG = "streams-watersheds-and-hand"
work, out, data = H.setup(SLUG)
numbers = {}
MAP_KM2 = 369 * 270 * 900 / 1e6

H.make_filled(work)
BASE = H.FILLED + '''
[[nodes]]
id = "accumulation"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.raster" }
'''

# ------------------------------------------------------------------ streams and links at several thresholds
THRESHOLDS = [100, 250, 500, 1000, 2500, 5000]
m = BASE
w = {"accumulation.map": "acc.tif"}
for t in THRESHOLDS:
    m += f'''
[[nodes]]
id = "streams_{t}"
op = "hydro.streams"
inputs = {{ accumulation = "accumulation.map" }}
params = {{ threshold = {t} }}

[[nodes]]
id = "links_{t}"
op = "hydro.stream_links"
inputs = {{ dem = "filled.raster", streams = "streams_{t}.map" }}

[[nodes]]
id = "hand_{t}"
op = "hydro.hand"
inputs = {{ dem = "filled.raster", streams = "streams_{t}.map" }}
'''
    w[f"streams_{t}.map"] = f"streams_{t}.tif"
    w[f"links_{t}.map"] = f"links_{t}.tif"
    w[f"links_{t}.table"] = f"links_{t}.csv"
    w[f"hand_{t}.map"] = f"hand_{t}.tif"
# InVEST's traced streams from the mfd accumulation
m += '''
[[nodes]]
id = "acc_mfd"
op = "hydro.flow_accumulation"
inputs = { dem = "filled.raster" }
params = { method = "mfd" }

[[nodes]]
id = "traced"
op = "hydro.streams"
inputs = { accumulation = "acc_mfd.map", dem = "filled.raster" }
params = { threshold = 500, trace = 0.7 }

[[nodes]]
id = "mfd_plain"
op = "hydro.streams"
inputs = { accumulation = "acc_mfd.map" }
params = { threshold = 500 }

[[nodes]]
id = "distance"
op = "hydro.distance_to_channel"
inputs = { dem = "filled.raster", streams = "streams_500.map" }

[[nodes]]
id = "subbasins"
op = "hydro.watersheds"
inputs = { dem = "filled.raster", seeds = "links_500.map" }
'''
w.update({"traced.map": "traced.tif", "mfd_plain.map": "mfd_plain.tif", "distance.map": "distance.tif",
          "subbasins.map": "subbasins.tif"})
figs.run(figs.add_writes(m, w), work)


def rd(n):
    return H.rd(work, n)


z = rd("elevation_utm.tif")
shade = rd("shade.tif").filled(np.nan)
acc = rd("acc.tif")

ORDER = {1: ("Order 1", "#9ecae1"), 2: ("Order 2", "#4a98c9"), 3: ("Order 3", "#1d6fb0"), 4: ("Order 4", "#08458a"),
         5: ("Order 5", "#e8901f"), 6: ("Order 6", "#b5452c")}


def order_map(links_tif, table):
    lk = rd(links_tif).filled(0).astype(int)
    lut = np.zeros(int(lk.max()) + 1)
    for r in table:
        lut[int(r["link"])] = r["order"]
    o = lut[lk]
    return np.ma.masked_where(o == 0, o)


def widen(a, size=2):
    """Lines drawn wider for small figures: the largest value around each cell."""
    from scipy import ndimage
    f = ndimage.grey_dilation(np.ma.filled(a, 0), size=(size, size))
    return np.ma.masked_where(f == 0, f)


frames = []
summary = []
for t in THRESHOLDS:
    tab = H.table(work, f"links_{t}")
    st = rd(f"streams_{t}.tif")
    km = sum(r["length"] for r in tab) / 1000
    orders = [int(r["order"]) for r in tab]
    summary.append(dict(threshold=t, area_ha=t * 0.09, cells=int(st.sum()), links=len(tab), km=round(km, 1),
                        density=round(km / MAP_KM2, 2), max_order=max(orders)))
    om = order_map(f"links_{t}.tif", tab)
    H.fig_map(out / f"streams_{t}.png", widen(om), None, None, classes={k: v for k, v in ORDER.items()},
              under=shade, alpha=1.0, legend=False, width=520)
numbers["thresholds"] = summary
figs.write_csv(data / "density.csv", [{"threshold_ha": s["area_ha"], "Drainage density (km/km²)": s["density"],
                                      "Links": s["links"]} for s in summary])
H.legend_strip(out / "order-legend.png", {k: ORDER[k] for k in range(1, max(x["max_order"] for x in summary) + 1)})

# the links of the 500-cell network: the order map, the table by order
tab = H.table(work, "links_500")
om = order_map("links_500.tif", tab)
present = sorted(set(int(r["order"]) for r in tab))
H.fig_map(out / "orders.png", widen(om), None, "Stream links by Strahler order",
          classes={k: ORDER[k] for k in present}, under=shade, counts=False, width=640)
by = []
for o in sorted(set(int(r["order"]) for r in tab)):
    rows = [r for r in tab if int(r["order"]) == o]
    L = [r["length"] for r in rows]
    drop = [r["z_start"] - r["z_end"] for r in rows]
    by.append({"order": o, "links": len(rows), "length_km": round(sum(L) / 1000, 2),
               "mean_length_m": round(float(np.mean(L)), 0),
               "mean_gradient_pct": round(float(np.sum(drop) / np.sum(L) * 100), 2)})
figs.write_csv(data / "orders.csv", by)
numbers["by_order"] = by
# a few rows of the table as Terraflux writes it
figs.write_csv(data / "links-head.csv", [{k: (round(v, 1) if isinstance(v, float) and k in ("length", "z_start", "z_end")
                                               else int(v) if isinstance(v, float) else v) for k, v in r.items()}
                                         for r in tab[:6]])
numbers["links_500"] = dict(n=len(tab), sources=sum(1 for r in tab if r["order"] == 1),
                            edge=sum(1 for r in tab if r["downstream"] == 0))

# ------------------------------------------------------------------ an outlet point, snapped, and its watershed
# a gauge on a tributary: a stream cell of 3,000 to 8,000 cells away from the edge, and the point 2 cells off it
st5 = rd("streams_500.tif").filled(0)
a = acc.filled(0)
cand = np.argwhere((st5 == 1) & (a >= 3000) & (a <= 8000))
cand = [tuple(c) for c in cand if 60 <= c[0] <= 300 and 40 <= c[1] <= 230]
gr, gc = cand[len(cand) // 2]
import rasterio  # noqa: E402
from rasterio.warp import transform as wtransform  # noqa: E402

with rasterio.open(work / "elevation_utm.tif") as ds:
    tr = ds.transform
x = tr.c + (gc + 0.5) * tr.a + 45.0          # 45 m east and 40 m north of the stream cell's centre
y = tr.f + (gr + 0.5) * tr.e + 40.0
lon, lat = wtransform("EPSG:32610", "EPSG:4326", [x], [y])
gauge = {"type": "FeatureCollection", "features": [{"type": "Feature", "properties": {"name": "gauge"},
                                                     "geometry": {"type": "Point", "coordinates": [round(lon[0], 6), round(lat[0], 6)]}}]}
(work / "outlets.geojson").write_text(json.dumps(gauge))
numbers["gauge"] = dict(row=int(gr), col=int(gc), acc=float(a[gr, gc]), x=x, y=y, lon=lon[0], lat=lat[0])

OUT = BASE + '''
[[nodes]]
id = "streams"
op = "hydro.streams"
inputs = { accumulation = "accumulation.map" }
params = { threshold = 500 }

[[nodes]]
id = "outlets"
op = "vector.read"
params = { path = "outlets.geojson" }

[[nodes]]
id = "links"
op = "hydro.stream_links"
inputs = { dem = "filled.raster", streams = "streams.map", outlets = "outlets.vector" }

[[nodes]]
id = "unsnapped"
op = "vector.rasterize"
inputs = { vector = "outlets.vector", like = "filled.raster" }

[[nodes]]
id = "ws_unsnapped"
op = "hydro.watersheds"
inputs = { dem = "filled.raster", seeds = "unsnapped.map" }
'''
figs.run(figs.add_writes(OUT, {"links.map": "olinks.tif", "links.table": "olinks.csv",
                               "ws_unsnapped.map": "ws_unsnapped.tif"}), work, name="outlet.toml")
otab = H.table(work, "olinks")
K = int(next(r["link"] for r in otab if r["outlet"] == 1))
krow = next(r for r in otab if int(r["link"]) == K)
numbers["outlet_link"] = dict(K=K, row=krow, n_links=len(otab))
WS = OUT + f'''
[[nodes]]
id = "seed"
op = "raster.calc"
inputs = {{ l = "links.map" }}
params = {{ expression = "if l == {K} then 1 else 0" }}

[[nodes]]
id = "watershed"
op = "hydro.watersheds"
inputs = {{ dem = "filled.raster", seeds = "seed.map" }}
'''
figs.run(figs.add_writes(WS, {"watershed.map": "watershed.tif", "seed.map": "seed.tif"}), work, name="ws.toml")
ws = rd("watershed.tif")
wsu = rd("ws_unsnapped.tif")
numbers["watershed"] = dict(cells=int((ws == 1).sum()), km2=float((ws == 1).sum() * 900 / 1e6),
                            unsnapped_cells=int((wsu == 1).sum()))
end_r, end_c = int(krow["end_row"]), int(krow["end_col"])
numbers["snap_move_m"] = float(np.hypot((end_c + 0.5) * 30 + tr.c - x, tr.f + (end_r + 0.5) * tr.e - y))
# drawn: the watershed, the network, the point and the snapped outlet, cropped around the watershed
rows_, cols_ = np.where(ws.filled(0) == 1)
pad = 12
crop = (max(rows_.min() - pad, 0), min(rows_.max() + pad, z.shape[0]), max(cols_.min() - pad, 0),
        min(cols_.max() + pad, z.shape[1]))
numbers["crop"] = crop
show = np.ma.masked_all(z.shape)
show[ws.filled(0) == 1] = 1
lk = rd("olinks.tif").filled(0)
show[(lk > 0)] = 2
show[(lk == K)] = 3
WSC = {1: ("Watershed of the gauge", "#a7c957"), 2: ("Streams", "#2f6f9f"), 3: (f"Link {K}, ending at the gauge", "#e8901f")}
gx = (x - tr.c) / tr.a - 0.5 - crop[2]
gy = (y - tr.f) / tr.e - 0.5 - crop[0]


def mark(ax):
    ax.plot([gx], [gy], marker="o", ms=8, mfc="white", mec=figs.INK, mew=1.5, zorder=5)
    ax.plot([end_c - crop[2]], [end_r - crop[0]], marker="s", ms=6, mfc=figs.EMBER, mec=figs.INK, mew=1, zorder=5)
    ax.annotate("the point", (gx, gy), xytext=(gx + 4, gy - 4), fontsize=9, color=figs.INK,
                arrowprops=dict(arrowstyle="-", color=figs.INK2, lw=0.8))


H.fig_map(out / "watershed.png", show, None, "The watershed of a gauge", classes=WSC, under=shade, alpha=0.8,
          counts=False, extra=mark, crop=crop, width=700)

# the subbasins of every link (each cell labelled with the link it first drains to)
sb = rd("subbasins.tif")
lab = sb.filled(0).astype(int)
rng = np.random.default_rng(3)
ids = [int(k) for k in np.unique(lab) if k > 0]
SB = {k: (str(k), figs.CLASSES[int(rng.integers(0, 10))]) for k in ids}
for i, k in enumerate(ids):                 # neighbours in raster order get different colours
    SB[k] = (str(k), figs.CLASSES[(i * 7 + int(rng.integers(0, 3))) % 10])
numbers["subbasins"] = dict(n=int(len(np.unique(lab[lab > 0]))), unlabelled=int((lab == 0).sum()))


def links_over(ax):
    ll = rd("links_500.tif").filled(0)
    ov = np.zeros(ll.shape + (4,))
    ov[ll > 0] = mcolors.to_rgba(figs.INK)
    ax.imshow(ov, interpolation="nearest")


fig_rgba = np.ma.masked_array(lab, mask=lab == 0)
H.fig_map(out / "subbasins.png", fig_rgba, None, "Subbasins: the area draining to each link", classes=SB,
          under=shade, alpha=0.6, legend=False, extra=links_over, width=560)

# traced (InVEST) streams against the D8 ones, the same threshold
st_d8 = rd("streams_500.tif")
st_tr = rd("traced.tif")
st_plain = rd("mfd_plain.tif")
numbers["traced"] = dict(d8=int(st_d8.sum()), traced=int(st_tr.sum()), mfd_plain=int(st_plain.sum()))
for name, s in (("streams_d8", st_d8), ("streams_traced", st_tr)):
    H.fig_map(out / f"{name}.png", widen(np.ma.masked_where(s.filled(0) == 0, s.filled(0))), None, None,
              classes={1: ("Stream", figs.WATER)}, under=shade, legend=False, width=520)

# ------------------------------------------------------------------ HAND and the distance to the streams
hand = rd("hand_500.tif")
numbers["hand"] = H.stats(hand)
H.fig_map(out / "hand.png", hand, "cividis", "HAND (m)", unit="metres above the nearest stream", under=shade,
          alpha=0.85, vmin=0, vmax=300, width=640)
dist = rd("distance.tif")
numbers["distance"] = H.stats(dist)
H.fig_map(out / "distance.png", dist * 30, "magma_r", "Distance to the streams (m)",
          unit="metres along the flow (30 m per cell length)", under=shade, alpha=0.85, vmin=0, vmax=1200, width=640)
# HAND below a few heights, for the slider
LOW = [2, 5, 10, 20, 50]
hf = hand.filled(np.nan)
lows = []
for h in LOW:
    sel = np.ma.masked_where(~(hf < h), np.ones_like(hf))
    H.fig_map(out / f"hand_below_{h}.png", sel, None, None, classes={1: (f"HAND < {h} m", "#2f6f9f")}, under=shade,
              alpha=0.85, legend=False, width=520)
    lows.append(dict(h=h, share=round(float(np.nansum(hf < h)) / np.isfinite(hf).sum() * 100, 1)))
numbers["hand_low"] = lows
# HAND against elevation, and HAND for three stream thresholds
rng = np.random.default_rng(1)
ok = np.argwhere(np.isfinite(hf) & ~z.mask)
pick = ok[rng.choice(len(ok), 600, replace=False)]
sample = sorted(({"elevation": round(float(z.data[i, j]), 1), "HAND": round(float(hf[i, j]), 1)} for i, j in pick),
                key=lambda r: r["elevation"])
figs.write_csv(data / "hand-elevation.csv", sample)
okm = np.isfinite(hf) & ~z.mask
numbers["corr_hand_z"] = float(np.corrcoef(hf[okm], z.data[okm])[0, 1])
hs = []
for t in THRESHOLDS:
    hh = rd(f"hand_{t}.tif")
    v = hh.compressed()
    hs.append(dict(t=t, ha=t * 0.09, median=round(float(np.median(v)), 1), p90=round(float(np.percentile(v, 90)), 1),
                   below5=round(float((v < 5).sum()) / v.size * 100, 1)))
    H.fig_map(out / f"hand_{t}.png", hh, "cividis", None, under=shade, alpha=0.85, vmin=0, vmax=300, legend=False,
              width=520)
numbers["hand_by_threshold"] = hs
# HAND against the distance to the streams
okd = np.isfinite(hf) & ~dist.mask
numbers["corr_hand_dist"] = float(np.corrcoef(hf[okd], dist.data[okd])[0, 1])

(work / "numbers.json").write_text(json.dumps(numbers, indent=1, default=float))
print(json.dumps(numbers, indent=1, default=float))
