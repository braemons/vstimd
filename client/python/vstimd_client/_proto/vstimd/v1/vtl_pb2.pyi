from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class VirtualTriggerLineKind(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    VIRTUAL_TRIGGER_LINE_KIND_UNSPECIFIED: _ClassVar[VirtualTriggerLineKind]
    VIRTUAL_TRIGGER_LINE_KIND_INPUT: _ClassVar[VirtualTriggerLineKind]
    VIRTUAL_TRIGGER_LINE_KIND_OUTPUT: _ClassVar[VirtualTriggerLineKind]

class VtlEdge(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    VTL_EDGE_RISING: _ClassVar[VtlEdge]
    VTL_EDGE_FALLING: _ClassVar[VtlEdge]

class VtlPolarity(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    VTL_POLARITY_ACTIVE_LOW: _ClassVar[VtlPolarity]
    VTL_POLARITY_ACTIVE_HIGH: _ClassVar[VtlPolarity]
VIRTUAL_TRIGGER_LINE_KIND_UNSPECIFIED: VirtualTriggerLineKind
VIRTUAL_TRIGGER_LINE_KIND_INPUT: VirtualTriggerLineKind
VIRTUAL_TRIGGER_LINE_KIND_OUTPUT: VirtualTriggerLineKind
VTL_EDGE_RISING: VtlEdge
VTL_EDGE_FALLING: VtlEdge
VTL_POLARITY_ACTIVE_LOW: VtlPolarity
VTL_POLARITY_ACTIVE_HIGH: VtlPolarity

class VirtualTriggerLineBankBit(_message.Message):
    __slots__ = ("bank", "bit")
    BANK_FIELD_NUMBER: _ClassVar[int]
    BIT_FIELD_NUMBER: _ClassVar[int]
    bank: int
    bit: int
    def __init__(self, bank: _Optional[int] = ..., bit: _Optional[int] = ...) -> None: ...

class VirtualTriggerLineHandle(_message.Message):
    __slots__ = ("bank_bit", "name", "kind")
    BANK_BIT_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    KIND_FIELD_NUMBER: _ClassVar[int]
    bank_bit: VirtualTriggerLineBankBit
    name: str
    kind: VirtualTriggerLineKind
    def __init__(self, bank_bit: _Optional[_Union[VirtualTriggerLineBankBit, _Mapping]] = ..., name: _Optional[str] = ..., kind: _Optional[_Union[VirtualTriggerLineKind, str]] = ...) -> None: ...

class SetVirtualTriggerLineNameRequest(_message.Message):
    __slots__ = ("bank", "bit", "kind", "name")
    BANK_FIELD_NUMBER: _ClassVar[int]
    BIT_FIELD_NUMBER: _ClassVar[int]
    KIND_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    bank: int
    bit: int
    kind: VirtualTriggerLineKind
    name: str
    def __init__(self, bank: _Optional[int] = ..., bit: _Optional[int] = ..., kind: _Optional[_Union[VirtualTriggerLineKind, str]] = ..., name: _Optional[str] = ...) -> None: ...

class ListVirtualTriggerLinesRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class VirtualTriggerLineInfo(_message.Message):
    __slots__ = ("name", "bank", "bit", "kind", "high")
    NAME_FIELD_NUMBER: _ClassVar[int]
    BANK_FIELD_NUMBER: _ClassVar[int]
    BIT_FIELD_NUMBER: _ClassVar[int]
    KIND_FIELD_NUMBER: _ClassVar[int]
    HIGH_FIELD_NUMBER: _ClassVar[int]
    name: str
    bank: int
    bit: int
    kind: VirtualTriggerLineKind
    high: bool
    def __init__(self, name: _Optional[str] = ..., bank: _Optional[int] = ..., bit: _Optional[int] = ..., kind: _Optional[_Union[VirtualTriggerLineKind, str]] = ..., high: bool = ...) -> None: ...

class ListVirtualTriggerLinesResponse(_message.Message):
    __slots__ = ("lines",)
    LINES_FIELD_NUMBER: _ClassVar[int]
    lines: _containers.RepeatedCompositeFieldContainer[VirtualTriggerLineInfo]
    def __init__(self, lines: _Optional[_Iterable[_Union[VirtualTriggerLineInfo, _Mapping]]] = ...) -> None: ...

class SetVirtualTriggerLineRequest(_message.Message):
    __slots__ = ("handle", "value")
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    handle: VirtualTriggerLineHandle
    value: bool
    def __init__(self, handle: _Optional[_Union[VirtualTriggerLineHandle, _Mapping]] = ..., value: bool = ...) -> None: ...

class ToggleVirtualTriggerLineRequest(_message.Message):
    __slots__ = ("handle",)
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    handle: VirtualTriggerLineHandle
    def __init__(self, handle: _Optional[_Union[VirtualTriggerLineHandle, _Mapping]] = ...) -> None: ...

class ClearVirtualTriggerLineLatchesRequest(_message.Message):
    __slots__ = ("handle",)
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    handle: VirtualTriggerLineHandle
    def __init__(self, handle: _Optional[_Union[VirtualTriggerLineHandle, _Mapping]] = ...) -> None: ...

class SetVirtualTriggerLineBankRequest(_message.Message):
    __slots__ = ("kind", "bank", "value")
    KIND_FIELD_NUMBER: _ClassVar[int]
    BANK_FIELD_NUMBER: _ClassVar[int]
    VALUE_FIELD_NUMBER: _ClassVar[int]
    kind: VirtualTriggerLineKind
    bank: int
    value: int
    def __init__(self, kind: _Optional[_Union[VirtualTriggerLineKind, str]] = ..., bank: _Optional[int] = ..., value: _Optional[int] = ...) -> None: ...

class VirtualTriggerLineStateResponse(_message.Message):
    __slots__ = ("high",)
    HIGH_FIELD_NUMBER: _ClassVar[int]
    high: bool
    def __init__(self, high: bool = ...) -> None: ...
