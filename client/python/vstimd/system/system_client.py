from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2, system_pb2
from vstimd._proto.vstimd.v1 import input_pb2, scene3d_pb2
from vstimd._proto.vstimd.v1 import color_pb2
from vstimd.response import ServerResponse
from vstimd.stimuli.color import Color
from .zones_models import CameraZone, CameraZoneStatus
from .system_models import (
    Camera3D,
    InputDeviceInfo,
    Lighting3D,
    CapturedFrame,
    DeferredModeStatus,
    FrameStats,
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
            StimulusListEntry(
                handle=StimulusHandle(e.handle),
                enabled=e.enabled,
                id=e.id,
                name=e.name,
                condition_indices=list(e.condition_indices),
            )
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

    # ── Frame statistics ─────────────────────────────────────────────────────

    def query_frame_stats(self) -> FrameStats:
        """Frame timing since the last :meth:`reset_frame_stats` (or server start)."""
        resp = self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            query_frame_stats=system_pb2.QueryFrameStatsRequest(),
        ))
        return FrameStats.from_proto(resp.frame_stats)

    def reset_frame_stats(self) -> FrameStats:
        """Start a new frame-statistics window, returning the one it closes.

        Returning the closed window makes "what did this trial cost" a single
        round trip, with no frame able to fall between a query and a reset::

            conn.system.reset_frame_stats()
            run_trial()
            if conn.system.reset_frame_stats().dropped_frames:
                ...
        """
        resp = self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            reset_frame_stats=system_pb2.ResetFrameStatsRequest(),
        ))
        return FrameStats.from_proto(resp.frame_stats)

    # ── Input devices ────────────────────────────────────────────────────────

    def list_input_devices(self) -> list[InputDeviceInfo]:
        """The rig's input devices, with live backend, staleness and axis readings.

        Check ``stale`` before and during a session: a stale treadmill means its
        reader process has stopped, and the camera it drives is standing still.
        """
        resp = self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            list_input_devices=input_pb2.ListInputDevicesRequest(),
        ))
        return [InputDeviceInfo.from_proto(d) for d in resp.input_device_list.devices]

    # ── 3-D scene ────────────────────────────────────────────────────────────

    def set_camera(self, camera: Camera3D) -> ServerResponse:
        """Replace the camera 3-D stimuli are seen through. Respects deferred mode.

        Raises:
            InvalidArgumentError: a field is out of range (``fov_y_deg`` outside
                (0, 180), ``near_cm`` not below ``far_cm``, …).
        """
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            set_camera=scene3d_pb2.SetCameraRequest(camera=camera.to_proto()),
        )))

    def query_camera(self) -> Camera3D:
        """The camera as currently on screen."""
        resp = self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            query_camera=scene3d_pb2.QueryCameraRequest(),
        ))
        return Camera3D.from_proto(resp.camera)

    def set_camera_zones(self, zones: list[CameraZone]) -> ServerResponse:
        """Replace every camera zone; ``[]`` removes them all. See :class:`CameraZone`.

        Raises:
            InvalidArgumentError: an output line, a line or name used twice, or
                an inverted range.
        """
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            set_camera_zones=scene3d_pb2.SetCameraZonesRequest(zones=[z.to_proto() for z in zones]),
        )))

    def list_camera_zones(self) -> list[CameraZoneStatus]:
        """Every camera zone, and whether the camera was inside it last frame."""
        resp = self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            list_camera_zones=scene3d_pb2.ListCameraZonesRequest(),
        ))
        return [
            CameraZoneStatus(CameraZone.from_proto(z.zone), z.inside)
            for z in resp.camera_zone_list.zones
        ]

    def set_lighting(self, lighting: Lighting3D) -> ServerResponse:
        """Replace the scene lighting. Respects deferred mode.

        Raises:
            InvalidArgumentError: ``sun_direction`` is zero.
        """
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            set_lighting=scene3d_pb2.SetLightingRequest(lighting=lighting.to_proto()),
        )))

    def query_lighting(self) -> Lighting3D:
        resp = self._send(service_pb2.Request(
            system=service_pb2.SystemTarget(),
            query_lighting=scene3d_pb2.QueryLightingRequest(),
        ))
        return Lighting3D.from_proto(resp.lighting)

    # ── Frame capture ────────────────────────────────────────────────────────

    def capture_frame(self) -> CapturedFrame:
        """Capture the next presented frame as a PNG.

        The frame includes every command acknowledged before this call, and
        is read back from the server's own swapchain — exactly what went to
        the display, overlay included. Takes about one frame plus the readback.

        Example::

            conn.system.capture_frame().save("frame.png")

        Raises:
            NotSupportedError: the server renders nothing to capture (null
                renderer, evdi).
            NotReadyError: no frame was rendered within a few seconds, e.g.
                a minimised window. Retrying is safe.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            capture_frame=system_pb2.CaptureFrameRequest(),
        )
        f = self._send(req).captured_frame
        return CapturedFrame(
            png=f.png, width_px=f.width_px, height_px=f.height_px, frame=f.frame
        )

    def wait_until(self, server_time_ns: int) -> ServerResponse:
        """Block until the server's monotonic clock reaches `server_time_ns`."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            wait_until=system_pb2.WaitUntilRequest(server_time_ns=server_time_ns),
        )
        return ServerResponse._from_proto(self._send(req))
