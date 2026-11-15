---
title: Circuit connectivity
description: "Circuitscape and Omniscape: advanced mode, Omniscape windows, conditional connectivity, INI files, and the raster and network modes of Circuitscape."
order: 100
---

The same in the app: [Circuit theory](/tutorials/circuit-connectivity/circuit-theory/), [Circuitscape](/tutorials/circuit-connectivity/circuitscape/), [Omniscape](/tutorials/circuit-connectivity/omniscape/) and [Resistance surfaces](/tutorials/circuit-connectivity/resistance-surfaces/). The same as a worked tutorial in Python: [Circuit connectivity in Python](/tutorials/circuit-connectivity/circuit-connectivity-in-python/). API: the [circuit connectivity](/python/api/circuits/) page ([`tf.omniscape`][terraflux.circuit.omniscape], [`tf.circuit.advanced`][terraflux.circuit.advanced.advanced] and the Circuitscape functions). Checked against Omniscape 0.7 and Circuitscape 6.1 with CHOLMOD (see [how each tool is checked](/about/validation/#circuit-connectivity-in-detail)).

`tf.circuit` treats a conductance raster as an electrical circuit, as
[Circuitscape](https://circuitscape.org) 6.1 does, and `tf.omniscape` is
[Omniscape](https://github.com/Circuitscape/Omniscape.jl) 0.7 (omnidirectional
moving-window connectivity) on top of it.

The examples run on a small random resistance surface (40 x 50 cells), written as the files the examples name: a GeoTIFF for the resistance, ASCII grids for the focal points, sources and grounds, and text files for a network of six nodes. A real study reads its own files:

```python
import numpy as np
import terraflux as tf

rng = np.random.default_rng(1)
resistance = np.exp(rng.normal(0, 0.5, (40, 50)))           # a random resistance surface
template = tf.load_raster(tf.example_data("clc"))[:40, :50]
tf.write_raster(template.copy(data=resistance), "resistance.tif")


def write_asc(path, a, nodata=-9999):
    """An ESRI ASCII grid, the format Circuitscape reads."""
    with open(path, "w") as f:
        f.write(f"ncols {a.shape[1]}\nnrows {a.shape[0]}\nxllcorner 0\nyllcorner 0\ncellsize 1\nNODATA_value {nodata}\n")
        np.savetxt(f, a, fmt="%g")


write_asc("resistance.asc", resistance)
sites = np.zeros((40, 50)); sites[5, 5], sites[30, 10], sites[20, 40] = 1, 2, 3
write_asc("sites.asc", sites)                               # focal points: a raster of ids
sources = np.zeros((40, 50)); sources[5, 5] = 1
write_asc("sources.asc", sources)
grounds = np.zeros((40, 50)); grounds[35, 45] = 1
write_asc("grounds.asc", grounds)
write_asc("regions.asc", np.zeros((40, 50)))                # no short-circuit regions
open("pairs.txt", "w").write("mode include\n1 2\n1 3\n")      # the pairs to solve
open("strengths.txt", "w").write("1 1.0\n2 2.0\n3 1.0\n")      # current injected at each point

edges = [(1, 2, 1.0), (2, 3, 1.0), (3, 4, 2.0), (4, 5, 1.0), (5, 6, 1.0), (6, 1, 1.0), (2, 5, 3.0)]
open("graph.txt", "w").write("".join(f"{a} {b} {r}\n" for a, b, r in edges))   # node, node, resistance
open("focal_nodes.txt", "w").write("1\n3\n5\n")
open("sources.txt", "w").write("1 1.0\n")                    # node, current
open("grounds.txt", "w").write("5 1.0\n")                    # node, resistance to ground
```

```python
conductance = 1 / resistance            # NaN: no node
source = np.zeros_like(conductance)     # amps injected per cell
ground = np.zeros_like(conductance)     # conductance to ground; np.inf: direct ground (0 V)
source[10, 12] = 1.0
ground[30, 45] = np.inf

r = tf.circuit.advanced(conductance, source, ground)   # four_neighbors=False, n_threads=0
r.current   # current through every cell (0 where nothing flows)
r.voltage   # node voltages (NaN without a node)
```

## Advanced mode

It is Circuitscape's advanced mode on in-memory rasters (`compute_omniscape_current`,
the solve behind every Omniscape window):

* every cell with a conductance > 0 is a node, joined to its 8 neighbours (4 with
  `four_neighbors=True`) by the average conductance of both cells, `(x + y) / 2`, or
  `(x + y) / (2√2)` on the diagonals;
* a cell that is both a source and a ground loses its source; every connected component
  with a source and a ground is solved (finite grounds on the diagonal, direct grounds at
  0 V), the others carry no current;
* the grounded Laplacian is factorised exactly: a multifrontal Cholesky over a
  nested dissection of the grid (every separator line a dense front, factorised with
  Eigen's dense kernels); independent components in parallel;
* a node's current is the larger of its inflow and outflow, leaving out branch currents
  under 1e-8 of the largest one, with Circuitscape's column-major node order.

## Omniscape

```python
ds = tf.omniscape(
    "resistance.tif", radius=10,      # window radius in cells (100 and more on real maps)
    block_size=5, buffer=0,           # Omniscape's options, same names
    source=None,                      # default: 1 / resistance (source_from_resistance, r_cutoff)
    flow_potential=True, normalized_current=True,
)
ds.cum_currmap                 # cumulative current
ds.flow_potential              # the same circuits with conductance 1
ds.normalized_cum_currmap      # cum_currmap / flow_potential
ds.cum_currmap.rio.to_raster("cum_currmap.tif")
```

Every `block_size` × `block_size` block whose sources add up to more than 0 is a target.
Its window is the circle of `radius + buffer` cells around it; the block's strength is
injected into the sources within `radius` (the block left out, shared in proportion to
their strengths) and flows to the grounded target cell. The window currents are summed
into `cum_currmap`. Omniscape's options map one to one: `source_threshold`, `r_cutoff`,
`resistance_is_conductance`, `four_neighbors` (`connect_four_neighbors_only`),
`correct_artifacts`, `mask_nodata`, `flow_potential` (`calc_flow_potential`) and
`normalized_current` (`calc_normalized_current`). Omniscape's quirks are kept on
purpose: windows and circles cut at the map edges, the target block zeroed at
`radius + buffer` cells from the window's corner (not at the target when the window is
cut on the left or the top), even block sizes made odd, and the block-artifact
correction.

How it is fast:

* each window is factorised by a multifrontal Cholesky built on a geometric nested
  dissection of the grid: the window is cut by its middle row or column, the halves
  recursively, and every separator line becomes a dense front whose pivots are
  factorised with dense kernels (LLᵀ, triangular solve, rank-k update) and whose Schur
  complement goes to the parent front. On a disk of radius 100 (31 k cells) that is
  12 ms against 26 ms with Eigen's sparse LDLᵀ in the same order (and 67 ms / 119 ms
  for Circuitscape's CHOLMOD / cg+amg);
* windows of the same shape (the interior disk, and each shape cut by the edges) share
  one symbolic structure between all threads; a window only refills its values; the
  forward substitution runs inside the factorisation, while each front is in cache;
* the flow potential has conductance 1 everywhere: its factorisation is done once per
  shape and thread, and the flow potentials of up to 8 targets are solved together;
* on x86-64 the circuit code is also built for AVX2 and FMA (`terraflux._circuit_avx2`,
  loaded only when the CPU has them; `TERRAFLUX_CIRCUIT_AVX2=0` keeps the baseline
  build): the dense kernels are 1.2-1.5x faster. Both builds give the same results to
  1e-11 and are each deterministic (buffers aligned to 64 bytes);
* windows are solved in parallel and added to the maps row by row in a fixed order, so
  the result does not depend on the number of threads;
* very large windows: each thread keeps the factor of a window (about 145 MB at radius
  300, plus twice that for the flow potential), so fewer threads run when they would not
  fit in `memory_limit` (default: half of the physical memory; `attrs["threads"]` tells
  how many ran).

Verification: [`python tools/make_reference_omniscape.py omniscape`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_omniscape.py) runs Omniscape 0.7's
`run_omniscape` (INI files, `cholmod` and `cg+amg`) on its own test rasters with its
test configurations 1-3, and on synthetic maps (NoData blobs and lines, source files with
NoData, block sizes 1-7, buffers, 4 neighbours, conductance input, windows wider than
the map). NoData and zero cells are the same, and every value agrees with CHOLMOD to
1.7e-11 (relative). Omniscape's default `cg+amg` is up to 1.6e-4 of the largest value away
from its own CHOLMOD result where the block correction amplifies its error.

## Conditional connectivity, reclassification, INI files and single precision

```python
# a class map (1-3) as the resistance, and the climate and land cover that condition it
res = template.copy(data=rng.integers(1, 4, (40, 50)).astype("float64"))
temperature_now = template.copy(data=np.add.outer(np.linspace(0, 2, 40), np.linspace(0, 2, 50)))
temperature_2050 = temperature_now + 1.0
landcover = template.copy(data=rng.integers(1, 3, (40, 50)))

# Sources connect only to targets of similar climate (Omniscape's conditional options):
ds = tf.omniscape(res, 10, block_size=5,
                  condition1=temperature_now, condition1_future=temperature_2050,
                  compare_to_future="1", comparison1="within",
                  condition1_lower=-1.0, condition1_upper=1.0,
                  condition2=landcover, comparison2="equal")

ds = tf.omniscape(res, 10, reclass_table={1: 10, 2: 1, 3: None})   # None / NaN: NoData
ds = tf.omniscape(res, 10, precision="single")                      # float32 in and out
```

<!-- no-run -->
```python
ds = tf.circuit.omniscape_ini("config.ini")    # an Omniscape INI file (your own), outputs in project_name
```

* Conditions (`conditional`, `n_conditions`, `compare_to_future`): after a window's
  sources are scaled, those whose present condition does not match the target block are
  set to 0 (so less than the block's strength may be injected, as in Omniscape).
  `"within"` keeps a source when `lower <= condition - median(block) <= upper`, `"equal"`
  when `condition == mode(block)` (StatsBase's mode: the first value, in column-major
  order, to reach the highest count); the block's value comes from the future condition
  when `compare_to_future` names it, else from the present one; NoData conditions keep the
  source.
* `reclass_table` (`reclassify_resistance`): rows (value, new value), each matched against
  the original values; the default sources come from the reclassified resistance.
* `precision="single"` reads the rasters as float32 and returns float32 maps, as
  Omniscape does; the arithmetic stays double (Omniscape's own float32 solves are up to
  2e-4 away from the double result).
* `tf.circuit.omniscape_ini(path)` reads every Omniscape option with its default
  (`parse_ini` is Omniscape's parser), the rasters (paths from the working directory, or
  next to the INI file) and the reclass table, checks sizes and projections, and writes
  `config.ini`, `cum_currmap`, `flow_potential`, `normalized_cum_currmap` and
  `classified_resistance` to `project_name` (`_1`, `_2`... when it exists), GeoTIFF or
  ASCII grid with NoData -9999. `solver`, `parallelize` and `parallel_batch_size` have no
  effect.

Verification: [`python tools/make_reference_omniscape.py ini`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_omniscape.py) runs Omniscape's own test
INI files (configurations 1-7, 4a-4c, 32 bit and reclass, with its test rasters, copied
to `tests/data/reference/circuit/omniscape_input`) through `run_omniscape`: every output
agrees with Omniscape's CHOLMOD run to 5e-13 (conditional `within` and `equal`, futures,
four neighbours, conductance input, reclassification), and the single-precision run to
Omniscape's float32 rounding.

Timings on an Intel i5-13600K (6 performance + 8 efficiency cores, 20 threads), random
log-normal resistance, source = 1 / resistance, flow potential and normalised current
([`tools/omniscape_window_benchmark.jl`](https://github.com/sacridini/terraflux-py/blob/main/tools/omniscape_window_benchmark.jl) times single windows):

| map | Omniscape 0.7, 20 threads (cg+amg / cholmod) | terraflux, 1 thread / 20 threads |
|---|---|---|
| 300×300, radius 25, block 3 (10,000 targets) | 17.4 s / 18.6 s | 7.2 s / 0.76 s |
| 800×800, radius 10, block 3 (70,756 targets) | 29.4 s / – | 6.5 s / 0.64 s |
| 700×700, radius 25, block 5 (19,600 targets) | 46.7 s / – | 14.3 s / 1.36 s |
| 1000×1000, radius 100, block 15 (4,356 targets) | 275 s / – | – / 12.4 s |

The results agree with Omniscape's `cg+amg` to its tolerance (1e-6 of the largest
current). Large windows are limited by memory bandwidth with many threads (a radius-100
window writes and reads a 13 MB factor): 6x from 1 to 20 threads there, 10x for radii
10-25.

## Circuitscape (raster modes)

```python
from terraflux.circuit import circuitscape_advanced, circuitscape_one_to_all, circuitscape_pairwise

r = circuitscape_advanced("resistance.tif", "sources.asc", "grounds.asc",   # paths, DataArrays or arrays
                          polygons="regions.asc", four_neighbors=False, avg_resistances=False,
                          ground_is_resistance=True, remove_src_or_gnd="keepall")
r.voltage, r.current, r.nodemap

# effective resistances between focal points (a raster of ids; several cells of one id: a region)
p = circuitscape_pairwise("resistance.tif", "sites.asc", included_pairs="pairs.txt", cum_map=True)
p.resistances          # ids in the first row and column, -1 where not connected
p.cum_current          # the pairs' current maps summed

# one-to-all (all_to_one=True: all-to-one): every focal point in turn against the others
o = circuitscape_one_to_all("resistance.tif", "sites.asc", strengths="strengths.txt", cum_map=True)
o.resistances          # ids and the voltage over the injected strength at each point

```

<!-- no-run -->
```python
from terraflux.circuit import circuitscape_ini

r = circuitscape_ini("job.ini")      # a Circuitscape INI file (your own); maps written next to output_file
```

Circuitscape 6.1 on rasters, with its conventions: the node map (habitat cells numbered in
column-major order; the cells of a short-circuit region that holds habitat merged into one
node, its non-habitat cells included), edges weighted by the average conductance or, with
`connect_using_avg_resistances`, the average resistance of the two cells (summed between
merged nodes), connected components that ignore edges of weight 0, the mask, reclass
tables applied to the values read, sources and grounds summed per node (maps or text
lists of `value x y`), grounds as resistances (0: direct ground), unit currents, direct
grounds and the four `remove_src_or_gnd` policies with their quirks. ASCII grids are read
in double, as Circuitscape does (GDAL would give float32). Each component is factorised by
the same multifrontal Cholesky as Omniscape's windows, with the short-circuit regions
eliminated last. Large components (an L of more than 32 MB) are factorised in parallel:
subtrees of the dissection of similar work on separate threads, each with its own stack
of update matrices; the fronts above them by heights (several at once, or a lone one with
parallel kernels); large fronts (128 pivots or more) by a blocked right-looking Cholesky
over fixed blocks (LLT of 128-column diagonal blocks, panel solves by 256-row chunks,
the trailing update by 256 × 128 tiles). Every front is built in an aligned workspace,
so the arithmetic, and the result, is the same bits with any number of threads; a
forward substitution fused into the factorisation keeps the single-thread order of its
operations (the contributions of a subtree to the rows outside it are replayed at its
root's turn). On a 1000×1000 raster the factorisation takes 0.7 s with 20 threads
instead of 3.4 s (advanced mode as a whole: 2.6 s instead of 5.2 s).

Implemented: `scenario = advanced` (voltage and current maps; `circuitscape_ini` writes
`<output>_voltmap` and `<output>_curmap`, ASCII grids or GeoTIFF, and the configuration
in `output_file`) and `scenario = pairwise`: focal points from a raster or a text list
(sorted by id, negative ids dropped), include/exclude pair files (a `min`/`max` matrix or
a `mode` list; include mode drops the points it does not name), resistances with
Circuitscape's rules (-1 between components, 0 between focal points on one node), every
pair's voltage and current maps, the cumulative and maximum current maps with
`set_focal_node_currents_to_zero`, `set_null_*_to_nodata` and `log_transform_maps`, and
`<output>_resistances.out` / `_resistances_3columns.out`. Each component is factorised
once, with one node grounded (the voltage differences are exact; Circuitscape solves the
singular Laplacian shifted by 10ε), and its pairs are solved in parallel batches of right-
hand sides; without maps and exclusions Circuitscape's shortcut applies (only the pairs of
the first focal point are solved, the others follow from its voltages, with its quirks).
Focal regions (several cells of one id) get a graph per pair, the two regions merged with
the polygons they touch, as Circuitscape does; the pairs run in parallel.
`scenario = one-to-all` / `all-to-one`: the focal points and regions are merged into the
short-circuit regions; for every focal point one-to-all injects 1 A (or its variable
source strength) there with every other point a direct ground, all-to-one grounds it
and injects at the others; only its component is solved (the points in parallel, on one
graph unless focal regions meet an include file); results `id, resistance` (one-to-all:
voltage over strength, -1 when not solved; all-to-one: 0), per-point voltage and current
maps (`_voltmap_<id>`, `_curmap_<id>`), cumulative and maximum maps. Circuitscape's
quirks are kept: the include file prunes the points in both of its modes, excluded
points still inject current in all-to-one, variable strengths are taken by position,
and a point alone on its map is skipped.

Verification: [`python tools/make_reference_circuitscape.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_circuitscape.py) copies Circuitscape's raster
test inputs (`tests/data/reference/circuit/circuitscape/input/raster`, MIT) and runs every
one of its INI files with the `cholmod` solver; in advanced mode (`mgVerify1`-`7`,
`ex_advanced`: polygons, sources and grounds, finite and direct grounds, 4 and 8
neighbours, average resistances, a 355×481 map) every map agrees to 5e-12 of its largest
value, with the same zero cells. In pairwise mode (`sgVerify1`-`17`: polygons, focal
regions, masks, included pairs, 4 and 8 neighbours, voltage, current, cumulative and
maximum maps) the resistances agree to 1e-12 (to 9e-10 relative on `sgVerify17`,
resistances up to 3e3, where CHOLMOD's shifted singular solve itself loses digits) and
every map to 3e-13, with exactly the same set of per-pair maps. In one-to-all and
all-to-one (`oneToAllVerify1`-`13`, `allToOneVerify1`-`12`: polygons, focal regions,
include files, variable strengths, masks, focal-node currents set to zero) the results
and every map agree to 5e-14.

Timings (i5-13600K, 20 threads; a 1000×1000 log-normal resistance ASCII grid, 20 focal
points; reading and writing the ASCII grids included; Circuitscape 6.1 with its default
cg+amg, after compilation):

| run | Circuitscape | terraflux |
|---|---|---|
| pairwise, resistances only | 8.7 s | 2.7 s |
| pairwise with the cumulative current map (190 pairs) | 76.5 s | 9.5 s |
| one-to-all with every point's current map | 21.8 s | 4.6 s |

The results agree with Circuitscape's cg+amg to its tolerance (resistances to 7e-7). In
one-to-all and all-to-one the focal points are the only nodes with sources or grounds, so
every component is factorised once without them and each point is solved through the
dense Schur complement on the focal nodes (Circuitscape factorises the graph once per
point). ASCII grids are written by a parallel C++ formatter (shortest round-trip values).

## Circuitscape (network mode)

```python
from terraflux.circuit import circuitscape_network_advanced, circuitscape_network_pairwise

# edge list: node, node, resistance (graph_is_resistance=False: conductance)
r = circuitscape_network_pairwise("graph.txt", "focal_nodes.txt", current_maps=True, voltage_maps=True)
r.resistances                        # ids in the first row and column, -1: not connected
r.maps[("voltages", 1, 5)]           # node, voltage of one pair (voltage_maps=True)
r.cum_node_currents, r.cum_branch_currents
a = circuitscape_network_advanced("graph.txt", "sources.txt", "grounds.txt")
a.voltages, a.node_currents, a.branch_currents
```

<!-- no-run -->
```python
from terraflux.circuit import circuitscape_ini

circuitscape_ini("network.ini")      # an INI file of your own: data_type = network, pairwise or advanced
```

Circuitscape 6.1's `data_type = network` with its conventions: nodes numbered from 1, or
from 0 (then every number, focal nodes, sources, grounds and pair files included, moves
up by 1, as Circuitscape.jl does; focal nodes from 0 move on their own too); duplicate
edges add their conductances (`sparse` in input order), then `A + Aᵀ` drops the edges
whose conductances add up to 0 and self loops are left out of the Laplacian; components
come from the remaining edges. Pairwise solves every pair of focal nodes per component
(no resistance shortcut: Circuitscape keeps it for rasters), resistances with the -1 / 0
rules of the raster mode, include / exclude pair files; with any map option every pair's
node currents (`max(inflow, outflow)`, Circuitscape's 1e-8 thresholds) and branch currents
(every edge, zeroed below 1e-8 of the largest, written above 1e-6) and, with
`voltage_maps`, its node voltages; node and branch currents summed over the pairs (the
first input line of an edge takes the sum). Advanced mode: sources and grounds by node
(`sources[node] = value`: the last line of a node wins), ground resistances or
conductances, direct grounds, the four conflict policies; node and branch currents of
the whole graph; `use_unit_currents` and `use_direct_grounds` do not apply to networks.
Files are written with Circuitscape's names (`_resistances.out`, `_voltages_<a>_<b>.txt`,
`_node_currents_cum.txt`, `_branch_currents.txt`...).

Graphs have no geometry, so the solver orders them by a nested dissection of their own:
breadth-first levels from a pseudo-peripheral node, the smallest level that leaves a
fifth of the nodes on either side as separator (trimmed of the nodes that touch only one
side), components apart; the sets of one depth are split in parallel and the tree is the
same with any number of threads. The fronts are those of the raster solver.

Verification: [`python tools/make_reference_circuitscape.py network`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_circuitscape.py) runs Circuitscape's
network test cases (`sgNetworkVerify1-3`, `mgNetworkVerify1-3`) and six synthetic ones
(numbering from 0 and 1, duplicate edges and self loops, edges that cancel, focal nodes
repeated, isolated or out of the graph, include / exclude pairs, finite and direct
grounds, `rmvgnd` / `rmvall`, a 4000-node graph) with CHOLMOD: every output file, the same
set of files, to 1.1e-14. On a 1000×1000 grid-like network (1 M nodes, 3 M edges, 20
focal nodes, resistances only; 20 threads) terraflux takes 5.0 s, Circuitscape 27.7 s
with CHOLMOD and 182 s with its default cg+amg. The resistances agree with CHOLMOD's to
5e-10 (relative); an exact sparse LU of the grounded Laplacian (scipy, no diagonal shift)
differs from both by as much: that is the conditioning of a 1 M-node Laplacian and
Circuitscape's 10ε shift, not the solver.

---

## References

- McRae, B. H. (2006). Isolation by resistance. *Evolution*, 60(8), 1551–1561. https://doi.org/10.1111/j.0014-3820.2006.tb00500.x
- McRae, B. H., Dickson, B. G., Keitt, T. H., & Shah, V. B. (2008). Using circuit theory to model connectivity in ecology, evolution, and conservation. *Ecology*, 89(10), 2712–2724. https://doi.org/10.1890/07-1861.1
- Landau, V. A., Shah, V. B., Anantharaman, R., & Hall, K. R. (2021). Omniscape.jl: software to compute omnidirectional landscape connectivity. *Journal of Open Source Software*, 6(57), 2829. https://doi.org/10.21105/joss.02829

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
