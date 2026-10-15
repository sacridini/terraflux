"""Refresh the data the website is built from, out of the terraflux binary:

- data/ops.json: every operator's signature (`terraflux ops --json`)
- data/cli.json: the help of every command and subcommand (`terraflux <command> --help`)

    python website/tools/refresh_data.py [--bin path/to/terraflux]

The files are committed, so building the site needs Python only.
"""

import argparse
import json
import pathlib
import re
import subprocess

ROOT = pathlib.Path(__file__).resolve().parents[2]
DATA = ROOT / "website" / "data"


def default_bin() -> pathlib.Path:
    for name in ("terraflux.exe", "terraflux"):
        p = ROOT / "target" / "release" / name
        if p.exists():
            return p
    return pathlib.Path("terraflux")


def run(bin_: pathlib.Path, *args: str) -> str:
    out = subprocess.run([str(bin_), *args], capture_output=True, text=True, encoding="utf-8")
    text = out.stdout or out.stderr
    # the help names the program after the file it was run from
    return text.replace("terraflux.exe", "terraflux")


def commands(help_text: str) -> list[str]:
    """The commands listed in a help text (the lines under `Commands:`)."""
    m = re.search(r"Commands:\n(.*?)(\n\n|\Z)", help_text, re.S)
    if not m:
        return []
    names = []
    for line in m.group(1).splitlines():
        w = line.split()
        if w and w[0] != "help":
            names.append(w[0])
    return names


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--bin", type=pathlib.Path, default=default_bin())
    a = ap.parse_args()
    DATA.mkdir(parents=True, exist_ok=True)

    ops = json.loads(run(a.bin, "ops", "--json"))
    (DATA / "ops.json").write_text(json.dumps(ops, indent=1, ensure_ascii=False), encoding="utf-8")

    top = run(a.bin, "--help")
    cli = {"version": run(a.bin, "--version").strip().replace("terraflux ", ""), "help": top, "commands": {}}
    for c in commands(top):
        h = run(a.bin, c, "--help")
        entry = {"help": h, "subcommands": {}}
        for s in commands(h):
            entry["subcommands"][s] = run(a.bin, c, s, "--help")
        cli["commands"][c] = entry
    # the subjects of the application's library (crates/app/src/builder.rs), for the operator reference
    src = (ROOT / "crates/app/src/builder.rs").read_text(encoding="utf-8")
    block = src[src.index("const SUBJECTS"): src.index("];", src.index("const SUBJECTS"))]
    subjects = []
    for m in re.finditer(r'\(\s*"([^"]+)",\s*&\[(.*?)\],?\s*\)', block, re.S):
        subjects.append({"title": m.group(1), "prefixes": re.findall(r'"([^"]+)"', m.group(2))})
    io = re.findall(r'"([^"]+)"', src[src.index("const IO"): src.index(";", src.index("const IO"))])
    (DATA / "subjects.json").write_text(json.dumps({"subjects": subjects, "io": io}, indent=1), encoding="utf-8")
    (DATA / "cli.json").write_text(json.dumps(cli, indent=1, ensure_ascii=False), encoding="utf-8")
    print(f"{len(ops)} operators, {len(cli['commands'])} commands, version {cli['version']}")


if __name__ == "__main__":
    main()
