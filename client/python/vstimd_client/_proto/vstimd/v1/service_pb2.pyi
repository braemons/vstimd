from vstimd_client._proto.vstimd.v1 import animations_pb2 as _animations_pb2
from vstimd_client._proto.vstimd.v1 import conditions_pb2 as _conditions_pb2
from vstimd_client._proto.vstimd.v1 import input_pb2 as _input_pb2
from vstimd_client._proto.vstimd.v1.stimuli import circle_pb2 as _circle_pb2
from vstimd_client._proto.vstimd.v1.stimuli import dots_pb2 as _dots_pb2
from vstimd_client._proto.vstimd.v1.stimuli import ellipse_pb2 as _ellipse_pb2
from vstimd_client._proto.vstimd.v1.stimuli import gaussian_splat_pb2 as _gaussian_splat_pb2
from vstimd_client._proto.vstimd.v1.stimuli import grating_pb2 as _grating_pb2
from vstimd_client._proto.vstimd.v1.stimuli import shapes3d_pb2 as _shapes3d_pb2
from vstimd_client._proto.vstimd.v1.stimuli import polygon_pb2 as _polygon_pb2
from vstimd_client._proto.vstimd.v1.stimuli import query_pb2 as _query_pb2
from vstimd_client._proto.vstimd.v1.stimuli import rect_pb2 as _rect_pb2
from vstimd_client._proto.vstimd.v1.stimuli import shapes_pb2 as _shapes_pb2
from vstimd_client._proto.vstimd.v1.stimuli import shared_set_requests_pb2 as _shared_set_requests_pb2
from vstimd_client._proto.vstimd.v1.stimuli import text_pb2 as _text_pb2
from vstimd_client._proto.vstimd.v1 import scene3d_pb2 as _scene3d_pb2
from vstimd_client._proto.vstimd.v1 import system_pb2 as _system_pb2
from vstimd_client._proto.vstimd.v1 import vtl_pb2 as _vtl_pb2
from google.protobuf.internal import enum_type_wrapper as _enum_type_wrapper
from google.protobuf import descriptor as _descriptor
from google.protobuf import message as _message
from collections.abc import Mapping as _Mapping
from typing import ClassVar as _ClassVar, Optional as _Optional, Union as _Union

DESCRIPTOR: _descriptor.FileDescriptor

class ErrorCode(int, metaclass=_enum_type_wrapper.EnumTypeWrapper):
    __slots__ = ()
    ERROR_CODE_UNSPECIFIED: _ClassVar[ErrorCode]
    ERROR_CODE_OK: _ClassVar[ErrorCode]
    ERROR_CODE_UNKNOWN: _ClassVar[ErrorCode]
    ERROR_CODE_HANDLE_NOT_FOUND: _ClassVar[ErrorCode]
    ERROR_CODE_WRONG_STIMULUS_TYPE: _ClassVar[ErrorCode]
    ERROR_CODE_WRONG_TARGET: _ClassVar[ErrorCode]
    ERROR_CODE_CREATION_FAILED: _ClassVar[ErrorCode]
    ERROR_CODE_INVALID_ARGUMENT: _ClassVar[ErrorCode]
    ERROR_CODE_NOT_SUPPORTED: _ClassVar[ErrorCode]
    ERROR_CODE_NOT_READY: _ClassVar[ErrorCode]
    ERROR_CODE_FILE_NOT_FOUND: _ClassVar[ErrorCode]
    ERROR_CODE_FILE_IO: _ClassVar[ErrorCode]
    ERROR_CODE_FILE_FORMAT: _ClassVar[ErrorCode]
    ERROR_CODE_UNSUPPORTED_VERSION: _ClassVar[ErrorCode]
    ERROR_CODE_FILE_ALREADY_EXISTS: _ClassVar[ErrorCode]
ERROR_CODE_UNSPECIFIED: ErrorCode
ERROR_CODE_OK: ErrorCode
ERROR_CODE_UNKNOWN: ErrorCode
ERROR_CODE_HANDLE_NOT_FOUND: ErrorCode
ERROR_CODE_WRONG_STIMULUS_TYPE: ErrorCode
ERROR_CODE_WRONG_TARGET: ErrorCode
ERROR_CODE_CREATION_FAILED: ErrorCode
ERROR_CODE_INVALID_ARGUMENT: ErrorCode
ERROR_CODE_NOT_SUPPORTED: ErrorCode
ERROR_CODE_NOT_READY: ErrorCode
ERROR_CODE_FILE_NOT_FOUND: ErrorCode
ERROR_CODE_FILE_IO: ErrorCode
ERROR_CODE_FILE_FORMAT: ErrorCode
ERROR_CODE_UNSUPPORTED_VERSION: ErrorCode
ERROR_CODE_FILE_ALREADY_EXISTS: ErrorCode

class SystemTarget(_message.Message):
    __slots__ = ()
    def __init__(self) -> None: ...

class Request(_message.Message):
    __slots__ = ("system", "stimulus", "create_rect", "create_circle", "create_ellipse", "create_grating", "create_text", "create_polygon", "create_dots", "create_cube_3d", "create_sphere_3d", "create_plane_3d", "create_corridor_3d", "create_gaussian_splat_3d", "set_enabled", "set_name", "delete", "set_position", "set_rotation", "set_fill_color", "set_alpha", "set_rect_size", "set_circle_diameter", "set_ellipse_size", "set_draw_mode", "set_outline_color", "set_outline_width", "bring_to_front", "send_to_back", "set_grating_phase", "set_grating_sf", "set_grating_contrast", "set_grating_waveform", "set_grating_mask", "set_grating_drift_speed", "set_grating_drift_decoupled", "set_grating_drift_angle", "set_grating_fore_color", "set_grating_back_color", "set_text", "set_text_color", "set_polygon_vertices", "set_transform_3d", "set_material_3d", "set_cube_3d_size", "set_sphere_3d_diameter", "set_plane_3d_size", "set_dots_direction", "set_dots_speed", "set_dots_coherence", "set_dots_count", "set_dots_size", "set_dots_color", "set_dots_aperture", "set_dots_field_size", "set_dots_lifetime", "set_dots_seed", "swap_draw_order", "set_background", "set_deferred_mode", "clear_stimuli", "clear_animations", "clear_all", "set_all_enabled", "set_camera", "set_lighting", "query_server_info", "query_stimulus", "list_stimuli", "query_camera", "query_lighting", "list_input_devices", "set_camera_zones", "list_camera_zones", "wait_for_frames", "wait_until", "capture_frame", "query_frame_stats", "reset_frame_stats", "set_virtual_trigger_line_name", "list_virtual_trigger_lines", "set_virtual_trigger_line", "toggle_virtual_trigger_line", "clear_virtual_trigger_line_latches", "set_virtual_trigger_line_bank", "create_animation", "arm_animation", "disarm_animation", "delete_animation", "list_animations", "query_animation", "cancel_animation", "set_nav_speed", "list_scene_configs", "load_scene_config", "upload_scene_config", "retrieve_scene_config", "set_condition", "declare_conditions", "list_conditions", "set_stimulus_conditions", "set_animation_conditions", "shutdown")
    SYSTEM_FIELD_NUMBER: _ClassVar[int]
    STIMULUS_FIELD_NUMBER: _ClassVar[int]
    CREATE_RECT_FIELD_NUMBER: _ClassVar[int]
    CREATE_CIRCLE_FIELD_NUMBER: _ClassVar[int]
    CREATE_ELLIPSE_FIELD_NUMBER: _ClassVar[int]
    CREATE_GRATING_FIELD_NUMBER: _ClassVar[int]
    CREATE_TEXT_FIELD_NUMBER: _ClassVar[int]
    CREATE_POLYGON_FIELD_NUMBER: _ClassVar[int]
    CREATE_DOTS_FIELD_NUMBER: _ClassVar[int]
    CREATE_CUBE_3D_FIELD_NUMBER: _ClassVar[int]
    CREATE_SPHERE_3D_FIELD_NUMBER: _ClassVar[int]
    CREATE_PLANE_3D_FIELD_NUMBER: _ClassVar[int]
    CREATE_CORRIDOR_3D_FIELD_NUMBER: _ClassVar[int]
    CREATE_GAUSSIAN_SPLAT_3D_FIELD_NUMBER: _ClassVar[int]
    SET_ENABLED_FIELD_NUMBER: _ClassVar[int]
    SET_NAME_FIELD_NUMBER: _ClassVar[int]
    DELETE_FIELD_NUMBER: _ClassVar[int]
    SET_POSITION_FIELD_NUMBER: _ClassVar[int]
    SET_ROTATION_FIELD_NUMBER: _ClassVar[int]
    SET_FILL_COLOR_FIELD_NUMBER: _ClassVar[int]
    SET_ALPHA_FIELD_NUMBER: _ClassVar[int]
    SET_RECT_SIZE_FIELD_NUMBER: _ClassVar[int]
    SET_CIRCLE_DIAMETER_FIELD_NUMBER: _ClassVar[int]
    SET_ELLIPSE_SIZE_FIELD_NUMBER: _ClassVar[int]
    SET_DRAW_MODE_FIELD_NUMBER: _ClassVar[int]
    SET_OUTLINE_COLOR_FIELD_NUMBER: _ClassVar[int]
    SET_OUTLINE_WIDTH_FIELD_NUMBER: _ClassVar[int]
    BRING_TO_FRONT_FIELD_NUMBER: _ClassVar[int]
    SEND_TO_BACK_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_PHASE_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_SF_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_CONTRAST_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_WAVEFORM_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_MASK_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_DRIFT_SPEED_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_DRIFT_DECOUPLED_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_DRIFT_ANGLE_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_FORE_COLOR_FIELD_NUMBER: _ClassVar[int]
    SET_GRATING_BACK_COLOR_FIELD_NUMBER: _ClassVar[int]
    SET_TEXT_FIELD_NUMBER: _ClassVar[int]
    SET_TEXT_COLOR_FIELD_NUMBER: _ClassVar[int]
    SET_POLYGON_VERTICES_FIELD_NUMBER: _ClassVar[int]
    SET_TRANSFORM_3D_FIELD_NUMBER: _ClassVar[int]
    SET_MATERIAL_3D_FIELD_NUMBER: _ClassVar[int]
    SET_CUBE_3D_SIZE_FIELD_NUMBER: _ClassVar[int]
    SET_SPHERE_3D_DIAMETER_FIELD_NUMBER: _ClassVar[int]
    SET_PLANE_3D_SIZE_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_DIRECTION_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_SPEED_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_COHERENCE_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_COUNT_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_SIZE_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_COLOR_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_APERTURE_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_FIELD_SIZE_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_LIFETIME_FIELD_NUMBER: _ClassVar[int]
    SET_DOTS_SEED_FIELD_NUMBER: _ClassVar[int]
    SWAP_DRAW_ORDER_FIELD_NUMBER: _ClassVar[int]
    SET_BACKGROUND_FIELD_NUMBER: _ClassVar[int]
    SET_DEFERRED_MODE_FIELD_NUMBER: _ClassVar[int]
    CLEAR_STIMULI_FIELD_NUMBER: _ClassVar[int]
    CLEAR_ANIMATIONS_FIELD_NUMBER: _ClassVar[int]
    CLEAR_ALL_FIELD_NUMBER: _ClassVar[int]
    SET_ALL_ENABLED_FIELD_NUMBER: _ClassVar[int]
    SET_CAMERA_FIELD_NUMBER: _ClassVar[int]
    SET_LIGHTING_FIELD_NUMBER: _ClassVar[int]
    QUERY_SERVER_INFO_FIELD_NUMBER: _ClassVar[int]
    QUERY_STIMULUS_FIELD_NUMBER: _ClassVar[int]
    LIST_STIMULI_FIELD_NUMBER: _ClassVar[int]
    QUERY_CAMERA_FIELD_NUMBER: _ClassVar[int]
    QUERY_LIGHTING_FIELD_NUMBER: _ClassVar[int]
    LIST_INPUT_DEVICES_FIELD_NUMBER: _ClassVar[int]
    SET_CAMERA_ZONES_FIELD_NUMBER: _ClassVar[int]
    LIST_CAMERA_ZONES_FIELD_NUMBER: _ClassVar[int]
    WAIT_FOR_FRAMES_FIELD_NUMBER: _ClassVar[int]
    WAIT_UNTIL_FIELD_NUMBER: _ClassVar[int]
    CAPTURE_FRAME_FIELD_NUMBER: _ClassVar[int]
    QUERY_FRAME_STATS_FIELD_NUMBER: _ClassVar[int]
    RESET_FRAME_STATS_FIELD_NUMBER: _ClassVar[int]
    SET_VIRTUAL_TRIGGER_LINE_NAME_FIELD_NUMBER: _ClassVar[int]
    LIST_VIRTUAL_TRIGGER_LINES_FIELD_NUMBER: _ClassVar[int]
    SET_VIRTUAL_TRIGGER_LINE_FIELD_NUMBER: _ClassVar[int]
    TOGGLE_VIRTUAL_TRIGGER_LINE_FIELD_NUMBER: _ClassVar[int]
    CLEAR_VIRTUAL_TRIGGER_LINE_LATCHES_FIELD_NUMBER: _ClassVar[int]
    SET_VIRTUAL_TRIGGER_LINE_BANK_FIELD_NUMBER: _ClassVar[int]
    CREATE_ANIMATION_FIELD_NUMBER: _ClassVar[int]
    ARM_ANIMATION_FIELD_NUMBER: _ClassVar[int]
    DISARM_ANIMATION_FIELD_NUMBER: _ClassVar[int]
    DELETE_ANIMATION_FIELD_NUMBER: _ClassVar[int]
    LIST_ANIMATIONS_FIELD_NUMBER: _ClassVar[int]
    QUERY_ANIMATION_FIELD_NUMBER: _ClassVar[int]
    CANCEL_ANIMATION_FIELD_NUMBER: _ClassVar[int]
    SET_NAV_SPEED_FIELD_NUMBER: _ClassVar[int]
    LIST_SCENE_CONFIGS_FIELD_NUMBER: _ClassVar[int]
    LOAD_SCENE_CONFIG_FIELD_NUMBER: _ClassVar[int]
    UPLOAD_SCENE_CONFIG_FIELD_NUMBER: _ClassVar[int]
    RETRIEVE_SCENE_CONFIG_FIELD_NUMBER: _ClassVar[int]
    SET_CONDITION_FIELD_NUMBER: _ClassVar[int]
    DECLARE_CONDITIONS_FIELD_NUMBER: _ClassVar[int]
    LIST_CONDITIONS_FIELD_NUMBER: _ClassVar[int]
    SET_STIMULUS_CONDITIONS_FIELD_NUMBER: _ClassVar[int]
    SET_ANIMATION_CONDITIONS_FIELD_NUMBER: _ClassVar[int]
    SHUTDOWN_FIELD_NUMBER: _ClassVar[int]
    system: SystemTarget
    stimulus: int
    create_rect: _rect_pb2.CreateRectRequest
    create_circle: _circle_pb2.CreateCircleRequest
    create_ellipse: _ellipse_pb2.CreateEllipseRequest
    create_grating: _grating_pb2.CreateGratingRequest
    create_text: _text_pb2.CreateTextRequest
    create_polygon: _polygon_pb2.CreatePolygonRequest
    create_dots: _dots_pb2.CreateDotsRequest
    create_cube_3d: _shapes3d_pb2.CreateCube3DRequest
    create_sphere_3d: _shapes3d_pb2.CreateSphere3DRequest
    create_plane_3d: _shapes3d_pb2.CreatePlane3DRequest
    create_corridor_3d: _shapes3d_pb2.CreateCorridor3DRequest
    create_gaussian_splat_3d: _gaussian_splat_pb2.CreateGaussianSplat3DRequest
    set_enabled: _shared_set_requests_pb2.SetEnabledRequest
    set_name: _shared_set_requests_pb2.SetNameRequest
    delete: _shared_set_requests_pb2.DeleteRequest
    set_position: _shared_set_requests_pb2.SetPositionRequest
    set_rotation: _shared_set_requests_pb2.SetRotationRequest
    set_fill_color: _shared_set_requests_pb2.SetFillColorRequest
    set_alpha: _shared_set_requests_pb2.SetAlphaRequest
    set_rect_size: _rect_pb2.SetRectSizeRequest
    set_circle_diameter: _circle_pb2.SetCircleDiameterRequest
    set_ellipse_size: _ellipse_pb2.SetEllipseSizeRequest
    set_draw_mode: _shapes_pb2.SetDrawModeRequest
    set_outline_color: _shapes_pb2.SetOutlineColorRequest
    set_outline_width: _shapes_pb2.SetOutlineWidthRequest
    bring_to_front: _shared_set_requests_pb2.BringToFrontRequest
    send_to_back: _shared_set_requests_pb2.SendToBackRequest
    set_grating_phase: _grating_pb2.SetGratingPhaseRequest
    set_grating_sf: _grating_pb2.SetGratingSfRequest
    set_grating_contrast: _grating_pb2.SetGratingContrastRequest
    set_grating_waveform: _grating_pb2.SetGratingWaveformRequest
    set_grating_mask: _grating_pb2.SetGratingMaskRequest
    set_grating_drift_speed: _grating_pb2.SetGratingDriftSpeedRequest
    set_grating_drift_decoupled: _grating_pb2.SetGratingDriftDecoupledRequest
    set_grating_drift_angle: _grating_pb2.SetGratingDriftAngleRequest
    set_grating_fore_color: _grating_pb2.SetGratingForeColorRequest
    set_grating_back_color: _grating_pb2.SetGratingBackColorRequest
    set_text: _text_pb2.SetTextRequest
    set_text_color: _text_pb2.SetTextColorRequest
    set_polygon_vertices: _polygon_pb2.SetPolygonVerticesRequest
    set_transform_3d: _shapes3d_pb2.SetTransform3DRequest
    set_material_3d: _shapes3d_pb2.SetMaterial3DRequest
    set_cube_3d_size: _shapes3d_pb2.SetCube3DSizeRequest
    set_sphere_3d_diameter: _shapes3d_pb2.SetSphere3DDiameterRequest
    set_plane_3d_size: _shapes3d_pb2.SetPlane3DSizeRequest
    set_dots_direction: _dots_pb2.SetDotsDirectionRequest
    set_dots_speed: _dots_pb2.SetDotsSpeedRequest
    set_dots_coherence: _dots_pb2.SetDotsCoherenceRequest
    set_dots_count: _dots_pb2.SetDotsCountRequest
    set_dots_size: _dots_pb2.SetDotsSizeRequest
    set_dots_color: _dots_pb2.SetDotsColorRequest
    set_dots_aperture: _dots_pb2.SetDotsApertureRequest
    set_dots_field_size: _dots_pb2.SetDotsFieldSizeRequest
    set_dots_lifetime: _dots_pb2.SetDotsLifetimeRequest
    set_dots_seed: _dots_pb2.SetDotsSeedRequest
    swap_draw_order: _shared_set_requests_pb2.SwapDrawOrderRequest
    set_background: _system_pb2.SetBackgroundRequest
    set_deferred_mode: _system_pb2.SetDeferredModeRequest
    clear_stimuli: _system_pb2.ClearStimuliRequest
    clear_animations: _system_pb2.ClearAnimationsRequest
    clear_all: _system_pb2.ClearAllRequest
    set_all_enabled: _system_pb2.SetAllEnabledRequest
    set_camera: _scene3d_pb2.SetCameraRequest
    set_lighting: _scene3d_pb2.SetLightingRequest
    query_server_info: _system_pb2.QueryServerInfoRequest
    query_stimulus: _query_pb2.QueryStimulusRequest
    list_stimuli: _system_pb2.ListStimuliRequest
    query_camera: _scene3d_pb2.QueryCameraRequest
    query_lighting: _scene3d_pb2.QueryLightingRequest
    list_input_devices: _input_pb2.ListInputDevicesRequest
    set_camera_zones: _scene3d_pb2.SetCameraZonesRequest
    list_camera_zones: _scene3d_pb2.ListCameraZonesRequest
    wait_for_frames: _system_pb2.WaitForFramesRequest
    wait_until: _system_pb2.WaitUntilRequest
    capture_frame: _system_pb2.CaptureFrameRequest
    query_frame_stats: _system_pb2.QueryFrameStatsRequest
    reset_frame_stats: _system_pb2.ResetFrameStatsRequest
    set_virtual_trigger_line_name: _vtl_pb2.SetVirtualTriggerLineNameRequest
    list_virtual_trigger_lines: _vtl_pb2.ListVirtualTriggerLinesRequest
    set_virtual_trigger_line: _vtl_pb2.SetVirtualTriggerLineRequest
    toggle_virtual_trigger_line: _vtl_pb2.ToggleVirtualTriggerLineRequest
    clear_virtual_trigger_line_latches: _vtl_pb2.ClearVirtualTriggerLineLatchesRequest
    set_virtual_trigger_line_bank: _vtl_pb2.SetVirtualTriggerLineBankRequest
    create_animation: _animations_pb2.CreateAnimationRequest
    arm_animation: _animations_pb2.ArmAnimationRequest
    disarm_animation: _animations_pb2.DisarmAnimationRequest
    delete_animation: _animations_pb2.DeleteAnimationRequest
    list_animations: _animations_pb2.ListAnimationsRequest
    query_animation: _animations_pb2.QueryAnimationRequest
    cancel_animation: _animations_pb2.CancelAnimationRequest
    set_nav_speed: _animations_pb2.SetNavSpeedRequest
    list_scene_configs: _system_pb2.ListSceneConfigsRequest
    load_scene_config: _system_pb2.LoadSceneConfigRequest
    upload_scene_config: _system_pb2.UploadSceneConfigRequest
    retrieve_scene_config: _system_pb2.RetrieveSceneConfigRequest
    set_condition: _conditions_pb2.SetConditionRequest
    declare_conditions: _conditions_pb2.DeclareConditionsRequest
    list_conditions: _conditions_pb2.ListConditionsRequest
    set_stimulus_conditions: _conditions_pb2.SetStimulusConditionsRequest
    set_animation_conditions: _conditions_pb2.SetAnimationConditionsRequest
    shutdown: _system_pb2.ShutdownRequest
    def __init__(self, system: _Optional[_Union[SystemTarget, _Mapping]] = ..., stimulus: _Optional[int] = ..., create_rect: _Optional[_Union[_rect_pb2.CreateRectRequest, _Mapping]] = ..., create_circle: _Optional[_Union[_circle_pb2.CreateCircleRequest, _Mapping]] = ..., create_ellipse: _Optional[_Union[_ellipse_pb2.CreateEllipseRequest, _Mapping]] = ..., create_grating: _Optional[_Union[_grating_pb2.CreateGratingRequest, _Mapping]] = ..., create_text: _Optional[_Union[_text_pb2.CreateTextRequest, _Mapping]] = ..., create_polygon: _Optional[_Union[_polygon_pb2.CreatePolygonRequest, _Mapping]] = ..., create_dots: _Optional[_Union[_dots_pb2.CreateDotsRequest, _Mapping]] = ..., create_cube_3d: _Optional[_Union[_shapes3d_pb2.CreateCube3DRequest, _Mapping]] = ..., create_sphere_3d: _Optional[_Union[_shapes3d_pb2.CreateSphere3DRequest, _Mapping]] = ..., create_plane_3d: _Optional[_Union[_shapes3d_pb2.CreatePlane3DRequest, _Mapping]] = ..., create_corridor_3d: _Optional[_Union[_shapes3d_pb2.CreateCorridor3DRequest, _Mapping]] = ..., create_gaussian_splat_3d: _Optional[_Union[_gaussian_splat_pb2.CreateGaussianSplat3DRequest, _Mapping]] = ..., set_enabled: _Optional[_Union[_shared_set_requests_pb2.SetEnabledRequest, _Mapping]] = ..., set_name: _Optional[_Union[_shared_set_requests_pb2.SetNameRequest, _Mapping]] = ..., delete: _Optional[_Union[_shared_set_requests_pb2.DeleteRequest, _Mapping]] = ..., set_position: _Optional[_Union[_shared_set_requests_pb2.SetPositionRequest, _Mapping]] = ..., set_rotation: _Optional[_Union[_shared_set_requests_pb2.SetRotationRequest, _Mapping]] = ..., set_fill_color: _Optional[_Union[_shared_set_requests_pb2.SetFillColorRequest, _Mapping]] = ..., set_alpha: _Optional[_Union[_shared_set_requests_pb2.SetAlphaRequest, _Mapping]] = ..., set_rect_size: _Optional[_Union[_rect_pb2.SetRectSizeRequest, _Mapping]] = ..., set_circle_diameter: _Optional[_Union[_circle_pb2.SetCircleDiameterRequest, _Mapping]] = ..., set_ellipse_size: _Optional[_Union[_ellipse_pb2.SetEllipseSizeRequest, _Mapping]] = ..., set_draw_mode: _Optional[_Union[_shapes_pb2.SetDrawModeRequest, _Mapping]] = ..., set_outline_color: _Optional[_Union[_shapes_pb2.SetOutlineColorRequest, _Mapping]] = ..., set_outline_width: _Optional[_Union[_shapes_pb2.SetOutlineWidthRequest, _Mapping]] = ..., bring_to_front: _Optional[_Union[_shared_set_requests_pb2.BringToFrontRequest, _Mapping]] = ..., send_to_back: _Optional[_Union[_shared_set_requests_pb2.SendToBackRequest, _Mapping]] = ..., set_grating_phase: _Optional[_Union[_grating_pb2.SetGratingPhaseRequest, _Mapping]] = ..., set_grating_sf: _Optional[_Union[_grating_pb2.SetGratingSfRequest, _Mapping]] = ..., set_grating_contrast: _Optional[_Union[_grating_pb2.SetGratingContrastRequest, _Mapping]] = ..., set_grating_waveform: _Optional[_Union[_grating_pb2.SetGratingWaveformRequest, _Mapping]] = ..., set_grating_mask: _Optional[_Union[_grating_pb2.SetGratingMaskRequest, _Mapping]] = ..., set_grating_drift_speed: _Optional[_Union[_grating_pb2.SetGratingDriftSpeedRequest, _Mapping]] = ..., set_grating_drift_decoupled: _Optional[_Union[_grating_pb2.SetGratingDriftDecoupledRequest, _Mapping]] = ..., set_grating_drift_angle: _Optional[_Union[_grating_pb2.SetGratingDriftAngleRequest, _Mapping]] = ..., set_grating_fore_color: _Optional[_Union[_grating_pb2.SetGratingForeColorRequest, _Mapping]] = ..., set_grating_back_color: _Optional[_Union[_grating_pb2.SetGratingBackColorRequest, _Mapping]] = ..., set_text: _Optional[_Union[_text_pb2.SetTextRequest, _Mapping]] = ..., set_text_color: _Optional[_Union[_text_pb2.SetTextColorRequest, _Mapping]] = ..., set_polygon_vertices: _Optional[_Union[_polygon_pb2.SetPolygonVerticesRequest, _Mapping]] = ..., set_transform_3d: _Optional[_Union[_shapes3d_pb2.SetTransform3DRequest, _Mapping]] = ..., set_material_3d: _Optional[_Union[_shapes3d_pb2.SetMaterial3DRequest, _Mapping]] = ..., set_cube_3d_size: _Optional[_Union[_shapes3d_pb2.SetCube3DSizeRequest, _Mapping]] = ..., set_sphere_3d_diameter: _Optional[_Union[_shapes3d_pb2.SetSphere3DDiameterRequest, _Mapping]] = ..., set_plane_3d_size: _Optional[_Union[_shapes3d_pb2.SetPlane3DSizeRequest, _Mapping]] = ..., set_dots_direction: _Optional[_Union[_dots_pb2.SetDotsDirectionRequest, _Mapping]] = ..., set_dots_speed: _Optional[_Union[_dots_pb2.SetDotsSpeedRequest, _Mapping]] = ..., set_dots_coherence: _Optional[_Union[_dots_pb2.SetDotsCoherenceRequest, _Mapping]] = ..., set_dots_count: _Optional[_Union[_dots_pb2.SetDotsCountRequest, _Mapping]] = ..., set_dots_size: _Optional[_Union[_dots_pb2.SetDotsSizeRequest, _Mapping]] = ..., set_dots_color: _Optional[_Union[_dots_pb2.SetDotsColorRequest, _Mapping]] = ..., set_dots_aperture: _Optional[_Union[_dots_pb2.SetDotsApertureRequest, _Mapping]] = ..., set_dots_field_size: _Optional[_Union[_dots_pb2.SetDotsFieldSizeRequest, _Mapping]] = ..., set_dots_lifetime: _Optional[_Union[_dots_pb2.SetDotsLifetimeRequest, _Mapping]] = ..., set_dots_seed: _Optional[_Union[_dots_pb2.SetDotsSeedRequest, _Mapping]] = ..., swap_draw_order: _Optional[_Union[_shared_set_requests_pb2.SwapDrawOrderRequest, _Mapping]] = ..., set_background: _Optional[_Union[_system_pb2.SetBackgroundRequest, _Mapping]] = ..., set_deferred_mode: _Optional[_Union[_system_pb2.SetDeferredModeRequest, _Mapping]] = ..., clear_stimuli: _Optional[_Union[_system_pb2.ClearStimuliRequest, _Mapping]] = ..., clear_animations: _Optional[_Union[_system_pb2.ClearAnimationsRequest, _Mapping]] = ..., clear_all: _Optional[_Union[_system_pb2.ClearAllRequest, _Mapping]] = ..., set_all_enabled: _Optional[_Union[_system_pb2.SetAllEnabledRequest, _Mapping]] = ..., set_camera: _Optional[_Union[_scene3d_pb2.SetCameraRequest, _Mapping]] = ..., set_lighting: _Optional[_Union[_scene3d_pb2.SetLightingRequest, _Mapping]] = ..., query_server_info: _Optional[_Union[_system_pb2.QueryServerInfoRequest, _Mapping]] = ..., query_stimulus: _Optional[_Union[_query_pb2.QueryStimulusRequest, _Mapping]] = ..., list_stimuli: _Optional[_Union[_system_pb2.ListStimuliRequest, _Mapping]] = ..., query_camera: _Optional[_Union[_scene3d_pb2.QueryCameraRequest, _Mapping]] = ..., query_lighting: _Optional[_Union[_scene3d_pb2.QueryLightingRequest, _Mapping]] = ..., list_input_devices: _Optional[_Union[_input_pb2.ListInputDevicesRequest, _Mapping]] = ..., set_camera_zones: _Optional[_Union[_scene3d_pb2.SetCameraZonesRequest, _Mapping]] = ..., list_camera_zones: _Optional[_Union[_scene3d_pb2.ListCameraZonesRequest, _Mapping]] = ..., wait_for_frames: _Optional[_Union[_system_pb2.WaitForFramesRequest, _Mapping]] = ..., wait_until: _Optional[_Union[_system_pb2.WaitUntilRequest, _Mapping]] = ..., capture_frame: _Optional[_Union[_system_pb2.CaptureFrameRequest, _Mapping]] = ..., query_frame_stats: _Optional[_Union[_system_pb2.QueryFrameStatsRequest, _Mapping]] = ..., reset_frame_stats: _Optional[_Union[_system_pb2.ResetFrameStatsRequest, _Mapping]] = ..., set_virtual_trigger_line_name: _Optional[_Union[_vtl_pb2.SetVirtualTriggerLineNameRequest, _Mapping]] = ..., list_virtual_trigger_lines: _Optional[_Union[_vtl_pb2.ListVirtualTriggerLinesRequest, _Mapping]] = ..., set_virtual_trigger_line: _Optional[_Union[_vtl_pb2.SetVirtualTriggerLineRequest, _Mapping]] = ..., toggle_virtual_trigger_line: _Optional[_Union[_vtl_pb2.ToggleVirtualTriggerLineRequest, _Mapping]] = ..., clear_virtual_trigger_line_latches: _Optional[_Union[_vtl_pb2.ClearVirtualTriggerLineLatchesRequest, _Mapping]] = ..., set_virtual_trigger_line_bank: _Optional[_Union[_vtl_pb2.SetVirtualTriggerLineBankRequest, _Mapping]] = ..., create_animation: _Optional[_Union[_animations_pb2.CreateAnimationRequest, _Mapping]] = ..., arm_animation: _Optional[_Union[_animations_pb2.ArmAnimationRequest, _Mapping]] = ..., disarm_animation: _Optional[_Union[_animations_pb2.DisarmAnimationRequest, _Mapping]] = ..., delete_animation: _Optional[_Union[_animations_pb2.DeleteAnimationRequest, _Mapping]] = ..., list_animations: _Optional[_Union[_animations_pb2.ListAnimationsRequest, _Mapping]] = ..., query_animation: _Optional[_Union[_animations_pb2.QueryAnimationRequest, _Mapping]] = ..., cancel_animation: _Optional[_Union[_animations_pb2.CancelAnimationRequest, _Mapping]] = ..., set_nav_speed: _Optional[_Union[_animations_pb2.SetNavSpeedRequest, _Mapping]] = ..., list_scene_configs: _Optional[_Union[_system_pb2.ListSceneConfigsRequest, _Mapping]] = ..., load_scene_config: _Optional[_Union[_system_pb2.LoadSceneConfigRequest, _Mapping]] = ..., upload_scene_config: _Optional[_Union[_system_pb2.UploadSceneConfigRequest, _Mapping]] = ..., retrieve_scene_config: _Optional[_Union[_system_pb2.RetrieveSceneConfigRequest, _Mapping]] = ..., set_condition: _Optional[_Union[_conditions_pb2.SetConditionRequest, _Mapping]] = ..., declare_conditions: _Optional[_Union[_conditions_pb2.DeclareConditionsRequest, _Mapping]] = ..., list_conditions: _Optional[_Union[_conditions_pb2.ListConditionsRequest, _Mapping]] = ..., set_stimulus_conditions: _Optional[_Union[_conditions_pb2.SetStimulusConditionsRequest, _Mapping]] = ..., set_animation_conditions: _Optional[_Union[_conditions_pb2.SetAnimationConditionsRequest, _Mapping]] = ..., shutdown: _Optional[_Union[_system_pb2.ShutdownRequest, _Mapping]] = ...) -> None: ...

class Response(_message.Message):
    __slots__ = ("handle", "code", "error", "id", "server_info", "stimulus_info", "stimulus_list", "virtual_trigger_line_list", "virtual_trigger_line_state", "animation_list", "query_animation_response", "scene_config_list", "retrieved_scene_config", "deferred_mode", "condition_list", "captured_frame", "camera", "lighting", "input_device_list", "camera_zone_list", "frame_stats", "server_time_ns", "frame_count")
    HANDLE_FIELD_NUMBER: _ClassVar[int]
    CODE_FIELD_NUMBER: _ClassVar[int]
    ERROR_FIELD_NUMBER: _ClassVar[int]
    ID_FIELD_NUMBER: _ClassVar[int]
    SERVER_INFO_FIELD_NUMBER: _ClassVar[int]
    STIMULUS_INFO_FIELD_NUMBER: _ClassVar[int]
    STIMULUS_LIST_FIELD_NUMBER: _ClassVar[int]
    VIRTUAL_TRIGGER_LINE_LIST_FIELD_NUMBER: _ClassVar[int]
    VIRTUAL_TRIGGER_LINE_STATE_FIELD_NUMBER: _ClassVar[int]
    ANIMATION_LIST_FIELD_NUMBER: _ClassVar[int]
    QUERY_ANIMATION_RESPONSE_FIELD_NUMBER: _ClassVar[int]
    SCENE_CONFIG_LIST_FIELD_NUMBER: _ClassVar[int]
    RETRIEVED_SCENE_CONFIG_FIELD_NUMBER: _ClassVar[int]
    DEFERRED_MODE_FIELD_NUMBER: _ClassVar[int]
    CONDITION_LIST_FIELD_NUMBER: _ClassVar[int]
    CAPTURED_FRAME_FIELD_NUMBER: _ClassVar[int]
    CAMERA_FIELD_NUMBER: _ClassVar[int]
    LIGHTING_FIELD_NUMBER: _ClassVar[int]
    INPUT_DEVICE_LIST_FIELD_NUMBER: _ClassVar[int]
    CAMERA_ZONE_LIST_FIELD_NUMBER: _ClassVar[int]
    FRAME_STATS_FIELD_NUMBER: _ClassVar[int]
    SERVER_TIME_NS_FIELD_NUMBER: _ClassVar[int]
    FRAME_COUNT_FIELD_NUMBER: _ClassVar[int]
    handle: int
    code: ErrorCode
    error: str
    id: str
    server_info: _system_pb2.QueryServerInfoResponse
    stimulus_info: _query_pb2.QueryStimulusResponse
    stimulus_list: _system_pb2.ListStimuliResponse
    virtual_trigger_line_list: _vtl_pb2.ListVirtualTriggerLinesResponse
    virtual_trigger_line_state: _vtl_pb2.VirtualTriggerLineStateResponse
    animation_list: _animations_pb2.ListAnimationsResponse
    query_animation_response: _animations_pb2.QueryAnimationResponse
    scene_config_list: _system_pb2.ListSceneConfigsResponse
    retrieved_scene_config: _system_pb2.RetrieveSceneConfigResponse
    deferred_mode: _system_pb2.SetDeferredModeResponse
    condition_list: _conditions_pb2.ListConditionsResponse
    captured_frame: _system_pb2.CaptureFrameResponse
    camera: _scene3d_pb2.Camera3D
    lighting: _scene3d_pb2.Lighting3D
    input_device_list: _input_pb2.ListInputDevicesResponse
    camera_zone_list: _scene3d_pb2.ListCameraZonesResponse
    frame_stats: _system_pb2.FrameStats
    server_time_ns: int
    frame_count: int
    def __init__(self, handle: _Optional[int] = ..., code: _Optional[_Union[ErrorCode, str]] = ..., error: _Optional[str] = ..., id: _Optional[str] = ..., server_info: _Optional[_Union[_system_pb2.QueryServerInfoResponse, _Mapping]] = ..., stimulus_info: _Optional[_Union[_query_pb2.QueryStimulusResponse, _Mapping]] = ..., stimulus_list: _Optional[_Union[_system_pb2.ListStimuliResponse, _Mapping]] = ..., virtual_trigger_line_list: _Optional[_Union[_vtl_pb2.ListVirtualTriggerLinesResponse, _Mapping]] = ..., virtual_trigger_line_state: _Optional[_Union[_vtl_pb2.VirtualTriggerLineStateResponse, _Mapping]] = ..., animation_list: _Optional[_Union[_animations_pb2.ListAnimationsResponse, _Mapping]] = ..., query_animation_response: _Optional[_Union[_animations_pb2.QueryAnimationResponse, _Mapping]] = ..., scene_config_list: _Optional[_Union[_system_pb2.ListSceneConfigsResponse, _Mapping]] = ..., retrieved_scene_config: _Optional[_Union[_system_pb2.RetrieveSceneConfigResponse, _Mapping]] = ..., deferred_mode: _Optional[_Union[_system_pb2.SetDeferredModeResponse, _Mapping]] = ..., condition_list: _Optional[_Union[_conditions_pb2.ListConditionsResponse, _Mapping]] = ..., captured_frame: _Optional[_Union[_system_pb2.CaptureFrameResponse, _Mapping]] = ..., camera: _Optional[_Union[_scene3d_pb2.Camera3D, _Mapping]] = ..., lighting: _Optional[_Union[_scene3d_pb2.Lighting3D, _Mapping]] = ..., input_device_list: _Optional[_Union[_input_pb2.ListInputDevicesResponse, _Mapping]] = ..., camera_zone_list: _Optional[_Union[_scene3d_pb2.ListCameraZonesResponse, _Mapping]] = ..., frame_stats: _Optional[_Union[_system_pb2.FrameStats, _Mapping]] = ..., server_time_ns: _Optional[int] = ..., frame_count: _Optional[int] = ...) -> None: ...
