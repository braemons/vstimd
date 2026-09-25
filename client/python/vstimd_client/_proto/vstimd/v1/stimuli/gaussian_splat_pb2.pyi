from vstimd_client._proto.vstimd.v1.stimuli import identity_pb2 as _identity_pb2
from vstimd_client._proto.vstimd.v1 import transform_pb2 as _transform_pb2
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class GaussianSplat3DParams(_message.Message):
    __slots__ = ("path",)
    PATH_FIELD_NUMBER: _ClassVar[int]
    path: str
    def __init__(self, path: _Optional[str] = ...) -> None: ...

class CreateGaussianSplat3DRequest(_message.Message):
    __slots__ = ("identity", "placement", "params")
    IDENTITY_FIELD_NUMBER: _ClassVar[int]
    PLACEMENT_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    identity: _identity_pb2.StimulusIdentity
    placement: _transform_pb2.Transform3D
    params: GaussianSplat3DParams
    def __init__(self, identity: _Optional[_Union[_identity_pb2.StimulusIdentity, _Mapping]] = ..., placement: _Optional[_Union[_transform_pb2.Transform3D, _Mapping]] = ..., params: _Optional[_Union[GaussianSplat3DParams, _Mapping]] = ...) -> None: ...
