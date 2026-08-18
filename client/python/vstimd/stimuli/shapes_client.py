from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1.stimuli import (
    circle_pb2,
    ellipse_pb2,
    polygon_pb2,
    rect_pb2,
    shapes_pb2,
)
from vstimd._proto.vstimd.v1.transform_pb2 import Transform2D
from vstimd.response import ServerResponse

from .color import Color
from .shapes_models import (
    CircleParams,
    EllipseParams,
    PolygonParams,
    RectParams,
    ShapeDrawMode,
    _SHAPE_DRAW_MODE_TO_PROTO,
)
from .stimulus_identity import StimulusIdentity
from .vec import Vec2

_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


def _placement(position_px: Vec2, rotation_deg: float) -> Transform2D:
    return Transform2D(pos_px=position_px.to_proto(), rotation_deg=rotation_deg)


class ShapesClient:
    """Create and mutate rect, circle, ellipse and polygon stimuli.

    Every ``create_*`` takes the same three things the wire does — who the
    stimulus is, where it sits, and what it looks like — so the params object a
    ``query`` hands back is the very object a create accepts.
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    # ── Creation ──────────────────────────────────────────────────────────────

    def create_rect(
        self,
        *,
        name: str = "",
        position_px: Vec2 = Vec2(0.0, 0.0),
        rotation_deg: float = 0.0,
        params: RectParams | None = None,
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_rect=rect_pb2.CreateRectRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=_placement(position_px, rotation_deg),
                params=(params or RectParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def create_circle(
        self,
        *,
        name: str = "",
        position_px: Vec2 = Vec2(0.0, 0.0),
        rotation_deg: float = 0.0,
        params: CircleParams | None = None,
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_circle=circle_pb2.CreateCircleRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=_placement(position_px, rotation_deg),
                params=(params or CircleParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def create_ellipse(
        self,
        *,
        name: str = "",
        position_px: Vec2 = Vec2(0.0, 0.0),
        rotation_deg: float = 0.0,
        params: EllipseParams | None = None,
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_ellipse=ellipse_pb2.CreateEllipseRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=_placement(position_px, rotation_deg),
                params=(params or EllipseParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def create_polygon(
        self,
        *,
        name: str = "",
        position_px: Vec2 = Vec2(0.0, 0.0),
        rotation_deg: float = 0.0,
        params: PolygonParams | None = None,
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_polygon=polygon_pb2.CreatePolygonRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=_placement(position_px, rotation_deg),
                params=(params or PolygonParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    # ── Geometry setters ──────────────────────────────────────────────────────

    def set_rect_size(
        self, handle: StimulusHandle, width_px: float, height_px: float
    ) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_rect_size=rect_pb2.SetRectSizeRequest(width_px=width_px, height_px=height_px),
            )
        ))

    def set_circle_diameter(self, handle: StimulusHandle, diameter_px: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_circle_diameter=circle_pb2.SetCircleDiameterRequest(diameter_px=diameter_px),
            )
        ))

    def set_ellipse_size(
        self, handle: StimulusHandle, width_px: float, height_px: float
    ) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_ellipse_size=ellipse_pb2.SetEllipseSizeRequest(
                    width_px=width_px, height_px=height_px
                ),
            )
        ))

    def set_polygon_vertices(
        self, handle: StimulusHandle, vertices_px: list[Vec2]
    ) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_polygon_vertices=polygon_pb2.SetPolygonVerticesRequest(
                    vertices_px=[v.to_proto() for v in vertices_px],
                ),
            )
        ))

    # ── Appearance setters ────────────────────────────────────────────────────

    def set_draw_mode(self, handle: StimulusHandle, mode: ShapeDrawMode) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_draw_mode=shapes_pb2.SetDrawModeRequest(
                    mode=_SHAPE_DRAW_MODE_TO_PROTO[mode],
                ),
            )
        ))

    def set_outline_color(self, handle: StimulusHandle, color: Color) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_outline_color=shapes_pb2.SetOutlineColorRequest(
                    color=color.to_proto(),
                ),
            )
        ))

    def set_outline_width(self, handle: StimulusHandle, line_width_px: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_outline_width=shapes_pb2.SetOutlineWidthRequest(
                    line_width_px=line_width_px
                ),
            )
        ))
