"""Run the Python examples of the site's pages, as the library's tools/check_docs.py did for its own docs.

    python website/tools/check_python_blocks.py                      # every page below
    python website/tools/check_python_blocks.py python/index.md python/api/patterns.md
    python website/tools/check_python_blocks.py --list              # the pages that would run
    python website/tools/check_python_blocks.py --python PATH --jobs 2 --timeout 900
    python website/tools/check_python_blocks.py --outputs tutorials/x/y.md   # the ```text blocks must match

The pages: content/python/** and every page of content/tutorials/** with a runnable ```python block (the
`### In Python` sections of the tutorials' parts, the `*-in-python.md` pages). Every fenced ```python block
of a page runs in order, the blocks of one page sharing a namespace, in a temporary working directory (what the
examples write goes there). For a tutorial's part (tutorials/<category>/<part>.md) that directory starts with the
files the reader has: the part's downloads (static/data/tutorials/<category>/<part>/) and what its graphs wrote, from
the work folder of its figure script (.cache/work/<category>/<part>/, made by website/figures/<category>/<part>.py),
and every file under static/data/ that the page links as a download, `(/data/...)`. A block preceded by a line `<!-- no-run -->` is skipped; blocks inside tabs (indented)
run like the others. With `--outputs`, what a block prints must be the ```text block right after it (trailing spaces aside): a
difference is shown and counts as a failure, the page going on. A page stops at its first failing block (the next ones depend on it). Each page runs in a
process of its own with a time limit (`--timeout`, 600 s), so one that hangs does not stop the others.

The interpreter must have the library installed (`pip install terraflux-py`, or an editable install of
extern/terraflux-py): `--python`, else the environment variable TERRAFLUX_CHECK_PYTHON, else the one running this
script. Threads: at most 4 in all. `--jobs` pages run at once (default 2, TERRAFLUX_CHECK_JOBS; never above 4) and the
threads of each (OMP_NUM_THREADS, RAYON_NUM_THREADS, TERRAFLUX_THREADS...) are 4 divided among them.
The run holds website/.cache/run.lock (tools/one_at_a_time.py): two checks started at once take turns.
Exit status 1 if a block fails or a page times out. Not part of the CI (it needs the library built and runs for a while).
"""

from __future__ import annotations

import argparse
import contextlib
import difflib
import io
import concurrent.futures as cf
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import textwrap
import time
import traceback

HERE = pathlib.Path(__file__).resolve().parent
CONTENT = HERE.parent / "content"
STATIC = HERE.parent / "static"
WORK = HERE.parent / ".cache" / "work"
DOWNLOAD = re.compile(r"\(/data/([^)\s#?]+)\)")
MAX_THREADS = 4
FENCE = re.compile(r"^(?P<indent>[ \t]*)(?P<fence>`{3,}|~{3,})\s*(?P<lang>[\w+-]*)[^\n]*$")


def blocks(text: str):
    """(line number, code) of the runnable python blocks of a page."""
    lines = text.splitlines()
    i, skip_next = 0, False
    while i < len(lines):
        line = lines[i]
        if line.strip() == "<!-- no-run -->":
            skip_next = True
            i += 1
            continue
        m = FENCE.match(line)
        if not m:
            if line.strip():
                skip_next = skip_next and line.strip().startswith("<!--")
            i += 1
            continue
        indent, fence, lang = m.group("indent"), m.group("fence"), m.group("lang")
        body = []
        j = i + 1
        while j < len(lines) and not (lines[j].strip().startswith(fence) and lines[j].strip().strip(fence[0]) == ""):
            body.append(lines[j][len(indent):] if lines[j].startswith(indent) else lines[j].lstrip())
            j += 1
        if lang in ("python", "py") and not skip_next:
            yield i + 2, textwrap.dedent("\n".join(body))  # the line of the block's first code line
        skip_next = False
        i = j + 1


def outputs(text: str) -> dict[int, str]:
    """The ```text block right after a python block (blank lines between), by the python block's line."""
    lines = text.splitlines()
    out, pending, i = {}, None, 0
    while i < len(lines):
        m = FENCE.match(lines[i])
        if not m:
            if lines[i].strip():
                pending = None
            i += 1
            continue
        indent, fence, lang = m.group("indent"), m.group("fence"), m.group("lang")
        j = i + 1
        body = []
        while j < len(lines) and not (lines[j].strip().startswith(fence) and lines[j].strip().strip(fence[0]) == ""):
            body.append(lines[j][len(indent):] if lines[j].startswith(indent) else lines[j].lstrip())
            j += 1
        if lang in ("python", "py"):
            pending = i + 2
        elif lang == "text" and pending is not None:
            out[pending] = "\n".join(body)
            pending = None
        else:
            pending = None
        i = j + 1
    return out


def same_output(got: str, want: str) -> bool:
    def norm(t: str) -> str:
        return "\n".join(line.rstrip() for line in t.strip("\n").splitlines()).strip("\n")

    return norm(got) == norm(want)


def pages(args: list[str]) -> list[pathlib.Path]:
    if args:
        out = []
        for a in args:
            p = pathlib.Path(a)
            p = p if p.is_absolute() else (CONTENT / a if (CONTENT / a).exists() else pathlib.Path.cwd() / a)
            if not p.is_file():
                sys.exit(f"{a}: no such page (paths are relative to website/content)")
            out.append(p.resolve())
        return out
    found = set(CONTENT.glob("python/**/*.md"))
    found |= {p for p in (CONTENT / "tutorials").rglob("*.md") if next(blocks(p.read_text(encoding="utf-8")), None)}
    return sorted(p for p in found if not p.name.startswith("_"))


def seed_files(page: pathlib.Path, tmp: pathlib.Path) -> None:
    """Copy into `tmp` the files a tutorial part's reader has: what its graphs wrote, then its downloads."""
    try:
        rel = page.relative_to(CONTENT / "tutorials")
    except ValueError:
        return
    if len(rel.parts) != 2:
        return
    where = pathlib.Path(rel.parts[0]) / rel.stem
    for folder in (WORK / where, STATIC / "data" / "tutorials" / where):
        if folder.is_dir():
            for f in folder.iterdir():
                if f.is_file():
                    shutil.copy2(f, tmp / f.name)
    # and every download the page links, wherever it lives under static/data/ (the reader downloaded it too)
    for link in set(DOWNLOAD.findall(page.read_text(encoding="utf-8"))):
        f = STATIC / "data" / link
        if f.is_file() and not (tmp / f.name).exists():
            shutil.copy2(f, tmp / f.name)


def child(page: pathlib.Path) -> int:
    """Run one page's blocks (in this process); prints the failure and returns 1."""
    os.environ.setdefault("TERRAFLUX_PLOT_SHOW", "0")  # a plot must not open a window
    os.environ.setdefault("MPLBACKEND", "Agg")
    rel = page.relative_to(CONTENT).as_posix() if CONTENT in page.parents else page.name
    namespace: dict = {"__name__": "__docs__"}
    cwd = os.getcwd()
    text = page.read_text(encoding="utf-8")
    code_blocks = list(blocks(text))
    want = outputs(text) if os.environ.get("TERRAFLUX_CHECK_OUTPUTS") == "1" else {}
    status = 0
    # ignore_cleanup_errors: lazy (dask) arrays can keep files open on Windows
    with tempfile.TemporaryDirectory(prefix="terraflux_site_", ignore_cleanup_errors=True) as tmp:
        seed_files(page, pathlib.Path(tmp))
        os.chdir(tmp)
        try:
            for n, (line, code) in enumerate(code_blocks, 1):
                try:
                    if line in want:
                        buf = io.StringIO()
                        with contextlib.redirect_stdout(buf):
                            exec(compile(code, f"{rel}:{line}", "exec"), namespace)
                        if not same_output(buf.getvalue(), want[line]):
                            diff = difflib.unified_diff(want[line].strip("\n").splitlines(),
                                                        buf.getvalue().strip("\n").splitlines(),
                                                        "the page", "the run", lineterm="", n=1)
                            print(f"OUTPUT {rel}, block {n} (line {line}) prints something else:\n"
                                  + "\n".join(list(diff)[:24]))
                            status = 1
                    else:
                        exec(compile(code, f"{rel}:{line}", "exec"), namespace)
                except BaseException:  # noqa: BLE001 - reported per block
                    print(f"FAIL {rel}, block {n} of {len(code_blocks)} (line {line})\n" + traceback.format_exc(limit=5))
                    status = 1
                    break
        finally:
            os.chdir(cwd)
            namespace.clear()
    return status


def run_one(page: pathlib.Path, python: str, timeout: int, threads: int):
    env = dict(os.environ)
    for k in ("OMP_NUM_THREADS", "RAYON_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS", "NUMEXPR_NUM_THREADS",
              "TERRAFLUX_THREADS", "GDAL_NUM_THREADS"):
        env[k] = str(threads)
    n = sum(1 for _ in blocks(page.read_text(encoding="utf-8")))
    t = time.perf_counter()
    try:
        r = subprocess.run([python, str(pathlib.Path(__file__).resolve()), "--child", str(page)], env=env,
                           capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=timeout)
        out, ok = (r.stdout + r.stderr).strip(), r.returncode == 0
    except subprocess.TimeoutExpired:
        out, ok = f"TIMEOUT {page.name}: more than {timeout} s", False
    rel = page.relative_to(CONTENT).as_posix() if CONTENT in page.parents else page.name
    return rel, n, ok, out, time.perf_counter() - t


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("pages", nargs="*", help="pages (relative to website/content); default: all of the set above")
    ap.add_argument("--list", action="store_true", help="list the pages and their number of blocks, run nothing")
    ap.add_argument("--python", default=os.environ.get("TERRAFLUX_CHECK_PYTHON", sys.executable))
    ap.add_argument("--jobs", type=int, default=int(os.environ.get("TERRAFLUX_CHECK_JOBS", "2")))
    ap.add_argument("--timeout", type=int, default=600, help="seconds a page may run (default 600)")
    ap.add_argument("--outputs", action="store_true",
                    help="also compare what each block prints with the ```text block right after it")
    ap.add_argument("--child", type=pathlib.Path, help=argparse.SUPPRESS)
    a = ap.parse_args()
    if a.child:
        sys.exit(child(a.child.resolve()))
    todo = pages(a.pages)
    if a.list:
        for p in todo:
            print(f"{sum(1 for _ in blocks(p.read_text(encoding='utf-8'))):4d}  {p.relative_to(CONTENT).as_posix()}")
        return
    if a.outputs:
        os.environ["TERRAFLUX_CHECK_OUTPUTS"] = "1"
    sys.path.insert(0, str(HERE))
    from one_at_a_time import held  # heavy runs take turns (website/.cache/run.lock)

    with held():
        run_all(todo, a)


def run_all(todo: list[pathlib.Path], a) -> None:
    jobs = max(1, min(a.jobs, MAX_THREADS, len(todo) or 1))
    threads = max(1, MAX_THREADS // jobs)
    failed = 0
    ran = 0
    with cf.ThreadPoolExecutor(max_workers=jobs) as ex:
        futs = [ex.submit(run_one, p, a.python, a.timeout, threads) for p in todo]
        for f in futs:
            rel, n, ok, out, secs = f.result()
            ran += n
            if n:
                print(f"{'ok' if ok else 'FAILED':6s} {rel} ({n} blocks, {secs:.1f} s)", flush=True)
            if not ok:
                failed += 1
                print("\n" + out + "\n", flush=True)
    print(f"{len(todo)} pages, {ran} blocks, {failed} pages failed")
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
