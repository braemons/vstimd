from vstimd_client._proto.vstimd.v1 import vec3_pb2 as _vec3_pb2
from vstimd_client._proto.vstimd.v1 import vtl_pb2 as _vtl_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Camera3D(_message.Message):
    __slots__ = ("position_cm", "yaw_deg", "pitch_deg", "roll_deg", "fov_y_deg", "near_cm", "far_cm")
    POSITION_CM_FIELD_NUMBER: _ClassVar[int]
    YAW_DEG_FIELD_NUMBER: _ClassVar[int]
    PITCH_DEG_FIELD_NUMBER: _ClassVar[int]
    ROLL_DEG_FIELD_NUMBER: _ClassVar[int]
    FOV_Y_DEG_FIELD_NUMBER: _ClassVar[int]
    NEAR_CM_FIELD_NUMBER: _ClassVar[int]
    FAR_CM_FIELD_NUMBER: _ClassVar[int]
    position_cm: _vec3_pb2.Vec3
    yaw_deg: float
    pitch_deg: float
    roll_deg: float
    fov_y_deg: float
    near_cm: float
    far_cm: float
    def __init__(self, position_cm: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ..., yaw_deg: _Optional[float] = ..., pitch_deg: _Optional[float] = ..., roll_deg: _Optional[float] = ..., fov_y_deg: _Optional[float] = ..., near_cm: _Optional[float] = ..., far_cm: _Optional[float] = ...) -> None: ...

class Lighting3D(_message.Message):
    __slots__ = ("ambient_color", "sun_direction", "sun_color")
    AMBIENT_COLOR_FIELD_NUMBER: _ClassVar[int]
    SUN_DIRECTION_FIELD_NUMBER: _ClassVar[int]
    SUN_COLOR_FIELD_NUMBER: _ClassVar[int]
    ambient_color: _vec3_pb2.Vec3
    sun_direction: _vec3_pb2.Vec3
    sun_color: _vec3_pb2.Vec3
    def __init__(self, ambient_color: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ..., sun_direction: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ..., sun_color: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ...) -> None: ...

class SetCameraRequest(_message.Message):
    __slots__ = ("camera",)
    CAMERA_FIELD_NUMBER: _ClassVar[int]
    camera: Camera3D
    def __init__(self, camera: _Optional[_Union[Camera3D, _Mapping]] = ...) -> None: ...

class QueryCameraRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class SetLightingRequest(_message.Message):
    __slots__ = ("lighting",)
    LIGHTING_FIELD_NUMBER: _ClassVar[int]
    lighting: Lighting3D
    def __init__(self, lighting: _Optional[_Union[Lighting3D, _Mapping]] = ...) -> None: ...

class QueryLightingRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class CameraZone(_message.Message):
    __slots__ = ("name", "line", "x_min_cm", "x_max_cm", "y_min_cm", "y_max_cm", "z_min_cm", "z_max_cm")
    NAME_FIELD_NUMBER: _ClassVar[int]
    LINE_FIELD_NUMBER: _ClassVar[int]
    X_MIN_CM_FIELD_NUMBER: _ClassVar[int]
    X_MAX_CM_FIELD_NUMBER: _ClassVar[int]
    Y_MIN_CM_FIELD_NUMBER: _ClassVar[int]
    Y_MAX_CM_FIELD_NUMBER: _ClassVar[int]
    Z_MIN_CM_FIELD_NUMBER: _ClassVar[int]
    Z_MAX_CM_FIELD_NUMBER: _ClassVar[int]
    name: str
    line: _vtl_pb2.VirtualTriggerLineHandle
    x_min_cm: float
    x_max_cm: float
    y_min_cm: float
    y_max_cm: float
    z_min_cm: float
    z_max_cm: float
    def __init__(self, name: _Optional[str] = ..., line: _Optional[_Union[_vtl_pb2.VirtualTriggerLineHandle, _Mapping]] = ..., x_min_cm: _Optional[float] = ..., x_max_cm: _Optional[float] = ..., y_min_cm: _Optional[float] = ..., y_max_cm: _Optional[float] = ..., z_min_cm: _Optional[float] = ..., z_max_cm: _Optional[float] = ...) -> None: ...

class SetCameraZonesRequest(_message.Message):
    __slots__ = ("zones",)
    ZONES_FIELD_NUMBER: _ClassVar[int]
    zones: _containers.RepeatedCompositeFieldContainer[CameraZone]
    def __init__(self, zones: _Optional[_Iterable[_Union[CameraZone, _Mapping]]] = ...) -> None: ...

class ListCameraZonesRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class CameraZoneInfo(_message.Message):
    __slots__ = ("zone", "inside")
    ZONE_FIELD_NUMBER: _ClassVar[int]
    INSIDE_FIELD_NUMBER: _ClassVar[int]
    zone: CameraZone
    inside: bool
    def __init__(self, zone: _Optional[_Union[CameraZone, _Mapping]] = ..., inside: bool = ...) -> None: ...

class ListCameraZonesResponse(_message.Message):
    __slots__ = ("zones",)
    ZONES_FIELD_NUMBER: _ClassVar[int]
    zones: _containers.RepeatedCompositeFieldContainer[CameraZoneInfo]
    def __init__(self, zones: _Optional[_Iterable[_Union[CameraZoneInfo, _Mapping]]] = ...) -> None: ...
