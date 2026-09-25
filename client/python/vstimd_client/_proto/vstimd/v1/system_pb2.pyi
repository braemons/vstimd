from vstimd_client._proto.vstimd.v1 import color_pb2 as _color_pb2
from vstimd_client._proto.vstimd.v1.stimuli import stimulus_type_pb2 as _stimulus_type_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class RenderBackend(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    RENDER_BACKEND_UNSPECIFIED: _ClassVar[RenderBackend]
    RENDER_BACKEND_DRM: _ClassVar[RenderBackend]
    RENDER_BACKEND_WINIT: _ClassVar[RenderBackend]
RENDER_BACKEND_UNSPECIFIED: RenderBackend
RENDER_BACKEND_DRM: RenderBackend
RENDER_BACKEND_WINIT: RenderBackend

class SetBackgroundRequest(_message.Message):
    __slots__ = ("color",)
    COLOR_FIELD_NUMBER: _ClassVar[int]
    color: _color_pb2.Color
    def __init__(self, color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ...) -> None: ...

class SetDeferredModeRequest(_message.Message):
    __slots__ = ("active", "cancel")
    ACTIVE_FIELD_NUMBER: _ClassVar[int]
    CANCEL_FIELD_NUMBER: _ClassVar[int]
    active: bool
    cancel: bool
    def __init__(self, active: bool = ..., cancel: bool = ...) -> None: ...

class SetDeferredModeResponse(_message.Message):
    __slots__ = ("deferred", "flip_scheduled", "was_deferred", "flip_frame")
    DEFERRED_FIELD_NUMBER: _ClassVar[int]
    FLIP_SCHEDULED_FIELD_NUMBER: _ClassVar[int]
    WAS_DEFERRED_FIELD_NUMBER: _ClassVar[int]
    FLIP_FRAME_FIELD_NUMBER: _ClassVar[int]
    deferred: bool
    flip_scheduled: bool
    was_deferred: bool
    flip_frame: int
    def __init__(self, deferred: bool = ..., flip_scheduled: bool = ..., was_deferred: bool = ..., flip_frame: _Optional[int] = ...) -> None: ...

class ClearStimuliRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ClearAnimationsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ClearAllRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class SetAllEnabledRequest(_message.Message):
    __slots__ = ("enabled",)
    ENABLED_FIELD_NUMBER: _ClassVar[int]
    enabled: bool
    def __init__(self, enabled: bool = ...) -> None: ...

class WaitForFramesRequest(_message.Message):
    __slots__ = ("count",)
    COUNT_FIELD_NUMBER: _ClassVar[int]
    count: int
    def __init__(self, count: _Optional[int] = ...) -> None: ...

class WaitUntilRequest(_message.Message):
    __slots__ = ("server_time_ns",)
    SERVER_TIME_NS_FIELD_NUMBER: _ClassVar[int]
    server_time_ns: int
    def __init__(self, server_time_ns: _Optional[int] = ...) -> None: ...

class QueryFrameStatsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ResetFrameStatsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class FrameStats(_message.Message):
    __slots__ = ("presented_frames", "dropped_frames", "mean_frame_interval_ms", "std_frame_interval_ms", "min_frame_interval_ms", "max_frame_interval_ms", "nominal_frame_interval_ms", "window_start_frame")
    PRESENTED_FRAMES_FIELD_NUMBER: _ClassVar[int]
    DROPPED_FRAMES_FIELD_NUMBER: _ClassVar[int]
    MEAN_FRAME_INTERVAL_MS_FIELD_NUMBER: _ClassVar[int]
    STD_FRAME_INTERVAL_MS_FIELD_NUMBER: _ClassVar[int]
    MIN_FRAME_INTERVAL_MS_FIELD_NUMBER: _ClassVar[int]
    MAX_FRAME_INTERVAL_MS_FIELD_NUMBER: _ClassVar[int]
    NOMINAL_FRAME_INTERVAL_MS_FIELD_NUMBER: _ClassVar[int]
    WINDOW_START_FRAME_FIELD_NUMBER: _ClassVar[int]
    presented_frames: int
    dropped_frames: int
    mean_frame_interval_ms: float
    std_frame_interval_ms: float
    min_frame_interval_ms: float
    max_frame_interval_ms: float
    nominal_frame_interval_ms: float
    window_start_frame: int
    def __init__(self, presented_frames: _Optional[int] = ..., dropped_frames: _Optional[int] = ..., mean_frame_interval_ms: _Optional[float] = ..., std_frame_interval_ms: _Optional[float] = ..., min_frame_interval_ms: _Optional[float] = ..., max_frame_interval_ms: _Optional[float] = ..., nominal_frame_interval_ms: _Optional[float] = ..., window_start_frame: _Optional[int] = ...) -> None: ...

class CaptureFrameRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class CaptureFrameResponse(_message.Message):
    __slots__ = ("png", "width_px", "height_px", "frame")
    PNG_FIELD_NUMBER: _ClassVar[int]
    WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    FRAME_FIELD_NUMBER: _ClassVar[int]
    png: bytes
    width_px: int
    height_px: int
    frame: int
    def __init__(self, png: _Optional[bytes] = ..., width_px: _Optional[int] = ..., height_px: _Optional[int] = ..., frame: _Optional[int] = ...) -> None: ...

class QueryServerInfoRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListStimuliRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class Version(_message.Message):
    __slots__ = ("major", "minor", "patch")
    MAJOR_FIELD_NUMBER: _ClassVar[int]
    MINOR_FIELD_NUMBER: _ClassVar[int]
    PATCH_FIELD_NUMBER: _ClassVar[int]
    major: int
    minor: int
    patch: int
    def __init__(self, major: _Optional[int] = ..., minor: _Optional[int] = ..., patch: _Optional[int] = ...) -> None: ...

class QueryServerInfoResponse(_message.Message):
    __slots__ = ("width_px", "height_px", "frame_rate_hz", "background_color", "backend", "version", "measured_frame_rate_hz")
    WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    FRAME_RATE_HZ_FIELD_NUMBER: _ClassVar[int]
    BACKGROUND_COLOR_FIELD_NUMBER: _ClassVar[int]
    BACKEND_FIELD_NUMBER: _ClassVar[int]
    VERSION_FIELD_NUMBER: _ClassVar[int]
    MEASURED_FRAME_RATE_HZ_FIELD_NUMBER: _ClassVar[int]
    width_px: int
    height_px: int
    frame_rate_hz: float
    background_color: _color_pb2.Color
    backend: RenderBackend
    version: Version
    measured_frame_rate_hz: float
    def __init__(self, width_px: _Optional[int] = ..., height_px: _Optional[int] = ..., frame_rate_hz: _Optional[float] = ..., background_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., backend: _Optional[_Union[RenderBackend, str]] = ..., version: _Optional[_Union[Version, _Mapping]] = ..., measured_frame_rate_hz: _Optional[float] = ...) -> None: ...

class StimulusEntry(_message.Message):
    __slots__ = ("handle", "stimulus_type", "enabled", "id", "name", "condition_indices")
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    STIMULUS_TYPE_FIELD_NUMBER: _ClassVar[int]
    ENABLED_FIELD_NUMBER: _ClassVar[int]
    ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    CONDITION_INDICES_FIELD_NUMBER: _ClassVar[int]
    handle: int
    stimulus_type: _stimulus_type_pb2.StimulusType
    enabled: bool
    id: str
    name: str
    condition_indices: _containers.RepeatedScalarFieldContainer[int]
    def __init__(self, handle: _Optional[int] = ..., stimulus_type: _Optional[_Union[_stimulus_type_pb2.StimulusType, str]] = ..., enabled: bool = ..., id: _Optional[str] = ..., name: _Optional[str] = ..., condition_indices: _Optional[_Iterable[int]] = ...) -> None: ...

class ListStimuliResponse(_message.Message):
    __slots__ = ("entries",)
    ENTRIES_FIELD_NUMBER: _ClassVar[int]
    entries: _containers.RepeatedCompositeFieldContainer[StimulusEntry]
    def __init__(self, entries: _Optional[_Iterable[_Union[StimulusEntry, _Mapping]]] = ...) -> None: ...

class ShutdownRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class ListSceneConfigsRequest(_message.Message):
    __slots__ = ("project",)
    PROJECT_FIELD_NUMBER: _ClassVar[int]
    project: str
    def __init__(self, project: _Optional[str] = ...) -> None: ...

class ListSceneConfigsResponse(_message.Message):
    __slots__ = ("names",)
    NAMES_FIELD_NUMBER: _ClassVar[int]
    names: _containers.RepeatedScalarFieldContainer[str]
    def __init__(self, names: _Optional[_Iterable[str]] = ...) -> None: ...

class LoadSceneConfigRequest(_message.Message):
    __slots__ = ("name", "additive")
    NAME_FIELD_NUMBER: _ClassVar[int]
    ADDITIVE_FIELD_NUMBER: _ClassVar[int]
    name: str
    additive: bool
    def __init__(self, name: _Optional[str] = ..., additive: bool = ...) -> None: ...

class UploadSceneConfigRequest(_message.Message):
    __slots__ = ("name", "json", "overwrite", "apply_now", "additive")
    NAME_FIELD_NUMBER: _ClassVar[int]
    JSON_FIELD_NUMBER: _ClassVar[int]
    OVERWRITE_FIELD_NUMBER: _ClassVar[int]
    APPLY_NOW_FIELD_NUMBER: _ClassVar[int]
    ADDITIVE_FIELD_NUMBER: _ClassVar[int]
    name: str
    json: str
    overwrite: bool
    apply_now: bool
    additive: bool
    def __init__(self, name: _Optional[str] = ..., json: _Optional[str] = ..., overwrite: bool = ..., apply_now: bool = ..., additive: bool = ...) -> None: ...

class RetrieveSceneConfigRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class RetrieveSceneConfigResponse(_message.Message):
    __slots__ = ("json",)
    JSON_FIELD_NUMBER: _ClassVar[int]
    json: str
    def __init__(self, json: _Optional[str] = ...) -> None: ...
