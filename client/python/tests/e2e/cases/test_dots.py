"""E2E tests for random dot kinematograms.

The on-screen descriptions matter more here than for a static stimulus: an RDK is
correct or not by how it *moves*, and several of the ways it can be wrong — dots
flickering in lockstep, a crisp circular edge where there should be none, a figure
that is visible in a freeze-frame — are invisible in a single captured image.
"""

from __future__ import annotations

import math
from dataclasses import replace

import pytest

from vstimd import Connection
from vstimd.stimuli import (
    Aperture,
    ApertureClip,
    ApertureShape,
    DotShape,
    DotsParams,
    NoiseRule,
    StimulusType,
    diameter_from_radius,
    direction_from_ptb_rad,
)
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "DOTS-01",
    "a field of white dots in a 400 px circle in the centre, all drifting "
    "rightward together",
)
def test_create_dots(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.dots.create_dots(
        params=DotsParams(
            field_width_px=400,
            field_height_px=400,
            dot_count=150,
            dot_size_px=8,
            aperture=Aperture(shape=ApertureShape.CIRCLE, width_px=400),
            direction_deg=0.0,
            speed_px_per_s=120.0,
            coherence=1.0,
            seed=1,
        ),
    )
    assert handle > 0

    info = conn.stimuli.query(handle)
    assert info.stimulus_type == StimulusType.DOTS
    assert isinstance(info.params, DotsParams)
    assert info.params.dot_count == 150
    assert info.params.dot_size_px == pytest.approx(8.0, abs=0.5)
    assert info.params.coherence == pytest.approx(1.0)
    assert info.params.aperture.shape == ApertureShape.CIRCLE
    # Sized by its diameter, not its radius.
    assert info.params.aperture.width_px == pytest.approx(400.0, abs=0.5)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "DOTS-02",
    "the same dot field with coherence stepped 1.0 → 0.5 → 0.0: it goes from "
    "every dot moving right together, to half of them, to none — a boil with no "
    "net direction",
)
def test_dots_coherence(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.dots.create_dots(
        params=DotsParams(
            field_width_px=500, field_height_px=500, dot_count=300, dot_size_px=8,
            aperture=Aperture(shape=ApertureShape.CIRCLE, width_px=500),
            speed_px_per_s=150.0, coherence=1.0, noise_rule=NoiseRule.DIRECTION, seed=2,
        ),
    )
    for coherence in (1.0, 0.5, 0.0):
        conn.stimuli.dots.set_coherence(handle, coherence)
        info = conn.stimuli.query(handle)
        assert isinstance(info.params, DotsParams)
        assert info.params.coherence == pytest.approx(coherence, abs=0.01)
        stage.step(f"coherence = {coherence}")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "DOTS-03",
    "a dot field whose direction steps right → up → left → down. The dots turn "
    "where they are; nothing jumps back to the middle at a turn",
)
def test_dots_direction_changes_are_continuous(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.dots.create_dots(
        params=DotsParams(
            field_width_px=600, field_height_px=600, dot_count=200, dot_size_px=8,
            speed_px_per_s=200.0, coherence=1.0, seed=3,
        ),
    )
    for direction in (0.0, 90.0, 180.0, 270.0):
        conn.stimuli.dots.set_direction(handle, direction)
        info = conn.stimuli.query(handle)
        assert isinstance(info.params, DotsParams)
        assert info.params.direction_deg == pytest.approx(direction, abs=0.1)
        stage.step(f"direction_deg = {direction}")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "DOTS-04",
    "black and white dots together on the grey background, half of each, each "
    "dot keeping its own polarity as it moves",
)
def test_dots_two_colors(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.dots.create_dots(
        params=DotsParams(
            field_width_px=500, field_height_px=500, dot_count=250, dot_size_px=10,
            dot_color=Color(1.0, 1.0, 1.0), dot_color_alt=Color(0.0, 0.0, 0.0),
            speed_px_per_s=120.0, seed=4,
        ),
    )
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, DotsParams)
    assert info.params.dot_color_alt == Color(0.0, 0.0, 0.0, 1.0)
    stage.step("black and white dots")

    conn.stimuli.dots.set_dot_color(handle, Color(1.0, 1.0, 1.0))
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, DotsParams)
    assert info.params.dot_color_alt is None, "clearing the alt colour"
    stage.step("white dots only — the black ones are gone")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "DOTS-05",
    "square dots, then round ones — the same field, redrawn",
)
def test_dots_shape(conn: Connection, stage: Stage) -> None:
    for shape in (DotShape.SQUARE, DotShape.ROUND):
        handle = conn.stimuli.dots.create_dots(
            params=DotsParams(
                field_width_px=400, field_height_px=400, dot_count=60,
                dot_size_px=24, dot_shape=shape, speed_px_per_s=60.0, seed=5,
            ),
        )
        info = conn.stimuli.query(handle)
        assert isinstance(info.params, DotsParams)
        assert info.params.dot_shape == shape
        stage.step(f"dot_shape = {shape}")
        conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "DOTS-06",
    "dots with a 10-frame lifetime: they twinkle steadily, a few replaced each "
    "frame. Nothing blinks all at once — if the whole field flashes in step, "
    "birth staggering is broken",
)
def test_dots_lifetime_does_not_flicker_in_lockstep(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.dots.create_dots(
        params=DotsParams(
            field_width_px=500, field_height_px=500, dot_count=200, dot_size_px=8,
            speed_px_per_s=150.0, dot_lifetime_frames=10, seed=6,
        ),
    )
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, DotsParams)
    assert info.params.dot_lifetime_frames == 10
    stage.step("dot_lifetime_frames = 10")

    conn.stimuli.dots.set_dot_lifetime(handle, 0)
    info = conn.stimuli.query(handle)
    assert isinstance(info.params, DotsParams)
    assert info.params.dot_lifetime_frames == 0
    stage.step("dot_lifetime_frames = 0 — infinite; the twinkling stops")
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "DOTS-07",
    "the same seed twice: the two fields are pixel-identical at rest. If the "
    "second differs from the first, a saved config no longer replays",
)
def test_dots_seed_reproduces(conn: Connection, stage: Stage) -> None:
    params = DotsParams(
        field_width_px=400, field_height_px=400, dot_count=100, dot_size_px=10,
        speed_px_per_s=0.0, seed=7,
    )
    a = conn.stimuli.dots.create_dots(params=params)
    stage.step("seed 7, first field")
    conn.stimuli.delete(a)
    b = conn.stimuli.dots.create_dots(params=params)
    stage.step("seed 7 again — the identical field")
    conn.stimuli.delete(b)


@pytest.mark.onscreen(
    "DOTS-08",
    "the figure-ground stimulus: dots everywhere, moving rightward outside a "
    "45°-wide circle and upward inside it. The circle should be invisible in a "
    "freeze-frame and obvious in motion — no edge, no density step, no dots cut "
    "in half at the boundary",
)
def test_figure_ground(conn: Connection, stage: Stage) -> None:
    """The reproduction target — see ``dev/design/RDK_PLAN.md``.

    The MATLAB's radii are doubled here and its angles mirrored, which is the whole
    of the port: ``dotSize = 1.5`` is a dot 3 deg across, ``R = 45/2`` a circle 45
    deg across, and ``dirAngleFigure = 3*pi/2`` is *upward*, i.e. 90°.
    """
    info = conn.system.query_server_info()
    screen = (float(info.width_px), float(info.height_px))
    # A stand-in for the rig's real deg2pix; the example script derives it from
    # the viewing geometry.
    px_per_deg = screen[0] / 96.0

    circle = Aperture(
        shape=ApertureShape.CIRCLE,
        width_px=diameter_from_radius(45.0 / 2.0) * px_per_deg,
        offset_px=Vec2(0.0, 0.0),
        clip=ApertureClip.DOT_CENTER,
    )
    common = dict(
        field_width_px=screen[0],
        field_height_px=screen[1],
        dot_count=round((screen[0] / px_per_deg) * (screen[1] / px_per_deg) / 25.0),
        dot_size_px=diameter_from_radius(1.5) * px_per_deg,
        speed_px_per_s=50.0 * px_per_deg,
        coherence=1.0,
        dot_lifetime_frames=0,
    )
    background = conn.stimuli.dots.create_dots(
        name="background",
        params=DotsParams(
            aperture=replace(circle, invert=True),
            direction_deg=direction_from_ptb_rad(0.0),  # 0°
            seed=1,
            **common,
        ),
    )
    figure = conn.stimuli.dots.create_dots(
        name="figure",
        params=DotsParams(
            aperture=circle,
            direction_deg=direction_from_ptb_rad(3 * math.pi / 2),  # 90°, up
            seed=2,
            **common,
        ),
    )
    fig_info = conn.stimuli.query(figure)
    bg_info = conn.stimuli.query(background)
    assert isinstance(fig_info.params, DotsParams)
    assert isinstance(bg_info.params, DotsParams)
    assert fig_info.params.direction_deg == pytest.approx(90.0), "3*pi/2 is UP, i.e. 90°"
    assert bg_info.params.direction_deg == pytest.approx(0.0)
    assert bg_info.params.aperture.invert, "the background is the circle's complement"
    assert not fig_info.params.aperture.invert
    stage.step("both fields moving — the figure is defined by direction alone")

    conn.stimuli.dots.set_direction(background, 90.0)
    stage.step("background switched to match the figure — the circle disappears")

    conn.stimuli.delete(figure)
    conn.stimuli.delete(background)
