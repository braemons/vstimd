from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class InputSemantic(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    INPUT_SEMANTIC_UNSPECIFIED: _ClassVar[InputSemantic]
    INPUT_SEMANTIC_ABSOLUTE: _ClassVar[InputSemantic]
    INPUT_SEMANTIC_CUMULATIVE: _ClassVar[InputSemantic]
    INPUT_SEMANTIC_RATE: _ClassVar[InputSemantic]
INPUT_SEMANTIC_UNSPECIFIED: InputSemantic
INPUT_SEMANTIC_ABSOLUTE: InputSemantic
INPUT_SEMANTIC_CUMULATIVE: InputSemantic
INPUT_SEMANTIC_RATE: InputSemantic

class InputAxisInfo(_message.Message):
    __slots__ = ("name", "semantic", "scale", "value", "delta")
    NAME_FIELD_NUMBER: _ClassVar[int]
    SEMANTIC_FIELD_NUMBER: _ClassVar[int]
    SCALE_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    DELTA_FIELD_NUMBER: _ClassVar[int]
    name: str
    semantic: InputSemantic
    scale: float
    value: float
    delta: float
    def __init__(self, name: _Optional[str] = ..., semantic: _Optional[_Union[InputSemantic, str]] = ..., scale: _Optional[float] = ..., value: _Optional[float] = ..., delta: _Optional[float] = ...) -> None: ...

class InputDeviceInfo(_message.Message):
    __slots__ = ("name", "backend", "connected", "stale", "torn_reads", "axes", "starved_frames")
    NAME_FIELD_NUMBER: _ClassVar[int]
    BACKEND_FIELD_NUMBER: _ClassVar[int]
    CONNECTED_FIELD_NUMBER: _ClassVar[int]
    STALE_FIELD_NUMBER: _ClassVar[int]
    TORN_READS_FIELD_NUMBER: _ClassVar[int]
    AXES_FIELD_NUMBER: _ClassVar[int]
    STARVED_FRAMES_FIELD_NUMBER: _ClassVar[int]
    name: str
    backend: str
    connected: bool
    stale: bool
    torn_reads: int
    axes: _containers.RepeatedCompositeFieldContainer[InputAxisInfo]
    starved_frames: int
    def __init__(self, name: _Optional[str] = ..., backend: _Optional[str] = ..., connected: bool = ..., stale: bool = ..., torn_reads: _Optional[int] = ..., axes: _Optional[_Iterable[_Union[InputAxisInfo, _Mapping]]] = ..., starved_frames: _Optional[int] = ...) -> None: ...

class ListInputDevicesRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListInputDevicesResponse(_message.Message):
    __slots__ = ("devices",)
    DEVICES_FIELD_NUMBER: _ClassVar[int]
    devices: _containers.RepeatedCompositeFieldContainer[InputDeviceInfo]
    def __init__(self, devices: _Optional[_Iterable[_Union[InputDeviceInfo, _Mapping]]] = ...) -> None: ...
