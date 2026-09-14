"""3-D stimuli: cube, sphere and plane.

World space is right-handed and Y-up, in centimetres. The default camera sits at
the origin looking down −Z, so an object at ``Vec3(0, 0, -60)`` is 60 cm straight
ahead. 3-D stimuli are drawn underneath every 2-D stimulus.

Sizes are full extents, like every size in vstimd: a sphere is sized by its
``diameter_cm``, a cube by ``size_cm``. ``Transform3D.scale`` multiplies them.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum

from vstimd._proto.vstimd.v1 import transform_pb2
from vstimd._proto.vstimd.v1.stimuli import shapes3d_pb2

from .color import Color
from .vec import Vec2, Vec3


class Shading(StrEnum):
    """How a 3-D surface is lit."""

    #: The albedo, exactly — no lighting. An unlit surface writes the same pixel
    #: values as a 2-D shape of the same colour.
    UNLIT = "unlit"
    #: Diffuse and specular light from the scene's one directional light, plus
    #: its ambient term (see ``conn.system.set_lighting``).
    PHONG = "phong"


_SHADING_TO_PROTO: dict[Shading, shapes3d_pb2.Shading] = {
    Shading.UNLIT: shapes3d_pb2.SHADING_UNLIT,
    Shading.PHONG: shapes3d_pb2.SHADING_PHONG,
}
_SHADING_FROM_PROTO: dict[int, Shading] = {v: k for k, v in _SHADING_TO_PROTO.items()}


def _zero3() -> Vec3:
    return Vec3(0.0, 0.0, 0.0)


@dataclass
class Transform3D:
    """Where a 3-D stimulus sits in world space."""

    position_cm: Vec3 = field(default_factory=_zero3)
    #: ``Vec3(yaw, pitch, roll)`` in degrees, applied yaw (about world +Y), then
    #: pitch (the object's +X), then roll (its +Z). Positive yaw turns
    #: counter-clockwise seen from above.
    rotation_deg: Vec3 = field(default_factory=_zero3)
    #: Per-axis multiplier on the geometry's size. Must not be negative.
    scale: Vec3 = field(default_factory=lambda: Vec3(1.0, 1.0, 1.0))

    def to_proto(self) -> transform_pb2.Transform3D:
        return transform_pb2.Transform3D(
            position_cm=self.position_cm.to_proto(),
            rotation_deg=self.rotation_deg.to_proto(),
            scale=self.scale.to_proto(),
        )

    @classmethod
    def from_proto(cls, proto: transform_pb2.Transform3D) -> Transform3D:
        return cls(
            position_cm=Vec3.from_proto(proto.position_cm),
            rotation_deg=Vec3.from_proto(proto.rotation_deg),
            scale=Vec3.from_proto(proto.scale),
        )


@dataclass
class Material3D:
    """How a 3-D surface looks."""

    albedo: Color = field(default_factory=lambda: Color(1.0, 1.0, 1.0))
    #: Linear RGB added after shading, for a surface that must reach a given
    #: luminance whatever the lighting.
    emissive: Vec3 = field(default_factory=_zero3)
    shading: Shading = Shading.UNLIT

    def to_proto(self) -> shapes3d_pb2.Material3D:
        return shapes3d_pb2.Material3D(
            albedo=self.albedo.to_proto(),
            emissive=self.emissive.to_proto(),
            shading=_SHADING_TO_PROTO[self.shading],
        )

    @classmethod
    def from_proto(cls, proto: shapes3d_pb2.Material3D) -> Material3D:
        return cls(
            albedo=Color.from_proto(proto.albedo),
            emissive=Vec3.from_proto(proto.emissive),
            shading=_SHADING_FROM_PROTO.get(proto.shading, Shading.UNLIT),
        )


@dataclass
class Cube3DParams:
    #: Full extents along x, y and z.
    size_cm: Vec3 = field(default_factory=lambda: Vec3(10.0, 10.0, 10.0))
    material: Material3D = field(default_factory=Material3D)

    def to_proto(self) -> shapes3d_pb2.Cube3DParams:
        return shapes3d_pb2.Cube3DParams(
            size_cm=self.size_cm.to_proto(), material=self.material.to_proto()
        )

    @classmethod
    def from_proto(cls, proto: shapes3d_pb2.Cube3DParams) -> Cube3DParams:
        return cls(
            size_cm=Vec3.from_proto(proto.size_cm),
            material=Material3D.from_proto(proto.material),
        )


@dataclass
class Sphere3DParams:
    diameter_cm: float = 10.0
    #: Latitude bands. Spheres with the same ``rings`` and ``sectors`` share one
    #: GPU mesh, whatever their size. At most 256.
    rings: int = 16
    #: Longitude slices. At most 256.
    sectors: int = 32
    material: Material3D = field(default_factory=Material3D)

    def to_proto(self) -> shapes3d_pb2.Sphere3DParams:
        return shapes3d_pb2.Sphere3DParams(
            diameter_cm=self.diameter_cm,
            rings=self.rings,
            sectors=self.sectors,
            material=self.material.to_proto(),
        )

    @classmethod
    def from_proto(cls, proto: shapes3d_pb2.Sphere3DParams) -> Sphere3DParams:
        return cls(
            diameter_cm=proto.diameter_cm,
            rings=proto.rings,
            sectors=proto.sectors,
            material=Material3D.from_proto(proto.material),
        )


@dataclass
class Plane3DParams:
    """A flat rectangle in the object's XZ plane, facing +Y — a floor as placed.

    Visible from its front side only.
    """

    #: ``x``: extent along X; ``y``: extent along Z.
    size_cm: Vec2 = field(default_factory=lambda: Vec2(100.0, 100.0))
    material: Material3D = field(default_factory=Material3D)

    def to_proto(self) -> shapes3d_pb2.Plane3DParams:
        return shapes3d_pb2.Plane3DParams(
            size_cm=self.size_cm.to_proto(), material=self.material.to_proto()
        )

    @classmethod
    def from_proto(cls, proto: shapes3d_pb2.Plane3DParams) -> Plane3DParams:
        return cls(
            size_cm=Vec2.from_proto(proto.size_cm),
            material=Material3D.from_proto(proto.material),
        )
