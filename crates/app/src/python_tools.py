# Terraflux's remaining operators in numpy and pandas, as Terraflux runs them: the statistics of maps and of stacks
# (raster.stats, stack.reduce, raster.correlation, raster.standardize), their windows and patches (raster.focal,
# majority, sieve, label, expand, shrink), classes by breaks (raster.classify), the terrain of gdaldem
# (raster.terrain), a map's legend, a stack's layers, the checks, value.calc, numpy's generator, batch.run and the
# agents (tables of attributes). A map is an xarray DataArray (no data: NaN), a table a DataFrame.
import re as _re
import warnings as _warnings

import numpy as _np
import pandas as _pd
import xarray as _xr


def _plane(x):
    """A map's cells as a 2-D array of floats (no data: NaN)."""
    a = _num(x)
    return a[0] if _np.ndim(a) == 3 and a.shape[0] == 1 else a


def _like(x, a):
    """The map x with other cells (a number stays a number)."""
    if hasattr(x, "dims"):
        out = x.copy(data=_np.asarray(a, dtype="float64").reshape(x.shape))
        try:
            out = out.rio.write_nodata(_np.nan)
        except Exception:
            pass
        return out
    return a


def _name(b):
    """A layer's name as text (1.0 as 1)."""
    if isinstance(b, (float, _np.floating)) and float(b).is_integer():
        return str(int(b))
    return str(b)


def _layers_of(x, port):
    """The layers of a map, a stack (band, y, x) or a list of maps: their names, their cells (2-D), what it is
    (map, stack, series) and the first map (to give the results its places)."""
    if isinstance(x, (list, tuple)):
        planes = [_plane(m) for m in x]
        if not planes:
            raise ValueError(f"{port} is an empty series")
        if any(p.shape != planes[0].shape for p in planes):
            raise ValueError(f"the maps of {port} are not of one grid")
        return [f"{port}_{i + 1}" for i in range(len(planes))], planes, "series", x[0]
    if hasattr(x, "dims") and "band" in x.dims and x.sizes["band"] > 1:
        a = _num(x)
        return [_name(b) for b in x["band"].values], [a[i] for i in range(a.shape[0])], "stack",             x.isel(band=0, drop=True)
    return [port], [_plane(x)], "map", x


def _relayer(x, kind, planes):
    """What `_layers_of` took apart, put together again with other cells."""
    if kind == "series":
        return [_like(m, p) for m, p in zip(x, planes)]
    if kind == "stack":
        return _like(x, _np.stack(planes))
    return _like(x, planes[0])


def _number_text(x):
    """A number as Terraflux writes it (3.0 as 3)."""
    x = float(x)
    if _np.isfinite(x) and x == _np.floor(x) and abs(x) < 9e15:
        return str(int(x))
    return _np.format_float_positional(x, trim="-")


# --- statistics ---

def _stat_code(s):
    """A statistic's name as (kind, percentile)."""
    s = {"mode": "majority", "variety": "unique", "sd": "std"}.get(s, s)
    if s in ("count", "nodata", "sum", "mean", "min", "max", "range", "std", "var", "median", "majority", "minority",
             "unique"):
        return s, 0.0
    if s.startswith("percentile_"):
        p = float(s[len("percentile_"):])
        if 0.0 <= p <= 100.0:
            return "percentile", p
    raise ValueError(f"unknown statistic {s!r} (count, nodata, sum, mean, min, max, range, std, var, median, "
                     "majority, minority, unique, percentile_<p>)")


def _run_lengths(s):
    """Of values sorted along axis 0 (no data last): the length of each run of equal values at its last element
    (0 elsewhere)."""
    k = s.shape[0]
    run = _np.zeros(s.shape, dtype="int64")
    cur = _np.zeros(s.shape[1:], dtype="int64")
    for i in range(k):
        valid = ~_np.isnan(s[i])
        same = valid & (s[i] == s[i - 1]) if i else _np.zeros(valid.shape, dtype=bool)
        cur = _np.where(valid, _np.where(same, cur + 1, 1), 0)
        run[i] = cur
    last = _np.ones(s.shape, dtype=bool)
    last[:-1] = ~(s[1:] == s[:-1])
    return _np.where(last, run, 0)


def _cells_stat(stat, a, inside=None):
    """A statistic of the values along axis 0 of `a` (no data: NaN) at every cell, the values with one only; the
    cells of `inside` the ones that count as without a value (nodata)."""
    kind, p = stat if isinstance(stat, tuple) else _stat_code(stat)
    a = _np.asarray(a, dtype="float64")
    valid = ~_np.isnan(a)
    n = valid.sum(axis=0)
    missing = (~valid).sum(axis=0) if inside is None else ((~valid) & inside).sum(axis=0)
    if kind == "count":
        return n.astype("float64")
    if kind == "nodata":
        return missing.astype("float64")
    none = n == 0
    with _warnings.catch_warnings(), _np.errstate(all="ignore"):
        _warnings.simplefilter("ignore", RuntimeWarning)
        s = _np.sort(a, axis=0)
        if kind == "unique":
            out = (~_np.isnan(s[0])).astype("int64") + ((s[1:] != s[:-1]) & ~_np.isnan(s[1:])).sum(axis=0)
            return _np.where(none, 0.0, out.astype("float64"))
        top = _np.take_along_axis(s, _np.maximum(n - 1, 0)[None], axis=0)[0]
        if kind == "sum":
            out = _np.nansum(a, axis=0)
        elif kind == "mean":
            out = _np.nansum(a, axis=0) / n
        elif kind in ("std", "var"):
            m = _np.nansum(a, axis=0) / n
            v = _np.nansum((a - m) ** 2, axis=0) / n
            out = _np.sqrt(v) if kind == "std" else v
        elif kind == "min":
            out = s[0]
        elif kind == "max":
            out = top
        elif kind == "range":
            out = top - s[0]
        elif kind in ("median", "percentile"):
            out = _np.nanpercentile(a, 50.0 if kind == "median" else p, axis=0)
        else:
            run = _run_lengths(s)
            if kind == "majority":
                pick = _np.argmax(run == run.max(axis=0), axis=0)
            else:
                pick = _np.argmax(run == _np.where(run > 0, run, run.max() + 1).min(axis=0), axis=0)
            out = _np.take_along_axis(s, pick[None], axis=0)[0]
        return _np.where(none, _np.nan, out)


def _table_of(columns, rows):
    return _pd.DataFrame(_np.array(rows, dtype="float64").reshape(-1, len(columns)), columns=list(columns))


_SUMMARY = ["count", "nodata", "sum", "mean", "std", "min", "percentile_5", "percentile_25", "median",
            "percentile_75", "percentile_95", "max"]


def _stats_list(stats, default):
    if stats is None:
        return list(default)
    if isinstance(stats, str):
        return [s.strip() for s in stats.split(",") if s.strip()]
    return list(stats)


def _raster_stats(x, stats=None, bins=20, range=None):
    """raster.stats: a row per layer of its statistics, and a histogram (layer, bin, from, to, count, share)."""
    names, planes, _, _ = _layers_of(x, "map")
    stats = _stats_list(stats, _SUMMARY)
    codes = [_stat_code(s) for s in stats]
    bins = int(bins)
    if bins == 0:
        raise ValueError("a histogram of no bins")
    if range is not None and not (range[0] < range[1]):
        raise ValueError("the range's from must be below its to")
    rows, hist = [], []
    for i, a in enumerate(planes):
        v = a.ravel()[:, None]
        rows.append([i + 1] + [float(_cells_stat(c, v)[0]) for c in codes])
        ok = _np.sort(a[~_np.isnan(a)])
        if ok.size == 0:
            continue
        lo, hi = (float(range[0]), float(range[1])) if range is not None else (float(ok[0]), float(ok[-1]))
        width = (hi - lo) / bins
        inside = ok[(ok >= lo) & (ok <= hi)]
        # (numpy's: the last bin holds its right edge)
        b = _np.floor((inside - lo) / width).astype("int64") if width > 0 else _np.zeros(inside.shape, "int64")
        counts = _np.bincount(_np.minimum(b, bins - 1), minlength=bins)
        for k in _np.arange(bins):
            frm = lo + width * k
            to = hi if k + 1 == bins else lo + width * (k + 1)
            hist.append([i + 1, k + 1, frm, to, counts[k], counts[k] / ok.size])
    return (_table_of(["layer"] + stats, rows),
            _table_of(["layer", "bin", "from", "to", "count", "share"], hist))


def _stack_reduce(x, stat="mean"):
    """stack.reduce: a statistic of the layers at every cell (argmax, argmin: the layer, 1.., the first of a tie)."""
    names, planes, _, first = _layers_of(x, "maps")
    a = _np.stack(planes)
    if stat in ("argmax", "argmin"):
        valid = ~_np.isnan(a)
        key = _np.where(valid, a, -_np.inf if stat == "argmax" else _np.inf)
        pick = (_np.argmax if stat == "argmax" else _np.argmin)(key, axis=0) + 1.0
        out = _np.where(valid.any(axis=0), pick, _np.nan)
    else:
        out = _cells_stat(stat, a)
    return _like(first, out)


def _ranks(x):
    from scipy import stats

    return stats.rankdata(x, method="average")


def _fit(x, y):
    """The least squares fit of y on x: n, correlation, covariance (n - 1), slope, intercept."""
    n = float(len(x))
    if len(x) < 2:
        return [n, _np.nan, _np.nan, _np.nan, _np.nan]
    mx, my = x.sum() / n, y.sum() / n
    sxx, syy, sxy = ((x - mx) ** 2).sum(), ((y - my) ** 2).sum(), ((x - mx) * (y - my)).sum()
    r = sxy / _np.sqrt(sxx * syy) if sxx > 0 and syy > 0 else _np.nan
    slope = sxy / sxx if sxx > 0 else _np.nan
    return [n, r, sxy / (n - 1.0), slope, my - slope * mx]


def _raster_correlation(x, y=None, method="pearson"):
    """raster.correlation: Pearson's or Spearman's correlation and covariance between every two layers (and those of
    y), over the cells where both have a value: the matrix, the covariances and the pairs."""
    names, planes, _, _ = _layers_of(x, "x")
    if y is not None:
        n2, p2, _, _ = _layers_of(y, "y")
        if p2[0].shape != planes[0].shape:
            raise ValueError("x and y are of different sizes: raster.match makes them one grid")
        names, planes = names + n2, planes + p2
    k = len(planes)
    if k < 2:
        raise ValueError("a correlation is between two maps: x a stack, or a map and y another")
    if method not in ("pearson", "spearman"):
        raise ValueError(f"method {method!r}: pearson or spearman")
    spearman = method == "spearman"
    pairs = [(a, b) for a in range(k) for b in range(a + 1, k)]
    fits = {}
    for a, b in pairs:
        both = ~_np.isnan(planes[a]) & ~_np.isnan(planes[b])
        u, v = planes[a][both], planes[b][both]
        f = _fit(u, v)
        if spearman:
            g = _fit(_ranks(u), _ranks(v))
            fits[(a, b)] = ([f[0], g[1], g[2], f[3], f[4]], f[1])
        else:
            fits[(a, b)] = (f, f[1])

    def variance(p):
        v = p[~_np.isnan(p)]
        v = _ranks(v) if spearman else v
        n = float(v.size)
        with _np.errstate(all="ignore"):
            return float(((v - v.sum() / n) ** 2).sum() / (n - 1.0))

    matrix, cov = [], []
    for a in range(k):
        r, c = [a + 1], [a + 1]
        for b in range(k):
            if a == b:
                v = variance(planes[a])
                r.append(1.0 if v > 0 else _np.nan)
                c.append(v)
            else:
                f = fits[(min(a, b), max(a, b))][0]
                r.append(f[1])
                c.append(f[2])
        matrix.append(r)
        cov.append(c)
    table = [[a + 1, b + 1, f[0], f[1], f[2], f[3], f[4], p * p] for (a, b), (f, p) in fits.items()]
    columns = ["layer"] + names
    return (_table_of(columns, matrix), _table_of(columns, cov),
            _table_of(["x", "y", "n", "correlation", "covariance", "slope", "intercept", "r2"], table))


def _raster_standardize(x, method="zscore", low=2.0, high=98.0):
    """raster.standardize: the layers on a common scale (z-score, min-max or percentile stretch) and a table of each
    layer's center and scale."""
    names, planes, kind, first = _layers_of(x, "map")
    if method == "percentile" and not (0.0 <= low < high <= 100.0):
        raise ValueError("the percentiles: 0 <= low < high <= 100")
    if method not in ("zscore", "minmax", "percentile"):
        raise ValueError(f"method {method!r}: zscore, minmax or percentile")
    rows, out = [], []
    for i, a in enumerate(planes):
        v = a.ravel()[:, None]
        stat = lambda s: float(_cells_stat(s, v)[0])
        if method == "zscore":
            center, scale = stat("mean"), stat("std")
        elif method == "minmax":
            center = stat("min")
            scale = stat("max") - center
        else:
            center = stat(f"percentile_{float(low)!r}")
            scale = stat(f"percentile_{float(high)!r}") - center
        rows.append([i + 1, center, scale])
        with _np.errstate(all="ignore"):
            z = (a - center) / scale if scale > 0 else _np.where(_np.isnan(a), _np.nan, 0.0)
        out.append(_np.where(_np.isnan(a), _np.nan, _np.clip(z, 0.0, 1.0) if method == "percentile" else z))
    return _relayer(x, kind, out), _table_of(["layer", "center", "scale"], rows)


# --- windows and patches ---

def _padded(a, r, value=_np.nan):
    return _np.pad(a, r, mode="constant", constant_values=value)


def _shifted(a, offsets, r, rows):
    """The values of a at each offset (row, column) from the cells of the rows `rows` of the map (no data beyond its
    edges), and where the offsets are on the map."""
    h, w = a.shape
    p, ins = _padded(a, r), _padded(_np.ones(a.shape, dtype=bool), r, False)
    lo, hi = rows
    vals = _np.stack([p[r + dr + lo:r + dr + hi, r + dc:r + dc + w] for dr, dc in offsets])
    inside = _np.stack([ins[r + dr + lo:r + dr + hi, r + dc:r + dc + w] for dr, dc in offsets])
    return vals, inside


def _chunks(h, w, k):
    """Bands of rows with about 4 million values of a window of k cells."""
    step = max(1, int(4_000_000 // max(1, k * w)))
    return [(i, min(h, i + step)) for i in range(0, h, step)]


def _raster_focal(m, stat="mean", shape="square", radius=1, inner=None, sigma=None, fill=False):
    """raster.focal: a statistic of the cells in a window around every cell (square, circle, annulus or Gaussian)."""
    a = _plane(m)
    h, w = a.shape
    stat = {"mode": "majority", "variety": "unique", "sd": "std"}.get(stat, stat)
    code = _stat_code(stat)
    radius = int(radius)
    r = radius
    inner = float(radius // 2 if inner is None else inner)
    sigma = float(max(radius / 2.0, 0.5) if sigma is None else sigma)
    allw = [(dr, dc) for dr in range(-r, r + 1) for dc in range(-r, r + 1)]
    d2 = lambda o: o[0] * o[0] + o[1] * o[1]
    if shape == "square":
        window = [(o, 1.0) for o in allw]
    elif shape == "circle":
        window = [(o, 1.0) for o in allw if d2(o) <= r * r]
    elif shape == "annulus":
        window = [(o, 1.0) for o in allw if d2(o) <= r * r and d2(o) > inner * inner]
    elif shape == "gaussian":
        if stat not in ("mean", "sum"):
            raise ValueError("a Gaussian window weighs the cells: its statistic is mean or sum")
        if sigma <= 0:
            raise ValueError("sigma must be above 0")
        window = [(o, float(_np.exp(-d2(o) / (2.0 * sigma * sigma)))) for o in allw if d2(o) <= r * r]
    else:
        raise ValueError(f"shape {shape!r}: square, circle, annulus or gaussian")
    if not window:
        raise ValueError("the window holds no cell")
    offsets = [o for o, _ in window]
    weights = _np.array([wt for _, wt in window])
    out = _np.full(a.shape, _np.nan)
    for lo, hi in _chunks(h, w, len(window)):
        vals, inside = _shifted(a, offsets, r, (lo, hi))
        if shape == "gaussian":
            ok = ~_np.isnan(vals)
            wsum = (weights[:, None, None] * ok).sum(axis=0)
            vsum = _np.where(ok, weights[:, None, None] * _np.where(ok, vals, 0.0), 0.0).sum(axis=0)
            with _np.errstate(all="ignore"):
                res = _np.where(wsum > 0, vsum / wsum if stat == "mean" else vsum, _np.nan)
        else:
            res = _cells_stat(code, vals, inside)
        out[lo:hi] = res if fill else _np.where(_np.isnan(a[lo:hi]), _np.nan, res)
    return _like(m, out)


def _top_value(vals, smallest):
    """The most common value along axis 0 (no data left out), its count, how many values hold the top count (the
    smallest of them when `smallest`; else a tie has none)."""
    s = _np.sort(vals, axis=0)
    run = _run_lengths(s)
    top = run.max(axis=0)
    first = _np.argmax(run == top, axis=0)
    value = _np.take_along_axis(s, first[None], axis=0)[0]
    return value, top, (run == top).sum(axis=0) * (top > 0)


def _raster_majority(m, neighbours=8, radius=1, replace="majority", iterations=1):
    """raster.majority: a cell takes the most common class around it when more than half (or at least half) of the
    cells with a value hold it; a tie between classes keeps the cell."""
    if neighbours not in (4, 8):
        raise ValueError(f"neighbours {neighbours}: 4 or 8")
    if replace not in ("majority", "half"):
        raise ValueError(f"replace {replace!r}: majority or half")
    a = _plane(m).copy()
    h, w = a.shape
    r = int(radius)
    window = [(dr, dc) for dr in range(-r, r + 1) for dc in range(-r, r + 1)
              if (dr, dc) != (0, 0) and (neighbours == 8 or abs(dr) + abs(dc) <= r)]
    for _ in range(int(iterations)):
        before = a.copy()
        for lo, hi in _chunks(h, w, max(1, len(window))):
            vals, _ = _shifted(before, window, r, (lo, hi))
            n = (~_np.isnan(vals)).sum(axis=0)
            value, top, ties = _top_value(vals, False)
            keep = (ties == 1) & (top > 0) & ((2 * top > n) | ((replace == "half") & (2 * top >= n)))
            cells = a[lo:hi]
            a[lo:hi] = _np.where(keep & ~_np.isnan(cells), value, cells)
    return _like(m, a)


def _around(neighbours):
    if neighbours not in (4, 8):
        raise ValueError(f"neighbours {neighbours}: 4 or 8")
    if neighbours == 4:
        return [(-1, 0), (0, -1), (0, 1), (1, 0)]
    return [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)]


def _patches(a, neighbours):
    """The patches of a map (groups of connected cells of one value; no data in none), numbered 0.. in the order of
    their first cell: the patch of every cell (-1: none), each patch's value and size."""
    from scipy import ndimage

    structure = ndimage.generate_binary_structure(2, 1 if neighbours == 4 else 2)
    ids = _np.full(a.shape, -1, dtype="int64")
    base = 0
    for v in _np.unique(a[~_np.isnan(a)]):
        lab, n = ndimage.label(a == v, structure=structure)
        ids = _np.where(lab > 0, lab + base - 1, ids)
        base += n
    if base == 0:
        return ids, _np.zeros(0), _np.zeros(0, dtype="int64")
    flat = ids.ravel()
    found, first = _np.unique(flat[flat >= 0], return_index=True)
    order = _np.argsort(_np.flatnonzero(flat >= 0)[first], kind="stable")
    renumber = _np.empty(base, dtype="int64")
    renumber[found[order]] = _np.arange(base)
    ids = _np.where(ids >= 0, renumber[_np.maximum(ids, 0)], -1)
    flat = ids.ravel()
    values = _np.zeros(base)
    values[flat[flat >= 0]] = a.ravel()[flat >= 0]
    return ids, values, _np.bincount(flat[flat >= 0], minlength=base)


def _raster_sieve(m, threshold, neighbours=4):
    """raster.sieve: the patches smaller than `threshold` cells merged, the smallest first, into the largest patch
    they touch (as gdal_sieve); the cells without a value stay so."""
    a = _plane(m)
    ids, values, sizes = _patches(a, neighbours)
    sizes = [int(s) for s in sizes]
    k = len(sizes)
    around = _around(neighbours)
    h, w = a.shape
    # the patches each touches, in the order its cells and neighbours find them
    p = _padded(ids.astype("float64"), 1, -1.0).astype("int64")
    pairs = _np.stack([p[1 + dr:1 + dr + h, 1 + dc:1 + dc + w] for dr, dc in around], axis=-1).reshape(-1, len(around))
    own = ids.reshape(-1, 1)
    keep = (own >= 0) & (pairs >= 0) & (pairs != own)
    touching = [[] for _ in range(k)]
    if k:
        key = _np.broadcast_to(own, pairs.shape)[keep] * k + pairs[keep]
        _, first = _np.unique(key, return_index=True)
        for t in key[_np.sort(first)]:
            touching[t // k].append(int(t % k))
    parent = list(range(k))

    def root(q):
        while parent[q] != q:
            parent[q] = parent[parent[q]]
            q = parent[q]
        return q

    for q in sorted((q for q in range(k) if sizes[q] < threshold), key=lambda q: (sizes[q], q)):
        x = root(q)
        if sizes[x] >= threshold:
            continue
        listed, touching[x] = touching[x], []
        best = None
        for t in listed:
            b = root(t)
            if b != x and (best is None or sizes[b] > sizes[best]):
                best = b
        if best is None:
            touching[x] = listed
        else:
            parent[x] = best
            sizes[best] += sizes[x]
            touching[best].extend(listed)
    final = _np.array([values[root(q)] for q in range(k)]) if k else _np.zeros(0)
    return _like(m, _np.where(ids >= 0, final[_np.maximum(ids, 0)] if k else _np.nan, _np.nan))


def _resolution(m):
    """A map's cell width and height (1 without places)."""
    try:
        x, y = m["x"].values, m["y"].values
        return abs(float(x[1] - x[0])), abs(float(y[1] - y[0]))
    except Exception:
        return 1.0, 1.0


def _raster_label(m, neighbours=4, classes=None):
    """raster.label: the patches numbered 1.. in the order of their first cell, and a table of each: id, value,
    cells, area (map units squared)."""
    a = _plane(m)
    if classes is not None:
        a = _np.where(_np.isin(a, _np.atleast_1d(_np.asarray(classes, dtype="float64"))), a, _np.nan)
    ids, values, sizes = _patches(a, neighbours)
    rw, rh = _resolution(m)
    table = [[i + 1, values[i], sizes[i], sizes[i] * rw * rh] for i in range(len(sizes))]
    return (_like(m, _np.where(ids >= 0, ids + 1.0, _np.nan)),
            _table_of(["id", "value", "cells", "area"], table))


def _grow(m, classes, expand, cells=1, neighbours=8, into_missing=False):
    """raster.expand and raster.shrink: the classes grown into the cells around them, or shrunk away from them, a
    cell a step: a cell takes the most common value of the right cells around it (a tie: the smallest)."""
    classes = _np.atleast_1d(_np.asarray(classes, dtype="float64"))
    if classes.size == 0:
        raise ValueError("choose the classes")
    around = _around(neighbours)
    data = _plane(m).copy()
    h, w = data.shape
    for _ in range(int(cells)):
        inside = _np.isin(data, classes)
        if expand:
            candidate = ~inside & (into_missing | ~_np.isnan(data))
        else:
            candidate = inside
        value, change = _np.full(data.shape, _np.nan), _np.zeros(data.shape, dtype=bool)
        for lo, hi in _chunks(h, w, len(around)):
            vals, _ = _shifted(data, around, 1, (lo, hi))
            right = _np.isin(vals, classes) == expand
            vals = _np.where(right & ~_np.isnan(vals), vals, _np.nan)
            best, top, _ = _top_value(vals, True)
            value[lo:hi] = best
            change[lo:hi] = candidate[lo:hi] & (top > 0)
        if not change.any():
            break
        data = _np.where(change, value, data)
    return _like(m, data)


# --- classes by breaks ---

def _raster_classify(m, method="quantile", n=5, breaks=None, sd_width=1.0):
    """raster.classify: the values in classes 1.. by quantiles, equal intervals, standard deviations or the breaks
    given (a class holds the values above its lower break up to its upper one, as R's cut), and a table of each
    class: class, from, to, cells, share."""
    a = _plane(m)
    ok = _np.sort(a[~_np.isnan(a)])
    if ok.size == 0:
        raise ValueError("the map has no value")
    lo, hi = float(ok[0]), float(ok[-1])
    n = int(n)
    if n == 0 and method not in ("breaks", "sd"):
        raise ValueError("n: one class at least")
    if method == "quantile":
        inner = [float(_np.percentile(ok, 100.0 * k / n)) for k in range(1, n)]
    elif method == "equal":
        inner = [lo + (hi - lo) * k / n for k in range(1, n)]
    elif method == "sd":
        mean = float(ok.sum() / ok.size)
        width = float(_np.sqrt(((ok - mean) ** 2).sum() / ok.size)) * float(sd_width)
        if _np.isnan(width) or width <= 0:
            inner = []
        else:
            k0, k1 = int(_np.floor((lo - mean) / width)), int(_np.ceil((hi - mean) / width))
            inner = [mean + k * width for k in range(k0, k1 + 1)]
    elif method == "breaks":
        if breaks is None:
            raise ValueError("give the breaks")
        inner = [float(b) for b in breaks]
    else:
        raise ValueError(f"method {method!r}: quantile, equal, sd or breaks")
    inner = sorted(set(inner))
    if method != "breaks":
        inner = [b for b in inner if lo < b < hi]
    classes = len(inner) + 1
    out = _np.where(_np.isnan(a), _np.nan, 1.0 + _np.searchsorted(_np.array(inner), a, side="left"))
    cells = _np.bincount(out[~_np.isnan(out)].astype("int64") - 1, minlength=classes)
    rows = []
    for k in range(classes):
        frm = min(lo, inner[0] if inner else lo) if k == 0 else inner[k - 1]
        to = max(hi, inner[-1] if inner else hi) if k + 1 == classes else inner[k]
        rows.append([k + 1, frm, to, cells[k], cells[k] / ok.size])
    return _like(m, out), _table_of(["class", "from", "to", "cells", "share"], rows)


# --- terrain (gdaldem's, Horn's method, the edges computed from the nearest cells) ---

def _raster_terrain(m, measure="slope"):
    """raster.terrain: slope (degrees), aspect (degrees from north, clockwise; flat cells without a value), hillshade
    (azimuth 315, altitude 45; 1-255), TRI (Riley's: the root of the squared differences to the 8 cells around), TPI (the cell less their
    mean) or roughness (largest less smallest of the 3 x 3), as gdaldem with -compute_edges (the cells at the edges from the cells extrapolated beyond)."""
    if measure not in ("slope", "aspect", "hillshade", "TRI", "TPI", "roughness"):
        raise ValueError(f"measure {measure!r}: slope, aspect, hillshade, TRI, TPI or roughness")
    a = _plane(m)
    ew, ns = _resolution(m)
    h, w = a.shape
    if h < 2 or w < 2:
        return _like(m, _np.full(a.shape, _np.nan))
    # the 3 x 3 windows: beyond the edges, the cells are extrapolated from the two nearest (linearly) along the edge's
    # normal; the top and bottom rows (corners too) first, copying the column beyond the corner
    left_right = _np.pad(_np.concatenate([2 * a[:, :1] - a[:, 1:2], a, 2 * a[:, -1:] - a[:, -2:-1]], axis=1),
                         ((1, 1), (0, 0)), mode="edge")
    top_bottom = _np.pad(_np.concatenate([2 * a[:1] - a[1:2], a, 2 * a[-1:] - a[-2:-1]], axis=0),
                         ((0, 0), (1, 1)), mode="edge")
    on_edge = _np.zeros((h, 1), dtype=bool)
    on_edge[[0, -1]] = True
    win = [_np.where(on_edge, top_bottom[i:i + h, j:j + w], left_right[i:i + h, j:j + w])
           for i in range(3) for j in range(3)]
    centre = win[4]
    # (a neighbour without a value takes the centre's)
    win = [_np.where(_np.isnan(v), centre, v) for v in win]
    if measure in ("slope", "aspect", "hillshade"):
        west_east = (win[0] + 2 * win[3] + win[6]) - (win[2] + 2 * win[5] + win[8])
        north_south = (win[6] + 2 * win[7] + win[8]) - (win[0] + 2 * win[1] + win[2])
        dx, dy = west_east / ew, north_south / ns
    with _np.errstate(all="ignore"):
        if measure == "slope":
            out = _np.degrees(_np.arctan(_np.sqrt(dx * dx + dy * dy) / 8.0))
        elif measure == "aspect":
            ax = _np.arctan2(north_south, west_east)
            out = _np.degrees(_np.where(ax > _np.pi / 2, 2 * _np.pi - ax + _np.pi / 2, _np.pi / 2 - ax))
            out = _np.where(out >= 360.0, 0.0, out)
            out = _np.where((west_east == 0) & (north_south == 0), _np.nan, out)
        elif measure == "hillshade":
            alt, az = _np.radians(45.0), _np.radians(315.0)
            k = 1.0 / 8.0
            aspect = _np.arctan2(-dy, dx)
            cang = (_np.sin(alt) - _np.cos(alt) * k * _np.sqrt(dx * dx + dy * dy) * _np.sin(aspect - az)) \
                / _np.sqrt(1 + k * k * (dx * dx + dy * dy))
            out = _np.where(cang <= 0, 1.0, 1.0 + 254.0 * cang)
            out = _np.floor(out + 0.5)
        elif measure == "TRI":
            out = _np.sqrt(sum((v - centre) ** 2 for i, v in enumerate(win) if i != 4))
        elif measure == "TPI":
            out = centre - sum(v for i, v in enumerate(win) if i != 4) / 8.0
        else:
            out = _np.nanmax(_np.stack(win), axis=0) - _np.nanmin(_np.stack(win), axis=0)
    out = _np.where(_np.isnan(centre), _np.nan, out)
    return _like(m, out.astype("float32"))


# --- a map's legend, a stack's layers ---

def _raster_legend(m, classes):
    """raster.legend: the same map carrying its classes' names and colours (terraflux_legend, as JSON)."""
    import json

    out = m.copy()
    out.attrs = dict(m.attrs)
    out.attrs["terraflux_legend"] = json.dumps(
        [{"value": float(c[0]), "name": c[1] if len(c) > 1 else "", "colour": c[2] if len(c) > 2 else ""}
         for c in classes])
    return out


def _stack_layers(x):
    """The layers of a stack (band, y, x), a series of maps or a map: their names (text) and maps."""
    if isinstance(x, (list, tuple)):
        return [str(i + 1) for i in range(len(x))], list(x)
    if hasattr(x, "dims") and "band" in x.dims:
        return [_name(b) for b in x["band"].values], [x.isel(band=i, drop=True) for i in range(x.sizes["band"])]
    return None, [x]


def _stack_layer(x, layer):
    """stack.extract_layer: a layer by its name, else by its 0-based number; `last` the last one."""
    names, maps = _stack_layers(x)
    layer = _name(layer)
    if names is None:
        if layer in ("0", "last"):
            return x
        raise ValueError(f"a map has no layers: no layer {layer!r}")
    if layer in names:
        return maps[names.index(layer)]
    if layer.isdigit() and int(layer) < len(names):
        return maps[int(layer)]
    if layer == "last" and names:
        return maps[-1]
    shown = ", ".join(names[:12]) + (f" and {len(names) - 12} more" if len(names) > 12 else "")
    raise ValueError(f"no layer {layer!r} (the layers: {shown})")


def _stack_set(x, m, name):
    """stack.set: the stack with the map in the layer of a name (replacing it, else added at the end)."""
    if not name:
        raise ValueError("give the layer's name")
    names, maps = _stack_layers(x)
    if hasattr(m, "dims") and "band" in m.dims:
        m = m.squeeze("band", drop=True)
    names, maps = list(names or ["0"]), list(maps)
    maps = [(m if n == name else v) for n, v in zip(names, maps)]
    if name not in names:
        names.append(name)
        maps.append(m)
    return _xr.concat([v.drop_vars("band", errors="ignore").expand_dims(band=[n]) for n, v in zip(names, maps)],
                      dim="band")


def _class_name(n):
    """A layer named by a class ("1") or a transition ("1_to_3") as Terraflux writes it."""
    n = _name(n)
    try:
        if "_to_" in n:
            f, t = n.split("_to_", 1)
            return f"{_number_text(float(f.strip()))}_to_{_number_text(float(t.strip()))}"
        return _number_text(float(n.strip()))
    except ValueError:
        raise ValueError(f'the layer "{n}" is not named by a class (or "from_to_to")') from None


def _stack_classes(x):
    """stack.classes: a stack whose layers are named by class ("1") or transition ("1_to_3")."""
    names, maps = _stack_layers(x)
    if names is None:
        raise ValueError("a stack of layers named by class")
    out = x.copy()
    return out.assign_coords(band=[_class_name(n) for n in names])


# --- checks ---

def _verdict(node, passed, default, detail, message=None, warn=False):
    """A check's end: passed, or failed with its message and what failed, stopping the script (or warning)."""
    if not passed:
        text = f"{default if message is None else message} ({detail})"
        if not warn:
            raise AssertionError(f'node "{node}": {text}')
        _warnings.warn(text)
    return bool(passed)


def _cells_of(x):
    """The cells with a value of a map or of every layer of a stack; None for a number."""
    if isinstance(x, (bool, int, float, _np.number)):
        return None
    a = _num(x)
    return a[~_np.isnan(a)]


def _check_range(node, value, minimum=None, maximum=None, message=None, warn=False):
    """check.range: every value (of a map's cells, a stack's, or a number) between min and max, both included."""
    if minimum is None and maximum is None:
        raise ValueError("give min, max or both")
    lo = -_np.inf if minimum is None else minimum
    hi = _np.inf if maximum is None else maximum
    if minimum is not None and maximum is not None:
        span = f"between {_number_text(minimum)} and {_number_text(maximum)}"
    elif minimum is not None:
        span = f"at least {_number_text(minimum)}"
    else:
        span = f"at most {_number_text(hi)}"
    cells = _cells_of(value)
    if cells is None:
        ok = lo <= value <= hi
        return _verdict(node, ok, "values are out of the range", f"{_number_text(value)} is not {span}", message, warn)
    bad = cells[(cells < lo) | (cells > hi)]
    detail = (f"every one of {cells.size} cells {span}" if bad.size == 0 else
              f"{bad.size} of {cells.size} cells not {span}, from {_number_text(bad.min())} to "
              f"{_number_text(bad.max())}")
    return _verdict(node, bad.size == 0, "values are out of the range", detail, message, warn)


def _check_no_nodata(node, x, within=None, max_share=0.0, message=None, warn=False):
    """check.no_nodata: no cell without a value in a map (every layer of a stack), only within the cells of `within`
    that have one, up to a share of the cells allowed."""
    a = _num(x)
    layers = list(a) if _np.ndim(a) == 3 else [a]
    keep = None if within is None else ~_np.isnan(_plane(within))
    missing = expected = 0
    for layer in layers:
        if keep is not None and keep.shape != layer.shape:
            raise ValueError('"map" and "within" differ in size: put them on one grid first')
        mask = _np.ones(layer.shape, dtype=bool) if keep is None else keep
        expected += int(mask.sum())
        missing += int((_np.isnan(layer) & mask).sum())
    share = 0.0 if expected == 0 else missing / expected
    detail = f"{missing} of {expected} cells without a value ({share:.4f} of them)"
    return _verdict(node, share <= max_share, "cells have no value", detail, message, warn)


def _check_area_kept(node, before, after, within=None, cls=None, tolerance=0.0, message=None, warn=False):
    """check.area_kept: the cells of a class (else the cells with a value) the same in both maps up to a tolerance;
    with `within`, every cell of it with a value keeps its class."""
    a, b = _plane(before), _plane(after)
    if a.shape != b.shape:
        raise ValueError("before and after differ in size: put them on one grid first")
    count = (lambda z: int((z == cls).sum())) if cls is not None else (lambda z: int((~_np.isnan(z)).sum()))
    n0, n1 = count(a), count(b)
    what = f"class {_number_text(cls)}" if cls is not None else "cells with a value"
    ok = abs(n1 - n0) <= tolerance
    detail = f"{what}: {n0} cells before, {n1} after"
    if within is not None:
        w = _plane(within)
        if w.shape != a.shape:
            raise ValueError("before and within differ in size: put them on one grid first")
        same = (a == b) | (_np.isnan(a) & _np.isnan(b))
        changed = int((~_np.isnan(w) & ~same).sum())
        ok = ok and changed == 0
        detail += f"; {changed} cells changed within the area kept"
    return _verdict(node, ok, "the area was not kept", detail, message, warn)


def _rounded(x):
    return _np.sign(x) * _np.floor(_np.abs(x) + 0.5)


def _check_accuracy(node, m, reference, minimum, measure="overall", message=None, warn=False):
    """check.accuracy: the overall agreement (or Cohen's kappa) of a map with a reference at least a minimum."""
    a, b = _plane(m), _plane(reference)
    if a.shape != b.shape:
        raise ValueError("map and reference differ in size: put them on one grid first")
    both = ~_np.isnan(a) & ~_np.isnan(b)
    n = int(both.sum())
    if n == 0:
        raise ValueError("no cell has a value in both maps")
    u, v = _rounded(a[both]).astype("int64"), _rounded(b[both]).astype("int64")
    po = float((u == v).sum()) / n
    if measure == "kappa":
        rows, cols = dict(zip(*_np.unique(u, return_counts=True))), dict(zip(*_np.unique(v, return_counts=True)))
        pe = sum(c * cols.get(k, 0) for k, c in rows.items()) / (float(n) * n)
        value = 1.0 if pe >= 1.0 else (po - pe) / (1.0 - pe)
    else:
        value = po
    name = "kappa" if measure == "kappa" else "overall agreement"
    detail = f"{name} {value:.4f} of {n} cells, the minimum {_number_text(minimum)}"
    return _verdict(node, value >= minimum, "the map is not accurate enough", detail, message, warn)


def _check_demand(node, landscape, demand, step, tolerance=0.0, relative=False, message=None, warn=False):
    """check.demand: the cells of each class of the demand in the map of the step equal to the demand of that step,
    up to a tolerance (in cells, or a share of the demand)."""
    step = int(step)
    if step < 0:
        raise ValueError(f"the step must be 0 or more, not {step}")
    table = demand.table
    if step >= len(table):
        raise ValueError(f"the demand has no step {step}")
    a = _plane(landscape)
    have = dict(zip(*_np.unique(_rounded(a[~_np.isnan(a)]).astype("int64"), return_counts=True)))
    off = []
    for c in table.columns:
        want = float(table.iloc[step][c])
        got = float(have.get(int(_rounded(float(c))), 0))
        allowed = tolerance * abs(want) if relative else tolerance
        if abs(got - want) > allowed + 1e-9:
            off.append(f"class {_number_text(c)}: {_number_text(got)} cells, the demand {_number_text(want)}")
    detail = f"the {len(table.columns)} classes of step {step} met" if not off else f"step {step}: " + "; ".join(off)
    return _verdict(node, not off, "the demand was not met", detail, message, warn)


def _numbers_of(inputs):
    """The inputs that are numbers (true and false as 1 and 0), and the tables' values of a row as table__column."""
    numbers = {}
    for k, v in inputs.items():
        if isinstance(v, (bool, _np.bool_)):
            numbers[k] = float(v)
        elif isinstance(v, (int, float, _np.number)):
            numbers[k] = float(v)
        elif hasattr(v, "ndim") and not isinstance(v, _pd.DataFrame) and _np.ndim(v) == 0:
            numbers[k] = float(v)
    return numbers


def _row_values(inputs, expression, row):
    """The values of the tables' row an expression names (`t.column`), as the variables `t__column`."""
    values, text = {}, expression
    for k, v in inputs.items():
        if not isinstance(v, _pd.DataFrame):
            continue
        n = len(v)
        i = n + row if row < 0 else row - 1
        for column in sorted(set(_re.findall(rf"\b{_re.escape(k)}\.([A-Za-z_]\w*)", expression))):
            if not 0 <= i < n:
                raise ValueError(f"{k} has {n} rows, no row {row}")
            values[f"{k}__{column}"] = float(v.iloc[i][column])
        text = _re.sub(rf"\b{_re.escape(k)}\.([A-Za-z_]\w*)", rf"{k}__\1", text)
    return values, text


def _check_assert(node, expression, message=None, warn=False, row=1, **inputs):
    """check.assert: `expression` over the inputs must hold: of every cell with a value when maps are among them,
    else true (not 0)."""
    if not expression.strip():
        raise ValueError("give the expression that must be true")
    if any(hasattr(v, "dims") for v in inputs.values()):
        res = _calc(expression, **inputs)["map"]
        cells = _num(res)
        known = cells[~_np.isnan(cells)]
        failed = int((known == 0).sum())
        passed, detail = failed == 0, f"{failed} of {known.size} cells fail"
    else:
        x = _value_calc(expression, row=row, **inputs)["value"]
        passed, detail = (x != 0 and not _np.isnan(x)), f"it is {_number_text(x)}"
    return _verdict(node, passed, "the check failed", f"{expression}: {detail}", message, warn)


# --- value.calc, numpy's generator ---

def _fill_text(template, values, value):
    """The text of value.calc: {name} replaced by an input's value ({value} by the expression's), {{ and }} as one."""
    out, i = [], 0
    while i < len(template):
        c = template[i]
        if c in "{}" and template[i + 1:i + 2] == c:
            out.append(c)
            i += 2
        elif c == "}":
            raise ValueError(f"a lone }} in {template!r} (}}}} for one)")
        elif c == "{":
            end = template.find("}", i)
            if end < 0:
                raise ValueError(f"a {{ without its }} in {template!r}")
            name = template[i + 1:end].strip()
            if name == "value" and value is not None:
                out.append(_number_text(value))
            elif isinstance(values.get(name), (str, bool, int, float, _np.number)):
                v = values[name]
                out.append(("true" if v else "false") if isinstance(v, (bool, _np.bool_)) else
                           _number_text(v) if isinstance(v, (float, _np.floating)) else str(v))
            else:
                raise ValueError(f"{{{name}}}: no input {name!r} of a number or a text")
            i = end + 1
        else:
            out.append(c)
            i += 1
    return "".join(out)


def _value_calc(expression=None, text=None, row=1, **inputs):
    """value.calc: a number from the inputs that are numbers (and a table's value as table.column of its row) by an
    expression in the calculator's syntax, and a text from a template of the inputs; as {"value", "text"}."""
    if expression is None and text is None:
        raise ValueError("give an expression, a text or both")
    out, value = {}, None
    if expression is not None:
        cells, expression = _row_values(inputs, expression, int(row))
        numbers = {**_numbers_of(inputs), **cells}
        value = float(_calc(expression, **numbers)["map"])
        out["value"] = value
    if text is not None:
        out["text"] = _fill_text(text, inputs, value)
    return out


# --- batch.run ---

def _batch_table(results):
    """batch.run's table (a row per input: its number, ok, skipped, seconds, files) and report."""
    rows = [[i + 1, float(r.ok), 0.0, r.seconds, len(r.outputs)] for i, r in enumerate(results)]
    report = "\n".join(f"{r.input}: ok, {len(r.outputs)} files" if r.ok else f"{r.input}: {r.error}" for r in results)
    return _table_of(["input", "ok", "skipped", "seconds", "files"], rows), report


# --- agents: a table of attributes, a row per agent ---

def _agents_table(agents=None, table=None):
    """agents.table: agents from a table, or from a list of objects of numbers (a column per name, in order of
    appearance, each object's names in order; no value: NaN)."""
    if table is not None:
        return table.copy()
    if agents is None:
        raise ValueError("give the agents or a table")
    columns = []
    for r in agents:
        columns.extend(k for k in sorted(r) if k not in columns)
    return _pd.DataFrame({c: [float(r.get(c, _np.nan)) for r in agents] for c in columns}, columns=columns)


def _agents_parcels(parcels, **maps):
    """agents.parcels: an agent per parcel (increasing id): id, cells, and the mean of each map over the parcel."""
    p = _plane(parcels).ravel()
    ids = _np.unique(p[~_np.isnan(p)])
    index = _np.searchsorted(ids, p)
    ok = ~_np.isnan(p)
    out = {"id": ids, "cells": _np.bincount(index[ok], minlength=ids.size).astype("float64")}
    for name, m in maps.items():
        v = _plane(m).ravel()
        if v.shape != p.shape:
            raise ValueError(f"{name} is not on the grid of the parcels")
        good = ok & ~_np.isnan(v)
        s = _np.bincount(index[good], weights=v[good], minlength=ids.size)
        c = _np.bincount(index[good], minlength=ids.size)
        with _np.errstate(all="ignore"):
            out[name] = _np.where(c > 0, s / c, _np.nan)
    return _pd.DataFrame(out)


def _agents_update(agents, rules, **numbers):
    """agents.update: the rules `attribute = expression`, in order (each sees the lines before), over the agents'
    attributes and the numbers given."""
    a = agents.copy()
    n = len(a)
    for line in _re.split(r"[\n;]", rules):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        name, eq, expr = line.partition("=")
        if not eq or expr.startswith("="):
            raise ValueError(f"{line!r}: give attribute = expression")
        name = name.strip()
        columns = {c: a[c].to_numpy(dtype="float64") for c in a.columns}
        v = _calc(expr.strip(), **{**columns, **_numbers_of(numbers)})["map"]
        a[name] = _np.broadcast_to(_np.asarray(v, dtype="float64"), (n,)).copy()
    return a


def _agents_map(agents, parcels, column):
    """agents.map: an attribute painted on the agents' parcels (the cell of the agent whose id is the parcel's)."""
    if "id" not in agents.columns:
        raise ValueError("the agents have no id")
    if column not in agents.columns:
        raise ValueError(f"the agents have no {column!r}")
    by_id = dict(zip(agents["id"].to_numpy(dtype="float64"), agents[column].to_numpy(dtype="float64")))
    p = _plane(parcels)
    return _like(parcels, _pd.Series(p.ravel()).map(by_id).to_numpy(dtype="float64").reshape(p.shape))


def _agents_summary(agents):
    """agents.summary: the number of agents and the sum and mean of each attribute (a row)."""
    columns, row = ["n"], [float(len(agents))]
    for c in agents.columns:
        v = agents[c].to_numpy(dtype="float64")
        v = v[~_np.isnan(v)]
        columns += [f"{c}_sum", f"{c}_mean"]
        row += [float(v.sum()), float(v.sum() / v.size) if v.size else _np.nan]
    return _pd.DataFrame([row], columns=columns)
