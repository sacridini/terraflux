"""Make website/data/python_api.json, the API reference of the Python library, from its sources.

    python website/tools/refresh_python_api.py                 # extern/terraflux-py -> website/data/python_api.json
    python website/tools/refresh_python_api.py --src DIR       # another checkout of terraflux-py

The package is read statically with griffe (nothing is imported, nothing is built): `pip install griffe` (or
griffelib) is all it needs, beside the submodule `extern/terraflux-py`. The docstrings are NumPy style; the Sphinx
roles they hold (:func:`mspa`, :class:`~terraflux.RasterCollection`) become `[`mspa`][terraflux.mspa]`, which the
site's build (lib/pyapi.py) links to the page that documents the object, or leaves as code.

The public objects are those of every public module (no underscore in its path): what its `__all__` names, and
what it defines with a name not starting with an underscore; a class brings its public methods, properties and
documented attributes (as mkdocstrings did, `__init__` is merged into the class). The file holds:

    version, repo                 the library's version (pyproject.toml) and its GitHub repository
    objects{path: {...}}          one entry per object, by the path of its definition
    aliases{public path: path}    the other names: `terraflux.mspa` is the function `terraflux.mspa.mspa`

An object: kind (function, class, method, classmethod, staticmethod, property, attribute, module), name, sig (the
signature as text, with annotations and defaults), summary, doc (the parsed docstring: sections `text`, `params`,
`returns`, `raises`, `ex`(amples), `adm`(onitions: Notes, See Also...)), src and line (the definition), members
(the paths of its methods and attributes, in source order), bases.
"""

from __future__ import annotations

import argparse
import inspect
import json
import pathlib
import re
import sys
import tomllib

import griffe

HERE = pathlib.Path(__file__).resolve().parent
WEBSITE = HERE.parent
ROOT = WEBSITE.parent
sys.path.insert(0, str(WEBSITE))
from lib.pyapi import resolve_path  # noqa: E402

REPO = "https://github.com/sacridini/terraflux-py"
ROLE = re.compile(r":(?:py:)?(func|class|meth|attr|data|mod|obj|exc):`(~?)([^`]+)`")
PACKAGE = "terraflux"
WIDTH = 88

OBJECTS: dict[str, dict] = {}
ALIASES: dict[str, str] = {}
RAW: dict[str, list] = {}  # path -> (context, sections) before the roles are resolved


def inside(obj) -> bool:
    return obj.path == PACKAGE or obj.path.startswith(PACKAGE + ".")


def private(path: str) -> bool:
    return any(p.startswith("_") for p in path.split("."))


def s(x) -> str:
    return "" if x is None else str(x)


# ------------------------------------------------------------------------------------------------ signatures


def params_of(obj, skip_first: bool) -> list:
    ps = list(obj.parameters)
    # `@threaded` adds the keyword n_threads at run time (the old docs' extension did the same)
    if obj.is_function and any(s(d.value).endswith("threaded") for d in obj.decorators) and "n_threads" not in obj.parameters:
        new = griffe.Parameter("n_threads", annotation="int | None", kind=griffe.ParameterKind.keyword_only, default="None")
        at = next((i for i, p in enumerate(ps) if p.kind is griffe.ParameterKind.var_keyword), len(ps))
        ps.insert(at, new)
    if skip_first and ps and ps[0].name in ("self", "cls"):
        ps = ps[1:]
    return ps


def format_params(ps, defaults: dict | None = None) -> list[str]:
    out, kw_marked, pos_only = [], False, any(p.kind is griffe.ParameterKind.positional_only for p in ps)
    last_po = max((i for i, p in enumerate(ps) if p.kind is griffe.ParameterKind.positional_only), default=-1)
    for i, p in enumerate(ps):
        k = p.kind
        if k is griffe.ParameterKind.var_positional:
            kw_marked = True
        if k is griffe.ParameterKind.keyword_only and not kw_marked:
            out.append("*")
            kw_marked = True
        name = {griffe.ParameterKind.var_positional: "*", griffe.ParameterKind.var_keyword: "**"}.get(k, "") + p.name
        default = defaults.get(p.name, "") if defaults is not None else s(p.default)
        if defaults is not None and p.name not in defaults:
            default = ""
        if p.annotation is not None:
            name += f": {p.annotation}"
            if default:
                name += f" = {default}"
        elif default:
            name += f"={default}"
        out.append(name)
        if pos_only and i == last_po:
            out.append("/")
    return out


def wrap_sig(head: str, items: list[str], tail: str) -> str:
    one = f"{head}({', '.join(items)}){tail}"
    if len(one) <= WIDTH or not items:
        return one
    return f"{head}(\n" + "".join(f"    {it},\n" for it in items) + f"){tail}"


def field_default(attr) -> str:
    v = s(attr.value) if attr is not None and attr.value is not None else ""
    if v.startswith("field("):
        m = re.search(r"default\s*=\s*([^,)]+)", v)
        return m.group(1) if m else "..."
    return v


def signature(obj, name: str) -> str | None:
    if obj.is_class:
        init = obj.members.get("__init__")
        if init is None or not init.is_function:
            return None
        if "dataclass" in obj.labels:
            ps = params_of(init, True)
            defaults = {p.name: field_default(obj.members.get(p.name)) for p in ps}
            return wrap_sig(name, format_params(ps, defaults), "")
        return wrap_sig(name, format_params(params_of(init, True)), "")
    if obj.is_function:
        method = obj.parent is not None and obj.parent.is_class and "staticmethod" not in obj.labels
        ret = f" -> {obj.returns}" if obj.returns is not None else ""
        return wrap_sig(name, format_params(params_of(obj, method)), ret)
    return None


# ------------------------------------------------------------------------------------------------ docstrings


def see_also(text: str) -> str:
    """NumPy's See Also lines (`a, b : what they do`) as a list of references."""
    out = []
    for line in text.splitlines():
        m = re.match(r"^(\S[^:]*?)\s*:\s*(.*)$", line)
        names, desc = (m.group(1), m.group(2)) if m else (line.strip(), "")
        if not names:
            continue
        refs = ", ".join(f":obj:`{n.strip()}`" for n in names.split(","))
        out.append(f"- {refs}" + (f": {desc}" if desc else ""))
    return "\n".join(out)


def raw_section(obj, title: str) -> str:
    """The text of a NumPy section as written (the parser splits prose at its colons)."""
    lines = inspect.cleandoc(obj.docstring.value).splitlines()
    dashes = lambda l: bool(l.strip()) and set(l.strip()) == {"-"}  # noqa: E731
    for i, line in enumerate(lines[:-1]):
        if line.strip() == title and dashes(lines[i + 1]):
            out = []
            for j in range(i + 2, len(lines)):
                if j + 1 < len(lines) and dashes(lines[j + 1]):
                    break
                out.append(lines[j])
            return "\n".join(out).strip()
    return ""


def parse_doc(obj) -> tuple[str, list]:
    """(summary, sections) of an object's docstring, texts not yet cleaned of roles."""
    if obj.docstring is None:
        return "", []
    secs = obj.docstring.parse("numpy", warn_unknown_params=False, warnings=False)
    out = []
    summary = ""
    for sec in secs:
        kind = sec.kind.value
        v = sec.value
        if kind == "text":
            text = v.strip()
            if not summary and not out:
                summary, _, rest = text.partition("\n\n")
                summary = " ".join(summary.split())
                text = rest.strip()
            if text:
                out.append({"t": "text", "x": text})
        elif kind in ("parameters", "other parameters", "attributes"):
            items = []
            for p in v:
                d = s(getattr(p, "default", None) if kind != "attributes" else None)
                items.append([p.name, s(p.annotation), p.description.strip(), d])
            out.append({"t": "params" if kind != "attributes" else "attrs", "h": sec.title or kind.capitalize(), "i": items})
        elif kind in ("returns", "yields", "receives"):
            items = [[p.name or "", s(p.annotation), p.description.strip()] for p in v]
            if all(not i[2] and not i[0] for i in items) and (len(items) > 1 or len(items[0][1]) > 60):
                # a section written as prose, not as "type / description": its lines are the text
                out.append({"t": "prose", "h": sec.title or kind.capitalize(), "x": raw_section(obj, sec.title or kind.capitalize())})
            else:
                out.append({"t": "returns", "h": sec.title or kind.capitalize(), "i": items})
        elif kind in ("raises", "warns"):
            out.append({"t": "raises", "h": sec.title or kind.capitalize(),
                        "i": [[s(p.annotation), p.description.strip()] for p in v]})
        elif kind == "examples":
            items = []
            for k, text in v:
                items.append(["c" if k.value == "examples" else "t", text.strip("\n")])
            out.append({"t": "ex", "h": "Examples", "i": items})
        elif kind == "admonition":
            text = v.description.strip()
            if (sec.title or "").lower() == "see also":
                text = see_also(text)
            out.append({"t": "adm", "h": sec.title or v.annotation, "x": text})
        else:
            out.append({"t": "adm", "h": sec.title or kind, "x": s(v)})
    return summary, out


def merge_init(cls) -> list:
    """The docstring sections of a class's __init__ that the class's own docstring lacks (mkdocstrings'
    merge_init_into_class)."""
    init = cls.members.get("__init__")
    if init is None or not init.is_function or init.docstring is None:
        return []
    _, sections = parse_doc(init)
    return sections


# ------------------------------------------------------------------------------------------------ collection


def kind_of(obj) -> str:
    if obj.is_module:
        return "module"
    if obj.is_class:
        return "class"
    if obj.is_function:
        if obj.parent is not None and obj.parent.is_class:
            for lab in ("classmethod", "staticmethod"):
                if lab in obj.labels:
                    return lab
            return "method"
        return "function"
    if "property" in obj.labels:
        return "property"
    return "attribute"


def filepath(obj) -> str:
    try:
        p = pathlib.Path(obj.filepath).resolve()
        return p.relative_to(SRC_ROOT).as_posix()
    except Exception:
        return ""


def add(obj, public: str, context_class: str | None = None) -> str:
    """Record an object (and its members) once, under the path of its definition; returns that path."""
    canon = obj.path
    if canon in OBJECTS:
        return canon
    kind = kind_of(obj)
    summary, secs = parse_doc(obj)
    if obj.is_class:
        have = {x["t"] for x in secs}
        for extra in merge_init(obj):
            if extra["t"] not in have:
                secs.append(extra)
    rec = {"kind": kind, "name": obj.name, "path": canon, "sig": signature(obj, obj.name), "summary": summary,
           "doc": secs, "src": filepath(obj), "line": obj.lineno or 0}
    if kind == "attribute" and obj.is_attribute and obj.value is not None:
        v = s(obj.value)
        rec["value"] = v if len(v) <= 120 else v[:117] + "..."
    if obj.is_attribute and obj.annotation is not None:
        rec["type"] = s(obj.annotation)
    OBJECTS[canon] = rec
    RAW[canon] = obj.path if obj.is_class else (obj.parent.path if obj.parent is not None and obj.parent.is_class else "")
    if obj.is_class:
        rec["bases"] = [s(b) for b in obj.bases]
        members = []
        for n, m in obj.members.items():
            if n.startswith("_") or m.is_alias:
                continue
            if m.is_function or m.is_class:
                if m.docstring is None:
                    continue
            elif m.is_attribute:
                if m.docstring is None:
                    continue
            else:
                continue
            members.append(add(m, f"{public}.{n}"))
        rec["members"] = members
    return canon


def final(m):
    try:
        return m.final_target
    except Exception:
        return None


VISITED: set[str] = set()


def add_module(mod, public: str, members: list[str]):
    """A module's own entry: its docstring and the objects it exposes (not where `terraflux.mspa` is a function)."""
    if public in ALIASES or mod.path in OBJECTS:
        return
    summary, secs = parse_doc(mod)
    OBJECTS[mod.path] = {"kind": "module", "name": mod.name, "path": mod.path, "sig": None, "summary": summary, "doc": secs,
                         "src": filepath(mod), "line": 1, "members": members}
    RAW[mod.path] = ""


def collect(mod, public: str) -> None:
    """The public objects of a module (and, below, of its submodules)."""
    if mod.path in VISITED:
        return
    VISITED.add(mod.path)
    exports = None
    if mod.exports is not None:
        exports = [s(e) for e in mod.exports]
    members: list[str] = []
    for n, m in list(mod.members.items()):
        if n.startswith("_"):
            continue
        ppath = f"{public}.{n}"
        if m.is_alias:
            t = final(m)
            if t is None or not inside(t):
                continue
            if exports is not None and n not in exports:
                continue
            if exports is None and not mod.is_init_module:
                continue
            if t.is_module:
                if not private(t.path) or n in (exports or []):
                    collect(t, ppath)
                continue
            canon = add(t, ppath)
            if ppath != canon:
                ALIASES[ppath] = canon
            members.append(canon)
        elif m.is_module:
            # `from .mspa import mspa` makes terraflux.mspa the function, not the module of the same name
            imp = mod.imports.get(n)
            if imp and imp.startswith(PACKAGE + "."):
                try:
                    tgt = mod.package[imp.removeprefix(PACKAGE + ".")]
                except Exception:
                    tgt = None
                if tgt is not None and (tgt.is_function or tgt.is_class):
                    canon = add(tgt, ppath)
                    ALIASES[ppath] = canon
                    members.append(canon)
            if not private(m.path):
                collect(m, ppath)
        elif m.is_function or m.is_class:
            members.append(add(m, ppath))
        elif m.is_attribute:
            if (exports is not None and n in exports) or m.docstring is not None:
                members.append(add(m, ppath))
    add_module(mod, public, members)


# ------------------------------------------------------------------------------------------------ roles


def role_converter():
    names = set(OBJECTS) | set(ALIASES)

    def resolve(target: str, context: str):
        target = target.rstrip("()")
        cands = [target]
        if not target.startswith(PACKAGE):
            cands += [f"{PACKAGE}.{target}", f"{PACKAGE}.conn.{target}", f"{PACKAGE}.circuit.{target}"]
            if context:
                cands.insert(0, f"{context}.{target}")
                parts = context.split(".")
                cands += [".".join(parts[:i]) + "." + target for i in range(len(parts) - 1, 0, -1)]
        return next((c for c in cands if c in names or resolve_path(c, OBJECTS, ALIASES)), None)

    def convert(text: str, context: str = "") -> str:
        def repl(m):
            role, short, target = m.groups()
            label = target.rstrip("()").split(".")[-1] if short or "." in target else target.rstrip("()")
            if role in ("func", "meth") and not label.endswith("()"):
                label += "()"
            hit = resolve(target, context)
            return f"[`{label}`][{hit}]" if hit else f"`{label}`"

        return ROLE.sub(repl, text)

    return convert


def clean_roles():
    conv = role_converter()
    for path, rec in OBJECTS.items():
        ctx = RAW.get(path, "")
        rec["summary"] = conv(rec["summary"], ctx)
        for sec in rec["doc"]:
            if "x" in sec:
                sec["x"] = conv(sec["x"], ctx)
            if sec["t"] in ("params", "attrs"):
                for it in sec["i"]:
                    it[2] = conv(it[2], ctx)
            elif sec["t"] == "returns":
                for it in sec["i"]:
                    it[2] = conv(it[2], ctx)
            elif sec["t"] == "raises":
                for it in sec["i"]:
                    it[1] = conv(it[1], ctx)
            elif sec["t"] == "ex":
                for it in sec["i"]:
                    if it[0] == "t":
                        it[1] = conv(it[1], ctx)


# ------------------------------------------------------------------------------------------------ main

SRC_ROOT = ROOT / "extern" / "terraflux-py"


def main():
    global SRC_ROOT
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--src", type=pathlib.Path, default=ROOT / "extern" / "terraflux-py")
    ap.add_argument("--out", type=pathlib.Path, default=WEBSITE / "data" / "python_api.json")
    a = ap.parse_args()
    SRC_ROOT = a.src.resolve()
    if not (SRC_ROOT / "src" / PACKAGE).is_dir():
        sys.exit(f"{SRC_ROOT / 'src' / PACKAGE} not found: git submodule update --init extern/terraflux-py")
    version = tomllib.loads((SRC_ROOT / "pyproject.toml").read_text(encoding="utf-8"))["project"]["version"]
    pkg = griffe.load(PACKAGE, search_paths=[str(SRC_ROOT / "src")], docstring_parser="numpy",
                      docstring_options={"warn_unknown_params": False})
    collect(pkg, PACKAGE)
    clean_roles()
    data = {"version": version, "repo": REPO,
            "objects": dict(sorted(OBJECTS.items())), "aliases": dict(sorted(ALIASES.items()))}
    a.out.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(data, ensure_ascii=False, separators=(",", ":"))
    with open(a.out, "w", encoding="utf-8", newline="\n") as fh:  # LF on every system: the file is committed
        fh.write(text + "\n")
    kinds: dict[str, int] = {}
    for o in OBJECTS.values():
        kinds[o["kind"]] = kinds.get(o["kind"], 0) + 1
    print(f"{a.out}: {len(OBJECTS)} objects ({', '.join(f'{n} {k}' for k, n in sorted(kinds.items()))}), "
          f"{len(ALIASES)} aliases, {len(text.encode('utf-8')) / 1e6:.2f} MB, library {version}")


if __name__ == "__main__":
    main()
