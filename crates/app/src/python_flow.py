# Terraflux's stacks, tables, features and containers in Python: stack.of and table.of, what control.each goes over
# and gathers, the case of control.switch as its branches name it; the tables' operators (filter, calc, aggregate,
# select, sort, join, pivot: tables as pandas' DataFrames) and the features' (GeoDataFrames) as Terraflux runs them.
import numbers as _numbers
import re as _re

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


# --- tables (pandas' DataFrames) ---


def _names(x):
    """A setting naming columns as a list: one name, names separated by commas, or a list (none: no name)."""
    if x is None:
        return []
    if isinstance(x, str):
        return [s.strip() for s in x.split(",") if s.strip()]
    return [str(s) for s in x]


def _has(table, names):
    """The names are columns of the table, or why not."""
    for c in names:
        if c not in table.columns:
            raise KeyError(f"the table has no column {c!r} (its columns: {', '.join(map(str, table.columns))})")


def _key(values):
    """A row's key of some columns, a missing value as None (equal to another missing one)."""
    return tuple(None if _pd.isna(v) else v for v in values)


_STATISTICS = ("sum", "mean", "count", "min", "max", "sd", "median")


def _statistic(stat, x):
    """A statistic of values, the missing ones left out (none left: missing; a count of none: 0)."""
    x = x[~_np.isnan(x)]
    if stat == "count":
        return float(x.size)
    if x.size == 0 or (stat == "sd" and x.size < 2):
        return _np.nan
    if stat == "sd":
        return float(_np.std(x, ddof=1))
    return float({"sum": _np.sum, "mean": _np.mean, "min": _np.min, "max": _np.max, "median": _np.median}[stat](x))


def _aggregate(table, by=None, columns=None, stats=None):
    """table.aggregate: a row per group of the values of `by` (in their order, missing values last; none: the whole
    table one group), its columns n (its rows) and column_statistic for every column and statistic (sum, mean, count,
    min, max, sd, median; sd the sample's, n - 1), the missing values left out of the statistics."""
    by, columns, stats = _names(by), _names(columns), _names(stats) or ["sum"]
    _has(table, by)
    if not columns:
        # every other column (of numbers: a table of Terraflux has only numbers)
        columns = [c for c in table.columns if c not in by and _pd.api.types.is_numeric_dtype(table[c])]
    _has(table, columns)
    for s in stats:
        if s not in _STATISTICS:
            raise ValueError(f"no statistic {s!r} (the statistics: {', '.join(_STATISTICS)})")
    t = table.reset_index(drop=True)
    values = {c: t[c].to_numpy(dtype="float64", na_value=_np.nan) for c in columns}
    groups = []
    if by:
        keys = list(zip(*(t[c].tolist() for c in by)))
        for i in t.sort_values(by, na_position="last", kind="stable").index:
            k = _key(keys[i])
            if groups and groups[-1][0] == k:
                groups[-1][2].append(i)
            else:
                groups.append((k, keys[i], [i]))
    else:
        groups.append(((), (), list(range(len(t)))))
    rows = []
    for _, key, members in groups:
        row = [*key, len(members)]
        for c in columns:
            x = values[c][_np.asarray(members, dtype=int)]
            row.extend(_statistic(s, x) for s in stats)
        rows.append(row)
    return _pd.DataFrame(rows, columns=[*by, "n", *(f"{c}_{s}" for c in columns for s in stats)])


def _select(table, columns=None, drop=None, rename=None):
    """table.select: the columns kept (in the order given; default every one), less those dropped, renamed."""
    keep, drop, rename = _names(columns) or list(table.columns), _names(drop), dict(rename or {})
    _has(table, drop)
    keep = [c for c in keep if c not in drop]
    _has(table, keep)
    for old in rename:
        if old not in keep:
            raise KeyError(f"no column {old!r} to rename among those kept")
    return _pd.DataFrame(table[keep]).rename(columns=rename).reset_index(drop=True)


def _sort(table, by, descending=False, top=None):
    """table.sort: the rows sorted by the columns `by` (the first, then the next where it ties; missing values last
    either way; rows of the same key in their order), the first `top` kept."""
    by = _names(by)
    if not by:
        raise ValueError("give the columns to sort by (by)")
    _has(table, by)
    out = table.sort_values(by, ascending=not descending, na_position="last", kind="stable")
    if top is not None:
        out = out.head(int(top))
    return out.reset_index(drop=True)


def _join(left, right, on, right_on=None, how="left", suffix="_right"):
    """table.join: every row of the left with each row of the right of the same key, in the left's order (left: the
    left's rows without one kept; inner: only those with one; full: the right's rows without one added after them); a
    missing value matches nothing. The right's other columns follow the left's, one of a name the left has with the
    suffix."""
    on = _names(on)
    right_on = _names(right_on) or on
    if not on:
        raise ValueError("give the key columns (on)")
    if len(right_on) != len(on):
        raise ValueError("on and right_on name as many columns")
    if how not in ("left", "inner", "full"):
        raise ValueError(f"how is left, inner or full, not {how!r}")
    _has(left, on)
    _has(right, right_on)
    left, right = left.reset_index(drop=True), right.reset_index(drop=True)

    def keys(t, cols):
        return [None if any(_pd.isna(v) for v in k) else k for k in zip(*(t[c].tolist() for c in cols))]

    index = {}
    for j, k in enumerate(keys(right, right_on)):
        if k is not None:
            index.setdefault(k, []).append(j)
    li, ri, matched = [], [], set()
    for i, k in enumerate(keys(left, on)):
        hits = index.get(k, []) if k is not None else []
        if not hits and how != "inner":
            li.append(i)
            ri.append(-1)
        for j in hits:
            li.append(i)
            ri.append(j)
            matched.add(j)
    extra = [j for j in range(len(right)) if j not in matched] if how == "full" else []
    out = left.reindex(li + [-1] * len(extra)).reset_index(drop=True)
    if extra:
        for a, b in zip(on, right_on):
            out[a] = _pd.concat([out[a].iloc[:len(li)], right[b].iloc[extra]], ignore_index=True)
    others = [c for c in right.columns if c not in right_on]
    side = right[others].reindex(ri + extra).reset_index(drop=True)
    side.columns = [f"{c}{suffix}" if c in left.columns else c for c in others]
    return _pd.concat([out, side], axis=1)


def _label(v):
    """A value as a column's name: a whole number without .0."""
    if isinstance(v, _numbers.Number) and not isinstance(v, bool) and float(v).is_integer() and abs(v) < 1e15:
        return str(int(v))
    return str(v)


def _common_prefix(names):
    """The start the names share, cut after its last character that is not a digit, . or - (to_ of to_1, to_12)."""
    if not names:
        return ""
    first, n = names[0], len(names[0])
    for x in names[1:]:
        k = 0
        while k < min(n, len(x)) and first[k] == x[k]:
            k += 1
        n = k
    p = first[:n]
    return p[:max((i + 1 for i, c in enumerate(p) if c not in "0123456789.-"), default=0)]


def _number_in(name, prefix):
    """The number in a column's name, after a prefix (none when there is none)."""
    try:
        return float((name[len(prefix):] if name.startswith(prefix) else name).strip())
    except ValueError:
        return None


def _pivot(table, direction, id=None, names=None, values=None, prefix=None, fill=None, columns=None,
           names_to="name", values_to="value"):
    """table.pivot, as tidyr's: wider, a row per value of the id columns (in their order), a column per value of the
    names column (the prefix, default the column's name and _, and the value) holding the values column (a
    combination absent: fill); longer, the columns given (default every one but the id ones) into rows of the names
    and the values, a column's name the number in it (after the prefix, default the one they share), else its place
    among them (1, 2...)."""
    ids = _names(id)
    _has(table, ids)
    t = table.reset_index(drop=True)
    if direction == "wider":
        if names is None or values is None:
            raise ValueError("wider needs the column of the names and the column of the values")
        _has(t, [names, values])
        prefix = f"{names}_" if prefix is None else prefix
        fill = _np.nan if fill is None else fill
        levels = sorted({v for v in t[names].tolist() if not _pd.isna(v)})
        if ids:
            keys = t[ids].drop_duplicates().sort_values(ids, na_position="last", kind="stable")
            keys = list(keys.itertuples(index=False, name=None))
        else:
            keys = [()] if len(t) else []
        of_rows = list(zip(*(t[c].tolist() for c in ids))) if ids else [()] * len(t)
        cell = {}
        for k, n, v in zip(of_rows, t[names].tolist(), t[values].tolist()):
            if not _pd.isna(n):
                cell.setdefault((_key(k), n), v)
        rows = [[*k, *(cell.get((_key(k), lv), fill) for lv in levels)] for k in keys]
        return _pd.DataFrame(rows, columns=[*ids, *(f"{prefix}{_label(lv)}" for lv in levels)])
    if direction == "longer":
        cols = _names(columns) or [c for c in t.columns if c not in ids]
        _has(t, cols)
        common = prefix if prefix else _common_prefix([str(c) for c in cols])
        labels = [_number_in(str(c), common) for c in cols]
        if any(x is None for x in labels):
            labels = [float(k) for k in range(1, len(cols) + 1)]
        out = {c: _np.repeat(t[c].to_numpy(), len(cols)) for c in ids}
        out[names_to] = _np.tile(_np.asarray(labels, dtype="float64"), len(t))
        out[values_to] = t[cols].to_numpy().ravel()
        return _pd.DataFrame(out)
    raise ValueError(f"the direction is wider or longer, not {direction!r}")


# --- map algebra over the rows of a table and the features of vectors (run by _calc, a value per row) ---

_WORDS = {"if", "else", "and", "or", "not", "out", "pi", "None", "True", "False"}
_QUOTED = r"""(?:"(?:[^"\\\n]|\\.)*"|'(?:[^'\\\n]|\\.)*')"""
_TOKEN = _re.compile(
    rf"(?P<text>{_QUOTED})|(?P<comment>#[^\n]*)|(?P<end>[;\n])|(?P<open>[(\[])|(?P<close>[)\]])"
    r"|(?P<geometry>(?<![\w.])geometry\s*\.\s*(?P<measure>[^\W\d]\w*))"
    rf"|(?P<float>(?<![\w.])float\s*\(\s*(?:(?P<ftext>{_QUOTED})|(?P<fname>[^\W\d]\w*))\s*\))"
    r"|(?P<name>(?<![\w.])[^\W\d]\w*)(?P<call>\s*\()?"
)


def _unquoted(t):
    """A text of the script as it is (a backslash keeps the character after it)."""
    return _re.sub(r"\\(.)", r"\1", t[1:-1], flags=_re.S)


def _script_name(name):
    """The name a script reads a column by: each character other than a letter, a digit or _ as _, and _ before a
    leading digit (individual-local-identifier as individual_local_identifier)."""
    s = "".join(c if c.isalnum() or c == "_" else "_" for c in str(name))
    return "_" + s if not s or s[0] in "0123456789" else s


def _is_text(column):
    return not (_pd.api.types.is_numeric_dtype(column) or _pd.api.types.is_bool_dtype(column))


def _parse(x):
    """A value as a number: a text read as one (none when it is not), a number as it is."""
    if x is None or (not isinstance(x, str) and _pd.isna(x)):
        return _np.nan
    if isinstance(x, str):
        try:
            return float(x.strip())
        except ValueError:
            return _np.nan
    return float(x)


def _measure(vector, m):
    """A measure of every feature's geometry, as vector.geometry gives it: the area and perimeter of polygons (their
    holes out of the area, their rings in the perimeter), the length of lines (0 for the others), the centroid's x
    and y (of the area, the length or the points)."""
    import shapely

    g = _np.asarray(vector.geometry.values)
    kind = shapely.get_type_id(g)
    polygons, lines = _np.isin(kind, (3, 6)), _np.isin(kind, (1, 5))
    if m == "area":
        return _np.where(polygons, shapely.area(g), 0.0)
    if m == "perimeter":
        return _np.where(polygons, shapely.length(g), 0.0)
    if m == "length":
        return _np.where(lines, shapely.length(g), 0.0)
    if m in ("x", "y"):
        c = shapely.centroid(g)
        return (shapely.get_x if m == "x" else shapely.get_y)(c)
    raise ValueError(f"the geometry has no measure {m!r} (its measures: area, perimeter, length, x, y)")


def _evaluate(frame, script, inputs=None, features=False):
    """A script of Terraflux's map algebra (Python's syntax) over the rows of a table or the features of vectors, run
    by _calc on a value per row: the columns by their names (the characters other than letters, digits and _ as _),
    feature (1..) and the geometry's measures (geometry.area...) of features, the other inputs (numbers and texts)
    by their names. Texts are computed as their places among every text the script meets, as Terraflux does:
    compared, chosen, tested (isnull: an empty text is none), counted, read as numbers by float(x). Its outputs by
    name (None: its last value, without out), a value per row each (texts as texts)."""
    import ast
    import keyword

    n = len(frame)
    geometry = frame.geometry.name if features else None
    # what the script reads by name: the inputs, feature, the columns (each over those before)
    readable = {}
    for k, x in (inputs or {}).items():
        if isinstance(x, _pd.DataFrame):
            continue
        if not isinstance(x, (str, _numbers.Number)):
            raise TypeError(f"{k} is neither a number nor a text (an expression's other inputs are)")
        readable[k] = ("input", x)
    if features and "feature" not in frame.columns:
        readable["feature"] = ("values", _np.arange(1, n + 1, dtype="float64"))
    columns = [c for c in frame.columns if c != geometry]
    taken = {str(c) for c in columns}
    for c in columns:
        s = _script_name(c)
        readable[s if s == str(c) or s not in taken else str(c)] = ("column", frame[c])
    alias = {k: f"_a{i}" for i, k in enumerate(readable)}
    # its statements (each ends at a new line outside brackets, or ;), as tokens
    statements, current, depth, pos = [], [], 0, 0
    for m in _TOKEN.finditer(script):
        current.append(("raw", script[pos:m.start()]))
        pos = m.end()
        if m["text"] is not None:
            current.append(("text", _unquoted(m["text"])))
        elif m["comment"] is not None:
            pass
        elif m["end"] is not None:
            if depth == 0:
                statements.append(current)
                current = []
            else:
                current.append(("raw", " "))
        elif m["open"] is not None or m["close"] is not None:
            depth += 1 if m["open"] is not None else -1
            current.append(("raw", m.group()))
        elif m["geometry"] is not None:
            current.append(("geometry", m["measure"]))
        elif m["float"] is not None:
            if m["ftext"] is not None:
                current.append(("number", _parse(_unquoted(m["ftext"]))))
            else:
                current.append(("float", m["fname"]))
        else:
            current.append(("name", m["name"], m["call"] is not None))
            if m["call"] is not None:
                current.append(("raw", m["call"]))
                depth += 1
    current.append(("raw", script[pos:]))
    statements.append(current)
    # every text it can meet, in order: its own, those of the columns and inputs it reads
    named = {t[1] for st in statements for t in st if t[0] in ("name", "float")} & readable.keys()
    texts = {t[1] for st in statements for t in st if t[0] == "text"}
    for k in named:
        how, x = readable[k]
        if how == "column" and _is_text(x):
            texts.update(str(v) for v in x if not _pd.isna(v))
        elif how == "input" and isinstance(x, str):
            texts.add(x)
    texts.discard("")
    texts = sorted(texts)
    place = {t: float(i) for i, t in enumerate(texts)}

    def code(t):
        return _np.nan if t is None or (not isinstance(t, str) and _pd.isna(t)) else place.get(str(t), _np.nan)

    # the values by the names of the script run, and which are texts
    values, kinds = {}, {}
    for k in named:
        how, x = readable[k]
        a = alias[k]
        if how == "column" and _is_text(x):
            values[a], kinds[a] = _np.array([code(v) for v in x], dtype="float64"), "text"
        elif how == "column":
            values[a] = x.to_numpy(dtype="float64", na_value=_np.nan)
        elif isinstance(x, str):
            values[a], kinds[a] = code(x), "text"
        else:
            values[a] = x if how == "values" else float(x)

    def ident(name):
        if name in alias:
            return alias[name]
        return "_k_" + name if keyword.iskeyword(name) and name not in _WORDS else name

    def unify(a, b):
        if {a, b} == {"text", "number"}:
            raise ValueError("its values are both texts or both numbers")
        return b if a == "missing" else a

    def kind(e):
        if isinstance(e, ast.Name):
            return kinds.get(e.id, "number")
        if isinstance(e, ast.Constant):
            return "missing" if e.value is None else "number"
        if isinstance(e, ast.IfExp):
            return unify(kind(e.body), kind(e.orelse))
        if isinstance(e, ast.Call) and getattr(e.func, "id", None) == "coalesce" and len(e.args) == 2:
            return unify(kind(e.args[0]), kind(e.args[1]))
        return "number"

    lines, outs, assigned, last = [], [], set(), "number"
    for st in statements:
        parts = []
        for t in st:
            if t[0] == "raw":
                parts.append(t[1])
            elif t[0] in ("text", "number"):
                a = f"_c{len(values)}"
                values[a] = code(t[1]) if t[0] == "text" else t[1]
                if t[0] == "text":
                    kinds[a] = "text"
                parts.append(a)
            elif t[0] == "geometry":
                if not features:
                    raise ValueError("geometry.<measure>: a table's rows have no geometry")
                a = f"_g_{t[1]}"
                if a not in values:
                    values[a] = _measure(frame, t[1])
                parts.append(a)
            elif t[0] == "float":
                w = ident(t[1])
                if w in assigned:
                    if kinds.get(w) == "text":
                        raise ValueError(f"float(x) reads a text attribute or a text input: {t[1]} is a text the "
                                         "script computes")
                    parts.append(w)
                elif t[1] in readable and kinds.get(w) == "text":
                    how, x = readable[t[1]]
                    a = f"_f{w}"
                    values[a] = _np.array([_parse(v) for v in x], dtype="float64") if how == "column" else _parse(x)
                    parts.append(a)
                else:
                    parts.append(w)
            else:
                parts.append(t[1] if t[2] or t[1] in _WORDS else ident(t[1]))
        line = "".join(parts).strip()
        if not line:
            continue
        lines.append(line)
        # the kinds of what it gives
        if line.startswith("out "):
            name, _, value = line[4:].partition("=")
            outs.append(name.strip())
            if not value.strip():
                last = kinds.get(name.strip(), "number")
                continue
            line = f"{name.strip()} = {value.strip()}"
        st = ast.parse(line).body[0]
        if isinstance(st, ast.Assign):
            target = st.targets[0]
            name = target.value.id if isinstance(target, ast.Subscript) else target.id
            k = kind(st.value)
            kinds[name] = unify(kinds.get(name, "number"), k) if isinstance(target, ast.Subscript) else k
            assigned.add(name)
            last = kinds[name]
        else:
            last = kind(st.value)
    given = _calc("\n".join(lines), **values)
    back = {a: k for k, a in alias.items()}

    def column(v, k):
        v = _np.array(_np.broadcast_to(_np.asarray(v, dtype="float64"), (n,)))
        if k != "text":
            return v
        return _np.array([texts[int(c)] if c == c and 0 <= c < len(texts) else None for c in v], dtype=object)

    if not outs:
        return {None: column(given["map"], last)}
    return {back.get(o, o[3:] if o.startswith("_k_") else o): column(given[o], kinds.get(o, "number")) for o in outs}


def _with(frame, columns):
    """The frame with columns (one of the same name replaced in its place, else added at the end)."""
    out = frame.copy()
    for k, v in columns.items():
        out[k] = v
    return out


def _kept(frame, given):
    """The rows (features) where an expression is true (missing: left out)."""
    if list(given) != [None]:
        raise ValueError("the rows kept are those where the script's last line is true: no out")
    keep = given[None]
    if keep.dtype == object:
        raise ValueError("the expression gives texts: a row is kept where it is true (uso == \"forest\")")
    return frame[~_np.isnan(keep) & (keep != 0)].reset_index(drop=True)


def _table_filter(table, expression, inputs=None):
    """table.filter: the rows where the expression is true (missing: left out)."""
    return _kept(table, _evaluate(table, expression, inputs))


def _table_calc(table, column, expression, inputs=None):
    """table.calc: a column computed row by row (one of the same name replaced, else added at the end)."""
    given = _evaluate(table, expression, inputs)
    if None not in given:
        raise ValueError("table.calc's expression is one value, without out")
    return _with(table, {column: given[None]})


def _features_filter(vector, expression, inputs=None):
    """vector.filter: the features where the expression (a script: its last line) is true (missing: left out)."""
    return _kept(vector, _evaluate(vector, expression, inputs, features=True))


def _features_calc(vector, expression, column=None, inputs=None):
    """vector.calc: the attributes a script gives (each out; without, its last value as `column`), one of the same
    name replaced, else added."""
    given = _evaluate(vector, expression, inputs, features=True)
    if None in given:
        if not column:
            raise ValueError("name the column, or give the attributes with out (out density = pop / geometry.area)")
        given = {column: given[None]}
    elif column:
        raise ValueError(f"the column {column!r} and a script with out: the out lines name the attributes")
    return _with(vector, given)


# --- vector features (geopandas' GeoDataFrames) ---


def _attributes(vector):
    """The features' attributes as a table: the feature (1..) and the attributes that are numbers."""
    out = _pd.DataFrame({"feature": _np.arange(1, len(vector) + 1)})
    for c in vector.columns:
        if c != vector.geometry.name and _pd.api.types.is_numeric_dtype(vector[c]):
            out[c] = vector[c].to_numpy()
    return out


def _measures(vector):
    """vector.geometry: the area and perimeter of polygons, the length of lines and the centroid (x, y)."""
    return _with(vector, {m: _measure(vector, m) for m in ("area", "perimeter", "length", "x", "y")})


def _geometries(vector):
    """The geometries of features as shapely's, in an array."""
    return _np.asarray(vector.geometry.values)


def _polygonal(geometries, what):
    """The geometries are polygons (or none), or why not."""
    for g in geometries:
        if g is not None and not g.is_empty and g.geom_type not in ("Polygon", "MultiPolygon"):
            raise ValueError(f"{what} is not polygons")


def _in_crs(other, vector):
    """Features in the coordinate system of others (reprojected when both are known and differ)."""
    if other.crs is not None and vector.crs is not None and other.crs != vector.crs:
        return other.to_crs(vector.crs)
    return other


def _extract(raster, points, **options):
    """vector.extract: the points as they are with the values of a map (value) or of every layer of a stack (by its
    name) at them, landschaft's extract; with a buffer, the statistics of the cells around each (count, mean, min,
    max and std by default)."""
    if options.get("buffer") is not None:
        options.setdefault("stats", ["count", "mean", "min", "max", "std"])
    got = ls.extract(raster, points.geometry, **options)
    new = [c for c in got.columns if c != got.geometry.name]
    if options.get("buffer") is None and len(new) == 1:
        got, new = got.rename(columns={new[0]: "value"}), ["value"]
    return _with(points, {c: got[c].to_numpy() for c in new})


def _buffer(vector, distance, segments=16, dissolve=False):
    """vector.buffer: the area within a distance of every feature (GEOS, segments to a quarter circle), its
    attributes kept; or every buffer merged into one feature (dissolve)."""
    import geopandas as gpd
    import shapely

    geometries = shapely.buffer(_geometries(vector), distance, quad_segs=int(segments))
    if dissolve:
        return gpd.GeoDataFrame(geometry=[shapely.union_all(geometries)], crs=vector.crs)
    out = vector.copy()
    out[vector.geometry.name] = geometries
    return out


def _clip(vector, mask):
    """vector.clip: each feature's part inside the mask (the union of its polygons, reprojected to the features),
    those outside it left out, their attributes kept."""
    import shapely

    mask = _in_crs(mask, vector)
    _polygonal(_geometries(mask), "the mask")
    cut = shapely.intersection(_geometries(vector), shapely.union_all(_geometries(mask)))
    keep = ~(shapely.is_empty(cut) | shapely.is_missing(cut))
    out = vector[keep].copy()
    out[vector.geometry.name] = cut[keep]
    return out.reset_index(drop=True)


def _dissolve(vector, by=None):
    """vector.dissolve: the features merged (GEOS's union), all into one or one per value of `by` (in the order of
    their first feature) with that value, and the features merged (count)."""
    import geopandas as gpd
    import shapely

    if by:
        if by not in vector.columns:
            raise KeyError(f"the features have no attribute {by!r}")
        codes = _pd.factorize(vector[by], use_na_sentinel=False)[0]
    else:
        codes = _np.zeros(len(vector), dtype=int)
    members = [_np.flatnonzero(codes == k) for k in range(codes.max() + 1 if len(codes) else 0)]
    data = {by: vector[by].to_numpy()[_np.array([m[0] for m in members], dtype=int)]} if by else {}
    data["count"] = [len(m) for m in members]
    geometries = [shapely.union_all(_geometries(vector)[m]) for m in members]
    return gpd.GeoDataFrame(data, geometry=geometries, crs=vector.crs)


def _reproject(vector, crs=None, like=None):
    """vector.reproject: the features in another coordinate system (PROJ): given, or of a map or features (like)."""
    if like is not None:
        crs = like.rio.crs if isinstance(like, _xr.DataArray) else like.crs
    if crs is None or (isinstance(crs, str) and not crs.strip()):
        raise ValueError("give the coordinate system (crs), or like")
    return vector.to_crs(crs)


def _join_features(vector, table, on=None, key=None):
    """vector.join: a table's columns as attributes of features, every feature taking the (first) row whose key
    column equals its attribute `on` (default its number 1..); the key by default zone, feature, plot_id or id if the
    table has one, else its first column; no value where no row matches; attributes of the same name replaced."""
    if key:
        _has(table, [key])
    else:
        key = next((k for k in ("zone", "feature", "plot_id", "id") if k in table.columns), table.columns[0])
    if on:
        if on not in vector.columns:
            raise KeyError(f"the features have no attribute {on!r}")
        of = [_parse(x) for x in vector[on]]
    else:
        of = [float(i) for i in range(1, len(vector) + 1)]
    t = table.reset_index(drop=True)
    first = {}
    for i, k in enumerate(_parse(x) for x in t[key]):
        if k == k:
            first.setdefault(k, i)
    side = t.drop(columns=[key]).reindex([first.get(x, -1) if x == x else -1 for x in of])
    return _with(vector, {c: side[c].to_numpy() for c in side.columns})


def _meets(a, b):
    """The pairs (i, j) of the features of a and b that meet (GEOS's intersects), by i then j."""
    if len(a) == 0 or len(b) == 0:
        return _np.zeros(0, dtype=int), _np.zeros(0, dtype=int)
    i, j = b.sindex.query(_geometries(a), predicate="intersects")
    order = _np.lexsort((j, i))
    return i[order], j[order]


def _joined(a, b, ia, ib, suffix):
    """The attributes of features made from those of two layers: a's, then b's (a name a has with the suffix), each
    from the feature of each it came from (-1: none, no value)."""
    left = _pd.DataFrame(a.drop(columns=a.geometry.name)).reset_index(drop=True).reindex(ia).reset_index(drop=True)
    right = _pd.DataFrame(b.drop(columns=b.geometry.name)).reset_index(drop=True).reindex(ib).reset_index(drop=True)
    right.columns = [f"{c}{suffix}" if c in left.columns else c for c in right.columns]
    return _pd.concat([left, right], axis=1)


def _spatial_join(vector, other, mode="attributes", fields=None, suffix="_2"):
    """vector.spatial_join: every feature with the attributes of the first feature of the other layer it meets
    (attributes), or the count of those it meets and the sums and means of their attributes `fields` (summary); the
    other layer reprojected to the first's."""
    import geopandas as gpd

    other = _in_crs(other, vector)
    i, j = _meets(vector, other)
    n = len(vector)
    if mode == "attributes":
        first = _np.full(n, -1)
        lead = _np.r_[True, i[1:] != i[:-1]] if len(i) else _np.zeros(0, dtype=bool)
        first[i[lead]] = j[lead]
        attributes = _joined(vector, other, list(range(n)), list(first), suffix)
        return gpd.GeoDataFrame(attributes, geometry=_geometries(vector), crs=vector.crs)
    if mode == "summary":
        columns = {"count": _np.bincount(i, minlength=n).astype("float64")}
        for f in _names(fields):
            if f not in other.columns:
                raise KeyError(f"the other layer has no attribute {f!r}")
            x = _np.array([_parse(v) for v in other[f]], dtype="float64")[j]
            ok = ~_np.isnan(x)
            sums = _np.bincount(i[ok], weights=x[ok], minlength=n)
            counts = _np.bincount(i[ok], minlength=n)
            columns[f"{f}_sum"] = sums
            columns[f"{f}_mean"] = _np.where(counts > 0, sums / _np.maximum(counts, 1), _np.nan)
        return _with(vector, columns)
    raise ValueError(f"mode {mode!r}: attributes or summary")


def _overlay(vector, other, how="intersection", suffix="_2"):
    """vector.overlay: two layers of polygons overlaid (GEOS): the intersection (a polygon where a feature of each
    meets, the attributes of both), the union (those, then the parts of each layer the other does not cover, with
    only their own attributes) or the difference (each feature of the first less the second's); the second layer
    reprojected to the first's."""
    import geopandas as gpd
    import shapely

    if how not in ("intersection", "union", "difference"):
        raise ValueError(f"how {how!r}: intersection, union or difference")
    other = _in_crs(other, vector)
    ga, gb = _geometries(vector), _geometries(other)
    _polygonal(ga, "the first layer")
    _polygonal(gb, "the second layer")
    i, j = _meets(vector, other)
    meets = [[] for _ in ga]
    for a, b in zip(i, j):
        meets[a].append(b)

    def polygonal(g):
        return g if g is not None and not g.is_empty and g.geom_type in ("Polygon", "MultiPolygon") else None

    def less(g, others):
        return g if not others else polygonal(shapely.difference(g, shapely.union_all(others)))

    geometries, ia, ib = [], [], []
    if how != "difference":
        for a, js in enumerate(meets):
            for b in js:
                g = polygonal(shapely.intersection(ga[a], gb[b]))
                if g is not None:
                    geometries.append(g)
                    ia.append(a)
                    ib.append(b)
    if how != "intersection":
        for a, js in enumerate(meets):
            g = less(ga[a], [gb[b] for b in js])
            if g is not None:
                geometries.append(g)
                ia.append(a)
                ib.append(-1)
    if how == "union":
        for b in range(len(gb)):
            g = less(gb[b], [ga[a] for a, js in enumerate(meets) if b in js])
            if g is not None:
                geometries.append(g)
                ia.append(-1)
                ib.append(b)
    if how == "difference":
        attributes = _pd.DataFrame(vector.drop(columns=vector.geometry.name)).iloc[ia].reset_index(drop=True)
    else:
        attributes = _joined(vector, other, ia, ib, suffix)
    return gpd.GeoDataFrame(attributes, geometry=geometries, crs=vector.crs)


def _rings(g):
    """The polygons of a geometry, each its rings (the outer first) as arrays of points."""
    if g is None or g.is_empty or g.geom_type not in ("Polygon", "MultiPolygon"):
        return []
    polygons = [g] if g.geom_type == "Polygon" else list(g.geoms)
    return [[_np.asarray(p.exterior.coords), *(_np.asarray(h.coords) for h in p.interiors)] for p in polygons]


def _in_ring(x, y, r):
    """Whether a point lies inside a ring (crossings of a ray to its right)."""
    ax, ay = r[:, 0], r[:, 1]
    bx, by = _np.roll(ax, 1), _np.roll(ay, 1)
    with _np.errstate(divide="ignore", invalid="ignore"):
        crossing = ((ay > y) != (by > y)) & (x < (bx - ax) * (y - ay) / (by - ay) + ax)
    return bool(crossing.sum() % 2)


def _inside(polygons, x, y):
    return any(_in_ring(x, y, p[0]) and not any(_in_ring(x, y, h) for h in p[1:]) for p in polygons)


def _points_inside(vector, method="random", n=10, spacing=None, seed=1):
    """vector.points, as Terraflux draws them: random, n points in every polygon drawn uniformly over its bounds and
    kept where inside (numpy's default_rng of the seed, x then y); regular, the centres of a grid of a spacing (from
    the polygons' lower left corner) inside each. Each point with its polygon's number (polygon), attributes, x, y."""
    import geopandas as gpd
    import shapely

    geometries = _geometries(vector)
    _polygonal(geometries, "the features")
    found = []
    if method == "random":
        rng, n = _np.random.default_rng(int(seed)), int(n)
        for i, g in enumerate(geometries):
            polygons = _rings(g)
            if not polygons or shapely.area(g) <= 0:
                continue
            x0, y0, x1, y1 = g.bounds
            got = tries = 0
            while got < n and tries < 1000 * max(n, 1):
                tries += 1
                x, y = x0 + rng.random() * (x1 - x0), y0 + rng.random() * (y1 - y0)
                if _inside(polygons, x, y):
                    found.append((i, x, y))
                    got += 1
    elif method == "regular":
        if spacing is None:
            raise ValueError("regular points: give the spacing")
        s = float(spacing)
        if s <= 0:
            raise ValueError("the spacing must be above 0")
        bx0, by0, bx1, by1 = vector.total_bounds
        if _np.isnan(bx0):
            raise ValueError("no polygons")
        nx, ny = int(_np.ceil((bx1 - bx0) / s)), int(_np.ceil((by1 - by0) / s))
        for i, g in enumerate(geometries):
            polygons = _rings(g)
            if not polygons:
                continue
            gx0, gy0, gx1, gy1 = g.bounds
            c0, c1 = int(_np.floor((gx0 - bx0) / s)), int(_np.ceil((gx1 - bx0) / s))
            r0, r1 = int(_np.floor((gy0 - by0) / s)), int(_np.ceil((gy1 - by0) / s))
            for r in range(r0, min(r1, ny)):
                for c in range(c0, min(c1, nx)):
                    x, y = bx0 + (c + 0.5) * s, by0 + (r + 0.5) * s
                    if _inside(polygons, x, y):
                        found.append((i, x, y))
    else:
        raise ValueError(f"method {method!r}: random or regular")
    of = _np.array([f[0] for f in found], dtype=int)
    x, y = _np.array([f[1] for f in found], dtype="float64"), _np.array([f[2] for f in found], dtype="float64")
    own = _pd.DataFrame(vector.drop(columns=[vector.geometry.name, *(["polygon"] if "polygon" in vector else [])]))
    attributes = _pd.concat([_pd.DataFrame({"polygon": of + 1}), own.iloc[of].reset_index(drop=True)], axis=1)
    attributes["x"], attributes["y"] = x, y
    return gpd.GeoDataFrame(attributes, geometry=gpd.points_from_xy(x, y), crs=vector.crs)


def _points_from(table, x="x", y="y", crs=None):
    """vector.from_table: a point per row with an x and a y, the other columns its attributes."""
    import geopandas as gpd

    _has(table, [x, y])
    xs, ys = (table[c].to_numpy(dtype="float64", na_value=_np.nan) for c in (x, y))
    keep = _np.isfinite(xs) & _np.isfinite(ys)
    rest = table.loc[keep, [c for c in table.columns if c not in (x, y)]].reset_index(drop=True)
    return gpd.GeoDataFrame(rest, geometry=gpd.points_from_xy(xs[keep], ys[keep]), crs=crs or None)


def _polygonize(raster, neighbours=4):
    """vector.polygonize, as gdal_polygonize (rasterio's shapes): a polygon per patch, the connected cells of one
    value (joined by their sides, 4, or also by their corners, 8), in the order of their first cell (row by row), its
    value, cells and area; the cells without a value in none."""
    import geopandas as gpd
    import shapely
    from rasterio import features
    from shapely.geometry import shape

    if int(neighbours) not in (4, 8):
        raise ValueError(f"neighbours {neighbours}: 4 or 8")
    a = _np.asarray(raster.values, dtype="float64")
    a = a.reshape(a.shape[-2:]) if a.ndim > 2 else a
    nodata = raster.rio.nodata
    valid = ~_np.isnan(a)
    if nodata is not None and not _np.isnan(nodata):
        valid &= a != nodata
    values, codes = _np.unique(a[valid], return_inverse=True)
    grid = _np.zeros(a.shape, dtype="int32")
    grid[valid] = codes
    t = raster.rio.transform()

    def to_map(xy):
        return _np.column_stack([t.c + xy[:, 0] * t.a + xy[:, 1] * t.b, t.f + xy[:, 0] * t.d + xy[:, 1] * t.e])

    found = []
    for g, k in features.shapes(grid, mask=valid, connectivity=int(neighbours)):
        p = shape(g)  # in columns and rows of the grid's corners
        ring = _np.asarray(p.exterior.coords)
        top = ring[:, 1].min()
        first = (top, ring[ring[:, 1] == top, 0].min())
        found.append((first, values[int(k)], round(p.area), shapely.transform(p, to_map)))
    found.sort(key=lambda f: f[0])
    cells = _np.array([f[2] for f in found], dtype="float64")
    return gpd.GeoDataFrame({"value": [f[1] for f in found], "cells": cells, "area": cells * abs(t.a * t.e)},
                            geometry=[f[3] for f in found], crs=raster.rio.crs)


def _distance(vector, like, all_touched=False):
    """vector.distance: the distance from every cell's centre to the nearest cell of the features as rasterize burns
    them, exactly (scipy's Euclidean distance transform, in the map's units; infinity without any)."""
    from scipy import ndimage

    burnt = ls.rasterize(vector, like, all_touched=all_touched)
    target = _np.asarray(burnt.values) != 0
    dx, dy = (abs(r) for r in burnt.rio.resolution())
    if target.any():
        d = ndimage.distance_transform_edt(~target, sampling=(dy, dx))
    else:
        d = _np.full(target.shape, _np.inf)
    return burnt.astype("float64").copy(data=d)


def _squares(raster, size):
    """vector.grid_zones: landschaft's squares tiling the map, numbered (zone), in its coordinate system."""
    import geopandas as gpd
    from shapely.geometry import shape

    zones = ls.grid_zones(raster, size)
    return gpd.GeoDataFrame({"zone": _np.arange(1, len(zones) + 1)}, geometry=[shape(z) for z in zones],
                            crs=raster.rio.crs)


# --- land change and prioritization (landschaft's simulate and prioritize) ---


def _by_class(probability):
    """glm.predict: landschaft's layer per class (class, y, x) as a stack, a layer named by its class."""
    return probability.rename({"class": "band"}).assign_coords(band=[_text(c) for c in probability["class"].values])


def _road_distance(network):
    """dinamica.roads' distance: the exact distance from every cell's centre to the roads (cells of a value other
    than 0), in map units; infinity without any."""
    from scipy import ndimage

    v = _np.asarray(network.values, dtype="float64")
    roads = ~_np.isnan(v) & (v != 0)
    if not roads.any():
        return network.astype("float64").copy(data=_np.full(v.shape, _np.inf))
    cell = abs(network.rio.resolution()[0])
    return network.astype("float64").copy(data=ndimage.distance_transform_edt(~roads) * cell)


def _stock_info(taken, quota):
    """dinamica.continuous_patcher's info: the quota, the stock taken, the quota left, patches and cells."""
    return _pd.DataFrame([[float(quota), float(_np.sum(_np.asarray(taken.taken))), taken.remaining,
                           float(taken.n_patches), float(taken.n_cells)]],
                         columns=["quota", "taken", "remaining", "patches", "cells"])


def _demand_table(spec):
    """demand.table: { classes, rows, times } as landschaft's demand (the times 0, 1... unless given)."""
    rows = [list(r) for r in spec["rows"]]
    times = spec.get("times") or list(range(len(rows)))
    if len(rows) < 2 or any(len(r) != len(spec["classes"]) for r in rows):
        raise ValueError("a demand needs the start and one step, each a value per class")
    return ls.simulate.AreaDemand(_pd.DataFrame(rows, index=_pd.Index(times, name="time"),
                                                columns=list(spec["classes"])))


def _unit_pairs(table, n):
    """prioritize.optimize's matrices of units: a table i, j, x (1-based, every unit) as a sparse n x n matrix
    (pairs given twice add up)."""
    from scipy import sparse

    i, j, x = (table[c].to_numpy(dtype="float64") for c in ("i", "j", "x"))
    for k in (i, j):
        bad = (k < 1) | (k > n) | (k != _np.floor(k))
        if bad.any():
            raise ValueError(f"{k[bad][0]} is no unit (1-{n})")
    return sparse.coo_matrix((x, (i.astype("int64") - 1, j.astype("int64") - 1)), shape=(n, n)).tocsc()
