"""Unit tests for the dot-field models and the Psychtoolbox unit helpers.

The helpers are where a port from Psychtoolbox crosses two silent traps — sizes
that are radii there and diameters here, and directions measured against an axis
that points the other way. Both fail by rendering something plausible, so they are
tested rather than trusted.
"""

from __future__ import annotations

import math

import pytest

from vstimd.stimuli import (
    Aperture,
    ApertureClip,
    ApertureShape,
    Color,
    DotShape,
    DotsParams,
    NoiseRule,
    Reinsertion,
    SignalRule,
    Vec2,
    diameter_from_radius,
    direction_from_ptb_rad,
    dots_for_density,
    lifetime_from_psychopy,
    px_per_deg,
)


# ── Psychtoolbox porting helpers ──────────────────────────────────────────────


def test_radius_doubles_into_a_diameter():
    # dotSize = 1.5 deg is a *radius*: the dot is 3 deg across.
    assert diameter_from_radius(1.5) == 3.0
    # R = 45/2 is a radius: the figure circle is 45 deg across.
    assert diameter_from_radius(45 / 2) == 45.0


@pytest.mark.parametrize(
    "ptb_rad, expected_deg",
    [
        (0.0, 0.0),  # rightward in both
        (math.pi / 2, 270.0),  # PTB down → vstimd 270°
        (math.pi, 180.0),  # leftward in both
        (3 * math.pi / 2, 90.0),  # PTB *up* → vstimd 90°, not 270°
    ],
)
def test_ptb_directions_mirror(ptb_rad, expected_deg):
    assert direction_from_ptb_rad(ptb_rad) == pytest.approx(expected_deg)


def test_the_two_directions_the_figure_ground_stimulus_uses():
    """`dirAngle = [0, 3*pi/2]` is rightward and upward — 0° and 90° here."""
    assert [direction_from_ptb_rad(a) for a in (0.0, 3 * math.pi / 2)] == [0.0, 90.0]


def test_density_converts_to_a_count():
    # 1 dot per (5 deg)² over a 96 × 54 deg screen.
    assert dots_for_density(1 / 25, 96.0, 54.0) == 207


def test_psychopy_infinite_lifetime_translates():
    assert lifetime_from_psychopy(-1) == 0  # PsychoPy's infinite
    assert lifetime_from_psychopy(0) == 0
    assert lifetime_from_psychopy(10) == 10


def test_px_per_deg_scales_with_distance():
    near = px_per_deg(1920, 52.0, 57.0)
    far = px_per_deg(1920, 52.0, 114.0)
    assert far == pytest.approx(2 * near, rel=1e-6)
    # A 57 cm viewing distance is the classic "1 cm ≈ 1 deg" setup.
    assert px_per_deg(1920, 52.0, 57.0) == pytest.approx(1920 / 52.0, rel=0.01)


# ── Model round trips ─────────────────────────────────────────────────────────


def test_params_round_trip_through_proto():
    sent = DotsParams(
        field_width_px=1920.0,
        field_height_px=1080.0,
        dot_count=207,
        aperture=Aperture(
            shape=ApertureShape.CIRCLE,
            width_px=900.0,
            offset_px=Vec2(200.0, -150.0),
            invert=True,
            clip=ApertureClip.PIXEL,
        ),
        dot_size_px=60.0,
        dot_color=Color(1.0, 1.0, 1.0, 1.0),
        dot_color_alt=Color(0.0, 0.0, 0.0, 1.0),
        dot_shape=DotShape.SQUARE,
        direction_deg=90.0,
        speed_px_per_s=1000.0,
        # Exactly representable in the float32 the wire carries; a value like
        # 0.35 would come back as 0.34999999 and the comparison would be about
        # float widths rather than about the round trip.
        coherence=0.375,
        signal_rule=SignalRule.DIFFERENT,
        noise_rule=NoiseRule.WALK,
        reinsertion=Reinsertion.RESPAWN,
        dot_lifetime_frames=12,
        seed=4242,
    )
    assert DotsParams.from_proto(sent.to_proto()) == sent


def test_zero_is_expressible_for_speed_and_coherence():
    """Zero is a real setting for both — a static field, and pure noise — so it
    must survive the trip rather than being read as 'unset'."""
    p = DotsParams(speed_px_per_s=0.0, coherence=0.0)
    back = DotsParams.from_proto(p.to_proto())
    assert back.speed_px_per_s == 0.0
    assert back.coherence == 0.0


def test_unset_speed_and_coherence_stay_unset():
    back = DotsParams.from_proto(DotsParams().to_proto())
    assert back.speed_px_per_s is None
    assert back.coherence is None


def test_no_alt_color_by_default():
    assert DotsParams.from_proto(DotsParams().to_proto()).dot_color_alt is None
    p = DotsParams(dot_color_alt=Color(0.0, 0.0, 0.0, 1.0))
    assert DotsParams.from_proto(p.to_proto()).dot_color_alt == Color(0.0, 0.0, 0.0, 1.0)


def test_inverted_aperture_round_trips():
    a = Aperture(shape=ApertureShape.CIRCLE, width_px=900.0, invert=True)
    assert Aperture.from_proto(a.to_proto()) == a
