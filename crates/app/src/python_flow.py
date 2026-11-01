# Terraflux's stacks, tables and containers in Python: stack.of and table.of, what control.each goes over and
# gathers, and the case of control.switch as its branches name it.
import numbers as _numbers

import numpy as _np
import pandas as _pd
import xarray as _xr


def _stack(layers):
    """Maps of one grid as a stack (band, y, x), a layer each named as its input, in the order of their names."""
    arrays = []
    for name, m in layers.items():
        if "band" in m.dims:
            m = m.squeeze("band", drop=True) if m.sizes["band"] == 1 else m.isel(band=0, drop=True)
        arrays.append(m.expand_dims(band=[name]))
    return _xr.concat(arrays, dim="band")


def _table(columns, rows):
    """A table of numbers written in the model."""
    return _pd.DataFrame([list(r) for r in rows], columns=list(columns))


def _text(x):
    if isinstance(x, bool):
        return "true" if x else "false"
    if isinstance(x, _numbers.Number) and float(x).is_integer():
        return str(int(x))
    return str(x)


def _elements(v):
    """The elements of a collection, by name, as control.each goes over them: the layers of a stack, the items of a
    list (named 1, 2...; values by themselves), the rows of a table (named by its first column), the features of
    vectors (named by their attribute `name`, else 1, 2...)."""
    if isinstance(v, _xr.DataArray) and "band" in v.dims:
        return [(_text(b), v.sel(band=b)) for b in v["band"].values]
    if isinstance(v, dict):
        return [(_text(k), x) for k, x in v.items()]
    if isinstance(v, _pd.DataFrame):
        if "geometry" in v.columns:
            names = v["name"] if "name" in v.columns and v["name"].dtype == object else None
            return [(str(names.iloc[i]) if names is not None else str(i + 1), v.iloc[[i]]) for i in range(len(v))]
        return [(_text(v.iloc[i, 0]), v.iloc[[i]]) for i in range(len(v))]
    items = list(v)
    if all(isinstance(x, (str, _numbers.Number)) for x in items):
        return [(_text(x), x) for x in items]
    return [(str(i + 1), x) for i, x in enumerate(items)]


def _gather(values, names, column):
    """What a loop over a collection collected: maps a stack of the elements' names, tables one table (a column of
    the element's name first), the rest a list."""
    if values and all(isinstance(x, _xr.DataArray) for x in values):
        shapes = {x.shape for x in values}
        if len(shapes) == 1:
            return _stack(dict(zip(names, values)))
    if values and all(isinstance(x, _pd.DataFrame) for x in values):
        parts = [x.assign(**{column: n}) for x, n in zip(values, names)]
        out = _pd.concat(parts, ignore_index=True)
        return out[[column] + [c for c in out.columns if c != column]]
    return list(values)


def _case(x):
    """A switch's case as its branches are named (a number without .0, true or false)."""
    return _text(x)
