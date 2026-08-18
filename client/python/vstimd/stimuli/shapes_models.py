from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from typing import Sequence

from vstimd._proto.vstimd.v1.stimuli import (
    circle_pb2,
    ellipse_pb2,
    polygon_pb2,
    rect_pb2,
    shapes_pb2,
)

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

    # ``None`` means "inherit": the wire leaves the field absent and the server
    # fills it from the scene's default_fill / default_outline. Defaulting these
    # to a concrete black instead would make every create silently override the
    # scene defaults — which is not the same picture. A query always comes back
    # with both set, since the server reports what the stimulus actually has.
    fill_color: Color | None = None
    outline_color: Color | None = None
    # 0 means unset here too, matching the proto: a 0-width outline draws nothing
    # anyway, so `draw_mode` is how an outline is turned off, not width.
    outline_width: float = 0.0
    draw_mode: ShapeDrawMode = ShapeDrawMode.FILLED

    @classmethod
    def from_proto(cls, proto: shapes_pb2.ShapeAppearance) -> ShapeAppearance:
        return cls(
            fill_color=Color.from_proto(proto.fill_color)
            if proto.HasField("fill_color")
            else None,
            outline_color=Color.from_proto(proto.outline_color)
            if proto.HasField("outline_color")
            else None,
            outline_width=proto.outline_width,
            draw_mode=_PROTO_TO_DRAW_MODE.get(proto.draw_mode, ShapeDrawMode.FILLED),
        )

    def to_proto(self) -> shapes_pb2.ShapeAppearance:
        return shapes_pb2.ShapeAppearance(
            fill_color=self.fill_color.to_proto() if self.fill_color else None,
            outline_color=self.outline_color.to_proto() if self.outline_color else None,
            outline_width=self.outline_width,
            draw_mode=_SHAPE_DRAW_MODE_TO_PROTO.get(
                self.draw_mode, shapes_pb2.SHAPE_DRAW_MODE_FILLED
            ),
        )


def _appearance_or_default(params: object) -> ShapeAppearance:
    """`appearance` off a shape params message, defaulted when absent."""
    proto = getattr(params, "appearance", None)
    return ShapeAppearance.from_proto(proto) if proto is not None else ShapeAppearance()


# ── Per-shape geometry ────────────────────────────────────────────────────────
#
# Sent by ``create_*`` and reported back by ``query``: each mirrors the one
# ``*Params`` message the wire uses in both directions. The size fields default
# to 0, which the server reads as "use your default" rather than as a zero-sized
# shape — the same convention the proto documents.


@dataclass
class RectParams:
    width: float = 0.0
    height: float = 0.0
    appearance: ShapeAppearance = field(default_factory=ShapeAppearance)

    def to_proto(self) -> rect_pb2.RectParams:
        return rect_pb2.RectParams(
            width=self.width,
            height=self.height,
            appearance=self.appearance.to_proto(),
        )


@dataclass
class CircleParams:
    # Diameter, not radius: a full extent, like every other geometry here.
    diameter: float = 0.0
    appearance: ShapeAppearance = field(default_factory=ShapeAppearance)

    def to_proto(self) -> circle_pb2.CircleParams:
        return circle_pb2.CircleParams(
            diameter=self.diameter,
            appearance=self.appearance.to_proto(),
        )


@dataclass
class EllipseParams:
    width: float = 0.0
    height: float = 0.0
    appearance: ShapeAppearance = field(default_factory=ShapeAppearance)

    def to_proto(self) -> ellipse_pb2.EllipseParams:
        return ellipse_pb2.EllipseParams(
            width=self.width,
            height=self.height,
            appearance=self.appearance.to_proto(),
        )


@dataclass
class PolygonParams:
    vertices: list[Vec2] = field(default_factory=list)
    close_shape: bool = True
    appearance: ShapeAppearance = field(default_factory=ShapeAppearance)

    def to_proto(self) -> polygon_pb2.PolygonParams:
        return polygon_pb2.PolygonParams(
            vertices=[v.to_proto() for v in self.vertices],
            close_shape=self.close_shape,
            appearance=self.appearance.to_proto(),
        )
