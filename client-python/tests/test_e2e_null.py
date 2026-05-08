"""E2E tests against wonderlamp_server in null (no-display) mode.

Runs in CI and on any machine — no display or GPU required.

    make test-e2e-null
    uv run pytest tests/test_e2e_null.py
"""

import pathlib
import subprocess
import time

import pytest
import zmq

from wonderlamp import Connection
from wonderlamp._proto import service_pb2, system_pb2
from ._e2e_cases import *  # noqa: F401, F403

_REPO_ROOT = pathlib.Path(__file__).parents[2]
_DEFAULT_ADDRESS = "tcp://localhost:5555"


def _reachable(address: str, timeout_ms: int = 500) -> bool:
    ctx = zmq.Context.instance()
    sock = ctx.socket(zmq.REQ)
    sock.setsockopt(zmq.LINGER, 0)
    sock.setsockopt(zmq.RCVTIMEO, timeout_ms)
    sock.connect(address)
    try:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            query_server_info=system_pb2.QueryServerInfo(),
        )
        sock.send(req.SerializeToString())
        sock.recv()
        return True
    except zmq.Again:
        return False
    finally:
        sock.close()


@pytest.fixture(scope="session")
def server_address() -> str:
    return _DEFAULT_ADDRESS


@pytest.fixture(scope="session", autouse=True)
def server_process(server_address: str):
    """Build and start the server in null mode. Never skipped."""
    if _reachable(server_address):
        yield
        return

    result = subprocess.run(["cargo", "build", "--release"], cwd=_REPO_ROOT)
    if result.returncode != 0:
        pytest.fail(f"cargo build --release failed (exit {result.returncode})")

    server_bin = _REPO_ROOT / "target" / "release" / "wonderlamp_server"
    proc = subprocess.Popen([str(server_bin), "--null"])

    for _ in range(20):
        if _reachable(server_address):
            break
        time.sleep(0.5)
    else:
        proc.terminate()
        pytest.fail("null server did not become ready in time")

    yield
    proc.terminate()
    proc.wait(timeout=5)


@pytest.fixture(scope="session")
def conn(server_address: str) -> Connection:
    c = Connection(server_address)
    yield c
    c.close()
