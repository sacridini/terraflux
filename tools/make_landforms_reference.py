# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft 1.50's new surfaces and Dinamica functors give, for Terraflux's tests, written to
tests/data/reference/landforms.npz (and the cases to landforms.json):

- the geomorphons of every case of landschaft's r.geomorphon references (extern/landschaft/tests/data/reference/
  surface/geomorphons.*), with their measures;
- the hilltops of a cone, of the srtm DEM of those references (with a plain) and of a corner of orelevation
  (longitude / latitude);
- the gravitational potential of towns (cells and map units, a mask, the FFT);
- the Patcher of a continuous state (a step, and three of a run) and the road functors (cost map, destinations,
  pathways down the costs and the directions, steps of the road builder).

    python tools/make_landforms_reference.py

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
GEO = ROOT / "extern" / "landschaft" / "tests" / "data" / "reference" / "surface"
MEASURES = ("intensity", "exposition", "range", "variance", "elongation", "azimuth", "extend", "width")


def f64(x) -> np.ndarray:
    return np.asarray(getattr(x, "values", x), np.float64)


def main() -> None:
    out: dict[str, np.ndarray] = {}
    meta: dict = {"landschaft": ls.__version__, "geomorphons": {}, "hilltop": [], "gravity": [], "roads": {}}

    data = np.load(GEO / "geomorphons.npz")
    cases = json.loads((GEO / "geomorphons.json").read_text())["cases"]
    for name, c in cases.items():
        z = data[f"{name}/dem"]
        ny, nx = z.shape
        ew, ns = c["cell"]
        x0, y0 = c["origin"]
        settings = {"search": c["search"], "skip": c["skip"], "flat": c["flat"], "dist": c["dist"],
                    "units": "map" if c.get("meters") else "cells", "comparison": c["comparison"],
                    "extended": c.get("extended", False), "measures": True}
        g = ls.surface.geomorphons(z, extent=(x0, x0 + nx * ew, y0 - ny * ns, y0), **settings)
        meta["geomorphons"][name] = {"transform": [x0, ew, 0.0, y0, 0.0, -ns], "settings": settings}
        out[f"geo/{name}/dem"] = f64(z)
        for k in ("forms", "ternary", "positive", "negative", *MEASURES):
            out[f"geo/{name}/{k}"] = f64(getattr(g, k))
        out[f"geo/{name}/share"] = g.share().reset_index()[["form", "cells", "share"]].to_numpy(np.float64)

    # hilltops: a cone 300 m high in a plain, the srtm DEM with a plain below 20 % of its range, orelevation's corner
    yy, xx = np.mgrid[0:100, 0:100] * 10.0
    cone = 500 + np.clip(300 - 1.0 * np.hypot(yy - 500, xx - 500), 0, None)
    srtm = data["default/dem"]
    lo, hi = np.nanmin(srtm), np.nanmax(srtm)
    plain = (srtm < lo + 0.2 * (hi - lo)).astype(np.float64)
    hills = [
        ("cone", cone, [0.0, 10.0, 0.0, 1000.0, 0.0, -10.0], None, {}),
        ("srtm", srtm, [300000.0, 30.0, 0.0, 7400000.0, 0.0, -30.0], plain,
         {"min_height": 20.0, "min_slope": 5.0, "slope": "line", "fraction": 0.5}),
        ("srtm_mean", srtm, [300000.0, 30.0, 0.0, 7400000.0, 0.0, -30.0], None, {"min_height": 15.0, "min_slope": 3.0}),
    ]
    for name, z, t, pl, kw in hills:
        ny, nx = z.shape
        ext = (t[0], t[0] + nx * t[1], t[3] + ny * t[5], t[3])
        h = ls.surface.hilltop(z, extent=ext, plain=pl, **kw)
        meta["hilltop"].append({"name": name, "transform": t, "settings": kw, "plain": pl is not None})
        out[f"hill/{name}/dem"] = f64(z)
        if pl is not None:
            out[f"hill/{name}/plain"] = pl
        out[f"hill/{name}/app"] = f64(h.app)
        out[f"hill/{name}/hill"] = f64(h.hill)
        tab = h.hills.reset_index()
        tab["base_kind"] = tab["base_kind"].map({"saddle": 0, "plain": 1, "lowest": 2})
        tab["qualifies"] = tab["qualifies"].astype(float)
        out[f"hill/{name}/hills"] = tab.to_numpy(np.float64)
    z = ls.load_raster(ls.surface.example_data("orelevation"))[:60, :70]
    h = ls.surface.hilltop(z, min_height=40.0, min_slope=4.0)
    meta["hilltop"].append({"name": "lonlat", "settings": {"min_height": 40.0, "min_slope": 4.0}, "plain": False})
    out["hill/lonlat/dem"] = f64(z)
    out["hill/lonlat/app"] = f64(h.app)
    out["hill/lonlat/hill"] = f64(h.hill)
    tab = h.hills.reset_index()
    tab["base_kind"] = tab["base_kind"].map({"saddle": 0, "plain": 1, "lowest": 2})
    tab["qualifies"] = tab["qualifies"].astype(float)
    out["hill/lonlat/hills"] = tab.to_numpy(np.float64)

    # gravity: towns on a 30 m grid
    rng = np.random.default_rng(5)
    towns = np.full((40, 50), np.nan)
    idx = rng.choice(towns.size, 25, replace=False)
    towns.flat[idx] = rng.uniform(10, 1000, 25).round(1)
    mask = np.ones(towns.shape)
    mask[:5, :7] = np.nan
    t = [500000.0, 30.0, 0.0, 8000000.0, 0.0, -30.0]
    ext = (t[0], t[0] + 50 * 30.0, t[3] - 40 * 30.0, t[3])
    out["gravity/sources"] = towns
    out["gravity/mask"] = mask
    meta["gravity_transform"] = t
    for name, kw in [("cells", {}), ("map", {"unit": "map", "exponent": 1.5}), ("mask", {"exponent": 2.5}),
                     ("fft", {"method": "fft"})]:
        m = mask if name == "mask" else None
        out[f"gravity/{name}"] = f64(ls.surface.gravity(towns, extent=ext, mask=m, **kw))
        meta["gravity"].append({"name": name, "settings": kw, "mask": m is not None})

    # the Patcher of a continuous state: biomass per hectare on 30 m cells
    t = [600000.0, 30.0, 0.0, 9000000.0, 0.0, -30.0]
    ext = (t[0], t[0] + 60 * 30.0, t[3] - 50 * 30.0, t[3])
    biomass = rng.uniform(50, 300, (50, 60))
    biomass[rng.random((50, 60)) < 0.1] = np.nan
    prob = rng.random((50, 60))
    stock, p = da_of(biomass, t), da_of(prob, t)
    out["stock/biomass"], out["stock/probability"] = biomass, prob
    meta["stock_transform"] = t
    cp = ls.simulate.ContinuousPatcher(mean=1.5, variance=0.4, isometry=1.3)
    a = cp.allocate(stock, p, quota=40000.0, seed=11)
    for k, attr in (("stock", "stock"), ("probability", "probability"), ("taken", "taken"), ("patch", "patches")):
        out[f"stock/allocate/{k}"] = f64(getattr(a, attr))
    out["stock/allocate/info"] = np.array([40000.0, float(np.asarray(a.taken).sum()), a.remaining, a.n_patches,
                                           a.n_cells])
    cpc = ls.simulate.ContinuousPatcher(mean=6, variance=4, isometry=0.6, unit="cells", window=(3, 5))
    sim = cpc.run(biomass, prob, [500.0, 800.0, 300.0], seed=4)
    for k in range(1, 4):
        out[f"stock/run/{k}/stock"] = sim.stock.values[k]
        out[f"stock/run/{k}/taken"] = sim.taken.values[k - 1]
    out["stock/run/table"] = sim.steps.reset_index().to_numpy(np.float64)

    # roads: a network along the top row and a column, friction and attraction on 100 m cells
    t = [700000.0, 100.0, 0.0, 9100000.0, 0.0, -100.0]
    ny, nx = 60, 70
    ext = (t[0], t[0] + nx * 100.0, t[3] - ny * 100.0, t[3])
    net = np.full((ny, nx), np.nan)
    net[2, :] = 1
    net[:, 5] = 1
    friction = rng.uniform(1, 5, (ny, nx))
    friction[30:34, 20:50] = np.nan
    attraction = rng.random((ny, nx))
    net_da, fr_da, att_da = da_of(net, t), da_of(friction, t), da_of(attraction, t)
    meta["roads_transform"] = t
    out["roads/network"], out["roads/friction"], out["roads/attraction"] = net, friction, attraction
    cost, dirs = ls.simulate.friction_cost(net_da, fr_da, directions=True)
    out["roads/cost"], out["roads/direction"] = f64(cost), f64(dirs)
    cost_rel = ls.simulate.friction_cost(net_da, fr_da, diagonal=False, relative=True)
    out["roads/cost_relative"] = f64(cost_rel)
    dest = ls.simulate.destinations(att_da, (3, 4), per_quadrant=2, min_distance=800.0, seed=9)
    out["roads/destinations"] = f64(dest)
    out["roads/paths_cost"] = f64(ls.simulate.pathways(net_da, dest, cost, seed=3))
    out["roads/paths_dirs"] = f64(ls.simulate.pathways(net_da, dest, directions=dirs, merge=False, value=7.0))
    out["roads/paths_lottery"] = f64(ls.simulate.pathways(net_da, dest, cost, lottery=True, seed=21))
    rb = ls.simulate.RoadBuilder(fr_da, att_da, quadrants=(2, 3), distance=(800.0, 2500.0), spacing=600.0)
    first = rb.step(net_da, seed=17)
    out["roads/step"] = f64(first)
    runs = rb.run(net_da, 2, seed=8)
    out["roads/run/1"] = f64(runs.values[1])
    out["roads/run/2"] = f64(runs.values[2])
    meta["roads"] = {"builder": {"quadrants": [2, 3], "distance": [800.0, 2500.0], "spacing": 600.0}}

    np.savez_compressed(REF / "landforms.npz", **out)
    (REF / "landforms.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "landforms.npz", len(out))


def da_of(v, t):
    """A raster on the grid of a geotransform."""
    import rioxarray  # noqa: F401
    import xarray as xr
    from affine import Affine

    ny, nx = v.shape
    x = t[0] + (np.arange(nx) + 0.5) * t[1]
    y = t[3] + (np.arange(ny) + 0.5) * t[5]
    d = xr.DataArray(v, dims=("y", "x"), coords={"y": y, "x": x})
    return d.rio.write_transform(Affine(t[1], 0, t[0], 0, t[5], t[3]))


if __name__ == "__main__":
    main()
