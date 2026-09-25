"""Gaussian splat scenes: the ``conn.stimuli.gaussian_splat`` sub-client.

A prototype, kept in a module of its own so splatting can be added, disabled or
removed without touching the rest of the 3-D surface. Everything a splat scene
needs beyond creation is generic: ``conn.stimuli.shapes3d.set_transform`` moves
it, ``conn.stimuli.set_alpha`` / ``set_enabled`` / ``delete`` work as for any
stimulus. It has no material, so ``set_material`` refuses it.
"""

from __future__ import annotations

from typing import Callable

from vstimd_client._handles import StimulusHandle
from vstimd_client._proto import service_pb2
from vstimd_client._proto.vstimd.v1.stimuli import gaussian_splat_pb2

from .gaussian_splat_models import GaussianSplat3DParams
from .shapes3d_models import Transform3D
from .stimulus_identity import StimulusIdentity

_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class GaussianSplatClient:
    """Create Gaussian splat scenes (3DGS).

    Accessed as ``conn.stimuli.gaussian_splat``. A splat scene is a 3-D stimulus
    like any other: seen through the scene camera, placed by a
    :class:`Transform3D` in centimetres, and mutated through the generic
    commands. Only its creation is splat-specific, which is why this is the
    sub-client's only method.

    Example::

        corridor = conn.stimuli.gaussian_splat.create(
            "/srv/scenes/corridor.ply",
            transform=Transform3D(
                scale=Vec3(100, 100, 100),      # centimetres per scene unit
                rotation_deg=Vec3(0, 180, 0),   # COLMAP scenes arrive upside down
            ),
        )
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    def create(
        self,
        path: str,
        *,
        name: str = "",
        transform: Transform3D | None = None,
    ) -> StimulusHandle:
        """Create a Gaussian splat scene from a file on the server.

        See :class:`GaussianSplat3DParams` for formats and placement.

        Raises:
            InvalidArgumentError: the file is missing, unreadable, or not a
                splat file the server reads; the message says which.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_gaussian_splat_3d=gaussian_splat_pb2.CreateGaussianSplat3DRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=(transform or Transform3D()).to_proto(),
                params=GaussianSplat3DParams(path=path).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)
