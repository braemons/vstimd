from vstimd_client._proto.vstimd.v1 import color_pb2 as _color_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class SetEnabledRequest(_message.Message):
    __slots__ = ("enabled",)
    ENABLED_FIELD_NUMBER: _ClassVar[int]
    enabled: bool
    def __init__(self, enabled: bool = ...) -> None: ...

class DeleteRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class SetNameRequest(_message.Message):
    __slots__ = ("name",)
    NAME_FIELD_NUMBER: _ClassVar[int]
    name: str
    def __init__(self, name: _Optional[str] = ...) -> None: ...

class SetPositionRequest(_message.Message):
    __slots__ = ("x_px", "y_px")
    X_PX_FIELD_NUMBER: _ClassVar[int]
    Y_PX_FIELD_NUMBER: _ClassVar[int]
    x_px: float
    y_px: float
    def __init__(self, x_px: _Optional[float] = ..., y_px: _Optional[float] = ...) -> None: ...

class SetRotationRequest(_message.Message):
    __slots__ = ("rotation_deg",)
    ROTATION_DEG_FIELD_NUMBER: _ClassVar[int]
    rotation_deg: float
    def __init__(self, rotation_deg: _Optional[float] = ...) -> None: ...

class SetFillColorRequest(_message.Message):
    __slots__ = ("color",)
    COLOR_FIELD_NUMBER: _ClassVar[int]
    color: _color_pb2.Color
    def __init__(self, color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...

class SetAlphaRequest(_message.Message):
    __slots__ = ("opacity",)
    OPACITY_FIELD_NUMBER: _ClassVar[int]
    opacity: float
    def __init__(self, opacity: _Optional[float] = ...) -> None: ...

class BringToFrontRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class SendToBackRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class SwapDrawOrderRequest(_message.Message):
    __slots__ = ("handle_a", "handle_b")
    HANDLE_A_FIELD_NUMBER: _ClassVar[int]
    HANDLE_B_FIELD_NUMBER: _ClassVar[int]
    handle_a: int
    handle_b: int
    def __init__(self, handle_a: _Optional[int] = ..., handle_b: _Optional[int] = ...) -> None: ...
