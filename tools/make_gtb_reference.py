# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft gives for the GuidosToolbox tools whose GTB outputs it keeps no digests of, for Terraflux's
tests: digests of the maps and the numbers of the reports, on the maps of tests/data/reference/gtb_inputs.npz
(tools/make_gtb_inputs.py), written to tests/data/reference/gtb_landschaft.json:

    python tools/make_gtb_reference.py

Run with the landschaft the submodule points at installed. Float maps are digested as float32, the cells
landschaft gives no value written NaN.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"


def digest(a) -> str:
    return hashlib.sha256(np.ascontiguousarray(np.asarray(a)).tobytes()).hexdigest()


def main() -> None:
    z = np.load(REF / "gtb_inputs.npz")
    forest, changed, small, gray = z["gtb_input"], z["gtb_input_changed"], z["blobs_101"], z["gscinput"]
    out: dict[str, list] = {}

    out["contagion"] = [{"input": "gtb_input", "window": w, "sha256": digest(ls.contagion_pff(forest, w))} for w in (7, 27)]
    out["clustering"] = [{"input": "gtb_input", "window": w, "sha256": digest(ls.clustering_fac(forest, w))}
                         for w in (7, 27)]
    e = ls.spatial_entropy(forest)
    out["entropy"] = [{"input": "gtb_input", "index": e.entropy_index, "histogram": [int(v) for v in e.histogram]}]
    out["entropy_map"] = [{"input": "blobs_101", "window": w,
                           "sha256": digest(np.asarray(ls.spatial_entropy_map(small, w), dtype=np.float32))}
                          for w in (5, 21)]
    out["gray_convolution"] = []
    for metric in ("mean", "median", "majority", "min", "max", "range", "stddev", "shannon_diversity",
                   "shannon_evenness", "diagonal_contagion"):
        for w, mask in ((7, True), (13, False)):
            g = np.asarray(ls.gray_spatial_convolution(gray, metric, w, mask_missing=mask), dtype=np.float32)
            out["gray_convolution"].append({"input": "gscinput", "metric": metric, "window": w, "mask_missing": mask,
                                            "sha256": digest(g)})
    out["landscape_mosaic_19"] = [{"input": n, "window": w, "sha256": digest(ls.landscape_mosaic(z[n], w, 19))}
                                  for n, w in (("random_a", 7), ("smooth_a", 27))]
    out["parcellation"] = []
    for name, conn in (("gtb_input", 8), ("gtb_input", 4), ("random_a", 8)):
        r = ls.parcellation(z[name], conn, return_labels=True)
        out["parcellation"].append({"input": name, "connectivity": conn, "sha256": digest(np.asarray(r.parcel_labels)),
                                    "global": {k: float(v) for k, v in r.global_stats.items()},
                                    "counts": [int(v) for v in r.stats["Count"]]})
    out["contortion"] = []
    for min_area in (1, 50):
        c = np.asarray(ls.contortion(forest, min_area))
        d = np.asarray(ls.contortion(forest, min_area, return_display=True))
        out["contortion"].append({"input": "gtb_input", "min_area": min_area, "sha256": digest(c.astype(np.uint32)),
                                  "display_sha256": digest(d)})
    d = ls.euclidean_distance(forest)
    signed = np.where(forest == 0, np.nan, np.asarray(d.signed_distance)).astype(np.float32)
    out["euclidean_distance"] = [{"input": "gtb_input", "sha256": digest(signed), "display_sha256": digest(d.byte_map),
                                  "stats": [d.fg_pixel_count, d.mean_fg_distance, d.max_fg_distance, d.bg_pixel_count,
                                            d.mean_bg_distance, d.max_bg_distance]}]
    s = ls.simple_change(forest, changed)
    out["simple_change"] = [{"before": "gtb_input", "after": "gtb_input_changed", "sha256": digest(s.change_map),
                             "counts": [s.stable_bg, s.gain, s.loss, s.stable_fg, s.missing, s.total_valid]}]
    m = ls.morphological_change(forest, changed)
    out["morphological_change"] = [{"before": "gtb_input", "after": "gtb_input_changed", "sha256": digest(m.change_map),
                                    "counts": [m.fg_area_t1, m.fg_area_t2, m.net_change, m.stable_bg, m.stable_fg,
                                               m.mcd_gain, m.mcd_loss, m.spurious, m.missing],
                                    "elasticity": m.elasticity}]
    fa, fb = ls.fos(forest, 27).map, ls.fos(changed, 27).map
    f = ls.fos_change(fa, fb)
    out["fos_change"] = [{"before": "gtb_input", "after": "gtb_input_changed", "window": 27,
                          "sha256": digest(f.change_map),
                          "counts": [f.fg_gain, f.fg_loss, f.stable_bg, f.forcom, f.diag_same_class, f.diff_class],
                          "percent": [f.frag_decrease_pct, f.frag_increase_pct],
                          "matrix": np.asarray(f.matrix_6x6).astype(int).tolist()}]
    path = REF / "gtb_landschaft.json"
    path.write_text(json.dumps({"landschaft": ls.__version__, **out}, indent=1) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
