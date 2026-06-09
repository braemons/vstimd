"""Shared type aliases for the vstimd.visual package."""

from __future__ import annotations

from typing import Protocol, runtime_checkable

# Any color value PsychoPy accepts
PsychoPyColor = str | tuple[float, ...] | list[float] | float | int | None

# A 2-D position or size as a sequence of two floats
PsychoPyVec2 = tuple[float, float] | list[float]


@runtime_checkable
class MonitorProtocol(Protocol):
    """Minimal interface of a psychopy.monitors.Monitor used for deg/cm conversion."""

    def deg2pix(self, deg: float) -> float: ...
    def cm2pix(self, cm: float) -> float: ...
