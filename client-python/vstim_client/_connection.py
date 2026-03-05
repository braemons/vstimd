"""ZMQ socket wrapper: send/recv JSON commands, reconnect on error.

Uses REQ/REP pattern. Each call to send() expects exactly one reply.
send_batch() sends a ZMQ multipart message (one frame per command), used by
deferred mode so the server holds all changes until the deferred_flip sentinel
arrives and applies them simultaneously at the next render frame.
"""

from __future__ import annotations

import json
import logging
from typing import Any

log = logging.getLogger(__name__)


class Connection:
    """Wraps a zmq.REQ socket with JSON encoding and simple reconnect logic."""

    def __init__(self, address: str = "tcp://localhost:5555") -> None:
        self.address = address
        self._socket: Any = None  # zmq.Socket, lazy-imported
        self._context: Any = None
        self._connect()

    # ------------------------------------------------------------------
    # Connection lifecycle
    # ------------------------------------------------------------------

    def _connect(self) -> None:
        try:
            import zmq  # type: ignore[import]
        except ImportError as exc:
            raise ImportError(
                "pyzmq is required: pip install pyzmq"
            ) from exc

        self._context = zmq.Context.instance()
        self._socket = self._context.socket(zmq.REQ)
        self._socket.setsockopt(zmq.LINGER, 0)
        self._socket.connect(self.address)
        log.debug("Connected to %s", self.address)

    def reconnect(self) -> None:
        """Tear down and re-establish the socket."""
        self.close()
        self._connect()

    def close(self) -> None:
        if self._socket is not None:
            self._socket.close()
            self._socket = None
        if self._context is not None:
            self._context.term()
            self._context = None

    # ------------------------------------------------------------------
    # Send / receive
    # ------------------------------------------------------------------

    def send(self, payload: dict[str, Any]) -> dict[str, Any]:
        """Send one JSON command and return the server reply."""
        raw = json.dumps(payload).encode()
        self._socket.send(raw)
        reply_raw = self._socket.recv()
        reply: dict[str, Any] = json.loads(reply_raw)
        if not reply.get("ok", False):
            raise RuntimeError(
                f"Server error for cmd={payload.get('cmd')!r}: "
                f"{reply.get('error', 'unknown error')}"
            )
        return reply

    def send_batch(self, payloads: list[dict[str, Any]]) -> dict[str, Any]:
        """Send multiple commands as a single ZMQ multipart message.

        Used by deferred mode: the batch ends with a deferred_flip sentinel so
        the server applies all changes simultaneously at the next render frame.
        Returns the reply to the final frame.
        """
        if not payloads:
            return {"ok": True}
        frames = [json.dumps(p).encode() for p in payloads]
        self._socket.send_multipart(frames)
        reply_raw = self._socket.recv()
        reply: dict[str, Any] = json.loads(reply_raw)
        if not reply.get("ok", False):
            raise RuntimeError(
                f"Server batch error: {reply.get('error', 'unknown error')}"
            )
        return reply

    # ------------------------------------------------------------------
    # Context manager support
    # ------------------------------------------------------------------

    def __enter__(self) -> "Connection":
        return self

    def __exit__(self, *_: Any) -> None:
        self.close()
