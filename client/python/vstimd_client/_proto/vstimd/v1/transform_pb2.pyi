from vstimd_client._proto.vstimd.v1 import vec2_pb2 as _vec2_pb2
from vstimd_client._proto.vstimd.v1 import vec3_pb2 as _vec3_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Transform2D(_message.Message):
    __slots__ = ("pos_px", "rotation_deg")
    POS_PX_FIELD_NUMBER: _ClassVar[int]
    ROTATION_DEG_FIELD_NUMBER: _ClassVar[int]
    pos_px: _vec2_pb2.Vec2
    rotation_deg: float
    def __init__(self, pos_px: _Optional[_Union[_vec2_pb2.Vec2, _Mapping]] = ..., rotation_deg: _Optional[float] = ...) -> None: ...

class Transform3D(_message.Message):
    __slots__ = ("position_cm", "rotation_deg", "scale")
    POSITION_CM_FIELD_NUMBER: _ClassVar[int]
    ROTATION_DEG_FIELD_NUMBER: _ClassVar[int]
    SCALE_FIELD_NUMBER: _ClassVar[int]
    position_cm: _vec3_pb2.Vec3
    rotation_deg: _vec3_pb2.Vec3
    scale: _vec3_pb2.Vec3
    def __init__(self, position_cm: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ..., rotation_deg: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ..., scale: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ...) -> None: ...
