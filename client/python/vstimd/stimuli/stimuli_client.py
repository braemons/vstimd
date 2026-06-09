from __future__ import annotations

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1 import color_pb2
from vstimd._proto.vstimd.v1.stimuli import (
    query_pb2,
    shapes_pb2,
    shared_set_requests_pb2,
)

from ._grating import GratingClient
from ._shapes import ShapesClient, _SendFn
from ._text import TextClient
from .stimuli_models import Color, DrawMode, StimulusInfo, Vec2

_DRAW_MODE_TO_PROTO: dict[DrawMode, shapes_pb2.ShapeDrawMode] = {
    DrawMode.FILLED: shapes_pb2.SHAPE_DRAW_MODE_FILLED,
    DrawMode.OUTLINED: shapes_pb2.SHAPE_DRAW_MODE_OUTLINED,
    DrawMode.FILLED_AND_OUTLINED: shapes_pb2.SHAPE_DRAW_MODE_FILLED_AND_OUTLINED,
}


class StimuliClient:
    """Top-level stimuli client; groups subclients by stimulus family."""

    def __init__(self, send: _SendFn) -> None:
        self.shapes = ShapesClient(send)
        self.grating = GratingClient(send)
        self.text = TextClient(send)
        self._send = send

    # ── Generic mutations ──────────────────────────────────────────────────────

    def set_name(self, handle: StimulusHandle, name: str) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_name=shared_set_requests_pb2.SetNameRequest(name=name),
            )
        )

    def set_enabled(self, handle: StimulusHandle, enabled: bool) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_enabled=shared_set_requests_pb2.SetEnabledRequest(enabled=enabled),
            )
        )

    def delete(self, handle: StimulusHandle) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                delete=shared_set_requests_pb2.DeleteRequest(),
            )
        )

    def set_position(self, handle: StimulusHandle, pos: Vec2) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_position=shared_set_requests_pb2.SetPositionRequest(
                    x=pos.x, y=pos.y
                ),
            )
        )

    def set_orientation(self, handle: StimulusHandle, angle_deg: float) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_orientation=shared_set_requests_pb2.SetOrientationRequest(
                    angle_deg=angle_deg
                ),
            )
        )

    def set_fill_color(self, handle: StimulusHandle, color: Color) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_fill_color=shared_set_requests_pb2.SetFillColorRequest(
                    color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
                ),
            )
        )

    def set_alpha(self, handle: StimulusHandle, opacity: float) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_alpha=shared_set_requests_pb2.SetAlphaRequest(opacity=opacity),
            )
        )

    def set_draw_mode(self, handle: StimulusHandle, mode: DrawMode) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_draw_mode=shared_set_requests_pb2.SetDrawModeRequest(
                    mode=_DRAW_MODE_TO_PROTO[mode],
                ),
            )
        )

    def set_outline_color(self, handle: StimulusHandle, color: Color) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_outline_color=shared_set_requests_pb2.SetOutlineColorRequest(
                    color=color_pb2.Color(r=color.r, g=color.g, b=color.b, a=color.a),
                ),
            )
        )

    def set_outline_width(self, handle: StimulusHandle, line_width: float) -> None:
        self._send(
            service_pb2.Request(
                stimulus=handle,
                set_outline_width=shared_set_requests_pb2.SetOutlineWidthRequest(
                    line_width=line_width
                ),
            )
        )

    # ── Query ──────────────────────────────────────────────────────────────────

    def query(self, handle: StimulusHandle) -> StimulusInfo:
        """Return current server-side properties for the given stimulus handle."""
        req = service_pb2.Request(
            stimulus=handle,
            query_stimulus=query_pb2.QueryStimulusRequest(),
        )
        return StimulusInfo.from_proto(self._send(req).stimulus_info)
