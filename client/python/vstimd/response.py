"""Server response envelope returned by every RPC."""
from __future__ import annotations

from dataclasses import dataclass
from enum import IntEnum

from vstimd._proto import service_pb2


class ErrorCode(IntEnum):
    """Machine-readable result codes from the server (mirrors proto ErrorCode enum)."""
    UNSPECIFIED   = 0  # default zero value / malformed response
    OK            = 1  # success
    UNKNOWN       = 2  # unexpected server-side error
    HANDLE_NOT_FOUND     = 3
    WRONG_STIMULUS_TYPE  = 4
    WRONG_TARGET         = 5
    CREATION_FAILED      = 6
    INVALID_ARGUMENT     = 7
    NOT_SUPPORTED        = 8
    NOT_READY            = 9


@dataclass
class ServerResponse:
    """Envelope fields the server attaches to every response.

    handle         -- newly allocated stimulus handle on create; -1 on mutations/deletes
    code           -- ErrorCode (OK on success)
    error          -- human-readable error detail; empty string on success
    id             -- stable UUID of newly created stimulus; empty string otherwise
    frame_count    -- render frames completed at the time of the response
    server_time_ns -- nanoseconds since server start (monotonic, like
                      QueryPerformanceCounter / CLOCK_MONOTONIC)
    """

    handle: int = -1
    code: ErrorCode = ErrorCode.UNSPECIFIED
    error: str = ""
    id: str = ""
    frame_count: int = 0
    server_time_ns: int = 0

    @classmethod
    def _from_proto(cls, resp: service_pb2.Response) -> "ServerResponse":
        return cls(
            handle=resp.handle,
            code=ErrorCode(resp.code),
            error=resp.error,
            id=resp.id,
            frame_count=resp.frame_count,
            server_time_ns=resp.server_time_ns,
        )
