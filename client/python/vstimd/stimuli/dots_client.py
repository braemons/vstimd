from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1.stimuli import dots_pb2
from vstimd._proto.vstimd.v1.transform_pb2 import Transform2D
from vstimd.response import ServerResponse

from .color import Color
from .dots_models import Aperture, DotsParams
from .stimulus_identity import StimulusIdentity
from .vec import Vec2

_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class DotsClient:
    """Create and mutate random dot kinematograms.

    ``create_dots`` takes the same ``DotsParams`` a ``query`` reports back — see
    ShapesClient for the identity/placement/params shape every create shares.

    Two families of RDK are built from this one stimulus:

    * a **classic RDK** — one field in a circular aperture the same size as the
      field, ``coherence`` swept across trials;
    * a **figure-ground RDK** — two fields over the same area, one masked to a
      circle and the other to its complement (``Aperture(invert=True)``),
      differing only in ``direction_deg``, so nothing but motion separates figure
      from ground in any single frame.

    Example — the figure-ground case::

        circle = Aperture(shape=ApertureShape.CIRCLE,
                          width_px=diameter_from_radius(radius_px),
                          offset_px=rf_center)
        common = DotsParams(field_width_px=1920, field_height_px=1080,
                            dot_count=166, dot_size_px=60, speed_px_per_s=1000)
        ground = conn.stimuli.dots.create_dots(params=replace(
            common, aperture=replace(circle, invert=True), direction_deg=0, seed=1))
        figure = conn.stimuli.dots.create_dots(params=replace(
            common, aperture=circle, direction_deg=90, seed=2))
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    # ── Creation ──────────────────────────────────────────────────────────────

    def create_dots(
        self,
        *,
        name: str = "",
        position_px: Vec2 = Vec2(0.0, 0.0),
        params: DotsParams | None = None,
    ) -> StimulusHandle:
        """Create a dot field and return its handle.

        ``position_px`` is the centre of the *field*; the aperture is placed
        relative to it by ``params.aperture.offset_px``. There is no
        ``rotation_deg``: a dot field has no orientation of its own, and the
        direction of motion is ``params.direction_deg``.

        For transparency use the shared ``conn.stimuli.set_alpha(handle, opacity)``.
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_dots=dots_pb2.CreateDotsRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=Transform2D(pos_px=position_px.to_proto(), rotation_deg=0.0),
                params=(params or DotsParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    # ── Dot-field mutations ───────────────────────────────────────────────────

    def set_direction(self, handle: StimulusHandle, direction_deg: float) -> ServerResponse:
        """Set the direction of coherent motion, CCW with 0° = right.

        Applied as a change of *velocity*, from wherever the dots currently are —
        they do not jump back onto a line through their birth positions. That is
        what makes a mid-trial direction switch (Psychtoolbox's ``noFigureFrames``)
        expressible.
        """
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_direction=dots_pb2.SetDotsDirectionRequest(direction_deg=direction_deg),
        )))

    def set_speed(self, handle: StimulusHandle, speed_px_per_s: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_speed=dots_pb2.SetDotsSpeedRequest(speed_px_per_s=speed_px_per_s),
        )))

    def set_coherence(self, handle: StimulusHandle, coherence: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_coherence=dots_pb2.SetDotsCoherenceRequest(coherence=coherence),
        )))

    def set_dot_count(self, handle: StimulusHandle, dot_count: int) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_count=dots_pb2.SetDotsCountRequest(dot_count=dot_count),
        )))

    def set_dot_size(self, handle: StimulusHandle, dot_size_px: float) -> ServerResponse:
        """Set the dot **diameter**, not its radius."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_size=dots_pb2.SetDotsSizeRequest(dot_size_px=dot_size_px),
        )))

    def set_dot_color(
        self,
        handle: StimulusHandle,
        color: Color,
        color_alt: Color | None = None,
    ) -> ServerResponse:
        """Set the dot colour, and optionally a second one.

        Both are set together: passing no ``color_alt`` clears it, giving a
        single-colour field.
        """
        req = dots_pb2.SetDotsColorRequest(dot_color=color.to_proto())
        if color_alt is not None:
            req.dot_color_alt.CopyFrom(color_alt.to_proto())
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle, set_dots_color=req,
        )))

    def set_aperture(self, handle: StimulusHandle, aperture: Aperture) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_aperture=dots_pb2.SetDotsApertureRequest(aperture=aperture.to_proto()),
        )))

    def set_field_size(
        self, handle: StimulusHandle, width_px: float, height_px: float
    ) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_field_size=dots_pb2.SetDotsFieldSizeRequest(
                width_px=width_px, height_px=height_px,
            ),
        )))

    def set_dot_lifetime(self, handle: StimulusHandle, dot_lifetime_frames: int) -> ServerResponse:
        """Set the dot lifetime in frames; 0 is infinite."""
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_lifetime=dots_pb2.SetDotsLifetimeRequest(
                dot_lifetime_frames=dot_lifetime_frames,
            ),
        )))

    def set_seed(self, handle: StimulusHandle, seed: int) -> ServerResponse:
        """Reseed the field, redrawing the sample and restarting it at frame 0.

        Never deferred: a seed is not a value that can be half-applied — the sample
        it describes either exists or does not — so this takes effect immediately
        even in deferred mode.
        """
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_dots_seed=dots_pb2.SetDotsSeedRequest(seed=seed),
        )))
