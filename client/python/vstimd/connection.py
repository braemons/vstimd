from __future__ import annotations

import time
import zmq  # type: ignore[import]
from google.protobuf.message import DecodeError

from vstimd._proto import service_pb2
from vstimd.stimuli import RectParams, ShapeAppearance, StimuliClient
from vstimd.system import SystemClient
from vstimd.vtl import VtlClient
from vstimd.animations import AnimationClient
from vstimd.conditions import ConditionsClient
from vstimd.scene_config import SceneConfigClient
from vstimd.exceptions import ProtocolError, error_for_code


def _request_context(req: service_pb2.Request) -> tuple[str | None, int | None]:
    """Name the command in *req* and the stimulus it addressed, for an error.

    "handle not found" on its own leaves you grepping the script for which of
    twenty mutations it came from; "handle not found (set_position, handle 7)"
    does not.
    """
    command = req.WhichOneof("body")
    handle = req.stimulus if req.WhichOneof("target") == "stimulus" else None
    return command, handle


class Connection:
    """ZMQ REQ socket connected to a single vstimd instance.

    Client sub-objects are available as attributes and cover the full command
    API:

    * ``stimuli`` — :class:`~vstimd.stimuli.StimuliClient`: create and mutate stimuli
    * ``system`` — :class:`~vstimd.system.SystemClient`: scene-wide commands and server queries
    * ``vtl`` — :class:`~vstimd.VtlClient`: Virtual Trigger Line control
    * ``animations`` — :class:`~vstimd.AnimationClient`: frame-accurate animation sequences
    * ``scene_config`` — :class:`~vstimd.scene_config.SceneConfigClient`: save, load, and
      retrieve named scene-configs
    * ``conditions`` — :class:`~vstimd.conditions.ConditionsClient`: switch between
      experimental conditions

    Example::

        with Connection() as conn:
            h = conn.stimuli.shapes.create_rect(
                params=RectParams(width_px=200, height_px=100,
                                  appearance=ShapeAppearance(fill_color=Color(1, 0, 0))),
            )
            conn.vtl.set_line_name(0, 0, VtlKind.OUTPUT, "frame_sync")
            anim = conn.animations.create_flash(h, duration_ms=500)
            conn.animations.arm(anim)

    Parameters
    ----------
    address:
        ZMQ endpoint of the server (default ``tcp://localhost:5555``).
    recv_timeout_s:
        If set, every request gives up after this many seconds and raises
        ``zmq.Again``.  ``None`` (default) blocks forever — the right choice
        for commands like :meth:`SystemClient.wait_for_frames` that are
        expected to block.
    """

    def __init__(
        self,
        address: str = "tcp://localhost:5555",
        *,
        wait_ready: bool = False,
        ready_timeout_s: float = 30.0,
        recv_timeout_s: float | None = None,
    ) -> None:
        self._address = address
        self._recv_timeout_ms = -1 if recv_timeout_s is None else max(1, int(recv_timeout_s * 1000))
        self._ctx = zmq.Context.instance()
        self._sock = self._connect()
        self.stimuli = StimuliClient(self._send)
        self.system = SystemClient(self._send)
        self.vtl = VtlClient(self._send)
        self.animations = AnimationClient(
            self._send,
            fps_getter=lambda: self.system.query_server_info().frame_rate_hz,
        )
        self.scene_config = SceneConfigClient(self._send)
        self.conditions = ConditionsClient(self._send)
        if wait_ready:
            self.wait_until_ready(timeout_s=ready_timeout_s)

    def _connect(self) -> "zmq.Socket":
        sock = self._ctx.socket(zmq.REQ)
        sock.setsockopt(zmq.LINGER, 0)
        sock.setsockopt(zmq.RCVTIMEO, self._recv_timeout_ms)
        sock.connect(self._address)
        return sock

    @property
    def address(self) -> str:
        """ZMQ endpoint this connection was opened against."""
        return self._address

    def _send(self, req: service_pb2.Request) -> service_pb2.Response:
        """Send one request and return the reply, raising if it is an error.

        The single choke point for error handling: no caller anywhere in the
        client sees a non-OK response, so none of them has to remember to look.
        """
        command, handle = _request_context(req)
        self._sock.send(req.SerializeToString())
        try:
            raw = self._sock.recv()
        except zmq.Again as exc:
            # A REQ socket that timed out waiting is stuck: it owes a reply it
            # will never get, and the next send would fail with EFSM. Reset it
            # here, once, rather than leaving every caller to know that.
            self._sock.close()
            self._sock = self._connect()
            raise TimeoutError(
                f"no reply from {self._address} within "
                f"{self._recv_timeout_ms / 1000:.1f}s ({command})"
            ) from exc

        resp = service_pb2.Response()
        try:
            resp.ParseFromString(raw)
        except DecodeError as exc:
            raise ProtocolError(
                f"could not decode the {len(raw)}-byte reply — is something "
                "other than vstimd listening on this address?",
                command=command,
                handle=handle,
            ) from exc

        if resp.code != service_pb2.ERROR_CODE_OK:
            raise error_for_code(resp.code, resp.error, command=command, handle=handle)
        return resp

    def __enter__(self) -> "Connection":
        return self

    def __exit__(self, *_: object) -> None:
        self.close()

    def wait_until_ready(
        self,
        timeout_s: float = 30.0,
        *,
        retry_interval_s: float = 0.5,
    ) -> None:
        """Block until the server is up and has rendered at least one frame.

        Retries the ZMQ connection if the server is not yet running.
        Raises ``TimeoutError`` if the server is not ready within *timeout_s*.
        """
        deadline = time.monotonic() + timeout_s
        attempt_ms = max(1, int(retry_interval_s * 1000))

        while True:
            if time.monotonic() >= deadline:
                raise TimeoutError(f"vstimd server not ready after {timeout_s}s")
            self._sock.setsockopt(zmq.RCVTIMEO, attempt_ms)
            try:
                self.system.wait_for_frames(1)
                return
            except TimeoutError:
                pass  # _send has already reset the socket for the next attempt
            finally:
                self._sock.setsockopt(zmq.RCVTIMEO, self._recv_timeout_ms)

    def close(self) -> None:
        """Close the ZMQ socket."""
        self._sock.close()
