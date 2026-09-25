from vstimd_client._proto.vstimd.v1 import animations_pb2 as _animations_pb2
from vstimd_client._proto.vstimd.v1 import input_pb2 as _input_pb2
from vstimd_client._proto.vstimd.v1.stimuli import query_pb2 as _query_pb2
from vstimd_client._proto.vstimd.v1 import system_pb2 as _system_pb2
from vstimd_client._proto.vstimd.v1 import vtl_pb2 as _vtl_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CommandLogEntry(_message.Message):
    __slots__ = ("handle", "summary", "code", "server_time_ns")
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    SUMMARY_FIELD_NUMBER: _ClassVar[int]
    CODE_FIELD_NUMBER: _ClassVar[int]
    SERVER_TIME_NS_FIELD_NUMBER: _ClassVar[int]
    handle: int
    summary: str
    code: int
    server_time_ns: int
    def __init__(self, handle: _Optional[int] = ..., summary: _Optional[str] = ..., code: _Optional[int] = ..., server_time_ns: _Optional[int] = ...) -> None: ...

class SceneSnapshot(_message.Message):
    __slots__ = ("server_info", "stimuli", "animations", "vtl_lines", "vtl_state", "command_log", "frame_count", "server_time_ns", "input_devices")
    SERVER_INFO_FIELD_NUMBER: _ClassVar[int]
    STIMULI_FIELD_NUMBER: _ClassVar[int]
    ANIMATIONS_FIELD_NUMBER: _ClassVar[int]
    VTL_LINES_FIELD_NUMBER: _ClassVar[int]
    VTL_STATE_FIELD_NUMBER: _ClassVar[int]
    COMMAND_LOG_FIELD_NUMBER: _ClassVar[int]
    FRAME_COUNT_FIELD_NUMBER: _ClassVar[int]
    SERVER_TIME_NS_FIELD_NUMBER: _ClassVar[int]
    INPUT_DEVICES_FIELD_NUMBER: _ClassVar[int]
    server_info: _system_pb2.QueryServerInfoResponse
    stimuli: _containers.RepeatedCompositeFieldContainer[_query_pb2.QueryStimulusResponse]
    animations: _animations_pb2.ListAnimationsResponse
    vtl_lines: _vtl_pb2.ListVirtualTriggerLinesResponse
    vtl_state: _vtl_pb2.VirtualTriggerLineStateResponse
    command_log: _containers.RepeatedCompositeFieldContainer[CommandLogEntry]
    frame_count: int
    server_time_ns: int
    input_devices: _input_pb2.ListInputDevicesResponse
    def __init__(self, server_info: _Optional[_Union[_system_pb2.QueryServerInfoResponse, _Mapping]] = ..., stimuli: _Optional[_Iterable[_Union[_query_pb2.QueryStimulusResponse, _Mapping]]] = ..., animations: _Optional[_Union[_animations_pb2.ListAnimationsResponse, _Mapping]] = ..., vtl_lines: _Optional[_Union[_vtl_pb2.ListVirtualTriggerLinesResponse, _Mapping]] = ..., vtl_state: _Optional[_Union[_vtl_pb2.VirtualTriggerLineStateResponse, _Mapping]] = ..., command_log: _Optional[_Iterable[_Union[CommandLogEntry, _Mapping]]] = ..., frame_count: _Optional[int] = ..., server_time_ns: _Optional[int] = ..., input_devices: _Optional[_Union[_input_pb2.ListInputDevicesResponse, _Mapping]] = ...) -> None: ...
