"""Render every formula of the metrics catalog (data/metrics.json) with KaTeX, as the browser will, and list the ones it
refuses: a card's `formula`, its symbols, and the inline math `\\( ... \\)` of its texts and of its variants'.

    python website/tools/check_formulas.py          # exit status 1 when a formula does not render
    python website/tools/check_formulas.py --json other.json

Needs Node.js. KaTeX is the version the pages load (lib/offline.py's KATEX), kept in website/.cache/vendor/katex
(downloaded the first time, as the offline build does).
"""

from __future__ import annotations

import io
import json
import pathlib
import re
import subprocess
import sys

HERE = pathlib.Path(__file__).resolve().parent
WEB = HERE.parent
sys.path.insert(0, str(WEB))
from lib import offline  # noqa: E402

INLINE = re.compile(r"\\\((.+?)\\\)", re.S)
TEXT_KEYS = ("name", "unit", "range", "what", "use", "text", "label")

NODE = r"""
const katex = require(process.argv[1]);
let data = '';
process.stdin.on('data', d => data += d);
process.stdin.on('end', () => {
  const bad = [];
  for (const f of JSON.parse(data)) {
    try { katex.renderToString(f.tex, {displayMode: f.display, throwOnError: true, strict: 'ignore'}); }
    catch (e) { bad.push({where: f.where, tex: f.tex, error: String(e.message || e)}); }
  }
  process.stdout.write(JSON.stringify(bad));
});
"""


def formulas(data: dict):
    for fam in data["families"]:
        for t in INLINE.findall(fam.get("intro", "")):
            yield f"{fam['family']} (intro)", t, False
        for m in fam["metrics"]:
            where = f"{fam['family']}: {m['id']}"
            fs = m["formula"] if isinstance(m["formula"], list) else [m["formula"]]
            for f in fs:
                yield where + " formula", f, True
            for k, v in m.get("symbols", {}).items():
                yield where + " symbol", k, False
                for t in INLINE.findall(v):
                    yield where + " symbols", t, False
            for k in TEXT_KEYS:
                for t in INLINE.findall(str(m.get(k, ""))):
                    yield f"{where} {k}", t, False
            for v in m.get("variants", []):
                if v.get("formula"):
                    yield f"{where} variant {v['id']} formula", v["formula"], True
                for k in TEXT_KEYS:
                    for t in INLINE.findall(str(v.get(k, ""))):
                        yield f"{where} variant {v['id']} {k}", t, False


def main() -> int:
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8", errors="replace")
    src = pathlib.Path(sys.argv[sys.argv.index("--json") + 1]) if "--json" in sys.argv else WEB / "data" / "metrics.json"
    data = json.loads(src.read_text(encoding="utf-8"))
    items = [{"where": w, "tex": t, "display": d} for w, t, d in formulas(data)]
    js = offline._cached(WEB / ".cache" / "vendor" / "katex", "katex.min.js", offline.KATEX + "katex.min.js")
    lib = WEB / ".cache" / "vendor" / "katex" / "katex.min.js"
    if not lib.is_file():
        lib.parent.mkdir(parents=True, exist_ok=True)
        lib.write_bytes(js)
    r = subprocess.run(["node", "-e", NODE, str(lib)], input=json.dumps(items), capture_output=True, text=True,
                       encoding="utf-8")
    if r.returncode != 0:
        print(r.stderr, file=sys.stderr)
        return 2
    bad = json.loads(r.stdout or "[]")
    for b in bad:
        print(f"{b['where']}: {b['error']}\n    {b['tex']}")
    print(f"{len(items)} formulas, {len(bad)} KaTeX does not render")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
