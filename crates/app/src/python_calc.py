# Terraflux's calculator (raster.calc) in numpy: the same script, in Python's syntax, cell by cell, a missing
# value (NaN) making the result missing; `out name = value` names the node's outputs.
import ast as _ast
import warnings as _warnings

import numpy as _np

_NA = _np.nan


def _num(x):
    """A value as numbers: a map's cells (no data: NaN), a stack (band, y, x), a number as it is."""
    if hasattr(x, "dims") and hasattr(x, "values"):
        a = _np.asarray(x.values, dtype="float64")
        try:
            nodata = x.rio.nodata
        except Exception:
            nodata = None
        if nodata is not None and not _np.isnan(nodata):
            a = _np.where(a == nodata, _np.nan, a)
        return a
    if isinstance(x, (bool, int, float)):
        return float(x)
    return _np.asarray(x, dtype="float64")


def _cmp(a, op, b):
    a, b = _num(a), _num(b)
    with _np.errstate(invalid="ignore"):
        r = {"<": _np.less, "<=": _np.less_equal, ">": _np.greater, ">=": _np.greater_equal,
             "==": _np.equal, "!=": _np.not_equal}[op](a, b)
    return _np.where(_np.isnan(a) | _np.isnan(b), _np.nan, r.astype("float64"))


def _and(*xs):
    xs = [_num(x) for x in xs]
    missing = _np.any([_np.isnan(x) for x in _np.broadcast_arrays(*xs)], axis=0)
    true = _np.all([x != 0 for x in _np.broadcast_arrays(*xs)], axis=0)
    return _np.where(missing, _np.nan, true.astype("float64"))


def _or(*xs):
    xs = [_num(x) for x in xs]
    missing = _np.any([_np.isnan(x) for x in _np.broadcast_arrays(*xs)], axis=0)
    true = _np.any([(x != 0) & ~_np.isnan(x) for x in _np.broadcast_arrays(*xs)], axis=0)
    return _np.where(missing, _np.nan, true.astype("float64"))


def _not(x):
    x = _num(x)
    return _np.where(_np.isnan(x), _np.nan, (x == 0).astype("float64"))


def _if(c, a, b):
    c = _num(c)
    return _np.where(_np.isnan(c), _np.nan, _np.where(c != 0, _num(a), _num(b)))


def _layers(xs):
    """The maps of the arguments of a statistic (a stack: each of its layers)."""
    out = []
    for x in xs:
        a = _num(x)
        out.extend(list(a) if _np.ndim(a) == 3 else [a])
    return _np.stack(_np.broadcast_arrays(*out))


def _stat(f, *xs):
    with _warnings.catch_warnings():
        _warnings.simplefilter("ignore", RuntimeWarning)
        if f == "percentile":
            *xs, p = xs
            return _np.nanpercentile(_layers(xs), p, axis=0)
        a = _layers(xs)
        n = _np.sum(~_np.isnan(a), axis=0)
        if f == "count":
            return n.astype("float64")
        if f == "sum":
            return _np.where(n > 0, _np.nansum(a, axis=0), _np.nan)
        if f == "sd":
            return _np.where(n > 1, _np.nanstd(a, axis=0, ddof=1), _np.nan)
        return {"mean": _np.nanmean, "median": _np.nanmedian, "min": _np.nanmin, "max": _np.nanmax}[f](a, axis=0)


def _reduce(f, x, *args):
    """A number from every cell with a value: x.mean(), x.percentile(90)."""
    a = _num(x).ravel()
    a = a[~_np.isnan(a)]
    if a.size == 0:
        return _np.nan
    if f == "percentile":
        return float(_np.percentile(a, args[0]))
    if f == "sd":
        return float(_np.std(a, ddof=1)) if a.size > 1 else _np.nan
    return float({"mean": _np.mean, "median": _np.median, "sum": _np.sum, "count": _np.size,
                  "min": _np.min, "max": _np.max}[f](a))


def _nb(f, x, r):
    from scipy import ndimage

    a, size = _num(x), 2 * int(r) + 1
    valid = ~_np.isnan(a)
    if f in ("nbmin", "nbmax"):
        fill = _np.inf if f == "nbmin" else -_np.inf
        g = ndimage.minimum_filter if f == "nbmin" else ndimage.maximum_filter
        out = g(_np.where(valid, a, fill), size=size, mode="constant", cval=fill)
        return _np.where(_np.isinf(out), _np.nan, out)
    n = ndimage.uniform_filter(valid.astype("float64"), size=size, mode="constant") * size * size
    s = ndimage.uniform_filter(_np.where(valid, a, 0.0), size=size, mode="constant") * size * size
    n = _np.round(n)
    if f == "nbcount":
        return n
    with _np.errstate(invalid="ignore", divide="ignore"):
        return _np.where(n > 0, s if f == "nbsum" else s / n, _np.nan)


def _round(x):
    x = _num(x)
    return _np.sign(x) * _np.floor(_np.abs(x) + 0.5)


def _extreme(f, *xs):
    if len(xs) == 1:
        return _stat(f, *xs)
    out = _num(xs[0])
    for x in xs[1:]:
        out = (_np.fmin if f == "min" else _np.fmax)(out, _num(x))
    return out


_FUNCTIONS = {
    "abs": lambda x: _np.abs(_num(x)),
    "sqrt": lambda x: _np.sqrt(_num(x)),
    "exp": lambda x: _np.exp(_num(x)),
    "log": lambda x: _np.log(_num(x)),
    "log10": lambda x: _np.log10(_num(x)),
    "floor": lambda x: _np.floor(_num(x)),
    "ceil": lambda x: _np.ceil(_num(x)),
    "round": _round,
    "min": lambda *x: _extreme("min", *x),
    "max": lambda *x: _extreme("max", *x),
    "clamp": lambda x, lo, hi: _np.clip(_num(x), lo, hi),
    "isnull": lambda x: _np.isnan(_num(x)).astype("float64"),
    "coalesce": lambda x, y: _np.where(_np.isnan(_num(x)), _num(y), _num(x)),
    "nbsum": lambda x, r: _nb("nbsum", x, r),
    "nbmean": lambda x, r: _nb("nbmean", x, r),
    "nbmin": lambda x, r: _nb("nbmin", x, r),
    "nbmax": lambda x, r: _nb("nbmax", x, r),
    "nbcount": lambda x, r: _nb("nbcount", x, r),
    "mean": lambda *x: _stat("mean", *x),
    "sd": lambda *x: _stat("sd", *x),
    "median": lambda *x: _stat("median", *x),
    "sum": lambda *x: _stat("sum", *x),
    "count": lambda *x: _stat("count", *x),
    "percentile": lambda *x: _stat("percentile", *x),
    "pi": _np.pi,
}
_REDUCTIONS = {"mean", "sd", "median", "sum", "count", "min", "max", "percentile"}


class _Cells(_ast.NodeTransformer):
    """The script's expressions made cell by cell (comparisons, and/or/not, a if c else b, None)."""

    def _call(self, name, args):
        return _ast.Call(func=_ast.Name(id=name, ctx=_ast.Load()), args=args, keywords=[])

    def visit_Compare(self, node):
        self.generic_visit(node)
        parts, left = [], node.left
        for op, right in zip(node.ops, node.comparators):
            sign = {_ast.Lt: "<", _ast.LtE: "<=", _ast.Gt: ">", _ast.GtE: ">=", _ast.Eq: "==", _ast.NotEq: "!="}
            parts.append(self._call("_cmp", [left, _ast.Constant(sign[type(op)]), right]))
            left = right
        return parts[0] if len(parts) == 1 else self._call("_and", parts)

    def visit_BoolOp(self, node):
        self.generic_visit(node)
        return self._call("_and" if isinstance(node.op, _ast.And) else "_or", node.values)

    def visit_UnaryOp(self, node):
        self.generic_visit(node)
        return self._call("_not", [node.operand]) if isinstance(node.op, _ast.Not) else node

    def visit_IfExp(self, node):
        self.generic_visit(node)
        return self._call("_if", [node.test, node.body, node.orelse])

    def visit_Constant(self, node):
        return _ast.Name(id="_NA", ctx=_ast.Load()) if node.value is None else node

    def visit_Call(self, node):
        self.generic_visit(node)
        f = node.func
        if isinstance(f, _ast.Attribute) and f.attr in _REDUCTIONS:
            return self._call("_reduce", [_ast.Constant(f.attr), f.value, *node.args])
        return node


def _calc(script, **inputs):
    """Run a calculator's script on its inputs; its outputs by name (`map`: the last value, without `out`)."""
    like = next((v for v in inputs.values() if hasattr(v, "dims")), None)
    names = {k: _num(v) for k, v in inputs.items()}
    outs, lines = [], []
    for line in script.replace(";", "\n").splitlines():
        text = line.split("#", 1)[0].strip()
        if not text:
            continue
        if text.startswith("out "):
            name, _, value = text[4:].partition("=")
            outs.append(name.strip())
            text = f"{name.strip()} = {value.strip()}" if value.strip() else ""
        if text:
            lines.append(text)
    tree = _ast.parse("\n".join(lines))
    body = []
    for st in tree.body:
        if isinstance(st, _ast.Assign) and isinstance(st.targets[0], _ast.Subscript):
            # name[where] = value: the cells where it holds, the others keeping theirs
            t = st.targets[0]
            st = _ast.Assign(
                targets=[_ast.Name(id=t.value.id, ctx=_ast.Store())],
                value=_ast.Call(func=_ast.Name(id="_set", ctx=_ast.Load()), args=[t.value, t.slice, st.value],
                                keywords=[]),
            )
        elif isinstance(st, _ast.Expr):
            st = _ast.Assign(targets=[_ast.Name(id="map", ctx=_ast.Store())], value=st.value)
        body.append(st)
    if not outs:
        last = body[-1]
        outs = [last.targets[0].id] if isinstance(last, _ast.Assign) else ["map"]
        if outs != ["map"]:
            body.append(_ast.parse(f"map = {outs[0]}").body[0])
            outs = ["map"]
    tree = _ast.fix_missing_locations(_Cells().visit(_ast.Module(body=body, type_ignores=[])))
    shape = next((_np.shape(v)[-2:] for v in names.values() if _np.ndim(v) >= 2), ())

    def _set(target, where, value):
        out = _np.array(_np.broadcast_to(_num(target), shape), dtype="float64")
        w = _num(where)
        mask = _np.broadcast_to((w != 0) & ~_np.isnan(w), shape)
        out[mask] = _np.broadcast_to(_num(value), shape)[mask]
        return out

    scope = {**_FUNCTIONS, "_cmp": _cmp, "_and": _and, "_or": _or, "_not": _not, "_if": _if, "_NA": _NA,
             "_reduce": _reduce, "_set": _set, "True": 1.0, "False": 0.0, **names}
    with _np.errstate(all="ignore"), _warnings.catch_warnings():
        _warnings.simplefilter("ignore", RuntimeWarning)
        exec(compile(tree, "<raster.calc>", "exec"), scope)

    def back(v):
        if like is not None and _np.ndim(v) >= 2 and _np.shape(v)[-2:] == like.shape[-2:]:
            v = _np.broadcast_to(v, like.shape) if _np.ndim(v) < like.ndim else v
            out = like.copy(data=_np.asarray(v, dtype="float64"))
            try:
                out = out.rio.write_nodata(_np.nan)
            except Exception:
                pass
            return out
        return float(v) if _np.ndim(v) == 0 else v

    return {k: back(scope[k]) for k in outs}
