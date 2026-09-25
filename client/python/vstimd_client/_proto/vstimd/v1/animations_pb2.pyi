from vstimd_client._proto.vstimd.v1 import conditions_pb2 as _conditions_pb2
from vstimd_client._proto.vstimd.v1 import vtl_pb2 as _vtl_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class AnimationState(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    ANIMATION_STATE_IDLE: _ClassVar[AnimationState]
    ANIMATION_STATE_ARMED: _ClassVar[AnimationState]
    ANIMATION_STATE_RUNNING: _ClassVar[AnimationState]
    ANIMATION_STATE_DONE: _ClassVar[AnimationState]

class TransformChannel(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    TRANSFORM_CHANNEL_UNSPECIFIED: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_POS_X: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_POS_Y: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_POS_Z: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_YAW: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_PITCH: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_ROLL: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_SCALE_X: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_SCALE_Y: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_SCALE_Z: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_SCALE_UNIFORM: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_FORWARD: _ClassVar[TransformChannel]
    TRANSFORM_CHANNEL_STRAFE: _ClassVar[TransformChannel]
ANIMATION_STATE_IDLE: AnimationState
ANIMATION_STATE_ARMED: AnimationState
ANIMATION_STATE_RUNNING: AnimationState
ANIMATION_STATE_DONE: AnimationState
TRANSFORM_CHANNEL_UNSPECIFIED: TransformChannel
TRANSFORM_CHANNEL_POS_X: TransformChannel
TRANSFORM_CHANNEL_POS_Y: TransformChannel
TRANSFORM_CHANNEL_POS_Z: TransformChannel
TRANSFORM_CHANNEL_YAW: TransformChannel
TRANSFORM_CHANNEL_PITCH: TransformChannel
TRANSFORM_CHANNEL_ROLL: TransformChannel
TRANSFORM_CHANNEL_SCALE_X: TransformChannel
TRANSFORM_CHANNEL_SCALE_Y: TransformChannel
TRANSFORM_CHANNEL_SCALE_Z: TransformChannel
TRANSFORM_CHANNEL_SCALE_UNIFORM: TransformChannel
TRANSFORM_CHANNEL_FORWARD: TransformChannel
TRANSFORM_CHANNEL_STRAFE: TransformChannel

class CoupleVisibilityToTriggerLine(_message.Message):
    __slots__ = ("trigger", "polarity")
    TRIGGER_FIELD_NUMBER: _ClassVar[int]
    POLARITY_FIELD_NUMBER: _ClassVar[int]
    trigger: _vtl_pb2.VirtualTriggerLineHandle
    polarity: _vtl_pb2.VtlPolarity
    def __init__(self, trigger: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., polarity: _Optional[_Union[_vtl_pb2.VtlPolarity, str]] = ...) -> None: ...

class EnableOnTriggerEdge(_message.Message):
    __slots__ = ("trigger", "edge", "enabled")
    TRIGGER_FIELD_NUMBER: _ClassVar[int]
    EDGE_FIELD_NUMBER: _ClassVar[int]
    ENABLED_FIELD_NUMBER: _ClassVar[int]
    trigger: _vtl_pb2.VirtualTriggerLineHandle
    edge: _vtl_pb2.VtlEdge
    enabled: bool
    def __init__(self, trigger: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., edge: _Optional[_Union[_vtl_pb2.VtlEdge, str]] = ..., enabled: bool = ...) -> None: ...

class FlashForNFrames(_message.Message):
    __slots__ = ("duration_frames",)
    DURATION_FRAMES_FIELD_NUMBER: _ClassVar[int]
    duration_frames: int
    def __init__(self, duration_frames: _Optional[int] = ...) -> None: ...

class FlickerForNFrames(_message.Message):
    __slots__ = ("on_frames", "off_frames", "total_frames", "start_on_phase")
    ON_FRAMES_FIELD_NUMBER: _ClassVar[int]
    OFF_FRAMES_FIELD_NUMBER: _ClassVar[int]
    TOTAL_FRAMES_FIELD_NUMBER: _ClassVar[int]
    START_ON_PHASE_FIELD_NUMBER: _ClassVar[int]
    on_frames: int
    off_frames: int
    total_frames: int
    start_on_phase: bool
    def __init__(self, on_frames: _Optional[int] = ..., off_frames: _Optional[int] = ..., total_frames: _Optional[int] = ..., start_on_phase: bool = ...) -> None: ...

class ExternalPosition2D(_message.Message):
    __slots__ = ("shm_name", "x_offset_px", "y_offset_px")
    SHM_NAME_FIELD_NUMBER: _ClassVar[int]
    X_OFFSET_PX_FIELD_NUMBER: _ClassVar[int]
    Y_OFFSET_PX_FIELD_NUMBER: _ClassVar[int]
    shm_name: str
    x_offset_px: float
    y_offset_px: float
    def __init__(self, shm_name: _Optional[str] = ..., x_offset_px: _Optional[float] = ..., y_offset_px: _Optional[float] = ...) -> None: ...

class MoveAlongPath2D(_message.Message):
    __slots__ = ("x_px", "y_px")
    X_PX_FIELD_NUMBER: _ClassVar[int]
    Y_PX_FIELD_NUMBER: _ClassVar[int]
    x_px: _containers.RepeatedScalarFieldContainer[float]
    y_px: _containers.RepeatedScalarFieldContainer[float]
    def __init__(self, x_px: _Optional[_Iterable[float]] = ..., y_px: _Optional[_Iterable[float]] = ...) -> None: ...

class MoveAlongSegments2D(_message.Message):
    __slots__ = ("x_px", "y_px", "speed_px_per_sec")
    X_PX_FIELD_NUMBER: _ClassVar[int]
    Y_PX_FIELD_NUMBER: _ClassVar[int]
    SPEED_PX_PER_SEC_FIELD_NUMBER: _ClassVar[int]
    x_px: _containers.RepeatedScalarFieldContainer[float]
    y_px: _containers.RepeatedScalarFieldContainer[float]
    speed_px_per_sec: float
    def __init__(self, x_px: _Optional[_Iterable[float]] = ..., y_px: _Optional[_Iterable[float]] = ..., speed_px_per_sec: _Optional[float] = ...) -> None: ...

class LinearNav3D(_message.Message):
    __slots__ = ("speed_cm_per_s", "wrap_period_cm", "source", "track_length_cm", "fade_frames")
    SPEED_CM_PER_S_FIELD_NUMBER: _ClassVar[int]
    WRAP_PERIOD_CM_FIELD_NUMBER: _ClassVar[int]
    SOURCE_FIELD_NUMBER: _ClassVar[int]
    TRACK_LENGTH_CM_FIELD_NUMBER: _ClassVar[int]
    FADE_FRAMES_FIELD_NUMBER: _ClassVar[int]
    speed_cm_per_s: float
    wrap_period_cm: float
    source: AxisRef
    track_length_cm: float
    fade_frames: int
    def __init__(self, speed_cm_per_s: _Optional[float] = ..., wrap_period_cm: _Optional[float] = ..., source: _Optional[_Union[AxisRef, _Mapping]] = ..., track_length_cm: _Optional[float] = ..., fade_frames: _Optional[int] = ...) -> None: ...

class AxisRef(_message.Message):
    __slots__ = ("device", "axis")
    DEVICE_FIELD_NUMBER: _ClassVar[int]
    AXIS_FIELD_NUMBER: _ClassVar[int]
    device: str
    axis: str
    def __init__(self, device: _Optional[str] = ..., axis: _Optional[str] = ...) -> None: ...

class AxisMap(_message.Message):
    __slots__ = ("axis", "channel", "gain", "deadzone", "invert", "clamp_min", "clamp_max", "wrap")
    AXIS_FIELD_NUMBER: _ClassVar[int]
    CHANNEL_FIELD_NUMBER: _ClassVar[int]
    GAIN_FIELD_NUMBER: _ClassVar[int]
    DEADZONE_FIELD_NUMBER: _ClassVar[int]
    INVERT_FIELD_NUMBER: _ClassVar[int]
    CLAMP_MIN_FIELD_NUMBER: _ClassVar[int]
    CLAMP_MAX_FIELD_NUMBER: _ClassVar[int]
    WRAP_FIELD_NUMBER: _ClassVar[int]
    axis: str
    channel: TransformChannel
    gain: float
    deadzone: float
    invert: bool
    clamp_min: float
    clamp_max: float
    wrap: float
    def __init__(self, axis: _Optional[str] = ..., channel: _Optional[_Union[TransformChannel, str]] = ..., gain: _Optional[float] = ..., deadzone: _Optional[float] = ..., invert: bool = ..., clamp_min: _Optional[float] = ..., clamp_max: _Optional[float] = ..., wrap: _Optional[float] = ...) -> None: ...

class DeviceDrivenTransform(_message.Message):
    __slots__ = ("device", "axes")
    DEVICE_FIELD_NUMBER: _ClassVar[int]
    AXES_FIELD_NUMBER: _ClassVar[int]
    device: str
    axes: _containers.RepeatedCompositeFieldContainer[AxisMap]
    def __init__(self, device: _Optional[str] = ..., axes: _Optional[_Iterable[_Union[AxisMap, _Mapping]]] = ...) -> None: ...

class AnimationStimuli(_message.Message):
    __slots__ = ("handles",)
    HANDLES_FIELD_NUMBER: _ClassVar[int]
    handles: _containers.RepeatedScalarFieldContainer[int]
    def __init__(self, handles: _Optional[_Iterable[int]] = ...) -> None: ...

class AnimationCamera(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class AnimationTarget(_message.Message):
    __slots__ = ("stimuli", "camera")
    STIMULI_FIELD_NUMBER: _ClassVar[int]
    CAMERA_FIELD_NUMBER: _ClassVar[int]
    stimuli: AnimationStimuli
    camera: AnimationCamera
    def __init__(self, stimuli: _Optional[_Union[AnimationStimuli, _Mapping]] = ..., camera: _Optional[_Union[AnimationCamera, _Mapping]] = ...) -> None: ...

class CreateAnimationRequest(_message.Message):
    __slots__ = ("name", "final_action_mask", "final_action_trigger_line", "final_action_level_line", "start_trigger", "start_edge", "cancel_trigger", "cancel_edge", "cancel_action_mask", "cancel_action_trigger_line", "target", "start_action_mask", "start_action_trigger_line", "couple_visibility_to_trigger_line", "enable_on_trigger_edge", "flash_for_n_frames", "flicker_for_n_frames", "move_along_path_2d", "move_along_segments_2d", "external_position_2d", "linear_nav_3d", "device_driven_transform")
    NAME_FIELD_NUMBER: _ClassVar[int]
    FINAL_ACTION_MASK_FIELD_NUMBER: _ClassVar[int]
    FINAL_ACTION_TRIGGER_LINE_FIELD_NUMBER: _ClassVar[int]
    FINAL_ACTION_LEVEL_LINE_FIELD_NUMBER: _ClassVar[int]
    START_TRIGGER_FIELD_NUMBER: _ClassVar[int]
    START_EDGE_FIELD_NUMBER: _ClassVar[int]
    CANCEL_TRIGGER_FIELD_NUMBER: _ClassVar[int]
    CANCEL_EDGE_FIELD_NUMBER: _ClassVar[int]
    CANCEL_ACTION_MASK_FIELD_NUMBER: _ClassVar[int]
    CANCEL_ACTION_TRIGGER_LINE_FIELD_NUMBER: _ClassVar[int]
    TARGET_FIELD_NUMBER: _ClassVar[int]
    START_ACTION_MASK_FIELD_NUMBER: _ClassVar[int]
    START_ACTION_TRIGGER_LINE_FIELD_NUMBER: _ClassVar[int]
    COUPLE_VISIBILITY_TO_TRIGGER_LINE_FIELD_NUMBER: _ClassVar[int]
    ENABLE_ON_TRIGGER_EDGE_FIELD_NUMBER: _ClassVar[int]
    FLASH_FOR_N_FRAMES_FIELD_NUMBER: _ClassVar[int]
    FLICKER_FOR_N_FRAMES_FIELD_NUMBER: _ClassVar[int]
    MOVE_ALONG_PATH_2D_FIELD_NUMBER: _ClassVar[int]
    MOVE_ALONG_SEGMENTS_2D_FIELD_NUMBER: _ClassVar[int]
    EXTERNAL_POSITION_2D_FIELD_NUMBER: _ClassVar[int]
    LINEAR_NAV_3D_FIELD_NUMBER: _ClassVar[int]
    DEVICE_DRIVEN_TRANSFORM_FIELD_NUMBER: _ClassVar[int]
    name: str
    final_action_mask: int
    final_action_trigger_line: _vtl_pb2.VirtualTriggerLineHandle
    final_action_level_line: _vtl_pb2.VirtualTriggerLineHandle
    start_trigger: _vtl_pb2.VirtualTriggerLineHandle
    start_edge: _vtl_pb2.VtlEdge
    cancel_trigger: _vtl_pb2.VirtualTriggerLineHandle
    cancel_edge: _vtl_pb2.VtlEdge
    cancel_action_mask: int
    cancel_action_trigger_line: _vtl_pb2.VirtualTriggerLineHandle
    target: AnimationTarget
    start_action_mask: int
    start_action_trigger_line: _vtl_pb2.VirtualTriggerLineHandle
    couple_visibility_to_trigger_line: CoupleVisibilityToTriggerLine
    enable_on_trigger_edge: EnableOnTriggerEdge
    flash_for_n_frames: FlashForNFrames
    flicker_for_n_frames: FlickerForNFrames
    move_along_path_2d: MoveAlongPath2D
    move_along_segments_2d: MoveAlongSegments2D
    external_position_2d: ExternalPosition2D
    linear_nav_3d: LinearNav3D
    device_driven_transform: DeviceDrivenTransform
    def __init__(self, name: _Optional[str] = ..., final_action_mask: _Optional[int] = ..., final_action_trigger_line: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., final_action_level_line: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., start_trigger: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., start_edge: _Optional[_Union[_vtl_pb2.VtlEdge, str]] = ..., cancel_trigger: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., cancel_edge: _Optional[_Union[_vtl_pb2.VtlEdge, str]] = ..., cancel_action_mask: _Optional[int] = ..., cancel_action_trigger_line: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., target: _Optional[_Union[AnimationTarget, _Mapping]] = ..., start_action_mask: _Optional[int] = ..., start_action_trigger_line: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., couple_visibility_to_trigger_line: _Optional[_Union[CoupleVisibilityToTriggerLine, _Mapping]] = ..., enable_on_trigger_edge: _Optional[_Union[EnableOnTriggerEdge, _Mapping]] = ..., flash_for_n_frames: _Optional[_Union[FlashForNFrames, _Mapping]] = ..., flicker_for_n_frames: _Optional[_Union[FlickerForNFrames, _Mapping]] = ..., move_along_path_2d: _Optional[_Union[MoveAlongPath2D, _Mapping]] = ..., move_along_segments_2d: _Optional[_Union[MoveAlongSegments2D, _Mapping]] = ..., external_position_2d: _Optional[_Union[ExternalPosition2D, _Mapping]] = ..., linear_nav_3d: _Optional[_Union[LinearNav3D, _Mapping]] = ..., device_driven_transform: _Optional[_Union[DeviceDrivenTransform, _Mapping]] = ...) -> None: ...

class ArmAnimationRequest(_message.Message):
    __slots__ = ("handle",)
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    handle: int
    def __init__(self, handle: _Optional[int] = ...) -> None: ...

class DisarmAnimationRequest(_message.Message):
    __slots__ = ("handle",)
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    handle: int
    def __init__(self, handle: _Optional[int] = ...) -> None: ...

class CancelAnimationRequest(_message.Message):
    __slots__ = ("handle",)
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    handle: int
    def __init__(self, handle: _Optional[int] = ...) -> None: ...

class DeleteAnimationRequest(_message.Message):
    __slots__ = ("handle",)
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    handle: int
    def __init__(self, handle: _Optional[int] = ...) -> None: ...

class QueryAnimationRequest(_message.Message):
    __slots__ = ("handle",)
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    handle: int
    def __init__(self, handle: _Optional[int] = ...) -> None: ...

class ListAnimationsRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class AnimationInfo(_message.Message):
    __slots__ = ("handle", "name", "state", "type_name", "condition_indices", "condition_enabled")
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    TYPE_NAME_FIELD_NUMBER: _ClassVar[int]
    CONDITION_INDICES_FIELD_NUMBER: _ClassVar[int]
    CONDITION_ENABLED_FIELD_NUMBER: _ClassVar[int]
    handle: int
    name: str
    state: AnimationState
    type_name: str
    condition_indices: _containers.RepeatedScalarFieldContainer[int]
    condition_enabled: bool
    def __init__(self, handle: _Optional[int] = ..., name: _Optional[str] = ..., state: _Optional[_Union[AnimationState, str]] = ..., type_name: _Optional[str] = ..., condition_indices: _Optional[_Iterable[int]] = ..., condition_enabled: bool = ...) -> None: ...

class QueryAnimationResponse(_message.Message):
    __slots__ = ("handle", "state", "params", "type_name", "condition_indices", "condition_action", "condition_enabled", "distance_travelled_cm", "device_backend", "device_stale")
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    STATE_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    TYPE_NAME_FIELD_NUMBER: _ClassVar[int]
    CONDITION_INDICES_FIELD_NUMBER: _ClassVar[int]
    CONDITION_ACTION_FIELD_NUMBER: _ClassVar[int]
    CONDITION_ENABLED_FIELD_NUMBER: _ClassVar[int]
    DISTANCE_TRAVELLED_CM_FIELD_NUMBER: _ClassVar[int]
    DEVICE_BACKEND_FIELD_NUMBER: _ClassVar[int]
    DEVICE_STALE_FIELD_NUMBER: _ClassVar[int]
    handle: int
    state: AnimationState
    params: CreateAnimationRequest
    type_name: str
    condition_indices: _containers.RepeatedScalarFieldContainer[int]
    condition_action: _conditions_pb2.ConditionAction
    condition_enabled: bool
    distance_travelled_cm: float
    device_backend: str
    device_stale: bool
    def __init__(self, handle: _Optional[int] = ..., state: _Optional[_Union[AnimationState, str]] = ..., params: _Optional[_Union[CreateAnimationRequest, _Mapping]] = ..., type_name: _Optional[str] = ..., condition_indices: _Optional[_Iterable[int]] = ..., condition_action: _Optional[_Union[_conditions_pb2.ConditionAction, str]] = ..., condition_enabled: bool = ..., distance_travelled_cm: _Optional[float] = ..., device_backend: _Optional[str] = ..., device_stale: bool = ...) -> None: ...

class SetNavSpeedRequest(_message.Message):
    __slots__ = ("handle", "speed_cm_per_s")
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    SPEED_CM_PER_S_FIELD_NUMBER: _ClassVar[int]
    handle: int
    speed_cm_per_s: float
    def __init__(self, handle: _Optional[int] = ..., speed_cm_per_s: _Optional[float] = ...) -> None: ...

class ListAnimationsResponse(_message.Message):
    __slots__ = ("animations",)
    ANIMATIONS_FIELD_NUMBER: _ClassVar[int]
    animations: _containers.RepeatedCompositeFieldContainer[AnimationInfo]
    def __init__(self, animations: _Optional[_Iterable[_Union[AnimationInfo, _Mapping]]] = ...) -> None: ...
