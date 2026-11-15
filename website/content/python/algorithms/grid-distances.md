---
title: Distances and routes on grids
description: "gdistance in Python: transitions, geographic correction, cost surfaces, least-cost distances and paths, random walks and randomised shortest paths."
order: 120
---

The same in the app: [Cost distance](/tutorials/grid-distances/cost-distance/) and [Random walks](/tutorials/grid-distances/random-walks/). The same as a worked tutorial in Python: [Grid routes in Python](/tutorials/grid-distances/grid-routes-in-python/). API: the [grid distances](/python/api/cost/) page (the `tf.cost` module). Checked against gdistance 1.6.5: identical bits for transitions, cost surfaces, least-cost distances and paths, to 1e-14 typically for the random walks (see [how each tool is checked](/about/validation/#results-by-tool)).

`tf.cost` ports the R package gdistance: a raster becomes a graph of
neighbouring cells whose connections carry a conductance (a *transition*),
over which cost surfaces, least-cost distances and paths and random-walk
measures are computed.

The examples run on a small random resistance surface (30 x 30 cells) written to `resistance.tif`, with points, a categorical raster and masks to work on:

```python
import numpy as np
import terraflux as tf

rng = np.random.default_rng(1)
n = 30
template = tf.load_raster(tf.example_data("clc"))[:n, :n]
tf.write_raster(template.copy(data=np.exp(rng.normal(0, 0.4, (n, n)))), "resistance.tif")
xs, ys = template.x.values, template.y.values


def cell(i, j):
    """The (x, y) coordinates of the centre of a cell."""
    return (float(xs[j]), float(ys[i]))


(x1, y1), (x2, y2) = cell(2, 3), cell(25, 20)
points = [cell(2, 3), cell(25, 20), cell(10, 27), cell(20, 5)]
origin, goal, goals = cell(2, 3), cell(25, 20), [cell(25, 20), cell(10, 27)]
sources, targets = points[:2], points[2:]
locations, locs_a, locs_b = points, points[:2], points[2:]
landcover = template.copy(data=rng.integers(1, 4, (n, n)))      # a categorical raster
mask_a = np.zeros((n, n), dtype=bool); mask_a[:3, :3] = True
mask_b = np.zeros((n, n), dtype=bool); mask_b[-3:, -3:] = True
origin_mask, goal_mask = template.copy(data=mask_a), template.copy(data=mask_b)
```

```python
import numpy as np
import terraflux as tf

r = tf.load_raster("resistance.tif", validate=None)
tr = 1 / tf.cost.transition(r, "mean", 8)            # conductance = 1 / mean resistance
tr = tf.cost.geo_correction(tr)                      # / distance between cell centres
up = tf.cost.transition(r, "diff", 8, symmetric=False)          # x[to] - x[from]
f = tf.cost.transition(r, lambda x: 2 / (x[0] + x[1]), 4)       # any vectorised function
p = tf.cost.normalize(tr, "row")                     # random-walk probabilities
both = tf.cost.sum_reciprocal(tr, tr)                # resistances in series (of two transitions)
tr.to_sparse(); tr.to_raster("NZcolMeans"); tr.adjacency()
```

- `Transition.values` is `(directions, cells)`: the value of the connection from
  each cell in each direction (`offsets`), 0 = none. Symmetric transitions keep
  the directions after (0, 0) and hold `f(x[lower index], x[higher])`, as the
  upper triangle of gdistance's `dsCMatrix`; asymmetric ones hold
  `f(x[from], x[to])` for all directions. Cells with a missing value have no
  connections; 16 directions add the knight moves (raster's `adjacent`); on a
  longitude/latitude grid spanning -180 to 180 (raster's `.isGlobalLonLat`, also
  a grid without CRS in that range) the columns wrap around.
- Built-in functions use R's arithmetic (`mean` in 80-bit with its second pass,
  `sum`/`prod` in 80-bit, x87 `log`/`exp` for `"geometric"`), so they give
  gdistance's bits; callables get numpy's arithmetic. `map("sqrt"|"exp"|"log")`
  applies R's (x87) functions; `tr ** p` is R's `^`.
- `geo_correction`: distances between cell centres (`xyFromCell`) as raster's
  `pointDistance`: planar with R's vector (x87) `sqrt`, or GeographicLib 2.0's
  `geod_inverse` on WGS84 with mingw-w64's maths (x87 `sin`, `cos`, `atan2`, the
  UCRT's `hypot`, Cephes' `cbrt`): the distances are raster's bits on Windows
  (199,999 random pairs out of 200,000 identical; the other is a NaN on both
  sides). Symmetric transitions measure from the lower-index cell (the upper
  triangle); type `"r"` multiplies north-south connections by `cos(pi/180 *
  rowMeans(lat))` (80-bit mean, x87 `cos`).
- `normalize` and `to_raster` sum rows and columns as the Matrix package (1.7)
  does: in double, in increasing index order.
- `"mahal"`: `cov` of the complete cells (R's `cov.c`, 80-bit), `solve` (LAPACK
  3.12's `dgetrf2`/`dgetrs` over the reference BLAS, ported), the quadratic
  forms as R's two `dgemv`, R's vector `sqrt` and `mean`, in raster's `adjacent`
  pair order. gdistance's own method cannot build its result (empty
  `transitionCells`); the reference runs its code with that fixed.
- Differences from gdistance: `number / tr` acts on the connections only (in R
  it would make a dense matrix), `tr + number` is refused, `normalize(tr,
  "symm")` works on grids with missing cells (NaN or an error in gdistance).

Least-cost analysis:

```python
cost = tf.cost.acc_cost(tr, [(x1, y1), (x2, y2)])       # raster: cost from the nearest origin
d = tf.cost.cost_distance(tr, points)                   # n x n (symmetric for symmetric transitions)
d2 = tf.cost.cost_distance(tr, sources, targets)        # n_from x n_to
path = tf.cost.shortest_path(tr, origin, goals)         # Transition, 1/n_goals on the path steps
cells = tf.cost.shortest_path(tr, origin, goals, output="cells")
lines = tf.cost.shortest_path(tr, origin, goals, output="lines")  # GeoDataFrame
```

- The step cost is `1 / T[a, b]` (R: `E(g)$weight <- 1 / E(g)$weight`); a cost is the
  sum along the cheapest path from the source, so any correct Dijkstra gives
  gdistance's bits. As in `costDistance`, among one set of points on a symmetric
  transition each pair is summed from its later point (`as.dist` keeps the lower
  triangle); `acc_cost` starts all origins at 0 (gdistance's extra node with
  weight `1 / Inf`).
- `shortest_path` reproduces `igraph_get_shortest_paths_dijkstra` (igraph 2.1.3):
  the two-way indexed heap (`core/indheap.c`: a new or improved element rises
  above equal ones), incident edges in increasing neighbour order (undirected:
  the smaller neighbours, then the larger ones), parents replaced only by
  strictly shorter paths. The path transition is a general matrix with
  `1/n_goals` added on both directions of every step, path after path.
- Distances use a 4-ary heap without decrease-key and unsorted neighbours;
  `cost_distance` runs its sources in parallel and stops once every target is
  settled. Points outside the grid give NaN rows and columns (gdistance drops
  them and misaligns its matrix).

Random walks:

```python
tr = tf.cost.geo_correction(1 / tf.cost.transition(r, "mean", 8), "r")
c = tf.cost.commute_distance(tr, points)                 # n x n
p = tf.cost.passage(tr, origin, goal)                    # raster of net passages
p = tf.cost.passage(tr, origin_mask, goal_mask, gdistance_compat=False)
```

- The graph is the cells with a connection (`transitionSolidify`: row mean
  > 1e-300), its Laplacian `diag(colSums(T)) - T` (column sums in the Matrix
  order); each connected part is grounded at its last cell and factorised
  with the circuit module's multifrontal Cholesky (the grid's nested
  dissection for 4/8 directions without wrapping, the graph's own
  otherwise). gdistance grounds the whole graph at its last cell and solves
  with CHOLMOD, so it fails on several parts and on points at that cell.
- `commute_distance` = volume x effective resistance, the resistances from
  `G = L_g^-1` on the points' columns (block solves in parallel); the volume
  is `sum(transitionMatrix)` for a connected graph, each part's own
  otherwise.
- `passage` (no theta): unit current in at the origins (1/n each), out at the
  goals; node passage = `colSums(|V_i - V_j| g_ij) / 2`, 1 at origins and
  goals, 0 off the graph. gdistance's `.currentR` gives the grounded cell the
  potential of the one before it (`c(x, x[length(x)])`): kept with
  `gdistance_compat=True`. Mask rasters are mapped to the graph's cells
  (gdistance uses the raster cell numbers as graph indices, which is wrong
  when cells without connections come before them).
- `HabitatGraph`'s commute-time distances use the same engine.

Randomised shortest paths:

```python
p = tf.cost.passage(tr, origin, goal, theta=1)                       # net passages per cell
p = tf.cost.passage(tr, origin, goals, theta=1, total_net="total")    # expected visits
f = tf.cost.passage(tr, origin, goal, theta=1, output="transition")   # per connection
d = tf.cost.rsp_distance(tr, sources, targets, theta=1)               # n_from x n_to
d2 = tf.cost.rsp_distance(tr, sources, targets, theta=0.5, method=2)
```

- `W = exp(-theta / T) * P` (method 1; `P = T / rowSums(T)`, R's x87 `exp`) or
  `P^theta` (method 2); the goals' rows are scaled by `Ij` (0 for one goal:
  absorbing; `1 - 1/n` for several, as `passage`); `z_i = A^-T e_i`,
  `z_j = A^-1 e_j`, `A = I - Ij W`; `N = diag(z_i) Ij W diag(z_j) / z_ij`.
  Passages per cell: total `max(rowSums(N), colSums(N))`, net
  `rowSums(|N - N'|) / 2` doubled at the origins and goals (NaN off the graph,
  all 0 when `z_ij < 1e-300`); per connection: `N`, or `max(N - N', 0)`. Distances:
  `sum(C * N)` (net: `max(N - N', 0)`), one goal at a time.
- Solvers: rows with `Ij = 0` are fixed (identity) and moved to the right-hand
  side. For symmetric transitions with method 1, row `k` scaled by
  `rowSums_k / Ij_k` gives `M = diag(s / Ij) - exp(-theta C) * T`: symmetric and
  (strictly) diagonally dominant, factorised with the circuit module's
  Cholesky; `A x = b` is `M x = Q b + K b_F`, `A' y = e` is `y = Q M^-1 e`.
  Otherwise (asymmetric transitions, method 1) a multifrontal LU without
  pivoting (`src/cpp/cost/frontal_lu.cpp`: the Cholesky's symbolic analysis,
  full square fronts keeping L and U12, the L part assembled from the reverse
  connections, subtrees in parallel then the fronts above by heights, dense
  kernels blocked by 64 with 256 x 128 tiles); method 2 (rows of `P^theta` can
  add up to more than 1) and a failed LU fall back to Eigen's pivoting
  `SparseLU` in nested-dissection order. Factorisations and solves flush
  subnormal numbers to zero (MXCSR FTZ/DAZ per OpenMP thread, restored after):
  the factors of `I - W` decay below 1e-308 and subnormal arithmetic made them
  2 to 30 times slower.
- `passage` uses the cells with outgoing connections (`transitionSolidify`),
  `rsp_distance` every cell with a connection (gdistance's `rSPDistance` does not
  solidify). gdistance's `rSPDistance(method = 2)` fails; terraflux's is checked
  against a dense computation of the formula.

Route overlap:

```python
out = tf.cost.path_inc(tr, origin, locations)                  # {"overlap": n x n, "nonoverlap": n x n}
out = tf.cost.path_inc(tr, origin, locs_a, locs_b, theta=0.5)  # n_a x n_b, randomised shortest paths
bars = tf.cost.barriers(r, 8, breaks=[100, 200])               # list of Transitions (resistance 1)
cats = tf.cost.barriers(landcover, 8)                          # one layer per pair of categories
out = tf.cost.path_inc(tr, origin, locations, weight=bars)     # (layers, n, n)
out = tf.cost.path_inc(tr, origin, locations, functions=[lambda a, b: abs(a - b)])
```

- Flows (per connection, both directions, the order of the solidified graph):
  random walk `|V_k - V_l| g_kl` for a unit current from the origin to the
  location (`.currentM`, with the grounded cell's potential quirk under
  `gdistance_compat`); randomised shortest paths the net passages
  `max(N - N', 0)` with the location absorbing (`.probPass` with one goal).
- Pairs: `sum_e f(flow_p, flow_q) * R_e`, `R` = 1 / conductance by default (Inf
  set to 0), a transition's values (reciprocals of conductances), or one row per
  layer; among one set of locations only the upper triangle is computed and the
  diagonal is 0 (gdistance's `dist`); locations off the graph give NaN.
- `barriers`: continuous: `min < b < max` of the two cells for each break;
  categorical: gdistance's pair order (`.matrIndex`: (2,1), (3,1), ..., (3,2)),
  both directions or one layer per direction; `areas`: `sum(v == k) / 2` with k
  = 1..n (gdistance compares with the index, not the category value).
- gdistance's pathInc with `to` returns only NA, and its default weights index the
  full matrix with the reduced matrix's positions (wrong when cells without
  connections precede others): the references use grids without missing cells.

Reference: [`tools/make_reference_cost.py`](https://github.com/sacridini/terraflux-py/blob/main/tools/make_reference_cost.py) (gdistance 1.6.5, raster 3.6-31,
Matrix 1.7-1, igraph 2.1.3, R 4.4.2; cases in `tests/cost_cases.py`), tests in
`tests/test_cost_transition.py`, `tests/test_cost_lcp.py`,
`tests/test_cost_randomwalk.py`, `tests/test_cost_rsp.py` and
`tests/test_cost_pathinc.py`.

## References

- van Etten, J. (2017). R package gdistance: distances and routes on geographical grids. *Journal of Statistical Software*, 76(13). https://doi.org/10.18637/jss.v076.i13
- Dijkstra, E. W. (1959). A note on two problems in connexion with graphs. *Numerische Mathematik*, 1(1), 269–271. https://doi.org/10.1007/BF01386390
- Saerens, M., Achbany, Y., Fouss, F., & Yen, L. (2009). Randomized shortest-path problems: two related models. *Neural Computation*, 21(8), 2363–2404. https://doi.org/10.1162/neco.2009.11-07-643

More references, with how the studies used each method, are at the end of the application's tutorials of the same subject (linked at the top of this page).
