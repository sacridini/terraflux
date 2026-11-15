"""The Python library's API reference (website/data/python_api.json) as the site's pages use it.

The JSON is made by tools/refresh_python_api.py (griffe, no import of the library). A content page asks for an
object with a line `::: terraflux.some.object` (the syntax of the old mkdocstrings pages), optionally followed by
indented options:

    ::: terraflux.Landscape
        options:
          members: [patch_metrics, class_metrics]

`expand()` turns each such line into a heading (the page's table of contents lists it) and a placeholder that
`fill()` replaces, after the Markdown is converted, by the rendered reference of the object. `autorefs()` turns
`[`label`][terraflux.x.y]` into a link to where that object is documented.
"""

from __future__ import annotations

import html
import json
import pathlib
import re
import textwrap

import yaml

DIRECTIVE = re.compile(r"^:::[ \t]+([A-Za-z_][\w.]*)[ \t]*$")
AUTOREF = re.compile(r"\[((?:`[^`\n]*`|[^\]\n])+)\]\[(terraflux[\w.]*)\]")
FENCE = re.compile(r"^\s*(```|~~~)")
KNOWN_OPTIONS = {"members", "show_root_heading", "show_root_full_path", "heading_level", "show_source",
                 "show_signature", "show_docstring"}


class ApiError(Exception):
    """A page asks for something the API data does not have (fails the build)."""


def resolve_path(path: str, objects: dict, aliases: dict) -> str | None:
    """The key in `objects` of a public path: the path itself, an alias (`terraflux.mspa` is the function
    `terraflux.mspa.mspa`, `terraflux.Landscape` the class of `terraflux.lsm`), or a member of an alias."""
    path = path.rstrip("()")
    if path in objects:
        return path
    if path in aliases:
        return aliases[path]
    if "." in path:
        head, last = path.rsplit(".", 1)
        base = resolve_path(head, objects, aliases)
        if base and f"{base}.{last}" in objects:
            return f"{base}.{last}"
    return None


class Api:
    def __init__(self, data_file: pathlib.Path):
        self.file = data_file
        self.loaded = data_file.exists()
        d = json.loads(data_file.read_text(encoding="utf-8")) if self.loaded else {}
        self.version = d.get("version", "")
        self.repo = d.get("repo", "")
        self.objects: dict = d.get("objects", {})
        self.aliases: dict = d.get("aliases", {})
        self.index: dict[str, tuple[str, str]] = {}  # any path of an object -> (page url, anchor)
        self.documented: list[tuple[str, str, str]] = []  # (page url, anchor, canonical path)
        self.unresolved: list[str] = []
        self.pending: dict[str, str] = {}  # placeholder -> html (filled by fill())
        self._n = 0

    def resolve(self, path: str) -> str | None:
        return resolve_path(path, self.objects, self.aliases)

    # ---------------------------------------------------------------------- directives

    def scan(self, body: str):
        """The directives of a page body: [(path, options)], the lines of the directive consumed."""
        out = []
        lines = body.split("\n")
        i, fenced = 0, None
        while i < len(lines):
            line = lines[i]
            fm = FENCE.match(line)
            if fm:
                fenced = None if fenced == fm.group(1) else (fenced or fm.group(1))
            m = None if fenced else DIRECTIVE.match(line)
            if not m:
                i += 1
                continue
            j = i + 1
            opt = []
            while j < len(lines) and (lines[j].startswith((" ", "\t")) or (not lines[j].strip() and j + 1 < len(lines)
                                                                         and lines[j + 1].startswith((" ", "\t")))):
                opt.append(lines[j])
                j += 1
            out.append((i, j, m.group(1), "\n".join(opt)))
            i = j
        return lines, out

    def options_of(self, text: str, page: str, path: str) -> dict:
        if not text.strip():
            return {}
        try:
            d = yaml.safe_load(textwrap.dedent(text)) or {}
        except yaml.YAMLError as e:
            raise ApiError(f"{page}: ::: {path}: the options are not valid YAML: {e}") from None
        o = d.get("options", d) if isinstance(d, dict) else None
        if not isinstance(o, dict):
            raise ApiError(f"{page}: ::: {path}: the options must be a mapping (`options:` then its keys)")
        return o

    def register(self, body: str, page: str, url: str):
        """Fill the index object -> page with the directives of a page (before any page is rendered)."""
        _, found = self.scan(body)
        for _, _, path, _opt in found:
            canon = self.resolve(path)
            if not canon:
                raise ApiError(self.unknown(page, path))
            loc = (url, path)
            for key in {path, canon, *[a for a, c in self.aliases.items() if c == canon]}:
                self.index.setdefault(key, loc)
            self.documented.append((url, path, canon))
            # the members shown with the object are reached by their own anchors
            for m, tail in self.members_of(canon, self.options_of(_opt, page, path)):
                self.index.setdefault(path + tail, (url, path + tail))
                self.index.setdefault(m, (url, path + tail))

    def members_of(self, canon: str, opts: dict) -> list[tuple[str, str]]:
        """The members shown under an object, as (their path, the tail of their public path): all of them, none
        (`members: false`) or those named (`members: [a, b]`, in that order)."""
        want = opts.get("members", True)
        have = self.objects[canon].get("members", [])
        if want is False or want is None:
            return []

        def tail(m):
            return m[len(canon):] if m.startswith(canon + ".") else "." + self.objects[m]["name"]

        out = [(m, tail(m)) for m in have]
        if isinstance(want, list):
            by_name = {self.objects[m]["name"]: (m, t) for m, t in out}
            missing = [w for w in want if str(w) not in by_name]
            if missing:
                raise ApiError(f"{canon}: members {missing} not found (it has {', '.join(by_name)})")
            out = [by_name[str(w)] for w in want]
        return out

    def unknown(self, page: str, path: str) -> str:
        near = [k for k in self.objects if k.rsplit(".", 1)[-1] == path.rsplit(".", 1)[-1]][:4]
        hint = f" (objects with that name: {', '.join(near)})" if near else ""
        if not self.loaded:
            return f"{page}: ::: {path}: {self.file} is missing, run `python website/tools/refresh_python_api.py`"
        return f"{page}: ::: {path}: no such object in {self.file.name}{hint}"

    def expand(self, body: str, page: str, render) -> str:
        """The body with each directive replaced by a heading and a placeholder (see fill)."""
        lines, found = self.scan(body)
        if not found:
            return body
        out, last = [], 0
        for i, j, path, opt in found:
            out.extend(lines[last:i])
            last = j
            canon = self.resolve(path)
            if not canon:
                raise ApiError(self.unknown(page, path))
            o = self.options_of(opt, page, path)
            for k in o:
                if k not in KNOWN_OPTIONS:
                    raise ApiError(f"{page}: ::: {path}: option {k!r} is not supported "
                                   f"(supported: {', '.join(sorted(KNOWN_OPTIONS))})")
            obj = self.objects[canon]
            level = int(o.get("heading_level", 3))
            title = path if o.get("show_root_full_path", False) else path.rsplit(".", 1)[-1]
            self._n += 1
            token = f"APIOBJECT{self._n:05d}END"
            self.pending[token] = render(self, path, canon, o, level)
            if o.get("show_root_heading", True):
                out.append(f"\n{'#' * level} `{title}` {{#{path}}}\n")
            else:
                out.append(f'\n<span id="{path}"></span>\n')
            out.append(f"\n{token}\n")
        out.extend(lines[last:])
        return "\n".join(out)

    def fill(self, text: str) -> str:
        for token, h in self.pending.items():
            text = re.sub(rf"<p>\s*{token}\s*</p>|{token}", lambda _m, h=h: h, text)
        self.pending.clear()
        return text

    # ---------------------------------------------------------------------- links

    def url_of(self, path: str) -> str | None:
        hit = self.index.get(path) or self.index.get(self.resolve(path) or "")
        if hit:
            return f"{hit[0]}#{hit[1]}"
        return None

    def autorefs(self, text: str) -> str:
        """`[label][terraflux.x]` as a Markdown link to where x is documented, or the label alone."""

        def rep(m):
            label, path = m.group(1), m.group(2)
            url = self.url_of(path)
            if url:
                return f"[{label}]({url})"
            self.unresolved.append(path)
            return label

        parts = re.split(r"(^(?:```|~~~).*?^(?:```|~~~)[ \t]*$)", text, flags=re.S | re.M)
        return "".join(p if p.startswith(("```", "~~~")) else AUTOREF.sub(rep, p) for p in parts)


def esc(s) -> str:
    return html.escape(str(s), quote=True)
