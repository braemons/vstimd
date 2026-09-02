"""E2E tests against vstimd in null (no-display) mode.

Runs in CI and on any machine — no display or GPU required.

    make test-e2e-null
    uv run pytest tests/e2e/test_e2e_null.py

Unless told otherwise, this suite runs its own server on a port of its own.
Sharing one is not safe: every test resets the scene, so two sessions against
the same server delete each other's stimuli and animations, and the failures
that come out of that look like anything but the cause. Point `--server` (or
`VSTIMD_SERVER`) somewhere to opt into an existing one.
"""

import os
import pathlib
import socket
import subprocess
import sys
import time

import pytest

from .cases import *  # noqa: F401, F403
from .conftest import DEFAULT_SERVER, reachable

_REPO_ROOT = pathlib.Path(__file__).parents[4]


def _free_port() -> int:
    """A port nothing is listening on, for this suite's own server."""
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _server_binary() -> pathlib.Path:
    exe = "vstimd.exe" if sys.platform == "win32" else "vstimd"
    binary = _REPO_ROOT / "target" / "release" / exe
    if not binary.exists():
        result = subprocess.run(["cargo", "build", "--release"], cwd=_REPO_ROOT)
        if result.returncode != 0:
            pytest.fail(f"cargo build --release failed (exit {result.returncode})")
    if not binary.exists():
        pytest.fail(f"server binary not found at {binary}")
    return binary


@pytest.fixture(scope="session")
def server_address(request: pytest.FixtureRequest) -> str:
    """This session's server: one of its own, unless one was asked for."""
    asked_for = request.config.getoption("--server")
    if asked_for != DEFAULT_SERVER or os.environ.get("VSTIMD_SERVER"):
        return asked_for
    return f"tcp://localhost:{_free_port()}"


@pytest.fixture(scope="session", autouse=True)
def server_process(server_address: str):
    """Build and start the server in null mode. Never skipped."""
    if reachable(server_address):
        yield  # one was asked for and is already up
        return

    port = server_address.rsplit(":", 1)[-1]
    # --no-web as well: the web surface would fight another server for 8080,
    # and nothing here talks to it.
    proc = subprocess.Popen(
        [str(_server_binary()), "--null", "--zmq-port", port, "--no-web"]
    )

    for _ in range(20):
        if reachable(server_address):
            break
        time.sleep(0.5)
    else:
        proc.terminate()
        pytest.fail("null server did not become ready in time")

    yield
    proc.terminate()
    proc.wait(timeout=5)


@pytest.fixture
def step_delay() -> float:
    return 0.0
