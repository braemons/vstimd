from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Sequence

from vstimd._proto.vstimd.v1.stimuli import shapes_pb2

from .color import Color
from .vec import Vec2


class ShapeDrawMode(StrEnum):
    FILLED = "filled"
    OUTLINED = "outlined"
    FILLED_AND_OUTLINED = "filled_and_outlined"


_PROTO_TO_DRAW_MODE: dict[int, ShapeDrawMode] = {
    shapes_pb2.SHAPE_DRAW_MODE_FILLED: ShapeDrawMode.FILLED,
    shapes_pb2.SHAPE_DRAW_MODE_OUTLINED: ShapeDrawMode.OUTLINED,
    shapes_pb2.SHAPE_DRAW_MODE_FILLED_AND_OUTLINED: ShapeDrawMode.FILLED_AND_OUTLINED,
}

_SHAPE_DRAW_MODE_TO_PROTO: dict[ShapeDrawMode, shapes_pb2.ShapeDrawMode] = {
    ShapeDrawMode.FILLED: shapes_pb2.SHAPE_DRAW_MODE_FILLED,
    ShapeDrawMode.OUTLINED: shapes_pb2.SHAPE_DRAW_MODE_OUTLINED,
    ShapeDrawMode.FILLED_AND_OUTLINED: shapes_pb2.SHAPE_DRAW_MODE_FILLED_AND_OUTLINED,
}


@dataclass
class ShapeAppearance:
    """Fill/outline state of a shape stimulus.

    Reported with the shape's own params rather than at the top level of a
    query, because only shapes have it: a grating has fore/back colours, text
    has a glyph colour, and neither has an outline. The alphas here are the
    colours' own — the shared ``StimulusInfo.opacity`` multiplies them.
    """

    fill_color: Color = field(default_factory=lambda: Color(0.0, 0.0, 0.0))
    outline_color: Color = field(default_factory=lambda: Color(0.0, 0.0, 0.0))
    outline_width: float = 0.0
    draw_mode: ShapeDrawMode = ShapeDrawMode.FILLED

    @classmethod
    def from_proto(cls, proto: shapes_pb2.ShapeAppearance) -> ShapeAppearance:
        return cls(
            fill_color=Color.from_proto(proto.fill_color)
            if proto.HasField("fill_color")
            else Color(0.0, 0.0, 0.0),
            outline_color=Color.from_proto(proto.outline_color)
            if proto.HasField("outline_color")
            else Color(0.0, 0.0, 0.0),
            outline_width=proto.outline_width,
            draw_mode=_PROTO_TO_DRAW_MODE.get(proto.draw_mode, ShapeDrawMode.FILLED),
        )

    def to_proto(self) -> shapes_pb2.ShapeAppearance:
        return shapes_pb2.ShapeAppearance(
            fill_color=self.fill_color.to_proto(),
            outline_color=self.outline_color.to_proto(),
            outline_width=self.outline_width,
            draw_mode=_DRAW_MODE_TO_PROTO.get(self.draw_mode, shapes_pb2.ShapeDrawMode.SHAPE_DRAW_MODE_FILLED),
        )


def _appearance_or_default(params: object) -> ShapeAppearance:
    """`appearance` off a shape params message, defaulted when absent."""
    proto = getattr(params, "appearance", None)
    return ShapeAppearance.from_proto(proto) if proto is not None else ShapeAppearance()


@dataclass
class RectParams:
    width: float
    height: float
    appearance: ShapeAppearance = field(default_factory=ShapeAppearance)


@dataclass
class CircleParams:
    radius: float
    appearance: ShapeAppearance = field(default_factory=ShapeAppearance)


@dataclass
class EllipseParams:
    width: float
    height: float
    appearance: ShapeAppearance = field(default_factory=ShapeAppearance)


@dataclass
class PolygonParams:
    vertices: list[Vec2]
    close_shape: bool = True
    appearance: ShapeAppearance = field(default_factory=ShapeAppearance)
