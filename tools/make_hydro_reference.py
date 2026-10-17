# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's terrain hydrology gives, for Terraflux's tests: on the DEMs of landschaft's WhiteboxTools
references (extern/landschaft/tests/data/reference/hydro/whitebox.npz: oregon, holes, walk, terraces, grids
without a coordinate system from 0, 0) and on a corner of orelevation (longitude / latitude), every function of
ls.hydro with some options, written to tests/data/reference/hydro.npz (and the cases to hydro.json):

    python tools/make_hydro_reference.py

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
WHITEBOX = ROOT / "extern" / "landschaft" / "tests" / "data" / "reference" / "hydro" / "whitebox.npz"
H = ls.hydro

# (key, function, extra settings); the DEM is the filled one unless the function says "dem"
CASES = [
    ("fill", "fill", {}),
    ("fill_eps", "fill", {"drain_flats": True}),
    ("breach", "breach", {}),
    ("breach_len", "breach", {"max_length": 5, "min_dist": False, "flat_increment": 0.01, "fill_rest": False}),
    ("d8", "flow_direction", {}),
    ("d8_raw", "flow_direction", {"flats": False}),
    ("dinf", "flow_direction", {"method": "dinf"}),
    ("mfd", "flow_direction", {"method": "mfd"}),
    ("acc_d8", "flow_accumulation", {}),
    ("acc_d8_sca", "flow_accumulation", {"units": "sca"}),
    ("acc_dinf", "flow_accumulation", {"method": "dinf", "units": "area"}),
    ("acc_fd8", "flow_accumulation", {"method": "fd8"}),
    ("acc_quinn", "flow_accumulation", {"method": "quinn", "exponent": 1.5, "flats": False}),
    ("acc_mfd", "flow_accumulation", {"method": "mfd"}),
    ("acc_w", "flow_accumulation", {"method": "fd8", "weights": True}),
    ("hand", "hand", {"threshold": 20}),
    ("twi", "twi", {}),
    ("twi_min", "twi", {"min_slope": 0.001, "method": "d8"}),
    ("spi", "spi", {"method": "quinn"}),
    ("sti", "sti", {"sca_exponent": 0.5, "slope_exponent": 1.2}),
    ("ic", "connectivity_index", {"threshold": 30}),
    ("ic_d8", "connectivity_index", {"threshold": 30, "method": "d8"}),
]


def weights_of(shape):
    """A weight raster: 1 to 2 along the columns."""
    return np.tile(np.linspace(1.0, 2.0, shape[1]), (shape[0], 1))


def run(fn, dem, kw, extent):
    kw = dict(kw)
    if kw.pop("weights", False):
        kw["weights"] = weights_of(dem.shape)
    if fn == "connectivity_index":
        return H.connectivity_index(ls.surface._grid.read_surface(dem, extent=extent),
                                    weights_of(dem.shape) / 10.0, **kw)
    return getattr(H, fn)(dem, extent=extent, **kw)


def main() -> None:
    wb = np.load(WHITEBOX)
    out: dict[str, np.ndarray] = {}
    meta: dict = {"landschaft": ls.__version__, "dems": [], "cases": [[k, f, {**kw, "weights": bool(kw.get("weights"))}]
                                                                    for k, f, kw in CASES]}
    dems = {}
    for name in ("oregon", "holes", "walk", "terraces"):
        dem = wb[f"{name}/dem"].astype(np.float64)
        dx, dy = (float(v) for v in wb[f"{name}/cell"])
        dems[name] = (dem, (0.0, dem.shape[1] * dx, 0.0, dem.shape[0] * dy))
    z = ls.load_raster(ls.surface.example_data("orelevation"))[:50, :60]
    dems["lonlat"] = (z.astype(np.float64), None)   # its own transform and coordinate system
    for name, (dem, extent) in dems.items():
        meta["dems"].append({"name": name, "extent": list(map(float, extent or (0, 0, 0, 0))),
                             "lonlat": name == "lonlat"})
        out[f"{name}/dem"] = np.asarray(dem, np.float64)
        filled = H.fill(dem, extent=extent)
        if extent is not None:
            filled = filled.values
        for key, fn, kw in CASES:
            base = dem if fn in ("fill", "breach") else filled
            if name == "lonlat" and fn == "connectivity_index":
                continue
            out[f"{name}/{key}"] = np.asarray(run(fn, base, kw, extent).values, np.float64)
        # streams: by a threshold, and traced from the mfd accumulation
        acc = H.flow_accumulation(filled, method="mfd", extent=extent)
        out[f"{name}/streams"] = H.streams(H.flow_accumulation(filled, extent=extent), 20).values
        out[f"{name}/streams_trace"] = H.streams(acc, 25, trace=0.7, dem=filled).values
        rivers = H.streams(H.flow_accumulation(filled, extent=extent), 20)
        out[f"{name}/hand_streams"] = H.hand(filled, rivers.values, extent=extent).values
        out[f"{name}/distance"] = H.distance_to_channel(filled, rivers.values, extent=extent).values
        out[f"{name}/distance_w"] = H.distance_to_channel(filled, rivers.values, weights=weights_of(dem.shape),
                                                          extent=extent).values
        links, table = H.stream_links(filled, rivers.values, extent=extent)
        out[f"{name}/links"] = links.values
        out[f"{name}/links_table"] = table.reset_index().to_numpy(np.float64)
        out[f"{name}/watersheds"] = H.watersheds(filled, np.nan_to_num(links.values), extent=extent).values
        if name != "lonlat":
            # two outlets: the centres of two stream cells, moved a little
            r, c = np.nonzero(np.nan_to_num(rivers.values) == 1)
            pick = [len(r) // 3, 2 * len(r) // 3]
            ny = dem.shape[0]
            dx = (extent[1] - extent[0]) / dem.shape[1]
            dy = (extent[3] - extent[2]) / ny
            pts = [(extent[0] + (c[k] + 0.7) * dx, extent[3] - (r[k] + 0.4) * dy) for k in pick]
            links, table = H.stream_links(filled, rivers.values, outlets=pts, snap=2 * dx, extent=extent)
            out[f"{name}/links_outlets"] = links.values
            out[f"{name}/links_outlets_table"] = table.reset_index().to_numpy(np.float64)
            meta["dems"][-1]["outlets"] = [list(map(float, p)) for p in pts]
            meta["dems"][-1]["snap"] = float(2 * dx)
    meta["links_columns"] = ["link", *table.columns]
    np.savez_compressed(REF / "hydro.npz", **out)
    (REF / "hydro.json").write_text(json.dumps(meta, indent=1) + "\n", encoding="utf-8")
    print(REF / "hydro.npz", len(out))


if __name__ == "__main__":
    main()
