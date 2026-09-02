"""E2E tests for the psychopy-compatible visual API against a null-mode server.

Runs in CI and on any machine — no display or GPU required.

    make test-e2e-null
    uv run pytest tests/e2e/test_psychopy_visual_null.py
"""

import subprocess
import time

import pytest

import vstimd.psychopy.visual as visual

from .psychopy_visual_cases import *  # noqa: F401, F403
from .conftest import reachable
# The same server this suite's sibling starts: one port, one server, whichever
# of the two files runs first.
from .test_e2e_null import _server_binary, server_address  # noqa: F401


@pytest.fixture(scope="session", autouse=True)
def server_process(server_address: str):
    """Build and start the server in null mode. Never skipped."""
    if reachable(server_address):
        yield
        return

    port = server_address.rsplit(":", 1)[-1]
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


@pytest.fixture(scope="session")
def win(server_address: str) -> visual.Window:
    w = visual.Window(address=server_address)
    yield w
    w.close()
