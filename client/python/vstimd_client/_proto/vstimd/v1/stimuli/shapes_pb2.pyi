from vstimd_client._proto.vstimd.v1 import color_pb2 as _color_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ShapeDrawMode(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SHAPE_DRAW_MODE_UNSPECIFIED: _ClassVar[ShapeDrawMode]
    SHAPE_DRAW_MODE_FILLED: _ClassVar[ShapeDrawMode]
    SHAPE_DRAW_MODE_OUTLINED: _ClassVar[ShapeDrawMode]
    SHAPE_DRAW_MODE_FILLED_AND_OUTLINED: _ClassVar[ShapeDrawMode]
SHAPE_DRAW_MODE_UNSPECIFIED: ShapeDrawMode
SHAPE_DRAW_MODE_FILLED: ShapeDrawMode
SHAPE_DRAW_MODE_OUTLINED: ShapeDrawMode
SHAPE_DRAW_MODE_FILLED_AND_OUTLINED: ShapeDrawMode

class SetDrawModeRequest(_message.Message):
    __slots__ = ("mode",)
    MODE_FIELD_NUMBER: _ClassVar[int]
    mode: ShapeDrawMode
    def __init__(self, mode: _Optional[_Union[ShapeDrawMode, str]] = ...) -> None: ...

class ShapeAppearance(_message.Message):
    __slots__ = ("fill_color", "outline_color", "outline_width_px", "draw_mode")
    FILL_COLOR_FIELD_NUMBER: _ClassVar[int]
    OUTLINE_COLOR_FIELD_NUMBER: _ClassVar[int]
    OUTLINE_WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    DRAW_MODE_FIELD_NUMBER: _ClassVar[int]
    fill_color: _color_pb2.Color
    outline_color: _color_pb2.Color
    outline_width_px: float
    draw_mode: ShapeDrawMode
    def __init__(self, fill_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., outline_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., outline_width_px: _Optional[float] = ..., draw_mode: _Optional[_Union[ShapeDrawMode, str]] = ...) -> None: ...

class SetOutlineColorRequest(_message.Message):
    __slots__ = ("color",)
    COLOR_FIELD_NUMBER: _ClassVar[int]
    color: _color_pb2.Color
    def __init__(self, color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...

class SetOutlineWidthRequest(_message.Message):
    __slots__ = ("line_width_px",)
    LINE_WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    line_width_px: float
    def __init__(self, line_width_px: _Optional[float] = ...) -> None: ...
