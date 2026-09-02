from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2, system_pb2
from vstimd._proto.vstimd.v1 import color_pb2
from vstimd.response import ServerResponse
from vstimd.stimuli.color import Color
from .system_models import (
    DeferredModeStatus,
    ServerInfo,
    ServerVersion,
    StimulusListEntry,
)


_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class SystemClient:
    """Scene-wide commands and server queries.

    Accessed as ``conn.system`` on a :class:`~vstimd.Connection` instance.

    Example::

        with Connection() as conn:
            info = conn.system.query_server_info()
            print(info.width_px, info.height_px, info.frame_rate_hz)
            conn.system.set_background(0.0, 0.0, 0.0)
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    # ── Queries ───────────────────────────────────────────────────────────────

    def query_server_info(self) -> ServerInfo:
        """Query server display properties and version."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            query_server_info=system_pb2.QueryServerInfoRequest(),
        )
        resp = self._send(req)
        info = resp.server_info
        v = info.version
        bg = info.background_color
        return ServerInfo(
            width_px=info.width_px,
            height_px=info.height_px,
            frame_rate_hz=info.frame_rate_hz,
            version=ServerVersion(v.major, v.minor, v.patch),
            background_color=Color(r=bg.r, g=bg.g, b=bg.b, a=bg.a),
        )

    def list_stimuli(self) -> list[StimulusListEntry]:
        """Return a list of all currently existing stimuli."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            list_stimuli=system_pb2.ListStimuliRequest(),
        )
        resp = self._send(req)
        return [
            StimulusListEntry(handle=StimulusHandle(e.handle), enabled=e.enabled, id=e.id, name=e.name)
            for e in resp.stimulus_list.entries
        ]

    # ── Scene mutations ───────────────────────────────────────────────────────

    def set_background(self, r: float, g: float, b: float, a: float = 1.0) -> ServerResponse:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            set_background=system_pb2.SetBackgroundRequest(
                color=color_pb2.Color(r=r, g=g, b=b, a=a)
            ),
        )
        return ServerResponse._from_proto(self._send(req))

    def set_deferred_mode(
        self, active: bool, *, cancel: bool = False
    ) -> DeferredModeStatus:
        """Begin, end or cancel deferred mode, and report what that did.

        ``active=True`` begins it: writes are staged rather than drawn.
        ``active=False`` ends it, queueing one atomic flip for the next vsync.
        ``cancel=True`` throws the staged state away instead.

        Ending or cancelling a mode that was never begun does nothing at all —
        which is worth knowing, so the reply says whether it had been on and
        which frame any flip lands on::

            begun = conn.system.set_deferred_mode(True)   # staging from frame N
            ...
            ended = conn.system.set_deferred_mode(False)  # lands on flip_frame
            if ended.flip_scheduled:
                conn.system.wait_for_frame(ended.flip_frame)  # now it is drawn
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            set_deferred_mode=system_pb2.SetDeferredModeRequest(active=active, cancel=cancel),
        )
        resp = self._send(req)
        status = resp.deferred_mode
        return DeferredModeStatus(
            deferred=status.deferred,
            flip_scheduled=status.flip_scheduled,
            was_deferred=status.was_deferred,
            flip_frame=status.flip_frame,
            frame_count=resp.frame_count,
        )

    def clear_stimuli(self) -> ServerResponse:
        """Remove every unprotected stimulus. Animations are left alone."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            clear_stimuli=system_pb2.ClearStimuliRequest(),
        )
        return ServerResponse._from_proto(self._send(req))

    def clear_animations(self) -> ServerResponse:
        """Remove every animation, whatever its state. Stimuli are left alone."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            clear_animations=system_pb2.ClearAnimationsRequest(),
        )
        return ServerResponse._from_proto(self._send(req))

    def clear_all(self) -> ServerResponse:
        """Clear the scene: every animation, then every unprotected stimulus.

        Scene-wide settings survive — background and default colours, the
        photodiode patch, and the VTL name map.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            clear_all=system_pb2.ClearAllRequest(),
        )
        return ServerResponse._from_proto(self._send(req))

    def set_all_enabled(self, enabled: bool) -> ServerResponse:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            set_all_enabled=system_pb2.SetAllEnabledRequest(enabled=enabled),
        )
        return ServerResponse._from_proto(self._send(req))

    # ── Lifecycle ─────────────────────────────────────────────────────────────

    def shutdown(self) -> ServerResponse:
        """Ask the server to exit cleanly.

        The server acknowledges first, then finishes the current frame, tears
        down Vulkan and the VT, and exits — equivalent to sending it SIGTERM.
        Subsequent requests on this connection will not be answered.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            shutdown=system_pb2.ShutdownRequest(),
        )
        return ServerResponse._from_proto(self._send(req))

    # ── Timing ───────────────────────────────────────────────────────────────

    def wait_for_frames(self, count: int) -> ServerResponse:
        """Block until `count` additional render frames have completed."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            wait_for_frames=system_pb2.WaitForFramesRequest(count=count),
        )
        return ServerResponse._from_proto(self._send(req))

    def wait_for_frame(self, frame_count: int) -> ServerResponse:
        """Block until the server has drawn frame number ``frame_count``.

        The protocol's wait is relative (``wait_for_frames(n)``), but the
        interesting number is often absolute — the frame a deferred flip lands
        on, say. Every response carries the current count, so the remaining
        distance is recomputed until it is covered.
        """
        resp = self.wait_for_frames(0)
        while resp.frame_count < frame_count:
            resp = self.wait_for_frames(frame_count - resp.frame_count)
        return resp

    def wait_until(self, server_time_ns: int) -> ServerResponse:
        """Block until the server's monotonic clock reaches `server_time_ns`."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            wait_until=system_pb2.WaitUntilRequest(server_time_ns=server_time_ns),
        )
        return ServerResponse._from_proto(self._send(req))
