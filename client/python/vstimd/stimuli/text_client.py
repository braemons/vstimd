from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1.stimuli import text_pb2
from vstimd._proto.vstimd.v1.transform_pb2 import Transform2D
from vstimd.response import ServerResponse
from vstimd.stimuli.stimulus_identity import StimulusIdentity

from .color import Color
from .text_models import TextParams
from .vec import Vec2

_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class TextClient:
    """Create and mutate text stimuli.

    ``create_text`` takes the same ``TextParams`` a ``query`` reports back — see
    ShapesClient for the identity/placement/params shape every create shares.
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    def create_text(
        self,
        *,
        name: str = "",
        position_px: Vec2 = Vec2(0.0, 0.0),
        rotation_deg: float = 0.0,
        params: TextParams | None = None,
    ) -> StimulusHandle:
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_text=text_pb2.CreateTextRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=Transform2D(pos_px=position_px.to_proto(), rotation_deg=rotation_deg),
                params=(params or TextParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    def set_text(self, handle: StimulusHandle, text: str) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_text=text_pb2.SetTextRequest(text=text),
        )))

    def set_text_color(self, handle: StimulusHandle, color: Color) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_text_color=text_pb2.SetTextColorRequest(
                color=color.to_proto(),
            ),
        )))
