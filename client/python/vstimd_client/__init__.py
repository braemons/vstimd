"""vstimd_client — Python client for vstimd.

Talks to the server over ZMQ using protobuf encoding.

Example::

    from vstimd_client import VstimdClient

    with VstimdClient() as conn:
        h = conn.stimuli.shapes.create_rect(
            position_px=Vec2(-200, 0),
            params=RectParams(width_px=300, height_px=200,
                              appearance=ShapeAppearance(fill_color=Color(1.0, 0.0, 0.0))),
        )
        conn.stimuli.set_enabled(h, False)
        conn.stimuli.delete(h)
        info = conn.system.query_server_info()
        print(info.version)
"""

from ._version import __version__
from .vstimd_client import VstimdClient
from ._handles import AnimationHandle, StimulusHandle
from .response import ErrorCode, ServerResponse
from .system import ServerInfo, ServerVersion, StimulusListEntry
from .vtl import VtlClient, VtlHandle, VtlKind, VtlLineInfo
from .scene_config import SceneConfigClient
from .conditions import Condition, ConditionAction, ConditionStatus, ConditionsClient
from .animations import (
    AnimationClient,
    AnimationDetails,
    AnimationInfo,
    AnimationState,
    CancelAction,
    FinalAction,
    StartAction,
    VtlEdge,
    VtlPolarity,
)
from .exceptions import (
    VstimdError,
    ProtocolError,
    StimulusError,
    HandleNotFoundError,
    WrongStimulusTypeError,
    WrongTargetError,
    CreationFailedError,
    InvalidArgumentError,
    NotSupportedError,
    NotReadyError,
    UnknownServerError,
    SceneConfigError,
    SceneConfigNotFoundError,
    SceneConfigIoError,
    SceneConfigFormatError,
    SceneConfigVersionError,
    SceneConfigAlreadyExistsError,
)
from . import psychopy
from vstimd_client.stimuli import RectParams, ShapeAppearance, Vec2

__all__ = [
    "__version__",
    "VstimdClient",
    "AnimationHandle",
    "StimulusHandle",
    "ErrorCode",
    "ServerResponse",
    "ServerInfo",
    "ServerVersion",
    "StimulusListEntry",
    "SceneConfigClient",
    "ConditionsClient",
    "Condition",
    "ConditionAction",
    "ConditionStatus",
    "SceneConfigError",
    "SceneConfigNotFoundError",
    "SceneConfigIoError",
    "SceneConfigFormatError",
    "SceneConfigVersionError",
    "SceneConfigAlreadyExistsError",
    "VstimdError",
    "ProtocolError",
    "StimulusError",
    "HandleNotFoundError",
    "WrongStimulusTypeError",
    "WrongTargetError",
    "CreationFailedError",
    "InvalidArgumentError",
    "NotSupportedError",
    "NotReadyError",
    "UnknownServerError",
    "VtlClient",
    "VtlHandle",
    "VtlKind",
    "VtlLineInfo",
    "AnimationClient",
    "AnimationDetails",
    "AnimationInfo",
    "AnimationState",
    "CancelAction",
    "FinalAction",
    "StartAction",
    "VtlEdge",
    "VtlPolarity",
    "psychopy",
]
