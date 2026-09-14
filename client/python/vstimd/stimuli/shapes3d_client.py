from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1.stimuli import shapes3d_pb2
from vstimd.response import ServerResponse

from .shapes3d_models import (
    Corridor3DParams,
    Cube3DParams,
    Material3D,
    Plane3DParams,
    Sphere3DParams,
    Transform3D,
)
from .stimulus_identity import StimulusIdentity
from .vec import Vec2, Vec3

_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class Shapes3DClient:
    """Create and mutate 3-D stimuli: cubes, spheres and planes.

    Accessed as ``conn.stimuli.shapes3d``. 3-D stimuli are seen through the scene
    camera (``conn.system.set_camera``) and drawn underneath every 2-D stimulus.
    Placement is a :class:`Transform3D` in centimetres; the 2-D
    ``set_position`` / ``set_rotation`` refuse a 3-D stimulus, and
    :meth:`set_transform` replaces them. ``set_alpha``, ``set_enabled`` and
    ``delete`` work as for any stimulus.

    Example::

        ball = conn.stimuli.shapes3d.create_sphere(
            transform=Transform3D(position_cm=Vec3(0, 0, -60)),
            params=Sphere3DParams(
                diameter_cm=20,
                material=Material3D(albedo=Color(0.2, 0.6, 1.0), shading=Shading.PHONG),
            ),
        )
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    # ── Creation ──────────────────────────────────────────────────────────────

    def create_cube(
        self,
        *,
        name: str = "",
        transform: Transform3D | None = None,
        params: Cube3DParams | None = None,
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_cube_3d=shapes3d_pb2.CreateCube3DRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=(transform or Transform3D()).to_proto(),
                params=(params or Cube3DParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def create_sphere(
        self,
        *,
        name: str = "",
        transform: Transform3D | None = None,
        params: Sphere3DParams | None = None,
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_sphere_3d=shapes3d_pb2.CreateSphere3DRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=(transform or Transform3D()).to_proto(),
                params=(params or Sphere3DParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def create_plane(
        self,
        *,
        name: str = "",
        transform: Transform3D | None = None,
        params: Plane3DParams | None = None,
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_plane_3d=shapes3d_pb2.CreatePlane3DRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=(transform or Transform3D()).to_proto(),
                params=(params or Plane3DParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def create_corridor(
        self,
        *,
        name: str = "",
        transform: Transform3D | None = None,
        params: Corridor3DParams | None = None,
    ) -> StimulusHandle:
        """Create an endless corridor. See :class:`Corridor3DParams`."""
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_corridor_3d=shapes3d_pb2.CreateCorridor3DRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=(transform or Transform3D()).to_proto(),
                params=(params or Corridor3DParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    # ── Mutations ─────────────────────────────────────────────────────────────

    def set_transform(self, handle: StimulusHandle, transform: Transform3D) -> ServerResponse:
        """Replace the placement of any 3-D stimulus."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_transform_3d=shapes3d_pb2.SetTransform3DRequest(transform=transform.to_proto()),
        )))

    def set_material(self, handle: StimulusHandle, material: Material3D) -> ServerResponse:
        """Replace the material of any 3-D stimulus."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_material_3d=shapes3d_pb2.SetMaterial3DRequest(material=material.to_proto()),
        )))

    def set_cube_size(self, handle: StimulusHandle, size_cm: Vec3) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_cube_3d_size=shapes3d_pb2.SetCube3DSizeRequest(size_cm=size_cm.to_proto()),
        )))

    def set_sphere_diameter(self, handle: StimulusHandle, diameter_cm: float) -> ServerResponse:
        """Resize a sphere. Its tessellation is fixed at creation."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_sphere_3d_diameter=shapes3d_pb2.SetSphere3DDiameterRequest(
                diameter_cm=diameter_cm
            ),
        )))

    def set_plane_size(self, handle: StimulusHandle, size_cm: Vec2) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_plane_3d_size=shapes3d_pb2.SetPlane3DSizeRequest(size_cm=size_cm.to_proto()),
        )))
