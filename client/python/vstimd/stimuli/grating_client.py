from __future__ import annotations

from typing import Callable

from vstimd._handles import StimulusHandle
from vstimd._proto import service_pb2
from vstimd._proto.vstimd.v1.stimuli import grating_pb2
from vstimd._proto.vstimd.v1.transform_pb2 import Transform2D
from vstimd.response import ServerResponse

from .color import Color
from .grating_models import (
    GratingMask,
    GratingParams,
    GratingTexture,
    _MASK_TO_PROTO,
    _WAVEFORM_TO_PROTO,
)
from .stimulus_identity import StimulusIdentity
from .vec import Vec2

_SendFn = Callable[[service_pb2.Request], service_pb2.Response]


class GratingClient:
    """Create and mutate grating stimuli.

    ``create_grating`` takes the same ``GratingParams`` a ``query`` reports back —
    see ShapesClient for the identity/placement/params shape every create shares.
    """

    def __init__(self, send: _SendFn) -> None:
        self._send = send

    # ── Creation ──────────────────────────────────────────────────────────────

    def create_grating(
        self,
        *,
        name: str = "",
        position_px: Vec2 = Vec2(0.0, 0.0),
        rotation_deg: float = 0.0,
        params: GratingParams | None = None,
    ) -> StimulusHandle:
        """Create a grating stimulus and return its handle.

        ``rotation_deg`` is the stripe rotation_deg, not the patch's: 0° gives vertical
        stripes varying along X. It is the placement's rotation_deg because it is the
        same property ``set_rotation`` sets.

        The grating interpolates between ``params.back_color`` (carrier = -1) and
        ``params.fore_color`` (carrier = +1), modulated by contrast. For
        transparency use the shared ``conn.stimuli.set_alpha(handle, opacity)``.

        ``params.mask_param`` interpretation (0 = use default):
          - GratingMask.GAUSS:      SD in normalized units where patch radius = 1 (default 1/3)
          - GratingMask.RAISED_COS: fringe proportion [0, 1] (default 0.2)
        """
        req = service_pb2.Request(
            system=service_pb2.SystemTarget(),
            create_grating=grating_pb2.CreateGratingRequest(
                identity=StimulusIdentity(name=name).to_proto(),
                placement=Transform2D(pos_px=position_px.to_proto(), rotation_deg=rotation_deg),
                params=(params or GratingParams()).to_proto(),
            ),
        )
        return StimulusHandle(self._send(req).handle)

    # ── Grating-specific mutations ─────────────────────────────────────────────

    def set_phase(self, handle: StimulusHandle, phase_cycles: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_phase=grating_pb2.SetGratingPhaseRequest(phase_cycles=phase_cycles),
        )))

    def set_sf(self, handle: StimulusHandle, sf_cycles_per_px: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_sf=grating_pb2.SetGratingSfRequest(sf_cycles_per_px=sf_cycles_per_px),
        )))

    def set_contrast(self, handle: StimulusHandle, contrast: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_contrast=grating_pb2.SetGratingContrastRequest(contrast=contrast),
        )))

    def set_waveform(self, handle: StimulusHandle, waveform: GratingTexture) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_waveform=grating_pb2.SetGratingWaveformRequest(
                waveform=_WAVEFORM_TO_PROTO[waveform],
            ),
        )))

    def set_mask(self, handle: StimulusHandle, mask: GratingMask) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_mask=grating_pb2.SetGratingMaskRequest(mask=_MASK_TO_PROTO[mask]),
        )))

    def set_drift_speed(self, handle: StimulusHandle, drift_speed_hz: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_drift_speed=grating_pb2.SetGratingDriftSpeedRequest(speed_hz=drift_speed_hz),
        )))

    def set_drift_decoupled(self, handle: StimulusHandle, drift_decoupled: bool) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_drift_decoupled=grating_pb2.SetGratingDriftDecoupledRequest(
                decoupled=drift_decoupled,
            ),
        )))

    def set_drift_angle(self, handle: StimulusHandle, drift_angle_deg: float) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_drift_angle=grating_pb2.SetGratingDriftAngleRequest(drift_angle_deg=drift_angle_deg),
        )))

    def set_fore_color(self, handle: StimulusHandle, color: Color) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_fore_color=grating_pb2.SetGratingForeColorRequest(
                fore_color=color.to_proto(),
            ),
        )))

    def set_back_color(self, handle: StimulusHandle, color: Color) -> ServerResponse:
        return ServerResponse._from_proto(self._send(service_pb2.Request(
            stimulus=handle,
            set_grating_back_color=grating_pb2.SetGratingBackColorRequest(
                back_color=color.to_proto(),
            ),
        )))

    # Opacity is a shared property: use ``conn.stimuli.set_alpha`` — it works on
    # gratings, shapes and text alike.
