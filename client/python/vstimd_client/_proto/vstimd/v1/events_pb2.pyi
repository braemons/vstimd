from vstimd_client._proto.vstimd.v1 import vtl_pb2 as _vtl_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Event(_message.Message):
    __slots__ = ("sequence", "topic_sequence", "monotonic_us", "frame", "topic", "frame_dropped", "frame_presented", "vtl_line_changed", "animation_state_changed", "server_started", "command_applied")
    SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    TOPIC_SEQUENCE_FIELD_NUMBER: _ClassVar[int]
    MONOTONIC_US_FIELD_NUMBER: _ClassVar[int]
    FRAME_FIELD_NUMBER: _ClassVar[int]
    TOPIC_FIELD_NUMBER: _ClassVar[int]
    FRAME_DROPPED_FIELD_NUMBER: _ClassVar[int]
    FRAME_PRESENTED_FIELD_NUMBER: _ClassVar[int]
    VTL_LINE_CHANGED_FIELD_NUMBER: _ClassVar[int]
    ANIMATION_STATE_CHANGED_FIELD_NUMBER: _ClassVar[int]
    SERVER_STARTED_FIELD_NUMBER: _ClassVar[int]
    COMMAND_APPLIED_FIELD_NUMBER: _ClassVar[int]
    sequence: int
    topic_sequence: int
    monotonic_us: int
    frame: int
    topic: str
    frame_dropped: FrameDropped
    frame_presented: FramePresented
    vtl_line_changed: VtlLineChanged
    animation_state_changed: AnimationStateChanged
    server_started: ServerStarted
    command_applied: CommandApplied
    def __init__(self, sequence: _Optional[int] = ..., topic_sequence: _Optional[int] = ..., monotonic_us: _Optional[int] = ..., frame: _Optional[int] = ..., topic: _Optional[str] = ..., frame_dropped: _Optional[_Union[FrameDropped, _Mapping]] = ..., frame_presented: _Optional[_Union[FramePresented, _Mapping]] = ..., vtl_line_changed: _Optional[_Union[VtlLineChanged, _Mapping]] = ..., animation_state_changed: _Optional[_Union[AnimationStateChanged, _Mapping]] = ..., server_started: _Optional[_Union[ServerStarted, _Mapping]] = ..., command_applied: _Optional[_Union[CommandApplied, _Mapping]] = ...) -> None: ...

class FrameDropped(_message.Message):
    __slots__ = ("count", "total_since_start")
    COUNT_FIELD_NUMBER: _ClassVar[int]
    TOTAL_SINCE_START_FIELD_NUMBER: _ClassVar[int]
    count: int
    total_since_start: int
    def __init__(self, count: _Optional[int] = ..., total_since_start: _Optional[int] = ...) -> None: ...

class FramePresented(_message.Message):
    __slots__ = ("since_previous_us",)
    SINCE_PREVIOUS_US_FIELD_NUMBER: _ClassVar[int]
    since_previous_us: int
    def __init__(self, since_previous_us: _Optional[int] = ...) -> None: ...

class VtlLineChanged(_message.Message):
    __slots__ = ("bank", "bit", "edge", "kind")
    BANK_FIELD_NUMBER: _ClassVar[int]
    BIT_FIELD_NUMBER: _ClassVar[int]
    EDGE_FIELD_NUMBER: _ClassVar[int]
    KIND_FIELD_NUMBER: _ClassVar[int]
    bank: int
    bit: int
    edge: _vtl_pb2.VtlEdge
    kind: _vtl_pb2.VirtualTriggerLineKind
    def __init__(self, bank: _Optional[int] = ..., bit: _Optional[int] = ..., edge: _Optional[_Union[_vtl_pb2.VtlEdge, str]] = ..., kind: _Optional[_Union[_vtl_pb2.VirtualTriggerLineKind, str]] = ...) -> None: ...

class AnimationStateChanged(_message.Message):
    __slots__ = ("handle", "state")
    class State(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
        __slots__ = ()
        STATE_UNSPECIFIED: _ClassVar[AnimationStateChanged.State]
        STATE_ARMED: _ClassVar[AnimationStateChanged.State]
        STATE_RUNNING: _ClassVar[AnimationStateChanged.State]
        STATE_DONE: _ClassVar[AnimationStateChanged.State]
    STATE_UNSPECIFIED: AnimationStateChanged.State
    STATE_ARMED: AnimationStateChanged.State
    STATE_RUNNING: AnimationStateChanged.State
    STATE_DONE: AnimationStateChanged.State
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    handle: int
    state: AnimationStateChanged.State
    def __init__(self, handle: _Optional[int] = ..., state: _Optional[_Union[AnimationStateChanged.State, str]] = ...) -> None: ...

class CommandApplied(_message.Message):
    __slots__ = ("request", "accepted", "response_handle", "error_code")
    REQUEST_FIELD_NUMBER: _ClassVar[int]
    ACCEPTED_FIELD_NUMBER: _ClassVar[int]
    RESPONSE_HANDLE_FIELD_NUMBER: _ClassVar[int]
    ERROR_CODE_FIELD_NUMBER: _ClassVar[int]
    request: bytes
    accepted: bool
    response_handle: int
    error_code: int
    def __init__(self, request: _Optional[bytes] = ..., accepted: bool = ..., response_handle: _Optional[int] = ..., error_code: _Optional[int] = ...) -> None: ...

class ServerStarted(_message.Message):
    __slots__ = ("instance_id", "version")
    INSTANCE_ID_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    instance_id: str
    version: str
    def __init__(self, instance_id: _Optional[str] = ..., version: _Optional[str] = ...) -> None: ...
