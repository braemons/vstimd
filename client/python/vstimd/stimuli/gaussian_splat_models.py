"""Gaussian splat scenes: the parameters of a 3DGS stimulus.

A prototype, kept in a module of its own — with
:mod:`vstimd.stimuli.gaussian_splat_client` — so splatting can be added,
disabled or removed without touching the rest of the 3-D surface. Placement is
the shared :class:`~vstimd.stimuli.Transform3D`, in centimetres, like any other
3-D stimulus.
"""

from __future__ import annotations

from dataclasses import dataclass

from vstimd._proto.vstimd.v1.stimuli import gaussian_splat_pb2


@dataclass
class GaussianSplat3DParams:
    """A trained 3-D Gaussian splat scene, loaded from a file on the server.

    ``path`` names a ``.ply`` (the 3DGS reference layout; only the
    view-independent colour is used) or an antimatter15 ``.splat`` file on the
    *server's* filesystem. It is checked when the stimulus is created and loaded
    in the background: the stimulus draws nothing until the load finishes, which
    takes about a second for a million splats.

    A trained scene has no units and, straight out of COLMAP, is upside down in
    vstimd's Y-up world. Place it with the stimulus' :class:`Transform3D`:
    ``scale`` is centimetres per scene unit, and ``rotation_deg=Vec3(0, 180, 0)``
    (a 180° pitch) usually turns a COLMAP scene upright.

    Splats draw after every other 3-D stimulus, sorted back to front. They are
    hidden behind opaque 3-D stimuli but never hide them. Their colours are
    baked in by training — no material, no lighting, not a calibrated
    luminance. ``set_alpha`` fades the whole scene.
    """

    path: str = ""

    def to_proto(self) -> gaussian_splat_pb2.GaussianSplat3DParams:
        return gaussian_splat_pb2.GaussianSplat3DParams(path=self.path)

    @classmethod
    def from_proto(
        cls, proto: gaussian_splat_pb2.GaussianSplat3DParams
    ) -> GaussianSplat3DParams:
        return cls(path=proto.path)
