"""Shared pytest fixtures: MockSocket, FakeWindow."""

from __future__ import annotations

import json
from typing import Any
from unittest.mock import MagicMock

import pytest

from vstim_client._connection import Connection
from vstim_client import visual


# ---------------------------------------------------------------------------
# MockSocket / MockConnection
# ---------------------------------------------------------------------------

class MockSocket:
    """Captures sent commands for assertion in tests."""

    def __init__(self) -> None:
        self._sent: list[dict[str, Any]] = []

    def send(self, raw: bytes) -> None:
        self._sent.append(json.loads(raw))

    def recv(self) -> bytes:
        return b'{"ok": true}'

    def send_multipart(self, frames: list[bytes]) -> None:
        for frame in frames:
            self._sent.append(json.loads(frame))

    def close(self) -> None:
        pass

    # --- Inspection helpers ---

    def sent_commands(self) -> list[dict[str, Any]]:
        """Return a copy of all captured commands."""
        return list(self._sent)

    def commands_by(self, cmd: str) -> list[dict[str, Any]]:
        return [c for c in self._sent if c.get("cmd") == cmd]

    def last_cmd(self, cmd: str) -> dict[str, Any] | None:
        cmds = self.commands_by(cmd)
        return cmds[-1] if cmds else None

    def clear(self) -> None:
        self._sent.clear()


class MockConnection(Connection):
    """Connection subclass that substitutes a MockSocket."""

    def __init__(self) -> None:
        # Bypass real zmq — store a mock socket directly
        self._socket = MockSocket()
        self._context = None
        self.address = "tcp://mock:5555"

    def _connect(self) -> None:
        pass

    def send(self, payload: dict[str, Any]) -> dict[str, Any]:
        raw = json.dumps(payload).encode()
        self._socket.send(raw)
        return {"ok": True}

    def send_batch(self, payloads: list[dict[str, Any]]) -> dict[str, Any]:
        frames = [json.dumps(p).encode() for p in payloads]
        self._socket.send_multipart(frames)
        return {"ok": True}

    def close(self) -> None:
        pass


# ---------------------------------------------------------------------------
# FakeWindow fixture
# ---------------------------------------------------------------------------

def _make_mock_window(**kwargs: Any) -> visual.Window:
    """Return a Window whose ZMQ connection is replaced by MockConnection."""
    win = object.__new__(visual.Window)
    # Manually initialise fields, bypassing __init__ to avoid real ZMQ connect
    win.size = kwargs.get("size", (800, 600))
    win.pos = kwargs.get("pos", None)
    win.colorSpace = kwargs.get("colorSpace", "rgb")
    win.fullscr = kwargs.get("fullscr", False)
    win.monitor = kwargs.get("monitor", None)
    win.units = kwargs.get("units", "pix")
    win.screen = kwargs.get("screen", 0)
    win.waitBlanking = kwargs.get("waitBlanking", True)
    win.name = kwargs.get("name", "test_window")
    win.title = kwargs.get("title", "test")
    win.deferred = kwargs.get("deferred", True)
    win.address = "tcp://mock:5555"
    win.autoLog = False
    win._color = [0.0, 0.0, 0.0, 1.0]
    win._queue = []
    win._to_draw_once = set()
    conn = MockConnection()
    win._connection = conn
    return win


@pytest.fixture
def mock_win() -> visual.Window:
    """A Window with a MockConnection (no real ZMQ)."""
    return _make_mock_window()


@pytest.fixture
def mock_socket(mock_win: visual.Window) -> MockSocket:
    """Direct access to the underlying MockSocket."""
    return mock_win._connection._socket  # type: ignore[attr-defined]
