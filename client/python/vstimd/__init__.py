"""vstimd — Python client for vstimd.

Talks to the server over ZMQ using protobuf encoding.

Example::

    from vstimd import Connection

    with Connection() as conn:
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

# Extend the package search path so that `from vstimd.v1 import ...`
# in the generated proto stubs resolves to _proto/vstimd/v1/ without
# shadowing this package's own namespace.
import os as _os
__path__ = list(__path__) + [_os.path.join(_os.path.dirname(__file__), "_proto", "vstimd")]

from ._version import __version__
from .connection import Connection
from ._handles import AnimationHandle, StimulusHandle
from .response import ErrorCode, ServerResponse
from .system import ServerInfo, ServerVersion, StimulusListEntry
from .vtl import VtlClient, VtlHandle, VtlKind, VtlLineInfo
from .config import ConfigClient
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
    ConfigError,
    ConfigNotFoundError,
    ConfigIoError,
    ConfigFormatError,
    ConfigVersionError,
    ConfigAlreadyExistsError,
)
from . import psychopy
from vstimd.stimuli import RectParams, ShapeAppearance, Vec2

__all__ = [
    "__version__",
    "Connection",
    "AnimationHandle",
    "StimulusHandle",
    "ErrorCode",
    "ServerResponse",
    "ServerInfo",
    "ServerVersion",
    "StimulusListEntry",
    "ConfigClient",
    "ConfigError",
    "ConfigNotFoundError",
    "ConfigIoError",
    "ConfigFormatError",
    "ConfigVersionError",
    "ConfigAlreadyExistsError",
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
