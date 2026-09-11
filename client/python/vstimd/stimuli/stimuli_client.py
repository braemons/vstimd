from __future__ import annotations

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1.stimuli import (
    query_pb2,
    shared_set_requests_pb2,
)
from vstimd.response import ServerResponse

from .dots_client import DotsClient
from .grating_client import GratingClient
from .shapes_client import ShapesClient, _SendFn
from .text_client import TextClient
from .color import Color
from .stimuli_models import StimulusInfo
from .vec import Vec2


class StimuliClient:
    """Stimulus creation and mutation commands.

    Accessed as ``conn.stimuli`` on a :class:`~vstimd.Connection` instance.
    Groups sub-clients by stimulus family:

    * ``shapes`` — :class:`~vstimd.stimuli.ShapesClient`: rect, circle, ellipse, polygon
    * ``grating`` — :class:`~vstimd.stimuli.GratingClient`: grating stimuli
    * ``text`` — :class:`~vstimd.stimuli.TextClient`: text stimuli
    * ``dots`` — :class:`~vstimd.stimuli.DotsClient`: random dot kinematograms

    Example::

        with Connection() as conn:
            h = conn.stimuli.shapes.create_rect(
                position_px=Vec2(0, 0),
                params=RectParams(
                    width_px=200, height_px=100,
                    appearance=ShapeAppearance(fill_color=Color(1, 0, 0)),
                ),
            )
            conn.stimuli.set_enabled(h, False)
            conn.stimuli.delete(h)
    """

    def __init__(self, send: _SendFn) -> None:
        self.shapes = ShapesClient(send)
        self.grating = GratingClient(send)
        self.text = TextClient(send)
        self.dots = DotsClient(send)
        self._send = send

    # ── Generic mutations ──────────────────────────────────────────────────────

    def set_name(self, handle: StimulusHandle, name: str) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_name=shared_set_requests_pb2.SetNameRequest(name=name),
            )
        ))

    def set_enabled(self, handle: StimulusHandle, enabled: bool) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_enabled=shared_set_requests_pb2.SetEnabledRequest(enabled=enabled),
            )
        ))

    def delete(self, handle: StimulusHandle) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                delete=shared_set_requests_pb2.DeleteRequest(),
            )
        ))

    def set_position(self, handle: StimulusHandle, pos_px: Vec2) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_position=shared_set_requests_pb2.SetPositionRequest(
                    x_px=pos_px.x, y_px=pos_px.y
                ),
            )
        ))

    def set_rotation(self, handle: StimulusHandle, rotation_deg: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_rotation=shared_set_requests_pb2.SetRotationRequest(
                    rotation_deg=rotation_deg
                ),
            )
        ))

    def set_fill_color(self, handle: StimulusHandle, color: Color) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_fill_color=shared_set_requests_pb2.SetFillColorRequest(
                    color=color.to_proto(),
                ),
            )
        ))

    def set_alpha(self, handle: StimulusHandle, opacity: float) -> ServerResponse:
        """Set whole-stimulus opacity in [0, 1]. Valid for every stimulus type.

        The value multiplies whatever alpha the stimulus' own colours carry, so a
        shape with a half-transparent fill and an opaque outline keeps that
        relationship at every opacity. Out-of-range values are clamped.
        """
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                set_alpha=shared_set_requests_pb2.SetAlphaRequest(opacity=opacity),
            )
        ))

    def bring_to_front(self, handle: StimulusHandle) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                bring_to_front=shared_set_requests_pb2.BringToFrontRequest(),
            )
        ))

    def send_to_back(self, handle: StimulusHandle) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                stimulus=handle,
                send_to_back=shared_set_requests_pb2.SendToBackRequest(),
            )
        ))

    def swap_draw_order(self, handle_a: StimulusHandle, handle_b: StimulusHandle) -> ServerResponse:
        return ServerResponse._from_proto(self._send(
            service_pb2.Request(
                system=service_pb2.SystemTarget(),
                swap_draw_order=shared_set_requests_pb2.SwapDrawOrderRequest(
                    handle_a=handle_a, handle_b=handle_b,
                ),
            )
        ))

    # ── Query ──────────────────────────────────────────────────────────────────

    def query(self, handle: StimulusHandle) -> StimulusInfo:
        """Return current server-side properties for the given stimulus handle."""
        req = service_pb2.Request(
            stimulus=handle,
            query_stimulus=query_pb2.QueryStimulusRequest(),
        )
        return StimulusInfo.from_proto(self._send(req).stimulus_info)
