"""The metrics catalog: website/data/metrics/*.yaml checked against the library, run on an example map, and written as
website/data/metrics.json (committed, so the site's build does not need the library).

    python website/tools/one_at_a_time.py -- python website/tools/metrics_catalog.py
    python website/tools/one_at_a_time.py -- python website/tools/metrics_catalog.py --strict   # fail on uncovered metrics
    python website/tools/one_at_a_time.py -- python website/tools/metrics_catalog.py --check    # write nothing; fail if
                                                                                                 # metrics.json differs

PYTHONPATH=extern/terraflux-py/src serves when the library is not installed (the schema is in AUTHORING.md, "The
metrics catalog"). One YAML file per family; this script is the only reader of them. For every entry it

  * checks the keys (unknown or missing ones fail), the types, the ids (unique site-wide) and the operator and the
    API path it names (they must exist in data/ops.json and data/python_api.json);
  * checks it against the library's own list (`library: lsm` or `surface`): an entry the library does not have fails;
    a metric the library has that no entry covers is only reported (a list, per family), and fails with --strict;
  * runs the entry's Python call on the family's example map (`example:`) and stores the value; a call that raises fails.

The library's names, calls and values for the metrics of a `library:` are made by a Lib class below: a new family of a
library the catalog does not know yet adds one (or writes every entry's `python:` and has no `library:`).
"""

from __future__ import annotations

import argparse
import ast
import json
import math
import os
import pathlib
import re
import sys
import traceback

import yaml

HERE = pathlib.Path(__file__).resolve().parent
WEB = HERE.parent
sys.path.insert(0, str(WEB))

for _v in ("OMP_NUM_THREADS", "RAYON_NUM_THREADS", "TERRAFLUX_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS"):
    os.environ.setdefault(_v, "4")

from lib.pyapi import resolve_path  # noqa: E402

SRC = WEB / "data" / "metrics"
OUT = WEB / "data" / "metrics.json"
CONTENT = WEB / "content" / "metrics"

FAMILY_REQUIRED = ("family", "title", "order", "source", "intro", "example", "metrics")
FAMILY_OPTIONAL = ("library", "prefix", "data")
ENTRY_REQUIRED = ("id", "name", "level", "theme", "unit", "range", "what", "formula", "symbols", "use")
ENTRY_OPTIONAL = ("python", "operator", "setting", "refs", "aliases", "variants", "api", "listed", "options",
                  "variant_base")

# The summary statistics of a patch metric at the class and landscape level (the library's `_mn`, `_sd`...), as the
# library computes them over the patches of a class (or of the landscape): label, definition.
LSM_STATS = {
    "mn": ("mean", r"\bar{x} = \frac{1}{n}\sum_{i=1}^{n} x_i"),
    "sd": ("standard deviation", r"s = \sqrt{\frac{1}{n-1}\sum_{i=1}^{n} (x_i - \bar{x})^2}"),
    "cv": ("coefficient of variation, in %", r"\mathrm{CV} = 100\, s / \bar{x}"),
    "am": ("area-weighted mean", r"\frac{\sum_{i=1}^{n} a_i x_i}{\sum_{i=1}^{n} a_i}"),
    "md": ("median", r"\mathrm{median}(x_1, \dots, x_n)"),
    "ra": ("range", r"\max_i x_i - \min_i x_i"),
}
LSM_STAT_TEXT = {
    "mn": "the mean over the patches", "sd": "the sample standard deviation over the patches (n - 1)",
    "cv": "the standard deviation as a percentage of the mean", "am": "the mean weighted by patch area",
    "md": "the median", "ra": "the range (largest minus smallest)",
}
LSM_LEVEL = {"p": "patch", "c": "class", "l": "landscape"}


class CatalogError(Exception):
    pass


def num(v) -> str:
    """A number as a short text: four significant digits, no trailing zeros, no exponent."""
    if v is None or (isinstance(v, float) and math.isnan(v)):
        return "NA"
    if float(v).is_integer() and abs(float(v)) < 1e15:
        return str(int(float(v)))  # a count stays exact: 264641, not 264600
    f = float(f"{float(v):.4g}")
    s = f"{f:.10f}".rstrip("0").rstrip(".")
    return "0" if s in ("-0", "") else s


# ------------------------------------------------------------------------------------------------ libraries


class Lib:
    """What the catalog knows of the metrics of one library function family."""

    name = ""

    def keys(self) -> set[str]:
        raise NotImplementedError

    def call(self, key: str, data: str, options: str | None = None) -> str:
        """The call that computes `key` on `data`; `options`: more keyword arguments, as text ("search_radius=300")."""
        raise NotImplementedError

    def operator(self, key: str) -> tuple[str, str] | None:
        return None

    def api(self, key: str) -> str | None:
        return None

    def variants(self, key: str, suffixes: list[str], base: str | None = None) -> list[dict]:
        return []

    def value(self, key: str, result) -> str:
        return generic_value(result)

    def level(self, key: str) -> str | None:
        return None


class LsmLib(Lib):
    """tf.landscape_metrics: the keys are landscapemetrics' function names (lsm_c_ca...), extras included."""

    name = "lsm"

    def __init__(self, tf):
        self.tf = tf
        self.table = tf.lsm.list_metrics(extras=True)
        self._keys = set(self.table["function_name"])

    def keys(self):
        return self._keys

    def level(self, key):
        return LSM_LEVEL[key.split("_")[1]]

    def call(self, key, data, options=None):
        more = f", {options}" if options else ""
        return f'tf.landscape_metrics({data}, what=["{key}"]{more})'

    def operator(self, key):
        _, lv, metric = key.split("_", 2)
        return f"metrics.{LSM_LEVEL[lv]}", f'metrics = ["{metric}"]'

    def api(self, key):
        return "terraflux.landscape_metrics"

    def variants(self, key, suffixes, base=None):
        """The class- and landscape-level statistics of a patch metric `lsm_p_<base>` the library has (`base`: their
        name when it is not the patch metric's, as landscapemetrics' `dcore_*`, the statistics of `ncore`)."""
        parts = key.split("_", 2)
        if parts[1] != "p":
            raise CatalogError(f"{key}: `variants` are the statistics of a patch-level metric (lsm_p_...)")
        out = []
        for lv in ("c", "l"):
            for s in suffixes:
                if s not in LSM_STATS:
                    raise CatalogError(f"{key}: unknown variant {s!r} (the library has {', '.join(LSM_STATS)})")
                k = f"lsm_{lv}_{base or parts[2]}_{s}"
                if k in self._keys:
                    out.append({"id": k, "stat": s, "level": LSM_LEVEL[lv]})
        return out

    def value(self, key, result):
        df = result
        level = LSM_LEVEL[key.split("_")[1]]
        rows = df[df["level"] == level]
        if level == "landscape":
            return num(rows["value"].iloc[0])
        if level == "class":
            return ", ".join(f"class {c}: {num(v)}" for c, v in zip(rows["class"], rows["value"]))
        vals = [num(v) for v in rows["value"].iloc[:4]]
        return f"{len(rows)} patches; the first four: {', '.join(vals)}"


class SurfaceLib(Lib):
    """tf.surface: the keys are the names of tf.surface.list_metrics() (sa, sq...)."""

    name = "surface"

    def __init__(self, tf):
        self.tf = tf
        self._keys = set(tf.surface.list_metrics()["metric"])

    def keys(self):
        return self._keys

    def level(self, key):
        return "map"

    def call(self, key, data, options=None):
        more = f", {options}" if options else ""
        if hasattr(self.tf.surface, key):
            return f"tf.surface.{key}({data}{more})"
        return f'tf.surface.metrics({data}, metrics=["{key}"]{more})["{key}"]'

    def operator(self, key):
        return "surface.metrics", f'metrics = ["{key}"]'

    def api(self, key):
        return f"terraflux.surface.{key}" if key and hasattr(self.tf.surface, key) else "terraflux.surface.metrics"


def generic_value(r) -> str:
    """A call's result as text: a number rounded, a table or a series as its lines."""
    import numpy as np
    import pandas as pd

    if isinstance(r, (bool, str)):
        return str(r)
    if isinstance(r, (int, float, np.integer, np.floating)):
        return num(r)
    if isinstance(r, pd.Series) and len(r) > 12 and pd.api.types.is_numeric_dtype(r):
        r = r.to_numpy()  # a long column: its summary below, not one line per row
    elif isinstance(r, pd.Series):
        return "\n".join(f"{k}  {num(v) if isinstance(v, (int, float, np.number)) else v}" for k, v in r.items())
    if isinstance(r, pd.DataFrame):
        return r.round(4).to_string(index=False)
    if isinstance(r, tuple) and len(r) <= 4:
        return "; ".join(generic_value(x) for x in r)
    # a map (numpy array, xarray DataArray, a raster): its summary, not its repr
    a = None
    if isinstance(r, np.ndarray):
        a = r
    elif hasattr(r, "values") and isinstance(getattr(r, "values"), np.ndarray):
        a = r.values
    elif hasattr(r, "__array__") and not isinstance(r, (list, dict)):
        try:
            a = np.asarray(r)
        except Exception:  # noqa: BLE001 - not an array after all
            a = None
    if a is not None and a.dtype.kind in "biuf":
        if a.ndim <= 1 and a.size <= 6:
            return ", ".join(num(v) for v in a.ravel())
        f = a.astype("float64", copy=False)
        ok = np.isfinite(f)
        what, gap = ("a map", "cells without a value") if a.ndim >= 2 else (f"{a.size} values", "missing")
        if not ok.any():
            return f"{what}, none with a value"
        text = f"{what}: mean {num(f[ok].mean())}, from {num(f[ok].min())} to {num(f[ok].max())}"
        missing = int(a.size - ok.sum())
        return text + (f"; {missing} {gap}" if missing else "")
    return str(r)


# ------------------------------------------------------------------------------------------------ checks


def need_str(path: str, d: dict, key: str, errors: list[str], allow_list=False):
    v = d.get(key)
    if allow_list and isinstance(v, list):
        if not v or not all(isinstance(x, str) and x.strip() for x in v):
            errors.append(f"{path}: `{key}` must be a text or a list of non-empty texts")
    elif not isinstance(v, str) or not v.strip():
        errors.append(f"{path}: `{key}` must be a non-empty text")


def check_family(f: dict, fname: str, errors: list[str]):
    if not isinstance(f, dict):
        errors.append(f"{fname}: the file must be a mapping")
        return False
    for k in f:
        if k not in FAMILY_REQUIRED + FAMILY_OPTIONAL:
            errors.append(f"{fname}: unknown key `{k}` (known: {', '.join(FAMILY_REQUIRED + FAMILY_OPTIONAL)})")
    for k in FAMILY_REQUIRED:
        if k not in f:
            errors.append(f"{fname}: missing `{k}`")
    if errors:
        return False
    for k in ("family", "title", "source", "intro", "example"):
        need_str(fname, f, k, errors)
    if not isinstance(f["order"], int):
        errors.append(f"{fname}: `order` must be a whole number")
    if f.get("family") and f["family"] != pathlib.Path(fname).stem:
        errors.append(f"{fname}: `family` is {f['family']!r}, the file's name is {pathlib.Path(fname).stem!r}")
    if not isinstance(f["metrics"], list) or not f["metrics"]:
        errors.append(f"{fname}: `metrics` must be a non-empty list")
        return False
    return True


def check_entry(e, where: str, f: dict, errors: list[str]) -> bool:
    if not isinstance(e, dict):
        errors.append(f"{where}: an entry must be a mapping")
        return False
    where = f"{where} {e.get('id', '?')}"
    n0 = len(errors)
    for k in e:
        if k not in ENTRY_REQUIRED + ENTRY_OPTIONAL:
            errors.append(f"{where}: unknown key `{k}` (known: {', '.join(ENTRY_REQUIRED + ENTRY_OPTIONAL)})")
    for k in ENTRY_REQUIRED:
        if k not in e:
            errors.append(f"{where}: missing `{k}`")
    if len(errors) > n0:
        return False
    for k in ("id", "name", "level", "theme", "unit", "range", "what", "use"):
        need_str(where, e, k, errors)
    need_str(where, e, "formula", errors, allow_list=True)
    if not re.fullmatch(r"[A-Za-z][\w-]*", str(e["id"])):
        errors.append(f"{where}: `id` is an anchor: letters, digits, _ and - only")
    sym = e["symbols"]
    if not isinstance(sym, dict) or not all(isinstance(k, str) and isinstance(v, str) and v.strip() for k, v in sym.items()):
        errors.append(f"{where}: `symbols` must be a mapping from a LaTeX symbol to its meaning")
    for k in ("options", "variant_base"):
        if k in e and not f.get("library"):
            errors.append(f"{where}: `{k}` is for the generated calls of a family with a `library:`")
    if "options" in e and "python" in e:
        errors.append(f"{where}: `options` add to the generated call; with your own `python:` write them there")
    for k in ("python", "operator", "setting", "api", "options", "variant_base"):
        if k in e and (not isinstance(e[k], str) or not e[k].strip()):
            errors.append(f"{where}: `{k}` must be a non-empty text")
    for k in ("refs", "aliases", "variants"):
        if k in e and (not isinstance(e[k], list) or not all(isinstance(x, str) and x.strip() for x in e[k])):
            errors.append(f"{where}: `{k}` must be a list of texts")
    if "listed" in e and (e["listed"] is not False or "python" not in e or not f.get("library")):
        errors.append(f"{where}: `listed: false` (and only false) marks, in a family with a `library:`, a metric its "
                      "list does not hold; it needs its own `python:`")
    if "setting" in e and "operator" not in e:
        errors.append(f"{where}: `setting` needs `operator` (the operator it is a setting of)")
    if not f.get("library") and "python" not in e:
        errors.append(f"{where}: the family has no `library:`, so the entry needs its own `python:`")
    return len(errors) == n0


def check_operator(where: str, op: str, setting: str | None, ops: dict, errors: list[str]):
    if op not in ops:
        errors.append(f"{where}: the operator {op!r} is not in data/ops.json")
        return
    if setting:
        name = re.match(r"\s*([A-Za-z_]\w*)\s*=", setting)
        have = {p["name"] for p in ops[op]["params"]}
        if not name or name.group(1) not in have:
            errors.append(f"{where}: `setting` ({setting!r}) must start with a setting of {op} ({', '.join(sorted(have))})")


def run(code: str, ns: dict):
    """Run a snippet in `ns`; the value of its last expression, else of `result`."""
    tree = ast.parse(code)
    last = None
    if tree.body and isinstance(tree.body[-1], ast.Expr):
        last = ast.Expression(tree.body.pop().value)
    exec(compile(tree, "<metric>", "exec"), ns)
    if last is not None:
        return eval(compile(last, "<metric>", "eval"), ns)
    if "result" in ns:
        return ns["result"]
    raise CatalogError("the snippet has no final expression and defines no `result`")


# ------------------------------------------------------------------------------------------------ main


def build(only=None):
    import terraflux as tf

    ops = {o["name"]: o for o in json.loads((WEB / "data" / "ops.json").read_text(encoding="utf-8"))}
    api = json.loads((WEB / "data" / "python_api.json").read_text(encoding="utf-8"))
    libs = {"lsm": LsmLib(tf), "surface": SurfaceLib(tf)}
    errors: list[str] = []
    families = []
    seen_ids: dict[str, str] = {}
    coverage = {}

    files = sorted(f for f in SRC.glob("*.yaml") if not only or f.stem in only)
    if not files:
        raise CatalogError(f"no family in {SRC}")
    for p in files:
        fname = p.relative_to(WEB).as_posix()
        try:
            f = yaml.safe_load(p.read_text(encoding="utf-8"))
        except yaml.YAMLError as ex:
            errors.append(f"{fname}: not valid YAML (a LaTeX key with a backslash goes in single quotes): {ex}")
            continue
        if not check_family(f, fname, errors):
            continue
        lib = libs.get(f.get("library")) if f.get("library") else None
        if f.get("library") and lib is None:
            errors.append(f"{fname}: unknown `library: {f['library']}` (known: {', '.join(libs)})")
            continue
        prefix = f.get("prefix", "")
        data = f.get("data", "z" if lib and lib.name == "surface" else "landscape")
        page = CONTENT / f"{f['family']}.md"
        if not page.is_file():
            errors.append(f"{fname}: no page {page.relative_to(WEB).as_posix()} (the directive `::: metrics {f['family']}`)")
        elif f["example"].strip() not in page.read_text(encoding="utf-8"):
            errors.append(f"{fname}: the page's Python example does not contain the family's `example:` code as it is")
        ns: dict = {}
        try:
            exec(f["example"], ns)
        except Exception as ex:  # noqa: BLE001
            errors.append(f"{fname}: the example map's code raises: {type(ex).__name__}: {ex}")
            continue
        covered: set[str] = set()
        metrics = []
        for e in f["metrics"]:
            if not check_entry(e, fname, f, errors):
                continue
            where = f"{fname}: {e['id']}"
            if e["id"] in seen_ids:
                errors.append(f"{where}: duplicate id (also in {seen_ids[e['id']]})")
                continue
            seen_ids[e["id"]] = fname
            out = {k: e[k] for k in ENTRY_REQUIRED}
            out.update({k: e[k] for k in ("refs", "aliases") if k in e})
            key = None
            if lib:
                key = e["id"][len(prefix):] if e["id"].startswith(prefix) else None
                if key is None or key not in lib.keys():
                    if e.get("listed") is False and e.get("python"):
                        key = None  # a metric of the library its list does not hold: its own call
                    else:
                        errors.append(f"{where}: the library has no metric {key or e['id']!r} (family `library: "
                                      f"{lib.name}`; an entry outside the library's list says `listed: false` and gives "
                                      f"its `python:`)")
                        continue
                if key is not None:
                    covered.add(key)
                    if lib.level(key) and lib.level(key) != e["level"]:
                        errors.append(f"{where}: `level` is {e['level']!r}, the library's is {lib.level(key)!r}")
            code = e.get("python") or (lib.call(key, data, e.get("options")) if lib else None)
            out["python"] = code
            op = lib.operator(key) if lib and key is not None else None
            operator = e.get("operator") or (op[0] if op else None)
            setting = e.get("setting") or (op[1] if op and not e.get("operator") else None)
            if operator:
                check_operator(where, operator, setting, ops, errors)
                out["operator"], out["setting"] = operator, setting
            apipath = e.get("api") or (lib.api(key) if lib and key is not None else None)
            if apipath:
                if not resolve_path(apipath, api["objects"], api["aliases"]):
                    errors.append(f"{where}: no object {apipath!r} in data/python_api.json")
                out["api"] = apipath
            try:
                result = run(code, dict(ns))
                out["value"] = lib.value(key, result) if lib and not e.get("python") else generic_value(result)
            except Exception as ex:  # noqa: BLE001
                errors.append(f"{where}: the call raises: {type(ex).__name__}: {ex}\n    {code}")
                continue
            variants = []
            if e.get("variants"):
                if not lib:
                    errors.append(f"{where}: `variants` need a family `library:`")
                    continue
                try:
                    vs = lib.variants(key, e["variants"], e.get("variant_base"))
                except CatalogError as ex:
                    errors.append(f"{where}: {ex}")
                    continue
                for v in vs:
                    if v["id"] in seen_ids:
                        errors.append(f"{where}: variant {v['id']} is also an id in {seen_ids[v['id']]}")
                        continue
                    seen_ids[v["id"]] = fname
                    covered.add(v["id"])
                    try:
                        v["value"] = lib.value(v["id"], run(lib.call(v["id"], data, e.get("options")), dict(ns)))
                    except Exception as ex:  # noqa: BLE001
                        errors.append(f"{where}: variant {v['id']} raises: {type(ex).__name__}: {ex}")
                        continue
                    label, formula = LSM_STATS[v["stat"]]
                    v["name"] = f"{e['name']}, {label} ({v['level']} level)"
                    v["label"] = label
                    v["text"] = LSM_STAT_TEXT[v["stat"]]
                    v["formula"] = formula
                    _, metric = v["id"].split("_", 2)[1:]
                    v["operator"], v["setting"] = f"metrics.{v['level']}", f'metrics = ["{metric}"]'
                    variants.append(v)
                if not variants:
                    errors.append(f"{where}: none of the variants {e['variants']} exists in the library")
            if variants:
                out["variants"] = variants
            metrics.append(out)
        fam = {k: f[k] for k in ("family", "title", "order", "source", "intro", "example")}
        fam["data"] = data
        fam["metrics"] = metrics
        families.append(fam)
        if lib:
            coverage.setdefault(lib.name, set()).update(covered)
    if errors:
        raise CatalogError("\n".join(errors))
    families.sort(key=lambda x: (x["order"], x["family"]))
    # per library: (what it has, what the catalog covers, what it lacks); several families may share a library
    report = {name: (len(libs[name].keys()), len(got), sorted(libs[name].keys() - got)) for name, got in coverage.items()}
    return {"library_version": tf.__version__, "families": families}, report


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    ap.add_argument("--strict", action="store_true", help="fail when the library has metrics no entry covers")
    ap.add_argument("--check", action="store_true", help="write nothing; exit 1 when data/metrics.json would change")
    ap.add_argument("--list", action="store_true", help="print every uncovered metric, not the first ones")
    ap.add_argument("--only", nargs="+", metavar="FAMILY",
                    help="check only these families (data/metrics/<family>.yaml) and write nothing: for work on one family "
                         "while others are being edited")
    ap.add_argument("--out", type=pathlib.Path, metavar="FILE",
                    help="with --only: write those families' JSON to FILE (for tools/check_formulas.py --json FILE)")
    a = ap.parse_args()
    try:
        data, coverage = build(set(a.only) if a.only else None)
    except CatalogError as e:
        print(e, file=sys.stderr)
        print("metrics catalog: FAILED", file=sys.stderr)
        return 1
    except Exception:  # noqa: BLE001
        traceback.print_exc()
        return 1
    n = sum(len(f["metrics"]) + sum(len(m.get("variants", [])) for m in f["metrics"]) for f in data["families"])
    print(f"metrics catalog: {len(data['families'])} families, {sum(len(f['metrics']) for f in data['families'])} cards, "
          f"{n} metrics with their variants")
    missing = 0
    for fam, (have, got, rest) in coverage.items():
        missing += len(rest)
        shown = rest if a.list else rest[:12]
        print(f"  library {fam}: it has {have}, the catalog covers {got}, {len(rest)} not yet"
              + (f": {', '.join(shown)}{' ...' if len(shown) < len(rest) else ''}" if rest else ""))
    text = json.dumps(data, ensure_ascii=False, indent=1) + "\n"
    if a.only and a.out:
        a.out.write_text(text, encoding="utf-8")
        print(f"--only: wrote {a.out} (not data/metrics.json)")
    elif a.only:
        print("--only: nothing written")
    elif a.check:
        old = OUT.read_text(encoding="utf-8") if OUT.is_file() else ""
        if old != text:
            print("data/metrics.json is out of date: run the generator", file=sys.stderr)
            return 1
    else:
        OUT.write_text(text, encoding="utf-8")
        print(f"wrote {OUT.relative_to(WEB.parent).as_posix()}")
    if a.strict and missing:
        print(f"--strict: {missing} metrics of the library have no entry", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
