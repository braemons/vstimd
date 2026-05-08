"""Low-level ZMQ + protobuf connection to wonderlamp_server."""

from __future__ import annotations

import sys
import os

# The generated stubs use bare `from v1 import ...` imports.
_PROTO_DIR = os.path.join(os.path.dirname(__file__), "_proto")
if _PROTO_DIR not in sys.path:
    sys.path.insert(0, _PROTO_DIR)

import zmq  # type: ignore[import]

from v1 import service_pb2 as _svc
from v1 import stimuli_pb2 as _stim
from v1 import system_pb2 as _sys
from v1 import common_pb2 as _common


class ServerVersion:
    """Version triple returned by :meth:`Connection.query_server_info`."""

    def __init__(self, major: int, minor: int, patch: int) -> None:
        self.major = major
        self.minor = minor
        self.patch = patch

    def __repr__(self) -> str:
        return f"ServerVersion({self.major}, {self.minor}, {self.patch})"

    def __str__(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"

    def __eq__(self, other: object) -> bool:
        if not isinstance(other, ServerVersion):
            return NotImplemented
        return (self.major, self.minor, self.patch) == (other.major, other.minor, other.patch)

    def __lt__(self, other: "ServerVersion") -> bool:
        return (self.major, self.minor, self.patch) < (other.major, other.minor, other.patch)


class ServerInfo:
    """Display and version information returned by :meth:`Connection.query_server_info`."""

    def __init__(
        self,
        width: int,
        height: int,
        frame_rate: float,
        version: ServerVersion,
    ) -> None:
        self.width = width
        self.height = height
        self.frame_rate = frame_rate
        self.version = version

    def __repr__(self) -> str:
        return (
            f"ServerInfo(width={self.width}, height={self.height}, "
            f"frame_rate={self.frame_rate:.1f}, version={self.version})"
        )


class Connection:
    """ZMQ REQ socket connected to a single wonderlamp_server instance.

    Parameters
    ----------
    address:
        ZMQ endpoint of the server (default ``tcp://localhost:5555``).
    """

    def __init__(self, address: str = "tcp://localhost:5555") -> None:
        self._ctx = zmq.Context.instance()
        self._sock = self._ctx.socket(zmq.REQ)
        self._sock.setsockopt(zmq.LINGER, 0)
        self._sock.connect(address)

    # ── internal ──────────────────────────────────────────────────────────────

    def _send(self, req: _svc.Request) -> _svc.Response:
        self._sock.send(req.SerializeToString())
        raw = self._sock.recv()
        resp = _svc.Response()
        resp.ParseFromString(raw)
        if resp.error:
            raise RuntimeError(f"server error: {resp.error}")
        return resp

    # ── queries ───────────────────────────────────────────────────────────────

    def query_server_info(self) -> ServerInfo:
        """Query server display properties and version."""
        req = _svc.Request(
            system=_svc.SystemTarget(),
            query_server_info=_sys.QueryServerInfo(),
        )
        resp = self._send(req)
        info = resp.server_info
        v = info.version
        return ServerInfo(
            width=info.width,
            height=info.height,
            frame_rate=info.frame_rate,
            version=ServerVersion(v.major, v.minor, v.patch),
        )

    # ── commands ──────────────────────────────────────────────────────────────

    def create_circle(
        self,
        *,
        x: float = 0.0,
        y: float = 0.0,
        radius: float = 50.0,
        r: float = 1.0,
        g: float = 1.0,
        b: float = 1.0,
        a: float = 1.0,
    ) -> int:
        """Create a disc stimulus and return its handle."""
        req = _svc.Request(
            system=_svc.SystemTarget(),
            create_circle=_stim.CreateCircle(
                center=_common.Vec2(x=x, y=y),
                radius=radius,
                fill=_common.Color(r=r, g=g, b=b, a=a),
            ),
        )
        return self._send(req).handle

    def create_rect(
        self,
        *,
        x: float = 0.0,
        y: float = 0.0,
        width: float = 100.0,
        height: float = 100.0,
        r: float = 1.0,
        g: float = 1.0,
        b: float = 1.0,
        a: float = 1.0,
    ) -> int:
        """Create a rectangle stimulus and return its handle."""
        req = _svc.Request(
            system=_svc.SystemTarget(),
            create_rect=_stim.CreateRect(
                center=_common.Vec2(x=x, y=y),
                width=width,
                height=height,
                fill=_common.Color(r=r, g=g, b=b, a=a),
            ),
        )
        return self._send(req).handle

    def set_enabled(self, handle: int, enabled: bool) -> None:
        """Enable or disable the stimulus identified by *handle*."""
        req = _svc.Request(
            stimulus=handle,
            set_enabled=_stim.SetEnabled(enabled=enabled),
        )
        self._send(req)

    def delete(self, handle: int) -> None:
        """Permanently remove the stimulus identified by *handle*."""
        req = _svc.Request(stimulus=handle, delete=_stim.Delete())
        self._send(req)

    # ── context manager ───────────────────────────────────────────────────────

    def __enter__(self) -> "Connection":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def close(self) -> None:
        """Close the ZMQ socket."""
        self._sock.close()
