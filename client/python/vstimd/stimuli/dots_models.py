from __future__ import annotations

import math
from dataclasses import dataclass, field
from enum import StrEnum

from vstimd._proto.vstimd.v1.stimuli import dots_pb2

from .color import Color
from .vec import Vec2


class DotShape(StrEnum):
    ROUND = "round"
    #: What Psychtoolbox's ``dot_type=0`` gives.
    SQUARE = "square"


class ApertureShape(StrEnum):
    RECT = "rect"
    CIRCLE = "circle"


class ApertureClip(StrEnum):
    """How the aperture edge cuts a dot."""

    #: A dot is drawn whole when its *centre* is inside, so dots overhang the edge
    #: uncut. The default, and what a motion-defined figure wants: cutting dots at
    #: the boundary draws a crisp outline of the aperture, which is a static form
    #: cue. It is also what Psychtoolbox scripts do, testing one pixel of the mask
    #: and then blitting the whole dot.
    DOT_CENTER = "dotCenter"
    #: A dot is cut at the aperture edge, per pixel. For a classic RDK whose hard
    #: aperture is meant to be seen.
    PIXEL = "pixel"


class SignalRule(StrEnum):
    """Is a dot's signal/noise role fixed, or redrawn every frame?

    PsychoPy's ``signalDots``; one half of the Scase, Braddick & Raymond (1996)
    taxonomy. Orthogonal to :class:`NoiseRule` because the two are independent
    choices and papers report them independently.
    """

    SAME = "same"
    DIFFERENT = "different"


class NoiseRule(StrEnum):
    """How a noise dot moves. PsychoPy's ``noiseDots``."""

    #: A fresh uniform position every frame — the dot reappears rather than moves.
    POSITION = "position"
    #: A random but *constant* direction, drawn at birth.
    DIRECTION = "direction"
    #: A fresh random direction every frame, at signal speed.
    WALK = "walk"


class Reinsertion(StrEnum):
    """What happens to a dot that leaves the field."""

    #: Re-enter from the opposite edge, holding density exactly constant. The
    #: default: with the field separate from the aperture, the wrap boundary is not
    #: a visible boundary, so it leaks no edge cue.
    WRAP = "wrap"
    RESPAWN = "respawn"


_DOT_SHAPE_TO_PROTO = {
    DotShape.ROUND: dots_pb2.DOT_SHAPE_ROUND,
    DotShape.SQUARE: dots_pb2.DOT_SHAPE_SQUARE,
}
_PROTO_TO_DOT_SHAPE = {v: k for k, v in _DOT_SHAPE_TO_PROTO.items()}

_APERTURE_SHAPE_TO_PROTO = {
    ApertureShape.RECT: dots_pb2.APERTURE_SHAPE_RECT,
    ApertureShape.CIRCLE: dots_pb2.APERTURE_SHAPE_CIRCLE,
}
_PROTO_TO_APERTURE_SHAPE = {v: k for k, v in _APERTURE_SHAPE_TO_PROTO.items()}

_APERTURE_CLIP_TO_PROTO = {
    ApertureClip.DOT_CENTER: dots_pb2.APERTURE_CLIP_DOT_CENTER,
    ApertureClip.PIXEL: dots_pb2.APERTURE_CLIP_PIXEL,
}
_PROTO_TO_APERTURE_CLIP = {v: k for k, v in _APERTURE_CLIP_TO_PROTO.items()}

_SIGNAL_RULE_TO_PROTO = {
    SignalRule.SAME: dots_pb2.SIGNAL_RULE_SAME,
    SignalRule.DIFFERENT: dots_pb2.SIGNAL_RULE_DIFFERENT,
}
_PROTO_TO_SIGNAL_RULE = {v: k for k, v in _SIGNAL_RULE_TO_PROTO.items()}

_NOISE_RULE_TO_PROTO = {
    NoiseRule.POSITION: dots_pb2.NOISE_RULE_POSITION,
    NoiseRule.DIRECTION: dots_pb2.NOISE_RULE_DIRECTION,
    NoiseRule.WALK: dots_pb2.NOISE_RULE_WALK,
}
_PROTO_TO_NOISE_RULE = {v: k for k, v in _NOISE_RULE_TO_PROTO.items()}

_REINSERTION_TO_PROTO = {
    Reinsertion.WRAP: dots_pb2.REINSERTION_WRAP,
    Reinsertion.RESPAWN: dots_pb2.REINSERTION_RESPAWN,
}
_PROTO_TO_REINSERTION = {v: k for k, v in _REINSERTION_TO_PROTO.items()}


@dataclass
class Aperture:
    """Where dots are *visible* — a separate thing from the field they live in.

    Conflating the two is invisible for a classic RDK, where the aperture is the
    field, and fatal for a figure-ground one: its background dots must fill the
    screen while being visible only outside a circle, and its figure dots only
    inside the same circle. Hence a mask with its own size, its own offset, and an
    ``invert`` flag, over a field that is always a plain rectangle.

    ``width_px``/``height_px`` are full extents, never half-extents. For
    ``CIRCLE``, ``width_px`` is the **diameter** and ``height_px`` is ignored.
    Psychtoolbox scripts specify a radius — double it (see
    :func:`diameter_from_radius`).

    Zero width or height means "the field", i.e. no crop in that axis.
    """

    shape: ApertureShape = ApertureShape.RECT
    width_px: float = 0.0
    height_px: float = 0.0
    #: Aperture centre relative to the stimulus position, which is the field centre.
    offset_px: Vec2 = field(default_factory=lambda: Vec2(0.0, 0.0))
    #: Draw *outside* the shape. This is the whole of "background dots, everywhere
    #: but the figure".
    invert: bool = False
    clip: ApertureClip = ApertureClip.DOT_CENTER

    @classmethod
    def from_proto(cls, proto: dots_pb2.Aperture) -> Aperture:
        return cls(
            shape=_PROTO_TO_APERTURE_SHAPE.get(proto.shape, ApertureShape.RECT),
            width_px=proto.width_px,
            height_px=proto.height_px,
            offset_px=Vec2(proto.offset_x_px, proto.offset_y_px),
            invert=proto.invert,
            clip=_PROTO_TO_APERTURE_CLIP.get(proto.clip, ApertureClip.DOT_CENTER),
        )

    def to_proto(self) -> dots_pb2.Aperture:
        return dots_pb2.Aperture(
            shape=_APERTURE_SHAPE_TO_PROTO[self.shape],
            width_px=self.width_px,
            height_px=self.height_px,
            offset_x_px=self.offset_px.x,
            offset_y_px=self.offset_px.y,
            invert=self.invert,
            clip=_APERTURE_CLIP_TO_PROTO[self.clip],
        )


@dataclass
class DotsParams:
    """Everything about a random dot kinematogram except the dots themselves.

    Sent by ``create_dots`` and reported back by ``query``, mirroring the one
    ``DotsParams`` message the wire uses in both directions. Sizes and counts
    follow the usual convention — 0 means *default*, not literally zero — with two
    deliberate exceptions: ``speed_px_per_s`` and ``coherence`` are ``None`` when
    unset, because zero is meaningful for both (a static field, and a field of
    pure noise) and would otherwise be inexpressible.

    The **sample is a function of** ``seed`` **and the frame index alone**, so
    replaying a saved config reproduces the stimulus rather than merely one like
    it. Record the seed; do not leave it to chance.
    """

    # ── field: where dots live and wrap; invisible ──
    field_width_px: float = 0.0
    field_height_px: float = 0.0
    #: Stored rather than derived from a density, because this is the number a
    #: methods section quotes. See :func:`dots_for_density` for the conversion.
    dot_count: int = 0

    # ── aperture: where dots are visible ──
    aperture: Aperture = field(default_factory=Aperture)

    # ── appearance ──
    #: Dot **diameter**, not radius.
    dot_size_px: float = 0.0
    dot_color: Color = field(default_factory=lambda: Color(1.0, 1.0, 1.0, 1.0))
    #: A second colour, assigned to each dot at birth with probability ½ —
    #: Psychtoolbox's ``bwSameTrial``. ``None`` gives a single-colour field.
    dot_color_alt: Color | None = None
    dot_shape: DotShape = DotShape.ROUND

    # ── motion ──
    #: CCW, 0° = right — the same convention as ``rotation_deg``. Psychtoolbox
    #: angles are measured against a downward Y and negate across; see
    #: :func:`direction_from_ptb_rad`.
    direction_deg: float = 0.0
    #: Per *second*, not per frame. ``None`` → the server default (100).
    speed_px_per_s: float | None = None
    #: Fraction of dots carrying the coherent direction, [0, 1]. ``None`` → 1.
    coherence: float | None = None
    signal_rule: SignalRule = SignalRule.SAME
    noise_rule: NoiseRule = NoiseRule.DIRECTION
    reinsertion: Reinsertion = Reinsertion.WRAP

    # ── lifetime ──
    #: Frames before a dot is reborn; 0 is infinite. PsychoPy spells infinite as
    #: -1 — :func:`lifetime_from_psychopy` translates. Births are staggered
    #: uniformly by construction, so a field never flickers in lockstep.
    dot_lifetime_frames: int = 0

    # ── reproducibility ──
    seed: int = 0

    @classmethod
    def from_proto(cls, proto: dots_pb2.DotsParams) -> DotsParams:
        alt = Color.from_proto(proto.dot_color_alt) if proto.HasField("dot_color_alt") else None
        color = (
            Color.from_proto(proto.dot_color)
            if proto.HasField("dot_color")
            else Color(1.0, 1.0, 1.0, 1.0)
        )
        return cls(
            field_width_px=proto.field_width_px,
            field_height_px=proto.field_height_px,
            dot_count=proto.dot_count,
            aperture=Aperture.from_proto(proto.aperture),
            dot_size_px=proto.dot_size_px,
            dot_color=color,
            dot_color_alt=alt,
            dot_shape=_PROTO_TO_DOT_SHAPE.get(proto.dot_shape, DotShape.ROUND),
            direction_deg=proto.direction_deg,
            speed_px_per_s=proto.speed_px_per_s if proto.HasField("speed_px_per_s") else None,
            coherence=proto.coherence if proto.HasField("coherence") else None,
            signal_rule=_PROTO_TO_SIGNAL_RULE.get(proto.signal_rule, SignalRule.SAME),
            noise_rule=_PROTO_TO_NOISE_RULE.get(proto.noise_rule, NoiseRule.DIRECTION),
            reinsertion=_PROTO_TO_REINSERTION.get(proto.reinsertion, Reinsertion.WRAP),
            dot_lifetime_frames=proto.dot_lifetime_frames,
            seed=proto.seed,
        )

    def to_proto(self) -> dots_pb2.DotsParams:
        proto = dots_pb2.DotsParams(
            field_width_px=self.field_width_px,
            field_height_px=self.field_height_px,
            dot_count=self.dot_count,
            aperture=self.aperture.to_proto(),
            dot_size_px=self.dot_size_px,
            dot_color=self.dot_color.to_proto(),
            dot_shape=_DOT_SHAPE_TO_PROTO[self.dot_shape],
            direction_deg=self.direction_deg,
            signal_rule=_SIGNAL_RULE_TO_PROTO[self.signal_rule],
            noise_rule=_NOISE_RULE_TO_PROTO[self.noise_rule],
            reinsertion=_REINSERTION_TO_PROTO[self.reinsertion],
            dot_lifetime_frames=self.dot_lifetime_frames,
            seed=self.seed,
        )
        # Set only when given: absence is what carries "use the default" for these
        # three, since zero is a value each of them can legitimately take.
        if self.dot_color_alt is not None:
            proto.dot_color_alt.CopyFrom(self.dot_color_alt.to_proto())
        if self.speed_px_per_s is not None:
            proto.speed_px_per_s = self.speed_px_per_s
        if self.coherence is not None:
            proto.coherence = self.coherence
        return proto


# ── Unit helpers ──────────────────────────────────────────────────────────────
#
# vstimd speaks pixels; the RDK literature speaks degrees of visual angle, dots
# per deg² and deg/s. The conversion needs the rig geometry, which the server does
# not carry, so it lives here — on the client, where the experimenter knows their
# viewing distance. The config still records exactly what was shown, in pixels.


def px_per_deg(screen_width_px: int, screen_width_cm: float, viewing_distance_cm: float) -> float:
    """Pixels per degree of visual angle, at the centre of the screen.

    Uses the small-angle relation at the screen centre, which is what
    Psychtoolbox's ``deg2pix`` does. It overestimates eccentric sizes on a flat
    screen; for a 45° stimulus that error is real but is the same error the
    original made, which is what matters when reproducing one.
    """
    px_per_cm = screen_width_px / screen_width_cm
    cm_per_deg = 2.0 * viewing_distance_cm * math.tan(math.radians(0.5))
    return px_per_cm * cm_per_deg


def dots_for_density(density_per_deg2: float, field_width_deg: float,
                     field_height_deg: float) -> int:
    """Dot count for a density over a field, rounded to the nearest whole dot.

    MWorks takes a density and derives the count; Psychtoolbox and PsychoPy take a
    count. vstimd stores the count — it is what a methods section quotes and what
    the config must record — so the density is converted here, once, at create
    time.
    """
    return round(density_per_deg2 * field_width_deg * field_height_deg)


def diameter_from_radius(radius: float) -> float:
    """Double a radius into the diameter vstimd wants.

    Every size in vstimd is a full extent. Psychtoolbox specifies dot sizes and
    aperture radii as half-extents, so a port crosses this boundary — silently, if
    it forgets: the stimulus renders, at half the intended size.
    """
    return radius * 2.0


def direction_from_ptb_rad(angle_rad: float) -> float:
    """Convert a Psychtoolbox direction to ``direction_deg``.

    Psychtoolbox adds ``sin(angle)`` to a *row index*, which grows downward, so its
    angles run clockwise. vstimd is Y-up and counter-clockwise, like
    ``rotation_deg``. The mapping is a mirror: ``3*pi/2`` — which is **upward** on a
    Psychtoolbox screen — becomes 90°, not 270°.
    """
    return (-math.degrees(angle_rad)) % 360.0


def lifetime_from_psychopy(dot_life: int) -> int:
    """Convert PsychoPy's ``dotLife`` to ``dot_lifetime_frames``.

    PsychoPy spells an infinite lifetime ``-1``; vstimd and MWorks spell it ``0``.
    """
    return 0 if dot_life < 0 else dot_life
