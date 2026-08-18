from __future__ import annotations

from dataclasses import dataclass, field

from vstimd._handles import StimulusHandle
from vstimd.stimuli.color import Color


@dataclass(order=True, repr=False)
class ServerVersion:
    """Semver triple reported by the server."""

    major: int
    minor: int
    patch: int

    def __repr__(self) -> str:
        return f"ServerVersion({self.major}, {self.minor}, {self.patch})"

    def __str__(self) -> str:
        return f"{self.major}.{self.minor}.{self.patch}"


@dataclass(repr=False)
class ServerInfo:
    """Display and version information returned by :meth:`SystemClient.query_server_info`."""

    width_px: int
    height_px: int
    frame_rate_hz: float
    version: ServerVersion
    background_color: Color = field(default_factory=lambda: Color(0.0, 0.0, 0.0))

    def __repr__(self) -> str:
        return (
            f"ServerInfo(width_px={self.width_px}, height_px={self.height_px}, "
            f"frame_rate_hz={self.frame_rate_hz:.1f}, version={self.version})"
        )


@dataclass
class StimulusListEntry:
    """One entry returned by :meth:`SystemClient.list_stimuli`."""

    handle: StimulusHandle
    enabled: bool
    id: str
    name: str
