from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum

from vstimd._proto.vstimd.v1.stimuli import grating_pb2

from .color import Color


class GratingTexture(StrEnum):
    SIN = "sin"
    SQR = "sqr"
    SAW = "saw"
    TRI = "tri"


class GratingMask(StrEnum):
    NONE       = "none"
    CIRCLE     = "circle"
    GAUSS      = "gauss"
    RAISED_COS = "raisedCos"
    HANN       = "hann"


_WAVEFORM_TO_PROTO: dict[GratingTexture, grating_pb2.WaveformType] = {
    GratingTexture.SIN: grating_pb2.WAVEFORM_TYPE_SIN,
    GratingTexture.SQR: grating_pb2.WAVEFORM_TYPE_SQR,
    GratingTexture.SAW: grating_pb2.WAVEFORM_TYPE_SAW,
    GratingTexture.TRI: grating_pb2.WAVEFORM_TYPE_TRI,
}

_PROTO_TO_WAVEFORM: dict[grating_pb2.WaveformType, GratingTexture] = {v: k for k, v in _WAVEFORM_TO_PROTO.items()}

_MASK_TO_PROTO: dict[GratingMask, grating_pb2.MaskType] = {
    GratingMask.NONE:       grating_pb2.MASK_TYPE_NONE,
    GratingMask.CIRCLE:     grating_pb2.MASK_TYPE_CIRCLE,
    GratingMask.GAUSS:      grating_pb2.MASK_TYPE_GAUSS,
    GratingMask.RAISED_COS: grating_pb2.MASK_TYPE_RAISED_COS,
    GratingMask.HANN:       grating_pb2.MASK_TYPE_HANN,
}

_PROTO_TO_MASK: dict[grating_pb2.MaskType, GratingMask] = {v: k for k, v in _MASK_TO_PROTO.items()}


@dataclass
class GratingParams:
    """Geometry and carrier of a grating stimulus.

    Sent by ``create_grating`` and reported back by ``query``, mirroring the one
    ``GratingParams`` message the wire uses in both directions. Every field
    defaults to the server's "unset" value, so a create names only what it
    cares about and inherits the rest — 0 means *default*, not literally zero,
    for ``width``/``height``/``sf``/``contrast``.
    """

    width: float = 0.0
    height: float = 0.0
    sf: float = 0.0
    phase: float = 0.0
    contrast: float = 0.0
    waveform: GratingTexture = GratingTexture.SIN
    mask: GratingMask = GratingMask.NONE
    mask_param: float = 0.0
    drift_speed: float = 0.0
    drift_coupled: bool = True
    drift_angle: float = 0.0
    fore_color: Color = field(default_factory=lambda: Color(1.0, 1.0, 1.0, 1.0))
    back_color: Color = field(default_factory=lambda: Color(0.0, 0.0, 0.0, 1.0))

    @classmethod
    def from_proto(cls, proto: grating_pb2.GratingParams) -> GratingParams:
        fore = (
            Color.from_proto(proto.fore_color)
            if proto.HasField("fore_color")
            else Color(1.0, 1.0, 1.0, 1.0)
        )
        back = (
            Color.from_proto(proto.back_color)
            if proto.HasField("back_color")
            else Color(0.0, 0.0, 0.0, 1.0)
        )
        return cls(
            width=proto.width,
            height=proto.height,
            sf=proto.sf,
            phase=proto.phase,
            contrast=proto.contrast,
            waveform=_PROTO_TO_WAVEFORM.get(proto.waveform, GratingTexture.SIN),
            mask=_PROTO_TO_MASK.get(proto.mask, GratingMask.NONE),
            mask_param=proto.mask_param,
            drift_speed=proto.drift_speed,
            drift_coupled=not proto.drift_decoupled,
            drift_angle=proto.drift_angle,
            fore_color=fore,
            back_color=back,
        )

    def to_proto(self) -> grating_pb2.GratingParams:
        return grating_pb2.GratingParams(
            width=self.width,
            height=self.height,
            sf=self.sf,
            phase=self.phase,
            contrast=self.contrast,
            waveform=_WAVEFORM_TO_PROTO.get(self.waveform, grating_pb2.WAVEFORM_TYPE_SIN),
            mask=_MASK_TO_PROTO.get(self.mask, grating_pb2.MASK_TYPE_NONE),
            mask_param=self.mask_param,
            drift_speed=self.drift_speed,
            drift_decoupled=not self.drift_coupled,
            drift_angle=self.drift_angle,
            fore_color=self.fore_color.to_proto(),
            back_color=self.back_color.to_proto(),
        )
