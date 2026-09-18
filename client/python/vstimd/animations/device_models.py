"""Models for animations driven by rig input devices."""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum

from vstimd._proto.vstimd.v1 import animations_pb2


class TransformChannel(StrEnum):
    """What an :class:`AxisMap` drives."""

    #: Position: pixels on a 2-D stimulus, cm on a 3-D stimulus or the camera.
    POS_X = "pos_x"
    POS_Y = "pos_y"
    POS_Z = "pos_z"
    #: Degrees. ``YAW`` is a 2-D stimulus' rotation.
    YAW = "yaw"
    PITCH = "pitch"
    ROLL = "roll"
    #: 3-D stimuli only.
    SCALE_X = "scale_x"
    SCALE_Y = "scale_y"
    SCALE_Z = "scale_z"
    SCALE_UNIFORM = "scale_uniform"
    #: The camera's horizontal forward / rightward direction, cm. Camera only,
    #: and rate or cumulative axes only.
    FORWARD = "forward"
    STRAFE = "strafe"


_CHANNEL_TO_PROTO: dict[TransformChannel, animations_pb2.TransformChannel] = {
    TransformChannel.POS_X: animations_pb2.TRANSFORM_CHANNEL_POS_X,
    TransformChannel.POS_Y: animations_pb2.TRANSFORM_CHANNEL_POS_Y,
    TransformChannel.POS_Z: animations_pb2.TRANSFORM_CHANNEL_POS_Z,
    TransformChannel.YAW: animations_pb2.TRANSFORM_CHANNEL_YAW,
    TransformChannel.PITCH: animations_pb2.TRANSFORM_CHANNEL_PITCH,
    TransformChannel.ROLL: animations_pb2.TRANSFORM_CHANNEL_ROLL,
    TransformChannel.SCALE_X: animations_pb2.TRANSFORM_CHANNEL_SCALE_X,
    TransformChannel.SCALE_Y: animations_pb2.TRANSFORM_CHANNEL_SCALE_Y,
    TransformChannel.SCALE_Z: animations_pb2.TRANSFORM_CHANNEL_SCALE_Z,
    TransformChannel.SCALE_UNIFORM: animations_pb2.TRANSFORM_CHANNEL_SCALE_UNIFORM,
    TransformChannel.FORWARD: animations_pb2.TRANSFORM_CHANNEL_FORWARD,
    TransformChannel.STRAFE: animations_pb2.TRANSFORM_CHANNEL_STRAFE,
}
_CHANNEL_FROM_PROTO = {v: k for k, v in _CHANNEL_TO_PROTO.items()}


@dataclass(frozen=True)
class AxisRef:
    """One axis of a rig-config input device, both by name."""

    device: str
    axis: str

    def to_proto(self) -> animations_pb2.AxisRef:
        return animations_pb2.AxisRef(device=self.device, axis=self.axis)


@dataclass(frozen=True)
class AxisMap:
    """One device axis driving one transform channel.

    The axis' semantic, declared in the rig-config, decides how its value
    becomes a channel update: an absolute axis sets the channel, a cumulative
    axis adds its change, a rate axis adds value × frame time.
    """

    axis: str
    channel: TransformChannel
    #: Device units → px, cm or degrees.
    gain: float = 1.0
    #: Absolute and rate values within ±deadzone read as zero.
    deadzone: float = 0.0
    invert: bool = False
    #: Keep the channel within ``(min, max)``.
    clamp: tuple[float, float] | None = None
    #: Wrap the channel into ``[0, wrap)`` — a corridor period.
    wrap: float | None = None

    def to_proto(self) -> animations_pb2.AxisMap:
        m = animations_pb2.AxisMap(
            axis=self.axis,
            channel=_CHANNEL_TO_PROTO[self.channel],
            gain=self.gain,
            deadzone=self.deadzone,
            invert=self.invert,
            wrap=self.wrap or 0.0,
        )
        if self.clamp is not None:
            m.clamp_min, m.clamp_max = self.clamp
        return m

    @classmethod
    def from_proto(cls, m: animations_pb2.AxisMap) -> AxisMap:
        clamp = (m.clamp_min, m.clamp_max) if m.HasField("clamp_min") and m.HasField("clamp_max") else None
        return cls(
            axis=m.axis,
            channel=_CHANNEL_FROM_PROTO[m.channel],
            gain=m.gain,
            deadzone=m.deadzone,
            invert=m.invert,
            clamp=clamp,
            wrap=m.wrap or None,
        )
