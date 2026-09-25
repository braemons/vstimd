from vstimd_client._proto.vstimd.v1 import vec2_pb2 as _vec2_pb2
from vstimd_client._proto.vstimd.v1.stimuli import identity_pb2 as _identity_pb2
from vstimd_client._proto.vstimd.v1.stimuli import shapes_pb2 as _shapes_pb2
from vstimd_client._proto.vstimd.v1 import transform_pb2 as _transform_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CreatePolygonRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform2D
    params: PolygonParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform2D, _Mapping]] = ..., params: _Optional[_Union[PolygonParams, _Mapping]] = ...) -> None: ...

class SetPolygonVerticesRequest(_message.Message):
    __slots__ = ("vertices_px",)
    VERTICES_PX_FIELD_NUMBER: _ClassVar[int]
    vertices_px: _containers.RepeatedCompositeFieldContainer[_vec2_pb2.Vec2]
    def __init__(self, vertices_px: _Optional[_Iterable[_Union[_vec2_pb2.Vec2, _Mapping]]] = ...) -> None: ...

class PolygonParams(_message.Message):
    __slots__ = ("vertices_px", "close_shape", "appearance")
    VERTICES_PX_FIELD_NUMBER: _ClassVar[int]
    CLOSE_SHAPE_FIELD_NUMBER: _ClassVar[int]
    APPEARANCE_FIELD_NUMBER: _ClassVar[int]
    vertices_px: _containers.RepeatedCompositeFieldContainer[_vec2_pb2.Vec2]
    close_shape: bool
    appearance: _shapes_pb2.ShapeAppearance
    def __init__(self, vertices_px: _Optional[_Iterable[_Union[_vec2_pb2.Vec2, _Mapping]]] = ..., close_shape: bool = ..., appearance: _Optional[_Union[_shapes_pb2.ShapeAppearance, _Mapping]] = ...) -> None: ...
