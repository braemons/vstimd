from __future__ import annotations

from dataclasses import dataclass, field
from enum import Enum
from typing import Union

from vstimd._proto.vstimd.v1.stimuli import query_pb2, stimulus_type_pb2

from .color import Color
from .dots_models import DotsParams
from .grating_models import GratingParams
from .shapes3d_models import (
    Corridor3DParams,
    Cube3DParams,
    GaussianSplat3DParams,
    Plane3DParams,
    Sphere3DParams,
    Transform3D,
)
from .shapes_models import (
    CircleParams,
    EllipseParams,
    PolygonParams,
    RectParams,
    ShapeAppearance,
    ShapeDrawMode,
    _appearance_or_default,
)
from .text_models import TextParams
from .vec import Vec2


class StimulusType(Enum):
    """What the server calls a stimulus, as reported by ``query`` and ``list``.

    The wire enum also reserves BITMAP, SHADER and PARTICLE, which no vstimd
    server can construct or report. They are absent here for the same reason
    the server's own ``StimulusType`` omits them: a member you can never
    receive is a member you cannot write code against. Any type this client
    does not know — those three, or one added by a newer server — reads as
    ``UNKNOWN`` rather than raising, so an old client survives a new rig.
    """

    UNKNOWN = "unknown"
    RECT = "rect"
    CIRCLE = "circle"
    ELLIPSE = "ellipse"
    GRATING = "grating"
    TEXT = "text"
    POLYGON = "polygon"
    DOTS = "dots"
    CUBE_3D = "cube3d"
    SPHERE_3D = "sphere3d"
    PLANE_3D = "plane3d"
    CORRIDOR_3D = "corridor3d"
    GAUSSIAN_SPLAT_3D = "gaussiansplat3d"


StimulusParams = Union[
    RectParams,
    CircleParams,
    EllipseParams,
    GratingParams,
    TextParams,
    PolygonParams,
    DotsParams,
    Cube3DParams,
    Sphere3DParams,
    Plane3DParams,
    Corridor3DParams,
    GaussianSplat3DParams,
]

_STIMULUS_TYPE_MAP: dict[int, StimulusType] = {
    stimulus_type_pb2.STIMULUS_TYPE_RECT: StimulusType.RECT,
    stimulus_type_pb2.STIMULUS_TYPE_CIRCLE: StimulusType.CIRCLE,
    stimulus_type_pb2.STIMULUS_TYPE_ELLIPSE: StimulusType.ELLIPSE,
    stimulus_type_pb2.STIMULUS_TYPE_GRATING: StimulusType.GRATING,
    stimulus_type_pb2.STIMULUS_TYPE_TEXT: StimulusType.TEXT,
    stimulus_type_pb2.STIMULUS_TYPE_POLYGON: StimulusType.POLYGON,
    stimulus_type_pb2.STIMULUS_TYPE_DOTS: StimulusType.DOTS,
    stimulus_type_pb2.STIMULUS_TYPE_CUBE_3D: StimulusType.CUBE_3D,
    stimulus_type_pb2.STIMULUS_TYPE_SPHERE_3D: StimulusType.SPHERE_3D,
    stimulus_type_pb2.STIMULUS_TYPE_PLANE_3D: StimulusType.PLANE_3D,
    stimulus_type_pb2.STIMULUS_TYPE_CORRIDOR_3D: StimulusType.CORRIDOR_3D,
    stimulus_type_pb2.STIMULUS_TYPE_GAUSSIAN_SPLAT_3D: StimulusType.GAUSSIAN_SPLAT_3D,
}


@dataclass
class StimulusInfo:
    """Current state of one stimulus, as reported by ``query``.

    The fields here are the ones every stimulus has. Type-specific state lives in
    ``params`` — including the fill/outline appearance of a shape, which is
    reached through the ``fill_color`` / ``outline_color`` / ``outline_width_px`` /
    ``draw_mode`` convenience properties below and is ``None`` for a grating or a
    text stimulus, which have no such thing.

    ``pos_px`` and ``rotation_deg`` come from the 2-D placement. They are ``None``
    for a stimulus placed in 3-D space, which reports ``transform_3d`` instead —
    and that is ``None`` for a 2-D stimulus.
    """

    stimulus_type: StimulusType
    enabled: bool
    pos_px: Vec2 | None
    rotation_deg: float | None
    # Shared per-stimulus opacity in [0, 1]; multiplies the alpha of every colour
    # the stimulus carries.
    opacity: float
    params: StimulusParams | None
    id: str = ""
    name: str = ""
    anim_enabled: bool = True  # animation-level enable (False when animation holds it off)
    draw_order: int = 0  # 0-based position in scene draw order (0 = drawn first / behind)
    #: Conditions this stimulus is active in; empty means every condition.
    condition_indices: list[int] = field(default_factory=list)
    #: Condition-level enable: False while the active condition excludes it.
    #: Independent of ``enabled``, which a condition switch never touches.
    condition_enabled: bool = True
    #: World-space placement of a 3-D stimulus; ``None`` for a 2-D one.
    transform_3d: Transform3D | None = None

    @classmethod
    def from_proto(cls, proto: query_pb2.QueryStimulusResponse) -> StimulusInfo:
        shape_which = (
            proto.params.WhichOneof("shape") if proto.HasField("params") else None
        )
        if shape_which == "rect":
            params: StimulusParams | None = RectParams(
                width_px=proto.params.rect.width_px,
                height_px=proto.params.rect.height_px,
                appearance=_appearance_or_default(proto.params.rect),
            )
        elif shape_which == "circle":
            params = CircleParams(
                diameter_px=proto.params.circle.diameter_px,
                appearance=_appearance_or_default(proto.params.circle),
            )
        elif shape_which == "ellipse":
            params = EllipseParams(
                width_px=proto.params.ellipse.width_px,
                height_px=proto.params.ellipse.height_px,
                appearance=_appearance_or_default(proto.params.ellipse),
            )
        elif shape_which == "grating":
            params = GratingParams.from_proto(proto.params.grating)
        elif shape_which == "dots":
            params = DotsParams.from_proto(proto.params.dots)
        elif shape_which == "text":
            params = TextParams.from_proto(proto.params.text)
        elif shape_which == "polygon":
            params = PolygonParams(
                vertices_px=[Vec2(v.x, v.y) for v in proto.params.polygon.vertices_px],
                close_shape=proto.params.polygon.close_shape,
                appearance=_appearance_or_default(proto.params.polygon),
            )
        elif shape_which == "cube_3d":
            params = Cube3DParams.from_proto(proto.params.cube_3d)
        elif shape_which == "sphere_3d":
            params = Sphere3DParams.from_proto(proto.params.sphere_3d)
        elif shape_which == "plane_3d":
            params = Plane3DParams.from_proto(proto.params.plane_3d)
        elif shape_which == "corridor_3d":
            params = Corridor3DParams.from_proto(proto.params.corridor_3d)
        elif shape_which == "gaussian_splat_3d":
            params = GaussianSplat3DParams.from_proto(proto.params.gaussian_splat_3d)
        else:
            params = None

        is_2d = proto.WhichOneof("placement") == "transform_2d"
        is_3d = proto.WhichOneof("placement") == "transform_3d"
        return cls(
            stimulus_type=_STIMULUS_TYPE_MAP.get(
                proto.stimulus_type, StimulusType.UNKNOWN
            ),
            enabled=proto.enabled,
            pos_px=Vec2.from_proto(proto.transform_2d.pos_px) if is_2d else None,
            rotation_deg=proto.transform_2d.rotation_deg if is_2d else None,
            opacity=proto.opacity,
            params=params,
            id=proto.id,
            name=proto.name,
            anim_enabled=proto.anim_enabled,
            draw_order=proto.draw_order,
            condition_indices=list(proto.condition_indices),
            condition_enabled=proto.condition_enabled,
            transform_3d=Transform3D.from_proto(proto.transform_3d) if is_3d else None,
        )

    # ── Shape appearance, reached through the params ──────────────────────────
    #
    # These read the appearance a shape carries in its own params. They are None
    # for stimulus types that have no fill/outline model at all — a grating
    # reports its colours as fore_color/back_color, text as text_color, and
    # neither has an outline.

    @property
    def appearance(self) -> ShapeAppearance | None:
        return getattr(self.params, "appearance", None)

    @property
    def fill_color(self) -> Color | None:
        a = self.appearance
        return a.fill_color if a else None

    @property
    def outline_color(self) -> Color | None:
        a = self.appearance
        return a.outline_color if a else None

    @property
    def outline_width_px(self) -> float | None:
        a = self.appearance
        return a.outline_width_px if a else None

    @property
    def draw_mode(self) -> ShapeDrawMode | None:
        a = self.appearance
        return a.draw_mode if a else None
