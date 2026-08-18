"""E2E tests for shape draw-mode and outline properties (shapes.proto)."""

from __future__ import annotations

import pytest

from vstimd import Connection
from vstimd.stimuli import CircleParams, EllipseParams, RectParams, ShapeAppearance, ShapeDrawMode
from vstimd.stimuli.stimuli_models import Color, Vec2

from ._helpers import Stage


@pytest.mark.onscreen(
    "SHAPE-01",
    "a 100×100 px square drawn OUTLINED: a hollow white frame in the centre, "
    "background showing through the middle",
)
def test_set_draw_mode_outlined(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(params=RectParams(width_px=100, height_px=100))
    conn.stimuli.shapes.set_draw_mode(handle, ShapeDrawMode.OUTLINED)
    info = conn.stimuli.query(handle)
    assert info.draw_mode == ShapeDrawMode.OUTLINED

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHAPE-02",
    "a 100 px disc drawn FILLED_AND_OUTLINED: a solid disc in the centre with "
    "an outline ring around its rim",
)
def test_set_draw_mode_filled_and_outlined(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_circle(params=CircleParams(diameter_px=100))
    conn.stimuli.shapes.set_draw_mode(handle, ShapeDrawMode.FILLED_AND_OUTLINED)
    info = conn.stimuli.query(handle)
    assert info.draw_mode == ShapeDrawMode.FILLED_AND_OUTLINED

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHAPE-03",
    "a 100×80 px rect in the centre. Its outline colour is set to orange, but "
    "the draw mode stays FILLED, so the rect still looks plain white",
)
def test_set_outline_color_roundtrip(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(params=RectParams(width_px=100, height_px=80))
    conn.stimuli.shapes.set_outline_color(handle, Color(1.0, 0.5, 0.0, 0.8))
    info = conn.stimuli.query(handle)
    assert info.outline_color.r == pytest.approx(1.0, abs=0.01)
    assert info.outline_color.g == pytest.approx(0.5, abs=0.01)
    assert info.outline_color.b == pytest.approx(0.0, abs=0.01)
    assert info.outline_color.a == pytest.approx(0.8, abs=0.01)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHAPE-04",
    "a 120×80 px ellipse in the centre. Its outline width is set to 6 px, but "
    "the draw mode stays FILLED, so the outline itself is not drawn",
)
def test_set_outline_width_roundtrip(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_ellipse(params=EllipseParams(width_px=120, height_px=80))
    conn.stimuli.shapes.set_outline_width(handle, 6.0)
    info = conn.stimuli.query(handle)
    assert info.outline_width_px == pytest.approx(6.0, abs=0.1)

    stage.hold()
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHAPE-05",
    "a square, then a disc, then an ellipse in turn — each solid white with no "
    "outline, because FILLED is what a shape defaults to",
)
def test_draw_mode_default_is_filled(conn: Connection, stage: Stage) -> None:
    shapes = [
        ("square", lambda: conn.stimuli.shapes.create_rect(
            params=RectParams(width_px=100, height_px=100))),
        ("disc", lambda: conn.stimuli.shapes.create_circle(
            params=CircleParams(diameter_px=100))),
        ("ellipse", lambda: conn.stimuli.shapes.create_ellipse(
            params=EllipseParams(width_px=100, height_px=60))),
    ]
    for shape, create in shapes:
        h = create()
        info = conn.stimuli.query(h)
        assert info.draw_mode == ShapeDrawMode.FILLED

        stage.step(f"{shape}: solid, no outline (the default draw mode)", hold=0.5)
        conn.stimuli.delete(h)


@pytest.mark.onscreen(
    "SHAPE-06",
    "one 100×100 px square cycling through the draw modes: hollow frame "
    "(OUTLINED), then solid with a rim (FILLED_AND_OUTLINED), then plain solid "
    "(FILLED)",
)
def test_draw_mode_cycle(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(params=RectParams(width_px=100, height_px=100))
    conn.stimuli.shapes.set_outline_color(handle, Color(1.0, 1.0, 0.0))
    conn.stimuli.shapes.set_outline_width(handle, 6.0)
    for mode, description in (
        (ShapeDrawMode.OUTLINED, "OUTLINED — yellow frame only, hollow inside"),
        (ShapeDrawMode.FILLED_AND_OUTLINED, "FILLED_AND_OUTLINED — white fill, yellow rim"),
        (ShapeDrawMode.FILLED, "FILLED — white fill, no rim"),
    ):
        conn.stimuli.shapes.set_draw_mode(handle, mode)
        info = conn.stimuli.query(handle)
        assert info.draw_mode == mode
        stage.step(description, hold=0.7)
    conn.stimuli.delete(handle)


@pytest.mark.onscreen(
    "SHAPE-07",
    "three shapes side by side on dark grey — blue rect, orange disc, green "
    "ellipse — shown three times over, once per draw mode, with 6 px yellow "
    "outlines",
)
def test_outline_visual(conn: Connection, stage: Stage) -> None:
    """Display each draw mode so a human can visually verify outlines."""
    conn.system.set_background(r=0.15, g=0.15, b=0.15)

    ROWS = [
        (ShapeDrawMode.FILLED,
         "FILLED — three solid shapes, no yellow anywhere"),
        (ShapeDrawMode.OUTLINED,
         "OUTLINED — three yellow outlines, grey background showing through"),
        (ShapeDrawMode.FILLED_AND_OUTLINED,
         "FILLED_AND_OUTLINED — the same solid shapes, each ringed in yellow"),
    ]

    for mode, description in ROWS:
        rect = conn.stimuli.shapes.create_rect(
            position_px=Vec2(-200, 0),
            params=RectParams(
                width_px=180,
                height_px=120,
                appearance=ShapeAppearance(fill_color=Color(0.2, 0.5, 0.9)),
            ),
        )
        circ = conn.stimuli.shapes.create_circle(
            position_px=Vec2(0, 0),
            params=CircleParams(
                diameter_px=140,
                appearance=ShapeAppearance(fill_color=Color(0.9, 0.4, 0.2)),
            ),
        )
        ell = conn.stimuli.shapes.create_ellipse(
            position_px=Vec2(200, 0),
            params=EllipseParams(
                width_px=200,
                height_px=100,
                appearance=ShapeAppearance(fill_color=Color(0.3, 0.8, 0.3)),
            ),
        )
        for h in (rect, circ, ell):
            conn.stimuli.shapes.set_draw_mode(h, mode)
            conn.stimuli.shapes.set_outline_color(h, Color(1.0, 1.0, 0.0))
            conn.stimuli.shapes.set_outline_width(h, 6.0)

        stage.step(description)

        for h in (rect, circ, ell):
            conn.stimuli.delete(h)

    conn.system.set_background(r=0.0, g=0.0, b=0.0)


@pytest.mark.onscreen(
    "SHAPE-08",
    "a square whose fill goes red then green while its outline colour stays "
    "blue. Draw mode is FILLED, so only the fill change is visible",
)
def test_outline_independent_of_fill_color(conn: Connection, stage: Stage) -> None:
    handle = conn.stimuli.shapes.create_rect(
        params=RectParams(
            width_px=100,
            height_px=100,
            appearance=ShapeAppearance(fill_color=Color(1.0, 0.0, 0.0)),
        ),
    )
    conn.stimuli.shapes.set_outline_color(handle, Color(0.0, 0.0, 1.0))
    info = conn.stimuli.query(handle)
    assert info.fill_color.r == pytest.approx(1.0, abs=0.01)
    assert info.outline_color.b == pytest.approx(1.0, abs=0.01)
    stage.step("red fill (outline set to blue, but not drawn)", hold=0.5)

    conn.stimuli.set_fill_color(handle, Color(0.0, 1.0, 0.0))
    info = conn.stimuli.query(handle)
    assert info.fill_color.g == pytest.approx(1.0, abs=0.01)
    assert info.outline_color.b == pytest.approx(1.0, abs=0.01)
    stage.step("green fill — the outline colour underneath is still blue")
    conn.stimuli.delete(handle)
