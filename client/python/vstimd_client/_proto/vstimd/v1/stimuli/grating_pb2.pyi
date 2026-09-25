from vstimd_client._proto.vstimd.v1 import color_pb2 as _color_pb2
from vstimd_client._proto.vstimd.v1.stimuli import identity_pb2 as _identity_pb2
from vstimd_client._proto.vstimd.v1 import transform_pb2 as _transform_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class WaveformType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    WAVEFORM_TYPE_UNSPECIFIED: _ClassVar[WaveformType]
    WAVEFORM_TYPE_SIN: _ClassVar[WaveformType]
    WAVEFORM_TYPE_SQR: _ClassVar[WaveformType]
    WAVEFORM_TYPE_SAW: _ClassVar[WaveformType]
    WAVEFORM_TYPE_TRI: _ClassVar[WaveformType]

class MaskType(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    MASK_TYPE_UNSPECIFIED: _ClassVar[MaskType]
    MASK_TYPE_NONE: _ClassVar[MaskType]
    MASK_TYPE_CIRCLE: _ClassVar[MaskType]
    MASK_TYPE_GAUSS: _ClassVar[MaskType]
    MASK_TYPE_HANN: _ClassVar[MaskType]
    MASK_TYPE_RAISED_COS: _ClassVar[MaskType]
WAVEFORM_TYPE_UNSPECIFIED: WaveformType
WAVEFORM_TYPE_SIN: WaveformType
WAVEFORM_TYPE_SQR: WaveformType
WAVEFORM_TYPE_SAW: WaveformType
WAVEFORM_TYPE_TRI: WaveformType
MASK_TYPE_UNSPECIFIED: MaskType
MASK_TYPE_NONE: MaskType
MASK_TYPE_CIRCLE: MaskType
MASK_TYPE_GAUSS: MaskType
MASK_TYPE_HANN: MaskType
MASK_TYPE_RAISED_COS: MaskType

class CreateGratingRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform2D
    params: GratingParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform2D, _Mapping]] = ..., params: _Optional[_Union[GratingParams, _Mapping]] = ...) -> None: ...

class SetGratingPhaseRequest(_message.Message):
    __slots__ = ("phase_cycles",)
    PHASE_CYCLES_FIELD_NUMBER: _ClassVar[int]
    phase_cycles: float
    def __init__(self, phase_cycles: _Optional[float] = ...) -> None: ...

class SetGratingSfRequest(_message.Message):
    __slots__ = ("sf_cycles_per_px",)
    SF_CYCLES_PER_PX_FIELD_NUMBER: _ClassVar[int]
    sf_cycles_per_px: float
    def __init__(self, sf_cycles_per_px: _Optional[float] = ...) -> None: ...

class SetGratingContrastRequest(_message.Message):
    __slots__ = ("contrast",)
    CONTRAST_FIELD_NUMBER: _ClassVar[int]
    contrast: float
    def __init__(self, contrast: _Optional[float] = ...) -> None: ...

class SetGratingWaveformRequest(_message.Message):
    __slots__ = ("waveform",)
    WAVEFORM_FIELD_NUMBER: _ClassVar[int]
    waveform: WaveformType
    def __init__(self, waveform: _Optional[_Union[WaveformType, str]] = ...) -> None: ...

class SetGratingMaskRequest(_message.Message):
    __slots__ = ("mask",)
    MASK_FIELD_NUMBER: _ClassVar[int]
    mask: MaskType
    def __init__(self, mask: _Optional[_Union[MaskType, str]] = ...) -> None: ...

class SetGratingDriftSpeedRequest(_message.Message):
    __slots__ = ("speed_hz",)
    SPEED_HZ_FIELD_NUMBER: _ClassVar[int]
    speed_hz: float
    def __init__(self, speed_hz: _Optional[float] = ...) -> None: ...

class SetGratingDriftDecoupledRequest(_message.Message):
    __slots__ = ("decoupled",)
    DECOUPLED_FIELD_NUMBER: _ClassVar[int]
    decoupled: bool
    def __init__(self, decoupled: bool = ...) -> None: ...

class SetGratingDriftAngleRequest(_message.Message):
    __slots__ = ("drift_angle_deg",)
    DRIFT_ANGLE_DEG_FIELD_NUMBER: _ClassVar[int]
    drift_angle_deg: float
    def __init__(self, drift_angle_deg: _Optional[float] = ...) -> None: ...

class SetGratingForeColorRequest(_message.Message):
    __slots__ = ("fore_color",)
    FORE_COLOR_FIELD_NUMBER: _ClassVar[int]
    fore_color: _color_pb2.Color
    def __init__(self, fore_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...

class SetGratingBackColorRequest(_message.Message):
    __slots__ = ("back_color",)
    BACK_COLOR_FIELD_NUMBER: _ClassVar[int]
    back_color: _color_pb2.Color
    def __init__(self, back_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...

class GratingParams(_message.Message):
    __slots__ = ("width_px", "height_px", "sf_cycles_per_px", "phase_cycles", "contrast", "waveform", "mask", "mask_param", "drift_speed_hz", "drift_decoupled", "drift_angle_deg", "fore_color", "back_color")
    WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    SF_CYCLES_PER_PX_FIELD_NUMBER: _ClassVar[int]
    PHASE_CYCLES_FIELD_NUMBER: _ClassVar[int]
    CONTRAST_FIELD_NUMBER: _ClassVar[int]
    WAVEFORM_FIELD_NUMBER: _ClassVar[int]
    MASK_FIELD_NUMBER: _ClassVar[int]
    MASK_PARAM_FIELD_NUMBER: _ClassVar[int]
    DRIFT_SPEED_HZ_FIELD_NUMBER: _ClassVar[int]
    DRIFT_DECOUPLED_FIELD_NUMBER: _ClassVar[int]
    DRIFT_ANGLE_DEG_FIELD_NUMBER: _ClassVar[int]
    FORE_COLOR_FIELD_NUMBER: _ClassVar[int]
    BACK_COLOR_FIELD_NUMBER: _ClassVar[int]
    width_px: float
    height_px: float
    sf_cycles_per_px: float
    phase_cycles: float
    contrast: float
    waveform: WaveformType
    mask: MaskType
    mask_param: float
    drift_speed_hz: float
    drift_decoupled: bool
    drift_angle_deg: float
    fore_color: _color_pb2.Color
    back_color: _color_pb2.Color
    def __init__(self, width_px: _Optional[float] = ..., height_px: _Optional[float] = ..., sf_cycles_per_px: _Optional[float] = ..., phase_cycles: _Optional[float] = ..., contrast: _Optional[float] = ..., waveform: _Optional[_Union[WaveformType, str]] = ..., mask: _Optional[_Union[MaskType, str]] = ..., mask_param: _Optional[float] = ..., drift_speed_hz: _Optional[float] = ..., drift_decoupled: bool = ..., drift_angle_deg: _Optional[float] = ..., fore_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., back_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...
