from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1 import vec2_pb2, color_pb2
from vstimd._proto.vstimd.v1.stimuli import (
    rect_pb2, circle_pb2, ellipse_pb2, text_pb2,
    shared_set_requests_pb2, shapes_pb2,
)
from .stimuli_models import Color, DrawMode, LanguageStyle, Vec2

_SendFn = Callable[[service_pb2.Request], service_pb2.Response]

_DRAW_MODE_TO_PROTO: dict[DrawMode, int] = {
    DrawMode.FILLED:              shapes_pb2.SHAPE_DRAW_MODE_FILLED,
    DrawMode.OUTLINED:            shapes_pb2.SHAPE_DRAW_MODE_OUTLINED,
    DrawMode.FILLED_AND_OUTLINED: shapes_pb2.SHAPE_DRAW_MODE_FILLED_AND_OUTLINED,
}

_LANGUAGE_STYLE_TO_PROTO: dict[LanguageStyle, text_pb2.LanguageStyle] = {
    LanguageStyle.LTR:    text_pb2.LANGUAGE_STYLE_LTR,
    LanguageStyle.RTL:    text_pb2.LANGUAGE_STYLE_RTL,
    LanguageStyle.ARABIC: text_pb2.LANGUAGE_STYLE_ARABIC,
}


class ShapesClient:
    """Create and mutate rect, circle, ellipse, and text stimuli."""

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

    def create_text(
        self,
        *,
        text: str = "",
        pos: Vec2 = Vec2(0.0, 0.0),
        box_width: float = 400.0,
        box_height: float = 100.0,
        letter_height: float = 32.0,
        font: str = "",
        anchor: str = "center",
        color: Color = Color(1.0, 1.0, 1.0),
        fill_color: Color = Color(0.0, 0.0, 0.0, 0.0),
        language_style: LanguageStyle = LanguageStyle.LTR,
        name: str = "",
        id: str = "",
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_text=text_pb2.CreateTextRequest(
                text=text,
                font=font,
                letter_height=letter_height,
                size=vec2_pb2.Vec2(x=box_width, y=box_height),
                pos=vec2_pb2.Vec2(x=pos.x, y=pos.y),
                anchor=anchor,
                color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
                fill_color=color_pb2.Color(
                    r=fill_color.r, g=fill_color.g, b=fill_color.b, a=fill_color.a
                ),
                language_style=_LANGUAGE_STYLE_TO_PROTO[language_style],
                name=name,
                id=id,
            ),
        )
        return StimulusHandle(self._send(req).handle)

    # ── Generic mutations ──────────────────────────────────────────────────────

    def set_name(self, handle: StimulusHandle, name: str) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_name=shared_set_requests_pb2.SetNameRequest(name=name),
        ))

    def set_enabled(self, handle: StimulusHandle, enabled: bool) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_enabled=shared_set_requests_pb2.SetEnabledRequest(enabled=enabled),
        ))

    def delete(self, handle: StimulusHandle) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            delete=shared_set_requests_pb2.DeleteRequest(),
        ))

    def set_position(self, handle: StimulusHandle, pos: Vec2) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_position=shared_set_requests_pb2.SetPositionRequest(x=pos.x, y=pos.y),
        ))

    def set_orientation(self, handle: StimulusHandle, angle_deg: float) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_orientation=shared_set_requests_pb2.SetOrientationRequest(angle_deg=angle_deg),
        ))

    def set_fill_color(self, handle: StimulusHandle, color: Color) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_fill_color=shared_set_requests_pb2.SetFillColorRequest(
                color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
            ),
        ))

    def set_alpha(self, handle: StimulusHandle, opacity: float) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_alpha=shared_set_requests_pb2.SetAlphaRequest(opacity=opacity),
        ))

    def set_draw_mode(self, handle: StimulusHandle, mode: DrawMode) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_draw_mode=shared_set_requests_pb2.SetDrawModeRequest(
                mode=_DRAW_MODE_TO_PROTO[mode],
            ),
        ))

    def set_outline_color(self, handle: StimulusHandle, color: Color) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_outline_color=shared_set_requests_pb2.SetOutlineColorRequest(
                color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
            ),
        ))

    def set_outline_width(self, handle: StimulusHandle, line_width: float) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_outline_width=shared_set_requests_pb2.SetOutlineWidthRequest(line_width=line_width),
        ))

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

    # ── Text mutations ─────────────────────────────────────────────────────────

    def set_text(self, handle: StimulusHandle, text: str) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_text=text_pb2.SetTextRequest(text=text),
        ))

    def set_text_color(self, handle: StimulusHandle, color: Color) -> None:
        self._send(service_pb2.Request(
            stimulus=handle,
            set_text_color=text_pb2.SetTextColorRequest(
                color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
            ),
        ))
