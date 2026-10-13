# SPDX-License-Identifier: GPL-3.0-or-later
"""What landschaft's Biodiverse port gives, for Terraflux's tests, written to tests/data/reference/bio.json and
bio_spatial.npz:

    python tools/make_bio_reference.py

Basedata of Biodiverse's example sites (cells of 100 km, coordinates as written, a text axis); spatial analyses
of every calculation without trees, matrices or properties under many spatial conditions (one or two
neighbour sets, definition queries): the scalar indices and the list indices as the engine returns them.

Run with the landschaft the submodule points at installed.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import landschaft as ls

ROOT = Path(__file__).resolve().parents[1]
REF = ROOT / "tests" / "data" / "reference"
B = ls.bio

BASEDATA = {
    "cells": dict(group_columns=["x", "y"], label_columns=["genus", "species"], cell_sizes=[100000, 100000]),
    "origin": dict(group_columns=["x", "y"], label_columns=["species"], cell_sizes=[200000, 150000],
                   cell_origins=[50000, -25000]),
    "as_written": dict(group_columns=["x", "y"], label_columns=["genus", "species"], cell_sizes=[0, 0]),
    "text_axis": dict(group_columns=["genus", "x"], label_columns=["species"], cell_sizes=[-1, 250000]),
    "counts": dict(group_columns=["x", "y"], label_columns=["genus"], sample_count_columns=["num"],
                   cell_sizes=[300000, 300000]),
}

CASES = [
    (["sp_circle (radius => 200000)"], None),
    (["sp_self_only()", "sp_circle (radius => 300000)"], None),
    (["sp_block (size => 500000)"], None),
    (["sp_block (size => [400000, 600000], origin => [50000, 0])", "sp_select_all()"], None),
    (["$D <= 250000 && $nbr_y > $y"], None),
    (["sp_annulus (inner_radius => 100000, outer_radius => 300000)", "sp_select_all()"], None),
    (["sp_ellipse (major_radius => 400000, minor_radius => 150000, rotate_angle_deg => 30)"], None),
    (["sp_is_left_of (vector_angle_deg => 45)"], "$x <= 2500000"),
    (["sp_square (size => 300000)"], None),
    (["sp_rectangle (sizes => [300000, 500000])", "sp_circle (radius => 600000)"], None),
    (["sp_circle_cell (radius => 2)"], None),
    (["sp_richness_greater_than (threshold => 3)"], None),
    (["sp_select_sequence (frequency => 10, first_offset => 2)"], None),
    (["sp_point_in_poly (polygon => [[2000000, 0], [4000000, 0], [4000000, 2000000], [2000000, 0]])"], None),
    (["my $r = 200000; $D <= $r || $nbr_x == $x"], None),
    (["sp_match_regex (re => qr/^2/)"], None),
    (["sp_match_text (text => $coord[1], axis => 1)"], None),
    (["$d[0] <= 100000 and abs ($d[1]) <= 300000", "$D <= 500000"], "$y > 500000"),
    (["sp_block (size => 300000) && $nbr_x < 3000000"], None),
    (["sp_select_element (element => '2650000:1150000')"], None),
]


def calculations() -> list[str]:
    reg = B._spatial._registry()
    out = []
    for name, info in reg.items():
        if info["uses_tree"] or name in B._spatial._MATRIX_CALCS or name.startswith(("calc_lbprop", "calc_gpprop")):
            continue
        out.append(name.removeprefix("calc_"))
    return sorted(out)


def main() -> None:
    sites = B.example_data()
    out: dict = {"landschaft": ls.__version__, "basedata": {}, "spatial": []}
    arrays: dict[str, np.ndarray] = {}
    for key, kw in BASEDATA.items():
        bd = B.BaseData.from_table(sites, **kw)
        out["basedata"][key] = {"options": kw, "groups": bd.groups, "labels": bd.labels}
        arrays[f"bd/{key}/indptr"] = bd.counts.indptr.astype(np.float64)
        arrays[f"bd/{key}/indices"] = bd.counts.indices.astype(np.float64)
        arrays[f"bd/{key}/counts"] = bd.counts.data.astype(np.float64)
        arrays[f"bd/{key}/coords"] = bd.coords.astype(np.float64)
    # the feature import: the sites as points (a plain floor into cells of 150 km, from an origin)
    import geopandas as gpd
    import pandas as pd

    df = pd.read_csv(sites)
    gdf = gpd.GeoDataFrame(df, geometry=gpd.points_from_xy(df.x, df.y), crs="EPSG:3577")
    pts = B.BaseData.from_points(gdf, ["genus", "species"], [150000, 150000], [10000, 20000], count_column="num")
    out["basedata_points"] = {"groups": pts.groups, "labels": pts.labels}
    arrays["bd/points/counts"] = pts.counts.data.astype(np.float64)
    arrays["bd/points/indices"] = pts.counts.indices.astype(np.float64)
    arrays["bd/points/coords"] = pts.coords.astype(np.float64)
    bd = B.BaseData.from_table(sites, **BASEDATA["cells"])
    calcs = calculations()
    out["calculations"] = calcs
    for k, (conds, query) in enumerate(CASES):
        res = B.spatial_analysis(bd, conds, calcs, query, prng_seed=1, n_threads=4)
        case = {"conditions": conds, "query": query, "groups": list(res.table.index), "columns": list(res.table.columns),
                "lists": []}
        arrays[f"sp/{k}/table"] = res.table.to_numpy(dtype=np.float64)
        for name in res.lists:
            kind, ip, keys, values = res.lists.raw(name)
            case["lists"].append([name, int(kind)])
            arrays[f"sp/{k}/list/{name}/indptr"] = np.asarray(ip, dtype=np.float64)
            arrays[f"sp/{k}/list/{name}/keys"] = np.asarray(keys, dtype=np.float64)
            arrays[f"sp/{k}/list/{name}/values"] = np.asarray(values, dtype=np.float64)
        case["texts"] = list(res.lists._texts)
        out["spatial"].append(case)
    (REF / "bio.json").write_text(json.dumps(out) + "\n", encoding="utf-8")
    np.savez_compressed(REF / "bio_spatial.npz", **arrays)
    print(REF / "bio.json", len(arrays))


TREES = ["Example_tree.nex", "Example_tree_with_colours.nex", "tree_nu.nex", "tree_polytomy.nex", "tree_species.nex",
         "tree_trim.nex", "tree_with_nonmatching_names.nex"]
NEWICKS = ["((a:1,b:2):1,c:3);", "((a,a),'b c');", "(x:1,(y:2,z:0.5e1)w:1.5)r:0;(p,q);", "((a:0,b:0):0,c:0);"]
TREE_CASES = [
    ("Example_tree.nex", ["sp_circle (radius => 200000)"]),
    ("tree_trim.nex", ["sp_circle (radius => 300000)"]),
    ("tree_polytomy.nex", ["sp_self_only()", "sp_circle (radius => 300000)"]),
    ("tree_with_nonmatching_names.nex", ["sp_block (size => 400000)"]),
    ("tree_nu.nex", ["sp_circle (radius => 150000)", "sp_select_all()"]),
]


def trees() -> None:
    """Trees read as Biodiverse reads them, and the phylogenetic indices (bio_trees.json, bio_phylo.npz)."""
    data = ROOT / "extern" / "landschaft" / "tests" / "data" / "bio"
    out: dict = {"files": {}, "newicks": [], "phylo": []}

    def dump(t):
        return {"name": t.name, "names": t.names, "parent": t.parent.tolist(), "length": t.length.tolist(),
                "children": t.children}

    for f in TREES:
        out["files"][f] = [dump(t) for t in B.read_trees(data / f)]
    for nwk in NEWICKS:
        out["newicks"].append([nwk, [dump(t) for t in B.read_trees(nwk)]])
    bd = B.BaseData.from_table(B.example_data(), **BASEDATA["cells"])
    reg = B._spatial._registry()
    calcs = sorted(k.removeprefix("calc_") for k, v in reg.items() if v["uses_tree"])
    out["calculations"] = calcs
    arrays: dict[str, np.ndarray] = {}
    for k, (f, conds) in enumerate(TREE_CASES):
        tree = B.read_tree(data / f)
        res = B.spatial_analysis(bd, conds, calcs, tree=tree, prng_seed=12345, n_threads=4)
        case = {"tree": f, "conditions": conds, "groups": list(res.table.index), "columns": list(res.table.columns),
                "lists": [], "texts": list(res.lists._texts), "nodes": list(res.lists._node_names)}
        arrays[f"ph/{k}/table"] = res.table.to_numpy(dtype=np.float64)
        for name in res.lists:
            kind, ip, keys, values = res.lists.raw(name)
            case["lists"].append([name, int(kind)])
            arrays[f"ph/{k}/list/{name}/indptr"] = np.asarray(ip, dtype=np.float64)
            arrays[f"ph/{k}/list/{name}/keys"] = np.asarray(keys, dtype=np.float64)
            arrays[f"ph/{k}/list/{name}/values"] = np.asarray(values, dtype=np.float64)
        out["phylo"].append(case)
    (REF / "bio_trees.json").write_text(json.dumps(out) + "\n", encoding="utf-8")
    np.savez_compressed(REF / "bio_phylo.npz", **arrays)
    print(REF / "bio_trees.json", len(arrays))


RAND_CALCS = ["richness", "endemism_central", "endemism_whole", "pd", "pe", "phylo_rpe2", "phylo_rpd2", "rarity_central_lists",
              "redundancy"]
RAND_CASES = [
    ("rand_nochange", {}),
    ("rand_csr_by_group", {}),
    ("rand_structured", {}),
    ("rand_structured", {"richness_multiplier": 1.5, "richness_addition": 1}),
    ("rand_spatially_structured", {}),
    ("rand_diffusion", {"spatial_conditions_for_label_allocation": "sp_circle (radius => 300000)"}),
    ("rand_random_walk", {"spatial_conditions_for_seed_location": "$x > 3000000"}),
    ("rand_independent_swaps", {"swap_count": 300}),
    ("rand_independent_swaps_modified", {}),
    ("rand_curveball", {"use_max_swap": True}),
    ("rand_structured", {"spatial_conditions_for_subset": "sp_block (size => 1000000)"}),
    ("rand_csr_by_group", {"definition_query": "$y > 1000000"}),
    ("rand_structured", {"labels_not_to_randomise": ["Genus:sp1", "Genus:sp5"]}),
    ("rand_csr_by_group", {"randomise_trees_by": "shuffle_terminal_names"}),
]


def randomisations() -> None:
    """Randomisations of an analysis with a tree (bio_rand.json, bio_rand.npz)."""
    bd = B.BaseData.from_table(B.example_data(), **BASEDATA["cells"])
    tree = B.read_tree(B.example_data("tree"))
    res = B.spatial_analysis(bd, "sp_circle (radius => 200000)", RAND_CALCS, tree=tree, prng_seed=1, n_threads=4)
    out: dict = {"calculations": RAND_CALCS, "columns": list(res.table.columns), "cases": []}
    arrays: dict[str, np.ndarray] = {}
    for k, (fn, kw) in enumerate(RAND_CASES):
        rnd = B.randomise(res, fn, iterations=5, seed=100 + k, n_threads=4, **kw)
        case = {"function": fn, "args": kw, "seed": 100 + k, "lists": []}
        arrays[f"r/{k}/comparison"] = rnd.comparison().to_numpy(dtype=np.float64)
        arrays[f"r/{k}/p_rank"] = rnd.p_rank().to_numpy(dtype=np.float64)
        arrays[f"r/{k}/z"] = rnd.z_scores().to_numpy(dtype=np.float64)
        arrays[f"r/{k}/canape"] = rnd.canape()["CANAPE_CODE"].to_numpy(dtype=np.float64)
        for lname in rnd._lists:
            df = rnd.comparison(lname)
            case["lists"].append([lname, df["group"].tolist(), df["key"].astype(str).tolist()])
            arrays[f"r/{k}/list/{lname}"] = df[["C", "Q", "T", "SUMX", "SUMXX"]].to_numpy(dtype=np.float64)
        out["cases"].append(case)
    (REF / "bio_rand.json").write_text(json.dumps(out) + "\n", encoding="utf-8")
    np.savez_compressed(REF / "bio_rand.npz", **arrays)
    print(REF / "bio_rand.json", len(arrays))


if __name__ == "__main__":
    main()
    trees()
    randomisations()
