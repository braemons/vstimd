from __future__ import annotations

import os
from dataclasses import dataclass, field

from vstimd._handles import StimulusHandle
from vstimd._proto.vstimd.v1 import scene3d_pb2
from vstimd.stimuli.vec import Vec3
from vstimd.stimuli.color import Color


@dataclass(order=True, repr=False)
class ServerVersion:
    """Semver triple reported by the server."""

    major: int
    minor: int
    patch: int

    def __repr__(self) -> str:
        return f"ServerVersion({self.major}, {self.minor}, {self.patch})"

    def __str__(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"


@dataclass
class DeferredModeStatus:
    """What a :meth:`SystemClient.set_deferred_mode` call actually did.

    Ending or cancelling deferred mode that was never begun is a no-op rather
    than an error — a client is allowed to say "off, whatever the state" — so
    the reply distinguishes the two, and says when the staged frame lands.
    """

    #: Deferred mode after the call: true only when it was just begun.
    deferred: bool
    #: A flip is queued and lands on the next vsync.
    flip_scheduled: bool
    #: Deferred mode before the call: `False` here with `active=False` means the
    #: call did nothing, because there was nothing staged to end.
    was_deferred: bool
    #: The frame the staged state is first drawn from; 0 when no flip is queued.
    #: Pass it to :meth:`SystemClient.wait_for_frame` to wait for exactly that.
    flip_frame: int
    #: The frame the server was on when it handled this call. On the call that
    #: begins deferred mode this is where the staging started; on the one that
    #: ends it, where it ended — with ``flip_frame`` saying where it lands.
    frame_count: int

    @property
    def was_a_no_op(self) -> bool:
        """True when the call found nothing to do — neither begun nor staged."""
        return not self.deferred and not self.was_deferred

    @property
    def frames_staged(self) -> int | None:
        """How many frames the staged batch spans, if this call ended one.

        ``None`` for the call that begins deferred mode, and for one that found
        nothing to end: there is no span to report.
        """
        if not self.flip_scheduled:
            return None
        return self.flip_frame - self.frame_count


@dataclass(repr=False)
class CapturedFrame:
    """One presented frame, returned by :meth:`SystemClient.capture_frame`.

    ``png`` is the frame exactly as it went to the display — overlay included —
    encoded as 8-bit RGB PNG.
    """

    png: bytes
    width_px: int
    height_px: int
    #: The frame index, in the numbering ``ServerResponse.frame_count`` and the
    #: event stream use.
    frame: int

    def save(self, path: str | os.PathLike[str]) -> None:
        """Write the PNG to ``path``."""
        with open(path, "wb") as f:
            f.write(self.png)

    def __repr__(self) -> str:
        return (
            f"CapturedFrame(frame={self.frame}, {self.width_px}x{self.height_px}, "
            f"{len(self.png)} bytes)"
        )


@dataclass(repr=False)
class ServerInfo:
    """Display and version information returned by :meth:`SystemClient.query_server_info`."""

    width_px: int
    height_px: int
    frame_rate_hz: float
    version: ServerVersion
    background_color: Color = field(default_factory=lambda: Color(0.0, 0.0, 0.0))

    def __repr__(self) -> str:
        return (
            f"ServerInfo(width_px={self.width_px}, height_px={self.height_px}, "
            f"frame_rate_hz={self.frame_rate_hz:.1f}, version={self.version})"
        )


@dataclass
class StimulusListEntry:
    """One entry returned by :meth:`SystemClient.list_stimuli`."""

    handle: StimulusHandle
    enabled: bool
    id: str
    name: str
    #: Conditions this stimulus is active in; empty means every condition.
    condition_indices: list[int] = field(default_factory=list)


@dataclass
class Camera3D:
    """The camera 3-D stimuli are seen through.

    World space is right-handed and Y-up, in centimetres. At zero rotation the
    camera looks down −Z. Positive ``yaw_deg`` turns the view left, positive
    ``pitch_deg`` tilts it up; they apply yaw, then pitch, then roll.
    """

    position_cm: Vec3 = field(default_factory=lambda: Vec3(0.0, 0.0, 0.0))
    yaw_deg: float = 0.0
    pitch_deg: float = 0.0
    roll_deg: float = 0.0
    #: Vertical field of view, in (0, 180); the horizontal one follows from the
    #: display's aspect ratio.
    fov_y_deg: float = 60.0
    near_cm: float = 1.0
    far_cm: float = 50_000.0

    def to_proto(self) -> scene3d_pb2.Camera3D:
        return scene3d_pb2.Camera3D(
            position_cm=self.position_cm.to_proto(),
            yaw_deg=self.yaw_deg,
            pitch_deg=self.pitch_deg,
            roll_deg=self.roll_deg,
            fov_y_deg=self.fov_y_deg,
            near_cm=self.near_cm,
            far_cm=self.far_cm,
        )

    @classmethod
    def from_proto(cls, proto: scene3d_pb2.Camera3D) -> Camera3D:
        return cls(
            position_cm=Vec3.from_proto(proto.position_cm),
            yaw_deg=proto.yaw_deg,
            pitch_deg=proto.pitch_deg,
            roll_deg=proto.roll_deg,
            fov_y_deg=proto.fov_y_deg,
            near_cm=proto.near_cm,
            far_cm=proto.far_cm,
        )


@dataclass
class Lighting3D:
    """Scene lighting for ``Shading.PHONG`` surfaces; unlit ones ignore it.

    Colours are linear RGB multipliers and may exceed 1.
    """

    ambient_color: Vec3 = field(default_factory=lambda: Vec3(0.1, 0.1, 0.1))
    #: The direction light travels, from the sun. Any non-zero length.
    sun_direction: Vec3 = field(default_factory=lambda: Vec3(-0.5, -1.0, -0.3))
    sun_color: Vec3 = field(default_factory=lambda: Vec3(1.0, 1.0, 1.0))

    def to_proto(self) -> scene3d_pb2.Lighting3D:
        return scene3d_pb2.Lighting3D(
            ambient_color=self.ambient_color.to_proto(),
            sun_direction=self.sun_direction.to_proto(),
            sun_color=self.sun_color.to_proto(),
        )

    @classmethod
    def from_proto(cls, proto: scene3d_pb2.Lighting3D) -> Lighting3D:
        return cls(
            ambient_color=Vec3.from_proto(proto.ambient_color),
            sun_direction=Vec3.from_proto(proto.sun_direction),
            sun_color=Vec3.from_proto(proto.sun_color),
        )
