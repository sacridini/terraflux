"""Run a command while holding the website's run lock, so heavy runs started by several people (or agents) at once
take turns instead of sharing the processors.

    python website/tools/one_at_a_time.py -- python -c "import terraflux as tf; ..."
    python website/tools/one_at_a_time.py -- python website/build.py --check

The lock is website/.cache/run.lock (an OS lock on the file: it goes when the process ends, even killed).
check_python_blocks.py takes the same lock for its whole run. The command gets at most 4 threads
(OMP_NUM_THREADS, RAYON_NUM_THREADS, TERRAFLUX_THREADS...), unless it sets fewer.
"""

from __future__ import annotations

import contextlib
import os
import pathlib
import subprocess
import sys
import time

LOCK = pathlib.Path(__file__).resolve().parent.parent / ".cache" / "run.lock"
THREAD_VARS = ("OMP_NUM_THREADS", "RAYON_NUM_THREADS", "OPENBLAS_NUM_THREADS", "MKL_NUM_THREADS",
               "NUMEXPR_NUM_THREADS", "TERRAFLUX_THREADS", "GDAL_NUM_THREADS")


@contextlib.contextmanager
def held(quiet: bool = False):
    """Hold the run lock (waiting for it) for the duration of the block."""
    if os.environ.get("TERRAFLUX_RUN_LOCK_HELD") == "1":  # a parent holds it already
        yield
        return
    LOCK.parent.mkdir(parents=True, exist_ok=True)
    f = open(LOCK, "a+b")
    t0, said = time.time(), False
    try:
        while True:
            try:
                if os.name == "nt":
                    import msvcrt
                    f.seek(0)
                    msvcrt.locking(f.fileno(), msvcrt.LK_NBLCK, 1)
                else:
                    import fcntl
                    fcntl.flock(f.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except OSError:
                if not said and not quiet:
                    print("waiting for the run lock (another heavy run is going)...", file=sys.stderr, flush=True)
                    said = True
                time.sleep(2)
        if said and not quiet:
            print(f"got the run lock after {time.time() - t0:.0f} s", file=sys.stderr, flush=True)
        os.environ["TERRAFLUX_RUN_LOCK_HELD"] = "1"
        try:
            yield
        finally:
            os.environ.pop("TERRAFLUX_RUN_LOCK_HELD", None)
            if os.name == "nt":
                import msvcrt
                f.seek(0)
                with contextlib.suppress(OSError):
                    msvcrt.locking(f.fileno(), msvcrt.LK_UNLCK, 1)
    finally:
        f.close()


def main() -> None:
    args = sys.argv[1:]
    if args and args[0] == "--":
        args = args[1:]
    if not args:
        sys.exit(__doc__)
    env = dict(os.environ)
    for k in THREAD_VARS:
        try:
            env[k] = str(min(int(env.get(k, "4")), 4))
        except ValueError:
            env[k] = "4"
    with held():
        env["TERRAFLUX_RUN_LOCK_HELD"] = "1"
        sys.exit(subprocess.run(args, env=env).returncode)


if __name__ == "__main__":
    main()
