"""The engine of the calculators' playgrounds: the crate terraflux-calc-web (Terraflux's own map algebra, its
editor's help and its scripts over features, from crates/ops) built to WebAssembly, and written in base64 as
website/static/js/calc-engine.js, which calc.js loads (a script, so that the site works from its files too).

Run it after a change to crates/ops/src/algebra.rs, algebra_tools.rs, vector_algebra.rs or measures.rs (needs
the target: rustup target add wasm32-unknown-unknown):

    python website/tools/build_calc.py
"""

import base64
import os
import pathlib
import subprocess

WEB = pathlib.Path(__file__).resolve().parents[1]
ROOT = WEB.parent
TARGET = "wasm32-unknown-unknown"

# small rather than fast: the page computes a few thousand cells at a time
env = dict(os.environ, CARGO_PROFILE_RELEASE_DEBUG="false", CARGO_PROFILE_RELEASE_STRIP="true",
           CARGO_PROFILE_RELEASE_LTO="true", CARGO_PROFILE_RELEASE_CODEGEN_UNITS="1",
           CARGO_PROFILE_RELEASE_OPT_LEVEL="s", CARGO_PROFILE_RELEASE_PANIC="abort")
subprocess.run(["cargo", "build", "-p", "terraflux-calc-web", "--target", TARGET, "--release", "-j", "4"],
               cwd=ROOT, env=env, check=True)
wasm = ROOT / "target" / TARGET / "release" / "terraflux_calc_web.wasm"
version = next(line.split('"')[1] for line in (ROOT / "Cargo.toml").read_text(encoding="utf-8").splitlines()
               if line.startswith("version = "))
js = WEB / "static" / "js" / "calc-engine.js"
js.write_text(
    f"/* Terraflux {version}'s calculators (crates/calc-web) built to WebAssembly by website/tools/build_calc.py. */\n"
    f'window.TFCalcEngine = {{ version: "{version}", wasm: "{base64.b64encode(wasm.read_bytes()).decode()}" }};\n',
    encoding="utf-8",
)
print(f"{js.relative_to(ROOT)}: {wasm.stat().st_size // 1024} kB of WebAssembly")
