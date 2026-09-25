from vstimd_client._proto.vstimd.v1 import color_pb2 as _color_pb2
from vstimd_client._proto.vstimd.v1.stimuli import identity_pb2 as _identity_pb2
from vstimd_client._proto.vstimd.v1 import transform_pb2 as _transform_pb2
from vstimd_client._proto.vstimd.v1 import vec2_pb2 as _vec2_pb2
from vstimd_client._proto.vstimd.v1 import vec3_pb2 as _vec3_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class Shading(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    SHADING_UNSPECIFIED: _ClassVar[Shading]
    SHADING_UNLIT: _ClassVar[Shading]
    SHADING_PHONG: _ClassVar[Shading]
SHADING_UNSPECIFIED: Shading
SHADING_UNLIT: Shading
SHADING_PHONG: Shading

class Material3D(_message.Message):
    __slots__ = ("albedo", "emissive", "shading")
    ALBEDO_FIELD_NUMBER: _ClassVar[int]
    EMISSIVE_FIELD_NUMBER: _ClassVar[int]
    SHADING_FIELD_NUMBER: _ClassVar[int]
    albedo: _color_pb2.Color
    emissive: _vec3_pb2.Vec3
    shading: Shading
    def __init__(self, albedo: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., emissive: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ..., shading: _Optional[_Union[Shading, str]] = ...) -> None: ...

class Repeat3D(_message.Message):
    __slots__ = ("period_cm", "ahead", "behind")
    PERIOD_CM_FIELD_NUMBER: _ClassVar[int]
    AHEAD_FIELD_NUMBER: _ClassVar[int]
    BEHIND_FIELD_NUMBER: _ClassVar[int]
    period_cm: float
    ahead: int
    behind: int
    def __init__(self, period_cm: _Optional[float] = ..., ahead: _Optional[int] = ..., behind: _Optional[int] = ...) -> None: ...

class Cube3DParams(_message.Message):
    __slots__ = ("size_cm", "material", "texture_path", "repeat")
    SIZE_CM_FIELD_NUMBER: _ClassVar[int]
    MATERIAL_FIELD_NUMBER: _ClassVar[int]
    TEXTURE_PATH_FIELD_NUMBER: _ClassVar[int]
    REPEAT_FIELD_NUMBER: _ClassVar[int]
    size_cm: _vec3_pb2.Vec3
    material: Material3D
    texture_path: str
    repeat: Repeat3D
    def __init__(self, size_cm: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ..., material: _Optional[_Union[Material3D, _Mapping]] = ..., texture_path: _Optional[str] = ..., repeat: _Optional[_Union[Repeat3D, _Mapping]] = ...) -> None: ...

class Sphere3DParams(_message.Message):
    __slots__ = ("diameter_cm", "rings", "sectors", "material", "texture_path", "repeat")
    DIAMETER_CM_FIELD_NUMBER: _ClassVar[int]
    RINGS_FIELD_NUMBER: _ClassVar[int]
    SECTORS_FIELD_NUMBER: _ClassVar[int]
    MATERIAL_FIELD_NUMBER: _ClassVar[int]
    TEXTURE_PATH_FIELD_NUMBER: _ClassVar[int]
    REPEAT_FIELD_NUMBER: _ClassVar[int]
    diameter_cm: float
    rings: int
    sectors: int
    material: Material3D
    texture_path: str
    repeat: Repeat3D
    def __init__(self, diameter_cm: _Optional[float] = ..., rings: _Optional[int] = ..., sectors: _Optional[int] = ..., material: _Optional[_Union[Material3D, _Mapping]] = ..., texture_path: _Optional[str] = ..., repeat: _Optional[_Union[Repeat3D, _Mapping]] = ...) -> None: ...

class Plane3DParams(_message.Message):
    __slots__ = ("size_cm", "material", "texture_path", "repeat")
    SIZE_CM_FIELD_NUMBER: _ClassVar[int]
    MATERIAL_FIELD_NUMBER: _ClassVar[int]
    TEXTURE_PATH_FIELD_NUMBER: _ClassVar[int]
    REPEAT_FIELD_NUMBER: _ClassVar[int]
    size_cm: _vec2_pb2.Vec2
    material: Material3D
    texture_path: str
    repeat: Repeat3D
    def __init__(self, size_cm: _Optional[_Union[_vec2_pb2.Vec2, _Mapping]] = ..., material: _Optional[_Union[Material3D, _Mapping]] = ..., texture_path: _Optional[str] = ..., repeat: _Optional[_Union[Repeat3D, _Mapping]] = ...) -> None: ...

class Corridor3DParams(_message.Message):
    __slots__ = ("width_cm", "height_cm", "period_cm", "periods_ahead", "periods_behind", "floor_color", "wall_color", "stripe_color", "ceiling", "material")
    WIDTH_CM_FIELD_NUMBER: _ClassVar[int]
    HEIGHT_CM_FIELD_NUMBER: _ClassVar[int]
    PERIOD_CM_FIELD_NUMBER: _ClassVar[int]
    PERIODS_AHEAD_FIELD_NUMBER: _ClassVar[int]
    PERIODS_BEHIND_FIELD_NUMBER: _ClassVar[int]
    FLOOR_COLOR_FIELD_NUMBER: _ClassVar[int]
    WALL_COLOR_FIELD_NUMBER: _ClassVar[int]
    STRIPE_COLOR_FIELD_NUMBER: _ClassVar[int]
    CEILING_FIELD_NUMBER: _ClassVar[int]
    MATERIAL_FIELD_NUMBER: _ClassVar[int]
    width_cm: float
    height_cm: float
    period_cm: float
    periods_ahead: int
    periods_behind: int
    floor_color: _color_pb2.Color
    wall_color: _color_pb2.Color
    stripe_color: _color_pb2.Color
    ceiling: bool
    material: Material3D
    def __init__(self, width_cm: _Optional[float] = ..., height_cm: _Optional[float] = ..., period_cm: _Optional[float] = ..., periods_ahead: _Optional[int] = ..., periods_behind: _Optional[int] = ..., floor_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., wall_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., stripe_color: _Optional[_Union[_color_pb2.Color, _Mapping]] = ..., ceiling: bool = ..., material: _Optional[_Union[Material3D, _Mapping]] = ...) -> None: ...

class CreateCube3DRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform3D
    params: Cube3DParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform3D, _Mapping]] = ..., params: _Optional[_Union[Cube3DParams, _Mapping]] = ...) -> None: ...

class CreateSphere3DRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform3D
    params: Sphere3DParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform3D, _Mapping]] = ..., params: _Optional[_Union[Sphere3DParams, _Mapping]] = ...) -> None: ...

class CreatePlane3DRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform3D
    params: Plane3DParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform3D, _Mapping]] = ..., params: _Optional[_Union[Plane3DParams, _Mapping]] = ...) -> None: ...

class CreateCorridor3DRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform3D
    params: Corridor3DParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform3D, _Mapping]] = ..., params: _Optional[_Union[Corridor3DParams, _Mapping]] = ...) -> None: ...

class SetTransform3DRequest(_message.Message):
    __slots__ = ("transform",)
    TRANSFORM_FIELD_NUMBER: _ClassVar[int]
    transform: _transform_pb2.Transform3D
    def __init__(self, transform: _Optional[_Union[_transform_pb2.Transform3D, _Mapping]] = ...) -> None: ...

class SetMaterial3DRequest(_message.Message):
    __slots__ = ("material",)
    MATERIAL_FIELD_NUMBER: _ClassVar[int]
    material: Material3D
    def __init__(self, material: _Optional[_Union[Material3D, _Mapping]] = ...) -> None: ...

class SetCube3DSizeRequest(_message.Message):
    __slots__ = ("size_cm",)
    SIZE_CM_FIELD_NUMBER: _ClassVar[int]
    size_cm: _vec3_pb2.Vec3
    def __init__(self, size_cm: _Optional[_Union[_vec3_pb2.Vec3, _Mapping]] = ...) -> None: ...

class SetSphere3DDiameterRequest(_message.Message):
    __slots__ = ("diameter_cm",)
    DIAMETER_CM_FIELD_NUMBER: _ClassVar[int]
    diameter_cm: float
    def __init__(self, diameter_cm: _Optional[float] = ...) -> None: ...

class SetPlane3DSizeRequest(_message.Message):
    __slots__ = ("size_cm",)
    SIZE_CM_FIELD_NUMBER: _ClassVar[int]
    size_cm: _vec2_pb2.Vec2
    def __init__(self, size_cm: _Optional[_Union[_vec2_pb2.Vec2, _Mapping]] = ...) -> None: ...
