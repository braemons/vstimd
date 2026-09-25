from vstimd_client._proto.vstimd.v1.stimuli import circle_pb2 as _circle_pb2
from vstimd_client._proto.vstimd.v1.stimuli import dots_pb2 as _dots_pb2
from vstimd_client._proto.vstimd.v1.stimuli import ellipse_pb2 as _ellipse_pb2
from vstimd_client._proto.vstimd.v1.stimuli import gaussian_splat_pb2 as _gaussian_splat_pb2
from vstimd_client._proto.vstimd.v1.stimuli import grating_pb2 as _grating_pb2
from vstimd_client._proto.vstimd.v1.stimuli import shapes3d_pb2 as _shapes3d_pb2
from vstimd_client._proto.vstimd.v1.stimuli import polygon_pb2 as _polygon_pb2
from vstimd_client._proto.vstimd.v1.stimuli import rect_pb2 as _rect_pb2
from vstimd_client._proto.vstimd.v1.stimuli import stimulus_type_pb2 as _stimulus_type_pb2
from vstimd_client._proto.vstimd.v1.stimuli import text_pb2 as _text_pb2
from vstimd_client._proto.vstimd.v1 import transform_pb2 as _transform_pb2
from google.protobuf.internal import containers as _containers
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Iterable as _Iterable, Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class QueryStimulusRequest(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class StimulusParams(_message.Message):
    __slots__ = ("rect", "circle", "ellipse", "grating", "text", "polygon", "dots", "cube_3d", "sphere_3d", "plane_3d", "corridor_3d", "gaussian_splat_3d")
    RECT_FIELD_NUMBER: _ClassVar[int]
    CIRCLE_FIELD_NUMBER: _ClassVar[int]
    ELLIPSE_FIELD_NUMBER: _ClassVar[int]
    GRATING_FIELD_NUMBER: _ClassVar[int]
    TEXT_FIELD_NUMBER: _ClassVar[int]
    POLYGON_FIELD_NUMBER: _ClassVar[int]
    DOTS_FIELD_NUMBER: _ClassVar[int]
    CUBE_3D_FIELD_NUMBER: _ClassVar[int]
    SPHERE_3D_FIELD_NUMBER: _ClassVar[int]
    PLANE_3D_FIELD_NUMBER: _ClassVar[int]
    CORRIDOR_3D_FIELD_NUMBER: _ClassVar[int]
    GAUSSIAN_SPLAT_3D_FIELD_NUMBER: _ClassVar[int]
    rect: _rect_pb2.RectParams
    circle: _circle_pb2.CircleParams
    ellipse: _ellipse_pb2.EllipseParams
    grating: _grating_pb2.GratingParams
    text: _text_pb2.TextParams
    polygon: _polygon_pb2.PolygonParams
    dots: _dots_pb2.DotsParams
    cube_3d: _shapes3d_pb2.Cube3DParams
    sphere_3d: _shapes3d_pb2.Sphere3DParams
    plane_3d: _shapes3d_pb2.Plane3DParams
    corridor_3d: _shapes3d_pb2.Corridor3DParams
    gaussian_splat_3d: _gaussian_splat_pb2.GaussianSplat3DParams
    def __init__(self, rect: _Optional[_Union[_rect_pb2.RectParams, _Mapping]] = ..., circle: _Optional[_Union[_circle_pb2.CircleParams, _Mapping]] = ..., ellipse: _Optional[_Union[_ellipse_pb2.EllipseParams, _Mapping]] = ..., grating: _Optional[_Union[_grating_pb2.GratingParams, _Mapping]] = ..., text: _Optional[_Union[_text_pb2.TextParams, _Mapping]] = ..., polygon: _Optional[_Union[_polygon_pb2.PolygonParams, _Mapping]] = ..., dots: _Optional[_Union[_dots_pb2.DotsParams, _Mapping]] = ..., cube_3d: _Optional[_Union[_shapes3d_pb2.Cube3DParams, _Mapping]] = ..., sphere_3d: _Optional[_Union[_shapes3d_pb2.Sphere3DParams, _Mapping]] = ..., plane_3d: _Optional[_Union[_shapes3d_pb2.Plane3DParams, _Mapping]] = ..., corridor_3d: _Optional[_Union[_shapes3d_pb2.Corridor3DParams, _Mapping]] = ..., gaussian_splat_3d: _Optional[_Union[_gaussian_splat_pb2.GaussianSplat3DParams, _Mapping]] = ...) -> None: ...

class QueryStimulusResponse(_message.Message):
    __slots__ = ("stimulus_type", "enabled", "opacity", "params", "id", "name", "anim_enabled", "draw_order", "handle", "transform_2d", "transform_3d", "condition_indices", "condition_enabled")
    STIMULUS_TYPE_FIELD_NUMBER: _ClassVar[int]
    ENABLED_FIELD_NUMBER: _ClassVar[int]
    OPACITY_FIELD_NUMBER: _ClassVar[int]
    PARAMS_FIELD_NUMBER: _ClassVar[int]
    ID_FIELD_NUMBER: _ClassVar[int]
    NAME_FIELD_NUMBER: _ClassVar[int]
    ANIM_ENABLED_FIELD_NUMBER: _ClassVar[int]
    DRAW_ORDER_FIELD_NUMBER: _ClassVar[int]
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    TRANSFORM_2D_FIELD_NUMBER: _ClassVar[int]
    TRANSFORM_3D_FIELD_NUMBER: _ClassVar[int]
    CONDITION_INDICES_FIELD_NUMBER: _ClassVar[int]
    CONDITION_ENABLED_FIELD_NUMBER: _ClassVar[int]
    stimulus_type: _stimulus_type_pb2.StimulusType
    enabled: bool
    opacity: float
    params: StimulusParams
    id: str
    name: str
    anim_enabled: bool
    draw_order: int
    handle: int
    transform_2d: _transform_pb2.Transform2D
    transform_3d: _transform_pb2.Transform3D
    condition_indices: _containers.RepeatedScalarFieldContainer[int]
    condition_enabled: bool
    def __init__(self, stimulus_type: _Optional[_Union[_stimulus_type_pb2.StimulusType, str]] = ..., enabled: bool = ..., opacity: _Optional[float] = ..., params: _Optional[_Union[StimulusParams, _Mapping]] = ..., id: _Optional[str] = ..., name: _Optional[str] = ..., anim_enabled: bool = ..., draw_order: _Optional[int] = ..., handle: _Optional[int] = ..., transform_2d: _Optional[_Union[_transform_pb2.Transform2D, _Mapping]] = ..., transform_3d: _Optional[_Union[_transform_pb2.Transform3D, _Mapping]] = ..., condition_indices: _Optional[_Iterable[int]] = ..., condition_enabled: bool = ...) -> None: ...
