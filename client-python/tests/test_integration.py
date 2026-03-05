"""Integration tests — require a live vstim_server.

These tests are skipped by default. Run with:

    pytest --run-integration tests/test_integration.py

The ``vstim_server`` fixture starts (or connects to) the server at the address
specified by ``VSTIM_SERVER_ADDR`` env var (default: tcp://localhost:5555).
"""

from __future__ import annotations

import os

import pytest

from vstim_client import visual


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

def pytest_addoption(parser: pytest.Parser) -> None:
    parser.addoption(
        "--run-integration",
        action="store_true",
        default=False,
        help="Run integration tests that require a live vstim_server",
    )


@pytest.fixture(scope="session")
def vstim_address() -> str:
    return os.environ.get("VSTIM_SERVER_ADDR", "tcp://localhost:5555")


class _ServerHandle:
    def __init__(self, address: str) -> None:
        self.address = address

    def query_stimulus(self, handle: int) -> dict:
        """Query server for current state of a stimulus by handle."""
        import zmq  # type: ignore[import]
        import json
        ctx = zmq.Context.instance()
        s = ctx.socket(zmq.REQ)
        s.setsockopt(zmq.LINGER, 0)
        s.connect(self.address)
        s.send(json.dumps({"cmd": "query_stimulus", "handle": handle}).encode())
        reply = json.loads(s.recv())
        s.close()
        return reply


@pytest.fixture(scope="module")
def vstim_server(request: pytest.FixtureRequest, vstim_address: str) -> _ServerHandle:
    if not request.config.getoption("--run-integration", default=False):
        pytest.skip("Integration tests disabled. Pass --run-integration to enable.")
    return _ServerHandle(vstim_address)


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

@pytest.mark.integration
def test_window_open_close(vstim_server: _ServerHandle) -> None:
    win = visual.Window(address=vstim_server.address, deferred=True)
    win.close()


@pytest.mark.integration
def test_circle_appears_on_screen(vstim_server: _ServerHandle) -> None:
    win = visual.Window(address=vstim_server.address, deferred=True)
    c = visual.Circle(win, radius=50)
    c.autoDraw = True
    win.flip()
    state = vstim_server.query_stimulus(c._handle)
    assert state.get("ok") is True
    assert state.get("enabled") is True
    assert state.get("radius") == pytest.approx(50.0)
    win.close()


@pytest.mark.integration
def test_rect_move(vstim_server: _ServerHandle) -> None:
    win = visual.Window(address=vstim_server.address, deferred=True)
    r = visual.Rect(win, width=100, height=50)
    r.pos = (200, 100)
    win.flip()
    state = vstim_server.query_stimulus(r._handle)
    assert state["pos"] == pytest.approx([200.0, 100.0])
    win.close()


@pytest.mark.integration
def test_deferred_batch_is_atomic(vstim_server: _ServerHandle) -> None:
    """Two setPos calls in one frame should both be applied before rendering."""
    win = visual.Window(address=vstim_server.address, deferred=True)
    c = visual.Circle(win, radius=30)
    c.pos = (100, 0)
    c.pos = (200, 0)  # overrides first within same frame
    win.flip()
    state = vstim_server.query_stimulus(c._handle)
    assert state["pos"] == pytest.approx([200.0, 0.0])
    win.close()
