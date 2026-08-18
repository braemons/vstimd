"""Walk the on-screen e2e suite test by test, forwards or backwards.

    cd client/python && make test-e2e-browse

A pytest session runs its tests once, in order: `j` and `5j` can be handled
inside it by skipping ahead, but `k` cannot — a test that has run is done. So
the browser owns the loop. It runs pytest, and when the pause prompt asks to go
backwards, pytest exits with the target test number in a small JSON file and the
browser starts it again from there. Notes taken along the way live in the same
file, so nothing is lost across a jump.

It also owns the server, for two reasons: restarting it per session would make
backward jumps slow and flashy, and a windowed server leaves the terminal the
prompt is waiting in visible — which a fullscreen one does not.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import subprocess
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).parents[2]))

from tests.e2e.conftest import reachable  # noqa: E402

_PYTHON_CLIENT = pathlib.Path(__file__).parents[2]
_REPO_ROOT = _PYTHON_CLIENT.parents[1]
_SUITES = ["tests/e2e/test_e2e.py", "tests/e2e/test_psychopy_visual.py"]


def _start_server(address: str, window: str | None) -> subprocess.Popen | None:
    """Start a server for the whole browse session, if one is not up already."""
    if reachable(address):
        print(f"browse: using the server already at {address}")
        return None

    binary = _REPO_ROOT / "target" / "release" / (
        "vstimd.exe" if sys.platform == "win32" else "vstimd"
    )
    if not binary.exists():
        if subprocess.run(["cargo", "build", "--release"], cwd=_REPO_ROOT).returncode:
            sys.exit("browse: cargo build --release failed")

    argv = [str(binary)] + (["--windowed", window] if window else [])
    print(f"browse: starting {' '.join(argv)}")
    proc = subprocess.Popen(argv)
    for _ in range(40):
        if reachable(address):
            return proc
        time.sleep(0.5)
    proc.terminate()
    sys.exit("browse: the server did not come up")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--server", default="tcp://localhost:5555")
    parser.add_argument(
        "--windowed",
        default="1280x720",
        help="Size of the server window (default: 1280x720). --fullscreen "
        "overrides it, at the price of covering this terminal",
    )
    parser.add_argument("--fullscreen", action="store_true")
    parser.add_argument("--start-at", type=int, default=1, help="Test number to open on")
    parser.add_argument(
        "--state",
        default=".e2e-browse.json",
        help="Where the jump target and the notes are kept between sessions",
    )
    parser.add_argument(
        "--suites",
        nargs="+",
        default=_SUITES,
        help="Test files to browse (default: the two on-screen suites)",
    )
    parser.add_argument(
        "pytest_args", nargs="*", help="Anything else is passed on to pytest"
    )
    args = parser.parse_args(argv)

    state = pathlib.Path(args.state)
    state.write_text(json.dumps({"jump": None, "flags": []}), encoding="utf-8")

    server = _start_server(args.server, None if args.fullscreen else args.windowed)
    start = args.start_at
    try:
        while True:
            result = subprocess.run(
                [
                    sys.executable, "-m", "pytest", *args.suites, "-q",
                    "--pause=test",
                    f"--server={args.server}",
                    f"--start-at={start}",
                    f"--nav-file={state}",
                    *args.pytest_args,
                ],
                cwd=_PYTHON_CLIENT,
            )
            recorded = json.loads(state.read_text(encoding="utf-8"))
            jump = recorded.pop("jump", None)
            # Spend the jump before acting on it: a target left in the file
            # would send the next session back to the same test for ever.
            state.write_text(json.dumps({**recorded, "jump": None}, indent=2),
                             encoding="utf-8")
            if jump is None:
                return result.returncode
            print(f"\nbrowse: back to test {jump}\n")
            start = jump
    finally:
        if server is not None:
            server.terminate()


if __name__ == "__main__":
    raise SystemExit(main())
