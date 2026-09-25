from vstimd_client._proto.vstimd.v1 import color_pb2 as _color_pb2
from vstimd_client._proto.vstimd.v1.stimuli import identity_pb2 as _identity_pb2
from vstimd_client._proto.vstimd.v1 import transform_pb2 as _transform_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class DotShape(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    DOT_SHAPE_UNSPECIFIED: _ClassVar[DotShape]
    DOT_SHAPE_ROUND: _ClassVar[DotShape]
    DOT_SHAPE_SQUARE: _ClassVar[DotShape]

class ApertureShape(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    APERTURE_SHAPE_UNSPECIFIED: _ClassVar[ApertureShape]
    APERTURE_SHAPE_RECT: _ClassVar[ApertureShape]
    APERTURE_SHAPE_CIRCLE: _ClassVar[ApertureShape]

class ApertureClip(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    APERTURE_CLIP_UNSPECIFIED: _ClassVar[ApertureClip]
    APERTURE_CLIP_DOT_CENTER: _ClassVar[ApertureClip]
    APERTURE_CLIP_PIXEL: _ClassVar[ApertureClip]

class SignalRule(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SIGNAL_RULE_UNSPECIFIED: _ClassVar[SignalRule]
    SIGNAL_RULE_SAME: _ClassVar[SignalRule]
    SIGNAL_RULE_DIFFERENT: _ClassVar[SignalRule]

class NoiseRule(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    NOISE_RULE_UNSPECIFIED: _ClassVar[NoiseRule]
    NOISE_RULE_POSITION: _ClassVar[NoiseRule]
    NOISE_RULE_DIRECTION: _ClassVar[NoiseRule]
    NOISE_RULE_WALK: _ClassVar[NoiseRule]

class Reinsertion(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    REINSERTION_UNSPECIFIED: _ClassVar[Reinsertion]
    REINSERTION_WRAP: _ClassVar[Reinsertion]
    REINSERTION_RESPAWN: _ClassVar[Reinsertion]
DOT_SHAPE_UNSPECIFIED: DotShape
DOT_SHAPE_ROUND: DotShape
DOT_SHAPE_SQUARE: DotShape
APERTURE_SHAPE_UNSPECIFIED: ApertureShape
APERTURE_SHAPE_RECT: ApertureShape
APERTURE_SHAPE_CIRCLE: ApertureShape
APERTURE_CLIP_UNSPECIFIED: ApertureClip
APERTURE_CLIP_DOT_CENTER: ApertureClip
APERTURE_CLIP_PIXEL: ApertureClip
SIGNAL_RULE_UNSPECIFIED: SignalRule
SIGNAL_RULE_SAME: SignalRule
SIGNAL_RULE_DIFFERENT: SignalRule
NOISE_RULE_UNSPECIFIED: NoiseRule
NOISE_RULE_POSITION: NoiseRule
NOISE_RULE_DIRECTION: NoiseRule
NOISE_RULE_WALK: NoiseRule
REINSERTION_UNSPECIFIED: Reinsertion
REINSERTION_WRAP: Reinsertion
REINSERTION_RESPAWN: Reinsertion

class Aperture(_message.Message):
    __slots__ = ("shape", "width_px", "height_px", "offset_x_px", "offset_y_px", "invert", "clip")
    SHAPE_FIELD_NUMBER: _ClassVar[int]
    WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    OFFSET_X_PX_FIELD_NUMBER: _ClassVar[int]
    OFFSET_Y_PX_FIELD_NUMBER: _ClassVar[int]
    INVERT_FIELD_NUMBER: _ClassVar[int]
    CLIP_FIELD_NUMBER: _ClassVar[int]
    shape: ApertureShape
    width_px: float
    height_px: float
    offset_x_px: float
    offset_y_px: float
    invert: bool
    clip: ApertureClip
    def __init__(self, shape: _Optional[_Union[ApertureShape, str]] = ..., width_px: _Optional[float] = ..., height_px: _Optional[float] = ..., offset_x_px: _Optional[float] = ..., offset_y_px: _Optional[float] = ..., invert: bool = ..., clip: _Optional[_Union[ApertureClip, str]] = ...) -> None: ...

class CreateDotsRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform2D
    params: DotsParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform2D, _Mapping]] = ..., params: _Optional[_Union[DotsParams, _Mapping]] = ...) -> None: ...

class SetDotsDirectionRequest(_message.Message):
    __slots__ = ("direction_deg",)
    DIRECTION_DEG_FIELD_NUMBER: _ClassVar[int]
    direction_deg: float
    def __init__(self, direction_deg: _Optional[float] = ...) -> None: ...

class SetDotsSpeedRequest(_message.Message):
    __slots__ = ("speed_px_per_s",)
    SPEED_PX_PER_S_FIELD_NUMBER: _ClassVar[int]
    speed_px_per_s: float
    def __init__(self, speed_px_per_s: _Optional[float] = ...) -> None: ...

class SetDotsCoherenceRequest(_message.Message):
    __slots__ = ("coherence",)
    COHERENCE_FIELD_NUMBER: _ClassVar[int]
    coherence: float
    def __init__(self, coherence: _Optional[float] = ...) -> None: ...

class SetDotsCountRequest(_message.Message):
    __slots__ = ("dot_count",)
    DOT_COUNT_FIELD_NUMBER: _ClassVar[int]
    dot_count: int
    def __init__(self, dot_count: _Optional[int] = ...) -> None: ...

class SetDotsSizeRequest(_message.Message):
    __slots__ = ("dot_size_px",)
    DOT_SIZE_PX_FIELD_NUMBER: _ClassVar[int]
    dot_size_px: float
    def __init__(self, dot_size_px: _Optional[float] = ...) -> None: ...

class SetDotsColorRequest(_message.Message):
    __slots__ = ("dot_color", "dot_color_alt")
    DOT_COLOR_FIELD_NUMBER: _ClassVar[int]
    DOT_COLOR_ALT_FIELD_NUMBER: _ClassVar[int]
    dot_color: _color_pb2.Color
    dot_color_alt: _color_pb2.Color
    def __init__(self, dot_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., dot_color_alt: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...

class SetDotsApertureRequest(_message.Message):
    __slots__ = ("aperture",)
    APERTURE_FIELD_NUMBER: _ClassVar[int]
    aperture: Aperture
    def __init__(self, aperture: _Optional[_Union[Aperture, _Mapping]] = ...) -> None: ...

class SetDotsFieldSizeRequest(_message.Message):
    __slots__ = ("width_px", "height_px")
    WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    width_px: float
    height_px: float
    def __init__(self, width_px: _Optional[float] = ..., height_px: _Optional[float] = ...) -> None: ...

class SetDotsLifetimeRequest(_message.Message):
    __slots__ = ("dot_lifetime_frames",)
    DOT_LIFETIME_FRAMES_FIELD_NUMBER: _ClassVar[int]
    dot_lifetime_frames: int
    def __init__(self, dot_lifetime_frames: _Optional[int] = ...) -> None: ...

class SetDotsSeedRequest(_message.Message):
    __slots__ = ("seed",)
    SEED_FIELD_NUMBER: _ClassVar[int]
    seed: int
    def __init__(self, seed: _Optional[int] = ...) -> None: ...

class DotsParams(_message.Message):
    __slots__ = ("field_width_px", "field_height_px", "dot_count", "aperture", "dot_size_px", "dot_color", "dot_color_alt", "dot_shape", "direction_deg", "speed_px_per_s", "coherence", "signal_rule", "noise_rule", "reinsertion", "dot_lifetime_frames", "seed")
    FIELD_WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    FIELD_HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    DOT_COUNT_FIELD_NUMBER: _ClassVar[int]
    APERTURE_FIELD_NUMBER: _ClassVar[int]
    DOT_SIZE_PX_FIELD_NUMBER: _ClassVar[int]
    DOT_COLOR_FIELD_NUMBER: _ClassVar[int]
    DOT_COLOR_ALT_FIELD_NUMBER: _ClassVar[int]
    DOT_SHAPE_FIELD_NUMBER: _ClassVar[int]
    DIRECTION_DEG_FIELD_NUMBER: _ClassVar[int]
    SPEED_PX_PER_S_FIELD_NUMBER: _ClassVar[int]
    COHERENCE_FIELD_NUMBER: _ClassVar[int]
    SIGNAL_RULE_FIELD_NUMBER: _ClassVar[int]
    NOISE_RULE_FIELD_NUMBER: _ClassVar[int]
    REINSERTION_FIELD_NUMBER: _ClassVar[int]
    DOT_LIFETIME_FRAMES_FIELD_NUMBER: _ClassVar[int]
    SEED_FIELD_NUMBER: _ClassVar[int]
    field_width_px: float
    field_height_px: float
    dot_count: int
    aperture: Aperture
    dot_size_px: float
    dot_color: _color_pb2.Color
    dot_color_alt: _color_pb2.Color
    dot_shape: DotShape
    direction_deg: float
    speed_px_per_s: float
    coherence: float
    signal_rule: SignalRule
    noise_rule: NoiseRule
    reinsertion: Reinsertion
    dot_lifetime_frames: int
    seed: int
    def __init__(self, field_width_px: _Optional[float] = ..., field_height_px: _Optional[float] = ..., dot_count: _Optional[int] = ..., aperture: _Optional[_Union[Aperture, _Mapping]] = ..., dot_size_px: _Optional[float] = ..., dot_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., dot_color_alt: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., dot_shape: _Optional[_Union[DotShape, str]] = ..., direction_deg: _Optional[float] = ..., speed_px_per_s: _Optional[float] = ..., coherence: _Optional[float] = ..., signal_rule: _Optional[_Union[SignalRule, str]] = ..., noise_rule: _Optional[_Union[NoiseRule, str]] = ..., reinsertion: _Optional[_Union[Reinsertion, str]] = ..., dot_lifetime_frames: _Optional[int] = ..., seed: _Optional[int] = ...) -> None: ...
