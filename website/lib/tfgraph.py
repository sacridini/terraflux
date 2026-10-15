"""Terraflux model graphs drawn as SVG, the way the application's canvas shows them.

A ```tfgraph block holds a model in TOML (a whole model file, or some of its [[nodes]]). Lines starting with
`#!` are options (they are TOML comments, so the block stays a valid model):

    #! caption: MSPA on a forest map
    #! highlight: mspa, write       nodes drawn as selected (orange)
    #! numbered: true               a number on every node, in the order of the file
    #! fragment: true               part of a model: not checked by `build.py --check`
    #! toml: false                  no "model file" toggle under the drawing
    #! params: false                no settings inside the nodes
"""

from __future__ import annotations

import html
import re
import tomllib
from dataclasses import dataclass, field

# geometry (px)
CH = 6.3  # width of a character at 11px
HEAD = 36
ROW = 17
PROW = 15
PAD = 10
COL_GAP = 64
ROW_GAP = 22
PILL_H = 24
MAX_PARAMS = 4

REF = re.compile(r"^[A-Za-z_][\w-]*\.[A-Za-z_][\w]*$")

TYPE_CLASS = {
    "map": "map", "raster": "map", "stack": "stack", "series": "stack", "table": "table", "vector": "vector",
    "number": "num", "integer": "num", "bool": "num", "text": "text", "agents": "agents",
}


def esc(s) -> str:
    return html.escape(str(s), quote=True)


def short(v, n=30) -> str:
    if isinstance(v, str):
        s = v.split("\n")[0] + (" …" if "\n" in v else "")
        s = f'"{s}"' if not s.startswith(("$", "@")) else s
    elif isinstance(v, bool):
        s = "true" if v else "false"
    elif isinstance(v, (list, tuple)):
        s = "[" + ", ".join(short(x, 10) for x in v[:4]) + (", …" if len(v) > 4 else "") + "]"
    elif isinstance(v, dict):
        s = "{ " + ", ".join(f"{k} = {short(x, 10)}" for k, x in list(v.items())[:2]) + (", … }" if len(v) > 2 else " }")
    else:
        s = str(v)
    return s if len(s) <= n else s[: n - 1] + "…"


def tw(s: str, scale=1.0) -> float:
    return len(s) * CH * scale


@dataclass
class Port:
    name: str
    lit: str | None = None  # a literal value given to an input
    type: str = "any"


@dataclass
class Box:
    id: str
    kind: str  # node | src | sink | ext
    label: str
    sub: str = ""
    ins: list[Port] = field(default_factory=list)
    outs: list[Port] = field(default_factory=list)
    params: list[str] = field(default_factory=list)
    inner: "Scope | None" = None
    highlight: bool = False
    number: int | None = None
    x: float = 0
    y: float = 0
    w: float = 0
    h: float = 0
    rank: int = 0

    def port_y(self, name: str, side: str) -> float:
        ports = self.ins if side == "in" else self.outs
        if self.kind != "node":
            return self.y + self.h / 2
        for i, p in enumerate(ports):
            if p.name == name:
                return self.y + HEAD + i * ROW + ROW / 2
        return self.y + HEAD / 2


@dataclass
class Edge:
    src: str
    sport: str
    dst: str
    dport: str
    type: str = "any"


@dataclass
class Scope:
    boxes: dict[str, Box]
    edges: list[Edge]
    w: float = 0
    h: float = 0


class GraphError(Exception):
    pass


class Renderer:
    def __init__(self, ops: dict[str, dict]):
        self.ops = ops
        self.counter = 0

    # ---------------------------------------------------------------- building the scopes

    def signature(self, op: str) -> dict:
        return self.ops.get(op, {"inputs": [], "outputs": [], "params": []})

    def node_box(self, n: dict, outer_ids: set[str], used_outs: dict[str, set[str]], opts) -> Box:
        op = n.get("op", "?")
        sig = self.signature(op)
        b = Box(id=n["id"], kind="node", label=n["id"], sub=op)
        given = n.get("inputs", {}) or {}
        order = [p["name"] for p in sig["inputs"]]
        names = [k for k in order if k in given] + [k for k in given if k not in order]
        types = {p["name"]: p["type"] for p in sig["inputs"]}
        for k in names:
            v = given[k]
            is_ref = isinstance(v, str) and (v.startswith(("$", "@")) or REF.match(v))
            b.ins.append(Port(k, None if is_ref else short(v, 18), types.get(k, "any")))
        outs = [Port(p["name"], type=p["type"]) for p in sig["outputs"]]
        known = {p.name for p in outs}
        for name in sorted(used_outs.get(b.id, ())):
            if name not in known:
                outs.append(Port(name))
        body = n.get("body")
        if body:
            for k in list((body.get("carry") or {}).keys()) + list((body.get("collect") or {}).keys()) + list(
                (body.get("outputs") or {}).keys()
            ):
                if k not in {p.name for p in outs}:
                    outs.append(Port(k, type="stack" if k in (body.get("collect") or {}) else "any"))
        b.outs = outs
        if opts.get("params", True):
            ps = list((n.get("params") or {}).items())
            for k, v in ps[:MAX_PARAMS]:
                b.params.append(f"{k} = {short(v, 30)}")
            if len(ps) > MAX_PARAMS:
                b.params.append(f"+ {len(ps) - MAX_PARAMS} more settings")
        if body:
            b.inner = self.scope_body(n, body, outer_ids | {x["id"] for x in body.get("nodes", [])}, opts)
        return b

    def refs(self, nodes: list[dict]) -> list[tuple[str, str, str]]:
        """(node id, input name, reference) of every input that is a reference."""
        out = []
        for n in nodes:
            for k, v in (n.get("inputs") or {}).items():
                if isinstance(v, str) and (v.startswith(("$", "@")) or REF.match(v)):
                    out.append((n["id"], k, v))
        return out

    def used_outputs(self, nodes: list[dict], extra: list[str]) -> dict[str, set[str]]:
        used: dict[str, set[str]] = {}
        refs = [r for _, _, r in self.refs(nodes)] + extra
        for n in nodes:
            body = n.get("body") or {}
            refs += [v for v in (body.get("carry") or {}).values()]
            refs += [v for v in (body.get("collect") or {}).values()]
            refs += [v for v in (body.get("outputs") or {}).values()]
            refs += [r for _, _, r in self.refs(body.get("nodes", []))]
        for r in refs:
            if isinstance(r, str) and REF.match(r):
                a, b = r.split(".", 1)
                used.setdefault(a, set()).add(b)
        return used

    def scope_top(self, model: dict, opts) -> Scope:
        nodes = model.get("nodes", []) or []
        ids = {n["id"] for n in nodes}
        outputs = model.get("outputs", {}) or {}
        out_refs = [o.get("from", "") for o in outputs.values() if isinstance(o, dict)]
        used = self.used_outputs(nodes, out_refs)
        boxes: dict[str, Box] = {}
        edges: list[Edge] = []
        for name, spec in (model.get("inputs") or {}).items():
            kind = spec.get("kind", "") if isinstance(spec, dict) else ""
            boxes["$" + name] = Box(id="$" + name, kind="src", label="$" + name, sub=kind)
        hl = set(opts.get("highlight", []))
        for i, n in enumerate(nodes):
            b = self.node_box(n, ids, used, opts)
            b.highlight = n["id"] in hl
            if opts.get("numbered"):
                b.number = i + 1
            boxes[n["id"]] = b
        for nid, port, r in self.refs(nodes):
            self.link(boxes, edges, ids, set(), nid, port, r, top=True)
        for name, o in outputs.items():
            src = o.get("from", "") if isinstance(o, dict) else ""
            boxes["out:" + name] = Box(id="out:" + name, kind="sink", label=name, sub="output")
            if REF.match(src) and src.split(".")[0] in ids:
                a, p = src.split(".", 1)
                edges.append(Edge(a, p, "out:" + name, "", self.out_type(boxes[a], p)))
        return self.layout(Scope(boxes, edges))

    def scope_body(self, n: dict, body: dict, all_ids: set[str], opts) -> Scope:
        nodes = body.get("nodes", []) or []
        ids = {x["id"] for x in nodes}
        targets = list((body.get("carry") or {}).values()) + list((body.get("collect") or {}).values()) + list(
            (body.get("outputs") or {}).values()
        )
        used = self.used_outputs(nodes, targets)
        boxes: dict[str, Box] = {}
        edges: list[Edge] = []
        if n.get("op") == "control.repeat":
            boxes["@step"] = Box(id="@step", kind="src", label="@step", sub="loop")
        for k in (n.get("inputs") or {}):
            if k != "times":
                boxes["@" + k] = Box(id="@" + k, kind="src", label="@" + k, sub="carried" if k in (body.get("carry") or {}) else "")
        hl = set(opts.get("highlight", []))
        for x in nodes:
            b = self.node_box(x, all_ids, used, opts)
            b.highlight = x["id"] in hl
            boxes[x["id"]] = b
        for nid, port, r in self.refs(nodes):
            self.link(boxes, edges, ids, all_ids, nid, port, r, top=False)
        for kind, mapping in (("carry", body.get("carry")), ("collect", body.get("collect")), ("out", body.get("outputs"))):
            for name, src in (mapping or {}).items():
                sid = f"{kind}:{name}"
                label = {"carry": f"→ @{name}", "collect": f"collect {name}", "out": name}[kind]
                boxes[sid] = Box(id=sid, kind="sink", label=label, sub="next step" if kind == "carry" else "")
                if isinstance(src, str) and REF.match(src) and src.split(".")[0] in ids:
                    a, p = src.split(".", 1)
                    edges.append(Edge(a, p, sid, "", self.out_type(boxes[a], p)))
        return self.layout(Scope(boxes, edges))

    def out_type(self, b: Box, port: str) -> str:
        for p in b.outs:
            if p.name == port:
                return p.type
        return "any"

    def link(self, boxes, edges, ids, outer_ids, nid, port, r, top):
        dst = boxes[nid]
        dtype = next((p.type for p in dst.ins if p.name == port), "any")
        if r.startswith("$"):
            key = r
            if key not in boxes:
                boxes[key] = Box(id=key, kind="src" if top else "ext", label=r, sub="input" if top else "")
            edges.append(Edge(key, "", nid, port, dtype))
        elif r.startswith("@"):
            if r not in boxes:
                boxes[r] = Box(id=r, kind="src", label=r, sub="loop")
            edges.append(Edge(r, "", nid, port, dtype))
        else:
            a, p = r.split(".", 1)
            if a in ids:
                edges.append(Edge(a, p, nid, port, self.out_type(boxes[a], p)))
            elif not top and a in outer_ids:
                if r not in boxes:
                    boxes[r] = Box(id=r, kind="ext", label=r, sub="outside the loop")
                edges.append(Edge(r, "", nid, port, dtype))
            else:
                # not a node of the model: a literal after all
                for q in dst.ins:
                    if q.name == port:
                        q.lit = short(r, 18)

    # ---------------------------------------------------------------- layout

    def size(self, b: Box):
        if b.kind != "node":
            b.w = max(70, tw(b.label) + 26)
            b.h = PILL_H
            return
        in_w = max([tw(p.name + (f" = {p.lit}" if p.lit else "")) for p in b.ins] or [0])
        out_w = max([tw(p.name) for p in b.outs] or [0])
        head_w = max(tw(b.label, 1.1), tw(b.sub, 0.95)) + (22 if b.number else 0)
        par_w = max([tw(s, 1.0) + 6 for s in b.params] or [0])
        b.w = max(150, head_w + 2 * PAD, in_w + out_w + 3 * PAD + 6, par_w + 2 * PAD)
        rows = max(len(b.ins), len(b.outs), 1)
        b.h = HEAD + rows * ROW + (len(b.params) * PROW + 8 if b.params else 0) + 8
        if b.inner:
            b.w = max(b.w, b.inner.w + 2 * PAD)
            b.h += b.inner.h + PAD + 18

    def layout(self, s: Scope) -> Scope:
        for b in s.boxes.values():
            self.size(b)
        preds: dict[str, list[str]] = {k: [] for k in s.boxes}
        for e in s.edges:
            preds[e.dst].append(e.src)
        rank: dict[str, int] = {}

        def r(k, seen=()):
            if k in rank:
                return rank[k]
            if k in seen:
                return 0
            ps = preds[k]
            rank[k] = 0 if not ps else 1 + max(r(p, seen + (k,)) for p in ps)
            return rank[k]

        for k in s.boxes:
            r(k)
        # sinks (outputs of the model or of a loop) in a column of their own, the last
        last = max([rank[k] for k, b in s.boxes.items() if b.kind != "sink"] or [0])
        for k, b in s.boxes.items():
            if b.kind == "sink":
                rank[k] = last + 1
        # sources that only feed later columns sit just before their first reader
        succ: dict[str, list[str]] = {k: [] for k in s.boxes}
        for e in s.edges:
            succ[e.src].append(e.dst)
        for k, b in s.boxes.items():
            if b.kind in ("src", "ext") and succ[k]:
                rank[k] = max(0, min(rank[x] for x in succ[k]) - 1)
        cols: dict[int, list[str]] = {}
        for k in s.boxes:  # file order
            cols.setdefault(rank[k], []).append(k)
        order = {k: i for c in cols.values() for i, k in enumerate(c)}
        for _ in range(3):
            for c in sorted(cols):
                if c == 0:
                    continue
                def bary(k):
                    ps = [order[p] for p in preds[k] if p in order]
                    return sum(ps) / len(ps) if ps else order[k]
                cols[c].sort(key=bary)
                for i, k in enumerate(cols[c]):
                    order[k] = i
        x = PAD
        maxh = 0.0
        for c in sorted(cols):
            ks = cols[c]
            cw = max(s.boxes[k].w for k in ks)
            y = PAD
            for k in ks:
                b = s.boxes[k]
                # near the ports that feed it
                want = []
                for e in s.edges:
                    if e.dst == k and rank[e.src] < c:
                        sb = s.boxes[e.src]
                        want.append(sb.port_y(e.sport, "out") - (b.port_y(e.dport, "in") - b.y))
                target = sum(want) / len(want) if want else y
                b.y = max(y, target)
                b.x = x
                b.rank = c
                y = b.y + b.h + ROW_GAP
                maxh = max(maxh, b.y + b.h)
            x += cw + COL_GAP
        s.w = x - COL_GAP + PAD
        s.h = maxh + PAD
        return s

    # ---------------------------------------------------------------- drawing

    def draw_scope(self, s: Scope, ox: float, oy: float, out: list[str]):
        for e in s.edges:
            a, b = s.boxes[e.src], s.boxes[e.dst]
            x1 = ox + a.x + a.w
            y1 = oy + a.port_y(e.sport, "out")
            x2 = ox + b.x
            y2 = oy + b.port_y(e.dport, "in")
            dx = max(30, abs(x2 - x1) / 2)
            t = TYPE_CLASS.get(e.type, "any")
            out.append(
                f'<path class="tfg-edge t-{t}" d="M{x1:.1f},{y1:.1f} C{x1 + dx:.1f},{y1:.1f} {x2 - dx:.1f},{y2:.1f} {x2:.1f},{y2:.1f}"/>'
            )
        for b in s.boxes.values():
            self.draw_box(b, ox, oy, out)

    def draw_box(self, b: Box, ox: float, oy: float, out: list[str]):
        x, y = ox + b.x, oy + b.y
        if b.kind != "node":
            cls = {"src": "tfg-src", "sink": "tfg-sink", "ext": "tfg-ext"}[b.kind]
            out.append(f'<g class="tfg-pill {cls}"><title>{esc(b.label)} {esc(b.sub)}</title>')
            out.append(f'<rect x="{x:.1f}" y="{y:.1f}" width="{b.w:.1f}" height="{b.h}" rx="12"/>')
            out.append(f'<text x="{x + b.w / 2:.1f}" y="{y + 16:.1f}" text-anchor="middle">{esc(b.label)}</text>')
            cx = x + b.w if b.kind in ("src", "ext") else x
            out.append(f'<circle class="tfg-port" cx="{cx:.1f}" cy="{y + b.h / 2:.1f}" r="3.6"/></g>')
            return
        cls = "tfg-node" + (" is-hl" if b.highlight else "") + (" is-container" if b.inner else "")
        out.append(f'<g class="{cls}" data-node="{esc(b.id)}"><title>{esc(b.id)} ({esc(b.sub)})</title>')
        out.append(f'<rect class="tfg-box" x="{x:.1f}" y="{y:.1f}" width="{b.w:.1f}" height="{b.h:.1f}" rx="5"/>')
        tx = x + PAD
        if b.number:
            out.append(f'<circle class="tfg-num" cx="{x + PAD + 7:.1f}" cy="{y + 17:.1f}" r="8"/>')
            out.append(f'<text class="tfg-numt" x="{x + PAD + 7:.1f}" y="{y + 20.5:.1f}" text-anchor="middle">{b.number}</text>')
            tx += 20
        out.append(f'<text class="tfg-id" x="{tx:.1f}" y="{y + 15:.1f}">{esc(b.label)}</text>')
        out.append(f'<text class="tfg-op" x="{tx:.1f}" y="{y + 28:.1f}">{esc(b.sub)}</text>')
        out.append(f'<circle class="tfg-status" cx="{x + b.w - 8:.1f}" cy="{y + 8:.1f}" r="2.6"/>')
        for i, p in enumerate(b.ins):
            py = y + HEAD + i * ROW + ROW / 2
            t = TYPE_CLASS.get(p.type, "any")
            if p.lit is None:
                out.append(f'<circle class="tfg-port t-{t}" cx="{x:.1f}" cy="{py:.1f}" r="3.6"><title>{esc(p.name)}: {esc(p.type)}</title></circle>')
                out.append(f'<text class="tfg-pin" x="{x + PAD:.1f}" y="{py + 3.8:.1f}">{esc(p.name)}</text>')
            else:
                out.append(f'<circle class="tfg-port is-lit" cx="{x:.1f}" cy="{py:.1f}" r="3"/>')
                out.append(
                    f'<text class="tfg-pin" x="{x + PAD:.1f}" y="{py + 3.8:.1f}">{esc(p.name)}<tspan class="tfg-lit"> = {esc(p.lit)}</tspan></text>'
                )
        for i, p in enumerate(b.outs):
            py = y + HEAD + i * ROW + ROW / 2
            t = TYPE_CLASS.get(p.type, "any")
            out.append(f'<text class="tfg-pout" x="{x + b.w - PAD:.1f}" y="{py + 3.8:.1f}" text-anchor="end">{esc(p.name)}</text>')
            out.append(f'<circle class="tfg-port t-{t}" cx="{x + b.w:.1f}" cy="{py:.1f}" r="3.6"><title>{esc(p.name)}: {esc(p.type)}</title></circle>')
        rows = max(len(b.ins), len(b.outs), 1)
        py = y + HEAD + rows * ROW + 4
        if b.params:
            out.append(f'<line class="tfg-sep" x1="{x + 1:.1f}" x2="{x + b.w - 1:.1f}" y1="{py:.1f}" y2="{py:.1f}"/>')
            for i, s in enumerate(b.params):
                out.append(f'<text class="tfg-param" x="{x + PAD:.1f}" y="{py + 13 + i * PROW:.1f}">{esc(s)}</text>')
            py += len(b.params) * PROW + 8
        if b.inner:
            iy = py + 14
            out.append(f'<text class="tfg-body-label" x="{x + PAD:.1f}" y="{iy - 2:.1f}">body (runs every step)</text>' if b.sub == "control.repeat" else f'<text class="tfg-body-label" x="{x + PAD:.1f}" y="{iy - 2:.1f}">body</text>')
            out.append(
                f'<rect class="tfg-body" x="{x + PAD / 2:.1f}" y="{iy:.1f}" width="{b.w - PAD:.1f}" height="{b.inner.h:.1f}" rx="4"/>'
            )
            self.draw_scope(b.inner, x + PAD / 2, iy, out)
        out.append("</g>")

    # ---------------------------------------------------------------- the block

    def render(self, source: str) -> tuple[str, dict, str]:
        opts: dict = {}
        for line in source.splitlines():
            m = re.match(r"^\s*#!\s*(\w+)\s*:\s*(.*)$", line)
            if m:
                k, v = m.group(1), m.group(2).strip()
                if k == "highlight":
                    opts[k] = [x.strip() for x in v.split(",") if x.strip()]
                elif v.lower() in ("true", "false"):
                    opts[k] = v.lower() == "true"
                else:
                    opts[k] = v
        try:
            model = tomllib.loads(source)
        except tomllib.TOMLDecodeError as e:
            raise GraphError(f"tfgraph: the model is not valid TOML: {e}")
        for n in model.get("nodes", []):
            if "id" not in n:
                raise GraphError("tfgraph: a node without an id")
            if n.get("op") and n["op"] not in self.ops:
                raise GraphError(f"tfgraph: unknown operator {n['op']!r} (node {n['id']!r})")
        s = self.scope_top(model, opts)
        out: list[str] = []
        self.draw_scope(s, 0, 0, out)
        w, h = s.w, s.h
        self.counter += 1
        svg = (
            f'<svg class="tfg" viewBox="0 0 {w:.0f} {h:.0f}" width="{w:.0f}" height="{h:.0f}" '
            f'style="--w:{w:.0f}px" role="img" aria-label="{esc(opts.get("caption", "A Terraflux model graph"))}" '
            f'xmlns="http://www.w3.org/2000/svg">' + "".join(out) + "</svg>"
        )
        return svg, opts, model.get("name", "")
