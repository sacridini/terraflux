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
    c = ls.fad_multiscale_change(forest, changed, window_sizes=(7, 13, 27))
    out["fad_change"] = [{"before": "gtb_input", "after": "gtb_input_changed", "windows": [7, 13, 27],
                          "scales": [{"matrix": np.asarray(sc.matrix).astype(int).tolist(), "fg_a": sc.fg_area_a,
                                      "fg_b": sc.fg_area_b, "forcom": sc.forcom, "same": sc.same_class,
                                      "changed": sc.changed, "decrease": sc.frag_decrease_pct,
                                      "increase": sc.frag_increase_pct} for sc in c.scales.values()],
                          "averages": c.averages.to_numpy(np.float64).tolist(), "report": c.summary("mscale")}]
    out["mosaic_heatmap"] = []
    for n, w in (("random_a", 7), ("smooth_a", 27)):
        h = ls.landscape_mosaic_heatmap(z[n], w)
        out["mosaic_heatmap"].append({"input": n, "window": w, "values": [float(v) for v in h.values],
                                      "csv": h.csv_text()})
    c = ls.landscape_mosaic_change(z["random_a"], z["smooth_a"], 7)
    out["mosaic_change"] = [{"before": "random_a", "after": "smooth_a", "window": 7,
                             "values": [float(v) for v in c.values], "csv": c.csv_text()}]
    out["influence_zones"] = []
    for min_area, core, buffer in ((50, 0, 0), (30, 2, 3)):
        r = ls.influence_zones(z["blobs_a"], min_area, core_buffer=core, buffer=buffer)
        out["influence_zones"].append({"input": "blobs_a", "min_area": min_area, "core_buffer": core, "buffer": buffer,
                                       "classes": digest(np.asarray(r.classes)),
                                       "ids": digest(np.asarray(r.ids).astype(np.int32)),
                                       "zones": digest(np.asarray(r.zones).astype(np.int32)),
                                       "areas": [int(v) for v in r.object_area]})
    out["proximity"] = []
    for min_area, core, prox in ((50, 0, 0), (30, 2, 12)):
        r = ls.proximity(z["blobs_a"], min_area, core_buffer=core, prox_max=prox)
        out["proximity"].append({"input": "blobs_a", "min_area": min_area, "core_buffer": core, "prox_max": prox,
                                 "classes": digest(np.asarray(r.classes)),
                                 "ids": digest(np.asarray(r.ids).astype(np.int32)),
                                 "zones": digest(np.asarray(r.zones).astype(np.int32)),
                                 "distance": digest(np.asarray(r.distance).astype(np.int32)),
                                 "cag": digest(np.asarray(r.cag).astype(np.int32)),
                                 "areas": [int(v) for v in r.object_area], "csv": r.csv_text("")})
    # the Restoration Planner on blobs_a: its resistance, status, and between its two largest objects
    blobs = z["blobs_a"]
    resist = np.asarray(ls.distance_to_resistance(blobs))
    acc = ls.accounting(blobs)
    ids = np.asarray(acc.ids)
    mark_a = (ids == acc.largest[0][0]).astype(np.uint8)
    mark_b = (ids == acc.largest[1][0]).astype(np.uint8)
    marker = np.where(mark_a > 0, 1, np.where(mark_b > 0, 2, np.where(blobs == 0, 3, 0))).astype(np.uint8)
    np.savez_compressed(REF / "gtb_planner.npz", a=mark_a, b=mark_b, marker=marker)

    def f32(a):
        a = np.asarray(a, dtype=np.float64)
        return digest(np.where(np.isfinite(a), a, np.nan).astype(np.float32))

    st = ls.restoration_status(blobs)
    lcp = ls.least_cost_path(resist, mark_a, mark_b)
    cor = ls.restoration_corridor(blobs, resist, mark_a, mark_b)
    iso_a = ls.isochrone_map(resist, np.where(marker == 2, 0, marker).astype(np.uint8))
    iso_ab = ls.isochrone_map_ab(resist, marker)
    out["planner"] = [{
        "input": "blobs_a", "resistance": digest(resist),
        "status": [float(v) for v in st.to_dict().values()],
        "cost_distance": f32(ls.cost_distance(resist, mark_a)),
        "lcp_cost": f32(lcp.combined_cost), "lcp_path": digest(np.asarray(lcp.lcp_mask).astype(np.uint8)),
        "lcp": [lcp.path_length, lcp.rest_pixels, lcp.expense, lcp.avg_distance],
        "corridor": [cor.size_a, cor.size_b, cor.rest_pixels, cor.lcp_result.avg_distance, cor.expense, cor.efficiency,
                     cor.eca_orig, cor.eca_new, cor.delta_eca, cor.doc_orig, cor.doc_new, cor.delta_doc],
        "isochrone_a": {"map": digest(np.asarray(iso_a.map)), "cost": f32(np.where(np.asarray(iso_a.cost) == -2**31, np.nan,
                                                                                   np.asarray(iso_a.cost))),
                        "min": iso_a.min_cost, "max": iso_a.max_cost},
        "isochrone_ab": {"map": digest(np.asarray(iso_ab.map)),
                         "cost": f32(np.where(np.asarray(iso_ab.cost) == -2**31, np.nan, np.asarray(iso_ab.cost))),
                         "path": digest(np.asarray(iso_ab.lcp).astype(np.uint8)), "min": iso_ab.min_cost,
                         "max": iso_ab.max_cost}}]
    # GTN of the MSPA of gtb_input (default MSPA)
    ms = ls.mspa(forest)
    ms = np.asarray(getattr(ms, "values", ms))
    comps = ls.gtn_components(ms)
    imp = ls.gtn_importance(ms, components=comps)
    cn, cl = ls.conefor_inputs(ms)
    out["gtn"] = [{"input": "gtb_input", "mspa": digest(ms.astype(np.uint8)),
                   "colored": digest(np.asarray(comps.colored_map)),
                   "components": digest(np.asarray(comps.components).astype(np.int32)),
                   "stats": comps.stats[["component_id", "core_pixels", "bridge_pixels", "total_pixels"]]
                   .to_numpy(np.float64).tolist(),
                   "summary": [comps.num_components, comps.num_connected_components, comps.num_isolated_cores,
                               comps.num_nodes, comps.num_links, comps.eca, comps.eca_pixels, comps.eca_max,
                               comps.eca_max_pixels, comps.doc],
                   "report": comps.summary(),
                   "importance_colored": digest(np.asarray(imp.colored_map)),
                   "importance": digest(np.asarray(imp.importance_map).astype(np.float32)),
                   "ids": digest(np.asarray(imp.node_link_ids).astype(np.int32)),
                   "links": imp.links[["link_id", "component_id", "pixels", "dpc", "normalized_importance"]]
                   .to_numpy(np.float64).tolist(),
                   "nodes": imp.nodes[["node_id", "component_id", "pixels", "dpc", "degree", "normalized_importance"]]
                   .to_numpy(np.float64).tolist() if len(imp.nodes) else [],
                   "importance_report": imp.summary_text(),
                   "conefor_nodes": cn.to_numpy(np.float64).tolist(), "conefor_links": cl.to_numpy(np.float64).tolist()}]
    # the text reports (summary()) of FOS, SPA and Accounting, in GTB's layout
    out["reports"] = []
    for inp, params in [("gtb_input", {"method": "FAD", "classes": 6}), ("gtb_input", {"method": "FED", "classes": 5}),
                        ("gtb_input", {"method": "FAC", "classes": 6, "window_size": 7, "pixel_resolution": 30.0}),
                        ("gscinput", {"method": "FAD", "classes": 5})]:
        out["reports"].append({"tool": "fos", "input": inp, "params": params,
                               "text": ls.fos(z[inp], **params).summary()})
    for v in (2, 3, 5, 6):
        out["reports"].append({"tool": "spa", "input": "gtb_input", "params": {"variant": v},
                               "text": ls.spa(forest, v).summary()})
    for params in [{}, {"highlight_largest": True, "pixel_resolution": 10.0}, {"thresholds": [10, 100]}]:
        out["reports"].append({"tool": "accounting", "input": "gtb_input", "params": params,
                               "text": ls.accounting(forest, **params).summary()})
    path = REF / "gtb_landschaft.json"
    path.write_text(json.dumps({"landschaft": ls.__version__, **out}, indent=1) + "\n", encoding="utf-8")
    print(path)


if __name__ == "__main__":
    main()
