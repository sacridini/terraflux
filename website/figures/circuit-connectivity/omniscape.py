"""Figures of "Omniscape: connectivity everywhere" (circuit connectivity, part 4): the cumulative current, the
flow potential and the normalised current of the CORINE landscape, and how they change with the window's radius."""

import pathlib
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1] / "habitat-networks"))
import _clc  # noqa: E402
from _clc import figs, np  # noqa: E402

SLUG = "circuit-connectivity/omniscape"
work = figs.workdir(SLUG)
out = figs.img_dir(SLUG)
data = figs.data_dir(SLUG)
_clc.setup(work)

RADII = [10, 20, 30, 50]
BLOCK = {10: 3, 20: 5, 30: 5, 50: 7}
RULES = _clc.RESISTANCE_RULES.replace("\n", "\\n")
MODEL = f"""format = "terraflux/1"

[[nodes]]
id = "land"
op = "raster.read"
params = {{ path = "clc_100m.tif" }}

[[nodes]]
id = "resistance"
op = "raster.reclassify"
inputs = {{ map = "land.map" }}
params = {{ rules = "{RULES}" }}
"""
writes = {}
for r in RADII:
    MODEL += f"""
[[nodes]]
id = "omniscape_{r}"
op = "circuit.omniscape"
inputs = {{ resistance = "resistance.map" }}
params = {{ radius = {r}, block_size = {BLOCK[r]}, flow_potential = true, normalized_current = true }}
"""
    writes[f"omniscape_{r}.normalized"] = f"normalized_{r}.tif"
    writes[f"omniscape_{r}.current"] = f"current_{r}.tif"
    writes[f"omniscape_{r}.flow_potential"] = f"flow_{r}.tif"
figs.run(figs.add_writes(MODEL, writes), work)

land = np.ma.filled(figs.read(work / "clc_100m.tif")[0], 0)
forest = np.isin(land, [23, 24, 25])
frames, rows = [], []
for r in RADII:
    n = figs.read(work / f"normalized_{r}.tif")[0]
    a = np.ma.filled(np.ma.clip(n, 0, 3).astype(float), np.nan)
    figs.raster(None, out / f"normalized_{r}.png", array=a, cmap="RdBu_r", diverging=False, vmin=0, vmax=3,
                legend=False, width=760, title=None) if False else None
    # centred on 1: a diverging ramp with 1 in the middle
    from matplotlib import colors as mc, pyplot as plt  # noqa: E402
    fig = plt.figure(figsize=(760 / 150, 760 / 150 * 1.0 + 0.45), dpi=150)
    ax = fig.add_axes([0, 0.1, 1, 0.9])
    ax.set_axis_off()
    im = ax.imshow(a, cmap="RdBu_r", norm=mc.TwoSlopeNorm(vmin=0, vcenter=1, vmax=3), interpolation="nearest")
    cax = fig.add_axes([0.2, 0.04, 0.6, 0.025])
    cb = fig.colorbar(im, cax=cax, orientation="horizontal")
    cb.outline.set_visible(False)
    cb.ax.tick_params(labelsize=8)
    cb.set_label("Normalised current (1: as without resistance)", fontsize=8.5, color=figs.INK2)
    png = out / f"normalized_{r}.png"
    fig.savefig(png, dpi=150)
    plt.close(fig)
    figs._shrink(png)
    v = n.compressed()
    ch = float((v > 1.5).mean() * 100)
    im_ = float((v < 0.5).mean() * 100)
    nf = n[~forest].compressed()
    frames.append({"value": f"{r / 10:g} km", "src": f"/img/tutorials/{SLUG}/normalized_{r}.png",
                   "stats": {"Radius": f"{r} cells", "Channelled (> 1.5)": f"{ch:.1f}%", "Impeded (< 0.5)": f"{im_:.1f}%",
                             "Median": f"{float(np.median(v)):.2f}"}})
    rows.append({"radius": f"{r / 10:g} km", "channelled": round(ch, 2), "impeded": round(im_, 2)})
    print(r, frames[-1]["stats"], "outside forest channelled", round(float((nf > 1.5).mean() * 100), 1))
figs.write_csv(data / "shares.csv", rows)

R = 30
cur = figs.read(work / f"current_{R}.tif")[0]
flow = figs.read(work / f"flow_{R}.tif")[0]
for name, a, unit in (("current", cur, "Cumulative current"), ("flow", flow, "Flow potential")):
    hi = float(np.percentile(a.compressed(), 99.5))
    lo = float(np.percentile(a.compressed(), 1))
    figs.raster(None, out / f"{name}_{R}.png", array=np.ma.filled(a.astype(float), np.nan), cmap="magma", log=True,
                vmin=max(lo, hi / 1e3), vmax=hi, unit=unit, width=640)
import json  # noqa: E402
(work / "frames.json").write_text(json.dumps(frames, indent=1), encoding="utf-8")
