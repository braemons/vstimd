from vstimd_client._proto.vstimd.v1.stimuli import identity_pb2 as _identity_pb2
from vstimd_client._proto.vstimd.v1.stimuli import shapes_pb2 as _shapes_pb2
from vstimd_client._proto.vstimd.v1 import transform_pb2 as _transform_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class CreateEllipseRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform2D
    params: EllipseParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform2D, _Mapping]] = ..., params: _Optional[_Union[EllipseParams, _Mapping]] = ...) -> None: ...

class SetEllipseSizeRequest(_message.Message):
    __slots__ = ("width_px", "height_px")
    WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    width_px: float
    height_px: float
    def __init__(self, width_px: _Optional[float] = ..., height_px: _Optional[float] = ...) -> None: ...

class EllipseParams(_message.Message):
    __slots__ = ("width_px", "height_px", "appearance")
    WIDTH_PX_FIELD_NUMBER: _ClassVar[int]
    HEIGHT_PX_FIELD_NUMBER: _ClassVar[int]
    APPEARANCE_FIELD_NUMBER: _ClassVar[int]
    width_px: float
    height_px: float
    appearance: _shapes_pb2.ShapeAppearance
    def __init__(self, width_px: _Optional[float] = ..., height_px: _Optional[float] = ..., appearance: _Optional[_Union[_shapes_pb2.ShapeAppearance, _Mapping]] = ...) -> None: ...
