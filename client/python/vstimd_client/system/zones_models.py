"""Camera zones: regions of the 3-D world that act as trigger-line inputs."""

from __future__ import annotations

from dataclasses import dataclass

from vstimd_client._proto.vstimd.v1 import scene3d_pb2
from vstimd_client.vtl.vtl_client import VtlHandle
from vstimd_client.vtl.vtl_models import VtlKind


@dataclass(frozen=True)
class CameraZone:
    """A region whose entry and exit by the 3-D camera drive an input trigger line.

    While the camera is inside, the line reads HIGH; entering is a rising edge
    and leaving a falling one. Use the line anywhere a trigger line goes — an
    animation's ``start_trigger``, ``create_enable_on_trigger_edge``,
    ``create_couple_visibility_to_trigger_line`` — and have that animation pulse
    an output line to reach the DAQ. Pick an input line the DAQ does not drive.

    Bounds are world-space centimetres; an axis left at ``None`` is unbounded.
    With a wrapping corridor, keep zones within one period ``[0, period_cm)``:
    they then fire once per lap. The camera is tested as last rendered, so a
    reaction starts one frame after the crossing.
    """

    name: str
    line: VtlHandle
    x_cm: tuple[float, float] | None = None
    y_cm: tuple[float, float] | None = None
    z_cm: tuple[float, float] | None = None

    def to_proto(self) -> scene3d_pb2.CameraZone:
        z = scene3d_pb2.CameraZone(name=self.name, line=self.line._to_proto())
        for axis, r in (("x", self.x_cm), ("y", self.y_cm), ("z", self.z_cm)):
            if r is not None:
                setattr(z, f"{axis}_min_cm", r[0])
                setattr(z, f"{axis}_max_cm", r[1])
        return z

    @classmethod
    def from_proto(cls, z: scene3d_pb2.CameraZone) -> CameraZone:
        def rng(axis: str) -> tuple[float, float] | None:
            lo, hi = f"{axis}_min_cm", f"{axis}_max_cm"
            if z.HasField(lo) and z.HasField(hi):  # type: ignore[arg-type]
                return (getattr(z, lo), getattr(z, hi))
            return None

        bb = z.line.bank_bit
        return cls(
            name=z.name,
            line=VtlHandle(VtlKind.INPUT, bank=bb.bank, bit=bb.bit),
            x_cm=rng("x"),
            y_cm=rng("y"),
            z_cm=rng("z"),
        )


@dataclass(frozen=True)
class CameraZoneStatus:
    zone: CameraZone
    #: The camera was inside on the last frame.
    inside: bool
