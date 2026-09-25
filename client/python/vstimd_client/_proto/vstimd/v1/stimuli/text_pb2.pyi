from vstimd_client._proto.vstimd.v1 import vec2_pb2 as _vec2_pb2
from vstimd_client._proto.vstimd.v1 import color_pb2 as _color_pb2
from vstimd_client._proto.vstimd.v1.stimuli import identity_pb2 as _identity_pb2
from vstimd_client._proto.vstimd.v1 import transform_pb2 as _transform_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class LanguageStyle(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    LANGUAGE_STYLE_UNSPECIFIED: _ClassVar[LanguageStyle]
    LANGUAGE_STYLE_LTR: _ClassVar[LanguageStyle]
    LANGUAGE_STYLE_RTL: _ClassVar[LanguageStyle]
    LANGUAGE_STYLE_ARABIC: _ClassVar[LanguageStyle]
LANGUAGE_STYLE_UNSPECIFIED: LanguageStyle
LANGUAGE_STYLE_LTR: LanguageStyle
LANGUAGE_STYLE_RTL: LanguageStyle
LANGUAGE_STYLE_ARABIC: LanguageStyle

class CreateTextRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform2D
    params: TextParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform2D, _Mapping]] = ..., params: _Optional[_Union[TextParams, _Mapping]] = ...) -> None: ...

class SetTextRequest(_message.Message):
    __slots__ = ("text",)
    TEXT_FIELD_NUMBER: _ClassVar[int]
    text: str
    def __init__(self, text: _Optional[str] = ...) -> None: ...

class SetTextColorRequest(_message.Message):
    __slots__ = ("color",)
    COLOR_FIELD_NUMBER: _ClassVar[int]
    color: _color_pb2.Color
    def __init__(self, color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...

class TextParams(_message.Message):
    __slots__ = ("text", "font", "letter_height_px", "box_size_px", "anchor", "fill_color", "border_color", "flip_horiz", "language_style", "text_color")
    TEXT_FIELD_NUMBER: _ClassVar[int]
    FONT_FIELD_NUMBER: _ClassVar[int]
    LETTER_HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    BOX_SIZE_PX_FIELD_NUMBER: _ClassVar[int]
    ANCHOR_FIELD_NUMBER: _ClassVar[int]
    FILL_COLOR_FIELD_NUMBER: _ClassVar[int]
    BORDER_COLOR_FIELD_NUMBER: _ClassVar[int]
    FLIP_HORIZ_FIELD_NUMBER: _ClassVar[int]
    LANGUAGE_STYLE_FIELD_NUMBER: _ClassVar[int]
    TEXT_COLOR_FIELD_NUMBER: _ClassVar[int]
    text: str
    font: str
    letter_height_px: float
    box_size_px: _vec2_pb2.Vec2
    anchor: str
    fill_color: _color_pb2.Color
    border_color: _color_pb2.Color
    flip_horiz: bool
    language_style: LanguageStyle
    text_color: _color_pb2.Color
    def __init__(self, text: _Optional[str] = ..., font: _Optional[str] = ..., letter_height_px: _Optional[float] = ..., box_size_px: _Optional[_Union[_vec2_pb2.Vec2, _Mapping]] = ..., anchor: _Optional[str] = ..., fill_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., border_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., flip_horiz: bool = ..., language_style: _Optional[_Union[LanguageStyle, str]] = ..., text_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...
