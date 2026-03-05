"""Command dataclasses and JSON serialisation.

Each command is a plain dataclass that serialises to a dict (→ JSON).
The server expects: {"cmd": "<name>", ...fields...}
"""

from __future__ import annotations

from dataclasses import dataclass, asdict, field
from typing import Any


# ---------------------------------------------------------------------------
# Base
# ---------------------------------------------------------------------------

@dataclass
class _BaseCmd:
    cmd: str

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


# ---------------------------------------------------------------------------
# Window-level commands
# ---------------------------------------------------------------------------

@dataclass
class SetWindowColorCmd(_BaseCmd):
    cmd: str = field(default="set_window_color", init=False)
    color: list[float]  # [r, g, b, a]


@dataclass
class DeferredFlipCmd(_BaseCmd):
    cmd: str = field(default="deferred_flip", init=False)


@dataclass
class CloseWindowCmd(_BaseCmd):
    cmd: str = field(default="close_window", init=False)


# ---------------------------------------------------------------------------
# Stimulus creation
# ---------------------------------------------------------------------------

@dataclass
class CreateCircleCmd(_BaseCmd):
    cmd: str = field(default="create_circle", init=False)
    handle: int
    radius: float
    pos: list[float]
    fill_color: list[float] | None
    line_color: list[float] | None
    line_width: float
    ori: float
    opacity: float
    enabled: bool


@dataclass
class CreateRectCmd(_BaseCmd):
    cmd: str = field(default="create_rect", init=False)
    handle: int
    width: float
    height: float
    pos: list[float]
    fill_color: list[float] | None
    line_color: list[float] | None
    line_width: float
    ori: float
    opacity: float
    enabled: bool


@dataclass
class CreatePolygonCmd(_BaseCmd):
    cmd: str = field(default="create_polygon", init=False)
    handle: int
    radius: float
    edges: int
    pos: list[float]
    fill_color: list[float] | None
    line_color: list[float] | None
    line_width: float
    ori: float
    opacity: float
    enabled: bool


@dataclass
class CreateLineCmd(_BaseCmd):
    cmd: str = field(default="create_line", init=False)
    handle: int
    start: list[float]
    end: list[float]
    line_color: list[float] | None
    line_width: float
    ori: float
    opacity: float
    enabled: bool


@dataclass
class CreateShapeCmd(_BaseCmd):
    cmd: str = field(default="create_shape", init=False)
    handle: int
    vertices: list[list[float]]
    pos: list[float]
    fill_color: list[float] | None
    line_color: list[float] | None
    line_width: float
    ori: float
    opacity: float
    enabled: bool


# ---------------------------------------------------------------------------
# Property setters (shared across all stimulus types)
# ---------------------------------------------------------------------------

@dataclass
class SetPosCmd(_BaseCmd):
    cmd: str = field(default="set_pos", init=False)
    handle: int
    pos: list[float]


@dataclass
class SetOriCmd(_BaseCmd):
    cmd: str = field(default="set_ori", init=False)
    handle: int
    ori: float


@dataclass
class SetOpacityCmd(_BaseCmd):
    cmd: str = field(default="set_opacity", init=False)
    handle: int
    opacity: float


@dataclass
class SetFillColorCmd(_BaseCmd):
    cmd: str = field(default="set_fill_color", init=False)
    handle: int
    fill_color: list[float] | None


@dataclass
class SetLineColorCmd(_BaseCmd):
    cmd: str = field(default="set_line_color", init=False)
    handle: int
    line_color: list[float] | None


@dataclass
class SetLineWidthCmd(_BaseCmd):
    cmd: str = field(default="set_line_width", init=False)
    handle: int
    line_width: float


@dataclass
class SetEnabledCmd(_BaseCmd):
    cmd: str = field(default="set_enabled", init=False)
    handle: int
    enabled: bool


@dataclass
class SetAutoDrawCmd(_BaseCmd):
    cmd: str = field(default="set_autodraw", init=False)
    handle: int
    autodraw: bool


@dataclass
class SetSizeCmd(_BaseCmd):
    cmd: str = field(default="set_size", init=False)
    handle: int
    size: list[float]


@dataclass
class DestroyCmd(_BaseCmd):
    cmd: str = field(default="destroy", init=False)
    handle: int
