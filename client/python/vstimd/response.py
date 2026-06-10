"""Server response envelope returned by every RPC."""
from __future__ import annotations

from dataclasses import dataclass

from vstimd._proto import service_pb2


@dataclass
class ServerResponse:
    """Envelope fields the server attaches to every response.

    handle         -- newly allocated stimulus handle on create; -1 on mutations/deletes
    code           -- ErrorCode integer (0 = unspecified, 1 = OK; see proto ErrorCode enum)
    error          -- human-readable error detail; empty string on success
    id             -- stable UUID of newly created stimulus; empty string otherwise
    frame_count    -- render frames completed at the time of the response
    server_time_ns -- nanoseconds since server start (monotonic, like
                      QueryPerformanceCounter / CLOCK_MONOTONIC)
    """

    handle: int = -1
    code: int = 0
    error: str = ""
    id: str = ""
    frame_count: int = 0
    server_time_ns: int = 0

    @classmethod
    def _from_proto(cls, resp: service_pb2.Response) -> "ServerResponse":
        return cls(
            handle=resp.handle,
            code=resp.code,
            error=resp.error,
            id=resp.id,
            frame_count=resp.frame_count,
            server_time_ns=resp.server_time_ns,
        )
