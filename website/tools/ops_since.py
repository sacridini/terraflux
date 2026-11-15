"""The first version of Terraflux that had each operator: data/ops_since.json {operator: version}.

Walks the git history of website/data/ops.json, oldest first. The version of a snapshot is the one in data/cli.json
at the same commit, else the one in that commit's message ("chore: release 1.2.3", "... 1.2.3"). Operators in the
very first snapshot get that version too (the history of the data starts there, not the operator's), so the file
also tells that version as "floor" and the site shows "Since" only for operators added after it.

    python website/tools/ops_since.py
"""
import json
import pathlib
import re
import subprocess

HERE = pathlib.Path(__file__).resolve().parent.parent
ROOT = HERE.parent


def git(*args: str) -> str:
    r = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, check=True)
    return r.stdout.decode("utf-8", "replace")


def version_at(sha: str) -> str | None:
    try:
        v = json.loads(git("show", f"{sha}:website/data/cli.json")).get("version")
        if v:
            return v
    except (subprocess.CalledProcessError, ValueError):
        pass
    m = re.search(r"\b(\d+\.\d+\.\d+)\b", git("log", "-1", "--format=%s", sha))
    return m.group(1) if m else None


def vkey(v: str):
    return tuple(int(x) for x in v.split("."))


def main() -> None:
    shas = git("log", "--format=%H", "--", "website/data/ops.json").split()[::-1]
    since: dict[str, str] = {}
    floor = None
    last = None
    for sha in shas:
        v = version_at(sha) or last
        if v is None:
            continue
        if last and vkey(v) < vkey(last):
            v = last  # a snapshot never goes back in time
        last = v
        try:
            names = [o["name"] for o in json.loads(git("show", f"{sha}:website/data/ops.json"))]
        except (subprocess.CalledProcessError, ValueError):
            continue
        if floor is None:
            floor = v
        for n in names:
            since.setdefault(n, v)
    now = {o["name"] for o in json.loads((HERE / "data" / "ops.json").read_text(encoding="utf-8"))}
    out = {"floor": floor, "ops": {n: since[n] for n in sorted(now) if n in since}}
    (HERE / "data" / "ops_since.json").write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    first = sum(1 for n in out["ops"].values() if n == floor)
    print(f"{len(out['ops'])}/{len(now)} operators, floor {floor} ({first} in the first snapshot)")


if __name__ == "__main__":
    main()
