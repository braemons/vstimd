from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1 import vec2_pb2, color_pb2
from vstimd._proto.vstimd.v1.stimuli import rect_pb2, circle_pb2, ellipse_pb2
from .stimuli_models import Color, Vec2

_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class ShapesClient:
    """Create and mutate rect, circle, and ellipse stimuli."""

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    # ── Creation ──────────────────────────────────────────────────────────────

    def create_rect(
        self,
        *,
        pos: Vec2 = Vec2(0.0, 0.0),
        width: float = 100.0,
        height: float = 100.0,
        color: Color = Color(1.0, 1.0, 1.0),
        name: str = "",
        id: str = "",
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_rect=rect_pb2.CreateRectRequest(
                center=vec2_pb2.Vec2(x=pos.x, y=pos.y),
                width=width,
                height=height,
                fill_color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
                name=name,
                id=id,
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def create_circle(
        self,
        *,
        pos: Vec2 = Vec2(0.0, 0.0),
        radius: float = 50.0,
        color: Color = Color(1.0, 1.0, 1.0),
        name: str = "",
        id: str = "",
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_circle=circle_pb2.CreateCircleRequest(
                center=vec2_pb2.Vec2(x=pos.x, y=pos.y),
                radius=radius,
                fill_color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
                name=name,
                id=id,
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def create_ellipse(
        self,
        *,
        pos: Vec2 = Vec2(0.0, 0.0),
        width: float = 100.0,
        height: float = 50.0,
        angle: float = 0.0,
        color: Color = Color(1.0, 1.0, 1.0),
        name: str = "",
        id: str = "",
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_ellipse=ellipse_pb2.CreateEllipseRequest(
                center=vec2_pb2.Vec2(x=pos.x, y=pos.y),
                width=width,
                height=height,
                angle=angle,
                fill_color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
                name=name,
                id=id,
            ),
        )
        return StimulusHandle(self._send(req).handle)

    # ── Shape-specific mutations ───────────────────────────────────────────────

    def set_rect_size(self, handle: StimulusHandle, width: float, height: float) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_rect_size=rect_pb2.SetRectSizeRequest(width=width, height=height),
        ))

    def set_circle_radius(self, handle: StimulusHandle, radius: float) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_circle_radius=circle_pb2.SetCircleRadiusRequest(radius=radius),
        ))

    def set_ellipse_size(self, handle: StimulusHandle, width: float, height: float) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_ellipse_size=ellipse_pb2.SetEllipseSizeRequest(width=width, height=height),
        ))
